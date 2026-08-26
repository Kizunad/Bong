#!/usr/bin/env python3
"""体素生物**动画**的公共底座：读 bbmodel 骨树 → 正解 / 逆解 / 平衡 → 写回关键帧。

和建模层的 voxel_rig.py 是一对：那边负责"长什么样"，这边负责"怎么动"。来源同样是
怠怒之狮 dainu_lion/rig.py + gen_anim.py 里那套自带实现；那份就地留着不动（已合入
主线），本模块把其中**与物种无关**的部分抽出来给后续生物共用：

  Bone / Channel / Pose   骨、单骨一帧的变换、整只一帧的姿态
  Rig.world()             正解（父先于子，一遍算完）
  Rig.solve_limb()        肢体逆解（腿根按比例预定 + 剩余两节闭式解，对目标处处连续）
  Rig.mass_center()       体积加权质心 —— 两足动物"重心必须落在支撑脚上"要靠它
  Rig.chain_bends()       量出一条链的静止折角，用来把弯链**拉直**
  keyed / pulse / …       关键点插值与脉冲
  write_bbmodel / …       采样结果 → bbmodel animations / GeckoLib animation.json

物种相关的东西（哪几根骨是腿、各关节限位、静止站姿、平衡策略）留给各生物自己的
rig.py，本模块一概不猜。

坐标沿用建模层：16 单位 = 1 格 = 1 米，地面 y=0，生物头朝 −z。骨旋转沿用 Blockbench
的 R = Rz·Ry·Rx，绕自身 pivot、在父骨已变换的坐标系里施加，与 render_bbmodel.py 对
元素的处理一致。
"""

from __future__ import annotations

import json
import math
import sys
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import animcore  # noqa: E402


# ---------------------------------------------------------------- 线性代数 / 曲线
# 这些原本在本模块和 animkit.py 里各写了一份逐字相同的实现。现在统一在 animcore，
# 这里只做转发 —— 既有 `from anim_rig import rotmat, smooth, ...` 的调用点一个不用改。
rotmat = animcore.rotmat
euler = animcore.euler
affine = animcore.affine
wrap = animcore.wrap
smooth = animcore.smooth
ease_out = animcore.ease_out
pulse = animcore.pulse
keyed = animcore.keyed
jitter = animcore.jitter
decay_shake = animcore.decay_shake


def euler_of(R: np.ndarray) -> list[float]:
    """`euler()` 的逆：旋转矩阵 → Blockbench 顺序（Rz·Ry·Rx）的三个角（度）。

    本模块的调用点把它当三元序列用（拆包、直接塞进 Channel.rot），所以维持 list
    返回值；animkit 那份返回 ndarray。算法本身在 `animcore.euler_xyz`。
    """
    return list(animcore.euler_xyz(R))


# ---------------------------------------------------------------- 骨 / 姿态
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


