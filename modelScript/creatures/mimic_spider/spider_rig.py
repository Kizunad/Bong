#!/usr/bin/env python3
"""拟态灰烬蛛 —— 绑定层：腿逆解 + 折叠姿 + 姿态工具。

动画不手搓关键帧（与狮子同则）：移动类动画只给「脚该踩在哪」，关节角由逆解
算出——支撑相脚锁死在世界坐标上，不滑步。蛛腿去掉基节后是**标准二连杆**
（腿节+胫节），闭式解唯一、无 CCD 的解支跳变问题。

求解顺序：① 基节沿目标水平方位摆（保持静止仰角）② 腿节+胫节在过方位角的
铅垂面内闭式解（膝上折符号锁死）③ 跗节按给定触地角落地。全部几何在
**头胸部静止空间**内解——世界目标先经 root·prosoma 仿射逆变换，身体怎么
起伏摇摆都不影响腿链局部角。

方向 → 骨骼欧拉：共轭旋转 W = A(目标)·A(静止)⁻¹，逐骨 R_local = W_父⁻¹·W_骨，
按 Blockbench R=Rz·Ry·Rx 分解。FK 底座借用 dainu_lion/rig.py 的通用 Rig。
"""

from __future__ import annotations

import math
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "dainu_lion"))
sys.path.insert(0, str(HERE))

import gen_frame as F  # noqa: E402
from rig import Channel, Pose, Rig, rotmat  # noqa: E402  (dainu_lion 通用 FK)

MODELS = {
    "frame": F.OUT_DIR / "MimicSpiderFrame.bbmodel",
    "shell": F.OUT_DIR / "MimicSpiderShell.bbmodel",
}
SHELL = MODELS["shell"]
BLOCK = 16.0

LEG_KEYS = tuple((pair, side) for pair in (1, 2, 3, 4) for side in ("l", "r"))
CHAIN = ("coxa", "femur", "tibia", "tarsus")


# ---------------------------------------------------------------- 方向↔欧拉
def frame_of(d: np.ndarray) -> np.ndarray:
    """把 x̂ 映到方向 d 的旋转 A = Ry(φ)·Rz(e)（roll 取 0）。"""
    d = d / np.linalg.norm(d)
    e = math.degrees(math.asin(max(-1.0, min(1.0, d[1]))))
    phi = math.degrees(math.atan2(-d[2], d[0])) if abs(d[0]) + abs(d[2]) > 1e-9 else 0.0
    return rotmat(phi, 1) @ rotmat(e, 2)


def retarget(rest_dir: np.ndarray, target_dir: np.ndarray) -> np.ndarray:
    """世界系下把 rest 方向转到目标方向的旋转。"""
    return frame_of(target_dir) @ np.linalg.inv(frame_of(rest_dir))


def to_euler(R: np.ndarray) -> list[float]:
    """R = Rz(γ)·Ry(β)·Rx(α) 分解（与 rig.euler 互逆）。"""
    beta = -math.degrees(math.asin(max(-1.0, min(1.0, R[2, 0]))))
    alpha = math.degrees(math.atan2(R[2, 1], R[2, 2]))
    gamma = math.degrees(math.atan2(R[1, 0], R[0, 0]))
    return [alpha, beta, gamma]


def dir_of(az_deg: float, elev_deg: float, sx: float) -> np.ndarray:
    """gen_frame 腿几何约定下的方向向量：水平方位 az + 仰角 elev。"""
    az, e = math.radians(az_deg), math.radians(elev_deg)
    u = np.array([sx * math.cos(az), 0.0, -math.sin(az)])
    return u * math.cos(e) + np.array([0.0, 1.0, 0.0]) * math.sin(e)


# ---------------------------------------------------------------- 腿几何缓存
def _leg_rest(pair: int, side: str):
    pts = [np.array(p) for p in F.leg_joints(pair, side)]
    dirs = [(pts[i + 1] - pts[i]) for i in range(4)]
    lens = [float(np.linalg.norm(d)) for d in dirs]
    return pts, [d / n for d, n in zip(dirs, lens)], lens


_REST = {(p, s): _leg_rest(p, s) for p, s in LEG_KEYS}


def rest_targets() -> dict[tuple[int, str], np.ndarray]:
    """静止姿八爪落点（世界坐标）。步态在此基础上摆目标。"""
    return {(p, s): _REST[(p, s)][0][-1].copy() for p, s in LEG_KEYS}


def pose_leg_dirs(pose: Pose, pair: int, side: str, tgt_dirs: list[np.ndarray]) -> None:
    """按四节段目标方向写腿链局部欧拉（共轭旋转逐骨下传）。"""
    _pts, rest_dirs, _lens = _REST[(pair, side)]
    key = f"{pair}_{side}"
    W_parent = np.eye(3)
    for prefix, r, t in zip(CHAIN, rest_dirs, tgt_dirs):
        W = retarget(r, t)
        pose[f"{prefix}{key}"].rot = to_euler(np.linalg.inv(W_parent) @ W)
        W_parent = W


