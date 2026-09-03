#!/usr/bin/env python3
"""凡铁采药刀三条动画共用的持刀架势与**俯身姿态发生器**。

## 上一版为什么整个人是散的（重做的由来）

`torso` 的枢轴在**脖子**不在腰，所以 `torso.pitch` 前倾会把胯甩到身后 `12·sinθ`px，
与腿顶脱开。补法有两个，几何上等价，差别只在谁动（详见 `_hip_follow`）：原版潜行让
**腿跟着胯往身后挪**；上一版自创了另一半——让 **torso/head/双臂各自往身前平移**，
而两条腿一格不动、还按自己一套 pitch 摆。于是躯干在 z 上滑、下半身站在原地，人看到
的就是上下半身各干各的。上一版另外还全程 `torso.yaw=14°` 而腿正对前方，那是同一个
毛病的第二份：胸口拧着、胯没跟。

**这一版不再自己发明**，两处都换回仓库/原版既有的解：

| 事 | 这一版怎么做 | 出处 |
|----|-------------|------|
| 俯身 | `torso.pitch` + **两条腿同量 `z` 后挪** + 屈膝 + 胯后推 | 原版潜行 `leg.z=4.0` 配 `body.xRot=0.5rad` |
| 屈膝幅度 | `bend` 到 46°，`pitch` 到 −12° | `harvest_crouch`（26°/48°）、`bow_salute`（30°/−10°/15°）|
| 转体 | `torso.yaw` 与**腿的 `yaw` 同时转**（胯跟 55%） | `DaggerStanceTest`「站架要整个人转」|

而且这些全部由 `stance()` 的两个入参（`depth` / `twist`）派生，不是每帧手写——
**"改一处忘三处"这个失败模式在这套接口里不存在**。

## 这套骨架**蹲不下去**（写死在这里，免得下一轮又去试）

`torso` 的枢轴恒在 y=24（脖子），`rightLeg/leftLeg` 的枢轴恒在 y=12（胯）——都是常量。
屈膝只会把**脚**抬起来（实测 `bend=48°` 才抬 0.5px，配上 `pitch` 让脚前伸后净剩 0），
胯一格都不会下沉，因为胯就是那个不动的枢轴点。**整个人下沉这件事，只有 `body.y`
做得到**，这也正是 `harvest_crouch` 用它的唯一理由。

所以本套动画的"俯身"= **折腰 + 探臂**，不是"下蹲"。刃够到的是一格高灵草的**茎中段**
（世界 y≈12），不是贴地的根部——贴地在不动 `body.y` 的前提下做不到，硬凑只会把手臂
拉成怪姿势。

## `body.*` 一格不用（这一版的口径）

库源码给了确定的部分：`PlayerRendererMixin.applyBodyTransforms` 写的是
`translate(x, y+0.7, z)` → 转 → `translate(0, -0.7, 0)`，那对 ±0.7 是**旋转枢轴**
（腰），净位移就是 `(x, y, z)`，单位是**格**（1 格 = 16px）。

不确定的是**符号**，而且是真不确定：按源码所在的空间推，`+y` 是上、`+z` 是身后；
可仓库里四条名字自证方向的资产**全部相反**（`dash_forward` `+0.30`、`fist_punch_right`
撞击 `+0.22` 且 conventions §12 白纸黑字写着"前冲 lunge"、`dodge_back` `-0.50`、
`harvest_crouch` 下蹲 `body.y=+0.3`）。四位作者同时写反、还有人在游戏里验过写进正典，
比"我这段推导有漏"要不可信得多——所以这条**没定论**，与
`project_body_axis_preview_drift` 记的"符号存疑、动前先进游戏实证"一致。

而且**两边都是 0.3 格 = 4.8px 的大位移**：`harvest_crouch` 那条要么把脚埋进地里
4.6px，要么整个人浮起 5.1px——两种都很显眼。它多半也没被实机验过，所以它不构成
"照抄就安全"的先例。

上一版正是拿一个**自以为已定论**的前提（而且读反了）去禁通道 + 自创补偿，才有了这次
返工。这一版的处理是：**不赌**，也不照抄一个自己也没被验过的资产。三条动画一格
`body.*` 都不写；代价写在上一节（蹲不下去，只能折腰），收益是没有任何一个未定符号
能让它在游戏里出洋相。符号要定，只能 `/anim test` 实机看一眼。

## 手臂：roll 收进仓库既有包络

上一版右腕 `roll` 最深到 **58°**，而仓库 163 条动画里手臂 roll 从没超过 ±35°
（`fist_punch_right` v10 的 ±35 是极值，绝大多数在 ±12 以内）。roll 转的是**肘的折弯
平面**：58° 会把前臂从矢状面掀到侧面去，读感就是"肘往外翻"。这一版封顶 ±20°。

## 通道口径

- **能用**：`pitch/yaw/roll/bend/axis`（两边都是 ModelPart 直接吃，有逐点对拍单测）、
  `body.x/y/z`（本仓正典通道，见上表；预览侧的格→px 换算与 z 符号见
  `preview_player_anim.BODY_PX_PER_BLOCK`）、`torso/head` 的 `x/y/z`（静止枢轴 (0,0,0)，
  运行时的绝对赋值与预览的相加恒等）。
- **不用**：手臂/腿的 `x/y`——静止枢轴非零（臂 y=2、腿 y=12），运行时绝对赋值 vs 预览
  相加差 2~12px。腿的 `z` 另有 §7.2 的坑（"把腿贴回腹部"只会让断连更严重）。

## 符号（量出来的，不是猜的）

- 手臂 `pitch` **负 = 往身前**摆，正 = 往身后。
- 腿 `pitch` **负 = 脚往身前**（胯相对后推），正 = 脚往身后。
- `body.z` **正 = 身前**，`body.y` **正 = 下沉**。
"""

