#!/usr/bin/env bash
# slot_registry.sh 饱和契约测试：全局审计、一一所有权、真实进驻门、冻结与锁释放。
set -euo pipefail

REG=$(realpath "$(dirname "$0")/../slot_registry.sh")
SANDBOX=$(mktemp -d /tmp/slot-registry-test.XXXXXX)
TEST_PIDS=()
FIFO_FDS=()
RELEASE_FDS=()
cleanup() {
  trap - EXIT
  local pid fd
  for fd in "${RELEASE_FDS[@]}"; do printf 'release\n' >&"$fd" 2>/dev/null || true; done
  for pid in "${TEST_PIDS[@]}"; do wait "$pid" 2>/dev/null || true; done
  for fd in "${FIFO_FDS[@]}"; do eval "exec ${fd}>&-" || true; done
  rm -rf "$SANDBOX"
}
trap cleanup EXIT

PASS=0; FAIL=0
pass() { printf '  PASS: %s\n' "$1"; PASS=$((PASS + 1)); }
fail() { printf '  FAIL: %s\n' "$1"; FAIL=$((FAIL + 1)); }
check() { local d="$1"; shift; if "$@" >/dev/null 2>&1; then pass "$d"; else fail "$d"; fi; }
check_not() { local d="$1"; shift; if "$@" >/dev/null 2>&1; then fail "$d"; else pass "$d"; fi; }
expect_fail() {
  local desc="$1" pattern="$2"; shift 2
  local out="$SANDBOX/expect.out" err="$SANDBOX/expect.err"
  if "$@" >"$out" 2>"$err"; then fail "$desc（意外成功）"
  elif grep -Eq "$pattern" "$err"; then pass "$desc"
  else fail "$desc（stderr 未含 $pattern：$(<"$err")）"; fi
}
register_fifo() {
  local fifo="$1" result="$2" fd
  mkfifo "$fifo"; exec {fd}<>"$fifo"; FIFO_FDS+=("$fd"); printf -v "$result" '%s' "$fd"
}
field() {
  python3 - "$1" <<'PYFIELD'
from pathlib import Path
import sys
raw=Path(sys.argv[1]).read_bytes(); assert raw.endswith(b'\0') and b'\0' not in raw[:-1]
sys.stdout.write(raw[:-1].decode())
PYFIELD
}
write_field() { printf '%s\0' "$2" > "$1/$3"; }
snapshot() {
  python3 - "$1" <<'PYSNAP'
from pathlib import Path
import hashlib,sys
root=Path(sys.argv[1]); h=hashlib.sha256()
for p in sorted(root.rglob('*')):
    h.update(str(p.relative_to(root)).encode()); h.update(b'D' if p.is_dir() else b'F')
    if p.is_file(): h.update(p.read_bytes())
print(h.hexdigest())
PYSNAP
}
assert_fail_unchanged() {
  local desc="$1" pattern="$2"; shift 2
  local before after out="$SANDBOX/unchanged.out" err="$SANDBOX/unchanged.err"
  before=$(snapshot "$REGROOT")
  if "$@" >"$out" 2>"$err"; then fail "$desc（意外成功）"; return; fi
  after=$(snapshot "$REGROOT")
  if [[ "$before" == "$after" ]] && grep -Eq "$pattern" "$err"; then pass "$desc"
  else fail "$desc（registry 改变或 stderr 不符：$(<"$err")）"; fi
}
owner_token_from() {
  local out="$1" token
  token=$(printf '%s\n' "$out" | perl -ne 'print $1 if /^OWNER_TOKEN=([0-9a-f]{64})$/')
  [[ "$token" =~ ^[0-9a-f]{64}$ ]] || return 1
  printf '%s\n' "$token"
}
acquire() {
  local slot="$1" task="$2" branch="${3:-bugfix/$2}" agent="${4:-agent-$2}" claim="${5:-$SHA}"
  bash scripts/slot_registry.sh acquire --slot "$slot" --task "$task" --branch "$branch" --claim-sha "$claim" --agent "$agent"
}
operation_id_from() {
  local out="$1" operation_id
  operation_id=$(printf '%s\n' "$out" | perl -ne 'print $1 if /^OPERATION_ID=([0-9a-f]{32})$/')
  [[ "$operation_id" =~ ^[0-9a-f]{32}$ ]] || return 1
  printf '%s\n' "$operation_id"
}
resume_unfreeze() {
  bash scripts/slot_registry.sh resume-unfreeze-blocked --slot "$1" --task "$2" --branch "$3" \
    --claim-sha "$SHA" --recovery-agent "$4" --operation-id "$5" --owner-token "$6" \
    --operator tester --reason "${7:-fixture recovery}"
}
mutate() {
  local cmd="$1" slot="$2" task="$3" agent="$4" token="$5"; shift 5
  bash scripts/slot_registry.sh "$cmd" --slot "$slot" --task "$task" --agent "$agent" --owner-token "$token" "$@"
}
new_reservation() {
  local result_var="$1" slot="$2" task="$3" branch="${4:-bugfix/$3}" agent="${5:-agent-$3}" out generated_token
  out=$(acquire "$slot" "$task" "$branch" "$agent"); generated_token=$(owner_token_from "$out") || return 1
  printf -v "$result_var" '%s' "$generated_token"
}
force_unfreeze() {
  bash scripts/slot_registry.sh force-unfreeze-blocked --slot "$1" --task "$2" --branch "$3" \
    --claim-sha "$SHA" --agent "$4" --recovery-agent "${5:-recovery-$4}" --operator tester --reason "fixture recovery"
}
make_remote_branch() {
  local branch="$1" sha="$2"
  git branch -f "$branch" "$sha" >/dev/null
  git update-ref "refs/remotes/origin/$branch" "$sha"
}
create_slot() {
  local slot="$1" branch="$2" sha="$3"
  rm -rf "$ROOT/.agent-worktrees/$slot"
  git worktree add --lock --detach "$ROOT/.agent-worktrees/$slot" "$sha" >/dev/null
  git -C "$ROOT/.agent-worktrees/$slot" switch "$branch" >/dev/null
  git -C "$ROOT/.agent-worktrees/$slot" config "branch.$branch.remote" origin
  git -C "$ROOT/.agent-worktrees/$slot" config "branch.$branch.merge" "refs/heads/$branch"
  [[ "$(git -C "$ROOT/.agent-worktrees/$slot" rev-parse HEAD)" == "$sha" ]]
}
destroy_slot() {
  local slot="$1"
  local path="$ROOT/.agent-worktrees/$slot"
  if [[ -e "$path" ]]; then git worktree unlock "$path" >/dev/null 2>&1 || true; git worktree remove "$path" >/dev/null 2>&1 || true; fi
  git worktree prune >/dev/null
}
reset_registry() {
  local d slot task branch agent state token out recovery_agent operation_id handoff
  for d in "$REGROOT"/slot-*.lock; do
    [[ -d "$d" ]] || continue
    slot=$(basename "$d" .lock); task=$(field "$d/task_id"); branch=$(field "$d/branch")
    agent=$(field "$d/agent_id"); state=$(field "$d/state"); token=$(field "$d/owner_token")
    handoff="$REGROOT/manual-handoff.lock/$slot"
    if [[ -d "$handoff" && ! -L "$handoff" ]]; then
      recovery_agent=$(field "$handoff/recovery_agent")
      token=$(field "$handoff/new_token")
      operation_id=$(field "$handoff/operation_id")
      out=$(resume_unfreeze "$slot" "$task" "$branch" "$recovery_agent" "$operation_id" "$token")
      agent=$recovery_agent; state=$(field "$d/state")
    fi
    case "$state" in
      reserved) mutate rollback "$slot" "$task" "$agent" "$token" >/dev/null ;;
      occupied) mutate release "$slot" "$task" "$agent" "$token" >/dev/null ;;
      blocked_frozen_from_reserved)
        recovery_agent="reset-$agent"; out=$(force_unfreeze "$slot" "$task" "$branch" "$agent" "$recovery_agent")
        token=$(owner_token_from "$out"); operation_id=$(operation_id_from "$out")
        resume_unfreeze "$slot" "$task" "$branch" "$recovery_agent" "$operation_id" "$token" >/dev/null
        mutate rollback "$slot" "$task" "$recovery_agent" "$token" >/dev/null
        ;;
      blocked_frozen_from_occupied)
        recovery_agent="reset-$agent"; out=$(force_unfreeze "$slot" "$task" "$branch" "$agent" "$recovery_agent")
        token=$(owner_token_from "$out"); operation_id=$(operation_id_from "$out")
        resume_unfreeze "$slot" "$task" "$branch" "$recovery_agent" "$operation_id" "$token" >/dev/null
        mutate release "$slot" "$task" "$recovery_agent" "$token" >/dev/null
        ;;
    esac
  done
}

