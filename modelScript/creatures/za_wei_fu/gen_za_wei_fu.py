#!/usr/bin/env python3
"""杂尾蝠（za_wei_fu）Blockbench .bbmodel 程序化生成器 —— Round 3/3（翼膜暗褐带血管纹、撕裂代替破洞、侧视加厚），只做几何。

正典依据：docs/worldview.md §十三 幽暗地穴——地穴里成群的蝙蝠，循真元波动扑人。任务卡：末世生物批次 #7
（.task-wasteland-creatures.md）「尾巴杂接着别种兽尾的地穴蝙蝠，循真元波动扑人，成群」。

外形：瘦小的灰黑蝙蝠，比原版大一圈（翼展约 36px，加耳高约 37px）。翼膜是暗褐皮膜（逐格体素铺出，
带血管纹、斜向撕裂和参差的后缘），翼骨细长，每侧翅有肩 / 上臂 / 前臂 / 腕 / 拇指爪 / 三根指骨；大耳朵（里面是粉色耳肉）、
塌鼻、满口细牙、一点红的眼。特征是尾巴不是它自己的：接上来的一截——鼠尾（裸皮，带环纹和稀毛）→ 蜥尾（鳞片 +
背棘）→ 蛇尾（深色细鳞、浅色腹纹，尾尖卷起），三个接口各有红肉缝圈、黑线针脚和十字骨钉。

坐标：1 block = 16px，+y 向上，正面朝 -z（GeckoLib 约定），+x 为蝙蝠的右侧（骨名 *_r）。悬停姿态：
身体直立、翼向两侧撑开、尾巴垂在身后下方。破膜的破洞左右不同（刻意不对称），其余部件左右镜像。
分部件：part_*() 逐件写，--part 单件预览；骨架在 BONES 里一处定义。

用法：
    python3 modelScript/creatures/za_wei_fu/gen_za_wei_fu.py --self-test   # 门禁差分自证
    python3 modelScript/creatures/za_wei_fu/gen_za_wei_fu.py --part head    # 单件预览
    python3 modelScript/creatures/za_wei_fu/gen_za_wei_fu.py                # 全量：模型 + 接触表
"""

from __future__ import annotations

import argparse
import copy
import importlib.util
import json
import math
import sys
from pathlib import Path

from PIL import Image, ImageDraw

from bbmodel_maker.gates import gatekit
from bbmodel_maker.render.render_bbmodel import render
from bbmodel_maker.rig.rigkit import Rig

REPO = Path(__file__).resolve().parents[3]
CREATURE_DIR = Path(__file__).resolve().parent
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/creatures/za_wei_fu")
PARTS_DIR = REVIEW_DIR / "parts"

sys.path.insert(0, str(REPO / "modelScript" / "tools"))
from cjk_font import load_font  # noqa: E402

# 仓库内的共面 (Z-fighting) 门禁：modelScript/bbmodel_maker 与已安装的同名库同名，按路径单独加载
_spec = importlib.util.spec_from_file_location(
    "repo_coplanar", REPO / "modelScript" / "bbmodel_maker" / "gates" / "coplanar.py")
coplanar = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(coplanar)

MODEL_ID = "za_wei_fu"

# ───────────────────────────────────────────────────────────── 材质（RGB，每种占贴图一个 8×8 色块）
MATS = {
    "fur_dk":      (46, 44, 48),      # 灰黑主毛
    "fur":         (68, 64, 68),
    "fur_lt":      (98, 92, 94),      # 毛尖 / 颊毛
    "belly":       (156, 144, 130),   # 胸前浅色 V 纹
    "membrane":    (80, 64, 52),      # 翼膜（暗褐皮膜，Round 3 由偏红改为暗褐）
    "membrane_lt": (100, 80, 62),
    "membrane_vein": (100, 54, 46),   # 膜上的血管纹（暗红，细线）
    "muscle":      (124, 60, 58),     # 翼臂肌腱
    "ear_skin":    (172, 112, 110),   # 耳内粉肉
    "ear_vein":    (142, 86, 88),
    "nose":        (160, 92, 92),
    "void":        (20, 18, 20),      # 鼻孔 / 口腔深处
    "eye":         (18, 14, 16),
    "eye_red":     (216, 66, 48),     # 眼里一点红（靠对比度）
    "tooth":       (234, 228, 208),
    "gum":         (148, 66, 72),     # 牙龈 / 舌
    "bone":        (190, 178, 152),   # 翼骨
    "bone_lt":     (216, 206, 180),
    "bone_dk":     (142, 132, 110),
    "claw":        (44, 40, 40),
    "rat":         (162, 124, 118),   # 鼠尾裸皮
    "rat_dk":      (128, 94, 92),
    "hair":        (52, 42, 42),      # 鼠尾稀毛
    "flesh_ring":  (152, 62, 60),     # 缝合处的红肉圈
    "thread":      (28, 24, 26),      # 缝线
    "pin":         (214, 202, 172),   # 骨钉
    "lizard":      (98, 108, 74),
    "lizard_dk":   (68, 80, 54),
    "lizard_lt":   (130, 136, 94),
    "snake":       (58, 64, 66),
    "snake_dk":    (38, 42, 46),
    "snake_lt":    (148, 142, 118),   # 蛇腹浅纹
}
# 允许有薄重叠的材质对（缝线 / 骨钉压在肉圈和尾皮上，膜覆在骨 / 肌腱上，毛盖毛）
_SOFT = (
    ("thread", "flesh_ring"), ("thread", "rat"), ("thread", "rat_dk"), ("thread", "lizard"),
    ("thread", "lizard_dk"), ("thread", "snake"), ("thread", "snake_dk"), ("thread", "pin"),
    ("pin", "flesh_ring"), ("pin", "rat"), ("pin", "lizard"), ("pin", "snake"), ("pin", "rat_dk"),
    ("flesh_ring", "rat"), ("flesh_ring", "rat_dk"), ("flesh_ring", "lizard"), ("flesh_ring", "snake"),
    ("flesh_ring", "lizard_dk"), ("flesh_ring", "snake_dk"), ("flesh_ring", "fur_dk"), ("flesh_ring", "fur"),
    ("membrane", "bone"), ("membrane", "bone_lt"), ("membrane", "bone_dk"), ("membrane", "muscle"),
    ("membrane_lt", "bone"), ("membrane_lt", "bone_lt"), ("membrane_lt", "bone_dk"), ("membrane_lt", "muscle"),
    ("membrane", "fur_dk"), ("membrane_lt", "fur_dk"), ("membrane", "fur"), ("membrane_lt", "fur"),
    ("muscle", "bone"), ("muscle", "bone_lt"), ("muscle", "bone_dk"), ("muscle", "fur_dk"),
    ("hair", "rat"), ("hair", "rat_dk"), ("gum", "tooth"), ("gum", "fur_dk"), ("gum", "fur"),
    ("fur_lt", "fur_dk"), ("fur_lt", "fur"), ("fur", "fur_dk"), ("belly", "fur_dk"), ("belly", "fur"),
    ("ear_skin", "fur_dk"), ("ear_skin", "fur"), ("ear_vein", "ear_skin"), ("ear_vein", "fur_dk"),
    ("bone", "bone_lt"), ("bone", "bone_dk"), ("bone_lt", "bone_dk"), ("claw", "bone_dk"),
    ("eye", "fur_dk"), ("eye_red", "eye"), ("void", "fur_dk"), ("nose", "fur_dk"), ("nose", "void"),
)
# 血管格与膜同层，软覆盖关系同 membrane
_SOFT += tuple(("membrane_vein", other) for pair in _SOFT if "membrane" in pair for other in pair if other != "membrane")
SOFT_OVER = frozenset(frozenset(p) for p in _SOFT)

