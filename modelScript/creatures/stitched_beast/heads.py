#!/usr/bin/env python3
"""异变缝合兽 —— 部件层·头颅：一颗捡来的头在这具身体上长成什么样。

肢体层回答的是"这条腿要扛住多少力"，头颅层回答的是另一个问题：**这颗头原来是拿来
吃什么的**。头骨是一台取食机器，它的每一处形状都是某个取食动作留下的痕迹；把这些
痕迹算出来，狼头就自己长成狼头，牛头就自己长成牛头，不需要一张"狼头长这样"的表。

**这一层最初的那句总纲是错的，先把它写在最前面。**

初版写的是：1 px = 6.25 cm，脸上的软组织只有 5–15 mm，连四分之一像素都不到，所以
"一颗头 = 它的头骨 + 它的咀嚼肌"，再没有第三样东西能被看见。据此我把毛、唇、鼻头、
眼睑全删了，只把颅骨、颧弓、下颌和常年露在外面的犬齿逐块涂色画出来。用户看了一眼就
问："你这是什么头颅？是头骨吗？"——是的，那就是头骨，是解剖标本的画法。

错在一个我没查的换算：**6.25 cm/px 是这只兽的世界尺度，不是头的尺度。** 头是被养大过
的（px_per_m = 模板长 ÷ 供体真实头长），按供体自己量，1 px 只值 0.6–4.6 cm。毛在十种
供体里有七种是 0.8–1.9 px——兔子的毛占它整颗头宽的三分之一。那句"看不见"错了 2.5–10 倍，
而且恰恰错在最需要它的那几种身上。

改过来之后这一层的总纲是：

    一颗头 = 它的头骨 + 它的咀嚼肌 + 绷在外面的那层皮毛

前两项决定**形状**，第三项决定**能不能读成一只活兽**。所以推导照旧全落在头骨与肌肉上
（凡是算出来的都直接是看得见的形状——推出"这块咬肌要 167 cm² 横截面"，那就是渲出来的
那块腮帮子），但渲染时整个头骨按体表厚度外扩一圈、统一涂成供体自己的皮毛，眼/鼻/唇/
角/露在外面的齿再长到毛的外面去。嘴默认是**闭着的**：活兽闭嘴时能看见的只有一道唇缝，
露在外面的齿是例外，而例外有明确条件（见 `_teeth`）。

推导链（每一步都能被下一步用上，不是各推各的）：

  ① **脑颅** ← 实测脑重。脑重按 M^0.74 涨，头骨长度按 M^0.33 涨，所以体型越大脑颅
     占脸的比例越小：鼠头 43% 是脑壳，牛头只有 20%——剩下 80% 全是脸。这一条就把
     "小兽头圆、大兽头长"分开了，不需要分别造型。
  ② **齿列 + 咬合面** ← 吃什么。剪切（食肉）是一条刃，磨（食草）是一整排臼齿，
     啃（啮齿）是一把凿子，吞（蛙）根本不切只是钳住。
  ③ **咬合力** = 食物断裂应力 × 咬合接触面积。注意方向：不是"想咬多大力"，是"把这口
     东西弄断需要多大应力乘多大面积"。于是牛的绝对咬合力比狼大——这是真的。
  ④ **颌关节高度** ← 咀嚼行程。下颌绕关节转 θ，关节高出咬合面 h，齿列就会横向滑
     θ·h。磨牙的一个冲程要扫过一颗臼齿的宽度 ⇒ h = 齿宽/θ。牛按这条算出来关节高出
     咬合面 11 cm，实测 10–12 cm。剪切类必须 h=0：滑动会把刃磨钝，而且关节只能受压。
  ⑤ **咀嚼肌** ← 咬合力 ÷ 力臂。力臂本身也是推的：颞肌臂 = 冠状突高（冠状突顶到脑颅
     顶，那是它能长到的极限），咬肌臂 = 关节到颧弓前根的一半。
  ⑥ **矢状嵴与颧弓外张** ← 肌肉截面要有地方装。这一步有个漂亮的极值解：要把截面积 A
     的肌肉装进"高 h、宽 t"的窝里，骨头长得越少越好 ⇒ min(嵴高 + 外张) s.t. h·t=A
     ⇒ **正方形的颞窝**，边长 √A。脑颅已经够高就不长嵴，不够就把差额长成嵴。狼有嵴、
     牛没有，是这条极值解算出来的，不是抄来的。
  ⑦ **眼位** ← 捕食还是被捕食。捕食者要在咬到的那一瞬间双眼都看得见猎物 ⇒ 重叠角
     = 2·atan(猎物半径 / 头长)；被捕食者要身后无盲区 ⇒ 单眼视野 + 2φ = 360°。狼算出
     50.6°、羊算出 82.5°，实测 ~50° 与 ~85°。
  ⑧ **耳** ← 散热与听觉取大者。散热面积 = 那一份代谢产热 / (对流系数 × 温差)；听觉
     口径 = 半波长。兔不会喘息散热，只能拿耳朵当散热片，于是耳朵大得离谱——那是热
     力学算出来的，不是兔子的造型特征。
  ⑨ **角** ← 一次对撞的冲量。F = m v²/(2s)，基部按抗弯解半径（直接复用肢体层的
     `bone_radius`）。**卷曲也是推的**：角质在基部成环生长，外侧比内侧长得快，比值 k
     固定 ⇒ 曲率 κ = (1−k)/d，而 d 向尖端收细 ⇒ 越靠尖卷得越紧 = 对数螺线。盘羊按这条
     算出来 1.2 圈，实测 1–1.25 圈。

**力学算的是比例，不是尺寸。** 这只兽把捡来的头养大了，所以绝对尺寸只由
`HEAD_TEMPLATES` 的长度 × scale 定；上面每一步都在**供体的真实尺度**上算，最后按
px_per_m = 模板长 / 供体真实头长 一次性换算。副产品是模板里的高和宽从此不是输入而是
**预测**——`report` 会把推出来的高宽和模板值并排列出来对拍。

用法:
  python3 modelScript/creatures/stitched_beast/heads.py            # 十种供体头颅的推导表
  python3 modelScript/creatures/stitched_beast/heads.py --seed 7   # 某一只兽的头
"""

from __future__ import annotations

import argparse
import math
import sys
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "core"))
sys.path.insert(0, str(HERE))

import core as C  # noqa: E402
import genome as GN  # noqa: E402
import limbs as LB  # noqa: E402
from bbmodel_maker.rig import voxel_rig as VR  # noqa: E402

PX = LB.PX
G = LB.G
SIGMA_BONE = LB.SIGMA_BONE
SIGMA_MUSCLE = LB.SIGMA_MUSCLE
SAFETY = LB.SAFETY

# ---------------------------------------------------------------- 物性常数
RHO_BRAIN = 1040.0         # 脑组织密度 kg/m³
BRAIN_FILL = 0.88          # 脑占脑腔的体积比（哺乳类；两栖类腔大脑小，见 DONOR）
SIGMA_KERATIN = 130e6      # 干角质（角/蹄/喙）抗弯强度 Pa
SIGMA_DENTIN = 200e6       # 牙本质抗弯强度 Pa —— 犬齿能长多长是它定的
U_KERATIN = 0.25e6         # 角质在工作应变下的储能密度 J/m³ —— 角是**溃缩区**不是梁，
                           # 见 horn_base_r
H_CONV = 15.0              # 体表自由对流换热系数 W/m²K（微风）
DT_SKIN = 8.0              # 耳表面与空气的温差 K
KLEIBER = 3.4              # 基础代谢 P = 3.4·M^0.75 W
V_NASAL = 10.0             # 鼻道内气流速度 m/s（安静呼吸 3–5，剧烈时 10–15）
VENT_PEAK = 12.0           # 剧烈活动时分钟通气量是静息的多少倍
SCROLL_PACK = 9.0          # 鼻甲卷占的截面 / 自由气道截面。嗅觉发达的兽鼻腔里几乎全是
                           # 卷曲的骨片，气流只从缝隙里过（狼的鼻腔横截面实测 15–20 cm²，
                           # 自由气道不到 2 cm²）

# 食物的断裂应力 Pa。**咬合力不是想咬多重，是把这口东西弄断需要的应力乘接触面积。**
FOOD_SIGMA: dict[str, float] = {
    "flesh": 1.0e6,      # 肌肉与筋膜横向撕裂
    "bone": 50e6,        # 骨的横向断裂（远低于抗压 170 MPa——骨是被掰断的不是压碎的）
    "grass": 3.0e6,      # 草叶维管束 + 硅质体
    "browse": 8.0e6,     # 木质嫩枝
    "seed": 35e6,        # 种壳
    "insect": 4.0e6,     # 甲壳
}

# ---------------------------------------------------------------- 观察值
# 下面这些是**量出来的**，和 limbs.SOFT_OVER_BONE 同类：它们不参与推导，是推导的输入。
# 每一条都写清楚量的是什么，别当成可以拧的旋钮。
TOOTH_W_RATIO = 0.035      # 臼齿宽 / 头骨全长。牛 20/600、羊 12/300、狼 8/280、鼠 2/45
CHEW_FRAC = 0.15           # 磨牙时同时受力的那一段占齿列的比例。食草兽**单侧咀嚼**，
                           # 一个冲程里真正咬合的只有两三颗牙，接触区还在齿列上来回扫
EAR_FILL = 0.7             # 耳廓面积 / 它的外接矩形。耳廓是圆角三角形不是方板
EAR_ASPECT = 1.7           # 耳廓长宽比。实测：狼 11/7、狐 9/6、兔 11/6、牛 25/12、
                           # 猪 20/15 —— 都在 1.3–2.1，取 1.7。初版写 2.6，耳朵窄到只有
                           # 1.8 px 宽而 4.8 px 长，渲出来是两根天线不是耳朵
JAW_ASPECT = 2.2           # 下颌体 深/宽
ARCH_MIN = 0.08            # 颧弓外张的下限 / 头长——再省也得让肌腱通过
ARCH_COST = 3.0            # 把头**加宽**一格相当于把矢状嵴**加高**几格。嵴是一片薄骨，
                           # 加宽却要连整条颧弓一起加粗——所以颞窝是竖长的不是方的
FRONTAL = 0.25             # 脑颅前缘到眶前缘那一段（额区）占**脸**的比例。脑腔只装脑，
                           # 而额区装的是颞窝、眶和鼻腔后段——它属于颅不属于吻。漏掉它
                           # （round 1）狼吻占了头长的 71%（实测 ~55%），头读成一根管
ARCH_REACH = 0.20          # 颧弓前根伸到脸的哪里（占脸长）。咬肌就挂在这一段下面，
                           # 它的力臂由此定——上一版误取"脑颅前缘"，力臂小了三倍，
                           # 反推出来的咬肌横截面 957 cm²，颧弓外张 31 px（头才 10 px 长）
