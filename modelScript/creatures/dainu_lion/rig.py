#!/usr/bin/env python3
"""怠怒之狮 —— 绑定层：从 bbmodel 读骨树，做正解 / 逆解 / 摆姿。

动画不手搓关键帧。四足动物的腿一旦靠"看着差不多"拧角度，落地那一刻脚必然在地上
滑（foot skate）——这是劣质四足动画唯一最大的破绽，而且渲染静帧看不出来，只能靠
算：支撑相里脚掌世界坐标必须贴地、且以恒定速度后移。所以这里给腿装真逆解，动画
只给「脚该踩在哪」，关节角由 IK 解出来，再用 `contact_report()` 量残差。

坐标沿用模型约定：16 单位 = 1 格 = 1 米，地面 y=0，兽头朝 −z。骨旋转沿用
Blockbench 的 R = Rz·Ry·Rx，绕自身 pivot、在父骨已变换的坐标系里施加，与
render_bbmodel.py 对元素的处理一致。
"""

from __future__ import annotations

import json
import math
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
MODELS = Path(__file__).resolve().parents[2] / "models" / "dainu_lion"
PELT = MODELS / "DainuLionPelt.bbmodel"

# 腿链：从近端到远端，末端骨的脚掌着地点由几何算（见 Rig._contact_point）
FORELEG = ("scapula_{s}", "humerus_{s}", "radius_{s}", "forefoot_{s}")
HINDLEG = ("femur_{s}", "tibia_{s}", "tarsus_{s}", "hindfoot_{s}")
SPINE = ("hips", "lumbar", "thorax_back", "thorax_front", "neck_base", "neck_mid", "neck_top", "skull")
TAIL = tuple(f"tail_{i:02d}" for i in range(1, 11))

# 各关节相对静止姿的活动范围（度，绕 X = 矢状面）。限位不是装饰：不夹的话 CCD 会把
# 肘/跗反关节解到反向折叠，渲出来是断腿。
LIMITS = {
    # 猫科的肩胛本身就是主要推进节段（可转 25°+），不是固定块；肱骨摆幅也大。
    # 早先按 ±18/±48 夹，前伸和抬腿相全顶到限位，脚陷地 1.5 单位。
    "scapula": (-26.0, 26.0),
    "humerus": (-74.0, 74.0),
    "radius": (-44.0, 92.0),    # 负 = 伸直，正 = 腕往后折
    "femur": (-48.0, 50.0),
    "tibia": (-74.0, 34.0),     # 负 = 膝往前折
    "tarsus": (-34.0, 84.0),    # 正 = 跗往后折
}
ABDUCT = 15.0  # 腿根外展上限（绕 Z），用于抵消躯干侧倾造成的横向偏差

# 腿根（肩胛 / 股骨）分担整肢摆动的比例，剩下的由闭式二连杆解。给少了，远端关节被迫
# 代偿：肩胛只分 0.34 时肱骨要在支撑相里扫 56°，看着像"大臂划水"。猫科的肩胛本身就是
# 主要推进节段，不是固定块。
FORE_SHARE = 0.50
HIND_SHARE = 0.50


def rotmat(deg: float, axis: int) -> np.ndarray:
    a = math.radians(deg)
    c, s = math.cos(a), math.sin(a)
    if axis == 0:
        return np.array([[1, 0, 0], [0, c, -s], [0, s, c]])
    if axis == 1:
        return np.array([[c, 0, s], [0, 1, 0], [-s, 0, c]])
    return np.array([[c, -s, 0], [s, c, 0], [0, 0, 1]])


def euler(rot) -> np.ndarray:
    """Blockbench 顺序 Rz·Ry·Rx。"""
    if not any(rot):
        return np.eye(3)
    return rotmat(rot[2], 2) @ rotmat(rot[1], 1) @ rotmat(rot[0], 0)


def affine(R: np.ndarray, t: np.ndarray) -> np.ndarray:
    M = np.eye(4)
    M[:3, :3] = R
    M[:3, 3] = t
    return M


@dataclass
class Bone:
    name: str
    uuid: str
    origin: np.ndarray
    parent: str | None
    children: list[str] = field(default_factory=list)
    elements: list[str] = field(default_factory=list)


@dataclass
class Channel:
    """单骨一帧的变换。rot 度、pos 单位、scale 倍率。"""

    rot: list[float] = field(default_factory=lambda: [0.0, 0.0, 0.0])
    pos: list[float] = field(default_factory=lambda: [0.0, 0.0, 0.0])
    scale: list[float] = field(default_factory=lambda: [1.0, 1.0, 1.0])

    def moved(self) -> bool:
        return any(self.rot) or any(self.pos) or any(abs(s - 1.0) > 1e-6 for s in self.scale)

    def copy(self) -> "Channel":
        return Channel(list(self.rot), list(self.pos), list(self.scale))


