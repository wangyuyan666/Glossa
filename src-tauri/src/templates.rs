//! 提示词模板。
//!
//! 释义与追问两类模板独立选用。释义模板由模型自行判断单词、句子或译成英文。
//!
//! **内置模板留在代码里，不写进 `settings.json`**，配置只记「当前选了哪个 id」。
//! 内置提示词以后还会改（比如又发现一类输出 bug），把副本存进用户配置的话，
//! 升级后用户还在跑旧提示词，而且完全看不出来。

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TemplateKind {
    /// 统一释义。模型自行判断 word / sentence / translate，输出对应 JSON 分支。
    Explain,
    /// 追问对话。自由文本，无字段契约。
    Chat,
    /// 旧版模板类型。只为兼容已有 settings.json，不再参与真实释义分流。
    Word,
    /// 旧版模板类型。只保留用户内容，不再参与真实释义分流。
    Sentence,
    /// 旧版模板类型。只保留用户内容，不再参与真实释义分流。
    Translate,
}

impl TemplateKind {
    /// 设置页可选、真实请求会使用的模板类型。
    pub const SELECTABLE: [TemplateKind; 2] = [Self::Explain, Self::Chat];
    /// 含旧版类型。用于清理历史内置副本及兼容测试。
    pub const ALL: [TemplateKind; 5] = [
        Self::Explain,
        Self::Chat,
        Self::Word,
        Self::Sentence,
        Self::Translate,
    ];
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PromptTemplate {
    pub id: String,
    pub name: String,
    pub kind: TemplateKind,
    pub body: String,
    /// 内置模板不可改不可删。用户模板反序列化时该字段缺省为 false，
    /// 保存时也会被强制置回 false，防止伪造。
    #[serde(default)]
    pub builtin: bool,
}

pub const BUILTIN_EXPLAIN_ID: &str = "builtin-explain";
pub const BUILTIN_CHAT_ID: &str = "builtin-chat";
// 旧 id 仍参与 sanitize，防止旧 settings.json 里的内置副本被当作用户模板保存。
pub const BUILTIN_WORD_ID: &str = "builtin-word";
pub const BUILTIN_SENTENCE_ID: &str = "builtin-sentence";
pub const BUILTIN_TRANSLATE_ID: &str = "builtin-translate";

/// 模板正文里可用的占位符。静态检查据此判定「未知变量」。
pub const VARIABLES: [&str; 2] = ["nativeLanguage", "context"];

/// 正文超过这个长度就提醒——附加约束太多会稀释 JSON 格式要求，模型开始不听话。
const LONG_BODY_CHARS: usize = 2000;

pub fn builtin_id(kind: TemplateKind) -> &'static str {
    match kind {
        TemplateKind::Explain => BUILTIN_EXPLAIN_ID,
        TemplateKind::Chat => BUILTIN_CHAT_ID,
        TemplateKind::Word => BUILTIN_WORD_ID,
        TemplateKind::Sentence => BUILTIN_SENTENCE_ID,
        TemplateKind::Translate => BUILTIN_TRANSLATE_ID,
    }
}

pub fn builtin(kind: TemplateKind) -> PromptTemplate {
    let (name, body) = match kind {
        TemplateKind::Explain => ("内置 · 统一释义", EXPLAIN_BODY),
        TemplateKind::Chat => ("内置 · 追问对话", CHAT_BODY),
        // 旧版内置只用于兼容测试和识别历史 id，不再发给设置页，也不参与真实请求。
        TemplateKind::Word => ("旧版内置 · 单词释义", WORD_BODY),
        TemplateKind::Sentence => ("旧版内置 · 句子释义", SENTENCE_BODY),
        TemplateKind::Translate => ("旧版内置 · 译成英文", TRANSLATE_BODY),
    };
    PromptTemplate {
        id: builtin_id(kind).to_string(),
        name: name.to_string(),
        kind,
        body: body.to_string(),
        builtin: true,
    }
}

