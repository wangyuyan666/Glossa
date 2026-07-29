import { useEffect, useState } from "react";

import * as api from "../lib/api";
import type {
  PromptTemplate,
  TemplateIssue,
  TemplateProbe,
} from "../lib/types";
import { TEMPLATE_KIND_LABELS } from "../lib/types";

interface Props {
  template: PromptTemplate;
  variables: string[];
  onChange: (template: PromptTemplate) => void;
  onClose: () => void;
}

const VARIABLE_HELP: Record<string, string> = {
  nativeLanguage: "解释语言，取「通用」里配的值",
  context: "选中词所在的原句；没有时替换成空串。通常不需要——选中的内容本身在用户消息里发",
};

/** 编辑一个用户模板：正文、静态检查、实测。 */
export function TemplateEditor({ template, variables, onChange, onClose }: Props) {
  const [issues, setIssues] = useState<TemplateIssue[]>([]);
  const [probe, setProbe] = useState<TemplateProbe | null>(null);
  const [probing, setProbing] = useState(false);
  const [probeError, setProbeError] = useState<string | null>(null);

  // 静态检查免费且瞬时，边打字边跑，不用等用户点按钮。
  useEffect(() => {
    const timer = setTimeout(() => {
      void api.checkTemplate(template.kind, template.body).then(setIssues);
    }, 300);
    return () => clearTimeout(timer);
  }, [template.kind, template.body]);

  const runProbe = async () => {
    setProbing(true);
    setProbeError(null);
    setProbe(null);
    try {
      setProbe(await api.probeTemplate(template.kind, template.body));
    } catch (e) {
      setProbeError(String(e));
    } finally {
      setProbing(false);
    }
  };

  return (
    <div className="editor">
      <div className="editor__head">
        <input
          className="editor__name"
          value={template.name}
          placeholder="模板名称"
          onChange={(e) => onChange({ ...template, name: e.target.value })}
        />
        <span className="editor__kind">{TEMPLATE_KIND_LABELS[template.kind]}</span>
        <button type="button" onClick={onClose}>
          收起
        </button>
      </div>

      <textarea
        className="editor__body"
        value={template.body}
        rows={16}
        spellCheck={false}
        onChange={(e) => onChange({ ...template, body: e.target.value })}
      />

      <details className="editor__vars">
        <summary>可用变量</summary>
        <dl>
          {variables.map((name) => (
            <div key={name}>
              <dt>
                <code>{`{{${name}}}`}</code>
              </dt>
              <dd>{VARIABLE_HELP[name] ?? ""}</dd>
            </div>
          ))}
        </dl>
        <p className="muted">
          选中的词或句子<strong>不是</strong>变量——它在用户消息里发，模板只负责「怎么解释」。
        </p>
      </details>

      {issues.map((issue, i) => (
        <p key={i} className={`status status--${issue.level === "error" ? "fail" : "busy"}`}>
          {issue.level === "error" ? "✗ " : "⚠ "}
          {issue.message}
        </p>
      ))}

      <div className="editor__actions">
        <button type="button" onClick={() => void runProbe()} disabled={probing}>
          {probing ? "实测中…" : "实测一次"}
        </button>
        <span className="muted">
          用固定样例真发一次请求。静态检查抓不到「模型不听你的」，只有实测能。
        </span>
      </div>

      {probeError && <p className="status status--fail">{probeError}</p>}

      {probe && (
        <div className="probe">
          <p className={`status status--${probe.parsed ? "ok" : "fail"}`}>
            {probe.parsed ? "✓ 输出可用" : "✗ 没解析出合法 JSON"}
          </p>
          {probe.missingFields.length > 0 && (
            <p className="status status--fail">
              缺少字段：{probe.missingFields.join("、")}。释义卡片会缺掉对应部分
            </p>
          )}
          <pre className="probe__raw">{probe.raw}</pre>
        </div>
      )}
    </div>
  );
}
