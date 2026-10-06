#!/usr/bin/env python3
"""v2 重做生物的绑定稿：把终审通过的静态 bbmodel 分进可动画的骨骼树，并转成游戏朝向。

终审稿（``modelScript/models/*.bbmodel``）只读不写，外观以它为准。本脚本做三件事：

1. **分骨**：按 cube 名（前缀，必要时再加后缀）把几何挂进骨骼，每根骨的枢轴放在真实关节处，
   动画直接绕关节转，不需要任何位移补偿。
2. **朝向**：这批模型都是面朝 +Z 建的，而游戏里 GeckoLib 生物约定面朝 -Z
   （现役 ash_spider / dainu_lion 的头都在 -Z）。导出前把整件几何绕 Y 轴转 180°：
   坐标 (x, z) → (-x, -z)，north↔south、east↔west 两对侧面的 UV 对调，
   顶 / 底面 UV 旋转 180°（两个角对调）。刚体旋转不改变任何一个面从外面看上去的样子。
3. **离地**：个别模型有几何在地面以下（骨煞的祭布与铁链最低到 y = -12），按物种声明的
   抬升量整体上移，实体原点仍在脚底。

骨名沿用建模时的零件名（``leg_l_*`` / ``arm_l`` 等），「l / r」指的是建模稿里的那一侧，
转向后它们整体换到另一侧，名字不随之改。

产物写到 ``modelScript/models/fauna_v2/<Stem>Rig.bbmodel``（gitignored，可随时重建），
再交给 ``exporters/creature_codec.cjs`` 导出 GeckoLib geo（见同目录 ``export.py``）。

用法::

    python3 modelScript/creatures/fauna_v2/gen_rig.py             # 四种都出
    python3 modelScript/creatures/fauna_v2/gen_rig.py --only fuya_v2
"""

from __future__ import annotations

import argparse
import copy
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Callable
from uuid import NAMESPACE_URL, uuid5

HERE = Path(__file__).resolve().parent
MODELS = HERE.parents[1] / "models"
OUT = MODELS / "fauna_v2"

# 绕 Y 轴 180° 时，四个侧面两两互换；顶 / 底面留在原位但图案转半圈。
SIDE_FACE_SWAP = {"north": "south", "south": "north", "east": "west", "west": "east"}


@dataclass(frozen=True)
class Bone:
    """一根骨：名字、父骨、关节枢轴（建模稿坐标，px），以及认领哪些 cube。

    cube 名以某个 ``prefixes`` 开头、并且（给了 ``suffixes`` 时）以某个 ``suffixes`` 结尾，
    就归这根骨；多根骨都能认领时，前缀 + 后缀匹配得最长的那根赢。
    """

    name: str
    parent: str | None
    pivot: tuple[float, float, float]
    prefixes: tuple[str, ...] = ()
    suffixes: tuple[str, ...] = ()


@dataclass(frozen=True)
class Species:
    stem: str  # 终审稿文件名（不含扩展名）
    bones: Callable[[dict], tuple[Bone, ...]]  # 收到终审稿，返回骨表（有的物种要按几何算关节）
    lift: float = 0.0  # 整体上移多少 px