# ───────────────────────────────────────────────────────────── 骨骼树
# (name, parent, pivot[px, 世界坐标], 说明)。骨骼先父后子。+x = 右侧（*_r）。
BONES: list[tuple[str, str | None, tuple[float, float, float], str]] = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("body_hips", "root", (0.0, 15.8, 0.2), "臀 / 腰（尾巴和后腿的根）"),
    ("body_chest", "body_hips", (0.0, 20.5, 0.0), "胸（翼肩 / 颈的根）：毛层 + 浅色 V 纹 + 脊刺"),
    ("neck", "body_chest", (0.0, 24.0, -0.2), "颈（一圈毛领）"),
    ("head", "neck", (0.0, 25.2, -1.2), "头：脑颅 / 塌鼻 / 眼 / 上牙"),
    ("jaw", "head", (0.0, 24.6, -1.8), "下颌：张开的口，下牙 + 舌"),
    ("ear_l", "head", (-2.0, 29.2, 0.2), "左耳（根 + 中段，粉色耳肉）"),
    ("ear_r", "head", (2.0, 29.2, 0.2), "右耳"),
    ("ear_tip_l", "ear_l", (-3.3, 34.0, 0.2), "左耳尖（豁口）"),
    ("ear_tip_r", "ear_r", (3.3, 34.0, 0.2), "右耳尖"),
    ("wing_shoulder_l", "body_chest", (-3.6, 22.2, -0.6), "左翼肩（肩球 + 肌块）"),
    ("wing_shoulder_r", "body_chest", (3.6, 22.2, -0.6), "右翼肩"),
    ("wing_arm_l", "wing_shoulder_l", (-3.8, 22.4, -0.6), "左上臂骨（肱骨 + 肌腱）"),
    ("wing_arm_r", "wing_shoulder_r", (3.8, 22.4, -0.6), "右上臂骨"),
    ("wing_fore_l", "wing_arm_l", (-7.64, 24.80, -0.7), "左前臂骨（肘节 + 桡骨）"),
    ("wing_fore_r", "wing_arm_r", (7.64, 24.80, -0.7), "右前臂骨"),
    ("wing_hand_l", "wing_fore_l", (-11.72, 27.68, -0.8), "左腕（腕节 + 指根关节）"),
    ("wing_hand_r", "wing_fore_r", (11.72, 27.68, -0.8), "右腕"),
    ("thumb_l", "wing_hand_l", (-11.72, 28.48, -1.0), "左拇指爪（钩向外上）"),
    ("thumb_r", "wing_hand_r", (11.72, 28.48, -1.0), "右拇指爪"),
    ("finger_a_l", "wing_hand_l", (-11.72, 27.68, -0.8), "左第一指（前缘，最长）"),
    ("finger_a_r", "wing_hand_r", (11.72, 27.68, -0.8), "右第一指"),
    ("finger_b_l", "wing_hand_l", (-11.72, 27.68, -0.8), "左第二指"),
    ("finger_b_r", "wing_hand_r", (11.72, 27.68, -0.8), "右第二指"),
    ("finger_c_l", "wing_hand_l", (-11.72, 27.68, -0.8), "左第三指（后缘）"),
    ("finger_c_r", "wing_hand_r", (11.72, 27.68, -0.8), "右第三指"),
    ("membrane_in_l", "wing_arm_l", (-5.0, 20.0, 0.0), "左内侧翼膜（贴身一侧，破洞）"),
    ("membrane_in_r", "wing_arm_r", (5.0, 20.0, 0.0), "右内侧翼膜"),
    ("membrane_out_l", "wing_hand_l", (-14.00, 21.5, 0.0), "左外侧翼膜（指间，撕边）"),
    ("membrane_out_r", "wing_hand_r", (14.00, 21.5, 0.0), "右外侧翼膜"),
    ("leg_thigh_l", "body_hips", (-2.0, 14.6, 0.4), "左大腿"),
    ("leg_thigh_r", "body_hips", (2.0, 14.6, 0.4), "右大腿"),
    ("leg_shin_l", "leg_thigh_l", (-1.8, 11.6, 0.2), "左小腿"),
    ("leg_shin_r", "leg_thigh_r", (1.8, 11.6, 0.2), "右小腿"),
    ("foot_l", "leg_shin_l", (-1.8, 8.6, 0.2), "左脚（四趾 + 钩爪）"),
    ("foot_r", "leg_shin_r", (1.8, 8.6, 0.2), "右脚"),
    ("tail_1", "body_hips", (0.0, 14.0, 1.8), "尾 1：鼠尾根（与身体缝合）"),
    ("tail_2", "tail_1", (0.0, 11.3, 1.8), "尾 2：鼠尾（环纹 + 稀毛）"),
    ("tail_3", "tail_2", (0.0, 8.6, 1.8), "尾 3：蜥尾根（与鼠尾缝合）"),
    ("tail_4", "tail_3", (0.0, 5.9, 1.8), "尾 4：蜥尾（鳞片 + 背棘）"),
    ("tail_5", "tail_4", (0.0, 3.2, 1.8), "尾 5：蛇尾根（与蜥尾缝合）"),
    ("tail_6", "tail_5", (0.0, 0.5, 2.0), "尾 6：蛇尾（细鳞，尾尖卷起）"),
]


# ───────────────────────────────────────────────────────────── 建模辅助
class Puppet:
    """Rig 的薄封装：左右镜像成对建件，并记录各部件的立方体归属以便单件预览。"""

    def __init__(self) -> None:
        self.rig = Rig(MATS, tex=64, swatch=8)
        for name, parent, pivot, _ in BONES:
            self.rig.bone(name, pivot, parent)

    def box(self, bone, name, f, t, mat, rot=None, org=None):
        return self.rig.cube(bone, name, f, t, mat=mat, rot=rot, org=org)

    def pair(self, bone, name, f, t, mat, rot=None, org=None, sided=True):
        """f/t 按右侧（+x）坐标给；左侧自动镜像（x 取反，旋转的 y/z 分量取反）。

        sided=True 时 bone 是骨名词干，右件挂 bone_r、左件挂 bone_l；
        sided=False 时左右两件都挂在 bone（中线骨上的成对件，如髂骨、肋骨）。
        """
        bone_r = f"{bone}_r" if sided else bone
        bone_l = f"{bone}_l" if sided else bone
        self.rig.cube(bone_r, f"{name}_r", f, t, mat=mat, rot=rot, org=org)
        fl = (-t[0], f[1], f[2])
        tl = (-f[0], t[1], t[2])
        rl = None if rot is None else (rot[0], -rot[1], -rot[2])
        ol = None if org is None else (-org[0], org[1], org[2])
        self.rig.cube(bone_l, f"{name}_l", fl, tl, mat=mat, rot=rl, org=ol)

    def polar(self, bone, name, center, r, ang, tang, rad, z0, z1, mat):
        """在 xy 平面（正对 -z）上、极坐标 (r, ang) 处放一块「切向长 tang × 径向厚 rad × z0..z1」的盒子。

        ang 自 +y 起顺时针（从 -z 看）；盒子绕自身中心转到该处切线方向。
        命名按左右镜像规则：ang 与 360-ang 互为镜像对（_r 在 +x 侧，_l 在 -x 侧），正上 / 正下的件在中线。
        """
        a = ang % 360.0
        px = center[0] + r * math.sin(math.radians(a))
        py = center[1] + r * math.cos(math.radians(a))
        key = int(round(min(a, 360.0 - a) * 10))
        on_axis = abs(a) < 1e-6 or abs(a - 180.0) < 1e-6 or abs(a - 360.0) < 1e-6
        tag = f"{name}_{key}" if on_axis else f"{name}_{key}_{'r' if a < 180.0 else 'l'}"
        return self.rig.cube(bone, tag, (px - tang / 2, py - rad / 2, z0), (px + tang / 2, py + rad / 2, z1),
                             mat=mat, rot=(0.0, 0.0, -a), org=(px, py, (z0 + z1) / 2))

    def total(self):
        return len(self.rig.elements)


# ───────────────────────────────────────────────────────────── 分部件：头 / 耳 / 下颌
def part_head(p: Puppet) -> None:
    """蝠头：脑颅（毛层逐级收边）+ 颊毛 + 塌鼻（粉鼻垫 + 鼻孔）+ 凹眼（黑底红点）+ 眉棱 + 上牙（门齿 / 犬齿 / 侧牙）。"""
    b = "head"
    p.box(b, "cranium", (-2.6, 25.2, -3.6), (2.6, 29.4, 1.4), "fur_dk")
    p.box(b, "cranium_top", (-2.0, 29.4, -3.0), (2.0, 30.0, 0.8), "fur")
    p.box(b, "cranium_back", (-2.2, 25.8, 1.4), (2.2, 29.0, 1.9), "fur")
    p.pair(b, "cheek_fur", (2.6, 25.6, -2.8), (3.1, 27.6, 0.6), "fur_lt", sided=False)
    p.pair(b, "cheek_tuft", (2.7, 25.2, -1.6), (3.3, 25.9, -0.4), "fur", sided=False)
    p.pair(b, "crown_tuft_a", (0.0, 30.0, -1.8), (0.6, 30.8, -0.8), "fur_lt", sided=False)
    p.pair(b, "crown_tuft_b", (0.1, 30.0, -0.6), (0.5, 30.6, 0.4), "fur", sided=False)
    # 塌鼻：短而宽的吻 + 粉鼻垫，鼻孔朝前
    p.box(b, "snout", (-1.6, 25.4, -5.4), (1.6, 27.6, -3.6), "fur_dk")
    p.box(b, "snout_bridge", (-1.0, 27.6, -4.8), (1.0, 28.2, -3.2), "fur")
    p.box(b, "nose_pad", (-1.3, 26.0, -6.0), (1.3, 27.3, -5.4), "nose")
    p.pair(b, "nostril", (0.3, 26.5, -6.3), (0.9, 27.0, -6.0), "void", sided=False)
    p.pair(b, "whisker_pad", (1.0, 25.3, -5.0), (1.8, 26.4, -4.4), "fur_lt", sided=False)
    # 眼：凹进眉棱下的黑底，里面一点红
    p.pair(b, "brow", (0.4, 28.2, -4.0), (2.7, 28.9, -3.2), "fur", sided=False)
    p.pair(b, "eye", (1.7, 27.2, -4.0), (2.5, 28.2, -3.6), "eye", sided=False)
    p.pair(b, "eye_glint", (1.9, 27.5, -4.4), (2.3, 27.9, -4.0), "eye_red", sided=False)
    # 上牙：牙龈 + 前排六颗细牙 + 一对犬齿 + 每侧两颗侧牙
    p.box(b, "gum_up", (-1.5, 25.0, -5.4), (1.5, 25.4, -3.4), "gum")
    for i, x in enumerate((0.1, 0.65, 1.2)):
        p.pair(b, f"incisor_{i}", (x, 24.4, -5.4), (x + 0.4, 25.0, -5.1), "tooth", sided=False)
    p.pair(b, "canine_up", (1.4, 23.8, -5.2), (1.9, 25.1, -4.7), "tooth", sided=False)
    for i, z in enumerate((-4.3, -3.8)):
        p.pair(b, f"side_tooth_up_{i}", (1.3, 24.4, z), (1.7, 25.0, z + 0.4), "tooth", sided=False)


