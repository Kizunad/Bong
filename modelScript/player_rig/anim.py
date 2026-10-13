"""五个校准动画与索引。

关键帧按 Blockbench 动画面板数值书写（panel），导出时按 Bedrock 导出器规则转换：
位移 X 反号、旋转 X/Y 反号（与 Bong creature_codec.cjs 的 exportAnimations 同一规则）。
"""

from __future__ import annotations

from .rig import JOINTS

PREFIX = "animation.kizuna_players_animation.player."
AXES = ("x", "y", "z")
SWING = 30  # calib_axes 每个轴转到的角度（度）
STEP = 0.5  # calib_axes 每步秒数


def _num(value: float):
    value = round(value, 5)
    return 0 if value == 0 else (int(value) if value == int(value) else value)


def _time(seconds: float) -> str:
    return ("%.5f" % seconds).rstrip("0").rstrip(".")


def export_vector(channel: str, vector) -> list:
    """Blockbench 面板数值 -> Bedrock 导出值。"""
    x, y, z = vector
    if channel == "rotation":
        x, y = -x, -y
    elif channel == "position":
        x = -x
    return [_num(x), _num(y), _num(z)]


def channel(name: str, frames) -> object:
    """frames 为 [(秒, 面板向量)]；单个零秒关键帧导出为常量数组，与 Blockbench 一致。"""
    if len(frames) == 1 and frames[0][0] == 0:
        return export_vector(name, frames[0][1])
    return {_time(t): export_vector(name, v) for t, v in frames}


def _doc(key: str, length: float, loop, bones: dict, timeline: dict | None = None) -> dict:
    body = {"loop": loop, "animation_length": length, "bones": bones}
    if timeline:
        body["timeline"] = timeline
    return {"format_version": "1.8.0", "animations": {PREFIX + key: body}}


def calib_zero() -> dict:
    zero = [(0, (0, 0, 0))]
    bones = {b: {"rotation": channel("rotation", zero), "position": channel("position", zero)} for b in JOINTS}
    return _doc("calib_zero", 1.0, True, bones)


def calib_axes() -> dict:
    bones = {}
    block = 3 * 2 * STEP  # 每根骨 X/Y/Z 各 0→30→0
    for index, bone in enumerate(JOINTS):
        frames = [(index * block, (0, 0, 0))]
        for axis in range(3):
            start = index * block + axis * 2 * STEP
            peak = [0, 0, 0]
            peak[axis] = SWING
            frames.append((start + STEP, tuple(peak)))
            frames.append((start + 2 * STEP, (0, 0, 0)))
        bones[bone] = {"rotation": channel("rotation", frames)}
    return _doc("calib_axes", len(JOINTS) * block, False, bones)


def calib_item() -> dict:
    # 原版 ITEM 手臂姿势：pitch = pitch*0.5 - 0.31415927 rad，即上臂向前 18°；模板 X 向前为正。
    raise_arm = [(0, (18, 0, 0))]
    wrist = [(0, (-45, 0, 0)), (1.5, (45, 0, 0)), (3.0, (-45, 0, 0))]
    bones = {}
    for side in "rl":
        bones[f"{side}_upper_arm"] = {"rotation": channel("rotation", raise_arm)}
        bones[f"{side}_forearm"] = {"rotation": channel("rotation", [(0, (0, 0, 0))])}
        bones[f"{side}_hand"] = {"rotation": channel("rotation", wrist)}
    return _doc("calib_item", 3.0, True, bones)


def calib_mirror() -> dict:
    # 右臂挥击：后摆（带 Y/Z 分量，便于看出镜像规则）→ 前挥击中 → 回收。
    upper = [(0, (0, 0, 0)), (0.3, (-50, 25, 40)), (0.5, (95, -20, -10)), (0.9, (30, 0, 0)), (1.2, (0, 0, 0))]
    fore = [(0, (0, 0, 0)), (0.3, (60, 0, 0)), (0.5, (15, 0, 0)), (1.2, (0, 0, 0))]
    hand = [(0, (0, 0, 0)), (0.3, (-20, 0, 15)), (0.5, (25, 0, 0)), (1.2, (0, 0, 0))]
    bones = {
        "r_upper_arm": {"rotation": channel("rotation", upper)},
        "r_forearm": {"rotation": channel("rotation", fore)},
        "r_hand": {"rotation": channel("rotation", hand)},
    }
    return _doc("calib_mirror", 1.2, False, bones, {"0.2": ["step_r"], "0.5": ["hit"]})


def calib_layer() -> dict:
    # 右臂侧举过头，前臂和手左右摆动；腰胸头轻微配合，腿不写轨道。
    sway = [(0, (0, 0, -20)), (0.3, (0, 0, 25)), (0.6, (0, 0, -20)), (0.9, (0, 0, 25)), (1.2, (0, 0, -20))]
    wrist = [(0, (0, 0, 8)), (0.3, (0, 0, -8)), (0.6, (0, 0, 8)), (0.9, (0, 0, -8)), (1.2, (0, 0, 8))]
    bones = {
        "r_upper_arm": {"rotation": channel("rotation", [(0, (0, 0, -140))])},
        "r_forearm": {"rotation": channel("rotation", sway)},
        "r_hand": {"rotation": channel("rotation", wrist)},
        "head": {"rotation": channel("rotation", [(0, (0, 0, 5)), (0.6, (0, 0, -5)), (1.2, (0, 0, 5))])},
    }
    return _doc("calib_layer", 1.2, True, bones)


#: id -> (构造函数, mask, priority, blend_in, blend_out, hand, fpv)
ANIMATIONS = {
    "calib_zero": (calib_zero, "full", 5, 0.1, 0.1, "none", False),
    "calib_axes": (calib_axes, "full", 5, 0.1, 0.1, "none", False),
    "calib_item": (calib_item, "upper", 10, 0.1, 0.1, "both", False),
    "calib_mirror": (calib_mirror, "r_arm", 20, 0.05, 0.15, "main", False),
    "calib_layer": (calib_layer, "upper", 10, 0.15, 0.15, "none", False),
}


def index() -> dict:
    return {"animations": [
        {"id": name, "file": f"animations/{name}.animation.json", "mask": mask, "priority": priority,
         "blend_in": blend_in, "blend_out": blend_out, "hand": hand, "fpv": fpv}
        for name, (_, mask, priority, blend_in, blend_out, hand, fpv) in ANIMATIONS.items()
    ]}
