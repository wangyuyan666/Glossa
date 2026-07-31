import { useId, useSyncExternalStore } from "react";

import { cancel, speak, speakingId, subscribe } from "../lib/speech";
import { IconSpeaker } from "../ui/icons";

interface Props {
  /** 要朗读的内容。空串时整个按钮不渲染，不留一个点了没反应的壳。 */
  text: string;
  /** 无障碍标签兼 tooltip，例如「朗读例句」。 */
  label: string;
  /** 流式期间文本还在长，先禁用——否则念出来是半句。 */
  disabled?: boolean;
}

/**
 * 朗读一段文本的小喇叭，再点一次停。
 *
 * 页面上有好几个喇叭，同时只有一个响：谁在响由 `speech` 模块记着，这里只订阅。
 * 朗读失败不在这儿报——正文里弹一条错误太吵，设置页的试听会说。
 */
export function SpeakButton({ text, label, disabled = false }: Props) {
  const id = useId();
  const active = useSyncExternalStore(subscribe, speakingId) === id;
  const content = text.trim();

  if (!content) return null;

  return (
    <button
      type="button"
      className={`speak${active ? " speak--on" : ""}`}
      title={label}
      aria-label={label}
      disabled={disabled}
      onClick={() => {
        if (active) {
          cancel();
        } else {
          void speak(id, content).catch(() => {});
        }
      }}
    >
      <IconSpeaker />
    </button>
  );
}
