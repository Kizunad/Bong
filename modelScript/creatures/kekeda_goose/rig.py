#!/usr/bin/env python3
"""珂珂达 —— 绑定层：鹅的骨架怎么动，以及"动得对不对"由什么数字判定。

通用的骨树/正解/逆解/导出在 bbmodel_maker.rig.anim_rig，这里只放**鹅特有**的四件事：

1. **腿是鸟腿**。股骨近水平埋在体腔里，外面看到的那个"反向的膝"其实是跗间关节
   （踝）。所以链是 femur → tibia(胫跗) → tarsus(跗跖) → foot(蹼)，限位按鸟给，
   不能套哺乳动物。

2. **两足要算平衡**。四足动画的头号判据是滑步，两足的是**质心**：单支撑相里质心
   没落在支撑脚上，这只鹅就是靠"观众看不出来"站着的。而鹅之所以摇摆着走，正是
   因为髋距（±1.70）远小于体宽（±4.00）—— 每一步都得把整个身子挪到支撑脚正上方。
   所以摇摆幅度不是手调的，是由"质心要落到脚上"反解出来的（见 Waddle.ROLL）。

3. **颈有 17 节，是一条 S**。弯曲信息全藏在 pivot 折线里（骨骼静止旋转都是 0），
   所以"把脖子伸直"不能靠给每节加同一个角度 —— 那是折得更弯。得先量出每个关节
   的静止折角再逐节抵掉，这就是 anim_rig.Rig.chain_bends 的用处。

4. **有个泄殖腔口**。拉粑粑和下蛋都要往那儿放东西，所以它必须是个真实定位点，
   且要能验证"那一帧它下方是通畅的"，而不是拍脑袋写个偏移。

角度符号一律在这里收口成**语义参数**（抬头为正、张嘴为正、翘尾为正），各动画不直接
写 rot[0]=±x —— 骨的朝向各不相同，正负号靠记必然记错。
"""

from __future__ import annotations

import math
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "core"))

from bbmodel_maker.rig.anim_rig import Pose, Rig, smooth, wrap  # noqa: E402

MODELS = Path(__file__).resolve().parents[2] / "models" / "kekeda_goose"
PLUME = MODELS / "KekedaPlume.bbmodel"

# ---------------------------------------------------------------- 骨名
NECK = tuple(f"neck_{i}" for i in range(17))
NECK_CHAIN = (*NECK, "skull")
TRUNK = ("hips", "trunk_back", "trunk_front")
SIDES = ("l", "r")
LEG = ("femur_{s}", "tibia_{s}", "tarsus_{s}", "foot_{s}")
WING = ("wing_{s}", "forearm_{s}", "hand_{s}")

# 鸟腿限位（度，绕 X = 矢状面，相对静止姿）。股骨在鸟身上几乎不参与摆动——它被
# 腹壁包着近乎水平，真正扫的是胫跗骨；跗跖只在落地缓冲和蹲下时大幅折。
LIMITS = {
    "femur": (-26.0, 26.0),
    "tibia": (-78.0, 46.0),     # 负 = 踝往后送（伸腿）
    "tarsus": (-52.0, 84.0),    # 正 = 跗跖往后折（蹲）
}
# 外展上限比狮子大：鹅的髋距只有体宽的四成，摇摆步里躯干横移量相对腿长很大，给
# ±15° 的话支撑腿在摆幅峰值顶死限位，脚会被拖着横向滑。
ABDUCT = 24.0
FEMUR_SHARE = 0.42   # 股骨分担整肢摆动的比例；鸟腿主要靠胫跗，所以比狮子的 0.5 低

