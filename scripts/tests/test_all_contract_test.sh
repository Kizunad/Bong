#!/usr/bin/env bash
# test-all.sh 的外置 contract pin；使用临时 fixture，不启动 Redis/LLM/Gradle/Cargo。
set -uo pipefail

TEST_DIR="$(cd -- "$(dirname -- "$0")" && pwd -P)"
ROOT="$(cd -- "$TEST_DIR/../.." && pwd -P)"
SCRIPT="$ROOT/scripts/test-all.sh"
SANDBOX="$(mktemp -d /tmp/test-all-contract.XXXXXX)"
trap 'rm -rf -- "$SANDBOX"' EXIT

PASS=0
FAIL=0
LAST_RC=0
LAST_OUT=""
LAST_ERR=""

pass() {
    printf '  PASS: %s\n' "$1"
    PASS=$((PASS + 1))
}

fail() {
    printf '  FAIL: %s\n' "$1"
    FAIL=$((FAIL + 1))
}

run_capture() {
    local out="$1" err="$2"
    shift 2
    "$@" >"$out" 2>"$err"
    LAST_RC=$?
    LAST_OUT="$out"
    LAST_ERR="$err"
}

assert_rc() {
    local expected="$1" message="$2"
    if [[ "$LAST_RC" -eq "$expected" ]]; then pass "$message"; else fail "$message (expected=$expected actual=$LAST_RC)"; fi
}

assert_file() {
    local path="$1" message="$2"
    if [[ -f "$path" ]]; then pass "$message"; else fail "$message (missing=$path)"; fi
}

assert_contains() {
    local path="$1" needle="$2" message="$3"
    if grep -Fq -- "$needle" "$path"; then pass "$message"; else fail "$message (needle=$needle)"; fi
}

printf '=== test-all.sh contract ===\n'

run_capture "$SANDBOX/help.out" "$SANDBOX/help.err" bash "$SCRIPT" --help
assert_rc 0 '--help 返回 0'
assert_contains "$SANDBOX/help.out" '--profile unit|contract|full|e2e|preview' '--help 暴露 profile 契约'
assert_contains "$SANDBOX/help.out" '--list' '--help 暴露 list 契约'

assert_contains "$SCRIPT" "xvfb-run -a --server-args='-screen 0 1280x720x24'" 'preview client 使用固定 xvfb 屏幕参数'
assert_contains "$SCRIPT" 'elif ! need_command xvfb-run; then' 'preview 缺 xvfb-run 时显式 SKIP'
assert_contains "$SCRIPT" 'trap preview_exit_cleanup EXIT' 'preview 注册 EXIT cleanup'
assert_contains "$SCRIPT" "trap 'preview_signal_cleanup 130' INT" 'preview 注册 SIGINT cleanup'
assert_contains "$SCRIPT" "trap 'preview_signal_cleanup 143' TERM" 'preview 注册 SIGTERM cleanup'
assert_contains "$SCRIPT" 'stop-server-headless.sh' 'preview cleanup 调用身份安全停服 wrapper'
assert_contains "$SCRIPT" 'BONG_PREVIEW_LOG_FILE="$REPORT_DIR/preview-server.log"' 'preview server 日志进入 run-private 目录'
assert_contains "$ROOT/scripts/preview/run-server-headless.sh" 'BONG_PREVIEW_LOG_FILE:-/tmp/bong-preview-server.log' 'preview wrapper 保留可覆盖的历史默认日志路径'
assert_contains "$SCRIPT" 'preview_launcher_pid=$!' 'preview 记录 wrapper child PID'
assert_contains "$SCRIPT" 'kill "-$signal_name" "$preview_launcher_pid"' 'preview signal cleanup 转发到 wrapper child'
assert_contains "$SCRIPT" 'if BONG_PREVIEW_PID_FILE="$REPORT_DIR/preview-server.pid"' 'preview 仅在 stop 成功分支处理 authority cleanup'
assert_contains "$SCRIPT" 'preview_exit_cleanup_active=1' 'preview EXIT cleanup 有重入保护'
assert_contains "$SCRIPT" 'BONG_PREVIEW_LAUNCH_STATE_FILE="$preview_launch_state_file"' 'preview 显式传递 authority handoff 状态文件'
assert_contains "$SCRIPT" 'if preview_launch_state_published; then' 'preview 非零 wrapper 仍识别已发布 authority'
assert_contains "$ROOT/scripts/preview/run-server-headless.sh" 'bong_server_publish_launch_state' 'preview wrapper 发布 authority handoff marker'
assert_contains "$SCRIPT" 'launcher_status == PREVIEW_HANDOFF_FAILURE_EXIT' 'preview marker fallback 绑定 wrapper 专用失败码'
assert_contains "$ROOT/scripts/preview/run-server-headless.sh" 'BONG_PREVIEW_HANDOFF_FAILURE_EXIT=75' 'preview handoff failure 使用独立退出码'
if grep -Fq 'bong_server_post_publication_rollback "preview launch-state publication rollback" || true' \
    "$ROOT/scripts/preview/run-server-headless.sh"; then
    fail 'launch-state publication rollback 不得吞掉失败'
