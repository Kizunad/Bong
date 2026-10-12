#!/usr/bin/env python3
"""工具类玩家使用动画（铜斧 / 石斧 / 铜镐 / 石镐 / 刨锄 / 刮刀 / 骨骸钳 / 钝气夹 / 阵旗）。

**从零写的关键帧**。这份生成器没有以任何已有动画脚本或已有动画 JSON 为底：姿态表是看着
`preview_player_anim.py` 渲出来的真玩家（带皮肤 + 带贴图手持物）逐件调出来的。

## 约定

- 输出 Emotecraft v3 JSON 到 ``client/src/main/resources/assets/bong/player_animation/<id>_use.json``，
  `degrees:false`，一条 move 一个 axis（出错时好定位）。
- 只用 ``rightArm / leftArm / torso / head / leftLeg / rightLeg`` 的旋转与 bend，以及 ``body`` 的**旋转**。
  **不用 ``body.x/y/z`` 位移**：预览工具把它渲小了 16 倍（见 ``preview_player_anim.BODY_PX_PER_BLOCK_TRUE``），
  凭预览调出来的位移到游戏里会错。重心靠 ``torso`` 前后倾 + 腿 bend 表达。
- **不用 ``rightItem`` 关键帧**：物品按模型 display 自然握持，刃口 / 工作面朝向靠手臂与躯干姿态给。
- 单手物就单手：左臂只做平衡（自然放松，随主动作反向摆一点），不去贴物品。
- 一次性动画，``endTick`` 的姿态 = ``tick 0``（自动补，连续触发不用经过「先垂手再举」）。
- ``stopTick = endTick + 2``（必须 > endTick + 1，否则构造函数 +3 兜底）。

## easing 写法

库语义是 ``before.ease``：写在某一帧上的 easing 管**这一帧 → 下一帧**那一段。所以这里把 easing 写在
每段的**起始帧**上：蓄力段 INOUTSINE，爆发段 OUTQUAD（急刹到位），回收段 INOUTSINE。

用法::

    python3 modelScript/generators/gen_tool_use_anims.py            # 写全部
    python3 modelScript/generators/gen_tool_use_anims.py axe_copper # 只写一件
"""

from __future__ import annotations

import argparse
import json
import math
from dataclasses import dataclass, field
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
OUT_DIR = REPO / "client" / "src" / "main" / "resources" / "assets" / "bong" / "player_animation"

PARTS = ("head", "torso", "body", "rightArm", "leftArm", "rightLeg", "leftLeg")
ROTATION_AXES = ("pitch", "yaw", "roll", "bend", "axis")   # 度数，emit 时转弧度
Pose = dict[str, dict[str, float]]


@dataclass(frozen=True)
class Key:
    tick: int
    pose: Pose
    easing: str = "INOUTSINE"        # 管这一帧 → 下一帧


@dataclass
class ToolAnim:
    item_id: str
    description: str
    keys: list[Key]
    end_tick: int
    loop_back: bool = True           # 末帧补成 tick 0 的姿态

    def all_keys(self) -> list[Key]:
        keys = sorted(self.keys, key=lambda key: key.tick)
        if self.loop_back and keys[-1].tick != self.end_tick:
            keys.append(Key(self.end_tick, keys[0].pose, keys[0].easing))
        return keys


def _radians(axis: str, value: float) -> float:
    return math.radians(value) if axis in ROTATION_AXES else value


def emit(anim: ToolAnim) -> dict:
    """姿态表 → Emotecraft v3 JSON：每个 (tick, part, axis) 一条 move。

    关键：起始姿态里出现过的每个 (part, axis)，在每一个后续关键帧里也要有值——
    不写就是「这一帧该轴未设值」，库会在相邻两个写过的帧之间直接插值，把中间帧的意图抹掉。
    这里把缺的补成 0（vanilla 值），让一张稀疏的姿态表也能表达「这帧回到 0」。
    """
    keys = anim.all_keys()
    used = {(part, axis) for key in keys for part, axes in key.pose.items() for axis in axes}
    moves = []
    for key in keys:
        for part, axis in sorted(used):
            value = key.pose.get(part, {}).get(axis, 0.0)
            moves.append({"tick": key.tick, "easing": key.easing, part: {axis: round(_radians(axis, value), 5)}})
    return {
        "version": 3,
        "author": "Bong",
        "name": f"{anim.item_id}_use",
        "description": anim.description,
        "emote": {
            "beginTick": 0,
            "endTick": anim.end_tick,
            "stopTick": anim.end_tick + 2,
            "isLoop": False,
            "returnTick": 0,
            "nsfw": False,
            "degrees": False,
            "moves": moves,
        },
    }


