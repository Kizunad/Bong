#!/usr/bin/env python3
"""守家傀儡（niche_house_puppet）模型生成器。

物品依据 server/assets/items/niche/house_puppet.toml：「异兽骨与阵石封成的灵龛守家傀儡。触发五次后阵核耗尽。」
1×2 格的器物，放在灵龛门口：矮墩的人形傀儡，骨板做身子，阵石做核和底座。

坐标：1 block = 16px，原点在底座底面中心，+y 向上，正面朝 +z。总高约 16.4px。
分部件：base / legs / torso / core / arms / head / spine，逐件在 part_* 里写，最后 all_cubes() 拼接。

    python3 modelScript/generators/gen_niche_house_puppet.py
"""

from __future__ import annotations

from typing import List

import handheld_common as hc
from handheld_common import Cube, cube

MODEL_ID = "niche_house_puppet"

CORE_FACE_Z = 3.2           # 阵核前面（z），五道刻痕画在它上面
TALLY_COUNT = 5             # 触发五次后阵核耗尽：五道刻痕


def part_base() -> List[Cube]:
    """底座：一块扁石板，四角各一个小石墩，板面刻一圈阵纹槽（暗石细条）。"""
    cubes = [cube("base_plate", (-6.0, 0.0, -4.5), (6.0, 1.5, 4.5), "stone", "base")]
    for sx, tag_x in ((-1, "l"), (1, "r")):
        for sz, tag_z in ((-1, "b"), (1, "f")):
            cx, cz = sx * 5.5, sz * 4.0
            cubes.append(cube(f"base_stud_{tag_z}_{tag_x}", (cx - 0.5, 1.5, cz - 0.5), (cx + 0.5, 2.2, cz + 0.5),
                              "stone_lit", "base"))
    # 阵纹槽：前缘一条、两侧各一条，细而暗，略凸出板面免得共面
    cubes.append(cube("base_rune_front", (-4.0, 1.5, 3.2), (4.0, 1.65, 3.7), "stone_dark", "base"))
    cubes.append(cube("base_rune_l", (-5.2, 1.5, -2.5), (-4.7, 1.65, 2.5), "stone_dark", "base"))
    cubes.append(cube("base_rune_r", (4.7, 1.5, -2.5), (5.2, 1.65, 2.5), "stone_dark", "base"))
    return cubes


def part_legs() -> List[Cube]:
    """两条矮腿：骨板小腿 + 石膝盖 + 向前探的脚掌板。"""
    cubes = []
    for sx, tag in ((-1, "l"), (1, "r")):
        x0, x1 = (-4.0, -1.0) if sx < 0 else (1.0, 4.0)
        cubes.append(cube(f"leg_foot_{tag}", (x0, 1.5, -1.8), (x1, 2.8, 2.8), "bone_dark", "legs"))
        cubes.append(cube(f"leg_shin_{tag}", (x0 + 0.3, 2.8, -1.5), (x1 - 0.3, 5.0, 1.5), "bone_main", "legs"))
        cubes.append(cube(f"leg_knee_{tag}", (x0 + 0.5, 3.8, 1.5), (x1 - 0.5, 5.0, 2.2), "stone", "legs"))
    return cubes


def part_torso() -> List[Cube]:
    """躯干：胯部石环 + 骨板壳（背板、两侧板、顶底板），正面中间敞开露出阵核，两根骨肋横过胸口。"""
    cubes = [cube("hip_ring", (-4.0, 5.0, -2.5), (4.0, 6.0, 2.5), "stone", "torso")]
    cubes.append(cube("torso_back", (-4.8, 6.0, -3.0), (4.8, 11.5, -2.0), "bone_main", "torso"))
    cubes.append(cube("torso_side_l", (-4.8, 6.0, -2.0), (-3.8, 11.5, 3.0), "bone_dark", "torso"))
    cubes.append(cube("torso_side_r", (3.8, 6.0, -2.0), (4.8, 11.5, 3.0), "bone_dark", "torso"))
    cubes.append(cube("torso_top", (-3.8, 11.0, -2.0), (3.8, 11.5, 3.0), "bone_main", "torso"))
    cubes.append(cube("torso_bottom", (-3.8, 6.0, -2.0), (3.8, 6.5, 3.0), "bone_main", "torso"))
    cubes.append(cube("rib_upper", (-3.8, 10.0, 2.4), (3.8, 11.0, 3.0), "bone_lit", "torso"))
    cubes.append(cube("rib_lower", (-3.8, 6.5, 2.4), (3.8, 7.5, 3.0), "bone_lit", "torso"))
    return cubes


