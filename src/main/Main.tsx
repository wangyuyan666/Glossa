import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import * as api from "../lib/api";
import * as speech from "../lib/speech";
import type { LookupPayload } from "../lib/types";
import { AskBox } from "../lookup/AskBox";
import { LookupView } from "../lookup/LookupView";
import { useLookup } from "../lookup/useLookup";
import { IconClose, IconGear, IconSearch } from "../ui/icons";
import { HistorySidebar, SidebarToggle, useHistory } from "./HistorySidebar";
import "../lookup/lookup.css";
import "./main.css";

const COLLAPSED_KEY = "glossa.sidebarCollapsed";

export function Main() {
  const lookup = useLookup();
  const history = useHistory();
  const { start } = lookup;

  const [input, setInput] = useState("");
  const [activeId, setActiveId] = useState<string | null>(null);
  const [configured, setConfigured] = useState(true);

  // 收起状态只是窗口布局偏好，没必要走后端 settings，放 localStorage 就够。
  const [collapsed, setCollapsed] = useState(
    () => localStorage.getItem(COLLAPSED_KEY) === "1",
  );

  useEffect(() => {
    localStorage.setItem(COLLAPSED_KEY, collapsed ? "1" : "0");
  }, [collapsed]);

  // ⌘\ 开关侧栏，和按钮 title 上标的一致。
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "\\") {
        e.preventDefault();
        setCollapsed((v) => !v);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // 设置窗口保存后不会通知过来，所以每次窗口重新获得焦点都重读一遍——
  // 从设置切回来就是这个时机。拿不到 focus 事件时退化成「重启后生效」，
  // 和加发音之前的行为一样，不会更糟。
  useEffect(() => {
    const apply = () => {
      void api.getSettings().then((s) => {
        setConfigured(s.fast !== null);
        speech.configure({ voice: s.voice, rate: s.speechRate });
      });
      // 嗓子列表用来兜「配置里那个嗓子已经被卸载了」，见 speech.speak。
      speech.loadVoices();
    };
    apply();
    window.addEventListener("focus", apply);
    return () => window.removeEventListener("focus", apply);
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
    <div className={`app${collapsed ? " app--collapsed" : ""}`}>
      {!collapsed && (
        <HistorySidebar
          items={history.items}
          activeId={activeId}
          query={history.query}
          onQueryChange={history.setQuery}
          onPick={pick}
          onDelete={remove}
          onClear={clear}
          onToggle={() => setCollapsed(true)}
        />
      )}

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
          {collapsed && <SidebarToggle collapsed onToggle={() => setCollapsed(false)} />}
          <div className="field">
            <IconSearch className="field__icon" />
            <input
              value={input}
              placeholder="输入单词、短语或整句，回车查询"
              autoFocus
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && !e.nativeEvent.isComposing) submit();
              }}
            />
            {input && (
              <button
                type="button"
                className="plain"
                title="清空输入"
                onClick={() => setInput("")}
              >
                <IconClose />
              </button>
            )}
          </div>
          <button type="button" className="primary" onClick={submit} disabled={!input.trim()}>
            查询
            <kbd>⏎</kbd>
          </button>
          <button type="button" onClick={() => void api.openSettings()}>
            <IconGear />
            设置
          </button>
        </div>

        <div className="surface workspace__result">
          <LookupView
            lookup={lookup}
            idleHint="在上方输入要查的词或句子，或在任意 app 里划词用 PopClip 触发。"
          />
        </div>

        <AskBox phase={lookup.phase} answering={lookup.answering} onAsk={lookup.ask} />
      </main>
    </div>
  );
}
