import { useCallback, useRef, useState } from "react";

import * as api from "../lib/api";
import { parsePartialJson } from "../lib/jsonish";
import { startStream } from "../lib/stream";
import type { ChatMessage, Explanation, LookupDetail } from "../lib/types";

export type Phase = "idle" | "explaining" | "ready" | "error";

/**
 * 把释义卡片压成一段自然语言，作为追问会话的第一条 assistant 消息。
 *
 * 三种输出分支都要覆盖，否则某类查询的追问会丢掉上文。
 */
export function explanationAsText(exp: Partial<Explanation>, fallback: string): string {
  const lines: string[] = [];
  const text = (value: unknown) => (typeof value === "string" ? value : "");
  const records = (value: unknown): Record<string, unknown>[] =>
    Array.isArray(value)
      ? value.filter(
          (item): item is Record<string, unknown> =>
            item !== null && typeof item === "object" && !Array.isArray(item),
        )
      : [];

  const mode =
    exp.mode === "word" || exp.mode === "sentence" || exp.mode === "translate"
      ? exp.mode
      : null;

  // 旧历史没有 mode，仍覆盖三套字段；新输出只采用明确分支，忽略模型误带的其他字段。
  if (mode !== "translate") {
    const grammarIssue = text(exp.grammar?.issue);
    if (grammarIssue) {
      const corrected = text(exp.grammar?.corrected);
      const fix = corrected ? `\n改正：${corrected}` : "";
      lines.push(`语法问题：${grammarIssue}${fix}`);
    }
  }

  if (!mode || mode === "word") {
    const word = text(exp.word);
    if (word) lines.push(`${word} ${text(exp.phonetic)} ${text(exp.pos)}`.trim());
    if (text(exp.senseHere)) lines.push(text(exp.senseHere));
    if (text(exp.why)) lines.push(text(exp.why));
    const collocations = Array.isArray(exp.collocations)
      ? exp.collocations.filter((item): item is string => typeof item === "string")
      : [];
    if (collocations.length) lines.push(`常见搭配：${collocations.join("、")}`);
    if (exp.example && typeof exp.example === "object") {
      const exampleEn = text(exp.example.en);
      if (exampleEn) lines.push(`例句：${exampleEn} — ${text(exp.example.zh)}`);
    }
  }

  if (!mode || mode === "sentence") {
    if (text(exp.translation)) lines.push(`译文：${text(exp.translation)}`);
    if (text(exp.structure)) lines.push(text(exp.structure));
    const keyPoints = records(exp.keyPoints);
    if (keyPoints.length) {
      lines.push(keyPoints.map((point) => `${text(point.term)}：${text(point.note)}`).join("\n"));
    }
  }

  if (!mode || mode === "translate") {
    if (text(exp.english)) lines.push(`英文表达：${text(exp.english)}`);
    const wordChoice = records(exp.wordChoice);
    if (wordChoice.length) {
      lines.push(wordChoice.map((point) => `${text(point.term)}：${text(point.note)}`).join("\n"));
    }
    const alternatives = records(exp.alternatives);
    if (alternatives.length) {
      lines.push(
        `其他说法：${alternatives
          .map((alt) => {
            const when = text(alt.when);
            return `${text(alt.text)}${when ? `（${when}）` : ""}`;
          })
          .join("；")}`,
      );
    }
  }

  if (!lines.length) return fallback;
  if (mode) lines.unshift(`释义模式：${mode}`);
  return lines.join("\n");
}

function lookupAsText(text: string, context: string | null): string {
  return context ? `选中的内容：${text}\n\n所在原句：${context}` : `选中的内容：${text}`;
}

