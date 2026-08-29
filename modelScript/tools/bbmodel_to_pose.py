#!/usr/bin/env python3
"""把在 Blockbench 里手调过的 `*PlayerAnim.bbmodel` 读回成生成器能吃的 POSE 表。

## 这条路以前是断的

`gen_*_player_anim.py` 只有**去程**：`player_animation/*.json` → 分组 bbmodel，人可以在
Blockbench 里播、拖、改。改完呢？没有回程——手改的姿态卡在 bbmodel 里，`client/tools/
gen_<anim>.py` 那份 POSE 表还是旧的，两边就此分叉。2026-08-26 用户手摆了一帧过顶举棍，
是靠人肉解出四层 group 的合成旋转再抄回去的；这个脚本就是把那次的人肉活儿固化下来。

## 两件不显然的事

1. **单轴分层会被拖 gizmo 打破。** 生成器把每个轴放进单独一层 group（`arm_right_pitch`
   / `_yaw` / `_roll`），为的是绕开 Blockbench 与 MC 的欧拉顺序差异。可人一旦直接拖
   旋转手柄，Blockbench 会把三个轴一起写进**他当时选中的那一层**。所以读回来不能按层
   取值，必须把 roll→yaw→pitch 三层**乘起来**再分解。
2. **bend 层拧出来的 y/z 残差在 MC 里表达不了。** MC 的 bend 只能绕水平轴转。默认遇到
   >1° 的残差就报错，`--tolerate-bend-twist` 可显式接受丢失。
3. **读 Blockbench 存的文件和读我们自己生成的文件，符号不一样。** Blockbench 读入 animation
   通道时对 X/Y 取反、存盘时不取反（见 `core/bb_anim_axes`），所以生成器写文件要预先取反，
   而它存出来的是未取反的内部值。本脚本按 `meta.format_version` 自动区分——生成器只写
   `4.10`，Blockbench 5 存盘一律变成 `5.0`，这也正是本仓「5.0 = 手改过」那条既有判据。
   拿不准就用 `--assume` 显式指定。

## 它靠一条全仓不变量：POSE 的键 == 出料 JSON 的 tick

bbmodel 是从 `player_animation/*.json` 烘出来的，所以这里读到的帧号就是**出料 tick**。
下面把它原样当作 POSE 的键打印出来——这只有在「生成器 POSE 的键 == 出料 JSON 的 tick」
时才是对的。全仓每个动画生成器都满足这条，`PoseTickContractTest`（在
`modelScript/tests/test_bb_anim_roundtrip.py`）逐个扫过去钉住了它
（`anim_common` §重定时 也从另一头写明：落位要写进 POSE 键，不许在出料一步搬帧）。

不满足会怎样：假设某个生成器把 8 tick 的 POSE 在出料时拉长成 10 tick，这里读回来的是
0/3/4/5/6/7/9/10，贴进那份 POSE 就会覆盖掉 0/2/3/4/5/6/7/8，再出料又拉长一次——姿态
一个没错，节奏整条改掉，而且**不报错**。所以那条不变量必须由测试守住，不能靠自觉。

## 用法

    # 看某条动画的全部关键帧
    python3 modelScript/tools/bbmodel_to_pose.py modelScript/models/ClubPlayerAnim.bbmodel \\
        --anim club_smash

    # 只要某一帧（tick），直接贴回 gen_club_smash.py 的 POSE
    python3 modelScript/tools/bbmodel_to_pose.py modelScript/models/ClubPlayerAnim.bbmodel \\
        --anim club_smash --tick 5

    # 和现有 JSON 逐轴比，只列出人改动过的地方
    python3 modelScript/tools/bbmodel_to_pose.py modelScript/models/ClubPlayerAnim.bbmodel \\
        --anim club_smash --diff
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path

import numpy as np

LIB = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(LIB / "core"))
import workspace  # noqa: E402

import bb_anim_axes as AX  # noqa: E402
from animkit import euler_of  # noqa: E402  R = Rz·Ry·Rx 的逆解


def pick_layers(doc: dict, assume: str = "auto"):
    """这份文件该按哪一侧的符号读？→ (layers, 说明)。"""
    if assume == "blockbench":
        return AX.READ_LAYERS, "显式指定：Blockbench 存盘"
    if assume == "generator":
        return AX.WRITE_LAYERS, "显式指定：生成器直出"
    version = str(doc.get("meta", {}).get("format_version", ""))
    if version.startswith("5"):
        return AX.READ_LAYERS, f"format_version {version} → Blockbench 存过盘"
    return AX.WRITE_LAYERS, f"format_version {version} → 生成器直出，未经 Blockbench"

_WS = workspace.Workspace.discover(start=Path(__file__))
REPO = _WS.root
ANIM_DIR = _WS.player_animations
TICKS_PER_SECOND = 20.0

# MC part 名 → bbmodel 里的 group 前缀。与 `gen_jian_player_anim.PART_GROUPS` 同源，
# 但这里只需要前缀，不需要"有没有 bend 层"（有没有直接看文件里在不在）。
PART_PREFIX = {
    "head": "head", "torso": "torso",
    "rightArm": "arm_right", "leftArm": "arm_left",
    "rightLeg": "leg_right", "leftLeg": "leg_left",
}
ROUND = 4


def _rotmat(rx: float, ry: float, rz: float) -> np.ndarray:
    """Blockbench / MC 的 bone 顺序 R = Rz·Ry·Rx。"""
    def one(deg, axis):
        c, s = math.cos(math.radians(deg)), math.sin(math.radians(deg))
        if axis == 0:
            return np.array([[1, 0, 0], [0, c, -s], [0, s, c]])
        if axis == 1:
            return np.array([[c, 0, s], [0, 1, 0], [-s, 0, c]])
        return np.array([[c, -s, 0], [s, c, 0], [0, 0, 1]])
    return one(rz, 2) @ one(ry, 1) @ one(rx, 0)


def _animator(anim: dict, bone: str):
    for entry in anim["animators"].values():
        if entry.get("name") == bone:
            return entry
    return None


def _value_at(anim: dict, bone: str, channel: str, tick: float):
    """该 bone 在 `tick` 上的关键帧值；没有正好落在这个 tick 的就返回 None。

    **只取关键帧、不插值**——回程要的是"人摆的那一帧"，插出来的中间值没有权威性。
    """
    entry = _animator(anim, bone)
    if entry is None:
        return None
    want = tick / TICKS_PER_SECOND
    for kf in _keyframes(entry):
        if kf["channel"] == channel and abs(kf["time"] - want) < 1e-6:
            point = kf["data_points"][0]
            return [float(point.get(axis, 0) or 0) for axis in "xyz"]
    return None


def _keyframes(entry: dict) -> list:
    """animator 的关键帧列表；**没有关键帧的骨头连 `keyframes` 键都没有**。

    Blockbench 会给「在动画模式下点选过、但一帧没打」的骨头也写一条 animator 记录，形如
    `{"name": ..., "type": "bone", "rotation_global": false, ...}`——`keyframes` 整个缺席。
    本仓的 `club_right_*`（棍子挂在手上的静态转接层）就是这种。直接下标会 KeyError。
    """
    return entry.get("keyframes", [])


def keyframe_ticks(anim: dict) -> list[float]:
    times = {kf["time"] for entry in anim["animators"].values() for kf in _keyframes(entry)}
    return sorted(round(t * TICKS_PER_SECOND, 4) for t in times)


def _axes_from_euler(triple, layers) -> dict:
    """合成好的 bb 欧拉三元组 → MC 轴，按指定那一侧的符号。"""
    return {name: float(triple[index]) / sign for name, index, sign in layers}


def _position_from(triple, layers) -> dict:
    """bb position → MC 米。

    position 的 X 与 rotation 的 X **取反关系相反**：写侧 rotation 写 `+pitch` 而
    position 写 `-x`；读侧反过来。所以这里取 pitch 那一层符号的**负数**。
    """
    x_sign = -next(sign for name, _i, sign in layers if name == "pitch")
    return {"x": float(triple[0]) * x_sign / AX.PX_PER_BLOCK,
            "y": -float(triple[1]) / AX.PX_PER_BLOCK,
            "z": float(triple[2]) / AX.PX_PER_BLOCK}


def _bend_from(bb_x: float, layers) -> tuple[float, float]:
    x_sign = next(sign for name, _i, sign in layers if name == "pitch")
    value = float(bb_x) * x_sign
    if abs(value) < 1e-9:
        return 0.0, 180.0
    return (-value, 180.0) if value < 0 else (value, 0.0)


def read_pose(anim: dict, tick: float, tolerate_twist: bool = False,
              layers=None) -> dict:
    """→ `{part: {pitch,yaw,roll,bend,axis}, "_body": {...}}`，MC 轴、度。

    `layers` 决定按哪一侧的符号解，默认读侧（Blockbench 存盘）。用 `pick_layers()` 取。
    """
    layers = layers or AX.READ_LAYERS
    pose: dict = {}
    for part, prefix in PART_PREFIX.items():
        matrix = np.eye(3)
        seen = False
        for layer in reversed(AX.AXIS_ORDER):          # 外 roll → 内 pitch
            triple = _value_at(anim, f"{prefix}_{layer}", "rotation", tick)
            if triple is not None:
                matrix = matrix @ _rotmat(*triple)
                seen = True
        if not seen:
            continue
        axes = {k: round(v, ROUND) for k, v in _axes_from_euler(euler_of(matrix), layers).items()}
        bend_triple = _value_at(anim, f"{prefix}_bend", "rotation", tick)
        if bend_triple is not None:
            if not tolerate_twist and max(abs(bend_triple[1]), abs(bend_triple[2])) > 1.0:
                AX.assert_pure_x(bend_triple, where=f"{part} @ tick {tick:g}")
            bend, axis = _bend_from(bend_triple[0], layers)
            axes["bend"] = round(bend, ROUND)
            axes["axis"] = axis
        # part 级位移烘在**最外层**那一层 group 上（与生成器同一处），别忘了读
        offset = _value_at(anim, f"{prefix}_{AX.AXIS_ORDER[-1]}", "position", tick)
        if offset is not None:
            axes.update({k: round(v, 5) for k, v in _position_from(offset, layers).items()
                         if abs(v) > 1e-9})
        pose[part] = axes

    body: dict = {}
    position = _value_at(anim, "root_pos", "position", tick)
    if position is not None:
        body.update({k: round(v, 5) for k, v in _position_from(position, layers).items()})
    matrix = np.eye(3)
    seen = False
    for layer in reversed(AX.AXIS_ORDER):
        triple = _value_at(anim, f"root_{layer}", "rotation", tick)
        if triple is not None:
            matrix = matrix @ _rotmat(*triple)
            seen = True
    if seen:
        body.update({k: round(v, ROUND)
                     for k, v in _axes_from_euler(euler_of(matrix), layers).items()})
    if body:
        pose["_body"] = body
    return pose


# ── 输出 ──────────────────────────────────────────────────────────────────


def _fmt(value: float) -> str:
    """整数写成整数，其余留小数——**人摆出来的角度不许凑整**，凑了就是偷改姿态。"""
    if abs(value - round(value)) < 1e-6:
        return f"{int(round(value)):+d}"
    return f"{value:+.4g}"


def as_pose_source(pose: dict, tick: float, easing: str = "INOUTSINE") -> str:
    """打成能直接贴进 `gen_<anim>.py` POSE 表的 Python 源码。"""
    lines = [f"    {tick:g}: dict(", f'        easing="{easing}",']
    body = pose.get("_body")
    if body:
        parts = ", ".join(f"{k}={_fmt(v)}" for k, v in body.items() if abs(v) > 1e-9 or k in "xyz")
        lines.append(f"        body=dict({parts}),")
    for part in ("head", "torso", "rightArm", "leftArm", "rightLeg", "leftLeg"):
        axes = pose.get(part)
        if not axes:
            continue
        order = [k for k in ("pitch", "yaw", "roll", "bend", "axis") if k in axes]
        parts = ", ".join(f"{k}={_fmt(axes[k])}" for k in order)
        lines.append(f"        {part}=dict({parts}),")
    lines.append("    ),")
    return "\n".join(lines)


def load_json_pose(name: str, tick: float) -> dict:
    """现网 `player_animation/<name>.json` 在同一 tick 的姿态，用来做 --diff。"""
    sys.path.insert(0, str(LIB / "core"))
    import render_player_pose as RP

    _n, _e, table = RP.anim_pose_table(ANIM_DIR / f"{name}.json")
    for source_tick, pose in table:
        if abs(source_tick - tick) < 1e-6:
            return pose
    return {}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("bbmodel", type=Path)
    parser.add_argument("--anim", required=True, help="bbmodel 里的动画名")
    parser.add_argument("--tick", type=float, default=None, help="只看某一帧（默认全部）")
    parser.add_argument("--diff", action="store_true",
                        help="和现网 player_animation JSON 逐轴比，只列出改动过的")
    parser.add_argument("--tolerate-bend-twist", action="store_true",
                        help="bend 层的 y/z 残差表达不了，默认报错；加这个显式接受丢失")
    parser.add_argument("--assume", choices=("auto", "blockbench", "generator"),
                        default="auto",
                        help="这份文件按哪一侧符号读；默认按 meta.format_version 自动判")
    args = parser.parse_args()

    doc = json.loads(args.bbmodel.read_text(encoding="utf-8"))
    anims = {a["name"]: a for a in doc.get("animations", [])}
    if args.anim not in anims:
        raise SystemExit(f"{args.bbmodel} 里没有动画 {args.anim!r}；有的是 {sorted(anims)}")
    anim = anims[args.anim]
    layers, why = pick_layers(doc, args.assume)
    ticks = [args.tick] if args.tick is not None else keyframe_ticks(anim)

    if args.diff:
        print(f"# {args.anim}：bbmodel 与 player_animation/{args.anim}.json 的逐轴差异")
        print(f"# 读取口径：{why}")
        clean = True
        for tick in ticks:
            mine = read_pose(anim, tick, args.tolerate_bend_twist, layers)
            theirs = load_json_pose(args.anim, tick)
            for part in sorted(set(mine) | set(theirs)):
                a = mine.get(part, {})
                b = theirs.get(part, {})
                for axis in sorted(set(a) | set(b)):
                    if axis == "axis":
                        continue
                    va, vb = float(a.get(axis, 0.0)), float(b.get(axis, 0.0))
                    if abs(va - vb) > 0.05:
                        clean = False
                        print(f"  t{tick:<5g} {part:9s} {axis:6s} "
                              f"JSON {vb:+9.3f}  →  bbmodel {va:+9.3f}   Δ{va - vb:+.3f}")
        if clean:
            print("  （无差异：bbmodel 和 JSON 一致）")
        return 0

    print(f"# 从 {args.bbmodel.name} 读回 {args.anim}，可直接贴进 gen_{args.anim}.py 的 POSE")
    print(f"# 读取口径：{why}")
    print("# ⚠ easing 是 bbmodel 表达不了的，下面一律填占位值，贴回去要自己改")
    for tick in ticks:
        print(as_pose_source(read_pose(anim, tick, args.tolerate_bend_twist, layers), tick))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
