import { describe, expect, it } from "vitest";

import { filterModelOptions } from "./ModelCombobox";

const models = ["deepseek-v4-flash", "DeepSeek-Reasoner", "gpt-5.5"];

describe("filterModelOptions", () => {
  it("shows every model when the field is opened", () => {
    expect(filterModelOptions(models, "")).toEqual(models);
  });

  it("filters model names case-insensitively", () => {
    expect(filterModelOptions(models, "DEEPSEEK")).toEqual([
      "deepseek-v4-flash",
      "DeepSeek-Reasoner",
    ]);
  });

  it("keeps manual input possible when nothing matches", () => {
    expect(filterModelOptions(models, "custom-model")).toEqual([]);
  });
});
