#!/usr/bin/env python3
"""JianPlayerAnim.bbmodel —— 玩家 + 双锏 + 内嵌 Blockbench 动画，在 Blockbench 里直接播。

和 gen_jian_player.py 的分工：那个出静态握姿对比模型（你手改过的那份，本脚本不碰）；
这个把 emotecraft v3 动画烘成 Blockbench animation，供 Animate 模式播放/手调。

**为什么要换骨架结构**：
1. 静态版把玩家六件套全塞在一个 player_ref group 里，没法逐部位做关键帧；
2. Blockbench 没有 bend 概念（那是 bendy-lib 在渲染期折 cuboid 顶点），所以每个可弯
   部位拆成「上段 group（承担 pitch/yaw/roll）→ 下段 group（承担 bend，pivot 落在
   cuboid 几何中心）」两层，肘/膝/腰才有得看。

    root_pos                                   ← body 位移
    └ root_pitch                               ← body 前倾（单轴）
      ├ head_roll → head_yaw → head_pitch
      ├ torso_roll → torso_yaw → torso_pitch → torso_bend
      ├ arm_right_roll → …_yaw → …_pitch → arm_right_bend
      │                                        └ jian_right_roll → jian_right_pitch → 锏
      ├ arm_left_…（同上）
      ├ leg_right_roll → …_yaw → …_pitch → leg_right_bend
      └ leg_left_…（同上）

**角度换算**（MC 模型空间 y 向下 → bbmodel y 向上）：
    bb.x = -pitch,  bb.y = +yaw,  bb.z = -roll
    body 位移 bb = (x, -y, z) × 16（米 → px）

**每个轴一层 group，绝不把多轴写进同一个 group**：MC 的 ModelPart 是
`rotationZYX(roll, yaw, pitch)`（先绕 X、再 Y、再 Z 作用到向量），Blockbench 的 bone
走 THREE.js Euler，多轴同时非零时组合顺序未必一致——手臂那种 pitch/yaw/roll 三轴都
大的姿态，顺序一反手就甩到身体另一侧。拆成嵌套单轴（内 pitch → 中 yaw → 外 roll，
与 ZYX 的作用次序一致）后顺序无从解释歧义，两边必然一致。静态版 JianPlayer 就是这么
做的，所以那份手改能逐像素对拍上。
bend 是「绕 (cos axis, 0, sin axis) 轴转 -bendValue」，本项目只用纯 X 折弯（axis 0/180）。

**插值**：Blockbench 只有 linear/catmullrom/step/bezier，写 linear——它忠实于关键帧
数值，不会引入原动画没有的过冲。真实游戏里的 easing 是 INOUTSINE/OUTQUAD，观感会比
这里更顺，别拿 Blockbench 的播放去评判缓动手感。

用法:
    python3 modelScript/generators/gen_jian_player_anim.py
    python3 modelScript/generators/gen_jian_player_anim.py --anims jian_dual_smash lower_sprint
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

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "core"))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))

from bbmodel_maker.rig import bb_anim_axes as AX  # noqa: E402  MC ↔ bbmodel 轴换算的唯一一处
from bbmodel_maker.render import held_item_render as H  # noqa: E402
from bbmodel_maker.render import render_player_pose as P  # noqa: E402

REPO = Path(__file__).resolve().parents[2]
ANIM_DIR = REPO / "client" / "src" / "main" / "resources" / "assets" / "bong" / "player_animation"
SRC_JIAN = Path(__file__).resolve().parents[1] / "models" / "BambooJianSingle.bbmodel"
OUT_BB = Path(__file__).resolve().parents[1] / "models" / "JianPlayerAnim.bbmodel"

DEFAULT_ANIMS = [
    "jian_stance_high_low", "jian_draw_waist", "jian_waist_spin_cross",
    "jian_dual_smash", "jian_dual_sweep",
    "lower_walk", "lower_jog", "lower_sprint", "lower_dash",
]

# part → (group 前缀, 是否有 bend 层)。实际 group 名 = <前缀>_{roll,yaw,pitch}[,_bend]
PART_GROUPS = {
    "head": ("head", False),
    "torso": ("torso", True),
    "rightArm": ("arm_right", True),
    "leftArm": ("arm_left", True),
    "rightLeg": ("leg_right", True),
    "leftLeg": ("leg_left", True),
}
# 单轴层与 bend 的换算收在 `core/bb_anim_axes`（唯一一处）。
#
# **这里原先写反了**：老注释说动画通道走「Bedrock 约定：X/Y 取反」，据此生成的 bbmodel
# 在 Blockbench 里是 pitch / yaw 双双镜像的姿态。2026-08-26 一次真实往返测出了正确符号
# （证据见 `bb_anim_axes` 模块 docstring）——静态 group.rotation 和动画关键帧是**同一套**
# 右手系。`JianPlayerAnim.bbmodel` 随本次修正一并重生成。
AXIS_LAYERS = AX.AXIS_LAYERS
bend_single_axis = AX.bend_to_bb

TICKS_PER_SECOND = 20.0
UPPER_PARTS = ("rightArm", "leftArm", "torso", "head")
STANCE_ANIM = "jian_stance_high_low"


def _uuid():
    return str(uuid.uuid4())


def euler_zyx_deg(M):
    """旋转矩阵 → ZYX 欧拉角（度），与 Blockbench/MC 的 bone 顺序一致。"""
    beta = math.asin(max(-1.0, min(1.0, -M[2][0])))
    if abs(math.cos(beta)) < 1e-8:      # 万向锁：把自由度并到 z
        alpha = math.atan2(-M[1][2], M[1][1])
        gamma = 0.0
    else:
        alpha = math.atan2(M[2][1], M[2][2])
        gamma = math.atan2(M[1][0], M[0][0])
    return [math.degrees(alpha), math.degrees(beta), math.degrees(gamma)]


# ── 几何：玩家分段 cube + 锏 ──────────────────────────────────────────────
def split_cubes(name: str):
    """把一个 part 的 cuboid 按 bend 中心切上下两段，返回 [(段名, from, to, uv, 是否下段)]。"""
    spec = P.PARTS[name]
    frm, to = [list(map(float, v)) for v in spec["box"]]
    size = (to[0] - frm[0], to[1] - frm[1], to[2] - frm[2])
    uv_full = H.box_uv(spec["uv"], size)
    if not spec["bendable"]:
        return [(name, frm, to, uv_full, False)]
    y_mid = spec["bend_center"][1]
    span = to[1] - frm[1]
    f = (y_mid - frm[1]) / span

    def sliced(v0, v1):
        out = {}
        for face, (u1, vv1, u2, vv2) in uv_full.items():
            if face in ("west", "east", "north", "south"):
                h = vv2 - vv1
                out[face] = [u1, vv1 + h * (1 - v1), u2, vv1 + h * (1 - v0)]
            else:
                out[face] = [u1, vv1, u2, vv2]
        return out

    return [
        (f"{name}_upper", [frm[0], y_mid, frm[2]], to, sliced(f, 1.0), False),
        (f"{name}_lower", frm, [to[0], y_mid, to[2]], sliced(0.0, f), True),
    ]


def cube_element(name, frm, to, uv, color=7):
    return {
        "name": name, "box_uv": False, "rescale": False, "locked": False,
        "render_order": "default", "allow_mirror_modeling": True, "type": "cube",
        "uuid": _uuid(), "from": [float(v) for v in frm], "to": [float(v) for v in to],
        "autouv": 0, "color": color, "origin": [0.0, 0.0, 0.0], "rotation": [0.0, 0.0, 0.0],
        "faces": {f: {"uv": [float(x) for x in v], "texture": 0} for f, v in uv.items()},
    }


def group(name, origin, children, rotation=(0.0, 0.0, 0.0), color=0):
    return {
        "name": name, "origin": [round(float(v), 4) for v in origin],
        "rotation": [round(float(v), 4) for v in rotation],
        "color": color, "uuid": _uuid(), "export": True, "mirror_uv": False,
        "isOpen": False, "locked": False, "visibility": True, "autouv": 0,
        "children": children,
    }


def build_geometry():
    """返回 (elements, outliner, group_uuid_by_name, atlas)。"""
    src = H.load_model_document(SRC_JIAN)
    jian_tex = Image.open(io.BytesIO(base64.b64decode(
        src["textures"][0]["source"].split(",", 1)[1]))).convert("RGBA")
    atlas = Image.new("RGBA", (H.ATLAS, H.ATLAS), (0, 0, 0, 0))
    atlas.paste(H.make_skin(), (0, 0))
    atlas.paste(jian_tex.resize((H.SKIN, H.SKIN), Image.NEAREST), (0, H.WEAPON_V_OFF))

    elements = []
    gmap = {}

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
        hand = H.HAND_REST[side]
        # 锏：几何搬自 BambooJian，平移到"握把中心落在手心"，UV 移进图集下半
        off = np.array(hand, float) - H.GRIP_ANCHOR
        jian_ids = []
        for e in src["elements"]:
            e = copy.deepcopy(e)
            e["uuid"] = _uuid()
            e["name"] = f"{e['name'].rsplit('_', 1)[0]}_{side}"
            for key in ("from", "to", "origin"):
                e[key] = [round(v + off[i], 4) for i, v in enumerate(e[key])]
            for fd in e["faces"].values():
                u1, v1, u2, v2 = fd["uv"]
                fd["uv"] = [u1, v1 + H.WEAPON_V_OFF, u2, v2 + H.WEAPON_V_OFF]
            elements.append(e)
            jian_ids.append(e["uuid"])
        # 锏沿小臂延长线，必须与 render_player_pose.jian_tris 的假设一致——那边显式把锏
        # 对齐到 bend 之后的小臂方向，架势参数（两尖汇聚、眼→尖下斜线）就是按这个搜的。
        #
        # 这里的 180° 不能省：锏的局部 +Y 是「柄尾→锏尖」，而手臂 cuboid 是从 pivot 向
        # 【下】(-Y) 长的。腕角归零意味着锏尖指向小臂的反方向（朝肘上方），必须绕 X 翻
        # 180° 才与小臂同向。静态 group.rotation 走标准右手系（不吃动画通道的 Bedrock
        # 取反），所以直接写 180。
        pitch_g = group(f"jian_{side}_pitch", hand, jian_ids, (180.0, 0.0, 0.0), color=1)
        roll_g = group(f"jian_{side}_roll", hand, [pitch_g], (0.0, 0.0, 0.0), color=1)
        gmap[f"jian_{side}_pitch"] = pitch_g["uuid"]
        gmap[f"jian_{side}_roll"] = roll_g["uuid"]
        # 锏挂在小臂（bend 段）之下：肘一弯，锏跟着走
        (bend_group or top)["children"].append(roll_g)
        arms.append(top)

    # body 也拆：位移一层、前倾一层（本项目的步态只用到 body 的 y/z/pitch）
    root_pitch = group("root_pitch", (0.0, 0.0, 0.0), [head, torso] + arms + legs, color=3)
    root_pos = group("root_pos", (0.0, 0.0, 0.0), [root_pitch], color=3)
    gmap["root_pitch"] = root_pitch["uuid"]
    gmap["root_pos"] = root_pos["uuid"]
    return elements, [root_pos], gmap, atlas


# ── 动画：emotecraft v3 → Blockbench animation ────────────────────────────
def keyframe(channel, time, values, interpolation="linear"):
    return {
        "channel": channel, "data_points": [{"x": str(values[0]), "y": str(values[1]),
                                             "z": str(values[2])}],
        "uuid": _uuid(), "time": round(time, 4), "color": -1,
        "interpolation": interpolation, "bezier_linked": True,
        "bezier_left_time": [-0.1, -0.1, -0.1], "bezier_left_value": [0.0, 0.0, 0.0],
        "bezier_right_time": [0.1, 0.1, 0.1], "bezier_right_value": [0.0, 0.0, 0.0],
    }


def stance_upper_axes():
    """架势 t0 的上半身姿态——用来给纯下半身动画补预览轨道。"""
    _n, _e, table = P.anim_pose_table(ANIM_DIR / f"{STANCE_ANIM}.json")
    return {part: axes for part, axes in table[0][1].items() if part in UPPER_PARTS}


def convert_animation(json_path: Path, gmap: dict):
    name, emote, table = P.anim_pose_table(json_path)
    animators = {}
    # 纯下半身动画（lower_*）按分身契约不写手臂，Blockbench 里播它们时手臂会停在零姿态、
    # 锏垂下去，看着像"握法变了"。游戏里的真实效果是 LOWER_BODY 步态 + UPPER_BODY 架势
    # 两层叠加，所以这里补一份恒定的架势上半身轨道——【只补预览，emotecraft 源文件不动】，
    # 分身契约不破。
    has_upper = any(part in UPPER_PARTS for _t, pose in table for part in pose)
    filler = None if has_upper else stance_upper_axes()

    def track(group_name):
        gid = gmap[group_name]
        animators.setdefault(gid, {"name": group_name, "type": "bone", "keyframes": []})
        return animators[gid]["keyframes"]

    for tick, pose in table:
        t = tick / TICKS_PER_SECOND
        body = pose.pop("_body", None)
        if body:
            if abs(body.get("yaw", 0.0)) > 1e-9 or abs(body.get("roll", 0.0)) > 1e-9:
                raise AssertionError("body 目前只支持 pitch 单轴（步态用到的就这一轴）")
            track("root_pos").append(
                keyframe("position", t, AX.body_position_to_bb(body)))
            track("root_pitch").append(
                keyframe("rotation", t, AX.rotation_to_bb(body, "pitch")))
        frame = dict(pose)
        if filler:
            frame.update(filler)
        for part, axes in frame.items():
            if part not in PART_GROUPS:
                continue
            prefix, has_bend = PART_GROUPS[part]
            for axis_name in AX.AXIS_ORDER:
                track(f"{prefix}_{axis_name}").append(
                    keyframe("rotation", t, AX.rotation_to_bb(axes, axis_name)))
            if has_bend:
                bend = bend_single_axis(axes.get("bend", 0.0), axes.get("axis", 0.0))
                track(f"{prefix}_bend").append(keyframe("rotation", t, [round(bend, 4), 0.0, 0.0]))

    if filler:
        print(f"    （{name}: 补了架势上半身轨道供预览，源 JSON 未改）")
    return {
        "uuid": _uuid(), "name": name,
        "loop": "loop" if emote.get("isLoop") else "once",
        "override": False, "length": round(emote["endTick"] / TICKS_PER_SECOND, 4),
        "snapping": int(TICKS_PER_SECOND), "selected": False, "saved": True, "path": "",
        "anim_time_update": "", "blend_weight": "", "start_delay": "", "loop_delay": "",
        "animators": animators,
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--anims", nargs="*", default=DEFAULT_ANIMS)
    ap.add_argument("--out", default=str(OUT_BB))
    args = ap.parse_args()

    elements, outliner, gmap, atlas = build_geometry()
    animations = []
    for anim in args.anims:
        path = ANIM_DIR / f"{anim}.json"
        if not path.exists():
            raise SystemExit(f"找不到动画 {path}")
        animations.append(convert_animation(path, gmap))

    buf = io.BytesIO()
    atlas.save(buf, format="PNG")
    model = {
        "meta": {"format_version": "4.10", "model_format": "free", "box_uv": False},
        "name": "jian_player_anim", "model_identifier": "geometry.bong.jian_player_anim",
        "visible_box": [3.0, 3.0, 2.0], "resolution": {"width": H.ATLAS, "height": H.ATLAS},
        "elements": elements, "outliner": outliner, "animations": animations,
        "textures": [{
            "path": "", "name": "jian_player_anim.png", "folder": "item", "namespace": "bong",
            "id": "0", "width": H.ATLAS, "height": H.ATLAS,
            "uv_width": H.ATLAS, "uv_height": H.ATLAS,
            "particle": False, "render_mode": "default", "visible": True, "mode": "bitmap",
            "saved": False, "uuid": _uuid(),
            "source": "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode(),
        }],
    }
    out = Path(args.out)
    out.write_text(json.dumps(model, ensure_ascii=False, indent=1))
    print(f"JianPlayerAnim: {len(elements)} elements / {len(animations)} animations")
    for a in animations:
        print(f"  {a['name']:18s} {a['length']:.2f}s {a['loop']:5s} "
              f"bones={len(a['animators'])} keyframes={sum(len(v['keyframes']) for v in a['animators'].values())}")
    print(f"  → {out.relative_to(REPO) if out.is_relative_to(REPO) else out} ({out.stat().st_size} B)")


if __name__ == "__main__":
    main()
