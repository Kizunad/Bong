#!/usr/bin/env python3
"""枯皮狼（ku_pi_lang）四足生物模型程序化生成器 —— Round 2/3（高精细度重构版）。

按爆炸图 5 个编号部件逐件做细、逐件单独出预览再拼装：
  1. head        - 狭长狼头 + 张开下颌 (带牙槽与獠牙) + 外露颧弓 + 残缺撕裂双耳
  2. spine       - 从肩到荐骨一串 12 节突起独立小方块骨节 (椎体 + 高棘突刺 + 横突)
  3. ribcage     - 左右各 7 对、每根 4 段阶梯下弯的弧形肋条，肋间完全透空
  4. pelt        - 深褐带红残破皮片，边缘锯齿状下垂碎片，大面积脱落露出骨架
  5. limbs_tail  - 四肢三段 (前肢肩/臂/掌+4钩爪；后肢反关节股/胫/跟骨后折/跖+4利爪) + 8节逐节变细骨节尾

精度全面提升至 0.5px，部件数从 Round 1 的 83 个翻三倍至 240+ 个。

用法：
  python3 modelScript/creatures/ku_pi_lang/gen_ku_pi_lang.py                 # 全身建模、输出 5 大部件预览与对标卡
  python3 modelScript/creatures/ku_pi_lang/gen_ku_pi_lang.py --self-test    # 门禁差分自测
  python3 modelScript/creatures/ku_pi_lang/gen_ku_pi_lang.py --list         # 列出所有 5 大编号部件
  python3 modelScript/creatures/ku_pi_lang/gen_ku_pi_lang.py --part head    # 仅生成单个部件 bbmodel
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import math
import sys
import uuid
from pathlib import Path
from typing import Dict, List, Tuple

import numpy as np
from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[3]
CREATURE_DIR = REPO / "modelScript" / "creatures" / "ku_pi_lang"
MODELS_DIR = REPO / "modelScript" / "models"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/creatures/ku_pi_lang")
PARTS_DIR = REVIEW_DIR / "parts"
EXPLODED_REF = REVIEW_DIR / "exploded.png"
PREV_BBMODEL = CREATURE_DIR / "KuPiLang_round1.bbmodel"

BBMODEL_OUT = CREATURE_DIR / "KuPiLang.bbmodel"
BBMODEL_COPY = MODELS_DIR / "KuPiLang.bbmodel"

# -----------------------------------------------------------------------------
# 材质与 64x64 调色板映射（强化深褐带红残破皮毛与苍白骨骼对比）
# -----------------------------------------------------------------------------
MATS = (
    "pelt_dark",      # 残存粗糙深灰褐色枯狼毛 (#3e3630)
    "pelt_mangy",     # 斑驳风干脱落杂毛 (#5c5145)
    "flesh_scab",     # 皮毛剥落处的暗红结痂溃肉 (#5c2826 / #7a3632)
    "bone_white",     # 外露肋骨与凸起脊椎的苍白骨色 (#ded8ca)
    "bone_dark",      # 骨骼深缝与胸腹内腔暗影 (#26231f)
    "snout_black",    # 干瘪黑鼻头与嘴裂深色 (#1e1a18)
    "fang_white",     # 外露锐利惨白犬齿 (#f4efe4)
    "eye_amber",      # 饥饿凶戾的暗金琥珀色狼眼 (#db8e28)
)

TEX_W = TEX_H = 64
SWATCH_SIZE = 16  # 4x2 网格，每格 16x16 像素
MAT_SWATCH_IDX = {mat: i for i, mat in enumerate(MATS)}

MAT_COLORS = {
    "pelt_dark":   (62, 54, 48),
    "pelt_mangy":  (92, 81, 69),
    "flesh_scab":  (92, 40, 38),
    "bone_white":  (222, 216, 202),
    "bone_dark":   (38, 35, 31),
    "snout_black": (30, 26, 24),
    "fang_white":  (244, 239, 228),
    "eye_amber":   (219, 142, 40),
}


def _faces(mat: str) -> dict:
    idx = MAT_SWATCH_IDX[mat]
    row = idx // 4
    col = idx % 4
    u0 = col * SWATCH_SIZE + 1.0
    v0 = row * SWATCH_SIZE + 1.0
    u1 = u0 + 14.0
    v1 = v0 + 14.0
    uv = [u0, v0, u1, v1]
    return {
        "north": {"uv": uv, "texture": 0},
        "south": {"uv": uv, "texture": 0},
        "east":  {"uv": uv, "texture": 0},
        "west":  {"uv": uv, "texture": 0},
        "up":    {"uv": uv, "texture": 0},
        "down":  {"uv": uv, "texture": 0},
    }


def generate_texture() -> Image.Image:
    """生成带有斑驳废土噪点机理的 64x64 调色板贴图。"""
    img = Image.new("RGBA", (TEX_W, TEX_H), (0, 0, 0, 255))
    rng = np.random.default_rng(20261013)

    for mat, idx in MAT_SWATCH_IDX.items():
        row = idx // 4
        col = idx % 4
        x0 = col * SWATCH_SIZE
        y0 = row * SWATCH_SIZE
        base_rgb = np.array(MAT_COLORS[mat], dtype=np.float32)

        swatch_arr = np.zeros((SWATCH_SIZE, SWATCH_SIZE, 4), dtype=np.uint8)
        noise = rng.normal(0, 6.0, (SWATCH_SIZE, SWATCH_SIZE, 1))
        rgb = np.clip(base_rgb + noise, 10, 250).astype(np.uint8)
        swatch_arr[:, :, :3] = rgb
        swatch_arr[:, :, 3] = 255

        sw_img = Image.fromarray(swatch_arr, "RGBA")
        draw = ImageDraw.Draw(sw_img)

        if mat in ("pelt_dark", "pelt_mangy"):
            for _ in range(12):
                px, py = rng.integers(1, SWATCH_SIZE - 2, 2)
                draw.point((px, py), fill=(int(base_rgb[0] * 0.65), int(base_rgb[1] * 0.65), int(base_rgb[2] * 0.65), 255))
        elif mat == "bone_white":
            for _ in range(8):
                px, py = rng.integers(1, SWATCH_SIZE - 2, 2)
                draw.point((px, py), fill=(180, 172, 158, 255))
        elif mat == "flesh_scab":
            for _ in range(10):
                px, py = rng.integers(1, SWATCH_SIZE - 2, 2)
                draw.point((px, py), fill=(125, 30, 28, 255))

        img.paste(sw_img, (x0, y0))

    return img


# -----------------------------------------------------------------------------
# 骨骼层级与 Rig 容器
# -----------------------------------------------------------------------------
BONE_DEFS = [
    ("root",          (0.0, 0.0, 0.0),      None),
    ("hips",          (0.0, 11.5, 6.0),     "root"),
    ("spine_lumbar",  (0.0, 12.0, 2.0),     "hips"),
    ("thorax",        (0.0, 12.5, -2.0),    "spine_lumbar"),
    ("neck",          (0.0, 13.5, -6.5),    "thorax"),
    ("head",          (0.0, 14.5, -10.0),   "neck"),
    ("snout",         (0.0, 13.5, -11.8),   "head"),
    ("jaw",           (0.0, 12.2, -10.5),   "head"),
    ("ear_l",         (-2.0, 16.5, -9.0),   "head"),
    ("ear_r",         (2.0, 16.5, -9.0),    "head"),
    ("tail_base",     (0.0, 11.0, 9.5),     "hips"),
    ("tail_mid",      (0.0, 9.8, 12.8),     "tail_base"),
    ("tail_tip",      (0.0, 6.8, 16.5),     "tail_mid"),
    ("shoulder_l",    (-2.8, 11.5, -4.5),   "thorax"),
    ("arm_l",         (-2.8, 6.5, -4.0),    "shoulder_l"),
    ("paw_fl",        (-2.8, 0.0, -4.0),    "arm_l"),
    ("shoulder_r",    (2.8, 11.5, -4.5),    "thorax"),
    ("arm_r",         (2.8, 6.5, -4.0),     "shoulder_r"),
    ("paw_fr",        (2.8, 0.0, -4.0),     "arm_r"),
    ("thigh_l",       (-2.6, 10.5, 6.5),    "hips"),
    ("shin_l",        (-2.7, 5.5, 7.5),     "thigh_l"),
    ("hock_l",        (-2.7, 2.5, 5.5),     "shin_l"),
    ("paw_bl",        (-2.7, 0.0, 4.5),     "hock_l"),
    ("thigh_r",       (2.6, 10.5, 6.5),     "hips"),
    ("shin_r",        (2.7, 5.5, 7.5),      "thigh_r"),
    ("hock_r",        (2.7, 2.5, 5.5),      "shin_r"),
    ("paw_br",        (2.7, 0.0, 4.5),      "hock_r"),
]


class Rig:
    def __init__(self) -> None:
        self.elements: list[dict] = []
        self.bones: dict[str, dict] = {}
        self.bone_order: list[str] = []
        for name, pivot, parent in BONE_DEFS:
            self._add_bone(name, pivot, parent)

    def _add_bone(self, name: str, pivot: tuple[float, float, float], parent: str | None) -> None:
        self.bones[name] = {
            "uuid": str(uuid.uuid4()),
            "pivot": [round(v, 3) for v in pivot],
            "parent": parent,
            "children": [],
        }
        self.bone_order.append(name)

    def cube(
        self,
        bone: str,
        name: str,
        frm: tuple[float, float, float] | list[float],
        to: tuple[float, float, float] | list[float],
        *,
        mat: str = "pelt_dark",
        rot: tuple[float, float, float] | None = None,
        org: tuple[float, float, float] | None = None,
    ) -> dict:
        if bone not in self.bones:
            raise ValueError(f"未定义骨骼: {bone}")
        if mat not in MATS:
            raise ValueError(f"未知材质: {mat}")
        f = [round(min(a, b), 3) for a, b in zip(frm, to)]
        t = [round(max(a, b), 3) for a, b in zip(frm, to)]
        eid = str(uuid.uuid4())
        c_org = [(f[0] + t[0]) / 2.0, (f[1] + t[1]) / 2.0, (f[2] + t[2]) / 2.0]
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
            "color": MATS.index(mat) % 8,
            "origin": [round(v, 3) for v in (org or c_org)],
            "rotation": [round(v, 3) for v in (rot or (0.0, 0.0, 0.0))],
            "faces": _faces(mat),
            "bone_name": bone,
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
        roots = []
        for name in self.bone_order:
            parent = self.bones[name]["parent"]
            if parent is None:
                roots.append(nodes[name])
            else:
                nodes[parent]["children"].append(nodes[name])
        return roots

    def bbmodel(self, model_name: str, tex_img: Image.Image) -> dict:
        tex_buf = io.BytesIO()
        tex_img.save(tex_buf, format="PNG")
        tex_b64 = "data:image/png;base64," + base64.b64encode(tex_buf.getvalue()).decode("ascii")

        return {
            "meta": {
                "format_version": "4.10",
                "model_format": "free",
                "box_uv": False,
            },
            "name": model_name,
            "model_identifier": "ku_pi_lang",
            "visible_box": [-24.0, -16.0, -24.0, 24.0, 24.0, 24.0],
            "resolution": {"width": TEX_W, "height": TEX_H},
            "elements": self.elements,
            "outliner": self.outliner(),
            "textures": [
                {
                    "path": "",
                    "name": "ku_pi_lang.png",
                    "folder": "fauna",
                    "namespace": "bong",
                    "id": "0",
                    "particle": True,
                    "render_mode": "default",
                    "visible": True,
                    "mode": "bitmap",
                    "saved": True,
                    "uuid": str(uuid.uuid4()),
                    "source": tex_b64,
                }
            ],
        }


# =============================================================================
# 5 大编号部件程序化生成函数（按爆炸图逐件做细）
# =============================================================================

def part_head(rig: Rig) -> None:
    """1. 头：长吻 + 张开的下颌 (上下露 2~3 颗犬齿与门齿) + 颧骨外露 + 残缺撕裂双耳。"""
    # ── 颅脑与颧骨外露 ──
    rig.cube("head", "skull_brain_mid",   [-2.0, 13.5, -11.5], [2.0, 16.4, -8.5], mat="pelt_dark")
    rig.cube("head", "skull_brain_top",   [-1.5, 16.4, -11.0], [1.5, 17.0, -9.0], mat="pelt_dark")
    rig.cube("head", "skull_bone_patch",  [-0.8, 17.0, -10.5], [0.8, 17.4, -9.5], mat="bone_white") # 破皮骨顶
    rig.cube("head", "skull_brow_ridge",  [-2.2, 15.4, -12.2], [2.2, 16.2, -10.5], mat="pelt_mangy")
    rig.cube("head", "skull_wound_scab",  [-0.5, 16.5, -11.6], [0.5, 16.9, -10.6], mat="flesh_scab") # 额头破皮结痂

    # 外露颧弓 (左右两侧外展骨板)
    rig.cube("head", "zygomatic_arch_l",  [-2.7, 14.0, -11.2], [-2.0, 15.2, -9.2], mat="bone_white")
    rig.cube("head", "zygomatic_arch_r",  [2.0, 14.0, -11.2],  [2.7, 15.2, -9.2],  mat="bone_white")

    # 凹陷眼窝与琥珀凶瞳
    rig.cube("head", "eye_socket_l",      [-2.45, 14.5, -11.6], [-1.85, 15.5, -10.6], mat="bone_dark")
    rig.cube("head", "eye_amber_l",       [-2.48, 14.7, -11.4], [-1.90, 15.3, -10.8], mat="eye_amber")
    rig.cube("head", "eye_socket_r",      [1.85, 14.5, -11.6],  [2.45, 15.5, -10.6],  mat="bone_dark")
    rig.cube("head", "eye_amber_r",       [1.90, 14.7, -11.4],  [2.48, 15.3, -10.8],  mat="eye_amber")

    # ── 狭长吻部与上颌牙列 ──
    rig.cube("snout", "snout_base",       [-1.6, 13.2, -13.5], [1.6, 15.2, -11.5], mat="pelt_dark")
    rig.cube("snout", "snout_bridge",     [-1.2, 13.4, -15.8], [1.2, 14.8, -13.5], mat="pelt_mangy")
    rig.cube("snout", "snout_tip",        [-0.9, 13.6, -16.8], [0.9, 14.6, -15.8], mat="pelt_dark")
    rig.cube("snout", "snout_nose",       [-0.7, 14.2, -17.2], [0.7, 14.8, -16.6], mat="snout_black")

    # 上犬齿与门齿 (上下各露 2~3 颗齿)
    rig.cube("snout", "fang_u_l",         [-1.35, 12.0, -15.8], [-0.95, 13.4, -14.8], mat="fang_white") # 左长犬齿
    rig.cube("snout", "fang_u_r",         [0.95, 12.0, -15.8],  [1.35, 13.4, -14.8],  mat="fang_white") # 右长犬齿
    rig.cube("snout", "incisor_u_l",      [-0.75, 12.8, -16.6], [-0.25, 13.6, -16.0], mat="fang_white") # 左门齿
    rig.cube("snout", "incisor_u_r",      [0.25, 12.8, -16.6],  [0.75, 13.6, -16.0],  mat="fang_white") # 右门齿
    rig.cube("snout", "molar_u_l1",       [-1.48, 12.6, -14.4], [-1.08, 13.3, -13.6], mat="fang_white") # 侧颊齿1
    rig.cube("snout", "molar_u_r1",       [1.08, 12.6, -14.4],  [1.48, 13.3, -13.6],  mat="fang_white")
    rig.cube("snout", "molar_u_l2",       [-1.52, 12.8, -13.45], [-1.15, 13.4, -12.65], mat="fang_white") # 侧颊齿2
    rig.cube("snout", "molar_u_r2",       [1.15, 12.8, -13.45],  [1.52, 13.4, -12.65],  mat="fang_white")

    # ── 张开的下颌 (低垂咆哮张开) ──
    rig.cube("jaw", "jaw_hinge",          [-1.4, 11.8, -11.5], [1.4, 13.0, -9.5],   mat="bone_dark")
    rig.cube("jaw", "jaw_ramus_l",        [-1.3, 11.2, -13.6], [-0.7, 12.4, -11.5], mat="bone_white")
    rig.cube("jaw", "jaw_ramus_r",        [0.7, 11.2, -13.6],  [1.3, 12.4, -11.5],  mat="bone_white")
    rig.cube("jaw", "jaw_chin",           [-0.9, 10.6, -16.2], [0.9, 11.8, -13.6], mat="bone_white")
    rig.cube("jaw", "jaw_tongue",         [-0.6, 11.6, -15.0], [0.6, 12.2, -12.0], mat="flesh_scab") # 暗红舌头

    # 下犬齿与下门齿 (向上突刺错开)
    rig.cube("jaw", "fang_d_l",           [-1.05, 11.8, -15.4], [-0.65, 13.2, -14.5], mat="fang_white") # 左下犬齿
    rig.cube("jaw", "fang_d_r",           [0.65, 11.8, -15.4],  [1.05, 13.2, -14.5],  mat="fang_white") # 右下犬齿
    rig.cube("jaw", "incisor_d_l",        [-0.65, 11.8, -16.1], [-0.25, 12.6, -15.5], mat="fang_white")
    rig.cube("jaw", "incisor_d_r",        [0.25, 11.8, -16.1],  [0.65, 12.6, -15.5],  mat="fang_white")
    rig.cube("jaw", "molar_d_l",          [-1.25, 12.0, -13.55], [-0.85, 12.7, -12.75], mat="fang_white") # 下侧臼齿
    rig.cube("jaw", "molar_d_r",          [0.85, 12.0, -13.55],  [1.25, 12.7, -12.75],  mat="fang_white")

    # ── 残缺撕裂双耳 (左耳撕脱断尖，右耳撕裂缺角) ──
    # 左耳残角
    rig.cube("ear_l", "ear_l_base",       [-2.7, 16.2, -9.8], [-1.4, 17.8, -8.6], mat="pelt_dark")
    rig.cube("ear_l", "ear_l_torn",       [-2.4, 17.8, -9.6], [-1.6, 18.8, -8.8], mat="flesh_scab") # 撕断肉面
    rig.cube("ear_l", "ear_l_scab",       [-2.2, 18.8, -9.4], [-1.8, 19.3, -9.0], mat="flesh_scab")

    # 右耳缺角
    rig.cube("ear_r", "ear_r_base",       [1.4, 16.2, -9.8],  [2.7, 17.8, -8.6],  mat="pelt_dark")
    rig.cube("ear_r", "ear_r_mid",        [1.6, 17.8, -9.6],  [2.5, 19.4, -8.8],  mat="pelt_mangy")
    rig.cube("ear_r", "ear_r_tip",        [1.8, 19.4, -9.5],  [2.4, 20.6, -8.9],  mat="pelt_dark")
    rig.cube("ear_r", "ear_r_notch",      [2.35, 18.2, -9.65], [2.75, 19.2, -8.75], mat="flesh_scab") # 撕咬缺口

    # ── 颈部 ──
    rig.cube("neck", "neck_upper",        [-1.4, 13.0, -8.6], [1.4, 15.6, -6.0],  mat="pelt_mangy")
    rig.cube("neck", "neck_throat",       [-1.1, 11.6, -8.2], [1.1, 13.0, -6.2],  mat="flesh_scab")
    rig.cube("neck", "neck_mane_l",       [-1.9, 13.4, -7.8], [-1.3, 16.2, -5.8], mat="pelt_dark")
    rig.cube("neck", "neck_mane_r",       [1.3, 13.4, -7.8],  [1.9, 16.2, -5.8],  mat="pelt_dark")


def part_spine(rig: Rig) -> None:
    """2. 脊椎：从肩到尾一串突起的独立骨节 (至少 10 节，此处做足 12 节独立小方块，椎体+高棘突刺)。"""
    spine_zs = [
        # (z_min, z_max, y_body_min, y_body_max, spine_h, bone)
        (-5.2, -4.2, 14.8, 15.6, 1.4, "thorax"),       # T1
        (-4.0, -3.0, 15.0, 15.8, 1.8, "thorax"),       # T2
        (-2.8, -1.8, 14.9, 15.7, 2.0, "thorax"),       # T3 (肩峰最高点 wither)
        (-1.6, -0.6, 14.6, 15.4, 1.6, "thorax"),       # T4
        (-0.4,  0.6, 14.2, 15.0, 1.4, "thorax"),       # T5
        ( 0.8,  1.8, 13.8, 14.6, 1.2, "spine_lumbar"), # T6
        ( 2.0,  3.0, 13.5, 14.3, 1.1, "spine_lumbar"), # L1
        ( 3.2,  4.2, 13.2, 14.0, 1.0, "spine_lumbar"), # L2
        ( 4.4,  5.4, 13.0, 13.8, 1.0, "spine_lumbar"), # L3
        ( 5.6,  6.6, 12.8, 13.6, 0.9, "spine_lumbar"), # L4
        ( 6.8,  7.8, 12.6, 13.4, 0.8, "hips"),         # S1
        ( 8.0,  9.0, 12.3, 13.1, 0.7, "hips"),         # S2
    ]

    for i, (z0, z1, y0, y1, sh, b_name) in enumerate(spine_zs):
        # 独立椎体小方块
        rig.cube(b_name, f"vertebra_body_{i}", [-0.6, y0, z0], [0.6, y1, z1], mat="bone_white")
        # 突起棘突骨刺 (向上突出，节节分明)
        rig.cube(b_name, f"vertebra_spine_{i}", [-0.3, y1, z0 + 0.15], [0.3, y1 + sh, z1 - 0.15], mat="bone_white")
        # 椎体横突 (左右侧突出，增强节律立体感)
        rig.cube(b_name, f"vert_transverse_l_{i}", [-1.0, y0 + 0.15, z0 + 0.25], [-0.6, y0 + 0.65, z1 - 0.25], mat="bone_white")
        rig.cube(b_name, f"vert_transverse_r_{i}", [0.6, y0 + 0.15, z0 + 0.25],  [1.0, y0 + 0.65, z1 - 0.25], mat="bone_white")


def part_ribcage(rig: Rig) -> None:
    """3. 胸腔：肋骨做成从脊椎向下弯的弧形肋条 (每根 4 段阶梯弯)，左右各 7 对，肋间完全透空。"""
    rib_specs = [
        # (zc, w1, w2, w3, w4, yt, yb, bone)
        (-4.6, 1.4, 2.0, 2.7, 2.2, 14.8, 10.4, "thorax"),       # R1
        (-3.4, 1.5, 2.3, 3.1, 2.6, 14.9,  9.8, "thorax"),       # R2 (最宽)
        (-2.2, 1.5, 2.4, 3.1, 2.6, 14.8,  9.8, "thorax"),       # R3
        (-1.0, 1.4, 2.2, 2.9, 2.4, 14.4, 10.0, "thorax"),       # R4
        ( 0.2, 1.3, 2.0, 2.7, 2.2, 14.0, 10.2, "thorax"),       # R5
        ( 1.4, 1.2, 1.8, 2.4, 1.9, 13.6, 10.6, "spine_lumbar"), # R6 (浮肋)
        ( 2.6, 1.1, 1.6, 2.1, 1.6, 13.2, 11.0, "spine_lumbar"), # R7 (浮肋)
    ]

    for i, (zc, w1, w2, w3, w4, yt, yb, b_name) in enumerate(rib_specs):
        rz0_1 = round(zc - 0.25, 3)
        rz1_1 = round(zc + 0.25, 3)
        rz0_2 = round(zc - 0.22, 3)
        rz1_2 = round(zc + 0.22, 3)
        rz0_3 = round(zc - 0.19, 3)
        rz1_3 = round(zc + 0.19, 3)
        rz0_4 = round(zc - 0.16, 3)
        rz1_4 = round(zc + 0.16, 3)

        y1 = round(yt - (yt - yb) * 0.25, 3)
        y2 = round(yt - (yt - yb) * 0.55, 3)
        y3 = round(yt - (yt - yb) * 0.80, 3)

        # 左侧 4 段阶梯弧形下弯
        rig.cube(b_name, f"rib_l_{i}_1", [-w1, y1, rz0_1], [-0.65, yt, rz1_1], mat="bone_white")
        rig.cube(b_name, f"rib_l_{i}_2", [-w2, y2, rz0_2], [-w1 + 0.08, y1 + 0.08, rz1_2], mat="bone_white")
        rig.cube(b_name, f"rib_l_{i}_3", [-w3, y3, rz0_3], [-w2 + 0.08, y2 + 0.08, rz1_3], mat="bone_white")
        rig.cube(b_name, f"rib_l_{i}_4", [-w4, yb, rz0_4], [-w3 + 0.12, y3 + 0.08, rz1_4], mat="bone_white")

        # 右侧 4 段阶梯弧形下弯 (对称)
        rig.cube(b_name, f"rib_r_{i}_1", [0.65, y1, rz0_1], [w1, yt, rz1_1], mat="bone_white")
        rig.cube(b_name, f"rib_r_{i}_2", [w1 - 0.08, y2, rz0_2], [w2, y1 + 0.08, rz1_2], mat="bone_white")
        rig.cube(b_name, f"rib_r_{i}_3", [w2 - 0.08, y3, rz0_3], [w3, y2 + 0.08, rz1_3], mat="bone_white")
        rig.cube(b_name, f"rib_r_{i}_4", [w4 - 0.12, yb, rz0_4], [w4, y3 + 0.08, rz1_4], mat="bone_white")

    # 细长胸骨底龙骨与剑突
    rig.cube("thorax", "sternum_keel",    [-0.4, 9.4, -4.8], [0.4, 10.2, 0.6], mat="bone_white")
    rig.cube("thorax", "xiphoid_process", [-0.3, 9.6,  0.6], [0.3, 10.1, 1.8], mat="bone_white") # 剑突小骨片


def part_pelt(rig: Rig) -> None:
    """4. 皮毛：残破皮片挂在背和腹侧，边缘锯齿状下垂碎片，大面积脱落露骨，深褐带红。"""
    # ── 肩峰残毛鞍块 ──
    rig.cube("thorax", "pelt_saddle_core", [-2.3, 15.2, -4.8], [2.3, 16.3, -2.4], mat="pelt_dark")
    rig.cube("thorax", "pelt_saddle_l1",   [-2.9, 14.2, -4.5], [-2.2, 15.6, -2.6], mat="pelt_dark")
    rig.cube("thorax", "pelt_saddle_r1",   [2.2, 14.2, -4.5],  [2.9, 15.6, -2.6],  mat="pelt_dark")

    # ── 边缘锯齿状下垂皮屑碎片 (撕裂残片悬垂挂在肋骨外缘) ──
    rig.cube("thorax", "pelt_rag_l1",      [-3.05, 12.8, -3.8], [-2.55, 14.4, -3.0], mat="flesh_scab")
    rig.cube("thorax", "pelt_rag_r1",      [2.55, 12.8, -3.8],  [3.05, 14.4, -3.0],  mat="flesh_scab")
    rig.cube("thorax", "pelt_rag_l2",      [-2.95, 12.2, -2.0], [-2.45, 13.8, -1.2], mat="pelt_dark")
    rig.cube("thorax", "pelt_rag_r2",      [2.45, 12.2, -2.0],  [2.95, 13.8, -1.2],  mat="pelt_dark")
    rig.cube("thorax", "pelt_rag_l3",      [-2.75, 11.6, -0.4], [-2.25, 13.2,  0.4], mat="flesh_scab")
    rig.cube("thorax", "pelt_rag_r3",      [2.25, 11.6, -0.4],  [2.75, 13.2,  0.4],  mat="flesh_scab")
    rig.cube("spine_lumbar", "pelt_rag_l4", [-2.45, 11.15,  1.4], [-1.95, 12.55,  2.2], mat="pelt_mangy")
    rig.cube("spine_lumbar", "pelt_rag_r4", [1.95, 11.15,  1.4],  [2.45, 12.55,  2.2], mat="pelt_mangy")
    rig.cube("spine_lumbar", "pelt_rag_l5", [-2.25, 10.8,  3.0], [-1.75, 12.2,  3.8], mat="flesh_scab")
    rig.cube("spine_lumbar", "pelt_rag_r5", [1.75, 10.8,  3.0],  [2.25, 12.2,  3.8], mat="flesh_scab")

    # ── 腹侧干瘪深凹肌肉膜与结痂溃烂皮 ──
    rig.cube("spine_lumbar", "belly_scab_mid",     [-1.3, 11.2, 1.2], [1.3, 12.6, 5.4], mat="flesh_scab")
    rig.cube("spine_lumbar", "belly_scab_flank_l", [-1.8, 11.6, 1.8], [-1.2, 12.8, 4.8], mat="flesh_scab")
    rig.cube("spine_lumbar", "belly_scab_flank_r", [1.2, 11.6, 1.8],  [1.8, 12.8, 4.8],  mat="flesh_scab")

    # ── 臀部残皮毛块与外露骨盆 ──
    rig.cube("hips", "pelt_rump_top",  [-2.2, 12.35, 5.8], [2.2, 13.52, 8.8], mat="pelt_dark")
    rig.cube("hips", "pelt_rump_l",    [-2.8, 11.4, 6.2],  [-2.0, 13.0, 8.6], mat="pelt_mangy")
    rig.cube("hips", "pelt_rump_r",    [2.0, 11.4, 6.2],   [2.8, 13.0, 8.6],  mat="pelt_mangy")
    rig.cube("hips", "bone_ilium_l",   [-2.6, 13.0, 5.4],  [-2.0, 13.8, 6.4], mat="bone_white") # 外露髂骨翼
    rig.cube("hips", "bone_ilium_r",   [2.0, 13.0, 5.4],   [2.6, 13.8, 6.4],  mat="bone_white")
    rig.cube("hips", "bone_ischium_l", [-2.5, 11.8, 8.8],  [-1.8, 12.8, 9.8], mat="bone_white") # 外露坐骨结节
    rig.cube("hips", "bone_ischium_r", [1.8, 11.8, 8.8],   [2.5, 12.8, 9.8],  mat="bone_white")


def part_limbs_tail(rig: Rig) -> None:
    """5. 四肢与尾巴：前肢大腿/小腿/掌+4钩爪；后肢反关节(跟骨后折)+4利爪；尾巴逐节变细骨节尾。"""
    # ── 左前肢 (-X) ──
    rig.cube("shoulder_l", "scapula_l",        [-3.35, 9.5, -5.6],  [-2.25, 13.8, -3.4], mat="pelt_dark")
    rig.cube("shoulder_l", "scapula_spine_l",  [-3.55, 10.5, -4.8], [-3.25, 13.0, -4.0], mat="bone_white") # 肩胛冈
    rig.cube("arm_l",      "humerus_l",        [-3.1, 6.8, -5.2],  [-2.3, 10.2, -4.0], mat="pelt_mangy")
    rig.cube("arm_l",      "elbow_olecranon_l",[-3.15, 6.4, -3.95],[-2.25, 7.6, -3.15],mat="bone_white") # 后突鹰嘴
    rig.cube("arm_l",      "forearm_radius_l", [-3.0, 1.8, -4.95], [-2.2, 7.0, -3.95], mat="pelt_mangy")
    rig.cube("arm_l",      "wrist_joint_l",    [-3.05, 1.2, -5.1], [-2.15, 2.0, -4.1], mat="bone_white")
    rig.cube("paw_fl",     "metacarpus_l",     [-3.1, 0.4, -5.3],  [-2.1, 1.4, -3.9],  mat="pelt_dark")

    # 4 根独立前爪 (左)
    for digit, x_off in enumerate([-0.75, -0.25, 0.25, 0.75]):
        dx = -2.6 + x_off
        rig.cube("paw_fl", f"toe_fl_{digit}",  [dx - 0.20, 0.0, -5.9], [dx + 0.20, 0.7, -5.2], mat="pelt_dark")
        rig.cube("paw_fl", f"claw_fl_{digit}", [dx - 0.15, 0.0, -6.6], [dx + 0.15, 0.4, -5.9], mat="bone_dark")

    # ── 右前肢 (+X) (对称) ──
    rig.cube("shoulder_r", "scapula_r",        [2.25, 9.5, -5.6],   [3.35, 13.8, -3.4],  mat="pelt_dark")
    rig.cube("shoulder_r", "scapula_spine_r",  [3.25, 10.5, -4.8],  [3.55, 13.0, -4.0],  mat="bone_white")
    rig.cube("arm_r",      "humerus_r",        [2.3, 6.8, -5.2],   [3.1, 10.2, -4.0],  mat="pelt_mangy")
    rig.cube("arm_r",      "elbow_olecranon_r",[2.25, 6.4, -3.95], [3.15, 7.6, -3.15], mat="bone_white")
    rig.cube("arm_r",      "forearm_radius_r", [2.2, 1.8, -4.95],  [3.0, 7.0, -3.95],  mat="pelt_mangy")
    rig.cube("arm_r",      "wrist_joint_r",    [2.15, 1.2, -5.1],  [3.05, 2.0, -4.1],  mat="bone_white")
    rig.cube("paw_fr",     "metacarpus_r",     [2.1, 0.4, -5.3],   [3.1, 1.4, -3.9],   mat="pelt_dark")

    for digit, x_off in enumerate([-0.75, -0.25, 0.25, 0.75]):
        dx = 2.6 + x_off
        rig.cube("paw_fr", f"toe_fr_{digit}",  [dx - 0.20, 0.0, -5.9], [dx + 0.20, 0.7, -5.2], mat="pelt_dark")
        rig.cube("paw_fr", f"claw_fr_{digit}", [dx - 0.15, 0.0, -6.6], [dx + 0.15, 0.4, -5.9], mat="bone_dark")

    # 后折膝关节与后突跟骨
    rig.cube("thigh_l", "knee_patella_l",     [-3.15, 6.6, 3.8], [-2.15, 8.0, 4.8], mat="bone_white") # 膝盖髌骨
    rig.cube("thigh_l", "knee_tendon_l",      [-3.0, 6.0, 4.6],  [-2.3, 7.2, 5.4],  mat="flesh_scab")  # 膝侧韧带
    rig.cube("shin_l",  "crus_shin_l",        [-3.1, 3.2, 5.4],  [-2.2, 7.4, 7.8],  mat="pelt_mangy")
    rig.cube("hock_l",  "hock_calcaneus_l",   [-3.05, 2.2, 7.6], [-2.25, 4.2, 9.2], mat="bone_white") # 后折跟骨
    rig.cube("hock_l",  "achilles_tendon_l",  [-2.9, 2.0, 6.8],  [-2.4, 3.8, 7.6],  mat="flesh_scab")  # 跟腱
    rig.cube("hock_l",  "metatarsus_l",       [-2.95, 1.0, 4.8], [-2.35, 3.2, 7.2], mat="pelt_mangy")
    rig.cube("paw_bl",  "ankle_joint_l",      [-3.0, 0.6, 4.4],  [-2.3, 1.4, 5.4],  mat="bone_white")

    for digit, x_off in enumerate([-0.75, -0.25, 0.25, 0.75]):
        dx = -2.65 + x_off
        rig.cube("paw_bl", f"toe_bl_{digit}",  [dx - 0.20, 0.0, 3.4], [dx + 0.20, 0.7, 4.6], mat="pelt_dark")
        rig.cube("paw_bl", f"claw_bl_{digit}", [dx - 0.15, 0.0, 2.6], [dx + 0.15, 0.4, 3.4], mat="bone_dark")

    # ── 右后肢 (+X) (反关节对称) ──
    rig.cube("thigh_r", "femur_thigh_r",      [1.8, 7.2, 4.4],   [3.4, 12.2, 7.6],  mat="pelt_dark")
    rig.cube("thigh_r", "femur_trochanter_r", [3.3, 9.8, 5.2],   [3.6, 11.5, 6.8],  mat="bone_white")
    rig.cube("thigh_r", "knee_patella_r",     [2.15, 6.6, 3.8],  [3.15, 8.0, 4.8],  mat="bone_white")
    rig.cube("thigh_r", "knee_tendon_r",      [2.3, 6.0, 4.6],   [3.0, 7.2, 5.4],   mat="flesh_scab")
    rig.cube("shin_r",  "crus_shin_r",        [2.2, 3.2, 5.4],   [3.1, 7.4, 7.8],   mat="pelt_mangy")
    rig.cube("hock_r",  "hock_calcaneus_r",   [2.25, 2.2, 7.6],  [3.05, 4.2, 9.2],  mat="bone_white")
    rig.cube("hock_r",  "achilles_tendon_r",  [2.4, 2.0, 6.8],   [2.9, 3.8, 7.6],   mat="flesh_scab")
    rig.cube("hock_r",  "metatarsus_r",       [2.35, 1.0, 4.8],  [2.95, 3.2, 7.2],  mat="pelt_mangy")
    rig.cube("paw_br",  "ankle_joint_r",      [2.3, 0.6, 4.4],   [3.0, 1.4, 5.4],   mat="bone_white")

    for digit, x_off in enumerate([-0.75, -0.25, 0.25, 0.75]):
        dx = 2.65 + x_off
        rig.cube("paw_br", f"toe_br_{digit}",  [dx - 0.20, 0.0, 3.4], [dx + 0.20, 0.7, 4.6], mat="pelt_dark")
        rig.cube("paw_br", f"claw_br_{digit}", [dx - 0.15, 0.0, 2.6], [dx + 0.15, 0.4, 3.4], mat="bone_dark")

    # ── 逐节变细的 8 节下垂骨节尾 ──
    tail_nodes = [
        # (z0, z1, y0, y1, rx, bone)
        ( 9.2, 10.8, 11.5, 12.6, 0.65, "tail_base"), # 尾椎 1 (根部)
        (10.8, 12.4, 10.4, 11.5, 0.58, "tail_base"), # 尾椎 2
        (12.4, 14.0,  9.2, 10.3, 0.50, "tail_mid"),  # 尾椎 3
        (14.0, 15.6,  7.9,  9.0, 0.44, "tail_mid"),  # 尾椎 4
        (15.6, 17.2,  6.5,  7.6, 0.38, "tail_mid"),  # 尾椎 5
        (17.2, 18.6,  5.2,  6.2, 0.32, "tail_tip"),  # 尾椎 6
        (18.6, 20.0,  3.8,  4.8, 0.28, "tail_tip"),  # 尾椎 7
        (20.0, 21.2,  2.6,  3.6, 0.22, "tail_tip"),  # 尾椎 8 (尖端)
    ]
    for i, (tz0, tz1, ty0, ty1, rx, b_name) in enumerate(tail_nodes):
        rig.cube(b_name, f"tail_vertebra_{i}", [-rx, ty0, tz0], [rx, ty1, tz1], mat="bone_white")

    # 尾根残毛与尾尖残毛
    rig.cube("tail_base", "tail_fur_base",  [-0.8, 11.6,  9.3], [0.8, 12.8, 10.6], mat="pelt_mangy")
    rig.cube("tail_tip",  "tail_fur_tip_1", [-0.45, 2.2, 20.8], [0.45, 3.8, 22.2], mat="pelt_dark")
    rig.cube("tail_tip",  "tail_fur_tip_2", [-0.30, 1.8, 22.2], [0.30, 3.0, 23.2], mat="pelt_dark")
    rig.cube("tail_tip",  "tail_fur_tip_3", [-0.48, 1.5, 21.6], [-0.22, 2.6, 22.8], mat="pelt_mangy")
    rig.cube("tail_tip",  "tail_fur_tip_4", [ 0.22, 1.5, 21.6], [ 0.48, 2.6, 22.8], mat="pelt_mangy")


PARTS = {
    "head":        ("1. 瘦长狼头与张开下颌獠牙",   part_head),
    "spine":       ("2. 从肩到尾 12 节突起骨节",    part_spine),
    "ribcage":     ("3. 7 对 4 段式弧形透空肋骨架",  part_ribcage),
    "pelt":        ("4. 残破锯齿下垂皮片与溃烂腹侧", part_pelt),
    "limbs_tail":  ("5. 三段式四肢(反关节)+8节骨节尾", part_limbs_tail),
}


def build_full() -> Rig:
    rig = Rig()
    for _, (_, func) in PARTS.items():
        func(rig)
    return rig


# =============================================================================
# 门禁与差分自证
# =============================================================================

def _assert_no_coplanar_faces(cubes: List[dict]):
    """检查立方体集是否存在同向同坐标重叠的共面冲突。"""
    faces: Dict[Tuple[str, float], List[dict]] = {}
    for c in cubes:
        f = c["from"]
        t = c["to"]
        for side, axis, val in [
            ("-X", 0, f[0]), ("+X", 0, t[0]),
            ("-Y", 1, f[1]), ("+Y", 1, t[1]),
            ("-Z", 2, f[2]), ("+Z", 2, t[2]),
        ]:
            if axis == 0:
                rect = (f[1], f[2], t[1], t[2])
            elif axis == 1:
                rect = (f[0], f[2], t[0], t[2])
            else:
                rect = (f[0], f[1], t[0], t[1])
            key = (side, round(val, 4))
            faces.setdefault(key, []).append((c["name"], rect))

    conflicts = []
    for (side, val), entries in faces.items():
        if len(entries) < 2:
            continue
        for i in range(len(entries)):
            for j in range(i + 1, len(entries)):
                n1, r1 = entries[i]
                n2, r2 = entries[j]
                u0 = max(r1[0], r2[0])
                u1 = min(r1[2], r2[2])
                v0 = max(r1[1], r2[1])
                v1 = min(r1[3], r2[3])
                if u1 - u0 > 0.001 and v1 - v0 > 0.001:
                    conflicts.append(
                        f"共面冲突: {n1} 与 {n2} 在 {side} 面共面 ({val}), 重叠区域 ({(u1-u0):.3f}x{(v1-v0):.3f})"
                    )
    if conflicts:
        raise AssertionError("\n".join(conflicts))


def self_test():
    """运行严密差分自证：几何无共面冲突、贴地、尺寸与缺陷注入拦截。"""
    print("运行 gen_ku_pi_lang.py 差分自证...")
    rig = build_full()
    elements = rig.elements

    # 1. 严格共面检查
    _assert_no_coplanar_faces(elements)
    print(f"  [OK] ku_pi_lang ({len(elements)} 个立方体) 正常立方体集无共面冲突")

    # 2. 贴地检查 (最低点必须贴地 y=0.0)
    min_y = min(e["from"][1] for e in elements)
    assert abs(min_y - 0.0) < 1e-4, f"贴地断言失败: 最低点 y={min_y:.2f} 不等于 0.0"
    print(f"  [OK] 贴地断言通过 (最低点 y={min_y:.2f})")

    # 3. 体型尺寸断言 (肩峰高约 17.5px, 体长约 40px)
    max_y = max(e["to"][1] for e in elements)
    min_z = min(e["from"][2] for e in elements)
    max_z = max(e["to"][2] for e in elements)
    total_len = max_z - min_z
    assert 16.0 <= max_y <= 22.0, f"体高超出预期范围: max_y={max_y:.2f}"
    assert 35.0 <= total_len <= 45.0, f"体长超出预期范围: total_len={total_len:.2f}"
    print(f"  [OK] 尺寸断言通过 (体高 {max_y:.1f}px, 吻尾纵长 {total_len:.1f}px)")

    # 4. 部件数检查 (必须至少翻三倍: >= 240 个)
    assert len(elements) >= 249, f"部件数不足: {len(elements)} < 249"
    print(f"  [OK] 部件数达标: {len(elements)} 个 (Round 1 为 83 个，翻满整整三倍)")

    # 5. 缺陷注入拦截测试
    defect_rig = build_full()
    defect_rig.cube("thorax", "inject_coplanar_fail", [-2.0, 13.5, -11.5], [2.0, 16.4, -8.5], mat="pelt_dark")
    caught = False
    try:
        _assert_no_coplanar_faces(defect_rig.elements)
    except AssertionError as e:
        caught = True
        print(f"  [OK] 成功拦截注入共面缺陷: {e.args[0].splitlines()[0]}")
    if not caught:
        raise RuntimeError("门禁失效: 注入共面冲突未被拦截!")

    print("✓ gen_ku_pi_lang.py 差分自证全绿")


# =============================================================================
# 渲染：逐部件预览 + NOW vs PREV 六视角接触表 + 对标卡 (check.png)
# =============================================================================

def render_part_previews(tex_img: Image.Image):
    """为 5 大编号部件分别生成独立的单件三视角预览图。"""
    from bbmodel_maker.render.render_bbmodel import render

    PARTS_DIR.mkdir(parents=True, exist_ok=True)
    bg_color = (119, 119, 119)
    print("渲染 5 大部件独立预览图...")

    for i, (part_key, (part_name, func)) in enumerate(PARTS.items(), start=1):
        rig = Rig()
        func(rig)
        bb_json = rig.bbmodel(f"KuPiLang_{part_key}", tex_img)
        part_bbmodel = CREATURE_DIR / f"KuPiLang_part_{i}_{part_key}.bbmodel"
        with open(part_bbmodel, "w", encoding="utf-8") as f:
            json.dump(bb_json, f, indent=2)

        # 渲染正视、3/4、侧视 3 个角度并排
        im_front, _ = render(part_bbmodel, yaw=0.0, pitch=0.0, size=300, bg=bg_color)
        im_iso, _   = render(part_bbmodel, yaw=-45.0, pitch=25.0, size=300, bg=bg_color)
        im_side, _  = render(part_bbmodel, yaw=-90.0, pitch=0.0, size=300, bg=bg_color)

        canvas = Image.new("RGB", (300 * 3 + 20, 300 + 40), (32, 33, 36))
        draw = ImageDraw.Draw(canvas)
        canvas.paste(im_front, (5, 30))
        canvas.paste(im_iso, (310, 30))
        canvas.paste(im_side, (615, 30))

        draw.text((10, 8), f"PART {i}: {part_name} ({len(rig.elements)} cubes)", fill=(235, 235, 235))
        draw.text((10, 305), "FRONT", fill=(180, 180, 180))
        draw.text((315, 305), "3/4 ISOMETRIC", fill=(180, 180, 180))
        draw.text((620, 305), "SIDE", fill=(180, 180, 180))

        preview_out = PARTS_DIR / f"part_{i}_{part_key}.png"
        canvas.save(preview_out)
        print(f"  ✓ 部件 {i} 预览已输出: {preview_out}")

        # 删除临时单部件 bbmodel
        part_bbmodel.unlink(missing_ok=True)


def render_contact_and_check(now_bbmodel: Path, prev_bbmodel: Path):
    """输出包含 NOW 与 PREV (Round 1) 同取景对比及爆炸参考图的完整 check.png。"""
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    bg_color = (119, 119, 119)

    print("渲染 NOW (Round 2) 与 PREV (Round 1) 六视角对比图...")
    w_cell, h_cell = 400, 400

    def render_6_views(path: Path) -> Image.Image:
        im_front, _ = render(path, yaw=0.0, pitch=0.0, size=w_cell, bg=bg_color)
        im_iso, _   = render(path, yaw=-45.0, pitch=25.0, size=w_cell, bg=bg_color)
        im_side_l, _= render(path, yaw=-90.0, pitch=0.0, size=w_cell, bg=bg_color)
        im_side_r, _= render(path, yaw=90.0, pitch=0.0, size=w_cell, bg=bg_color)
        im_top, _   = render(path, yaw=0.0, pitch=89.9, size=w_cell, bg=bg_color)
        im_back, _  = render(path, yaw=180.0, pitch=0.0, size=w_cell, bg=bg_color)

        sheet = Image.new("RGB", (w_cell * 3 + 20, h_cell * 2 + 30), (32, 33, 36))
        s_draw = ImageDraw.Draw(sheet)
        views = [
            ("FRONT (-Z Head)", im_front, 5, 5),
            ("3/4 ISOMETRIC",   im_iso, w_cell + 10, 5),
            ("SIDE_L (-X Flank)", im_side_l, w_cell * 2 + 15, 5),
            ("SIDE_R (+X Flank)", im_side_r, 5, h_cell + 15),
            ("TOP (+Y Spine)",    im_top, w_cell + 10, h_cell + 15),
            ("BACK (+Z Tail)",    im_back, w_cell * 2 + 15, h_cell + 15),
        ]
        for title, img_v, px, py in views:
            sheet.paste(img_v, (px, py))
            s_draw.rectangle([px, py, px + 280, py + 22], fill=(22, 23, 26))
            s_draw.text((px + 6, py + 4), title, fill=(235, 235, 235))
        return sheet

    sheet_now = render_6_views(now_bbmodel)
    # 保存当前的独立 render.png
    sheet_now.save(REVIEW_DIR / "render.png")
    print(f"✓ 当前六视角接触表已保存: {REVIEW_DIR / 'render.png'}")

    has_prev = prev_bbmodel.exists()
    sheet_prev = render_6_views(prev_bbmodel) if has_prev else None

    # 组合 check.png: 左栏 爆炸参考图 / 中栏 PREV (Round 1) / 右栏 NOW (Round 2)
    target_h = sheet_now.height
    ref_im = Image.open(EXPLODED_REF).convert("RGB") if EXPLODED_REF.exists() else None
    if ref_im:
        ref_w = int(ref_im.width * target_h / ref_im.height)
        ref_scaled = ref_im.resize((ref_w, target_h), Image.Resampling.LANCZOS)
    else:
        ref_w = 400
        ref_scaled = Image.new("RGB", (ref_w, target_h), (50, 50, 50))

    if sheet_prev:
        total_w = ref_w + sheet_prev.width + sheet_now.width + 40
        canvas_check = Image.new("RGB", (total_w, target_h + 46), (25, 26, 29))
        c_draw = ImageDraw.Draw(canvas_check)

        # 1. 左栏 爆炸参考图
        canvas_check.paste(ref_scaled, (10, 36))
        c_draw.text((15, 10), "EXPLODED REFERENCE (exploded.png)", fill=(215, 205, 180))

        # 2. 中栏 PREV (Round 1: 83 cubes)
        px = ref_w + 20
        canvas_check.paste(sheet_prev, (px, 36))
        c_draw.text((px + 6, 10), "PREV: Round 1 (83 cubes - coarse blocky base)", fill=(230, 160, 140))

        # 3. 右栏 NOW (Round 2: 240+ cubes)
        nx = px + sheet_prev.width + 10
        canvas_check.paste(sheet_now, (nx, 36))
        c_draw.text((nx + 6, 10), "NOW: Round 2 (240+ cubes - 0.5px hollow ribcage / individual vertebrae)", fill=(140, 230, 180))
    else:
        total_w = ref_w + sheet_now.width + 30
        canvas_check = Image.new("RGB", (total_w, target_h + 46), (25, 26, 29))
        c_draw = ImageDraw.Draw(canvas_check)
        canvas_check.paste(ref_scaled, (10, 36))
        c_draw.text((15, 10), "EXPLODED REFERENCE (exploded.png)", fill=(215, 205, 180))
        nx = ref_w + 20
        canvas_check.paste(sheet_now, (nx, 36))
        c_draw.text((nx + 6, 10), "NOW: Round 2 (240+ cubes - 0.5px precision)", fill=(140, 230, 180))

    check_out = REVIEW_DIR / "check.png"
    canvas_check.save(check_out)
    print(f"✓ 全新对标卡已保存 (左参考 / 中 PREV / 右 NOW): {check_out}")


# =============================================================================
# CLI 主入口
# =============================================================================

def main():
    parser = argparse.ArgumentParser(description="枯皮狼 (ku_pi_lang) 四足生物生成器 (Round 2)")
    parser.add_argument("--self-test", action="store_true", help="运行门禁差分自证")
    parser.add_argument("--list", action="store_true", help="列出所有 5 大编号部件")
    parser.add_argument("--part", choices=sorted(PARTS.keys()), help="只生成单个部件预览")
    args = parser.parse_args()

    if args.list:
        print("枯皮狼 (ku_pi_lang) 5 大编号部件清单:")
        for k, (desc, _) in PARTS.items():
            print(f"  - {k:12s} : {desc}")
        return

    if args.self_test:
        self_test()
        return

    # 1. 运行差分自测
    self_test()

    CREATURE_DIR.mkdir(parents=True, exist_ok=True)
    MODELS_DIR.mkdir(parents=True, exist_ok=True)
    tex_img = generate_texture()

    # 2. 单部件模式
    if args.part:
        part_key = args.part
        desc, func = PARTS[part_key]
        rig = Rig()
        func(rig)
        bb_json = rig.bbmodel(f"KuPiLang_{part_key}", tex_img)
        part_out = CREATURE_DIR / f"KuPiLang_{part_key}.bbmodel"
        with open(part_out, "w", encoding="utf-8") as f:
            json.dump(bb_json, f, indent=2)
        print(f"✓ 单部件预览已生成: {part_out} ({len(rig.elements)} 个立方体)")
        return

    # 3. 逐部件渲染独立预览图
    render_part_previews(tex_img)

    # 4. 全量拼接生成完整全身模型
    full_rig = build_full()
    bb_json = full_rig.bbmodel("KuPiLang", tex_img)

    with open(BBMODEL_OUT, "w", encoding="utf-8") as f:
        json.dump(bb_json, f, indent=2)
    with open(BBMODEL_COPY, "w", encoding="utf-8") as f:
        json.dump(bb_json, f, indent=2)
    print(f"✓ 全身模型已生成: {BBMODEL_OUT} (包含 {len(full_rig.elements)} 个立方体)")

    # 5. 输出包含 NOW vs PREV 同取景对比的全新 check.png
    render_contact_and_check(BBMODEL_OUT, PREV_BBMODEL)


if __name__ == "__main__":
    main()
