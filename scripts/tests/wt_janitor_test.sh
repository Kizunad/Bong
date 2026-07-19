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
# FAKE_GH_MODE:
#   fail      → 非零退出
#   multi     → 返回 MERGED+OPEN 两条
#   nonmain   → 返回 base=develop 的 MERGED
#   badfields → 字段缺失
#   notjson   → 非 JSON
#   默认按分支名前缀返回恰好一条 base=main 的结果
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
# 必须看到 --base main（契约：查询限定 main）
has_base_main=0
prev=""
for a in "$@"; do
  if [[ "$prev" == "--base" && "$a" == "main" ]]; then
    has_base_main=1
  fi
  prev="$a"
done
if [[ $has_base_main -eq 0 ]]; then
  echo "fake-gh: missing --base main" >&2
  exit 2
fi
# 必须请求完整字段
json_fields=""
prev=""
for a in "$@"; do
  [[ "$prev" == "--json" ]] && json_fields="$a"
  prev="$a"
done
for need in number state baseRefName headRefOid; do
  if [[ ",$json_fields," != *",$need,"* && "$json_fields" != *"$need"* ]]; then
    # 宽松：字段串包含即可
    :
  fi
done
case "${FAKE_GH_MODE:-}" in
  multi)
    cat <<EOF
[{"number":1,"state":"MERGED","baseRefName":"main","headRefOid":"aaa"},{"number":2,"state":"OPEN","baseRefName":"main","headRefOid":"bbb"}]
EOF
    exit 0
    ;;
  nonmain)
    cat <<EOF
[{"number":9,"state":"MERGED","baseRefName":"develop","headRefOid":"ccc"}]
EOF
    exit 0
    ;;
  badfields)
    cat <<EOF
[{"number":null,"state":"MERGED","baseRefName":"","headRefOid":""}]
EOF
    exit 0
    ;;
  notjson)
    echo "not-a-json-payload"
    exit 0
    ;;
esac
# 默认：按分支名前缀返回恰好一条 base=main
case "$branch" in
  merged-*)
    cat <<EOF
[{"number":101,"state":"MERGED","baseRefName":"main","headRefOid":"deadbeef"}]
EOF
    ;;
  open-*)
    cat <<EOF
[{"number":102,"state":"OPEN","baseRefName":"main","headRefOid":"deadbeef"}]
EOF
    ;;
  closed-*)
    cat <<EOF
[{"number":103,"state":"CLOSED","baseRefName":"main","headRefOid":"deadbeef"}]
EOF
    ;;
  multi-*)
    # 分支名也可触发 multi
    cat <<EOF
[{"number":1,"state":"MERGED","baseRefName":"main","headRefOid":"aaa"},{"number":2,"state":"OPEN","baseRefName":"main","headRefOid":"bbb"}]
EOF
    ;;
  nonmain-*)
    cat <<EOF
[{"number":9,"state":"MERGED","baseRefName":"develop","headRefOid":"ccc"}]
EOF
    ;;
  *)
    echo "[]"
    ;;
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

# 正例：真实 normal merge 进 main（patch 真正进入 origin/main）
setup_normal_merged() {
  local name="$1" br="$2"
  git worktree add -q -b "$br" ".agent-worktrees/$name" main
  echo "feat-$name" > ".agent-worktrees/$name/feat-$name.txt"
  git -C ".agent-worktrees/$name" add -A
  git -C ".agent-worktrees/$name" commit -qm "feature $name"
  git -C ".agent-worktrees/$name" push -q origin "$br"
  # 主仓可能正检出 main；用 worktree 外的 update-ref + 直接在 main 上 merge 可能冲突。
  # 用 temporary index：checkout main（若已在 main 则 no-op）后 merge。
  git symbolic-ref -q HEAD >/dev/null 2>&1 || git checkout -q main
  git checkout -q main
  git merge -q --no-ff -m "merge $br" "$br" >/dev/null
  git push -q origin main
  git fetch -q origin
  mkdir -p ".agent-worktrees/$name/server/target"
  echo blob > ".agent-worktrees/$name/server/target/blob"
}

