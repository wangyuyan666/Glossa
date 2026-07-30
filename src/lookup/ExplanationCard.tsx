import type { ReactNode } from "react";

import type { Explanation } from "../lib/types";
import {
  IconFix,
  IconKey,
  IconQuote,
  IconStructure,
  IconSwap,
  IconTranslate,
} from "../ui/icons";

interface Props {
  explanation: Partial<Explanation> | null;
  streaming: boolean;
  /** 解析不出 JSON 时兜底展示的原始输出。 */
  raw: string;
}

export type Mode = "word" | "sentence" | "translate";

/**
 * 卡片用哪套字段。
 *
 * grammar 两套 schema 都有，不能拿它分流，只看各自独有的字段。流式期间字段是
 * 一个个到的，所以先到什么就按什么渲染，不等整份 JSON。
 */
export function modeOf(explanation: Partial<Explanation> | null): Mode {
  if (!explanation) return "word";
  if (
    explanation.english ||
    explanation.wordChoice?.length ||
    explanation.alternatives?.length
  ) {
    return "translate";
  }
  if (explanation.translation || explanation.structure || explanation.keyPoints?.length) {
    return "sentence";
  }
  return "word";
}

/** 一块内容一张带色标的卡。tone 决定色标，只是分类信号，不表达好坏。 */
function Panel({
  tone,
  icon,
  title,
  children,
}: {
  tone: "danger" | "ok" | "accent" | "plain";
  icon: ReactNode;
  title: string;
  children: ReactNode;
}) {
  return (
    <section className={`panel panel--${tone}`}>
      <h2 className="panel__head">
        {icon}
        {title}
      </h2>
      {children}
    </section>
  );
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

  const mode = modeOf(explanation);

  return (
    <div className={`card${streaming ? " card--streaming" : ""}`}>
      {mode === "translate" ? (
        <TranslateBody {...explanation} />
      ) : mode === "sentence" ? (
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
    <Panel tone="danger" icon={<IconFix />} title="句子修正">
      <p className="card__grammar-issue">{issue}</p>
      {corrected && <p className="card__grammar-fix">{corrected}</p>}
    </Panel>
  );
}

/** 术语 + 说明的成对列表，句子模式的难点和翻译模式的选词都是这个形状。 */
function TermRows({ items }: { items: { term: string; note: string }[] }) {
  return (
    <ul className="rows">
      {items.map((item, i) => (
        <li key={i}>
          <span className="rows__term">{item.term}</span>
          <span className="rows__note">{item.note}</span>
        </li>
      ))}
    </ul>
  );
}

function SentenceBody({ grammar, translation, structure, keyPoints }: Partial<Explanation>) {
  return (
    <>
      <GrammarNote grammar={grammar} />

      {translation && (
        <Panel tone="ok" icon={<IconTranslate />} title="中文翻译">
          <p className="card__sense">{translation}</p>
        </Panel>
      )}

      {(structure || !!keyPoints?.length) && (
        <Panel tone="accent" icon={<IconStructure />} title="语法解析">
          {structure && <p className="card__grammar-issue">{structure}</p>}
          {!!keyPoints?.length && <TermRows items={keyPoints} />}
        </Panel>
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
      {english && (
        <Panel tone="ok" icon={<IconTranslate />} title="英文表达">
          <p className="card__english">{english}</p>
        </Panel>
      )}

      {!!wordChoice?.length && (
        <Panel tone="accent" icon={<IconKey />} title="选词解析">
          <div className="keygrid">
            {wordChoice.map((point, i) => (
              <div key={i} className="keycard">
                <p className="keycard__term">{point.term}</p>
                <p className="keycard__note">{point.note}</p>
              </div>
            ))}
          </div>
        </Panel>
      )}

      {!!alternatives?.length && (
        <Panel tone="plain" icon={<IconSwap />} title="其他说法">
          <ul className="card__alternatives">
            {alternatives.map((alt, i) => (
              <li key={i}>
                <p className="card__alt-text">{alt.text}</p>
                {alt.when && <p className="card__alt-when">{alt.when}</p>}
              </li>
            ))}
          </ul>
        </Panel>
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

      {(senseHere || why || !!collocations?.length) && (
        <Panel tone="ok" icon={<IconTranslate />} title="这里的意思">
          {senseHere && <p className="card__sense">{senseHere}</p>}
          {why && <p className="card__why">{why}</p>}

          {!!collocations?.length && (
            <ul className="card__collocations">
              {collocations.map((c, i) => (
                <li key={i}>{c}</li>
              ))}
            </ul>
          )}
        </Panel>
      )}

      {example?.en && (
        <Panel tone="accent" icon={<IconQuote />} title="例句">
          <p className="card__example-en">{example.en}</p>
          {example.zh && <p className="card__example-zh">{example.zh}</p>}
        </Panel>
      )}
    </>
  );
}