# 动态沙箱仓，测试不访问/探测/signal 仓外进程。
git init -q -b main "$SANDBOX/repo"
cd "$SANDBOX/repo"
git config user.email slot-test@bong.local; git config user.name slot-test
git remote add origin "$SANDBOX/repo"
printf 'base\n' > base.txt; git add base.txt; git commit -qm base
ROOT=$PWD
mkdir -p scripts .agent-worktrees/test-registry .agent-worktrees/test-locks
cp "$REG" scripts/slot_registry.sh; chmod +x scripts/slot_registry.sh
export SLOT_REGISTRY_ROOT_OVERRIDE="$ROOT/.agent-worktrees/test-registry"
export SLOT_REGISTRY_LOCK_ROOT_OVERRIDE="$ROOT/.agent-worktrees/test-locks"
REGROOT=$SLOT_REGISTRY_ROOT_OVERRIDE; LOCKROOT=$SLOT_REGISTRY_LOCK_ROOT_OVERRIDE
SHA=$(git rev-parse HEAD)

printf '== 1. init / help / fixed pool\n'
out=$(bash scripts/slot_registry.sh init --max 4)
check "init capacity=4" grep -q 'capacity=4 held=0' <<<"$out"
check "独立 flock root" test -f "$LOCKROOT/acquire.lock"
check_not "registry 不混入 flock" test -e "$REGROOT/acquire.lock"
help=$(bash scripts/slot_registry.sh --help)
check "help 含唯一进驻说明" grep -q 'occupy 是唯一生产进驻门' <<<"$help"
check "help 明示 manual-report-only" grep -q '不实现 PID/liveness 自动恢复' <<<"$help"
check_not "help 不泄漏脚本正文" grep -q '^set -euo' <<<"$help"
expect_fail "slot-0 拒绝" 'out of pool' acquire slot-0 zero
expect_fail "slot-max+1 拒绝" 'out of pool' acquire slot-5 over
expect_fail "非法 branch 拒绝" 'invalid branch' acquire slot-1 bad 'bad branch'

printf '== 2. acquire token / 默认 status 不泄漏\n'
out=$(acquire slot-1 owner-a bugfix/owner-a agent-a); token_a=$(owner_token_from "$out")
check "acquire 输出 256-bit owner token" test ${#token_a} -eq 64
check "token 持久化" test "$(field "$REGROOT/slot-1.lock/owner_token")" = "$token_a"
status=$(bash scripts/slot_registry.sh status); json=$(bash scripts/slot_registry.sh status --json)
check_not "文本 status 不泄漏 token" grep -q "$token_a" <<<"$status"
check_not "JSON status 不含 owner_token key" grep -q 'owner_token' <<<"$json"
check "JSON schema 可解析" python3 -c 'import json,sys;o=json.load(sys.stdin);assert o["held"]==1 and o["slots"][0]["task_id"]=="owner-a" and "owner_token" not in o["slots"][0]' <<<"$json"
manual_report=$(bash scripts/slot_registry.sh manual-report --slot slot-1)
check_not "manual report 不泄漏 token" grep -q "$token_a" <<<"$manual_report"
mutate rollback slot-1 owner-a agent-a "$token_a" >/dev/null

printf '== 3. 全局完整性与一一关系 fail-closed\n'
new_reservation token_a slot-1 unique-a bugfix/unique-a agent-a
for spec in task_id:unique-a branch:bugfix/unique-a agent_id:agent-a owner_token:"$token_a"; do
  f=${spec%%:*}; v=${spec#*:}
  mkdir "$REGROOT/slot-2.lock"
  for required in task_id branch claim_sha agent_id owner_token state created_local_branch reserved_at; do
    case "$required" in
      task_id) x=unique-b ;; branch) x=bugfix/unique-b ;; claim_sha) x="$SHA" ;; agent_id) x=agent-b ;;
      owner_token) x=$(python3 -c 'import secrets;print(secrets.token_hex(32))') ;; state) x=reserved ;;
      created_local_branch) x=false ;; reserved_at) x=2026-07-20T00:00:00Z ;;
    esac
    write_field "$REGROOT/slot-2.lock" "$x" "$required"
  done
  write_field "$REGROOT/slot-2.lock" "$v" "$f"
  assert_fail_unchanged "已有 duplicate $f 时 acquire 全局拒绝" "duplicate $f" acquire slot-3 next-$f "bugfix/next-$f" "agent-next-$f"
  rm -rf "$REGROOT/slot-2.lock"
done
for mode in missing-field bad-nul bad-state bad-created outside invalid-name symlink orphan-temp; do
  case "$mode" in
    missing-field) mkdir "$REGROOT/slot-2.lock"; write_field "$REGROOT/slot-2.lock" x task_id ;;
    bad-nul) cp -a "$REGROOT/slot-1.lock" "$REGROOT/slot-2.lock"; printf broken > "$REGROOT/slot-2.lock/task_id" ;;
    bad-state) cp -a "$REGROOT/slot-1.lock" "$REGROOT/slot-2.lock"; write_field "$REGROOT/slot-2.lock" impossible state ;;
    bad-created) cp -a "$REGROOT/slot-1.lock" "$REGROOT/slot-2.lock"; write_field "$REGROOT/slot-2.lock" maybe created_local_branch ;;
    outside) cp -a "$REGROOT/slot-1.lock" "$REGROOT/slot-5.lock" ;;
    invalid-name) cp -a "$REGROOT/slot-1.lock" "$REGROOT/bogus.lock" ;;
    symlink) ln -s slot-1.lock "$REGROOT/slot-2.lock" ;;
    orphan-temp) mkdir "$REGROOT/.slot-2.reservation.orphan" ;;
  esac
  assert_fail_unchanged "$mode reservation 使 acquire fail-closed" 'incomplete|corrupt|outside|invalid reservation|not a real|orphan' acquire slot-3 "next-$mode" "bugfix/next-$mode" "agent-next-$mode"
  rm -rf "$REGROOT/slot-2.lock" "$REGROOT/slot-5.lock" "$REGROOT/bogus.lock" "$REGROOT/.slot-2.reservation.orphan"
