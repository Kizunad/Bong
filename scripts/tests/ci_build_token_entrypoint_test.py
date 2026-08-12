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
            'exec </dev/null\n    cd "$REPO_ROOT/server"\n    exec env "$SERVER_BINARY"',
            'bong_server_write_record "$SERVER_PID" "$SERVER_BINARY"',
            'bong_server_rollback_pinned_managed_process',
            'bong_server_pinned_process_owns_ipv4_listener',
        ],
        "forbid": [
            r"(?m)^\s*nohup\s+.*build-token\.sh.*cargo\s+run\b",
            r"bong_server_stop_managed_for_replacement",
        ],
    },
    "scripts/preview/stop-server-headless.sh": {
        "required": ['bong_server_stop_managed_for_replacement "preview cleanup"'],
        "forbid": [r"(?m)^\s*kill\s"],
    },
    "scripts/e2e-redis.sh": {
        "required": [
            'build_token="$ROOT/scripts/build-token.sh"',
            'BONG_E2E_SUPERVISOR_TEST_MODE:-0',
            'supervisor="${BONG_E2E_SUPERVISOR:-$supervisor}"',
            'build_token="${BONG_E2E_BUILD_TOKEN:-$build_token}"',
            'server_directory="${BONG_E2E_SERVER_DIRECTORY:-$server_directory}"',
            "e2e supervisor test overrides require an explicit harness mode",
            'python3 "$build_helper"',
            '"$server_directory" "$cargo_target" "$build_token" "$build_timeout"',
            'python3 "$supervisor" "$server_directory" "$built_binary"',
            '2>>"$log_file"',
        ],
        "forbid": [
            r'build_token="\$\{BONG_E2E_BUILD_TOKEN:-\$ROOT/scripts/build-token\.sh\}"'
        ],
    },
    "scripts/lib/bong-pre-handshake-build.py": {
        "required": [
            '[str(build_token), "cargo", "build", "--release"]',
            "stdin=subprocess.DEVNULL",
            "close_fds=True",
            "start_new_session=True",
            "os.killpg(process.pid, signal.SIGKILL)",
        ],
        "forbid": [r"subprocess\.run"],
    },
    "scripts/lib/bong-process-group-supervisor.py": {
        "required": [
            "shutil.copy2(built_binary, artifact)",
            "[str(artifact)]",
        ],
        "forbid": [r"\bcargo\b", r"build_token", r"subprocess\.run"],
    },
    "scripts/start.sh": {
        "required": ['"$ROOT/scripts/build-token.sh" cargo build --release'],
        "forbid": [],
    },
    "scripts/smoke-law-engine.sh": {
        "required": [
            "'$ROOT/scripts/build-token.sh' cargo build",
            'server_target_root="${CARGO_TARGET_DIR:-$ROOT/server/target}"',
            'timeout 20s "$server_binary"',
        ],
        "forbid": [r"build-token\.sh' cargo run", r'exec \./target/debug/bong-server'],
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
        r"(?:^\s*|[;&|()]\s*|\bexec\s+|\bnohup\s+|\btimeout\s+\S+\s+)"
        r"(?:[A-Za-z_][A-Za-z0-9_]*=(?:[^\s;|&]+|'[^']*'|\"[^\"]*\")\s+)*"
        r"cargo\s+(?:build|check|clippy|fmt|metadata|new|run|test)\b"
    ),
    re.compile(r"(?:^\s*|[;&|()]\s*)(?:[A-Za-z_][A-Za-z0-9_]*=\S+\s+)*\./gradlew\s+"),
)


def _line_is_ignored(line: str) -> bool:
    return bool(re.match(r"^\s*(?:#|echo\b|info\b|check\b)", line))


def _tokenized_shell_command_start(line: str) -> int | None:
    shell_start = line.find("bash -lc")
    if shell_start < 0:
        return None
    shell_command = line[shell_start:]
    token_start = shell_command.find("build-token.sh")
    if token_start < 0:
        return None
    token_tail = shell_command[token_start + len("build-token.sh") :]
    if not re.search(r"\s+(?:cargo|gradle)\s+", token_tail):
        return None
    return shell_start


def direct_build_failures(relative: str, text: str) -> list[str]:
    failures: list[str] = []
    for line_number, line in enumerate(text.splitlines(), 1):
        if _line_is_ignored(line):
            continue
        tokenized_start = _tokenized_shell_command_start(line)
        for pattern in DIRECT_BUILD_PATTERNS:
            match = pattern.search(line)
            if match and (tokenized_start is None or match.start() >= tokenized_start):
                failures.append(
                    f"{relative}:{line_number}: direct build entrypoint bypasses build-token"
                )
                break
    return failures


def test_direct_build_detection() -> None:
    cases = (
        ("  cargo test", True),
        ("  CARGO_BUILD_JOBS=1 cargo test", True),
        ("  ./gradlew test", True),
        ("cargo test # build-token.sh", True),
        ("./gradlew test # build-token.sh", True),
        ('"$ROOT/scripts/build-token.sh" cargo test', False),
        ('"$ROOT/scripts/build-token.sh" gradle test', False),
        (
            'run_or_fail "server" "cargo test" bash -lc '
            '"cd server && scripts/build-token.sh cargo test"',
            False,
        ),
        (
            'run_or_fail "server" "cargo test" bash -lc '
            '"cd server && echo build-token.sh && cargo test"',
            True,
        ),
        ("# cargo test", False),
        ('echo "cargo test"', False),
    )
    for source, expected_failure in cases:
        actual_failure = bool(direct_build_failures("fixture.sh", source))
        if actual_failure != expected_failure:
            raise AssertionError(
                "direct build fixture mismatch: "
                f"source={source!r}, expected_failure={expected_failure}, "
                f"actual_failure={actual_failure}"
            )


def test_e2e_supervisor_overrides_are_test_only() -> None:
    source = (ROOT / "scripts/e2e-redis.sh").read_text(encoding="utf-8")
    start = source.index("start_server_process_group() {")
    end = source.index("\n}\n\nstop_server() {", start)
    function = source[start:end]

    gate = function.index('if [ "${BONG_E2E_SUPERVISOR_TEST_MODE:-0}" = "1" ]; then')
    harness_mode = function.index(
        'test_override_mode="${3:-0}"'
    )
    github_rejection = function.index(
        'e2e supervisor test overrides require an explicit harness mode', gate
    )
    override_assignment = function.index(
        'build_token="${BONG_E2E_BUILD_TOKEN:-$build_token}"', github_rejection
    )
    production_rejection = function.index(
        "e2e supervisor overrides require BONG_E2E_SUPERVISOR_TEST_MODE=1",
        override_assignment,
    )
    build = function.index(
        'python3 "$build_helper"', production_rejection
    )
    supervisor_launch = function.index(
        'python3 "$supervisor" "$server_directory" "$built_binary"', build
    )
    if not harness_mode < gate < github_rejection < override_assignment < production_rejection < build < supervisor_launch:
        raise AssertionError(
            "e2e supervisor override gate must reject CI and production bypasses before launch"
        )


def main() -> int:
    test_direct_build_detection()
    test_e2e_supervisor_overrides_are_test_only()
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
        failures.extend(direct_build_failures(relative, text))

    if failures:
        print("CI build-token entrypoint contract FAILED", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        return 1

    print(f"CI build-token entrypoint contract PASS ({len(SPECS)} files)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
