#!/usr/bin/env python3
"""异变缝合兽 —— 头颅动画自检：把动画里的每条主张变成断言。

这一层的动作全部由头颅层的静态解剖派生（咀嚼行程 = 颌关节高度、合颌时间 = 肌肉缩短
速率、顶角速度 = 角基储能推导的输入）。**派生关系一旦断了，动画看上去还是会动**——
所以必须逐条量回去，而不是"渲一眼觉得挺像"。

十一条：

  ① 下颌不许穿进上颌（只能张不能反折），静止即闭合
  ② 循环动画首末帧必须接得上
  ③ 张口不许超过推出来的最大张口角
  ④ **咀嚼横滑要等于 θ·h**：磨的那一类滑一颗臼齿宽，剪的那一类一丝不滑
  ⑤ 咀嚼的闭合相塞得进一个咀嚼周期（两个独立来源的数对拍：异速拟合 vs 肌肉力学）
  ⑥ 没有耳廓的供体不许出现耳朵动作；没有角的不许有顶角
  ⑦ 威吓时耳朵必须压平——那是护具不是表情
  ⑧ 死亡单调：下颌只垂不回弹（没人拉着了，不是在开合）
  ⑨ 顶角的挥击角速度对得上 `HornStyle.speed`
  ⑩ 每条动作都真的动了东西（防空动画）
  ⑪ **导出产物里的周期数等于推导出来的周期数**

⑪ 是补的，因为前十条集体漏掉了一个真错：这些函数原先按**秒**写曲线，而导出与渲染都
按**归一化相位**喂进来，于是牛的三周期咀嚼实际只嚼了 0.93 下就跳回起点。前十条查不到，
是因为它们自己按秒去调函数——查的是一条从没被导出过的曲线。所以 ⑪ 不重算曲线，它直接
读 `build_tracks` 的输出，也就是**真正写进 .animation.json 的那串关键帧**。

用法:
  python3 modelScript/creatures/stitched_beast/check_head_anim.py
  python3 modelScript/creatures/stitched_beast/check_head_anim.py --donor cow
"""

from __future__ import annotations

import argparse
import math
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "core"))
sys.path.insert(0, str(HERE))

import genome as GN  # noqa: E402
import head_anim as HA  # noqa: E402
import heads as HD  # noqa: E402
from bbmodel_maker.rig.anim_rig import Rig, build_tracks  # noqa: E402

N = 48


def _track(hd, fn, length: float, n: int = N):
    """采一条动作，返回 [(秒, Pose)]。

    **喂进去的是归一化相位**（和 `build_tracks` 一致），返回的时间戳是秒（角速度一类的
    检查要真秒）。两者混用正是 ⑪ 要防的那个错。
    """
    return [(i / n * length, fn(hd, None, i / n, length)) for i in range(n + 1)]


def _peaks(v: list[float]) -> int:
    """一条曲线走了几个来回——数**严格**极大值，平台不重复计数。"""
    n = 0
    for i in range(1, len(v) - 1):
        if v[i] >= v[i - 1] and v[i] > v[i + 1]:
            n += 1
    return n


def _rot(pose, bone: str, idx: int = 0) -> float:
    ch = pose.get(bone)
    return 0.0 if ch is None else ch.rot[idx]


