#!/usr/bin/env python3
"""拟态灰烬蛛 —— 姿态预览：展开姿渲染 + 折叠姿 FK 包围盒断言。

折叠姿是本模型的硬约束来源：伪装态 client 渲真方块，模型必须能收进
16×16×16 的方块体积，否则渲染切换瞬间会看到腿露出来。这里用 FK 把
折叠 pose 应用到全部 element 角点上算精确包围盒——靠算，不靠目测。

折叠姿定义与共轭旋转机制在 spider_rig.py；本文件只做包围盒校验与渲染。
框架模型校验时留 0.5 单位甲壳加厚预留；甲壳模型是最终尺寸，直接对 16³。

用法:
  python3 modelScript/creatures/mimic_spider/preview.py                  # 框架折叠断言+渲染
  python3 modelScript/creatures/mimic_spider/preview.py --model shell    # 甲壳折叠断言+渲染
  python3 modelScript/creatures/mimic_spider/preview.py --stance [--model shell]
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "core"))
sys.path.insert(0, str(HERE))

from bbmodel_maker.render.render_bbmodel import render  # noqa: E402
from spider_rig import BLOCK, MODELS, Pose, SpiderRig, fold_pose  # noqa: E402

SHELL_RESERVE = 0.5  # 甲壳层加厚预留：框架折叠包围盒必须比方块再小一圈


def posed_bbox(rig: SpiderRig, pose: Pose, verbose: bool = False) -> tuple[np.ndarray, np.ndarray]:
    W = rig.world(pose)
    lo = np.array([1e9] * 3)
    hi = np.array([-1e9] * 3)
    lo_who = ["?"] * 3
    hi_who = ["?"] * 3
    for n in rig.order:
        pts = rig.bone_points(n)
        if not len(pts):
            continue
        wp = pts @ W[n][:3, :3].T + W[n][:3, 3]
        for a in range(3):
            if wp[:, a].min() < lo[a]:
                lo[a], lo_who[a] = wp[:, a].min(), n
            if wp[:, a].max() > hi[a]:
                hi[a], hi_who[a] = wp[:, a].max(), n
    if verbose:
        for a, name in enumerate("xyz"):
            print(f"  {name} 极值骨：min {lo_who[a]} ({lo[a]:+.2f}) · max {hi_who[a]} ({hi[a]:+.2f})")
    return lo, hi


def check_fold(rig: SpiderRig, pose: Pose, reserve: float) -> int:
    """折叠姿必须收进 16×16×16（含预留）。返回违例数。"""
    lo, hi = posed_bbox(rig, pose, verbose=True)
    ext = hi - lo
    print(f"折叠包围盒  x {lo[0]:+6.2f}..{hi[0]:+6.2f} ({ext[0]:5.2f})"
          f"  y {lo[1]:+6.2f}..{hi[1]:+6.2f} ({ext[1]:5.2f})"
          f"  z {lo[2]:+6.2f}..{hi[2]:+6.2f} ({ext[2]:5.2f})")
    problems = []
    half = BLOCK / 2 - reserve
    for axis, name, lim_lo, lim_hi in (
        (0, "x", -half, half),
        (1, "y", -0.35, BLOCK - reserve),
        (2, "z", -half, half),
    ):
        if lo[axis] < lim_lo - 1e-6:
            problems.append(f"{name} 下界超出方块 {lim_lo - lo[axis]:.2f}")
        if hi[axis] > lim_hi + 1e-6:
            problems.append(f"{name} 上界超出方块 {hi[axis] - lim_hi:.2f}")
    if problems:
        print("✗ 折叠姿溢出方块体积：")
        for p in problems:
            print(f"   {p}")
    else:
        print(f"✓ 折叠姿收进 16³ 方块（余量 x {BLOCK / 2 - max(-lo[0], hi[0]):.2f} · "
              f"y {BLOCK - hi[1]:.2f} · z {BLOCK / 2 - max(-lo[2], hi[2]):.2f}）")
    return len(problems)


VIEWS = {"side": (90.0, 6.0), "front": (180.0, 6.0), "34": (145.0, 22.0), "top": (90.0, 78.0)}


def render_pose(rig: SpiderRig, model_path: Path, pose: Pose, stem: str,
                views: tuple[str, ...]) -> None:
    xform = rig.element_xform(pose)
    lo, hi = posed_bbox(rig, pose)
    center = (lo + hi) / 2
    span = float((hi - lo).max()) * 1.3
    for v in views:
        yaw, pitch = VIEWS[v]
        im, _ = render(model_path, yaw=yaw, pitch=pitch, size=520,
                       xform=dict(xform), focus=(center, span))
        out = HERE / f"pose_{stem}_{v}.png"
        im.save(out)
        print(f"→ {out.relative_to(HERE.parent.parent.parent)}")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--stance", action="store_true", help="展开姿（静止 FK，验证与生成器一致）")
    ap.add_argument("--model", choices=sorted(MODELS), default="frame")
    args = ap.parse_args()

    model_path = MODELS[args.model]
    stem_suffix = "" if args.model == "frame" else f"_{args.model}"
    # 甲壳模型的折叠断言不吃预留——甲壳就是最终尺寸，直接对 16³ 校验
    reserve = SHELL_RESERVE if args.model == "frame" else 0.0

    rig = SpiderRig(model_path)
    if args.stance:
        render_pose(rig, model_path, Pose(), f"stance{stem_suffix}", ("side", "front", "34", "top"))
        return 0

    pose = fold_pose()
    bad = check_fold(rig, pose, reserve)
    render_pose(rig, model_path, pose, f"fold{stem_suffix}", ("side", "front", "34", "top"))
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
