#!/usr/bin/env bash

# Resolve a Cargo target root that cannot be shared by distinct checkouts.
# Cargo's Fresh result only describes source metadata, not which worktree owns
# the existing binary, so every checkout gets a deterministic private subdir.
bong_scoped_cargo_target() {
    local server_directory="${1:-}"
    local configured_target="${CARGO_TARGET_DIR:-/tmp/bong-target}"
    local checkout_id base_target

    server_directory="$(readlink -f -- "$server_directory")" || return 1
    [ -d "$server_directory" ] || return 1

    if [[ "$configured_target" = /* ]]; then
        base_target="$configured_target"
    else
        base_target="$server_directory/$configured_target"
    fi
    base_target="$(readlink -m -- "$base_target")" || return 1
    checkout_id="$(printf '%s\n' "$server_directory" | sha256sum | cut -c1-16)" || return 1
    [ "${#checkout_id}" -eq 16 ] || return 1

    printf '%s/bong-checkout-%s\n' "${base_target%/}" "$checkout_id"
}
