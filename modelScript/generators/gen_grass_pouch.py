#!/usr/bin/env python3
"""小草包（grass_pouch）Blockbench .bbmodel 生成器。

穿戴容器三档里的**中档**。同系已有两端：

    破草包 worn_grass_pouch  GrassPouchBack/Front.bbmodel（4 cube，6×6×4px）
                             —— 缝线快散，容量 8.0，磨损 0.008
    小草包 grass_pouch       ← **本件**，可制作升级款，容量 10.0，磨损 0.005
    背篓   back_basket       gen_back_basket.py（102 cube，竹架 + 兽皮盖）

问题背景：破草包和小草包在 client 里**共用同一套 geo 和同一张贴图**
（`grass_pouch_back.geo.json` 被 `WornPackModel.java` 手工转写成 vanilla
ModelPart，两个物品指同一份），数值上却是两个档位。玩家从外观完全分不出
身上背的是哪一款。本件给中档一个专属形制。

**与破草包的可辨差异**（三条，远处也认得出）：
  1. 编身**完好**且双股封边（破草包是散口 + 单层薄壁）
  2. 多一个**侧插袋**（右侧，草编，插草药露头）
  3. 翻盖由**两道草绳**系住并压出折痕（破草包只有一片平盖 + 一个小扣）

视觉语言（末法残土穷酸调，沿用背篓同色系但更素）：
  草编身 weave  —— 枯草黄压灰，横向粗编带
  编缝   seam   —— 编带间暗带，读出「一条条」而不是一块板
  封边   braid  —— 双股搓边，比编身亮一档（新编的边，是「完好」的证据）
  翻盖   flap   —— 同草编但压深一档，盖住上口
  草绳   cord   —— 系盖两道
  骨扣   bone   —— 泛黄骨栓
  针脚   stitch —— 深褐，密而整齐（对比破草包的散线）

**左右不对称是设计意图**：侧插袋只在右侧、盖压向左前。故 `check()` 只查
左右对称件（编身/封边/底），不对称件走 ASYM 白名单排除。

坐标：建模用居中空间（x/z ∈ [-8,8]，地面 y=0），写盘前平移进 MC 方块空间
0..16 —— 这样 rigkit.mirror_violations() 的 x=0 中轴判据可直接用。

用法:
    python3 modelScript/generators/gen_grass_pouch.py             # 生成 + 自检
    python3 modelScript/generators/gen_grass_pouch.py --check     # 只自检
    python3 modelScript/generators/gen_grass_pouch.py --part flap # 只出单件预览
    python3 modelScript/generators/gen_grass_pouch.py --list
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "core"))
from bbmodel_maker.gates import gatekit  # noqa: E402
from bbmodel_maker.gates.gatekit import Seat  # noqa: E402
from bbmodel_maker.rig.rigkit import Rig  # noqa: E402

REPO = Path(__file__).resolve().parents[2]
# 落 models/ 顶层：.gitignore:109 排掉 models/*/ 所有子目录（只白名单
# armor/baolongwang/handmade/lootcrate/rat），子目录里的 bbmodel 提不进仓库。
# 同级穿戴件 GrassPouchBack/DouliHat/SuoYiCloak 也都在顶层。
OUT_DIR = Path(__file__).resolve().parents[1] / "models"

PX = 16.0

# ── 材质（swatch=8，64×64 最多 64 种，这里 7 种）────────────────────
MATS = {
    "weave":  (152, 136, 102),   # 草编身：枯草黄压灰
    "seam":   (114, 100, 72),    # 编缝暗带：必须比 weave 暗才读出「一条条」
    "braid":  (176, 160, 120),   # 双股封边：比编身亮一档 = 新编的边
    "flap":   (134, 118, 88),    # 翻盖：同草编压深，和身分层
    "cord":   (140, 128, 98),    # 草绳
    "bone":   (206, 198, 170),   # 骨扣：泛黄骨白
    "stitch": (72, 56, 42),      # 针脚：深褐，比 cord 暗才读得出「缝」
}

# ── 主体尺寸（px，居中空间）─────────────────────────────────────────
# 对齐 grass_pouch_back.geo.json 的挂点区间（y 11.5–17.5、z 1.5–4.5，贴腰背）
# 但比破草包大一档：破草包 6×5.5×3px，本件 7.6×6.2×3.6px。
HALF_W = 3.5          # 口半宽 7.0px ≈ 0.44 格
HALF_D = 1.8          # 深 3.6px（贴背，扁）
Y0 = 0.0              # 包底（局部空间；接线时整体抬到腰背高度）
Y1 = 7.4              # 包口

# **收口**：底窄口宽。第一轮渲出来是个宽高比 1.28 的扁木箱 —— 四壁垂直、底
# 平，正面读作货箱不是草包。草包是软编物，装东西时底部被压窄、口撑开。
# 参考图（modelScript/ref/）三视都显示底部收得比我第二轮更狠、底角是圆的，
# 故从两段 0.85 内收改成**三段**，最底一段收 1.35：底宽 4.3 / 口宽 7.0 = 收
# 39%，逐段递减在正交渲染里读作圆底。z 深不收 —— 只 3.6px 总深，再收底板就成
# 刀片；参考图侧视也是深度基本不变、只有底角圆。
#   (每侧内收 px, 该段顶部 y)
TIERS = ((1.35, 2.00), (0.70, 4.20), (0.00, 7.4))
WALL = 0.9            # 壁厚
FLOOR = 0.8           # 底板厚

# 编带：带高与缝的比例决定「读不读得出编」。背篓那轮踩过坑 —— 缝不到带高
# 四成，九道带在 16px 空间糊成一块横纹木板。这里缝占带高 84%，同背篓修后值。
BAND_H = 0.60
PITCH = 1.10

# 翻盖压歪角（绕 z）。压盖绳必须**用同一个角**，否则水平绳压在斜盖上一头架空
# 一头陷进去 —— 第一轮实测左端悬 0.16px、右端咬 0.33px，左半段渲成一根梁。
FLAP_TILT = -3.5

# 前檐外表面 z。**骨扣的位置从这里派生**，不许两处各写一遍：前檐加深那轮就是
# 因为两处脱钩，骨扣被檐吞到只探出 0.16px（FRONT 实测仅 115px 可见）。
FLAP_FRONT_Z = HALF_D + 0.60

# 刻意不对称件：不参与左右镜像自检
ASYM = ("flap", "cord", "sidepocket", "stitch")


def _half_w_at(y: float) -> float:
    """给定高度处的壁半宽（跟着 TIERS 收口走）。编带/针脚都得查它对齐。"""
    for inset, top in TIERS:
        if y <= top:
            return HALF_W - inset
    return HALF_W


def part_body(rig: Rig) -> None:
    """草编身：底板 + 四壁，横向粗编带（完好，不散口）。"""
    rig.bone("body", (0.0, 0.0, 0.0))

    # 底板按最底一段的窄尺寸
    bw = HALF_W - TIERS[0][0]
    rig.cube("body", "floor",
             (-bw, Y0, -HALF_D), (bw, Y0 + FLOOR, HALF_D), mat="weave")

    # 四壁分三段收口。voxel 立方体是等截面的，锥形只能靠分段逼近 —— 同
    # rigkit.taper() 的思路，这里手写因为要保左右镜像成对。
    y0 = Y0 + FLOOR
    for tier, (inset, y1) in enumerate(TIERS):
        xh = HALF_W - inset
        for sz, side in ((1, "f"), (-1, "b")):
            z_out = sz * HALF_D
            z_in = sz * (HALF_D - WALL)
            rig.cube("body", f"wall_{side}_{tier}",
                     (-xh, y0, min(z_in, z_out)),
                     (xh, y1, max(z_in, z_out)), mat="weave")
        for sx, side in ((-1, "l"), (1, "r")):
            x_out = sx * xh
            x_in = sx * (xh - WALL)
            rig.cube("body", f"wall_{side}_{tier}",
                     (min(x_in, x_out), y0, -HALF_D + WALL),
                     (max(x_in, x_out), y1, HALF_D - WALL), mat="weave")
        y0 = y1

    # 横向编带：前后壁各铺一列暗缝带，读出「一条条草编」。
    # 带只压在壁外表面 0.22px —— 薄贴不算穿模（_interpenetrating 的 MIN_BITE
    # 是 0.55），但足够在正交渲染里投出一道暗线。
    y = Y0 + FLOOR + 0.45
    i = 0
    while y + BAND_H <= Y1 - 0.35:
        # 带宽跟着收口走，否则下段编带会飞出窄壁之外（悬空门抓不到这种「贴着
        # 壁但横向超出」的情形，只能在这里就算对）。取带顶所在段的半宽 —— 带
        # 跨段时按窄的那段算，宁可缩进 0.65px 也不许探头。
        xh = min(_half_w_at(y), _half_w_at(y + BAND_H))
        for sz, side in ((1, "f"), (-1, "b")):
            z_face = sz * HALF_D
            z_band = sz * (HALF_D - 0.22)
            rig.cube("body", f"band_{side}_{i}",
                     (-xh + 0.15, y, min(z_face, z_band)),
                     (xh - 0.15, y + BAND_H, max(z_face, z_band)), mat="seam")
        # 左右壁同高也要有带 —— 参考图的暗编纹是**绕着包身连续一圈**的，不是只
        # 在正反两面。前两轮只铺前后壁，差分实测侧视 seam 可见 0px：侧面渲成一
        # 整块素编身，收口锥度也跟着看不出来。左右带按同一 y/PITCH 铺，所以三面
        # 的暗线在 3/4 视角能对齐成一圈，而不是各走各的。
        for sx, side in ((-1, "l"), (1, "r")):
            x_face = sx * xh
            x_band = sx * (xh - 0.22)
            rig.cube("body", f"band_{side}_{i}",
                     (min(x_face, x_band), y, -HALF_D + 0.15),
                     (max(x_face, x_band), y + BAND_H, HALF_D - 0.15), mat="seam")
        y += PITCH
        i += 1


def part_braid(rig: Rig) -> None:
    """双股封边：包口一圈搓边，比编身亮一档 —— 「完好」的可辨证据。"""
    rig.bone("braid", (0.0, Y1, 0.0))

    # 两股上下叠，各自略外凸，形成搓绳的双棱。破草包是散口没有这个。
    for k, (yb, out) in enumerate(((Y1 - 0.62, 0.34), (Y1 - 0.02, 0.22))):
        eo = out
        rig.cube("braid", f"braid_f_{k}",
                 (-HALF_W - eo, yb, HALF_D - WALL - eo),
                 (HALF_W + eo, yb + 0.60, HALF_D + eo), mat="braid")
        rig.cube("braid", f"braid_b_{k}",
                 (-HALF_W - eo, yb, -HALF_D - eo),
                 (HALF_W + eo, yb + 0.60, -HALF_D + WALL + eo), mat="braid")
        rig.cube("braid", f"braid_l_{k}",
                 (-HALF_W - eo, yb, -HALF_D + WALL + eo),
                 (-HALF_W + WALL + eo, yb + 0.60, HALF_D - WALL - eo), mat="braid")
        rig.cube("braid", f"braid_r_{k}",
                 (HALF_W - WALL - eo, yb, -HALF_D + WALL + eo),
                 (HALF_W + eo, yb + 0.60, HALF_D - WALL - eo), mat="braid")


def part_strap(rig: Rig) -> None:
    """斜挎背带：搓绳一道弧，从左右封边起挂。

    **参考图（modelScript/ref/ 三视）里最显眼的部件，前两轮完全漏了** —— 三张
    图的画面占比里背带都仅次于包身，没它这件读作「放在地上的篮子」而不是穿戴容器。
    同系 GrassPouchBack.bbmodel 也没有带（它靠 vanilla ModelPart 直接贴在腰背，
    不需要），但本件要能单独立在 item 图标里，带就是必需的。

    形状按参考图：不是绕过肩的整圈（那要 20px 以上、爆 0..16 方块空间），而是
    **两段上挑的弧**，挂点在包口两侧封边，往上外侧收 —— 读作「带子往肩上去」，
    在 16px 里截断而不显得断裂。侧视图那条弧的顶点约在包口上方三分之一处。
    """
    rig.bone("strap", (0.0, Y1, 0.0))

    R = 0.42             # 搓绳半径：细于 ~0.45 在 16px 里渲不出一个像素（同 cord）
    z_s = -HALF_D + 0.55  # 贴背面挂，和翻盖前檐分开（否则正视糊成一块）

    # 每侧三段折线逼近弧：挂点 → 外挑 → 上收。用 shaft 逐段接，端点共享所以
    # 关节不会散（手写 from/to + rotation 会让端点绕 origin 飞走）。
    for sx, side in ((-1, "l"), (1, "r")):
        pts = [
            (sx * (HALF_W - 0.30), Y1 - 1.25, z_s),   # 挂点：咬进侧壁上段
            (sx * (HALF_W + 0.95), Y1 + 0.75, z_s),   # 外挑
            (sx * (HALF_W + 1.15), Y1 + 2.85, z_s),   # 上收（往肩去）
            (sx * (HALF_W + 0.55), Y1 + 4.30, z_s),   # 顶端：略回收，读作弧不是杆
        ]
        for k, (a, b) in enumerate(zip(pts, pts[1:]), start=1):
            rig.shaft("strap", f"strap_{side}_{k}", a, b, R, R, mat="cord")

        # 挂点绳结：一小段横箍，压住「带子系在封边上」这个交代。没有它带子
        # 看着是从壁里长出来的。
        rig.shaft("strap", f"strap_knot_{side}",
                  (sx * (HALF_W - 0.15), Y1 - 1.05, z_s - 0.55),
                  (sx * (HALF_W + 0.35), Y1 - 1.35, z_s + 0.55),
                  0.34, 0.34, mat="cord")


def part_sidepocket(rig: Rig) -> None:
    """侧插袋（**只在右侧**）：草编小兜，插草药露头。第二条可辨差异。"""
    rig.bone("sidepocket", (HALF_W, 2.4, 0.0))

    px0 = HALF_W - 0.30      # 咬进右壁 0.30px（薄贴，不算穿模）
    px1 = HALF_W + 1.15
    py0, py1 = 1.35, 4.30

    rig.cube("sidepocket", "pocket_body",
             (px0, py0, -1.25), (px1, py1, 1.25), mat="weave")
    # 兜口封边：和主体封边同材质，呼应「同一双手编的」
    rig.cube("sidepocket", "pocket_lip",
             (px0, py1 - 0.42, -1.40), (px1 + 0.16, py1 + 0.18, 1.40), mat="braid")
    # 兜身一道横缝，免得渲成一个素方块
    rig.cube("sidepocket", "pocket_band",
             (px1 - 0.22, py0 + 0.85, -1.10), (px1 + 0.02, py0 + 1.45, 1.10),
             mat="seam")

    # 露头草药：两根斜插。半径 0.42 —— 16px 空间里细于 ~0.45 的柱子渲不出
    # 一个像素直接消失（背篓 cord 半径注释同一个坑），故不敢再细。
    rig.shaft("sidepocket", "sprig_a",
              (px1 - 0.45, py1 - 0.30, 0.35), (px1 + 0.55, py1 + 2.10, 0.85),
              0.42, 0.42, mat="braid")
    rig.shaft("sidepocket", "sprig_b",
              (px1 - 0.50, py1 - 0.30, -0.45), (px1 + 0.20, py1 + 1.55, -1.05),
              0.42, 0.42, mat="braid")


def part_flap(rig: Rig) -> None:
    """翻盖：盖住上口，压向左前（刻意歪）。压出一道折痕。"""
    rig.bone("flap", (0.0, Y1, 0.0))

    # 压歪保留（设计意图），但**整片要压到最浅那端也咬住封边**。第一轮 −3.5°
    # 绕 z 转、盖底取 Y1+0.30：左端嵌入封边 0.54px 而右端只剩 0.02px，右侧渲出
    # 来整条翘着一道缝。倾角带来的两端高差 = 2·(HALF_W+0.45)·sin3.5° ≈ 0.49px，
    # 所以基准要比"刚好贴上"再低半像素，让高端也有 ≥0.35px 咬合。
    base = Y1 - 0.25
    # **盖比包身窄**（参考图三视都是这样：盖两侧各露出一截封边，一眼看出「盖是
    # 另一片盖上去的」）。前两轮做成比包身宽 0.45px —— 盖檐把封边整条罩住，正视
    # 读作一块顶板，双股封边这条可辨差异等于白做。改成每侧缩进 0.55px。
    fw = HALF_W - 0.55
    rig.cube("flap", "flap_top",
             (-fw, base, -HALF_D - 0.35),
             (fw, base + 0.75, HALF_D + 0.55),
             rot=(0.0, 0.0, FLAP_TILT), org=(0.0, base + 0.3, 0.0), mat="flap")
    # 前檐：垂下遮住前壁上沿。参考图垂得比前两轮深（约包高三成）。
    rig.cube("flap", "flap_front",
             (-fw + 0.22, base - 2.05, HALF_D + 0.05),
             (fw - 0.22, base + 0.12, FLAP_FRONT_Z),
             rot=(0.0, 0.0, FLAP_TILT), org=(0.0, base + 0.3, 0.0), mat="flap")
    # 檐角：两小块把方檐的角切短，逼近参考图的圆檐。
    for sx, side in ((-1, "l"), (1, "r")):
        rig.cube("flap", f"flap_corner_{side}",
                 (sx * (fw - 0.22), base - 1.42, HALF_D + 0.05),
                 (sx * fw, base + 0.12, HALF_D + 0.58),
                 rot=(0.0, 0.0, FLAP_TILT), org=(0.0, base + 0.3, 0.0), mat="flap")
    # 折痕：盖面上一道暗带，压 0.24px。没有它盖渲成一块素板。
    rig.cube("flap", "flap_crease",
             (-fw + 0.25, base + 0.51, -HALF_D + 0.10),
             (fw - 0.25, base + 0.75, HALF_D + 0.20),
             rot=(0.0, 0.0, FLAP_TILT), org=(0.0, base + 0.3, 0.0), mat="seam")


def part_cord(rig: Rig) -> None:
    """系盖两道草绳 + 骨扣。第三条可辨差异（破草包只有一个小扣）。"""
    rig.bone("cord", (0.0, Y1, 0.0))

    # 两道横绳：绕盖压住。半径 0.46 —— 细于 ~0.45 在 16px 里渲不出一个像素。
    # 绳高吃背篓那轮的教训（**压过头整条消失、压不够架成梁**，两个方向都踩过）。
    # 这里还多一层：盖是斜的，绳必须 **同角倾斜** 才能全长贴住。盖顶面局部高度
    # base+0.75，绳心比它高 0.36 → 底面咬进 0.10px、露背约 0.82px。
    # shaft() 不收 rot —— 它**从两端点自己解出朝向**。所以让绳跟着盖倾斜的正确
    # 做法是按倾角摆开两端高度，而不是套一个 rotation（手写 from/to + 手填
    # rotation 正是 shaft 要避开的那个坑：端点会绕 origin 飞走）。
    # 斜率**不手工推符号** —— 直接把盖顶面在两端的高度算出来，绳心 = 它 + 固定
    # 抬升。手推符号错过两次（照抄 FLAP_TILT 负号取反了方向；只翻 dy 正负等于把
    # 左右端对调、斜率没变），改成同一个函数求值就不可能再反。
    base = Y1 - 0.25
    x_end = HALF_W + 0.55
    LIFT = 0.36          # 绳心高出盖顶面：底面咬入 0.10px、露背 0.82px

    def _flap_top_at(x: float) -> float:
        """旋转后盖顶面在给定 x 处的高度（org=(0,base+0.3)，局部顶 base+0.75）。"""
        a = math.radians(FLAP_TILT)
        oy = base + 0.3
        return oy + (-x * math.sin(a) + (base + 0.75 - oy) * math.cos(a))

    for k, zc in enumerate((-0.85, 0.95)):
        rig.shaft("cord", f"lash_{k}",
                  (-x_end, _flap_top_at(-x_end) + LIFT, zc),
                  (x_end, _flap_top_at(x_end) + LIFT, zc),
                  0.46, 0.46, mat="cord")

    # 骨扣：**竖挂**在前檐外，taper 出两端骨节膨大的骨形（等截面柱读不出是骨）。
    # 前两轮做成横栓 —— 参考图三张全是竖挂的骨栓（上下各一个骨节、中腰一道绳结
    # 箍住），横栓读作一根横插的小棍，跟"扣"没关系。
    # 前伸沿用第二轮的结论：0.55px 前伸、坐进前檐 0.75px。第一轮 1.55px = 包深
    # 43%、与前檐只重叠 0.25px，SIDE 视图读作一块悬在外面的白方块。
    # z 位置**必须跟着前檐深度一起定**：前檐加深到 base−2.05 后，它的外表面到
    # z=2.40，而骨扣 pz=HALF_D+0.30 只到 2.56 —— 探出 0.16px，FRONT 实测骨扣仅
    # 115px 几乎被吞掉。骨扣是"扣在盖外面"的件，得整颗坐在前檐外表面之外：
    # 取前檐外表面 + 骨节半径，露出一整颗骨节的厚度。
    px = 0.0
    # 骨节半径 0.58（taper 首末段 rx=0.46 加 flat 展宽），骨心取「檐面 + 0.40」→
    # 骨背 z≈3.26、骨腹 z≈2.34 咬进檐面 0.06px。**别取 +0.52**：那样骨腹落在
    # 2.46 而檐面在 2.40，中间空 0.06px 是分离不是咬合（悬空门查 ≥2 轴重叠，
    # 骨扣与 peg_tie 重叠故不报，只能在这里算对）。
    pz = FLAP_FRONT_Z + 0.40
    py_mid = Y1 - 1.55        # 骨腰高度：落在前檐中段（前檐现在垂到 base−2.05）
    rig.taper("cord", "bone_peg",
              [(px, py_mid + 1.60, pz), (px, py_mid + 0.55, pz),
               (px, py_mid - 0.55, pz), (px, py_mid - 1.60, pz)],
              [0.58, 0.34, 0.34, 0.58], mat="bone", flat=1.0)

    # 腰箍绳结：**横箍在骨腰，不压骨节**。背篓那轮的教训是别把细长件劈成两半 ——
    # 这里骨栓竖着、腰段最细（半径 0.34 = 0.68px 宽），横绳压在腰上是参考图的
    # 系法，且压的是最细处不遮骨节，"两端粗中间细"的骨形照样读得出。
    rig.shaft("cord", "peg_tie",
              (px - 1.05, py_mid, pz - 0.12),
              (px + 1.05, py_mid, pz - 0.12),
              0.30, 0.30, mat="cord")


def part_stitch(rig: Rig) -> None:
    """针脚：**密而整齐**（对比破草包的散线）。缝在封边下与侧袋接缝上。"""
    rig.bone("stitch", (0.0, 0.0, 0.0))

    # 一排短竖针脚，前壁。间距均匀 = 「整齐」的可辨证据。
    #
    # **位置吃了一次实测教训**：前两轮缝在封边正下方（y≈6.05..6.68），而翻盖前檐
    # 垂到 y∈[4.94,7.44]、前表面 z=2.40 —— 整排针脚被盖檐罩死，差分实测七条合计
    # 只剩 15px 可见（等于没做）。改缝在**盖檐下沿之外**的裸露前壁上：盖檐底
    # y=4.94，往下留 0.25px 余量起针。参考图的针脚也正是缝在露出来的包身上，不是
    # 藏在盖底下。
    n = 7
    hem_y1 = 4.69                     # 盖檐下沿 4.94 − 0.25 余量
    for i in range(n):
        x = -HALF_W + 0.75 + i * (2 * HALF_W - 1.5) / (n - 1)
        rig.cube("stitch", f"hem_f_{i}",
                 (x - 0.16, hem_y1 - 0.63, HALF_D - 0.18),
                 (x + 0.16, hem_y1, HALF_D + 0.10), mat="stitch")

    # 侧袋接缝：袋body 贴壁那条竖缝，缝住不脱。
    # 同一个坑的另一半：缝面原本只到 x=3.56，而 pocket_lip 前表面 z=1.40、
    # pocket_body z=1.25 都比它更靠前，从正面/3-4 看整条被兜口包住。改成压在
    # **袋体外侧面**上（x 伸到 pocket_body 之外），朝 +x 露出来。
    # pocket_body 外壁在 x = HALF_W+1.15 = 4.65；针脚咬进去 0.18px、露出 0.12px。
    # 露太多会飘在兜外，露太少（<0.10）在 16px 里凑不满一个像素直接消失。
    px_out = HALF_W + 1.27
    for i in range(3):
        z = -0.85 + i * 0.85
        rig.cube("stitch", f"pocket_seam_{i}",
                 (px_out - 0.30, 1.55 + i * 0.05, z - 0.16),
                 (px_out, 4.05, z + 0.16), mat="stitch")


PARTS = {
    "body": part_body,
    "braid": part_braid,
    "strap": part_strap,
    "sidepocket": part_sidepocket,
    "flap": part_flap,
    "cord": part_cord,
    "stitch": part_stitch,
}
ORDER = ["body", "braid", "strap", "sidepocket", "flap", "cord", "stitch"]


def build(parts: list[str] | None = None) -> Rig:
    rig = Rig(MATS)
    for key in (parts or ORDER):
        PARTS[key](rig)
    return rig


def _shift_to_block_space(model: dict) -> None:
    """居中空间 → MC 方块空间：x/z +8，y 不动（地面已是 0）。"""
    for el in model["elements"]:
        for k in ("from", "to", "origin"):
            el[k][0] += 8.0
            el[k][2] += 8.0
    for node in model["outliner"]:
        node["origin"][0] += 8.0
        node["origin"][2] += 8.0


# ── 门禁声明：算法在 core/gatekit.py，这里只交代**本件的几何事实** ──────────
MATS_BY_COLOR = gatekit.mats_by_color(MATS)

# 侧袋外壁，侧袋针脚的宿主面
POCKET_OUT = HALF_W + 1.15

# 贴面件就位规格。符号方向是这道门唯一容易写反的地方：件长在宿主面**外侧**时，
# 「咬合」是两者的重叠厚度（宿主面 − 件的内侧面），不是件到宿主面的距离 —— 写成后者，
# 「把件往外推」这种明显的脱离反而会让数字变小（第一版就是这么反的：差分实测缺陷版
# 报 0、干净版报 2，符号一眼看出来错了）。
#
# 骨扣的门限单列 0.04：栓在檐外的硬件靠薄咬合站住（实测 0.06px），比编带压进壁里的
# 0.10 更浅是正常的，混用一个门限会误报干净模型。且只查两端粗头 —— 中间细腰 _02 是
# taper 收出来的束腰，本来就该比粗头浅 0.12px（实测 z 起 2.46 vs 前檐 2.40，差 0.06
# 悬空），但它整段被绳箍 peg_tie 罩住（箍 z 从 2.50 起、x±1.05 包住扣 x±0.46），
# 渲染上看不见那道缝。拿粗头当判据，缺陷来了照样抓得住。
#
# 兜身横缝 pocket_band 与盖折痕 flap_crease **刻意不列**：它们贴的是自己那件，
# 不是宿主壁，匹配不到 Seat 就不查。
SEATS = (
    Seat("bone_peg", axis=2, outward=+1, surface=FLAP_FRONT_Z,
         min_bite=0.04, max_bite=None, exclude=("bone_peg_02",), host="前檐"),
    Seat("band_f_", axis=2, outward=+1, surface=HALF_D,
         min_bite=0.10, max_bite=WALL, host="前壁"),
    Seat("hem_f_", axis=2, outward=+1, surface=HALF_D,
         min_bite=0.10, max_bite=WALL, host="前壁"),
    Seat("band_b_", axis=2, outward=-1, surface=-HALF_D,
         min_bite=0.10, max_bite=WALL, host="后壁"),
    Seat("band_r_", axis=0, outward=+1, surface=_half_w_at,
         min_bite=0.10, max_bite=WALL, host="右壁"),
    Seat("band_l_", axis=0, outward=-1, surface=lambda y: -_half_w_at(y),
         min_bite=0.10, max_bite=WALL, host="左壁"),
    Seat("pocket_seam_", axis=0, outward=+1, surface=POCKET_OUT,
         min_bite=0.10, max_bite=WALL, host="侧袋外壁"),
)

# 软覆盖白名单：只放行「软覆盖硬」的设计意图。**关键是留下硬对硬不放行** ——
# bone×flap / bone×braid / bone×weave 一律不列：骨扣扎穿翻盖或封边是缺陷不是构造。
SOFT_OVER = frozenset({
    # 编带/编缝压壁、封边罩口：本就该薄嵌
    frozenset(("seam", "weave")), frozenset(("braid", "weave")),
    frozenset(("braid", "seam")),
    # 盖罩在编身上、盖压封边：软覆盖
    frozenset(("flap", "weave")), frozenset(("flap", "seam")),
    frozenset(("flap", "braid")),
    # 绳捆盖/绕身/绕骨扣
    frozenset(("cord", "flap")), frozenset(("cord", "weave")),
    frozenset(("cord", "seam")), frozenset(("cord", "braid")),
    frozenset(("cord", "bone")),
    # 针脚咬壁/咬封边/咬盖
    frozenset(("stitch", "weave")), frozenset(("stitch", "seam")),
    frozenset(("stitch", "braid")), frozenset(("stitch", "flap")),
})

GATES = gatekit.AssetGates(
    "小草包 / grass_pouch",
    MATS,
    # 刻意不对称件（盖压歪、右侧插袋、针脚）走白名单排除 —— 那是设计意图不是缺陷
    asym=ASYM,
    # 露头草药上段刻意伸出兜口（悬在空中读作「插着的草」）
    free_floating=frozenset({"sprig_a", "sprig_b"}),
    soft_over=SOFT_OVER,
    seats=SEATS,
    seat_materials=frozenset({"seam", "stitch", "bone"}),
)

NOTE = ("注：立体感/比例/与破草包的可辨差异，自检量不出 —— 必须人眼看 "
        "render_bbmodel.py 三视图定夺。")


def check(rig: Rig) -> int:
    """七道门：孤儿 / 越界 / 退化薄片 / 悬空 / 穿模 / 贴面就位 / 对称件镜像。"""
    return GATES.report(rig, px=PX, note=NOTE)




def main() -> int:
    ap = argparse.ArgumentParser(description="小草包 bbmodel 生成器")
    ap.add_argument("--part", help="只生成单件（调试用）")
    ap.add_argument("--check", action="store_true", help="只跑自检，不写盘")
    ap.add_argument("--self-test", action="store_true",
                    help="差分自证：每道门注入它该抓的缺陷，报不出来就算这道门失效")
    ap.add_argument("--list", action="store_true", help="列出所有部件")
    args = ap.parse_args()

    if args.list:
        for k in ORDER:
            doc = (PARTS[k].__doc__ or "").strip().splitlines()[0]
            print(f"  {k:11s} {doc}")
        return 0

    if args.part and args.part not in PARTS:
        print(f"未知部件 {args.part}；可选：{', '.join(ORDER)}")
        return 2
    parts = [args.part] if args.part else None

    rig = build(parts)
    if args.self_test:
        return 1 if GATES.self_test(rig) else 0

    bad = check(rig)
    if args.check:
        return 1 if bad else 0

    name = f"GrassPouch_{args.part}" if args.part else "GrassPouch"
    model = rig.bbmodel(name)
    _shift_to_block_space(model)
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / f"{name}.bbmodel"
    out.write_text(json.dumps(model, ensure_ascii=False, indent=1))
    print(f"  → {out.relative_to(REPO)} ({out.stat().st_size} B)")
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
