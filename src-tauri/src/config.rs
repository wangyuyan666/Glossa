//! 配置读写。
//!
//! 配置文件为明文 JSON，含 API key。落盘位置 `~/Library/Application Support/EnAssistant/settings.json`，
//! 权限收紧到 0600（仅当前用户可读写）。这是用户明确选择的方案，替代 Keychain。

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

pub const APP_DIR_NAME: &str = "EnAssistant";
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
    let json = serde_json::to_string_pretty(settings)?;
    fs::write(&path, json).with_context(|| format!("写入配置失败: {}", path.display()))?;

    restrict_permissions(&path)?;
    Ok(())
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
