#!/bin/bash
# MVP 0.1 Smoke Test — 本地一键验证
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PASS=0
FAIL=0

pass() { echo "  ✓ $1"; ((PASS++)); }
fail() { echo "  ✗ $1"; ((FAIL++)); }

echo "=== [1/4] Rust fmt + clippy ==="
cd "$ROOT/server"
if cargo fmt --check 2>/dev/null; then pass "cargo fmt"; else fail "cargo fmt"; fi
if cargo clippy --all-targets -- -D warnings 2>/dev/null; then pass "clippy"; else fail "clippy"; fi

echo ""
echo "=== [2/4] Rust tests ==="
if cargo test 2>&1 | tee /tmp/bong-test.log | tail -5; then pass "cargo test"; else fail "cargo test"; fi

echo ""
echo "=== [3/4] Server smoke run (15s) ==="
timeout 15s cargo run 2>&1 | tee /tmp/bong-smoke.log || true
grep -q "tokio runtime started" /tmp/bong-smoke.log && pass "bridge startup" || fail "bridge startup"
grep -q "creating overworld" /tmp/bong-smoke.log && pass "world creation" || fail "world creation"
grep -q "registering player" /tmp/bong-smoke.log && pass "player system" || fail "player system"
grep -q "spawned zombie npc" /tmp/bong-smoke.log && pass "npc spawn" || fail "npc spawn"

echo ""
echo "=== [4/4] Fabric client build ==="
if [ -f "$ROOT/client/gradlew" ]; then
    cd "$ROOT/client"
    if ./gradlew test build 2>&1 | tail -10; then
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