def part_ears(p: Puppet) -> None:
    """大耳：七层 1.2px 的横条逐层外倾、逐层收窄，拼成斜向外的阔叶；每层 = 背毛 + 毛框 + 内凹粉色耳肉 + 耳脉。
    下四层属耳根骨，上三层属耳尖骨；第 6 层外缘撕掉一块豁口。"""
    rows, h = 7, 1.2
    for k in range(rows):
        y0, y1 = 29.2 + k * h, 29.2 + (k + 1) * h
        c, w = 2.4 + 0.3 * k, 3.8 - 0.42 * k
        x0, x1 = c - w / 2, c + w / 2
        if k == 5:
            x1 -= 0.6                                          # 撕豁口
        bone = "ear" if k < 4 else "ear_tip"
        p.pair(bone, f"ear_{k}_back", (x0, y0, 0.3), (x1, y1, 0.7), "fur")
        p.pair(bone, f"ear_{k}_frame", (x0, y0, -0.3), (x1, y1, 0.3), "fur_dk")
        if w > 1.5:
            p.pair(bone, f"ear_{k}_skin", (x0 + 0.45, y0, -0.55), (x1 - 0.45, y1, -0.3), "ear_skin")
            xm = (x0 + x1) / 2
            p.pair(bone, f"ear_{k}_vein", (xm - 0.15, y0, -0.8), (xm + 0.15, y1, -0.55), "ear_vein")


def part_jaw(p: Puppet) -> None:
    """下颌：张开的口——颌底毛层 + 牙龈 + 舌 + 下牙（前排四颗 + 犬齿 + 侧牙）+ 口腔深处一块深影。"""
    b = "jaw"
    p.box(b, "jaw_base", (-1.5, 21.6, -4.8), (1.5, 22.4, -2.6), "fur_dk")
    p.box(b, "chin_tuft", (-0.6, 21.0, -4.6), (0.6, 21.6, -3.6), "fur")
    p.pair(b, "jaw_side", (1.5, 21.8, -4.4), (2.0, 22.8, -2.6), "fur", sided=False)
    p.box(b, "gum_low", (-1.4, 22.4, -4.8), (1.4, 22.8, -2.8), "gum")
    p.box(b, "tongue", (-0.7, 22.8, -4.4), (0.7, 23.1, -2.8), "gum")
    for i, x in enumerate((0.1, 0.65)):
        p.pair(b, f"incisor_low_{i}", (x, 22.8, -4.8), (x + 0.4, 23.5, -4.5), "tooth", sided=False)
    p.pair(b, "canine_low", (1.0, 22.7, -4.6), (1.5, 24.0, -4.1), "tooth", sided=False)
    for i, z in enumerate((-3.8, -3.3)):
        p.pair(b, f"side_tooth_low_{i}", (1.0, 22.8, z), (1.4, 23.4, z + 0.4), "tooth", sided=False)
    p.box(b, "throat", (-1.4, 22.8, -2.8), (1.4, 24.2, -2.4), "void")


# ───────────────────────────────────────────────────────────── 分部件：颈 / 胸 / 臀
def part_neck(p: Puppet) -> None:
    """颈：短粗的颈芯 + 一圈毛领（上下两层错位）。"""
    b = "neck"
    p.box(b, "neck_core", (-1.6, 23.4, -1.6), (1.6, 25.2, 1.2), "fur_dk")
    p.box(b, "ruff_front", (-1.8, 23.6, -2.2), (1.8, 24.6, -1.6), "fur_lt")
    p.pair(b, "ruff_side", (1.6, 23.6, -1.6), (2.2, 24.8, 0.8), "fur", sided=False)
    p.box(b, "ruff_back", (-1.5, 23.8, 1.2), (1.5, 25.0, 1.8), "fur")


def part_chest(p: Puppet) -> None:
    """胸：毛层逐级收边的桶身 + 肩带毛垫 + 胸前浅色 V 纹 + 背脊骨刺 + 腹侧乱毛簇。"""
    b = "body_chest"
    p.box(b, "chest_core", (-2.6, 17.6, -3.2), (2.6, 21.4, 3.2), "fur_dk")
    p.box(b, "chest_upper", (-2.6, 21.4, -2.7), (2.6, 23.4, 2.8), "fur")
    p.pair(b, "yoke", (2.6, 21.4, -2.4), (3.0, 23.4, 2.4), "fur_dk", sided=False)
    # 胸前 V 纹（浅色）：上宽下窄两层
    p.box(b, "vpatch_upper", (-1.6, 20.0, -3.7), (1.6, 22.3, -3.2), "belly")
    p.box(b, "vpatch_lower", (-1.0, 17.8, -3.7), (1.0, 20.0, -3.2), "belly")
    p.pair(b, "vpatch_edge", (1.6, 20.6, -3.5), (2.1, 22.0, -2.8), "fur_lt", sided=False)
    # 背面：毛板 + 脊椎骨刺
    p.box(b, "back_fur", (-2.2, 17.8, 3.2), (2.2, 23.0, 3.7), "fur")
    for i in range(4):
        y = 18.4 + i * 1.3
        p.box(b, f"spine_nub_{i}", (-0.4, y, 3.7), (0.4, y + 0.7, 4.2), "bone_dk")
    p.pair(b, "flank_tuft_a", (2.6, 18.2, -1.0), (3.2, 19.2, 0.4), "fur_lt", sided=False)
    p.pair(b, "flank_tuft_b", (2.6, 19.6, 0.4), (3.1, 20.4, 1.6), "fur", sided=False)


def part_hips(p: Puppet) -> None:
    """臀：毛桶 + 腰窝 + 尾根毛簇（尾巴是接上来的，所以根部有一圈乱毛盖住缝合线）。"""
    b = "body_hips"
    p.box(b, "hips_core", (-2.2, 14.0, -2.7), (2.2, 17.2, 3.0), "fur_dk")
    p.box(b, "hips_cap", (-1.8, 17.2, -2.4), (1.8, 17.6, 2.6), "fur")
    p.pair(b, "hip_fur", (2.2, 14.8, -2.0), (2.7, 16.8, 2.2), "fur", sided=False)
    p.box(b, "rump_fur", (-1.8, 14.4, 3.0), (1.8, 16.6, 3.5), "fur")
    p.pair(b, "tail_root_tuft_a", (0.4, 13.8, 1.0), (1.4, 14.1, 2.6), "fur_lt", sided=False)


def part_fur(p: Puppet) -> None:
    """乱毛簇：胸侧 / 背面 / 臀 / 颈领上散的小毛块，打破桶身的方正轮廓（逐个落在已有表面上）。"""
    p.pair("body_chest", "fur_side_a", (2.7, 19.0, 1.8), (3.3, 19.9, 2.6), "fur", sided=False)
    p.pair("body_chest", "fur_side_b", (2.7, 17.8, -0.4), (3.3, 18.5, 0.5), "fur_lt", sided=False)
    p.pair("body_chest", "fur_side_c", (2.6, 20.6, -2.6), (3.0, 21.2, -1.8), "fur_lt", sided=False)
    p.pair("body_chest", "fur_back_a", (0.8, 19.6, 3.7), (1.5, 20.4, 4.3), "fur_lt", sided=False)
    p.pair("body_chest", "fur_back_b", (1.4, 21.4, 3.7), (2.0, 22.2, 4.2), "fur", sided=False)
    p.pair("body_chest", "fur_back_c", (0.3, 22.2, 3.75), (0.9, 22.9, 4.1), "fur", sided=False)
    p.pair("body_hips", "fur_rump_a", (0.75, 15.0, 3.5), (1.3, 15.9, 4.0), "fur_lt", sided=False)
    p.pair("body_hips", "fur_rump_b", (1.6, 16.0, 3.5), (2.1, 16.7, 3.9), "fur", sided=False)
    p.pair("body_hips", "fur_hip_side", (2.7, 15.4, -0.4), (3.2, 16.2, 0.4), "fur_lt", sided=False)
    p.pair("neck", "fur_ruff_tip", (1.0, 23.4, 1.8), (1.6, 24.3, 2.3), "fur_lt", sided=False)
    p.pair("head", "fur_brow_tuft", (1.0, 28.9, -3.7), (1.6, 29.5, -2.8), "fur_lt", sided=False)


