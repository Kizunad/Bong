#!/usr/bin/env python3
"""小刀三件套：石刃 / 凡铁匕首 / 骨刺的 bbmodel + SML OBJ 双出。

参考图：`orthograph #8`（石刃：打制燧石刃 + 劈开木柄 + 绳缠箍）、
`orthograph #9`（凡铁匕首：锻打起棱刃 + 金属护环 + 单铆钉木柄）、
`orthograph #10`（骨刺：劈开的长骨，保留关节头当握把，尖端磨出斜纹）。

三件都是 `server/assets/items/*.toml` 里的真物品，此前全在借原版皮：

    stone_knife  石刃      → item/stone_sword
    iron_dagger  凡铁匕首  → **注册表里根本没有，手持渲染是空手**
    bone_spike   骨刺      → item/bone（且在新手默认 loadout，开局第一眼）

## 参考图标定

三张图都是单件正 / 侧 / 背，没有人体做尺，所以只能取**比例**，绝对尺寸按
MC 手持物基线（axe_bone 全长 0.80、scale 0.85）定。逐段量色分出的分界：

    石刃    刃 41% / 绳缠 17% / 木柄 38%（余下 4% 是绳缠上方露出的劈开木口）
            前视长宽比 5.90，侧/正宽比 0.62
    铁匕首  刃 61% / 护环 2% / 木柄 37%；刃最宽处在护环往上 34% 处
            前视长宽比 6.90，侧/正宽比 0.71
    骨刺    关节头 19% / 骨干 64% / 磨尖 17%；骨干宽只有关节头的 51%
            前视长宽比 6.25，侧/正宽比 0.77

骨刺在参考图里关节头朝上、尖朝下，**这里整个翻过来**：手持物约定尖端朝 +Y，
关节头当握把落在 y=0（`assert_conventions` 会查）。

## 刻意偏离参考的地方

- **全长压到 0.76~0.84**，不照参考的细长比。MC 手持物的显示框就那么大，
  照 6.9:1 做出来在手里是一根针，GUI 图标里更是一条线。
- **刃厚放到 0.017~0.024**（按比例只该 0.008）。低于 0.015 的片在 MC 的
  item 光照下侧面几乎不受光，转到侧视整片刃会"消失"一帧。

## 贴图明度未经真机标定

这套走 SML item 光照，和护甲那条（方块分轴着色）不是一回事，我这里也跑不了
`client/tools/render_held_item.py`（缺 pyrender）。所以贴图只按参考量色 ×1.25
放亮，**这个系数是从既有手持物贴图（gen_shield_models 的木纹/骨纹）反推的经验值，
不是实测**，进游戏后大概率还要调一轮。
"""

from __future__ import annotations

import argparse
import random
import sys
from pathlib import Path

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "core"))

from bbmodel_maker.render.held_item_common import (  # noqa: E402
    Box,
    HeldItem,
    Material,
    blotch,
    build_mtl,
    build_obj,
    hand_display,
    noise_fill,
    write_assets,
)

REPO = Path(__file__).resolve().parents[2]
BBMODEL_DIR = Path(__file__).resolve().parents[1] / "models"
PREVIEW_DIR = Path(__file__).resolve().parents[1] / "out"
CLIENT_RESOURCES = REPO / "client" / "src" / "main" / "resources"


# ── 贴图 ──────────────────────────────────────────────────────────────────
# 每张 16²。OBJ 那条链是**每个面整张铺满**，所以只能画通用材质样本，
# 不能画"某个面的具体图案"——画了会被按面拉伸成条带。
#
# 底噪 / 斑两个原语在 `held_item_common`（`noise_fill` / `blotch`）：木棍那件要用
# 同一套画法，留在这里就成了本模块 docstring 里骂的那种"两套各写一份"。


