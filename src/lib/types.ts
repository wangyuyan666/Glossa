export type Protocol = "openai" | "anthropic";

export interface Provider {
  id: string;
  name: string;
  protocol: Protocol;
  /** 例如 https://api.openai.com/v1 或 https://api.anthropic.com，结尾带不带 /v1 都可以 */
  baseUrl: string;
  apiKey: string;
  /**
   * 该端点单次输出的 token 上限，null 表示用默认值（4000）。
   *
   * 含推理模型的思考 token。给小了会「思考写满、正文零字符」，给大了端点可能直接 400。
   */
  maxTokens: number | null;
}

/** provider 没配 maxTokens 时后端用的默认值，只用于界面上的提示文案。 */
export const DEFAULT_MAX_TOKENS = 4000;

export interface RoleBinding {
  providerId: string;
  model: string;
}

export type TemplateKind = "word" | "sentence" | "chat";

export const TEMPLATE_KIND_LABELS: Record<TemplateKind, string> = {
  word: "释义（单词）",
  sentence: "释义（句子）",
  chat: "追问对话",
};

export interface PromptTemplate {
  id: string;
  name: string;
  kind: TemplateKind;
  body: string;
  /** 内置模板不可改不可删 */
  builtin: boolean;
}

export interface TemplateIssue {
  /** error 会让模板不可用，warn 只是提醒 */
  level: "error" | "warn";
  message: string;
}

export interface TemplateProbe {
  /** 模型的原始输出 */
  raw: string;
  /** 释义类是否解析出合法 JSON；对话类只要非空即为 true */
  parsed: boolean;
  /** 契约里有、模型没输出的字段 */
  missingFields: string[];
}

export interface Settings {
  providers: Provider[];
  /** 释义角色，求快求便宜 */
  fast: RoleBinding | null;
  /** 对话角色，求强 */
  chat: RoleBinding | null;
  port: number;
  nativeLanguage: string;

  /** 用户自建的模板。内置模板不在这里，走 api.builtinTemplates() 取 */
  templates: PromptTemplate[];
  /** 各类当前启用的模板 id，null 或指向已删除的模板都回落到内置 */
  activeWord: string | null;
  activeSentence: string | null;
  activeChat: string | null;
}

/** 侧栏列表项，不含对话内容 */
export interface LookupSummary {
  id: string;
  text: string;
  /** 释义里的 senseHere，解析不出来是空串 */
  sense: string;
  createdAt: number;
}

/** 点开历史时加载的完整会话 */
export interface LookupDetail {
  id: string;
  text: string;
  context: string | null;
  /** 释义的原始 JSON 字符串，仍走 parsePartialJson，和流式路径共用渲染 */
  explanation: string;
  createdAt: number;
  turns: ChatMessage[];
}

export interface InstallOutcome {
  /** 生成的 .popclipext 目录路径 */
  path: string;
  /** 是否已交给 PopClip 打开。探测不到 PopClip 时为 false */
  opened: boolean;
}

export interface LookupPayload {
  text: string;
  /** 阶段一取词层拿不到上下文，恒为 null */
  context: string | null;
}

/**
 * 释义卡片。流式期间为部分字段，故用处都是 `Partial<Explanation>`。
 *
 * 单词和句子是两套字段，Rust 侧按选中内容长度决定用哪套提示词（见 prompts.rs 的
 * `is_sentence`）。卡片按字段存在性渲染，不需要额外的模式标记。
 */
export interface Explanation {
  // ---- 单词 / 短语 ----
  word: string;
  phonetic: string;
  pos: string;
  senseHere: string;
  why: string;
  collocations: string[];
  example: { en: string; zh: string };

  // ---- 句子 ----
  translation: string;
  structure: string;
  keyPoints: { term: string; note: string }[];
}

export interface ChatMessage {
  role: "user" | "assistant";
  content: string;
}

export type StreamEvent =
  | { kind: "delta"; streamId: string; text: string }
  /** 推理模型的思考增量。只用于「思考中」的展示，不参与 JSON 解析 */
  | { kind: "reasoning"; streamId: string; text: string }
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
