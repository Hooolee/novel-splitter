mod spiders;
mod ai;
mod browser_spider; // Expose browser spider

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use tauri::Emitter;
use chrono::Local;

// ... (Keep existing ai logic)

#[tauri::command]
async fn start_ai_analysis(
    app: tauri::AppHandle,
    api_base: String,
    api_key: String,
    model: String,
    prompt: String,
    content: String,
    response_json: Option<bool>, // 是否强制要求 JSON 返回
) -> Result<String, String> {
    // ... (Keep existing implementation)
    let app_handle = app.clone();
    
    let final_prompt = if prompt.trim().is_empty() {
        r#"你是一个拥有10年经验的网文主编，擅长拆解爆款小说的底层逻辑。
请将用户提供的这一章小说内容，反向还原为【细纲/章纲】。

要求：
1. 必须严格按照原文的叙事顺序，将内容拆解为关键情节节点。
2. 每个节点必须包含两个部分：
   - 【剧情概括】：用简练的语言概括发生了什么（Who Did What）。
   - 【写作目的】：深度分析作者写这一段的意图（例如：制造冲突、拉高期待、压抑情绪、制造危机、展示金手指、打脸爽点、埋下伏笔、转换地图等）。

请使用以下格式输出：

### 1. [剧情节点]
> **概括**: ...
> **目的**: (例如：制造冲突) ...

### 2. [剧情节点]
...

### 💡 本章核心总结
(一句话概括本章主旨)"#.to_string()
    } else {
        prompt
    };

    let config = ai::AiConfig {
        api_base,
        api_key,
        model,
    };

    let force_json = response_json.unwrap_or(false);

    tauri::async_runtime::spawn(async move {
        if let Err(e) = ai::stream_analysis(app_handle.clone(), config, final_prompt, content, force_json).await {
             let _ = app_handle.emit("ai-analysis-status", ai::Progress {
                message: format!("Error: {}", e),
                status: "error".to_string()
            });
        }
    });

    Ok("Analysis started".to_string())
}

// ... (Other existing commands) ...

#[tauri::command]
async fn fetch_ai_models(
    api_base: String,
    api_key: String,
) -> Result<Vec<String>, String> {
    let config = ai::AiConfig {
        api_base,
        api_key,
        model: "".to_string(), // Not needed for fetching models
    };
    ai::fetch_models(config).await
}

// New Command: Update Novel Metadata with AI Analysis
#[tauri::command]
fn update_novel_metadata(
    dir_name: String, 
    novel_name: String, 
    metadata: serde_json::Value // Use generic Value to allow flexible merging
) -> Result<String, String> {
    println!("Backend: update_novel_metadata called for {}", novel_name);
    let novel_path = Path::new(&dir_name).join(&novel_name);
    let info_path = novel_path.join("info.json");
    
    if !info_path.exists() {
        return Err("info.json not found".to_string());
    }
    
    // Read existing
    let content = fs::read_to_string(&info_path).map_err(|e| e.to_string())?;
    let mut current_meta: serde_json::Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    
    // Merge new metadata (assuming metadata is an object containing fields to update)
    if let Some(obj) = metadata.as_object() {
        if let Some(current_obj) = current_meta.as_object_mut() {
            for (k, v) in obj {
                current_obj.insert(k.clone(), v.clone());
            }
        }
    }
    
    // Write back
    let new_content = serde_json::to_string_pretty(&current_meta).unwrap_or_default();
    fs::write(info_path, new_content).map_err(|e| e.to_string())?;
    
    Ok("Metadata updated".to_string())
}

