#!/usr/bin/env bash
# slot_registry —— BugFix 常驻 slot 的原子 reservation 与唯一进驻门。
#
# 契约：
# - 全部 registry 读写由独立 flock 串行化；callback 非零时也总会 unlock/close FD。
# - acquire 发布前审计全部 reservation。任一记录损坏/不完整/越界，或 task、branch、
#   agent、owner token 非一一对应，均 fail-closed 且不改 registry。
# - acquire 返回随机 OWNER_TOKEN。默认 status 不显示 token；正常 holder mutation 必须
#   同时核对 slot+task+agent+token，旧 holder 不能操作后来的 reservation。
# - occupy 是唯一生产进驻门：reserved→occupied 前验证 canonical 固定 slot worktree
#   存在、已注册且 locked，branch/HEAD/upstream/claim 对拍，tracked/untracked 干净，
#   ignored 仅含窄缓存白名单。失败保持 state 不变。
# - 不实现 PID/liveness 自动恢复。manual-report 只报告；blocked_frozen_* 仅记录冻结前
#   状态与旧 reservation 身份。人工 force-unfreeze 必须携带完整旧身份、operator、reason、
#   recovery-agent，先写 durable 私有 handoff 与公开 intent，再一次性返回 OPERATION_ID 和新
#   OWNER_TOKEN；随后 recovery agent 必须以这两个凭据调用 resume-unfreeze-blocked。resume 可从
#   agent/token/state/completion 任一中断点继续；旧 holder 在 agent/token 轮换后失效。reserved
#   来源最终只回到 reserved，仍须过 occupy；occupied 来源最终回到 occupied。
#
# 状态机：
#   free --acquire--> reserved
#   reserved --mark-created-local(false→true)--> reserved
#   reserved --occupy(valid canonical worktree)--> occupied
#   reserved --freeze-blocked--> blocked_frozen_from_reserved
#   occupied --freeze-blocked--> blocked_frozen_from_occupied
#   blocked_frozen_from_reserved --force-unfreeze-blocked(prepare audited handoff)--> blocked_frozen_from_reserved
#   blocked_frozen_from_reserved --resume-unfreeze-blocked(resumable)--> reserved
#   blocked_frozen_from_occupied --force-unfreeze-blocked(prepare audited handoff)--> blocked_frozen_from_occupied
#   blocked_frozen_from_occupied --resume-unfreeze-blocked(resumable)--> occupied
#   reserved --rollback--> free
#   occupied --release--> free
# blocked_frozen_* 不得走普通 occupy/release/rollback/mark；free 上 mutation 也拒绝，因为
# reservation 已不存在，无法验证 owner token。
#
# 布局（相对主 checkout 根）：
#   .agent-worktrees/.slot-registry/{capacity,manual-recovery.audit.jsonl,manual-handoff.lock/,slot-<k>.lock/}
#   .agent-worktrees/.slot-registry-locks/acquire.lock
#   .agent-worktrees/slot-<k>/                 # canonical 常驻 locked worktree
#
# 用法：
#   bash scripts/slot_registry.sh init [--max N]
#   out=$(bash scripts/slot_registry.sh acquire --slot slot-1 --task T --branch B --claim-sha S --agent A)
#   owner_token=$(printf '%s\n' "$out" | perl -ne 'print $1 if /^OWNER_TOKEN=([0-9a-f]{64})$/')
#   bash scripts/slot_registry.sh mark-created-local --slot slot-1 --task T --agent A --owner-token "$owner_token" --value true
#   bash scripts/slot_registry.sh occupy --slot slot-1 --task T --agent A --owner-token "$owner_token"
#   bash scripts/slot_registry.sh freeze-blocked --slot slot-1 --task T --agent A --owner-token "$owner_token"
#   bash scripts/slot_registry.sh release --slot slot-1 --task T --agent A --owner-token "$owner_token"
#   bash scripts/slot_registry.sh rollback --slot slot-1 --task T --agent A --owner-token "$owner_token"
#       # rollback stdout：DELETE_LOCAL_BRANCH=true|false
#   bash scripts/slot_registry.sh manual-report --slot slot-1
#   bash scripts/slot_registry.sh force-unfreeze-blocked --slot slot-1 --task T --branch B \
#     --claim-sha S --agent OLD --recovery-agent NEW --operator '<human>' --reason '<ticket/reason>'
#       # stdout contains one-time OPERATION_ID + OWNER_TOKEN; save both, then call resume below.
#   bash scripts/slot_registry.sh resume-unfreeze-blocked --slot slot-1 --task T --branch B \
#     --claim-sha S --recovery-agent NEW --operation-id OP --owner-token "$owner_token" \
#     --operator '<human>' --reason '<same-ticket/reason>'
#   bash scripts/slot_registry.sh status [--json]
#   bash scripts/slot_registry.sh capacity
#   bash scripts/slot_registry.sh is-held --slot slot-1
#
# 测试注入（FIFO only，不读取、探测或 signal 任意 PID）：
# - SLOT_REGISTRY_TEST_HOLD_GATE_READY / _RELEASE / _INSTANCE：持锁 acquire 暂停。
# - SLOT_REGISTRY_TEST_WAIT_GATE_READY / _RELEASE / _ACK / _INSTANCE：竞争者在 flock 前
#   建立确定性 barrier。
# - SLOT_REGISTRY_TEST_FAIL_ACQUIRE_STEP=write|date|mv：临时 reservation fail-clean。
# - SLOT_REGISTRY_TEST_FAIL_UNFREEZE_STEP=audit|audit-absent|audit-written|prepare|agent|token|state|complete|cleanup：人工恢复故障注入。
#
# 失败一律非零 + stderr；任何查询异常不假装空闲。
set -euo pipefail
umask 077

cmd="${1:-}"
shift || true

ROOT=$(realpath "$(dirname "$(git rev-parse --git-common-dir)")")
REG_ROOT="$ROOT/.agent-worktrees/.slot-registry"
LOCK_ROOT="$ROOT/.agent-worktrees/.slot-registry-locks"
GATE_FILE="$LOCK_ROOT/acquire.lock"
MANUAL_AUDIT_FILE="$REG_ROOT/manual-recovery.audit.jsonl"
DEFAULT_MAX=2
GATE_WAIT_SEC="${SLOT_REGISTRY_GATE_WAIT_SEC:-5}"
CACHE_DIRS=("server/target" "client/build" "client/.gradle")
REQUIRED_FIELDS=(task_id branch claim_sha agent_id owner_token state created_local_branch reserved_at)
HANDOFF_DIR_NAME=manual-handoff
REQUIRED_HANDOFF_FIELDS=(operation_id task_id branch claim_sha old_agent old_token recovery_agent new_token from_state target_state operator reason timestamp)

