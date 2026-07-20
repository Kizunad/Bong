#!/usr/bin/env bash
# slot_registry —— BugFix 常驻 slot 的可观察原子占用/释放契约。
#
# 背景：仅靠「detached HEAD = 空闲」不够——两个 agent 可同时观察到同一 detached
# slot，后到的 checkout 会抢走前一个工作区。本脚本用 mkdir 原子创建 reservation
# 目录作为唯一占用权威，并记录 task/branch/claim_sha/created_local_branch。
#
# 布局（相对仓库根）：
#   .agent-worktrees/.slot-registry/
#     capacity                 # 正整数：允许同时 reserved+occupied 的上限
#     slot-<k>.lock/           # 原子占用目录（mkdir 成功 = 持有）
#       task_id
#       branch
#       claim_sha
#       agent_id
#       state                  # reserved | occupied | blocked_frozen
#       created_local_branch   # true|false（仅本轮新建才允许失败回滚删除）
#       reserved_at
#
# 用法（均在仓库根或任意 worktree 内执行，自动定位主仓根）：
#   bash scripts/slot_registry.sh init [--max N]
#   bash scripts/slot_registry.sh acquire --slot slot-1 --task T --branch B --claim-sha S --agent A
#   bash scripts/slot_registry.sh mark-created-local --slot slot-1 --task T --value true|false
#   bash scripts/slot_registry.sh occupy --slot slot-1 --task T
#   bash scripts/slot_registry.sh freeze-blocked --slot slot-1 --task T
#   bash scripts/slot_registry.sh release --slot slot-1 --task T
#   bash scripts/slot_registry.sh rollback --slot slot-1 --task T
#       # 失败回滚：detach 语义由调用方执行；本命令仅清 registry。
#       # stdout 额外一行：DELETE_LOCAL_BRANCH=true|false
#       # 仅当 created_local_branch=true 时为 true；既有分支永远 false。
#   bash scripts/slot_registry.sh status [--json]
#   bash scripts/slot_registry.sh capacity
#   bash scripts/slot_registry.sh is-held --slot slot-1
#
# 失败一律非零 + stderr 说明；任何查询异常不假装空闲。
set -euo pipefail

cmd="${1:-}"
shift || true

ROOT=$(realpath "$(dirname "$(git rev-parse --git-common-dir)")")
REG_ROOT="$ROOT/.agent-worktrees/.slot-registry"
DEFAULT_MAX=2

die() { echo "slot_registry: $*" >&2; exit 2; }
need() { [[ -n "${1:-}" ]] || die "missing $2"; }

ensure_reg() {
  mkdir -p "$REG_ROOT"
  if [[ ! -f "$REG_ROOT/capacity" ]]; then
    printf '%s\n' "$DEFAULT_MAX" > "$REG_ROOT/capacity"
  fi
}

read_capacity() {
  ensure_reg
  local c
  c=$(tr -d '[:space:]' < "$REG_ROOT/capacity")
  [[ "$c" =~ ^[1-9][0-9]*$ ]] || die "invalid capacity file: $c"
  printf '%s\n' "$c"
}

count_held() {
  ensure_reg
  local n=0 d
  for d in "$REG_ROOT"/slot-*.lock; do
    [[ -d "$d" ]] || continue
    n=$((n + 1))
  done
  printf '%s\n' "$n"
}

slot_lock_dir() {
  local slot="$1"
  [[ "$slot" =~ ^slot-[0-9]+$ ]] || die "invalid slot name: $slot (expect slot-<N>)"
  printf '%s/%s.lock\n' "$REG_ROOT" "$slot"
}

read_field() {
  local dir="$1" field="$2"
  [[ -f "$dir/$field" ]] || { printf ''; return 0; }
  tr -d '\r' < "$dir/$field" | head -n 1
}

write_field() {
  local dir="$1" field="$2" value="$3"
  printf '%s\n' "$value" > "$dir/$field"
}

require_holder() {
  local dir="$1" task="$2"
  [[ -d "$dir" ]] || die "slot not held"
  local cur
  cur=$(read_field "$dir" task_id)
  [[ "$cur" == "$task" ]] || die "holder mismatch: have=$cur want=$task"
}

