import { describe, expect, it } from "vitest";

import type { Provider } from "../lib/types";
import {
  canCommitModelRequest,
  changeRoleProvider,
  getCachedModels,
  isCurrentProviderGeneration,
  isLatestModelRequest,
  providerModelFingerprint,
} from "./Settings";

const provider = (fields: Partial<Provider> = {}): Provider => ({
  id: "provider-a",
  name: "Provider A",
  protocol: "openai",
  baseUrl: "https://api.example.com/v1",
  apiKey: "key-a",
  maxTokens: 4000,
  ...fields,
});

describe("changeRoleProvider", () => {
  it("clears the model when switching providers", () => {
    expect(
      changeRoleProvider({ providerId: "provider-a", model: "model-a" }, "provider-b"),
    ).toEqual({ providerId: "provider-b", model: "" });
  });

  it("starts a new provider binding with an empty model", () => {
    expect(changeRoleProvider(null, "provider-a")).toEqual({
      providerId: "provider-a",
      model: "",
    });
  });

  it("preserves the model when the provider is unchanged", () => {
    expect(
      changeRoleProvider({ providerId: "provider-a", model: "model-a" }, "provider-a"),
    ).toEqual({ providerId: "provider-a", model: "model-a" });
  });
});

describe("provider model cache", () => {
  it("invalidates when connection fields change", () => {
    const original = providerModelFingerprint(provider());

    expect(providerModelFingerprint(provider({ protocol: "anthropic" }))).not.toBe(original);
    expect(providerModelFingerprint(provider({ baseUrl: "https://other.example.com" }))).not.toBe(
      original,
    );
    expect(providerModelFingerprint(provider({ apiKey: "key-b" }))).not.toBe(original);
  });

  it("survives display name and output limit changes", () => {
    const original = providerModelFingerprint(provider());

    expect(providerModelFingerprint(provider({ name: "Renamed" }))).toBe(original);
    expect(providerModelFingerprint(provider({ maxTokens: 8000 }))).toBe(original);
  });

  it("uses matching cache for automatic loads, including an empty list", () => {
    const current = provider();
    const fingerprint = providerModelFingerprint(current);

    expect(getCachedModels({ fingerprint, models: ["model-a"] }, current, false)).toEqual([
      "model-a",
    ]);
    expect(getCachedModels({ fingerprint, models: [] }, current, false)).toEqual([]);
  });

  it("bypasses cache for manual loads", () => {
    const current = provider();
    const entry = {
      fingerprint: providerModelFingerprint(current),
      models: ["model-a"],
    };

    expect(getCachedModels(entry, current, true)).toBeNull();
  });

  it("rejects results from an outdated provider connection", () => {
    const requestProvider = provider();
    const fingerprint = providerModelFingerprint(requestProvider);

    expect(canCommitModelRequest(requestProvider, fingerprint)).toBe(true);
    expect(canCommitModelRequest(provider({ apiKey: "new-key" }), fingerprint)).toBe(false);
    expect(canCommitModelRequest(undefined, fingerprint)).toBe(false);
  });

  it("treats an untouched provider as generation zero", () => {
    expect(isCurrentProviderGeneration(undefined, 0)).toBe(true);
    expect(isCurrentProviderGeneration(1, 0)).toBe(false);
  });

  it("only commits the latest concurrent refresh", () => {
    expect(isLatestModelRequest(2, 2)).toBe(true);
    expect(isLatestModelRequest(2, 1)).toBe(false);
  });
});
