#!/usr/bin/env bash
# build-token.sh 契约 pin：固定容量/锁域、跨进程 counting token、崩溃释放与 argv 透传。
set -euo pipefail

TOKEN=$(realpath "$(dirname "$0")/../build-token.sh")
SANDBOX=$(mktemp -d /tmp/build-token-test.XXXXXX)
FAKE_REPO=$(mktemp -d /tmp/build-token-root-test.XXXXXX)
FAKE_REPO_B=$(mktemp -d /tmp/build-token-root-test-b.XXXXXX)
mkdir -p "$FAKE_REPO/scripts" "$FAKE_REPO/server" "$FAKE_REPO/client"
mkdir -p "$FAKE_REPO_B/scripts" "$FAKE_REPO_B/server" "$FAKE_REPO_B/client"
cp "$TOKEN" "$FAKE_REPO/scripts/build-token.sh"
cp "$TOKEN" "$FAKE_REPO_B/scripts/build-token.sh"
chmod +x "$FAKE_REPO/scripts/build-token.sh" "$FAKE_REPO_B/scripts/build-token.sh"
PIDS=()
cleanup() {
  trap - EXIT
  local pid
  for pid in "${PIDS[@]}"; do
    kill "$pid" 2>/dev/null || true
  done
  wait 2>/dev/null || true
  rm -rf "$SANDBOX" "$FAKE_REPO" "$FAKE_REPO_B"
}
trap cleanup EXIT

PASS=0
FAIL=0
pass() { printf '  PASS: %s\n' "$1"; PASS=$((PASS + 1)); }
fail() { printf '  FAIL: %s\n' "$1"; FAIL=$((FAIL + 1)); }

mkdir -p "$SANDBOX/bin" "$SANDBOX/locks"
chmod 700 "$SANDBOX/locks"
cat >"$SANDBOX/bin/cargo" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
name=$1
shift
if [[ $name == exit_75 ]]; then
  exit 75
fi
{
  printf 'start %s %s cwd=' "$name" "$(date +%s%N)"
  printf '%q' "$PWD"
  printf ' argv='
  printf '<%s>' "$@"
  printf '\n'
} >>"$BUILD_TOKEN_TEST_LOG"
printf '%s\0' "$@" >"$BUILD_TOKEN_TEST_DIR/$name.argv"
if [[ -e /proc/$$/fd/9 ]]; then
  printf 'build child inherited token fd 9\n' >&2
  exit 97
fi
printf '%s\n' "$$" >"$BUILD_TOKEN_TEST_DIR/$name.pid"
printf '%s\n' "$PPID" >"$BUILD_TOKEN_TEST_DIR/$name.parent"
touch "$BUILD_TOKEN_TEST_DIR/$name.started"
if [[ ${BUILD_TOKEN_TEST_SPAWN_DESCENDANT:-0} == 1 ]]; then
  (sleep 30) &
  printf '%s\n' "$!" >"$BUILD_TOKEN_TEST_DIR/$name.descendant"
fi
while [ ! -f "$BUILD_TOKEN_TEST_DIR/$name.release" ]; do sleep 0.02; done
printf 'end %s %s\n' "$name" "$(date +%s%N)" >>"$BUILD_TOKEN_TEST_LOG"
touch "$BUILD_TOKEN_TEST_DIR/$name.finished"
EOF
cat >"$SANDBOX/gradlew" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
name=$1
shift
printf 'start %s %s\n' "$name" "$(date +%s%N)" >>"$BUILD_TOKEN_TEST_LOG"
printf '%s\0' "$@" >"$BUILD_TOKEN_TEST_DIR/$name.argv"
if [[ -e /proc/$$/fd/9 ]]; then
  printf 'build child inherited token fd 9\n' >&2
  exit 97
