"""清浊草返工版：五片大宽叶交替舒展（青翠 #2ac0b0 与 鲜橙 #e07a2a 交替），中心挺拔紫色茎（#7a4ac8）。"""

from __future__ import annotations

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "soil_plate": (42, 38, 45),
    "stem_purple": (122, 74, 200),   # #7a4ac8 中心紫茎
    "clear_teal": (42, 192, 176),    # #2ac0b0 青瓣
    "turbid_orange": (224, 122, 42), # #e07a2a 橙瓣
    "core_eye": (185, 235, 225),
}


def part_soil_base(rig):
    rig.bone("soil_base", (0.0, 0.0, 0.0))
    # 底部杂色土台
    pad(rig, "soil_base", "soil_center", (0.0, 0.12, 0.0), (4.8, 0.85, 4.8), "soil_plate")
    pad(rig, "soil_base", "soil_ring", (0.0, 0.85, 0.0), (3.4, 0.85, 3.4), "soil_plate")


def part_purple_stalk(rig):
    rig.bone("purple_stalk", (0.0, 0.0, 0.0))
    # 中心直立向上挺拔的紫色粗主茎 (从地面 y=1.2 一直挺到 y=8.2，非常醒目)
    strand(rig, "purple_stalk", "stalk_lower", (0.0, 1.20, 0.0), (0.0, 4.50, 0.0), 0.48, "stem_purple")
    strand(rig, "purple_stalk", "stalk_upper", (0.0, 4.50, 0.0), (0.0, 7.80, 0.0), 0.38, "stem_purple")
    pad(rig, "purple_stalk", "stalk_crown", (0.0, 7.80, 0.0), (1.4, 1.4, 1.4), "stem_purple")


def part_five_broad_petals(rig):
    rig.bone("five_broad_petals", (0.0, 0.0, 0.0))
    # 五片巨大厚实宽叶放射舒展，青 (#2ac0b0) 与橙 (#e07a2a) 交替！
    # 72° 均布，每片由双层体素铺就（宽达 3.4px，向外伸展 5.2px，整体跨度 11.2px，高 7.0px）
    angles = [0.0, 72.0, 144.0, 216.0, 288.0]
    # 交替色：0青, 1橙, 2青, 3橙, 4双色拼合
    colors = ["clear_teal", "turbid_orange", "clear_teal", "turbid_orange", "clear_teal"]
    alt_colors = ["clear_teal", "turbid_orange", "clear_teal", "turbid_orange", "turbid_orange"]

    for idx, deg in enumerate(angles):
        rad = math.radians(deg)
        cos_a = math.cos(rad)
        sin_a = math.sin(rad)
        
        ortho_x = -sin_a * 0.75
        ortho_z = cos_a * 0.75
        
        mat1 = colors[idx]
        mat2 = alt_colors[idx]
        
        # 宽叶基部连接点 (y=3.5)
        p_base = (cos_a * 0.8, 3.5, sin_a * 0.8)
        # 宽叶中部拱起处 (y=5.2)
        p_mid = (cos_a * 3.4, 5.2, sin_a * 3.4)
        # 宽叶尖端向外下垂展平 (y=4.5)
        p_tip = (cos_a * 5.4, 4.5, sin_a * 5.4)
        
        # 铺出双排厚宽叶
        # 左扇面
        strand(rig, "five_broad_petals", f"petal_in_l_{idx}", 
               (p_base[0] - ortho_x, p_base[1], p_base[2] - ortho_z),
               (p_mid[0] - ortho_x * 0.9, p_mid[1], p_mid[2] - ortho_z * 0.9), 0.52, mat1)
        strand(rig, "five_broad_petals", f"petal_out_l_{idx}", 
               (p_mid[0] - ortho_x * 0.9, p_mid[1], p_mid[2] - ortho_z * 0.9),
               (p_tip[0], p_tip[1], p_tip[2]), 0.44, mat1)

        # 右扇面
        strand(rig, "five_broad_petals", f"petal_in_r_{idx}", 
               (p_base[0] + ortho_x, p_base[1], p_base[2] + ortho_z),
               (p_mid[0] + ortho_x * 0.9, p_mid[1], p_mid[2] + ortho_z * 0.9), 0.52, mat2)
        strand(rig, "five_broad_petals", f"petal_out_r_{idx}", 
               (p_mid[0] + ortho_x * 0.9, p_mid[1], p_mid[2] + ortho_z * 0.9),
               (p_tip[0], p_tip[1], p_tip[2]), 0.44, mat2)


def part_central_eye(rig):
    rig.bone("central_eye", (0.0, 0.0, 0.0))
    # 紫茎顶端托着的中和清澈灵珠灵眼 (y: 8.2 -> 9.4)
    pad(rig, "central_eye", "eye_core", (0.0, 8.4, 0.0), (1.2, 1.2, 1.2), "core_eye")
    pad(rig, "central_eye", "eye_halo", (0.0, 8.8, 0.0), (0.7, 0.7, 0.7), "clear_teal")


def build():
    return build_rig(MATS, (part_soil_base, part_purple_stalk, part_five_broad_petals, part_central_eye))


GATES = PlantGates("清浊草 / qing_zhuo_cao")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("QingZhuoCao", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
