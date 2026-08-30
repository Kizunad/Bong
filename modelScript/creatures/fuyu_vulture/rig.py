#!/usr/bin/env python3
"""腐羽鹫 —— 绑定层：鸟类专属的肢链、限位与摆姿原语。

通用的骨树正解 / 二连杆闭式逆解 / 曲线工具 / 关键帧导出在 `bbmodel_maker.rig.animkit`；
这里只放**鸟**的部分：Z 字腿的逆解参数、颈的分段加权、翼的抬掠折、尾的展收。

两足和四足的差别不只是"少两条腿"：
  · 四足有静态稳定的三角支撑，两足**每一步都在单支撑相**，重心必须横移到支撑脚上方
    —— 少了这一下，走路看着像在冰上平移。所以 `shift_over()` 是走/跑的必需件而非润色。
  · 鸟腿是 Z 字（股骨近水平埋在体内，膝前折、踝后折），髋的活动范围很小，摆动主要
    由膝踝供给。逆解的 root share 因此给得比狮子小得多。
  · 鸟走路头是"停-冲"的：支撑段头锁在世界坐标里不动，换步时猛地前送。原地循环里这
    表现为头相对身体缓缓后移再前冲，靠 `head_bob()` 摊到每节颈椎上做。

坐标沿用建模层：16 单位 = 1 格 = 1 m，地面 y=0，鸟头朝 −Z。
"""

from __future__ import annotations

import math
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "core"))

from bbmodel_maker.rig.animkit import (  # noqa: E402
    Pose, PoseRig, align, clamp01, euler, euler_of, slerp, smooth,
)

MODELS = Path(__file__).resolve().parents[2] / "models" / "fuyu_vulture"
LAYERS = MODELS / "layers"

LEG = ("femur_{s}", "tibiotarsus_{s}", "tarsometatarsus_{s}", "toes_{s}")
WING = ("coracoid_{s}", "humerus_{s}", "ulna_{s}", "carpus_{s}", "manus_{s}")
SIDES = ("l", "r")

# 各关节相对静止姿的活动范围（度，绕 X = 矢状面）。限位不是装饰：不夹的话闭式解会在
# 目标逼近可达边界时把胫跗解到反折，渲出来是断腿。
# 鸟的股骨几乎横卧在体内、被体羽包着，真实摆幅比狮子的肩胛小一大截；抬腿高度主要由
# 膝（胫跗）和踝（跗跖）供给，所以这两个给得宽。
# 上限量出来定的，不是拍的：跗跖夹在 62° 时抬腿只到 3.5U 就顶死（残差 1.4U，脚拔不
# 出地面），放到 92° 才够整个摆动相用。三档扫过一遍 dz∈[−7,8]U × dy∈[0,8]U 的可达域，
# 现在中/大档全域残差 < 0.05U，小档只在跨距 ≥7U 时够不着（步幅按髋高定，够不到那儿）。
LIMITS = {
    "femur": (-26.0, 30.0),
    "tibiotarsus": (-72.0, 60.0),
    "tarsometatarsus": (-58.0, 92.0),
}
FEMUR_SHARE = 0.30   # 肢根分担整肢摆动的比例，剩下的交给闭式二连杆
ABDUCT = 16.0        # 髋外展上限（绕 Z），用来把脚收到重心正下方


