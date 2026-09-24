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
# 模拟真实 gh：
#   1) 必须 exact-head + --state all 全量查询（不带 --base；默认 OPEN 会漏 MERGED）
#   2) 正常响应的 headRefOid = 本地 refs/heads/<branch> 真实 tip（40 hex）
#   3) 缺 --state all / 带 --base / 缺 JSON 字段 → 非零退出（锁生产契约）
# FAKE_GH_MODE:
#   fail / multi / nonmain / badfields / emptyoid / nulloid / shortoid / notjson /
#   dualbase / staleoid / no_state_all
# dualbase: 同 head 同时有 MERGED@main + MERGED@develop（真实 --base main 会隐藏 develop）
# staleoid: 返回合法 40-hex 但与当前 tip 不同（branch reuse 反例）
#
# 真正的 fake gh 在 MAIN 初始化后写入（见下方「重写 fake gh」），此处只占 PATH 槽位。
mkdir -p "$SANDBOX/bin"
REAL_GIT=$(command -v git)
cat > "$SANDBOX/bin/gh" <<'PLACEHOLDER'
#!/usr/bin/env bash
echo "fake-gh placeholder: MAIN not initialized" >&2
exit 99
PLACEHOLDER
chmod +x "$SANDBOX/bin/gh"
export PATH="$SANDBOX/bin:$PATH"
export FAKE_GH_MODE=""

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

# 现在 MAIN 已确定：重写 fake gh，把 MAIN_REPO 常量钉死（闭包不可变）
cat > "$SANDBOX/bin/gh" <<FAKEGH
#!/usr/bin/env bash
set -euo pipefail
REAL_GIT="$REAL_GIT"
MAIN_REPO="$MAIN"
if [[ "\${FAKE_GH_MODE:-}" == "fail" ]]; then
  echo "simulated gh failure" >&2
  exit 1
fi
if [[ "\${FAKE_GH_MODE:-}" == "no_state_all" ]]; then
  echo "fake-gh: FAKE_GH_MODE=no_state_all forces missing --state all semantics" >&2
  exit 2
fi
branch=""
prev=""
has_base=0
base_val=""
has_state_all=0
json_fields=""
args=("\$@")
for a in "\${args[@]}"; do
  [[ "\$prev" == "--head" ]] && branch="\$a"
  if [[ "\$prev" == "--base" ]]; then
    has_base=1
    base_val="\$a"
  fi
  if [[ "\$prev" == "--state" && "\$a" == "all" ]]; then
    has_state_all=1
  fi
  if [[ "\$a" == "--state=all" ]]; then
    has_state_all=1
  fi
  if [[ "\$prev" == "--json" ]]; then
    json_fields="\$a"
  fi
  prev="\$a"
done
if [[ \$has_base -eq 1 ]]; then
  echo "fake-gh: unexpected --base \$base_val (must query full head set without --base)" >&2
  exit 2
fi
if [[ \$has_state_all -ne 1 ]]; then
  echo "fake-gh: missing required --state all (got: \${args[*]})" >&2
  exit 2
fi
IFS=',' read -r -a _jf_arr <<< "\$json_fields"
for need in number state baseRefName headRefOid; do
  found=0
  for f in "\${_jf_arr[@]+"\${_jf_arr[@]}"}"; do
    [[ "\$f" == "\$need" ]] && found=1 && break
  done
  if [[ \$found -ne 1 ]]; then
    echo "fake-gh: missing required --json field: \$need (got: \$json_fields)" >&2
    exit 2
  fi
done
branch_tip=""
if [[ -n "\$branch" ]]; then
  branch_tip=\$("\$REAL_GIT" -C "\$MAIN_REPO" rev-parse --verify -q "refs/heads/\$branch^{commit}" 2>/dev/null || true)
fi
if [[ -z "\$branch_tip" ]]; then
  branch_tip="0000000000000000000000000000000000000000"
fi
stale_oid="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
if [[ "\$stale_oid" == "\$branch_tip" ]]; then
  stale_oid="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
