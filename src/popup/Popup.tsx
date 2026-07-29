import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";

import * as api from "../lib/api";
import type { LookupPayload } from "../lib/types";
import { AskBox } from "../lookup/AskBox";
import { LookupView } from "../lookup/LookupView";
import { useLookup } from "../lookup/useLookup";
import "../lookup/lookup.css";
import "./popup.css";

export function Popup() {
  const lookup = useLookup("popup");
  const { start } = lookup;

  // 冷启动时 lookup 事件可能早于本组件挂载，所以先主动取一次暂存的查询。
  useEffect(() => {
    void api.takePendingLookup().then((payload) => {
      if (payload) start(payload.text, payload.context);
    });

    const unlisten = listen<LookupPayload>("lookup", ({ payload }) =>
      start(payload.text, payload.context),
    );
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [start]);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") void api.hidePopup();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  return (
    <div className="popup">
      <header className="popup__bar" data-tauri-drag-region>
        <span className="popup__word" data-tauri-drag-region>
          {lookup.word ?? "EnAssistant"}
        </span>
        <div className="popup__actions">
          <button type="button" title="主窗口" onClick={() => void api.openMain()}>
            ⌂
          </button>
          <button type="button" title="设置" onClick={() => void api.openSettings()}>
            ⚙
          </button>
          <button type="button" title="关闭 (Esc)" onClick={() => void api.hidePopup()}>
            ✕
          </button>
        </div>
      </header>

      <LookupView
        lookup={lookup}
        idleHint="在任意 app 里选中英文，点 PopClip 的 EnAssistant 按钮即可查询。"
      />

      <AskBox phase={lookup.phase} answering={lookup.answering} onAsk={lookup.ask} />
    </div>
  );
}