# ───────────────────────────────────────────────────────────── 分部件：后腿
def part_legs(p: Puppet) -> None:
    """后腿（蝠脚是勾挂用的）：毛大腿 + 细小腿 + 踝骨节 + 脚掌 + 四根趾 + 下钩的爪尖。
    腿整体靠身前（z 偏 -2.2），给身后垂下的尾巴和接口骨钉让出位置。"""
    dz = -2.2
    b = "leg_thigh"
    p.pair(b, "thigh", (1.0, 11.8, -0.6 + dz), (2.8, 15.0, 1.6 + dz), "fur_dk")
    p.pair(b, "thigh_fur", (2.8, 12.6, -0.2 + dz), (3.2, 14.4, 1.2 + dz), "fur")
    p.pair(b, "knee", (1.2, 11.4, -0.8 + dz), (2.6, 11.8, 1.4 + dz), "bone_dk")
    b = "leg_shin"
    p.pair(b, "shin", (1.3, 8.8, -0.2 + dz), (2.3, 11.4, 1.0 + dz), "fur")
    p.pair(b, "shin_bone", (1.4, 9.0, -0.5 + dz), (2.2, 11.0, -0.2 + dz), "bone")
    p.pair(b, "ankle", (1.1, 8.4, -0.4 + dz), (2.5, 8.8, 1.2 + dz), "bone_dk")
    b = "foot"
    p.pair(b, "sole", (1.0, 7.2, -2.2 + dz), (2.6, 8.4, 0.8 + dz), "fur_dk")
    for i, x in enumerate((1.0, 1.5, 2.0, 2.5)):
        # 四趾：趾节 + 钩向下的爪尖（爪尖逐个略长短不一）
        p.pair(b, f"toe_{i}", (x, 7.2, -3.4 + dz), (x + 0.4, 7.9, -2.2 + dz), "bone_dk")
        p.pair(b, f"toe_claw_{i}", (x, 6.4 - 0.1 * (i % 2), -3.8 + dz), (x + 0.4, 7.2, -3.4 + dz), "claw")
    p.pair(b, "heel_claw", (1.6, 6.6, 0.8 + dz), (2.1, 7.6, 1.3 + dz), "claw")


# ───────────────────────────────────────────────────────────── 分部件：翼骨 / 翼膜
WING_SCALE = 1.2                 # 翼相对肩的放大（设计坐标 → 实际坐标），翼展约 38px
WING_S = (3.8, 22.4, -0.6)       # 肩（右侧坐标）


def _w(x: float, y: float) -> tuple[float, float]:
    """翼部设计坐标（肩在 3.8, 22.4）按 WING_SCALE 绕肩放大后的实际 xy。"""
    return WING_S[0] + WING_SCALE * (x - WING_S[0]), WING_S[1] + WING_SCALE * (y - WING_S[1])


def _w3(x: float, y: float, z: float) -> tuple[float, float, float]:
    return (*_w(x, y), z)


WING_E = _w3(7.0, 24.4, -0.7)    # 肘
WING_W = _w3(10.4, 26.8, -0.8)   # 腕
FINGER_TIPS = {"a": _w3(16.8, 28.0, -0.8), "b": _w3(16.2, 19.0, -0.8), "c": _w3(12.6, 12.0, -0.8)}
FINGER_KNUCKLE = {"a": _w3(13.6, 27.5, -0.8), "b": _w3(13.2, 23.0, -0.8), "c": _w3(11.6, 19.4, -0.8)}
MEMBRANE_OUTLINE = tuple(_w(x, y) for x, y in (   # 右翼膜外轮廓（x, y），顺时针；指尖之间向内凹成扇贝边
    (2.6, 22.6), (7.0, 24.6), (10.4, 27.0), (16.8, 27.8), (14.6, 23.6), (16.2, 19.0), (13.4, 16.4),
    (12.6, 12.0), (8.2, 14.2), (2.8, 14.8),
))
MEMBRANE_SPLIT_X = 10.0          # 以此分内侧 / 外侧两片
MEMBRANE_TEARS = {               # 撕裂（圆心 x, y, 半长, 半宽, 长轴角度°）：细长斜口、贴边豁口，左右不同（破膜本来就不对称）
    "r": ((14.0, 19.0, 3.2, 0.55, 35.0), (9.5, 16.2, 2.2, 0.5, -60.0), (16.2, 22.4, 2.0, 0.45, 20.0),
          (11.0, 12.0, 2.0, 0.5, 80.0), (7.6, 18.2, 1.1, 0.8, 20.0)),
    "l": ((13.0, 17.5, 2.8, 0.5, -30.0), (8.5, 21.5, 1.5, 0.7, 70.0), (17.0, 18.5, 2.2, 0.45, -15.0),
          (10.5, 12.4, 1.8, 0.5, 100.0), (15.0, 24.4, 1.3, 0.5, 45.0)),
}
# 后缘（指尖之间的扇贝边）：只在这一段做噪声撕边；贴臂骨和贴身的边不撕，免得膜脱离骨架
MEMBRANE_TRAILING = MEMBRANE_OUTLINE[3:10]
# 血管纹：从指节 / 肘往后缘扇贝凹口和贴身一侧放射的几条线，格子落在线 0.42px 内就用血管材质
VEIN_SEGMENTS = (
    (FINGER_KNUCKLE["a"], MEMBRANE_OUTLINE[4]), (FINGER_KNUCKLE["b"], MEMBRANE_OUTLINE[6]),
    (FINGER_KNUCKLE["c"], MEMBRANE_OUTLINE[8]), (WING_E[:2], MEMBRANE_OUTLINE[9]),
    (WING_W[:2], MEMBRANE_OUTLINE[5]), (WING_E[:2], MEMBRANE_OUTLINE[8]),
    (_w(8.0, 21.0), _w(11.0, 17.0)), (_w(12.4, 22.5), _w(14.6, 20.2)),
)


def _inside(poly, x: float, y: float) -> bool:
    inside = False
    for (x0, y0), (x1, y1) in zip(poly, poly[1:] + poly[:1]):
        if (y0 > y) != (y1 > y) and x < (x1 - x0) * (y - y0) / (y1 - y0) + x0:
            inside = not inside
    return inside


def _edge_distance(poly, x: float, y: float) -> float:
    best = 9e9
    for (x0, y0), (x1, y1) in zip(poly, poly[1:] + poly[:1]):
        dx, dy = x1 - x0, y1 - y0
        t = max(0.0, min(1.0, ((x - x0) * dx + (y - y0) * dy) / (dx * dx + dy * dy)))
        best = min(best, math.hypot(x - (x0 + t * dx), y - (y0 + t * dy)))
    return best


def _in_tear(tears, x: float, y: float, ix: int, iy: int) -> bool:
    """点是否落在某道撕裂里：旋转椭圆，边缘按格号加一点抖动，让口子的边是锯齿而不是光滑椭圆。"""
    for cx, cy, half_len, half_wid, angle in tears:
        c, sn = math.cos(math.radians(angle)), math.sin(math.radians(angle))
        u, v = (x - cx) * c + (y - cy) * sn, -(x - cx) * sn + (y - cy) * c
        jag = 0.22 * math.sin(ix * 2.1 + iy * 1.3)
        if (u / half_len) ** 2 + (v / half_wid) ** 2 < 1.0 + jag:
            return True
    return False


def _trailing_distance(x: float, y: float) -> float:
    best = 9e9
    for (x0, y0), (x1, y1) in zip(MEMBRANE_TRAILING, MEMBRANE_TRAILING[1:]):
        dx, dy = x1 - x0, y1 - y0
        t = max(0.0, min(1.0, ((x - x0) * dx + (y - y0) * dy) / (dx * dx + dy * dy)))
        best = min(best, math.hypot(x - (x0 + t * dx), y - (y0 + t * dy)))
    return best


def _near_vein(x: float, y: float) -> bool:
    for (x0, y0), (x1, y1) in ((a[:2], b[:2]) for a, b in VEIN_SEGMENTS):
        dx, dy = x1 - x0, y1 - y0
        t = max(0.0, min(1.0, ((x - x0) * dx + (y - y0) * dy) / (dx * dx + dy * dy)))
        if math.hypot(x - (x0 + t * dx), y - (y0 + t * dy)) < 0.42:
            return True
    return False


def membrane_cells(side: str) -> list[tuple[int, int]]:
    """右翼 / 左翼的膜格（1px 格的左下角整数坐标，x 为右侧坐标系下的格号）：多边形内 − 撕裂 − 后缘噪声撕边，
    再剔除没有四邻接的孤格（它们在门禁里会被判悬空）。左翼用不同的撕裂和噪声相位，所以左右不镜像。"""
    tears = MEMBRANE_TEARS[side]
    phase = 1.7 if side == "l" else 0.0
    cells = set()
    for ix in range(0, 24):
        for iy in range(8, 34):
            cx, cy = ix + 0.5, iy + 0.5
            if not _inside(MEMBRANE_OUTLINE, cx, cy):
                continue
            if 2 <= ix <= 4 and 19 <= iy <= 24:          # 肩球 / 肌块占着这一块，膜从它外面开始
                continue
            if _in_tear(tears, cx, cy, ix, iy):
                continue
            wobble = math.sin(ix * 1.7 + iy * 0.9 + phase) + math.sin(ix * 0.6 - iy * 2.3 + 1.0 + phase)
            if _trailing_distance(cx, cy) < min(2.1, max(0.0, 1.0 + 0.55 * wobble)):
                continue                                  # 后缘撕出深浅不一的豁口
            cells.add((ix, iy))
    changed = True
    while changed:
        changed = False
        for c in list(cells):
            ix, iy = c
            if not any((ix + dx, iy + dy) in cells for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1))):
                cells.discard(c)
                changed = True
    return sorted(cells)


