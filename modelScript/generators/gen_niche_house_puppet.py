#!/usr/bin/env python3
"""守家傀儡（niche_house_puppet）模型生成器。

物品依据 server/assets/items/niche/house_puppet.toml：「异兽骨与阵石封成的灵龛守家傀儡。触发五次后阵核耗尽。」
1×2 格的器物，放在灵龛门口：粗糙的石木拼成的人形傀儡，骷髅脸、笼格胸、长臂利爪，立在一块石底座上。

坐标：1 block = 16px，原点在底座底面中心，+y 向上，正面朝 +z。总高约 17px。
分部件：base / legs / torso / core / arms / head / spine，逐件在 part_* 里写，最后 all_cubes() 拼接。
材质只有「石」「木」两种斑驳拼色（见 handheld_common.MOTTLED_PAIRS）加暗石、深坑，底座保留原来的灰石。

    python3 modelScript/generators/gen_niche_house_puppet.py
"""

from __future__ import annotations

from typing import List

import handheld_common as hc
from handheld_common import Cube, cube

MODEL_ID = "niche_house_puppet"

TALLY_COUNT = 5             # 触发五次后阵核耗尽：胯部石环正面刻五道短痕
CAGE_X = (-2.0, 2.0)        # 胸口笼格开口：宽 4px
CAGE_Y = (7.0, 11.0)        # 高 4px
CAGE_BARS = 5               # 竖条数，间距 1px
CAGE_Z = (2.4, 3.0)         # 栅条前后厚度
FINGER_LENGTH = 3.0         # 三指爪：每指 1×1×3（z 向前）


def part_base() -> List[Cube]:
    """底座（保留原样）：一块扁石板，四角各一个小石墩，板面刻一圈阵纹槽。"""
    cubes = [cube("base_plate", (-6.0, 0.0, -4.5), (6.0, 1.5, 4.5), "base_stone", "base")]
    for sx, tag_x in ((-1, "l"), (1, "r")):
        for sz, tag_z in ((-1, "b"), (1, "f")):
            cx, cz = sx * 5.5, sz * 4.0
            cubes.append(cube(f"base_stud_{tag_z}_{tag_x}", (cx - 0.5, 1.5, cz - 0.5), (cx + 0.5, 2.2, cz + 0.5),
                              "stone", "base"))
    cubes.append(cube("base_rune_front", (-4.0, 1.5, 3.2), (4.0, 1.65, 3.7), "stone_dark", "base"))
    cubes.append(cube("base_rune_l", (-5.2, 1.5, -2.5), (-4.7, 1.65, 2.5), "stone_dark", "base"))
    cubes.append(cube("base_rune_r", (4.7, 1.5, -2.5), (5.2, 1.65, 2.5), "stone_dark", "base"))
    return cubes


def part_legs() -> List[Cube]:
    """两条矮腿：木脚板 + 石小腿 + 木膝盖。"""
    cubes = []
    for sx, tag in ((-1, "l"), (1, "r")):
        x0, x1 = (-4.0, -1.0) if sx < 0 else (1.0, 4.0)
        cubes.append(cube(f"leg_foot_{tag}", (x0, 1.5, -1.8), (x1, 2.8, 2.8), "wood", "legs"))
        cubes.append(cube(f"leg_shin_{tag}", (x0 + 0.3, 2.8, -1.5), (x1 - 0.3, 5.0, 1.5), "stone", "legs"))
        cubes.append(cube(f"leg_knee_{tag}", (x0 + 0.5, 3.8, 1.5), (x1 - 0.5, 5.0, 2.2), "wood", "legs"))
    return cubes


