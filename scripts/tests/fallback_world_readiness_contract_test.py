#!/usr/bin/env python3
"""Pin shell harnesses to the canonical fallback-world readiness contract."""

from __future__ import annotations

import pathlib
import re
import subprocess
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
READY_PREFIX = r"\[bong\]\[world\] BOT_FALLBACK_FLAT_READY"
TRACING_INFO_PREFIX = (
    r"(?:[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}"
    r"(?:\.[0-9]+)?Z\s+INFO\s+)?"
)
READY_PATTERN = re.compile(
    rf"^{TRACING_INFO_PREFIX}{READY_PREFIX} anchors=[1-9][0-9]* "
    r"chunks=[1-9][0-9]* view_distance_chunks=[1-9][0-9]*$"
)
STALE_MARKER = "creating overworld test area"
HARNESS_PATHS = (
    pathlib.Path("scripts/e2e-redis.sh"),
    pathlib.Path("scripts/e2e-offscreen-war.sh"),
    pathlib.Path("scripts/smoke-test.sh"),
    pathlib.Path("scripts/smoke-tiandao-fullstack.sh"),
)


class FallbackWorldReadinessContractTest(unittest.TestCase):
    def test_accepts_complete_numeric_marker(self) -> None:
        for line in (
            "[bong][world] BOT_FALLBACK_FLAT_READY anchors=3 chunks=5002 "
            "view_distance_chunks=20",
            "2026-08-11T23:25:37.123456Z  INFO [bong][world] "
            "BOT_FALLBACK_FLAT_READY anchors=3 chunks=1530 view_distance_chunks=4",
            "[bong][world] BOT_FALLBACK_FLAT_READY anchors=4294967295 "
            "chunks=8192 view_distance_chunks=18446744073709551615",
        ):
            with self.subTest(line=line):
                self.assertRegex(line, READY_PATTERN)

    def test_rejects_stale_incomplete_and_near_markers(self) -> None:
        for line in (
            "",
            "[bong][world] BOT_FALLBACK_FLAT_READY anchors=0 chunks=0 "
            "view_distance_chunks=0",
            "[bong][world] BOT_FALLBACK_FLAT_READY anchors=3 chunks=0 "
            "view_distance_chunks=20",
            "[bong][world] BOT_FALLBACK_FLAT_READY anchors=3 chunks=5002 "
            "view_distance_chunks=0",
            "[bong][world] creating overworld test area (16x16 chunks)",
            "[bong][world] BOT_FALLBACK_FLAT_READY",
            "[bong][world] BOT_FALLBACK_FLAT_READY anchors=3 chunks=1530",
            "[bong][world] BOT_FALLBACK_FLAT_READY anchors=3 chunks=1530 "
            "view_distance_chunks=four",
            "[bong][world] BOT_FALLBACK_FLAT_READY anchors=-1 chunks=1530 "
            "view_distance_chunks=4",
            "[bong][world] BOT_FALLBACK_FLAT_READY anchors=3 chunks=1.5 "
            "view_distance_chunks=4",
            "[bong][world] BOT_FALLBACK_FLAT_READYISH anchors=3 chunks=1530 "
            "view_distance_chunks=4",
            "2026-08-11T23:25:37.123456Z  INFO [bong][world] "
            "BOT_FALLBACK_FLAT_READY anchors=3 chunks=1530 "
            "view_distance_chunks=4 trailing-garbage",
            "prefix [bong][world] BOT_FALLBACK_FLAT_READY anchors=3 chunks=1530 "
            "view_distance_chunks=4",
            "2026-08-11T23:25:37.123456Z  WARN [bong][world] "
            "BOT_FALLBACK_FLAT_READY anchors=3 chunks=1530 view_distance_chunks=4",
            "2026-08-11 23:25:37 INFO [bong][world] "
            "BOT_FALLBACK_FLAT_READY anchors=3 chunks=1530 view_distance_chunks=4",
        ):
            with self.subTest(line=line):
                self.assertNotRegex(line, READY_PATTERN)

    def test_shell_ere_matches_the_same_contract(self) -> None:
        source = (ROOT / "scripts/bot-e2e.sh").read_text(encoding="utf-8")
        assignment = re.search(
            r"^BOT_FALLBACK_READY_PATTERN='([^']+)'$", source, re.MULTILINE
        )
        self.assertIsNotNone(assignment)
        shell_pattern = assignment.group(1)
        cases = (
            (
                "[bong][world] BOT_FALLBACK_FLAT_READY anchors=3 chunks=1530 "
                "view_distance_chunks=4\n",
                0,
            ),
            (
                "2026-08-11T23:25:37.123456Z  INFO [bong][world] "
                "BOT_FALLBACK_FLAT_READY anchors=3 chunks=1530 "
                "view_distance_chunks=4\n",
                0,
            ),
            (
                "2026-08-11T23:25:37.123456Z  WARN [bong][world] "
                "BOT_FALLBACK_FLAT_READY anchors=3 chunks=1530 "
                "view_distance_chunks=4\n",
                1,
            ),
            (
                "prefix [bong][world] BOT_FALLBACK_FLAT_READY anchors=3 "
                "chunks=1530 view_distance_chunks=4\n",
                1,
            ),
            (
                "[bong][world] BOT_FALLBACK_FLAT_READY anchors=3 chunks=1530 "
                "view_distance_chunks=four\n",
                1,
            ),
            (
                "[bong][world] BOT_FALLBACK_FLAT_READY anchors=0 chunks=0 "
                "view_distance_chunks=0\n",
                1,
            ),
            (
                "2026-08-11T23:25:37.123456Z  INFO [bong][world] "
                "BOT_FALLBACK_FLAT_READYISH anchors=3 chunks=1530 "
                "view_distance_chunks=4\n",
                1,
            ),
            ("[bong][world] creating overworld test area (16x16 chunks)\n", 1),
            ("", 1),
        )
        for contents, expected_status in cases:
            with self.subTest(contents=contents):
                with tempfile.NamedTemporaryFile("w", encoding="utf-8") as log:
                    log.write(contents)
                    log.flush()
                    matched = subprocess.run(
                        ["grep", "-Eq", shell_pattern, log.name],
                        check=False,
                    )
                self.assertEqual(matched.returncode, expected_status)

    def test_all_fallback_harnesses_share_one_canonical_marker(self) -> None:
        for relative in HARNESS_PATHS:
            source = (ROOT / relative).read_text(encoding="utf-8")
            with self.subTest(path=relative.as_posix()):
                self.assertNotIn(
                    STALE_MARKER,
                    source,
                    "已删除的 fallback 启服文案不得继续充当 readiness anchor",
                )
                self.assertIn(
                    "BOT_FALLBACK_FLAT_READY",
                    source,
                    "所有 fallback harness 必须等待生产端 canonical marker",
                )

    def test_e2e_redis_reuses_one_pattern_for_both_servers(self) -> None:
        source = (ROOT / "scripts/e2e-redis.sh").read_text(encoding="utf-8")
        assignment = (
            "FALLBACK_WORLD_READY_PATTERN='\\[bong\\]\\[world\\] "
            "BOT_FALLBACK_FLAT_READY anchors=[0-9]+ chunks=[0-9]+ "
            "view_distance_chunks=[0-9]+'"
        )
        self.assertIn(assignment, source)
        self.assertEqual(
            source.count('wait_for_pattern "$SERVER_LOG" "$FALLBACK_WORLD_READY_PATTERN"'),
            1,
        )
        self.assertEqual(
            source.count(
                'wait_for_pattern "$NORTH_RIFT_SERVER_LOG" '
                '"$FALLBACK_WORLD_READY_PATTERN"'
            ),
            1,
        )
    def test_bot_e2e_uses_structured_readiness_for_every_ownership_check(self) -> None:
        source = (ROOT / "scripts/bot-e2e.sh").read_text(encoding="utf-8")
        assignment = re.search(
            r"^BOT_FALLBACK_READY_PATTERN='([^']+)'$", source, re.MULTILINE
        )
        self.assertIsNotNone(assignment)
        pattern = assignment.group(1)
        self.assertTrue(pattern.startswith("^(") and pattern.endswith("$"))
        self.assertIn("INFO[[:space:]]+", pattern)
        self.assertNotIn("WARN", pattern)
        self.assertNotIn("BOT_FALLBACK_READY_PAYLOAD", source)
        self.assertEqual(
            source.count("fallback_ready_marker_present"),
            4,
        )
        self.assertEqual(
            source.count('grep -E -- "$BOT_FALLBACK_READY_PATTERN" >/dev/null'),
            1,
        )
        self.assertEqual(
            source.count("sed -E $'s/\\x1b\\\\[[0-9;]*[[:alpha:]]//g'"),
            1,
        )

    def test_owned_fallback_uses_private_redis_predicate(self) -> None:
        source = (ROOT / "scripts/bot-e2e.sh").read_text(encoding="utf-8")
        self.assertIn(
            '[ "$REUSE" != "1" ] && [ "$OWNED_WORLD_MODE" != "1" ] '
            '&& [ -z "${REDIS_URL:-}" ]',
            source,
        )
        self.assertIn(
            '[ "$REUSE" != "1" ] && { [ "$OWNED_WORLD_MODE" = "1" ] '
            '|| [ -z "${REDIS_URL:-}" ]; }',
            source,
        )

    def test_ci_runs_this_contract_suite(self) -> None:
        source = (ROOT / ".github/workflows/e2e.yml").read_text(encoding="utf-8")
        self.assertEqual(
            source.count("python3 scripts/tests/fallback_world_readiness_contract_test.py"),
            1,
        )


if __name__ == "__main__":
    result = unittest.main(exit=False)
    if result.result.wasSuccessful():
        print(
            "fallback world readiness contract PASS "
            f"({result.result.testsRun} tests, {len(HARNESS_PATHS)} harnesses)"
        )
    sys.exit(not result.result.wasSuccessful())
