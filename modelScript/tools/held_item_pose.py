#!/usr/bin/env python3
"""手持物动画的**解算与量测**台：给定"件的尖端该到哪儿"，反查手臂角度；给定动画，量出
件走了什么轨迹。

## 为什么需要它

手持物的朝向**不是**手臂的朝向。display 变换（`held_item_common.hand_display`）里的
`Rx(-80)` 让件沿前臂出虎口，于是 `pitch` 和 `bend` 在同一个旋向上**相加**决定件的指向。
「肘开一点件就更低」这种直觉是错的——木棍那条抡砸第一版按"手臂往前伸就是砸"写了
`pitch=-58 / bend=34`，量出来棍仰角是 **+15.7°、还朝上**，整条动画读成"举着棍往前捅"。

所以手持物的姿态要**解**，不要调。这个模块把当时现搭的那套扫格子固化下来。

## 三个入口

    # 1) 单点：这组角度把件摆到哪儿？
    python3 modelScript/tools/held_item_pose.py eval --item wooden_club \\
        --pitch -82.7 --yaw -20 --roll 12.1 --bend 92.4

    # 2) 反查：我要件头在肩上方 12~15px、贴近中线、仰角 50~80°
    #    ⚠ 负数区间必须写成 --right=-3:3（不带等号的话 argparse 会把 -3:3 当成选项名）
    python3 modelScript/tools/held_item_pose.py solve --item wooden_club \\
        --up 12:15 --right=-3:3 --elev 50:80 --hand-forward 1.0

    # 3) 量一条动画：件头三轴行程 / 峰速落在哪一 tick / 速度剖面
    python3 modelScript/tools/held_item_pose.py track --item wooden_club --anim club_smash

坐标口径一律是 **ModelPart 空间、相对右肩枢轴**，且已翻好符号：
`右+ / 上+ / 前+`（正数就是往右、往上、朝敌人）。
"""

from __future__ import annotations

import argparse
import importlib
import json
import math
import sys
from pathlib import Path

import numpy as np

LIB = Path(__file__).resolve().parents[1]
REPO = LIB.parent
for _d in (LIB / "core", LIB / "generators", LIB / "tools", REPO / "client" / "tools"):
    sys.path.insert(0, str(_d))

import preview_player_anim as P  # noqa: E402
import render_animation as RA  # noqa: E402

ANIM_DIR = REPO / "client" / "src" / "main" / "resources" / "assets" / "bong" / "player_animation"
SHOULDER = np.array(P.PIVOT_OF["rightArm_lo"], float)

# template_id → (生成器模块, 模块里的 HeldItem 变量名)
ITEMS = {
    "wooden_club": ("gen_wooden_club", "WOODEN_CLUB"),
    "stone_knife": ("gen_knife_trio", "STONE_KNIFE"),
    "iron_dagger": ("gen_knife_trio", "IRON_DAGGER"),
    "bone_spike": ("gen_knife_trio", "BONE_SPIKE"),
}


def load_item(key: str):
    if key not in ITEMS:
        raise SystemExit(f"不认识 {key!r}；已登记的是 {sorted(ITEMS)}")
    module_name, attr = ITEMS[key]
    item = getattr(importlib.import_module(module_name), attr)
    length = max(b.high[1] for b in item.boxes)
    # 出料系 px：`emit_offset` 已把握把点放到方块中心 (8,8,8)
    tip = np.array([8.0, 8.0 + (length - item.grip) * 16.0, 8.0, 1.0])
    grip = np.array([8.0, 8.0, 8.0, 1.0])
    return item, item.display["thirdperson_righthand"], tip, grip


def _mc(vec) -> tuple[float, float, float]:
    """ModelPart 向量 → (右+, 上+, 前+)。+X 是玩家左、+Y 朝下、+Z 朝身后。"""
    return (-float(vec[0]), -float(vec[1]), -float(vec[2]))


def evaluate(display, tip, grip, pitch, yaw, roll, bend, axis=180.0):
    """→ (件头, 手, 件仰角°)，都相对右肩。"""
    kfs = {"rightArm": {k: [(0, math.radians(v), "LINEAR")] for k, v in
                        dict(pitch=pitch, yaw=yaw, roll=roll, bend=bend, axis=axis).items()}}
    matrix = P.item_attach_modelpart(kfs, 0.0, display)
    direction = matrix[:3, :3] @ np.array([0.0, 1.0, 0.0])
    direction /= np.linalg.norm(direction)
    return (_mc((matrix @ tip)[:3] - SHOULDER),
            _mc((matrix @ grip)[:3] - SHOULDER),
            math.degrees(math.asin(-direction[1])))


def _range(text: str | None, default=(-1e9, 1e9)) -> tuple[float, float]:
    if not text:
        return default
    lo, _, hi = text.partition(":")
    return (float(lo) if lo else default[0], float(hi) if hi else default[1])


