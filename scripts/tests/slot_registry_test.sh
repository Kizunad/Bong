#!/usr/bin/env bash
# slot_registry.sh 饱和契约测试：原子容量、固定池边界、显式状态机、锁恢复与 JSON。
set -euo pipefail

REG=$(realpath "$(dirname "$0")/../slot_registry.sh")
SANDBOX=$(mktemp -d /tmp/slot-registry-test.XXXXXX)
TEST_PIDS=()
FIFO_FDS=()
RELEASE_FDS=()
PROTECTED_PID=2399867
register_test_pid() {
  local pid="$1"
  [[ "$pid" =~ ^[0-9]+$ ]] || return 1
  [[ "$pid" != "$PROTECTED_PID" ]] || { printf 'slot_registry_test: refuse to register protected PID %s\n' "$pid" >&2; return 1; }
  TEST_PIDS+=("$pid")
}
register_fifo_fd() {
  local fifo="$1" result_var="${2:-}"
  [[ -p "$fifo" ]] || return 1
  local fifo_fd
  exec {fifo_fd}<>"$fifo"
  FIFO_FDS+=("$fifo_fd")
  if [[ -n "$result_var" ]]; then
    printf -v "$result_var" '%s' "$fifo_fd"
  fi
}
pid_belongs_to_this_test() {
  local pid="$1"
  python3 - "$pid" "$SANDBOX" <<'PYOWN'
from pathlib import Path
import os, sys
pid, sandbox = sys.argv[1:]
try:
    env = Path(f"/proc/{pid}/environ").read_bytes()
except OSError:
    raise SystemExit(1)
raise SystemExit(0 if os.fsencode(sandbox) in env else 1)
PYOWN
}
cleanup() {
  trap - EXIT
  local pid fd
  for fd in "${RELEASE_FDS[@]}"; do
    printf 'release\n' >&"$fd" 2>/dev/null || true
  done
  for pid in "${TEST_PIDS[@]}"; do
    [[ "$pid" =~ ^[0-9]+$ ]] && wait "$pid" 2>/dev/null || true
  done
  for fd in "${FIFO_FDS[@]}"; do
    eval "exec ${fd}>&-" || true
  done
  rm -rf "$SANDBOX"
}
trap cleanup EXIT

