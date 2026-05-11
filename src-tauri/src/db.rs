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

// =========================================================================
//  Library Tab 卡片查询（任务四a）
// =========================================================================

/// 排序枚举（与前端 SortBy union type 对齐，serde rename snake_case）
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum NovelSortBy {
    UpdatedDesc,
    LatestRankAsc,
    ScanCountDesc,
}

impl Default for NovelSortBy {
    fn default() -> Self { NovelSortBy::UpdatedDesc }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Default)]
pub struct NovelListFilter {
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub consensus: Vec<String>,
    pub platform: Option<String>,
    #[serde(default)]
    pub sort_by: NovelSortBy,
}

#[derive(serde::Serialize, Debug, Clone)]
pub struct NovelListRow {
    pub id: i64,
    pub book_id: String,
    pub platform: String,
    pub title: String,
    pub author: Option<String>,
    pub tags: Vec<String>,
    pub word_count: Option<i64>,
    pub created_at: String,
    pub updated_at: String,

    /// 解析后的 ai_reviews_json（含 agents / consensus / meta），失败或为空则 null。
    pub ai_reviews: Option<serde_json::Value>,
    /// 最近一次上榜排名（最新 scan_report 的 rank_history 行），未上榜则 null。
    pub latest_rank: Option<i64>,
    /// 累计上榜次数 = rank_history 中按该 novel 的行数。
    pub scan_count: i64,
}

/// 查询书库卡片列表。
///
/// 过滤语义：
///   - `tags`：OR（任一 tag 命中即收）
///   - `consensus`：OR（基于 ai_reviews_json 字符串 LIKE 匹配）
///   - `platform`：精确等于
///
/// SQL 注入防护：tag / consensus / platform 使用参数化绑定，不直接拼字符串。
pub fn list_novels(conn: &Connection, filter: &NovelListFilter) -> Result<Vec<NovelListRow>> {
    let mut where_clauses: Vec<String> = Vec::new();
    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(p) = &filter.platform {
        where_clauses.push("n.platform = ?".to_string());
        params_vec.push(Box::new(p.clone()));
    }

    if !filter.tags.is_empty() {
        let ors: Vec<String> = filter.tags.iter().map(|_| "n.tags LIKE ?".to_string()).collect();
        where_clauses.push(format!("({})", ors.join(" OR ")));
        for t in &filter.tags {
            params_vec.push(Box::new(format!("%{}%", t)));
        }
    }

    if !filter.consensus.is_empty() {
        let ors: Vec<String> = filter
            .consensus
            .iter()
            .map(|_| "n.ai_reviews_json LIKE ?".to_string())
            .collect();
        where_clauses.push(format!("({})", ors.join(" OR ")));
        for c in &filter.consensus {
            params_vec.push(Box::new(format!("%\"consensus\":\"{}\"%", c)));
        }
    }

    let where_sql = if where_clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", where_clauses.join(" AND "))
    };

    let order_sql = match filter.sort_by {
        NovelSortBy::UpdatedDesc => "ORDER BY n.updated_at DESC",
        NovelSortBy::LatestRankAsc => "ORDER BY latest_rank ASC NULLS LAST, n.updated_at DESC",
        NovelSortBy::ScanCountDesc => "ORDER BY scan_count DESC, n.updated_at DESC",
    };

    let sql = format!(
        "SELECT n.id, n.book_id, n.platform, n.title, n.author, n.tags, n.word_count,
                n.ai_reviews_json, n.created_at, n.updated_at,
                (SELECT rh.rank FROM rank_history rh
                 JOIN scan_reports sr ON sr.id = rh.report_id
                 WHERE rh.novel_id = n.id
                 ORDER BY sr.scan_date DESC LIMIT 1) AS latest_rank,
                (SELECT COUNT(*) FROM rank_history rh WHERE rh.novel_id = n.id) AS scan_count
         FROM novels n
         {} {}",
        where_sql, order_sql,
    );

    let mut stmt = conn.prepare(&sql)?;
    let params_ref: Vec<&dyn rusqlite::ToSql> =
        params_vec.iter().map(|b| b.as_ref()).collect();

    let rows = stmt.query_map(params_ref.as_slice(), |row| {
        let tags_csv: Option<String> = row.get(5)?;
        let tags: Vec<String> = tags_csv
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let reviews_raw: Option<String> = row.get(7)?;
        let ai_reviews = reviews_raw
            .as_deref()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok());

        Ok(NovelListRow {
            id: row.get(0)?,
            book_id: row.get(1)?,
            platform: row.get(2)?,
            title: row.get(3)?,
            author: row.get(4)?,
            tags,
            word_count: row.get(6)?,
            created_at: row.get(8)?,
            updated_at: row.get(9)?,
            ai_reviews,
            latest_rank: row.get(10)?,
            scan_count: row.get(11)?,
        })
    })?;

    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

