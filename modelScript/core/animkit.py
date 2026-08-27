#!/usr/bin/env python3
"""骨骼动画工具箱 —— bbmodel 骨树的正解 / 逆解 / 摆姿 / 关键帧导出。

自 dainu_lion/rig.py + gen_anim.py 提炼：那两份把猫科解剖（四条腿、脊柱分段、尾行
波）和通用的骨树正解、二连杆闭式逆解、曲线工具、bbmodel 关键帧写盘混在一起，另做
物种就得整段抄。这里只留与物种无关的部分 —— 肢体链、限位、动作全由调用方给。

**不改 dainu_lion**：它的动画是照那份文件逐条量着调出来的（相位方向、腾空窗口、
尾巴总度数……），换掉它的依赖等于赌上那些调参。狮子留在原地，本工具箱只服务新物种。

坐标与旋转约定跟建模层一致：16 单位 = 1 格 = 1 m，地面 y=0，兽头朝 -Z；骨旋转按
Blockbench 的 R = Rz·Ry·Rx，绕自身 pivot、在父骨已变换的坐标系里施加。
"""

from __future__ import annotations

import json
import math
import sys
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "core"))
import animcore  # noqa: E402
from to_fmt410 import ensure_410  # noqa: E402

# ================================================================ 旋转 / 曲线
# 这些原本在本模块和 anim_rig.py 里各写了一份逐字相同的实现。现在统一在 animcore，
# 这里只做转发 —— 既有 `from animkit import rotmat, smooth, ...` 的调用点一个不用改。
rotmat = animcore.rotmat
euler = animcore.euler
align = animcore.align
slerp = animcore.slerp
affine = animcore.affine
wrap = animcore.wrap
clamp01 = animcore.clamp01
smooth = animcore.smooth
pulse = animcore.pulse
soft_clamp = animcore.soft_clamp
keyed = animcore.keyed


def euler_of(R: np.ndarray) -> np.ndarray:
    """R = Rz·Ry·Rx 的逆运算，返回 (x, y, z) 度。

    本模块的调用点拿它当向量算（做差、加权），所以维持 ndarray 返回值；
    anim_rig 那份返回 list。算法本身在 `animcore.euler_xyz`。
    """
    return np.array(animcore.euler_xyz(R))


# ================================================================ 骨 / 姿态


@dataclass
class Bone:
    name: str
    uuid: str
    origin: np.ndarray
    rest_rot: np.ndarray  # 绑定姿旋转（Blockbench 的 group.rotation）
    parent: str | None
    children: list[str] = field(default_factory=list)
    elements: list[str] = field(default_factory=list)


@dataclass
class Channel:
    """单骨一帧的变换。rot 度、pos 单位、scale 倍率。"""

    rot: list[float] = field(default_factory=lambda: [0.0, 0.0, 0.0])
    pos: list[float] = field(default_factory=lambda: [0.0, 0.0, 0.0])
    scale: list[float] = field(default_factory=lambda: [1.0, 1.0, 1.0])


class Pose(dict):
    """bone name → Channel，缺省即静止姿。"""

    def __missing__(self, key: str) -> Channel:
        ch = Channel()
        self[key] = ch
        return ch


