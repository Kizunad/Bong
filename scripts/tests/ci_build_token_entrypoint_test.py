#!/usr/bin/env python3
"""Pin CI and harness Cargo/Gradle entrypoints to the shared build token."""

from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]

SPECS = {
    ".github/workflows/e2e.yml": {
        "cargo": [
            "../scripts/build-token.sh cargo build --release",
            "../scripts/build-token.sh cargo test",
        ],
        "gradle": ["../scripts/build-token.sh gradle test"],
        "forbid": [r"(?m)^\s*run:\s*cargo\b", r"(?m)^\s*run:\s*\./gradlew\b"],
    },
    ".github/workflows/worldgen-preview.yml": {
        "cargo": ["../scripts/build-token.sh cargo build --release"],
        "gradle": [
            "../scripts/build-token.sh gradle clean --no-daemon",
            "../scripts/build-token.sh gradle runClientPreview --no-daemon --stacktrace --rerun-tasks",
        ],
        "forbid": [
            r"(?m)^\s*run:\s*cargo\b",
            r"(?m)^\s*(?:xvfb-run[^\n]*\\\n\s*)?\./gradlew\b",
        ],
    },
    "scripts/smoke-test-e2e.sh": {
        "cargo": ['"$ROOT/scripts/build-token.sh" cargo test'],
        "gradle": [],
        "forbid": [r"(?m)^\s*cargo\s+(?:build|check|clippy|run|test)\b"],
    },
    "scripts/e2e-chat-signal-window.sh": {
        "cargo": [
            'exec "$ROOT/scripts/build-token.sh" cargo run --locked "${PROFILE_FLAG[@]}"'
        ],
        "gradle": [],
        "forbid": [r"(?m)^\s*exec\s+cargo\s+run\b"],
    },
    "scripts/preview/run-server-headless.sh": {
        "cargo": [
            'nohup "$REPO_ROOT/scripts/build-token.sh" cargo run --locked $PROFILE'
        ],
        "gradle": [],
        "forbid": [r"(?m)^\s*nohup\s+cargo\s+run\b"],
    },
}


def main() -> int:
    failures: list[str] = []
    for relative, spec in SPECS.items():
        path = ROOT / relative
        text = path.read_text(encoding="utf-8")
        for required in (*spec["cargo"], *spec["gradle"]):
            if required not in text:
                failures.append(f"{relative}: missing tokenized entrypoint {required!r}")
        for pattern in spec["forbid"]:
            match = re.search(pattern, text)
            if match:
                line = text.count("\n", 0, match.start()) + 1
                failures.append(
                    f"{relative}:{line}: direct build entrypoint bypasses build-token"
                )

    if failures:
        print("CI build-token entrypoint contract FAILED", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        return 1

    print(f"CI build-token entrypoint contract PASS ({len(SPECS)} files)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
