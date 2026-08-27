#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
TMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/bong-preview-harness.XXXXXX")"
# 在 PATH 被 $TMP_ROOT/bin 遮蔽之前捕获真 python3 绝对路径——shim 的 fallback
# 必须回落到真实解释器，不能再用 command -v 动态解析（review finding：
# PATH 遮蔽后 command -v python3 解析回 shim 自身 → 无限递归 exec 挂死）。
REAL_PYTHON="$(command -v python3)"
cleanup() {
  BONG_PREVIEW_PID_FILE="$TMP_ROOT/runtime/server.pid" \
    bash "$REPO_ROOT/scripts/preview/stop-server-headless.sh" >/dev/null 2>&1 || true
  rm -rf -- "$TMP_ROOT"
}
trap cleanup EXIT

source "$REPO_ROOT/scripts/lib/bong-cargo-target.sh"
export CARGO_TARGET_DIR="$TMP_ROOT/target"
SCOPED_TARGET="$(bong_scoped_cargo_target "$REPO_ROOT/server")"

mkdir -p "$TMP_ROOT/runtime" "$SCOPED_TARGET/release" "$TMP_ROOT/bin" "$TMP_ROOT/fake-src"
chmod 700 "$TMP_ROOT/runtime" "$TMP_ROOT/bin" "$TMP_ROOT/fake-src"
cat >"$TMP_ROOT/fake-src/server.c" <<'EOF'
#define _GNU_SOURCE
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
    /* SO_REUSEPORT：同一 addr:port 允许第二个实例并存 bind，让「省略 pre-launch
       拒绝门禁、二次启动并覆盖记录」的错误实现能真正走到 write_record，否则第二
       个实例因端口被占 bind 失败提前死亡、碰巧不改记录，启动门禁回归测不出来。 */
    setsockopt(fd, SOL_SOCKET, SO_REUSEPORT, &one, sizeof(one));
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
gcc "$TMP_ROOT/fake-src/server.c" -o "$SCOPED_TARGET/release/bong-server"
cat >"$TMP_ROOT/bin/cargo" <<EOF
#!/usr/bin/env bash
set -euo pipefail
# review finding [3]：把 build 调用参数记录到日志，供断言「release/debug profile
# 下 build 命令必须随之变化」——旧 harness 的 cargo stub 无条件 exit 0，跳过
# build / 用错 profile 的实现照常绿。
printf '%s\n' "\$*" >>"$TMP_ROOT/cargo-invocations.log"
# review finding 31436388638 [major]：FAKE_SLOW_BUILD_SECONDS 让 stub 睡 N 秒模拟
# 慢构建——并发 stop 竞速用例需要在「start 持锁构建、记录未发布」的窗口内调 stop。
if [ -n "\${FAKE_SLOW_BUILD_SECONDS:-}" ]; then
  sleep "\$FAKE_SLOW_BUILD_SECONDS"
fi
exit 0
EOF
chmod 700 "$TMP_ROOT/bin/cargo"

export PATH="$TMP_ROOT/bin:$PATH"
export BONG_BUILD_TOKEN_TEST_MODE=1
export BONG_BUILD_TOKEN_DIR="$TMP_ROOT/build-token"
export BONG_PREVIEW_PID_FILE="$TMP_ROOT/runtime/server.pid"
export BONG_SKIP_SKIN_PREFETCH=1

run_preview() {
  local timeout="$1"
  shift
  bash "$REPO_ROOT/scripts/preview/run-server-headless.sh" --timeout "$timeout" "$@"
}

# 0 = 25565 上有 listener；1 = 无。供启动门禁用例断言「拒绝后不得拉新 server」。
listener_on_25565() {
  python3 - <<'PY'
import socket
import sys

try:
    socket.create_connection(("127.0.0.1", 25565), timeout=1.0).close()
except OSError:
    sys.exit(1)
sys.exit(0)
PY
}

# review finding 31447830772 [major]：stop 的「无记录 → 成功 no-op」契约从未被断言
# （cleanup trap 里的无记录 stop 调用以 || true 丢弃失败）——回归成「无记录时返回
# 错误」的实现能通过全部既有用例。先测冷态：从未发布过任何记录时 stop 必须返回 0，
# 且不产生记录。
if ! bash "$REPO_ROOT/scripts/preview/stop-server-headless.sh" \
    >"$TMP_ROOT/stop-cold-norecord.log" 2>&1; then
  echo "stop on a cold no-record state failed (no-op contract broken)" >&2
  exit 1
fi
[ ! -e "$BONG_PREVIEW_PID_FILE" ] && [ ! -L "$BONG_PREVIEW_PID_FILE" ]

: >"$TMP_ROOT/cargo-invocations.log"
export BONG_PREVIEW_LAUNCH_STATE_FILE="$TMP_ROOT/runtime/launch.state"
run_preview 5 >/dev/null
[ -f "$BONG_PREVIEW_PID_FILE" ]
# The wrapper must publish an explicit parent handoff only after the PID
# authority exists.  The outer test-all runner uses this marker when the
# wrapper later exits non-zero during post-publication rollback.
[ -f "$BONG_PREVIEW_LAUNCH_STATE_FILE" ] || {
  echo "successful preview did not publish the authority handoff marker" >&2
  exit 1
}
grep -qx 'state=authority_published' "$BONG_PREVIEW_LAUNCH_STATE_FILE" || {
  echo "authority handoff marker has no published state" >&2
  exit 1
}
# review finding [3]：默认 release profile 的 build 必须是 'build --locked --release'。
# 旧 harness 不观测 build 调用，跳过 build / 用错 profile（如省略 --locked、忽略
# --release）的实现照常绿。
grep -qx 'build --locked --release' "$TMP_ROOT/cargo-invocations.log" || {
  echo "release launch did not run 'build-token.sh cargo build --locked --release'" >&2
  exit 1
}
# review finding [major]：run-server-headless.sh 从未在「任何既有权限记录」存在时被
# 调用（合法运行中 / stale / malformed / mismatched / unconfirmable 五形态的启动门禁
# 都没测过）——省略 bong_server_refuse_existing_preview_record_locked、覆盖既有记录
# 再拉新进程的实现能通过全部用例。此处首启的合法运行中记录还在，直接发起第二次启动：
# 必须拒绝（非零退出）、记录逐字节不变、原 server 进程与 25565 listener 均原样保留。
#
# fake server 开 SO_REUSEPORT：两个实例可同时 bind 25565。省略门禁的错误实现此时能
# 成功二次启动并覆盖记录（而非 bind 失败提前死亡、碰巧没改记录），本组断言必红。
refuse_valid_before="$(cat "$BONG_PREVIEW_PID_FILE")"
refuse_valid_pid="$(sed -n 's/^pid=//p' "$BONG_PREVIEW_PID_FILE")"
if run_preview 1 >"$TMP_ROOT/refuse-valid.log" 2>&1; then
  echo "second launch while a valid running server's record exists unexpectedly succeeded" >&2
  exit 1
