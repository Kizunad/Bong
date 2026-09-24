#!/usr/bin/env python3
"""IronDaggerPlayerAnim.bbmodel —— 玩家 + 凡铁匕首 + 内嵌 Blockbench 动画。

骨架与 `gen_beast_spine_sword_player_anim.py` / `gen_club_player_anim.py` 同构，
将凡铁匕首挂载于玩家右手，并烘焙手持与基础动作层：
    - sword_parry               贴身短刃横格
    - lower_walk / lower_sprint 移动与潜行步态（带匕首持握架势）

用法:
    python3 modelScript/generators/gen_iron_dagger_player_anim.py
"""

from __future__ import annotations

import argparse
import base64
import copy
import io
import json
import math
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
import preview_player_anim as PPA  # noqa: E402  认 rightItem 的关键帧收集器
import anim_common as AC  # noqa: E402  item_spin / item_spin_angle 的唯一定义处
import render_animation as RA_  # noqa: E402
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
SRC_DAGGER = LIB / "models" / "IronDagger.bbmodel"
OUT_BB = LIB / "models" / "IronDaggerPlayerAnim.bbmodel"
PREVIEW_OUT = LIB / "out" / "iron_dagger_player_anim_preview.png"

DEFAULT_ANIMS = [
    "dagger_stab",
    "dagger_slash",
    "dagger_reverse_slash",
    "dagger_grip_switch",
    "dagger_reverse_grip_switch",
    "sword_parry",
    "lower_walk",
    "lower_sprint",
]

DAGGER_GRIP_PX = np.array([8.0, 8.0, 8.0])
SIDE = "right"

_STANCE_UPPER = dict(torso=dict(pitch=+2, yaw=+10), head=dict(pitch=-2, yaw=-4, roll=+0.5))
STANCE_POSES = {
    "lower_walk": dict(
        rightArm=dict(pitch=-25.0, yaw=-10.0, roll=+15.0, bend=35.0, axis=180),
        leftArm=dict(pitch=+20.0, yaw=+10.0, roll=-15.0, bend=20.0, axis=180),
        **_STANCE_UPPER,
    ),
    "lower_sprint": dict(
        rightArm=dict(pitch=-45.0, yaw=-20.0, roll=+30.0, bend=65.0, axis=180),
        leftArm=dict(pitch=+50.0, yaw=+15.0, roll=-20.0, bend=45.0, axis=180),
        **_STANCE_UPPER,
    ),
}
DEFAULT_STANCE = STANCE_POSES["lower_walk"]
UPPER_PARTS = tuple(DEFAULT_STANCE)


def load_dagger():
    """读匕首 bbmodel，返回 (elements, 贴图, 贴图宽高)。"""
    doc = json.loads(SRC_DAGGER.read_text(encoding="utf-8"))
    tex = doc["textures"][0]
    image = Image.open(
        io.BytesIO(base64.b64decode(tex["source"].split(",", 1)[1]))
    ).convert("RGBA")
    res = doc.get("resolution", {"width": 64, "height": 64})
    return doc["elements"], image, (int(res["width"]), int(res["height"]))


