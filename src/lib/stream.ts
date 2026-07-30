import { listen } from "@tauri-apps/api/event";

import type { StreamEvent } from "./types";

interface Handlers {
  onDelta: (text: string) => void;
  onDone: () => void;
  onError: (message: string) => void;
  /** 推理模型的思考增量。不关心思考过程的调用方可以不传。 */
  onReasoning?: (text: string) => void;
}

/**
 * 全局只挂一个 `llm-stream` 监听，按 streamId 分发给各自的处理器。
 * 每条流各挂一个监听会在频繁查询时留下清理不掉的残余。
 */
const handlers = new Map<string, Handlers>();
let listening = false;

function ensureListener() {
  if (listening) return;
  listening = true;
  void listen<StreamEvent>("llm-stream", ({ payload }) => {
    const handler = handlers.get(payload.streamId);
    if (!handler) return; // 已被取消的流，丢弃其余增量

    switch (payload.kind) {
      case "delta":
        handler.onDelta(payload.text);
        break;
      case "reasoning":
        handler.onReasoning?.(payload.text);
        break;
      case "done":
        handlers.delete(payload.streamId);
        handler.onDone();
        break;
      case "error":
        handlers.delete(payload.streamId);
        handler.onError(payload.message);
        break;
    }
  });
}

/**
 * 起一条流。
 *
 * @param start 拿到 streamId 后真正发起请求的函数（`api.explain` / `api.chatTurn`）
 * @returns 取消函数。取消后该流的后续事件被丢弃。
 */
export function startStream(
  start: (streamId: string) => Promise<void>,
  { onDelta, onDone, onError, onReasoning }: Handlers,
): () => void {
  ensureListener();

  const streamId = crypto.randomUUID();
  handlers.set(streamId, { onDelta, onDone, onError, onReasoning });

  start(streamId).catch((err) => {
    handlers.delete(streamId);
    onError(String(err));
  });

  return () => {
    handlers.delete(streamId);
  };
}