done
mutate rollback slot-1 unique-a agent-a "$token_a" >/dev/null

printf '== 4. sequential + deterministic concurrent uniqueness\n'
new_reservation token_a slot-1 seq-a bugfix/seq-a agent-seq-a
assert_fail_unchanged "重复 task sequential 拒绝" 'task_id already reserved' acquire slot-2 seq-a bugfix/seq-b agent-seq-b
assert_fail_unchanged "重复 branch sequential 拒绝" 'branch already reserved' acquire slot-2 seq-b bugfix/seq-a agent-seq-b
assert_fail_unchanged "重复 agent sequential 拒绝" 'agent_id already reserved' acquire slot-2 seq-b bugfix/seq-b agent-seq-a
mutate rollback slot-1 seq-a agent-seq-a "$token_a" >/dev/null

HOLD_READY="$SANDBOX/hold.ready"; HOLD_GO="$SANDBOX/hold.go"; WAIT_READY="$SANDBOX/wait.ready"; WAIT_GO="$SANDBOX/wait.go"; WAIT_ACK="$SANDBOX/wait.ack"
register_fifo "$HOLD_READY" HOLD_READY_FD; register_fifo "$HOLD_GO" HOLD_GO_FD; RELEASE_FDS+=("$HOLD_GO_FD")
register_fifo "$WAIT_READY" WAIT_READY_FD; register_fifo "$WAIT_GO" WAIT_GO_FD; RELEASE_FDS+=("$WAIT_GO_FD"); register_fifo "$WAIT_ACK" WAIT_ACK_FD
INSTANCE="barrier-$RANDOM-$BASHPID"
env SLOT_REGISTRY_TEST_INSTANCE="$INSTANCE" SLOT_REGISTRY_TEST_HOLD_GATE_READY="$HOLD_READY" SLOT_REGISTRY_TEST_HOLD_GATE_RELEASE="$HOLD_GO" \
  bash scripts/slot_registry.sh acquire --slot slot-1 --task concurrent --branch bugfix/concurrent --claim-sha "$SHA" --agent agent-concurrent >"$SANDBOX/winner.out" 2>"$SANDBOX/winner.err" &
p1=$!; TEST_PIDS+=("$p1")
IFS=' ' read -r word got <&"$HOLD_READY_FD"; [[ "$word $got" == "ready $INSTANCE" ]] || exit 1
before=$(snapshot "$REGROOT")
env SLOT_REGISTRY_TEST_INSTANCE="$INSTANCE" SLOT_REGISTRY_TEST_WAIT_GATE_READY="$WAIT_READY" SLOT_REGISTRY_TEST_WAIT_GATE_RELEASE="$WAIT_GO" SLOT_REGISTRY_TEST_WAIT_GATE_ACK="$WAIT_ACK" \
  bash scripts/slot_registry.sh acquire --slot slot-2 --task concurrent --branch bugfix/other --claim-sha "$SHA" --agent agent-other >"$SANDBOX/loser.out" 2>"$SANDBOX/loser.err" &
p2=$!; TEST_PIDS+=("$p2")
IFS=' ' read -r word got <&"$WAIT_READY_FD"; [[ "$word $got" == "ready $INSTANCE" ]] || exit 1
printf 'release\n' >&"$WAIT_GO_FD"; IFS=' ' read -r word got <&"$WAIT_ACK_FD"; [[ "$word $got" == "released $INSTANCE" ]] || exit 1
printf 'release\n' >&"$HOLD_GO_FD"
set +e; wait "$p1"; r1=$?; wait "$p2"; r2=$?; set -e
if [[ $r1 -eq 0 && $r2 -ne 0 ]] && grep -q 'task_id already reserved' "$SANDBOX/loser.err"; then pass "deterministic concurrent 重复 task 仅一方成功"; else fail "concurrent 结果 $r1/$r2"; fi
after=$(snapshot "$REGROOT"); check_not "winner 确实发布 reservation" test "$before" = "$after"
token_a=$(owner_token_from "$(<"$SANDBOX/winner.out")"); mutate rollback slot-1 concurrent agent-concurrent "$token_a" >/dev/null

printf '== 5. callback nonzero 后 flock 总释放且 registry 不变\n'
for step in write date mv; do
  before=$(snapshot "$REGROOT")
  expect_fail "$step 注入非零" "injected acquire $step failure" env SLOT_REGISTRY_TEST_FAIL_ACQUIRE_STEP="$step" bash scripts/slot_registry.sh acquire --slot slot-1 --task "fail-$step" --branch "bugfix/fail-$step" --claim-sha "$SHA" --agent "agent-fail-$step"
  check "$step 失败 registry 不变" test "$before" = "$(snapshot "$REGROOT")"
  if find "$REGROOT" -maxdepth 1 -name '.slot-*.reservation.*' -print -quit | grep -q .; then fail "$step 失败遗留 temp"; else pass "$step 失败无 temp"; fi
  out=$(SLOT_REGISTRY_GATE_WAIT_SEC=0.2 acquire slot-1 "after-$step" "bugfix/after-$step" "agent-after-$step")
  token=$(owner_token_from "$out"); mutate rollback slot-1 "after-$step" "agent-after-$step" "$token" >/dev/null
  pass "$step callback 非零后同一 flock 可立即重入"
done

printf '== 6. holder token/generation 防 ABA\n'
new_reservation old_token slot-1 aba bugfix/aba agent-aba
mutate rollback slot-1 aba agent-aba "$old_token" >/dev/null
new_reservation new_token slot-1 aba bugfix/aba agent-aba
check_not "新 reservation token 不复用" test "$old_token" = "$new_token"
for cmd in mark-created-local occupy freeze-blocked release rollback; do
  extra=(); [[ "$cmd" == mark-created-local ]] && extra=(--value true)
  assert_fail_unchanged "旧 token 不能 $cmd 新 reservation" 'reservation owner mismatch' mutate "$cmd" slot-1 aba agent-aba "$old_token" "${extra[@]}"
done
for identity in wrong-task wrong-agent; do
  if [[ "$identity" == wrong-task ]]; then task=other; agent=agent-aba; else task=aba; agent=other; fi
  assert_fail_unchanged "$identity 不能 mutation" 'reservation owner mismatch' mutate freeze-blocked slot-1 "$task" "$agent" "$new_token"
done
mutate rollback slot-1 aba agent-aba "$new_token" >/dev/null
expect_fail "free rollback 不再 task-only 幂等" 'slot not held' mutate rollback slot-1 aba agent-aba "$new_token"
expect_fail "free release 不再 task-only 幂等" 'slot not held' mutate release slot-1 aba agent-aba "$new_token"

