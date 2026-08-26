#!/usr/bin/env python3
"""MC 玩家动画轴 ↔ bbmodel/Blockbench 轴的**唯一换算处**，双向。

`player_animation/*.json`（Emotecraft v3，MC ModelPart 空间）和 `.bbmodel` 的
animation 通道之间只差一层坐标系翻转。这层换算此前散在两个生成器里各写一份，而且
**写反了**——本模块存在的全部理由就是让它只有一份、且带着证据。

## 换算

    rotation:  bb.x = -pitch,   bb.y = +yaw,   bb.z = -roll
    position:  bb = (x, -y, z) × 16          （米 → px，只翻 y）
    bend:      纯 X 轴，axis=180 → bb.x = +bend；axis=0 → bb.x = -bend

静态 `group.rotation` 与**动画关键帧**用的是**同一套**，别再分两套记。

## 这个符号是怎么定下来的（2026-08-26）

不是推的，是一次真实往返测出来的：把生成好的 `ClubPlayerAnim.bbmodel` 在 Blockbench
里打开、**只手动改 t=0 那一帧**、存盘，回头一比——其余每一个关键帧的 X / Y 都被整齐地
取了反、Z 原样，position 的 X 也取了反。Blockbench 只是读进来又写回去，能产生这种系统性
翻号，只可能是写文件用的约定和写进去的那套差一个负号。

此前 `gen_jian_player_anim` 的注释写着「动画通道走 Bedrock 约定：X/Y 取反」，据此生成的
文件在 Blockbench 里看到的是 **pitch / yaw 双双镜像**的姿态。

**离线核验抓不到这一类错。** 自己写的核验脚本用的是自己那套假设，两边同错照样逐点对拍
到 0.05px。唯一有效的判据是「拿进 Blockbench 转一圈再读回来」——所以本模块配的是
**往返测试**（`tests/test_bb_anim_roundtrip.py`），锁的是 `to_bb` / `from_bb` 互逆以及
生成器与读回器共用同一套常量，而不是再编一个"我觉得应该这样"的正向断言。
"""

from __future__ import annotations

import math

# (MC 轴名, bb 分量下标, 符号)。**内 pitch → 中 yaw → 外 roll** 的嵌套单轴顺序与 MC 的
# `rotationZYX(roll, yaw, pitch)` 作用次序一致，多轴同时非零时两边才不会解释出歧义。
AXIS_LAYERS = (("pitch", 0, -1.0), ("yaw", 1, +1.0), ("roll", 2, -1.0))
AXIS_ORDER = tuple(name for name, _, _ in AXIS_LAYERS)
PX_PER_BLOCK = 16.0


def rotation_to_bb(axes: dict, axis_name: str) -> list[float]:
    """某一层单轴 group 的 bb 三元组。`axes` 是该 part 的 MC 轴字典。"""
    for name, index, sign in AXIS_LAYERS:
        if name != axis_name:
            continue
        out = [0.0, 0.0, 0.0]
        out[index] = round(sign * float(axes.get(name, 0.0)), 4)
        return out
    raise KeyError(f"未知轴层 {axis_name!r}，可选 {AXIS_ORDER}")


def rotation_from_bb(triple, axis_name: str) -> float:
    """单轴 group 的 bb 三元组 → MC 角度（`rotation_to_bb` 的逆）。"""
    for name, index, sign in AXIS_LAYERS:
        if name == axis_name:
            return float(triple[index]) / sign
    raise KeyError(f"未知轴层 {axis_name!r}，可选 {AXIS_ORDER}")


def euler_to_mc(bb_xyz) -> dict:
    """一个**已经合成好**的 bb 欧拉三元组 → MC 的 pitch/yaw/roll。

    用户在 Blockbench 里拖 gizmo 时，三个轴会被写进同一个 group（单轴分层被打破），
    这时候得先把整条 group 链乘起来、分解成一个欧拉三元组，再用这个函数换算。
    """
    x, y, z = (float(v) for v in bb_xyz)
    return {"pitch": -x, "yaw": y, "roll": -z}


def mc_to_euler(axes: dict) -> list[float]:
    """`euler_to_mc` 的逆：MC 轴 → 单个 bb 欧拉三元组。"""
    return [-float(axes.get("pitch", 0.0)),
            float(axes.get("yaw", 0.0)),
            -float(axes.get("roll", 0.0))]


def body_position_to_bb(body: dict) -> list[float]:
    """`body.x/y/z`（米）→ bb position（px）。只翻 y。"""
    return [round(float(body.get("x", 0.0)) * PX_PER_BLOCK, 4),
            round(-float(body.get("y", 0.0)) * PX_PER_BLOCK, 4),
            round(float(body.get("z", 0.0)) * PX_PER_BLOCK, 4)]


def body_position_from_bb(triple) -> dict:
    x, y, z = (float(v) for v in triple)
    return {"x": x / PX_PER_BLOCK, "y": -y / PX_PER_BLOCK, "z": z / PX_PER_BLOCK}


def bend_to_bb(bend_deg: float, axis_deg: float) -> float:
    """bend → 单轴 X 旋转（度）。

    bendAxis 语义是"折弯方向绕主轴转多少"，轴 = (cos a, 0, sin a)。本仓的动画只用
    a=0（绕 +X）与 a=180（绕 −X）两种纯前后折弯，走解析分支而不是通用矩阵分解——JSON
    存的是弧度，π 转回度数是 180.000003，sin 不严格为 0，通用分解会渗出 1e-6 级的 y/z
    分量，落到单轴层里表达不了。
    """
    if abs(bend_deg) < 1e-9:
        return 0.0
    a = float(axis_deg) % 360.0
    if a < 1.0 or a > 359.0:
        return -float(bend_deg)      # 轴 +X
    if abs(a - 180.0) < 1.0:
        return +float(bend_deg)      # 轴 −X
    raise AssertionError(
        f"bendAxis={axis_deg}° 不是纯 X 折弯，单轴层表达不了——需要再拆一层斜轴 group")


def bend_from_bb(bb_x: float) -> tuple[float, float]:
    """单轴 X 旋转 → (bend, axis)。`bend_to_bb` 的逆，约定 bend 取正。"""
    value = float(bb_x)
    if abs(value) < 1e-9:
        return 0.0, 180.0
    return (value, 180.0) if value > 0 else (-value, 0.0)


def assert_pure_x(triple, where: str = "") -> tuple[float, float]:
    """bend 层必须是纯 X 转动；拖 gizmo 拧出来的 y/z 残差在 MC 里表达不了。

    返回 (bend, axis)，并把残差一并抛给调用方决定怎么处理——静默丢掉会让"读回来的姿态
    和 Blockbench 里看到的不是同一个"，那正是最难查的一类偏差。
    """
    x, y, z = (float(v) for v in triple)
    bend, axis = bend_from_bb(x)
    residual = (round(y, 3), round(z, 3))
    if max(abs(y), abs(z)) > 1.0:
        loc = f"{where}: " if where else ""
        raise ValueError(
            f"{loc}bend 层带着 y/z 残差 {residual}——MC 的 bend 只能绕水平轴转，"
            f"这部分表达不了。把它挪到手臂的 yaw/roll 上，或接受丢失后显式忽略")
    return bend, axis


def radians_if_needed(value: float, degrees_flag: bool) -> float:
    return value if degrees_flag else math.degrees(value)
