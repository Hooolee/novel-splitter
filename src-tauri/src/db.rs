use rusqlite::{params, Connection, Result};
use std::path::{Path, PathBuf};

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
pub fn get_conn() -> Result<Connection> {
    // 假设 db 文件在工作目录 (与原逻辑一致)
    let path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join("novel_intelligence.db");
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
