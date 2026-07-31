/**
 * 补回被输入法吞掉的第一次点击。
 *
 * macOS 上输入法处于组词态（拼音还没上屏）时，落在 webview 上的第一次 mousedown 被
 * 输入法拿去结束组词，**不转发**给 WKWebView。web 层只收到 mouseup，浏览器不会凭
 * 一个 mouseup 合成 click，React 的 onClick 于是根本不触发。症状是「在输入框里打了字
 * 之后，点任何控件都要点两下」——最容易被误认成某个按钮的 bug，实测用 Rime 输入法
 * 打半个拼音再点设置齿轮，第一下的事件序列只有 `compositionend` + `mouseup`。
 *
 * 这和 `first_mouse.rs` 治的是两回事：那个是 app 未激活时窗口吞掉首次点击，这个是
 * app 已在前台、纯输入法路径。两个都修完才没有「第一下不算数」。
 *
 * 办法：孤儿 mouseup（前面没有配对的 mousedown）就是被吞掉的那一下，给它补一个
 * click。浏览器自己补了的话就不重复派发。
 */
export function installImeClickRecovery() {
  let sawDown = false;
  let sawClick = false;

  addEventListener("mousedown", () => {
    sawDown = true;
  }, true);

  addEventListener("click", () => {
    sawClick = true;
  }, true);

  addEventListener(
    "mouseup",
    (e) => {
      const paired = sawDown;
      sawDown = false;
      if (paired || e.button !== 0) return;

      // 用 Element 而不是 HTMLElement：内联 SVG 图标是 SVGElement，图标按钮的
      // target 落在 <svg>/<path> 上，卡 HTMLElement 会把纯图标按钮全漏掉。
      const target = e.target;
      if (!(target instanceof Element)) return;

      sawClick = false;
      // 退到下一个任务再补：浏览器若自己派发了 click，它会先到，这里就不该重复。
      setTimeout(() => {
        if (sawClick) return;
        // 原生顺序是 mousedown → 聚焦 → click，落在文本框上时先把光标带过去，
        // 否则用户看到的是「点了别的输入框但光标没动」。
        const field = target.closest("input, textarea, select, [contenteditable]");
        if (field instanceof HTMLElement) field.focus();
        target.dispatchEvent(
          new MouseEvent("click", {
            bubbles: true,
            cancelable: true,
            composed: true,
            button: 0,
            clientX: e.clientX,
            clientY: e.clientY,
          }),
        );
      }, 0);
    },
    true,
  );
}
