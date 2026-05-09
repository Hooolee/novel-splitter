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
