#!/usr/bin/env bash
# build-token.sh 契约 pin：跨进程 counting token、不同构建器隔离与参数透传。
set -euo pipefail

TOKEN=$(realpath "$(dirname "$0")/../build-token.sh")
SANDBOX=$(mktemp -d /tmp/build-token-test.XXXXXX)
PIDS=()
cleanup() {
  trap - EXIT
  local pid
  for pid in "${PIDS[@]}"; do
    kill "$pid" 2>/dev/null || true
  done
  wait 2>/dev/null || true
  rm -rf "$SANDBOX"
}
trap cleanup EXIT

PASS=0
FAIL=0
pass() { printf '  PASS: %s\n' "$1"; PASS=$((PASS + 1)); }
fail() { printf '  FAIL: %s\n' "$1"; FAIL=$((FAIL + 1)); }

mkdir -p "$SANDBOX/bin" "$SANDBOX/locks"
cat >"$SANDBOX/bin/cargo" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf 'start %s %s\n' "$1" "$(date +%s%N)" >>"$BUILD_TOKEN_TEST_LOG"
touch "$BUILD_TOKEN_TEST_DIR/$1.started"
while [ ! -f "$BUILD_TOKEN_TEST_DIR/$1.release" ]; do sleep 0.02; done
printf 'end %s %s\n' "$1" "$(date +%s%N)" >>"$BUILD_TOKEN_TEST_LOG"
EOF
cat >"$SANDBOX/gradlew" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf 'start %s %s\n' "$1" "$(date +%s%N)" >>"$BUILD_TOKEN_TEST_LOG"
touch "$BUILD_TOKEN_TEST_DIR/$1.started"
while [ ! -f "$BUILD_TOKEN_TEST_DIR/$1.release" ]; do sleep 0.02; done
printf 'end %s %s\n' "$1" "$(date +%s%N)" >>"$BUILD_TOKEN_TEST_LOG"
EOF
chmod +x "$SANDBOX/bin/cargo" "$SANDBOX/gradlew"

export PATH="$SANDBOX/bin:$PATH"
export BUILD_TOKEN_TEST_DIR="$SANDBOX"
export BUILD_TOKEN_TEST_LOG="$SANDBOX/events.log"
export BONG_BUILD_TOKEN_DIR="$SANDBOX/locks"

wait_file() {
  local file=$1
  for _ in $(seq 1 200); do
    [ -f "$file" ] && return 0
    sleep 0.02
  done
  return 1
}
not_started_briefly() {
  local file=$1
  for _ in $(seq 1 15); do
    [ -f "$file" ] && return 1
    sleep 0.02
  done
  return 0
}
start_cargo() {
  (cd "$SANDBOX" && "$TOKEN" cargo "$1") >"$SANDBOX/$1.out" 2>"$SANDBOX/$1.err" &
  PIDS+=("$!")
}
start_gradle() {
  (cd "$SANDBOX" && "$TOKEN" gradle "$1") >"$SANDBOX/$1.out" 2>"$SANDBOX/$1.err" &
  PIDS+=("$!")
}

start_cargo cargo_a
start_cargo cargo_b
if wait_file "$SANDBOX/cargo_a.started" && wait_file "$SANDBOX/cargo_b.started"; then
  pass "cargo 允许两个进程并发获得槽位"
else
  fail "cargo 两个并发槽位未同时启动"
fi

start_cargo cargo_c
if not_started_briefly "$SANDBOX/cargo_c.started"; then
  pass "第三个 cargo 在两个槽位占满时等待"
else
  fail "第三个 cargo 绕过了 counting token"
fi

touch "$SANDBOX/cargo_a.release"
if wait_file "$SANDBOX/cargo_c.started"; then
  pass "cargo 槽位释放后等待者自动进入"
else
  fail "cargo 等待者未在槽位释放后进入"
fi

start_gradle gradle_a
if wait_file "$SANDBOX/gradle_a.started"; then
  pass "gradle 与 cargo 使用独立令牌池"
else
  fail "cargo 占用错误阻塞了 gradle"
fi
start_gradle gradle_b
if not_started_briefly "$SANDBOX/gradle_b.started"; then
  pass "第二个 gradle 在单槽位占用时等待"
else
  fail "gradle 单槽位限制失效"
fi

touch "$SANDBOX/gradle_a.release"
if wait_file "$SANDBOX/gradle_b.started"; then
  pass "gradle 槽位释放后等待者自动进入"
else
  fail "gradle 等待者未在槽位释放后进入"
fi

for name in cargo_b cargo_c gradle_b; do touch "$SANDBOX/$name.release"; done
for pid in "${PIDS[@]}"; do wait "$pid"; done
PIDS=()

if "$TOKEN" nope test >"$SANDBOX/unknown.out" 2>"$SANDBOX/unknown.err"; then
  fail "未知构建器应 fail closed"
elif grep -q "仅接受 cargo 或 gradle" "$SANDBOX/unknown.err"; then
  pass "未知构建器 fail closed 且给出用法"
else
  fail "未知构建器错误信息缺少修复线索"
fi

if BONG_BUILD_TOKEN_CARGO_SLOTS=0 "$TOKEN" cargo invalid >"$SANDBOX/zero.out" 2>"$SANDBOX/zero.err"; then
  fail "零槽位配置应被拒绝"
elif grep -q "必须是正整数" "$SANDBOX/zero.err"; then
  pass "非法槽位数 fail closed"
else
  fail "非法槽位数错误信息缺少修复线索"
fi

printf '%s\n' '---'
printf 'PASS=%d FAIL=%d\n' "$PASS" "$FAIL"
((FAIL == 0))
printf 'build-token 契约测试全部通过\n'
