#!/usr/bin/env bash
# slot_registry.sh 饱和契约测试：原子容量、固定池边界、显式状态机、锁恢复与 JSON。
set -euo pipefail

REG=$(realpath "$(dirname "$0")/../slot_registry.sh")
SANDBOX=$(mktemp -d /tmp/slot-registry-test.XXXXXX)
TEST_PIDS=()
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
  local pid
  for pid in "${TEST_PIDS[@]}"; do
    if [[ "$pid" =~ ^[0-9]+$ ]] && kill -0 "$pid" 2>/dev/null && pid_belongs_to_this_test "$pid"; then
      kill -KILL "$pid" 2>/dev/null || true
    fi
  done
  for pid in "${TEST_PIDS[@]}"; do
    [[ "$pid" =~ ^[0-9]+$ ]] && wait "$pid" 2>/dev/null || true
  done
  rm -rf "$SANDBOX"
}
trap cleanup EXIT

PASS=0
FAIL=0
pass() { printf '  PASS: %s\n' "$1"; PASS=$((PASS + 1)); }
fail() { printf '  FAIL: %s\n' "$1"; FAIL=$((FAIL + 1)); }
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
  local fifo="$1"
  IFS=' ' read -r GATE_WORD GATE_HOLDER_PID GATE_HOLDER_FD < "$fifo"
  if [[ "$GATE_WORD" == ready && "$GATE_HOLDER_PID" =~ ^[0-9]+$ && "$GATE_HOLDER_FD" =~ ^[0-9]+$ ]]; then
    return 0
  fi
  fail "gate ready 握手必须携带真实 holder PID/FD"
  return 1
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
READY="$SANDBOX/cap.ready"; GO="$SANDBOX/cap.go"; mkfifo "$READY" "$GO"
env SLOT_REGISTRY_TEST_HOLD_GATE_READY="$READY" SLOT_REGISTRY_TEST_HOLD_GATE_RELEASE="$GO" \
  bash scripts/slot_registry.sh acquire --slot slot-3 --task race-three --branch bugfix/race-three \
    --claim-sha "$SHA" --agent agent-race-three >"$SANDBOX/cap3.out" 2>"$SANDBOX/cap3.err" &
p3=$!; TEST_PIDS+=("$p3")
read_gate_ready "$READY"
p3_holder=$GATE_HOLDER_PID; TEST_PIDS+=("$p3_holder")
check "跨 slot holder 属于本次沙箱" pid_belongs_to_this_test "$p3_holder"
env SLOT_REGISTRY_GATE_WAIT_SEC=3 bash scripts/slot_registry.sh acquire --slot slot-4 --task race-four \
  --branch bugfix/race-four --claim-sha "$SHA" --agent agent-race-four \
  >"$SANDBOX/cap4.out" 2>"$SANDBOX/cap4.err" &
p4=$!; TEST_PIDS+=("$p4")
env SLOT_REGISTRY_GATE_WAIT_SEC=3 bash scripts/slot_registry.sh acquire --slot slot-3 --task same-slot \
  --branch bugfix/same-slot --claim-sha "$SHA" --agent agent-same-slot \
  >"$SANDBOX/cap-same.out" 2>"$SANDBOX/cap-same.err" &
p_same=$!; TEST_PIDS+=("$p_same")
printf 'release\n' > "$GO"
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
env SLOT_REGISTRY_TEST_HOLD_GATE_READY="$READY" SLOT_REGISTRY_TEST_HOLD_GATE_RELEASE="$GO" \
  bash scripts/slot_registry.sh acquire --slot slot-1 --task lock-holder --branch bugfix/lock-holder \
    --claim-sha "$SHA" --agent agent-lock-holder >"$SANDBOX/holder.out" 2>"$SANDBOX/holder.err" &
holder_wrapper_pid=$!; TEST_PIDS+=("$holder_wrapper_pid")
read_gate_ready "$READY"
holder_actual_pid=$GATE_HOLDER_PID; TEST_PIDS+=("$holder_actual_pid")
check "超时用例命中真实 holder" pid_belongs_to_this_test "$holder_actual_pid"
expect_fail "争用超时 fail-closed" 'acquire gate busy' "$SANDBOX/timeout.out" "$SANDBOX/timeout.err" \
  env SLOT_REGISTRY_GATE_WAIT_SEC=0.15 bash scripts/slot_registry.sh acquire --slot slot-2 --task timeout \
    --branch bugfix/timeout --claim-sha "$SHA" --agent timeout