fi
case "\${FAKE_GH_MODE:-}" in
  multi)
    cat <<EOF
[{"number":1,"state":"MERGED","baseRefName":"main","headRefOid":"\$branch_tip"},{"number":2,"state":"OPEN","baseRefName":"main","headRefOid":"\$branch_tip"}]
EOF
    exit 0
    ;;
  dualbase)
    cat <<EOF
[{"number":10,"state":"MERGED","baseRefName":"main","headRefOid":"\$branch_tip"},{"number":11,"state":"MERGED","baseRefName":"develop","headRefOid":"\$branch_tip"}]
EOF
    exit 0
    ;;
  nonmain)
    cat <<EOF
[{"number":9,"state":"MERGED","baseRefName":"develop","headRefOid":"\$branch_tip"}]
EOF
    exit 0
    ;;
  badfields)
    cat <<EOF
[{"number":null,"state":"MERGED","baseRefName":"","headRefOid":""}]
EOF
    exit 0
    ;;
  emptyoid)
    cat <<EOF
[{"number":201,"state":"MERGED","baseRefName":"main","headRefOid":""}]
EOF
    exit 0
    ;;
  nulloid)
    cat <<EOF
[{"number":202,"state":"MERGED","baseRefName":"main","headRefOid":null}]
EOF
    exit 0
    ;;
  shortoid)
    cat <<EOF
[{"number":203,"state":"MERGED","baseRefName":"main","headRefOid":"deadbeef"}]
EOF
    exit 0
    ;;
  staleoid)
    cat <<EOF
[{"number":204,"state":"MERGED","baseRefName":"main","headRefOid":"\$stale_oid"}]
EOF
    exit 0
    ;;
  notjson)
    echo "not-a-json-payload"
    exit 0
    ;;
esac
case "\$branch" in
  merged-reuse)
    cat <<EOF
[{"number":301,"state":"MERGED","baseRefName":"main","headRefOid":"\$stale_oid"}]
EOF
    ;;
  merged-*)
    cat <<EOF
[{"number":101,"state":"MERGED","baseRefName":"main","headRefOid":"\$branch_tip"}]
EOF
    ;;
  open-*)
    cat <<EOF
[{"number":102,"state":"OPEN","baseRefName":"main","headRefOid":"\$branch_tip"}]
EOF
    ;;
  closed-*)
    cat <<EOF
[{"number":103,"state":"CLOSED","baseRefName":"main","headRefOid":"\$branch_tip"}]
EOF
    ;;
  multi-*)
    cat <<EOF
[{"number":1,"state":"MERGED","baseRefName":"main","headRefOid":"\$branch_tip"},{"number":2,"state":"OPEN","baseRefName":"main","headRefOid":"\$branch_tip"}]
EOF
    ;;
  dualbase-*)
    cat <<EOF
[{"number":10,"state":"MERGED","baseRefName":"main","headRefOid":"\$branch_tip"},{"number":11,"state":"MERGED","baseRefName":"develop","headRefOid":"\$branch_tip"}]
EOF
    ;;
  nonmain-*)
    cat <<EOF
[{"number":9,"state":"MERGED","baseRefName":"develop","headRefOid":"\$branch_tip"}]
EOF
    ;;
  *)
    echo "[]"
    ;;
esac
FAKEGH
chmod +x "$SANDBOX/bin/gh"

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
  ts=$(date -d "@$(( $(date +%s) - days*86400 - 7200 ))" -R)
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


