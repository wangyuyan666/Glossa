import { useEffect, useState } from "react";

import * as api from "../lib/api";
import type {
  PromptTemplate,
  Settings as SettingsData,
  TemplateKind,
} from "../lib/types";
import { TEMPLATE_KIND_LABELS } from "../lib/types";
import { TemplateEditor } from "./TemplateEditor";

interface Props {
  settings: SettingsData;
  onPatch: (fields: Partial<SettingsData>) => void;
}

const KINDS: TemplateKind[] = ["word", "sentence", "chat"];

/** 各类当前选中的模板 id 存在不同字段里，这里统一读写。 */
function activeField(kind: TemplateKind): keyof SettingsData {
  return kind === "word"
    ? "activeWord"
    : kind === "sentence"
      ? "activeSentence"
      : "activeChat";
}

export function PromptSection({ settings, onPatch }: Props) {
  const [builtins, setBuiltins] = useState<PromptTemplate[]>([]);
  const [variables, setVariables] = useState<string[]>([]);
  const [editingId, setEditingId] = useState<string | null>(null);

  useEffect(() => {
    void api.builtinTemplates().then(setBuiltins);
    void api.templateVariables().then(setVariables);
  }, []);

  const all = [...builtins, ...settings.templates];
  const editing = settings.templates.find((t) => t.id === editingId) ?? null;

  const setActive = (kind: TemplateKind, id: string | null) => {
    onPatch({ [activeField(kind)]: id } as Partial<SettingsData>);
  };

  /** 从内置模板复制一份可编辑的。没人愿意从空白开始写提示词。 */
  const copyFrom = (source: PromptTemplate) => {
    const copy: PromptTemplate = {
      id: crypto.randomUUID(),
      name: `${source.name} 副本`,
      kind: source.kind,
      body: source.body,
      builtin: false,
    };
    onPatch({ templates: [...settings.templates, copy] });
    setEditingId(copy.id);
  };

  const updateTemplate = (next: PromptTemplate) => {
    onPatch({
      templates: settings.templates.map((t) => (t.id === next.id ? next : t)),
    });
  };

  const removeTemplate = (template: PromptTemplate) => {
    // 正被选用的模板删掉后，选用状态一并清空，否则会留下指向空气的 id。
    const patch: Partial<SettingsData> = {
      templates: settings.templates.filter((t) => t.id !== template.id),
    };
    if (settings[activeField(template.kind)] === template.id) {
      Object.assign(patch, { [activeField(template.kind)]: null });
    }
    onPatch(patch);
    if (editingId === template.id) setEditingId(null);
  };

  return (
    <section>
      <h2>提示词</h2>
      <p className="muted">
        内置模板不可修改、不可删除，但可以「复制为我的」再改。删掉正在用的模板会自动回落到内置。
      </p>

      {KINDS.map((kind) => {
        const active = settings[activeField(kind)] as string | null;
        const options = all.filter((t) => t.kind === kind);
        return (
          <div key={kind} className="role">
            <div className="role__label">
              <strong>{TEMPLATE_KIND_LABELS[kind]}</strong>
            </div>
            <select
              value={active ?? ""}
              onChange={(e) => setActive(kind, e.target.value || null)}
            >
              <option value="">内置</option>
              {options
                .filter((t) => !t.builtin)
                .map((t) => (
                  <option key={t.id} value={t.id}>
                    {t.name || "(未命名)"}
                  </option>
                ))}
            </select>
          </div>
        );
      })}

      <h3 className="prompts__subhead">管理模板</h3>
      <ul className="templates">
        {all.map((template) => (
          <li key={template.id} className="template">
            <span className="template__name">
              {template.builtin && <span title="内置，不可修改">🔒 </span>}
              {template.name || "(未命名)"}
            </span>
            <span className="template__kind">{TEMPLATE_KIND_LABELS[template.kind]}</span>
            {template.builtin ? (
              <button type="button" onClick={() => copyFrom(template)}>
                复制为我的
              </button>
            ) : (
              <>
                <button
                  type="button"
                  onClick={() =>
                    setEditingId(editingId === template.id ? null : template.id)
                  }
                >
                  {editingId === template.id ? "收起" : "编辑"}
                </button>
                <button
                  type="button"
                  className="danger"
                  onClick={() => removeTemplate(template)}
                >
                  删除
                </button>
              </>
            )}
          </li>
        ))}
      </ul>

      {editing && (
        <TemplateEditor
          template={editing}
          variables={variables}
          onChange={updateTemplate}
          onClose={() => setEditingId(null)}
        />
      )}
    </section>
  );
}
