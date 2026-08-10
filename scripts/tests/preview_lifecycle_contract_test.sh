#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
TMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/bong-preview-harness.XXXXXX")"
cleanup() {
  BONG_PREVIEW_PID_FILE="$TMP_ROOT/runtime/server.pid" \
    bash "$REPO_ROOT/scripts/preview/stop-server-headless.sh" >/dev/null 2>&1 || true
  rm -rf -- "$TMP_ROOT"
}
trap cleanup EXIT

mkdir -p "$TMP_ROOT/runtime" "$TMP_ROOT/target/release" "$TMP_ROOT/bin" "$TMP_ROOT/fake-src"
chmod 700 "$TMP_ROOT/runtime" "$TMP_ROOT/bin" "$TMP_ROOT/fake-src"
cat >"$TMP_ROOT/fake-src/server.c" <<'EOF'
#include <arpa/inet.h>
#include <signal.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/socket.h>

int main(void) {
    const char *mode = getenv("FAKE_SERVER_MODE");
    if (mode && strcmp(mode, "early") == 0) return 17;
    if (mode && strcmp(mode, "no_listener") == 0) { sleep(30); return 0; }
    int fd = socket(AF_INET, SOCK_STREAM, 0);
    if (fd < 0) return 18;
    int one = 1;
    setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &one, sizeof(one));
    struct sockaddr_in addr = {0};
    addr.sin_family = AF_INET;
    addr.sin_port = htons(25565);
    addr.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    if (bind(fd, (struct sockaddr *)&addr, sizeof(addr)) < 0) return 19;
    if (listen(fd, 4) < 0) return 20;
    for (;;) pause();
}
EOF
# Compile a native executable so /proc/<pid>/exe matches the selected artifact.
gcc "$TMP_ROOT/fake-src/server.c" -o "$TMP_ROOT/target/release/bong-server"
cat >"$TMP_ROOT/bin/cargo" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
exit 0
EOF
chmod 700 "$TMP_ROOT/bin/cargo"

export PATH="$TMP_ROOT/bin:$PATH"
export CARGO_TARGET_DIR="$TMP_ROOT/target"
export BONG_BUILD_TOKEN_TEST_MODE=1
export BONG_BUILD_TOKEN_DIR="$TMP_ROOT/build-token"
export BONG_PREVIEW_PID_FILE="$TMP_ROOT/runtime/server.pid"
export BONG_SKIP_SKIN_PREFETCH=1

run_preview() {
  bash "$REPO_ROOT/scripts/preview/run-server-headless.sh" --timeout "$1"
}

run_preview 5 >/dev/null
[ -f "$BONG_PREVIEW_PID_FILE" ]
bash "$REPO_ROOT/scripts/preview/stop-server-headless.sh"
[ ! -e "$BONG_PREVIEW_PID_FILE" ] && [ ! -L "$BONG_PREVIEW_PID_FILE" ]

export FAKE_SERVER_MODE=early
if run_preview 2 >"$TMP_ROOT/early.log" 2>&1; then
  echo "early-exit preview unexpectedly succeeded" >&2
  exit 1
fi
[ ! -e "$BONG_PREVIEW_PID_FILE" ] && [ ! -L "$BONG_PREVIEW_PID_FILE" ]
unset FAKE_SERVER_MODE

export FAKE_SERVER_MODE=no_listener
if run_preview 1 >"$TMP_ROOT/timeout.log" 2>&1; then
  echo "timeout preview unexpectedly succeeded" >&2
  exit 1
fi
[ ! -e "$BONG_PREVIEW_PID_FILE" ] && [ ! -L "$BONG_PREVIEW_PID_FILE" ]
unset FAKE_SERVER_MODE