def tex_stone_flake() -> Image.Image:
    """打制石刃：贝壳状剥片面。参考量得 H36 S12 V51，×1.25 放亮。

    石片的特征是**大块的平面 + 平面之间的棱**，不是均匀噪点；所以先铺底噪，
    再叠几块边界清楚的多边形亮/暗斑当剥片疤。
    """
    rng = random.Random(0x5701)
    img = Image.new("RGBA", (16, 16), (168, 161, 150, 255))
    noise_fill(img, rng, (168, 161, 150), 7, warm=2)
    blotch(img, rng, 5, (196, 191, 182), (2.6, 4.4))     # 新剥片：亮
    blotch(img, rng, 4, (132, 126, 118), (2.2, 3.8))     # 旧面：暗
    pixels = img.load()
    for _ in range(9):                                     # 棱：1px 亮线
        x0, y0 = rng.randint(0, 15), rng.randint(0, 15)
        dx, dy = rng.choice(((1, 1), (1, -1), (1, 0), (0, 1)))
        for step in range(rng.randint(3, 7)):
            x, y = x0 + dx * step, y0 + dy * step
            if 0 <= x < 16 and 0 <= y < 16:
                pixels[x, y] = (206, 202, 194, 255)
    return img


def tex_haft_dark() -> Image.Image:
    """石刃的木柄：风化开裂的深色木，竖向纹。参考 H33 S45 V37。"""
    rng = random.Random(0x5702)
    img = Image.new("RGBA", (16, 16), (123, 100, 70, 255))
    noise_fill(img, rng, (123, 100, 70), 8, warm=4)
    blotch(img, rng, 4, (99, 79, 54), (2.2, 4.0))         # 风化的暗块
    blotch(img, rng, 3, (148, 122, 88), (1.8, 3.2))       # 磨亮处
    pixels = img.load()
    # 木纹只画 2 条、断续、对比压到 8 个色阶。round 1 画成 2px 一条的连续竖条纹，
    # 手持物那个尺寸上整根柄读成"一捆木棍"，石刃那把直接成了火把；round 2 收到
    # 3 条仍嫌密——柄面在屏幕上不过十来像素，一张 16² 图铺上去，超过两条竖线
    # 就必然读成条纹布。
    for x in (4, 11):
        for y in range(16):
            if (x * 7 + y * 5) % 4:                        # 断续
                pixels[x, y] = (115, 94, 66, 255)
    for _ in range(2):                                     # 裂
        x = rng.randint(1, 14)
        for y in range(rng.randint(0, 6), rng.randint(9, 16)):
            pixels[x, y] = (72, 57, 38, 255)
    return img


def tex_cord() -> Image.Image:
    """绳缠：搓过的植物纤维，斜向拧纹。参考 H35 S45 V42。"""
    rng = random.Random(0x5703)
    img = Image.new("RGBA", (16, 16), (140, 114, 77, 255))
    noise_fill(img, rng, (140, 114, 77), 7, warm=3)
    pixels = img.load()
    for start in range(-8, 16, 3):
        for step in range(16):
            x, y = start + step, 15 - step
            if 0 <= x < 16 and 0 <= y < 16:
                pixels[x, y] = (114, 92, 60, 255)
    for start in range(-6, 16, 6):
        for step in range(16):
            x, y = start + step, 15 - step
            if 0 <= x < 16 and 0 <= y < 16:
                pixels[x, y] = (172, 145, 102, 255)
    return img


def tex_iron_forged() -> Image.Image:
    """锻打铁刃：锤痕面 + 锈斑。参考 H29 S20 V47（锈处 S 到 42）。"""
    rng = random.Random(0x5704)
    img = Image.new("RGBA", (16, 16), (132, 121, 110, 255))
    noise_fill(img, rng, (132, 121, 110), 9, warm=4)
    blotch(img, rng, 4, (176, 167, 156), (2.4, 4.2))      # 锤面高光
    # 暖偏不能去：round 1 做成纯灰（S10），渲出来和石刃那把分不开，读成石头。
    # 但锈也不能给满——round 2 给了 9 块大斑，96px 下整片刃读成"锈成一坨的铁块"
    # 而不是刃。参考图的锈是**沿棱和凹处的小片**，中间的锻面是干净的：这里收到
    # 5 块小斑（约两成面积）+ 3 条 1px 的棱锈线。
    # round 2 除了斑还画了 3 条 45° 的"沿棱锈线"，与参考并排一看是**斜划痕**不是
    # 锈——真锈是斑不是线，规则的等角斜线在任何尺度下都读成刮蹭。全删，只留斑，
    # 并把底色从 V59 压到 V52（参考量得 V47，×1.25 该是 V59；但参考那把是**深色
    # 重锈**的旧铁，纯按系数放亮会得到一把崭新的钢刀）。
    blotch(img, rng, 7, (126, 76, 42), (1.1, 2.2))        # 锈斑：多而小
    blotch(img, rng, 3, (78, 72, 66), (1.6, 2.8))         # 暗坑
    return img


