#!/usr/bin/env bash
# worktree janitor —— 巡检并回收遗留 worktree，防止「每个 worktree 一份 target」塞盘复发。
#
# 历史教训：merge/close 后没人收尸的 worktree（各背 16~54G 的 server/target）
# 曾把 444G 的盘塞到 100%（2026-07-17 实测 .agent-worktrees 138G + .claude/worktrees 69G）。
#
# 用法：
#   bash scripts/wt-janitor.sh                             # report-only：列出全部 worktree + PR 状态 + 判定
#   bash scripts/wt-janitor.sh --apply                     # 回收「可安全回收」的 worktree（见下方契约）
#   bash scripts/wt-janitor.sh --apply --clean-artifacts   # 额外删除「工作区干净、OPEN/NO_PR、无非缓存 ignored、≥7 天无提交」worktree 的构建产物
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
#     同一安全门也约束 --clean-artifacts：OPEN/NO_PR 闲置树若含非缓存 ignored，
#     不删 cache、不触碰树/密钥（fail-closed，避免“只清缓存”半破坏路径）。
#   - gh / PR 状态查询失败 → UNKNOWN（与「成功但 0 条 PR → NO_PR」严格区分）；
#     UNKNOWN 既不整树回收，也不 clean-artifacts
#   - CLOSED（未 merge）一律交人工（即使远端 tip 仍在、工作区干净）
#   - 本地分支相对 origin/main 无法证明 patch-equivalent（每个 MERGED 候选都必须
#     成功跑通 `git cherry origin/main refs/heads/$branch` 且无任何 `+`，且
#     `origin/main..branch` 不存在 merge commit——cherry 看不到 branch-only merge
#     的独有 tree/conflict resolution；远端可达性仅作诊断，绝不能单独放行）
#   - PR 状态：先 `gh pr list --head --state all` 拉完整集合（不带 --base，避免
#     隐藏同 head 的 develop PR），再本地要求恰好一条、base=main、且 headRefOid
#     完整 40 位 hex 并精确等于本地 refs/heads/$branch tip（多条/字段异常/OID
#     非法/branch reuse 导致 tip 不一致 → UNKNOWN）
#   - remove 被 git 拒绝、或 remove 后本地分支删除失败的树
#     （后者报告部分完成/交人工，不计完整回收；不用 --force）
#   - APPLY 真正删除前对当前候选重新扫描 /proc：命中新进程则保留并转人工（防 TOCTOU）
#
# 契约测试：bash scripts/tests/wt_janitor_test.sh（隔离沙箱仓库，锁死全部破坏性判定）
set -euo pipefail

APPLY=0
CLEAN_ARTIFACTS=0
IDLE_DAYS=7

