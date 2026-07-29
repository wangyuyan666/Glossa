import { describe, expect, it } from "vitest";

import { parsePartialJson } from "./jsonish";

interface Exp {
  word: string;
  senseHere: string;
  collocations: string[];
  example: { en: string; zh: string };
}

const FULL = {
  word: "take on",
  senseHere: "承担、接下",
  collocations: ["take on a challenge", "take on responsibility"],
  example: { en: "She took it on.", zh: "她接下了。" },
};

describe("parsePartialJson", () => {
  it("解析完整 JSON", () => {
    expect(parsePartialJson<Exp>(JSON.stringify(FULL))).toEqual(FULL);
  });

  it("剥掉 ```json 代码块包裹", () => {
    const raw = "```json\n" + JSON.stringify(FULL) + "\n```";
    expect(parsePartialJson<Exp>(raw)).toEqual(FULL);
  });

  it("流式期间收尾的 ``` 还没出现也能解析", () => {
    expect(parsePartialJson<Exp>("```json\n" + JSON.stringify(FULL))).toEqual(FULL);
  });

  it("忽略 JSON 之前的碎话", () => {
    expect(parsePartialJson<Exp>('好的，结果如下：\n{"word":"take on"}')).toEqual({
      word: "take on",
    });
  });

  it("字符串截断在半路时补全，已完成的字段照常返回", () => {
    const parsed = parsePartialJson<Exp>('{"word":"take on","senseHere":"承担、接');
    expect(parsed?.word).toBe("take on");
  });

  it("键写了值还没写时丢弃该键", () => {
    const parsed = parsePartialJson<Exp>('{"word":"take on","senseHere":');
    expect(parsed).toEqual({ word: "take on" });
  });

  it("尾随逗号不影响解析", () => {
    expect(parsePartialJson<Exp>('{"word":"take on",')).toEqual({ word: "take on" });
  });

  it("补全未闭合的数组", () => {
    const parsed = parsePartialJson<Exp>(
      '{"word":"take on","collocations":["take on a challenge","take on res',
    );
    expect(parsed?.word).toBe("take on");
    expect(parsed?.collocations?.[0]).toBe("take on a challenge");
  });

  it("补全嵌套对象", () => {
    const parsed = parsePartialJson<Exp>(
      '{"word":"take on","example":{"en":"She took it on.","zh":"她接',
    );
    expect(parsed?.example?.en).toBe("She took it on.");
  });

  it("字符串里的花括号和引号不影响括号配平", () => {
    const parsed = parsePartialJson<Exp>('{"word":"a \\"{\\" brace","senseHere":"x"}');
    expect(parsed).toEqual({ word: 'a "{" brace', senseHere: "x" });
  });

  it("转义符停在末尾时不会解析成坏字符串", () => {
    const parsed = parsePartialJson<Exp>('{"word":"take on","senseHere":"承担\\');
    expect(parsed?.word).toBe("take on");
  });

  it("还没出现 { 时返回 null", () => {
    expect(parsePartialJson<Exp>("好的")).toBeNull();
    expect(parsePartialJson<Exp>("")).toBeNull();
  });
});
