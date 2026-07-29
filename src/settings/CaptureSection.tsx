import { useCallback, useEffect, useState } from "react";

import * as api from "../lib/api";

interface Props {
  /** 表单里当前的端口，可能还没保存 */
  port: number;
  /** 已落盘的端口。一键安装和 snippet 都由 Rust 侧按落盘值生成 */
  savedPort: number | null;
}

type Status =
  | { kind: "idle" }
  | { kind: "ok"; message: string }
  | { kind: "fail"; message: string };

/**
 * 取词配置。
 *
 * 一键安装生成 `.popclipext` 目录再交给 PopClip；手动安装用 snippet。
 * 手动路径必须保留——一键安装依赖文件关联，Setapp 版、多版本共存、
 * 关联被别的软件抢走都可能让它失效。
 */
export function CaptureSection({ port, savedPort }: Props) {
  const [hasPopclip, setHasPopclip] = useState<boolean | null>(null);
  const [snippet, setSnippet] = useState("");
  const [status, setStatus] = useState<Status>({ kind: "idle" });
  const [showManual, setShowManual] = useState(false);
  const [copied, setCopied] = useState(false);

  const refresh = useCallback(() => {
    void api.popclipInstalled().then(setHasPopclip);
    void api.popclipSnippet().then(setSnippet);
  }, []);

  // savedPort 变了说明刚保存过，snippet 里的端口要跟着更新。
  useEffect(refresh, [refresh, savedPort]);

  const portDirty = savedPort !== null && savedPort !== port;

  const install = async () => {
    try {
      const outcome = await api.installPopclipExtension();
      setStatus(
        outcome.opened
          ? {
              kind: "ok",
              message: "已交给 PopClip，在它弹出的确认框里点安装即可。",
            }
          : {
              kind: "fail",
              message: `未检测到 PopClip，扩展已生成在 ${outcome.path}，装好 PopClip 后双击它即可。`,
            },
      );
      setHasPopclip(outcome.opened);
    } catch (e) {
      setStatus({ kind: "fail", message: String(e) });
    }
  };

  const copySnippet = async () => {
    await navigator.clipboard.writeText(snippet);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  return (
    <section>
      <h2>取词</h2>
      <p className="muted">
        阶段一的取词由 PopClip 完成：在任意 app 里选中英文，点 PopClip 条上的 EnAssistant
        图标即可查询。
      </p>

      {portDirty && (
        <p className="status status--fail">
          端口改成了 {port} 但还没保存。先点下方「保存」，再安装扩展，否则装出来的还是旧端口。
        </p>
      )}

      <button type="button" className="primary" onClick={() => void install()}>
        安装到 PopClip
      </button>

      {hasPopclip === false && status.kind === "idle" && (
        <p className="status status--fail">
          未检测到 PopClip。它是收费软件，需先从{" "}
          <a href="https://www.popclip.app/" target="_blank" rel="noreferrer">
            popclip.app
          </a>{" "}
          安装。
        </p>
      )}

      {status.kind !== "idle" && (
        <p className={`status status--${status.kind}`}>{status.message}</p>
      )}

      <button type="button" onClick={() => setShowManual((v) => !v)}>
        {showManual ? "收起手动安装" : "手动安装"}
      </button>

      {showManual && (
        <>
          <ol className="muted steps">
            <li>点「复制片段」</li>
            <li>粘贴到任意能选中文本的地方（备忘录、文本编辑等）</li>
            <li>
              <strong>选中整段</strong>（含开头的 <code>#popclip</code> 一行），PopClip
              条上会出现 <strong>Install Extension</strong>，点它
            </li>
            <li>PopClip 提示这是未签名扩展，确认安装</li>
          </ol>
          <pre className="snippet">{snippet}</pre>
          <button type="button" onClick={() => void copySnippet()}>
            {copied ? "已复制" : "复制片段"}
          </button>
        </>
      )}
    </section>
  );
}