#: 动画层的中立站高比绑定姿低这么多。**这不是调味，是绑定姿的腿建得太直。**
#:
#: 实测绑定姿下 tibia→foot 这对连杆已经伸到全长的 95.4%，一迈步就顶到 100%。而闭式解
#: 里膝角走 acos((d²−lb²−lc²)/2·lb·lc)，d→lb+lc 时导数发散 —— 目标动一丝，关节角就
#: 甩几十度。实测 tarsus 在触地那一帧从 +21.6° 跳到 −34.8°，动画里是落脚"咔"一下。
#: 建模时把腿建直是自然的（好量比例），但绑定姿从不该让肢体满伸：留不出关节余量，
#: 所有动画就都坐在奇异点上。
#: 沉 0.60 后最大伸展降到 91%，站高 15.98 → 15.38（少 3.8%，看不出来），腿反而更像鹅。
#: 参数空间实测：跨距几乎不影响伸展比（鹅的髋长在身体偏后，**触地前伸**才是极值，
#: 所以落脚窗口要往后偏），身高才是唯一有效的旋钮。
STANCE_DROP = 0.60

# 泄殖腔口，挂在 tail_base 上。**写绝对（模型空间）坐标，不是相对 pivot 的偏移**——
# 绑定姿态下每根骨的静止变换都是单位阵（几何全在 element 的绝对 from/to 里），所以
# "骨局部坐标"和模型坐标在静止姿下是同一个数；写成偏移量会让这个点跑到地底下去。
# y 取在臀羽下缘 5.85 稍下方，z 落在体腔最后端：再往前会被 body_rump 盖住（往下掉的
# 东西从身体里冒出来），再往后就悬在尾羽外面了。这不靠注释担保 —— check_anim 会在
# 释放帧实测"它正下方有没有自己的身体挡着"。
VENT = (0.0, 5.72, 4.65)


def leg_chain(side: str) -> list[str]:
    return [n.format(s=side) for n in LEG]


def wing_chain(side: str) -> list[str]:
    return [n.format(s=side) for n in WING]