def part_torso() -> List[Cube]:
    """躯干：胯部石环（正面五道刻痕）+ 木背板 + 石侧板，顶底板；正面是笼格（竖条栅格），栅后是阵符石。"""
    cubes = [cube("hip_ring", (-4.0, 5.0, -2.5), (4.0, 6.0, 2.5), "stone", "torso")]
    for i in range(TALLY_COUNT):
        x = -1.6 + i * 0.8
        cubes.append(cube(f"hip_tally_{i}", (x - 0.2, 5.2, 2.5), (x + 0.2, 5.8, 2.65), "stone_dark", "torso"))
    cubes.append(cube("torso_back", (-4.8, 6.0, -3.0), (4.8, 11.5, -2.0), "wood", "torso"))
    cubes.append(cube("torso_side_l", (-4.8, 6.0, -2.0), (-3.8, 11.5, 3.0), "stone", "torso"))
    cubes.append(cube("torso_side_r", (3.8, 6.0, -2.0), (4.8, 11.5, 3.0), "stone", "torso"))
    cubes.append(cube("torso_top", (-3.8, 11.0, -2.0), (3.8, 11.5, 3.0), "wood", "torso"))
    cubes.append(cube("torso_bottom", (-3.8, 6.0, -2.0), (3.8, 6.5, 3.0), "wood", "torso"))
    return cubes


def part_core() -> List[Cube]:
    """胸口笼格：4×4 的竖条栅格（5 根木条，间距 1px），上下各一道横框；格里嵌一块刻纹石符。"""
    cubes = [cube("cage_rail_bottom", (-3.8, 6.5, CAGE_Z[0]), (3.8, CAGE_Y[0], CAGE_Z[1]), "wood", "core")]
    for i in range(CAGE_BARS):
        x = CAGE_X[0] + i * (CAGE_X[1] - CAGE_X[0]) / (CAGE_BARS - 1)
        cubes.append(cube(f"cage_bar_{i}", (x - 0.25, CAGE_Y[0], CAGE_Z[0]), (x + 0.25, CAGE_Y[1], CAGE_Z[1]),
                          "wood", "core"))
    # 栅后的阵符石：一块方石，正面三条暗刻线拼成符纹（竖一横二），略凸出避免共面
    cubes.append(cube("rune_stone", (-1.7, 7.4, -1.0), (1.7, 10.6, 2.0), "stone", "core"))
    cubes.append(cube("rune_line_v", (-0.2, 7.8, 2.0), (0.2, 10.2, 2.15), "stone_dark", "core"))
    cubes.append(cube("rune_line_h_upper", (-1.2, 9.4, 2.05), (1.2, 9.8, 2.2), "stone_dark", "core"))
    cubes.append(cube("rune_line_h_lower", (-0.9, 8.0, 2.05), (0.9, 8.4, 2.2), "stone_dark", "core"))
    return cubes


def _guard_plates(tag: str, outward: int, x_edge: float) -> List[Cube]:
    """前臂外侧一排 4 片尖状护板：每片 = 贴臂的宽片 + 向外收窄的尖，越往下越短。"""
    cubes = []
    for i, (y0, reach) in enumerate(((6.1, 1.4), (5.3, 1.2), (4.5, 1.0), (3.9, 0.8))):
        lo_x, hi_x = (x_edge, x_edge + 0.5) if outward > 0 else (x_edge - 0.5, x_edge)
        cubes.append(cube(f"guard_{tag}_{i}", (lo_x, y0, -1.0), (hi_x, y0 + 0.6, 1.0), "stone", "arms"))
        tip_lo, tip_hi = (hi_x, hi_x + reach) if outward > 0 else (lo_x - reach, lo_x)
        cubes.append(cube(f"guard_{tag}_{i}_tip", (tip_lo, y0 + 0.1, -0.4), (tip_hi, y0 + 0.5, 0.4), "stone", "arms"))
    return cubes


