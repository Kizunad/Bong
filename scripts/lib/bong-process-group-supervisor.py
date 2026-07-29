#!/usr/bin/env python3
"""Keep an E2E server process group owned by a pinned, persistent leader."""

from __future__ import annotations

import os
import signal
import subprocess
import sys
import time


def ignore_signal(_signal_number: int, _frame: object) -> None:
    """Keep the supervisor alive while its process-group members are stopped."""


def private_process_group() -> int:
    """Return this supervisor's dedicated session/group or fail closed."""

    pid = os.getpid()
    pgid = os.getpgrp()
    sid = os.getsid(0)
    if pid <= 1 or pgid <= 1 or sid <= 1 or pgid != pid or sid != pid:
        raise RuntimeError(
            f"refusing rollback outside a private session: pid={pid} pgid={pgid} sid={sid}"
        )
    return pgid


def process_group_has_other_members(pgid: int) -> bool:
    """Return whether this Linux process group still contains a non-zombie peer."""

    try:
        entries = os.scandir("/proc")
    except OSError as error:
        raise RuntimeError(f"could not inspect /proc for rollback: {error}") from error

    with entries:
        for entry in entries:
            if not entry.name.isdecimal() or int(entry.name) == os.getpid():
                continue
            try:
                with open(f"/proc/{entry.name}/stat", encoding="utf-8") as handle:
                    raw = handle.read()
                close = raw.rfind(") ")
                if close < 0:
                    raise ValueError("malformed /proc stat")
                fields = raw[close + 2 :].split()
                if len(fields) < 3:
                    raise ValueError("short /proc stat")
                state, member_pgid = fields[0], int(fields[2])
            except FileNotFoundError:
                continue
            except (OSError, ValueError) as error:
                raise RuntimeError(
                    f"could not inspect process {entry.name} during rollback: {error}"
                ) from error
            if state != "Z" and member_pgid == pgid:
                return True
    return False


def wait_for_process_group_peers(pgid: int, timeout: float) -> bool:
    deadline = time.monotonic() + timeout
    while process_group_has_other_members(pgid):
        if time.monotonic() >= deadline:
            return False
        time.sleep(0.05)
    return True


def rollback_server(server: subprocess.Popen[bytes] | None) -> bool:
    """Stop every peer in the uncommitted private session and reap the child."""

    try:
        pgid = private_process_group()
    except RuntimeError as error:
        print(str(error), file=sys.stderr)
        return False
    try:
        os.killpg(pgid, signal.SIGTERM)
    except ProcessLookupError:
        pass

    try:
        group_gone = wait_for_process_group_peers(pgid, 10.0)
    except RuntimeError as error:
        print(str(error), file=sys.stderr)
        # The private-session invariant was proven immediately before TERM.
        # Revalidate it before escalation so an unexpected process context can
        # never turn rollback into a caller-group signal.
        try:
            private_process_group()
        except RuntimeError as boundary_error:
            print(str(boundary_error), file=sys.stderr)
            return False
        os.killpg(pgid, signal.SIGKILL)
        return False
    if not group_gone:
        try:
            private_process_group()
            os.killpg(pgid, signal.SIGKILL)
        except RuntimeError as error:
            print(str(error), file=sys.stderr)
        except ProcessLookupError:
            pass
        # SIGKILL includes this supervisor, so this branch cannot claim success.
        return False

    if server is not None:
        try:
            server.wait(timeout=1)
        except subprocess.TimeoutExpired:
            print("direct server child remained after rollback group exit", file=sys.stderr)
            return False
    return True


def main() -> int:
    if len(sys.argv) != 3:
        print(
            "usage: bong-process-group-supervisor.py SERVER_DIRECTORY BUILD_TOKEN",
            file=sys.stderr,
        )
        return 2

    try:
        os.setsid()
        private_process_group()
    except (OSError, RuntimeError) as error:
        print(f"failed to establish dedicated server session: {error}", file=sys.stderr)
        return 2

    # Caught dispositions reset to SIG_DFL across exec, so the server receives the
    # TERM/HUP defaults while this process remains as the non-reusable group owner.
    for signal_number in (signal.SIGINT, signal.SIGHUP, signal.SIGTERM):
        signal.signal(signal_number, ignore_signal)

    server: subprocess.Popen[bytes] | None = None
    try:
        server = subprocess.Popen(
            [sys.argv[2], "cargo", "run", "--release"],
            cwd=sys.argv[1],
            close_fds=True,
            stdin=subprocess.DEVNULL,
            stdout=sys.stderr,
            stderr=sys.stderr,
        )
        sys.stdout.buffer.write(f"READY pid={os.getpid()}\n".encode())
        sys.stdout.buffer.flush()
    except (OSError, BrokenPipeError) as error:
        print(f"failed to launch or publish release server readiness: {error}", file=sys.stderr)
        return 2 if rollback_server(server) else 3

    # Authority is not committed until the parent has pinned this PID, starttime,
    # executable identity, and PGID. EOF, read failure, or any byte other than C
    # rolls this newly-created private session back from inside its verified owner.
    try:
        command = sys.stdin.buffer.read(1)
    except OSError as error:
        print(f"failed to read startup commit command: {error}", file=sys.stderr)
        return 2 if rollback_server(server) else 3
    if command != b"C":
        return 2 if rollback_server(server) else 3

    # Publishing authority requires a second, post-consumption boundary: the
    # parent only trusts this exact flushed acknowledgement.
    try:
        sys.stdout.buffer.write(b"COMMITTED\n")
        sys.stdout.buffer.flush()
    except (OSError, BrokenPipeError) as error:
        print(f"failed to publish startup commit acknowledgement: {error}", file=sys.stderr)
        return 2 if rollback_server(server) else 3

    server.wait()

    # Do not relinquish the session/process-group identity when the direct child
    # exits. The E2E owner pins this PID/starttime/executable and removes us only
    # after every other group member is confirmed gone.
    while True:
        signal.pause()


if __name__ == "__main__":
    raise SystemExit(main())
