mod config;
mod first_mouse;
mod history;
mod llm;
mod popclip;
mod prompts;
mod server;
mod state;
mod templates;
mod windows;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WindowEvent};

use config::{Provider, Role, Settings};
use llm::{ChatMessage, ChatRequest, Delta};
use state::AppState;
use templates::{PromptTemplate, TemplateIssue, TemplateKind};
use windows::{LookupPayload, MAIN_LABEL, SETTINGS_LABEL};

/// 前端监听的流式事件名。
const STREAM_EVENT: &str = "llm-stream";
/// 历史有新记录时广播，主窗口据此刷新侧栏。
/// 没有它的话，从弹窗查的词不会出现在已经打开的主窗口里。
const HISTORY_EVENT: &str = "history-updated";

// 额度要按**推理模型**来给：思考 token 也算 completion token，额度不够时思考写满就
// 截断，正文一个字都没有（实际踩过：800 token 全被思考吃光，界面一片空白）。
// 释义正文本身只要 ~300 token，多出来的都是留给思考的余量。
//
// 默认值卡在 4000：4096 是相当一部分端点（老一代模型、各类中转）的输出硬上限，
// 超了直接 HTTP 400。端点撑得住更多的，在 provider 里单独配 `maxTokens` 抬高。
const DEFAULT_EXPLAIN_MAX_TOKENS: u32 = 4000;
const DEFAULT_CHAT_MAX_TOKENS: u32 = 4000;

/// 流正常结束但没有正文时给用户看的提示。
///
/// 静默出一张空卡片是最坏的失败方式——用户看不出是模型的问题还是 app 坏了。
const EMPTY_STREAM_MESSAGE: &str =
    "模型没有返回任何正文。若用的是推理模型，可能是额度全花在思考上了，换个模型或重试一次";

#[derive(Serialize, Clone)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum StreamEvent {
    Delta {
        stream_id: String,
        text: String,
    },
    /// 推理模型的思考增量。前端拿它显示「思考中…」，不参与 JSON 解析。
    Reasoning {
        stream_id: String,
        text: String,
    },
    Done {
        stream_id: String,
    },
    Error {
        stream_id: String,
        message: String,
    },
}

// ---------------------------------------------------------------- 配置命令

#[tauri::command]
fn get_settings(app: AppHandle) -> Settings {
    config::load(&app)
}

#[tauri::command]
fn save_settings(app: AppHandle, settings: Settings) -> Result<(), String> {
    config::save(&app, &settings).map_err(|e| e.to_string())
}

#[tauri::command]
fn settings_file_path(app: AppHandle) -> String {
    config::settings_path(&app)
        .map(|p| p.display().to_string())
        .unwrap_or_default()
}

/// 「测试连接」：发一个最小请求，返回模型的回声内容。
#[tauri::command]
async fn test_provider(provider: Provider, model: String) -> Result<String, String> {
    let req = ChatRequest {
        model,
        system: None,
        messages: vec![ChatMessage {
            role: "user".into(),
            content: "Reply with the single word: OK".into(),
        }],
        // 回声只要 1 个 token，但推理模型会先思考一大段——给 16 的话思考就把额度写满，
        // 测试连接永远失败，还会报成「检查模型名是否正确」这种指错方向的提示。
        max_tokens: provider.max_tokens_or(DEFAULT_EXPLAIN_MAX_TOKENS),
        temperature: 0.0,
    };

    let (tx, mut rx) = tokio::sync::mpsc::channel::<Delta>(16);
    let stream = llm::stream(&provider, req, tx);

    let (result, text) = tokio::join!(stream, collect_text(&mut rx));
    result.map_err(|e| e.to_string())?;

    let text = text.trim().to_string();
    if text.is_empty() {
        Err("连接成功但模型没有返回内容，检查模型名是否正确".into())
    } else {
        Ok(text)
    }
}

#[tauri::command]
async fn list_models(provider: Provider) -> Result<Vec<String>, String> {
    llm::list_models(&provider).await.map_err(|e| e.to_string())
}

