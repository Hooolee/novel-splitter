use serde::{Deserialize, Serialize};
use reqwest::Client;
use tauri::Emitter;
use futures::StreamExt;
use std::sync::Mutex;

#[derive(Serialize, Deserialize, Clone)]
pub struct AiConfig {
    pub api_base: String,
    pub api_key: String,
    pub model: String,
}

/// Tauri 全局状态：AI 配置（由前端 UI 设置）
pub struct GlobalAiConfig(pub Mutex<Option<AiConfig>>);

/// Tauri 全局状态：工作目录（由前端 UI 设置，默认项目根）
pub struct GlobalWorkspaceRoot(pub Mutex<String>);

#[derive(Serialize, Clone)]
struct AiStreamPayload {
    chunk: String,
}

#[derive(Serialize, Clone)]
pub struct Progress {
    pub message: String,
    pub status: String,
}

pub async fn stream_analysis(
    app: tauri::AppHandle,
    config: AiConfig,
    prompt: String,
    content: String,
    response_json: bool,
) -> Result<(), String> {
    
    let client = Client::new();

    let mut body = serde_json::json!({
        "model": config.model,
        "messages": [
            {"role": "system", "content": prompt.clone()},
            {"role": "user", "content": content.clone()}
        ],
        "stream": true,
        "temperature": 0.7
    });

    // 需要强制 JSON 时才附加 response_format
    if response_json {
        body["response_format"] = serde_json::json!({ "type": "json_object" });
    }

    // Ensure api_base doesn't double slash
    let base = config.api_base.trim_end_matches('/');
    let url = if base.ends_with("/chat/completions") {
        base.to_string()
    } else {
        format!("{}/chat/completions", base)
    };

    let prompt_preview: String = prompt.chars().take(150).collect();
    let content_preview: String = content.chars().take(100).collect();

    // --- 新增详细日志打印 ---
    println!("\\n==================== AI REQUEST LOG ====================");
    println!("URL: {}", url);
    println!("Model: {}", config.model);
    println!("System Prompt (preview): {}...", prompt_preview);
    println!("User Content (preview): {}...", content_preview);
    println!("Response JSON required: {}", response_json);
    println!("========================================================\\n");

    let _ = app.emit("ai-analysis-status", Progress {
        message: format!("Connecting to AI at {}...", url),
        status: "start".to_string()
    });

    let response = client.post(&url)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let err_text = response.text().await.unwrap_or_default();
        println!("API ERROR: {} - {}", status, err_text);
        return Err(format!("API Error {}: {}", status, err_text));
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut is_first = true;

    while let Some(item) = stream.next().await {
        let chunk = item.map_err(|e| e.to_string())?;
        let s = String::from_utf8_lossy(&chunk);
        
        if is_first {
            println!("--- AI FIRST CHUNK RECEIVED ---");
            println!("{}", s);
            println!("-------------------------------");
            is_first = false;
        }

        buffer.push_str(&s);

        // Simple SSE parser
        // Look for "data: " lines. Handle split chunks by only processing full lines.
        while let Some(idx) = buffer.find('\n') {
            let line = buffer[..idx].to_string();
            buffer = buffer[idx+1..].to_string();
            
            let trimmed = line.trim();
            if trimmed.starts_with("data: ") {
                let data = &trimmed[6..];
                if data == "[DONE]" {
                    continue;
                }
                
                // Parse JSON
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                    // OpenAI format: choices[0].delta
                    if let Some(delta) = json.get("choices").and_then(|c| c.get(0)).and_then(|c| c.get("delta")) {
                        let mut chunk_text = String::new();
                        
                        // DeepSeek-R1 reasoning content
                        if let Some(reasoning) = delta.get("reasoning_content").and_then(|c| c.as_str()) {
                            chunk_text.push_str(reasoning);
                        }
                        
                        // Standard content
                        if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                            chunk_text.push_str(content);
                        }

                        if !chunk_text.is_empty() {
                            let _ = app.emit("ai-analysis", AiStreamPayload {
                                chunk: chunk_text
                            });
                        }
                    } 
                } else {
                    println!("Warning: Failed to parse SSE JSON data chunk: {}", data);
                }
            }
        }
    }
    
     let _ = app.emit("ai-analysis-status", Progress {
        message: "Analysis Complete".to_string(),
        status: "done".to_string()
    });

    Ok(())
}