check_not "超时未创建 slot-2" test -e "$REGROOT/slot-2.lock"
printf 'release\n' > "$GO"
wait "$holder_wrapper_pid"
check_not "正常释放后 holder 已退出" kill -0 "$holder_actual_pid"
check "正常释放 gate 后可继续 acquire" acquire slot-2 after-timeout
release_occupied slot-1 lock-holder
release_occupied slot-2 after-timeout

printf '== 6. SIGKILL 真 holder 后内核释放同一 flock inode\n'
READY="$SANDBOX/kill.ready"; GO="$SANDBOX/kill.go"; mkfifo "$READY" "$GO"
env SLOT_REGISTRY_TEST_HOLD_GATE_READY="$READY" SLOT_REGISTRY_TEST_HOLD_GATE_RELEASE="$GO" \
  bash scripts/slot_registry.sh acquire --slot slot-1 --task doomed --branch bugfix/doomed \
    --claim-sha "$SHA" --agent agent-doomed >"$SANDBOX/doomed.out" 2>"$SANDBOX/doomed.err" &
doomed_wrapper_pid=$!; TEST_PIDS+=("$doomed_wrapper_pid")
read_gate_ready "$READY"
doomed_holder_pid=$GATE_HOLDER_PID; doomed_holder_fd=$GATE_HOLDER_FD
TEST_PIDS+=("$doomed_holder_pid")
check "SIGKILL 目标是真实本次 holder" pid_belongs_to_this_test "$doomed_holder_pid"
lock_inode_before=$(stat -Lc '%d:%i' "$LOCKROOT/acquire.lock")
holder_inode_before=$(stat -Lc '%d:%i' "/proc/$doomed_holder_pid/fd/$doomed_holder_fd")
if [[ "$lock_inode_before" == "$holder_inode_before" ]]; then
  pass "hook 回报 FD 确实持有当前 acquire.lock inode"
else
  fail "hook FD inode 与 acquire.lock 不一致"
fi
kill -KILL "$doomed_holder_pid"
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

printf '== 7. reserved 合法转换与 rollback 删除授权\n'
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

printf '== 8. occupied 状态合法/非法边\n'
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

printf '== 9. blocked_frozen 全部普通出口 fail-closed\n'
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

printf '== 10. 所有 slot 命令统一边界校验\n'
for cmd in is-held occupy freeze-blocked force-unfreeze-blocked release rollback; do
  expect_fail "$cmd 拒绝 slot-0" 'out of pool' "$SANDBOX/${cmd}0.out" "$SANDBOX/${cmd}0.err" \
    bash scripts/slot_registry.sh "$cmd" --slot slot-0 --task nobody
done
expect_fail "mark-created-local 拒绝 max+1" 'out of pool' "$SANDBOX/mark5.out" "$SANDBOX/mark5.err" \
  bash scripts/slot_registry.sh mark-created-local --slot slot-5 --task nobody --value true

printf '== 11. status --json 特殊字符 exact round-trip\n'
task=$'owner"\\actual\nnewline'
branch=$'bugfix/branch"\\actual\nnewline'
agent=$'worktree"\\actual\nerror'
acquire slot-1 "$task" "$branch" "$agent" >/dev/null
json=$(bash scripts/slot_registry.sh status --json)
TASK_EXPECT="$task" BRANCH_EXPECT="$branch" AGENT_EXPECT="$agent" JSON_PAYLOAD="$json" python3 - <<'PYJ'
import json, os
obj=json.loads(os.environ['JSON_PAYLOAD'])
assert obj['max'] == 4 and obj['held'] == 1
assert len(obj['slots']) == 1
slot=obj['slots'][0]
assert slot['task_id'] == os.environ['TASK_EXPECT']
assert slot['branch'] == os.environ['BRANCH_EXPECT']
assert slot['agent_id'] == os.environ['AGENT_EXPECT']
assert slot['state'] == 'reserved'
assert slot['created_local_branch'] == 'false'
assert isinstance(slot['reserved_at'], str) and slot['reserved_at']
PYJ
pass "Python json parser 解析并 exact round-trip 引号/反斜杠/实际换行"
out=$(bash scripts/slot_registry.sh status --json)
check "JSON 物理输出为单行" bash -c '[[ $(printf %s "$1" | wc -l) -eq 0 ]]' _ "$out"
bash scripts/slot_registry.sh rollback --slot slot-1 --task "$task" >/dev/null
json=$(bash scripts/slot_registry.sh status --json)
check "空 registry JSON schema" python3 -c 'import json,sys; o=json.load(sys.stdin); assert o["slots"]==[] and o["held"]==0' <<<"$json"

printf '== 12. 非法输入与 capacity 收缩 fail-closed\n'
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