def solve(display, tip, grip, *, up, right, forward, elev, bend_range,
          hand_forward, top=12, step=5):
    """扫格子反查手臂角度。**穷举不是偷懒**——pitch/yaw/roll/bend 四轴到件头位置的映射
    既非线性也非单射，梯度法只会掉进局部解；这个空间小到可以直接扫完。"""
    found = []
    for pitch in range(-100, 61, step):
        for yaw in range(-56, 57, step + 1):
            for roll in range(-56, 57, step + 1):
                for bend in range(int(bend_range[0]), int(bend_range[1]) + 1, step + 1):
                    head, hand, elevation = evaluate(display, tip, grip,
                                                     pitch, yaw, roll, bend)
                    if not (up[0] <= head[1] <= up[1]):
                        continue
                    if not (right[0] <= head[0] <= right[1]):
                        continue
                    if not (forward[0] <= head[2] <= forward[1]):
                        continue
                    if not (elev[0] <= elevation <= elev[1]):
                        continue
                    if hand[2] < hand_forward:
                        continue
                    centre = (sum(up) / 2, sum(right) / 2, sum(elev) / 2)
                    score = (abs(head[1] - centre[0]) + abs(head[0] - centre[1])
                             + abs(elevation - centre[2]) * 0.1)
                    found.append((score, pitch, yaw, roll, bend, elevation, head, hand))
    found.sort(key=lambda row: row[0])
    return found[:top]


def track(display, tip, name: str, samples: int = 0):
    emote = json.loads((ANIM_DIR / f"{name}.json").read_text(encoding="utf-8"))["emote"]
    kfs = RA.collect_keyframes(emote)
    end = float(emote["endTick"])
    n = samples or int(end * 16) + 1
    points = np.array([_mc((P.item_attach_modelpart(kfs, end * i / (n - 1), display) @ tip)[:3]
                           - SHOULDER) for i in range(n)])
    dt = end / (n - 1)
    speed = np.linalg.norm(np.diff(points, axis=0), axis=1) / dt
    return end, points, speed, dt


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="cmd", required=True)

    common = argparse.ArgumentParser(add_help=False)
    common.add_argument("--item", default="wooden_club", help=f"已登记：{sorted(ITEMS)}")

    one = sub.add_parser("eval", parents=[common], help="单点求值")
    for axis, default in (("pitch", 0.0), ("yaw", 0.0), ("roll", 0.0), ("bend", 0.0)):
        one.add_argument(f"--{axis}", type=float, default=default)
    one.add_argument("--axis", type=float, default=180.0)

    hunt = sub.add_parser("solve", parents=[common], help="按目标反查手臂角度")
    for flag, helptext in (("up", "件头相对肩的高度区间，如 12:15"),
                           ("right", "件头左右区间（右为正）"),
                           ("forward", "件头前后区间（朝敌人为正）"),
                           ("elev", "件的仰角区间（度）"),
                           ("bend", "肘弯搜索区间，默认 8:140")):
        hunt.add_argument(f"--{flag}", default=None, help=helptext)
    hunt.add_argument("--hand-forward", type=float, default=-1e9,
                      help="手至少要在肩前多少 px（FPV 可见性，见 conventions §3）")
    hunt.add_argument("--top", type=int, default=12)

    walk = sub.add_parser("track", parents=[common], help="量一条动画的件头轨迹")
    walk.add_argument("--anim", required=True)
    walk.add_argument("--profile", action="store_true", help="打印速度剖面")

    args = parser.parse_args()
    _item, display, tip, grip = load_item(args.item)

    if args.cmd == "eval":
        head, hand, elevation = evaluate(display, tip, grip, args.pitch, args.yaw,
                                         args.roll, args.bend, args.axis)
        print(f"仰角 {elevation:+.1f}°")
        print(f"件头  右{head[0]:+6.1f}  上{head[1]:+6.1f}  前{head[2]:+6.1f}")
        print(f"手    右{hand[0]:+6.1f}  上{hand[1]:+6.1f}  前{hand[2]:+6.1f}")
        return 0

    if args.cmd == "solve":
        rows = solve(display, tip, grip,
                     up=_range(args.up), right=_range(args.right),
                     forward=_range(args.forward), elev=_range(args.elev),
                     bend_range=_range(args.bend, (8.0, 140.0)),
                     hand_forward=args.hand_forward, top=args.top)
        if not rows:
            print("没有满足条件的姿态——把区间放宽，或者这个目标手臂根本够不到")
            return 1
        print("  pitch  yaw roll bend    仰角      件头 右/上/前         手 右/上/前")
        for _s, pitch, yaw, roll, bend, elevation, head, hand in rows:
            print(f"  {pitch:+5d}{yaw:+5d}{roll:+5d}{bend:5d}  {elevation:+6.1f}°   "
                  f"{head[0]:+6.1f} {head[1]:+6.1f} {head[2]:+6.1f}   "
                  f"{hand[0]:+5.1f} {hand[1]:+5.1f} {hand[2]:+5.1f}")
        return 0

    end, points, speed, dt = track(display, tip, args.anim)
    lateral = points[:, 0].max() - points[:, 0].min()
    vertical = points[:, 1].max() - points[:, 1].min()
    forward = points[:, 2].max() - points[:, 2].min()
    print(f"{args.anim}  endTick={end:g}")
    print(f"  件头行程   横 {lateral:5.1f}   竖 {vertical:5.1f}   前后 {forward:5.1f}"
          f"   竖/横 {vertical / max(lateral, 1e-6):.2f}")
    peak = int(speed.argmax())
    print(f"  峰速 {speed.max():.1f} px/tick @ t{end * peak / (len(speed)):.2f}")
    if args.profile:
        step = max(1, len(speed) // 24)
        for i in range(0, len(speed), step):
            bar = "█" * int(round(speed[i] / speed.max() * 40))
            print(f"   t{end * i / len(speed):5.2f}  {speed[i]:6.1f} {bar}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
