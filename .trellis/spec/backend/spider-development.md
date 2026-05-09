# 爬虫开发规范

本文档覆盖小说平台爬虫的开发模式和多平台统一接口。

## 统一 Trait 接口

所有平台爬虫实现 `NovelSpider` trait：

```rust
#[async_trait]
pub trait NovelSpider {
    async fn scrape_novel_info(&self, url: &str) -> Result<NovelInfo, String>;
    async fn scrape_chapters(&self, url: &str, chapter_count: usize) -> Result<Vec<Chapter>, String>;
}
```

### 数据模型

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct NovelInfo {
    pub title: String,
    pub tags: Vec<String>,
    pub word_count: String,
    pub description: String,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Chapter {
    pub index: usize,
    pub title: String,
    pub content: String,
}
```

## 平台路由

在 `spiders/mod.rs` 中路由到对应实现：

```rust
pub enum Platform {
    Fanqie,
    Qidian,
    Browser { visible: bool },
}

impl Platform {
    pub fn spider(&self) -> Box<dyn NovelSpider> {
        match self {
            Platform::Fanqie => Box::new(FanqieSpider),
            Platform::Qidian => Box::new(QidianSpider::new()),
            Platform::Browser { visible } => Box::new(BrowserSpider::new(*visible)),
        }
    }
}
```

## 爬虫实现模式

### 番茄小说

```rust
pub struct FanqieSpider;

#[async_trait]
impl NovelSpider for FanqieSpider {
    async fn scrape_novel_info(&self, url: &str) -> Result<NovelInfo, String> {
        let client = reqwest::Client::new();
        let html = client.get(url).send().await?.text().await?;
        let document = scraper::Html::parse_document(&html);

        // 使用 scraper 提取信息
        let title_sel = scraper::Selector::parse("h1").unwrap();
        let title = document.select(&title_sel)
            .next()
            .map(|el| el.text().collect::<String>())
            .unwrap_or_default();

        // ...
    }
}
```

### 起点中文网

起点特有 WAF 处理：

```rust
pub struct QidianSpider {
    pub spider_visible: bool,
}

impl QidianSpider {
    pub fn new(spider_visible: bool) -> Self {
        Self { spider_visible }
    }
}

#[async_trait]
impl NovelSpider for QidianSpider {
    async fn scrape_novel_info(&self, url: &str) -> Result<NovelInfo, String> {
        // 可能需要使用 BrowserSpider 处理 WAF
        if self.spider_visible {
            return BrowserSpider::new(true).scrape_novel_info(url).await;
        }
        // 标准 HTTP 请求
    }
}
```

### 无头浏览器爬虫

用于 WAF / JS 密集型网站：

```rust
pub struct BrowserSpider {
    pub visible: bool,
}

impl BrowserSpider {
    pub fn new(visible: bool) -> Self { Self { visible } }

    async fn create_page(&self, url: &str) -> Result<Page, String> {
        // 创建 headless / visible 浏览器页面
    }
}
```

## 下载流程

```
1. 前端调用 start_download(url, count, platform)
2. lib.rs 路由到对应平台 spider
3. 爬虫提取小说信息 (scrape_novel_info)
4. 创建下载目录 ../downloads/NovelName/
5. 保存 info.json 元数据
6. 逐章爬取并保存为 nn.txt
7. 每章发送 download-progress 事件
8. 下载完成后自动触发 AI 分析 (若有 API 配置)
```

## 添加新平台爬虫

1. 创建 `spiders/newplatform.rs`
2. 实现 `NovelSpider` trait
3. 在 `mod.rs` 的 `Platform` 枚举中添加变体
4. 在前端平台选择器中添加选项

## 最佳实践

1. **错误处理** - 所有网络请求要有超时和重试
2. **进度反馈** - 每章下载后发送进度事件
3. **失败隔离** - 单章失败不影响其他章节
4. **WAF 处理** - 起点等网站需要浏览器蜘蛛备选
5. **选择器维护** - 网站选择器变更时及时更新

## 常见陷阱

- 网站反爬升级 → 使用浏览器蜘蛛降级
- 编码问题 → 确保正确处理中文编码
- 请求过快 → 添加适当延迟避免被封
- 章节缺失 → 处理章节列表分页情况
