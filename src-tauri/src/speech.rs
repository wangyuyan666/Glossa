//! 发音：调 macOS 的 `say`。
//!
//! **为什么不用 web 的 `speechSynthesis`**：WebKit 只认老式注册的那套嗓子，用户
//! 在「系统设置 → 辅助功能 → 朗读内容 → 管理声音」下载的 Premium / Enhanced
//! 嗓子它一条都看不到（实测：`say -v '?'` 里有 `Ava (Premium)`，webview 的
//! `getVoices()` 里没有）。而 Premium 跟预装的 compact 版听感差一整代，正是
//! 用户会抱怨「难听」的那一档。WebKit 还会把名字里的括号剥掉，`Eddy (English (US))`
//! 变成 `Eddy`，同名重复，前端连唯一标识都拿不到。
//!
//! `say` 看得到全部嗓子，且不引入任何新依赖。代价是每次朗读多一次进程启动
//! （约 100ms），以及只能在 macOS 上用——这个 app 本来就只有 macOS。

use std::process::Stdio;

use serde::Serialize;
use tauri::AppHandle;
use tokio::io::AsyncWriteExt;
use tokio::process::{Child, Command};

use crate::state;

/// `say -r` 的单位是词/分钟，不是倍率。这是它不带 `-r` 时的默认语速。
const BASE_WORDS_PER_MINUTE: f32 = 175.0;

/// 和 `config.rs` 的 MIN/MAX_SPEECH_RATE 对齐。
const MIN_RATE: f32 = 0.5;
const MAX_RATE: f32 = 1.5;

/// 轮询子进程是否念完的间隔。一次朗读以秒计，这个粒度足够，也不占 CPU。
const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(80);

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Voice {
    /// `say -v` 认的名字，含 `(Premium)` 这类后缀。前端也拿它当唯一标识。
    pub name: String,
    /// BCP-47。`say` 报的是 `en_US`，这里统一成 `en-US`。
    pub lang: String,
}

/// 正在念的那条。`generation` 用来分辨「自己这条」和「后来把自己换掉的那条」。
pub struct Speaking {
    pub generation: u64,
    pub child: Child,
}

/// 解析 `say -v '?'` 的一行。
///
/// 格式是「名字 空白 语言 空白 # 示例句」，但**名字里可以有空格和括号**
/// （`Eddy (English (US))`、`Bad News`），列宽也不固定，所以不能按列切。
/// 可靠的锚点只有两个：`#` 之前的最后一个空白分隔词是语言，其余是名字。
fn parse_voice_line(line: &str) -> Option<Voice> {
    let head = line.split('#').next()?.trim_end();
    let (name, lang) = head.rsplit_once(char::is_whitespace)?;
    let name = name.trim();
    // 语言形如 `en_US` / `zh_CN`。没有下划线说明这是名字的一部分，不是语言列。
    if name.is_empty() || !lang.contains('_') {
        return None;
    }
    Some(Voice {
        name: name.to_string(),
        lang: lang.replace('_', "-"),
    })
}

/// 系统里装了的全部嗓子。
///
/// 取不到时返回空表而不是报错：前端会退回「自动」，仍然出得了声——`say` 不带
/// `-v` 用的就是系统默认嗓。为「列表拿不到」把整个发音判死不划算。
#[tauri::command]
pub async fn list_voices() -> Vec<Voice> {
    let Ok(output) = Command::new("say").arg("-v").arg("?").output().await else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(parse_voice_line)
        .collect()
}

fn words_per_minute(rate: f32) -> u32 {
    let rate = if rate.is_finite() {
        rate.clamp(MIN_RATE, MAX_RATE)
    } else {
        1.0
    };
    (BASE_WORDS_PER_MINUTE * rate).round() as u32
}

/// 停掉正在念的那条。没有在念也不算错。
#[tauri::command]
pub fn stop_speaking(app: AppHandle) {
    state::stop_speaking(&app);
}

