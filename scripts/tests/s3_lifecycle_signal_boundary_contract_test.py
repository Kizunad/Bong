#!/usr/bin/env python3
"""No-signal regression tests for process authority boundaries."""

from __future__ import annotations

import importlib.util
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


ROOT = pathlib.Path(__file__).resolve().parents[2]
LIFECYCLE = ROOT / "scripts/lib/bong-server-lifecycle.sh"
PIDFD_SIGNAL = ROOT / "scripts/lib/bong-pidfd-signal.py"
SUPERVISOR = ROOT / "scripts/lib/bong-process-group-supervisor.py"
SHUTDOWN_ORDER = ROOT / "scripts/test-tmux-shutdown-order.sh"
LIFECYCLE_TEST = ROOT / "scripts/test-server-lifecycle.sh"
E2E_WORKFLOW = ROOT / ".github/workflows/e2e.yml"


def load_supervisor_module():
    spec = importlib.util.spec_from_file_location("bong_process_group_supervisor", SUPERVISOR)
    if spec is None or spec.loader is None:
        raise RuntimeError("could not load process-group supervisor module")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class SignalBoundaryContractTest(unittest.TestCase):
    def test_shell_signal_ids_reject_reserved_and_malformed_values(self):
        script = f"""
set -euo pipefail
source {LIFECYCLE!s}
for value in '' -1 0 1 abc 1.0; do
    if bong_server_validate_signal_id "$value"; then
        printf 'accepted reserved signal id: %s\\n' "$value" >&2
        exit 1
    fi
done
for value in 2 4242; do
    bong_server_validate_signal_id "$value" || {{
        printf 'rejected ordinary process id: %s\\n' "$value" >&2
        exit 1
    }}
done
"""
        subprocess.run(["bash", "-c", script], check=True)

    def test_pidfd_entrypoint_rejects_reserved_ids_before_open(self):
        for pid in ("-1", "0", "1"):
            with self.subTest(pid=pid):
                result = subprocess.run(
                    [sys.executable, str(PIDFD_SIGNAL), pid, "1", "1:1", "TERM"],
                    check=False,
                    capture_output=True,
                    text=True,
                )
                self.assertEqual(
                    result.returncode,
                    2,
                    f"reserved pid {pid} must fail before pidfd_open/signaling",
                )
                self.assertEqual(result.stdout, "")
                self.assertEqual(result.stderr, "")

        for pgrp in ("0", "1"):
            with self.subTest(pgrp=pgrp):
                result = subprocess.run(
                    [
                        sys.executable,
                        str(PIDFD_SIGNAL),
                        "4242",
                        "1",
                        "1:1",
                        "TERM",
                        pgrp,
                    ],
                    check=False,
                    capture_output=True,
                    text=True,
                )
                self.assertEqual(
                    result.returncode,
                    2,
                    f"reserved process group {pgrp} must fail before pidfd_open/signaling",
                )
                self.assertEqual(result.stdout, "")
                self.assertEqual(result.stderr, "")

    def test_supervisor_requires_its_own_private_session_and_group(self):
        supervisor = load_supervisor_module()
        with (
            mock.patch.object(supervisor.os, "getpid", return_value=4242),
            mock.patch.object(supervisor.os, "getpgrp", return_value=4242),
            mock.patch.object(supervisor.os, "getsid", return_value=4242),
        ):
            self.assertEqual(supervisor.private_process_group(), 4242)

        unsafe_shapes = (
            (1, 1, 1),
            (4242, 0, 4242),
            (4242, 4241, 4242),
            (4242, 4242, 4241),
        )
        for pid, pgid, sid in unsafe_shapes:
            with (
                self.subTest(pid=pid, pgid=pgid, sid=sid),
                mock.patch.object(supervisor.os, "getpid", return_value=pid),
                mock.patch.object(supervisor.os, "getpgrp", return_value=pgid),
                mock.patch.object(supervisor.os, "getsid", return_value=sid),
            ):
                with self.assertRaisesRegex(RuntimeError, "private session"):
                    supervisor.private_process_group()

    def test_ready_pid_capture_survives_nested_regex_validation(self):
        script = f"""
set -euo pipefail
source {LIFECYCLE!s}
runtime="$(mktemp -d)"
trap 'rm -rf -- "$runtime"' EXIT
chmod 700 "$runtime"
ready="$runtime/ready"
printf 'pid=4242\\n' > "$ready"
chmod 600 "$ready"
[ "$(bong_server_read_ready_pid "$ready")" = 4242 ]
for value in 0 1 -1 abc; do
    printf 'pid=%s\\n' "$value" > "$ready"
    chmod 600 "$ready"
    if bong_server_read_ready_pid "$ready" >/dev/null; then
        printf 'accepted reserved or malformed ready pid: %s\\n' "$value" >&2
        exit 1
    fi
done
"""
        subprocess.run(["bash", "-c", script], check=True)

    def test_shutdown_order_is_ci_opt_in_and_uses_absolute_socket(self):
        shutdown_text = SHUTDOWN_ORDER.read_text(encoding="utf-8")
        lifecycle_text = LIFECYCLE_TEST.read_text(encoding="utf-8")
        workflow_text = E2E_WORKFLOW.read_text(encoding="utf-8")

        self.assertIn('${GITHUB_ACTIONS:-}', shutdown_text)
        self.assertIn('${BONG_RUN_TMUX_SHUTDOWN_ORDER_TEST:-0}', shutdown_text)
        self.assertIn('TMUX_SOCKET="$TEST_ROOT/tmux.sock"', shutdown_text)
        self.assertIn('tmux -S "$TMUX_SOCKET"', shutdown_text)
        self.assertNotIn("tmux -L", shutdown_text)
        self.assertNotIn("kill-server", shutdown_text)
        self.assertNotRegex(shutdown_text, r"(?m)^\s*kill\s")

        self.assertIn('${GITHUB_ACTIONS:-}', lifecycle_text)
        self.assertIn('${BONG_RUN_TMUX_SHUTDOWN_ORDER_TEST:-0}', lifecycle_text)
        self.assertIn('BONG_RUN_TMUX_SHUTDOWN_ORDER_TEST: "1"', workflow_text)

    def test_pinned_rollback_never_signals_replacement_record(self):
        script = f"""
set -euo pipefail
source {LIFECYCLE!s}
runtime="$(mktemp -d)"
trap 'rm -rf -- "$runtime"' EXIT
chmod 700 "$runtime"
export BONG_SERVER_PID_FILE="$runtime/server.pid"

# 落盘一条与旧启动身份不同的 replacement 权威记录（#846 原触发面）。
# rollback 只能信号旧身份；记录必须原样保留，绝不能被直接删除或覆盖
# （finding 6：此前空 PID 文件路径无法保护这条持久状态契约）。
replacement='pid=401\\nstarttime=40\\nexecutable=/new/server\\nexecutable_identity=401:40\\n'
printf '%b' "$replacement" > "$BONG_SERVER_PID_FILE"
chmod 600 "$BONG_SERVER_PID_FILE"
before="$(cat "$BONG_SERVER_PID_FILE")"

signaled=""
bong_server_stop_pinned_process() {{
    signaled="$1:$2:$3"
    return 1
}}
bong_server_pinned_process_status() {{
    return 1
}}
bong_server_clear_record_if_matches() {{
    [ "$1:$2:$3:$4" = "200:20:/old/server:2:20" ] || return 2
    return 1
}}

bong_server_rollback_pinned_managed_process \
    200 20 /old/server 2:20 "old launch rollback"
[ "$signaled" = "200:20:2:20" ] || {{
    printf 'rollback signaled unexpected identity: %s\\n' "$signaled" >&2
    exit 1
}}
[ -f "$BONG_SERVER_PID_FILE" ] || {{
    printf 'rollback removed the replacement authority record\\n' >&2
    exit 1
}}
[ "$(cat "$BONG_SERVER_PID_FILE")" = "$before" ] || {{
    printf 'rollback overwrote the replacement authority record\\n' >&2
    exit 1
}}
"""
        subprocess.run(["bash", "-c", script], check=True)

    def test_pinned_rollback_preserves_matching_record_when_process_survives(self):
        script = f"""
set -euo pipefail
source {LIFECYCLE!s}
runtime="$(mktemp -d)"
trap 'rm -rf -- "$runtime"' EXIT
chmod 700 "$runtime"
export BONG_SERVER_PID_FILE="$runtime/server.pid"

# 落盘与本次回滚身份一致的权威记录；进程存活时 rollback 必须原样保留它
# （finding 6：不再用空 PID 文件路径，直接验证持久记录未被删除或覆盖）。
matching='pid=200\\nstarttime=20\\nexecutable=/old/server\\nexecutable_identity=2:20\\n'
printf '%b' "$matching" > "$BONG_SERVER_PID_FILE"
chmod 600 "$BONG_SERVER_PID_FILE"
before="$(cat "$BONG_SERVER_PID_FILE")"

cleared=0
bong_server_stop_pinned_process() {{ return 1; }}
bong_server_pinned_process_status() {{ return 0; }}
bong_server_clear_record_if_matches() {{ cleared=1; return 0; }}

if bong_server_rollback_pinned_managed_process \
    200 20 /old/server 2:20 "surviving old launch rollback"; then
    printf 'rollback accepted a pinned process that survived cleanup\n' >&2
    exit 1
fi
[ "$cleared" -eq 0 ] || {{
    printf 'rollback cleared authority for a process that remained alive\n' >&2
    exit 1
}}
[ -f "$BONG_SERVER_PID_FILE" ] || {{
    printf 'rollback removed the surviving process authority record\\n' >&2
    exit 1
}}
[ "$(cat "$BONG_SERVER_PID_FILE")" = "$before" ] || {{
    printf 'rollback overwrote the surviving process authority record\\n' >&2
    exit 1
}}
"""
        subprocess.run(["bash", "-c", script], check=True)


