#!/bin/bash
# Inventory snapshot smoke test, authoritative proof
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PASS=0
FAIL=0

pass() { echo "  ✓ $1"; ((PASS+=1)); }
fail() { echo "  ✗ $1"; ((FAIL+=1)); }

echo "=== [1/4] Schema checks ==="
cd "$ROOT/agent/packages/schema"
if npm run check >/tmp/bong-schema-check.log 2>&1 && npm test >/tmp/bong-schema-test.log 2>&1 && npm run generate >/tmp/bong-schema-generate.log 2>&1; then
    pass "schema check + test + generate"
else
    tail -40 /tmp/bong-schema-check.log || true
    tail -40 /tmp/bong-schema-test.log || true
    tail -40 /tmp/bong-schema-generate.log || true
    fail "schema check + test + generate"
fi

echo ""
echo "=== [2/4] Server targeted checks ==="
cd "$ROOT/server"
if cargo test inventory_snapshot_emit >/tmp/bong-server-inventory.log 2>&1; then pass "cargo test inventory_snapshot_emit"; else tail -40 /tmp/bong-server-inventory.log; fail "cargo test inventory_snapshot_emit"; fi

echo ""
echo "=== [3/4] Client targeted checks ==="
cd "$ROOT/client"
if JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 PATH=/usr/lib/jvm/java-17-openjdk-amd64/bin:$PATH ./gradlew test --tests "*AuthoritativeInspectOpenTest" >/tmp/bong-client-authoritative-open.log 2>&1; then pass "AuthoritativeInspectOpenTest"; else tail -40 /tmp/bong-client-authoritative-open.log; fail "AuthoritativeInspectOpenTest"; fi

echo ""
echo "=== [4/4] Broader build gates ==="
cd "$ROOT/server"
if cargo check >/tmp/bong-server-check.log 2>&1 && cargo test >/tmp/bong-server-test.log 2>&1; then
    pass "server check + test"
else
    tail -40 /tmp/bong-server-check.log || true
    tail -40 /tmp/bong-server-test.log || true
    fail "server check + test"
fi

cd "$ROOT/client"
if JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 PATH=/usr/lib/jvm/java-17-openjdk-amd64/bin:$PATH ./gradlew test build >/tmp/bong-client-build.log 2>&1; then
    pass "client test build"
else
    tail -40 /tmp/bong-client-build.log
    fail "client test build"
fi

echo ""
echo "================================"
echo "Result: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ] && echo "ALL PASS" || echo "SOME FAILURES"
exit "$FAIL"