class VultureRig(PoseRig):
    """在 PoseRig 之上补出鸟的肢链与摆姿原语。

    骨名从**模型里读**而不是写死：三档的颈椎数各不相同（小 14 / 中 16 / 大 19），
    写死 16 的话小档会去摆一根不存在的骨头，大档最上面三节永远不动。
    """

    def __init__(self, path: Path | str):
        super().__init__(path)
        self.neck = [n for n in self.order if n.startswith("neck_")]  # 颈根 → 颅（父先于子）
        self.tail = [n for n in self.order if n.startswith("tail_")] + ["pygostyle"]
        self.limits = {f"{key}_{s}": lim for s in SIDES for key, lim in LIMITS.items()}
        # 尺度基准：跗跖长（踝→趾根）是全身最稳的比例尺，档间差异就靠它归一
        self.U = float(np.linalg.norm(self.bones["toes_l"].origin
                                      - self.bones["tarsometatarsus_l"].origin)) / 5.9
        self._rest: dict[str, np.ndarray] | None = None
        self._tuck: tuple[float, float, float] | None = None
        for n in ("root", "hips", "trunk_front", "skull", "jaw"):
            if n not in self.bones:
                raise SystemExit(f"{self.path.name}: 缺骨骼 {n}，不是腐羽鹫模型")

    # ---------------------------------------------------------------- 肢链

    def leg(self, side: str) -> list[str]:
        return [n.format(s=side) for n in LEG]

    def wing(self, side: str) -> list[str]:
        return [n.format(s=side) for n in WING]

    def rest_stance(self) -> dict[str, np.ndarray]:
        if self._rest is None:
            self._rest = {s: self.tip_world(Pose(), self.leg(s)) for s in SIDES}
        return {k: v.copy() for k, v in self._rest.items()}

    def solve_foot(self, pose: Pose, side: str, target, *, pitch: float = 0.0) -> float:
        return self.solve_limb(pose, self.leg(side), target, limits=self.limits,
                               share=FEMUR_SHARE, tip_pitch=pitch, abduct=ABDUCT)

    def plant(self, pose: Pose, ground: dict[str, np.ndarray],
              pitches: dict[str, float] | None = None) -> None:
        """躯干摆完之后，把仍在地上的脚逆解回它们的落点。"""
        for s, tgt in ground.items():
            self.solve_foot(pose, s, tgt, pitch=(pitches or {}).get(s, 0.0))

    # ---------------------------------------------------------------- 躯干

    def shift_over(self, pose: Pose, x: float, *, roll: float = 0.0) -> None:
        """重心横移 + 骨盆侧倾。两足动物单支撑相的必需件（见模块 docstring）。"""
        pose["root"].pos[0] += x
        pose["hips"].rot[2] += roll

    def breathe(self, pose: Pose, t: float, *, rate: float, depth: float) -> None:
        """呼吸：胸廓抬合。不用 scale —— 缩放会把挂在胸前的翼一起拉长。

        鸟没有横膈膜，靠胸骨（龙骨）前后摆动泵气，起伏方向以矢状面内的俯仰为主。
        """
        a = math.sin(2.0 * math.pi * rate * t)
        pose["trunk_front"].rot[0] += depth * 1.1 * a
        pose["trunk_front"].pos[1] += depth * 0.30 * self.U * a
        pose["hips"].rot[0] += -depth * 0.35 * a

    # ---------------------------------------------------------------- 颈

    def neck_curve(self, pose: Pose, *, pitch: float = 0.0, yaw: float = 0.0,
                   roll: float = 0.0, bias: float = 1.0, add: bool = True) -> None:
        """把**整条颈**的总角度摊到每节颈椎上。

        参数是总度数不是逐节度数：颈有 14~19 节，单节 −8° 听着不多，摊开累加是 −130°，
        脖子直接卷成一个圈。狮子那边在尾巴上栽过同一个跟头（总曲率 −264°）。
        bias > 1 把弯曲压向颈根（低头压身），< 1 压向头端（只是探头）。
        """
        n = len(self.neck)
        w = [((i + 1) / n) ** bias for i in range(n)]
        tot = sum(w) or 1.0
        for i, b in enumerate(self.neck):
            k = w[i] / tot
            ch = pose[b]
            if add:
                ch.rot[0] += pitch * k
                ch.rot[1] += yaw * k
                ch.rot[2] += roll * k
            else:
                ch.rot[0] = pitch * k
                ch.rot[1] = yaw * k
                ch.rot[2] = roll * k

    def head_bob(self, pose: Pose, back: float, lift: float = 0.0) -> None:
        """头相对躯干的前后位移，摊到每节颈椎（back > 0 = 头往后 = 世界里"锁住不动"）。

        不能只平移颈根一节：那等于把整条颈从躯干上拔下来，接缝当场撕开。摊到 n 节各走
        1/n，颈是**弯**过去的，接缝不动。
        """
        n = len(self.neck)
        for b in self.neck:
            pose[b].pos[2] += back / n
            pose[b].pos[1] += lift / n

    # ---------------------------------------------------------------- 翼

    def wing_pose(self, pose: Pose, side: str, *, elev: float = 0.0, sweep: float = 0.0,
                  twist: float = 0.0, flex: float = 0.0, hand: float = 0.0,
                  hand_twist: float = 0.0, shrug: float = 0.0) -> None:
        """一侧翼的摆姿。角度全用"两翼对称为正"的语义，左右符号在这里统一处理。

        elev  抬翼（+ 上）          sweep 前掠（+ 向前）      twist 翼根扭转（+ 前缘上仰）
        flex  前臂折收（+ 缩翼展）  hand  手部续折（+ 再缩）  hand_twist 初级飞羽扭转
        shrug 肩带耸动（+ 上）

        左右符号：绕 Z 抬翼时 −x 侧加正角是往**下**走（Rz 把 x 转向 y），右侧才是往上；
        绕 Y 折翼时两侧又反过来。统一在这里按 sgn 处理 —— 让调用方各自记符号，迟早写出
        一只单翼上扬的鸟。twist 是绕**体轴** x 的，两翼同号才是对称的迎角变化。

        量出来的效应（展翼中档，翼尖半展 42.1）：elev ±30 → 翼尖升降 ±20；sweep +30 →
        前移 19 且半展缩到 33.7；flex +30 → 半展缩到 36.3。
        """
        sgn = -1.0 if side == "l" else 1.0
        cor, hum, uln, car, man = self.wing(side)
        pose[cor].rot[2] += sgn * shrug
        pose[hum].rot[2] += sgn * elev
        pose[hum].rot[1] += sgn * sweep
        pose[hum].rot[0] += twist
        pose[uln].rot[1] += -sgn * flex
        pose[uln].rot[2] += sgn * flex * 0.22   # 折的同时略抬，肘不会塌到翼面下
        pose[car].rot[1] += -sgn * hand
        pose[man].rot[0] += hand_twist

    def wings(self, pose: Pose, **kw) -> None:
        for s in SIDES:
            self.wing_pose(pose, s, **kw)

    # ---------------------------------------------------------------- 尾

    def tail_pose(self, pose: Pose, *, pitch: float = 0.0, yaw: float = 0.0,
                  roll: float = 0.0) -> None:
        """尾：总角度摊到自由尾椎 + 尾综骨，越靠末端权重越大。

        鸟尾只有 5 节自由尾椎 + 尾综骨，能动的幅度远小于猫尾；尾羽的展收在这具模型里
        是一整块（rectrix 全挂在 pygostyle 上），做不出真正的扇形开合。
        """
        n = len(self.tail)
        w = [(i + 1) / n for i in range(n)]
        tot = sum(w)
        for i, b in enumerate(self.tail):
            k = w[i] / tot
            pose[b].rot[0] += pitch * k
            pose[b].rot[1] += yaw * k
            pose[b].rot[2] += roll * k

    # ---------------------------------------------------------------- 收腿

    def tuck_angles(self) -> tuple[float, float, float]:
        """飞行收腿的（股骨, 胫跗, 跗跖）角 —— 按目标位置**搜**出来，不写死。

        三档腿的比例并不一样（大档相对更长、跗跖更长），同一组角度小档能把趾尖收到腹
        下、大档却还吊在体外晃。所以这里给的是"趾尖该落在哪"，角度由搜索给。

        为什么不用逆解：收腿是把肢折到近乎对折，目标点离髋很近，闭式二连杆的可达环
        下界 |lb−lc| 直接把它夹在环上 —— 解得出一个"够不着"的近似，而不是折叠。

        目标：趾尖贴到**腹线**（体羽最低点，从模型量）稍上方、髋后约六成尾长处；踝压在
        腹线与髋之间且**不得翻到髋以上**（翻上去就成了膝盖顶穿后背）。股骨埋在体内，代
        价里带一项 |股骨角| 的惩罚，把折叠尽量交给膝踝 —— 否则搜索会给出一堆"整条腿向后
        甩"的等价解。

        趾尖高度按腹线定而不按髋高的固定比例：三档的腹/髋比是 0.51/0.52/0.54，看着接近，
        但按 0.47 给的话中档趾尖落在腹线下方 0.8 —— 滑翔时两只脚吊在肚子外面晃。
        """
        if self._tuck is not None:
            return self._tuck
        hip = self.bones["femur_l"].origin
        tail_z = self.bones["pygostyle"].origin[2]
        belly = min(float(self.bone_points(b)[:, 1].min()) for b in ("hips", "trunk_front"))
        want_y = belly + 1.5 * self.U
        want_z = hip[2] + 0.60 * (tail_z - hip[2]) + 2.5 * self.U
        want_ank = (want_y + hip[1]) / 2
        chain = self.leg("l")
        base = self.world()[self.bones[chain[0]].parent]
        contact = self.contact_point(chain[3])

        def cost(fe: float, ti: float, ta: float) -> float:
            p = Pose()
            p["femur_l"].rot[0] = fe
            p["tibiotarsus_l"].rot[0] = ti
            p["tarsometatarsus_l"].rot[0] = ta
            M = self.chain_world(chain, base, p)
            knee = M[1][:3, :3] @ self.bones[chain[1]].origin + M[1][:3, 3]
            ank = M[2][:3, :3] @ self.bones[chain[2]].origin + M[2][:3, 3]
            toe = M[3][:3, :3] @ contact + M[3][:3, 3]
            c = abs(toe[1] - want_y) + abs(toe[2] - want_z) + 0.6 * abs(ank[1] - want_ank)
            c += 0.22 * abs(fe)
            c += 40.0 * max(0.0, ank[1] - (hip[1] - 0.5))
            c += 40.0 * max(0.0, knee[1] - (hip[1] - 0.5))
            return c

        best = (1e18, 0.0, 0.0, 0.0)
        for fe in range(-55, 6, 5):
            for ti in range(-90, 6, 5):
                for ta in range(-20, 121, 5):
                    c = cost(fe, ti, ta)
                    if c < best[0]:
                        best = (c, float(fe), float(ti), float(ta))
        _c, fe, ti, ta = best
        for step in (2.0, 0.5):                      # 细化：粗搜的 5° 网格看得见台阶
            improved = True
            while improved:
                improved = False
                for d in (step, -step):
                    for i in range(3):
                        cand = [fe, ti, ta]
                        cand[i] += d
                        c = cost(*cand)
                        if c < _c - 1e-9:
                            _c, (fe, ti, ta) = c, cand
                            improved = True
        self._tuck = (round(fe, 2), round(ti, 2), round(ta, 2))
        return self._tuck

    def tuck_legs(self, pose: Pose, amount: float = 1.0) -> None:
        """把两腿折进飞行姿。amount ∈ [0,1]，0 = 保持当前姿（可与逆解落地姿混合）。"""
        fe, ti, ta = self.tuck_angles()
        for s in SIDES:
            chain = self.leg(s)
            for name, ang in zip(chain[:3], (fe, ti, ta)):
                pose[name].rot[0] = pose[name].rot[0] * (1.0 - amount) + ang * amount
            # 趾也要蜷起来：伸着的趾在飞行里像两把没收的钩子
            pose[chain[3]].rot[0] = pose[chain[3]].rot[0] * (1.0 - amount) + 34.0 * amount
            pose[chain[0]].rot[2] *= (1.0 - amount)

    # ---------------------------------------------------------------- 贴地

    def head_floor(self, pose: Pose, floor: float = 0.0, iters: int = 3) -> float:
        """低头动作的护栏：把头顶回地面以上，返回补偿量。

        低头的深度是"颈弯多少度 + 沿颈平移多少"两项叠出来的，而这两项对喙尖高度的贡献
        随档位**非线性**变化（大档颈长、头也大）。同一组系数中档刚好点到地面，大档就把
        整个下喙插进土里 5.4 单位。与其逐档手调系数，不如把约束写出来让它自己收敛。
        """
        total = 0.0
        for _ in range(iters):
            W = self.world(pose)
            low = 1e9
            for b in ("skull", "jaw"):
                pts = self.bone_points(b)
                if len(pts):
                    low = min(low, float((pts @ W[b][:3, :3].T + W[b][:3, 3])[:, 1].min()))
            if low >= floor - 1e-3:
                break
            self.head_bob(pose, back=0.0, lift=floor - low)
            total += floor - low
        return total

    def ground_clamp(self, pose: Pose, floor: float = 0.0) -> None:
        """把整只抬回地面。倒地/侧翻绕的是 y=0 的轴，身体横跨中线，转过去时背离侧会
        沉到地下 —— 必须逐帧夹，不能拍一个下沉量了事。"""
        low = self.lowest(pose)
        if low < floor:
            pose["root"].pos[1] += floor - low


