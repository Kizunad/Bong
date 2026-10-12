#!/usr/bin/env python3
"""手持物 / 器物的统一重贴图入口：按 id 出 <id>.bbmodel（平色）、<id>_tex.bbmodel（贴图）和 check_tex.png。

check_tex.png = 左参考（model-review/img/handheld/exploded/<id>.png）/ 中平色 / 右贴图，同一个视角。
画笔库和材质表见 handheld_texture.py；新增一件只要在 REGISTRY 里登记「几何函数 + 视角」。

    python3 modelScript/generators/handheld_retexture.py --list
    python3 modelScript/generators/handheld_retexture.py niche_house_puppet warning_trap
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from typing import Callable, Dict, List

import gen_blast_trap
import gen_niche_house_puppet
import gen_niche_zhenfa_trap
import gen_slow_trap
import gen_warning_trap
import handheld_common as hc
from handheld_common import Cube, View

PUPPET_VIEW: View = ("3/4 FRONT-RIGHT (yaw35 pitch20)", 35.0, 20.0)
GROUND_VIEW: View = ("3/4 TOP-DOWN FRONT-RIGHT (yaw35 pitch50)", 35.0, 50.0)


@dataclass
class Piece:
    cubes: Callable[[], List[Cube]]
    view: View


REGISTRY: Dict[str, Piece] = {
    "niche_house_puppet": Piece(gen_niche_house_puppet.all_cubes, PUPPET_VIEW),
    "warning_trap": Piece(gen_warning_trap.all_cubes, GROUND_VIEW),
    "slow_trap": Piece(gen_slow_trap.all_cubes, GROUND_VIEW),
    "blast_trap": Piece(gen_blast_trap.all_cubes, GROUND_VIEW),
    **{gen_niche_zhenfa_trap.model_id(tier): Piece(builder, GROUND_VIEW)
       for tier, builder in gen_niche_zhenfa_trap.BUILDERS.items()},
}


def retexture(piece_id: str) -> None:
    piece = REGISTRY[piece_id]
    cubes = piece.cubes()
    flat = hc.write_model(cubes, piece_id)
    tex = hc.write_model_tex(cubes, piece_id)
    print(f"✓ {piece_id}: {tex.name} → {hc.write_check_tex(flat, tex, piece_id, piece.view)}")


def main() -> None:
    parser = argparse.ArgumentParser(description="手持物统一重贴图")
    parser.add_argument("ids", nargs="*", help="件的 id")
    parser.add_argument("--list", action="store_true")
    args = parser.parse_args()
    if args.list or not args.ids:
        print("可用 id:", " ".join(REGISTRY))
        return
    for piece_id in args.ids:
        retexture(piece_id)


if __name__ == "__main__":
    main()