FOV_PRED = 155.0           # 捕食者单眼水平视野（°，实测狼 150–160）
FOV_PREY = 195.0           # 被捕食者单眼水平视野（°，实测马 190–200；视网膜包得更靠后）
EYE_ALLO = 12.0e-3         # 眼轴长 = EYE_ALLO·M^0.18 m（狼 23 mm / 牛 38 mm 实测吻合）
NOCTURNAL_EYE = 1.35       # 夜行的眼相对同体型放大多少
HORN_STOP = 0.20           # 对撞时的减速行程 m（角 + 颅窦 + 颈 + 躯干一起让出来的）
HORN_SHEATH = 0.35         # 角鞘厚 / 骨心半径
RENDER_MIN = 0.5           # 渲染半径下限 px，同 gen_beast.RENDER_MIN_R：半像素以下的
                           # 柱子渲不出来。这是模型精度的界，不是解剖的界


@dataclass(frozen=True)
class Diet:
    """吃什么。**咬合模式是这一层的总开关**——它同时决定齿型、颌关节高度、肌肉分配。"""

    food: str          # 查 FOOD_SIGMA
    occlusion: str     # shear 剪 / grind 磨 / gnaw 啃 / crush 压 / gulp 吞
    predator: bool
    prey_frac: float   # 一口猎物 / 自身体重——决定张口度，也决定捕食者要多大的重叠视野
    wear: float        # 齿冠磨耗速率 mm/yr（草里的硅质体最磨牙）
    diastema: float    # 门齿与臼齿之间的空档 / 头长。食草兽要一边在地面啃一边在后面磨，
                       # 中间那段就空出来了；食肉兽的齿列是连续的一条刃
    temporalis: float  # 颞肌分担的比例（其余归咬肌）。颞肌向后上拉——适合剪；咬肌向
                       # 前上拉并能出横向分量——磨牙全靠它
    chew_deg: float    # 一个咀嚼冲程的开合角（°）
    contact: float     # 咬合接触面积 / 臼齿宽²。牙**把力集中到刃和棱上**，接触面从来
                       # 不是整个牙冠的平面面积；实测口径见 bite_force
    palate: float      # 腭宽 / 臼齿宽。磨的那一类要给舌头留出翻食团的地方，腭就宽


DIETS: dict[str, Diet] = {
    "carnivore":  Diet("bone", "shear", True, 0.30, 0.15, 0.00, 0.70, 22.0, 0.25, 1.0),
    "omnivore":   Diet("seed", "crush", False, 0.05, 0.40, 0.05, 0.45, 18.0, 0.25, 1.2),
    "grazer":     Diet("grass", "grind", False, 0.00, 3.00, 0.22, 0.25, 10.0, 0.0, 2.0),
    "browser":    Diet("browse", "grind", False, 0.00, 1.00, 0.20, 0.30, 12.0, 0.0, 2.0),
    "gnawer":     Diet("seed", "gnaw", False, 0.02, 0.50, 0.30, 0.30, 14.0, 0.20, 1.0),
    "insectivore": Diet("insect", "gulp", True, 0.10, 0.20, 0.00, 0.50, 40.0, 0.30, 1.4),
}


@dataclass(frozen=True)
class Donor:
    """一个供体物种。全是**观察值**——这颗头被捡来之前是谁。

    `brain_g` 用实测脑重而不是异速式：牛的脑比哺乳类均值小一半以上，套均值算出来的
    牛脑颅会大出三成，脸跟着短掉，牛头就读成了鹿头。
    """

    mass: float        # kg 成体体重
    head_m: float      # m 头骨全长（枕髁到门齿）
    brain_g: float     # g 脑重
    lifespan: float    # yr 寿命——角长多长、齿冠磨掉多少，都是"长了多少年"
    diet: str
    ear_share: float   # 代谢产热里由耳廓散掉的那一份。会喘息/出汗/打滚的低，都不会的高
    coat: str = "fur"  # 体表：fur 毛 / wool 羊毛 / bristle 鬃 / plume 羽 / hide 裸皮
    coat_cm: float = 0.0   # **头上那层毛有多厚（cm，观察值）**，见 coat_px 的注释。
                           # 注意量的是**头**不是身体：头恰恰是大多数兽毛最短的地方
                           # （兔颈背 2.5 cm，脸上只有 1.0；狼颈鬃 6–8，吻上 1.5）。
                           # 拿体毛顶替会让兔头凭空高出三分之二
    nocturnal: bool = False
    horn: str = ""     # "" / ram 盘角 / hook 弯角 / spike 直角
    pinna: bool = True  # 有没有外耳廓（禽类与两栖类没有）
    beak: bool = False  # 无齿、靠角质喙钳
    brain_fill: float = BRAIN_FILL


DONOR: dict[str, Donor] = {
    # 食肉：剪切
    "wolf":    Donor(40.0, 0.280, 150.0, 12.0, "carnivore", 0.05, "fur", 1.5),
    "fox":     Donor(6.0, 0.150, 50.0, 8.0, "carnivore", 0.12, "fur", 1.5, nocturnal=True),
    # 食草：磨
    "cow":     Donor(600.0, 0.600, 450.0, 20.0, "grazer", 0.03, "hide", 0.8, horn="hook"),
    "sheep":   Donor(70.0, 0.300, 130.0, 12.0, "grazer", 0.06, "wool", 2.5, horn="ram"),
    "goat":    Donor(60.0, 0.270, 130.0, 15.0, "browser", 0.06, "fur", 2.0, horn="spike"),
    "rabbit":  Donor(2.0, 0.085, 11.0, 9.0, "browser", 0.35, "fur", 1.0, nocturnal=True),
    # 杂食：压
    "pig":     Donor(90.0, 0.400, 180.0, 15.0, "omnivore", 0.15, "bristle", 0.2),
    # 鼠的散热主要走**尾**不走耳（无毛、血管密），所以 ear_share 低——它的耳朵才不会
    # 被算成兔耳。这一条是观察，不是为了凑形状
    "rat":     Donor(0.35, 0.045, 2.0, 3.0, "gnawer", 0.10, "fur", 0.3, nocturnal=True),
    # 禽：喙，无耳廓
    "chicken": Donor(2.5, 0.080, 4.0, 8.0, "omnivore", 0.10, "plume", 1.0, pinna=False, beak=True),
    # 两栖：吞，无耳廓（只有鼓膜），脑腔松
    "frog":    Donor(0.15, 0.050, 0.10, 8.0, "insectivore", 0.02, "hide", 0.0,
                     pinna=False, brain_fill=0.45),
}

# 角的打法。**这是观察**：盘羊正面对撞、山羊立起来往下劈、牛用角根往前顶并挑。
# 打法定三样东西：撞击速度、角伸出去的方向、以及卷曲平面。
@dataclass(frozen=True)
class HornStyle:
    speed: float       # 单侧冲撞速度 m/s
    rate: float        # 角的生长速率 m/yr（沿弧长）
    inner: float       # 内侧/外侧生长速率之比 k —— 卷曲的唯一来源
    out_deg: float     # 基部朝外张开多少度
    back_deg: float    # 基部朝后仰多少度
    hit_frac: float    # 撞击点在弧长的哪一处（力臂按基部到该点的**弦长**算，不是弧长）


# 生长速率是**成年后的**速率（实测：公盘羊角弧长 0.75–0.9 m / 8 年；山羊 0.3–0.5 m /
# 9 年；牛 0.2–0.3 m / 12 年）。round 1 给牛写了 0.05 m/yr，算出 0.6 m 的角——13 px 的
# 头上伸出 13 px 的角，两边张开比整颗头还宽，渲出来是一对机翼。
# 卷曲的转轴（右侧那只；左侧按 (a_x, −a_y, −a_z) 镜像）。
HORN_AXIS: dict[str, tuple[float, float, float]] = {
    "ram":   (1.0, 0.0, 0.25),
    "spike": (1.0, 0.0, 0.0),
    "hook":  (0.0, 1.0, 0.0),
}

HORN_STYLE: dict[str, HornStyle] = {
    "ram":   HornStyle(4.5, 0.100, 0.55, 28.0, 10.0, 0.35),
    "spike": HornStyle(3.0, 0.045, 0.88, 12.0, 55.0, 0.60),
    "hook":  HornStyle(2.0, 0.022, 0.95, 62.0, 5.0, 0.55),
}

# 体表材质直接借肢体层那套（`limbs.LIMB_MATS` 的 fur / wool / bristle / plume / hide）
# ——同一个供体的头和腿必须是同一张皮，各起一套颜色就等于告诉玩家这是两只兽的零件。
COAT_MAT = {"fur": "fur", "wool": "wool", "bristle": "bristle",
            "plume": "plume", "hide": "hide"}

# 这些是**露在毛外面的特征**：不按体表厚度外扩、也不涂成体表材质。其余每一块都属于头的
# 肉身。按名字判而不是逐块标记——加一块新几何时忘记标记的默认后果是"它被毛盖住"，
# 那比"它凭空露出一块骨头"安全。
FEATURE_PARTS = ("eye", "sclera", "canine", "incisor", "tusk", "dental_pad", "horn",
                 "beak", "nostril", "rhinarium", "rooting_disc", "tympan", "conch",
                 "collar", "lip")

HEAD_MATS: dict[str, tuple[int, int, int]] = {
    "skull": (108, 96, 84),      # 头骨外面那层薄皮：比躯干 hide 亮，因为绷在骨上
    "jowl": (114, 90, 78),       # 咀嚼肌鼓出来的那块腮——它是算出来的，不是画上去的。
                                 # 颜色只比 skull 暗一档：**肌肉本身是看不见的**，看见的
                                 # 是绷在它上面的同一张皮，差别只在形状。round 1 给了它
                                 # 一个明显偏红的色，两块肌肉读成贴在头两侧的粉色垫子
    "muzzle": (74, 62, 56),      # 鼻端/唇周的裸皮
    "tooth": (198, 190, 168),    # 齿：全头最亮
    "horn": (92, 84, 70),        # 角质角
    "beak": (150, 128, 84),      # 喙
    "eye": (24, 22, 26),         # 眼：湿的暗
    "sclera": (146, 138, 126),   # 巩膜——只有侧眼的猎物才露得出来
    "ear_in": (128, 92, 90),     # 耳廓内面
    "tympan": (96, 88, 72),      # 蛙的鼓膜
    "nostril": (40, 34, 32),
    "lip": (52, 42, 40),         # 唇线：闭着的嘴在这个分辨率下就是一道暗缝
}

FWD = LB.FWD
UP = LB.UP


# ---------------------------------------------------------------- 小工具
def _n(v: np.ndarray) -> np.ndarray:
    m = float(np.linalg.norm(v))
    return v / m if m > 1e-9 else v


def sphere_r(mass_kg: float, rho: float = 1000.0) -> float:
    """一团 mass_kg 的肉揉成球，半径多少（m）。猎物半径、一口吞多大都用它。"""
    return (3.0 * mass_kg / (4.0 * math.pi * rho)) ** (1.0 / 3.0)


