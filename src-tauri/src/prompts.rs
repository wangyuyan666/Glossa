//! 提示词请求消息与固定实测样例。
//!
//! 真实释义不再在 Rust 侧预判单词、句子或翻译；统一模板在同一次 LLM 调用里自行判断。

use crate::templates::TemplateKind;

/// 首轮用户消息。选中的内容不进系统提示词，避免用户文本被误当成模板指令。
pub fn explain_user(text: &str, context: Option<&str>) -> String {
    match context.filter(|ctx| !ctx.trim().is_empty()) {
        Some(ctx) => format!("选中的内容：{text}\n\n所在原句：{ctx}"),
        None => format!("选中的内容：{text}"),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ProbeCase {
    pub label: &'static str,
    pub input: &'static str,
    pub expected_mode: Option<&'static str>,
}

/// 固定样例才能横向比较不同模板。统一释义必须三类都跑，否则测不出自主分流。
pub fn probe_cases(kind: TemplateKind) -> Vec<ProbeCase> {
    match kind {
        TemplateKind::Explain => vec![
            ProbeCase {
                label: "单词 / 短语",
                input: "take on",
                expected_mode: Some("word"),
            },
            ProbeCase {
                label: "英文句子",
                input: "Himan, what's your name",
                expected_mode: Some("sentence"),
            },
            ProbeCase {
                label: "译成英文",
                input: "这事儿我来扛。",
                expected_mode: Some("translate"),
            },
        ],
        TemplateKind::Chat => vec![ProbeCase {
            label: "追问对话",
            input: "这个表达还有更口语的说法吗？",
            expected_mode: None,
        }],
        // 旧版模板已停用，但保留单例实测，方便用户查看迁移前自定义内容是否仍有效。
        TemplateKind::Word => vec![ProbeCase {
            label: "旧版单词释义",
            input: "take on",
            expected_mode: None,
        }],
        TemplateKind::Sentence => vec![ProbeCase {
            label: "旧版句子释义",
            input: "She agreed to take on the project despite the tight deadline.",
            expected_mode: None,
        }],
        TemplateKind::Translate => vec![ProbeCase {
            label: "旧版译成英文",
            input: "这事儿我来扛。",
            expected_mode: None,
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explain_user_never_preclassifies_the_selection() {
        for text in ["resilient", "What's your name", "这事儿我来扛。"] {
            assert_eq!(explain_user(text, None), format!("选中的内容：{text}"));
        }
    }

    #[test]
    fn explain_user_includes_context_when_available() {
        assert_eq!(
            explain_user("take on", Some("She had to take on more responsibility.")),
            "选中的内容：take on\n\n所在原句：She had to take on more responsibility."
        );
    }

    #[test]
    fn empty_context_is_ignored() {
        assert_eq!(explain_user("take on", Some("  ")), "选中的内容：take on");
    }

    #[test]
    fn unified_probe_covers_all_modes() {
        let cases = probe_cases(TemplateKind::Explain);
        assert_eq!(cases.len(), 3);
        assert_eq!(
            cases
                .iter()
                .filter_map(|case| case.expected_mode)
                .collect::<Vec<_>>(),
            vec!["word", "sentence", "translate"]
        );
    }
}
