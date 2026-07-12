#!/usr/bin/env python3
"""End-to-end test for the real Claude Code BugFix adapter."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
ADAPTER = Path(__file__).with_name("claude_code_adapter.py")
PROMPT_READY = "只回复字符串 BUGFIX_ADAPTER_E2E_READY，不调用工具。"
PROMPT_RESUMED = "只回复字符串 BUGFIX_ADAPTER_E2E_RESUMED，不调用工具。"


def run_adapter(*args: str) -> dict[str, object]:
    result = subprocess.run(
        [sys.executable, str(ADAPTER), "--cwd", str(ROOT), *args],
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        raise AssertionError(result.stderr or result.stdout)
    return json.loads(result.stdout)


def stop(short_id: object) -> None:
    if not isinstance(short_id, str):
        return
    subprocess.run(["claude", "stop", short_id], check=False, capture_output=True)


def wait_terminal(session_id: str, timeout: float = 180.0) -> dict[str, object]:
    return run_adapter(
        "wait",
        "--session-id",
        session_id,
        "--interval",
        "2",
        "--timeout",
        str(timeout),
    )


def main() -> int:
    spawned: dict[str, object] = {}
    resumed: dict[str, object] = {}
    try:
        spawned = run_adapter("spawn", "--prompt", PROMPT_READY)
        canonical = spawned.get("sessionId")
        assert isinstance(canonical, str) and canonical, spawned
        assert spawned.get("state") in {"working", "completed"}, spawned

        status = run_adapter("status", "--session-id", canonical)
        assert status.get("sessionId") == canonical, status
        terminal = wait_terminal(canonical)
        assert terminal.get("state") == "completed" or terminal.get("status") == "idle", terminal

        resumed = run_adapter(
            "resume", "--session-id", canonical, "--prompt", PROMPT_RESUMED
        )
        resumed_id = resumed.get("sessionId")
        assert isinstance(resumed_id, str) and resumed_id, resumed
        assert resumed.get("resumedFromSessionId") == canonical, resumed
        resumed_terminal = wait_terminal(resumed_id)
        assert resumed_terminal.get("state") == "completed" or resumed_terminal.get("status") == "idle", resumed_terminal

        print(
            json.dumps(
                {
                    "spawnSessionId": canonical,
                    "spawnState": terminal.get("state"),
                    "resumeSessionId": resumed_id,
                    "resumeState": resumed_terminal.get("state"),
                },
                ensure_ascii=False,
                sort_keys=True,
            )
        )
        return 0
    finally:
        stop(spawned.get("id"))
        stop(resumed.get("id"))


if __name__ == "__main__":
    raise SystemExit(main())
