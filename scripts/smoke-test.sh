#!/bin/bash
# MVP 0.1 Smoke Test — 本地一键验证
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PASS=0
FAIL=0
export BONG_SKIP_SKIN_PREFETCH="${BONG_SKIP_SKIN_PREFETCH:-1}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/server/target}"
# 相对 target 目录按构建起点 $ROOT/server 归一化，避免 launch 阶段按错误目录解析
if [[ "$CARGO_TARGET_DIR" != /* ]]; then
    CARGO_TARGET_DIR="$ROOT/server/$CARGO_TARGET_DIR"
fi
FALLBACK_WORLD_READY_PATTERN='\[bong\]\[world\] BOT_FALLBACK_FLAT_READY anchors=[0-9]+ chunks=[0-9]+ view_distance_chunks=[0-9]+'

pass() { echo "  ✓ $1"; ((PASS+=1)); }
fail() { echo "  ✗ $1"; ((FAIL+=1)); }

echo "=== [1/4] Rust fmt + clippy ==="
cd "$ROOT/server"
if "$ROOT/scripts/build-token.sh" cargo fmt --check 2>/dev/null; then pass "cargo fmt"; else fail "cargo fmt"; fi
if "$ROOT/scripts/build-token.sh" cargo clippy --all-targets -- -D warnings 2>/dev/null; then pass "clippy"; else fail "clippy"; fi

echo ""
echo "=== [2/4] Rust tests ==="
if "$ROOT/scripts/build-token.sh" cargo test 2>&1 | tee /tmp/bong-test.log | tail -5; then pass "cargo test"; else fail "cargo test"; fi

echo ""
echo "=== [3/4] Server smoke run (30s) ==="
if "$ROOT/scripts/build-token.sh" cargo build 2>/dev/null; then
    pass "cargo build"
    timeout 30s "$CARGO_TARGET_DIR/debug/bong-server" 2>&1 | tee /tmp/bong-smoke.log || true
else
    fail "cargo build"
    : >/tmp/bong-smoke.log
fi
grep -q "\[bong\]\[bridge\] tokio runtime started" /tmp/bong-smoke.log && pass "bridge startup" || fail "bridge startup"
grep -Eq "$FALLBACK_WORLD_READY_PATTERN" /tmp/bong-smoke.log && pass "world creation" || fail "world creation"
grep -q "\[bong\]\[player\] registering player init/cleanup systems" /tmp/bong-smoke.log && pass "player system" || fail "player system"
grep -q "\[bong\]\[redis\] connecting to" /tmp/bong-smoke.log && pass "redis bridge startup" || fail "redis bridge startup"

echo ""
echo "=== [4/4] Fabric client build ==="
if [ -f "$ROOT/client/gradlew" ]; then
    cd "$ROOT/client"
    if [ -d "/usr/lib/jvm/java-17-openjdk-amd64" ]; then
        export JAVA_HOME="/usr/lib/jvm/java-17-openjdk-amd64"
        export PATH="$JAVA_HOME/bin:$PATH"
    fi
    export GRADLE_USER_HOME="${GRADLE_USER_HOME:-/tmp/bong-gradle}"
    if "$ROOT/scripts/build-token.sh" gradle test build 2>&1 | tail -10; then
        pass "gradlew test build"
        JAR=$(find build/libs -name "*.jar" -not -name "*-sources*" 2>/dev/null | head -1)
        if [ -n "$JAR" ]; then pass "jar: $JAR"; else fail "no jar produced"; fi
    else
        fail "gradlew test build"
    fi
else
    echo "  - client/gradlew not found, skipping (Task 9 not yet landed)"
fi

echo ""
echo "================================"
echo "Result: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ] && echo "ALL PASS" || echo "SOME FAILURES"
exit "$FAIL"