// ---------------------------------------------------------------- 查询命令

#[tauri::command]
fn take_pending_lookup(app: AppHandle) -> Option<LookupPayload> {
    state::take_pending_lookup(&app)
}

/// 释义流跑完后要落库的信息。
///
/// 完整释义文本 Rust 侧本来就有（`spawn_stream` 收齐了所有 delta），
/// 所以落库放在这里，不用让前端把文本再传回来一次。
struct PersistLookup {
    id: String,
    text: String,
    context: Option<String>,
}

/// 释义。走 `fast` 角色，输出受提示词约束的 JSON，由前端容错增量解析。
///
/// `lookup_id` 由前端生成，后续的追问用同一个 id 往 `turns` 里追加，
/// 这样一条历史点开就是完整会话。
#[tauri::command]
fn explain(
    app: AppHandle,
    stream_id: String,
    lookup_id: String,
    text: String,
    context: Option<String>,
) -> Result<(), String> {
    let settings = config::load(&app);
    let (provider, model) = settings
        .resolve(Role::Fast)
        .ok_or("尚未配置「释义」模型，请先到设置里配置")?;

    let template = settings.template(prompts::kind_for(&text));
    let req = ChatRequest {
        model: model.to_string(),
        system: Some(templates::render(
            &template.body,
            &settings.native_language,
            context.as_deref(),
        )),
        messages: vec![ChatMessage {
            role: "user".into(),
            content: prompts::explain_user(&text, context.as_deref()),
        }],
        max_tokens: provider.max_tokens_or(DEFAULT_EXPLAIN_MAX_TOKENS),
        temperature: 0.2,
    };

    let persist = PersistLookup {
        id: lookup_id,
        text,
        context,
    };
    spawn_stream(app.clone(), stream_id, provider.clone(), req, Some(persist));
    Ok(())
}

/// 追问。走 `chat` 角色，`messages` 是前端维护的完整会话历史。
///
/// 这里不落库——对话轮次由前端在每轮结束后调 `history_append_turn` 追加，
/// 因为用户提问那一条在流开始前就该入库，和流的成败无关。
#[tauri::command]
fn chat_turn(app: AppHandle, stream_id: String, messages: Vec<ChatMessage>) -> Result<(), String> {
    let settings = config::load(&app);
    let (provider, model) = settings
        .resolve(Role::Chat)
        .ok_or("尚未配置「对话」模型，请先到设置里配置")?;

    let template = settings.template(TemplateKind::Chat);
    let req = ChatRequest {
        model: model.to_string(),
        system: Some(templates::render(
            &template.body,
            &settings.native_language,
            None,
        )),
        messages,
        max_tokens: provider.max_tokens_or(DEFAULT_CHAT_MAX_TOKENS),
        temperature: 0.6,
    };

    spawn_stream(app.clone(), stream_id, provider.clone(), req, None);
    Ok(())
}

