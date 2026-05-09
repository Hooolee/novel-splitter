# AI 集成规范

本文档覆盖 LLM API 调用、流式响应、提示词管理以及 AI 分析流程。

## 架构概览

```
前端触发分析 → start_ai_analysis 命令 → 读取小说章节 →
合并内容 → 调用 LLM API → 流式响应 → ai-analysis 事件 →
前端解析 JSON → 保存元数据
```

## LLM API 调用

### 流式分析

```rust
async fn stream_ai_analysis(
    window: &tauri::Window,
    api_key: &str,
    base_url: &str,
    model: &str,
    prompt: &str,
    content: &str,
) -> Result<serde_json::Value, String> {
    // 1. 构建请求体
    let body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": prompt},
            {"role": "user", "content": content}
        ],
        "stream": true,
    });

    // 2. 发送流式请求
    let client = reqwest::Client::new();
    let response = client.post(format!("{}/chat/completions", base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&body)
        .send().await?;

    // 3. 读取 SSE 流
    let mut full_response = String::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let text = String::from_utf8_lossy(&chunk);
        // 解析 SSE 格式, 提取 content
        // 发送 ai-analysis 事件
        window.emit("ai-analysis", serde_json::json!({
            "content": delta_content,
        }))?;
        full_response.push_str(&delta_content);
    }

    // 4. 解析 JSON 响应
    let result: serde_json::Value = serde_json::from_str(&full_response)
        .map_err(|e| format!("JSON 解析失败: {}", e))?;
    Ok(result)
}
```

### 非流式调用

```rust
async fn call_llm(
    api_key: &str,
    base_url: &str,
    model: &str,
    messages: Vec<serde_json::Value>,
) -> Result<String, String> {
    let client = reqwest::Client::new();
    let response = client.post(format!("{}/chat/completions", base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&serde_json::json!({
            "model": model,
            "messages": messages,
            "stream": false,
        }))
        .send().await?;

    let data: serde_json::Value = response.json().await?;
    let content = data["choices"][0]["message"]["content"]
        .as_str()
        .ok_or("无效响应格式")?;
    Ok(content.to_string())
}
```

## 提示词管理

### 默认提示词

AI 分析提示词定义在 `ai.rs` 中，用户可在设置中自定义：

```
你是一个小说分析专家。分析以下小说内容，提取元数据。
请以 JSON 格式返回：
{
  "genre": "题材分类",
  "style": "写作风格",
  "goldfinger": "金手指/核心卖点",
  "opening": "故事开头简介",
  "highlights": "核心看点"
}
```

### 上下文控制

- 限制输入长度：3000-15000 字
- 默认读取前 5 章
- 长章节自动截断

## 事件通信

| 事件名 | 触发时机 | 数据 |
|--------|----------|------|
| `ai-analysis-status` | 开始/完成/错误 | `{ status: "start"|"complete"|"error", message }` |
| `ai-analysis` | 流式传输中 | `{ content: "片段文本" }` |

## 自动分析

下载完成后自动触发 AI 分析（若已配置 API）：

```
1. 下载完成 → 发送完成事件
2. 前端 autoAnalyze() 检测到 API 已配置
3. 调用 start_ai_analysis 命令
4. 读取前 N 章 → 合并内容 → 调用 LLM
5. 流式解析 JSON 响应
6. 通过 update_novel_metadata 保存结果
7. 刷新 UI 显示
```

---

## Multi-Agent 分歧驱动评估（任务二）

### 1. Scope / Trigger

- 触发场景：流水线 `run_full_analysis_pipeline` 末尾 Phase 4 自动调用 + 前端手动调 `evaluate_novel(novel_id)` 单本重跑
- 触发原因：跨层契约——`novels.ai_reviews_json` 是新数据契约，前端书库 / 情报大盘将消费该 JSON
- 跨层影响：DB 字段约定（无 migration，已预留）、新 Tauri 命令、前端展示（任务三/四消费）

### 2. Signatures

```rust
// ai.rs
pub async fn multi_agent_review(
    config: AiConfig,
    title: &str,
    tags: &str,
    outline_blob: &str,
    input_chapters: usize,
) -> Result<String, String>;
```

```rust
// lib.rs — Tauri 命令
#[tauri::command]
async fn evaluate_novel(app: tauri::AppHandle, novel_id: i64) -> Result<String, String>;
```

