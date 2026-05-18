#!/usr/bin/env python3
"""三个小型叙事结构体：

1. 倒塌丹方碑 (8×12×5) — 碑座龟趺 + 碑身倒地
2. 丹师枯骨 (8×3×12) — 骸骨散布 + 散落丹瓶
3. 宗主石棺 (14×6×20) — 棺室 + 棺台 + 壁龛 + 石门
"""

from __future__ import annotations
import os, sys, random

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from nbt_builder import StructureBuilder

random.seed(42_004)


# ═══════════════════════════════════════════════════
# 1. 倒塌丹方碑 — 8×12×5
# ═══════════════════════════════════════════════════
#
# 侧视（x-y 面）：
#
#  y=11  ·         碑额（原本在最顶，现在碎在地上）
#  y=10  ·
#   ...
#  y=4   ████████   碑身（倒地，横躺在地面上）
#  y=3   ████████   厚 2 格
#  y=2   ██  ██     碑座（龟趺底座，仍直立）
#  y=1   ██████
#  y=0   ████████   地基

def build_stele() -> StructureBuilder:
    W, H, D = 8, 12, 5
    sb = StructureBuilder(W, H, D)

    # 地基（破碎石砖地面）
    for x in range(W):
        for z in range(D):
            r = random.random()
            if r < 0.3:
                sb.set_block(x, 0, z, "minecraft:mossy_cobblestone")
            elif r < 0.5:
                sb.set_block(x, 0, z, "minecraft:gravel")
            else:
                sb.set_block(x, 0, z, "minecraft:cobblestone")

    # 碑座（龟趺）— 仍然直立，4×3×3
    # 龟趺 = polished_blackstone 基座 + 背上驮碑
    base_x, base_z = 1, 1
    # 底座
    for x in range(base_x, base_x + 4):
        for z in range(base_z, base_z + 3):
            sb.set_block(x, 1, z, "minecraft:polished_blackstone")
    # 龟背隆起
    for x in range(base_x + 1, base_x + 3):
        sb.set_block(x, 2, base_z + 1, "minecraft:polished_blackstone_slab", {"type": "bottom"})
    # 龟头
    sb.set_block(base_x, 1, base_z + 1, "minecraft:polished_blackstone_wall")

    # 碑身 — 倒地横躺（z 方向倒下，从碑座 z=3 开始往 z+ 方向倒）
    # 碑身 6×8 高，倒下后变成 z 方向 8 格长，y=1-2 高
    stele_z_start = base_z + 3
    for dz in range(8):
        z = stele_z_start + dz
        if z >= D:
            break
        # 碑身宽 6 格 (x=1..6)
        for x in range(1, 7):
            if random.random() > 0.05:
                sb.set_block(x, 1, z, "minecraft:chiseled_polished_blackstone")
            if dz < 6 and random.random() > 0.1:
                sb.set_block(x, 2, z, "minecraft:polished_blackstone")

    # 碑身上的"丹方"纹饰（中间几格用 chiseled_stone_bricks 表示文字）
    for dz in range(2, 6):
        z = stele_z_start + dz
        if z < D:
            for x in range(2, 6):
                sb.set_block(x, 2, z, "minecraft:chiseled_stone_bricks")

    # 碑额碎片（散落在碑身旁）
    sb.set_block(0, 1, stele_z_start + 2, "minecraft:stone_brick_slab", {"type": "bottom"})
    sb.set_block(7, 1, stele_z_start + 4, "minecraft:cracked_stone_bricks")

    # 碑身裂痕
    crack_z = stele_z_start + 3
    if crack_z < D:
        sb.set_block(3, 2, crack_z, "minecraft:cracked_polished_blackstone_bricks")
        sb.set_block(4, 2, crack_z, "minecraft:cracked_polished_blackstone_bricks")

    return sb


# ═══════════════════════════════════════════════════
# 2. 丹师枯骨 — 8×3×12
# ═══════════════════════════════════════════════════
#
# 俯视（x-z 面）：
#
#  z=0   [丹瓶] [卷轴]
#  z=1            ·
#  z=2   [右臂伸出]──[躯干]──[左臂弯折]
#  z=3            ·  [骨盆]
#  z=4                 ·
#  z=5   [散落法器]  [右腿] [左腿]
#  z=6                ·       ·
#  z=7           [右胫] [左胫]
#  z=8                ·
#  z=9   [散落碎片]
#  z=10  [丹方残页]
#  z=11           ·

