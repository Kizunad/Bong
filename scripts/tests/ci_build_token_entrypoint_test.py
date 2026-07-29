#!/usr/bin/env python3
"""Pin CI and harness Cargo/Gradle entrypoints to the shared build token."""

from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]

SPECS = {
    ".github/workflows/e2e.yml": {
        "required": [
            "../scripts/build-token.sh cargo build --release",
            "../scripts/build-token.sh cargo test",
            "../scripts/build-token.sh gradle test",
        ],
        "forbid": [r"(?m)^\s*run:\s*cargo\b", r"(?m)^\s*run:\s*\./gradlew\b"],
    },
    ".github/workflows/worldgen-preview.yml": {
        "required": [
            "../scripts/build-token.sh cargo build --release",
            "../scripts/build-token.sh gradle clean --no-daemon",
            "../scripts/build-token.sh gradle runClientPreview --no-daemon --stacktrace --rerun-tasks",
            "bash scripts/preview/stop-server-headless.sh",
        ],
        "forbid": [
            r"(?m)^\s*run:\s*cargo\b",
            r"(?m)^\s*(?:xvfb-run[^\n]*\\\n\s*)?\./gradlew\b",
            r"(?m)^\s*kill\s+-TERM\s+\"?\$PID",
        ],
    },
    "scripts/smoke-test-e2e.sh": {
        "required": ['"$ROOT/scripts/build-token.sh" cargo test'],
        "forbid": [],
    },
    "scripts/e2e-chat-signal-window.sh": {
        "required": [
            'exec "$ROOT/scripts/build-token.sh" cargo run --locked "${PROFILE_FLAG[@]}"'
        ],
        "forbid": [],
    },
    "scripts/preview/run-server-headless.sh": {
        "required": [
            '"$REPO_ROOT/scripts/build-token.sh" cargo "${BUILD_ARGS[@]}"',
            'exec env "$SERVER_BINARY"',
            'bong_server_write_record "$SERVER_PID" "$SERVER_BINARY"',
            'bong_server_pinned_process_owns_ipv4_listener',
        ],
        "forbid": [r"(?m)^\s*nohup\s+.*build-token\.sh.*cargo\s+run\b"],
    },
    "scripts/preview/stop-server-headless.sh": {
        "required": ['bong_server_stop_managed_for_replacement "preview cleanup"'],
        "forbid": [r"(?m)^\s*kill\s"],
    },
    "scripts/e2e-redis.sh": {
        "required": [
            'build_token="${BONG_E2E_BUILD_TOKEN:-$ROOT/scripts/build-token.sh}"',
            'python3 "$supervisor" "$server_directory" "$build_token"',
        ],
        "forbid": [],
    },
    "scripts/lib/bong-process-group-supervisor.py": {
        "required": ['[sys.argv[2], "cargo", "run", "--release"]'],
        "forbid": [r'subprocess\.Popen\(\s*\["cargo"'],
    },
    "scripts/start.sh": {
        "required": ['"$ROOT/scripts/build-token.sh" cargo build --release'],
        "forbid": [],
    },
    "scripts/dev-reload.sh": {
        "required": [
            'ROOT="$(git rev-parse --show-toplevel)"',
            '(cd server && "$ROOT/scripts/build-token.sh" cargo build',
        ],
        "forbid": [r'\(cd server && "\$PWD/scripts/build-token\.sh"'],
    },
}

# Every executable repository shell script is a supported local/CI entrypoint.
# Direct Cargo/Gradle invocations are forbidden everywhere except the shared
# wrapper itself and isolated test fixtures that intentionally emulate it.
DIRECT_COMMAND_EXEMPTIONS = {
    "scripts/build-token.sh",
    "scripts/test-supervisor-protocol.sh",
    "scripts/test-server-lifecycle.sh",
}
DIRECT_BUILD_PATTERNS = (
    re.compile(
        r"(?m)^(?!\s*(?:#|echo\b|info\b|check\b))(?!.*build-token\.sh).*"
        r"(?:^|[;&|()]\s*|\bexec\s+|\bnohup\s+|\btimeout\s+\S+\s+)"
        r"cargo\s+(?:build|check|clippy|fmt|metadata|new|run|test)\b"
    ),
    re.compile(r"(?m)^(?!\s*(?:#|echo\b))(?!.*build-token\.sh).*\./gradlew\s+"),
)



def main() -> int:
    failures: list[str] = []
    for relative, spec in SPECS.items():
        path = ROOT / relative
        text = path.read_text(encoding="utf-8")
        for required in spec["required"]:
            if required not in text:
                failures.append(f"{relative}: missing tokenized entrypoint {required!r}")
        for pattern in spec["forbid"]:
            match = re.search(pattern, text)
            if match:
                line = text.count("\n", 0, match.start()) + 1
                failures.append(
                    f"{relative}:{line}: direct build entrypoint bypasses build-token"
                )

    for path in sorted((ROOT / "scripts").rglob("*.sh")):
        relative = path.relative_to(ROOT).as_posix()
        if relative in DIRECT_COMMAND_EXEMPTIONS:
            continue
        text = path.read_text(encoding="utf-8")
        for pattern in DIRECT_BUILD_PATTERNS:
            match = pattern.search(text)
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