class PoseRig:
    """读一份 .bbmodel 的骨树，做正解 / 逆解 / 摆姿。

    兼容 fmt 4.x（outliner 内联 group）与 fmt 5.0（groups 数组 + uuid 引用树）——
    生成器出的是前者，Blockbench 手工存盘出的是后者，两种都得能读。
    """

    def __init__(self, path: Path | str):
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
            self.bones[name] = Bone(
                name, node["uuid"],
                np.array(g.get("origin", [0, 0, 0]), float),
                np.array(g.get("rotation", [0, 0, 0]), float),
                parent,
            )
            self.order.append(name)
            if parent:
                self.bones[parent].children.append(name)
            for c in node.get("children", []):
                walk(c, name)

        for root in d["outliner"]:
            walk(root, None)
        if not self.bones:
            raise SystemExit(f"{self.path}: 读不到骨骼层级")

        self.elements = {e["uuid"]: e for e in d["elements"]}
        self._contact: dict[str, np.ndarray] = {}
        self._sole_half: dict[str, float] = {}
        self._pts: dict[str, np.ndarray] = {}

    # ---------------------------------------------------------------- 正解

    def _local(self, b: Bone, ch: Channel) -> np.ndarray:
        """骨自身的仿射：绕 pivot 旋转/缩放，再按 pos 平移（Bedrock 语义）。

        绑定姿旋转与动画旋转**逐分量相加**后再解成矩阵 —— Blockbench 就是这么合的，
        换成矩阵相乘在两者都非零时会差出一个可见的角度。
        """
        R = euler(b.rest_rot + np.array(ch.rot, float))
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
            A = self._local(b, pose[name] if name in pose else Channel())
            out[name] = A if b.parent is None else out[b.parent] @ A
        return out

    def chain_world(self, chain: list[str], base: np.ndarray, pose: Pose) -> list[np.ndarray]:
        """只沿一条链正解（搜索/逆解的内循环用，别整机重算 —— 收腿角搜索一次要试上万
        个候选，整机 FK 把 5 秒的活拖成 5 分钟）。"""
        out, M = [], base
        for name in chain:
            M = M @ self._local(self.bones[name], pose[name])
            out.append(M)
        return out

    def joint(self, name: str, W: dict[str, np.ndarray]) -> np.ndarray:
        """骨 pivot 的世界坐标。"""
        b = self.bones[name]
        return W[name][:3, :3] @ b.origin + W[name][:3, 3]

    def element_xform(self, pose: Pose) -> dict[str, np.ndarray]:
        W = self.world(pose)
        return {u: W[n] for n in self.order for u in self.bones[n].elements}

    # ---------------------------------------------------------------- 几何

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
            pts = [self.corners(self.elements[u]) for u in self.bones[name].elements
                   if u in self.elements]
            self._pts[name] = np.vstack(pts) if pts else np.zeros((0, 3))
        return self._pts[name]

    def reach(self, name: str) -> float:
        """该骨的**力臂**：pivot 到自己整条子树几何最远处的距离。

        转这根骨 θ 度，最远那块方块就走 reach·θ。裁剪容差、误差预算都得按它折算 —— 同
        一个角度在下颌和在肩关节上完全不是一回事。
        """
        if not hasattr(self, "_reach"):
            self._reach: dict[str, float] = {}
        if name not in self._reach:
            o = self.bones[name].origin
            far = 0.0
            for n in self.subtree(name):
                pts = self.bone_points(n)
                if len(pts):
                    far = max(far, float(np.linalg.norm(pts - o, axis=1).max()))
            self._reach[name] = far
        return self._reach[name]

    def depth(self, name: str) -> int:
        """从根算起的层数（根 = 1）。裁剪预算要按它分摊，见 _prune_tol。"""
        d, b = 1, self.bones[name]
        while b.parent is not None:
            d += 1
            b = self.bones[b.parent]
        return d

    def subtree(self, name: str) -> list[str]:
        out, stack = [], [name]
        while stack:
            n = stack.pop()
            out.append(n)
            stack.extend(self.bones[n].children)
        return out

    def lowest(self, pose: Pose | None = None, bones: list[str] | None = None) -> float:
        """最低点世界 y（穿地检查、倒地贴地夹持都要它）。"""
        W = self.world(pose)
        lo = 1e9
        for n in (bones or self.order):
            pts = self.bone_points(n)
            if len(pts):
                lo = min(lo, float((pts @ W[n][:3, :3].T + W[n][:3, 3])[:, 1].min()))
        return lo

    def contact_point(self, foot: str, band: float = 0.75) -> np.ndarray:
        """脚掌着地点（静止姿模型坐标）：该骨最低一层角点的形心。"""
        if foot not in self._contact:
            pts = self.bone_points(foot)
            if not len(pts):
                raise ValueError(f"{foot}: 没有几何，取不到着地点")
            lo = pts[:, 1].min()
            sole = pts[pts[:, 1] <= lo + band]
            self._contact[foot] = sole.mean(axis=0)
            self._sole_half[foot] = float(sole[:, 2].max() - sole[:, 2].min()) / 2
        return self._contact[foot]

    def sole_half(self, foot: str) -> float:
        """掌心到掌尖的距离。绕掌心翻脚时用它把掌尖抬回地面。"""
        self.contact_point(foot)
        return self._sole_half[foot]

    def tip_world(self, pose: Pose, chain: list[str]) -> np.ndarray:
        W = self.world(pose)
        foot = chain[-1]
        return (W[foot] @ np.append(self.contact_point(foot), 1.0))[:3]

    # ---------------------------------------------------------------- 逆解

    def solve_limb(self, pose: Pose, chain: list[str], target, *,
                   limits: dict[str, tuple[float, float]], share: float = 0.5,
                   tip_pitch: float = 0.0, abduct: float = 15.0) -> float:
        """三节肢 + 末端的逆解：肢根按比例预定 + 剩余两节闭式二连杆。返回落点残差。

        为什么不是 CCD：三节链对一个点目标是冗余的，CCD 每帧独立求解、又没有时间连续
        性，目标一逼近可达边界就在两个解支之间跳（狮子那边实测相邻帧 −64°→+64°，动画
        里是腿凭空翻一下）。解析解没有这个自由度：肢根角由目标方向按固定比例给出，两
        连杆的弯曲方向由静止姿的符号锁死，目标超出可达范围就夹到边界（肢伸直），因此
        对目标处处连续。

        三步：① 绕 Z 定外展，把目标转进肢的矢状面（躯干侧倾会把肢根横向挪走，纯矢状
        解补不了）② 面内闭式解 ③ 末端摆到指定**世界**俯仰。
        """
        foot = chain[-1]
        b0, b1, b2 = (self.bones[n] for n in chain[:3])
        base_bone = self.bones[chain[0]].parent
        W = self.world(pose)
        P = W[base_bone] if base_bone else np.eye(4)
        Pinv = np.linalg.inv(P)

        # 目标（着地点）→ 踝目标：先定末端世界俯仰，反推踝该在哪
        ankle_rest = self.bones[foot].origin
        sole_off = rotmat(tip_pitch, 0) @ (self.contact_point(foot) - ankle_rest)
        t_local = (Pinv @ np.append(np.asarray(target, float) - sole_off, 1.0))[:3]

        o0, o1, o2, o3 = b0.origin, b1.origin, b2.origin, ankle_rest

        # ① 外展：绕 Z 转多少能把目标转进可解平面。平面是 x = **踝的静止 x**，不是
        # x = 肢根 x —— 链上只有绕 X 的转，x 分量根本改不了，踝恒落在自己的静止 x 上。
        dx0 = o3[0] - o0[0]
        dx, dy = t_local[0] - o0[0], t_local[1] - o0[1]
        r = math.hypot(dx, dy)
        yr = -math.sqrt(max(0.0, r * r - dx0 * dx0))
        tz = math.degrees(math.atan2(dy, dx) - math.atan2(yr, dx0)) if r > 1e-6 else 0.0
        tz = soft_clamp((tz + 180.0) % 360.0 - 180.0, -abduct, abduct, 2.0)
        pose[chain[0]].rot[2] = tz
        u = np.array([o0[0] + dx0, o0[1] + yr, t_local[2]])

        # ② 面内闭式解：YZ 平面当复平面，绕 X 转 = 乘 e^{iθ}
        def cx(v):
            return complex(v[1], v[2])

        a, b, c = cx(o1 - o0), cx(o2 - o1), cx(o3 - o2)
        D, D_rest = cx(u - o0), cx(o3 - o0)

        lo0, hi0 = limits.get(chain[0], (-45.0, 45.0))
        swing = math.degrees(np.angle(D / D_rest)) if abs(D_rest) > 1e-9 else 0.0
        th0 = share * swing

        # 肢根只按比例给会在远端够不着：剩下两节的可达环是 [|lb−lc|, lb+lc]，先算出让
        # 目标落进这个环所需的最小肢根修正，不够就补足（补足量随距离连续变化，不会像
        # "解不出来就夹住"那样在边界上跳）。
        lb, lc = abs(b), abs(c)
        if abs(D) > 1e-9 and abs(a) > 1e-9:
            # 两个 np.angle 相减不自动归一化：不归一化会把 313.6° 当成有效夹角用。
            base_ang = (math.degrees(np.angle(D) - np.angle(a)) + 180.0) % 360.0 - 180.0

            def _rmax(lim: float) -> float:
                """目标落在半径 lim 的圆上时，肢根相对 base_ang 允许的最大偏角。

                cos 出界的两种情形必须给出**极限值**而不是"跳过这一项"：跳过等于把约束
                整条关掉，而它下一帧又会重新打开 —— 关节角在两帧之间硬跳一次。
                """
                cos_r = (abs(D) ** 2 + abs(a) ** 2 - lim * lim) / (2 * abs(D) * abs(a))
                if cos_r >= 1.0:
                    return 0.0          # 该圆整个够不着
                if cos_r <= -1.0:
                    return 180.0        # 该圆处处可达
                return math.degrees(math.acos(cos_r))

            # 约束本质是把 |rho| 夹进 [内圈, 外圈] 这个环 —— 写成夹取而不是两条 if，越界
            # 与不越界就接在同一条曲线上；再用软夹抹掉拐点。
            hi_r, lo_r = _rmax(lb + lc), _rmax(abs(lb - lc))
            rho = (base_ang - th0 + 180.0) % 360.0 - 180.0
            mag = soft_clamp(abs(rho), lo_r, hi_r, max(0.5, 0.08 * (hi_r - lo_r)))
            th0 = base_ang - math.copysign(mag, rho if rho else 1.0)
        th0 = soft_clamp(th0, lo0, hi0, 2.0)

        E = D * complex(math.cos(math.radians(-th0)), math.sin(math.radians(-th0))) - a
        # 夹进两连杆的可达环。用软夹：硬夹会让关节在目标出界期间彻底冻住，回到可达域时
        # 突然弹回来（见 soft_clamp 的说明）。knee 取环宽的 6%，够抹平那个拐点又不至于
        # 在正常范围内引入可见偏差。
        ring = (lb + lc) - abs(lb - lc)
        d = soft_clamp(abs(E), abs(lb - lc), lb + lc, max(1e-3, 0.06 * ring))
        E = E / abs(E) * d if abs(E) > 1e-9 else complex(d, 0)

        cos_d = max(-1.0, min(1.0, (d * d - lb * lb - lc * lc) / (2 * lb * lc)))
        rest_d = np.angle(c / b)
        delta = math.copysign(math.acos(cos_d), rest_d if abs(rest_d) > 1e-9 else 1.0)
        gamma = math.atan2(lc * math.sin(delta), lb + lc * math.cos(delta))
        th1 = math.degrees(np.angle(E / b) - gamma)
        th2 = math.degrees(delta - rest_d)

        for name, th in zip(chain[:3], (th0, th1, th2)):
            lo, hi = limits.get(name, (-45.0, 45.0))
            pose[name].rot[0] = soft_clamp((th + 180.0) % 360.0 - 180.0, lo, hi, 2.0)

        # ③ 末端摆到指定**世界**俯仰。除了肢链自身，还得抵掉躯干带来的俯仰 —— 只抵肢
        # 链时，躯干一低头脚就跟着倾，蓄力帧整只脚扎进地里。
        fwd = P[:3, :3] @ np.array([0.0, 0.0, -1.0])
        base_pitch = math.degrees(math.atan2(fwd[1], -fwd[2]))
        pose[foot].rot[0] = tip_pitch - base_pitch - sum(pose[n].rot[0] for n in chain[:3])
        right = P[:3, :3] @ np.array([1.0, 0.0, 0.0])
        pose[foot].rot[2] = -math.degrees(math.atan2(right[1], right[0])) - tz
        return float(np.linalg.norm(self.tip_world(pose, chain) - np.asarray(target, float)))


