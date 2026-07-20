#!/usr/bin/env bash
# slot_registry.sh 契约测试：锁死原子占用 / 容量 / 失败回滚 created_local_branch。
# 断言口径 = 外部可观察状态（registry 目录/字段/exit code/stdout），不绑内部实现细节。
set -euo pipefail

REG=$(realpath "$(dirname "$0")/../slot_registry.sh")
SANDBOX=$(mktemp -d /tmp/slot-registry-test.XXXXXX)
cleanup() { rm -rf "$SANDBOX"; }
trap cleanup EXIT

PASS=0
FAIL=0
check() {
  local desc="$1"; shift
  if "$@" >/dev/null 2>&1; then
    echo "  PASS: $desc"; PASS=$((PASS + 1))
  else
    echo "  FAIL: $desc"; FAIL=$((FAIL + 1))
  fi
}
check_not() {
  local desc="$1"; shift
  if "$@" >/dev/null 2>&1; then
    echo "  FAIL: $desc"; FAIL=$((FAIL + 1))
  else
    echo "  PASS: $desc"; PASS=$((PASS + 1))
  fi
}

# 隔离沙箱仓
git init -q -b main "$SANDBOX/repo"
cd "$SANDBOX/repo"
git config user.email slot-test@bong.local
git config user.name slot-test
echo base > base.txt
git add -A && git commit -qm base
mkdir -p scripts .agent-worktrees
cp "$REG" scripts/slot_registry.sh
chmod +x scripts/slot_registry.sh
SHA=$(git rev-parse HEAD)
SHA2=$(printf '%s' "$SHA" | tr '0-9a-f' 'a-f0-9' | head -c 40)
# ensure SHA2 still 40 hex and different if possible
if [[ "$SHA2" == "$SHA" ]]; then
  SHA2="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
fi

echo "== 1. init / capacity 默认与显式"
out=$(bash scripts/slot_registry.sh init)
check "init 默认 capacity=2" grep -q 'capacity=2' <<<"$out"
out=$(bash scripts/slot_registry.sh capacity)
check "capacity 报告 max=2 held=0" grep -q 'max=2 held=0' <<<"$out"
out=$(bash scripts/slot_registry.sh init --max 3)
check "init --max 3" grep -q 'capacity=3' <<<"$out"
printf '2\n' > .agent-worktrees/.slot-registry/capacity

echo "== 2. acquire 原子成功路径 + 字段"
out=$(bash scripts/slot_registry.sh acquire \
  --slot slot-1 --task task-a --branch bugfix/plan-a \
  --claim-sha "$SHA" --agent agent-a)
check "acquire 成功" grep -q 'OK acquire slot-1' <<<"$out"
check "lock 目录存在" test -d .agent-worktrees/.slot-registry/slot-1.lock
check "task_id 字段" bash -c '[[ "$(cat .agent-worktrees/.slot-registry/slot-1.lock/task_id)" == "task-a" ]]'
check "state=reserved" bash -c '[[ "$(cat .agent-worktrees/.slot-registry/slot-1.lock/state)" == "reserved" ]]'
check "created_local 默认 false" bash -c '[[ "$(cat .agent-worktrees/.slot-registry/slot-1.lock/created_local_branch)" == "false" ]]'
check "claim_sha 小写 40hex" bash -c '[[ "$(cat .agent-worktrees/.slot-registry/slot-1.lock/claim_sha)" == "$(printf %s "'"$SHA"'" | tr A-F a-f)" ]]'
check "is-held 返回 0" bash scripts/slot_registry.sh is-held --slot slot-1

echo "== 3. 并发竞争同一 slot：第二个必须失败，不破坏第一个"
if bash scripts/slot_registry.sh acquire \
  --slot slot-1 --task task-b --branch bugfix/plan-b \
  --claim-sha "$SHA" --agent agent-b >/tmp/slot-race.out 2>/tmp/slot-race.err; then
  echo "  FAIL: 并发第二 acquire 应失败"; FAIL=$((FAIL + 1))
else
  echo "  PASS: 并发第二 acquire 失败"; PASS=$((PASS + 1))
fi
check "竞争后仍是 task-a" bash -c '[[ "$(cat .agent-worktrees/.slot-registry/slot-1.lock/task_id)" == "task-a" ]]'
check "stderr 含 busy" grep -q 'slot busy' /tmp/slot-race.err