def beam_depth(M: float, width: float, sigma: float = SIGMA_BONE) -> float:
    """矩形截面梁在弯矩 M 下需要的深度：σ = 6M/(w·h²)。下颌体就是这样一根梁。"""
    if M <= 0.0 or width <= 0.0:
        return 0.0
    return math.sqrt(6.0 * M * SAFETY / (sigma * width))


@dataclass(frozen=True)
class Piece:
    """一块渲染基元，坐标是**头局部** px：(r 右, u 上, f 前)。

    a→b 是长轴，`r1` 是沿 e_r（水平）的半宽，`r2` 是剩下那个垂直方向的半厚。用长轴
    表达而不是"中心+半尺寸"，是因为渲染要走 `Rig.shaft`：它把 a→b 解成 pitch/yaw，
    roll 恒为 0，而头的标架正好是零 roll 的（e_r 由 e_f 与世界上方叉出来，必然水平）。
    """

    part: str      # 归哪根骨：skull / jaw / ear_l / ear_r / horn_l / horn_r
    name: str
    a: tuple[float, float, float]
    b: tuple[float, float, float]
    r1: float
    r2: float
    mat: str
    soft: bool = False   # **算出来比渲染下限还薄就整块不画**，而不是撑到下限。
                         # 撑是撒谎：鸡的颊肌算出来单侧只有 0.23 px，撑到 0.5 px 之后两侧
                         # 加起来占掉 2.9 px 宽的头的七成——推导说"这只兽的腮不鼓"，画出来
                         # 却是满脸腮。结构件（脑颅/脸/下颌/癒合环/眼/耳/角/喙）不适用：
                         # 它们缺了就不成其为头，那才是真正该被下限托住的东西


@dataclass
class Head:
    gene: GN.HeadGene
    sock: C.Socket
    donor: Donor
    diet: Diet

    org: np.ndarray                 # 枕髁（头与颈的交界）世界坐标
    e_r: np.ndarray                 # 右（水平）
    e_u: np.ndarray                 # 上
    e_f: np.ndarray                 # 前（= 挂载点法向：头是从这个方向长出去的）
    px_m: float                     # px per m（供体真实尺度 → 模型 px）

    L: float                        # 头长 px（模板 × scale，唯一的尺寸输入）
    pieces: list[Piece] = field(default_factory=list)

    # —— 推导出来的量（报表 + 自检都读它们）
    brain_px: tuple[float, float, float] = (0.0, 0.0, 0.0)   # 脑颅 长/高/宽 px
    bite_N: float = 0.0             # 咬合力 N
    bite_px: float = 0.0            # 咬合点在局部 f 上的位置
    tmj: tuple[float, float] = (0.0, 0.0)   # 颌关节 (u, f) px
    occ: float = 0.0                # 咬合面高度 u px（齿列所在的那一层）
    pcsa: tuple[float, float] = (0.0, 0.0)  # (颞肌, 咬肌) 生理横截面 m²
    crest: float = 0.0              # 矢状嵴高 px
    arch: float = 0.0               # 颧弓外张（单侧半宽）px
    face_w: float = 0.0             # 脸宽（两条齿列 + 腭）px
    jaw_depth: float = 0.0          # 下颌体深 px
    phi: float = 0.0                # 眼的方位角（离中线，°）
    overlap: float = 0.0            # 双眼重叠 °
    blind: float = 0.0              # 身后盲区 °
    eye_r: float = 0.0              # 眼球半径 px
    ear_plate: tuple[float, float] = (0.0, 0.0)   # 耳廓 (长, 宽) px
    horn_len: float = 0.0           # 角的弧长 px
    horn_turn: float = 0.0          # 角卷过的总角度 °
    horn_r: float = 0.0             # 角基半径 px（按储能定的）
    horn_bend: float = 0.0          # 同一只角只按抗弯要多粗 px——报表里和上面对拍
    pred_H: float = 0.0             # 推出来的头高 px（模板里的高是预测不是输入）
    pred_W: float = 0.0             # 推出来的头宽 px
    gape: float = 0.0               # 最大张口角 °

    @property
    def name(self) -> str:
        return self.sock.name

    @property
    def kind(self) -> str:
        return self.gene.kind

    def world(self, p) -> np.ndarray:
        """局部 (r,u,f) px → 世界坐标。"""
        return self.org + self.e_r * p[0] + self.e_u * p[1] + self.e_f * p[2]

    def eye_dirs(self) -> list[np.ndarray]:
        """两只眼的**视轴**世界方向。视野覆盖、被自己身体挡住多少都靠它。"""
        out = []
        for sgn in (-1.0, 1.0):
            a = math.radians(self.phi)
            e = math.radians(self.eye_el)
            d = (self.e_f * math.cos(a) * math.cos(e)
                 + self.e_r * sgn * math.sin(a) * math.cos(e)
                 + self.e_u * math.sin(e))
            out.append(_n(d))
        return out

    eye_el: float = 0.0             # 眼的仰角 °（蛙的眼长在头顶）
    eye_pos: list = field(default_factory=list)   # 两只眼的局部坐标
    dropped: list = field(default_factory=list)   # 薄过渲染下限、整块没画的东西（见 Piece.soft）
    standoff: float = 0.0           # 枕髁离表皮多远 px —— 融合基座的高度，见 _standoff


# ---------------------------------------------------------------- 标架
def head_frame(sock: C.Socket, *, aim: np.ndarray | None = None
               ) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    """头的朝向。**轴由它长出来的地方定，滚转由重力定。**

    e_f 直接取挂载点法向——头是从那个方向长出去的，没有颈子去把它扳回来（这只兽的头
    是直接缝在核心上的）。所以背上那颗头就是朝天长的，胸口那颗就是朝地长的，正典要的
    就是这个不合理。

    但**滚转不是自由的**：下颌靠重力垂着、眼睛要看平，所以头会绕自己的长轴转到"上"
    尽量朝天。于是 e_u = 世界上方去掉沿 e_f 的分量。这一步还有个副作用是渲染要的：
    这样定出来的 e_r 必然水平，而 `Rig.shaft` 解出来的方块 roll 恒为 0、宽度轴恒水平
    ——两者正好对上，头上的每一块都能用 shaft 表达。
    """
    # `aim` 是被邻居顶开之后的朝向（见 separate）；没有邻居冲突时它就是法向本身
    e_f = _n(np.asarray(aim if aim is not None else sock.normal, float))
    ref = UP
    if abs(float(ref @ e_f)) > 0.999:          # 正朝天或正朝地：拿前后向当参考
        ref = FWD
    e_u = _n(ref - float(ref @ e_f) * e_f)
    e_r = _n(np.cross(e_f, e_u))
    return e_r, e_u, e_f


# ---------------------------------------------------------------- 推导
def brain_case(d: Donor) -> tuple[float, float, float]:
    """脑颅的 (长, 高, 宽)，单位 m。

    椭球，三轴比 1.30 : 0.95 : 1.00（长 : 高 : 宽，哺乳类脑颅实测）。体积由实测脑重
    除以脑组织密度再除以充填率得到。

    这一条推导单独就把"小兽头圆、大兽头长"分开了：脑重按 M^0.74 涨而头骨长按 M^0.33
    涨，所以脑颅占头长的比例随体型下降。实测口径：鼠 43%、兔 40%、狼 29%、猪 22%、
    牛 20%——剩下的全是脸。
    """
    v = (d.brain_g * 1e-3 / RHO_BRAIN) / d.brain_fill
    k = (v / ((4.0 / 3.0) * math.pi * 1.30 * 0.95 * 1.00)) ** (1.0 / 3.0)
    return 2.60 * k, 1.90 * k, 2.00 * k


def tooth_row(d: Donor, diet: Diet) -> float:
    """颊齿列的长度 m。脸的四成多是齿列，扣掉门齿与臼齿之间的空档。"""
    return d.head_m * (1.0 - diet.diastema) * 0.42


def bite_force(d: Donor, diet: Diet) -> tuple[float, float]:
    """(咬合力 N, 咬合点到枕髁的距离 m)。

    力 = 食物断裂应力 × 咬合接触面积。**方向是这个方向**：不是"想咬多大力"，是"把这口
    东西弄断需要多大应力乘多大面积"。

    接触面积不是牙冠的平面面积——牙的全部本事就是**把力集中到刃和棱上**。剪切的刃、
    啃咬的凿口、压碎时压进种壳的那一小块，量出来都落在臼齿宽的平方的 0.2–0.3 倍。
    所以真正把食肉与食草分开的**不是接触形状而是同时咬几颗牙**：

      · 剪 / 压 / 啃 / 吞：永远只有**一颗**牙（或一条刃）在受力，面积 ~0.25 w²。
      · 磨：**一整段齿列**同时受力。食草兽单侧咀嚼，一个冲程里两三颗牙咬合，
        面积 = CHEW_FRAC × 齿列长 × 齿宽 —— 比单颗大一到两个数量级。

    于是牛的绝对咬合力比狼大（实测牛 2–3 kN、狼 1.2–1.5 kN），而狼在裂齿那条**刃**上
    的应力远高于牛——两件事同时成立，这正是"力"和"应力"的区别。
    """
    w = TOOTH_W_RATIO * d.head_m
    sig = FOOD_SIGMA[diet.food]
    if diet.occlusion == "grind":
        area = CHEW_FRAC * tooth_row(d, diet) * w
        at = d.head_m * 0.55
    else:
        area = diet.contact * w * w
        at = {"shear": 0.42, "crush": 0.50, "gnaw": 0.97, "gulp": 0.95}[diet.occlusion]
        at *= d.head_m
    return sig * area, at


def nasal_side(d: Donor) -> float:
    """鼻腔的边长 m（截面取方，宽与深同一个数）。**大兽的长脸里装的是鼻子，不是牙。**

    自由气道截面 = 剧烈活动时的分钟通气量 / 气流速度；而嗅觉发达的兽鼻腔里几乎全是
    卷曲的鼻甲骨片，气流只从缝里过 ⇒ 整个鼻腔截面 = 自由气道 × SCROLL_PACK。

    **宽和深必须一起由它定，不能只定深。** round 1 拿"截面 ÷ 脸宽"当深度、脸宽另由齿列
    定，于是狼的吻只有 1.16 px 宽而 2.6 px 深——渲出来是一块竖着的板，不是口鼻。齿列
    只是宽度的**下限**（两条齿列加中间的腭总得放得下），真正撑开吻的是鼻腔。

    狼算出边长 4.2 cm、牛 12.3 cm，实测狼吻宽 4–5 cm、牛 ~12 cm。
    """
    vent = 0.5 * d.mass ** 0.8 / 60.0 / 1000.0 * VENT_PEAK   # m³/s
    return math.sqrt(vent / V_NASAL * SCROLL_PACK)