printf '== 7. occupy 正常生产进驻\n'
make_remote_branch bugfix/good "$SHA"; create_slot slot-1 bugfix/good "$SHA"
new_reservation token slot-1 good bugfix/good agent-good
mutate mark-created-local slot-1 good agent-good "$token" --value true >/dev/null
mutate occupy slot-1 good agent-good "$token" >/dev/null
check "真实 canonical locked worktree reserved→occupied" test "$(field "$REGROOT/slot-1.lock/state")" = occupied
mutate release slot-1 good agent-good "$token" >/dev/null; destroy_slot slot-1

printf '== 8. occupy 饱和 fail-closed 矩阵\n'
run_occupy_case() {
  local name="$1" setup="$2" pattern="$3" branch="bugfix/case-$1" token before
  make_remote_branch "$branch" "$SHA"; new_reservation token slot-1 "case-$name" "$branch" "agent-case-$name"
  eval "$setup"
  before=$(snapshot "$REGROOT/slot-1.lock")
  expect_fail "occupy $name 拒绝" "$pattern" mutate occupy slot-1 "case-$name" "agent-case-$name" "$token"
  check "occupy $name 保持 reservation 全字段" test "$before" = "$(snapshot "$REGROOT/slot-1.lock")"
  # 恢复测试 fixture 后 rollback（只操作动态沙箱）。
  case "$name" in
    missing) ;;
    unlocked) git worktree lock "$ROOT/.agent-worktrees/slot-1" >/dev/null ;;
    wrong-branch) git -C "$ROOT/.agent-worktrees/slot-1" checkout "$branch" >/dev/null; git -C "$ROOT/.agent-worktrees/slot-1" branch --set-upstream-to="origin/$branch" "$branch" >/dev/null ;;
    wrong-head) ;;
    no-upstream) git -C "$ROOT/.agent-worktrees/slot-1" branch --set-upstream-to="origin/$branch" "$branch" >/dev/null ;;
    dirty) git -C "$ROOT/.agent-worktrees/slot-1" checkout -- base.txt >/dev/null ;;
    untracked) rm -f "$ROOT/.agent-worktrees/slot-1/untracked.txt" ;;
    ignored) rm -f "$ROOT/.agent-worktrees/slot-1/secret.env"; rm -f "$ROOT/.agent-worktrees/slot-1/.git/info/exclude" 2>/dev/null || true ;;
    cache) ;;
  esac
  mutate rollback slot-1 "case-$name" "agent-case-$name" "$token" >/dev/null
  destroy_slot slot-1
}
run_occupy_case missing ':' 'canonical slot worktree missing'
run_occupy_case unlocked 'create_slot slot-1 "$branch" "$SHA"; git worktree unlock "$ROOT/.agent-worktrees/slot-1"' 'unlocked'
run_occupy_case wrong-branch 'make_remote_branch bugfix/wrong-physical "$SHA"; create_slot slot-1 bugfix/wrong-physical "$SHA"' 'branch mismatch'
# Wrong HEAD fixture uses a second commit and matching upstream while reservation claim stays old SHA.
SHA2=$(printf 'next tree\n' | git commit-tree "$(git rev-parse "$SHA^{tree}")" -p "$SHA")
run_occupy_case wrong-head 'make_remote_branch "$branch" "$SHA2"; create_slot slot-1 "$branch" "$SHA2"' 'HEAD mismatch|HEAD does not equal'
run_occupy_case no-upstream 'create_slot slot-1 "$branch" "$SHA"; git -C "$ROOT/.agent-worktrees/slot-1" branch --unset-upstream' 'no upstream'
run_occupy_case dirty 'create_slot slot-1 "$branch" "$SHA"; printf dirty >> "$ROOT/.agent-worktrees/slot-1/base.txt"' 'tracked/untracked changes'
run_occupy_case untracked 'create_slot slot-1 "$branch" "$SHA"; printf x > "$ROOT/.agent-worktrees/slot-1/untracked.txt"' 'tracked/untracked changes'
run_occupy_case ignored 'create_slot slot-1 "$branch" "$SHA"; printf "secret.env\n" >> "$ROOT/.git/info/exclude"; printf x > "$ROOT/.agent-worktrees/slot-1/secret.env"' 'non-whitelisted ignored path'
# Whitelisted cache is accepted.
branch=bugfix/cache; make_remote_branch "$branch" "$SHA"; create_slot slot-1 "$branch" "$SHA"
mkdir -p "$ROOT/.agent-worktrees/slot-1/server/target"; printf x > "$ROOT/.agent-worktrees/slot-1/server/target/cache"
printf 'server/target/\n' >> "$ROOT/.git/info/exclude"
new_reservation token slot-1 cache "$branch" agent-cache
check "occupy 接受窄 cache whitelist" mutate occupy slot-1 cache agent-cache "$token"
mutate release slot-1 cache agent-cache "$token" >/dev/null; destroy_slot slot-1
# restore shared dynamic repository exclude fixture
: > "$ROOT/.git/info/exclude"

printf '== 9. blocked_frozen 来源保持与 durable audited owner handoff\n'
# reserved 冻结必须 handoff 给恢复者并恢复 reserved，不能绕过唯一 occupy 进驻门。
new_reservation token slot-1 frozen-reserved bugfix/frozen-reserved agent-frozen-reserved
old_token=$token
mutate freeze-blocked slot-1 frozen-reserved agent-frozen-reserved "$old_token" >/dev/null
check "reserved 冻结记录来源" test "$(field "$REGROOT/slot-1.lock/state")" = blocked_frozen_from_reserved
for cmd in occupy release rollback mark-created-local freeze-blocked; do
  extra=(); [[ "$cmd" == mark-created-local ]] && extra=(--value true)
  assert_fail_unchanged "frozen 普通 $cmd fail-closed" 'invalid state transition' mutate "$cmd" slot-1 frozen-reserved agent-frozen-reserved "$old_token" "${extra[@]}"
done
report=$(bash scripts/slot_registry.sh manual-report --slot slot-1)
check "manual-report 明示只报告" grep -q 'RECOVERY_MODE=manual-report-only' <<<"$report"
check "manual-report 初始无 pending handoff" grep -q 'pending_handoff=false' <<<"$report"
check "manual-report 不泄漏旧 token" grep -qv "$old_token" <<<"$report"
check "manual-report 不改 frozen" test "$(field "$REGROOT/slot-1.lock/state")" = blocked_frozen_from_reserved
expect_fail "force-unfreeze 缺 operator/reason/recovery-agent 拒绝" 'missing --operator|missing --reason|missing --recovery-agent' bash scripts/slot_registry.sh force-unfreeze-blocked --slot slot-1 --task frozen-reserved --branch bugfix/frozen-reserved --claim-sha "$SHA" --agent agent-frozen-reserved
expect_fail "force-unfreeze task-only/错身份拒绝" 'manual recovery identity mismatch' bash scripts/slot_registry.sh force-unfreeze-blocked --slot slot-1 --task other --branch bugfix/frozen-reserved --claim-sha "$SHA" --agent agent-frozen-reserved --recovery-agent recovery-reserved --operator tester --reason ticket
expect_fail "force-unfreeze recovery-agent 不得复用旧 holder" 'must differ' bash scripts/slot_registry.sh force-unfreeze-blocked --slot slot-1 --task frozen-reserved --branch bugfix/frozen-reserved --claim-sha "$SHA" --agent agent-frozen-reserved --recovery-agent agent-frozen-reserved --operator tester --reason ticket