def arm(**axes: float) -> dict[str, float]:
    """手臂姿态。本仓约定手臂 bend 的 axis=180（肘往身前折）——写在这里，姿态表里不用每处重复。"""
    return {**axes, "axis": 180.0} if axes.get("bend") else dict(axes)


#: 手持物柄（或工作方向）在「前—上」平面里的仰角 φ（0 = 水平朝前，90 = 竖直朝上，-90 = 朝下，
#: 180 = 水平朝后）与手臂姿态的关系，是用 `preview_player_anim.hand_transform` 把手持物方向量到
#: 世界空间后拟合出来的：
#:     φ ≈ -rightArm.pitch + rightArm.bend - OFFSET
#: OFFSET 取决于模型 display 把物品「斜着握」的那一截：
#:   - 旧默认 `[0, -90, 55]`（铜斧 / 铜镐 / 石斧 / 石镐）→ 65
#:   - `held_item_common.hand_display` 的 `[-80, ±90, 0]`（刨锄 / 刮刀 / 骨骸钳 / 钝气夹 / 阵旗）→ 80
OFFSET_DEFAULT_HOLD = 65.0
OFFSET_KIT_HOLD = 80.0


def haft_arm(phi: float, bend: float, offset: float, **extra: float) -> dict[str, float]:
    """按「想让物品朝哪个仰角 φ」反算手臂：pitch = bend - φ - offset，肘 bend 由调用处定。

    只在右臂用；姿态表里写 φ 比写 pitch 好读——「柄竖直朝上」「柄水平朝前」比「pitch -75」直观。
    拟合是近似的（±10°），每个关键帧写完都要拿手持物世界姿态再量一遍。
    """
    return arm(pitch=bend - phi - offset, bend=bend, **extra)


ANIMS: dict[str, ToolAnim] = {}


def register(anim: ToolAnim) -> ToolAnim:
    ANIMS[anim.item_id] = anim
    return anim


