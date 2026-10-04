"""针芥子返工版：圆球形刺团（像海胆），深墨绿球形主体（#1e2a1e）+ 向外放射一圈亮绿短刺（#6ab04a），底部短茎。"""

from __future__ import annotations

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "crag_base": (45, 48, 44),
    "urchin_body": (30, 42, 30),     # #1e2a1e 深墨绿球体
    "needle_green": (106, 176, 74),   # #6ab04a 亮绿尖刺
    "needle_tip": (168, 224, 110),
    "short_stem": (48, 62, 42),
}


def part_crag_base(rig):
    rig.bone("crag_base", (0.0, 0.0, 0.0))
    # 底部石隙与短粗茎基
    pad(rig, "crag_base", "crag_stone_base", (0.0, 0.12, 0.0), (5.2, 0.85, 5.2), "crag_base")
    pad(rig, "crag_base", "crag_rock_l", (-1.6, 0.12, 0.5), (2.4, 1.4, 2.2), "crag_base")
    pad(rig, "crag_base", "crag_rock_r", (1.6, 0.12, -0.5), (2.4, 1.2, 2.2), "crag_base")
    # 底部短茎 (高 1.6px，支撑海胆球体)
    strand(rig, "crag_base", "crag_short_stem", (0.0, 0.85, 0.0), (0.0, 2.80, 0.0), 0.55, "short_stem")


def part_urchin_body(rig):
    rig.bone("urchin_body", (0.0, 0.0, 0.0))
    # 海胆状饱满浑圆球形主体 (y: 2.4 -> 7.4，高 5.0px，直径 5.4px)
    pad(rig, "urchin_body", "urchin_core_cube", (0.0, 2.8, 0.0), (4.8, 4.4, 4.8), "urchin_body")
    pad(rig, "urchin_body", "urchin_bulge_x", (0.0, 3.2, 0.0), (5.6, 3.6, 4.2), "urchin_body")
    pad(rig, "urchin_body", "urchin_bulge_z", (0.0, 3.2, 0.0), (4.2, 3.6, 5.6), "urchin_body")
    pad(rig, "urchin_body", "urchin_top_dome", (0.0, 6.8, 0.0), (3.4, 1.2, 3.4), "urchin_body")


def part_radial_spikes(rig):
    rig.bone("radial_spikes", (0.0, 0.0, 0.0))
    # 向四周 360° 全方位立体放射的亮绿短刺 (#6ab04a & #a8e06e)，像极了绿海胆！
    # 球心位置：(0.0, 5.0, 0.0)，半径约 2.6
    
    # 1. 赤道水平放射圈 (8 根刺，平伸)
    for i in range(8):
        deg = i * 45.0
        rad = math.radians(deg)
        cos_a = math.cos(rad)
        sin_a = math.sin(rad)
        p_base = (cos_a * 2.4, 5.0, sin_a * 2.4)
        p_tip = (cos_a * 4.4, 5.0, sin_a * 4.4)
        strand(rig, "radial_spikes", f"spike_eq_{i}", p_base, p_tip, 0.28, "needle_green")
        pad(rig, "radial_spikes", f"spike_eq_tip_{i}", (p_tip[0], p_tip[1], p_tip[2]), (0.45, 0.45, 0.45), "needle_tip")

    # 2. 北半球斜上放射圈 (6 根刺，45° 斜上)
    for i in range(6):
        deg = i * 60.0 + 30.0
        rad = math.radians(deg)
        cos_a = math.cos(rad)
        sin_a = math.sin(rad)
        p_base = (cos_a * 1.8, 6.2, sin_a * 1.8)
        p_tip = (cos_a * 3.6, 7.8, sin_a * 3.6)
        strand(rig, "radial_spikes", f"spike_up_{i}", p_base, p_tip, 0.26, "needle_green")
        pad(rig, "radial_spikes", f"spike_up_tip_{i}", (p_tip[0], p_tip[1], p_tip[2]), (0.42, 0.42, 0.42), "needle_tip")

    # 3. 顶极直立刺 (垂直向上)
    strand(rig, "radial_spikes", "spike_top_polar", (0.0, 7.2, 0.0), (0.0, 9.4, 0.0), 0.28, "needle_green")
    pad(rig, "radial_spikes", "spike_top_tip", (0.0, 9.5, 0.0), (0.45, 0.45, 0.45), "needle_tip")

    # 4. 南半球斜下放射圈 (4 根刺，斜向下扎入石缝)
    for i in range(4):
        deg = i * 90.0 + 45.0
        rad = math.radians(deg)
        cos_a = math.cos(rad)
        sin_a = math.sin(rad)
        p_base = (cos_a * 2.1, 3.8, sin_a * 2.1)
        p_tip = (cos_a * 3.8, 2.6, sin_a * 3.8)
        strand(rig, "radial_spikes", f"spike_down_{i}", p_base, p_tip, 0.26, "needle_green")


def build():
    return build_rig(MATS, (part_crag_base, part_urchin_body, part_radial_spikes))


GATES = PlantGates("针芥子 / zhen_jie_zi")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("ZhenJieZi", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
