#!/usr/bin/env python3
"""百草丹殿 — 200×50×120 三进院落宗门大殿（废墟版）

独立设计每个建筑部分，逐步搭建。

三进院落总布局（z 正方向 = 由南向北深入）：

    z=0-12     山门（牌楼 + 石狮/鼎）
    z=12-50    第一进·前殿（正殿/大丹炉厅 80×25）
    z=50-55    连廊过渡 + 甬道
    z=55-85    第二进·中庭（露天院 + 东西厢房）
    z=85-90    连廊过渡
    z=90-120   第三进·后殿（藏经阁/祭坛 60×25）

    x: 0 ──── 100 (中轴) ──── 200

中国古建筑要素：
    - 台基（须弥座 / 月台）：三层递升
    - 柱网：开间 × 进深，柱高比 1:6-8
    - 歇山顶：正脊 + 垂脊 + 戗脊 + 飞檐
    - 斗拱：柱头 → 斗 → 拱 → 檐（用阶梯/半砖近似）
    - 格扇门窗：用 fence + trapdoor 近似
"""

from __future__ import annotations
import os, sys, random, math

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from nbt_builder import StructureBuilder

W, H, D = 200, 50, 120
CX = W // 2  # 100

random.seed(42_005)

def _s(ruin=0.35):
    """Stone bricks with ruin variants."""
    r = random.random()
    if r < ruin * 0.3: return "minecraft:cracked_stone_bricks"
    if r < ruin: return "minecraft:mossy_stone_bricks"
    return "minecraft:stone_bricks"

def _b():
    """Blackstone variants."""
    if random.random() < 0.12: return "minecraft:cracked_polished_blackstone_bricks"
    return "minecraft:polished_blackstone_bricks"

def _roof():
    """Roof block."""
    if random.random() < 0.15: return "minecraft:purple_terracotta"
    return "minecraft:dark_oak_planks"

def _skip(r=0.1):
    return random.random() < r


def build() -> StructureBuilder:
    sb = StructureBuilder(W, H, D)

    # ═══════════════════════════════════════
    # PART 1: 三层台基
    # ═══════════════════════════════════════
    _build_platforms(sb)

    # ═══════════════════════════════════════
    # PART 2: 山门
    # ═══════════════════════════════════════
    _build_gate(sb)

    # ═══════════════════════════════════════
    # PART 3: 前殿（正殿）
    # ═══════════════════════════════════════
    _build_front_hall(sb)

    # ═══════════════════════════════════════
    # PART 4: 中庭 + 东西厢房
    # ═══════════════════════════════════════
    _build_courtyard(sb)

    # ═══════════════════════════════════════
    # PART 5: 后殿（藏经阁）
    # ═══════════════════════════════════════
    _build_rear_hall(sb)

    # ═══════════════════════════════════════
    # PART 6: 连廊
    # ═══════════════════════════════════════
    _build_corridors(sb)

    # ═══════════════════════════════════════
    # PART 7: 中轴铺装 + 散落物
    # ═══════════════════════════════════════
    _build_axis_and_debris(sb)

    # ═══════════════════════════════════════
    # PART 8: 装饰精修（Round 2 打磨）
    # ═══════════════════════════════════════
    _decoration_pass(sb)

    return sb


# ── PART 1: 台基 ──

def _build_platforms(sb):
    """三层须弥座台基。"""

    # 第一层：总基座（整个 200×120 范围，高 3 格）
    for x in range(5, 195):
        for z in range(0, 120):
            for y in range(3):
                if not _skip(0.01):
                    sb.set_block(x, y, z, _s(0.2))

    # 台基边沿台阶（四面一圈）
    for x in range(5, 195):
        sb.set_block(x, 0, 0, "minecraft:stone_brick_stairs", {"facing": "south", "half": "bottom"})
        sb.set_block(x, 1, 0, "minecraft:stone_brick_stairs", {"facing": "south", "half": "bottom"})
    for z in range(0, 120):
        sb.set_block(5, 0, z, "minecraft:stone_brick_stairs", {"facing": "east", "half": "bottom"})
        sb.set_block(194, 0, z, "minecraft:stone_brick_stairs", {"facing": "west", "half": "bottom"})

    # 第二层：殿宇基座（前殿、后殿下方，高 5 格）
    hall_regions = [
        (55, 12, 145, 50),   # 前殿
        (65, 90, 135, 118),  # 后殿
    ]
    for (x1, z1, x2, z2) in hall_regions:
        for x in range(x1 - 3, x2 + 3):
            for z in range(z1 - 2, z2 + 2):
                for y in range(3, 5):
                    if 0 <= x < W and 0 <= z < D:
                        sb.set_block(x, y, z, _b())

    # 第三层：殿内地面（高 6 格）
    for (x1, z1, x2, z2) in hall_regions:
        for x in range(x1, x2):
            for z in range(z1, z2):
                sb.set_block(x, 5, z, _b())

    # 月台台阶（前殿正面，宽大台阶）
    for step in range(3):
        y = 3 + step
        z = 12 - step
        for x in range(CX - 15, CX + 15):
            sb.set_block(x, y, z, "minecraft:polished_blackstone_stairs",
                        {"facing": "north", "half": "bottom"})