fi
[ "$(cat "$BONG_PREVIEW_PID_FILE")" = "$refuse_valid_before" ] || {
  echo "second launch overwrote the running server's authority record" >&2
  exit 1
}
kill -0 "$refuse_valid_pid" 2>/dev/null || {
  echo "second-launch refusal left the running server's process gone" >&2
  exit 1
}
listener_on_25565 || {
  echo "second-launch refusal left the running server's 25565 listener gone" >&2
  exit 1
}
bash "$REPO_ROOT/scripts/preview/stop-server-headless.sh"
[ ! -e "$BONG_PREVIEW_PID_FILE" ] && [ ! -L "$BONG_PREVIEW_PID_FILE" ]
rm -f -- "$BONG_PREVIEW_LAUNCH_STATE_FILE"
unset BONG_PREVIEW_LAUNCH_STATE_FILE
# 同一契约的稳定态：一次成功停服后记录已清，再调 stop 必须仍返回 0（幂等 no-op）——
# 回归成「无记录时返回错误」的实现此断言必红（review finding 31447830772 [major]）。
if ! bash "$REPO_ROOT/scripts/preview/stop-server-headless.sh" \
    >"$TMP_ROOT/stop-stable-norecord.log" 2>&1; then
  echo "stop on the stable no-record state failed (idempotent no-op contract broken)" >&2
  exit 1
fi
[ ! -e "$BONG_PREVIEW_PID_FILE" ] && [ ! -L "$BONG_PREVIEW_PID_FILE" ]

# review finding [3]：--debug 必须改用 debug profile——build 命令不带 --release
# （'build --locked'），且解析/启动的产物必须是 target/debug/bong-server。忽略
# --debug（仍用 release 产物与 release build 命令）的实现在此红。debug fake 同样
# 编译为原生可执行文件，/proc/<pid>/exe 才能匹配上钉扎的 debug artifact。
mkdir -p "$SCOPED_TARGET/debug"
gcc "$TMP_ROOT/fake-src/server.c" -o "$SCOPED_TARGET/debug/bong-server"
: >"$TMP_ROOT/cargo-invocations.log"
run_preview 5 --debug >/dev/null
[ -f "$BONG_PREVIEW_PID_FILE" ]
debug_pid="$(sed -n 's/^pid=//p' "$BONG_PREVIEW_PID_FILE")"
debug_exe="$(readlink -f -- "/proc/$debug_pid/exe" 2>/dev/null || true)"
[ "$debug_exe" = "$SCOPED_TARGET/debug/bong-server" ] || {
  echo "--debug launch did not start the debug artifact (ignored --debug or wrong profile)" >&2
  exit 1
}
grep -qx 'build --locked' "$TMP_ROOT/cargo-invocations.log" || {
  echo "--debug launch did not run 'build-token.sh cargo build --locked'" >&2
  exit 1
}
bash "$REPO_ROOT/scripts/preview/stop-server-headless.sh"
[ ! -e "$BONG_PREVIEW_PID_FILE" ] && [ ! -L "$BONG_PREVIEW_PID_FILE" ]

export FAKE_SERVER_MODE=early
early_start=$SECONDS
if run_preview 2 >"$TMP_ROOT/early.log" 2>&1; then
  echo "early-exit preview unexpectedly succeeded" >&2
  exit 1
fi
early_elapsed=$((SECONDS - early_start))
# review finding 31436388638 [minor]：旧实现的身份钉扎循环对已死进程不 break，把
# 500 次迭代 × sleep 0.01（≈5s）全跑完才报失败。启动即退出的场景（配置失败 / 早期
# 崩溃）应尽早失败，不得按固定迭代预算白耗。窗口上限 3s：修复后首次迭代即检测到
# 进程死亡并 break，耗时 ≪ 1s；错误实现固定 ~5s，两者区分度足够。
[ "$early_elapsed" -lt 3 ] || {
  echo "early-exit failure took ${early_elapsed}s — identity-pinning loop did not break on a dead process" >&2
  exit 1
}
# 失败路径必须是「已提前退出」分支（区别于「身份无法确认」分支）——启动器要明确
# 报告 server 已死的原因，而不是含糊地拒绝钉扎。
grep -q "已提前退出" "$TMP_ROOT/early.log" || {
  echo "early-exit failure did not report the dead-server cause" >&2
  exit 1
}
[ ! -e "$BONG_PREVIEW_PID_FILE" ] && [ ! -L "$BONG_PREVIEW_PID_FILE" ]
unset FAKE_SERVER_MODE

export FAKE_SERVER_MODE=no_listener
timeout_out="$(run_preview 1 2>&1)" && timeout_rc=0 || timeout_rc=$?
if [ "$timeout_rc" -eq 0 ]; then
  echo "timeout preview unexpectedly succeeded" >&2
  exit 1
