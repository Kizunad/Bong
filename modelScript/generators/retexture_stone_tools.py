#!/usr/bin/env python3
"""石斧 / 石镐 / 灵木剑的统一重贴图入口：出 <id>.bbmodel（平色）、<id>_tex.bbmodel（贴图）和 check_tex.png。

三件的几何都是 `HeldItem`（`held_item_common.Box`，单位 = 方块，授权系）：
`held_cubes()` 换成 px 立方体（和 `held_item_common.build_bbmodel` 同一个 `emit_offset` 平移），
再按 MATERIAL_MAP 把它们自己的材质名换成画笔库（`handheld_texture.py`）里的材质名。
几何不动，生成器自己的门禁照常把关。

    python3 modelScript/generators/retexture_stone_tools.py stone_axe stone_pickaxe lingmu_sword
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from typing import Callable, Dict, List

import gen_lingmu_sword
import gen_stone_axe
import gen_stone_pickaxe
import handheld_common as hc
from bbmodel_maker.render.held_item_common import emit_offset
from handheld_common import Cube, View

# 本件自己的材质名 → 画笔库材质名
MATERIAL_MAP: Dict[str, str] = {
    "weathered_wood": "wood",
    "rough_stone": "stone_chipped",       # 画笔库里没有深色玄武岩 / 燧石，粗打石最接近
    "stone_edge": "stone_chipped",        # 同上，刃口没有单独的亮一档
    "hemp_binding": "rope",
    "spirit_wood": "wood",                # 画笔库里没有浅色灵木
    "wood_edge": "wood",
    "spirit_vein": "copper_green",        # 画笔库里没有绿色灵纹，铜绿最接近
    "fine_cord": "rope",
}

WHOLE_VIEW: View = ("3/4 (yaw-35 pitch25)", -35.0, 25.0)
CLOSEUP_SPAN = 12.0


def held_cubes(item) -> List[Cube]:
    """HeldItem（Box，授权系 = 方块）→ px 立方体。"""
    off = emit_offset(item)
    return [
        hc.cube(box.name, [(v + off[i]) * 16.0 for i, v in enumerate(box.low)],
                [(v + off[i]) * 16.0 for i, v in enumerate(box.high)],
                MATERIAL_MAP.get(box.material, box.material), box.material)
        for box in item.boxes
    ]


def head_views(center: tuple, span: float) -> Callable[[List[Cube]], List[View]]:
    """斧头 / 镐头：整件 3/4 + 头部特写（3/4）+ 头部正对（yaw 0，x–y 平面）。"""
    return lambda cubes: [
        WHOLE_VIEW,
        ("HEAD CLOSE-UP (yaw-35 pitch25)", -35.0, 25.0, (center, span)),
        ("HEAD FRONT (yaw0 pitch0)", 0.0, 0.0, (center, span)),
    ]


def long_item_views(cubes: List[Cube]) -> List[View]:
    """长条形（剑）：整件 3/4 + 剑尖特写 + 握把端特写。"""
    top = max(c["to"][1] for c in cubes)
    bottom = min(c["from"][1] for c in cubes)
    cx = sum((c["from"][0] + c["to"][0]) / 2 for c in cubes) / len(cubes)
    cz = sum((c["from"][2] + c["to"][2]) / 2 for c in cubes) / len(cubes)
    return [
        WHOLE_VIEW,
        ("TIP CLOSE-UP (yaw-35 pitch25)", -35.0, 25.0, ((cx, top - CLOSEUP_SPAN / 2.5, cz), CLOSEUP_SPAN)),
        ("GRIP CLOSE-UP (yaw-35 pitch25)", -35.0, 25.0, ((cx, bottom + CLOSEUP_SPAN / 2.5, cz), CLOSEUP_SPAN)),
    ]


@dataclass
class Piece:
    item: Callable[[], object]
    views: Callable[[List[Cube]], List[View]]


PIECES: Dict[str, Piece] = {
    "stone_axe": Piece(gen_stone_axe.stone_axe, head_views((7.3, 16.3, 8.0), 11.0)),
    "stone_pickaxe": Piece(gen_stone_pickaxe.stone_pickaxe, head_views((8.0, 16.3, 8.0), 14.0)),
    "lingmu_sword": Piece(gen_lingmu_sword.lingmu_sword, long_item_views),
}


def retexture(piece_id: str) -> None:
    piece = PIECES[piece_id]
    cubes = held_cubes(piece.item())
    flat = hc.write_model(cubes, piece_id, check=False)     # HeldItem 的几何由各自生成器的门禁把关
    tex = hc.write_model_tex(cubes, piece_id)
    print(f"✓ {piece_id}: {tex.name} → {hc.write_check_tex(flat, tex, piece_id, piece.views(cubes))}")


def main() -> None:
    parser = argparse.ArgumentParser(description="石斧 / 石镐 / 灵木剑统一重贴图")
    parser.add_argument("ids", nargs="*", help="件的 id")
    args = parser.parse_args()
    for piece_id in args.ids or list(PIECES):
        retexture(piece_id)


if __name__ == "__main__":
    main()