# ================================================================ 关键帧导出


_uuid = animcore.stable_uuid


def _kf(channel: str, time: float, vec, idx: int, seed: str) -> dict:
    """关键帧。种子拼成 `名+骨+通道+序号` —— 这是本模块的历史拼法，动它会让既有产物的
    uuid 全变，而 uuid 只是 Blockbench 的索引键。"""
    return animcore.keyframe(channel, time, vec, f"{seed}{idx}")


def _prune(vals: list[tuple[float, list[float]]], tol: float) -> list[tuple[float, list[float]]]:
    """丢掉落在前后两点连线上（误差 < tol）的关键帧 —— Ramer–Douglas–Peucker。

    导出的插值就是 linear，所以共线点是**精确冗余**：删掉它，曲线一模一样。慢通道上省
    得最多（长呼吸、缓慢扫视的颈椎，几十帧其实是一条直线）。首末帧永远保留，循环动画的
    末帧 = 首帧这条约定因此不受影响。
    """
    if len(vals) < 3:
        return vals
    keep = [False] * len(vals)
    keep[0] = keep[-1] = True
    stack = [(0, len(vals) - 1)]
    while stack:
        i, j = stack.pop()
        if j - i < 2:
            continue
        t0, v0 = vals[i]
        t1, v1 = vals[j]
        span = (t1 - t0) or 1.0
        worst, at = 0.0, -1
        for m in range(i + 1, j):
            tm, vm = vals[m]
            s = (tm - t0) / span
            e = max(abs(vm[k] - (v0[k] + (v1[k] - v0[k]) * s)) for k in range(3))
            if e > worst:
                worst, at = e, m
        if worst > tol and at > 0:
            keep[at] = True
            stack += [(i, at), (at, j)]
    return [v for v, k in zip(vals, keep) if k]


