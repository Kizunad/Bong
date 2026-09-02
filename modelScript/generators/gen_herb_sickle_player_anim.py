#!/usr/bin/env python3
"""HerbSicklePlayerAnim.bbmodel —— 玩家 + 采药刀 + 内嵌 Blockbench 玩家手持采药动作。

挂右手持采药刀，并把**采药刀自己的四条动作**与两条基础位移烘培进 Blockbench：

    1. harvest_crouch    - 割地上的药株（循环 20t，刃尖离地 7.5px）
    2. sickle_reap       - 割根一刀（一次性 10t，刀尖行程 15.5px）
    3. sickle_stand_cut  - 站立割齐胸的藤蔓（循环 24t，刃尖离地 15.9px）
    4. sickle_defend     - 应急防身横划（一次性 8t，肘最浅 42° 不打直）
    5. lower_walk        - 基础行走
    6. lower_sprint      - 基础疾跑

**`dagger_slash` 不在这里**：它是刀三件套（石刃 / 凡铁匕首 / 骨刺）按 `WoundKind` 选的
普攻，站架和身法都是兵器的（`body.yaw=-34°` 完整格斗架、腰转 62° 送肩）。采药刀是
`category=tool` 的凡器，防身走自己的 `sickle_defend`（人往后躲、肘折更深、副手护脸）。
把它烘进采药刀的 bbmodel 会让审图的人以为那是采药刀的动作。

用法:
    python3 modelScript/generators/gen_herb_sickle_player_anim.py
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
from gen_club_player_anim import (  # noqa: E402
    convert_animation as bake_animation,
    fill_upper_body,
)
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
SRC_SICKLE = LIB / "models" / "HerbSickle.bbmodel"
OUT_BB = LIB / "models" / "HerbSicklePlayerAnim.bbmodel"

DEFAULT_ANIMS = [
    "harvest_crouch",
    "sickle_reap",
    "sickle_stand_cut",
    "sickle_defend",
    "lower_walk",
    "lower_sprint",
]

SICKLE_GRIP_PX = np.array([8.0, 8.0, 8.0])
SIDE = "right"

# ── 携行架势：纯下半身动画的上半身补位（只补预览，出料 JSON 一个字节不动）─────────
#
# `lower_walk` / `lower_sprint` 按分身契约只写 leftLeg/rightLeg/body，运行时上半身
# 交给招式动画或 vanilla 透传。但 bbmodel 是离线审图用的、没有"上层"，不补的话预览
# 里两条胳膊死垂、刀悬在身侧——采药刀这份此前正是如此（剑和刀那两份早就有架势填充，
# 唯独这里没有）。
#
# **这里用的是动态架势**（`fill_upper_body` 支持的相位表形态），不是剑/刀那种恒定
# 携行姿：采药人空手走路时手臂本来就该摆。四组数**来自仓库所有者 2026-09-02 在
# Blockbench 里手摆的姿态**，这里只做两处必要的搬运：
#
#   1. 肘弯当常量。手改稿只在 t0 打了一帧 bend，读回来另一端是 0——那是单帧在 loop
#      里被插值回 defaultValue 的假象，不是作者设成了 0。
#   2. 两个极值按腿的相位落位。相位 0 处 `rightLeg.pitch = -swing`（右腿在前），所以
#      相位 0 取「右臂后摆 / 左臂前摆」那一端，半周期换边，相位 1 收口回 0。
#      符号：arm.pitch < 0 = 手臂前摆（实测 pitch=-70 时手前伸 9.8px）。
#
# 两端不是互为镜像（右臂 +30↔0、左臂 -7.5↔+17.5），摆幅本就不对称——那是手改稿的
# 原样，没有替作者"修正"成对称。
def _swing(r0, l0, r1, l1, bend):
    """(相位0 右/左臂 pitch, 半周期 右/左臂 pitch, 恒定肘弯) → 相位表。"""
    def pose(r, l):
        return dict(
            rightArm=dict(pitch=r, yaw=0.0, roll=0.0, bend=bend, axis=180),
            leftArm=dict(pitch=l, yaw=0.0, roll=0.0, bend=bend, axis=180),
        )
    return {0.0: pose(r0, l0), 0.5: pose(r1, l1), 1.0: pose(r0, l0)}


STANCE_POSES = {
    "lower_walk": _swing(+30.0, -7.5, 0.0, +17.5, 15.0),
    "lower_sprint": _swing(+25.0, -30.0, -25.0, +35.0, 30.0),
}
#: 没登记的纯下半身动画退回行走摆臂。
DEFAULT_STANCE = STANCE_POSES["lower_walk"]
UPPER_PARTS = ("rightArm", "leftArm")


def _has_upper_body(json_path) -> bool:
    """出料 JSON 已经写了手臂 → 那是招式动画，绝不许被架势覆盖。"""
    _name, _emote, table = P.anim_pose_table(json_path)
    return any(part in UPPER_PARTS for _tick, pose in table for part in pose)


def load_sickle():
    """读采药刀 bbmodel，如果不存在则现场由 gen_herb_sickle 生成"""
    if not SRC_SICKLE.exists():
        import gen_herb_sickle as GHS
        bb = GHS.build_bbmodel_json()
        SRC_SICKLE.parent.mkdir(parents=True, exist_ok=True)
        SRC_SICKLE.write_text(json.dumps(bb, indent=2, ensure_ascii=False), encoding="utf-8")

    doc = json.loads(SRC_SICKLE.read_text(encoding="utf-8"))
    tex = doc["textures"][0]
    image = Image.open(
        io.BytesIO(base64.b64decode(tex["source"].split(",", 1)[1]))
    ).convert("RGBA")
    res = doc.get("resolution", {"width": 64, "height": 64})
    return doc["elements"], image, (int(res["width"]), int(res["height"]))


def build_geometry():
    """→ (elements, outliner, group_uuid_by_name, atlas)"""
    sickle_elements, sickle_tex, (sickle_w, sickle_h) = load_sickle()
    if sickle_w > H.ATLAS or sickle_h + H.WEAPON_V_OFF > H.ATLAS:
        raise SystemExit(f"采药刀贴图 {sickle_w}×{sickle_h} 放不进 {H.ATLAS}² 图集的下半")
    atlas = Image.new("RGBA", (H.ATLAS, H.ATLAS), (0, 0, 0, 0))
    atlas.paste(H.make_skin(), (0, 0))
    atlas.paste(sickle_tex, (0, H.WEAPON_V_OFF))

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
            offset = np.array(hand, float) - SICKLE_GRIP_PX
            sickle_ids = []
            for source in sickle_elements:
                el = copy.deepcopy(source)
                el["uuid"] = _uuid()
                el["name"] = f"sickle_{source['name']}"
                for key in ("from", "to", "origin"):
                    if key in el:
                        el[key] = [round(v + offset[i], 4) for i, v in enumerate(el[key])]
                for face in el["faces"].values():
                    u1, v1, u2, v2 = face["uv"]
                    face["uv"] = [u1, v1 + H.WEAPON_V_OFF, u2, v2 + H.WEAPON_V_OFF]
                elements.append(el)
                sickle_ids.append(el["uuid"])

            # 180° 翻转：采药刀出料系是刀柄底在 y=0，刀刃朝 +Y。
            # 而手臂 cuboid 是从 pivot 向下 (-Y) 长的，需绕 X 轴 180° 翻转使刀身顺着手臂向下或自然手持握持。
            pitch_g = group(f"sickle_{side}_pitch", hand, sickle_ids, (180.0, 0.0, 0.0), color=1)
            roll_g = group(f"sickle_{side}_roll", hand, [pitch_g], (0.0, 0.0, 0.0), color=1)
            gmap[f"sickle_{side}_pitch"] = pitch_g["uuid"]
            gmap[f"sickle_{side}_roll"] = roll_g["uuid"]
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


def build_bbmodel():
    elements, outliner, gmap, atlas = build_geometry()

    buf = io.BytesIO()
    atlas.save(buf, format="PNG")
    b64 = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")

    # 烘培动画
    baked_animations = []
    for anim_name in DEFAULT_ANIMS:
        f = ANIM_DIR / f"{anim_name}.json"
        if not f.exists():
            # 静默跳过会让 bbmodel 少烘一条而没人发现（审图的人只会看到"这条没做"）。
            # 这些资产都由 `client/tools/gen_<name>.py` 出料，缺了就是没跑生成器。
            raise SystemExit(
                f"缺动画资产: {f}\n"
                f"    先跑 python3 client/tools/gen_{anim_name}.py 出料，再回来烘。")
        baked = bake_animation(f, gmap)
        if not _has_upper_body(f):
            fill_upper_body(baked, gmap,
                            STANCE_POSES.get(anim_name, DEFAULT_STANCE), PART_GROUPS)
            print(f"    （{anim_name}: 补了持刀摆臂架势供预览，源 JSON 未改）")
        baked_animations.append(baked)

    bbmodel = {
        "meta": {
            "format_version": "4.10",
            "model_format": "free",
            "box_uv": False,
        },
        "name": "HerbSicklePlayerAnim",
        "model_identifier": "herb_sickle_player_anim",
        "visible_box": [1, 1, 0],
        "resolution": {"width": H.ATLAS, "height": H.ATLAS},
        "elements": elements,
        "outliner": outliner,
        "textures": [
            {
                "path": "",
                "name": "herb_sickle_player_atlas.png",
                "folder": "item",
                "namespace": "bong",
                "id": "0",
                "particle": False,
                "render_mode": "default",
                "saved": True,
                "uuid": _uuid(),
                "source": b64,
            }
        ],
        "animations": baked_animations,
    }
    return bbmodel


def main():
    parser = argparse.ArgumentParser(description="采药刀玩家手持与采药动作 Blockbench 烘培模型生成器")
    parser.parse_args()

    OUT_BB.parent.mkdir(parents=True, exist_ok=True)
    doc = build_bbmodel()
    OUT_BB.write_text(json.dumps(doc, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"✓ 成功导出玩家手持采药动作模型: {OUT_BB.relative_to(REPO)}")


if __name__ == "__main__":
    main()
