#!/usr/bin/env bash
# worktree janitor —— 巡检并回收遗留 worktree，防止「每个 worktree 一份 target」塞盘复发。
#
# 历史教训：merge/close 后没人收尸的 worktree（各背 16~54G 的 server/target）
# 曾把 444G 的盘塞到 100%（2026-07-17 实测 .agent-worktrees 138G + .claude/worktrees 69G）。
#
# 用法：
#   bash scripts/wt-janitor.sh                             # report-only：列出全部 worktree + PR 状态 + 判定
#   bash scripts/wt-janitor.sh --apply                     # 回收「可安全回收」的 worktree（见下方契约）
#   bash scripts/wt-janitor.sh --apply --clean-artifacts   # 额外删除「工作区干净、OPEN/NO_PR、≥7 天无提交」worktree 的构建产物
#   bash scripts/wt-janitor.sh --apply --clean-artifacts=3 # 同上，闲置阈值改为 3 天
#
# 一切破坏动作都锁在 --apply 之后；--clean-artifacts 不带 --apply 时只报告「待清」。
#
# 安全优先于自动回收率：任何查询异常一律 fail-closed 交人工。
#
# 永不触碰 / 不自动回收：
#   - 主 checkout
#   - 常驻编译 slot（.agent-worktrees/slot-*，BugFix 工作流的保温缓存所在）
#   - 有进程正引用其路径（cmdline 含路径，或 cwd 在树内）的 worktree
#   - 工作区不干净（含 non-ignored untracked）、或 git status 查不出来的 worktree
#   - 存在「非 allowlist 缓存」的 ignored / untracked 路径（尤其 .env、私有日志）。
#     普通 `git status --porcelain` 看不见 ignored；Git 2.47 下不带 --force 的
#     `git worktree remove` 仍会成功并静默删掉这些文件——因此脚本在回收前主动枚举
#     ignored，只允许 CACHE_DIRS 白名单，绝不依赖 remove 拒绝。
#   - gh / PR 状态查询失败 → UNKNOWN（与「成功但 0 条 PR → NO_PR」严格区分）；
#     UNKNOWN 既不整树回收，也不 clean-artifacts
#   - CLOSED（未 merge）一律交人工（即使远端 tip 仍在、工作区干净）
#   - 本地分支存在远端不可达且无法证明已 patch-equivalent 合入 origin/main 的提交
#     （squash-merge 后远端 branch 删除时，原 PR commits 的 SHA 不再可达；
#      用 `git cherry origin/main <branch>`：出现任意 `+` = 有未合入 patch → 交人工；
#      全为 `-` 或空 + PR=MERGED + 无额外本地提交 = 可回收）
#   - remove 被 git 拒绝的树（转交人工，本轮继续处理其它树；不用 --force）
#
# 契约测试：bash scripts/tests/wt_janitor_test.sh（隔离沙箱仓库，锁死全部破坏性判定）
set -euo pipefail

APPLY=0
CLEAN_ARTIFACTS=0
IDLE_DAYS=7

for arg in "$@"; do
  case "$arg" in
    --apply) APPLY=1 ;;
    --clean-artifacts) CLEAN_ARTIFACTS=1 ;;
    --clean-artifacts=*)
      CLEAN_ARTIFACTS=1
      IDLE_DAYS="${arg#*=}"
      [[ "$IDLE_DAYS" =~ ^[0-9]+$ ]] || { echo "非法闲置天数: $IDLE_DAYS" >&2; exit 2; }
      ;;
    -h|--help)
      sed -n '2,40p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *) echo "未知参数: $arg（支持 --apply / --clean-artifacts[=天数]）" >&2; exit 2 ;;
  esac
done

# 主仓根 = git 公共目录的上一级（从任意 worktree 内运行都指向主 checkout，
# 用 --show-toplevel 会误把当前 worktree 当主仓、把主 checkout 当回收候选）。
# realpath 归一化：仓库经符号链接到达时，字符串比对不能作为「不碰主 checkout」的防线
ROOT=$(realpath "$(dirname "$(git rev-parse --git-common-dir)")")
cd "$ROOT"
NOW=$(date +%s)
HAVE_GH=1
command -v gh >/dev/null 2>&1 || HAVE_GH=0
[[ $HAVE_GH -eq 0 ]] && echo "警告：gh 不可用，PR 状态一律按 UNKNOWN 处理（不会 --apply 回收任何树，也不会 clean-artifacts）" >&2