fi
[ ! -e "$BONG_PREVIEW_PID_FILE" ] && [ ! -L "$BONG_PREVIEW_PID_FILE" ]
# review finding [1]：超时回滚只断言「记录被清」放过了「删记录却把 server 进程裸留
# 成 untracked 孤儿」的错误实现（进程仍占着 25565，只是不再被记录跟踪）。回滚后
# 必须验证被 spawn 的 server 进程已真实终止。PID 从启动器 stdout 的 'PID=$SERVER_PID'
# 行取（write_record/disown 后、readiness 前打印），再核对 /proc/<pid>/exe 不再指向
# fake binary——若 PID 已被复用为无关进程，exe 路径也必然不是 fake binary。
timeout_pid="$(sed -n 's/.*PID=\([0-9][0-9]*\).*/\1/p' <<<"$timeout_out" | head -1)"
[ -n "$timeout_pid" ] || {
  echo "timeout preview did not report the spawned server PID" >&2
  exit 1
}
timeout_exe="$(readlink -f -- "/proc/$timeout_pid/exe" 2>/dev/null || true)"
[ "$timeout_exe" != "$SCOPED_TARGET/release/bong-server" ] || {
  echo "timeout rollback left the untracked server process alive (PID $timeout_pid)" >&2
  exit 1
}
unset FAKE_SERVER_MODE

REAL_STAT="$(command -v stat)"
cat >"$TMP_ROOT/bin/stat" <<EOF
#!/usr/bin/env bash
set -euo pipefail
# 四种注入模式，按测试分场景单独启用：
#   1. FAKE_STAT_IDENTITY_FAILURE=1       —— 对 /proc/<pid>/exe 持续失败
#   2. FAKE_STAT_FAIL_AFTER_RECORD=1      —— 记录发布后失败一次（标志文件保证只一次）
#   3. FAKE_STAT_FAIL_FROM_CALL=<n>       —— 第 n 次起对 /proc/<pid>/exe 持续失败
#   4. FAKE_STAT_SLOW_EXE_SLEEP=<s>       —— 每次 /proc/<pid>/exe 读取先睡 s 秒
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
# 模式 5：记录发布后**持续**失败——readiness 的 process_status 永久返回 2，回滚
# 重查同样拿不到 identity（review finding 31447830772 [major]：单次失败模式 2
# 让回滚重查恢复，覆盖不到「回滚未确认、进程裸留」的路径）。配合
# FAKE_PIDFD_SIGNAL_FAIL=1（pidfd 停服也返回 2），锁定「回滚未确认后必须走有界
# 直接子进程回收」的兜底。
if [ "\${FAKE_STAT_FAIL_AFTER_RECORD_PERSISTENT:-0}" = "1" ] \
    && [ -e "$TMP_ROOT/runtime/server.pid" ]; then
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
# 模式 4：FAKE_STAT_SLOW_EXE_SLEEP=<seconds> —— 每次 /proc/*/exe 的 stat 读取先睡
# N 秒，拉宽「spawn → 记录发布」窗口（身份钉扎与 write_record 都经 stat 读
# executable_identity），让取消保护用例能在记录发布前发 SIGTERM（review finding [2]）。
if [ -n "\${FAKE_STAT_SLOW_EXE_SLEEP:-}" ]; then
  sleep "\$FAKE_STAT_SLOW_EXE_SLEEP"
fi
exec "$REAL_STAT" "\$@"
EOF
chmod 700 "$TMP_ROOT/bin/stat"

# Install the pidfd seam before the process-status=2 regression so that its
# identity-safe rollback is forced into the bounded direct-child fallback. The
# later python3 shim extends this same seam with listener-owner injection.
cat >"$TMP_ROOT/bin/python3" <<EOF
#!/usr/bin/env bash
set -euo pipefail
if [ "\${FAKE_PIDFD_SIGNAL_FAIL:-0}" = "1" ] \
    && [[ "\$*" == *"bong-pidfd-signal.py"* ]]; then
  exit 2
fi
exec "$REAL_PYTHON" "\$@"
EOF
chmod 700 "$TMP_ROOT/bin/python3"

# process inspection is tri-state: kill -0 success plus an unavailable ps
# metadata read means status 2 (the PID is live but uncertain), not absence.
# Keep the seam narrow so listener-owner and process-group probes still use the
# real ps implementation.
REAL_PS="$(command -v ps)"
cat >"$TMP_ROOT/bin/ps" <<EOF
#!/usr/bin/env bash
set -euo pipefail
if [ "\${FAKE_PROCESS_STATUS_UNCERTAIN:-0}" = "1" ] \
    && [ -e "$TMP_ROOT/runtime/server.pid" ] \
    && [[ "\$*" == "-o stat= -p "* ]]; then
  exit 1
fi
exec "$REAL_PS" "\$@"
EOF
chmod 700 "$TMP_ROOT/bin/ps"

# Publication rollback failure must be testable without weakening the
# identity-safe production path: make descendant enumeration fail, while
# leaving pgrep -x (used only by this harness) real for leak assertions.
REAL_PGREP="$(command -v pgrep)"
cat >"$TMP_ROOT/bin/pgrep" <<EOF
#!/usr/bin/env bash
set -euo pipefail
if [ "\${FAKE_PGREP_CHILD_ENUM_FAIL:-0}" = "1" ] && [[ "\$*" == "-P "* ]]; then
  exit 2
fi
exec "$REAL_PGREP" "\$@"
EOF
chmod 700 "$TMP_ROOT/bin/pgrep"

# review finding [1]：process-status=2 must be treated as a live direct child.
# Inject uncertainty only after the authority record is published, so readiness
# reaches the post-publication rollback and its bounded direct-child fallback.
# The old `if ! process_is_running` path treated it as gone and waited forever;
# the regression is bounded and proves both the process and listener are gone.
export FAKE_PROCESS_STATUS_UNCERTAIN=1
export FAKE_PIDFD_SIGNAL_FAIL=1
if timeout 20 bash "$REPO_ROOT/scripts/preview/run-server-headless.sh" --timeout 2 \
    >"$TMP_ROOT/process-status2.log" 2>&1; then
  unset FAKE_PROCESS_STATUS_UNCERTAIN FAKE_PIDFD_SIGNAL_FAIL
  echo "process-status=2 preview unexpectedly succeeded" >&2
  exit 1
else
  rc=$?
