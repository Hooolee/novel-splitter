pub mod spiders;
pub mod ai;
pub mod browser_spider;
pub mod scheduler;
pub mod analysis_engine;
pub mod db;

#[cfg(test)]
mod tests;

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
use tauri::{Emitter, Manager};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
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

#[tauri::command]
fn list_reports(workspace_root: String) -> Result<Vec<String>, String> {
    let mut files: Vec<String> = Vec::new();
    
    // 搜索工作目录下的 reports
    let ws_reports = Path::new(&workspace_root).join("reports");
    collect_report_files(&ws_reports, &mut files);
    
    // 同时搜索项目根目录下的 reports（可能不同于工作目录）
    let project_reports = get_project_root().join("reports");
    if project_reports != ws_reports {
        collect_report_files(&project_reports, &mut files);
    }
    
    files.sort();
    files.dedup();
    files.sort_by(|a, b| b.cmp(a)); // 最新的排前面
    Ok(files)
}

fn collect_report_files(dir: &Path, files: &mut Vec<String>) {
    if !dir.exists() { return; }
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with(".md") {
                    files.push(name.to_string());
                }
            }
        }
    }
}

#[tauri::command]
fn read_report(workspace_root: String, filename: String) -> Result<String, String> {
    // 优先从工作目录读，找不到就从项目根目录读
    let ws_path = Path::new(&workspace_root).join("reports").join(&filename);
    if ws_path.exists() {
        return fs::read_to_string(ws_path).map_err(|e| e.to_string());
    }
    let project_path = get_project_root().join("reports").join(&filename);
    fs::read_to_string(project_path).map_err(|e| e.to_string())
}

#[tauri::command]
async fn trigger_full_scan(app: tauri::AppHandle, target_url: Option<String>, platform: Option<String>) -> Result<(), String> {
    // 异步执行，不阻塞前端
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = trigger_full_scan_internal(&app_clone, target_url, platform).await;
    });
    Ok(())
}

