import { describe, expect, it } from "vitest";

import {
  detectedMode,
  hasRenderableExplanation,
  modeOf,
} from "./ExplanationCard";

describe("detectedMode", () => {
  it("prefers the explicit mode from unified output", () => {
    expect(detectedMode({ mode: "sentence", word: "wrong branch" })).toBe("sentence");
  });

  it("keeps old history working by inferring substantive unique fields", () => {
    expect(detectedMode({ senseHere: "承担" })).toBe("word");
    expect(detectedMode({ translation: "你叫什么名字？" })).toBe("sentence");
    expect(detectedMode({ english: "I've got this." })).toBe("translate");
    expect(
      detectedMode({
        senseHere: "承担",
        english: "",
        wordChoice: [],
        alternatives: [],
      }),
    ).toBe("word");
  });

  it("does not claim word mode when only common grammar arrived", () => {
    expect(detectedMode({ grammar: { issue: "", corrected: "" } })).toBeNull();
    expect(modeOf({ grammar: { issue: "", corrected: "" } })).toBe("word");
  });

  it("ignores malformed runtime values instead of calling string methods on them", () => {
    const malformed = {
      mode: "sentence",
      translation: 123,
      structure: false,
      keyPoints: "not-an-array",
    } as unknown as Parameters<typeof hasRenderableExplanation>[0];
    expect(hasRenderableExplanation(malformed)).toBe(false);
  });

  it("does not treat mode alone as renderable explanation content", () => {
    expect(hasRenderableExplanation({ mode: "sentence" })).toBe(false);
    expect(
      hasRenderableExplanation({ mode: "sentence", translation: "你叫什么名字？" }),
    ).toBe(true);
    expect(
      hasRenderableExplanation({ mode: "sentence", senseHere: "错误分支" }),
    ).toBe(false);
  });
});