class SpiderRig(Rig):
    """通用 FK Rig + 蛛腿逆解。"""

    def __init__(self, path: Path = SHELL):
        super().__init__(path)
        self._tarsus_rest_elev = {
            (p, s): math.degrees(math.asin(_REST[(p, s)][1][3][1])) for p, s in LEG_KEYS
        }

    # ---------- 头胸部仿射（腿链的求解空间） ----------
    def prosoma_affine(self, pose: Pose) -> np.ndarray:
        A = self._local(self.bones["root"], pose["root"]) if "root" in pose else np.eye(4)
        B = self._local(self.bones["prosoma"], pose["prosoma"]) if "prosoma" in pose else np.eye(4)
        return A @ B

    # ---------- 逆解 ----------
    def solve_leg(self, pose: Pose, pair: int, side: str, target,
                  *, tarsus_elev: float | None = None) -> float:
        """脚踩到世界点 target。返回落点残差（世界单位）。"""
        pts, rest_dirs, lens = _REST[(pair, side)]
        p0 = pts[0]
        lc, lf, lt, ltar = lens
        theta = self._tarsus_rest_elev[(pair, side)] if tarsus_elev is None else tarsus_elev

        P = self.prosoma_affine(pose)
        t = (np.linalg.inv(P) @ np.append(np.asarray(target, float), 1.0))[:3]

        # ① 水平方位：从基节根指向目标
        dx, dz = t[0] - p0[0], t[2] - p0[2]
        if abs(dx) + abs(dz) < 1e-9:
            u = rest_dirs[0] * np.array([1.0, 0.0, 1.0])
            u = u / np.linalg.norm(u)
        else:
            u = np.array([dx, 0.0, dz]) / math.hypot(dx, dz)
        up = np.array([0.0, 1.0, 0.0])

        e_c = math.degrees(math.asin(rest_dirs[0][1]))       # 基节保持静止仰角
        d_coxa = u * math.cos(math.radians(e_c)) + up * math.sin(math.radians(e_c))
        p1 = p0 + d_coxa * lc

        # ② 跗节反推踝目标 → 铅垂面内二连杆闭式解（膝上折）
        d_tar = u * math.cos(math.radians(theta)) + up * math.sin(math.radians(theta))
        ankle = t - d_tar * ltar
        r = float(np.dot(ankle - p1, u))
        h = float(ankle[1] - p1[1])
        D = math.hypot(r, h)
        D = min(lf + lt - 1e-4, max(abs(lf - lt) + 1e-4, D))
        base = math.atan2(h, r)
        off = math.acos(max(-1.0, min(1.0, (lf * lf + D * D - lt * lt) / (2 * lf * D))))
        e_f = base + off                                     # + = 膝在连线上方，符号锁死
        knee = p1 + (u * math.cos(e_f) + up * math.sin(e_f)) * lf
        va = ankle - knee
        e_t = math.atan2(va[1], float(np.dot(va, u)))

        d_femur = u * math.cos(e_f) + up * math.sin(e_f)
        d_tibia = u * math.cos(e_t) + up * math.sin(e_t)
        pose_leg_dirs(pose, pair, side, [d_coxa, d_femur, d_tibia, d_tar])
        return float(np.linalg.norm(self.foot_world(pose, pair, side) - np.asarray(target, float)))

    def foot_world(self, pose: Pose, pair: int, side: str) -> np.ndarray:
        W = self.world(pose)
        tip = _REST[(pair, side)][0][-1]
        M = W[f"tarsus{pair}_{side}"]
        return (M @ np.append(tip, 1.0))[:3]

    def plant(self, pose: Pose, targets: dict[tuple[int, str], np.ndarray]) -> float:
        """八腿全部踩到指定落点，返回最大残差。"""
        return max(self.solve_leg(pose, p, s, targets[(p, s)]) for p, s in LEG_KEYS)


# ---------------------------------------------------------------- 折叠姿
# 折叠形态"蹲伏死蜷"：前三对腿后掠贴体（膝上折、胫节向前对折），第 4 对反向
# （腿节向前折过头、胫节向后对折）。参数推导与包围盒收敛过程见 round 1-2 提交记录。
FOLD_ROOT_POS = (0.0, -3.2, 0.6)
FOLD_LEGS = {
    1: ((-84.0, 15.0), (-84.0, 40.0), (85.0, -60.0), (85.0, 45.0)),
    2: ((-84.0, 15.0), (-84.0, 45.0), (85.0, -63.0), (85.0, 45.0)),
    3: ((-84.0, 15.0), (-84.0, 58.0), (85.0, -68.0), (85.0, 45.0)),
    4: ((-65.0, 15.0), (80.0, 60.0), (-85.0, -78.0), (-85.0, 40.0)),
}
FOLD_ABDOMEN_PITCH = 25.0
FOLD_ABDOMEN_SHIFT = -1.9
FOLD_CHELICERA_PITCH = -80.0
PALP_REST = ((2.5, 1.7, -2.0), (0.8, -0.7, -2.8))
PALP_FOLD = ((0.55, -0.75, 0.35), (-0.2, -0.25, 0.9))