# ================================================================ 道伥
# 驼背人形骷髅：髋是全身的根，躯干 / 双腿都挂在髋上，驼背前倾只转 torso，双脚不离地。
# 破布（前裙 / 两侧裙 / 背后长须 / 肩披 / 兜帽垂条）各自一根骨，摆动才能错开相位。
DAOXIANG_BONES = (
    Bone("root", None, (0.0, 0.0, 0.0)),
    Bone("hips", "root", (0.0, 10.8, 1.2), ("robe_waist_belt_ring", "pouch_")),
    Bone("skirt_front", "hips", (0.0, 10.0, 2.9), ("robe_skirt_f_",)),
    Bone("skirt_l", "hips", (-3.2, 10.0, 1.3), ("robe_skirt_flank_l_",)),
    Bone("skirt_r", "hips", (3.2, 10.0, 1.3), ("robe_skirt_flank_r_",)),
    Bone("torso", "hips", (0.0, 11.0, 0.3),
         ("spine_", "rib_", "sternum_", "strap_", "robe_front_", "robe_back_base_band")),
    Bone("robe_back", "torso", (0.0, 10.5, -1.2), ("robe_back_strip_", "robe_back_mid_peeking_")),
    Bone("cape_l", "torso", (-3.4, 20.8, 1.0), ("cape_shoulder_l_", "cape_fringe_l_")),
    Bone("cape_r", "torso", (3.4, 20.8, 1.0), ("cape_shoulder_r_", "cape_fringe_r_")),
    Bone("head", "torso", (0.0, 21.6, 1.6), ("skull_", "hood_")),
    Bone("jaw", "head", (0.0, 21.2, 2.4), ("skull_mandible_base", "skull_lower_tooth_")),
    Bone("hood_tail_l", "head", (-2.7, 26.4, 2.7), ("hood_strip_l_",)),
    Bone("hood_tail_r", "head", (2.7, 26.4, 2.7), ("hood_strip_r_",)),
    Bone("hood_nape", "head", (0.0, 23.5, -0.5), ("hood_nape_",)),
    Bone("arm_l", "torso", (-3.6, 20.4, 0.5),
         ("arm_l_shoulder_joint", "arm_l_sleeve_rag", "arm_l_humerus_bone")),
    Bone("forearm_l", "arm_l", (-4.2, 14.4, 1.9), ("arm_l_",)),
    Bone("arm_r", "torso", (3.6, 20.4, 0.5),
         ("arm_r_shoulder_joint", "arm_r_sleeve_rag", "arm_r_humerus_bone")),
    Bone("forearm_r", "arm_r", (4.2, 14.4, 1.9), ("arm_r_",)),
    Bone("leg_l", "hips", (-2.0, 11.0, 1.15),
         ("leg_l_femur_bone", "leg_l_knee_joint", "leg_l_knee_bandage")),
    Bone("shin_l", "leg_l", (-2.0, 5.7, 1.15), ("leg_l_",)),
    Bone("leg_r", "hips", (2.0, 11.0, 1.15),
         ("leg_r_femur_bone", "leg_r_knee_joint", "leg_r_knee_bandage")),
    Bone("shin_r", "leg_r", (2.0, 5.7, 1.15), ("leg_r_",)),
)

# ================================================================ 负压畸变体
# 四足低伏重甲兽：四条腿直接挂 root（身体起伏时脚掌仍踩死地面），每条腿拆成
# 肌肉大腿 + 石甲小腿两节，枢轴在大腿根和石甲护膝处。背核独立一根骨做脉动。
FUYA_BONES = (
    Bone("root", None, (0.0, 0.0, 0.0)),
    Bone("body", "root", (0.0, 8.0, -1.0), ("carapace_", "tubercle_")),
    Bone("core", "body", (0.0, 12.5, -1.0), ("vortex_",)),
    Bone("head", "body", (0.0, 8.0, 6.0), ("head_",)),
    Bone("jaw", "head", (0.0, 3.3, 9.0), ("head_lower_jaw_", "head_tooth_lower_fang_")),
    Bone("leg_fl", "root", (-8.2, 8.0, 3.6), ("leg_fl_thigh_",)),
    Bone("foot_fl", "leg_fl", (-9.6, 4.4, 4.2), ("leg_fl_",)),
    Bone("leg_fr", "root", (8.2, 8.0, 3.6), ("leg_fr_thigh_",)),
    Bone("foot_fr", "leg_fr", (9.6, 4.4, 4.2), ("leg_fr_",)),
    Bone("leg_bl", "root", (-7.8, 8.0, -5.6), ("leg_bl_thigh_",)),
    Bone("foot_bl", "leg_bl", (-9.0, 4.4, -6.2), ("leg_bl_",)),
    Bone("leg_br", "root", (7.8, 8.0, -5.6), ("leg_br_thigh_",)),
    Bone("foot_br", "leg_br", (9.0, 4.4, -6.2), ("leg_br_",)),
)