fi
unset FAKE_PROCESS_STATUS_UNCERTAIN FAKE_PIDFD_SIGNAL_FAIL
[ "$rc" -ne 124 ] || {
  echo "process-status=2 preview hung instead of bounded cleanup" >&2
  exit 1
}
[ ! -e "$BONG_PREVIEW_PID_FILE" ] && [ ! -L "$BONG_PREVIEW_PID_FILE" ] || {
  echo "process-status=2 preview left an authority record" >&2
  exit 1
}
if listener_on_25565; then
  echo "process-status=2 preview left the direct child holding 25565" >&2
  exit 1
fi

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

# 找所有 exe 指向 release fake binary 的存活进程（取消用例断言泄漏用）。
# 用 pgrep 先缩小候选再逐一 readlink 校验 exe 路径，避免对 /proc 全量 readlink——
# 满载时全量扫描可达数秒，超过 FAKE_STAT_SLOW_EXE_SLEEP 注入的「spawn → 记录发布」
# 窗口，取消用例的轮询会把「记录已发布」误判为窗口错过（实测：load ~9 下 5 连败，
# 改为 pgrep 后窗口内 0.05s 级命中）。语义不变：仍按 exe 绝对路径精确匹配。
fake_server_pids() {
  local pd exe
  for pd in $(pgrep -x bong-server 2>/dev/null || true); do
    exe="$(readlink -f -- "/proc/$pd/exe" 2>/dev/null || true)"
    [ "$exe" = "$SCOPED_TARGET/release/bong-server" ] && printf '%s\n' "$pd"
  done
  return 0  # 无匹配时循环末命令 [ ] 返回非零；set -euo pipefail 下独立赋值
           # `x="$(fake_server_pids | head -1)"` 会继承该非零直接 abort（实测坑）。
}

# review finding [2]：spawn 之后、记录发布之前取消启动器，不得留下 untracked
# server。旧实现无此窗口的清理——server 子 shell trap '' HUP 忽略挂断信号，启动器
# 被杀后它作为孤儿继续持有 25565。注入 stat 延迟（FAKE_STAT_SLOW_EXE_SLEEP）拉宽
# 窗口，后台启动后在记录发布前 SIGTERM 启动器，断言：launcher 退出、fake server
# 进程消失、无记录残留、25565 已释放。
if [ -n "$(fake_server_pids)" ]; then
  echo "precondition failed: fake server already alive before cancellation test" >&2
  exit 1
fi
FAKE_STAT_SLOW_EXE_SLEEP=2 \
  bash "$REPO_ROOT/scripts/preview/run-server-headless.sh" --timeout 30 \
  >"$TMP_ROOT/cancel.log" 2>&1 &
cancel_launch_pid=$!
# 等 fake server 出现（= spawn 完成、进入身份钉扎循环，即记录发布前窗口）。
cancel_server_pid=""
for _ in $(seq 1 200); do
  cancel_server_pid="$(fake_server_pids | head -1)"
  [ -n "$cancel_server_pid" ] && break
  sleep 0.05
done
[ -n "$cancel_server_pid" ] || {
  echo "cancellation test: fake server did not appear before deadline" >&2
  kill "$cancel_launch_pid" 2>/dev/null || true
  exit 1
}
[ ! -e "$BONG_PREVIEW_PID_FILE" ] && [ ! -L "$BONG_PREVIEW_PID_FILE" ] || {
  echo "cancellation test: record was already published before cancellation (window missed)" >&2
  kill "$cancel_launch_pid" 2>/dev/null || true
  exit 1
}
kill -TERM "$cancel_launch_pid" 2>/dev/null || true
# 等启动器退出（trap 回收 server 后 exit 1）。
launcher_gone=0
for _ in $(seq 1 200); do
  if ! kill -0 "$cancel_launch_pid" 2>/dev/null; then
    launcher_gone=1
    break
  fi
  sleep 0.05
done
if [ "$launcher_gone" -eq 0 ]; then
  echo "launcher did not exit after SIGTERM (pre-publication cancellation)" >&2
  kill -KILL "$cancel_launch_pid" 2>/dev/null || true
  exit 1
fi
cancel_remaining="$(fake_server_pids)"
[ -z "$cancel_remaining" ] || {
  echo "pre-publication cancellation left untracked server(s) alive: $cancel_remaining" >&2
  kill -KILL $cancel_remaining 2>/dev/null || true
  exit 1
}
[ ! -e "$BONG_PREVIEW_PID_FILE" ] && [ ! -L "$BONG_PREVIEW_PID_FILE" ] || {
  echo "pre-publication cancellation left an authority record" >&2
  exit 1
}
if listener_on_25565; then
  echo "pre-publication cancellation left an untracked server holding 25565" >&2
  exit 1
fi

# review finding [3]：记录已发布后、disown/trap-clear 完成前收到的 TERM/HUP 必须
# 走 post-publication rollback，而不是沿用「无记录」的 pre-publication 分支。显式
# delay 把窗口扩大到可确定观测的范围；每个信号都断言 launcher、server、记录和
# listener 四路终态，避免只测到 launcher 自己退出。
run_post_publication_signal_case() {
  local signal="$1" launch_pid="" recorded_pid="" launcher_gone=0 remaining

  BONG_PREVIEW_POST_PUBLICATION_DELAY_SECONDS=3 \
    bash "$REPO_ROOT/scripts/preview/run-server-headless.sh" --timeout 30 \
    >"$TMP_ROOT/post-publication-${signal}.log" 2>&1 &
  launch_pid=$!
  for _ in $(seq 1 200); do
    if [ -f "$BONG_PREVIEW_PID_FILE" ]; then
      break
    fi
    sleep 0.05
  done
  [ -f "$BONG_PREVIEW_PID_FILE" ] || {
    echo "post-publication $signal test: authority record was not published" >&2
    kill "$launch_pid" 2>/dev/null || true
    return 1
  }
  recorded_pid="$(sed -n 's/^pid=//p' "$BONG_PREVIEW_PID_FILE")"
  [ -n "$recorded_pid" ] || {
    echo "post-publication $signal test: published record has no PID" >&2
    kill -KILL "$launch_pid" 2>/dev/null || true
    return 1
  }
  kill -"$signal" "$launch_pid" 2>/dev/null || {
    echo "post-publication $signal test: failed to signal launcher" >&2
    kill -KILL "$launch_pid" 2>/dev/null || true
    return 1
  }
  for _ in $(seq 1 200); do
    if ! kill -0 "$launch_pid" 2>/dev/null; then
      launcher_gone=1
      break
    fi
    sleep 0.05
  done
  [ "$launcher_gone" -eq 1 ] || {
    echo "post-publication $signal test: launcher did not exit" >&2
    kill -KILL "$launch_pid" 2>/dev/null || true
    return 1
  }
  wait "$launch_pid" 2>/dev/null || true
  [ ! -e "$BONG_PREVIEW_PID_FILE" ] && [ ! -L "$BONG_PREVIEW_PID_FILE" ] || {
    echo "post-publication $signal test: authority record survived rollback" >&2
    return 1
  }
  remaining="$(fake_server_pids)"
  [ -z "$remaining" ] || {
    echo "post-publication $signal test: server survived rollback: $remaining" >&2
    kill -KILL $remaining 2>/dev/null || true
    return 1
  }
  if listener_on_25565; then
    echo "post-publication $signal test: listener survived rollback" >&2
    return 1
  fi
}
run_post_publication_signal_case TERM
run_post_publication_signal_case HUP