echo "== 4. 容量上限：held==max 时拒绝新 slot"
bash scripts/slot_registry.sh acquire \
  --slot slot-2 --task task-b --branch bugfix/plan-b \
  --claim-sha "$SHA" --agent agent-b >/dev/null
if bash scripts/slot_registry.sh acquire \
  --slot slot-3 --task task-c --branch bugfix/plan-c \
  --claim-sha "$SHA" --agent agent-c >/tmp/slot-cap.out 2>/tmp/slot-cap.err; then
  echo "  FAIL: capacity full 应拒绝"; FAIL=$((FAIL + 1))
else
  echo "  PASS: capacity full 拒绝"; PASS=$((PASS + 1))
fi
check "stderr 含 capacity full" grep -q 'capacity full' /tmp/slot-cap.err
out=$(bash scripts/slot_registry.sh capacity)
check "held=2" grep -q 'held=2' <<<"$out"

echo "== 5. occupy / freeze-blocked 状态转换"
bash scripts/slot_registry.sh occupy --slot slot-1 --task task-a >/dev/null
check "occupied 状态" bash -c '[[ "$(cat .agent-worktrees/.slot-registry/slot-1.lock/state)" == "occupied" ]]'
bash scripts/slot_registry.sh freeze-blocked --slot slot-1 --task task-a >/dev/null
check "blocked_frozen 状态" bash -c '[[ "$(cat .agent-worktrees/.slot-registry/slot-1.lock/state)" == "blocked_frozen" ]]'
# holder mismatch 必须失败
if bash scripts/slot_registry.sh occupy --slot slot-1 --task stranger >/dev/null 2>&1; then
  echo "  FAIL: holder mismatch 应失败"; FAIL=$((FAIL + 1))
else
  echo "  PASS: holder mismatch 失败"; PASS=$((PASS + 1))
fi

echo "== 6. release 幂等 + 释放后可复用"
bash scripts/slot_registry.sh release --slot slot-1 --task task-a >/dev/null
check_not "release 后 lock 消失" test -d .agent-worktrees/.slot-registry/slot-1.lock
bash scripts/slot_registry.sh release --slot slot-1 --task task-a >/dev/null
check "release 幂等" true
# 释放一个后 held 下降，可再 acquire slot-3
bash scripts/slot_registry.sh acquire \
  --slot slot-3 --task task-c --branch bugfix/plan-c \
  --claim-sha "$SHA" --agent agent-c >/dev/null
check "释放后可 acquire 新 slot" test -d .agent-worktrees/.slot-registry/slot-3.lock
bash scripts/slot_registry.sh release --slot slot-2 --task task-b >/dev/null
bash scripts/slot_registry.sh release --slot slot-3 --task task-c >/dev/null

echo "== 7. rollback：既有分支 created_local=false → DELETE_LOCAL_BRANCH=false"
bash scripts/slot_registry.sh acquire \
  --slot slot-1 --task task-x --branch bugfix/plan-x \
  --claim-sha "$SHA" --agent agent-x >/dev/null
# 模拟「本地分支已存在」：保持默认 false，并在仓内放一个既有分支 tip 不同的提交
git branch bugfix/plan-x
echo residual > residual.txt
git add residual.txt && git commit -qm "既有残留提交"
EXIST_SHA=$(git rev-parse HEAD)
git branch -f bugfix/plan-x HEAD >/dev/null
# claim SHA 与既有分支 tip 不同（进驻会对拍失败）
check "既有 tip 不同于 claim" bash -c '[[ "'"$EXIST_SHA"'" != "'"$SHA"'" ]]'
out=$(bash scripts/slot_registry.sh rollback --slot slot-1 --task task-x)
check "rollback 输出 DELETE=false" grep -q 'DELETE_LOCAL_BRANCH=false' <<<"$out"
check_not "rollback 后 lock 已清" test -d .agent-worktrees/.slot-registry/slot-1.lock
# 既有分支与残留提交必须仍在
check "既有分支仍在" git show-ref -q refs/heads/bugfix/plan-x
check "残留 tip 仍在" bash -c '[[ "$(git rev-parse refs/heads/bugfix/plan-x)" == "'"$EXIST_SHA"'" ]]'

