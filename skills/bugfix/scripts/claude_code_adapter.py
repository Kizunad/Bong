#!/usr/bin/env python3
"""Claude Code background-agent adapter for the BugFix scheduler.

This is an executable bridge over the installed ``claude`` CLI.  It emits JSON
only, so a host can use Bash for spawn/resume/status and Monitor for wait.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

TERMINAL_STATES = {"completed", "failed", "stopped"}


class AdapterError(RuntimeError):
    pass


def run(*args: str, cwd: Path | None = None) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        ["claude", *args], cwd=cwd, text=True, capture_output=True, check=False
    )
    if result.returncode != 0:
        raise AdapterError(
            f"claude {' '.join(args)} failed ({result.returncode}): "
            f"{result.stderr.strip() or result.stdout.strip()}"
        )
    return result


def sessions(cwd: Path, include_completed: bool = True) -> list[dict[str, Any]]:
    args = ["agents", "--json", "--cwd", str(cwd)]
    if include_completed:
        args.insert(2, "--all")
    payload = json.loads(run(*args).stdout)
    if not isinstance(payload, list):
        raise AdapterError("claude agents --json returned a non-list payload")
    return payload


def require_session(cwd: Path, session_id: str) -> dict[str, Any]:
    matches = [item for item in sessions(cwd) if item.get("sessionId") == session_id]
    if len(matches) != 1:
        raise AdapterError(
            f"expected one canonical sessionId={session_id}, found {len(matches)}"
        )
    return matches[0]


def parse_background_session(stdout: str, cwd: Path) -> dict[str, Any]:
    first = stdout.splitlines()[0] if stdout.splitlines() else ""
    if not first.startswith("backgrounded · "):
        raise AdapterError(f"unexpected background spawn output: {stdout!r}")
    short_id = first.removeprefix("backgrounded · ").strip()
    matches = [item for item in sessions(cwd) if item.get("id") == short_id]
    if len(matches) != 1 or not matches[0].get("sessionId"):
        raise AdapterError(f"cannot resolve canonical session ID for {short_id}")
    return matches[0]


def spawn(cwd: Path, prompt: str, model: str | None, permission_mode: str) -> dict[str, Any]:
    args = ["--background", "--permission-mode", permission_mode]
    if model:
        args.extend(["--model", model])
    args.append(prompt)
    return parse_background_session(run(*args, cwd=cwd).stdout, cwd)


def resume(
    cwd: Path, session_id: str, prompt: str, model: str | None, permission_mode: str
) -> dict[str, Any]:
    require_session(cwd, session_id)
    args = [
        "--resume",
        session_id,
        "--background",
        "--permission-mode",
        permission_mode,
    ]
    if model:
        args.extend(["--model", model])
    args.append(prompt)
    item = parse_background_session(run(*args, cwd=cwd).stdout, cwd)
    item["resumedFromSessionId"] = session_id
    return item


def wait(cwd: Path, session_id: str, interval: float, timeout: float) -> dict[str, Any]:
    deadline = time.monotonic() + timeout
    first_poll = True
    while True:
        item = require_session(cwd, session_id)
        state = item.get("state")
        status = item.get("status")
        if state in TERMINAL_STATES:
            return item
        # A freshly backgrounded session is briefly reported as working/idle
        # before its process changes to busy. Require one settled poll before
        # accepting idle, otherwise wait returns before the task starts.
        if status == "idle" and not first_poll:
            return item
        if time.monotonic() >= deadline:
            raise AdapterError(f"wait timed out for {session_id} in state={state!r}")
        first_poll = False
        time.sleep(interval)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    parser.add_argument("--cwd", type=Path, required=True)
    sub = parser.add_subparsers(dest="command", required=True)

    spawn_parser = sub.add_parser("spawn")
    spawn_parser.add_argument("--prompt", required=True)
    spawn_parser.add_argument("--model")
    spawn_parser.add_argument("--permission-mode", default="plan")

    status_parser = sub.add_parser("status")
    status_parser.add_argument("--session-id")

    resume_parser = sub.add_parser("resume")
    resume_parser.add_argument("--session-id", required=True)
    resume_parser.add_argument("--prompt", required=True)
    resume_parser.add_argument("--model")
    resume_parser.add_argument("--permission-mode", default="plan")

    wait_parser = sub.add_parser("wait")
    wait_parser.add_argument("--session-id", required=True)
    wait_parser.add_argument("--interval", type=float, default=2.0)
    wait_parser.add_argument("--timeout", type=float, default=1200.0)
    return parser


def main() -> int:
    args = build_parser().parse_args()
    cwd = args.cwd.resolve()
    try:
        if args.command == "spawn":
            payload: Any = spawn(cwd, args.prompt, args.model, args.permission_mode)
        elif args.command == "status":
            payload = (
                require_session(cwd, args.session_id)
                if args.session_id
                else sessions(cwd)
            )
        elif args.command == "resume":
            payload = resume(
                cwd,
                args.session_id,
                args.prompt,
                args.model,
                args.permission_mode,
            )
        else:
            payload = wait(cwd, args.session_id, args.interval, args.timeout)
        print(json.dumps(payload, ensure_ascii=False, sort_keys=True))
        return 0
    except (AdapterError, json.JSONDecodeError, OSError) as error:
        print(json.dumps({"error": str(error)}, ensure_ascii=False), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