pub fn builtins() -> Vec<PromptTemplate> {
    TemplateKind::SELECTABLE
        .iter()
        .copied()
        .map(builtin)
        .collect()
}

/// JSON 顶层必须带的字段。统一模板正文还会按各 mode 检查对应分支字段。
pub fn required_fields(kind: TemplateKind) -> &'static [&'static str] {
    match kind {
        TemplateKind::Explain => &["mode"],
        TemplateKind::Word => word_fields(),
        TemplateKind::Sentence => sentence_fields(),
        TemplateKind::Translate => translate_fields(),
        TemplateKind::Chat => &[],
    }
}

pub fn word_fields() -> &'static [&'static str] {
    &[
        "word",
        "phonetic",
        "pos",
        "senseHere",
        "why",
        "collocations",
        "example",
    ]
}

pub fn sentence_fields() -> &'static [&'static str] {
    &["translation", "structure", "keyPoints"]
}

pub fn translate_fields() -> &'static [&'static str] {
    &["english", "wordChoice", "alternatives"]
}

pub fn fields_for_mode(mode: &str) -> &'static [&'static str] {
    match mode {
        "word" => word_fields(),
        "sentence" => sentence_fields(),
        "translate" => translate_fields(),
        _ => &[],
    }
}

/// 替换占位符。未知变量原样保留——静默变成空串的话，用户拼错了也发现不了，
/// 只会觉得「模板好像没生效」。静态检查会把它们报出来。
pub fn render(body: &str, native_language: &str, context: Option<&str>) -> String {
    body.replace("{{nativeLanguage}}", native_language)
        .replace("{{context}}", context.unwrap_or_default())
}

/// 正文里出现的所有 `{{...}}` 变量名，按出现顺序、去重。
pub fn variables_used(body: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut rest = body;

    while let Some(start) = rest.find("{{") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else { break };
        let name = after[..end].trim().to_string();
        if !name.is_empty() && !found.contains(&name) {
            found.push(name);
        }
        rest = &after[end + 2..];
    }
    found
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TemplateIssue {
    /// "error" 会让模板不可用，"warn" 只是提醒。
    pub level: String,
    pub message: String,
}

impl TemplateIssue {
    fn error(message: impl Into<String>) -> Self {
        Self {
            level: "error".into(),
            message: message.into(),
        }
    }

    fn warn(message: impl Into<String>) -> Self {
        Self {
            level: "warn".into(),
            message: message.into(),
        }
    }
}

fn mentions_field(body: &str, field: &str) -> bool {
    let quoted = format!("\"{field}\"");
    let json_key = body
        .match_indices(&quoted)
        .any(|(start, _)| body[start + quoted.len()..].trim_start().starts_with(':'));
    json_key || body.contains(&format!("`{field}`"))
}

/// 静态检查：本地、免费、瞬时。
///
/// 只能防低级错误（拼错变量、漏写字段名）。「模型不听话」这类问题静态检查发现不了，
/// 那要靠实测（见 `lib.rs` 的 `probe_template`）。
pub fn check(kind: TemplateKind, body: &str) -> Vec<TemplateIssue> {
    let mut issues = Vec::new();

    if body.trim().is_empty() {
        issues.push(TemplateIssue::error("模板正文为空"));
        return issues;
    }

    let unknown: Vec<String> = variables_used(body)
        .into_iter()
        .filter(|name| !VARIABLES.contains(&name.as_str()))
        .collect();
    if !unknown.is_empty() {
        issues.push(TemplateIssue::error(format!(
            "未知变量：{}。可用的只有 {}",
            unknown.join("、"),
            VARIABLES
                .iter()
                .map(|v| format!("{{{{{v}}}}}"))
                .collect::<Vec<_>>()
                .join("、")
        )));
    }

    let required: Vec<&str> = if kind == TemplateKind::Explain {
        required_fields(kind)
            .iter()
            .copied()
            .chain(["grammar", "issue", "corrected"])
            .chain(word_fields().iter().copied())
            .chain(sentence_fields().iter().copied())
            .chain(translate_fields().iter().copied())
            .collect()
    } else {
        required_fields(kind).to_vec()
    };
    let missing: Vec<&str> = required
        .iter()
        .copied()
        .filter(|field| !mentions_field(body, field))
        .collect();
    if !missing.is_empty() {
        issues.push(TemplateIssue::error(format!(
            "正文里没有提到这些必需字段：{}。缺了它们，释义卡片会渲染不出对应部分",
            missing.join("、")
        )));
    }

    if body.chars().count() > LONG_BODY_CHARS {
        issues.push(TemplateIssue::warn(format!(
            "正文超过 {LONG_BODY_CHARS} 字。约束越多，模型越容易忽略 JSON 格式要求，建议精简"
        )));
    }

    issues
}