/// 起一条流：LLM 增量经 channel 转成事件广播给所有窗口，结束或出错各发一个终止事件。
fn spawn_stream(
    app: AppHandle,
    stream_id: String,
    provider: Provider,
    req: ChatRequest,
    persist: Option<PersistLookup>,
) {
    let handle = tauri::async_runtime::spawn({
        let app = app.clone();
        async move {
            let (tx, mut rx) = tokio::sync::mpsc::channel::<Delta>(64);

            // forwarder 顺带把全文攒起来，落库要用。
            let forwarder = tauri::async_runtime::spawn({
                let app = app.clone();
                let stream_id = stream_id.clone();
                async move {
                    let mut full = String::new();
                    while let Some(delta) = rx.recv().await {
                        let event = match delta {
                            // 只有正文进 full：落库和前端解析用的都是它。
                            Delta::Text(text) => {
                                full.push_str(&text);
                                StreamEvent::Delta {
                                    stream_id: stream_id.clone(),
                                    text,
                                }
                            }
                            Delta::Reasoning(text) => StreamEvent::Reasoning {
                                stream_id: stream_id.clone(),
                                text,
                            },
                        };
                        emit(&app, event);
                    }
                    full
                }
            });

            // llm::stream 返回时 tx 被丢弃，forwarder 随之结束。
            let result = llm::stream(&provider, req, tx).await;
            let full = forwarder.await.unwrap_or_default();

            match result {
                // 流跑完了但正文是空的（推理模型把额度花在思考上、或端点吐了个空回复）。
                // 当成错误报出来，也不落库——历史里存一条空释义，点开还是空白。
                Ok(()) if full.trim().is_empty() => emit(
                    &app,
                    StreamEvent::Error {
                        stream_id,
                        message: EMPTY_STREAM_MESSAGE.to_string(),
                    },
                ),
                Ok(()) => {
                    if let Some(p) = persist {
                        // 落库失败不该影响用户看到释义，记日志即可。
                        match history::save_lookup(
                            &history::state(&app),
                            &p.id,
                            &p.text,
                            p.context.as_deref(),
                            &full,
                        ) {
                            Ok(()) => {
                                let _ = app.emit(HISTORY_EVENT, ());
                            }
                            Err(e) => eprintln!("[history] 写入查询记录失败: {e}"),
                        }
                    }
                    emit(&app, StreamEvent::Done { stream_id })
                }
                Err(e) => emit(
                    &app,
                    StreamEvent::Error {
                        stream_id,
                        message: e.to_string(),
                    },
                ),
            }
        }
    });

    state::replace_stream(&app, handle);
}

/// 收干一条流的正文，思考增量丢弃。
///
/// 「测试连接」和「实测模板」都只关心正文。注意必须收到 channel 关闭为止：
/// 提前 return 会让 `llm::stream` 那侧的 `tx.send` 失败，它会当成接收端没了而中断拉流。
async fn collect_text(rx: &mut tokio::sync::mpsc::Receiver<Delta>) -> String {
    let mut out = String::new();
    while let Some(delta) = rx.recv().await {
        if let Delta::Text(text) = delta {
            out.push_str(&text);
        }
    }
    out
}

/// 广播给所有窗口，前端按 streamId 分发。
fn emit(app: &AppHandle, event: StreamEvent) {
    if let Err(e) = app.emit(STREAM_EVENT, event) {
        eprintln!("[emit] 发送流事件失败: {e}");
    }
}

// ---------------------------------------------------------------- 模板命令

/// 内置模板。不落配置文件，每次从代码里取——升级后用户立刻拿到新版内置提示词。
#[tauri::command]
fn builtin_templates() -> Vec<PromptTemplate> {
    templates::builtins()
}

/// 模板正文里可用的变量，设置页拿去做说明表。
#[tauri::command]
fn template_variables() -> Vec<String> {
    templates::VARIABLES.iter().map(|v| v.to_string()).collect()
}

/// 静态检查。本地、免费、瞬时，输入时就能跑。
#[tauri::command]
fn check_template(kind: TemplateKind, body: String) -> Vec<TemplateIssue> {
    templates::check(kind, &body)
}

/// 「实测一次」的结果。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct TemplateProbe {
    /// 模型的原始输出，让用户肉眼判断质量。
    raw: String,
    /// 释义类是否解析出了合法 JSON。对话类恒为 true（本来就没有格式要求）。
    parsed: bool,
    /// 契约里有、但模型没输出的字段。
    missing_fields: Vec<String>,
}

