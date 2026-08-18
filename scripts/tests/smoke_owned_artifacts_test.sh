#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
# shellcheck source=../lib/smoke-owned-artifacts.sh
source "$ROOT/scripts/lib/smoke-owned-artifacts.sh"

for harness in "$ROOT/scripts/smoke-offscreen-war.sh" "$ROOT/scripts/smoke-tiandao-fullstack.sh"; do
  grep -Fq 'source "$ROOT/scripts/lib/smoke-owned-artifacts.sh"' "$harness" \
    || { echo "FAIL: helper is not sourced by $harness" >&2; exit 1; }
  grep -Fq 'EVIDENCE_DIR="$(realpath -e -- "$EVIDENCE_DIR")"' "$harness" \
    || { echo "FAIL: evidence root is not canonicalized by $harness" >&2; exit 1; }
  grep -Fq 'case "$RUN_DIR" in' "$harness" \
    || { echo "FAIL: strict evidence-child guard is missing from $harness" >&2; exit 1; }
  grep -Fq 'smoke_cleanup_owned_artifacts "$RUN_DIR"' "$harness" \
    || { echo "FAIL: exact RUN_DIR cleanup is missing from $harness" >&2; exit 1; }
done

grep -Fq 'trap - EXIT' "$ROOT/scripts/smoke-offscreen-war.sh" \
  || { echo "FAIL: offscreen cleanup does not disable recursive EXIT trap" >&2; exit 1; }
grep -Fq 'trap - EXIT' "$ROOT/scripts/smoke-tiandao-fullstack.sh" \
  || { echo "FAIL: fullstack cleanup does not disable recursive EXIT trap" >&2; exit 1; }
grep -Fq 'SERVER_BINARY="$RUN_DIR/bong-server"' "$ROOT/scripts/smoke-offscreen-war.sh" \
  || { echo "FAIL: offscreen binary is not an exact RUN_DIR child" >&2; exit 1; }
grep -Fq 'FULLSTACK_SERVER_BINARY="$RUN_DIR/bong-server"' "$ROOT/scripts/smoke-tiandao-fullstack.sh" \
  || { echo "FAIL: fullstack binary is not an exact RUN_DIR child" >&2; exit 1; }
grep -Fq 'CARGO_TARGET_ROOT="$RUN_DIR/bong-target"' "$ROOT/scripts/smoke-offscreen-war.sh" \
  || { echo "FAIL: offscreen target is not an exact RUN_DIR child" >&2; exit 1; }
grep -Fq 'FULLSTACK_CARGO_TARGET_ROOT="$RUN_DIR/bong-target"' "$ROOT/scripts/smoke-tiandao-fullstack.sh" \
  || { echo "FAIL: fullstack target is not an exact RUN_DIR child" >&2; exit 1; }
if grep -Fq 'smoke_cleanup_owned_artifacts "$RUN_DIR" "$RUN_DIR/server.log"' "$ROOT/scripts/smoke-offscreen-war.sh"; then
  echo "FAIL: offscreen logs became cleanup candidates" >&2
  exit 1
fi

# bot-e2e 的清理契约：run dir 是 EVIDENCE_DIR（mktemp 直建、无独立 RUN_DIR 再层叠）。
# 它必须在 mktemp 之后、任何基于它的子目录创建之前归一化，cleanup 以 "$EVIDENCE_DIR"
# 精确调用 helper；fallback 模式的簇断言只接受 BOT_E2E_RUN_TAG=ci，须在启动前拒绝默认
# PID 派生 run tag（review finding：默认自调用晚失败）。
bot_e2e="$ROOT/scripts/bot-e2e.sh"
grep -Fq 'source "$ROOT/scripts/lib/smoke-owned-artifacts.sh"' "$bot_e2e" \
  || { echo "FAIL: helper is not sourced by bot-e2e.sh" >&2; exit 1; }
grep -Fq 'smoke_cleanup_owned_artifacts "$EVIDENCE_DIR"' "$bot_e2e" \
  || { echo "FAIL: bot-e2e cleanup does not use exact EVIDENCE_DIR" >&2; exit 1; }