PASS=0
FAIL=0
pass() { printf '  PASS: %s\n' "$1"; PASS=$((PASS + 1)); }
fail() { printf '  FAIL: %s\n' "$1"; FAIL=$((FAIL + 1)); }
fatal() { printf '  FAIL: %s\n' "$1" >&2; exit 1; }
check() {
  local desc="$1"; shift
  if "$@" >/dev/null 2>&1; then pass "$desc"; else fail "$desc"; fi
}
check_not() {
  local desc="$1"; shift
  if "$@" >/dev/null 2>&1; then fail "$desc"; else pass "$desc"; fi
}
expect_fail() {
  local desc="$1" pattern="$2" out="$3" err="$4"; shift 4
  if "$@" >"$out" 2>"$err"; then
    fail "$desc（意外成功）"
  elif grep -q "$pattern" "$err"; then
    pass "$desc"
  else
    fail "$desc（stderr 未含 $pattern）"
  fi
}
read_gate_ready() {
  local fifo="$1" expected_instance="$2"
  IFS=' ' read -r GATE_WORD GATE_HOLDER_PID GATE_HOLDER_FD GATE_HOLDER_START GATE_INSTANCE < "$fifo"
  if [[ "$GATE_WORD" == ready && "$GATE_HOLDER_PID" =~ ^[0-9]+$ && "$GATE_HOLDER_FD" =~ ^[0-9]+$ &&
        "$GATE_HOLDER_START" =~ ^[0-9]+$ && "$GATE_INSTANCE" == "$expected_instance" ]]; then
    return 0
  fi
  printf 'slot_registry_test: gate ready handshake mismatch\n' >&2
  return 1
}
read_waiter_ready() {
  local fifo="$1" word pid fd
  IFS=' ' read -r word pid fd < "$fifo"
  [[ "$word" == ready && "$pid" =~ ^[0-9]+$ && "$fd" =~ ^[0-9]+$ ]]
}
release_waiter() {
  local fd="$1"
  printf 'release\n' >&"$fd"
}
read_waiter_ack() {
  local fifo="$1" word pid fd
  IFS=' ' read -r word pid fd < "$fifo"
  [[ "$word" == released && "$pid" =~ ^[0-9]+$ && "$fd" =~ ^[0-9]+$ ]]
}
proc_start_ticks() {
  python3 - "$1" <<'PYSTART'
from pathlib import Path
import sys
raw = Path(f"/proc/{sys.argv[1]}/stat").read_text()
right = raw.rfind(")")
print(raw[right + 2:].split()[19])
PYSTART
}
kill_verified_gate_holder() {
  local pid="$1" fd="$2" start_ticks="$3" lock_path="$4" token="$5" ready_fifo="$6" release_fifo="$7"
  python3 - "$pid" "$fd" "$start_ticks" "$lock_path" "$token" "$ready_fifo" "$release_fifo" "$SANDBOX" <<'PYKILL'
from pathlib import Path
import fcntl
import os
import signal
import sys

pid_s, fd_s, expected_start, lock_path, token, ready_fifo, release_fifo, sandbox = sys.argv[1:]

def abort(reason: str) -> None:
    print(reason, file=sys.stderr)
    raise SystemExit(2)

if not pid_s.isdigit() or not fd_s.isdigit() or not expected_start.isdigit():
    abort("invalid holder PID/FD/start ticks")
pid = int(pid_s)
fd_number = int(fd_s)
if pid == 2399867:
    abort("refuse protected historical PID")
if pid <= 1:
    abort("unsafe holder PID")

try:
    pidfd = os.pidfd_open(pid, 0)
except OSError as exc:
    abort(f"holder PID is not alive: {exc}")

probe_fd = None
try:
    proc = Path(f"/proc/{pid}")
    stat_raw = (proc / "stat").read_text()
    right = stat_raw.rfind(")")
    actual_start = stat_raw[right + 2:].split()[19]
    if actual_start != expected_start:
        abort("holder PID start ticks do not match this handshake")

    env = (proc / "environ").read_bytes().split(b"\0")
    expected = {
        b"SLOT_REGISTRY_TEST_INSTANCE=" + os.fsencode(token),
        b"SLOT_REGISTRY_TEST_HOLD_GATE_READY=" + os.fsencode(ready_fifo),
        b"SLOT_REGISTRY_TEST_HOLD_GATE_RELEASE=" + os.fsencode(release_fifo),
        b"SLOT_REGISTRY_ROOT_OVERRIDE=" + os.fsencode(str(Path(sandbox) / "repo/.agent-worktrees/test-registry")),
        b"SLOT_REGISTRY_LOCK_ROOT_OVERRIDE=" + os.fsencode(str(Path(sandbox) / "repo/.agent-worktrees/test-locks")),
    }
    if not expected.issubset(set(env)):
        abort("holder identity does not belong to this handshake")

    cmdline = (proc / "cmdline").read_bytes().split(b"\0")
    script = (Path(sandbox) / "repo/scripts/slot_registry.sh").resolve()
    cwd = Path(os.readlink(proc / "cwd"))
    script_args = []
    for raw_arg in cmdline:
        if not raw_arg:
            continue
        arg = Path(os.fsdecode(raw_arg))
        candidate = arg if arg.is_absolute() else cwd / arg
        try:
            if candidate.resolve() == script:
                script_args.append(raw_arg)
        except OSError:
            continue
    if not script_args or b"acquire" not in cmdline:
        abort("holder command is not this test acquire")

    lock_stat = os.stat(lock_path)
    fd_path = proc / "fd" / fd_s
    fd_stat = os.stat(fd_path)
    lock_identity = (lock_stat.st_dev, lock_stat.st_ino)
    if (fd_stat.st_dev, fd_stat.st_ino) != lock_identity:
        abort("holder FD target inode does not match current acquire gate")

    fdinfo = (proc / "fdinfo" / fd_s).read_text().splitlines()
    expected_lock_id = f"{os.major(lock_stat.st_dev):02x}:{os.minor(lock_stat.st_dev):02x}:{lock_stat.st_ino}".lower()
    owns_reported_fd_lock = False
    for line in fdinfo:
        if not line.startswith("lock:\t"):
            continue
        parts = line.split()
        if len(parts) >= 7 and parts[2:5] == ["FLOCK", "ADVISORY", "WRITE"]:
            if parts[6].lower() == expected_lock_id:
                owns_reported_fd_lock = True
                break
    if not owns_reported_fd_lock:
        abort("reported holder FD does not carry the current acquire gate flock")

    probe_fd = os.open(lock_path, os.O_RDWR)
    try:
        fcntl.flock(probe_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        pass
    else:
        fcntl.flock(probe_fd, fcntl.LOCK_UN)
        abort("current acquire gate is not locked by the reported holder handshake")

    signal.pidfd_send_signal(pidfd, signal.SIGKILL)
finally:
    if probe_fd is not None:
        os.close(probe_fd)
    os.close(pidfd)
PYKILL
}
assert_no_hook_holder() {
  local ready_fifo="$1"
  python3 - "$ready_fifo" <<'PYPROC'
from pathlib import Path
import os, sys
needle = os.fsencode("SLOT_REGISTRY_TEST_HOLD_GATE_READY=" + sys.argv[1]) + b"\0"
found = []
for proc in Path("/proc").iterdir():
    if not proc.name.isdigit():
        continue
    try:
        env = (proc / "environ").read_bytes()
    except OSError:
        continue
    if needle in env:
        found.append(int(proc.name))
if found:
    print("hook holder processes remain:", found, file=sys.stderr)
    raise SystemExit(1)
PYPROC
}
assert_no_temp_reservations() {
  local found
  found=$(find "$REGROOT" -mindepth 1 -maxdepth 1 -type d -name '.slot-*.reservation.*' -print -quit)
  [[ -z "$found" ]]
}
assert_acquire_failure_clean() {
  local step="$1" slot="$2" task="$3" permanent_lock="$4"
  local before after gate_before gate_after out="$SANDBOX/inject-$step.out" err="$SANDBOX/inject-$step.err" rc=0
  before=$(snapshot "$REGROOT")
  gate_before=$(stat -Lc '%d:%i' "$LOCKROOT/acquire.lock")
  env SLOT_REGISTRY_TEST_FAIL_ACQUIRE_STEP="$step" \
    bash scripts/slot_registry.sh acquire --slot "$slot" --task "$task" \
      --branch "bugfix/$task" --claim-sha "$SHA" --agent "agent-$task" >"$out" 2>"$err" || rc=$?
  after=$(snapshot "$REGROOT")
  gate_after=$(stat -Lc '%d:%i' "$LOCKROOT/acquire.lock")
  [[ $rc -ne 0 ]] || return 1
  grep -q "injected acquire $step failure" "$err" || return 1
  [[ "$before" == "$after" ]] || return 1
  [[ "$gate_before" == "$gate_after" ]] || return 1
  assert_no_temp_reservations || return 1
  [[ ! -e "$REGROOT/$slot.lock" ]] || return 1
  [[ -d "$permanent_lock" ]] || return 1
}
field() {
  python3 - "$1" <<'PYF'
from pathlib import Path
import sys
raw = Path(sys.argv[1]).read_bytes()
assert raw.endswith(b"\0"), raw
sys.stdout.write(raw[:-1].decode())
PYF
}
snapshot() {
  python3 - "$1" <<'PYS'
from pathlib import Path
import hashlib, sys
root=Path(sys.argv[1])
h=hashlib.sha256()
for p in sorted(root.rglob('*')):
    h.update(str(p.relative_to(root)).encode())
    h.update(b'\0D\0' if p.is_dir() else b'\0F\0')
    if p.is_file(): h.update(p.read_bytes())
print(h.hexdigest())
PYS
}
assert_unchanged_after_failure() {
  local desc="$1" lock="$2" pattern="$3"; shift 3
  local before after out="$SANDBOX/fail.out" err="$SANDBOX/fail.err"
  before=$(snapshot "$lock")
  if "$@" >"$out" 2>"$err"; then
    fail "$desc（意外成功）"
    return
  fi
  after=$(snapshot "$lock")
  if [[ "$before" == "$after" ]] && grep -q "$pattern" "$err"; then
    pass "$desc"
  else
    fail "$desc（失败后字段变化或 stderr 不符）"
  fi
}
acquire() {
  local slot="$1" task="$2" branch="${3:-bugfix/$2}" agent="${4:-agent-$2}"
  bash scripts/slot_registry.sh acquire --slot "$slot" --task "$task" \
    --branch "$branch" --claim-sha "$SHA" --agent "$agent"
}
release_occupied() {
  bash scripts/slot_registry.sh occupy --slot "$1" --task "$2" >/dev/null
  bash scripts/slot_registry.sh release --slot "$1" --task "$2" >/dev/null
}
reset_registry() {
  local d slot task state
  for d in .agent-worktrees/test-registry/slot-*.lock; do
    [[ -d "$d" ]] || continue
    slot=$(basename "$d" .lock)
    task=$(field "$d/task_id")
    state=$(field "$d/state")
    case "$state" in
      reserved) bash scripts/slot_registry.sh rollback --slot "$slot" --task "$task" >/dev/null ;;
      occupied) bash scripts/slot_registry.sh release --slot "$slot" --task "$task" >/dev/null ;;
      blocked_frozen)
        bash scripts/slot_registry.sh force-unfreeze-blocked --slot "$slot" --task "$task" >/dev/null
        bash scripts/slot_registry.sh release --slot "$slot" --task "$task" >/dev/null
        ;;
      *) fail "reset 遇到未知状态 $state"; return 1 ;;
    esac
  done
}

