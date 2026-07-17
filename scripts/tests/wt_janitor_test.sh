#!/usr/bin/env bash
# wt-janitor.sh 契约测试：在隔离沙箱仓库里锁死全部破坏性判定。
# 断言口径是「外部可观察行为」（树/分支/文件是否还在），不绑内部实现。
#
# 用法：bash scripts/tests/wt_janitor_test.sh
set -euo pipefail

JANITOR=$(realpath "$(dirname "$0")/../wt-janitor.sh")
SANDBOX=$(mktemp -d /tmp/wt-janitor-test.XXXXXX)
SLEEPER_PID=""
cleanup() {
  [[ -n "$SLEEPER_PID" ]] && kill "$SLEEPER_PID" 2>/dev/null || true
  rm -rf "$SANDBOX"
}
trap cleanup EXIT

PASS=0
FAIL=0
check() { # check <描述> <命令...>：命令为真 = PASS
  local desc="$1"; shift
  if "$@" >/dev/null 2>&1; then
    echo "  PASS: $desc"; PASS=$((PASS + 1))
  else
    echo "  FAIL: $desc"; FAIL=$((FAIL + 1))
  fi
}

# ---- fake gh：按分支名前缀返回 canned PR 状态（merged-* → MERGED，open-* → OPEN，其余 → 无 PR）
mkdir -p "$SANDBOX/bin"
cat > "$SANDBOX/bin/gh" <<'FAKEGH'
#!/usr/bin/env bash
branch=""
prev=""
for a in "$@"; do
  [[ "$prev" == "--head" ]] && branch="$a"
  prev="$a"
done
case "$branch" in
  merged-*) echo "MERGED" ;;
  open-*)   echo "OPEN" ;;
  *)        echo "" ;;
esac
FAKEGH
chmod +x "$SANDBOX/bin/gh"
export PATH="$SANDBOX/bin:$PATH"

# ---- 沙箱主仓 + bare 远端
git init -q --bare "$SANDBOX/origin.git"
MAIN="$SANDBOX/repo"
git init -q -b main "$MAIN"
cd "$MAIN"
git config user.email janitor-test@bong.local
git config user.name janitor-test
echo base > base.txt
# 与真实仓库一致：构建产物是 ignored 的（否则假产物会把树判脏，测不到产物清理路径）
printf 'server/target/\nclient/build/\nclient/.gradle/\n' > .gitignore
git add -A && git commit -qm "沙箱基线"
git remote add origin "$SANDBOX/origin.git"
git push -q origin main
mkdir -p .agent-worktrees

# new_wt <名字> <分支>：建 worktree、推远端、fetch 远端跟踪 ref
new_wt() {
  local name="$1" br="$2"
  git worktree add -q -b "$br" ".agent-worktrees/$name" main
  git -C ".agent-worktrees/$name" push -q origin "$br"
  git fetch -q origin
}

new_wt wt-merged-clean  merged-clean
new_wt wt-merged-ahead  merged-ahead
new_wt wt-merged-dirty  merged-dirty
new_wt wt-open-idle     open-idle
new_wt wt-nopr-idle     nopr-idle
new_wt slot-1           merged-slotted   # 名字命中 slot-*：即使 PR 已 merge 也必须跳过

# merged-ahead：工作区干净但有远端不可达提交（模拟 squash-merge 后本地追加）
echo ahead > ".agent-worktrees/wt-merged-ahead/ahead.txt"
git -C ".agent-worktrees/wt-merged-ahead" add -A
git -C ".agent-worktrees/wt-merged-ahead" commit -qm "本地未推提交"

# merged-dirty：未提交改动 + 假构建产物（脏树的产物也不许清）
echo dirty > ".agent-worktrees/wt-merged-dirty/base.txt"
mkdir -p ".agent-worktrees/wt-merged-dirty/server/target"
echo blob > ".agent-worktrees/wt-merged-dirty/server/target/blob"

