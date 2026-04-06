#!/bin/bash
# deploy.sh — 云端部署：拉取最新代码，安装依赖，编译
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# Rust 工具链（云端 taxi 用户需要）
export PATH="/opt/rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH"
# target/ 可能被 root 占用，用临时目录
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/bong-target}"

echo "=== [1/4] git pull ==="
git pull --ff-only || { echo "Pull failed — resolve conflicts first"; exit 1; }

echo ""
echo "=== [2/4] Rust server build ==="
cd "$ROOT/server"
cargo build --release 2>&1 | tail -5
echo "Server binary: $CARGO_TARGET_DIR/release/bong-server"

echo ""
echo "=== [3/4] Schema package ==="
cd "$ROOT/agent/packages/schema"
npm install --prefer-offline 2>&1 | tail -3
npm run build

echo ""
echo "=== [4/4] Tiandao agent ==="
cd "$ROOT/agent"
npm install --prefer-offline 2>&1 | tail -3

echo ""
echo "=== Deploy complete ==="
echo "Run: bash scripts/start.sh"
