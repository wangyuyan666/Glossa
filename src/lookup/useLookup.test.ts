import { describe, expect, it } from "vitest";

import { explanationAsText } from "./useLookup";

describe("explanationAsText", () => {
  it("keeps translate output in the follow-up conversation seed", () => {
    const text = explanationAsText(
      {
        mode: "translate",
        english: "I've got this.",
        wordChoice: [{ term: "I've got this", note: "口语里表示我来处理。" }],
        alternatives: [{ text: "Leave it to me.", when: "更强调交给我。" }],
      },
      "fallback",
    );

    expect(text).toContain("释义模式：translate");
    expect(text).toContain("英文表达：I've got this.");
    expect(text).toContain("I've got this：口语里表示我来处理。");
    expect(text).toContain("Leave it to me.（更强调交给我。）");
  });

  it("ignores fields from branches other than the explicit mode", () => {
    const text = explanationAsText(
      {
        mode: "translate",
        english: "I've got this.",
        senseHere: "错误分支",
      },
      "fallback",
    );
    expect(text).toContain("英文表达：I've got this.");
    expect(text).not.toContain("错误分支");
  });

  it("uses raw fallback when a truncated response contains only mode", () => {
    expect(explanationAsText({ mode: "sentence" }, "{\"mode\":\"sentence\"}")).toBe(
      "{\"mode\":\"sentence\"}",
    );
  });
});
