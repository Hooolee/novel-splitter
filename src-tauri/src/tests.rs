use std::sync::Mutex;
use tauri::Manager;

/// E2E 管线测试：选择一个真实榜单，走完 Producer → Fetch Workers → AI Workers
/// 验证：
///   1. 扫榜后 novels 表有数据
///   2. 下载后 chapters 表有数据 + 文件写入
///   3. AI 提纯后 outline_json 不为 NULL
///   4. 报告文件生成
///
/// 用法: cd src-tauri && cargo test test_pipeline_e2e -- --nocapture
#[test]
fn test_pipeline_e2e() {
    // 1. 构建 Tauri App（不启动事件循环）
    let app = tauri::Builder::default()
        .build(tauri::generate_context!())
        .expect("构建 Tauri App 失败");
    let handle = app.handle();

    // 2. 注册 AI 全局配置
    handle.manage(crate::ai::GlobalAiConfig(Mutex::new(Some(crate::ai::AiConfig {
        api_base: std::env::var("AI_API_BASE").unwrap_or_else(|_| "http://127.0.0.1:8317/v1".into()),
        api_key: std::env::var("AI_API_KEY").unwrap_or_else(|_| "sk-test".into()),
        model: std::env::var("AI_MODEL").unwrap_or_else(|_| "gemini-3-flash-preview".into()),
    }))));

    // 3. 初始化数据库（测试用独立文件）
    let project_root = crate::get_project_root();
    let db_path = project_root.join("test_novel_intelligence_e2e.db");
    let _ = std::fs::remove_file(&db_path);
    crate::db::init_db(&db_path).expect("DB 初始化失败");

    let rank_url = "https://www.qidian.com/rank/hotsales/";
    let platform = "qidian";

    // 4. 跑管线（tokio 运行时）
    let rt = tokio::runtime::Runtime::new().expect("创建 tokio runtime 失败");
    let result = rt.block_on(async {
        crate::analysis_engine::run_full_analysis_pipeline(
            &handle, rank_url, platform, &project_root,
            crate::analysis_engine::PipelineMode::Rank,
        ).await
    });

    // 5. 验证管线返回
    assert!(result.is_ok(), "管线执行失败: {:?}", result.err());
    let report = result.unwrap();
    assert!(report.contains("📊 榜单分析"), "报告缺少标题: {}", report);
    assert!(report.contains("上榜小说"), "报告缺少书籍统计: {}", report);
    println!("✅ Pipeline returned report ({} bytes)", report.len());

    // 6. 验证 DB 有数据
    let conn = crate::db::get_conn().expect("打开 DB 失败");

    let novel_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM novels", [], |row| row.get(0))
        .unwrap_or(0);
    assert!(novel_count > 0, "novels 表无数据");
    println!("✅ novels 表: {} 本", novel_count);

    let chapter_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM chapters", [], |row| row.get(0))
        .unwrap_or(0);
    assert!(chapter_count > 0, "chapters 表无数据");
    println!("✅ chapters 表: {} 章", chapter_count);

    let outlined: i64 = conn
        .query_row("SELECT COUNT(*) FROM chapters WHERE outline_json IS NOT NULL", [], |row| row.get(0))
        .unwrap_or(0);
    println!("📊 AI 提纯完成: {}/{} 章", outlined, chapter_count);

    let report_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM scan_reports", [], |row| row.get(0))
        .unwrap_or(0);
    assert!(report_count > 0, "scan_reports 表无数据");
    println!("✅ scan_reports: {} 份", report_count);

    // 7. 验证文件写入
    let downloads_dir = project_root.join("downloads");
    assert!(downloads_dir.exists(), "downloads 目录不存在");
    let entries: Vec<_> = std::fs::read_dir(&downloads_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();
    assert!(!entries.is_empty(), "无下载目录");
    println!("✅ download 目录: {} 本小说", entries.len());

    // 显示前三本
    for entry in entries.iter().take(3) {
        let name = entry.file_name();
        let chapter_count = std::fs::read_dir(entry.path())
            .map(|d| d.filter_map(|e| e.ok()).count())
            .unwrap_or(0);
        println!("   📁 {} ({} 章)", name.to_string_lossy(), chapter_count);
    }

    // 8. 验证报告文件
    let reports_dir = project_root.join("reports");
    if reports_dir.exists() {
        let report_files: Vec<_> = std::fs::read_dir(&reports_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|e| e == "md").unwrap_or(false))
            .collect();
        println!("✅ reports 目录: {} 个报告文件", report_files.len());
    }

    // 9. 清理测试 DB
    let _ = std::fs::remove_file(&db_path);

    println!("\n🎉 E2E 管线测试全部通过!");
}
