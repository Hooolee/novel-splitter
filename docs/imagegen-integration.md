# 图片生成脚本关系说明

本文档说明项目内 `scripts/gen_image.sh` 与 Codex 自带 `imagegen` skill 脚本之间的关系，方便后续维护和改动。

## 当前调用链路

当前项目的图片生成链路如下：

1. 你在项目里执行 `scripts/gen_image.sh`
2. `scripts/gen_image.sh` 读取项目根目录的 `account.env`
3. 脚本把 `apikey` 转成 `OPENAI_API_KEY`
4. 脚本把 `apibase` 转成 `OPENAI_BASE_URL`
5. 脚本再调用 Codex 自带的底层脚本：

```text
~/.codex/skills/.system/imagegen/scripts/image_gen.py
```

6. 底层脚本再去调用图片模型接口，生成图片并写入 `--out` 指定的位置

## 两层脚本的职责划分

### 项目脚本：`scripts/gen_image.sh`

这是“项目包装层”，主要负责项目自己的约定：

- 读取 `account.env`
- 统一注入环境变量
- 固定底层脚本路径
- 设置本项目默认输出目录 `output/imagegen/`
- 允许以后补充项目自己的默认参数或预设

这层脚本适合放“项目相关改动”，因为它不会直接改动系统 skill。

### 底层脚本：`~/.codex/skills/.system/imagegen/scripts/image_gen.py`

这是“能力实现层”，主要负责图片生成逻辑本身：

- 默认模型选择
- `generate` / `edit` / `generate-batch` 子命令
- prompt 增强逻辑
- 与图片接口的请求格式
- 输出格式、尺寸、质量等底层参数校验

这层脚本适合放“生成能力相关改动”。

## 以后要改什么，应该改哪一层

### 优先改 `scripts/gen_image.sh` 的情况

以下需求优先改项目脚本：

- 想换 `account.env` 的字段名
- 想把默认输出目录换成别的位置
- 想追加项目默认风格参数
- 想封装更短的命令，例如 `--preset cat-cartoon`
- 想切换项目使用的 Python 路径或额外依赖路径

原因是这类变更属于“项目使用方式”，不需要动到底层 skill。

### 需要看 `image_gen.py` 的情况

以下需求需要参考或修改底层脚本：

- 想改默认模型
- 想调整 prompt 自动增强逻辑
- 想新增底层命令参数
- 想修改图片接口调用方式
- 想改 `generate-batch`、`edit` 的内部行为

原因是这些已经属于“图片生成能力本身”。

## 当前已知默认值

当前这套链路里，底层脚本默认值是：

- 默认模型：`gpt-image-2`
- 默认质量：`medium`
- 默认输出格式：`png`

如果你没有在 `scripts/gen_image.sh` 调用时显式传 `--model`，那么就会走底层脚本默认值。

## 推荐维护方式

建议按下面顺序维护：

1. 先尽量在 `scripts/gen_image.sh` 做包装
2. 如果包装层无法满足，再查看底层 `image_gen.py`
3. 除非确实需要改底层能力，否则尽量不要直接修改系统 skill

这样做的好处是：

- 项目改动集中在仓库内
- 后续迁移或备份更方便
- 不容易因为系统 skill 更新而丢失项目侧逻辑

## 常用示例

生成图片：

```bash
./scripts/gen_image.sh \
  --prompt "一只可爱的小猫，儿童绘本风格" \
  --out output/imagegen/cat2.png
```

显式指定底层命令：

```bash
./scripts/gen_image.sh generate \
  --prompt "一只可爱的小鸟，水彩插画风格" \
  --out output/imagegen/bird2.png
```

如果后面你要做透明背景、批量生成、局部编辑，也建议先保留这个包装层入口，再逐步给它补参数。