pub async fn fetch_models(config: AiConfig) -> Result<Vec<String>, String> {
    let client = Client::new();
    
    // Ensure api_base doesn't double slash
    let base = config.api_base.trim_end_matches('/');
    // Check if user provided full path or just base
    // Usually /v1/models or just /models depending on provider
    // Heuristic: try assuming standard OpenAI style first
    let url = if base.ends_with("/chat/completions") {
        base.replace("/chat/completions", "/models")
    } else {
        format!("{}/models", base)
    };

    let response = client.get(&url)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !response.status().is_success() {
       return Err(format!("API Error {}", response.status()));
    }

    // Parse as generic JSON Value to handle different formats
    let json: serde_json::Value = response.json().await
        .map_err(|e| format!("Parse error: {}", e))?;

    // Try standard "data": [...]
    if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
        let models: Vec<String> = data.iter()
            .filter_map(|m| m.get("id").and_then(|id| id.as_str()).map(|s| s.to_string()))
            .collect();
        return Ok(models);
    }
    
    Err(format!("Unknown response format: {:?}", json))
}

pub async fn call_ai(
    config: AiConfig,
    prompt: String,
    content: String,
    response_json: bool,
) -> Result<String, String> {
    let client = Client::new();
    let mut body = serde_json::json!({
        "model": config.model,
        "messages": [
            {"role": "system", "content": prompt},
            {"role": "user", "content": content}
        ],
        "temperature": 0.7
    });

    if response_json {
        body["response_format"] = serde_json::json!({ "type": "json_object" });
    }

    let base = config.api_base.trim_end_matches('/');
    let url = if base.ends_with("/chat/completions") {
        base.to_string()
    } else {
        format!("{}/chat/completions", base)
    };

    let response = client.post(&url)
        .header("Authorization", format!("Bearer {}", config.api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let err_text = response.text().await.unwrap_or_default();
        return Err(format!("API Error {}: {}", status, err_text));
    }

    let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    let content = json.get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|s| s.as_str())
        .ok_or_else(|| "Failed to get content from AI response".to_string())?;

    Ok(content.to_string())
}

// ============================================================================
//  Multi-Agent Review (任务二) — 三视角分歧驱动评估
// ============================================================================

/// 毒舌读者：站在普通读者立场，挑剔追读欲望与套路爽感
const READER_PROMPT: &str = r#"你是一个挑剔到苛刻的网文老读者，喜欢吐槽，对套路免疫，对开篇拖沓零容忍。
请基于下方提供的小说细纲与元数据，从【普通读者会不会追读下去】这个角度评判。

输出严格的 JSON 对象（无任何额外文字、不要 Markdown 代码块、不要解释）：
{
  "vote": "yes|no|maybe",
  "focus": ["...", "...", "..."],
  "comment": "..."
}

字段要求：
- vote: yes=会追读 / no=明确弃书 / maybe=可能再看几章
- focus: 三条犀利具体的槽点或亮点（每条 8-15 字）
- comment: 一句话主观感受，不超过 30 字

只输出该 JSON。"#;

/// 资深主编：从签约/推荐角度判断市场潜力
const EDITOR_PROMPT: &str = r#"你是一个网文平台的资深签约主编，每天浏览数十本投稿，擅长判断市场潜力与商业化空间。
请基于下方小说细纲与元数据，从【是否值得签约推荐】这个角度评判。

输出严格的 JSON 对象（无任何额外文字、不要 Markdown 代码块、不要解释）：
{
  "vote": "yes|no|maybe",
  "focus": ["...", "...", "..."],
  "comment": "..."
}

字段要求：
- vote: yes=值得签约推 / no=不予签约 / maybe=待观察
- focus: 三条市场判断要点（题材热度、卖点辨识、节奏问题等，每条 8-15 字）
- comment: 一句话商业判断，不超过 30 字

只输出该 JSON。"#;

/// 白金作者：从写作技法层面挑剔评估
const AUTHOR_PROMPT: &str = r#"你是一个白金大神级别的网文作者，看过无数扑街与爆款，从写作技法层面挑剔。
请基于下方小说细纲与元数据，从【写作技术上是否合格】这个角度评判。