# ================================================================ 灰烬蛛
# 八条腿每条拆成两节：股节（贴身的 femur_in / 向上拱的 femur_out / 膝结）绕腿根转，
# 胫节（tibia / 踝结 / 跗节 / 爪尖）绕膝结转。腿根 = femur_in 贴着头胸甲那一面的中心。
def ash_spider_bones(source: dict) -> tuple[Bone, ...]:
    by_name = {element["name"]: element for element in source["elements"]}

    def centre(name: str) -> tuple[float, float, float]:
        element = by_name[name]
        return tuple((element["from"][axis] + element["to"][axis]) / 2 for axis in range(3))

    bones = [
        Bone("root", None, (0.0, 0.0, 0.0)),
        Bone("body", "root", (0.0, 5.0, 0.0)),
        Bone("prosoma", "body", (0.0, 5.0, 2.0), ("prosoma_", "spider_eye_", "crust_prosoma_")),
        Bone("chelicerae", "prosoma", (0.0, 4.8, 7.6), ("chelicera_", "fang_")),
        Bone("palp_l", "prosoma", (-3.25, 4.2, 6.2), ("pedipalp_",), ("_l",)),
        Bone("palp_r", "prosoma", (3.25, 4.2, 6.2), ("pedipalp_",), ("_r",)),
        Bone("abdomen", "body", (0.0, 5.5, -1.0), ("abdomen_", "crust_abdomen_")),
    ]
    for side in "lr":
        for index in range(4):
            leg = f"leg_{side}_{index}"
            femur = by_name[f"{leg}_femur_in"]
            inner_x = femur["to"][0] if side == "l" else femur["from"][0]  # 贴身那一面
            _, hip_y, hip_z = centre(f"{leg}_femur_in")
            bones.append(Bone(leg, "body", (inner_x, hip_y, hip_z),
                              (f"{leg}_femur_", f"{leg}_knee_node")))
            bones.append(Bone(f"shin_{side}_{index}", leg, centre(f"{leg}_knee_node"), (f"{leg}_",)))
    return tuple(bones)


# ================================================================ 骨煞
# 漂浮主颅 + 五颗外挂小颅 + 脊柱肋笼 + 祭布 + 三挂铁链。外挂小颅、祭布、铁链各自分骨，
# 才能错相摆动；外挂小颅的枢轴放在身体中轴上，转起来是绕身环行而不是原地自转。
SKULL_FIEND_BONES = (
    Bone("root", None, (0.0, 0.0, 0.0)),
    Bone("body", "root", (0.0, 6.0, 0.0)),
    Bone("head", "body", (0.0, 6.0, 0.0), ("skull_", "crown_", "spine_")),
    Bone("eye_fire", "head", (0.0, 10.3, 4.6), ("eye_fire_",)),
    Bone("jaw", "head", (0.0, 4.5, 2.0), ("jaw_",)),
    Bone("ribcage", "body", (0.0, 5.7, -2.2), ("rib_",)),
    Bone("skull_upper_l", "body", (0.0, 12.4, 0.5), ("side_skull_",), ("_upper_l",)),
    Bone("skull_upper_r", "body", (0.0, 12.4, 0.5), ("side_skull_",), ("_upper_r",)),
    Bone("skull_lower_l", "body", (0.0, 8.1, 1.2), ("side_skull_",), ("_lower_l",)),
    Bone("skull_lower_r", "body", (0.0, 8.1, 1.2), ("side_skull_",), ("_lower_r",)),
    Bone("skull_front", "body", (0.0, 5.4, 2.0), ("side_skull_",), ("_front_c",)),
    Bone("cloth_front", "ribcage", (0.0, 2.5, 4.2), ("cloth_shred_",), ("_front_l", "_front_c", "_front_r")),
    Bone("cloth_l", "ribcage", (-5.2, 4.0, 0.0), ("cloth_shred_",), ("_mid_flank_l",)),
    Bone("cloth_r", "ribcage", (5.2, 4.0, 0.0), ("cloth_shred_",), ("_mid_flank_r",)),
    Bone("cloth_rear", "ribcage", (0.0, 1.0, -3.8), ("cloth_shred_",), ("_rear_shred_1", "_rear_shred_2")),
    Bone("chain_l", "body", (-5.2, 11.2, -0.6),
         ("chain_left_", "chain_weight_body_left", "chain_weight_rivet_band_left")),
    Bone("chain_r", "body", (5.2, 11.2, -0.6),
         ("chain_right_", "chain_weight_body_right", "chain_weight_rivet_band_right")),
    Bone("chain_rear", "body", (0.0, 11.2, -4.6),
         ("chain_rear_", "chain_weight_body_rear", "chain_weight_rivet_band_rear")),
)

