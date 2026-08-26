#!/usr/bin/env python3
"""MC 玩家动画轴 ↔ bbmodel/Blockbench 轴的**唯一换算处**。

## ⚠ 读和写不是同一套符号

这是本模块存在的主要理由，也是踩过两次的坑。Blockbench 对 `.bbmodel` 的 **animation
通道**：

    读文件时  对 X / Y 取反（position 的 X 同样取反）
    写文件时  不取反

**不对称。** 于是：

    我们要写给它看  → `WRITE_LAYERS`：bb.x = +pitch, bb.y = -yaw, bb.z = -roll
    它存盘后我们读  → `READ_LAYERS` ：pitch = -bb.x, yaw = +bb.y, roll = -bb.z

静态 `group.rotation` 不走这条路，用的是标准右手系（= `READ_LAYERS` 那套）：
`bb.x = -pitch, bb.y = +yaw, bb.z = -roll`，这一套由 `render_player_pose.part_matrix`
独立佐证（`R._rotmat(-roll,2) @ R._rotmat(yaw,1) @ R._rotmat(-pitch,0)`）。

## 这两条是怎么定下来的（都来自实测，不是推的）

1. **写侧**：2026-08-26 按「读写同一套」生成了 `ClubPlayerAnim.bbmodel`，用户打开后报
   「整个身体（除了头）都反转了、朝后」——四肢镜像。头看着没事，是因为头的世界朝向由
   `body.yaw + head.yaw` 抵消掉大半，四肢没有这层抵消。⇒ 写文件必须**预先取反**去抵消
   它读入时的取反。
2. **读侧**：同一天用户在 Blockbench 里手摆了一帧「棍举过头顶」并存盘。存盘后**未改动的
   每一个关键帧** X / Y 都被整齐地取了反、Z 原样，position 的 X 也取了反——即它写出来的
   是未取反的内部值。按 `READ_LAYERS` 解那一帧，得到的是一个合理的过顶姿态
   （棍仰角 +78.6°、双手举起）；按写侧那套解会得到手臂朝后下方的乱姿态。

**两条各自独立，别把其中一条推广到另一条。** 上一轮就是拿证据 2 去改了写侧，把资产写
镜像了。

## 还没做的验证

真正的一锤定音是让 Blockbench 自己说话：`core/bbmodel_to_geckolib.py` 已经能用 Playwright
驱动 web 版 Blockbench，加载一份带动画的 bbmodel、读回 bone 的实际旋转即可把这条不对称
钉死。在那之前，本模块的符号来自上面两次实测，改动前请先复现它们。

## 其余换算

    position（写）: bb = (-x, -y, z) × 16        （米 → px；X 预取反、y 翻）
    position（读）: x = -bb.x/16, y = -bb.y/16, z = bb.z/16
    bend（写）    : 纯 X 轴，axis=180 → bb.x = -bend；axis=0 → bb.x = +bend
                    （bend 也走 animation 通道，同样吃写侧的 X 预取反）
"""

from __future__ import annotations

import math

# (MC 轴名, bb 分量下标, 符号)。**内 pitch → 中 yaw → 外 roll** 的嵌套单轴顺序与 MC 的
# `rotationZYX(roll, yaw, pitch)` 作用次序一致，多轴同时非零时两边才不会解释出歧义。
#
# 写侧：预先取反 X/Y，抵消 Blockbench 读入时的取反。
WRITE_LAYERS = (("pitch", 0, +1.0), ("yaw", 1, -1.0), ("roll", 2, -1.0))
# 读侧：Blockbench 存盘写的是未取反的内部值，标准右手系。
READ_LAYERS = (("pitch", 0, -1.0), ("yaw", 1, +1.0), ("roll", 2, -1.0))
AXIS_ORDER = tuple(name for name, _, _ in WRITE_LAYERS)
PX_PER_BLOCK = 16.0

# 老名字：一律指**写侧**（生成器用得最多）。留着是为了 import 不断，新代码请直呼
# WRITE_LAYERS / READ_LAYERS，把"这是哪一侧"写在脸上。
AXIS_LAYERS = WRITE_LAYERS


def rotation_to_bb(axes: dict, axis_name: str) -> list[float]:
    """MC 轴 → **写进文件**的 bb 三元组（走写侧符号）。"""
    for name, index, sign in WRITE_LAYERS:
        if name != axis_name:
            continue
        out = [0.0, 0.0, 0.0]
        out[index] = round(sign * float(axes.get(name, 0.0)), 4)
        return out
    raise KeyError(f"未知轴层 {axis_name!r}，可选 {AXIS_ORDER}")


def rotation_from_bb(triple, axis_name: str) -> float:
    """**Blockbench 存盘的**单轴 group 三元组 → MC 角度（走读侧符号）。

    注意它**不是** `rotation_to_bb` 的逆——两侧符号不同，见模块 docstring。
    """
    for name, index, sign in READ_LAYERS:
        if name == axis_name:
            return float(triple[index]) / sign
    raise KeyError(f"未知轴层 {axis_name!r}，可选 {AXIS_ORDER}")


def euler_to_mc(bb_xyz) -> dict:
    """**Blockbench 存盘的**合成欧拉三元组 → MC 的 pitch/yaw/roll（读侧）。

    用户在 Blockbench 里拖 gizmo 时，三个轴会被写进同一个 group（单轴分层被打破），
    这时候得先把整条 group 链乘起来、分解成一个欧拉三元组，再用这个函数换算。
    """
    x, y, z = (float(v) for v in bb_xyz)
    return {"pitch": -x, "yaw": y, "roll": -z}


def mc_to_euler(axes: dict) -> list[float]:
    """MC 轴 → 单个 bb 欧拉三元组，**读侧口径**（= 静态 group.rotation 那一套）。

    静态字段走这个；动画关键帧走 `rotation_to_bb`（写侧）。
    """
    return [-float(axes.get("pitch", 0.0)),
            float(axes.get("yaw", 0.0)),
            -float(axes.get("roll", 0.0))]


def body_position_to_bb(body: dict) -> list[float]:
    """`body.x/y/z`（米）→ **写进文件**的 bb position（px）。X 预取反 + y 翻。"""
    return [round(-float(body.get("x", 0.0)) * PX_PER_BLOCK, 4),
            round(-float(body.get("y", 0.0)) * PX_PER_BLOCK, 4),
            round(float(body.get("z", 0.0)) * PX_PER_BLOCK, 4)]


def body_position_from_bb(triple) -> dict:
    """**Blockbench 存盘的** bb position → MC 米。同样不是 to 的逆（X 差一个负号）。"""
    x, y, z = (float(v) for v in triple)
    return {"x": -x / PX_PER_BLOCK, "y": -y / PX_PER_BLOCK, "z": z / PX_PER_BLOCK}


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
    # bend 也走 animation 通道 → 同样吃写侧的 X 预取反
    if a < 1.0 or a > 359.0:
        return +float(bend_deg)      # 轴 +X
    if abs(a - 180.0) < 1.0:
        return -float(bend_deg)      # 轴 −X
    raise AssertionError(
        f"bendAxis={axis_deg}° 不是纯 X 折弯，单轴层表达不了——需要再拆一层斜轴 group")


def bend_from_bb(bb_x: float) -> tuple[float, float]:
    """**Blockbench 存盘的**单轴 X 旋转 → (bend, axis)，读侧口径，约定 bend 取正。"""
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
