"""噬脉根：生于负灵域浅层裂隙的剧毒块根，肿胀紫黑根茎（#442637 / #733e5f）盘结成团，生出多道狰狞蠕行根须与剧毒结节。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "negative_soil": (36, 28, 42),        # 负灵域裂隙黑土
    "root_flesh_dark": (68, 38, 55),      # #442637 肿胀紫黑根肉
    "root_flesh_mid": (115, 62, 95),      # #733e5f 噬脉紫红根身
    "root_tendril": (168, 92, 142),       # #a85c8e 蠕行须根
    "toxic_pustule": (215, 160, 205),     # #d7a0cd 剧毒脓疱
}


def part_chasm_soil(rig):
    rig.bone("chasm_soil", (0.0, 0.0, 0.0))
    # 裂隙碎石黑土底座 (高 2.2px，宽 6.6px)
    pad(rig, "chasm_soil", "soil_chasm_slab", (0.0, 0.12, 0.0), (6.4, 1.8, 6.4), "negative_soil")
    pad(rig, "chasm_soil", "soil_crevice_ledge", (-1.6, 0.12, 0.6), (3.2, 2.4, 2.6), "negative_soil")


def part_swollen_rhizome(rig):
    rig.bone("swollen_rhizome", (0.0, 0.0, 0.0))
    # 狰狞肿胀的中央主块根与根茎结节（生姜状/块根状厚实成团，y: 1.6 -> 7.8，体量丰满）
    # 1. 核心大块茎
    pad(rig, "swollen_rhizome", "rhizome_core_tuber", (0.0, 2.2, 0.0), (4.8, 4.2, 4.4), "root_flesh_dark")
    # 2. 侧向肿胀分节肉瘤
    pad(rig, "swollen_rhizome", "rhizome_tuber_l", (-1.8, 2.8, -0.4), (3.4, 3.4, 3.2), "root_flesh_mid")
    pad(rig, "swollen_rhizome", "rhizome_tuber_r", (1.6, 2.6, 0.6), (3.4, 3.6, 3.2), "root_flesh_mid")
    # 3. 顶端膨大主根头 (y: 5.8 -> 7.6)
    pad(rig, "swollen_rhizome", "rhizome_crown_knob", (0.2, 5.8, -0.2), (3.2, 2.0, 3.2), "root_flesh_dark")


def part_creeping_tendrils(rig):
    rig.bone("creeping_tendrils", (0.0, 0.0, 0.0))
    # 从块茎向四周裂隙延伸蠕行的强劲爪状根爪与须根（展宽 10.4px，高至 8.4px）
    strands = (
        # 前爪延伸根
        ("tendril_claw_f1", (0.2, 2.6, 1.8), (0.4, 1.4, 4.2), 0.44, "root_flesh_mid"),
        ("tendril_claw_f2", (0.4, 1.4, 4.2), (0.6, 0.8, 5.2), 0.30, "root_tendril"),
        # 后抓地根
        ("tendril_claw_b1", (-0.2, 2.8, -1.8), (-0.4, 1.6, -3.8), 0.44, "root_flesh_mid"),
        ("tendril_claw_b2", (-0.4, 1.6, -3.8), (-0.5, 0.9, -4.8), 0.30, "root_tendril"),
        # 左侧大弓根
        ("tendril_claw_l1", (-2.4, 3.6, 0.2), (-4.2, 4.8, 0.6), 0.42, "root_flesh_mid"),
        ("tendril_claw_l2", (-4.2, 4.8, 0.6), (-5.0, 2.2, 1.0), 0.28, "root_tendril"),
        # 右侧上扬触须根
        ("tendril_claw_r1", (2.2, 3.6, -0.2), (3.8, 5.2, -0.6), 0.42, "root_flesh_mid"),
        ("tendril_claw_r2", (3.8, 5.2, -0.6), (4.8, 6.8, -0.8), 0.28, "root_tendril"),
        # 顶端破土刺苗
        ("tendril_top_spine", (0.2, 7.4, -0.2), (0.4, 8.8, -0.3), 0.30, "root_tendril"),
    )
    for name, start, end, r, mat in strands:
        strand(rig, "creeping_tendrils", name, start, end, r, mat)


def part_toxic_nodes(rig):
    rig.bone("toxic_nodes", (0.0, 0.0, 0.0))
    # 块根表面散布的剧毒发光脓疱与经脉渗出结节 (#d7a0cd)
    nodes = (
        ("node_pustule_f", (0.8, 4.2, 2.2), (1.1, 1.1, 0.8), "toxic_pustule"),
        ("node_pustule_l", (-2.6, 4.4, -0.8), (0.9, 0.9, 0.9), "toxic_pustule"),
        ("node_pustule_r", (2.6, 4.0, 1.2), (0.9, 0.9, 0.9), "toxic_pustule"),
        ("node_pustule_top", (0.0, 6.8, 1.1), (0.8, 0.8, 0.8), "toxic_pustule"),
    )
    for name, pos, size, mat in nodes:
        pad(rig, "toxic_nodes", name, pos, size, mat)


def build():
    return build_rig(MATS, (part_chasm_soil, part_swollen_rhizome, part_creeping_tendrils, part_toxic_nodes))


GATES = PlantGates("噬脉根 / shi_mai_gen")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("ShiMaiGen", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
