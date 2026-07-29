export type Protocol = "openai" | "anthropic";

export interface Provider {
  id: string;
  name: string;
  protocol: Protocol;
  /** 例如 https://api.openai.com/v1 或 https://api.anthropic.com，结尾带不带 /v1 都可以 */
  baseUrl: string;
  apiKey: string;
}

export interface RoleBinding {
  providerId: string;
  model: string;
}

export interface Settings {
  providers: Provider[];
  /** 释义角色，求快求便宜 */
  fast: RoleBinding | null;
  /** 对话角色，求强 */
  chat: RoleBinding | null;
  port: number;
  nativeLanguage: string;
}

export interface LookupPayload {
  text: string;
  /** 阶段一取词层拿不到上下文，恒为 null */
  context: string | null;
}

/** 释义卡片。流式期间为部分字段，故全部可选。 */
export interface Explanation {
  word: string;
  phonetic: string;
  pos: string;
  senseHere: string;
  why: string;
  collocations: string[];
  example: { en: string; zh: string };
}

export interface ChatMessage {
  role: "user" | "assistant";
  content: string;
}

export type StreamEvent =
  | { kind: "delta"; streamId: string; text: string }
  | { kind: "done"; streamId: string }
  | { kind: "error"; streamId: string; message: string };

export const PROTOCOL_LABELS: Record<Protocol, string> = {
  openai: "OpenAI 兼容",
  anthropic: "Anthropic",
};

/** 新建 provider 时按协议给出的默认 base_url。 */
export const PROTOCOL_DEFAULT_BASE_URL: Record<Protocol, string> = {
  openai: "https://api.openai.com/v1",
  anthropic: "https://api.anthropic.com",
};