# 隔离沙箱仓；通过 override 防止测试触及 worktree registry。
git init -q -b main "$SANDBOX/repo"
cd "$SANDBOX/repo"
git config user.email slot-test@bong.local
git config user.name slot-test
printf 'base\n' > base.txt
git add base.txt && git commit -qm base
mkdir -p scripts .agent-worktrees/test-registry .agent-worktrees/test-locks
cp "$REG" scripts/slot_registry.sh
chmod +x scripts/slot_registry.sh
export SLOT_REGISTRY_ROOT_OVERRIDE="$PWD/.agent-worktrees/test-registry"
export SLOT_REGISTRY_LOCK_ROOT_OVERRIDE="$PWD/.agent-worktrees/test-locks"
SHA=$(git rev-parse HEAD)
REGROOT="$SLOT_REGISTRY_ROOT_OVERRIDE"
LOCKROOT="$SLOT_REGISTRY_LOCK_ROOT_OVERRIDE"

printf '== 1. init / capacity / 锁目录分离\n'
out=$(bash scripts/slot_registry.sh init)
check "init 默认 capacity=2" grep -q 'capacity=2 held=0' <<<"$out"
check "永久 registry 存在" test -d "$REGROOT"
check "独立 lock root 存在" test -d "$LOCKROOT"
check "flock 文件不在永久 registry" test -f "$LOCKROOT/acquire.lock"
check_not "永久 registry 内无 acquire gate" test -e "$REGROOT/acquire.lock"
out=$(bash scripts/slot_registry.sh init --max 4)
check "init --max 4" grep -q 'capacity=4 held=0' <<<"$out"
out=$(bash scripts/slot_registry.sh capacity)
check "capacity 报告 max=4 held=0" grep -q 'max=4 held=0' <<<"$out"
help=$(bash scripts/slot_registry.sh --help)
check "help 含用法" grep -q '^用法（均在仓库根' <<<"$help"
check_not "help 不泄漏 set -euo pipefail" grep -q 'set -euo pipefail' <<<"$help"
check_not "help 不泄漏执行变量" grep -q '^cmd=' <<<"$help"

printf '== 2. 固定池边界 0 / 1 / max / max+1\n'
expect_fail "slot-0 拒绝" 'out of pool' "$SANDBOX/s0.out" "$SANDBOX/s0.err" acquire slot-0 zero
out=$(acquire slot-1 one)
check "slot-1 接受" grep -q 'OK acquire slot-1' <<<"$out"
out=$(acquire slot-4 max)
check "slot-max 接受" grep -q 'OK acquire slot-4' <<<"$out"
expect_fail "slot-max+1 拒绝" 'out of pool' "$SANDBOX/s5.out" "$SANDBOX/s5.err" acquire slot-5 over
expect_fail "前导零 slot-01 拒绝" 'out of pool' "$SANDBOX/s01.out" "$SANDBOX/s01.err" acquire slot-01 leading
check_not "slot-0 无 reservation" test -e "$REGROOT/slot-0.lock"
check_not "slot-5 无 reservation" test -e "$REGROOT/slot-5.lock"
release_occupied slot-1 one
release_occupied slot-4 max