before_lock=$(snapshot "$REGROOT/slot-1.lock")
expect_fail "private handoff prepare 失败保持 reservation" 'manual handoff persistence failed' env SLOT_REGISTRY_TEST_FAIL_UNFREEZE_STEP=prepare bash scripts/slot_registry.sh force-unfreeze-blocked --slot slot-1 --task frozen-reserved --branch bugfix/frozen-reserved --claim-sha "$SHA" --agent agent-frozen-reserved --recovery-agent recovery-reserved --operator tester --reason injected
check "prepare 失败 reservation byte-identical" test "$before_lock" = "$(snapshot "$REGROOT/slot-1.lock")"
check_not "prepare 失败不留 pending handoff" test -e "$REGROOT/manual-handoff.lock/slot-1"

before_lock=$(snapshot "$REGROOT/slot-1.lock")
expect_fail "审计写入失败保持 frozen" 'manual audit intent persistence failed' env SLOT_REGISTRY_TEST_FAIL_UNFREEZE_STEP=audit bash scripts/slot_registry.sh force-unfreeze-blocked --slot slot-1 --task frozen-reserved --branch bugfix/frozen-reserved --claim-sha "$SHA" --agent agent-frozen-reserved --recovery-agent recovery-reserved --operator tester --reason injected
check "审计失败 reservation byte-identical" test "$before_lock" = "$(snapshot "$REGROOT/slot-1.lock")"
check_not "审计失败撤回 private handoff" test -e "$REGROOT/manual-handoff.lock/slot-1"

# 模拟进程在 private handoff fsync 后、public intent 写入前被 kill：private operation 必须保留，
# 所有普通命令 fail-closed；只有携带完整一次性凭据的 resume 可先补 audit intent 再 mutation。
prepared=$(force_unfreeze slot-1 frozen-reserved bugfix/frozen-reserved agent-frozen-reserved recovery-reserved)
missing_intent_token=$(owner_token_from "$prepared"); missing_intent_operation=$(operation_id_from "$prepared")
python3 - "$REGROOT/manual-recovery.audit.jsonl" "$missing_intent_operation" <<'PYDROPINTENT'
import json, os, pathlib, sys, tempfile
path=pathlib.Path(sys.argv[1]); operation=sys.argv[2]
rows=[json.loads(line) for line in path.read_text(encoding='utf-8').splitlines()]
rows=[row for row in rows if row['operation_id'] != operation]
fd,tmp=tempfile.mkstemp(prefix='.audit.', dir=path.parent)
try:
    with os.fdopen(fd, 'w', encoding='utf-8') as out:
        for row in rows:
            out.write(json.dumps(row, ensure_ascii=False, separators=(',', ':'))+'\n')
        out.flush(); os.fsync(out.fileno())
    os.replace(tmp, path)
    dfd=os.open(path.parent, os.O_RDONLY|os.O_DIRECTORY)
    try: os.fsync(dfd)
    finally: os.close(dfd)
finally:
    if os.path.exists(tmp): os.unlink(tmp)
PYDROPINTENT
assert_fail_unchanged "缺 public intent 时 status fail-closed" 'no durable public intent' bash scripts/slot_registry.sh status
check "缺 intent 时 reservation 保持旧 frozen owner" test "$(field "$REGROOT/slot-1.lock/agent_id")/$(field "$REGROOT/slot-1.lock/owner_token")/$(field "$REGROOT/slot-1.lock/state")" = "agent-frozen-reserved/$old_token/blocked_frozen_from_reserved"
resume_unfreeze slot-1 frozen-reserved bugfix/frozen-reserved recovery-reserved "$missing_intent_operation" "$missing_intent_token" >/dev/null
check "resume 先补 intent 再完成 handoff" python3 - "$REGROOT/manual-recovery.audit.jsonl" "$missing_intent_operation" <<'PYRECOVERINTENT'
import json,sys
rows=[json.loads(line) for line in open(sys.argv[1]) if json.loads(line).get('operation_id') == sys.argv[2]]
assert [row['event'] for row in rows] == ['force-unfreeze-blocked-intent','force-unfreeze-blocked-completed']
PYRECOVERINTENT
mutate rollback slot-1 frozen-reserved recovery-reserved "$missing_intent_token" >/dev/null

new_reservation token slot-1 frozen-reserved bugfix/frozen-reserved agent-frozen-reserved
old_token=$token
mutate freeze-blocked slot-1 frozen-reserved agent-frozen-reserved "$old_token" >/dev/null
before_lock=$(snapshot "$REGROOT/slot-1.lock")
expect_fail "public intent 已 fsync 后中断保留可恢复 transaction" 'post-audit-write interruption' env SLOT_REGISTRY_TEST_FAIL_UNFREEZE_STEP=audit-written bash scripts/slot_registry.sh force-unfreeze-blocked --slot slot-1 --task frozen-reserved --branch bugfix/frozen-reserved --claim-sha "$SHA" --agent agent-frozen-reserved --recovery-agent recovery-reserved --operator tester --reason 'fixture recovery'
check "audit-written 中断 reservation byte-identical" test "$before_lock" = "$(snapshot "$REGROOT/slot-1.lock")"
check "audit-written 中断保留 private handoff" test -d "$REGROOT/manual-handoff.lock/slot-1"
operation_id=$(field "$REGROOT/manual-handoff.lock/slot-1/operation_id")
new_token=$(field "$REGROOT/manual-handoff.lock/slot-1/new_token")
resume_unfreeze slot-1 frozen-reserved bugfix/frozen-reserved recovery-reserved "$operation_id" "$new_token" >/dev/null
check "audit-written 中断 resume 不重复 intent" python3 - "$REGROOT/manual-recovery.audit.jsonl" "$operation_id" <<'PYAUDITWRITTEN'
import json,sys
rows=[json.loads(line) for line in open(sys.argv[1]) if json.loads(line).get('operation_id') == sys.argv[2]]
assert [row['event'] for row in rows] == ['force-unfreeze-blocked-intent','force-unfreeze-blocked-completed']
PYAUDITWRITTEN
mutate rollback slot-1 frozen-reserved recovery-reserved "$new_token" >/dev/null