// ---------------------------------------------------------------- 内置正文
//
// 释义走结构化 JSON，但**不使用** `response_format: json_schema` 或 tool use——
// 多数 OpenAI 兼容端点（Ollama、LM Studio、部分中转）不支持或行为不一致。
//
// 统一模板不是把三套字段全塞进同一个对象：模型先判断 mode，再只输出对应分支。
// 如果要求所有字段齐全，word/phonetic/pos 仍会逼模型从整句或中文里硬挑一个词。

const EXPLAIN_BODY: &str = r#"你是一位英语老师，帮助母语为{{nativeLanguage}}的学习者理解或表达语言。

用户会给出一段选中的内容，并可能附带所在原句。先根据语言结构和用户实际意图判断模式，**不要按词数、字符数或是否有句末标点硬判**：
- `word`：英文单词、固定搭配或名词短语，用户想知道它在语境中的意思。
- `sentence`：英文完整话语、分句或可独立理解的表达；短问句、口语、省略句、缺标点或有拼写错误时仍可属于句子。
- `translate`：内容主要不是英文，用户想知道地道英文怎么说。

只输出一个 JSON 对象，不要输出解释性文字，不要包裹代码块。`mode` 必须是第一个字段，值只能是 `word`、`sentence`、`translate`。判断后只输出对应分支的字段，**不要补另外两个分支的键**。

`word` 分支：结合所在原句解释此处义项；没有原句时给最常见义项。先检查选中内容的拼写、词形或搭配，有错写进 `grammar`，无错两项给空字符串。
{
  "mode": "word",
  "grammar": { "issue": "用{{nativeLanguage}}说明错误；无错留空", "corrected": "改正后的英文；无错留空" },
  "word": "词条原形",
  "phonetic": "IPA 音标，不确定留空",
  "pos": "词性缩写",
  "senseHere": "用{{nativeLanguage}}给出此处含义，一句话",
  "why": "用{{nativeLanguage}}说明语感或为什么在此处是这个意思",
  "collocations": ["常见英文搭配，2 到 4 个"],
  "example": { "en": "地道英文例句", "zh": "{{nativeLanguage}}翻译" }
}

`sentence` 分支：解释整段内容，**不许只挑其中一个单词讲**。先检查拼写和语法；有错给出改正后的完整表达，后续仍讲用户原文。
{
  "mode": "sentence",
  "grammar": { "issue": "用{{nativeLanguage}}说明错误；无错留空", "corrected": "改正后的完整英文；无错留空" },
  "translation": "完整、自然的{{nativeLanguage}}翻译",
  "structure": "用{{nativeLanguage}}说明句子结构或关键语法，一到两句",
  "keyPoints": [{ "term": "真正影响理解的原文词或短语", "note": "用{{nativeLanguage}}解释此处用法" }]
}
`keyPoints` 给 1 到 4 项；没有难点可给空数组。

`translate` 分支：给一个母语者会自然说出口的英文版本，不要逐字直译。默认日常口语，原文明显正式或粗俗时跟随语气。`wordChoice` 解释地道搭配、近义词取舍或意译理由。
{
  "mode": "translate",
  "english": "最地道的英文说法",
  "wordChoice": [{ "term": "译文里的词或短语", "note": "用{{nativeLanguage}}说明为什么这样选词" }],
  "alternatives": [{ "text": "实质不同的另一种完整说法", "when": "用{{nativeLanguage}}说明语气或场合差别" }]
}