def check(kind: str) -> list[str]:
    bad: list[str] = []
    hd, path = HA.build_head(kind)
    b = HA.bones(hd)
    table = HA.anims(hd)

    # ⑪ 读**导出产物本身**：把 build_tracks 跑一遍，数关键帧序列里的周期数。
    # 循环动画的周期数是推出来的（咀嚼频率 × 时长、呼吸频率 × 时长），导出的曲线必须
    # 一模一样地走那么多个来回。
    rig = Rig(path)
    for name, hz in (("head_chew", HA.chew_hz(hd)), ("head_idle", HA.breath_hz(hd))):
        if name not in table:
            continue
        length, loop, n, fn = table[name]
        tracks = build_tracks(rig, lambda t, f=fn, ln=length: f(hd, rig, t, ln),
                              length, loop, n)
        want = int(HA.cycles(hz, length))
        bone = b["jaw"] if name == "head_chew" else b["head"]
        got = _peaks([v[0] for _t, v in tracks.get(bone, {}).get("rotation", [])])
        if got != want:
            bad.append(f"{name} 导出的曲线走了 {got} 个周期，推出来的是 {want} 个"
                       f"（{hz:.2f} Hz × {length:.2f} s）——写进 .animation.json 的"
                       f"和推导对不上")

    for name, (length, loop, _n, fn) in table.items():
        tr = _track(hd, fn, length)

        # ① 下颌只能张不能反折；每条动作的第一帧都必须是闭合的
        for t, p in tr:
            j = _rot(p, b["jaw"])
            if j < -0.5:
                bad.append(f"{name} t={t:.2f}s 下颌 pitch={j:+.1f}° 是反向的——"
                           f"那是穿进上颌，不是张嘴")
                break
        if abs(_rot(tr[0][1], b["jaw"])) > 1e-6 and name != "head_death":
            bad.append(f"{name} 起手第一帧下颌就没闭上（{_rot(tr[0][1], b['jaw']):+.2f}°）"
                       f"——动作要能从静止接进来")

        # ② 循环动画首末必须接得上
        if loop:
            for bone in set(b.values()):
                a0 = tr[0][1].get(bone)
                a1 = fn(hd, None, 1.0, length).get(bone)
                if a0 is None and a1 is None:
                    continue
                r0 = a0.rot if a0 else [0.0] * 3
                r1 = a1.rot if a1 else [0.0] * 3
                p0 = a0.pos if a0 else [0.0] * 3
                p1 = a1.pos if a1 else [0.0] * 3
                if max(abs(x - y) for x, y in zip(r0 + p0, r1 + p1)) > 0.6:
                    bad.append(f"{name} 的 {bone} 首末帧对不上——循环会跳一下")
                    break

        # ③ 张口不许超过推出来的上限
        mx = max(_rot(p, b["jaw"]) for _t, p in tr)
        if mx > hd.gape + 0.6:
            bad.append(f"{name} 张口 {mx:.1f}° 超过推出来的最大张口角 {hd.gape:.1f}°")

        # ⑩ 空动画
        moved = any(ch.moved() for _t, p in tr for ch in p.values())
        if not moved:
            bad.append(f"{name} 一帧都没动——空动画")

    # ④ 咀嚼横滑 = θ·h
    if "head_chew" in table:
        length = table["head_chew"][0]
        tr = _track(hd, HA.anim_chew, length)
        lat = max(abs(p.get(b["jaw"]).pos[0]) if p.get(b["jaw"]) else 0.0 for _t, p in tr)
        want, _ = HA.chew_shift(hd)
        if hd.diet.occlusion == "grind":
            if want <= 1e-6:
                bad.append(f"{kind} 是磨牙型却算出零横滑——颌关节高度那条推导断了")
            elif abs(lat - want) > 0.05 + want * 0.1:
                bad.append(f"{kind} 咀嚼横滑 {lat:.2f} px ≠ 推出来的 θ·h = {want:.2f} px")
        elif lat > 0.02:
            bad.append(f"{kind} 是 {hd.diet.occlusion} 型却横滑 {lat:.2f} px——"
                       f"剪切的关节在咬合面上，滑动会把刃磨钝，必须为 0")

        # ⑤ 闭合相塞得进一个周期
        t_close = HA.close_time(hd, force=True)
        period = 1.0 / max(HA.chew_hz(hd), 1e-6)
        if t_close > period * 0.75:
            bad.append(f"{kind} 出力冲程闭合要 {t_close * 1000:.0f} ms，而一个咀嚼周期只有"
                       f" {period * 1000:.0f} ms——异速拟合的频率和肌肉力学推的闭合时间"
                       f"对不上，两者必有一个错")

    # ⑥ 没有的部件不许有动作
    if not hd.donor.pinna and ("ear_l" in b or "ear_r" in b):
        bad.append(f"{kind} 没有外耳廓却建了耳骨")
    if not hd.donor.horn and "head_butt" in table:
        bad.append(f"{kind} 没有角却有顶角动作")
    if hd.donor.horn and "head_butt" not in table:
        bad.append(f"{kind} 有角却没有顶角动作")
    if hd.diet.occlusion == "gulp" and "head_chew" in table:
        bad.append(f"{kind} 是整吞型却有咀嚼动作")

    # ⑦ 威吓时耳朵压平
    if hd.donor.pinna and "head_threat" in table:
        length = table["head_threat"][0]
        tr = _track(hd, HA.anim_threat, length)
        flat = min(_rot(p, b.get("ear_l", "")) for _t, p in tr)
        if flat > -40.0:
            bad.append(f"{kind} 威吓时耳朵只压到 {flat:.0f}°——耳廓是打起来最先被撕掉的"
                       f"部件，威吓姿态必须先把它收起来")

    # ⑧ 死亡单调
    length = table["head_death"][0]
    tr = _track(hd, HA.anim_death, length)
    seq = [_rot(p, b["jaw"]) for _t, p in tr]
    for i in range(1, len(seq)):
        if seq[i] < seq[i - 1] - 0.3:
            bad.append(f"{kind} 死亡动作里下颌回收了（{seq[i - 1]:.1f}→{seq[i]:.1f}°）——"
                       f"没人拉着的下颌只会垂下去")
            break

    # ⑨ 顶角挥击角速度对得上 HornStyle.speed
    if hd.donor.horn:
        st = HD.HORN_STYLE[hd.donor.horn]
        length = table["head_butt"][0]
        tr = _track(hd, HA.anim_butt, length, 96)
        v = 0.0
        for i in range(1, len(tr)):
            dt = tr[i][0] - tr[i - 1][0]
            dd = _rot(tr[i][1], b["head"]) - _rot(tr[i - 1][1], b["head"])
            v = max(v, abs(math.radians(dd) / max(dt, 1e-9)))
        # 角尖线速度 = 角速度 × 力臂（角伸出去多长）
        tip = v * (hd.horn_len / hd.px_m)
        if tip < st.speed * 0.5 or tip > st.speed * 2.5:
            bad.append(f"{kind} 顶角时角尖线速度 {tip:.1f} m/s，而角基粗细是按 "
                       f"{st.speed:.1f} m/s 的对撞推出来的——几何和动画用的不是同一个数")
    return bad


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--donor", default="")
    args = ap.parse_args()

    kinds = [args.donor] if args.donor else sorted(GN.HEAD_TEMPLATES)
    total = 0
    for k in kinds:
        bad = check(k)
        total += len(bad)
        hd, _ = HA.build_head(k)
        mark = "✓" if not bad else "✗"
        print(f"{mark} {k:<8} {len(HA.anims(hd))} 条动作  "
              f"咀嚼 {HA.chew_hz(hd):.2f} Hz  张口 {hd.gape:.0f}°")
        for x in bad:
            print(f"    {x}")
    if total:
        print(f"\n✗ 共 {total} 处问题")
        return 1
    print("\n✓ 下颌不反折 / 循环接得上 / 张口不超限 / 横滑=θ·h / 闭合塞得进周期 / "
          "没有的部件不动 / 威吓压耳 / 死亡单调 / 顶角速度对拍 / 无空动画 全部通过")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