# ---- branch-only merge：非 merge patch 已 patch-equivalent 进 main，但 branch 仍有 merge ----
# 这是 git cherry 的经典盲区：cherry 全为 - / 空，但 origin/main..branch 含 merge commit
# （含 conflict resolution 的独有 tree 风险）。实现必须 fail-closed 保留。
git checkout -q main
git worktree add -q -b merged-mergeonly ".agent-worktrees/wt-merged-mergeonly" main
echo mergeonly-feat > ".agent-worktrees/wt-merged-mergeonly/mergeonly-feat.txt"
git -C ".agent-worktrees/wt-merged-mergeonly" add -A
git -C ".agent-worktrees/wt-merged-mergeonly" commit -qm "mergeonly feature"
# main 前进 + squash 合入 feature patch
echo main-adv-for-mergeonly > main-adv-mergeonly.txt
git add main-adv-mergeonly.txt
git commit -qm "main advances before squash"
git merge --squash merged-mergeonly
git commit -qm "squash mergeonly feature into main"
git push -q origin main
git fetch -q origin
# 分支 merge 新 main → 产生仅存在于 branch 的 merge commit
git -C ".agent-worktrees/wt-merged-mergeonly" merge -q -m "branch-only merge of main" origin/main
# 在 merge 上制造独有 conflict-resolution 树：再开旁支冲突合入
git checkout -q -b side-conflict-for-mergeonly main
echo conflict-side > conflict-res.txt
git add conflict-res.txt
git commit -qm "side conflict tip"
git checkout -q main
echo conflict-main > conflict-res.txt
git add conflict-res.txt
git commit -qm "main conflict tip"
git push -q origin main
git fetch -q origin
git -C ".agent-worktrees/wt-merged-mergeonly" merge -q origin/main || true
# 若无冲突，强制写 unique resolution 并作为 merge 结果提交
if git -C ".agent-worktrees/wt-merged-mergeonly" rev-parse -q --verify MERGE_HEAD >/dev/null; then
  echo conflict-UNIQUE-resolution > ".agent-worktrees/wt-merged-mergeonly/conflict-res.txt"
  git -C ".agent-worktrees/wt-merged-mergeonly" add -A
  git -C ".agent-worktrees/wt-merged-mergeonly" commit -qm "branch-only merge unique resolution"
else
  # 干净 merge 后仍追加一个 merge from side 制造 merge commit
  git -C ".agent-worktrees/wt-merged-mergeonly" merge -q -m "branch-only merge side" side-conflict-for-mergeonly || true
  if git -C ".agent-worktrees/wt-merged-mergeonly" rev-parse -q --verify MERGE_HEAD >/dev/null; then
    echo conflict-UNIQUE-resolution > ".agent-worktrees/wt-merged-mergeonly/conflict-res.txt"
    git -C ".agent-worktrees/wt-merged-mergeonly" add -A
    git -C ".agent-worktrees/wt-merged-mergeonly" commit -qm "branch-only merge unique resolution"
  fi
fi
git -C ".agent-worktrees/wt-merged-mergeonly" push -q origin merged-mergeonly
git fetch -q origin
# 把 side 非 merge patch 也弄进 main，尽量让 cherry 干净，只剩 merge 盲区
git checkout -q main
# main 已有 conflict-main；side 的 tip 不进 main，但我们把 side 的文件内容用另一路径吸收非必要
# 关键：至少保证存在 merge commit；若 cherry 仍有 + 也必须保留
MERGE_ONLY_MERGES=$(git rev-list --merges origin/main..merged-mergeonly | wc -l)
[[ "$MERGE_ONLY_MERGES" -ge 1 ]] || { echo "setup failed: no branch-only merge on merged-mergeonly" >&2; exit 1; }
echo "mergeonly merges=$MERGE_ONLY_MERGES cherry=$(git cherry origin/main merged-mergeonly | wc -l) plus=$(git cherry origin/main merged-mergeonly | grep -c '^+')||true)"
git checkout -q main