输出严格的 JSON 对象（无任何额外文字、不要 Markdown 代码块、不要解释）：
{
  "vote": "yes|no|maybe",
  "focus": ["...", "...", "..."],
  "comment": "..."
}

字段要求：
- vote: yes=技法过关 / no=技法明显粗糙 / maybe=部分合格
- focus: 三条技术性观察（伏笔、人物、节奏、视角等，每条 8-15 字）
- comment: 一句话技术评价，不超过 30 字

只输出该 JSON。"#;

/// 三视角 Multi-Agent 评估。
///
/// 输出 JSON schema（落入 novels.ai_reviews_json）：
/// ```json
/// {
///   "agents": {
///     "reader": { "vote": "yes|no|maybe", "focus": ["…","…","…"], "comment": "…" } | null,
///     "editor": { ... } | null,
///     "author": { ... } | null
///   },
///   "consensus": "all_yes | all_no | majority_yes | majority_no | divergent",
///   "meta": { "model": "<string>", "generated_at": "<ISO-8601>", "input_chapters": <int> }
/// }
/// ```
///
/// 失败语义：
/// - 单 Agent JSON 解析失败 / 调用失败 → 该 agent 字段为 `null`，其余正常写入
/// - 三 Agent 全失败 → 返回 `Err`，调用方应跳过 UPDATE
pub async fn multi_agent_review(
    config: AiConfig,
    title: &str,
    tags: &str,
    outline_blob: &str,
    input_chapters: usize,
) -> Result<String, String> {
    let user_content = format!(
        "## 元数据\n书名: {}\n标签: {}\n\n## 章节细纲\n{}",
        title, tags, outline_blob
    );

    let cfg_a = config.clone();
    let cfg_b = config.clone();
    let cfg_c = config.clone();
    let content_a = user_content.clone();
    let content_b = user_content.clone();
    let content_c = user_content;

    let (reader_res, editor_res, author_res) = tokio::join!(
        call_ai(cfg_a, READER_PROMPT.to_string(), content_a, true),
        call_ai(cfg_b, EDITOR_PROMPT.to_string(), content_b, true),
        call_ai(cfg_c, AUTHOR_PROMPT.to_string(), content_c, true),
    );

    let reader_obj = parse_agent_response(reader_res, "reader");
    let editor_obj = parse_agent_response(editor_res, "editor");
    let author_obj = parse_agent_response(author_res, "author");

    if reader_obj.is_none() && editor_obj.is_none() && author_obj.is_none() {
        return Err("三 Agent 全部失败".to_string());
    }

    let consensus = compute_consensus(
        reader_obj.as_ref(),
        editor_obj.as_ref(),
        author_obj.as_ref(),
    );

    let meta = serde_json::json!({
        "model": config.model,
        "generated_at": chrono::Utc::now().to_rfc3339(),
        "input_chapters": input_chapters,
    });

    let final_json = serde_json::json!({
        "agents": {
            "reader": reader_obj.unwrap_or(serde_json::Value::Null),
            "editor": editor_obj.unwrap_or(serde_json::Value::Null),
            "author": author_obj.unwrap_or(serde_json::Value::Null),
        },
        "consensus": consensus,
        "meta": meta,
    });

    serde_json::to_string(&final_json)
        .map_err(|e| format!("最终 JSON 序列化失败: {}", e))
}

/// 解析单个 Agent 的返回。LLM 返回非 JSON / 调用失败 → None。
fn parse_agent_response(
    res: Result<String, String>,
    agent_name: &str,
) -> Option<serde_json::Value> {
    match res {
        Ok(s) => match serde_json::from_str::<serde_json::Value>(&s) {
            Ok(v) => Some(v),
            Err(e) => {
                let preview: String = s.chars().take(120).collect();
                eprintln!(
                    "[Multi-Agent] ⚠️ {} JSON 解析失败: {} (raw: {})",
                    agent_name, e, preview
                );
                None
            }
        },
        Err(e) => {
            eprintln!("[Multi-Agent] ❌ {} 调用失败: {}", agent_name, e);
            None
        }
    }
}

