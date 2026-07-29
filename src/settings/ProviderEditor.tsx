import { useState } from "react";

import * as api from "../lib/api";
import type { Protocol, Provider } from "../lib/types";
import { PROTOCOL_DEFAULT_BASE_URL, PROTOCOL_LABELS } from "../lib/types";

interface Props {
  provider: Provider;
  models: string[];
  onChange: (provider: Provider) => void;
  onRemove: () => void;
  onModelsLoaded: (models: string[]) => void;
}

type Status =
  | { kind: "idle" }
  | { kind: "busy"; label: string }
  | { kind: "ok"; message: string }
  | { kind: "fail"; message: string };

export function ProviderEditor({
  provider,
  models,
  onChange,
  onRemove,
  onModelsLoaded,
}: Props) {
  const [revealKey, setRevealKey] = useState(false);
  const [testModel, setTestModel] = useState("");
  const [status, setStatus] = useState<Status>({ kind: "idle" });

  const patch = (fields: Partial<Provider>) => onChange({ ...provider, ...fields });

  const changeProtocol = (protocol: Protocol) => {
    // base_url 还停在上一个协议的默认值时才跟着换，避免覆盖用户填的自定义地址。
    const isDefault =
      !provider.baseUrl.trim() ||
      Object.values(PROTOCOL_DEFAULT_BASE_URL).includes(provider.baseUrl.trim());
    patch({
      protocol,
      baseUrl: isDefault ? PROTOCOL_DEFAULT_BASE_URL[protocol] : provider.baseUrl,
    });
  };

  const fetchModels = async () => {
    setStatus({ kind: "busy", label: "拉取模型…" });
    try {
      const list = await api.listModels(provider);
      onModelsLoaded(list);
      setStatus(
        list.length
          ? { kind: "ok", message: `拉到 ${list.length} 个模型` }
          : { kind: "fail", message: "端点返回空列表，请手填模型名" },
      );
    } catch (e) {
      // 不少兼容端点没实现 /models，这不算配置错误，提示手填即可。
      setStatus({ kind: "fail", message: `拉取失败，请手填模型名：${e}` });
    }
  };

  const test = async () => {
    const model = testModel.trim();
    if (!model) {
      setStatus({ kind: "fail", message: "先填一个模型名再测试" });
      return;
    }
    setStatus({ kind: "busy", label: "测试中…" });
    try {
      const reply = await api.testProvider(provider, model);
      setStatus({ kind: "ok", message: `连接正常，模型回复：${reply}` });
    } catch (e) {
      setStatus({ kind: "fail", message: String(e) });
    }
  };

  const busy = status.kind === "busy";
  const listId = `models-${provider.id}`;

  return (
    <fieldset className="provider">
      <div className="provider__head">
        <input
          className="provider__name"
          value={provider.name}
          placeholder="名称，如 OpenAI / DeepSeek / 本地 Ollama"
          onChange={(e) => patch({ name: e.target.value })}
        />
        <button type="button" className="danger" onClick={onRemove}>
          删除
        </button>
      </div>

      <label>
        协议
        <select
          value={provider.protocol}
          onChange={(e) => changeProtocol(e.target.value as Protocol)}
        >
          {Object.entries(PROTOCOL_LABELS).map(([value, label]) => (
            <option key={value} value={value}>
              {label}
            </option>
          ))}
        </select>
      </label>

      <label>
        base_url
        <input
          value={provider.baseUrl}
          placeholder={PROTOCOL_DEFAULT_BASE_URL[provider.protocol]}
          onChange={(e) => patch({ baseUrl: e.target.value })}
        />
      </label>

      <label>
        api_key
        <span className="key-field">
          <input
            type={revealKey ? "text" : "password"}
            value={provider.apiKey}
            autoComplete="off"
            spellCheck={false}
            onChange={(e) => patch({ apiKey: e.target.value })}
          />
          <button
            type="button"
            className="key-field__eye"
            title={revealKey ? "隐藏" : "显示明文"}
            onClick={() => setRevealKey((v) => !v)}
          >
            {revealKey ? "🙈" : "👁"}
          </button>
        </span>
      </label>

      <label>
        测试模型
        <span className="key-field">
          <input
            value={testModel}
            list={listId}
            placeholder="填一个该端点上的模型名"
            onChange={(e) => setTestModel(e.target.value)}
          />
          <datalist id={listId}>
            {models.map((m) => (
              <option key={m} value={m} />
            ))}
          </datalist>
        </span>
      </label>

      <div className="provider__buttons">
        <button type="button" onClick={() => void fetchModels()} disabled={busy}>
          拉取模型列表
        </button>
        <button type="button" onClick={() => void test()} disabled={busy}>
          测试连接
        </button>
      </div>

      {status.kind !== "idle" && (
        <p className={`status status--${status.kind}`}>
          {status.kind === "busy" ? status.label : status.message}
        </p>
      )}
    </fieldset>
  );
}
