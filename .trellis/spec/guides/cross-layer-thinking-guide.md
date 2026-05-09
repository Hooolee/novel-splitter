# 跨层思维指南

> **目的**: 实现跨越 Tauri 前后端边界的功能前，系统地思考数据流。
>
> **核心原则**: 30 分钟思考节省 3 小时调试。

## 何时使用

当你的功能：

- 跨越 2+ 层（前端事件 → Tauri 命令 → 爬虫 → 文件系统）
- 涉及数据格式转换（TypeScript ←→ Rust 结构体）
- 有实时或事件驱动组件
- 从外部获取数据（爬虫 API、LLM API）

## 检查清单

### 1. 层识别

功能涉及哪些层？

- [ ] Vue 前端 UI
- [ ] Vue composable / 状态管理
- [ ] Tauri invoke 命令
- [ ] Tauri 事件系统
- [ ] Rust 爬虫模块
- [ ] Rust AI 模块
- [ ] 文件系统 (info.json, chapter files)
- [ ] 外部 API (爬虫目标站, LLM API)

### 2. 数据流动方向

```
下载流程: 前端输入 → invoke('start_download') → Rust 爬虫 → 文件系统 → 事件通知 → 前端刷新
AI 流程:  前端触发 → invoke('start_ai_analysis') → Rust 读取文件 → LLM API → 流式事件 → 前端渲染
文件流程: 前端触发 → invoke('get_file_tree') → Rust 扫描目录 → JSON 返回 → 前端渲染树
```

- [ ] 只读？ 数据从文件到 UI
- [ ] 写入？ 数据从 UI 到文件
- [ ] 双向？ 下载 + 进度反馈

### 3. 各层数据格式

| 层 | 格式 | 示例 |
|----|------|------|
| Vue 组件 | TypeScript 对象 | `NovelMetadata { title, tags, ... }` |
| invoke 参数 | JSON (camelCase) | `{ chapterCount: 5 }` |
| Rust 命令 | Rust struct | `NovelInfo { title, tags, ... }` |
| 文件存储 | JSON 文件 | `info.json` |
| 爬虫响应 | HTML/DOM | scraper 文档树 |
| 事件 | JSON payload | `{ current, total, chapter }` |

### 4. 边界问题

**Vue / Tauri 边界：**

- invoke 参数名：前端 camelCase → Rust snake_case（Tauri 自动转换）
- 命令名：前端 camelCase → Rust snake_case（Tauri 自动转换）
- 事件 payload：扁平 JSON，避免嵌套过深

**Rust 命令 / 文件系统边界：**

- 路径处理：使用 PathBuf，避免字符串拼接
- 编码：文件统一 UTF-8
- 序列化：serde_json::to_string_pretty

**爬虫 / 外部 API 边界：**

- 超时处理：所有 HTTP 请求设置超时
- 错误处理：网络错误、解析错误、HTTP 状态码
- 限流：控制请求频率，避免被封

### 5. 边界情况

- [ ] 网络请求超时/失败？
- [ ] 文件不存在/权限错误？
- [ ] API 密钥未配置？
- [ ] LLM 返回格式异常？
- [ ] 用户中途取消操作？
- [ ] 爬虫目标网站变更新？

---

## 常见面包陷阱

| Bug | 根因 | 预防 |
|-----|------|------|
| invoke 返回错误 | 命令未注册到 invoke_handler | 添加新命令后检查 lib.rs |
| 事件收不到 | 事件名前后端不匹配 | 使用常量定义事件名 |
| JSON 解析失败 | LLM 输出不标准 | 添加容错解析 |
| 下载后文件树不更新 | 未调用 refreshTreeFiles | 操作完成后始终刷新 |
| 章节顺序错乱 | 文件名排序问题 | 使用零填充编号 (01.txt) |
| 类型不匹配 | Rust struct 字段名与前端接口不一致 | 同步两边定义 |

---

## 检查模板

```markdown
## 功能: [名称]

### 涉及层
- [ ] Vue 组件
- [ ] Tauri invoke
- [ ] Tauri 事件
- [ ] Rust 命令
- [ ] 爬虫
- [ ] AI
- [ ] 文件系统

### 数据流
[描述流程]

### 各层格式
| 层 | 格式 |
|---|---|
| ... | ... |

### 边界情况
- [ ] 错误处理
- [ ] 用户取消
- [ ] 空状态
```
