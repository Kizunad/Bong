#!/usr/bin/env python3
"""sword_cleave_fpv —— sword.cleave 的第一人称变体（plan-fpv-cast-av-v1 P2）。

目标：贴脸视角下双手**交叉合握**剑柄沿中线劈砍，且**全程**（含关键帧之间的
快速劈击插值段）不脱手。

演进：
  round 1 固定偏移 → 两手没合上（左手物理够不到右拳）。
  round 2 双臂离线 IK（右臂 yaw/roll 中线校正 + 左臂四轴 IK 合握）→ 关键帧上
    合握，但 t13→t16 快速劈击段左臂配置差 152°，MC 线性插关节角时手端画圆弧
    中途向左鼓出（三视图实证：t14/t15 左拳甩到体侧）。
  round 3（本版）加密左臂锚点：左臂**每 tick 一个 IK 关键帧**，插值段 ≤1 tick，
    手端来不及偏离；右臂/头/躯干/腿仍只保留原 TPV 关键帧（连带缓动 easing），
    保挥击质感不被拍平。中间 tick 的左手握把目标 = 对**已校正右臂**按 MC 真实
    缓动（destination 帧的 easing）插值后 FK，与运行时右手轨迹逐帧对齐。

全部生成期离线烘焙（MC 运行时无 IK，conventions §9）：复用 render_animation.py
的正向运动学（PIVOTS / part_rotation_matrix / bent_end_local，v8 真机对拍过）。
`body.*` 位移减半防相机晃（conventions §16.1）。维护（§16.2）：TPV 改了挥击轴/
时序，重跑本生成器即可——两臂逐帧重解自动跟随。
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


# ----- 缓动（对齐 MC/Emotecraft 对 destination 帧应用 easing）----------------

def _ease(name: str, a: float) -> float:
    n = (name or "linear").upper()
    if n in ("LINEAR", "CONSTANT"):
        return a
    if n == "INSINE":
        return 1.0 - math.cos(a * math.pi / 2)
    if n == "OUTSINE":
        return math.sin(a * math.pi / 2)
    if n == "INOUTSINE":
        return -(math.cos(math.pi * a) - 1) / 2
    if n == "INQUAD":
        return a * a
    if n == "OUTQUAD":
        return 1 - (1 - a) ** 2
    if n == "INOUTQUAD":
        return 2 * a * a if a < 0.5 else 1 - (-2 * a + 2) ** 2 / 2
    if n == "INCUBIC":
        return a ** 3
    if n == "OUTCUBIC":
        return 1 - (1 - a) ** 3
    return a  # 未知缓动退化为线性


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


def solve_right(auth: dict) -> dict:
    """右臂中线校正：只解 yaw/roll 把拳端拉到 x=MIDLINE_X，返回完整校正姿态（度）。

    近垂直举臂段 yaw 对 x 失效（绕臂轴自旋），roll 补位——两轴联合覆盖全程。
    """
    _, auth_hand = _arm_points("rightArm", auth)

    def clamp(k: str, v: float) -> float:
        base = float(auth.get(k, 0.0))
        dev = RIGHT_MAX_DEV[k]
        return min(base + dev, max(base - dev, v))

    def full(p: dict) -> dict:
        return {**auth, "yaw": p["yaw"], "roll": p["roll"]}

    def energy(p: dict) -> float:
        _, hand = _arm_points("rightArm", full(p))
        e = (hand[0] - MIDLINE_X) ** 2
        e += 0.15 * ((hand[1] - auth_hand[1]) ** 2 + (hand[2] - auth_hand[2]) ** 2)
        e += W_RIGHT_AUTH * sum(
            (p[k] - float(auth.get(k, 0.0))) ** 2 for k in RIGHT_KEYS
        )
        return e

    seed = {k: float(auth.get(k, 0.0)) for k in RIGHT_KEYS}
    return full(_descend(energy, seed, clamp))


def grip_target(right_pose: dict) -> np.ndarray:
    """握把目标点：右拳沿前臂轴往柄尾（肘方向）退 GRIP_STACK——左拳叠在右腕上。"""
    elbow, hand = _arm_points("rightArm", right_pose)
    fore = hand - elbow
    n = fore / (np.linalg.norm(fore) + 1e-9)
    return hand - GRIP_STACK * n


def solve_left(
    target: np.ndarray, seeds: list, prev: dict, w_prev: float = W_LEFT_PREV
) -> tuple[dict, float]:
    """左臂四轴 IK：多起点坐标下降，取最优解。prev = 帧间正则锚点（度）。

    w_prev 控连续性正则强度：锚点用默认（弱压解跳变）；加密的中间帧用近零权重
    （平滑改由每 tick 一锚点的密度保证，正则再拉会把解拽离握把靶——快速段实测能
    偏 ~3 模型单位），只靠起点选 basin 维持配置连续。
    """

    def clamp(k: str, v: float) -> float:
        lo, hi = LEFT_BOUNDS[k]
        return min(hi, max(lo, v))

    def hand_dist2(p: dict) -> float:
        _, hand = _arm_points("leftArm", {**p, "axis": AXIS_FIXED})
        diff = hand - target
        return float(diff @ diff)

    def energy(p: dict) -> float:
        return hand_dist2(p) + w_prev * sum(
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


def _left_move(solved: dict) -> dict:
    return {
        "pitch": round(solved["pitch"], 1),
        "yaw": round(solved["yaw"], 1),
        "roll": round(solved["roll"], 1),
        "bend": round(solved["bend"], 1),
        "axis": AXIS_FIXED,
    }


def build_pose_table() -> dict:
    tpv_ticks = sorted(tpv.POSE.keys())
    lo, hi = tpv_ticks[0], tpv_ticks[-1]

    # ---- 一遍：解 TPV 锚点（右臂中线校正 + 左臂 IK），记右臂校正角供加密插值 ----
    anchor_pose: dict[int, dict] = {}  # 完整锚点姿态（度，body 已减半）
    corrected_right: dict[int, dict] = {}  # 校正后右臂角（度），供中间 tick 插值
    anchor_left: dict[int, dict] = {}  # 锚点左臂解（度），供中间 tick 混合起点
    prev_left = {k: float(tpv.GUARD["leftArm"].get(k, 0.0)) for k in LEFT_KEYS}
    right_cache: dict[tuple, dict] = {}
    left_cache: dict[tuple, dict] = {}  # 按右臂姿态缓存左臂解 → t20 复用 t0 保收势闭合
    print("tick |  right yaw/roll (auth->solved) | left pitch/yaw/roll/bend  resid")
    for tick in tpv_ticks:
        p = copy.deepcopy(tpv.POSE[tick])
        body = p.get("body")
        if body:  # body 位移减半（防相机晃）
            for ax in ("x", "y", "z"):
                if ax in body:
                    body[ax] *= BODY_DISP_SCALE
        auth_right = p["rightArm"]
        rkey = tuple(
            round(float(auth_right.get(k, 0.0)), 3)
            for k in ("pitch", "yaw", "roll", "bend", "axis")
        )
        right = right_cache.get(rkey) or solve_right(auth_right)
        right_cache[rkey] = right
        if rkey in left_cache:  # 右臂姿态相同 → 握把目标相同 → 复用左臂解（t0/t20 闭合）
            left, lres = left_cache[rkey], -1.0
        else:
            seeds = [prev_left, p["leftArm"]]  # 前帧解 + 当帧 TPV 左臂（防局部极小）
            left, lres = solve_left(grip_target(right), seeds, prev_left)
            left_cache[rkey] = left
        prev_left = left

        p["rightArm"] = {
            "pitch": float(auth_right["pitch"]),
            "yaw": round(float(right["yaw"]), 1),
            "roll": round(float(right["roll"]), 1),
            "bend": float(auth_right["bend"]),
            "axis": AXIS_FIXED,
        }
        p["leftArm"] = _left_move(left)
        anchor_pose[tick] = p
        corrected_right[tick] = dict(p["rightArm"])
        anchor_left[tick] = dict(left)
        tag = "(cached)" if lres < 0 else f"{lres:5.2f}"
        print(
            f"{tick:4d} | y {float(auth_right.get('yaw', 0)):6.1f}->{right['yaw']:6.1f}"
            f"  r {float(auth_right.get('roll', 0)):6.1f}->{right['roll']:6.1f} | "
            f"{left['pitch']:6.1f}/{left['yaw']:6.1f}/{left['roll']:6.1f}/{left['bend']:5.1f}"
            f"  {tag}"
        )

    # ---- 二遍：左臂每 tick 一个 IK 锚点，防插值中脱手 ----
    def right_at(tick: int) -> dict:
        """MC 真实缓动下的校正右臂角（度）：对 destination 帧 easing 插两侧锚点。"""
        if tick in corrected_right:
            return corrected_right[tick]
        a = max(t for t in tpv_ticks if t < tick)
        b = min(t for t in tpv_ticks if t > tick)
        alpha = _ease(tpv.POSE[b].get("easing", "linear"), (tick - a) / (b - a))
        ra, rb = corrected_right[a], corrected_right[b]
        return {
            k: ra[k] + (rb[k] - ra[k]) * alpha
            for k in ("pitch", "yaw", "roll", "bend", "axis")
        }

    out: dict[int, dict] = {}
    prev_left = {k: float(anchor_pose[lo]["leftArm"][k]) for k in LEFT_KEYS}
    max_resid = 0.0
    for tick in range(lo, hi + 1):
        if tick in anchor_pose:
            out[tick] = anchor_pose[tick]  # 完整锚点（保 TPV 右臂/身体缓动）
            prev_left = dict(anchor_left[tick])
            continue
        # 中间 tick：只补左臂锚点，右臂/身体走 MC 自然缓动插值。
        # 起点 = 前帧解 + 两侧锚点解按缓动混合（顺插值路径收敛，消尾段跳变）。
        a = max(t for t in tpv_ticks if t < tick)
        b = min(t for t in tpv_ticks if t > tick)
        alpha = _ease(tpv.POSE[b].get("easing", "linear"), (tick - a) / (b - a))
        blend = {
            k: anchor_left[a][k] + (anchor_left[b][k] - anchor_left[a][k]) * alpha
            for k in LEFT_KEYS
        }
        target = grip_target(right_at(tick))
        # 多起点：前帧解 + 缓动混合 + 两侧锚点解本身（锚点是已知贴靶的低残差配置，
        # 直接从它们出发下降避免中间帧卡局部极小）；近零正则让每 tick 都贴紧握把靶。
        seeds = [prev_left, blend, anchor_left[a], anchor_left[b]]
        left, lres = solve_left(target, seeds, prev_left, w_prev=1e-5)
        max_resid = max(max_resid, lres)
        out[tick] = {"easing": "linear", "leftArm": _left_move(left)}
        prev_left = left
    print(f"densify: 左臂锚点 {lo}..{hi} 全 tick，中间帧最大合握残差 {max_resid:.2f}")
    return out


def main() -> int:
    emit_json(
        build_pose_table(),
        name="sword_cleave_fpv",
        description=(
            "sword.cleave 第一人称变体：挥击平面收到身体中线的双手交叉合握劈砍——"
            "右臂 yaw/roll 中线校正（pitch/bend 挥击弧线继承 TPV），左臂逐 tick 离线"
            "IK 钉到右拳柄尾握把点（关键帧间也不脱手），body 位移减半防晃"
            "（plan-fpv-cast-av-v1 P2 round 3）。"
        ),
        end_tick=20,
        stop_tick=22,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
