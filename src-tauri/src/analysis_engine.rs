use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use chrono::Local;
use tauri::Manager;
use tokio::sync::Semaphore;
use tokio::time::{sleep, Duration};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NovelRankInfo {
    pub book_id: String,
    pub title: String,
    pub url: String,
    pub rank: usize,
    pub last_rank: Option<usize>,
    pub rank_change: i32,
    pub is_new: bool,
    pub metadata: Option<serde_json::Value>,
    pub ai_analysis: Option<serde_json::Value>,
}

pub struct HistoryManager {
    base_dir: PathBuf,
}

impl HistoryManager {
    pub fn new(workspace_root: &Path) -> Self {
        let base_dir = workspace_root.join("analysis_data");
        if !base_dir.exists() {
            let _ = fs::create_dir_all(&base_dir);
        }
        Self { base_dir }
    }

    pub fn save_snapshot(&self, novels: &Vec<NovelRankInfo>) -> Result<(), String> {
        let today = Local::now().format("%Y-%m-%d").to_string();
        let file_path = self.base_dir.join(format!("snapshot_{}.json", today));
        let content = serde_json::to_string_pretty(novels).map_err(|e| e.to_string())?;
        fs::write(file_path, content).map_err(|e| e.to_string())
    }

    pub fn load_snapshot(&self, date_str: &str) -> Option<Vec<NovelRankInfo>> {
        let file_path = self.base_dir.join(format!("snapshot_{}.json", date_str));
        if let Ok(content) = fs::read_to_string(file_path) {
            serde_json::from_str(&content).ok()
        } else {
            None
        }
    }

    pub fn get_yesterday_date(&self) -> String {
        use chrono::Duration;
        let yesterday = Local::now() - Duration::days(1);
        yesterday.format("%Y-%m-%d").to_string()
    }
}

pub fn calculate_velocity(current_list: &mut Vec<NovelRankInfo>, last_list: Option<Vec<NovelRankInfo>>) {
    let last_map: HashMap<String, usize> = last_list
        .map(|list| list.into_iter().map(|n| (n.book_id, n.rank)).collect())
        .unwrap_or_default();

    for novel in current_list.iter_mut() {
        if let Some(&last_rank) = last_map.get(&novel.book_id) {
            novel.last_rank = Some(last_rank);
            novel.rank_change = (last_rank as i32) - (novel.rank as i32);
            novel.is_new = false;
        } else {
            novel.last_rank = None;
            novel.rank_change = 0;
            novel.is_new = true;
        }
    }
}

// ---------- AI 提纯提示词：输出 JSON 细纲 ----------
const OUTLINE_ANALYSIS_PROMPT: &str = r#"你是一个专业网文拆解助手。
请将以下小说章节内容拆解为细纲，严格按照原文叙事顺序。

对于每个关键情节节点，输出一个 JSON 对象数组：

[
  {
    "event": "剧情概括（一句话描述该节点发生的事件）",
    "purpose": "写作目的（如：制造冲突、拉高期待、压抑情绪、展示金手指、打脸爽点、埋下伏笔、转换地图、引入新角色等）",
    "emotion": "该段落的主要情绪基调（如：紧张、兴奋、压抑、爽快、悬疑、温馨、热血等）",
    "highlight": "核心看点/吸引点（读者为什么会被这段吸引）"
  }
]

要求：
1. 严格按原文顺序
2. 每个节点 1-3 句话
3. 只输出 JSON 数组，不要任何额外文字
4. 如果无法拆解请输出 []"#;

const MAX_CONCURRENCY: usize = 3;
const TARGET_CHAPTERS: usize = 3;

