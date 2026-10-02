"""刺舌蒿返工版：深绿尖叶簇成刺球状（#2a3a28 / #4a5a3a）、底座压着坚硬碎岩、一滴黄汁（#e0a020）下垂。"""

from __future__ import annotations

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "base_rock": (48, 52, 50),
    "leaf_dark": (42, 58, 40),    # #2a3a28
    "leaf_mid": (74, 90, 58),     # #4a5a3a
    "leaf_spine": (118, 142, 70),
    "amber_drop": (224, 160, 32), # #e0a020 垂滴
}


def part_rock_base(rig):
    rig.bone("rock_base", (0.0, 0.0, 0.0))
    # 底部压着的粗粝碎岩石块 (高 2.5px，宽 6.2px)
    pad(rig, "rock_base", "rock_core", (0.0, 0.12, 0.0), (5.8, 2.0, 5.8), "base_rock")
    pad(rig, "rock_base", "rock_ledge_l", (-1.8, 0.12, 0.6), (2.8, 2.4, 2.6), "base_rock")
    pad(rig, "rock_base", "rock_ledge_r", (1.6, 0.12, -0.6), (2.6, 1.8, 2.8), "base_rock")


def part_spiky_sphere(rig):
    rig.bone("spiky_sphere", (0.0, 0.0, 0.0))
    # 密集放射尖叶簇成的厚重刺球团 (高耸至 8.8px，占据中心，像刺猬球)
    # 球核体 (y: 2.2 -> 6.5)
    pad(rig, "spiky_sphere", "spiky_core", (0.0, 2.2, 0.0), (4.4, 4.4, 4.4), "leaf_dark")
    pad(rig, "spiky_sphere", "spiky_mid_c", (0.0, 2.8, 0.0), (5.2, 3.2, 5.2), "leaf_mid")
    
    # 放射刺状尖叶（多圈立体发散展开：下层下斜、中层平展、上层上挺）
    # 1. 中层平展尖叶 (8 个方向)
    for i in range(8):
        deg = i * 45.0
        rad = math.radians(deg)
        cos_a = math.cos(rad)
        sin_a = math.sin(rad)
        p_base = (cos_a * 2.2, 4.4, sin_a * 2.2)
        p_tip = (cos_a * 4.6, 4.2, sin_a * 4.6)
        strand(rig, "spiky_sphere", f"spiky_mid_thorn_{i}", p_base, p_tip, 0.32, "leaf_spine")

    # 2. 上层斜挺刺尖 (6 个方向，向上斜指，y: 4.8 -> 8.6)
    for i in range(6):
        deg = i * 60.0 + 15.0
        rad = math.radians(deg)
        cos_a = math.cos(rad)
        sin_a = math.sin(rad)
        p_base = (cos_a * 1.6, 5.2, sin_a * 1.6)
        p_tip = (cos_a * 3.8, 8.2, sin_a * 3.8)
        strand(rig, "spiky_sphere", f"spiky_high_thorn_{i}", p_base, p_tip, 0.28, "leaf_spine")

    # 3. 顶心直立长刺
    strand(rig, "spiky_sphere", "spiky_top_spine", (0.0, 5.8, 0.0), (0.0, 8.8, 0.0), 0.30, "leaf_spine")


def part_hanging_drop(rig):
    rig.bone("hanging_drop", (0.0, 0.0, 0.0))
    # 从右前方尖叶末端悬挂垂落的一滴晶莹鲜黄色药汁 (#e0a020)
    # 挂在 x=3.4, z=1.8 的外伸尖刺下
    strand(rig, "hanging_drop", "drop_filament", (3.2, 4.2, 1.6), (3.2, 2.6, 1.6), 0.16, "amber_drop")
    pad(rig, "hanging_drop", "drop_bead", (3.2, 1.8, 1.6), (0.75, 0.95, 0.75), "amber_drop")


def build():
    return build_rig(MATS, (part_rock_base, part_spiky_sphere, part_hanging_drop))


GATES = PlantGates("刺舌蒿 / ci_she_hao")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("CiSheHao", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
