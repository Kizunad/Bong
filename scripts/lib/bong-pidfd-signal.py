#!/usr/bin/env python3
"""Identity-safe Linux process signaling through pidfds."""

from __future__ import annotations

import os
import signal
import sys
from pathlib import Path


def process_starttime_and_pgrp(pid: int) -> tuple[str, str]:
    raw = Path(f"/proc/{pid}/stat").read_text(encoding="utf-8")
    close = raw.rfind(") ")
    if close < 0:
        raise ValueError("malformed /proc stat")
    fields = raw[close + 2 :].split()
    if len(fields) < 20:
        raise ValueError("short /proc stat")
    return fields[19], fields[2]


def executable_identity(pid: int) -> str:
    stat = os.stat(f"/proc/{pid}/exe")
    return f"{stat.st_dev}:{stat.st_ino}"


def main() -> int:
    if len(sys.argv) not in (5, 6):
        return 2

    try:
        pid = int(sys.argv[1])
    except ValueError:
        return 2
    expected_starttime = sys.argv[2]
    expected_executable_identity = sys.argv[3]
    signal_name = sys.argv[4]
    expected_pgrp = sys.argv[5] if len(sys.argv) == 6 else None
    if expected_pgrp is not None and not expected_pgrp.isdecimal():
        return 2
    try:
        signal_number = int(signal_name)
    except ValueError:
        signal_number = getattr(signal, f"SIG{signal_name}", 0)
    if signal_number <= 0:
        return 2

    if not hasattr(os, "pidfd_open") or not hasattr(signal, "pidfd_send_signal"):
        print("pidfd signaling is unavailable", file=sys.stderr)
        return 2

    try:
        pidfd = os.pidfd_open(pid, 0)
    except ProcessLookupError:
        return 1
    except (OSError, ValueError) as error:
        print(f"pidfd_open failed: {error}", file=sys.stderr)
        return 2

    try:
        try:
            actual_starttime, actual_pgrp = process_starttime_and_pgrp(pid)
            if actual_starttime != expected_starttime:
                return 1
            if expected_pgrp is not None and actual_pgrp != expected_pgrp:
                return 1
            if executable_identity(pid) != expected_executable_identity:
                return 1
        except FileNotFoundError:
            return 1
        except (OSError, ValueError) as error:
            print(f"process identity inspection failed: {error}", file=sys.stderr)
            return 2

        try:
            signal.pidfd_send_signal(pidfd, signal_number, None, 0)
        except ProcessLookupError:
            return 1
        except OSError as error:
            print(f"pidfd_send_signal failed: {error}", file=sys.stderr)
            return 2
    finally:
        os.close(pidfd)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
