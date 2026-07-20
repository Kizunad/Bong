#!/usr/bin/env bash
# slot_registry —— BugFix 常驻 slot 的可观察原子占用/释放契约。
#
# 背景：仅靠「detached HEAD = 空闲」不够——两个 agent 可同时观察到同一 detached
# slot，后到的 checkout 会抢走前一个工作区。本脚本用 reservation 目录作为唯一占用
# 权威，并记录 task/branch/claim_sha/created_local_branch。
#
# 容量门：count_held 与 reservation 创建若无串行化，多进程并发抢不同 slot 会产生
# 跨 slot TOCTOU。所有 registry 读写都获取独立 lock root 中的 flock；acquire 在同一
# 临界区内重读 capacity、统计 held、创建并完整初始化 reservation。flock 有界等待、
# 超时 fail-closed，进程异常退出时由内核释放，不删除永久 registry 状态或锁文件。
#
# 状态机（显式转换，非法一律 fail-closed 且不改字段）：
#   free --acquire--> reserved
#   reserved --mark-created-local(false→true)--> reserved
#   reserved --occupy--> occupied
#   reserved|occupied --freeze-blocked--> blocked_frozen
#   blocked_frozen --force-unfreeze-blocked--> occupied（仅人工恢复）
#   reserved --rollback--> free
#   occupied --release--> free
#   free --release|rollback--> free（幂等）
# blocked_frozen 不得普通 occupy/release/rollback/mark 解冻或清除。
#
# 布局（相对仓库根）：
#   .agent-worktrees/.slot-registry/       # 永久可观察 registry 状态
#     capacity                             # 正整数：池成员固定为 slot-1..slot-max
#     slot-<k>.lock/                       # reservation 目录；k∈[1,max]
#       task_id
#       branch
#       claim_sha
#       agent_id
#       state                              # reserved | occupied | blocked_frozen
#       created_local_branch               # false→true 单向授权
#       reserved_at
#   .agent-worktrees/.slot-registry-locks/ # 与永久状态严格分离的 flock 对象
#     acquire.lock                         # 不删除；锁生命周期绑定持有 FD
#
# 用法（均在仓库根或任意 worktree 内执行，自动定位主仓根）：
#   bash scripts/slot_registry.sh init [--max N]
#   bash scripts/slot_registry.sh acquire --slot slot-1 --task T --branch B --claim-sha S --agent A
#   bash scripts/slot_registry.sh mark-created-local --slot slot-1 --task T --value true|false
#   bash scripts/slot_registry.sh occupy --slot slot-1 --task T
#   bash scripts/slot_registry.sh freeze-blocked --slot slot-1 --task T
#   bash scripts/slot_registry.sh force-unfreeze-blocked --slot slot-1 --task T
#   bash scripts/slot_registry.sh release --slot slot-1 --task T
#   bash scripts/slot_registry.sh rollback --slot slot-1 --task T
#       # rollback stdout：DELETE_LOCAL_BRANCH=true|false
#   bash scripts/slot_registry.sh status [--json]
#   bash scripts/slot_registry.sh capacity
#   bash scripts/slot_registry.sh is-held --slot slot-1
#
# 测试专用故障注入：SLOT_REGISTRY_TEST_HOLD_GATE_READY / _RELEASE FIFO 仅在 acquire
# 获锁后、修改永久 registry 前阻塞，用于确定性验证超时与 SIGKILL 后内核释放。
# 失败一律非零 + stderr 说明；任何查询异常不假装空闲。
set -euo pipefail

cmd="${1:-}"
shift || true

ROOT=$(realpath "$(dirname "$(git rev-parse --git-common-dir)")")
REG_ROOT="$ROOT/.agent-worktrees/.slot-registry"
LOCK_ROOT="$ROOT/.agent-worktrees/.slot-registry-locks"
GATE_FILE="$LOCK_ROOT/acquire.lock"
DEFAULT_MAX=2
GATE_WAIT_SEC="${SLOT_REGISTRY_GATE_WAIT_SEC:-5}"

