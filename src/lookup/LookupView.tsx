import { useEffect, useRef } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

import * as api from "../lib/api";
import { ExplanationCard } from "./ExplanationCard";
import type { useLookup } from "./useLookup";

interface Props {
  lookup: ReturnType<typeof useLookup>;
  /** 还没发起任何查询时显示的引导文案，两个窗口各说各的。 */
  idleHint: string;
}

/**
 * 释义卡片 + 追问对话的滚动区。
 *
 * 只负责渲染，状态全在 useLookup 里；输入框由各窗口自己摆，因为弹窗贴底、
 * 主窗口在顶部，位置不一样。
 */
export function LookupView({ lookup, idleHint }: Props) {
  const { phase, error, raw, explanation, turns, answering } = lookup;
  const bodyRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bodyRef.current?.scrollTo({ top: bodyRef.current.scrollHeight });
  }, [explanation, turns, answering]);

  return (
    <div className="lookup" ref={bodyRef}>
      {phase === "idle" && <p className="lookup__hint">{idleHint}</p>}

      {phase === "error" && (
        <div className="lookup__error">
          <p>{error}</p>
          <button type="button" onClick={() => void api.openSettings()}>
            打开设置
          </button>
        </div>
      )}

      {(phase === "explaining" || phase === "ready") && (
        <ExplanationCard
          explanation={explanation}
          streaming={phase === "explaining"}
          raw={raw}
        />
      )}

      {turns.map((turn, i) => (
        <div key={i} className={`turn turn--${turn.role}`}>
          {turn.role === "user" ? (
            <p>{turn.content}</p>
          ) : (
            <ReactMarkdown remarkPlugins={[remarkGfm]}>{turn.content}</ReactMarkdown>
          )}
        </div>
      ))}

      {answering !== null && (
        <div className="turn turn--assistant">
          {answering ? (
            <ReactMarkdown remarkPlugins={[remarkGfm]}>{answering}</ReactMarkdown>
          ) : (
            <span className="dots">···</span>
          )}
        </div>
      )}
    </div>
  );
}