printf '== 3. acquire 字段 / 同 slot 确定性并发\n'
acquire slot-1 owner-a 'bugfix/quote"slash\\line' 'agent-a' >/dev/null
check "state=reserved" python3 -c 'from pathlib import Path; assert Path("'$REGROOT'/slot-1.lock/state").read_bytes()==b"reserved\0"'
check "created_local=false" python3 -c 'from pathlib import Path; assert Path("'$REGROOT'/slot-1.lock/created_local_branch").read_bytes()==b"false\0"'
json=$(bash scripts/slot_registry.sh status --json)
check "is-held 返回 HELD" bash scripts/slot_registry.sh is-held --slot slot-1
check "status 文本可观察" grep -q 'slot-1: task=owner-a state=reserved' <<<"$(bash scripts/slot_registry.sh status)"
check "status JSON 基础 schema" python3 -c 'import json,sys; o=json.load(sys.stdin); assert o["max"]==4 and o["held"]==1 and o["slots"][0]["task_id"]=="owner-a"' <<<"$json"
check "claim_sha 小写" python3 -c 'from pathlib import Path; import sys; assert Path("'$REGROOT'/slot-1.lock/claim_sha").read_bytes()==sys.argv[1].encode()+b"\0"' "$SHA"
expect_fail "同 slot 第二 acquire 拒绝" 'slot busy' "$SANDBOX/same.out" "$SANDBOX/same.err" acquire slot-1 owner-b
check "竞争后 holder 不变" python3 -c 'from pathlib import Path; assert Path("'$REGROOT'/slot-1.lock/task_id").read_bytes()==b"owner-a\0"'
release_occupied slot-1 owner-a

printf '== 4. 跨/同 slot 高并发 capacity 不超限\n'
bash scripts/slot_registry.sh init --max 4 >/dev/null
acquire slot-1 base-1 >/dev/null
acquire slot-2 base-2 >/dev/null
HOLD_READY="$SANDBOX/cap.hold.ready"; HOLD_GO="$SANDBOX/cap.hold.go"
P4_READY="$SANDBOX/cap4.ready"; P4_GO="$SANDBOX/cap4.go"; P4_ACK="$SANDBOX/cap4.ack"
SAME_READY="$SANDBOX/cap-same.ready"; SAME_GO="$SANDBOX/cap-same.go"; SAME_ACK="$SANDBOX/cap-same.ack"
mkfifo "$HOLD_READY" "$HOLD_GO" "$P4_READY" "$P4_GO" "$P4_ACK" "$SAME_READY" "$SAME_GO" "$SAME_ACK"
register_fifo_fd "$HOLD_READY"
register_fifo_fd "$HOLD_GO" CAP_HOLD_GO_FD; RELEASE_FDS+=("$CAP_HOLD_GO_FD")
register_fifo_fd "$P4_READY"
register_fifo_fd "$P4_GO" P4_GO_FD; RELEASE_FDS+=("$P4_GO_FD")
register_fifo_fd "$P4_ACK"
register_fifo_fd "$SAME_READY"
register_fifo_fd "$SAME_GO" SAME_GO_FD; RELEASE_FDS+=("$SAME_GO_FD")
register_fifo_fd "$SAME_ACK"
CAP_TOKEN="cap-$RANDOM-$BASHPID"
env SLOT_REGISTRY_TEST_INSTANCE="$CAP_TOKEN" \
  SLOT_REGISTRY_TEST_HOLD_GATE_READY="$HOLD_READY" SLOT_REGISTRY_TEST_HOLD_GATE_RELEASE="$HOLD_GO" \
  bash scripts/slot_registry.sh acquire --slot slot-3 --task race-three --branch bugfix/race-three \
    --claim-sha "$SHA" --agent agent-race-three >"$SANDBOX/cap3.out" 2>"$SANDBOX/cap3.err" &
p3=$!; register_test_pid "$p3"
read_gate_ready "$HOLD_READY" "$CAP_TOKEN"
p3_holder=$GATE_HOLDER_PID; register_test_pid "$p3_holder"
if pid_belongs_to_this_test "$p3_holder"; then
  pass "跨 slot holder 属于本次沙箱"
else
  fail "跨 slot holder 不属于本次沙箱"
fi
env SLOT_REGISTRY_GATE_WAIT_SEC=3 SLOT_REGISTRY_TEST_WAIT_GATE_READY="$P4_READY" \
  SLOT_REGISTRY_TEST_WAIT_GATE_RELEASE="$P4_GO" SLOT_REGISTRY_TEST_WAIT_GATE_ACK="$P4_ACK" \
  bash scripts/slot_registry.sh acquire --slot slot-4 --task race-four \
    --branch bugfix/race-four --claim-sha "$SHA" --agent agent-race-four \
    >"$SANDBOX/cap4.out" 2>"$SANDBOX/cap4.err" &
p4=$!; register_test_pid "$p4"
env SLOT_REGISTRY_GATE_WAIT_SEC=3 SLOT_REGISTRY_TEST_WAIT_GATE_READY="$SAME_READY" \
  SLOT_REGISTRY_TEST_WAIT_GATE_RELEASE="$SAME_GO" SLOT_REGISTRY_TEST_WAIT_GATE_ACK="$SAME_ACK" \
  bash scripts/slot_registry.sh acquire --slot slot-3 --task same-slot \
    --branch bugfix/same-slot --claim-sha "$SHA" --agent agent-same-slot \
    >"$SANDBOX/cap-same.out" 2>"$SANDBOX/cap-same.err" &
p_same=$!; register_test_pid "$p_same"
if read_waiter_ready "$P4_READY" && read_waiter_ready "$SAME_READY"; then
  pass "所有不同/同 slot 竞争者均到达确定性 ready barrier"
else
  fail "竞争者未全部到达确定性 ready barrier"
fi
release_waiter "$P4_GO_FD"
release_waiter "$SAME_GO_FD"
if read_waiter_ack "$P4_ACK" && read_waiter_ack "$SAME_ACK"; then
  pass "所有竞争者统一 release 后才进入 flock 竞争"
else
  fail "竞争者 release ACK 不完整"
fi
printf 'release\n' >&"$CAP_HOLD_GO_FD"
set +e
wait "$p3"; rc3=$?
wait "$p4"; rc4=$?
wait "$p_same"; rcs=$?
set -e
if [[ $rc3 -eq 0 && $rc4 -eq 0 && $rcs -ne 0 ]]; then
  pass "两个不同空闲 slot 并发成功、同 slot 竞争者失败"
else
  fail "跨/同 slot 并发结果 rc=$rc3/$rc4/$rcs"