# ================================================================ 步态


class BipedGait:
    """两足步态：相位 + 支撑相占空比 + 落脚窗口。

    单支撑相占比 = 1 − 2·(1 − duty)。duty 低于 0.5 就是**跑**（有腾空段），高于 0.5
    才是走。这条不是风格选择：duty=0.5 时两脚同时离地零帧，再低就必须给躯干配抛物弧，
    否则脚在腾空段还锁在地上，解出来全是滑步。
    """

    def __init__(self, rig: VultureRig, *, duty: float, fwd: float, back: float,
                 lift: float, swing_ease: float = 1.8, toe_off: float = 26.0,
                 phase: float = 0.5, medial: float = 0.0):
        self.rig = rig
        self.duty = duty
        self.fwd, self.back = fwd, back
        self.lift = lift
        self.swing_ease = swing_ease
        self.toe_off = toe_off
        self.phases = {"l": 0.0, "r": phase}
        # medial：落脚点向中线收拢的比例。站着的时候两脚是分开的，走起来鸟几乎踩在一条
        # 线上 —— 这不是风格，是省力：脚离中线越远，每步要横移的重心就越多。收拢之后同
        # 样的平衡质量只要一半的横摆幅度。
        self.rest = rig.rest_stance()
        for v in self.rest.values():
            v[0] *= (1.0 - medial)

    def stance(self, side: str, t: float) -> bool:
        return (t + self.phases[side]) % 1.0 < self.duty

    def target(self, side: str, t: float) -> tuple[np.ndarray, float]:
        """返回 (着地点世界目标, 脚掌俯仰)。"""
        r = self.rest[side].copy()
        u = (t + self.phases[side]) % 1.0
        z_front, z_back = r[2] - self.fwd, r[2] + self.back
        half = self.rig.sole_half(self.rig.leg(side)[-1])
        if u < self.duty:                       # 支撑相：脚锁地，相对身体匀速后移
            s = u / self.duty
            z = z_front + (z_back - z_front) * s
            pitch = -self.toe_off * smooth(max(0.0, (s - 0.62) / 0.38))
            # 逆解锁的是掌心，蹬离真正的支点却是趾尖：绕掌心翻 26° 会把趾尖压进地里。
            # 抬掌心 half·sin|θ| 等价于绕趾尖翻。
            y = r[1] + half * abs(math.sin(math.radians(pitch)))
        else:                                    # 摆动相：提起、前送、放下
            s = (u - self.duty) / (1.0 - self.duty)
            z = z_back + (z_front - z_back) * (1.0 - (1.0 - clamp01(s)) ** self.swing_ease)
            # 起点必须接上支撑相末尾的蹬离角，落点归零准备平掌着地；接不上的话离地那
            # 一帧脚掌从 −26° 硬跳回 0，看着就是"啪"地一抖。
            pitch = -self.toe_off * (1.0 - s) - 14.0 * math.sin(math.pi * s)
            y = (r[1] + self.lift * math.sin(math.pi * s) ** 0.72
                 + half * abs(math.sin(math.radians(pitch))))
        return np.array([r[0], y, z]), pitch

    def solve(self, pose: Pose, t: float) -> None:
        for s in SIDES:
            tgt, pitch = self.target(s, t)
            self.rig.solve_foot(pose, s, tgt, pitch=pitch)

    def support_x(self, t: float) -> float:
        """当前支撑脚的横向位置（两脚都着地时取中点）—— 重心该压在这上面。"""
        xs = [self.rest[s][0] for s in SIDES if self.stance(s, t)]
        return sum(xs) / len(xs) if xs else 0.0


