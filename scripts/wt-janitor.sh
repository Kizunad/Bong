#!/usr/bin/env bash
# worktree janitor —— 巡检并回收遗留 worktree，防止「每个 worktree 一份 target」塞盘复发。
#
# 历史教训：merge/close 后没人收尸的 worktree（各背 16~54G 的 server/target）
# 曾把 444G 的盘塞到 100%（2026-07-17 实测 .agent-worktrees 138G + .claude/worktrees 69G）。
#
# 用法：
#   bash scripts/wt-janitor.sh                             # report-only：列出全部 worktree + PR 状态 + 判定
#   bash scripts/wt-janitor.sh --apply                     # 回收「PR 已 MERGED/CLOSED 且工作区干净」的 worktree
#   bash scripts/wt-janitor.sh --apply --clean-artifacts   # 额外删除「无 PR/OPEN 且 ≥7 天无提交」worktree 的构建产物
#   bash scripts/wt-janitor.sh --apply --clean-artifacts=3 # 同上，闲置阈值改为 3 天
#
# 一切破坏动作都锁在 --apply 之后；--clean-artifacts 不带 --apply 时只报告「待清」。
#
# 永不触碰：
#   - 主 checkout
#   - 常驻编译 slot（.agent-worktrees/slot-*，BugFix 工作流的保温缓存所在）
#   - 有进程正引用其路径（cmdline 含路径，或 cwd 在树内）的 worktree
#   - 工作区不干净、或 git status 查不出来（fail-closed 当脏处理）的 worktree
#   - remove 被 git 拒绝的树（残留非缓存 ignored 文件时不用 --force 硬删，转交人工）
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
      sed -n '2,24p' "$0" | sed 's/^# \{0,1\}//'
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
[[ $HAVE_GH -eq 0 ]] && echo "警告：gh 不可用，PR 状态一律按 UNKNOWN 处理（不会 --apply 回收任何树）" >&2

# 已知可再生的构建缓存目录（相对 worktree 根）；回收前先物理删除，
# 之后 remove 不带 --force——残留的其它 ignored 文件（.env、私有日志等
# dirty 门看不见的东西）会让 git 拒绝 remove，正好转交人工而不是被硬吞
CACHE_DIRS=("server/target" "client/build" "client/.gradle")

reclaimed=0
freed_hint=""

# 进程占用检测：pgrep -f 只能抓 cmdline 字面含路径的进程；
# `cd <树内> && cargo test` 的子进程 argv 不含路径，必须扫 /proc/*/cwd 补上
busy() {
  local p="$1" cwd target
  pgrep -f "$p" >/dev/null 2>&1 && return 0
  for cwd in /proc/[0-9]*/cwd; do
    target=$(readlink "$cwd" 2>/dev/null) || continue
    case "$target" in
      "$p"|"$p"/*) return 0 ;;
    esac
  done
  return 1
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
  if ! dirty=$(git -C "$path" status --porcelain 2>/dev/null); then
    printf '%-70s %-12s %-8s %-8s %s\n' "$path" "-" "${size:--}" "-" "git status 查询失败 → 交人工"
    continue
  fi
  last_commit=$(git -C "$path" log -1 --format=%ct 2>/dev/null || echo "$NOW")
  idle_days=$(( (NOW - last_commit) / 86400 ))

  pr_state="NO_BRANCH"
  if [[ -n "$branch" ]]; then
    if [[ $HAVE_GH -eq 1 ]]; then
      pr_state=$(gh pr list --head "$branch" --state all --json state --jq '.[0].state' 2>/dev/null || true)
      [[ -z "$pr_state" ]] && pr_state="NO_PR"
    else
      pr_state="UNKNOWN"
    fi
  fi

  verdict=""
  case "$pr_state" in
    MERGED|CLOSED)
      if [[ -n "$dirty" ]]; then
        verdict="PR 已 $pr_state 但工作区不干净 → 交人工"
      else
        verdict="可回收（PR 已 $pr_state）"
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
            verdict="已回收（PR $pr_state，本地分支已删）"
            reclaimed=$((reclaimed + 1))
            freed_hint="$freed_hint $size"
          else
            verdict="remove 被拒（残留非缓存 ignored 文件？）→ 交人工"
          fi
        fi
      fi
      ;;
    OPEN)
      verdict="保留（PR OPEN）"
      ;;
    *)
      verdict="无 PR/未知 → 交人工"
      ;;
  esac

  # 构建产物回收：无 PR 或 OPEN 但长期闲置的树，target/build 可再生。
  # 与所有破坏动作一样锁在 --apply 之后；单棵失败不中断巡检
  if [[ $CLEAN_ARTIFACTS -eq 1 && "$verdict" != 已回收* && $idle_days -ge $IDLE_DAYS ]]; then
    if [[ -d "$path/server/target" || -d "$path/client/build" ]]; then
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
