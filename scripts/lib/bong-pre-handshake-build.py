#!/usr/bin/env python3
"""Build the release server before opening the supervisor READY transaction."""

from __future__ import annotations

import os
import signal
import subprocess
import sys
import time
from pathlib import Path


def process_group_has_live_members(pgid: int) -> bool:
    """Return whether a Linux process group still has a non-zombie member."""

    with os.scandir("/proc") as entries:
        for entry in entries:
            if not entry.name.isdecimal():
                continue
            try:
                raw = Path(f"/proc/{entry.name}/stat").read_text(encoding="utf-8")
                close = raw.rfind(") ")
                fields = raw[close + 2 :].split()
                state, member_pgid = fields[0], int(fields[2])
            except FileNotFoundError:
                continue
            except (OSError, ValueError, IndexError) as error:
                raise RuntimeError(
                    f"could not inspect build process {entry.name}: {error}"
                ) from error
            if state != "Z" and member_pgid == pgid:
                return True
    return False


def wait_for_build_group_exit(pgid: int, timeout: float) -> bool:
    deadline = time.monotonic() + timeout
    while process_group_has_live_members(pgid):
        if time.monotonic() >= deadline:
            return False
        time.sleep(0.05)
    return True


def stop_build_group(process: subprocess.Popen[bytes]) -> None:
    """Bound teardown to the private build session created by this helper."""

    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    if not wait_for_build_group_exit(process.pid, 5):
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        if not wait_for_build_group_exit(process.pid, 5):
            raise RuntimeError("private build process group survived SIGKILL")
    process.wait()


def main() -> int:
    if len(sys.argv) != 5:
        print(
            "usage: bong-pre-handshake-build.py SERVER_DIRECTORY CARGO_TARGET_DIR "
            "BUILD_TOKEN TIMEOUT_SECONDS",
            file=sys.stderr,
        )
        return 2

    try:
        server_directory = Path(sys.argv[1]).resolve(strict=True)
        cargo_target = Path(sys.argv[2])
        build_token = Path(sys.argv[3]).resolve(strict=True)
        timeout_seconds = int(sys.argv[4])
    except (OSError, ValueError) as error:
        print(f"invalid pre-handshake build input: {error}", file=sys.stderr)
        return 2
    if timeout_seconds <= 0 or not cargo_target.is_absolute():
        print("build timeout and cargo target must be positive/absolute", file=sys.stderr)
        return 2

    environment = os.environ.copy()
    environment["CARGO_TARGET_DIR"] = str(cargo_target)
    try:
        process = subprocess.Popen(
            [str(build_token), "cargo", "build", "--release"],
            cwd=server_directory,
            env=environment,
            stdin=subprocess.DEVNULL,
            close_fds=True,
            start_new_session=True,
        )
    except OSError as error:
        print(f"failed to launch release server build: {error}", file=sys.stderr)
        return 1

    try:
        return process.wait(timeout=timeout_seconds)
    except subprocess.TimeoutExpired:
        print(
            f"release server build exceeded {timeout_seconds}s; stopping private build session",
            file=sys.stderr,
        )
        stop_build_group(process)
        return 124


if __name__ == "__main__":
    raise SystemExit(main())
