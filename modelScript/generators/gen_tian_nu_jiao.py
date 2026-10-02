"""天怒椒返工版：一根粗大倒挂的黑红弯辣椒（长约 8px，截面自 3.2px 逐段收细至 0.9px 尖端并向一侧大弯弧），色 #3a0a08 / #8a1a10，表面贯穿橙红发光裂纹（#ff6a1a），顶上一段绿梗与短茎（#37552a）。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "charred_soil": (42, 30, 30),          # 焦土碎石座
    "green_calyx": (55, 85, 42),           # 绿梗与短茎
    "pepper_dark_blood": (58, 10, 8),      # #3a0a08 压暗焦黑红椒身
    "pepper_ember": (148, 48, 18),         # 焦红过渡
    "flame_crack": (255, 106, 26),         # #ff6a1a 橙红发光裂纹
}


def part_scorched_base(rig):
    rig.bone("scorched_base", (0.0, 0.0, 0.0))
    # 焦土碎石底座 (高 1.6px，宽 5.8px)
    pad(rig, "scorched_base", "base_charred_slab", (0.0, 0.12, 0.0), (5.8, 1.4, 5.8), "charred_soil")


def part_green_stem_calyx(rig):
    rig.bone("green_stem_calyx", (0.0, 0.0, 0.0))
    # 挺立向上拱起倒扣的短茎与花梗 (y: 1.2 -> 9.0)
    strand(rig, "green_stem_calyx", "calyx_stem_main", (-0.8, 1.2, -0.6), (-1.2, 7.2, -0.8), 0.44, "green_calyx")
    strand(rig, "green_stem_calyx", "calyx_arch_over", (-1.2, 7.2, -0.8), (0.2, 8.8, 0.0), 0.38, "green_calyx")
    strand(rig, "green_stem_calyx", "calyx_hook_down", (0.2, 8.8, 0.0), (0.6, 7.8, 0.4), 0.36, "green_calyx")
    # 倒扣椒蒂萼片
    pad(rig, "green_stem_calyx", "calyx_cap_ring", (0.6, 7.6, 0.4), (2.8, 0.8, 2.8), "green_calyx")
    pad(rig, "green_stem_calyx", "calyx_sepal_l", (-0.6, 7.4, 0.4), (1.1, 0.5, 1.1), "green_calyx")
    pad(rig, "green_stem_calyx", "calyx_sepal_r", (1.6, 7.4, 0.4), (1.1, 0.5, 1.1), "green_calyx")


def part_curved_chili_pepper(rig):
    rig.bone("curved_chili_pepper", (0.0, 0.0, 0.0))
    # 一根粗大倒挂的黑红弯辣椒（长约 8px，截面从 3.6px 逐级收缩到 1.1px，加粗厚实，向右前方向大弯弧）
    # 1. 椒头与上段 (加粗至 3.6x3.6，焦黑深血红 #3a0a08，y: 6.0 -> 7.6)
    pad(rig, "curved_chili_pepper", "pepper_seg_0_head", (0.6, 6.2, 0.4), (3.6, 1.6, 3.6), "pepper_dark_blood")
    strand(rig, "curved_chili_pepper", "pepper_seg_1_upper", (0.6, 6.6, 0.4), (1.2, 5.2, 0.7), 0.60, "pepper_dark_blood")

    # 2. 弯曲中段 (加粗至截面 2.8x2.8，黑红主体 #3a0a08，y: 3.8 -> 5.2)
    strand(rig, "curved_chili_pepper", "pepper_seg_2_mid", (1.2, 5.2, 0.7), (2.2, 3.8, 1.3), 0.52, "pepper_dark_blood")

    # 3. 弯曲下段 (截面 2.0x2.0，微透焦红 #943012，大幅向外侧弯挑，y: 2.4 -> 3.8)
    strand(rig, "curved_chili_pepper", "pepper_seg_3_lower", (2.2, 3.8, 1.3), (3.4, 2.6, 2.0), 0.40, "pepper_ember")

    # 4. 尖锐尖端 (截面 1.2x1.2，向外上微翘，y: 1.8 -> 2.6)
    strand(rig, "curved_chili_pepper", "pepper_seg_4_tip", (3.4, 2.6, 2.0), (4.5, 2.1, 2.4), 0.28, "pepper_ember")


def part_flame_cracks(rig):
    rig.bone("flame_cracks", (0.0, 0.0, 0.0))
    # 贯穿弯椒弧背凸面与椒尖的橙红发光裂纹 (#ff6a1a)
    strand(rig, "flame_cracks", "crack_upper_back", (0.6, 7.2, 2.25), (1.2, 5.6, 2.5), 0.24, "flame_crack")
    strand(rig, "flame_cracks", "crack_mid_curve", (1.2, 5.6, 2.5), (2.3, 4.0, 2.85), 0.22, "flame_crack")
    strand(rig, "flame_cracks", "crack_tip_glow", (2.3, 4.0, 2.85), (3.8, 2.8, 2.9), 0.20, "flame_crack")
    pad(rig, "flame_cracks", "crack_apex_spark", (4.5, 2.1, 2.5), (0.85, 0.85, 0.85), "flame_crack")


def build():
    return build_rig(MATS, (part_scorched_base, part_green_stem_calyx, part_curved_chili_pepper, part_flame_cracks))


GATES = PlantGates("天怒椒 / tian_nu_jiao")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("TianNuJiao", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
