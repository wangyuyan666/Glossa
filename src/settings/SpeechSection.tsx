import { useEffect, useState, useSyncExternalStore } from "react";

import {
  MAX_RATE,
  MIN_RATE,
  clampRate,
  configure,
  loadVoices,
  pickVoice,
  sortVoices,
  speak,
  subscribeVoices,
  voiceSnapshot,
  voiceTier,
} from "../lib/speech";
import type { Settings as SettingsData, Voice } from "../lib/types";
import { IconChevronDown, IconRefresh } from "../ui/icons";

interface Props {
  settings: SettingsData;
  onPatch: (fields: Partial<SettingsData>) => void;
}

/** 试听句：长短适中，含常见连读和一个多音节词，够听出嗓子之间的差别。 */
const SAMPLE = "The quick brown fox jumps over the lazy dog.";

const TIER_LABELS = ["", "增强", "高品质"];

/** 下拉里的显示名：把 macOS 的档位后缀换成中文，`Ava (Premium)` → `Ava · 高品质`。 */
function voiceLabel(voice: Voice): string {
  const tier = voiceTier(voice.name);
  const bare = voice.name.replace(/\s*\((premium|enhanced)\)\s*$/i, "");
  const quality = tier > 0 ? ` · ${TIER_LABELS[tier]}` : "";
  return `${bare}${quality}（${voice.lang}）`;
}

/**
 * 发音设置：挑嗓子、调语速、试听。
 *
 * 嗓子列表由 Rust 侧解析 `say -v '?'` 得来，不是 webview 那份——见
 * `src-tauri/src/speech.rs` 顶部。
 */
export function SpeechSection({ settings, onPatch }: Props) {
  const all = useSyncExternalStore(subscribeVoices, voiceSnapshot);
  const [error, setError] = useState<string | null>(null);
  const rate = clampRate(settings.speechRate);
  const fillPercent = ((rate - MIN_RATE) / (MAX_RATE - MIN_RATE)) * 100;

  useEffect(() => loadVoices(), []);

  // 试听放的必须是表单里当前这套，不是已落盘那套，否则调完听不出变化。
  useEffect(() => {
    configure({ voice: settings.voice, rate });
  }, [settings.voice, rate]);

  const voices = sortVoices(all, "en-US");
  const auto = pickVoice(voices, "en-US");
  // 列表还没加载完时先别下结论，否则每次打开设置页都先弹一条「没有高品质嗓音」。
  const missingGoodVoice =
    voices.length > 0 && !voices.some((voice) => voiceTier(voice.name) > 0);

  const preview = () => {
    setError(null);
    void speak("settings-preview", SAMPLE).catch((e) => setError(String(e)));
  };

  const rescan = () => {
    setError(null);
    loadVoices(true);
  };

  return (
    <section className="settings-card">
      <h2>发音</h2>
      <p className="muted">朗读用 macOS 自带的语音合成，不联网，离线可用。</p>

      <label className="inline inline--wide">
        嗓音
        <span className="voice-field">
          <span className="select-field">
            <select
              value={settings.voice ?? ""}
              onChange={(e) => onPatch({ voice: e.target.value || null })}
            >
              <option value="">自动{auto ? `（${voiceLabel(auto)}）` : ""}</option>
              {voices.map((voice) => (
                <option key={voice.name} value={voice.name}>
                  {voiceLabel(voice)}
                </option>
              ))}
            </select>
            <IconChevronDown className="select-field__arrow" />
          </span>
          <button type="button" title="重新扫描系统嗓音" onClick={rescan}>
            <IconRefresh />
          </button>
          <button type="button" onClick={preview}>
            试听
          </button>
        </span>
      </label>

      <label className="inline inline--wide">
        语速
        <span className="speech-rate">
          <input
            type="range"
            min={MIN_RATE}
            max={MAX_RATE}
            step={0.05}
            value={rate}
            onChange={(e) => onPatch({ speechRate: Number(e.target.value) })}
            // 原生 range 没法只给已选那段上色，把百分比交给 CSS 画渐变。
            style={{ "--fill": `${fillPercent}%` } as React.CSSProperties}
          />
          <span className="speech-rate__value">{rate.toFixed(2)}×</span>
        </span>
      </label>

      {error && <p className="status status--fail">朗读失败：{error}</p>}

      {missingGoodVoice && (
        <p className="status status--fail">
          没找到高品质英语嗓音，现在念的是压缩版合成音，听感一般。到「系统设置 →
          辅助功能 → 朗读内容 → 系统声音 → 管理声音 → 英语」下载 Ava、Zoe 或
          Evan，装完点上面的刷新即可选到。
        </p>
      )}
    </section>
  );
}