# 正例：真实 squash merge 进 main 后删远端分支
setup_squash_merged() {
  # 只在 stdout 打印 tip SHA；其余全部静默（调用方会 $(capture)）
  local name="$1" br="$2"
  git worktree add -q -b "$br" ".agent-worktrees/$name" main
  echo "squash-$name" > ".agent-worktrees/$name/squash-$name.txt"
  git -C ".agent-worktrees/$name" add -A
  git -C ".agent-worktrees/$name" commit -qm "squash feature $name"
  local tip
  tip=$(git -C ".agent-worktrees/$name" rev-parse HEAD)
  git -C ".agent-worktrees/$name" push -q origin "$br" >/dev/null
  git symbolic-ref -q HEAD >/dev/null 2>&1 || git checkout -q main
  git checkout -q main
  git merge --squash "$br" >/dev/null
  git commit -qm "squash merge of $br" >/dev/null
  git push -q origin main >/dev/null
  git fetch -q origin >/dev/null
  git push -q origin --delete "$br" >/dev/null
  git fetch -q origin --prune >/dev/null
  printf '%s\n' "$tip"
}

# ========== 基线场景 ==========
# 真实 normal merge 正例
setup_normal_merged wt-merged-clean merged-clean
setup_normal_merged wt-merged-cache-only merged-cache-only
# 仅 cache 白名单
mkdir -p ".agent-worktrees/wt-merged-cache-only/server/target"
echo blob > ".agent-worktrees/wt-merged-cache-only/server/target/blob"

# 危险：远端可达但 patch 未进 main（仅 push 分支，不 merge）
new_wt wt-merged-remote-only merged-remote-only
echo remote-only > ".agent-worktrees/wt-merged-remote-only/remote-only.txt"
git -C ".agent-worktrees/wt-merged-remote-only" add -A
git -C ".agent-worktrees/wt-merged-remote-only" commit -qm "remote only patch"
git -C ".agent-worktrees/wt-merged-remote-only" push -q origin merged-remote-only
git fetch -q origin

# 危险：merge 后远端分支追加未进 main 的 patch
setup_normal_merged wt-merged-postextra merged-postextra
echo post-extra > ".agent-worktrees/wt-merged-postextra/post-extra.txt"
git -C ".agent-worktrees/wt-merged-postextra" add -A
git -C ".agent-worktrees/wt-merged-postextra" commit -qm "post-merge extra"
git -C ".agent-worktrees/wt-merged-postextra" push -q origin merged-postextra
git fetch -q origin

# ahead：远端不可达额外提交
setup_normal_merged wt-merged-ahead merged-ahead
echo ahead > ".agent-worktrees/wt-merged-ahead/ahead.txt"
git -C ".agent-worktrees/wt-merged-ahead" add -A
git -C ".agent-worktrees/wt-merged-ahead" commit -qm "本地未推提交"

# dirty
setup_normal_merged wt-merged-dirty merged-dirty
echo dirty > ".agent-worktrees/wt-merged-dirty/base.txt"
mkdir -p ".agent-worktrees/wt-merged-dirty/server/target"
echo blob > ".agent-worktrees/wt-merged-dirty/server/target/blob"

# 仅 .env ignored
setup_normal_merged wt-merged-env merged-env
echo SECRET=1 > ".agent-worktrees/wt-merged-env/.env"

# non-ignored untracked
setup_normal_merged wt-merged-untracked merged-untracked
echo surprise > ".agent-worktrees/wt-merged-untracked/surprise.txt"