def tex_haft_pale() -> Image.Image:
    """铁匕首的木柄：新削的浅色木，比石刃那把亮一大截。参考 H32 S37 V65。"""
    rng = random.Random(0x5705)
    img = Image.new("RGBA", (16, 16), (206, 172, 130, 255))
    noise_fill(img, rng, (206, 172, 130), 7, warm=4)
    blotch(img, rng, 4, (180, 148, 108), (2.2, 4.2))
    pixels = img.load()
    for x in (4, 11):                                      # 只两条断续纹，同上
        for y in range(16):
            if (x * 3 + y * 7) % 3:
                pixels[x, y] = (186, 154, 114, 255)
    return img


def tex_iron_dark() -> Image.Image:
    """护环与铆钉：比刃暗一档的熟铁，少锈（常摩擦）。"""
    rng = random.Random(0x5706)
    img = Image.new("RGBA", (16, 16), (112, 108, 104, 255))
    noise_fill(img, rng, (112, 108, 104), 9)
    blotch(img, rng, 3, (150, 146, 142), (2.2, 4.0))
    blotch(img, rng, 2, (84, 62, 44), (1.4, 2.4))
    return img


def tex_bone_shaft() -> Image.Image:
    """骨干：奶白，细密纵纹 + 淡褐土沁。参考 H39 S27 V72。"""
    rng = random.Random(0x5707)
    img = Image.new("RGBA", (16, 16), (229, 214, 182, 255))
    noise_fill(img, rng, (229, 214, 182), 6, warm=3)
    blotch(img, rng, 5, (204, 186, 150), (2.4, 4.6))
    pixels = img.load()
    for x in (5, 12):                                      # 纵纹只两条、断续、低对比
        for y in range(16):
            if (x * 5 + y * 3) % 3:
                pixels[x, y] = (214, 198, 166, 255)
    blotch(img, rng, 4, (186, 162, 118), (1.8, 3.4))      # 土沁
    return img


def tex_bone_joint() -> Image.Image:
    """关节头：疏松骨质 + 更重的染色，明显比骨干脏。参考 H34 S51 V60。"""
    rng = random.Random(0x5708)
    img = Image.new("RGBA", (16, 16), (199, 172, 124, 255))
    noise_fill(img, rng, (199, 172, 124), 9, warm=5)
    blotch(img, rng, 6, (160, 128, 82), (1.8, 3.6))
    blotch(img, rng, 3, (222, 202, 164), (1.6, 2.8))
    return img


def tex_bone_ground() -> Image.Image:
    """磨出来的尖：斜向磨痕。参考图那一段的斜纹是这把刀最认得出的做工痕迹。"""
    rng = random.Random(0x5709)
    img = Image.new("RGBA", (16, 16), (221, 205, 170, 255))
    noise_fill(img, rng, (221, 205, 170), 6, warm=2)
    pixels = img.load()
    for start in range(-10, 16, 2):                        # 斜磨痕，1px
        for step in range(16):
            x, y = start + step, step
            if 0 <= x < 16 and 0 <= y < 16:
                pixels[x, y] = (190, 172, 136, 255)
    return img


# ── 为什么三把都比参考"粗"一大截 ────────────────────────────────────────
# 参考图是实物照，长宽比 5.9 / 6.9 / 6.25。**照抄进 MC 是错的**：手持物在屏幕上
# 就三十来像素，6.9:1 渲出来是一根针；round 1 三把分别读成了火把、针、电视塔。
# 这里统一压到 3.9~4.7:1（原版剑连护手大约就是 4:1），保住"这是把刀"的读法，
# 代价是失掉参考那种细长的考古感——这是 MC 手持物尺寸的硬下限，不是没做够。
#
# 刃厚同理放到 0.019~0.024（按参考比例只该 0.008）：低于 0.015 的片在 item 光照
# 下侧面几乎不受光，转到侧视整片刃会"消失"。
#
# 收刃**不用等分多段**。round 1 每把七段等长，段高和段间宽差同量级，渲出来是
# 一道楼梯。改成"根部一小段 + 中段一长条等宽 + 尖部两三段快收"：中段那条长的
# 才是眼睛读到的"刃"，楼梯感被挤进最后 30%，正好是真刀该收尖的地方。


