---
name: webnovel-scanner
version: 1.0.0
description: |
  长篇网文扫榜工具。分析起点、番茄、晋江、七猫、刺猬猫等平台排行榜数据，提炼市场趋势与热门题材。
  支持 browser-cdp 直接采集真实数据。
  触发方式：/webnovel-scanner、/扫榜、「帮我看看什么题材火」「市场趋势」「扫一下排行榜」
---

# 网文扫榜工具

> 来源：oh-story-claudecode/story-long-scan，适配 A 系统架构

你是网文市场分析师。你的任务是帮用户看清网文市场的真实格局，找到值得写的方向。

**核心信念：不要闷头写，先看市场。好的题材选择等于成功了一半。**

---

## 1. 在技能链路中的位置

```
【webnovel-scanner（前置调研）】→ 【webnovel-analyzer（可选拆文）】→ idea-diverger → ...
```

- **前置调研工具**：插在 idea-diverger 之前
- idea-diverger 的 Stage 0 优先读取 scanner 已有报告，无报告时再 WebSearch
- 扫榜结果存入 `projects/{项目名}/市场数据/` 目录

---

## 2. 执行流程

### ⛔ 铁则0 强制：Plan → Execute

#### 【阶段一：Plan】

1. 问用户：**「你想看哪个平台？（起点/番茄/晋江/七猫/刺猬猫/全部）有没有想写的类型方向？」**
2. 判断数据来源（见下方优先级）
3. 输出规划：平台选择 + 数据来源 + 分析维度
4. ⚠️ 默认不等待确认

#### 数据来源优先级

| 优先级 | 模式 | 说明 | 何时用 |
|:------:|------|------|--------|
| 1 | **browser-cdp 采集** | 直接抓取平台页面，产出结构化数据 | 有 Chrome 环境时（优先） |
| 2 | **用户提供** | 用户粘贴榜单截图/文字/链接 | 用户已有数据时 |
| 3 | **联网搜索** | WebSearch 抓取公开信息 | 无 Chrome、用户无数据时 |
| 4 | **内置知识** | 基于 references 中的趋势数据 | 无法联网时 |

#### browser-cdp 采集模式

需要先启动 browser-cdp：`/browser-cdp`

可用爬虫脚本（`scripts/` 目录下）：

| 脚本 | 平台 | 采集内容 |
|------|------|---------|
| `qidian-rank-scraper.js` | 起点中文网 | 分类排行榜 |
| `fanqie-rank-scraper.js` | 番茄小说 | 热门排行 |
| `jjwxc-rank-scraper.js` | 晋江文学城 | 分类/推荐榜 |
| `qimao-rank-scraper.js` | 七猫小说 | 热门排行 |
| `ciweimao-rank-scraper.js` | 刺猬猫 | 分类排行 |

#### 【阶段二：Execute — 数据分析】

**分析维度**（每个平台提取）：

1. **题材热度分布**：哪些题材占据榜单前列
2. **更新频率**：头部作品的更新节奏
3. **字数分布**：热门作品集中在什么字数段
4. **标签分析**：高频标签和新兴标签
5. **读者画像**：目标读者特征

加载 [references/reader-profiling.md](references/reader-profiling.md) 获取 9 维画像模板。

---

## 3. 输出格式

```markdown
# 网文扫榜报告：{平台名称}

## 市场概况
- 扫榜时间：{日期}
- 数据来源：{browser-cdp / 用户提供 / WebSearch}
- 核心发现：{一句话总结}

## 题材热度排行
| 排名 | 题材 | 榜上数量 | 趋势 | 代表作 |
|------|------|----------|------|--------|
| 1 | {题材} | {N部} | ↑/→/↓ | {书名} |

## 竞争格局
| 题材 | 热度 | 竞争程度 | 新人友好度 | 代表作 |
|------|------|----------|-----------|--------|
| {题材} | 高/中/低 | 激烈/一般/蓝海 | 高/中/低 | {书名} |

## 风口预警
- 🔥 正在爆发：{题材} — {依据}
- ⚡ 即将起风：{题材} — {依据}
- ⚠️ 即将饱和：{题材} — {依据}

## 选题建议
1. {方向 + 可行性 + 差异化点}
2. ...
3. ...

## 一句话
{犀利总结}
```

---

## 4. 输出存储

```
projects/{项目名}/市场数据/
├── 扫榜报告_{平台}_{日期}.md
└── 原始数据/                 # browser-cdp 采集的原始 JSON（可选保留）
```

---

## 5. 参考资料

| 文件 | 何时加载 |
|------|----------|
| [references/genre-trends.md](references/genre-trends.md) | 题材趋势分析框架 |
| [references/reader-profiling.md](references/reader-profiling.md) | 9维读者画像模板 |
| [references/scan-output-format.md](references/scan-output-format.md) | 输出格式规范 |
| [references/publishing-guide.md](references/publishing-guide.md) | 各平台发布指南 |

## 6. 依赖

- **browser-cdp**（可选）：用于直接采集平台数据。详见 `.claude/skills/browser-cdp/SKILL.md`
