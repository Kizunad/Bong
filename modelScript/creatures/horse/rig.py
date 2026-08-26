#!/usr/bin/env python3
"""马 —— 绑定层：从 bbmodel 读骨树，做正解 / 逆解 / 摆姿。

动画不手搓关键帧。四足动物的腿一旦靠"看着差不多"拧角度，落地那一刻蹄必然在地上滑
（foot skate）——这是劣质四足动画唯一最大的破绽，而且渲染静帧看不出来，只能靠算：
支撑相里蹄的世界坐标必须贴地、且以恒定速度后移。所以这里给腿装真逆解，动画只给
「蹄该踩在哪」，关节角由 IK 解出来，再用 `contact_report()` 量残差。

与猫科绑定的两处结构差异：
  · **前肢比后肢多一节**——前肢自肩胛起（肩胛 → 肱骨 → 桡骨 → 腕），后肢自股骨起
    （股骨 → 胫骨 → 跗 → 球节）。所以两条链的"末端骨"不同层级：前肢末端是腕（其下
    挂整根管骨），后肢末端是球节（其下只挂系骨与蹄）。着地点一律从末端骨**连同其所有
    子孙**的最低面取，不看它挂在哪一节。
  · 马蹄只有一个着地面（第三指），不像猫掌有四趾垫——着地点就是蹄底那一片。

坐标沿用模型约定：16 单位 = 1 格 = 1 米，地面 y=0，兽头朝 −z。骨旋转沿用 Blockbench
element 的 R = Rz·Ry·Rx，绕自身 pivot、在父骨已变换的坐标系里施加。

**骨动画通道另有一套符号约定**，见 `bb_rot` / `bb_pos`——这里的 Pose 一律用几何约定，
只在写盘/回读那一层转换。
"""

from __future__ import annotations

import json
import math
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
FINAL = Path(__file__).resolve().parents[2] / "models" / "horse"

# 腿链：前三节由 IK 解，末端骨只定世界俯仰（其下的管骨/系/蹄随之刚性摆动）
FORELEG = ("scapula_{s}", "humerus_{s}", "radius_{s}", "carpus_{s}")
HINDLEG = ("femur_{s}", "tibia_{s}", "tarsus_{s}", "fetlock_h_{s}")
SPINE = ("hips", "lumbar", "thorax_back", "thorax_front", *(f"neck_{i + 1}" for i in range(7)), "skull")
TAIL = tuple(f"tail_{i:02d}" for i in range(1, 9))

# 各关节相对静止姿的活动范围（度，绕 X = 矢状面）。限位不是装饰：不夹的话解会把
# 肘/跗反关节解到反向折叠，渲出来是断腿。
# 数值按马的实测关节活动度（马比猫科**硬**得多——这是承重与长距离行进的代价）：
LIMITS = {
    "scapula": (-17.0, 17.0),  # 马的肩胛摆幅远小于猫科（15-20°），别照抄 ±26
    "humerus": (-52.0, 52.0),
    # 肘/跗的**负向**是伸展。首版按 −30/−24 夹，蹬离相里两者双双顶死限位，蹄从
    # 目标点掉下去 0.6-1.4 单位（逐帧打 IK 才看得见：残差在支撑相前 2/3 是 0.00，
    # 一到蹬离就跳到 1.8，且角度栏带 ! ）。放到接近真马的伸展极限。
    "radius": (-44.0, 78.0),  # 正 = 腕往后折（前肢只能往后折，不能反向）
    "femur": (-42.0, 46.0),
    "tibia": (-58.0, 26.0),  # 负 = 膝往前折
    "tarsus": (-40.0, 66.0),  # 正 = 跗往后折
}
ABDUCT = 9.0  # 腿根外展上限（绕 Z）。马的四肢几乎在矢状面内摆，给太大就成了螃蟹步

# 腿根分担整肢摆动的比例，剩下的由闭式二连杆解。
# 给少了远端关节被迫代偿：首版 0.34 时肩胛在整个步周期只摆了 11°（可用 ±17 用了
# 三分之一），代价全压在肘上，蹬离相肘顶限位。腿根多担一点，远端就松了。
FORE_SHARE = 0.52  # 肩胛
HIND_SHARE = 0.58  # 股骨


