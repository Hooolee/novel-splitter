use reqwest::Client;
use scraper::{Html, Selector};
use regex::Regex;
use crate::log_to_file;

// Helper to get debug directory path
fn get_debug_dir() -> std::path::PathBuf {
    // Try to find project root by looking for src-tauri directory
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            let mut current = parent;
            for _ in 0..5 {
                if current.join("src-tauri").exists() {
                    let debug_dir = current.join("debug");
                    if !debug_dir.exists() {
                        let _ = std::fs::create_dir_all(&debug_dir);
                    }
                    return debug_dir;
                }
                if let Some(p) = current.parent() {
                    current = p;
                } else {
                    break;
                }
            }
        }
    }
    // Fallback: use current directory + debug
    let debug_dir = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("debug");
    if !debug_dir.exists() {
        let _ = std::fs::create_dir_all(&debug_dir);
    }
    debug_dir
}

// Using the same struct as Fanqie for consistency
pub use super::fanqie::NovelMetadata;

pub async fn fetch_rank_list(app: &AppHandle, url: &str, debug_visible: bool) -> Result<Vec<String>, String> {
    log_to_file(&format!("Starting browser spider for rank list: {}", url));
    
    // 1. Fetch via Browser Spider
    let html = crate::browser_spider::fetch_via_window(app, url, debug_visible).await
        .map_err(|e| format!("Browser spider failed: {}", e))?;

    // Debug: Save rank page HTML
    use std::fs;
    let mut debug_path = get_debug_dir();
    debug_path.push("debug_2_rank.html");
    
    if let Err(e) = fs::write(&debug_path, &html) {
        log::error!("Failed to save rank HTML: {}", e);
    } else {
        log_to_file(&format!("Saved rank HTML to {:?}", debug_path));
    }
    
    // 2. Parse
    let document = Html::parse_document(&html);
    
    // Selectors:
    // 1. #rank-view-list .book-mid-info h2 a (Standard Desktop Rank)
    // 2. .book-img-text .book-mid-info h2 a (New Desktop Rank)
    // 3. .rank-list a.book-layout (Generic)
    // Removed .rank-body a.name as it matches authors.
    let selector = Selector::parse("#rank-view-list .book-mid-info h2 a, .book-img-text .book-mid-info h2 a, .rank-list a.book-layout")
        .map_err(|e| format!("Selector parse error: {:?}", e))?;

    let mut links = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for element in document.select(&selector) {
        let href = element.value().attr("href").unwrap_or_default();
        if !href.is_empty() {
             // Qidian links can be "//book.qidian.com/..." or "/book/..."
             let full_url = if href.starts_with("//") {
                 format!("https:{}", href)
             } else if href.starts_with("/") {
                 // Check if it is mobile or desktop
                 if url.contains("m.qidian.com") {
                     format!("https://m.qidian.com{}", href)
                 } else {
                     format!("https://www.qidian.com{}", href)
                 }
             } else {
                 href.to_string()
             };
             
             if full_url.contains("/book/") && !seen.contains(&full_url) {
                 seen.insert(full_url.clone());
                 links.push(full_url);
             }
        }
    }
    
    // Do NOT sort, as it destroys the rank order!
    
    log_to_file(&format!("Found {} novels in rank list.", links.len()));

    Ok(links)
}

use tauri::AppHandle;

