#!/usr/bin/env python3
"""游商傀儡（trade_puppet）Blockbench .bbmodel 程序化生成器 —— Round 1/3，只做几何。

正典依据：docs/worldview.md §九 L860「游商傀儡：真正有身价的大能从不亲自出面。用异变兽骨骼和阵法
炼制交易傀儡在野外游荡。抢劫傀儡 → 傀儡自锁 + 远在万里之外的主人立刻收到你的坐标」。
任务卡：末世生物批次 #2（.task-wasteland-creatures.md）。

外形：异变兽骨拼成的瘦高骨架（比玩家高一头）。关节处两截骨头错位搭接，用绳和铜箍扎住；背上驮一个带
三层抽屉和挂钩的木货箱，挂小布包与骨币串；胸腔里嵌一块圆形阵盘（自锁时亮——本轮只留「亮纹 vs 暗底」
的对比度，不做发光特效）；头是长吻兽颅骨，眼窝里一点余烬光，下颌独立可张合。

坐标：1 block = 16px，原点在两脚中间的地面，+y 向上，正面朝 -z（GeckoLib 约定），+x 为傀儡的右侧
（骨名 *_r）、-x 为左侧（*_l）。总高约 42px（玩家 28.8px），约合 2.6 格。
分部件：part_*() 逐件写，--part 单件预览；骨架在 BONES 里一处定义。

用法：
    python3 modelScript/creatures/trade_puppet/gen_trade_puppet.py --self-test   # 门禁差分自证
    python3 modelScript/creatures/trade_puppet/gen_trade_puppet.py --part head    # 单件预览
    python3 modelScript/creatures/trade_puppet/gen_trade_puppet.py                # 全量：模型 + 接触表
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
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/creatures/trade_puppet")
PARTS_DIR = REVIEW_DIR / "parts"

sys.path.insert(0, str(REPO / "modelScript" / "tools"))
from cjk_font import load_font  # noqa: E402

# 仓库内的共面 (Z-fighting) 门禁：modelScript/bbmodel_maker 与已安装的同名库同名，按路径单独加载
_spec = importlib.util.spec_from_file_location(
    "repo_coplanar", REPO / "modelScript" / "bbmodel_maker" / "gates" / "coplanar.py")
coplanar = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(coplanar)

MODEL_ID = "trade_puppet"

# ───────────────────────────────────────────────────────────── 材质（RGB，每种占贴图一个 8×8 色块）
MATS = {
    "bone":       (206, 194, 164),   # 异变兽骨主色
    "bone_lt":    (224, 214, 186),   # 骨关节头 / 牙 / 高光骨
    "bone_dk":    (168, 154, 124),   # 骨内侧阴面 / 夹板骨
    "bone_stain": (126, 106, 78),    # 骨缝血渍 / 裂纹 / 老化斑
    "wood":       (118, 84, 52),
    "wood_dk":    (80, 57, 37),
    "wood_lt":    (152, 114, 74),
    "rope":       (156, 130, 84),
    "rope_dk":    (110, 88, 55),
    "copper":     (178, 110, 62),
    "copper_dk":  (112, 70, 42),
    "verdigris":  (92, 138, 116),
    "iron":       (72, 68, 66),
    "cloth_red":  (140, 62, 52),
    "cloth_grey": (106, 108, 106),
    "cloth_sage": (96, 110, 84),
    "cloth_sand": (162, 142, 106),
    "coin":       (216, 204, 170),
    "stone_dk":   (60, 60, 66),      # 阵盘暗底
    "rune":       (156, 228, 204),   # 阵盘亮纹（自锁态靠它和暗底的反差）
    "void":       (22, 20, 20),      # 眼窝 / 鼻孔 / 缝隙深影
    "ember":      (236, 150, 66),    # 眼窝余烬
}
# 材质对（有序无关）：软件盖硬件 / 绑缚压在骨上，允许有薄重叠，不算穿模
SOFT_OVER = frozenset(frozenset(p) for p in (
    ("rope", "bone"), ("rope", "bone_lt"), ("rope", "bone_dk"), ("rope", "bone_stain"),
    ("rope_dk", "bone"), ("rope_dk", "bone_lt"), ("rope_dk", "bone_dk"), ("rope", "copper"),
    ("rope", "wood"), ("rope", "wood_dk"), ("rope_dk", "wood"), ("rope", "iron"),
    ("copper", "bone"), ("copper", "bone_lt"), ("copper", "bone_dk"), ("copper_dk", "bone"),
    ("cloth_red", "bone"), ("cloth_red", "rope"), ("cloth_grey", "rope"), ("cloth_sage", "rope"),
    ("cloth_sand", "rope"), ("cloth_red", "wood"), ("cloth_grey", "wood"), ("cloth_sage", "wood"),
    ("cloth_sand", "wood"), ("cloth_red", "copper"), ("cloth_grey", "copper"),
    ("coin", "rope"), ("coin", "copper"), ("wood", "wood_dk"), ("wood_lt", "wood"),
    ("wood_lt", "wood_dk"), ("copper", "wood"), ("copper", "wood_dk"), ("copper", "wood_lt"),
    ("iron", "wood"), ("iron", "wood_dk"), ("iron", "wood_lt"), ("copper_dk", "wood"),
    ("copper_dk", "wood_dk"), ("copper_dk", "wood_lt"), ("rope", "wood_lt"), ("rope_dk", "wood_lt"),
    ("copper_dk", "rope"), ("copper_dk", "bone_lt"), ("copper_dk", "bone_dk"), ("verdigris", "copper"),
    ("copper", "stone_dk"), ("rope", "stone_dk"), ("copper_dk", "stone_dk"),
))

# ───────────────────────────────────────────────────────────── 骨骼树
# (name, parent, pivot[px, 世界坐标], 说明)。骨骼先父后子。+x = 右侧（*_r）。
BONES: list[tuple[str, str | None, tuple[float, float, float], str]] = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("hips", "root", (0.0, 19.0, 1.4), "骨盆（髂骨 / 骶骨 / 耻骨）+ 绳腰带 + 铜扣"),
    ("spine_lower", "hips", (0.0, 21.0, 1.4), "腰椎 3 节，椎间用绳扎"),
    ("spine_upper", "spine_lower", (0.0, 24.5, 1.4), "胸椎 7 节 + 6 对肋 + 胸骨"),
    ("neck", "spine_upper", (0.0, 32.6, 1.2), "颈椎 2 节 + 铜项圈"),
    ("head", "neck", (0.0, 34.5, 1.0), "兽颅骨：脑颅 / 长吻 / 眼窝 / 上颌齿 / 颧弓"),
    ("jaw", "head", (0.0, 35.2, 0.4), "下颌：双支 + 下牙 + 舌"),
    ("horn_l", "head", (-2.4, 40.0, 1.0), "左角（3 段 + 铜箍）"),
    ("horn_r", "head", (2.4, 40.0, 1.0), "右角"),
    ("charm_l", "head", (-3.6, 40.0, 1.0), "左挂件：铜环 + 绳 + 红布穗 + 铜牌"),
    ("charm_r", "head", (3.6, 40.0, 1.0), "右挂件"),
    ("array_disc", "spine_upper", (0.0, 28.4, -3.0), "胸口阵盘（铜缘 / 暗底 / 亮纹，自锁时亮）"),
    ("shoulder_l", "spine_upper", (-1.2, 32.0, -0.6), "左锁骨 + 肩峰 + 肩胛"),
    ("shoulder_r", "spine_upper", (1.2, 32.0, -0.6), "右肩"),
    ("upper_arm_l", "shoulder_l", (-6.8, 30.0, 0.3), "左上臂（肱骨 + 夹板 + 铜箍）"),
    ("upper_arm_r", "shoulder_r", (6.8, 30.0, 0.3), "右上臂"),
    ("forearm_l", "upper_arm_l", (-6.8, 22.8, 0.3), "左前臂（尺桡骨 + 绳扎 + 铜箍）"),
    ("forearm_r", "upper_arm_r", (6.8, 22.8, 0.3), "右前臂"),
    ("hand_l", "forearm_l", (-6.8, 14.4, 0.3), "左掌（腕骨 / 掌骨）"),
    ("hand_r", "forearm_r", (6.8, 14.4, 0.3), "右掌"),
    ("claws_l", "hand_l", (-6.9, 10.6, 0.3), "左三指 + 爪尖"),
    ("claws_r", "hand_r", (6.9, 10.6, 0.3), "右三指 + 爪尖"),
    ("thumb_l", "hand_l", (-5.7, 12.0, 0.2), "左拇指 + 爪"),
    ("thumb_r", "hand_r", (5.7, 12.0, 0.2), "右拇指 + 爪"),
    ("leg_thigh_l", "hips", (-3.3, 17.5, 0.3), "左股骨（夹板 + 绳 + 铜箍）"),
    ("leg_thigh_r", "hips", (3.3, 17.5, 0.3), "右股骨"),
    ("leg_shin_l", "leg_thigh_l", (-3.3, 10.5, 0.3), "左胫腓骨（髌骨 / 铜箍 / 踝）"),
    ("leg_shin_r", "leg_thigh_r", (3.3, 10.5, 0.3), "右胫腓骨"),
    ("foot_l", "leg_shin_l", (-3.3, 3.2, 0.3), "左跗骨 / 跟骨 / 跖骨"),
    ("foot_r", "leg_shin_r", (3.3, 3.2, 0.3), "右脚"),
    ("toes_l", "foot_l", (-3.3, 0.9, -3.4), "左三趾 + 爪"),
    ("toes_r", "foot_r", (3.3, 0.9, -3.4), "右三趾"),
    ("cloth_front", "hips", (0.0, 19.8, -1.8), "前裆布（红 / 灰 / 青三幅）"),
    ("cloth_back", "hips", (0.0, 19.8, 3.4), "后裆布"),
    ("crate", "spine_upper", (0.0, 27.5, 4.0), "背负木货箱：框 / 板 / 角铜 / 提手 / 挂钩 / 肩架"),
    ("drawer_top", "crate", (0.0, 30.0, 11.0), "上层抽屉"),
    ("drawer_mid", "crate", (0.0, 26.3, 11.0), "中层抽屉"),
    ("drawer_bot", "crate", (0.0, 22.6, 11.0), "下层抽屉"),
    ("bundle_a", "crate", (-6.4, 29.4, 8.0), "左上红布包"),
    ("bundle_b", "crate", (-6.4, 24.6, 6.4), "左下青布包"),
    ("bundle_c", "crate", (6.4, 28.4, 7.4), "右布包"),
    ("rope_coil", "crate", (7.0, 25.8, 5.6), "右下挂钩上的麻绳圈"),
    ("coin_string", "crate", (5.0, 33.4, 12.1), "骨币串（六枚，绳穿）"),
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


def band(p: Puppet, bone: str, name: str, mat: str, x: tuple, y: tuple, z: tuple, side: str | None = None):
    """绑在骨上的一圈带（绳 / 铜箍）：比被绑的骨略大的整块，视觉上读作一圈。"""
    if side is None:
        p.box(bone, name, (x[0], y[0], z[0]), (x[1], y[1], z[1]), mat)
    else:
        p.pair(bone, name, (x[0], y[0], z[0]), (x[1], y[1], z[1]), mat)


# ───────────────────────────────────────────────────────────── 分部件：腿 / 脚
def part_legs(p: Puppet) -> None:
    """两条长腿：股骨（夹板 + 三道绳 + 铜箍）→ 胫腓骨（髌骨 / 绳 / 铜箍 / 踝）→ 脚（跟骨 + 跖骨 + 绳十字）→ 三趾。"""
    # —— 股骨（bone leg_thigh，右侧坐标，x 中心 3.3）——
    b = "leg_thigh"
    p.pair(b, "femur_shaft", (2.2, 11.6, -0.9), (4.4, 16.6, 1.5), "bone")
    p.pair(b, "femur_ridge", (2.9, 12.0, -1.3), (3.7, 16.0, -0.9), "bone_dk")        # 前缘粗线
    p.pair(b, "femur_head", (2.5, 16.6, -1.1), (4.7, 17.9, 1.7), "bone_lt")          # 股骨头球
    p.pair(b, "femur_trochanter", (4.4, 15.2, -0.6), (5.1, 16.8, 1.2), "bone_lt")    # 大转子
    p.pair(b, "femur_condyle", (2.1, 10.5, -1.4), (4.5, 11.6, 1.8), "bone_lt")       # 髁
    p.pair(b, "femur_condyle_front", (2.6, 10.5, -1.8), (4.0, 11.2, -1.4), "bone_dk")
    p.pair(b, "femur_splint", (4.4, 12.4, -0.4), (4.9, 15.0, 1.0), "bone_stain")     # 外侧夹板骨
    # 夹板用三道绳扎在股骨上（绳比骨宽，读作一圈）
    for i, (y0, y1) in enumerate(((12.6, 13.1), (13.7, 14.2), (14.3, 14.8))):
        mat = "rope" if i % 2 == 0 else "rope_dk"
        p.pair(b, f"femur_lash_{i}", (2.0, y0, -1.2), (5.1, y1, 1.8), mat)
    p.pair(b, "femur_lash_knot", (3.0, 12.7, -1.6), (3.6, 13.0, -1.2), "rope_dk")
    p.pair(b, "femur_lash_tail", (3.2, 11.9, -1.6), (3.6, 12.6, -1.3), "rope_dk")
    # 铜箍（下段，靠近膝）+ 两颗铆钉
    p.pair(b, "femur_hoop", (1.9, 11.7, -1.2), (4.7, 12.4, 1.8), "copper")
    p.pair(b, "femur_hoop_rivet_a", (2.3, 11.8, -1.5), (2.8, 12.2, -1.2), "copper_dk")
    p.pair(b, "femur_hoop_rivet_b", (3.8, 11.8, -1.5), (4.3, 12.2, -1.2), "copper_dk")
    # —— 胫腓骨（bone leg_shin）——
    b = "leg_shin"
    p.pair(b, "tibia_plateau", (2.2, 9.5, -1.2), (4.4, 10.5, 1.6), "bone_lt")
    p.pair(b, "patella", (2.5, 9.9, -2.1), (4.1, 11.5, -1.2), "bone_lt")             # 髌骨凸出在膝前
    p.pair(b, "tibia_shaft", (2.5, 5.2, -0.8), (4.1, 9.5, 1.2), "bone")
    p.pair(b, "tibia_crest", (3.0, 5.4, -1.2), (3.6, 9.3, -0.8), "bone_dk")          # 胫骨前嵴
    p.pair(b, "fibula", (4.1, 4.8, -0.4), (4.8, 9.2, 0.8), "bone_dk")                # 腓骨贴外侧
    p.pair(b, "shin_lash_a", (2.3, 8.2, -1.0), (5.0, 8.8, 1.4), "rope")
    p.pair(b, "shin_lash_b", (2.3, 8.9, -1.0), (5.0, 9.4, 1.4), "rope_dk")
    p.pair(b, "shin_hoop", (2.2, 6.4, -1.1), (5.0, 7.6, 1.5), "copper")              # 小腿中部宽铜箍
    p.pair(b, "shin_hoop_rim", (2.0, 6.7, -1.3), (5.2, 7.3, 1.7), "copper_dk")
    p.pair(b, "shin_hoop_stud_a", (2.6, 6.8, -1.6), (3.2, 7.2, -1.3), "bone_lt")
    p.pair(b, "shin_hoop_stud_b", (3.9, 6.8, -1.6), (4.5, 7.2, -1.3), "bone_lt")
    p.pair(b, "ankle_block", (2.1, 3.2, -0.9), (4.7, 4.4, 1.5), "bone_lt")            # 踝
    p.pair(b, "ankle_hoop", (2.0, 4.4, -1.1), (4.6, 5.2, 1.7), "copper")
    p.pair(b, "ankle_hoop_rivet", (3.0, 4.6, -1.4), (3.6, 5.0, -1.1), "copper_dk")
    p.pair(b, "ankle_lash", (2.1, 5.3, -1.0), (4.7, 5.8, 1.6), "rope")
    # —— 脚（bone foot）——
    b = "foot"
    p.pair(b, "talus", (2.4, 2.4, -0.6), (4.2, 3.2, 1.4), "bone_lt")
    p.pair(b, "heel", (2.4, 0.0, 1.0), (4.2, 2.4, 3.0), "bone")
    p.pair(b, "heel_pad", (2.6, 0.0, 3.0), (4.0, 1.2, 3.4), "bone_dk")
    p.pair(b, "midfoot", (2.3, 0.0, -1.6), (4.3, 2.4, 1.0), "bone")
    for i, (x0, x1) in enumerate(((2.3, 3.0), (3.1, 3.8), (3.9, 4.6))):                 # 三根跖骨
        p.pair(b, f"metatarsal_{i}", (x0, 0.0, -3.4), (x1, 1.6, -1.6), "bone_lt" if i == 1 else "bone")
    p.pair(b, "foot_lash", (2.1, 1.4, -1.9), (4.5, 1.9, 1.2), "rope")                 # 脚背绳圈
    p.pair(b, "foot_cross_a", (3.0, 2.4, -1.4), (3.5, 2.8, 1.0), "rope_dk", rot=(0.0, 35.0, 0.0))
    p.pair(b, "foot_cross_b", (3.0, 2.4, -1.4), (3.5, 2.8, 1.0), "rope", rot=(0.0, -35.0, 0.0))
    # —— 三趾（bone toes）：近节 / 中节 / 爪尖，爪尖逐段略向下 ——
    b = "toes"
    for i, (x0, x1) in enumerate(((2.3, 3.0), (3.1, 3.8), (3.9, 4.6))):
        p.pair(b, f"toe_prox_{i}", (x0, 0.0, -4.6), (x1, 1.2, -3.4), "bone")
        p.pair(b, f"toe_mid_{i}", (x0 + 0.05, 0.0, -5.5), (x1 - 0.05, 1.0, -4.6), "bone_lt")
        p.pair(b, f"toe_claw_{i}", (x0 + 0.1, 0.0, -6.2), (x1 - 0.1, 0.7, -5.5), "bone_stain")
    p.pair(b, "toe_lash", (2.2, 0.2, -4.9), (4.7, 1.3, -4.5), "rope")


# ───────────────────────────────────────────────────────────── 分部件：骨盆 / 腰带 / 裆布
def part_hips(p: Puppet) -> None:
    """骨盆：髂骨翼 + 髂嵴 + 髋臼 + 耻骨 + 骶骨尾椎；一圈绳腰带，前面铜扣；前后两幅裆布。"""
    b = "hips"
    p.pair(b, "ilium", (1.6, 18.6, -0.8), (4.6, 21.0, 2.6), "bone", sided=False)
    p.pair(b, "ilium_crest", (2.4, 21.0, -0.4), (5.0, 21.8, 2.4), "bone_lt", sided=False)
    p.pair(b, "acetabulum", (2.4, 18.0, -0.9), (4.6, 18.6, 1.7), "bone_dk", sided=False)
    p.pair(b, "ischium", (1.4, 17.0, 0.2), (2.4, 18.6, 2.4), "bone_dk", sided=False)
    p.pair(b, "pubis", (0.1, 17.4, -1.2), (2.0, 18.6, 0.0), "bone_lt", sided=False)
    p.box(b, "sacrum", (-1.4, 17.8, 1.0), (1.4, 21.0, 2.8), "bone_lt")
    p.box(b, "coccyx", (-0.5, 16.6, 1.6), (0.5, 17.8, 2.6), "bone_dk")
    for i in range(3):                                          # 骶孔：背面三个暗点
        p.box(b, f"sacral_hole_{i}", (-0.4, 18.4 + i * 0.9, 2.8), (0.4, 18.9 + i * 0.9, 3.0), "bone_stain")
    # 腰带：前带 / 两侧带 / 后带围成一圈，贴在髂骨外面
    p.box(b, "belt_front", (-5.2, 19.6, -1.6), (5.2, 20.6, -1.0), "rope")
    p.pair(b, "belt_side", (4.6, 19.6, -1.0), (5.2, 20.6, 2.6), "rope_dk", sided=False)
    p.box(b, "belt_back", (-5.2, 19.6, 2.6), (5.2, 20.6, 3.2), "rope")
    p.box(b, "buckle", (-1.2, 19.4, -1.9), (1.2, 20.8, -1.6), "copper")
    p.box(b, "buckle_stud", (-0.4, 19.8, -2.2), (0.4, 20.4, -1.9), "copper_dk")
    p.pair(b, "buckle_rivet", (0.7, 19.6, -2.2), (1.0, 19.9, -1.9), "copper_dk", sided=False)
    p.pair(b, "belt_tail", (2.2, 18.2, -1.5), (2.7, 19.6, -1.1), "rope_dk", sided=False)      # 垂下的绳头


def part_cloth(p: Puppet) -> None:
    """前后裆布：三幅（红 / 灰 / 青）参差下摆，缀铜珠；后幅更窄更短（被货箱压着）。"""
    b = "cloth_front"
    p.box(b, "cf_red", (-1.3, 14.6, -2.6), (1.3, 19.6, -2.2), "cloth_red")
    p.box(b, "cf_red_hem", (-1.0, 13.4, -2.6), (0.4, 14.6, -2.2), "cloth_red")
    p.box(b, "cf_red_hem2", (0.4, 13.8, -2.6), (1.1, 14.6, -2.2), "cloth_red")
    p.pair(b, "cf_grey", (1.3, 15.6, -2.4), (3.3, 19.6, -2.0), "cloth_grey", sided=False)
    p.pair(b, "cf_grey_hem", (1.6, 14.8, -2.4), (2.4, 15.6, -2.0), "cloth_grey", sided=False)
    p.pair(b, "cf_bead", (1.3, 14.2, -2.9), (1.8, 14.8, -2.4), "copper", sided=False)
    p.box(b, "cf_bead_c", (-0.3, 12.8, -2.9), (0.3, 13.4, -2.4), "copper")
    b = "cloth_back"
    p.box(b, "cb_sage", (-2.0, 15.8, 3.2), (2.0, 19.6, 3.6), "cloth_sage")
    p.pair(b, "cb_sage_hem", (0.2, 14.8, 3.2), (1.6, 15.8, 3.6), "cloth_sage", sided=False)
    p.pair(b, "cb_red_edge", (2.0, 16.8, 3.2), (3.2, 19.6, 3.6), "cloth_red", sided=False)


# ───────────────────────────────────────────────────────────── 分部件：脊柱 / 肋骨 / 胸骨
def _vertebra(p: Puppet, bone: str, tag: str, y: float, h: float, spur: float) -> None:
    """一节椎骨：椎体 + 左右横突 + 向后（+z）的棘突；spur 是棘突高度。"""
    p.box(bone, f"vert_{tag}", (-1.2, y, 0.4), (1.2, y + h, 2.4), "bone")
    p.pair(bone, f"vert_{tag}_tp", (1.2, y + 0.2, 1.0), (2.4, y + h - 0.1, 2.0), "bone_dk", sided=False)
    p.box(bone, f"vert_{tag}_spur", (-0.4, y + h - spur, 2.4), (0.4, y + h, 3.1), "bone_lt")


def part_spine(p: Puppet) -> None:
    """腰椎 3 节（椎间绳扎）+ 胸椎 7 节（椎间暗色软骨）。"""
    for i in range(3):
        y = 21.0 + i * 1.2
        _vertebra(p, "spine_lower", f"l{i}", y, 0.9 if i < 2 else 0.9, 0.8)
        if i < 2:
            band(p, "spine_lower", f"lash_l{i}", "rope" if i == 0 else "rope_dk",
                 (-1.5, 1.5), (y + 0.9, y + 1.2), (0.2, 2.6))
    for i in range(7):
        y = 24.5 + i * 1.2
        _vertebra(p, "spine_upper", f"t{i}", y, 0.9, 1.0 if i % 2 == 0 else 0.7)
        if i < 6:
            p.box("spine_upper", f"disc_t{i}", (-1.0, y + 0.9, 0.6), (1.0, y + 1.2, 2.2), "bone_stain")


RIBS = [  # (y, 外缘 x, 是否有前臂) 自下而上：两根浮肋、四根真肋；肋骨自上而下略收
    (24.9, 4.2, False), (26.1, 4.8, False), (27.3, 5.0, True),
    (28.5, 5.0, True), (29.7, 4.6, True), (30.9, 3.9, True),
]


def part_ribs(p: Puppet) -> None:
    """肋骨 6 对：背臂（椎→外）+ 侧臂（沿 z）+ 前臂（向胸骨收），拼成桶形；胸骨 + 剑突 + 胸骨柄。"""
    b = "spine_upper"
    for i, (y, R, full) in enumerate(RIBS):
        t = 0.8
        p.pair(b, f"rib_back_{i}", (1.4, y, 1.1), (R - 0.8, y + t, 1.9), "bone", sided=False)
        z0 = -1.8 if full else -0.8
        p.pair(b, f"rib_side_{i}", (R - 0.8, y, z0), (R, y + t, 1.9), "bone_lt" if i % 2 else "bone", sided=False)
        if full:
            p.pair(b, f"rib_front_{i}", (0.7, y, -2.6), (R - 0.8, y + t, -1.8), "bone", sided=False)
        else:
            p.pair(b, f"rib_tip_{i}", (R - 0.8, y, z0 - 0.5), (R - 0.3, y + t, z0), "bone_dk", sided=False)
    p.box(b, "sternum", (-0.7, 25.7, -2.6), (0.7, 31.7, -1.8), "bone_lt")
    p.box(b, "xiphoid", (-0.4, 24.9, -2.4), (0.4, 25.7, -1.8), "bone_dk")
    p.box(b, "manubrium", (-1.6, 31.7, -2.6), (1.6, 32.5, -1.7), "bone_lt")
    # 胸椎背面与肋骨交接处的绳扎（把两节肋骨组扎成一束）
    p.box(b, "rib_lash_low", (-4.6, 26.05, 1.9), (4.6, 26.55, 2.2), "rope_dk")
    p.box(b, "rib_lash_high", (-4.2, 29.65, 1.9), (4.2, 30.15, 2.2), "rope")


def part_disc(p: Puppet) -> None:
    """胸口阵盘：铜缘 16 段 → 暗底外圈（8 个亮符点）→ 亮纹圈 12 段 → 暗底内圈 → 8 道亮辐条 → 铜心；
    四根拴绳把盘缘拴在肋骨上。盘厚 1.3，背面落在肋骨前缘 z=-2.6。

    发光只靠提高对比度：亮纹用 rune（淡青白），底用 stone_dk（近黑），盘缘 / 铜心是铜色不发光。
    """
    b = "array_disc"
    c = (0.0, 28.4)
    zb = -2.6
    # 盘底：四根错 45° 的长条拼成八角圆底，逐根错开 0.1 厚，避免同面重叠
    for i in range(4):
        p.box(b, f"disc_base_{i * 45}", (-2.7, c[1] - 1.15, zb - 0.5 - 0.1 * i), (2.7, c[1] + 1.15, zb + 0.1),
              "stone_dk", rot=(0.0, 0.0, 45.0 * i) if i else None, org=(0.0, c[1], zb - 0.2))
    for i in range(16):                                                  # 铜缘
        p.polar(b, "disc_rim", c, 3.45, 22.5 * i, 2.0 * math.pi * 3.45 / 16 * 1.06, 0.7, zb - 1.3, zb, "copper")
    for i in range(16):                                                  # 外圈暗底
        p.polar(b, "disc_outer", c, 2.75, 22.5 * i, 2.0 * math.pi * 2.75 / 16 * 1.06, 0.7, zb - 0.7, zb, "stone_dk")
    for i in range(8):                                                   # 外圈 8 个亮符点（落在暗段缝上）
        p.polar(b, "disc_glyph", c, 2.75, 22.5 + 45.0 * i, 0.6, 0.6, zb - 0.95, zb - 0.7, "rune")
    for i in range(12):                                                  # 亮纹圈
        p.polar(b, "disc_ring", c, 2.05, 30.0 * i, 2.0 * math.pi * 2.05 / 12 * 1.04, 0.5, zb - 0.9, zb, "rune")
    for i in range(8):                                                   # 内圈暗底
        p.polar(b, "disc_inner", c, 1.4, 45.0 * i, 2.0 * math.pi * 1.4 / 8 * 1.06, 0.7, zb - 0.7, zb, "stone_dk")
    for i in range(8):                                                   # 辐条
        p.polar(b, "disc_spoke", c, 1.75, 45.0 * i, 0.3, 0.7, zb - 1.2, zb - 0.9, "rune")
    p.box(b, "disc_core_a", (-0.6, c[1] - 0.6, zb - 1.2), (0.6, c[1] + 0.6, zb - 0.7), "copper")
    p.box(b, "disc_core_b", (-0.6, c[1] - 0.6, zb - 1.5), (0.6, c[1] + 0.6, zb - 1.2), "copper_dk",
          rot=(0.0, 0.0, 45.0), org=(0.0, c[1], zb - 1.35))
    for ang in (45.0, 135.0, 225.0, 315.0):                              # 拴绳：盘缘 → 肋骨
        p.polar(b, "disc_lash", c, 3.9, ang, 1.2, 0.5, zb - 0.8, zb - 0.3, "rope")


# ───────────────────────────────────────────────────────────── 分部件：颈 / 头 / 下颌 / 角 / 挂件
def part_neck(p: Puppet) -> None:
    """颈椎 2 节，椎间绳扎，根部一圈铜项圈（前面两颗铆钉）。"""
    b = "neck"
    p.box(b, "cerv_0", (-1.1, 32.6, 0.4), (1.1, 33.4, 2.0), "bone")
    p.box(b, "cerv_0_spur", (-0.4, 32.8, 2.0), (0.4, 33.4, 2.7), "bone_lt")
    p.pair(b, "cerv_0_tp", (1.1, 32.8, 0.8), (2.0, 33.2, 1.8), "bone_dk", sided=False)
    p.box(b, "cerv_lash", (-1.4, 33.4, 0.2), (1.4, 33.7, 2.2), "rope")
    p.box(b, "cerv_1", (-1.1, 33.7, 0.4), (1.1, 34.5, 2.0), "bone_lt")
    p.box(b, "cerv_1_spur", (-0.4, 33.9, 2.0), (0.4, 34.5, 2.6), "bone")
    p.box(b, "collar", (-1.8, 32.7, -0.3), (1.8, 33.3, 2.5), "copper")
    p.pair(b, "collar_rivet", (0.9, 32.8, -0.6), (1.3, 33.1, -0.3), "copper_dk", sided=False)


def part_head(p: Puppet) -> None:
    """长吻兽颅骨：脑颅两层 + 圆顶 + 矢状嵴；眼眶框（眉脊 / 眶底 / 外缘）带深影与一点余烬；
    吻部两节 + 上颌侧板 + 鼻垫（带鼻孔）；上牙（门齿 / 犬齿 / 前臼齿 / 裂齿）；颧弓；枕骨。"""
    b = "head"
    p.box(b, "occ_condyle", (-1.4, 34.5, 0.6), (1.4, 35.2, 2.4), "bone_dk")
    p.box(b, "case_low", (-2.6, 35.2, -1.4), (2.6, 37.2, 3.2), "bone")
    p.box(b, "case_up", (-2.4, 37.2, -1.2), (2.4, 40.0, 3.0), "bone_lt")
    p.box(b, "dome", (-1.8, 40.0, -0.8), (1.8, 40.8, 2.6), "bone")
    p.box(b, "crest", (-0.4, 40.8, 0.2), (0.4, 41.7, 2.8), "bone_dk")
    p.box(b, "occiput", (-2.0, 35.8, 3.2), (2.0, 39.0, 3.8), "bone_dk")
    p.box(b, "nuchal_ridge", (-2.2, 38.6, 3.8), (2.2, 39.2, 4.2), "bone_lt")
    p.pair(b, "temporal", (2.6, 36.4, -0.4), (2.9, 39.2, 2.6), "bone_lt", sided=False)
    p.pair(b, "cheek_arch", (2.9, 36.2, -1.9), (3.5, 37.2, 1.4), "bone_dk", sided=False)
    p.pair(b, "crack", (1.9, 40.0, 0.0), (2.3, 40.2, 1.5), "bone_stain", sided=False)
    # 眼眶：眉脊 + 眶底 + 外缘围出一个开口向前的凹槽，槽底是深影，影前一点余烬
    p.pair(b, "brow", (0.9, 39.0, -2.4), (2.9, 39.9, -1.2), "bone_lt", sided=False)
    p.pair(b, "orbit_floor", (0.9, 36.8, -2.2), (2.9, 37.6, -1.4), "bone", sided=False)
    p.pair(b, "orbit_outer", (2.5, 37.6, -2.0), (2.9, 39.0, -1.4), "bone_dk", sided=False)
    p.pair(b, "socket", (0.9, 37.6, -1.4), (2.5, 39.0, -1.1), "void", sided=False)
    p.pair(b, "ember", (1.5, 38.0, -1.7), (2.0, 38.5, -1.4), "ember", sided=False)
    # 吻部
    p.box(b, "nasal_bridge", (-0.9, 36.8, -2.4), (0.9, 39.9, -1.4), "bone")
    p.box(b, "snout_top1", (-1.0, 37.4, -6.0), (1.0, 38.8, -2.4), "bone_lt")
    p.box(b, "snout_top2", (-0.8, 37.4, -8.6), (0.8, 38.4, -6.0), "bone")
    p.box(b, "nose_pad", (-0.8, 37.0, -9.4), (0.8, 38.2, -8.6), "bone_stain")
    p.pair(b, "nostril", (0.2, 37.5, -9.7), (0.6, 37.9, -9.4), "void", sided=False)
    p.pair(b, "maxilla", (1.0, 36.4, -6.4), (1.7, 38.0, -2.4), "bone", sided=False)
    p.pair(b, "premaxilla", (0.8, 36.6, -9.0), (1.3, 38.0, -6.4), "bone_lt", sided=False)
    p.box(b, "palate", (-1.0, 36.4, -6.4), (1.0, 37.4, -2.4), "bone_dk")
    p.box(b, "palate_front", (-0.7, 36.6, -9.0), (0.7, 37.4, -6.4), "bone_dk")
    # 上牙：门齿 2 + 犬齿 + 前臼齿 3 + 裂齿
    p.pair(b, "inc_a", (0.2, 35.9, -9.0), (0.6, 36.6, -8.6), "bone_lt", sided=False)
    p.pair(b, "inc_b", (0.8, 35.9, -8.8), (1.2, 36.6, -8.4), "bone_lt", sided=False)
    p.pair(b, "canine_up", (1.1, 35.0, -7.8), (1.6, 36.6, -7.2), "bone_lt", sided=False)
    for i, z0 in enumerate((-6.2, -4.9, -3.6)):
        p.pair(b, f"premolar_up_{i}", (1.2, 35.8, z0), (1.7, 36.4, z0 + 0.6), "bone", sided=False)
    p.pair(b, "carnassial_up", (1.2, 35.6, -2.8), (1.8, 36.4, -2.0), "bone_lt", sided=False)


def part_jaw(p: Puppet) -> None:
    """下颌：双支（带冠突 / 关节髁 / 下角钩）+ 颏 + 颌底 + 舌 + 下牙（和上牙错位咬合）。"""
    b = "jaw"
    p.pair(b, "mandible", (1.0, 34.2, -7.4), (1.8, 35.0, -1.0), "bone", sided=False)
    p.pair(b, "coronoid", (1.6, 35.0, -1.0), (2.1, 36.4, 0.2), "bone_lt", sided=False)
    p.pair(b, "jaw_hinge", (1.8, 34.4, -0.8), (2.4, 35.4, 0.8), "bone_dk", sided=False)
    p.pair(b, "jaw_angle", (1.0, 33.6, 0.0), (2.4, 34.4, 1.2), "bone_dk", sided=False)
    p.box(b, "chin", (-0.9, 34.2, -8.2), (0.9, 35.0, -7.4), "bone_lt")
    p.box(b, "jaw_floor", (-1.0, 34.2, -7.4), (1.0, 34.6, -1.0), "bone_dk")
    p.box(b, "tongue", (-0.6, 34.6, -6.2), (0.6, 35.2, -2.0), "cloth_red")
    p.pair(b, "inc_low", (0.2, 35.0, -8.2), (0.6, 35.5, -7.8), "bone_lt", sided=False)
    p.pair(b, "canine_low", (1.0, 35.0, -6.6), (1.4, 36.0, -6.0), "bone_lt", sided=False)
    for i, z0 in enumerate((-5.4, -4.1, -2.8)):
        p.pair(b, f"premolar_low_{i}", (1.0, 35.0, z0), (1.4, 35.6, z0 + 0.6), "bone", sided=False)


def part_horns(p: Puppet) -> None:
    """双角：铜箍 + 三节（逐节向外错位变细）+ 绳缠。"""
    b = "horn"
    p.pair(b, "horn_ring", (1.6, 39.9, 0.2), (3.2, 40.6, 1.8), "copper")
    p.pair(b, "horn_seg1", (2.0, 40.6, 0.5), (2.9, 41.8, 1.5), "bone_lt")
    p.pair(b, "horn_lash", (1.9, 40.7, 0.4), (3.0, 41.0, 1.6), "rope")
    p.pair(b, "horn_seg2", (2.4, 41.8, 0.6), (3.2, 43.0, 1.4), "bone")
    p.pair(b, "horn_tip", (2.8, 43.0, 0.7), (3.4, 43.9, 1.3), "bone_dk")


def part_charms(p: Puppet) -> None:
    """头侧挂件：铜眼环 → 绳 → 绳结 → 红布穗（下摆参差）→ 铜牌。"""
    b = "charm"
    p.pair(b, "charm_eyelet", (3.2, 40.0, 0.7), (3.9, 40.5, 1.3), "copper")
    p.pair(b, "charm_cord", (3.5, 38.6, 0.8), (3.9, 40.0, 1.2), "rope")
    p.pair(b, "charm_knot", (3.4, 38.0, 0.7), (4.0, 38.6, 1.3), "rope_dk")
    p.pair(b, "charm_tassel", (3.2, 35.8, 0.5), (4.2, 38.0, 1.5), "cloth_red")
    p.pair(b, "charm_tassel_a", (3.2, 35.0, 0.5), (3.6, 35.8, 1.5), "cloth_red")
    p.pair(b, "charm_tassel_b", (3.8, 35.4, 0.5), (4.2, 35.8, 1.5), "cloth_red")
    p.pair(b, "charm_tag", (3.4, 34.0, 0.8), (4.0, 35.0, 1.2), "copper")


# ───────────────────────────────────────────────────────────── 分部件：肩 / 臂 / 手
def part_shoulders(p: Puppet) -> None:
    """锁骨 + 肩峰（两层）+ 肩胛板（背面，带棘）；锁骨与肩峰交接处绳扎。"""
    b = "shoulder"
    p.pair(b, "clavicle", (1.6, 31.7, -1.4), (5.4, 32.5, -0.4), "bone")
    p.pair(b, "clavicle_lash", (4.4, 31.6, -1.6), (4.9, 32.6, -0.2), "rope")
    p.pair(b, "acromion", (5.4, 30.6, -1.5), (8.2, 32.6, 1.9), "bone_lt")
    p.pair(b, "acromion_top", (5.8, 32.6, -1.1), (7.8, 33.2, 1.5), "bone")
    p.pair(b, "acromion_hoop", (5.1, 31.1, -1.8), (8.4, 31.7, 2.1), "copper")             # 肩峰铜箍
    p.pair(b, "acromion_hoop_rivet_a", (5.8, 31.25, -2.1), (6.3, 31.6, -1.8), "copper_dk")
    p.pair(b, "acromion_hoop_rivet_b", (7.1, 31.25, -2.1), (7.6, 31.6, -1.8), "copper_dk")
    p.pair(b, "scapula", (2.4, 28.0, 2.0), (6.2, 31.8, 2.6), "bone_dk")
    p.pair(b, "scapula_spine", (2.6, 30.6, 2.6), (6.0, 31.1, 3.0), "bone_lt")


def part_arms(p: Puppet) -> None:
    """长臂：上臂（肱骨 + 夹板 + 双绳 + 铜箍）→ 前臂（尺 / 桡骨 + 肘突 + 绳束 + 双铜箍）→ 掌 → 三指 + 拇指（爪尖）。"""
    b = "upper_arm"
    p.pair(b, "humerus_head", (5.8, 29.5, -0.9), (7.8, 30.6, 1.5), "bone_lt")
    p.pair(b, "humerus_shaft", (6.0, 24.0, -0.7), (7.6, 29.5, 1.3), "bone")
    p.pair(b, "deltoid_ridge", (7.6, 27.0, -0.3), (8.0, 29.3, 0.9), "bone_lt")
    p.pair(b, "biceps_groove", (6.5, 25.0, -1.0), (7.1, 28.5, -0.7), "bone_dk")
    p.pair(b, "humerus_splint", (5.5, 25.4, -0.2), (6.0, 28.4, 0.8), "bone_stain")
    p.pair(b, "arm_lash_a", (5.3, 26.0, -0.9), (7.9, 26.6, 1.5), "rope")
    p.pair(b, "arm_lash_b", (5.3, 27.2, -0.9), (7.9, 27.8, 1.5), "rope_dk")
    p.pair(b, "elbow_knob", (5.8, 23.0, -1.0), (7.8, 24.0, 1.6), "bone_lt")
    p.pair(b, "arm_band", (5.7, 24.2, -1.1), (7.9, 25.2, 1.6), "copper")
    p.pair(b, "arm_band_rivet_a", (6.1, 24.5, -1.3), (6.6, 24.9, -1.0), "copper_dk")
    p.pair(b, "arm_band_rivet_b", (7.0, 24.5, -1.3), (7.5, 24.9, -1.0), "copper_dk")
    b = "forearm"
    p.pair(b, "olecranon", (6.2, 21.8, 1.2), (7.4, 23.4, 2.4), "bone_lt")
    p.pair(b, "ulna", (6.8, 15.4, -0.4), (7.8, 22.8, 1.0), "bone")
    p.pair(b, "radius", (5.9, 15.8, -0.6), (6.8, 22.4, 0.8), "bone_lt")
    p.pair(b, "fore_hoop", (5.6, 17.8, -0.9), (8.1, 18.8, 1.5), "copper")
    p.pair(b, "fore_hoop_rivet_a", (6.0, 18.0, -1.2), (6.5, 18.6, -0.9), "copper_dk")
    p.pair(b, "fore_hoop_rivet_b", (7.3, 18.0, -1.2), (7.8, 18.6, -0.9), "copper_dk")
    for i, (y0, y1) in enumerate(((20.0, 20.5), (20.6, 21.1), (21.2, 21.7))):
        p.pair(b, f"fore_lash_{i}", (5.7, y0, -0.8), (8.0, y1, 1.4), "rope" if i % 2 == 0 else "rope_dk")
    p.pair(b, "fore_lash_tail", (6.4, 19.0, -1.1), (6.7, 20.0, -0.6), "rope_dk")
    p.pair(b, "wrist_knob", (5.8, 14.4, -0.9), (7.9, 15.8, 1.3), "bone_lt")
    p.pair(b, "wrist_hoop", (5.6, 15.7, -1.0), (8.0, 16.6, 1.4), "copper")
    b = "hand"
    p.pair(b, "carpal", (5.8, 12.2, -0.7), (7.9, 14.4, 1.3), "bone")
    p.pair(b, "carpal_back", (5.9, 12.4, 1.3), (7.8, 14.0, 1.6), "bone_dk")
    p.pair(b, "wrist_lash", (5.6, 13.6, -0.9), (8.1, 14.1, 1.5), "rope")
    cols = ((5.8, 6.5), (6.55, 7.25), (7.3, 7.9))
    for i, (x0, x1) in enumerate(cols):
        p.pair(b, f"metacarpal_{i}", (x0, 10.8, -0.5), (x1, 12.2, 1.1), "bone_lt" if i == 1 else "bone")
    b = "claws"
    for i, (x0, x1) in enumerate(cols):
        p.pair(b, f"finger_prox_{i}", (x0, 8.6, -0.3), (x1, 10.8, 0.9), "bone")
        dz = 0.05 * (i % 2)                  # 相邻指的绳圈错开一点，避免共面
        p.pair(b, f"finger_lash_{i}", (x0 - 0.1, 8.7 + 0.1 * (i % 2), -0.4 + dz), (x1 + 0.1, 9.0 + 0.1 * (i % 2), 1.0 - dz),
               "rope" if i % 2 == 0 else "rope_dk")
        p.pair(b, f"finger_mid_{i}", (x0 + 0.05, 7.0, -0.6), (x1 - 0.05, 8.6, 0.6), "bone_lt")
        p.pair(b, f"finger_claw_{i}", (x0 + 0.1, 5.8, -0.9), (x1 - 0.1, 7.0, 0.3), "bone_stain")
        p.pair(b, f"finger_tip_{i}", (x0 + 0.2, 5.2, -1.0), (x1 - 0.2, 5.8, 0.0), "iron")
    b = "thumb"
    p.pair(b, "thumb_prox", (5.2, 10.8, -0.4), (5.8, 12.2, 0.6), "bone")
    p.pair(b, "thumb_lash", (5.1, 10.6, -0.6), (5.9, 10.9, 0.7), "rope")
    p.pair(b, "thumb_mid", (5.1, 9.4, -0.6), (5.7, 10.6, 0.4), "bone_lt")
    p.pair(b, "thumb_claw", (5.0, 8.2, -0.8), (5.6, 9.4, 0.2), "bone_stain")
    p.pair(b, "thumb_tip", (5.1, 7.6, -0.7), (5.5, 8.2, -0.1), "iron")


# ───────────────────────────────────────────────────────────── 分部件：货箱 / 抽屉 / 挂件
# 货箱的几何事实集中定义：抽屉三层的底 y、高 h，箱体前后范围。
CRATE_X = 5.5                      # 箱体半宽（外缘）
CRATE_Z = (4.6, 11.6)              # 箱体前（贴背）/ 后（抽屉朝外）
DRAWER_H = 3.0
DRAWER_Y0 = {"drawer_bot": 21.8, "drawer_mid": 25.4, "drawer_top": 29.0}


def part_crate(p: Puppet) -> None:
    """背负木货箱：肩架（两立柱 + 三道横梁，绳扎）→ 四角立柱 + 侧板（四块条板）+ 底板 / 搁板 / 顶板 +
    前后铜角箍 + 层盖（压条 + 铜带 + 提手）；两个挂钩侧板外伸。抽屉另在 part_drawers。"""
    b = "crate"
    # —— 肩架：贴着脊背的两根立柱和横梁，货箱就挂在它上面 ——
    p.pair(b, "frame_post", (4.4, 20.6, 3.2), (5.6, 34.8, 4.6), "wood_dk", sided=False)
    for i, (y0, y1) in enumerate(((21.4, 22.4), (27.4, 28.4), (32.4, 33.6))):
        p.box(b, f"frame_bar_{i}", (-4.4, y0, 3.2), (4.4, y1, 4.6), "wood" if i != 1 else "wood_lt")
        p.pair(b, f"frame_tie_{i}", (4.2, y0 - 0.15, 3.0), (4.8, y1 + 0.15, 4.8), "rope" if i % 2 == 0 else "rope_dk",
               sided=False)
    # 肩带：自肩架上横梁越过肩峰，在胸前垂下一截绳头
    p.pair(b, "shoulder_strap", (5.0, 33.2, -1.0), (6.6, 33.8, 4.8), "rope", sided=False)
    p.pair(b, "shoulder_strap_tail", (5.2, 31.0, -1.9), (6.2, 33.8, -1.5), "rope_dk", sided=False)
    p.pair(b, "shoulder_strap_knot", (5.0, 30.4, -2.1), (6.4, 31.0, -1.5), "copper_dk", sided=False)
    # —— 箱体 ——
    zf, zb = CRATE_Z
    p.pair(b, "post_front", (4.5, 21.0, zf), (CRATE_X, 34.0, zf + 1.0), "wood_dk", sided=False)
    p.pair(b, "post_back", (4.5, 21.0, zb - 1.0), (CRATE_X, 34.0, zb), "wood_dk", sided=False)
    for i, (y0, y1) in enumerate(((21.8, 24.8), (24.8, 27.8), (27.8, 30.8), (30.8, 33.4))):
        p.pair(b, f"side_plank_{i}", (4.9, y0, zf + 1.0), (CRATE_X, y1, zb - 1.0), "wood" if i % 2 == 0 else "wood_lt",
               sided=False)
    p.box(b, "back_panel", (-4.5, 21.8, zf), (4.5, 33.4, 5.0), "wood_dk")
    p.box(b, "bottom_board", (-4.5, 21.0, zf), (4.5, 21.8, zb), "wood_dk")
    for i, y0 in enumerate((24.8, 28.4)):                                       # 两道搁板
        p.box(b, f"shelf_{i}", (-4.5, y0, 5.0), (4.5, y0 + 0.6, zb), "wood")
    p.box(b, "top_board", (-4.5, 32.0, 5.0), (4.5, 33.4, zb), "wood_dk")
    # 铜角箍：四根立柱上下各一圈（上圈挨着层盖，下圈挨着底板）
    for tag, z0 in (("f", zf), ("b", zb - 1.0)):
        p.pair(b, f"corner_band_top_{tag}", (4.3, 32.5, z0 - 0.2), (5.7, 33.3, z0 + 1.1), "copper", sided=False)
        p.pair(b, f"corner_band_low_{tag}", (4.3, 21.2, z0 - 0.2), (5.7, 21.9, z0 + 1.1), "copper", sided=False)
    for i, y in enumerate((23.0, 26.0, 29.0)):                                    # 侧板钉头
        p.pair(b, f"side_nail_{i}", (CRATE_X, y, 6.8), (CRATE_X + 0.3, y + 0.4, 7.2), "copper_dk", sided=False)
        p.pair(b, f"side_nail_b_{i}", (CRATE_X, y, 9.6), (CRATE_X + 0.3, y + 0.4, 10.0), "copper_dk", sided=False)
    # 层盖：伸出一圈檐的木盖 + 三条压条 + 两道铜带 + 绳提手
    p.box(b, "lid", (-5.9, 33.4, 4.2), (5.9, 34.2, 12.0), "wood_lt")
    p.box(b, "lid_rib_mid", (-0.2, 34.2, 4.6), (0.2, 34.5, 11.6), "wood_dk")
    p.pair(b, "lid_rib", (2.0, 34.2, 4.6), (2.4, 34.5, 11.6), "wood_dk", sided=False)
    p.pair(b, "lid_strap", (3.5, 33.0, 4.0), (4.3, 34.5, 12.2), "copper", sided=False)
    p.pair(b, "handle_post", (1.9, 34.5, 7.2), (2.5, 35.3, 8.4), "wood_dk", sided=False)
    p.box(b, "handle_bar", (-2.5, 35.3, 7.2), (2.5, 35.9, 8.4), "wood")
    p.box(b, "handle_wrap", (-1.7, 35.9, 7.0), (1.7, 36.3, 8.6), "rope")
    # 两对挂钩：侧板上钉一块铁背板，向外伸一根杆，杆端向上翘；布包挂在杆端环上
    for i, (y, z) in enumerate(((31.0, 8.0), (25.6, 5.6))):
        p.pair(b, f"hook_plate_{i}", (CRATE_X, y - 0.5, z - 0.8), (CRATE_X + 0.3, y + 1.0, z + 0.8), "iron", sided=False)
        p.pair(b, f"hook_arm_{i}", (CRATE_X + 0.3, y, z - 0.4), (7.8, y + 0.5, z + 0.4), "iron", sided=False)
        p.pair(b, f"hook_tip_{i}", (7.3, y + 0.5, z - 0.4), (7.8, y + 1.1, z + 0.4), "iron", sided=False)


def part_drawers(p: Puppet) -> None:
    """三层抽屉：面板（带凸起嵌板 / 铜拉环 / 骨签 / 锁孔 / 四颗钉）+ 抽屉盒（底 / 双侧 / 后板）。"""
    zf = CRATE_Z[1] - 0.6                                  # 面板里侧 z=11.0
    for bone, y0 in DRAWER_Y0.items():
        p.box(bone, f"{bone}_face", (-4.4, y0, zf), (4.4, y0 + DRAWER_H, 11.8), "wood_lt")
        p.box(bone, f"{bone}_panel", (-3.6, y0 + 0.5, 11.8), (3.6, y0 + 2.5, 12.1), "wood")
        p.pair(bone, f"{bone}_post", (0.9, y0 + 1.2, 12.1), (1.2, y0 + 1.8, 12.4), "copper", sided=False)
        p.box(bone, f"{bone}_pull", (-1.2, y0 + 1.2, 12.4), (1.2, y0 + 1.8, 12.7), "copper")
        p.box(bone, f"{bone}_label", (-1.4, y0 + 2.0, 12.1), (1.4, y0 + 2.4, 12.4), "bone_lt")
        p.box(bone, f"{bone}_keyhole", (-0.2, y0 + 0.7, 12.1), (0.2, y0 + 1.1, 12.4), "void")
        p.pair(bone, f"{bone}_nail_t", (3.8, y0 + 2.3, 11.8), (4.2, y0 + 2.7, 12.1), "copper_dk", sided=False)
        p.pair(bone, f"{bone}_nail_b", (3.8, y0 + 0.3, 11.8), (4.2, y0 + 0.7, 12.1), "copper_dk", sided=False)
        p.box(bone, f"{bone}_floor", (-4.0, y0 + 0.2, 5.6), (4.0, y0 + 0.6, zf), "wood_dk")
        p.pair(bone, f"{bone}_wall", (3.6, y0 + 0.6, 5.6), (4.0, y0 + 2.4, zf), "wood_dk", sided=False)
        p.box(bone, f"{bone}_rear", (-3.6, y0 + 0.6, 5.6), (3.6, y0 + 2.4, 6.0), "wood_dk")


def _bag(p: Puppet, bone: str, cx: float, top: float, cz: float, w: float, h: float, d: float,
         cloth: str, patch: str) -> None:
    """布包：吊绳 → 绳结 → 肩（收窄）→ 腹（最宽）→ 底（收窄）→ 两只扎口耳 + 一块补丁 + 一道针脚。"""
    p.box(bone, f"{bone}_cord", (cx - 0.2, top - 1.0, cz - 0.2), (cx + 0.2, top, cz + 0.2), "rope")
    p.box(bone, f"{bone}_tie", (cx - 0.7, top - 1.6, cz - 0.7), (cx + 0.7, top - 1.0, cz + 0.7), "rope_dk")
    y_shoulder = top - 1.6
    p.box(bone, f"{bone}_ear_a", (cx - 0.9, top - 1.0, cz - 0.3), (cx - 0.3, top - 0.3, cz + 0.3), cloth)
    p.box(bone, f"{bone}_ear_b", (cx + 0.3, top - 1.0, cz - 0.3), (cx + 0.9, top - 0.5, cz + 0.3), cloth)
    p.box(bone, f"{bone}_shoulder", (cx - w / 2 + 0.5, y_shoulder - 0.7, cz - d / 2 + 0.4),
          (cx + w / 2 - 0.5, y_shoulder, cz + d / 2 - 0.4), cloth)
    p.box(bone, f"{bone}_belly", (cx - w / 2, y_shoulder - 0.7 - h, cz - d / 2),
          (cx + w / 2, y_shoulder - 0.7, cz + d / 2), cloth)
    p.box(bone, f"{bone}_base", (cx - w / 2 + 0.5, y_shoulder - 1.2 - h, cz - d / 2 + 0.4),
          (cx + w / 2 - 0.5, y_shoulder - 0.7 - h, cz + d / 2 - 0.4), cloth)
    y_mid = y_shoulder - 0.7 - h / 2
    p.box(bone, f"{bone}_patch", (cx - 0.8, y_mid - 0.6, cz + d / 2), (cx + 0.8, y_mid + 0.6, cz + d / 2 + 0.3), patch)
    p.box(bone, f"{bone}_stitch", (cx - 0.5, y_mid - 0.15, cz + d / 2 + 0.3), (cx + 0.5, y_mid + 0.15, cz + d / 2 + 0.6),
          "rope_dk")


def part_bundles(p: Puppet) -> None:
    """三个布包：左上红 / 左下青 / 右上沙色，分别挂在三个挂钩杆端上；右下钩子挂一圈麻绳。"""
    _bag(p, "bundle_a", -7.55, 31.0, 8.0, 3.0, 3.4, 2.6, "cloth_red", "cloth_sand")
    _bag(p, "bundle_b", -7.55, 25.6, 5.6, 2.6, 3.0, 2.2, "cloth_sage", "cloth_grey")
    _bag(p, "bundle_c", 7.55, 31.0, 8.0, 3.2, 3.6, 2.8, "cloth_sand", "cloth_red")
    # 右下挂钩（y 25.6 z 5.6）：三圈麻绳绕在杆上，一截绳头垂下
    for i, x in enumerate((6.2, 6.8, 7.4)):
        p.box("rope_coil", f"rope_coil_{i}", (x - 0.2, 25.2, 4.8), (x + 0.2, 26.4, 6.4), "rope" if i % 2 == 0 else "rope_dk")
    p.box("rope_coil", "rope_coil_tail", (7.0, 23.8, 5.4), (7.4, 25.2, 5.8), "rope_dk")


def part_coins(p: Puppet) -> None:
    """骨币串：铜眼环挂在层盖右后角，六枚方孔骨币（每枚八片拼成八角环）用绳串起，末端一撮红布穗 + 铜牌。"""
    b = "coin_string"
    cx, z0, z1 = 5.4, 11.6, 12.2
    p.box(b, "coin_eyelet", (cx - 0.8, 33.0, 12.0), (cx + 0.8, 34.0, 12.6), "copper")
    ys = [32.0 - 2.2 * i for i in range(6)]
    for i, yc in enumerate(ys):
        p.box(b, f"coin_{i}_top", (cx - 0.62, yc + 0.55, z0), (cx + 0.62, yc + 1.0, z1), "coin")
        p.box(b, f"coin_{i}_bot", (cx - 0.62, yc - 1.0, z0), (cx + 0.62, yc - 0.55, z1), "coin")
        p.box(b, f"coin_{i}_left", (cx - 1.0, yc - 0.55, z0), (cx - 0.55, yc + 0.55, z1), "coin")
        p.box(b, f"coin_{i}_right", (cx + 0.55, yc - 0.55, z0), (cx + 1.0, yc + 0.55, z1), "coin")
        for sx in (-1, 1):
            for sy in (-1, 1):
                x0 = cx + (0.62 if sx > 0 else -1.0)
                y0 = yc + (0.55 if sy > 0 else -1.0)
                p.box(b, f"coin_{i}_corner_{'p' if sx > 0 else 'n'}{'u' if sy > 0 else 'd'}",
                      (x0, y0, z0 + 0.05), (x0 + 0.38, y0 + 0.45, z1 - 0.05), "bone_dk")
        if i < 5:
            p.box(b, f"coin_link_{i}", (cx - 0.25, ys[i + 1] + 0.9, 11.7), (cx + 0.25, yc - 0.9, 12.1), "rope_dk")
    p.box(b, "coin_tassel_knot", (cx - 0.4, ys[-1] - 1.4, 11.7), (cx + 0.4, ys[-1] - 0.9, 12.1), "rope")
    p.box(b, "coin_tassel", (cx - 0.7, ys[-1] - 3.4, 11.7), (cx + 0.7, ys[-1] - 1.4, 12.1), "cloth_red")
    p.box(b, "coin_tassel_hem", (cx - 0.7, ys[-1] - 4.0, 11.7), (cx - 0.1, ys[-1] - 3.4, 12.1), "cloth_red")
    p.box(b, "coin_tag", (cx + 0.1, ys[-1] - 4.4, 11.8), (cx + 0.7, ys[-1] - 3.4, 12.1), "copper")


# ───────────────────────────────────────────────────────────── 门禁
MIN_BONES = 28                      # 任务卡「骨骼数下限」：人形 / 傀儡 ≥ 28
# 刻意不对称的骨：布包 / 骨币串（挂的东西不必左右一样）、裆布（下摆参差）
ASYM_BONES = frozenset({"bundle_a", "bundle_b", "bundle_c", "rope_coil", "coin_string", "cloth_front", "cloth_back"})


class EntityGates(gatekit.AssetGates):
    """实体模型的门禁：去掉「越出 0..16 方块空间」一道 —— 生物不是方块，GeckoLib 实体不受它限制。"""

    def specs(self):
        return tuple(spec for spec in super().specs() if spec[0] != "overflow")


GATES = EntityGates("游商傀儡 / trade_puppet", MATS, asym=ASYM_BONES, soft_over=SOFT_OVER)


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
    # 骨骼数门的差分：去掉所有手部骨的几何后骨数须掉到下限以下（用一个只含腿的预览模型代替）
    small = build([part_legs])
    if moving_bone_count(small) >= MIN_BONES:
        broken += 1
        print("  ✗ 骨骼数门没有鉴别力")
    else:
        print(f"  ✓ 骨骼数: 完整 {moving_bone_count(p)} ≥ {MIN_BONES}；只含腿的件 {moving_bone_count(small)} 被拦下")
    if broken:
        raise SystemExit(f"自证未通过：{broken} 道门失效")
    print("✓ 差分自证全绿")


def build(parts: list) -> Puppet:
    p = Puppet()
    for fn in parts:
        fn(p)
    return p


PART_FUNCS = {
    "legs": part_legs,
    "hips": part_hips,
    "cloth": part_cloth,
    "spine": part_spine,
    "ribs": part_ribs,
    "disc": part_disc,
    "neck": part_neck,
    "head": part_head,
    "jaw": part_jaw,
    "horns": part_horns,
    "charms": part_charms,
    "shoulders": part_shoulders,
    "arms": part_arms,
    "crate": part_crate,
    "drawers": part_drawers,
    "bundles": part_bundles,
    "coins": part_coins,
}


# ───────────────────────────────────────────────────────────── 输出：bbmodel / 分件预览 / 接触表 / check.png
BG = (119, 119, 119)
# 六个诚实命名的视角：名字写的是相机所在位置（傀儡正面朝 -z，右侧 = +x）。
# render() 的 yaw=180 时相机在 -z 看正面，yaw=0 在 +z 看背面，yaw=90 在 -x 看傀儡左侧。
SIX_VIEWS = (
    ("FRONT 正视（相机在 -z）", 180.0, 0.0),
    ("3/4 前右斜视（相机在 -z / +x 上方）", 150.0, 18.0),
    ("LEFT 左侧视（相机在 -x，看 *_l 侧与货箱侧板）", 90.0, 0.0),
    ("RIGHT 右侧视（相机在 +x，看 *_r 侧与布包 / 骨币串）", -90.0, 0.0),
    ("TOP 俯视（相机在 +y，吻部朝画面下方）", 180.0, 89.9),
    ("BACK 背视（相机在 +z，看货箱抽屉面）", 0.0, 0.0),
)
REF_PANELS = ((0, 405), (405, 815), (815, 1223))     # 参考图上排三张：正 / 侧 / 背
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
        write_model(part, tmp, f"TradePuppet_{key}")
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
    ref_top = ref.crop((0, 0, ref.width, int(ref.height * 0.70)))
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
    d.text((14, 34), f"总骨骼 {bones}（含 root）｜带几何的可动骨 {moving}｜任务卡人形 / 傀儡下限 ≥ {MIN_BONES} ✓",
           fill=(240, 210, 150), font=font_s)
    d.text((14, 54), f"立方体 {p.total()}｜材质 {len(MATS)} 种｜0.5px 精度（坐标均为 0.05px 的整数倍）", fill=(190, 210, 235), font=font_s)
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
    six = render_six_views(model_path, f"trade_puppet（游商傀儡）round 1/3 · {p.total()} cubes · "
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
    d.text((x, 8), "本模型（几何 only：无贴图 / 动画 / 接线 / manifest）", fill=(140, 230, 180), font=font_h)
    canvas.paste(six, (x, 38))
    canvas.paste(row, (x, 38 + six.height + 10))
    x += mid_w + 15
    d.text((x, 8), f"骨骼树（{len(p.rig.bones)} 骨）", fill=(180, 220, 240), font=font_h)
    canvas.paste(tree, (x, 38))
    out = REVIEW_DIR / "check.png"
    canvas.save(out)
    print(f"✓ check.png → {out} ({canvas.width}×{canvas.height})")


def full_run() -> None:
    """全量：自证 → 全套门禁 → 写 bbmodel → 分件预览 → 接触表 / 骨骼树 / check.png。"""
    p = build(list(PART_FUNCS.values()))
    self_test(p)
    total = run_gates(p)
    if total:
        raise SystemExit(f"门禁仍有 {total} 处违例，不出图")
    model = CREATURE_DIR / "TradePuppet.bbmodel"
    write_model(p, model, "TradePuppet")
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
    args = ap.parse_args()
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
    out.write_text(json.dumps(p.rig.bbmodel("TradePuppet")), encoding="utf-8")
    print(f"{p.total()} cubes → {out}")


if __name__ == "__main__":
    main()
