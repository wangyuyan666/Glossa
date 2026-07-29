//! 弹窗的显示与定位。

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, LogicalPosition, Manager, WebviewWindow};

pub const POPUP_LABEL: &str = "popup";
pub const SETTINGS_LABEL: &str = "settings";

/// 光标与弹窗左上角的偏移，避免窗口正好压住选区。
const CURSOR_OFFSET_X: f64 = 16.0;
const CURSOR_OFFSET_Y: f64 = 18.0;
/// 贴边时与屏幕边缘的最小留白。
const SCREEN_MARGIN: f64 = 8.0;
/// 窗口从未显示过时 `outer_size()` 可能返回 0，用 tauri.conf.json 里的尺寸兜底，
/// 否则首次查询算出来的边界是错的，弹窗会掉出屏幕底部。
const FALLBACK_SIZE: (f64, f64) = (420.0, 520.0);

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LookupPayload {
    pub text: String,
    /// 选中文本所在的上下文。阶段一取词层拿不到，恒为 None；阶段二自建取词层会填。
    pub context: Option<String>,
}

pub fn popup_window(app: &AppHandle) -> Result<WebviewWindow> {
    app.get_webview_window(POPUP_LABEL)
        .context("找不到 popup 窗口")
}

/// 把弹窗移到光标附近并显示、聚焦。
pub fn present(app: &AppHandle, payload: &LookupPayload) -> Result<()> {
    let window = popup_window(app)?;
    if let Err(e) = position_at_cursor(app, &window) {
        // 定位失败不该挡住查询本身，退回窗口原位置。
        eprintln!("[popup] 定位失败，使用原位置: {e}");
    }
    window.show()?;
    window.unminimize().ok();
    window.set_focus()?;
    app.emit_to(POPUP_LABEL, "lookup", payload.clone())?;
    Ok(())
}

/// 矩形，逻辑坐标。
#[derive(Clone, Copy, Debug, PartialEq)]
struct Rect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

/// 算出弹窗左上角该放哪：光标右下方偏一点，越界则拉回屏幕内。
///
/// 纯函数，方便对边界情况做断言——真机上没法把鼠标挪到屏幕角落来测。
fn place(cursor: (f64, f64), size: (f64, f64), monitor: Rect) -> (f64, f64) {
    let (width, height) = size;

    let min_x = monitor.x + SCREEN_MARGIN;
    let min_y = monitor.y + SCREEN_MARGIN;
    let max_x = monitor.x + monitor.width - width - SCREEN_MARGIN;
    let max_y = monitor.y + monitor.height - height - SCREEN_MARGIN;

    // max 可能小于 min（窗口比屏幕还大），先 min 后 max 保证不越过左上边界。
    let x = (cursor.0 + CURSOR_OFFSET_X).min(max_x).max(min_x);
    let y = (cursor.1 + CURSOR_OFFSET_Y).min(max_y).max(min_y);
    (x, y)
}

fn position_at_cursor(app: &AppHandle, window: &WebviewWindow) -> Result<()> {
    let cursor = app.cursor_position()?;
    let scale = window.scale_factor().unwrap_or(1.0);

    // 统一在逻辑坐标下计算，避免多屏不同缩放时算错。
    let cursor_logical = (cursor.x / scale, cursor.y / scale);

    let measured = window.outer_size()?.to_logical::<f64>(scale);
    let size = (
        if measured.width > 1.0 {
            measured.width
        } else {
            FALLBACK_SIZE.0
        },
        if measured.height > 1.0 {
            measured.height
        } else {
            FALLBACK_SIZE.1
        },
    );

    // monitor_from_point 收的是逻辑坐标，而 cursor_position 给的是物理坐标——
    // 传错会让光标偏下时落在逻辑屏幕范围之外，返回 None，钳制被整个跳过，弹窗掉出屏幕。
    let monitor = window
        .monitor_from_point(cursor_logical.0, cursor_logical.1)
        .ok()
        .flatten()
        .or_else(|| window.current_monitor().ok().flatten())
        .or_else(|| window.primary_monitor().ok().flatten());

    let (x, y) = match monitor {
        Some(monitor) => {
            let m_scale = monitor.scale_factor();
            let m_pos = monitor.position().to_logical::<f64>(m_scale);
            let m_size = monitor.size().to_logical::<f64>(m_scale);
            place(
                cursor_logical,
                size,
                Rect {
                    x: m_pos.x,
                    y: m_pos.y,
                    width: m_size.width,
                    height: m_size.height,
                },
            )
        }
        // 一个显示器都问不到就不钳制，至少还落在光标旁边。
        _ => (
            cursor_logical.0 + CURSOR_OFFSET_X,
            cursor_logical.1 + CURSOR_OFFSET_Y,
        ),
    };

    window.set_position(LogicalPosition::new(x, y))?;
    Ok(())
}

pub fn show_settings(app: &AppHandle) -> Result<()> {
    let window = app
        .get_webview_window(SETTINGS_LABEL)
        .context("找不到 settings 窗口")?;
    window.show()?;
    window.unminimize().ok();
    window.set_focus()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect {
        x: 0.0,
        y: 0.0,
        width: 1920.0,
        height: 1080.0,
    };
    const POPUP: (f64, f64) = (420.0, 520.0);

    #[test]
    fn places_below_right_of_cursor_when_there_is_room() {
        assert_eq!(place((800.0, 300.0), POPUP, SCREEN), (816.0, 318.0));
    }

    #[test]
    fn pulls_back_inside_when_cursor_is_near_bottom_right() {
        let (x, y) = place((1900.0, 1070.0), POPUP, SCREEN);
        assert_eq!((x, y), (1492.0, 552.0));
        assert!(x + POPUP.0 <= SCREEN.width);
        assert!(y + POPUP.1 <= SCREEN.height);
    }

    #[test]
    fn respects_monitor_origin_on_a_secondary_display() {
        let right = Rect {
            x: 1920.0,
            ..SCREEN
        };
        let (x, y) = place((3800.0, 1070.0), POPUP, right);
        assert!(x >= right.x && x + POPUP.0 <= right.x + right.width);
        assert!(y + POPUP.1 <= right.height);
    }

    #[test]
    fn keeps_top_left_visible_when_popup_is_larger_than_screen() {
        let tiny = Rect {
            width: 300.0,
            height: 300.0,
            ..SCREEN
        };
        assert_eq!(place((100.0, 100.0), POPUP, tiny), (8.0, 8.0));
    }
}