// Use browser spider for metadata to bypass WAF
pub async fn fetch_novel_metadata(client: &Client, url: &str, app: &AppHandle, debug_visible: bool) -> Result<NovelMetadata, String> {
    let start_time = std::time::Instant::now();
    log_to_file(&format!("[START] fetch_novel_metadata: {}", url));
    
    // 1) 先尝试浏览器蜘蛛（可过大部分 WAF）
    let html = match crate::browser_spider::fetch_via_window(app, url, debug_visible).await {
        Ok(h) => {
            log_to_file(&format!("Browser spider succeeded, got {} bytes", h.len()));
            h
        },
        Err(e) => {
            // 浏览器蜘蛛失败，尝试移动端纯 HTTP 兜底
            log::warn!("Browser spider failed: {}. Trying mobile fallback...", e);
            return fetch_mobile_metadata(client, url).await;
        }
    };
    
    // Debug: Save metadata page HTML
    use std::fs;
    let mut debug_path = get_debug_dir();
    debug_path.push("debug_1_metadata.html");
    
    match fs::write(&debug_path, &html) {
        Ok(_) => log_to_file(&format!("Saved metadata HTML to {:?} (size: {} bytes)", debug_path, html.len())),
        Err(e) => log::error!("Failed to save metadata HTML to {:?}: {}", debug_path, e),
    }
    
    let document = Html::parse_document(&html);
    
    // Title: h1 or head > title
    let title_sel = Selector::parse("h1, #bookName").unwrap();
    let title = document.select(&title_sel).next()
        .map(|el| el.text().collect::<String>())
        .or_else(|| {
             // Fallback to <title> tag
             let meta_title_sel = Selector::parse("title").unwrap();
             document.select(&meta_title_sel).next()
                .map(|el| {
                    let full_title = el.text().collect::<String>();
                    // Usually "Novel Name_Author_..."
                    full_title.split('_').next().unwrap_or(&full_title).to_string()
                })
        })
        .unwrap_or_else(|| "Unknown Title".to_string()); // Soft fail if browser loaded something else

    // Check if we are still on WAF page?
    if title.contains("Just a moment") || title.contains("Security checking") {
        log_to_file(&format!("[FAILED] fetch_novel_metadata: WAF detected after {} ms", start_time.elapsed().as_millis()));
        return Err("Browser Spider still caught by WAF".to_string());
    }

    // Description: Prioritize #book-intro-detail (User Request: 作品简介)
    // Then try .intro (short summary) or meta
    let desc_sel_main = Selector::parse("#book-intro-detail").unwrap();
    let desc_sel_fallback = Selector::parse(".book-intro, .intro").unwrap();
    
    let description = document.select(&desc_sel_main).next()
        .map(|el| el.text().collect::<String>().trim().to_string())
        .or_else(|| {
             document.select(&desc_sel_fallback).next()
                .map(|el| el.text().collect::<String>().trim().to_string())
        })
        .or_else(|| {
             let meta_desc_sel = Selector::parse("meta[name='description']").unwrap();
             document.select(&meta_desc_sel).next()
                .and_then(|el| el.value().attr("content").map(|s| s.to_string()))
        })
        .unwrap_or_default();

    // Tags: Extract from multiple locations and deduplicate
    let mut tags = Vec::new();
    
    // 1. Standard tags from book attribute (e.g. 连载, 签约, VIP, 都市, 异术超能)
    let attr_tags_sel = Selector::parse(".book-attribute a").unwrap();
    for el in document.select(&attr_tags_sel) {
        let t = el.text().collect::<String>().trim().to_string();
        if !t.is_empty() { tags.push(t); }
    }

    // 2. Extra tags below description (e.g. 男生月票榜No.1, 系统流, 腹黑, 轻松)
    // Matches structure: <p class="all-label"> ... <a ...>Tag</a> ... </p>
    let extra_tags_sel = Selector::parse(".intro-honor-label .all-label a, .all-label a").unwrap();
    for el in document.select(&extra_tags_sel) {
         let t = el.text().collect::<String>().trim().to_string();
         if !t.is_empty() { tags.push(t); }
    }
    
    // Deduplicate
    tags.sort();
    tags.dedup();

    // Word count: .count em (first one)
    let count_sel = Selector::parse(".count em").unwrap();
    let word_count = document.select(&count_sel).next()
        .map(|el| el.text().collect::<String>())
        .unwrap_or_else(|| "未知".to_string());
    
    let metadata = NovelMetadata {
        title,
        url: url.to_string(),
        tags,
        word_count,
        description,
    };
    
    log_to_file(&format!("[SUCCESS] fetch_novel_metadata: {} in {} ms", metadata.title, start_time.elapsed().as_millis()));
    Ok(metadata)
}