# ---------------------------------------------------------------- Blockbench 骨动画通道约定
# **实测**于 web.blockbench.net（探针见 verify_anim.py --probe 打印的复现步骤），不是推测：
# 往 animator 写 rotation (rx,ry,rz)，场景里拿到的是 Rz(rz)·Ry(−ry)·Rx(−rx)；
# 写 position (px,py,pz)，拿到的位移是 (−px, +py, +pz)；scale 绕 pivot 逐轴相乘、同号。
#
# 注意 element **自身**的静态 rotation 不走这套（那是 Rz·Ry·Rx、三轴同号，与本文件的
# `euler()` 一致）——所以几何层不受影响，只有动画通道要转。这个不对称是 Bedrock 动画格式
# 带进来的历史包袱，Blockbench 的 BoneAnimator 照单全收。
#
# 首版漏了这层转换：预览渲的是内存里的 Pose，Blockbench 播的是写盘的关键帧，两者俯仰
# 相反。表现是"预览全绿、Blockbench 里马头朝下 / 四肢外翻散架"。
# 转换是**对合**的（自己是自己的逆），写盘与回读共用同一对函数。
def bb_rot(v) -> list[float]:
    return [-v[0], -v[1], v[2]]


def bb_pos(v) -> list[float]:
    return [-v[0], v[1], v[2]]


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


class Pose(dict):
    """bone name → Channel，缺省即静止姿。"""

    def __missing__(self, key: str) -> Channel:
        ch = Channel()
        self[key] = ch
        return ch