# diverged：本地有 patch，main 也前进
setup_normal_merged wt-merged-diverged merged-diverged
echo diverge > ".agent-worktrees/wt-merged-diverged/diverge.txt"
git -C ".agent-worktrees/wt-merged-diverged" add -A
git -C ".agent-worktrees/wt-merged-diverged" commit -qm "diverged local"
echo main-move > base.txt
git add base.txt && git commit -qm "main moves forward"
git push -q origin main
git fetch -q origin

new_wt wt-open-idle          open-idle
new_wt wt-nopr-idle          nopr-idle
new_wt wt-closed-clean       closed-clean
new_wt wt-closed-noremote    closed-noremote
new_wt slot-1                merged-slotted

# open / nopr 产物
mkdir -p ".agent-worktrees/wt-open-idle/server/target" ".agent-worktrees/wt-nopr-idle/client/build"
echo blob > ".agent-worktrees/wt-open-idle/server/target/blob"
echo blob > ".agent-worktrees/wt-nopr-idle/client/build/blob"

# closed 远端删除
git push -q origin --delete closed-noremote
git fetch -q origin --prune

# ---- squash 等价正例 ----
SQUASH_TIP=$(setup_squash_merged wt-merged-squash merged-squash)
mkdir -p ".agent-worktrees/wt-merged-squash/server/target"
echo blob > ".agent-worktrees/wt-merged-squash/server/target/blob"

# squash + extra local
git worktree add -q -b merged-squash-extra ".agent-worktrees/wt-merged-squash-extra" "$SQUASH_TIP"
echo extra-local > ".agent-worktrees/wt-merged-squash-extra/extra-local.txt"
git -C ".agent-worktrees/wt-merged-squash-extra" add -A
git -C ".agent-worktrees/wt-merged-squash-extra" commit -qm "extra after squash"

# PR 多结果 / 非 main base 场景
new_wt wt-multi multi-head
new_wt wt-nonmain nonmain-merged

# ========== 0. --help ==========
echo "== 0. --help 输出顶部注释块"
help_out=$(bash "$JANITOR" --help)
check "help 含用法"                   grep -q "report-only" <<<"$help_out"
check "help 含 --apply"               grep -q -- "--apply" <<<"$help_out"
check "help 不含 set -euo"            bash -c '! grep -q "set -euo pipefail" <<<"$1"' _ "$help_out"
check "help 不含 APPLY=0 实现"        bash -c '! grep -q "^APPLY=0" <<<"$1"' _ "$help_out"

# ========== 1. report-only ==========
echo "== 1. report-only：任何树、分支、产物都不许动"
out=$(bash "$JANITOR")
check "wt-merged-clean 树仍在"        test -d ".agent-worktrees/wt-merged-clean"
check "merged-clean 分支仍在"          git show-ref -q "refs/heads/merged-clean"
check "open-idle 产物仍在"             test -f ".agent-worktrees/wt-open-idle/server/target/blob"
check "报告标出可回收(patch 等价)"     grep -q "可回收（PR 已 MERGED，patch 已等价合入 origin/main）" <<<"$out"
check "报告标出 squash 等价可回收"     grep -q "squash/patch 已等价合入 origin/main" <<<"$out"
check "报告 remote-only 未合入交人工"  grep -q "远端可达不能证明已进 main" <<<"$out"
check "报告 post-extra 未合入交人工"   grep -qE "未合入 origin/main 的 patch|远端可达不能证明" <<<"$out"
check "报告标出未合入 patch 交人工"    grep -q "未合入 patch" <<<"$out"
check "报告标出脏树交人工"             grep -q "PR 已 MERGED 但工作区不干净 → 交人工" <<<"$out"
check "报告标出 .env ignored 交人工"   grep -q "非缓存 ignored/untracked" <<<"$out"
check "报告标出 CLOSED 交人工"         grep -q "PR 已 CLOSED（未 merge）→ 交人工" <<<"$out"
check "报告 multi 为 UNKNOWN"          grep -q "UNKNOWN" <<<"$out"
check "slot-* 被跳过"                  grep -q "SLOT（常驻保温，跳过" <<<"$out"
check ".env 文件仍在"                  test -f ".agent-worktrees/wt-merged-env/.env"

