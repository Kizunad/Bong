#!/usr/bin/env python3
"""枯草裹脚（ku_cao_guo_jiao）Blockbench .bbmodel 程序化生成器 —— Round 1/3：几何骨架与部件。

任务卡：末土装备 #17（.task-wasteland-gear.md）。
挂点：双脚（脚底 y∈[-0.85, 0.04]，脚背与脚掌 y∈[0.0, 2.8]，踝部 y∈[2.2, 4.2]）。
穿戴约定见 modelScript/gear/_kit/gear_kit.py：
  脚底 y=0，玩家面朝 -z，+z 是背面，+x = 穿戴者右侧。
  玩家双腿：右腿 x∈[0,4], 左腿 x∈[-4,0], y∈[0,12], z∈[-2,2]
  门禁 clearance：任何方块不得扎进双腿（咬入 ≤ 0.05px；在 y∈[0,12] 且 z∈[-2,2] 内不可穿透）。
  脚底草鞋底位于 y ∈ [-0.85, 0.02]（贴合 y=0 底面）；
  前掌外伸部位于 z ∈ [-3.5, -2.08]（在腿盒 z=-2 之外，可向上隆起包覆脚尖）。

外形：
  枯草编织的厚草底垫在脚底，鞋底边缘有草茎交错的粗编滚边，前掌微翘；
  脚背与脚掌被多层交错破布条缠裹，带磨损毛边与缝补补丁；
  脚跟后方有草编护跟（z≈2.08..2.45）；
  脚踝上方（y≈2.2..4.2）用双股皮绳斜向穿缠收束并打结，带垂带；
  右脚背系带穿有一根磨扁骨扣，左鞋外侧草耳下挂一枚磨白小骨牙。

分部件：
  - sandal_sole: 左右脚枯草编织鞋底（厚草垫、前伸翘头、鞋底编条滚边）
  - heel_guard: 脚后跟草编包根与后跟加固草耳
  - instep_wraps: 脚背粗布层叠缠裹（斜向交错布层、补丁与粗麻毛边）
  - ankle_straps: 脚踝交叉扎紧的皮绳、草耳与紧箍带
  - shoe_details: 兽骨鞋鼻扣、前脚掌缝线与左脚挂骨牙坠（不对称）

用法：
    python3 modelScript/gear/ku_cao_guo_jiao/gen_ku_cao_guo_jiao.py --self-test   # 门禁差分自证
    python3 modelScript/gear/ku_cao_guo_jiao/gen_ku_cao_guo_jiao.py --part sandal_sole # 单件预览
    python3 modelScript/gear/ku_cao_guo_jiao/gen_ku_cao_guo_jiao.py --worn        # 只出穿戴三面
    python3 modelScript/gear/ku_cao_guo_jiao/gen_ku_cao_guo_jiao.py               # 全量：模型 + check.png
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "_kit"))
import gear_kit  # noqa: E402
from gear_kit import Gear, GearSpec, soft_pairs  # noqa: E402

# ───────────────────────────────────────────────────────────── 材质定义
MATS = {
    "straw":        (168, 142, 96),    # 枯草编织主色
    "straw_lt":     (194, 172, 118),   # 草编亮面/凸起编草
    "straw_dk":     (118, 96, 62),     # 草编阴面/底缝
    "linen":        (142, 126, 98),    # 裹脚粗布
    "linen_lt":     (172, 154, 122),   # 粗布亮面/折痕
    "linen_dk":     (108, 92, 70),     # 粗布暗面
    "patch":        (120, 88, 64),     # 兽皮耐磨补丁
    "patch_dk":     (86, 62, 44),      # 补丁暗面
    "cord":         (150, 126, 84),    # 扎踝皮绳/草绳
    "cord_dk":      (104, 84, 54),     # 绳结深色
    "stitch":       (188, 176, 148),   # 粗缝线
    "bone":         (206, 194, 164),   # 骨鼻扣与骨坠
    "bone_lt":      (224, 214, 186),   # 骨尖高光
    "bone_dk":      (156, 142, 112),   # 骨阴影
    "void":         (28, 24, 22),      # 草耳穿绳暗孔
}

SOFT = ("straw", "straw_lt", "straw_dk", "linen", "linen_lt", "linen_dk", "patch", "patch_dk", "cord", "cord_dk", "stitch")
SOFT_OVER = soft_pairs(SOFT, tuple(MATS)) | soft_pairs(("void",), tuple(MATS))

# ───────────────────────────────────────────────────────────── 骨骼树（按挂点分骨 ≥ 10）
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("foot_r_mount", "root", (2.0, 0.0, 0.0), "右脚安装基准骨"),
    ("sole_main_r", "foot_r_mount", (2.0, -0.4, -0.6), "右脚草鞋底主体"),
    ("heel_guard_r", "foot_r_mount", (2.0, 1.2, 2.1), "右脚后跟草编包根"),
    ("instep_wrap_r", "foot_r_mount", (2.0, 1.4, -1.8), "右脚背粗布裹脚层"),
    ("ankle_strap_r", "foot_r_mount", (2.0, 2.8, 0.0), "右脚踝绑缚皮绳"),
    ("shoe_bone_r", "instep_wrap_r", (2.0, 1.6, -2.6), "右脚背兽骨鞋扣"),
    ("foot_l_mount", "root", (-2.0, 0.0, 0.0), "左脚安装基准骨"),
    ("sole_main_l", "foot_l_mount", (-2.0, -0.4, -0.6), "左脚草鞋底主体"),
    ("heel_guard_l", "foot_l_mount", (-2.0, 1.2, 2.1), "左脚后跟草编包根"),
    ("instep_wrap_l", "foot_l_mount", (-2.0, 1.4, -1.8), "左脚背粗布裹脚层"),
    ("ankle_strap_l", "foot_l_mount", (-2.0, 2.8, 0.0), "左脚踝绑缚皮绳"),
    ("hang_tooth_l", "foot_l_mount", (-4.2, 1.8, 0.0), "左脚外草耳挂骨牙坠"),
]

ASYM_BONES = frozenset([
    "shoe_bone_r", "hang_tooth_l",
])


# ───────────────────────────────────────────────────────────── 分部件实现
def part_sandal_sole(g: Gear) -> None:
    """枯草编织鞋底：
    脚底下方 y ∈ [-0.85, 0.02]（咬入腿盒 y=0 的深度 ≤ 0.02px，满足 clearance 门的 0.05px 容差）。
    脚掌前伸部分在 z ≤ -2.08，不受腿盒限制，向上翻翘包住脚尖。
    内侧留出 0.08px 间隙避免左右脚冲突。
    全部使用 g.pair 镜像到左脚（sided=True）。
    """
    b = "sole_main"
    # 脚底草鞋厚底板 (y: -0.78 .. 0.02, z: -1.95 .. 2.05)
    g.pair(b, "sole_base_rear", (0.08, -0.78, -1.95), (4.02, 0.02, 2.05), "straw", sided=True)
    g.pair(b, "sole_bottom_strip", (0.35, -0.88, -1.85), (3.75, -0.62, 1.95), "straw_dk", sided=True)
    g.pair(b, "sole_weave_braid1", (0.5, -0.82, -1.75), (1.5, -0.55, 1.85), "straw_lt", sided=True)
    g.pair(b, "sole_weave_braid2", (2.6, -0.82, -1.75), (3.6, -0.55, 1.85), "straw_lt", sided=True)

    # 外侧鞋底边缘编包边 (x ∈ [4.04, 4.42], y ∈ [-0.75, 0.45])
    g.pair(b, "sole_side_rim", (4.04, -0.75, -1.88), (4.42, 0.45, 1.88), "straw_dk", sided=True)
    g.pair(b, "sole_side_cord", (4.12, -0.45, -1.65), (4.48, 0.25, 1.65), "cord", sided=True)

    # 前脚掌伸出部底板 (z: -3.45 .. -1.98, y: -0.78 .. 0.42)
    g.pair(b, "sole_front_slab", (0.08, -0.78, -3.45), (4.02, 0.42, -1.98), "straw", sided=True)
    g.pair(b, "sole_front_strip", (0.4, -0.85, -3.42), (3.7, -0.55, -2.02), "straw_dk", sided=True)
    # 脚尖上翻包头 (z ∈ [-3.58, -3.15], y ∈ [0.25, 1.15])
    g.pair(b, "sole_toe_up", (0.45, 0.25, -3.58), (3.65, 1.15, -3.15), "straw_dk", sided=True)
    g.pair(b, "sole_toe_crest", (0.8, 0.52, -3.64), (3.3, 1.05, -3.22), "straw_lt", sided=True)
    g.pair(b, "sole_toe_rim", (0.6, 1.02, -3.55), (3.5, 1.32, -3.18), "straw", sided=True)


def part_heel_guard(g: Gear) -> None:
    """脚后跟草编包根与后跟加固带：紧贴脚后部 z ∈ [2.08, 2.45]，y ∈ [0.05, 2.4]。"""
    b = "heel_guard"
    # 后跟草编主包裹片
    g.pair(b, "heel_back_main", (0.08, 0.05, 2.08), (4.02, 2.05, 2.38), "straw", sided=True)
    g.pair(b, "heel_back_crest", (0.6, 0.35, 2.22), (3.5, 1.85, 2.46), "straw_lt", sided=True)
    g.pair(b, "heel_rim_top", (0.3, 1.95, 2.12), (3.8, 2.38, 2.42), "straw_dk", sided=True)
    g.pair(b, "heel_pleat_v1", (1.0, 0.45, 2.15), (1.6, 1.75, 2.42), "straw", sided=True)
    g.pair(b, "heel_pleat_v2", (2.4, 0.45, 2.15), (3.0, 1.75, 2.42), "straw_dk", sided=True)

    # 后跟外侧延伸 (x ∈ [4.05, 4.38], z ∈ [0.8, 2.15], y ∈ [0.35, 2.05])
    g.pair(b, "heel_side_out", (4.05, 0.35, 0.8), (4.38, 2.05, 2.15), "straw_dk", sided=True)
    g.pair(b, "heel_side_cord", (4.12, 0.75, 1.0), (4.44, 1.65, 2.05), "cord", sided=True)
    g.pair(b, "heel_side_pleat", (4.08, 0.55, 1.3), (4.42, 1.85, 1.85), "straw_lt", sided=True)


def part_instep_wraps(g: Gear) -> None:
    """脚背粗布层叠缠裹：位于前脚掌与脚背处 (z ∈ [-3.45, -2.08]，y ∈ [0.42, 2.8])。"""
    b = "instep_wrap"
    # 前脚掌裹布底层 (z: -3.42 .. -2.08, y: 0.42 .. 1.25)
    g.pair(b, "wrap_toe_base", (0.12, 0.42, -3.42), (3.98, 1.25, -2.08), "linen", sided=True)
    g.pair(b, "wrap_toe_fold1", (0.4, 0.62, -3.38), (3.7, 1.35, -2.68), "linen_lt", sided=True)
    g.pair(b, "wrap_toe_fold2", (0.2, 0.62, -2.65), (3.9, 1.35, -2.12), "linen_dk", sided=True)
    g.pair(b, "wrap_toe_seam", (1.2, 0.82, -3.32), (2.8, 1.12, -2.45), "stitch", sided=True)

    # 脚背向上爬升折层 (z: -2.85 .. -2.08, y: 1.28 .. 2.55)
    g.pair(b, "wrap_instep_slope", (0.15, 1.22, -2.82), (3.95, 2.42, -2.16), "linen", sided=True)
    g.pair(b, "wrap_instep_crest", (0.6, 1.55, -2.88), (3.5, 2.25, -2.26), "linen_lt", sided=True)
    g.pair(b, "wrap_instep_shade", (0.22, 1.28, -2.75), (3.88, 1.85, -2.18), "linen_dk", sided=True)
    g.pair(b, "wrap_cross_pleat_a", (0.5, 1.68, -2.86), (2.12, 2.38, -2.35), "linen_dk", sided=True)
    g.pair(b, "wrap_cross_pleat_b", (1.88, 1.58, -2.76), (3.45, 2.28, -2.36), "linen_lt", sided=True)

    # 前脚掌外侧兽皮补强片 (patch)
    g.pair(b, "wrap_patch_out", (3.2, 0.68, -3.25), (4.15, 1.55, -2.35), "patch", sided=True)
    g.pair(b, "wrap_patch_worn", (3.4, 0.85, -3.15), (4.18, 1.28, -2.55), "patch_dk", sided=True)

    # 脚掌外侧固定布舌 (x ∈ [4.05, 4.38], z ∈ [-2.5, -0.6])
    g.pair(b, "wrap_tongue_out", (4.05, 0.45, -2.4), (4.38, 1.65, -0.6), "linen_dk", sided=True)


def part_ankle_straps(g: Gear) -> None:
    """脚踝上方绑缚草绳、草耳与绳结 (y ∈ [2.2, 4.2])。"""
    b = "ankle_strap"
    # 下圈绑绳 (y: 2.72 .. 3.28, 前、外、后相连)
    g.pair(b, "ankle_cord_f", (0.05, 2.75, -2.42), (4.22, 3.25, -2.15), "cord", sided=True)
    g.pair(b, "ankle_cord_out", (4.05, 2.72, -2.28), (4.44, 3.28, 2.28), "cord", sided=True)
    g.pair(b, "ankle_cord_b", (0.05, 2.75, 2.15), (4.22, 3.25, 2.42), "cord", sided=True)
    g.pair(b, "ankle_stay_f", (1.2, 2.35, -2.48), (2.8, 3.65, -2.22), "cord_dk", sided=True)

    # 上圈加固细绳 (y: 3.52 .. 3.98, 前、外、后相连，绝不悬空)
    g.pair(b, "ankle_cord_hi_f", (0.05, 3.55, -2.38), (4.22, 3.95, -2.15), "cord_dk", sided=True)
    g.pair(b, "ankle_cord_hi_out", (4.05, 3.52, -2.28), (4.42, 3.98, 2.28), "cord_dk", sided=True)
    g.pair(b, "ankle_cord_hi_b", (0.05, 3.55, 2.15), (4.22, 3.95, 2.38), "cord_dk", sided=True)

    # 外侧草编系耳 (提鞋耳圈，x ∈ [4.12, 4.58], y ∈ [1.6, 2.95], z ∈ [-0.42, 0.42])
    g.pair(b, "sandal_ear_loop", (4.12, 1.65, -0.42), (4.56, 2.95, 0.42), "straw_dk", sided=True)
    g.pair(b, "sandal_ear_hole", (4.24, 1.95, -0.24), (4.58, 2.65, 0.24), "void", sided=True)

    # 踝前绑带系结 (y: 2.55 .. 3.45, z: -2.62 .. -2.2)
    g.pair(b, "ankle_knot_main", (1.6, 2.58, -2.62), (2.4, 3.38, -2.24), "cord_dk", sided=True)
    g.pair(b, "ankle_knot_tail1", (1.75, 1.65, -2.56), (2.15, 2.65, -2.25), "cord", sided=True)
    g.pair(b, "ankle_knot_tail2", (2.2, 1.75, -2.56), (2.55, 2.65, -2.25), "cord_dk", sided=True)


def part_shoe_details(g: Gear) -> None:
    """细部特征：右脚背上的兽骨鼻扣、加固缝线，与左脚外耳挂小骨牙（不对称挂件）。"""
    # 右脚背兽骨扣（长宽厚 ≥ 0.65px，供穿模差分自证）
    sb = "shoe_bone_r"
    g.box(sb, "bone_buckle_main", (1.52, 1.25, -2.96), (2.48, 1.95, -2.28), "bone")
    g.box(sb, "bone_buckle_crest", (1.72, 1.45, -2.99), (2.28, 1.78, -2.32), "bone_lt")
    g.box(sb, "bone_buckle_pin", (1.82, 1.15, -2.92), (2.18, 2.05, -2.34), "bone_dk")
    # 脚掌加固缝线迹
    for k, x in enumerate((0.7, 2.7)):
        g.box(sb, f"stitch_toe_{k}", (x, 0.85, -3.25), (x + 0.5, 1.18, -2.82), "stitch")

    # 左脚外侧挂兽牙小坠（尺寸厚度 ≥ 0.65px，供穿模差分自证）
    ht = "hang_tooth_l"
    g.box(ht, "tooth_cord", (-4.65, 2.15, -0.18), (-4.28, 2.85, 0.18), "cord_dk")
    g.box(ht, "tooth_head", (-4.88, 1.35, -0.38), (-4.22, 2.15, 0.38), "bone_dk")
    g.box(ht, "tooth_body", (-4.85, 0.55, -0.35), (-4.25, 1.35, 0.35), "bone")
    g.box(ht, "tooth_tip", (-4.78, -0.05, -0.32), (-4.32, 0.55, 0.32), "bone_lt")


PART_FUNCS = {
    "sandal_sole": part_sandal_sole,
    "heel_guard": part_heel_guard,
    "instep_wraps": part_instep_wraps,
    "ankle_straps": part_ankle_straps,
    "shoe_details": part_shoe_details,
}

SPEC = GearSpec(
    gear_id="ku_cao_guo_jiao",
    model_name="KuCaoGuoJiao",
    title="枯草裹脚 / ku_cao_guo_jiao",
    mats=MATS,
    bones=BONES,
    parts=PART_FUNCS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    wear_note="挂点：双脚（y∈[-0.85, 4.2]，右脚 x∈[0,4], 左脚 x∈[-4,0]）",
    worn_focus=((0.0, 1.5, 0.0), 16.0),
    round_label="round 1/3",
    textured=False,
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