对应分支的字段必须齐全。无法判断的字符串给空字符串，列表给空数组，不要省略该分支的键。"#;

// 以下三份正文只为读取和保留旧版自定义模板、识别旧内置 id；真实释义不再使用。
const WORD_BODY: &str = r#"你是一位英语老师，帮助母语为{{nativeLanguage}}的学习者理解英文词汇。

用户会给出选中的词。如果同时给出了它所在的原句，你必须解释**这个词在该句中的具体含义**，而不是罗列全部义项，`why` 字段说明为什么在此处是这个意思；如果没有给出原句，给出最常见的义项，`why` 字段简述该词的核心语感或最典型的使用场景。

解释之前先检查选中内容本身有没有拼写或语法错误（拼错、词形不对、搭配错、多余或缺失的成分等）。有错就在 `grammar` 里指出错在哪、给出改正后的写法。**不许默默按改正后的内容解释而不吭声**——其余字段可以按正确写法讲，但错误必须先在 `grammar` 里点出来。选中内容没有错误时，`grammar` 的两项都给空字符串。

只输出一个 JSON 对象，不要输出任何解释性文字，不要包裹代码块。JSON 结构如下：

{
  "grammar": {
    "issue": "用{{nativeLanguage}}说明选中内容的拼写或语法错误，一到两句；没有错误则给空字符串",
    "corrected": "改正后的英文写法；没有错误则给空字符串"
  },
  "word": "词条原形",
  "phonetic": "IPA 音标，不确定则留空字符串",
  "pos": "词性缩写，如 v. / n. / adj. / phr.",
  "senseHere": "用{{nativeLanguage}}给出此处含义，一句话，不超过 40 字",
  "why": "用{{nativeLanguage}}说明理由或语感，一到两句",
  "collocations": ["常见搭配，2 到 4 个，英文原文"],
  "example": { "en": "一个地道英文例句", "zh": "该例句的{{nativeLanguage}}翻译" }
}

字段必须齐全。无法判断的字段给空字符串或空数组，不要省略键。grammar 的两个子字段要么都填，要么都留空，不要只填一个。"#;

const SENTENCE_BODY: &str = r#"你是一位英语老师，帮助母语为{{nativeLanguage}}的学习者读懂英文句子。

用户选中的是一整句或一个较长的片段。**不要只挑其中一个单词来解释**，要先给出整句的意思，再点出真正影响理解的地方。

解释之前先检查这句英文本身有没有语法错误（时态、单复数、冠词、主谓一致、介词搭配、语序等）。有错就在 `grammar` 里指出错在哪、给出改正后的完整句子，后面的翻译和讲解仍然针对**用户给的原句**，不要换成改正后的句子。只是措辞不地道、但语法正确的，不算错误，`grammar` 留空。

只输出一个 JSON 对象，不要输出任何解释性文字，不要包裹代码块。JSON 结构如下：

{
  "grammar": {
    "issue": "用{{nativeLanguage}}说明句子的语法错误，一到两句；句子没有语法错误则给空字符串",
    "corrected": "改正后的完整英文句子；句子没有语法错误则给空字符串"
  },
  "translation": "整句的{{nativeLanguage}}翻译。通顺自然，不要逐词直译",
  "structure": "用{{nativeLanguage}}说明句子结构或语法要点，一到两句",
  "keyPoints": [
    { "term": "句中值得注意的词或短语，英文原文", "note": "用{{nativeLanguage}}解释它在此处的含义或用法" }
  ]
}

keyPoints 给 2 到 4 项，挑真正影响理解的难点，简单词不要罗列。
字段必须齐全。无法判断的字段给空字符串或空数组，不要省略键。grammar 的两个子字段要么都填，要么都留空，不要只填一个。"#;