class Pose(dict):
    """bone name → Channel，缺省即静止姿。"""

    def __missing__(self, key: str) -> Channel:
        ch = Channel()
        self[key] = ch
        return ch


class Rig:
    def __init__(self, path: Path = PELT):
        self.path = Path(path)
        d = json.loads(self.path.read_text())
        self.doc = d
        gdefs = {g["uuid"]: g for g in d.get("groups", [])}
        self.bones: dict[str, Bone] = {}
        self.order: list[str] = []  # 父先于子

        def walk(node, parent: str | None):
            if isinstance(node, str):
                if parent:
                    self.bones[parent].elements.append(node)
                return
            g = gdefs.get(node["uuid"], node)
            name = g["name"]
            b = Bone(name, node["uuid"], np.array(g.get("origin", [0, 0, 0]), float), parent)
            self.bones[name] = b
            self.order.append(name)
            if parent:
                self.bones[parent].children.append(name)
            for c in node.get("children", []):
                walk(c, name)

        for root in d["outliner"]:
            walk(root, None)

        self.elements = {e["uuid"]: e for e in d["elements"]}
        self._contact: dict[str, np.ndarray] = {}
        self._sole_half: dict[str, float] = {}
        self._pts: dict[str, np.ndarray] = {}
        self._rest_stance: dict[str, np.ndarray] | None = None

    # ---------- 正解 ----------

    def _local(self, b: Bone, ch: Channel) -> np.ndarray:
        """骨自身的仿射：绕 pivot 旋转/缩放，再按 pos 平移（Bedrock 语义）。"""
        R = euler(ch.rot)
        if any(abs(s - 1.0) > 1e-6 for s in ch.scale):
            R = R @ np.diag(ch.scale)
        o = b.origin
        return affine(R, o + np.array(ch.pos, float) - R @ o)

    def world(self, pose: Pose | None = None) -> dict[str, np.ndarray]:
        """每骨的世界矩阵（父先于子，一遍算完）。"""
        pose = pose if pose is not None else Pose()
        out: dict[str, np.ndarray] = {}
        for name in self.order:
            b = self.bones[name]
            A = self._local(b, pose[name]) if name in pose else self._local(b, Channel())
            out[name] = A if b.parent is None else out[b.parent] @ A
        return out

    def chain_world(self, chain: list[str], base: np.ndarray, pose: Pose) -> list[np.ndarray]:
        """只沿一条链正解（IK 内循环用，别整机重算）。"""
        out, M = [], base
        for name in chain:
            M = M @ self._local(self.bones[name], pose[name])
            out.append(M)
        return out

    def joint(self, name: str, W: dict[str, np.ndarray]) -> np.ndarray:
        b = self.bones[name]
        return W[name][:3, :3] @ b.origin + W[name][:3, 3]

    # ---------- 几何 ----------

    def corners(self, e: dict) -> np.ndarray:
        f, t = np.array(e["from"], float), np.array(e["to"], float)
        pts = np.array([[x, y, z] for x in (f[0], t[0]) for y in (f[1], t[1]) for z in (f[2], t[2])])
        rot = e.get("rotation", [0, 0, 0])
        if any(rot):
            o = np.array(e.get("origin", [0, 0, 0]), float)
            pts = (euler(rot) @ (pts - o).T).T + o
        return pts

    def bone_points(self, name: str) -> np.ndarray:
        if name not in self._pts:
            pts = [self.corners(self.elements[u]) for u in self.bones[name].elements]
            self._pts[name] = np.vstack(pts) if pts else np.zeros((0, 3))
        return self._pts[name]

    def lowest(self, pose: Pose | None = None) -> float:
        """整只模型的最低点世界 y（穿地检查、倒地贴地夹持都要它）。"""
        W = self.world(pose)
        lo = 1e9
        for n in self.order:
            pts = self.bone_points(n)
            if len(pts):
                lo = min(lo, float((pts @ W[n][:3, :3].T + W[n][:3, 3])[:, 1].min()))
        return lo

    def contact_point(self, foot: str) -> np.ndarray:
        """脚掌着地点（静止姿模型坐标）：取该骨最低一层角点的形心。"""
        if foot not in self._contact:
            pts = self.bone_points(foot)
            lo = pts[:, 1].min()
            sole = pts[pts[:, 1] <= lo + 0.75]
            self._contact[foot] = sole.mean(axis=0)
            self._sole_half[foot] = float(sole[:, 2].max() - sole[:, 2].min()) / 2
        return self._contact[foot]

    def sole_half(self, foot: str) -> float:
        """掌心到掌尖的距离。绕掌心翻脚时用它把掌尖抬回地面。"""
        self.contact_point(foot)
        return self._sole_half[foot]

    def element_xform(self, pose: Pose) -> dict[str, np.ndarray]:
        W = self.world(pose)
        return {u: W[n] for n in self.order for u in self.bones[n].elements}

    # ---------- 逆解 ----------

    def leg_chain(self, side: str, hind: bool) -> list[str]:
        return [n.format(s=side) for n in (HINDLEG if hind else FORELEG)]

    def solve_leg(self, pose: Pose, side: str, hind: bool, target: np.ndarray,
                  *, foot_pitch: float = 0.0) -> float:
        """腿逆解：腿根按比例预定 + 剩余两节闭式二连杆。返回脚掌落点残差。

        为什么不是 CCD：三节链对一个点目标是冗余的，CCD 每帧独立求解、又没有时间
        连续性，目标一逼近可达边界就在两个解支之间跳——实测 run 的 humerus 相邻帧
        −64°→+64°、radius −30°→+79°，动画里是腿凭空翻一下。解析解没有这个自由度：
        腿根角由目标方向按固定比例给出，两连杆的弯曲方向由静止姿的符号锁死，目标
        超出可达范围就夹到边界（腿伸直），因此对目标处处连续。

        三步：① 绕 Z 定外展，把目标转进腿的矢状面（躯干侧倾会把腿根横向挪走，纯
        矢状解补不了）② 面内闭式解 ③ 脚掌摆到指定世界俯仰。
        """
        chain = self.leg_chain(side, hind)
        foot = chain[-1]
        b0, b1, b2 = (self.bones[n] for n in chain[:3])
        base_bone = self.bones[chain[0]].parent
        W = self.world(pose)
        P = W[base_bone] if base_bone else np.eye(4)
        Pinv = np.linalg.inv(P)

        # 目标（脚掌）→ 踝目标：先定脚掌世界俯仰，反推踝该在哪
        ankle_rest = self.bones[foot].origin
        sole_off = rotmat(foot_pitch, 0) @ (self.contact_point(foot) - ankle_rest)
        t_local = (Pinv @ np.append(np.asarray(target, float) - sole_off, 1.0))[:3]

        o0, o1, o2, o3 = b0.origin, b1.origin, b2.origin, ankle_rest

        # ① 外展：绕 Z 转多少能把目标转进腿的可解平面。
        # 平面是 x = **踝的静止 x**，不是 x = 腿根 x——链上只有绕 X 的转，x 分量根本
        # 改不了，踝恒落在自己的静止 x 上（前掌 −3.70 而肩胛 −2.40，静止姿本就差
        # 1.3 单位）。按腿根定平面时连静止姿都解不出来，四只脚齐齐浮空 1.3 单位。
        dx0 = o3[0] - o0[0]
        dx, dy = t_local[0] - o0[0], t_local[1] - o0[1]
        r = math.hypot(dx, dy)
        yr = -math.sqrt(max(0.0, r * r - dx0 * dx0))
        tz = math.degrees(math.atan2(dy, dx) - math.atan2(yr, dx0)) if r > 1e-6 else 0.0
        tz = min(ABDUCT, max(-ABDUCT, (tz + 180.0) % 360.0 - 180.0))
        pose[chain[0]].rot[2] = tz
        u = np.array([o0[0] + dx0, o0[1] + yr, t_local[2]])

        # ② 面内闭式解：YZ 平面当复平面，绕 X 转 = 乘 e^{iθ}
        def cx(v):
            return complex(v[1], v[2])

        a, b, c = cx(o1 - o0), cx(o2 - o1), cx(o3 - o2)
        D, D_rest = cx(u - o0), cx(o3 - o0)

        key0 = chain[0].rsplit("_", 1)[0]
        lo0, hi0 = LIMITS.get(key0, (-45.0, 45.0))
        share = HIND_SHARE if hind else FORE_SHARE
        swing = math.degrees(np.angle(D / D_rest)) if abs(D_rest) > 1e-9 else 0.0
        th0 = share * swing

        # 腿根只按比例给会在远端够不着：剩下两节的可达环是 [|lb−lc|, lb+lc]，先算出
        # 让目标落进这个环所需的最小腿根修正，不够就补足（补足量随距离连续变化，
        # 不会像"解不出来就夹住"那样在边界上跳）。
        lb, lc = abs(b), abs(c)
        if abs(D) > 1e-9 and abs(a) > 1e-9:
            # 两个 np.angle 相减不自动归一化：实测 166.4°−(−147.2°)=313.6° 被当成
            # 有效夹角用，把肩胛一路推到 +24° 限位（该往回收却往前伸），脚差 8 单位。
            base_ang = (math.degrees(np.angle(D) - np.angle(a)) + 180.0) % 360.0 - 180.0
            for lim, hi_side in ((lb + lc, True), (abs(lb - lc), False)):
                cos_r = (abs(D) ** 2 + abs(a) ** 2 - lim * lim) / (2 * abs(D) * abs(a))
                if -1.0 < cos_r < 1.0:
                    rmax = math.degrees(math.acos(cos_r))
                    rho = (base_ang - th0 + 180.0) % 360.0 - 180.0
                    if (hi_side and abs(rho) > rmax) or (not hi_side and abs(rho) < rmax):
                        th0 = base_ang - math.copysign(rmax, rho if rho else 1.0)
        th0 = min(hi0, max(lo0, th0))

        E = D * complex(math.cos(math.radians(-th0)), math.sin(math.radians(-th0))) - a
        d = abs(E)
        d = min(lb + lc - 1e-4, max(abs(lb - lc) + 1e-4, d))   # 夹进可达环 → 连续
        E = E / abs(E) * d if abs(E) > 1e-9 else complex(d, 0)

        cos_d = max(-1.0, min(1.0, (d * d - lb * lb - lc * lc) / (2 * lb * lc)))
        rest_d = np.angle(c / b)
        delta = math.copysign(math.acos(cos_d), rest_d if abs(rest_d) > 1e-9 else 1.0)
        gamma = math.atan2(lc * math.sin(delta), lb + lc * math.cos(delta))
        th1 = math.degrees(np.angle(E / b) - gamma)
        th2 = math.degrees(delta - rest_d)

        for name, th in zip(chain[:3], (th0, th1, th2)):
            k = name.rsplit("_", 1)[0]
            lo, hi = LIMITS.get(k, (-45.0, 45.0))
            pose[name].rot[0] = min(hi, max(lo, (th + 180.0) % 360.0 - 180.0))

        # ③ 脚掌摆到指定**世界**俯仰。除了腿链自身，还得抵掉躯干带来的俯仰——只抵腿链
        # 时，胸椎一低头爪子就跟着倾，咆哮蓄力帧实测整只前掌扎进地下 1.9 单位。
        fwd = P[:3, :3] @ np.array([0.0, 0.0, -1.0])
        base_pitch = math.degrees(math.atan2(fwd[1], -fwd[2]))
        pose[foot].rot[0] = foot_pitch - base_pitch - sum(pose[n].rot[0] for n in chain[:3])
        # 侧倾同理：躯干一压重心，着地的爪子会跟着歪，外侧角扎地（挥爪动作实测 0.58）
        right = P[:3, :3] @ np.array([1.0, 0.0, 0.0])
        pose[foot].rot[2] = -math.degrees(math.atan2(right[1], right[0])) - tz
        return float(np.linalg.norm(self.foot_world(pose, side, hind) - np.asarray(target, float)))

    def foot_world(self, pose: Pose, side: str, hind: bool) -> np.ndarray:
        W = self.world(pose)
        foot = self.leg_chain(side, hind)[-1]
        return (W[foot] @ np.append(self.contact_point(foot), 1.0))[:3]

    def rest_stance(self) -> dict[str, np.ndarray]:
        if self._rest_stance is None:
            self._rest_stance = {f"{'h' if h else 'f'}{s}": self.foot_world(Pose(), s, h)
                                 for h in (False, True) for s in ("l", "r")}
        return {k: v.copy() for k, v in self._rest_stance.items()}


