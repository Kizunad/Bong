#!/usr/bin/env python3
"""sword_cleave_fpv —— sword.cleave 的第一人称变体（plan-fpv-cast-av-v1 P2）。

目标：贴脸视角下双手**交叉合握**剑柄沿中线劈砍。round 1 真机反馈"两手没握到
一起"；round 2 排查发现根因不在左手偏移量——TPV 右臂全程在右肩矢状面
（x≈-5）里挥，发力顶点右拳甩到右胯后下方，离左肩球面（臂长 12）之外，
左手**物理不可达**（IK 残差 9.2 模型单位实证）。真·双手劈砍 = 挥击平面收
到身体中线，两只手都要动：

  1. 右臂中线校正：pitch/bend（挥击弧线与发力时序）原样继承 TPV，只解
     yaw/roll 两轴把右拳钉到中线面（x=MIDLINE_X），强正则贴原姿态。
  2. 左臂 IK 合握：解 pitch/yaw/roll/bend 四轴，左拳钉到「校正后右拳沿前臂
     轴往柄尾退 GRIP_STACK」的握把点——两腕交叠 = 交叉合握。

全部离线烘焙（MC 运行时无 IK，conventions §9）：复用 render_animation.py 的
正向运动学（PIVOTS / part_rotation_matrix / bent_end_local，v8 真机对拍过），
解出的角度写回普通关键帧，运行时零额外成本。头/躯干/腿继承 TPV，`body.*`
位移减半防相机晃（conventions §16.1）。维护（§16.2）：TPV 改了挥击轴/时序，
重跑本生成器即可——两臂逐帧重解自动跟随。

求解器：坐标下降 + 步长减半（目标函数便宜，无需 scipy）。左臂多起点
（前帧解 / 当帧 TPV 左臂）防局部极小（round 2 初版单起点热启动在 t13→t16
快速下挥段被卡在举臂解上）；帧间正则贴前帧解保时间连贯；右臂姿态相同的帧
（t0 guard / t20 收势回 guard）复用同一解保首尾闭合。
"""

from __future__ import annotations

import copy
import math

import gen_sword_cleave as tpv
import numpy as np
from anim_common import emit_json
from render_animation import (
    PIVOTS,
    bend_center,
    bent_end_local,
    part_rotation_matrix,
)

BODY_DISP_SCALE = 0.5  # FPV body 位移减半防晃（conventions §16.1）
MIDLINE_X = -0.8  # 目标挥击平面：略偏右的中线（右手主握的自然偏置）
GRIP_STACK = 1.6  # 左拳压在右拳柄尾侧的距离（沿右前臂轴往肘方向，模型单位）
AXIS_FIXED = 180.0  # 小臂向体前折（同 TPV 全族约定）

# 左臂求解边界：肘不反关节（bend ≥ 0）、roll 不拧断腕、pitch 覆盖举剑过头到劈落。
LEFT_BOUNDS = {
    "pitch": (-175.0, 85.0),
    "yaw": (-85.0, 85.0),
    "roll": (-100.0, 100.0),
    "bend": (0.0, 115.0),
}
LEFT_KEYS = ("pitch", "yaw", "roll", "bend")
W_LEFT_PREV = 0.0005  # 帧间正则：Δ40° ≈ 0.9 模型单位误差的代价，压解跳变不挡够靶

# 右臂校正边界：相对 TPV 原值的最大偏移（挥击主轴 pitch/bend 不动）。
RIGHT_KEYS = ("yaw", "roll")
RIGHT_MAX_DEV = {"yaw": 55.0, "roll": 65.0}
W_RIGHT_AUTH = 0.002  # 强正则贴 TPV 原姿态：中线优先，偏移最小化


def _arm_points(part: str, pose_deg: dict) -> tuple[np.ndarray, np.ndarray]:
    """body 帧内 (肘, 拳端) 位置。双臂共享 body 变换，作相对目标时可整体略去。"""
    pivot = np.array(PIVOTS[part], dtype=np.float64)
    rot = part_rotation_matrix(
        math.radians(pose_deg.get("pitch", 0.0)),
        math.radians(pose_deg.get("yaw", 0.0)),
        math.radians(pose_deg.get("roll", 0.0)),
    )
    hand_local = bent_end_local(
        part,
        math.radians(pose_deg.get("axis", 0.0)),
        math.radians(pose_deg.get("bend", 0.0)),
    )
    return pivot + rot @ bend_center(part), pivot + rot @ hand_local


def _descend(energy, params: dict, clamp) -> dict:
    """坐标下降 + 步长减半。energy(params)->float，clamp(key,val)->val。"""
    best = energy(params)
    step = 24.0
    while step > 0.05:
        improved = False
        for k in params:
            for sign in (1.0, -1.0):
                cand = dict(params)
                cand[k] = clamp(k, cand[k] + sign * step)
                e = energy(cand)
                if e < best - 1e-9:
                    params, best = cand, e
                    improved = True
        if not improved:
            step *= 0.5
    return params