# ── 石刃 stone_knife（全长 0.72，长宽比 3.9:1）────────────────────────────
# 刃 y0.400~0.722 / 绳缠 0.274~0.400 / 木柄 0~0.274。
# 刃分四段，每段的 x 半宽和中心都不同——参考图那片燧石是**不对称**的（一边直、
# 一边弧），左右对称写出来就成了一把工业冲压的小刀，正是这把最不该有的味道。
STONE_KNIFE = HeldItem(
    key="stone_knife",
    display_name="石刃",
    host_item="stone_sword",
    boxes=(
        Box("handle_butt", "stone_haft", (0.0, 0.0300, 0.0), (0.0520, 0.0300, 0.0400)),
        Box("handle_low", "stone_haft", (0.0, 0.1050, 0.0), (0.0596, 0.0480, 0.0458)),
        Box("handle_up", "stone_haft", (0.0, 0.2140, 0.0), (0.0648, 0.0655, 0.0502)),
        # 绳缠四道。厚度交替 0.058/0.055，x 中心左右各偏 0.004：等厚等宽的四道会
        # 读成"车出来的四道凹槽"，参差才读成一圈圈绕上去的绳。
        Box("wrap_a", "stone_cord", (-0.0040, 0.2900, 0.0), (0.0715, 0.0168, 0.0580)),
        Box("wrap_b", "stone_cord", (+0.0043, 0.3235, 0.0), (0.0706, 0.0164, 0.0554)),
        Box("wrap_c", "stone_cord", (-0.0038, 0.3560, 0.0), (0.0710, 0.0161, 0.0575)),
        Box("wrap_d", "stone_cord", (+0.0041, 0.3862, 0.0), (0.0698, 0.0158, 0.0549)),
        # 劈开的木口：两瓣夹住刃根，中间留缝——参考图侧视看得见那道缝。
        Box("ferrule_l", "stone_haft", (-0.0475, 0.4180, 0.0), (0.0262, 0.0242, 0.0472)),
        Box("ferrule_r", "stone_haft", (+0.0483, 0.4180, 0.0), (0.0255, 0.0236, 0.0464)),
        # 燧石刃四段：根 → 长中段（最宽，眼睛读到的就是它）→ 收 → 尖
        Box("blade_root", "stone_flake", (+0.0030, 0.4285, 0.0), (0.0782, 0.0325, 0.0240)),
        Box("blade_belly", "stone_flake", (+0.0044, 0.5120, 0.0), (0.0918, 0.0520, 0.0224)),
        Box("blade_mid", "stone_flake", (+0.0018, 0.6020, 0.0), (0.0724, 0.0398, 0.0189)),
        # 尖再收一档、拉长一档：round 2 的 tip 宽 0.076 高 0.072 接近正方，
        # 与参考并排是个"钝头"，参考那片燧石是明确收出来的偏锋尖。
        Box("blade_upper", "stone_flake", (+0.0044, 0.6620, 0.0), (0.0498, 0.0212, 0.0156)),
        Box("blade_tip", "stone_flake", (+0.0086, 0.7005, 0.0), (0.0268, 0.0215, 0.0112)),
    ),
    materials=(
        Material("stone_flake", (0.66, 0.63, 0.59), tex_stone_flake()),
        Material("stone_haft", (0.48, 0.39, 0.27), tex_haft_dark()),
        Material("stone_cord", (0.55, 0.45, 0.30), tex_cord()),
    ),
    # 拳心对准木柄+绳缠段的中点（柄 0~0.274 / 绳缠 0.274~0.400）：0.72 的显示
    # 缩放下拳头占模型 0.25/0.72 = 0.347 方块，正好被这段 0.400 的握把吃住。
    grip=0.20,
    display=hand_display(0.72, 0.20, 0.722),
)


