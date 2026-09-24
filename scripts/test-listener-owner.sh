#!/usr/bin/env bash
set -euo pipefail
umask 077

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
source "$ROOT/scripts/lib/bong-server-lifecycle.sh"
TEST_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/bong-listener-owner.XXXXXX")"
LISTENER_PID=""
FOREIGN_PID=""

cleanup() {
    for pid in "${LISTENER_PID:-}" "${FOREIGN_PID:-}"; do
        [[ "$pid" =~ ^[0-9]+$ ]] || continue
        kill -KILL "$pid" 2>/dev/null || true
        wait "$pid" 2>/dev/null || true
    done
    rm -rf -- "$TEST_ROOT"
}
trap cleanup EXIT

fail() {
    echo "FAIL: $*" >&2
    exit 1
}

listener="$TEST_ROOT/listener.py"
ready="$TEST_ROOT/listener.ready"
cat > "$listener" <<'PY'
#!/usr/bin/env python3
import signal
import socket
import sys

ready = sys.argv[1]
sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
sock.bind(("127.0.0.1", 0))
sock.listen()
with open(ready, "w", encoding="utf-8") as handle:
    handle.write(str(sock.getsockname()[1]))
while True:
    signal.pause()
PY
chmod +x "$listener"
"$listener" "$ready" &
LISTENER_PID=$!
for _ in $(seq 1 100); do
    [ -s "$ready" ] && break
    sleep 0.01
done
[ -s "$ready" ] || fail "listener fixture did not publish its port"
port="$(<"$ready")"
starttime="$(bong_server_process_starttime "$LISTENER_PID")"
identity="$(bong_server_process_executable_identity "$LISTENER_PID")"
pgrp="$(ps -o pgrp= -p "$LISTENER_PID" 2>/dev/null | tr -d '[:space:]')"

bong_server_port_is_open "$port" || fail "listener fixture port is not reachable"
bong_server_pinned_process_owns_ipv4_listener \
    "$LISTENER_PID" "$starttime" "$identity" "$port" "$pgrp" \
    || fail "exact pinned listener owner was not accepted"

sleep 30 &
FOREIGN_PID=$!
foreign_starttime="$(bong_server_process_starttime "$FOREIGN_PID")"
foreign_identity="$(bong_server_process_executable_identity "$FOREIGN_PID")"
foreign_pgrp="$(ps -o pgrp= -p "$FOREIGN_PID" 2>/dev/null | tr -d '[:space:]')"
bong_server_port_is_open "$port" || fail "foreign-owner fixture lost its reachable listener"
if bong_server_pinned_process_owns_ipv4_listener \
    "$FOREIGN_PID" "$foreign_starttime" "$foreign_identity" "$port" "$foreign_pgrp"; then
    fail "reachable foreign listener must not be attributed to the pinned non-owner"
fi

if bong_server_pinned_process_owns_ipv4_listener \
    "$LISTENER_PID" "$((starttime + 1))" "$identity" "$port" "$pgrp"; then
    fail "stale starttime must not own a listener"
fi
if bong_server_pinned_process_owns_ipv4_listener \
    "$LISTENER_PID" "$starttime" "$identity" "$((port + 1))" "$pgrp"; then
    fail "adjacent port must not match the pinned listener"
fi
if bong_server_pinned_process_owns_ipv4_listener \
    "$LISTENER_PID" "$starttime" "$identity" "$port" "$((pgrp + 1))"; then
    fail "wrong process-group pin must not own a listener"
fi

python3 - "$ROOT/scripts/lib/bong-listener-owner.py" <<'PY'
import importlib.util
import pathlib
import sys

module_path = pathlib.Path(sys.argv[1])
spec = importlib.util.spec_from_file_location("bong_listener_owner", module_path)
module = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(module)

assert module.listener_inodes_from_text(
    "sl local_address rem_address st tx_queue rx_queue tr tm->when retrnsmt uid timeout inode\n"
    "0: 0100007F:63DD 00000000:0000 0A 0:0 00:0 0 1000 0 4242\n",
    25565,
) == {"4242"}
assert module.listener_inodes_from_text(
    "sl local_address rem_address st tx_queue rx_queue tr tm->when retrnsmt uid timeout inode\n"
    "0: 00000000:63DD 00000000:0000 01 0:0 00:0 0 1000 0 4242\n",
    25565,
) == set()
assert module.listener_inodes_from_text(
    "sl local_address rem_address st tx_queue rx_queue tr tm->when retrnsmt uid timeout inode\n"
    "0: 0100007F:63DE 00000000:0000 0A 0:0 00:0 0 1000 0 4242\n",
    25565,
) == set()
try:
    module.listener_inodes_from_text("header\nmalformed\n", 25565)
except module.InspectionError:
    pass
else:
    raise AssertionError("malformed TCP rows must fail closed")
PY

printf 'PASS: exact pinned listener ownership rejects reachable foreign ports\n'