# open-idle / nopr-idle：干净 + 假构建产物
mkdir -p ".agent-worktrees/wt-open-idle/server/target" ".agent-worktrees/wt-nopr-idle/client/build"
echo blob > ".agent-worktrees/wt-open-idle/server/target/blob"
echo blob > ".agent-worktrees/wt-nopr-idle/client/build/blob"

# merged-clean 也放构建产物：验证「先清缓存再普通 remove」路径
mkdir -p ".agent-worktrees/wt-merged-clean/server/target"
echo blob > ".agent-worktrees/wt-merged-clean/server/target/blob"

echo "== 1. report-only：任何树、分支、产物都不许动"
out=$(bash "$JANITOR")
check "wt-merged-clean 树仍在"        test -d ".agent-worktrees/wt-merged-clean"
check "merged-clean 分支仍在"          git show-ref -q "refs/heads/merged-clean"
check "open-idle 产物仍在"             test -f ".agent-worktrees/wt-open-idle/server/target/blob"
check "报告标出可回收"                 grep -q "可回收（PR 已 MERGED）" <<<"$out"
check "报告标出远端不可达提交"         grep -q "远端不可达提交 → 交人工" <<<"$out"
check "报告标出脏树交人工"             grep -q "PR 已 MERGED 但工作区不干净 → 交人工" <<<"$out"
check "slot-* 被跳过"                  grep -q "SLOT（常驻保温，跳过" <<<"$out"

echo "== 2. --clean-artifacts 不带 --apply：只报告待清，不删"
out=$(bash "$JANITOR" --clean-artifacts=0)
check "open-idle 产物仍在"             test -f ".agent-worktrees/wt-open-idle/server/target/blob"
check "报告标出待清"                   grep -q "构建产物待清" <<<"$out"

echo "== 3. BUSY：cwd 在树内的进程必须挡住回收"
( cd ".agent-worktrees/wt-merged-clean" && exec sleep 300 ) &
SLEEPER_PID=$!
sleep 0.2
out=$(bash "$JANITOR" --apply)
check "BUSY 树未被回收"                test -d ".agent-worktrees/wt-merged-clean"
check "报告标出 BUSY"                  grep -q "BUSY（有进程引用，跳过）" <<<"$out"
kill "$SLEEPER_PID" 2>/dev/null || true
wait "$SLEEPER_PID" 2>/dev/null || true
SLEEPER_PID=""

echo "== 4. --apply：只回收 MERGED+干净+已推送；ahead/dirty/slot 一律保留"
bash "$JANITOR" --apply >/dev/null
check "wt-merged-clean 已回收"         test ! -d ".agent-worktrees/wt-merged-clean"
check "merged-clean 分支已删"          bash -c '! git show-ref -q refs/heads/merged-clean'
check "wt-merged-ahead 保留（未推送提交）" test -d ".agent-worktrees/wt-merged-ahead"
check "merged-ahead 分支保留"          git show-ref -q "refs/heads/merged-ahead"
check "wt-merged-dirty 保留（脏树）"   test -d ".agent-worktrees/wt-merged-dirty"
check "wt-open-idle 保留（PR OPEN）"   test -d ".agent-worktrees/wt-open-idle"
check "wt-nopr-idle 保留（无 PR）"     test -d ".agent-worktrees/wt-nopr-idle"
check "slot-1 保留（slot 豁免）"       test -d ".agent-worktrees/slot-1"

echo "== 5. --apply --clean-artifacts=0：只清 干净+OPEN/无PR 的产物；脏树产物不碰"
bash "$JANITOR" --apply --clean-artifacts=0 >/dev/null
check "open-idle 产物已清"             test ! -e ".agent-worktrees/wt-open-idle/server/target"
check "nopr-idle 产物已清"             test ! -e ".agent-worktrees/wt-nopr-idle/client/build"
check "脏树产物未动"                   test -f ".agent-worktrees/wt-merged-dirty/server/target/blob"
check "open-idle 树本体仍在"           test -d ".agent-worktrees/wt-open-idle"

echo "---"
echo "PASS=$PASS FAIL=$FAIL"
[[ $FAIL -eq 0 ]] || exit 1
echo "wt-janitor 契约测试全部通过"
