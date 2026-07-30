import { useLayoutEffect, useRef, useState } from "react";

import { IconSend } from "../ui/icons";
import type { Phase } from "./useLookup";

interface Props {
  phase: Phase;
  answering: string | null;
  onAsk: (question: string) => void;
}

/** 追问输入框。释义出完之前禁用——没有释义做上下文，追问没有意义。 */
export function AskBox({ phase, answering, onAsk }: Props) {
  const [input, setInput] = useState("");
  const boxRef = useRef<HTMLTextAreaElement>(null);
  const disabled = phase !== "ready" || answering !== null;

  // 跟着内容长高，到上限（CSS 里的 max-height）为止自己滚。
  useLayoutEffect(() => {
    const box = boxRef.current;
    if (!box) return;
    box.style.height = "auto";
    box.style.height = `${box.scrollHeight}px`;
  }, [input]);

  const submit = () => {
    if (disabled || !input.trim()) return;
    onAsk(input);
    setInput("");
  };

  return (
    <div className="askbox">
      <div className="askbox__row">
        <div className="field askbox__field">
          <textarea
            ref={boxRef}
            rows={1}
            value={input}
            placeholder={phase === "ready" ? "继续提问，深入学习…" : "等释义出完再追问"}
            disabled={disabled}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              // 中文输入法组词途中回车是在选字，不该当作发送。
              if (e.key !== "Enter" || e.nativeEvent.isComposing) return;
              // Shift + Enter 换行，交给 textarea 自己处理。
              if (e.shiftKey) return;
              e.preventDefault();
              submit();
            }}
          />
        </div>
        <button
          type="button"
          className="primary askbox__send"
          onClick={submit}
          disabled={disabled || !input.trim()}
        >
          <IconSend />
          发送
        </button>
      </div>
      <p className="askbox__hint">提示：按 Enter 发送，Shift + Enter 换行</p>
    </div>
  );
}
