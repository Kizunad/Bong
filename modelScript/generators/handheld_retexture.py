#!/usr/bin/env python3
"""手持物 / 器物的统一重贴图入口：按 id 出 <id>.bbmodel（平色）、<id>_tex.bbmodel（贴图）和 check_tex.png。

check_tex.png = 左参考（model-review/img/handheld/exploded/<id>.png）/ 中平色 / 右贴图；长条形的件再加一对剑头 / 矛头特写。
画笔库和材质表见 handheld_texture.py；新增一件只要在 REGISTRY 里登记「几何函数 + 视角」。

两类几何来源：
- 本文件夹里用 handheld_common.cube 写的立方体（px 坐标）：守家傀儡、阵符、灵龛诡雷；
- `HeldItem`（held_item_common 的 Box，单位 = 方块，授权系）：矛族、灵锋剑——用 held_cubes() 换成 px 立方体，
  并按 MATERIAL_MAP 把它们的材质名换成画笔库里的材质名。

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
LONG_VIEW: View = ("3/4 (yaw-35 pitch25)", -35.0, 25.0)
CLOSEUP_SPAN = 12.0             # 特写取景跨度（px）

# 授权系 HeldItem 的材质名 → 画笔库材质名（没列出的原样用，要求画笔库里有同名）
MATERIAL_MAP: Dict[str, str] = {
    "hemp": "rope", "stone": "stone_chipped", "bone": "bone_main", "rust": "rust_heavy", "rust_dark": "rust_iron",
    "grip": "wood_dark", "cord": "rope", "glow": "glow_edge",
    # 工具五件（刨锄 / 刮刀 / 骨骸钳 / 钝气夹 / 阵旗）
    "hoe_wood": "wood", "scr_wood": "wood", "flag_wood": "wood",
    "hoe_iron": "rust_iron", "scr_iron": "rust_iron", "clamp_iron": "rust_iron", "tong_iron": "rust_iron",
    "hoe_edge": "steel", "scr_edge": "steel", "clamp_edge": "steel", "tong_edge": "steel",
    "hoe_cord": "rope", "scr_cord": "rope", "clamp_cord": "rope", "tong_cord": "rope",
    "tong_bone": "bone_main", "flag_bone": "bone_main", "clamp_pad": "leather", "flag_leather": "leather",
    "flag_cloth": "cloth_tan", "flag_rune": "glow_edge",
}


def held_cubes(item) -> List[Cube]:
    """HeldItem（Box，授权系 = 方块）→ px 立方体：和 held_item_common.build_bbmodel 同一个平移（emit_offset）。"""
    from bbmodel_maker.render.held_item_common import emit_offset

    off = emit_offset(item)
    cubes = []
    for box in item.boxes:
        material = MATERIAL_MAP.get(box.material, box.material)
        cubes.append(hc.cube(box.name, [(v + off[i]) * 16.0 for i, v in enumerate(box.low)],
                             [(v + off[i]) * 16.0 for i, v in enumerate(box.high)], material, box.material))
    return cubes


def long_item_views(cubes: List[Cube]) -> List[View]:
    """长条形（矛 / 剑）：整件 3/4 + 顶端特写（矛头 / 剑尖）+ 握把端特写。"""
    top = max(c["to"][1] for c in cubes)
    bottom = min(c["from"][1] for c in cubes)
    cx = sum((c["from"][0] + c["to"][0]) / 2 for c in cubes) / len(cubes)
    cz = sum((c["from"][2] + c["to"][2]) / 2 for c in cubes) / len(cubes)
    return [
        LONG_VIEW,
        ("TIP CLOSE-UP (yaw-35 pitch25)", -35.0, 25.0, ((cx, top - CLOSEUP_SPAN / 2.5, cz), CLOSEUP_SPAN)),
        ("GRIP CLOSE-UP (yaw-35 pitch25)", -35.0, 25.0, ((cx, bottom + CLOSEUP_SPAN / 2.5, cz), CLOSEUP_SPAN)),
    ]


@dataclass
class Piece:
    cubes: Callable[[], List[Cube]]
    views: Callable[[List[Cube]], List[View]]
    check_geometry: bool = True     # HeldItem 的几何由它们自己的生成器门禁把关，这里不再重复共面检查


def _fixed(view: View) -> Callable[[List[Cube]], List[View]]:
    return lambda cubes: [view]


def _spear_cubes(key: str) -> Callable[[], List[Cube]]:
    def build() -> List[Cube]:
        import gen_spear_family

        return held_cubes(next(item for item in gen_spear_family.items() if item.key == key))

    return build


def _sword_cubes() -> List[Cube]:
    import gen_ling_feng_sword

    return held_cubes(gen_ling_feng_sword.item())


REGISTRY: Dict[str, Piece] = {
    "niche_house_puppet": Piece(gen_niche_house_puppet.all_cubes, _fixed(PUPPET_VIEW)),
    "warning_trap": Piece(gen_warning_trap.all_cubes, _fixed(GROUND_VIEW)),
    "slow_trap": Piece(gen_slow_trap.all_cubes, _fixed(GROUND_VIEW)),
    "blast_trap": Piece(gen_blast_trap.all_cubes, _fixed(GROUND_VIEW)),
    **{gen_niche_zhenfa_trap.model_id(tier): Piece(builder, _fixed(GROUND_VIEW))
       for tier, builder in gen_niche_zhenfa_trap.BUILDERS.items()},
    "crude_stone_spear": Piece(_spear_cubes("crude_stone_spear"), long_item_views, check_geometry=False),
    "bone_tip_spear": Piece(_spear_cubes("bone_tip_spear"), long_item_views, check_geometry=False),
    "rusted_iron_spear": Piece(_spear_cubes("rusted_iron_spear"), long_item_views, check_geometry=False),
    "ling_feng_sword": Piece(_sword_cubes, long_item_views, check_geometry=False),
}


def _held_piece(module_name: str) -> Callable[[], List[Cube]]:
    """单件 HeldItem 模块（模块里 items() 只有一件）的几何。"""
    def build() -> List[Cube]:
        import importlib

        return held_cubes(importlib.import_module(module_name).items()[0])

    return build


REGISTRY.update({
    "bao_chu": Piece(_held_piece("gen_bao_chu"), long_item_views, check_geometry=False),
    "gua_dao": Piece(_held_piece("gen_gua_dao"), long_item_views, check_geometry=False),
    "gu_hai_qian": Piece(_held_piece("gen_gu_hai_qian"), long_item_views, check_geometry=False),
    "dun_qi_jia": Piece(_held_piece("gen_dun_qi_jia"), long_item_views, check_geometry=False),
    "array_flag": Piece(_held_piece("gen_array_flag_held"), long_item_views, check_geometry=False),
})


def retexture(piece_id: str) -> None:
    piece = REGISTRY[piece_id]
    cubes = piece.cubes()
    flat = hc.write_model(cubes, piece_id, check=piece.check_geometry)
    tex = hc.write_model_tex(cubes, piece_id)
    print(f"✓ {piece_id}: {tex.name} → {hc.write_check_tex(flat, tex, piece_id, piece.views(cubes))}")


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