def part_wing_bones(p: Puppet) -> None:
    """翼骨：肩球 + 肌块 → 上臂（骨 + 肌腱袖）→ 肘节 → 前臂 → 腕节（指根三个小关节）→ 拇指爪（三节钩）→
    三根指骨（每根两段 + 指节球，近段骨色、远段亮骨色，尖端收成针）。肘以外的件按 WING_SCALE 绕肩放大。"""

    def wp(bone, name, f, t, mat):          # 翼上的成对盒：xy 按设计坐标给，自动放大
        (fx, fy), (tx, ty) = _w(f[0], f[1]), _w(t[0], t[1])
        p.pair(bone, name, (fx, fy, f[2]), (tx, ty, t[2]), mat)

    p.pair("wing_shoulder", "shoulder_ball", (3.0, 21.0, -1.6), (4.8, 23.4, 0.4), "bone_lt")
    p.pair("wing_shoulder", "shoulder_muscle", (3.0, 19.8, -1.4), (4.6, 21.0, 0.2), "muscle")
    p.pair("wing_shoulder", "shoulder_tuft", (3.0, 25.4, -1.2), (3.9, 26.0, 0.0), "fur_dk")
    p.rig.shaft("wing_arm_r", "humerus_r", WING_S, WING_E, 0.9, 0.9, mat="bone")
    p.rig.shaft("wing_arm_l", "humerus_l", _mx(WING_S), _mx(WING_E), 0.9, 0.9, mat="bone")
    sleeve_a, sleeve_b = (*_w(4.6, 21.9), -0.6), (*_w(6.4, 23.0), -0.65)
    p.rig.shaft("wing_arm_r", "arm_sleeve_r", sleeve_a, sleeve_b, 1.15, 1.15, mat="muscle")
    p.rig.shaft("wing_arm_l", "arm_sleeve_l", _mx(sleeve_a), _mx(sleeve_b), 1.15, 1.15, mat="muscle")
    wp("wing_fore", "elbow_knob", (6.2, 23.6, -1.5), (7.8, 25.2, 0.1), "bone_lt")
    wp("wing_fore", "elbow_pin", (6.7, 24.0, -1.9), (7.3, 24.6, -1.5), "flesh_ring")
    p.rig.shaft("wing_fore_r", "radius_r", WING_E, WING_W, 0.75, 0.75, mat="bone")
    p.rig.shaft("wing_fore_l", "radius_l", _mx(WING_E), _mx(WING_W), 0.75, 0.75, mat="bone")
    fa, fb = (*_w(8.0, 25.0), -0.75), (*_w(9.4, 25.9), -0.8)
    p.rig.shaft("wing_fore_r", "fore_sleeve_r", fa, fb, 1.0, 1.0, mat="muscle")
    p.rig.shaft("wing_fore_l", "fore_sleeve_l", _mx(fa), _mx(fb), 1.0, 1.0, mat="muscle")
    wp("wing_hand", "wrist_knob", (9.6, 25.9, -1.6), (11.2, 27.5, 0.0), "bone_dk")
    wp("wing_hand", "wrist_pin", (10.1, 26.3, -2.0), (10.7, 26.9, -1.6), "flesh_ring")
    for i, (kx, ky) in enumerate(((11.0, 27.5), (11.4, 26.4), (11.0, 25.4))):
        wp("wing_hand", f"finger_root_{i}", (kx - 0.1, ky - 0.5, -1.4), (kx + 0.7, ky + 0.5, -0.2), "bone_lt")
    # 拇指爪：三节钩，向外上弯
    wp("thumb", "thumb_base", (9.8, 27.5, -1.5), (11.0, 28.7, -0.5), "bone_dk")
    wp("thumb", "thumb_claw_a", (10.3, 28.7, -1.4), (11.3, 29.9, -0.6), "claw")
    wp("thumb", "thumb_claw_b", (10.9, 29.9, -1.3), (11.7, 30.9, -0.7), "claw")
    wp("thumb", "thumb_claw_c", (11.5, 30.9, -1.2), (12.2, 31.5, -0.8), "claw")
    # 三根指骨
    for k in ("a", "b", "c"):
        pts = [WING_W, FINGER_KNUCKLE[k], FINGER_TIPS[k]]
        for seg, (r, mat) in enumerate(((0.65, "bone"), (0.4, "bone_lt"))):
            a, bb = pts[seg], pts[seg + 1]
            p.rig.shaft(f"finger_{k}_r", f"finger_{k}_{seg}_r", a, bb, r, r, mat=mat)
            p.rig.shaft(f"finger_{k}_l", f"finger_{k}_{seg}_l", _mx(a), _mx(bb), r, r, mat=mat)
        kx, ky, kz = FINGER_KNUCKLE[k]
        p.pair(f"finger_{k}", f"knuckle_{k}", (kx - 0.8, ky - 0.8, kz - 0.9), (kx + 0.8, ky + 0.8, kz + 0.6), "bone_dk")


def _mx(pt):
    return (-pt[0], pt[1], pt[2])


def part_membranes(p: Puppet) -> None:
    """翼膜：1px 体素格逐格铺出（厚 0.4），撕裂 + 后缘撕边 + 血管纹；内外两片分骨，右翼与左翼破洞不同；
    格子的明暗按位置哈希在 membrane / membrane_lt 间错开，读作斑驳的旧膜。"""
    for side, sign in (("r", 1), ("l", -1)):
        for ix, iy in membrane_cells(side):
            x0, x1 = (ix, ix + 1) if sign > 0 else (-ix - 1, -ix)
            bone = f"membrane_{'in' if ix + 0.5 < MEMBRANE_SPLIT_X else 'out'}_{side}"
            if _near_vein(ix + 0.5, iy + 0.5):
                mat = "membrane_vein"
            else:
                mat = "membrane_lt" if (ix * 5 + iy * 3) % 7 == 0 else "membrane"
            p.box(bone, f"mem_{ix:02d}_{iy:02d}_{side}x", (x0, iy, -0.2), (x1, iy + 1, 0.2), mat)


# ───────────────────────────────────────────────────────────── 分部件：杂接的尾（鼠 → 蜥 → 蛇）
TAIL_CZ = 1.8                       # 尾轴 z（身后，垂直下垂）


def _suture(p: Puppet, bone: str, y: float, hx: float, cz: float) -> None:
    """接口（方截面，半宽 hx）：红肉圈四条边 + 每面黑线针脚 + 前后各一对骨钉（竖钉带上下钉帽）+ 左右各一根骨钉。
    y 为接缝中心；整套件挂在接缝下方那一节的骨上。"""
    t = 0.25
    top, bot = cz - hx, cz + hx
    p.box(bone, f"{bone}_seam_front", (-hx - t, y - 0.35, top - t), (hx + t, y + 0.35, top), "flesh_ring")
    p.box(bone, f"{bone}_seam_back", (-hx - t, y - 0.35, bot), (hx + t, y + 0.35, bot + t), "flesh_ring")
    p.pair(bone, f"{bone}_seam_side", (hx, y - 0.35, top), (hx + t, y + 0.35, bot), "flesh_ring", sided=False)
    p.box(bone, f"{bone}_st_f0", (-0.15, y - 0.8, top - t - 0.3), (0.15, y + 0.8, top - t), "thread")
    p.box(bone, f"{bone}_st_b0", (-0.15, y - 0.8, bot + t), (0.15, y + 0.8, bot + t + 0.3), "thread")
    p.pair(bone, f"{bone}_st_f1", (0.55, y - 0.8, top - t - 0.3), (0.85, y + 0.8, top - t), "thread", sided=False)
    p.pair(bone, f"{bone}_st_b1", (0.55, y - 0.8, bot + t), (0.85, y + 0.8, bot + t + 0.3), "thread", sided=False)
    for i, dz in enumerate((-0.4, 0.4)):                       # 侧面针脚
        p.pair(bone, f"{bone}_st_s{i}", (hx + t, y - 0.8, cz + dz - 0.15), (hx + t + 0.3, y + 0.8, cz + dz + 0.15),
               "thread", sided=False)
    # 骨钉：前、后各一对（在针脚两侧），侧面一根；每根竖插、上下各一个钉帽
    for tag, z0, z1, zc0, zc1 in (("f", top - t - 0.7, top - t - 0.2, top - t - 0.9, top - t),
                                  ("b", bot + t + 0.2, bot + t + 0.7, bot + t, bot + t + 0.9)):
        p.pair(bone, f"{bone}_pin_{tag}", (0.9, y - 0.9, z0), (1.4, y + 0.9, z1), "pin", sided=False)
        p.pair(bone, f"{bone}_pin_{tag}_cap_a", (0.7, y + 0.9, zc0), (1.6, y + 1.3, zc1), "pin", sided=False)
        p.pair(bone, f"{bone}_pin_{tag}_cap_b", (0.7, y - 1.3, zc0), (1.6, y - 0.9, zc1), "pin", sided=False)
    p.pair(bone, f"{bone}_pin_s", (hx + t + 0.3, y - 1.0, cz - 0.25), (hx + t + 0.8, y + 1.0, cz + 0.25), "pin", sided=False)
    p.pair(bone, f"{bone}_pin_s_cap_a", (hx + t, y + 1.0, cz - 0.45), (hx + t + 0.9, y + 1.4, cz + 0.45), "pin", sided=False)
    p.pair(bone, f"{bone}_pin_s_cap_b", (hx + t, y - 1.4, cz - 0.45), (hx + t + 0.9, y - 1.0, cz + 0.45), "pin", sided=False)


