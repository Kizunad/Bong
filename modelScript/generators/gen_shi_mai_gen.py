"""噬脉根返工版：一条斜着半露地面的扭曲黑红长根（长约 9px，3 段折转，#1a1214），表面盘附暗红脉络（#8a1a2a）与亮红脉结，周围生出数条侧生细须根（#373e32）。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "soil_chasm": (38, 30, 42),           # 负灵域裂隙土
    "root_black_flesh": (26, 18, 20),     # #1a1214 黑红块根
    "pulse_vein_red": (138, 26, 42),      # #8a1a2a 暗红脉络
    "vein_glow": (210, 58, 80),           # 亮红脉结
    "fine_rootlet": (55, 62, 50),         # 侧生细须根
}


def part_chasm_base(rig):
    rig.bone("chasm_base", (0.0, 0.0, 0.0))
    # 底部裂隙黑土座 (高 1.6px，宽 6.4px)
    pad(rig, "chasm_base", "base_crevice_slab", (0.0, 0.12, 0.0), (6.4, 1.4, 6.4), "soil_chasm")
    pad(rig, "chasm_base", "base_fissure_rock", (-1.4, 0.12, 0.6), (2.8, 2.0, 2.6), "soil_chasm")


def part_twisted_root_trunk(rig):
    rig.bone("twisted_root_trunk", (0.0, 0.0, 0.0))
    # 一条斜着半露出地面的扭曲黑红块根（全长约 9px，分 3 段剧烈折转扭曲，高至 8.2px，宽 8.6px）
    
    # 第 1 段：从地表裂缝斜破土而出 (从左后斜向中前拔起，y: 1.2 -> 3.2)
    strand(rig, "twisted_root_trunk", "root_seg_1_lower", (-2.6, 1.2, -1.8), (-1.2, 3.2, -0.6), 0.58, "root_black_flesh")
    pad(rig, "twisted_root_trunk", "root_knot_1", (-1.2, 3.0, -0.6), (2.2, 1.8, 2.2), "root_black_flesh")

    # 第 2 段：扭曲肿胀的主根躯干 (向右前方剧烈拧转上升，y: 3.2 -> 5.8)
    strand(rig, "twisted_root_trunk", "root_seg_2_mid", (-1.2, 3.2, -0.6), (0.8, 5.6, 0.4), 0.54, "root_black_flesh")
    pad(rig, "twisted_root_trunk", "root_knot_2_bulge", (0.2, 4.6, 0.1), (2.8, 2.4, 2.6), "root_black_flesh")

    # 第 3 段：向外上方拱起后下勾的狰狞根头 (y: 5.8 -> 8.2 -> 7.0)
    strand(rig, "twisted_root_trunk", "root_seg_3_arch", (0.8, 5.6, 0.4), (2.6, 7.8, 1.2), 0.46, "root_black_flesh")
    strand(rig, "twisted_root_trunk", "root_seg_3_hook", (2.6, 7.8, 1.2), (3.6, 6.8, 1.8), 0.36, "root_black_flesh")
    pad(rig, "twisted_root_trunk", "root_head_snarl", (2.6, 7.6, 1.2), (1.6, 1.4, 1.6), "root_black_flesh")


def part_pulse_red_veins(rig):
    rig.bone("pulse_red_veins", (0.0, 0.0, 0.0))
    # 贯穿整条扭曲块根上表面与前脊背的暗红血色脉络与亮红脉节 (#8a1a2a / #d23a50)
    # 前脊隆起脉线
    strand(rig, "pulse_red_veins", "vein_seg_1", (-2.2, 1.8, 0.2), (-0.8, 3.8, 0.9), 0.26, "pulse_vein_red")
    pad(rig, "pulse_red_veins", "vein_node_1", (-0.8, 3.8, 0.9), (1.1, 1.1, 1.1), "vein_glow")
    
    strand(rig, "pulse_red_veins", "vein_seg_2", (-0.8, 3.8, 0.9), (0.6, 6.2, 1.6), 0.26, "pulse_vein_red")
    pad(rig, "pulse_red_veins", "vein_node_2", (0.6, 6.2, 1.6), (1.2, 1.2, 1.2), "vein_glow")
    
    strand(rig, "pulse_red_veins", "vein_seg_3", (0.6, 6.2, 1.6), (2.8, 8.4, 1.6), 0.22, "pulse_vein_red")
    pad(rig, "pulse_red_veins", "vein_node_3", (2.8, 8.4, 1.6), (1.0, 1.0, 1.0), "vein_glow")


def part_fine_rootlets(rig):
    rig.bone("fine_rootlets", (0.0, 0.0, 0.0))
    # 块根旁边数根向地面爬抓的细须根 (#373e32)
    strands = (
        ("rootlet_side_l", (-1.2, 2.6, -0.6), (-3.6, 1.2, 0.4), 0.26),
        ("rootlet_side_r", (0.2, 4.2, 0.0), (2.8, 2.4, -1.8), 0.26),
        ("rootlet_front", (0.8, 5.0, 0.4), (1.4, 1.8, 3.2), 0.24),
        ("rootlet_tail", (3.6, 6.8, 1.8), (4.6, 4.8, 2.6), 0.22),
    )
    for name, start, end, r in strands:
        strand(rig, "fine_rootlets", name, start, end, r, "fine_rootlet")


def build():
    return build_rig(MATS, (part_chasm_base, part_twisted_root_trunk, part_pulse_red_veins, part_fine_rootlets))


GATES = PlantGates("噬脉根 / shi_mai_gen")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("ShiMaiGen", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
