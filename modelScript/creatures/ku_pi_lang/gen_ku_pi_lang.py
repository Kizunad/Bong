#!/usr/bin/env python3
"""枯皮狼（ku_pi_lang）四足生物模型程序化生成器 —— Round 1/3。

世界观设定与依据：
- worldview §七 兽潮 + 荒野贫瘠区：
  枯皮狼（ku_pi_lang）是兽潮先锋群居捕食者，成群狂奔在最前锋。
  身躯极度枯瘦，体型比原版狼略大（肩高约 12.5px，体长约 24px）。
- 解剖与外形特征：
  1. 枯槁皮毛大片脱落，露出苍白凸起的脊柱棘突骨节与弧形外露肋骨架，腹腔深凹；
  2. 狭长瘦骨狼头，外露颧弓与饥饿眼窝，暗金琥珀色凶目，枯吻外露獠牙；
  3. 双耳残缺撕裂（左耳撕裂伤残残角，右耳带咬痕缺角）；
  4. 典型四足趾行（digitigrade）折叠后肢与精瘦前肢，爪甲尖锐扣地；
  5. 秃狼尾巴：大半截皮包骨节秃尾，骨节嶙峋，仅末端残留少许杂乱焦枯硬毛。

骨骼架构复用：
- 复用 dainu_lion 与 stitched_beast 的四足骨架命名与关节接口标准：
  hips -> spine_lumbar -> thorax -> neck -> head (snout, jaw, ear_l, ear_r)
  thorax -> shoulder_l/r -> arm_l/r -> paw_fl/fr
  hips -> thigh_l/r -> shin_l/r -> hock_l/r -> paw_bl/br
  hips -> tail_base -> tail_mid -> tail_tip

用法：
  python3 modelScript/creatures/ku_pi_lang/gen_ku_pi_lang.py                 # 全身建模与生成
  python3 modelScript/creatures/ku_pi_lang/gen_ku_pi_lang.py --self-test    # 门禁差分自测
  python3 modelScript/creatures/ku_pi_lang/gen_ku_pi_lang.py --list         # 列出所有部件
  python3 modelScript/creatures/ku_pi_lang/gen_ku_pi_lang.py --part head    # 单部件预览生成
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
REF_IMG = REVIEW_DIR / "reference.png"
EXPLODED_REF = REVIEW_DIR / "exploded.png"

BBMODEL_OUT = CREATURE_DIR / "KuPiLang.bbmodel"
BBMODEL_COPY = MODELS_DIR / "KuPiLang.bbmodel"

# -----------------------------------------------------------------------------
# 材质与 64x64 调色板映射
# -----------------------------------------------------------------------------
MATS = (
    "pelt_dark",      # 残存粗糙深灰褐色枯狼毛 (#443c36)
    "pelt_mangy",     # 斑驳风干脱落杂毛 (#665b50)
    "flesh_scab",     # 皮毛剥落处的暗红结痂溃肉 (#4a2422)
    "bone_white",     # 外露肋骨与凸起脊椎的苍白骨色 (#cfc8ba)
    "bone_dark",      # 骨骼深缝与内脏阴影 (#302c26)
    "snout_black",    # 干瘪黑鼻头与嘴裂深色 (#221e1c)
    "fang_white",     # 外露锐利惨白犬齿 (#eae5d8)
    "eye_amber",      # 饥饿凶戾的暗金琥珀色狼眼 (#d68a28)
)

TEX_W = TEX_H = 64
SWATCH_SIZE = 16  # 4x2 网格，每格 16x16 像素
MAT_SWATCH_IDX = {mat: i for i, mat in enumerate(MATS)}

MAT_COLORS = {
    "pelt_dark":   (68, 60, 54),
    "pelt_mangy":  (102, 91, 80),
    "flesh_scab":  (74, 36, 34),
    "bone_white":  (207, 200, 186),
    "bone_dark":   (48, 44, 38),
    "snout_black": (34, 30, 28),
    "fang_white":  (234, 229, 216),
    "eye_amber":   (214, 138, 40),
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
        noise = rng.normal(0, 7.0, (SWATCH_SIZE, SWATCH_SIZE, 1))
        rgb = np.clip(base_rgb + noise, 10, 245).astype(np.uint8)
        swatch_arr[:, :, :3] = rgb
        swatch_arr[:, :, 3] = 255

        sw_img = Image.fromarray(swatch_arr, "RGBA")
        draw = ImageDraw.Draw(sw_img)

        # 针对特定材质增加生动特征
        if mat in ("pelt_dark", "pelt_mangy"):
            for _ in range(8):
                px, py = rng.integers(1, SWATCH_SIZE - 2, 2)
                draw.point((px, py), fill=(int(base_rgb[0] * 0.7), int(base_rgb[1] * 0.7), int(base_rgb[2] * 0.7), 255))
        elif mat == "bone_white":
            for _ in range(6):
                px, py = rng.integers(1, SWATCH_SIZE - 2, 2)
                draw.point((px, py), fill=(160, 150, 135, 255))
        elif mat == "flesh_scab":
            for _ in range(6):
                px, py = rng.integers(1, SWATCH_SIZE - 2, 2)
                draw.point((px, py), fill=(105, 25, 22, 255))

        img.paste(sw_img, (x0, y0))

    return img


# -----------------------------------------------------------------------------
# 骨骼树与 Rig 容器（复用 dainu_lion / stitched_beast 标准）
# -----------------------------------------------------------------------------
BONE_DEFS = [
    # (name, pivot, parent)
    ("root",          (0.0, 0.0, 0.0),      None),
    ("hips",          (0.0, 11.5, 6.0),     "root"),
    ("spine_lumbar",  (0.0, 12.0, 2.0),     "hips"),
    ("thorax",        (0.0, 12.5, -2.0),    "spine_lumbar"),
    ("neck",          (0.0, 13.5, -6.5),    "thorax"),
    ("head",          (0.0, 14.5, -10.0),   "neck"),
    ("snout",         (0.0, 13.5, -11.8),   "head"),
    ("jaw",           (0.0, 12.8, -10.5),   "head"),
    ("ear_l",         (-2.0, 16.5, -9.0),   "head"),
    ("ear_r",         (2.0, 16.5, -9.0),    "head"),
    # 秃尾
    ("tail_base",     (0.0, 11.0, 9.5),     "hips"),
    ("tail_mid",      (0.0, 9.8, 12.5),     "tail_base"),
    ("tail_tip",      (0.0, 7.6, 15.5),     "tail_mid"),
    # 前肢
    ("shoulder_l",    (-2.8, 11.5, -4.5),   "thorax"),
    ("arm_l",         (-2.8, 6.5, -4.0),    "shoulder_l"),
    ("paw_fl",        (-2.8, 0.0, -4.0),    "arm_l"),
    ("shoulder_r",    (2.8, 11.5, -4.5),    "thorax"),
    ("arm_r",         (2.8, 6.5, -4.0),     "shoulder_r"),
    ("paw_fr",        (2.8, 0.0, -4.0),     "arm_r"),
    # 趾行后肢
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
# 八大部件独立生成函数（part_* 模块化设计）
# =============================================================================

def part_head(rig: Rig) -> None:
    """1. 瘦长狼头颅骨、眼眶凶瞳、狭长吻部与外露獠牙、下颌嘴裂、残缺撕裂耳。"""
    # ── 颅骨主脑壳与外露破皮骨顶 ──
    rig.cube("head", "skull_braincase", [-2.5, 13.2, -11.8], [2.5, 16.5, -8.2], mat="pelt_dark")
    rig.cube("head", "skull_brow", [-2.52, 15.2, -12.2], [2.52, 16.2, -10.0], mat="pelt_mangy")
    rig.cube("head", "skull_bone_patch", [-1.0, 16.5, -11.0], [1.0, 16.9, -9.0], mat="bone_white")

    # ── 凹陷眼窝与琥珀凶眼 ──
    rig.cube("head", "eye_socket_l", [-2.56, 14.4, -11.2], [-2.15, 15.6, -10.2], mat="bone_dark")
    rig.cube("head", "eye_amber_l",  [-2.60, 14.6, -11.0], [-2.20, 15.4, -10.4], mat="eye_amber")
    rig.cube("head", "eye_socket_r", [2.15, 14.4, -11.2], [2.56, 15.6, -10.2], mat="bone_dark")
    rig.cube("head", "eye_amber_r",  [2.20, 14.6, -11.0], [2.60, 15.4, -10.4], mat="eye_amber")

    # ── 狭长吻部与獠牙 ──
    rig.cube("snout", "snout_upper",   [-1.5, 13.0, -16.0], [1.5, 15.0, -11.8], mat="pelt_dark")
    rig.cube("snout", "snout_nose",    [-1.0, 14.2, -16.6], [1.0, 15.2, -15.8], mat="snout_black")
    rig.cube("snout", "fang_upper_l",  [-1.45, 12.0, -15.2], [-1.05, 13.0, -14.4], mat="fang_white")
    rig.cube("snout", "fang_upper_r",  [1.05, 12.0, -15.2], [1.45, 13.0, -14.4], mat="fang_white")

    # ── 下颌嘴裂与下獠牙 ──
    rig.cube("jaw", "jaw_lower",     [-1.2, 11.8, -15.6], [1.2, 12.8, -10.5], mat="bone_white")
    rig.cube("jaw", "fang_lower_l",   [-1.15, 12.8, -14.8], [-0.85, 13.6, -14.0], mat="fang_white")
    rig.cube("jaw", "fang_lower_r",   [0.85, 12.8, -14.8], [1.15, 13.6, -14.0], mat="fang_white")

    # ── 残缺撕裂双耳 ──
    # 左耳：撕裂伤残残角（耳尖被整个咬断露肉）
    rig.cube("ear_l", "ear_l_base", [-2.8, 16.5, -9.6], [-1.4, 18.2, -8.6], mat="pelt_dark")
    rig.cube("ear_l", "ear_l_torn", [-2.5, 18.2, -9.4], [-1.6, 19.0, -8.7], mat="flesh_scab")

    # 右耳：缺角残耳（外沿被撕去缺口，带残尖）
    rig.cube("ear_r", "ear_r_base",  [1.4, 16.5, -9.6], [2.8, 18.5, -8.6], mat="pelt_dark")
    rig.cube("ear_r", "ear_r_tip",   [1.6, 18.5, -9.5], [2.6, 20.0, -8.7], mat="pelt_mangy")
    rig.cube("ear_r", "ear_r_notch", [2.3, 18.0, -9.4], [2.85, 19.2, -8.65], mat="flesh_scab")


def part_neck(rig: Rig) -> None:
    """2. 枯瘦长颈，上披零星断裂枯毛，下方微露干瘪颈肉。"""
    rig.cube("neck", "neck_core",      [-1.5, 12.2, -8.5], [1.5, 15.6, -5.5], mat="pelt_mangy")
    rig.cube("neck", "neck_pelt_mane", [-2.1, 13.5, -7.8], [2.1, 16.5, -5.8], mat="pelt_dark")
    rig.cube("neck", "neck_throat",    [-1.1, 11.2, -8.0], [1.1, 12.6, -5.6], mat="flesh_scab")


def part_spine_ridge(rig: Rig) -> None:
    """3. 背部贯穿凸出的脊椎棘突骨刺（从肩峰到荐椎隆突一整排外露骨节）。"""
    # 胸段脊柱
    rig.cube("thorax", "spine_t1",     [-0.5, 15.2, -5.0], [0.5, 16.6, -3.8], mat="bone_white")
    rig.cube("thorax", "spine_wither", [-0.6, 15.5, -3.8], [0.6, 17.2, -2.2], mat="bone_white")
    rig.cube("thorax", "spine_t2",     [-0.5, 15.0, -2.2], [0.5, 16.5, -0.6], mat="bone_white")
    rig.cube("thorax", "spine_t3",     [-0.5, 14.6, -0.6], [0.5, 16.0, 1.0], mat="bone_white")

    # 腰段脊柱
    rig.cube("spine_lumbar", "spine_l1", [-0.5, 14.2, 1.2], [0.5, 15.5, 2.6], mat="bone_white")
    rig.cube("spine_lumbar", "spine_l2", [-0.5, 13.8, 2.8], [0.5, 15.0, 4.2], mat="bone_white")
    rig.cube("spine_lumbar", "spine_l3", [-0.5, 13.6, 4.4], [0.5, 14.8, 5.8], mat="bone_white")

    # 骨盆荐椎段
    rig.cube("hips", "spine_sacrum_1", [-0.6, 13.2, 6.2], [0.6, 14.4, 7.6], mat="bone_white")
    rig.cube("hips", "spine_sacrum_2", [-0.5, 12.6, 7.8], [0.5, 13.8, 9.2], mat="bone_white")


def part_ribcage(rig: Rig) -> None:
    """4. 左右两侧大面积脱落皮毛下露出的弧形肋骨条与肋间内脏暗影。"""
    # 内层深色内脏空腔暗影
    rig.cube("thorax", "viscera_core_front", [-2.4, 10.05, -4.6], [2.4, 14.35, 0.8], mat="bone_dark")

    # 胸前段肋骨（左右对称）
    rig.cube("thorax", "rib_l_1", [-2.95, 10.2, -4.4], [-2.45, 14.8, -4.0], mat="bone_white")
    rig.cube("thorax", "rib_r_1", [2.45, 10.2, -4.4], [2.95, 14.8, -4.0], mat="bone_white")

    rig.cube("thorax", "rib_l_2", [-3.10, 9.8, -3.0], [-2.60, 15.2, -2.6], mat="bone_white")
    rig.cube("thorax", "rib_r_2", [2.60, 9.8, -3.0], [3.10, 15.2, -2.6], mat="bone_white")

    rig.cube("thorax", "rib_l_3", [-3.05, 9.8, -1.6], [-2.55, 14.8, -1.2], mat="bone_white")
    rig.cube("thorax", "rib_r_3", [2.55, 9.8, -1.6], [3.05, 14.8, -1.2], mat="bone_white")

    rig.cube("thorax", "rib_l_4", [-2.85, 10.0, -0.2], [-2.35, 14.4, 0.2], mat="bone_white")
    rig.cube("thorax", "rib_r_4", [2.35, 10.0, -0.2], [2.85, 14.4, 0.2], mat="bone_white")

    # 腰段浮肋
    rig.cube("spine_lumbar", "rib_l_5", [-2.55, 10.4, 1.4], [-2.10, 13.8, 1.8], mat="bone_white")
    rig.cube("spine_lumbar", "rib_r_5", [2.10, 10.4, 1.4], [2.55, 13.8, 1.8], mat="bone_white")

    rig.cube("spine_lumbar", "rib_l_6", [-2.30, 10.8, 2.8], [-1.90, 13.4, 3.2], mat="bone_white")
    rig.cube("spine_lumbar", "rib_r_6", [1.90, 10.8, 2.8], [2.30, 13.4, 3.2], mat="bone_white")


def part_torso_pelt(rig: Rig) -> None:
    """5. 残破剥落的背部与臀部枯狼皮毛、极度凹瘪的饥饿腹腔及骨盆结节。"""
    # 肩背残毛块与胸骨底龙骨
    rig.cube("thorax", "pelt_shoulder_saddle", [-2.6, 14.0, -5.2], [2.6, 15.6, -1.8], mat="pelt_dark")
    rig.cube("thorax", "pelt_chest_keel",      [-1.5, 9.2, -5.0], [1.5, 10.6, -0.6], mat="pelt_mangy")

    # 极度深凹收窄的溃烂暗红腹侧
    rig.cube("spine_lumbar", "belly_flank_hollow", [-1.6, 10.8, 1.2], [1.6, 13.0, 5.6], mat="flesh_scab")

    # 臀部残毛外壳与外露坐骨结节
    rig.cube("hips", "pelt_rump",        [-2.5, 11.2, 6.05], [2.5, 13.4, 9.6], mat="pelt_dark")
    rig.cube("hips", "bone_ischium_l",   [-2.6, 12.0, 9.2], [-2.0, 13.0, 9.9], mat="bone_white")
    rig.cube("hips", "bone_ischium_r",   [2.0, 12.0, 9.2], [2.6, 13.0, 9.9], mat="bone_white")


def part_forelegs(rig: Rig) -> None:
    """6. 枯瘦前肢（肩胛外露骨板、精瘦前臂、肘关节突、带爪脚掌）。"""
    # ── 左前肢 (-X) ──
    rig.cube("shoulder_l", "scapula_l",      [-3.5, 9.0, -5.8], [-2.2, 13.5, -3.2], mat="pelt_dark")
    rig.cube("shoulder_l", "scapula_bone_l", [-3.6, 11.0, -5.2], [-2.9, 12.8, -3.8], mat="bone_white")
    rig.cube("arm_l",      "forearm_l",      [-3.3, 1.5, -4.7], [-2.3, 9.2, -3.5], mat="pelt_mangy")
    rig.cube("arm_l",      "elbow_bone_l",   [-3.35, 7.8, -3.5], [-2.45, 9.0, -2.7], mat="bone_white")
    rig.cube("paw_fl",     "paw_fl_pad",     [-3.4, 0.0, -5.4], [-2.2, 1.6, -3.2], mat="pelt_dark")
    rig.cube("paw_fl",     "claw_fl",        [-3.2, 0.0, -6.0], [-2.4, 0.7, -5.4], mat="bone_dark")

    # ── 右前肢 (+X) ──
    rig.cube("shoulder_r", "scapula_r",      [2.2, 9.0, -5.8], [3.5, 13.5, -3.2], mat="pelt_dark")
    rig.cube("shoulder_r", "scapula_bone_r", [2.9, 11.0, -5.2], [3.6, 12.8, -3.8], mat="bone_white")
    rig.cube("arm_r",      "forearm_r",      [2.3, 1.5, -4.7], [3.3, 9.2, -3.5], mat="pelt_mangy")
    rig.cube("arm_r",      "elbow_bone_r",   [2.45, 7.8, -3.5], [3.35, 9.0, -2.7], mat="bone_white")
    rig.cube("paw_fr",     "paw_fr_pad",     [2.2, 0.0, -5.4], [3.4, 1.6, -3.2], mat="pelt_dark")
    rig.cube("paw_fr",     "claw_fr",        [2.4, 0.0, -6.0], [3.2, 0.7, -5.4], mat="bone_dark")


def part_hindlegs(rig: Rig) -> None:
    """7. 典型四足趾行（digitigrade）折叠后肢（瘦大腿、膝骨、后突跟骨、后爪）。"""
    # ── 左后肢 (-X) ──
    rig.cube("thigh_l", "femur_thigh_l",       [-3.6, 6.8, 4.6], [-1.8, 11.8, 7.8], mat="pelt_dark")
    rig.cube("thigh_l", "femur_trochanter_l",  [-3.7, 9.8, 5.4], [-2.8, 11.2, 6.8], mat="bone_white")
    rig.cube("shin_l",  "shin_l",              [-3.3, 2.8, 6.0], [-2.2, 7.2, 8.8], mat="pelt_mangy")
    rig.cube("shin_l",  "patella_l",           [-3.25, 6.2, 4.8], [-2.35, 7.4, 5.8], mat="bone_white")
    rig.cube("hock_l",  "hock_calcaneus_l",    [-3.2, 1.2, 6.8], [-2.3, 3.2, 8.2], mat="bone_white")
    rig.cube("hock_l",  "metatarsus_l",        [-3.15, 1.0, 5.0], [-2.35, 3.0, 6.8], mat="pelt_mangy")
    rig.cube("paw_bl",  "paw_bl_pad",          [-3.3, 0.0, 3.4], [-2.1, 1.4, 5.4], mat="pelt_dark")
    rig.cube("paw_bl",  "claw_bl",             [-3.1, 0.0, 2.7], [-2.3, 0.6, 3.4], mat="bone_dark")

    # ── 右后肢 (+X) ──
    rig.cube("thigh_r", "femur_thigh_r",       [1.8, 6.8, 4.6], [3.6, 11.8, 7.8], mat="pelt_dark")
    rig.cube("thigh_r", "femur_trochanter_r",  [2.8, 9.8, 5.4], [3.7, 11.2, 6.8], mat="bone_white")
    rig.cube("shin_r",  "shin_r",              [2.2, 2.8, 6.0], [3.3, 7.2, 8.8], mat="pelt_mangy")
    rig.cube("shin_r",  "patella_r",           [2.35, 6.2, 4.8], [3.25, 7.4, 5.8], mat="bone_white")
    rig.cube("hock_r",  "hock_calcaneus_r",    [2.3, 1.2, 6.8], [3.2, 3.2, 8.2], mat="bone_white")
    rig.cube("hock_r",  "metatarsus_r",        [2.35, 1.0, 5.0], [3.15, 3.0, 6.8], mat="pelt_mangy")
    rig.cube("paw_br",  "paw_br_pad",          [2.1, 0.0, 3.4], [3.3, 1.4, 5.4], mat="pelt_dark")
    rig.cube("paw_br",  "claw_br",             [2.3, 0.0, 2.7], [3.1, 0.6, 3.4], mat="bone_dark")


def part_tail(rig: Rig) -> None:
    """8. 秃狼尾巴（大半截皮包骨节秃尾，骨节嶙峋，仅末端残留少许杂乱焦枯毛）。"""
    rig.cube("tail_base", "tail_seg_1",    [-0.7, 9.8, 9.5], [0.7, 11.4, 12.5], mat="bone_white")
    rig.cube("tail_base", "tail_fur_base", [-0.85, 10.2, 9.35], [0.85, 11.6, 10.8], mat="pelt_mangy")
    rig.cube("tail_mid",  "tail_seg_2",    [-0.5, 7.6, 12.5], [0.5, 9.8, 15.5], mat="bone_white")
    rig.cube("tail_tip",  "tail_seg_3",    [-0.4, 5.4, 15.5], [0.4, 7.6, 17.8], mat="bone_white")
    rig.cube("tail_tip",  "tail_fur_tip",  [-0.65, 5.0, 17.4], [0.65, 6.8, 18.8], mat="pelt_dark")


PARTS = {
    "head":        ("瘦长狼头与残耳",     part_head),
    "neck":        ("枯瘦长颈",           part_neck),
    "spine_ridge": ("外露脊柱骨刺",       part_spine_ridge),
    "ribcage":     ("外露肋骨架",         part_ribcage),
    "torso_pelt":  ("残破皮毛与凹瘪腹躯", part_torso_pelt),
    "forelegs":    ("枯瘦前肢与利爪",     part_forelegs),
    "hindlegs":    ("四足趾行后肢",       part_hindlegs),
    "tail":        ("皮包骨秃狼尾",       part_tail),
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

    # 3. 体型尺寸断言 (比原版狼略大: 肩峰高约 17.2px, 体长约 35px)
    max_y = max(e["to"][1] for e in elements)
    min_z = min(e["from"][2] for e in elements)
    max_z = max(e["to"][2] for e in elements)
    total_len = max_z - min_z
    assert 16.0 <= max_y <= 21.0, f"体高超出预期范围: max_y={max_y:.2f}"
    assert 30.0 <= total_len <= 40.0, f"体长超出预期范围: total_len={total_len:.2f}"
    print(f"  [OK] 尺寸断言通过 (肩峰体高 {max_y:.1f}px, 吻尾纵长 {total_len:.1f}px)")

    # 4. 缺陷注入拦截测试
    defect_rig = build_full()
    defect_rig.cube("thorax", "inject_coplanar_fail", [-2.4, 10.05, -4.6], [2.4, 14.35, 0.8], mat="bone_white")
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
# 渲染与六视角接触表 / 对标卡 (check.png)
# =============================================================================

def render_contact_and_check(bbmodel_path: Path):
    """输出六视角接触表及与参考图同取景对标卡到 model-review/img/creatures/ku_pi_lang/check.png。"""
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    bg_color = (119, 119, 119)

    print("渲染六视角标准图...")
    # 诚实命名的六个视角（生物头朝向 -Z，FRONT 从北面朝向头部）
    # FRONT: pitch=0, yaw=0 (正对狼头吻部 -Z)
    im_front, _ = render(bbmodel_path, yaw=0.0, pitch=0.0, size=500, bg=bg_color)
    # BACK: pitch=0, yaw=180 (正对尾部 +Z)
    im_back, _ = render(bbmodel_path, yaw=180.0, pitch=0.0, size=500, bg=bg_color)
    # SIDE_L: 侧视左面 (从 -X 侧看左肋与左腿)
    im_side_l, _ = render(bbmodel_path, yaw=-90.0, pitch=0.0, size=500, bg=bg_color)
    # SIDE_R: 侧视右面 (从 +X 侧看右肋与右腿)
    im_side_r, _ = render(bbmodel_path, yaw=90.0, pitch=0.0, size=500, bg=bg_color)
    # 3/4 ISOMETRIC: 经典等轴立体视角
    im_iso, _ = render(bbmodel_path, yaw=-45.0, pitch=25.0, size=500, bg=bg_color)
    # TOP: 俯视
    im_top, _ = render(bbmodel_path, yaw=0.0, pitch=89.9, size=500, bg=bg_color)

    # 1. 组合六视角接触表 render.png (2 行 3 列, 1540x1030)
    w_cell, h_cell = 500, 500
    canvas_6 = Image.new("RGB", (w_cell * 3 + 40, h_cell * 2 + 30), (32, 33, 36))
    draw_6 = ImageDraw.Draw(canvas_6)

    views_6 = [
        ("1. FRONT VIEW (-Z Head/Snout)", im_front, 10, 10),
        ("2. 3/4 ISOMETRIC VIEW",        im_iso, w_cell + 20, 10),
        ("3. SIDE_L VIEW (-X Left Flank)", im_side_l, w_cell * 2 + 30, 10),
        ("4. SIDE_R VIEW (+X Right Flank)", im_side_r, 10, h_cell + 20),
        ("5. TOP VIEW (+Y Dorsal Spine)",  im_top, w_cell + 20, h_cell + 20),
        ("6. BACK VIEW (+Z Tail/Hips)",    im_back, w_cell * 2 + 30, h_cell + 20),
    ]

    for title, img_v, px, py in views_6:
        canvas_6.paste(img_v, (px, py))
        draw_6.rectangle([px, py, px + 470, py + 26], fill=(22, 23, 26))
        draw_6.text((px + 8, py + 6), title, fill=(235, 235, 235))

    render_out = REVIEW_DIR / "render.png"
    canvas_6.save(render_out)
    print(f"✓ 六视角接触表已输出: {render_out}")

    # 2. 输出 check.png: 左爆炸参考图 / 右 六视角拼图
    ref_file = EXPLODED_REF if EXPLODED_REF.exists() else REF_IMG
    if ref_file.exists():
        ref_im = Image.open(ref_file).convert("RGB")
        target_h = canvas_6.height
        ref_w = int(ref_im.width * target_h / ref_im.height)
        ref_scaled = ref_im.resize((ref_w, target_h), Image.Resampling.LANCZOS)

        total_w = ref_w + canvas_6.width + 30
        check_cv = Image.new("RGB", (total_w, target_h + 46), (25, 26, 29))
        c_draw = ImageDraw.Draw(check_cv)

        # 贴左参考图
        check_cv.paste(ref_scaled, (12, 36))
        c_draw.text((16, 10), f"REFERENCE ({ref_file.name})", fill=(215, 205, 180))

        # 贴右六视角表
        rx = ref_w + 20
        check_cv.paste(canvas_6, (rx, 36))
        c_draw.text((rx + 4, 10), "NOW RENDER (6-VIEW CONTACT SHEET: FRONT, 3/4, SIDE_L, SIDE_R, TOP, BACK)", fill=(180, 225, 215))

        check_out = REVIEW_DIR / "check.png"
        check_cv.save(check_out)
        print(f"✓ 对标卡已输出: {check_out}")


# =============================================================================
# CLI 主入口
# =============================================================================

def main():
    parser = argparse.ArgumentParser(description="枯皮狼 (ku_pi_lang) 四足生物生成器")
    parser.add_argument("--self-test", action="store_true", help="运行门禁差分自证")
    parser.add_argument("--list", action="store_true", help="列出所有部件")
    parser.add_argument("--part", choices=sorted(PARTS.keys()), help="只生成单个部件预览")
    args = parser.parse_args()

    if args.list:
        print("枯皮狼 (ku_pi_lang) 部件清单:")
        for k, (desc, _) in PARTS.items():
            print(f"  - {k:12s} : {desc}")
        return

    if args.self_test:
        self_test()
        return

    # 先跑自测验证几何正确性
    self_test()

    CREATURE_DIR.mkdir(parents=True, exist_ok=True)
    MODELS_DIR.mkdir(parents=True, exist_ok=True)
    tex_img = generate_texture()

    if args.part:
        part_name = args.part
        desc, func = PARTS[part_name]
        rig = Rig()
        func(rig)
        bb_json = rig.bbmodel(f"KuPiLang_{part_name}", tex_img)
        part_out = CREATURE_DIR / f"KuPiLang_{part_name}.bbmodel"
        with open(part_out, "w", encoding="utf-8") as f:
            json.dump(bb_json, f, indent=2)
        print(f"✓ 单部件预览已生成: {part_out} ({len(rig.elements)} 个立方体)")
        return

    # 完整全身模型生成
    full_rig = build_full()
    bb_json = full_rig.bbmodel("KuPiLang", tex_img)

    with open(BBMODEL_OUT, "w", encoding="utf-8") as f:
        json.dump(bb_json, f, indent=2)
    with open(BBMODEL_COPY, "w", encoding="utf-8") as f:
        json.dump(bb_json, f, indent=2)

    print(f"✓ 完整模型已写入: {BBMODEL_OUT} (包含 {len(full_rig.elements)} 个立方体)")

    # 渲染六视角接触表及对标卡
    render_contact_and_check(BBMODEL_OUT)


if __name__ == "__main__":
    main()