class Rig:
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

        self.elements = {e["uuid"]: e for e in d["elements"]}
        self._contact: dict[str, np.ndarray] = {}
        self._sole_half: dict[str, float] = {}
        self._pts: dict[str, np.ndarray] = {}
        self._deep: dict[str, np.ndarray] = {}
        self._rest_stance: dict[str, np.ndarray] | None = None
        # 逆解残差流水。solve_leg 一直**返回**残差，但所有调用方都把它丢了——目标够不到时
        # 关节顶死限位、蹄停在半空，而这在静帧上完全看不出来。自检从这里读。
        self.residuals: list[tuple[str, float]] = []
        # 上层（步态可达域 / 吃草颈弯 / 倒毙收量）的求解缓存挂在**实例**上。
        # 别用 `id(rig)` 当外部字典的键：Rig 被回收后新对象会复用同一地址，缓存就跨体型
        # 串味——实测表现是同一份文件连跑两次结果不同（矮马全绿、挽马一片红）。
        self.cache: dict = {}

    # ---------- 正解 ----------

    def _local(self, b: Bone, ch: Channel) -> np.ndarray:
        """骨自身的仿射：绕 pivot 旋转/缩放，再按 pos 平移（Bedrock 语义）。"""
        R = euler(ch.rot)
        if any(abs(s - 1.0) > 1e-6 for s in ch.scale):
            R = R @ np.diag(ch.scale)
        o = b.origin
        return affine(R, o + np.array(ch.pos, float) - R @ o)

    def world(self, pose: Pose | None = None) -> dict[str, np.ndarray]:
        pose = pose if pose is not None else Pose()
        out: dict[str, np.ndarray] = {}
        for name in self.order:
            b = self.bones[name]
            A = self._local(b, pose[name]) if name in pose else self._local(b, Channel())
            out[name] = A if b.parent is None else out[b.parent] @ A
        return out

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

    def deep_points(self, name: str) -> np.ndarray:
        """本骨 + 全部子孙的角点（静止姿下即模型坐标，因为静止姿所有局部变换都是单位阵）。

        马的着地面挂在末端骨的**子孙**上（前肢末端是腕，蹄在其下三级），只看本骨的
        element 会取到管骨顶端当"蹄底"，四蹄整体悬空一整根管骨的高度。
        """
        if name not in self._deep:
            acc = [self.bone_points(name)]
            for c in self.bones[name].children:
                acc.append(self.deep_points(c))
            acc = [a for a in acc if len(a)]
            self._deep[name] = np.vstack(acc) if acc else np.zeros((0, 3))
        return self._deep[name]

    def lowest(self, pose: Pose | None = None) -> float:
        W = self.world(pose)
        lo = 1e9
        for n in self.order:
            pts = self.bone_points(n)
            if len(pts):
                lo = min(lo, float((pts @ W[n][:3, :3].T + W[n][:3, 3])[:, 1].min()))
        return lo

    def contact_point(self, foot: str) -> np.ndarray:
        """蹄底着地点（静止姿模型坐标）：取该骨（含子孙）最低一层角点的形心。"""
        if foot not in self._contact:
            pts = self.deep_points(foot)
            lo = pts[:, 1].min()
            # 取带必须**窄**：挽马的距毛下缘离地只有 0.26，带宽 0.45 会把毛的底面
            # 一起算进"蹄底"，触地点被平均到 y=+0.13，四蹄整体悬空。蹄底是平面，
            # 0.10 足够收全底面四角。
            sole = pts[pts[:, 1] <= lo + 0.10]
            self._contact[foot] = sole.mean(axis=0)
            self._sole_half[foot] = float(sole[:, 2].max() - sole[:, 2].min()) / 2
        return self._contact[foot]

    def sole_half(self, foot: str) -> float:
        """蹄心到蹄尖的距离。绕蹄心翻蹄时用它把蹄尖抬回地面。"""
        self.contact_point(foot)
        return self._sole_half[foot]

    def element_xform(self, pose: Pose) -> dict[str, np.ndarray]:
        W = self.world(pose)
        return {u: W[n] for n in self.order for u in self.bones[n].elements}

    # ---------- 逆解 ----------

    def leg_chain(self, side: str, hind: bool) -> list[str]:
        return [n.format(s=side) for n in (HINDLEG if hind else FORELEG)]

    def solve_leg(self, pose: Pose, side: str, hind: bool, target: np.ndarray, *,
                  foot_pitch: float = 0.0, level: float = 1.0, refine: int = 3) -> float:
        """腿逆解 + 不动点修正。返回蹄底落点残差。

        闭式解把"蹄底相对踝的偏移"当成世界空间的常量减掉，这在躯干接近直立时误差可忽略，
        但躯干**大幅旋转**时会整段失准——倒毙动画侧翻 84° 后前肢残差 5.3，而同一目标在
        不侧翻时是 0.00。补偿项推起来要卷进整条链的 Rz/Rx 复合次序，不如直接迭代：
        解一次 → 量实际落点 → 把误差加回目标再解。三轮内收敛到 <0.01，且修正量随目标
        连续变化，不会在边界跳解支。
        """
        goal = np.asarray(target, float)
        adj = goal.copy()
        kw = dict(foot_pitch=foot_pitch, level=level)
        best = self._solve_leg_once(pose, side, hind, adj, **kw)
        for _ in range(refine):
            err = goal - best
            if float(np.linalg.norm(err)) < 1e-3:
                break
            adj = adj + err
            best = self._solve_leg_once(pose, side, hind, adj, **kw)
        resid = float(np.linalg.norm(best - goal))
        self.residuals.append((f"{'h' if hind else 'f'}{side}", resid))
        return resid

    def _solve_leg_once(self, pose: Pose, side: str, hind: bool, target: np.ndarray, *,
                        foot_pitch: float = 0.0, level: float = 1.0) -> np.ndarray:
        """单趟闭式解。返回蹄底**实际**落点（世界坐标）。

        为什么不是 CCD：三节链对一个点目标是冗余的，CCD 每帧独立求解、又没有时间连续性，
        目标一逼近可达边界就在两个解支之间跳，动画里是腿凭空翻一下。解析解没有这个自由
        度：腿根角由目标方向按固定比例给出，两连杆的弯曲方向由静止姿的符号锁死，目标
        超出可达范围就夹到边界（腿伸直），因此对目标处处连续。

        三步：① 绕 Z 定外展，把目标转进腿的矢状面 ② 面内闭式解 ③ 末端骨摆到指定世界俯仰。
        """
        chain = self.leg_chain(side, hind)
        foot = chain[-1]
        b0, b1, b2 = (self.bones[n] for n in chain[:3])
        base_bone = self.bones[chain[0]].parent
        W = self.world(pose)
        P = W[base_bone] if base_bone else np.eye(4)
        Pinv = np.linalg.inv(P)

        ankle_rest = self.bones[foot].origin
        sole_rel = rotmat(foot_pitch, 0) @ (self.contact_point(foot) - ankle_rest)
        # level=1：末端骨摆到指定**世界**俯仰（承重的蹄要平贴地面）；level=0：俯仰相对
        # **躯干**。侧卧 / 腾空的肢体必须往 0 走——躺倒的马蹄是跟着身体转的，硬摆平会给
        # 末端骨补上 +84° 的 Z 旋转，把蹄甩到体侧 10 单位外，二连杆解出来的踝位再准也
        # 没用（倒毙首版残差 4.5 就是这么来的）。
        # 取**连续插值**而不是布尔：布尔在切换那一帧整只蹄瞬移，关键帧线性插值会把这一跳
        # 摊成半个周期的抽搐，而这恰好是本轮要根除的那类不连续。
        sole_off = level * sole_rel + (1.0 - level) * (P[:3, :3] @ sole_rel)
        t_local = (Pinv @ np.append(np.asarray(target, float) - sole_off, 1.0))[:3]

        o0, o1, o2, o3 = b0.origin, b1.origin, b2.origin, ankle_rest

        # ① 外展：链上只有绕 X 的转，x 分量改不了，末端恒落在自己的静止 x 上。
        # 所以可解平面是 x = **末端骨的静止 x**，不是 x = 腿根 x。
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

        # 腿根只按比例给会在远端够不着：剩下两节的可达环是 [|lb−lc|, lb+lc]，先算出让目标
        # 落进这个环所需的最小腿根修正，不够就补足（补足量随距离连续变化，不会像"解不出
        # 来就夹住"那样在边界上跳）。
        lb, lc = abs(b), abs(c)
        if abs(D) > 1e-9 and abs(a) > 1e-9:
            # 两个 np.angle 相减不自动归一化，不折回 ±180 会把腿根一路推到限位
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
        d = min(lb + lc - 1e-4, max(abs(lb - lc) + 1e-4, d))  # 夹进可达环 → 连续
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

        # ③ 末端骨摆到指定俯仰。world_level 时还要抵掉躯干带来的俯仰与侧倾——只抵腿链
        # 时，胸椎一低头蹄子就跟着倾，低头吃草那一帧整只前蹄扎进地下。
        fwd = P[:3, :3] @ np.array([0.0, 0.0, -1.0])
        right = P[:3, :3] @ np.array([1.0, 0.0, 0.0])
        base_pitch = level * math.degrees(math.atan2(fwd[1], -fwd[2]))
        base_roll = level * math.degrees(math.atan2(right[1], right[0]))
        pose[foot].rot[0] = foot_pitch - base_pitch - sum(pose[n].rot[0] for n in chain[:3])
        pose[foot].rot[2] = -base_roll - tz
        return self.foot_world(pose, side, hind)

    def foot_world(self, pose: Pose, side: str, hind: bool) -> np.ndarray:
        W = self.world(pose)
        foot = self.leg_chain(side, hind)[-1]
        return (W[foot] @ np.append(self.contact_point(foot), 1.0))[:3]

    def reach_span(self, side: str, hind: bool, *, tol: float = 0.08, lo: float = 0.0, hi: float = 24.0) -> tuple[float, float]:
        """静止姿下蹄在**地面上**能到的最前 / 最后距离（相对静止落点，二分求）。

        落蹄窗口不许再靠猜。腿的可达域由骨长和关节限位共同决定，超出去逆解只能夹到
        边界——表现是蹄从目标点掉下去、支撑相后段滑步，而这两样在静帧上都看不出来。
        步态先量可达域再定窗口，比"跑出来不对再回头调数字"可靠得多。
        """
        rest = self.rest_stance()[f"{'h' if hind else 'f'}{side}"]
        # 探针**故意**去解够不到的目标（二分就是靠"解不出来"收敛的），这些残差不能混进
        # self.residuals——否则自检会把探针的失败当成动画的失败报出来。退出时截断回去。
        mark = len(self.residuals)

        def ok(dz: float) -> bool:
            pose = Pose()
            tgt = rest + np.array([0.0, 0.0, dz])
            return self.solve_leg(pose, side, hind, tgt) <= tol

        out = []
        for sign in (-1.0, 1.0):  # -1 = 向前（−z），+1 = 向后
            a, b = lo, hi
            if ok(sign * b):
                out.append(b)
                continue
            for _ in range(24):
                m = (a + b) / 2
                if ok(sign * m):
                    a = m
                else:
                    b = m
            out.append(a)
        del self.residuals[mark:]
        return out[0], out[1]

    def rest_stance(self) -> dict[str, np.ndarray]:
        if self._rest_stance is None:
            self._rest_stance = {
                f"{'h' if h else 'f'}{s}": self.foot_world(Pose(), s, h) for h in (False, True) for s in ("l", "r")
            }
        return {k: v.copy() for k, v in self._rest_stance.items()}