# 测试只隔离 registry/lock；canonical slot 仍是沙箱仓的真实 .agent-worktrees/slot-N。
if [[ -n "${SLOT_REGISTRY_ROOT_OVERRIDE:-}" ]]; then
  REG_ROOT=$(realpath -m "$SLOT_REGISTRY_ROOT_OVERRIDE")
  MANUAL_AUDIT_FILE="$REG_ROOT/manual-recovery.audit.jsonl"
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
ensure_roots() { mkdir -p "$REG_ROOT" "$LOCK_ROOT"; }
validate_field() {
  local label="$1" value="$2"
  [[ -n "$value" ]] || die "empty $label"
  [[ ${#value} -le 4096 ]] || die "invalid $label: too long"
}
validate_branch() {
  validate_field branch "$1"
  git check-ref-format "refs/heads/$1" >/dev/null 2>&1 || die "invalid branch: $1"
}
validate_claim() { [[ "$1" =~ ^[0-9a-fA-F]{40}$ ]] || die "claim-sha must be 40-hex"; }
validate_owner_token() { [[ "$1" =~ ^[0-9a-f]{64}$ ]] || die "owner-token must be 64 lowercase hex"; }
validate_capacity_value() {
  local value="$1"
  python3 - "$value" <<'PYVALIDATECAPACITY'
import re, sys
value = sys.argv[1]
if not re.fullmatch(r"[1-9][0-9]*", value) or int(value) > 2**63 - 1:
    raise SystemExit(2)
PYVALIDATECAPACITY
}

read_capacity() {
  [[ -f "$REG_ROOT/capacity" && ! -L "$REG_ROOT/capacity" ]] || die "registry not initialized: run init"
  local c
  c=$(python3 - "$REG_ROOT/capacity" <<'PYCAPACITY'
from pathlib import Path
import re, sys
raw = Path(sys.argv[1]).read_bytes()
if not re.fullmatch(rb"[1-9][0-9]*\n?", raw):
    raise SystemExit(2)
value = raw.rstrip(b"\n")
# Bound the persisted pool to Bash's signed arithmetic range before any 10# expansion.
if int(value) > 2**63 - 1:
    raise SystemExit(2)
sys.stdout.write(value.decode("ascii"))
PYCAPACITY
  ) || die "invalid capacity file"
  printf '%s\n' "$c"
}
write_capacity_atomic() {
  local value="$1" tmp
  tmp=$(mktemp "$REG_ROOT/.capacity.XXXXXX") || return
  if ! printf '%s\n' "$value" > "$tmp"; then rm -f -- "$tmp"; return 1; fi
  if ! mv -T -- "$tmp" "$REG_ROOT/capacity"; then rm -f -- "$tmp"; return 1; fi
}
require_slot_in_pool() {
  local slot="$1" max="$2" n
  [[ "$slot" =~ ^slot-([0-9]+)$ ]] || die "invalid slot name: $slot (expect slot-<N>)"
  n="${BASH_REMATCH[1]}"
  [[ "$n" =~ ^[1-9][0-9]*$ ]] || die "slot out of pool: $slot (allowed slot-1..slot-$max)"
  [[ "$slot" == "slot-$((10#$n))" ]] || die "non-canonical slot name: $slot"
  (( 10#$n <= max )) || die "slot out of pool: $slot (allowed slot-1..slot-$max)"
}

read_field_into() {
  local result_var="$1" dir="$2" field="$3" value rc=0
  local path="$dir/$field"
  [[ -f "$path" && ! -L "$path" ]] || { printf 'slot_registry: missing/corrupt field: %s\n' "$field" >&2; return 2; }
  if ! python3 - "$path" <<'PYFIELD'
from pathlib import Path
import sys
raw = Path(sys.argv[1]).read_bytes()
raise SystemExit(0 if raw.endswith(b"\0") and b"\0" not in raw[:-1] else 2)
PYFIELD
  then
    printf 'slot_registry: corrupt field (require exactly one trailing NUL): %s\n' "$field" >&2
    return 2
  fi
  IFS= read -r -d '' value < "$path" || rc=$?
  [[ $rc -eq 0 ]] || { printf 'slot_registry: corrupt field: %s\n' "$field" >&2; return 2; }
  printf -v "$result_var" '%s' "$value"
}
write_field() { printf '%s\0' "$3" > "$1/$2"; }
write_field_atomic() {
  local dir="$1" field="$2" value="$3" tmp
  tmp=$(mktemp "$dir/.${field}.XXXXXX") || return
  if ! printf '%s\0' "$value" > "$tmp"; then rm -f -- "$tmp"; return 1; fi
  if ! mv -T -- "$tmp" "$dir/$field"; then rm -f -- "$tmp"; return 1; fi
  fsync_dir "$dir"
}
fsync_dir() {
  python3 - "$1" <<'PYFSYNCDIR'
import os, sys
fd = os.open(sys.argv[1], os.O_RDONLY | os.O_DIRECTORY)
try:
    os.fsync(fd)
finally:
    os.close(fd)
PYFSYNCDIR
}

handoff_progress() {
  local lock="$1" old_agent="$2" old_token="$3" recovery_agent="$4" new_token="$5" from_state="$6" target_state="$7"
  local agent token state
  read_field_into agent "$lock" agent_id || exit $?
  read_field_into token "$lock" owner_token || exit $?
  read_field_into state "$lock" state || exit $?
  if [[ "$agent" == "$old_agent" && "$token" == "$old_token" && "$state" == "$from_state" ]]; then printf '0\n'; return; fi
  if [[ "$agent" == "$recovery_agent" && "$token" == "$old_token" && "$state" == "$from_state" ]]; then printf '1\n'; return; fi
  if [[ "$agent" == "$recovery_agent" && "$token" == "$new_token" && "$state" == "$from_state" ]]; then printf '2\n'; return; fi
  if [[ "$agent" == "$recovery_agent" && "$token" == "$new_token" && "$state" == "$target_state" ]]; then printf '3\n'; return; fi
  die "manual handoff fields are inconsistent; manual inspection required"
}

AUDIT_SLOTS=(); AUDIT_TASKS=(); AUDIT_BRANCHES=(); AUDIT_CLAIMS=()
AUDIT_AGENTS=(); AUDIT_TOKENS=(); AUDIT_STATES=(); AUDIT_CREATED=(); AUDIT_RESERVED=()
AUDIT_HANDOFF_SLOTS=(); AUDIT_HANDOFF_OPERATIONS=(); AUDIT_HANDOFF_OLD_AGENTS=(); AUDIT_HANDOFF_OLD_TOKENS=()
AUDIT_HANDOFF_RECOVERY_AGENTS=(); AUDIT_HANDOFF_NEW_TOKENS=()
AUDIT_MANUAL_INTENT_OPERATIONS=(); AUDIT_MANUAL_COMPLETED_OPERATIONS=()
AUDIT_ALLOW_MISSING_INTENT=0

audit_value_exists_other_slot() {
  local needle="$1" kind="$2" skip_slot="$3" i
  for i in "${!AUDIT_SLOTS[@]}"; do
    [[ "${AUDIT_SLOTS[$i]}" == "$skip_slot" ]] && continue
    case "$kind" in
      task) [[ "${AUDIT_TASKS[$i]}" == "$needle" ]] && return 0 ;;
      branch) [[ "${AUDIT_BRANCHES[$i]}" == "$needle" ]] && return 0 ;;
      agent) [[ "${AUDIT_AGENTS[$i]}" == "$needle" ]] && return 0 ;;
      token) [[ "${AUDIT_TOKENS[$i]}" == "$needle" ]] && return 0 ;;
      *) die "unknown audited field: $kind" ;;
    esac
  done
  return 1
}