cat >"$TMP_ROOT/bin/python3" <<EOF
#!/usr/bin/env bash
set -euo pipefail
# 两条故障注入都按 flag 门控、只在其专属用例里启用（review finding 31447830772
# [major]：listener-owner 分支无条件 exit 2 曾泄漏到后续的并发 stop 竞速用例——start
# 的 readiness 端口检查命中注入自毁回滚，把「stop 假成功」的回归证据擦掉）。
# FAKE_PIDFD_SIGNAL_FAIL=1 时让 pidfd-signal 返回 2（identity 无法确认），触发
# 「记录发布失败恢复路径」里 stop 未确认、进程仍存活的分支（review finding [1]：
# 旧实现这时无限 wait）。
if [ "\${FAKE_PIDFD_SIGNAL_FAIL:-0}" = "1" ] && [[ "\$*" == *"bong-pidfd-signal.py"* ]]; then
  exit 2
fi
if [ "\${FAKE_LISTENER_OWNER_FAIL:-0}" = "1" ] && [[ "\$*" == *"bong-listener-owner.py"* ]]; then
  exit 2
fi
exec "$REAL_PYTHON" "\$@"
EOF
chmod 700 "$TMP_ROOT/bin/python3"
export FAKE_LISTENER_OWNER_FAIL=1
if run_preview 2 >"$TMP_ROOT/owner.log" 2>&1; then
  unset FAKE_LISTENER_OWNER_FAIL
  echo "listener-owner inspection failure unexpectedly succeeded" >&2
  exit 1
fi
unset FAKE_LISTENER_OWNER_FAIL
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

# review finding 31447830772 [major]：记录发布后 identity **持续**不可确认（非单次）
# 时，readiness 的 process_status 永久返回 2；回滚重查同样拿不到 identity，pidfd
# 停服也返回 2（FAKE_PIDFD_SIGNAL_FAIL=1）——旧实现把回滚失败 || true 吞掉直接
# 退出，被 disown 的 server 无限持有 25565，后续启动被保留的记录拒绝。修复后走
# 有界直接子进程回收：停进程树 + 释放端口 + 按相同身份清记录，命令有界非零返回。
rm -f "$TMP_ROOT/stat-post-record-fired"
export FAKE_STAT_FAIL_AFTER_RECORD_PERSISTENT=1
export FAKE_PIDFD_SIGNAL_FAIL=1
if timeout 20 bash "$REPO_ROOT/scripts/preview/run-server-headless.sh" --timeout 2 \
    >"$TMP_ROOT/status2persist.log" 2>&1; then
  echo "persistent post-publication identity-failure preview unexpectedly succeeded" >&2
  exit 1
else
  rc=$?
fi
unset FAKE_STAT_FAIL_AFTER_RECORD_PERSISTENT FAKE_PIDFD_SIGNAL_FAIL
[ "$rc" -ne 124 ] || {
  echo "persistent status-2 preview hung (bounded cleanup missed)" >&2
  exit 1
}
[ ! -e "$BONG_PREVIEW_PID_FILE" ] && [ ! -L "$BONG_PREVIEW_PID_FILE" ] || {
  echo "persistent status-2 preview left an authority record (server leak)" >&2
  exit 1
}
status2_persist_pids="$(fake_server_pids)"
[ -z "$status2_persist_pids" ] || {
  echo "persistent status-2 preview left server(s) alive: $status2_persist_pids" >&2
  kill -KILL $status2_persist_pids 2>/dev/null || true
  exit 1
}
if listener_on_25565; then
  echo "persistent status-2 preview left a server holding 25565" >&2
  exit 1
fi

# 同一兜底在 listener-owner 不可确认路径上的覆盖：readiness 的端口 owner 检查持续
# 返回 2（FAKE_LISTENER_OWNER_FAIL=1），回滚的 pidfd 停服也返回 2——同样必须走
# 有界直接子进程回收，不能带着存活进程直接退出。
export FAKE_LISTENER_OWNER_FAIL=1
export FAKE_PIDFD_SIGNAL_FAIL=1
if timeout 20 bash "$REPO_ROOT/scripts/preview/run-server-headless.sh" --timeout 2 \
    >"$TMP_ROOT/owner2persist.log" 2>&1; then
  echo "persistent listener-owner failure preview unexpectedly succeeded" >&2
  exit 1
else
  rc=$?
fi
unset FAKE_LISTENER_OWNER_FAIL FAKE_PIDFD_SIGNAL_FAIL
[ "$rc" -ne 124 ] || {
  echo "persistent listener-owner failure preview hung (bounded cleanup missed)" >&2
  exit 1
}
[ ! -e "$BONG_PREVIEW_PID_FILE" ] && [ ! -L "$BONG_PREVIEW_PID_FILE" ] || {
  echo "persistent listener-owner failure preview left an authority record (server leak)" >&2
  exit 1
}
owner2_persist_pids="$(fake_server_pids)"
[ -z "$owner2_persist_pids" ] || {
  echo "persistent listener-owner failure preview left server(s) alive: $owner2_persist_pids" >&2
  kill -KILL $owner2_persist_pids 2>/dev/null || true
  exit 1
}
if listener_on_25565; then
  echo "persistent listener-owner failure preview left a server holding 25565" >&2
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