```rust
// db.rs — 评估专用 helper
pub fn load_novel_for_review(
    conn: &Connection,
    novel_id: i64,
) -> rusqlite::Result<(String, String, String, usize)>;
//   返回 (title, tags, outline_blob, chapter_count)

pub fn update_ai_reviews(
    conn: &Connection,
    novel_id: i64,
    reviews_json: &str,
) -> rusqlite::Result<()>;
//   同时刷新 updated_at = CURRENT_TIMESTAMP
```

DB 字段（已存在于 `novels` 表，无 migration 必要）：

```sql
ai_reviews_json TEXT NULL  -- JSON 字符串，schema 见 §3
```

### 3. Contracts

#### 输出 JSON Schema（写入 `novels.ai_reviews_json`）

```json
{
  "agents": {
    "reader":  { "vote": "yes|no|maybe", "focus": ["...","...","..."], "comment": "..." } | null,
    "editor":  { "vote": "...",          "focus": [...],               "comment": "..." } | null,
    "author":  { "vote": "...",          "focus": [...],               "comment": "..." } | null
  },
  "consensus": "all_yes" | "all_no" | "majority_yes" | "majority_no" | "divergent",
  "meta": {
    "model": "<string, 非空>",
    "generated_at": "<ISO-8601, chrono::Utc::now().to_rfc3339()>",
    "input_chapters": <usize, >= 1>
  }
}
```

#### Agent 子对象规约

| 字段 | 类型 | 约束 |
|------|------|------|
| `vote` | string | enum: `"yes"` / `"no"` / `"maybe"` |
| `focus` | string[] | 长度 3，每条 8-15 字符 |
| `comment` | string | ≤ 30 字符 |

单 Agent 失败 → 整个对象置为 `null`，其余 Agent 仍写入。

#### Consensus 计算规则（`compute_consensus`）

仅统计 `vote ∈ {yes, no}` 的票数（maybe / 失败 Agent 不计入 yes/no 计数，但参与 total）：

| 输入 | 输出 |
|------|------|
| 三票全 yes | `all_yes` |
| 三票全 no | `all_no` |
| yes > no | `majority_yes` |
| no > yes | `majority_no` |
| 平票 / 全 maybe / 全失败 | `divergent` |

#### 输入要求（`outline_blob`）

`load_novel_for_review` 产出格式：

```text
## 第1章 章节标题
[chapters.outline_json 原文]

## 第2章 章节标题
[...]
```

**调用方必须**在调用 `multi_agent_review` 前完成 ~6000 chars 截断（按字符边界，避免切坏中文）：

```rust
let truncated: String = if blob.chars().count() > 6000 {
    blob.chars().take(6000).collect()
} else {
    blob
};
```

### 4. Validation & Error Matrix

| 条件 | 行为 |
|------|------|
| AI 配置缺失（`GlobalAiConfig` 为 `None`） | `evaluate_novel` 返回 `Err("AI 配置未设置...")` |
| `novel_id` 不存在 | `load_novel_for_review` → `QueryReturnedNoRows`，上层包装 `Err("未找到 novel_id={}...")` |
| 该书无任何 outline_json（chapter_count == 0） | 调用方返回 `Err("该书没有 outline_json...")` |
| 单 Agent JSON 解析失败 | `parse_agent_response` 返回 None；该 agent 字段为 `null`，eprintln 警告 |
| 单 Agent HTTP 调用失败 | 同上 |
| 三 Agent 全失败 | `multi_agent_review` 返回 `Err("三 Agent 全部失败")`；调用方跳过 UPDATE，保留旧值 |

### 5. Good / Base / Bad Cases

| 等级 | 场景 | 结果 |
|------|------|------|
| **Good** | 三 Agent 全成功且 vote 一致 | `consensus=all_yes\|all_no`，`ai_reviews_json` 写入，`updated_at` 刷新 |
| **Base** | 单 Agent 解析失败 | 该 agent 字段 `null`，consensus 按剩余 vote 算，仍写入 |
| **Bad** | 三 Agent 全失败（超时 / 返回非 JSON） | 返回 Err，**不写半成品**，`ai_reviews_json` 维持原值 |

### 6. Tests Required

#### 单元测试（`ai.rs::tests`，`cargo test --lib ai::tests`）