# 测试可通过环境隔离 registry；生产默认仍是主仓 .agent-worktrees。
if [[ -n "${SLOT_REGISTRY_ROOT_OVERRIDE:-}" ]]; then
  REG_ROOT=$(realpath -m "$SLOT_REGISTRY_ROOT_OVERRIDE")
fi
if [[ -n "${SLOT_REGISTRY_LOCK_ROOT_OVERRIDE:-}" ]]; then
  LOCK_ROOT=$(realpath -m "$SLOT_REGISTRY_LOCK_ROOT_OVERRIDE")
  GATE_FILE="$LOCK_ROOT/acquire.lock"
fi

die() { printf 'slot_registry: %s\n' "$*" >&2; exit 2; }
need() { [[ -n "${1:-}" ]] || die "missing $2"; }

validate_wait_seconds() {
  [[ "$GATE_WAIT_SEC" =~ ^([0-9]+)([.][0-9]+)?$ ]] || die "invalid SLOT_REGISTRY_GATE_WAIT_SEC: $GATE_WAIT_SEC"
}

ensure_reg() {
  mkdir -p "$REG_ROOT" "$LOCK_ROOT"
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

require_slot_in_pool() {
  local slot="$1" max="$2" n
  [[ "$slot" =~ ^slot-([0-9]+)$ ]] || die "invalid slot name: $slot (expect slot-<N>)"
  n="${BASH_REMATCH[1]}"
  [[ "$n" =~ ^[1-9][0-9]*$ ]] || die "slot out of pool: $slot (allowed slot-1..slot-$max)"
  [[ "$slot" == "slot-$((10#$n))" ]] || die "non-canonical slot name: $slot"
  (( 10#$n <= max )) || die "slot out of pool: $slot (allowed slot-1..slot-$max)"
}

slot_lock_dir() {
  local slot="$1" max
  max=$(read_capacity)
  require_slot_in_pool "$slot" "$max"
  printf '%s/%s.lock\n' "$REG_ROOT" "$slot"
}

validate_field() {
  local label="$1" value="$2"
  [[ -n "$value" ]] || die "empty $label"
  [[ ${#value} -le 4096 ]] || die "invalid $label: too long"
}

read_field() {
  local dir="$1" field="$2"
  [[ -f "$dir/$field" ]] || { printf ''; return 0; }
  local value rc=0
  IFS= read -r -d '' value < "$dir/$field" || rc=$?
  if [[ $rc -ne 0 ]]; then
    printf 'slot_registry: corrupt field (missing NUL terminator): %s\n' "$field" >&2
    return 2
  fi
  printf '%s' "$value"
}

write_field() {
  local dir="$1" field="$2" value="$3"
  printf '%s\0' "$value" > "$dir/$field"
}

require_holder() {
  local dir="$1" task="$2" cur
  [[ -d "$dir" ]] || die "slot not held"
  cur=$(read_field "$dir" task_id) || return
  [[ "$cur" == "$task" ]] || die "holder mismatch: have=$cur want=$task"
}

require_state() {
  local dir="$1"
  shift
  local cur allowed want
  cur=$(read_field "$dir" state) || return
  for allowed in "$@"; do
    [[ "$cur" == "$allowed" ]] && return 0
  done
  want=$(IFS=','; printf '%s' "$*")
  die "invalid state transition: state=$cur allowed=$want"
}

with_registry_lock() {
  validate_wait_seconds
  ensure_reg
  local fn="$1"
  shift
  exec {REGISTRY_FD}>"$GATE_FILE"
  if ! flock -w "$GATE_WAIT_SEC" "$REGISTRY_FD"; then
    exec {REGISTRY_FD}>&-
    die "acquire gate busy: timeout after ${GATE_WAIT_SEC}s (fail-closed)"
  fi
  "$fn" "$@"
  local rc=$?
  flock -u "$REGISTRY_FD" || true
  exec {REGISTRY_FD}>&-
  return "$rc"
}

maybe_hold_gate_for_test() {
  local ready="${SLOT_REGISTRY_TEST_HOLD_GATE_READY:-}"
  local release="${SLOT_REGISTRY_TEST_HOLD_GATE_RELEASE:-}"
  [[ -z "$ready" && -z "$release" ]] && return 0
  [[ -n "$ready" && -n "$release" ]] || die "test gate hold requires ready and release FIFOs"
  [[ -p "$ready" && -p "$release" ]] || die "test gate hold paths must be FIFOs"
  printf 'ready\n' > "$ready"
  local signal
  IFS= read -r signal < "$release"
  [[ "$signal" == "release" ]] || die "invalid test gate release signal"
}

parse_kv() {
  SLOT=""; TASK=""; BRANCH=""; CLAIM=""; AGENT=""; VALUE=""; JSON=0
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --slot) [[ $# -ge 2 ]] || die "missing value for --slot"; SLOT="$2"; shift 2 ;;
      --task) [[ $# -ge 2 ]] || die "missing value for --task"; TASK="$2"; shift 2 ;;
      --branch) [[ $# -ge 2 ]] || die "missing value for --branch"; BRANCH="$2"; shift 2 ;;
      --claim-sha) [[ $# -ge 2 ]] || die "missing value for --claim-sha"; CLAIM="$2"; shift 2 ;;
      --agent) [[ $# -ge 2 ]] || die "missing value for --agent"; AGENT="$2"; shift 2 ;;
      --value) [[ $# -ge 2 ]] || die "missing value for --value"; VALUE="$2"; shift 2 ;;
      --max) [[ $# -ge 2 ]] || die "missing value for --max"; VALUE="$2"; shift 2 ;;
      --json) JSON=1; shift ;;
      *) die "unknown arg: $1" ;;
    esac
  done
}

init_locked() {
  local requested="$1" current held
  current=$(read_capacity)
  held=$(count_held)
  if [[ -n "$requested" ]]; then
    [[ "$requested" =~ ^[1-9][0-9]*$ ]] || die "invalid --max $requested"
    if (( held > requested )); then
      die "cannot shrink capacity below held count: held=$held requested=$requested"
    fi
    local d slot n
    for d in "$REG_ROOT"/slot-*.lock; do
      [[ -d "$d" ]] || continue
      slot=$(basename "$d" .lock)
      [[ "$slot" =~ ^slot-([1-9][0-9]*)$ ]] || die "invalid reservation directory: $slot"
      n="${BASH_REMATCH[1]}"
      (( 10#$n <= requested )) || die "cannot shrink capacity below held slot: $slot"
    done
    printf '%s\n' "$requested" > "$REG_ROOT/capacity"
    current="$requested"
  fi
  printf 'OK init capacity=%s held=%s\n' "$current" "$held"
}

cmd_init() {
  parse_kv "$@"
  with_registry_lock init_locked "$VALUE"
}

acquire_locked() {
  local max held lock other tmp
  maybe_hold_gate_for_test
  max=$(read_capacity)
  require_slot_in_pool "$SLOT" "$max"
  lock="$REG_ROOT/$SLOT.lock"
  if [[ -d "$lock" ]]; then
    other=$(read_field "$lock" task_id) || return
    die "slot busy: $SLOT held_by=${other:-unknown}"
  fi
  held=$(count_held)
  (( held < max )) || die "capacity full: held=$held max=$max"
  tmp="$REG_ROOT/.${SLOT}.reservation.$$.$RANDOM"
  mkdir "$tmp"
  trap 'rm -rf -- "$tmp"' RETURN
  write_field "$tmp" task_id "$TASK"
  write_field "$tmp" branch "$BRANCH"
  write_field "$tmp" claim_sha "$(printf '%s' "$CLAIM" | tr 'A-F' 'a-f')"
  write_field "$tmp" agent_id "$AGENT"
  write_field "$tmp" state reserved
  write_field "$tmp" created_local_branch false
  write_field "$tmp" reserved_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  if [[ -e "$lock" ]]; then
    die "slot busy: $SLOT reservation appeared during acquire"
  fi
  mv -T -- "$tmp" "$lock"
  trap - RETURN
  printf 'OK acquire %s task=%s state=reserved\n' "$SLOT" "$TASK"
}

cmd_acquire() {
  parse_kv "$@"
  need "$SLOT" --slot; need "$TASK" --task; need "$BRANCH" --branch
  need "$CLAIM" --claim-sha; need "$AGENT" --agent
  [[ "$CLAIM" =~ ^[0-9a-fA-F]{40}$ ]] || die "claim-sha must be 40-hex"
  validate_field task "$TASK"
  validate_field branch "$BRANCH"
  validate_field agent "$AGENT"
  with_registry_lock acquire_locked
}

mark_created_local_locked() {
  local lock current
  lock=$(slot_lock_dir "$SLOT")
  require_holder "$lock" "$TASK"
  require_state "$lock" reserved
  current=$(read_field "$lock" created_local_branch) || return
  case "$VALUE:$current" in
    false:false|true:true) ;;
    true:false) write_field "$lock" created_local_branch true ;;
    false:true) die "created_local_branch is monotonic: true cannot become false" ;;
    *) die "invalid created_local_branch field: $current" ;;
  esac
  printf 'OK mark-created-local %s=%s\n' "$SLOT" "$VALUE"
}

cmd_mark_created_local() {
  parse_kv "$@"
  need "$SLOT" --slot; need "$TASK" --task; need "$VALUE" --value
  case "$VALUE" in true|false) ;; *) die "--value must be true|false" ;; esac
  validate_field task "$TASK"
  with_registry_lock mark_created_local_locked
}

occupy_locked() {
  local lock
  lock=$(slot_lock_dir "$SLOT")
  require_holder "$lock" "$TASK"
  require_state "$lock" reserved
  write_field "$lock" state occupied
  printf 'OK occupy %s task=%s\n' "$SLOT" "$TASK"
}

cmd_occupy() {
  parse_kv "$@"; need "$SLOT" --slot; need "$TASK" --task
  validate_field task "$TASK"
  with_registry_lock occupy_locked
}

freeze_blocked_locked() {
  local lock
  lock=$(slot_lock_dir "$SLOT")
  require_holder "$lock" "$TASK"
  require_state "$lock" reserved occupied
  write_field "$lock" state blocked_frozen
  printf 'OK freeze-blocked %s task=%s\n' "$SLOT" "$TASK"
}

cmd_freeze_blocked() {
  parse_kv "$@"; need "$SLOT" --slot; need "$TASK" --task
  validate_field task "$TASK"
  with_registry_lock freeze_blocked_locked
}

force_unfreeze_locked() {
  local lock
  lock=$(slot_lock_dir "$SLOT")
  require_holder "$lock" "$TASK"
  require_state "$lock" blocked_frozen
  write_field "$lock" state occupied
  printf 'OK force-unfreeze-blocked %s task=%s state=occupied\n' "$SLOT" "$TASK"
}

cmd_force_unfreeze_blocked() {
  parse_kv "$@"; need "$SLOT" --slot; need "$TASK" --task
  validate_field task "$TASK"
  with_registry_lock force_unfreeze_locked
}

release_locked() {
  local lock
  lock=$(slot_lock_dir "$SLOT")
  if [[ ! -d "$lock" ]]; then
    printf 'OK release %s (already free)\n' "$SLOT"
    return 0
  fi
  require_holder "$lock" "$TASK"
  require_state "$lock" occupied
  rm -rf -- "$lock"
  printf 'OK release %s task=%s\n' "$SLOT" "$TASK"
}

cmd_release() {
  parse_kv "$@"; need "$SLOT" --slot; need "$TASK" --task
  validate_field task "$TASK"
  with_registry_lock release_locked
}

rollback_locked() {
  local lock created=false
  lock=$(slot_lock_dir "$SLOT")
  if [[ ! -d "$lock" ]]; then
    printf 'DELETE_LOCAL_BRANCH=false\nOK rollback %s (already free)\n' "$SLOT"
    return 0
  fi
  require_holder "$lock" "$TASK"
  require_state "$lock" reserved
  created=$(read_field "$lock" created_local_branch) || return
  [[ "$created" == true ]] || created=false
  rm -rf -- "$lock"
  printf 'DELETE_LOCAL_BRANCH=%s\n' "$created"
  printf 'OK rollback %s task=%s created_local_branch=%s\n' "$SLOT" "$TASK" "$created"
}

cmd_rollback() {
  parse_kv "$@"; need "$SLOT" --slot; need "$TASK" --task
  validate_field task "$TASK"
  with_registry_lock rollback_locked
}

is_held_locked() {
  local lock
  lock=$(slot_lock_dir "$SLOT")
  if [[ -d "$lock" ]]; then
    local task state
    task=$(read_field "$lock" task_id) || return
    state=$(read_field "$lock" state) || return
    printf 'HELD task=%s state=%s\n' "$task" "$state"
    return 0
  fi
  printf 'FREE\n'
  return 1
}

cmd_is_held() {
  parse_kv "$@"; need "$SLOT" --slot
  with_registry_lock is_held_locked
}

capacity_locked() {
  printf 'max=%s held=%s\n' "$(read_capacity)" "$(count_held)"
}

cmd_capacity() { with_registry_lock capacity_locked; }

json_string() {
  python3 -c 'import json,sys; print(json.dumps(sys.stdin.buffer.read().decode("utf-8"), ensure_ascii=False), end="")'
}

status_locked() {
  local max held first=1 d slot task branch claim agent state created reserved
  max=$(read_capacity); held=$(count_held)
  if [[ $JSON -eq 1 ]]; then
    printf '{"max":%s,"held":%s,"slots":[' "$max" "$held"
    for d in "$REG_ROOT"/slot-*.lock; do
      [[ -d "$d" ]] || continue
      slot=$(basename "$d" .lock)
      task=$(read_field "$d" task_id) || return
      branch=$(read_field "$d" branch) || return
      claim=$(read_field "$d" claim_sha) || return
      agent=$(read_field "$d" agent_id) || return
      state=$(read_field "$d" state) || return
      created=$(read_field "$d" created_local_branch) || return
      reserved=$(read_field "$d" reserved_at) || return
      [[ $first -eq 1 ]] || printf ','
      first=0
      printf '{"slot":'; printf '%s' "$slot" | json_string
      printf ',"task_id":'; printf '%s' "$task" | json_string
      printf ',"branch":'; printf '%s' "$branch" | json_string
      printf ',"claim_sha":'; printf '%s' "$claim" | json_string
      printf ',"agent_id":'; printf '%s' "$agent" | json_string
      printf ',"state":'; printf '%s' "$state" | json_string
      printf ',"created_local_branch":'; printf '%s' "$created" | json_string
      printf ',"reserved_at":'; printf '%s' "$reserved" | json_string
      printf '}'
    done
    printf ']}\n'
    return 0
  fi
  printf 'capacity max=%s held=%s\n' "$max" "$held"
  for d in "$REG_ROOT"/slot-*.lock; do
    [[ -d "$d" ]] || continue
    task=$(read_field "$d" task_id) || return
    state=$(read_field "$d" state) || return
    branch=$(read_field "$d" branch) || return
    created=$(read_field "$d" created_local_branch) || return
    agent=$(read_field "$d" agent_id) || return
    printf '  %s: task=%q state=%q branch=%q created_local=%q agent=%q\n' \
      "$(basename "$d" .lock)" "$task" "$state" "$branch" "$created" "$agent"
  done
}

cmd_status() { parse_kv "$@"; with_registry_lock status_locked; }

case "$cmd" in
  init) cmd_init "$@" ;;
  acquire) cmd_acquire "$@" ;;
  mark-created-local) cmd_mark_created_local "$@" ;;
  occupy) cmd_occupy "$@" ;;
  freeze-blocked) cmd_freeze_blocked "$@" ;;
  force-unfreeze-blocked) cmd_force_unfreeze_blocked "$@" ;;
  release) cmd_release "$@" ;;
  rollback) cmd_rollback "$@" ;;
  is-held) cmd_is_held "$@" ;;
  capacity) cmd_capacity "$@" ;;
  status) cmd_status "$@" ;;
  ""|-h|--help)
    perl -ne 'if ($. >= 2 && $. <= 56) { s/^# ?//; print }' "$0"
    exit 0
    ;;
  *) die "unknown command: $cmd" ;;
esac
