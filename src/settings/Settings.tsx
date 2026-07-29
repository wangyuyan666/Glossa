import { useEffect, useMemo, useState } from "react";

import * as api from "../lib/api";
import type { RoleBinding, Settings as SettingsData } from "../lib/types";
import { PROTOCOL_DEFAULT_BASE_URL } from "../lib/types";
import { ProviderEditor } from "./ProviderEditor";
import "./settings.css";

const EMPTY: SettingsData = {
  providers: [],
  fast: null,
  chat: null,
  port: 8765,
  nativeLanguage: "中文",
};

/**
 * PopClip 的 snippet 格式：必须以 `#popclip` 开头，其余部分是 YAML。
 * 用单行 shell script 而不是 `|` 块，省掉 YAML 缩进出错的可能。
 */
function popclipSnippet(port: number): string {
  return `#popclip
name: EnAssistant
icon: symbol:character.book.closed
interpreter: bash
shell script: curl -s -X POST http://127.0.0.1:${port}/lookup --data-urlencode "q=$POPCLIP_TEXT" -o /dev/null`;
}

export function Settings() {
  const [settings, setSettings] = useState<SettingsData>(EMPTY);
  const [modelsByProvider, setModelsByProvider] = useState<Record<string, string[]>>({});
  const [filePath, setFilePath] = useState("");
  const [saved, setSaved] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    void api.getSettings().then(setSettings);
    void api.settingsFilePath().then(setFilePath);
  }, []);

  const patch = (fields: Partial<SettingsData>) => {
    setSettings((prev) => ({ ...prev, ...fields }));
    setSaved(null);
  };

  const addProvider = () => {
    patch({
      providers: [
        ...settings.providers,
        {
          id: crypto.randomUUID(),
          name: "",
          protocol: "openai",
          baseUrl: PROTOCOL_DEFAULT_BASE_URL.openai,
          apiKey: "",
        },
      ],
    });
  };

  const removeProvider = (id: string) => {
    // 角色绑定指向被删的 provider 就一并清掉，避免存下悬空引用。
    patch({
      providers: settings.providers.filter((p) => p.id !== id),
      fast: settings.fast?.providerId === id ? null : settings.fast,
      chat: settings.chat?.providerId === id ? null : settings.chat,
    });
  };

  const save = async () => {
    try {
      await api.saveSettings(settings);
      setSaved("已保存");
    } catch (e) {
      setSaved(`保存失败：${e}`);
    }
  };

  const snippet = useMemo(() => popclipSnippet(settings.port), [settings.port]);

  const copySnippet = async () => {
    await navigator.clipboard.writeText(snippet);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  const roleRow = (
    label: string,
    hint: string,
    binding: RoleBinding | null,
    onChange: (binding: RoleBinding | null) => void,
  ) => {
    const models = binding ? (modelsByProvider[binding.providerId] ?? []) : [];
    const listId = `role-models-${label}`;
    return (
      <div className="role">
        <div className="role__label">
          <strong>{label}</strong>
          <small>{hint}</small>
        </div>
        <select
          value={binding?.providerId ?? ""}
          onChange={(e) =>
            onChange(
              e.target.value
                ? { providerId: e.target.value, model: binding?.model ?? "" }
                : null,
            )
          }
        >
          <option value="">未配置</option>
          {settings.providers.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name || "(未命名)"}
            </option>
          ))}
        </select>
        <input
          value={binding?.model ?? ""}
          list={listId}
          placeholder="模型名"
          disabled={!binding}
          onChange={(e) =>
            binding && onChange({ ...binding, model: e.target.value })
          }
        />
        <datalist id={listId}>
          {models.map((m) => (
            <option key={m} value={m} />
          ))}
        </datalist>
      </div>
    );
  };

  return (
    <div className="settings">
      <h1>EnAssistant 设置</h1>

      <section>
        <h2>模型服务</h2>
        <p className="muted">
          <code>openai</code> 协议覆盖 OpenAI 官方、DeepSeek、硅基流动、OpenRouter、Groq、
          Ollama、LM Studio 等；<code>anthropic</code> 协议覆盖 Anthropic 官方及兼容代理。
          base_url 结尾带不带 <code>/v1</code> 都可以。
        </p>

        {settings.providers.map((provider) => (
          <ProviderEditor
            key={provider.id}
            provider={provider}
            models={modelsByProvider[provider.id] ?? []}
            onChange={(next) =>
              patch({
                providers: settings.providers.map((p) =>
                  p.id === next.id ? next : p,
                ),
              })
            }
            onRemove={() => removeProvider(provider.id)}
            onModelsLoaded={(models) =>
              setModelsByProvider((prev) => ({ ...prev, [provider.id]: models }))
            }
          />
        ))}

        <button type="button" onClick={addProvider}>
          + 添加服务
        </button>
      </section>

      <section>
        <h2>角色绑定</h2>
        <p className="muted">两个角色可以用不同厂商的模型。</p>
        {roleRow("释义", "划词后出释义，求快求便宜", settings.fast, (fast) =>
          patch({ fast }),
        )}
        {roleRow("对话", "追问时使用，求强", settings.chat, (chat) => patch({ chat }))}
      </section>

      <section>
        <h2>通用</h2>
        <label className="inline">
          解释语言
          <input
            value={settings.nativeLanguage}
            onChange={(e) => patch({ nativeLanguage: e.target.value })}
          />
        </label>
        <label className="inline">
          取词监听端口
          <input
            type="number"
            value={settings.port}
            onChange={(e) => patch({ port: Number(e.target.value) || 8765 })}
          />
        </label>
        <p className="muted">改端口后需重启 EnAssistant，并同步更新下方 PopClip 片段。</p>
      </section>

      <section>
        <h2>PopClip 取词配置</h2>
        <ol className="muted steps">
          <li>点下面的「复制片段」</li>
          <li>粘贴到任意能选中文本的地方（备忘录、文本编辑、浏览器地址栏之外的输入框都行）</li>
          <li>
            <strong>选中整段</strong>（含开头的 <code>#popclip</code> 一行），PopClip 条上会出现
            <strong> Install Extension</strong>，点它
          </li>
          <li>PopClip 会提示这是未签名扩展，确认安装即可</li>
        </ol>
        <pre className="snippet">{snippet}</pre>
        <button type="button" onClick={() => void copySnippet()}>
          {copied ? "已复制" : "复制片段"}
        </button>
      </section>

      <section>
        <h2>存储</h2>
        <p className="muted">配置文件：{filePath || "…"}</p>
        <p className="warn">
          API key 以明文保存在该文件中（权限 0600）。任何能读该文件的进程或用户都能拿到它，
          文件也会进入 Time Machine 备份与目录同步。
        </p>
      </section>

      <footer className="settings__footer">
        <button type="button" className="primary" onClick={() => void save()}>
          保存
        </button>
        {saved && <span className="muted">{saved}</span>}
      </footer>
    </div>
  );
}