# Must run under registry flock. Success fills AUDIT_*; failure is read-only.
audit_registry() {
  local max="$1" entry name slot n task branch claim agent token state created reserved field i
  local handoff_root operation old_agent old_token recovery_agent new_token from_state target_state operator reason timestamp lock
  local audit_operation audit_output
  local -a audit_operations=()
  AUDIT_SLOTS=(); AUDIT_TASKS=(); AUDIT_BRANCHES=(); AUDIT_CLAIMS=()
  AUDIT_AGENTS=(); AUDIT_TOKENS=(); AUDIT_STATES=(); AUDIT_CREATED=(); AUDIT_RESERVED=()
  AUDIT_HANDOFF_SLOTS=(); AUDIT_HANDOFF_OPERATIONS=(); AUDIT_HANDOFF_OLD_AGENTS=(); AUDIT_HANDOFF_OLD_TOKENS=()
  AUDIT_HANDOFF_RECOVERY_AGENTS=(); AUDIT_HANDOFF_NEW_TOKENS=()
  AUDIT_MANUAL_INTENT_OPERATIONS=(); AUDIT_MANUAL_COMPLETED_OPERATIONS=()

  for entry in "$REG_ROOT"/.slot-*.reservation.*; do
    [[ -e "$entry" || -L "$entry" ]] || continue
    die "orphan temporary reservation requires manual inspection: $(basename "$entry")"
  done
  for entry in "$REG_ROOT"/*.lock; do
    [[ -e "$entry" || -L "$entry" ]] || continue
    name=$(basename "$entry")
    [[ "$name" == "$HANDOFF_DIR_NAME.lock" ]] && continue
    [[ "$name" =~ ^slot-([1-9][0-9]*)[.]lock$ ]] || die "invalid reservation entry: $name"
    n="${BASH_REMATCH[1]}"; slot="slot-$((10#$n))"
    [[ "$name" == "$slot.lock" ]] || die "non-canonical reservation entry: $name"
    (( 10#$n <= max )) || die "reservation outside capacity: $slot max=$max"
    [[ -d "$entry" && ! -L "$entry" ]] || die "reservation is not a real directory: $name"
    for field in "${REQUIRED_FIELDS[@]}"; do
      [[ -f "$entry/$field" && ! -L "$entry/$field" ]] || die "incomplete reservation $slot: missing/corrupt $field"
    done
    read_field_into task "$entry" task_id || exit $?
    read_field_into branch "$entry" branch || exit $?
    read_field_into claim "$entry" claim_sha || exit $?
    read_field_into agent "$entry" agent_id || exit $?
    read_field_into token "$entry" owner_token || exit $?
    read_field_into state "$entry" state || exit $?
    read_field_into created "$entry" created_local_branch || exit $?
    read_field_into reserved "$entry" reserved_at || exit $?
    validate_field task "$task"; validate_branch "$branch"; validate_field agent "$agent"
    [[ "$claim" =~ ^[0-9a-f]{40}$ ]] || die "corrupt claim_sha in $slot"
    validate_owner_token "$token"
    case "$state" in reserved|occupied|blocked_frozen_from_reserved|blocked_frozen_from_occupied) ;; *) die "corrupt state in $slot: $state" ;; esac
    case "$created" in true|false) ;; *) die "corrupt created_local_branch in $slot: $created" ;; esac
    [[ "$reserved" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$ ]] || die "corrupt reserved_at in $slot"
    for i in "${!AUDIT_SLOTS[@]}"; do
      [[ "${AUDIT_TASKS[$i]}" != "$task" ]] || die "duplicate task_id across ${AUDIT_SLOTS[$i]} and $slot"
      [[ "${AUDIT_BRANCHES[$i]}" != "$branch" ]] || die "duplicate branch across ${AUDIT_SLOTS[$i]} and $slot"
      [[ "${AUDIT_AGENTS[$i]}" != "$agent" ]] || die "duplicate agent_id across ${AUDIT_SLOTS[$i]} and $slot"
      [[ "${AUDIT_TOKENS[$i]}" != "$token" ]] || die "duplicate owner_token across ${AUDIT_SLOTS[$i]} and $slot"
    done
    AUDIT_SLOTS+=("$slot"); AUDIT_TASKS+=("$task"); AUDIT_BRANCHES+=("$branch")
    AUDIT_CLAIMS+=("$claim"); AUDIT_AGENTS+=("$agent"); AUDIT_TOKENS+=("$token")
    AUDIT_STATES+=("$state"); AUDIT_CREATED+=("$created"); AUDIT_RESERVED+=("$reserved")
  done

  if [[ -e "$MANUAL_AUDIT_FILE" || -L "$MANUAL_AUDIT_FILE" ]]; then
    [[ -f "$MANUAL_AUDIT_FILE" && ! -L "$MANUAL_AUDIT_FILE" ]] || die "manual audit is not a real file"
    audit_output=$(python3 - "$MANUAL_AUDIT_FILE" <<'PYAUDITCHECK'
import json
import re
import sys
from pathlib import Path

path = Path(sys.argv[1])
raw_data = path.read_bytes()
if raw_data and not raw_data.endswith(b"\n"):
    raise SystemExit("manual audit must end with a newline")
required = {
    "event", "operation_id", "timestamp", "slot", "task_id", "branch",
    "claim_sha", "agent_id", "recovery_agent_id", "owner_token_sha256",
    "operator", "reason", "from_state", "target_state", "uid",
}
operations = {}
for line_number, raw in enumerate(path.read_bytes().splitlines(), 1):
    try:
        line = raw.decode("utf-8")
        row = json.loads(line)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise SystemExit(f"invalid manual audit JSON at line {line_number}: {exc}")
    if not isinstance(row, dict) or set(row) != required:
        raise SystemExit(f"invalid manual audit fields at line {line_number}")
    event = row["event"]
    if event not in {"force-unfreeze-blocked-intent", "force-unfreeze-blocked-completed"}:
        raise SystemExit(f"invalid manual audit event at line {line_number}")
    string_fields = required - {"uid"}
    if any(not isinstance(row[name], str) or not row[name] for name in string_fields):
        raise SystemExit(f"invalid manual audit string at line {line_number}")
    if not isinstance(row["uid"], int) or isinstance(row["uid"], bool) or row["uid"] < 0:
        raise SystemExit(f"invalid manual audit uid at line {line_number}")
    if not re.fullmatch(r"[0-9a-f]{32}", row["operation_id"]):
        raise SystemExit(f"invalid manual audit operation_id at line {line_number}")
    if not re.fullmatch(r"[0-9a-f]{40}", row["claim_sha"]):
        raise SystemExit(f"invalid manual audit claim_sha at line {line_number}")
    if not re.fullmatch(r"[0-9a-f]{64}", row["owner_token_sha256"]):
        raise SystemExit(f"invalid manual audit token digest at line {line_number}")
    if not re.fullmatch(r"slot-[1-9][0-9]*", row["slot"]):
        raise SystemExit(f"invalid manual audit slot at line {line_number}")
    if not re.fullmatch(r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z", row["timestamp"]):
        raise SystemExit(f"invalid manual audit timestamp at line {line_number}")
    if (row["from_state"], row["target_state"]) not in {
        ("blocked_frozen_from_reserved", "reserved"),
        ("blocked_frozen_from_occupied", "occupied"),
    }:
        raise SystemExit(f"invalid manual audit state pair at line {line_number}")
    operation_id = row["operation_id"]
    identity = {name: row[name] for name in required - {"event"}}
    operation = operations.setdefault(operation_id, {"identity": identity, "events": []})
    if operation["identity"] != identity:
        raise SystemExit(f"conflicting manual audit identity for operation {operation_id}")
    operation["events"].append(event)

for operation_id, operation in operations.items():
    events = operation["events"]
    if events not in [
        ["force-unfreeze-blocked-intent"],
        ["force-unfreeze-blocked-intent", "force-unfreeze-blocked-completed"],
    ]:
        raise SystemExit(f"invalid manual audit lifecycle for operation {operation_id}")
    print(("completed" if len(events) == 2 else "intent") + " " + operation_id)
PYAUDITCHECK
    ) || die "manual audit validation failed"
    if [[ -n "$audit_output" ]]; then
      mapfile -t audit_operations <<< "$audit_output"
    fi
    for audit_operation in "${audit_operations[@]}"; do
      case "$audit_operation" in
        "intent "*) AUDIT_MANUAL_INTENT_OPERATIONS+=("${audit_operation#intent }") ;;
        "completed "*)
          AUDIT_MANUAL_INTENT_OPERATIONS+=("${audit_operation#completed }")
          AUDIT_MANUAL_COMPLETED_OPERATIONS+=("${audit_operation#completed }")
          ;;
        *) die "manual audit validation returned an invalid record" ;;
      esac
    done
  fi

  handoff_root="$REG_ROOT/$HANDOFF_DIR_NAME.lock"
  if [[ -e "$handoff_root" || -L "$handoff_root" ]]; then
    [[ -d "$handoff_root" && ! -L "$handoff_root" ]] || die "manual handoff root is not a real directory"
    for entry in "$handoff_root"/* "$handoff_root"/.*; do
      [[ -e "$entry" || -L "$entry" ]] || continue
      name=$(basename "$entry")
      [[ "$name" != . && "$name" != .. ]] || continue
      [[ "$name" =~ ^slot-([1-9][0-9]*)$ ]] || die "invalid manual handoff entry: $name"
      n="${BASH_REMATCH[1]}"; slot="slot-$((10#$n))"
      [[ "$name" == "$slot" ]] || die "non-canonical manual handoff entry: $name"
      (( 10#$n <= max )) || die "manual handoff outside capacity: $slot max=$max"
      [[ -d "$entry" && ! -L "$entry" ]] || die "manual handoff is not a real directory: $name"
      for field in "${REQUIRED_HANDOFF_FIELDS[@]}"; do
        [[ -f "$entry/$field" && ! -L "$entry/$field" ]] || die "incomplete manual handoff $slot: missing/corrupt $field"
      done
      read_field_into operation "$entry" operation_id || exit $?
      read_field_into task "$entry" task_id || exit $?
      read_field_into branch "$entry" branch || exit $?
      read_field_into claim "$entry" claim_sha || exit $?
      read_field_into old_agent "$entry" old_agent || exit $?
      read_field_into old_token "$entry" old_token || exit $?
      read_field_into recovery_agent "$entry" recovery_agent || exit $?
      read_field_into new_token "$entry" new_token || exit $?
      read_field_into from_state "$entry" from_state || exit $?
      read_field_into target_state "$entry" target_state || exit $?
      read_field_into operator "$entry" operator || exit $?
      read_field_into reason "$entry" reason || exit $?
      read_field_into timestamp "$entry" timestamp || exit $?
      [[ "$operation" =~ ^[0-9a-f]{32}$ ]] || die "corrupt operation_id in manual handoff $slot"
      validate_field task "$task"; validate_branch "$branch"; validate_field old-agent "$old_agent"
      validate_field recovery-agent "$recovery_agent"; validate_field operator "$operator"; validate_field reason "$reason"
      [[ "$claim" =~ ^[0-9a-f]{40}$ ]] || die "corrupt claim_sha in manual handoff $slot"
      validate_owner_token "$old_token"; validate_owner_token "$new_token"
      [[ "$old_agent" != "$recovery_agent" ]] || die "manual handoff reuses agent in $slot"
      [[ "$old_token" != "$new_token" ]] || die "manual handoff reuses owner_token in $slot"
      case "$from_state:$target_state" in
        blocked_frozen_from_reserved:reserved|blocked_frozen_from_occupied:occupied) ;;
        *) die "corrupt state pair in manual handoff $slot" ;;
      esac
      [[ "$timestamp" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$ ]] || die "corrupt timestamp in manual handoff $slot"
      if value_exists "$operation" "${AUDIT_MANUAL_COMPLETED_OPERATIONS[@]}"; then
        [[ "$AUDIT_ALLOW_MISSING_INTENT" == 1 ]] \
          || die "completed manual audit still has pending handoff: $operation"
      fi
      if ! value_exists "$operation" "${AUDIT_MANUAL_INTENT_OPERATIONS[@]}"; then
        [[ "$AUDIT_ALLOW_MISSING_INTENT" == 1 ]] \
          || die "manual handoff has no durable public intent: $operation"
        [[ "$(handoff_progress "$REG_ROOT/$slot.lock" "$old_agent" "$old_token" "$recovery_agent" "$new_token" "$from_state" "$target_state")" == 0 ]] \
          || die "manual handoff without public intent has already mutated reservation: $operation"
      fi
      if value_exists "$operation" "${AUDIT_MANUAL_INTENT_OPERATIONS[@]}"; then
        if ! python3 - "$MANUAL_AUDIT_FILE" "$operation" "$slot" "$task" "$branch" "$claim" "$old_agent" "$recovery_agent" "$new_token" "$operator" "$reason" "$from_state" "$target_state" "$timestamp" <<'PYHANDOFFMATCH'
import hashlib
import json
import os
import sys

(path, operation_id, slot, task, branch, claim, old_agent, recovery_agent,
 new_token, operator, reason, from_state, target_state, timestamp) = sys.argv[1:]
expected = {
    "operation_id": operation_id,
    "timestamp": timestamp,
    "slot": slot,
    "task_id": task,
    "branch": branch,
    "claim_sha": claim,
    "agent_id": old_agent,
    "recovery_agent_id": recovery_agent,
    "owner_token_sha256": hashlib.sha256(new_token.encode("ascii")).hexdigest(),
    "operator": operator,
    "reason": reason,
    "from_state": from_state,
    "target_state": target_state,
    "uid": os.getuid(),
}
rows = [json.loads(line) for line in open(path, encoding="utf-8")]
intent = next((row for row in rows if row["operation_id"] == operation_id and
               row["event"] == "force-unfreeze-blocked-intent"), None)
if intent is None or {key: intent[key] for key in expected} != expected:
    raise SystemExit(2)
PYHANDOFFMATCH
        then
          die "manual handoff does not match durable public intent: $operation"
        fi
      fi
      value_exists "$operation" "${AUDIT_HANDOFF_OPERATIONS[@]}" && die "duplicate manual handoff operation_id"
      value_exists "$recovery_agent" "${AUDIT_HANDOFF_RECOVERY_AGENTS[@]}" && die "duplicate manual handoff recovery_agent"
      value_exists "$new_token" "${AUDIT_HANDOFF_NEW_TOKENS[@]}" && die "duplicate manual handoff new_token"
      audit_value_exists_other_slot "$recovery_agent" agent "$slot" && die "manual handoff recovery_agent already reserved"
      audit_value_exists_other_slot "$new_token" token "$slot" && die "manual handoff new_token already reserved"
      lock="$REG_ROOT/$slot.lock"
      [[ -d "$lock" && ! -L "$lock" ]] || die "orphan manual handoff without reservation: $slot"
      local reservation_task reservation_branch reservation_claim
      read_field_into reservation_task "$lock" task_id || exit $?
      read_field_into reservation_branch "$lock" branch || exit $?
      read_field_into reservation_claim "$lock" claim_sha || exit $?
      [[ "$reservation_task" == "$task" && "$reservation_branch" == "$branch" && "$reservation_claim" == "$claim" ]] \
        || die "manual handoff does not match reservation identity: $operation"
      handoff_progress "$lock" "$old_agent" "$old_token" "$recovery_agent" "$new_token" "$from_state" "$target_state" >/dev/null
      AUDIT_HANDOFF_SLOTS+=("$slot"); AUDIT_HANDOFF_OPERATIONS+=("$operation")
      AUDIT_HANDOFF_OLD_AGENTS+=("$old_agent"); AUDIT_HANDOFF_OLD_TOKENS+=("$old_token")
      AUDIT_HANDOFF_RECOVERY_AGENTS+=("$recovery_agent"); AUDIT_HANDOFF_NEW_TOKENS+=("$new_token")
    done
  fi
}

require_owner() {
  local dir="$1" task="$2" agent="$3" token="$4" cur_task cur_agent cur_token
  [[ -d "$dir" && ! -L "$dir" ]] || die "slot not held"
  read_field_into cur_task "$dir" task_id || exit $?
  read_field_into cur_agent "$dir" agent_id || exit $?
  read_field_into cur_token "$dir" owner_token || exit $?
  [[ "$cur_task" == "$task" && "$cur_agent" == "$agent" && "$cur_token" == "$token" ]] || die "reservation owner mismatch"
}
require_manual_identity() {
  local dir="$1" task="$2" branch="$3" claim="$4" agent="$5" a b c d
  [[ -d "$dir" && ! -L "$dir" ]] || die "slot not held"
  read_field_into a "$dir" task_id || exit $?
  read_field_into b "$dir" branch || exit $?
  read_field_into c "$dir" claim_sha || exit $?
  read_field_into d "$dir" agent_id || exit $?
  claim=$(printf '%s' "$claim" | tr 'A-F' 'a-f')
  [[ "$a" == "$task" && "$b" == "$branch" && "$c" == "$claim" && "$d" == "$agent" ]] || die "manual recovery identity mismatch"
}
require_state() {
  local dir="$1" cur allowed want; shift
  read_field_into cur "$dir" state || exit $?
  for allowed in "$@"; do [[ "$cur" == "$allowed" ]] && return 0; done
  want=$(IFS=','; printf '%s' "$*")
  die "invalid state transition: state=$cur allowed=$want"
}

maybe_wait_at_gate_for_test() {
  local ready="${SLOT_REGISTRY_TEST_WAIT_GATE_READY:-}" release="${SLOT_REGISTRY_TEST_WAIT_GATE_RELEASE:-}"
  local ack="${SLOT_REGISTRY_TEST_WAIT_GATE_ACK:-}" instance="${SLOT_REGISTRY_TEST_INSTANCE:-}" signal
  [[ -z "$ready" && -z "$release" && -z "$ack" ]] && return 0
  [[ -n "$ready" && -n "$release" && -n "$ack" ]] || die "test gate wait requires ready, release, and ack FIFOs"
  [[ -p "$ready" && -p "$release" && -p "$ack" ]] || die "test gate wait paths must be FIFOs"
  [[ "$instance" =~ ^[A-Za-z0-9._-]+$ ]] || die "test gate wait requires a safe instance token"
  if flock -n "$REGISTRY_FD"; then flock -u "$REGISTRY_FD" || true; die "test gate wait expected acquire gate to be held"; fi
  printf 'ready %s\n' "$instance" > "$ready"
  IFS= read -r signal < "$release"; [[ "$signal" == release ]] || die "invalid test gate wait release signal"
  printf 'released %s\n' "$instance" > "$ack"
}

# Callback runs in a child. Parent temporarily disables errexit only to capture its status, then
# unconditionally unlocks and closes the FD before restoring errexit.
with_registry_lock() {
  validate_wait_seconds; ensure_roots
  local fn="$1" rc=0 unlock_rc=0 had_errexit=0; shift
  exec {REGISTRY_FD}>"$GATE_FILE"
  [[ $- == *e* ]] && had_errexit=1
  set +e
  ( set -euo pipefail; maybe_wait_at_gate_for_test )
  rc=$?
  if [[ $rc -ne 0 ]]; then
    exec {REGISTRY_FD}>&-
    [[ $had_errexit -eq 1 ]] && set -e
    return "$rc"
  fi
  if ! flock -w "$GATE_WAIT_SEC" "$REGISTRY_FD"; then
    exec {REGISTRY_FD}>&-
    [[ $had_errexit -eq 1 ]] && set -e
    printf 'slot_registry: acquire gate busy: timeout after %ss (fail-closed)\n' "$GATE_WAIT_SEC" >&2
    return 2
  fi
  ( set -euo pipefail; "$fn" "$@" )
  rc=$?
  flock -u "$REGISTRY_FD" || unlock_rc=$?
  exec {REGISTRY_FD}>&-
  [[ $had_errexit -eq 1 ]] && set -e
  [[ $unlock_rc -eq 0 ]] || { printf 'slot_registry: failed to unlock registry gate\n' >&2; return 2; }
  return "$rc"
}
maybe_hold_gate_for_test() {
  local ready="${SLOT_REGISTRY_TEST_HOLD_GATE_READY:-}" release="${SLOT_REGISTRY_TEST_HOLD_GATE_RELEASE:-}"
  local instance="${SLOT_REGISTRY_TEST_INSTANCE:-}" signal
  [[ -z "$ready" && -z "$release" ]] && return 0
  [[ -n "$ready" && -n "$release" ]] || die "test gate hold requires ready and release FIFOs"
  [[ -p "$ready" && -p "$release" ]] || die "test gate hold paths must be FIFOs"
  [[ "$instance" =~ ^[A-Za-z0-9._-]+$ ]] || die "test gate hold requires a safe instance token"
  printf 'ready %s\n' "$instance" > "$ready"
  IFS= read -r signal < "$release"; [[ "$signal" == release ]] || die "invalid test gate release signal"
}

parse_kv() {
  SLOT=""; TASK=""; BRANCH=""; CLAIM=""; AGENT=""; RECOVERY_AGENT=""; OPERATION_ID=""; OWNER_TOKEN=""
  VALUE=""; REASON=""; OPERATOR=""; JSON=0
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --slot) [[ $# -ge 2 ]] || die "missing value for --slot"; SLOT="$2"; shift 2 ;;
      --task) [[ $# -ge 2 ]] || die "missing value for --task"; TASK="$2"; shift 2 ;;
      --branch) [[ $# -ge 2 ]] || die "missing value for --branch"; BRANCH="$2"; shift 2 ;;
      --claim-sha) [[ $# -ge 2 ]] || die "missing value for --claim-sha"; CLAIM="$2"; shift 2 ;;
      --agent) [[ $# -ge 2 ]] || die "missing value for --agent"; AGENT="$2"; shift 2 ;;
      --recovery-agent) [[ $# -ge 2 ]] || die "missing value for --recovery-agent"; RECOVERY_AGENT="$2"; shift 2 ;;
      --operation-id) [[ $# -ge 2 ]] || die "missing value for --operation-id"; OPERATION_ID="$2"; shift 2 ;;
      --owner-token) [[ $# -ge 2 ]] || die "missing value for --owner-token"; OWNER_TOKEN="$2"; shift 2 ;;
      --value) [[ $# -ge 2 ]] || die "missing value for --value"; VALUE="$2"; shift 2 ;;
      --max) [[ $# -ge 2 ]] || die "missing value for --max"; VALUE="$2"; shift 2 ;;
      --reason) [[ $# -ge 2 ]] || die "missing value for --reason"; REASON="$2"; shift 2 ;;
      --operator) [[ $# -ge 2 ]] || die "missing value for --operator"; OPERATOR="$2"; shift 2 ;;
      --json) JSON=1; shift ;;
      *) die "unknown arg: $1" ;;
    esac
  done
}
validate_normal_owner_args() {
  need "$SLOT" --slot; need "$TASK" --task; need "$AGENT" --agent; need "$OWNER_TOKEN" --owner-token
  validate_field task "$TASK"; validate_field agent "$AGENT"; validate_owner_token "$OWNER_TOKEN"
}

init_locked() {
  local requested="$1" current max held entry i
  if [[ -n "$requested" ]] && ! validate_capacity_value "$requested"; then
    die "invalid --max $requested"
  fi
  if [[ ! -e "$REG_ROOT/capacity" && ! -L "$REG_ROOT/capacity" ]]; then
    for entry in "$REG_ROOT"/slot-*.lock "$REG_ROOT"/.slot-*.reservation.* "$REG_ROOT/$HANDOFF_DIR_NAME.lock"; do
      [[ -e "$entry" || -L "$entry" ]] || continue
      die "registry has reservations but no capacity; manual inspection required"
    done
    max="${requested:-$DEFAULT_MAX}"; write_capacity_atomic "$max"
    printf 'OK init capacity=%s held=0\n' "$max"; return 0
  fi
  current=$(read_capacity); audit_registry "$current"; held=${#AUDIT_SLOTS[@]}
  if [[ -n "$requested" ]]; then
    (( held <= requested )) || die "cannot shrink capacity below held count: held=$held requested=$requested"
    for i in "${!AUDIT_SLOTS[@]}"; do
      [[ "${AUDIT_SLOTS[$i]}" =~ ^slot-([1-9][0-9]*)$ ]] || die "invalid audited slot"
      (( 10#${BASH_REMATCH[1]} <= requested )) || die "cannot shrink capacity below held slot: ${AUDIT_SLOTS[$i]}"
    done
    [[ "$requested" == "$current" ]] || write_capacity_atomic "$requested"
    current="$requested"
  fi
  printf 'OK init capacity=%s held=%s\n' "$current" "$held"
}
cmd_init() { parse_kv "$@"; with_registry_lock init_locked "$VALUE"; }

acquire_temp_cleanup() { [[ -z "$1" || ! -d "$1" ]] || rm -rf -- "$1"; }
generate_owner_token() { python3 -c 'import secrets; print(secrets.token_hex(32))'; }
acquire_write_temp() {
  local tmp="$1" token="$2" normalized reserved_at
  normalized=$(printf '%s' "$CLAIM" | tr 'A-F' 'a-f')
  [[ "${SLOT_REGISTRY_TEST_FAIL_ACQUIRE_STEP:-}" != write ]] || { printf 'slot_registry: injected acquire write failure\n' >&2; return 2; }
  write_field "$tmp" task_id "$TASK"; write_field "$tmp" branch "$BRANCH"; write_field "$tmp" claim_sha "$normalized"
  write_field "$tmp" agent_id "$AGENT"; write_field "$tmp" owner_token "$token"; write_field "$tmp" state reserved
  write_field "$tmp" created_local_branch false
  [[ "${SLOT_REGISTRY_TEST_FAIL_ACQUIRE_STEP:-}" != date ]] || { printf 'slot_registry: injected acquire date failure\n' >&2; return 2; }
  reserved_at=$(date -u +%Y-%m-%dT%H:%M:%SZ); write_field "$tmp" reserved_at "$reserved_at"
}
acquire_publish_temp() {
  [[ "${SLOT_REGISTRY_TEST_FAIL_ACQUIRE_STEP:-}" != mv ]] || { printf 'slot_registry: injected acquire mv failure\n' >&2; return 2; }
  mv -T -- "$1" "$2"
}
value_exists() { local needle="$1" value; shift; for value in "$@"; do [[ "$value" != "$needle" ]] || return 0; done; return 1; }
acquire_locked() {
  local max held lock tmp token rc attempts=0
  max=$(read_capacity); audit_registry "$max"; maybe_hold_gate_for_test; require_slot_in_pool "$SLOT" "$max"
  lock="$REG_ROOT/$SLOT.lock"; [[ ! -e "$lock" && ! -L "$lock" ]] || die "slot busy: $SLOT"
  value_exists "$TASK" "${AUDIT_TASKS[@]}" && die "task_id already reserved"
  value_exists "$BRANCH" "${AUDIT_BRANCHES[@]}" && die "branch already reserved"
  value_exists "$AGENT" "${AUDIT_AGENTS[@]}" && die "agent_id already reserved"
  value_exists "$AGENT" "${AUDIT_HANDOFF_RECOVERY_AGENTS[@]}" && die "agent_id reserved by pending manual handoff"
  held=${#AUDIT_SLOTS[@]}; (( held < max )) || die "capacity full: held=$held max=$max"
  while :; do
    token=$(generate_owner_token); validate_owner_token "$token"
    if ! value_exists "$token" "${AUDIT_TOKENS[@]}"; then break; fi
    attempts=$((attempts + 1)); (( attempts < 3 )) || die "owner token collision"
  done
  tmp=$(mktemp -d "$REG_ROOT/.${SLOT}.reservation.XXXXXX")
  if acquire_write_temp "$tmp" "$token"; then :; else rc=$?; acquire_temp_cleanup "$tmp"; return "$rc"; fi
  if [[ -e "$lock" || -L "$lock" ]]; then acquire_temp_cleanup "$tmp"; die "slot busy: $SLOT reservation appeared during acquire"; fi
  if acquire_publish_temp "$tmp" "$lock"; then :; else rc=$?; acquire_temp_cleanup "$tmp"; return "$rc"; fi
  printf 'OWNER_TOKEN=%s\nOK acquire %s task=%s state=reserved\n' "$token" "$SLOT" "$TASK"
}
cmd_acquire() {
  parse_kv "$@"; need "$SLOT" --slot; need "$TASK" --task; need "$BRANCH" --branch; need "$CLAIM" --claim-sha; need "$AGENT" --agent
  validate_claim "$CLAIM"; validate_field task "$TASK"; validate_branch "$BRANCH"; validate_field agent "$AGENT"
  with_registry_lock acquire_locked
}

prepare_owned_lock() {
  local max i found=0
  max=$(read_capacity); audit_registry "$max"; require_slot_in_pool "$SLOT" "$max"
  for i in "${!AUDIT_SLOTS[@]}"; do
    if [[ "${AUDIT_SLOTS[$i]}" == "$SLOT" ]]; then
      [[ "${AUDIT_TASKS[$i]}" == "$TASK" && "${AUDIT_AGENTS[$i]}" == "$AGENT" && "${AUDIT_TOKENS[$i]}" == "$OWNER_TOKEN" ]] || die "reservation owner mismatch"
      found=1
      break
    fi
  done
  [[ $found -eq 1 ]] || die "slot not held"
  OWNED_LOCK="$REG_ROOT/$SLOT.lock"
}
mark_created_local_locked() {
  local lock current; prepare_owned_lock; lock="$OWNED_LOCK"; require_state "$lock" reserved
  read_field_into current "$lock" created_local_branch || exit $?
  case "$VALUE:$current" in
    false:false|true:true) ;;
    true:false) write_field_atomic "$lock" created_local_branch true ;;
    false:true) die "created_local_branch is monotonic: true cannot become false" ;;
    *) die "invalid created_local_branch field: $current" ;;
  esac
  printf 'OK mark-created-local %s=%s\n' "$SLOT" "$VALUE"
}
cmd_mark_created_local() {
  parse_kv "$@"; validate_normal_owner_args; need "$VALUE" --value
  case "$VALUE" in true|false) ;; *) die "--value must be true|false" ;; esac
  with_registry_lock mark_created_local_locked
}

is_allowlisted_cache() {
  local rel="${1#./}" d; rel="${rel%/}"
  for d in "${CACHE_DIRS[@]}"; do [[ "$rel" == "$d" || "$rel" == "$d"/* ]] && return 0; done
  return 1
}
validate_canonical_slot_worktree() {
  local lock="$1" branch claim expected actual listing line path_real
  local current_path="" current_head="" current_branch="" current_locked=0
  local matches=0 matched_head="" matched_branch="" matched_locked=0
  local actual_branch actual_head upstream_ref upstream_head dirty ignored rel
  read_field_into branch "$lock" branch || exit $?; read_field_into claim "$lock" claim_sha || exit $?
  expected="$ROOT/.agent-worktrees/$SLOT"
  [[ -d "$expected" && ! -L "$expected" ]] || die "canonical slot worktree missing/not-real: $expected"
  actual=$(realpath "$expected") || die "cannot resolve canonical slot worktree: $expected"
  [[ "$actual" == "$expected" ]] || die "canonical slot path resolves elsewhere: $expected"
  listing=$(git worktree list --porcelain 2>/dev/null) || die "git worktree list failed"
  while IFS= read -r line; do
    case "$line" in
      worktree\ *)
        if [[ -n "$current_path" ]]; then
          path_real=$(realpath -m "$current_path")
          if [[ "$path_real" == "$expected" ]]; then matches=$((matches + 1)); matched_head="$current_head"; matched_branch="$current_branch"; matched_locked=$current_locked; fi
        fi
        current_path="${line#worktree }"; current_head=""; current_branch=""; current_locked=0 ;;
      HEAD\ *) current_head="${line#HEAD }" ;;
      branch\ *) current_branch="${line#branch }" ;;
      locked*) current_locked=1 ;;
      "")
        if [[ -n "$current_path" ]]; then
          path_real=$(realpath -m "$current_path")
          if [[ "$path_real" == "$expected" ]]; then matches=$((matches + 1)); matched_head="$current_head"; matched_branch="$current_branch"; matched_locked=$current_locked; fi
        fi
        current_path=""; current_head=""; current_branch=""; current_locked=0 ;;
    esac
  done <<< "$listing"$'\n'
  [[ $matches -eq 1 ]] || die "canonical slot is not exactly once in git worktree registry"
  [[ $matched_locked -eq 1 ]] || die "canonical slot worktree is unlocked"
  [[ "$matched_branch" == "refs/heads/$branch" ]] || die "canonical slot registered branch mismatch"
  matched_head=$(printf '%s' "$matched_head" | tr 'A-F' 'a-f'); [[ "$matched_head" == "$claim" ]] || die "canonical slot registered HEAD mismatch"
  actual_branch=$(git -C "$expected" symbolic-ref -q HEAD 2>/dev/null) || die "canonical slot is detached; expected task branch"
  [[ "$actual_branch" == "refs/heads/$branch" ]] || die "canonical slot branch mismatch"
  actual_head=$(git -C "$expected" rev-parse --verify 'HEAD^{commit}' 2>/dev/null) || die "cannot resolve canonical slot HEAD"
  actual_head=$(printf '%s' "$actual_head" | tr 'A-F' 'a-f'); [[ "$actual_head" == "$claim" ]] || die "canonical slot HEAD does not equal claim_sha"
  upstream_ref=$(git -C "$expected" rev-parse --symbolic-full-name '@{upstream}' 2>/dev/null) || die "canonical slot branch has no upstream"
  [[ "$upstream_ref" == "refs/remotes/origin/$branch" ]] || die "canonical slot upstream mismatch"
  upstream_head=$(git -C "$expected" rev-parse --verify '@{upstream}^{commit}' 2>/dev/null) || die "cannot resolve canonical slot upstream HEAD"
  upstream_head=$(printf '%s' "$upstream_head" | tr 'A-F' 'a-f'); [[ "$upstream_head" == "$claim" ]] || die "canonical slot upstream HEAD does not equal claim_sha"
  dirty=$(GIT_OPTIONAL_LOCKS=0 git -C "$expected" status --porcelain=v1 --untracked-files=all 2>/dev/null) || die "canonical slot git status failed"
  [[ -z "$dirty" ]] || die "canonical slot has tracked/untracked changes"
  ignored=$(GIT_OPTIONAL_LOCKS=0 git -C "$expected" status --porcelain=v1 --untracked-files=all --ignored=matching 2>/dev/null) || die "canonical slot ignored status failed"
  while IFS= read -r line; do
    [[ -z "$line" ]] && continue
    case "$line" in
      "!! "*) rel="${line:3}"; rel="${rel#\"}"; rel="${rel%\"}"; rel="${rel%/}"; is_allowlisted_cache "$rel" || die "canonical slot has non-whitelisted ignored path: $rel" ;;
      *) die "canonical slot status changed during ignored validation" ;;
    esac
  done <<< "$ignored"
}
occupy_locked() {
  local lock; prepare_owned_lock; lock="$OWNED_LOCK"; require_state "$lock" reserved; validate_canonical_slot_worktree "$lock"
  write_field_atomic "$lock" state occupied; printf 'OK occupy %s task=%s\n' "$SLOT" "$TASK"
}
cmd_occupy() { parse_kv "$@"; validate_normal_owner_args; with_registry_lock occupy_locked; }
freeze_blocked_locked() {
  local lock current frozen_state; prepare_owned_lock; lock="$OWNED_LOCK"; require_state "$lock" reserved occupied
  read_field_into current "$lock" state || exit $?
  case "$current" in
    reserved) frozen_state=blocked_frozen_from_reserved ;;
    occupied) frozen_state=blocked_frozen_from_occupied ;;
    *) die "invalid state transition: state=$current allowed=reserved,occupied" ;;
  esac
  write_field_atomic "$lock" state "$frozen_state"; printf 'OK freeze-blocked %s task=%s from=%s\n' "$SLOT" "$TASK" "$current"
}
cmd_freeze_blocked() { parse_kv "$@"; validate_normal_owner_args; with_registry_lock freeze_blocked_locked; }

append_manual_audit() {
  local event="$1" timestamp="$2" operation_id="$3" from_state="$4" target_state="$5" old_agent="$6" recovery_agent="$7" handoff_token="$8"
  case "$event" in
    force-unfreeze-blocked-intent|force-unfreeze-blocked-completed) ;;
    *)
      printf 'slot_registry: invalid manual audit event: %s\n' "$event" >&2
      return 2
      ;;
  esac
  if [[ -n "${SLOT_REGISTRY_TEST_FAIL_UNFREEZE_STEP:-}" && "$event" == force-unfreeze-blocked-intent ]]; then
    case "$SLOT_REGISTRY_TEST_FAIL_UNFREEZE_STEP" in
      audit)
        printf 'slot_registry: injected manual audit failure\n' >&2
        return 2
        ;;
      audit-absent)
        printf 'slot_registry: injected absent manual audit intent\n' >&2
        return 2
        ;;
      audit-written) ;;
    esac
  fi
  if [[ ! -e "$MANUAL_AUDIT_FILE" && ! -L "$MANUAL_AUDIT_FILE" ]]; then
    : > "$MANUAL_AUDIT_FILE" || return 1
    fsync_dir "$REG_ROOT" || return 1
  fi
  [[ -f "$MANUAL_AUDIT_FILE" && ! -L "$MANUAL_AUDIT_FILE" ]] || die "manual audit is not a real file"
  if [[ "$event" == force-unfreeze-blocked-completed ]] && value_exists "$operation_id" "${AUDIT_MANUAL_COMPLETED_OPERATIONS[@]}"; then
    return 0
  fi
  if [[ "$event" == force-unfreeze-blocked-completed && "${SLOT_REGISTRY_TEST_FAIL_UNFREEZE_STEP:-}" == complete ]]; then
    printf 'slot_registry: injected manual completion audit failure\n' >&2
    return 2
  fi
  python3 - "$MANUAL_AUDIT_FILE" "$event" "$timestamp" "$operation_id" "$SLOT" "$TASK" "$BRANCH" "$CLAIM" "$old_agent" "$recovery_agent" "$OPERATOR" "$REASON" "$from_state" "$target_state" "$handoff_token" <<'PYAUDIT'
import hashlib
import json
import os
import sys

(path, event, timestamp, operation_id, slot, task, branch, claim, old_agent,
 recovery_agent, operator, reason, from_state, target_state, handoff_token) = sys.argv[1:]
record = {"event": event, "operation_id": operation_id,
          "timestamp": timestamp, "slot": slot, "task_id": task, "branch": branch,
          "claim_sha": claim.lower(), "agent_id": old_agent,
          "recovery_agent_id": recovery_agent,
          "owner_token_sha256": hashlib.sha256(handoff_token.encode("ascii")).hexdigest(),
          "operator": operator, "reason": reason, "from_state": from_state,
          "target_state": target_state, "uid": os.getuid()}
data = (json.dumps(record, ensure_ascii=False, separators=(",", ":")) + "\n").encode()
created = not os.path.exists(path)
fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o600)
try:
    written = os.write(fd, data)
    if written != len(data):
        raise OSError(f"short audit write: {written}/{len(data)}")
    os.fsync(fd)
finally:
    os.close(fd)
if created:
    parent_fd = os.open(os.path.dirname(path), os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(parent_fd)
    finally:
        os.close(parent_fd)
PYAUDIT
  if [[ "${SLOT_REGISTRY_TEST_FAIL_UNFREEZE_STEP:-}" == audit-written && "$event" == force-unfreeze-blocked-intent ]]; then
    printf 'slot_registry: injected post-audit-write interruption\n' >&2
    return 2
  fi
}
remove_handoff_dir() {
  local dir="$1" parent
  rm -rf -- "$dir"
  parent=$(dirname "$dir")
  rmdir -- "$parent" 2>/dev/null || true
  fsync_dir "$REG_ROOT"
}
handoff_dir() { printf '%s/%s.lock/%s\n' "$REG_ROOT" "$HANDOFF_DIR_NAME" "$SLOT"; }
write_handoff_intent() {
  local dir="$1" operation_id="$2" old_agent="$3" old_token="$4" new_token="$5" frozen_state="$6" target_state="$7" timestamp="$8" tmp
  [[ ! -e "$dir" && ! -L "$dir" ]] || die "manual handoff already pending for $SLOT"
  mkdir -p "$REG_ROOT/$HANDOFF_DIR_NAME.lock"
  tmp=$(mktemp -d "$REG_ROOT/$HANDOFF_DIR_NAME.lock/.${SLOT}.XXXXXX") || return
  write_field "$tmp" operation_id "$operation_id"
  write_field "$tmp" task_id "$TASK"; write_field "$tmp" branch "$BRANCH"; write_field "$tmp" claim_sha "${CLAIM,,}"
  write_field "$tmp" old_agent "$old_agent"; write_field "$tmp" old_token "$old_token"
  write_field "$tmp" recovery_agent "$RECOVERY_AGENT"
  write_field "$tmp" new_token "$new_token"; write_field "$tmp" from_state "$frozen_state"; write_field "$tmp" target_state "$target_state"
  write_field "$tmp" operator "$OPERATOR"; write_field "$tmp" reason "$REASON"; write_field "$tmp" timestamp "$timestamp"
  [[ "${SLOT_REGISTRY_TEST_FAIL_UNFREEZE_STEP:-}" != prepare ]] || { rm -rf -- "$tmp"; printf 'slot_registry: injected manual handoff prepare failure\n' >&2; return 2; }
  fsync_dir "$tmp"
  mv -T -- "$tmp" "$dir" || { rm -rf -- "$tmp"; return 1; }
  fsync_dir "$(dirname "$dir")"
}
read_handoff_into_globals() {
  local dir="$1" field
  [[ -d "$dir" && ! -L "$dir" ]] || die "no pending manual handoff for $SLOT"
  for field in "${REQUIRED_HANDOFF_FIELDS[@]}"; do
    [[ -f "$dir/$field" && ! -L "$dir/$field" ]] || die "incomplete manual handoff $SLOT: $field"
  done
  read_field_into HANDOFF_OPERATION_ID "$dir" operation_id || exit $?
  read_field_into HANDOFF_TASK "$dir" task_id || exit $?
  read_field_into HANDOFF_BRANCH "$dir" branch || exit $?
  read_field_into HANDOFF_CLAIM "$dir" claim_sha || exit $?
  read_field_into HANDOFF_OLD_AGENT "$dir" old_agent || exit $?
  read_field_into HANDOFF_OLD_TOKEN "$dir" old_token || exit $?
  read_field_into HANDOFF_RECOVERY_AGENT "$dir" recovery_agent || exit $?
  read_field_into HANDOFF_NEW_TOKEN "$dir" new_token || exit $?
  read_field_into HANDOFF_FROM_STATE "$dir" from_state || exit $?
  read_field_into HANDOFF_TARGET_STATE "$dir" target_state || exit $?
  read_field_into HANDOFF_OPERATOR "$dir" operator || exit $?
  read_field_into HANDOFF_REASON "$dir" reason || exit $?
  read_field_into HANDOFF_TIMESTAMP "$dir" timestamp || exit $?
  validate_owner_token "$HANDOFF_OLD_TOKEN"; validate_owner_token "$HANDOFF_NEW_TOKEN"
}
current_handoff_step() {
  handoff_progress "$1" "$HANDOFF_OLD_AGENT" "$HANDOFF_OLD_TOKEN" "$HANDOFF_RECOVERY_AGENT" "$HANDOFF_NEW_TOKEN" "$HANDOFF_FROM_STATE" "$HANDOFF_TARGET_STATE"
}
continue_handoff_locked() {
  local lock="$1" dir="$2" step
  step=$(current_handoff_step "$lock")
  if (( step < 1 )); then
    [[ "${SLOT_REGISTRY_TEST_FAIL_UNFREEZE_STEP:-}" != agent ]] || die "injected owner handoff agent failure; durable intent requires manual inspection"
    write_field_atomic "$lock" agent_id "$HANDOFF_RECOVERY_AGENT" || die "owner handoff agent persistence failed; durable intent requires manual inspection"
  fi
  if (( step < 2 )); then
    [[ "${SLOT_REGISTRY_TEST_FAIL_UNFREEZE_STEP:-}" != token ]] || die "injected owner handoff token failure; durable intent requires manual inspection"
    write_field_atomic "$lock" owner_token "$HANDOFF_NEW_TOKEN" || die "owner handoff token persistence failed; durable intent requires manual inspection"
  fi
  if (( step < 3 )); then
    [[ "${SLOT_REGISTRY_TEST_FAIL_UNFREEZE_STEP:-}" != state ]] || die "injected unfreeze state failure; durable intent requires manual inspection"
    write_field_atomic "$lock" state "$HANDOFF_TARGET_STATE" || die "unfreeze state persistence failed; durable intent requires manual inspection"
  fi
  append_manual_audit force-unfreeze-blocked-completed "$HANDOFF_TIMESTAMP" "$HANDOFF_OPERATION_ID" "$HANDOFF_FROM_STATE" "$HANDOFF_TARGET_STATE" \
    "$HANDOFF_OLD_AGENT" "$HANDOFF_RECOVERY_AGENT" "$HANDOFF_NEW_TOKEN" \
    || die "unfreeze completed but completion audit persistence failed; durable intent requires manual inspection"
  if [[ "${SLOT_REGISTRY_TEST_FAIL_UNFREEZE_STEP:-}" == cleanup ]]; then
    printf 'slot_registry: injected manual handoff cleanup failure\n' >&2
    return 2
  fi
  remove_handoff_dir "$dir"
  printf 'OK resume-unfreeze-blocked %s task=%s state=%s recovery_agent=%s audit=%s operation_id=%s\n' \
    "$SLOT" "$TASK" "$HANDOFF_TARGET_STATE" "$HANDOFF_RECOVERY_AGENT" "$MANUAL_AUDIT_FILE" "$HANDOFF_OPERATION_ID"
}
force_unfreeze_locked() {
  local max lock timestamp operation_id frozen_state target_state old_agent old_token new_token attempts=0 dir
  max=$(read_capacity); audit_registry "$max"; require_slot_in_pool "$SLOT" "$max"; lock="$REG_ROOT/$SLOT.lock"; dir=$(handoff_dir)
  require_manual_identity "$lock" "$TASK" "$BRANCH" "$CLAIM" "$AGENT"
  require_state "$lock" blocked_frozen_from_reserved blocked_frozen_from_occupied
  read_field_into frozen_state "$lock" state || exit $?
  read_field_into old_agent "$lock" agent_id || exit $?
  read_field_into old_token "$lock" owner_token || exit $?
  case "$frozen_state" in
    blocked_frozen_from_reserved) target_state=reserved ;;
    blocked_frozen_from_occupied) target_state=occupied ;;
    *) die "invalid frozen state: $frozen_state" ;;
  esac
  [[ "$RECOVERY_AGENT" != "$old_agent" ]] || die "recovery-agent must differ from current agent"
  value_exists "$RECOVERY_AGENT" "${AUDIT_AGENTS[@]}" && die "recovery agent already reserved"
  value_exists "$RECOVERY_AGENT" "${AUDIT_HANDOFF_RECOVERY_AGENTS[@]}" && die "recovery agent reserved by pending manual handoff"
  while :; do
    new_token=$(generate_owner_token); validate_owner_token "$new_token"
    if ! value_exists "$new_token" "${AUDIT_TOKENS[@]}" && ! value_exists "$new_token" "${AUDIT_HANDOFF_NEW_TOKENS[@]}"; then break; fi
    attempts=$((attempts + 1)); (( attempts < 3 )) || die "owner token collision"
  done
  timestamp=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  operation_id=$(python3 -c 'import secrets; print(secrets.token_hex(16))')
  write_handoff_intent "$dir" "$operation_id" "$old_agent" "$old_token" "$new_token" "$frozen_state" "$target_state" "$timestamp" \
    || die "manual handoff persistence failed; state unchanged"
  if ! append_manual_audit force-unfreeze-blocked-intent "$timestamp" "$operation_id" "$frozen_state" "$target_state" \
    "$old_agent" "$RECOVERY_AGENT" "$new_token"; then
    if [[ "${SLOT_REGISTRY_TEST_FAIL_UNFREEZE_STEP:-}" != audit-written ]]; then
      remove_handoff_dir "$dir"
      die "manual audit intent persistence failed; state unchanged"
    fi
    die "manual audit intent write interrupted; durable handoff requires resume"
  fi
  printf 'OWNER_TOKEN=%s\nOPERATION_ID=%s\nHANDOFF_PREPARED slot=%s task=%s recovery_agent=%s\n' \
    "$new_token" "$operation_id" "$SLOT" "$TASK" "$RECOVERY_AGENT"
}
resume_handoff_locked() {
  local max lock dir handoff_index=-1 i old_allow_missing_intent="$AUDIT_ALLOW_MISSING_INTENT" audit_rc
  AUDIT_ALLOW_MISSING_INTENT=1
  max=$(read_capacity)
  audit_registry "$max" || {
    audit_rc=$?
    AUDIT_ALLOW_MISSING_INTENT="$old_allow_missing_intent"
    return "$audit_rc"
  }
  AUDIT_ALLOW_MISSING_INTENT="$old_allow_missing_intent"
  require_slot_in_pool "$SLOT" "$max"; lock="$REG_ROOT/$SLOT.lock"; dir=$(handoff_dir)
  for i in "${!AUDIT_HANDOFF_SLOTS[@]}"; do
    if [[ "${AUDIT_HANDOFF_SLOTS[$i]}" == "$SLOT" ]]; then handoff_index=$i; break; fi
  done
  (( handoff_index >= 0 )) || die "no pending manual handoff for $SLOT"
  read_handoff_into_globals "$dir"
  [[ "$HANDOFF_OPERATION_ID" == "$OPERATION_ID" && "$HANDOFF_TASK" == "$TASK" && "$HANDOFF_BRANCH" == "$BRANCH" \
     && "$HANDOFF_CLAIM" == "${CLAIM,,}" && "$HANDOFF_RECOVERY_AGENT" == "$RECOVERY_AGENT" \
     && "$HANDOFF_NEW_TOKEN" == "$OWNER_TOKEN" && "$HANDOFF_OPERATOR" == "$OPERATOR" \
     && "$HANDOFF_REASON" == "$REASON" ]] || die "manual handoff resume identity mismatch"
  AGENT="$HANDOFF_OLD_AGENT"; OPERATOR="$HANDOFF_OPERATOR"; REASON="$HANDOFF_REASON"
  if ! value_exists "$HANDOFF_OPERATION_ID" "${AUDIT_MANUAL_INTENT_OPERATIONS[@]}"; then
    [[ "$(current_handoff_step "$lock")" == 0 ]] \
      || die "manual handoff without public intent has already mutated reservation"
    append_manual_audit force-unfreeze-blocked-intent "$HANDOFF_TIMESTAMP" "$HANDOFF_OPERATION_ID" \
      "$HANDOFF_FROM_STATE" "$HANDOFF_TARGET_STATE" "$HANDOFF_OLD_AGENT" "$HANDOFF_RECOVERY_AGENT" \
      "$HANDOFF_NEW_TOKEN" \
      || die "manual handoff intent recovery failed; reservation remains frozen"
    AUDIT_MANUAL_INTENT_OPERATIONS+=("$HANDOFF_OPERATION_ID")
  fi
  continue_handoff_locked "$lock" "$dir"
}
cmd_force_unfreeze_blocked() {
  parse_kv "$@"; need "$SLOT" --slot; need "$TASK" --task; need "$BRANCH" --branch; need "$CLAIM" --claim-sha; need "$AGENT" --agent
  need "$RECOVERY_AGENT" --recovery-agent; need "$OPERATOR" --operator; need "$REASON" --reason
  validate_field task "$TASK"; validate_branch "$BRANCH"; validate_claim "$CLAIM"; validate_field agent "$AGENT"; validate_field recovery-agent "$RECOVERY_AGENT"
  validate_field operator "$OPERATOR"; validate_field reason "$REASON"; with_registry_lock force_unfreeze_locked
}
cmd_resume_unfreeze_blocked() {
  parse_kv "$@"; need "$SLOT" --slot; need "$TASK" --task; need "$BRANCH" --branch; need "$CLAIM" --claim-sha
  need "$RECOVERY_AGENT" --recovery-agent; need "$OPERATION_ID" --operation-id; need "$OWNER_TOKEN" --owner-token
  need "$OPERATOR" --operator; need "$REASON" --reason
  validate_field task "$TASK"; validate_branch "$BRANCH"; validate_claim "$CLAIM"; validate_field recovery-agent "$RECOVERY_AGENT"
  validate_owner_token "$OWNER_TOKEN"; validate_field operation-id "$OPERATION_ID"; validate_field operator "$OPERATOR"; validate_field reason "$REASON"
  with_registry_lock resume_handoff_locked
}
release_locked() {
  local lock; prepare_owned_lock; lock="$OWNED_LOCK"; require_state "$lock" occupied; rm -rf -- "$lock"; printf 'OK release %s task=%s\n' "$SLOT" "$TASK"
}
cmd_release() { parse_kv "$@"; validate_normal_owner_args; with_registry_lock release_locked; }
rollback_locked() {
  local lock created; prepare_owned_lock; lock="$OWNED_LOCK"; require_state "$lock" reserved
  read_field_into created "$lock" created_local_branch || exit $?; case "$created" in true|false) ;; *) die "invalid created_local_branch field: $created" ;; esac
  rm -rf -- "$lock"; printf 'DELETE_LOCAL_BRANCH=%s\nOK rollback %s task=%s created_local_branch=%s\n' "$created" "$SLOT" "$TASK" "$created"
}
cmd_rollback() { parse_kv "$@"; validate_normal_owner_args; with_registry_lock rollback_locked; }

is_held_locked() {
  local max lock task state; max=$(read_capacity); audit_registry "$max"; require_slot_in_pool "$SLOT" "$max"; lock="$REG_ROOT/$SLOT.lock"
  if [[ -d "$lock" && ! -L "$lock" ]]; then read_field_into task "$lock" task_id || exit $?; read_field_into state "$lock" state || exit $?; printf 'HELD task=%s state=%s\n' "$task" "$state"; return 0; fi
  printf 'FREE\n'; return 1
}
cmd_is_held() { parse_kv "$@"; need "$SLOT" --slot; with_registry_lock is_held_locked; }
capacity_locked() { local max; max=$(read_capacity); audit_registry "$max"; printf 'max=%s held=%s\n' "$max" "${#AUDIT_SLOTS[@]}"; }
cmd_capacity() { with_registry_lock capacity_locked; }
json_string() { python3 -c 'import json,sys; print(json.dumps(sys.stdin.buffer.read().decode("utf-8"), ensure_ascii=False), end="")'; }
status_locked() {
  local max first=1 i; max=$(read_capacity); audit_registry "$max"
  if [[ $JSON -eq 1 ]]; then
    printf '{"max":%s,"held":%s,"slots":[' "$max" "${#AUDIT_SLOTS[@]}"
    for i in "${!AUDIT_SLOTS[@]}"; do
      [[ $first -eq 1 ]] || printf ','; first=0
      printf '{"slot":'; printf '%s' "${AUDIT_SLOTS[$i]}" | json_string
      printf ',"task_id":'; printf '%s' "${AUDIT_TASKS[$i]}" | json_string
      printf ',"branch":'; printf '%s' "${AUDIT_BRANCHES[$i]}" | json_string
      printf ',"claim_sha":'; printf '%s' "${AUDIT_CLAIMS[$i]}" | json_string
      printf ',"agent_id":'; printf '%s' "${AUDIT_AGENTS[$i]}" | json_string
      printf ',"state":'; printf '%s' "${AUDIT_STATES[$i]}" | json_string
      printf ',"created_local_branch":'; printf '%s' "${AUDIT_CREATED[$i]}" | json_string
      printf ',"reserved_at":'; printf '%s' "${AUDIT_RESERVED[$i]}" | json_string; printf '}'
    done
    printf ']}\n'; return 0
  fi
  printf 'capacity max=%s held=%s\n' "$max" "${#AUDIT_SLOTS[@]}"
  for i in "${!AUDIT_SLOTS[@]}"; do
    printf '  %s: task=%q state=%q branch=%q created_local=%q agent=%q\n' "${AUDIT_SLOTS[$i]}" "${AUDIT_TASKS[$i]}" "${AUDIT_STATES[$i]}" "${AUDIT_BRANCHES[$i]}" "${AUDIT_CREATED[$i]}" "${AUDIT_AGENTS[$i]}"
  done
}
cmd_status() { parse_kv "$@"; with_registry_lock status_locked; }
manual_report_locked() {
  local max lock task branch claim agent state created reserved handoff_root handoff operation recovery_agent from_state target_state operator timestamp progress
  max=$(read_capacity); audit_registry "$max"; require_slot_in_pool "$SLOT" "$max"; lock="$REG_ROOT/$SLOT.lock"
  [[ -d "$lock" && ! -L "$lock" ]] || die "slot not held"
  read_field_into task "$lock" task_id || exit $?; read_field_into branch "$lock" branch || exit $?
  read_field_into claim "$lock" claim_sha || exit $?; read_field_into agent "$lock" agent_id || exit $?
  read_field_into state "$lock" state || exit $?; read_field_into created "$lock" created_local_branch || exit $?
  read_field_into reserved "$lock" reserved_at || exit $?
  printf 'RECOVERY_MODE=manual-report-only\nslot=%q task=%q branch=%q claim_sha=%q agent=%q state=%q created_local=%q reserved_at=%q' \
    "$SLOT" "$task" "$branch" "$claim" "$agent" "$state" "$created" "$reserved"
  handoff_root="$REG_ROOT/$HANDOFF_DIR_NAME.lock"; handoff="$handoff_root/$SLOT"
  if [[ -d "$handoff" && ! -L "$handoff" ]]; then
    read_handoff_into_globals "$handoff"
    operation="$HANDOFF_OPERATION_ID"; recovery_agent="$HANDOFF_RECOVERY_AGENT"
    from_state="$HANDOFF_FROM_STATE"; target_state="$HANDOFF_TARGET_STATE"
    operator="$HANDOFF_OPERATOR"; timestamp="$HANDOFF_TIMESTAMP"
    progress=$(current_handoff_step "$lock")
    printf ' pending_handoff=true operation_id=%q recovery_agent=%q from_state=%q target_state=%q operator=%q timestamp=%q progress=%q\n' \
      "$operation" "$recovery_agent" "$from_state" "$target_state" "$operator" "$timestamp" "$progress"
  else
    printf ' pending_handoff=false\n'
  fi
  printf 'No state changed. Inspect the slot, reservation, and any durable handoff manually; no PID/liveness recovery is implemented.\n'
}
cmd_manual_report() { parse_kv "$@"; need "$SLOT" --slot; with_registry_lock manual_report_locked; }

case "$cmd" in
  init) cmd_init "$@" ;;
  acquire) cmd_acquire "$@" ;;
  mark-created-local) cmd_mark_created_local "$@" ;;
  occupy) cmd_occupy "$@" ;;
  freeze-blocked) cmd_freeze_blocked "$@" ;;
  force-unfreeze-blocked) cmd_force_unfreeze_blocked "$@" ;;
  resume-unfreeze-blocked) cmd_resume_unfreeze_blocked "$@" ;;
  release) cmd_release "$@" ;;
  rollback) cmd_rollback "$@" ;;
  manual-report) cmd_manual_report "$@" ;;
  is-held) cmd_is_held "$@" ;;
  capacity) cmd_capacity ;;
  status) cmd_status "$@" ;;
  ""|-h|--help)
    perl -ne 'next if $. == 1; if (/^#/) { s/^# ?//; print } else { exit }' "$0"
    exit 0 ;;
  *) die "unknown command: $cmd" ;;
esac
