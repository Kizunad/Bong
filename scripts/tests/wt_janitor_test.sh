#!/usr/bin/env bash
# wt-janitor.sh 契约测试：在隔离沙箱仓库里锁死全部破坏性判定。
# 断言口径是「外部可观察行为」（树/分支/文件是否还在、报告 verdict 关键字），不绑内部实现。
#
# 用法：bash scripts/tests/wt_janitor_test.sh
set -euo pipefail

JANITOR=$(realpath "$(dirname "$0")/../wt-janitor.sh")
SANDBOX=$(mktemp -d /tmp/wt-janitor-test.XXXXXX)
SLEEPER_PID=""
cleanup() {
  [[ -n "${SLEEPER_PID:-}" ]] && kill "$SLEEPER_PID" 2>/dev/null || true
  # 若劫持了 git，先还原再清沙箱
  rm -f "$SANDBOX/bin/git" 2>/dev/null || true
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
check_not() { # 命令为假 = PASS
  local desc="$1"; shift
  if "$@" >/dev/null 2>&1; then
    echo "  FAIL: $desc"; FAIL=$((FAIL + 1))
  else
    echo "  PASS: $desc"; PASS=$((PASS + 1))
  fi
}

# ---- 可配置 fake gh ----
# FAKE_GH_MODE=fail → 非零退出；默认按分支名前缀返回状态
mkdir -p "$SANDBOX/bin"
cat > "$SANDBOX/bin/gh" <<'FAKEGH'
#!/usr/bin/env bash
if [[ "${FAKE_GH_MODE:-}" == "fail" ]]; then
  echo "simulated gh failure" >&2
  exit 1
fi
branch=""
prev=""
for a in "$@"; do
  [[ "$prev" == "--head" ]] && branch="$a"
  prev="$a"
done
case "$branch" in
  merged-*) echo "MERGED" ;;
  open-*)   echo "OPEN" ;;
  closed-*) echo "CLOSED" ;;
  *)        echo "" ;;
esac
FAKEGH
chmod +x "$SANDBOX/bin/gh"
export PATH="$SANDBOX/bin:$PATH"
export FAKE_GH_MODE=""
REAL_GIT=$(command -v git)

# ---- 沙箱主仓 + bare 远端 ----
git init -q --bare "$SANDBOX/origin.git"
MAIN="$SANDBOX/repo"
git init -q -b main "$MAIN"
cd "$MAIN"
git config user.email janitor-test@bong.local
git config user.name janitor-test
echo base > base.txt
printf 'server/target/\nclient/build/\nclient/.gradle/\n.env\n*.log\n' > .gitignore
git add -A && git commit -qm "沙箱基线"
git remote add origin "$SANDBOX/origin.git"
git push -q origin main
mkdir -p .agent-worktrees

new_wt() {
  local name="$1" br="$2" base="${3:-main}"
  git worktree add -q -b "$br" ".agent-worktrees/$name" "$base"
  git -C ".agent-worktrees/$name" push -q origin "$br"
  git fetch -q origin
}

stamp_commit() {
  # stamp_commit <worktree> <days_ago> <msg>：在 worktree 上做 empty commit 并 push
  local wt="$1" days="$2" msg="$3"
  local ts
  ts=$(date -d "@$(( $(date +%s) - days*86400 - 3600 ))" -R)
  GIT_AUTHOR_DATE="$ts" GIT_COMMITTER_DATE="$ts" \
    git -C ".agent-worktrees/$wt" commit --allow-empty -qm "$msg"
  git -C ".agent-worktrees/$wt" push -q origin "HEAD:$(git -C ".agent-worktrees/$wt" rev-parse --abbrev-ref HEAD)"
  git fetch -q origin
}

