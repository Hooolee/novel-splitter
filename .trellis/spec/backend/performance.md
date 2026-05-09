# 性能优化模式

本文档覆盖 Rust 后端性能优化模式：异步并发、请求控制、批量处理。

## 异步并发

### 使用 tokio 进行异步操作

```rust
use tokio::task;

// 并行执行独立任务
let (info, chapters) = tokio::join!(
    spider.scrape_novel_info(url),
    spider.scrape_chapters(url, count),
);
```

### 同时下载多个章节

```rust
let mut handles = vec![];
for (i, chapter_url) in chapter_urls.iter().enumerate() {
    let client = client.clone();
    let url = chapter_url.clone();
    handles.push(task::spawn(async move {
        let html = client.get(&url).send().await?;
        // 解析章节内容
        Ok::<Chapter, String>(parsed)
    }));
}

for handle in handles {
    let chapter = handle.await.map_err(|e| e.to_string())??;
    chapters.push(chapter);
}
```

## 并发控制

### 使用 Semaphore 限制并发

防止请求过快被网站封禁：

```rust
use tokio::sync::Semaphore;
use std::sync::Arc;

// 创建信号量，最多 5 个并发
let semaphore = Arc::new(Semaphore::new(5));

let mut handles = vec![];
for url in &chapter_urls {
    let permit = semaphore.clone().acquire_owned().await;
    let url = url.clone();
    handles.push(task::spawn(async move {
        let _permit = permit;
        // 执行带并发限制的请求
        let html = reqwest::get(&url).send().await?;
        // ...
    }));
}
```

### 请求间隔

```rust
use tokio::time::{sleep, Duration};

// 在请求之间添加延迟
for chapter_url in &chapter_urls {
    let html = client.get(chapter_url).send().await?;
    // 处理...
    sleep(Duration::from_millis(200)).await; // 200ms 间隔
}
```

## 重试机制

### 带指数退避的重试

```rust
async fn fetch_with_retry(url: &str, max_retries: u32) -> Result<String, String> {
    let mut last_err = String::new();
    for attempt in 1..=max_retries {
        match reqwest::get(url).await {
            Ok(resp) => return resp.text().await.map_err(|e| e.to_string()),
            Err(e) => {
                last_err = e.to_string();
                if attempt < max_retries {
                    let delay = 2u64.pow(attempt) * 1000; // 2^attempt 秒
                    sleep(Duration::from_millis(delay)).await;
                }
            }
        }
    }
    Err(format!("重试 {} 次后仍然失败: {}", max_retries, last_err))
}
```

## 内存优化

### 流式写入文件

爬取大量章节时避免全部保存在内存中：

```rust
use tokio::io::AsyncWriteExt;

async fn download_chapter_to_file(
    client: &reqwest::Client,
    url: &str,
    file_path: &std::path::Path,
) -> Result<(), String> {
    let response = client.get(url).send().await.map_err(|e| e.to_string())?;
    let mut file = tokio::fs::File::create(file_path).await.map_err(|e| e.to_string())?;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = futures_util::StreamExt::next(&mut stream).await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        file.write_all(&chunk).await.map_err(|e| e.to_string())?;
    }
    Ok(())
}
```

## 批量处理模式

### 分批处理榜单小说

```rust
async fn batch_download(urls: Vec<String>, batch_size: usize) -> Result<(), String> {
    for chunk in urls.chunks(batch_size) {
        let mut handles = vec![];
        for url in chunk {
            let url = url.clone();
            handles.push(task::spawn(async move {
                download_single_novel(&url).await
            }));
        }
        for handle in handles {
            handle.await.map_err(|e| e.to_string())??;
        }
        // 批次间休息
        sleep(Duration::from_secs(5)).await;
    }
    Ok(())
}
```

## 最佳实践

1. **tokio::spawn** - CPU 密集或长时间 IO 操作在独立任务中执行
2. **并发限制** - 爬虫请求必须控制并发数，避免被封
3. **超时设置** - 所有 HTTP 请求设置超时
4. **重试策略** - 临时错误自动重试
5. **资源清理** - tokio task 正确管理生命周期

## 常见陷阱

- 无限制并发 → 使用 Semaphore 控制
- 同步阻塞 → 所有 IO 操作必须用 async
- 请求过快 → 添加间隔或限制并发
- 忽略超时 → 长时间无响应的请求会挂起
- 文件句柄泄漏 → 使用 RAII 模式自动关闭