parse_kv() {
  # sets globals: SLOT TASK BRANCH CLAIM AGENT VALUE JSON
  SLOT=""; TASK=""; BRANCH=""; CLAIM=""; AGENT=""; VALUE=""; JSON=0
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --slot) SLOT="${2:-}"; shift 2 ;;
      --task) TASK="${2:-}"; shift 2 ;;
      --branch) BRANCH="${2:-}"; shift 2 ;;
      --claim-sha) CLAIM="${2:-}"; shift 2 ;;
      --agent) AGENT="${2:-}"; shift 2 ;;
      --value) VALUE="${2:-}"; shift 2 ;;
      --max) VALUE="${2:-}"; shift 2 ;;
      --json) JSON=1; shift ;;
      *) die "unknown arg: $1" ;;
    esac
  done
}

cmd_init() {
  parse_kv "$@"
  ensure_reg
  if [[ -n "$VALUE" ]]; then
    [[ "$VALUE" =~ ^[1-9][0-9]*$ ]] || die "invalid --max $VALUE"
    printf '%s\n' "$VALUE" > "$REG_ROOT/capacity"
  fi
  echo "OK init capacity=$(read_capacity) held=$(count_held)"
}

cmd_acquire() {
  parse_kv "$@"
  need "$SLOT" --slot
  need "$TASK" --task
  need "$BRANCH" --branch
  need "$CLAIM" --claim-sha
  need "$AGENT" --agent
  [[ "$CLAIM" =~ ^[0-9a-fA-F]{40}$ ]] || die "claim-sha must be 40-hex"
  ensure_reg
  local max held lock
  max=$(read_capacity)
  held=$(count_held)
  if [[ "$held" -ge "$max" ]]; then
    die "capacity full: held=$held max=$max"
  fi
  lock=$(slot_lock_dir "$SLOT")
  if ! mkdir "$lock" 2>/dev/null; then
    local other
    other=$(read_field "$lock" task_id)
    die "slot busy: $SLOT held_by=${other:-unknown}"
  fi
  write_field "$lock" task_id "$TASK"
  write_field "$lock" branch "$BRANCH"
  write_field "$lock" claim_sha "$(printf '%s' "$CLAIM" | tr 'A-F' 'a-f')"
  write_field "$lock" agent_id "$AGENT"
  write_field "$lock" state "reserved"
  write_field "$lock" created_local_branch "false"
  write_field "$lock" reserved_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "OK acquire $SLOT task=$TASK state=reserved"
}

cmd_mark_created_local() {
  parse_kv "$@"
  need "$SLOT" --slot
  need "$TASK" --task
  need "$VALUE" --value
  case "$VALUE" in
    true|false) ;;
    *) die "--value must be true|false" ;;
  esac
  local lock
  lock=$(slot_lock_dir "$SLOT")
  require_holder "$lock" "$TASK"
  write_field "$lock" created_local_branch "$VALUE"
  echo "OK mark-created-local $SLOT=$VALUE"
}

cmd_occupy() {
  parse_kv "$@"
  need "$SLOT" --slot
  need "$TASK" --task
  local lock
  lock=$(slot_lock_dir "$SLOT")
  require_holder "$lock" "$TASK"
  write_field "$lock" state "occupied"
  echo "OK occupy $SLOT task=$TASK"
}

cmd_freeze_blocked() {
  parse_kv "$@"
  need "$SLOT" --slot
  need "$TASK" --task
  local lock
  lock=$(slot_lock_dir "$SLOT")
  require_holder "$lock" "$TASK"
  write_field "$lock" state "blocked_frozen"
  echo "OK freeze-blocked $SLOT task=$TASK"
}