def contact_report(rig: Rig, sampler, legs, length: float, n: int = 48) -> str:
    """支撑相诊断：脚是否贴地、是否等速后移（滑步 = 四足动画头号破绽）。"""
    lines = []
    for key, (hind, side, is_stance) in legs.items():
        ys, zs, ts = [], [], []
        for i in range(n):
            t = i / n
            if not is_stance(t):
                continue
            pose = sampler(t)
            p = rig.foot_world(pose, side, hind)
            ys.append(p[1])
            zs.append(p[2])
            ts.append(t)
        if len(ts) < 3:
            lines.append(f"  {key}: 支撑相采样不足")
            continue
        A = np.vstack([np.array(ts), np.ones(len(ts))]).T
        slope, icpt = np.linalg.lstsq(A, np.array(zs), rcond=None)[0]
        resid = float(np.abs(np.array(zs) - (slope * np.array(ts) + icpt)).max())
        lines.append(f"  {key}: 触地 y {min(ys):+.2f}..{max(ys):+.2f}  "
                     f"后移速度 {slope / length:+.1f} u/s  滑步残差 {resid:.3f}")
    return "\n".join(lines)


if __name__ == "__main__":
    rig = Rig()
    print(f"骨 {len(rig.bones)} · 元素 {len(rig.elements)}")
    for k, v in rig.rest_stance().items():
        print(f"  {k} 触地点 ({v[0]:+6.2f},{v[1]:+6.2f},{v[2]:+7.2f})")
    pts = np.vstack([rig.bone_points(n) for n in rig.order if rig.bones[n].elements])
    print(f"  模型 y 范围 {pts[:, 1].min():.2f} .. {pts[:, 1].max():.2f}")
