//! PopClip 扩展的生成与安装。
//!
//! PopClip 有两种安装形式：
//!   - **snippet** —— 一段以 `#popclip` 开头的 YAML，用户**选中**它，PopClip 条上出现 Install Extension
//!   - **package** —— 一个 `.popclipext` 目录，里面放 `Config.yaml`，`open` 它就交给 PopClip 弹安装确认
//!
//! 一键安装走 package：我们生成目录再 `open`。snippet 那条路作为兜底保留在设置页里，
//! 因为一键安装依赖文件关联，Setapp 版、多版本共存、关联被别的软件抢走都可能失效。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use serde::Serialize;
use tauri::AppHandle;

use crate::config;

pub const EXTENSION_DIR_NAME: &str = "EnAssistant.popclipext";

/// PopClip 可能被装在这几个位置。Setapp 版路径和常规版不同。
const POPCLIP_PATHS: [&str; 2] = ["/Applications/PopClip.app", "/Applications/Setapp/PopClip.app"];

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InstallOutcome {
    /// 生成的 `.popclipext` 目录路径。
    pub path: String,
    /// 是否已经把它交给 PopClip 打开。探测不到 PopClip 时为 false。
    pub opened: bool,
}

/// 找已安装的 PopClip。找不到就别 `open`——macOS 会弹「没有可打开此文件的应用」，
/// 对用户来说是个不知所云的错误，不如直接说没装。
pub fn find_popclip() -> Option<PathBuf> {
    POPCLIP_PATHS
        .iter()
        .map(PathBuf::from)
        .chain(
            std::env::var_os("HOME")
                .map(|home| PathBuf::from(home).join("Applications/PopClip.app")),
        )
        .find(|path| path.exists())
}

/// 扩展的 `Config.yaml`。
///
/// package 形式**不需要** `#popclip` 头行，那是 snippet 专用的识别标志。
///
/// `identifier` 让 PopClip 认出这是同一个扩展：改端口后重新安装会覆盖原有的，
/// 而不是在 PopClip 条上多出一个重复图标。
pub fn config_yaml(port: u16) -> String {
    format!(
        r#"name: EnAssistant
identifier: com.peter.enassistant
icon: symbol:character.book.closed
interpreter: bash
shell script: curl -s -X POST http://127.0.0.1:{port}/lookup --data-urlencode "q=$POPCLIP_TEXT" -o /dev/null
"#
    )
}

/// snippet 形式，给设置页的「手动安装」用。必须以 `#popclip` 开头。
pub fn snippet(port: u16) -> String {
    format!(
        r#"#popclip
name: EnAssistant
identifier: com.peter.enassistant
icon: symbol:character.book.closed
interpreter: bash
shell script: curl -s -X POST http://127.0.0.1:{port}/lookup --data-urlencode "q=$POPCLIP_TEXT" -o /dev/null"#
    )
}

/// 生成扩展目录并交给 PopClip 打开。
pub fn install(app: &AppHandle) -> Result<InstallOutcome> {
    let settings = config::load(app);
    let dir = config::config_dir(app)?.join(EXTENSION_DIR_NAME);

    write_extension(&dir, settings.port)?;

    let opened = match find_popclip() {
        Some(_) => {
            open_with_finder(&dir)?;
            true
        }
        None => false,
    };

    Ok(InstallOutcome {
        path: dir.display().to_string(),
        opened,
    })
}

fn write_extension(dir: &Path, port: u16) -> Result<()> {
    fs::create_dir_all(dir).with_context(|| format!("创建扩展目录失败: {}", dir.display()))?;
    let config_path = dir.join("Config.yaml");
    fs::write(&config_path, config_yaml(port))
        .with_context(|| format!("写入 Config.yaml 失败: {}", config_path.display()))?;
    Ok(())
}

fn open_with_finder(dir: &Path) -> Result<()> {
    let status = Command::new("/usr/bin/open")
        .arg(dir)
        .status()
        .context("调用 open 失败")?;
    if !status.success() {
        anyhow::bail!("open 返回非零状态: {status}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_yaml_has_no_snippet_header() {
        // package 里出现 `#popclip` 不会报错，但说明写法搞混了——两种形式的要求正相反。
        assert!(!config_yaml(8765).contains("#popclip"));
    }

    #[test]
    fn snippet_starts_with_the_marker_popclip_looks_for() {
        assert!(snippet(8765).starts_with("#popclip\n"));
    }

    #[test]
    fn both_forms_carry_the_configured_port() {
        assert!(config_yaml(9001).contains("127.0.0.1:9001/lookup"));
        assert!(snippet(9001).contains("127.0.0.1:9001/lookup"));
    }

    #[test]
    fn both_forms_share_an_identifier_so_reinstall_overwrites() {
        assert!(config_yaml(8765).contains("identifier: com.peter.enassistant"));
        assert!(snippet(8765).contains("identifier: com.peter.enassistant"));
    }

    #[test]
    fn writes_config_into_the_extension_directory() {
        let base = std::env::temp_dir().join(format!("enassistant-test-{}", std::process::id()));
        let dir = base.join(EXTENSION_DIR_NAME);
        write_extension(&dir, 8765).unwrap();

        let written = fs::read_to_string(dir.join("Config.yaml")).unwrap();
        assert_eq!(written, config_yaml(8765));

        fs::remove_dir_all(&base).ok();
    }
}
