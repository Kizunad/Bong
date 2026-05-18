#!/usr/bin/env python3
"""露天炼丹大鼎 — 24×16×24

设计参照：中国古代青铜鼎（殷墟司母戊鼎形制）+ 末法残土千年废墟

结构分层（从下到上）：

  y=0-2   ：火坑（地面挖下去的凹坑，散落灰烬/煤炭/soul sand）
  y=3-4   ：鼎足（四足方鼎，每足 2×2，分布在四角）
  y=5-9   ：鼎腹（主体容器，厚壁中空，内壁有药渍/残留物）
  y=10-11 ：鼎沿（向外翻出的宽沿，上有纹饰方块）
  y=12-14 ：鼎耳（两侧各一个突出的环形把手）
  y=15    ：残留物（鼎内结晶/蜘蛛网/枯草——千年未清的药渣）

俯视（y=5-9 鼎腹截面）：

      0         12        24
      ├─────────┼─────────┤
   0  │    ·  ·   ·  ·    │
      │  ╔═══════════╗    │
   4  │  ║           ║    │  外壁 = polished_blackstone_bricks
      │  ║   内腔     ║    │  内壁 = 药渍 soul_soil / purple_terracotta
   8  │  ║  (中空)    ║    │
      │  ║           ║    │
  12  │  ╚═══════════╝    │
      │                    │
  16  │   散落碎片/灰烬    │
      │                    │
      └────────────────────┘

废墟化：
  - 一条腿断裂（西南角），鼎体微倾
  - 鼎沿局部缺失（东面 30% 破损）
  - 鼎内长满蛛网 + 枯草
  - 火坑积水（部分格子用 water）
  - 周围散落碎石/煤炭
"""

from __future__ import annotations
import os, sys, random, math

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from nbt_builder import StructureBuilder

W, H, D = 24, 16, 24
random.seed(42_001)