# 裁剪容差以**位移**为准，单位是建模单位（= 纹理像素）。旋转通道的角度容差由该骨的
# 力臂反推：同样 0.12°，落在下颌上是零点零几像素，落在肩关节上要乘以整条翼展。写死一个
# 角度容差的后果是大档翼尖被裁出两个像素的漂移，而下颌那边白白留了一堆冗余帧。
# 还要再除以链深：一条 19 节的颈上每节各自允许 0.12px，末端累加就是两个多像素。按深度
# 分摊之后，整条链的总偏差才被压在 PRUNE_PX 以内（保守，但这是唯一不用解耦合的写法）。
PRUNE_PX = 0.12


def _prune_tol(chan: str, reach: float, depth: int) -> float:
    px = PRUNE_PX / max(depth, 1)
    if chan == "position":
        return px
    return math.degrees(px / max(reach, 1e-3)) if chan == "rotation" else px / max(reach, 1e-3)


def build_tracks(rig: PoseRig, sampler, length: float, loop: bool, n: int,
                 extra: tuple[float, ...] = ()) -> dict[str, dict[str, list]]:
    """采样 → 每骨每通道的 (时间, 三元组) 序列。恒定通道直接丢掉，共线帧裁掉。

    extra 是**必须落点**（t01）：动作里有真折角的地方（振翅的上下死点、蹬地的换向）
    如果没有采样点正好压在上面，线性插值就把那个角切掉了 —— 而且切掉多少取决于最近的
    两个采样离它多远，于是"加密采样"反而可能更差，误差曲线上下横跳、怎么加都不收敛。
    """
    ts = sorted({i / n for i in range(n + 1)} | {t for t in extra if 0.0 < t < 1.0})
    frames = animcore.sample_frames(sampler, length, loop, ts)

    tracks: dict[str, dict[str, list]] = {}
    for bone in rig.order:
        for chan, attr, default in animcore.CHANNELS:
            vals = animcore.channel_values(bone, attr, default, frames)
            if chan == "rotation":
                animcore.unwrap_degrees(vals)
            if animcore.is_constant_default(vals, default):
                continue
            tracks.setdefault(bone, {})[chan] = _prune(
                vals, _prune_tol(chan, rig.reach(bone), rig.depth(bone)))
    return tracks