// ========================================================================
//  Phase 1: Producer — 扫榜分发, 只取 book_id + 书名 + URL
// ========================================================================
async fn producer_scan_rank(
    app: &tauri::AppHandle,
    rank_url: &str,
    platform: &str,
) -> Result<Vec<(i64, String, String, String)>, String> {
    eprintln!("[Producer] 扫榜: {}", rank_url);

    let novel_links = match platform {
        "qidian" => crate::spiders::qidian::fetch_rank_list(app, rank_url, false).await?,
        "fanqie" => return Err("番茄榜单暂未实现".to_string()),
        _ => return Err("不支持的平台".to_string()),
    };

    let max_books = std::env::var("PIPELINE_MAX_BOOKS").ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(30);
    let limit = std::cmp::min(novel_links.len(), max_books);
    if limit == 0 {
        return Err("榜单中没有找到小说".to_string());
    }

    // 扫榜成功后创建报告（避免空报告）
    let db_conn = crate::db::get_conn().ok();
    let mut report_id_opt = None;
    if let Some(ref conn) = db_conn {
        match crate::db::create_scan_report(conn, rank_url) {
            Ok(rid) => report_id_opt = Some(rid),
            Err(e) => eprintln!("[Producer] DB: 创建报告失败: {}", e),
        }
    }

    let client = reqwest::Client::new();
    let mut results = Vec::new();

    for (idx, url) in novel_links.iter().enumerate().take(limit) {
        let book_id = url.split("/book/")
            .last()
            .unwrap_or(url)
            .trim_end_matches('/')
            .to_string();

        let (title, author, tags) = match platform {
            "qidian" => {
                match crate::spiders::qidian::fetch_novel_metadata(&client, url, app, false).await {
                    Ok(meta) => (meta.title.clone(), "未知".to_string(), meta.tags.join(",")),
                    Err(e) => {
                        eprintln!("[Producer] 获取元数据失败 [{}]: {}", url, e);
                        (format!("未知书籍-{}", idx + 1), "未知".to_string(), String::new())
                    }
                }
            }
            _ => (format!("未知书籍-{}", idx + 1), "未知".to_string(), String::new()),
        };

        if let Some(ref conn) = db_conn {
            match crate::db::upsert_novel(conn, &book_id, platform, &title, &author, &tags, 0) {
                Ok(nid) => {
                    if let Some(rid) = report_id_opt {
                        let change_str = format!("+{}", idx + 1);
                        let _ = crate::db::insert_rank_history(conn, rid, nid, (idx + 1) as i64, &change_str);
                    }
                    results.push((nid, book_id.clone(), title.clone(), url.clone()));
                    eprintln!("[Producer] #{}/{} id={} title={}", idx + 1, limit, book_id, title);
                }
                Err(e) => eprintln!("[Producer] DB 写入失败: {}", e),
            }
        }
    }

    eprintln!("[Producer] 完成: 扫到 {} 本书", results.len());
    Ok(results)
}

// ========================================================================
//  Phase 2: Fetch Worker — 并发抓取章节 (Semaphore=3, 按书粒度)
// ========================================================================
async fn run_fetch_workers(
    app: &tauri::AppHandle,
    books: Vec<(i64, String, String)>,
    platform: &str,
    workspace_root: &Path,
    semaphore: Arc<Semaphore>,
) -> Result<(usize, usize), String> {
    if books.is_empty() {
        eprintln!("[Fetch Worker] 没有待抓取的小说");
        return Ok((0, 0));
    }

    eprintln!("[Fetch Worker] {} 本待抓取, Semaphore({}) 并发", books.len(), MAX_CONCURRENCY);

    let download_dir = workspace_root.join("downloads");
    let mut handles = Vec::new();

    for (novel_id, title, novel_url) in books {
        let permit = semaphore.clone().acquire_owned().await.map_err(|e| e.to_string())?;
        let app = app.clone();
        let d_dir = download_dir.clone();
        let plat = platform.to_string();

        handles.push(tokio::spawn(async move {
            let _permit = permit;
            let safe_title = title.replace("/", "_").replace("\\", "_");
            let novel_dir = d_dir.join(&safe_title);
            let _ = fs::create_dir_all(&novel_dir);

            let chapters = match plat.as_str() {
                "qidian" => crate::spiders::qidian::fetch_chapter_list(&app, &novel_url, false).await,
                _ => Err("不支持的平台".to_string()),
            };

            let chapters = match chapters {
                Ok(list) => list,
                Err(e) => {
                    eprintln!("[Fetch Worker] 获取章节列表失败 {}: {}", title, e);
                    return (0usize, 1usize);
                }
            };

            let mut success = 0usize;
            let mut fail = 0usize;
            let target = std::cmp::min(chapters.len(), TARGET_CHAPTERS);

            for i in 0..target {
                if i >= chapters.len() { break; }
                let (ch_title, ch_url) = &chapters[i];
                let filename = format!("{:02}.txt", i + 1);
                let file_path = novel_dir.join(&filename);

                if file_path.exists() {
                    success += 1;
                    continue;
                }

                let download = match plat.as_str() {
                    "qidian" => crate::spiders::qidian::download_chapter(&app, ch_url, false).await,
                    _ => Err("不支持的平台".to_string()),
                };

                match download {
                    Ok((_, content)) => {
                        let full = format!("标题: {}\n链接: {}\n{}\n\n{}", ch_title, ch_url, "=".repeat(50), content);
                        let _ = fs::write(&file_path, &full);

                        if let Ok(conn) = crate::db::get_conn() {
                            let _ = crate::db::upsert_chapter(&conn, novel_id, (i + 1) as i64, ch_title, &content, None);
                        }
                        success += 1;
                    }
                    Err(e) => {
                        eprintln!("[Fetch Worker] 下载章节失败 {}: {}", ch_title, e);
                        fail += 1;
                    }
                }

                sleep(Duration::from_millis(200)).await;
            }

            eprintln!("[Fetch Worker] {} 完成: 成功{} 失败{}", title, success, fail);
            (success, fail)
        }));
    }

    let mut total_ok = 0usize;
    let mut total_fail = 0usize;
    for h in handles {
        let (ok, fail) = h.await.unwrap_or((0, 1));
        total_ok += ok;
        total_fail += fail;
    }

    eprintln!("[Fetch Worker] 全部完成: 总成功{} 总失败{}", total_ok, total_fail);
    Ok((total_ok, total_fail))
}