// 旧版翻译正文。统一模板仍保留独立 translate 分支，且该分支不带 grammar：
// 原文本来就不是英文，纠英文语法无从谈起。
const TRANSLATE_BODY: &str = r#"你是一位英语老师，帮助母语为{{nativeLanguage}}的学习者把想说的话译成英文。

用户给的内容不是英文，这说明他想知道「这句话英语怎么说」。**不要解释这段话的意思，也不要逐字直译。**

`english` 给一个母语者真会说出口的版本：
- 按英语的表达习惯重组，该拆句就拆句、该换说法就换说法，不要迁就原文的语序和措辞习惯。
- 默认日常口语的语域；原文明显偏书面、正式或粗俗时，跟着原文的语气走。
- 只给一个版本，别的说法放 `alternatives`。

`wordChoice` 解释**为什么用这些词**，这是最重要的部分：挑 2 到 4 个真正体现选词功夫的地方（地道搭配、近义词取舍、原文没有直接对应词而改成意译的地方），说清为什么用它，而不是学习者最可能想到的那个直译词。人人都会的词不要罗列。

只输出一个 JSON 对象，不要输出任何解释性文字，不要包裹代码块。JSON 结构如下：

{
  "english": "最地道的英文说法",
  "wordChoice": [
    { "term": "译文里的词或短语，英文原文", "note": "用{{nativeLanguage}}说明为什么用它，一到两句；有对比就写明「不用 X 是因为……」" }
  ],
  "alternatives": [
    { "text": "另一种说法，完整的英文", "when": "用{{nativeLanguage}}说明它与上面那句的差别、什么场合用，一句话" }
  ]
}

alternatives 给 1 到 3 项，必须和 `english` 有实质差别（语域、语气、正式程度、句式），只换个同义词的不要给。
字段必须齐全。想不出的字段给空字符串或空数组，不要省略键。"#;

const CHAT_BODY: &str = r#"你是一位英语老师，正在和一位母语为{{nativeLanguage}}的学习者讨论他刚查询的内容。

