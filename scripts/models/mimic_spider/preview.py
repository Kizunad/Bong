#!/usr/bin/env python3
"""拟态灰烬蛛 —— 姿态预览：展开姿渲染 + 折叠姿 FK 包围盒断言。

折叠姿是本模型的硬约束来源：伪装态 client 渲真方块，模型必须能收进
16×16×16 的方块体积，否则渲染切换瞬间会看到腿露出来。这里用 FK 把
折叠 pose 应用到全部 element 角点上算精确包围盒——靠算，不靠目测。

折叠形态取"死蜷"（death curl）变体：所有腿后掠贴体（膝上折、胫节向前
对折回来、跗节前上收拢），像被从吻端拖走的死蛛。顶视图剪影 = 体块 + 腿束，
读作一坨，正好塞进方块。

FK 借用 dainu_lion/rig.py 的通用 Rig（读任意 bbmodel 骨树；本模型组
rest rotation 恒 0，与其假设一致）。腿的方位角处理用共轭旋转：
目标方向 → W = A(φ',e')·A(φ,e)⁻¹，逐骨 R_local = W_parent⁻¹·W_bone，
再按 Blockbench R=Rz·Ry·Rx 分解回欧拉角。

用法:
  python3 scripts/models/mimic_spider/preview.py           # 折叠姿渲染 + 包围盒断言
  python3 scripts/models/mimic_spider/preview.py --stance  # 展开姿三视
"""

from __future__ import annotations

import argparse
import math
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
sys.path.insert(0, str(HERE.parent / "dainu_lion"))
sys.path.insert(0, str(HERE))

import gen_frame as F  # noqa: E402
from render_bbmodel import render  # noqa: E402
from rig import Pose, Rig, rotmat  # noqa: E402

MODEL = F.OUT_DIR / "MimicSpiderFrame.bbmodel"
BLOCK = 16.0  # 拟态目标：一个方块

# 折叠参数（右侧约定；左侧经 sx 镜像自动成立）。
# az 用 gen_frame 约定：0 = 正侧向，正 = 朝前。形态是"蹲伏死蜷"：身体离地 ~2 单位，
# 前三对腿后掠收在体侧（膝上折、胫节向前对折），第 4 对反向——腿节向前折过头顶、
# 胫节向后对折（真实死蜷方向；也是唯一能同时躲开腹部(z)/顶(y)/壁(x)的折法）。
# 逐节给 (方位角, 仰角)：coxa / femur / tibia / tarsus。
FOLD_ROOT_POS = (0.0, -3.2, 0.2)
FOLD_LEGS = {
    1: ((-84.0, 15.0), (-84.0, 40.0), (85.0, -60.0), (85.0, 45.0)),
    2: ((-84.0, 15.0), (-84.0, 45.0), (85.0, -63.0), (85.0, 45.0)),
    3: ((-84.0, 15.0), (-84.0, 58.0), (85.0, -68.0), (85.0, 45.0)),
    4: ((-65.0, 15.0), (80.0, 60.0), (-85.0, -78.0), (-85.0, 40.0)),
}
FOLD_ABDOMEN_PITCH = 25.0    # 腹部下扣
FOLD_ABDOMEN_SHIFT = -1.9    # 腹部沿腹柄前挤（膜质，可压缩）
# 螯肢下折近水平贴颌（fang 平指向后）。注意不能用小角度+内旋凑：paturon 是宽>深的
# 扁盒，绕 y 内旋必有一只前角向外甩，唯有大 pitch 把整件转出 z 前界
FOLD_CHELICERA_ROT = (-80.0, 0.0)
SHELL_RESERVE = 0.5  # 甲壳层加厚预留：框架折叠包围盒必须比方块再小一圈


def dir_of(az_deg: float, elev_deg: float, sx: float) -> np.ndarray:
    """gen_frame 腿几何约定下的方向向量：水平方位 az + 仰角 elev。"""
    az, e = math.radians(az_deg), math.radians(elev_deg)
    u = np.array([sx * math.cos(az), 0.0, -math.sin(az)])
    return u * math.cos(e) + np.array([0.0, 1.0, 0.0]) * math.sin(e)


def frame_of(d: np.ndarray) -> np.ndarray:
    """把 x̂ 映到方向 d 的旋转 A = Ry(φ)·Rz(e)（roll 取 0）。"""
    e = math.degrees(math.asin(max(-1.0, min(1.0, d[1]))))
    phi = math.degrees(math.atan2(-d[2], d[0])) if abs(d[0]) + abs(d[2]) > 1e-9 else 0.0
    return rotmat(phi, 1) @ rotmat(e, 2)


def retarget(rest_dir: np.ndarray, target_dir: np.ndarray) -> np.ndarray:
    """世界系下把 rest 方向转到目标方向的旋转。"""
    return frame_of(target_dir / np.linalg.norm(target_dir)) @ np.linalg.inv(
        frame_of(rest_dir / np.linalg.norm(rest_dir)))


