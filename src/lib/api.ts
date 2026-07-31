import { invoke } from "@tauri-apps/api/core";

import type {
  ChatMessage,
  InstallOutcome,
  PromptTemplate,
  TemplateIssue,
  TemplateKind,
  TemplateProbe,
  LookupDetail,
  LookupPayload,
  LookupSummary,
  Provider,
  Settings,
  Voice,
} from "./types";

export const getSettings = () => invoke<Settings>("get_settings");

export const saveSettings = (settings: Settings) =>
  invoke<void>("save_settings", { settings });

export const settingsFilePath = () => invoke<string>("settings_file_path");

export const testProvider = (provider: Provider, model: string) =>
  invoke<string>("test_provider", { provider, model });

export const listModels = (provider: Provider) =>
  invoke<string[]>("list_models", { provider });

export const takePendingLookup = () =>
  invoke<LookupPayload | null>("take_pending_lookup");

export const explain = (
  streamId: string,
  lookupId: string,
  text: string,
  context: string | null,
) => invoke<void>("explain", { streamId, lookupId, text, context });

export const chatTurn = (streamId: string, messages: ChatMessage[]) =>
  invoke<void>("chat_turn", { streamId, messages });

export const builtinTemplates = () =>
  invoke<PromptTemplate[]>("builtin_templates");

export const templateVariables = () => invoke<string[]>("template_variables");

export const checkTemplate = (kind: TemplateKind, body: string) =>
  invoke<TemplateIssue[]>("check_template", { kind, body });

export const probeTemplate = (kind: TemplateKind, body: string) =>
  invoke<TemplateProbe>("probe_template", { kind, body });

export const historyList = (limit: number, offset: number, query: string | null) =>
  invoke<LookupSummary[]>("history_list", { limit, offset, query });

export const historyGet = (id: string) =>
  invoke<LookupDetail | null>("history_get", { id });

export const historyAppendTurn = (lookupId: string, role: string, content: string) =>
  invoke<void>("history_append_turn", { lookupId, role, content });

export const historyDelete = (id: string) => invoke<void>("history_delete", { id });

export const historyClear = () => invoke<void>("history_clear");

export const installPopclipExtension = () =>
  invoke<InstallOutcome>("install_popclip_extension");

export const popclipInstalled = () => invoke<boolean>("popclip_installed");

export const openSettings = () => invoke<void>("open_settings");

export const openMain = () => invoke<void>("open_main");

export const closeSettings = () => invoke<void>("close_settings");

export const listVoices = () => invoke<Voice[]>("list_voices");

/** 念完（或被打断）才 resolve，前端靠它决定喇叭何时熄灭。 */
export const speak = (text: string, voice: string | null, rate: number) =>
  invoke<void>("speak", { text, voice, rate });

export const stopSpeaking = () => invoke<void>("stop_speaking");