# ---------------------------------------------------------------- 鹅 Rig
class Goose(Rig):
    def __init__(self, path: Path = PLUME):
        super().__init__(path)
        self.neck_bends = self.chain_bends(NECK_CHAIN)   # 16 个，对齐 neck_1..neck_16
        self._rest_feet: dict[str, np.ndarray] | None = None
        self._rest_com: np.ndarray | None = None

    # -- 静止基准 ----------------------------------------------------------
    def rest_feet(self) -> dict[str, np.ndarray]:
        if self._rest_feet is None:
            self._rest_feet = {s: self.limb_tip(Pose(), leg_chain(s)) for s in SIDES}
        return {k: v.copy() for k, v in self._rest_feet.items()}

    def rest_com(self) -> np.ndarray:
        if self._rest_com is None:
            self._rest_com = self.mass_center(Pose())
        return self._rest_com.copy()

    # -- 语义参数 → 骨旋转 --------------------------------------------------
    def neck_elev(self, pose: Pose | None = None) -> float:
        """颈弦（颈根关节 → 颅底关节）在矢状面里的仰角，度：
        0 = 水平前伸，+90 = 竖直向上，负 = 头低于颈根。静止姿约 +66°。"""
        W = self.world(pose)
        d = self.joint("skull", W) - self.joint("neck_0", W)
        return math.degrees(math.atan2(d[1], -d[2]))

    def neck_reach(self, pose: Pose | None = None) -> float:
        """颈弦长（颈根 → 颅底）。静止 4.39，缩到底 ≈2.4，全伸 ≈7.0。"""
        W = self.world(pose)
        return float(np.linalg.norm(self.joint("skull", W) - self.joint("neck_0", W)))

    def neck(self, pose: Pose, *, straight: float = 0.0, arc: float = 0.0,
             yaw: float = 0.0, aim: float | None = None, rounds: int = 2) -> None:
        """颈：两个正交旋钮 —— straight 管**多长**，aim 管**朝哪**。

        全部是整条的**总度数**，函数内部再分摊到 17 节。别写成逐节度数：单节 8° 听着
        不多，17 节累加是 136°，脖子直接卷到背上去。

        straight  把静止那条 S 拉直的比例（逐节抵掉自身折角，全条共 −146°）。
                  0 = 原样，1 = 拉成一根直棍（颈弦 4.39 → 7.04）。**可以给负数** ——
                  负 = 折得更紧 = 缩颈，−0.25 时颈弦 3.2，头缩进肩里。低于 −0.35
                  喙会插进胸口，别再往下给。
        arc       总度数 正 = 整条往后弓，负 = 往前弓。straight 之外的姿态微调。
        yaw       总度数 正 = 头往 +x 扭；权重偏上段，鸟转头主要靠上颈。
        aim       给了就**反解**颈根旋转，让颈弦落在这个世界仰角上（0 = 水平前伸，
                  +90 = 竖直向上，静止是 +66）。

        为什么 aim 要反解而不是直接写角度：拉直这条 S 会把整条颈甩向后上方（seg[0]
        本来就朝后 41°），straight 一变，头的指向就跟着变 —— 手写颈根角度的话每调一次
        straight 都得重配一次，两个旋钮就锁死在一起了。反解只多一次正解的开销。顺带
        还白拿一个真实存在的鸟类行为：躯干怎么起伏，颈都把头稳在同一个仰角上。

        **安全区（头与躯干的间隙，静止姿是 0.95；负数 = 头已经嵌进身体）**：

            straight ＼ aim   20     35     50     66     80     95
                −0.30       −2.27  −1.75  −1.64  −1.01  −0.73  −0.18
                −0.12       −1.32  −0.89  −0.54  −0.06  +0.69  +0.76
                 0.00       −0.68  −0.24  +0.33  +0.96  +1.27  +1.39
                +0.15       +0.10  +0.65  +1.39  +1.58  +1.99  +2.16
                +0.35       +1.06  +1.85  +1.56  +2.36  +2.89  +3.13

        一条通则：**aim 压得越低，颈就必须越长**（大致 straight ≳ (66 − aim)/200）。
        短着脖子想往前下方看，头必然埋进胸口 —— 真鸟也正是先伸颈再低头的。首轮六段
        动画栽在这里：物理指标一项不差，渲出来中段"没有脖子"，整只读成一团白。
        check_anim 的剪影项会实测这个间隙，别只信这张表。
        """
        n = len(NECK)
        wsum = sum(((j + 1) / n) ** 2 for j in range(n))
        for i, b in enumerate(NECK):
            if i >= 1:
                pose[b].rot[0] += -straight * self.neck_bends[i - 1]
            pose[b].rot[0] += arc / n
            pose[b].rot[1] += yaw * ((i + 1) / n) ** 2 / wsum
        if aim is not None:
            for _ in range(rounds):
                pose[NECK[0]].rot[0] += aim - self.neck_elev(pose)

    def head(self, pose: Pose, *, pitch: float = 0.0, yaw: float = 0.0, roll: float = 0.0) -> None:
        """头：pitch 正 = 抬头（喙上翘）。"""
        pose["skull"].rot[0] += pitch
        pose["skull"].rot[1] += yaw
        pose["skull"].rot[2] += roll

    def bill(self, pose: Pose, opening: float) -> None:
        """张嘴：正 = 张开的度数（下喙下沉）。"""
        pose["jaw"].rot[0] += -opening

    def tail(self, pose: Pose, *, lift: float = 0.0, yaw: float = 0.0) -> None:
        """尾：lift 正 = 上翘。拉粑粑/下蛋全靠它把出口让开。"""
        pose["tail_base"].rot[0] += -lift
        pose["tail_base"].rot[1] += yaw

    def wings(self, pose: Pose, *, spread: float = 0.0, lift: float = 0.0,
              fold: float = 0.0, asym: float = 0.0) -> None:
        """翼：spread 正 = 向外张开（绕 Z），lift 正 = 前缘上抬（绕 X），
        fold 正 = 腕关节收拢。asym 给左右加一点反相差，免得双翼像一块板。"""
        for sx, s in ((-1, "l"), (1, "r")):
            pose[f"wing_{s}"].rot[2] += sx * (spread + sx * asym)
            pose[f"wing_{s}"].rot[0] += lift
            pose[f"forearm_{s}"].rot[2] += sx * (spread * 0.55 - fold)
            pose[f"hand_{s}"].rot[2] += sx * (spread * 0.30 - fold * 1.4)

    def breathe(self, pose: Pose, t: float, *, rate: float, depth: float) -> None:
        """呼吸：胸廓抬合 + 腰背起伏。不用 scale —— 缩放会把挂在躯干下的腿一起拉长。"""
        a = math.sin(2.0 * math.pi * rate * t)
        b = math.sin(2.0 * math.pi * rate * t - 0.6)
        pose["trunk_front"].rot[0] += -depth * 0.9 * a
        pose["trunk_back"].rot[0] += depth * 0.6 * b
        pose["trunk_front"].pos[1] += depth * 0.30 * a
        pose["hips"].pos[1] += depth * 0.16 * b

    # -- 测量 --------------------------------------------------------------
    def bill_tip(self, pose: Pose | None = None) -> np.ndarray:
        """喙尖世界坐标。判"抬没抬头/伸没伸颈"用它，不看角度 —— 角度会互相抵消。"""
        return self.point("bill_upper", (0.0, 13.75, -8.72), pose)

    def vent(self, pose: Pose | None = None) -> np.ndarray:
        return self.point("tail_base", VENT, pose)

    def plant(self, pose: Pose, feet: dict[str, np.ndarray],
              pitches: dict[str, float] | None = None, *, level: bool = True) -> dict[str, float]:
        """躯干摆完之后，把脚逆解回它们的落点。返回逐脚残差。

        level：解完再把扎进地里的脚整体抬回地面。为什么要这一步 —— 逆解锁的是**掌心
        一个点**，而蹼板是块 3.3×3.6 的大平板。掌板的俯仰可以事先补（`Waddle.target`
        里那个 half·|sin θ|），但**侧倾补不了**：它是解算过程中由躯干姿态反算出来的，
        事前不知道。3° 的侧倾就让外侧角低 0.07 —— 实测小跑最低点 −0.069，正是这个。
        误差是线性的，量一次补一次就够。
        """
        err = {s: self.solve_limb(pose, leg_chain(s), tgt, limits=LIMITS, abduct=ABDUCT,
                                  share=FEMUR_SHARE, tip_pitch=(pitches or {}).get(s, 0.0))
               for s, tgt in feet.items()}
        if not level:
            return err
        W = self.world(pose)
        fixed = {}
        for s, tgt in feet.items():
            n = f"foot_{s}"
            dip = float((self.bone_points(n) @ W[n][:3, :3].T + W[n][:3, 3])[:, 1].min())
            if dip < -1e-4:
                fixed[s] = tgt + np.array([0.0, -dip, 0.0])
        return self.plant(pose, {**feet, **fixed}, pitches, level=False) if fixed else err

    def support_z(self, pose: Pose | None = None) -> tuple[float, float]:
        """两只蹼板着地部分合起来的前后范围 (z_min, z_max)。

        这就是支撑多边形在矢状方向上的边界：质心的 z 一旦跑出去，这只鹅就该往前扑
        或者向后坐了。静止姿约 −3.3..+1.4，而质心在 −0.35 —— 往后只有 1.7 的余量，
        所以翘尾巴这类把质量往后挪的动作必须同时前倾，否则就是屁股墩。
        """
        W = self.world(pose)
        zs: list[float] = []
        for s in SIDES:
            n = f"foot_{s}"
            pts = self.bone_points(n) @ W[n][:3, :3].T + W[n][:3, 3]
            sole = pts[pts[:, 1] <= pts[:, 1].min() + 0.75]
            zs += [float(sole[:, 2].min()), float(sole[:, 2].max())]
        return min(zs), max(zs)

    def balance(self, pose: Pose, *, feet: dict[str, np.ndarray], x: float | None = None,
                z: float | None = None, pitches: dict[str, float] | None = None,
                rounds: int = 2) -> tuple[float, float]:
        """把质心挪到指定的 (x, z)：调 root.pos，每轮之后重新落脚。返回最终残差。

        为什么要迭代：root 一平移，质心并不是 1:1 跟着走 —— 脚被逆解锁在世界坐标上，
        那部分质量原地不动。两轮就收敛到 0.01 以内。
        """
        for _ in range(rounds):
            self.plant(pose, feet, pitches)
            com = self.mass_center(pose)
            if x is not None:
                pose["root"].pos[0] += x - float(com[0])
            if z is not None:
                pose["root"].pos[2] += z - float(com[2])
        self.plant(pose, feet, pitches)
        com = self.mass_center(pose)
        return (float(com[0]) - (x if x is not None else float(com[0])),
                float(com[2]) - (z if z is not None else float(com[2])))

    def settle(self, pose: Pose, feet: dict[str, np.ndarray] | None = None, *,
               lean: float = 0.0, pitches: dict[str, float] | None = None) -> tuple[float, float]:
        """站定：把质心稳在**静止姿那个位置**上（lean 负 = 额外前倾几个单位）。

        原地动作（鸣叫、威吓、拉粑粑、下蛋）一律走这个。目标取静止姿的质心而不是
        支撑区正中：模型的站姿是建模层定的，动画层没资格顺手把它整体推前半个单位。

        真正被补偿的大头是**颈**：颈伸直往前 3 个单位，头颈这一坨的力矩足够让质心
        前移 0.5 以上（尾巴只有两小块，翘到 50° 也才动 0.01，别指望它）。手写躯干
        前倾角度的话，每改一次颈的姿态那个角度就过时了。
        """
        feet = self.rest_feet() if feet is None else feet
        pose["root"].pos[1] -= STANCE_DROP        # 见 STANCE_DROP：给关节留余量
        return self.balance(pose, feet=feet, x=0.0, z=float(self.rest_com()[2]) + lean,
                            pitches=pitches)

    def reach(self, pose: Pose | None = None) -> float:
        """两连杆（膝→踝）此刻伸展到全长的百分之多少。超过 ~0.96 就进 acos 奇异区，
        逆解会对目标的微小变化爆炸式响应。诊断动画抽搐第一个看它。"""
        W = self.world(pose)
        o = [self.bones[n].origin for n in leg_chain("r")]
        full = abs(complex(*(o[2] - o[1])[1:])) + abs(complex(*(o[3] - o[2])[1:]))
        return max(float(np.linalg.norm(self.joint(f"foot_{s}", W) - self.joint(f"tibia_{s}", W)))
                   for s in SIDES) / full


