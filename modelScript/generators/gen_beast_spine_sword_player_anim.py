#!/usr/bin/env python3
"""BeastSpineSwordPlayerAnim.bbmodel —— 玩家 + 异兽脊骨剑 (BeastSpineSword) + 内嵌手持与劈砍/横扫动画。

将玩家模型与异兽脊骨剑绑定，并烘入剑法核心动作：
- `sword_cleave` (双手重劈)
- `sword_swing_horiz` (横扫千军)
- `sword_thrust` (破甲穿刺)
- `sword_parry` (架剑招架)
- `sword_infuse` (引煞封灵蓄力)

在 Blockbench Animate 模式中可实时预览、拖拽调姿。
"""

from __future__ import annotations

import argparse
import base64
import copy
import io
import json
import math
import sys
import uuid
from pathlib import Path

import numpy as np
from PIL import Image

LIB = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(LIB / "core"))
sys.path.insert(0, str(LIB / "tools"))
sys.path.insert(0, str(LIB / "generators"))

from bbmodel_maker.rig import bb_anim_axes as AX
from bbmodel_maker.render import held_item_render as H
from bbmodel_maker.render import render_player_pose as P
from gen_jian_player_anim import (
    PART_GROUPS,
    TICKS_PER_SECOND,
    _uuid,
    cube_element,
    group,
    keyframe,
    split_cubes,
)

REPO = LIB.parent
ANIM_DIR = REPO / "client" / "src" / "main" / "resources" / "assets" / "bong" / "player_animation"
SRC_SWORD = LIB / "models" / "BeastSpineSword.bbmodel"
OUT_BB = LIB / "models" / "BeastSpineSwordPlayerAnim.bbmodel"

DEFAULT_ANIMS = [
    "sword_spine_slash",
    "sword_cleave",
    "sword_swing_horiz",
    "sword_thrust",
    "sword_parry",
    "sword_infuse",
    "lower_walk",
    "lower_sprint",
]

# 异兽脊骨剑握把点 (约在 y=3.4px 处，中心对准手心)
SWORD_GRIP_PX = np.array([0.0, 3.4, 0.0])
SIDE = "right"


def _png_data_url(img: Image.Image) -> str:
    buf = io.BytesIO()
    img.save(buf, format="PNG")
    return "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode()



def load_sword():
    """读取 BeastSpineSword.bbmodel，返回 (elements, 贴图, 贴图尺寸)。"""
    doc = json.loads(SRC_SWORD.read_text(encoding="utf-8"))
    tex = doc["textures"][0]
    image = Image.open(
        io.BytesIO(base64.b64decode(tex["source"].split(",", 1)[1]))
    ).convert("RGBA")
    w = int(tex.get("width", doc.get("resolution", {}).get("width", 64)))
    h = int(tex.get("height", doc.get("resolution", {}).get("height", 64)))
    return doc["elements"], image, (w, h)


