import { useEffect, useRef } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

import * as api from "../lib/api";
import { ExplanationCard, modeOf } from "./ExplanationCard";
import { ThinkingNote } from "./ThinkingNote";
import type { useLookup } from "./useLookup";

/**
 * 判定用了哪套提示词是在 Rust 侧做的，前端看不到结果，只能从回来的字段反推。
 * 告诉用户「它被当成什么处理了」——查一个词却走了句子模板时，这是唯一的线索。
 */
const MODE_LABELS = {
  word: "检测为单词或短语",
  sentence: "检测为英文句子",
  translate: "检测为非英文，译成英文",
} as const;

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
  const { word, phase, error, raw, explanation, reasoning, turns, answering } = lookup;
  const bodyRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bodyRef.current?.scrollTo({ top: bodyRef.current.scrollHeight });
  }, [explanation, turns, answering]);

  /** 释义流还在思考、正文一个字都没到的时候才显示——正文一来它就不该占地方了。 */
  const thinkingBeforeExplanation = phase === "explaining" && !explanation;

  return (
    <div className="lookup" ref={bodyRef}>
      {phase === "idle" && <p className="lookup__hint">{idleHint}</p>}

      {/* 回显查询原文，当这一页的标题。划词进来的内容用户不一定记得自己选中了
          什么，句子模式下尤其需要对照着看。 */}
      {phase !== "idle" && word && (
        <header className="result__head">
          <h1 className="result__title">{word}</h1>
          {/* 字段还没到齐时不显示：这时候推断出来的模式可能是错的。 */}
          {explanation && <span className="badge">{MODE_LABELS[modeOf(explanation)]}</span>}
        </header>
      )}

      {phase === "error" && (
        <div className="lookup__error">
          <p>{error}</p>
          <button type="button" onClick={() => void api.openSettings()}>
            打开设置
          </button>
        </div>
      )}

      {/* 思考期间用它顶替骨架屏：同样是「在跑」的信号，但看得出跑到哪了。 */}
      {thinkingBeforeExplanation && reasoning ? (
        <ThinkingNote text={reasoning} />
      ) : (
        (phase === "explaining" || phase === "ready") && (
          <ExplanationCard
            explanation={explanation}
            streaming={phase === "explaining"}
            raw={raw}
          />
        )
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
          ) : reasoning ? (
            <ThinkingNote text={reasoning} />
          ) : (
            <span className="dots">···</span>
          )}
        </div>
      )}
    </div>
  );
}