def tmj_lift(d: Donor, diet: Diet) -> float:
    """颌关节高出咬合面多少（m）。**这是磨与剪的分水岭。**

    下颌绕关节转 θ 时，齿列除了张开还会沿前后向滑 θ·h（h = 关节高出咬合面的距离）。
    磨牙的一个冲程要扫过一整颗臼齿的宽度，于是 h = 齿宽 / θ。牛按这条算出来 11 cm，
    实测 10–12 cm。

    剪切类必须 h = 0：滑动会把刃磨钝，而且关节在剪切时只能受压不能被撬开。所以食肉兽
    的颌关节正好落在齿列平面上——这也是它张不开多大、却能在裂齿上出巨力的原因。

    啃（啮齿）是第三种：滑动方向是**前后**而不是横向，关节因此不抬高，改成一条前后向
    的长槽。这里只体现"不抬高"。
    """
    if diet.occlusion in ("shear", "gnaw", "gulp"):
        return 0.0
    w = TOOTH_W_RATIO * d.head_m
    return w / math.radians(diet.chew_deg)


def eye_geom(d: Donor, diet: Diet) -> tuple[float, float, float, float, float]:
    """(眼球半径 m, 方位角 φ°, 双眼重叠°, 身后盲区°, 仰角°)。

    **捕食者**：咬中的那一瞬间猎物就在吻端，双眼都得看得见它 ⇒ 重叠角
    = 2·atan(猎物半径 / 头长)。猎物半径由"一口吃多大"反推（猎物质量 = prey_frac ×
    自身体重，揉成球）。狼算出重叠 54°、眼离中线 50.6°，实测 ~50°。

    **被捕食者**：目标不是看清而是身后不留盲区 ⇒ 单眼视野 + 2φ = 360°。羊算出 82.5°，
    连带前方还剩 30° 重叠——实测食草兽正前方确实还有约 30° 的双眼区。

    单眼视野本身两类不同（155° vs 195°）：被捕食者的视网膜包得更靠后。这是观察值。

    **蛙是第三种**：它伏在水里等，只把眼睛露出水面 ⇒ 眼长在颅顶（仰角 70°）。这不是
    造型，是"身体在水下、眼睛要在水上"这一条约束的唯一解。
    """
    axial = EYE_ALLO * d.mass ** 0.18 * (NOCTURNAL_EYE if d.nocturnal else 1.0)
    if diet.predator:
        r_prey = sphere_r(max(diet.prey_frac, 1e-6) * d.mass)
        ov = 2.0 * math.degrees(math.atan2(r_prey, d.head_m))
        ov = min(max(ov, 20.0), 120.0)
        phi = max((FOV_PRED - ov) / 2.0, 5.0)
        fov = FOV_PRED
    else:
        fov = FOV_PREY
        phi = min((360.0 - fov) / 2.0, 95.0)
        ov = max(fov - 2.0 * phi, 0.0)
    blind = max(360.0 - (fov + 2.0 * phi), 0.0)
    el = 70.0 if diet.occlusion == "gulp" else 8.0
    return axial / 2.0, phi, ov, blind, el


def ear_plate(d: Donor) -> tuple[float, float]:
    """耳廓 (长, 宽) m。散热与听觉**取大者**——同 `bone_radius` 取失效判据上界的做法。

    · **散热**：代谢产热 P = 3.4·M^0.75 W，其中由耳廓散掉的那一份 ear_share 要靠
      对流带走 ⇒ 双耳总表面积 = share·P/(h·ΔT)。耳廓两面都散热，所以单面板面积再折半。
      兔子不会喘息、不出汗、只能拿耳朵当散热片（share 0.35），于是耳朵大得离谱：这条
      算出来 10.4 × 4.0 cm，实测家兔 11 × 6 cm。**兔耳大是热力学，不是造型特征。**
    · **听觉**只定形状与朝向，不定面积——这是算完才知道的：十种供体里没有一种是被
      "拢声漏斗"的下限卡住的，全部由散热定。所以耳朵大小是热力学问题，听觉是几何问题。

    禽类与两栖类没有外耳廓，返回 (0, 0)——它们的听觉走别的解（鼓膜直接贴在体表）。
    """
    if not d.pinna:
        return 0.0, 0.0
    p = KLEIBER * d.mass ** 0.75
    a = d.ear_share * p / (H_CONV * DT_SKIN) / 4.0   # 双耳 → 单耳 → 单面
    a = max(a, (0.25 * d.head_m) ** 2 / EAR_ASPECT)   # 再小也得是个能拢声的漏斗
    # 耳廓是个**圆角三角形**，不是矩形：同样的面积要摊得更长。实测填充率约 0.7（外接
    # 矩形的七成），狼按这条算出 11.6 cm，实测 11 cm；按矩形算只有 9.8 cm。
    h = math.sqrt(a * EAR_ASPECT / EAR_FILL)
    return h, h / EAR_ASPECT


def horn_axis(d: Donor, st: HornStyle) -> tuple[list[tuple[float, float]], float, float]:
    """角的中轴：[(弧长 s, 局部直径 d)]、总弧长、总转角（rad）。

    **卷曲是推出来的，不是画出来的。** 角质在基部成环沉积；外侧比内侧长得快，比值 k
    固定，于是中轴的曲率 κ = (1−k)/d。而 d 随年龄向尖端收细 ⇒ 越靠尖端卷得越紧，
    这正是对数螺线。盘羊按这条算出来转 7.6 rad ≈ 1.2 圈，实测公盘羊 1–1.25 圈。

    收细的原因也不用另找：角环一年一道，基部周长随年龄增长，所以**先长的那一段（尖）
    最细**。这里取尖端收到基部的 15%。
    """
    age = 0.6 * d.lifespan
    s_tot = st.rate * age
    d0 = 2.0 * horn_base_r(d, st)
    pts, turn = [], 0.0
    n = 24
    for i in range(n + 1):
        u = i / n
        dd = d0 * (1.0 - 0.85 * u)
        pts.append((s_tot * u, dd))
        if i:
            turn += (1.0 - st.inner) / dd * (s_tot / n)
    return pts, s_tot, turn


def coat_px(hd: "Head") -> float:
    """体表那一层有多厚（px）。**这一条是量出来的，不是推的**，得说清楚为什么。

    先按热平衡推过一遍：毛要挡住的是代谢产热往外漏，d = k·ΔT·体表面积 / 产热，
    取 k=0.04 W/mK（毛里滞留的空气）、ΔT=25 K、体表面积按 Meeh 式 0.1·M^0.65。
    狼解出 2.0 cm（实测 3）、兔 2.7 cm（实测 2.5）——这两个对得上。但**鼠解出 3.2 cm
    而实测只有 0.5**，牛解出 7.7 cm 而实测 1.2。两头都错六倍，方向还相反：
    小兽根本不靠毛保温（钻洞 + 抬高单位质量代谢），大兽的问题是散热不是保温。也就是说
    毛厚由**气候与行为**主导，不由这一条平衡式主导，硬推等于给自己编一个假的因果。

    所以这一列走观察值（`Donor.coat_cm`）。

    换算这里有个我自己踩过的坑：**1 px = 6.25 cm 是这只兽的世界尺度，不是头的尺度。**
    头是被养大过的（px_per_m = 模板长 / 供体真实头长），按供体自己量，1 px 只值
    0.6–4.6 cm。于是毛在十种供体里有七种是 0.8–1.9 px——兔子的毛占它整颗头宽的三分之一。
    初版按 6.25 cm 判成"连四分之一像素都不到"，据此把毛、唇、鼻头、眼睑全删了，
    只把颅骨、颧弓、下颌、犬齿逐块涂色画出来：那是解剖标本，不是活物。
    """
    return hd.donor.coat_cm / 100.0 * hd.px_m


def horn_impact(d: Donor, st: HornStyle) -> tuple[float, float]:
    """一次对撞的 (动能 J, 峰值力 N)。峰值力按 HORN_STOP 的减速行程折算（角变形 +
    颅窦 + 颈 + 躯干一起让出来的那一段）。公羊 70 kg / 4.5 m/s / 0.20 m 算出 3.5 kN，
    实测公羊对撞峰值 ~3.4 kN。"""
    ke = 0.5 * d.mass * st.speed ** 2
    return ke, d.mass * st.speed ** 2 / (2.0 * HORN_STOP)


def horn_base_r(d: Donor, st: HornStyle) -> float:
    """角基半径 m。**角不是梁，是溃缩区**——这一条是算出来才知道的。

    先按抗弯算了一遍：公羊 3.5 kN、力臂 0.19 m，解出基部半径 2.2 cm，代回去应力只有
    19 MPa，是角质强度的七分之一。也就是说**按"别掰断"来设计，角可以细得像根筷子**。
    可实测公羊角基直径 10 cm 以上。所以掰断根本不是它要防的失效模式。

    真正的约束是**能量**：对撞的动能必须在颅骨之前被吃掉，否则脑子先受不了。角是一段
    工作在弹性范围内的角质，能存的能量 = 体积 × 储能密度 ⇒ 体积 = 那一半动能 / 储能
    密度。再配上"长度由生长速率 × 年龄定、直径向尖端线性收到 15%"，基部直径就唯一确定：

        V = ∫π(d/2)²ds = (π/4)·d₀²·S·0.391  ⇒  d₀ = √(4V / (π·0.391·S))

    公羊解出 8.0 cm、山羊 5.7 cm、牛 11.4 cm，实测 10 / 5 / 9–12 cm。**抗弯那条判据
    在这里从头到尾都不是瓶颈**，报表里把两者并排列出来，差三倍以上。
    """
    ke, _f = horn_impact(d, st)
    vol = 0.5 * ke / U_KERATIN            # 一半动能给角，另一半给颈与躯干
    s_tot = st.rate * 0.6 * d.lifespan
    d0 = math.sqrt(4.0 * vol / (math.pi * 0.391 * max(s_tot, 1e-6)))
    return d0 / 2.0


def horn_bend_r(d: Donor, st: HornStyle) -> float:
    """同一只角**只按抗弯**要多粗（m）。只用来在报表里和上面那条对拍。"""
    _ke, f = horn_impact(d, st)
    arc = st.rate * 0.6 * d.lifespan * st.hit_frac
    chord = arc * 0.75                     # 弯着的一段，弦 ≈ 弧的四分之三
    r, _ = LB.bone_radius(chord / PX, chord / PX, f)
    return r * PX


# ---------------------------------------------------------------- 装配
# 线段最短距离在公共底座里（`voxel_rig.seg_dist`）——肢体层的姿态求解也用同一份。
seg_dist = VR.seg_dist


def head_capsules(hd: "Head"):
    """这颗头渲染出来的每一块，近似成胶囊（轴 + 半径）。半径取两个半尺寸的几何平均。"""
    return [(hd.world(p.a), hd.world(p.b),
             math.sqrt(max(p.r1, 1e-3) * max(p.r2, 1e-3)), p.name)
            for p in hd.pieces]


