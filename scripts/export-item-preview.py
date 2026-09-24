#!/usr/bin/env python3
"""从服务端物品 TOML 导出原生 UI 预览配置，不维护另一份物品定义。"""

import argparse
import json
from pathlib import Path
import tomllib


def main() -> None:
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", type=Path, default=root / "client/window-ui-preview.json")
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--item", action="append", dest="item_ids")
    args = parser.parse_args()
    item_ids = args.item_ids or [
        "pickaxe_iron", "herb_bundle", "armor_iron_leggings", "grass_pouch", "weathered_stone"
    ]
    templates = {}
    for path in sorted((root / "server/assets/items").glob("*.toml")):
        with path.open("rb") as source:
            for item in tomllib.load(source).get("item", []):
                if item["id"] in item_ids:
                    if item["id"] in templates:
                        raise ValueError(f"重复物品定义: {item['id']}")
                    templates[item["id"]] = item
    config = json.loads(args.config.read_text(encoding="utf-8"))
    config["items"] = []
    for index, item_id in enumerate(item_ids, start=3101):
        item = templates[item_id]
        config["items"].append({
            "instance_id": index,
            "item_id": item["id"],
            "display_name": item["name"],
            "grid_width": item["grid_w"],
            "grid_height": item["grid_h"],
            "weight": item["base_weight"],
            "rarity": item["rarity"],
            "description": item.get("description", ""),
            "stack_count": 1,
            "spirit_quality": item["spirit_quality_initial"],
            "durability": 1.0,
        })
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(config, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(args.output)


if __name__ == "__main__":
    main()