# release：正常闭环 / BLOCKED 干净释放后清 registry。
# 调用方负责：detach slot、（若 CLOSED）删本地分支、不 remove slot 目录。
cmd_release() {
  parse_kv "$@"
  need "$SLOT" --slot
  need "$TASK" --task
  local lock
  lock=$(slot_lock_dir "$SLOT")
  if [[ ! -d "$lock" ]]; then
    # 幂等：未持有也算释放成功
    echo "OK release $SLOT (already free)"
    return 0
  fi
  require_holder "$lock" "$TASK"
  rm -rf "$lock"
  echo "OK release $SLOT task=$TASK"
}

# rollback：进驻失败路径。stdout 含 DELETE_LOCAL_BRANCH=...
# 仅当本轮 registry 记录 created_local_branch=true 时允许调用方删本地分支；
# 既有分支（含 SHA 冲突 / BLOCKED 残留）永远 DELETE_LOCAL_BRANCH=false。
cmd_rollback() {
  parse_kv "$@"
  need "$SLOT" --slot
  need "$TASK" --task
  local lock created="false"
  lock=$(slot_lock_dir "$SLOT")
  if [[ ! -d "$lock" ]]; then
    echo "DELETE_LOCAL_BRANCH=false"
    echo "OK rollback $SLOT (already free)"
    return 0
  fi
  require_holder "$lock" "$TASK"
  created=$(read_field "$lock" created_local_branch)
  [[ "$created" == "true" ]] || created="false"
  rm -rf "$lock"
  echo "DELETE_LOCAL_BRANCH=$created"
  echo "OK rollback $SLOT task=$TASK created_local_branch=$created"
}

cmd_is_held() {
  parse_kv "$@"
  need "$SLOT" --slot
  local lock
  lock=$(slot_lock_dir "$SLOT")
  if [[ -d "$lock" ]]; then
    echo "HELD task=$(read_field "$lock" task_id) state=$(read_field "$lock" state)"
    return 0
  fi
  echo "FREE"
  return 1
}

cmd_capacity() {
  echo "max=$(read_capacity) held=$(count_held)"
}

cmd_status() {
  parse_kv "$@"
  ensure_reg
  local max held
  max=$(read_capacity)
  held=$(count_held)
  if [[ $JSON -eq 1 ]]; then
    # 最小 JSON（无 jq 依赖手拼）
    local first=1 d slot
    printf '{"max":%s,"held":%s,"slots":[' "$max" "$held"
    for d in "$REG_ROOT"/slot-*.lock; do
      [[ -d "$d" ]] || continue
      slot=$(basename "$d" .lock)
      if [[ $first -eq 0 ]]; then printf ','; fi
      first=0
      printf '{"slot":"%s","task_id":"%s","branch":"%s","claim_sha":"%s","agent_id":"%s","state":"%s","created_local_branch":"%s"}' \
        "$slot" \
        "$(read_field "$d" task_id)" \
        "$(read_field "$d" branch)" \
        "$(read_field "$d" claim_sha)" \
        "$(read_field "$d" agent_id)" \
        "$(read_field "$d" state)" \
        "$(read_field "$d" created_local_branch)"
    done
    printf ']}\n'
    return 0
  fi
  echo "capacity max=$max held=$held"
  local d
  for d in "$REG_ROOT"/slot-*.lock; do
    [[ -d "$d" ]] || continue
    echo "  $(basename "$d" .lock): task=$(read_field "$d" task_id) state=$(read_field "$d" state) branch=$(read_field "$d" branch) created_local=$(read_field "$d" created_local_branch) agent=$(read_field "$d" agent_id)"
  done
}

case "$cmd" in
  init) cmd_init "$@" ;;
  acquire) cmd_acquire "$@" ;;
  mark-created-local) cmd_mark_created_local "$@" ;;
  occupy) cmd_occupy "$@" ;;
  freeze-blocked) cmd_freeze_blocked "$@" ;;
  release) cmd_release "$@" ;;
  rollback) cmd_rollback "$@" ;;
  is-held) cmd_is_held "$@" ;;
  capacity) cmd_capacity "$@" ;;
  status) cmd_status "$@" ;;
  ""|-h|--help)
    sed -n '2,40p' "$0" | sed 's/^# \{0,1\}//'
    exit 0
    ;;
  *) die "unknown command: $cmd" ;;
esac