# dualbase head 名
new_wt wt-dualbase dualbase-head

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
check "报告 post-extra 未合入交人工"   grep -qE "未合入 origin/main 的 patch|远端可达不能证明|patch/merge" <<<"$out"
check "报告 mergeonly 未合入交人工"    grep -qE "patch/merge|未合入" <<<"$out"
check "报告标出未合入 patch 交人工"    grep -q "未合入 patch" <<<"$out"
check "报告标出脏树交人工"             grep -q "PR 已 MERGED 但工作区不干净 → 交人工" <<<"$out"
check "报告标出 .env ignored 交人工"   grep -q "非缓存 ignored/untracked" <<<"$out"
check "报告标出 CLOSED 交人工"         grep -q "PR 已 CLOSED（未 merge）→ 交人工" <<<"$out"
check "报告 multi 为 UNKNOWN"          grep -q "UNKNOWN" <<<"$out"
check "slot-* 被跳过"                  grep -q "SLOT（常驻保温，跳过" <<<"$out"
check ".env 文件仍在"                  test -f ".agent-worktrees/wt-merged-env/.env"
check_not "mergeonly 不可回收正例"     grep -qE "wt-merged-mergeonly.*可回收" <<<"$out"
check_not "dualbase 不可回收正例"      grep -qE "wt-dualbase.*可回收" <<<"$out"

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
echo "== 3b. 删除前 busy 复扫注入（cwd/fd/rel_argv 语义）"
# 启动时不 busy，但 APPLY 删除前注入
CLEAN_ABS=$(realpath ".agent-worktrees/wt-merged-clean")
out=$(WT_JANITOR_BUSY_INJECT="$CLEAN_ABS" bash "$JANITOR" --apply)
check "pre-delete busy：树保留"        test -d ".agent-worktrees/wt-merged-clean"
check "pre-delete busy：分支保留"      git show-ref -q "refs/heads/merged-clean"
check "pre-delete busy：报告交人工"    grep -q "删除前复扫发现进程引用" <<<"$out"
# fd 语义注入（实现扫 /proc/*/fd；seam 确定性覆盖该路径）
out=$(WT_JANITOR_BUSY_INJECT="$CLEAN_ABS" WT_JANITOR_BUSY_INJECT_MODE=fd bash "$JANITOR" --apply)
check "pre-delete fd inject：树保留"   test -d ".agent-worktrees/wt-merged-clean"
check "pre-delete fd inject：报告BUSY" grep -q "删除前复扫发现进程引用" <<<"$out"
# rel_argv 语义注入
out=$(WT_JANITOR_BUSY_INJECT="$CLEAN_ABS" WT_JANITOR_BUSY_INJECT_MODE=rel_argv bash "$JANITOR" --apply)
check "pre-delete rel_argv：树保留"    test -d ".agent-worktrees/wt-merged-clean"
check "pre-delete rel_argv：报告BUSY"  grep -q "删除前复扫发现进程引用" <<<"$out"

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
check "wt-merged-mergeonly 保留"       test -d ".agent-worktrees/wt-merged-mergeonly"
check "merged-mergeonly 分支保留"      git show-ref -q "refs/heads/merged-mergeonly"
check "unique resolution 仍在"         test -f ".agent-worktrees/wt-merged-mergeonly/conflict-res.txt" || test -f ".agent-worktrees/wt-merged-mergeonly/mergeonly-feat.txt"
check "wt-dualbase 保留"               test -d ".agent-worktrees/wt-dualbase"
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

# ========== 5a. clean-artifacts 删除前 busy 复扫 ==========
echo "== 5a. clean-artifacts 删除前 busy 复扫挡住迟到编译"
# 上一段已经清空 open-idle；重建缓存，再用确定性 seam 模拟初检之后才进入树的进程。
mkdir -p ".agent-worktrees/wt-open-idle/server/target"
echo blob > ".agent-worktrees/wt-open-idle/server/target/blob"
OPEN_IDLE_ABS=$(realpath ".agent-worktrees/wt-open-idle")
out=$(WT_JANITOR_BUSY_INJECT="$OPEN_IDLE_ABS" bash "$JANITOR" --apply --clean-artifacts=0)
check "artifact pre-delete busy：缓存保留" test -f ".agent-worktrees/wt-open-idle/server/target/blob"
check "artifact pre-delete busy：树保留"   test -d ".agent-worktrees/wt-open-idle"
check "artifact pre-delete busy：报告不 clean" grep -q "构建产物删除前复扫发现进程引用" <<<"$out"
check_not "artifact pre-delete busy：不得宣称已清" grep -qE "wt-open-idle.*构建产物已清" <<<"$out"

