#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BASE_REF="${BASE_REF:?BASE_REF is required}"

git -C "$ROOT" fetch --no-tags --depth=1 origin \
  "$BASE_REF:refs/remotes/origin/$BASE_REF"
base_commit="$(git -C "$ROOT" rev-parse --verify "refs/remotes/origin/$BASE_REF^{commit}")"
proto_type="$(git -C "$ROOT" cat-file -t "$base_commit:proto" 2>/dev/null || true)"

if [ "$proto_type" = "tree" ]; then
  (
    cd "$ROOT/proto"
    buf breaking --against "../.git#ref=$base_commit,subdir=proto"
  )
elif [ -z "$proto_type" ]; then
  echo "proto/ not found on verified base commit $base_commit — skipping breaking check (first PR)"
else
  echo "verified base path proto has unexpected git object type: $proto_type" >&2
  exit 1
fi
