#!/usr/bin/env python3
"""残背驮兽（can_bei_tuo）Blockbench .bbmodel 程序化生成器 —— Round 1/3。

设计理念与世界观（正典依据）：
- 任务卡：末世生物批次 #4 can_bei_tuo 残背驮兽（§七 大迁徙）。
- 外形要点：
  低矮笨重的驮兽（介于牛和犀之间的粗壮体型，四短腿、宽背、垂头），
  背脊上长满一层层灰绿色噬灵藓块，兽潮主体，性情迟钝。
- 风格与意象：
  末法废土风格，灰褐苍老糙皮、厚实粗壮柱腿、下垂重头宽吻。
  背脊与肩驼峰上长满粗粝干枯的灰绿噬灵藓（阶梯叠块分布与藓瘤结节）。
- 骨骼架构：
  复用 dainu_lion / stitched_beast / fauna_v2 标准四足骨架规范：
  地面 y=0，头朝 -Z，尾朝 +Z。
  骨骼层级：root -> body (含 neck->head->jaw, tail)；四足 leg_fl/foot_fl, leg_fr/foot_fr, leg_bl/foot_bl, leg_br/foot_br。

用法：
    python3 modelScript/creatures/can_bei_tuo/gen_can_bei_tuo.py             # 生成完整模型与审阅图
    python3 modelScript/creatures/can_bei_tuo/gen_can_bei_tuo.py --part head # 单部件预览
    python3 modelScript/creatures/can_bei_tuo/gen_can_bei_tuo.py --self-test # 运行门禁差分自证
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import math
import random
import sys
import uuid
from pathlib import Path

from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[3]
CREATURE_DIR = Path(__file__).resolve().parent
MODELS_DIR = REPO / "modelScript" / "models"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/creatures/can_bei_tuo")

# 引入渲染工具
sys.path.insert(0, str(REPO / "modelScript" / "core"))
from bbmodel_maker.render.render_bbmodel import render  # noqa: E402

# ──────────────────────────────────────────────────────────────── 材质调色板
# 10 种末法废土生物材质，每种分配调色板色块
MAT_PALETTE = {
    "hide_base":    {"color": (92, 86, 78),   "noise": 8,  "desc": "灰褐泥土主皮色"},
    "hide_dark":    {"color": (76, 70, 64),   "noise": 8,  "desc": "暗土灰褐糙背皮"},
    "hide_belly":   {"color": (110, 102, 92), "noise": 6,  "desc": "腹侧磨浅糙皮"},
    "moss_layer1":  {"color": (76, 96, 74),   "noise": 10, "desc": "噬灵藓深层灰绿底菌"},
    "moss_layer2":  {"color": (98, 122, 94),  "noise": 10, "desc": "噬灵藓灰绿苔藓主体"},
    "moss_crust":   {"color": (124, 146, 116),"noise": 10, "desc": "噬灵藓表层老干苔藓/瘤斑"},
    "snout_horn":   {"color": (60, 55, 52),   "noise": 6,  "desc": "钝吻/低矮骨质鼻瘤"},
    "hoof_horn":    {"color": (46, 42, 40),   "noise": 5,  "desc": "粗壮脚蹄深角质"},
    "eye":          {"color": (38, 36, 35),   "noise": 3,  "desc": "浑浊呆滞小眼"},
    "socket":       {"color": (30, 28, 27),   "noise": 2,  "desc": "鼻孔与耳窝凹陷深影"},
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
def setup_bones(rig: CreatureRig) -> None:
    """定义残背驮兽标准四足骨架层级（复用 dainu_lion / stitched_beast 规范）。"""
    # 根节点
    rig.bone("root", (0.0, 0.0, 0.0), parent=None)

    # 躯干（以背部中段为枢轴，低矮厚重）
    rig.bone("body", (0.0, 13.0, 0.0), parent="root")

    # 粗短垂颈与重头
    rig.bone("neck", (0.0, 13.0, -10.0), parent="body")
    rig.bone("head", (0.0, 10.5, -16.0), parent="neck")
    rig.bone("jaw", (0.0, 6.0, -18.0), parent="head")

    # 短尾
    rig.bone("tail", (0.0, 13.5, 12.0), parent="body")

    # 四根粗柱短腿（直立承重，每肢拆为股部 + 小腿蹄块）
    # 前左肢
    rig.bone("leg_fl", (-6.5, 9.0, -7.0), parent="root")
    rig.bone("foot_fl", (-6.5, 4.5, -7.0), parent="leg_fl")

    # 前右肢
    rig.bone("leg_fr", (6.5, 9.0, -7.0), parent="root")
    rig.bone("foot_fr", (6.5, 4.5, -7.0), parent="leg_fr")

    # 后左肢
    rig.bone("leg_bl", (-6.5, 9.5, 7.5), parent="root")
    rig.bone("foot_bl", (-6.5, 4.5, 7.5), parent="leg_bl")

    # 后右肢
    rig.bone("leg_br", (6.5, 9.5, 7.5), parent="root")
    rig.bone("foot_br", (6.5, 4.5, 7.5), parent="leg_br")


# ──────────────────────────────────────────────────────────────── 分部件建模
def part_body(rig: CreatureRig) -> None:
    """1. 躯干部分：宽厚桶形胸肋、下腹部、肩驼峰前拱与圆阔臀部。"""
    # 躯干主核心
    rig.cube("body", "torso_core", (-7.0, 7.0, -9.0), (7.0, 16.2, 11.0), mat="hide_base")
    # 前胸厚肌面
    rig.cube("body", "chest_front", (-6.4, 7.8, -11.0), (6.4, 16.0, -9.1), mat="hide_dark")
    # 丰满左肋侧腹
    rig.cube("body", "belly_flank_l", (-8.4, 7.6, -6.0), (-7.1, 15.0, 8.0), mat="hide_base")
    # 丰满右肋侧腹
    rig.cube("body", "belly_flank_r", (7.1, 7.6, -6.0), (8.4, 15.0, 8.0), mat="hide_base")
    # 垂坠下腹底板（低矮底盘）
    rig.cube("body", "belly_under", (-5.8, 6.0, -5.0), (5.8, 6.9, 7.0), mat="hide_belly")
    # 臀部后倾圆面
    rig.cube("body", "hips_rear", (-6.4, 8.0, 11.1), (6.4, 16.0, 12.5), mat="hide_dark")
    # 肩背前拱驼峰（牛科典型前驼峰）
    rig.cube("body", "withers_hump", (-5.6, 16.3, -8.0), (5.6, 18.6, 0.7), mat="hide_dark")
    # 平缓腰背脊柱
    rig.cube("body", "back_spine", (-5.0, 16.35, 0.9), (5.0, 17.6, 10.8), mat="hide_dark")


def part_moss(rig: CreatureRig) -> None:
    """2. 背脊噬灵藓（长满一层层灰绿噬灵藓块与风化瘤节）。"""
    # ── 第 1 层：背脊宽厚灰绿基底 ──
    rig.cube("body", "moss_base_mid", (-5.8, 17.7, -7.0), (5.8, 18.8, 9.0), mat="moss_layer1")
    # 左侧垂挂藓边
    rig.cube("body", "moss_flank_l", (-6.8, 16.6, -5.0), (-5.9, 18.2, 7.0), mat="moss_layer1")
    # 右侧垂挂藓边
    rig.cube("body", "moss_flank_r", (5.9, 16.6, -5.0), (6.8, 18.2, 7.0), mat="moss_layer1")

    # ── 第 2 层：肩背驼峰厚苔块与腰背阶梯叠块 ──
    rig.cube("body", "moss_hump_layer", (-4.6, 18.9, -6.0), (4.6, 20.8, -0.2), mat="moss_layer2")
    rig.cube("body", "moss_rump_layer", (-4.2, 17.8, 1.2), (4.2, 19.5, 8.0), mat="moss_layer2")

    # ── 第 3 层：老干藓瘤与粗糙起伏结节 ──
    # 驼峰前隆起主藓瘤
    rig.cube("body", "moss_crest_front", (-2.6, 20.9, -5.0), (2.6, 22.4, -1.5), mat="moss_crust")
    # 驼峰顶端后藓块
    rig.cube("body", "moss_crest_mid", (-2.2, 20.95, -1.2), (2.2, 22.0, 1.0), mat="moss_crust")
    # 腰臀散布小瘤节 A
    rig.cube("body", "moss_crest_rear_l", (-3.2, 19.6, 2.5), (-0.6, 20.6, 5.0), mat="moss_crust")
    # 腰臀散布小瘤节 B
    rig.cube("body", "moss_crest_rear_r", (0.6, 19.6, 4.5), (3.0, 20.4, 7.2), mat="moss_crust")
    # 前肩侧生小藓斑
    rig.cube("body", "moss_nodule_front", (-3.6, 18.95, -8.4), (3.6, 19.8, -6.8), mat="moss_crust")


def part_neck(rig: CreatureRig) -> None:
    """3. 颈部：粗壮短颈，下倾支撑重头。"""
    # 颈主体向前下倾斜
    rig.cube("neck", "neck_main", (-4.8, 9.5, -15.0), (4.8, 15.8, -10.2), mat="hide_base")
    # 喉部下垂褶皮
    rig.cube("neck", "neck_throat", (-3.8, 8.0, -14.0), (3.8, 9.4, -10.5), mat="hide_belly")
    # 颈背厚皮垫
    rig.cube("neck", "neck_top", (-3.6, 15.9, -14.5), (3.6, 17.5, -10.4), mat="hide_dark")


def part_head(rig: CreatureRig) -> None:
    """4. 头部与下颌：垂头、宽钝吻部、低矮骨瘤、呆滞小眼与厚下颌。"""
    # 沉重下垂颅骨
    rig.cube("head", "skull_box", (-4.2, 7.5, -22.0), (4.2, 13.5, -15.2), mat="hide_base")
    # 平阔前额斜面
    rig.cube("head", "forehead_slope", (-3.6, 9.0, -20.0), (3.6, 14.2, -16.0), mat="hide_dark")
    # 宽方厚实上吻
    rig.cube("head", "snout_upper", (-3.6, 6.2, -27.2), (3.6, 11.2, -22.2), mat="hide_dark")
    # 钝鼻镜端面
    rig.cube("head", "snout_pad", (-3.2, 6.5, -27.9), (3.2, 10.5, -27.3), mat="snout_horn")
    # 鼻梁低矮磨秃粗骨瘤（牛犀交融特征）
    rig.cube("head", "snout_horn_boss", (-1.4, 11.3, -25.5), (1.4, 12.8, -23.0), mat="snout_horn")
    # 左鼻孔深面
    rig.cube("head", "nostril_l", (-2.6, 6.8, -28.2), (-1.0, 8.2, -28.0), mat="socket")
    # 右鼻孔深面
    rig.cube("head", "nostril_r", (1.0, 6.8, -28.2), (2.6, 8.2, -28.0), mat="socket")
    # 呆滞小眼框（深凹内嵌）
    rig.cube("head", "eye_l", (-4.4, 10.2, -19.5), (-4.25, 11.8, -17.5), mat="eye")
    rig.cube("head", "eye_r", (4.25, 10.2, -19.5), (4.4, 11.8, -17.5), mat="eye")
    # 左圆钝下垂厚耳
    rig.cube("head", "ear_l", (-5.8, 10.8, -16.5), (-4.25, 13.2, -14.2), mat="hide_dark")
    # 右圆钝下垂厚耳
    rig.cube("head", "ear_r", (4.25, 10.8, -16.5), (5.8, 13.2, -14.2), mat="hide_dark")

    # 下颌主体
    rig.cube("jaw", "jaw_main", (-3.2, 4.2, -26.0), (3.2, 6.1, -18.0), mat="hide_base")
    # 钝下巴前端
    rig.cube("jaw", "jaw_chin", (-2.6, 4.5, -26.7), (2.6, 6.0, -25.9), mat="hide_dark")


def part_legs(rig: CreatureRig) -> None:
    """5. 四肢短腿：四根粗短柱状腿，宽厚圆蹄，站立扎实。"""
    # ── 前左肢 (FL) ──
    rig.cube("leg_fl", "leg_fl_thigh", (-8.8, 4.5, -9.2), (-4.4, 11.0, -4.8), mat="hide_dark")
    rig.cube("foot_fl", "foot_fl_shank", (-8.5, 1.5, -8.9), (-4.7, 4.8, -5.1), mat="hide_base")
    rig.cube("foot_fl", "foot_fl_hoof_cushion", (-8.9, 0.0, -9.3), (-4.3, 1.8, -4.7), mat="hoof_horn")

    # ── 前右肢 (FR) ──
    rig.cube("leg_fr", "leg_fr_thigh", (4.4, 4.5, -9.2), (8.8, 11.0, -4.8), mat="hide_dark")
    rig.cube("foot_fr", "foot_fr_shank", (4.7, 1.5, -8.9), (8.5, 4.8, -5.1), mat="hide_base")
    rig.cube("foot_fr", "foot_fr_hoof_cushion", (4.3, 0.0, -9.3), (8.9, 1.8, -4.7), mat="hoof_horn")

    # ── 后左肢 (BL) ──
    rig.cube("leg_bl", "leg_bl_thigh", (-8.8, 4.5, 5.2), (-4.4, 11.5, 9.8), mat="hide_dark")
    rig.cube("foot_bl", "foot_bl_shank", (-8.5, 1.5, 5.5), (-4.7, 4.8, 9.5), mat="hide_base")
    rig.cube("foot_bl", "foot_bl_hoof_cushion", (-8.9, 0.0, 5.1), (-4.3, 1.8, 9.9), mat="hoof_horn")

    # ── 后右肢 (BR) ──
    rig.cube("leg_br", "leg_br_thigh", (4.4, 4.5, 5.2), (8.8, 11.5, 9.8), mat="hide_dark")
    rig.cube("foot_br", "foot_br_shank", (4.7, 1.5, 5.5), (8.5, 4.8, 9.5), mat="hide_base")
    rig.cube("foot_br", "foot_br_hoof_cushion", (4.3, 0.0, 5.1), (8.9, 1.8, 9.9), mat="hoof_horn")


def part_tail(rig: CreatureRig) -> None:
    """6. 短粗尾：下垂短尾与末端厚毛簇。"""
    rig.cube("tail", "tail_stem", (-1.2, 8.5, 12.6), (1.2, 13.5, 14.2), mat="hide_base")
    rig.cube("tail", "tail_tuft", (-1.6, 6.0, 12.8), (1.6, 8.4, 14.8), mat="hide_dark")


def build_rig(part: str = "all") -> CreatureRig:
    """根据指定的部件名称构建 Rig。"""
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
    assert_no_coplanar_faces(rig.elements)
    print(f"  [OK] 正常几何体集合 ({len(rig.elements)} 个立方体) 0 项共面冲突")

    # 注入缺陷测试
    defect_elements = list(rig.elements) + [{
        "name": "defect_coplanar_fail",
        "from": [-7.0, 7.0, -9.0],
        "to": [7.0, 16.2, 11.0],  # 完全与 torso_core 重合
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
def render_contact_sheet(bbmodel_path: Path) -> tuple[Image.Image, dict[str, Image.Image]]:
    """渲染六视角接触表（诚实命名视角）。"""
    bg = (119, 119, 119)  # 严格对齐参考图中性灰
    views = [
        ("FRONT",  0.0,   0.0, "正视 (从正面 -Z 直视吻部与前肢)"),
        ("SIDE_R", 90.0,  0.0, "右侧视 (从 +X 直视右侧体态与背脊藓块)"),
        ("BACK",   180.0, 0.0, "后视 (从后方 +Z 直视臀部、后肢与尾部)"),
        ("SIDE_L", -90.0, 0.0, "左侧视 (从 -X 直视左侧体态与驼峰)"),
        ("3/4",    -35.0, 22.0,"等轴 3/4 视 (前左侧立体观察)"),
        ("TOP",    0.0,   89.9,"俯视 (从正上方俯瞰宽背与噬灵藓分布)"),
    ]

    rendered_images: dict[str, Image.Image] = {}
    tile_size = 420
    for name, yaw, pitch, desc in views:
        im, _ = render(bbmodel_path, yaw=yaw, pitch=pitch, size=tile_size, bg=bg)
        rendered_images[name] = im

    # 拼成 2 行 3 列六视角大接触表
    cols, rows = 3, 2
    pad = 16
    margin = 24
    header_h = 36
    label_h = 24

    total_w = margin * 2 + cols * tile_size + (cols - 1) * pad
    total_h = margin * 2 + header_h + rows * (tile_size + label_h + pad)

    sheet = Image.new("RGB", (total_w, total_h), (35, 36, 40))
    draw = ImageDraw.Draw(sheet)
    draw.text((margin, 12), "can_bei_tuo (残背驮兽) 六视角接触表 (Honest 6-View Contact Sheet)", fill=(235, 235, 235))

    for idx, (name, _, _, desc) in enumerate(views):
        r = idx // cols
        c = idx % cols
        px = margin + c * (tile_size + pad)
        py = margin + header_h + r * (tile_size + label_h + pad)

        sheet.paste(rendered_images[name], (px, py))
        draw.rectangle([px, py + tile_size, px + tile_size, py + tile_size + label_h], fill=(24, 25, 28))
        draw.text((px + 6, py + tile_size + 4), f"{name}: {desc}", fill=(210, 215, 220))

    return sheet, rendered_images


def make_check_sheet(ref_path: Path, exploded_path: Path, contact_sheet: Image.Image, rendered_views: dict[str, Image.Image]) -> Image.Image:
    """生成并排对照卡：左侧堆叠概念参考图与爆炸分解图，右侧六视角接触表。"""
    ref_im = Image.open(ref_path).convert("RGB")
    exp_im = Image.open(exploded_path).convert("RGB") if exploded_path.exists() else None

    target_h = contact_sheet.height
    header_h = 36
    gap = 20

    if exp_im is not None:
        # 左侧两图上下堆叠
        sub_h = (target_h - gap) // 2
        sub_w = int(ref_im.width * sub_h / ref_im.height)

        ref_scaled = ref_im.resize((sub_w, sub_h), Image.Resampling.LANCZOS)
        exp_scaled = exp_im.resize((sub_w, sub_h), Image.Resampling.LANCZOS)

        left_w = sub_w
        right_w = contact_sheet.width

        total_w = left_w + right_w + gap * 3
        total_h = target_h + header_h + 24

        canvas = Image.new("RGB", (total_w, total_h), (28, 29, 33))
        draw = ImageDraw.Draw(canvas)

        draw.text((gap, 10), "REFERENCES: Top: Concept Art | Bottom: Exploded Schematic", fill=(215, 205, 185))
        draw.text((left_w + gap * 2, 10), "NOW 3D VOXEL MODEL: Honest 6-View Contact Sheet (FRONT / SIDE / BACK / 3/4 / TOP)", fill=(185, 220, 215))

        canvas.paste(ref_scaled, (gap, header_h))
        canvas.paste(exp_scaled, (gap, header_h + sub_h + gap))
        canvas.paste(contact_sheet, (left_w + gap * 2, header_h))
    else:
        ref_w = int(ref_im.width * target_h / ref_im.height)
        ref_scaled = ref_im.resize((ref_w, target_h), Image.Resampling.LANCZOS)

        total_w = ref_w + contact_sheet.width + gap * 3
        total_h = target_h + 48

        canvas = Image.new("RGB", (total_w, total_h), (28, 29, 33))
        draw = ImageDraw.Draw(canvas)

        draw.text((gap, 12), "CONCEPT REFERENCE (can_bei_tuo_ref.png)", fill=(210, 200, 180))
        draw.text((ref_w + gap * 2, 12), "NOW 3D VOXEL MODEL (6-View Honest Contact Sheet)", fill=(180, 220, 210))

        canvas.paste(ref_scaled, (gap, 36))
        canvas.paste(contact_sheet, (ref_w + gap * 2, 36))

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
    tex_img = generate_palette_texture()
    buf = io.BytesIO()
    tex_img.save(buf, format="PNG")
    tex_b64 = base64.b64encode(buf.getvalue()).decode("ascii")

    rig = build_rig(args.part)
    model_name = "CanBeiTuo" if args.part == "all" else f"CanBeiTuo_{args.part}"
    model_dict = rig.bbmodel(model_name, tex_b64)

    # 3. 保存模型文件
    CREATURE_DIR.mkdir(parents=True, exist_ok=True)
    MODELS_DIR.mkdir(parents=True, exist_ok=True)

    out_local = CREATURE_DIR / f"{model_name}.bbmodel"
    out_models = MODELS_DIR / f"{model_name}.bbmodel"

    model_json = json.dumps(model_dict, ensure_ascii=False, indent=2)
    out_local.write_text(model_json, encoding="utf-8")
    out_models.write_text(model_json, encoding="utf-8")
    print(f"✓ Blockbench 模型已生成: {out_local}")
    print(f"✓ 仓库模型目录副本已生成: {out_models}")

    # 4. 渲染审阅图
    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    print("正在渲染六视角接触表...")
    contact_sheet, rendered_views = render_contact_sheet(out_local)

    contact_path = REVIEW_DIR / "contact_sheet.png"
    contact_sheet.save(contact_path)
    print(f"✓ 六视角接触表已保存: {contact_path}")

    # 保存各独立视角图
    views_dir = REVIEW_DIR / "views"
    views_dir.mkdir(parents=True, exist_ok=True)
    for vname, vim in rendered_views.items():
        safe_name = vname.lower().replace("/", "_")
        vpath = views_dir / f"{safe_name}.png"
        vim.save(vpath)

    # 5. 生成 check.png 对照图
    ref_img_path = REVIEW_DIR / "can_bei_tuo_ref.png"
    exp_img_path = REVIEW_DIR / "can_bei_tuo_exploded.png"
    if ref_img_path.exists():
        print("正在生成与参考图并排对照卡 check.png...")
        check_sheet = make_check_sheet(ref_img_path, exp_img_path, contact_sheet, rendered_views)
        check_path = REVIEW_DIR / "check.png"
        check_sheet.save(check_path)
        print(f"✓ 并排对照卡 check.png 已保存: {check_path}")
    else:
        print(f"提示: 未找到 {ref_img_path}，跳过 check.png 生成")


if __name__ == "__main__":
    main()