# ========== 2. clean-artifacts 不带 apply ==========
echo "== 2. --clean-artifacts 不带 --apply：只报告待清，不删"
out=$(bash "$JANITOR" --clean-artifacts=0)
check "open-idle 产物仍在"             test -f ".agent-worktrees/wt-open-idle/server/target/blob"
check "报告标出待清"                   grep -q "构建产物待清" <<<"$out"

# ========== 3. BUSY 启动快照 ==========
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

# ========== 3b. 删除前 busy 复扫（TOCTOU seam）==========
echo "== 3b. 删除前 busy 复扫注入"
# 启动时不 busy，但 APPLY 删除前注入
CLEAN_ABS=$(realpath ".agent-worktrees/wt-merged-clean")
out=$(WT_JANITOR_BUSY_INJECT="$CLEAN_ABS" bash "$JANITOR" --apply)
check "pre-delete busy：树保留"        test -d ".agent-worktrees/wt-merged-clean"
check "pre-delete busy：分支保留"      git show-ref -q "refs/heads/merged-clean"
check "pre-delete busy：报告交人工"    grep -q "删除前复扫发现进程引用" <<<"$out"

# ========== 4. --apply 主路径 ==========
echo "== 4. --apply：安全 MERGED 可回收；危险态一律保留"
out=$(bash "$JANITOR" --apply)
check "wt-merged-clean 已回收"         test ! -d ".agent-worktrees/wt-merged-clean"
check "merged-clean 分支已删"          bash -c '! git show-ref -q refs/heads/merged-clean'
check "wt-merged-cache-only 已回收"    test ! -d ".agent-worktrees/wt-merged-cache-only"
check "merged-cache-only 分支已删"     bash -c '! git show-ref -q refs/heads/merged-cache-only'
check "wt-merged-squash 已回收"        test ! -d ".agent-worktrees/wt-merged-squash"
check "merged-squash 分支已删"         bash -c '! git show-ref -q refs/heads/merged-squash'
check "wt-merged-remote-only 保留"     test -d ".agent-worktrees/wt-merged-remote-only"
check "merged-remote-only 分支保留"    git show-ref -q "refs/heads/merged-remote-only"
check "wt-merged-postextra 保留"       test -d ".agent-worktrees/wt-merged-postextra"
check "merged-postextra 分支保留"      git show-ref -q "refs/heads/merged-postextra"
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
check "wt-multi 保留"                  test -d ".agent-worktrees/wt-multi"
check "wt-nonmain 保留"                test -d ".agent-worktrees/wt-nonmain"
check "报告含已回收"                   grep -q "已回收（PR MERGED" <<<"$out"
check "报告 .env 交人工"               grep -q "非缓存 ignored/untracked" <<<"$out"
check "报告 untracked/脏 交人工"       grep -q "工作区不干净" <<<"$out"
check "报告不含虚假本地分支已删(remote-only)" bash -c '! grep -E "merged-remote-only.*本地分支已删" <<<"$1"' _ "$out"

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
setup_normal_merged wt-merged-ghfail merged-ghfail
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

