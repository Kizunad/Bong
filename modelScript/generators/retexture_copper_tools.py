#!/usr/bin/env python3
"""铜斧 / 铜镐的统一重贴图入口：出 <id>.bbmodel（平色）、<id>_tex.bbmodel（贴图）和 check_tex.png。

几何不动（`gen_axe_copper.all_cubes` / `gen_pickaxe_copper.all_cubes`，px 立方体）；这里只做两件事：
1. 把它们自己的材质名换成画笔库（`handheld_texture.py`）里的材质名，见 MATERIAL_MAP；
2. 平色 / 贴图两个版本各写一份，拼成 check_tex.png：左参考 / 中平色 / 右贴图，另加斧头 / 镐头特写。

    python3 modelScript/generators/retexture_copper_tools.py axe_copper pickaxe_copper
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from typing import Callable, Dict, List

import gen_axe_copper
import gen_pickaxe_copper
import handheld_common as hc
from handheld_common import Cube, View

# 本件自己的材质名 → 画笔库材质名
MATERIAL_MAP: Dict[str, str] = {
    "copper_weathered": "copper_green",   # 铜绿斑驳的斧身 / 镐臂，局部露出铜色
    "copper_edge": "copper_edge",         # 刃口、锤背、铜环套的亮铜色
    "hemp_binding": "rope",
    "aged_wood": "wood",
}

WHOLE_VIEW: View = ("3/4 (yaw-35 pitch25)", -35.0, 25.0)
HEAD_SPAN = 22.0                          # 头部特写的取景跨度（px）


@dataclass
class Piece:
    cubes: Callable[[], List[Cube]]
    head_center: tuple                    # 头部特写的中心（px）


PIECES: Dict[str, Piece] = {
    "axe_copper": Piece(gen_axe_copper.all_cubes, (5.0, 22.0, 8.0)),
    "pickaxe_copper": Piece(gen_pickaxe_copper.all_cubes, (8.0, 19.0, 8.0)),
}


def mapped_cubes(piece: Piece) -> List[Cube]:
    return [dict(cube, material=MATERIAL_MAP[cube["material"]]) for cube in piece.cubes()]


def views(piece: Piece) -> List[View]:
    """整件 3/4 + 头部特写（3/4）+ 头部正对刃面（yaw 0：刃面所在的 x–y 平面）。"""
    return [
        WHOLE_VIEW,
        ("HEAD CLOSE-UP (yaw-35 pitch25)", -35.0, 25.0, (piece.head_center, HEAD_SPAN)),
        ("HEAD FRONT (yaw0 pitch0)", 0.0, 0.0, (piece.head_center, HEAD_SPAN)),
    ]


def retexture(piece_id: str) -> None:
    piece = PIECES[piece_id]
    cubes = mapped_cubes(piece)
    flat = hc.write_model(cubes, piece_id, check=False)     # 几何由各自生成器的门禁把关
    tex = hc.write_model_tex(cubes, piece_id)
    print(f"✓ {piece_id}: {tex.name} → {hc.write_check_tex(flat, tex, piece_id, views(piece))}")


def main() -> None:
    parser = argparse.ArgumentParser(description="铜斧 / 铜镐统一重贴图")
    parser.add_argument("ids", nargs="*", help="件的 id")
    args = parser.parse_args()
    for piece_id in args.ids or list(PIECES):
        retexture(piece_id)


if __name__ == "__main__":
    main()
