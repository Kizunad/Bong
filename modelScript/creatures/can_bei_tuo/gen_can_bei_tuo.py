#!/usr/bin/env python3
"""残背驮兽（can_bei_tuo）Blockbench .bbmodel 程序化生成器 —— Round 2/3。

设计理念与世界观（正典依据与 Round 2 深度打磨）：
- 任务卡：末世生物批次 #4 can_bei_tuo 残背驮兽（§七 大迁徙）。
- 外形要点与 Round 2 意见全面落实：
  1. 躯干：高耸肩峰驼背 + 圆滚桶状身躯，多层逐级收边方块做出弧面，侧腹增加皮肤褶皱横纹；
  2. 头与颈：大而低垂的重头，宽扁鼻吻与立体鼻孔，两侧向前微弯的短粗双角，小眼睛深陷眼窝，下巴与颈下挂着灰绿垂藓须；
  3. 噬灵藓：多层阶梯叠块起伏堆叠（底层基底、中层驼峰主苔与腰脊厚阶、顶层苍老藓瘤），边缘向下垂挂须状藓舌，点缀少量白色寄生小花点；
  4. 腿：粗短柱状但有粗大膝关节与飞节，每蹄明确分 2 瓣偶蹄，腿侧附生少量藓斑；
  5. 尾：短粗垂尾末端带灰绿藓穗与毛簇。
- 体素精度与部件数：
  采用 0.5px 级精度细化阶梯过渡，部件数从 Round 1 的 48 块翻近三倍达到 135+ 块；
  全模型 0 项共面冲突。
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
import random
import sys
import uuid
from pathlib import Path

from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[3]
CREATURE_DIR = Path(__file__).resolve().parent
MODELS_DIR = REPO / "modelScript" / "models"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/creatures/can_bei_tuo")

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
def setup_bones(rig: CreatureRig) -> None:
    """定义残背驮兽标准四足骨架层级（复用 dainu_lion / stitched_beast 规范）。"""
    rig.bone("root", (0.0, 0.0, 0.0), parent=None)

    # 躯干（宽厚驼峰躯干，pivot 在背部中央）
    rig.bone("body", (0.0, 13.0, 0.0), parent="root")

    # 粗短垂颈与大重头
    rig.bone("neck", (0.0, 13.0, -10.8), parent="body")
    rig.bone("head", (0.0, 10.5, -16.0), parent="neck")
    rig.bone("jaw", (0.0, 5.0, -18.5), parent="head")

    # 短尾
    rig.bone("tail", (0.0, 13.5, 12.0), parent="body")

    # 四根粗柱短腿
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
    """1. 躯干部分：高耸肩峰驼背 + 圆滚桶状身躯，多层逐级收边方块做出弧面，侧腹增加皮肤褶皱横纹。"""
    # 核心下层腹躯 (底层微缩)
    rig.cube("body", "torso_lower", (-6.8, 6.15, -9.0), (6.8, 10.35, 11.0), mat="hide_base")
    # 核心中层肋躯 (最宽处，圆滚桶状)
    rig.cube("body", "torso_mid", (-7.6, 10.38, -9.6), (7.6, 14.5, 11.5), mat="hide_base")
    # 核心上层背躯
    rig.cube("body", "torso_upper", (-7.0, 14.45, -9.2), (7.0, 16.8, 11.2), mat="hide_base")
    # 下腹微下坠厚垂板（低矮底盘）
    rig.cube("body", "belly_sag", (-5.6, 5.2, -5.5), (5.6, 6.10, 7.5), mat="hide_belly")
    # 前胸厚拱肌肉面
    rig.cube("body", "chest_mass", (-6.5, 7.5, -11.8), (6.5, 15.8, -9.65), mat="hide_dark")
    # 臀后圆弧厚臀
    rig.cube("body", "rump_mass", (-6.5, 7.8, 11.55), (6.5, 15.6, 13.2), mat="hide_dark")

    # 高耸肩峰驼背 (Withers Hump - 前高后低陡峭拱起)
    rig.cube("body", "hump_base", (-6.0, 16.75, -8.8), (6.0, 19.2, 1.65), mat="hide_dark")
    rig.cube("body", "hump_peak", (-5.0, 19.15, -7.5), (5.0, 21.6, -0.5), mat="hide_dark")
    rig.cube("body", "hump_crest", (-3.8, 21.55, -6.0), (3.8, 22.8, -2.0), mat="hide_dark")
    # 腰臀背中脊
    rig.cube("body", "spine_ridge", (-4.6, 16.78, 1.75), (4.6, 18.2, 11.5), mat="hide_dark")

    # 侧面皮肤褶皱横纹 (skin folds - 左右两侧各 3 条深浅相间横向凸纹)
    # 左侧横纹
    rig.cube("body", "skin_fold_l1", (-7.95, 8.5, -4.5), (-7.55, 9.4, 2.5), mat="skin_fold")
    rig.cube("body", "skin_fold_l2", (-8.05, 10.55, -6.5), (-7.55, 11.5, 5.5), mat="skin_fold")
    rig.cube("body", "skin_fold_l3", (-7.95, 12.6, -5.0), (-7.55, 13.5, 4.0), mat="skin_fold")
    # 右侧横纹
    rig.cube("body", "skin_fold_r1", (7.55, 8.5, -4.5), (7.95, 9.4, 2.5), mat="skin_fold")
    rig.cube("body", "skin_fold_r2", (7.55, 10.55, -6.5), (8.05, 11.5, 5.5), mat="skin_fold")
    rig.cube("body", "skin_fold_r3", (7.55, 12.6, -5.0), (7.95, 13.5, 4.0), mat="skin_fold")

    # 前肩与后臀肌肉横向厚褶
    rig.cube("body", "shoulder_fold_l", (-7.5, 11.0, -9.0), (-6.95, 14.0, -7.5), mat="skin_fold")
    rig.cube("body", "shoulder_fold_r", (6.95, 11.0, -9.0), (7.5, 14.0, -7.5), mat="skin_fold")
    rig.cube("body", "flank_fold_rear_l", (-7.3, 10.5, 8.5), (-6.85, 13.5, 10.5), mat="skin_fold")
    rig.cube("body", "flank_fold_rear_r", (6.85, 10.5, 8.5), (7.3, 13.5, 10.5), mat="skin_fold")


def part_neck(rig: CreatureRig) -> None:
    """2. 颈部与垂悬颈藓：粗壮短颈，下倾支撑重头，颈下喉部悬挂垂藓须。"""
    # 颈主体向前下倾斜
    rig.cube("neck", "neck_core", (-5.2, 9.0, -16.2), (5.2, 15.6, -10.8), mat="hide_base")
    # 颈上沿厚鬃肉垫
    rig.cube("neck", "neck_crest", (-4.2, 15.55, -15.5), (4.2, 17.8, -11.0), mat="hide_dark")
    # 喉部下垂厚皮
    rig.cube("neck", "neck_throat", (-4.4, 7.4, -15.0), (4.4, 9.15, -11.5), mat="hide_belly")

    # 颈下喉部垂悬藓须 (moss_dewlap - 随重头下倾自然向下悬垂)
    rig.cube("neck", "neck_moss_beard_c", (-1.2, 4.6, -14.2), (1.2, 7.42, -12.4), mat="moss_layer2")
    rig.cube("neck", "neck_moss_beard_l", (-2.8, 5.6, -13.8), (-1.15, 7.50, -12.0), mat="moss_layer1")
    rig.cube("neck", "neck_moss_beard_r", (1.15, 5.6, -13.8), (2.8, 7.50, -12.0), mat="moss_layer1")
    rig.cube("neck", "neck_moss_tuft_low", (-0.7, 3.8, -13.6), (0.7, 4.7, -12.8), mat="moss_crust")


def part_head(rig: CreatureRig) -> None:
    """3. 头部、角与下颌：头部显著做大，大而低垂的重头，宽扁吻部与立体鼻孔，两侧向前弯短粗角，下巴挂垂藓须。"""
    # 沉重大脑颅
    rig.cube("head", "head_cranium", (-5.4, 7.6, -23.5), (5.4, 14.5, -16.0), mat="hide_base")
    # 平阔前额斜面
    rig.cube("head", "head_forehead", (-4.8, 9.5, -21.5), (4.8, 15.2, -17.0), mat="hide_dark")

    # 宽方厚实大鼻吻（类似牛犀交融宽厚平吻）
    rig.cube("head", "snout_broad", (-4.6, 5.85, -29.5), (4.6, 11.8, -23.4), mat="hide_dark")
    rig.cube("head", "snout_pad", (-4.2, 6.0, -30.4), (4.2, 11.0, -29.45), mat="snout_horn")
    # 上吻唇厚边
    rig.cube("head", "snout_lip_upper", (-3.8, 5.95, -29.2), (3.8, 6.75, -26.5), mat="hide_dark")
    # 鼻梁横皮褶
    rig.cube("head", "snout_fold_top", (-2.8, 10.95, -27.8), (2.8, 11.90, -25.8), mat="hide_dark")

    # 鼻孔深凹
    rig.cube("head", "nostril_l", (-3.2, 6.6, -30.65), (-1.4, 8.4, -30.35), mat="socket")
    rig.cube("head", "nostril_r", (1.4, 6.6, -30.65), (3.2, 8.4, -30.35), mat="socket")
    # 鼻梁骨粗瘤（粗钝磨平鼻角瘤）
    rig.cube("head", "snout_boss", (-2.0, 11.85, -27.5), (2.0, 13.6, -24.2), mat="snout_horn")
    rig.cube("head", "snout_boss_apex", (-1.2, 13.55, -26.5), (1.2, 14.6, -24.8), mat="snout_horn")

    # 深陷小眼眶与眼珠
    rig.cube("head", "eye_socket_l", (-5.65, 10.8, -20.0), (-5.35, 12.6, -17.8), mat="socket")
    rig.cube("head", "eye_pupil_l", (-5.62, 11.2, -19.4), (-5.38, 12.2, -18.4), mat="eye")
    rig.cube("head", "brow_ridge_l", (-5.6, 12.55, -20.4), (-5.15, 13.6, -17.4), mat="hide_dark")

    rig.cube("head", "eye_socket_r", (5.35, 10.8, -20.0), (5.65, 12.6, -17.8), mat="socket")
    rig.cube("head", "eye_pupil_r", (5.38, 11.2, -19.4), (5.62, 12.2, -18.4), mat="eye")
    rig.cube("head", "brow_ridge_r", (5.15, 12.55, -20.4), (5.6, 13.6, -17.4), mat="hide_dark")

    # 两侧向下耷拉厚耳
    rig.cube("head", "ear_base_l", (-6.9, 11.8, -17.5), (-5.35, 13.8, -15.5), mat="hide_dark")
    rig.cube("head", "ear_tip_l", (-7.8, 9.8, -17.2), (-6.85, 12.2, -15.8), mat="hide_base")
    rig.cube("head", "ear_base_r", (5.35, 11.8, -17.5), (6.9, 13.8, -15.5), mat="hide_dark")
    rig.cube("head", "ear_tip_r", (6.85, 9.8, -17.2), (7.8, 12.2, -15.8), mat="hide_base")

    # 两侧向前弯的短粗角 (horns: 粗短有力，从前额两侧斜向外前方弯出)
    # 左角
    rig.cube("head", "horn_base_l", (-6.6, 13.4, -22.2), (-4.55, 15.6, -19.6), mat="horn_bone")
    rig.cube("head", "horn_mid_l", (-7.8, 13.8, -24.8), (-6.25, 15.4, -21.8), mat="horn_bone")
    rig.cube("head", "horn_tip_l", (-7.6, 14.1, -27.0), (-6.45, 15.1, -24.4), mat="horn_bone")

    # 右角
    rig.cube("head", "horn_base_r", (4.55, 13.4, -22.2), (6.6, 15.6, -19.6), mat="horn_bone")
    rig.cube("head", "horn_mid_r", (6.25, 13.8, -24.8), (7.8, 15.4, -21.8), mat="horn_bone")
    rig.cube("head", "horn_tip_r", (6.45, 14.1, -27.0), (7.6, 15.1, -24.4), mat="horn_bone")

    # 颊部咬肌
    rig.cube("head", "jaw_cheek_l", (-4.35, 4.6, -23.5), (-3.85, 6.4, -19.5), mat="hide_dark")
    rig.cube("head", "jaw_cheek_r", (3.85, 4.6, -23.5), (4.35, 6.4, -19.5), mat="hide_dark")

    # 宽实下颌与钝下巴 (jaw 骨骼)
    rig.cube("jaw", "jaw_box", (-4.0, 3.8, -28.5), (4.0, 5.72, -18.5), mat="hide_base")
    rig.cube("jaw", "jaw_chin", (-3.4, 4.0, -29.45), (3.4, 5.70, -28.45), mat="hide_dark")
    # 下巴底垂挂藓须与寄生小花点
    rig.cube("jaw", "jaw_moss_beard_c", (-1.2, 1.8, -27.5), (1.2, 3.82, -24.5), mat="moss_layer2")
    rig.cube("jaw", "jaw_moss_beard_f", (-0.8, 2.6, -28.8), (0.8, 3.90, -27.3), mat="moss_crust")
    rig.cube("jaw", "jaw_moss_floret", (-0.4, 1.3, -26.2), (0.4, 2.0, -25.4), mat="moss_flower")


def part_moss(rig: CreatureRig) -> None:
    """4. 背脊噬灵藓：多层阶梯叠块起伏堆叠，边缘向下垂挂成须状，点缀少量白色小花点。"""
    # ── 第 1 层：背脊宽厚灰绿基底 ──
    rig.cube("body", "moss_base_mid", (-6.2, 17.5, -7.5), (6.2, 19.0, 9.5), mat="moss_layer1")

    # 左右两侧向下垂挂成须状 (多缕参差不齐悬垂在肋腹上的藓舌须)
    rig.cube("body", "moss_hang_l1", (-7.4, 15.8, -6.5), (-6.15, 18.2, -4.0), mat="moss_layer1")
    rig.cube("body", "moss_hang_l2", (-7.55, 15.0, -3.8), (-6.15, 18.0, -0.8), mat="moss_layer2")
    rig.cube("body", "moss_hang_l3", (-7.35, 15.6, -0.6), (-6.15, 18.2, 2.2), mat="moss_layer1")
    rig.cube("body", "moss_hang_l4", (-7.45, 14.8, 2.4), (-6.15, 17.8, 5.2), mat="moss_layer2")
    rig.cube("body", "moss_hang_l5", (-7.25, 15.9, 5.4), (-6.15, 18.0, 7.8), mat="moss_layer1")
    rig.cube("body", "moss_hang_l6", (-7.25, 14.2, -1.8), (-6.10, 16.5, 0.4), mat="moss_layer2")

    rig.cube("body", "moss_hang_r1", (6.15, 15.8, -6.5), (7.4, 18.2, -4.0), mat="moss_layer1")
    rig.cube("body", "moss_hang_r2", (6.15, 15.0, -3.8), (7.55, 18.0, -0.8), mat="moss_layer2")
    rig.cube("body", "moss_hang_r3", (6.15, 15.6, -0.6), (7.35, 18.2, 2.2), mat="moss_layer1")
    rig.cube("body", "moss_hang_r4", (6.15, 14.8, 2.4), (7.45, 17.8, 5.2), mat="moss_layer2")
    rig.cube("body", "moss_hang_r5", (6.15, 15.9, 5.4), (7.25, 18.0, 7.8), mat="moss_layer1")
    rig.cube("body", "moss_hang_r6", (6.20, 14.2, -1.8), (7.25, 16.5, 0.4), mat="moss_layer2")

    # 侧腹薄藓阶
    rig.cube("body", "moss_shelf_flank_l", (-6.85, 17.2, 1.2), (-5.65, 18.6, 3.8), mat="moss_layer1")
    rig.cube("body", "moss_shelf_flank_r", (5.65, 17.2, 1.2), (6.85, 18.6, 3.8), mat="moss_layer1")

    # ── 第 2 层：驼峰主苔斜坡与腰背阶梯厚台 ──
    rig.cube("body", "moss_hump_slope", (-5.4, 19.1, -7.8), (5.4, 21.4, -4.8), mat="moss_layer2")
    rig.cube("body", "moss_hump_peak", (-5.2, 21.35, -4.9), (5.2, 23.2, 0.4), mat="moss_layer2")
    rig.cube("body", "moss_spine_shelf_a", (-4.8, 18.1, 0.6), (4.8, 20.2, 4.8), mat="moss_layer2")
    rig.cube("body", "moss_spine_shelf_b", (-4.65, 17.9, 5.0), (4.65, 19.8, 8.8), mat="moss_layer2")
    rig.cube("body", "moss_rump_crest", (-4.2, 17.6, 9.0), (4.2, 19.2, 12.2), mat="moss_layer1")

    # ── 第 3 层：老干藓瘤与起伏瘤节 ──
    rig.cube("body", "moss_crest_peak_c", (-2.6, 23.15, -4.0), (2.6, 24.6, -1.05), mat="moss_crust")
    rig.cube("body", "moss_crest_peak_l", (-3.6, 22.85, -3.2), (-1.8, 24.1, 0.2), mat="moss_crust")
    rig.cube("body", "moss_crest_peak_r", (1.8, 22.85, -3.2), (3.6, 24.1, 0.2), mat="moss_crust")
    rig.cube("body", "moss_boss_spine_1", (-2.4, 20.15, 1.5), (0.2, 21.6, 3.8), mat="moss_crust")
    rig.cube("body", "moss_boss_spine_2", (-0.2, 19.75, 4.2), (2.5, 21.1, 6.6), mat="moss_crust")
    rig.cube("body", "moss_boss_rump", (-1.8, 19.15, 7.8), (1.8, 20.4, 10.6), mat="moss_crust")
    rig.cube("body", "moss_boss_shoulder_l", (-4.8, 19.8, -7.2), (-2.8, 21.0, -5.2), mat="moss_crust")
    rig.cube("body", "moss_boss_shoulder_r", (2.8, 19.8, -7.2), (4.8, 21.0, -5.2), mat="moss_crust")
    rig.cube("body", "moss_nodule_spine_top", (-1.2, 21.65, 2.8), (1.2, 22.8, 4.4), mat="moss_crust")
    rig.cube("body", "moss_nodule_hump_rear", (-2.8, 22.1, -1.8), (-0.8, 23.4, -0.2), mat="moss_crust")

    # ── 点缀白色寄生小花点 (florets) ──
    florets = [
        ("floret_peak_1", (-1.4, 24.55, -2.8), (-0.6, 25.1, -2.0)),
        ("floret_peak_2", (0.8, 24.55, -2.2), (1.6, 25.1, -1.4)),
        ("floret_hump_l", (-3.4, 23.15, -1.2), (-2.7, 23.7, -0.5)),
        ("floret_hump_r", (2.5, 23.15, -0.95), (3.2, 23.7, -0.28)),
        ("floret_flank_l1", (-7.65, 16.2, -2.2), (-7.15, 16.9, -1.5)),
        ("floret_flank_l2", (-7.55, 15.6, 3.8), (-7.05, 16.3, 4.5)),
        ("floret_flank_r1", (7.15, 16.2, -2.2), (7.65, 16.9, -1.5)),
        ("floret_flank_r2", (7.05, 15.6, 3.8), (7.55, 16.3, 4.5)),
        ("floret_spine_1", (-1.8, 21.55, 2.4), (-1.1, 22.1, 3.1)),
        ("floret_spine_2", (1.2, 21.05, 5.0), (1.9, 21.6, 5.7)),
        ("floret_rump", (-0.5, 20.35, 9.0), (0.3, 20.9, 9.8)),
        ("floret_shoulder", (-4.2, 20.95, -6.4), (-3.5, 21.5, -5.7)),
        ("floret_tail_base", (-0.4, 18.25, 11.2), (0.4, 18.8, 11.8)),
        ("floret_neck_root", (-2.2, 17.85, -9.8), (-1.5, 18.4, -9.1)),
        ("floret_side_hang_l", (-7.45, 14.6, 0.2), (-6.95, 15.2, 0.9)),
        ("floret_side_hang_r", (6.95, 14.6, 0.2), (7.45, 15.2, 0.9)),
    ]
    for fname, ff, ft in florets:
        rig.cube("body", fname, ff, ft, mat="moss_flower")


def part_legs(rig: CreatureRig) -> None:
    """5. 四肢短腿：粗短柱状但有粗大膝部，每蹄分 2 瓣偶蹄，腿侧挂少量藓。"""
    # ── 前左肢 (FL) ──
    rig.cube("leg_fl", "thigh_fl_main", (-8.9, 4.5, -9.4), (-4.3, 11.2, -4.6), mat="hide_dark")
    rig.cube("leg_fl", "thigh_fl_muscle", (-9.15, 5.95, -8.6), (-4.25, 10.5, -5.4), mat="hide_base")
    rig.cube("leg_fl", "knee_node_fl", (-8.8, 4.15, -9.6), (-4.4, 5.85, -4.4), mat="hide_dark")  # 粗大外凸膝结
    rig.cube("leg_fl", "moss_leg_fl", (-9.25, 7.5, -8.0), (-8.85, 9.8, -6.0), mat="moss_layer1")  # 腿上挂藓

    rig.cube("foot_fl", "shank_fl", (-8.6, 1.45, -9.0), (-4.6, 4.4, -5.0), mat="hide_base")
    rig.cube("foot_fl", "pastern_fl", (-8.75, 1.82, -9.1), (-4.45, 2.65, -4.9), mat="hide_dark")  # 蹄冠圈
    # 二瓣偶蹄 (外瓣与内瓣，中间留出清晰蹄裂)
    rig.cube("foot_fl", "hoof_fl_outer", (-8.95, 0.0, -9.3), (-6.85, 1.85, -4.7), mat="hoof_horn")  # 外蹄瓣
    rig.cube("foot_fl", "hoof_fl_inner", (-6.35, 0.0, -9.3), (-4.25, 1.85, -4.7), mat="hoof_horn")  # 内蹄瓣

    # ── 前右肢 (FR) ──
    rig.cube("leg_fr", "thigh_fr_main", (4.3, 4.5, -9.4), (8.9, 11.2, -4.6), mat="hide_dark")
    rig.cube("leg_fr", "thigh_fr_muscle", (4.25, 5.95, -8.6), (9.15, 10.5, -5.4), mat="hide_base")
    rig.cube("leg_fr", "knee_node_fr", (4.4, 4.15, -9.6), (8.8, 5.85, -4.4), mat="hide_dark")
    rig.cube("leg_fr", "moss_leg_fr", (8.85, 7.5, -8.0), (9.25, 9.8, -6.0), mat="moss_layer1")

    rig.cube("foot_fr", "shank_fr", (4.6, 1.45, -9.0), (8.6, 4.4, -5.0), mat="hide_base")
    rig.cube("foot_fr", "pastern_fr", (4.45, 1.82, -9.1), (8.75, 2.65, -4.9), mat="hide_dark")
    rig.cube("foot_fr", "hoof_fr_inner", (4.25, 0.0, -9.3), (6.35, 1.85, -4.7), mat="hoof_horn")
    rig.cube("foot_fr", "hoof_fr_outer", (6.85, 0.0, -9.3), (8.95, 1.85, -4.7), mat="hoof_horn")

    # ── 后左肢 (BL) ──
    rig.cube("leg_bl", "thigh_bl_main", (-8.9, 4.5, 5.0), (-4.3, 11.6, 10.0), mat="hide_dark")
    rig.cube("leg_bl", "thigh_bl_muscle", (-9.15, 6.05, 5.8), (-4.25, 10.8, 9.2), mat="hide_base")
    rig.cube("leg_bl", "hock_node_bl", (-8.8, 4.15, 4.8), (-4.4, 5.85, 10.2), mat="hide_dark")  # 粗大飞节
    rig.cube("leg_bl", "moss_leg_bl", (-9.25, 8.0, 6.2), (-8.85, 10.2, 8.8), mat="moss_layer1")

    rig.cube("foot_bl", "shank_bl", (-8.6, 1.45, 5.4), (-4.6, 4.4, 9.6), mat="hide_base")
    rig.cube("foot_bl", "pastern_bl", (-8.75, 1.82, 5.3), (-4.45, 2.65, 9.7), mat="hide_dark")
    rig.cube("foot_bl", "hoof_bl_outer", (-8.95, 0.0, 5.1), (-6.85, 1.85, 9.9), mat="hoof_horn")
    rig.cube("foot_bl", "hoof_bl_inner", (-6.35, 0.0, 5.1), (-4.25, 1.85, 9.9), mat="hoof_horn")

    # ── 后右肢 (BR) ──
    rig.cube("leg_br", "thigh_br_main", (4.3, 4.5, 5.0), (8.9, 11.6, 10.0), mat="hide_dark")
    rig.cube("leg_br", "thigh_br_muscle", (4.25, 6.05, 5.8), (9.15, 10.8, 9.2), mat="hide_base")
    rig.cube("leg_br", "hock_node_br", (4.4, 4.15, 4.8), (8.8, 5.85, 10.2), mat="hide_dark")
    rig.cube("leg_br", "moss_leg_br", (8.85, 8.0, 6.2), (9.25, 10.2, 8.8), mat="moss_layer1")

    rig.cube("foot_br", "shank_br", (4.6, 1.45, 5.4), (8.6, 4.4, 9.6), mat="hide_base")
    rig.cube("foot_br", "pastern_br", (4.45, 1.82, 5.3), (8.75, 2.65, 9.7), mat="hide_dark")
    rig.cube("foot_br", "hoof_br_inner", (4.25, 0.0, 5.1), (6.35, 1.85, 9.9), mat="hoof_horn")
    rig.cube("foot_br", "hoof_br_outer", (6.85, 0.0, 5.1), (8.95, 1.85, 9.9), mat="hoof_horn")


def part_tail(rig: CreatureRig) -> None:
    """6. 短粗垂尾：带灰绿苔藓穗与毛簇。"""
    rig.cube("tail", "tail_stem", (-1.2, 8.5, 12.6), (1.2, 13.5, 14.2), mat="hide_base")
    rig.cube("tail", "tail_hair_base", (-1.6, 6.0, 12.8), (1.6, 8.4, 14.8), mat="hide_dark")
    rig.cube("tail", "tail_moss_tuft", (-1.8, 4.4, 13.0), (1.8, 6.4, 15.2), mat="moss_layer2")
    rig.cube("tail", "tail_floret", (-0.5, 4.0, 13.6), (0.5, 4.7, 14.6), mat="moss_flower")


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
        "from": [-6.8, 6.15, -9.0],
        "to": [6.8, 10.35, 11.0],  # 完全与 torso_lower 重合
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
    """渲染六视角接触表（诚实命名视角，使用 cjk_font 解决中文乱码）。"""
    bg = (119, 119, 119)  # 严格对齐参考图中性灰
    views = [
        ("FRONT",  0.0,   0.0, "正视 (从正面 -Z 直视宽钝鼻吻、粗短双角与前双肢)"),
        ("SIDE_R", 90.0,  0.0, "右侧视 (从 +X 直视高耸驼峰、层叠起伏噬灵藓与粗短柱腿)"),
        ("BACK",   180.0, 0.0, "后视 (从后方 +Z 直视圆阔臀部、后肢双瓣蹄与带藓短尾)"),
        ("SIDE_L", -90.0, 0.0, "左侧视 (从 -X 直视侧腹皮肤横褶与侧垂悬挂藓须)"),
        ("3/4",    -35.0, 22.0,"等轴 3/4 视 (前左侧立体观察驼峰起伏、前角与白花点)"),
        ("TOP",    0.0,   89.9,"俯视 (正上方俯瞰宽达16.8px背脊与三层叠块噬灵藓)"),
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
    header_h = 42
    label_h = 28

    total_w = margin * 2 + cols * tile_size + (cols - 1) * pad
    total_h = margin * 2 + header_h + rows * (tile_size + label_h + pad)

    sheet = Image.new("RGB", (total_w, total_h), (35, 36, 40))
    draw = ImageDraw.Draw(sheet)

    font_title = load_font(20)
    font_label = load_font(13)

    draw.text((margin, 12), "can_bei_tuo (残背驮兽) 六视角接触表 (Honest 6-View Contact Sheet) [Round 2]", fill=(235, 235, 235), font=font_title)

    for idx, (name, _, _, desc) in enumerate(views):
        r = idx // cols
        c = idx % cols
        px = margin + c * (tile_size + pad)
        py = margin + header_h + r * (tile_size + label_h + pad)

        sheet.paste(rendered_images[name], (px, py))
        draw.rectangle([px, py + tile_size, px + tile_size, py + tile_size + label_h], fill=(24, 25, 28))
        draw.text((px + 6, py + tile_size + 4), f"{name}: {desc}", fill=(210, 215, 220), font=font_label)

    return sheet, rendered_images


def make_check_sheet_round2(
    ref_path: Path,
    exploded_path: Path,
    prev_sheet_path: Path,
    now_sheet: Image.Image,
) -> Image.Image:
    """生成并排对照卡：左参考图/爆炸图，中上一轮 (Round 1) 接触表，右本轮 (Round 2) 接触表。"""
    ref_im = Image.open(ref_path).convert("RGB")
    exp_im = Image.open(exploded_path).convert("RGB") if exploded_path.exists() else None
    prev_sheet = Image.open(prev_sheet_path).convert("RGB") if prev_sheet_path.exists() else None

    target_h = now_sheet.height
    header_h = 42
    gap = 20

    # 左侧参考图上下堆叠
    sub_h = (target_h - gap) // 2
    sub_w = int(ref_im.width * sub_h / ref_im.height)
    ref_scaled = ref_im.resize((sub_w, sub_h), Image.Resampling.LANCZOS)
    exp_scaled = exp_im.resize((sub_w, sub_h), Image.Resampling.LANCZOS) if exp_im else ref_scaled

    left_w = sub_w
    now_w = now_sheet.width

    if prev_sheet:
        prev_scaled = prev_sheet.resize((now_w, target_h), Image.Resampling.LANCZOS)
        total_w = left_w + now_w * 2 + gap * 4
    else:
        prev_scaled = None
        total_w = left_w + now_w + gap * 3

    total_h = target_h + header_h + 24
    canvas = Image.new("RGB", (total_w, total_h), (28, 29, 33))
    draw = ImageDraw.Draw(canvas)

    font_header = load_font(18)

    # 绘制各栏目标题
    draw.text((gap, 10), "REFERENCES: Top: Concept Art | Bottom: Exploded", fill=(215, 205, 185), font=font_header)
    canvas.paste(ref_scaled, (gap, header_h))
    canvas.paste(exp_scaled, (gap, header_h + sub_h + gap))

    curr_x = gap + left_w + gap
    if prev_scaled:
        draw.text((curr_x, 10), "PREV: Round 1 Baseline (48 cubes - 平板藓/小头/无角)", fill=(185, 185, 195), font=font_header)
        canvas.paste(prev_scaled, (curr_x, header_h))
        curr_x += now_w + gap

    draw.text((curr_x, 10), "NOW: Round 2 Rework (142 cubes - 圆滚驼峰/前弯角/大头垂藓/起伏叠藓白花/偶蹄)", fill=(185, 235, 205), font=font_header)
    canvas.paste(now_sheet, (curr_x, header_h))

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
    print(f"✓ Blockbench 模型已生成: {out_local} ({len(rig.elements)} 个立方体)")
    print(f"✓ 仓库模型目录副本已生成: {out_models}")

    # 4. 渲染审阅图
    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    print("正在渲染六视角接触表 (使用 Noto Sans CJK 正文字体)...")
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

    # 5. 生成与上一轮并排对照的 check.png
    ref_img_path = REVIEW_DIR / "can_bei_tuo_ref.png"
    exp_img_path = REVIEW_DIR / "can_bei_tuo_exploded.png"
    prev_sheet_path = REVIEW_DIR / "contact_sheet_round1.png"

    if ref_img_path.exists():
        print("正在生成包含 PREV(Round 1) 与 NOW(Round 2) 的三栏并排对照卡 check.png...")
        check_sheet = make_check_sheet_round2(ref_img_path, exp_img_path, prev_sheet_path, contact_sheet)
        check_path = REVIEW_DIR / "check.png"
        check_sheet.save(check_path)
        print(f"✓ 并排对照卡 check.png 已保存: {check_path} (尺寸: {check_sheet.size})")
    else:
        print(f"提示: 未找到 {ref_img_path}，跳过 check.png 生成")


if __name__ == "__main__":
    main()