# ---------------------------------------------------------------- Rig
class Rig:
    """从 bbmodel 读骨树，提供正解 / 逆解 / 几何量测。"""

    def __init__(self, path: Path):
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

        self.elements = {e["uuid"]: e for e in d["elements"] if e.get("type", "cube") == "cube"}
        self._contact: dict[str, np.ndarray] = {}
        self._sole_half: dict[str, np.ndarray] = {}   # 掌底角点相对接触点的偏移
        self._pts: dict[str, np.ndarray] = {}
        self._mass: dict[str, tuple[np.ndarray, np.ndarray]] = {}

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

    def joint(self, name: str, W: dict[str, np.ndarray]) -> np.ndarray:
        b = self.bones[name]
        return W[name][:3, :3] @ b.origin + W[name][:3, 3]

    def element_xform(self, pose: Pose) -> dict[str, np.ndarray]:
        W = self.world(pose)
        return {u: W[n] for n in self.order for u in self.bones[n].elements}

    def point(self, bone: str, local, pose: Pose | None = None) -> np.ndarray:
        """挂在某骨上的一个定位点 → 世界坐标（喷口 / 蛋位 / 挂点都用它）。"""
        W = self.world(pose)
        return (W[bone] @ np.append(np.asarray(local, float), 1.0))[:3]

    # ---------- 几何 ----------

    @staticmethod
    def corners(e: dict) -> np.ndarray:
        f, t = np.array(e["from"], float), np.array(e["to"], float)
        pts = np.array([[x, y, z] for x in (f[0], t[0]) for y in (f[1], t[1]) for z in (f[2], t[2])])
        rot = e.get("rotation", [0, 0, 0])
        if any(rot):
            o = np.array(e.get("origin", [0, 0, 0]), float)
            pts = (euler(rot) @ (pts - o).T).T + o
        return pts

    def bone_points(self, name: str) -> np.ndarray:
        if name not in self._pts:
            pts = [self.corners(self.elements[u]) for u in self.bones[name].elements if u in self.elements]
            self._pts[name] = np.vstack(pts) if pts else np.zeros((0, 3))
        return self._pts[name]

    def bone_mass(self, name: str) -> tuple[np.ndarray, np.ndarray]:
        """(每件体积, 每件静止姿形心)。旋转不改体积，形心就是 8 角点均值。"""
        if name not in self._mass:
            vols, cens = [], []
            for u in self.bones[name].elements:
                e = self.elements.get(u)
                if e is None:
                    continue
                f, t = np.array(e["from"], float), np.array(e["to"], float)
                vols.append(float(np.prod(np.maximum(t - f, 1e-3))))
                cens.append(self.corners(e).mean(axis=0))
            self._mass[name] = (np.array(vols), np.array(cens).reshape(-1, 3))
        return self._mass[name]

    def mass_center(self, pose: Pose | None = None) -> np.ndarray:
        """体积加权质心（世界坐标）。

        两足动物的动画对不对，第一判据就是它：单支撑相里质心必须落在支撑脚上方，
        否则这只鹅是靠"观众看不出来"站着的。体素密度不等于真密度，但口径一致就能
        用来量帧间变化。
        """
        W = self.world(pose)
        num = np.zeros(3)
        den = 0.0
        for n in self.order:
            vols, cens = self.bone_mass(n)
            if not len(vols):
                continue
            wp = cens @ W[n][:3, :3].T + W[n][:3, 3]
            num += (wp * vols[:, None]).sum(axis=0)
            den += float(vols.sum())
        return num / max(den, 1e-9)

    def lowest(self, pose: Pose | None = None) -> float:
        """整只模型的最低点世界 y（穿地检查、倒地贴地夹持都要它）。"""
        W = self.world(pose)
        lo = 1e9
        for n in self.order:
            pts = self.bone_points(n)
            if len(pts):
                lo = min(lo, float((pts @ W[n][:3, :3].T + W[n][:3, 3])[:, 1].min()))
        return lo

    def lowest_parts(self, pose: Pose | None = None, k: int = 3) -> list[tuple[float, str]]:
        """最低的 k 根骨。只报一个总最低点，排查时还得自己翻是哪块。"""
        W = self.world(pose)
        rows = []
        for n in self.order:
            pts = self.bone_points(n)
            if len(pts):
                rows.append((float((pts @ W[n][:3, :3].T + W[n][:3, 3])[:, 1].min()), n))
        return sorted(rows)[:k]

    def blocked_below(self, world_pt, pose: Pose | None = None, *, skip: tuple[str, ...] = ()) -> list[str]:
        """从某点垂直向下，被自己身上哪些**件**挡住。返回件名（空 = 通畅）。

        排泄口 / 产卵口这类"要往下掉东西"的挂点，光看坐标对不对没用 —— 得确认它
        下方没有自己的身体，否则掉出来的东西是从躯干里冒出来的。

        逐 element 判，不能逐骨：一根骨上挂五块的话，合并包围盒会把块与块之间的空当
        也算成实心（躯干那五块合起来正好把尾下这块空间填满，实际是通的）。
        """
        W = self.world(pose)
        x, y, z = (float(v) for v in world_pt)
        hit = []
        for n in self.order:
            if n in skip:
                continue
            M = W[n]
            for u in self.bones[n].elements:
                e = self.elements.get(u)
                if e is None:
                    continue
                wp = self.corners(e) @ M[:3, :3].T + M[:3, 3]
                lo, hi = wp.min(axis=0), wp.max(axis=0)
                if lo[0] <= x <= hi[0] and lo[2] <= z <= hi[2] and lo[1] < y:
                    hit.append(e.get("name", u))
        return hit

    def contact_point(self, foot: str) -> np.ndarray:
        """脚掌着地点（静止姿模型坐标）：取该骨最低一层角点的形心。"""
        if foot not in self._contact:
            pts = self.bone_points(foot)
            sole = pts[pts[:, 1] <= pts[:, 1].min() + 0.75]
            self._contact[foot] = sole.mean(axis=0)
            self._sole_half[foot] = sole - self._contact[foot]
        return self._contact[foot]

    def sole_lift(self, foot: str, pitch: float = 0.0, roll: float = 0.0) -> float:
        """脚掌绕**接触点**转了 pitch/roll 之后，要抬多少才能让最低那个角回到地面。

        不能用"半个掌长 × sinθ"糊弄。接触点取的是掌底角点的**形心**，而蹼板是三级
        递宽的（形心 z −1.51，几何中点 z −0.93，差 0.58）。按半掌长补会多抬 0.14 ——
        蹬离那一瞬整只脚离地 0.12，后验里表现为「双支撑被误判成单支撑，横向平衡余量
        假负 0.18」。逐角点算就没有这个偏差，顺带把侧倾也一起补了。
        """
        self.contact_point(foot)
        rel = self._sole_half[foot]
        if not (pitch or roll):
            return 0.0
        turned = rel @ (rotmat(roll, 2) @ rotmat(pitch, 0)).T
        # 返回**旋转带来的增量**，不是绝对下沉量：目标 y 用的是接触点高度，未旋转时
        # 掌底本来就正好贴地，这里再叠一次绝对量的话整只脚凭空抬 0.29。
        return max(0.0, float(rel[:, 1].min()) - float(turned[:, 1].min()))

    def chain_bends(self, chain) -> list[float]:
        """链上每个关节的**静止折角**（度，绕 X / 矢状面），对齐 chain[1:-1]。

        用途：把一条弯着的链**拉直**。骨骼静止旋转全是 0，弯曲信息只藏在 pivot 连成
        的折线里 —— 所以"拉直"不能靠给每节加同一个角度（那只会折得更弯），得先量出
        每个关节相对上一段偏了多少度，再逐节抵掉。给 bone i 施加 rot[0] = −bends[i]
        就让它的出段与入段共线，全链施加 = 一根直棍。
        """
        piv = [self.bones[n].origin for n in chain]
        segs = [complex(b[1] - a[1], b[2] - a[2]) for a, b in zip(piv, piv[1:])]
        out = []
        for prev, cur in zip(segs, segs[1:]):
            out.append(math.degrees(np.angle(cur / prev)) if abs(prev) > 1e-9 else 0.0)
        return out

    # ---------- 逆解 ----------

    def solve_limb(self, pose: Pose, chain, target, *, limits: dict,
                   abduct: float = 15.0, share: float = 0.5, tip_pitch: float = 0.0) -> float:
        """四节肢逆解（根 + 两连杆 + 末端）：根按比例预定 + 剩余两节闭式解。

        为什么不是 CCD：三节链对一个点目标是冗余的，CCD 每帧独立求解、又没有时间
        连续性，目标一逼近可达边界就在两个解支之间跳，动画里是腿凭空翻一下。解析解
        没有这个自由度：根角由目标方向按固定比例给出，两连杆的弯曲方向由静止姿的符号
        锁死，目标超出可达范围就夹到边界（腿伸直），因此对目标处处连续。

        三步：① 绕 Z 定外展，把目标转进肢体的矢状面（躯干侧倾会把根横向挪走，纯矢状
        解补不了）② 面内闭式解 ③ 末端摆到指定**世界**俯仰。

        limits: 去掉左右后缀的骨名 → (下限, 上限)，度。限位不是装饰：不夹的话解会
        把反关节折向反面，渲出来是断腿。
        """
        chain = list(chain)
        tip = chain[-1]
        b0, b1, b2 = (self.bones[n] for n in chain[:3])
        base_bone = self.bones[chain[0]].parent
        W = self.world(pose)
        P = W[base_bone] if base_bone else np.eye(4)
        Pinv = np.linalg.inv(P)

        # 目标（掌心）→ 末端关节目标：先定末端世界俯仰，反推关节该在哪
        tip_rest = self.bones[tip].origin
        sole_off = rotmat(tip_pitch, 0) @ (self.contact_point(tip) - tip_rest)
        t_local = (Pinv @ np.append(np.asarray(target, float) - sole_off, 1.0))[:3]

        o0, o1, o2, o3 = b0.origin, b1.origin, b2.origin, tip_rest

        # ① 外展：绕 Z 转多少能把目标转进可解平面。平面是 x = **末端关节的静止 x**，
        # 不是 x = 肢根 x —— 链上只有绕 X 的转，x 分量根本改不了。
        dx0 = o3[0] - o0[0]
        dx, dy = t_local[0] - o0[0], t_local[1] - o0[1]
        r = math.hypot(dx, dy)
        yr = -math.sqrt(max(0.0, r * r - dx0 * dx0))
        tz = math.degrees(math.atan2(dy, dx) - math.atan2(yr, dx0)) if r > 1e-6 else 0.0
        tz = min(abduct, max(-abduct, (tz + 180.0) % 360.0 - 180.0))
        pose[chain[0]].rot[2] = tz
        u = np.array([o0[0] + dx0, o0[1] + yr, t_local[2]])

        # ② 面内闭式解：YZ 平面当复平面，绕 X 转 = 乘 e^{iθ}
        def cx(v):
            return complex(v[1], v[2])

        a, b, c = cx(o1 - o0), cx(o2 - o1), cx(o3 - o2)
        D, D_rest = cx(u - o0), cx(o3 - o0)

        key0 = chain[0].rsplit("_", 1)[0]
        lo0, hi0 = limits.get(key0, (-45.0, 45.0))
        swing = math.degrees(np.angle(D / D_rest)) if abs(D_rest) > 1e-9 else 0.0
        th0 = share * swing

        # 根只按比例给会在远端够不着：剩下两节的可达环是 [|lb−lc|, lb+lc]，先算出让目标
        # 落进这个环所需的最小根修正，不够就补足（补足量随距离连续变化，不会像"解不出来
        # 就夹住"那样在边界上跳）。
        lb, lc = abs(b), abs(c)
        if abs(D) > 1e-9 and abs(a) > 1e-9:
            # 两个 np.angle 相减不自动归一化，直接用会把 313.6° 当成有效夹角。
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
            lo, hi = limits.get(k, (-45.0, 45.0))
            pose[name].rot[0] = min(hi, max(lo, (th + 180.0) % 360.0 - 180.0))

        # ③ 末端摆到指定**世界**俯仰。除了肢链自身，还得抵掉躯干带来的俯仰 —— 只抵肢链
        # 时，躯干一低头脚掌就跟着倾，蹲姿帧整只脚会扎进地里。
        fwd = P[:3, :3] @ np.array([0.0, 0.0, -1.0])
        base_pitch = math.degrees(math.atan2(fwd[1], -fwd[2]))
        pose[tip].rot[0] = tip_pitch - base_pitch - sum(pose[n].rot[0] for n in chain[:3])
        right = P[:3, :3] @ np.array([1.0, 0.0, 0.0])
        pose[tip].rot[2] = -math.degrees(math.atan2(right[1], right[0])) - tz
        return float(np.linalg.norm(self.limb_tip(pose, chain) - np.asarray(target, float)))

    def limb_tip(self, pose: Pose, chain) -> np.ndarray:
        W = self.world(pose)
        tip = chain[-1]
        return (W[tip] @ np.append(self.contact_point(tip), 1.0))[:3]


