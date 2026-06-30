#!/usr/bin/env bash
# 打开 Bong 模块图谱 webui（单文件自包含 HTML，file:// 直开，无需服务器）。
# 用法: bash scripts/runwebui.sh   或在 Claude Code 里 /runwebui
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HTML="$REPO_ROOT/module-map/index.html"

if [[ ! -f "$HTML" ]]; then
  echo "❌ 找不到 $HTML —— 先生成模块图谱。" >&2
  exit 1
fi

echo "📊 模块图谱: $HTML"

# WSL: 转 Windows 路径用默认浏览器打开
if command -v wslpath >/dev/null 2>&1 && command -v explorer.exe >/dev/null 2>&1; then
  WIN_PATH="$(wslpath -w "$HTML")"
  echo "→ explorer.exe \"$WIN_PATH\""
  explorer.exe "$WIN_PATH" || true   # explorer 对成功打开常返回非 0，忽略
  exit 0
fi

# 原生 Linux 桌面
if command -v xdg-open >/dev/null 2>&1; then
  xdg-open "$HTML" >/dev/null 2>&1 &
  exit 0
fi

# macOS
if command -v open >/dev/null 2>&1; then
  open "$HTML"
  exit 0
fi

echo "⚠️ 未检测到浏览器打开方式，请手动打开: file://$HTML" >&2
