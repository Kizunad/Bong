#!/usr/bin/env python3
"""逐帧检查单手剑的刃口朝向：刃线（模型 z 轴）是否与挥击方向平行。

输入：player_animation JSON（只读 JSON 里的关键帧，不依赖生成器）。
做法（与游戏的手持链路一致，见 docs/player-animation-conventions.md §10 与 render_block_figure.py）：
  1. 逐 tick（含半 tick）用 PlayerAnimator 的插值（bbmodel_maker.rig.emote_anim）取 rightArm / rightItem 姿态；
  2. 手（挂点）世界位置 = R_arm·(pivot + B·(P·t − c) + c)，前臂弯折 B 绕肘中心 c；
  3. 挥击方向 = 相邻采样点的手位移方向（速度低于阈值的静止段不评）；
  4. 模型的刃线轴有两种来源，必须分开看：
       - **bbmodel**（预览工具画的、用户审阅看的画面）：刃线沿模型 x 轴，刀面法线是 z 轴；
       - **OBJ**（游戏加载的 bone_sword_v2.obj）：刃线沿模型 z 轴，刀面法线是 x 轴。
     两者刀身相对护手转了 90°，见 model-review/item-anim-batch1.md「模型来源问题」。
     M = R_arm·B·P·R_item·Rd；world 轴 = M·{x̂,ŷ,ẑ}（ŷ 为刃长，朝上为正）。
  5. 判据（用 --edge-axis 指定，默认 x = bbmodel 画面）：挥击段（速度 > 阈值）内，
     |刃线·挥击方向| 的最小值 ≥ ALIGN_MIN，且刀面法线与挥击方向的 |cos| 最大值 ≤ FACE_MAX。
     同时报告另一种来源的刃线对齐，作为游戏 OBJ 的参考。

用法：
    python3 client/tools/check_blade_edge.py <player_animation.json> --display <bone_sword_v2.json>
    输出逐帧表格 + 判据结论；退出码 0 = 通过，1 = 不通过。
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path

import numpy as np

LIB = Path(__file__).resolve().parents[2] / "modelScript"
sys.path.insert(0, str(LIB))
from bbmodel_maker.rig.emote_anim import AXIS_NAMES, sample_axis  # noqa: E402

META_KEYS = frozenset({"tick", "easing", "comment", "turn"})
ALIGN_MIN = 0.9          # 挥击段刃线与挥击方向的 |cos| 下限（约 25°）
SPEED_MIN = 1.0          # 手速低于此值（像素/tick）视为静止，不评
FACE_MAX = 0.5           # 刀面法线与挥击方向的 |cos| 上限：刀面不能正对挥击方向

def rx(a):
    c, s = math.cos(a), math.sin(a)
    return np.array([[1, 0, 0], [0, c, -s], [0, s, c]], float)

def ry(a):
    c, s = math.cos(a), math.sin(a)
    return np.array([[c, 0, s], [0, 1, 0], [-s, 0, c]], float)

def rz(a):
    c, s = math.cos(a), math.sin(a)
    return np.array([[c, -s, 0], [s, c, 0], [0, 0, 1]], float)

def zyx(pitch, yaw, roll):
    return rz(roll) @ ry(yaw) @ rx(pitch)

def rot_axis(axis, ang):
    axis = np.asarray(axis, float)
    axis = axis / np.linalg.norm(axis)
    K = np.array([[0, -axis[2], axis[1]], [axis[2], 0, -axis[0]], [-axis[1], axis[0], 0]])
    return np.eye(3) + math.sin(ang) * K + (1 - math.cos(ang)) * (K @ K)

# 手持链路常量（右手）：P = Rx(-90°)·Ry(180°)，挂点 t = (+1, +2, -10) px，肩枢轴 (-5, 2, 0)，肘中心 (-1, 4, 0)
P_PRE = rx(-math.pi / 2) @ ry(math.pi)
T_ATT = np.array([1.0, 2.0, -10.0])
PIVOT_R = np.array([-5.0, 2.0, 0.0])
ELBOW_R = np.array([-1.0, 4.0, 0.0])
BLOCK_PX = 16.0


def keyframes(emote: dict) -> dict:
    kfs: dict = {}
    for move in emote["moves"]:
        tick = int(move["tick"])
        easing = move.get("easing", "linear")
        for key, val in move.items():
            if key in META_KEYS or not isinstance(val, dict):
                continue
            for axis, value in val.items():
                if axis in AXIS_NAMES:
                    kfs.setdefault(key, {}).setdefault(axis, []).append((tick, float(value), easing))
    for part in kfs.values():
        for track in part.values():
            track.sort(key=lambda t: t[0])
    return kfs


def sample(kfs, part: str, tick: float) -> dict:
    return {ax: sample_axis(kfs, part, ax, tick) for ax in ("pitch", "yaw", "roll", "bend", "axis")}


def display_matrix(display_json: Path):
    """读物品 display 的 thirdperson_righthand：返回 (Rd 旋转矩阵, 缩放, 平移 px)。"""
    disp = json.loads(display_json.read_text(encoding="utf-8"))["display"]["thirdperson_righthand"]
    rx_, ry_, rz_ = (math.radians(v) for v in disp["rotation"])
    rd = rx(rx_) @ ry(ry_) @ rz(rz_)
    scale = float(disp["scale"][0])
    trans = np.array(disp["translation"], float)
    return rd, scale, trans


def frame_axes(kfs, tick, rd):
    arm = sample(kfs, "rightArm", tick)
    item = sample(kfs, "rightItem", tick)
    r_arm = zyx(arm["pitch"], arm["yaw"], arm["roll"])
    b = rot_axis([math.cos(arm["axis"]), 0.0, math.sin(arm["axis"])], arm["bend"])
    r_item = zyx(item["pitch"], item["yaw"], item["roll"])
    m = r_arm @ b @ P_PRE @ r_item @ rd
    hand = r_arm @ (PIVOT_R + b @ (P_PRE @ T_ATT - ELBOW_R) + ELBOW_R)
    return m[:, 0], m[:, 1], m[:, 2], hand


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("anim", type=Path, help="player_animation JSON")
    ap.add_argument("--display", type=Path, required=True, help="物品模型 JSON（取 thirdperson_righthand）")
    ap.add_argument("--strike", default="", help="挥击段 tick 范围 a-b（缺省用全部有手速的帧）")
    ap.add_argument("--edge-axis", choices=("x", "z"), default="x",
                    help="判据用的刃线轴：x = bbmodel 画面（默认），z = 游戏 OBJ")
    args = ap.parse_args()

    emote = json.loads(args.anim.read_text(encoding="utf-8"))["emote"]
    if emote.get("degrees", True):
        raise SystemExit("emote.degrees 必须为 false")
    kfs = keyframes(emote)
    rd, scale, _ = display_matrix(args.display)  # 方向只用 Rd；缩放与平移不影响方向
    end = float(emote["endTick"])

    samples = [i / 2 for i in range(int(end * 2) + 1)]
    frames = {t: frame_axes(kfs, t, rd) for t in samples}
    lo, hi = (float(v) for v in args.strike.split("-")) if args.strike else (0.0, end)

    edge_i = {"x": 0, "z": 2}[args.edge_axis]
    flat_i = 2 if args.edge_axis == "x" else 0
    other_i = 0 if args.edge_axis == "z" else 2   # 另一种来源的刃线轴（x↔z），只作参考
    worst = 1.0
    worst_face = 0.0
    worst_other = 1.0
    print(f"{'tick':>5} {'speed':>6} {'edge|·v|':>9} {'flat|·v|':>9} {'ref|·v|':>8} {'up(-y)':>7}  note")
    for i, t in enumerate(samples):
        if i == 0 or i == len(samples) - 1:
            continue
        prev_hand = frames[samples[i - 1]][3]
        next_hand = frames[samples[i + 1]][3]
        vel = next_hand - prev_hand                      # 两个半 tick 的位移
        speed = float(np.linalg.norm(vel)) / 2.0         # 像素 / tick
        axes = frames[t]
        up = -float(axes[1][1])
        if speed < SPEED_MIN:
            print(f"{t:5.1f} {speed:6.2f} {'-':>9} {'-':>9} {'-':>8} {up:+7.2f}  静止，不评")
            continue
        v = vel / np.linalg.norm(vel)
        align = abs(float(axes[edge_i] @ v))
        face = abs(float(axes[flat_i] @ v))
        other = abs(float(axes[other_i] @ v))
        in_strike = lo <= t <= hi
        if in_strike:
            worst = min(worst, align)
            worst_face = max(worst_face, face)
            worst_other = min(worst_other, other)
        note = "挥击段" if in_strike else ""
        print(f"{t:5.1f} {speed:6.2f} {align:9.3f} {face:9.3f} {other:8.3f} {up:+7.2f}  {note}")

    ok = worst >= ALIGN_MIN and worst_face <= FACE_MAX
    print()
    print(f"挥击段（{lo:g}-{hi:g} tick，手速 ≥ {SPEED_MIN} px/tick）：")
    print(f"  判据刃线轴 = 模型 {args.edge_axis} 轴（{'bbmodel 画面' if args.edge_axis == 'x' else '游戏 OBJ'}）")
    print(f"  刃线与挥击方向 |cos| 最小值 = {worst:.3f}（要求 ≥ {ALIGN_MIN}）")
    print(f"  刀面法线与挥击方向 |cos| 最大值 = {worst_face:.3f}（要求 ≤ {FACE_MAX}）")
    print(f"  参考：另一种来源（模型 {'z' if args.edge_axis == 'x' else 'x'} 轴）刃线 |cos| 最小值 = {worst_other:.3f}")
    print("  结论：" + ("通过" if ok else "不通过"))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
