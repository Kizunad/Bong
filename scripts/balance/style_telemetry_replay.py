#!/usr/bin/env python3
"""Aggregate style-balance telemetry and flag physics drift.

Input is JSONL from bong:style_balance_telemetry. The script only needs the
optional physical fields added by plan-style-balance-v1; rows without enough
data are reported as incomplete instead of guessed.
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from collections import defaultdict
from collections.abc import Iterable, Iterator
from dataclasses import dataclass
from pathlib import Path
from typing import NoReturn

QI_DECAY_PER_BLOCK = 0.03
QI_EXCRETION_BASE = 0.30
DEFAULT_THRESHOLD = 0.30
DISTANCE_BUCKET_STEP = 1.0


SAMPLE_EVENTS = [
    {
        "attacker_style": "baomai",
        "defender_style": "jiemai",
        "attacker_rejection_rate": 0.65,
        "defender_resistance": 0.95,
        "attacker_qi": 20.0,
        "distance_blocks": 0.0,
        "defender_lost": 0.59,
    },
    {
        "attacker_style": "dugu",
        "defender_style": "jiemai",
        "attacker_rejection_rate": 0.05,
        "defender_resistance": 0.95,
        "attacker_qi": 5.0,
        "distance_blocks": 0.0,
        "defender_lost": 0.18,
    },
    {
        "attacker_style": "anqi",
        "defender_style": "jiemai",
        "attacker_rejection_rate": 0.45,
        "defender_resistance": 0.95,
        "attacker_qi": 8.0,
        "distance_blocks": 0.0,
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


GroupKey = tuple[str, str, str]


def fail(message: str) -> NoReturn:
    raise SystemExit(message)


def finite_number(value: object) -> float | None:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return None
    number = float(value)
    return number if math.isfinite(number) else None


def distance_bucket(value: object) -> tuple[str, float | None]:
    distance = finite_number(value)
    if distance is None:
        return "unknown", None
    bucket = round(max(0.0, distance) / DISTANCE_BUCKET_STEP) * DISTANCE_BUCKET_STEP
    return f"{bucket:.1f}", bucket


def expected_efficiency_pct(rejection_rate: float, resistance: float, distance_blocks: float) -> float:
    """Approximate Rust core formula under neutral env and default medium."""
    rho = max(0.0, min(1.0, rejection_rate))
    r = max(0.0, min(1.0, resistance))
    attenuated_fraction = (1.0 - QI_DECAY_PER_BLOCK) ** max(0.0, distance_blocks)
    effective_fraction = attenuated_fraction * max(0.0, 1.0 - QI_EXCRETION_BASE * (rho + r * 0.5))
    mitigation_fraction = 1.0 - min(r, 0.95)
    return effective_fraction * mitigation_fraction * 100.0


def observed_efficiency_pct(event: dict) -> float | None:
    if "observed_efficiency_pct" in event:
        value = finite_number(event["observed_efficiency_pct"])
        return value if value is not None else None

    attacker_qi = finite_number(event.get("attacker_qi"))
    defender_lost = finite_number(event.get("defender_lost"))
    if attacker_qi is None or defender_lost is None:
        return None
    if attacker_qi <= 0:
        return None
    return max(0.0, defender_lost) / attacker_qi * 100.0


def load_events(path: Path | None, *, sample: bool) -> Iterator[dict]:
    if sample:
        for event in SAMPLE_EVENTS:
            yield dict(event)
        return

    source = sys.stdin if path is None else path.open("r", encoding="utf-8")
    close_source = path is not None
    try:
        for line_no, line in enumerate(source, start=1):
            stripped = line.strip()
            if not stripped:
                continue
            try:
                event = json.loads(stripped)
            except json.JSONDecodeError as exc:
                fail(f"invalid JSONL at line {line_no}: {exc}")
            if not isinstance(event, dict):
                fail(f"line {line_no} is not a JSON object")
            yield event
    finally:
        if close_source:
            source.close()


def aggregate(events: Iterable[dict]) -> dict[GroupKey, Aggregate]:
    groups: dict[GroupKey, Aggregate] = defaultdict(Aggregate)
    for event in events:
        attacker_style = event.get("attacker_style") or "unknown_attacker"
        defender_style = event.get("defender_style") or "unknown_defender"
        distance_label, distance = distance_bucket(event.get("distance_blocks"))
        group = groups[(str(attacker_style), str(defender_style), distance_label)]

        rho = finite_number(event.get("attacker_rejection_rate"))
        resistance = finite_number(event.get("defender_resistance"))
        observed = observed_efficiency_pct(event)
        if rho is None or resistance is None or distance is None or observed is None:
            group.add_incomplete()
            continue
        group.add_complete(expected_efficiency_pct(rho, resistance, distance), observed)
    return groups


def render_report(groups: dict[GroupKey, Aggregate], threshold: float) -> str:
    lines = [
        "attacker_style,defender_style,distance_blocks,samples,incomplete,expected_efficiency_pct,observed_efficiency_pct,relative_delta,status"
    ]
    for (attacker_style, defender_style, distance_label), group in sorted(groups.items()):
        expected = group.expected_avg
        observed = group.observed_avg
        if expected is None or observed is None:
            lines.append(
                f"{attacker_style},{defender_style},{distance_label},0,{group.incomplete},n/a,n/a,n/a,INCOMPLETE"
            )
            continue

        delta = abs(observed - expected) / max(expected, 1e-9)
        status = "DRIFT" if delta > threshold else "OK"
        lines.append(
            f"{attacker_style},{defender_style},{distance_label},{group.samples},{group.incomplete},"
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
    events = load_events(args.path, sample=args.sample)
    print(render_report(aggregate(events), max(0.0, args.threshold)))


if __name__ == "__main__":
    main()