def part_arms() -> List[Cube]:
    """两条长臂：石肩 + 木上臂 + 石肘 + 木前臂，垂到大腿中部；前臂外侧四片尖护板；末端三指爪。"""
    cubes = []
    for sx, tag in ((-1, "l"), (1, "r")):
        x0, x1 = (-8.0, -5.0) if sx < 0 else (5.0, 8.0)
        cubes.append(cube(f"shoulder_{tag}", (x0, 10.8, -1.6), (x1, 12.6, 1.6), "stone", "arms"))
        cubes.append(cube(f"arm_upper_{tag}", (x0 + 0.3, 7.6, -1.4), (x1 - 0.3, 10.8, 1.4), "wood", "arms"))
        cubes.append(cube(f"arm_elbow_{tag}", (x0 + 0.1, 6.8, -1.6), (x1 - 0.1, 7.6, 1.6), "stone", "arms"))
        cubes.append(cube(f"arm_fore_{tag}", (x0 + 0.3, 3.8, -1.3), (x1 - 0.3, 6.8, 1.5), "wood", "arms"))
        cubes.extend(_guard_plates(tag, sx, x0 + 0.3 if sx < 0 else x1 - 0.3))
        center = (x0 + x1) / 2
        for k, offset in enumerate((-0.95, 0.0, 0.95)):
            cubes.append(cube(f"claw_{tag}_{k}", (center + offset - 0.45, 2.8, 1.5),
                              (center + offset + 0.45, 3.8, 1.5 + FINGER_LENGTH), "stone", "arms"))
    return cubes


def part_head() -> List[Cube]:
    """骷髅头：石颈 + 颅骨（正面一块带两个 2×2 深眼窝的前板）+ 单独的下颌（下沉 1px，和颅骨留 0.5px 缝）。"""
    cubes = [cube("neck", (-1.0, 11.5, -1.0), (1.0, 12.0, 1.0), "stone_dark", "head")]
    # 下颌：比颅骨窄一点，下沉；与颅骨底（13.7）之间留 0.5px 缝
    cubes.append(cube("jaw", (-2.6, 12.0, -2.0), (2.6, 13.2, 3.0), "wood", "head"))
    for i in range(3):
        x = -1.5 + i * 1.5
        cubes.append(cube(f"jaw_tooth_{i}", (x - 0.35, 13.2, 2.4), (x + 0.35, 13.7, 2.9), "stone", "head"))
    cubes.append(cube("skull_back", (-3.0, 13.7, -3.0), (3.0, 16.9, 2.0), "stone", "head"))
    # 前板（z 2..3）：下沿整条、上沿整条、眼窝高度上留出两个 2×2 的洞（中间鼻梁 + 两侧颧）
    cubes.append(cube("skull_brow", (-3.0, 16.4, 2.0), (3.0, 16.9, 3.0), "stone", "head"))
    cubes.append(cube("skull_cheek", (-3.0, 13.7, 2.0), (3.0, 14.4, 3.0), "stone", "head"))
    for name, (x0, x1) in (("outer_l", (-3.0, -2.5)), ("nose", (-0.5, 0.5)), ("outer_r", (2.5, 3.0))):
        cubes.append(cube(f"skull_{name}", (x0, 14.4, 2.0), (x1, 16.4, 3.0), "stone", "head"))
    # 眼窝：2×2 的坑，底部是一块深色（凹进 0.8px）
    for tag, (x0, x1) in (("l", (-2.5, -0.5)), ("r", (0.5, 2.5))):
        cubes.append(cube(f"eye_socket_{tag}", (x0, 14.4, 2.0), (x1, 16.4, 2.2), "void", "head"))
    cubes.append(cube("skull_crest", (-0.5, 16.9, -2.2), (0.5, 17.5, 2.0), "wood", "head"))
    return cubes


def part_spine() -> List[Cube]:
    """背脊：贴着背板的一条石脊柱，两侧交错伸出五对木刺。"""
    cubes = [cube("spine_column", (-0.6, 6.5, -4.0), (0.6, 13.0, -3.0), "stone", "spine")]
    for i in range(5):
        y = 7.0 + i * 1.2
        side = -1 if i % 2 == 0 else 1
        x0, x1 = (-2.4, -0.6) if side < 0 else (0.6, 2.4)
        cubes.append(cube(f"spine_spike_{i}", (x0, y, -4.0 - 0.2 * (i % 3)), (x1, y + 0.7, -3.0), "wood", "spine"))
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
