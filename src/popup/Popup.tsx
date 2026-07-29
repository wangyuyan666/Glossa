import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

import * as api from "../lib/api";
import { parsePartialJson } from "../lib/jsonish";
import { startStream } from "../lib/stream";
import type { ChatMessage, Explanation, LookupPayload } from "../lib/types";
import { ExplanationCard } from "./ExplanationCard";
import "./popup.css";

type Phase = "idle" | "explaining" | "ready" | "error";

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

export function Popup() {
  const [lookup, setLookup] = useState<LookupPayload | null>(null);
  const [phase, setPhase] = useState<Phase>("idle");
  const [error, setError] = useState<string | null>(null);

  const [raw, setRaw] = useState("");
  const [explanation, setExplanation] = useState<Partial<Explanation> | null>(null);

  /** 释义构成的会话种子（user + assistant 各一条），追问时拼在最前面。 */
  const [seed, setSeed] = useState<ChatMessage[]>([]);
  const [turns, setTurns] = useState<ChatMessage[]>([]);
  const [answering, setAnswering] = useState<string | null>(null);
  const [input, setInput] = useState("");

  const cancelRef = useRef<(() => void) | null>(null);
  const bodyRef = useRef<HTMLDivElement>(null);

  const runLookup = useCallback((payload: LookupPayload) => {
    cancelRef.current?.();

    setLookup(payload);
    setPhase("explaining");
    setError(null);
    setRaw("");
    setExplanation(null);
    setSeed([]);
    setTurns([]);
    setAnswering(null);
    setInput("");

    let acc = "";
    cancelRef.current = startStream(
      (streamId) => api.explain(streamId, payload.text, payload.context),
      {
        onDelta: (text) => {
          acc += text;
          setRaw(acc);
          const parsed = parsePartialJson<Explanation>(acc);
          if (parsed) setExplanation(parsed);
        },
        onDone: () => {
          const parsed = parsePartialJson<Explanation>(acc);
          if (parsed) setExplanation(parsed);
          setSeed([
            { role: "user", content: lookupAsText(payload.text, payload.context) },
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
  }, []);

  // 冷启动时 lookup 事件可能早于本组件挂载，所以先主动取一次暂存的查询。
  useEffect(() => {
    void api.takePendingLookup().then((payload) => {
      if (payload) runLookup(payload);
    });

    const unlisten = listen<LookupPayload>("lookup", ({ payload }) => runLookup(payload));
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [runLookup]);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") void api.hidePopup();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  useEffect(() => {
    bodyRef.current?.scrollTo({ top: bodyRef.current.scrollHeight });
  }, [explanation, turns, answering]);

  const ask = useCallback(() => {
    const question = input.trim();
    if (!question || phase !== "ready" || answering !== null) return;

    const history: ChatMessage[] = [
      ...seed,
      ...turns,
      { role: "user", content: question },
    ];
    setTurns((prev) => [...prev, { role: "user", content: question }]);
    setInput("");
    setAnswering("");

    let acc = "";
    cancelRef.current?.();
    cancelRef.current = startStream((streamId) => api.chatTurn(streamId, history), {
      onDelta: (text) => {
        acc += text;
        setAnswering(acc);
      },
      onDone: () => {
        setTurns((prev) => [...prev, { role: "assistant", content: acc }]);
        setAnswering(null);
      },
      onError: (message) => {
        setTurns((prev) => [
          ...prev,
          { role: "assistant", content: `⚠️ ${message}` },
        ]);
        setAnswering(null);
      },
    });
  }, [input, phase, answering, seed, turns]);

  return (
    <div className="popup">
      <header className="popup__bar" data-tauri-drag-region>
        <span className="popup__word" data-tauri-drag-region>
          {lookup?.text ?? "EnAssistant"}
        </span>
        <div className="popup__actions">
          <button type="button" title="设置" onClick={() => void api.openSettings()}>
            ⚙
          </button>
          <button type="button" title="关闭 (Esc)" onClick={() => void api.hidePopup()}>
            ✕
          </button>
        </div>
      </header>

      <div className="popup__body" ref={bodyRef}>
        {phase === "idle" && (
          <p className="popup__hint">
            在任意 app 里选中英文，点 PopClip 的 EnAssistant 按钮即可查询。
          </p>
        )}

        {phase === "error" && (
          <div className="popup__error">
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

      <footer className="popup__ask">
        <input
          value={input}
          placeholder={phase === "ready" ? "继续追问…" : "等释义出完再追问"}
          disabled={phase !== "ready" || answering !== null}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.nativeEvent.isComposing) ask();
          }}
        />
        <button
          type="button"
          onClick={ask}
          disabled={phase !== "ready" || answering !== null || !input.trim()}
        >
          发送
        </button>
      </footer>
    </div>
  );
}