def head_overlap(a: "Head", b: "Head"):
    """两颗头之间最深的互穿：(深度 px, 接触点)。"""
    worst, at = 0.0, None
    for (a0, a1, ra, _an) in head_capsules(a):
        for (b0, b1, rb, _bn) in head_capsules(b):
            dd, mid = seg_dist(a0, a1, b0, b1)
            if ra + rb - dd > worst:
                worst, at = ra + rb - dd, mid
    return worst, at


def separate(heads: dict[str, "Head"], *, rounds: int = 12,
             max_deg: float = 40.0) -> dict[str, "Head"]:
    """**挤在一起的两颗头互相把对方顶开。**

    朝向本来取挂载点法向（头是从那儿长出去的）。但两颗头挨着长的时候，法向并不保证
    它们不打架——实测三头的个体里，两块颞肌互穿 1.18 px、一只耳朵插进隔壁的脑颅。

    顶开是有依据的，不是为了好看：**两团同时在长的组织彼此挤压，只能朝还有空的那边
    长**。所以让每颗头的朝向沿"背离对方"的方向偏一点，反复几轮直到不再互穿。偏转量
    封顶 `max_deg`——超过这个角度就不是"挤开"而是"长到别处去了"，那种情况保留互穿并
    交给自检报出来，说明这个基因组本来就该换槽。
    """
    names = sorted(heads)
    aim = {n: np.asarray(heads[n].e_f, float) for n in names}
    base = {n: _n(np.asarray(heads[n].sock.normal, float)) for n in names}
    for _ in range(rounds):
        push = {n: np.zeros(3) for n in names}
        hot = False
        for i, na in enumerate(names):
            for nb in names[i + 1:]:
                ov, at = head_overlap(heads[na], heads[nb])
                if ov <= 0.25:
                    continue
                hot = True
                # 顶开的方向取**接触点**相对各自枕髁的方位，不是两个枕髁的连线：打架的
                # 常常是伸出去的角和耳，沿枕髁连线推根本推不到它们身上（实测一只耳插进
                # 隔壁的角里，按枕髁连线推了十二轮还剩 1.42 px）。
                for me, other in ((na, nb), (nb, na)):
                    v = heads[me].org - at
                    nn = float(np.linalg.norm(v))
                    v = v / nn if nn > 1e-6 else base[me]
                    push[me] += v * min(ov, 3.0) * 0.16
        if not hot:
            break
        for n in names:
            if not push[n].any():
                continue
            v = _n(aim[n] + push[n])
            # 封顶：偏离挂载点法向不得超过 max_deg
            c = float(np.clip(v @ base[n], -1.0, 1.0))
            lim = math.cos(math.radians(max_deg))
            if c < lim:
                perp = _n(v - c * base[n])
                v = _n(base[n] * lim + perp * math.sqrt(max(1.0 - lim * lim, 0.0)))
            aim[n] = v
            heads[n] = solve_head(heads[n].gene, heads[n].sock, aim=v)
    return heads


def solve_head(gene: GN.HeadGene, sock: C.Socket, *,
               aim: np.ndarray | None = None) -> Head:
    """把上面每一步串起来，长出一颗头。

    顺序不能乱：脑颅 → 齿列 → 咬合力 → 颌关节 → 肌肉 → 骨头往哪长 → 眼耳角。后面每
    一步都要用前面的结果，反过来没有一步回头改前面的——所以整条链没有迭代，同一个
    供体永远得同一颗头。
    """
    d = DONOR[gene.kind]
    diet = DIETS[d.diet]
    e_r, e_u, e_f = head_frame(sock, aim=aim)
    L_px = GN.HEAD_TEMPLATES[gene.kind][0] * gene.scale
    px_m = L_px / d.head_m
    hd = Head(gene=gene, sock=sock, donor=d, diet=diet,
              org=np.asarray(sock.pos, float), e_r=e_r, e_u=e_u, e_f=e_f,
              px_m=px_m, L=L_px)

    def P(x: float) -> float:
        return x * px_m

    # ① 脑颅 -------------------------------------------------------------
    bl, bh, bw = brain_case(d)
    hd.brain_px = (P(bl), P(bh), P(bw))
    f_brain = P(bl)                       # 脑颅前缘（局部 f）
    u_top = P(bh) / 2.0                   # 颅顶
    u_bot = -P(bh) / 2.0                  # 颅底

    # ② 咬合面与咬合力 ----------------------------------------------------
    bite_N, bite_at = bite_force(d, diet)
    hd.bite_N = bite_N
    hd.bite_px = P(bite_at)

    # 脸的宽度：**鼻腔撑开的**，齿列只是下限。两条齿列加中间的腭总得放得下（磨的那一类
    # 要给舌头留出翻食团的地方，腭就宽），但真正决定吻有多粗的是里面那个鼻腔。
    w_tooth = TOOTH_W_RATIO * d.head_m
    nasal = nasal_side(d)
    face_w = max((2.0 + diet.palate) * w_tooth, nasal)
    if diet.occlusion == "gulp":          # 整只吞的，口裂必须比猎物粗
        face_w = max(face_w, 2.0 * sphere_r(diet.prey_frac * d.mass))

    # 齿冠必须撑过一辈子的磨耗：冠高 = 磨耗速率 × 寿命，牙根再按冠高的一半算。
    # 牛 20 年 × 3 mm/yr = 60 mm 冠，实测牛臼齿冠高 55–70 mm。
    #
    # **腭往下压多少只由牙根定，鼻腔是往上顶的**：鼻腔在腭之上、吻背之下，顶出来的是
    # 吻背的高度而不是腭的深度。上一版把鼻腔也算进腭的下沉里，咬合面被压到脑颅底下
    # 四公分，颌关节跟着掉下去，颞肌力臂反而比咬合力臂还长——狼的矢状嵴因此算成 0.16 px
    # （实测狼的嵴 1.5 cm）。改回来之后狼算出 1.3 cm。
    #
    # 于是"吻背比颅顶低还是高"成了一条**推出来的**判别：狼低（有额段），牛高（额面隆
    # 起）——这正是两者侧影最直观的差别，而它不是画出来的。
    crown = diet.wear * 1e-3 * d.lifespan
    u_occ = u_bot - P(crown * 1.5)         # 咬合面（齿列所在的高度）
    u_face = u_occ + P(crown * 1.5 + nasal)                    # 吻背

    # ③ 颌关节 -----------------------------------------------------------
    f_tmj = P(0.12 * d.head_m)
    u_tmj = u_occ + P(tmj_lift(d, diet))
    hd.tmj = (u_tmj, f_tmj)
    hd.occ = u_occ

    # ④ 咀嚼肌：力臂也是推的 ----------------------------------------------
    # 颞肌臂 = 冠状突高。冠状突能长到的极限就是颅顶——再高就顶到皮了。
    # 咬肌臂 = 关节到**颧弓前根**的一半：咬肌从颧弓垂下来插在下颌角上，合力作用点在这段
    # 的中间。颧弓比脑颅前缘还往前伸出脸长的两成（ARCH_REACH），漏掉这一截力臂会小三倍。
    f_row_end = f_brain                   # 颊齿列后端 ≈ 脑颅前缘（眶下）
    f_arch = f_brain + ARCH_REACH * max(L_px - f_brain, 0.0)
    arm_t = max(u_top - u_tmj, 0.05 * L_px) / px_m
    arm_m = max(0.5 * (f_arch - f_tmj), 0.05 * L_px) / px_m
    arm_bite = max(hd.bite_px - f_tmj, 0.05 * L_px) / px_m

    f_t = diet.temporalis * bite_N * arm_bite / arm_t
    f_m = (1.0 - diet.temporalis) * bite_N * arm_bite / arm_m
    a_t, a_m = f_t / SIGMA_MUSCLE, f_m / SIGMA_MUSCLE
    hd.pcsa = (a_t, a_m)

    # ⑤ 骨头往哪长：最省骨的解 --------------------------------------------
    # 咬肌：从颧弓垂到下颌角，横截面是**水平**的一片（前后长 × 横向厚）。前后长由几何
    # 定死（关节到颧弓前根），所以横向厚度 = 面积 / 长度，颧弓就得外张这么多才装得下。
    m_len = max((f_arch - f_tmj) / px_m, 1e-4)
    t_mass = (a_m / 2.0) / m_len                      # 单侧
    # 颞肌：截面是**竖直**的一片（高 × 横向厚），塞在脑颅与颧弓之间的颞窝里。高和厚都
    # 自由，骨往哪长是个极值问题：min(嵴高 + λ·外张) s.t. 高×厚 = 截面积。
    #
    # λ 不是 1。**加高比加宽便宜**：矢状嵴是一片薄骨，而颧弓外张要把整个头加宽，弓本身
    # 还得更粗才拉得住咬肌。取 λ=3 解出 高 = √(λA)、厚 = √(A/λ)——于是颞窝是**竖长的**，
    # 不是正方形。取 λ=1（初版）解出正方形颞窝，狼的颧弓被顶到离脑颅 4.2 cm（实测
    # 1.5 cm），整颗头宽出一半。
    a_side = a_t / 2.0
    h_need = math.sqrt(ARCH_COST * a_side)
    h_brain = bh
    crest_m = max(0.0, h_need - h_brain)
    t_temp = a_side / max(h_brain, h_need)
    hd.crest = P(crest_m)
    hd.arch = P(max(t_mass, t_temp, ARCH_MIN * d.head_m * 0.5)) + P(bw) / 2.0
    hd.face_w = P(face_w)

    # ⑥ 下颌：一根梁 ------------------------------------------------------
    # 咬合点受力、关节受反力，中段弯矩最大。截面深宽比取哺乳类下颌实测的 2.2。
    m_jaw = bite_N * arm_bite * 0.5
    w_jaw = (6.0 * m_jaw * SAFETY / (SIGMA_BONE * JAW_ASPECT ** 2)) ** (1.0 / 3.0)
    hd.jaw_depth = P(w_jaw * JAW_ASPECT)

    # ⑦ 眼 ---------------------------------------------------------------
    er, phi, ov, blind, el = eye_geom(d, diet)
    hd.eye_r, hd.phi, hd.overlap, hd.blind, hd.eye_el = P(er), phi, ov, blind, el

    # ⑧ 耳 ---------------------------------------------------------------
    eh, ew = ear_plate(d)
    hd.ear_plate = (P(eh), P(ew))

    # ⑨ 角 ---------------------------------------------------------------
    if d.horn:
        st = HORN_STYLE[d.horn]
        pts, s_tot, turn = horn_axis(d, st)
        hd.horn_len, hd.horn_turn = P(s_tot), math.degrees(turn)
        hd.horn_r = P(pts[0][1] / 2.0)
        hd.horn_bend = P(horn_bend_r(d, st))

    # ⑩ 几何 -------------------------------------------------------------
    _assemble(hd, f_brain, u_top, u_bot, u_occ, u_face, f_tmj, u_tmj, f_row_end)
    hd.standoff = _standoff(hd)
    hd.org = hd.org + hd.e_f * hd.standoff
    lo, hi = piece_bounds(hd, {"skull", "jaw"})
    hd.pred_H, hd.pred_W = hi[1] - lo[1], hi[0] - lo[0]
    hd.gape = _gape(hd)
    return hd