# ── PART 2: 山门 ──

def _build_gate(sb):
    """山门牌楼：3 开间，中间大两侧小。"""
    by = 3  # 台基上
    gz = 5  # 山门 z 位置

    # 6 根门柱（3 开间 = 4 柱，加两端 = 6 柱）
    pillar_xs = [CX - 20, CX - 10, CX - 4, CX + 4, CX + 10, CX + 20]
    for px in pillar_xs:
        for y in range(by, by + 12):
            for dx in range(2):
                if not _skip(0.02):
                    sb.set_block(px + dx, y, gz, "minecraft:polished_blackstone")
                    sb.set_block(px + dx, y, gz + 1, "minecraft:polished_blackstone")

    # 横梁（额枋）
    for x in range(CX - 21, CX + 22):
        sb.set_block(x, by + 10, gz, _s())
        sb.set_block(x, by + 11, gz, "minecraft:purple_terracotta")

    # 匾额"百草門"
    for dx in range(-3, 4):
        sb.set_block(CX + dx, by + 9, gz, "minecraft:chiseled_stone_bricks")

    # 门楼屋顶（小歇山顶）
    _build_mini_roof(sb, CX - 22, gz - 3, CX + 22, gz + 4, by + 12, 6, collapse=0.25)

    # 石鼎（山门前两侧）
    for sign in [-1, 1]:
        dx = sign * 25
        bx = CX + dx
        # 鼎座 3×3
        for ddx in range(-1, 2):
            for ddz in range(-1, 2):
                sb.set_block(bx + ddx, by, gz - 3 + ddz, "minecraft:polished_blackstone")
        sb.set_block(bx, by + 1, gz - 3, "minecraft:cauldron")
        if random.random() > 0.4:
            sb.set_block(bx, by + 2, gz - 3, "minecraft:soul_campfire", {"lit": "true"})


# ── PART 3: 前殿 ──

