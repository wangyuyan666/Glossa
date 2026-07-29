import { useState } from "react";

import type { Phase } from "./useLookup";

interface Props {
  phase: Phase;
  answering: string | null;
  onAsk: (question: string) => void;
}

/** 追问输入框。释义出完之前禁用——没有释义做上下文，追问没有意义。 */
export function AskBox({ phase, answering, onAsk }: Props) {
  const [input, setInput] = useState("");
  const disabled = phase !== "ready" || answering !== null;

  const submit = () => {
    if (disabled || !input.trim()) return;
    onAsk(input);
    setInput("");
  };

  return (
    <div className="askbox">
      <input
        value={input}
        placeholder={phase === "ready" ? "继续追问…" : "等释义出完再追问"}
        disabled={disabled}
        onChange={(e) => setInput(e.target.value)}
        onKeyDown={(e) => {
          // 中文输入法组词途中回车是在选字，不该当作发送。
          if (e.key === "Enter" && !e.nativeEvent.isComposing) submit();
        }}
      />
      <button type="button" onClick={submit} disabled={disabled || !input.trim()}>
        发送
      </button>
    </div>
  );
}