#[cfg(test)]
mod list_novels_tests {
    use super::*;

    /// 构造一个独立临时 DB 并填充 3 本测试 novel：
    ///   - 玄幻 + all_yes
    ///   - 都市 + divergent
    ///   - 玄幻+系统（无 ai_reviews）
    fn seed() -> (std::path::PathBuf, Connection) {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let tmp = std::env::temp_dir().join(format!(
            "test_list_novels_{}_{}.db",
            std::process::id(),
            id,
        ));
        let _ = std::fs::remove_file(&tmp);
        let conn = init_db(&tmp).expect("init_db");

        let n1 = upsert_novel(&conn, "b001", "qidian", "玄幻一", "X", "玄幻", 0).unwrap();
        let n2 = upsert_novel(&conn, "b002", "qidian", "都市二", "Y", "都市", 0).unwrap();
        let _n3 = upsert_novel(&conn, "b003", "qidian", "系统三", "Z", "玄幻,系统", 0).unwrap();

        // ai_reviews_json
        update_ai_reviews(
            &conn, n1,
            r#"{"agents":{"reader":{"vote":"yes","focus":[]},"editor":{"vote":"yes","focus":[]},"author":{"vote":"yes","focus":[]}},"consensus":"all_yes","meta":{"model":"m","generated_at":"t","input_chapters":3}}"#
        ).unwrap();
        update_ai_reviews(
            &conn, n2,
            r#"{"agents":{"reader":{"vote":"yes","focus":[]},"editor":{"vote":"no","focus":[]},"author":{"vote":"maybe","focus":[]}},"consensus":"divergent","meta":{"model":"m","generated_at":"t","input_chapters":3}}"#
        ).unwrap();

        (tmp, conn)
    }

    #[test]
    fn returns_all_novels_when_no_filter() {
        let (tmp, conn) = seed();
        let rows = list_novels(&conn, &NovelListFilter::default()).expect("list_novels");
        assert_eq!(rows.len(), 3);
        // ai_reviews parse 后是 Object（前两本）或 None（第三本）
        let with_reviews = rows.iter().filter(|r| r.ai_reviews.is_some()).count();
        assert_eq!(with_reviews, 2);
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn filters_by_tag_or_semantic() {
        let (tmp, conn) = seed();
        let filter = NovelListFilter {
            tags: vec!["玄幻".to_string()],
            ..Default::default()
        };
        let rows = list_novels(&conn, &filter).expect("list_novels");
        // 玄幻一 + 系统三（tags 含"玄幻"），不含都市二
        assert_eq!(rows.len(), 2);
        let titles: Vec<String> = rows.iter().map(|r| r.title.clone()).collect();
        assert!(titles.contains(&"玄幻一".to_string()));
        assert!(titles.contains(&"系统三".to_string()));
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn filters_by_consensus() {
        let (tmp, conn) = seed();
        let filter = NovelListFilter {
            consensus: vec!["all_yes".to_string()],
            ..Default::default()
        };
        let rows = list_novels(&conn, &filter).expect("list_novels");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, "玄幻一");
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn parses_tags_csv_to_vec() {
        let (tmp, conn) = seed();
        let rows = list_novels(&conn, &NovelListFilter::default()).unwrap();
        let three = rows.iter().find(|r| r.title == "系统三").unwrap();
        assert_eq!(three.tags, vec!["玄幻".to_string(), "系统".to_string()]);
        let _ = std::fs::remove_file(&tmp);
    }
}
