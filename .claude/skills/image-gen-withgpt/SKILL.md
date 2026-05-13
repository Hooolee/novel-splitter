---
name: "image-gen-withgpt"
description: "通过第三方 API 生成或编辑图片（凭据来自 account.env）"
---

# Image Gen With GPT Account

通过 `account.env` 中的第三方 API Key 和 Base URL 调用 GPT Image 模型。
支持三种操作：**生成 (generate)**、**编辑 (edit)**、**批量生成 (generate-batch)**。

## 入口

入口脚本是本 skill 目录下的 `scripts/image_gen.py`。
调用时用本 SKILL.md 所在目录的绝对路径拼接 `scripts/image_gen.py`：

```bash
python3 <SKILL_DIR>/scripts/image_gen.py generate ...
python3 <SKILL_DIR>/scripts/image_gen.py edit ...
python3 <SKILL_DIR>/scripts/image_gen.py generate-batch ...
```

> `<SKILL_DIR>` = 本文件 (SKILL.md) 所在的目录。Agent 加载 skill 时已知此路径，无需手动指定。

## 凭据

在本 skill 目录的 `account.env` 中配置：

```env
apikey=sk-your-api-key-here
apibase=https://your-api-provider.com
```

- 脚本自动将 `apikey` → `OPENAI_API_KEY`，`apibase` → `OPENAI_BASE_URL`
- 如果 `apibase` 不以 `/v1` 结尾，自动追加
- 已有同名环境变量时，环境变量优先

## 常用命令

### 生成

```bash
python3 <SKILL_DIR>/scripts/image_gen.py generate \
  --prompt "A cozy alpine cabin at dawn" \
  --out output/imagegen/alpine-cabin.png
```

### 编辑

```bash
python3 <SKILL_DIR>/scripts/image_gen.py edit \
  --image input.png \
  --prompt "Replace the background with a warm sunset gradient" \
  --out output/imagegen/sunset-edit.png
```

### 透明背景

```bash
python3 <SKILL_DIR>/scripts/image_gen.py generate \
  --model gpt-image-1 \
  --background transparent \
  --output-format png \
  --prompt "A clean product cutout on a transparent background" \
  --out output/imagegen/product-cutout.png
```

### 批量生成

准备 JSONL 文件（每行一个 prompt 或 JSON 对象）：

```jsonl
"A red sports car on a mountain road"
{"prompt": "A blue ocean wave at sunset", "size": "1536x1024"}
{"prompt": "A cat in a spacesuit", "n": 2}
```

执行：

```bash
python3 <SKILL_DIR>/scripts/image_gen.py generate-batch \
  --input jobs.jsonl \
  --out-dir output/imagegen/batch \
  --concurrency 3
```

## 主要参数

| 参数 | 默认值 | 说明 |
|------|--------|------|
| `--model` | `gpt-image-2` | 模型名（需以 `gpt-image-` 开头） |
| `--size` | `auto` | 尺寸，如 `1024x1024`、`1536x1024` |
| `--quality` | `medium` | `low` / `medium` / `high` / `auto` |
| `--background` | — | `transparent` / `opaque` / `auto` |
| `--output-format` | `png` | `png` / `jpeg` / `webp` |
| `--n` | `1` | 单次生成数量（1-10） |
| `--out` | `output/imagegen/output.png` | 输出路径 |
| `--out-dir` | — | 输出目录（批量模式必填） |
| `--force` | — | 覆盖已有输出 |
| `--dry-run` | — | 仅打印请求参数，不实际调用 API |
| `--no-augment` | — | 跳过 prompt 结构化增强 |

### Prompt 增强提示词（可选）

`--use-case`、`--scene`、`--subject`、`--style`、`--composition`、`--lighting`、`--palette`、`--materials`、`--text`、`--constraints`、`--negative`

### 编辑模式额外参数

`--image`（必填，可多次）、`--mask`、`--input-fidelity`

## 文件结构

```
image-gen-withgpt/
├── SKILL.md              # 本文件
├── account.env            # 第三方 API 凭据（勿提交到 git）
├── account.env.example    # 凭据模板
├── scripts/
│   ├── image_gen.py       # 唯一入口：生成/编辑/批量
│   └── remove_chroma_key.py  # Chroma key 背景移除工具
└── references/            # API / Prompting 参考文档
```

## 故障排查

| 症状 | 原因 | 解决 |
|------|------|------|
| `No module named 'openai'` | Python 环境缺包 | `pip3 install openai` |
| `OPENAI_API_KEY 未设置` | 凭据未配置 | 检查 `account.env` 或导出环境变量 |
| `429 / rate limit` | API 速率限制 | 批量模式会自动重试；降低 `--concurrency` |
| 透明背景无效 | 格式不支持透明 | 使用 `--output-format png` 或 `webp` |
