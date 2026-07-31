import { useEffect, useRef } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

import * as api from "../lib/api";
import {
  detectedMode,
  ExplanationCard,
  hasRenderableExplanation,
} from "./ExplanationCard";
import { SpeakButton } from "./SpeakButton";
import { ThinkingNote } from "./ThinkingNote";
import type { useLookup } from "./useLookup";

/** 模式由统一释义模型返回；旧历史没有 mode 时由卡片字段回退推断。 */
const MODE_LABELS = {
  word: "单词 / 短语释义",
  sentence: "英文句子释义",
  translate: "译成英文",
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

  /** mode 本身不是正文；至少有一块可见内容后才撤掉思考提示。 */
  const thinkingBeforeExplanation =
    phase === "explaining" && !hasRenderableExplanation(explanation);
  const displayMode = detectedMode(explanation);

  return (
    <div className="lookup" ref={bodyRef}>
      {phase === "idle" && <p className="lookup__hint">{idleHint}</p>}

      {/* 回显查询原文，当这一页的标题。划词进来的内容用户不一定记得自己选中了
          什么，句子模式下尤其需要对照着看。 */}
      {phase !== "idle" && word && (
        <header className="result__head">
          <h1 className="result__title">{word}</h1>
          {/* 单词和句子都是选中原文本身要念的；翻译模式的原文不是英文，念了没用。
              等 mode 到了再挂喇叭，免得先冒出来又缩回去。 */}
          {(displayMode === "word" || displayMode === "sentence") && (
            <SpeakButton text={word} label="朗读原文" />
          )}
          {/* mode 或独有字段还没到时不显示，避免流式开头先闪成默认 word。 */}
          {displayMode && <span className="badge">{MODE_LABELS[displayMode]}</span>}
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
