#!/usr/bin/env python3
"""HerbKnifeIronPlayerAnim.bbmodel —— 玩家 + 凡铁采药刀 + 内嵌 Blockbench 专属采药/折刀动画。

包含动画：
    1. herb_harvest            - 凡铁采药刀俯身勾割灵草
    2. herb_knife_slash        - 凡铁采药刀反手横掠割
    3. herb_knife_unfold       - 凡铁折叠采药刀甩腕开刃
    4. herb_knife_carry_walk   - 持刀携行步态（**真正会上线的那条**，自带手臂摆动）
    5. lower_sprint            - 全局冲刺步态（采药刀暂无携行变体，上半身是现补的预览）

## 第 4 条为什么是 carry 而不是 lower_walk

`GaitVariants` 在手持采药刀时会把 WALK 档换成 `herb_knife_carry_walk`，所以在
Blockbench 里该看的是**它**。`lower_walk` 是空手/别的武器才播的那条，摆在这个工程里
只会让人对着一条根本不会跟采药刀同时出现的动画调姿态。

carry 那条自带手臂轨道，`_fill_upper_body` 不会再给它补预览架势（`_has_upper_body`
判真）——所见即所得。`lower_sprint` 还没有携行变体（理由见
`client/tools/gen_herb_knife_carry_gait.py` 模块文档），仍然走补预览那条路。

用法:
    python3 modelScript/generators/gen_herb_knife_iron_player_anim.py
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
SRC_KNIFE = LIB / "models" / "HerbKnifeIron.bbmodel"
OUT_BB = LIB / "models" / "HerbKnifeIronPlayerAnim.bbmodel"

DEFAULT_ANIMS = [
    "herb_harvest",
    "herb_knife_slash",
    "herb_knife_unfold",
    "herb_knife_carry_walk",
    "lower_sprint",
]

KNIFE_GRIP_PX = np.array([8.0, 8.0, 8.0])
SIDE = "right"


def load_knife():
    """读取 HerbKnifeIron.bbmodel"""
    if not SRC_KNIFE.exists():
        import gen_herb_knife_iron as GHKI
        bb, cubes, tex = GHKI.build_bbmodel()
        SRC_KNIFE.parent.mkdir(parents=True, exist_ok=True)
        SRC_KNIFE.write_text(json.dumps(bb, indent=2, ensure_ascii=False), encoding="utf-8")

    doc = json.loads(SRC_KNIFE.read_text(encoding="utf-8"))
    tex = doc["textures"][0]
    image = Image.open(
        io.BytesIO(base64.b64decode(tex["source"].split(",", 1)[1]))
    ).convert("RGBA")
    res = doc.get("resolution", {"width": 64, "height": 64})
    return doc["elements"], image, (int(res["width"]), int(res["height"]))


def build_geometry():
    """→ (elements, outliner, group_uuid_by_name, atlas)"""
    knife_elements, knife_tex, (knife_w, knife_h) = load_knife()
    if knife_w > H.ATLAS or knife_h + H.WEAPON_V_OFF > H.ATLAS:
        raise SystemExit(f"采药刀贴图 {knife_w}×{knife_h} 放不进 {H.ATLAS}² 图集的下半")
    atlas = Image.new("RGBA", (H.ATLAS, H.ATLAS), (0, 0, 0, 0))
    atlas.paste(H.make_skin(), (0, 0))
    atlas.paste(knife_tex, (0, H.WEAPON_V_OFF))

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
            offset = np.array(hand, float) - KNIFE_GRIP_PX
            knife_ids = []
            for source in knife_elements:
                el = copy.deepcopy(source)
                el["uuid"] = _uuid()
                el["name"] = f"knife_{source['name']}"
                for key in ("from", "to", "origin"):
                    if key in el:
                        el[key] = [round(v + offset[i], 4) for i, v in enumerate(el[key])]
                for face in el["faces"].values():
                    u1, v1, u2, v2 = face["uv"]
                    face["uv"] = [u1, v1 + H.WEAPON_V_OFF, u2, v2 + H.WEAPON_V_OFF]
                elements.append(el)
                knife_ids.append(el["uuid"])

            # 挂接刀身。这一组静态旋转是**拟合出来的**，不是随手摆的：目标是让
            # Blockbench 里看到的刀和游戏里一致。游戏那边走
            # `held_item_common.hand_display` 的 `[-80, 90, 0]` + 方块中心重定心
            # （见 `preview_player_anim.item_attach_modelpart` 逐字对齐的那条链），
            # 静止臂下柄朝下 (0,-0.98,+0.17)、刃朝上后 (-0.53,+0.84,-0.15)。
            # 上一版照抄脊骨剑的 (-90,0,0)+(0,0,90)，那是**竖直握姿**：柄笔直朝前
            # (0,0,+1)，等于在 Blockbench 里对着一把"从拳头里向前捅出去"的刀调姿态，
            # 而游戏里它是垂下来的——两边看到的根本不是同一件事。
            # 拟合残差 0.069（两个方向向量的欧氏距离和）；det 差一个镜像，刀是扁片，
            # 方向上体现不出来。
            knife_g = group(
                f"knife_{side}", hand, knife_ids, (-75.0, 80.0, 105.0), color=1
            )
            gmap[f"knife_{side}"] = knife_g["uuid"]
            (bend_group or top)["children"].append(knife_g)
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


_STANCE_UPPER = dict(torso=dict(pitch=+2, yaw=+10), head=dict(pitch=-2, yaw=-4, roll=0.0))
STANCE_POSES = {
    # `herb_knife_carry_walk` 不在此列——它自带手臂轨道，不需要补。
    "lower_walk": dict(
        rightArm=dict(pitch=-35.0, yaw=-15.0, roll=+20.0, bend=30.0, axis=180),
        leftArm=dict(pitch=+15.0, yaw=+15.0, roll=-10.0, bend=15.0, axis=180),
        **_STANCE_UPPER,
    ),
    "lower_sprint": dict(
        rightArm=dict(pitch=-30.0, yaw=-20.0, roll=+25.0, bend=40.0, axis=180),
        leftArm=dict(pitch=+20.0, yaw=+20.0, roll=-15.0, bend=20.0, axis=180),
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
                # **必须走 `AX.bend_to_bb`**，不能直接把 bend 塞进 X。bend 也走
                # animation 通道，同样吃写侧的 X 预取反：`axis=180`（本仓手臂全用它）
                # 时正解是 `bb.x = -bend`。这里原先写的是裸 `+bend`，符号正好反 ——
                # 于是 lower_walk / lower_sprint 在 Blockbench 里**两条手臂的肘都朝
                # 反方向折**。（`convert_animation` 那条主路径一直是对的，只有这个
                # 补预览用的分支漏了。）
                track(f"{prefix}_bend").append(
                    keyframe("rotation", t,
                             [round(AX.bend_to_bb(axes.get("bend", 0.0),
                                                  axes.get("axis", 0.0)), 4), 0.0, 0.0]))


def build_bbmodel():
    elements, outliner, gmap, atlas = build_geometry()
    buf = io.BytesIO()
    atlas.save(buf, format="PNG")
    tex_b64 = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")

    # 烘焙动画
    anims_json = []
    for anim_name in DEFAULT_ANIMS:
        anim_path = ANIM_DIR / f"{anim_name}.json"
        if not anim_path.exists():
            continue
        try:
            baked = bake_animation(anim_path, gmap)
            if not _has_upper_body(anim_path):
                stance = STANCE_POSES.get(anim_name, DEFAULT_STANCE)
                _fill_upper_body(baked, gmap, stance)
                print(f"    （{anim_name}: 补了持刀架势上半身轨道供预览，源 JSON 未改）")
            anims_json.append(baked)
        except Exception as e:
            print(f"Warning: bake anim {anim_name} failed: {e}")

    bbmodel = {
        "meta": {
            "format_version": "4.10",
            "model_format": "free",
            "box_uv": False,
        },
        "name": "HerbKnifeIronPlayerAnim",
        "model_identifier": "herb_knife_iron_player_anim",
        "visible_box": [-24.0, -8.0, -24.0, 24.0, 40.0, 24.0],
        "resolution": {"width": H.ATLAS, "height": H.ATLAS},
        "elements": elements,
        "outliner": outliner,
        "textures": [
            {
                "path": "",
                "name": "player_and_herb_knife_iron",
                "folder": "item",
                "namespace": "bong",
                "id": "0",
                "particle": False,
                "render_mode": "default",
                "visible": True,
                "mode": "bitmap",
                "saved": True,
                "uuid": _uuid(),
                "source": tex_b64,
            }
        ],
        "animations": anims_json,
    }
    return bbmodel


def main():
    parser = argparse.ArgumentParser(description="生成 玩家+凡铁采药刀 PlayerAnim .bbmodel")
    parser.add_argument("--out", type=Path, default=OUT_BB)
    args = parser.parse_args()

    args.out.parent.mkdir(parents=True, exist_ok=True)
    bbmodel = build_bbmodel()
    args.out.write_text(json.dumps(bbmodel, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"✓ 成功落盘凡铁采药刀玩家动画 .bbmodel: {args.out} (含 {len(bbmodel.get('animations', []))} 条动画)")


if __name__ == "__main__":
    main()