def write_animated_bbmodel(rig: PoseRig, anims: list[dict], out: Path, name: str) -> None:
    """把动画塞进 bbmodel 的 animations（Blockbench 直接打开、GeckoLib codec 可导出）。

    骨 uuid 从**已解析的骨树**取，不从 doc["groups"] —— 生成器出的 fmt 4.x 根本没有
    groups 数组，照那里取会得到一份空表，动画静悄悄地绑不上任何骨头。
    """
    doc = json.loads(json.dumps(rig.doc))
    entries = []
    for a in anims:
        animators = animcore.animators_of(
            a["tracks"],
            lambda bone: rig.bones[bone].uuid,
            lambda bone, chan, i, _n=a["name"]: f"{_n}{bone}{chan}{i}",
        )
        entries.append(animcore.animation_entry(name, a["name"], a["length"], a["loop"],
                                                animators))
    doc["animations"] = entries
    doc["name"] = name
    doc["model_identifier"] = name
    # 强制 4.10 落盘：本函数是"读源模型 → 挂动画 → 写回"，格式版本跟着源文件走。源模型
    # 一旦被 Blockbench 5 手工存过盘，产物就悄悄变成 5.0，而 5.0 在 4.x 里打开是一个
    # cube 都看不见（见 to_fmt410 的说明）。不报错、不闪退，只是空场景。
    Path(out).write_text(json.dumps(ensure_410(doc), ensure_ascii=False))


def write_geckolib(anims: list[dict], out: Path, namespace: str, model_id: str) -> None:
    """直出 GeckoLib animation.json —— **参考用，未经引擎侧验证，别直接当资产提交**。

    Bedrock 动画的旋转符号约定与 Blockbench 面板显示是否一致（X/Y 是否取反），仓库里
    没有可对拍的同源实例。正经路径是把带动画的 .bbmodel 交给
    modelScript/core/bbmodel_to_geckolib.py（驱动 Blockbench 官方 codec 导出），由 codec
    负责这层约定；本函数只用于人眼查曲线和兜底。
    """
    entries = [(a["name"], a["length"], a["loop"], a["tracks"]) for a in anims]
    Path(out).write_text(
        json.dumps(animcore.geckolib_document(entries, namespace, model_id),
                   indent="\t", ensure_ascii=False))