# Handoff marker publication failure + identity-safe rollback failure must
# preserve the matching PID authority for the caller's stop retry. A preexisting
# marker is the wrapper's fail-closed publication failure; pidfd and descendant
# enumeration seams make both rollback paths genuinely unconfirmed. This runs
# the real wrapper and the real stop command, unlike the outer-runner fixture.
printf 'state=foreign\n' > "$TMP_ROOT/runtime/preexisting-launch.state"
chmod 600 "$TMP_ROOT/runtime/preexisting-launch.state"
export BONG_PREVIEW_LAUNCH_STATE_FILE="$TMP_ROOT/runtime/preexisting-launch.state"
export FAKE_PIDFD_SIGNAL_FAIL=1
export FAKE_PGREP_CHILD_ENUM_FAIL=1
if timeout 20 bash "$REPO_ROOT/scripts/preview/run-server-headless.sh" --timeout 2 \
    >"$TMP_ROOT/handoff-publish-failure.log" 2>&1; then
  unset BONG_PREVIEW_LAUNCH_STATE_FILE FAKE_PIDFD_SIGNAL_FAIL FAKE_PGREP_CHILD_ENUM_FAIL
  echo "handoff publication failure unexpectedly succeeded" >&2
  exit 1
else
  rc=$?
fi
unset FAKE_PIDFD_SIGNAL_FAIL FAKE_PGREP_CHILD_ENUM_FAIL
[ "$rc" -ne 124 ] || {
  echo "handoff publication failure rollback hung" >&2
  exit 1
}
[ "$rc" -eq 75 ] || {
  echo "handoff publication failure returned $rc instead of dedicated exit 75" >&2
  exit 1
}
grep -q "launch-state publication rollback 未确认" "$TMP_ROOT/handoff-publish-failure.log" || {
  echo "handoff publication failure did not report rollback uncertainty" >&2
  exit 1
}
grep -q "保留 authority" "$TMP_ROOT/handoff-publish-failure.log" || {
  echo "handoff publication failure did not hand off retained authority" >&2
  exit 1
}
[ -f "$BONG_PREVIEW_PID_FILE" ] || {
  echo "handoff publication failure lost the matching PID authority" >&2
  exit 1
}
[ "$(cat "$TMP_ROOT/runtime/preexisting-launch.state")" = "state=foreign" ] || {
  echo "handoff publication failure overwrote the preexisting launch marker" >&2
  exit 1
}
handoff_pid="$(sed -n 's/^pid=//p' "$BONG_PREVIEW_PID_FILE" 2>/dev/null || true)"
[ -n "$handoff_pid" ] || {
  echo "handoff publication failure lost the retained server PID" >&2
  exit 1
}
kill -0 "$handoff_pid" 2>/dev/null || {
  echo "handoff publication failure did not preserve the live server for stop retry (pid=$handoff_pid)" >&2
  exit 1
}
# Restore normal lifecycle inspection and exercise the real identity-safe stop
# against the retained record; no PID-only deletion is permitted here.
unset BONG_PREVIEW_LAUNCH_STATE_FILE
if ! bash "$REPO_ROOT/scripts/preview/stop-server-headless.sh" \
    >"$TMP_ROOT/handoff-publish-stop.log" 2>&1; then
  echo "identity-safe stop could not clean retained authority" >&2
  cat "$TMP_ROOT/handoff-publish-stop.log" >&2
  exit 1
fi
[ ! -e "$BONG_PREVIEW_PID_FILE" ] && [ ! -L "$BONG_PREVIEW_PID_FILE" ] || {
  echo "identity-safe stop left the retained authority record" >&2
  exit 1
}
rm -f "$TMP_ROOT/runtime/preexisting-launch.state"
if listener_on_25565; then
  echo "identity-safe stop left the retained server listener" >&2
  exit 1
fi

# review finding [major]：identity-safe 停服的 security-critical 分支从未被测试——
# stale / mismatched 记录。旧实现若只按记录 PID 发信号、不校验钉扎的
# starttime/executable_identity，会把无关进程杀掉而不红。此处直接手写记录（格式同
# bong_server_write_record：pid/starttime/executable/executable_identity 四行，mode 600）：
#
# (a) mismatched：记录指向**存活但身份不匹配**的无关进程（同 pid、错 starttime/
#     identity）→ stop 必须拒绝（refuse），进程与记录都保留，不得按 PID 直接杀。
sleep 5 &
unrelated_pid=$!
cat >"$BONG_PREVIEW_PID_FILE" <<EOF
pid=$unrelated_pid
starttime=1
executable=$SCOPED_TARGET/release/bong-server
executable_identity=0:1
EOF
chmod 600 "$BONG_PREVIEW_PID_FILE"
if bash "$REPO_ROOT/scripts/preview/stop-server-headless.sh" >"$TMP_ROOT/mismatch.log" 2>&1; then
  echo "stop on mismatched-record unexpectedly succeeded" >&2
  kill "$unrelated_pid" 2>/dev/null || true
  exit 1
fi
kill -0 "$unrelated_pid" 2>/dev/null || {
  echo "stop on mismatched-record killed the unrelated process (identity-safe refusal missed)" >&2
  kill "$unrelated_pid" 2>/dev/null || true
  exit 1
}
kill "$unrelated_pid" 2>/dev/null || true
[ -f "$BONG_PREVIEW_PID_FILE" ] || {
  echo "stop on mismatched-record cleared the record despite refusing" >&2
  exit 1
}
rm -f "$BONG_PREVIEW_PID_FILE"