def contact_report(rig: Rig, sampler, legs, length: float, n: int = 48) -> tuple[str, float, float]:
    """支撑相诊断：蹄是否贴地、是否等速后移（滑步 = 四足动画头号破绽）。

    自变量取**支撑相内的相位 u**而不是全局时间 t：多数腿的支撑区间跨过 t=1 的接缝
    （例如相位 0.25、占空 0.62 的那条腿，支撑相是 [0.75,1)∪[0,0.37)），拿 t 去做
    线性回归会把两段接反，残差凭空变成整个跨距那么大——那不是滑步，是诊断自己算错了。

    legs: key → (hind, side, stance_u)，其中 stance_u(t) 返回支撑相内的 u∈[0,1) 或 None。
    返回 (报告文本, 最大离地绝对值, 最大滑步残差)。
    """
    lines = []
    worst_y = 0.0
    worst_slip = 0.0
    for key, (hind, side, stance_u) in legs.items():
        ys, zs, ts = [], [], []
        for i in range(n):
            t = i / n
            u = stance_u(t)
            if u is None:
                continue
            p = rig.foot_world(sampler(t), side, hind)
            ys.append(p[1])
            zs.append(p[2])
            ts.append(u)
        if len(ts) < 3:
            lines.append(f"  {key}: 支撑相采样不足（{len(ts)} 帧）")
            continue
        A = np.vstack([np.array(ts), np.ones(len(ts))]).T
        slope, icpt = np.linalg.lstsq(A, np.array(zs), rcond=None)[0]
        resid = float(np.abs(np.array(zs) - (slope * np.array(ts) + icpt)).max())
        worst_y = max(worst_y, max(abs(min(ys)), abs(max(ys))))
        worst_slip = max(worst_slip, resid)
        lines.append(
            f"  {key}: 触地 y {min(ys):+.2f}..{max(ys):+.2f}  后移 {slope / length:+.1f} u/s  滑步残差 {resid:.3f}"
        )
    return "\n".join(lines), worst_y, worst_slip


if __name__ == "__main__":
    import sys

    size = sys.argv[1] if len(sys.argv) > 1 else "medium"
    rig = Rig(FINAL / f"HorsePelt_rust_{size}.bbmodel")
    print(f"骨 {len(rig.bones)} · 元素 {len(rig.elements)}")
    for k, v in rig.rest_stance().items():
        print(f"  {k} 触地点 ({v[0]:+6.2f},{v[1]:+6.2f},{v[2]:+7.2f})")
    pts = np.vstack([rig.bone_points(n) for n in rig.order if rig.bones[n].elements])
    print(f"  模型 y 范围 {pts[:, 1].min():.2f} .. {pts[:, 1].max():.2f}")
