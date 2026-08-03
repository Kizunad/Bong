#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
TMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/bong-preview-harness.XXXXXX")"
cleanup() {
  BONG_PREVIEW_PID_FILE="$TMP_ROOT/runtime/server.pid" \
    bash "$REPO_ROOT/scripts/preview/stop-server-headless.sh" >/dev/null 2>&1 || true
  rm -rf -- "$TMP_ROOT"
}
trap cleanup EXIT

mkdir -p "$TMP_ROOT/runtime" "$TMP_ROOT/target/release" "$TMP_ROOT/bin" "$TMP_ROOT/fake-src"
chmod 700 "$TMP_ROOT/runtime" "$TMP_ROOT/bin" "$TMP_ROOT/fake-src"
cat >"$TMP_ROOT/fake-src/server.c" <<'EOF'
#include <arpa/inet.h>
#include <signal.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/socket.h>

int main(void) {
    const char *mode = getenv("FAKE_SERVER_MODE");
    if (mode && strcmp(mode, "early") == 0) return 17;
    if (mode && strcmp(mode, "no_listener") == 0) { sleep(30); return 0; }
    int fd = socket(AF_INET, SOCK_STREAM, 0);
    if (fd < 0) return 18;
    int one = 1;
    setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &one, sizeof(one));
    struct sockaddr_in addr = {0};
    addr.sin_family = AF_INET;
    addr.sin_port = htons(25565);
    addr.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    if (bind(fd, (struct sockaddr *)&addr, sizeof(addr)) < 0) return 19;
    if (listen(fd, 4) < 0) return 20;
    for (;;) pause();
}
EOF
# Compile a native executable so /proc/<pid>/exe matches the selected artifact.
gcc "$TMP_ROOT/fake-src/server.c" -o "$TMP_ROOT/target/release/bong-server"
cat >"$TMP_ROOT/bin/cargo" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
exit 0
EOF
chmod 700 "$TMP_ROOT/bin/cargo"

export PATH="$TMP_ROOT/bin:$PATH"
export CARGO_TARGET_DIR="$TMP_ROOT/target"
export BONG_BUILD_TOKEN_TEST_MODE=1
export BONG_BUILD_TOKEN_DIR="$TMP_ROOT/build-token"
export BONG_PREVIEW_PID_FILE="$TMP_ROOT/runtime/server.pid"
export BONG_SKIP_SKIN_PREFETCH=1

run_preview() {
  bash "$REPO_ROOT/scripts/preview/run-server-headless.sh" --timeout "$1"
}

run_preview 5 >/dev/null
[ -f "$BONG_PREVIEW_PID_FILE" ]
bash "$REPO_ROOT/scripts/preview/stop-server-headless.sh"
[ ! -e "$BONG_PREVIEW_PID_FILE" ] && [ ! -L "$BONG_PREVIEW_PID_FILE" ]

export FAKE_SERVER_MODE=early
if run_preview 2 >"$TMP_ROOT/early.log" 2>&1; then
  echo "early-exit preview unexpectedly succeeded" >&2
  exit 1
fi
[ ! -e "$BONG_PREVIEW_PID_FILE" ] && [ ! -L "$BONG_PREVIEW_PID_FILE" ]
unset FAKE_SERVER_MODE

export FAKE_SERVER_MODE=no_listener
if run_preview 1 >"$TMP_ROOT/timeout.log" 2>&1; then
  echo "timeout preview unexpectedly succeeded" >&2
  exit 1
fi
[ ! -e "$BONG_PREVIEW_PID_FILE" ] && [ ! -L "$BONG_PREVIEW_PID_FILE" ]
unset FAKE_SERVER_MODE

REAL_STAT="$(command -v stat)"
cat >"$TMP_ROOT/bin/stat" <<EOF
#!/usr/bin/env bash
set -euo pipefail
if [ "\${FAKE_STAT_IDENTITY_FAILURE:-0}" = "1" ] && [[ "\${*: -1}" == /proc/*/exe ]]; then
  count=0
  [ ! -f "$TMP_ROOT/stat-count" ] || read -r count <"$TMP_ROOT/stat-count"
  count=\$((count + 1))
  printf '%s\n' "\$count" >"$TMP_ROOT/stat-count"
  if [ "\$count" -eq 3 ]; then exit 1; fi
fi
exec "$REAL_STAT" "\$@"
EOF
chmod 700 "$TMP_ROOT/bin/stat"
export FAKE_STAT_IDENTITY_FAILURE=1
if run_preview 2 >"$TMP_ROOT/identity.log" 2>&1; then
  echo "identity-inspection failure unexpectedly succeeded" >&2
  exit 1
fi
unset FAKE_STAT_IDENTITY_FAILURE
rm -f "$TMP_ROOT/stat-count"
[ ! -e "$BONG_PREVIEW_PID_FILE" ] && [ ! -L "$BONG_PREVIEW_PID_FILE" ]

cat >"$TMP_ROOT/bin/python3" <<EOF
#!/usr/bin/env bash
set -euo pipefail
if [[ "\$*" == *"bong-listener-owner.py"* ]]; then exit 2; fi
exec "$(command -v python3)" "\$@"
EOF
chmod 700 "$TMP_ROOT/bin/python3"
if run_preview 2 >"$TMP_ROOT/owner.log" 2>&1; then
  echo "listener-owner inspection failure unexpectedly succeeded" >&2
  exit 1
fi
[ ! -e "$BONG_PREVIEW_PID_FILE" ] && [ ! -L "$BONG_PREVIEW_PID_FILE" ]

echo "preview lifecycle harness: PASS"
