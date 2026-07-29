import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import * as api from "../lib/api";
import type { LookupSummary } from "../lib/types";

interface Props {
  items: LookupSummary[];
  activeId: string | null;
  query: string;
  onQueryChange: (query: string) => void;
  onPick: (id: string) => void;
  onDelete: (id: string) => void;
  onClear: () => void;
}

function relativeTime(ms: number): string {
  const diff = Date.now() - ms;
  const minute = 60_000;
  const hour = 60 * minute;
  const day = 24 * hour;

  if (diff < minute) return "刚刚";
  if (diff < hour) return `${Math.floor(diff / minute)} 分钟前`;
  if (diff < day) return `${Math.floor(diff / hour)} 小时前`;
  if (diff < 7 * day) return `${Math.floor(diff / day)} 天前`;
  return new Date(ms).toLocaleDateString();
}

export function HistorySidebar({
  items,
  activeId,
  query,
  onQueryChange,
  onPick,
  onDelete,
  onClear,
}: Props) {
  const [confirmingClear, setConfirmingClear] = useState(false);

  // 离开侧栏或列表变化后撤销待确认状态，免得确认按钮一直挂在那里。
  useEffect(() => {
    if (!confirmingClear) return;
    const timer = setTimeout(() => setConfirmingClear(false), 4000);
    return () => clearTimeout(timer);
  }, [confirmingClear]);

  return (
    <aside className="sidebar">
      <div className="sidebar__search">
        <input
          value={query}
          placeholder="搜索历史"
          onChange={(e) => onQueryChange(e.target.value)}
        />
      </div>

      <ul className="sidebar__list">
        {items.length === 0 && (
          <li className="sidebar__empty">{query ? "没有匹配的记录" : "还没有查询记录"}</li>
        )}

        {items.map((item) => (
          <li
            key={item.id}
            className={`entry${item.id === activeId ? " entry--active" : ""}`}
          >
            <button type="button" className="entry__main" onClick={() => onPick(item.id)}>
              <span className="entry__word">{item.text}</span>
              {item.sense && <span className="entry__sense">{item.sense}</span>}
              <span className="entry__meta">{relativeTime(item.createdAt)}</span>
            </button>
            <button
              type="button"
              className="entry__delete"
              title="删除这条记录"
              onClick={() => onDelete(item.id)}
            >
              ✕
            </button>
          </li>
        ))}
      </ul>

      <div className="sidebar__footer">
        {confirmingClear ? (
          <>
            <button
              type="button"
              className="danger"
              onClick={() => {
                setConfirmingClear(false);
                onClear();
              }}
            >
              确认清空
            </button>
            <button type="button" onClick={() => setConfirmingClear(false)}>
              取消
            </button>
          </>
        ) : (
          <button
            type="button"
            disabled={items.length === 0}
            onClick={() => setConfirmingClear(true)}
          >
            清空历史
          </button>
        )}
      </div>
    </aside>
  );
}

/** 侧栏数据的加载与增删，抽出来免得主窗口组件里塞满 CRUD。 */
export function useHistory() {
  const [items, setItems] = useState<LookupSummary[]>([]);
  const [query, setQuery] = useState("");

  const reload = useCallback(() => {
    void api
      .historyList(200, 0, query.trim() || null)
      .then(setItems)
      .catch(() => setItems([]));
  }, [query]);

  // 输入防抖，避免每敲一个字符打一次库。
  useEffect(() => {
    const timer = setTimeout(reload, 150);
    return () => clearTimeout(timer);
  }, [reload]);

  // 从弹窗查的词也要出现在这里，所以监听后端的落库广播，
  // 而不是只在本窗口自己的查询结束时刷新。
  useEffect(() => {
    const unlisten = listen("history-updated", () => reload());
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [reload]);

  return { items, query, setQuery, reload };
}
