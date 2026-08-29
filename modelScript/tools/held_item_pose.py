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
import functools
import importlib
import json
import math
import sys
from pathlib import Path

import numpy as np

LIB = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(LIB / "core"))
import workspace  # noqa: E402

_WS = workspace.Workspace.discover(start=Path(__file__))
REPO = _WS.root
for _d in (LIB / "core", LIB / "generators", LIB / "tools", REPO / "client" / "tools"):
    sys.path.insert(0, str(_d))

import preview_player_anim as P  # noqa: E402
import render_animation as RA  # noqa: E402

ANIM_DIR = _WS.player_animations
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


def _axis_span(near, tol, key, default):
    """`--near` 给的邻帧值 ± 容差；没给就用默认全域。"""
    if near is None or key not in near:
        return default
    return (near[key] - tol, near[key] + tol)


def solve(display, tip, grip, *, up, right, forward, elev, bend_range,
          hand_forward, top=12, step=5, near=None, near_tol=30.0, two_hand=None):
    """扫格子反查手臂角度。**穷举不是偷懒**——pitch/yaw/roll/bend 四轴到件头位置的映射
    既非线性也非单射，梯度法只会掉进局部解；这个空间小到可以直接扫完。

    `near` 传上一帧的姿态时，搜索被夹在它 ±`near_tol` 的盒子里。这解决的是**帧间连续
    性**：件头位置只约束「棍在哪儿」，同一个位置往往有一堆解，其中不少要求前臂在两帧之间
    反向拧七八十度——数值全部达标，动起来是抽搐。约束到邻帧附近就把这类解筛掉了。

    `two_hand` 传 `{"item": ..., "span": ..., "tol": ...}` 时，只留副手也搭得上棍身的解。
    """
    if near_tol <= 0:
        raise ValueError(
            f"near_tol 是 `--near` 盒子的半径，必须为正，收到 {near_tol!r}"
            "——负数会让区间首尾颠倒，扫描直接空转，读起来却像'这个目标够不到'")
    p_lo, p_hi = _axis_span(near, near_tol, "pitch", (-100, 60))
    y_lo, y_hi = _axis_span(near, near_tol, "yaw", (-56, 56))
    r_lo, r_hi = _axis_span(near, near_tol, "roll", (-56, 56))
    found = []
    for pitch in range(int(p_lo), int(p_hi) + 1, step):
        for yaw in range(int(y_lo), int(y_hi) + 1, step + 1):
            for roll in range(int(r_lo), int(r_hi) + 1, step + 1):
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
    if two_hand is None:
        return found[:top]
    # 双手招式：件头位置达标还不够，副手还得够得着棍身。这一层**不能事后补**——满足位置
    # 约束的解里有一大半是「单手甩出去」的姿态，左臂再怎么摆也搭不上，等选完再发现就得
    # 推翻重来（木棍横抡的撞击帧实测最好也差 8.4px，而一个拳头才 4px 宽）。
    kept = []
    for row in found:
        right_pose = dict(pitch=row[1], yaw=row[2], roll=row[3], bend=row[4], axis=180)
        best = solve_off_hand(two_hand["item"], display, right_pose,
                              span=two_hand.get("span", (0.28, 0.55)),
                              top=1, step=two_hand.get("step", 12))
        if best and best[0][0] <= two_hand.get("tol", 2.0):
            kept.append(row + (best[0],))
        if len(kept) >= top:
            break
    return kept


def _kfs(**parts) -> dict:
    """把「一个 tick 的姿态」搭成 `render_animation` 认的关键帧结构。

    `axis`（bendDirection）和三个转轴一样按弧度存——`item_attach_modelpart` 直接拿它去算
    `cos(-axis)`，塞度数进去会让折弯方向变成一个几乎随机的斜轴。
    """
    out = {}
    for part, axes in parts.items():
        if not axes:
            continue
        out[part] = {k: [(0, math.radians(v) if k not in ("x", "y", "z") else float(v),
                          "LINEAR")]
                     for k, v in axes.items()}
    return out


