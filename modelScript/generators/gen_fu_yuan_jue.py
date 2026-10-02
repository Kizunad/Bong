"""负元蕨返工版：4~6片羽状蕨叶（主脉两侧多排小叶），叶尖卷成漩涡（#3a2a5a / #7a4ac8），成丛拱展。"""

from __future__ import annotations

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "soil_plate": (38, 28, 42),
    "fern_stalk": (58, 42, 90),       # #3a2a5a
    "fern_frond": (90, 60, 145),
    "fern_pinnule": (122, 74, 200),    # #7a4ac8 亮紫羽片
    "spiral_tip": (175, 115, 245),
}


def part_soil_base(rig):
    rig.bone("soil_base", (0.0, 0.0, 0.0))
    pad(rig, "soil_base", "soil_center", (0.0, 0.12, 0.0), (4.8, 0.85, 4.8), "soil_plate")
    pad(rig, "soil_base", "soil_crown", (0.0, 0.85, 0.0), (3.2, 0.85, 3.2), "fern_stalk")


def part_pinnate_fronds(rig):
    rig.bone("pinnate_fronds", (0.0, 0.0, 0.0))
    # 5 根羽状蕨叶大幅拱展（角度：0, 72, 144, 216, 288 度）
    # 每根蕨叶带主脉 + 两侧对称排布的小羽片 (pinnules)，极富质感与面积感
    angles = [0.0, 72.0, 144.0, 216.0, 288.0]
    lengths = [5.6, 5.8, 5.4, 5.8, 5.6]
    
    for idx, (deg, l) in enumerate(zip(angles, lengths)):
        rad = math.radians(deg)
        cos_a = math.cos(rad)
        sin_a = math.sin(rad)
        
        ortho_x = -sin_a * 0.65
        ortho_z = cos_a * 0.65
        
        # 主脉从根部拔起至中部
        p0 = (cos_a * 0.8, 1.4, sin_a * 0.8)
        p1 = (cos_a * 2.8, 4.2, sin_a * 2.8)
        p2 = (cos_a * 4.6, 5.8, sin_a * 4.6)
        
        strand(rig, "pinnate_fronds", f"frond_stalk_low_{idx}", p0, p1, 0.42, "fern_stalk")
        strand(rig, "pinnate_fronds", f"frond_stalk_high_{idx}", p1, p2, 0.32, "fern_frond")
        
        # 两侧羽片 (小叶裂片，沿主脉两侧伸展)
        # 第一对羽片 (低位)
        strand(rig, "pinnate_fronds", f"frond_pin_low_l_{idx}", 
               (p1[0] - ortho_x * 0.4, p1[1], p1[2] - ortho_z * 0.4),
               (p1[0] - ortho_x * 1.8, p1[1] + 0.3, p1[2] - ortho_z * 1.8), 0.26, "fern_pinnule")
        strand(rig, "pinnate_fronds", f"frond_pin_low_r_{idx}", 
               (p1[0] + ortho_x * 0.4, p1[1], p1[2] + ortho_z * 0.4),
               (p1[0] + ortho_x * 1.8, p1[1] + 0.3, p1[2] + ortho_z * 1.8), 0.26, "fern_pinnule")

        # 第二对羽片 (高位)
        strand(rig, "pinnate_fronds", f"frond_pin_high_l_{idx}", 
               (p2[0] - ortho_x * 0.3, p2[1], p2[2] - ortho_z * 0.3),
               (p2[0] - ortho_x * 1.4, p2[1] + 0.2, p2[2] - ortho_z * 1.4), 0.22, "fern_pinnule")
        strand(rig, "pinnate_fronds", f"frond_pin_high_r_{idx}", 
               (p2[0] + ortho_x * 0.3, p2[1], p2[2] + ortho_z * 0.3),
               (p2[0] + ortho_x * 1.4, p2[1] + 0.2, p2[2] + ortho_z * 1.4), 0.22, "fern_pinnule")


def part_spiral_coils(rig):
    rig.bone("spiral_coils", (0.0, 0.0, 0.0))
    # 5 根蕨叶顶端向内卷回的漩涡卷芽（fiddlehead / spiral coil）
    angles = [0.0, 72.0, 144.0, 216.0, 288.0]
    for idx, deg in enumerate(angles):
        rad = math.radians(deg)
        cos_a = math.cos(rad)
        sin_a = math.sin(rad)
        
        # 卷尖起点 (p2)
        p2 = (cos_a * 4.6, 5.8, sin_a * 4.6)
        # 向内、向下卷成漩涡
        p_loop1 = (cos_a * 5.2, 6.4, sin_a * 5.2)
        p_loop2 = (cos_a * 4.8, 6.8, sin_a * 4.8)
        p_center = (cos_a * 4.4, 6.4, sin_a * 4.4)
        
        strand(rig, "spiral_coils", f"coil_arch_{idx}", p2, p_loop1, 0.26, "fern_pinnule")
        strand(rig, "spiral_coils", f"coil_back_{idx}", p_loop1, p_loop2, 0.22, "spiral_tip")
        pad(rig, "spiral_coils", f"coil_eye_{idx}", (p_center[0], p_center[1], p_center[2]), (0.6, 0.6, 0.6), "spiral_tip")

    # 中央直立微卷的幼芽心 (高 7.2px)
    strand(rig, "spiral_coils", "coil_center_crozier", (0.0, 1.4, 0.0), (0.1, 6.4, 0.1), 0.34, "fern_stalk")
    pad(rig, "spiral_coils", "coil_center_head", (0.1, 6.7, -0.15), (0.9, 0.9, 0.8), "spiral_tip")


def build():
    return build_rig(MATS, (part_soil_base, part_pinnate_fronds, part_spiral_coils))


GATES = PlantGates("负元蕨 / fu_yuan_jue")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("FuYuanJue", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