new_reservation token slot-1 frozen-reserved bugfix/frozen-reserved agent-frozen-reserved
old_token=$token
mutate freeze-blocked slot-1 frozen-reserved agent-frozen-reserved "$old_token" >/dev/null
prepared=$(force_unfreeze slot-1 frozen-reserved bugfix/frozen-reserved agent-frozen-reserved recovery-reserved)
new_token=$(owner_token_from "$prepared"); operation_id=$(operation_id_from "$prepared")
check_not "prepared handoff 轮换 token" test "$old_token" = "$new_token"
check "force 只 prepare 不改 frozen state" test "$(field "$REGROOT/slot-1.lock/state")" = blocked_frozen_from_reserved
check "force 只 prepare 不改旧 agent" test "$(field "$REGROOT/slot-1.lock/agent_id")" = agent-frozen-reserved
check "force 只 prepare 不改旧 token" test "$(field "$REGROOT/slot-1.lock/owner_token")" = "$old_token"
check "private handoff durable 存在" test -d "$REGROOT/manual-handoff.lock/slot-1"
report=$(bash scripts/slot_registry.sh manual-report --slot slot-1)
check "manual-report 暴露 operation id" grep -q "operation_id=$operation_id" <<<"$report"
check "manual-report 暴露恢复身份与状态对" grep -q 'recovery_agent=recovery-reserved.*from_state=blocked_frozen_from_reserved.*target_state=reserved' <<<"$report"
check_not "manual-report 不泄漏旧 token" grep -q "$old_token" <<<"$report"
check_not "manual-report 不泄漏新 token" grep -q "$new_token" <<<"$report"
check_not "public audit 不泄漏旧 token" grep -q "$old_token" "$REGROOT/manual-recovery.audit.jsonl"
check_not "public audit 不泄漏新 token" grep -q "$new_token" "$REGROOT/manual-recovery.audit.jsonl"
check "public audit 仅含新 token digest" python3 - "$REGROOT/manual-recovery.audit.jsonl" "$operation_id" "$new_token" <<'PYDIGEST'
import hashlib,json,sys
rows=[json.loads(x) for x in open(sys.argv[1]) if json.loads(x).get('operation_id') == sys.argv[2]]
assert [r['event'] for r in rows] == ['force-unfreeze-blocked-intent']
assert rows[0]['owner_token_sha256'] == hashlib.sha256(sys.argv[3].encode()).hexdigest()
assert 'owner_token' not in rows[0]
PYDIGEST
assert_fail_unchanged "wrong operation id 拒绝 resume" 'manual handoff resume identity mismatch' resume_unfreeze slot-1 frozen-reserved bugfix/frozen-reserved recovery-reserved 00000000000000000000000000000000 "$new_token"
assert_fail_unchanged "wrong recovery agent 拒绝 resume" 'manual handoff resume identity mismatch' resume_unfreeze slot-1 frozen-reserved bugfix/frozen-reserved wrong-recovery "$operation_id" "$new_token"
assert_fail_unchanged "wrong token 拒绝 resume" 'manual handoff resume identity mismatch' resume_unfreeze slot-1 frozen-reserved bugfix/frozen-reserved recovery-reserved "$operation_id" 0000000000000000000000000000000000000000000000000000000000000000
assert_fail_unchanged "wrong reason 拒绝 resume" 'manual handoff resume identity mismatch' resume_unfreeze slot-1 frozen-reserved bugfix/frozen-reserved recovery-reserved "$operation_id" "$new_token" wrong-reason
assert_fail_unchanged "pending recovery agent 不能 acquire 另一 slot" 'reserved by pending manual handoff' acquire slot-2 unrelated bugfix/unrelated recovery-reserved

expect_fail "agent 写入前中断可留 durable handoff" 'owner handoff agent failure' env SLOT_REGISTRY_TEST_FAIL_UNFREEZE_STEP=agent bash scripts/slot_registry.sh resume-unfreeze-blocked --slot slot-1 --task frozen-reserved --branch bugfix/frozen-reserved --claim-sha "$SHA" --recovery-agent recovery-reserved --operation-id "$operation_id" --owner-token "$new_token" --operator tester --reason 'fixture recovery'
check "agent 中断仍是旧 frozen owner" test "$(field "$REGROOT/slot-1.lock/agent_id")/$(field "$REGROOT/slot-1.lock/owner_token")/$(field "$REGROOT/slot-1.lock/state")" = "agent-frozen-reserved/$old_token/blocked_frozen_from_reserved"
expect_fail "token 写入前中断可续跑" 'owner handoff token failure' env SLOT_REGISTRY_TEST_FAIL_UNFREEZE_STEP=token bash scripts/slot_registry.sh resume-unfreeze-blocked --slot slot-1 --task frozen-reserved --branch bugfix/frozen-reserved --claim-sha "$SHA" --recovery-agent recovery-reserved --operation-id "$operation_id" --owner-token "$new_token" --operator tester --reason 'fixture recovery'
check "token 中断已交 recovery agent 但仍旧 token" test "$(field "$REGROOT/slot-1.lock/agent_id")/$(field "$REGROOT/slot-1.lock/owner_token")/$(field "$REGROOT/slot-1.lock/state")" = "recovery-reserved/$old_token/blocked_frozen_from_reserved"
expect_fail "state 写入前中断可续跑" 'unfreeze state failure' env SLOT_REGISTRY_TEST_FAIL_UNFREEZE_STEP=state bash scripts/slot_registry.sh resume-unfreeze-blocked --slot slot-1 --task frozen-reserved --branch bugfix/frozen-reserved --claim-sha "$SHA" --recovery-agent recovery-reserved --operation-id "$operation_id" --owner-token "$new_token" --operator tester --reason 'fixture recovery'
check "state 中断已轮换新 owner 但仍 frozen" test "$(field "$REGROOT/slot-1.lock/agent_id")/$(field "$REGROOT/slot-1.lock/owner_token")/$(field "$REGROOT/slot-1.lock/state")" = "recovery-reserved/$new_token/blocked_frozen_from_reserved"
expect_fail "completion audit 中断可续跑" 'completion audit persistence failed' env SLOT_REGISTRY_TEST_FAIL_UNFREEZE_STEP=complete bash scripts/slot_registry.sh resume-unfreeze-blocked --slot slot-1 --task frozen-reserved --branch bugfix/frozen-reserved --claim-sha "$SHA" --recovery-agent recovery-reserved --operation-id "$operation_id" --owner-token "$new_token" --operator tester --reason 'fixture recovery'
check "completion 中断已到 target state" test "$(field "$REGROOT/slot-1.lock/state")" = reserved
expect_fail "completion 后 cleanup 中断可幂等续跑" 'cleanup failure' env SLOT_REGISTRY_TEST_FAIL_UNFREEZE_STEP=cleanup bash scripts/slot_registry.sh resume-unfreeze-blocked --slot slot-1 --task frozen-reserved --branch bugfix/frozen-reserved --claim-sha "$SHA" --recovery-agent recovery-reserved --operation-id "$operation_id" --owner-token "$new_token" --operator tester --reason 'fixture recovery'
check "cleanup 中断仍保留 private handoff" test -d "$REGROOT/manual-handoff.lock/slot-1"
resume_unfreeze slot-1 frozen-reserved bugfix/frozen-reserved recovery-reserved "$operation_id" "$new_token" >/dev/null
check_not "最终 resume 删除 private handoff" test -e "$REGROOT/manual-handoff.lock/slot-1"
check "reserved 人工恢复仍为 reserved" test "$(field "$REGROOT/slot-1.lock/state")" = reserved
check "reserved handoff 写入 recovery agent" test "$(field "$REGROOT/slot-1.lock/agent_id")" = recovery-reserved
check "reserved handoff 写入新 token" test "$(field "$REGROOT/slot-1.lock/owner_token")" = "$new_token"
for cmd in mark-created-local occupy freeze-blocked release rollback; do
  extra=(); [[ "$cmd" == mark-created-local ]] && extra=(--value true)
  assert_fail_unchanged "handoff 后旧 holder 不能 $cmd" 'reservation owner mismatch' mutate "$cmd" slot-1 frozen-reserved agent-frozen-reserved "$old_token" "${extra[@]}"