# 已知可再生的构建缓存目录（相对 worktree 根）。
# 回收前：① 主动枚举 ignored/untracked，只允许这些白名单路径；② 再物理删除缓存；
# ③ 最后不带 --force 调用 worktree remove。安全门在脚本侧 fail-closed，
# 不依赖 git remove 是否拒绝（Git 2.47 会对含 ignored 的干净树直接删掉）。
CACHE_DIRS=("server/target" "client/build" "client/.gradle")

reclaimed=0
freed_hint=""

# 进程占用快照：整轮巡检只扫一次 /proc，避免「每 worktree 全扫」在进程多时把
# 巡检拖到数十秒（契约测试会连跑多次 janitor，累计超时）。
# - cwd：cd 进树内的进程（argv 可能不含路径）
# - cmdline：字面含路径的进程（固定字符串匹配，不用 pgrep -f 正则）
BUSY_CWDS=()
BUSY_CMDS=()
# 进程可能在枚举中途退出：所有 /proc 读都 fail-soft，不让 set -e 中断巡检
for _cwd in /proc/[0-9]*/cwd; do
  _t=$(readlink "$_cwd" 2>/dev/null) || continue
  [[ -n "$_t" ]] && BUSY_CWDS+=("$_t")
done
for _cmdf in /proc/[0-9]*/cmdline; do
  [[ -r "$_cmdf" ]] || continue
  # tr 把 NUL 变空格；空 cmdline（僵尸等）跳过
  _c=$(tr '\0' ' ' < "$_cmdf" 2>/dev/null || true)
  [[ -n "${_c// /}" ]] && BUSY_CMDS+=("$_c")
done