// ========================================================================
//  Phase 3: AI Worker — 并发 AI 提纯 (Semaphore=3, 按章粒度)
// ========================================================================
async fn run_ai_workers(
    ai_config: crate::ai::AiConfig,
    semaphore: Arc<Semaphore>,
) -> Result<usize, String> {
    let pending: Vec<(i64, String, String)> = {
        let conn = crate::db::get_conn().map_err(|e| format!("DB 连接失败: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT c.id, c.title, c.content FROM chapters c
             WHERE c.outline_json IS NULL AND c.content IS NOT NULL AND c.content != ''
             LIMIT 50"
        ).map_err(|e| e.to_string())?;

        let items = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        }).map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect::<Vec<_>>();
        items
    };

    if pending.is_empty() {
        eprintln!("[AI Worker] 没有待提纯的章节");
        return Ok(0);
    }

    eprintln!("[AI Worker] {} 章节待提纯, Semaphore({}) 并发", pending.len(), MAX_CONCURRENCY);

    let prompt = OUTLINE_ANALYSIS_PROMPT.to_string();
    let mut handles = Vec::new();

    for (ch_id, title, content) in pending {
        let permit = semaphore.clone().acquire_owned().await.map_err(|e| e.to_string())?;
        let config = ai_config.clone();
        let prompt = prompt.clone();

        handles.push(tokio::spawn(async move {
            let _permit = permit;

            // 按 char 边界安全截断中文内容
            let truncated = if content.len() > 8000 {
                let boundary = content.char_indices()
                    .nth(4000)
                    .map(|(i, _)| i)
                    .unwrap_or(content.len());
                content[..boundary].to_string()
            } else {
                content
            };

            match crate::ai::call_ai(config, prompt, truncated, true).await {
                Ok(json_str) => {
                    match serde_json::from_str::<serde_json::Value>(&json_str) {
                        Ok(_) => {
                            if let Ok(conn) = crate::db::get_conn() {
                                let _ = conn.execute(
                                    "UPDATE chapters SET outline_json = ?1 WHERE id = ?2",
                                    rusqlite::params![json_str, ch_id],
                                );
                            }
                            eprintln!("[AI Worker] ✅ {} 提纯完成", title);
                            1usize
                        }
                        Err(e) => {
                            eprintln!("[AI Worker] ⚠️ {} JSON 校验失败: {}", title, e);
                            0usize
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[AI Worker] ❌ {} AI 调用失败: {}", title, e);
                    0usize
                }
            }
        }));
    }

    let mut total = 0usize;
    for h in handles {
        total += h.await.unwrap_or(0);
    }

    eprintln!("[AI Worker] 全部完成: 成功提纯 {} 章", total);
    Ok(total)
}

// ========================================================================
//  报告生成 — 从 DB 读取数据生成 Markdown
// ========================================================================
async fn generate_report(rank_url: &str) -> Result<String, String> {
    let conn = crate::db::get_conn().map_err(|e| format!("DB 连接失败: {}", e))?;

    let mut report_stmt = conn.prepare(
        "SELECT id FROM scan_reports WHERE rank_type = ?1 ORDER BY id DESC LIMIT 1"
    ).map_err(|e| e.to_string())?;
    let report_id: i64 = report_stmt.query_row([rank_url], |row| row.get(0))
        .map_err(|_| "未找到扫榜报告".to_string())?;

    let mut stmt = conn.prepare(
        "SELECT n.title, n.book_id, n.tags, rh.rank, rh.rank_change
         FROM rank_history rh
         JOIN novels n ON n.id = rh.novel_id
         WHERE rh.report_id = ?1
         ORDER BY rh.rank ASC"
    ).map_err(|e| e.to_string())?;

    let rows: Vec<(String, String, String, i64, String)> = stmt.query_map([report_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, String>(4)?,
        ))
    }).map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .collect();

    if rows.is_empty() {
        return Ok(format!("# 📊 榜单分析: {}\n\n> ⚠️ 未在榜单中发现小说\n", rank_url));
    }

    let mut report = format!("# 📊 榜单分析: {}\n\n**扫榜时间**: {}\n**上榜小说**: {} 本\n\n",
        rank_url,
        Local::now().format("%Y-%m-%d %H:%M"),
        rows.len(),
    );

    for (title, book_id, tags, rank, change) in &rows {
        report.push_str(&format!(
            "### #{}. 《{}》 (变动: {})\n> book_id: `{}` | tags: {}\n\n",
            rank, title, change, book_id, tags,
        ));
    }

    Ok(report)
}