# ── 凡铁匕首 iron_dagger（全长 0.775，长宽比 4.7:1）───────────────────────
# 刃 0.292~0.775 / 护环 0.269~0.293 / 木柄 0~0.269。
# 刃对称（锻件不是石片），最宽段落在护环往上 28%~46% 那一整条（参考实测最宽点
# 在 34%）——摆到中点会读成柳叶刀，摆到根部读成矛头。
IRON_DAGGER = HeldItem(
    key="iron_dagger",
    display_name="凡铁匕首",
    host_item="iron_ingot",
    boxes=(
        Box("handle_butt", "dagger_haft", (0.0, 0.0200, 0.0), (0.0478, 0.0200, 0.0398)),
        # 顶到 0.2695 而不是 0.2635：护环底在 0.2688，原值与它之间留着 0.0053 的缝，
        # 和本段注释自己写的「木柄 0~0.269」也对不上。`assert_boxes_are_connected`
        # 上线时揪出来的——整条刃 + 护环因此是**一块和柄不相连的浮空体**。
        Box("handle_body", "dagger_haft", (0.0, 0.1525, 0.0), (0.0542, 0.1170, 0.0438)),
        # 单铆钉：参考图在柄下段约 88% 处，前后两面各露一个头 → 一根穿透的钉。
        Box("rivet", "dagger_fitting", (0.0, 0.0600, 0.0), (0.0132, 0.0132, 0.0476)),
        Box("guard", "dagger_fitting", (0.0, 0.2810, 0.0), (0.0788, 0.0122, 0.0518)),
        Box("blade_root", "dagger_iron", (0.0, 0.3178, 0.0), (0.0598, 0.0258, 0.0238)),
        Box("blade_body", "dagger_iron", (0.0, 0.4280, 0.0), (0.0822, 0.0855, 0.0223)),
        Box("blade_taper", "dagger_iron", (0.0, 0.5560, 0.0), (0.0661, 0.0428, 0.0184)),
        Box("blade_upper", "dagger_iron", (0.0, 0.6520, 0.0), (0.0458, 0.0535, 0.0146)),
        Box("blade_tip", "dagger_iron", (0.0, 0.7400, 0.0), (0.0212, 0.0350, 0.0096)),
    ),
    materials=(
        Material("dagger_iron", (0.59, 0.54, 0.49), tex_iron_forged()),
        Material("dagger_haft", (0.81, 0.67, 0.51), tex_haft_pale()),
        Material("dagger_fitting", (0.44, 0.42, 0.41), tex_iron_dark()),
    ),
    # 木柄只有 0~0.269，比一个拳头(0.25/0.74 = 0.338)还短——护环正好压在拳头
    # 上沿，这是护环该在的位置；拳心取 0.14 让柄尾露出一点点。
    grip=0.14,
    display=hand_display(0.74, 0.14, 0.775),
)


# ── 骨刺 bone_spike（全长 0.795，长宽比 4.5:1）────────────────────────────
# 参考图里关节头朝上、尖朝下，这里整个翻过来（手持物约定尖朝 +Y）。
# 关节头 0~0.16 / 骨干 0.156~0.53 / 磨尖 0.528~0.795。
# 关节头做成两瓣不等大的髁——真骨的远端关节就是两个不对称的球，做成一个方块
# 就成了"棒槌"。骨干带一道背面的纵沟：参考图侧视能看到它是**劈开的半根骨**，
# 里面是空的，那道沟是整件最认得出"这是骨不是木棍"的地方。
BONE_SPIKE = HeldItem(
    key="bone_spike",
    display_name="骨刺",
    host_item="bone",
    boxes=(
        # 关节头：round 2 做成两个等大的方块，96px 下只读成"稍宽的底座"。真骨的
        # 远端关节是**两个不等大的球 + 中间一道髁间窝**，这里用大小/高低/前后都
        # 错开的三块 + 一顶帽做出来：大髁最高最靠前，小髁矮半格且后缩，中间那块
        # 窄的把两瓣连起来并留出可见的凹。
        # round 2 把大髁往 -x 甩了 0.0455、还压在最底，正视整个头歪向一边，读成
        # 一只靴子。这轮把两髁收回中线附近（±0.030），改用**高度差**而不是偏移
        # 差来做不对称：大髁高 0.104、小髁 0.078，中间的髁间窝更浅。
        Box("condyle_major", "spike_joint", (-0.0300, 0.0520, -0.0055), (0.0472, 0.0520, 0.0520)),
        Box("condyle_minor", "spike_joint", (+0.0332, 0.0430, +0.0068), (0.0428, 0.0390, 0.0452)),
        Box("intercondylar", "spike_joint", (+0.0022, 0.0330, 0.0), (0.0188, 0.0272, 0.0368)),
        Box("condyle_cap", "spike_joint", (-0.0232, 0.1008, -0.0038), (0.0356, 0.0172, 0.0408)),
        Box("joint_neck", "spike_joint", (0.0, 0.1418, 0.0), (0.0468, 0.0342, 0.0416)),
        Box("shaft_low", "spike_bone", (0.0, 0.2620, -0.0050), (0.0468, 0.0862, 0.0372)),
        Box("shaft_up", "spike_bone", (0.0, 0.4360, -0.0032), (0.0424, 0.0928, 0.0334)),
        # 背面的纵沟：劈开的半骨露出的髓腔。压进骨干体内，只露一道暗缝。
        Box("marrow_groove", "spike_joint", (0.0, 0.3400, +0.0292), (0.0112, 0.1800, 0.0118)),
        Box("point_base", "spike_ground", (0.0, 0.5900, 0.0), (0.0358, 0.0624, 0.0281)),
        Box("point_mid", "spike_ground", (0.0, 0.6900, 0.0), (0.0241, 0.0398, 0.0192)),
        Box("point_tip", "spike_ground", (0.0, 0.7620, 0.0), (0.0113, 0.0332, 0.0091)),
    ),
    materials=(
        Material("spike_bone", (0.90, 0.84, 0.71), tex_bone_shaft()),
        Material("spike_joint", (0.78, 0.67, 0.49), tex_bone_joint()),
        Material("spike_ground", (0.87, 0.80, 0.67), tex_bone_ground()),
    ),
    # 握把是关节头(0~0.16)加一小段骨干：拳心取 0.14，髁压在掌心、虎口卡在骨干上。
    grip=0.14,
    display=hand_display(0.76, 0.14, 0.795),
)


