import { describe, expect, it } from "vitest";

import { clampRate, pickVoice, sortVoices, voiceTier, type VoiceLike } from "./speech";

function voice(name: string, lang: string): VoiceLike {
  return { name, lang };
}

describe("pickVoice", () => {
  it("prefers a downloaded premium voice over the preinstalled compact one", () => {
    const voices = [
      voice("Samantha", "en-US"),
      voice("Evan (Enhanced)", "en-US"),
      voice("Ava (Premium)", "en-US"),
    ];
    expect(pickVoice(voices, "en-US")?.name).toBe("Ava (Premium)");
  });

  it("honors the voice the user picked in settings, tier be damned", () => {
    const voices = [voice("Ava (Premium)", "en-US"), voice("Daniel", "en-GB")];
    expect(pickVoice(voices, "en-US", "Daniel")?.name).toBe("Daniel");
  });

  it("falls back to auto when the picked voice is no longer installed", () => {
    const voices = [voice("Samantha", "en-US"), voice("Ava (Premium)", "en-US")];
    expect(pickVoice(voices, "en-US", "Zoe (Premium)")?.name).toBe("Ava (Premium)");
  });

  it("accepts underscore locales some platforms report", () => {
    expect(pickVoice([voice("Alex", "en_US")], "en-US")?.name).toBe("Alex");
  });

  it("falls back to the language family rather than staying silent", () => {
    const voices = [voice("Ting-Ting", "zh-CN"), voice("Daniel", "en-GB")];
    expect(pickVoice(voices, "en-US")?.name).toBe("Daniel");
  });

  it("never returns a voice from another language", () => {
    expect(pickVoice([voice("Ting-Ting", "zh-CN")], "en-US")).toBeNull();
    expect(pickVoice([], "en-US")).toBeNull();
  });

  it("picks the same voice every time when several share the top tier", () => {
    const voices = [voice("Zoe (Premium)", "en-US"), voice("Ava (Premium)", "en-US")];
    expect(pickVoice(voices, "en-US")?.name).toBe("Ava (Premium)");
    expect(pickVoice([...voices].reverse(), "en-US")?.name).toBe("Ava (Premium)");
  });
});

describe("voiceTier", () => {
  it("reads the quality tier off the name suffix macOS uses", () => {
    expect(voiceTier("Ava (Premium)")).toBe(2);
    expect(voiceTier("Evan (Enhanced)")).toBe(1);
    expect(voiceTier("Samantha")).toBe(0);
  });

  it("does not mistake a locale suffix for a quality tier", () => {
    expect(voiceTier("Eddy (English (US))")).toBe(0);
  });
});

describe("sortVoices", () => {
  it("keeps the whole language family but floats the good ones up", () => {
    const voices = [
      voice("Zarvox", "en-US"),
      voice("Daniel", "en-GB"),
      voice("Ava (Premium)", "en-US"),
      voice("Karen", "en-AU"),
      voice("Ting-Ting", "zh-CN"),
    ];
    expect(sortVoices(voices, "en-US").map((v) => v.name)).toEqual([
      "Ava (Premium)",
      "Daniel",
      "Karen",
      "Zarvox",
    ]);
  });
});

describe("clampRate", () => {
  it("keeps the rate inside the audible range", () => {
    expect(clampRate(0)).toBe(0.5);
    expect(clampRate(10)).toBe(1.5);
    expect(clampRate(0.85)).toBe(0.85);
    expect(clampRate(Number.NaN)).toBe(1);
  });
});