done
check "成功 handoff completion 唯一且同 operation" python3 - "$REGROOT/manual-recovery.audit.jsonl" "$operation_id" <<'PYCOMPLETE'
import json,sys
rows=[json.loads(x) for x in open(sys.argv[1]) if json.loads(x).get('operation_id') == sys.argv[2]]
assert [r['event'] for r in rows] == ['force-unfreeze-blocked-intent','force-unfreeze-blocked-completed']
assert all(r['task_id']=='frozen-reserved' and r['agent_id']=='agent-frozen-reserved' for r in rows)
assert all(r['recovery_agent_id']=='recovery-reserved' for r in rows)
assert all('owner_token' not in r and len(r['owner_token_sha256'])==64 for r in rows)
PYCOMPLETE
status=$(bash scripts/slot_registry.sh status); json=$(bash scripts/slot_registry.sh status --json); report=$(bash scripts/slot_registry.sh manual-report --slot slot-1)
check_not "handoff 后文本 status 不泄漏新 token" grep -q "$new_token" <<<"$status"
check_not "handoff 后 JSON status 不泄漏新 token" grep -q "$new_token" <<<"$json"
check_not "handoff 后 manual-report 不泄漏新 token" grep -q "$new_token" <<<"$report"
check "handoff 完成后 report 无 pending" grep -q 'pending_handoff=false' <<<"$report"
expect_fail "恢复 reserved 后新 holder 仍须走 occupy 门" 'canonical slot worktree missing' mutate occupy slot-1 frozen-reserved recovery-reserved "$new_token"
mutate rollback slot-1 frozen-reserved recovery-reserved "$new_token" >/dev/null

# occupied 冻结 handoff 后恢复 occupied；新 holder 可 release，旧 holder 不可操作。
branch=bugfix/frozen-occupied; make_remote_branch "$branch" "$SHA"; create_slot slot-1 "$branch" "$SHA"
new_reservation token slot-1 frozen-occupied "$branch" agent-frozen-occupied
old_token=$token
mutate occupy slot-1 frozen-occupied agent-frozen-occupied "$old_token" >/dev/null
mutate freeze-blocked slot-1 frozen-occupied agent-frozen-occupied "$old_token" >/dev/null
check "occupied 冻结记录来源" test "$(field "$REGROOT/slot-1.lock/state")" = blocked_frozen_from_occupied
prepared=$(force_unfreeze slot-1 frozen-occupied "$branch" agent-frozen-occupied recovery-occupied)
new_token=$(owner_token_from "$prepared"); operation_id=$(operation_id_from "$prepared")
resume_unfreeze slot-1 frozen-occupied "$branch" recovery-occupied "$operation_id" "$new_token" >/dev/null
check "occupied 人工恢复回 occupied" test "$(field "$REGROOT/slot-1.lock/state")" = occupied
check "occupied handoff 写入 recovery agent" test "$(field "$REGROOT/slot-1.lock/agent_id")" = recovery-occupied
assert_fail_unchanged "occupied handoff 后旧 holder 不能 release" 'reservation owner mismatch' mutate release slot-1 frozen-occupied agent-frozen-occupied "$old_token"
check "occupied 人工恢复持久化 audited identity" python3 - "$REGROOT/manual-recovery.audit.jsonl" "$operation_id" <<'PYA'
import json,sys
rows=[json.loads(x) for x in open(sys.argv[1]) if json.loads(x).get('operation_id') == sys.argv[2]]
assert [r['event'] for r in rows] == ['force-unfreeze-blocked-intent','force-unfreeze-blocked-completed']
assert all(r['task_id']=='frozen-occupied' and r['agent_id']=='agent-frozen-occupied' for r in rows)
assert all(r['recovery_agent_id']=='recovery-occupied' and r['operator']=='tester' for r in rows)
assert all(r['reason']=='fixture recovery' and r['from_state']=='blocked_frozen_from_occupied' and r['target_state']=='occupied' for r in rows)
PYA
mutate release slot-1 frozen-occupied recovery-occupied "$new_token" >/dev/null; destroy_slot slot-1

printf '== 9b. private handoff / public audit corruption 全局 fail-closed\n'
for mode in root-symlink entry-symlink orphan missing-field bad-nul bad-pair audit-symlink audit-bad-json audit-bad-fields audit-bad-digest audit-conflict audit-completion-only; do
  reset_registry
  new_reservation token slot-1 "handoff-$mode" "bugfix/handoff-$mode" "agent-handoff-$mode"
  mutate freeze-blocked slot-1 "handoff-$mode" "agent-handoff-$mode" "$token" >/dev/null
  case "$mode" in
    root-symlink)
      mkdir "$REGROOT/handoff-target"; ln -s handoff-target "$REGROOT/manual-handoff.lock" ;;
    entry-symlink)
      mkdir "$REGROOT/manual-handoff.lock"; ln -s ../slot-1.lock "$REGROOT/manual-handoff.lock/slot-1" ;;
    orphan)
      prepared=$(force_unfreeze slot-1 "handoff-$mode" "bugfix/handoff-$mode" "agent-handoff-$mode" "recovery-$mode")
      mv "$REGROOT/slot-1.lock" "$REGROOT/slot-1.saved" ;;
    missing-field)
      prepared=$(force_unfreeze slot-1 "handoff-$mode" "bugfix/handoff-$mode" "agent-handoff-$mode" "recovery-$mode")
      rm "$REGROOT/manual-handoff.lock/slot-1/reason" ;;
    bad-nul)
      prepared=$(force_unfreeze slot-1 "handoff-$mode" "bugfix/handoff-$mode" "agent-handoff-$mode" "recovery-$mode")
      printf broken > "$REGROOT/manual-handoff.lock/slot-1/operation_id" ;;
    bad-pair)
      prepared=$(force_unfreeze slot-1 "handoff-$mode" "bugfix/handoff-$mode" "agent-handoff-$mode" "recovery-$mode")
      write_field "$REGROOT/manual-handoff.lock/slot-1" occupied target_state ;;
    audit-symlink)
      mv "$REGROOT/manual-recovery.audit.jsonl" "$REGROOT/audit-saved"; ln -s audit-saved "$REGROOT/manual-recovery.audit.jsonl" ;;
    audit-bad-json)
      printf '{broken\n' > "$REGROOT/manual-recovery.audit.jsonl" ;;
    audit-bad-fields)
      printf '{}\n' > "$REGROOT/manual-recovery.audit.jsonl" ;;
    audit-bad-digest)
      prepared=$(force_unfreeze slot-1 "handoff-$mode" "bugfix/handoff-$mode" "agent-handoff-$mode" "recovery-$mode")
      python3 - "$REGROOT/manual-recovery.audit.jsonl" <<'PYCORRUPTDIGEST'