else
    pass 'launch-state publication rollback 显式传播未确认状态'
fi
assert_contains "$SCRIPT" 'preview_authority_record_present' 'preview marker 失败时仍交给 identity-safe stop 清理'
if grep -Fq 'local failed=0 server_started=0 preview_launcher_pid=0' "$SCRIPT"; then
    fail 'preview cleanup 状态不得在管道函数中声明为局部变量'
else
    pass 'preview cleanup 状态跨越管道函数返回保持可用'
fi
if awk '
    /^    preview_exit_cleanup\(\) \{/ { in_cleanup=1; next }
    in_cleanup && /^    preview_signal_cleanup\(\) \{/ {
        exit (cleanup_seen && !trap_after) ? 0 : 1
    }
    in_cleanup && /preview_cleanup "\$exit_code"/ { cleanup_seen=1; next }
    in_cleanup && cleanup_seen && /trap - EXIT INT TERM/ { trap_after=1 }
    END { if (cleanup_seen && !trap_after) exit 0; exit 1 }
' "$SCRIPT"; then
    pass 'preview EXIT cleanup 在 stop 尝试后才解除 trap'
else
    fail 'preview EXIT cleanup 不得在 cleanup 调用后无条件解除 trap'
fi
START_LINE="$(grep -n 'bash "\$ROOT/scripts/preview/run-server-headless.sh" --debug &' "$SCRIPT" | cut -d: -f1)"
EXIT_TRAP_LINE="$(grep -n 'trap preview_exit_cleanup EXIT' "$SCRIPT" | cut -d: -f1)"
if [[ -n "$START_LINE" && -n "$EXIT_TRAP_LINE" && "$EXIT_TRAP_LINE" -lt "$START_LINE" ]]; then
    pass 'preview 在 server 启动前注册 cleanup trap，覆盖启动取消窗口'
else
    fail 'preview cleanup trap 必须先于 server 启动调用注册'
fi
if grep -Fq -- 'preview_start_owned' "$SCRIPT"; then
    fail 'preview 不得以启动前 PID 文件缺失冒充 server ownership'
else
    pass 'preview 启动阶段不以 PID pathname 冒充 server ownership'
fi

run_capture "$SANDBOX/list.out" "$SANDBOX/list.err" bash "$SCRIPT" --list
assert_rc 0 '--list 返回 0'
LIST_LINES="$(wc -l < "$SANDBOX/list.out" | tr -d '[:space:]')"
if [[ "$LIST_LINES" -eq 6 ]]; then pass '--list 输出 header 加五个 owner 行'; else fail "--list 行数应为 6，实际 $LIST_LINES"; fi
for suite in server client schema tiandao scripts; do
    assert_contains "$SANDBOX/list.out" "$suite" "--list 包含 $suite owner"
done
if awk -F '\t' 'NR == 1 || NF == 7 {next} {exit 1}' "$SANDBOX/list.out"; then
    pass '--list 每行包含可核验的七列矩阵'
else
    fail '--list 存在列数错误的行'
fi

run_capture "$SANDBOX/unknown.out" "$SANDBOX/unknown.err" bash "$SCRIPT" --unknown
assert_rc 2 '未知参数返回 usage/config 码 2'
run_capture "$SANDBOX/profile.out" "$SANDBOX/profile.err" bash "$SCRIPT" --profile nope
assert_rc 2 '未知 profile 返回 2'
run_capture "$SANDBOX/suite.out" "$SANDBOX/suite.err" bash "$SCRIPT" --profile unit --suite scripts
assert_rc 2 'unit 选择 scripts 返回 2，防止 contract 混入 unit'

MISSING_REPORT="$SANDBOX/missing-tools-report"
run_capture "$SANDBOX/missing.out" "$SANDBOX/missing.err" \
    env PATH=/usr/bin:/bin /bin/bash "$SCRIPT" --profile unit --suite server --report-dir "$MISSING_REPORT"
assert_rc 2 '缺 Rust 工具以明确的前置失败码返回'
assert_file "$MISSING_REPORT/summary.json" '缺工具仍生成 summary.json'
assert_file "$MISSING_REPORT/server/status" '缺工具仍生成 suite status'
assert_contains "$MISSING_REPORT/server/status" 'SKIP' '缺工具状态显式为 SKIP'
assert_contains "$MISSING_REPORT/server/stderr.log" '缺少 cargo/rustc' '缺工具原因写入 suite 日志'

FIXTURE="$SANDBOX/fixture"
mkdir -p "$FIXTURE/bin" "$FIXTURE/server/tests" "$FIXTURE/client" \
    "$FIXTURE/agent/packages/schema" "$FIXTURE/agent/packages/tiandao" \
    "$FIXTURE/scripts/tests" "$FIXTURE/client/src/gametest" \
    "$FIXTURE/agent/packages/schema/tests" "$FIXTURE/agent/packages/tiandao/tests"
cp "$SCRIPT" "$FIXTURE/scripts/test-all.sh"
cp "$ROOT/scripts/test-all-owners.tsv" "$FIXTURE/scripts/test-all-owners.tsv"
chmod +x "$FIXTURE/scripts/test-all.sh"

cat >"$FIXTURE/scripts/build-token.sh" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$FIXTURE_LOG"
if [[ "$FAKE_FAIL" == server && "$*" == *clippy* ]]; then
    printf 'intentional fake clippy failure\n' >&2
    exit 23
fi
exit 0
EOF
cat >"$FIXTURE/bin/cargo" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
cat >"$FIXTURE/bin/rustc" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
cat >"$FIXTURE/bin/java" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' 'openjdk version "17.0.10"' >&2
EOF
cat >"$FIXTURE/client/gradlew" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
chmod +x "$FIXTURE/scripts/build-token.sh" "$FIXTURE/bin/cargo" \
    "$FIXTURE/bin/rustc" "$FIXTURE/bin/java" "$FIXTURE/client/gradlew"

FIXTURE_LOG="$SANDBOX/fixture-commands.log"
CONTINUE_REPORT="$SANDBOX/continue-report"
run_capture "$SANDBOX/continue.out" "$SANDBOX/continue.err" \
    env PATH="$FIXTURE/bin:/usr/bin:/bin" FIXTURE_LOG="$FIXTURE_LOG" FAKE_FAIL=server \
    /bin/bash "$FIXTURE/scripts/test-all.sh" \
    --profile full --suite server --suite client --continue --report-dir "$CONTINUE_REPORT"
assert_rc 1 '--continue 最终保留失败退出码'
assert_file "$CONTINUE_REPORT/server/status" '失败 suite 写入 status'
assert_file "$CONTINUE_REPORT/client/status" '--continue 后续 suite 仍被执行'
assert_contains "$CONTINUE_REPORT/server/status" 'FAIL' '失败 suite 状态为 FAIL'
assert_contains "$CONTINUE_REPORT/client/status" 'PASS' '继续执行的 client suite 状态为 PASS'
assert_contains "$CONTINUE_REPORT/summary.json" '"exit_code":23' 'summary 保留 PIPESTATUS[0] 的真实 23'
assert_contains "$FIXTURE_LOG" 'gradle test build' 'continue 确实调用后续 client 命令'
assert_contains "$CONTINUE_REPORT/summary.json" '"owner":"Server owner"' 'summary 含 owner 映射'

UNIT_CLIENT_REPORT="$SANDBOX/unit-client-report"
UNIT_FIXTURE_LOG="$SANDBOX/unit-fixture-commands.log"
run_capture "$SANDBOX/unit-client.out" "$SANDBOX/unit-client.err" \
    env PATH="$FIXTURE/bin:/usr/bin:/bin" FIXTURE_LOG="$UNIT_FIXTURE_LOG" \
    /bin/bash "$FIXTURE/scripts/test-all.sh" \
    --profile unit --suite client --report-dir "$UNIT_CLIENT_REPORT"
assert_rc 0 'unit client suite 可独立通过'
assert_contains "$UNIT_FIXTURE_LOG" 'gradle test' 'unit client 只调用既有 gradle test'
if grep -Fxq 'gradle test build' "$UNIT_FIXTURE_LOG"; then
    fail 'unit client 不应偷偷追加 build'
else
    pass 'unit client 不偷偷追加 build'
fi

for e2e_script in smoke-test-e2e.sh bot-e2e.sh e2e-chat-signal-window.sh; do
    printf '#!/usr/bin/env bash\nprintf '\''e2e stub %s\\n'\'' >>"$FIXTURE_LOG"\n' "$e2e_script" \
        > "$FIXTURE/scripts/$e2e_script"
    chmod 0644 "$FIXTURE/scripts/$e2e_script"
done
E2E_REPORT="$SANDBOX/e2e-report"
run_capture "$SANDBOX/e2e.out" "$SANDBOX/e2e.err" \
    env PATH="$FIXTURE/bin:/usr/bin:/bin" FIXTURE_LOG="$FIXTURE_LOG" \
    /bin/bash "$FIXTURE/scripts/test-all.sh" \
    --profile e2e --suite scripts --report-dir "$E2E_REPORT"
assert_rc 0 'e2e 可调用既有但非 executable-bit 的 bash 脚本'
assert_contains "$E2E_REPORT/scripts/status" 'PASS' 'e2e suite 成功写入 status'
assert_contains "$FIXTURE_LOG" 'e2e stub bot-e2e.sh' 'e2e 按既有脚本顺序调用 bot 入口'

PREVIEW_REPORT="$SANDBOX/preview-report"
run_capture "$SANDBOX/preview.out" "$SANDBOX/preview.err" \
    env -u BONG_TERRAIN_RASTER_DIR -u BONG_TERRAIN_RASTER_PATH \
    -u BONG_CLIENT_PREVIEW_DIR PATH="$FIXTURE/bin:/usr/bin:/bin" \
    /bin/bash "$FIXTURE/scripts/test-all.sh" \
    --profile preview --suite scripts --report-dir "$PREVIEW_REPORT"
assert_rc 2 'preview 缺外部 raster handoff 以 BLOCKED 前置失败码返回'
assert_contains "$PREVIEW_REPORT/scripts/status" 'BLOCKED' 'preview 缺外部 raster 明确标记 BLOCKED'
assert_contains "$PREVIEW_REPORT/scripts/stderr.log" '缺少 BONG_TERRAIN_RASTER' 'preview BLOCKED 原因指向外部输入'

PREVIEW_TOOL_FIXTURE="$SANDBOX/preview-tool-fixture"
mkdir -p "$PREVIEW_TOOL_FIXTURE/scripts/preview" \
    "$PREVIEW_TOOL_FIXTURE/client" "$PREVIEW_TOOL_FIXTURE/raster" \
    "$PREVIEW_TOOL_FIXTURE/client/src/gametest" "$PREVIEW_TOOL_FIXTURE/agent/packages/schema/tests" \
    "$PREVIEW_TOOL_FIXTURE/agent/packages/tiandao/tests" "$PREVIEW_TOOL_FIXTURE/scripts/tests" \
    "$PREVIEW_TOOL_FIXTURE/server/tests" \
    "$PREVIEW_TOOL_FIXTURE/bin"
cp "$SCRIPT" "$PREVIEW_TOOL_FIXTURE/scripts/test-all.sh"
cp "$ROOT/scripts/test-all-owners.tsv" "$PREVIEW_TOOL_FIXTURE/scripts/test-all-owners.tsv"
chmod +x "$PREVIEW_TOOL_FIXTURE/scripts/test-all.sh"
touch "$PREVIEW_TOOL_FIXTURE/scripts/preview/run-server-headless.sh" \
    "$PREVIEW_TOOL_FIXTURE/scripts/preview/stop-server-headless.sh" \
    "$PREVIEW_TOOL_FIXTURE/client/preview-harness.json" \
    "$PREVIEW_TOOL_FIXTURE/raster/focus-layout-preview.png" \
    "$PREVIEW_TOOL_FIXTURE/raster/focus-surface-preview.png"
for tool in bash python3 cargo rustc git date mkdir basename dirname; do
    ln -s "$(command -v "$tool")" "$PREVIEW_TOOL_FIXTURE/bin/$tool"
done
cp "$FIXTURE/bin/java" "$PREVIEW_TOOL_FIXTURE/bin/java"
cp "$FIXTURE/client/gradlew" "$PREVIEW_TOOL_FIXTURE/client/gradlew"
cp "$FIXTURE/scripts/build-token.sh" "$PREVIEW_TOOL_FIXTURE/scripts/build-token.sh"
chmod +x "$PREVIEW_TOOL_FIXTURE/client/gradlew" "$PREVIEW_TOOL_FIXTURE/scripts/build-token.sh"
PREVIEW_TOOL_REPORT="$SANDBOX/preview-tool-report"
run_capture "$SANDBOX/preview-tool.out" "$SANDBOX/preview-tool.err" \
    env PATH="$PREVIEW_TOOL_FIXTURE/bin" \
    BONG_TERRAIN_RASTER_DIR="$PREVIEW_TOOL_FIXTURE/raster" \
    BONG_CLIENT_PREVIEW_DIR="$PREVIEW_TOOL_FIXTURE/client" \
    BONG_PREVIEW_CONFIG="$PREVIEW_TOOL_FIXTURE/client/preview-harness.json" \
    /bin/bash "$PREVIEW_TOOL_FIXTURE/scripts/test-all.sh" \
    --profile preview --suite scripts --report-dir "$PREVIEW_TOOL_REPORT"
assert_rc 2 'preview 缺 xvfb-run 以明确的前置失败码返回'
assert_contains "$PREVIEW_TOOL_REPORT/scripts/status" 'SKIP' 'preview 缺 xvfb-run 状态为 SKIP'
assert_contains "$PREVIEW_TOOL_REPORT/scripts/stderr.log" '需要 xvfb-run' 'preview 缺 xvfb-run 原因写入报告'

# wrapper 已发布 authority 后非零退出时，外层必须仍调用 stop；同时不能把
# 「需要清理」误当成「已 ready」而启动客户端。这个 fixture 只模拟 runner
# contract，不启动真实 server/Gradle/Redis/LLM。
PREVIEW_AUTHORITY_FIXTURE="$SANDBOX/preview-authority-fixture"
mkdir -p "$PREVIEW_AUTHORITY_FIXTURE/scripts/preview" \
    "$PREVIEW_AUTHORITY_FIXTURE/client/src/gametest" \
    "$PREVIEW_AUTHORITY_FIXTURE/raster" \
    "$PREVIEW_AUTHORITY_FIXTURE/agent/packages/schema/tests" \
    "$PREVIEW_AUTHORITY_FIXTURE/agent/packages/tiandao/tests" \
    "$PREVIEW_AUTHORITY_FIXTURE/scripts/tests" \
    "$PREVIEW_AUTHORITY_FIXTURE/server/tests" \
    "$PREVIEW_AUTHORITY_FIXTURE/bin"
cp "$SCRIPT" "$PREVIEW_AUTHORITY_FIXTURE/scripts/test-all.sh"
cp "$ROOT/scripts/test-all-owners.tsv" "$PREVIEW_AUTHORITY_FIXTURE/scripts/test-all-owners.tsv"
cp "$FIXTURE/scripts/build-token.sh" "$PREVIEW_AUTHORITY_FIXTURE/scripts/build-token.sh"
chmod +x "$PREVIEW_AUTHORITY_FIXTURE/scripts/test-all.sh"
touch "$PREVIEW_AUTHORITY_FIXTURE/client/preview-harness.json" \
    "$PREVIEW_AUTHORITY_FIXTURE/raster/focus-layout-preview.png" \
    "$PREVIEW_AUTHORITY_FIXTURE/raster/focus-surface-preview.png" \
    "$PREVIEW_AUTHORITY_FIXTURE/client/gradlew"
cat > "$PREVIEW_AUTHORITY_FIXTURE/scripts/preview/run-server-headless.sh" <<'EOF'
#!/usr/bin/env bash
set -u
if [[ "${PREVIEW_WRAPPER_MODE:-failure}" == refuse_existing ]]; then
    exit 1
fi
printf 'pid=4242\nstarttime=1\nexecutable=fixture-server\nexecutable_identity=1:1\n' > "$BONG_PREVIEW_PID_FILE"
if [[ "${PREVIEW_WRAPPER_MODE:-failure}" != failure_no_marker \
    && "${PREVIEW_WRAPPER_MODE:-failure}" != refuse_existing ]]; then
    printf 'state=authority_published\n' > "$BONG_PREVIEW_LAUNCH_STATE_FILE"
fi
if [[ "${PREVIEW_WRAPPER_MODE:-failure}" == success ]]; then
    exit 0
fi
if [[ "${PREVIEW_WRAPPER_MODE:-failure}" == failure_no_marker ]]; then
    exit 75
fi
if [[ "${PREVIEW_WRAPPER_MODE:-failure}" == refuse_existing ]]; then
    exit 1
fi
exit 23
EOF
cat > "$PREVIEW_AUTHORITY_FIXTURE/scripts/preview/stop-server-headless.sh" <<'EOF'
#!/usr/bin/env bash
set -u
printf 'stop\n' >> "$PREVIEW_STOP_LOG"
if ! awk '
    NR == 1 && $0 == "pid=4242" { pid=1 }
    NR == 2 && $0 == "starttime=1" { starttime=1 }
    NR == 3 && $0 == "executable=fixture-server" { executable=1 }
    NR == 4 && $0 == "executable_identity=1:1" { identity=1 }
    END { exit !(pid && starttime && executable && identity && NR == 4) }
' "$BONG_PREVIEW_PID_FILE"; then
    printf 'identity-safe fixture stop rejected malformed authority\n' >&2
    exit 19
fi
if [[ -n "${PREVIEW_STOP_COUNT_FILE:-}" ]]; then
    count=0
    [[ ! -f "$PREVIEW_STOP_COUNT_FILE" ]] || read -r count < "$PREVIEW_STOP_COUNT_FILE"
    count=$((count + 1))
    printf '%s\n' "$count" > "$PREVIEW_STOP_COUNT_FILE"
    if [[ "${PREVIEW_STOP_FAIL_ONCE:-0}" == 1 && "$count" -eq 1 ]]; then
        exit 17
    fi
fi
rm -f -- "$BONG_PREVIEW_PID_FILE" "$BONG_PREVIEW_LAUNCH_STATE_FILE"
exit 0
EOF
cat > "$PREVIEW_AUTHORITY_FIXTURE/scripts/preview/validate_snapshots.py" <<'EOF'
#!/usr/bin/env python3
raise SystemExit(0)
EOF
cat > "$PREVIEW_AUTHORITY_FIXTURE/scripts/preview/compose_grid.py" <<'EOF'
#!/usr/bin/env python3
raise SystemExit(0)
EOF
cat > "$PREVIEW_AUTHORITY_FIXTURE/bin/java" <<'EOF'
#!/usr/bin/env bash
printf 'openjdk version "17.0.10"\n' >&2
EOF
cat > "$PREVIEW_AUTHORITY_FIXTURE/bin/xvfb-run" <<'EOF'
#!/usr/bin/env bash
printf 'client-preview-called\n' >> "$PREVIEW_STOP_LOG"
exit 0
EOF
for tool in cargo rustc; do
    ln -s "$(command -v "$tool")" "$PREVIEW_AUTHORITY_FIXTURE/bin/$tool"
done
chmod +x "$PREVIEW_AUTHORITY_FIXTURE/scripts/preview/run-server-headless.sh" \
    "$PREVIEW_AUTHORITY_FIXTURE/scripts/preview/stop-server-headless.sh" \
    "$PREVIEW_AUTHORITY_FIXTURE/scripts/preview/validate_snapshots.py" \
    "$PREVIEW_AUTHORITY_FIXTURE/scripts/preview/compose_grid.py" \
    "$PREVIEW_AUTHORITY_FIXTURE/scripts/build-token.sh" \
    "$PREVIEW_AUTHORITY_FIXTURE/bin/java" "$PREVIEW_AUTHORITY_FIXTURE/bin/xvfb-run" \
    "$PREVIEW_AUTHORITY_FIXTURE/client/gradlew"
PREVIEW_AUTHORITY_REPORT="$SANDBOX/preview-authority-report"
PREVIEW_STOP_LOG="$SANDBOX/preview-stop.log"
run_capture "$SANDBOX/preview-authority.out" "$SANDBOX/preview-authority.err" \
    env PATH="$PREVIEW_AUTHORITY_FIXTURE/bin:/usr/bin:/bin" \
    BONG_TERRAIN_RASTER_DIR="$PREVIEW_AUTHORITY_FIXTURE/raster" \
    BONG_CLIENT_PREVIEW_DIR="$PREVIEW_AUTHORITY_FIXTURE/client" \
    BONG_PREVIEW_CONFIG="$PREVIEW_AUTHORITY_FIXTURE/client/preview-harness.json" \
    PREVIEW_STOP_LOG="$PREVIEW_STOP_LOG" \
    /bin/bash "$PREVIEW_AUTHORITY_FIXTURE/scripts/test-all.sh" \
    --profile preview --suite scripts --report-dir "$PREVIEW_AUTHORITY_REPORT"
assert_rc 1 'wrapper 非零但已发布 authority 时 preview 仍诚实失败'
assert_contains "$PREVIEW_STOP_LOG" 'stop' 'wrapper 非零且有 authority 时外层重试 stop cleanup'
if [[ ! -e "$PREVIEW_AUTHORITY_REPORT/preview-server.pid" ]] \
    && [[ -z "$(find "$PREVIEW_AUTHORITY_REPORT" -maxdepth 1 -name '.preview-launch-*.state' -print -quit)" ]]; then
    pass 'wrapper 非零 authority cleanup 后 PID/state 均清理'
else
    fail 'wrapper 非零 authority cleanup 遗留 PID/state'
fi
if grep -Fq 'client-preview-called' "$PREVIEW_STOP_LOG"; then
    fail 'wrapper 非零 authority 不应被当成 ready 启动客户端'
else
    pass 'wrapper 非零 authority 不冒充 ready'
fi

# handoff marker 发布自身失败时，父 runner 仍必须把普通 authority record
# 交给 identity-safe stop；不得因 marker 缺失而把 server 留成孤儿。
PREVIEW_NO_MARKER_REPORT="$SANDBOX/preview-no-marker-report"
PREVIEW_NO_MARKER_STOP_LOG="$SANDBOX/preview-no-marker-stop.log"
run_capture "$SANDBOX/preview-no-marker.out" "$SANDBOX/preview-no-marker.err" \
    env PATH="$PREVIEW_AUTHORITY_FIXTURE/bin:/usr/bin:/bin" \
    BONG_TERRAIN_RASTER_DIR="$PREVIEW_AUTHORITY_FIXTURE/raster" \
    BONG_CLIENT_PREVIEW_DIR="$PREVIEW_AUTHORITY_FIXTURE/client" \
    BONG_PREVIEW_CONFIG="$PREVIEW_AUTHORITY_FIXTURE/client/preview-harness.json" \
    PREVIEW_STOP_LOG="$PREVIEW_NO_MARKER_STOP_LOG" \
    PREVIEW_WRAPPER_MODE=failure_no_marker \
    /bin/bash "$PREVIEW_AUTHORITY_FIXTURE/scripts/test-all.sh" \
    --profile preview --suite scripts --report-dir "$PREVIEW_NO_MARKER_REPORT"
assert_rc 1 'handoff marker 缺失时 preview 仍诚实失败'
assert_contains "$PREVIEW_NO_MARKER_STOP_LOG" 'stop' 'handoff marker 缺失时仍调用 identity-safe stop cleanup'
if [[ -e "$PREVIEW_NO_MARKER_REPORT/preview-server.pid" ]]; then
    fail 'handoff marker 缺失时 cleanup 遗留 authority record'
else
    pass 'handoff marker 缺失时 cleanup 不遗留 authority record'
fi
if grep -Fq 'client-preview-called' "$PREVIEW_NO_MARKER_STOP_LOG"; then
    fail 'handoff marker 缺失时不应把 wrapper 失败当成 ready 启动客户端'
else
    pass 'handoff marker 缺失时不冒充 ready'
fi

# A normal preexisting-record refusal is not the handoff-publication failure
# code; reusing a report directory must therefore leave its authority alone.
PREVIEW_REFUSAL_REPORT="$SANDBOX/preview-refusal-report"
PREVIEW_REFUSAL_STOP_LOG="$SANDBOX/preview-refusal-stop.log"
mkdir -p "$PREVIEW_REFUSAL_REPORT"
printf 'pid=4242\nstarttime=1\nexecutable=fixture-server\nexecutable_identity=1:1\n' \
    > "$PREVIEW_REFUSAL_REPORT/preview-server.pid"
run_capture "$SANDBOX/preview-refusal.out" "$SANDBOX/preview-refusal.err" \
    env PATH="$PREVIEW_AUTHORITY_FIXTURE/bin:/usr/bin:/bin" \
    BONG_TERRAIN_RASTER_DIR="$PREVIEW_AUTHORITY_FIXTURE/raster" \
    BONG_CLIENT_PREVIEW_DIR="$PREVIEW_AUTHORITY_FIXTURE/client" \
    BONG_PREVIEW_CONFIG="$PREVIEW_AUTHORITY_FIXTURE/client/preview-harness.json" \
    PREVIEW_STOP_LOG="$PREVIEW_REFUSAL_STOP_LOG" \
    PREVIEW_WRAPPER_MODE=refuse_existing \
    /bin/bash "$PREVIEW_AUTHORITY_FIXTURE/scripts/test-all.sh" \
    --profile preview --suite scripts --report-dir "$PREVIEW_REFUSAL_REPORT"
assert_rc 1 'preexisting authority refusal 仍诚实失败'
if [[ -s "$PREVIEW_REFUSAL_STOP_LOG" ]]; then
    fail 'preexisting authority refusal 不得因共享 PID path 触发 stop'
else
    pass 'preexisting authority refusal 不因共享 PID path 触发 stop'
fi
if [[ -e "$PREVIEW_REFUSAL_REPORT/preview-server.pid" ]]; then
    pass 'preexisting authority refusal 保留原 authority record'
else
    fail 'preexisting authority refusal 意外清除了原 authority record'
fi

# stop 首次失败后由 EXIT trap 重试时，cleanup 状态必须仍存在于管道子 shell；
# 这是 Kody finding 的真实回归 pin，而不是只检查静态变量声明。
PREVIEW_RETRY_REPORT="$SANDBOX/preview-retry-report"
PREVIEW_RETRY_STOP_COUNT="$SANDBOX/preview-stop-count"
run_capture "$SANDBOX/preview-retry.out" "$SANDBOX/preview-retry.err" \
    env PATH="$PREVIEW_AUTHORITY_FIXTURE/bin:/usr/bin:/bin" \
    BONG_TERRAIN_RASTER_DIR="$PREVIEW_AUTHORITY_FIXTURE/raster" \
    BONG_CLIENT_PREVIEW_DIR="$PREVIEW_AUTHORITY_FIXTURE/client" \
    BONG_PREVIEW_CONFIG="$PREVIEW_AUTHORITY_FIXTURE/client/preview-harness.json" \
    PREVIEW_STOP_LOG="$PREVIEW_STOP_LOG" \
    PREVIEW_STOP_COUNT_FILE="$PREVIEW_RETRY_STOP_COUNT" \
    PREVIEW_STOP_FAIL_ONCE=1 PREVIEW_WRAPPER_MODE=success \
    /bin/bash "$PREVIEW_AUTHORITY_FIXTURE/scripts/test-all.sh" \
    --profile preview --suite scripts --report-dir "$PREVIEW_RETRY_REPORT"
assert_rc 1 'stop 首次失败仍保留 preview 的诚实非零结果'
if [[ "$(cat "$PREVIEW_RETRY_STOP_COUNT")" -eq 2 ]]; then
    pass 'stop 首次失败后 EXIT cleanup 使用持久状态完成第二次清理'
else
    fail "stop 首次失败后应调用两次 cleanup，实际 $(cat "$PREVIEW_RETRY_STOP_COUNT" 2>/dev/null || printf 0) 次"
fi
if [[ ! -e "$PREVIEW_RETRY_REPORT/preview-server.pid" ]] \
    && [[ -z "$(find "$PREVIEW_RETRY_REPORT" -maxdepth 1 -name '.preview-launch-*.state' -print -quit)" ]]; then
    pass 'EXIT cleanup retry 后不遗留 PID/state'
else
    fail 'EXIT cleanup retry 后遗留 PID/state'
fi
if grep -Fq 'unbound variable' "$SANDBOX/preview-retry.err"; then
    fail 'EXIT cleanup retry 不得因管道函数局部变量失效而触发 unbound variable'
else
    pass 'EXIT cleanup retry 未触发局部变量失效异常'
fi

BAD_FIXTURE="$SANDBOX/bad-fixture"
mkdir -p "$BAD_FIXTURE/scripts" "$BAD_FIXTURE/server/tests" "$BAD_FIXTURE/client" \
    "$BAD_FIXTURE/agent/packages/schema" "$BAD_FIXTURE/agent/packages/tiandao" \
    "$BAD_FIXTURE/scripts/tests" "$BAD_FIXTURE/client/src/gametest" \
    "$BAD_FIXTURE/agent/packages/schema/tests" "$BAD_FIXTURE/agent/packages/tiandao/tests"
cp "$SCRIPT" "$BAD_FIXTURE/scripts/test-all.sh"
sed 's#scripts/tests#scripts/does-not-exist#' \
    "$ROOT/scripts/test-all-owners.tsv" > "$BAD_FIXTURE/scripts/test-all-owners.tsv"
run_capture "$SANDBOX/bad-list.out" "$SANDBOX/bad-list.err" \
    /bin/bash "$BAD_FIXTURE/scripts/test-all.sh" --list
assert_rc 2 'owner evidence path 缺失时 --list 返回 2'
assert_contains "$SANDBOX/bad-list.err" 'evidence path' 'owner path 校验给出修复线索'

printf 'Result: %d passed, %d failed\n' "$PASS" "$FAIL"
[[ "$FAIL" -eq 0 ]]