规则：
- 用{{nativeLanguage}}讲解，英文例句保留英文原文。
- 简洁。默认三句话以内说清，用户要求展开时才展开。
- 举例优先于抽象定义。讲用法差异时给对比例句。
- 不确定的语料不要编造，直说不确定。
- 可以用 Markdown，但不要用大标题，最多用列表和加粗。"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_pass_their_own_checks() {
        // 内置模板自己都过不了检查的话，检查规则就是错的。
        for kind in TemplateKind::ALL {
            let issues = check(kind, &builtin(kind).body);
            assert!(issues.is_empty(), "{kind:?} 内置模板有问题: {issues:?}");
        }
    }

    #[test]
    fn builtins_only_use_known_variables() {
        for kind in TemplateKind::ALL {
            for name in variables_used(&builtin(kind).body) {
                assert!(VARIABLES.contains(&name.as_str()), "未知变量 {name}");
            }
        }
    }

    #[test]
    fn settings_only_exposes_unified_explain_and_chat_builtins() {
        let kinds: Vec<TemplateKind> = builtins().into_iter().map(|t| t.kind).collect();
        assert_eq!(kinds, vec![TemplateKind::Explain, TemplateKind::Chat]);
    }

    #[test]
    fn unified_template_delegates_classification_without_hardcoded_length_rules() {
        let body = builtin(TemplateKind::Explain).body;
        assert!(body.contains("不要按词数、字符数或是否有句末标点硬判"));
        assert!(body.contains("\"mode\": \"word\""));
        assert!(body.contains("\"mode\": \"sentence\""));
        assert!(body.contains("\"mode\": \"translate\""));
        assert!(body.contains("不要补另外两个分支的键"));
    }

    #[test]
    fn legacy_word_and_sentence_schemas_stay_separate() {
        // 合成一套就会回到「整句只解释一个词」的 bug。
        let sentence = builtin(TemplateKind::Sentence).body;
        assert!(!sentence.contains("\"word\""));
        assert!(!sentence.contains("\"phonetic\""));

        let word = builtin(TemplateKind::Word).body;
        assert!(!word.contains("\"translation\""));
    }

    #[test]
    fn legacy_translate_schema_stays_separate_too() {
        // 借单词 schema 的话，`phonetic`/`pos` 会逼模型给中文标音标。
        let translate = builtin(TemplateKind::Translate).body;
        assert!(!translate.contains("\"phonetic\""));
        assert!(!translate.contains("\"pos\""));
        // 原文不是英文，纠英文语法无从谈起。
        assert!(!translate.contains("\"grammar\""));
    }

    #[test]
    fn both_legacy_english_branches_check_grammar() {
        // 统一模板也延续该约束；这里锁住旧模板，保证用户保留的内容语义不变。
        for kind in [TemplateKind::Word, TemplateKind::Sentence] {
            assert!(
                builtin(kind).body.contains("\"grammar\""),
                "{kind:?} 模板没有 grammar 字段"
            );
        }
    }

    #[test]
    fn render_substitutes_known_variables() {
        let out = render(
            "用{{nativeLanguage}}解释。原句：{{context}}",
            "中文",
            Some("Hi."),
        );
        assert_eq!(out, "用中文解释。原句：Hi.");
    }

    #[test]
    fn missing_context_renders_empty_not_literal() {
        assert_eq!(render("原句：{{context}}", "中文", None), "原句：");
    }

    #[test]
    fn unknown_variables_survive_rendering_so_they_stay_visible() {
        // 静默替换成空串的话，用户拼错了只会觉得「模板好像没生效」。
        assert_eq!(render("{{languague}}", "中文", None), "{{languague}}");
    }

    #[test]
    fn finds_variables_without_duplicates() {
        assert_eq!(
            variables_used("{{a}} {{b}} {{a}}"),
            vec!["a".to_string(), "b".to_string()]
        );
        assert!(variables_used("没有变量").is_empty());
        // 没闭合的不算变量，也不该让扫描卡住。
        assert!(variables_used("{{unterminated").is_empty());
    }

    #[test]
    fn flags_unknown_variables() {
        let issues = check(TemplateKind::Chat, "用{{languague}}讲解");
        assert!(issues.iter().any(|i| i.message.contains("languague")));
    }

    #[test]
    fn flags_missing_required_fields() {
        let issues = check(TemplateKind::Sentence, "翻译一下，输出 `translation` 就行");
        let message = &issues[0].message;
        assert!(message.contains("structure"));
        assert!(message.contains("keyPoints"));
        assert!(!message.contains("translation"));
    }

    #[test]
    fn field_matching_requires_a_json_key_or_backticked_field_name() {
        assert!(!mentions_field("model decides", "mode"));
        assert!(!mentions_field("输出 mode 字段", "mode"));
        assert!(mentions_field("输出 `mode` 字段", "mode"));
        assert!(mentions_field(r#"{"mode":"word"}"#, "mode"));
        assert!(!mentions_field(r#"{"kind":"mode"}"#, "mode"));
        assert!(!mentions_field(r#"{"mode":"word"}"#, "word"));
    }

    #[test]
    fn unified_check_requires_both_grammar_subfields() {
        let body = builtin(TemplateKind::Explain)
            .body
            .replace("issue", "problem")
            .replace("corrected", "fixed");
        let issues = check(TemplateKind::Explain, &body);
        let message = issues
            .iter()
            .find(|issue| issue.level == "error")
            .map(|issue| issue.message.as_str())
            .unwrap_or_default();
        assert!(message.contains("issue"));
        assert!(message.contains("corrected"));
    }

    #[test]
    fn chat_has_no_field_contract() {
        assert!(check(TemplateKind::Chat, "随便讲讲").is_empty());
    }

    #[test]
    fn flags_empty_body() {
        assert_eq!(check(TemplateKind::Chat, "   ").len(), 1);
    }

    #[test]
    fn long_bodies_warn_but_do_not_error() {
        let body = format!("随便讲讲{}", "啊".repeat(LONG_BODY_CHARS));
        let issues = check(TemplateKind::Chat, &body);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].level, "warn");
    }
}