fi
out=$(bash scripts/slot_registry.sh capacity)
check "高并发后 held=max=4" grep -q 'max=4 held=4' <<<"$out"
check "不同空闲 slot-3 holder 正确" python3 -c 'from pathlib import Path; assert Path("'$REGROOT'/slot-3.lock/task_id").read_bytes()==b"race-three\0"'
check "不同空闲 slot-4 holder 正确" python3 -c 'from pathlib import Path; assert Path("'$REGROOT'/slot-4.lock/task_id").read_bytes()==b"race-four\0"'
check "同 slot loser 报 busy 或 capacity" grep -Eq 'slot busy|capacity full' "$SANDBOX/cap-same.err"
release_occupied slot-1 base-1
release_occupied slot-2 base-2
release_occupied slot-3 race-three
release_occupied slot-4 race-four

printf '== 5. flock 超时 fail-closed 与正常释放\n'
READY="$SANDBOX/timeout.ready"; GO="$SANDBOX/timeout.go"; mkfifo "$READY" "$GO"
register_fifo_fd "$READY"
register_fifo_fd "$GO" TIMEOUT_GO_FD; RELEASE_FDS+=("$TIMEOUT_GO_FD")
TIMEOUT_TOKEN="timeout-$RANDOM-$BASHPID"
env SLOT_REGISTRY_TEST_INSTANCE="$TIMEOUT_TOKEN" \
  SLOT_REGISTRY_TEST_HOLD_GATE_READY="$READY" SLOT_REGISTRY_TEST_HOLD_GATE_RELEASE="$GO" \
  bash scripts/slot_registry.sh acquire --slot slot-1 --task lock-holder --branch bugfix/lock-holder \
    --claim-sha "$SHA" --agent agent-lock-holder >"$SANDBOX/holder.out" 2>"$SANDBOX/holder.err" &
holder_wrapper_pid=$!; register_test_pid "$holder_wrapper_pid"
read_gate_ready "$READY" "$TIMEOUT_TOKEN"
holder_actual_pid=$GATE_HOLDER_PID; register_test_pid "$holder_actual_pid"
check "超时用例命中真实 holder" pid_belongs_to_this_test "$holder_actual_pid"
expect_fail "争用超时 fail-closed" 'acquire gate busy' "$SANDBOX/timeout.out" "$SANDBOX/timeout.err" \
  env SLOT_REGISTRY_GATE_WAIT_SEC=0.15 bash scripts/slot_registry.sh acquire --slot slot-2 --task timeout \
    --branch bugfix/timeout --claim-sha "$SHA" --agent timeout
check_not "超时未创建 slot-2" test -e "$REGROOT/slot-2.lock"
printf 'release\n' >&"$TIMEOUT_GO_FD"
wait "$holder_wrapper_pid"
check_not "正常释放后 holder 已退出" kill -0 "$holder_actual_pid"
check "正常释放 gate 后可继续 acquire" acquire slot-2 after-timeout
release_occupied slot-1 lock-holder
release_occupied slot-2 after-timeout

printf '== 6. SIGKILL 真 holder 身份硬门与同一 flock inode 恢复\n'
READY="$SANDBOX/kill.ready"; GO="$SANDBOX/kill.go"; mkfifo "$READY" "$GO"
register_fifo_fd "$READY"
register_fifo_fd "$GO" KILL_GO_FD; RELEASE_FDS+=("$KILL_GO_FD")
KILL_TOKEN="kill-$RANDOM-$BASHPID"
env SLOT_REGISTRY_TEST_INSTANCE="$KILL_TOKEN" \
  SLOT_REGISTRY_TEST_HOLD_GATE_READY="$READY" SLOT_REGISTRY_TEST_HOLD_GATE_RELEASE="$GO" \
  bash scripts/slot_registry.sh acquire --slot slot-1 --task doomed --branch bugfix/doomed \
    --claim-sha "$SHA" --agent agent-doomed >"$SANDBOX/doomed.out" 2>"$SANDBOX/doomed.err" &
doomed_wrapper_pid=$!; register_test_pid "$doomed_wrapper_pid"
read_gate_ready "$READY" "$KILL_TOKEN"
doomed_holder_pid=$GATE_HOLDER_PID; doomed_holder_fd=$GATE_HOLDER_FD
register_test_pid "$doomed_holder_pid"
holder_start_ticks=$GATE_HOLDER_START
lock_inode_before=$(stat -Lc '%d:%i' "$LOCKROOT/acquire.lock")

SENTINEL_TOKEN="sentinel-$RANDOM-$BASHPID"
SENTINEL_GO="$SANDBOX/sentinel.go"; mkfifo "$SENTINEL_GO"
register_fifo_fd "$SENTINEL_GO" SENTINEL_GO_FD; RELEASE_FDS+=("$SENTINEL_GO_FD")
env SLOT_REGISTRY_TEST_INSTANCE="$SENTINEL_TOKEN" bash -c 'IFS= read -r _ < "$1"' _ "$SENTINEL_GO" \
  >"$SANDBOX/sentinel.out" 2>"$SANDBOX/sentinel.err" &
sentinel_pid=$!; register_test_pid "$sentinel_pid"
sentinel_start_before=$(proc_start_ticks "$sentinel_pid")
if kill_verified_gate_holder "$sentinel_pid" 0 "$sentinel_start_before" \
    "$LOCKROOT/acquire.lock" "$KILL_TOKEN" "$READY" "$GO" \
    >"$SANDBOX/wrong-live.out" 2>"$SANDBOX/wrong-live.err"; then
  fatal "任意存活进程意外通过 holder 身份硬门"
elif kill -0 "$sentinel_pid" 2>/dev/null && [[ "$(proc_start_ticks "$sentinel_pid")" == "$sentinel_start_before" ]]; then
  pass "错误握手不会触碰任意存活哨兵进程"
else
  fatal "错误握手触碰了存活哨兵进程"
fi