print_help() {
  # 输出 shebang 之后、首个非注释行之前的顶部注释块（稳健，不绑行号）
  local line
  local seen_shebang=0
  while IFS= read -r line || [[ -n "$line" ]]; do
    if [[ $seen_shebang -eq 0 ]]; then
      if [[ "$line" == \#!* ]]; then
        seen_shebang=1
      fi
      continue
    fi
    case "$line" in
      \#*)
        # 去掉前导 "# " 或单独的 "#"
        if [[ "$line" == "# " ]]; then
          printf '\n'
        elif [[ "$line" == \#\ * ]]; then
          printf '%s\n' "${line#\# }"
        else
          printf '%s\n' "${line#\#}"
        fi
        ;;
      *)
        break
        ;;
    esac
  done < "$0"
}

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
      print_help
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

# 已知可再生的构建缓存目录（相对 worktree 根）——单一真相源。
# 回收前：① 主动枚举 ignored/untracked，只允许这些白名单路径；② 再物理删除缓存；
# ③ 最后不带 --force 调用 worktree remove。安全门在脚本侧 fail-closed，
# 不依赖 git remove 是否拒绝（Git 2.47 会对含 ignored 的干净树直接删掉）。
CACHE_DIRS=("server/target" "client/build" "client/.gradle")

reclaimed=0
partial=0
freed_hint=""

# 进程占用快照：整轮巡检只扫一次 /proc，避免「每 worktree 全扫」在进程多时把
# 巡检拖到数十秒（契约测试会连跑多次 janitor，累计超时）。
# - cwd：cd 进树内的进程（argv 可能不含路径）
# - cmdline：字面含路径的进程（固定字符串匹配，不用 pgrep -f 正则）
# APPLY 真正删除前会对当前候选再扫一次（防启动快照与删除之间的 TOCTOU）。
BUSY_CWDS=()
BUSY_CMDS=()
# 进程可能在枚举中途退出：所有 /proc 读都 fail-soft，不让 set -e 中断巡检。
# 用 readlink -f 批量 + mapfile 降子 shell；cmdline 用 bash 内建读代替 tr 管道。
_snapshot_busy() {
  local _cwd _t _cmdf _c _parts
  BUSY_CWDS=()
  BUSY_CMDS=()
  for _cwd in /proc/[0-9]*/cwd; do
    _t=$(readlink "$_cwd" 2>/dev/null) || continue
    [[ -n "$_t" ]] && BUSY_CWDS+=("$_t")
  done
  for _cmdf in /proc/[0-9]*/cmdline; do
    [[ -r "$_cmdf" ]] || continue
    # bash 读二进制：用 mapfile -d '' 拆 NUL 字段再拼空格
    # 注意：set -e 下 ((0)) 会非零退出，空 cmdline 必须用 [[ ]] 判断
    _c=""
    _parts=()
    if mapfile -d '' -t _parts < "$_cmdf" 2>/dev/null; then
      if [[ ${#_parts[@]} -gt 0 ]]; then
        local IFS=' '
        _c="${_parts[*]}"
      fi
    fi
    [[ -n "${_c// /}" ]] && BUSY_CMDS+=("$_c")
  done
  return 0
}
_snapshot_busy

path_is_busy_against() {
  # path_is_busy_against <path> <cwd...> -- <cmdline...>
  # 分隔符 "--"：左侧 cwd 列表，右侧 cmdline 列表
  local p="$1"; shift
  local mode=cwd t
  for t in "$@"; do
    if [[ "$t" == "--" ]]; then
      mode=cmd
      continue
    fi
    if [[ "$mode" == "cwd" ]]; then
      case "$t" in
        "$p"|"$p"/*) return 0 ;;
      esac
    else
      [[ "$t" == *"$p"* ]] && return 0
    fi
  done
  return 1
}

busy() {
  local p="$1"
  path_is_busy_against "$p" "${BUSY_CWDS[@]+"${BUSY_CWDS[@]}"}" -- "${BUSY_CMDS[@]+"${BUSY_CMDS[@]}"}"
}

# 删除前对当前候选重扫 /proc（不复用启动快照）。
# 测试注入 seam：WT_JANITOR_BUSY_INJECT=<绝对路径> 时，对该路径强制视为 busy。
busy_live() {
  # 删除前再扫 /proc：cwd / open-fd / cmdline（含相对 argv fail-closed）。
  # 所有 /proc 读 fail-soft；find 扫 /proc 常因权限返回非零，必须 || true 后再判命中，
  # 否则 set -o pipefail 会把「已命中但 find RC=1」当成未命中。
  local p="$1" t c pid cwd_path arg resolved base fd_hit
  if [[ -n "${WT_JANITOR_BUSY_INJECT:-}" ]]; then
    case "${WT_JANITOR_BUSY_INJECT}" in
      "$p"|"$p"/*) return 0 ;;
    esac
  fi

  # 1) cwd 在树内
  for t in /proc/[0-9]*/cwd; do
    c=$(readlink "$t" 2>/dev/null) || continue
    case "$c" in
      "$p"|"$p"/*) return 0 ;;
    esac
  done

  # 2) open fd 指向树内：find -regex 避免 ARG_MAX；-print -quit 首命中即停
  fd_hit=$(find /proc -regextype posix-extended \
    -regex '/proc/[0-9]+/fd/[0-9]+' \
    \( -lname "$p" -o -lname "$p/*" \) \
    -print -quit 2>/dev/null || true)
  [[ -n "$fd_hit" ]] && return 0

  # 3) cmdline 绝对路径；相对 argv 解析到树内，或相对片段含 worktree basename
  base="${p##*/}"
  local _parts
  for t in /proc/[0-9]*/cmdline; do
    [[ -r "$t" ]] || continue
    pid="${t#/proc/}"; pid="${pid%%/*}"
    _parts=()
    mapfile -d '' -t _parts < "$t" 2>/dev/null || continue
    [[ ${#_parts[@]} -gt 0 ]] || continue
    local IFS=' '
    c="${_parts[*]}"
    [[ -n "${c// /}" && "$c" == *"$p"* ]] && return 0
    cwd_path=$(readlink "/proc/$pid/cwd" 2>/dev/null) || cwd_path=""
    [[ -n "$cwd_path" ]] || continue
    case "$cwd_path" in
      "$p"|"$p"/*) return 0 ;;
    esac
    for arg in "${_parts[@]}"; do
      [[ -z "$arg" || "$arg" == -* || "$arg" == /* ]] && continue
      if [[ "$arg" == */* || "$arg" == *.* ]]; then
        resolved=$(realpath -m -- "$cwd_path/$arg" 2>/dev/null) || resolved=""
        if [[ -n "$resolved" ]]; then
          case "$resolved" in
            "$p"|"$p"/*) return 0 ;;
          esac
        fi
        if [[ -n "$base" && "$arg" == *"$base"* ]]; then
          return 0
        fi
      fi
    done
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

# 候选树是否存在任一 CACHE_DIRS 目录（单一真相源，禁止散落硬编码）
cache_dir_present() {
  local p="$1" d
  for d in "${CACHE_DIRS[@]}"; do
    [[ -d "$p/$d" ]] && return 0
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
# 每个 MERGED 自动回收候选都必须无条件调用本函数；远端可达性不得绕过。
branch_patches_in_main() {
  # 0=patch 已在 main 且无 branch-only merge；1=有未合入内容；2=查询失败
  # git cherry 只比较非 merge 提交的 patch equivalence，看不到 branch-only merge
  # commit 的独有 tree（含 conflict resolution）。故 cherry 无 + 之后仍须
  # 检查 origin/main..branch 是否存在 merge commit —— 有则 fail-closed。
  local br="$1"
  local out line merges
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
  # branch-only merge commits：cherry 静默忽略，但 tree 可能含未进 main 的决议
  if ! merges=$(git rev-list --merges "origin/main..refs/heads/$br" 2>/dev/null); then
    return 2
  fi
  if [[ -n "$merges" ]]; then
    return 1
  fi
  return 0
}

# 解析 gh pr list JSON（先 exact head + state=all 全量，不带 --base）。
# stdout：单一明确状态（OPEN/MERGED/CLOSED）或 NO_PR/UNKNOWN
# 规则：
#   - 查询失败 / 非数组 / 字段异常 → UNKNOWN
#   - 0 条 → NO_PR
#   - >1 条 → UNKNOWN（含同 head 多 base；真实 gh --base main 会隐藏 develop）
#   - 1 条：baseRefName 必须为 main；state 必须为 OPEN|MERGED|CLOSED；否则 UNKNOWN
#   - 唯一 PR 的 headRefOid 必须是完整 40 位 hex，且与本地 refs/heads/$branch tip
#     精确相等（防同名 branch reuse：历史 MERGED PR 不得授权删除新 tip）
resolve_pr_state() {
  local branch="$1"
  local raw rc=0
  # 故意不带 --base：必须拉回该 head 的完整 PR 集合，再本地要求唯一且 base=main。
  # --state all 强制：真实 gh 默认只列 OPEN，漏掉会把 MERGED 误判成 NO_PR。
  raw=$(gh pr list --head "$branch" --state all \
    --json number,state,baseRefName,headRefOid 2>/dev/null) || rc=$?
  if [[ $rc -ne 0 ]]; then
    echo "UNKNOWN"
    return 0
  fi
  if ! printf '%s' "$raw" | jq -e 'type == "array"' >/dev/null 2>&1; then
    echo "UNKNOWN"
    return 0
  fi
  local count
  count=$(printf '%s' "$raw" | jq 'length')
  if [[ "$count" == "0" ]]; then
    echo "NO_PR"
    return 0
  fi
  # 任一结果字段缺失/null/类型不对 → UNKNOWN
  local bad
  bad=$(printf '%s' "$raw" | jq '
    map(select(
      (.number == null) or
      ((.number|type) != "number" and (.number|type) != "string") or
      ((.number|tostring) == "") or
      (.state == null) or (.state|type) != "string" or .state == "" or
      (.baseRefName == null) or (.baseRefName|type) != "string" or .baseRefName == "" or
      (.headRefOid == null) or (.headRefOid|type) != "string" or .headRefOid == ""
    )) | length') || bad="err"
  if [[ "$bad" == "err" || "$bad" != "0" ]]; then
    echo "UNKNOWN"
    return 0
  fi
  if [[ "$count" != "1" ]]; then
    # 多结果：含 OPEN、或多 base（main+develop）一律 UNKNOWN
    echo "UNKNOWN"
    return 0
  fi
  local state base number head_oid branch_tip
  state=$(printf '%s' "$raw" | jq -r '.[0].state')
  base=$(printf '%s' "$raw" | jq -r '.[0].baseRefName')
  number=$(printf '%s' "$raw" | jq -r '.[0].number')
  head_oid=$(printf '%s' "$raw" | jq -r '.[0].headRefOid')
  if [[ -z "$state" || -z "$base" || -z "$number" || -z "$head_oid" \
     || "$state" == "null" || "$base" == "null" || "$number" == "null" || "$head_oid" == "null" ]]; then
    echo "UNKNOWN"
    return 0
  fi
  if [[ "$base" != "main" ]]; then
    echo "UNKNOWN"
    return 0
  fi
  # headRefOid 必须是完整 Git OID，并与待判定本地分支 tip 精确绑定。
  # 短 OID / 杂质 / 解析失败 / tip 不一致 → UNKNOWN（fail-closed，防 branch reuse）。
  head_oid=$(printf '%s' "$head_oid" | tr 'A-F' 'a-f')
  if [[ ! "$head_oid" =~ ^[0-9a-f]{40}$ ]]; then
    echo "UNKNOWN"
    return 0
  fi
  if ! branch_tip=$(git rev-parse --verify -q "refs/heads/$branch^{commit}" 2>/dev/null); then
    echo "UNKNOWN"
    return 0
  fi
  branch_tip=$(printf '%s' "$branch_tip" | tr 'A-F' 'a-f')
  if [[ "$head_oid" != "$branch_tip" ]]; then
    echo "UNKNOWN"
    return 0
  fi
  case "$state" in
    OPEN|MERGED|CLOSED)
      echo "$state"
      ;;
    *)
      echo "UNKNOWN"
      ;;
  esac
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

  # 有进程正引用该路径 → 一律跳过（启动快照）
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
      pr_state=$(resolve_pr_state "$branch")
    else
      pr_state="UNKNOWN"
    fi
  fi

  # 远端可达性仅诊断：不得单独放行回收。每个 MERGED 候选都必须过 cherry。
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
      elif [[ -z "$branch" ]]; then
        verdict="PR 已 MERGED 但无本地分支名 → 交人工"
      else
        # 无条件对 origin/main 做 patch-equivalence；远端可达不能放行
        cherry_rc=0
        branch_patches_in_main "$branch" || cherry_rc=$?
        if [[ $cherry_rc -eq 0 ]]; then
          can_reclaim=1
          if [[ "$unpushed" == "0" ]]; then
            verdict="可回收（PR 已 MERGED，patch 已等价合入 origin/main）"
          elif [[ "$unpushed" == "ERR" ]]; then
            verdict="可回收（PR 已 MERGED，patch 已等价合入 origin/main；远端可达性查询失败仅诊断）"
          else
            verdict="可回收（PR 已 MERGED，squash/patch 已等价合入 origin/main）"
          fi
        elif [[ $cherry_rc -eq 1 ]]; then
          # 含：非 merge patch 未进 main，或 branch-only merge commit（独有 tree）
          if [[ "$unpushed" == "0" ]]; then
            verdict="PR 已 MERGED 但本地分支有未合入 origin/main 的 patch/merge（远端可达不能证明已进 main）→ 交人工"
          else
            verdict="PR 已 MERGED 但本地分支有未合入 patch/merge（$unpushed 个远端不可达提交）→ 交人工"
          fi
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
      # 删除前对当前候选重扫 /proc，防止启动快照与删除之间的 TOCTOU
      if busy_live "$path_real"; then
        verdict="BUSY（删除前复扫发现进程引用）→ 交人工"
      else
        [[ $locked -eq 1 ]] && git worktree unlock "$path" 2>/dev/null || true
        cache_ok=1
        for d in "${CACHE_DIRS[@]}"; do
          rm -rf "${path:?}/$d" 2>/dev/null || cache_ok=0
        done
        if [[ $cache_ok -eq 0 ]]; then
          verdict="缓存清理失败（权限/挂载问题？）→ 交人工"
        elif git worktree remove "$path" 2>/dev/null; then
          # can_reclaim 仅在非空 branch + MERGED + cherry 通过时置 1；此处 branch 必非空
          branch_del_ok=1
          if ! git branch -D "$branch" >/dev/null 2>&1; then
            branch_del_ok=0
          fi
          if [[ $branch_del_ok -eq 1 ]] && ! git show-ref --verify --quiet "refs/heads/$branch"; then
            verdict="已回收（PR MERGED，本地分支已删）"
            reclaimed=$((reclaimed + 1))
            freed_hint="$freed_hint $size"
          else
            # 树已移除但本地分支仍在：部分完成，不计完整回收
            verdict="已移除 worktree 但本地分支删除失败（$branch 仍在）→ 交人工"
            partial=$((partial + 1))
          fi
        else
          verdict="remove 被拒 → 交人工"
        fi
      fi
    fi
  fi

  # 构建产物回收：只作用于「工作区干净」且「OPEN 或 NO_PR」的长期闲置树，
  # 且 ignored/untracked 安全门通过（unsafe_rc==0）。
  # 存在 .env 等非白名单 ignored 时 fail-closed：树/密钥/缓存一律不碰。
  # （MERGED 干净树走上面的整树回收；脏树/UNKNOWN/CLOSED/NO_BRANCH 一律不碰）。
  # 与所有破坏动作一样锁在 --apply 之后。默认 IDLE_DAYS=7。
  if [[ $CLEAN_ARTIFACTS -eq 1 && -z "$dirty" && ( "$pr_state" == "OPEN" || "$pr_state" == "NO_PR" ) && $idle_days -ge $IDLE_DAYS ]]; then
    if [[ $unsafe_rc -eq 2 ]]; then
      verdict="$verdict + ignored 查询失败 → 不 clean（交人工）"
    elif [[ $unsafe_rc -ne 0 ]]; then
      verdict="$verdict + 存在非缓存 ignored/untracked（$unsafe_reason）→ 不 clean（交人工）"
    elif cache_dir_present "$path"; then
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
  echo "已回收 $reclaimed 个 worktree（各自大小:${freed_hint:- 无}）；部分完成 $partial 个；已执行 git worktree prune"
fi

echo "---"
df -h "$ROOT" | tail -1
