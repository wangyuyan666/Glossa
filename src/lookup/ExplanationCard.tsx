import type { ReactNode } from "react";

import type { Explanation, ExplanationMode } from "../lib/types";
import {
  IconFix,
  IconKey,
  IconQuote,
  IconStructure,
  IconSwap,
  IconTranslate,
} from "../ui/icons";
import { SpeakButton } from "./SpeakButton";

interface Props {
  explanation: Partial<Explanation> | null;
  streaming: boolean;
  /** 解析不出 JSON 时兜底展示的原始输出。 */
  raw: string;
}

export type Mode = ExplanationMode;

type UnknownRecord = Record<string, unknown>;

/**
 * 分支正文的 props。字段照旧整份摊开，另带一个 `streaming`——朗读按钮要靠它
 * 判断文本长完了没有，念半句比没得念更糟。
 */
type BodyProps = Partial<Explanation> & { streaming: boolean };

function textValue(value: unknown): string {
  return typeof value === "string" ? value : "";
}

function objectItems(value: unknown): UnknownRecord[] {
  return Array.isArray(value)
    ? value.filter(
        (item): item is UnknownRecord =>
          item !== null && typeof item === "object" && !Array.isArray(item),
      )
    : [];
}

function stringItems(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string")
    : [];
}

function exampleValue(value: unknown): UnknownRecord | null {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    ? (value as UnknownRecord)
    : null;
}

/**
 * 卡片用哪套字段。
 *
 * 新输出优先看模型给的 mode。旧历史没有 mode，仍按各分支独有字段推断。
 * 流式期间 mode 应当最先到；模型没遵守时，字段回退仍能避免空卡。
 */
export function detectedMode(explanation: Partial<Explanation> | null): Mode | null {
  if (!explanation) return null;
  if (
    explanation.mode === "word" ||
    explanation.mode === "sentence" ||
    explanation.mode === "translate"
  ) {
    return explanation.mode;
  }
  if (
    textValue(explanation.english).trim() ||
    objectItems(explanation.wordChoice).length ||
    objectItems(explanation.alternatives).length
  ) {
    return "translate";
  }
  if (
    textValue(explanation.translation).trim() ||
    textValue(explanation.structure).trim() ||
    objectItems(explanation.keyPoints).length
  ) {
    return "sentence";
  }
  const example = exampleValue(explanation.example);
  if (
    textValue(explanation.word).trim() ||
    textValue(explanation.phonetic).trim() ||
    textValue(explanation.pos).trim() ||
    textValue(explanation.senseHere).trim() ||
    textValue(explanation.why).trim() ||
    stringItems(explanation.collocations).length ||
    textValue(example?.en).trim()
  ) {
    return "word";
  }
  return null;
}

export function modeOf(explanation: Partial<Explanation> | null): Mode {
  return detectedMode(explanation) ?? "word";
}