# ================================================================ 执念
# 拖地破袍的持剑残魂。髋是全身的根，躯干 / 双腿挂在髋上，头和两臂挂在躯干上。
# 长袍的前 / 后 / 左 / 右四组布条各自一根骨，摆动才能错开相位；飘发（左 / 右 / 后脑）、
# 双手爪、右手长剑也各自分骨。肩甲跟着上臂走。「l / r」沿用建模稿，l 在 +x 侧。
# 终审稿面朝 +Z、中轴在原点（见 gen_zhinian.py 的 turn_to_plus_z / recenter_to_origin）。
ZHINIAN_BONES = (
    Bone("root", None, (0.0, 0.0, 0.0)),
    Bone("hips", "root", (0.0, 13.2, 0.0),
         ("core_pelvis", "robe_core", "robe_c_", "robe_waist", "belt_")),
    Bone("leg_l", "hips", (2.0, 11.2, 0.0),
         ("core_thigh", "core_knee", "core_shin", "core_ankle", "core_foot"), ("_l",)),
    Bone("leg_r", "hips", (-2.0, 11.2, 0.0),
         ("core_thigh", "core_knee", "core_shin", "core_ankle", "core_foot"), ("_r",)),
    Bone("robe_front", "hips", (0.0, 13.4, 1.3), ("robe_f",)),
    Bone("robe_back", "hips", (0.0, 13.4, -1.3), ("robe_b",)),
    Bone("robe_side_l", "hips", (3.0, 13.4, 0.0), ("robe_l",)),
    Bone("robe_side_r", "hips", (-3.0, 13.4, 0.0), ("robe_r",)),
    Bone("torso", "hips", (0.0, 13.3, 0.0),
         ("core_waist", "core_spine", "core_chest", "core_rib", "core_collar",
          "core_shoulder", "wraps_under", "wrap_")),
    Bone("head", "torso", (0.0, 21.8, 0.0), ("core_neck", "head_")),
    Bone("hair_l", "head", (2.0, 24.5, -0.5), ("hair_l_",)),
    Bone("hair_r", "head", (-2.0, 24.5, -0.5), ("hair_r_",)),
    Bone("hair_back", "head", (0.0, 24.0, -1.8), ("hair_b_",)),
    Bone("arm_l", "torso", (3.6, 20.8, 0.0), ("pauldron_l_", "arm_l_", "sleeve_l_")),
    Bone("forearm_l", "arm_l", (3.9, 14.6, 0.1), ("arm_l_forearm", "arm_l_wrist")),
    Bone("claw_l", "forearm_l", (3.5, 9.5, 1.2), ("claw_l_",)),
    Bone("arm_r", "torso", (-3.6, 20.8, 0.0), ("pauldron_r_", "arm_r_", "sleeve_r_")),
    Bone("forearm_r", "arm_r", (-3.9, 14.6, 0.1), ("arm_r_forearm", "arm_r_wrist")),
    Bone("claw_r", "forearm_r", (-3.5, 9.5, 1.2), ("claw_r_",)),
    Bone("sword", "forearm_r", (-3.6, 9.9, 0.55), ("sword_",)),
)

