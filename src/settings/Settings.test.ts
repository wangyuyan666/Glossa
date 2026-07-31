import { describe, expect, it } from "vitest";

import { changeRoleProvider } from "./Settings";

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