async fn trigger_full_scan_internal(app_handle: &tauri::AppHandle, target_url: Option<String>, platform_opt: Option<String>) -> Result<(), String> {
    println!("Manual trigger from frontend/tray: scan started");
    let project_root = get_project_root();
    let config_path = project_root.join("workflow_config.json");

    let content = std::fs::read_to_string(&config_path).map_err(|e| e.to_string())?;
    let config = serde_json::from_str::<serde_json::Value>(&content).map_err(|e| e.to_string())?;

    // 从 Tauri State 读取工作目录（与前端选择一致）
    let workspace_root = get_workspace_root(app_handle);
    let mut aggregated_report = String::new();
    let mut any_success = false;

    if let Some(target) = target_url {
        let platform = platform_opt.unwrap_or_else(|| {
            if target.contains("fanqie") { "fanqie".to_string() } else { "qidian".to_string() }
        });
        // URL 含 /book/ 或 /info/ 视为单本，否则按榜单处理
        let mode = if target.contains("/book/") || target.contains("/info/") {
            crate::analysis_engine::PipelineMode::Single
        } else {
            crate::analysis_engine::PipelineMode::Rank
        };
        println!("Manual: Triggering analysis ({:?}) for target {} on platform {}", mode, target, platform);
        match crate::analysis_engine::run_full_analysis_pipeline(
            app_handle, &target, &platform, &workspace_root, mode
        ).await {
            Ok(partial) => {
                any_success = true;
                aggregated_report.push_str(&partial);
                aggregated_report.push_str("\n\n---\n\n");
            }
            Err(e) => eprintln!("Manual: Pipeline failed for {}: {}", target, e),
        }
    } else {
        if let Some(rank_urls) = config["rank_urls"].as_array() {
            println!("Manual: Found {} rank URLs to process.", rank_urls.len());
            for rank_url_val in rank_urls {
                if let Some(rank_url) = rank_url_val.as_str() {
                    let platform = if rank_url.contains("fanqie") { "fanqie" } else { "qidian" };
                    println!("Manual: Triggering analysis for {}", rank_url);
                    match crate::analysis_engine::run_full_analysis_pipeline(
                        app_handle, rank_url, platform, &workspace_root,
                        crate::analysis_engine::PipelineMode::Rank,
                    ).await {
                        Ok(partial) => {
                            any_success = true;
                            println!("Manual: Done for {}", rank_url);
                            aggregated_report.push_str(&partial);
                            aggregated_report.push_str("\n\n---\n\n");
                        }
                        Err(e) => {
                            eprintln!("Manual: Pipeline failed for {}: {}", rank_url, e);
                        }
                    }
                }
            }
        } else {
            eprintln!("Manual: 'rank_urls' not found or not an array in config.");
            return Err("No rank URLs found in config".to_string());
        }
    }
    
    if any_success {
        let full_report = format!("# 手动全量扫榜深度报告 ({})\n\n{}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            aggregated_report);
        let reports_dir = workspace_root.join("reports");
        let _ = std::fs::create_dir_all(&reports_dir);
        let report_path = reports_dir.join(format!("manual_report_{}.md", chrono::Local::now().format("%Y%m%d_%H%M%S")));
        let _ = std::fs::write(report_path, full_report);
        println!("Manual pipeline: report saved.");
    } else {
        println!("Manual pipeline: all targets failed, no report saved.");
    }

    // 发送事件通知前端更新列表
    let _ = app_handle.emit("report-generated", ());

    Ok(())
}

#[tauri::command]
async fn set_workspace_root(app: tauri::AppHandle, root: String) -> Result<(), String> {
    let state = app.state::<crate::ai::GlobalWorkspaceRoot>();
    *state.0.lock().map_err(|e| e.to_string())? = root;
    Ok(())
}

#[tauri::command]
async fn update_ai_config(app: tauri::AppHandle, api_base: String, api_key: String, model: String) -> Result<(), String> {
    let config = crate::ai::AiConfig { api_base, api_key, model };
    let state = app.state::<crate::ai::GlobalAiConfig>();
    *state.0.lock().map_err(|e| e.to_string())? = Some(config);
    println!("AI config updated via frontend settings");
    Ok(())
}

/// 手动触发单本小说的多 Agent 评估（任务二 Phase 4 的独立入口）。
///
/// 步骤：
/// 1. 从全局 AI 配置读取 API 凭证
/// 2. 从 DB 取该 novel 的 title/tags + 所有 chapters.outline_json 拼成 outline_blob
/// 3. 调用 ai::multi_agent_review 触发三 Agent 并行
/// 4. 把结果 UPDATE 到 novels.ai_reviews_json
/// 5. 返回新生成的 JSON 字符串给前端
///
/// 失败语义：AI 配置缺失 / 没有 outline_json / 三 Agent 全挂 → Err
#[tauri::command]
async fn evaluate_novel(app: tauri::AppHandle, novel_id: i64) -> Result<String, String> {
    let ai_config = {
        let state = app.state::<crate::ai::GlobalAiConfig>();
        let guard = state.0.lock().map_err(|e| format!("获取 AI 配置失败: {}", e))?;
        guard.clone().ok_or("AI 配置未设置，请在设置中配置 API Key")?
    };

    let conn = crate::db::get_conn().map_err(|e| format!("DB 连接失败: {}", e))?;
    let (title, tags, outline_blob, chapter_count) =
        crate::db::load_novel_for_review(&conn, novel_id)
            .map_err(|e| format!("未找到 novel_id={} 或读取失败: {}", novel_id, e))?;

    if chapter_count == 0 || outline_blob.trim().is_empty() {
        return Err("该书没有 outline_json，请先跑流水线 Phase 3".to_string());
    }

    // 按字符截断到 6000 chars
    let truncated: String = if outline_blob.chars().count() > 6000 {
        outline_blob.chars().take(6000).collect()
    } else {
        outline_blob
    };

    let reviews_json = crate::ai::multi_agent_review(
        ai_config,
        &title,
        &tags,
        &truncated,
        chapter_count,
    )
    .await?;

    crate::db::update_ai_reviews(&conn, novel_id, &reviews_json)
        .map_err(|e| format!("写入 ai_reviews_json 失败: {}", e))?;

    Ok(reviews_json)
}

/// library Tab 卡片列表查询（任务四a）：返回 novels + parsed ai_reviews + latest_rank + scan_count。
/// filter 字段全部可选，传 null/缺省时返回全部书。
#[tauri::command]
fn list_novels(filter: Option<crate::db::NovelListFilter>) -> Result<Vec<crate::db::NovelListRow>, String> {
    let conn = crate::db::get_conn().map_err(|e| format!("DB 连接失败: {}", e))?;
    let f = filter.unwrap_or_default();
    crate::db::list_novels(&conn, &f).map_err(|e| format!("查询书库失败: {}", e))
}

#[cfg(not(test))]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // 拦截关闭按钮，改为隐藏窗口
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .setup(|app| {
            // 0. 初始化数据库
            let project_root = get_project_root();
            let db_path = project_root.join("novel_intelligence.db");
            match db::init_db(&db_path) {
                Ok(_) => println!("Database initialized at {:?}", db_path),
                Err(e) => eprintln!("Failed to initialize database: {}", e),
            }

            // 0.5 注册全局状态
            app.manage(ai::GlobalAiConfig(Mutex::new(None)));
            app.manage(ai::GlobalWorkspaceRoot(Mutex::new(
                get_project_root().to_string_lossy().to_string()
            )));

            // 1. 创建托盘菜单
            let quit_i = MenuItem::with_id(app, "quit", "退出应用", true, None::<&str>)?;
            let show_i = MenuItem::with_id(app, "show", "显示主界面", true, None::<&str>)?;
            let run_i = MenuItem::with_id(app, "run_now", "立即全量扫榜", true, None::<&str>)?;
            
            let tray_menu = Menu::with_items(app, &[
                &show_i,
                &run_i,
                &quit_i,
            ])?;

            // 2. 创建托盘图标
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&tray_menu)
                .on_menu_event(|app, event| {
                    match event.id.as_ref() {
                        "quit" => {
                            std::process::exit(0);
                        }
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "run_now" => {
                            let app_handle = app.clone();
                            tauri::async_runtime::spawn(async move {
                                let _ = trigger_full_scan_internal(&app_handle, None, None).await;
                            });
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            // 3. 初始化调度器
            scheduler::init(app.handle().clone());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_file_content,
            get_file_tree,
            start_ai_analysis,
            fetch_ai_models,
            read_log_file,
            clear_log,
            export_chapter,
            update_novel_metadata,
            get_auto_analysis_prompt,
            ensure_workspace_dirs,
            list_reports,
            read_report,
            trigger_full_scan,
            update_ai_config,
            set_workspace_root,
            evaluate_novel,
            list_novels
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// Helper to get project root directory (parent of src-tauri)
pub fn get_project_root() -> std::path::PathBuf {
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

pub fn get_workspace_root(app: &tauri::AppHandle) -> std::path::PathBuf {
    let state = app.state::<crate::ai::GlobalWorkspaceRoot>();
    state.0.lock()
        .map(|g| std::path::PathBuf::from(g.clone()))
        .unwrap_or_else(|_| get_project_root())
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