# ---------------------------------------------------------------- 导出
_uuid = animcore.stable_uuid


def _kf(channel: str, time: float, vec, idx: int, aname: str) -> dict:
    """关键帧。种子拼成 `名+骨+通道 + 通道 + 序号`（通道名出现两次）—— 这是本模块的
    历史拼法，动它会让既有产物的 uuid 全变，而 uuid 只是 Blockbench 的索引键。"""
    return animcore.keyframe(channel, time, vec, f"{aname}{channel}{idx}")


def build_tracks(rig: Rig, sampler, length: float, loop: bool, n: int):
    """采样 → 每骨每通道的 (时间, 三元组) 序列。恒定通道直接丢掉。"""
    frames = animcore.sample_frames(sampler, length, loop, [i / n for i in range(n + 1)])

    tracks: dict[str, dict[str, list]] = {}
    for bone in rig.order:
        for chan, attr, default in animcore.CHANNELS:
            vals = animcore.channel_values(bone, attr, default, frames)
            if animcore.is_constant_default(vals, default):
                continue
            # 恒定但非默认的通道只留首末两帧。整条动画都是同一个值的通道（典型是"其余
            # 骨保持某个非静止姿"）逐帧写没有任何信息量，却按采样数线性吃关键帧——
            # 缝合兽有 17 条芽，每条嫁接动画里另外 16 条都恒定压在休眠尺寸，不收的话
            # 光这一项就是 16×采样数。留两帧而不是一帧：单帧在部分动画运行时会被当成
            # 只在该时刻生效，两帧则任何插值方式下都恒定。
            first = vals[0][1]
            if len(vals) > 2 and all(all(abs(v[k] - first[k]) < 1e-4 for k in range(3))
                                     for _, v in vals):
                vals = [vals[0], vals[-1]]
            tracks.setdefault(bone, {})[chan] = vals
    return tracks