def _build_front_hall(sb):
    """前殿（正殿）80×38，5 开间，6 对大柱，中央丹炉台。"""
    x1, z1, x2, z2 = 60, 14, 140, 48
    by = 6  # 殿内地面
    hall_h = 18

    # 柱网：5 开间 = 6 列柱，3 进深 = 4 排
    col_xs = [x1 + 5, x1 + 20, x1 + 35, x2 - 35, x2 - 20, x2 - 5]
    col_zs = [z1 + 4, z1 + 14, z2 - 14, z2 - 4]

    for px in col_xs:
        for pz in col_zs:
            # 2×2 大柱
            for dx in range(2):
                for dz in range(2):
                    for y in range(by, by + hall_h - 2):
                        if not _skip(0.01):
                            sb.set_block(px + dx, y, pz + dz, "minecraft:polished_blackstone")
            # 柱础（底部扩大一圈）
            for dx in range(-1, 3):
                for dz in range(-1, 3):
                    sb.set_block(px + dx, by, pz + dz, "minecraft:polished_blackstone_slab", {"type": "bottom"})

    # 墙体（檐墙 / 山墙）
    for y in range(by + 1, by + hall_h):
        sr = 0.03 + 0.10 * max(0, (y - by - 12) / hall_h)

        # 南墙（正面），留 10 格宽正门
        for x in range(x1, x2):
            if CX - 5 <= x <= CX + 5 and y < by + 8:
                continue  # 正门
            if not _skip(sr):
                sb.set_block(x, y, z1, _s())

        # 北墙，留 6 格后门
        for x in range(x1, x2):
            if CX - 3 <= x <= CX + 3 and y < by + 6:
                continue
            if not _skip(sr):
                sb.set_block(x, y, z2, _s())

        # 东西山墙
        for z in range(z1 + 1, z2):
            if not _skip(sr):
                sb.set_block(x1, y, z, _s())
            if not _skip(sr):
                sb.set_block(x2 - 1, y, z, _s())

    # 格扇窗（东西墙各 4 个, 3×4 格）
    for wall_x in [x1, x2 - 1]:
        for wz_start in [z1 + 6, z1 + 14, z1 + 22, z2 - 10]:
            for wz in range(wz_start, wz_start + 3):
                for wy in range(by + 3, by + 7):
                    sb.set_block(wall_x, wy, wz, "minecraft:purple_stained_glass_pane")

    # 正门装饰
    # 门柱
    for y in range(by, by + 8):
        sb.set_block(CX - 6, y, z1, "minecraft:polished_blackstone")
        sb.set_block(CX + 6, y, z1, "minecraft:polished_blackstone")
    # 匾额
    for dx in range(-4, 5):
        sb.set_block(CX + dx, by + 8, z1, "minecraft:chiseled_stone_bricks")
    for dx in range(-5, 6):
        sb.set_block(CX + dx, by + 9, z1, "minecraft:purple_terracotta")

    # 中央丹炉台 (9×9 平台 + 中心大鼎)
    altar_z = (z1 + z2) // 2
    for dx in range(-4, 5):
        for dz in range(-4, 5):
            sb.set_block(CX + dx, by, altar_z + dz, "minecraft:polished_blackstone_slab", {"type": "bottom"})
    # 中心鼎
    sb.set_block(CX, by + 1, altar_z, "minecraft:cauldron")
    sb.set_block(CX, by + 2, altar_z, "minecraft:cauldron")
    # 四角灯
    for (dx, dz) in [(-4, -4), (4, -4), (-4, 4), (4, 4)]:
        sb.set_block(CX + dx, by + 1, altar_z + dz, "minecraft:soul_lantern")
    # 香炉
    sb.set_block(CX - 2, by + 1, altar_z - 3, "minecraft:flower_pot")
    sb.set_block(CX + 2, by + 1, altar_z - 3, "minecraft:flower_pot")

    # 前殿屋顶
    _build_hip_gable_roof(sb, x1 - 4, z1 - 4, x2 + 4, z2 + 4, by + hall_h, 10, collapse=0.35)


# ── PART 4: 中庭 ──

def _build_courtyard(sb):
    """中庭：露天庭院 + 东西厢房。"""
    by = 3  # 台基上

    # 庭院地面
    court_x1, court_z1, court_x2, court_z2 = 30, 55, 170, 85
    for x in range(court_x1, court_x2):
        for z in range(court_z1, court_z2):
            r = random.random()
            if r < 0.06:
                sb.set_block(x, by, z, "minecraft:podzol")
            elif r < 0.12:
                sb.set_block(x, by, z, "minecraft:mossy_cobblestone")
            else:
                sb.set_block(x, by, z, _s(0.15))

    # 中央古树残桩
    for dx in range(-2, 3):
        for dz in range(-2, 3):
            dist = abs(dx) + abs(dz)
            if dist <= 3:
                h = max(1, 5 - dist)
                for dy in range(h):
                    sb.set_block(CX + dx, by + 1 + dy, 70 + dz, "minecraft:oak_log", {"axis": "y"})
    # 根系
    for angle in range(0, 360, 30):
        rx = CX + int(4 * math.cos(math.radians(angle)))
        rz = 70 + int(4 * math.sin(math.radians(angle)))
        if court_x1 < rx < court_x2 and court_z1 < rz < court_z2:
            sb.set_block(rx, by + 1, rz, "minecraft:oak_log", {"axis": "y"})

    # 石凳（6 对沿中轴）
    for dz in [-10, -6, -2, 2, 6, 10]:
        for sign in [-1, 1]:
            bx = CX + sign * 15
            bz = 70 + dz
            sb.set_block(bx, by + 1, bz, "minecraft:stone_brick_slab", {"type": "bottom"})
            sb.set_block(bx + sign, by + 1, bz, "minecraft:stone_brick_slab", {"type": "bottom"})

    # 东厢房 (25×20)
    _build_wing_house(sb, 150, 58, 175, 82, by, 10, "east")
    # 西厢房 (25×20)
    _build_wing_house(sb, 25, 58, 50, 82, by, 10, "west")

    # 庭院散落物
    for _ in range(40):
        x = random.randint(court_x1 + 5, court_x2 - 5)
        z = random.randint(court_z1 + 3, court_z2 - 3)
        if abs(x - CX) > 4 or abs(z - 70) > 4:
            if random.random() < 0.5:
                sb.set_block(x, by + 1, z, "minecraft:dead_bush")
            else:
                sb.set_block(x, by + 1, z, "minecraft:cobweb")


