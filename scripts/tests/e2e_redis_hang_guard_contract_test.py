#!/usr/bin/env python3
"""Pin the bounded, workspace-local task-13 Tiandao execution contract."""

from __future__ import annotations

import pathlib
import re
import shlex
import subprocess
import tempfile
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]
E2E_REDIS = ROOT / "scripts/e2e-redis.sh"
SMOKE_E2E = ROOT / "scripts/smoke-test-e2e.sh"


class E2ERedisHangGuardContractTest(unittest.TestCase):
    def test_tiandao_uses_workspace_tsx_and_is_bounded(self) -> None:
        source = E2E_REDIS.read_text(encoding="utf-8")
        self.assertNotRegex(
            source,
            r"\bnpx\s+tsx\b",
            "task-13 入口不得通过 npx 在运行时访问 registry",
        )
        self.assertIn('TIANDAO_TSX="$NODE_BIN/tsx"', source)
        self.assertIn('if [ ! -x "$TIANDAO_TSX" ]; then', source)
        self.assertIn("workspace tsx executable is missing", source)
        self.assertIn('TIANDAO_TIMEOUT_SECONDS="${BONG_E2E_TIANDAO_TIMEOUT_SECONDS:-120}"', source)
        self.assertIn('TIANDAO_KILL_GRACE_SECONDS="${BONG_E2E_TIANDAO_KILL_GRACE_SECONDS:-5}"', source)

        stage = source.index('CURRENT_STAGE="tiandao"')
        launch = source.index(
            '"$TIANDAO_TSX" "$ROOT/agent/packages/tiandao/src/task-13-one-tick.ts"',
            stage,
        )
        timeout = source.index("timeout \\", stage)
        capture = source.index("TIANDAO_EXIT=$?", launch)
        elapsed = source.index("TIANDAO_ELAPSED_SECONDS=", capture)
        first_anchor = source.index(
            'if wait_for_pattern "$TIANDAO_LOG" "\\\\[tiandao\\\\] connected',
            elapsed,
        )
        self.assertLess(timeout, launch)
        self.assertLess(launch, capture)
        self.assertLess(capture, elapsed)
        self.assertLess(elapsed, first_anchor)
        self.assertIn('--signal=TERM', source[timeout:launch])
        self.assertIn('--kill-after="${TIANDAO_KILL_GRACE_SECONDS}s"', source[timeout:launch])
        self.assertIn('"${TIANDAO_TIMEOUT_SECONDS}s"', source[timeout:launch])
        self.assertIn("Non-mock Tiandao timed out", source)
        self.assertIn("elapsed=${TIANDAO_ELAPSED_SECONDS}s", source)
        self.assertIn("log=$TIANDAO_LOG; run_dir=$RUN_DIR", source)

    def test_other_e2e_redis_foreground_stages_are_bounded(self) -> None:
        source = E2E_REDIS.read_text(encoding="utf-8")
        stage = source.index('CURRENT_STAGE="schema"')
        schema = source.index("npm run build", stage)
        self.assertIn(
            'cd "$ROOT/agent/packages/schema" &&',
            source[stage:schema],
            "schema 构建必须在目录切换失败时停止，不能在错误目录继续执行",
        )
        self.assertIn("timeout --signal=TERM", source[max(stage, schema - 180) : schema])

        preview = source.index('python3 "$ROOT/scripts/bot/run_scenarios.py"')
        self.assertIn(
            "timeout --signal=TERM --kill-after=5s 300s",
            source[preview - 140 : preview],
        )

        probe = source.index("node --input-type=module")
        self.assertIn(
            "timeout --signal=TERM --kill-after=2s 10s",
            source[probe - 140 : probe],
        )

    def test_smoke_nested_commands_use_frontend_timeout_and_keep_exit_codes(self) -> None:
        source = SMOKE_E2E.read_text(encoding="utf-8")
        self.assertIn("run_bounded() {", source)
        self.assertIn("timeout \\\n    --signal=TERM", source)
        self.assertIn(
            'SMOKE_E2E_TIMEOUT_SECONDS="${BONG_SMOKE_E2E_TIMEOUT_SECONDS:-1500}"',
            source,
        )
        self.assertIn(
            'SMOKE_E2E_KILL_GRACE_SECONDS="${BONG_SMOKE_E2E_KILL_GRACE_SECONDS:-40}"',
            source,
            "e2e 外层 KILL 宽限必须覆盖 e2e-redis.sh 的 cleanup 窗口",
        )
        self.assertIn(
            'require_positive_timeout "BONG_SMOKE_E2E_KILL_GRACE_SECONDS"',
            source,
        )
        self.assertIn("exit=$stage_exit", source)

        commands = (
            'bash "$ROOT/scripts/test-server-lifecycle.sh"',
            'bash "$ROOT/scripts/test-dev-reload-disown.sh"',
            'npm run check',
            'npm test',
            'npm run generate',
            '"$ROOT/scripts/build-token.sh" cargo test',
            'bash "$ROOT/scripts/e2e-redis.sh"',
        )
        for command in commands:
            with self.subTest(command=command):
                position = source.index(command)
                prefix = source[max(0, position - 260) : position]
                self.assertIn(
                    "run_bounded",
                    prefix,
                    f"{command} 必须通过 run_bounded 前台执行",
                )

        self.assertNotIn(
            'if bash "$ROOT/scripts/e2e-redis.sh"',
            source,
            "smoke wrapper 不得留下无界 e2e 子进程",
        )
        e2e_stage = source[source.index('CURRENT_STAGE="e2e"') :]
        self.assertIn(
            'run_bounded "$SMOKE_E2E_TIMEOUT_SECONDS" "$SMOKE_E2E_KILL_GRACE_SECONDS"',
            e2e_stage,
        )

    def test_bounded_runner_preserves_native_failure_and_timeout(self) -> None:
        source = SMOKE_E2E.read_text(encoding="utf-8")
        function_start = source.index("run_bounded() {")
        function_end = source.index("\n}\n\necho", function_start) + 2
        function = source[function_start:function_end]
        probe = f"""
{function}
set +e
run_bounded 2 1 bash -c 'exit 23'
normal_exit=$?
run_bounded 1 1 sleep 5
timeout_exit=$?
printf 'normal=%s timeout=%s\\n' "$normal_exit" "$timeout_exit"
"""
        completed = subprocess.run(
            ["bash", "-c", probe],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertIn("normal=23 timeout=124", completed.stdout)

    def test_tiandao_stage_reports_timeout_and_native_exit(self) -> None:
        source = E2E_REDIS.read_text(encoding="utf-8")
        stage_start = source.index(
            'if ! [[ "$TIANDAO_TIMEOUT_SECONDS" =~ ^[1-9][0-9]*$ ]]; then'
        )
        anchor_start = source.index(
            '\nif wait_for_pattern "$TIANDAO_LOG"', stage_start
        )
        stage = source[stage_start:anchor_start]

        def run_stage(body: str, timeout_seconds: int) -> subprocess.CompletedProcess[str]:
            with tempfile.TemporaryDirectory() as temporary:
                root = pathlib.Path(temporary)
                bin_dir = root / "bin"
                run_dir = root / "run"
                bin_dir.mkdir()
                run_dir.mkdir()
                tsx = bin_dir / "tsx"
                tsx.write_text(
                    "#!/usr/bin/env bash\nset -euo pipefail\n" + body,
                    encoding="utf-8",
                )
                tsx.chmod(0o755)
                prelude = "\n".join(
                    (
                        "set -euo pipefail",
                        f"ROOT={shlex.quote(str(ROOT))}",
                        f"NODE_BIN={shlex.quote(str(bin_dir))}",
                        f"RUN_DIR={shlex.quote(str(run_dir))}",
                        f"TIANDAO_LOG={shlex.quote(str(run_dir / 'tiandao.log'))}",
                        "REDIS_URL=redis://127.0.0.1:6379",
                        f"TIANDAO_TIMEOUT_SECONDS={timeout_seconds}",
                        "TIANDAO_KILL_GRACE_SECONDS=1",
                        "finalize_failure() { printf 'FAIL stage=%s message=%s\\n' \"$1\" \"$2\"; exit 1; }",
                    )
                )
                return subprocess.run(
                    ["bash", "-c", prelude + "\n" + stage],
                    check=False,
                    capture_output=True,
                    text=True,
                )

        timed_out = run_stage("sleep 30\n", timeout_seconds=1)
        self.assertEqual(timed_out.returncode, 1, timed_out.stderr)
        self.assertIn("Non-mock Tiandao timed out", timed_out.stdout)
        self.assertIn("exit=124", timed_out.stdout)
        self.assertRegex(timed_out.stdout, r"elapsed=[0-9]+s")
        self.assertIn("log=", timed_out.stdout)
        self.assertIn("run_dir=", timed_out.stdout)

        native_failure = run_stage("exit 23\n", timeout_seconds=2)
        self.assertEqual(native_failure.returncode, 1, native_failure.stderr)
        self.assertIn("Non-mock Tiandao exited with code 23", native_failure.stdout)
        self.assertIn("exit=23", native_failure.stdout)


if __name__ == "__main__":
    result = unittest.main(exit=False)
    if result.result.wasSuccessful():
        print(
            "e2e Redis hang guard contract PASS "
            f"({result.result.testsRun} tests)"
        )
    raise SystemExit(not result.result.wasSuccessful())
