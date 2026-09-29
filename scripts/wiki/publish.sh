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

if ! command -v gh >/dev/null 2>&1; then
  echo "需要 gh 才能验证目标 Wiki 仓库的 private 状态，拒绝发布" >&2
  exit 1
fi

require_private_github_remote() {
  local fetch_output push_output remote_url repo_path visibility
  local -a remote_urls

  fetch_output=$(git -C "$WIKI_REPO" remote get-url --all origin) || {
    echo "Bong-wiki 缺少 origin 远端，拒绝发布" >&2
    exit 1
  }
  push_output=$(git -C "$WIKI_REPO" remote get-url --push --all origin) || {
    echo "无法解析 Bong-wiki 的 push 远端，拒绝发布" >&2
    exit 1
  }
  mapfile -t remote_urls <<<"$fetch_output"
  while IFS= read -r remote_url; do
    remote_urls+=("$remote_url")
  done <<<"$push_output"

  if ((${#remote_urls[@]} == 0)); then
    echo "Bong-wiki 没有可验证的 origin 远端，拒绝发布" >&2
    exit 1
  fi

  for remote_url in "${remote_urls[@]}"; do
    case "$remote_url" in
      git@github.com:*)
        repo_path=${remote_url#git@github.com:}
        ;;
      ssh://git@github.com/*)
        repo_path=${remote_url#ssh://git@github.com/}
        ;;
      https://github.com/*|http://github.com/*)
        repo_path=${remote_url#*github.com/}
        ;;
      *)
        echo "目标 Wiki 远端必须是 github.com 仓库，实际为：$remote_url" >&2
        exit 1
        ;;
    esac
    repo_path=${repo_path%.git}
    if [[ ! "$repo_path" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]]; then
      echo "无法从 Wiki 远端解析安全的 GitHub 仓库路径：$remote_url" >&2
      exit 1
    fi
    if [[ "${repo_path,,}" != "kizunad/bong-wiki" ]]; then
      echo "Wiki 只能发布到 Kizunad/Bong-wiki，实际 origin 为：$repo_path" >&2
      exit 1
    fi
    if ! visibility=$(gh api "repos/$repo_path" --jq .visibility); then
      echo "无法验证 GitHub 仓库可见性，拒绝发布：$repo_path" >&2
      exit 1
    fi
    visibility=${visibility^^}
    [[ "$visibility" == "PRIVATE" ]] || {
      echo "Wiki 仓库必须保持 private，实际仓库 $repo_path 为 $visibility" >&2
      exit 1
    }
  done
}

if [[ ! -d "$WIKI_REPO/.git" ]]; then
  mkdir -p "$(dirname "$WIKI_REPO")"
  git clone "$REMOTE" "$WIKI_REPO"
fi

require_private_github_remote

if git -C "$WIKI_REPO" rev-parse --verify HEAD >/dev/null 2>&1; then
  git -C "$WIKI_REPO" fetch origin
  git -C "$WIKI_REPO" pull --ff-only origin main
elif git -C "$WIKI_REPO" ls-remote --exit-code --heads origin main >/dev/null 2>&1; then
  git -C "$WIKI_REPO" fetch origin main
  git -C "$WIKI_REPO" checkout -B main origin/main
fi

if [[ -n "$(git -C "$WIKI_REPO" status --porcelain=v1)" ]]; then
  echo "Bong-wiki 工作区有未提交改动，拒绝覆盖：$WIKI_REPO" >&2
  exit 1
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
