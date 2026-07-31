import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import * as api from "../lib/api";
import type { LookupSummary } from "../lib/types";
import { IconClose, IconSearch, IconSidebar, IconTrash } from "../ui/icons";

interface Props {
  items: LookupSummary[];
  activeId: string | null;
  query: string;
  onQueryChange: (query: string) => void;
  onPick: (id: string) => void;
  onDelete: (id: string) => void;
  onClear: () => void;
  onToggle: () => void;
}

/**
 * 侧栏开关。展开时挂在侧栏顶部、收起时挂在查询栏行首，两处共用同一个按钮，
 * 位置上正好接得住——收起后工作区的左边缘就是原来侧栏的位置。
 */
export function SidebarToggle({ collapsed, onToggle }: { collapsed: boolean; onToggle: () => void }) {
  return (
    <button
      type="button"
      className="plain sidebar-toggle"
      title={`${collapsed ? "展开" : "收起"}侧栏 ⌘\\`}
      aria-label={`${collapsed ? "展开" : "收起"}侧栏`}
      aria-expanded={!collapsed}
      onClick={onToggle}
    >
      <IconSidebar />
    </button>
  );
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

/** 按自然日分组的组名。跨天要靠日历判断，不能拿「距今 24 小时」算今天。 */
function dayLabel(ms: number): string {
  const then = new Date(ms);
  const now = new Date();
  const midnight = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate());
  // 按两边各自的零点相减，才不会因为「今天凌晨」正好等于零点而被算成昨天。
  const days = Math.round(
    (midnight(now).getTime() - midnight(then).getTime()) / 86_400_000,
  );

  if (days <= 0) return "今天";
  if (days === 1) return "昨天";
  if (days < 7) return "本周";
  return then.toLocaleDateString(undefined, { year: "numeric", month: "long" });
}

/** 列表已按时间倒序，顺着切成连续的日期段即可，不用再排序。 */
function groupByDay(items: LookupSummary[]): { label: string; items: LookupSummary[] }[] {
  const groups: { label: string; items: LookupSummary[] }[] = [];
  for (const item of items) {
    const label = dayLabel(item.createdAt);
    const last = groups[groups.length - 1];
    if (last?.label === label) last.items.push(item);
    else groups.push({ label, items: [item] });
  }
  return groups;
}

export function HistorySidebar({
  items,
  activeId,
  query,
  onQueryChange,
  onPick,
  onDelete,
  onClear,
  onToggle,
}: Props) {
  const [confirmingClear, setConfirmingClear] = useState(false);
  const searchRef = useRef<HTMLInputElement>(null);

  // 离开侧栏或列表变化后撤销待确认状态，免得确认按钮一直挂在那里。
  useEffect(() => {
    if (!confirmingClear) return;
    const timer = setTimeout(() => setConfirmingClear(false), 4000);
    return () => clearTimeout(timer);
  }, [confirmingClear]);

  // ⌘K 聚焦搜索框。搜索框上标了这个快捷键，不实现就是骗人。
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        searchRef.current?.focus();
        searchRef.current?.select();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <aside className="sidebar">
      <header className="brand">
        <img className="brand__logo" src="/glossa-logo.png" alt="" />
        <div className="brand__text">
          <span className="brand__name">Glossa</span>
          <span className="brand__tagline">英语学习 · 句子助手</span>
        </div>
        <SidebarToggle collapsed={false} onToggle={onToggle} />
      </header>

      <div className="sidebar__search">
        <div className="field">
          <IconSearch className="field__icon" />
          <input
            ref={searchRef}
            value={query}
            placeholder="搜索历史记录"
            onChange={(e) => onQueryChange(e.target.value)}
          />
          {query ? (
            <button
              type="button"
              className="plain"
              title="清除搜索"
              onClick={() => onQueryChange("")}
            >
              <IconClose />
            </button>
          ) : (
            <kbd>⌘K</kbd>
          )}
        </div>
      </div>

      <div className="sidebar__list">
        {items.length === 0 && (
          <p className="sidebar__empty">{query ? "没有匹配的记录" : "还没有查询记录"}</p>
        )}

        {groupByDay(items).map((group) => (
          <section key={group.label}>
            <h2 className="sidebar__group">{group.label}</h2>
            <ul className="sidebar__items">
              {group.items.map((item) => (
                <li
                  key={item.id}
                  className={`entry${item.id === activeId ? " entry--active" : ""}`}
                >
                  <button
                    type="button"
                    className="entry__main"
                    onClick={() => onPick(item.id)}
                  >
                    <span className="entry__word">{item.text}</span>
                    <span className="entry__row">
                      <span className="entry__sense">{item.sense}</span>
                      <span className="entry__meta">{relativeTime(item.createdAt)}</span>
                    </span>
                  </button>
                  <button
                    type="button"
                    className="entry__delete"
                    title="删除这条记录"
                    onClick={() => onDelete(item.id)}
                  >
                    <IconClose />
                  </button>
                </li>
              ))}
            </ul>
          </section>
        ))}
      </div>

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
            <IconTrash />
            清空历史记录
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