busy() {
  local p="$1" t c
  for t in "${BUSY_CWDS[@]+"${BUSY_CWDS[@]}"}"; do
    case "$t" in
      "$p"|"$p"/*) return 0 ;;
    esac
  done
  for c in "${BUSY_CMDS[@]+"${BUSY_CMDS[@]}"}"; do
    # 固定子串，避免 pgrep -f 把路径当正则（. 通配）
    [[ "$c" == *"$p"* ]] && return 0
  done
  return 1
}

# 路径是否落在 CACHE_DIRS 白名单（精确匹配或子路径）
is_allowlisted_cache() {
  local rel="${1#./}"
  rel="${rel%/}"
  local d
  for d in "${CACHE_DIRS[@]}"; do
    if [[ "$rel" == "$d" || "$rel" == "$d"/* ]]; then
      return 0
    fi
  done
  return 1
}

# 枚举 worktree 内 ignored + untracked；若存在非白名单项则 fail-closed。
# 返回：0=仅白名单缓存或空；1=存在不安全项（stdout 写首个路径）；2=查询失败
unsafe_non_cache_paths() {
  local p="$1"
  local listing item rel
  # --untracked-files=all：展开目录，避免只看到顶层 dir 名
  # --ignored=matching：列出被 ignore 规则命中的路径
  if ! listing=$(git -C "$p" status --porcelain=v1 --untracked-files=all --ignored=matching 2>/dev/null); then
    echo "git status --ignored 查询失败"
    return 2
  fi
  while IFS= read -r item; do
    [[ -z "$item" ]] && continue
    # porcelain: XY PATH；ignored 前缀 "!! "，untracked "?? "
    case "$item" in
      "!! "*|"?? "*)
        rel="${item:3}"
        rel="${rel%% -> *}"
        rel="${rel#\"}"
        rel="${rel%\"}"
        if ! is_allowlisted_cache "$rel"; then
          echo "$rel"
          return 1
        fi
        ;;
      *)
        # 其它状态（M/A/D/...）属于 dirty，由调用方用无 --ignored 的 status 处理
        ;;
    esac
  done <<<"$listing"
  return 0
}

# MERGED 且本地相对 origin/main 无未合入 patch（squash 友好）
# 0=可视为内容已在 main；1=有未合入 patch；2=查询失败
branch_patches_in_main() {
  local br="$1"
  local out line
  if ! git rev-parse --verify -q "refs/remotes/origin/main" >/dev/null; then
    return 2
  fi
  if ! out=$(git cherry "origin/main" "refs/heads/$br" 2>/dev/null); then
    return 2
  fi
  while IFS= read -r line; do
    [[ -z "$line" ]] && continue
    case "$line" in
      +*) return 1 ;;
      -*) ;;
      *) return 2 ;;
    esac
  done <<<"$out"
  return 0
}

# 解析 git worktree list --porcelain 的块结构
current_path=""
current_branch=""
current_locked=0
paths=()
branches=()
lockeds=()
while IFS= read -r line; do
  case "$line" in
    worktree\ *)
      current_path="${line#worktree }"
      current_branch=""
      current_locked=0
      ;;
    branch\ *)
      current_branch="${line#branch refs/heads/}"
      ;;
    locked*)
      current_locked=1
      ;;
    "")
      if [[ -n "$current_path" ]]; then
        paths+=("$current_path")
        branches+=("$current_branch")
        lockeds+=("$current_locked")
      fi
      current_path=""
      ;;
  esac
done < <(git worktree list --porcelain; echo)

printf '%-70s %-12s %-8s %-8s %s\n' "WORKTREE" "PR" "SIZE" "IDLE(d)" "判定"

for i in "${!paths[@]}"; do
  path="${paths[$i]}"
  branch="${branches[$i]}"
  locked="${lockeds[$i]}"
  path_real=$(realpath -m "$path" 2>/dev/null) || path_real="$path"

  # 跳过主 checkout 与常驻 slot
  [[ "$path_real" == "$ROOT" ]] && continue
  case "$path_real" in
    "$ROOT"/.agent-worktrees/slot-*)
      # detached = 空闲，检出着分支 = 被该任务占用（BugFix slot 所有权语义）
      slot_state="空闲(detached)"
      [[ -n "$branch" ]] && slot_state="被占用: $branch"
      printf '%-70s %-12s %-8s %-8s %s\n' "$path" "-" "-" "-" "SLOT（常驻保温，跳过；$slot_state）"
      continue
      ;;
  esac

  if [[ ! -d "$path" ]]; then
    printf '%-70s %-12s %-8s %-8s %s\n' "$path" "-" "-" "-" "目录已消失（prune 可清注册项）"
    continue
  fi

  # 有进程正引用该路径 → 一律跳过
  if busy "$path_real"; then
    printf '%-70s %-12s %-8s %-8s %s\n' "$path" "-" "-" "-" "BUSY（有进程引用，跳过）"
    continue
  fi

  # size 仅用于报告展示，du 失败显示 "?" 而非静默空值；失败不中断巡检
  size=$(du -sh "$path" 2>/dev/null | cut -f1) || size="?"

  # dirty 判定 fail-closed：git status 本身失败（index.lock 被占/元数据损坏）
  # 与「确认干净」必须区分，查不出来就当脏交人工，绝不 fail-open 放行删除。
  # 不接 head：set -o pipefail 下 head 提前关管道会让 git status 吃 SIGPIPE(141)
  # 此处不含 --ignored：non-ignored untracked / 修改都会让 dirty 非空。
  if ! dirty=$(git -C "$path" status --porcelain=v1 --untracked-files=all 2>/dev/null); then
    printf '%-70s %-12s %-8s %-8s %s\n' "$path" "-" "${size:--}" "-" "git status 查询失败 → 交人工"
    continue
  fi
  last_commit=$(git -C "$path" log -1 --format=%ct 2>/dev/null || echo "$NOW")
  idle_days=$(( (NOW - last_commit) / 86400 ))

  pr_state="NO_BRANCH"
  if [[ -n "$branch" ]]; then
    if [[ $HAVE_GH -eq 1 ]]; then
      # 区分：命令失败 → UNKNOWN；成功但 0 条 → NO_PR。禁止 || true 吞失败。
      pr_out=""
      pr_rc=0
      pr_out=$(gh pr list --head "$branch" --state all --json state --jq '.[0].state // empty' 2>/dev/null) || pr_rc=$?
      if [[ $pr_rc -ne 0 ]]; then
        pr_state="UNKNOWN"
      elif [[ -z "$pr_out" ]]; then
        pr_state="NO_PR"
      else
        pr_state="$pr_out"
      fi
    else
      pr_state="UNKNOWN"
    fi
  fi

  # 未推送提交守卫 + squash 等价判定：
  # - unpushed==0：本地提交均被某远端 ref 覆盖 → 安全
  # - unpushed>0 且 PR=MERGED：用 git cherry origin/main 判断 patch 是否已在 main
  #   （squash 改写 SHA 后 rev-list --not --remotes 会永久 >0；cherry 的 `-` = patch 已在）
  # - 任意 `+` / 查询失败 / origin/main 缺失 → 交人工
  unpushed="0"
  if [[ -n "$branch" ]]; then
    unpushed=$(git rev-list --count "refs/heads/$branch" --not --remotes 2>/dev/null) || unpushed="ERR"
  fi

  # ignored/untracked 安全门（独立于 dirty）：只允许 CACHE_DIRS
  unsafe_reason=""
  unsafe_rc=0
  unsafe_reason=$(unsafe_non_cache_paths "$path") || unsafe_rc=$?

  verdict=""
  can_reclaim=0
  case "$pr_state" in
    MERGED)
      if [[ -n "$dirty" ]]; then
        verdict="PR 已 MERGED 但工作区不干净 → 交人工"
      elif [[ $unsafe_rc -eq 2 ]]; then
        verdict="ignored 查询失败 → 交人工"
      elif [[ $unsafe_rc -ne 0 ]]; then
        verdict="PR 已 MERGED 但存在非缓存 ignored/untracked（$unsafe_reason）→ 交人工"
      elif [[ "$unpushed" == "ERR" ]]; then
        verdict="提交可达性查询失败 → 交人工"
      elif [[ "$unpushed" == "0" ]]; then
        can_reclaim=1
        verdict="可回收（PR 已 MERGED，远端可达）"
      else
        cherry_rc=0
        branch_patches_in_main "$branch" || cherry_rc=$?
        if [[ $cherry_rc -eq 0 ]]; then
          can_reclaim=1
          verdict="可回收（PR 已 MERGED，squash/patch 已等价合入 origin/main）"
        elif [[ $cherry_rc -eq 1 ]]; then
          verdict="PR 已 MERGED 但本地分支有未合入 patch（$unpushed 个远端不可达提交）→ 交人工"
        else
          verdict="squash 等价判定失败 → 交人工"
        fi
      fi
      ;;
    CLOSED)
      # CLOSED 未 merge：一律人工。远端 tip 是否可恢复由人判断，脚本不自动删。
      verdict="PR 已 CLOSED（未 merge）→ 交人工"
      ;;
    OPEN)
      verdict="保留（PR OPEN）"
      ;;
    UNKNOWN)
      verdict="PR 状态 UNKNOWN → 交人工（不回收/不 clean）"
      ;;
    NO_PR)
      verdict="无 PR → 交人工"
      ;;
    *)
      verdict="无 PR/未知 → 交人工"
      ;;
  esac

  if [[ $can_reclaim -eq 1 ]]; then
    if [[ $APPLY -eq 1 ]]; then
      [[ $locked -eq 1 ]] && git worktree unlock "$path" 2>/dev/null || true
      cache_ok=1
      for d in "${CACHE_DIRS[@]}"; do
        rm -rf "${path:?}/$d" 2>/dev/null || cache_ok=0
      done
      if [[ $cache_ok -eq 0 ]]; then
        verdict="缓存清理失败（权限/挂载问题？）→ 交人工"
      elif git worktree remove "$path" 2>/dev/null; then
        [[ -n "$branch" ]] && git branch -D "$branch" >/dev/null 2>&1 || true
        verdict="已回收（PR MERGED，本地分支已删）"
        reclaimed=$((reclaimed + 1))
        freed_hint="$freed_hint $size"
      else
        verdict="remove 被拒 → 交人工"
      fi
    fi
  fi

  # 构建产物回收：只作用于「工作区干净」且「OPEN 或 NO_PR」的长期闲置树
  # （MERGED 干净树走上面的整树回收；脏树/UNKNOWN/CLOSED/NO_BRANCH 一律不碰）。
  # 与所有破坏动作一样锁在 --apply 之后。默认 IDLE_DAYS=7。
  if [[ $CLEAN_ARTIFACTS -eq 1 && -z "$dirty" && ( "$pr_state" == "OPEN" || "$pr_state" == "NO_PR" ) && $idle_days -ge $IDLE_DAYS ]]; then
    if [[ -d "$path/server/target" || -d "$path/client/build" || -d "$path/client/.gradle" ]]; then
      if [[ $APPLY -eq 1 ]]; then
        artifacts_ok=1
        for d in "${CACHE_DIRS[@]}"; do
          rm -rf "${path:?}/$d" 2>/dev/null || artifacts_ok=0
        done
        if [[ $artifacts_ok -eq 1 ]]; then
          verdict="$verdict + 构建产物已清（闲置 ${idle_days}d ≥ ${IDLE_DAYS}d）"
        else
          verdict="$verdict + 构建产物清理失败 → 交人工"
        fi
      else
        verdict="$verdict + 构建产物待清（闲置 ${idle_days}d，加 --apply 生效）"
      fi
    fi
  fi

  printf '%-70s %-12s %-8s %-8s %s\n' "$path" "$pr_state" "${size:--}" "$idle_days" "$verdict"
done

if [[ $APPLY -eq 1 ]]; then
  git worktree prune
  echo "---"
  echo "已回收 $reclaimed 个 worktree（各自大小:${freed_hint:- 无}）；已执行 git worktree prune"
fi

echo "---"
df -h "$ROOT" | tail -1