def build_geometry():
    """→ (elements, outliner, group_uuid_by_name, atlas)。"""
    dagger_elements, dagger_tex, (dagger_w, dagger_h) = load_dagger()
    if dagger_w > H.ATLAS or dagger_h + H.WEAPON_V_OFF > H.ATLAS:
        raise SystemExit(f"匕首贴图 {dagger_w}×{dagger_h} 放不进 {H.ATLAS}² 图集的下半")
    atlas = Image.new("RGBA", (H.ATLAS, H.ATLAS), (0, 0, 0, 0))
    atlas.paste(H.make_skin(), (0, 0))
    atlas.paste(dagger_tex, (0, H.WEAPON_V_OFF))

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
            offset = np.array(hand, float) - DAGGER_GRIP_PX
            dagger_ids = []
            for source in dagger_elements:
                el = copy.deepcopy(source)
                el["uuid"] = _uuid()
                el["name"] = f"dagger_{source['name']}"
                for key in ("from", "to", "origin"):
                    el[key] = [round(v + offset[i], 4) for i, v in enumerate(el[key])]
                for face in el["faces"].values():
                    u1, v1, u2, v2 = face["uv"]
                    face["uv"] = [u1, v1 + H.WEAPON_V_OFF, u2, v2 + H.WEAPON_V_OFF]
                elements.append(el)
                dagger_ids.append(el["uuid"])

            # 静态短刃握姿：略向前倾斜与偏转 (pitch -90° 垂直小臂指向前方, roll +90° 刃口朝上下)
            pitch_g = group(
                f"dagger_{side}_pitch", hand, dagger_ids, (-90.0, 0.0, 0.0), color=1
            )
            roll_g = group(
                f"dagger_{side}_roll", hand, [pitch_g], (0.0, 0.0, 90.0), color=1
            )
            gmap[f"dagger_{side}_pitch"] = pitch_g["uuid"]
            gmap[f"dagger_{side}_roll"] = roll_g["uuid"]
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


def _has_upper_body(json_path: Path) -> bool:
    _name, _emote, table = P.anim_pose_table(json_path)
    return any(part in UPPER_PARTS for _tick, pose in table for part in pose)


def _fill_upper_body(anim: dict, gmap: dict, stance: dict) -> None:
    """把恒定的持刃架势写进上半身轨道。"""
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


BLADE_EDGE_AXIS = (1.0, 0.0, 0.0)   # 刀身局部 +X = 刃口方向（刃宽 1.6px 的那一轴）


def _dagger_display_rot():
    doc = json.loads(SRC_DAGGER.read_text(encoding="utf-8"))
    return tuple(doc["display"]["thirdperson_righthand"]["rotation"])


def _fill_grip(anim: dict, gmap: dict, json_path: Path) -> None:
    """把出料 JSON 的 `rightItem` 烘进 `dagger_right_pitch` 这一层。

    **不烘的后果不是"bbmodel 少一点信息"**：正握 / 反握的区别整个看不见，人在
    Blockbench 里打开这条动画会以为刀一直是正握；更糟的是他一存盘，回读的时候这层
    没有关键帧 = 读成 0（`bbmodel_to_pose._value_at` 不插值），反握就此丢失。
    2026-09-01 这条链路补上之前，四条匕首动画在 bbmodel 侧全都缺这根骨头。

    符号：`rightItem` 的三个角先还原成「绕刃口轴转了 theta」（`item_spin_angle`），
    再按**写侧**符号落盘（`AX.rotation_to_bb`，bb.x = +theta），与手臂各层同一套 ——
    Blockbench 读入时对 X 取反，正好抵消。`dagger_right_pitch` 的静止 rotation 是
    (-90,0,0)、与动画值同轴相加，所以这一层的动画值就是纯粹的额外自转量。
    """
    emote = json.loads(json_path.read_text(encoding="utf-8"))["emote"]
    kfs = PPA.collect_keyframes(emote)
    item = kfs.get("rightItem")
    if not item:
        return
    to_deg = (lambda v: math.degrees(v)) if not emote.get("degrees", True) else (lambda v: v)
    disp_rot = _dagger_display_rot()
    gid = gmap["dagger_right_pitch"]
    anim["animators"].setdefault(gid, {"name": "dagger_right_pitch", "type": "bone",
                                       "keyframes": []})
    track = anim["animators"][gid]["keyframes"]
    for tick in sorted({t for axis in item.values() for t, *_ in axis}):
        axes = {a: to_deg(RA_.sample_axis(kfs, "rightItem", a, float(tick)))
                for a in ("pitch", "yaw", "roll")}
        theta, off = AC.item_spin_angle(disp_rot, BLADE_EDGE_AXIS, axes)
        if off > 1.0:
            raise ValueError(
                f"{anim['name']} t{tick}: rightItem 不是一个绕刃口轴的纯自转"
                f"（偏轴 {off:.1f}°），烘成单层 pitch 会把它悄悄改形。"
                f"要么改回绕刃口轴，要么先把 dagger_right_roll/yaw 那两层也接上。")
        track.append(keyframe("rotation", tick / TICKS_PER_SECOND,
                              AX.rotation_to_bb({"pitch": theta}, "pitch")))