import json,pathlib,sys
path=pathlib.Path(sys.argv[1]); rows=[json.loads(x) for x in path.read_text().splitlines()]
rows[-1]['owner_token_sha256']='bad'
path.write_text('\n'.join(json.dumps(x,separators=(',',':')) for x in rows)+'\n')
PYCORRUPTDIGEST
      ;;
    audit-conflict)
      prepared=$(force_unfreeze slot-1 "handoff-$mode" "bugfix/handoff-$mode" "agent-handoff-$mode" "recovery-$mode")
      python3 - "$REGROOT/manual-recovery.audit.jsonl" <<'PYCONFLICTAUDIT'
import json,pathlib,sys
path=pathlib.Path(sys.argv[1]); rows=[json.loads(x) for x in path.read_text().splitlines()]
conflict=dict(rows[-1]); conflict['event']='force-unfreeze-blocked-completed'; conflict['reason']='different'
with path.open('a') as out: out.write(json.dumps(conflict,separators=(',',':'))+'\n')
PYCONFLICTAUDIT
      ;;
    audit-completion-only)
      prepared=$(force_unfreeze slot-1 "handoff-$mode" "bugfix/handoff-$mode" "agent-handoff-$mode" "recovery-$mode")
      python3 - "$REGROOT/manual-recovery.audit.jsonl" <<'PYCOMPLETIONONLY'
import json,pathlib,sys
path=pathlib.Path(sys.argv[1]); rows=[json.loads(x) for x in path.read_text().splitlines()]
rows[-1]['event']='force-unfreeze-blocked-completed'
path.write_text('\n'.join(json.dumps(x,separators=(',',':')) for x in rows)+'\n')
PYCOMPLETIONONLY
      ;;
  esac
  assert_fail_unchanged "$mode handoff/audit 使 status fail-closed" 'manual handoff|manual audit|invalid reservation|corrupt field|invalid manual audit|conflicting manual audit' bash scripts/slot_registry.sh status
  case "$mode" in
    root-symlink) rm "$REGROOT/manual-handoff.lock"; rm -rf "$REGROOT/handoff-target" ;;
    entry-symlink) rm "$REGROOT/manual-handoff.lock/slot-1"; rmdir "$REGROOT/manual-handoff.lock" ;;
    orphan) mv "$REGROOT/slot-1.saved" "$REGROOT/slot-1.lock" ;;
    missing-field) write_field "$REGROOT/manual-handoff.lock/slot-1" 'fixture recovery' reason ;;
    bad-nul) write_field "$REGROOT/manual-handoff.lock/slot-1" "$(operation_id_from "$prepared")" operation_id ;;
    bad-pair) write_field "$REGROOT/manual-handoff.lock/slot-1" reserved target_state ;;
    audit-symlink) rm "$REGROOT/manual-recovery.audit.jsonl" "$REGROOT/audit-saved" ;;
    audit-bad-json|audit-bad-fields) rm "$REGROOT/manual-recovery.audit.jsonl" ;;
    audit-bad-digest|audit-conflict|audit-completion-only)
      operation_id=$(field "$REGROOT/manual-handoff.lock/slot-1/operation_id")
      new_token=$(field "$REGROOT/manual-handoff.lock/slot-1/new_token")
      rm "$REGROOT/manual-recovery.audit.jsonl"
      resume_unfreeze slot-1 "handoff-$mode" "bugfix/handoff-$mode" "recovery-$mode" "$operation_id" "$new_token" >/dev/null
      ;;
  esac
  reset_registry
done
printf '== 10. capacity 严格 schema / NUL/Unicode exact identity / init fail-closed\n'
capacity_before=$(<"$REGROOT/capacity")
for spec in 'empty:' 'space:2 0' 'multi:2\n0\n' 'leading-zero:02\n' 'nondigit:2x\n' 'too-large:9223372036854775808\n'; do
  name=${spec%%:*}; payload=${spec#*:}
  printf '%b' "$payload" > "$REGROOT/capacity"
  assert_fail_unchanged "capacity $name 损坏 fail-closed" 'invalid capacity file' bash scripts/slot_registry.sh status
  printf '%s\n' "$capacity_before" > "$REGROOT/capacity"
done

capacity_snapshot=$(python3 - "$REGROOT/capacity" <<'PYCAPSNAP'
from pathlib import Path
import base64, sys
print(base64.b64encode(Path(sys.argv[1]).read_bytes()).decode("ascii"))
PYCAPSNAP
)
expect_fail "init 拒绝超 signed-64 容量" 'invalid --max 9223372036854775808' \
  bash scripts/slot_registry.sh init --max 9223372036854775808
check "非法 init 不毒化既有 capacity" python3 - "$REGROOT/capacity" "$capacity_snapshot" <<'PYCAPUNCHANGED'
from pathlib import Path
import base64, sys
assert Path(sys.argv[1]).read_bytes() == base64.b64decode(sys.argv[2])
PYCAPUNCHANGED

missing_capacity_root="$SANDBOX/registry-init-too-large"
missing_lock_root="$SANDBOX/flock-init-too-large"
expect_fail "首次 init 也拒绝超 signed-64 容量" 'invalid --max 9223372036854775808' \
  env SLOT_REGISTRY_ROOT_OVERRIDE="$missing_capacity_root" \
      SLOT_REGISTRY_LOCK_ROOT_OVERRIDE="$missing_lock_root" \
      bash scripts/slot_registry.sh init --max 9223372036854775808
check_not "非法首次 init 不创建 capacity" test -e "$missing_capacity_root/capacity"

task=$'任务\n尾随\n'; agent=$'代理\n尾随\n'; branch='bugfix/unicode'
out=$(acquire slot-1 "$task" "$branch" "$agent"); token=$(owner_token_from "$out")
check "NUL 字段逐字节 round-trip" python3 - "$REGROOT/slot-1.lock/task_id" "$task" <<'PYF'
from pathlib import Path
import os,sys
assert Path(sys.argv[1]).read_bytes()==os.fsencode(sys.argv[2])+b'\0'
PYF
mutate mark-created-local slot-1 "$task" "$agent" "$token" --value true >/dev/null
wrong=${task%$'\n'}
assert_fail_unchanged "尾随 LF identity 不可近似匹配" 'reservation owner mismatch' mutate rollback slot-1 "$wrong" "$agent" "$token"
mutate rollback slot-1 "$task" "$agent" "$token" >/dev/null
new_reservation token slot-4 high bugfix/high agent-high
expect_fail "init 不得缩过 held slot" 'cannot shrink capacity below held slot' bash scripts/slot_registry.sh init --max 3
mutate rollback slot-4 high agent-high "$token" >/dev/null
bash scripts/slot_registry.sh init --max 2 >/dev/null
check "最终 max=2 held=0" grep -q 'max=2 held=0' <<<"$(bash scripts/slot_registry.sh capacity)"

printf '%s\n' '---'
printf 'PASS=%s FAIL=%s\n' "$PASS" "$FAIL"
[[ $FAIL -eq 0 ]] || exit 1
printf 'slot_registry 契约测试全部通过\n'