def build_geometry():
    """构建玩家与异兽脊骨剑几何及图集。"""
    sword_elements, sword_tex, (sword_w, sword_h) = load_sword()
    atlas = Image.new("RGBA", (H.ATLAS, H.ATLAS), (0, 0, 0, 0))
    atlas.paste(H.make_skin(), (0, 0))
    # 剑贴图拼入图集下半区
    atlas.paste(sword_tex, (0, H.WEAPON_V_OFF))

    elements: list[dict] = []
    gmap: dict[str, str] = {}

    def add_part(part_name):
        spec = P.PARTS[part_name]
        prefix, has_bend = PART_GROUPS[part_name]
        upper_ids, lower_ids = [], []
        for seg_name, frm, to, uv, is_lower in split_cubes(part_name):
            el = cube_element(seg_name, frm, to, uv)
            elements.append(el)
            (lower_ids if is_lower else upper_ids).append(el["uuid"])
        bend_group = None
        if has_bend:
            bend_group = group(
                f"{prefix}_bend", spec["bend_center"], lower_ids, color=6
            )
            gmap[f"{prefix}_bend"] = bend_group["uuid"]
        node = list(upper_ids) + ([bend_group] if bend_group else [])
        for axis_name in ("pitch", "yaw", "roll"):
            g = group(f"{prefix}_{axis_name}", spec["pivot"], node, color=7)
            gmap[f"{prefix}_{axis_name}"] = g["uuid"]
            node = [g]
        return node[0], bend_group

    head, _ = add_part("head")
    torso, _ = add_part("torso")
    legs = [add_part("rightLeg")[0], add_part("leftLeg")[0]]

    arms = []
    for part_name, side in (("rightArm", "right"), ("leftArm", "left")):
        top, bend_group = add_part(part_name)
        if side == SIDE:
            hand = H.HAND_REST[side]
            offset = np.array(hand, float) - SWORD_GRIP_PX
            sword_ids = []
            for source in sword_elements:
                el = copy.deepcopy(source)
                el["uuid"] = _uuid()
                el["name"] = f"sword_{source['name']}"
                for key in ("from", "to", "origin"):
                    el[key] = [
                        round(v + offset[i], 4) for i, v in enumerate(el[key])
                    ]
                for face in el["faces"].values():
                    u1, v1, u2, v2 = face["uv"]
                    face["uv"] = [
                        u1,
                        v1 + H.WEAPON_V_OFF,
                        u2,
                        v2 + H.WEAPON_V_OFF,
                    ]
                elements.append(el)
                sword_ids.append(el["uuid"])

            # 绕 X 翻转 180° 让剑尖顺着小臂方向延伸
            pitch_g = group(
                f"sword_{side}_pitch", hand, sword_ids, (180.0, 0.0, 0.0), color=1
            )
            roll_g = group(
                f"sword_{side}_roll", hand, [pitch_g], (0.0, 0.0, 0.0), color=1
            )
            gmap[f"sword_{side}_pitch"] = pitch_g["uuid"]
            gmap[f"sword_{side}_roll"] = roll_g["uuid"]
            (bend_group or top)["children"].append(roll_g)
        arms.append(top)

    node = [head, torso] + arms + legs
    for axis_name in ("pitch", "yaw", "roll"):
        g = group(f"root_{axis_name}", (0.0, 0.0, 0.0), node, color=3)
        gmap[f"root_{axis_name}"] = g["uuid"]
        node = [g]
    root_pos = group("root_pos", (0.0, 0.0, 0.0), node, color=3)
    gmap["root_pos"] = root_pos["uuid"]
    return elements, [root_pos], gmap, atlas


def bake_emotecraft_anim(name: str, doc: dict, gmap: dict[str, str]) -> dict:
    """把 emotecraft v3 JSON 转换为 Blockbench animation 格式。"""
    moves = doc.get("moves", [])
    if not moves:
        return {"name": name, "length": 0.0, "animators": {}}

    end_tick = max(m.get("tick", 0) for m in moves)
    anim_len = end_tick / TICKS_PER_SECOND

    animators: dict[str, dict] = {}

    def get_animator(bone_uuid: str) -> dict:
        if bone_uuid not in animators:
            animators[bone_uuid] = {
                "name": bone_uuid,
                "type": "bone",
                "rotation": [],
                "position": [],
            }
        return animators[bone_uuid]

    for m in moves:
        t_sec = m.get("tick", 0) / TICKS_PER_SECOND

        # 1. body translation
        body = m.get("body", {})
        if "x" in body or "y" in body or "z" in body:
            pos_x = body.get("x", 0.0) * 16.0
            pos_y = -body.get("y", 0.0) * 16.0
            pos_z = body.get("z", 0.0) * 16.0
            anim = get_animator(gmap["root_pos"])
            anim["position"].append(keyframe(t_sec, [pos_x, pos_y, pos_z]))

        # 2. body rotation
        if "yaw" in body or "pitch" in body or "roll" in body:
            byaw = body.get("yaw", 0.0)
            bpitch = body.get("pitch", 0.0)
            broll = body.get("roll", 0.0)
            if "root_pitch" in gmap and bpitch != 0.0:
                get_animator(gmap["root_pitch"])["rotation"].append(
                    keyframe(t_sec, [-bpitch, 0, 0])
                )
            if "root_yaw" in gmap and byaw != 0.0:
                get_animator(gmap["root_yaw"])["rotation"].append(
                    keyframe(t_sec, [0, byaw, 0])
                )
            if "root_roll" in gmap and broll != 0.0:
                get_animator(gmap["root_roll"])["rotation"].append(
                    keyframe(t_sec, [0, 0, -broll])
                )

        # 3. 各部位单轴与 bend
        for part_name, (prefix, has_bend) in PART_GROUPS.items():
            pdata = m.get(part_name, {})
            if not pdata:
                continue

            pitch = pdata.get("pitch", 0.0)
            yaw = pdata.get("yaw", 0.0)
            roll = pdata.get("roll", 0.0)

            # 单轴旋转
            if f"{prefix}_pitch" in gmap:
                get_animator(gmap[f"{prefix}_pitch"])["rotation"].append(
                    keyframe(t_sec, [-pitch, 0, 0])
                )
            if f"{prefix}_yaw" in gmap:
                get_animator(gmap[f"{prefix}_yaw"])["rotation"].append(
                    keyframe(t_sec, [0, yaw, 0])
                )
            if f"{prefix}_roll" in gmap:
                get_animator(gmap[f"{prefix}_roll"])["rotation"].append(
                    keyframe(t_sec, [0, 0, -roll])
                )

            # bend 折弯层
            if has_bend and "bend" in pdata and f"{prefix}_bend" in gmap:
                bend_val = pdata.get("bend", 0.0)
                axis_val = pdata.get("axis", 0.0)
                rot_bend = AX.bend_to_bb(bend_val, axis_val)
                get_animator(gmap[f"{prefix}_bend"])["rotation"].append(
                    keyframe(t_sec, rot_bend)
                )

    return {
        "name": name,
        "uuid": _uuid(),
        "loop": "once",
        "length": round(anim_len, 4),
        "animators": animators,
    }