def write(anim: ToolAnim, out_dir: Path = OUT_DIR) -> Path:
    out_dir.mkdir(parents=True, exist_ok=True)
    path = out_dir / f"{anim.item_id}_use.json"
    path.write_text(json.dumps(emit(anim), ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return path


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("ids", nargs="*", help="只写这几件（缺省全部）")
    parser.add_argument("--out", type=Path, default=OUT_DIR, help="输出目录")
    args = parser.parse_args()

    wanted = args.ids or list(ANIMS)
    for item_id in wanted:
        path = write(ANIMS[item_id], args.out)
        print(f"{item_id}: {path}")


# ── 铜斧 axe_copper ───────────────────────────────────────────────────────
# 铜斧：厚重的竖劈。蓄力慢（高举过肩、身体后仰），落斧快，砍进去一顿，再收回。
# 刃在建模稿 -X 侧（模型约定）。姿态符号：rightArm.pitch 负 = 往前上抬；
# torso.yaw 正 = 右肩被拽到身后（蓄力），负 = 右肩送向前（发力）。
#
# 【标定】用 `preview_player_anim` 的手持物变换把斧柄方向量到世界空间，发现斧柄在「前—上」
# 平面里的仰角 φ（0 = 水平朝前，90 = 竖直朝上，180 = 水平朝后）近似满足
#     φ ≈ -rightArm.pitch + rightArm.bend - 65
# （-65 是 display 把斧斜着握的那一截）。斧刃方向永远是斧柄方向在该平面里再转 -90°：
# 斧柄朝上时刃朝前，斧柄朝前时刃朝下——所以「柄竖直 → 柄水平朝前」这一段天然是刃口领路的
# 竖劈，不需要、也不许去扭手腕。下面每个关键帧先定 φ，再反算手臂 pitch / bend。
#   guard    φ≈105  斧扛在右肩前上方，刃朝前上
#   windup   φ≈135  举过头顶后仰，刃朝前
#   impact   φ≈ 25  落斧，柄近水平朝前、刃口朝下
#   overshoot φ≈ 5  顿一下，再略往下压

register(ToolAnim(
    item_id="axe_copper",
    description="铜斧竖劈：高举过肩蓄力 → 落斧砍入一顿 → 收回",
    end_tick=14,
    keys=[
        # guard
        Key(0, {
            "torso": {"pitch": 4, "yaw": 8},
            "head": {"pitch": -4},
            "rightArm": arm(pitch=-80, bend=90),
            "leftArm": arm(pitch=-18, roll=-10, bend=18),
            "rightLeg": {"pitch": 8},
            "leftLeg": {"pitch": -10},
        }),
        # windup：蓄力慢，5 tick 才到顶；躯干后仰拧开，左臂往前探着平衡
        Key(5, {
            "torso": {"pitch": -12, "yaw": 22},
            "head": {"pitch": -8},
            "body": {"pitch": -4},
            "rightArm": arm(pitch=-135, bend=65),
            "leftArm": arm(pitch=-34, roll=-24, bend=22),
            "rightLeg": {"pitch": 12, "bend": 8},
            "leftLeg": {"pitch": -16, "bend": 4},
        }, easing="OUTQUAD"),
        # impact：2 tick 落下；躯干前压、腿蹬住，左臂反向收回
        Key(7, {
            "torso": {"pitch": 32, "yaw": -14},
            "head": {"pitch": 8},
            "body": {"pitch": 8},
            "rightArm": arm(pitch=-50, bend=38),
            "leftArm": arm(pitch=8, roll=-14, bend=26),
            "rightLeg": {"pitch": 12, "bend": 20},
            "leftLeg": {"pitch": -22, "bend": 14},
        }, easing="INOUTSINE"),
        # overshoot / 砍入后顿一下
        Key(8, {
            "torso": {"pitch": 36, "yaw": -16},
            "head": {"pitch": 10},
            "body": {"pitch": 9},
            "rightArm": arm(pitch=-46, bend=32),
            "leftArm": arm(pitch=10, roll=-14, bend=28),
            "rightLeg": {"pitch": 14, "bend": 30},
            "leftLeg": {"pitch": -26, "bend": 22},
        }),
        Key(9, {
            "torso": {"pitch": 35, "yaw": -15},
            "head": {"pitch": 10},
            "body": {"pitch": 9},
            "rightArm": arm(pitch=-46, bend=33),
            "leftArm": arm(pitch=10, roll=-14, bend=28),
            "rightLeg": {"pitch": 14, "bend": 30},
            "leftLeg": {"pitch": -26, "bend": 22},
        }),
    ],
))


# ── 石斧 stone_axe ────────────────────────────────────────────────────────
# 同一套竖劈，但石斧轻、柄短：蓄力 4 tick（铜斧 5）、落斧同样 2 tick、顿得短，躯干前压与腿的下沉都小一档。
# 握法与铜斧同（旧默认 display，刃在 -X）。

register(ToolAnim(
    item_id="stone_axe",
    description="石斧竖劈：比铜斧轻快的蓄力 → 落斧 → 短顿 → 收回",
    end_tick=11,
    keys=[
        Key(0, {
            "torso": {"pitch": 4, "yaw": 8},
            "head": {"pitch": -4},
            "rightArm": haft_arm(100, 90, OFFSET_DEFAULT_HOLD),
            "leftArm": arm(pitch=-16, roll=-10, bend=16),
            "rightLeg": {"pitch": 8},
            "leftLeg": {"pitch": -10},
        }),
        Key(4, {
            "torso": {"pitch": -8, "yaw": 18},
            "head": {"pitch": -6},
            "body": {"pitch": -3},
            "rightArm": haft_arm(130, 65, OFFSET_DEFAULT_HOLD),
            "leftArm": arm(pitch=-30, roll=-22, bend=20),
            "rightLeg": {"pitch": 10, "bend": 6},
            "leftLeg": {"pitch": -14, "bend": 4},
        }, easing="OUTQUAD"),
        Key(6, {
            "torso": {"pitch": 26, "yaw": -12},
            "head": {"pitch": 7},
            "body": {"pitch": 7},
            "rightArm": haft_arm(25, 34, OFFSET_DEFAULT_HOLD),
            "leftArm": arm(pitch=6, roll=-12, bend=24),
            "rightLeg": {"pitch": 12, "bend": 20},
            "leftLeg": {"pitch": -20, "bend": 12},
        }),
        Key(7, {
            "torso": {"pitch": 29, "yaw": -14},
            "head": {"pitch": 9},
            "body": {"pitch": 8},
            "rightArm": haft_arm(12, 28, OFFSET_DEFAULT_HOLD),
            "leftArm": arm(pitch=8, roll=-12, bend=26),
            "rightLeg": {"pitch": 12, "bend": 22},
            "leftLeg": {"pitch": -22, "bend": 14},
        }),
    ],
))


# ── 铜镐 pickaxe_copper ───────────────────────────────────────────────────
# 刨挖：过顶抡下，尖头扎进地面，再往回撬一下（镐尖在土里当杠杆），最后拔出收回。
# 镐的两臂沿 ±X，刃在 -X 一侧领路（旧默认 display，与铜斧同一个速度方向规则）。
# 要扎的是脚前的地面，所以落点的 φ 是负的（柄朝前下）、躯干前压得比劈斧深，腿蹬得更低。
# 铜镐重：蓄力 6 tick、落 2 tick、扎进去停 2 tick、撬 3 tick。

register(ToolAnim(
    item_id="pickaxe_copper",
    description="铜镐刨挖：过顶抡下 → 尖头扎地 → 往回撬 → 拔出收回",
    end_tick=17,
    keys=[
        Key(0, {
            "torso": {"pitch": 6, "yaw": 8},
            "head": {"pitch": -4},
            "rightArm": haft_arm(95, 90, OFFSET_DEFAULT_HOLD),
            "leftArm": arm(pitch=-16, roll=-10, bend=16),
            "rightLeg": {"pitch": 8},
            "leftLeg": {"pitch": -10},
        }),
        Key(6, {
            "torso": {"pitch": -14, "yaw": 24},
            "head": {"pitch": -8},
            "body": {"pitch": -5},
            "rightArm": haft_arm(135, 62, OFFSET_DEFAULT_HOLD),
            "leftArm": arm(pitch=-36, roll=-24, bend=22),
            "rightLeg": {"pitch": 12, "bend": 10},
            "leftLeg": {"pitch": -16, "bend": 6},
        }, easing="OUTQUAD"),
        # 扎地：躯干压到 40°，腿蹬住下沉
        Key(8, {
            "torso": {"pitch": 40, "yaw": -14},
            "head": {"pitch": 12},
            "body": {"pitch": 10},
            "rightArm": haft_arm(-20, 15, OFFSET_DEFAULT_HOLD),
            "leftArm": arm(pitch=8, roll=-14, bend=26),
            "rightLeg": {"pitch": 16, "bend": 34},
            "leftLeg": {"pitch": -28, "bend": 26},
        }),
        Key(10, {
            "torso": {"pitch": 40, "yaw": -14},
            "head": {"pitch": 12},
            "body": {"pitch": 10},
            "rightArm": haft_arm(-22, 16, OFFSET_DEFAULT_HOLD),
            "leftArm": arm(pitch=8, roll=-14, bend=26),
            "rightLeg": {"pitch": 16, "bend": 34},
            "leftLeg": {"pitch": -28, "bend": 26},
        }),
        # 往回撬：手往回带、躯干抬起一截，镐头被扳起
        Key(13, {
            "torso": {"pitch": 28, "yaw": -6},
            "head": {"pitch": 6},
            "body": {"pitch": 4},
            "rightArm": haft_arm(-55, 35, OFFSET_DEFAULT_HOLD),
            "leftArm": arm(pitch=-14, roll=-14, bend=22),
            "rightLeg": {"pitch": 14, "bend": 22},
            "leftLeg": {"pitch": -22, "bend": 16},
        }),
    ],
))


# ── 石镐 stone_pickaxe ────────────────────────────────────────────────────
# 同一套刨挖，石镐轻：蓄力 4 tick、扎地停 1 tick、撬 2 tick，幅度小一档。

register(ToolAnim(
    item_id="stone_pickaxe",
    description="石镐刨挖：比铜镐轻快的抡下 → 扎地 → 撬 → 收回",
    end_tick=13,
    keys=[
        Key(0, {
            "torso": {"pitch": 6, "yaw": 8},
            "head": {"pitch": -4},
            "rightArm": haft_arm(95, 90, OFFSET_DEFAULT_HOLD),
            "leftArm": arm(pitch=-16, roll=-10, bend=16),
            "rightLeg": {"pitch": 8},
            "leftLeg": {"pitch": -10},
        }),
        Key(4, {
            "torso": {"pitch": -10, "yaw": 20},
            "head": {"pitch": -6},
            "body": {"pitch": -4},
            "rightArm": haft_arm(130, 62, OFFSET_DEFAULT_HOLD),
            "leftArm": arm(pitch=-32, roll=-22, bend=20),
            "rightLeg": {"pitch": 10, "bend": 8},
            "leftLeg": {"pitch": -14, "bend": 4},
        }, easing="OUTQUAD"),
        Key(6, {
            "torso": {"pitch": 34, "yaw": -12},
            "head": {"pitch": 10},
            "body": {"pitch": 8},
            "rightArm": haft_arm(-15, 15, OFFSET_DEFAULT_HOLD),
            "leftArm": arm(pitch=8, roll=-14, bend=24),
            "rightLeg": {"pitch": 14, "bend": 28},
            "leftLeg": {"pitch": -24, "bend": 20},
        }),
        Key(7, {
            "torso": {"pitch": 34, "yaw": -12},
            "head": {"pitch": 10},
            "body": {"pitch": 8},
            "rightArm": haft_arm(-17, 16, OFFSET_DEFAULT_HOLD),
            "leftArm": arm(pitch=8, roll=-14, bend=24),
            "rightLeg": {"pitch": 14, "bend": 28},
            "leftLeg": {"pitch": -24, "bend": 20},
        }),
        Key(9, {
            "torso": {"pitch": 24, "yaw": -6},
            "head": {"pitch": 5},
            "body": {"pitch": 4},
            "rightArm": haft_arm(-48, 32, OFFSET_DEFAULT_HOLD),
            "leftArm": arm(pitch=-12, roll=-14, bend=20),
            "rightLeg": {"pitch": 12, "bend": 18},
            "leftLeg": {"pitch": -20, "bend": 14},
        }),
    ],
))


# ── 刨锄 bao_chu ──────────────────────────────────────────────────────────
# 下刨后拉：举锄 → 锄板往前下方刨进土里 → 向自己这边拉（把土带过来）→ 松手收回。
# 锄板在建模稿 -X 侧。**这件要求 display 的 rotation 取 [-80, -90, 0]**（`hand_display` 默认
# 的 [-80, 90, 0] 会让 -X 的锄板在柄朝前时朝上——翻了半圈）。`gen_bao_chu.py` 已同步改成 -90。
# 拉的那一段 φ 变负：柄朝前下，锄板的速度方向 (U -cos φ, B -sin φ) 变成「往下并朝身体」，
# 正是把土往自己这边带。

register(ToolAnim(
    item_id="bao_chu",
    description="刨锄：举锄 → 刨进土里 → 向身前拉 → 松手收回",
    end_tick=18,
    keys=[
        Key(0, {
            "torso": {"pitch": 6, "yaw": 8},
            "head": {"pitch": -4},
            "rightArm": haft_arm(100, 85, OFFSET_KIT_HOLD),
            "leftArm": arm(pitch=-16, roll=-10, bend=16),
            "rightLeg": {"pitch": 8},
            "leftLeg": {"pitch": -10},
        }),
        Key(5, {
            "torso": {"pitch": -10, "yaw": 20},
            "head": {"pitch": -6},
            "body": {"pitch": -4},
            "rightArm": haft_arm(125, 60, OFFSET_KIT_HOLD),
            "leftArm": arm(pitch=-32, roll=-22, bend=20),
            "rightLeg": {"pitch": 10, "bend": 8},
            "leftLeg": {"pitch": -14, "bend": 4},
        }, easing="OUTQUAD"),
        # 刨入：锄板朝前下，躯干前压
        Key(7, {
            "torso": {"pitch": 28, "yaw": -12},
            "head": {"pitch": 10},
            "body": {"pitch": 8},
            "rightArm": haft_arm(35, 30, OFFSET_KIT_HOLD),
            "leftArm": arm(pitch=6, roll=-12, bend=24),
            "rightLeg": {"pitch": 12, "bend": 26},
            "leftLeg": {"pitch": -22, "bend": 18},
        }),
        Key(8, {
            "torso": {"pitch": 30, "yaw": -12},
            "head": {"pitch": 10},
            "body": {"pitch": 8},
            "rightArm": haft_arm(28, 32, OFFSET_KIT_HOLD),
            "leftArm": arm(pitch=6, roll=-12, bend=24),
            "rightLeg": {"pitch": 12, "bend": 28},
            "leftLeg": {"pitch": -22, "bend": 20},
        }),
        # 向身前拉：手臂回收、肘折深，躯干抬起，身体后坐
        Key(14, {
            "torso": {"pitch": 12, "yaw": -4},
            "head": {"pitch": 4},
            "body": {"pitch": -3},
            "rightArm": haft_arm(-35, 70, OFFSET_KIT_HOLD),
            "leftArm": arm(pitch=-26, roll=-16, bend=30),
            "rightLeg": {"pitch": 10, "bend": 12},
            "leftLeg": {"pitch": -16, "bend": 8},
        }),
    ],
))


# ── 刮刀 gua_dao ──────────────────────────────────────────────────────────
# 贴面刮削：刀口按在一个竖面（树皮 / 矿膜）上，向下短刮两下，每下之间提刀回到起点。
# 刃在 +X 侧；默认 display 下刀平举时 +X 朝下，所以刃口贴着面往下走是顺的。
# 刮是手臂的肘伸缩 + 肩微降，φ 基本不变（刀一直是同一个角度贴着面）：
# 手往下走 = pitch 变小（-95 → -58），肘同步多折一点把 φ 守在 20° 附近。
# 小件、轻：节奏快、幅度小，躯干只跟着前探，不做大动作。

register(ToolAnim(
    item_id="gua_dao",
    description="刮刀贴面刮削：刀口按住竖面，向下刮两下，中间提刀回到起点",
    end_tick=18,
    keys=[
        Key(0, {
            "torso": {"pitch": 8, "yaw": 4},
            "head": {"pitch": 6},
            "rightArm": arm(pitch=-98, bend=10),
            "leftArm": arm(pitch=-12, roll=-8, bend=14),
            "rightLeg": {"pitch": 4},
            "leftLeg": {"pitch": -6},
        }),
        # 第一刮：从高处落下去
        Key(2, {
            "torso": {"pitch": 10, "yaw": 3},
            "head": {"pitch": 8},
            "rightArm": arm(pitch=-104, bend=6),
            "leftArm": arm(pitch=-12, roll=-8, bend=14),
            "rightLeg": {"pitch": 4},
            "leftLeg": {"pitch": -6},
        }),
        Key(6, {
            "torso": {"pitch": 14, "yaw": 0},
            "head": {"pitch": 12},
            "rightArm": arm(pitch=-46, bend=54),
            "leftArm": arm(pitch=-14, roll=-10, bend=16),
            "rightLeg": {"pitch": 5},
            "leftLeg": {"pitch": -7},
        }),
        # 提刀：手抬回起点，刀口离面
        Key(8, {
            "torso": {"pitch": 10, "yaw": 3},
            "head": {"pitch": 8},
            "rightArm": arm(pitch=-92, bend=20),
            "leftArm": arm(pitch=-12, roll=-8, bend=14),
            "rightLeg": {"pitch": 4},
            "leftLeg": {"pitch": -6},
        }),
        # 第二刮
        Key(10, {
            "torso": {"pitch": 10, "yaw": 3},
            "head": {"pitch": 8},
            "rightArm": arm(pitch=-104, bend=6),
            "leftArm": arm(pitch=-12, roll=-8, bend=14),
            "rightLeg": {"pitch": 4},
            "leftLeg": {"pitch": -6},
        }),
        Key(14, {
            "torso": {"pitch": 14, "yaw": 0},
            "head": {"pitch": 12},
            "rightArm": arm(pitch=-46, bend=54),
            "leftArm": arm(pitch=-14, roll=-10, bend=16),
            "rightLeg": {"pitch": 5},
            "leftLeg": {"pitch": -7},
        }),
    ],
))


# ── 骨骸钳 gu_hai_qian ────────────────────────────────────────────────────
# 夹取 + 拔出：钳口朝前下探进去夹住骨头 → 夹紧顿一下 → 向后上方一扳把它拔出来 → 松手。
# 钳口方向就是物品的柄方向（钳口在柄的尖端），所以 φ 就是钳口朝向：探进去 φ≈-12（朝前下），
# 拔出时 φ 抬到 +40（钳口朝前上，手收回）。重工具：拔的时候身体后坐、躯干顿一下，腿蹬住。

register(ToolAnim(
    item_id="gu_hai_qian",
    description="骨骸钳夹取拔出：钳口探进去 → 夹紧一顿 → 向后上方扳出 → 松手",
    end_tick=20,
    keys=[
        Key(0, {
            "torso": {"pitch": 6, "yaw": 4},
            "head": {"pitch": 4},
            "rightArm": haft_arm(20, 60, OFFSET_KIT_HOLD),
            "leftArm": arm(pitch=-14, roll=-10, bend=16),
            "rightLeg": {"pitch": 6},
            "leftLeg": {"pitch": -8},
        }),
        # 探进去：手臂前伸，躯干前压，钳口朝前下
        Key(5, {
            "torso": {"pitch": 24, "yaw": -4},
            "head": {"pitch": 12},
            "body": {"pitch": 5},
            "rightArm": haft_arm(-10, 14, OFFSET_KIT_HOLD),
            "leftArm": arm(pitch=-22, roll=-14, bend=22),
            "rightLeg": {"pitch": 10, "bend": 20},
            "leftLeg": {"pitch": -16, "bend": 14},
        }),
        # 夹紧：几乎不动，只有躯干再往下沉一点
        Key(7, {
            "torso": {"pitch": 27, "yaw": -4},
            "head": {"pitch": 13},
            "body": {"pitch": 6},
            "rightArm": haft_arm(-12, 14, OFFSET_KIT_HOLD),
            "leftArm": arm(pitch=-22, roll=-14, bend=22),
            "rightLeg": {"pitch": 10, "bend": 24},
            "leftLeg": {"pitch": -16, "bend": 16},
        }, easing="OUTQUAD"),
        # 扳出：向后上方，手收回、躯干抬起后坐
        Key(10, {
            "torso": {"pitch": 2, "yaw": 6},
            "head": {"pitch": -2},
            "body": {"pitch": -5},
            "rightArm": haft_arm(45, 52, OFFSET_KIT_HOLD),
            "leftArm": arm(pitch=-4, roll=-10, bend=14),
            "rightLeg": {"pitch": 12, "bend": 10},
            "leftLeg": {"pitch": -18, "bend": 6},
        }),
        Key(11, {
            "torso": {"pitch": 0, "yaw": 7},
            "head": {"pitch": -3},
            "body": {"pitch": -6},
            "rightArm": haft_arm(52, 56, OFFSET_KIT_HOLD),
            "leftArm": arm(pitch=-2, roll=-10, bend=14),
            "rightLeg": {"pitch": 12, "bend": 8},
            "leftLeg": {"pitch": -18, "bend": 4},
        }),
    ],
))


# ── 钝气夹 dun_qi_jia ─────────────────────────────────────────────────────
# 夹取 + 拔出，但是小件：胸前这一格的精细活——夹口探向前下的毒草 / 刺，夹住，往上一提拔出来。
# 比骨骸钳快、幅度小：躯干只前探一点，没有后坐。

register(ToolAnim(
    item_id="dun_qi_jia",
    description="钝气夹夹取拔出：夹口探向前下 → 夹住 → 向上一提 → 收回",
    end_tick=14,
    keys=[
        Key(0, {
            "torso": {"pitch": 6, "yaw": 3},
            "head": {"pitch": 6},
            "rightArm": haft_arm(25, 70, OFFSET_KIT_HOLD),
            "leftArm": arm(pitch=-10, roll=-8, bend=14),
            "rightLeg": {"pitch": 4},
            "leftLeg": {"pitch": -6},
        }),
        Key(4, {
            "torso": {"pitch": 16, "yaw": -2},
            "head": {"pitch": 12},
            "rightArm": haft_arm(-5, 28, OFFSET_KIT_HOLD),
            "leftArm": arm(pitch=-14, roll=-10, bend=18),
            "rightLeg": {"pitch": 6, "bend": 10},
            "leftLeg": {"pitch": -10, "bend": 6},
        }),
        Key(5, {
            "torso": {"pitch": 17, "yaw": -2},
            "head": {"pitch": 12},
            "rightArm": haft_arm(-6, 28, OFFSET_KIT_HOLD),
            "leftArm": arm(pitch=-14, roll=-10, bend=18),
            "rightLeg": {"pitch": 6, "bend": 10},
            "leftLeg": {"pitch": -10, "bend": 6},
        }, easing="OUTQUAD"),
        Key(7, {
            "torso": {"pitch": 6, "yaw": 3},
            "head": {"pitch": 4},
            "rightArm": haft_arm(40, 62, OFFSET_KIT_HOLD),
            "leftArm": arm(pitch=-8, roll=-8, bend=14),
            "rightLeg": {"pitch": 4},
            "leftLeg": {"pitch": -6},
        }),
        Key(8, {
            "torso": {"pitch": 5, "yaw": 3},
            "head": {"pitch": 4},
            "rightArm": haft_arm(44, 64, OFFSET_KIT_HOLD),
            "leftArm": arm(pitch=-8, roll=-8, bend=14),
            "rightLeg": {"pitch": 4},
            "leftLeg": {"pitch": -6},
        }),
    ],
))


# ── 阵旗 array_flag ───────────────────────────────────────────────────────
# 挥旗：举旗过头 → 向身体左侧一挥 → 再向右侧一挥 → 收回。
# 旗杆是物品的柄方向（旗幡在 -X 侧、杆头骨饰在尖端），所以 φ≈90 就是旗杆竖直。
# 这件**不做插地**：这版旗杆的尖端是骨饰、握把末端才是柄尾，插地要「柄尾朝下」，而这个握法里
# 手臂放下来时旗杆是朝前下、骨饰端朝地——是倒着插。用手臂 yaw 左右扫出旗杆的弧、躯干与头跟着转，
# 旗幡随杆左右甩，是「指挥阵眼气机」的挥旗，不是插旗。

register(ToolAnim(
    item_id="array_flag",
    description="阵旗挥旗：举旗过头 → 向左一挥 → 向右一挥 → 收回",
    end_tick=22,
    keys=[
        Key(0, {
            "torso": {"pitch": 2, "yaw": 4},
            "head": {"pitch": -2},
            "rightArm": haft_arm(92, 82, OFFSET_KIT_HOLD),
            "leftArm": arm(pitch=-12, roll=-10, bend=14),
            "rightLeg": {"pitch": 4},
            "leftLeg": {"pitch": -6},
        }),
        # 举旗：手高举，旗杆竖直，身体略后仰拔高
        Key(4, {
            "torso": {"pitch": -6, "yaw": 10},
            "head": {"pitch": -8},
            "body": {"pitch": -2},
            "rightArm": haft_arm(95, 55, OFFSET_KIT_HOLD),
            "leftArm": arm(pitch=-26, roll=-20, bend=20),
            "rightLeg": {"pitch": 6},
            "leftLeg": {"pitch": -8},
        }),
        # 向左一挥：手臂 yaw 把旗杆扫向身体左侧，躯干与头跟着转
        Key(8, {
            "torso": {"pitch": -2, "yaw": -30},
            "head": {"pitch": -6, "yaw": -10},
            "body": {"pitch": -1},
            "rightArm": haft_arm(88, 55, OFFSET_KIT_HOLD, yaw=-44, roll=26),
            "leftArm": arm(pitch=-34, roll=-26, bend=24),
            "rightLeg": {"pitch": 8, "bend": 6},
            "leftLeg": {"pitch": -10, "bend": 4},
        }),
        # 向右一挥：反向扫到右侧，幅度略小，旗幡随杆甩回
        Key(13, {
            "torso": {"pitch": -2, "yaw": 22},
            "head": {"pitch": -6, "yaw": 8},
            "body": {"pitch": -1},
            "rightArm": haft_arm(92, 55, OFFSET_KIT_HOLD, yaw=36, roll=-22),
            "leftArm": arm(pitch=-20, roll=-14, bend=18),
            "rightLeg": {"pitch": 6, "bend": 6},
            "leftLeg": {"pitch": -8, "bend": 4},
        }),
        # 拉回中线：旗杆竖直立在身前一格
        Key(17, {
            "torso": {"pitch": 0, "yaw": 6},
            "head": {"pitch": -3},
            "rightArm": haft_arm(94, 70, OFFSET_KIT_HOLD, yaw=4),
            "leftArm": arm(pitch=-14, roll=-10, bend=16),
            "rightLeg": {"pitch": 5},
            "leftLeg": {"pitch": -7},
        }),
    ],
))


if __name__ == "__main__":
    main()
