#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
source "$ROOT/scripts/lib/bong-cargo-target.sh"

TMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/bong-cargo-target.XXXXXX")"
trap 'rm -rf -- "$TMP_ROOT"' EXIT
mkdir -p "$TMP_ROOT/checkout-a/server" "$TMP_ROOT/checkout-b/server"

target_a="$(CARGO_TARGET_DIR="$TMP_ROOT/shared" bong_scoped_cargo_target "$TMP_ROOT/checkout-a/server")"
target_a_again="$(CARGO_TARGET_DIR="$TMP_ROOT/shared" bong_scoped_cargo_target "$TMP_ROOT/checkout-a/server")"
target_b="$(CARGO_TARGET_DIR="$TMP_ROOT/shared" bong_scoped_cargo_target "$TMP_ROOT/checkout-b/server")"

[ "$target_a" = "$target_a_again" ] \
  || { echo "same checkout must resolve to a stable target root" >&2; exit 1; }
[ "$target_a" != "$target_b" ] \
  || { echo "distinct checkouts must not share a target root" >&2; exit 1; }

relative_target="$(CARGO_TARGET_DIR=relative-target bong_scoped_cargo_target "$TMP_ROOT/checkout-a/server")"
case "$relative_target" in
  "$TMP_ROOT/checkout-a/server/relative-target/bong-checkout-")
    ;;
  "$TMP_ROOT/checkout-a/server/relative-target/bong-checkout-"*)
    ;;
  *)
    echo "relative target must remain rooted under the server checkout: $relative_target" >&2
    exit 1
    ;;
esac

echo "cargo target scope contract: PASS"
