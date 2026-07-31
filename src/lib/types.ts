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

export type TemplateKind =
  | "explain"
  | "chat"
  | "word"
  | "sentence"
  | "translate";

export const TEMPLATE_KIND_LABELS: Record<TemplateKind, string> = {
  explain: "统一释义",
  chat: "追问对话",
  word: "旧版 · 单词释义",
  sentence: "旧版 · 句子释义",
  translate: "旧版 · 译成英文",
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

export interface TemplateProbeCase {
  label: string;
  input: string;
  expectedMode: ExplanationMode | null;
  actualMode: string | null;
  raw: string;
  parsed: boolean;
  missingFields: string[];
  typeErrors: string[];
  unexpectedFields: string[];
  passed: boolean;
}

export interface TemplateProbe {
  passed: boolean;
  cases: TemplateProbeCase[];
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
  /** 统一释义当前启用的模板 id，null 或指向已删除的模板都回落到内置 */
  activeExplain: string | null;
  /** 旧版选择记录只为无损保留已有配置，不再参与真实释义 */
  activeWord: string | null;
  activeSentence: string | null;
  activeTranslate: string | null;
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

export type ExplanationMode = "word" | "sentence" | "translate";

/**
 * 释义卡片。流式期间为部分字段，故用处都是 `Partial<Explanation>`。
 *
 * 统一提示词让模型自行判断模式；`mode` 必须先输出。旧历史没有 mode，前端仍按
 * 独有字段回退推断，保证升级后可继续打开。
 */
export interface Explanation {
  mode: ExplanationMode;
  /**
   * 选中内容本身的拼写 / 语法纠错。释义的两套 schema 都有，所以不能拿它区分词和句；
   * 翻译模式没有这一项（原文本来就不是英文）。
   * 没有问题时两项都是空串，卡片不渲染这一块。
   */
  grammar: { issue: string; corrected: string };

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

  // ---- 译成英文（选中内容不是英文时）----
  /** 最地道的英文说法 */
  english: string;
  /** 为什么用这些词。翻译模式的重点，不是附赠 */
  wordChoice: { term: string; note: string }[];
  /** 别的说法 + 适用场合 */
  alternatives: { text: string; when: string }[];
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