def build_bone() -> StructureBuilder:
    W, H, D = 8, 3, 12
    sb = StructureBuilder(W, H, D)

    # 地面（紫色丹毒侵染的土）
    for x in range(W):
        for z in range(D):
            r = random.random()
            if r < 0.3:
                sb.set_block(x, 0, z, "minecraft:podzol")
            elif r < 0.5:
                sb.set_block(x, 0, z, "minecraft:purple_terracotta")
            else:
                sb.set_block(x, 0, z, "minecraft:coarse_dirt")

    # 头骨 (z=1, 居中)
    sb.set_block(4, 1, 1, "minecraft:skeleton_skull", {"rotation": "8"})

    # 脊椎 / 躯干 (z=2-4)
    for z in [2, 3, 4]:
        sb.set_block(4, 1, z, "minecraft:bone_block", {"axis": "z"})

    # 右臂伸出 (向 x- 方向, z=2)
    sb.set_block(3, 1, 2, "minecraft:bone_block", {"axis": "x"})
    sb.set_block(2, 1, 2, "minecraft:bone_block", {"axis": "x"})
    # 右手抓着丹瓶
    sb.set_block(1, 1, 2, "minecraft:flower_pot")

    # 左臂弯折 (z=2-3)
    sb.set_block(5, 1, 2, "minecraft:bone_block", {"axis": "x"})
    sb.set_block(6, 1, 2, "minecraft:bone_block", {"axis": "x"})
    sb.set_block(6, 1, 3, "minecraft:bone_block", {"axis": "z"})

    # 骨盆
    sb.set_block(3, 1, 4, "minecraft:bone_block", {"axis": "x"})
    sb.set_block(5, 1, 4, "minecraft:bone_block", {"axis": "x"})

    # 腿骨 (z=5-8)
    # 右腿
    sb.set_block(3, 1, 5, "minecraft:bone_block", {"axis": "z"})
    sb.set_block(3, 1, 6, "minecraft:bone_block", {"axis": "z"})
    sb.set_block(3, 1, 7, "minecraft:bone_block", {"axis": "z"})
    # 左腿（略弯）
    sb.set_block(5, 1, 5, "minecraft:bone_block", {"axis": "z"})
    sb.set_block(5, 1, 6, "minecraft:bone_block", {"axis": "z"})
    sb.set_block(6, 1, 7, "minecraft:bone_block", {"axis": "x"})

    # 散落物品
    # 丹瓶 (花盆代替)
    sb.set_block(1, 1, 0, "minecraft:flower_pot")
    sb.set_block(6, 1, 0, "minecraft:flower_pot")

    # 卷轴 (用 banner 代替)
    sb.set_block(3, 1, 0, "minecraft:white_banner", {"rotation": "4"})

    # 残破法器
    sb.set_block(0, 1, 5, "minecraft:iron_nugget")  # 用矿渣代替
    sb.set_block(7, 1, 9, "minecraft:chain")

    # 丹方残页（地面装饰）
    sb.set_block(2, 1, 10, "minecraft:birch_pressure_plate")  # 纸张
    sb.set_block(5, 1, 11, "minecraft:birch_pressure_plate")

    # 身体周围紫色药渍（丹毒渗入土壤的痕迹）
    for dx in range(-1, 2):
        for dz in range(-1, 2):
            x, z = 4 + dx, 3 + dz
            if 0 <= x < W and 0 <= z < D:
                if random.random() < 0.5:
                    sb.set_block(x, 0, z, "minecraft:purple_terracotta")

    return sb


# ═══════════════════════════════════════════════════
# 3. 宗主石棺 — 14×6×20
# ═══════════════════════════════════════════════════
#
# 侧视（z-y 面）：
#
#  y=5        [壁龛天花]
#  y=4   [壁龛]  [壁龛]   壁龛内放祭品
#  y=3   [壁龛]  [壁龛]
#  y=2   ████ [棺盖] ████
#  y=1   ████ [棺体] ████   棺台石砖基座
#  y=0   ██████████████     地板
#
# 俯视（x-z 面，y=1 层）：
#
#  z=0   [石门]  (入口端)
#  z=1-3 [走道]
#  z=4   [壁龛左] [走道] [壁龛右]
#  z=5-6 [壁龛]          [壁龛]
#  z=7-8 ·
#  z=9-11  ████ [棺台] ████
#  z=12-14 ████ [棺台] ████
#  z=15-16 ·
#  z=17-18 [后壁龛]
#  z=19   [后墙]

