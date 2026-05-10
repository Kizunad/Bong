#!/usr/bin/env python3
"""Aggregate style-balance telemetry and flag physics drift.

Input is JSONL from bong:style_balance_telemetry. The script only needs the
optional physical fields added by plan-style-balance-v1; rows without enough
data are reported as incomplete instead of guessed.
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

QI_EXCRETION_BASE = 0.30
DEFAULT_THRESHOLD = 0.30


SAMPLE_EVENTS = [
    {
        "attacker_style": "baomai",
        "defender_style": "jiemai",
        "attacker_rejection_rate": 0.65,
        "defender_resistance": 0.95,
        "attacker_qi": 20.0,
        "defender_lost": 0.59,
    },
    {
        "attacker_style": "dugu",
        "defender_style": "jiemai",
        "attacker_rejection_rate": 0.05,
        "defender_resistance": 0.95,
        "attacker_qi": 5.0,
        "defender_lost": 0.18,
    },
    {
        "attacker_style": "anqi",
        "defender_style": "jiemai",
        "attacker_rejection_rate": 0.45,
        "defender_resistance": 0.95,
        "attacker_qi": 8.0,
        "defender_lost": 0.26,
    },
]


@dataclass
class Aggregate:
    samples: int = 0
    incomplete: int = 0
    expected_sum: float = 0.0
    observed_sum: float = 0.0

    def add_complete(self, expected: float, observed: float) -> None:
        self.samples += 1
        self.expected_sum += expected
        self.observed_sum += observed

    def add_incomplete(self) -> None:
        self.incomplete += 1

    @property
    def expected_avg(self) -> float | None:
        return self.expected_sum / self.samples if self.samples else None

    @property
    def observed_avg(self) -> float | None:
        return self.observed_sum / self.samples if self.samples else None


def expected_efficiency_pct(rejection_rate: float, resistance: float) -> float:
    rho = max(0.0, min(1.0, rejection_rate))
    r = max(0.0, min(1.0, resistance))
    effective_fraction = max(0.0, 1.0 - QI_EXCRETION_BASE * (rho + r * 0.5))
    mitigation_fraction = 1.0 - min(r, 0.95)
    return effective_fraction * mitigation_fraction * 100.0


def observed_efficiency_pct(event: dict) -> float | None:
    if "observed_efficiency_pct" in event:
        value = event["observed_efficiency_pct"]
        return float(value) if isinstance(value, (int, float)) else None

    attacker_qi = event.get("attacker_qi")
    defender_lost = event.get("defender_lost")
    if not isinstance(attacker_qi, (int, float)) or not isinstance(defender_lost, (int, float)):
        return None
    if attacker_qi <= 0:
        return None
    return max(0.0, float(defender_lost)) / float(attacker_qi) * 100.0


def load_events(path: Path | None, sample: bool) -> list[dict]:
    if sample:
        return [dict(event) for event in SAMPLE_EVENTS]

    source = sys.stdin if path is None else path.open("r", encoding="utf-8")
    events: list[dict] = []
    with source:
        for line_no, line in enumerate(source, start=1):
            stripped = line.strip()
            if not stripped:
                continue
            try:
                event = json.loads(stripped)
            except json.JSONDecodeError as exc:
                raise SystemExit(f"invalid JSONL at line {line_no}: {exc}") from exc
            if not isinstance(event, dict):
                raise SystemExit(f"line {line_no} is not a JSON object")
            events.append(event)
    return events


def aggregate(events: Iterable[dict]) -> dict[tuple[str, str], Aggregate]:
    groups: dict[tuple[str, str], Aggregate] = defaultdict(Aggregate)
    for event in events:
        attacker_style = event.get("attacker_style") or "unknown_attacker"
        defender_style = event.get("defender_style") or "unknown_defender"
        group = groups[(str(attacker_style), str(defender_style))]

        rho = event.get("attacker_rejection_rate")
        resistance = event.get("defender_resistance")
        observed = observed_efficiency_pct(event)
        if not isinstance(rho, (int, float)) or not isinstance(resistance, (int, float)) or observed is None:
            group.add_incomplete()
            continue
        group.add_complete(expected_efficiency_pct(float(rho), float(resistance)), observed)
    return groups


def render_report(groups: dict[tuple[str, str], Aggregate], threshold: float) -> str:
    lines = [
        "attacker_style,defender_style,samples,incomplete,expected_efficiency_pct,observed_efficiency_pct,relative_delta,status"
    ]
    for (attacker_style, defender_style), group in sorted(groups.items()):
        expected = group.expected_avg
        observed = group.observed_avg
        if expected is None or observed is None:
            lines.append(
                f"{attacker_style},{defender_style},0,{group.incomplete},n/a,n/a,n/a,INCOMPLETE"
            )
            continue

        delta = abs(observed - expected) / max(expected, 1e-9)
        status = "DRIFT" if delta > threshold else "OK"
        lines.append(
            f"{attacker_style},{defender_style},{group.samples},{group.incomplete},"
            f"{expected:.3f},{observed:.3f},{delta:.3f},{status}"
        )
    return "\n".join(lines)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("path", nargs="?", type=Path, help="JSONL telemetry file; defaults to stdin")
    parser.add_argument("--sample", action="store_true", help="run the built-in smoke sample")
    parser.add_argument(
        "--threshold",
        type=float,
        default=DEFAULT_THRESHOLD,
        help="relative drift threshold; default 0.30",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    events = load_events(args.path, args.sample)
    print(render_report(aggregate(events), max(0.0, args.threshold)))


if __name__ == "__main__":
    main()