if kill_verified_gate_holder 999999999 "$doomed_holder_fd" "$holder_start_ticks" \
    "$LOCKROOT/acquire.lock" "$KILL_TOKEN" "$READY" "$GO" \
    >"$SANDBOX/dead-pid.out" 2>"$SANDBOX/dead-pid.err"; then
  fatal "不存活 PID 意外通过 holder 身份硬门"
elif kill -0 "$doomed_holder_pid" 2>/dev/null; then
  pass "不存活 PID 握手失败不会触碰真实 holder"
else
  fatal "不存活 PID 负向用例触碰了真实 holder"
fi

WRONG_TOKEN="wrong-$RANDOM-$BASHPID"
if kill_verified_gate_holder "$doomed_holder_pid" "$doomed_holder_fd" "$holder_start_ticks" \
    "$LOCKROOT/acquire.lock" "$WRONG_TOKEN" "$READY" "$GO" \
    >"$SANDBOX/wrong-token.out" 2>"$SANDBOX/wrong-token.err"; then
  fatal "非本轮 token 意外通过 holder 身份硬门"
elif kill -0 "$doomed_holder_pid" 2>/dev/null; then
  pass "非本轮 token 握手失败不会触碰真实 holder"
else
  fatal "非本轮 token 负向用例触碰了真实 holder"
fi

wrong_start=$((holder_start_ticks + 1))
if kill_verified_gate_holder "$doomed_holder_pid" "$doomed_holder_fd" "$wrong_start" \
    "$LOCKROOT/acquire.lock" "$KILL_TOKEN" "$READY" "$GO" \
    >"$SANDBOX/wrong-start.out" 2>"$SANDBOX/wrong-start.err"; then
  fatal "start ticks 不符意外通过 holder 身份硬门"
elif kill -0 "$doomed_holder_pid" 2>/dev/null; then
  pass "start ticks 不符不会触碰真实 holder"
else
  fatal "start ticks 负向用例触碰了真实 holder"
fi

if kill_verified_gate_holder "$doomed_holder_pid" 0 "$holder_start_ticks" \
    "$LOCKROOT/acquire.lock" "$KILL_TOKEN" "$READY" "$GO" \
    >"$SANDBOX/wrong-inode.out" 2>"$SANDBOX/wrong-inode.err"; then
  fatal "FD target inode 不符意外通过 holder 身份硬门"
elif kill -0 "$doomed_holder_pid" 2>/dev/null; then
  pass "FD target inode 不符不会触碰真实 holder"
else
  fatal "FD inode 负向用例触碰了真实 holder"
fi

if ! kill -0 "$doomed_holder_pid" 2>/dev/null; then
  fatal "真实 holder 在验证 SIGKILL 前已退出"
fi
if ! kill_verified_gate_holder "$doomed_holder_pid" "$doomed_holder_fd" "$holder_start_ticks" \
    "$LOCKROOT/acquire.lock" "$KILL_TOKEN" "$READY" "$GO"; then
  fatal "真实 holder 身份硬门未通过，已中止且未执行 SIGKILL"
fi
pass "PID 存活、本轮身份、start ticks、FD target inode 与锁状态全通过后才 SIGKILL"
set +e
wait "$doomed_wrapper_pid" 2>/dev/null
kill_rc=$?
set -e
if [[ $kill_rc -ne 0 ]]; then pass "真实持锁进程被 SIGKILL"; else fail "SIGKILL 进程意外零退出"; fi
check_not "真实 holder PID 已退出" kill -0 "$doomed_holder_pid"
check_not "旧 holder FD 已关闭" test -e "/proc/$doomed_holder_pid/fd/$doomed_holder_fd"
check "本次 hook 无 orphan 进程" assert_no_hook_holder "$READY"
check "SIGKILL 发生在 reservation 前" test ! -e "$REGROOT/slot-1.lock"
check "锁路径仍为同一 inode（未删除重建假恢复）" \
  test "$(stat -Lc '%d:%i' "$LOCKROOT/acquire.lock")" = "$lock_inode_before"
out=$(SLOT_REGISTRY_GATE_WAIT_SEC=1 acquire slot-2 recovered)
check "异常退出后同一 inode 上后继 acquire 成功" grep -q 'OK acquire slot-2' <<<"$out"
check "恢复后本次 hook 仍无 orphan" assert_no_hook_holder "$READY"
release_occupied slot-2 recovered
printf 'release\n' >&"$SENTINEL_GO_FD"
wait "$sentinel_pid"

printf '== 7. acquire 临时 reservation 故障注入 fail-clean\n'
acquire slot-1 permanent-owner >/dev/null
permanent_lock="$REGROOT/slot-1.lock"
for step in write date mv; do
  if assert_acquire_failure_clean "$step" slot-2 "inject-$step" "$permanent_lock"; then
    pass "$step 失败显式非零、无 temp/假 held 且永久 registry 不变"
  else
    fail "$step 失败未满足 fail-clean 契约"
  fi
done
check "故障注入后 capacity 仍只计永久 holder" bash -c 'grep -q "max=4 held=1" <<<"$(bash scripts/slot_registry.sh capacity)"'
release_occupied slot-1 permanent-owner

printf '== 8. reserved 合法转换与 rollback 删除授权\n'
acquire slot-1 sm-reserved >/dev/null
bash scripts/slot_registry.sh mark-created-local --slot slot-1 --task sm-reserved --value false >/dev/null
check "reserved 可幂等 mark false" python3 -c 'from pathlib import Path; assert Path("'$REGROOT'/slot-1.lock/created_local_branch").read_bytes()==b"false\0"'
bash scripts/slot_registry.sh mark-created-local --slot slot-1 --task sm-reserved --value true >/dev/null
check "reserved 可 false→true" python3 -c 'from pathlib import Path; assert Path("'$REGROOT'/slot-1.lock/created_local_branch").read_bytes()==b"true\0"'
bash scripts/slot_registry.sh mark-created-local --slot slot-1 --task sm-reserved --value true >/dev/null
check "reserved 可幂等 mark true" python3 -c 'from pathlib import Path; assert Path("'$REGROOT'/slot-1.lock/created_local_branch").read_bytes()==b"true\0"'
assert_unchanged_after_failure "created_local true→false 拒绝且字段不变" "$REGROOT/slot-1.lock" 'monotonic' \
  bash scripts/slot_registry.sh mark-created-local --slot slot-1 --task sm-reserved --value false
