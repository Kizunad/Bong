#!/usr/bin/env python3
"""药圃石篱 — 40×4×40（内环）/ 48×4×48（外环）

设计：上古百草门药圃，九宫格分区种植。

俯视布局（40×40 内环版）：

    ┌────┬────┬────┐
    │ 蜕 │ 步 │ 兽 │   每格 ~12×12 内种植区
    │ 骨 │ 道 │ 心 │   石篱围墙 2 格高
    │ 藤 │    │ 草 │   格内 podzol 种灵草
    ├────┼────┼────┤
    │ 步 │水池│ 步 │   中央水池 6×6（灌溉/仪式）
    │ 道 │    │ 道 │   步道 2 格宽 stone brick
    ├────┼────┼────┤
    │ 龙 │ 步 │ 续 │
    │ 鳞 │ 道 │ 元 │
    │ 苔 │    │ 蕊 │
    └────┴────┴────┘

细节：
  - 石篱墙壁用 mossy_stone_bricks + cracked 做废墟感
  - 步道用 stone_brick_slab 铺设，有缺口
  - 水池有 soul_sand 底（千年灵水变质）
  - 四角有石灯笼残骸
  - 格内 podzol 上长 crimson_roots / warped_roots 代替灵草
"""

from __future__ import annotations
import os, sys, random, math

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from nbt_builder import StructureBuilder

random.seed(42_002)


def _wall_block():
    r = random.random()
    if r < 0.25: return "minecraft:mossy_stone_bricks"
    if r < 0.35: return "minecraft:cracked_stone_bricks"
    return "minecraft:stone_bricks"


def build_inner() -> StructureBuilder:
    """内环药圃 40×4×40。"""
    W, H, D = 40, 4, 40
    sb = StructureBuilder(W, H, D)

    path_w = 2       # 步道宽
    cell_size = 12   # 每格种植区（含墙）
    wall_h = 2       # 石篱高度

    # 地面全铺
    for x in range(W):
        for z in range(D):
            sb.set_block(x, 0, z, "minecraft:coarse_dirt")

    # 九宫格：3×3 区域
    # 步道网格（十字步道）
    # 水平步道 z = 13-14, 25-26
    # 垂直步道 x = 13-14, 25-26
    path_zs = [(13, 14), (25, 26)]
    path_xs = [(13, 14), (25, 26)]

    for x in range(W):
        for z in range(D):
            is_path = False
            for (pz1, pz2) in path_zs:
                if pz1 <= z <= pz2:
                    is_path = True
            for (px1, px2) in path_xs:
                if px1 <= x <= px2:
                    is_path = True

            if is_path:
                if random.random() < 0.85:
                    sb.set_block(x, 0, z, "minecraft:stone_bricks")
                else:
                    sb.set_block(x, 0, z, "minecraft:mossy_cobblestone")

    # 种植区（九宫格中的 4 个角格 + 中央水池）
    plant_cells = [
        # (x_start, z_start, plant_type)
        (1, 1, "minecraft:crimson_roots"),       # 西南：蜕骨藤
        (27, 1, "minecraft:warped_roots"),        # 东南：兽心草
        (1, 27, "minecraft:twisting_vines"),      # 西北：龙鳞苔
        (27, 27, "minecraft:crimson_fungus"),      # 东北：续元蕊
    ]

    for (cx, cz, plant) in plant_cells:
        # 石篱围墙
        for dy in range(wall_h):
            for x in range(cx, cx + cell_size):
                if random.random() > 0.12:
                    sb.set_block(x, 1 + dy, cz, _wall_block())
                if random.random() > 0.12:
                    sb.set_block(x, 1 + dy, cz + cell_size - 1, _wall_block())
            for z in range(cz, cz + cell_size):
                if random.random() > 0.12:
                    sb.set_block(cx, 1 + dy, z, _wall_block())
                if random.random() > 0.12:
                    sb.set_block(cx + cell_size - 1, 1 + dy, z, _wall_block())

        # 内部种植土
        for x in range(cx + 1, cx + cell_size - 1):
            for z in range(cz + 1, cz + cell_size - 1):
                sb.set_block(x, 0, z, "minecraft:podzol")
                # 种植灵草（60% 覆盖率，千年后部分枯萎）
                if random.random() < 0.6:
                    sb.set_block(x, 1, z, plant)

        # 入口（每格南面留 2 格门洞）
        mid = cx + cell_size // 2
        for dy in range(wall_h):
            sb.set_block(mid, 1 + dy, cz, "minecraft:air")
            sb.set_block(mid - 1, 1 + dy, cz, "minecraft:air")

    # 中央水池 6×6
    pool_x1, pool_z1 = 17, 17
    pool_x2, pool_z2 = 22, 22
    for x in range(pool_x1, pool_x2 + 1):
        for z in range(pool_z1, pool_z2 + 1):
            if x == pool_x1 or x == pool_x2 or z == pool_z1 or z == pool_z2:
                sb.set_block(x, 0, z, "minecraft:mossy_stone_bricks")
                if random.random() > 0.3:
                    sb.set_block(x, 1, z, "minecraft:mossy_stone_brick_wall")
            else:
                sb.set_block(x, 0, z, "minecraft:soul_sand")
                sb.set_block(x, 1, z, "minecraft:water")

    # 四角石灯笼（残骸）
    lantern_spots = [(2, 2), (37, 2), (2, 37), (37, 37)]
    for (lx, lz) in lantern_spots:
        sb.set_block(lx, 1, lz, "minecraft:cobblestone_wall")
        if random.random() > 0.3:
            sb.set_block(lx, 2, lz, "minecraft:cobblestone_wall")
        if random.random() > 0.5:
            sb.set_block(lx, 3, lz, "minecraft:soul_lantern")

    # ── 装饰精修（Round 2） ──

    # 步道边缘排水沟（半砖凹槽）
    for (pz1, pz2) in [(13, 14), (25, 26)]:
        for x in range(40):
            if random.random() < 0.7:
                sb.set_block(x, 0, pz1 - 1, "minecraft:stone_brick_slab", {"type": "bottom"})
                sb.set_block(x, 0, pz2 + 1, "minecraft:stone_brick_slab", {"type": "bottom"})

    # 种植格入口石板（小台阶）
    for (cx, cz, _) in plant_cells:
        mid = cx + cell_size // 2
        sb.set_block(mid, 0, cz, "minecraft:stone_brick_stairs", {"facing": "north", "half": "bottom"})
        sb.set_block(mid - 1, 0, cz, "minecraft:stone_brick_stairs", {"facing": "north", "half": "bottom"})

    # 水池边种小花（残存观赏植物）
    for dx in [-2, -1, 0, 1, 2]:
        for dz in [-2, 2]:
            x, z = 19 + dx, 19 + dz
            if random.random() < 0.4:
                sb.set_block(x, 1, z, random.choice([
                    "minecraft:dandelion", "minecraft:azure_bluet",
                    "minecraft:oxeye_daisy"
                ]))

    # 散落的堆肥桶/花盆（废弃的园艺工具）
    tool_spots = [(5, 15), (25, 15), (15, 5), (15, 35), (35, 25)]
    for (tx, tz) in tool_spots:
        if random.random() < 0.5:
            sb.set_block(tx, 1, tz, "minecraft:composter")
        else:
            sb.set_block(tx, 1, tz, "minecraft:flower_pot")

    # 石篱墙顶偶尔长苔藓
    for (cx, cz, _) in plant_cells:
        for x in range(cx, cx + cell_size):
            for z_wall in [cz, cz + cell_size - 1]:
                if random.random() < 0.12:
                    sb.set_block(x, 3, z_wall, "minecraft:moss_carpet")

    return sb