// 兜底：请求移动端页面（通常 WAF 较宽松）
async fn fetch_mobile_metadata(client: &Client, url: &str) -> Result<NovelMetadata, String> {
    // 从 URL 中提取 bookId
    let re = Regex::new(r"book/([0-9]+)/?").map_err(|e| e.to_string())?;
    let book_id = re
        .captures(url)
        .and_then(|cap| cap.get(1).map(|m| m.as_str()))
        .ok_or_else(|| "无法从 URL 提取 bookId".to_string())?;

    let mobile_url = format!("https://m.qidian.com/book/{}", book_id);
    let resp = client
        .get(&mobile_url)
        .header("User-Agent", "Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.0 Mobile/15E148 Safari/604.1")
        .header("Referer", "https://m.qidian.com/")
        .send()
        .await
        .map_err(|e| format!("移动端请求失败: {}", e))?;

    let html = resp.text().await.map_err(|e| e.to_string())?;
    let document = Html::parse_document(&html);

    // 移动端标题选择器尝试
    let title_sel = Selector::parse("h1, .book-title, .detail h2").unwrap();
    let title = document
        .select(&title_sel)
        .next()
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Unknown Title".to_string());

    // 描述选择器
    let desc_sel = Selector::parse(".book-intro, .intro, meta[name='description']").unwrap();
    let description = document
        .select(&desc_sel)
        .next()
        .map(|el| {
            if el.value().name() == "meta" {
                el.value()
                    .attr("content")
                    .unwrap_or_default()
                    .to_string()
            } else {
                el.text().collect::<String>().trim().to_string()
            }
        })
        .unwrap_or_default();

    Ok(NovelMetadata {
        title,
        url: mobile_url,
        tags: vec![],
        word_count: "未知".to_string(),
        description,
    })
}

// Fetch chapter list using browser spider (to bypass WAF/JS render)
pub async fn fetch_chapter_list(app: &AppHandle, url: &str, debug_visible: bool) -> Result<Vec<(String, String)>, String> {
    let start_time = std::time::Instant::now();
    log_to_file(&format!("[START] fetch_chapter_list: {}", url));
    log_to_file(&format!("Debug visible: {}", debug_visible));
    
    // 1. Extract Book ID
    let re = Regex::new(r"book/([0-9]+)").map_err(|e| {
        log_to_file(&format!("[FAILED] fetch_chapter_list: Regex error: {}", e));
        e.to_string()
    })?;
    let book_id = re.captures(url)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str())
        .ok_or_else(|| {
            let err = "Failed to extract book ID for catalog";
            log_to_file(&format!("[FAILED] fetch_chapter_list: {}", err));
            err.to_string()
        })?;
    
    log_to_file(&format!("Extracted book ID: {}", book_id));

    // 2. Construct Mobile Catalog URL
    let catalog_url = format!("https://m.qidian.com/book/{}/catalog", book_id);
    log_to_file(&format!("Fetching catalog from: {}", catalog_url));
    log_to_file("Calling browser spider...");

    // 3. Fetch via Browser Spider
    let html = crate::browser_spider::fetch_via_window(app, &catalog_url, debug_visible).await
        .map_err(|e| {
            log_to_file(&format!("[FAILED] fetch_chapter_list: Browser spider error: {}", e));
            e
        })?;
    
    log_to_file(&format!("Browser spider returned HTML: {} bytes", html.len()));
    log_to_file(&format!("HTML preview (first 200 chars): {}", html.chars().take(200).collect::<String>()));
    
    // Debug: Save catalog page HTML
    use std::fs;
    let debug_dir = get_debug_dir();
    log_to_file(&format!("Debug directory: {:?}", debug_dir));
    let mut debug_path = debug_dir.clone();
    debug_path.push("debug_2_catalog.html");
    log_to_file(&format!("Attempting to save HTML to: {:?}", debug_path));
    
    match fs::write(&debug_path, &html) {
        Ok(_) => log_to_file(&format!("✓ Saved catalog HTML to {:?} (size: {} bytes)", debug_path, html.len())),
        Err(e) => log_to_file(&format!("✗ Failed to save catalog HTML to {:?}: {}", debug_path, e)),
    }
    
    // 4. Parse
    let document = Html::parse_document(&html);
    
    // Selectors for mobile catalog
    // Updated: matches .y-list__item a (standard list) or class contianing chapterItem (robustness)
    let selector = Selector::parse(".y-list__item a, a[class*='chapterItem']").map_err(|_| "Selector error".to_string())?;
    
    let mut chapters = Vec::new();
    for element in document.select(&selector) {
        let title = element.text().collect::<String>().trim().to_string();
        let href = element.value().attr("href").unwrap_or_default().to_string();

        if !title.is_empty() && !href.is_empty() && !href.contains("javascript") {
             let full_url = if href.starts_with("//") {
                 format!("https:{}", href)
             } else if href.starts_with("/") {
                 format!("https://m.qidian.com{}", href)
             } else {
                 href
             };
             
             // Simple dedup check or validation?
             // Only add if it looks like a chapter link
             if full_url.contains("/chapter/") || full_url.contains("/read/") {
                  chapters.push((title, full_url));
             }
        }
    }
    
    if chapters.is_empty() {
        // Debug: Log HTML snippet to see what happened
        let snippet: String = html.chars().take(1000).collect();
        log_to_file(&format!("Qidian Spider: No chapters found! HTML Snippet: {}", snippet));
        
        // Also try to write full HTML to a file for debugging
        let mut error_debug_path = get_debug_dir();
        error_debug_path.push("qidian_catalog_debug.html");
        let _ = fs::write(&error_debug_path, &html);
        log_to_file(&format!("Saved error catalog HTML to {:?}", error_debug_path));

        return Err(format!("No chapters found in catalog. Check {:?}", error_debug_path));
    }

    log_to_file(&format!("[SUCCESS] fetch_chapter_list: Found {} chapters in {} ms", chapters.len(), start_time.elapsed().as_millis()));
    Ok(chapters)
}