fi
printf '%s\n' "$$" >"$BUILD_TOKEN_TEST_DIR/$name.pid"
printf '%s\n' "$PPID" >"$BUILD_TOKEN_TEST_DIR/$name.parent"
touch "$BUILD_TOKEN_TEST_DIR/$name.started"
while [ ! -f "$BUILD_TOKEN_TEST_DIR/$name.release" ]; do sleep 0.02; done
printf 'end %s %s\n' "$name" "$(date +%s%N)" >>"$BUILD_TOKEN_TEST_LOG"
touch "$BUILD_TOKEN_TEST_DIR/$name.finished"
EOF
cat >"$FAKE_REPO/server/cargo" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$PWD" >"$BUILD_TOKEN_TEST_DIR/root-cargo.cwd"
if [[ ${1:-} == root_cargo ]]; then
  exit 0
fi
exec "$BUILD_TOKEN_TEST_DIR/bin/cargo" "$@"
EOF
cat >"$FAKE_REPO/client/gradlew" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$PWD" >"$BUILD_TOKEN_TEST_DIR/root-gradle.cwd"
if [[ ${1:-} == root_gradle ]]; then
  exit 0
fi
exec "$BUILD_TOKEN_TEST_DIR/gradlew" "$@"
EOF
cp "$FAKE_REPO/client/gradlew" "$FAKE_REPO_B/client/gradlew"
chmod +x "$SANDBOX/bin/cargo" "$SANDBOX/gradlew" "$FAKE_REPO/server/cargo" "$FAKE_REPO/client/gradlew" "$FAKE_REPO_B/client/gradlew"
export BUILD_TOKEN_TEST_DIR="$SANDBOX"
export BUILD_TOKEN_TEST_LOG="$SANDBOX/events.log"
export BONG_BUILD_TOKEN_TEST_MODE=1
export BONG_BUILD_TOKEN_DIR="$SANDBOX/locks"
TEST_TOKEN="$FAKE_REPO/scripts/build-token.sh"
ORIGINAL_PATH=$PATH
if ! (cd "$FAKE_REPO" && PATH="$FAKE_REPO/server:$ORIGINAL_PATH" "$TEST_TOKEN" cargo root_cargo) >"$SANDBOX/root-cargo.out" 2>"$SANDBOX/root-cargo.err"; then
  fail "根目录 cargo 调用未能到达 server 构建根目录"
fi
if ! (cd "$FAKE_REPO" && PATH="$ORIGINAL_PATH" "$TEST_TOKEN" gradle root_gradle) >"$SANDBOX/root-gradle.out" 2>"$SANDBOX/root-gradle.err"; then
  fail "根目录 gradle 调用未能到达 client 构建根目录"
