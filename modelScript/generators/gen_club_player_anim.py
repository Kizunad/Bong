#!/usr/bin/env python3
"""ClubPlayerAnim.bbmodel —— 玩家 + 木棍 + 内嵌 Blockbench 动画，在 Blockbench 里直接调。

`club_smash` / `club_sweep` 两条动画烘成 Blockbench animation，**逐部位分组**，可以在
Animate 模式里播放、拖关键帧、改姿态。骨架结构与 `gen_jian_player_anim.py` 同构（那份
是双锏版），这里换成木棍、并补上 `body.yaw` 那一层。

## 骨架（**每个轴一层 group**，这是硬约定不是洁癖）

    root_pos                                   ← body 位移
    └ root_roll → root_yaw → root_pitch        ← body 整体旋转（本项目只用到 yaw）
      ├ head_roll → head_yaw → head_pitch
      ├ torso_roll → …_yaw → …_pitch → torso_bend
      ├ arm_right_roll → …_yaw → …_pitch → arm_right_bend
      │                                        └ club_right_roll → club_right_pitch → 木棍
      ├ arm_left_…（同上）
      └ leg_right_… / leg_left_…（同上）

MC 的 ModelPart 是 `rotationZYX(roll, yaw, pitch)`（先绕 X、再 Y、再 Z 作用到向量），
Blockbench 的 bone 走 THREE.js Euler，多轴同时非零时组合顺序未必一致——木棍这两条动画
手臂 pitch/yaw/roll **三轴都大**（横抡的 guard 是 +5/+50/+56），顺序一反手就甩到身体另一
侧。拆成嵌套单轴（内 pitch → 中 yaw → 外 roll，与 ZYX 的作用次序一致）之后顺序无从产生
歧义，两边必然一致。

Blockbench 没有 bend 概念（那是 bendy-lib 在渲染期折 cuboid 顶点），所以每个可弯部位再
拆一层「下段 group，pivot 落在 cuboid 几何中心」，肘/膝才有得看。

## 角度换算（改完姿态要写回 `client/tools/gen_club_*.py` 时按这个反推）

    静态 group.rotation 和动画 keyframe **同一套**（都是标准右手系）:
        bb.x = -pitch,  bb.y = +yaw,  bb.z = -roll
    body 位移: bb = (x, -y, z) × 16      （米 → px，只翻 y）

第一版按锏那份的注释写成了「动画通道 X/Y 取反」，结果 Blockbench 里看到的是 pitch/yaw
双双镜像的姿态。修正的依据是一次真实往返，详见 `AXIS_LAYERS` 那段注释。

## 木棍怎么挂在手上

沿用锏那套：木棍几何整体平移到「握把点落在手心 `HAND_REST`」，再套两层静态 group
（`club_right_pitch` 180° 把棍尖从朝上翻成顺着小臂朝下、`club_right_roll` 备用），挂在
小臂 bend 段之下——肘一弯棍跟着走。

**这不是游戏里的 display 变换**（那套是 `rotation [-80, 90, 0]` + 方块中心重定心，见
`held_item_common.hand_display`），而是一个便于手调的近似：两层静态 group 就是留给你改
握角的。要看真实手持姿态用
`modelScript/tools/preview_player_anim.py --hold ... --display ...`。

## 插值

Blockbench 只有 linear/catmullrom/step/bezier，这里写 linear——它忠实于关键帧数值，不会
引入原动画没有的过冲。游戏里的真实 easing 是 OUTSINE/INCUBIC/OUTQUAD（`club_smash` 的
重量感有一半来自它们），**别拿 Blockbench 的播放去评判缓动手感**。

## ⚠ 手改过就别再跑生成器

这份是**给人手调的**。你在 Blockbench 里改完存盘，文件会变成 `format_version 5.0`；
重跑本脚本会**整份覆盖**，改动全没（棺材那批踩过，见 memory `project_coffin_models`）。
改完之后把新数值反推回 `client/tools/gen_club_smash.py` / `gen_club_sweep.py` 的 POSE 表
（换算见上一节），生成器和资产才不会分叉。

用法:
    python3 modelScript/generators/gen_club_player_anim.py
    python3 modelScript/generators/gen_club_player_anim.py --anims club_smash
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
from gen_jian_player_anim import (  # noqa: E402  骨架/关键帧的公共件，别再抄一份
    PART_GROUPS,
    TICKS_PER_SECOND,
    _uuid,
    cube_element,
    group,
    keyframe,
    split_cubes,
)

# 轴换算全部走 `core/bb_anim_axes`——那里是唯一一处，且带着"怎么测出来的"证据。
REPO = LIB.parent
ANIM_DIR = REPO / "client" / "src" / "main" / "resources" / "assets" / "bong" / "player_animation"
SRC_CLUB = LIB / "models" / "WoodenClub.bbmodel"
OUT_BB = LIB / "models" / "ClubPlayerAnim.bbmodel"

DEFAULT_ANIMS = ["club_smash", "club_sweep"]

# 木棍 bbmodel 是**出料系**：`emit_offset` 已把握把点放到方块中心 (8,8,8)px，
# 棍尖朝 +Y。挂到手上就是把这一点搬到 HAND_REST。
CLUB_GRIP_PX = np.array([8.0, 8.0, 8.0])
# 只挂右手：两条动画都是右手持棍（左手是活的副手，不持械）。
SIDE = "right"


def load_club():
    """读木棍 bbmodel，返回 (elements, 贴图, 贴图宽高)。"""
    doc = json.loads(SRC_CLUB.read_text(encoding="utf-8"))
    tex = doc["textures"][0]
    image = Image.open(io.BytesIO(base64.b64decode(
        tex["source"].split(",", 1)[1]))).convert("RGBA")
    return doc["elements"], image, (int(tex["width"]), int(tex["height"]))


def build_geometry():
    """→ (elements, outliner, group_uuid_by_name, atlas)。"""
    club_elements, club_tex, (club_w, club_h) = load_club()
    atlas = Image.new("RGBA", (H.ATLAS, H.ATLAS), (0, 0, 0, 0))
    atlas.paste(H.make_skin(), (0, 0))
    # **不缩放**贴到图集下半：木棍图集是 48×16 的窄条，缩成 64² 会把三档材质糊在一起。
    # UV 因此只需整体下移 WEAPON_V_OFF，横向原样。
    atlas.paste(club_tex, (0, H.WEAPON_V_OFF))
    if club_w > H.ATLAS or club_h + H.WEAPON_V_OFF > H.ATLAS:
        raise SystemExit(f"木棍贴图 {club_w}×{club_h} 放不进 {H.ATLAS}² 图集的下半")

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
            offset = np.array(hand, float) - CLUB_GRIP_PX
            club_ids = []
            for source in club_elements:
                el = copy.deepcopy(source)
                el["uuid"] = _uuid()
                el["name"] = f"club_{source['name']}"
                for key in ("from", "to", "origin"):
                    el[key] = [round(v + offset[i], 4) for i, v in enumerate(el[key])]
                for face in el["faces"].values():
                    u1, v1, u2, v2 = face["uv"]
                    face["uv"] = [u1, v1 + H.WEAPON_V_OFF, u2, v2 + H.WEAPON_V_OFF]
                elements.append(el)
                club_ids.append(el["uuid"])
            # 180° 不能省：棍的局部 +Y 是「握把→棍头」，而手臂 cuboid 是从 pivot 向
            # **下**(-Y) 长的。腕角归零意味着棍头指向小臂的反方向（朝肘上方），必须绕 X
            # 翻 180° 才与小臂同向。静态 group.rotation 走标准右手系（不吃动画通道的
            # Bedrock 取反），所以直接写 180。
            pitch_g = group(f"club_{side}_pitch", hand, club_ids, (180.0, 0.0, 0.0), color=1)
            roll_g = group(f"club_{side}_roll", hand, [pitch_g], (0.0, 0.0, 0.0), color=1)
            gmap[f"club_{side}_pitch"] = pitch_g["uuid"]
            gmap[f"club_{side}_roll"] = roll_g["uuid"]
            # 挂在小臂（bend 段）之下：肘一弯，棍跟着走
            (bend_group or top)["children"].append(roll_g)
        arms.append(top)

    # body：位移一层 + 三层单轴旋转。锏那份只做了 pitch（步态用不到别的），木棍这两条
    # 动画的**站架恒定 body.yaw**（抡砸 −16、横抡 −24），缺了 yaw 层整个人就不侧身了。
    node = [head, torso] + arms + legs
    for axis_name in ("pitch", "yaw", "roll"):
        g = group(f"root_{axis_name}", (0.0, 0.0, 0.0), node, color=3)
        gmap[f"root_{axis_name}"] = g["uuid"]
        node = [g]
    root_pos = group("root_pos", (0.0, 0.0, 0.0), node, color=3)
    gmap["root_pos"] = root_pos["uuid"]
    return elements, [root_pos], gmap, atlas


def convert_animation(json_path: Path, gmap: dict) -> dict:
    name, emote, table = P.anim_pose_table(json_path)
    animators: dict[str, dict] = {}

    def track(group_name):
        gid = gmap[group_name]
        animators.setdefault(gid, {"name": group_name, "type": "bone", "keyframes": []})
        return animators[gid]["keyframes"]

    for tick, pose in table:
        t = tick / TICKS_PER_SECOND
        pose = dict(pose)
        body = pose.pop("_body", None)
        if body:
            track("root_pos").append(
                keyframe("position", t, AX.body_position_to_bb(body)))
            for axis_name in AX.AXIS_ORDER:
                track(f"root_{axis_name}").append(
                    keyframe("rotation", t, AX.rotation_to_bb(body, axis_name)))
        for part, axes in pose.items():
            if part not in PART_GROUPS:
                continue
            prefix, has_bend = PART_GROUPS[part]
            for axis_name in AX.AXIS_ORDER:
                track(f"{prefix}_{axis_name}").append(
                    keyframe("rotation", t, AX.rotation_to_bb(axes, axis_name)))
            # **part 级位移也要烘**。锏那份只烘旋转，于是腿的 z（步幅前后错开，
            # ±0.05~0.10 格 = 0.8~1.6px）在 bbmodel 里整个丢了——`bbmodel_to_pose --diff`
            # 会把它当成"人改过"一路报出来，是个永久的假阳性。
            if any(abs(axes.get(k, 0.0)) > 1e-9 for k in "xyz"):
                track(f"{prefix}_{AX.AXIS_ORDER[-1]}").append(
                    keyframe("position", t, AX.body_position_to_bb(axes)))
            if has_bend:
                track(f"{prefix}_bend").append(
                    keyframe("rotation", t,
                             [round(AX.bend_to_bb(axes.get("bend", 0.0),
                                                  axes.get("axis", 0.0)), 4), 0.0, 0.0]))

    return {
        "uuid": _uuid(), "name": name,
        "loop": "loop" if emote.get("isLoop") else "once",
        "override": False, "length": round(emote["endTick"] / TICKS_PER_SECOND, 4),
        "snapping": int(TICKS_PER_SECOND), "selected": False, "saved": True, "path": "",
        "anim_time_update": "", "blend_weight": "", "start_delay": "", "loop_delay": "",
        "animators": animators,
    }


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
        animations.append(convert_animation(path, gmap))

    buf = io.BytesIO()
    atlas.save(buf, format="PNG")
    model = {
        "meta": {"format_version": "4.10", "model_format": "free", "box_uv": False},
        "name": "club_player_anim",
        "model_identifier": "geometry.bong.club_player_anim",
        "visible_box": [3.0, 3.0, 2.0],
        "resolution": {"width": H.ATLAS, "height": H.ATLAS},
        "elements": elements, "outliner": outliner, "animations": animations,
        "textures": [{
            "path": "", "name": "club_player_anim.png", "folder": "item", "namespace": "bong",
            "id": "0", "width": H.ATLAS, "height": H.ATLAS,
            "uv_width": H.ATLAS, "uv_height": H.ATLAS,
            "particle": False, "render_mode": "default", "visible": True, "mode": "bitmap",
            "saved": False, "uuid": _uuid(),
            "source": "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode(),
        }],
    }
    out = Path(args.out)
    out.write_text(json.dumps(model, ensure_ascii=False, indent=1), encoding="utf-8")
    print(f"ClubPlayerAnim: {len(elements)} elements / {len(animations)} animations")
    for a in animations:
        frames = sum(len(v["keyframes"]) for v in a["animators"].values())
        print(f"  {a['name']:12s} {a['length']:.2f}s {a['loop']:5s} "
              f"bones={len(a['animators'])} keyframes={frames}")
    print(f"  → {out.relative_to(REPO) if out.is_relative_to(REPO) else out} "
          f"({out.stat().st_size} B)")


if __name__ == "__main__":
    main()