# 恢复无 busy 路径，确认同一候选随后仍可正常清理。
out=$(bash "$JANITOR" --apply --clean-artifacts=0)
check "artifact busy 解除后缓存已清" test ! -e ".agent-worktrees/wt-open-idle/server/target"

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
export FAKE_GH_MODE=emptyoid
out=$(bash "$JANITOR" --apply)
check "emptyoid：ghfail 树保留"        test -d ".agent-worktrees/wt-merged-ghfail"
check "emptyoid：报告 UNKNOWN"         grep -q "UNKNOWN" <<<"$out"
export FAKE_GH_MODE=nulloid
out=$(bash "$JANITOR" --apply)
check "nulloid：报告 UNKNOWN"          grep -q "UNKNOWN" <<<"$out"
export FAKE_GH_MODE=dualbase
out=$(bash "$JANITOR" --apply)
check "dualbase 模式：树保留"          test -d ".agent-worktrees/wt-dualbase"
check "dualbase 模式：报告 UNKNOWN"    grep -q "UNKNOWN" <<<"$out"
check "dualbase 模式：分支保留"        git show-ref -q "refs/heads/dualbase-head"
export FAKE_GH_MODE=shortoid
out=$(bash "$JANITOR" --apply)
check "shortoid 模式：ghfail 树保留"   test -d ".agent-worktrees/wt-merged-ghfail"
check "shortoid 模式：报告 UNKNOWN"    grep -q "UNKNOWN" <<<"$out"
export FAKE_GH_MODE=staleoid
out=$(bash "$JANITOR" --apply)
check "staleoid 模式：ghfail 树保留"   test -d ".agent-worktrees/wt-merged-ghfail"
check "staleoid 模式：报告 UNKNOWN"    grep -q "UNKNOWN" <<<"$out"
export FAKE_GH_MODE=""
# 分支名触发 multi/nonmain/dualbase（与 FAKE_GH_MODE 解耦）
out=$(bash "$JANITOR")
check "multi-head 报告 UNKNOWN"        grep -qE "wt-multi.*UNKNOWN|UNKNOWN.*wt-multi" <<<"$out" || grep -q "UNKNOWN" <<<"$out"
check "nonmain-merged 报告 UNKNOWN"    grep -q "UNKNOWN" <<<"$out"
check "dualbase-head 报告 UNKNOWN"     grep -q "UNKNOWN" <<<"$out"

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


# ========== 13b. headRefOid 必须绑定本地 tip；branch reuse fail-closed ==========
echo "== 13b. headRefOid/tip 绑定 + branch reuse 反例 + --state all 契约"
# 13b-1: 正常 MERGED 路径返回真实 tip 时可回收（setup_normal_merged + 默认 fake）
setup_normal_merged wt-merged-tipok merged-tipok
TIPOK=$(git rev-parse refs/heads/merged-tipok)
# 直接问 fake gh，确认返回 tip
gh_out=$(gh pr list --head merged-tipok --state all --json number,state,baseRefName,headRefOid)
check "fake gh 返回真实 tip" bash -c 'grep -q "$1" <<<"$2"' _ "$TIPOK" "$gh_out"
out=$(bash "$JANITOR" --apply)
check "tipok 已回收"                   test ! -d ".agent-worktrees/wt-merged-tipok"
check "tipok 分支已删"                 bash -c '! git show-ref -q refs/heads/merged-tipok'