# ================================================================ 秘境守灵
# 石甲巨像：髋是全身的根，躯干 / 双腿挂在髋上，头和两臂挂在躯干上。
# 每条臂拆两节（上臂 + 肩甲 + 肩环绕肩转；肘环 + 前臂 + 石拳绕肘转），每条腿拆两节
# （髋环 + 大腿绕髋转；膝环 + 小腿 + 石座脚绕膝转）。腰带前襟 / 后襟 / 两侧碎布各自一根骨，
# 两侧腰石片挂髋，摆动才能错开相位。「l / r」沿用建模稿的名字：r 在建模稿 +x 侧。
# 终审稿面朝 +Z、中轴在原点（见 gen_tsy_sentinel.py 的 recenter_to_origin），不需要转向前的修正。
def _sentinel_side_bones(side: str, sign: float) -> tuple[Bone, ...]:
    return (
        Bone(f"arm_{side}", "torso", (sign * 5.1, 26.5, 0.0),
             (f"core_shoulder_{side}", f"core_upperarm_{side}", f"core_upperarm_strip_{side}",
              f"core_upperarm_band_{side}", f"pauldron_{side}_", f"arm_{side}_upper")),
        Bone(f"forearm_{side}", f"arm_{side}", (sign * 7.0, 20.3, 0.0),
             (f"core_elbow_{side}", f"core_forearm_{side}", f"core_forearm_strip_{side}",
              f"core_wrist_band_{side}", f"core_fist_{side}", f"arm_{side}_fore",
              f"arm_{side}_wrist", f"arm_{side}_fist", f"arm_{side}_finger")),
        Bone(f"leg_{side}", "hips", (sign * 2.8, 16.5, 0.0),
             (f"core_hip_{side}", f"core_thigh_{side}", f"core_thigh_strip_{side}",
              f"core_thigh_band_{side}", f"leg_{side}_thigh")),
        Bone(f"shin_{side}", f"leg_{side}", (sign * 3.5, 9.7, 0.0),
             (f"core_knee_{side}", f"core_shin_{side}", f"core_shin_strip_{side}",
              f"core_ankle_band_{side}", f"leg_{side}_shin", f"leg_{side}_ankle",
              f"leg_{side}_foot", f"leg_{side}_heel")),
        Bone(f"hipguard_{side}", "hips", (sign * 4.0, 15.0, 4.0), (f"tabard_stone_{side}",)),
        Bone(f"tabard_side_{side}", "hips", (sign * 5.6, 15.0, 0.0), (f"tabard_side_{side}",)),
    )


TSY_SENTINEL_BONES = (
    Bone("root", None, (0.0, 0.0, 0.0)),
    Bone("hips", "root", (0.0, 15.5, 0.0), ("core_pelvis", "belt_")),
    Bone("torso", "hips", (0.0, 16.5, 0.0),
         ("core_spine", "core_clavicle", "core_sternum", "core_rib", "core_chest_hollow", "chest_")),
    Bone("head", "torso", (0.0, 27.4, 0.0), ("core_neck", "head_")),
    Bone("tabard_front", "hips", (0.0, 15.0, 4.0), ("tabard_front",)),
    Bone("tabard_back", "hips", (0.0, 15.0, -3.1), ("tabard_back",)),
    *_sentinel_side_bones("r", 1.0),
    *_sentinel_side_bones("l", -1.0),
)

SPECIES = {
    "ash_spider_v2": Species("AshSpiderV2", ash_spider_bones),
    # 骨煞的终审稿在 #2325 落库时沿用了 SkullFiend.bbmodel 这个名字。
    # 最低的祭布须尖在 y = -12；抬 13.5 让它悬在地面上 1.5px，idle 起伏 1px 也不会入地。
    "skull_fiend_v2": Species("SkullFiend", lambda source: SKULL_FIEND_BONES, lift=13.5),
    "daoxiang_v2": Species("DaoxiangV2", lambda source: DAOXIANG_BONES),
    "fuya_v2": Species("FuyaV2", lambda source: FUYA_BONES),
    "zhinian_v2": Species("ZhinianV2", lambda source: ZHINIAN_BONES),
    "tsy_sentinel_v2": Species("TsySentinelV2", lambda source: TSY_SENTINEL_BONES),
}


