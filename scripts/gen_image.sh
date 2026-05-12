#!/usr/bin/env bash
set -euo pipefail

# 关系说明:
# - 本脚本是项目内包装层，目的是把 account.env、默认路径和常用调用方式固定下来。
# - 真正执行图片生成的是 Codex 自带 imagegen skill 中的 Python 脚本:
#   ~/.codex/skills/.system/imagegen/scripts/image_gen.py
# - 因此这里适合改“项目侧配置”，例如:
#   1) account.env 的读取方式
#   2) 默认输出目录
#   3) PYTHONPATH / Python 解释器
#   4) 是否追加项目自己的预设参数
# - 如果要改“图片生成能力本身”，例如:
#   1) 默认模型
#   2) prompt 增强逻辑
#   3) generate / edit / generate-batch 的参数行为
#   4) 与 OpenAI 图片接口的交互细节
#   则需要去看并修改底层 skill 脚本。
# - 为了避免覆盖系统级 skill，推荐优先在本脚本增加包装逻辑；只有确实需要改底层能力时，
#   再参考 docs/imagegen-integration.md 中记录的关系说明去调整。

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENV_FILE="${IMAGEGEN_ENV_FILE:-$ROOT_DIR/account.env}"
CODEX_HOME_DIR="${CODEX_HOME:-$HOME/.codex}"
IMAGEGEN_SCRIPT="${IMAGEGEN_SCRIPT:-$CODEX_HOME_DIR/skills/.system/imagegen/scripts/image_gen.py}"
PYTHON_BIN="${PYTHON_BIN:-python}"
PYTHON_FALLBACK_SITE="${IMAGEGEN_PYTHONPATH:-}"
DEFAULT_OUTPUT_DIR="${IMAGEGEN_OUTPUT_DIR:-$ROOT_DIR/output}"

usage() {
  cat <<'EOF'
用法:
  ./scripts/gen_image.sh --prompt "一只可爱的小鸟" --out output/imagegen/bird.png
  ./scripts/gen_image.sh generate --prompt "一只可爱的小鸟" --out output/imagegen/bird.png
  ./scripts/gen_image.sh edit --input input.png --prompt "改成水彩风" --out output/imagegen/output.png

说明:
  - 默认命令是 generate
  - 默认读取仓库根目录的 account.env
  - 默认输出目录是仓库根目录的 output/
  - account.env 中支持:
      apikey=...
      apibase=...
  - 也支持直接通过环境变量传入:
      OPENAI_API_KEY
      OPENAI_BASE_URL
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

if [[ ! -f "$IMAGEGEN_SCRIPT" ]]; then
  echo "未找到 imagegen 脚本: $IMAGEGEN_SCRIPT" >&2
  exit 1
fi

if [[ -f "$ENV_FILE" ]]; then
  set -a
  # shellcheck disable=SC1090
  source "$ENV_FILE"
  set +a
fi

export OPENAI_API_KEY="${OPENAI_API_KEY:-${apikey:-}}"
if [[ -z "${OPENAI_API_KEY:-}" ]]; then
  echo "未找到 OPENAI_API_KEY。请在 account.env 中设置 apikey，或直接导出 OPENAI_API_KEY。" >&2
  exit 1
fi

if [[ -z "${OPENAI_BASE_URL:-}" && -n "${apibase:-}" ]]; then
  export OPENAI_BASE_URL="${apibase%/}/v1"
fi

if [[ -n "${PYTHON_FALLBACK_SITE:-}" && -d "$PYTHON_FALLBACK_SITE" ]]; then
  export PYTHONPATH="$PYTHON_FALLBACK_SITE${PYTHONPATH:+:$PYTHONPATH}"
fi

if ! "$PYTHON_BIN" -c "import openai" >/dev/null 2>&1; then
  echo "当前 Python 环境缺少 openai 包。可先执行: pip3 install openai" >&2
  exit 1
fi

mkdir -p "$DEFAULT_OUTPUT_DIR"

command_name="generate"
if [[ $# -gt 0 ]]; then
  case "$1" in
    generate|edit|generate-batch)
      command_name="$1"
      shift
      ;;
  esac
fi

has_output_target=false
for arg in "$@"; do
  case "$arg" in
    --out|--out-dir)
      has_output_target=true
      break
      ;;
  esac
done

if [[ "$has_output_target" == false ]]; then
  set -- --out-dir "$DEFAULT_OUTPUT_DIR" "$@"
fi

exec "$PYTHON_BIN" "$IMAGEGEN_SCRIPT" "$command_name" "$@"