/// 实测：拿固定样例真发一次请求。
///
/// 静态检查只能防拼写错误，**「模型不听你的」只有实测能发现**——那是自定义提示词
/// 最常见的失败方式。所以这不是锦上添花，是用户唯一的自查手段。
#[tauri::command]
async fn probe_template(
    app: AppHandle,
    kind: TemplateKind,
    body: String,
) -> Result<TemplateProbe, String> {
    let settings = config::load(&app);
    // 释义类用 fast 角色，对话类用 chat 角色，和真实调用保持一致。
    let role = match kind {
        TemplateKind::Chat => Role::Chat,
        _ => Role::Fast,
    };
    let (provider, model) = settings
        .resolve(role)
        .ok_or("尚未配置模型，请先在上方配置并绑定角色")?;

    let user = match kind {
        TemplateKind::Chat => prompts::probe_input(kind).to_string(),
        _ => prompts::explain_user(prompts::probe_input(kind), None),
    };

    let req = ChatRequest {
        model: model.to_string(),
        system: Some(templates::render(&body, &settings.native_language, None)),
        messages: vec![ChatMessage {
            role: "user".into(),
            content: user,
        }],
        max_tokens: provider.max_tokens_or(DEFAULT_EXPLAIN_MAX_TOKENS),
        temperature: 0.2,
    };

    let (tx, mut rx) = tokio::sync::mpsc::channel::<Delta>(64);
    let stream = llm::stream(provider, req, tx);
    let (result, raw) = tokio::join!(stream, collect_text(&mut rx));
    result.map_err(|e| e.to_string())?;

    Ok(evaluate_probe(kind, raw))
}

/// 把模型输出对着字段契约核一遍。抽出来是为了能脱离网络做单测。
fn evaluate_probe(kind: TemplateKind, raw: String) -> TemplateProbe {
    let required = templates::required_fields(kind);
    if required.is_empty() {
        // 对话类没有契约，拿到内容就算通过。
        return TemplateProbe {
            parsed: !raw.trim().is_empty(),
            raw,
            missing_fields: Vec::new(),
        };
    }

    // 模型常给 JSON 裹代码块，和释义主流程一样先剥掉再解析。
    let trimmed = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    match serde_json::from_str::<serde_json::Value>(trimmed) {
        Ok(value) => {
            let missing = required
                .iter()
                .filter(|field| value.get(**field).is_none())
                .map(|field| field.to_string())
                .collect();
            TemplateProbe {
                raw,
                parsed: true,
                missing_fields: missing,
            }
        }
        Err(_) => TemplateProbe {
            raw,
            parsed: false,
            missing_fields: required.iter().map(|f| f.to_string()).collect(),
        },
    }
}

// ---------------------------------------------------------------- 历史命令

