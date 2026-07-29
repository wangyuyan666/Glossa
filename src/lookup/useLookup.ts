import { useCallback, useRef, useState } from "react";

import * as api from "../lib/api";
import { parsePartialJson } from "../lib/jsonish";
import { startStream } from "../lib/stream";
import type { ChatMessage, Explanation, LookupDetail } from "../lib/types";

export type Phase = "idle" | "explaining" | "ready" | "error";

/** 查询的发起入口，落库时记下来。 */
export type Source = "popup" | "main";

/** 把释义卡片压成一段自然语言，作为追问会话的第一条 assistant 消息。 */
function explanationAsText(exp: Partial<Explanation>, fallback: string): string {
  const lines: string[] = [];
  if (exp.word) lines.push(`${exp.word} ${exp.phonetic ?? ""} ${exp.pos ?? ""}`.trim());
  if (exp.senseHere) lines.push(exp.senseHere);
  if (exp.why) lines.push(exp.why);
  if (exp.collocations?.length) lines.push(`常见搭配：${exp.collocations.join("、")}`);
  if (exp.example?.en) lines.push(`例句：${exp.example.en} — ${exp.example.zh ?? ""}`);
  return lines.length ? lines.join("\n") : fallback;
}

function lookupAsText(text: string, context: string | null): string {
  return context ? `选中的词：${text}\n\n所在原句：${context}` : `选中的词：${text}`;
}

/**
 * 一次查询的完整状态机：释义流式渲染 + 多轮追问。
 *
 * 弹窗和主窗口共用这一份——两边各写一遍的话，行为迟早会走偏。
 */
export function useLookup(source: Source) {
  const [word, setWord] = useState<string | null>(null);
  const [phase, setPhase] = useState<Phase>("idle");
  const [error, setError] = useState<string | null>(null);

  const [raw, setRaw] = useState("");
  const [explanation, setExplanation] = useState<Partial<Explanation> | null>(null);

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
        (streamId) => api.explain(streamId, lookupId, text, context, source),
        {
          onDelta: (delta) => {
            acc += delta;
            setRaw(acc);
            const parsed = parsePartialJson<Explanation>(acc);
            if (parsed) setExplanation(parsed);
          },
          onDone: () => {
            const parsed = parsePartialJson<Explanation>(acc);
            if (parsed) setExplanation(parsed);
            setSeed([
              { role: "user", content: lookupAsText(text, context) },
              { role: "assistant", content: explanationAsText(parsed ?? {}, acc) },
            ]);
            setPhase("ready");
          },
          onError: (message) => {
            setError(message);
            setPhase("error");
          },
        },
      );
    },
    [reset, source],
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
        onDone: () => {
          setTurns((prev) => [...prev, { role: "assistant", content: acc }]);
          setAnswering(null);
          if (lookupId) {
            void api.historyAppendTurn(lookupId, "assistant", acc).catch(() => {});
          }
        },
        onError: (message) => {
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
    turns,
    answering,
    start,
    restore,
    ask,
  };
}
