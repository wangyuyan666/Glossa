mod config;
mod history;
mod llm;
mod popclip;
mod popup;
mod prompts;
mod server;
mod state;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WindowEvent};

use config::{Provider, Role, Settings};
use llm::{ChatMessage, ChatRequest, Delta};
use popup::{LookupPayload, MAIN_LABEL, POPUP_LABEL, SETTINGS_LABEL};
use state::AppState;

/// 前端监听的流式事件名。
const STREAM_EVENT: &str = "llm-stream";
/// 历史有新记录时广播，主窗口据此刷新侧栏。
/// 没有它的话，从弹窗查的词不会出现在已经打开的主窗口里。
const HISTORY_EVENT: &str = "history-updated";

const EXPLAIN_MAX_TOKENS: u32 = 800;
const CHAT_MAX_TOKENS: u32 = 1500;

#[derive(Serialize, Clone)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum StreamEvent {
    Delta { stream_id: String, text: String },
    Done { stream_id: String },
    Error { stream_id: String, message: String },
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
        max_tokens: 16,
        temperature: 0.0,
    };

    let (tx, mut rx) = tokio::sync::mpsc::channel::<Delta>(16);
    let stream = llm::stream(&provider, req, tx);

    let collect = async {
        let mut out = String::new();
        while let Some(Delta::Text(t)) = rx.recv().await {
            out.push_str(&t);
        }
        out
    };

    let (result, text) = tokio::join!(stream, collect);
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
    /// "popup" | "main"，标记这次查询从哪个入口发起。
    source: String,
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
    source: String,
) -> Result<(), String> {
    let settings = config::load(&app);
    let (provider, model) = settings
        .resolve(Role::Fast)
        .ok_or("尚未配置「释义」模型，请先到设置里配置")?;

    let req = ChatRequest {
        model: model.to_string(),
        system: Some(prompts::explain_system(
            &settings.native_language,
            context.as_deref(),
        )),
        messages: vec![ChatMessage {
            role: "user".into(),
            content: prompts::explain_user(&text, context.as_deref()),
        }],
        max_tokens: EXPLAIN_MAX_TOKENS,
        temperature: 0.2,
    };

    let persist = PersistLookup {
        id: lookup_id,
        text,
        context,
        source,
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

    let req = ChatRequest {
        model: model.to_string(),
        system: Some(prompts::chat_system(&settings.native_language)),
        messages,
        max_tokens: CHAT_MAX_TOKENS,
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
                    while let Some(Delta::Text(text)) = rx.recv().await {
                        full.push_str(&text);
                        emit(
                            &app,
                            StreamEvent::Delta {
                                stream_id: stream_id.clone(),
                                text,
                            },
                        );
                    }
                    full
                }
            });

            // llm::stream 返回时 tx 被丢弃，forwarder 随之结束。
            let result = llm::stream(&provider, req, tx).await;
            let full = forwarder.await.unwrap_or_default();

            match result {
                Ok(()) => {
                    if let Some(p) = persist {
                        // 落库失败不该影响用户看到释义，记日志即可。
                        match history::save_lookup(
                            &history::state(&app),
                            &p.id,
                            &p.text,
                            p.context.as_deref(),
                            &full,
                            &p.source,
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

/// 广播而不是只发给弹窗——主窗口也要收。前端按 streamId 分发，串不了台。
fn emit(app: &AppHandle, event: StreamEvent) {
    if let Err(e) = app.emit(STREAM_EVENT, event) {
        eprintln!("[emit] 发送流事件失败: {e}");
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
fn hide_popup(app: AppHandle) -> Result<(), String> {
    popup::popup_window(&app)
        .and_then(|w| w.hide().map_err(Into::into))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn open_settings(app: AppHandle) -> Result<(), String> {
    popup::show_settings(&app).map_err(|e| e.to_string())
}

#[tauri::command]
fn open_main(app: AppHandle) -> Result<(), String> {
    popup::show_main(&app).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------- 入口

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .setup(|app| {
            let handle = app.handle().clone();
            let settings = config::load(&handle);

            app.manage(history::History::open(&handle)?);
            server::spawn(handle.clone(), settings.port);

            // 主窗口是应用的门面，启动就打开。没配模型时它自己会挂提示条引导去设置，
            // 比一上来甩个设置页更符合预期。
            popup::show_main(&handle)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // 三个窗口都是常驻隐藏，关闭按钮只隐藏不销毁，避免下次用到要重建 webview。
            if let WindowEvent::CloseRequested { api, .. } = event {
                let label = window.label();
                if label == POPUP_LABEL || label == SETTINGS_LABEL || label == MAIN_LABEL {
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
            history_list,
            history_get,
            history_append_turn,
            history_delete,
            history_clear,
            install_popclip_extension,
            popclip_installed,
            popclip_snippet,
            hide_popup,
            open_settings,
            open_main,
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app, event| {
            // 点 Dock 图标要能把主窗口唤回来。窗口是隐藏不销毁的，
            // 没有这段的话关掉主窗口后就再也打不开了。
            if let tauri::RunEvent::Reopen { .. } = event {
                let _ = popup::show_main(app);
            }
        });
}