def _build_wing_house(sb, x1, z1, x2, z2, by, h, side):
    """厢房：独立小殿，带自己的屋顶。"""
    # 地面
    for x in range(x1, x2):
        for z in range(z1, z2):
            sb.set_block(x, by, z, _b())

    # 柱子（4 角 + 中间 2 根）
    pillar_spots = [
        (x1 + 2, z1 + 2), (x2 - 3, z1 + 2),
        (x1 + 2, z2 - 3), (x2 - 3, z2 - 3),
        ((x1 + x2) // 2, z1 + 2), ((x1 + x2) // 2, z2 - 3),
    ]
    for (px, pz) in pillar_spots:
        for y in range(by + 1, by + h - 1):
            if not _skip(0.02):
                sb.set_block(px, y, pz, "minecraft:polished_blackstone")

    # 墙
    for y in range(by + 1, by + h):
        sr = 0.05 + 0.12 * max(0, (y - by - 6) / h)
        for x in range(x1, x2):
            if not _skip(sr): sb.set_block(x, y, z1, _s(0.4))
            if not _skip(sr): sb.set_block(x, y, z2 - 1, _s(0.4))
        for z in range(z1, z2):
            # 朝庭院侧留门
            inner_x = x1 if side == "west" else x2 - 1
            outer_x = x2 - 1 if side == "west" else x1
            if not _skip(sr): sb.set_block(outer_x, y, z, _s(0.4))
            # 朝庭院侧门洞
            mid_z = (z1 + z2) // 2
            if abs(z - mid_z) <= 2 and y < by + 5:
                continue  # 门洞
            if not _skip(sr): sb.set_block(inner_x, y, z, _s(0.4))

    # 屋顶
    _build_mini_roof(sb, x1 - 2, z1 - 2, x2 + 2, z2 + 2, by + h, 5, collapse=0.3)


# ── PART 5: 后殿 ──

def _build_rear_hall(sb):
    """后殿（藏经阁）：60×28，3 开间。"""
    x1, z1, x2, z2 = 70, 92, 130, 118
    by = 6
    h = 15

    # 柱网 (3 开间 = 4 列)
    col_xs = [x1 + 5, x1 + 20, x2 - 20, x2 - 5]
    col_zs = [z1 + 4, z2 - 4]
    for px in col_xs:
        for pz in col_zs:
            for dx in range(2):
                for dz in range(2):
                    for y in range(by, by + h - 2):
                        if not _skip(0.015):
                            sb.set_block(px + dx, y, pz + dz, "minecraft:polished_blackstone")

    # 墙
    for y in range(by + 1, by + h):
        sr = 0.04 + 0.12 * max(0, (y - by - 8) / h)
        for x in range(x1, x2):
            if CX - 3 <= x <= CX + 3 and y < by + 6:
                continue
            if not _skip(sr): sb.set_block(x, y, z1, _s())
            if not _skip(sr): sb.set_block(x, y, z2 - 1, _s())
        for z in range(z1, z2):
            if not _skip(sr): sb.set_block(x1, y, z, _s())
            if not _skip(sr): sb.set_block(x2 - 1, y, z, _s())

    # 后殿祭坛
    altar_z = z2 - 6
    for dx in range(-5, 6):
        for dz in range(-2, 3):
            sb.set_block(CX + dx, by, altar_z + dz, "minecraft:polished_blackstone_slab", {"type": "bottom"})
    sb.set_block(CX, by + 1, altar_z, "minecraft:chiseled_polished_blackstone")
    sb.set_block(CX - 3, by + 1, altar_z, "minecraft:soul_lantern")
    sb.set_block(CX + 3, by + 1, altar_z, "minecraft:soul_lantern")

    # 书架（藏经阁）
    for bz in range(z1 + 3, z2 - 5, 4):
        for bx in [x1 + 2, x1 + 3, x2 - 3, x2 - 4]:
            for y in range(by + 1, by + 4):
                if not _skip(0.2):
                    sb.set_block(bx, y, bz, "minecraft:bookshelf")

    # 屋顶
    _build_hip_gable_roof(sb, x1 - 3, z1 - 3, x2 + 3, z2 + 3, by + h, 8, collapse=0.4)


# ── PART 6: 连廊 ──

def _build_corridors(sb):
    """四段连廊连接前殿→中庭→后殿。"""
    by = 3
    ch = 7

    # 前殿 → 中庭（两侧）
    pairs = [
        (55, 50, 60, 55),   # 西侧
        (140, 50, 145, 55),  # 东侧
        (65, 85, 70, 90),   # 西侧（中庭→后殿）
        (130, 85, 135, 90),  # 东侧
    ]
    for (x1, z1, x2, z2) in pairs:
        # 柱列
        for z in range(z1, z2 + 1, 3):
            for x in [x1, x2 - 1]:
                for y in range(by + 1, by + ch):
                    if not _skip(0.05):
                        sb.set_block(x, y, z, "minecraft:polished_blackstone")
        # 顶
        for x in range(x1, x2):
            for z in range(z1, z2):
                if not _skip(0.25):
                    sb.set_block(x, by + ch, z, "minecraft:dark_oak_slab", {"type": "top"})
        # 地面
        for x in range(x1, x2):
            for z in range(z1, z2):
                sb.set_block(x, by, z, _s(0.15))


# ── PART 7: 中轴 + 散落物 ──

def _build_axis_and_debris(sb):
    """中轴石板路 + 室内散落碎石/蛛网。"""
    by = 3

    # 中轴路 (8 格宽)
    for z in range(0, 120):
        for dx in range(-4, 5):
            x = CX + dx
            if 0 <= x < W:
                if abs(dx) <= 1:
                    sb.set_block(x, by, z, "minecraft:polished_blackstone")
                else:
                    sb.set_block(x, by, z, "minecraft:polished_blackstone_slab", {"type": "bottom"})

    # 前殿内散落
    for _ in range(80):
        x = random.randint(62, 138)
        z = random.randint(16, 46)
        r = random.random()
        if r < 0.25: sb.set_block(x, 7, z, "minecraft:cobweb")
        elif r < 0.4: sb.set_block(x, 7, z, "minecraft:dead_bush")
        elif r < 0.6: sb.set_block(x, 7, z, "minecraft:gravel")

    # 后殿内散落
    for _ in range(40):
        x = random.randint(72, 128)
        z = random.randint(94, 116)
        if random.random() < 0.5:
            sb.set_block(x, 7, z, "minecraft:cobweb")
        else:
            sb.set_block(x, 7, z, "minecraft:gravel")

    # 高处蛛网（殿角）
    corners_front = [(62, 16), (138, 16), (62, 46), (138, 46)]
    for (cx, cz) in corners_front:
        for dy in range(14, 18):
            for ddx in range(-1, 2):
                for ddz in range(-1, 2):
                    if not _skip(0.4):
                        sb.set_block(cx + ddx, 6 + dy, cz + ddz, "minecraft:cobweb")


# ── PART 8: 装饰精修 ──

def _decoration_pass(sb):
    """精细装饰：藤蔓/灯笼/家具/墙饰/地面纹路/植被入侵。"""

    # ── 前殿内部装饰 ──
    by = 6
    fx1, fz1, fx2, fz2 = 60, 14, 140, 48

    # 沿墙陈列架（东西墙内侧，暗色橡木+半砖做展台）
    for wall_x, dx in [(fx1 + 2, 1), (fx2 - 3, -1)]:
        for sz in range(fz1 + 4, fz2 - 4, 5):
            # 展台底座
            sb.set_block(wall_x, by + 1, sz, "minecraft:dark_oak_planks")
            sb.set_block(wall_x, by + 1, sz + 1, "minecraft:dark_oak_planks")
            sb.set_block(wall_x, by + 2, sz, "minecraft:dark_oak_slab", {"type": "bottom"})
            # 展品（花盆=丹瓶 / 书架=丹方 / 空=被掠夺）
            r = random.random()
            if r < 0.3:
                sb.set_block(wall_x, by + 2, sz + 1, "minecraft:flower_pot")
            elif r < 0.5:
                sb.set_block(wall_x, by + 2, sz + 1, "minecraft:bookshelf")
            # 展台上方壁灯
            if not _skip(0.4):
                sb.set_block(wall_x + dx, by + 4, sz, "minecraft:soul_lantern")

    # 前殿柱间挂链+灯（从天花板垂下）
    col_xs = [fx1 + 5, fx1 + 20, fx1 + 35, fx2 - 35, fx2 - 20, fx2 - 5]
    col_zs = [fz1 + 4, fz1 + 14, fz2 - 14, fz2 - 4]
    for i, px in enumerate(col_xs[:-1]):
        px_next = col_xs[i + 1]
        mid_x = (px + px_next) // 2
        for pz in col_zs:
            if not _skip(0.35):
                # 链条 + 灯笼
                for chain_y in range(by + 14, by + 17):
                    sb.set_block(mid_x, chain_y, pz, "minecraft:chain")
                sb.set_block(mid_x, by + 13, pz, "minecraft:soul_lantern")

    # 前殿地面纹路——中央丹炉台周围铺设同心方环
    altar_z = (fz1 + fz2) // 2
    for ring_r in [6, 8, 10]:
        for angle in range(0, 360, 5):
            rx = CX + int(ring_r * math.cos(math.radians(angle)))
            rz = altar_z + int(ring_r * math.sin(math.radians(angle)))
            if fx1 + 3 < rx < fx2 - 3 and fz1 + 3 < rz < fz2 - 3:
                sb.set_block(rx, by, rz, "minecraft:chiseled_polished_blackstone")

    # ── 后殿装饰 ──
    rx1, rz1, rx2, rz2 = 70, 92, 130, 118

    # 书架更密+散落书本
    for bz in range(rz1 + 2, rz2 - 4, 3):
        for bx in [rx1 + 2, rx1 + 3, rx2 - 3, rx2 - 4]:
            for y in range(by + 1, by + 5):
                if not _skip(0.15):
                    sb.set_block(bx, y, bz, "minecraft:bookshelf")
    # 散落书本（地面）
    for _ in range(15):
        x = random.randint(rx1 + 4, rx2 - 5)
        z = random.randint(rz1 + 3, rz2 - 4)
        sb.set_block(x, by + 1, z, "minecraft:lectern")

    # 后殿墙壁丹方壁画（用 glazed_terracotta 做纹饰墙面）
    mural_zs = [rz1 + 5, rz1 + 10, rz1 + 15]
    for mz in mural_zs:
        for y in range(by + 2, by + 5):
            # 东墙
            sb.set_block(rx2 - 2, y, mz, "minecraft:purple_glazed_terracotta")
            sb.set_block(rx2 - 2, y, mz + 1, "minecraft:purple_glazed_terracotta")
            # 西墙
            sb.set_block(rx1 + 1, y, mz, "minecraft:purple_glazed_terracotta")
            sb.set_block(rx1 + 1, y, mz + 1, "minecraft:purple_glazed_terracotta")

    # ── 中庭装饰 ──

    # 庭院石板路图案（十字甬道 + 圆形中心）
    for angle in range(0, 360, 3):
        for r in [8, 12, 16]:
            x = CX + int(r * math.cos(math.radians(angle)))
            z = 70 + int(r * math.sin(math.radians(angle)))
            if 30 < x < 170 and 55 < z < 85:
                sb.set_block(x, 3, z, "minecraft:polished_blackstone_slab", {"type": "bottom"})

    # 庭院灯柱（4 根，十字分布）
    for (lx, lz) in [(CX, 58), (CX, 82), (45, 70), (155, 70)]:
        for y in range(4, 8):
            sb.set_block(lx, y, lz, "minecraft:cobblestone_wall")
        if not _skip(0.3):
            sb.set_block(lx, 8, lz, "minecraft:soul_lantern")

    # ── 藤蔓入侵（外墙+屋顶边缘） ──
    vine_walls = [
        (fx1, fz1, fz2, "east"),   # 前殿西墙外
        (fx2 - 1, fz1, fz2, "west"),  # 前殿东墙外
        (rx1, rz1, rz2, "east"),
        (rx2 - 1, rz1, rz2, "west"),
    ]
    for (wx, wz1, wz2, facing) in vine_walls:
        for z in range(wz1, wz2, 2):
            if not _skip(0.6):
                vine_h = random.randint(2, 6)
                start_y = by + random.randint(8, 14)
                for dy in range(vine_h):
                    y = start_y - dy
                    if y > by:
                        sb.set_block(wx, y, z, "minecraft:vine",
                                    {facing: "true"})

    # ── 山门装饰补充 ──
    # 山门前石板铺地 + 散落旗帜残骸
    for dx in range(-30, 31):
        for dz in range(-2, 3):
            x = CX + dx
            z = 3 + dz
            if 5 < x < 195 and 0 < z < 10:
                if (dx + dz) % 4 == 0:
                    sb.set_block(x, 3, z, "minecraft:chiseled_stone_bricks")

    # 残破旗帜（山门两侧）
    for sign in [-1, 1]:
        bx = CX + sign * 15
        sb.set_block(bx, 4, 4, "minecraft:oak_fence")
        sb.set_block(bx, 5, 4, "minecraft:oak_fence")
        sb.set_block(bx, 6, 4, "minecraft:oak_fence")
        if not _skip(0.3):
            color = random.choice(["purple", "red", "white"])
            sb.set_block(bx, 7, 4, f"minecraft:{color}_banner", {"rotation": "8"})

    # ── 连廊灯笼 ──
    corridor_pairs = [(57, 52), (142, 52), (67, 87), (132, 87)]
    for (cx, cz) in corridor_pairs:
        for dz in range(0, 5, 2):
            if not _skip(0.4):
                sb.set_block(cx, 9, cz + dz, "minecraft:chain")
                sb.set_block(cx, 8, cz + dz, "minecraft:soul_lantern")


# ── 屋顶辅助函数 ──

def _build_hip_gable_roof(sb, x1, z1, x2, z2, base_y, max_rise, collapse=0.3):
    """歇山顶：四坡收顶 + 正脊。"""
    w = x2 - x1
    d = z2 - z1
    cx = (x1 + x2) // 2
    cz = (z1 + z2) // 2

    for x in range(x1, x2):
        for z in range(z1, z2):
            # 到边缘的距离决定高度
            dx = min(x - x1, x2 - 1 - x)
            dz = min(z - z1, z2 - 1 - z)
            d_edge = min(dx, dz)

            rise = min(d_edge, max_rise)
            y = base_y + rise

            # 坍塌概率：距离边缘越远越容易塌
            c = collapse * (d_edge / max(max_rise, 1))
            if not _skip(c):
                sb.set_block(x, y, z, _roof())

            # 飞檐（边缘第一格向外挑出一格）
            if d_edge == 0:
                if not _skip(0.1):
                    sb.set_block(x, base_y - 1, z, "minecraft:dark_oak_stairs",
                                {"facing": "south", "half": "top"})

    # 正脊
    for x in range(x1 + max_rise, x2 - max_rise):
        if not _skip(collapse * 0.5):
            sb.set_block(x, base_y + max_rise, cz, "minecraft:polished_blackstone_slab", {"type": "top"})


def _build_mini_roof(sb, x1, z1, x2, z2, base_y, max_rise, collapse=0.2):
    """小型歇山顶（山门/厢房用）。"""
    _build_hip_gable_roof(sb, x1, z1, x2, z2, base_y, max_rise, collapse)


# ════════════════════════════════════════════
if __name__ == "__main__":
    print("Building 百草丹殿 v3 (200×50×120)...")
    sb = build()

    out = "server/structures/dan_zong/dan_zong_great_hall.nbt"
    os.makedirs(os.path.dirname(out), exist_ok=True)
    sb.save(out)

    stats = sb.get_stats()
    print(f"Size: {stats['size'][0]}×{stats['size'][1]}×{stats['size'][2]}")
    print(f"Blocks: {stats['total_blocks']}, Palette: {stats['palette_size']}")
    for bt, cnt in sorted(stats["block_counts"].items(), key=lambda x: -x[1])[:15]:
        print(f"  {bt}: {cnt}")

    # Preview
    sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
    from view_structures_3d import BLOCK_COLORS, DEFAULT_COLOR, generate_html

    blocks = []
    for b in sb._blocks:
        pos = b["pos"]
        pe = sb._palette[b["state"]]
        name = pe["Name"]
        c = BLOCK_COLORS.get(name, DEFAULT_COLOR)
        blocks.append({"x": pos[0], "y": pos[1], "z": pos[2], "c": c, "n": name})

    data = {"dan_zong_great_hall_v3": {"blocks": blocks, "size": [W, H, D]}}
    html = generate_html(data)
    preview_path = os.path.join(os.path.dirname(__file__), "..", "nbt", "great_hall_v3_preview.html")
    with open(preview_path, "w") as f:
        f.write(html)
    print(f"\nPreview: {preview_path}")