# 13b-2: 短 OID → UNKNOWN，树/分支保留
setup_normal_merged wt-merged-shortoid merged-shortoid
export FAKE_GH_MODE=shortoid
out=$(bash "$JANITOR" --apply)
check "shortoid：树保留"               test -d ".agent-worktrees/wt-merged-shortoid"
check "shortoid：分支保留"             git show-ref -q "refs/heads/merged-shortoid"
check "shortoid：报告 UNKNOWN"         grep -q "UNKNOWN" <<<"$out"
export FAKE_GH_MODE=""

# 13b-3: staleoid 模式（合法 40-hex 但 ≠ tip）→ UNKNOWN
setup_normal_merged wt-merged-staleoid merged-staleoid
export FAKE_GH_MODE=staleoid
out=$(bash "$JANITOR" --apply)
check "staleoid：树保留"               test -d ".agent-worktrees/wt-merged-staleoid"
check "staleoid：分支保留"             git show-ref -q "refs/heads/merged-staleoid"
check "staleoid：报告 UNKNOWN"         grep -q "UNKNOWN" <<<"$out"
export FAKE_GH_MODE=""

# 13b-4: branch reuse 反例
# 历史：同名分支曾 MERGED（patch 已进 main）；本地同名分支被重建为与 main 等价 tip，
# 但尚未开新 PR。fake 对 merged-reuse 固定返回旧 stale OID。
# 即使 git cherry 为空，也必须 UNKNOWN，树和分支均保留。
git checkout -q main
# 先造一个已合入的 feature，模拟“历史 PR 对应 tip”
git worktree add -q -b merged-reuse ".agent-worktrees/wt-merged-reuse" main
echo "reuse-old" > ".agent-worktrees/wt-merged-reuse/reuse-old.txt"
git -C ".agent-worktrees/wt-merged-reuse" add -A
git -C ".agent-worktrees/wt-merged-reuse" commit -qm "reuse old feature"
OLD_TIP=$(git -C ".agent-worktrees/wt-merged-reuse" rev-parse HEAD)
git -C ".agent-worktrees/wt-merged-reuse" push -q origin merged-reuse
git checkout -q main
git merge -q --no-ff -m "merge merged-reuse" merged-reuse >/dev/null
git push -q origin main
git fetch -q origin
# 删除 worktree + 本地分支，模拟历史 PR 已收工；再在 main tip 上重建同名分支
git worktree remove ".agent-worktrees/wt-merged-reuse"
git branch -D merged-reuse >/dev/null
git worktree add -q -b merged-reuse ".agent-worktrees/wt-merged-reuse" main
NEW_TIP=$(git rev-parse refs/heads/merged-reuse)
# 新 tip 与 main 等价（cherry 空），但 ≠ 历史 OLD_TIP；fake 对 merged-reuse 返回 stale≠NEW_TIP
check "reuse 新 tip 不同于旧 tip" bash -c '[[ "$1" != "$2" ]]' _ "$OLD_TIP" "$NEW_TIP"
# cherry 应为空（无 +）
plus_cnt=$(git cherry origin/main refs/heads/merged-reuse | grep -c '^+') || plus_cnt=0
check "reuse cherry 无未合入 +" bash -c '[[ "$1" -eq 0 ]]' _ "$plus_cnt"
out=$(bash "$JANITOR" --apply)
check "reuse：树保留"                  test -d ".agent-worktrees/wt-merged-reuse"
check "reuse：分支保留"                git show-ref -q "refs/heads/merged-reuse"
check "reuse：报告 UNKNOWN"            grep -q "UNKNOWN" <<<"$out"
check_not "reuse：不可回收"            grep -qE "wt-merged-reuse.*可回收|wt-merged-reuse.*已回收" <<<"$out"

# 13b-5: --state all 契约
# a) 生产路径：实现必须传 --state all，且 tip 绑定后可回收
# b) 缺 --state all 时 fake 直接非零（锁契约，防止回归）
setup_normal_merged wt-merged-stateall merged-stateall
# 先备份完整 fake，再装 wrapper（顺序反了会 wrapper→wrapper 死递归）
cp -f "$SANDBOX/bin/gh" "$SANDBOX/bin/gh.real"
cat > "$SANDBOX/bin/gh" <<'WRAPGH'
#!/usr/bin/env bash
has_state_all=0
prev=""
for a in "$@"; do
  if [[ "$prev" == "--state" && "$a" == "all" ]]; then has_state_all=1; fi
  if [[ "$a" == "--state=all" ]]; then has_state_all=1; fi
  prev="$a"