# ========== 基线场景 ==========
new_wt wt-merged-clean       merged-clean
new_wt wt-merged-ahead       merged-ahead
new_wt wt-merged-dirty       merged-dirty
new_wt wt-merged-env         merged-env
new_wt wt-merged-cache-only  merged-cache-only
new_wt wt-merged-untracked   merged-untracked
new_wt wt-merged-diverged    merged-diverged
new_wt wt-open-idle          open-idle
new_wt wt-nopr-idle          nopr-idle
new_wt wt-closed-clean       closed-clean
new_wt wt-closed-noremote    closed-noremote
new_wt slot-1                merged-slotted

# ahead：远端不可达额外提交
echo ahead > ".agent-worktrees/wt-merged-ahead/ahead.txt"
git -C ".agent-worktrees/wt-merged-ahead" add -A
git -C ".agent-worktrees/wt-merged-ahead" commit -qm "本地未推提交"

# dirty
echo dirty > ".agent-worktrees/wt-merged-dirty/base.txt"
mkdir -p ".agent-worktrees/wt-merged-dirty/server/target"
echo blob > ".agent-worktrees/wt-merged-dirty/server/target/blob"

# 仅 .env ignored
echo SECRET=1 > ".agent-worktrees/wt-merged-env/.env"

# 仅 allowlist cache
mkdir -p ".agent-worktrees/wt-merged-cache-only/server/target"
echo blob > ".agent-worktrees/wt-merged-cache-only/server/target/blob"

# non-ignored untracked
echo surprise > ".agent-worktrees/wt-merged-untracked/surprise.txt"

# diverged：本地有 patch，main 也前进
echo diverge > ".agent-worktrees/wt-merged-diverged/diverge.txt"
git -C ".agent-worktrees/wt-merged-diverged" add -A
git -C ".agent-worktrees/wt-merged-diverged" commit -qm "diverged local"
echo main-move > base.txt
git add base.txt && git commit -qm "main moves forward"
git push -q origin main
git fetch -q origin

# open / nopr 产物
mkdir -p ".agent-worktrees/wt-open-idle/server/target" ".agent-worktrees/wt-nopr-idle/client/build"
echo blob > ".agent-worktrees/wt-open-idle/server/target/blob"
echo blob > ".agent-worktrees/wt-nopr-idle/client/build/blob"

# clean 也带 cache
mkdir -p ".agent-worktrees/wt-merged-clean/server/target"
echo blob > ".agent-worktrees/wt-merged-clean/server/target/blob"

# closed 远端删除
git push -q origin --delete closed-noremote
git fetch -q origin --prune

# ---- squash 等价：feature 合入 main 后远端 branch 删除 ----
git worktree add -q -b merged-squash ".agent-worktrees/wt-merged-squash" main
echo squash-feat > ".agent-worktrees/wt-merged-squash/squash-feat.txt"
git -C ".agent-worktrees/wt-merged-squash" add -A
git -C ".agent-worktrees/wt-merged-squash" commit -qm "squash feature commit"
SQUASH_TIP=$(git -C ".agent-worktrees/wt-merged-squash" rev-parse HEAD)
git -C ".agent-worktrees/wt-merged-squash" push -q origin merged-squash
git checkout -q main
git merge --squash merged-squash
git commit -qm "squash merge of merged-squash"
git push -q origin main
git fetch -q origin
git push -q origin --delete merged-squash
git fetch -q origin --prune
git checkout -q main

# squash + extra local
git worktree add -q -b merged-squash-extra ".agent-worktrees/wt-merged-squash-extra" "$SQUASH_TIP"
echo extra-local > ".agent-worktrees/wt-merged-squash-extra/extra-local.txt"
git -C ".agent-worktrees/wt-merged-squash-extra" add -A
git -C ".agent-worktrees/wt-merged-squash-extra" commit -qm "extra after squash"

