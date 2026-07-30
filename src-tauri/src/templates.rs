//! 提示词模板。
//!
//! 三类模板各自独立选用：用户想只改释义风格、保留默认对话，不该被迫连对话一起写。
//!
//! **内置模板留在代码里，不写进 `settings.json`**，配置只记「当前选了哪个 id」。
//! 内置提示词以后还会改（比如又发现一类输出 bug），把副本存进用户配置的话，
//! 升级后用户还在跑旧提示词，而且完全看不出来。

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TemplateKind {
    /// 单词 / 短语释义。输出 JSON，字段契约见 [`required_fields`]。
    Word,
    /// 整句释义。输出 JSON。
    Sentence,
    /// 追问对话。自由文本，无字段契约。
    Chat,
}

impl TemplateKind {
    pub const ALL: [TemplateKind; 3] = [Self::Word, Self::Sentence, Self::Chat];
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

pub const BUILTIN_WORD_ID: &str = "builtin-word";
pub const BUILTIN_SENTENCE_ID: &str = "builtin-sentence";
pub const BUILTIN_CHAT_ID: &str = "builtin-chat";

/// 模板正文里可用的占位符。静态检查据此判定「未知变量」。
pub const VARIABLES: [&str; 2] = ["nativeLanguage", "context"];

/// 正文超过这个长度就提醒——附加约束太多会稀释 JSON 格式要求，模型开始不听话。
const LONG_BODY_CHARS: usize = 2000;

pub fn builtin_id(kind: TemplateKind) -> &'static str {
    match kind {
        TemplateKind::Word => BUILTIN_WORD_ID,
        TemplateKind::Sentence => BUILTIN_SENTENCE_ID,
        TemplateKind::Chat => BUILTIN_CHAT_ID,
    }
}

pub fn builtin(kind: TemplateKind) -> PromptTemplate {
    let (name, body) = match kind {
        TemplateKind::Word => ("内置 · 单词释义", WORD_BODY),
        TemplateKind::Sentence => ("内置 · 句子释义", SENTENCE_BODY),
        TemplateKind::Chat => ("内置 · 追问对话", CHAT_BODY),
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
    TemplateKind::ALL.iter().copied().map(builtin).collect()
}

/// 释义 JSON 必须带的字段。静态检查在正文里搜这些名字，实测则检查模型真的输出了它们。
pub fn required_fields(kind: TemplateKind) -> &'static [&'static str] {
    match kind {
        TemplateKind::Word => &[
            "word",
            "phonetic",
            "pos",
            "senseHere",
            "why",
            "collocations",
            "example",
        ],
        TemplateKind::Sentence => &["translation", "structure", "keyPoints"],
        // 对话输出自由文本，没有契约。
        TemplateKind::Chat => &[],
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

    let missing: Vec<&str> = required_fields(kind)
        .iter()
        .copied()
        .filter(|field| !body.contains(field))
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
// 单词和句子是两套字段，别合成一套：句子走单词 schema 时，`word`（词条原形）/
// `phonetic`/`pos` 会逼着模型从句子里挑一个词来填，症状就是「选了一整句却只翻译
// 其中一个单词」。这是实际踩过的 bug。

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
    fn word_and_sentence_schemas_stay_separate() {
        // 合成一套就会回到「整句只解释一个词」的 bug。
        let sentence = builtin(TemplateKind::Sentence).body;
        assert!(!sentence.contains("\"word\""));
        assert!(!sentence.contains("\"phonetic\""));

        let word = builtin(TemplateKind::Word).body;
        assert!(!word.contains("\"translation\""));
    }

    #[test]
    fn both_explain_templates_check_grammar() {
        // 只加在句子模板上不够：`how's is goging today` 这种短错句被判成词组，
        // 走的是单词模板，纠错就丢了。
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
        let issues = check(TemplateKind::Sentence, "翻译一下，输出 translation 就行");
        let message = &issues[0].message;
        assert!(message.contains("structure"));
        assert!(message.contains("keyPoints"));
        assert!(!message.contains("translation"));
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
