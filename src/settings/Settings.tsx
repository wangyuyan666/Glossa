import { useEffect, useState } from "react";

import * as api from "../lib/api";
import type { RoleBinding, Settings as SettingsData } from "../lib/types";
import { PROTOCOL_DEFAULT_BASE_URL } from "../lib/types";
import { CaptureSection } from "./CaptureSection";
import { PromptSection } from "./PromptSection";
import { ProviderEditor } from "./ProviderEditor";
import "./settings.css";

const EMPTY: SettingsData = {
  providers: [],
  fast: null,
  chat: null,
  port: 8765,
  nativeLanguage: "中文",
  templates: [],
  activeWord: null,
  activeSentence: null,
  activeTranslate: null,
  activeChat: null,
};

export function Settings() {
  const [settings, setSettings] = useState<SettingsData>(EMPTY);
  const [modelsByProvider, setModelsByProvider] = useState<Record<string, string[]>>({});
  const [filePath, setFilePath] = useState("");
  const [saved, setSaved] = useState<string | null>(null);
  /** 已落盘的端口。取词 section 据此判断表单里的端口改了还没保存 */
  const [savedPort, setSavedPort] = useState<number | null>(null);

  useEffect(() => {
    void api.getSettings().then((loaded) => {
      setSettings(loaded);
      setSavedPort(loaded.port);
    });
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
          maxTokens: null,
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
      setSavedPort(settings.port);
    } catch (e) {
      setSaved(`保存失败：${e}`);
    }
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
        <p className="muted">改端口后需重启 EnAssistant，并重新安装一次 PopClip 扩展。</p>
      </section>

      <CaptureSection port={settings.port} savedPort={savedPort} />

      <PromptSection settings={settings} onPatch={patch} />

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
