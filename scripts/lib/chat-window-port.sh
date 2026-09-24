#!/usr/bin/env bash
# Shared helpers for chat-signal-window e2e port ownership checks.
# Source this file; do not execute it as a main script.

# Injected seam for unit tests. Defaults to system `ss`.
CHAT_WINDOW_SS_CMD="${CHAT_WINDOW_SS_CMD:-ss}"

query_port_listeners() {
  local port="$1"
  local output
  local status

  # Fail closed: if ss itself fails, callers must not treat the port as owned.
  set +e
  output="$("$CHAT_WINDOW_SS_CMD" -4 -H -ltnp "sport = :$port" 2>/dev/null)"
  status=$?
  set -e
  if [ "$status" -ne 0 ]; then
    echo "[chat-window-port] ss failed for port=${port} cmd=${CHAT_WINDOW_SS_CMD} status=${status}" >&2
    return 1
  fi
  printf '%s\n' "$output"
}

extract_listener_pids() {
  local listeners_output="$1"
  printf '%s\n' "$listeners_output" \
    | grep -oE 'pid=[0-9]+' \
    | cut -d= -f2 \
    | sort -u
}

pid_belongs_to_tree() {
  local candidate="$1"
  local root_pid="$2"
  while [[ "$candidate" =~ ^[0-9]+$ ]] && [ "$candidate" -gt 1 ]; do
    [ "$candidate" = "$root_pid" ] && return 0
    candidate="$(
      awk '/^PPid:/ { print $2; exit }' "/proc/$candidate/status" 2>/dev/null || true
    )"
  done
  return 1
}

port_owned_by_tree() {
  local root_pid="$1"
  local port="$2"
  local listeners_output
  local listener_pid

  if ! listeners_output="$(query_port_listeners "$port")"; then
    return 1
  fi

  while IFS= read -r listener_pid; do
    if [ -n "$listener_pid" ] && pid_belongs_to_tree "$listener_pid" "$root_pid"; then
      return 0
    fi
  done < <(extract_listener_pids "$listeners_output")
  return 1
}