REAL_STAT="$(command -v stat)"
cat >"$TMP_ROOT/bin/stat" <<EOF
#!/usr/bin/env bash
set -euo pipefail
# 三种注入模式，按测试分场景单独启用：
#   1. FAKE_STAT_IDENTITY_FAILURE=1       —— 对 /proc/<pid>/exe 持续失败
#   2. FAKE_STAT_FAIL_AFTER_RECORD=1      —— 记录发布后失败一次（标志文件保证只一次）
#   3. FAKE_STAT_FAIL_FROM_CALL=<n>       —— 第 n 次起对 /proc/<pid>/exe 持续失败
# 仅拦截 /proc/*/exe 的 shell stat；readlink（executable）、/proc/<pid>/stat
# 重定向读（starttime）、Python os.stat（pidfd-signal / listener-owner）均不受影响。
if [[ "\${*: -1}" != /proc/*/exe ]]; then
  exec "$REAL_STAT" "\$@"
fi
# 模式 1：身份收集全程不可确认（review finding：单次失败会被 500 次重试吞掉，
# 必须持续失败才能锁定「身份不可确认」的回收路径）。
if [ "\${FAKE_STAT_IDENTITY_FAILURE:-0}" = "1" ]; then
  exit 1
fi
# 模式 2：记录已发布后恰好失败一次——readiness 循环的 process_status 拿不到
# identity 返回 2，而回滚重查时已恢复，能确认身份、停服并清记录（review
# finding：status 2 也必须走 identity-safe 回滚，不能裸留进程+记录直接退出）。
if [ "\${FAKE_STAT_FAIL_AFTER_RECORD:-0}" = "1" ] \
    && [ -e "$TMP_ROOT/runtime/server.pid" ] \
    && [ ! -e "$TMP_ROOT/stat-post-record-fired" ]; then
  : > "$TMP_ROOT/stat-post-record-fired"
  exit 1
fi
# 模式 3：write_record 自身的 identity 读取失败，但 launch 循环的钉扎成功——
# 触发「记录发布失败后对有界回收」的恢复路径（review finding：旧实现把未确认
# 的 stop 结果只记日志就无限 wait，进程仍存活时永远挂死）。
if [ "\${FAKE_STAT_FAIL_FROM_CALL:-0}" -gt 0 ]; then
  count=0
  [ ! -f "$TMP_ROOT/stat-count" ] || read -r count <"$TMP_ROOT/stat-count"
  count=\$((count + 1))
  printf '%s\n' "\$count" >"$TMP_ROOT/stat-count"
  if [ "\$count" -ge "\$FAKE_STAT_FAIL_FROM_CALL" ]; then
    exit 1
  fi
fi
exec "$REAL_STAT" "\$@"
EOF
chmod 700 "$TMP_ROOT/bin/stat"
export FAKE_STAT_IDENTITY_FAILURE=1
if run_preview 2 >"$TMP_ROOT/identity.log" 2>&1; then
  echo "identity-inspection failure unexpectedly succeeded" >&2
  exit 1
fi
unset FAKE_STAT_IDENTITY_FAILURE
[ ! -e "$BONG_PREVIEW_PID_FILE" ] && [ ! -L "$BONG_PREVIEW_PID_FILE" ]
# review finding：身份不可确认时未记录 server 不得被裸留 25565。PID 记录从未
# 发布，仅断言记录不存在不足以证明进程被回收——旧实现对存活进程拒发信号、
# 进程裸留照样满足这条断言。直接验证端口已释放：进程还占着 25565 即有界回收
# 未生效。
if ! python3 - <<'PY'
import socket
import sys

try:
    socket.create_connection(("127.0.0.1", 25565), timeout=1.0).close()
except OSError:
    sys.exit(0)
print("identity-failure preview left the untracked server holding 25565", file=sys.stderr)
sys.exit(1)
PY
then
  exit 1
fi

cat >"$TMP_ROOT/bin/python3" <<EOF
#!/usr/bin/env bash
set -euo pipefail
# 模式 2 用真 python3 做端口检查与回滚（os.stat 不受 stat shim 影响）。
# FAKE_PIDFD_SIGNAL_FAIL=1 时让 pidfd-signal 返回 2（identity 无法确认），
# 触发「记录发布失败恢复路径」里 stop 未确认、进程仍存活的分支（review
# finding [1]：旧实现这时无限 wait）。
if [ "\${FAKE_PIDFD_SIGNAL_FAIL:-0}" = "1" ] && [[ "\$*" == *"bong-pidfd-signal.py"* ]]; then
  exit 2
fi
if [[ "\$*" == *"bong-listener-owner.py"* ]]; then exit 2; fi
exec "$(command -v python3)" "\$@"
EOF
chmod 700 "$TMP_ROOT/bin/python3"
if run_preview 2 >"$TMP_ROOT/owner.log" 2>&1; then
  echo "listener-owner inspection failure unexpectedly succeeded" >&2
  exit 1
fi
[ ! -e "$BONG_PREVIEW_PID_FILE" ] && [ ! -L "$BONG_PREVIEW_PID_FILE" ]

# review finding：readiness 循环 process_status 返回 2（进程存活但身份不可确认）
# 时，必须走 identity-safe 回滚，不能跳过直接退出把进程+记录都留下。注入方式：
# 记录发布后 stat 恰好失败一次——launch 钉扎与 write_record 都发生在记录创建前
# （不受影响，记录正常发布），readiness 的 process_status 拿不到 identity 返回 2，
# 回滚重查时 stat 已恢复、能确认身份停服并清记录。
rm -f "$TMP_ROOT/stat-post-record-fired"
export FAKE_STAT_FAIL_AFTER_RECORD=1
if run_preview 2 >"$TMP_ROOT/readiness2.log" 2>&1; then
  echo "readiness status-2 preview unexpectedly succeeded" >&2
  exit 1
fi
unset FAKE_STAT_FAIL_AFTER_RECORD
rm -f "$TMP_ROOT/stat-post-record-fired"
[ ! -e "$BONG_PREVIEW_PID_FILE" ] && [ ! -L "$BONG_PREVIEW_PID_FILE" ]
if ! python3 - <<'PY'
import socket
import sys

try:
    socket.create_connection(("127.0.0.1", 25565), timeout=1.0).close()
except OSError:
    sys.exit(0)
print("readiness status-2 preview left the server holding 25565", file=sys.stderr)
sys.exit(1)
PY
then
  exit 1
fi

# review finding：记录发布失败（write_record 的 identity 读取失败）后，恢复路径
# 调 stop 若返回未确认（2）、进程仍存活，旧实现只记日志就无限 wait——命令挂死
# 而不是报启动失败。注入方式：launch 循环的首次身份读取成功（call #1，恒为
# launch 钉扎），write_record 的 identity 读取（call #2）起持续失败 → 记录发布
# 失败；同时 pidfd-signal 返回 2（进程存活，stop 未确认）。修复后走有界回收：
# 直接子进程 → 停进程树 + 释放端口，命令有界返回。
rm -f "$TMP_ROOT/stat-count"
export FAKE_STAT_FAIL_FROM_CALL=2
export FAKE_PIDFD_SIGNAL_FAIL=1
if timeout 20 bash "$REPO_ROOT/scripts/preview/run-server-headless.sh" --timeout 2 \
    >"$TMP_ROOT/pubfail.log" 2>&1; then
  echo "record-publication failure preview unexpectedly succeeded" >&2
  exit 1
else
  rc=$?
fi
unset FAKE_STAT_FAIL_FROM_CALL FAKE_PIDFD_SIGNAL_FAIL
rm -f "$TMP_ROOT/stat-count"
[ "$rc" -ne 124 ] || {
  echo "record-publication failure preview hung (bounded cleanup missed)" >&2
  exit 1
}
[ ! -e "$BONG_PREVIEW_PID_FILE" ] && [ ! -L "$BONG_PREVIEW_PID_FILE" ]
if ! python3 - <<'PY'
import socket
import sys

try:
    socket.create_connection(("127.0.0.1", 25565), timeout=1.0).close()
except OSError:
    sys.exit(0)
print("record-publication failure preview left the server holding 25565", file=sys.stderr)
sys.exit(1)
PY
then
  exit 1
fi

echo "preview lifecycle harness: PASS"
