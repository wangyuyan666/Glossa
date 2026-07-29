mod config;
mod llm;
mod popup;
mod prompts;
mod server;
mod state;

use serde::Serialize;
use tauri::{AppHandle, Emitter, WindowEvent};

use config::{Provider, Role, Settings};
use llm::{ChatMessage, ChatRequest, Delta};
use popup::{LookupPayload, POPUP_LABEL, SETTINGS_LABEL};
use state::AppState;

/// 前端监听的流式事件名。
const STREAM_EVENT: &str = "llm-stream";

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

/// 释义。走 `fast` 角色，输出受提示词约束的 JSON，由前端容错增量解析。
#[tauri::command]
fn explain(
    app: AppHandle,
    stream_id: String,
    text: String,
    context: Option<String>,
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

    spawn_stream(app.clone(), stream_id, provider.clone(), req);
    Ok(())
}

/// 追问。走 `chat` 角色，`messages` 是前端维护的完整会话历史。
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

    spawn_stream(app.clone(), stream_id, provider.clone(), req);
    Ok(())
}

/// 起一条流：LLM 增量经 channel 转成事件发给弹窗，结束或出错各发一个终止事件。
fn spawn_stream(app: AppHandle, stream_id: String, provider: Provider, req: ChatRequest) {
    let handle = tauri::async_runtime::spawn({
        let app = app.clone();
        async move {
            let (tx, mut rx) = tokio::sync::mpsc::channel::<Delta>(64);

            let forwarder = tauri::async_runtime::spawn({
                let app = app.clone();
                let stream_id = stream_id.clone();
                async move {
                    while let Some(Delta::Text(text)) = rx.recv().await {
                        emit(
                            &app,
                            StreamEvent::Delta {
                                stream_id: stream_id.clone(),
                                text,
                            },
                        );
                    }
                }
            });

            // llm::stream 返回时 tx 被丢弃，forwarder 随之结束。
            let result = llm::stream(&provider, req, tx).await;
            let _ = forwarder.await;

            match result {
                Ok(()) => emit(&app, StreamEvent::Done { stream_id }),
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

fn emit(app: &AppHandle, event: StreamEvent) {
    if let Err(e) = app.emit_to(POPUP_LABEL, STREAM_EVENT, event) {
        eprintln!("[emit] 发送流事件失败: {e}");
    }
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

// ---------------------------------------------------------------- 入口

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .setup(|app| {
            let handle = app.handle().clone();
            let settings = config::load(&handle);

            server::spawn(handle.clone(), settings.port);

            // 没有可用的释义配置说明是首次启动，直接把用户领到设置页。
            if settings.resolve(Role::Fast).is_none() {
                popup::show_settings(&handle)?;
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // 两个窗口都是常驻隐藏，关闭按钮只隐藏不销毁，避免下次查询要重建 webview。
            if let WindowEvent::CloseRequested { api, .. } = event {
                let label = window.label();
                if label == POPUP_LABEL || label == SETTINGS_LABEL {
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
            hide_popup,
            open_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
