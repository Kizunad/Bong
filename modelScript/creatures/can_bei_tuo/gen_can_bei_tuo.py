#!/usr/bin/env python3
"""残背驮兽（can_bei_tuo）Blockbench .bbmodel 程序化生成器 —— Round 2/3。

设计理念与世界观（正典依据与 Round 2 深度打磨）：
- 任务卡：末世生物批次 #4 can_bei_tuo 残背驮兽（§七 大迁徙）。
- 外形要点与 Round 2 意见全面落实（0.5px 精度，部件数比 Round 1 翻四倍以上）：
  1. 躯干：高耸肩峰驼背 + 圆滚桶状身躯，多层逐级收边方块做出弧面，侧腹增加 4 道皮肤褶皱横纹；
  2. 头与颈：大而低垂的重头，宽扁鼻吻与外扩立体鼻孔，两侧向前弯的 4 段式短粗双角，小眼深陷眼窝带下眼袋与粗眉骨，下巴与颈下挂着分缕灰绿垂藓须；
  3. 噬灵藓：背上三层高低错落的藓块，边缘向下垂成 16 缕参差须状藓舌，点缀 18 处白色寄生小花点，分挂在 7 根独立藓骨上走路能晃；
  4. 腿：粗短柱状，膝部与后肢飞节粗大外凸，每蹄明确分两瓣偶蹄并带前倾蹄尖与后悬蹄，腿侧挂少量藓斑与垂须；
  5. 尾：4 节可摆动短尾，末端带蓬松噬灵藓穗与白花。
- 骨骼架构：
  满足四足兽 >= 32 根硬性下限，共定义 41 根骨骼（含 40 根带几何独立可动关节 + 1 root）：
  脊柱 3 节（hips, lumbar, thorax）、颈 2 节（neck_base, neck_top）+ 颈垂藓 1、
  头 1、下颌 1、下巴垂藓 1、双耳 2、双角 2、尾 4 节、四腿各 4 段（共 16）、背部与侧肋噬灵藓动骨 7 根。

用法：
    python3 modelScript/creatures/can_bei_tuo/gen_can_bei_tuo.py             # 生成分部件预览、完整模型与审阅图
    python3 modelScript/creatures/can_bei_tuo/gen_can_bei_tuo.py --part head # 单部件模型生成
    python3 modelScript/creatures/can_bei_tuo/gen_can_bei_tuo.py --self-test # 运行门禁差分自证
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import random
import sys
import uuid
from pathlib import Path

from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[3]
CREATURE_DIR = Path(__file__).resolve().parent
MODELS_DIR = REPO / "modelScript" / "models"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/creatures/can_bei_tuo")
PARTS_DIR = REVIEW_DIR / "parts"

# 引入渲染工具与中文字体支持
sys.path.insert(0, str(REPO / "modelScript" / "core"))
sys.path.insert(0, str(REPO / "modelScript" / "tools"))
from bbmodel_maker.render.render_bbmodel import render  # noqa: E402
from cjk_font import load_font  # noqa: E402

# ──────────────────────────────────────────────────────────────── 材质调色板
# 13 种末法废土生物材质，每种分配调色板色块
MAT_PALETTE = {
    "hide_base":    {"color": (92, 86, 78),   "noise": 8,  "desc": "灰褐泥土主皮色"},
    "hide_dark":    {"color": (76, 70, 64),   "noise": 8,  "desc": "暗土灰褐糙背皮"},
    "hide_belly":   {"color": (110, 102, 92), "noise": 6,  "desc": "腹侧磨浅糙皮"},
    "skin_fold":    {"color": (62, 56, 50),   "noise": 4,  "desc": "侧面皮肤褶皱横纹暗色"},
    "moss_layer1":  {"color": (76, 96, 74),   "noise": 10, "desc": "噬灵藓深层灰绿底菌"},
    "moss_layer2":  {"color": (98, 122, 94),  "noise": 10, "desc": "噬灵藓灰绿苔藓主体"},
    "moss_crust":   {"color": (124, 146, 116),"noise": 10, "desc": "噬灵藓表层老干苔藓/瘤斑"},
    "moss_flower":  {"color": (238, 242, 235),"noise": 4,  "desc": "噬灵藓寄生微小白花点"},
    "horn_bone":    {"color": (68, 62, 56),   "noise": 6,  "desc": "短粗前弯骨角角质"},
    "snout_horn":   {"color": (58, 52, 48),   "noise": 6,  "desc": "钝吻/低矮骨质鼻瘤"},
    "hoof_horn":    {"color": (46, 42, 40),   "noise": 5,  "desc": "粗壮双瓣脚蹄深角质"},
    "eye":          {"color": (36, 34, 33),   "noise": 2,  "desc": "浑浊呆滞小眼"},
    "socket":       {"color": (28, 26, 25),   "noise": 2,  "desc": "鼻孔与眼窝凹陷深影"},
}

MAT_KEYS = list(MAT_PALETTE.keys())
TEX_W, TEX_H = 64, 64
SWATCH_SIZE = 16


def generate_palette_texture() -> Image.Image:
    """生成 64x64 调色板贴图。"""
    img = Image.new("RGBA", (TEX_W, TEX_H), (0, 0, 0, 255))
    rng = random.Random(20261013)
    for idx, key in enumerate(MAT_KEYS):
        info = MAT_PALETTE[key]
        base_rgb = info["color"]
        noise_amp = info["noise"]
        gx = (idx % 4) * SWATCH_SIZE
        gy = (idx // 4) * SWATCH_SIZE
        for y in range(gy, gy + SWATCH_SIZE):
            for x in range(gx, gx + SWATCH_SIZE):
                nr = base_rgb[0] + rng.randint(-noise_amp, noise_amp)
                ng = base_rgb[1] + rng.randint(-noise_amp, noise_amp)
                nb = base_rgb[2] + rng.randint(-noise_amp, noise_amp)
                r = max(0, min(255, nr))
                g = max(0, min(255, ng))
                b = max(0, min(255, nb))
                img.putpixel((x, y), (r, g, b, 255))
    return img


def get_material_faces(mat: str) -> dict:
    """获取对应材质的 UV 面映射。"""
    idx = MAT_KEYS.index(mat) if mat in MAT_KEYS else 0
    gx = (idx % 4) * SWATCH_SIZE
    gy = (idx // 4) * SWATCH_SIZE
    uv = [gx + 1, gy + 1, gx + SWATCH_SIZE - 1, gy + SWATCH_SIZE - 1]
    return {
        d: {"uv": uv, "texture": 0}
        for d in ("north", "south", "east", "west", "up", "down")
    }


# ──────────────────────────────────────────────────────────────── 骨骼树 Rig
class CreatureRig:
    """用于构建四足动物 Blockbench 骨骼树的容器。"""

    def __init__(self, tex_w: int = 64, tex_h: int = 64) -> None:
        self.elements: list[dict] = []
        self.bones: dict[str, dict] = {}
        self.bone_order: list[str] = []
        self.tex_w = tex_w
        self.tex_h = tex_h

    def bone(self, name: str, pivot: tuple[float, float, float], parent: str | None = None) -> str:
        if name in self.bones:
            raise ValueError(f"重复骨骼: {name}")
        if parent is not None and parent not in self.bones:
            raise ValueError(f"{name} 的父骨骼 {parent} 尚未定义（骨骼须先父后子）")
        self.bones[name] = {
            "uuid": str(uuid.uuid4()),
            "pivot": [round(v, 3) for v in pivot],
            "parent": parent,
            "children": [],
        }
        self.bone_order.append(name)
        return name

    def cube(
        self,
        bone: str,
        name: str,
        frm: tuple[float, float, float],
        to: tuple[float, float, float],
        *,
        mat: str = "hide_base",
        rot: tuple[float, float, float] | None = None,
        org: tuple[float, float, float] | None = None,
    ) -> dict:
        if bone not in self.bones:
            raise ValueError(f"未定义骨骼: {bone}")
        f = [round(min(a, b), 3) for a, b in zip(frm, to)]
        t = [round(max(a, b), 3) for a, b in zip(frm, to)]
        eid = str(uuid.uuid4())
        c = [(a + b) / 2 for a, b in zip(f, t)]
        color_idx = MAT_KEYS.index(mat) % 8 if mat in MAT_KEYS else 0
        el = {
            "name": name,
            "box_uv": False,
            "rescale": False,
            "locked": False,
            "render_order": "default",
            "allow_mirror_modeling": True,
            "type": "cube",
            "uuid": eid,
            "from": f,
            "to": t,
            "autouv": 0,
            "color": color_idx,
            "origin": [round(v, 3) for v in (org or c)],
            "rotation": [round(v, 3) for v in (rot or (0.0, 0.0, 0.0))],
            "faces": get_material_faces(mat),
        }
        self.elements.append(el)
        self.bones[bone]["children"].append(eid)
        return el

    def outliner(self) -> list[dict]:
        nodes: dict[str, dict] = {}
        for name in self.bone_order:
            b = self.bones[name]
            nodes[name] = {
                "name": name,
                "origin": b["pivot"],
                "rotation": [0.0, 0.0, 0.0],
                "uuid": b["uuid"],
                "export": True,
                "mirror_uv": False,
                "isOpen": False,
                "locked": False,
                "visibility": True,
                "autouv": 0,
                "children": list(b["children"]),
            }
        roots: list[dict] = []
        for name in self.bone_order:
            b = self.bones[name]
            p = b["parent"]
            if p is None:
                roots.append(nodes[name])
            else:
                nodes[p]["children"].append(nodes[name])
        return roots

    def bbmodel(self, model_name: str, tex_b64: str) -> dict:
        return {
            "meta": {
                "format_version": "4.10",
                "model_format": "free",
                "box_uv": False,
            },
            "name": model_name,
            "model_identifier": model_name,
            "resolution": {"width": self.tex_w, "height": self.tex_h},
            "elements": self.elements,
            "outliner": self.outliner(),
            "textures": [
                {
                    "path": "",
                    "name": f"{model_name}.png",
                    "folder": "",
                    "namespace": "bong",
                    "id": "0",
                    "particle": True,
                    "render_mode": "default",
                    "visible": True,
                    "mode": "bitmap",
                    "saved": False,
                    "uuid": str(uuid.uuid4()),
                    "source": "data:image/png;base64," + tex_b64,
                }
            ],
        }


# ──────────────────────────────────────────────────────────────── 骨骼树结构定义
# 41 根骨骼定义（含 40 根带几何可动关节 + 1 root，满足四足兽 >= 32 根硬性下限）
BONES_DEF: list[tuple[str, str | None, tuple[float, float, float], str]] = [
    # (name, parent, pivot, description)
    ("root", None, (0.0, 0.0, 0.0), "世界根节点"),

    # 脊柱与主干链 (hips -> lumbar -> thorax -> neck -> head -> jaw)
    ("hips", "root", (0.0, 12.0, 8.0), "骨盆/后臀脊柱 (脊柱第3节)"),
    ("lumbar", "hips", (0.0, 13.5, 2.0), "腰脊柱中段 (脊柱第2节)"),
    ("thorax", "lumbar", (0.0, 14.5, -6.0), "前胸胸廓与肩峰根 (脊柱第1节)"),

    # 颈部 (2 节) 与头部器官
    ("neck_base", "thorax", (0.0, 13.5, -10.8), "颈根下段 (颈第1节)"),
    ("neck_top", "neck_base", (0.0, 11.5, -15.0), "颈上段 (颈第2节)"),
    ("neck_moss_dewlap", "neck_base", (0.0, 6.5, -13.5), "颈喉部悬挂垂藓裙须"),
    ("head", "neck_top", (0.0, 10.0, -17.5), "硕大低垂头颅骨"),
    ("jaw", "head", (0.0, 5.5, -19.5), "张合下颌"),
    ("moss_beard", "jaw", (0.0, 3.8, -26.0), "下巴底悬挂垂藓须"),
    ("ear_l", "head", (-5.35, 12.8, -16.5), "左耳可动关节"),
    ("ear_r", "head", (5.35, 12.8, -16.5), "右耳可动关节"),
    ("horn_l", "head", (-5.0, 14.5, -20.5), "左前弯短粗角"),
    ("horn_r", "head", (5.0, 14.5, -20.5), "右前弯短粗角"),

    # 多节尾链 (4 节)
    ("tail_1", "hips", (0.0, 13.5, 12.0), "尾节 1 (尾根)"),
    ("tail_2", "tail_1", (0.0, 11.0, 12.8), "尾节 2 (尾中)"),
    ("tail_3", "tail_2", (0.0, 8.5, 13.4), "尾节 3 (尾梢)"),
    ("tail_moss", "tail_3", (0.0, 6.0, 13.8), "尾末藓穗毛簇"),

    # 前左肢 (4 段)
    ("leg_fl_thigh", "thorax", (-6.5, 11.0, -7.0), "前左肩胛大腿"),
    ("leg_fl_knee", "leg_fl_thigh", (-6.5, 7.5, -7.0), "前左粗大膝关节"),
    ("leg_fl_shank", "leg_fl_knee", (-6.5, 4.5, -7.0), "前左腕管小腿"),
    ("foot_fl", "leg_fl_shank", (-6.5, 1.8, -7.0), "前左二瓣偶蹄掌"),

    # 前右肢 (4 段)
    ("leg_fr_thigh", "thorax", (6.5, 11.0, -7.0), "前右肩胛大腿"),
    ("leg_fr_knee", "leg_fr_thigh", (6.5, 7.5, -7.0), "前右粗大膝关节"),
    ("leg_fr_shank", "leg_fr_knee", (6.5, 4.5, -7.0), "前右腕管小腿"),
    ("foot_fr", "leg_fr_shank", (6.5, 1.8, -7.0), "前右二瓣偶蹄掌"),

    # 后左肢 (4 段)
    ("leg_bl_thigh", "hips", (-6.5, 11.5, 7.5), "后左股部大腿"),
    ("leg_bl_hock", "leg_bl_thigh", (-6.5, 8.0, 7.5), "后左粗大飞节关节"),
    ("leg_bl_shank", "leg_bl_hock", (-6.5, 4.5, 7.5), "后左跖部小腿"),
    ("foot_bl", "leg_bl_shank", (-6.5, 1.8, 7.5), "后左二瓣偶蹄掌"),

    # 后右肢 (4 段)
    ("leg_br_thigh", "hips", (6.5, 11.5, 7.5), "后右股部大腿"),
    ("leg_br_hock", "leg_br_thigh", (6.5, 8.0, 7.5), "后右粗大飞节关节"),
    ("leg_br_shank", "leg_br_hock", (6.5, 4.5, 7.5), "后右跖部小腿"),
    ("foot_br", "leg_br_shank", (6.5, 1.8, 7.5), "后右二瓣偶蹄掌"),

    # 背脊与侧悬噬灵藓动骨 (7 根，行走/呼吸时晃动)
    ("moss_hump_crest", "thorax", (0.0, 21.0, -3.0), "驼峰顶起伏主藓瘤"),
    ("moss_flank_fl", "thorax", (-6.0, 16.5, -4.0), "前左侧垂挂藓裙"),
    ("moss_flank_fr", "thorax", (6.0, 16.5, -4.0), "前右侧垂挂藓裙"),
    ("moss_spine_mid", "lumbar", (0.0, 18.5, 3.0), "腰脊背阶梯叠苔"),
    ("moss_flank_bl", "lumbar", (-6.0, 16.0, 4.0), "后左侧垂挂藓裙"),
    ("moss_flank_br", "lumbar", (6.0, 16.0, 4.0), "后右侧垂挂藓裙"),
    ("moss_rump_crest", "hips", (0.0, 17.5, 8.0), "后臀背起伏藓台"),
]


def setup_bones(rig: CreatureRig) -> None:
    """定义残背驮兽标准四足骨架层级（41 根骨骼）。"""
    for bname, p, piv, _ in BONES_DEF:
        rig.bone(bname, piv, parent=p)


# ──────────────────────────────────────────────────────────────── 分部件建模
def part_body(rig: CreatureRig) -> None:
    """1. 躯干部分：高耸肩峰驼背 + 圆滚桶状身体，多层 0.5px 逐级收边做出弧面，侧面皮肤褶皱横纹。"""
    # ── hips 骨骼（骨盆与圆润后臀弧面）──
    rig.cube("hips", "torso_lower", (-6.8, 6.15, -9.0), (6.8, 9.42, 11.0), mat="hide_base")
    rig.cube("hips", "torso_lower_bevel", (-6.2, 5.65, -8.2), (6.2, 6.12, 10.2), mat="hide_belly")
    rig.cube("hips", "rump_bevel_mid", (-6.7, 8.4, 11.05), (6.7, 15.0, 11.52), mat="hide_base")
    rig.cube("hips", "rump_mass", (-6.3, 7.8, 11.55), (6.3, 15.6, 13.15), mat="hide_dark")
    rig.cube("hips", "rump_cap_back", (-5.5, 8.8, 13.20), (5.5, 14.6, 13.95), mat="hide_dark")
    rig.cube("hips", "rump_top_slope", (-5.7, 15.65, 11.25), (5.7, 16.55, 12.75), mat="hide_dark")
    rig.cube("hips", "flank_fold_rear_l1", (-7.35, 10.2, 8.4), (-6.85, 13.6, 10.4), mat="skin_fold")
    rig.cube("hips", "flank_fold_rear_r1", (6.85, 10.2, 8.4), (7.35, 13.6, 10.4), mat="skin_fold")
    rig.cube("hips", "flank_fold_rear_l2", (-7.15, 8.8, 10.5), (-6.65, 12.2, 11.45), mat="skin_fold")
    rig.cube("hips", "flank_fold_rear_r2", (6.65, 8.8, 10.5), (7.15, 12.2, 11.45), mat="skin_fold")

    # ── lumbar 骨骼（圆滚桶状中段腰腹 + 0.5px 弧面过渡 + 4 道侧腹横向褶皱）──
    rig.cube("lumbar", "torso_mid_lower_bevel", (-7.2, 9.45, -9.3), (7.2, 10.35, 11.25), mat="hide_base")
    rig.cube("lumbar", "torso_mid", (-7.6, 10.38, -9.6), (7.6, 14.5, 11.5), mat="hide_base")
    rig.cube("lumbar", "torso_mid_upper_bevel", (-7.3, 14.53, -9.4), (7.3, 15.45, 11.35), mat="hide_base")
    rig.cube("lumbar", "belly_sag", (-5.6, 5.10, -5.5), (5.6, 5.62, 7.5), mat="hide_belly")
    rig.cube("lumbar", "belly_keel", (-4.4, 4.60, -3.5), (4.4, 5.07, 5.5), mat="hide_belly")
    rig.cube("lumbar", "spine_ridge", (-4.6, 16.82, 1.75), (4.6, 18.2, 11.5), mat="hide_dark")

    # 侧面皮肤褶皱横纹 (左右各 4 条高低错落横向皮褶)
    rig.cube("lumbar", "skin_fold_l1", (-7.95, 8.5, -4.5), (-7.25, 9.3, 2.5), mat="skin_fold")
    rig.cube("lumbar", "skin_fold_l2", (-8.10, 10.55, -6.5), (-7.63, 11.35, 5.5), mat="skin_fold")
    rig.cube("lumbar", "skin_fold_l3", (-8.00, 12.05, -5.8), (-7.63, 12.80, 4.8), mat="skin_fold")
    rig.cube("lumbar", "skin_fold_l4", (-7.90, 13.50, -5.0), (-7.63, 14.25, 4.0), mat="skin_fold")

    rig.cube("lumbar", "skin_fold_r1", (7.25, 8.5, -4.5), (7.95, 9.3, 2.5), mat="skin_fold")
    rig.cube("lumbar", "skin_fold_r2", (7.63, 10.55, -6.5), (8.10, 11.35, 5.5), mat="skin_fold")
    rig.cube("lumbar", "skin_fold_r3", (7.63, 12.05, -5.8), (8.00, 12.80, 4.8), mat="skin_fold")
    rig.cube("lumbar", "skin_fold_r4", (7.63, 13.50, -5.0), (7.90, 14.25, 4.0), mat="skin_fold")

    # ── thorax 骨骼（前胸弧面与高耸肩峰驼背 4 层阶梯收边）──
    rig.cube("thorax", "torso_upper", (-7.0, 15.48, -9.2), (7.0, 16.8, 11.2), mat="hide_base")
    rig.cube("thorax", "chest_bevel", (-6.8, 7.2, -10.6), (6.8, 15.9, -9.63), mat="hide_base")
    rig.cube("thorax", "chest_mass", (-6.3, 7.6, -11.8), (6.3, 15.6, -10.63), mat="hide_dark")
    rig.cube("thorax", "chest_prow", (-5.6, 8.2, -12.5), (5.6, 14.6, -11.83), mat="hide_dark")
    rig.cube("thorax", "sternum_plate", (-4.6, 6.2, -11.2), (4.6, 7.55, -8.5), mat="hide_belly")

    rig.cube("thorax", "shoulder_fold_l1", (-7.55, 10.8, -9.0), (-7.05, 14.2, -7.6), mat="skin_fold")
    rig.cube("thorax", "shoulder_fold_r1", (7.05, 10.8, -9.0), (7.55, 14.2, -7.6), mat="skin_fold")
    rig.cube("thorax", "shoulder_fold_l2", (-7.40, 9.2, -10.2), (-6.85, 12.6, -9.1), mat="skin_fold")
    rig.cube("thorax", "shoulder_fold_r2", (6.85, 9.2, -10.2), (7.40, 12.6, -9.1), mat="skin_fold")

    # 高耸肩峰驼背 (Withers Hump - 4 层 0.5px 阶梯收边隆起)
    rig.cube("thorax", "hump_base", (-6.0, 16.83, -8.8), (6.0, 18.8, 1.65), mat="hide_dark")
    rig.cube("thorax", "hump_mid", (-5.4, 18.83, -8.2), (5.4, 20.4, 0.6), mat="hide_dark")
    rig.cube("thorax", "hump_peak", (-4.7, 20.43, -7.5), (4.7, 21.8, -0.5), mat="hide_dark")
    rig.cube("thorax", "hump_crest", (-3.8, 21.83, -6.2), (3.8, 22.8, -1.8), mat="hide_dark")


def part_neck(rig: CreatureRig) -> None:
    """2. 颈部与颈下垂藓：粗短垂颈分 2 节（neck_base, neck_top），喉底垂悬多缕藓须（neck_moss_dewlap）。"""
    # ── neck_base（颈根下段）──
    rig.cube("neck_base", "neck_core_rear", (-5.2, 8.8, -14.2), (5.2, 15.4, -10.82), mat="hide_base")
    rig.cube("neck_base", "neck_throat_rear", (-4.4, 7.2, -14.0), (4.4, 8.77, -11.23), mat="hide_belly")
    rig.cube("neck_base", "neck_fold_l", (-5.55, 9.6, -13.8), (-5.23, 14.4, -12.2), mat="skin_fold")
    rig.cube("neck_base", "neck_fold_r", (5.23, 9.6, -13.8), (5.55, 14.4, -12.2), mat="skin_fold")

    # ── neck_top（颈上段前倾）──
    rig.cube("neck_top", "neck_core_front", (-4.9, 8.2, -16.8), (4.9, 14.8, -14.23), mat="hide_base")
    rig.cube("neck_top", "neck_crest", (-4.2, 15.43, -15.8), (4.2, 17.4, -11.9), mat="hide_dark")
    rig.cube("neck_top", "neck_throat_front", (-4.0, 6.8, -16.4), (4.0, 8.17, -14.05), mat="hide_belly")

    # ── neck_moss_dewlap（喉底悬挂垂藓须，随步伐晃动）──
    rig.cube("neck_moss_dewlap", "neck_moss_base", (-3.2, 6.2, -15.2), (3.2, 7.17, -11.8), mat="moss_layer1")
    rig.cube("neck_moss_dewlap", "neck_moss_beard_c", (-1.3, 4.2, -14.4), (1.3, 6.17, -12.4), mat="moss_layer2")
    rig.cube("neck_moss_dewlap", "neck_moss_beard_l", (-2.8, 4.8, -13.9), (-1.35, 6.17, -12.1), mat="moss_layer1")
    rig.cube("neck_moss_dewlap", "neck_moss_beard_r", (1.35, 4.8, -13.9), (2.8, 6.17, -12.1), mat="moss_layer1")
    rig.cube("neck_moss_dewlap", "neck_moss_strand_l", (-2.3, 3.6, -13.5), (-1.4, 4.77, -12.5), mat="moss_crust")
    rig.cube("neck_moss_dewlap", "neck_moss_strand_r", (1.4, 3.6, -13.5), (2.3, 4.77, -12.5), mat="moss_crust")
    rig.cube("neck_moss_dewlap", "neck_moss_tuft_low", (-0.8, 2.9, -13.8), (0.8, 4.17, -12.8), mat="moss_crust")
    rig.cube("neck_moss_dewlap", "neck_moss_floret", (-0.35, 3.3, -14.55), (0.35, 3.9, -13.95), mat="moss_flower")


def part_head(rig: CreatureRig) -> None:
    """3. 头部、双耳、前弯双角与下巴胡藓：大而低垂，宽扁吻部与立体鼻孔，4 段前弯短粗角，深陷小眼。"""
    # ── head 骨骼（硕大低垂脑颅与宽扁鼻吻）──
    rig.cube("head", "head_cranium", (-5.4, 7.6, -23.5), (5.4, 14.5, -16.0), mat="hide_base")
    rig.cube("head", "head_crown", (-4.6, 14.53, -22.2), (4.6, 15.6, -16.6), mat="hide_dark")
    rig.cube("head", "head_forehead", (-4.9, 10.2, -25.2), (4.9, 14.47, -23.53), mat="hide_dark")
    rig.cube("head", "head_moss_patch", (-2.8, 15.63, -20.8), (2.8, 16.4, -17.2), mat="moss_layer1")

    # 宽扁吻部 (0.5px 多级收边与外扩上唇)
    rig.cube("head", "snout_bridge", (-4.4, 9.6, -27.4), (4.4, 12.6, -25.23), mat="hide_base")
    rig.cube("head", "snout_broad", (-4.8, 5.85, -29.6), (4.8, 11.2, -23.55), mat="hide_dark")
    rig.cube("head", "snout_lip_flare_l", (-5.25, 6.0, -29.2), (-4.83, 8.8, -25.6), mat="hide_base")
    rig.cube("head", "snout_lip_flare_r", (4.83, 6.0, -29.2), (5.25, 8.8, -25.6), mat="hide_base")
    rig.cube("head", "snout_pad", (-4.3, 6.0, -30.5), (4.3, 10.8, -29.63), mat="snout_horn")
    rig.cube("head", "snout_pad_bevel", (-3.6, 6.4, -30.95), (3.6, 10.2, -30.53), mat="snout_horn")
    rig.cube("head", "snout_lip_upper", (-3.9, 5.35, -29.4), (3.9, 5.82, -26.2), mat="hide_dark")
    rig.cube("head", "snout_fold_1", (-3.2, 11.23, -28.6), (3.2, 11.85, -27.5), mat="skin_fold")
    rig.cube("head", "snout_fold_2", (-3.6, 11.23, -26.8), (3.6, 11.95, -25.6), mat="skin_fold")

    # 立体外扩鼻翼与深凹鼻孔
    rig.cube("head", "nostril_rim_l", (-5.15, 7.2, -29.5), (-4.86, 9.5, -27.8), mat="snout_horn")
    rig.cube("head", "nostril_l", (-3.4, 6.8, -31.25), (-1.5, 8.6, -30.98), mat="socket")
    rig.cube("head", "nostril_rim_r", (4.86, 7.2, -29.5), (5.15, 9.5, -27.8), mat="snout_horn")
    rig.cube("head", "nostril_r", (1.5, 6.8, -31.25), (3.4, 8.6, -30.98), mat="socket")

    # 鼻梁骨质粗钝角瘤
    rig.cube("head", "snout_boss", (-2.1, 12.63, -27.2), (2.1, 13.8, -24.2), mat="snout_horn")
    rig.cube("head", "snout_boss_apex", (-1.3, 13.83, -26.4), (1.3, 14.6, -24.8), mat="snout_horn")

    # 深陷小眼眶、浑浊眼珠、粗重眉脊与眼袋
    rig.cube("head", "eye_socket_l", (-5.65, 10.6, -20.2), (-5.43, 12.5, -17.8), mat="socket")
    rig.cube("head", "eye_pupil_l", (-5.78, 11.0, -19.5), (-5.67, 12.0, -18.5), mat="eye")
    rig.cube("head", "brow_ridge_l", (-5.95, 12.53, -20.6), (-5.15, 13.6, -17.4), mat="hide_dark")
    rig.cube("head", "eye_bag_l", (-5.80, 9.8, -20.0), (-5.43, 10.57, -18.0), mat="skin_fold")

    rig.cube("head", "eye_socket_r", (5.43, 10.6, -20.2), (5.65, 12.5, -17.8), mat="socket")
    rig.cube("head", "eye_pupil_r", (5.67, 11.0, -19.5), (5.78, 12.0, -18.5), mat="eye")
    rig.cube("head", "brow_ridge_r", (5.15, 12.53, -20.6), (5.95, 13.6, -17.4), mat="hide_dark")
    rig.cube("head", "eye_bag_r", (5.43, 9.8, -20.0), (5.80, 10.57, -18.0), mat="skin_fold")

    # 颊部厚咬肌
    rig.cube("head", "jaw_cheek_l", (-5.1, 5.8, -23.2), (-4.1, 7.57, -17.5), mat="hide_dark")
    rig.cube("head", "jaw_cheek_r", (4.1, 5.8, -23.2), (5.1, 7.57, -17.5), mat="hide_dark")

    # ── ear_l & ear_r 骨骼（双耳独立可动，3 段下垂厚耳）──
    rig.cube("ear_l", "ear_base_l", (-6.9, 11.6, -17.3), (-5.43, 13.45, -15.5), mat="hide_dark")
    rig.cube("ear_l", "ear_mid_l", (-7.8, 10.2, -17.1), (-6.93, 12.2, -15.7), mat="hide_base")
    rig.cube("ear_l", "ear_tip_l", (-8.3, 9.1, -16.9), (-7.83, 10.8, -15.9), mat="hide_dark")

    rig.cube("ear_r", "ear_base_r", (5.43, 11.6, -17.3), (6.9, 13.45, -15.5), mat="hide_dark")
    rig.cube("ear_r", "ear_mid_r", (6.93, 10.2, -17.1), (7.8, 12.2, -15.7), mat="hide_base")
    rig.cube("ear_r", "ear_tip_r", (7.83, 9.1, -16.9), (8.3, 10.8, -15.9), mat="hide_dark")

    # ── horn_l & horn_r 骨骼（两侧向前弯的短粗角，4 段 0.5px 递进弧度）──
    rig.cube("horn_l", "horn_root_l", (-6.5, 13.2, -22.0), (-5.43, 15.5, -19.5), mat="horn_bone")
    rig.cube("horn_l", "horn_base_l", (-7.6, 13.5, -23.8), (-6.53, 15.4, -21.0), mat="horn_bone")
    rig.cube("horn_l", "horn_mid_l", (-8.2, 13.8, -25.8), (-7.0, 15.3, -23.83), mat="horn_bone")
    rig.cube("horn_l", "horn_tip_l", (-7.8, 14.1, -27.6), (-6.6, 15.2, -25.83), mat="snout_horn")

    rig.cube("horn_r", "horn_root_r", (5.43, 13.2, -22.0), (6.5, 15.5, -19.5), mat="horn_bone")
    rig.cube("horn_r", "horn_base_r", (6.53, 13.5, -23.8), (7.6, 15.4, -21.0), mat="horn_bone")
    rig.cube("horn_r", "horn_mid_r", (7.0, 13.8, -25.8), (8.2, 15.3, -23.83), mat="horn_bone")
    rig.cube("horn_r", "horn_tip_r", (6.6, 14.1, -27.6), (7.8, 15.2, -25.83), mat="snout_horn")

    # ── jaw 骨骼（宽实下颌与钝下巴）──
    rig.cube("jaw", "jaw_box", (-4.0, 3.8, -28.4), (4.0, 5.75, -18.5), mat="hide_base")
    rig.cube("jaw", "jaw_chin", (-3.4, 4.0, -29.6), (3.4, 5.65, -28.43), mat="hide_dark")
    rig.cube("jaw", "jaw_under", (-3.0, 3.2, -26.8), (3.0, 3.77, -20.2), mat="hide_belly")

    # ── moss_beard 骨骼（下巴底垂挂多缕藓须与小花点）──
    rig.cube("moss_beard", "jaw_moss_beard_c", (-1.2, 1.6, -27.5), (1.2, 3.17, -24.5), mat="moss_layer2")
    rig.cube("moss_beard", "jaw_moss_beard_f", (-0.85, 2.2, -29.0), (0.85, 3.95, -27.53), mat="moss_crust")
    rig.cube("moss_beard", "jaw_moss_beard_l", (-2.2, 2.1, -26.6), (-1.23, 3.75, -24.8), mat="moss_layer1")
    rig.cube("moss_beard", "jaw_moss_beard_r", (1.23, 2.1, -26.6), (2.2, 3.75, -24.8), mat="moss_layer1")
    rig.cube("moss_beard", "jaw_moss_tip", (-0.6, 0.8, -26.6), (0.6, 1.57, -25.4), mat="moss_crust")
    rig.cube("moss_beard", "jaw_moss_floret", (-0.35, 1.2, -27.95), (0.35, 1.85, -27.3), mat="moss_flower")


def part_moss(rig: CreatureRig) -> None:
    """4. 背脊噬灵藓：三层高低错落藓块，边缘向下垂成 16 缕参差须状，点缀 18 处白花点，分挂 7 根动骨。"""
    # ── moss_hump_crest 骨骼（驼峰顶三层错落主苔与老干藓瘤）──
    rig.cube("moss_hump_crest", "moss_hump_base", (-5.8, 18.85, -8.5), (5.8, 20.38, -4.75), mat="moss_layer1")
    rig.cube("moss_hump_crest", "moss_hump_slope", (-5.3, 20.42, -7.8), (5.3, 21.78, -4.93), mat="moss_layer2")
    rig.cube("moss_hump_crest", "moss_hump_peak", (-5.1, 21.85, -4.9), (5.1, 23.3, 0.4), mat="moss_layer2")
    rig.cube("moss_hump_crest", "moss_crest_peak_c", (-2.6, 23.33, -4.0), (2.6, 24.6, -1.05), mat="moss_crust")
    rig.cube("moss_hump_crest", "moss_crest_peak_l", (-3.8, 23.33, -3.2), (-2.63, 24.15, 0.2), mat="moss_crust")
    rig.cube("moss_hump_crest", "moss_crest_peak_r", (2.63, 23.33, -3.2), (3.8, 24.15, 0.2), mat="moss_crust")
    rig.cube("moss_hump_crest", "moss_nodule_hump_front", (-2.0, 21.82, -7.2), (2.0, 22.75, -5.0), mat="moss_crust")
    rig.cube("moss_hump_crest", "moss_nodule_hump_rear", (-2.5, 23.33, -0.95), (-0.8, 24.0, 0.3), mat="moss_crust")
    rig.cube("moss_hump_crest", "floret_peak_1", (-1.4, 24.63, -2.8), (-0.6, 25.15, -2.0), mat="moss_flower")
    rig.cube("moss_hump_crest", "floret_peak_2", (0.8, 24.63, -2.2), (1.6, 25.15, -1.4), mat="moss_flower")
    rig.cube("moss_hump_crest", "floret_hump_l", (-3.5, 24.18, -1.8), (-2.8, 24.7, -1.1), mat="moss_flower")
    rig.cube("moss_hump_crest", "floret_hump_r", (2.8, 24.18, -1.5), (3.5, 24.7, -0.8), mat="moss_flower")
    rig.cube("moss_hump_crest", "floret_hump_front", (-0.5, 22.78, -6.4), (0.3, 23.3, -5.6), mat="moss_flower")

    # ── moss_flank_fl 骨骼（前左侧垂挂参差藓须裙）──
    rig.cube("moss_flank_fl", "moss_hang_l1", (-7.35, 15.6, -7.2), (-6.23, 18.2, -5.0), mat="moss_layer1")
    rig.cube("moss_flank_fl", "moss_hang_l1_tip", (-7.45, 14.3, -6.6), (-6.55, 15.57, -5.4), mat="moss_crust")
    rig.cube("moss_flank_fl", "moss_hang_l2", (-7.55, 14.8, -4.8), (-6.23, 18.0, -2.2), mat="moss_layer2")
    rig.cube("moss_flank_fl", "moss_hang_l2_tip", (-7.65, 13.4, -4.1), (-6.75, 14.77, -2.8), mat="moss_layer1")
    rig.cube("moss_flank_fl", "moss_hang_l3", (-7.40, 15.5, -2.0), (-6.23, 18.2, 0.2), mat="moss_layer1")
    rig.cube("moss_flank_fl", "moss_boss_shoulder_l", (-4.8, 20.48, -7.2), (-2.8, 21.2, -5.2), mat="moss_crust")
    rig.cube("moss_flank_fl", "floret_shoulder_l", (-4.2, 21.23, -6.4), (-3.5, 21.75, -5.7), mat="moss_flower")
    rig.cube("moss_flank_fl", "floret_flank_l1", (-7.85, 16.2, -3.8), (-7.58, 16.9, -3.1), mat="moss_flower")

    # ── moss_flank_fr 骨骼（前右侧垂挂参差藓须裙）──
    rig.cube("moss_flank_fr", "moss_hang_r1", (6.23, 15.6, -7.2), (7.35, 18.2, -5.0), mat="moss_layer1")
    rig.cube("moss_flank_fr", "moss_hang_r1_tip", (6.55, 14.3, -6.6), (7.45, 15.57, -5.4), mat="moss_crust")
    rig.cube("moss_flank_fr", "moss_hang_r2", (6.23, 14.8, -4.8), (7.55, 18.0, -2.2), mat="moss_layer2")
    rig.cube("moss_flank_fr", "moss_hang_r2_tip", (6.75, 13.4, -4.1), (7.65, 14.77, -2.8), mat="moss_layer1")
    rig.cube("moss_flank_fr", "moss_hang_r3", (6.23, 15.5, -2.0), (7.40, 18.2, 0.2), mat="moss_layer1")
    rig.cube("moss_flank_fr", "moss_boss_shoulder_r", (2.8, 20.48, -7.2), (4.8, 21.2, -5.2), mat="moss_crust")
    rig.cube("moss_flank_fr", "floret_shoulder_r", (3.5, 21.23, -6.4), (4.2, 21.75, -5.7), mat="moss_flower")
    rig.cube("moss_flank_fr", "floret_flank_r1", (7.58, 16.2, -3.8), (7.85, 16.9, -3.1), mat="moss_flower")

    # ── moss_spine_mid 骨骼（腰脊背三层阶梯叠苔）──
    rig.cube("moss_spine_mid", "moss_base_mid", (-6.2, 17.5, -4.55), (6.2, 19.0, 9.2), mat="moss_layer1")
    rig.cube("moss_spine_mid", "moss_spine_shelf_a", (-5.0, 19.03, 0.45), (5.0, 20.3, 4.8), mat="moss_layer2")
    rig.cube("moss_spine_mid", "moss_spine_shelf_b", (-4.65, 19.03, 4.85), (4.65, 19.95, 8.8), mat="moss_layer2")
    rig.cube("moss_spine_mid", "moss_boss_spine_1", (-2.4, 20.33, 1.2), (0.2, 21.6, 3.8), mat="moss_crust")
    rig.cube("moss_spine_mid", "moss_boss_spine_2", (-0.2, 20.33, 3.85), (2.5, 21.2, 6.4), mat="moss_crust")
    rig.cube("moss_spine_mid", "moss_nodule_spine_top", (-1.2, 21.63, 2.2), (1.2, 22.6, 3.75), mat="moss_crust")
    rig.cube("moss_spine_mid", "floret_spine_1", (-1.8, 21.63, 1.4), (-1.1, 22.15, 2.1), mat="moss_flower")
    rig.cube("moss_spine_mid", "floret_spine_2", (1.2, 21.23, 5.0), (1.9, 21.75, 5.7), mat="moss_flower")
    rig.cube("moss_spine_mid", "floret_spine_3", (-0.3, 22.63, 2.7), (0.4, 23.15, 3.4), mat="moss_flower")

    # ── moss_flank_bl 骨骼（后左肋垂挂参差藓须裙）──
    rig.cube("moss_flank_bl", "moss_hang_l4", (-7.45, 15.2, 0.4), (-6.23, 18.1, 2.8), mat="moss_layer2")
    rig.cube("moss_flank_bl", "moss_hang_l4_tip", (-7.55, 13.8, 1.0), (-6.65, 15.17, 2.2), mat="moss_crust")
    rig.cube("moss_flank_bl", "moss_hang_l5", (-7.50, 14.6, 3.0), (-6.23, 17.8, 5.6), mat="moss_layer1")
    rig.cube("moss_flank_bl", "moss_hang_l5_tip", (-7.65, 13.2, 3.7), (-6.75, 14.57, 4.9), mat="moss_layer2")
    rig.cube("moss_flank_bl", "moss_hang_l6", (-7.30, 15.6, 5.8), (-6.23, 18.0, 8.4), mat="moss_layer1")
    rig.cube("moss_flank_bl", "moss_hang_l7", (-6.95, 16.0, 8.6), (-5.85, 18.2, 10.8), mat="moss_layer2")
    rig.cube("moss_flank_bl", "floret_flank_l2", (-7.78, 15.6, 3.8), (-7.53, 16.3, 4.5), mat="moss_flower")
    rig.cube("moss_flank_bl", "floret_side_hang_l", (-7.72, 14.55, 1.2), (-7.48, 15.1, 1.9), mat="moss_flower")

    # ── moss_flank_br 骨骼（后右肋垂挂参差藓须裙）──
    rig.cube("moss_flank_br", "moss_hang_r4", (6.23, 15.2, 0.4), (7.45, 18.1, 2.8), mat="moss_layer2")
    rig.cube("moss_flank_br", "moss_hang_r4_tip", (6.65, 13.8, 1.0), (7.55, 15.17, 2.2), mat="moss_crust")
    rig.cube("moss_flank_br", "moss_hang_r5", (6.23, 14.6, 3.0), (7.50, 17.8, 5.6), mat="moss_layer1")
    rig.cube("moss_flank_br", "moss_hang_r5_tip", (6.75, 13.2, 3.7), (7.65, 14.57, 4.9), mat="moss_layer2")
    rig.cube("moss_flank_br", "moss_hang_r6", (6.23, 15.6, 5.8), (7.30, 18.0, 8.4), mat="moss_layer1")
    rig.cube("moss_flank_br", "moss_hang_r7", (5.85, 16.0, 8.6), (6.95, 18.2, 10.8), mat="moss_layer2")
    rig.cube("moss_flank_br", "floret_flank_r2", (7.53, 15.6, 3.8), (7.78, 16.3, 4.5), mat="moss_flower")
    rig.cube("moss_flank_br", "floret_side_hang_r", (7.48, 14.55, 1.2), (7.72, 15.1, 1.9), mat="moss_flower")

    # ── moss_rump_crest 骨骼（后臀背起伏藓台）──
    rig.cube("moss_rump_crest", "moss_rump_base", (-4.4, 17.2, 9.25), (4.4, 18.8, 12.4), mat="moss_layer1")
    rig.cube("moss_rump_crest", "moss_rump_shelf", (-3.6, 18.83, 8.85), (3.6, 19.65, 11.6), mat="moss_layer2")
    rig.cube("moss_rump_crest", "moss_boss_rump", (-1.8, 19.68, 7.8), (1.8, 20.6, 10.6), mat="moss_crust")
    rig.cube("moss_rump_crest", "floret_rump_1", (-0.5, 20.63, 9.0), (0.3, 21.15, 9.8), mat="moss_flower")
    rig.cube("moss_rump_crest", "floret_rump_2", (1.2, 19.68, 10.8), (1.9, 20.2, 11.5), mat="moss_flower")


def part_legs(rig: CreatureRig) -> None:
    """5. 四肢短腿：粗短柱状，膝部与飞节粗大外凸，蹄分两瓣偶蹄（带前倾蹄尖与后悬蹄），腿侧挂少量藓。"""
    # ── 前左肢 (FL: thigh -> knee -> shank -> foot) ──
    rig.cube("leg_fl_thigh", "thigh_fl_main", (-8.9, 5.85, -9.4), (-4.3, 11.2, -4.6), mat="hide_dark")
    rig.cube("leg_fl_thigh", "thigh_fl_muscle", (-9.20, 6.4, -8.6), (-8.93, 10.5, -5.4), mat="hide_base")
    rig.cube("leg_fl_thigh", "moss_leg_fl_top", (-9.45, 7.8, -8.2), (-9.23, 10.0, -5.8), mat="moss_layer1")
    rig.cube("leg_fl_thigh", "moss_leg_fl_tip", (-9.55, 6.5, -7.6), (-9.23, 7.77, -6.4), mat="moss_layer2")

    rig.cube("leg_fl_knee", "knee_node_fl", (-9.05, 4.15, -9.75), (-4.25, 5.82, -4.35), mat="hide_dark")
    rig.cube("leg_fl_knee", "knee_cap_fl", (-8.5, 4.4, -10.15), (-4.8, 5.6, -9.78), mat="skin_fold")

    rig.cube("leg_fl_shank", "shank_fl", (-8.6, 1.85, -9.0), (-4.6, 4.12, -5.0), mat="hide_base")
    rig.cube("leg_fl_shank", "pastern_fl", (-8.8, 1.85, -9.2), (-4.4, 2.55, -9.03), mat="hide_dark")
    rig.cube("leg_fl_shank", "dewclaw_fl", (-7.6, 1.6, -4.97), (-5.6, 2.4, -4.4), mat="hoof_horn")

    # 二瓣偶蹄 (外瓣与内瓣，留 0.5px 清晰蹄裂 + 前突蹄尖)
    rig.cube("foot_fl", "hoof_fl_outer", (-8.95, 0.0, -9.3), (-6.85, 1.82, -4.7), mat="hoof_horn")
    rig.cube("foot_fl", "hoof_fl_outer_tip", (-8.85, 0.0, -9.85), (-6.95, 1.25, -9.33), mat="hoof_horn")
    rig.cube("foot_fl", "hoof_fl_inner", (-6.35, 0.0, -9.3), (-4.25, 1.82, -4.7), mat="hoof_horn")
    rig.cube("foot_fl", "hoof_fl_inner_tip", (-6.25, 0.0, -9.85), (-4.35, 1.25, -9.33), mat="hoof_horn")

    # ── 前右肢 (FR: thigh -> knee -> shank -> foot) ──
    rig.cube("leg_fr_thigh", "thigh_fr_main", (4.3, 5.85, -9.4), (8.9, 11.2, -4.6), mat="hide_dark")
    rig.cube("leg_fr_thigh", "thigh_fr_muscle", (8.93, 6.4, -8.6), (9.20, 10.5, -5.4), mat="hide_base")
    rig.cube("leg_fr_thigh", "moss_leg_fr_top", (9.23, 7.8, -8.2), (9.45, 10.0, -5.8), mat="moss_layer1")
    rig.cube("leg_fr_thigh", "moss_leg_fr_tip", (9.23, 6.5, -7.6), (9.55, 7.77, -6.4), mat="moss_layer2")

    rig.cube("leg_fr_knee", "knee_node_fr", (4.25, 4.15, -9.75), (9.05, 5.82, -4.35), mat="hide_dark")
    rig.cube("leg_fr_knee", "knee_cap_fr", (4.8, 4.4, -10.15), (8.5, 5.6, -9.78), mat="skin_fold")

    rig.cube("leg_fr_shank", "shank_fr", (4.6, 1.85, -9.0), (8.6, 4.12, -5.0), mat="hide_base")
    rig.cube("leg_fr_shank", "pastern_fr", (4.4, 1.85, -9.2), (8.8, 2.55, -9.03), mat="hide_dark")
    rig.cube("leg_fr_shank", "dewclaw_fr", (5.6, 1.6, -4.97), (7.6, 2.4, -4.4), mat="hoof_horn")

    rig.cube("foot_fr", "hoof_fr_inner", (4.25, 0.0, -9.3), (6.35, 1.82, -4.7), mat="hoof_horn")
    rig.cube("foot_fr", "hoof_fr_inner_tip", (4.35, 0.0, -9.85), (6.25, 1.25, -9.33), mat="hoof_horn")
    rig.cube("foot_fr", "hoof_fr_outer", (6.85, 0.0, -9.3), (8.95, 1.82, -4.7), mat="hoof_horn")
    rig.cube("foot_fr", "hoof_fr_outer_tip", (6.95, 0.0, -9.85), (8.85, 1.25, -9.33), mat="hoof_horn")

    # ── 后左肢 (BL: thigh -> hock -> shank -> foot) ──
    rig.cube("leg_bl_thigh", "thigh_bl_main", (-8.9, 5.85, 5.0), (-4.3, 11.6, 10.0), mat="hide_dark")
    rig.cube("leg_bl_thigh", "thigh_bl_muscle", (-9.20, 6.4, 5.8), (-8.93, 10.8, 9.2), mat="hide_base")
    rig.cube("leg_bl_thigh", "moss_leg_bl_top", (-9.45, 8.0, 6.2), (-9.23, 10.2, 8.8), mat="moss_layer1")
    rig.cube("leg_bl_thigh", "moss_leg_bl_tip", (-9.55, 6.7, 6.8), (-9.23, 7.97, 8.2), mat="moss_crust")

    rig.cube("leg_bl_hock", "hock_node_bl", (-9.05, 4.15, 4.8), (-4.25, 5.82, 10.35), mat="hide_dark")
    rig.cube("leg_bl_hock", "hock_spur_bl", (-8.4, 4.4, 10.38), (-4.9, 5.6, 10.85), mat="skin_fold")

    rig.cube("leg_bl_shank", "shank_bl", (-8.6, 1.85, 5.4), (-4.6, 4.12, 9.6), mat="hide_base")
    rig.cube("leg_bl_shank", "pastern_bl", (-8.8, 1.85, 5.15), (-4.4, 2.55, 5.37), mat="hide_dark")
    rig.cube("leg_bl_shank", "dewclaw_bl", (-7.6, 1.6, 9.63), (-5.6, 2.4, 10.2), mat="hoof_horn")

    rig.cube("foot_bl", "hoof_bl_outer", (-8.95, 0.0, 5.1), (-6.85, 1.82, 9.9), mat="hoof_horn")
    rig.cube("foot_bl", "hoof_bl_outer_tip", (-8.85, 0.0, 4.55), (-6.95, 1.25, 5.07), mat="hoof_horn")
    rig.cube("foot_bl", "hoof_bl_inner", (-6.35, 0.0, 5.1), (-4.25, 1.82, 9.9), mat="hoof_horn")
    rig.cube("foot_bl", "hoof_bl_inner_tip", (-6.25, 0.0, 4.55), (-4.35, 1.25, 5.07), mat="hoof_horn")

    # ── 后右肢 (BR: thigh -> hock -> shank -> foot) ──
    rig.cube("leg_br_thigh", "thigh_br_main", (4.3, 5.85, 5.0), (8.9, 11.6, 10.0), mat="hide_dark")
    rig.cube("leg_br_thigh", "thigh_br_muscle", (8.93, 6.4, 5.8), (9.20, 10.8, 9.2), mat="hide_base")
    rig.cube("leg_br_thigh", "moss_leg_br_top", (9.23, 8.0, 6.2), (9.45, 10.2, 8.8), mat="moss_layer1")
    rig.cube("leg_br_thigh", "moss_leg_br_tip", (9.23, 6.7, 6.8), (9.55, 7.97, 8.2), mat="moss_crust")

    rig.cube("leg_br_hock", "hock_node_br", (4.25, 4.15, 4.8), (9.05, 5.82, 10.35), mat="hide_dark")
    rig.cube("leg_br_hock", "hock_spur_br", (4.9, 4.4, 10.38), (8.4, 5.6, 10.85), mat="skin_fold")

    rig.cube("leg_br_shank", "shank_br", (4.6, 1.85, 5.4), (8.6, 4.12, 9.6), mat="hide_base")
    rig.cube("leg_br_shank", "pastern_br", (4.4, 1.85, 5.15), (8.8, 2.55, 5.37), mat="hide_dark")
    rig.cube("leg_br_shank", "dewclaw_br", (5.6, 1.6, 9.63), (7.6, 2.4, 10.2), mat="hoof_horn")

    rig.cube("foot_br", "hoof_br_inner", (4.25, 0.0, 5.1), (6.35, 1.82, 9.9), mat="hoof_horn")
    rig.cube("foot_br", "hoof_br_inner_tip", (4.35, 0.0, 4.55), (6.25, 1.25, 5.07), mat="hoof_horn")
    rig.cube("foot_br", "hoof_br_outer", (6.85, 0.0, 5.1), (8.95, 1.82, 9.9), mat="hoof_horn")
    rig.cube("foot_br", "hoof_br_outer_tip", (6.95, 0.0, 4.55), (8.85, 1.25, 5.07), mat="hoof_horn")


def part_tail(rig: CreatureRig) -> None:
    """6. 短粗垂尾：4 节可摆动骨骼链（tail_1 -> tail_2 -> tail_3 -> tail_moss），末端带蓬松噬灵藓穗。"""
    rig.cube("tail_1", "tail_root", (-1.5, 11.2, 13.4), (1.5, 13.8, 14.35), mat="hide_dark")
    rig.cube("tail_1", "tail_stem", (-1.2, 8.6, 12.7), (1.2, 11.17, 14.2), mat="hide_base")

    rig.cube("tail_2", "tail_hair_base", (-1.45, 6.2, 12.9), (1.45, 8.57, 14.6), mat="hide_dark")
    rig.cube("tail_2", "tail_moss_wrap", (-1.75, 6.6, 13.1), (-1.48, 8.1, 14.4), mat="moss_layer1")
    rig.cube("tail_2", "tail_moss_wrap_r", (1.48, 6.6, 13.1), (1.75, 8.1, 14.4), mat="moss_layer1")

    rig.cube("tail_3", "tail_moss_tuft", (-1.85, 4.3, 13.0), (1.85, 6.17, 15.1), mat="moss_layer2")
    rig.cube("tail_3", "tail_moss_side_l", (-2.25, 4.6, 13.3), (-1.88, 5.9, 14.7), mat="moss_layer1")
    rig.cube("tail_3", "tail_moss_side_r", (1.88, 4.6, 13.3), (2.25, 5.9, 14.7), mat="moss_layer1")

    rig.cube("tail_moss", "tail_tassel_tip", (-1.2, 3.0, 13.3), (1.2, 4.27, 14.8), mat="moss_crust")
    rig.cube("tail_moss", "tail_floret", (-0.45, 3.4, 14.83), (0.45, 4.1, 15.4), mat="moss_flower")


PARTS_META = {
    "body": ("1. 躯干：高耸肩峰驼背 + 圆滚桶状多层弧面 + 4 道侧腹皮褶", [part_body]),
    "head": ("2. 头部与垂颈：硕大低垂头 + 宽吻鼻孔 + 4段前弯角 + 喉/颏垂藓", [part_neck, part_head]),
    "moss": ("3. 噬灵藓：三层错落叠苔 + 16 缕侧垂藓须 + 18 处白花点 (7 根动骨)", [part_moss]),
    "legs": ("4. 四肢短腿：粗短柱腿 + 外凸粗膝/飞节 + 双瓣偶蹄 + 腿侧附藓", [part_legs]),
    "tail": ("5. 短尾：4 节可动短尾 + 蓬松垂藓穗与白花", [part_tail]),
}


def build_rig(part: str = "all") -> CreatureRig:
    """根据指定部件名称构建 Rig。"""
    rig = CreatureRig(tex_w=TEX_W, tex_h=TEX_H)
    setup_bones(rig)

    if part in ("all", "body"):
        part_body(rig)
    if part in ("all", "moss"):
        part_moss(rig)
    if part in ("all", "neck"):
        part_neck(rig)
    if part in ("all", "head"):
        part_head(rig)
    if part in ("all", "legs"):
        part_legs(rig)
    if part in ("all", "tail"):
        part_tail(rig)

    return rig


def bone_tree_lines(rig: CreatureRig) -> list[str]:
    """生成骨骼层级树文本行，并标注每根骨骼挂载的几何块数。"""
    children_map: dict[str | None, list[str]] = {}
    for name, parent, _, _ in BONES_DEF:
        children_map.setdefault(parent, []).append(name)

    lines: list[str] = []

    def walk(name: str, prefix: str, is_last: bool, is_top: bool) -> None:
        b = rig.bones[name]
        c_cnt = len(b["children"])
        pivot = tuple(b["pivot"])
        tag = f" ({c_cnt} cubes)" if c_cnt > 0 else " (root pivot)"
        if is_top:
            lines.append(f"{name} {pivot}{tag}")
        else:
            branch = "`- " if is_last else "+- "
            lines.append(f"{prefix}{branch}{name} {pivot}{tag}")
        kids = children_map.get(name, [])
        for idx, kid in enumerate(kids):
            next_prefix = "" if is_top else prefix + ("   " if is_last else "|  ")
            walk(kid, next_prefix, idx == len(kids) - 1, False)

    walk("root", "", True, True)
    return lines


def moving_bones_count(rig: CreatureRig) -> int:
    """统计带几何的可独立运动关节骨骼数（除静止 root 外）。"""
    return sum(1 for name, _, _, _ in BONES_DEF if name != "root" and len(rig.bones[name]["children"]) > 0)


# ──────────────────────────────────────────────────────────────── 门禁检查
def assert_no_coplanar_faces(cubes: list[dict], tol: float = 1e-4) -> None:
    """严格核验几何体 0 项共面冲突。"""
    n = len(cubes)
    for i in range(n):
        c1 = cubes[i]
        f1, t1 = c1["from"], c1["to"]
        for j in range(i + 1, n):
            c2 = cubes[j]
            f2, t2 = c2["from"], c2["to"]

            # X 轴共面检查
            for x1, x2 in [(t1[0], f2[0]), (f1[0], t2[0]), (f1[0], f2[0]), (t1[0], t2[0])]:
                if abs(x1 - x2) < tol:
                    oy = min(t1[1], t2[1]) - max(f1[1], f2[1])
                    oz = min(t1[2], t2[2]) - max(f1[2], f2[2])
                    if oy > tol and oz > tol:
                        raise ValueError(f"X 面共面冲突: {c1['name']} 与 {c2['name']}, x={x1:.3f}, oy={oy:.3f}, oz={oz:.3f}")

            # Y 轴共面检查
            for y1, y2 in [(t1[1], f2[1]), (f1[1], t2[1]), (f1[1], f2[1]), (t1[1], t2[1])]:
                if abs(y1 - y2) < tol:
                    ox = min(t1[0], t2[0]) - max(f1[0], f2[0])
                    oz = min(t1[2], t2[2]) - max(f1[2], f2[2])
                    if ox > tol and oz > tol:
                        raise ValueError(f"Y 面共面冲突: {c1['name']} 与 {c2['name']}, y={y1:.3f}, ox={ox:.3f}, oz={oz:.3f}")

            # Z 轴共面检查
            for z1, z2 in [(t1[2], f2[2]), (f1[2], t2[2]), (f1[2], f2[2]), (t1[2], t2[2])]:
                if abs(z1 - z2) < tol:
                    ox = min(t1[0], t2[0]) - max(f1[0], f2[0])
                    oy = min(t1[1], t2[1]) - max(f1[1], f2[1])
                    if ox > tol and oy > tol:
                        raise ValueError(f"Z 面共面冲突: {c1['name']} 与 {c2['name']}, z={z1:.3f}, ox={ox:.3f}, oy={oy:.3f}")


def self_test() -> None:
    """运行门禁差分自证。"""
    print("运行 gen_can_bei_tuo.py 门禁差分自证...")
    rig = build_rig("all")
    elements = rig.elements

    # 1. 共面冲突检查
    assert_no_coplanar_faces(elements)
    print(f"  [OK] 正常几何体集合 ({len(elements)} 个立方体) 0 项共面冲突")

    # 2. 贴地检查
    min_y = min(e["from"][1] for e in elements)
    assert abs(min_y - 0.0) < 1e-4, f"贴地断言失败: 最低点 y={min_y:.3f} != 0.0"
    print(f"  [OK] 贴地断言通过 (最低点 y={min_y:.2f})")

    # 3. 部件数检查（比 Round 1 的 48 个至少翻三倍 >= 144）
    assert len(elements) >= 144, f"部件数不足: {len(elements)} < 144 (Round 1 为 48)"
    print(f"  [OK] 部件数达标: {len(elements)} 个 (Round 1 为 48 个，翻 {len(elements)/48:.1f} 倍)")

    # 4. 骨骼数检查（四足兽 >= 32 根）
    total_bones = len(BONES_DEF)
    moving_bones = moving_bones_count(rig)
    assert total_bones >= 32 and moving_bones >= 32, (
        f"骨骼数不足: 总骨骼={total_bones}, 可动带几何骨骼={moving_bones} (要求 >= 32)"
    )
    print(f"  [OK] 骨骼数达标: 共 {total_bones} 根骨骼 (其中带几何独立可动关节 {moving_bones} 根 >= 32)")

    # 5. 注入缺陷测试
    defect_elements = list(elements) + [{
        "name": "defect_coplanar_fail",
        "from": [-6.8, 6.15, -9.0],
        "to": [6.8, 9.42, 11.0],  # 完全与 torso_lower 重合
        "type": "cube",
        "uuid": str(uuid.uuid4()),
    }]
    caught = False
    try:
        assert_no_coplanar_faces(defect_elements)
    except ValueError as e:
        caught = True
        print(f"  [OK] 成功捕获注入共面缺陷: {e}")

    if not caught:
        raise RuntimeError("门禁失效: 注入共面冲突未被拦截!")

    print("✓ gen_can_bei_tuo.py 差分自证全绿")


# ──────────────────────────────────────────────────────────────── 渲染与审阅图
def render_part_previews(tex_b64: str) -> None:
    """逐件单独生成预览图与分部件 bbmodel，确认无误后再拼接。"""
    PARTS_DIR.mkdir(parents=True, exist_ok=True)
    bg_color = (119, 119, 119)
    font_title = load_font(16)
    font_sub = load_font(13)

    # 先导出 6 个单件 bbmodel (body, moss, neck, head, legs, tail)
    for part_name in ("body", "moss", "neck", "head", "legs", "tail"):
        prig = build_rig(part_name)
        mname = f"CanBeiTuo_{part_name}"
        mjson = json.dumps(prig.bbmodel(mname, tex_b64), ensure_ascii=False, indent=2)
        (CREATURE_DIR / f"{mname}.bbmodel").write_text(mjson, encoding="utf-8")
        (MODELS_DIR / f"{mname}.bbmodel").write_text(mjson, encoding="utf-8")

    print("正在渲染 5 大结构组件独立预览图...")
    for idx, (pkey, (pdesc, pfuncs)) in enumerate(PARTS_META.items(), start=1):
        rig = CreatureRig(tex_w=TEX_W, tex_h=TEX_H)
        setup_bones(rig)
        for fn in pfuncs:
            fn(rig)
        tmp_path = CREATURE_DIR / f"_tmp_part_{idx}_{pkey}.bbmodel"
        tmp_path.write_text(json.dumps(rig.bbmodel(f"CanBeiTuo_part_{pkey}", tex_b64)), encoding="utf-8")

        im_front, _ = render(tmp_path, yaw=0.0, pitch=0.0, size=300, bg=bg_color)
        im_iso, _ = render(tmp_path, yaw=-35.0, pitch=22.0, size=300, bg=bg_color)
        im_side, _ = render(tmp_path, yaw=90.0, pitch=0.0, size=300, bg=bg_color)
        tmp_path.unlink(missing_ok=True)

        canvas = Image.new("RGB", (300 * 3 + 20, 300 + 44), (32, 33, 36))
        draw = ImageDraw.Draw(canvas)
        canvas.paste(im_front, (5, 32))
        canvas.paste(im_iso, (310, 32))
        canvas.paste(im_side, (615, 32))

        draw.text((10, 6), f"部件 {idx}: {pdesc} ({len(rig.elements)} cubes)", fill=(235, 235, 235), font=font_title)
        draw.text((10, 308), "正视 FRONT (-Z)", fill=(200, 200, 200), font=font_sub)
        draw.text((315, 308), "3/4 等轴视 ISO", fill=(200, 200, 200), font=font_sub)
        draw.text((620, 308), "右侧视 SIDE_R (+X)", fill=(200, 200, 200), font=font_sub)

        out_png = PARTS_DIR / f"part_{idx}_{pkey}.png"
        canvas.save(out_png)
        print(f"  ✓ 部件 {idx} 预览图已生成: {out_png} ({len(rig.elements)} cubes)")


def render_6_views_sheet(
    bbmodel_path: Path,
    header_title: str,
) -> tuple[Image.Image, dict[str, Image.Image]]:
    """渲染六视角接触表（诚实命名视角，使用 cjk_font Noto Sans CJK 字体）。"""
    bg = (119, 119, 119)
    views = [
        ("FRONT",  0.0,   0.0,  "正视 FRONT (-Z 宽吻/前弯角/前肢)"),
        ("3/4",    -35.0, 22.0, "3/4 等轴视 ISO (驼峰叠藓/白花/立体角)"),
        ("SIDE_L", -90.0, 0.0,  "左视图 SIDE_L (-X 侧肋皮褶/垂挂藓须)"),
        ("SIDE_R", 90.0,  0.0,  "右视图 SIDE_R (+X 驼峰弧线/短粗四腿)"),
        ("TOP",    0.0,   89.9, "俯视图 TOP (+Y 三层噬灵藓/宽圆桶身)"),
        ("BACK",   180.0, 0.0,  "后视图 BACK (+Z 圆臀/双瓣偶蹄/藓穗尾)"),
    ]

    rendered_images: dict[str, Image.Image] = {}
    w_cell, h_cell = 400, 400
    for name, yaw, pitch, _ in views:
        im, _ = render(bbmodel_path, yaw=yaw, pitch=pitch, size=w_cell, bg=bg)
        rendered_images[name] = im

    sheet = Image.new("RGB", (w_cell * 3 + 20, h_cell * 2 + 56), (32, 33, 36))
    draw = ImageDraw.Draw(sheet)
    font_header = load_font(18)
    font_label = load_font(13)

    draw.text((10, 6), header_title, fill=(235, 235, 235), font=font_header)

    coords = [
        (5, 34),
        (w_cell + 10, 34),
        (w_cell * 2 + 15, 34),
        (5, h_cell + 44),
        (w_cell + 10, h_cell + 44),
        (w_cell * 2 + 15, h_cell + 44),
    ]
    for idx, (name, _, _, desc) in enumerate(views):
        px, py = coords[idx]
        sheet.paste(rendered_images[name], (px, py))
        draw.rectangle([px, py, px + 290, py + 24], fill=(22, 23, 26))
        draw.text((px + 6, py + 3), desc, fill=(235, 235, 235), font=font_label)

    return sheet, rendered_images


def render_bone_tree_panel(rig: CreatureRig, target_height: int) -> Image.Image:
    """绘制骨骼层级树面板（标明骨骼总数、可动关节数与完整层级树）。"""
    lines = bone_tree_lines(rig)
    total_b = len(BONES_DEF)
    moving_b = moving_bones_count(rig)
    total_c = len(rig.elements)

    font_h = load_font(16)
    font_sub = load_font(13)
    font_tree = load_font(12)

    line_step = 18
    panel_w = 430
    needed_h = 90 + len(lines) * line_step + 20
    panel_h = max(target_height, needed_h)

    panel = Image.new("RGB", (panel_w, panel_h), (32, 33, 37))
    draw = ImageDraw.Draw(panel)

    draw.rectangle([0, 0, panel_w - 1, panel_h - 1], outline=(70, 75, 85), width=2)
    draw.text((14, 10), "骨骼层级树 (BONE HIERARCHY)", fill=(180, 235, 200), font=font_h)
    draw.text(
        (14, 34),
        f"总骨骼: {total_b} 根 (可动关节 {moving_b} 根 >= 32, +1 root)",
        fill=(240, 210, 150),
        font=font_sub,
    )
    draw.text(
        (14, 54),
        f"几何部件: {total_c} cubes (Round 1 为 48, 翻 {total_c/48:.1f} 倍)",
        fill=(190, 210, 235),
        font=font_sub,
    )
    draw.line([(12, 76), (panel_w - 12, 76)], fill=(65, 70, 80), width=1)

    y = 82
    for ln in lines:
        color = (230, 230, 235) if "root pivot" not in ln else (175, 180, 190)
        draw.text((14, y), ln, fill=color, font=font_tree)
        y += line_step

    return panel


def make_check_sheet_round2(
    ref_path: Path,
    exploded_path: Path,
    sheet_prev: Image.Image,
    sheet_now: Image.Image,
    tree_panel: Image.Image,
    total_c: int,
    total_b: int,
    moving_b: int,
) -> Image.Image:
    """生成四栏并排总审阅图 check.png：参考/爆炸图 + PREV(Round 1 同取景) + NOW(Round 2 六视角) + 骨骼层级树。"""
    ref_im = Image.open(ref_path).convert("RGB")
    exp_im = Image.open(exploded_path).convert("RGB") if exploded_path.exists() else ref_im

    row_h = max(sheet_now.height, tree_panel.height)
    gap = 12
    sub_h = (row_h - gap) // 2
    sub_w = int(ref_im.width * sub_h / ref_im.height)
    ref_scaled = ref_im.resize((sub_w, sub_h), Image.Resampling.LANCZOS)
    exp_scaled = exp_im.resize((sub_w, sub_h), Image.Resampling.LANCZOS)

    total_w = sub_w + sheet_prev.width + sheet_now.width + tree_panel.width + 50
    canvas = Image.new("RGB", (total_w, row_h + 48), (25, 26, 29))
    draw = ImageDraw.Draw(canvas)
    font_header = load_font(18)

    # 1. 左栏：概念参考图 + 爆炸分解图
    x_cur = 10
    draw.text((x_cur + 4, 8), "参考图 REFERENCES (上: 整兽参考 | 下: 爆炸分解)", fill=(215, 205, 180), font=font_header)
    canvas.paste(ref_scaled, (x_cur, 38))
    canvas.paste(exp_scaled, (x_cur, 38 + sub_h + gap))

    # 2. 中左栏：PREV (Round 1 同取景基线)
    x_cur += sub_w + 10
    draw.text((x_cur + 4, 8), "PREV: Round 1 同取景基线 (48 cubes, 14 骨骼 - 细节不足/平板藓)", fill=(230, 160, 140), font=font_header)
    canvas.paste(sheet_prev, (x_cur, 38))

    # 3. 中右栏：NOW (Round 2 精细重构)
    x_cur += sheet_prev.width + 10
    draw.text(
        (x_cur + 4, 8),
        f"NOW: Round 2 返工 ({total_c} cubes, {total_b} 骨骼/{moving_b} 可动关节, 0.5px 驼峰/弯角/垂藓/偶蹄)",
        fill=(140, 230, 180),
        font=font_header,
    )
    canvas.paste(sheet_now, (x_cur, 38))

    # 4. 右栏：骨骼层级树
    x_cur += sheet_now.width + 10
    draw.text((x_cur + 4, 8), f"骨骼树 BONE TREE ({total_b} 根骨骼 >= 32)", fill=(180, 220, 240), font=font_header)
    canvas.paste(tree_panel, (x_cur, 38))

    return canvas


# ──────────────────────────────────────────────────────────────── 主函数入口
def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--part", choices=["all", "body", "moss", "neck", "head", "legs", "tail"], default="all", help="分部件生成预览")
    parser.add_argument("--self-test", action="store_true", help="运行门禁自证")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    # 1. 运行自测
    self_test()

    # 2. 构建贴图与模型
    CREATURE_DIR.mkdir(parents=True, exist_ok=True)
    MODELS_DIR.mkdir(parents=True, exist_ok=True)
    REVIEW_DIR.mkdir(parents=True, exist_ok=True)

    tex_img = generate_palette_texture()
    buf = io.BytesIO()
    tex_img.save(buf, format="PNG")
    tex_b64 = base64.b64encode(buf.getvalue()).decode("ascii")

    if args.part != "all":
        rig = build_rig(args.part)
        model_name = f"CanBeiTuo_{args.part}"
        model_json = json.dumps(rig.bbmodel(model_name, tex_b64), ensure_ascii=False, indent=2)
        (CREATURE_DIR / f"{model_name}.bbmodel").write_text(model_json, encoding="utf-8")
        (MODELS_DIR / f"{model_name}.bbmodel").write_text(model_json, encoding="utf-8")
        print(f"✓ 单部件模型已生成: {CREATURE_DIR / f'{model_name}.bbmodel'} ({len(rig.elements)} cubes)")
        return

    # 3. 逐部件渲染独立预览图并导出分部件 bbmodel
    render_part_previews(tex_b64)

    # 4. 生成完整拼装模型
    rig = build_rig("all")
    model_name = "CanBeiTuo"
    model_dict = rig.bbmodel(model_name, tex_b64)

    out_local = CREATURE_DIR / f"{model_name}.bbmodel"
    out_models = MODELS_DIR / f"{model_name}.bbmodel"

    model_json = json.dumps(model_dict, ensure_ascii=False, indent=2)
    out_local.write_text(model_json, encoding="utf-8")
    out_models.write_text(model_json, encoding="utf-8")
    total_c = len(rig.elements)
    total_b = len(BONES_DEF)
    moving_b = moving_bones_count(rig)
    print(f"✓ 完整模型已生成: {out_local} ({total_c} cubes, {total_b} 骨骼/{moving_b} 可动关节)")

    # 5. 渲染 NOW 与 PREV 六视角接触表（均使用 cjk_font 正文字体）
    print("正在渲染 Round 2 NOW 与 Round 1 PREV 六视角接触表...")
    sheet_now, rendered_views = render_6_views_sheet(
        out_local,
        f"can_bei_tuo (残背驮兽) Round 2 NOW | 骨骼: {total_b} ({moving_b} 可动关节) | 部件: {total_c} cubes (0.5px 精度)",
    )
    contact_path = REVIEW_DIR / "contact_sheet.png"
    sheet_now.save(contact_path)
    print(f"✓ 六视角接触表已保存: {contact_path}")

    views_dir = REVIEW_DIR / "views"
    views_dir.mkdir(parents=True, exist_ok=True)
    for vname, vim in rendered_views.items():
        safe_name = vname.lower().replace("/", "_")
        vim.save(views_dir / f"{safe_name}.png")

    round1_bbmodel = REVIEW_DIR / "CanBeiTuo_round1.bbmodel"
    if round1_bbmodel.exists():
        sheet_prev, _ = render_6_views_sheet(
            round1_bbmodel,
            "can_bei_tuo (残背驮兽) Round 1 PREV 基线 | 骨骼: 14 | 部件: 48 cubes",
        )
        sheet_prev.save(REVIEW_DIR / "contact_sheet_round1.png")
    else:
        sheet_prev = sheet_now

    # 6. 生成骨骼层级树面板
    tree_panel = render_bone_tree_panel(rig, sheet_now.height)
    tree_panel.save(REVIEW_DIR / "bones_tree.png")
    print(f"✓ 骨骼层级树面板已保存: {REVIEW_DIR / 'bones_tree.png'}")

    # 7. 生成并排对照卡 check.png
    ref_img_path = REVIEW_DIR / "can_bei_tuo_ref.png"
    exp_img_path = REVIEW_DIR / "can_bei_tuo_exploded.png"
    check_sheet = make_check_sheet_round2(
        ref_img_path,
        exp_img_path,
        sheet_prev,
        sheet_now,
        tree_panel,
        total_c,
        total_b,
        moving_b,
    )
    check_path = REVIEW_DIR / "check.png"
    check_sheet.save(check_path)
    print(f"✓ 并排对照卡 check.png 已保存: {check_path} (尺寸: {check_sheet.size})")


if __name__ == "__main__":
    main()