def wrist(kfs, side: str, tick: float = 0.0) -> np.ndarray:
    """腕（手持物挂点）在 ModelPart 世界系的位置。

    走的是 `item_attach_modelpart` 的**同一条链**、到 `T(hand)` 为止，所以右腕算出来必然
    落在棍的握把附近——两边用不同近似的话，「副手够不够得到棍」就没法比。
    """
    part = RA.sample_part(kfs, side, tick)
    pivot = (np.array(P.PIVOT_OF[f"{side}_lo"], float)
             + np.array([part["x"], part["y"], part["z"]], float))
    R_arm = RA.part_rotation_matrix(part["pitch"], part["yaw"], part["roll"])
    axis = float(part["axis"])
    R_bend = RA.rotate_about_axis(
        np.array([np.cos(-axis), 0.0, np.sin(-axis)]), float(part["bend"]))
    hand = P.HAND_OFFSET_PX * (1.0 if side == "rightArm" else np.array([-1.0, 1.0, 1.0]))
    matrix = (P._aff(np.eye(3), pivot) @ P._aff(R_arm, np.zeros(3))
              @ P._about(R_bend, P.ITEM_BEND_PIVOT_PX)
              @ P._aff(P.R_ATTACH, np.zeros(3)) @ P._aff(np.eye(3), hand))
    return (matrix @ np.array([0.0, 0.0, 0.0, 1.0]))[:3]


def shaft(kfs, item, display, tick: float = 0.0):
    """棍身线段 (棍尾, 棍头)，ModelPart 世界系。"""
    matrix = P.item_attach_modelpart(kfs, tick, display)
    length = max(b.high[1] for b in item.boxes)
    butt = (matrix @ np.array([8.0, 8.0 - item.grip * 16.0, 8.0, 1.0]))[:3]
    head = (matrix @ np.array([8.0, 8.0 + (length - item.grip) * 16.0, 8.0, 1.0]))[:3]
    return butt, head


def on_shaft(point, butt, head):
    """→ (到轴线的距离 px, 落在棍身的比例 0~1)。比例夹在两端，握到棍外不算握住。"""
    span = head - butt
    frac = float(np.clip(np.dot(point - butt, span) / np.dot(span, span), 0.0, 1.0))
    return float(np.linalg.norm(point - (butt + span * frac))), frac


# 副手搜索域。yaw/roll 给到 ±90 是必须的：副手横过身体去够扫到另一侧的棍时，肩要外展到
# 接近极限。第一版沿用右臂那套 ±56，最优解全部顶在 yaw=+60 的格子边界上、离棍还有 7px——
# 那不是「够不着」，是**格子画小了**，读成够不着就会去改姿态，把本来对的设计推翻。
OFF_HAND_YAW = 90
OFF_HAND_ROLL = 90


@functools.lru_cache(maxsize=8)
def _off_hand_table(step: int, pitch_range: tuple, bend_range: tuple):
    """左腕位置表：`(姿态 N×4, 腕位 N×3)`。

    左腕在哪儿**只取决于左臂自己的四个角度**，和右手拿什么、摆哪儿完全无关。所以这张表
    算一次就够，之后每个右臂候选只是拿它做一次向量化的点到线段距离。第一版没缓存，每个
    候选重扫一遍网格，`solve --two-hand` 跑了两分钟没出结果。
    """
    poses, points = [], []
    for pitch in range(int(pitch_range[0]), int(pitch_range[1]) + 1, step):
        for yaw in range(-OFF_HAND_YAW, OFF_HAND_YAW + 1, step):
            for roll in range(-OFF_HAND_ROLL, OFF_HAND_ROLL + 1, step):
                for bend in range(int(bend_range[0]), int(bend_range[1]) + 1, step):
                    poses.append((pitch, yaw, roll, bend))
                    points.append(wrist(_kfs(leftArm=dict(
                        pitch=pitch, yaw=yaw, roll=roll, bend=bend, axis=180)), "leftArm"))
    return np.array(poses, float), np.array(points, float)


def solve_off_hand(item, display, right, *, span=(0.28, 0.55), top=8, step=6,
                   pitch_range=(-110, 40), bend_range=(20, 150)):
    """双手握棍：右手姿态定死棍在哪儿，反查**左臂**该怎么摆才能握在棍身上。

    这是单手招式没有的约束。手持物挂在右手上，左手是自由的——想让它「也握着棍」，就得让
    左腕落到棍身线段上。眼睛摆不准：左腕差 3px 就是「手悬在棍旁边」，而 3px 在正视图里
    只有一个像素多点，截图上看不出来，进游戏一转视角就露馅。

    `span` 是允许的握点范围（沿棍身从尾到头的比例）。
    """
    butt, head = shaft(_kfs(rightArm=right), item, display)
    poses, points = _off_hand_table(step, tuple(pitch_range), tuple(bend_range))
    axis = head - butt
    frac = np.clip((points - butt) @ axis / float(axis @ axis), 0.0, 1.0)
    dist = np.linalg.norm(points - (butt + frac[:, None] * axis), axis=1)
    ok = np.flatnonzero((frac >= span[0]) & (frac <= span[1]))
    if not ok.size:
        return []
    order = ok[np.argsort(dist[ok])][:top]
    return [(float(dist[i]), int(poses[i][0]), int(poses[i][1]), int(poses[i][2]),
             int(poses[i][3]), float(frac[i])) for i in order]


