import {
  IconCheckCircle,
  IconRefresh,
  IconSpinner,
  IconTest,
  IconXCircle,
} from "../ui/icons";

export type ModelAction = "models" | "test";

export interface ModelLoadResult {
  models: string[];
  source: "cache" | "network";
}

export type ModelActionStatus =
  | { kind: "idle" }
  | { kind: "loading"; action: ModelAction }
  | { kind: "success"; action: ModelAction; message: string }
  | { kind: "error"; action: ModelAction; message: string };

interface ModelActionButtonsProps {
  status: ModelActionStatus;
  disabled?: boolean;
  onGetModels: () => void;
  onTestConnection: () => void;
}

export function ModelActionButtons({
  status,
  disabled = false,
  onGetModels,
  onTestConnection,
}: ModelActionButtonsProps) {
  const loading = status.kind === "loading";
  const gettingModels = loading && status.action === "models";
  const testingConnection = loading && status.action === "test";

  return (
    <span className="settings-model-actions">
      <button
        type="button"
        onClick={onGetModels}
        disabled={disabled || loading}
        aria-busy={gettingModels || undefined}
      >
        {gettingModels ? <IconSpinner className="settings-model-spinner" /> : <IconRefresh />}
        {gettingModels ? "获取中…" : "获取模型"}
      </button>
      <button
        type="button"
        onClick={onTestConnection}
        disabled={disabled || loading}
        aria-busy={testingConnection || undefined}
      >
        {testingConnection ? <IconSpinner className="settings-model-spinner" /> : <IconTest />}
        {testingConnection ? "测试中…" : "测试连接"}
      </button>
    </span>
  );
}

export function ModelStatusBanner({ status }: { status: ModelActionStatus }) {
  if (status.kind !== "success" && status.kind !== "error") return null;

  return (
    <div
      className={`settings-model-status settings-model-status--${status.kind}`}
      role={status.kind === "error" ? "alert" : "status"}
      aria-live="polite"
    >
      {status.kind === "success" ? <IconCheckCircle /> : <IconXCircle />}
      <span>{status.message}</span>
    </div>
  );
}
