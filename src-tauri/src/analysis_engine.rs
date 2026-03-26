use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use chrono::Local;

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

    // 保存今日快照
    pub fn save_snapshot(&self, novels: &Vec<NovelRankInfo>) -> Result<(), String> {
        let today = Local::now().format("%Y-%m-%d").to_string();
        let file_path = self.base_dir.join(format!("snapshot_{}.json", today));
        let content = serde_json::to_string_pretty(novels).map_err(|e| e.to_string())?;
        fs::write(file_path, content).map_err(|e| e.to_string())
    }

    // 读取指定日期的快照
    pub fn load_snapshot(&self, date_str: &str) -> Option<Vec<NovelRankInfo>> {
        let file_path = self.base_dir.join(format!("snapshot_{}.json", date_str));
        if let Ok(content) = fs::read_to_string(file_path) {
            serde_json::from_str(&content).ok()
        } else {
            None
        }
    }

    // 获取昨日快照日期 (简单逻辑)
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

// 核心流水线实现
pub async fn run_full_analysis_pipeline(
    app: &tauri::AppHandle,
    rank_url: &str,
    platform: &str,
    ai_config: crate::ai::AiConfig,
    workspace_root: &Path,
) -> Result<String, String> {
    println!("Pipeline: Starting analysis for {}", rank_url);
    
    let history = HistoryManager::new(workspace_root);
    let last_list = history.load_snapshot(&history.get_yesterday_date());
    
    // 1. 扫榜获取全量列表
    let novel_links = match platform {
        "qidian" => crate::spiders::qidian::fetch_rank_list(app, rank_url, false).await?,
        "fanqie" => return Err("Fanqie rank not implemented".to_string()),
        _ => return Err("Unsupported platform".to_string()),
    };

    let mut current_novels = Vec::new();
    let client = reqwest::Client::new();

    // 2. 建立临时数据结构并计算增量
    for (idx, url) in novel_links.iter().enumerate() {
        let book_id = url.split("/book/").last().unwrap_or(url).to_string();
        current_novels.push(NovelRankInfo {
            book_id,
            title: "".to_string(),
            url: url.clone(),
            rank: idx + 1,
            last_rank: None,
            rank_change: 0,
            is_new: false,
            metadata: None,
            ai_analysis: None,
        });
    }

    calculate_velocity(&mut current_novels, last_list.clone());
    println!("Pipeline: Scanned {} novels. Start analyzing top 30...", current_novels.len());

    // 缓存索引
    let last_analysis_map: HashMap<String, serde_json::Value> = last_list
        .unwrap_or_default()
        .into_iter()
        .filter_map(|n| n.ai_analysis.map(|a| (n.book_id, a)))
        .collect();

    let target_limit = 30; // 榜单前 30
    let mut report_segment = format!("## 📊 榜单分析: {}\n\n", rank_url);

    if current_novels.is_empty() {
        println!("Pipeline: Warning - No novels found for {}", rank_url);
        report_segment.push_str("> ⚠️ 未能在该榜单中发现小说，请检查 URL 是否正确或是否存在反爬墙。\n\n");
    }

    let total_to_analyze = std::cmp::min(current_novels.len(), target_limit);
    // 3. 逐一深度分析
    for i in 0..total_to_analyze {
        let novel = &mut current_novels[i];
        println!("Pipeline: Analyzing novel #{}/{} - {}", i + 1, total_to_analyze, novel.url);
        
        // 尝试获取元数据以确定书名
        match crate::spiders::qidian::fetch_novel_metadata(&client, &novel.url, app, false).await {
            Ok(meta) => {
                novel.title = meta.title.clone();
                novel.metadata = Some(serde_json::to_value(&meta).unwrap());
                
                // 检查缓存
                if let Some(cached_analysis) = last_analysis_map.get(&novel.book_id) {
                    novel.ai_analysis = Some(cached_analysis.clone());
                    let ai_report = cached_analysis.as_str().unwrap_or_default();
                    report_segment.push_str(&format!("### [{}]《{}》 (排名: {}, 变动: {})\n", 
                        if novel.is_new { "新上榜" } else { "稳坐" },
                        novel.title, novel.rank, novel.rank_change));
                    report_segment.push_str(&format!("> **微观拆解 (缓存)**: {}\n\n", ai_report));
                } else {
                    // 没有缓存，走全量下载和 AI 分析
                    println!("Pipeline: Cache miss for {}. Fetching chapters...", novel.title);
                    match crate::spiders::qidian::fetch_chapter_list(app, &novel.url, false).await {
                        Ok(chapters) => {
                            let mut combined_content = String::new();
                            for (title, ch_url) in chapters.into_iter().take(3) {
                                println!("  -> Downloading chapter: {}", title);
                                if let Ok((_, content)) = crate::spiders::qidian::download_chapter(app, &ch_url, false).await {
                                    combined_content.push_str(&format!("## {}\n{}\n\n", title, content));
                                }
                            }

                            let prompt = r#"你是一个资深网文主编。请深度拆解这前三章正文：核心冲突点、金手指底层逻辑与限制、情绪节奏、章末勾子。精简回答。"#.to_string();
                            println!("  -> Requesting AI analysis...");
                            match crate::ai::call_ai(ai_config.clone(), prompt, combined_content, false).await {
                                Ok(ai_report) => {
                                    novel.ai_analysis = Some(serde_json::to_value(&ai_report).unwrap());
                                    report_segment.push_str(&format!("### [{}]《{}》 (排名: {}, 变动: {})\n", 
                                        if novel.is_new { "新上榜" } else { "稳坐" },
                                        novel.title, novel.rank, novel.rank_change));
                                    report_segment.push_str(&format!("> **微观拆解**: {}\n\n", ai_report));
                                },
                                Err(e) => eprintln!("  -> AI failed: {}", e),
                            }
                        },
                        Err(e) => eprintln!("  -> Fetch chapters failed: {}", e),
                    }
                }
            },
            Err(e) => {
                eprintln!("Pipeline: Error fetching metadata for {}: {}", novel.url, e);
            }
        }
        
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    // 保存今日快照
    let _ = history.save_snapshot(&current_novels);
    
    Ok(report_segment)
}
