import type { Explanation } from "../lib/types";

interface Props {
  explanation: Partial<Explanation> | null;
  streaming: boolean;
  /** 解析不出 JSON 时兜底展示的原始输出。 */
  raw: string;
}

/**
 * 释义卡片。字段是流式逐个到齐的，所以每块都独立判空——
 * 不能等整份 JSON 收完再渲染，那样就没有"边收边看"的效果了。
 */
export function ExplanationCard({ explanation, streaming, raw }: Props) {
  // 还没解析出任何字段：显示骨架屏，而不是空白或原始 JSON 碎片。
  if (!explanation) {
    return streaming ? (
      <div className="card card--skeleton">
        <span />
        <span />
        <span />
      </div>
    ) : (
      <pre className="card card--raw">{raw}</pre>
    );
  }

  const { phonetic, pos, senseHere, why, collocations, example } = explanation;
  // 提示词没规定音标带不带斜杠，模型两种都给。统一剥掉再由我们包，
  // 否则自带斜杠的会显示成 //rɪˈzɪliənt//。
  const barePhonetic = phonetic?.trim().replace(/^\/+|\/+$/g, "");

  return (
    <div className={`card${streaming ? " card--streaming" : ""}`}>
      {(barePhonetic || pos) && (
        <p className="card__meta">
          {barePhonetic && <span className="card__phonetic">/{barePhonetic}/</span>}
          {pos && <span className="card__pos">{pos}</span>}
        </p>
      )}

      {senseHere && <p className="card__sense">{senseHere}</p>}
      {why && <p className="card__why">{why}</p>}

      {!!collocations?.length && (
        <ul className="card__collocations">
          {collocations.map((c, i) => (
            <li key={i}>{c}</li>
          ))}
        </ul>
      )}

      {example?.en && (
        <blockquote className="card__example">
          <p className="card__example-en">{example.en}</p>
          {example.zh && <p className="card__example-zh">{example.zh}</p>}
        </blockquote>
      )}
    </div>
  );
}