from __future__ import annotations

# ---------------------------------------------------------------------------
# 持刀架势（三条动画的首帧 / 收势帧）
# ---------------------------------------------------------------------------
#
# 读感目标：肘收在肋侧、前臂斜向前下、刀从虎口出去指向前下方——"这人手里有把干活
# 的小刀，随时能下去割"。不是战斗握姿（那是 dagger 那两条的活），所以肩不端、刀不
# 举过肩线。
GUARD: dict[str, dict[str, float]] = {
    "head": dict(pitch=6.0, yaw=-8.0),
    "rightArm": dict(pitch=-16.0, yaw=-4.0, roll=6.0, bend=54.0, axis=180.0),
    "leftArm": dict(pitch=-8.0, yaw=6.0, roll=-4.0, bend=20.0, axis=180.0),
}

#: 俯身到底（`depth=1`）时下半身的落点，逐项对着 `harvest_crouch`（仓库里已有的采集
#: 姿态：`torso.pitch=26° / 腿 bend=48°`）与 `bow_salute`（作揖，腿 `pitch=-10°` 胯后推）
#: 抄。**唯一自己加的是腿的 `z`**，理由见 `_hip_follow`。
CROUCH = dict(torso_pitch=34.0, leg_bend=46.0, leg_pitch=-12.0)
UPRIGHT = dict(torso_pitch=4.0, leg_bend=6.0, leg_pitch=0.0)

#: 胯跟随系数：躯干前倾 θ 时两条腿整体往身后挪多少（占几何值 `12·sinθ` 的比例）。
#: 原版潜行取的就是这个解——`PlayerModel.setupAnim` 里 `leg.z = 4.0F` 配
#: `body.xRot = 0.5rad`（28.6°），而 `12·sin(28.6°) = 5.74`，比值 0.70。照抄。
HIP_FOLLOW = 0.70

#: 胯跟着躯干转多少。躯干拧 `twist` 时两条腿也转 `twist·HIP_TWIST`——不给这一项，
#: 就是"胸口拧着、胯和腿正对前方"，也就是上一版被点名的"上半身下半身分离"的另一半。
HIP_TWIST = 0.55


def _hip_follow(torso_pitch_deg: float) -> float:
    """躯干前倾 θ° 时，两条腿该整体往身后挪多少 px（`leg.z`，正 = 身后）。

    `torso` 的枢轴在**脖子**（ModelPart `(0,0,0)`），不在腰。所以 `torso.pitch` 转出来
    的是"从肩往前折"：胯那一端被甩到身后 `12·sinθ` px，与腿顶脱开——这就是"腰断"。

    补法有两个，几何上等价（刚体转完把枢轴挪回去 = 换了个转轴），差别只在**谁在动**：

    - **腿往身后挪**（原版潜行、本函数）：脚跟着退半步，上半身留在原地；
    - **上半身往身前挪**（上一版自创）：脚钉住，肩/头/双臂整体前移 `-12·sinθ`。

    上一版选了后者，而且只搬 torso/head/双臂、腿一格不动——于是躯干在 z 上滑，两条
    腿站在原地按自己的一套 pitch 摆，人看到的就是上下半身各干各的。这一版换回原版那
    一半：**动的是腿，且两条腿一起动**，上半身完全不做平移，"整体位移"这件事根本不
    发生，也就没有"谁跟谁脱节"的余地。

    腿的 `z` 是可用通道（静止枢轴 z=0.1px，运行时绝对赋值 vs 预览相加只差 0.1px）。
    注意别把它和 conventions §7.2 那条坑搞混：那说的是"给腿加 z **试图把腿贴回腹部**"
    （单腿、为了掩盖 `leg.pitch` 过大造成的断连），这里是两条腿**同量**跟随胯的位移。
    """
    import math
    return round(12.0 * math.sin(math.radians(torso_pitch_deg)) * HIP_FOLLOW, 2)