// Default prompt for auto (front) analysis: moved to backend for single source of truth
#[tauri::command]
fn get_auto_analysis_prompt() -> String {
    r#"你是一个专业的网文商业分析师。请阅读以上小说开篇内容（前5章），分析并以纯 JSON 格式返回以下信息（不要使用 Markdown 代码块）：
{
  "genre": "题材类型 (如：玄幻/系统/都市文)",
  "style": "整体风格 (如：轻松搞笑/热血/暗黑)",
  "goldfinger": "金手指设定 (简要概括主角的特殊能力或系统)",
  "opening": "开篇故事梗概 (100字以内)",
  "highlights": "核心看点与爽点分析 (50字以内)"
}"#.to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            start_download,
            scan_and_download_rank,
            get_file_content,
            get_file_tree,
            start_ai_analysis,
            fetch_ai_models,
            read_log_file,
            clear_log,
            delete_novel,
            delete_chapter,
            export_chapter,
            update_novel_metadata, // Register new command
            get_auto_analysis_prompt,
            ensure_workspace_dirs
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// Re-adding structs that were accidentally removed
#[derive(serde::Serialize, Clone)]
struct ProgressPayload {
    message: String,
    status: String, // "running", "completed", "error"
}

// Helper to get project root directory (parent of src-tauri)
fn get_project_root() -> std::path::PathBuf {
    // Get current exe directory, then go up to find project root
    if let Ok(exe_path) = std::env::current_exe() {
        // In development: exe is in target/debug/
        // In production: exe is in the app bundle
        if let Some(parent) = exe_path.parent() {
            // Try to find src-tauri directory
            let mut current = parent;
            for _ in 0..5 { // Search up to 5 levels
                if current.join("src-tauri").exists() {
                    return current.to_path_buf();
                }
                if let Some(p) = current.parent() {
                    current = p;
                } else {
                    break;
                }
            }
        }
    }
    // Fallback: use current directory
    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
}

// Helper for file logging (shared across modules)
// Now accepts optional workspace_root parameter
pub(crate) fn log_to_file(msg: &str) {
    log_to_file_with_root(msg, None);
}

pub(crate) fn log_to_file_with_root(msg: &str, workspace_root: Option<&Path>) {
    let log_path = match workspace_root {
        Some(root) => root.join("logs").join("app.log"),
        None => get_project_root().join("app.log"), // Fallback for backward compatibility
    };

    // Ensure logs directory exists
    if let Some(parent) = log_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    let log_msg = format!("[{}] {}\n", timestamp, msg);

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) {
        let _ = file.write_all(log_msg.as_bytes());
    }
}

// New command: Ensure workspace directories exist
#[tauri::command]
fn ensure_workspace_dirs(workspace_root: String) -> Result<String, String> {
    let root = Path::new(&workspace_root);

    // Create downloads subdirectory
    let downloads_dir = root.join("downloads");
    if !downloads_dir.exists() {
        fs::create_dir_all(&downloads_dir).map_err(|e| format!("创建 downloads 目录失败: {}", e))?;
    }

    // Create logs subdirectory
    let logs_dir = root.join("logs");
    if !logs_dir.exists() {
        fs::create_dir_all(&logs_dir).map_err(|e| format!("创建 logs 目录失败: {}", e))?;
    }

    Ok("Workspace directories created".to_string())
}

#[tauri::command]
fn read_log_file(workspace_root: Option<String>) -> Result<String, String> {
    let log_path = match workspace_root {
        Some(root) => Path::new(&root).join("logs").join("app.log"),
        None => get_project_root().join("app.log"),
    };

    if log_path.exists() {
         // Read last 100kb to avoid huge files? For now just read all.
         fs::read_to_string(log_path).map_err(|e| e.to_string())
    } else {
        Ok("暂无日志".to_string())
    }
}

#[tauri::command]
fn clear_log(workspace_root: Option<String>) -> Result<String, String> {
    println!("Backend: clear_log called");
    let log_path = match workspace_root {
        Some(root) => Path::new(&root).join("logs").join("app.log"),
        None => get_project_root().join("app.log"),
    };
    // Write empty string to clear the log file
    fs::write(log_path, "").map_err(|e| e.to_string())?;
    Ok("日志已清空".to_string())
}

