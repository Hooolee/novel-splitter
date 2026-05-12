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
            if let Ok(config) = crate::ensure_workflow_config() {
                let target_time = config["schedule_time"].as_str().unwrap_or("03:00");
                let enabled = config["enabled"].as_bool().unwrap_or(false);

                if enabled && current_time == target_time {
                    println!("Scheduler: Time to scan! [{}]", current_time);

                    let workspace_root_buf = crate::get_workspace_root(&app_handle);
                    let workspace_root = workspace_root_buf.as_path();
                    let mut aggregated_report = String::new();
                    let mut any_success = false;

                    if let Some(rank_urls) = config["rank_urls"].as_array() {
                        for rank_url_val in rank_urls {
                            if let Some(rank_url) = rank_url_val.as_str() {
                                let platform = if rank_url.contains("fanqie") { "fanqie" } else { "qidian" };
                                let res = crate::analysis_engine::run_full_analysis_pipeline(
                                    &app_handle,
                                    rank_url,
                                    platform,
                                    workspace_root,
                                    crate::analysis_engine::PipelineMode::Rank,
                                ).await;

                                match res {
                                    Ok(partial_report) => {
                                        any_success = true;
                                        aggregated_report.push_str(&partial_report);
                                        aggregated_report.push_str("\n\n---\n\n");
                                    },
                                    Err(e) => eprintln!("Scheduler: Rank failed {}: {}", rank_url, e),
                                }
                            }
                        }
                    }

                    if any_success {
                        let full_report = format!("# 今日多维度网文题材深度报告 ({})\n\n{}",
                            Local::now().format("%Y-%m-%d"), aggregated_report);
                        let reports_dir = workspace_root.join("reports");
                        let _ = std::fs::create_dir_all(&reports_dir);
                        let report_path = reports_dir.join(format!("report_{}.md", Local::now().format("%Y-%m-%d")));
                        let _ = std::fs::write(&report_path, full_report);
                        println!("Scheduler: Final report generated at {:?}", report_path);
                    } else {
                        println!("Scheduler: All ranks failed, no report saved.");
                    }
                }
            }
        }
    });
}