def solve_off_hand_chain(item, display, right_poses, *, span=(0.20, 0.65),
                         near_tol=34.0, step=6, tol=3.0, seed=None):
    """整条右臂轨迹 → 逐帧解出**连贯**的副手。

    单帧各解各的会得到一串互不相干的最优解——每帧都贴着棍，串起来却是左臂在乱抽。所以
    这里按帧推进，让"贴棍"和"跟上一帧连贯"一起进目标函数。首帧无约束（没有"上一帧"）。

    **`near_tol` 是权重，不是闸门——这是有意的，别改成硬过滤。** 它的作用是把"转了多少
    度"折算成"离棍多少 px"（`swing / near_tol` 加进距离分）。一次真的抡击本来就要求手臂
    在两 tick 内转过大角度：拿 `near_tol=34`（产出 club_sweep 时的默认值）硬过滤，撞击帧
    和 overshoot 帧会直接判成无解——**工具会报"这一招做不出来"，而那一招已经出料了**。
    转角不掩盖：它作为第 7 项 `swing` 原样返回，`chain` 子命令逐帧打印"本帧最大转角"，
    超没超出人能做到的范围由看的人判断，工具不替他拍板。这条策略由
    `OffHandChainPolicyTest` 钉住。

    真正的硬闸是 `tol`（离轴距离）：够不着棍身的候选直接淘汰，整帧无解就放 `None`。

    返回和 `right_poses` 等长的列表，元素是
    `(dist, pitch, yaw, roll, bend, frac, swing)`；某帧连 `tol` 都满足不了就放 `None`，
    由调用方决定是放宽还是改右臂。
    """
    if near_tol <= 0:
        raise ValueError(
            f"near_tol 是帧间转角折算成 px 的分母，必须为正，收到 {near_tol!r}"
            "——传 0 会当场除零，传负数会把'转得越多分越高'，静默解出乱抽的副手")
    out, prev = [], seed
    for right in right_poses:
        rows = [r for r in solve_off_hand(item, display, right, span=span,
                                          top=4000, step=step)
                if r[0] <= tol]
        if not rows:
            out.append(None)
            continue
        if prev is None:
            best, swing = rows[0], 0.0
        else:
            # 贴棍 + 连贯的**加权取舍**，不是按 near_tol 硬过滤——理由见 docstring
            # 「near_tol 是权重，不是闸门」，硬过滤会把 club_sweep 的撞击帧判成无解。
            # 转角不掩盖：原样作为 swing 返回并逐帧打印。
            scored = [(r[0] + max(abs(r[i + 1] - prev[i + 1]) for i in range(4))
                       / near_tol, r) for r in rows]
            scored.sort(key=lambda row: row[0])
            best = scored[0][1]
            swing = max(abs(best[i + 1] - prev[i + 1]) for i in range(4))
        out.append(best + (swing,))
        prev = best
    return out


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
    hunt.add_argument("--near", default=None,
                      help="邻帧姿态 pitch:yaw:roll，把搜索夹在它附近保证帧间连续")
    hunt.add_argument("--near-tol", type=float, default=30.0, help="--near 的容差（度）")
    hunt.add_argument("--two-hand", action="store_true",
                      help="只留副手也够得着棍身的解（双手招式必开）")
    hunt.add_argument("--span", default="0.28:0.55", help="--two-hand 的握点区间")
    hunt.add_argument("--grip-tol", type=float, default=2.0,
                      help="--two-hand 判定：左腕离棍身轴线最多多少 px")

    hold = sub.add_parser("grip", parents=[common],
                          help="双手握：给定右臂姿态，反查左臂怎么摆才握得到棍身")
    for _axis in ("pitch", "yaw", "roll", "bend"):
        hold.add_argument(f"--{_axis}", type=float, required=True, help=f"右臂 {_axis}")
    hold.add_argument("--span", default="0.28:0.55", help="允许的握点区间（沿棍身比例）")
    hold.add_argument("--top", type=int, default=8)

    link = sub.add_parser("chain", parents=[common],
                          help="整条右臂轨迹 → 逐帧解出连贯的副手")
    link.add_argument("--right", action="append", required=True, metavar="P:Y:R:B",
                      help="一帧右臂姿态，按 tick 顺序重复给出")
    link.add_argument("--span", default="0.20:0.65", help="握点区间")
    link.add_argument("--near-tol", type=float, default=34.0, help="帧间连续容差（度）")
    link.add_argument("--grip-tol", type=float, default=3.0, help="左腕离轴上限 px")
    link.add_argument("--seed", default=None, metavar="P:Y:R:B",
                      help="首帧副手锚定值；不给就取首帧最贴棍的解")

    walk = sub.add_parser("track", parents=[common], help="量一条动画的件头轨迹")
    walk.add_argument("--anim", required=True)
    walk.add_argument("--profile", action="store_true", help="打印速度剖面")
    walk.add_argument("--dump", action="store_true",
                      help="逐 tick 打印棍头位置 + 离头部包围盒多远（判「挡不挡脸」）")
    walk.add_argument("--grip", action="store_true",
                      help="逐 tick 报副手离棍身多远（双手招式的握持连续性）")

    args = parser.parse_args()
    item, display, tip, grip = load_item(args.item)

    if args.cmd == "eval":
        head, hand, elevation = evaluate(display, tip, grip, args.pitch, args.yaw,
                                         args.roll, args.bend, args.axis)
        print(f"仰角 {elevation:+.1f}°")
        print(f"件头  右{head[0]:+6.1f}  上{head[1]:+6.1f}  前{head[2]:+6.1f}")
        print(f"手    右{hand[0]:+6.1f}  上{hand[1]:+6.1f}  前{hand[2]:+6.1f}")
        return 0

    if args.cmd == "grip":
        right_pose = dict(pitch=args.pitch, yaw=args.yaw, roll=args.roll,
                          bend=args.bend, axis=180)
        rows = solve_off_hand(item, display, right_pose, span=_range(args.span),
                              top=args.top)
        butt, head = shaft(_kfs(rightArm=right_pose), item, display)
        rdist, rfrac = on_shaft(wrist(_kfs(rightArm=right_pose), "rightArm"), butt, head)
        print(f"  右腕落在棍身 {rfrac*100:.1f}%（离轴 {rdist:.2f}px）")
        if not rows:
            print("  左臂够不到棍身——右臂姿态本身就把棍甩到了副手够不着的地方")
            return 1
        print("  pitch  yaw roll bend   左腕离轴   握点")
        for dist, pitch, yaw, roll, bend, frac in rows:
            print(f"  {pitch:+5d}{yaw:+5d}{roll:+5d}{bend:5d}   {dist:6.2f}px   {frac*100:5.1f}%")
        return 0

    if args.cmd == "chain":
        rights = []
        for text in args.right:
            pitch, yaw, roll, bend = (float(v) for v in text.split(":"))
            rights.append(dict(pitch=pitch, yaw=yaw, roll=roll, bend=bend, axis=180))
        seed = None
        if args.seed:
            seed = (0.0,) + tuple(int(float(v)) for v in args.seed.split(":")) + (0.0,)
        rows = solve_off_hand_chain(item, display, rights, span=_range(args.span),
                                    near_tol=args.near_tol, tol=args.grip_tol, seed=seed)
        print("  帧   右臂 p/y/r/bend            副手 p/y/r/bend      左腕离轴   握点")
        bad = 0
        for i, (right, best) in enumerate(zip(rights, rows)):
            head = (f"  #{i}  {right['pitch']:+7.1f}{right['yaw']:+7.1f}"
                    f"{right['roll']:+7.1f}{right['bend']:6.1f}")
            if best is None:
                bad += 1
                print(head + "   —— 这一帧副手搭不上棍（放宽 --grip-tol/--near-tol，"
                             "或改右臂）")
                continue
            dist, lp, ly, lr, lb, frac, swing = best
            print(head + f"      {lp:+5d}{ly:+5d}{lr:+5d}{lb:5d}   "
                         f"{dist:6.2f}px   {frac*100:5.1f}%   本帧最大转角 {swing:5.1f}°")
        return 1 if bad else 0

    if args.cmd == "solve":
        near = None
        if args.near:
            near = dict(zip(("pitch", "yaw", "roll"),
                            (float(v) for v in args.near.split(":"))))
        rows = solve(display, tip, grip,
                     up=_range(args.up), right=_range(args.right),
                     forward=_range(args.forward), elev=_range(args.elev),
                     bend_range=_range(args.bend, (8.0, 140.0)),
                     hand_forward=args.hand_forward, top=args.top,
                     near=near, near_tol=args.near_tol,
                     two_hand=(dict(item=item, span=_range(args.span), tol=args.grip_tol)
                               if args.two_hand else None))
        if not rows:
            print("没有满足条件的姿态——把区间放宽，或者这个目标手臂根本够不到"
                  + ("（开了 --two-hand：也可能是副手搭不上棍）" if args.two_hand else ""))
            return 1
        cols = "  pitch  yaw roll bend    仰角      件头 右/上/前         手 右/上/前"
        print(cols + ("      副手 p/y/r/bend   离轴" if args.two_hand else ""))
        for row in rows:
            _s, pitch, yaw, roll, bend, elevation, head, hand = row[:8]
            line = (f"  {pitch:+5d}{yaw:+5d}{roll:+5d}{bend:5d}  {elevation:+6.1f}°   "
                    f"{head[0]:+6.1f} {head[1]:+6.1f} {head[2]:+6.1f}   "
                    f"{hand[0]:+5.1f} {hand[1]:+5.1f} {hand[2]:+5.1f}")
            if args.two_hand:
                dist, lp, ly, lr, lb, frac = row[8]
                line += f"    {lp:+5d}{ly:+5d}{lr:+5d}{lb:5d} {dist:5.2f}px@{frac*100:.0f}%"
            print(line)
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
    if args.dump:
        emote = json.loads((ANIM_DIR / f"{args.anim}.json").read_text(
            encoding="utf-8"))["emote"]
        kfs = RA.collect_keyframes(emote)
        print("  逐 tick 棍头（右/上/前，相对右肩）+ 离头部包围盒：")
        for i in range(int(end * 4) + 1):
            t = i / 4.0
            head_pt = _mc((P.item_attach_modelpart(kfs, t, display) @ tip)[:3] - SHOULDER)
            # 头的轮廓，同一口径（相对右肩）。右肩在中线**右** 5px，所以头心在
            # 右 −5、上 +6 —— 写成 +5 会把整颗头镜像到身体外侧，于是"棍从脸前扫过"
            # 永远测不出来（第一版就是这么漏掉的）。
            # 判据是**正面投影**不是三维相交：棍在脸前方 10px 处横过去并不穿模，可它
            # 就是把脸挡住了，而那正是要禁的东西。所以只比 右/上，再要求棍在身前。
            gap = max(abs(head_pt[0] + 5.0) - 4.0, abs(head_pt[1] - 6.0) - 4.0)
            if head_pt[2] <= 0.0:          # 棍在身后，挡不到脸
                gap = max(gap, 0.01)
            flag = "  ← 挡脸" if gap < 0 else ""
            print(f"    t{t:4.2f}  右{head_pt[0]:+7.1f} 上{head_pt[1]:+7.1f} "
                  f"前{head_pt[2]:+7.1f}   离头 {gap:+6.1f}px{flag}")

    if args.grip:
        emote = json.loads((ANIM_DIR / f"{args.anim}.json").read_text(
            encoding="utf-8"))["emote"]
        kfs = RA.collect_keyframes(emote)
        # 取样到 1/4 tick。**只卡整 tick 会漏**：两条手臂各自在关节空间插值，棍由右臂
        # 带着走，左臂走的是另一条路——关键帧上都对齐，中段照样能甩脱。木棍横抡实测整
        # tick 最远 2.05px，而 t4.5 实际是 4.31px，差了一倍。
        print("  副手握持（1/4 tick 取样）：")
        worst = (0.0, -1.0)
        for i in range(int(end * 4) + 1):
            t = i / 4.0
            butt, head = shaft(kfs, item, display, t)
            dist, frac = on_shaft(wrist(kfs, "leftArm", t), butt, head)
            worst = max(worst, (dist, t))
            if abs(t - round(t)) < 1e-9 or dist > 1.5:
                flag = "  ←" if dist > 1.5 else ""
                print(f"    t{t:4.2f}  左腕离轴 {dist:5.2f}px   握点 {frac*100:5.1f}%{flag}")
        print(f"    最远 {worst[0]:.2f}px @ t{worst[1]:g}")
    if args.profile:
        step = max(1, len(speed) // 24)
        for i in range(0, len(speed), step):
            bar = "█" * int(round(speed[i] / speed.max() * 40))
            print(f"   t{end * i / len(speed):5.2f}  {speed[i]:6.1f} {bar}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
