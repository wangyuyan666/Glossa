/**
 * 容错的增量 JSON 解析。
 *
 * 释义走提示词约束的 JSON 而非 `response_format: json_schema`——多数 OpenAI 兼容端点
 * 不支持后者。代价是要自己处理两件事：
 *   1. 模型可能给 JSON 裹上 ```json 代码块；
 *   2. 流式期间拿到的永远是半截 JSON，但界面要边收边渲染。
 *
 * 做法：剥掉包裹 → 补全未闭合的字符串/括号 → 解析；解析失败就回退到更早的安全切点重试。
 */

/** 扫描结果：未闭合的括号栈、是否停在字符串内、可安全截断的位置。 */
interface Scan {
  closers: string[];
  inString: boolean;
  /** 各个「刚结束一个成员」的位置（顶层对象之内的逗号处），从早到晚。 */
  cutPoints: number[];
}

function scan(s: string): Scan {
  const closers: string[] = [];
  const cutPoints: number[] = [];
  let inString = false;
  let escaped = false;

  for (let i = 0; i < s.length; i++) {
    const c = s[i];

    if (escaped) {
      escaped = false;
      continue;
    }
    if (inString) {
      if (c === "\\") escaped = true;
      else if (c === '"') inString = false;
      continue;
    }

    if (c === '"') inString = true;
    else if (c === "{") closers.push("}");
    else if (c === "[") closers.push("]");
    else if (c === "}" || c === "]") closers.pop();
    else if (c === ",") cutPoints.push(i);
  }

  return { closers, inString, cutPoints };
}

/** 补全并尝试解析。失败返回 null。 */
function closeAndParse(s: string): unknown | null {
  const { closers, inString } = scan(s);

  let out = s;
  if (inString) out += '"';

  // 丢弃结尾未成形的片段：尾随逗号，以及只写了键还没写值的 `"key":`
  out = out.replace(/[\s,]+$/, "");
  out = out.replace(/"[^"]*"\s*:\s*$/, "");
  out = out.replace(/[\s,]+$/, "");

  for (let i = closers.length - 1; i >= 0; i--) out += closers[i];

  try {
    return JSON.parse(out);
  } catch {
    return null;
  }
}

/**
 * 从（可能不完整的）模型输出里解析出对象。
 *
 * @returns 已收到的字段构成的部分对象；一个字段都还没成形时返回 null。
 */
export function parsePartialJson<T>(raw: string): Partial<T> | null {
  let s = raw.trim();
  if (!s) return null;

  // 剥掉代码块包裹。收尾的 ``` 在流结束前不一定出现，所以两侧分开处理。
  s = s.replace(/^```[a-zA-Z]*\s*/, "").replace(/```\s*$/, "");

  const start = s.indexOf("{");
  if (start < 0) return null;
  s = s.slice(start);

  const direct = closeAndParse(s);
  if (direct && typeof direct === "object") return direct as Partial<T>;

  // 直接补全失败，说明结尾那段坏得比较深（例如数字写到一半、转义没写完）。
  // 回退到更早的逗号处重试，最多退 5 次，避免长输入上做无谓的全量回溯。
  const { cutPoints } = scan(s);
  for (let i = cutPoints.length - 1, tried = 0; i >= 0 && tried < 5; i--, tried++) {
    const candidate = closeAndParse(s.slice(0, cutPoints[i]));
    if (candidate && typeof candidate === "object") return candidate as Partial<T>;
  }

  return null;
}
