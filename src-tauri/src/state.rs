//! 跨命令共享的运行时状态。

use std::sync::Mutex;

use tauri::{AppHandle, Manager};

use crate::windows::LookupPayload;

#[derive(Default)]
pub struct AppState {
    /// 冷启动时前端还没挂上事件监听，先把查询暂存在这里。
    pending_lookup: Mutex<Option<LookupPayload>>,
    /// 当前进行中的 LLM 流。发起新查询时中止旧的，避免两股增量交错渲染。
    current_stream: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
}

pub fn set_pending_lookup(app: &AppHandle, payload: LookupPayload) {
    if let Ok(mut slot) = app.state::<AppState>().pending_lookup.lock() {
        *slot = Some(payload);
    }
}

/// 取走暂存的查询（取后即清）。
pub fn take_pending_lookup(app: &AppHandle) -> Option<LookupPayload> {
    app.state::<AppState>()
        .pending_lookup
        .lock()
        .ok()
        .and_then(|mut slot| slot.take())
}

/// 登记新流并中止上一条。
pub fn replace_stream(app: &AppHandle, handle: tauri::async_runtime::JoinHandle<()>) {
    if let Ok(mut slot) = app.state::<AppState>().current_stream.lock() {
        if let Some(previous) = slot.replace(handle) {
            previous.abort();
        }
    }
}