def build_coffin() -> StructureBuilder:
    W, H, D = 14, 6, 20
    sb = StructureBuilder(W, H, D)
    cx = W // 2  # 7

    # 地板
    for x in range(W):
        for z in range(D):
            r = random.random()
            if r < 0.15:
                sb.set_block(x, 0, z, "minecraft:mossy_stone_bricks")
            elif r < 0.25:
                sb.set_block(x, 0, z, "minecraft:cracked_stone_bricks")
            else:
                sb.set_block(x, 0, z, "minecraft:polished_blackstone")

    # 墙壁（四面）
    for y in range(1, H):
        skip_r = 0.03 + 0.08 * max(0, (y - 3) / H)
        for x in range(W):
            if random.random() > skip_r:
                sb.set_block(x, y, 0, "minecraft:polished_blackstone_bricks")
            if random.random() > skip_r:
                sb.set_block(x, y, D - 1, "minecraft:polished_blackstone_bricks")
        for z in range(D):
            if random.random() > skip_r:
                sb.set_block(0, y, z, "minecraft:polished_blackstone_bricks")
            if random.random() > skip_r:
                sb.set_block(W - 1, y, z, "minecraft:polished_blackstone_bricks")

    # 石门（z=0 中央，3 格宽 4 格高）
    for x in range(cx - 1, cx + 2):
        for y in range(1, 5):
            sb.set_block(x, y, 0, "minecraft:air")
    # 门框
    sb.set_block(cx - 2, 1, 0, "minecraft:chiseled_polished_blackstone")
    sb.set_block(cx - 2, 2, 0, "minecraft:chiseled_polished_blackstone")
    sb.set_block(cx - 2, 3, 0, "minecraft:chiseled_polished_blackstone")
    sb.set_block(cx + 2, 1, 0, "minecraft:chiseled_polished_blackstone")
    sb.set_block(cx + 2, 2, 0, "minecraft:chiseled_polished_blackstone")
    sb.set_block(cx + 2, 3, 0, "minecraft:chiseled_polished_blackstone")
    # 门楣
    for x in range(cx - 2, cx + 3):
        sb.set_block(x, 4, 0, "minecraft:chiseled_polished_blackstone")

    # 棺台（z=9-14，居中，4×6 宽深，高 2 格）
    coffin_x1, coffin_x2 = cx - 2, cx + 3
    coffin_z1, coffin_z2 = 9, 14
    # 台基
    for x in range(coffin_x1 - 1, coffin_x2 + 1):
        for z in range(coffin_z1 - 1, coffin_z2 + 1):
            sb.set_block(x, 1, z, "minecraft:polished_blackstone_slab", {"type": "bottom"})
    # 棺体
    for x in range(coffin_x1, coffin_x2):
        for z in range(coffin_z1, coffin_z2):
            sb.set_block(x, 1, z, "minecraft:polished_blackstone")
            # 棺盖
            if x == coffin_x1 or x == coffin_x2 - 1 or z == coffin_z1 or z == coffin_z2 - 1:
                sb.set_block(x, 2, z, "minecraft:polished_blackstone_slab", {"type": "top"})
            else:
                sb.set_block(x, 2, z, "minecraft:polished_blackstone_slab", {"type": "top"})

    # 棺盖破裂（部分被掀开）
    sb.set_block(cx, 2, coffin_z1 + 1, "minecraft:air")
    sb.set_block(cx + 1, 2, coffin_z1 + 1, "minecraft:air")
    # 掀开的棺盖碎片落在旁边
    sb.set_block(coffin_x2 + 1, 1, coffin_z1 + 1, "minecraft:polished_blackstone_slab", {"type": "bottom"})

    # 棺内：师叔之印放置点（chiseled_polished_blackstone 标记）
    sb.set_block(cx, 1, (coffin_z1 + coffin_z2) // 2, "minecraft:chiseled_polished_blackstone")

    # 壁龛（两侧各 2 个，z=4-6 和 z=15-17）
    niche_configs = [
        (1, 4),    # 左前
        (W - 3, 4),  # 右前
        (1, 15),   # 左后
        (W - 3, 15), # 右后
    ]
    for (nx, nz) in niche_configs:
        # 壁龛凹进（2×3×2 的小空间）
        for x in range(nx, nx + 2):
            for z in range(nz, nz + 2):
                for y in range(1, 4):
                    sb.set_block(x, y, z, "minecraft:air")
        # 壁龛底
        for x in range(nx, nx + 2):
            for z in range(nz, nz + 2):
                sb.set_block(x, 1, z, "minecraft:polished_blackstone_slab", {"type": "bottom"})
        # 壁龛内祭品
        sb.set_block(nx, 2, nz, "minecraft:soul_lantern")
        if random.random() > 0.3:
            sb.set_block(nx + 1, 2, nz, "minecraft:flower_pot")

    # 天花板
    for x in range(1, W - 1):
        for z in range(1, D - 1):
            if random.random() > 0.15:
                sb.set_block(x, H - 1, z, "minecraft:polished_blackstone_slab", {"type": "top"})

    # 棺室内散落物
    debris_spots = [(3, 2), (10, 3), (2, 8), (11, 7), (4, 16), (9, 17)]
    for (dx, dz) in debris_spots:
        r = random.random()
        if r < 0.3:
            sb.set_block(dx, 1, dz, "minecraft:cobweb")
        elif r < 0.6:
            sb.set_block(dx, 1, dz, "minecraft:gravel")
        else:
            sb.set_block(dx, 1, dz, "minecraft:dead_bush")

    # 碑文石板（后壁，z=18）
    for x in range(3, W - 3):
        sb.set_block(x, 2, D - 2, "minecraft:chiseled_stone_bricks")
        sb.set_block(x, 3, D - 2, "minecraft:chiseled_polished_blackstone")

    return sb


# ════════════════════════════════════════════
if __name__ == "__main__":
    structures = [
        ("fallen_recipe_stele", build_stele),
        ("fallen_alchemist_bone", build_bone),
        ("master_sarcophagus", build_coffin),
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
    preview = os.path.join(os.path.dirname(__file__), "..", "nbt", "narrative_preview.html")
    with open(preview, "w") as f:
        f.write(html)
    print(f"Preview: {preview}")
