//! 配置读写。
//!
//! 配置文件为明文 JSON，含 API key。落盘位置 `~/Library/Application Support/Glossa/settings.json`，
//! 权限收紧到 0600（仅当前用户可读写）。这是用户明确选择的方案，替代 Keychain。

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::templates::{self, PromptTemplate, TemplateKind};

pub const APP_DIR_NAME: &str = "Glossa";
pub const SETTINGS_FILE: &str = "settings.json";
pub const DEFAULT_PORT: u16 = 8765;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Openai,
    Anthropic,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub protocol: Protocol,
    /// 例如 `https://api.openai.com/v1` 或 `https://api.anthropic.com`。
    /// 结尾带不带 `/v1` 都能接受，见 `llm::endpoint`。
    pub base_url: String,
    pub api_key: String,
    /// 该端点单次输出的 token 上限，None 表示用内置默认值。
    ///
    /// 这是**端点能力**而不是全局偏好，所以挂在 provider 上：支持 64K 输出的 reasoner
    /// 和上限 4096 的中转，需要的值正相反。填太大的后果是端点直接返回 HTTP 400。
    ///
    /// 注意它**含推理模型的思考 token**，给小了会出现「思考写满、正文零字符」。
    #[serde(default)]
    pub max_tokens: Option<u32>,
}

/// 低于这个值的上限一律当没填。
///
/// 输入框清空、手滑填个 0，不该把额度掐到发不出一次完整释义。
const MIN_MAX_TOKENS: u32 = 256;

impl Provider {
    /// 该 provider 的输出上限，没配就用传入的默认值。
    pub fn max_tokens_or(&self, default: u32) -> u32 {
        self.max_tokens
            .filter(|v| *v >= MIN_MAX_TOKENS)
            .unwrap_or(default)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RoleBinding {
    pub provider_id: String,
    pub model: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default)]
    pub providers: Vec<Provider>,
    /// 释义角色：求快求便宜。
    #[serde(default)]
    pub fast: Option<RoleBinding>,
    /// 对话角色：求强。
    #[serde(default)]
    pub chat: Option<RoleBinding>,
    #[serde(default = "default_port")]
    pub port: u16,
    /// 释义用的母语，决定 LLM 用哪种语言解释。
    #[serde(default = "default_native_language")]
    pub native_language: String,

    /// 用户自建的提示词模板。内置模板不在这里——它们留在代码中，见 `templates.rs`。
    #[serde(default)]
    pub templates: Vec<PromptTemplate>,
    /// 统一释义当前启用的模板 id。旧配置没有该字段时回落新版内置模板。
    #[serde(default)]
    pub active_explain: Option<String>,
    /// 旧版选择记录只为无损读取和保存已有 settings.json，不再参与真实释义。
    #[serde(default)]
    pub active_word: Option<String>,
    #[serde(default)]
    pub active_sentence: Option<String>,
    #[serde(default)]
    pub active_translate: Option<String>,
    #[serde(default)]
    pub active_chat: Option<String>,

    /// 朗读用的系统嗓子名（`speechSynthesis` 里的 `voice.name`）。None 表示自动挑。
    ///
    /// 存名字而不是索引：嗓子列表随系统的安装 / 卸载变动，索引会错位到别人身上。
    /// 名字出了本机就没意义，换台机器没装这个嗓子时前端自动回落，不必迁移。
    #[serde(default)]
    pub voice: Option<String>,
    /// 朗读语速，1.0 是正常速度。学英语常用 0.8 上下，所以做成可调而不是写死。
    #[serde(default = "default_speech_rate")]
    pub speech_rate: f32,
}

fn default_port() -> u16 {
    DEFAULT_PORT
}

fn default_speech_rate() -> f32 {
    1.0
}

/// 语速的可用区间。再慢会一个词一个词地拖，再快就听不清了，都不是「能用」的范围。
pub const MIN_SPEECH_RATE: f32 = 0.5;
pub const MAX_SPEECH_RATE: f32 = 1.5;

fn default_native_language() -> String {
    "中文".to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            providers: Vec::new(),
            fast: None,
            chat: None,
            port: default_port(),
            native_language: default_native_language(),
            templates: Vec::new(),
            active_explain: None,
            active_word: None,
            active_sentence: None,
            active_translate: None,
            active_chat: None,
            voice: None,
            speech_rate: default_speech_rate(),
        }
    }
}

impl Settings {
    pub fn provider(&self, id: &str) -> Option<&Provider> {
        self.providers.iter().find(|p| p.id == id)
    }

    /// 解析角色绑定为 (provider, model)。绑定缺失或指向已删除的 provider 时返回 None。
    pub fn resolve(&self, role: Role) -> Option<(&Provider, &str)> {
        let binding = match role {
            Role::Fast => self.fast.as_ref(),
            Role::Chat => self.chat.as_ref(),
        }?;
        let provider = self.provider(&binding.provider_id)?;
        Some((provider, binding.model.as_str()))
    }

