#!/usr/bin/env python3
"""No-signal regression tests for process authority boundaries."""

from __future__ import annotations

import importlib.util
import pathlib
import subprocess
import sys
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


if __name__ == "__main__":
    unittest.main()