def _lerp(a: float, b: float, t: float) -> float:
    return round(a + (b - a) * t, 3)


def stance(depth: float, twist: float, *, head=None,
           right_arm=None, left_arm=None, leg_split: float = 0.0,
           leg_depth: float | None = None) -> dict:
    """按俯身深度 `depth`（0=直立架势，1=俯身到底）与躯干拧转 `twist` 组一帧姿态。

    **躯干前倾、双膝屈度、胯后推、腿的跟随位移、胯的跟随转体——全部由这两个数派生。**
    上一版把它们拆成每帧手写的独立数字，改一处忘三处，上半身就自己走了；这套接口里
    没有"忘改"这个失败模式。

    只写这一帧和架势不同的**上半身**（头/双臂）。`leg_split` 给前后脚一点分差
    （正值 = 右脚在前），免得站成并脚立正。

    `leg_depth` 让**腿领先躯干**（kinetic chain 第一环是腿，§2.2）：不给就跟 `depth`
    走。注意它只挪相位，腿的四项（pitch/bend/z 跟随/yaw 跟随）仍然由一个数派生——
    "改一处忘三处"那个失败模式没有回来。
    """
    d = max(0.0, min(1.0, float(depth)))
    dl = d if leg_depth is None else max(0.0, min(1.0, float(leg_depth)))
    torso_pitch = _lerp(UPRIGHT["torso_pitch"], CROUCH["torso_pitch"], d)
    leg_pitch = _lerp(UPRIGHT["leg_pitch"], CROUCH["leg_pitch"], dl)
    leg_bend = _lerp(UPRIGHT["leg_bend"], CROUCH["leg_bend"], dl)
    leg_z = _hip_follow(torso_pitch)
    leg_yaw = round(twist * HIP_TWIST, 2)
    return {
        "torso": dict(pitch=torso_pitch, yaw=round(twist * (1.0 - HIP_TWIST), 2)),
        "head": dict(GUARD["head"], **(head or {})),
        "rightArm": dict(GUARD["rightArm"], **(right_arm or {})),
        "leftArm": dict(GUARD["leftArm"], **(left_arm or {})),
        # 两条腿共用同一个屈膝量与同一个跟随位移（真人下蹲两膝同屈、胯是一个整体），
        # 只在 pitch 上分前后脚。
        "rightLeg": dict(pitch=round(leg_pitch - leg_split, 3), yaw=leg_yaw,
                         bend=leg_bend, axis=0.0, z=leg_z),
        "leftLeg": dict(pitch=round(leg_pitch + leg_split, 3), yaw=leg_yaw,
                        bend=leg_bend, axis=0.0, z=leg_z),
    }


def guard_pose(easing: str = "INOUTSINE") -> dict:
    """架势帧。`herb_harvest` / `herb_knife_slash` 的首末帧、`herb_knife_unfold` 的收势帧。"""
    pose = stance(0.0, 14.0, leg_split=4.0)
    pose["easing"] = easing
    return pose


# ---------------------------------------------------------------------------
# 目标区（门禁与设计共用一份口径）
# ---------------------------------------------------------------------------
#
# 全部是 **Bedrock 世界 px**（脚底 y=0、头顶 y=32、-z 是身前）。

#: 草区：一株一格高（16px）的灵草长在身前，割的是茎的中下段。
#: 上限 13.5 是**这套骨架够得到的极限**留一点余量：屈膝不会让上半身下沉（见上文
#: "蹲不下去"），刃最低只能到 y≈11.5，割的是一格高灵草的茎中段。
HERB_ZONE = dict(y_min=2.0, y_max=13.5, z_max=-4.0)

#: 挥击帧刃尖至少要够出身前多少 px（躯干前脸在 z=-2）。
SLASH_REACH_Z = -7.0

#: 相邻两段之间允许的最大**真空隙**（px）。这一项量的是两个 OBB 的最小间距，
#: 0 = 还连着；它不会把"手臂正常转了 24°"误报成断裂（原版就那样转），而上一版的
#: `hip_seam`（量单个解剖锚点的错位）会，所以那道门在真断裂时也是绿的。
#: 门限按**仓库已认可资产**定，不是自己拍的：同一判据下 `bow_salute` 髋缝 2.49px、
#: `harvest_crouch` 1.50px、`dagger_slash` 0.19px。取 1.50 = 采集姿态本尊那一档。
LIMB_GAP_MAX = 1.50

#: 刀（含绳穗）与身体各段允许的最深互穿。0.75px 以下是渲染分辨率噪声。
SELF_CLIP_MAX = 0.75

#: 脚底允许的最大下陷（px）。
GROUND_SINK_MAX = 0.50

#: 手臂 roll 封顶。roll 转的是肘的折弯平面，越界就是"肘往外翻"。
ARM_ROLL_MAX = 20.0