assert_unchanged_after_failure "reserved 上 release 拒绝且不变" "$REGROOT/slot-1.lock" 'invalid state transition' \
  bash scripts/slot_registry.sh release --slot slot-1 --task sm-reserved
assert_unchanged_after_failure "reserved 上 force-unfreeze 拒绝且不变" "$REGROOT/slot-1.lock" 'invalid state transition' \
  bash scripts/slot_registry.sh force-unfreeze-blocked --slot slot-1 --task sm-reserved
out=$(bash scripts/slot_registry.sh rollback --slot slot-1 --task sm-reserved)
check "reserved rollback 输出 DELETE=true" grep -q 'DELETE_LOCAL_BRANCH=true' <<<"$out"
check_not "rollback 清 reservation" test -e "$REGROOT/slot-1.lock"
out=$(bash scripts/slot_registry.sh rollback --slot slot-1 --task sm-reserved)
check "free rollback 幂等 DELETE=false" grep -q 'DELETE_LOCAL_BRANCH=false' <<<"$out"

printf '== 9. occupied 状态合法/非法边\n'
acquire slot-1 sm-occupied >/dev/null
bash scripts/slot_registry.sh occupy --slot slot-1 --task sm-occupied >/dev/null
check "reserved→occupied" python3 -c 'from pathlib import Path; assert Path("'$REGROOT'/slot-1.lock/state").read_bytes()==b"occupied\0"'
assert_unchanged_after_failure "occupied→occupy 拒绝且不变" "$REGROOT/slot-1.lock" 'invalid state transition' \
  bash scripts/slot_registry.sh occupy --slot slot-1 --task sm-occupied
assert_unchanged_after_failure "occupied 上 mark 拒绝且不变" "$REGROOT/slot-1.lock" 'invalid state transition' \
  bash scripts/slot_registry.sh mark-created-local --slot slot-1 --task sm-occupied --value true
assert_unchanged_after_failure "occupied 上 rollback 拒绝且不变" "$REGROOT/slot-1.lock" 'invalid state transition' \
  bash scripts/slot_registry.sh rollback --slot slot-1 --task sm-occupied
assert_unchanged_after_failure "holder mismatch 拒绝且不变" "$REGROOT/slot-1.lock" 'holder mismatch' \
  bash scripts/slot_registry.sh freeze-blocked --slot slot-1 --task stranger
