//! 让 webview 接受「app 未激活时的第一次点击」。
//!
//! NSWindow 默认 `acceptsFirstMouse: NO`：app 不在前台时，落到窗口上的第一次点击
//! 只用来激活 app，**不派发**给下层视图，React 的 onClick 根本不触发。症状是任何控件
//! 第一下都没反应、点第二下才行，最容易被误认成某个按钮的 bug。
//!
//! `tauri.conf.json` 里的 `acceptFirstMouse: true` 在这个场景下不可靠：
//!
//! - tao 给窗口的 content view 覆写了 `acceptsFirstMouse:` 返回 YES；
//! - wry 给 `WryWebView`（WKWebView 的子类）也覆写了，按配置返回内部 ivar；
//! - 实测 AppKit 问的就是 `WryWebView`，但只靠配置时第一次点击仍会被吞。这是上游已有的
//!   「窗口按不同路径显示时行为不一致」问题，不是 React 事件问题。
//!
//! 上游还没修：[wry#637](https://github.com/tauri-apps/wry/issues/637)、
//! [tauri#6781](https://github.com/tauri-apps/tauri/issues/6781)、
//! [tauri#4316](https://github.com/tauri-apps/tauri/issues/4316)。wry 0.56 也没有。
//!
//! 所以在这里自己动手：拿到 webview 的 NSView，找到视图树里真正的 `WKWebView`
//! 实例，把它所属类的 `acceptsFirstMouse:` 实现替换成恒返回 YES。实测 AppKit 在首次
//! 点击时问的就是 wry 的 `WryWebView`，替换后同一次点击既激活 app，也到达 React。
//!
//! 代价与风险，改之前先看清楚：
//!
//! - 查找依赖 `WKWebView` 的公开类关系，不依赖 WebKit 私有子视图名；macOS / wry 若改了
//!   宿主层级，这里会静默失效、退回「点两下」的老行为，不会崩。所以配置里的
//!   `acceptFirstMouse` 留着，哪天上游修好了两边指向同一个行为。
//! - 替换是**类级别**的，影响进程内所有 `WryWebView` 实例。本 app 只有这两个 webview。
//! - 激活那一下的点击会真的生效（手正好落在「清空历史」上就真清了）。这是这个方案的
//!   固有代价，对划词工具这笔账划算。
//!
//! 验证只能用真实的 `CGEventPost` 点击：AppleScript 的 `click at` 走 AX，绕过整个
//! first-mouse 行为，测不出来。

#[cfg(target_os = "macos")]
mod imp {
    use std::collections::HashSet;
    use std::ffi::CStr;

    use objc2::ffi::class_replaceMethod;
    use objc2::runtime::{AnyClass, AnyObject, Bool, Imp, Sel};
    use objc2::{msg_send, sel};

    /// 视图树的遍历深度上限。WKWebView 内部就那么几层，给足余量即可，
    /// 有上限是为了万一碰到成环的层级不会栈溢出。
    const MAX_DEPTH: u32 = 12;

    /// `BOOL acceptsFirstMouse:(NSEvent *)` 的类型编码。
    ///
    /// arm64 上 `BOOL` 就是 C 的 `bool`（编码 `B`），x86_64 上是 `signed char`（编码 `c`）。
    /// 这串只是给运行时做内省用的元数据，但既然要填就填对。
    #[cfg(target_arch = "aarch64")]
    const TYPES: &CStr = c"B@:@";
    #[cfg(not(target_arch = "aarch64"))]
    const TYPES: &CStr = c"c@:@";

    extern "C-unwind" fn accepts_first_mouse(
        _this: *mut AnyObject,
        _cmd: Sel,
        _event: *mut AnyObject,
    ) -> Bool {
        Bool::YES
    }

    /// 找出 `view` 子树里的 WKWebView，把其所属类的 `acceptsFirstMouse:` 换成恒 YES。
    ///
    /// # Safety
    ///
    /// `view` 必须是有效的 `NSView *`，且只能在主线程调用（AppKit 的硬要求）。
    pub unsafe fn patch(view: *mut AnyObject) {
        let Some(wkwebview) = AnyClass::get(c"WKWebView") else {
            return;
        };
        let mut patched = HashSet::new();
        walk(view, wkwebview, &mut patched, 0);
    }

    unsafe fn walk(
        view: *mut AnyObject,
        wkwebview: &AnyClass,
        patched: &mut HashSet<usize>,
        depth: u32,
    ) {
        if view.is_null() || depth > MAX_DEPTH {
            return;
        }

        let is_webview: Bool = msg_send![view, isKindOfClass: wkwebview];
        if is_webview.as_bool() {
            let class: *const AnyClass = msg_send![view, class];
            // 同一个类只需替换一次；main/settings 的 webview 属于同一个 WryWebView 类。
            if !class.is_null() && patched.insert(class as usize) {
                let imp: Imp = std::mem::transmute::<
                    extern "C-unwind" fn(*mut AnyObject, Sel, *mut AnyObject) -> Bool,
                    Imp,
                >(accepts_first_mouse);
                class_replaceMethod(
                    class as *mut AnyClass,
                    sel!(acceptsFirstMouse:),
                    imp,
                    TYPES.as_ptr(),
                );
            }
        }

        let subviews: *mut AnyObject = msg_send![view, subviews];
        if subviews.is_null() {
            return;
        }
        let count: usize = msg_send![subviews, count];
        for i in 0..count {
            let subview: *mut AnyObject = msg_send![subviews, objectAtIndex: i];
            walk(subview, wkwebview, patched, depth + 1);
        }
    }
}

/// 同步给某个窗口的 webview 放行首次点击。
///
/// 只能在主线程调用。启动时必须在 `show()` **之前**走这里：若先显示、再用
/// `run_on_main_thread` 异步排队，用户点得快就能赶在补丁前，症状仍是第一次被吞。
#[cfg(target_os = "macos")]
pub fn allow_now(window: &tauri::WebviewWindow) {
    match window.ns_view() {
        Ok(view) => unsafe { imp::patch(view as *mut objc2::runtime::AnyObject) },
        Err(e) => eprintln!("[first_mouse] 拿不到 NSView，跳过: {e}"),
    }
}

/// 从任意线程给窗口放行首次点击。
///
/// 常规 `show()` / `present()` 路径走这里做防御性重补。类方法替换本身幂等。
#[cfg(target_os = "macos")]
pub fn allow(window: &tauri::WebviewWindow) {
    let window = window.clone();
    let dispatched = window
        .clone()
        .run_on_main_thread(move || allow_now(&window));
    if let Err(e) = dispatched {
        eprintln!("[first_mouse] 派发到主线程失败，跳过: {e}");
    }
}

#[cfg(not(target_os = "macos"))]
pub fn allow_now(_window: &tauri::WebviewWindow) {}

#[cfg(not(target_os = "macos"))]
pub fn allow(_window: &tauri::WebviewWindow) {}