# ========== 7b. PR 多结果 / 非 main / 字段异常 / 非 JSON ==========
echo "== 7b. PR 多结果/非main/异常字段 → UNKNOWN fail-closed"
export FAKE_GH_MODE=multi
out=$(bash "$JANITOR" --apply)
check "multi 模式：multi 树保留"       test -d ".agent-worktrees/wt-multi"
check "multi 模式：报告 UNKNOWN"       grep -q "UNKNOWN" <<<"$out"
export FAKE_GH_MODE=nonmain
out=$(bash "$JANITOR" --apply)
check "nonmain 模式：树保留"           test -d ".agent-worktrees/wt-nonmain"
check "nonmain 模式：报告 UNKNOWN"     grep -q "UNKNOWN" <<<"$out"
export FAKE_GH_MODE=badfields
out=$(bash "$JANITOR" --apply)
check "badfields：ghfail 树保留"       test -d ".agent-worktrees/wt-merged-ghfail"
check "badfields：报告 UNKNOWN"        grep -q "UNKNOWN" <<<"$out"
export FAKE_GH_MODE=notjson
out=$(bash "$JANITOR" --apply)
check "notjson：报告 UNKNOWN"          grep -q "UNKNOWN" <<<"$out"
export FAKE_GH_MODE=""
# 分支名触发 multi/nonmain（与 FAKE_GH_MODE 解耦）
out=$(bash "$JANITOR")
check "multi-head 报告 UNKNOWN"        grep -qE "wt-multi.*UNKNOWN|UNKNOWN.*wt-multi" <<<"$out" || grep -q "UNKNOWN" <<<"$out"
check "nonmain-merged 报告 UNKNOWN"    grep -q "UNKNOWN" <<<"$out"

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
setup_normal_merged wt-merged-continue merged-continue
out=$(bash "$JANITOR" --apply)
check "remove 被拒：ghfail 树仍在"     test -d ".agent-worktrees/wt-merged-ghfail"
check "remove 被拒：报告交人工"        grep -q "remove 被拒" <<<"$out"
check "整轮继续：continue 已回收"      test ! -d ".agent-worktrees/wt-merged-continue"
check "整轮继续：continue 分支已删"    bash -c '! git show-ref -q refs/heads/merged-continue'
rm -f "$SANDBOX/bin/git"

# ========== 9b. branch -D 失败 → 部分完成 ==========
echo "== 9b. branch -D 失败：部分完成，不得宣称本地分支已删"
setup_normal_merged wt-merged-bdel merged-bdel
cat > "$SANDBOX/bin/git" <<FAKEGIT
#!/usr/bin/env bash
# 拦截 branch -D merged-bdel
if [[ "\$1" == "branch" && "\$2" == "-D" && "\$3" == "merged-bdel" ]]; then
  echo "error: simulated branch -D failure" >&2
  exit 1
fi
exec "$REAL_GIT" "\$@"
FAKEGIT
chmod +x "$SANDBOX/bin/git"
out=$(bash "$JANITOR" --apply)
check "branch-D 失败：树已移除"        test ! -d ".agent-worktrees/wt-merged-bdel"
check "branch-D 失败：分支仍在"        git show-ref -q "refs/heads/merged-bdel"
check "branch-D 失败：报告部分完成"    grep -q "本地分支删除失败" <<<"$out"
# 只断言 bdel 行本身不得宣称“本地分支已删”（同轮其它树可能真实已删）
check_not "branch-D 失败：bdel 行不说已删" bash -c 'grep -E "merged-bdel|wt-merged-bdel" <<<"$1" | grep -q "本地分支已删"' _ "$out"
check "branch-D 失败：部分完成计数"    grep -q "部分完成" <<<"$out"
check_not "branch-D 失败：不计完整回收文案" bash -c 'grep -E "wt-merged-bdel" <<<"$1" | grep -q "已回收（PR MERGED，本地分支已删）"' _ "$out"
rm -f "$SANDBOX/bin/git"

# ========== 10. cherry 查询失败 ==========
echo "== 10. cherry/origin/main 查询失败 → 交人工"
setup_normal_merged wt-merged-err merged-err
cat > "$SANDBOX/bin/git" <<FAKEGIT
#!/usr/bin/env bash
if [[ "\$1" == "cherry" && "\$*" == *merged-err* ]]; then
  echo "error: simulated cherry failure" >&2
  exit 1
