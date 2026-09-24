#!/usr/bin/env bash
# Contract tests for scripts/lib/chat-window-port.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/chat-window-port.sh
source "$ROOT/scripts/lib/chat-window-port.sh"

pass=0
fail=0

assert_ok() {
  local name="$1"
  shift
  if "$@"; then
    echo "PASS $name"
    pass=$((pass + 1))
  else
    echo "FAIL $name" >&2
    fail=$((fail + 1))
  fi
}

assert_fail() {
  local name="$1"
  shift
  if "$@"; then
    echo "FAIL $name (expected non-zero)" >&2
    fail=$((fail + 1))
  else
    echo "PASS $name"
    pass=$((pass + 1))
  fi
}

# 1) ss failure must fail closed
CHAT_WINDOW_SS_CMD=false
assert_fail "ss failure fails closed" port_owned_by_tree "$$" 65530

# 2) extract_listener_pids parses pid= tokens
pids="$(extract_listener_pids $'users:(("cargo",pid=4242,fd=12))\nusers:(("x",pid=4242,fd=1))\nusers:(("y",pid=7,fd=3))')"
expected=$'4242\n7'
if [ "$pids" = "$expected" ]; then
  echo "PASS extract_listener_pids unique sorted"
  pass=$((pass + 1))
else
  echo "FAIL extract_listener_pids got=$(printf %q "$pids")" >&2
  fail=$((fail + 1))
fi

# 3) pid_belongs_to_tree: self belongs to self
assert_ok "pid belongs to self" pid_belongs_to_tree "$$" "$$"

# 4) pid_belongs_to_tree: unrelated pid does not belong
assert_fail "unrelated pid rejected" pid_belongs_to_tree 1 "$$"

# 5) port_owned_by_tree with injected ss listing our pid
CHAT_WINDOW_SS_CMD="$(mktemp)"
cat >"$CHAT_WINDOW_SS_CMD" <<SS
#!/usr/bin/env bash
echo 'LISTEN 0 128 127.0.0.1:65530 0.0.0.0:* users:(("test",pid='"$$"',fd=3))'
SS
chmod +x "$CHAT_WINDOW_SS_CMD"
assert_ok "owned when listener pid is self" port_owned_by_tree "$$" 65530

# 6) listener pid outside tree is not owned
cat >"$CHAT_WINDOW_SS_CMD" <<'SS'
#!/usr/bin/env bash
echo 'LISTEN 0 128 127.0.0.1:65530 0.0.0.0:* users:(("test",pid=1,fd=3))'
SS
assert_fail "foreign listener pid rejected" port_owned_by_tree "$$" 65530
rm -f "$CHAT_WINDOW_SS_CMD"
unset CHAT_WINDOW_SS_CMD
CHAT_WINDOW_SS_CMD="${CHAT_WINDOW_SS_CMD:-ss}"

echo "chat-window-port tests: $pass passed, $fail failed"
if [ "$fail" -ne 0 ]; then
  exit 1
fi
