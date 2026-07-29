//! 系统提示词。
//!
//! 释义走结构化 JSON，但**不使用** `response_format: json_schema` 或 tool use——
//! 多数 OpenAI 兼容端点（Ollama、LM Studio、部分中转）不支持或行为不一致。
//! 改用提示词约束 + 前端容错增量解析，是跨协议唯一稳的做法。
//!
//! 单词和句子用两套提示词与两套 JSON 结构。只有一套的话，句子会被单词的 schema
//! 逼着填 `word`（词条原形）/`phonetic`/`pos`，模型只能从句子里挑一个词来填——
//! 表现就是「选了一整句，却只翻译其中一个单词」。

/// 选中的是句子还是词/短语。
///
/// 判定放在 Rust 侧而不是交给模型：确定性、可单测，也不用多花一次调用。
pub fn is_sentence(text: &str) -> bool {
    let trimmed = text.trim();
    let words = trimmed.split_whitespace().count();

    // 六个词以上基本不会是要查的词组了。
    // 三词以上且有句末标点也算——"I get it." 这种短句同样不该走单词模式。
    words >= 6 || (words >= 3 && trimmed.ends_with(['.', '!', '?']))
}

/// 释义提示词。按选中内容是词还是句子分流。
///
/// `context` 为选中文本所在的上下文；PopClip 取词拿不到，传 None 时单词模式
/// 降级为「最常见义项」，这正是自建取词层要补上的能力（见 AGENTS.md 阶段二 TODO）。
pub fn explain_system(native_language: &str, context: Option<&str>, sentence: bool) -> String {
    if sentence {
        sentence_system(native_language)
    } else {
        word_system(native_language, context)
    }
}

fn word_system(native_language: &str, context: Option<&str>) -> String {
    let context_rule = match context {
        Some(_) => {
            "用户会给出选中的词以及它所在的原句。你必须解释**这个词在该句中的具体含义**，\
             而不是罗列全部义项。`why` 字段说明为什么在此处是这个意思。"
        }
        None => {
            "本次没有上下文，只有孤立的词。给出最常见的义项，\
             `why` 字段简述该词的核心语感或最典型的使用场景。"
        }
    };

    format!(
        r#"你是一位英语老师，帮助母语为{native_language}的学习者理解英文词汇。

{context_rule}

只输出一个 JSON 对象，不要输出任何解释性文字，不要包裹代码块。JSON 结构如下：

{{
  "word": "词条原形",
  "phonetic": "IPA 音标，不确定则留空字符串",
  "pos": "词性缩写，如 v. / n. / adj. / phr.",
  "senseHere": "用{native_language}给出此处含义，一句话，不超过 40 字",
  "why": "用{native_language}说明理由或语感，一到两句",
  "collocations": ["常见搭配，2 到 4 个，英文原文"],
  "example": {{ "en": "一个地道英文例句", "zh": "该例句的{native_language}翻译" }}
}}

字段必须齐全。无法判断的字段给空字符串或空数组，不要省略键。"#
    )
}

/// 句子模式不需要额外上下文——句子本身就是它自己的上下文。
fn sentence_system(native_language: &str) -> String {
    format!(
        r#"你是一位英语老师，帮助母语为{native_language}的学习者读懂英文句子。

用户选中的是一整句或一个较长的片段。**不要只挑其中一个单词来解释**，
要先给出整句的意思，再点出真正影响理解的地方。

只输出一个 JSON 对象，不要输出任何解释性文字，不要包裹代码块。JSON 结构如下：

{{
  "translation": "整句的{native_language}翻译。通顺自然，不要逐词直译",
  "structure": "用{native_language}说明句子结构或语法要点，一到两句",
  "keyPoints": [
    {{ "term": "句中值得注意的词或短语，英文原文", "note": "用{native_language}解释它在此处的含义或用法" }}
  ]
}}

keyPoints 给 2 到 4 项，挑真正影响理解的难点，简单词不要罗列。
字段必须齐全。无法判断的字段给空字符串或空数组，不要省略键。"#
    )
}

/// 追问对话提示词。会话里已经带了本次查询的内容与释义，这里只定人设与输出风格。
pub fn chat_system(native_language: &str) -> String {
    format!(
        r#"你是一位英语老师，正在和一位母语为{native_language}的学习者讨论他刚查询的内容。

规则：
- 用{native_language}讲解，英文例句保留英文原文。
- 简洁。默认三句话以内说清，用户要求展开时才展开。
- 举例优先于抽象定义。讲用法差异时给对比例句。
- 不确定的语料不要编造，直说不确定。
- 可以用 Markdown，但不要用大标题，最多用列表和加粗。"#
    )
}

/// 首轮用户消息。
pub fn explain_user(text: &str, context: Option<&str>) -> String {
    if is_sentence(text) {
        return format!("选中的句子：{text}");
    }
    match context {
        Some(ctx) => format!("选中的词：{text}\n\n所在原句：{ctx}"),
        None => format!("选中的词：{text}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_words_and_phrases_are_not_sentences() {
        assert!(!is_sentence("resilient"));
        assert!(!is_sentence("take on"));
        assert!(!is_sentence("on the other hand"));
        assert!(!is_sentence("take on a challenge"));
    }

    #[test]
    fn long_selections_are_sentences() {
        assert!(is_sentence(
            "She agreed to take on the project despite the tight deadline."
        ));
        // 没有句末标点也算，选中时经常漏掉句号。
        assert!(is_sentence("She had to take on more responsibility"));
    }

    #[test]
    fn short_selections_with_terminal_punctuation_are_sentences() {
        assert!(is_sentence("I get it."));
        assert!(is_sentence("What do you mean?"));
        // 两个词太短，标点也不足以判成句子。
        assert!(!is_sentence("Really?"));
    }

    #[test]
    fn ignores_surrounding_whitespace() {
        assert!(!is_sentence("  take on  "));
        assert!(is_sentence("  I get it.  "));
    }

    #[test]
    fn user_message_labels_sentences_differently() {
        assert!(explain_user("resilient", None).starts_with("选中的词："));
        assert!(explain_user("I get it.", None).starts_with("选中的句子："));
    }

    #[test]
    fn sentence_mode_does_not_ask_for_word_fields() {
        // 句子 schema 里出现 word/phonetic/pos 就会把模型带回「挑一个词解释」的老路。
        let prompt = explain_system("中文", None, true);
        assert!(!prompt.contains("\"word\""));
        assert!(!prompt.contains("\"phonetic\""));
        assert!(!prompt.contains("\"pos\""));
        assert!(prompt.contains("\"translation\""));
    }

    #[test]
    fn word_mode_keeps_the_word_fields() {
        let prompt = explain_system("中文", None, false);
        assert!(prompt.contains("\"word\""));
        assert!(!prompt.contains("\"translation\""));
    }
}