def build_bbmodel(anim_names: list[str] = DEFAULT_ANIMS) -> dict:
    elements, outliner, gmap, atlas = build_geometry()

    animations = []
    for aname in anim_names:
        afile = ANIM_DIR / f"{aname}.json"
        if afile.exists():
            try:
                doc = json.loads(afile.read_text(encoding="utf-8"))
                anim_obj = bake_emotecraft_anim(aname, doc, gmap)
                animations.append(anim_obj)
            except Exception as e:
                print(f"  ⚠ 烘焙动画 {aname} 失败: {e}")
        else:
            print(f"  ⚠ 动画文件不存在: {afile}")

    tex_uuid = _uuid()
    return {
        "meta": {
            "format_version": "4.10",
            "model_format": "free",
            "box_uv": False,
        },
        "name": "BeastSpineSwordPlayerAnim",
        "model_identifier": "beast_spine_sword_player_anim",
        "visible_box": [1, 1, 0],
        "resolution": {"width": H.ATLAS, "height": H.ATLAS},
        "elements": elements,
        "outliner": outliner,
        "textures": [
            {
                "path": "player_sword_atlas.png",
                "name": "player_sword_atlas",
                "folder": "item",
                "namespace": "bong",
                "id": "0",
                "particle": False,
                "render_mode": "default",
                "visible": True,
                "mode": "bitmap",
                "saved": True,
                "uuid": tex_uuid,
                "source": _png_data_url(atlas),
            }
        ],
        "animations": animations,
    }


def main():
    parser = argparse.ArgumentParser(
        description="生成玩家持异兽脊骨剑及动画预览 .bbmodel"
    )
    parser.add_argument(
        "--anims", nargs="+", default=DEFAULT_ANIMS, help="内嵌的动画列表"
    )
    parser.add_argument(
        "--out", type=Path, default=OUT_BB, help="输出 .bbmodel 路径"
    )
    args = parser.parse_args()

    args.out.parent.mkdir(parents=True, exist_ok=True)
    bb_data = build_bbmodel(args.anims)
    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(bb_data, f, indent=2, ensure_ascii=False)

    print(f"✓ 成功生成玩家持剑动画模型: {args.out}")
    print(
        f"  包含 {len(bb_data['elements'])} 个立方体，{len(bb_data['animations'])} 条内嵌动画。"
    )


if __name__ == "__main__":
    main()
