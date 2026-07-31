/**
 * 发音：单词、例句、整句都念。
 *
 * 播放在 Rust 侧走 `say`（见 `src-tauri/src/speech.rs` 顶部关于为什么不用
 * webview 自带 `speechSynthesis` 的说明）。这里只管三件事：挑嗓子、记住谁在念、
 * 把设置里的嗓子和语速带上。
 *
 * 嗓子和语速存在 `settings.json`（`config.rs` 的 `voice` / `speechRate`），
 * 两个窗口各自读配置后调 `configure()`。
 */

import * as api from "./api";
import type { Voice } from "./types";

/** 挑嗓子只看这两个字段，单测里不必造完整的 Voice。 */
export interface VoiceLike {
  name: string;
  lang: string;
}

/** 语速区间，和 `config.rs` 的 MIN/MAX_SPEECH_RATE 对齐。 */
export const MIN_RATE = 0.5;
export const MAX_RATE = 1.5;

export function clampRate(rate: number): number {
  if (!Number.isFinite(rate)) return 1;
  return Math.min(MAX_RATE, Math.max(MIN_RATE, rate));
}

function normalizeLang(lang: string): string {
  return lang.toLowerCase().replace(/_/g, "-");
}

/**
 * 音质档位：Premium 2 / Enhanced 1 / 其余 0。
 *
 * macOS 预装的是 compact 版合成音（Samantha 那一档），Premium 与 Enhanced 要
 * 用户自己到「系统设置 → 辅助功能 → 朗读内容 → 管理声音」下载，听感差一整代。
 * 名字后缀是唯一能认出档位的线索；macOS 改了命名就退回 0，挑法和加档位之前一样。
 * 注意 `Eddy (English (US))` 那种括号是地区不是档位，所以匹配要锚在结尾。
 */
export function voiceTier(name: string): number {
  if (/\(premium\)\s*$/i.test(name)) return 2;
  if (/\(enhanced\)\s*$/i.test(name)) return 1;
  return 0;
}

/** 一池嗓子里挑最好的：先比音质档位，同档按名字定序，保证每次挑的是同一个。 */
function bestOf<T extends VoiceLike>(pool: readonly T[]): T | null {
  return (
    [...pool].sort(
      (a, b) => voiceTier(b.name) - voiceTier(a.name) || a.name.localeCompare(b.name),
    )[0] ?? null
  );
}

/**
 * 挑一个嗓子。`preferredName` 是用户在设置里选的，认得出就直接用。
 *
 * 用户没选、或选的那个嗓子被卸载了，就自动挑：音质档位 → 同语言 → 同语族。
 * 语族兜底是给「只装了 en-GB 却要 en-US」用的，口音不对也好过不出声；
 * 不跨语族兜底——拿中文嗓念英文比不出声更糟。
 */
export function pickVoice<T extends VoiceLike>(
  voices: readonly T[],
  lang: string,
  preferredName?: string | null,
): T | null {
  const chosen = preferredName
    ? voices.find((voice) => voice.name === preferredName)
    : undefined;
  if (chosen) return chosen;

  const want = normalizeLang(lang);
  const family = want.split("-")[0];
  const exact = voices.filter((voice) => normalizeLang(voice.lang) === want);
  const kin = voices.filter(
    (voice) => normalizeLang(voice.lang).split("-")[0] === family,
  );

  return bestOf(exact) ?? bestOf(kin) ?? null;
}

/**
 * 设置页下拉里列的嗓子：同语族的全留下，好的排前面。
 *
 * 不按 `lang` 精确过滤——en-GB 的 Daniel 该出现在列表里，是不是美音由用户定。
 * 但趣味音（Bad News、Zarvox）也在其中，没法可靠地按名字剔掉，只能靠排序压到后面。
 */
export function sortVoices<T extends VoiceLike>(
  voices: readonly T[],
  lang: string,
): T[] {
  const family = normalizeLang(lang).split("-")[0];
  return voices
    .filter((voice) => normalizeLang(voice.lang).split("-")[0] === family)
    .sort(
      (a, b) => voiceTier(b.name) - voiceTier(a.name) || a.name.localeCompare(b.name),
    );
}

// ---------------------------------------------------------------- 运行时状态

const NO_VOICES: Voice[] = [];

let allVoices: Voice[] = NO_VOICES;
let loading: Promise<void> | null = null;
let preferredVoice: string | null = null;
let rate = 1;
let speaking: string | null = null;

const voiceListeners = new Set<() => void>();
const speakingListeners = new Set<() => void>();

function notify(listeners: Set<() => void>) {
  for (const listener of listeners) listener();
}

/**
 * 应用配置里的嗓子与语速。只传其中一项时另一项保持不变。
 *
 * 设置页每改一下就调一次，好让试听放的是当前这套而不是已落盘那套。
 */
export function configure(options: {
  voice?: string | null;
  rate?: number;
}): void {
  if (options.voice !== undefined) preferredVoice = options.voice || null;
  if (options.rate !== undefined) rate = clampRate(options.rate);
}

/**
 * 当前嗓子列表。
 *
 * 返回的必须是同一个引用——它喂给 `useSyncExternalStore`，每次现算一个新数组
 * 会被判定成「快照一直在变」，直接死循环。
 */
export function voiceSnapshot(): Voice[] {
  return allVoices;
}

export function subscribeVoices(listener: () => void): () => void {
  voiceListeners.add(listener);
  return () => {
    voiceListeners.delete(listener);
  };
}

/**
 * 去 Rust 侧问一遍系统装了哪些嗓子。
 *
 * `force` 是给「用户刚在系统设置里装完嗓子」用的：列表不会自己变，得有人重扫。
 */
export function loadVoices(force = false): void {
  if (loading || (allVoices !== NO_VOICES && !force)) return;
  loading = api
    .listVoices()
    .then((list) => {
      allVoices = list;
    })
    .catch(() => {
      // 列表拿不到不算发音坏了：不带 -v 的 say 仍会用系统默认嗓出声。
      allVoices = NO_VOICES;
    })
    .finally(() => {
      loading = null;
      notify(voiceListeners);
    });
}

export function subscribe(listener: () => void): () => void {
  speakingListeners.add(listener);
  return () => {
    speakingListeners.delete(listener);
  };
}

/** 当前正在朗读的按钮 id，给 useSyncExternalStore 取快照。 */
export function speakingId(): string | null {
  return speaking;
}

function setSpeaking(id: string | null) {
  if (speaking === id) return;
  speaking = id;
  notify(speakingListeners);
}

/**
 * 念一段文本。`id` 标识是谁在念——页面上有好几个喇叭，但同时只有一个响。
 *
 * 返回的 Promise 在念完或被打断时 resolve；失败时 reject，由调用方决定是不是
 * 要显示出来（卡片里的喇叭不显示，设置页的试听显示——那是用户明确发起的动作）。
 */
export function speak(id: string, text: string): Promise<void> {
  // 配置里存的嗓子可能已经被卸载了，`say -v` 遇到不认识的名字直接失败。
  // 在这儿过一遍 pickVoice：认得出就用，认不出自动换一个，列表还没到就交给
  // 系统默认嗓。让「卸载了一个嗓子」变成音色变了，而不是发音整个坏掉。
  const resolved = pickVoice(allVoices, "en-US", preferredVoice);
  setSpeaking(id);
  return api.speak(text, resolved?.name ?? null, rate).finally(() => {
    // 期间可能已经换人念了，别把接任者的状态清掉。
    if (speaking === id) setSpeaking(null);
  });
}

export function cancel(): void {
  setSpeaking(null);
  void api.stopSpeaking().catch(() => {});
}
