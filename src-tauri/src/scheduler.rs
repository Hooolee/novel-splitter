use tauri::AppHandle;
use tokio::time::{interval, Duration};
use chrono::Local;

pub fn init(app_handle: AppHandle) {
    // 启动一个后台任务
    tauri::async_runtime::spawn(async move {
        let mut check_interval = interval(Duration::from_secs(60));
        println!("Scheduler: Loop started.");
        
        loop {
            check_interval.tick().await;
            let now = Local::now();
            let current_time = now.format("%H:%M").to_string();
            
            // 1. 尝试加载配置 (通常放在工作区根目录)
            // 这里我们预设一个路径，或者从 AppHandle 拿到当前工作区
            let config_path = std::path::PathBuf::from("/Users/a10763/codes/projects/muse/workflow_config.json");
            
            if let Ok(content) = std::fs::read_to_string(&config_path) {
                if let Ok(config) = serde_json::from_str::<serde_json::Value>(&content) {
                    let target_time = config["schedule_time"].as_str().unwrap_or("03:00");
                    let enabled = config["enabled"].as_bool().unwrap_or(false);

                    if enabled && current_time == target_time {
                        println!("Scheduler: Time to scan! [{}]", current_time);
                        
                        let ai_config = crate::ai::AiConfig {
                            api_base: config["ai"]["api_base"].as_str().unwrap_or_default().to_string(),
                            api_key: config["ai"]["api_key"].as_str().unwrap_or_default().to_string(),
                            model: config["ai"]["model"].as_str().unwrap_or_default().to_string(),
                        };
                        let workspace_root = std::path::Path::new("/Users/a10763/codes/projects/muse/novel-splitter");
                        let mut aggregated_report = format!("# 今日多维度网文题材深度报告 ({})\n\n", Local::now().format("%Y-%m-%d"));

                        if let Some(rank_urls) = config["rank_urls"].as_array() {
                            for rank_url_val in rank_urls {
                                if let Some(rank_url) = rank_url_val.as_str() {
                                    // 执行单个排行榜分析逻辑
                                    let res = crate::analysis_engine::run_full_analysis_pipeline(
                                        &app_handle,
                                        rank_url,
                                        "qidian",
                                        ai_config.clone(),
                                        workspace_root,
                                    ).await;

                                    match res {
                                        Ok(partial_report) => {
                                            aggregated_report.push_str(&partial_report);
                                            aggregated_report.push_str("\n\n---\n\n");
                                        },
                                        Err(e) => eprintln!("Scheduler: Rank failed {}: {}", rank_url, e),
                                    }
                                }
                            }
                        }

                        // 保存最终聚合报告
                        let reports_dir = workspace_root.join("reports");
                        let _ = std::fs::create_dir_all(&reports_dir);
                        let report_path = reports_dir.join(format!("report_{}.md", Local::now().format("%Y-%m-%d")));
                        let _ = std::fs::write(report_path, aggregated_report);
                        println!("Scheduler: Final report generated at {:?}", report_path);
                    }
                }
            }
        }
    });
}