/** 一次查询的完整状态机：释义流式渲染 + 多轮追问 + 落库。 */
export function useLookup() {
  const [word, setWord] = useState<string | null>(null);
  const [phase, setPhase] = useState<Phase>("idle");
  const [error, setError] = useState<string | null>(null);

  const [raw, setRaw] = useState("");
  const [explanation, setExplanation] = useState<Partial<Explanation> | null>(null);

  /**
   * 推理模型的思考增量。
   *
   * 推理模型在吐正文之前可能思考十几秒，这段时间里骨架屏一动不动，看起来像卡死。
   * 攒在这里给「思考中…」用，正文一到就不再展示——它不是答案。
   */
  const [reasoning, setReasoning] = useState("");

  /** 释义构成的会话种子（user + assistant 各一条），追问时拼在最前面。 */
  const [seed, setSeed] = useState<ChatMessage[]>([]);
  const [turns, setTurns] = useState<ChatMessage[]>([]);
  const [answering, setAnswering] = useState<string | null>(null);

  /** 当前查询在历史库里的 id，追问的每一轮都往它下面追加。 */
  const lookupIdRef = useRef<string | null>(null);
  const cancelRef = useRef<(() => void) | null>(null);

  const reset = useCallback(() => {
    cancelRef.current?.();
    setError(null);
    setRaw("");
    setExplanation(null);
    setReasoning("");
    setSeed([]);
    setTurns([]);
    setAnswering(null);
  }, []);

  /** 发起一次新查询。 */
  const start = useCallback(
    (text: string, context: string | null) => {
      reset();
      setWord(text);
      setPhase("explaining");

      const lookupId = crypto.randomUUID();
      lookupIdRef.current = lookupId;

      let acc = "";
      cancelRef.current = startStream(
        (streamId) => api.explain(streamId, lookupId, text, context),
        {
          onDelta: (delta) => {
            acc += delta;
            setRaw(acc);
            const parsed = parsePartialJson<Explanation>(acc);
            if (parsed) setExplanation(parsed);
          },
          onReasoning: (delta) => setReasoning((prev) => prev + delta),
          onDone: () => {
            setReasoning("");
            const parsed = parsePartialJson<Explanation>(acc);
            if (parsed) setExplanation(parsed);
            setSeed([
              { role: "user", content: lookupAsText(text, context) },
              { role: "assistant", content: explanationAsText(parsed ?? {}, acc) },
            ]);
            setPhase("ready");
          },
          onError: (message) => {
            setReasoning("");
            setError(message);
            setPhase("error");
          },
        },
      );
    },
    [reset],
  );

  /** 从历史里恢复一次查询，不重新请求 LLM。 */
  const restore = useCallback(
    (detail: LookupDetail) => {
      reset();
      setWord(detail.text);
      lookupIdRef.current = detail.id;

      const parsed = parsePartialJson<Explanation>(detail.explanation);
      setRaw(detail.explanation);
      setExplanation(parsed);
      setSeed([
        { role: "user", content: lookupAsText(detail.text, detail.context) },
        {
          role: "assistant",
          content: explanationAsText(parsed ?? {}, detail.explanation),
        },
      ]);
      setTurns(detail.turns);
      setPhase("ready");
    },
    [reset],
  );

  const ask = useCallback(
    (question: string) => {
      const trimmed = question.trim();
      if (!trimmed || phase !== "ready" || answering !== null) return;

      const lookupId = lookupIdRef.current;
      const history: ChatMessage[] = [
        ...seed,
        ...turns,
        { role: "user", content: trimmed },
      ];
      setTurns((prev) => [...prev, { role: "user", content: trimmed }]);
      setAnswering("");
      setReasoning("");

      // 提问先落库：它和这条流的成败无关，流失败了问题本身也该留在历史里。
      if (lookupId) {
        void api.historyAppendTurn(lookupId, "user", trimmed).catch(() => {});
      }

      let acc = "";
      cancelRef.current?.();
      cancelRef.current = startStream((streamId) => api.chatTurn(streamId, history), {
        onDelta: (delta) => {
          acc += delta;
          setAnswering(acc);
        },
        onReasoning: (delta) => setReasoning((prev) => prev + delta),
        onDone: () => {
          setReasoning("");
          setTurns((prev) => [...prev, { role: "assistant", content: acc }]);
          setAnswering(null);
          if (lookupId) {
            void api.historyAppendTurn(lookupId, "assistant", acc).catch(() => {});
          }
        },
        onError: (message) => {
          setReasoning("");
          setTurns((prev) => [
            ...prev,
            { role: "assistant", content: `⚠️ ${message}` },
          ]);
          setAnswering(null);
        },
      });
    },
    [phase, answering, seed, turns],
  );

  return {
    word,
    phase,
    error,
    raw,
    explanation,
    reasoning,
    turns,
    answering,
    start,
    restore,
    ask,
  };
}
