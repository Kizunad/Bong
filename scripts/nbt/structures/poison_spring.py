#!/usr/bin/env python3
"""蒸气毒泉 — 主泉 30×10×30 / 小泉 16×8×16

设计：千年丹毒侵染地下水脉涌出的毒泉。

主泉俯视：
                30
    ┌──────────────────────┐
    │     岩石嶙峋外沿       │
    │   ┌──────────────┐   │
    │   │  阶梯式池壁    │   │   三层梯田式：
    │   │  ┌────────┐  │   │     外层 y=1-2  浅水
    │   │  │ 深水池  │  │   │     中层 y=3-5  中水
    │   │  │ + 气泡  │  │   │     内层 y=6-8  深水+气泡柱
    │   │  │  柱     │  │   │
    │   │  └────────┘  │   │
    │   └──────────────┘   │
    │                      │
    └──────────────────────┘

特征：
  - 紫色水（用 purple_stained_glass 水面 + 下方 water）
  - soul_sand 底驱动 bubble_column
  - 岩壁用 blackstone + andesite 混合（火山岩质感）
  - 蒸气用 soul_campfire 产烟
  - 边缘结晶（amethyst_block / purple_terracotta）
  - 废墟化：部分池壁崩塌，水溢出到外圈
"""

from __future__ import annotations
import os, sys, random, math

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from nbt_builder import StructureBuilder

random.seed(42_003)


def _volcanic_rock():
    r = random.random()
    if r < 0.3: return "minecraft:blackstone"
    if r < 0.5: return "minecraft:polished_blackstone"
    if r < 0.7: return "minecraft:andesite"
    return "minecraft:cobblestone"


def build_main() -> StructureBuilder:
    """主毒泉 30×10×30。"""
    W, H, D = 30, 10, 30
    sb = StructureBuilder(W, H, D)
    cx, cz = W // 2, D // 2

    # ── 外沿：不规则岩石圈 ──
    outer_r = 14
    for x in range(W):
        for z in range(D):
            dist = math.sqrt((x - cx) ** 2 + (z - cz) ** 2)
            if outer_r - 2 < dist <= outer_r:
                # 不规则高度（1-3 格）
                noise = random.random()
                h = 1 + int(noise * 2.5)
                for y in range(h):
                    if random.random() > 0.1:
                        sb.set_block(x, y, z, _volcanic_rock())

            # 外沿紫色结晶点缀
            if outer_r - 3 < dist <= outer_r - 1:
                if random.random() < 0.08:
                    sb.set_block(x, 0, z, "minecraft:amethyst_block")

    # ── 三层梯田池 ──

    # 外层 (r=10-12)：浅水区，高 2 格
    tier1_r_outer, tier1_r_inner = 12, 10
    for x in range(W):
        for z in range(D):
            dist = math.sqrt((x - cx) ** 2 + (z - cz) ** 2)
            if tier1_r_inner < dist <= tier1_r_outer:
                sb.set_block(x, 0, z, _volcanic_rock())  # 池底
                sb.set_block(x, 1, z, "minecraft:water")
                if random.random() < 0.15:
                    sb.set_block(x, 2, z, "minecraft:purple_stained_glass")  # 紫色水面

    # 外层池壁
    for x in range(W):
        for z in range(D):
            dist = math.sqrt((x - cx) ** 2 + (z - cz) ** 2)
            if abs(dist - tier1_r_outer) < 0.8:
                for y in range(3):
                    # 东面部分崩塌
                    if x > cx + 5 and random.random() < 0.25:
                        continue
                    if random.random() > 0.08:
                        sb.set_block(x, y, z, _volcanic_rock())

    # 中层 (r=7-10)：中深区，高 4 格
    tier2_r_outer, tier2_r_inner = 10, 7
    for x in range(W):
        for z in range(D):
            dist = math.sqrt((x - cx) ** 2 + (z - cz) ** 2)
            if tier2_r_inner < dist <= tier2_r_outer:
                for y in range(2):
                    sb.set_block(x, y, z, _volcanic_rock())
                sb.set_block(x, 2, z, "minecraft:water")
                sb.set_block(x, 3, z, "minecraft:water")

    # 中层池壁
    for x in range(W):
        for z in range(D):
            dist = math.sqrt((x - cx) ** 2 + (z - cz) ** 2)
            if abs(dist - tier2_r_outer) < 0.8:
                for y in range(4):
                    if random.random() > 0.1:
                        sb.set_block(x, y, z, _volcanic_rock())

    # 内层 (r<7)：深水区 + bubble column
    for x in range(W):
        for z in range(D):
            dist = math.sqrt((x - cx) ** 2 + (z - cz) ** 2)
            if dist <= tier2_r_inner:
                # 深底
                sb.set_block(x, 0, z, "minecraft:soul_sand")  # bubble source
                for y in range(1, 6):
                    sb.set_block(x, y, z, "minecraft:water")
                # 表面紫色玻璃
                if random.random() < 0.3:
                    sb.set_block(x, 6, z, "minecraft:purple_stained_glass")

    # 中央 bubble 柱 + 蒸气
    sb.set_block(cx, 7, cz, "minecraft:soul_campfire", {"lit": "true"})
    sb.set_block(cx - 1, 7, cz, "minecraft:soul_campfire", {"lit": "true"})
    sb.set_block(cx + 1, 7, cz, "minecraft:soul_campfire", {"lit": "true"})

    # 岸边装饰
    for _ in range(20):
        angle = random.uniform(0, 2 * math.pi)
        r = outer_r + random.uniform(0.5, 2)
        x = int(cx + r * math.cos(angle))
        z = int(cz + r * math.sin(angle))
        if 0 <= x < W and 0 <= z < D:
            r_val = random.random()
            if r_val < 0.3:
                sb.set_block(x, 0, z, "minecraft:purple_terracotta")
            elif r_val < 0.6:
                sb.set_block(x, 0, z, "minecraft:gravel")
            else:
                sb.set_block(x, 1, z, "minecraft:dead_bush")

    return sb


