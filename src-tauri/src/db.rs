use rusqlite::{params, Connection, Result};
use std::path::Path;

pub fn init_db<P: AsRef<Path>>(db_path: P) -> Result<Connection> {
    let conn = Connection::open(db_path)?;

    // 1. Novels 表
    conn.execute(
        "CREATE TABLE IF NOT EXISTS novels (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            book_id TEXT NOT NULL,
            platform TEXT NOT NULL,
            title TEXT NOT NULL,
            author TEXT,
            tags TEXT,
            word_count INTEGER,
            ai_reviews_json TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(book_id, platform)
        )",
        [],
    )?;

    // 2. Chapters 表
    conn.execute(
        "CREATE TABLE IF NOT EXISTS chapters (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            novel_id INTEGER NOT NULL,
            chapter_index INTEGER NOT NULL,
            title TEXT NOT NULL,
            content TEXT,
            outline_json TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY(novel_id) REFERENCES novels(id) ON DELETE CASCADE,
            UNIQUE(novel_id, chapter_index)
        )",
        [],
    )?;

    // 3. Scan_Reports 表
    conn.execute(
        "CREATE TABLE IF NOT EXISTS scan_reports (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            scan_date DATETIME DEFAULT CURRENT_TIMESTAMP,
            rank_type TEXT NOT NULL,
            macro_insights_json TEXT
        )",
        [],
    )?;

    // 4. Rank_History 表
    conn.execute(
        "CREATE TABLE IF NOT EXISTS rank_history (
            report_id INTEGER NOT NULL,
            novel_id INTEGER NOT NULL,
            rank INTEGER NOT NULL,
            rank_change TEXT,
            PRIMARY KEY(report_id, novel_id),
            FOREIGN KEY(report_id) REFERENCES scan_reports(id) ON DELETE CASCADE,
            FOREIGN KEY(novel_id) REFERENCES novels(id) ON DELETE CASCADE
        )",
        [],
    )?;

    println!("Database initialized successfully with 4 core tables.");

    Ok(conn)
}

// 获取项目根目录下的数据库连接
// 与 lib.rs::get_project_root() 保持一致，避免 Tauri dev 模式下路径不一致
pub fn get_conn() -> Result<Connection> {
    let root = crate::get_project_root();
    let path = root.join("novel_intelligence.db");
    Connection::open(path)
}

/// 插入或更新一本书，返回它的内部自增 ID
pub fn upsert_novel(
    conn: &Connection,
    book_id: &str,
    platform: &str,
    title: &str,
    author: &str,
    tags: &str,
    word_count: i64,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO novels (book_id, platform, title, author, tags, word_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(book_id, platform) DO UPDATE SET 
            title=excluded.title, 
            author=excluded.author, 
            tags=excluded.tags, 
            word_count=excluded.word_count,
            updated_at=CURRENT_TIMESTAMP",
        params![book_id, platform, title, author, tags, word_count],
    )?;

    // 查询它的 ID (因为 ON CONFLICT 可能没产生 last_insert_rowid)
    let id: i64 = conn.query_row(
        "SELECT id FROM novels WHERE book_id = ?1 AND platform = ?2",
        params![book_id, platform],
        |row| row.get(0),
    )?;
    Ok(id)
}

/// 创建一条宏观扫描报告，返回 report_id
pub fn create_scan_report(conn: &Connection, rank_type: &str) -> Result<i64> {
    conn.execute(
        "INSERT INTO scan_reports (rank_type) VALUES (?1)",
        params![rank_type],
    )?;
    Ok(conn.last_insert_rowid())
}

/// 插入快照记录
pub fn insert_rank_history(
    conn: &Connection,
    report_id: i64,
    novel_id: i64,
    rank: i64,
    rank_change: &str,
) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO rank_history (report_id, novel_id, rank, rank_change)
         VALUES (?1, ?2, ?3, ?4)",
        params![report_id, novel_id, rank, rank_change],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_db_creates_tables() {
        let tmp = std::env::temp_dir().join("test_novel_intelligence.db");
        // 清理旧文件
        let _ = std::fs::remove_file(&tmp);

        let conn = init_db(&tmp).expect("init_db should succeed");
        assert!(conn.is_autocommit());

        // 检查四张表都存在
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert!(tables.contains(&"chapters".to_string()));
        assert!(tables.contains(&"novels".to_string()));
        assert!(tables.contains(&"rank_history".to_string()));
        assert!(tables.contains(&"scan_reports".to_string()));

        // 测试基本 CRUD
        let nid = upsert_novel(&conn, "test_book_001", "qidian", "测试小说", "测试作者", "玄幻,系统", 0)
            .expect("upsert_novel should succeed");
        assert!(nid > 0);

        let rid = create_scan_report(&conn, "test_rank_url").expect("create_scan_report should succeed");
        assert!(rid > 0);

        insert_rank_history(&conn, rid, nid, 1, "+1").expect("insert_rank_history should succeed");

        upsert_chapter(&conn, nid, 1, "第一章", "内容略", None).expect("upsert_chapter should succeed");

        // 清理
        let _ = std::fs::remove_file(&tmp);
        println!("✅ DB 单元测试全部通过");
    }
}

/// 插入单章及其细纲
pub fn upsert_chapter(
    conn: &Connection,
    novel_id: i64,
    chapter_index: i64,
    title: &str,
    content: &str,
    outline_json: Option<&str>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO chapters (novel_id, chapter_index, title, content, outline_json)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(novel_id, chapter_index) DO UPDATE SET
            title=excluded.title,
            content=excluded.content,
            outline_json=COALESCE(excluded.outline_json, chapters.outline_json)",
        params![novel_id, chapter_index, title, content, outline_json],
    )?;
    Ok(())
}

/// 加载多 Agent 评估所需的小说上下文：title / tags / 拼接好的 outline_blob / 实际有 outline 的章节数。
/// outline_blob 形如：
/// ```text
/// ## 第1章 章节标题
/// [...outline_json 原文...]
///
/// ## 第2章 ...
/// ```
pub fn load_novel_for_review(
    conn: &Connection,
    novel_id: i64,
) -> Result<(String, String, String, usize)> {
    let (title, tags): (String, String) = conn.query_row(
        "SELECT title, COALESCE(tags, '') FROM novels WHERE id = ?1",
        params![novel_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    let mut stmt = conn.prepare(
        "SELECT chapter_index, title, outline_json FROM chapters
         WHERE novel_id = ?1 AND outline_json IS NOT NULL AND outline_json != ''
         ORDER BY chapter_index ASC",
    )?;
    let rows = stmt.query_map(params![novel_id], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;

    let mut blob = String::new();
    let mut chapter_count = 0usize;
    for r in rows {
        if let Ok((idx, ch_title, outline)) = r {
            blob.push_str(&format!("\n## 第{}章 {}\n{}\n", idx, ch_title, outline));
            chapter_count += 1;
        }
    }

    Ok((title, tags, blob, chapter_count))
}

/// 写入 multi-agent 评估结果到 novels.ai_reviews_json 并刷新 updated_at
pub fn update_ai_reviews(
    conn: &Connection,
    novel_id: i64,
    reviews_json: &str,
) -> Result<()> {
    conn.execute(
        "UPDATE novels SET ai_reviews_json = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
        params![reviews_json, novel_id],
    )?;
    Ok(())
}