canon_line="$(grep -n -F 'EVIDENCE_DIR="$(realpath -e -- "$EVIDENCE_DIR")"' "$bot_e2e" | cut -d: -f1)"
mktemp_line="$(grep -n -F 'EVIDENCE_DIR="$(mktemp -d "$EVIDENCE_ROOT/run.XXXXXXXXXX")"' "$bot_e2e" | cut -d: -f1)"
[ -n "$canon_line" ] && [ -n "$mktemp_line" ] \
  || { echo "FAIL: bot-e2e canonicalization/mktemp line is missing" >&2; exit 1; }
[ "$canon_line" -gt "$mktemp_line" ] \
  || { echo "FAIL: bot-e2e canonicalizes EVIDENCE_DIR before creating it" >&2; exit 1; }
first_child_line="$(grep -n -F 'SERVER_RUNTIME_DIR="$(mktemp -d "$EVIDENCE_DIR/server-runtime.XXXXXX")"' "$bot_e2e" | head -1 | cut -d: -f1)"
[ -n "$first_child_line" ] && [ "$canon_line" -lt "$first_child_line" ] \
  || { echo "FAIL: bot-e2e creates EVIDENCE_DIR children before canonicalizing" >&2; exit 1; }
grep -Fq 'if [ "$FALLBACK_MODE" = "1" ] && [ "$BOT_E2E_RUN_TAG" != "ci" ]; then' "$bot_e2e" \
  || { echo "FAIL: fallback mode does not preflight BOT_E2E_RUN_TAG=ci before startup" >&2; exit 1; }

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

TMP_ROOT="$(mktemp -d /tmp/bong-smoke-owned-artifacts-test.XXXXXX)"
RM_BIN="$(command -v rm)"
trap '"$RM_BIN" -rf -- "$TMP_ROOT"' EXIT

run_dir="$TMP_ROOT/run"
outside="$TMP_ROOT/outside"
mkdir -p "$run_dir" "$outside"
run_dir="$(realpath -e -- "$run_dir")"
log="$run_dir/server.log"
rm_bin="$(command -v rm)"

make_artifacts() {
  mkdir -p "$run_dir/bong-target"
  printf 'target\n' >"$run_dir/bong-target/object"
  printf '#!/bin/sh\n' >"$run_dir/bong-server"
  printf 'log evidence\n' >"$log"
}

make_artifacts

if smoke_cleanup_owned_artifacts "" "$run_dir/bong-target" "$run_dir/bong-server"; then
  fail "empty run root was accepted"
fi
if smoke_cleanup_owned_artifacts / "" ""; then
  fail "filesystem root was accepted"
fi
if smoke_cleanup_owned_artifacts "$run_dir" "$run_dir" ""; then
  fail "run root deletion was accepted"
fi
alias_root="$TMP_ROOT/run-alias"
ln -s "$run_dir" "$alias_root"
if smoke_cleanup_owned_artifacts "$alias_root" "$run_dir/bong-target" ""; then
  fail "symlink run root was accepted"
fi
if smoke_cleanup_owned_artifacts "$run_dir" "$run_dir/../run/bong-target" ""; then
  fail "non-canonical candidate path was accepted"
fi

printf 'outside target\n' >"$outside/bong-target"
if smoke_cleanup_owned_artifacts "$run_dir" "$outside/bong-target" "$run_dir/bong-server"; then
  fail "outside target was accepted"
fi
[ -e "$outside/bong-target" ] || fail "outside target was deleted"
[ -f "$run_dir/bong-server" ] || fail "valid binary was removed before all candidates validated"

ln -s "$outside" "$run_dir/symlink-target"
if smoke_cleanup_owned_artifacts "$run_dir" "$run_dir/symlink-target" ""; then
  fail "symlink target was accepted"
fi
[ -e "$outside/bong-target" ] || fail "symlink target contents were deleted"

ln -s "$outside/missing-target" "$run_dir/dangling-target"
if smoke_cleanup_owned_artifacts "$run_dir" "$run_dir/dangling-target" ""; then
  fail "dangling symlink target was accepted"
fi