echo "== 8. rollback：本轮新建 created_local=true → DELETE_LOCAL_BRANCH=true"
bash scripts/slot_registry.sh acquire \
  --slot slot-1 --task task-new --branch bugfix/plan-new \
  --claim-sha "$SHA" --agent agent-new >/dev/null
bash scripts/slot_registry.sh mark-created-local --slot slot-1 --task task-new --value true >/dev/null
out=$(bash scripts/slot_registry.sh rollback --slot slot-1 --task task-new)
check "新建分支 rollback DELETE=true" grep -q 'DELETE_LOCAL_BRANCH=true' <<<"$out"
check_not "新建 rollback lock 已清" test -d .agent-worktrees/.slot-registry/slot-1.lock
# 既有分支仍不受影响
check "既有分支仍未被删" git show-ref -q refs/heads/bugfix/plan-x

echo "== 9. claim-sha 非法 / slot 名非法 fail-closed"
if bash scripts/slot_registry.sh acquire \
  --slot slot-1 --task t --branch b --claim-sha deadbeef --agent a >/dev/null 2>&1; then
  echo "  FAIL: 短 claim-sha 应失败"; FAIL=$((FAIL + 1))
else
  echo "  PASS: 短 claim-sha 拒绝"; PASS=$((PASS + 1))
fi
if bash scripts/slot_registry.sh acquire \
  --slot not-a-slot --task t --branch b --claim-sha "$SHA" --agent a >/dev/null 2>&1; then
  echo "  FAIL: 非法 slot 名应失败"; FAIL=$((FAIL + 1))
else
  echo "  PASS: 非法 slot 名拒绝"; PASS=$((PASS + 1))
fi
check_not "非法输入后无残留 lock" test -d .agent-worktrees/.slot-registry/slot-1.lock

echo "== 10. status --json 可观察"
bash scripts/slot_registry.sh acquire \
  --slot slot-1 --task task-json --branch bugfix/j \
  --claim-sha "$SHA" --agent agent-j >/dev/null
json=$(bash scripts/slot_registry.sh status --json)
check "json 含 max" grep -q '"max":2' <<<"$json"
check "json 含 task-json" grep -q 'task-json' <<<"$json"
check "json 含 reserved" grep -q 'reserved' <<<"$json"
bash scripts/slot_registry.sh release --slot slot-1 --task task-json >/dev/null

echo "== 11. 并发 acquire 竞态（后台并行）"
# 两个并行进程抢 slot-9；恰好一个成功
rm -rf .agent-worktrees/.slot-registry/slot-9.lock
(
  bash scripts/slot_registry.sh acquire \
    --slot slot-9 --task race-1 --branch bugfix/r1 \
    --claim-sha "$SHA" --agent r1 >"$SANDBOX/race1.out" 2>"$SANDBOX/race1.err" &
  bash scripts/slot_registry.sh acquire \
    --slot slot-9 --task race-2 --branch bugfix/r2 \
    --claim-sha "$SHA" --agent r2 >"$SANDBOX/race2.out" 2>"$SANDBOX/race2.err" &
  wait
)
ok_cnt=0
grep -q 'OK acquire' "$SANDBOX/race1.out" 2>/dev/null && ok_cnt=$((ok_cnt+1))
grep -q 'OK acquire' "$SANDBOX/race2.out" 2>/dev/null && ok_cnt=$((ok_cnt+1))
if [[ $ok_cnt -eq 1 ]]; then
  echo "  PASS: 并行竞争恰好一成功"; PASS=$((PASS + 1))
else
  echo "  FAIL: 并行竞争 ok_cnt=$ok_cnt 期望 1"; FAIL=$((FAIL + 1))
fi
check "并行后 lock 唯一" test -d .agent-worktrees/.slot-registry/slot-9.lock
holder=$(cat .agent-worktrees/.slot-registry/slot-9.lock/task_id)
bash scripts/slot_registry.sh release --slot slot-9 --task "$holder" >/dev/null

echo "---"
echo "PASS=$PASS FAIL=$FAIL"
[[ $FAIL -eq 0 ]] || exit 1
echo "slot_registry 契约测试全部通过"
