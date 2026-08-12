#!/bin/bash
# Inventory 完整闭环 smoke —— 比 smoke-inventory-snapshot.sh 更广：
# 覆盖 snapshot / discard / pickup / death-drop / dropped-loot sync / weight penalty /
# tooltip / world billboard 渲染相关代码的回归。
#
# 用法：bash scripts/smoke-inventory-complete.sh
# 预期：每项 PASS；有 FAIL 时下面会打印关联 log 的尾 40 行便于定位。
#
# 注意：这是自动化 gates。完整实机 QA 见 docs/plan-inventory-v1.md §7.5 Manual QA Checklist。

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PASS=0
FAIL=0
LOG_DIR="$(mktemp -d "${TMPDIR:-/tmp}/bong-inventory.XXXXXX")"
trap 'rm -rf -- "$LOG_DIR"' EXIT

pass() { echo "  ✓ $1"; ((PASS+=1)); }
fail() { echo "  ✗ $1"; ((FAIL+=1)); }

run_with_log() {
    local label="$1"
    local logfile="$2"
    shift 2
    if "$@" >"$logfile" 2>&1; then
        pass "$label"
    else
        tail -40 "$logfile" || true
        fail "$label"
    fi
}

echo "=== [1/4] Schema（TypeBox source of truth + samples）==="
cd "$ROOT/agent/packages/schema"
run_with_log "schema check" "$LOG_DIR/schema-check.log" npm run check
run_with_log "schema test" "$LOG_DIR/schema-test.log" npm test
run_with_log "schema generate" "$LOG_DIR/schema-generate.log" npm run generate

echo ""
echo "=== [2/4] Server inventory 模块 targeted tests ==="
cd "$ROOT/server"
# 构建令牌 wrapper cargo test 按 module path / function name 过滤。inventory:: 模块含所有 inventory/discard/
# pickup/death-drop/weight 测试；network::inventory_snapshot_emit 是单独 emit 模块。
run_with_log "cargo test inventory" "$LOG_DIR/server-inventory.log" "$ROOT/scripts/build-token.sh" cargo test --package bong-server inventory
run_with_log "cargo test inventory_snapshot_emit" "$LOG_DIR/server-inventory-emit.log" "$ROOT/scripts/build-token.sh" cargo test --package bong-server inventory_snapshot_emit
run_with_log "cargo test dropped_loot" "$LOG_DIR/server-dropped-loot.log" "$ROOT/scripts/build-token.sh" cargo test --package bong-server dropped_loot

echo ""
echo "=== [3/4] Client inventory + HUD targeted tests ==="
cd "$ROOT/client"
# DroppedItemStore tie-breaker + DroppedLootSyncHandler + InventoryEventHandler +
# InspectScreen apply-pill / move-intent + Overweight/DroppedItem HUD planner
run_with_log "client DroppedItem* tests" "$LOG_DIR/client-dropped.log" \
    "$ROOT/scripts/build-token.sh" gradle test --tests "*DroppedItem*"
run_with_log "client Inventory* tests" "$LOG_DIR/client-inventory.log" \
    "$ROOT/scripts/build-token.sh" gradle test --tests "*Inventory*"
run_with_log "client InspectScreen* tests" "$LOG_DIR/client-inspect.log" \
    "$ROOT/scripts/build-token.sh" gradle test --tests "*InspectScreen*"
run_with_log "client Overweight + BongHud tests" "$LOG_DIR/client-hud.log" \
    "$ROOT/scripts/build-token.sh" gradle test --tests "*Overweight*" --tests "*BongHud*"

echo ""
echo "=== [4/4] 全量 build gates（保护不破别的模块）==="
cd "$ROOT/server"
run_with_log "server cargo check" "$LOG_DIR/server-check.log" "$ROOT/scripts/build-token.sh" cargo check
run_with_log "server cargo test" "$LOG_DIR/server-test.log" "$ROOT/scripts/build-token.sh" cargo test

cd "$ROOT/client"
run_with_log "client gradle test build" "$LOG_DIR/client-build.log" "$ROOT/scripts/build-token.sh" gradle test build

echo ""
echo "=========================="
echo "PASS: $PASS   FAIL: $FAIL"
echo "=========================="
if [[ $FAIL -gt 0 ]]; then
    exit 1
fi