// ========================================================================
//  核心公开 API — 三段式管线 + 全局 AI 配置
// ========================================================================
pub async fn run_full_analysis_pipeline(
    app: &tauri::AppHandle,
    rank_url: &str,
    platform: &str,
    workspace_root: &Path,
) -> Result<String, String> {
    eprintln!("\n========== Pipeline: {} ==========", rank_url);
    let started = Local::now();

    // 从 Tauri 全局状态读取 AI 配置（由前端 UI 设置）
    let ai_config = {
        let state = app.state::<crate::ai::GlobalAiConfig>();
        let guard = state.0.lock().map_err(|e| format!("获取 AI 配置失败: {}", e))?;
        guard.clone().ok_or_else(|| "AI 配置未设置，请在设置中配置 API Key".to_string())?
    };
    let db_conn = crate::db::get_conn().ok();
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENCY));

    // ------ Phase 1: Producer ------
    eprintln!("[Pipeline 1/3] Producer...");
    let books = producer_scan_rank(app, rank_url, platform).await?;
    if books.is_empty() {
        return Err("Producer 未扫到有效书籍".to_string());
    }

    // ------ Phase 2: Fetch Workers (Semaphore=3) ------
    eprintln!("[Pipeline 2/3] Fetch Workers...");
    let fetch_list: Vec<(i64, String, String)> = books.iter()
        .map(|(id, _, title, url)| (*id, title.clone(), url.clone()))
        .collect();
    let (_ch_ok, _ch_fail) = run_fetch_workers(
        app, fetch_list, platform, workspace_root, semaphore.clone()
    ).await?;

    // ------ Phase 3: AI Workers (Semaphore=3) ------
    eprintln!("[Pipeline 3/3] AI Workers...");
    let _ai_ok = run_ai_workers(ai_config, semaphore.clone()).await?;

    // ------ 快照 + 速度分析 ------
    {
        let history = HistoryManager::new(workspace_root);
        let last = history.load_snapshot(&history.get_yesterday_date());
        let mut current: Vec<NovelRankInfo> = books.iter().map(|(_, bid, title, url)| {
            NovelRankInfo {
                book_id: bid.clone(),
                title: title.clone(),
                url: url.clone(),
                rank: 0,
                last_rank: None,
                rank_change: 0,
                is_new: false,
                metadata: None,
                ai_analysis: None,
            }
        }).collect();
        if let Some(ref conn) = db_conn {
            let _ = conn.execute("UPDATE novels SET updated_at = CURRENT_TIMESTAMP WHERE id IN (
                SELECT novel_id FROM rank_history WHERE report_id = (SELECT id FROM scan_reports ORDER BY id DESC LIMIT 1)
            )", []);
        }
        calculate_velocity(&mut current, last);
        let _ = history.save_snapshot(&current);
    }

    let elapsed = Local::now().signed_duration_since(started);
    eprintln!("========== Pipeline 完成: {:.1}s ==========",
        elapsed.num_seconds() as f64 + elapsed.num_milliseconds() as f64 / 1000.0);

    let report = generate_report(rank_url).await?;
    Ok(report)
}
