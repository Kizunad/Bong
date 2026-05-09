from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

from .fields import LAYER_REGISTRY


def layer_registry_entries() -> list[dict[str, Any]]:
    return [
        {
            "name": name,
            "export_type": spec.export_type,
            "safe_default": float(spec.safe_default),
        }
        for name, spec in LAYER_REGISTRY.items()
    ]


def dump_layer_registry_json() -> str:
    return json.dumps(layer_registry_entries(), ensure_ascii=False, indent=2) + "\n"


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Dump terrain_gen.fields::LAYER_REGISTRY as the Rust fixture JSON."
    )
    parser.add_argument(
        "--output",
        type=Path,
        help="Optional output path. Prints to stdout when omitted.",
    )
    args = parser.parse_args()

    payload = dump_layer_registry_json()
    if args.output is None:
        print(payload, end="")
        return

    args.output.write_text(payload, encoding="utf-8")


if __name__ == "__main__":
    main()