done
if [[ $has_state_all -ne 1 ]]; then
  echo "wrap-gh: missing --state all" >&2
  exit 2
fi
exec "$(dirname "$0")/gh.real" "$@"
WRAPGH
chmod +x "$SANDBOX/bin/gh" "$SANDBOX/bin/gh.real"
out=$(bash "$JANITOR" --apply)
if [[ -d ".agent-worktrees/wt-merged-stateall" ]]; then
  echo "  FAIL: state-all 契约：实现可能未传 --state all 或 tip 对拍失败"
  FAIL=$((FAIL + 1))
else
  echo "  PASS: state-all 契约：带 --state all 且 tip 绑定后已回收"
  PASS=$((PASS + 1))
fi
# 反向：直接调底层 fake，故意省略 --state all → 必须非零
if "$SANDBOX/bin/gh.real" pr list --head merged-stateall --json number,state,baseRefName,headRefOid >/dev/null 2>&1; then
  echo "  FAIL: fake gh 缺 --state all 应非零"
  FAIL=$((FAIL + 1))
else
  echo "  PASS: fake gh 缺 --state all 非零"
  PASS=$((PASS + 1))
fi
# 正向：带 --state all 必须成功（即使分支已删，也至少不能因缺参失败；用 open-idle）
if ! "$SANDBOX/bin/gh.real" pr list --head open-idle --state all --json number,state,baseRefName,headRefOid >/dev/null 2>&1; then
  echo "  FAIL: fake gh 带 --state all 应成功"
  FAIL=$((FAIL + 1))
else
  echo "  PASS: fake gh 带 --state all 成功"
  PASS=$((PASS + 1))
fi
# 还原 gh（去掉 wrapper）
mv -f "$SANDBOX/bin/gh.real" "$SANDBOX/bin/gh"
chmod +x "$SANDBOX/bin/gh"

# ========== 14. 真实 fd 占用：打开树内文件的进程挡住删除 ==========
echo "== 14. 真实 /proc/fd 指向树内 → BUSY"
setup_normal_merged wt-merged-fdhold merged-fdhold
# hold 放在 allowlist cache 内，避免 dirty/unsafe 门挡住 reclaim 路径
mkdir -p ".agent-worktrees/wt-merged-fdhold/server/target"
FDHOLD_FILE=$(realpath ".agent-worktrees/wt-merged-fdhold/server/target")/hold.bin
echo hold > "$FDHOLD_FILE"
# 子进程保持 fd 打开，cwd 在树外（只靠 fd 探测）
( cd /tmp && exec 9<"$FDHOLD_FILE" && sleep 300 ) &
SLEEPER_PID=$!
sleep 0.25
out=$(bash "$JANITOR" --apply)
check "fdhold 树未被回收"              test -d ".agent-worktrees/wt-merged-fdhold"
check "fdhold 报告 BUSY 或删除前复扫"  bash -c 'grep -qE "BUSY|删除前复扫" <<<"$1"' _ "$out"
kill "$SLEEPER_PID" 2>/dev/null || true
wait "$SLEEPER_PID" 2>/dev/null || true
SLEEPER_PID=""
# 释放后应可回收
out=$(bash "$JANITOR" --apply)
check "fd 释放后 fdhold 已回收"        test ! -d ".agent-worktrees/wt-merged-fdhold"
check "fd 释放后分支已删"              bash -c '! git show-ref -q refs/heads/merged-fdhold'

echo "---"
echo "PASS=$PASS FAIL=$FAIL"
[[ $FAIL -eq 0 ]] || exit 1
echo "wt-janitor 契约测试全部通过"
