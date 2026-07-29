//! 提示词的组装：判定用哪类模板、拼首轮用户消息。
//!
//! 模板正文本身（含内置模板）在 [`crate::templates`]。

use crate::templates::TemplateKind;

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

pub fn kind_for(text: &str) -> TemplateKind {
    if is_sentence(text) {
        TemplateKind::Sentence
    } else {
        TemplateKind::Word
    }
}

/// 首轮用户消息。选中的内容走这里，不进系统提示词。
pub fn explain_user(text: &str, context: Option<&str>) -> String {
    if is_sentence(text) {
        return format!("选中的句子：{text}");
    }
    match context {
        Some(ctx) => format!("选中的词：{text}\n\n所在原句：{ctx}"),
        None => format!("选中的词：{text}"),
    }
}

/// 「实测一次」用的样例，各类模板一个。固定样例才能横向比较不同模板的表现。
pub fn probe_input(kind: TemplateKind) -> &'static str {
    match kind {
        TemplateKind::Word => "take on",
        TemplateKind::Sentence => "She agreed to take on the project despite the tight deadline.",
        TemplateKind::Chat => "这个词还有别的用法吗？",
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
    fn probe_inputs_match_their_kind() {
        // 句子样例必须真被判成句子，否则实测走的模板和用户选的对不上。
        assert!(is_sentence(probe_input(TemplateKind::Sentence)));
        assert!(!is_sentence(probe_input(TemplateKind::Word)));
    }
}
