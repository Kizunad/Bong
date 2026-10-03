"""断戟刺：扎在古战场金属遗物缝里的暗血荆刺，斜插破损断戟，密布暗血荆棘与尖锐倒钩血刺。"""

from __future__ import annotations

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "rusted_metal": (58, 48, 44),       # 古战场生锈金属残片
    "thorn_dark_blood": (72, 28, 25),   # 暗血荆身
    "thorn_sharp_red": (165, 42, 36),   # 锐利血刺
    "metal_barb": (185, 160, 142),      # 锈铁断戟尖端白刃
    "spite_aura": (135, 30, 45),
}


def part_rusted_relic(rig):
    rig.bone("rusted_relic", (0.0, 0.0, 0.0))
    # 底部古战场遗留的厚原生铁断刃与戟杆碎块 (高 4.5px，斜插在地)
    pad(rig, "rusted_relic", "relic_ground_base", (0.0, 0.12, 0.0), (5.2, 0.85, 5.2), "rusted_metal")
    # 斜插的大号断戟铁头 (y: 0.6 -> 5.5)
    strand(rig, "rusted_relic", "relic_broken_halberd", (-1.2, 0.6, -0.8), (1.4, 4.8, 0.8), 0.75, "rusted_metal")
    # 破戟残锋金属反光尖端
    pad(rig, "rusted_relic", "relic_blade_shard", (1.6, 5.2, 0.9), (1.2, 1.4, 0.6), "metal_barb", rotation=(-12.0, 32.0, 18.0))


def part_blood_brambles(rig):
    rig.bone("blood_brambles", (0.0, 0.0, 0.0))
    # 从金属缝隙中挣脱盘绕而出的粗壮暗血荆棘条带（成团簇拥，高至 9.2px，阔 10.4px）
    # 1. 缠绕主荆条 A (顺戟身向上缠绕攀附)
    strand(rig, "blood_brambles", "bramble_main_a1", (-1.4, 0.8, 0.4), (-0.4, 3.8, 0.9), 0.48, "thorn_dark_blood")
    strand(rig, "blood_brambles", "bramble_main_a2", (-0.4, 3.8, 0.9), (0.8, 6.8, 0.2), 0.40, "thorn_dark_blood")
    strand(rig, "blood_brambles", "bramble_main_a3", (0.8, 6.8, 0.2), (1.6, 9.4, -0.4), 0.32, "thorn_dark_blood")

    # 2. 侧向外拱荆条 B (向左后侧大弧外拱)
    strand(rig, "blood_brambles", "bramble_arch_b1", (-0.8, 1.2, -0.6), (-2.8, 3.6, -1.8), 0.44, "thorn_dark_blood")
    strand(rig, "blood_brambles", "bramble_arch_b2", (-2.8, 3.6, -1.8), (-4.4, 5.6, -2.4), 0.36, "thorn_dark_blood")
    strand(rig, "blood_brambles", "bramble_arch_b3", (-4.4, 5.6, -2.4), (-5.0, 7.8, -2.8), 0.28, "thorn_dark_blood")

    # 3. 侧向外拱荆条 C (向右前侧大弧外展)
    strand(rig, "blood_brambles", "bramble_arch_c1", (0.6, 1.2, 0.6), (2.8, 3.4, 2.2), 0.44, "thorn_dark_blood")
    strand(rig, "blood_brambles", "bramble_arch_c2", (2.8, 3.4, 2.2), (4.2, 5.8, 3.0), 0.36, "thorn_dark_blood")
    strand(rig, "blood_brambles", "bramble_arch_c3", (4.2, 5.8, 3.0), (4.8, 7.6, 3.4), 0.28, "thorn_dark_blood")


def part_barbed_spikes(rig):
    rig.bone("barbed_spikes", (0.0, 0.0, 0.0))
    # 荆条上密布的鲜红锋锐倒钩刺芒（怨念回响所在，#a52a24）
    spikes = (
        ("spike_tip_top", (1.6, 9.4, -0.4), (2.2, 10.6, -0.6), 0.22, "thorn_sharp_red"),
        ("spike_tip_l", (-5.0, 7.8, -2.8), (-5.8, 8.8, -3.2), 0.20, "thorn_sharp_red"),
        ("spike_tip_r", (4.8, 7.6, 3.4), (5.5, 8.6, 3.9), 0.20, "thorn_sharp_red"),
        # 沿身横向倒钩血刺 (怨念暗血光泽)
        ("spike_barb_1", (-0.4, 4.2, 1.1), (0.2, 5.0, 1.8), 0.22, "spite_aura"),
        ("spike_barb_2", (0.8, 7.2, 0.3), (0.2, 8.1, 0.9), 0.22, "spite_aura"),
        ("spike_barb_3", (-2.8, 4.0, -1.9), (-3.6, 4.6, -1.2), 0.22, "spite_aura"),
        ("spike_barb_4", (2.8, 3.8, 2.3), (3.6, 4.4, 1.6), 0.22, "spite_aura"),
    )
    for name, start, end, r, mat in spikes:
        strand(rig, "barbed_spikes", name, start, end, r, mat)


def build():
    return build_rig(MATS, (part_rusted_relic, part_blood_brambles, part_barbed_spikes))


GATES = PlantGates("断戟刺 / duan_ji_ci")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("DuanJiCi", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
