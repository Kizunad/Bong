#!/usr/bin/env bash
set -euo pipefail

# 三端文档由各自的标准工具生成；本脚本只负责编排和统一入口，不解析业务源代码。
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)"
WIKI_DIR="${BONG_WIKI_OUTPUT:-$ROOT/tmp/wiki}"
WORK_DIR="${BONG_WIKI_WORK:-$ROOT/tmp/wiki-work}"
TYPEDOC_VERSION="${BONG_TYPEDOC_VERSION:-0.27.9}"

stage_start=$SECONDS
run_stage() {
  local name=$1
  shift
  local started=$SECONDS
  printf '[wiki] %-12s 开始\n' "$name"
  "$@"
  printf '[wiki] %-12s 完成（%ss）\n' "$name" "$((SECONDS - started))"
}

rm -rf -- "$WIKI_DIR" "$WORK_DIR"
mkdir -p -- "$WIKI_DIR/server" "$WIKI_DIR/client" "$WIKI_DIR/agent" "$WORK_DIR"

run_stage rustdoc bash -c '
  cd "$1"
  "$1/scripts/build-token.sh" cargo doc --no-deps --target-dir "$2/server-target"
' _ "$ROOT" "$WORK_DIR"
cp -a "$WORK_DIR/server-target/doc/." "$WIKI_DIR/server/"

run_stage javadoc bash -c '
  cd "$1/client"
  "$1/scripts/build-token.sh" gradle javadoc
' _ "$ROOT"
cp -a "$ROOT/client/build/docs/javadoc/." "$WIKI_DIR/client/"

run_stage typedoc bash -c '
  cd "$1/agent"
  mkdir -p "$2/agent/schema" "$2/agent/tiandao"
  npx --yes "typedoc@$3" \
    --tsconfig packages/schema/tsconfig.json \
    --entryPointStrategy expand \
    --entryPoints packages/schema/src/index.ts \
    --out "$2/agent/schema" \
    --exclude "**/*.test.ts" \
    --exclude "**/*.spec.ts" \
    --skipErrorChecking
  npx --yes "typedoc@$3" \
    --tsconfig packages/tiandao/tsconfig.json \
    --entryPointStrategy expand \
    --entryPoints packages/tiandao/src/main.ts \
    --out "$2/agent/tiandao" \
    --exclude "**/*.test.ts" \
    --exclude "**/*.spec.ts" \
    --skipErrorChecking
  cat >"$2/agent/index.html" <<EOF
<!doctype html><meta charset="utf-8"><title>Agent TypeDoc</title>
<h1>Agent TypeDoc</h1>
<ul><li><a href="schema/index.html">@bong/schema</a></li>
<li><a href="tiandao/index.html">@bong/tiandao</a></li></ul>
EOF
' _ "$ROOT" "$WIKI_DIR" "$TYPEDOC_VERSION"

run_stage index python3 "$ROOT/scripts/wiki/index.py" --output "$WIKI_DIR"

printf '[wiki] output=%s size=' "$WIKI_DIR"
du -sh -- "$WIKI_DIR" | awk '{print $1}'
printf '[wiki] total elapsed=%ss\n' "$((SECONDS - stage_start))"
