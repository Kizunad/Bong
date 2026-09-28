#!/usr/bin/env bash
# 本地炼丹验收服务端；Windows Native 客户端在服务端 ready 后另行启动。
# 用法：bash scripts/alchemy-test-server.sh
# 资源包须由本地 HTTP 服务提供（默认 127.0.0.1:18765）。
set -euo pipefail

if [[ "$#" -gt 0 ]]; then
    echo "用法：bash scripts/alchemy-test-server.sh" >&2
    exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT/server"

# 使用当前工作树的构建目录，避免误启动另一工作树的旧二进制。
export CARGO_TARGET_DIR="$ROOT/server/target"
bash "$ROOT/scripts/build-token.sh" cargo build --bin bong-server

# /scene 由 TEST_ENV 单独注册；OP 或 DEV_MODE 均不能替代此开关。
# offline 模式还需显式信任本地验收账号，客户端才能收到管理命令树。
echo "[alchemy-test] 已启用 /scene；OP：${BONG_OPERATORS:-炼丹验收}"
BONG_TEST_ENV=1 \
BONG_OPERATORS="${BONG_OPERATORS:-炼丹验收}" \
BONG_OPERATORS_ALLOW_OFFLINE=1 \
BONG_SKIP_SKIN_PREFETCH=1 \
BONG_RESOURCE_PACK_URL="${BONG_RESOURCE_PACK_URL:-http://127.0.0.1:18765/bong-full-v1.zip}" \
    exec "$CARGO_TARGET_DIR/debug/bong-server"
