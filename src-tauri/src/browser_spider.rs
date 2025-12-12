use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, Listener};
use std::time::Duration;
use tokio::sync::oneshot;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
struct SpiderResult {
    html: String,
}

pub async fn fetch_via_window(app: &AppHandle, url: &str, debug_visible: bool) -> Result<String, String> {
    let label = "spider_worker";
    
    // Close existing if any
    if let Some(w) = app.get_webview_window(label) {
        let _ = w.close();
    }

    // Prepare channel for async result
    let (tx, rx) = oneshot::channel();
    
    // Wrap tx in a thread-safe container (Mutex + Option) to move into closure
    let tx_mutex = std::sync::Arc::new(std::sync::Mutex::new(Some(tx)));
    let tx_clone = tx_mutex.clone();

    // Listen for event
    // Note: In Tauri v2, `listen` returns a handler id. We need to unlisten later or rely on one-time nature.
    // Ideally we use `once` but `AppHandle` doesn't expose `once` directly in all versions. 
    // We'll use `listen` and a unique event name per request or just generic.
    // For simplicity, generic event "spider_response".
    
    let event_id = app.listen("spider_response", move |event| {
        if let Ok(payload) = serde_json::from_str::<SpiderResult>(event.payload()) {
            if let Ok(mut guard) = tx_clone.lock() {
                if let Some(sender) = guard.take() {
                    let _ = sender.send(Ok(payload.html));
                }
            }
        }
    });

    // Initialization script:
    // - Waits for DOM ready or load.
    // - Uses a hard fallback to avoid hanging if some resources block the load event.
    // - Emits once with the page HTML.
    let init_script = r#"
        (() => {
            let sent = false;
            const emitOnce = () => {
                if (sent) return;
                sent = true;
                try {
                    const html = document.documentElement?.outerHTML || document.body?.outerHTML || '';
                    console.log('[Spider] Sending HTML, length:', html.length);
                    window.__TAURI__?.event?.emit('spider_response', { html });
                } catch (e) {
                    console.error('[Spider] Error getting HTML:', e);
                    window.__TAURI__?.event?.emit('spider_response', { html: '' });
                }
            };

            const scheduleSend = (delay) => {
                console.log('[Spider] Scheduling send in', delay, 'ms');
                setTimeout(emitOnce, delay);
            };

            // Wait for Qidian specific elements, but don't wait forever
            const checkAndSend = () => {
                 console.log('[Spider] Checking for Qidian elements...');
                 // Check for various Qidian page elements:
                 // - .y-list__item: Mobile catalog list items
                 // - .chapter-li-a: Desktop catalog links
                 // - .j_chapterName: Chapter title
                 // - .book-intro: Book introduction
                 // - main.content: Main content area (desktop chapter page)
                 const catalogMobile = document.querySelector('.y-list__item');
                 const catalogDesktop = document.querySelector('.chapter-li-a');
                 const chapterTitle = document.querySelector('.j_chapterName');
                 const bookIntro = document.querySelector('.book-intro');
                 const mainContent = document.querySelector('main.content');
                 
                 if (catalogMobile || catalogDesktop || chapterTitle || bookIntro || mainContent) {
                     console.log('[Spider] Found Qidian element, scheduling send in 2s');
                     scheduleSend(2000); // Wait 2s for full render after finding key elements
                 } else {
                     // Fallback: send after reasonable wait
                     console.log('[Spider] No specific elements found, using fallback');
                     scheduleSend(5000);
                 }
            };
            
            console.log('[Spider] Init script loaded, readyState:', document.readyState);
            
            if (document.readyState === 'complete' || document.readyState === 'interactive') {
                checkAndSend();
            } else {
                window.addEventListener('DOMContentLoaded', checkAndSend, { once: true });
            }

            // Hard fallback in case nothing triggers
            scheduleSend(10000);
        })();
    "#;

    // Build window
    // Note: We use 1x1 pixel or hidden
    let window_builder = WebviewWindowBuilder::new(app, label, WebviewUrl::External(url.parse().map_err(|e: url::ParseError| e.to_string())?))
        .title("Spider Worker")
        .visible(debug_visible) 
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .initialization_script(init_script);

    let _window = window_builder.build().map_err(|e| format!("Failed to create window: {}", e))?;

    // Wait for result with timeout
    let result = tokio::select! {
        res = rx => {
            app.unlisten(event_id);
            res.map_err(|_| "Channel closed".to_string())?
        }
        _ = tokio::time::sleep(Duration::from_secs(45)) => {
            app.unlisten(event_id);
            Err("Timeout waiting for spider".to_string())
        }
    };
    
    // Cleanup window
    if let Some(w) = app.get_webview_window(label) {
        let _ = w.close();
    }

    result
}
