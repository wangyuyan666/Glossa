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
 *
 * 单词、句子、译成英文三套字段，按存在性分流：模型先吐哪个字段就先渲染哪套。
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

  // grammar 释义的两套 schema 都有，不能拿它分流，只看各自独有的字段。
  const isTranslate =
    !!explanation.english || !!explanation.wordChoice?.length || !!explanation.alternatives?.length;
  const isSentence =
    !!explanation.translation || !!explanation.structure || !!explanation.keyPoints?.length;

  return (
    <div className={`card${streaming ? " card--streaming" : ""}`}>
      {isTranslate ? (
        <TranslateBody {...explanation} />
      ) : isSentence ? (
        <SentenceBody {...explanation} />
      ) : (
        <WordBody {...explanation} />
      )}
    </div>
  );
}

/**
 * 原句 / 选中内容的语法纠错，排在释义之前。
 *
 * 只有真挑出问题才显示：没毛病时模型给的是空串，不能因为键存在就渲染一个空框。
 */
function GrammarNote({ grammar }: Pick<Partial<Explanation>, "grammar">) {
  const issue = grammar?.issue?.trim();
  const corrected = grammar?.corrected?.trim();
  if (!issue) return null;

  return (
    <div className="card__grammar">
      <p className="card__grammar-issue">{issue}</p>
      {corrected && <p className="card__grammar-fix">{corrected}</p>}
    </div>
  );
}

function SentenceBody({ grammar, translation, structure, keyPoints }: Partial<Explanation>) {
  return (
    <>
      <GrammarNote grammar={grammar} />

      {translation && <p className="card__sense">{translation}</p>}
      {structure && <p className="card__why">{structure}</p>}

      {!!keyPoints?.length && (
        <dl className="card__points">
          {keyPoints.map((point, i) => (
            <div key={i} className="card__point">
              <dt>{point.term}</dt>
              <dd>{point.note}</dd>
            </div>
          ))}
        </dl>
      )}
    </>
  );
}

/**
 * 选中内容不是英文时的卡片：译文在最上，选词理由是主体。
 *
 * 这里没有 GrammarNote——原文本来就不是英文。
 */
function TranslateBody({ english, wordChoice, alternatives }: Partial<Explanation>) {
  return (
    <>
      {english && <p className="card__english">{english}</p>}

      {!!wordChoice?.length && (
        <dl className="card__points">
          {wordChoice.map((point, i) => (
            <div key={i} className="card__point">
              <dt>{point.term}</dt>
              <dd>{point.note}</dd>
            </div>
          ))}
        </dl>
      )}

      {!!alternatives?.length && (
        <ul className="card__alternatives">
          {alternatives.map((alt, i) => (
            <li key={i}>
              <p className="card__alt-text">{alt.text}</p>
              {alt.when && <p className="card__alt-when">{alt.when}</p>}
            </li>
          ))}
        </ul>
      )}
    </>
  );
}

function WordBody({
  grammar,
  phonetic,
  pos,
  senseHere,
  why,
  collocations,
  example,
}: Partial<Explanation>) {
  // 提示词没规定音标带不带斜杠，模型两种都给。统一剥掉再由我们包，
  // 否则自带斜杠的会显示成 //rɪˈzɪliənt//。
  const barePhonetic = phonetic?.trim().replace(/^\/+|\/+$/g, "");

  return (
    <>
      <GrammarNote grammar={grammar} />

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
    </>
  );
}