fi
export PATH="$FAKE_REPO/server:$ORIGINAL_PATH"
[ "$(cat "$SANDBOX/root-cargo.cwd")" = "$FAKE_REPO/server" ] || fail "根目录 cargo 调用未切到 server"
[ "$(cat "$SANDBOX/root-gradle.cwd")" = "$FAKE_REPO/client" ] || fail "根目录 gradle 调用未切到 client"
TOKEN="$TEST_TOKEN"

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
process_is_running() {
  local state
  state=$(ps -o stat= -p "$1" 2>/dev/null || true)
  [[ -n "$state" && "$state" != Z* ]]
}
start_cargo() {
  local name=$1
  shift
  (cd "$SANDBOX" && "$TOKEN" cargo "$name" "$@") >"$SANDBOX/$name.out" 2>"$SANDBOX/$name.err" &
  PIDS+=("$!")
}
start_gradle() {
  local name=$1
  shift
  (cd "$SANDBOX" && "$TOKEN" gradle "$name" "$@") >"$SANDBOX/$name.out" 2>"$SANDBOX/$name.err" &
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
if grep -q "均在使用，等待可用令牌" "$SANDBOX/cargo_c.err"; then
  pass "等待者输出一次可行动的等待日志"
else
  fail "等待者缺少容量已满日志"
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

start_cargo argv_case "" "two words" -- "--literal=*"
if wait_file "$SANDBOX/argv_case.started" && python3 - "$SANDBOX/argv_case.argv" <<'PY'
import pathlib, sys
actual = pathlib.Path(sys.argv[1]).read_bytes().split(b"\0")[:-1]
expected = [b"", b"two words", b"--", b"--literal=*"]
raise SystemExit(0 if actual == expected else 1)
PY
then
  pass "空参数、空格、-- 与通配字面量逐项透传"
else
  fail "cargo argv 透传发生漂移"
fi
touch "$SANDBOX/argv_case.release"
wait "${PIDS[-1]}"
PIDS=()

set +e
(cd "$SANDBOX" && "$TOKEN" cargo exit_75) >"$SANDBOX/exit_75.out" 2>"$SANDBOX/exit_75.err"
exit_75_status=$?
set -e
if ((exit_75_status == 75)) && grep -q "构建命令返回 75" "$SANDBOX/exit_75.err"; then
  pass "真实构建退出码 75 原样透传且不误判槽位冲突"
else
  fail "真实构建退出码 75 被吞掉或误判为等待"
fi

start_cargo crash_holder
start_cargo survivor
if wait_file "$SANDBOX/crash_holder.started" && wait_file "$SANDBOX/survivor.started"; then
  start_cargo crash_waiter
  if not_started_briefly "$SANDBOX/crash_waiter.started"; then
    crash_wrapper_pid="$(cat "$SANDBOX/crash_holder.parent")"
    crash_child_pid="$(cat "$SANDBOX/crash_holder.pid")"
    kill -9 "$crash_wrapper_pid"
    wait "$crash_wrapper_pid" 2>/dev/null || true
    if wait_file "$SANDBOX/crash_waiter.started"; then
      pass "持锁 wrapper 被 SIGKILL 后等待者立即获得槽位"
    else
      fail "SIGKILL 后槽位未自动释放"
    fi
    if process_is_running "$crash_child_pid"; then
      pass "orphan build 可存活但因 FD 9 隔离不再持有 token"
      touch "$SANDBOX/crash_holder.release"
      wait_file "$SANDBOX/crash_holder.finished" || fail "orphan build 未能正常结束"
    else
      pass "wrapped command 已随 wrapper 结束"
    fi
  else
    fail "崩溃测试等待者未被双槽位阻塞"
  fi
else
  fail "崩溃测试持锁进程未进入"
fi
for name in survivor crash_waiter; do touch "$SANDBOX/$name.release"; done
for pid in "${PIDS[@]:1}"; do wait "$pid" 2>/dev/null || true; done
PIDS=()

if find "$SANDBOX/locks" -maxdepth 1 -name '.acquired-*' -print -quit | grep -q .; then
  fail "wrapper 崩溃后不得留下 acquisition marker"
else
  pass "token acquisition 不依赖持久 marker 文件"
fi

start_cargo cwd_first
start_cargo cwd_occupant
if wait_file "$SANDBOX/cwd_first.started" && wait_file "$SANDBOX/cwd_occupant.started"; then
  other="$SANDBOX/other-worktree"
  mkdir "$other"
  (cd "$other" && "$TOKEN" cargo cwd_second) >"$SANDBOX/cwd_second.out" 2>"$SANDBOX/cwd_second.err" &
  PIDS+=("$!")
  if not_started_briefly "$SANDBOX/cwd_second.started"; then
    pass "不同 cwd/worktree 仍共享同一默认测试锁池"
  else
    fail "不同 cwd 错误分裂了锁池"
  fi
else
  fail "cwd 共享测试两个持锁进程未同时启动"
fi
for name in cwd_first cwd_occupant cwd_second; do touch "$SANDBOX/$name.release"; done
for pid in "${PIDS[@]}"; do wait "$pid" 2>/dev/null || true; done
PIDS=()

rm -f "$SANDBOX/production_first.started" "$SANDBOX/production_first.release" \
  "$SANDBOX/production_second.started" "$SANDBOX/production_second.release"
(
  cd "$FAKE_REPO"
  env -u BONG_BUILD_TOKEN_DIR \
    BONG_BUILD_TOKEN_TEST_MODE=0 \
    BUILD_TOKEN_TEST_DIR="$SANDBOX" \
    BUILD_TOKEN_TEST_LOG="$BUILD_TOKEN_TEST_LOG" \
    "$FAKE_REPO/scripts/build-token.sh" gradle production_first
) >"$SANDBOX/production_first.out" 2>"$SANDBOX/production_first.err" &
PIDS+=("$!")
if wait_file "$SANDBOX/production_first.started"; then
  (
    cd "$FAKE_REPO_B"
    env -u BONG_BUILD_TOKEN_DIR \
      BONG_BUILD_TOKEN_TEST_MODE=0 \
      BUILD_TOKEN_TEST_DIR="$SANDBOX" \
      BUILD_TOKEN_TEST_LOG="$BUILD_TOKEN_TEST_LOG" \
      "$FAKE_REPO_B/scripts/build-token.sh" gradle production_second
  ) >"$SANDBOX/production_second.out" 2>"$SANDBOX/production_second.err" &
  PIDS+=("$!")
  if not_started_briefly "$SANDBOX/production_second.started"; then
    pass "不同 worktree 在 production lock domain 共享 gradle 槽位"
  else
    fail "production lock domain 被不同 worktree 错误分裂"
  fi
  touch "$SANDBOX/production_first.release"
  if wait_file "$SANDBOX/production_second.started"; then
    pass "production lock domain 槽位释放后允许第二个 worktree 进入"
  else
    fail "production lock domain 第二个 worktree 未在释放后进入"
  fi
  touch "$SANDBOX/production_second.release"
else
  fail "production lock domain 首个 worktree 未获得 gradle 槽位"
fi
for pid in "${PIDS[@]}"; do wait "$pid" 2>/dev/null || true; done
PIDS=()

if "$TOKEN" nope test >"$SANDBOX/unknown.out" 2>"$SANDBOX/unknown.err"; then
  fail "未知构建器应 fail closed"
elif grep -q "仅接受 cargo 或 gradle" "$SANDBOX/unknown.err"; then
  pass "未知构建器 fail closed 且给出用法"
else
  fail "未知构建器错误信息缺少修复线索"
fi

if BONG_BUILD_TOKEN_CARGO_SLOTS=3 "$TOKEN" cargo invalid >"$SANDBOX/slots.out" 2>"$SANDBOX/slots.err"; then
  fail "扩大 cargo 容量必须被拒绝"
elif grep -q "固定为 cargo=2" "$SANDBOX/slots.err"; then
  pass "环境变量不能扩大生产槽位上限"
else
  fail "容量覆写拒绝缺少修复线索"
fi

if BONG_BUILD_TOKEN_TEST_MODE=0 BONG_BUILD_TOKEN_DIR="$SANDBOX/other-locks" "$TOKEN" cargo invalid >"$SANDBOX/root.out" 2>"$SANDBOX/root.err"; then
  fail "生产调用改变共享锁域必须被拒绝"
elif grep -q "生产锁域固定" "$SANDBOX/root.err"; then
  pass "生产调用不能分裂共享锁域"
else
  fail "锁域覆写拒绝缺少修复线索"
fi

artifact_target="$SANDBOX/artifact-target"
artifact_output_dir="$SANDBOX/artifact-output"
mkdir -p "$artifact_target/release" "$artifact_output_dir"
chmod 700 "$artifact_output_dir"
printf 'exact-build-artifact\n' >"$artifact_target/release/bong-server"
chmod 700 "$artifact_target/release/bong-server"

for name in artifact_reader_a artifact_reader_b; do
  (
    cd "$SANDBOX"
    CARGO_TARGET_DIR="$artifact_target" "$TOKEN" cargo "$name"
  ) >"$SANDBOX/$name.out" 2>"$SANDBOX/$name.err" &
  PIDS+=("$!")
done
if wait_file "$SANDBOX/artifact_reader_a.started" \
  && wait_file "$SANDBOX/artifact_reader_b.started"; then
  (
    cd "$SANDBOX"
    CARGO_TARGET_DIR="$artifact_target" \
      BONG_BUILD_TOKEN_SERVER_ARTIFACT="$artifact_output_dir/bong-server" \
      "$TOKEN" cargo artifact_export
  ) >"$SANDBOX/artifact_export.out" 2>"$SANDBOX/artifact_export.err" &
  PIDS+=("$!")
  if not_started_briefly "$SANDBOX/artifact_export.started"; then
    pass "artifact 构建以 target 独占锁等待同 target 普通构建退出"
  else
    fail "artifact 构建未隔离同 target 并发写入"
  fi
  touch "$SANDBOX/artifact_reader_a.release" "$SANDBOX/artifact_reader_b.release"
  if wait_file "$SANDBOX/artifact_export.started"; then
    touch "$SANDBOX/artifact_export.release"
  else
    fail "同 target 普通构建退出后 artifact 构建未获得独占锁"
  fi
else
  fail "同 target 普通 cargo 构建未保持共享锁并发"
fi
for pid in "${PIDS[@]}"; do wait "$pid" 2>/dev/null || true; done
PIDS=()
if [ "$(cat "$artifact_output_dir/bong-server" 2>/dev/null || true)" = exact-build-artifact ] \
  && [ "$(stat -c %a "$artifact_output_dir/bong-server" 2>/dev/null || true)" = 700 ]; then
  pass "artifact 在 target 独占锁内原子复制为私有可执行文件"
else
  fail "artifact 构建未输出精确的私有可执行副本"
fi

chmod 755 "$artifact_output_dir"
CARGO_TARGET_DIR="$artifact_target" \
  BONG_BUILD_TOKEN_SERVER_ARTIFACT="$artifact_output_dir/rejected" \
  "$TOKEN" cargo artifact_bad_dir >"$SANDBOX/artifact_bad_dir.out" 2>"$SANDBOX/artifact_bad_dir.err" &
bad_artifact_pid=$!
wait_file "$SANDBOX/artifact_bad_dir.started" || true
touch "$SANDBOX/artifact_bad_dir.release"
if wait "$bad_artifact_pid"; then
  fail "宽权限 artifact 目录必须 fail closed"
elif [ ! -e "$artifact_output_dir/rejected" ]; then
  pass "artifact 拒绝写入非 0700 目标目录"
else
  fail "artifact 目录校验失败后仍产生了输出"
fi
chmod 700 "$artifact_output_dir"

symlink_root="$SANDBOX/symlink-locks"
ln -s "$SANDBOX/locks" "$symlink_root"
if BONG_BUILD_TOKEN_DIR="$symlink_root" "$TOKEN" cargo invalid >"$SANDBOX/symlink.out" 2>"$SANDBOX/symlink.err"; then
  fail "符号链接锁目录必须被拒绝"
elif grep -q "不得是符号链接" "$SANDBOX/symlink.err"; then
  pass "锁目录 symlink fail closed"
else
  fail "锁目录 symlink 拒绝缺少修复线索"
fi

hardlink_root="$SANDBOX/hardlink-locks"
mkdir "$hardlink_root"
chmod 700 "$hardlink_root"
printf 'victim-content\n' >"$SANDBOX/hardlink-victim"
chmod 640 "$SANDBOX/hardlink-victim"
ln "$SANDBOX/hardlink-victim" "$hardlink_root/cargo-1.lock"
if BONG_BUILD_TOKEN_DIR="$hardlink_root" "$TOKEN" cargo invalid >"$SANDBOX/hardlink.out" 2>"$SANDBOX/hardlink.err"; then
  fail "hard-link slot lock 必须被拒绝"
elif [ "$(stat -c %a "$SANDBOX/hardlink-victim")" != 640 ] \
  || [ "$(cat "$SANDBOX/hardlink-victim")" != victim-content ]; then
  fail "拒绝 hard-link lock 前不得修改 victim mode/content"
elif grep -q "单链接普通文件" "$SANDBOX/hardlink.err"; then
  pass "hard-link lock fail closed 且 victim 未被修改"
else
  fail "hard-link lock 拒绝缺少修复线索"
fi

printf '%s\n' '---'
printf 'PASS=%d FAIL=%d\n' "$PASS" "$FAIL"
((FAIL == 0))
printf 'build-token 契约测试全部通过\n'
