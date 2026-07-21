#!/usr/bin/env python3
"""sword_cleave_fpv —— sword.cleave 的第一人称变体（plan-fpv-cast-av-v1 P2）。

只调左手，让双手在贴脸视角合到剑柄（右手握剑=剑柄基准，左手并上去）。右臂 / 头 /
躯干继承 TPV（gen_sword_cleave），`body.*` 位移减半防相机晃（conventions §16.1），
只做腰以上（相机外的腿不做无效帧）。

维护约定（conventions §16.2）：TPV `gen_sword_cleave.py` 改了主打击轴/时序，必须连带
复核本文件（本脚本 import TPV 的 POSE，右臂时序自动跟随；左手 clasp 参数需人工复核）。

左手 clasp 推导（右臂定剑柄，左手够过去；MC 无 IK，手调三轴 + bend）：
  pitch ← 跟右臂同高（+微降，左手叠在右手下方握把）
  yaw   ← 从左肩(+5,2,0) 往中线/右侧(-X) 拉过去，够到右手
  bend  ← 左肩离剑柄比右肩远，多折小臂把手缩到剑柄（够不到伸、过了折）
  roll  ← 顺腕，取右臂 roll 反号
"""

from __future__ import annotations

import copy

import gen_sword_cleave as tpv
from anim_common import emit_json

# 左手 clasp 参数（相对右臂的偏移；render_animation.py --fpv 迭代后定稿）。
LEFT_PITCH_DROP = 6.0  # 左手比右手略低，叠在握把下段
LEFT_YAW_CROSS = -26.0  # 从左肩往中线右侧横跨够到剑柄（负=player 右）
LEFT_BEND_ADD = 20.0  # 左臂够得远，多折小臂缩回剑柄
BODY_DISP_SCALE = 0.5  # FPV body 位移减半防晃


def clasp_left_from_right(right: dict) -> dict:
    """给定右臂姿态，推左手并到剑柄的姿态。"""
    return dict(
        pitch=right["pitch"] + LEFT_PITCH_DROP,
        yaw=right.get("yaw", 0.0) + LEFT_YAW_CROSS,
        roll=-right.get("roll", 0.0),
        bend=right["bend"] + LEFT_BEND_ADD,
        axis=180,
    )


def fpv_pose(tpv_pose: dict) -> dict:
    p = copy.deepcopy(tpv_pose)
    # body 位移减半（防相机晃）
    body = p.get("body")
    if body:
        for ax in ("x", "y", "z"):
            if ax in body:
                body[ax] *= BODY_DISP_SCALE
    # 左手改写为 clasp 右手（右臂原样保留）
    if "rightArm" in p:
        p["leftArm"] = clasp_left_from_right(p["rightArm"])
    return p


def build_pose_table() -> dict:
    out = {}
    for tick, pose in tpv.POSE.items():
        # tpv.POSE[20] 是 inherit(GUARD)（已是完整 dict），其余是普通 dict——统一深拷贝改写。
        out[tick] = fpv_pose(pose)
    return out


def main() -> int:
    emit_json(
        build_pose_table(),
        name="sword_cleave_fpv",
        description=(
            "sword.cleave 第一人称变体：右臂/头/躯干继承 TPV，左手并到剑柄（双手合握），"
            "body 位移减半防晃。腰以上贴脸视角专用（plan-fpv-cast-av-v1 P2）。"
        ),
        end_tick=20,
        stop_tick=22,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
