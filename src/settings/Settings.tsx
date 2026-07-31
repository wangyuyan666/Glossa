import { useCallback, useEffect, useRef, useState } from "react";

import * as api from "../lib/api";
import type {
  Provider,
  RoleBinding,
  Settings as SettingsData,
} from "../lib/types";
import { PROTOCOL_DEFAULT_BASE_URL } from "../lib/types";
import { IconChevronDown, IconCopy, IconWarn } from "../ui/icons";
import { CaptureSection } from "./CaptureSection";
import {
  ModelActionButtons,
  ModelStatusBanner,
  type ModelActionStatus,
  type ModelLoadResult,
} from "./ModelActions";
import { ModelCombobox } from "./ModelCombobox";
import { PromptSection } from "./PromptSection";
import { ProviderEditor } from "./ProviderEditor";
import { SpeechSection } from "./SpeechSection";
import "./settings.css";

const EMPTY: SettingsData = {
  providers: [],
  fast: null,
  chat: null,
  port: 8765,
  nativeLanguage: "中文",
  templates: [],
  activeExplain: null,
  activeWord: null,
  activeSentence: null,
  activeTranslate: null,
  activeChat: null,
  voice: null,
  speechRate: 1,
};

const IDLE_ACTION: ModelActionStatus = { kind: "idle" };
type RoleKey = "fast" | "chat";

export interface ModelCacheEntry {
  fingerprint: string;
  models: string[];
}

export function providerModelFingerprint(provider: Provider): string {
  return JSON.stringify([provider.protocol, provider.baseUrl.trim(), provider.apiKey]);
}

export function getCachedModels(
  entry: ModelCacheEntry | undefined,
  provider: Provider,
  force: boolean,
): string[] | null {
  if (force || !entry || entry.fingerprint !== providerModelFingerprint(provider)) {
    return null;
  }
  return entry.models;
}

export function canCommitModelRequest(
  provider: Provider | undefined,
  requestFingerprint: string,
): boolean {
  return !!provider && providerModelFingerprint(provider) === requestFingerprint;
}

export function isLatestModelRequest(currentVersion: number, requestVersion: number): boolean {
  return currentVersion === requestVersion;
}

export function isCurrentProviderGeneration(
  currentGeneration: number | undefined,
  requestGeneration: number,
): boolean {
  return (currentGeneration ?? 0) === requestGeneration;
}

export function changeRoleProvider(
  binding: RoleBinding | null,
  providerId: string,
): RoleBinding | null {
  if (!providerId) return null;
  return {
    providerId,
    model: binding?.providerId === providerId ? binding.model : "",
  };
}

