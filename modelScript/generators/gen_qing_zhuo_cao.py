"""清浊草：叶五瓣，生于两界交界，半清青翠半浊灰黑，中央中和眼。"""

from __future__ import annotations

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "boundary_soil": (42, 40, 44),
    "stem_neutral": (55, 62, 58),
    "clear_cyan": (78, 155, 142),
    "turbid_dark": (34, 32, 38),
    "neutral_eye": (145, 195, 185),
}


def part_soil_base(rig):
    rig.bone("soil_base", (0.0, 0.0, 0.0))
    # 交界处的杂色泥台
    pad(rig, "soil_base", "soil_center", (0.0, 0.12, 0.0), (4.4, 0.75, 4.4), "boundary_soil")
    pad(rig, "soil_base", "soil_f", (0.2, 0.12, 1.6), (2.8, 0.55, 1.2), "boundary_soil")
    pad(rig, "soil_base", "soil_b", (-0.2, 0.12, -1.6), (2.8, 0.55, 1.2), "boundary_soil")


def part_central_stem(rig):
    rig.bone("central_stem", (0.0, 0.0, 0.0))
    # 短粗直立中央分瓣主轴 (y: 0.55 -> 4.50)
    strand(rig, "central_stem", "stem_base", (0.0, 0.55, 0.0), (0.0, 2.50, 0.0), 0.38, "stem_neutral")
    strand(rig, "central_stem", "stem_neck", (0.0, 2.50, 0.0), (0.0, 4.50, 0.0), 0.32, "stem_neutral")


def part_five_petals(rig):
    rig.bone("five_petals", (0.0, 0.0, 0.0))
    # 严格 5 瓣放射舒展（72° 均布）
    # 每瓣由清半边 (clear_cyan) 和浊半边 (turbid_dark) 双片合抱组成！
    angles = [0.0, 72.0, 144.0, 216.0, 288.0]
    petal_len = 3.6
    for idx, deg in enumerate(angles):
        rad = math.radians(deg)
        cos_a = math.cos(rad)
        sin_a = math.sin(rad)

        # 瓣基 (y=4.5)
        p_base = (0.0, 4.5, 0.0)
        # 瓣尖外展微拱起 (y=4.5 -> y=5.8 -> y=5.2)
        p_mid = (cos_a * 1.9, 5.8, sin_a * 1.9)
        p_tip = (cos_a * petal_len, 5.2, sin_a * petal_len)

        # 清半边 (顺时针偏置微量角度)
        strand(rig, "five_petals", f"petal_clear_in_{idx}", p_base, p_mid, 0.26, "clear_cyan")
        strand(rig, "five_petals", f"petal_clear_out_{idx}", p_mid, p_tip, 0.22, "clear_cyan")

        # 浊半边 (并排紧贴清半边)
        ortho_x = -sin_a * 0.28
        ortho_z = cos_a * 0.28
        p_mid_t = (p_mid[0] + ortho_x, 5.75, p_mid[2] + ortho_z)
        p_tip_t = (p_tip[0] + ortho_x * 0.8, 5.15, p_tip[2] + ortho_z * 0.8)
        strand(rig, "five_petals", f"petal_turbid_in_{idx}", p_base, p_mid_t, 0.24, "turbid_dark")
        strand(rig, "five_petals", f"petal_turbid_out_{idx}", p_mid_t, p_tip_t, 0.20, "turbid_dark")


def part_neutral_eye(rig):
    rig.bone("neutral_eye", (0.0, 0.0, 0.0))
    # 五瓣交汇处的两界中和核心晶眼 (y: 4.60 -> 5.60)
    pad(rig, "neutral_eye", "core_eye_center", (0.0, 4.65, 0.0), (1.1, 0.95, 1.1), "neutral_eye")
    pad(rig, "neutral_eye", "core_eye_apex", (0.0, 5.55, 0.0), (0.6, 0.55, 0.6), "clear_cyan")


def build():
    return build_rig(MATS, (part_soil_base, part_central_stem, part_five_petals, part_neutral_eye))


GATES = PlantGates("清浊草 / qing_zhuo_cao")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("QingZhuoCao", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