ARM = ("coracoid_{s}", "humerus_{s}", "ulna_{s}", "carpus_{s}", "manus_{s}")


def unfold_pose(folded: VultureRig, spread: VultureRig) -> Pose:
    """解出把**收翼**绑定姿摆成**展翼**外形的那一个姿态。

    不手调 —— 两份模型都在盘上，差值是可以算出来的，而且算完能逐件对拍验证。三层：

    1. **臂骨**：两个姿态的骨段长度完全相同（实测差 < 0.0005），所以收→展是纯旋转。逐节
       求"把当前世界朝向转到目标朝向"的最小旋转，再换算回该骨的局部角。取最小旋转会留下
       一个自由的绕轴扭转，无所谓 —— 羽的朝向在第 2 步里是照**实际算出来的**父骨世界系
       解的，父骨扭多少都会被吸收掉。
    2. **羽的朝向**：每根羽自带骨、绑定旋转烙着羽轴，所以目标就是"让这根羽骨的世界朝向
       等于展翼模型里的"。Blockbench 把绑定角与动画角**逐分量相加**，于是动画角 =
       目标欧拉 − 绑定欧拉，精确、无多解。
    3. **羽根位置与长度**：两个姿态里羽根沿骨的落点不同（收翼铺 0.12~0.98、展翼铺满整
       根），长度也差着 0.62 —— 分别走 position 与 scale 通道。少了这两条，展开后翼在
       关节处又会露出没羽的带、翼尖也短一截。
    """
    pose = Pose()
    for s in SIDES:
        chain = [n.format(s=s) for n in ARM]
        # ---- 1. 臂骨：逐节最小旋转
        for i, bone in enumerate(chain):
            nxt = chain[i + 1] if i + 1 < len(chain) else None
            R = _fit_rot(folded, spread, bone)
            if R is None:                     # 该骨没有自己的几何：退回按下一节 pivot 定向
                v, tgt = _aim(folded, bone, nxt), _aim(spread, bone, nxt)
                if v is None or tgt is None:
                    continue
                R = align(v, tgt)
            W = folded.world(pose)
            pose[bone].rot = list(euler_of(np.linalg.inv(W[bone][:3, :3]) @ R))

        # ---- 2/3. 每根羽：朝向 / 羽根位置 / 长度
        W = folded.world(pose)
        Ws = spread.world()
        for name, b in folded.bones.items():
            if not name.startswith("q_") or f"_{s}_" not in name or name not in spread.bones:
                continue
            sb = spread.bones[name]
            parent = b.parent
            # 朝向：目标是这根羽骨在展翼模型里的**世界**朝向；动画角 = 目标欧拉 − 绑定欧拉
            want = Ws[parent][:3, :3] @ euler(sb.rest_rot)
            pose[name].rot = list(euler_of(np.linalg.inv(W[parent][:3, :3]) @ want) - b.rest_rot)
            # 羽根：位移通道是在**父骨已旋转**的坐标系里生效的，所以不能拿两份模型的
            # "相对父 pivot 偏移"直接相减 —— 那个差值是在未旋转的模型系里量的，套到转过去
            # 的父骨上会被再转一次（实测末端次级飞羽偏出 10.5 个单位）。正解是把目标世界
            # 位置拉回父骨当前的局部系再减。
            tgt = (Ws[parent] @ np.append(sb.origin, 1.0))[:3]
            pose[name].pos = list((np.linalg.inv(W[parent]) @ np.append(tgt, 1.0))[:3] - b.origin)
            # 三个轴都要缩：羽在两个姿态里不只长度不同，**截面**也是反着装的（收翼
            # 0.21 宽 / 1.26 厚，展翼 2.0 宽 / 0.21 厚）。只缩长度的话展开之后翼又变回
            # 一把梳子 —— 羽根位置对了，羽面还是没有宽度。
            fb, sb_ = _quill_box(folded, name), _quill_box(spread, name)
            pose[name].scale = [sb_[i] / max(fb[i], 1e-6) for i in range(3)]
    return pose