def _standoff(hd: Head) -> float:
    """这颗头得从表皮上**坐起来**多高（px）。

    头贴着表皮放的时候，凡是比枕髁宽的部位（腮帮子、颧弓、下颌角）都会陷进核心里——
    表皮是弯的，头是往两侧长的，这是几何必然，不是摆错了。真实的移植体也不是贴上去的：
    融合处会堆起一圈组织，头骑在那个基座上。

    高度不是拍的，是**解出来的最小值**：沿自己的朝向往外挪，挪到癒合区以外再没有一块
    压在等值面里为止。挪多了就成了长脖子（这东西没有颈），所以取最小。
    """
    fuse = max(hd.brain_px) * 0.7
    for k in range(25):
        t = k * 0.25
        ok = True
        for p in hd.pieces:
            for e in (p.a, p.b):
                q = hd.org + hd.e_f * t + hd.e_r * e[0] + hd.e_u * e[1] + hd.e_f * e[2]
                if float(np.linalg.norm(q - (hd.org + hd.e_f * t))) < fuse:
                    continue
                if C.fld(q - C.CORE_CENTER) >= C.ISO:
                    ok = False
                    break
            if not ok:
                break
        if ok:
            return t
    return 6.0


def _gape(hd: Head) -> float:
    """最大张口角（°）。吞食类要能把整只猎物塞进去 ⇒ 口裂张开的净空 ≥ 猎物直径。"""
    if hd.diet.prey_frac <= 0.0:
        return hd.diet.chew_deg * 1.6
    r = sphere_r(hd.diet.prey_frac * hd.donor.mass) * hd.px_m
    jaw = max(hd.L - hd.tmj[1], 1e-6)
    return min(math.degrees(2.0 * math.asin(min(r / jaw, 1.0))), 110.0)


def piece_bounds(hd: Head, parts: set[str] | None = None) -> tuple[np.ndarray, np.ndarray]:
    """基元在**局部**坐标下的包围盒。`parts` 限定只量哪几根骨上的东西。

    量"推出来的头高头宽"时必须只取 skull + jaw：角和耳当然会伸到头外面去，把它们算进
    来就等于拿角的展幅去和头颅模板对拍，那个数字没有意义。
    """
    lo = np.array([1e9] * 3)
    hi = np.array([-1e9] * 3)
    for p in hd.pieces:
        if parts is not None and p.part not in parts:
            continue
        for e in (p.a, p.b):
            v = np.array(e, float)
            rad = np.array([p.r1, p.r2, p.r2])
            lo = np.minimum(lo, v - rad)
            hi = np.maximum(hi, v + rad)
    return lo, hi


def _assemble(hd: Head, f_brain, u_top, u_bot, u_occ, u_face, f_tmj, u_tmj, f_row_end) -> None:
    """把推导结果摊成一堆可渲染的块。这里只做**摆放**，不再做任何力学决定。"""
    d, diet = hd.donor, hd.diet
    L, px = hd.L, hd.px_m

    coat = coat_px(hd)
    cmat = COAT_MAT[d.coat]

    def add(p: Piece) -> None:
        # 可省的块：算出来薄过渲染下限的一半就整块丢掉，不撑到下限（见 Piece.soft）
        if p.soft and min(p.r1, p.r2) < RENDER_MIN * 0.55:
            hd.dropped.append(p.name)
            return
        if not p.name.startswith(FEATURE_PARTS):
            # **头的肉身**：按体表厚度整圈外扩，并统一涂成供体自己的体表材质。
            # 这一步是把"头骨"变成"头"的全部：外扩把颅骨、颧弓、下颌那些台阶填成一个
            # 连续的体，统一材质把"解剖标本"变回"一只兽的脑袋"。
            k = 0.4 if p.name.startswith("pinna") else 1.0   # 耳廓是薄片，毛也薄
            p = Piece(p.part, p.name, p.a, p.b,
                      p.r1 + coat * k, p.r2 + coat * k, cmat, p.soft)
        hd.pieces.append(p)
    bl, bh, bw = hd.brain_px

    # —— 脑颅：一个盒子，顶上按需要加矢状嵴
    add(Piece("skull", "braincase", (0.0, 0.0, 0.0), (0.0, 0.0, bl),
              bw / 2.0, bh / 2.0, "skull"))
    if hd.crest > RENDER_MIN:
        add(Piece("skull", "crest", (0.0, u_top, bl * 0.15),
                  (0.0, u_top + hd.crest, bl * 0.95),
                  max(bw * 0.10, RENDER_MIN), hd.crest / 2.0, "skull", soft=True))

    # —— 额区：脑腔前缘到眶前缘。它装的是颞窝、眶和鼻腔后段，属于颅不属于吻——**把它
    #    算进吻里，狼的口鼻会占掉头长的七成**（实测约五成半），整颗头读成一根管。
    f_orbit = f_brain + FRONTAL * max(L - f_brain, 0.0)
    if f_orbit - f_brain > 0.2:
        add(Piece("skull", "frontal", (0.0, (u_top + u_occ) * 0.5, f_brain),
                  (0.0, (u_top + u_occ) * 0.5, f_orbit),
                  max(bw / 2.0 * 0.92, RENDER_MIN),
                  max((u_top - u_occ) / 2.0 * 0.9, RENDER_MIN), "skull"))

    # —— 脸：一根**等应力悬臂**。吻端受咬合反力，根部弯矩最大 ⇒ 等强度截面 h ∝ √x，
    #    所以侧面看是个抛物线收细的口鼻，不是一根等粗的管。分三段够了（再细超过模型精度）。
    f_muzzle = f_orbit                    # 吻从眶前缘起，不从脑腔前缘起
    face = max(L - f_muzzle, 0.5)
    h_face = max(u_face - u_occ, RENDER_MIN)      # 腭到吻背：脸的全高
    w_face = max(hd.face_w, RENDER_MIN * 2)
    ns = 3
    for i in range(ns):
        t0, t1 = i / ns, (i + 1) / ns
        f0, f1 = f_muzzle + face * t0, f_muzzle + face * t1
        # 从吻端往回量的距离越大截面越深：x = 1-t
        s0, s1 = math.sqrt(1.0 - t0) if t0 < 1 else 0.0, math.sqrt(max(1.0 - t1, 0.0))
        k = 0.5 * (s0 + s1)
        hh = max(h_face * 0.5 * (0.30 + 0.70 * k), RENDER_MIN)
        ww = max(w_face * 0.5 * (0.62 + 0.38 * k), RENDER_MIN)
        mid_u = u_occ + hh
        add(Piece("skull", f"face_{i}", (0.0, mid_u, f0), (0.0, mid_u, f1),
                  ww, hh, "skull"))

    # —— 两块咀嚼肌：**这是整层推导直接变成的可见物**，不是贴上去的装饰。
    #    颞肌填在脑颅侧面与颧弓之间、从关节高度一直到嵴顶；咬肌挂在颧弓下面、从关节
    #    往前到弓的前根。两块的横截面就是前面解出来的 PCSA，摆在它们各自该在的地方。
    #    round 1 只画了一条 1 px 高的"腮"，等于把算出来的东西又扔了。
    stand = max(hd.arch - bw / 2.0, RENDER_MIN)      # 颧弓离脑颅多远 = 肌肉的厚度
    f_arch = f_orbit + (L - f_orbit) * 0.20
    j_bot = u_occ - hd.jaw_depth                      # 下颌下缘：咬肌的下界
    for sgn, tag in ((-1.0, "l"), (1.0, "r")):
        cx = sgn * (bw / 2.0 + stand / 2.0)
        # 颞肌：从关节高度一直填到嵴顶，前后占满枕到眶的那一段（就是颞窝本身）
        # 竖直范围就是推导里那个"可用窝高"：脑腔底到嵴顶。用关节高度当下界是错的——
        # 颧弓以下归咬肌，颞肌是从弓的内侧穿过去的。
        h_t = max(u_top + hd.crest - u_bot, RENDER_MIN * 2)
        add(Piece("skull", f"temporalis_{tag}", (cx, u_bot, f_orbit * 0.5),
                  (cx, u_bot + h_t, f_orbit * 0.5 + h_t * 0.10),
                  stand / 2.0, max(f_orbit * 0.45, RENDER_MIN), "jowl", soft=True))
        # 咬肌：**上界是颧弓、下界是下颌下缘**，不能自己再往下垂一截（round 1 的牛头就是
        # 这样在下颌以下挂了一大块）
        cu = 0.5 * (u_tmj + j_bot)
        add(Piece("skull", f"masseter_{tag}", (cx, cu, f_tmj), (cx, cu, f_arch),
                  stand / 2.0, max((u_tmj - j_bot) / 2.0, RENDER_MIN), "jowl", soft=True))

    # —— 下颌：**左右两条梁** + 联合 + 升支 + 冠状突。
    #    梁的截面是解出来的（深宽比 2.2，深 = 抗弯要的深度），两条分别贴在齿列外侧。
    #    round 1 画成一整块 2.1 px 宽 × 1.0 px 深的板——比它自己还宽，那不是下颌是托盘。
    j_u = u_occ - hd.jaw_depth / 2.0
    w_jaw = max(hd.jaw_depth / JAW_ASPECT / 2.0, RENDER_MIN)
    tip = L * (1.0 - 0.02)
    jx = max(hd.face_w / 2.0 - w_jaw, w_jaw)
    f_back = f_tmj + (f_row_end - f_tmj) * 0.15
    for sgn, tag in ((-1.0, "l"), (1.0, "r")):
        add(Piece("jaw", f"corpus_{tag}", (sgn * jx, j_u, f_back),
                  (sgn * jx, j_u, tip - hd.face_w * 0.4), w_jaw,
                  max(hd.jaw_depth / 2.0, RENDER_MIN), "skull"))
        add(Piece("jaw", f"ramus_{tag}", (sgn * jx, j_u, f_tmj),
                  (sgn * jx, u_tmj, f_tmj), w_jaw,
                  max(hd.jaw_depth / 2.0, RENDER_MIN), "skull"))
        cor = u_top - u_tmj
        if cor > RENDER_MIN:
            add(Piece("jaw", f"coronoid_{tag}", (sgn * jx, u_tmj, f_tmj + w_jaw * 1.2),
                      (sgn * jx, u_tmj + cor, f_tmj + w_jaw * 2.4),
                      w_jaw, max(w_jaw * 0.8, RENDER_MIN), "skull"))
    # 下颌联合：两条梁在颏部合成一块，这是下颌能整体转的原因
    add(Piece("jaw", "symphysis", (0.0, j_u, tip - hd.face_w * 0.55), (0.0, j_u, tip),
              max(hd.face_w / 2.0, RENDER_MIN), max(hd.jaw_depth / 2.0, RENDER_MIN),
              "skull"))
    # 颌间的那一团：舌、舌骨、二腹肌。它不是装饰——**两条下颌梁之间本来就是实心的**，
    # 空着的话侧面看下巴是两根悬空的杆。上界是咬合面（舌顶着腭），下界是下颌下缘。
    if hd.face_w > w_jaw * 2.2:
        add(Piece("jaw", "throat", (0.0, j_u, f_back), (0.0, j_u, tip - hd.face_w * 0.5),
                  max(hd.face_w / 2.0 - w_jaw, RENDER_MIN),
                  max(hd.jaw_depth / 2.0, RENDER_MIN), "muzzle"))

    # —— 齿：每种咬合模式露出来的是不同的东西（默认在嘴里，见 _teeth）
    _teeth(hd, u_occ, j_u, tip, bw)

    # —— 唇缝：**闭着的嘴在这个分辨率下就是一道暗缝**，别的什么都看不见。没有它，
    #    整颗头是一坨没有开口的肉；有了它，同一坨肉立刻读成"这是个脑袋，那是嘴"。
    #    缝从口角走到吻端；口角的位置是推出来的——下颌能张开的那一段的前端，也就是
    #    颊齿列的后端。
    if not d.beak:
        mouth_x = max(hd.face_w / 2.0 + coat * 0.6, RENDER_MIN)
        for sgn, tag in ((-1.0, "l"), (1.0, "r")):
            add(Piece("skull", f"lip_{tag}", (sgn * mouth_x, u_occ, f_row_end),
                      (sgn * mouth_x, u_occ, tip), RENDER_MIN * 0.5,
                      max(coat * 0.5, RENDER_MIN * 0.7), "lip"))

    # —— 鼻 / 喙 / 鼻盘
    _snout_end(hd, u_occ, tip, bw, h_face, coat)

    # —— 眼：眶位由 φ 定，眼球嵌在脸的侧上方
    # 眶所在那一段的**真实外表面**是颧弓（它一直伸到眶前缘之前），不是脑颅侧壁。
    # 拿脑颅宽定眼位，眼会被埋进腮帮子里——实测狼眼在 ±1.57 而那一处的皮在 ±2.81。
    _eyes(hd, f_orbit, max(u_top, u_face), u_occ,
          2.0 * max(bw / 2.0, hd.arch, hd.face_w / 2.0), coat)

    # —— 耳 / 鼓膜
    _ears(hd, bl, bh, bw, u_top, coat)

    # —— 角
    if d.horn:
        _horns(hd, bl, bw, u_top + coat)

    # —— 缝：这颗头是**缝上去**的，根部得有一圈癒合环
    # 癒合环：这颗头是**缝上去**的。它同时要把基座那一段填上（见 _standoff），所以
    # 往后延伸的长度是基座高度加一点余量——否则头会浮在离表皮几像素的地方。
    add(Piece("skull", "collar", (0.0, 0.0, -L * 0.06 - hd.standoff),
              (0.0, 0.0, L * 0.05),
              max(bw * 0.58, RENDER_MIN), max(bh * 0.56, RENDER_MIN), "collar"))


