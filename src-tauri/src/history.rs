//! 查询历史。
//!
//! SQLite 而不是 JSON：路线图上的生词本 + FSRS 复习需要按到期时间查、按熟练度排序，
//! 那必须有真正的存储层。现在用 JSON、之后再迁移等于白做一遍。
//!
//! 两个入口（弹窗划词、主窗口输入）都落库，否则历史只记一半，日常主力入口反而没记录。
//!
//! 这是**历史流水**，不是词表：同一个词查两次记两条。去重是生词本的事。

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::config;

pub const DB_FILE: &str = "history.db";

/// 侧栏列表用的摘要，不含对话内容。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LookupSummary {
    pub id: String,
    pub text: String,
    /// 释义里的 senseHere，解析不出来就是空串。侧栏拿它当副标题。
    pub sense: String,
    pub source: String,
    pub created_at: i64,
}

/// 点开一条历史时加载的完整会话。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LookupDetail {
    pub id: String,
    pub text: String,
    pub context: Option<String>,
    /// 释义的原始 JSON 字符串，前端仍用 parsePartialJson 解析，和流式路径共用一套渲染。
    pub explanation: String,
    pub source: String,
    pub created_at: i64,
    pub turns: Vec<Turn>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Turn {
    pub role: String,
    pub content: String,
}

/// 连接常驻。SQLite 单连接串行访问，用 Mutex 包住即可，历史读写频率远达不到需要连接池的程度。
pub struct History(Mutex<Connection>);

impl History {
    pub fn open(app: &AppHandle) -> Result<Self> {
        let dir = config::config_dir(app)?;
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("创建数据目录失败: {}", dir.display()))?;

        let conn = Connection::open(dir.join(DB_FILE)).context("打开 history.db 失败")?;
        migrate(&conn)?;
        Ok(Self(Mutex::new(conn)))
    }

    fn conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.0
            .lock()
            .map_err(|_| anyhow::anyhow!("history 连接锁已中毒"))
    }
}

fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS lookups (
            id          TEXT PRIMARY KEY,
            text        TEXT NOT NULL,
            context     TEXT,
            explanation TEXT NOT NULL,
            source      TEXT NOT NULL,
            created_at  INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS turns (
            id        INTEGER PRIMARY KEY AUTOINCREMENT,
            lookup_id TEXT NOT NULL REFERENCES lookups(id) ON DELETE CASCADE,
            seq       INTEGER NOT NULL,
            role      TEXT NOT NULL,
            content   TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_lookups_created ON lookups(created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_turns_lookup ON turns(lookup_id, seq);
        "#,
    )
    .context("建表失败")?;
    Ok(())
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

/// 从释义 JSON 里取 senseHere 当侧栏副标题。
///
/// 模型可能给 JSON 裹上代码块、也可能整个输出不合法——取不到就返回空串，
/// 不该因为副标题解析失败就让整条历史存不进去。
fn extract_sense(explanation: &str) -> String {
    let trimmed = explanation
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    serde_json::from_str::<serde_json::Value>(trimmed)
        .ok()
        .and_then(|v| v["senseHere"].as_str().map(str::to_string))
        .unwrap_or_default()
}

pub fn save_lookup(
    history: &History,
    id: &str,
    text: &str,
    context: Option<&str>,
    explanation: &str,
    source: &str,
) -> Result<()> {
    history.conn()?.execute(
        "INSERT OR REPLACE INTO lookups (id, text, context, explanation, source, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, text, context, explanation, source, now_ms()],
    )?;
    Ok(())
}

pub fn append_turn(history: &History, lookup_id: &str, role: &str, content: &str) -> Result<()> {
    let conn = history.conn()?;
    let seq: i64 = conn.query_row(
        "SELECT COALESCE(MAX(seq), -1) + 1 FROM turns WHERE lookup_id = ?1",
        params![lookup_id],
        |row| row.get(0),
    )?;
    conn.execute(
        "INSERT INTO turns (lookup_id, seq, role, content) VALUES (?1, ?2, ?3, ?4)",
        params![lookup_id, seq, role, content],
    )?;
    Ok(())
}

pub fn list(
    history: &History,
    limit: u32,
    offset: u32,
    query: Option<&str>,
) -> Result<Vec<LookupSummary>> {
    let conn = history.conn()?;
    // LIKE 的通配符在参数里拼，避免把用户输入直接塞进 SQL 文本。
    // 空白输入不当作过滤条件，否则清空搜索框后列表会是空的。
    let pattern = query
        .map(str::trim)
        .filter(|q| !q.is_empty())
        .map(|q| format!("%{q}%"));

    let mut stmt = conn.prepare(
        "SELECT id, text, explanation, source, created_at
           FROM lookups
          WHERE (?1 IS NULL OR text LIKE ?1)
          ORDER BY created_at DESC
          LIMIT ?2 OFFSET ?3",
    )?;

    let rows = stmt.query_map(params![pattern, limit, offset], |row| {
        let explanation: String = row.get(2)?;
        Ok(LookupSummary {
            id: row.get(0)?,
            text: row.get(1)?,
            sense: extract_sense(&explanation),
            source: row.get(3)?,
            created_at: row.get(4)?,
        })
    })?;

    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn get(history: &History, id: &str) -> Result<Option<LookupDetail>> {
    let conn = history.conn()?;

    let mut stmt = conn.prepare(
        "SELECT id, text, context, explanation, source, created_at FROM lookups WHERE id = ?1",
    )?;
    let mut rows = stmt.query(params![id])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };

    let detail = LookupDetail {
        id: row.get(0)?,
        text: row.get(1)?,
        context: row.get(2)?,
        explanation: row.get(3)?,
        source: row.get(4)?,
        created_at: row.get(5)?,
        turns: Vec::new(),
    };
    drop(rows);

    let mut stmt =
        conn.prepare("SELECT role, content FROM turns WHERE lookup_id = ?1 ORDER BY seq")?;
    let turns = stmt
        .query_map(params![id], |row| {
            Ok(Turn {
                role: row.get(0)?,
                content: row.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(Some(LookupDetail { turns, ..detail }))
}

pub fn delete(history: &History, id: &str) -> Result<()> {
    // turns 靠 ON DELETE CASCADE 一并清掉，前提是 PRAGMA foreign_keys 已开（见 migrate）。
    history
        .conn()?
        .execute("DELETE FROM lookups WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn clear(history: &History) -> Result<()> {
    history.conn()?.execute_batch("DELETE FROM lookups;")?;
    Ok(())
}

pub fn state(app: &AppHandle) -> tauri::State<'_, History> {
    app.state::<History>()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_history() -> History {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        History(Mutex::new(conn))
    }

    fn seed(history: &History, id: &str, text: &str, sense: &str) {
        let explanation = format!(r#"{{"word":"{text}","senseHere":"{sense}"}}"#);
        save_lookup(history, id, text, None, &explanation, "main").unwrap();
    }

    #[test]
    fn lists_newest_first() {
        let h = memory_history();
        seed(&h, "a", "take on", "承担");
        seed(&h, "b", "resilient", "有韧性的");

        let items = list(&h, 10, 0, None).unwrap();
        assert_eq!(items.len(), 2);
        // created_at 同毫秒时靠插入顺序无法保证，只断言两条都在。
        assert!(items.iter().any(|i| i.text == "take on"));
        assert!(items.iter().any(|i| i.text == "resilient"));
    }

    #[test]
    fn pulls_sense_out_of_the_explanation_json() {
        let h = memory_history();
        seed(&h, "a", "take on", "承担、接下");
        assert_eq!(list(&h, 10, 0, None).unwrap()[0].sense, "承担、接下");
    }

    #[test]
    fn survives_an_unparsable_explanation() {
        let h = memory_history();
        // 模型输出被截断的情况，副标题取不到，但历史本身必须存得进去。
        save_lookup(&h, "a", "take on", None, "{\"word\":\"take", "popup").unwrap();
        let items = list(&h, 10, 0, None).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].sense, "");
    }

    #[test]
    fn strips_code_fence_before_reading_sense() {
        let h = memory_history();
        save_lookup(
            &h,
            "a",
            "take on",
            None,
            "```json\n{\"senseHere\":\"承担\"}\n```",
            "main",
        )
        .unwrap();
        assert_eq!(list(&h, 10, 0, None).unwrap()[0].sense, "承担");
    }

    #[test]
    fn filters_by_substring() {
        let h = memory_history();
        seed(&h, "a", "take on", "承担");
        seed(&h, "b", "resilient", "有韧性的");

        assert_eq!(list(&h, 10, 0, Some("resil")).unwrap().len(), 1);
        // 敲第一个字母就该筛，这是搜索框的正常预期。
        assert_eq!(list(&h, 10, 0, Some("r")).unwrap().len(), 1);
        // 中间匹配也算，不只是前缀。
        assert_eq!(list(&h, 10, 0, Some("ke o")).unwrap().len(), 1);
    }

    #[test]
    fn blank_query_does_not_filter() {
        let h = memory_history();
        seed(&h, "a", "take on", "承担");
        seed(&h, "b", "resilient", "有韧性的");

        // 清空搜索框后必须回到完整列表，不能变成空。
        assert_eq!(list(&h, 10, 0, Some("  ")).unwrap().len(), 2);
        assert_eq!(list(&h, 10, 0, Some("")).unwrap().len(), 2);
        assert_eq!(list(&h, 10, 0, None).unwrap().len(), 2);
    }

    #[test]
    fn appends_turns_in_order() {
        let h = memory_history();
        seed(&h, "a", "take on", "承担");
        append_turn(&h, "a", "user", "还有别的用法吗").unwrap();
        append_turn(&h, "a", "assistant", "有的").unwrap();

        let turns = get(&h, "a").unwrap().unwrap().turns;
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].role, "user");
        assert_eq!(turns[1].content, "有的");
    }

    #[test]
    fn deleting_a_lookup_takes_its_turns_with_it() {
        let h = memory_history();
        seed(&h, "a", "take on", "承担");
        append_turn(&h, "a", "user", "问题").unwrap();

        delete(&h, "a").unwrap();
        assert!(get(&h, "a").unwrap().is_none());

        let orphans: i64 = h
            .conn()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM turns", [], |r| r.get(0))
            .unwrap();
        assert_eq!(orphans, 0, "turns 应随 lookup 级联删除");
    }

    #[test]
    fn get_returns_none_for_unknown_id() {
        assert!(get(&memory_history(), "nope").unwrap().is_none());
    }

    #[test]
    fn clear_empties_everything() {
        let h = memory_history();
        seed(&h, "a", "take on", "承担");
        append_turn(&h, "a", "user", "问题").unwrap();

        clear(&h).unwrap();
        assert!(list(&h, 10, 0, None).unwrap().is_empty());
    }
}