# (b) stale：记录指向已死 pid → stop 走清理路径，清记录并成功，不 panic、不误杀。
cat >"$BONG_PREVIEW_PID_FILE" <<EOF
pid=99999999
starttime=1
executable=$SCOPED_TARGET/release/bong-server
executable_identity=0:1
EOF
chmod 600 "$BONG_PREVIEW_PID_FILE"
if ! bash "$REPO_ROOT/scripts/preview/stop-server-headless.sh" >"$TMP_ROOT/stale.log" 2>&1; then
  echo "stop on stale-record failed" >&2
  exit 1
fi
[ ! -e "$BONG_PREVIEW_PID_FILE" ] && [ ! -L "$BONG_PREVIEW_PID_FILE" ] || {
  echo "stale-record stop did not clear the dead record" >&2
  exit 1
}

# review finding [major]（launch-time 门禁的其余记录形态）：stale / malformed /
# mismatched / unconfirmable 记录存在时，run-server-headless.sh 同样必须拒绝——不覆盖、
# 不清除、不重拉。stop-time 的 mismatched/stale 用例只测 stop，启动门禁从未被调用过。
# 各段手写记录（格式同 bong_server_write_record：四行、mode 600），断言「非零退出 +
# 记录逐字节不变 + 25565 无新监听」。pid-only / 无门禁实现必红。

# (a) stale：记录指向已死 pid → 启动必须拒绝（stale 记录在 READY/覆盖前就要挡下）。
cat >"$BONG_PREVIEW_PID_FILE" <<EOF
pid=99999999
starttime=1
executable=$SCOPED_TARGET/release/bong-server
executable_identity=0:1
EOF
chmod 600 "$BONG_PREVIEW_PID_FILE"
launch_stale_before="$(cat "$BONG_PREVIEW_PID_FILE")"
if run_preview 1 >"$TMP_ROOT/refuse-stale.log" 2>&1; then
  echo "launch while a stale authority record exists unexpectedly succeeded" >&2
  exit 1
fi
[ "$(cat "$BONG_PREVIEW_PID_FILE")" = "$launch_stale_before" ] || {
  echo "launch while a stale authority record exists overwrote or cleared it" >&2
  exit 1
}
if listener_on_25565; then
  echo "refused launch while a stale record exists left a new server on 25565" >&2
  exit 1
fi
rm -f "$BONG_PREVIEW_PID_FILE"

# (b) malformed：记录无法解析 → 启动必须拒绝（read_record 失败即拒，绝不覆盖坏记录）。
printf 'pid=1\nnot-a-valid-record\n' >"$BONG_PREVIEW_PID_FILE"
chmod 600 "$BONG_PREVIEW_PID_FILE"
launch_malformed_before="$(cat "$BONG_PREVIEW_PID_FILE")"
if run_preview 1 >"$TMP_ROOT/refuse-malformed.log" 2>&1; then
  echo "launch while a malformed authority record exists unexpectedly succeeded" >&2
  exit 1
fi
[ "$(cat "$BONG_PREVIEW_PID_FILE")" = "$launch_malformed_before" ] || {
  echo "launch while a malformed authority record exists overwrote or cleared it" >&2
  exit 1
}
if listener_on_25565; then
  echo "refused launch while a malformed record exists left a new server on 25565" >&2
  exit 1
fi
rm -f "$BONG_PREVIEW_PID_FILE"

# (c) mismatched：记录指向**存活但身份不匹配**的无关进程 → 启动必须拒绝，无关进程保留
#     （不得按记录 PID 清理或信号化）。
sleep 5 &
launch_unrelated_pid=$!
cat >"$BONG_PREVIEW_PID_FILE" <<EOF
pid=$launch_unrelated_pid
starttime=1
executable=$SCOPED_TARGET/release/bong-server
executable_identity=0:1
EOF
chmod 600 "$BONG_PREVIEW_PID_FILE"
launch_mismatch_before="$(cat "$BONG_PREVIEW_PID_FILE")"
if run_preview 1 >"$TMP_ROOT/refuse-mismatch.log" 2>&1; then
  echo "launch while a mismatched authority record exists unexpectedly succeeded" >&2
  kill "$launch_unrelated_pid" 2>/dev/null || true
  exit 1
fi
[ "$(cat "$BONG_PREVIEW_PID_FILE")" = "$launch_mismatch_before" ] || {
  echo "launch while a mismatched authority record exists overwrote or cleared it" >&2
  kill "$launch_unrelated_pid" 2>/dev/null || true
  exit 1
}
kill -0 "$launch_unrelated_pid" 2>/dev/null || {
  echo "launch-refusal on a mismatched record killed the unrelated process" >&2
  exit 1
}
kill "$launch_unrelated_pid" 2>/dev/null || true
if listener_on_25565; then
  echo "refused launch while a mismatched record exists left a new server on 25565" >&2
  exit 1
fi
rm -f "$BONG_PREVIEW_PID_FILE"

# (d) unconfirmable：记录格式合法、指向存活进程，但身份检查无法确认 → 启动必须拒绝
#     （status 2 → 「记录无法确认」，拒绝覆盖，绝不把该进程按 PID 信号化）。
#     注入：先用真 stat 取存活进程的真实 starttime/executable_identity 写记录（记录
#     看起来完全合法），再开 FAKE_STAT_IDENTITY_FAILURE 使 record_matches_process 的
#     executable_identity 读取（stat /proc/*/exe）持续失败 → status 2。
sleep 5 &
launch_unconfirmable_pid=$!
launch_unconfirmable_starttime="$(awk '{print $22}' "/proc/$launch_unconfirmable_pid/stat")"
launch_unconfirmable_identity="$("$REAL_STAT" -Lc '%d:%i' -- "/proc/$launch_unconfirmable_pid/exe")"
cat >"$BONG_PREVIEW_PID_FILE" <<EOF
pid=$launch_unconfirmable_pid
starttime=$launch_unconfirmable_starttime
executable=$SCOPED_TARGET/release/bong-server
executable_identity=$launch_unconfirmable_identity
EOF
chmod 600 "$BONG_PREVIEW_PID_FILE"
launch_unconfirmable_before="$(cat "$BONG_PREVIEW_PID_FILE")"
export FAKE_STAT_IDENTITY_FAILURE=1
if run_preview 1 >"$TMP_ROOT/refuse-unconfirmable.log" 2>&1; then
  unset FAKE_STAT_IDENTITY_FAILURE
  echo "launch while an unconfirmable authority record exists unexpectedly succeeded" >&2
  kill "$launch_unconfirmable_pid" 2>/dev/null || true
  exit 1