/// 念一段文本，念完（或被打断）才返回。
///
/// 前端靠这个 Promise 何时 resolve 来决定喇叭图标何时熄灭，所以必须一直等到
/// 进程退出，不能 spawn 完就返回。
#[tauri::command]
pub async fn speak(
    app: AppHandle,
    text: String,
    voice: Option<String>,
    rate: f32,
) -> Result<(), String> {
    let mut command = Command::new("say");
    command.arg("-r").arg(words_per_minute(rate).to_string());
    if let Some(name) = voice.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        command.arg("-v").arg(name);
    }
    // **文本走 stdin 而不是参数**：不经过 shell，没有注入面，不受 argv 长度限制，
    // 而且以 `-` 开头的选中内容不会被 say 当成选项。
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let mut child = command.spawn().map_err(|e| e.to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(text.as_bytes()).await;
        // stdin 不关掉的话 say 会一直等输入，永远不退出。
        drop(stdin);
    }

    // 登记的同时掐掉上一条——同时响两个嗓子比不出声更糟。
    let generation = state::start_speaking(&app, child);

    // 等它念完。tokio 的 Child 存在 Mutex 里没法直接 await，只能轮询；每轮都要
    // 确认 state 里的还是不是自己那条，被换走就立刻收工，不然会一直等到别人念完。
    loop {
        tokio::time::sleep(POLL_INTERVAL).await;
        match state::poll_speech(&app, generation) {
            Poll::Running => {}
            Poll::Done => return Ok(()),
            // 静默失败是最坏的：用户点了喇叭没声音，看不出是嗓音没了还是 app 坏了。
            Poll::Failed => {
                return Err("say 没能念出来，选的嗓音可能已被卸载".to_string())
            }
        }
    }
}

/// app 退出前掐掉还在念的进程。
///
/// `say` 是独立进程，不管它的话窗口都关了声音还在响，用户只能干等它念完。
pub fn shutdown(app: &AppHandle) {
    state::stop_speaking(app);
}

pub enum Poll {
    Running,
    Done,
    /// `say` 非零退出。最常见的原因是选的嗓音已经被卸载了。
    Failed,
}

/// 供 `state` 查某一代朗读的进展。放在这里是因为 `Speaking` 的语义属于本模块。
///
/// 被 `stop_speaking` 停掉的那条已经从 slot 里摘走了，走的是 `None` 分支，
/// 所以**用户主动停不会被当成失败**——只有自然退出且非零才算。
pub fn poll(slot: &mut Option<Speaking>, generation: u64) -> Poll {
    match slot.as_mut() {
        // 已经被停掉、或被后来的一条换走了，自己这条到此为止。
        None => Poll::Done,
        Some(speaking) if speaking.generation != generation => Poll::Done,
        Some(speaking) => match speaking.child.try_wait() {
            Ok(Some(status)) => {
                *slot = None;
                if status.success() {
                    Poll::Done
                } else {
                    Poll::Failed
                }
            }
            // 查不动了（进程已被别处回收），当结束处理，别把任务挂死。
            Err(_) => {
                *slot = None;
                Poll::Done
            }
            Ok(None) => Poll::Running,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_names_that_contain_spaces_and_parens() {
        assert_eq!(
            parse_voice_line("Ava (Premium)       en_US    # Hello! My name is Ava."),
            Some(Voice {
                name: "Ava (Premium)".into(),
                lang: "en-US".into(),
            })
        );
        assert_eq!(
            parse_voice_line("Eddy (English (US)) en_US    # Hello! My name is Eddy."),
            Some(Voice {
                name: "Eddy (English (US))".into(),
                lang: "en-US".into(),
            })
        );
        assert_eq!(
            parse_voice_line("Bad News            en_US    # Hello! My name is Bad News."),
            Some(Voice {
                name: "Bad News".into(),
                lang: "en-US".into(),
            })
        );
    }

    #[test]
    fn skips_lines_that_are_not_voices() {
        assert_eq!(parse_voice_line(""), None);
        assert_eq!(parse_voice_line("# just a comment"), None);
        // 末词没有下划线，说明这行没有语言列。
        assert_eq!(parse_voice_line("Samantha"), None);
    }

    #[test]
    fn maps_the_rate_multiplier_onto_words_per_minute() {
        assert_eq!(words_per_minute(1.0), 175);
        assert_eq!(words_per_minute(0.8), 140);
        // 越界和 NaN 都得落在能听清的范围里，别让手改的配置把语速拉到听不清。
        assert_eq!(words_per_minute(0.0), 88);
        assert_eq!(words_per_minute(99.0), 263);
        assert_eq!(words_per_minute(f32::NAN), 175);
    }
}