export function hasRenderableExplanation(
  explanation: Partial<Explanation> | null,
): boolean {
  if (!explanation) return false;
  if (textValue(explanation.grammar?.issue).trim()) return true;

  switch (detectedMode(explanation)) {
    case "word": {
      const example = exampleValue(explanation.example);
      return Boolean(
        textValue(explanation.phonetic).trim() ||
          textValue(explanation.pos).trim() ||
          textValue(explanation.senseHere).trim() ||
          textValue(explanation.why).trim() ||
          stringItems(explanation.collocations).length ||
          textValue(example?.en).trim(),
      );
    }
    case "sentence":
      return Boolean(
        textValue(explanation.translation).trim() ||
          textValue(explanation.structure).trim() ||
          objectItems(explanation.keyPoints).length,
      );
    case "translate":
      return Boolean(
        textValue(explanation.english).trim() ||
          objectItems(explanation.wordChoice).length ||
          objectItems(explanation.alternatives).length,
      );
    default:
      return false;
  }
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
  // mode 先到时还没有可见内容，继续显示骨架；流结束仍没有内容则展示原始输出。
  if (!hasRenderableExplanation(explanation)) {
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
        <TranslateBody {...explanation} streaming={streaming} />
      ) : mode === "sentence" ? (
        <SentenceBody {...explanation} />
      ) : (
        <WordBody {...explanation} streaming={streaming} />
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
  const issue = textValue(grammar?.issue).trim();
  const corrected = textValue(grammar?.corrected).trim();
  if (!issue) return null;

  return (
    <Panel tone="danger" icon={<IconFix />} title="句子修正">
      <p className="card__grammar-issue">{issue}</p>
      {corrected && <p className="card__grammar-fix">{corrected}</p>}
    </Panel>
  );
}

/** 术语 + 说明的成对列表，句子模式的难点和翻译模式的选词都是这个形状。 */
function TermRows({ items }: { items: UnknownRecord[] }) {
  return (
    <ul className="rows">
      {items.map((item, i) => (
        <li key={i}>
          <span className="rows__term">{textValue(item.term)}</span>
          <span className="rows__note">{textValue(item.note)}</span>
        </li>
      ))}
    </ul>
  );
}

function SentenceBody({ grammar, translation, structure, keyPoints }: Partial<Explanation>) {
  const translationText = textValue(translation);
  const structureText = textValue(structure);
  const points = objectItems(keyPoints);

  return (
    <>
      <GrammarNote grammar={grammar} />

      {translationText && (
        <Panel tone="ok" icon={<IconTranslate />} title="中文翻译">
          <p className="card__sense">{translationText}</p>
        </Panel>
      )}

      {(structureText || points.length > 0) && (
        <Panel tone="accent" icon={<IconStructure />} title="语法解析">
          {structureText && <p className="card__grammar-issue">{structureText}</p>}
          {points.length > 0 && <TermRows items={points} />}
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
function TranslateBody({
  streaming,
  english,
  wordChoice,
  alternatives,
}: BodyProps) {
  const englishText = textValue(english);
  const choices = objectItems(wordChoice);
  const alternativesList = objectItems(alternatives);

  return (
    <>
      {englishText && (
        <Panel tone="ok" icon={<IconTranslate />} title="英文表达">
          <p className="card__english">
            {englishText}
            <SpeakButton
              text={englishText}
              label="朗读英文表达"
              disabled={streaming}
            />
          </p>
        </Panel>
      )}

      {choices.length > 0 && (
        <Panel tone="accent" icon={<IconKey />} title="选词解析">
          <div className="keygrid">
            {choices.map((point, i) => (
              <div key={i} className="keycard">
                <p className="keycard__term">{textValue(point.term)}</p>
                <p className="keycard__note">{textValue(point.note)}</p>
              </div>
            ))}
          </div>
        </Panel>
      )}

      {alternativesList.length > 0 && (
        <Panel tone="plain" icon={<IconSwap />} title="其他说法">
          <ul className="card__alternatives">
            {alternativesList.map((alt, i) => {
              const when = textValue(alt.when);
              return (
                <li key={i}>
                  <p className="card__alt-text">{textValue(alt.text)}</p>
                  {when && <p className="card__alt-when">{when}</p>}
                </li>
              );
            })}
          </ul>
        </Panel>
      )}
    </>
  );
}

function WordBody({
  streaming,
  grammar,
  phonetic,
  pos,
  senseHere,
  why,
  collocations,
  example,
}: BodyProps) {
  // 提示词没规定音标带不带斜杠，模型两种都给。统一剥掉再由我们包，
  // 否则自带斜杠的会显示成 //rɪˈzɪliənt//。
  const barePhonetic = textValue(phonetic).trim().replace(/^\/+|\/+$/g, "");
  const posText = textValue(pos);
  const senseText = textValue(senseHere);
  const whyText = textValue(why);
  const collocationList = stringItems(collocations);
  const exampleRecord = exampleValue(example);
  const exampleEn = textValue(exampleRecord?.en);
  const exampleZh = textValue(exampleRecord?.zh);

  return (
    <>
      <GrammarNote grammar={grammar} />

      {(barePhonetic || posText) && (
        <p className="card__meta">
          {barePhonetic && <span className="card__phonetic">/{barePhonetic}/</span>}
          {posText && <span className="card__pos">{posText}</span>}
        </p>
      )}

      {(senseText || whyText || collocationList.length > 0) && (
        <Panel tone="ok" icon={<IconTranslate />} title="这里的意思">
          {senseText && <p className="card__sense">{senseText}</p>}
          {whyText && <p className="card__why">{whyText}</p>}

          {collocationList.length > 0 && (
            <ul className="card__collocations">
              {collocationList.map((collocation, i) => (
                <li key={i}>{collocation}</li>
              ))}
            </ul>
          )}
        </Panel>
      )}

      {exampleEn && (
        <Panel tone="accent" icon={<IconQuote />} title="例句">
          <p className="card__example-en">
            {exampleEn}
            <SpeakButton text={exampleEn} label="朗读例句" disabled={streaming} />
          </p>
          {exampleZh && <p className="card__example-zh">{exampleZh}</p>}
        </Panel>
      )}
    </>
  );
}
