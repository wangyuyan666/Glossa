//! 窗口显示与待处理查询的载荷类型。

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

pub const MAIN_LABEL: &str = "main";
pub const SETTINGS_LABEL: &str = "settings";

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LookupPayload {
    pub text: String,
    /// 选中文本所在的上下文。PopClip 取词拿不到，恒为 None；
    /// 自建取词层（见 AGENTS.md 的阶段二 TODO）会填。
    pub context: Option<String>,
}

/// 显示并聚焦某个常驻窗口。窗口都是启动时建好、平时隐藏，这里只负责唤出来。
pub fn show(app: &AppHandle, label: &str) -> Result<()> {
    let window = app
        .get_webview_window(label)
        .with_context(|| format!("找不到 {label} 窗口"))?;
    window.show()?;
    window.unminimize().ok();
    window.set_focus()?;
    crate::first_mouse::allow(&window);
    Ok(())
}

pub fn show_settings(app: &AppHandle) -> Result<()> {
    show(app, SETTINGS_LABEL)
}

pub fn show_main(app: &AppHandle) -> Result<()> {
    show(app, MAIN_LABEL)
}

/// 唤出主窗口并把查询送过去。取词层（`server.rs`）的落点。
///
/// 这里不能只调 `show_main`：查询是 PopClip 发起的，那一刻前台 app 是别人，
/// 而 macOS 15 收紧了跨 app 激活，后台进程光靠 `set_focus()` 抢不到前台——
/// 窗口会显示但压在别人下面，用户以为没反应。
///
/// 办法是先临时置顶，把窗口 order 到最前，再撤销置顶：窗口已经在前面了，
/// 撤销不会把它压回去，也就不会变成一个永远盖住别人的窗口。
pub fn present(app: &AppHandle, payload: &LookupPayload) -> Result<()> {
    let window = app
        .get_webview_window(MAIN_LABEL)
        .context("找不到 main 窗口")?;

    window.set_always_on_top(true)?;
    window.show()?;
    window.unminimize().ok();
    window.set_focus()?;
    window.set_always_on_top(false)?;
    crate::first_mouse::allow(&window);

    app.emit_to(MAIN_LABEL, "lookup", payload.clone())?;
    Ok(())
}