def assign(element_name: str, bones: tuple[Bone, ...]) -> str:
    """最长匹配优先：``arm_l_humerus_bone`` 进上臂，其余 ``arm_l_*`` 才落到前臂。"""

    best: tuple[int, str] | None = None
    for bone in bones:
        for prefix in bone.prefixes:
            if not element_name.startswith(prefix):
                continue
            for suffix in bone.suffixes or ("",):
                if element_name.endswith(suffix):
                    score = len(prefix) + len(suffix)
                    if best is None or score > best[0]:
                        best = (score, bone.name)
    if best is None:
        raise ValueError(f"cube {element_name!r} 没有骨认领；请在骨表里补前缀")
    return best[1]


def turn_to_minus_z(point: list[float], lift: float) -> list[float]:
    return [-point[0], point[1] + lift, -point[2]]


def turn_element(element: dict, lift: float) -> None:
    """把一个 cube 绕 Y 转 180° 并上移，原地改写。

    带旋转的 cube 也能转：整件绕 Y 转 180° 是对称共轭，原先绕 X、Z 轴的转角反号，绕 Y 的不变，
    即欧拉角 (rx, ry, rz) -> (-rx, ry, -rz)，旋转中心 origin 与几何同样转向。
    """

    start, end = element["from"], element["to"]
    element["from"] = [-end[0], start[1] + lift, -end[2]]
    element["to"] = [-start[0], end[1] + lift, -start[2]]
    if element.get("rotation") and any(element["rotation"]):
        if "origin" not in element:
            raise ValueError(f"cube {element['name']!r} 有旋转却没有 origin，无法转向")
        rx, ry, rz = element["rotation"]
        element["rotation"] = [-rx, ry, -rz]
    if "origin" in element:
        element["origin"] = turn_to_minus_z(element["origin"], lift)

    faces = {}
    for face, data in element.get("faces", {}).items():
        data = dict(data)
        if face in SIDE_FACE_SWAP:
            faces[SIDE_FACE_SWAP[face]] = data
        else:  # up / down：图案转半圈 = UV 两个角对调
            u1, v1, u2, v2 = data["uv"]
            data["uv"] = [u2, v2, u1, v1]
            faces[face] = data
    element["faces"] = faces


def build_rig(source: dict, name: str, bones: tuple[Bone, ...], lift: float = 0.0) -> dict:
    """返回新的 bbmodel：几何朝 -Z、上移 lift，outliner 换成骨骼树。"""

    doc = copy.deepcopy(source)
    groups = {
        bone.name: {
            "uuid": str(uuid5(NAMESPACE_URL, f"bong/fauna_v2/{name}/{bone.name}")),
            "name": bone.name,
            "origin": turn_to_minus_z(list(bone.pivot), lift),
            "rotation": [0.0, 0.0, 0.0],
            "children": [],
        }
        for bone in bones
    }
    for element in doc["elements"]:
        # 生成器写的 4.10 稿省略了 type；Blockbench 对缺省 type 一律按 cube 读，
        # 离线 codec 则要求显式写出，这里补上同义字段。
        element.setdefault("type", "cube")
        groups[assign(element["name"], bones)]["children"].append(element["uuid"])
        turn_element(element, lift)

    roots = []
    for bone in bones:
        group = groups[bone.name]
        if bone.parent is None:
            roots.append(group)
        else:
            groups[bone.parent]["children"].append(group)

    doc["outliner"] = roots
    doc["groups"] = []
    doc["animations"] = []
    return doc


def load_species(name: str) -> tuple[dict, dict, tuple[Bone, ...]]:
    """返回（终审稿, 绑定稿, 骨表）。"""

    species = SPECIES[name]
    source = json.loads((MODELS / f"{species.stem}.bbmodel").read_text(encoding="utf-8"))
    bones = species.bones(source)
    return source, build_rig(source, name, bones, species.lift), bones


def rig_path(name: str) -> Path:
    return OUT / f"{name}_rig.bbmodel"


def write_rig(name: str) -> Path:
    _, rig, bones = load_species(name)
    path = rig_path(name)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(rig, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"{name}: {len(bones)} 根骨、{len(rig['elements'])} 个 cube → {path}")
    return path


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--only", choices=tuple(SPECIES), action="append")
    args = parser.parse_args()
    for name in args.only or SPECIES:
        write_rig(name)


if __name__ == "__main__":
    main()
