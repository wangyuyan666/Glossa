//! 配置读写。
//!
//! 配置文件为明文 JSON，含 API key。落盘位置 `~/Library/Application Support/Glossa/settings.json`，
//! 权限收紧到 0600（仅当前用户可读写）。这是用户明确选择的方案，替代 Keychain。

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::templates::{self, PromptTemplate, TemplateKind};

pub const APP_DIR_NAME: &str = "Glossa";
pub const SETTINGS_FILE: &str = "settings.json";

/// 改名 Glossa 之前用的目录名。老用户的 API key 和历史都还落在这里，见 `migrate_legacy_dir`。
const LEGACY_APP_DIR_NAME: &str = "EnAssistant";
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
    /// 各类当前启用的模板 id。None 或指向已删除的模板都回落到内置。
    #[serde(default)]
    pub active_word: Option<String>,
    #[serde(default)]
    pub active_sentence: Option<String>,
    #[serde(default)]
    pub active_translate: Option<String>,
    #[serde(default)]
    pub active_chat: Option<String>,
}

fn default_port() -> u16 {
    DEFAULT_PORT
}

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
            active_word: None,
            active_sentence: None,
            active_translate: None,
            active_chat: None,
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
            TemplateKind::Word => self.active_word.as_ref(),
            TemplateKind::Sentence => self.active_sentence.as_ref(),
            TemplateKind::Translate => self.active_translate.as_ref(),
            TemplateKind::Chat => self.active_chat.as_ref(),
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

/// 把 `EnAssistant` 时代的数据目录搬到新名下。必须在读配置、开数据库之前调。
///
/// 不搬的后果不是「少了点历史」：settings.json 里有 API key，读不到就等于让老用户
/// 从头配一遍模型服务，还会误以为配置丢了。
pub fn migrate_legacy_dir(app: &AppHandle) {
    let Ok(new_dir) = config_dir(app) else { return };
    let Some(parent) = new_dir.parent() else { return };
    migrate_dir(&parent.join(LEGACY_APP_DIR_NAME), &new_dir);
}

/// 迁移的纯路径逻辑，抽出来是为了能用临时目录做单测。
///
/// 用 rename 而不是拷贝再删：同卷内是原子操作，中途失败不会留下半份数据，
/// 也不需要单独一步「删旧目录」——改名本身就让旧路径消失了。
fn migrate_dir(legacy: &Path, new_dir: &Path) {
    // 新目录已存在说明新版跑过并写过东西。此时搬过去要么覆盖、要么得合并，
    // 而删掉旧目录就是丢一份没合并的数据——宁可原地留着让用户自己处置。
    if new_dir.exists() || !legacy.is_dir() {
        return;
    }

    if let Err(e) = fs::rename(legacy, new_dir) {
        eprintln!("[config] 迁移旧数据目录失败，将按首次启动处理: {e}");
        return;
    }
    eprintln!(
        "[config] 已迁移旧数据目录 {} -> {}",
        legacy.display(),
        new_dir.display()
    );

    // 旧名的 PopClip 扩展暂存目录跟着搬过来了，但它带的是旧 identifier，
    // 留着只会让人以为那才是当前装着的扩展。装扩展时会按新名重新生成。
    fs::remove_dir_all(new_dir.join("EnAssistant.popclipext")).ok();
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
            templates: vec![user_template("mine", TemplateKind::Word)],
            active_word: Some("mine".into()),
            ..Settings::default()
        };
        assert_eq!(settings.template(TemplateKind::Word).body, "自定义正文");
    }

    #[test]
    fn falls_back_when_the_selected_template_was_deleted() {
        // 提示词是主流程的一环，指向空气时不能就没有提示词了。
        let settings = Settings {
            active_word: Some("已删除".into()),
            ..Settings::default()
        };
        assert_eq!(
            settings.template(TemplateKind::Word).id,
            templates::builtin_id(TemplateKind::Word)
        );
    }

    #[test]
    fn ignores_a_template_selected_for_the_wrong_kind() {
        // 把对话模板选进释义位置，会输出自由文本，卡片直接废掉。
        let settings = Settings {
            templates: vec![user_template("mine", TemplateKind::Chat)],
            active_word: Some("mine".into()),
            ..Settings::default()
        };
        assert_eq!(
            settings.template(TemplateKind::Word).id,
            templates::builtin_id(TemplateKind::Word)
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

    /// 造一个临时的 `Application Support` 父目录，返回 (父目录, 旧目录, 新目录)。
    fn migration_fixture(tag: &str) -> (PathBuf, PathBuf, PathBuf) {
        let parent = std::env::temp_dir().join(format!(
            "glossa-migrate-{}-{}-{tag}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&parent).unwrap();
        let legacy = parent.join(LEGACY_APP_DIR_NAME);
        let new_dir = parent.join(APP_DIR_NAME);
        (parent, legacy, new_dir)
    }

    #[test]
    fn migration_carries_the_old_settings_over() {
        let (parent, legacy, new_dir) = migration_fixture("carry");
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join(SETTINGS_FILE), r#"{"port":9001}"#).unwrap();

        migrate_dir(&legacy, &new_dir);

        assert!(!legacy.exists(), "改名后旧路径不该还在");
        let raw = fs::read_to_string(new_dir.join(SETTINGS_FILE)).unwrap();
        assert!(raw.contains("9001"), "API key 和端口必须跟着搬过来");

        fs::remove_dir_all(&parent).ok();
    }

    #[test]
    fn migration_drops_the_stale_popclip_staging_dir() {
        // 搬过来的扩展带旧 identifier，留着会让人以为那是当前装着的那个。
        let (parent, legacy, new_dir) = migration_fixture("popclip");
        fs::create_dir_all(legacy.join("EnAssistant.popclipext")).unwrap();

        migrate_dir(&legacy, &new_dir);

        assert!(!new_dir.join("EnAssistant.popclipext").exists());
        fs::remove_dir_all(&parent).ok();
    }

    #[test]
    fn migration_leaves_the_old_dir_alone_when_the_new_one_exists() {
        // 两边都有数据时搬过去等于覆盖，删旧目录等于丢一份没合并的数据。
        let (parent, legacy, new_dir) = migration_fixture("both");
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join(SETTINGS_FILE), "旧").unwrap();
        fs::create_dir_all(&new_dir).unwrap();
        fs::write(new_dir.join(SETTINGS_FILE), "新").unwrap();

        migrate_dir(&legacy, &new_dir);

        assert_eq!(fs::read_to_string(legacy.join(SETTINGS_FILE)).unwrap(), "旧");
        assert_eq!(fs::read_to_string(new_dir.join(SETTINGS_FILE)).unwrap(), "新");

        fs::remove_dir_all(&parent).ok();
    }

    #[test]
    fn migration_is_a_no_op_for_a_fresh_install() {
        let (parent, legacy, new_dir) = migration_fixture("fresh");

        migrate_dir(&legacy, &new_dir);

        assert!(!new_dir.exists(), "没有旧目录就不该凭空造出新目录");
        fs::remove_dir_all(&parent).ok();
    }
}