def write_bbmodel(src: Path, out: Path, model_name: str, entries) -> Path:
    """把动画塞进 bbmodel 的 animations（Blockbench 可直接打开、GeckoLib codec 可导出）。

    entries: [(名字, 时长秒, 是否循环, tracks)]。源模型只读不写 —— 用户在 Blockbench
    里的手改留在那一份里。
    """
    doc = json.loads(Path(src).read_text())
    uuids = {g["name"]: g["uuid"] for g in doc.get("groups", [])}
    if not uuids:   # voxel_rig 直出的文件把骨骼写在 outliner 里，没有 groups 段
        def collect(node):
            if isinstance(node, str):
                return
            uuids[node["name"]] = node["uuid"]
            for c in node.get("children", []):
                collect(c)
        for root in doc["outliner"]:
            collect(root)

    anims = []
    for name, length, loop, tracks in entries:
        animators = animcore.animators_of(
            tracks,
            lambda bone: uuids[bone],
            lambda bone, chan, i, _n=name: f"{_n}{bone}{chan}{chan}{i}",
        )
        anims.append(animcore.animation_entry(model_name, name, length, loop, animators))
    doc["animations"] = anims
    doc["name"] = model_name
    doc["model_identifier"] = model_name
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(doc, ensure_ascii=False))
    return out


def write_geckolib(out: Path, namespace: str, model_id: str, entries) -> Path:
    """直出 GeckoLib animation.json —— **参考用，未经引擎侧验证，不要直接当资产提交**。

    Bedrock 动画的旋转符号约定和 Blockbench 面板显示是否一致（X/Y 是否取反），仓库里
    没有可对拍的同源实例。正经路径是把带动画的 .bbmodel 交给
    modelScript/core/bbmodel_to_geckolib.py（驱动 Blockbench 官方 codec 导出），由 codec
    负责这层约定；本函数只用于人眼查看曲线和兜底。
    """
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(animcore.geckolib_document(entries, namespace, model_id),
                              indent="\t", ensure_ascii=False))
    return out
