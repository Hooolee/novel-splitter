use serde::{Deserialize, Serialize};
use reqwest::Client;
use tauri::Emitter;
use futures::StreamExt;

#[derive(Serialize, Deserialize)]
pub struct AiConfig {
    pub api_base: String,
    pub api_key: String,
    pub model: String,
}

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
            {"role": "system", "content": prompt},
            {"role": "user", "content": content}
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
        return Err(format!("API Error {}: {}", status, err_text));
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    while let Some(item) = stream.next().await {
        let chunk = item.map_err(|e| e.to_string())?;
        let s = String::from_utf8_lossy(&chunk);
        buffer.push_str(&s);

        // Simple SSE parser
        // Look for "data: " lines. Handle split chunks by only processing full lines.
        while let Some(idx) = buffer.find('\n') {
            let line = buffer[..idx].to_string();
            buffer.remove(0); // Remove leading chars... carefully. 
            // Better: split off
            buffer = buffer[idx+1..].to_string();
            
            let trimmed = line.trim();
            if trimmed.starts_with("data: ") {
                let data = &trimmed[6..];
                if data == "[DONE]" {
                    continue;
                }
                
                // Parse JSON
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                    // OpenAI format: choices[0].delta.content
                    if let Some(delta) = json.get("choices").and_then(|c| c.get(0)).and_then(|c| c.get("delta")) {
                         if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                            let _ = app.emit("ai-analysis", AiStreamPayload {
                                chunk: content.to_string()
                            });
                         }
                    } 
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
