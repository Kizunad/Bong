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
            "rightArm": arm(pitch=-50, bend=22),
            "leftArm": arm(pitch=8, roll=-14, bend=26),
            "rightLeg": {"pitch": 12, "bend": 20},
            "leftLeg": {"pitch": -22, "bend": 14},
        }, easing="INOUTSINE"),
        # overshoot / 砍入后顿一下
        Key(8, {
            "torso": {"pitch": 36, "yaw": -16},
            "head": {"pitch": 10},
            "body": {"pitch": 9},
            "rightArm": arm(pitch=-44, bend=14),
            "leftArm": arm(pitch=10, roll=-14, bend=28),
            "rightLeg": {"pitch": 14, "bend": 30},
            "leftLeg": {"pitch": -26, "bend": 22},
        }),
        Key(9, {
            "torso": {"pitch": 35, "yaw": -15},
            "head": {"pitch": 10},
            "body": {"pitch": 9},
            "rightArm": arm(pitch=-45, bend=15),
            "leftArm": arm(pitch=10, roll=-14, bend=28),
            "rightLeg": {"pitch": 14, "bend": 30},
            "leftLeg": {"pitch": -26, "bend": 22},
        }),
    ],
))


if __name__ == "__main__":
    main()