# ========== 1. report-only ==========
echo "== 1. report-only：任何树、分支、产物都不许动"
out=$(bash "$JANITOR")
check "wt-merged-clean 树仍在"        test -d ".agent-worktrees/wt-merged-clean"
check "merged-clean 分支仍在"          git show-ref -q "refs/heads/merged-clean"
check "open-idle 产物仍在"             test -f ".agent-worktrees/wt-open-idle/server/target/blob"
check "报告标出可回收(远端可达)"       grep -q "可回收（PR 已 MERGED，远端可达）" <<<"$out"
check "报告标出未合入 patch 交人工"    grep -q "未合入 patch" <<<"$out"
check "报告标出脏树交人工"             grep -q "PR 已 MERGED 但工作区不干净 → 交人工" <<<"$out"
check "报告标出 .env ignored 交人工"   grep -q "非缓存 ignored/untracked" <<<"$out"
check "报告标出 CLOSED 交人工"         grep -q "PR 已 CLOSED（未 merge）→ 交人工" <<<"$out"
check "报告标出 squash 等价可回收"     grep -q "squash/patch 已等价合入 origin/main" <<<"$out"
check "slot-* 被跳过"                  grep -q "SLOT（常驻保温，跳过" <<<"$out"
check ".env 文件仍在"                  test -f ".agent-worktrees/wt-merged-env/.env"

# ========== 2. clean-artifacts 不带 apply ==========
echo "== 2. --clean-artifacts 不带 --apply：只报告待清，不删"
out=$(bash "$JANITOR" --clean-artifacts=0)
check "open-idle 产物仍在"             test -f ".agent-worktrees/wt-open-idle/server/target/blob"
check "报告标出待清"                   grep -q "构建产物待清" <<<"$out"

# ========== 3. BUSY ==========
echo "== 3. BUSY：cwd 在树内的进程必须挡住回收"
( cd ".agent-worktrees/wt-merged-clean" && exec sleep 300 ) &
SLEEPER_PID=$!
sleep 0.15
out=$(bash "$JANITOR" --apply)
check "BUSY 树未被回收"                test -d ".agent-worktrees/wt-merged-clean"
check "报告标出 BUSY"                  grep -q "BUSY（有进程引用，跳过）" <<<"$out"
kill "$SLEEPER_PID" 2>/dev/null || true
wait "$SLEEPER_PID" 2>/dev/null || true
SLEEPER_PID=""

# ========== 4. --apply 主路径 ==========
echo "== 4. --apply：安全 MERGED 可回收；危险态一律保留"
out=$(bash "$JANITOR" --apply)
check "wt-merged-clean 已回收"         test ! -d ".agent-worktrees/wt-merged-clean"
check "merged-clean 分支已删"          bash -c '! git show-ref -q refs/heads/merged-clean'
check "wt-merged-cache-only 已回收"    test ! -d ".agent-worktrees/wt-merged-cache-only"
check "merged-cache-only 分支已删"     bash -c '! git show-ref -q refs/heads/merged-cache-only'
check "wt-merged-squash 已回收"        test ! -d ".agent-worktrees/wt-merged-squash"
check "merged-squash 分支已删"         bash -c '! git show-ref -q refs/heads/merged-squash'
check "wt-merged-ahead 保留"           test -d ".agent-worktrees/wt-merged-ahead"
check "merged-ahead 分支保留"          git show-ref -q "refs/heads/merged-ahead"
check "wt-merged-dirty 保留"           test -d ".agent-worktrees/wt-merged-dirty"
check "wt-merged-env 树保留"           test -d ".agent-worktrees/wt-merged-env"
check ".env 未静默删除"                test -f ".agent-worktrees/wt-merged-env/.env"
check "merged-env 分支保留"            git show-ref -q "refs/heads/merged-env"
check "wt-merged-untracked 保留"       test -d ".agent-worktrees/wt-merged-untracked"
check "surprise 仍在"                  test -f ".agent-worktrees/wt-merged-untracked/surprise.txt"
check "wt-merged-diverged 保留"        test -d ".agent-worktrees/wt-merged-diverged"
check "wt-merged-squash-extra 保留"    test -d ".agent-worktrees/wt-merged-squash-extra"
check "extra-local 仍在"               test -f ".agent-worktrees/wt-merged-squash-extra/extra-local.txt"
check "wt-closed-clean 保留"           test -d ".agent-worktrees/wt-closed-clean"
check "wt-closed-noremote 保留"        test -d ".agent-worktrees/wt-closed-noremote"
check "wt-open-idle 保留"              test -d ".agent-worktrees/wt-open-idle"
check "wt-nopr-idle 保留"              test -d ".agent-worktrees/wt-nopr-idle"
check "slot-1 保留"                    test -d ".agent-worktrees/slot-1"
check "报告含已回收"                   grep -q "已回收（PR MERGED" <<<"$out"
check "报告 .env 交人工"               grep -q "非缓存 ignored/untracked" <<<"$out"
check "报告 untracked/脏 交人工"       grep -q "工作区不干净" <<<"$out"

