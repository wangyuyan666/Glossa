import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import * as api from "../lib/api";
import type { LookupPayload } from "../lib/types";
import { AskBox } from "../lookup/AskBox";
import { LookupView } from "../lookup/LookupView";
import { useLookup } from "../lookup/useLookup";
import { HistorySidebar, useHistory } from "./HistorySidebar";
import "../lookup/lookup.css";
import "./main.css";

export function Main() {
  const lookup = useLookup();
  const history = useHistory();
  const { start } = lookup;

  const [input, setInput] = useState("");
  const [activeId, setActiveId] = useState<string | null>(null);
  const [configured, setConfigured] = useState(true);

  useEffect(() => {
    void api.getSettings().then((s) => setConfigured(s.fast !== null));
  }, []);

  // 划词触发的查询。冷启动时事件可能早于本组件挂载，所以先主动取一次暂存的。
  useEffect(() => {
    void api.takePendingLookup().then((payload) => {
      if (payload) {
        setActiveId(null);
        start(payload.text, payload.context);
      }
    });

    const unlisten = listen<LookupPayload>("lookup", ({ payload }) => {
      setActiveId(null);
      start(payload.text, payload.context);
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [start]);

  // 侧栏的刷新由后端的 history-updated 广播驱动，见 useHistory。

  const submit = () => {
    const text = input.trim();
    if (!text) return;
    setActiveId(null);
    lookup.start(text, null);
    setInput("");
  };

  const pick = (id: string) => {
    void api.historyGet(id).then((detail) => {
      if (!detail) return;
      setActiveId(id);
      lookup.restore(detail);
    });
  };

  const remove = (id: string) => {
    void api.historyDelete(id).then(() => {
      if (id === activeId) setActiveId(null);
      history.reload();
    });
  };

  const clear = () => {
    void api.historyClear().then(() => {
      setActiveId(null);
      history.reload();
    });
  };

  return (
    <div className="app">
      <HistorySidebar
        items={history.items}
        activeId={activeId}
        query={history.query}
        onQueryChange={history.setQuery}
        onPick={pick}
        onDelete={remove}
        onClear={clear}
      />

      <main className="workspace">
        {!configured && (
          <div className="banner">
            <span>还没配置模型，查询无法进行。</span>
            <button type="button" onClick={() => void api.openSettings()}>
              去设置
            </button>
          </div>
        )}

        <div className="workspace__query">
          <input
            value={input}
            placeholder="输入单词、短语或整句，回车查询"
            autoFocus
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.nativeEvent.isComposing) submit();
            }}
          />
          <button type="button" onClick={submit} disabled={!input.trim()}>
            查询
          </button>
          <button type="button" title="设置" onClick={() => void api.openSettings()}>
            ⚙
          </button>
        </div>

        <LookupView
          lookup={lookup}
          idleHint="在上方输入要查的词，或在任意 app 里划词用 PopClip 触发。查过的词会出现在左侧。"
        />

        <AskBox phase={lookup.phase} answering={lookup.answering} onAsk={lookup.ask} />
      </main>
    </div>
  );
}
