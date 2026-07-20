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
    --claim-sha "$SHA" --agent "$4" --operator tester --reason "fixture recovery"
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
  local d slot task branch agent state token
  for d in "$REGROOT"/slot-*.lock; do
    [[ -d "$d" ]] || continue
    slot=$(basename "$d" .lock); task=$(field "$d/task_id"); branch=$(field "$d/branch")
    agent=$(field "$d/agent_id"); state=$(field "$d/state"); token=$(field "$d/owner_token")
    case "$state" in
      reserved) mutate rollback "$slot" "$task" "$agent" "$token" >/dev/null ;;
      occupied) mutate release "$slot" "$task" "$agent" "$token" >/dev/null ;;
      blocked_frozen_from_reserved) force_unfreeze "$slot" "$task" "$branch" "$agent" >/dev/null; mutate rollback "$slot" "$task" "$agent" "$token" >/dev/null ;;
      blocked_frozen_from_occupied) force_unfreeze "$slot" "$task" "$branch" "$agent" >/dev/null; mutate release "$slot" "$task" "$agent" "$token" >/dev/null ;;
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

printf '== 9. blocked_frozen 来源保持与 write-ahead 人工审计恢复\n'
# reserved 冻结必须恢复 reserved，不能绕过唯一 occupy 进驻门。
new_reservation token slot-1 frozen-reserved bugfix/frozen-reserved agent-frozen-reserved
mutate freeze-blocked slot-1 frozen-reserved agent-frozen-reserved "$token" >/dev/null
check "reserved 冻结记录来源" test "$(field "$REGROOT/slot-1.lock/state")" = blocked_frozen_from_reserved
for cmd in occupy release rollback mark-created-local freeze-blocked; do
  extra=(); [[ "$cmd" == mark-created-local ]] && extra=(--value true)
  assert_fail_unchanged "frozen 普通 $cmd fail-closed" 'invalid state transition' mutate "$cmd" slot-1 frozen-reserved agent-frozen-reserved "$token" "${extra[@]}"
done
report=$(bash scripts/slot_registry.sh manual-report --slot slot-1)
check "manual-report 明示只报告" grep -q 'RECOVERY_MODE=manual-report-only' <<<"$report"
check "manual-report 初始无未完成 intent" grep -q 'pending_unfreeze_intents=0' <<<"$report"
check "manual-report 不改 frozen" test "$(field "$REGROOT/slot-1.lock/state")" = blocked_frozen_from_reserved
expect_fail "force-unfreeze 缺 operator/reason 拒绝" 'missing --operator|missing --reason' bash scripts/slot_registry.sh force-unfreeze-blocked --slot slot-1 --task frozen-reserved --branch bugfix/frozen-reserved --claim-sha "$SHA" --agent agent-frozen-reserved
expect_fail "force-unfreeze task-only/错身份拒绝" 'manual recovery identity mismatch' bash scripts/slot_registry.sh force-unfreeze-blocked --slot slot-1 --task other --branch bugfix/frozen-reserved --claim-sha "$SHA" --agent agent-frozen-reserved --operator tester --reason ticket
assert_fail_unchanged "审计写入失败保持 frozen 且零记录" 'manual audit intent persistence failed' env SLOT_REGISTRY_TEST_FAIL_UNFREEZE_STEP=audit bash scripts/slot_registry.sh force-unfreeze-blocked --slot slot-1 --task frozen-reserved --branch bugfix/frozen-reserved --claim-sha "$SHA" --agent agent-frozen-reserved --operator tester --reason injected
expect_fail "审计后 state 写入失败 fail-closed" 'durable intent requires manual inspection' env SLOT_REGISTRY_TEST_FAIL_UNFREEZE_STEP=state bash scripts/slot_registry.sh force-unfreeze-blocked --slot slot-1 --task frozen-reserved --branch bugfix/frozen-reserved --claim-sha "$SHA" --agent agent-frozen-reserved --operator tester --reason injected
check "state 失败保留 frozen" test "$(field "$REGROOT/slot-1.lock/state")" = blocked_frozen_from_reserved
check "state 失败已有 durable intent" python3 - "$REGROOT/manual-recovery.audit.jsonl" <<'PYAFAIL'
import json,sys
rows=[json.loads(x) for x in open(sys.argv[1])]
r=rows[-1]
assert r['event']=='force-unfreeze-blocked-intent' and r['task_id']=='frozen-reserved'
assert r['from_state']=='blocked_frozen_from_reserved' and r['target_state']=='reserved'
PYAFAIL
report=$(bash scripts/slot_registry.sh manual-report --slot slot-1)
check "manual-report 暴露未完成 intent" grep -Eq 'pending_unfreeze_intents=[1-9][0-9]*' <<<"$report"
force_unfreeze slot-1 frozen-reserved bugfix/frozen-reserved agent-frozen-reserved >/dev/null
check "reserved 人工恢复仍为 reserved" test "$(field "$REGROOT/slot-1.lock/state")" = reserved
expect_fail "恢复 reserved 后仍须走 occupy 门" 'canonical slot worktree missing' mutate occupy slot-1 frozen-reserved agent-frozen-reserved "$token"
mutate rollback slot-1 frozen-reserved agent-frozen-reserved "$token" >/dev/null

# occupied 冻结恢复 occupied；审计记录不含 owner token。
branch=bugfix/frozen-occupied; make_remote_branch "$branch" "$SHA"; create_slot slot-1 "$branch" "$SHA"
new_reservation token slot-1 frozen-occupied "$branch" agent-frozen-occupied
mutate occupy slot-1 frozen-occupied agent-frozen-occupied "$token" >/dev/null
mutate freeze-blocked slot-1 frozen-occupied agent-frozen-occupied "$token" >/dev/null
check "occupied 冻结记录来源" test "$(field "$REGROOT/slot-1.lock/state")" = blocked_frozen_from_occupied
force_unfreeze slot-1 frozen-occupied "$branch" agent-frozen-occupied >/dev/null
check "occupied 人工恢复回 occupied" test "$(field "$REGROOT/slot-1.lock/state")" = occupied
check "人工恢复持久化 audited identity" python3 - "$REGROOT/manual-recovery.audit.jsonl" <<'PYA'
import json,sys
rows=[json.loads(x) for x in open(sys.argv[1])]
r=rows[-1]
assert r['event']=='force-unfreeze-blocked-intent' and r['task_id']=='frozen-occupied'
assert r['operator']=='tester' and r['reason']=='fixture recovery'
assert r['from_state']=='blocked_frozen_from_occupied' and r['target_state']=='occupied'
assert len(r['operation_id'])==32 and 'owner_token' not in r
PYA
mutate release slot-1 frozen-occupied agent-frozen-occupied "$token" >/dev/null; destroy_slot slot-1

printf '== 10. capacity 严格 schema / NUL/Unicode exact identity / init fail-closed\n'
capacity_before=$(<"$REGROOT/capacity")
for spec in 'empty:' 'space:2 0' 'multi:2\n0\n' 'leading-zero:02\n' 'nondigit:2x\n' 'too-large:9223372036854775808\n'; do
  name=${spec%%:*}; payload=${spec#*:}
  printf '%b' "$payload" > "$REGROOT/capacity"
  assert_fail_unchanged "capacity $name 损坏 fail-closed" 'invalid capacity file' bash scripts/slot_registry.sh status
  printf '%s\n' "$capacity_before" > "$REGROOT/capacity"
done

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