def build() -> StructureBuilder:
    sb = StructureBuilder(W, H, D)
    cx, cz = W // 2, D // 2  # 中心 (12, 12)

    # ═══════════════════════════════════════
    # Layer 0-2: 火坑（挖入地面的凹坑）
    # ═══════════════════════════════════════
    # 圆形凹坑，半径 8，深 3 格
    pit_r = 8
    for x in range(W):
        for z in range(D):
            dist = math.sqrt((x - cx) ** 2 + (z - cz) ** 2)
            if dist <= pit_r:
                # 坑底
                if dist <= pit_r - 2:
                    # 内圈：灰烬/煤/soul sand
                    r = random.random()
                    if r < 0.15:
                        sb.set_block(x, 0, z, "minecraft:soul_sand")
                    elif r < 0.3:
                        sb.set_block(x, 0, z, "minecraft:coal_block")
                    elif r < 0.4:
                        sb.set_block(x, 0, z, "minecraft:water")  # 积水
                    else:
                        sb.set_block(x, 0, z, "minecraft:soul_soil")
                else:
                    # 外圈：坑壁石头
                    sb.set_block(x, 0, z, "minecraft:cobblestone")

                # 坑壁 y=1-2（只在边缘）
                if pit_r - 2 <= dist <= pit_r:
                    for y in range(1, 3):
                        if random.random() > 0.15:  # 15% 缺口
                            block = random.choice([
                                "minecraft:cobblestone", "minecraft:mossy_cobblestone",
                                "minecraft:stone_bricks"
                            ])
                            sb.set_block(x, y, z, block)

    # 火坑中央残留的火焰/烟
    sb.set_block(cx, 1, cz, "minecraft:soul_campfire", {"lit": "true"})
    sb.set_block(cx - 1, 1, cz, "minecraft:campfire", {"lit": "false"})
    sb.set_block(cx + 1, 1, cz, "minecraft:campfire", {"lit": "false"})

    # ═══════════════════════════════════════
    # Layer 3-4: 鼎足（四足方鼎）
    # ═══════════════════════════════════════
    # 四足位置：以鼎腹外壁为准，四角各一足
    belly_r_outer = 6  # 鼎腹外半径
    foot_positions = [
        (cx - 5, cz - 5),  # 西南
        (cx + 4, cz - 5),  # 东南
        (cx - 5, cz + 4),  # 西北
        (cx + 4, cz + 4),  # 东北
    ]

    for i, (fx, fz) in enumerate(foot_positions):
        # 西南角（index 0）断裂——只剩 1 格高
        is_broken_leg = (i == 0)
        leg_height = 1 if is_broken_leg else 2

        for dy in range(leg_height):
            for dx in range(2):
                for dz in range(2):
                    block = "minecraft:polished_blackstone"
                    if is_broken_leg and dy == 0:
                        block = "minecraft:cracked_polished_blackstone_bricks"
                    sb.set_block(fx + dx, 3 + dy, fz + dz, block)

    # 断腿处散落碎块
    sb.set_block(cx - 7, 3, cz - 7, "minecraft:polished_blackstone_slab", {"type": "bottom"})
    sb.set_block(cx - 6, 3, cz - 8, "minecraft:cobblestone")
    sb.set_block(cx - 8, 3, cz - 6, "minecraft:gravel")

    # ═══════════════════════════════════════
    # Layer 5-9: 鼎腹（主体容器）
    # ═══════════════════════════════════════
    # 椭圆形鼎腹，外壁 polished_blackstone_bricks，内壁药渍
    belly_r_inner = 4  # 内径

    for y in range(5, 10):
        # 鼎腹从下到上略微收窄（下宽上窄）
        y_progress = (y - 5) / 4  # 0 到 1
        r_outer = belly_r_outer - y_progress * 0.5
        r_inner = belly_r_inner - y_progress * 0.3

        for x in range(W):
            for z in range(D):
                dist = math.sqrt((x - cx) ** 2 + (z - cz) ** 2)

                if r_inner < dist <= r_outer:
                    # 外壁
                    if random.random() < 0.08:
                        # 破损
                        sb.set_block(x, y, z, "minecraft:cracked_polished_blackstone_bricks")
                    else:
                        # 正常壁体——交替使用两种方块做"纹饰"效果
                        if (x + z + y) % 3 == 0:
                            sb.set_block(x, y, z, "minecraft:chiseled_polished_blackstone")
                        else:
                            sb.set_block(x, y, z, "minecraft:polished_blackstone_bricks")

                elif dist <= r_inner and y == 5:
                    # 鼎底——药渍残留
                    r = random.random()
                    if r < 0.3:
                        sb.set_block(x, y, z, "minecraft:soul_soil")  # 药渣
                    elif r < 0.5:
                        sb.set_block(x, y, z, "minecraft:purple_terracotta")  # 丹毒沉淀
                    else:
                        sb.set_block(x, y, z, "minecraft:polished_blackstone")

    # 鼎内残留物（y=6-8）
    for x in range(cx - 3, cx + 4):
        for z in range(cz - 3, cz + 4):
            dist = math.sqrt((x - cx) ** 2 + (z - cz) ** 2)
            if dist <= belly_r_inner - 1:
                r = random.random()
                if r < 0.12:
                    sb.set_block(x, 6, z, "minecraft:cobweb")
                elif r < 0.2:
                    sb.set_block(x, 6, z, "minecraft:dead_bush")

    # ═══════════════════════════════════════
    # Layer 10-11: 鼎沿（翻沿，向外扩展 2 格）
    # ═══════════════════════════════════════
    rim_r = belly_r_outer + 2

    for y in [10, 11]:
        for x in range(W):
            for z in range(D):
                dist = math.sqrt((x - cx) ** 2 + (z - cz) ** 2)

                if belly_r_outer - 0.5 < dist <= rim_r:
                    # 东面（x > cx+3）30% 概率缺失（破损）
                    if x > cx + 3 and random.random() < 0.3:
                        continue

                    if y == 10:
                        # 下层沿：阶梯方块做翻沿效果
                        # 根据方向选择朝向
                        dx, dz = x - cx, z - cz
                        angle = math.atan2(dz, dx)
                        if -0.785 < angle <= 0.785:
                            facing = "west"
                        elif 0.785 < angle <= 2.356:
                            facing = "south"
                        elif -2.356 < angle <= -0.785:
                            facing = "north"
                        else:
                            facing = "east"
                        sb.set_block(x, y, z, "minecraft:polished_blackstone_stairs",
                                    {"facing": facing, "half": "top"})
                    else:
                        # 上层沿：半砖
                        if random.random() < 0.85:
                            sb.set_block(x, y, z, "minecraft:polished_blackstone_slab",
                                        {"type": "top"})

    # ═══════════════════════════════════════
    # Layer 12-14: 鼎耳（两侧环形把手）
    # ═══════════════════════════════════════
    # 东西两侧各一个"耳"，用 wall/fence 做环形
    for ear_x_center in [cx - belly_r_outer - 1, cx + belly_r_outer + 1]:
        # 耳柱
        sb.set_block(ear_x_center, 10, cz - 1, "minecraft:polished_blackstone_wall")
        sb.set_block(ear_x_center, 10, cz + 1, "minecraft:polished_blackstone_wall")
        sb.set_block(ear_x_center, 11, cz - 1, "minecraft:polished_blackstone_wall")
        sb.set_block(ear_x_center, 11, cz + 1, "minecraft:polished_blackstone_wall")
        # 耳顶横梁
        sb.set_block(ear_x_center, 12, cz - 1, "minecraft:polished_blackstone_wall")
        sb.set_block(ear_x_center, 12, cz, "minecraft:polished_blackstone_wall")
        sb.set_block(ear_x_center, 12, cz + 1, "minecraft:polished_blackstone_wall")

    # 东侧耳部分断裂
    sb.set_block(cx + belly_r_outer + 1, 12, cz, "minecraft:air")

    # ═══════════════════════════════════════
    # 周围地面：散落物
    # ═══════════════════════════════════════
    for _ in range(30):
        x = random.randint(1, W - 2)
        z = random.randint(1, D - 2)
        dist = math.sqrt((x - cx) ** 2 + (z - cz) ** 2)
        if dist > pit_r + 1:
            r = random.random()
            if r < 0.3:
                sb.set_block(x, 0, z, "minecraft:gravel")
            elif r < 0.5:
                sb.set_block(x, 0, z, "minecraft:cobblestone")
            elif r < 0.7:
                sb.set_block(x, 0, z, "minecraft:coal_ore")
            else:
                sb.set_block(x, 1, z, "minecraft:dead_bush")

    # ═══════════════════════════════════════
    # 装饰精修（Round 2）
    # ═══════════════════════════════════════

    # 鼎身外壁藤蔓（苔藓攀爬效果）
    for angle in range(0, 360, 20):
        x = cx + int(belly_r_outer * math.cos(math.radians(angle)))
        z = cz + int(belly_r_outer * math.sin(math.radians(angle)))
        if 0 <= x < W and 0 <= z < D and random.random() < 0.5:
            vine_h = random.randint(1, 3)
            for dy in range(vine_h):
                facing = "north" if z > cz else "south"
                sb.set_block(x, 8 - dy, z, "minecraft:vine", {facing: "true"})

    # 鼎底火坑边缘蘑菇/枯草
    for angle in range(0, 360, 25):
        x = cx + int((pit_r - 1) * math.cos(math.radians(angle)))
        z = cz + int((pit_r - 1) * math.sin(math.radians(angle)))
        if 0 <= x < W and 0 <= z < D:
            if random.random() < 0.4:
                sb.set_block(x, 1, z, random.choice([
                    "minecraft:brown_mushroom", "minecraft:red_mushroom",
                    "minecraft:dead_bush"
                ]))

    # 鼎耳上挂的链条残骸
    for ear_x in [cx - belly_r_outer - 1, cx + belly_r_outer + 1]:
        sb.set_block(ear_x, 11, cz, "minecraft:chain")

    # 鼎内药渣结晶（紫水晶簇——千年丹毒结晶化）
    for _ in range(5):
        dx = random.randint(-2, 2)
        dz = random.randint(-2, 2)
        if abs(dx) + abs(dz) <= 3:
            sb.set_block(cx + dx, 6, cz + dz, "minecraft:amethyst_cluster",
                        {"facing": "up"})

    # 祭台痕迹（鼎前方 4 格处的供台残骸）
    for dx in range(-2, 3):
        sb.set_block(cx + dx, 0, cz - pit_r - 2, "minecraft:polished_blackstone_slab",
                    {"type": "bottom"})
    sb.set_block(cx, 1, cz - pit_r - 2, "minecraft:flower_pot")
    sb.set_block(cx - 1, 1, cz - pit_r - 2, "minecraft:candle", {"candles": "3", "lit": "false"})

    # 断腿处碎片更丰富
    sb.set_block(cx - 7, 3, cz - 5, "minecraft:cracked_polished_blackstone_bricks")
    sb.set_block(cx - 8, 3, cz - 5, "minecraft:gravel")
    sb.set_block(cx - 6, 4, cz - 6, "minecraft:polished_blackstone_slab", {"type": "bottom"})

    return sb