def build_outer() -> StructureBuilder:
    """外环药圃 48×4×48——更大版本，格局相同但更荒废。"""
    W, H, D = 48, 4, 48
    sb = StructureBuilder(W, H, D)

    # 类似内环但尺寸更大，荒废程度更高
    cell_size = 14
    wall_h = 2
    path_zs = [(15, 16), (31, 32)]
    path_xs = [(15, 16), (31, 32)]

    for x in range(W):
        for z in range(D):
            sb.set_block(x, 0, z, "minecraft:coarse_dirt")

    # 步道
    for x in range(W):
        for z in range(D):
            is_path = False
            for (pz1, pz2) in path_zs:
                if pz1 <= z <= pz2: is_path = True
            for (px1, px2) in path_xs:
                if px1 <= x <= px2: is_path = True
            if is_path:
                if random.random() < 0.7:  # 更破损
                    sb.set_block(x, 0, z, "minecraft:stone_bricks")

    # 种植区
    cells = [
        (1, 1, "minecraft:crimson_roots"),
        (33, 1, "minecraft:warped_roots"),
        (1, 33, "minecraft:twisting_vines"),
        (33, 33, "minecraft:crimson_fungus"),
    ]
    for (cx, cz, plant) in cells:
        for dy in range(wall_h):
            for x in range(cx, cx + cell_size):
                if random.random() > 0.2:  # 更多缺口
                    sb.set_block(x, 1 + dy, cz, _wall_block())
                if random.random() > 0.2:
                    sb.set_block(x, 1 + dy, cz + cell_size - 1, _wall_block())
            for z in range(cz, cz + cell_size):
                if random.random() > 0.2:
                    sb.set_block(cx, 1 + dy, z, _wall_block())
                if random.random() > 0.2:
                    sb.set_block(cx + cell_size - 1, 1 + dy, z, _wall_block())

        for x in range(cx + 1, cx + cell_size - 1):
            for z in range(cz + 1, cz + cell_size - 1):
                sb.set_block(x, 0, z, "minecraft:podzol")
                if random.random() < 0.45:  # 更稀疏
                    sb.set_block(x, 1, z, plant)

    # 中央水池
    pool_x1, pool_z1 = 20, 20
    pool_x2, pool_z2 = 27, 27
    for x in range(pool_x1, pool_x2 + 1):
        for z in range(pool_z1, pool_z2 + 1):
            if x == pool_x1 or x == pool_x2 or z == pool_z1 or z == pool_z2:
                sb.set_block(x, 0, z, "minecraft:mossy_stone_bricks")
            else:
                sb.set_block(x, 0, z, "minecraft:soul_sand")
                sb.set_block(x, 1, z, "minecraft:water")

    return sb


if __name__ == "__main__":
    for name, builder in [("herb_garden_pen_inner", build_inner), ("herb_garden_pen_outer", build_outer)]:
        sb = builder()
        out = f"server/structures/dan_zong/{name}.nbt"
        os.makedirs(os.path.dirname(out), exist_ok=True)
        sb.save(out)
        stats = sb.get_stats()
        print(f"{name}: {stats['size'][0]}×{stats['size'][1]}×{stats['size'][2]}, {stats['total_blocks']} blocks")

    # preview both
    sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
    from view_structures_3d import BLOCK_COLORS, DEFAULT_COLOR, generate_html

    data = {}
    for name, builder in [("herb_garden_inner_40", build_inner), ("herb_garden_outer_48", build_outer)]:
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
    preview = os.path.join(os.path.dirname(__file__), "..", "nbt", "garden_preview.html")
    with open(preview, "w") as f:
        f.write(html)
    print(f"Preview: {preview}")
