#!/usr/bin/env python3
"""一条命令生成全部交付物到模组仓：

    python3 -m modelScript.player_rig.generate --mod-repo ~/Code/Kizuna-Players-Animation

写入（相对模组仓）：
    src/main/resources/assets/kizuna_players_animation/geo/player_skeleton.geo.json
    src/main/resources/assets/kizuna_players_animation/animations/calib_*.animation.json
    src/main/resources/assets/kizuna_players_animation/player_anim_index.json
    models/player_skeleton_{classic,slim}.bbmodel、models/player_debug_skin.png
"""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

from . import anim, bbmodel, skin

NAMESPACE = "kizuna_players_animation"


def _dump(document, path: Path) -> None:
    """缩进输出，但数值数组保持单行（便于审阅 diff）。"""
    text = json.dumps(document, indent=2, ensure_ascii=False)
    text = re.sub(r"\[\s+(-?[\d.eE+\-]+(?:,\s+-?[\d.eE+\-]+)*)\s+\]",
                  lambda m: "[" + re.sub(r"\s+", " ", m.group(1)) + "]", text)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text + "\n", encoding="utf-8")


def generate(mod_repo: Path) -> None:
    assets = mod_repo / "src/main/resources/assets" / NAMESPACE
    models = mod_repo / "models"
    models.mkdir(parents=True, exist_ok=True)
    skin.build().save(models / "player_debug_skin.png")
    paths = {}
    for variant in ("classic", "slim"):
        paths[variant] = models / f"player_skeleton_{variant}.bbmodel"
        paths[variant].write_text(json.dumps(bbmodel.build(variant), ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    geometry = bbmodel.export_geometry(paths["classic"], paths["slim"])
    _dump(geometry, assets / "geo/player_skeleton.geo.json")
    for name, (build, *_rest) in anim.ANIMATIONS.items():
        _dump(build(), assets / f"animations/{name}.animation.json")
    _dump(anim.index(), assets / "player_anim_index.json")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    parser.add_argument("--mod-repo", type=Path, required=True)
    generate(parser.parse_args().mod_repo.expanduser())


if __name__ == "__main__":
    main()