def items() -> tuple[HeldItem, ...]:
    return (STONE_KNIFE, IRON_DAGGER, BONE_SPIKE)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--no-preview", action="store_true", help="只写 bbmodel/资源，不出图")
    parser.add_argument("--install", action="store_true",
                        help="【当前会直接报错】写进 client 资源树。三把刀的宿主全部"
                             "撞车，装机前必须先落地 plan-held-item-registration-v1")
    parser.add_argument("--dump-obj", metavar="KEY", help="把某件的 OBJ/MTL 打到 stdout")
    args = parser.parse_args()

    if args.dump_obj:
        chosen = next((i for i in items() if i.key == args.dump_obj), None)
        if chosen is None:
            raise SystemExit(f"没有这件：{args.dump_obj}（可选 {[i.key for i in items()]}）")
        print(build_obj(chosen))
        print(build_mtl(chosen))
        return

    if args.install:
        # **这三把刀现在一件也装不了**，不是忘了装，是宿主机制没有空位：
        #
        #   stone_knife → item/stone_sword   已被 6 个模板共用（bone_sword / gua_dao /
        #                                    三把 flawed 剑），装上去它们会一起变成石刃
        #   bone_spike  → item/bone          已指向 bone_dagger，装上去匕首变骨刺
        #   iron_dagger → item/iron_ingot    没被占，但那是**玩家真会看到的原版物品**，
        #                                    劫持它等于全服铁锭长成匕首
        #
        # 换个"没人用的冷门 vanilla item"只是把问题推后一件——剩下 22 件借皮的模板
        # 没那么多冷门 item 可烧（见 plan-held-item-registration-v1 §1.3）。
        # 用户 2026-08-24 裁决：根治，即每个模板注册自己的 render-only Item。
        #
        # `held_item_common.assert_host_is_claimable` 那道闸也会拦住前两件，但它给不出
        # 第三件的理由（iron_ingot 确实没被占），所以在这里连同背景一起说清。
        raise SystemExit(
            "拒绝 --install：三把刀的 vanilla 宿主全部撞车（stone_sword 被 6 个模板共用 / "
            "bone 已指向 bone_dagger / iron_ingot 是玩家可见的原版物品）。\n"
            "装机前先落地 docs/plans-skeleton/plan-held-item-registration-v1.md，"
            "让每个模板注册自己的 render-only Item。"
        )

    outputs = write_assets(
        items(),
        bbmodel_dir=BBMODEL_DIR,
        client_resources=None,      # 见上面 --install 那段：现在一件也装不了
        preview_dir=PREVIEW_DIR,
        render_previews=not args.no_preview,
    )
    for key, path in outputs.items():
        print(f"[{key}] {path.relative_to(REPO)}")


if __name__ == "__main__":
    main()