def _teeth(hd: Head, u_occ, j_u, tip, bw) -> None:
    """齿。**默认在嘴里，不在外面。**

    初版把每一颗都画出来了，于是狼永远呲着四颗白牙、羊永远露着一排门齿——那是把标本
    当活物画。活兽闭着嘴的时候能看见的只有一道唇缝；露在外面的齿是**例外**，而例外有
    明确的条件：

      · **终生生长的齿按定义长过唇**——啮齿的门齿、猪的獠牙磨多少长多少，唇包不住。
        （啮齿的唇还在门齿**之后**闭合，那正是它能一边啃一边不吃进碎屑的原因。）
      · 其余的齿只有齿冠长过唇厚（毛 + 皮）才露出个尖。狼的犬齿冠算出 1.1 px、唇厚
        0.9 px，露 0.2 px——在模型精度以下，所以它闭着嘴时看不见牙。真狼也是这样。
    """
    add = hd.pieces.append
    diet = hd.diet
    if hd.donor.beak:            # 喙就是它的齿，另见 _snout_end
        return
    lip = coat_px(hd) + 0.3      # 唇：毛 + 皮
    w = TOOTH_W_RATIO * hd.donor.head_m * hd.px_m
    t = max(w * 0.5, RENDER_MIN)
    if diet.occlusion == "shear":
        # 犬齿。**长度是强度定的**：猎物挣扎时齿尖受侧向力，圆锥形的齿在根部弯断，
        # σ = 4FL/(πr³) ⇒ L = σπr³/(4F)。取 F = 咬合力的两成（挣扎的横向分量）。
        r = max(w * 0.55, RENDER_MIN)
        fl = 0.20 * hd.bite_N
        lm = SIGMA_DENTIN * math.pi * (r / hd.px_m) ** 3 / (4.0 * max(fl, 1e-6))
        ln = min(hd.px_m * lm, hd.L * 0.10)
        # 闭口时**下犬齿在上犬齿的前面、上犬齿在下颌的外侧**——这是剪切类咬合的定式，
        # 也是几何必需：round 1 把两者放在同一个横位、下齿还排在上齿之后，上犬齿直接
        # 穿过下颌体。
        show = ln - lip                      # 露在唇外的那一截
        if show < 0.35:                      # 包得住 ⇒ 闭嘴时根本看不见
            hd.dropped.append("canine（唇包住了）")
            return
        half = hd.face_w / 2.0
        for sgn, tag in ((-1.0, "l"), (1.0, "r")):
            xu, xd = sgn * (half + r * 0.6), sgn * max(half - r * 0.6, r)
            add(Piece("skull", f"canine_u{tag}", (xu, u_occ - lip, tip * 0.84),
                      (xu, u_occ - lip - show, tip * 0.84), r, r, "tooth", soft=True))
            add(Piece("jaw", f"canine_d{tag}", (xd, j_u + lip, tip * 0.90),
                      (xd, j_u + lip + show, tip * 0.90), r, r, "tooth", soft=True))
    elif diet.occlusion == "gnaw":
        # 门齿是一把**终生生长的凿子**：磨掉多少长回多少，露在外面的那一截由唇线定。
        for sgn, tag in ((-1.0, "l"), (1.0, "r")):
            cx = sgn * bw * 0.16
            add(Piece("skull", f"incisor_u{tag}", (cx, u_occ, tip),
                      (cx, u_occ - hd.L * 0.10, tip - hd.L * 0.03), t, t, "tooth"))
            add(Piece("jaw", f"incisor_d{tag}", (cx, j_u, tip - hd.L * 0.02),
                      (cx, j_u + hd.L * 0.09, tip - hd.L * 0.05), t, t, "tooth"))
    elif diet.occlusion == "grind":
        # 食草兽上颌没有门齿，只有一块角质垫；下门齿顶着它把草切断。
        add(Piece("skull", "dental_pad", (0.0, u_occ, tip - hd.L * 0.05),
                  (0.0, u_occ, tip), max(bw * 0.34, RENDER_MIN), t * 0.6, "muzzle"))
        # 下门齿在唇后面顶着角质垫切草，闭嘴时看不见——不画
    elif diet.occlusion == "crush":
        # 杂食：门齿铲 + 露出来的下犬齿（獠牙）。獠牙也终生生长，所以按寿命算长度。
        tu = hd.L * 0.05 * (hd.donor.lifespan / 15.0)
        if tu > RENDER_MIN:
            for sgn, tag in ((-1.0, "l"), (1.0, "r")):
                cx = sgn * bw * 0.30
                add(Piece("jaw", f"tusk_{tag}", (cx, j_u, tip * 0.72),
                          (cx, j_u + tu, tip * 0.72 - tu * 0.4), t, t, "tooth"))


def _snout_end(hd: Head, u_occ, tip, bw, h_face, coat) -> None:
    """吻端：喙 / 鼻盘 / 鼻镜。三种都是**功能件**，不是装饰。"""
    add = hd.pieces.append
    d, diet = hd.donor, hd.diet
    tip = tip + coat            # 吻端的特征同样要长到毛外面去
    if d.beak:                                          # 禽：喙
        # 喙是一把角质钳。没有牙，全部咬合力集中在喙尖 ⇒ 它是一根短悬臂，按抗弯定粗细。
        r, _ = LB.bone_radius(hd.L * 0.22, hd.L * 0.22, hd.bite_N)
        r = max(r * SIGMA_BONE / SIGMA_KERATIN, RENDER_MIN)
        add(Piece("skull", "beak_u", (0.0, u_occ + h_face * 0.30, tip - hd.L * 0.26),
                  (0.0, u_occ - hd.L * 0.02, tip + hd.L * 0.04),
                  max(bw * 0.24, RENDER_MIN), r, "beak"))
        add(Piece("jaw", "beak_d", (0.0, u_occ - hd.L * 0.06, tip - hd.L * 0.24),
                  (0.0, u_occ - hd.L * 0.04, tip),
                  max(bw * 0.20, RENDER_MIN), max(r * 0.6, RENDER_MIN), "beak"))
        return
    if d.diet == "omnivore":
        # 猪的鼻盘是一把**铲**：拱土的力由颈部出，铲面积 = 力 / 土的承载力。用的正是
        # 肢体层给脚掌定尺寸的那个 BEARING——同一块地，同一条承载力。
        f = 0.6 * d.mass * G
        a = f / LB.BEARING
        rr = math.sqrt(a / math.pi) * hd.px_m
        add(Piece("skull", "rooting_disc", (0.0, u_occ + h_face * 0.32, tip - hd.L * 0.04),
                  (0.0, u_occ + h_face * 0.32, tip + hd.L * 0.03),
                  max(rr, RENDER_MIN), max(rr, RENDER_MIN), "muzzle"))
        return
    # 其余：鼻镜 + 两个鼻孔
    add(Piece("skull", "rhinarium", (0.0, u_occ + h_face * 0.42, tip - hd.L * 0.05),
              (0.0, u_occ + h_face * 0.36, tip + hd.L * 0.01),
              max(bw * 0.26, RENDER_MIN), max(hd.L * 0.045, RENDER_MIN), "muzzle"))
    for sgn, tag in ((-1.0, "l"), (1.0, "r")):
        add(Piece("skull", f"nostril_{tag}",
                  (sgn * bw * 0.16, u_occ + h_face * 0.44, tip - hd.L * 0.01),
                  (sgn * bw * 0.16, u_occ + h_face * 0.44, tip + hd.L * 0.015),
                  RENDER_MIN * 0.8, RENDER_MIN * 0.8, "nostril", soft=True))


