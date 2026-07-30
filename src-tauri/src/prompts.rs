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

/// 选中的内容不是英文——那多半是「这句话英语怎么说」，不是「这是什么意思」。
///
/// 判据：出现任意一个**非拉丁字母**的字母字符。码位 > U+024F（Latin Extended-B 之后）
/// 的字母覆盖中日韩、假名、谚文、西里尔、希腊、阿拉伯等；`café` / `naïve` 这类带变音
/// 符号的仍算英文，不会被误判。
///
/// 阈值就是「一个也算」：中英混排里只要冒出中文，用户想要的基本都是英文说法。
pub fn looks_foreign(text: &str) -> bool {
    text.chars().any(|c| c.is_alphabetic() && c > '\u{24F}')
}

pub fn kind_for(text: &str) -> TemplateKind {
    // 先判语言：中文没有空格，`is_sentence` 数出来永远是 1 个词，
    // 不先拦一道的话整段中文都会落进单词模板。
    if looks_foreign(text) {
        TemplateKind::Translate
    } else if is_sentence(text) {
        TemplateKind::Sentence
    } else {
        TemplateKind::Word
    }
}

/// 首轮用户消息。选中的内容走这里，不进系统提示词。
pub fn explain_user(text: &str, context: Option<&str>) -> String {
    if looks_foreign(text) {
        return format!("要译成英文的内容：{text}");
    }
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
        TemplateKind::Translate => "这事儿我来扛。",
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
        assert!(explain_user("这事儿我来扛。", None).starts_with("要译成英文的内容："));
    }

    #[test]
    fn english_is_not_foreign() {
        assert!(!looks_foreign("take on"));
        assert!(!looks_foreign("She agreed to take on the project."));
        // 变音符号仍是拉丁字母，别把这类词判成外语。
        assert!(!looks_foreign("café"));
        assert!(!looks_foreign("naïve résumé"));
        // 标点、数字、emoji 都不是字母，不该单独触发。
        assert!(!looks_foreign("50% off — really?! 🎉"));
    }

    #[test]
    fn non_latin_scripts_are_foreign() {
        assert!(looks_foreign("这事儿我来扛"));
        assert!(looks_foreign("お疲れ様"));
        assert!(looks_foreign("привет"));
        // 中英混排：只要冒出一个中文字就当翻译需求。
        assert!(looks_foreign("帮我 take on 这个项目"));
    }

    #[test]
    fn foreign_text_wins_over_sentence_detection() {
        // 中文没有空格，`is_sentence` 数出来是 1 个词——不先判语言的话，
        // 整段中文会落到单词模板，被逼着标音标。
        let zh = "这个方案我觉得还得再想想，不然到时候不好收场。";
        assert!(!is_sentence(zh));
        assert_eq!(kind_for(zh), TemplateKind::Translate);
    }

    #[test]
    fn probe_inputs_match_their_kind() {
        // 样例必须真被判成对应的类，否则实测走的模板和用户选的对不上。
        for kind in [
            TemplateKind::Word,
            TemplateKind::Sentence,
            TemplateKind::Translate,
        ] {
            assert_eq!(kind_for(probe_input(kind)), kind);
        }
    }
}
