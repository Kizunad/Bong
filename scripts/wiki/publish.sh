#!/usr/bin/env bash
set -euo pipefail

# 生成物仓库只接受普通提交；有未提交人工改动时宁可停下，也不覆盖它们。
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)"
WIKI_DIR="${BONG_WIKI_OUTPUT:-$ROOT/tmp/wiki}"
WIKI_REPO="${BONG_WIKI_REPO:-$ROOT/tmp/Bong-wiki}"
REMOTE="${BONG_WIKI_REMOTE:-git@github.com:Kizunad/Bong-wiki.git}"

[[ -f "$WIKI_DIR/index.html" ]] || {
  echo "请先运行 bash scripts/wiki/build.sh 生成 $WIKI_DIR" >&2
  exit 1
}

if [[ ! -d "$WIKI_REPO/.git" ]]; then
  mkdir -p "$(dirname "$WIKI_REPO")"
  git clone "$REMOTE" "$WIKI_REPO"
else
  if git -C "$WIKI_REPO" rev-parse --verify HEAD >/dev/null 2>&1; then
    git -C "$WIKI_REPO" fetch origin
    git -C "$WIKI_REPO" pull --ff-only origin main
  elif git -C "$WIKI_REPO" ls-remote --exit-code --heads origin main >/dev/null 2>&1; then
    git -C "$WIKI_REPO" fetch origin main
    git -C "$WIKI_REPO" checkout -B main origin/main
  fi
fi

if [[ -n "$(git -C "$WIKI_REPO" status --porcelain=v1)" ]]; then
  echo "Bong-wiki 工作区有未提交改动，拒绝覆盖：$WIKI_REPO" >&2
  exit 1
fi

if command -v gh >/dev/null 2>&1; then
  visibility=$(gh api repos/Kizunad/Bong-wiki --jq .visibility)
  visibility=${visibility^^}
  [[ "$visibility" == "PRIVATE" ]] || {
    echo "Bong-wiki 必须保持 private，当前为 $visibility" >&2
    exit 1
  }
fi

find "$WIKI_REPO" -mindepth 1 -maxdepth 1 ! -name .git ! -name README.md -exec rm -rf -- {} +
cp -a "$WIKI_DIR/." "$WIKI_REPO/"
if [[ ! -f "$WIKI_REPO/README.md" ]]; then
  cat >"$WIKI_REPO/README.md" <<'EOF'
# Bong 代码 Wiki

这是由 Bong 主仓 `scripts/wiki/build.sh` 生成的静态 HTML 代码文档，不要手工修改。

在本地克隆后直接打开 `index.html`；页面和三端文档均支持 `file://` 离线查看。
EOF
fi

source_sha=$(git -C "$ROOT" rev-parse HEAD)
git -C "$WIKI_REPO" add -A
if git -C "$WIKI_REPO" diff --cached --quiet; then
  echo "wiki 没有新的生成物"
  exit 0
fi
git -C "$WIKI_REPO" commit -m "按 Bong $source_sha 重新生成 wiki"
git -C "$WIKI_REPO" push origin HEAD:main