def part_tail_rat(p: Puppet) -> None:
    """鼠尾（尾 1 / 2）：裸皮粗尾，一圈圈环纹（深浅交替的环）+ 稀疏黑毛；与身体的接口缝合在尾 1 上。"""
    cz = TAIL_CZ
    for bone, y0, y1, hx in (("tail_1", 11.3, 14.0, 1.7), ("tail_2", 8.6, 11.3, 1.55)):
        n = 3
        for i in range(n):
            ya, yb = y0 + (y1 - y0) * i / n, y0 + (y1 - y0) * (i + 1) / n
            p.box(bone, f"{bone}_seg{i}", (-hx, ya, cz - hx), (hx, yb - 0.3, cz + hx), "rat" if i % 2 == 0 else "rat_dk")
            p.box(bone, f"{bone}_ring{i}", (-hx - 0.15, yb - 0.3, cz - hx - 0.15), (hx + 0.15, yb, cz + hx + 0.15), "rat_dk")
        hair_dy = 0.2 if bone == "tail_1" else 1.7
        for i, dz in enumerate((-0.4, 0.5)):                       # 稀毛：左右各一簇，背面中线一簇
            dy = hair_dy
            p.pair(bone, f"{bone}_hair{i}", (hx, y0 + dy, cz + dz - 0.15), (hx + 0.5, y0 + dy + 0.8, cz + dz + 0.15),
                   "hair", sided=False)
        p.box(bone, f"{bone}_hair_back", (-0.15, y0 + 1.0, cz + hx + 0.15), (0.15, y0 + 1.9, cz + hx + 0.65), "hair")
    _suture(p, "tail_1", 14.0, 1.7, cz)


def part_tail_lizard(p: Puppet) -> None:
    """蜥尾（尾 3 / 4）：深绿核 + 每节两行错位鳞片（四面都贴，前后面分中片 / 边片，侧面分前后半）+ 背面一排小棘；
    与鼠尾的接口缝合在尾 3 上。"""
    cz = TAIL_CZ
    for bone, y0, y1, hx in (("tail_3", 5.9, 8.6, 1.9), ("tail_4", 3.2, 5.9, 1.7)):
        p.box(bone, f"{bone}_core", (-hx, y0, cz - hx), (hx, y1, cz + hx), "lizard_dk")
        rows = 2
        top_y = y1 - (0.45 if bone == "tail_3" else 0.0)       # 尾 3 上端是接缝，鳞片到缝下留一道边
        for r in range(rows):
            ya, yb = y0 + (top_y - y0) * r / rows, y0 + (top_y - y0) * (r + 1) / rows
            even = (r % 2 == 0)
            mat_a, mat_b = ("lizard", "lizard_lt") if even else ("lizard_lt", "lizard")
            for face, zlo, zhi in (("f", cz - hx - 0.35, cz - hx), ("b", cz + hx, cz + hx + 0.35)):
                if even:
                    p.pair(bone, f"{bone}_sc{r}{face}", (0.0, ya, zlo), (hx, yb, zhi), mat_a, sided=False)
                else:
                    p.box(bone, f"{bone}_sc{r}{face}_mid", (-hx / 2, ya, zlo), (hx / 2, yb, zhi), mat_a)
                    p.pair(bone, f"{bone}_sc{r}{face}", (hx / 2, ya, zlo), (hx, yb, zhi), mat_b, sided=False)
            p.pair(bone, f"{bone}_scs{r}a", (hx, ya, cz - hx), (hx + 0.35, yb, cz), mat_a, sided=False)
            p.pair(bone, f"{bone}_scs{r}b", (hx, ya, cz), (hx + 0.35, yb, cz + hx), mat_b, sided=False)
            p.box(bone, f"{bone}_spike{r}", (-0.25, ya + 0.2, cz + hx + 0.35), (0.25, yb - 0.1, cz + hx + 0.95), "lizard_lt")
    _suture(p, "tail_3", 8.6, 1.9, cz)


def part_tail_snake(p: Puppet) -> None:
    """蛇尾（尾 5 / 6）：深色细鳞，前面一条浅色腹纹，鳞片逐行错位；尾 6 逐段收细并向 +z 卷起成钩；
    与蜥尾的接口缝合在尾 5 上。"""
    cz = TAIL_CZ
    hx = 1.3
    p.box("tail_5", "tail_5_core", (-hx, 0.5, cz - hx), (hx, 3.2, cz + hx), "snake_dk")
    for r in range(3):
        ya, yb = 0.5 + 2.25 * r / 3, 0.5 + 2.25 * (r + 1) / 3
        even = (r % 2 == 0)
        p.box("tail_5", f"tail_5_belly{r}", (-0.45, ya, cz - hx - 0.3), (0.45, yb, cz - hx), "snake_lt")
        p.pair("tail_5", f"tail_5_scf{r}", (0.45, ya, cz - hx - 0.3), (hx, yb, cz - hx), "snake" if even else "snake_dk", sided=False)
        p.pair("tail_5", f"tail_5_scb{r}", (0.0, ya, cz + hx), (hx, yb, cz + hx + 0.3), "snake" if even else "snake_dk", sided=False)
        p.pair("tail_5", f"tail_5_scs{r}", (hx, ya, cz - hx), (hx + 0.3, yb, cz + hx), "snake" if not even else "snake_dk",
               sided=False)
    _suture(p, "tail_5", 3.2, 1.3, cz)
    # 尾 6：三段逐段变细并后移，最后一截水平卷出的尖
    steps = ((0.5, -0.6, 1.0, 0.0), (-0.6, -1.5, 0.8, 0.7), (-1.5, -2.2, 0.55, 1.6))
    for i, (ya, yb, h, dz) in enumerate(steps):
        p.box("tail_6", f"tail_6_seg{i}", (-h, yb, cz + dz - h), (h, ya, cz + dz + h), "snake_dk" if i % 2 == 0 else "snake")
        p.box("tail_6", f"tail_6_belly{i}", (-h * 0.55, yb, cz + dz - h - 0.25), (h * 0.55, ya, cz + dz - h), "snake_lt")
    p.box("tail_6", "tail_6_hook", (-0.35, -2.5, cz + 1.6 + 0.55), (0.35, -1.7, cz + 4.2), "snake_dk")
    p.box("tail_6", "tail_6_point", (-0.25, -2.3, cz + 4.2), (0.25, -1.5, cz + 5.0), "snake")


def build(parts: list) -> Puppet:
    p = Puppet()
    for fn in parts:
        fn(p)
    return p


PART_FUNCS = {
    "head": part_head,
    "ears": part_ears,
    "jaw": part_jaw,
    "neck": part_neck,
    "chest": part_chest,
    "hips": part_hips,
    "fur": part_fur,
    "legs": part_legs,
    "wing_bones": part_wing_bones,
    "membranes": part_membranes,
    "tail_rat": part_tail_rat,
    "tail_lizard": part_tail_lizard,
    "tail_snake": part_tail_snake,
}


# ───────────────────────────────────────────────────────────── 门禁
MIN_BONES = 24                      # 任务卡「骨骼数下限」：飞行类（蝠 / 蛾）≥ 24
# 刻意不对称的骨：翼膜（破洞和撕边左右不同）
ASYM_BONES = frozenset({"membrane_in_l", "membrane_in_r", "membrane_out_l", "membrane_out_r"})


class EntityGates(gatekit.AssetGates):
    """实体模型的门禁：去掉「越出 0..16 方块空间」一道 —— 生物不是方块，GeckoLib 实体不受它限制。"""

    def specs(self):
        return tuple(spec for spec in super().specs() if spec[0] != "overflow")


GATES = EntityGates("杂尾蝠 / za_wei_fu", MATS, asym=ASYM_BONES, soft_over=SOFT_OVER)