ln -s "$outside/bong-target" "$run_dir/symlink-server"
if smoke_cleanup_owned_artifacts "$run_dir" "" "$run_dir/symlink-server"; then
  fail "symlink binary was accepted"
fi
[ -e "$outside/bong-target" ] || fail "symlink binary target was deleted"

if smoke_cleanup_owned_artifacts "$run_dir" "$run_dir" "$run_dir/bong-server"; then
  fail "run root candidate was accepted"
fi
if smoke_cleanup_owned_artifacts "$run_dir" "/" ""; then
  fail "root target candidate was accepted"
fi

smoke_cleanup_owned_artifacts "$run_dir" "$run_dir/bong-target" "$run_dir/bong-server" \
  || fail "valid run-private cleanup was rejected"
[ ! -e "$run_dir/bong-target" ] || fail "valid target survived cleanup"
[ ! -e "$run_dir/bong-server" ] || fail "valid binary survived cleanup"
[ -f "$log" ] || fail "retained log was deleted by valid cleanup"

# Deterministically inject a target rm -rf failure without relying on uid or filesystem mode.
make_artifacts
rm() {
  if [ "${1:-}" = "-rf" ]; then
    return 91
  fi
  "$rm_bin" "$@"
}
if smoke_cleanup_owned_artifacts "$run_dir" "$run_dir/bong-target" "$run_dir/bong-server"; then
  unset -f rm
  fail "injected rm -rf failure was not propagated"
fi
unset -f rm
[ -d "$run_dir/bong-target" ] || fail "target disappeared despite injected rm -rf failure"
[ -f "$run_dir/bong-server" ] || fail "binary was removed after target rm -rf failure"
[ -f "$log" ] || fail "log disappeared after target rm -rf failure"

# Deterministically inject a binary rm -f failure after allowing target removal.
rm() {
  if [ "${1:-}" = "-f" ]; then
    return 92
  fi
  "$rm_bin" "$@"
}
if smoke_cleanup_owned_artifacts "$run_dir" "$run_dir/bong-target" "$run_dir/bong-server"; then
  unset -f rm
  fail "injected rm -f failure was not propagated"
fi
unset -f rm
[ ! -e "$run_dir/bong-target" ] || fail "target survived successful rm -rf before binary failure"
[ -f "$run_dir/bong-server" ] || fail "binary disappeared despite injected rm -f failure"
[ -f "$log" ] || fail "log disappeared after binary rm -f failure"


# bot-e2e 经符号链接 checkout 启动的回归形状（review finding）：mktemp 返回的文本路径
# 带别名祖先（目录本身不是 symlink），helper 因 run dir != realpath 拒绝清理并把整轮
# 场景变成 exit 1；harness 紧随 mktemp 的 realpath 归一化才是可清理路径。用等价运行
# 形状复现：归一化前 raw 路径必须被拒，归一化后 run-private 清理必须通过。
real_root="$TMP_ROOT/real-root"
linked_root="$TMP_ROOT/linked-root"
mkdir -p "$real_root"
ln -s "$real_root" "$linked_root"
alias_run="$(mktemp -d "$linked_root/run.XXXXXXXXXX")"
if smoke_cleanup_owned_artifacts "$alias_run" "$alias_run/bong-target" ""; then
  fail "pre-canonicalization alias-ancestor run dir was accepted"
fi
canonical_run="$(realpath -e -- "$alias_run")"
mkdir -p "$canonical_run/bong-target"
printf 'target\n' >"$canonical_run/bong-target/object"
printf '#!/bin/sh\n' >"$canonical_run/bong-server"
printf 'log evidence\n' >"$canonical_run/server.log"
smoke_cleanup_owned_artifacts "$canonical_run" "$canonical_run/bong-target" "$canonical_run/bong-server" \
  || fail "post-canonicalization run-private cleanup was rejected"
[ ! -e "$canonical_run/bong-target" ] || fail "canonicalized target survived cleanup"
[ ! -e "$canonical_run/bong-server" ] || fail "canonicalized binary survived cleanup"
[ -f "$canonical_run/server.log" ] || fail "canonicalized log was deleted by cleanup"

printf 'final\n' >>"$log"
echo "smoke-owned-artifacts adversarial checks PASS"
