import { useEffect, useRef, useState } from "react";

import * as api from "../lib/api";
import type { Protocol, Provider } from "../lib/types";
import {
  DEFAULT_MAX_TOKENS,
  PROTOCOL_DEFAULT_BASE_URL,
  PROTOCOL_LABELS,
} from "../lib/types";
import { IconChevronDown, IconEye, IconEyeOff } from "../ui/icons";
import {
  ModelActionButtons,
  ModelStatusBanner,
  type ModelActionStatus,
  type ModelLoadResult,
} from "./ModelActions";
import { ModelCombobox } from "./ModelCombobox";

interface Props {
  provider: Provider;
  models: string[];
  onChange: (provider: Provider) => void;
  onRemove: () => void;
  onLoadModels: (provider: Provider, force: boolean) => Promise<ModelLoadResult>;
}

export function ProviderEditor({
  provider,
  models,
  onChange,
  onRemove,
  onLoadModels,
}: Props) {
  const [revealKey, setRevealKey] = useState(false);
  const [testModel, setTestModel] = useState("");
  const [status, setStatus] = useState<ModelActionStatus>({ kind: "idle" });
  const operation = useRef(0);

  const patch = (fields: Partial<Provider>) => onChange({ ...provider, ...fields });

  useEffect(() => {
    operation.current += 1;
    setStatus({ kind: "idle" });
  }, [provider.protocol, provider.baseUrl, provider.apiKey, provider.maxTokens]);

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
    const currentOperation = operation.current + 1;
    operation.current = currentOperation;
    setStatus({ kind: "loading", action: "models" });
    try {
      const { models: loadedModels } = await onLoadModels(provider, true);
      if (operation.current !== currentOperation) return;
      setStatus(
        loadedModels.length
          ? {
              kind: "success",
              action: "models",
              message: `已获取 ${loadedModels.length} 个模型`,
            }
          : {
              kind: "error",
              action: "models",
              message: "未获取到模型。该端点可能不支持模型列表，请手动填写模型名。",
            },
      );
    } catch (e) {
      if (operation.current !== currentOperation) return;
      setStatus({
        kind: "error",
        action: "models",
        message: `获取模型失败：${e}。可手动填写模型名后继续测试。`,
      });
    }
  };

  const test = async () => {
    const model = testModel.trim();
    if (!model) {
      operation.current += 1;
      setStatus({
        kind: "error",
        action: "test",
        message: "请先填写模型名，再测试连接。",
      });
      return;
    }

    const currentOperation = operation.current + 1;
    operation.current = currentOperation;
    setStatus({ kind: "loading", action: "test" });
    try {
      const reply = await api.testProvider(provider, model);
      if (operation.current !== currentOperation) return;
      setStatus({
        kind: "success",
        action: "test",
        message: `连接正常，模型回复：${reply}`,
      });
    } catch (e) {
      if (operation.current !== currentOperation) return;
      setStatus({
        kind: "error",
        action: "test",
        message: `测试连接失败：${e}`,
      });
    }
  };

  const handleTestModelChange = (value: string) => {
    setTestModel(value);
    if (status.kind !== "idle" && status.action === "test") {
      operation.current += 1;
      setStatus({ kind: "idle" });
    }
  };

  const inputId = `test-model-${provider.id}`;

  return (
    <fieldset className="provider">
      <div className="provider__head">
        <input
          className="provider__name"
          value={provider.name}
          placeholder="名称，如 OpenAI / DeepSeek / 本地 Ollama"
          onChange={(e) => patch({ name: e.target.value })}
        />
        <button type="button" className="danger provider__remove" onClick={onRemove}>
          删除
        </button>
      </div>

      <label>
        协议
        <span className="select-field">
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
          <IconChevronDown className="select-field__arrow" />
        </span>
      </label>

      <label>
        API Endpoint
        <input
          value={provider.baseUrl}
          placeholder={PROTOCOL_DEFAULT_BASE_URL[provider.protocol]}
          onChange={(e) => patch({ baseUrl: e.target.value })}
        />
      </label>

      <label>
        API Key
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
            onClick={() => setRevealKey((value) => !value)}
          >
            {revealKey ? <IconEyeOff /> : <IconEye />}
          </button>
        </span>
      </label>

      <label>
        输出上限
        <input
          type="number"
          min={256}
          step={1000}
          value={provider.maxTokens ?? ""}
          placeholder={`${DEFAULT_MAX_TOKENS}（默认）`}
          onChange={(e) => {
            // 清空 → null（回落默认值）。解析不出数字也当没填，别把 NaN 存进配置。
            const value = Number(e.target.value);
            patch({ maxTokens: Number.isFinite(value) && value > 0 ? value : null });
          }}
        />
        <small className="provider__note">
          单次输出的 token 上限，<strong>含推理模型的思考部分</strong>。留空用{" "}
          {DEFAULT_MAX_TOKENS}。端点撑得住更多时才往上调；填超过端点自身上限（常见是
          4096）会直接报 400。
        </small>
      </label>

      <div className="provider__test">
        <label htmlFor={inputId}>测试模型</label>
        <div className="settings-model-control">
          <ModelCombobox
            id={inputId}
            value={testModel}
            options={models}
            placeholder="填一个该端点上的模型名"
            onChange={handleTestModelChange}
          />
          <ModelActionButtons
            status={status}
            onGetModels={() => void fetchModels()}
            onTestConnection={() => void test()}
          />
        </div>
      </div>

      <ModelStatusBanner status={status} />
    </fieldset>
  );
}