def _fit_rot(folded: VultureRig, spread: VultureRig, bone: str):
    """把这根骨自己的几何从收翼姿最小二乘拟合到展翼姿（Kabsch）。

    比"对齐一个方向"强的地方在**扭转**：单方向对齐只定住骨轴，绕轴转多少是自由的。翼羽
    因为各自单独解朝向看不出来，但骑在骨上的零件会跑偏 —— 实测翼爪落在离正确位置 1.1 个
    单位处（大档）。按整组顶点拟合把这个自由度也定死。
    """
    fe = {folded.elements[u]["name"]: folded.elements[u] for u in folded.bones[bone].elements}
    se = {spread.elements[u]["name"]: spread.elements[u]
          for u in spread.bones.get(bone, folded.bones[bone]).elements} if bone in spread.bones else {}
    names = sorted(set(fe) & set(se))
    if not names:
        return None
    P = np.vstack([folded.corners(fe[n]) for n in names]) - folded.bones[bone].origin
    Q = np.vstack([spread.corners(se[n]) for n in names]) - spread.bones[bone].origin
    U, _S, Vt = np.linalg.svd(P.T @ Q)
    d = np.sign(np.linalg.det(Vt.T @ U.T))
    return Vt.T @ np.diag([1.0, 1.0, d]) @ U.T