if __name__ == "__main__":
    sb = build()

    out = "server/structures/dan_zong/ruined_open_furnace.nbt"
    os.makedirs(os.path.dirname(out), exist_ok=True)
    sb.save(out)

    stats = sb.get_stats()
    print(f"炼丹大鼎 v2: {stats['size'][0]}×{stats['size'][1]}×{stats['size'][2]}")
    print(f"Blocks: {stats['total_blocks']}, Palette: {stats['palette_size']}")
    for bt, cnt in sorted(stats["block_counts"].items(), key=lambda x: -x[1])[:10]:
        print(f"  {bt}: {cnt}")

    # 3D preview
    sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
    from view_structures_3d import BLOCK_COLORS, DEFAULT_COLOR, generate_html
    import json

    blocks = []
    for b in sb._blocks:
        pos = b["pos"]
        pe = sb._palette[b["state"]]
        name = pe["Name"]
        c = BLOCK_COLORS.get(name, DEFAULT_COLOR)
        blocks.append({"x": pos[0], "y": pos[1], "z": pos[2], "c": c, "n": name})

    data = {"ruined_open_furnace_v2": {"blocks": blocks, "size": [W, H, D]}}
    html = generate_html(data)
    preview = os.path.join(os.path.dirname(__file__), "..", "nbt", "furnace_preview.html")
    os.makedirs(os.path.dirname(preview), exist_ok=True)
    with open(preview, "w") as f:
        f.write(html)
    print(f"\nPreview: {preview}")