def fold_pose() -> Pose:
    """折叠姿：root 落地 + 八腿死蜷 + 腹部下扣前挤 + 螯肢/触肢后收。"""
    pose = Pose()
    pose["root"].pos = list(FOLD_ROOT_POS)
    for pair, side in LEG_KEYS:
        sx = 1.0 if side == "r" else -1.0
        pose_leg_dirs(pose, pair, side,
                      [dir_of(az, elev, sx) for az, elev in FOLD_LEGS[pair]])
    pose["abdomen"].rot = [FOLD_ABDOMEN_PITCH, 0.0, 0.0]
    pose["abdomen"].pos = [0.0, 0.0, FOLD_ABDOMEN_SHIFT]
    for side, sx in (("l", -1.0), ("r", 1.0)):
        r1 = np.array([sx * PALP_REST[0][0], *PALP_REST[0][1:]])
        r2 = np.array([sx * PALP_REST[1][0], *PALP_REST[1][1:]])
        W1 = retarget(r1, np.array([sx * PALP_FOLD[0][0], *PALP_FOLD[0][1:]]))
        W2 = retarget(r2, np.array([sx * PALP_FOLD[1][0], *PALP_FOLD[1][1:]]))
        pose[f"palp1_{side}"].rot = to_euler(W1)
        pose[f"palp2_{side}"].rot = to_euler(np.linalg.inv(W1) @ W2)
        pose[f"chelicera_{side}"].rot = [FOLD_CHELICERA_PITCH, 0.0, 0.0]
    return pose


def lerp_pose(a: Pose, b: Pose, s: float, bones: list[str]) -> Pose:
    """逐通道线性插值（欧拉小角近似够用；大角骨骼由调用方分段给关键姿控制）。"""
    out = Pose()
    for n in bones:
        ca = a[n] if n in a else Channel()
        cb = b[n] if n in b else Channel()
        ch = out[n]
        ch.rot = [ca.rot[i] + (cb.rot[i] - ca.rot[i]) * s for i in range(3)]
        ch.pos = [ca.pos[i] + (cb.pos[i] - ca.pos[i]) * s for i in range(3)]
    return out


def contact_report(rig: SpiderRig, sampler, stance_of, length: float, n: int = 48) -> str:
    """支撑相诊断：脚是否贴地、是否随支撑进度等速后移（滑步 = 步态头号破绽）。

    stance_of(pair, side, t) → 支撑进度 frac ∈ [0,1) 或 None。回归 z 对 **frac**
    而不是对 t——相位跨环的腿支撑相在 t 轴上是两段，对 t 回归必然拟出垃圾。"""
    lines = []
    for pair, side in LEG_KEYS:
        ys, zs, fs = [], [], []
        for i in range(n):
            t = i / n
            frac = stance_of(pair, side, t)
            if frac is None:
                continue
            pose = sampler(t)
            p = rig.foot_world(pose, pair, side)
            ys.append(p[1])
            zs.append(p[2])
            fs.append(frac)
        if len(fs) < 3:
            lines.append(f"  {pair}_{side}: 支撑相采样不足")
            continue
        A = np.vstack([np.array(fs), np.ones(len(fs))]).T
        slope, icpt = np.linalg.lstsq(A, np.array(zs), rcond=None)[0]
        resid = float(np.abs(np.array(zs) - (slope * np.array(fs) + icpt)).max())
        lines.append(f"  {pair}_{side}: 触地 y {min(ys):+.2f}..{max(ys):+.2f}  "
                     f"支撑行程 {slope:+.1f} u  滑步残差 {resid:.3f}")
    return "\n".join(lines)


if __name__ == "__main__":
    rig = SpiderRig()
    print(f"骨 {len(rig.bones)} · 元素 {len(rig.elements)}")
    # 自检：解到静止落点应复现静止姿（角度≈0、残差≈0）
    pose = Pose()
    worst = rig.plant(pose, rest_targets())
    angs = [abs(v) for p, s in LEG_KEYS for v in pose[f"femur{p}_{s}"].rot]
    print(f"静止复现残差 {worst:.4f} · femur 角度最大 |{max(angs):.3f}|°")
    # 自检：踩到前移 3 单位的落点，残差应仍然小
    tgts = {k: v + np.array([0.0, 0.0, -3.0]) for k, v in rest_targets().items()}
    print(f"前移 3 单位残差 {rig.plant(Pose(), tgts):.4f}")
