#!/usr/bin/env python3
"""粗布面巾（cu_bu_mian_jin）Blockbench .bbmodel 程序化生成器 —— Round 1/3：几何骨架与部件。

任务卡：末土装备 #12（.task-wasteland-gear.md）。
挂点：口鼻 / 颈 / 上胸（头面部 y∈[24,28.6] + 颈胸下摆 y∈[14.5,24]）。
穿戴约定见 modelScript/gear/_kit/gear_kit.py：
  脚底 y=0，玩家面朝 -z，+z 是背面，+x = 穿戴者右侧。
  头盒 x∈[-4,4], y∈[24,32], z∈[-4,4]
  躯干 x∈[-4,4], y∈[12,24], z∈[-2,2]
  双臂 x∈[4,8] / [-8,-4], y∈[12,24], z∈[-2,2]
  门禁 clearance：任何方块不得扎进头盒、躯干或双臂（咬入 ≤ 0.05px）。

外形：
  粗麻残布围裹口鼻（y≈24.0..28.5），顺着下巴和脸颊向后收束；
  两侧贴着脸颊（x≈±4.08..4.45），脑后有粗皮绳/布结（z≈4.08..4.9）；
  口鼻处有破洞层叠与补丁；
  面巾下沿向下垂落形成遮风下摆，覆盖颈部（y≈22.2..24.0）并沿前胸（z≈-2.7..-2.08, y≈14.5..23.5）参差垂落；
  右颊下方挂有一只锈铁小环与磨尖兽骨针挂件。

分部件：
  - mask_face: 口鼻正面包裹层（梯级圆润贴合，隆起避开头盒 -z 面）
  - mask_cheeks: 左右面颊包覆层（贴合左右 x=±4 面）
  - mask_knots: 脑后系结与绑带（贴合后脑 z=4 面，系结与两截垂带）
  - bib_neck: 颈部环绕过渡层（连接头盒下部与躯干上部）
  - bib_drape: 前胸下垂布摆（多层参差叠落）
  - mask_details: 表面粗布补丁、破洞暗衬与缝线粗线迹
  - trinket_bone: 面巾侧系带下挂的锈铁小环与小骨针

用法：
    python3 modelScript/gear/cu_bu_mian_jin/gen_cu_bu_mian_jin.py --self-test   # 门禁差分自证
    python3 modelScript/gear/cu_bu_mian_jin/gen_cu_bu_mian_jin.py --part mask_face # 单件预览
    python3 modelScript/gear/cu_bu_mian_jin/gen_cu_bu_mian_jin.py --worn        # 只出穿戴三面
    python3 modelScript/gear/cu_bu_mian_jin/gen_cu_bu_mian_jin.py               # 全量：模型 + check.png
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "_kit"))
import gear_kit  # noqa: E402
from gear_kit import Gear, GearSpec, soft_pairs  # noqa: E402

# ───────────────────────────────────────────────────────────── 材质定义
MATS = {
    "linen":        (142, 126, 98),    # 粗布主色
    "linen_lt":     (172, 154, 122),   # 粗布凸起高光/亮面
    "linen_dk":     (108, 92, 70),     # 粗布褶皱阴面/折痕
    "patch":        (120, 88, 64),     # 废皮/深色粗布补丁
    "patch_dk":     (86, 62, 44),      # 补丁阴影
    "patch_lt":     (148, 114, 86),    # 补丁磨白边缘
    "cord":         (150, 126, 84),    # 粗草绳/麻绳
    "cord_dk":      (104, 84, 54),     # 绳结深色
    "stitch":       (188, 176, 148),   # 粗线迹
    "rust":         (146, 84, 50),     # 锈铁环
    "bone":         (206, 194, 164),   # 骨针/骨坠
    "bone_lt":      (224, 214, 186),   # 骨尖高光
    "bone_dk":      (156, 142, 112),   # 骨阴面
    "void":         (28, 24, 22),      # 破洞内暗衬
}

SOFT = ("linen", "linen_lt", "linen_dk", "patch", "patch_dk", "patch_lt", "cord", "cord_dk", "stitch")
# 软件压在任何材质上均允许软重叠；硬件间（rust, bone, void）互穿仍受门禁检查
SOFT_OVER = soft_pairs(SOFT, tuple(MATS)) | soft_pairs(("void",), SOFT)

# ───────────────────────────────────────────────────────────── 骨骼树（按挂点分骨 ≥ 10）
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("head_mount", "root", (0.0, 24.0, 0.0), "头戴基准骨"),
    ("mask_face", "head_mount", (0.0, 26.0, -4.2), "口鼻正面遮罩布层"),
    ("mask_cheek_r", "head_mount", (4.2, 26.0, 0.0), "右面颊护布"),
    ("mask_cheek_l", "head_mount", (-4.2, 26.0, 0.0), "左面颊护布"),
    ("mask_knot_back", "head_mount", (0.0, 26.5, 4.3), "后脑系结"),
    ("knot_tail_r", "mask_knot_back", (0.6, 26.0, 4.5), "脑后垂带（右）"),
    ("knot_tail_l", "mask_knot_back", (-0.6, 26.0, 4.5), "脑后垂带（左）"),
    ("neck_wrap", "root", (0.0, 23.0, 0.0), "颈围过渡圈"),
    ("bib_chest", "root", (0.0, 22.0, -2.3), "胸前下摆主片"),
    ("bib_fray_r", "bib_chest", (2.0, 18.0, -2.4), "胸前下摆右磨损片"),
    ("bib_fray_l", "bib_chest", (-2.0, 18.0, -2.4), "胸前下摆左磨损片"),
    ("mask_details", "bib_chest", (0.0, 21.0, -2.4), "补丁与粗麻缝线"),
    ("hang_iron_ring", "head_mount", (4.3, 25.0, -2.0), "右颊挂绳锈铁环"),
    ("hang_bone_charm", "hang_iron_ring", (4.4, 23.5, -2.0), "挂环下兽骨针坠"),
]

ASYM_BONES = frozenset([
    "knot_tail_r", "knot_tail_l", "bib_fray_r", "bib_fray_l", "mask_details",
    "hang_iron_ring", "hang_bone_charm",
])


# ───────────────────────────────────────────────────────────── 分部件实现
def part_mask_face(g: Gear) -> None:
    """口鼻正面遮罩：贴在头盒前端 z ∈ [-4.9, -4.08]，y ∈ [24.08, 28.7]。
    
    全部件均在 z ≤ -4.08，绝不进入头盒 z ∈ [-4, 4]。
    中线件严格关于 x=0 对称，成对件严格 _r / _l 配对。
    """
    b = "mask_face"
    # 主鼻梁中心隆起 (y: 26.6 .. 28.4)
    g.box(b, "nose_ridge_main", (-1.2, 26.6, -4.72), (1.2, 28.4, -4.12), "linen")
    g.box(b, "nose_ridge_crest", (-0.7, 27.1, -4.88), (0.7, 28.2, -4.42), "linen_lt")
    # 左右鼻翼斜坡 (成对件)
    g.pair(b, "nose_slope", (1.2, 26.7, -4.62), (2.8, 28.3, -4.15), "linen")
    # 鼻梁上边缘滚边（中线件）
    g.box(b, "nose_top_hem", (-2.6, 28.4, -4.52), (2.6, 28.7, -4.18), "linen_lt")

    # 口唇部正前方中线与两侧 (y: 25.2 .. 26.6)
    g.box(b, "mouth_main_c", (-1.8, 25.2, -4.68), (1.8, 26.6, -4.12), "linen")
    g.box(b, "mouth_fold_crest", (-1.5, 25.7, -4.82), (1.5, 26.3, -4.4), "linen_lt")
    g.pair(b, "mouth_side", (1.8, 25.2, -4.56), (3.6, 26.6, -4.15), "linen")

    # 下巴包底与前突 (y: 24.08 .. 25.2)
    # 必须在 y ≥ 24.08，z ≤ -4.08 处环抱下巴前沿
    g.box(b, "chin_curve_c", (-2.0, 24.08, -4.6), (2.0, 25.2, -4.12), "linen")
    g.pair(b, "chin_side", (2.0, 24.1, -4.48), (3.8, 25.2, -4.15), "linen")
    # 下巴底缘包边（y: 24.12 .. 24.35，前突在 z: -4.75 .. -4.3）
    g.box(b, "chin_bottom_wrap", (-3.2, 24.12, -4.75), (3.2, 24.35, -4.3), "linen_dk")

    # 面巾正前表面折痕筋线 (中线件自身严格对称，且与底面避免共面)
    g.box(b, "fold_ridge_1", (-2.4, 27.5, -4.84), (2.4, 27.75, -4.48), "linen_lt")
    g.box(b, "fold_shadow_1", (-2.4, 27.22, -4.76), (2.4, 27.48, -4.45), "linen_dk")
    g.box(b, "fold_ridge_2", (-2.75, 26.08, -4.78), (2.75, 26.32, -4.42), "linen_lt")
    g.box(b, "fold_shadow_2", (-2.75, 25.78, -4.72), (2.75, 26.02, -4.36), "linen_dk")
    g.box(b, "fold_ridge_3", (-3.0, 24.78, -4.7), (3.0, 25.02, -4.34), "linen_lt")


def part_mask_cheeks(g: Gear) -> None:
    """左右面颊包覆层：紧贴头盒侧面 x ∈ [±4.08, ±4.48]，y ∈ [24.2, 28.2]，z ∈ [-4.1, 4.05]。
    全部使用 g.pair 自动镜像，保证左右严格对齐。
    """
    b = "mask_cheek"  # pair 将自动分派到 mask_cheek_r 与 mask_cheek_l
    # 颊前段
    g.pair(b, "cheek_front", (4.08, 24.6, -4.08), (4.42, 28.2, -1.5), "linen", sided=True)
    # 颊中段（厚度稍变，避免与前段共面）
    g.pair(b, "cheek_mid", (4.1, 24.8, -1.48), (4.45, 28.0, 1.48), "linen", sided=True)
    # 颊后段
    g.pair(b, "cheek_back", (4.08, 25.0, 1.5), (4.42, 27.8, 4.05), "linen", sided=True)
    # 颊部加固皮绳
    g.pair(b, "cheek_strap_1", (4.16, 26.6, -3.8), (4.52, 27.28, 3.8), "cord", sided=True)
    # 顶边与底边压缝
    g.pair(b, "cheek_hem_top", (4.12, 27.92, -3.9), (4.46, 28.25, 3.5), "linen_lt", sided=True)
    g.pair(b, "cheek_hem_bot", (4.12, 24.58, -3.5), (4.44, 24.95, 2.5), "linen_dk", sided=True)


def part_mask_knots(g: Gear) -> None:
    """脑后绑带与系结：贴合后脑 z ∈ [4.08, 5.05]，y ∈ [25.0, 27.8]。"""
    bk = "mask_knot_back"
    # 后脑横过绑带（中线件）
    g.box(bk, "strap_back_main", (-4.05, 25.6, 4.08), (4.05, 27.4, 4.4), "linen")
    g.box(bk, "strap_back_cord", (-3.8, 26.1, 4.25), (3.8, 26.9, 4.52), "cord")
    # 绳结核心块
    g.box(bk, "knot_core", (-1.2, 25.8, 4.38), (1.2, 27.35, 4.88), "cord_dk")
    g.box(bk, "knot_cross_1", (-1.4, 26.2, 4.45), (1.4, 27.0, 4.84), "cord")
    g.box(bk, "knot_cross_2", (-0.9, 25.6, 4.52), (0.9, 27.6, 4.86), "cord")
    g.box(bk, "knot_center_wrap", (-0.6, 26.0, 4.62), (0.6, 27.2, 5.05), "cord_dk")

    # 右垂带（稍长，不对称骨）
    kr = "knot_tail_r"
    g.box(kr, "tail_r_upper", (0.3, 23.8, 4.45), (1.5, 25.95, 4.82), "linen")
    g.box(kr, "tail_r_mid", (0.5, 21.6, 4.38), (1.7, 23.78, 4.75), "linen_dk")
    g.box(kr, "tail_r_end", (0.7, 19.5, 4.28), (1.8, 21.58, 4.65), "linen")
    g.box(kr, "tail_r_fray", (0.8, 18.6, 4.28), (1.6, 19.48, 4.58), "linen_lt")

    # 左垂带（稍短，不对称骨）
    kl = "knot_tail_l"
    g.box(kl, "tail_l_upper", (-1.6, 24.2, 4.45), (-0.4, 25.95, 4.82), "linen")
    g.box(kl, "tail_l_mid", (-1.8, 22.2, 4.38), (-0.6, 24.18, 4.75), "linen_dk")
    g.box(kl, "tail_l_end", (-1.9, 20.6, 4.28), (-0.8, 22.18, 4.65), "linen")
    g.box(kl, "tail_l_fray", (-1.7, 19.8, 4.28), (-1.0, 20.58, 4.58), "linen_lt")


def part_neck_wrap(g: Gear) -> None:
    """颈围过渡圈：位于 y ∈ [22.2, 23.95]。
    注意：在 y < 24 时，躯干是 x∈[-4,4], z∈[-2,2]；双臂是 x∈[4,8]/[-8,-4], z∈[-2,2]！
    因此前颈只在 z ≤ -2.08 处，后颈在 z ≥ 2.08 处，绝对不在侧面进入双臂空间！
    """
    b = "neck_wrap"
    # 前颈宽折布 (y: 22.2 .. 23.95, z: -3.6 .. -2.08)
    g.box(b, "neck_front_main", (-3.8, 22.2, -3.35), (3.8, 23.95, -2.12), "linen")
    g.box(b, "neck_front_crest", (-3.2, 22.75, -3.62), (3.2, 23.85, -2.62), "linen_lt")
    g.box(b, "neck_front_shadow", (-3.5, 22.25, -3.48), (3.5, 22.72, -2.42), "linen_dk")

    # 后颈包覆 (z ∈ [2.12, 3.25])
    g.box(b, "neck_back_main", (-3.8, 22.6, 2.12), (3.8, 23.95, 3.08), "linen")
    g.box(b, "neck_back_fold", (-3.4, 22.95, 2.58), (3.4, 23.85, 3.25), "linen_dk")


def part_bib_drape(g: Gear) -> None:
    """胸前下摆主布：沿前胸向下垂落 (y ∈ [14.5, 22.8], z ∈ [-2.72, -2.08])。
    收拢在 x ∈ [-3.8, 3.8] 避开双臂。
    """
    bc = "bib_chest"
    # 上胸主挂面 (y: 20.8 .. 22.6)
    g.box(bc, "bib_top_plate", (-3.75, 20.8, -2.62), (3.75, 22.6, -2.15), "linen")
    g.box(bc, "bib_top_crest", (-3.2, 21.4, -2.76), (3.2, 22.45, -2.28), "linen_lt")
    g.pair(bc, "bib_top_pleat", (1.0, 20.9, -2.82), (2.8, 22.5, -2.22), "linen_lt")

    # 中胸主体 (y: 18.2 .. 20.78)
    g.box(bc, "bib_mid_plate", (-3.6, 18.2, -2.58), (3.6, 20.78, -2.12), "linen")
    g.box(bc, "bib_mid_shadow", (-3.4, 18.25, -2.52), (3.4, 19.18, -2.15), "linen_dk")
    g.box(bc, "bib_mid_pleat_c", (-1.2, 18.6, -2.74), (1.2, 20.7, -2.22), "linen_lt")
    g.pair(bc, "bib_mid_pleat", (1.4, 18.4, -2.68), (3.2, 20.65, -2.18), "linen")

    # 下胸中腹收窄与破边波浪 (y: 16.0 .. 18.18)
    g.box(bc, "bib_lower_plate", (-3.2, 16.4, -2.5), (3.2, 18.18, -2.14), "linen")
    g.box(bc, "bib_lower_c", (-1.6, 15.6, -2.46), (1.6, 16.38, -2.12), "linen_dk")
    g.box(bc, "bib_tip_center", (-0.8, 14.8, -2.42), (0.8, 15.58, -2.15), "linen")

    # 右侧参差磨损摆角（不对称骨）
    fr = "bib_fray_r"
    g.box(fr, "fray_r_main", (1.6, 15.4, -2.46), (3.4, 17.5, -2.16), "linen")
    g.box(fr, "fray_r_tip1", (2.2, 14.4, -2.38), (3.2, 15.38, -2.16), "linen_lt")
    g.box(fr, "fray_r_tip2", (1.2, 14.8, -2.36), (2.0, 15.38, -2.15), "linen_dk")
    g.box(fr, "fray_r_fringe", (2.5, 13.7, -2.32), (3.0, 14.38, -2.1), "linen_lt")

    # 左侧参差磨损摆角（不对称骨）
    fl = "bib_fray_l"
    g.box(fl, "fray_l_main", (-3.5, 15.8, -2.46), (-1.8, 17.6, -2.18), "linen_dk")
    g.box(fl, "fray_l_tip1", (-3.1, 14.9, -2.38), (-2.0, 15.98, -2.14), "linen")
    g.box(fl, "fray_l_tip2", (-3.6, 14.2, -2.35), (-2.8, 14.88, -2.15), "linen_lt")
    g.box(fl, "fray_l_fringe", (-2.6, 14.1, -2.32), (-2.2, 14.88, -2.1), "linen_dk")


def part_mask_details(g: Gear) -> None:
    """表面补丁、破洞暗衬与缝线细节（均在 mask_details 不对称骨内）。"""
    d = "mask_details"
    # 面巾正面右下破洞暗衬
    g.box(d, "hole_backing", (1.4, 25.3, -4.75), (2.74, 26.2, -4.52), "void")
    g.box(d, "hole_fray_top", (1.2, 26.1, -4.84), (2.6, 26.35, -4.58), "linen_lt")
    g.box(d, "hole_fray_bot", (1.5, 25.12, -4.82), (2.9, 25.38, -4.58), "linen_dk")
    g.box(d, "hole_stitch_a", (1.8, 25.2, -4.88), (2.05, 26.24, -4.62), "stitch")
    g.box(d, "hole_stitch_b", (2.3, 25.2, -4.88), (2.55, 26.24, -4.62), "stitch")

    # 面巾左鼻侧粗布补丁
    g.box(d, "patch_nose_base", (-2.6, 26.8, -4.82), (-1.0, 28.0, -4.42), "patch")
    g.box(d, "patch_nose_shade", (-2.7, 26.72, -4.75), (-2.32, 27.9, -4.46), "patch_dk")
    g.box(d, "patch_nose_worn", (-2.1, 27.2, -4.88), (-1.3, 27.8, -4.52), "patch_lt")
    # 补丁四边缝线（厚度与宽度均 ≥ 0.22px）
    g.box(d, "patch_stitch_top", (-2.38, 27.95, -4.86), (-1.22, 28.18, -4.6), "stitch")
    g.box(d, "patch_stitch_bot", (-2.38, 26.62, -4.86), (-1.22, 26.85, -4.6), "stitch")
    g.box(d, "patch_stitch_l", (-2.68, 26.9, -4.86), (-2.42, 27.8, -4.6), "stitch")
    g.box(d, "patch_stitch_r", (-1.18, 26.9, -4.86), (-0.93, 27.8, -4.6), "stitch")

    # 胸前下摆中央的大块深色兽皮补丁 (y: 18.8 .. 20.6)
    g.box(d, "chest_patch_base", (-1.8, 18.8, -2.76), (0.8, 20.6, -2.32), "patch")
    g.box(d, "chest_patch_rim", (-1.9, 18.62, -2.72), (0.9, 18.86, -2.35), "patch_dk")
    g.box(d, "chest_patch_worn", (-1.4, 19.2, -2.84), (0.4, 20.2, -2.42), "patch_lt")
    # X 形与周边缝线
    g.box(d, "chest_stitch_h1", (-1.6, 20.35, -2.85), (-0.8, 20.57, -2.42), "stitch")
    g.box(d, "chest_stitch_h2", (-0.2, 20.35, -2.85), (0.6, 20.57, -2.42), "stitch")
    g.box(d, "chest_stitch_b1", (-1.6, 18.85, -2.85), (-0.8, 19.08, -2.42), "stitch")
    g.box(d, "chest_stitch_b2", (-0.2, 18.85, -2.85), (0.6, 19.08, -2.42), "stitch")
    g.box(d, "chest_stitch_v1", (-1.78, 19.3, -2.85), (-1.52, 20.1, -2.42), "stitch")
    g.box(d, "chest_stitch_v2", (0.58, 19.3, -2.85), (0.82, 20.1, -2.42), "stitch")

    # 右侧胸前磨损缝合线
    g.box(d, "seam_r_line", (1.5, 17.5, -2.62), (1.8, 20.7, -2.32), "linen_dk")
    for k, y in enumerate((18.0, 18.7, 19.4, 20.1)):
        g.box(d, f"seam_r_cross_{k}", (1.3, y, -2.7), (2.0, y + 0.35, -2.35), "stitch")


def part_trinket_bone(g: Gear) -> None:
    """右颊挂件：挂绳 + 锈铁小环 + 打磨兽骨针（跨骨硬件，用于穿模门差分自证）。
    在 y < 24 时，为避开双臂 (x∈[4,8], y∈[12,24], z∈[-2,2])，挂件全部前移至 z ≤ -2.12！
    注意：为使穿模门差分自证可靠触发（INTERPEN_BITE=0.55），骨件与铁件尺寸深度设为 ≥ 0.6px。
    各件坐标精细微错开，避免产生共面冲突。
    """
    # 锈铁环挂在 hang_iron_ring 骨骼 (y: 24.12 .. 25.42)
    hr = "hang_iron_ring"
    g.box(hr, "ring_cord", (4.22, 25.42, -2.75), (4.48, 26.5, -2.45), "cord")
    g.box(hr, "iron_ring_top", (4.16, 24.82, -2.95), (4.74, 25.42, -2.25), "rust")
    g.box(hr, "iron_ring_bot", (4.16, 24.12, -2.95), (4.74, 24.62, -2.25), "rust")
    g.box(hr, "iron_ring_side1", (4.14, 24.52, -3.05), (4.76, 24.92, -2.76), "rust")
    g.box(hr, "iron_ring_side2", (4.14, 24.52, -2.44), (4.76, 24.92, -2.15), "rust")

    # 兽骨针挂在 hang_bone_charm 骨骼 (z ∈ [-2.95, -2.2]，避开 z ∈ [-2, 2] 的右臂)
    hb = "hang_bone_charm"
    g.box(hb, "bone_knot", (4.25, 23.8, -2.85), (4.65, 24.1, -2.35), "cord_dk")
    g.box(hb, "bone_head", (4.15, 23.0, -2.92), (4.75, 23.8, -2.22), "bone")
    g.box(hb, "bone_body", (4.25, 21.6, -2.85), (4.68, 23.0, -2.3), "bone")
    g.box(hb, "bone_tip", (4.32, 20.4, -2.78), (4.65, 21.6, -2.38), "bone_lt")
    g.box(hb, "bone_shade", (4.18, 21.8, -2.88), (4.52, 23.2, -2.35), "bone_dk")


PART_FUNCS = {
    "mask_face": part_mask_face,
    "mask_cheeks": part_mask_cheeks,
    "mask_knots": part_mask_knots,
    "neck_wrap": part_neck_wrap,
    "bib_drape": part_bib_drape,
    "mask_details": part_mask_details,
    "trinket_bone": part_trinket_bone,
}

SPEC = GearSpec(
    gear_id="cu_bu_mian_jin",
    model_name="CuBuMianJin",
    title="粗布面巾 / cu_bu_mian_jin",
    mats=MATS,
    bones=BONES,
    parts=PART_FUNCS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    wear_note="挂点：口鼻 / 颈 / 前胸下摆（头面部 y∈[24,28.7] + 颈胸下摆 y∈[13.7,24]）",
    worn_focus=((0.0, 24.0, 0.0), 22.0),
    round_label="round 1/3",
    textured=False,
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
