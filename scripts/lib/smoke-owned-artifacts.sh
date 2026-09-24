#!/usr/bin/env bash
# Helpers for smoke harnesses that create a private Cargo target and/or install a
# server binary beside the run's retained logs. Source this file; do not execute
# it as a main script.

smoke_cleanup_owned_artifacts() {
  local run_dir="${1:-}"
  local target_dir="${2:-}"
  local server_binary="${3:-}"
  local run_root target_real binary_real path_real canonical_path current_root
  local unsafe_path=0

  # The run root is an ownership boundary, not an arbitrary rm base. It must be
  # the exact canonical directory created by this invocation; in particular,
  # '/' and symlink/relative aliases are never accepted.
  if [ -z "$run_dir" ] || [ "$run_dir" = "/" ] || [ -L "$run_dir" ] || [ ! -d "$run_dir" ]; then
    echo "[smoke-cleanup] refusing artifact cleanup: run directory is not a real non-root directory: $run_dir" >&2
    return 1
  fi
  run_root="$(realpath -e -- "$run_dir" 2>/dev/null)" || {
    echo "[smoke-cleanup] refusing artifact cleanup: cannot resolve run directory: $run_dir" >&2
    return 1
  }
  if [ "$run_root" = "/" ] || [ "$run_dir" != "$run_root" ]; then
    echo "[smoke-cleanup] refusing artifact cleanup: run directory is not canonical: $run_dir" >&2
    return 1
  fi

  # Resolve and type-check every requested deletion before removing either one.
  # realpath -m also rejects relative, '..', and symlink-parent aliases when the
  # candidate does not yet exist; -L catches final and dangling symlinks first.
  for path in "$target_dir" "$server_binary"; do
    [ -n "$path" ] || continue
    canonical_path="$(realpath -m -- "$path" 2>/dev/null)" || {
      echo "[smoke-cleanup] refusing artifact cleanup: cannot canonicalize path: $path" >&2
      unsafe_path=1
      continue
    }
    if [ "$path" != "$canonical_path" ]; then
      echo "[smoke-cleanup] refusing artifact cleanup: candidate is not canonical: $path" >&2
      unsafe_path=1
      continue
    fi
    if [ "$path" = "$run_root" ] || [ "$path" = "/" ]; then
      echo "[smoke-cleanup] refusing to remove root/run directory: $path" >&2
      unsafe_path=1
      continue
    fi
    case "$canonical_path" in
      "$run_root"/*) ;;
      *)
        echo "[smoke-cleanup] refusing artifact cleanup outside run directory: $canonical_path" >&2
        unsafe_path=1
        continue
        ;;
    esac
    if [ -L "$path" ]; then
      echo "[smoke-cleanup] refusing artifact cleanup: symlink path: $path" >&2
      unsafe_path=1
      continue
    fi
    [ -e "$path" ] || continue
    path_real="$(realpath -e -- "$path" 2>/dev/null)" || {
      echo "[smoke-cleanup] refusing artifact cleanup: cannot resolve path: $path" >&2
      unsafe_path=1
      continue
    }
    if [ "$path_real" != "$path" ]; then
      echo "[smoke-cleanup] refusing artifact cleanup: path changed to non-canonical target: $path" >&2
      unsafe_path=1
      continue
    fi
    if [ "$path" = "$target_dir" ] && [ ! -d "$path" ]; then
      echo "[smoke-cleanup] refusing target cleanup: expected a real directory: $path" >&2
      unsafe_path=1
      continue
    fi
    if [ "$path" = "$server_binary" ] && [ ! -f "$path" ]; then
      echo "[smoke-cleanup] refusing binary cleanup: expected a real file: $path" >&2
      unsafe_path=1
      continue
    fi
  done
  [ "$unsafe_path" -eq 0 ] || return 1

  # Revalidate the original root and the exact candidate immediately before each
  # rm. The rm argument is the newly resolved canonical child, never caller text.
  if [ -n "$target_dir" ] && [ -e "$target_dir" -o -L "$target_dir" ]; then
    if [ -L "$run_dir" ] || [ ! -d "$run_dir" ]; then
      echo "[smoke-cleanup] refusing target cleanup: run directory changed before removal" >&2
      return 1
    fi
    current_root="$(realpath -e -- "$run_dir" 2>/dev/null)" || return 1
    if [ "$current_root" != "$run_root" ] || [ "$current_root" = "/" ]; then
      echo "[smoke-cleanup] refusing target cleanup: run directory changed before removal" >&2
      return 1
    fi
    if [ -L "$target_dir" ] || [ ! -d "$target_dir" ]; then
      echo "[smoke-cleanup] refusing target replacement: expected a real directory: $target_dir" >&2
      return 1
    fi
    target_real="$(realpath -e -- "$target_dir" 2>/dev/null)" || return 1
    if [ "$target_real" != "$target_dir" ]; then
      echo "[smoke-cleanup] refusing target cleanup: candidate changed from canonical path" >&2
      return 1
    fi
    case "$target_real" in
      "$run_root"/*) ;;
      *)
        echo "[smoke-cleanup] refusing to remove target outside run directory: $target_real" >&2
        return 1
        ;;
    esac
    if [ "$target_real" = "$run_root" ] || [ "$target_real" = "/" ] || [ -L "$target_real" ]; then
      echo "[smoke-cleanup] refusing target canonical-path replacement: $target_real" >&2
      return 1
    fi
    if ! rm -rf -- "$target_real"; then
      echo "[smoke-cleanup] failed to remove run-private target: $target_real" >&2
      return 1
    fi
  fi

  if [ -n "$server_binary" ] && [ -e "$server_binary" -o -L "$server_binary" ]; then
    if [ -L "$run_dir" ] || [ ! -d "$run_dir" ]; then
      echo "[smoke-cleanup] refusing binary cleanup: run directory changed before removal" >&2
      return 1
    fi
    current_root="$(realpath -e -- "$run_dir" 2>/dev/null)" || return 1
    if [ "$current_root" != "$run_root" ] || [ "$current_root" = "/" ]; then
      echo "[smoke-cleanup] refusing binary cleanup: run directory changed before removal" >&2
      return 1
    fi
    if [ -L "$server_binary" ] || [ ! -f "$server_binary" ]; then
      echo "[smoke-cleanup] refusing binary replacement: expected a real file: $server_binary" >&2
      return 1
    fi
    binary_real="$(realpath -e -- "$server_binary" 2>/dev/null)" || return 1
    if [ "$binary_real" != "$server_binary" ]; then
      echo "[smoke-cleanup] refusing binary cleanup: candidate changed from canonical path" >&2
      return 1
    fi
    case "$binary_real" in
      "$run_root"/*) ;;
      *)
        echo "[smoke-cleanup] refusing to remove binary outside run directory: $binary_real" >&2
        return 1
        ;;
    esac
    if [ "$binary_real" = "$run_root" ] || [ "$binary_real" = "/" ] || [ -L "$binary_real" ]; then
      echo "[smoke-cleanup] refusing binary canonical-path replacement: $binary_real" >&2
      return 1
    fi
    if ! rm -f -- "$binary_real"; then
      echo "[smoke-cleanup] failed to remove run-private server binary: $binary_real" >&2
      return 1
    fi
  fi
}