/// 由三 Agent 的 vote 字段计算 consensus。
///
/// 规则（仅统计有效 vote = yes/no；maybe / 解析失败 / 缺字段都视作非 y/n）：
/// - 三票全 yes → all_yes
/// - 三票全 no → all_no
/// - yes 多 → majority_yes
/// - no 多 → majority_no
/// - 其余（含全 maybe / 平票 / 全部无效） → divergent
fn compute_consensus(
    reader: Option<&serde_json::Value>,
    editor: Option<&serde_json::Value>,
    author: Option<&serde_json::Value>,
) -> &'static str {
    let votes: Vec<&str> = [reader, editor, author]
        .iter()
        .flat_map(|a| a.as_ref())
        .filter_map(|v| v.get("vote").and_then(|s| s.as_str()))
        .collect();

    if votes.is_empty() {
        return "divergent";
    }

    let yes = votes.iter().filter(|&&v| v == "yes").count();
    let no = votes.iter().filter(|&&v| v == "no").count();
    let total = votes.len();

    if yes == total {
        return "all_yes";
    }
    if no == total {
        return "all_no";
    }
    if yes > no {
        return "majority_yes";
    }
    if no > yes {
        return "majority_no";
    }
    "divergent"
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn consensus_all_yes() {
        let r = json!({"vote":"yes","focus":[],"comment":""});
        let e = json!({"vote":"yes","focus":[],"comment":""});
        let a = json!({"vote":"yes","focus":[],"comment":""});
        assert_eq!(compute_consensus(Some(&r), Some(&e), Some(&a)), "all_yes");
    }

    #[test]
    fn consensus_all_no() {
        let r = json!({"vote":"no"});
        let e = json!({"vote":"no"});
        let a = json!({"vote":"no"});
        assert_eq!(compute_consensus(Some(&r), Some(&e), Some(&a)), "all_no");
    }

    #[test]
    fn consensus_majority_yes() {
        let r = json!({"vote":"yes"});
        let e = json!({"vote":"yes"});
        let a = json!({"vote":"no"});
        assert_eq!(compute_consensus(Some(&r), Some(&e), Some(&a)), "majority_yes");
    }

    #[test]
    fn consensus_majority_no() {
        let r = json!({"vote":"no"});
        let e = json!({"vote":"no"});
        let a = json!({"vote":"yes"});
        assert_eq!(compute_consensus(Some(&r), Some(&e), Some(&a)), "majority_no");
    }

    #[test]
    fn consensus_divergent_with_maybe() {
        // 两 maybe + 一 yes：yes=1, no=0, total(=有效 vote 字符串 yes/no 计数=1) → yes>no → majority_yes
        // 注意：上面规则只统计 yes/no 字符串；maybe 不计 yes/no 但仍算入 votes 总数
        let r = json!({"vote":"maybe"});
        let e = json!({"vote":"maybe"});
        let a = json!({"vote":"yes"});
        // votes = ["maybe","maybe","yes"], total=3, yes=1, no=0 → yes>no → majority_yes
        assert_eq!(compute_consensus(Some(&r), Some(&e), Some(&a)), "majority_yes");
    }

    #[test]
    fn consensus_divergent_split() {
        // yes=1, no=1, maybe=1 → 平票 → divergent
        let r = json!({"vote":"yes"});
        let e = json!({"vote":"no"});
        let a = json!({"vote":"maybe"});
        assert_eq!(compute_consensus(Some(&r), Some(&e), Some(&a)), "divergent");
    }

    #[test]
    fn consensus_one_failed_agent_majority() {
        // editor 失败（None），其余两人 yes
        let r = json!({"vote":"yes"});
        let a = json!({"vote":"yes"});
        assert_eq!(compute_consensus(Some(&r), None, Some(&a)), "all_yes");
    }

    #[test]
    fn consensus_all_failed_returns_divergent() {
        assert_eq!(compute_consensus(None, None, None), "divergent");
    }

    #[test]
    fn parse_agent_response_ok() {
        let res = Ok(r#"{"vote":"yes","focus":["a","b","c"]}"#.to_string());
        let v = parse_agent_response(res, "reader");
        assert!(v.is_some());
        assert_eq!(v.unwrap()["vote"], "yes");
    }

    #[test]
    fn parse_agent_response_invalid_json() {
        let res = Ok("not a json".to_string());
        let v = parse_agent_response(res, "reader");
        assert!(v.is_none());
    }

    #[test]
    fn parse_agent_response_call_failed() {
        let res: Result<String, String> = Err("network".to_string());
        let v = parse_agent_response(res, "reader");
        assert!(v.is_none());
    }
}