def _aim(rig: VultureRig, bone: str, nxt: str | None):
    """这根骨的定向参考：优先指向下一节的 pivot；同点（腕/掌、肩带/肱骨）时退回自己
    几何的形心方向。

    没有这条退路，`carpus→manus` 这种零长骨段会被整段跳过 —— 手部保持收翼朝向不动，
    实测翼爪落在离正确位置 20 个单位的地方（而翼羽因为是单独解的，看不出问题）。
    """
    if nxt is not None:
        v = rig.bones[nxt].origin - rig.bones[bone].origin
        if np.linalg.norm(v) > 1e-6:
            return v
    pts = rig.bone_points(bone)
    if not len(pts):
        return None
    v = pts.mean(axis=0) - rig.bones[bone].origin
    return v if np.linalg.norm(v) > 1e-6 else None


def _quill_box(rig: VultureRig, bone: str) -> np.ndarray:
    """羽骨局部系里这根羽的三轴尺寸（宽 / 长 / 厚）。元素是从 pivot 沿 +Y 伸出去的
    正方盒，所以直接取包围盒即可；多段（羽尖压色）取并集。"""
    lo = np.array([np.inf] * 3)
    hi = np.array([-np.inf] * 3)
    for u in rig.bones[bone].elements:
        e = rig.elements.get(u)
        if e:
            lo = np.minimum(lo, e["from"])
            hi = np.maximum(hi, e["to"])
    return hi - lo