// Qidian chapter pages. We use browser spider to bypass WAF.
pub async fn download_chapter(app: &AppHandle, url: &str, debug_visible: bool) -> Result<(String, String), String> {
    let start_time = std::time::Instant::now();
    log_to_file(&format!("[START] download_chapter: {}", url));
    
    // Force WWW url if it is mobile, to ensure we get desktop page (better for scraping usually, or consistent with UA)
    let target_url = url.replace("m.qidian.com", "www.qidian.com");
    
    // Use browser spider
    let html = crate::browser_spider::fetch_via_window(app, &target_url, debug_visible).await
        .map_err(|e| {
            log_to_file(&format!("[FAILED] download_chapter: Browser spider error: {}", e));
            e
        })?;
    
    // Debug: Save chapter page HTML
    use std::fs;
    let mut debug_path = get_debug_dir();
    debug_path.push("debug_3_chapter.html");
    
    match fs::write(&debug_path, &html) {
        Ok(_) => log_to_file(&format!("Saved chapter HTML to {:?} (size: {} bytes)", debug_path, html.len())),
        Err(e) => log_to_file(&format!("Failed to save chapter HTML to {:?}: {}", debug_path, e)),
    }
    
    let document = Html::parse_document(&html);

    // Selectors for WWW site
    // Title: .j_chapterName, .text-head h3, or h1
    let title_sel = Selector::parse(".j_chapterName, .text-head h3, h1, .chapter-name").unwrap();
    let title = document.select(&title_sel).next()
        .map(|el| el.text().collect::<String>().trim().to_string())
        .unwrap_or_else(|| "".to_string()); // Title is optional here as we have it from list, but good for verify
        
    // Content: .read-content or .main-text-wrap
    // NOTE: Qidian sometimes splits content into multiple paragraphs/elements.
    // Updated: matches new desktop structure (main.content)
    let content_sel = Selector::parse("main.content, .read-content, .main-text-wrap, .j_readContent, #reader-content").unwrap();
    
    let content = if let Some(container) = document.select(&content_sel).next() {
        // We prefer to iterate over paragraphs <p> if they exist to keep formatting
        let p_sel = Selector::parse("p").unwrap();
        let mut lines = Vec::new();
        for p in container.select(&p_sel) {
            lines.push(p.text().collect::<String>());
        }
        
        if !lines.is_empty() {
             lines.join("\n\n")
        } else {
             // Fallback: just raw text
             container.text().collect::<String>()
        }
    } else {
        // Enhanced Debugging
        let snippet: String = html.chars().take(500).collect();
        log_to_file(&format!("Failed to find content for url: {}\nSelectors tried: main.content, .read-content, .main-text-wrap, .j_readContent, #reader-content\nHTML Snippet: {}", url, snippet));
        log_to_file(&format!("[FAILED] download_chapter: Content not found after {} ms", start_time.elapsed().as_millis()));
        return Err("Failed to find content (WAF or Selector Mismatch). See logs.".to_string());
    };
    
    // Extra cleaner? Qidian sometimes has hidden elements or anti-copy. 
    // For now, let's trust simple text extraction.
    
    log_to_file(&format!("[SUCCESS] download_chapter: {} ({} chars) in {} ms", title, content.len(), start_time.elapsed().as_millis()));
    Ok((title, content))
}