    fn active_id(&self, kind: TemplateKind) -> Option<&String> {
        match kind {
            TemplateKind::Explain => self.active_explain.as_ref(),
            TemplateKind::Chat => self.active_chat.as_ref(),
            TemplateKind::Word => self.active_word.as_ref(),
            TemplateKind::Sentence => self.active_sentence.as_ref(),
            TemplateKind::Translate => self.active_translate.as_ref(),
        }
    }

    /// 该类当前启用的模板。选中的模板被删掉、或从没选过，都回落到内置——
    /// 提示词是主流程的一环，任何情况下都不能没有。
    pub fn template(&self, kind: TemplateKind) -> PromptTemplate {
        self.active_id(kind)
            .and_then(|id| {
                self.templates
                    .iter()
                    .find(|t| &t.id == id && t.kind == kind)
                    .cloned()
            })
            .unwrap_or_else(|| templates::builtin(kind))
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Role {
    Fast,
    Chat,
}

pub fn config_dir(app: &AppHandle) -> Result<PathBuf> {
    let home = app.path().home_dir().context("无法定位用户主目录")?;
    Ok(home
        .join("Library")
        .join("Application Support")
        .join(APP_DIR_NAME))
}

pub fn settings_path(app: &AppHandle) -> Result<PathBuf> {
    Ok(config_dir(app)?.join(SETTINGS_FILE))
}

/// 读配置。文件不存在或损坏时返回默认值，不报错——首次启动就是这条路径。
pub fn load(app: &AppHandle) -> Settings {
    let Ok(path) = settings_path(app) else {
        return Settings::default();
    };
    let Ok(raw) = fs::read_to_string(&path) else {
        return Settings::default();
    };
    serde_json::from_str(&raw).unwrap_or_else(|e| {
        eprintln!("[config] settings.json 解析失败，回退默认值: {e}");
        Settings::default()
    })
}

pub fn save(app: &AppHandle, settings: &Settings) -> Result<()> {
    let dir = config_dir(app)?;
    fs::create_dir_all(&dir).with_context(|| format!("创建配置目录失败: {}", dir.display()))?;

    let path = dir.join(SETTINGS_FILE);
    let json = serde_json::to_string_pretty(&sanitize(settings.clone()))?;
    fs::write(&path, json).with_context(|| format!("写入配置失败: {}", path.display()))?;

    restrict_permissions(&path)?;
    Ok(())
}

/// 落盘前清理：
///
/// - 内置模板永远不写进配置。写了的话，以后改内置提示词，用户还在跑旧副本且毫不知情。
/// - `builtin` 标记强制置 false，防止伪造出一个「不可删」的用户模板。
/// - 过小的 `maxTokens` 归一成 None（回落默认值），不把它原样存下来。
fn sanitize(mut settings: Settings) -> Settings {
    for provider in &mut settings.providers {
        provider.max_tokens = provider.max_tokens.filter(|v| *v >= MIN_MAX_TOKENS);
    }

    // 手改过的 settings.json 里 0 或 10 都可能出现，前者一声不吭、后者听不清，
    // 两种都会被当成「发音坏了」。滑块本来就限了范围，这里兜住绕过滑块的路径。
    settings.speech_rate = if settings.speech_rate.is_finite() {
        settings.speech_rate.clamp(MIN_SPEECH_RATE, MAX_SPEECH_RATE)
    } else {
        default_speech_rate()
    };
    // 空串挑不到任何嗓子，等同于没选，别让它变成一个选不掉的坏值。
    settings.voice = settings.voice.filter(|name| !name.trim().is_empty());

    let builtin_ids: Vec<&str> = TemplateKind::ALL
        .iter()
        .map(|k| templates::builtin_id(*k))
        .collect();

    settings
        .templates
        .retain(|t| !builtin_ids.contains(&t.id.as_str()));
    for template in &mut settings.templates {
        template.builtin = false;
    }
    settings
}

/// 明文 key 落盘，权限收紧到 0600，避免同机其他用户读取。
#[cfg(unix)]
fn restrict_permissions(path: &PathBuf) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &PathBuf) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user_template(id: &str, kind: TemplateKind) -> PromptTemplate {
        PromptTemplate {
            id: id.into(),
            name: "我的".into(),
            kind,
            body: "自定义正文".into(),
            builtin: false,
        }
    }

    #[test]
    fn falls_back_to_builtin_when_nothing_is_selected() {
        let settings = Settings::default();
        for kind in TemplateKind::ALL {
            assert_eq!(settings.template(kind).id, templates::builtin_id(kind));
        }
    }

    #[test]
    fn uses_the_selected_user_template() {
        let settings = Settings {
            templates: vec![user_template("mine", TemplateKind::Explain)],
            active_explain: Some("mine".into()),
            ..Settings::default()
        };
        assert_eq!(settings.template(TemplateKind::Explain).body, "自定义正文");
    }

    #[test]
    fn falls_back_when_the_selected_template_was_deleted() {
        // 提示词是主流程的一环，指向空气时不能就没有提示词了。
        let settings = Settings {
            active_explain: Some("已删除".into()),
            ..Settings::default()
        };
        assert_eq!(
            settings.template(TemplateKind::Explain).id,
            templates::builtin_id(TemplateKind::Explain)
        );
    }