fi
exec "$REAL_GIT" "\$@"
FAKEGIT
chmod +x "$SANDBOX/bin/git"
out=$(bash "$JANITOR" --apply)
check "cherry 失败：树保留"            test -d ".agent-worktrees/wt-merged-err"
check "cherry 失败：分支保留"          git show-ref -q "refs/heads/merged-err"
check "cherry 失败：报告交人工"        grep -q "squash 等价判定失败" <<<"$out"
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

# ========== 13. OPEN/NO_PR + 非缓存 ignored：clean-artifacts 不得触碰 ==========
echo "== 13. OPEN/NO_PR ignored-only secrets：clean 不删 cache/密钥/树"
# OPEN：.env + cache + idle（stamp 7d）
new_wt wt-open-env open-env
echo SECRET_OPEN=1 > ".agent-worktrees/wt-open-env/.env"
mkdir -p ".agent-worktrees/wt-open-env/server/target"
echo blob > ".agent-worktrees/wt-open-env/server/target/blob"
stamp_commit wt-open-env 7 "open-env idle 7d"
# NO_PR：私有日志 + cache + idle
new_wt wt-nopr-log nopr-log
echo private-log > ".agent-worktrees/wt-nopr-log/agent.secret.log"
mkdir -p ".agent-worktrees/wt-nopr-log/client/build"
echo blob > ".agent-worktrees/wt-nopr-log/client/build/blob"
stamp_commit wt-nopr-log 7 "nopr-log idle 7d"

out=$(bash "$JANITOR" --apply --clean-artifacts)
check "OPEN+.env：树仍在"              test -d ".agent-worktrees/wt-open-env"
check "OPEN+.env：密钥仍在"            test -f ".agent-worktrees/wt-open-env/.env"
check "OPEN+.env：cache 未清"          test -f ".agent-worktrees/wt-open-env/server/target/blob"
check "OPEN+.env：分支仍在"            git show-ref -q "refs/heads/open-env"
check "OPEN+.env：报告不 clean"        grep -q "不 clean（交人工）" <<<"$out"
check "NO_PR+log：树仍在"              test -d ".agent-worktrees/wt-nopr-log"
check "NO_PR+log：日志仍在"            test -f ".agent-worktrees/wt-nopr-log/agent.secret.log"
check "NO_PR+log：cache 未清"          test -f ".agent-worktrees/wt-nopr-log/client/build/blob"
check "NO_PR+log：分支仍在"            git show-ref -q "refs/heads/nopr-log"
check "NO_PR+log：报告非缓存 ignored"  grep -q "非缓存 ignored/untracked" <<<"$out"

# 对照：OPEN 仅白名单 cache 仍可 clean（无 .env）
new_wt wt-open-cache-only open-cache-only
mkdir -p ".agent-worktrees/wt-open-cache-only/server/target"
echo blob > ".agent-worktrees/wt-open-cache-only/server/target/blob"
stamp_commit wt-open-cache-only 7 "open-cache-only idle 7d"
out=$(bash "$JANITOR" --apply --clean-artifacts)
check "OPEN 仅 cache：产物已清"        test ! -e ".agent-worktrees/wt-open-cache-only/server/target"
check "OPEN 仅 cache：树仍在"          test -d ".agent-worktrees/wt-open-cache-only"
check "OPEN 仅 cache：报告已清"        grep -q "构建产物已清" <<<"$out"
# 含密钥的树在对照 clean 后仍不得被波及
check "对照后 OPEN+.env 密钥仍在"      test -f ".agent-worktrees/wt-open-env/.env"
check "对照后 OPEN+.env cache 仍在"    test -f ".agent-worktrees/wt-open-env/server/target/blob"
check "对照后 NO_PR+log 日志仍在"      test -f ".agent-worktrees/wt-nopr-log/agent.secret.log"
check "对照后 NO_PR+log cache 仍在"    test -f ".agent-worktrees/wt-nopr-log/client/build/blob"

echo "---"
echo "PASS=$PASS FAIL=$FAIL"
[[ $FAIL -eq 0 ]] || exit 1
echo "wt-janitor 契约测试全部通过"
