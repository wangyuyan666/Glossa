//! 本地取词监听。
//!
//! 只绑 `127.0.0.1`，不对外暴露。PopClip 扩展用 shell script action 打 `POST /lookup`。
//!
//! 用本地端口而不是 `enassistant://` deep link 的原因：macOS 不支持运行时注册 URL scheme，
//! deep link 只有安装到 `/Applications` 的打包 .app 才能测，`tauri dev` 下无法调试。

use anyhow::Result;
use axum::extract::{Form, Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::Router;
use serde::Deserialize;
use tauri::AppHandle;

use crate::popup::{self, LookupPayload};

#[derive(Deserialize)]
pub struct LookupParams {
    /// 选中的文本。
    q: String,
    /// 选中文本所在的上下文，阶段二取词层才会带。
    #[serde(default)]
    context: Option<String>,
}

pub fn spawn(app: AppHandle, port: u16) {
    tauri::async_runtime::spawn(async move {
        if let Err(e) = serve(app, port).await {
            eprintln!("[server] 取词监听启动失败（端口 {port} 可能被占用）: {e}");
        }
    });
}

async fn serve(app: AppHandle, port: u16) -> Result<()> {
    let router = Router::new()
        .route("/ping", get(|| async { "EnAssistant" }))
        .route("/lookup", get(lookup_get).post(lookup_post))
        .with_state(app);

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    eprintln!("[server] 取词监听已启动: http://127.0.0.1:{port}/lookup");
    axum::serve(listener, router).await?;
    Ok(())
}

async fn lookup_get(
    State(app): State<AppHandle>,
    Query(params): Query<LookupParams>,
) -> (StatusCode, &'static str) {
    handle(&app, params)
}

async fn lookup_post(
    State(app): State<AppHandle>,
    Form(params): Form<LookupParams>,
) -> (StatusCode, &'static str) {
    handle(&app, params)
}

fn handle(app: &AppHandle, params: LookupParams) -> (StatusCode, &'static str) {
    let text = params.q.trim().to_string();
    if text.is_empty() {
        return (StatusCode::BAD_REQUEST, "empty selection");
    }

    let payload = LookupPayload {
        text,
        context: params.context.filter(|c| !c.trim().is_empty()),
    };

    // 前端可能还没挂上监听（冷启动首次查询），先存一份供其挂载后主动取。
    crate::state::set_pending_lookup(app, payload.clone());

    match popup::present(app, &payload) {
        Ok(()) => (StatusCode::OK, "ok"),
        Err(e) => {
            eprintln!("[server] 弹窗失败: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "failed")
        }
    }
}
