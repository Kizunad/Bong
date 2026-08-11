#!/usr/bin/env python3
"""Keep an E2E server process group owned by a pinned, persistent leader."""

from __future__ import annotations

import os
import shutil
import signal
import subprocess
import sys
import tempfile
import time
from pathlib import Path


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


def build_server_binary(server_directory: Path, build_token: Path) -> Path:
    environment = os.environ.copy()
    target_root = Path(
        environment.get("CARGO_TARGET_DIR", str(server_directory / "target"))
    )
    if not target_root.is_absolute():
        target_root = server_directory / target_root
    subprocess.run(
        [str(build_token), "cargo", "build", "--release"],
        cwd=server_directory,
        env=environment,
        stdin=subprocess.DEVNULL,
        stdout=sys.stderr,
        stderr=sys.stderr,
        check=True,
        close_fds=True,
    )
    built_binary = target_root / "release" / "bong-server"
    if not built_binary.is_file():
        raise RuntimeError(f"successful cargo build did not produce {built_binary}")
    artifact_dir = Path(tempfile.mkdtemp(prefix="bong-e2e-server-"))
    artifact = artifact_dir / "bong-server"
    try:
        shutil.copy2(built_binary, artifact)
        artifact.chmod(0o700)
    except Exception:
        # Ownership of the temporary directory transfers to the caller only after
        # all fallible artifact setup has completed; a copy/chmod failure must not
        # leak a bong-e2e-server-* directory in the system temporary area.
        shutil.rmtree(artifact_dir, ignore_errors=True)
        raise
    return artifact


def remove_artifact(artifact: Path | None) -> None:
    if artifact is None:
        return
    try:
        artifact.unlink(missing_ok=True)
        artifact.parent.rmdir()
    except OSError as error:
        print(f"failed to remove immutable server artifact: {error}", file=sys.stderr)


def abort_startup(artifact: Path | None, server: subprocess.Popen[bytes] | None) -> int:
    # Remove the temporary artifact before rollback: a stubborn server makes
    # rollback escalate to SIGKILL of this entire private group, including the
    # supervisor itself, so nothing after the rollback call is guaranteed to run.
    # Once the server has been spawned, the copied binary has no further use.
    remove_artifact(artifact)
    rolled_back = rollback_server(server)
    return 2 if rolled_back else 3


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
    artifact: Path | None = None
    try:
        server_directory = Path(sys.argv[1]).resolve(strict=True)
        build_token = Path(sys.argv[2]).resolve(strict=True)
        artifact = build_server_binary(server_directory, build_token)
        server = subprocess.Popen(
            [str(artifact)],
            cwd=server_directory,
            close_fds=True,
            stdin=subprocess.DEVNULL,
            stdout=sys.stderr,
            stderr=sys.stderr,
        )
        sys.stdout.buffer.write(f"READY pid={os.getpid()}\n".encode())
        sys.stdout.buffer.flush()
    except (OSError, RuntimeError, subprocess.CalledProcessError, BrokenPipeError) as error:
        print(f"failed to build, launch, or publish release server readiness: {error}", file=sys.stderr)
        return abort_startup(artifact, server)

    # Authority is not committed until the parent has pinned this PID, starttime,
    # executable identity, and PGID. EOF, read failure, or any byte other than C
    # rolls this newly-created private session back from inside its verified owner.
    try:
        command = sys.stdin.buffer.read(1)
    except OSError as error:
        print(f"failed to read startup commit command: {error}", file=sys.stderr)
        return abort_startup(artifact, server)
    if command != b"C":
        return abort_startup(artifact, server)

    # Publishing authority requires a second, post-consumption boundary: the
    # parent only trusts this exact flushed acknowledgement.
    try:
        sys.stdout.buffer.write(b"COMMITTED\n")
        sys.stdout.buffer.flush()
    except (OSError, BrokenPipeError) as error:
        print(f"failed to publish startup commit acknowledgement: {error}", file=sys.stderr)
        return abort_startup(artifact, server)

    server.wait()
    remove_artifact(artifact)

    # Do not relinquish the session/process-group identity when the direct child
    # exits. The E2E owner pins this PID/starttime/executable and removes us only
    # after every other group member is confirmed gone.
    while True:
        signal.pause()


if __name__ == "__main__":
    raise SystemExit(main())