# ========== 5. clean-artifacts=0 ==========
echo "== 5. --apply --clean-artifacts=0：只清干净 OPEN/NO_PR 产物"
out=$(bash "$JANITOR" --apply --clean-artifacts=0)
check "open-idle 产物已清"             test ! -e ".agent-worktrees/wt-open-idle/server/target"
check "nopr-idle 产物已清"             test ! -e ".agent-worktrees/wt-nopr-idle/client/build"
check "脏树产物未动"                   test -f ".agent-worktrees/wt-merged-dirty/server/target/blob"
check "open-idle 树本体仍在"           test -d ".agent-worktrees/wt-open-idle"

# 恢复产物供阈值边界
mkdir -p ".agent-worktrees/wt-open-idle/server/target"
echo blob > ".agent-worktrees/wt-open-idle/server/target/blob"

# ========== 6. 默认 IDLE_DAYS=7 边界 ==========
echo "== 6. 默认 IDLE_DAYS=7：6 天不清 / 7 天清"
stamp_commit wt-open-idle 6 "idle stamp 6d"
out=$(bash "$JANITOR" --apply --clean-artifacts)
check "6天闲置产物未清"                test -f ".agent-worktrees/wt-open-idle/server/target/blob"
check_not "6天闲置报告无已清"          grep -q "构建产物已清" <<<"$out"

stamp_commit wt-open-idle 7 "idle stamp 7d"
out=$(bash "$JANITOR" --apply --clean-artifacts)
check "7天闲置产物已清"                test ! -e ".agent-worktrees/wt-open-idle/server/target"
check "7天闲置报告已清"                grep -q "构建产物已清" <<<"$out"

# ========== 7. gh 失败 → UNKNOWN ==========
echo "== 7. gh 非零退出 → UNKNOWN；不 clean/reclaim"
new_wt wt-merged-ghfail merged-ghfail
mkdir -p ".agent-worktrees/wt-open-idle/server/target"
echo blob > ".agent-worktrees/wt-open-idle/server/target/blob"
export FAKE_GH_MODE=fail
out=$(bash "$JANITOR" --apply --clean-artifacts=0 2>/dev/null || true)
check "gh 失败：merged-ghfail 保留"    test -d ".agent-worktrees/wt-merged-ghfail"
check "gh 失败：分支保留"              git show-ref -q "refs/heads/merged-ghfail"
check "gh 失败：open 产物未清"         test -f ".agent-worktrees/wt-open-idle/server/target/blob"
check "gh 失败：报告 UNKNOWN"          grep -q "UNKNOWN" <<<"$out"
check "gh 失败：不回收/不 clean 文案"  grep -q "不回收/不 clean" <<<"$out"
export FAKE_GH_MODE=""

# ========== 8. HAVE_GH=0 ==========
echo "== 8. HAVE_GH=0 → UNKNOWN 且不 clean/reclaim"
mv "$SANDBOX/bin/gh" "$SANDBOX/bin/gh.hidden"
out=$(bash "$JANITOR" --apply --clean-artifacts=0 2>/dev/null || true)
check "无 gh：merged-ghfail 保留"      test -d ".agent-worktrees/wt-merged-ghfail"
check "无 gh：open 产物未清"           test -f ".agent-worktrees/wt-open-idle/server/target/blob"
check "无 gh：报告 UNKNOWN"            grep -q "UNKNOWN" <<<"$out"
mv "$SANDBOX/bin/gh.hidden" "$SANDBOX/bin/gh"

