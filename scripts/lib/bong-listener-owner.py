#!/usr/bin/env python3
"""Verify that an exact pinned Linux process owns an IPv4 TCP listener."""

from __future__ import annotations

import os
import sys
from pathlib import Path


class InspectionError(RuntimeError):
    """The target remained present but its listener identity was uninspectable."""


def parse_process_snapshot(pid: int) -> tuple[str, str, str]:
    try:
        raw = Path(f"/proc/{pid}/stat").read_text(encoding="utf-8")
    except FileNotFoundError:
        raise ProcessLookupError(pid) from None
    except OSError as error:
        raise InspectionError(f"could not read /proc/{pid}/stat: {error}") from error

    close = raw.rfind(") ")
    if close < 0:
        raise InspectionError(f"malformed /proc/{pid}/stat")
    fields = raw[close + 2 :].split()
    if len(fields) < 20:
        raise InspectionError(f"short /proc/{pid}/stat")
    state, pgrp, starttime = fields[0], fields[2], fields[19]
    if not pgrp.isdecimal() or not starttime.isdecimal():
        raise InspectionError(f"invalid identity fields in /proc/{pid}/stat")
    if state == "Z":
        raise ProcessLookupError(pid)
    return starttime, pgrp, state


def executable_identity(pid: int) -> str:
    try:
        metadata = os.stat(f"/proc/{pid}/exe")
    except FileNotFoundError:
        raise ProcessLookupError(pid) from None
    except OSError as error:
        raise InspectionError(f"could not inspect /proc/{pid}/exe: {error}") from error
    return f"{metadata.st_dev}:{metadata.st_ino}"


def validate_identity(
    pid: int,
    expected_starttime: str,
    expected_executable_identity: str,
    expected_pgrp: str | None,
) -> bool:
    starttime, pgrp, _state = parse_process_snapshot(pid)
    identity = executable_identity(pid)
    return (
        starttime == expected_starttime
        and identity == expected_executable_identity
        and (expected_pgrp is None or pgrp == expected_pgrp)
    )


def listener_inodes_from_text(raw: str, port: int) -> set[str]:
    expected_port = f"{port:04X}"
    inodes: set[str] = set()
    lines = raw.splitlines()
    if not lines:
        raise InspectionError("empty IPv4 TCP table")

    for line in lines[1:]:
        if not line.strip():
            continue
        fields = line.split()
        if len(fields) < 10:
            raise InspectionError("malformed IPv4 TCP table row")
        local_endpoint, state, inode = fields[1], fields[3], fields[9]
        try:
            address, local_port = local_endpoint.split(":", 1)
        except ValueError as error:
            raise InspectionError("malformed IPv4 TCP local endpoint") from error
        if len(address) != 8 or len(local_port) != 4:
            raise InspectionError("malformed IPv4 TCP local endpoint width")
        try:
            int(address, 16)
            int(local_port, 16)
        except ValueError as error:
            raise InspectionError("non-hex IPv4 TCP local endpoint") from error
        if len(state) != 2:
            raise InspectionError("malformed IPv4 TCP state")
        try:
            int(state, 16)
        except ValueError as error:
            raise InspectionError("non-hex IPv4 TCP state") from error
        if not inode.isdecimal():
            raise InspectionError("non-decimal IPv4 TCP socket inode")
        if (
            state == "0A"
            and local_port.upper() == expected_port
            and address.upper() in {"00000000", "0100007F"}
        ):
            inodes.add(inode)
    return inodes


def read_listener_inodes(pid: int, port: int) -> set[str]:
    try:
        raw = Path(f"/proc/{pid}/net/tcp").read_text(encoding="utf-8")
    except FileNotFoundError:
        raise ProcessLookupError(pid) from None
    except OSError as error:
        raise InspectionError(f"could not read /proc/{pid}/net/tcp: {error}") from error
    return listener_inodes_from_text(raw, port)


def process_socket_inodes(pid: int) -> set[str]:
    directory = f"/proc/{pid}/fd"
    try:
        entries = os.scandir(directory)
    except FileNotFoundError:
        raise ProcessLookupError(pid) from None
    except OSError as error:
        raise InspectionError(f"could not enumerate {directory}: {error}") from error

    inodes: set[str] = set()
    with entries:
        for entry in entries:
            try:
                target = os.readlink(entry.path)
            except FileNotFoundError:
                # Unrelated descriptors may close while the table is enumerated.
                continue
            except OSError as error:
                raise InspectionError(f"could not inspect {entry.path}: {error}") from error
            if target.startswith("socket:[") and target.endswith("]"):
                inode = target[8:-1]
                if not inode.isdecimal():
                    raise InspectionError(f"malformed socket link at {entry.path}")
                inodes.add(inode)
    return inodes


def owns_listener(
    pid: int,
    expected_starttime: str,
    expected_executable_identity: str,
    port: int,
    expected_pgrp: str | None,
) -> bool:
    try:
        pidfd = os.pidfd_open(pid)
    except ProcessLookupError:
        return False
    except (AttributeError, OSError) as error:
        raise InspectionError(f"could not open pidfd for pid {pid}: {error}") from error

    try:
        if not validate_identity(
            pid, expected_starttime, expected_executable_identity, expected_pgrp
        ):
            return False
        listener_inodes = read_listener_inodes(pid, port)
        if not listener_inodes:
            return False
        if not listener_inodes.intersection(process_socket_inodes(pid)):
            return False
        return validate_identity(
            pid, expected_starttime, expected_executable_identity, expected_pgrp
        )
    finally:
        os.close(pidfd)


def main() -> int:
    if len(sys.argv) not in (5, 6):
        print(
            "usage: bong-listener-owner.py PID STARTTIME EXECUTABLE_IDENTITY PORT [PGRP]",
            file=sys.stderr,
        )
        return 2

    pid_text, expected_starttime, expected_identity, port_text = sys.argv[1:5]
    expected_pgrp = sys.argv[5] if len(sys.argv) == 6 else None
    if not pid_text.isdecimal() or not expected_starttime.isdecimal():
        return 2
    if expected_pgrp is not None and not expected_pgrp.isdecimal():
        return 2
    identity_parts = expected_identity.split(":", 1)
    if len(identity_parts) != 2 or not all(part.isdecimal() for part in identity_parts):
        return 2
    if not port_text.isdecimal() or not 1 <= int(port_text) <= 65535:
        return 2

    try:
        return (
            0
            if owns_listener(
                int(pid_text),
                expected_starttime,
                expected_identity,
                int(port_text),
                expected_pgrp,
            )
            else 1
        )
    except ProcessLookupError:
        return 1
    except InspectionError as error:
        print(str(error), file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