# ---------------------------------------------------------------- 摇摆步
class Waddle:
    """两足摇摆步：相位表 + 支撑占空比 + 落脚窗口 + 由平衡反解出的侧倾。

    鹅走路摇摆不是风格化，是几何逼出来的：脚间距 ±1.88 而质心高 8 上下，想把质心
    压到支撑脚上，整个身子就得侧倾十几度。所以这里**不手调摇摆幅度** —— 给定
    「质心要压过去多少」，侧倾角由 asin 反解，剩下的零头才用横移补。
    """

    #: 静态平衡只做到这个比例。走路是动态稳定的，质心本来就允许朝下一只脚"落"过去；
    #: 拉满到 1.0 时侧倾 13° 以上，摆幅峰值把支撑腿的外展顶到限位，看着像喝多了。
    BALANCE = 0.70

    def __init__(self, goose: Goose, *, duty: float, fwd: float, back: float,
                 lift: float, swing_ease: float = 1.8, toeoff: float = 14.0,
                 hover_ramp: float = 0.30, balance: float | None = None,
                 phases: dict[str, float] | None = None):
        self.g = goose
        self.BALANCE = self.BALANCE if balance is None else balance
        self.duty = duty
        self.fwd, self.back = fwd, back
        self.lift = lift
        self.swing_ease = swing_ease
        self.toeoff = toeoff
        self.hover_ramp = hover_ramp
        self.phases = phases or {"l": 0.0, "r": 0.5}
        self.rest = goose.rest_feet()
        # 双支撑重叠段（两脚同时着地的那一段）。duty≤0.5 就没有重叠，纯跑。
        self.overlap = max(1e-3, duty - 0.5)
        com_h = float(goose.rest_com()[1])
        foot_x = float(abs(self.rest["r"][0]))
        self.roll = math.degrees(math.asin(min(1.0, self.BALANCE * foot_x / com_h)))
        self.com_h, self.foot_x = com_h, foot_x
        self._fit_lean()

    def _fit_lean(self, n: int = 256) -> None:
        """把压力中心的横向信号拟合成一条正弦 —— 相位和幅度都从步态相位表里**算**出来。

        为什么不直接追压力中心：CoP 在双支撑段里几乎是瞬移（这里实测 0.1 个周期内从
        +1.88 跳到 −1.60）。真身体做不到，追它的话侧倾在 0.09 秒里扫 17°，看着像抽搐。
        质心是 CoP 的二次积分，只跟得上基频；所以取 CoP 的一次谐波，其余谐波丢掉。
        相位由此自动落在单支撑相正中，不用手数"峰值该排在第几帧"。
        """
        ts = [i / n for i in range(n)]
        sig = []
        for t in ts:
            num = den = 0.0
            for s in SIDES:
                w = self.load(s, t)
                num += w * float(self.rest[s][0])
                den += w
            sig.append(num / den if den > 1e-6 else 0.0)
        a = 2.0 * sum(v * math.cos(2 * math.pi * t) for v, t in zip(sig, ts)) / n
        b = 2.0 * sum(v * math.sin(2 * math.pi * t) for v, t in zip(sig, ts)) / n
        amp = math.hypot(a, b)
        # 归一化成 ±1 的纯正弦，幅度另由 BALANCE·foot_x 给 —— 否则方波的基频会超出 4/π 倍
        self._lean_a = a / amp if amp > 1e-9 else 0.0
        self._lean_b = b / amp if amp > 1e-9 else 1.0

    def lean(self, t: float) -> float:
        """质心该往哪偏，−1..+1（+1 = 完全压在右脚上）。"""
        return self._lean_a * math.cos(2 * math.pi * t) + self._lean_b * math.sin(2 * math.pi * t)

    def stance(self, side: str, t: float) -> bool:
        return wrap(t + self.phases[side]) < self.duty

    def load(self, side: str, t: float) -> float:
        """该脚此刻承重多少（0..1）。双支撑段里两脚平滑交接，压力中心才不会瞬移。"""
        u = wrap(t + self.phases[side])
        if u >= self.duty:
            return 0.0
        ov = self.overlap
        if u < ov:
            return smooth(u / ov)
        if u > self.duty - ov:
            return smooth((self.duty - u) / ov)
        return 1.0

    @staticmethod
    def hover(s: float, ramp: float = 0.30) -> float:
        """摆动相的抬脚高度曲线：中段平顶、**两端斜率为 0**。

        别用 sin(πs)^0.8 那类指数小于 1 的形状：s→0 时 sin(πs)^0.8 ≈ (πs)^0.8，导数
        ∝ s^−0.2 是**发散**的。实测离地和触地各有一帧脚瞬移 0.6 个单位，逆解跟着一帧
        折 56°（tarsus 21.6°→−34.8°），动画里就是抬脚"啪"一下、落脚"咚"一下。
        中段要平：脚在空中多悬一会儿才像走路，不然是原地蹬。
        """
        return smooth(min(1.0, s / ramp)) * smooth(min(1.0, (1.0 - s) / ramp))

    def target(self, side: str, t: float) -> tuple[np.ndarray, float]:
        """(脚掌世界目标, 脚掌俯仰)。"""
        r = self.rest[side].copy()
        u = wrap(t + self.phases[side])
        z_front, z_back = r[2] - self.fwd, r[2] + self.back
        foot = leg_chain(side)[-1]
        if u < self.duty:                       # 支撑相：脚锁地，相对身体匀速后移
            s = u / self.duty
            z = z_front + (z_back - z_front) * s
            pitch = -self.toeoff * smooth(max(0.0, (s - 0.66) / 0.34))
            # 逆解锁的是掌心，蹬离真正的支点是掌尖：绕掌心翻角度会把掌尖压进地里。
            # 抬多少由 sole_lift 逐角点算 —— 蹼板三级递宽，形心不在几何中点上。
            y = r[1] + self.g.sole_lift(foot, pitch)
        else:                                   # 摆动相：抬起、前送、落下
            s = (u - self.duty) / (1.0 - self.duty)
            z = z_back + (z_front - z_back) * (1.0 - (1.0 - s) ** self.swing_ease)
            # 起点接上支撑相末尾的蹬离角，落点归零准备平掌着地；蹼很大，落地前必须放平，
            # 否则掌尖先着地会读成"踮脚"。
            pitch = -self.toeoff * (1.0 - s) - 10.0 * math.sin(math.pi * s)
            y = r[1] + self.lift * self.hover(s, self.hover_ramp) + self.g.sole_lift(foot, pitch)
        return np.array([r[0], y, z]), pitch

    def apply(self, pose: Pose, t: float, *, roll_scale: float = 1.0) -> float:
        """摆完躯干后调用：侧倾 + 平衡横移 + 落脚。返回质心残差。"""
        lean = self.lean(t)
        pose["root"].pos[1] -= STANCE_DROP        # 见 STANCE_DROP：给关节留余量
        # 侧倾绕 root（地面高度），身子往支撑脚那侧压：Rz 的 x' = x·cosφ − y·sinφ，
        # 质心在 y≈8 处，所以 φ 取负才把它送到 +x。
        pose["root"].rot[2] += -self.roll * lean * roll_scale
        pose["hips"].rot[2] += 0.35 * self.roll * lean * roll_scale   # 骨盆回一点，别整只像块板
        feet, pitches = {}, {}
        for s in SIDES:
            tgt, pitch = self.target(s, t)
            feet[s], pitches[s] = tgt, pitch
        return self.g.balance(pose, feet=feet, pitches=pitches,
                              x=self.BALANCE * self.foot_x * lean)[0]


if __name__ == "__main__":
    g = Goose()
    com = g.rest_com()
    print(f"骨 {len(g.bones)} · element {len(g.elements)}")
    print(f"静止质心 ({com[0]:+.2f}, {com[1]:+.2f}, {com[2]:+.2f}) · 最低点 {g.lowest():+.3f}")
    for s in SIDES:
        p = g.rest_feet()[s]
        print(f"  {s} 掌心 ({p[0]:+.2f}, {p[1]:+.2f}, {p[2]:+.2f}) · 掌半长 {g.sole_half(f'foot_{s}'):.2f}")
    print(f"喙尖 {np.round(g.bill_tip(), 2)} · 泄殖腔口 {np.round(g.vent(), 2)}")
    print(f"颈静止总折角 {sum(g.neck_bends):+.1f}°（17 节，逐节 "
          f"{min(g.neck_bends):+.1f}..{max(g.neck_bends):+.1f}）")
    w = Waddle(g, duty=0.62, fwd=2.6, back=3.4, lift=1.9)
    print(f"摇摆步：质心高 {w.com_h:.2f} · 脚距 ±{w.foot_x:.2f} → 侧倾 ±{w.roll:.1f}°")
