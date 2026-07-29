import { invoke } from "@tauri-apps/api/core";

import type { ChatMessage, LookupPayload, Provider, Settings } from "./types";

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
  text: string,
  context: string | null,
) => invoke<void>("explain", { streamId, text, context });

export const chatTurn = (streamId: string, messages: ChatMessage[]) =>
  invoke<void>("chat_turn", { streamId, messages });

export const hidePopup = () => invoke<void>("hide_popup");

export const openSettings = () => invoke<void>("open_settings");