- `compute_consensus`：全枚举覆盖（all_yes / all_no / majority_yes / majority_no / divergent / 单 Agent 失败兜底 / 三 Agent 全失败）
- `parse_agent_response`：合法 JSON / 非法 JSON / 调用失败 三种入参

#### E2E（`pipeline_e2e.rs`，`cargo run --bin pipeline_e2e`）

注入 chapter outline_json，调真实 LLM 触发 `multi_agent_review`，断言：

- `agents.{reader|editor|author}` 字段存在（值可为 null）
- `consensus ∈ {all_yes, all_no, majority_yes, majority_no, divergent}`
- `meta.model` 非空、`meta.input_chapters >= 1`
- `update_ai_reviews` 写入后 DB 回读 == 原 JSON

### 7. Wrong vs Correct

#### Wrong: 串行调用三 Agent

```rust
// ❌ 慢 3 倍，无理由串行；单 Agent 失败用 ? 直接 short-circuit，丢失其他 Agent 的输出
let r = call_ai(cfg.clone(), READER_PROMPT.into(), content.clone(), true).await?;
let e = call_ai(cfg.clone(), EDITOR_PROMPT.into(), content.clone(), true).await?;
let a = call_ai(cfg, AUTHOR_PROMPT.into(), content, true).await?;
```

#### Correct: `tokio::join!` 三路并行 + fail-soft

```rust
// ✅ 三路并发，单路失败不影响其他；最终全失败才 Err
let (reader_res, editor_res, author_res) = tokio::join!(
    call_ai(cfg_a, READER_PROMPT.to_string(), content_a, true),
    call_ai(cfg_b, EDITOR_PROMPT.to_string(), content_b, true),
    call_ai(cfg_c, AUTHOR_PROMPT.to_string(), content_c, true),
);

let reader_obj = parse_agent_response(reader_res, "reader");
let editor_obj = parse_agent_response(editor_res, "editor");
let author_obj = parse_agent_response(author_res, "author");

if reader_obj.is_none() && editor_obj.is_none() && author_obj.is_none() {
    return Err("三 Agent 全部失败".into());
}
```

#### Wrong: 直接 SQL 拼接更新 `ai_reviews_json`

```rust
// ❌ 引号转义、注入风险、丢失 updated_at 刷新
conn.execute(
    &format!("UPDATE novels SET ai_reviews_json = '{}'", reviews_json),
    [],
)?;
```

#### Correct: 走封装 helper

```rust
// ✅ 参数化绑定 + 自动刷新 updated_at
crate::db::update_ai_reviews(&conn, novel_id, &reviews_json)?;
```

#### Wrong: 跨 await 持有 `Connection`

```rust
// ❌ rusqlite::Connection 不是 Send；跨 await 会编译失败或 panic
let conn = crate::db::get_conn()?;
let blob = build_blob(&conn);
let result = ai::multi_agent_review(cfg, ..., &blob, n).await?;  // conn 仍在作用域
db::update_ai_reviews(&conn, id, &result)?;
```

#### Correct: 用完即释，重新 open

```rust
// ✅ 在 await 之前 drop conn；await 之后再 get_conn 一次（SQLite 单文件，开销可忽略）
let blob = {
    let conn = crate::db::get_conn()?;
    build_blob(&conn)
};
let result = ai::multi_agent_review(cfg, ..., &blob, n).await?;
let conn = crate::db::get_conn()?;
db::update_ai_reviews(&conn, id, &result)?;
```

> 实际实现走的是另一种模式：把整段「load → call → update」放在同一个 `tokio::spawn` 任务里（`run_multi_agent_phase`），任务内部连续持有 conn 也不跨 spawn 边界，照样安全。

---

## 最佳实践

1. **流式优先** - 长文本分析使用流式避免超时
2. **错误恢复** - 网络错误重试，API 错误提示用户
3. **上下文控制** - 控制输入长度以控制成本和性能
4. **JSON 解析** - LLM 输出 JSON 可能不标准，需要容错解析
5. **多个模型** - 支持 OpenAI、DeepSeek 等兼容 API

## 常见陷阱

- API 密钥未配置 → 设置中明确提示
- JSON 解析失败 → LLM 输出不标准时的降级处理
- 上下文超长 → 合理截断输入内容
- 流式中断 → 重连或提示用户重试
- 模型不可用 → 检查 base_url 和模型名正确性
