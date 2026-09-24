#!/usr/bin/env python3
"""腐羽鹫（fuyu_vulture）骨架生成器 —— Round 1/3。

真鸟骨架，不是把四足兽压扁：按鸟纲（Accipitriformes 兀鹫类）解剖建骨——
  颅：气腔化脑颅 + 巨眼眶（内含**巩膜环**）+ 上喙下钩 + 方骨铰链 + 无牙下颌
  颈：14~18 节，缩颈站姿下折成 **S**（乙状）——鸟颈的辨识度全在这条曲线上
  躯干：胸椎愈合成 **notarium**，腰荐愈合成 **synsacrum**（与髂骨长成一整块）
  尾：尾椎 5~6 节 + **pygostyle 尾综骨**（犁形，尾羽根部）
  胸廓：6~8 对肋，每根带 **钩状突（uncinate process）** 斜搭后一根 —— 鸟类特有
  胸骨：巨大 **龙骨突（carina）**，飞行肌的全部附着面
  肩带：**乌喙骨（coracoid）** 撑住肩关节 + **叉骨（furcula）** V 形横在胸前 + 细长肩胛
  翼：肱骨 → 桡+尺 → 腕骨 → **腕掌骨（carpometacarpus，愈合夹窗）** → 小翼指/主指/第三指
  腿：股骨（藏在体内，膝不外露）→ **胫跗骨** + 退化腓骨 → **跗跖骨**（离地竖立）→ 3 前 1 后趾

姿态取**趾行 + 收翼站立**：跗跖竖直、趾着地、翼沿体侧 Z 字折叠。把鸟做成"膝盖反着长"
是最常见的错——外露那个向后的关节是**踝（跗间关节）**，真膝在体羽里。

物种特征（末法残土的异变，三档递增）：颈椎棘突穿出裸颈成一列**骨钉**；小翼指长出
**骨爪**（攀尸山用）；大型档尾综骨末端延成**骨刃**、额顶起**骨嵴**。

三档不是等比缩放，走异速生长（allometry）：小档眼眶/龙骨/跗跖相对更大，大档喙更
厚重、骨更粗、相对翼展更小（绝对仍最大）。

骨骼层级供 GeckoLib 驱动；element 一律写**绝对坐标**（绑定姿态下与骨骼 pivot 自洽），
因为 render_bbmodel.py 只读 elements 不读 outliner。

用法:
  python3 modelScript/creatures/fuyu_vulture/gen_skeleton.py                    # 三档全出
  python3 modelScript/creatures/fuyu_vulture/gen_skeleton.py --size mid         # 单档
  python3 modelScript/creatures/fuyu_vulture/gen_skeleton.py --size mid --pose spread
  python3 modelScript/creatures/fuyu_vulture/gen_skeleton.py --size mid --part skull
  python3 modelScript/creatures/fuyu_vulture/gen_skeleton.py --check            # 只跑自检
  python3 modelScript/creatures/fuyu_vulture/gen_skeleton.py --list
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from dataclasses import dataclass
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

# --- modelScript 路径引导：共用底座在 core/ ---
import sys as _sys
from pathlib import Path as _Path
_sys.path.insert(0, str(_Path(__file__).resolve().parents[2] / "core"))
from bbmodel_maker.rig.rigkit import Rig, Vec, lerp, normalize, perp_to  # noqa: E402

REPO = HERE.parents[2]
# 骨架 / 肌肉 / 各种预览都是**中间产物**，一律落 layers/；顶层只放最终的 9 个外观。
OUT_DIR = Path(__file__).resolve().parents[2] / "models" / "fuyu_vulture" / "layers"

# ---------------------------------------------------------------- 材质
# socket = 深色"孔洞"色。体素没有布尔减法，正交投影也没有凹陷阴影——眼窝/鼻孔/腕掌窗
# 这类洞只能靠一块凹进去的深色面来读，否则侧视永远是一坨实心骨板。
MATS = {
    "bone": (214, 205, 184),
    "bone_dark": (176, 165, 142),
    "bone_air": (226, 219, 202),  # 气骨（鸟骨中空，骨面更薄更亮）
    "cartilage": (198, 200, 192),
    "keratin": (58, 46, 34),  # 喙鞘（角质），乌黑
    "keratin_pale": (126, 104, 76),  # 喙基蜡膜 / 爪根
    "claw": (38, 30, 24),
    "socket": (70, 62, 52),
}


# ================================================================ 档位规格
@dataclass(frozen=True)
class Spec:
    """一档腐羽鹫。尺寸给目标值，比例性状给相对量——check() 会量实际值对拍。"""

    key: str
    cn: str
    model: str
    total_len: float  # 目标：喙尖 → 尾综骨末（z 跨度，单位）
    stand_h: float  # 目标：头顶高
    cervicals: int  # 颈椎节数
    ribs: int  # 肋对数
    tail_verts: int  # 自由尾椎节数
    skull_len_r: float  # 颅长 / total_len
    beak_len_r: float  # 喙长 / total_len
    beak_depth: float  # 喙根纵深（×U）
    hook: float  # 喙尖下钩量（×U）
    orbit_r: float  # 眼眶半径 / 颅长（按颅长取，绝对值会让小档眼眶比脑袋还长）
    crest: float  # 额嵴高（×U，0 = 无）
    neck_spike: float  # 颈椎棘突外露骨钉长（×U）
    keel_depth: float  # 龙骨突下探（×U）
    wing_half_r: float  # 翼骨半跨 / total_len
    alula_claw: float  # 小翼指骨爪长（×U，0 = 无）
    pygo_blade: float  # 尾综骨骨刃长（×U，0 = 无）
    tarsus_r: float  # 跗跖长 / 髋高
    bone_r: float  # 骨粗细系数


SPECS: dict[str, Spec] = {
    # 小：地面型腐食者，专啄骨缝抠髓。眼眶与跗跖相对最大（找食、奔走），喙细尖微钩。
    "small": Spec(
        key="small",
        cn="啄髓鹫",
        model="FuyuVultureSkeletonSmall",
        total_len=22.0,
        stand_h=19.0,
        cervicals=14,
        ribs=6,
        tail_verts=5,
        skull_len_r=0.150,
        beak_len_r=0.125,
        beak_depth=1.85,
        hook=1.05,
        orbit_r=0.300,
        crest=0.0,
        neck_spike=0.30,
        keel_depth=4.30,
        wing_half_r=0.86,
        alula_claw=0.0,
        pygo_blade=0.0,
        tarsus_r=0.40,
        bone_r=0.95,
    ),
    # 中：族群主力，尸场上的常见形。钩喙成形、小翼指长出骨爪、颈钉半露。
    "mid": Spec(
        key="mid",
        cn="锈翎鹫",
        model="FuyuVultureSkeletonMid",
        total_len=38.0,
        stand_h=32.0,
        cervicals=16,
        ribs=7,
        tail_verts=5,
        skull_len_r=0.135,
        beak_len_r=0.135,
        beak_depth=3.00,
        hook=2.20,
        orbit_r=0.275,
        crest=0.75,
        neck_spike=0.80,
        keel_depth=4.00,
        wing_half_r=0.78,
        alula_claw=0.90,
        pygo_blade=0.0,
        tarsus_r=0.34,
        bone_r=1.00,
    ),
    # 大：老而巨的覆尸鹫。喙厚到能碎骨，额起骨嵴、颈钉全露、尾综骨延成骨刃。
    "large": Spec(
        key="large",
        cn="覆尸鹫",
        model="FuyuVultureSkeletonLarge",
        total_len=67.0,
        stand_h=54.0,
        cervicals=18,
        ribs=8,
        tail_verts=6,
        skull_len_r=0.145,
        beak_len_r=0.150,
        beak_depth=5.20,
        hook=2.80,
        orbit_r=0.245,
        crest=1.90,
        neck_spike=1.80,
        keel_depth=3.60,
        wing_half_r=0.70,
        alula_claw=2.20,
        pygo_blade=3.00,
        tarsus_r=0.28,
        bone_r=1.15,
    ),
}


def bezier(p0: Vec, p1: Vec, p2: Vec, p3: Vec, t: float) -> Vec:
    """三次贝塞尔。颈的 S 用它而不是切线积分——积分只能给切线，端点会漂；
    颈必须**同时**钉死两端（下接 T1、上接枕髁）且中段自由弯，控制点是唯一顺手的写法。"""
    u = 1 - t
    w = (u * u * u, 3 * u * u * t, 3 * u * t * t, t * t * t)
    return tuple(sum(w[i] * p[a] for i, p in enumerate((p0, p1, p2, p3))) for a in range(3))


def _mix(a: Vec, b: Vec, t: float) -> Vec:
    return (lerp(a[0], b[0], t), lerp(a[1], b[1], t), lerp(a[2], b[2], t))


def _add(p: Vec, dx: float = 0.0, dy: float = 0.0, dz: float = 0.0) -> Vec:
    return (p[0] + dx, p[1] + dy, p[2] + dz)


def _step(p: Vec, d: Vec, length: float) -> Vec:
    n = math.sqrt(sum(c * c for c in d))
    return (p[0] + d[0] / n * length, p[1] + d[1] / n * length, p[2] + d[2] / n * length)


# ================================================================ 解剖布点
class Anatomy:
    """把一档 Spec 解成全部关节点。部件函数只从这里取点，不各算各的——

    首版让翼和肩带各自推算肩关节位置，差了 0.6 单位，渲出来肱骨头浮在乌喙骨外面。
    """

    def __init__(self, spec: Spec, pose: str = "folded") -> None:
        self.spec = spec
        self.pose = pose
        L, H = spec.total_len, spec.stand_h
        self.U = U = L / 38.0  # 尺度基准：中档 = 1.0

        # ---- 纵向分段：比例之和恰为 1，全长自然落在 total_len 上 ----
        # Round 1 各段独立给系数，加起来只有 0.82 —— 生成物比目标短 18%，
        # 而且躯干只占 39%（真鸟 ~42%），看着头重脚轻。改成累加分配。
        # 头（喙 + 颅）按档位单独给，剩下的长度由躯干各段**按权重瓜分** —— 写死七个
        # 系数就得手工凑够 1.0，改任何一档的喙长都会让全长悄悄偏掉。
        head_r = spec.beak_len_r + spec.skull_len_r
        if not 0.15 < head_r < 0.55:
            raise ValueError(f"[{spec.key}] 头部占比 {head_r:.3f} 离谱（喙+颅 / 全长）")
        # 尾**骨**只占体长一成出头 —— 鸟看着尾巴长，长的是尾羽，骨头短得很。
        # Round 2 给了 21.5%，渲出来一排水平方板拖在身后，比躯干还抢戏。
        weights = (
            0.075,  # 颈的**水平**跨度（缩颈站姿下颈近乎竖直，跨度很小）
            0.285,  # notarium（愈合胸椎）
            0.225,  # synsacrum（综荐骨）
            0.085,  # 自由尾椎
            0.040,  # 尾综骨（真骨很短，长的是尾羽）
        )
        k = (1.0 - head_r) / sum(weights)
        seg = (spec.beak_len_r, spec.skull_len_r, *(w * k for w in weights))
        z = -0.50 * L
        self.beak_tip_z = z
        z += seg[0] * L
        self.brow_z = z  # 喙根 = 额前缘
        z += seg[1] * L
        self.occiput_z = z
        z += seg[2] * L
        self.t1_z = z  # 第一胸椎 = 颈胸交界
        z += seg[3] * L
        self.notarium_back_z = z
        z += seg[4] * L
        self.synsacrum_back_z = z  # 综荐骨后缘 = 尾根
        z += seg[5] * L
        self.tail_end_z = z  # 自由尾椎末
        z += seg[6] * L
        self.pygo_end_z = z  # 尾综骨末（= 全长后端）

        # ---- 躯干中轴（脊柱高度恒定，鸟的背在站立时近水平）----
        self.trunk_y = ty = 0.5625 * H

        # ---- 胸廓 ----
        # 鸟胸深大于宽，但 Round 1 给成 4.6 : 9.6 的刀片，肋弓只剩一排竖百叶。
        # 兀鹫实测宽/深 ≈ 0.75。
        self.thorax_half_w = 3.20 * U
        self.thorax_depth = 0.260 * H
        self.sternum_y = ty - self.thorax_depth
        self.sternum_front_z = self.t1_z + 1.6 * U
        self.sternum_back_z = self.notarium_back_z + 2.2 * U
        self.keel_y = self.sternum_y - spec.keel_depth * U

        # ---- 骨盆 / 腿 ----
        self.hip = lambda sx: (sx * 2.55 * U, ty - 1.0 * U, 0.145 * L)
        hip_y = ty - 1.0 * U
        self.hip_y = hip_y
        # 趾根不落在 y=0：趾骨自己有厚度，趾根压到地面等于把整根趾埋进地里。
        # 腿的垂直预算要先扣掉这一截，剩下的才由股骨 + 胫跗 + 跗跖分。
        self.toe_lift = 0.62 * U
        tarsus_len = spec.tarsus_r * hip_y
        rest = hip_y - tarsus_len - self.toe_lift  # 股骨 + 胫跗 的垂直分量之和
        self.femur_drop = rest * 0.38
        self.tibio_drop = rest * 0.62
        self.tarsus_len = tarsus_len

        # ---- 颈：S 曲线（贝塞尔控制点在矢状面内）----
        # 缩颈站姿：从胸腔近乎竖直地升起，中段微向后弓，再向前转出头颈。
        # Round 1 把两个控制点抬到 +0.16H / +0.33H，拱出一个天鹅似的高背驼峰。
        self.occiput = (0.0, 0.860 * H, self.occiput_z)  # 枕髁
        # 下段后弓要给足：0.055L 的后移在 16 节里摊薄后看不出弯，整条颈读成一段 C 弧。
        # S 之所以是 S，靠的是下段**向后**顶出去的那一下。
        self.neck_ctrl = (
            (0.0, ty, self.t1_z),  # P0 = T1
            (0.0, ty + 0.115 * H, self.t1_z + 0.105 * L),  # P1 后弓（S 的下半）
            (0.0, ty + 0.255 * H, self.t1_z + 0.010 * L),  # P2 前转（S 的上半）
            self.occiput,  # P3 = 枕髁
        )

        # ---- 颅 / 喙 ----
        self.skull_len = spec.skull_len_r * L
        self.beak_len = spec.beak_len_r * L
        self.skull_y = self.occiput[1] + 0.4 * U
        self.beak_root = (0.0, self.skull_y - 0.5 * U, self.brow_z)
        # 喙尖：向前 + 略降，钩尖再下探 hook
        self.beak_tip = (0.0, self.beak_root[1] - 1.1 * U, self.beak_tip_z)
        self.jaw_hinge = (0.0, self.skull_y - 2.4 * U, self.occiput_z + 0.4 * U)

        # ---- 肩带 / 翼 ----
        # 关节盂在脊柱**略下方**外侧（肩胛贴着肋骨背侧走，乌喙骨从这儿斜下去撑住胸骨）。
        # 早先放在 ty+1.9U —— 比背线还高，于是龙骨到肩凭空拉开 10 单位，胸大肌只能
        # 拉成两片垂到腹下的长板，翼也折到背脊上头去了。
        self.shoulder = lambda sx: (sx * 2.35 * U, ty - 0.35 * U, self.t1_z + 0.9 * U)
        self.furcula_bottom = (0.0, self.sternum_y + 1.2 * U, self.sternum_front_z - 0.6 * U)
        W = spec.wing_half_r * L  # 翼骨半跨（肩 → 指尖的伸展长度）
        self.wing_span_half = W
        # 段长比例取自兀鹫实测：肱 : 尺 : 手 ≈ 0.32 : 0.37 : 0.31
        self.humerus_len = 0.32 * W
        self.ulna_len = 0.37 * W
        self.manus_len = 0.31 * W

    # ------------------------------------------------------------ 颈
    def neck_points(self) -> list[Vec]:
        p0, p1, p2, p3 = self.neck_ctrl
        n = self.spec.cervicals
        return [bezier(p0, p1, p2, p3, i / n) for i in range(n + 1)]

    # ------------------------------------------------------------ 翼
    def wing_joints(self, sx: int) -> tuple[Vec, Vec, Vec, Vec]:
        """(肩, 肘, 腕, 指尖)。段长由 Anatomy 定死，姿态只给方向——

        直接手写四个关节坐标会让骨长随姿态变（收翼时翼骨凭空缩短），
        所以一律「定长 + 定向」正向推。
        """
        sh = self.shoulder(sx)
        if self.pose == "spread":
            # 展开：**肘后掠、腕前伸**，翼尖再微收。这个折线关系决定了前缘 ——
            # 肩腕连线靠前、肘落在它后方，翼膜才有地方绷成一条直边。
            # 反过来让肘凸向前，膜就只能填到后缘去，翼前缘成了折线。
            # 三段**同高**：翼骨一旦有上反角，铺在上面的扁羽板就分居在不同高度上，正面
            # 看是两三个平面而不是一整片翼（实测原来从肩到翼尖抬 3 个单位，加上覆羽只铺
            # 到手部 35%、再往外只剩更高的飞羽，两截错开一整个身位）。真兀鹫确实持 V 形，
            # 但那点角度在体素扁板上只会变成台阶，换不来观感。
            d_hu = (sx * 0.96, 0.0, 0.30)
            d_ul = (sx * 0.99, 0.0, -0.16)
            d_mn = (sx * 0.99, 0.0, -0.12)
        else:
            # 收翼：肱骨向后下 → 前臂折回向前上 → 手再向后，Z 字贴体侧
            d_hu = (sx * 0.26, -0.62, 0.74)
            d_ul = (sx * -0.08, 0.56, -0.82)
            d_mn = (sx * 0.05, -0.32, 0.95)
        el = _step(sh, d_hu, self.humerus_len)
        wr = _step(el, d_ul, self.ulna_len)
        tip = _step(wr, d_mn, self.manus_len)
        return sh, el, wr, tip

    # ------------------------------------------------------------ 腿
    def leg_joints(self, sx: int) -> tuple[Vec, Vec, Vec, Vec]:
        """(髋, 膝, 踝, 趾根)。膝在体内靠前、踝在体外靠后 —— 鸟腿的 Z 字。"""
        U = self.U
        hip = self.hip(sx)
        knee = (sx * (2.9 * U), hip[1] - self.femur_drop, hip[2] - 3.4 * U)
        ankle = (sx * (2.9 * U), knee[1] - self.tibio_drop, knee[2] + 4.1 * U)
        toe = (sx * (2.9 * U), ankle[1] - self.tarsus_len, ankle[2] - 1.5 * U)
        return hip, knee, ankle, toe


# ================================================================ 部件：颅骨
def part_skull(rig: Rig, A: Anatomy) -> None:
    """脑颅 + 巨眼眶（含巩膜环）+ 上喙（下钩）+ 鼻孔 + 方骨 + 额嵴。

    鸟颅是**眼睛撑起来的**：眼眶直径接近脑颅本身，两眶之间只隔一片薄骨（眶间隔）。
    照兽头那样把颅侧做成实心骨板，出来就是个鸡头方块。
    """
    U, S = A.U, A.spec
    occ, sk_y = A.occiput, A.skull_y
    brow_z = A.brow_z
    r = S.bone_r

    # 颅挂在寰椎（C1 = neck_01）上，不是挂在颈根——转头要绕枕髁转
    rig.bone("skull", (0.0, sk_y, occ[2] - A.skull_len * 0.45), parent="neck_01")

    # 脑颅：圆而短，气腔化（bone_air）。分三块堆出穹隆，单块方盒读不出圆颅。
    # 只占颅长的 ~45%，剩下的全归眼眶 —— 鸟颅是被眼睛撑起来的。Round 1 给了 2.05U
    # 半宽 × 64% 颅长，渲出来是台电视机，喙缩成挂在盒子下的一根小杆。
    sl = A.skull_len
    rig.cube("skull", "cranium_mid", (-1.35 * U * r, sk_y - 1.30 * U, occ[2] - sl * 0.52),
             (1.35 * U * r, sk_y + 1.45 * U, occ[2] - 0.10 * U), mat="bone_air")
    rig.cube("skull", "cranium_top", (-0.98 * U * r, sk_y + 1.25 * U, occ[2] - sl * 0.46),
             (0.98 * U * r, sk_y + 2.05 * U, occ[2] - 0.45 * U), mat="bone_air")
    rig.cube("skull", "cranium_rear", (-1.15 * U * r, sk_y - 1.15 * U, occ[2] - 0.30 * U),
             (1.15 * U * r, sk_y + 1.30 * U, occ[2] + 0.75 * U), mat="bone_dark")
    # 枕髁（与寰椎相接的球）
    rig.cube("skull", "occipital_condyle", (-0.7 * U, sk_y - 2.2 * U, occ[2] + 0.2 * U),
             (0.7 * U, sk_y - 1.0 * U, occ[2] + 1.2 * U), mat="cartilage")
    # 眶间隔：两眶之间那片薄骨，把脑颅和喙根连起来（鸟颅这里薄到半透明）
    rig.cube("skull", "interorbital", (-0.48 * U, sk_y - 0.95 * U, brow_z),
             (0.48 * U, sk_y + 1.35 * U, occ[2] - sl * 0.48), mat="bone_air")

    # 眶半径按**颅长**取，不是按 U —— 直接给绝对值会让小档的眼眶比自己的脑袋还长
    orb_r = S.orbit_r * sl
    orb_z = occ[2] - sl * 0.60  # 眶心：颅长中前部
    orb_y = sk_y + 0.15 * U
    for sx, side in ((-1, "l"), (1, "r")):
        # --- 眶环：四段围出眶孔（体素无布尔减法，洞只能"围"出来）---
        rig.cube("skull", f"supraorbital_{side}",  # 眉弓（向外上挑，猛禽的凶相来源）
                 (sx * 0.6 * U, orb_y + orb_r * 0.72, orb_z - orb_r * 0.9),
                 (sx * (1.5 * U + orb_r * 0.55), orb_y + orb_r * 1.05, orb_z + orb_r * 0.85),
                 rot=(0.0, 0.0, sx * 9.0), org=(sx * 0.8 * U, orb_y + orb_r * 0.8, orb_z))
        rig.cube("skull", f"jugal_{side}",  # 颧弓：一根细杆，鸟的颧弓退化成筷子
                 (sx * 0.75 * U, orb_y - orb_r * 0.98, orb_z - orb_r * 1.15),
                 (sx * (0.75 * U + 0.62 * U * r), orb_y - orb_r * 0.55, orb_z + orb_r * 0.95),
                 mat="bone_dark")
        rig.cube("skull", f"lacrimal_{side}",  # 泪骨：眶前壁，向下前伸成一根钩
                 (sx * 0.7 * U, orb_y - orb_r * 0.7, orb_z - orb_r * 1.25),
                 (sx * (0.7 * U + 0.7 * U * r), orb_y + orb_r * 0.95, orb_z - orb_r * 0.8))
        rig.cube("skull", f"postorbital_{side}",  # 眶后突
                 (sx * 0.7 * U, orb_y - orb_r * 0.5, orb_z + orb_r * 0.75),
                 (sx * (0.7 * U + 0.62 * U * r), orb_y + orb_r * 0.9, orb_z + orb_r * 1.1))
        # 眶窝：深色底，把"洞"读出来。只填眶心 —— 铺满整个眶环会糊成一整片黑板，
        # 眶环骨和巩膜环全被吞掉，正面看就是脑袋上开了两个方洞。
        rig.cube("skull", f"orbit_socket_{side}",
                 (sx * 0.42 * U, orb_y - orb_r * 0.52, orb_z - orb_r * 0.55),
                 (sx * (0.42 * U + 0.78 * U), orb_y + orb_r * 0.55, orb_z + orb_r * 0.52),
                 mat="socket")
        # --- 巩膜环：眼球壁里的一圈骨片。鸟类骨架最好认的特征之一，8 片围一圈 ---
        for k in range(8):
            ang = math.radians(k * 45.0)
            ry = orb_r * 0.80 * math.sin(ang)
            rz = orb_r * 0.80 * math.cos(ang)
            plate = 0.30 * U * r
            rig.cube("skull", f"sclerotic_{side}_{k + 1}",
                     (sx * (1.05 * U + orb_r * 0.10), orb_y + ry - plate, orb_z + rz - plate),
                     (sx * (1.05 * U + orb_r * 0.42), orb_y + ry + plate, orb_z + rz + plate),
                     mat="bone_air")
        # 耳孔（眶后下方）
        rig.cube("skull", f"ear_opening_{side}",
                 (sx * 1.2 * U, sk_y - 1.4 * U, occ[2] - 1.2 * U),
                 (sx * 1.85 * U, sk_y - 0.55 * U, occ[2] - 0.35 * U), mat="socket")
        # 方骨：颅底后方的小骨，下颌的铰链（鸟能张大口全靠它）
        rig.cube("skull", f"quadrate_{side}",
                 (sx * 1.0 * U, A.jaw_hinge[1] - 0.35 * U, occ[2] - 0.9 * U),
                 (sx * 1.9 * U, sk_y - 1.2 * U, occ[2] + 0.5 * U),
                 rot=(12.0, 0.0, 0.0), org=(sx * 1.45 * U, sk_y - 1.2 * U, occ[2]),
                 mat="cartilage")

    # 额嵴（异变族征，大档最显）：向后上斜出的一片骨刃，逐段收尖。
    # 做成方柱会在头顶竖起一根烟囱 —— 大档整个脑袋就只剩这个了。
    if S.crest > 0:
        base = (0.0, sk_y + 1.9 * U, occ[2] - sl * 0.42)
        top = (0.0, sk_y + 1.9 * U + S.crest * U, occ[2] + 0.5 * U)
        pts = [_mix(base, top, i / 3) for i in range(4)]
        rig.taper("skull", "frontal_crest", pts,
                  [U * v for v in (0.95, 0.74, 0.48, 0.22)], mat="keratin_pale", flat=0.38)

    part_beak(rig, A)


def part_beak(rig: Rig, A: Anatomy) -> None:
    """上喙：前颌骨延长，角质鞘包覆，尖端下钩。

    钩不是"末段斜一点"——兀鹫的钩是**尖端垂直下折**成一枚倒刺，用来撕开尸皮。
    做成缓斜就成了鸭嘴。
    """
    U, S = A.U, A.spec
    root, tip = A.beak_root, A.beak_tip
    depth = S.beak_depth * U
    r = S.bone_r

    n = 5
    pts, radii = [], []
    for i in range(n + 1):
        t = i / n
        p = _mix(root, tip, t)
        # 喙背轻微下凹（culmen 弧），越靠尖越薄
        p = (p[0], p[1] - 0.35 * U * math.sin(math.pi * t) - depth * 0.5 * t * 0.35, p[2])
        pts.append(p)
        radii.append(lerp(depth * 0.5, depth * 0.16, t))
    # flat = 宽/深。兀鹫喙深略大于宽（侧扁），0.62 太扁 —— 渲出来是把刀不是喙。
    rig.taper("skull", "rhamphotheca", pts, radii, mat="keratin", flat=0.76)

    # 上颌骨：把喙根接回眶前下缘。少了这块，喙看着像根单插上去的杆，
    # 颅和喙之间硬生生断一节。
    rig.cube("skull", "maxilla",
             (-depth * 0.30, root[1] - depth * 0.26, root[2] - 0.4 * U),
             (depth * 0.30, root[1] + depth * 0.40, root[2] + A.skull_len * 0.42))

    # 钩：自喙尖垂直下折成一枚倒刺 —— 撕尸皮的那一下全靠它，必须做粗做狠
    hook_len = S.hook * U
    if hook_len > 0:
        rig.shaft("skull", "beak_hook", _add(tip, dz=0.3 * U), _add(tip, dy=-hook_len, dz=-0.1 * U),
                  0.42 * U * r, 0.62 * U * r, mat="keratin")
        rig.cube("skull", "beak_hook_tip",
                 (-0.30 * U, tip[1] - hook_len - 0.55 * U, tip[2] - 0.62 * U),
                 (0.30 * U, tip[1] - hook_len + 0.35 * U, tip[2] + 0.30 * U),
                 mat="keratin_pale")

    # 蜡膜（cere）：喙根那截裸皮，兀鹫上有大鼻孔
    rig.cube("skull", "cere", (-depth * 0.34, root[1] - depth * 0.42, root[2] - 1.3 * U),
             (depth * 0.34, root[1] + depth * 0.30, root[2] + 0.5 * U), mat="keratin_pale")
    for sx, side in ((-1, "l"), (1, "r")):
        rig.cube("skull", f"naris_{side}",
                 (sx * depth * 0.16, root[1] - depth * 0.18, root[2] - 1.15 * U),
                 (sx * depth * 0.36, root[1] + depth * 0.16, root[2] - 0.25 * U),
                 mat="socket")


def part_jaw(rig: Rig, A: Anatomy) -> None:
    """下颌：两条细长骨支，后端吊在方骨上，前端在中线联合。鸟无牙。"""
    U, S = A.U, A.spec
    hinge, root, tip = A.jaw_hinge, A.beak_root, A.beak_tip
    r = S.bone_r
    rig.bone("jaw", hinge, parent="skull")

    jaw_tip = (0.0, tip[1] + 0.30 * S.beak_depth * U, tip[2] + 0.55 * U)
    for sx, side in ((-1, "l"), (1, "r")):
        a = (sx * 1.15 * U, hinge[1], hinge[2])
        b = (sx * 0.42 * U, jaw_tip[1] + 0.25 * U, jaw_tip[2] + 0.9 * U)
        rig.shaft("jaw", f"mandible_ramus_{side}", a, b, 0.34 * U * r, 0.50 * U * r)
        # 下颌角（后端向后突出的小片，颌肌附着）
        rig.cube("jaw", f"mandible_angle_{side}",
                 (sx * 0.9 * U, hinge[1] - 0.30 * U, hinge[2] - 0.1 * U),
                 (sx * 1.6 * U, hinge[1] + 0.75 * U, hinge[2] + 1.5 * U), mat="bone_dark")
    # 联合部 + 下喙鞘（比上喙短，闭口时藏在钩下）
    rig.shaft("jaw", "mandible_symphysis",
              (-0.42 * U, jaw_tip[1] + 0.25 * U, jaw_tip[2] + 0.9 * U),
              (0.42 * U, jaw_tip[1] + 0.25 * U, jaw_tip[2] + 0.9 * U),
              0.30 * U * r, 0.42 * U * r)
    lower = [_mix((0.0, jaw_tip[1] + 0.25 * U, jaw_tip[2] + 0.9 * U), jaw_tip, i / 3) for i in range(4)]
    rig.taper("jaw", "lower_rhamphotheca", lower,
              [S.beak_depth * U * v for v in (0.30, 0.24, 0.18, 0.10)], mat="keratin", flat=0.66)


# ================================================================ 部件：颈
def part_neck(rig: Rig, A: Anatomy) -> None:
    """颈椎 N 节沿 S 曲线；每节一根骨骼（鸟颈的表现力全在逐节弯曲上）。

    棘突向背侧穿出成骨钉 —— 末法异变的族征，档位越高越长。
    """
    U, S = A.U, A.spec
    pts = A.neck_points()
    n = S.cervicals
    r = S.bone_r

    parent = "trunk_front"
    for i in range(n):
        # 自尾端（C_n，接胸）向头端编号，与解剖习惯一致：C1 = 寰椎在最前
        a, b = pts[i], pts[i + 1]
        idx = n - i  # C_idx
        t = i / max(1, n - 1)
        name = f"neck_{idx:02d}"
        rig.bone(name, a, parent=parent)
        # 椎体：中段最粗（颈中承重最大），两端收
        thick = lerp(1.18, 0.74, abs(t - 0.42) / 0.58) * U * r
        rig.shaft(name, f"cervical_{idx}", a, b, thick, thick * 1.15, mat="bone_air")

        d = (b[0] - a[0], b[1] - a[1], b[2] - a[2])
        seg = math.sqrt(sum(c * c for c in d)) or 1.0
        # 背侧法线：切线在矢状面内顺时针转 90°，(z,y) → (ty, -tz)。
        # 取成逆时针会把棘突全甩到腹侧——从喉咙里长出一排骨钉。
        ny, nz = -d[2] / seg, d[1] / seg
        mid = _mix(a, b, 0.5)
        # 棘突 + 外露骨钉
        # 骨钉只在颈的中后段外露（前段被头压着，长不出来），且**隔节**才穿出皮面——
        # 每节都顶一根，整条颈就成了一把梳子，异变感变成装饰花纹。
        spike = S.neck_spike * U * max(0.0, math.sin(math.pi * min(1.0, t * 1.28)) ** 0.7)
        if idx % 2:
            spike *= 0.22
        out = 0.72 * U + spike
        tipp = (mid[0], mid[1] + ny * out, mid[2] + nz * out)
        rig.shaft(name, f"cervical_spine_{idx}", mid, tipp, 0.22 * U * r, 0.30 * U * r,
                  mat="bone_dark" if spike < 0.35 * U else "keratin_pale")
        # 横突（左右一对小翼，颈侧肌附着）
        for sx, side in ((-1, "l"), (1, "r")):
            rig.cube(name, f"cervical_tp_{idx}_{side}",
                     (sx * thick * 0.9, mid[1] - 0.35 * U, mid[2] - 0.32 * U),
                     (sx * (thick * 0.9 + 0.75 * U * r), mid[1] + 0.25 * U, mid[2] + 0.32 * U),
                     mat="bone_dark")
        parent = name


# ================================================================ 部件：躯干
def part_trunk(rig: Rig, A: Anatomy) -> None:
    """notarium（愈合胸椎）+ synsacrum（综荐骨，与髂骨长成一块）。

    鸟的躯干脊柱几乎不能动 —— 飞行要求刚性箱体。所以这里只给两根骨骼
    （trunk_front / hips），不像兽那样逐节可动。
    """
    U, S = A.U, A.spec
    ty = A.trunk_y
    r = S.bone_r

    rig.bone("hips", (0.0, ty - 0.4 * U, A.synsacrum_back_z - 3.0 * U), parent="root")
    rig.bone("trunk_front", (0.0, ty, A.t1_z), parent="hips")

    # --- notarium：T1 → 愈合段后端，一条带棘突脊的整骨 ---
    nb = A.notarium_back_z
    rig.cube("trunk_front", "notarium", (-0.72 * U * r, ty - 0.80 * U, A.t1_z),
             (0.72 * U * r, ty + 0.72 * U, nb), mat="bone_air")
    # 背侧愈合脊：基座是连续的板（椎骨确实长在一起了），但**棘突仍逐节可辨**。
    # Round 2 只给一条光板 + 几道横棱，整个背读成一根长方棍 —— 一眼假。
    rig.cube("trunk_front", "notarium_crest", (-0.26 * U, ty + 0.62 * U, A.t1_z + 0.3 * U),
             (0.26 * U, ty + 1.55 * U, nb - 0.4 * U), mat="bone_dark")
    n_sp = max(4, int(6 * A.U ** 0.4))
    for k in range(n_sp):
        t = k / max(1, n_sp - 1)
        z = lerp(A.t1_z + 0.9 * U, nb - 0.9 * U, t)
        # 前高后低：鬐甲在肩带上方（飞行肌的拉力最大处）
        h = lerp(2.75, 1.75, t) * U
        rig.cube("trunk_front", f"notarium_spine_{k + 1}",
                 (-0.30 * U * r, ty + 1.3 * U, z - 0.34 * U),
                 (0.30 * U * r, ty + h, z + 0.34 * U),
                 rot=(lerp(-9.0, 7.0, t), 0.0, 0.0), org=(0.0, ty + 1.3 * U, z))
        # 横突：肋头就搭在这上面
        for sx, side in ((-1, "l"), (1, "r")):
            rig.cube("trunk_front", f"notarium_tp_{k + 1}_{side}",
                     (sx * 0.66 * U * r, ty - 0.30 * U, z - 0.26 * U),
                     (sx * (0.66 * U * r + 0.85 * U), ty + 0.34 * U, z + 0.26 * U),
                     mat="bone_dark")

    # --- synsacrum：宽板，前接 notarium，后到尾根；髂骨翼与它愈合 ---
    sb = A.synsacrum_back_z
    rig.cube("hips", "synsacrum", (-0.85 * U * r, ty - 0.90 * U, nb),
             (0.85 * U * r, ty + 0.72 * U, sb), mat="bone_air")
    for sx, side in ((-1, "l"), (1, "r")):
        # 髂骨：与综荐骨愈合的宽翼，向外后铺开成骨盆顶盖
        rig.cube("hips", f"ilium_{side}", (sx * 0.9 * U, ty + 0.15 * U, nb + 0.6 * U),
                 (sx * (0.9 * U + 2.5 * U * r), ty + 1.15 * U, sb - 0.3 * U),
                 rot=(0.0, 0.0, sx * -7.0), org=(sx * 0.9 * U, ty + 0.6 * U, nb))
        # 坐骨（向后下）+ 耻骨（细，向后，鸟的耻骨不闭合 —— 为产卵）
        hip = A.hip(sx)
        rig.cube("hips", f"ischium_{side}", (sx * 1.0 * U, hip[1] - 1.5 * U, hip[2] + 0.2 * U),
                 (sx * (1.0 * U + 2.0 * U * r), hip[1] + 0.9 * U, sb + 0.9 * U),
                 rot=(9.0, 0.0, 0.0), org=(sx * 1.5 * U, hip[1], hip[2]), mat="bone_dark")
        rig.shaft("hips", f"pubis_{side}", _add(hip, dx=sx * 0.1 * U, dy=-1.7 * U),
                  (sx * 1.5 * U, hip[1] - 2.4 * U, sb + 2.6 * U), 0.30 * U * r, mat="bone_dark")
        # 髋臼
        rig.cube("hips", f"acetabulum_{side}", (sx * 0.95 * U, hip[1] - 0.95 * U, hip[2] - 0.95 * U),
                 (sx * (0.95 * U + 1.5 * U), hip[1] + 0.95 * U, hip[2] + 0.95 * U), mat="cartilage")


def part_tail(rig: Rig, A: Anatomy) -> None:
    """自由尾椎 N 节 + 尾综骨（pygostyle）。大档尾综骨末端延成骨刃。"""
    U, S = A.U, A.spec
    ty = A.trunk_y
    parent = "hips"
    z0, z1 = A.synsacrum_back_z, A.tail_end_z
    for i in range(S.tail_verts):
        t = i / max(1, S.tail_verts - 1)
        za = lerp(z0, z1, i / S.tail_verts)
        zb = lerp(z0, z1, (i + 1) / S.tail_verts)
        # 尾略下垂
        ya = ty - 0.5 * U - 1.1 * U * (i / S.tail_verts) ** 1.5
        yb = ty - 0.5 * U - 1.1 * U * ((i + 1) / S.tail_verts) ** 1.5
        name = f"tail_{i + 1:02d}"
        rig.bone(name, (0.0, ya, za), parent=parent)
        rad = lerp(1.05, 0.68, t) * U * S.bone_r
        rig.shaft(name, f"caudal_{i + 1}", (0.0, ya, za), (0.0, yb, zb), rad, rad * 1.1)
        # 尾椎横突（尾羽根部肌肉附着）
        for sx, side in ((-1, "l"), (1, "r")):
            rig.cube(name, f"caudal_tp_{i + 1}_{side}",
                     (sx * rad * 0.9, (ya + yb) / 2 - 0.22 * U, (za + zb) / 2 - 0.28 * U),
                     (sx * (rad * 0.9 + 0.6 * U), (ya + yb) / 2 + 0.22 * U, (za + zb) / 2 + 0.28 * U),
                     mat="bone_dark")
        parent = name

    # 尾综骨：犁形，向后上翘 —— 尾羽扇形排在它上面
    yb = ty - 0.5 * U - 1.1 * U
    rig.bone("pygostyle", (0.0, yb, z1), parent=parent)
    pyg_end = (0.0, yb + 1.6 * U, A.pygo_end_z)
    pts = [_mix((0.0, yb, z1), pyg_end, i / 3) for i in range(4)]
    rig.taper("pygostyle", "pygostyle", pts, [U * v for v in (1.05, 1.0, 0.85, 0.55)], flat=0.55)
    if S.pygo_blade > 0:
        # 骨刃：自尾综骨末端向后上挑出，逐段收尖（方块版本是块斜插的门板）
        blade = S.pygo_blade * U
        tipb = (0.0, pyg_end[1] + blade * 0.86, pyg_end[2] + blade * 0.52)
        pts = [_mix(pyg_end, tipb, i / 3) for i in range(4)]
        rig.taper("pygostyle", "pygostyle_blade", pts,
                  [U * v for v in (0.52, 0.40, 0.28, 0.14)], mat="keratin_pale", flat=0.40)


# ================================================================ 部件：胸廓
def part_ribcage(rig: Rig, A: Anatomy) -> None:
    """N 对肋（椎肋 + 胸肋两段折弯）+ 钩状突 + 胸骨板 + 龙骨突。

    钩状突（uncinate process）是鸟类独有：每根肋后缘伸出一片骨，斜搭在**后一根**肋上，
    把整个胸廓锁成刚性框——没有它，飞行时的肌力会把胸廓拉塌。少了这排斜片，
    骨架一眼就不是鸟。
    """
    U, S = A.U, A.spec
    ty, r = A.trunk_y, S.bone_r
    hw, depth = A.thorax_half_w, A.thorax_depth
    st_y = A.sternum_y
    z_front, z_back = A.t1_z + 0.8 * U, A.notarium_back_z - 0.4 * U

    for i in range(S.ribs):
        t = i / max(1, S.ribs - 1)
        z = lerp(z_front, z_back, t)
        # 中段胸廓最鼓
        bulge = math.exp(-(((t - 0.45) / 0.55) ** 2))
        w = hw * lerp(0.78, 1.0, bulge)
        d = depth * lerp(0.86, 1.0, bulge)
        for sx, side in ((-1, "l"), (1, "r")):
            # 椎肋：自椎体向外下后
            p_vert = (sx * 0.95 * U * r, ty - 0.5 * U, z)
            p_bend = (sx * w, ty - d * 0.55, z + 0.9 * U)
            # 胸肋：向内下前，接胸骨侧缘
            sz = lerp(A.sternum_front_z + 1.0 * U, A.sternum_back_z - 0.6 * U, t)
            p_stern = (sx * (hw * 0.52), st_y + 0.5 * U, sz)
            rig.shaft(rig_bone_for(A, z), f"rib_{i + 1}_{side}_vert", p_vert, p_bend, 0.30 * U * r, 0.38 * U * r)
            rig.shaft(rig_bone_for(A, z), f"rib_{i + 1}_{side}_stern", p_bend, p_stern,
                      0.26 * U * r, 0.34 * U * r, mat="cartilage")
            # 钩状突：自弯折点向后上斜出一片，搭到后一根肋
            if i < S.ribs - 1:
                rig.cube(rig_bone_for(A, z), f"uncinate_{i + 1}_{side}",
                         (sx * (w - 0.25 * U), p_bend[1] - 0.18 * U, p_bend[2] - 0.22 * U),
                         (sx * (w + 0.28 * U * r), p_bend[1] + 2.3 * U, p_bend[2] + 0.22 * U),
                         rot=(38.0, 0.0, 0.0), org=(sx * w, p_bend[1], p_bend[2]), mat="bone_dark")

    # --- 胸骨板：宽、平、长，一直铺到腹部 ---
    rig.cube("trunk_front", "sternum_plate", (-hw * 0.52, st_y, A.sternum_front_z),
             (hw * 0.52, st_y + 0.55 * U, A.sternum_back_z), mat="bone_air")
    # 胸骨前缘（叉骨/乌喙骨的插槽）
    rig.cube("trunk_front", "sternum_rostrum", (-0.75 * U, st_y + 0.4 * U, A.sternum_front_z - 1.5 * U),
             (0.75 * U, st_y + 1.5 * U, A.sternum_front_z + 0.6 * U), mat="bone_dark")
    # --- 龙骨突：中线向下伸出的薄板，飞行肌全靠它 ---
    # 底缘是**弧**不是直角：前深后浅，末端收进腹壁。整块矩形板渲出来是个挂在
    # 胸下的箱子，比什么都抢眼。
    keel_y = A.keel_y
    kz0, kz1 = A.sternum_front_z - 1.0 * U, A.sternum_back_z - 1.6 * U
    n_k = 5
    for k in range(n_k):
        t0, t1 = k / n_k, (k + 1) / n_k
        za, zb = lerp(kz0, kz1, t0), lerp(kz0, kz1, t1)
        # 最深处在前 1/3（胸大肌的着力点），向后抬起
        drop = math.sin(math.pi * min(1.0, 0.35 + 0.65 * (1 - t0))) ** 0.8
        rig.cube("trunk_front", f"keel_{k + 1}",
                 (-0.24 * U * r, lerp(st_y + 0.3 * U, keel_y, drop), za),
                 (0.24 * U * r, st_y + 0.3 * U, zb), mat="bone_air")
    # 龙骨前缘加厚（着力最大处），并把胸骨龙骨接缝补实
    rig.shaft("trunk_front", "keel_ridge",
              (0.0, keel_y + 0.35 * U, kz0 + 0.2 * U),
              (0.0, st_y + 0.4 * U, kz0 - 1.0 * U), 0.42 * U * r, 0.50 * U * r,
              mat="bone_dark")


def rig_bone_for(A: Anatomy, z: float) -> str:
    """肋挂在哪根躯干骨上 —— 愈合段挂 trunk_front，后段挂 hips。"""
    return "trunk_front" if z < A.notarium_back_z - 0.5 else "hips"


# ================================================================ 部件：肩带
def part_shoulder(rig: Rig, A: Anatomy) -> None:
    """乌喙骨 + 叉骨 + 肩胛 —— 鸟类肩带的三件套。

    乌喙骨是**受压支柱**：把肩关节顶在离胸骨一段距离的位置，抵住飞行时向内的拉力；
    哺乳类没有这根骨（退化成肩胛上一个突起），照兽做就会让翅膀直接长在肋骨上。
    """
    U, S = A.U, A.spec
    r = S.bone_r
    for sx, side in ((-1, "l"), (1, "r")):
        sh = A.shoulder(sx)
        # 乌喙骨：肩 → 胸骨前缘，粗壮
        coracoid_foot = (sx * 0.95 * U, A.sternum_y + 1.0 * U, A.sternum_front_z - 0.3 * U)
        rig.bone(f"coracoid_{side}", sh, parent="trunk_front")
        rig.shaft(f"coracoid_{side}", f"coracoid_{side}_shaft", sh, coracoid_foot,
                  0.55 * U * r, 0.72 * U * r)
        # 关节盂（肩臼）
        rig.cube(f"coracoid_{side}", f"glenoid_{side}",
                 (sh[0] - sx * 0.85 * U, sh[1] - 0.8 * U, sh[2] - 0.8 * U),
                 (sh[0] + sx * 0.85 * U, sh[1] + 0.8 * U, sh[2] + 0.8 * U), mat="cartilage")
        # 肩胛：细长刀片，自肩关节沿肋骨背侧向后平铺（鸟肩胛不竖立）
        scap_end = (sx * 1.5 * U, A.trunk_y + 1.0 * U, A.notarium_back_z - 0.5 * U)
        rig.shaft(f"coracoid_{side}", f"scapula_{side}", _add(sh, dy=0.3 * U), scap_end,
                  0.28 * U * r, 0.85 * U * r, mat="bone_dark")
        # 叉骨（锁骨）：自肩向前下汇到中线
        rig.shaft("trunk_front", f"furcula_{side}", _add(sh, dx=-sx * 0.2 * U, dy=-0.3 * U),
                  A.furcula_bottom, 0.34 * U * r, 0.46 * U * r, mat="bone_air")
    # 叉骨下端愈合处（V 底的那颗结）
    rig.cube("trunk_front", "furcula_symphysis",
             (-0.62 * U, A.furcula_bottom[1] - 0.55 * U, A.furcula_bottom[2] - 0.5 * U),
             (0.62 * U, A.furcula_bottom[1] + 0.5 * U, A.furcula_bottom[2] + 0.5 * U), mat="bone_air")


# ================================================================ 部件：翼
def part_wing(rig: Rig, A: Anatomy, sx: int, side: str) -> None:
    """肱骨 → 桡+尺 → 腕骨 → 腕掌骨（夹窗）→ 小翼指(+骨爪) / 主指 / 第三指。"""
    U, S = A.U, A.spec
    r = S.bone_r
    sh, el, wr, tip = A.wing_joints(sx)

    hu = rig.bone(f"humerus_{side}", sh, parent=f"coracoid_{side}")
    ul = rig.bone(f"ulna_{side}", el, parent=hu)
    ca = rig.bone(f"carpus_{side}", wr, parent=ul)

    # 肱骨：短粗，近端有三角肌嵴（鸟的肱骨头很方）
    rig.shaft(hu, f"humerus_{side}_shaft", sh, el, 0.62 * U * r, 0.72 * U * r, mat="bone_air")
    rig.cube(hu, f"deltoid_crest_{side}",
             (min(sh[0], sh[0] + sx * 1.1 * U), sh[1] - 0.2 * U, sh[2] - 0.55 * U),
             (max(sh[0], sh[0] + sx * 1.1 * U), sh[1] + 1.15 * U, sh[2] + 0.9 * U), mat="bone_dark")
    rig.cube(hu, f"humerus_{side}_condyle",
             (el[0] - sx * 0.8 * U, el[1] - 0.75 * U, el[2] - 0.8 * U),
             (el[0] + sx * 0.8 * U, el[1] + 0.75 * U, el[2] + 0.8 * U), mat="cartilage")

    # 前臂：尺骨粗（承羽）、桡骨细，两骨之间留缝。
    # 岔开方向必须落在**翼面内**：展翼时翼面近水平，桡尺前后并排；收翼时翼面竖在
    # 体侧，两骨上下摞。参考向量选错，展翼的前臂会立起来变成两根叠在一起的柱子。
    d_perp = _perp(el, wr, (0.0, 0.0, 1.0) if A.pose == "spread" else (0.0, 1.0, 0.0))
    ul_a = _add(el, *(c * 0.42 * U for c in d_perp))
    ul_b = _add(wr, *(c * 0.42 * U for c in d_perp))
    ra_a = _add(el, *(-c * 0.48 * U for c in d_perp))
    ra_b = _add(wr, *(-c * 0.48 * U for c in d_perp))
    rig.shaft(ul, f"ulna_{side}_shaft", ul_a, ul_b, 0.46 * U * r, 0.52 * U * r, mat="bone_air")
    rig.shaft(ul, f"radius_{side}_shaft", ra_a, ra_b, 0.30 * U * r, 0.34 * U * r, mat="bone_dark")
    # 羽茎瘤（quill knobs）：次级飞羽插在尺骨上的一排小疙瘩
    knobs = max(3, int(6 * A.wing_span_half / 30))
    for k in range(knobs):
        p = _mix(ul_a, ul_b, (k + 0.6) / (knobs + 0.2))
        # 疙瘩朝**外侧**突出，两端一律带 sx —— 写成 p[0]±常量 + sx·偏移，
        # 左右两串就整体错开一截，check 的镜像对拍会当场逮住。
        rig.cube(ul, f"quill_knob_{side}_{k + 1}",
                 (p[0] - sx * 0.22 * U, p[1] - 0.20 * U, p[2] - 0.22 * U),
                 (p[0] + sx * 0.52 * U, p[1] + 0.20 * U, p[2] + 0.22 * U), mat="bone_dark")

    # 腕骨两枚（桡腕 + 尺腕）
    for k, off in enumerate((0.45, -0.45)):
        p = _add(wr, *(c * off * U for c in d_perp))
        rig.cube(ca, f"carpal_{side}_{k + 1}",
                 (p[0] - 0.42 * U, p[1] - 0.42 * U, p[2] - 0.42 * U),
                 (p[0] + 0.42 * U, p[1] + 0.42 * U, p[2] + 0.42 * U), mat="cartilage")

    # 腕掌骨：两条愈合骨夹一个窗（鸟手的招牌），窗心填 socket 才读得出是"框"
    mn = rig.bone(f"manus_{side}", wr, parent=ca)
    mc_end = _mix(wr, tip, 0.62)
    a_hi = _add(wr, *(c * 0.40 * U for c in d_perp))
    b_hi = _add(mc_end, *(c * 0.34 * U for c in d_perp))
    a_lo = _add(wr, *(-c * 0.40 * U for c in d_perp))
    b_lo = _add(mc_end, *(-c * 0.34 * U for c in d_perp))
    rig.shaft(mn, f"carpometacarpus_{side}_major", a_hi, b_hi, 0.34 * U * r, 0.42 * U * r, mat="bone_air")
    rig.shaft(mn, f"carpometacarpus_{side}_minor", a_lo, b_lo, 0.24 * U * r, 0.30 * U * r, mat="bone_dark")
    mid_a, mid_b = _mix(a_hi, a_lo, 0.5), _mix(b_hi, b_lo, 0.5)
    rig.shaft(mn, f"carpometacarpus_{side}_window", _mix(mid_a, mid_b, 0.18), _mix(mid_a, mid_b, 0.82),
              0.16 * U, 0.16 * U, mat="socket")

    # 主指（digit II）2 节 + 第三指
    d2_a = mc_end
    d2_b = _mix(wr, tip, 0.86)
    rig.shaft(mn, f"digit2_{side}_p1", d2_a, d2_b, 0.26 * U * r, 0.32 * U * r)
    rig.shaft(mn, f"digit2_{side}_p2", d2_b, tip, 0.18 * U * r, 0.24 * U * r, mat="bone_dark")
    d3_end = _add(_mix(wr, tip, 0.80), *(-c * 0.55 * U for c in d_perp))
    rig.shaft(mn, f"digit3_{side}", _mix(mid_a, mid_b, 0.95), d3_end, 0.16 * U * r, 0.20 * U * r,
              mat="bone_dark")

    # 小翼指（alula，digit I）：自腕部前缘岔出的短指，撑着那簇"小翼羽"。
    # 异变档在它末端长出骨爪 —— 攀尸山的抓钩。
    al_base = _add(wr, *(c * 0.55 * U for c in d_perp))
    al_axis = _norm((tip[0] - wr[0], tip[1] - wr[1], tip[2] - wr[2]))
    al_dir = tuple(al_axis[i] * 0.75 + d_perp[i] * 0.66 for i in range(3))
    al_end = _step(al_base, al_dir, 0.34 * A.manus_len)
    rig.shaft(mn, f"alula_{side}", al_base, al_end, 0.22 * U * r, 0.26 * U * r)
    if S.alula_claw > 0:
        # 爪自小翼指尖再向前弯出去（与指骨成钝角），沿翼前缘朝外
        claw_dir = tuple(al_axis[i] * 0.55 + d_perp[i] * 0.42 for i in range(3))
        claw_dir = (claw_dir[0], claw_dir[1] - 0.45, claw_dir[2])
        claw_tip = _step(al_end, claw_dir, S.alula_claw * U)
        rig.shaft(mn, f"alula_claw_{side}", al_end, claw_tip, 0.16 * U * r, 0.20 * U * r, mat="claw")


# 岔开方向 / 归一化搬去 rigkit 供肌肉层共用（前臂的伸屈肌要沿同一条轴岔开）
_perp = perp_to
_norm = normalize


# ================================================================ 部件：腿
def part_leg(rig: Rig, A: Anatomy, sx: int, side: str) -> None:
    """股骨（体内）→ 髌 → 胫跗 + 退化腓骨 → 跗跖 → 3 前 1 后趾。

    外露的那个向后折的关节是**踝**不是膝 —— 鸟腿看着"反关节"就是因为膝藏在体羽里。
    秃鹫的爪钝而直：它不抓活物，只在尸体上站稳。
    """
    U, S = A.U, A.spec
    r = S.bone_r
    hip, knee, ankle, toe = A.leg_joints(sx)

    fe = rig.bone(f"femur_{side}", hip, parent="hips")
    ti = rig.bone(f"tibiotarsus_{side}", knee, parent=fe)
    ta = rig.bone(f"tarsometatarsus_{side}", ankle, parent=ti)
    tz = rig.bone(f"toes_{side}", toe, parent=ta)

    rig.shaft(fe, f"femur_{side}_shaft", hip, knee, 0.78 * U * r, mat="bone_air")
    rig.cube(fe, f"femur_{side}_head",
             (hip[0] - sx * 0.9 * U, hip[1] - 0.7 * U, hip[2] - 0.7 * U),
             (hip[0] + sx * 0.35 * U, hip[1] + 0.7 * U, hip[2] + 0.7 * U), mat="cartilage")
    rig.cube(fe, f"patella_{side}",  # 髌骨：膝前那颗小豆
             (knee[0] - sx * 0.45 * U, knee[1] - 0.1 * U, knee[2] - 1.35 * U),
             (knee[0] + sx * 0.45 * U, knee[1] + 1.0 * U, knee[2] - 0.45 * U), mat="cartilage")

    # 胫跗骨（膝 → 踝）+ 腓骨（退化成一根贴在外侧的细刺，只到一半）
    rig.shaft(ti, f"tibiotarsus_{side}_shaft", knee, ankle, 0.62 * U * r, 0.70 * U * r, mat="bone_air")
    rig.shaft(ti, f"fibula_{side}", _add(knee, dx=sx * 0.72 * U, dy=-0.3 * U),
              _mix(_add(knee, dx=sx * 0.72 * U), ankle, 0.55), 0.20 * U * r, mat="bone_dark")
    rig.cube(ti, f"tarsal_joint_{side}",  # 跗间关节（外露的"反膝"）
             (ankle[0] - sx * 0.8 * U, ankle[1] - 0.7 * U, ankle[2] - 0.75 * U),
             (ankle[0] + sx * 0.8 * U, ankle[1] + 0.7 * U, ankle[2] + 0.75 * U), mat="cartilage")

    # 跗跖骨：竖直，横截面前后扁（鸟的"小腿"其实是脚背）
    rig.shaft(ta, f"tarsometatarsus_{side}", ankle, toe, 0.52 * U * r, 0.62 * U * r, mat="bone_air")
    rig.cube(ta, f"hypotarsus_{side}",  # 后上方的滑车突（屈肌腱穿过）
             (ankle[0] - sx * 0.45 * U, ankle[1] - 1.6 * U, ankle[2] + 0.55 * U),
             (ankle[0] + sx * 0.45 * U, ankle[1] - 0.2 * U, ankle[2] + 1.25 * U), mat="bone_dark")

    # 趾：3 前 1 后（anisodactyl）。前趾略外撇，后趾（拇趾）向后短。
    front = ((-0.62, -1.0), (0.0, -1.0), (0.62, -1.0))  # (横向偏, 前向)
    for k, (spread_x, fwd) in enumerate(front, start=1):
        _toe(rig, A, tz, side, f"{k}", toe, sx, spread_x, fwd, segs=3,
             length=2.55 * U * (1.0 if k == 2 else 0.82))
    _toe(rig, A, tz, side, "h", toe, sx, 0.0, 1.0, segs=2, length=1.55 * U)


def _toe(rig: Rig, A: Anatomy, bone: str, side: str, tag: str, base: Vec, sx: int,
         spread_x: float, fwd: float, *, segs: int, length: float) -> None:
    """一根趾：逐节落到地面，末端一枚钝爪。

    趾必须**踩在 y=0 上**：抬高一点点在三视图里看不出，进游戏就是浮空鸟。
    """
    U, S = A.U, A.spec
    r = S.bone_r
    # 趾平铺在地上，末节中心停在自身半径的高度 —— 这样**底面**贴地而不是轴心贴地。
    ground = 0.30 * U * r
    p = _add(base, dx=sx * spread_x * 0.9 * U)
    seg_len = length / segs
    for i in range(segs):
        t = (i + 1) / segs
        nxt = (p[0] + sx * spread_x * 0.35 * U,
               lerp(base[1], ground, t),
               p[2] + fwd * seg_len)
        rig.shaft(bone, f"toe{tag}_{side}_p{i + 1}", p, nxt,
                  lerp(0.34, 0.24, t) * U * r, lerp(0.38, 0.26, t) * U * r,
                  mat="bone" if i == 0 else "bone_dark")
        p = nxt
    # 爪：钝而直（秃鹫不抓活物，爪在尸骨上磨平了）
    claw_end = (p[0] + sx * spread_x * 0.2 * U, ground * 0.88, p[2] + fwd * 0.95 * U)
    rig.shaft(bone, f"claw{tag}_{side}", p, claw_end, 0.22 * U * r, 0.26 * U * r, mat="claw")


# ================================================================ 装配
def _root(rig: Rig) -> None:
    rig.bone("root", (0.0, 0.0, 0.0))


def _spine_stub(rig: Rig, A: Anatomy) -> None:
    """单件预览用：只建父骨骼链，不铺全身。"""
    _root(rig)
    part_trunk(rig, A)
    part_neck(rig, A)


PARTS: dict[str, tuple[str, str]] = {
    "skull": ("颅骨 + 喙", "skull"),
    "jaw": ("下颌", "jaw"),
    "neck": ("颈椎 S 曲线", "neck"),
    "trunk": ("躯干（notarium + synsacrum）", "trunk"),
    "tail": ("尾椎 + 尾综骨", "tail"),
    "ribcage": ("胸廓 + 龙骨突", "ribcage"),
    "shoulder": ("肩带（乌喙骨/叉骨/肩胛）", "shoulder"),
    "wing": ("翼", "wing"),
    "leg": ("腿", "leg"),
}


def build(spec: Spec, pose: str = "folded", part: str | None = None) -> tuple[Rig, Anatomy]:
    rig = Rig(MATS)
    A = Anatomy(spec, pose=pose)

    if part is None:
        _root(rig)
        part_trunk(rig, A)
        part_ribcage(rig, A)
        part_neck(rig, A)
        part_skull(rig, A)
        part_jaw(rig, A)
        part_tail(rig, A)
        part_shoulder(rig, A)
        for sx, side in ((-1, "l"), (1, "r")):
            part_wing(rig, A, sx, side)
            part_leg(rig, A, sx, side)
        return rig, A

    # 单件预览：补齐该件所需的父骨骼链
    if part in ("skull", "jaw", "neck"):
        _root(rig)
        part_trunk(rig, A)
        part_neck(rig, A)
        if part in ("skull", "jaw"):
            part_skull(rig, A)
        if part == "jaw":
            part_jaw(rig, A)
    elif part == "trunk":
        _root(rig)
        part_trunk(rig, A)
    elif part == "tail":
        _root(rig)
        part_trunk(rig, A)
        part_tail(rig, A)
    elif part == "ribcage":
        _root(rig)
        part_trunk(rig, A)
        part_ribcage(rig, A)
    elif part == "shoulder":
        _root(rig)
        part_trunk(rig, A)
        part_ribcage(rig, A)
        part_shoulder(rig, A)
    elif part == "wing":
        _root(rig)
        part_trunk(rig, A)
        part_shoulder(rig, A)
        for sx, side in ((-1, "l"), (1, "r")):
            part_wing(rig, A, sx, side)
    elif part == "leg":
        _root(rig)
        part_trunk(rig, A)
        for sx, side in ((-1, "l"), (1, "r")):
            part_leg(rig, A, sx, side)
    else:
        raise ValueError(f"未知部件: {part}")
    return rig, A


# ================================================================ 自检
def check(spec: Spec, pose: str = "folded", verbose: bool = True) -> int:
    """结构自检：镜像 · 贴地 · 尺度对拍 · 骨链。返回违例数。

    目视核不出这些 —— 尺度偏了 15% 在单张渲染里看着"也还行"，三档摆一起才露馅。
    """
    rig, A = build(spec, pose=pose)
    problems: list[str] = list(rig.mirror_violations())
    (x0, y0, z0), (x1, y1, z1) = rig.bounds()

    # 贴地：最低点是爪尖，应 ≈0
    if not -0.05 <= y0 <= 0.55 * A.U:
        problems.append(f"贴地异常：最低点 y={y0:.2f}（应 0~{0.55 * A.U:.2f}，爪尖着地）")

    # 尺度对拍：实际 vs spec 目标
    total = z1 - z0
    for label, actual, target in (("全长", total, spec.total_len), ("站高", y1, spec.stand_h)):
        dev = abs(actual - target) / target
        if dev > 0.08:
            problems.append(f"{label}偏差 {dev * 100:.1f}%：实际 {actual:.1f} vs 目标 {target:.1f}")

    # 龙骨突不得穿地；折翼时指尖不得插地
    if A.keel_y < 0.5:
        problems.append(f"龙骨突下缘 y={A.keel_y:.2f} 触地（应留出腿的空间）")

    if verbose:
        print(f"[{spec.key}] {spec.cn} · {pose}")
        print(f"  骨骼 {len(rig.bones)} 根 · cube {len(rig.elements)} 个")
        print(f"  全长 {total:.1f} 单位 = {total / 16:.2f} m（目标 {spec.total_len / 16:.2f} m）"
              f" · 站高 {y1:.1f} = {y1 / 16:.2f} m（目标 {spec.stand_h / 16:.2f} m）")
        # 翼展只有展翼姿态量得出来；收翼时 x 跨度量到的是**体宽**，
        # 拿它当翼展报出来会得到"翼展 0.46 m 的秃鹫"这种荒唐数字。
        sp_rig, _ = build(spec, pose="spread")
        (sx0, _, _), (sx1, _, _) = sp_rig.bounds()
        span = (sx1 - sx0) / 16
        print(f"  体宽（收翼）{x1 - x0:.1f} 单位 = {(x1 - x0) / 16:.2f} m · "
              f"骨翼展（展翼）{sx1 - sx0:.1f} = {span:.2f} m（含飞羽约 {span / 0.65:.2f} m）")
        print(f"  最低点 y={y0:.2f} · 颈椎 {spec.cervicals} · 肋 {spec.ribs} 对 · "
              f"龙骨下缘 y={A.keel_y:.2f}")
        if problems:
            print(f"  ✗ {len(problems)} 处违例：")
            for p in problems[:12]:
                print(f"     {p}")
            if len(problems) > 12:
                print(f"     …另 {len(problems) - 12} 处")
        else:
            print("  ✓ 镜像 / 贴地 / 尺度 全部通过")
    return len(problems)


def main() -> int:
    ap = argparse.ArgumentParser(description="腐羽鹫骨架生成器")
    ap.add_argument("--size", choices=sorted(SPECS), help="只出单档（默认三档全出）")
    ap.add_argument("--pose", choices=("folded", "spread"), default="folded", help="收翼 / 展翼")
    ap.add_argument("--part", choices=sorted(PARTS), help="只生成单个部件（预览用）")
    ap.add_argument("--check", action="store_true", help="只跑结构自检，不写文件")
    ap.add_argument("--list", action="store_true", help="列出档位与部件")
    ap.add_argument("--out-dir", type=Path, help="输出目录（默认 modelScript/models/fuyu_vulture）")
    args = ap.parse_args()

    if args.list:
        print("档位：")
        for k in ("small", "mid", "large"):
            s = SPECS[k]
            print(f"  {k:6s} {s.cn}  全长 {s.total_len / 16:.2f} m · 站高 {s.stand_h / 16:.2f} m")
        print("部件：")
        for k, (label, _) in PARTS.items():
            print(f"  {k:9s} {label}")
        return 0

    keys = [args.size] if args.size else ["small", "mid", "large"]

    if args.check:
        bad = 0
        for k in keys:
            bad += check(SPECS[k], pose=args.pose)
            print()
        return 1 if bad else 0

    out_dir = args.out_dir or OUT_DIR
    out_dir.mkdir(parents=True, exist_ok=True)
    for k in keys:
        spec = SPECS[k]
        rig, A = build(spec, pose=args.pose, part=args.part)
        name = spec.model
        if args.part:
            name = f"{spec.model}_{args.part}"
        elif args.pose == "spread":
            name = f"{spec.model}_spread"
        out = out_dir / f"{name}.bbmodel"
        out.write_text(json.dumps(rig.bbmodel(name), ensure_ascii=False, indent=1))
        (lo, hi) = rig.bounds()
        print(f"→ {out.relative_to(REPO)}")
        print(f"   骨骼 {len(rig.bones)} · cube {len(rig.elements)} · "
              f"全长 {hi[2] - lo[2]:.1f} · 高 {hi[1]:.1f} · 宽 {hi[0] - lo[0]:.1f}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