fi
unset FAKE_STAT_IDENTITY_FAILURE
[ "$(cat "$BONG_PREVIEW_PID_FILE")" = "$launch_unconfirmable_before" ] || {
  echo "launch while an unconfirmable authority record exists overwrote or cleared it" >&2
  kill "$launch_unconfirmable_pid" 2>/dev/null || true
  exit 1
}
kill -0 "$launch_unconfirmable_pid" 2>/dev/null || {
  echo "launch-refusal on an unconfirmable record killed the live process" >&2
  exit 1
}
kill "$launch_unconfirmable_pid" 2>/dev/null || true
if listener_on_25565; then
  echo "refused launch while an unconfirmable record exists left a new server on 25565" >&2
  exit 1
fi
rm -f "$BONG_PREVIEW_PID_FILE"

# review finding 31436388638 [major]：start 与 stop / start 的整个构建窗口必须由同一
# lifecycle lock 串行化。两个竞速 oracle 都必须严格区分「预期 lock timeout」与任何
# 其它失败：非零但没有 timeout 证据不是 PASS。
#
# 第一组：两个 start 同时进入构建窗口。第二个 start 必须在显式 1s lock timeout 后
# 失败，且不得执行第二次 build、覆盖记录或占用 listener。
: >"$TMP_ROOT/cargo-invocations.log"
FAKE_SLOW_BUILD_SECONDS=5 \
  bash "$REPO_ROOT/scripts/preview/run-server-headless.sh" --timeout 30 \
  >"$TMP_ROOT/race-start.log" 2>&1 &
race_start_pid=$!
race_in_build=0
for _ in $(seq 1 200); do
  if [ -s "$TMP_ROOT/cargo-invocations.log" ]; then
    race_in_build=1
    break
  fi
  sleep 0.05
done
[ "$race_in_build" -eq 1 ] || {
  echo "concurrent-start race: first start did not enter the build window" >&2
  kill "$race_start_pid" 2>/dev/null || true
  exit 1
}
[ ! -e "$BONG_PREVIEW_PID_FILE" ] && [ ! -L "$BONG_PREVIEW_PID_FILE" ] || {
  echo "concurrent-start race: first start published before the build-window oracle" >&2
  kill "$race_start_pid" 2>/dev/null || true
  exit 1
}
first_build_count="$(wc -l <"$TMP_ROOT/cargo-invocations.log")"
race_second_rc=0
if BONG_SERVER_LIFECYCLE_LOCK_TIMEOUT_SECONDS=1 \
    bash "$REPO_ROOT/scripts/preview/run-server-headless.sh" --timeout 30 \
    >"$TMP_ROOT/race-second-start.log" 2>&1; then
  race_second_rc=0
else
  race_second_rc=$?
fi
[ "$race_second_rc" -ne 0 ] || {
  echo "concurrent-start race: second start unexpectedly succeeded" >&2
  kill "$race_start_pid" 2>/dev/null || true
  exit 1
}
grep -q "timed out after 1s waiting for lifecycle lock" "$TMP_ROOT/race-second-start.log" || {
  echo "concurrent-start race: second start failed without the expected lock-timeout oracle" >&2
  kill "$race_start_pid" 2>/dev/null || true
  exit 1
}
[ "$(wc -l <"$TMP_ROOT/cargo-invocations.log")" -eq "$first_build_count" ] || {
  echo "concurrent-start race: timed-out second start still ran a build" >&2
  kill "$race_start_pid" 2>/dev/null || true
  exit 1
}

# 第二组：stop 在同一构建窗口内必须 also fail honestly when its explicit lock
# timeout is shorter than the build. A bare nonzero or a swallowed no-op is not an
# acceptable substitute for the timeout evidence.
race_stop_rc=0
if BONG_SERVER_LIFECYCLE_LOCK_TIMEOUT_SECONDS=1 \
    bash "$REPO_ROOT/scripts/preview/stop-server-headless.sh" \
    >"$TMP_ROOT/race-stop-timeout.log" 2>&1; then
  race_stop_rc=0
else
  race_stop_rc=$?
fi
[ "$race_stop_rc" -ne 0 ] || {
  echo "concurrent-stop race: stop unexpectedly reported success during a locked build" >&2
  kill "$race_start_pid" 2>/dev/null || true
  exit 1
}
grep -q "timed out after 1s waiting for lifecycle lock" "$TMP_ROOT/race-stop-timeout.log" || {
  echo "concurrent-stop race: stop failed without the expected lock-timeout oracle" >&2
  kill "$race_start_pid" 2>/dev/null || true
  exit 1
}

# Once the first start releases the lock, ordinary stop must still clean the
# server it actually started. This also proves the timeout probes did not leave
# a hidden second authority or listener.
wait "$race_start_pid" 2>/dev/null || true
[ -f "$BONG_PREVIEW_PID_FILE" ] || {
  echo "concurrent-start race: first start did not publish its authority record" >&2
  exit 1
}
if ! bash "$REPO_ROOT/scripts/preview/stop-server-headless.sh" \
    >"$TMP_ROOT/race-stop-final.log" 2>&1; then
  echo "concurrent-stop race: final stop failed after the first start completed" >&2
  cat "$TMP_ROOT/race-stop-final.log" >&2
  exit 1
fi
[ ! -e "$BONG_PREVIEW_PID_FILE" ] && [ ! -L "$BONG_PREVIEW_PID_FILE" ] || {
  echo "concurrent-stop race: final stop left an authority record" >&2
  exit 1
}
race_remaining="$(fake_server_pids)"
[ -z "$race_remaining" ] || {
  echo "concurrent-stop race: final stop left server(s) alive: $race_remaining" >&2
  kill -KILL $race_remaining 2>/dev/null || true
  exit 1
}
if listener_on_25565; then
  echo "concurrent-stop race: final stop left 25565 occupied" >&2
  exit 1
fi

echo "preview lifecycle harness: PASS"