def blend_pose(rig: VultureRig, pose: Pose, w: float) -> Pose:
    """把一个目标姿态按 w∈[0,1] 淡进来。

    旋转走**球面插值**而不是按欧拉角线性缩放：收→展每根羽要转近 90°，线性插欧拉角时三
    个分量各走各的直线，中途的姿态既不在两端"之间"、也不是最短弧 —— 整片翼会散成互不
    相连的板（t≈0.3~0.45 两帧最明显，正是"炸成三层"的样子）。

    位移线性；**缩放先行**（w^0.45）：收翼的羽是画短画窄的（顺体轴叠成一摞，露在外面
    只有一截），展开时它既要转出去、也要"长"到全尺寸。两者同速的话，羽根已经分开一半、
    羽宽才长到一半，中途整片翼是一排互不相连的板。让宽度跑在前面，展开全程都盖得住。
    缩放从 1 起插，不是从 0。
    """
    out = Pose()
    ws = w ** 0.45 if w > 0.0 else 0.0
    for name, ch in pose.items():
        bind = rig.bones[name].rest_rot if name in rig.bones else np.zeros(3)
        if any(ch.rot):
            R = slerp(euler(bind), euler(bind + np.array(ch.rot, float)), w)
            out[name].rot = list(euler_of(R) - bind)
        out[name].pos = [v * w for v in ch.pos]
        out[name].scale = [1.0 + (v - 1.0) * ws for v in ch.scale]
    return out


def default_rig(size: str = "mid", morph: str = "jin", spread: bool = False) -> VultureRig:
    name = {"small": "Small", "mid": "Mid", "large": "Large"}[size]
    if spread:
        return VultureRig(LAYERS / f"FuyuVulturePelt{name}_{morph}_spread.bbmodel")
    return VultureRig(MODELS / f"FuyuVulturePelt{name}_{morph}.bbmodel")


if __name__ == "__main__":
    for key in ("small", "mid", "large"):
        for spread in (False, True):
            rig = default_rig(key, spread=spread)
            tag = "展翼" if spread else "收翼"
            print(f"{key:<6}{tag}  骨 {len(rig.bones):3d} 颈 {len(rig.neck):2d} "
                  f"元素 {len(rig.elements):4d}  U={rig.U:.3f}  最低点 {rig.lowest():+.3f}")
            for s in SIDES:
                p = rig.rest_stance()[s]
                print(f"        {s} 静止落点 ({p[0]:+6.2f},{p[1]:+5.2f},{p[2]:+6.2f})")
            fe, ti, ta = rig.tuck_angles()
            tp = Pose()
            rig.tuck_legs(tp)
            toe = rig.tip_world(tp, rig.leg("l"))
            hip = rig.bones["femur_l"].origin
            print(f"        收腿角 {fe:+.1f}/{ti:+.1f}/{ta:+.1f} → 趾尖 "
                  f"({toe[1]:.1f},{toe[2]:.1f}) 髋高比 {toe[1] / hip[1]:.2f}")