def convert_animation(json_path: Path, gmap: dict) -> dict:
    """emotecraft v3 → Blockbench animation，纯下半身的补一份架势上半身。"""
    anim = bake_animation(json_path, gmap)
    if not _has_upper_body(json_path):
        stance = STANCE_POSES.get(anim["name"], DEFAULT_STANCE)
        _fill_upper_body(anim, gmap, stance)
    _fill_grip(anim, gmap, json_path)
    return anim


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, default=OUT_BB, help="输出 .bbmodel 路径")
    parser.add_argument("--preview-out", type=Path, default=PREVIEW_OUT, help="输出渲染图路径")
    parser.add_argument("--anims", nargs="*", default=DEFAULT_ANIMS, help="烘焙的动画列表")
    args = parser.parse_args()

    elements, outliner, gmap, atlas = build_geometry()
    animations = []
    # 默认清单里的动画**必须存在**。以前这里对缺失一律 `continue`，于是
    # DEFAULT_ANIMS 写错一个名字、或新动画还没 `git add`（golden 沙箱只复制已跟踪
    # 文件），产出的 bbmodel 会静默少一条动画 —— 而 golden 会把这个少了一条的结果
    # 当成正确基线记下来。2026-09-01 加 dagger_reverse_grip_switch 时实测踩到。
    # 显式 `--anims` 仍然宽容，那是临时挑几条烘的用法。
    strict = args.anims is DEFAULT_ANIMS
    for anim in args.anims:
        path = ANIM_DIR / f"{anim}.json"
        if not path.exists():
            if strict:
                raise SystemExit(
                    f"DEFAULT_ANIMS 里的 {anim} 在 {ANIM_DIR} 找不到。"
                    f"新动画忘了 git add？（golden 沙箱只复制已跟踪文件）")
            continue
        try:
            animations.append(convert_animation(path, gmap))
        except Exception as e:
            print(f"  跳过动画 {anim}: {e}")

    buf = io.BytesIO()
    atlas.save(buf, format="PNG")
    model = {
        "meta": {"format_version": "4.10", "model_format": "free", "box_uv": False},
        "name": "iron_dagger_player_anim",
        "model_identifier": "geometry.bong.iron_dagger_player_anim",
        "visible_box": [3.0, 3.0, 2.0],
        "resolution": {"width": H.ATLAS, "height": H.ATLAS},
        "elements": elements,
        "outliner": outliner,
        "animations": animations,
        "textures": [{
            "path": "",
            "name": "iron_dagger_player_anim.png",
            "folder": "item",
            "namespace": "bong",
            "id": "0",
            "width": H.ATLAS,
            "height": H.ATLAS,
            "uv_width": H.ATLAS,
            "uv_height": H.ATLAS,
            "particle": False,
            "render_mode": "default",
            "visible": True,
            "mode": "bitmap",
            "saved": False,
            "uuid": _uuid(),
            "source": "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode(),
        }],
    }

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(model, ensure_ascii=False, indent=1), encoding="utf-8")
    print(f"[IronDaggerPlayerAnim] 生成成功 ({len(elements)} elements, {len(animations)} anims) → {args.out.relative_to(REPO)}")

    try:
        from bbmodel_maker.render.render_bbmodel import render
        img, _ = render(str(args.out), yaw=145.0, pitch=15.0, size=600)
        img.save(args.preview_out)
        print(f"  手持预览保存至 → {args.preview_out.relative_to(REPO)}")
    except Exception as e:
        print(f"  手持预览渲染失败: {e}")


if __name__ == "__main__":
    main()