def gate_rig(p: Puppet) -> Rig:
    """门禁用副本：rigkit 把 color 写成「材质序号 % 8」，材质多于 8 种时会撞号，
    门禁按材质名判同材质 / 软覆盖，所以副本里改回完整序号。"""
    rig = copy.deepcopy(p.rig)
    names = list(MATS)
    for el in rig.elements:
        el["color"] = el["color"]  # 先保留，下面按名字重写
    # elements 与材质的对应关系：重新按贴图色块 uv 反推材质序号
    per_row = rig.tex // rig.swatch
    for el in rig.elements:
        uv = el["faces"]["north"]["uv"]
        idx = int(uv[1] // rig.swatch) * per_row + int(uv[0] // rig.swatch)
        el["color"] = idx
    GATES.colors = {i: n for i, n in enumerate(names)}
    return rig


def coplanar_violations(p: Puppet) -> list[str]:
    """共面（Z-fighting）门：只查没有旋转的件（旋转件的 from/to 不是真实包围盒）。"""
    flat = [e for e in p.rig.elements if not any(e["rotation"])]
    return coplanar.check_coplanar_faces(flat)


def moving_bone_count(p: Puppet) -> int:
    """真正带几何、能独立动的骨（不含只做分组的 root）。"""
    return sum(1 for name, b in p.rig.bones.items() if b["children"])


def run_gates(p: Puppet) -> int:
    rig = gate_rig(p)
    total = GATES.report(rig, px=16.0, note="（实体不受 16px 方块空间限制，已去掉越界一道）")
    cop = coplanar_violations(p)
    print(f"  {'✓' if not cop else '✗'} 共面 (Z-fighting): {len(cop)}")
    for line in cop[:8]:
        print(f"      - {line}")
    bones, moving = len(p.rig.bones), moving_bone_count(p)
    ok = moving >= MIN_BONES
    print(f"  {'✓' if ok else '✗'} 骨骼数: 共 {bones}（含 root），带几何的可动骨 {moving}（下限 {MIN_BONES}）")
    return total + len(cop) + (0 if ok else 1)


def self_test(p: Puppet) -> None:
    """差分自证：先在干净模型上跑一遍（须全绿），再给每道门注入缺陷，须被抓到。"""
    rig = gate_rig(p)
    broken = GATES.self_test(rig)
    # 共面门的差分：复制一个已有的无旋转件，原位放一份，必须报共面
    clean = coplanar_violations(p)
    victim = next(e for e in p.rig.elements if not any(e["rotation"]))
    twin = copy.deepcopy(victim)
    twin["name"] = victim["name"] + "__twin"
    hits = coplanar.check_coplanar_faces([e for e in p.rig.elements if not any(e["rotation"])] + [twin])
    if clean or not hits:
        broken += 1
        print(f"  ✗ 共面: 干净 {len(clean)} / 注入后 {len(hits)}（须 0 / >0）")
    else:
        print(f"  ✓ 共面: 干净 0 → 注入同位复制件 {victim['name']} 后 {len(hits)}")
    # 骨骼数门的差分：只含后腿的预览模型骨数须掉到下限以下
    small = build([part_legs])
    if moving_bone_count(small) >= MIN_BONES:
        broken += 1
        print("  ✗ 骨骼数门没有鉴别力")
    else:
        print(f"  ✓ 骨骼数: 完整 {moving_bone_count(p)} ≥ {MIN_BONES}；只含后腿的件 {moving_bone_count(small)} 被拦下")
    if broken:
        raise SystemExit(f"自证未通过：{broken} 道门失效")
    print("✓ 差分自证全绿")


# ───────────────────────────────────────────────────────────── 输出：bbmodel / 分件预览 / 接触表 / check.png
BG = (119, 119, 119)
# 六个诚实命名的视角：名字写的是相机所在位置（傀儡正面朝 -z，右侧 = +x）。
# render() 的 yaw=180 时相机在 -z 看正面，yaw=0 在 +z 看背面，yaw=90 在 -x 看傀儡左侧。
SIX_VIEWS = (
    ("FRONT 正视（相机在 -z，看脸 / 胸前 V 纹 / 翼膜正面）", 180.0, 0.0),
    ("3/4 前右斜视（相机在 -z / +x 上方）", 150.0, 18.0),
    ("LEFT 左侧视（相机在 -x，吻朝画面左，尾 / 缝合线）", 90.0, 0.0),
    ("RIGHT 右侧视（相机在 +x，吻朝画面右）", -90.0, 0.0),
    ("TOP 俯视（相机在 +y，吻朝画面下方）", 180.0, 89.9),
    ("BACK 背视（相机在 +z，看背毛 / 尾巴）", 0.0, 0.0),
)
REF_PANELS = ((0, 615), (615, 925), (925, 1536))     # 参考图上排三张：正 / 侧 / 背
REF_TOP_FRAC = 0.52                                 # 上排占整图高度的比例
SAME_FRAMING_VIEWS = (("FRONT 正", 180.0, 0.0), ("LEFT 侧（吻朝左）", 90.0, 0.0), ("BACK 背", 0.0, 0.0))


def write_model(p: Puppet, path: Path, name: str) -> None:
    path.write_text(json.dumps(p.rig.bbmodel(name), ensure_ascii=False, indent=1), encoding="utf-8")


def render_model(path: Path, yaw: float, pitch: float, size: int) -> Image.Image:
    return render(path, yaw=yaw, pitch=pitch, size=size, bg=BG)[0]


def render_part_previews(tmp: Path) -> None:
    """逐件单独预览：每个 part_*() 只放自己的立方体，出正 / 3/4 / 侧三视角。"""
    PARTS_DIR.mkdir(parents=True, exist_ok=True)
    font_t, font_s = load_font(15), load_font(12)
    for idx, (key, fn) in enumerate(PART_FUNCS.items(), start=1):
        part = build([fn])
        write_model(part, tmp, f"ZaWeiFu_{key}")
        w = 300
        sheet = Image.new("RGB", (w * 3, w + 44), (32, 33, 36))
        d = ImageDraw.Draw(sheet)
        d.text((8, 5), f"part_{key}: {fn.__doc__.splitlines()[0]}  ({part.total()} cubes)", fill=(235, 235, 235), font=font_t)
        for k, (label, yaw, pitch) in enumerate((("FRONT -z", 180.0, 0.0), ("3/4 前右", 150.0, 18.0), ("LEFT -x", 90.0, 0.0))):
            sheet.paste(render_model(tmp, yaw, pitch, w), (k * w, 24))
            d.text((k * w + 6, w + 26), label, fill=(200, 200, 200), font=font_s)
        sheet.save(PARTS_DIR / f"part_{idx:02d}_{key}.png")
    print(f"✓ 分件预览 {len(PART_FUNCS)} 张 → {PARTS_DIR}")


def render_six_views(model_path: Path, title: str, cell: int = 380) -> Image.Image:
    font_h, font_l = load_font(17), load_font(12)
    sheet = Image.new("RGB", (cell * 3 + 20, cell * 2 + 70), (32, 33, 36))
    d = ImageDraw.Draw(sheet)
    d.text((10, 6), title, fill=(235, 235, 235), font=font_h)
    for i, (label, yaw, pitch) in enumerate(SIX_VIEWS):
        x, y = 5 + (i % 3) * (cell + 5), 32 + (i // 3) * (cell + 28)
        sheet.paste(render_model(model_path, yaw, pitch, cell), (x, y + 22))
        d.rectangle([x, y, x + cell - 1, y + 21], fill=(22, 23, 26))
        d.text((x + 6, y + 3), label, fill=(235, 235, 235), font=font_l)
    return sheet


def figure_box(im: Image.Image, bg_tol: float = 38.0) -> tuple[int, int, int, int]:
    """图里「不是背景」的包围盒：背景取四条边缘像素的中位色。"""
    import numpy as np
    a = np.asarray(im.convert("RGB"), dtype=float)
    edge = np.concatenate([a[0], a[-1], a[:, 0], a[:, -1]])
    bg = np.median(edge, axis=0)
    mask = np.abs(a - bg).sum(axis=2) > bg_tol
    ys, xs = np.where(mask)
    return int(xs.min()), int(ys.min()), int(xs.max()) + 1, int(ys.max()) + 1


def fit_height(im: Image.Image, h: int) -> Image.Image:
    box = figure_box(im)
    pad = int(0.03 * (box[3] - box[1]))
    crop = im.crop((max(0, box[0] - pad), max(0, box[1] - pad), min(im.width, box[2] + pad), min(im.height, box[3] + pad)))
    return crop.resize((max(1, round(crop.width * h / crop.height)), h), Image.Resampling.LANCZOS)


def same_framing_row(model_path: Path, ref_path: Path, h: int = 340) -> Image.Image:
    """参考图上排三张（正 / 侧 / 背）与本模型同视角，各自裁到人形包围盒后缩到同一高度并排：上行参考，下行模型。"""
    font_h, font_l = load_font(17), load_font(12)
    ref = Image.open(ref_path).convert("RGB")
    ref_top = ref.crop((0, 0, ref.width, int(ref.height * REF_TOP_FRAC)))
    ref_tiles, mdl_tiles = [], []
    for (x0, x1), (_, yaw, pitch) in zip(REF_PANELS, SAME_FRAMING_VIEWS):
        ref_tiles.append(fit_height(ref_top.crop((x0, 0, x1, ref_top.height)), h))
        mdl_tiles.append(fit_height(render_model(model_path, yaw, pitch, 700), h))
    col_w = [max(a.width, b.width) + 12 for a, b in zip(ref_tiles, mdl_tiles)]
    canvas = Image.new("RGB", (sum(col_w) + 10, 2 * h + 96), (32, 33, 36))
    d = ImageDraw.Draw(canvas)
    d.text((10, 6), "同取景对照：上 参考，下 模型（裁到包围盒、同高）", fill=(235, 235, 235), font=font_h)
    x = 5
    for (label, _, _), rt, mt, cw in zip(SAME_FRAMING_VIEWS, ref_tiles, mdl_tiles, col_w):
        d.text((x + 4, 30), f"参考 · {label}", fill=(215, 205, 180), font=font_l)
        canvas.paste(rt, (x + (cw - rt.width) // 2, 48))
        d.text((x + 4, 54 + h), f"模型 · {label}", fill=(150, 230, 180), font=font_l)
        canvas.paste(mt, (x + (cw - mt.width) // 2, 72 + h))
        x += cw
    return canvas


def bone_tree_lines(p: Puppet) -> list[str]:
    children: dict[str | None, list[str]] = {}
    for name, parent, _, _ in BONES:
        children.setdefault(parent, []).append(name)
    desc = {name: d for name, _, _, d in BONES}
    lines: list[str] = []

    def walk(name: str, prefix: str, last: bool, root: bool) -> None:
        n = len(p.rig.bones[name]["children"]) if name in p.rig.bones else 0
        # children 里既有 element uuid 也不含子骨（子骨在 outliner 里挂），所以这里的数就是该骨自己的立方体数
        branch = "" if root else ("└─ " if last else "├─ ")
        lines.append(f"{prefix}{branch}{name}  [{n}]  {desc[name]}")
        kids = children.get(name, [])
        for i, k in enumerate(kids):
            walk(k, prefix + ("" if root else ("   " if last else "│  ")), i == len(kids) - 1, False)

    walk("root", "", True, True)
    return lines


def bone_tree_panel(p: Puppet, height: int) -> Image.Image:
    lines = bone_tree_lines(p)
    font_h, font_s, font_t = load_font(17), load_font(13), load_font(12)
    step = 17
    w = 760
    h = max(height, 110 + step * len(lines) + 20)
    panel = Image.new("RGB", (w, h), (32, 33, 37))
    d = ImageDraw.Draw(panel)
    d.rectangle([0, 0, w - 1, h - 1], outline=(70, 75, 85), width=2)
    bones, moving = len(p.rig.bones), moving_bone_count(p)
    d.text((14, 8), "骨骼层级树 BONE HIERARCHY（[n] = 该骨自带的立方体数）", fill=(180, 235, 200), font=font_h)
    d.text((14, 34), f"总骨骼 {bones}（含 root）｜带几何的可动骨 {moving}｜任务卡飞行类下限 ≥ {MIN_BONES} ✓",
           fill=(240, 210, 150), font=font_s)
    d.text((14, 54), f"立方体 {p.total()}｜材质 {len(MATS)} 种｜0.5px 精度（部件坐标 0.05px 的整数倍）", fill=(190, 210, 235), font=font_s)
    d.line([(12, 78), (w - 12, 78)], fill=(65, 70, 80), width=1)
    y = 86
    for ln in lines:
        d.text((14, y), ln, fill=(228, 228, 234), font=font_t)
        y += step
    return panel


def make_check(p: Puppet, model_path: Path, gate_total: int) -> None:
    ref_p = REVIEW_DIR / f"{MODEL_ID}_ref.png"
    exp_p = REVIEW_DIR / f"{MODEL_ID}_exploded.png"
    font_h = load_font(18)
    six = render_six_views(model_path, f"za_wei_fu（杂尾蝠）round 3/3 · {p.total()} cubes · "
                                       f"{len(p.rig.bones)} 骨（可动 {moving_bone_count(p)}）· 门禁违例 {gate_total}")
    row = same_framing_row(model_path, ref_p)
    tree = bone_tree_panel(p, six.height + row.height + 10)
    six.save(REVIEW_DIR / "contact_sheet.png")
    tree.save(REVIEW_DIR / "bones_tree.png")
    ref, exp = Image.open(ref_p).convert("RGB"), Image.open(exp_p).convert("RGB")
    lw = 640
    ref_s = ref.resize((lw, round(ref.height * lw / ref.width)), Image.Resampling.LANCZOS)
    exp_s = exp.resize((lw, round(exp.height * lw / exp.width)), Image.Resampling.LANCZOS)
    mid_w = max(six.width, row.width)
    total_h = max(ref_s.height + exp_s.height + 120, six.height + row.height + 70, tree.height + 44)
    canvas = Image.new("RGB", (lw + mid_w + tree.width + 50, total_h), (25, 26, 29))
    d = ImageDraw.Draw(canvas)
    d.text((10, 8), "参考图（生图，gen_exploded_ref.py）", fill=(215, 205, 180), font=font_h)
    canvas.paste(ref_s, (10, 38))
    d.text((10, 48 + ref_s.height), "爆炸分解图", fill=(215, 205, 180), font=font_h)
    canvas.paste(exp_s, (10, 78 + ref_s.height))
    x = lw + 25
    d.text((x, 8), "本模型（Round 3 几何：无贴图 / 动画 / 接线 / manifest）", fill=(140, 230, 180), font=font_h)
    canvas.paste(six, (x, 38))
    canvas.paste(row, (x, 38 + six.height + 10))
    x += mid_w + 15
    d.text((x, 8), f"骨骼树（{len(p.rig.bones)} 骨）", fill=(180, 220, 240), font=font_h)
    canvas.paste(tree, (x, 38))
    out = REVIEW_DIR / "check.png"
    canvas.save(out)
    print(f"✓ check.png → {out} ({canvas.width}×{canvas.height})")


# ───────────────────────────────────────────────────────────── 像素贴图（复用 generators/handheld_texture.py 画笔库）
sys.path.insert(0, str(REPO / "modelScript" / "generators"))
import handheld_texture  # noqa: E402

# 本文件的色块材质名 → 画笔库材质名（同名的不列）
TEX_MATERIAL = {
    "fur_dk": "fur_dark", "fur_lt": "fur_lit", "belly": "belly_fur", "membrane_lt": "membrane_lit",
    "muscle": "muscle_flesh", "nose": "nose_pink", "eye": "void", "eye_red": "ink_cinnabar", "tooth": "bone_lit",
    "bone": "bone_main", "bone_lt": "bone_lit", "bone_dk": "bone_dark", "claw": "claw_dark",
    "rat": "rat_skin", "rat_dk": "rat_skin_dark", "hair": "hair_dark", "flesh_ring": "flesh_seam",
    "thread": "silk", "pin": "bone_lit", "lizard": "lizard_scale", "lizard_dk": "lizard_scale_dark",
    "lizard_lt": "lizard_scale_lit", "snake": "snake_scale", "snake_dk": "snake_scale_dark", "snake_lt": "snake_belly",
}


def tex_run() -> None:
    """贴图版 ZaWeiFu_tex.bbmodel + 平色对照 + check_tex.png（参考图 | 平色 3/4 | 贴图 3/4 / 正 / 侧 / 背）。"""
    p = build(list(PART_FUNCS.values()))
    out = CREATURE_DIR / "ZaWeiFu_tex.bbmodel"
    out.write_text(json.dumps(handheld_texture.textured_rig_doc(p.rig, "ZaWeiFu_tex", TEX_MATERIAL), ensure_ascii=False, indent=1),
                   encoding="utf-8")
    flat = CREATURE_DIR / "_tmp_flat.bbmodel"
    flat.write_text(json.dumps(handheld_texture.textured_rig_doc(p.rig, "ZaWeiFu_flat", TEX_MATERIAL, flat=True)), encoding="utf-8")
    try:
        h = 760
        font = load_font(16)
        ref = Image.open(REVIEW_DIR / f"{MODEL_ID}_ref.png").convert("RGB")
        ref = ref.resize((round(ref.width * h / ref.height), h), Image.Resampling.LANCZOS)
        tiles = [("参考图", ref), ("平色 3/4", fit_height(render_model(flat, 150.0, 18.0, 900), h))]
        for label, yaw, pitch in (("贴图 3/4", 150.0, 18.0), ("贴图 正", 180.0, 0.0), ("贴图 左侧", 90.0, 0.0), ("贴图 背", 0.0, 0.0)):
            tiles.append((label, fit_height(render_model(out, yaw, pitch, 900), h)))
        sheet = Image.new("RGB", (sum(t.width + 12 for _, t in tiles) + 12, h + 44), (28, 29, 33))
        d = ImageDraw.Draw(sheet)
        x = 12
        for label, tile in tiles:
            d.text((x, 10), label, fill=(235, 235, 235), font=font)
            sheet.paste(tile, (x, 36))
            x += tile.width + 12
        REVIEW_DIR.mkdir(parents=True, exist_ok=True)
        sheet.save(REVIEW_DIR / "check_tex.png")
    finally:
        flat.unlink(missing_ok=True)
    print(f"✓ {out}\n✓ check_tex.png → {REVIEW_DIR / 'check_tex.png'}")


def full_run() -> None:
    """全量：自证 → 全套门禁 → 写 bbmodel → 分件预览 → 接触表 / 骨骼树 / check.png。"""
    p = build(list(PART_FUNCS.values()))
    self_test(p)
    total = run_gates(p)
    if total:
        raise SystemExit(f"门禁仍有 {total} 处违例，不出图")
    model = CREATURE_DIR / "ZaWeiFu.bbmodel"
    write_model(p, model, "ZaWeiFu")
    print(f"✓ {model}: {p.total()} cubes / {len(p.rig.bones)} bones")
    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    tmp = CREATURE_DIR / "_tmp_part.bbmodel"
    try:
        render_part_previews(tmp)
    finally:
        tmp.unlink(missing_ok=True)
    make_check(p, model, total)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--part", choices=sorted(PART_FUNCS))
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--gates", action="store_true")
    ap.add_argument("--tex", action="store_true", help="只出贴图版 bbmodel 与 check_tex.png")
    args = ap.parse_args()
    if args.tex:
        tex_run()
        return
    if not (args.part or args.self_test or args.gates):
        full_run()
        return
    p = build([PART_FUNCS[args.part]] if args.part else list(PART_FUNCS.values()))
    if args.self_test:
        self_test(p)
        return
    if args.gates:
        run_gates(p)
        return
    out = CREATURE_DIR / f"_tmp_{args.part or 'all'}.bbmodel"
    out.write_text(json.dumps(p.rig.bbmodel("ZaWeiFu")), encoding="utf-8")
    print(f"{p.total()} cubes → {out}")


if __name__ == "__main__":
    main()