def part_core() -> List[Cube]:
    """阵核：胸口一块方石，正面五道竖向刻痕（每触发一次对应一道，耗尽时刻满五道）。"""
    cubes = [cube("core_block", (-2.5, 7.5, -1.0), (2.5, 10.0, CORE_FACE_Z), "stone", "core")]
    for i in range(TALLY_COUNT):
        x = -1.8 + i * 0.9
        cubes.append(cube(f"core_tally_{i}", (x - 0.2, 7.9, CORE_FACE_Z), (x + 0.2, 9.6, CORE_FACE_Z + 0.15),
                          "stone_dark", "core"))
    # 阵核顶、底各一枚石楔，把核卡在壳里
    cubes.append(cube("core_wedge_top", (-1.0, 10.0, 0.0), (1.0, 10.6, 2.4), "stone_lit", "core"))
    return cubes


def part_arms() -> List[Cube]:
    """两条粗短臂：石肩 + 骨板上臂 + 石肘 + 骨板前臂，末端三根骨爪向前下方探出。"""
    cubes = []
    for sx, tag in ((-1, "l"), (1, "r")):
        x0, x1 = (-7.6, -5.0) if sx < 0 else (5.0, 7.6)
        cubes.append(cube(f"shoulder_{tag}", (x0, 10.0, -1.6), (x1, 12.0, 1.6), "stone", "arms"))
        cubes.append(cube(f"arm_upper_{tag}", (x0 + 0.3, 7.6, -1.4), (x1 - 0.3, 10.0, 1.4), "bone_main", "arms"))
        cubes.append(cube(f"arm_elbow_{tag}", (x0 + 0.1, 6.8, -1.6), (x1 - 0.1, 7.6, 1.6), "stone", "arms"))
        cubes.append(cube(f"arm_fore_{tag}", (x0 + 0.3, 4.2, -1.3), (x1 - 0.3, 6.8, 1.5), "bone_dark", "arms"))
        for k in range(3):
            cx = (x0 + x1) / 2 + (k - 1) * 0.8
            cubes.append(cube(f"claw_{tag}_{k}", (cx - 0.25, 3.0, 1.0 + 0.2 * (k % 2)),
                              (cx + 0.25, 4.2, 2.2 + 0.3 * (k % 2)), "bone_lit", "arms"))
    return cubes


def part_head() -> List[Cube]:
    """头：石颈 + 骨板方颅，正面两道中空的眼缝（暗色），顶上一道骨脊。"""
    cubes = [cube("neck", (-1.0, 11.5, -1.0), (1.0, 12.2, 1.0), "stone_dark", "head")]
    cubes.append(cube("skull", (-3.0, 12.2, -3.0), (3.0, 16.4, 3.0), "bone_main", "head"))
    for sx, tag in ((-1, "l"), (1, "r")):
        x0, x1 = (-2.2, -0.8) if sx < 0 else (0.8, 2.2)
        cubes.append(cube(f"eye_slit_{tag}", (x0, 14.0, 3.0), (x1, 15.4, 3.2), "void", "head"))
    cubes.append(cube("skull_crest", (-0.5, 16.4, -2.2), (0.5, 17.0, 2.0), "bone_lit", "head"))
    return cubes


def part_spine() -> List[Cube]:
    """背脊：贴着背板的一条石脊柱，两侧交错伸出五对骨刺。"""
    cubes = [cube("spine_column", (-0.6, 6.5, -4.0), (0.6, 13.0, -3.0), "stone", "spine")]
    for i in range(5):
        y = 7.0 + i * 1.2
        side = -1 if i % 2 == 0 else 1
        x0, x1 = (-2.4, -0.6) if side < 0 else (0.6, 2.4)
        cubes.append(cube(f"spine_spike_{i}", (x0, y, -4.0 - 0.2 * (i % 3)), (x1, y + 0.7, -3.0), "bone_lit", "spine"))
    return cubes


def all_cubes() -> List[Cube]:
    cubes: List[Cube] = []
    for part in (part_base, part_legs, part_torso, part_core, part_arms, part_head, part_spine):
        cubes.extend(part())
    return cubes


VIEWS = (
    ("3/4 FRONT-RIGHT (yaw35 pitch20)", 35.0, 20.0),
    ("SIDE +X (yaw90)", 90.0, 8.0),
    ("BACK -Z (yaw200)", 200.0, 20.0),
)


def main() -> None:
    cubes = all_cubes()
    model = hc.write_model(cubes, MODEL_ID)
    print(f"✓ {MODEL_ID}: {len(cubes)} 个立方体 → {model}")
    print(f"✓ check.png → {hc.write_check(model, MODEL_ID, VIEWS)}")


if __name__ == "__main__":
    main()
