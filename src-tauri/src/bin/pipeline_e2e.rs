//! E2E 管线集成测试
//!
//! 测试模式：
//!   1. 完整模式（需 App 运行）: 启动后在前端「报告」Tab 扫榜
//!   2. 离线模式（本 binary）: 跳过蜘蛛，测试 DB + AI Worker
//!
//! 离线模式运行:
//!   cd src-tauri && cargo run --bin pipeline_e2e
//!   使用 test.env 或环境变量设置 AI_API_BASE / AI_API_KEY / AI_MODEL

use std::sync::Mutex;
use tauri::Manager;

fn main() {
    load_test_env();

    let api_base = std::env::var("AI_API_BASE")
        .expect("需要 AI_API_BASE (设置于 test.env 或环境变量)");
    let api_key = std::env::var("AI_API_KEY")
        .expect("需要 AI_API_KEY (设置于 test.env 或环境变量)");
    let model = std::env::var("AI_MODEL")
        .expect("需要 AI_MODEL (设置于 test.env 或环境变量)");

    // 1. 构建 Tauri App（用于 Tauri State + 事件）
    let app = tauri::Builder::default()
        .build(tauri::generate_context!())
        .expect("构建 Tauri App 失败");
    let handle = app.handle();

    handle.manage(fanqie_app_lib::ai::GlobalAiConfig(Mutex::new(
        Some(fanqie_app_lib::ai::AiConfig { api_base, api_key, model })
    )));

    let project_root = fanqie_app_lib::get_project_root();
    let db_path = project_root.join("test_pipeline_e2e.db");
    let _ = std::fs::remove_file(&db_path);
    fanqie_app_lib::db::init_db(&db_path).expect("DB 初始化失败");

    // 2. 测试断点续跑：手动插入一条测试数据
    eprintln!("\n--- 测试 1: DB 写/读 ---");
    {
        let conn = fanqie_app_lib::db::get_conn().unwrap();
        let nid = fanqie_app_lib::db::upsert_novel(
            &conn, "test_book_001", "qidian", "测试小说", "测试作者", "玄幻,系统", 0,
        ).expect("upsert_novel 失败");
        fanqie_app_lib::db::upsert_chapter(
            &conn, nid, 1, "第一章测试",
            "这是一个测试章节内容，用于验证 AI 提纯管线可以正常调用并返回 JSON 格式的细纲。",
            None,
        ).expect("upsert_chapter 失败");
        eprintln!("  ✅ 写入测试章节");
    }

    // 3. 测试 AI Worker（Phase 3）
    eprintln!("\n--- 测试 2: AI Worker (outline_json 提纯) ---");
    {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async {
            let state = handle.state::<fanqie_app_lib::ai::GlobalAiConfig>();
            let guard = state.0.lock().unwrap();
            let config = guard.clone().unwrap();
            drop(guard);
            fanqie_app_lib::ai::call_ai(config,
                "你是一个章节细纲提取助手。将以下内容拆解为 JSON 数组：[{\"event\":\"\",\"purpose\":\"\",\"emotion\":\"\",\"highlight\":\"\"}]".to_string(),
                "这是一个测试章节内容，主角在街头遇到神秘老人，老人递给他一枚古玉后消失。".to_string(), true).await
        });

        match result {
            Ok(json) => {
                let preview: String = json.chars().take(200).collect();
                eprintln!("  ✅ AI 调用成功，返回 JSON:\n  {}", preview);
            },
            Err(e) => eprintln!("  ⚠️  AI 调用失败 (可能 API 未就绪): {}", e),
        }
    }

    // 4. 测试 Multi-Agent Review (Phase 4)
    eprintln!("\n--- 测试 3: Multi-Agent Review (任务二) ---");
    {
        // 先给测试章节注入一个 outline_json，让 multi_agent_review 有可用上下文
        let conn = fanqie_app_lib::db::get_conn().unwrap();
        conn.execute(
            "UPDATE chapters SET outline_json = ?1 WHERE chapter_index = 1",
            [r#"[{"event":"主角街头偶遇神秘老人","purpose":"展示金手指","emotion":"悬疑","highlight":"古玉来历不明"}]"#],
        ).expect("注入 outline_json 失败");

        let nid: i64 = conn.query_row(
            "SELECT id FROM novels WHERE book_id = 'test_book_001'",
            [],
            |row| row.get(0),
        ).expect("查询测试 novel 失败");

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async {
            let config = {
                let state = handle.state::<fanqie_app_lib::ai::GlobalAiConfig>();
                let guard = state.0.lock().unwrap();
                guard.clone().unwrap()
            };

            let conn2 = fanqie_app_lib::db::get_conn().unwrap();
            let (title, tags, blob, chapters) =
                fanqie_app_lib::db::load_novel_for_review(&conn2, nid)
                    .expect("load_novel_for_review 失败");
            assert!(chapters >= 1, "测试 novel 至少需要 1 章 outline_json");

            fanqie_app_lib::ai::multi_agent_review(config, &title, &tags, &blob, chapters).await
        });

        match result {
            Ok(json) => {
                let preview: String = json.chars().take(200).collect();
                eprintln!("  ✅ Multi-Agent 返回 (preview): {}", preview);

                // Schema 校验
                let v: serde_json::Value =
                    serde_json::from_str(&json).expect("multi_agent_review JSON 解析失败");

                assert!(v.get("agents").is_some(), "缺 agents 字段");
                for k in &["reader", "editor", "author"] {
                    assert!(
                        v.pointer(&format!("/agents/{}", k)).is_some(),
                        "缺 agents.{} 字段",
                        k
                    );
                }
                let consensus = v
                    .get("consensus")
                    .and_then(|x| x.as_str())
                    .expect("缺 consensus 字段");
                assert!(
                    matches!(
                        consensus,
                        "all_yes" | "all_no" | "majority_yes" | "majority_no" | "divergent"
                    ),
                    "consensus 枚举非法: {}",
                    consensus
                );
                let model = v
                    .pointer("/meta/model")
                    .and_then(|x| x.as_str())
                    .expect("缺 meta.model 字段");
                assert!(!model.is_empty(), "meta.model 不能为空");
                let chs = v
                    .pointer("/meta/input_chapters")
                    .and_then(|x| x.as_u64())
                    .expect("缺 meta.input_chapters");
                assert!(chs >= 1, "input_chapters 至少为 1");
                eprintln!(
                    "  ✅ Schema 校验通过 consensus={} model={} chapters={}",
                    consensus, model, chs
                );

                // 顺便验证 update_ai_reviews 写入 + 回读
                fanqie_app_lib::db::update_ai_reviews(&conn, nid, &json)
                    .expect("update_ai_reviews 失败");
                let written: String = conn
                    .query_row(
                        "SELECT ai_reviews_json FROM novels WHERE id = ?1",
                        [nid],
                        |row| row.get(0),
                    )
                    .expect("回读 ai_reviews_json 失败");
                assert_eq!(written, json, "DB 回读 ai_reviews_json 与原 JSON 不一致");
                eprintln!("  ✅ ai_reviews_json 写入并回读校验通过");
            }
            Err(e) => eprintln!("  ⚠️  Multi-Agent 调用失败 (可能 API 未就绪): {}", e),
        }
    }

    // 5. 完整管线指引
    eprintln!("\n--- 测试 4: 完整管线 (需要 App 运行) ---");
    eprintln!("  蜘蛛在 headless 下不可用，完整 E2E 请运行:");
    eprintln!("  ┌─────────────────────────────────────────────────────────┐");
    eprintln!("  │  npm run tauri dev                                      │");
    eprintln!("  │  → 打开 App 后在左下角⚙️确认 AI 配置             │");
    eprintln!("  │  → 切到「报告」Tab → 选榜单 → 点「立即扫榜」  │");
    eprintln!("  │  → 观察终端输出 [Pipeline 1/4] [2/4] [3/4] [4/4]      │");
    eprintln!("  └─────────────────────────────────────────────────────────┘");

    // 6. 清理
    if std::env::var("SKIP_CLEANUP").is_err() {
        let _ = std::fs::remove_file(&db_path);
        eprintln!("\n✅ 测试数据已清理");
    }
}

fn load_test_env() {
    let env_path = {
        let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.pop();
        p.push("test.env");
        p
    };
    let content = match std::fs::read_to_string(&env_path) {
        Ok(c) => c,
        Err(_) => return,
    };
    let map = [
        ("api_key", "AI_API_KEY"),
        ("api_base_url", "AI_API_BASE"),
        ("api_model", "AI_MODEL"),
    ];
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        if let Some(eq) = line.find('=') {
            let key = line[..eq].trim();
            let val = line[eq+1..].trim();
            for &(src, dst) in &map {
                if key == src && std::env::var(dst).is_err() {
                    std::env::set_var(dst, val);
                }
            }
        }
    }
}
