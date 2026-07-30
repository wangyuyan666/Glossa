import { useEffect, useRef } from "react";

interface Props {
  /** 已收到的思考文本。空串时只显示「思考中…」。 */
  text: string;
}

/**
 * 「思考中…」。
 *
 * 推理模型吐正文之前可能思考十几秒，骨架屏一动不动，看起来像卡死。把思考流露出来
 * 至少证明它在动。
 *
 * 这不是答案：样式刻意做得比释义弱（小字、灰、可滚动的固定高度），正文一到就撤掉。
 */
export function ThinkingNote({ text }: Props) {
  const bodyRef = useRef<HTMLDivElement>(null);

  // 思考是流进来的，跟着往下滚，否则用户只能看见开头那几行。
  useEffect(() => {
    bodyRef.current?.scrollTo({ top: bodyRef.current.scrollHeight });
  }, [text]);

  return (
    <div className="thinking">
      <span className="thinking__label">思考中···</span>
      {text && (
        <div className="thinking__text" ref={bodyRef}>
          {text}
        </div>
      )}
    </div>
  );
}