def build_small() -> StructureBuilder:
    """小毒泉 16×8×16。"""
    W, H, D = 16, 8, 16
    sb = StructureBuilder(W, H, D)
    cx, cz = W // 2, D // 2

    # 单层池
    pool_r = 5
    wall_r = 6

    # 岩壁
    for x in range(W):
        for z in range(D):
            dist = math.sqrt((x - cx) ** 2 + (z - cz) ** 2)
            if pool_r < dist <= wall_r:
                h = 1 + int(random.random() * 2)
                for y in range(h):
                    if random.random() > 0.12:
                        sb.set_block(x, y, z, _volcanic_rock())
            elif dist <= pool_r:
                sb.set_block(x, 0, z, "minecraft:soul_sand")
                for y in range(1, 4):
                    sb.set_block(x, y, z, "minecraft:water")
                if random.random() < 0.2:
                    sb.set_block(x, 4, z, "minecraft:purple_stained_glass")

    # 蒸气
    sb.set_block(cx, 5, cz, "minecraft:soul_campfire", {"lit": "true"})

    # 紫色结晶点缀
    for _ in range(8):
        angle = random.uniform(0, 2 * math.pi)
        r = wall_r + random.uniform(0.5, 1.5)
        x, z = int(cx + r * math.cos(angle)), int(cz + r * math.sin(angle))
        if 0 <= x < W and 0 <= z < D:
            sb.set_block(x, 0, z, "minecraft:amethyst_block")

    return sb


if __name__ == "__main__":
    structures = [
        ("vapor_poison_spring_main", build_main),
        ("vapor_poison_spring_small", build_small),
    ]
    for name, builder in structures:
        sb = builder()
        out = f"server/structures/dan_zong/{name}.nbt"
        os.makedirs(os.path.dirname(out), exist_ok=True)
        sb.save(out)
        stats = sb.get_stats()
        print(f"{name}: {stats['size'][0]}×{stats['size'][1]}×{stats['size'][2]}, {stats['total_blocks']} blocks")

    # preview
    sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
    from view_structures_3d import BLOCK_COLORS, DEFAULT_COLOR, generate_html
    data = {}
    for name, builder in structures:
        sb = builder()
        blocks = []
        for b in sb._blocks:
            pos = b["pos"]
            pe = sb._palette[b["state"]]
            bname = pe["Name"]
            c = BLOCK_COLORS.get(bname, DEFAULT_COLOR)
            blocks.append({"x": pos[0], "y": pos[1], "z": pos[2], "c": c, "n": bname})
        data[name] = {"blocks": blocks, "size": list(sb.size)}
    html = generate_html(data)
    preview = os.path.join(os.path.dirname(__file__), "..", "nbt", "spring_preview.html")
    with open(preview, "w") as f:
        f.write(html)
    print(f"Preview: {preview}")
