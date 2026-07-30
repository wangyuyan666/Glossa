//! PopClip 扩展的生成与安装。
//!
//! 安装走 package 形式：生成一个 `.popclipext` 目录，里面放 `Config.yaml`，
//! `open` 它就交给 PopClip 弹安装确认。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use serde::Serialize;
use tauri::AppHandle;

use crate::config;

pub const EXTENSION_DIR_NAME: &str = "Glossa.popclipext";

/// PopClip 可能被装在这几个位置。Setapp 版路径和常规版不同。
const POPCLIP_PATHS: [&str; 2] = [
    "/Applications/PopClip.app",
    "/Applications/Setapp/PopClip.app",
];

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
/// `identifier` 让 PopClip 认出这是同一个扩展：改端口后重新安装会覆盖原有的，
/// 而不是在 PopClip 条上多出一个重复图标。
pub fn config_yaml(port: u16) -> String {
    format!(
        r#"name: Glossa
identifier: com.github.glossa
icon: symbol:character.book.closed
interpreter: bash
shell script: curl -s -X POST http://127.0.0.1:{port}/lookup --data-urlencode "q=$POPCLIP_TEXT" -o /dev/null
"#
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
    fn config_yaml_carries_the_configured_port() {
        assert!(config_yaml(9001).contains("127.0.0.1:9001/lookup"));
    }

    #[test]
    fn config_yaml_has_an_identifier_so_reinstall_overwrites() {
        assert!(config_yaml(8765).contains("identifier: com.github.glossa"));
    }

    #[test]
    fn writes_config_into_the_extension_directory() {
        let base = std::env::temp_dir().join(format!("glossa-test-{}", std::process::id()));
        let dir = base.join(EXTENSION_DIR_NAME);
        write_extension(&dir, 8765).unwrap();

        let written = fs::read_to_string(dir.join("Config.yaml")).unwrap();
        assert_eq!(written, config_yaml(8765));

        fs::remove_dir_all(&base).ok();
    }
}