# ========== 9. remove 被拒，整轮继续 ==========
echo "== 9. remove 被拒 → 交人工，其它可回收树继续"
cat > "$SANDBOX/bin/git" <<FAKEGIT
#!/usr/bin/env bash
if [[ "\$1" == "worktree" && "\$2" == "remove" ]]; then
  target="\$3"
  case "\$target" in
    *wt-merged-ghfail*)
      echo "error: simulated remove refusal" >&2
      exit 1
      ;;
  esac
fi
exec "$REAL_GIT" "\$@"
FAKEGIT
chmod +x "$SANDBOX/bin/git"
new_wt wt-merged-continue merged-continue
out=$(bash "$JANITOR" --apply)
check "remove 被拒：ghfail 树仍在"     test -d ".agent-worktrees/wt-merged-ghfail"
check "remove 被拒：报告交人工"        grep -q "remove 被拒" <<<"$out"
check "整轮继续：continue 已回收"      test ! -d ".agent-worktrees/wt-merged-continue"
check "整轮继续：continue 分支已删"    bash -c '! git show-ref -q refs/heads/merged-continue'
rm -f "$SANDBOX/bin/git"

# ========== 10. unpushed=ERR ==========
echo "== 10. unpushed 查询失败 → 交人工"
new_wt wt-merged-err merged-err
cat > "$SANDBOX/bin/git" <<FAKEGIT
#!/usr/bin/env bash
if [[ "\$1" == "rev-list" && "\$*" == *refs/heads/merged-err* ]]; then
  echo "error: simulated rev-list failure" >&2
  exit 1
fi
exec "$REAL_GIT" "\$@"
FAKEGIT
chmod +x "$SANDBOX/bin/git"
out=$(bash "$JANITOR" --apply)
check "rev-list 失败：树保留"          test -d ".agent-worktrees/wt-merged-err"
check "rev-list 失败：分支保留"        git show-ref -q "refs/heads/merged-err"
check "rev-list 失败：报告交人工"      grep -q "提交可达性查询失败" <<<"$out"
rm -f "$SANDBOX/bin/git"

# ========== 11. OPEN/NO_PR dirty 不 clean ==========
echo "== 11. OPEN dirty 不 clean；NO_PR 干净 idle 才 clean"
echo dirty-open > ".agent-worktrees/wt-open-idle/base.txt"
mkdir -p ".agent-worktrees/wt-open-idle/server/target"
echo blob > ".agent-worktrees/wt-open-idle/server/target/blob"
mkdir -p ".agent-worktrees/wt-nopr-idle/client/build"
echo blob > ".agent-worktrees/wt-nopr-idle/client/build/blob"
out=$(bash "$JANITOR" --apply --clean-artifacts=0)
check "OPEN dirty 产物未清"            test -f ".agent-worktrees/wt-open-idle/server/target/blob"
check "NO_PR 干净产物已清"             test ! -e ".agent-worktrees/wt-nopr-idle/client/build"
git -C ".agent-worktrees/wt-open-idle" checkout -q -- base.txt 2>/dev/null || true

# ========== 12. report-only 关键字 ==========
echo "== 12. report-only：slot / CLOSED 关键字"
out=$(bash "$JANITOR")
check "slot 报告"                      grep -q "SLOT（常驻保温，跳过" <<<"$out"
check "CLOSED 报告"                    grep -q "CLOSED" <<<"$out"

echo "---"
echo "PASS=$PASS FAIL=$FAIL"
[[ $FAIL -eq 0 ]] || exit 1
echo "wt-janitor 契约测试全部通过"