    #[test]
    fn ignores_a_template_selected_for_the_wrong_kind() {
        // 把对话模板选进释义位置，会输出自由文本，卡片直接废掉。
        let settings = Settings {
            templates: vec![user_template("mine", TemplateKind::Chat)],
            active_explain: Some("mine".into()),
            ..Settings::default()
        };
        assert_eq!(
            settings.template(TemplateKind::Explain).id,
            templates::builtin_id(TemplateKind::Explain)
        );
    }

    fn provider(max_tokens: Option<u32>) -> Provider {
        Provider {
            id: "p".into(),
            name: "测试".into(),
            protocol: Protocol::Openai,
            base_url: "https://example.com".into(),
            api_key: "k".into(),
            max_tokens,
        }
    }

    #[test]
    fn old_configs_without_max_tokens_still_load() {
        // 加字段不能让已有用户的 settings.json 解析失败——那会静默回落到空配置，
        // 表现是「升级后 provider 和 key 全没了」。
        let raw = r#"{"providers":[{"id":"p","name":"DeepSeek","protocol":"openai",
            "baseUrl":"https://api.deepseek.com","apiKey":"k"}]}"#;
        let settings: Settings = serde_json::from_str(raw).unwrap();
        assert_eq!(settings.providers[0].max_tokens, None);
        assert_eq!(settings.providers[0].max_tokens_or(4000), 4000);
    }

    #[test]
    fn old_prompt_config_loads_without_losing_provider_or_templates() {
        // 旧 kind 若反序列化失败，load 会整份回退默认值，连 provider 和 API key 都一起消失。
        let raw = r#"{
          "providers": [{"id":"p","name":"DeepSeek","protocol":"openai","baseUrl":"https://api.deepseek.com","apiKey":"secret"}],
          "templates": [
            {"id":"w","name":"我的单词","kind":"word","body":"word body","builtin":false},
            {"id":"s","name":"我的句子","kind":"sentence","body":"sentence body","builtin":false},
            {"id":"t","name":"我的翻译","kind":"translate","body":"translate body","builtin":false}
          ],
          "activeWord":"w",
          "activeSentence":"s",
          "activeTranslate":"t"
        }"#;
        let settings: Settings = serde_json::from_str(raw).unwrap();
        assert_eq!(settings.providers[0].api_key, "secret");
        assert_eq!(settings.templates.len(), 3);
        assert_eq!(settings.active_word.as_deref(), Some("w"));
        assert_eq!(settings.active_explain, None);
        assert_eq!(
            settings.template(TemplateKind::Explain).id,
            templates::BUILTIN_EXPLAIN_ID
        );
    }

    #[test]
    fn max_tokens_falls_back_to_the_default_when_unset() {
        assert_eq!(provider(None).max_tokens_or(4000), 4000);
        assert_eq!(provider(Some(16000)).max_tokens_or(4000), 16000);
    }

    #[test]
    fn absurdly_small_limits_fall_back_instead_of_starving_the_stream() {
        // 输入框清空会送来 0；照用的话思考还没写完就截断，正文永远是空的。
        assert_eq!(provider(Some(0)).max_tokens_or(4000), 4000);
        assert_eq!(provider(Some(64)).max_tokens_or(4000), 4000);
    }

    #[test]
    fn sanitize_normalizes_small_limits_to_unset() {
        let cleaned = sanitize(Settings {
            providers: vec![provider(Some(0)), provider(Some(8000))],
            ..Settings::default()
        });
        assert_eq!(cleaned.providers[0].max_tokens, None);
        assert_eq!(cleaned.providers[1].max_tokens, Some(8000));
    }

    #[test]
    fn sanitize_keeps_speech_rate_audible() {
        let clamp = |rate: f32| {
            sanitize(Settings {
                speech_rate: rate,
                ..Settings::default()
            })
            .speech_rate
        };
        assert_eq!(clamp(0.0), MIN_SPEECH_RATE);
        assert_eq!(clamp(10.0), MAX_SPEECH_RATE);
        assert_eq!(clamp(f32::NAN), 1.0);
        assert_eq!(clamp(0.85), 0.85);
    }

    #[test]
    fn sanitize_treats_blank_voice_as_unset() {
        let cleaned = sanitize(Settings {
            voice: Some("  ".to_string()),
            ..Settings::default()
        });
        assert_eq!(cleaned.voice, None);
    }

    #[test]
    fn sanitize_drops_builtin_copies_and_forged_flags() {
        let settings = Settings {
            templates: vec![
                templates::builtin(TemplateKind::Word),
                PromptTemplate {
                    builtin: true,
                    ..user_template("mine", TemplateKind::Word)
                },
            ],
            ..Settings::default()
        };

        let cleaned = sanitize(settings);
        assert_eq!(cleaned.templates.len(), 1);
        assert_eq!(cleaned.templates[0].id, "mine");
        assert!(!cleaned.templates[0].builtin);
    }
}