#[tauri::command]
fn history_list(
    app: AppHandle,
    limit: u32,
    offset: u32,
    query: Option<String>,
) -> Result<Vec<history::LookupSummary>, String> {
    history::list(&history::state(&app), limit, offset, query.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
fn history_get(app: AppHandle, id: String) -> Result<Option<history::LookupDetail>, String> {
    history::get(&history::state(&app), &id).map_err(|e| e.to_string())
}

#[tauri::command]
fn history_append_turn(
    app: AppHandle,
    lookup_id: String,
    role: String,
    content: String,
) -> Result<(), String> {
    history::append_turn(&history::state(&app), &lookup_id, &role, &content)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn history_delete(app: AppHandle, id: String) -> Result<(), String> {
    history::delete(&history::state(&app), &id).map_err(|e| e.to_string())
}

#[tauri::command]
fn history_clear(app: AppHandle) -> Result<(), String> {
    history::clear(&history::state(&app)).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------- 取词命令

/// 生成 `.popclipext` 扩展并交给 PopClip 打开，由用户在 PopClip 的确认框里完成安装。
#[tauri::command]
fn install_popclip_extension(app: AppHandle) -> Result<popclip::InstallOutcome, String> {
    popclip::install(&app).map_err(|e| e.to_string())
}

/// 设置页据此决定是显示一键安装还是「未检测到 PopClip」。
#[tauri::command]
fn popclip_installed() -> bool {
    popclip::find_popclip().is_some()
}

/// 手动安装用的 snippet 文本。生成逻辑放在 Rust 侧，和一键安装共用同一份端口与 identifier。
#[tauri::command]
fn popclip_snippet(app: AppHandle) -> String {
    popclip::snippet(config::load(&app).port)
}

// ---------------------------------------------------------------- 窗口命令

#[tauri::command]
fn open_settings(app: AppHandle) -> Result<(), String> {
    windows::show_settings(&app).map_err(|e| e.to_string())
}

#[tauri::command]
fn open_main(app: AppHandle) -> Result<(), String> {
    windows::show_main(&app).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------- 入口

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // 必须在最前面注册：第二次启动不再创建另一套窗口，只唤回已有主窗口。
        // 本 app 还占固定本地端口；没有单实例时，第二进程端口失败但窗口照开，
        // 两套一模一样的窗口会让点击行为和终端日志完全对不上。
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let _ = windows::show_main(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .setup(|app| {
            let handle = app.handle().clone();
            let settings = config::load(&handle);

            app.manage(history::History::open(&handle)?);
            server::spawn(handle.clone(), settings.port);

            // setup 在 AppKit 主线程执行。必须在任何窗口 show 之前同步补：先显示再异步排队，
            // 用户点得快就能赶在补丁前，第一次点击仍会被系统吞掉。
            for label in [MAIN_LABEL, SETTINGS_LABEL] {
                if let Some(window) = app.get_webview_window(label) {
                    first_mouse::allow_now(&window);
                }
            }

            // 主窗口是应用的门面，启动就打开。没配模型时它自己会挂提示条引导去设置，
            // 比一上来甩个设置页更符合预期。
            windows::show_main(&handle)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // 两个窗口都是常驻隐藏，关闭按钮只隐藏不销毁，避免下次用到要重建 webview。
            if let WindowEvent::CloseRequested { api, .. } = event {
                let label = window.label();
                if label == MAIN_LABEL || label == SETTINGS_LABEL {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            settings_file_path,
            test_provider,
            list_models,
            take_pending_lookup,
            explain,
            chat_turn,
            builtin_templates,
            template_variables,
            check_template,
            probe_template,
            history_list,
            history_get,
            history_append_turn,
            history_delete,
            history_clear,
            install_popclip_extension,
            popclip_installed,
            popclip_snippet,
            open_settings,
            open_main,
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app, event| {
            // 点 Dock 图标要能把主窗口唤回来。窗口是隐藏不销毁的，
            // 没有这段的话关掉主窗口后就再也打不开了。
            if let tauri::RunEvent::Reopen { .. } = event {
                let _ = windows::show_main(app);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_reports_missing_fields() {
        let probe = evaluate_probe(
            TemplateKind::Sentence,
            r#"{"translation":"译文"}"#.to_string(),
        );
        assert!(probe.parsed);
        assert_eq!(probe.missing_fields, vec!["structure", "keyPoints"]);
    }

    #[test]
    fn probe_accepts_a_complete_response() {
        let probe = evaluate_probe(
            TemplateKind::Sentence,
            r#"{"translation":"a","structure":"b","keyPoints":[]}"#.to_string(),
        );
        assert!(probe.parsed);
        assert!(probe.missing_fields.is_empty());
    }

    #[test]
    fn probe_strips_code_fences_like_the_real_path_does() {
        let probe = evaluate_probe(
            TemplateKind::Sentence,
            "```json\n{\"translation\":\"a\",\"structure\":\"b\",\"keyPoints\":[]}\n```"
                .to_string(),
        );
        assert!(probe.parsed, "裹了代码块也该判成功: {:?}", probe.raw);
    }

    #[test]
    fn probe_flags_output_that_is_not_json_at_all() {
        // 用户把释义模板写成了自由文本要求——这正是实测要抓的那类失败。
        let probe = evaluate_probe(TemplateKind::Word, "take on 的意思是承担。".to_string());
        assert!(!probe.parsed);
        assert_eq!(probe.missing_fields.len(), 7);
    }

    #[test]
    fn chat_probe_has_no_contract() {
        let probe = evaluate_probe(TemplateKind::Chat, "随便一段回答".to_string());
        assert!(probe.parsed);
        assert!(probe.missing_fields.is_empty());
    }

    #[test]
    fn chat_probe_fails_on_empty_output() {
        assert!(!evaluate_probe(TemplateKind::Chat, "   ".to_string()).parsed);
    }
}