def to_euler(R: np.ndarray) -> list[float]:
    """R = Rz(γ)·Ry(β)·Rx(α) 分解（与 rig.euler 互逆）。"""
    beta = -math.degrees(math.asin(max(-1.0, min(1.0, R[2, 0]))))
    alpha = math.degrees(math.atan2(R[2, 1], R[2, 2]))
    gamma = math.degrees(math.atan2(R[1, 0], R[0, 0]))
    return [alpha, beta, gamma]


def fold_pose() -> Pose:
    """折叠姿：root 落地后移 + 八腿死蜷 + 触肢/螯肢内收。"""
    pose = Pose()
    pose["root"].pos = list(FOLD_ROOT_POS)

    for pair in (1, 2, 3, 4):
        for side, sx in (("l", -1.0), ("r", 1.0)):
            key = f"{pair}_{side}"
            joints = [np.array(p) for p in F.leg_joints(pair, side)]
            rest = [joints[i + 1] - joints[i] for i in range(4)]
            tgt = [dir_of(az, elev, sx) for az, elev in FOLD_LEGS[pair]]
            W_parent = np.eye(3)
            for bone_prefix, r, t in zip(("coxa", "femur", "tibia", "tarsus"), rest, tgt):
                W = retarget(r, t)
                pose[f"{bone_prefix}{key}"].rot = to_euler(np.linalg.inv(W_parent) @ W)
                W_parent = W

    # 腹部下扣 + 前挤：单靠旋转收不进后界（倾斜箱体的上后角会被甩出去），
    # 腹柄是膜质，蹲伏时压缩前移是解剖上说得通的
    pose["abdomen"].rot = [FOLD_ABDOMEN_PITCH, 0.0, 0.0]
    pose["abdomen"].pos = [0.0, 0.0, FOLD_ABDOMEN_SHIFT]

    # 触肢：前举 → 沿体侧向后下收拢；螯肢连牙整体后收贴胸
    for side, sx in (("l", -1.0), ("r", 1.0)):
        rest1 = np.array([sx * 2.5, 1.7, -2.0])
        rest2 = np.array([sx * 0.8, -0.7, -2.8])
        W1 = retarget(rest1, np.array([sx * 0.55, -0.75, 0.35]))
        W2 = retarget(rest2, np.array([sx * -0.2, -0.25, 0.9]))
        pose[f"palp1_{side}"].rot = to_euler(W1)
        pose[f"palp2_{side}"].rot = to_euler(np.linalg.inv(W1) @ W2)
        pose[f"chelicera_{side}"].rot = [FOLD_CHELICERA_ROT[0], -sx * FOLD_CHELICERA_ROT[1], 0.0]
    return pose


def posed_bbox(rig: Rig, pose: Pose, verbose: bool = False) -> tuple[np.ndarray, np.ndarray]:
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


def check_fold(rig: Rig, pose: Pose) -> int:
    """折叠姿必须收进 16×16×16（x/z ∈ ±8，y ∈ 0..16）。返回违例数。"""
    lo, hi = posed_bbox(rig, pose, verbose=True)
    ext = hi - lo
    print(f"折叠包围盒  x {lo[0]:+6.2f}..{hi[0]:+6.2f} ({ext[0]:5.2f})"
          f"  y {lo[1]:+6.2f}..{hi[1]:+6.2f} ({ext[1]:5.2f})"
          f"  z {lo[2]:+6.2f}..{hi[2]:+6.2f} ({ext[2]:5.2f})")
    problems = []
    half = BLOCK / 2 - SHELL_RESERVE
    for axis, name, lim_lo, lim_hi in (
        (0, "x", -half, half),
        (1, "y", -0.35, BLOCK - SHELL_RESERVE),
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


def render_pose(rig: Rig, pose: Pose, stem: str, views: tuple[str, ...]) -> None:
    xform = rig.element_xform(pose)
    lo, hi = posed_bbox(rig, pose)
    center = (lo + hi) / 2
    span = float((hi - lo).max()) * 1.3
    for v in views:
        yaw, pitch = VIEWS[v]
        im, _ = render(MODEL, yaw=yaw, pitch=pitch, size=520,
                       xform={u: M for u, M in xform.items()}, focus=(center, span))
        out = HERE / f"pose_{stem}_{v}.png"
        im.save(out)
        print(f"→ {out.relative_to(HERE.parent.parent.parent)}")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--stance", action="store_true", help="展开姿（静止 FK，验证与生成器一致）")
    args = ap.parse_args()

    rig = Rig(MODEL)
    if args.stance:
        render_pose(rig, Pose(), "stance", ("side", "front", "34", "top"))
        return 0

    pose = fold_pose()
    bad = check_fold(rig, pose)
    render_pose(rig, pose, "fold", ("side", "front", "34", "top"))
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