export function Settings() {
  const [settings, setSettings] = useState<SettingsData>(EMPTY);
  const [modelCache, setModelCache] = useState<Record<string, ModelCacheEntry>>({});
  const [roleStatus, setRoleStatus] = useState<Record<RoleKey, ModelActionStatus>>({
    fast: IDLE_ACTION,
    chat: IDLE_ACTION,
  });
  const [filePath, setFilePath] = useState("");
  const [saved, setSaved] = useState<string | null>(null);
  const [copiedPath, setCopiedPath] = useState(false);
  /** 已落盘的端口。取词 section 据此判断表单里的端口改了还没保存 */
  const [savedPort, setSavedPort] = useState<number | null>(null);

  const settingsRef = useRef(settings);
  const modelCacheRef = useRef(modelCache);
  const modelRequests = useRef(new Map<string, Promise<ModelLoadResult>>());
  const providerGeneration = useRef<Record<string, number>>({});
  const providerRequestVersion = useRef<Record<string, number>>({});
  const roleOperation = useRef<Record<RoleKey, number>>({ fast: 0, chat: 0 });

  settingsRef.current = settings;
  modelCacheRef.current = modelCache;

  const replaceSettings = (next: SettingsData) => {
    settingsRef.current = next;
    setSettings(next);
    setSaved(null);
  };

  const patch = (fields: Partial<SettingsData>) => {
    replaceSettings({ ...settingsRef.current, ...fields });
  };

  const invalidateRoleStatus = (role: RoleKey) => {
    roleOperation.current[role] += 1;
    setRoleStatus((prev) => ({ ...prev, [role]: IDLE_ACTION }));
  };

  /** 从磁盘读一遍。「取消」复用它——丢弃改动就是回到落盘的那一份。 */
  const load = useCallback(() => {
    void api.getSettings().then((loaded) => {
      settingsRef.current = loaded;
      setSettings(loaded);
      setSavedPort(loaded.port);
      setSaved(null);
      roleOperation.current.fast += 1;
      roleOperation.current.chat += 1;
      setRoleStatus({ fast: IDLE_ACTION, chat: IDLE_ACTION });
    });
  }, []);

  useEffect(() => {
    load();
    void api.settingsFilePath().then(setFilePath);
  }, [load]);

  const loadModels = useCallback(
    async (provider: Provider, force: boolean): Promise<ModelLoadResult> => {
      const cached = getCachedModels(modelCacheRef.current[provider.id], provider, force);
      if (cached !== null) return { models: cached, source: "cache" };

      const fingerprint = providerModelFingerprint(provider);
      const requestKey = JSON.stringify([provider.id, fingerprint]);
      if (!force) {
        const running = modelRequests.current.get(requestKey);
        if (running) return running;
      }

      const generation = providerGeneration.current[provider.id] ?? 0;
      const requestVersion = (providerRequestVersion.current[provider.id] ?? 0) + 1;
      providerRequestVersion.current[provider.id] = requestVersion;
      const request = api.listModels(provider).then((models) => {
        const currentProvider = settingsRef.current.providers.find((p) => p.id === provider.id);
        if (
          isCurrentProviderGeneration(
            providerGeneration.current[provider.id],
            generation,
          ) &&
          isLatestModelRequest(
            providerRequestVersion.current[provider.id],
            requestVersion,
          ) &&
          canCommitModelRequest(currentProvider, fingerprint)
        ) {
          const nextCache = {
            ...modelCacheRef.current,
            [provider.id]: { fingerprint, models },
          };
          modelCacheRef.current = nextCache;
          setModelCache(nextCache);
        }
        return { models, source: "network" as const };
      });

      modelRequests.current.set(requestKey, request);
      try {
        return await request;
      } finally {
        if (modelRequests.current.get(requestKey) === request) {
          modelRequests.current.delete(requestKey);
        }
      }
    },
    [],
  );

  const addProvider = () => {
    patch({
      providers: [
        ...settingsRef.current.providers,
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

  const updateProvider = (nextProvider: Provider) => {
    const previous = settingsRef.current.providers.find((p) => p.id === nextProvider.id);
    const connectionChanged =
      !!previous &&
      providerModelFingerprint(previous) !== providerModelFingerprint(nextProvider);
    const testSettingsChanged =
      connectionChanged || (!!previous && previous.maxTokens !== nextProvider.maxTokens);

    if (connectionChanged) {
      providerGeneration.current[nextProvider.id] =
        (providerGeneration.current[nextProvider.id] ?? 0) + 1;
      const nextCache = { ...modelCacheRef.current };
      delete nextCache[nextProvider.id];
      modelCacheRef.current = nextCache;
      setModelCache(nextCache);
      if (settingsRef.current.fast?.providerId === nextProvider.id) {
        invalidateRoleStatus("fast");
      }
      if (settingsRef.current.chat?.providerId === nextProvider.id) {
        invalidateRoleStatus("chat");
      }
    } else if (testSettingsChanged) {
      if (
        settingsRef.current.fast?.providerId === nextProvider.id &&
        roleStatus.fast.kind !== "idle" &&
        roleStatus.fast.action === "test"
      ) {
        invalidateRoleStatus("fast");
      }
      if (
        settingsRef.current.chat?.providerId === nextProvider.id &&
        roleStatus.chat.kind !== "idle" &&
        roleStatus.chat.action === "test"
      ) {
        invalidateRoleStatus("chat");
      }
    }

    patch({
      providers: settingsRef.current.providers.map((provider) =>
        provider.id === nextProvider.id ? nextProvider : provider,
      ),
    });
  };

  const removeProvider = (id: string) => {
    // 角色绑定指向被删的 provider 就一并清掉，避免存下悬空引用。
    providerGeneration.current[id] = (providerGeneration.current[id] ?? 0) + 1;
    const nextCache = { ...modelCacheRef.current };
    delete nextCache[id];
    modelCacheRef.current = nextCache;
    setModelCache(nextCache);
    if (settingsRef.current.fast?.providerId === id) invalidateRoleStatus("fast");
    if (settingsRef.current.chat?.providerId === id) invalidateRoleStatus("chat");
    patch({
      providers: settingsRef.current.providers.filter((provider) => provider.id !== id),
      fast: settingsRef.current.fast?.providerId === id ? null : settingsRef.current.fast,
      chat: settingsRef.current.chat?.providerId === id ? null : settingsRef.current.chat,
    });
  };

  const save = async () => {
    const snapshot = settingsRef.current;
    try {
      await api.saveSettings(snapshot);
      if (settingsRef.current === snapshot) {
        setSaved("已保存");
        setSavedPort(snapshot.port);
      }
    } catch (e) {
      if (settingsRef.current === snapshot) {
        setSaved(`保存失败：${e}`);
      }
    }
  };

  const runRoleModelLoad = async (role: RoleKey, provider: Provider, force: boolean) => {
    const operation = roleOperation.current[role] + 1;
    roleOperation.current[role] = operation;
    setRoleStatus((prev) => ({
      ...prev,
      [role]: { kind: "loading", action: "models" },
    }));

    try {
      const { models } = await loadModels(provider, force);
      if (
        roleOperation.current[role] !== operation ||
        settingsRef.current[role]?.providerId !== provider.id
      ) {
        return;
      }
      setRoleStatus((prev) => ({
        ...prev,
        [role]: models.length
          ? {
              kind: "success",
              action: "models",
              message: `已获取 ${models.length} 个模型`,
            }
          : {
              kind: "error",
              action: "models",
              message: "未获取到模型。该端点可能不支持模型列表，请手动填写模型名。",
            },
      }));
    } catch (e) {
      if (roleOperation.current[role] !== operation) return;
      setRoleStatus((prev) => ({
        ...prev,
        [role]: {
          kind: "error",
          action: "models",
          message: `获取模型失败：${e}。可手动填写模型名后继续测试。`,
        },
      }));
    }
  };

  const runRoleTest = async (role: RoleKey, provider: Provider, modelValue: string) => {
    const model = modelValue.trim();
    if (!model) {
      invalidateRoleStatus(role);
      setRoleStatus((prev) => ({
        ...prev,
        [role]: {
          kind: "error",
          action: "test",
          message: "请先填写模型名，再测试连接。",
        },
      }));
      return;
    }

    const operation = roleOperation.current[role] + 1;
    roleOperation.current[role] = operation;
    setRoleStatus((prev) => ({
      ...prev,
      [role]: { kind: "loading", action: "test" },
    }));
    try {
      const reply = await api.testProvider(provider, model);
      if (
        roleOperation.current[role] !== operation ||
        settingsRef.current[role]?.providerId !== provider.id ||
        settingsRef.current[role]?.model.trim() !== model
      ) {
        return;
      }
      setRoleStatus((prev) => ({
        ...prev,
        [role]: {
          kind: "success",
          action: "test",
          message: `连接正常，模型回复：${reply}`,
        },
      }));
    } catch (e) {
      if (roleOperation.current[role] !== operation) return;
      setRoleStatus((prev) => ({
        ...prev,
        [role]: {
          kind: "error",
          action: "test",
          message: `测试连接失败：${e}`,
        },
      }));
    }
  };

  const handleRoleProviderChange = (
    role: RoleKey,
    binding: RoleBinding | null,
    providerId: string,
  ) => {
    const nextBinding = changeRoleProvider(binding, providerId);
    invalidateRoleStatus(role);
    patch(role === "fast" ? { fast: nextBinding } : { chat: nextBinding });

    if (nextBinding && nextBinding.providerId !== binding?.providerId) {
      const provider = settingsRef.current.providers.find(
        (candidate) => candidate.id === nextBinding.providerId,
      );
      if (provider) void runRoleModelLoad(role, provider, false);
    }
  };

  const handleRoleModelChange = (
    role: RoleKey,
    binding: RoleBinding,
    model: string,
  ) => {
    const status = roleStatus[role];
    if (status.kind !== "idle" && status.action === "test") {
      invalidateRoleStatus(role);
    }
    const nextBinding = { ...binding, model };
    patch(role === "fast" ? { fast: nextBinding } : { chat: nextBinding });
  };

  const modelsForProvider = (provider: Provider): string[] =>
    getCachedModels(modelCache[provider.id], provider, false) ?? [];

  const roleRow = (role: RoleKey, label: string, hint: string, binding: RoleBinding | null) => {
    const provider = binding
      ? settings.providers.find((candidate) => candidate.id === binding.providerId)
      : undefined;
    const models = provider ? modelsForProvider(provider) : [];
    return (
      <div className="role">
        <div className="role__label">
          <strong>{label}</strong>
          <small>{hint}</small>
        </div>
        <span className="select-field">
          <select
            value={binding?.providerId ?? ""}
            onChange={(e) => handleRoleProviderChange(role, binding, e.target.value)}
          >
            {/* 只在还没绑的时候当占位。绑好之后列表里只剩真实厂商，
                不留一个选了会把绑定清掉的空项。 */}
            {!binding && <option value="">未配置</option>}
            {settings.providers.map((candidate) => (
              <option key={candidate.id} value={candidate.id}>
                {candidate.name || "(未命名)"}
              </option>
            ))}
          </select>
          <IconChevronDown className="select-field__arrow" />
        </span>
        <div className="settings-model-control">
          <ModelCombobox
            value={binding?.model ?? ""}
            options={models}
            placeholder="模型名"
            disabled={!provider || !binding}
            onChange={(model) => binding && handleRoleModelChange(role, binding, model)}
          />
          <ModelActionButtons
            status={roleStatus[role]}
            disabled={!provider || !binding}
            onGetModels={() => provider && void runRoleModelLoad(role, provider, true)}
            onTestConnection={() =>
              provider && binding && void runRoleTest(role, provider, binding.model)
            }
          />
        </div>
        <div className="role__status">
          <ModelStatusBanner status={roleStatus[role]} />
        </div>
      </div>
    );
  };

  /**
   * 取消 = 丢弃未保存的改动并收起窗口。
   *
   * 必须先 load()：窗口是隐藏不销毁的，webview 状态会留到下次打开，
   * 不重读磁盘的话下次看到的还是这次改脏的表单。
   */
  const cancel = () => {
    load();
    void api.closeSettings();
  };

  const copyPath = async () => {
    await navigator.clipboard.writeText(filePath);
    setCopiedPath(true);
    setTimeout(() => setCopiedPath(false), 1500);
  };

  return (
    <div className="settings">
      <div className="settings__body">
        <div className="settings__inner">
          <header className="settings__head">
            <h1>设置</h1>
            <p className="muted">配置模型服务、角色绑定及应用行为</p>
          </header>

          <section className="settings-card">
            <h2>模型服务</h2>
            <p className="muted">
              配置 OpenAI 兼容服务，如 OpenAI、DeepSeek、OpenRouter、Groq、Ollama 等。
            </p>

            {settings.providers.map((provider) => (
              <ProviderEditor
                key={provider.id}
                provider={provider}
                models={modelsForProvider(provider)}
                onChange={updateProvider}
                onRemove={() => removeProvider(provider.id)}
                onLoadModels={loadModels}
              />
            ))}

            <button type="button" className="add-provider" onClick={addProvider}>
              + 添加服务
            </button>
          </section>

          <section className="settings-card">
            <h2>角色绑定</h2>
            <p className="muted">两个角色可以用不同厂商的模型。</p>
            {roleRow("fast", "释义", "划词后出释义，求快求便宜", settings.fast)}
            {roleRow("chat", "对话", "追问时使用，求强", settings.chat)}
          </section>

          <section className="settings-card">
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
            <p className="muted">改端口后需重启 Glossa，并重新安装一次 PopClip 扩展。</p>
          </section>

          <CaptureSection port={settings.port} savedPort={savedPort} />

          <PromptSection settings={settings} onPatch={patch} />

          <SpeechSection settings={settings} onPatch={patch} />

          <section className="settings-card">
            <h2>存储</h2>
            <label className="inline inline--wide">
              配置文件
              <span className="path-field">
                <input value={filePath} readOnly spellCheck={false} />
                <button
                  type="button"
                  title="复制路径"
                  onClick={() => void copyPath()}
                  disabled={!filePath}
                >
                  <IconCopy />
                  {copiedPath ? "已复制" : "复制"}
                </button>
              </span>
            </label>
            <p className="warn">
              <IconWarn />
              <span>
                API key 以明文保存在该文件中（权限 0600）。任何能读该文件的进程或用户都能拿到它，
                文件也会进入 Time Machine 备份与目录同步。
              </span>
            </p>
          </section>
        </div>
      </div>

      <footer className="settings__footer">
        {saved && <span className="muted">{saved}</span>}
        <button type="button" onClick={cancel}>
          取消
        </button>
        <button type="button" className="primary" onClick={() => void save()}>
          保存
        </button>
      </footer>
    </div>
  );
}
