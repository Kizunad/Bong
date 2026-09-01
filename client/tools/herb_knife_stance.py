#!/usr/bin/env python3
"""凡铁采药刀三条动画共用的持刀架势、俯身补偿与目标区口径。

## 为什么单独一个模块

三条动画要接得上：`herb_knife_unfold` 甩开刃之后停在持刀架势，`herb_harvest` 和
`herb_knife_slash` 从这个架势起手、也回到它。架势的数值只能有一份——抄三份必然漂，
而漂了以后连招之间会"跳一下"，正是 conventions §2.1 要治的那个廉价僵硬感。

## 只用「预览与运行时同解」的通道（硬约束，不是洁癖）

写这三条动画时逐条查过 PlayerAnimator 1.0.2-rc1 的源码，通道分三档：

**能用**
- `pitch / yaw / roll / bend / axis` —— 两边都是 ModelPart 直接吃，`AnimationApplier`
  和 `preview_player_anim` 有逐点对拍单测锁死。
- `torso.x/y/z`、`head.x/y/z` —— 运行时 `updatePart` 是**绝对赋值**
  （`part.pivotY = 动画值`），预览是`静止枢轴 + 偏移`；这两个部件的静止枢轴恰好是
  (0,0,0)，所以两边恒等。
- 腿的 `z` —— 静止枢轴 z=0.1px，两边差 0.1px，肉眼不可见。

**禁用**
- `body.*` —— 位移单位是**格**：`PlayerRendererMixin.applyBodyTransforms` 拿到值直接
  `matrixStack.translate(x, y+0.7, z)`，那是实体空间（1 格 = 16px），而预览按 px 用，
  差 16 倍；旋转还多一层手性：游戏在 `scale(-1,-1,1)` **之前**的空间里转，预览在
  ModelPart 空间里转，共轭下来 pitch/yaw 的符号是反的。这两条都没进游戏实测过，
  仓库里 `+z 当前进`（fist_punch）和 `-z 当前进`（dodge_back）两种写法并存就是证据。
  少用一个不确定源，动画就少一次返工——所以这三条一格 body 都不用。
- 手臂/腿的 `x/y` —— 静止枢轴非零（臂 y=2、腿 y=12），绝对赋值 vs 相加差 2~12px。

## 手臂角度的符号（量出来的，不是猜的）

`pitch` 负 = 手往**身前**摆，正 = 往身后。`herb_harvest` 上一版把"左手前探按住草根"
写成 `leftArm.pitch=+48`，那只手实际甩到了身后。
"""

from __future__ import annotations

import math

# ---------------------------------------------------------------------------
# 持刀架势（三条动画的首帧 / 收势帧）
# ---------------------------------------------------------------------------
#
# 读感目标：肘收在肋侧、前臂斜向前下、刀从虎口出去指向前下方——"这人手里有把干活
# 的小刀，随时能下去割"。不是战斗握姿（那是 dagger 那两条的活），所以肩不端、刀不
# 举过肩线。
#
# 数值是扫格子解出来再逐帧渲图筛的：刃落在世界 y 15.2~16.6（腰线附近）、身前 2.6~6.7px、
# 偏右 0.4~6.5px，刃仰角 +5°（近水平略朝下）。刀不能已经伸在身前最远处——那样"探身
# 去割"这一下就没有行程可走了，上一版的通病之一就是刃在架势帧比在割入帧还靠前。
GUARD: dict[str, dict[str, float]] = {
    "torso": dict(pitch=8.0, yaw=14.0),
    "head": dict(pitch=10.0, yaw=-12.0),
    "rightArm": dict(pitch=-4.0, yaw=24.0, roll=18.0, bend=52.0, axis=180.0),
    "leftArm": dict(pitch=8.0, yaw=14.0, roll=-10.0, bend=22.0, axis=180.0),
    "rightLeg": dict(pitch=-5.0, yaw=10.0, bend=8.0, axis=0.0),
    "leftLeg": dict(pitch=4.0, yaw=8.0, bend=5.0, axis=0.0),
}

#: 架势自带的躯干前倾角。上半身的 z 前移按它算（见 `hip_hinge`）。
GUARD_TORSO_PITCH = GUARD["torso"]["pitch"]


