#!/usr/bin/env python3
"""BronzeSaberPlayerAnim.bbmodel —— 玩家 + 青铜单刀 + 内嵌 Blockbench 动画。

骨架与 gen_beast_spine_sword_player_anim.py / gen_jian_player_anim.py 同构：
- 挂右手持青铜单刀，并将专属刀法动画烘培进 Blockbench：
    1. saber_slash_down   - 青铜单刀单手顺步下劈斩
    2. saber_swing_horiz  - 青铜单刀大角度破风平抹斩
    3. sword_parry        - 单刀格挡架势
    4. sword_infuse       - 注气蓄力架势
    5. lower_walk / lower_sprint - 基础步态

用法:
    python3 modelScript/generators/gen_bronze_saber_player_anim.py
"""

from __future__ import annotations

import argparse
import base64
import copy
import io
import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image

LIB = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(LIB / "tools"))
sys.path.insert(0, str(LIB / "generators"))

from bbmodel_maker.render import held_item_render as H  # noqa: E402
from bbmodel_maker.render import render_player_pose as P  # noqa: E402
from bbmodel_maker.rig import bb_anim_axes as AX  # noqa: E402
from gen_club_player_anim import convert_animation as bake_animation  # noqa: E402
from gen_jian_player_anim import (  # noqa: E402
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
SRC_SABER = LIB / "models" / "BronzeSaber.bbmodel"
OUT_BB = LIB / "models" / "BronzeSaberPlayerAnim.bbmodel"

DEFAULT_ANIMS = [
    "saber_slash_down",
    "saber_swing_horiz",
    "sword_parry",
    "sword_infuse",
    "lower_walk",
    "lower_sprint",
]

SABER_GRIP_PX = np.array([8.0, 8.0, 8.0])
SIDE = "right"


def load_saber():
    """读刀 bbmodel，如果不存在则现场由 gen_bronze_saber 生成"""
    if not SRC_SABER.exists():
        import gen_bronze_saber as GBS
        cubes = GBS.build_cubes()
        tex = GBS.make_texture_atlas()
        bb = GBS.build_bbmodel(cubes, tex)
        SRC_SABER.parent.mkdir(parents=True, exist_ok=True)
        SRC_SABER.write_text(json.dumps(bb, indent=2, ensure_ascii=False), encoding="utf-8")

    doc = json.loads(SRC_SABER.read_text(encoding="utf-8"))
    tex = doc["textures"][0]
    image = Image.open(
        io.BytesIO(base64.b64decode(tex["source"].split(",", 1)[1]))
    ).convert("RGBA")
    res = doc.get("resolution", {"width": 64, "height": 64})
    return doc["elements"], image, (int(res["width"]), int(res["height"]))


def build_geometry():
    """→ (elements, outliner, group_uuid_by_name, atlas)"""
    saber_elements, saber_tex, (saber_w, saber_h) = load_saber()
    if saber_w > H.ATLAS or saber_h + H.WEAPON_V_OFF > H.ATLAS:
        raise SystemExit(f"刀贴图 {saber_w}×{saber_h} 放不进 {H.ATLAS}² 图集的下半")
    atlas = Image.new("RGBA", (H.ATLAS, H.ATLAS), (0, 0, 0, 0))
    atlas.paste(H.make_skin(), (0, 0))
    atlas.paste(saber_tex, (0, H.WEAPON_V_OFF))

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
            bend_group = group(f"{prefix}_bend", spec["bend_center"], lower_ids, color=6)
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
            offset = np.array(hand, float) - SABER_GRIP_PX
            saber_ids = []
            for source in saber_elements:
                el = copy.deepcopy(source)
                el["uuid"] = _uuid()
                el["name"] = f"saber_{source['name']}"
                for key in ("from", "to", "origin"):
                    if key in el:
                        el[key] = [round(v + offset[i], 4) for i, v in enumerate(el[key])]
                for face in el["faces"].values():
                    u1, v1, u2, v2 = face["uv"]
                    face["uv"] = [u1, v1 + H.WEAPON_V_OFF, u2, v2 + H.WEAPON_V_OFF]
                elements.append(el)
                saber_ids.append(el["uuid"])

            # 挂接刀身（垂直于小臂）
            pitch_g = group(
                f"saber_{side}_pitch", hand, saber_ids, (-90.0, 0.0, 0.0), color=1
            )
            roll_g = group(
                f"saber_{side}_roll", hand, [pitch_g], (0.0, 0.0, 90.0), color=1
            )
            gmap[f"saber_{side}_pitch"] = pitch_g["uuid"]
            gmap[f"saber_{side}_roll"] = roll_g["uuid"]
            (bend_group or top)["children"].append(roll_g)
        arms.append(top)

    # body：位移一层 + 三层单轴旋转
    node = [head, torso] + arms + legs
    for axis_name in ("pitch", "yaw", "roll"):
        g = group(f"root_{axis_name}", (0.0, 0.0, 0.0), node, color=3)
        gmap[f"root_{axis_name}"] = g["uuid"]
        node = [g]
    root_pos = group("root_pos", (0.0, 0.0, 0.0), node, color=3)
    gmap["root_pos"] = root_pos["uuid"]
    return elements, [root_pos], gmap, atlas


_STANCE_UPPER = dict(torso=dict(pitch=+2, yaw=+14), head=dict(pitch=-2, yaw=-6, roll=+0.7))
STANCE_POSES = {
    "lower_walk": dict(
        rightArm=dict(pitch=-45.0, yaw=-15.0, roll=+10.0, bend=25.0, axis=180),
        leftArm=dict(pitch=+20.0, yaw=+20.0, roll=-15.0, bend=20.0, axis=180),
        **_STANCE_UPPER,
    ),
    "lower_sprint": dict(
        rightArm=dict(pitch=-35.0, yaw=-25.0, roll=+15.0, bend=35.0, axis=180),
        leftArm=dict(pitch=+25.0, yaw=+25.0, roll=-20.0, bend=25.0, axis=180),
        **_STANCE_UPPER,
    ),
}
DEFAULT_STANCE = STANCE_POSES["lower_walk"]
UPPER_PARTS = tuple(DEFAULT_STANCE)


def _has_upper_body(json_path: Path) -> bool:
    _name, _emote, table = P.anim_pose_table(json_path)
    return any(part in UPPER_PARTS for _tick, pose in table for part in pose)


def _fill_upper_body(anim: dict, gmap: dict, stance: dict) -> None:
    """把恒定的持刀架势写进上半身轨道。只补预览，emotecraft 源文件不动。"""
    animators = anim["animators"]

    def track(group_name):
        gid = gmap[group_name]
        animators.setdefault(gid, {"name": group_name, "type": "bone", "keyframes": []})
        return animators[gid]["keyframes"]

    for t in (0.0, anim["length"]):
        for part, axes in stance.items():
            prefix, has_bend = PART_GROUPS[part]
            for axis_name in AX.AXIS_ORDER:
                track(f"{prefix}_{axis_name}").append(
                    keyframe("rotation", t, AX.rotation_to_bb(axes, axis_name)))
            if has_bend:
                bend = AX.bend_to_bb(axes.get("bend", 0.0), axes.get("axis", 0.0))
                track(f"{prefix}_bend").append(
                    keyframe("rotation", t, [round(bend, 4), 0.0, 0.0]))


def convert_animation(json_path: Path, gmap: dict) -> dict:
    """emotecraft v3 → Blockbench animation，纯下半身的补一份持刀架势上半身。"""
    anim = bake_animation(json_path, gmap)
    if not _has_upper_body(json_path):
        stance = STANCE_POSES.get(anim["name"], DEFAULT_STANCE)
        _fill_upper_body(anim, gmap, stance)
        which = "冲刺横持" if stance is STANCE_POSES["lower_sprint"] else "持刀携行"
        print(f"    （{anim['name']}: 补了{which}架势上半身轨道供预览，源 JSON 未改）")
    return anim


def build_bbmodel(anim_names: list[str]) -> dict:
    elements, outliner, gmap, atlas = build_geometry()

    buf = io.BytesIO()
    atlas.save(buf, format="PNG")
    tex_b64 = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")

    animations = []
    for anim_name in anim_names:
        anim_path = ANIM_DIR / f"{anim_name}.json"
        if anim_path.exists():
            baked = convert_animation(anim_path, gmap)
            if baked:
                animations.append(baked)

    return {
        "meta": {
            "format_version": "4.10",
            "model_format": "free",
            "box_uv": False,
        },
        "name": "BronzeSaberPlayerAnim",
        "model_identifier": "bronze_saber_player_anim",
        "resolution": {"width": H.ATLAS, "height": H.ATLAS},
        "elements": elements,
        "outliner": outliner,
        "textures": [
            {
                "path": "",
                "name": "player_saber_atlas.png",
                "folder": "bong",
                "namespace": "bong",
                "id": "0",
                "particle": False,
                "render_mode": "default",
                "saved": True,
                "uuid": _uuid(),
                "source": tex_b64,
            }
        ],
        "animations": animations,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--anims", nargs="*", default=DEFAULT_ANIMS, help="烘焙的动画列表")
    parser.add_argument("--out", default=str(OUT_BB), help="输出 .bbmodel 路径")
    args = parser.parse_args()

    out_path = Path(args.out)
    bbmodel_dict = build_bbmodel(args.anims)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    with open(out_path, "w", encoding="utf-8") as f:
        json.dump(bbmodel_dict, f, indent=2, ensure_ascii=False)
    print(f"✓ 成功落盘青铜刀玩家动画 .bbmodel: {out_path} (含 {len(bbmodel_dict['animations'])} 条动画)")


if __name__ == "__main__":
    main()