#[tauri::command]
fn delete_novel(dir_name: String, novel_name: String, workspace_root: Option<String>) -> Result<String, String> {
    println!("Backend: delete_novel called with dir={}, novel={}", dir_name, novel_name);
    let novel_path = Path::new(&dir_name).join(&novel_name);
    
    if !novel_path.exists() {
        return Err("小说目录不存在".to_string());
    }
    
    if !novel_path.is_dir() {
        return Err("路径不是目录".to_string());
    }
    
    // Delete the entire directory
    fs::remove_dir_all(&novel_path).map_err(|e| format!("删除失败: {}", e))?;
    
    let workspace_path = workspace_root.as_ref().map(|r| Path::new(r));
    log_to_file_with_root(&format!("已删除小说: {}", novel_name), workspace_path);
    Ok(format!("已删除《{}》", novel_name))
}

#[tauri::command]
fn delete_chapter(dir_name: String, novel_name: String, chapter_file: String, workspace_root: Option<String>) -> Result<String, String> {
    println!("Backend: delete_chapter called with dir={}, novel={}, chapter={}", dir_name, novel_name, chapter_file);
    let chapter_path = Path::new(&dir_name).join(&novel_name).join(&chapter_file);
    
    if !chapter_path.exists() {
        return Err("章节文件不存在".to_string());
    }
    
    if !chapter_path.is_file() {
        return Err("路径不是文件".to_string());
    }
    
    // Delete the file
    fs::remove_file(&chapter_path).map_err(|e| format!("删除失败: {}", e))?;
    
    let workspace_path = workspace_root.as_ref().map(|r| Path::new(r));
    log_to_file_with_root(&format!("已删除章节: {}/{}", novel_name, chapter_file), workspace_path);
    Ok(format!("已删除章节: {}", chapter_file))
}

#[tauri::command]
fn export_chapter(novel_title: String, chapter_index: i32, content: String, workspace_root: Option<String>) -> Result<String, String> {
    println!("Backend: export_chapter called for {}", novel_title);
    // Create result directory structure: <project_root>/result/<novel_title>/ or <workspace_root>/result/<novel_title>/
    let result_dir = if let Some(root) = &workspace_root {
        Path::new(root).join("result").join(&novel_title)
    } else {
        get_project_root().join("result").join(&novel_title)
    };
    
    if !result_dir.exists() {
        fs::create_dir_all(&result_dir).map_err(|e| format!("创建目录失败: {}", e))?;
    }
    
    // Filename: <chapter_index>.md
    let filename = format!("{}.md", chapter_index);
    let file_path = result_dir.join(&filename);
    
    // Write content to file
    fs::write(&file_path, content).map_err(|e| format!("写入文件失败: {}", e))?;
    
    let path_str = file_path.to_string_lossy().to_string();
    let workspace_path = workspace_root.as_ref().map(|r| Path::new(r));
    log_to_file_with_root(&format!("已导出章节到: {}", path_str), workspace_path);
    
    Ok(path_str)
}

