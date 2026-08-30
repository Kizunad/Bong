#!/usr/bin/env python3
"""BeastSpineSwordPlayerAnim.bbmodel —— 玩家 + 异兽脊骨剑 + 内嵌 Blockbench 动画。

骨架与 `gen_club_player_anim.py` 同构（每轴一层 group + bend 层 + 三轴 body），换成
异兽脊骨剑挂右手，并把这几条烘进去供 Animate 模式播放/拖关键帧：

    sword_spine_slash   本剑专属单手斜斩：扛剑过右肩 → 过顶 → 斜劈左前下 → 撕扯回抽
    sword_spine_cleave  本剑专属双手竖斩：双手握柄举剑 → 沿中线整条劈下 → 挂住提回
    sword_swing_horiz   反手大斜斩：剑指左上 → 转刃过顶 → 斜劈右前下（走另一条对角线）
    sword_thrust / sword_parry / sword_infuse   借用的通用剑招（尚未按本剑重做）
    lower_walk / lower_sprint   纯下半身步态（上半身由架势轨道补齐，见下）

## 两个坑（第一版都踩了，症状分别是"没有动画"和"剑飘在身外"）

**1. 关键帧格式不是 `{"rotation": [...], "position": [...]}`。** Blockbench 的 animator
要的是 `{"keyframes": [{"channel": "rotation"|"position", ...}]}`。第一版自己写了一份
`bake_emotecraft_anim`，① 从 `doc["moves"]` 取动作（真实路径是 `doc["emote"]["moves"]`，
取到的永远是空列表）② `keyframe()` 的参数顺序错位 ③ 弧度当角度用 ④ 轴符号按读侧写。
四条叠在一起，加上一个 `except Exception: print("⚠ 失败")` 的静默兜底，产出的 8 条动画
全是 `length=0 / bones=0`——文件在 Blockbench 里打得开，Animate 模式里一帧都没有。
现在直接复用 `gen_club_player_anim.convert_animation`（轴换算收在 `rig.bb_anim_axes`
的唯一一处），**并且不再吞异常**：烘不出来就崩，别再让空动画混过去。

**2. 剑必须用出料系的握把点。** `BeastSpineSword.bbmodel` 现在按
`held_item_common.emit_offset` 出料，握把点就落在方块中心 (8,8,8)（授权系那份柄尾在
y=0 的坐标只活在生成器的 box 表里）。挂手就是把这一点搬到 `HAND_REST`。

## 握姿是**静态**的，不是逐动画关键帧

两层静态 group（`sword_right_pitch` −90° + `sword_right_roll` +90°）把剑身扳成**垂直于
小臂**，见下面 `add_part` 里的长注释。真机里这一层由物品的 `display.thirdperson_righthand`
承担——一个招式一个握角在游戏里表达不出来，所以在 Blockbench 里拖 `sword_*` 骨只能当
取景草稿，**必须反解回手臂四轴**才算数（`bbmodel-to-pose` + 本目录的反解流程）。

## ⚠ 手改过就别再跑生成器

这份是给人手调的。在 Blockbench 里改完存盘文件会变成 `format_version 5.0`，重跑本脚本
会**整份覆盖**。改完把数值反推回 `client/tools/gen_sword_spine_slash.py` 的 POSE 表
（换算见 `bbmodel-to-pose`），生成器和资产才不会分叉。

用法:
    python3 modelScript/generators/gen_beast_spine_sword_player_anim.py
    python3 modelScript/generators/gen_beast_spine_sword_player_anim.py --anims sword_spine_slash
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
from bbmodel_maker.rig import bb_anim_axes as AX  # noqa: E402  轴换算的唯一一处
from gen_club_player_anim import convert_animation as bake_animation  # noqa: E402
from gen_jian_player_anim import (  # noqa: E402  骨架/关键帧的公共件，别再抄一份
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
    "sword_spine_cleave",
    "sword_swing_horiz",
    "sword_thrust",
    "sword_parry",
    "sword_infuse",
    "lower_walk",
    "lower_sprint",
]

# 异兽脊骨剑是**出料系**：`gen_beast_spine_sword.EMIT_OFFSET` 已把握把点放到方块中心
# (8,8,8)px，剑尖朝 +Y。挂到手上就是把这一点搬到 HAND_REST。
SWORD_GRIP_PX = np.array([8.0, 8.0, 8.0])
# 只挂右手：左手是活的副手，双手握由 leftArm 的姿态去贴，不再复制一把剑。
SIDE = "right"

# 纯下半身动画（lower_*）没有手臂轨道，播起来剑会垂到零姿态、读作"握法变了"。
# 补一份恒定的持剑架势上半身。
#
# **这份架势只活在预览里，不进 emotecraft 源文件。** `lower_walk` / `lower_sprint`
# 按设计只写 leftLeg/rightLeg/body（`gen_lower_body_gait.assert_lower_only` 挡着），
# 上半身在真机里由招式动画或 vanilla 透传接管——真要做"持剑跑动"得另起一条
# UPPER_BODY 通道的架势动画并接触发，不在本文件范围内。
#
# **走和跑是两个握法**（用户 2026-08-30 拍板）：
#   walk   —— 扛在肩上（借 `sword_spine_slash` 的起手帧），行走时省力的携行姿态
#   sprint —— 横在身前、双手都搭在缠绳握把上（用户在 Blockbench 里 `lower_sprint`
#             t0 手摆，按静态垂直握姿反解回手臂四轴：剑尖残差 0.13px、左手离柄 0.56px）
# torso/head 两条共用同一组（用户未改动的原值）。
_STANCE_UPPER = dict(torso=dict(pitch=+2, yaw=+14), head=dict(pitch=-2, yaw=-6, roll=+0.7))
STANCE_POSES = {
    "lower_walk": dict(
        rightArm=dict(pitch=-117.9, yaw=-3.3, roll=-17.4, bend=44.5, axis=180),
        leftArm=dict(pitch=+23.5, yaw=+28.0, roll=-24.0, bend=29.5, axis=180),
        **_STANCE_UPPER,
    ),
    "lower_sprint": dict(
        rightArm=dict(pitch=-62.7, yaw=+23.7, roll=-101.5, bend=24.9, axis=180),
        leftArm=dict(pitch=-79.3, yaw=+47.2, roll=-31.7, bend=16.8, axis=180),
        **_STANCE_UPPER,
    ),
}
#: 没登记的纯下半身动画退回扛肩姿态（携行是默认，横持是冲刺专用）。
DEFAULT_STANCE = STANCE_POSES["lower_walk"]
UPPER_PARTS = tuple(DEFAULT_STANCE)


def load_sword():
    """读剑 bbmodel，返回 (elements, 贴图, 贴图宽高)。"""
    doc = json.loads(SRC_SWORD.read_text(encoding="utf-8"))
    tex = doc["textures"][0]
    image = Image.open(
        io.BytesIO(base64.b64decode(tex["source"].split(",", 1)[1]))
    ).convert("RGBA")
    res = doc.get("resolution", {"width": 64, "height": 64})
    return doc["elements"], image, (int(res["width"]), int(res["height"]))


def build_geometry():
    """→ (elements, outliner, group_uuid_by_name, atlas)。"""
    sword_elements, sword_tex, (sword_w, sword_h) = load_sword()
    if sword_w > H.ATLAS or sword_h + H.WEAPON_V_OFF > H.ATLAS:
        raise SystemExit(f"剑贴图 {sword_w}×{sword_h} 放不进 {H.ATLAS}² 图集的下半")
    atlas = Image.new("RGBA", (H.ATLAS, H.ATLAS), (0, 0, 0, 0))
    atlas.paste(H.make_skin(), (0, 0))
    # **不缩放**贴到图集下半：剑贴图是 64² 四象限图集，缩放会把四种材质糊到一起。
    # UV 因此只需整体下移 WEAPON_V_OFF，横向原样。
    atlas.paste(sword_tex, (0, H.WEAPON_V_OFF))

    elements: list[dict] = []
    gmap: dict[str, str] = {}

    def add_part(part_name):
        """→ (最外层 roll group, bend group or None)。三层单轴：pitch→yaw→roll 由内到外。"""
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
        for axis_name in ("pitch", "yaw", "roll"):   # 内 → 外
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
                    el[key] = [round(v + offset[i], 4) for i, v in enumerate(el[key])]
                for face in el["faces"].values():
                    u1, v1, u2, v2 = face["uv"]
                    face["uv"] = [u1, v1 + H.WEAPON_V_OFF, u2, v2 + H.WEAPON_V_OFF]
                elements.append(el)
                sword_ids.append(el["uuid"])

            # 握姿 = **剑身垂直于小臂**，不是顺着小臂。这两层静态角就是那个握姿：
            #   pitch −90°  剑的局部 +Y（握把→剑尖）从"顺着手臂"扳成指向 −Z，
            #               而 −Z 正是模型正面（`framing.LEGACY_FACING`）。手自然
            #               垂下时剑平指身前，拳心真的能合拢在缠绳握把上。
            #   roll  +90°  绕剑身自转，让刃口朝上下、剑面朝左右（本剑 blade 在局部
            #               X 上宽 4.1px、Z 上厚 1.26px，刃在 ±X、面在 ±Z）。
            #
            # 上一版写的是 pitch 180°——剑尖顺着小臂朝下、和前臂完全平行，看着像"从
            # 拳头里捅出来一根骨头"，握不住。用户在 Blockbench 里手摆 spine_slash 首帧
            # 时把它改成了这个垂直握姿（实测局部欧拉 ≈ (−89.8, +2.5, +95.0)，2~5° 是拖
            # gizmo 的抖动，这里取整为正交值）。
            #
            # 静态 group.rotation 走标准右手系（不吃动画通道的预取反），直接写角度。
            # 真机里这一层由物品的 display.thirdperson_righthand 承担，不是动画的事——
            # 所以**不要**改成逐动画的关键帧，那在游戏里表达不出来。
            pitch_g = group(
                f"sword_{side}_pitch", hand, sword_ids, (-90.0, 0.0, 0.0), color=1
            )
            roll_g = group(
                f"sword_{side}_roll", hand, [pitch_g], (0.0, 0.0, 90.0), color=1
            )
            gmap[f"sword_{side}_pitch"] = pitch_g["uuid"]
            gmap[f"sword_{side}_roll"] = roll_g["uuid"]
            # 挂在小臂（bend 段）之下：肘一弯，剑跟着走
            (bend_group or top)["children"].append(roll_g)
        arms.append(top)

    # body：位移一层 + 三层单轴旋转（本剑的动作有恒定 body.yaw，缺 yaw 层人就不侧身）
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
    """把恒定的持剑架势写进上半身轨道。**只补预览，emotecraft 源文件不动**。"""
    animators = anim["animators"]

    def track(group_name):
        gid = gmap[group_name]
        animators.setdefault(gid, {"name": group_name, "type": "bone", "keyframes": []})
        return animators[gid]["keyframes"]

    # 首末各钉一帧：单帧在 loop 动画里会被插值回 defaultValue（PlayerAnimator 那条坑
    # 的 Blockbench 版本），两帧同值才真的"恒定"。
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
    """emotecraft v3 → Blockbench animation，纯下半身的补一份架势上半身。"""
    anim = bake_animation(json_path, gmap)
    if not _has_upper_body(json_path):
        stance = STANCE_POSES.get(anim["name"], DEFAULT_STANCE)
        _fill_upper_body(anim, gmap, stance)
        which = "横持" if stance is STANCE_POSES["lower_sprint"] else "扛肩"
        print(f"    （{anim['name']}: 补了{which}持剑架势上半身轨道供预览，源 JSON 未改）")
    return anim


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--anims", nargs="*", default=DEFAULT_ANIMS)
    ap.add_argument("--out", default=str(OUT_BB))
    args = ap.parse_args()

    elements, outliner, gmap, atlas = build_geometry()
    animations = []
    for anim in args.anims:
        path = ANIM_DIR / f"{anim}.json"
        if not path.exists():
            raise SystemExit(f"找不到动画 {path}")
        # **不吞异常**：第一版的静默兜底正是"8 条动画全空"能混出门的原因。
        animations.append(convert_animation(path, gmap))

    buf = io.BytesIO()
    atlas.save(buf, format="PNG")
    model = {
        "meta": {"format_version": "4.10", "model_format": "free", "box_uv": False},
        "name": "beast_spine_sword_player_anim",
        "model_identifier": "geometry.bong.beast_spine_sword_player_anim",
        "visible_box": [3.0, 3.0, 2.0],
        "resolution": {"width": H.ATLAS, "height": H.ATLAS},
        "elements": elements, "outliner": outliner, "animations": animations,
        "textures": [{
            "path": "", "name": "beast_spine_sword_player_anim.png",
            "folder": "item", "namespace": "bong",
            "id": "0", "width": H.ATLAS, "height": H.ATLAS,
            "uv_width": H.ATLAS, "uv_height": H.ATLAS,
            "particle": False, "render_mode": "default", "visible": True, "mode": "bitmap",
            "saved": False, "uuid": _uuid(),
            "source": "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode(),
        }],
    }
    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(model, ensure_ascii=False, indent=1), encoding="utf-8")
    print(f"BeastSpineSwordPlayerAnim: {len(elements)} elements / {len(animations)} animations")
    for a in animations:
        frames = sum(len(v["keyframes"]) for v in a["animators"].values())
        print(f"  {a['name']:18s} {a['length']:.2f}s {a['loop']:5s} "
              f"bones={len(a['animators'])} keyframes={frames}")
    print(f"  → {out.relative_to(REPO) if out.is_relative_to(REPO) else out} "
          f"({out.stat().st_size} B)")


if __name__ == "__main__":
    main()
