"""PlayerAnimator / Emotecraft v3 关键帧采样。

从 `client/tools/render_animation.py` 提出来的：那份是 Bong 的客户端开发工具，
而这段逻辑处理的是 **通用 MC 动画格式**——关键帧收集、easing 曲线、按 tick 采样，
跟 Bong 没有一点关系。

提出来是为了掰正依赖方向。`render_player_pose.py` 原先把 `client/tools` 插进
sys.path 去 import 它（`parents[2] / "client" / "tools"`），等于渲染底座反过来
依赖调用方的仓库布局——库被搬进独立 repo 后这条必断。

easing 那段的原注释保留：整 tick 采样看不出线性和缓动的差别（所有缓动都满足
f(0)=0、f(1)=1），只有按子 tick 出 GIF 才暴露，别当成可有可无的润色。
"""

from __future__ import annotations

import math
from typing import Dict, List, Tuple

BODY_PART_NAMES = {"body", "head", "torso", "leftArm", "rightArm", "leftLeg", "rightLeg"}
AXIS_NAMES = {"x", "y", "z", "pitch", "yaw", "roll", "bend", "axis"}


def default_axis_value(axis_name: str) -> float:
    # MC rightLeg rest z = 0.1, leftLeg = 0.1? In vanilla, both legs have z=0.1?
    # For the bare-bones stick figure this default doesn't matter much; 0 is fine.
    return 0.0


def collect_keyframes(emote: dict) -> Dict[str, Dict[str, List[Tuple[int, float, str]]]]:
    """{part_name: {axis_name: [(tick, value, easing), ...]}} sorted by tick."""
    kfs: Dict[str, Dict[str, List[Tuple[int, float, str]]]] = {}
    for move in emote["moves"]:
        tick = int(move["tick"])
        easing = move.get("easing", "linear")
        for k, v in move.items():
            if k in ("tick", "comment", "easing", "turn"):
                continue
            if k not in BODY_PART_NAMES or not isinstance(v, dict):
                continue
            for axis, value in v.items():
                if axis not in AXIS_NAMES:
                    continue
                kfs.setdefault(k, {}).setdefault(axis, []).append((tick, float(value), easing))
    for part_kfs in kfs.values():
        for axis_list in part_kfs.values():
            axis_list.sort(key=lambda t: t[0])
    return kfs


# ---- easing ---------------------------------------------------------------
# 此前这里是纯线性插值，easing 字段被丢掉。整 tick 采样时看不出来（所有缓动
# 函数都满足 f(0)=0、f(1)=1，关键帧上取值一模一样），可一旦按子 tick 采样出
# GIF/视频，节奏就全平了——而节奏正是 easing 唯一负责的东西。

def _ease_in(kind: float, a: float) -> float:
    return a ** kind


_EASE_IN = {
    "SINE": lambda a: 1.0 - math.cos(a * math.pi / 2.0),
    "QUAD": lambda a: a * a,
    "CUBIC": lambda a: a ** 3,
    "QUART": lambda a: a ** 4,
    "QUINT": lambda a: a ** 5,
    "EXPO": lambda a: 0.0 if a <= 0.0 else 2.0 ** (10.0 * a - 10.0),
    "CIRC": lambda a: 1.0 - math.sqrt(max(0.0, 1.0 - a * a)),
}


def apply_easing(name: str, alpha: float) -> float:
    """Emotecraft/PlayerAnimator 的 easing 名 → [0,1] 曲线。

    命名规则是 IN/OUT/INOUT + 族名。OUT 是 IN 的反射，INOUT 是两半拼接——
    照标准 Penner 定义实现，未知名字回退线性（宁可平也不要静默算错）。
    """
    a = min(1.0, max(0.0, float(alpha)))
    # 端点**精确**返回 0/1。不这么钉的话 INSINE(1) = 1-cos(π/2) = 0.9999999999999999，
    # 关键帧上的取值就会有 1e-16 的漂移——本身无害，但"整 tick 取值不受 easing
    # 影响"这条保证一旦不成立，就没法断言既有的一大批整 tick 预览未被本次改动波及。
    if a <= 0.0:
        return 0.0
    if a >= 1.0:
        return 1.0
    n = (name or "linear").upper()
    if n in ("LINEAR", ""):
        return a
    for prefix in ("INOUT", "IN", "OUT"):
        if n.startswith(prefix):
            fam = _EASE_IN.get(n[len(prefix):])
            if fam is None:
                return a
            if prefix == "IN":
                return fam(a)
            if prefix == "OUT":
                return 1.0 - fam(1.0 - a)
            return fam(2.0 * a) / 2.0 if a < 0.5 else 1.0 - fam(2.0 - 2.0 * a) / 2.0
    return a


def sample_axis(
    kfs: Dict[str, Dict[str, List[Tuple[int, float, str]]]],
    part: str,
    axis: str,
    tick: float,
) -> float:
    axis_list = kfs.get(part, {}).get(axis)
    if not axis_list:
        return default_axis_value(axis)
    if tick <= axis_list[0][0]:
        return axis_list[0][1]
    if tick >= axis_list[-1][0]:
        return axis_list[-1][1]
    for i in range(len(axis_list) - 1):
        t0, v0, e0 = axis_list[i]
        t1, v1, _ = axis_list[i + 1]
        if t0 <= tick <= t1:
            if t1 == t0:
                return v1
            alpha = (tick - t0) / (t1 - t0)
            # easing 取**起始帧**那条：PlayerAnimator 的 isEasingBefore 默认 false，
            # 用的是 before.ease，所以某帧的 easing 管的是「本帧 → 下一帧」这一段。
            # 详见 docs/player-animation-conventions.md §15。
            return v0 + (v1 - v0) * apply_easing(e0, alpha)
    return axis_list[-1][1]


def sample_part(kfs, part: str, tick: float) -> Dict[str, float]:
    return {axis: sample_axis(kfs, part, axis, tick) for axis in AXIS_NAMES}