// Helper function to process a single novel download
async fn process_novel_download(
    client: &reqwest::Client,
    url: &str,
    chapter_count: usize,
    base_dir: &Path,
    app_handle: &tauri::AppHandle,
    is_batch: bool,
    platform: &str,
    debug_spider_visible: bool,
    workspace_root: Option<&Path>
) -> Result<String, String> {

    // 1. Get Metadata
    let msg = format!("正在获取元数据: {}", url);
    log_to_file_with_root(&msg, workspace_root);
    let _ = app_handle.emit("download-progress", ProgressPayload {
        message: msg,
        status: "running".to_string(),
    });

    // Dispatch Metadata Fetching
    let metadata_result = match platform {
        "fanqie" => spiders::fanqie::fetch_novel_metadata(client, url).await,
        "qidian" => spiders::qidian::fetch_novel_metadata(client, url, app_handle, debug_spider_visible).await,
        _ => {
            let e = format!("不支持的平台: {}", platform);
            log_to_file_with_root(&e, workspace_root);
            Err(e)
        },
    };

    // Propagate error
    let metadata = match metadata_result {
        Ok(m) => m,
        Err(e) => {
             let msg = format!("获取元数据失败: {}", e);
             log_to_file_with_root(&msg, workspace_root);
             let _ = app_handle.emit("download-progress", ProgressPayload {
                message: msg.clone(),
                status: "error".to_string(),
            });
            return Err(msg);
        }
    };

    // Clean title
    let safe_title = metadata.title.replace("/", "_").replace("\\", "_");
    let novel_dir = base_dir.join(&safe_title);

    if !novel_dir.exists() {
        fs::create_dir_all(&novel_dir).map_err(|e| e.to_string())?;
    }

    // Save/Update metadata (always update to keep metadata fresh)
    let metadata_json = serde_json::to_string_pretty(&metadata).unwrap_or_default();
    let _ = fs::write(novel_dir.join("info.json"), metadata_json);

    // 2. Fetch Chapters
    let _ = app_handle.emit("download-progress", ProgressPayload {
        message: format!("正在获取章节列表 [{}]...", safe_title),
        status: "running".to_string(),
    });

    // dispatching list scraping based on platform
    let chapters = match platform {
        "fanqie" => {
             let resp = client.get(url).header("User-Agent", "Mozilla/5.0").send().await.map_err(|e| e.to_string())?;
             let html = resp.text().await.unwrap_or_default();
             let document = scraper::Html::parse_document(&html);
             let title_selector = scraper::Selector::parse(".chapter-item-title").unwrap();
             let mut chs = Vec::new();
             for element in document.select(&title_selector) {
                let title = element.text().collect::<String>();
                let href = element.value().attr("href").unwrap_or_default();
                if !href.is_empty() {
                    chs.push((title, href.to_string()));
                }
            }
            chs
        },
        "qidian" => {
             // For Qidian, use browser spider to get catalog
             match spiders::qidian::fetch_chapter_list(app_handle, url, debug_spider_visible).await {
                 Ok(list) => list,
                 Err(e) => {
                     let msg = format!("获取章节列表失败: {}", e);
                     let _ = app_handle.emit("download-progress", ProgressPayload {
                        message: msg.clone(),
                        status: "error".to_string(),
                    });
                     return Err(msg); // 直接失败，避免空列表导致"下载完成"假象
                 }
             }
        },
        _ => return Err(format!("Unknown platform for chapter list: {}", platform)),
    };

    // Skip first (latest) logic - Fanqie specific? Maybe.
    let chapters_to_download: Vec<_> = if platform == "fanqie" {
        chapters.into_iter().skip(1).take(chapter_count).collect()
    } else {
        chapters.into_iter().take(chapter_count).collect()
    };

    let count = chapters_to_download.len();
    let msg = format!("准备下载 {} 章 (总请求: {})...", count, chapter_count);
    log_to_file_with_root(&msg, workspace_root);
    let _ = app_handle.emit("download-progress", ProgressPayload {
        message: msg,
        status: "running".to_string(),
    });

    if count == 0 {
        log_to_file_with_root("警告: 待下载章节数为 0，任务提前结束。", workspace_root);
    }

    let mut downloaded_count = 0;
    let mut skipped_count = 0;

    for (i, (title, href)) in chapters_to_download.iter().enumerate() {
        let filename = format!("{:02}.txt", i + 1);
        let chapter_file_path = novel_dir.join(&filename);

        // 章节去重检查：如果章节文件已存在，则跳过
        if chapter_file_path.exists() {
            let msg = format!("跳过已存在章节 [{}] - {}", safe_title, title);
            log_to_file_with_root(&msg, workspace_root);
            if !is_batch {
                let _ = app_handle.emit("download-progress", ProgressPayload {
                    message: msg,
                    status: "skipped".to_string(),
                });
            }
            skipped_count += 1;
            continue;
        }

        if !is_batch {
             let _ = app_handle.emit("download-progress", ProgressPayload {
                message: format!("下载 [{}] - {}", safe_title, title),
                status: "running".to_string(),
            });
        }

        let chapter_url = if platform == "fanqie" {
            format!("https://fanqienovel.com{}", href)
        } else {
             href.to_string()
        };

        let mut attempt = 0;
        let max_retries = 3;

        while attempt < max_retries {
             // Dispatch Chapter Content Download
             let download_result = match platform {
                "fanqie" => spiders::fanqie::download_chapter(client, &chapter_url).await,
                "qidian" => spiders::qidian::download_chapter(app_handle, &chapter_url, debug_spider_visible).await, // Use browser spider logic
                _ => Err("Unsupported".to_string()),
            };

            if let Ok((_, content)) = download_result {
                let full_content = format!("标题: {}\n链接: {}\n{}\n\n{}", title, chapter_url, "=".repeat(50), content);
                let _ = fs::write(&chapter_file_path, full_content);
                downloaded_count += 1;
                break;
            }
            attempt += 1;
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    // 输出下载统计
    if skipped_count > 0 {
        let msg = format!("《{}》下载统计: 新下载 {} 章, 跳过已存在 {} 章", safe_title, downloaded_count, skipped_count);
        log_to_file_with_root(&msg, workspace_root);
    }

    Ok(safe_title)
}

#[tauri::command]
async fn start_download(
    app: tauri::AppHandle, 
    url: String, 
    count: usize, 
    dir_name: String,
    platform: String,
    debug_spider_visible: bool,
    workspace_root: Option<String>
) -> Result<String, String> {
    let app_handle = app.clone();
    
    if url.is_empty() || dir_name.is_empty() {
        return Err("请输入小说链接".to_string());
    }

    tauri::async_runtime::spawn(async move {
        let save_path = std::path::PathBuf::from(&dir_name);
        if !save_path.exists() {
             let _ = fs::create_dir_all(&save_path);
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();
        
        let workspace_path = workspace_root.as_ref().map(|r| std::path::PathBuf::from(r));
        let workspace_path_ref = workspace_path.as_deref();
            
        match process_novel_download(&client, &url, count, &save_path, &app_handle, false, &platform, debug_spider_visible, workspace_path_ref).await {
            Ok(title) => {
                 let msg = format!("《{}》下载完成!", title);
                 log_to_file_with_root(&msg, workspace_path_ref);
                 let _ = app_handle.emit("download-progress", ProgressPayload {
                    message: msg,
                    status: "completed".to_string(),
                });
            },
            Err(e) => {
                 let msg = format!("Error: {}", e);
                 log_to_file_with_root(&msg, workspace_path_ref);
                 let _ = app_handle.emit("download-progress", ProgressPayload {
                    message: msg,
                    status: "error".to_string(),
                });
            }
        }
    });

    Ok("Task started".to_string())
}

#[tauri::command]
async fn scan_and_download_rank(
    app: tauri::AppHandle, 
    rank_url: String, 
    max_novels: usize, 
    count_per_novel: usize, 
    dir_name: String,
    platform: String,
    debug_spider_visible: bool,
    workspace_root: Option<String>
) -> Result<String, String> {
    let app_handle = app.clone();
    
    tauri::async_runtime::spawn(async move {
        let _ = app_handle.emit("download-progress", ProgressPayload {
            message: "开始分析榜单...".to_string(),
            status: "running".to_string(),
        });

        // Create client inside spawn
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap();
        
        let workspace_path = workspace_root.as_ref().map(|r| std::path::PathBuf::from(r));
        let workspace_path_ref = workspace_path.as_deref();
        
        // 1. Fetch Rank List - Dispatch (Async)
        let novel_links_res = match platform.as_str() {
             "fanqie" => spiders::fanqie::fetch_rank_list(&client, &rank_url).await,
             "qidian" => spiders::qidian::fetch_rank_list(&app_handle, &rank_url, debug_spider_visible).await,
             _ => Err(format!("不支持的平台: {}", platform)),
        };

        match novel_links_res {
            Ok(links) => {
                 let total = std::cmp::min(links.len(), max_novels);
                 let target_links: Vec<String> = links.into_iter().take(total).collect();
                 
                  let _ = app_handle.emit("download-progress", ProgressPayload {
                    message: format!("分析完成，准备抓取前 {} 本小说...", total),
                    status: "running".to_string(),
                });
                
                let save_path_buf = std::path::PathBuf::from(&dir_name);
                
                for (idx, url) in target_links.iter().enumerate() {
                    let _ = app_handle.emit("download-progress", ProgressPayload {
                        message: format!("正在处理 [{}/{}] 正在解析...", idx + 1, total),
                        status: "running".to_string(),
                    });
                    
                    // Call Async Process
                    match process_novel_download(&client, url, count_per_novel, &save_path_buf, &app_handle, true, &platform, debug_spider_visible, workspace_path_ref).await {
                        Ok(title) => {
                            let msg = format!("《{}》下载完成!", title);
                            log_to_file_with_root(&msg, workspace_path_ref);
                            let _ = app_handle.emit("download-progress", ProgressPayload {
                                message: msg,
                                status: "completed".to_string(),
                            });
                        },
                        Err(e) => {
                            let msg = format!("Skipped one: {}", e);
                            log_to_file_with_root(&msg, workspace_path_ref);
                            let _ = app_handle.emit("download-progress", ProgressPayload {
                                message: msg,
                                status: "error".to_string(),
                            });
                        }
                    }
                }
                
                let _ = app_handle.emit("download-progress", ProgressPayload {
                    message: "榜单扫摸全部完成!".to_string(),
                    status: "completed".to_string(),
                });
            },
            Err(e) => {
                 let _ = app_handle.emit("download-progress", ProgressPayload {
                    message: format!("榜单获取失败: {}", e),
                    status: "error".to_string(),
                });
            }
        }
    });

    Ok("Batch task started".to_string())
}

#[tauri::command]
fn get_file_content(dir: String, filename: String) -> Result<String, String> {
    let path = Path::new(&dir).join(&filename);
    fs::read_to_string(path).map_err(|e| e.to_string())
}


#[derive(serde::Serialize)]
struct FileNode {
    name: String,
    path: String, // Relative path from base
    is_dir: bool,
    children: Vec<FileNode>,
}

fn read_dir_recursive(base_path: &Path, relative_path: &Path) -> Vec<FileNode> {
    let target_path = base_path.join(relative_path);
    let mut nodes = Vec::new();
    
    if let Ok(entries) = fs::read_dir(target_path) {
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                let is_dir = path.is_dir();
                let new_rel_path = relative_path.join(&name);
                
                // Filter: Only dirs or txt/json files
                if !is_dir {
                    let ext = path.extension().unwrap_or_default();
                    if ext != "txt" && ext != "json" {
                        continue;
                    }
                }

                let mut children = Vec::new();
                if is_dir {
                    children = read_dir_recursive(base_path, &new_rel_path);
                }

                nodes.push(FileNode {
                    name,
                    path: new_rel_path.to_string_lossy().to_string(),
                    is_dir,
                    children,
                });
            }
        }
    }
    // Sort: Dirs first, then files
    nodes.sort_by(|a, b| {
        if a.is_dir == b.is_dir {
            a.name.cmp(&b.name)
        } else {
            b.is_dir.cmp(&a.is_dir)
        }
    });
    nodes
}

#[tauri::command]
fn get_file_tree(dir_name: String) -> Result<Vec<FileNode>, String> {
    let path = Path::new(&dir_name);
    if !path.exists() {
        return Ok(Vec::new());
    }
    Ok(read_dir_recursive(path, Path::new("")))
}