bash scripts/slot_registry.sh release --slot slot-1 --task sm-occupied >/dev/null
check_not "occupied→free release" test -e "$REGROOT/slot-1.lock"
out=$(bash scripts/slot_registry.sh release --slot slot-1 --task sm-occupied)
check "free release 幂等" grep -q 'already free' <<<"$out"
out=$(bash scripts/slot_registry.sh rollback --slot slot-1 --task sm-occupied)
check "free rollback 幂等且无删除授权" grep -q 'DELETE_LOCAL_BRANCH=false' <<<"$out"
for spec in occupy:occupy freeze-blocked:freeze force-unfreeze-blocked:force mark-created-local:mark; do
  cmd=${spec%%:*}; label=${spec#*:}
  args=(bash scripts/slot_registry.sh "$cmd" --slot slot-1 --task free-task)
  [[ "$cmd" == mark-created-local ]] && args+=(--value true)
  expect_fail "free 上 $label 拒绝" 'slot not held' "$SANDBOX/free-${cmd}.out" "$SANDBOX/free-${cmd}.err" "${args[@]}"
  check_not "free 上 $label 失败不创建 reservation" test -e "$REGROOT/slot-1.lock"
done

printf '== 10. blocked_frozen 全部普通出口 fail-closed\n'
acquire slot-1 sm-frozen-r >/dev/null
bash scripts/slot_registry.sh freeze-blocked --slot slot-1 --task sm-frozen-r >/dev/null
check "reserved→blocked_frozen" python3 -c 'from pathlib import Path; assert Path("'$REGROOT'/slot-1.lock/state").read_bytes()==b"blocked_frozen\0"'
for spec in occupy:occupy mark-created-local:mark rollback:rollback release:release freeze-blocked:freeze; do
  cmd=${spec%%:*}; label=${spec#*:}
  args=(bash scripts/slot_registry.sh "$cmd" --slot slot-1 --task sm-frozen-r)
  [[ "$cmd" == mark-created-local ]] && args+=(--value true)
  assert_unchanged_after_failure "frozen 上 $label 拒绝且不变" "$REGROOT/slot-1.lock" 'invalid state transition' "${args[@]}"
done
bash scripts/slot_registry.sh force-unfreeze-blocked --slot slot-1 --task sm-frozen-r >/dev/null
check "人工解冻 blocked_frozen→occupied" python3 -c 'from pathlib import Path; assert Path("'$REGROOT'/slot-1.lock/state").read_bytes()==b"occupied\0"'
assert_unchanged_after_failure "occupied 上 force-unfreeze 拒绝" "$REGROOT/slot-1.lock" 'invalid state transition' \
  bash scripts/slot_registry.sh force-unfreeze-blocked --slot slot-1 --task sm-frozen-r
bash scripts/slot_registry.sh freeze-blocked --slot slot-1 --task sm-frozen-r >/dev/null
check "occupied→blocked_frozen" python3 -c 'from pathlib import Path; assert Path("'$REGROOT'/slot-1.lock/state").read_bytes()==b"blocked_frozen\0"'
bash scripts/slot_registry.sh force-unfreeze-blocked --slot slot-1 --task sm-frozen-r >/dev/null
bash scripts/slot_registry.sh release --slot slot-1 --task sm-frozen-r >/dev/null

printf '== 11. 所有 slot 命令统一边界校验\n'
for cmd in is-held occupy freeze-blocked force-unfreeze-blocked release rollback; do
  expect_fail "$cmd 拒绝 slot-0" 'out of pool' "$SANDBOX/${cmd}0.out" "$SANDBOX/${cmd}0.err" \
    bash scripts/slot_registry.sh "$cmd" --slot slot-0 --task nobody
done
expect_fail "mark-created-local 拒绝 max+1" 'out of pool' "$SANDBOX/mark5.out" "$SANDBOX/mark5.err" \
  bash scripts/slot_registry.sh mark-created-local --slot slot-5 --task nobody --value true

printf '== 12. NUL 字段 trailing LF / 内嵌 LF / Unicode exact round-trip\n'
task=$'持有者\n内嵌\n尾随\n\n'
branch=$'bugfix/分支\n尾随一行\n'
agent=$'代理-雪\n内嵌\n尾随三行\n\n\n'
acquire slot-1 "$task" "$branch" "$agent" >/dev/null
check "task NUL 字段逐字节保留一个/多个尾随 LF" python3 - "$REGROOT/slot-1.lock/task_id" "$task" <<'PYFIELD'
from pathlib import Path
import os, sys
assert Path(sys.argv[1]).read_bytes() == os.fsencode(sys.argv[2]) + b'\0'
PYFIELD
check "branch NUL 字段逐字节保留 Unicode/内嵌/尾随 LF" python3 - "$REGROOT/slot-1.lock/branch" "$branch" <<'PYFIELD'
from pathlib import Path
import os, sys
assert Path(sys.argv[1]).read_bytes() == os.fsencode(sys.argv[2]) + b'\0'
PYFIELD
check "agent NUL 字段逐字节保留多个尾随 LF" python3 - "$REGROOT/slot-1.lock/agent_id" "$agent" <<'PYFIELD'
from pathlib import Path
import os, sys
assert Path(sys.argv[1]).read_bytes() == os.fsencode(sys.argv[2]) + b'\0'
PYFIELD
check "holder identity 含尾随 LF 时 exact 匹配可操作" \
  bash scripts/slot_registry.sh mark-created-local --slot slot-1 --task "$task" --value true
wrong_task=${task%$'\n'}
expect_fail "holder identity 少一个尾随 LF 必须拒绝" 'holder mismatch' \
  "$SANDBOX/trailing-holder.out" "$SANDBOX/trailing-holder.err" \
  bash scripts/slot_registry.sh rollback --slot slot-1 --task "$wrong_task"
json=$(bash scripts/slot_registry.sh status --json)
TASK_EXPECT="$task" BRANCH_EXPECT="$branch" AGENT_EXPECT="$agent" JSON_PAYLOAD="$json" python3 - <<'PYJ'
import json, os
obj=json.loads(os.environ['JSON_PAYLOAD'])
assert obj['max'] == 4 and obj['held'] == 1
assert len(obj['slots']) == 1
slot=obj['slots'][0]
assert slot['task_id'].encode() == os.environ['TASK_EXPECT'].encode()
assert slot['branch'].encode() == os.environ['BRANCH_EXPECT'].encode()
assert slot['agent_id'].encode() == os.environ['AGENT_EXPECT'].encode()
assert slot['state'] == 'reserved'
assert slot['created_local_branch'] == 'true'
assert isinstance(slot['reserved_at'], str) and slot['reserved_at']
PYJ
pass "status --json exact round-trip Unicode、内嵌 LF 与全部尾随 LF"
out=$(bash scripts/slot_registry.sh rollback --slot slot-1 --task "$task")
check "exact holder rollback 保留 DELETE_LOCAL_BRANCH=true" grep -q 'DELETE_LOCAL_BRANCH=true' <<<"$out"
json=$(bash scripts/slot_registry.sh status --json)
check "空 registry JSON schema" python3 -c 'import json,sys; o=json.load(sys.stdin); assert o["slots"]==[] and o["held"]==0' <<<"$json"

printf '== 13. 非法输入与 capacity 收缩 fail-closed\n'
expect_fail "短 claim-sha 拒绝" 'claim-sha must' "$SANDBOX/sha.out" "$SANDBOX/sha.err" \
  bash scripts/slot_registry.sh acquire --slot slot-1 --task bad --branch bad --claim-sha deadbeef --agent bad
expect_fail "非法 slot 名拒绝" 'invalid slot name' "$SANDBOX/name.out" "$SANDBOX/name.err" acquire not-a-slot bad
acquire slot-4 high-slot >/dev/null
expect_fail "init 不得缩到 held slot 以下" 'cannot shrink capacity below held slot' "$SANDBOX/shrink.out" "$SANDBOX/shrink.err" \
  bash scripts/slot_registry.sh init --max 3
check "失败收缩保持 capacity=4" bash -c 'grep -q "max=4 held=1" <<<"$(bash scripts/slot_registry.sh capacity)"'
release_occupied slot-4 high-slot
bash scripts/slot_registry.sh init --max 2 >/dev/null
check "最终恢复 max=2 held=0" bash -c 'grep -q "max=2 held=0" <<<"$(bash scripts/slot_registry.sh capacity)"'
mkdir "$REGROOT/slot-1.lock"
printf broken > "$REGROOT/slot-1.lock/task_id"
printf 'reserved\0' > "$REGROOT/slot-1.lock/state"
expect_fail "损坏字段查询 fail-closed" 'corrupt field' "$SANDBOX/corrupt.out" "$SANDBOX/corrupt.err" \
  bash scripts/slot_registry.sh is-held --slot slot-1
rm -rf "$REGROOT/slot-1.lock"

printf '%s\n' '---'
printf 'PASS=%s FAIL=%s\n' "$PASS" "$FAIL"
[[ $FAIL -eq 0 ]] || exit 1
printf 'slot_registry 契约测试全部通过\n'