def hip_hinge(torso_pitch_deg: float) -> float:
    """躯干前倾 θ° 时，上半身该整体往身前挪多少 px 才不腰断。返回的是 **z 偏移**（负 = 身前）。

    `torso` 的枢轴在**脖子**（ModelPart `(0,0,0)`），不在腰。所以 `torso.pitch` 转出来
    的是"从肩往前折"：胯那一端被甩到身后 `12·sinθ` px，与腿顶脱开。这是"腰断"的真正
    来源——上一版 `herb_harvest` 实测髋缝 5.6~7.1px，一条腿才 4px 宽。

    绕脖子转 θ、再整体平移 `-12·sinθ`，等价于**绕胯转 θ**（刚体：转完把枢轴挪回原位
    就是换了个转轴）。于是胯回到腿顶上方，腰接上了，而肩/头/手被送到身前——这正是
    "俯身去够地上的草"该有的样子。头和两条手臂必须跟着同一个 z 走：它们和 torso 是
    **兄弟节点**不是子节点（conventions §7.3），torso 动了它们不会自己跟。

    原版潜行走的是另一半解：不挪上半身，改把两条腿往身后挪（`PlayerModel.setupAnim`
    里 `rightLeg.z = leftLeg.z = 4.0F` 配 `body.xRot = 0.5rad`）。两种解的髋缝完全一样
    （实测 θ=26° 都是 0.41px），差别在**脚**和**够不够得着**：
      - 腿后挪：脚从 z[-2,2] 滑到 z[3.3,7.3]（人往后撤了半步），刃前伸停在 z=-8.5；
      - 上身前挪：脚钉在 z[-2,2] 不动，刃前伸到 z=-13.7。
    采药是"钉住脚、探身去够"，所以这里取后者。

    残差是竖直分量：胯端同时抬高 `12(1-cosθ)`（θ=26° 时 1.2px）。它可以用 `torso.y`
    补平，但手臂的 y 通道不可用（见模块文档），补了躯干、手臂就脱节。所以只补 z，
    把前倾控制在 32° 以内，髋缝就留在 1px 以下（实测 θ=32° 为 0.83px）。
    """
    return -round(12.0 * math.sin(math.radians(torso_pitch_deg)), 2)


def stance(torso_pitch: float, torso_yaw: float, *, head=None,
           right_arm=None, left_arm=None,
           right_leg=None, left_leg=None) -> dict:
    """按躯干前倾角组一帧姿态，上半身的 z 前移自动跟上。

    只写"这一帧和架势不同的地方"，其余沿用 `GUARD`。`z` 永远由 `hip_hinge` 给、不接受
    手写——手写过一次就会有人只改 `torso.pitch` 忘了改补偿，而漏改在静态图上要盯着
    髋缝看才发现得了（那正是上一版的死法）。
    """
    lean = hip_hinge(torso_pitch)
    return {
        "torso": dict(pitch=torso_pitch, yaw=torso_yaw, z=lean),
        "head": dict(GUARD["head"], **(head or {}), z=lean),
        "rightArm": dict(GUARD["rightArm"], **(right_arm or {}), z=lean),
        "leftArm": dict(GUARD["leftArm"], **(left_arm or {}), z=lean),
        "rightLeg": dict(GUARD["rightLeg"], **(right_leg or {})),
        "leftLeg": dict(GUARD["leftLeg"], **(left_leg or {})),
    }


def guard_pose(easing: str = "INOUTSINE") -> dict:
    """架势帧。`herb_harvest` / `herb_knife_slash` 的首末帧、`herb_knife_unfold` 的收势帧。"""
    pose = stance(GUARD_TORSO_PITCH, GUARD["torso"]["yaw"])
    pose["easing"] = easing
    return pose


# ---------------------------------------------------------------------------
# 目标区（门禁与设计共用一份口径）
# ---------------------------------------------------------------------------
#
# 全部是 **Bedrock 世界 px**（脚底 y=0、头顶 y=32、-z 是身前），因为门禁量的就是这个系。
#
# 草区：一株一格高的灵草长在身前。刃要真的探进去才算"在采"，而不是在肚子前面比划。
# 下限 y=9 是站着俯身能够到的极限（再低必须蹲，而下蹲要用 body.y，本套动画禁用它）。
# 上限**不能取腰线 18**：肘一折刀就举到胸前，那儿照样满足"身前 4px 以外"，于是"把
# 刀收回胸口"也能骗过这道门（差分自证实测：注入"幅度收到 45%"，深度只从 9.75 掉到
# 9.12，等于没区分力）。收到 14 —— 大腿中段那一档 —— 才是"人真的弯下去了"：重做版
# 割入帧刃最低 12.4px（余量 1.6），同一条动画收到 45% 幅度后是 14.9px（超限 0.9）。
# z 上限 -4 表示"越过躯干前脸（z=-2）再往前 2px"。
HERB_ZONE = dict(y_min=9.0, y_max=14.0, z_max=-4.0)

#: 挥击帧刃尖至少要够出身前多少 px（躯干前脸在 z=-2）。上一版反手割在 -2.3 停住，
#: 等于刀根本没离开身体，读作"在自己肚子上蹭"。
SLASH_REACH_Z = -7.0

#: 髋缝上限（px）。补完 z 之后剩下的是**竖直残差** `12(1-cosθ)`：胯端绕脖子转的时候
#: 同时抬高了那么多，而这一项没法在不拆散手臂的前提下补（手臂的 y 通道不可用）。
#: θ=26° 时是 1.21px，所以门限取 1.40 —— 前倾封顶 26°。
#: 区分力：上一版 `herb_harvest` 在同一判据下实测 6.61px，是门限的 4.7 倍。
HIP_SEAM_MAX = 1.40

#: 刀（含绳穗）与身体各段允许的最深互穿。0.75px 以下是渲染分辨率噪声。
SELF_CLIP_MAX = 0.75

#: 脚底允许的最大下陷（px）。
GROUND_SINK_MAX = 0.50
