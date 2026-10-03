"""灵草返工版：深蓝绿发光宽叶 + 底部圆球根，5~6片向外张开宽叶，叶面亮蓝裂纹。"""

from __future__ import annotations

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "bulb_base": (30, 48, 45),
    "leaf_dark": (30, 74, 90),     # #1e4a5a
    "leaf_mid": (58, 138, 160),     # #3a8aa0
    "leaf_light": (105, 195, 215),
    "blue_crack": (138, 224, 255),  # #8ae0ff 亮蓝细纹
}


def part_bulb_root(rig):
    rig.bone("bulb_root", (0.0, 0.0, 0.0))
    # 底部饱满圆球根 (占据底部中心，宽约 5.5px，高约 3.8px)
    pad(rig, "bulb_root", "bulb_core", (0.0, 0.12, 0.0), (5.2, 3.2, 5.2), "bulb_base")
    pad(rig, "bulb_root", "bulb_waist_x", (0.0, 0.6, 0.0), (5.8, 2.2, 4.4), "bulb_base")
    pad(rig, "bulb_root", "bulb_waist_z", (0.0, 0.6, 0.0), (4.4, 2.2, 5.8), "bulb_base")
    pad(rig, "bulb_root", "bulb_neck", (0.0, 2.8, 0.0), (3.6, 1.2, 3.6), "leaf_dark")


def part_spread_broad_leaves(rig):
    rig.bone("spread_broad_leaves", (0.0, 0.0, 0.0))
    # 5 片向外大弧度舒展张开的厚实宽叶（成团，占满画面中心，全宽达 10.5px，高 8.5px）
    # 角度均匀分布在四周：0, 72, 144, 216, 288 度
    angles = [0.0, 72.0, 144.0, 216.0, 288.0]
    for idx, deg in enumerate(angles):
        rad = math.radians(deg)
        cos_a = math.cos(rad)
        sin_a = math.sin(rad)
        
        # 宽叶内段 (从球根颈部拔出，厚 0.6px，宽 2.4px)
        # 用两条并排 strand 铺成宽叶面
        ortho_x = -sin_a * 0.75
        ortho_z = cos_a * 0.75
        
        p_base = (cos_a * 0.8, 3.2, sin_a * 0.8)
        p_mid = (cos_a * 3.4, 6.2, sin_a * 3.4)
        p_tip = (cos_a * 5.2, 8.2, sin_a * 5.2)
        
        # 左半片
        strand(rig, "spread_broad_leaves", f"leaf_in_l_{idx}", 
               (p_base[0] - ortho_x, p_base[1], p_base[2] - ortho_z),
               (p_mid[0] - ortho_x * 0.8, p_mid[1], p_mid[2] - ortho_z * 0.8), 0.55, "leaf_dark")
        strand(rig, "spread_broad_leaves", f"leaf_out_l_{idx}", 
               (p_mid[0] - ortho_x * 0.8, p_mid[1], p_mid[2] - ortho_z * 0.8),
               (p_tip[0], p_tip[1], p_tip[2]), 0.42, "leaf_mid")
               
        # 右半片
        strand(rig, "spread_broad_leaves", f"leaf_in_r_{idx}", 
               (p_base[0] + ortho_x, p_base[1], p_base[2] + ortho_z),
               (p_mid[0] + ortho_x * 0.8, p_mid[1], p_mid[2] + ortho_z * 0.8), 0.55, "leaf_dark")
        strand(rig, "spread_broad_leaves", f"leaf_out_r_{idx}", 
               (p_mid[0] + ortho_x * 0.8, p_mid[1], p_mid[2] + ortho_z * 0.8),
               (p_tip[0], p_tip[1], p_tip[2]), 0.42, "leaf_mid")

    # 中央挺立的幼心厚叶 (高耸至 y=9.2)
    pad(rig, "spread_broad_leaves", "leaf_center_core", (0.0, 3.6, 0.0), (2.2, 5.0, 2.2), "leaf_mid")
    pad(rig, "spread_broad_leaves", "leaf_center_tip", (0.0, 7.8, 0.0), (1.4, 1.4, 1.4), "leaf_light")


def part_spirit_cracks(rig):
    rig.bone("spirit_cracks", (0.0, 0.0, 0.0))
    # 5 片大宽叶中央凸起的亮蓝发光裂纹与能量脉线 (#8ae0ff)
    angles = [0.0, 72.0, 144.0, 216.0, 288.0]
    for idx, deg in enumerate(angles):
        rad = math.radians(deg)
        cos_a = math.cos(rad)
        sin_a = math.sin(rad)
        
        p_base = (cos_a * 0.9, 3.6, sin_a * 0.9)
        p_mid = (cos_a * 3.4, 6.55, sin_a * 3.4)
        p_tip = (cos_a * 4.9, 8.45, sin_a * 4.9)
        
        strand(rig, "spirit_cracks", f"crack_in_{idx}", p_base, p_mid, 0.22, "blue_crack")
        strand(rig, "spirit_cracks", f"crack_out_{idx}", p_mid, p_tip, 0.18, "blue_crack")


def build():
    return build_rig(MATS, (part_bulb_root, part_spread_broad_leaves, part_spirit_cracks))


GATES = PlantGates("灵草 / spirit_grass")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("SpiritGrass", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