def _eyes(hd: Head, f_brain, u_top, u_occ, bw, coat) -> None:
    """眼球摆在眶里。φ 是视轴离中线的角，眼球沿这个方向从脸侧凸出来。"""
    add = hd.pieces.append
    r = max(hd.eye_r, RENDER_MIN)
    a, el = math.radians(hd.phi), math.radians(hd.eye_el)
    # 眶的前后位置：眼要看得见吻端（不然自己的鼻子挡住猎物），所以贴在脸的后段。
    f_eye = f_brain * 0.92
    # 眶的高度：在颅底与颅顶之间，按视轴仰角抬。蛙的 70° 仰角把它直接顶到颅顶。
    u_eye = u_occ + (u_top - u_occ) * (0.62 + 0.38 * math.sin(el))
    # **眼长在毛的外面**：眶是骨上的窝，可眼睑与睫毛在体表那一层。不把眼往外推一个毛厚，
    # 加上体表包络之后整张脸会变成一块没有五官的板——实测狼头的眼和鼻都被埋掉，正视图
    # 是一整块平板。
    half_w = max(bw / 2.0, r) + coat
    for sgn, tag in ((-1.0, "l"), (1.0, "r")):
        # 眼球**嵌在**眶里，只鼓出去小半个：眶缘是骨，眼球比它退一点才有眼皮的位置。
        cx = sgn * half_w * 0.92
        c = (cx, u_eye, f_eye)
        out = (cx + sgn * r * 0.55 * math.sin(a), u_eye + r * 0.55 * math.sin(el),
               f_eye + r * 0.55 * math.cos(a) * math.cos(el))
        hd.eye_pos.append(c)
        # 侧眼的猎物露巩膜（那圈白是它转着眼睛看四周留下的），正眼的捕食者不露
        if hd.phi > 60.0:
            inw = (cx - sgn * r * 0.35, u_eye, f_eye)
            add(Piece("skull", f"sclera_{tag}", inw, c, r * 1.12, r * 1.12, "sclera"))
        add(Piece("skull", f"eye_{tag}", c, out, r, r, "eye"))


def _ears(hd: Head, bl, bh, bw, u_top, coat) -> None:
    add = hd.pieces.append
    d = hd.donor
    if not d.pinna:
        if d.diet == "insectivore":     # 蛙：鼓膜贴在体表，直径由听的频段定
            rr = max(hd.eye_r * 0.8, RENDER_MIN)
            for sgn, tag in ((-1.0, "l"), (1.0, "r")):
                cx = sgn * (bw / 2.0 * 0.9 + coat)
                add(Piece("skull", f"tympan_{tag}", (cx, 0.0, bl * 0.55),
                          (cx + sgn * rr * 0.4, 0.0, bl * 0.55), rr, rr, "tympan"))
        return
    eh, ew = hd.ear_plate
    if eh < RENDER_MIN * 2:
        return
    # 耳廓从颅顶后角立起来，稍稍朝外（听身后——身前有眼睛管）。开口朝前外。
    #
    # **分两段收细**：耳廓是个三角形的漏斗，一根等宽的板渲出来是一片桨。长度已经在
    # `ear_plate` 里按填充率补过了（同样的散热面积，三角形要摊得更长）。
    for sgn, tag in ((-1.0, "l"), (1.0, "r")):
        # 耳根接在**外耳道**的位置：脑颅后段的侧面、颧弓上方，不是颅顶。接在颅顶上
        # 长角的那三种就会变成角在下、耳在上，正好反了。
        x0, y0, z0 = sgn * bw * 0.42, u_top * 0.55, bl * 0.30
        dx, dy, dz = sgn * eh * 0.20, eh * 0.80, -eh * 0.12
        for k, wk in ((0, 1.0), (1, 0.58)):
            a = (x0 + dx * k * 0.5, y0 + dy * k * 0.5, z0 + dz * k * 0.5)
            b = (x0 + dx * (k + 1) * 0.5, y0 + dy * (k + 1) * 0.5, z0 + dz * (k + 1) * 0.5)
            add(Piece(f"ear_{tag}", f"pinna_{tag}{k}", a, b,
                      max(ew * 0.5 * wk, RENDER_MIN), max(ew * 0.16, RENDER_MIN), "skull"))
        inner = (x0 + dx * 0.35, y0 + dy * 0.35, z0 + dz * 0.35)
        add(Piece(f"ear_{tag}", f"conch_{tag}", (x0, y0, z0), inner,
                  max(ew * 0.28, RENDER_MIN), max(ew * 0.12, RENDER_MIN), "ear_in"))


def _horns(hd: Head, bl, bw, u_top) -> None:
    """角：沿推出来的对数螺线摆一串收细的段。"""
    add = hd.pieces.append
    d = hd.donor
    st = HORN_STYLE[d.horn]
    pts, _s, _t = horn_axis(d, st)
    n_seg = 6
    for sgn, tag in ((-1.0, "l"), (1.0, "r")):
        # 起始方向：朝外 out_deg、朝后 back_deg
        ao, ab = math.radians(st.out_deg), math.radians(st.back_deg)
        dirv = np.array([sgn * math.sin(ao), math.cos(ao) * math.cos(ab),
                         -math.sin(ab)], float)
        dirv = dirv / np.linalg.norm(dirv)
        # 卷曲平面。**镜像不是把轴的 x 取反**：向量 v 镜像成 Mv 之后，绕 a 转 θ 对应绕
        # (a_x, −a_y, −a_z) 转同一个 θ。把 x 取反（round 1 的写法）会让左右两只角朝相反
        # 的方向卷——盘羊一只往后盘、另一只往前盘，整对角在正面看是散开的两片。
        #   · 盘角绕**横轴**转 ⇒ 在近矢状面里盘：上 → 后 → 下 → 前 → 上
        #   · 直角同样绕横轴，只是转得少
        #   · 钩角绕**竖轴**转 ⇒ 先横着支出去，再往前钩
        axis = np.array(HORN_AXIS[d.horn], float)
        if sgn < 0:
            axis = np.array([axis[0], -axis[1], -axis[2]])
        axis = axis / np.linalg.norm(axis)
        p = np.array([sgn * bw * 0.34, u_top, bl * 0.30], float)
        for i in range(n_seg):
            u0, u1 = i / n_seg, (i + 1) / n_seg
            s0, d0 = _lerp_pt(pts, u0)
            s1, d1 = _lerp_pt(pts, u1)
            ds = (s1 - s0) * hd.px_m
            # 这一段中点的曲率 → 转过多少
            dm = 0.5 * (d0 + d1)
            dth = (1.0 - st.inner) / max(dm, 1e-6) * (s1 - s0)
            q = p + dirv * ds
            add(Piece(f"horn_{tag}", f"horn_{tag}_{i}", tuple(p), tuple(q),
                      max(0.5 * d0 * hd.px_m, RENDER_MIN),
                      max(0.5 * d1 * hd.px_m, RENDER_MIN), "horn"))
            dirv = _rot(dirv, axis, dth)
            p = q


def _lerp_pt(pts, u: float) -> tuple[float, float]:
    i = min(int(u * (len(pts) - 1)), len(pts) - 2)
    t = u * (len(pts) - 1) - i
    a, b = pts[i], pts[i + 1]
    return a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t


def _rot(v: np.ndarray, axis: np.ndarray, th: float) -> np.ndarray:
    c, s = math.cos(th), math.sin(th)
    return v * c + np.cross(axis, v) * s + axis * float(axis @ v) * (1.0 - c)


# ---------------------------------------------------------------- 整只兽
def build(seed: int, *, socks: dict[str, C.Socket] | None = None) -> dict[str, Head]:
    socks = socks or C.sockets()
    gen = GN.sample(seed, socks=socks)
    return separate({hg.socket: solve_head(hg, socks[hg.socket]) for hg in gen.heads})


def vision(heads: dict[str, Head]) -> tuple[float, float]:
    """整只兽的**水平视野覆盖**（度）与最大连续盲区（度）。

    多头是这只兽唯一真正的便宜：几颗朝向不同的头拼起来，覆盖能接近全周。这是基因组
    随机装配出来的涌现属性，不是设计的——所以值得报出来。
    """
    if not heads:
        return 0.0, 360.0
    seen = np.zeros(360, bool)
    for hd in heads.values():
        fov = FOV_PRED if hd.diet.predator else FOV_PREY
        for dv in hd.eye_dirs():
            a = math.degrees(math.atan2(float(dv[0]), -float(dv[2])))
            for k in range(360):
                dd = abs((k - a + 180.0) % 360.0 - 180.0)
                if dd <= fov / 2.0:
                    seen[k] = True
    cov = float(seen.sum())
    run = best = 0
    for k in range(720):
        if seen[k % 360]:
            run = 0
        else:
            run += 1
            best = max(best, run)
    return cov, float(min(best, 360))


def report(heads: dict[str, Head] | None = None) -> str:
    """推导表。没给 heads 就把十种供体各出一颗标准头（scale=1）排出来。"""
    if heads is None:
        socks = C.sockets()
        s = socks["head_c"]
        heads = {k: solve_head(GN.HeadGene("head_c", k, 1.0), s)
                 for k in sorted(GN.HEAD_TEMPLATES)}
    rows = ["  供体     咬合  咬合力N  脑颅%  嵴px  颧张px  颌深px  眼φ°  重叠°  盲°"
            "  耳px      角px/圈/储能÷抗弯   高px(模板)  宽px(模板)"]
    for k, hd in heads.items():
        tpl = GN.HEAD_TEMPLATES[hd.kind]
        horn = "—"
        if hd.donor.horn:
            horn = (f"{hd.horn_len:.1f}/{hd.horn_turn / 360.0:.2f}圈/"
                    f"{hd.horn_r / max(hd.horn_bend, 1e-9):.1f}×")
        ear = f"{hd.ear_plate[0]:.1f}×{hd.ear_plate[1]:.1f}" if hd.ear_plate[0] else "—"
        rows.append(
            f"  {hd.kind:<8} {hd.diet.occlusion:<5} {hd.bite_N:7.0f} "
            f"{hd.brain_px[0] / hd.L:5.0%} {hd.crest:5.1f} {hd.arch:6.1f} "
            f"{hd.jaw_depth:6.1f} {hd.phi:5.1f} {hd.overlap:5.1f} {hd.blind:4.0f} "
            f"{ear:<9} {horn:<18} {hd.pred_H:5.1f}({tpl[1] * hd.gene.scale:.1f}) "
            f"{hd.pred_W:5.1f}({tpl[2] * hd.gene.scale:.1f})")
    return "\n".join(rows)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--seed", type=int, default=None)
    args = ap.parse_args()
    if args.seed is None:
        print("十种供体（scale=1，挂在 head_c 上）：")
        print(report())
        return 0
    heads = build(args.seed)
    print(f"seed={args.seed}：{len(heads)} 颗头")
    print(report(heads))
    cov, blind = vision(heads)
    print(f"合眼水平视野覆盖 {cov:.0f}°，最大连续盲区 {blind:.0f}°")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
