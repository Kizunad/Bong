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

printf 'final\n' >>"$log"
echo "smoke-owned-artifacts adversarial checks PASS"