def solve_right(auth: dict) -> tuple[dict, float]:
    """右臂中线校正：只解 yaw/roll 把拳端拉到 x=MIDLINE_X，返回 (完整姿态, 残差)。

    近垂直举臂段 yaw 对 x 失效（绕臂轴自旋），roll 补位——两轴联合覆盖全程。
    """
    _, auth_hand = _arm_points("rightArm", auth)
    target_x = MIDLINE_X

    def clamp(k: str, v: float) -> float:
        base = float(auth.get(k, 0.0))
        dev = RIGHT_MAX_DEV[k]
        return min(base + dev, max(base - dev, v))

    def full(p: dict) -> dict:
        return {**auth, "yaw": p["yaw"], "roll": p["roll"]}

    def energy(p: dict) -> float:
        _, hand = _arm_points("rightArm", full(p))
        # x 钉中线为主；y/z 弱贴原轨迹（防 roll 把拳甩离挥击弧线）
        e = (hand[0] - target_x) ** 2
        e += 0.15 * ((hand[1] - auth_hand[1]) ** 2 + (hand[2] - auth_hand[2]) ** 2)
        e += W_RIGHT_AUTH * sum(
            (p[k] - float(auth.get(k, 0.0))) ** 2 for k in RIGHT_KEYS
        )
        return e

    seed = {k: float(auth.get(k, 0.0)) for k in RIGHT_KEYS}
    solved = _descend(energy, seed, clamp)
    result = full(solved)
    _, hand = _arm_points("rightArm", result)
    return result, abs(float(hand[0]) - target_x)


def grip_target(right_pose: dict) -> np.ndarray:
    """握把目标点：右拳沿前臂轴往柄尾（肘方向）退 GRIP_STACK——左拳叠在右腕上。"""
    elbow, hand = _arm_points("rightArm", right_pose)
    fore = hand - elbow
    n = fore / (np.linalg.norm(fore) + 1e-9)
    return hand - GRIP_STACK * n


def solve_left(target: np.ndarray, seeds: list, prev: dict) -> tuple[dict, float]:
    """左臂四轴 IK：多起点坐标下降，取最优解。prev = 帧间正则锚点（度）。"""

    def clamp(k: str, v: float) -> float:
        lo, hi = LEFT_BOUNDS[k]
        return min(hi, max(lo, v))

    def hand_dist2(p: dict) -> float:
        _, hand = _arm_points("leftArm", {**p, "axis": AXIS_FIXED})
        diff = hand - target
        return float(diff @ diff)

    def energy(p: dict) -> float:
        return hand_dist2(p) + W_LEFT_PREV * sum(
            (p[k] - prev.get(k, 0.0)) ** 2 for k in LEFT_KEYS
        )

    best_params, best_e = None, math.inf
    for seed in seeds:
        params = {k: clamp(k, float(seed.get(k, 0.0))) for k in LEFT_KEYS}
        params = _descend(energy, params, clamp)
        e = energy(params)
        if e < best_e:
            best_params, best_e = params, e
    return best_params, math.sqrt(hand_dist2(best_params))


def build_pose_table() -> dict:
    out = {}
    solved_cache: dict[tuple, tuple[dict, dict]] = {}
    prev_left = {k: float(tpv.GUARD["leftArm"].get(k, 0.0)) for k in LEFT_KEYS}
    print("tick |  right yaw/roll (auth->solved)  x-resid | left pitch/yaw/roll/bend  resid")
    for tick in sorted(tpv.POSE.keys()):
        p = copy.deepcopy(tpv.POSE[tick])
        body = p.get("body")
        if body:  # body 位移减半（防相机晃）
            for ax in ("x", "y", "z"):
                if ax in body:
                    body[ax] *= BODY_DISP_SCALE
        auth_right = p["rightArm"]
        key = tuple(
            round(float(auth_right.get(k, 0.0)), 3)
            for k in ("pitch", "yaw", "roll", "bend", "axis")
        )
        if key in solved_cache:  # 右臂姿态相同 → 复用解（t0/t20 首尾闭合）
            right, left = solved_cache[key]
            print(f"{tick:4d} | (cached)")
        else:
            right, rx = solve_right(auth_right)
            seeds = [prev_left, p["leftArm"]]  # 前帧解 + 当帧 TPV 左臂（防局部极小）
            left, lres = solve_left(grip_target(right), seeds, prev_left)
            solved_cache[key] = (right, left)
            print(
                f"{tick:4d} | y {float(auth_right.get('yaw', 0)):6.1f}->{right['yaw']:6.1f}"
                f"  r {float(auth_right.get('roll', 0)):6.1f}->{right['roll']:6.1f}"
                f"  {rx:5.2f} | "
                f"{left['pitch']:6.1f}/{left['yaw']:6.1f}/{left['roll']:6.1f}/{left['bend']:5.1f}"
                f"  {lres:5.2f}"
            )
        prev_left = left
        p["rightArm"] = {
            "pitch": float(auth_right["pitch"]),
            "yaw": round(float(right["yaw"]), 1),
            "roll": round(float(right["roll"]), 1),
            "bend": float(auth_right["bend"]),
            "axis": AXIS_FIXED,
        }
        p["leftArm"] = {
            "pitch": round(left["pitch"], 1),
            "yaw": round(left["yaw"], 1),
            "roll": round(left["roll"], 1),
            "bend": round(left["bend"], 1),
            "axis": AXIS_FIXED,
        }
        out[tick] = p
    return out


def main() -> int:
    emit_json(
        build_pose_table(),
        name="sword_cleave_fpv",
        description=(
            "sword.cleave 第一人称变体：挥击平面收到身体中线的双手交叉合握劈砍——"
            "右臂 yaw/roll 中线校正（pitch/bend 挥击弧线继承 TPV），左臂逐关键帧"
            "离线 IK 钉到右拳柄尾握把点，body 位移减半防晃"
            "（plan-fpv-cast-av-v1 P2 round 2）。"
        ),
        end_tick=20,
        stop_tick=22,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