class SupervisorBuildArtifactContractTest(unittest.TestCase):
    """build_server_binary 路径解析与失败清理的聚焦契约（finding 4/7）。"""

    def _fake_built_binary(self, server_directory, relative_target):
        release = server_directory / relative_target / "release"
        release.mkdir(parents=True, exist_ok=True)
        built = release / "bong-server"
        built.write_text("#!/bin/sh\nexit 0\n")
        return built

    def test_build_server_binary_resolves_relative_cargo_target_dir(self):
        supervisor = load_supervisor_module()
        server_directory = pathlib.Path(
            tempfile.mkdtemp(prefix="bong-supervisor-unit-")
        )
        self.addCleanup(shutil.rmtree, server_directory, ignore_errors=True)
        artifact_parent = pathlib.Path(
            tempfile.mkdtemp(prefix="bong-e2e-server-unit-")
        )
        self.addCleanup(shutil.rmtree, artifact_parent, ignore_errors=True)
        self._fake_built_binary(server_directory, "custom-target")

        environment = os.environ.copy()
        environment["CARGO_TARGET_DIR"] = "custom-target"
        with (
            mock.patch.object(supervisor.os, "environ", environment),
            mock.patch.object(supervisor.subprocess, "run"),
            mock.patch.object(
                supervisor.tempfile, "mkdtemp", return_value=str(artifact_parent)
            ),
        ):
            artifact = supervisor.build_server_binary(
                server_directory, pathlib.Path("/unused/build-token")
            )

        # 相对 CARGO_TARGET_DIR 必须按 server 目录解析；若按 supervisor 调用目录
        # 解析，built_binary 找不到会先抛 RuntimeError，此断言根本走不到。
        self.assertTrue(artifact.is_file(), "相对 CARGO_TARGET_DIR 必须解析到 server 目录")

    def test_failed_artifact_copy_removes_artifact_directory(self):
        supervisor = load_supervisor_module()
        server_directory = pathlib.Path(
            tempfile.mkdtemp(prefix="bong-supervisor-unit-")
        )
        self.addCleanup(shutil.rmtree, server_directory, ignore_errors=True)
        self._fake_built_binary(server_directory, "target")
        artifact_parent = pathlib.Path(
            tempfile.mkdtemp(prefix="bong-e2e-server-unit-")
        )

        with (
            mock.patch.object(supervisor.subprocess, "run"),
            mock.patch.object(
                supervisor.tempfile, "mkdtemp", return_value=str(artifact_parent)
            ),
            mock.patch.object(
                supervisor.shutil, "copy2", side_effect=OSError("disk full")
            ),
        ):
            with self.assertRaises(OSError):
                supervisor.build_server_binary(
                    server_directory, pathlib.Path("/unused/build-token")
                )

        self.assertFalse(
            artifact_parent.exists(),
            "copy 失败后 mkdtemp 目录必须被就地清理，不得泄漏 bong-e2e-server-*",
        )


if __name__ == "__main__":
    unittest.main()
