//! 跨命令共享的运行时状态。

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use tauri::{AppHandle, Manager};

use crate::speech::{self, Speaking};
use crate::windows::LookupPayload;

#[derive(Default)]
pub struct AppState {
    /// 冷启动时前端还没挂上事件监听，先把查询暂存在这里。
    pending_lookup: Mutex<Option<LookupPayload>>,
    /// 当前进行中的 LLM 流。发起新查询时中止旧的，避免两股增量交错渲染。
    current_stream: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    /// 正在念的 `say` 子进程。同时只留一条，新的一条会掐掉旧的。
    speaking: Mutex<Option<Speaking>>,
    /// 朗读的代号发号器，用来分辨「自己那条」和「把自己换掉的那条」。
    speech_generation: AtomicU64,
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

/// 登记新的朗读并掐掉上一条，返回这一条的代号。
pub fn start_speaking(app: &AppHandle, child: tokio::process::Child) -> u64 {
    let state = app.state::<AppState>();
    let generation = state.speech_generation.fetch_add(1, Ordering::Relaxed) + 1;
    if let Ok(mut slot) = state.speaking.lock() {
        if let Some(mut previous) = slot.replace(Speaking { generation, child }) {
            // start_kill 不等回收，调用方不必 await 就能立刻发下一条。
            let _ = previous.child.start_kill();
        }
    }
    generation
}

/// 停掉正在念的那条。没有在念也不算错。
pub fn stop_speaking(app: &AppHandle) {
    if let Ok(mut slot) = app.state::<AppState>().speaking.lock() {
        if let Some(mut speaking) = slot.take() {
            let _ = speaking.child.start_kill();
        }
    }
}

/// 查某一代朗读的进展：还在念、已结束（念完 / 被停 / 被换走）、还是失败了。
pub fn poll_speech(app: &AppHandle, generation: u64) -> speech::Poll {
    match app.state::<AppState>().speaking.lock() {
        Ok(mut slot) => speech::poll(&mut slot, generation),
        // 锁毒化了就别再等了，否则这个任务会永远轮询下去。
        Err(_) => speech::Poll::Done,
    }
}
