#!/usr/bin/env python3
"""末法残土 Phase B — Anvil 地形后处理

在 Phase A (Datapack + Chunky) 生成的 .mca 世界上做方块级装饰增强。
通过 mcworldlib 读写 1.20.1 Anvil 格式，不需要启动 MC 服务端。

装饰规则按 Y 高度分四层：
  high_peak  (Y>200): 骷髅头、蜘蛛网、灵魂灯笼
  mid_slope  (Y 80~200): 枯死珊瑚扇、黑石按钮碎石、蜘蛛网
  low_valley (Y 30~80): sculk 传感器、枯灌木、蘑菇
  abyss      (Y<30): sculk 块、sculk 脉络

特殊规则：
  岩浆块上方 30% 概率放发光地衣，5% 放菌光体

用法:
  cd worldgen
  source .venv/bin/activate
  python3 scripts/postprocess.py [world_path]

默认 world_path: server/mofa-world
"""

import math
import os
import random
import sys
from collections import defaultdict

import mcworldlib as mc
from nbtlib import Compound, List, Long, String


# ── 常量 ──────────────────────────────────────────────

WORLD_PATH = "server/mofa-world"
SEED = 42

# 装饰方块定义
def block(name: str, props: dict | None = None) -> Compound:
    """创建一个 block state compound"""
    c = Compound({"Name": String(f"minecraft:{name}")})
    if props:
        c["Properties"] = Compound({k: String(v) for k, v in props.items()})
    return c


# 装饰配置
DECORATIONS = {
    # 高山顶（Y>200）：骷髅、蜘蛛网
    "high_peak": {
        "y_min": 200,
        "y_max": 320,
        "surface_scatter": [
            (block("skeleton_skull", {"rotation": "0"}), 0.003),
            (block("skeleton_skull", {"rotation": "4"}), 0.002),
            (block("skeleton_skull", {"rotation": "8"}), 0.002),
            (block("cobweb"), 0.005),
            (block("soul_lantern", {"hanging": "false", "waterlogged": "false"}), 0.001),
        ],
    },
    # 中层（Y 80-200）：枯死珊瑚、按钮碎石
    "mid_slope": {
        "y_min": 80,
        "y_max": 200,
        "surface_scatter": [
            (block("dead_brain_coral_fan", {"waterlogged": "false"}), 0.004),
            (block("dead_tube_coral_fan", {"waterlogged": "false"}), 0.004),
            (block("dead_fire_coral_fan", {"waterlogged": "false"}), 0.003),
            (block("polished_blackstone_button", {"face": "floor", "facing": "north", "powered": "false"}), 0.003),
            (block("cobweb"), 0.002),
        ],
    },
    # 低谷（Y 30-80）：发光地衣、sculk
    "low_valley": {
        "y_min": 30,
        "y_max": 80,
        "surface_scatter": [
            (block("sculk_sensor", {"sculk_sensor_phase": "inactive", "power": "0", "waterlogged": "false"}), 0.002),
            (block("dead_bush"), 0.005),
            (block("brown_mushroom"), 0.003),
        ],
    },
    # 深渊（Y<30）：sculk、发光地衣
    "abyss": {
        "y_min": -64,
        "y_max": 30,
        "surface_scatter": [
            (block("sculk"), 0.008),
            (block("sculk_sensor", {"sculk_sensor_phase": "inactive", "power": "0", "waterlogged": "false"}), 0.003),
            (block("sculk_vein", {"down": "true", "east": "false", "north": "false", "south": "false", "up": "false", "waterlogged": "false", "west": "false"}), 0.005),
        ],
    },
}

# 岩浆块周围的发光装饰
MAGMA_NEIGHBORS_DECORATIONS = [
    (block("glow_lichen", {"down": "true", "east": "false", "north": "false", "south": "false", "up": "false", "waterlogged": "false", "west": "false"}), 0.3),
    (block("shroomlight"), 0.05),
]

# 固体方块集合（用于判断是否为地表）
SOLID_BLOCKS = {
    "minecraft:netherrack", "minecraft:blackstone", "minecraft:basalt",
    "minecraft:smooth_basalt", "minecraft:deepslate", "minecraft:magma_block",
    "minecraft:nether_wart_block", "minecraft:red_nether_bricks",
    "minecraft:crimson_planks", "minecraft:polished_blackstone",
    "minecraft:polished_blackstone_bricks", "minecraft:dripstone_block",
}

AIR_BLOCK = block("air")


# ── 方块读写工具 ─────────────────────────────────────

def decode_packed_indices(data_longs: list, bits_per_entry: int, total: int = 4096) -> list[int]:
    """解码 1.18+ packed block state 索引（条目不跨越 long 边界）"""
    indices = []
    mask = (1 << bits_per_entry) - 1
    entries_per_long = 64 // bits_per_entry

    for long_val in data_longs:
        # nbtlib 的 Long 可能是有符号的，转为无符号
        val = long_val & 0xFFFFFFFFFFFFFFFF
        for j in range(entries_per_long):
            if len(indices) >= total:
                break
            indices.append(val & mask)
            val >>= bits_per_entry

    return indices[:total]


def encode_packed_indices(indices: list[int], bits_per_entry: int) -> list:
    """编码为 1.18+ packed long array"""
    entries_per_long = 64 // bits_per_entry
    longs = []
    mask = (1 << bits_per_entry) - 1

    for i in range(0, len(indices), entries_per_long):
        val = 0
        for j in range(min(entries_per_long, len(indices) - i)):
            val |= (indices[i + j] & mask) << (j * bits_per_entry)
        # 转为有符号 long
        if val >= (1 << 63):
            val -= (1 << 64)
        longs.append(Long(val))

    return longs


def get_section_by_y(chunk, section_y: int):
    """获取指定 Y 的 section（Y=-4 对应 y=-64~-49）"""
    for section in chunk["sections"]:
        if int(section["Y"]) == section_y:
            return section
    return None


def block_name(compound) -> str:
    """从 palette compound 中提取方块名"""
    return str(compound.get("Name", "minecraft:air"))


def section_y_to_world_y(section_y: int) -> int:
    """Section Y index → 世界最低 Y 坐标"""
    return section_y * 16


def world_y_to_section(world_y: int) -> tuple[int, int]:
    """世界 Y → (section_y, local_y 0-15)"""
    section_y = world_y >> 4 if world_y >= 0 else -(-world_y >> 4) - (1 if world_y % 16 != 0 else 0)
    local_y = world_y - section_y * 16
    return section_y, local_y


def get_block_at_section(section, lx: int, ly: int, lz: int, cached=None) -> str:
    """读取 section 内某个方块的名称。cached 是预解码的 (palette, indices)"""
    if cached:
        palette, indices = cached
    else:
        bs = section.get("block_states")
        if not bs:
            return "minecraft:air"
        palette = list(bs["palette"])
        if "data" not in bs:
            return block_name(palette[0]) if palette else "minecraft:air"
        bits = max(4, math.ceil(math.log2(max(len(palette), 1))))
        indices = decode_packed_indices(list(bs["data"]), bits)

    idx = ly * 256 + lz * 16 + lx  # YZX order
    return block_name(palette[indices[idx]])


def cache_section(section) -> tuple | None:
    """预解码 section 的 palette 和 indices"""
    bs = section.get("block_states")
    if not bs or "palette" not in bs:
        return None
    palette = list(bs["palette"])
    if "data" not in bs:
        return palette, [0] * 4096
    bits = max(4, math.ceil(math.log2(max(len(palette), 1))))
    indices = decode_packed_indices(list(bs["data"]), bits)
    return palette, indices


def set_block_at_section(section, lx: int, ly: int, lz: int, new_block: Compound):
    """在 section 内设置一个方块"""
    bs = section["block_states"]
    palette = list(bs["palette"])

    # 解码当前 indices
    if "data" not in bs:
        indices = [0] * 4096
    else:
        bits = max(4, math.ceil(math.log2(max(len(palette), 1))))
        indices = decode_packed_indices(list(bs["data"]), bits)

    # 查找或添加 new_block 到 palette
    new_idx = None
    new_name = str(new_block["Name"])
    new_props = new_block.get("Properties")

    for i, p in enumerate(palette):
        if str(p["Name"]) == new_name:
            p_props = p.get("Properties")
            if new_props is None and p_props is None:
                new_idx = i
                break
            if new_props and p_props and dict(new_props) == dict(p_props):
                new_idx = i
                break

    if new_idx is None:
        new_idx = len(palette)
        palette.append(new_block)

    # 设置 index
    pos = ly * 256 + lz * 16 + lx
    indices[pos] = new_idx

    # 重新编码
    bits = max(4, math.ceil(math.log2(max(len(palette), 1))))
    bs["palette"] = List[Compound](palette)
    bs["data"] = encode_packed_indices(indices, bits)


# ── 地形分析 ──────────────────────────────────────────

def find_surface_blocks(chunk) -> list[tuple[int, int, int, str]]:
    """找出 chunk 中所有地表方块位置（固体方块且上方是空气）

    返回 [(lx, world_y, lz, block_name), ...]
    """
    surfaces = []

    # 构建每列的最高固体方块
    # 遍历所有 section，从高到低
    sections_by_y = {}
    for section in chunk["sections"]:
        sy = int(section["Y"])
        cached = cache_section(section)
        if cached:
            sections_by_y[sy] = (section, cached)

    for lx in range(16):
        for lz in range(16):
            # 从最高 section 往下扫描
            found_air = True  # 顶部算空气
            for sy in sorted(sections_by_y.keys(), reverse=True):
                section, (palette, indices) = sections_by_y[sy]
                for ly in range(15, -1, -1):
                    pos = ly * 256 + lz * 16 + lx
                    name = block_name(palette[indices[pos]])

                    if name in SOLID_BLOCKS:
                        if found_air:
                            world_y = sy * 16 + ly
                            surfaces.append((lx, world_y, lz, name))
                            found_air = False
                            break  # 只取每列最高的地表
                    else:
                        found_air = True

                if not found_air:
                    break  # 已找到该列的地表

    return surfaces


# ── 装饰逻辑 ──────────────────────────────────────────

def scatter_decorations(chunk, surfaces: list, rng: random.Random) -> int:
    """在地表上方散布装饰方块，返回放置数量"""
    placed = 0

    for lx, world_y, lz, surface_name in surfaces:
        # 根据 Y 确定装饰层
        for layer_name, config in DECORATIONS.items():
            if config["y_min"] <= world_y < config["y_max"]:
                for deco_block, chance in config["surface_scatter"]:
                    if rng.random() < chance:
                        # 放在地表上方一格
                        place_y = world_y + 1
                        sy, ly = world_y_to_section(place_y)
                        section = get_section_by_y(chunk, sy)
                        if section is None:
                            continue

                        # 确认目标位置是空气
                        cached = cache_section(section)
                        if cached:
                            current = get_block_at_section(section, lx, ly, lz, cached)
                            if current == "minecraft:air":
                                set_block_at_section(section, lx, ly, lz, deco_block)
                                placed += 1
                        break  # 每个地表位置最多放一个装饰
                break  # 只匹配第一个 Y 层

    return placed


def decorate_near_magma(chunk, surfaces: list, rng: random.Random) -> int:
    """岩浆块附近放发光装饰"""
    placed = 0

    magma_positions = [
        (lx, wy, lz)
        for lx, wy, lz, name in surfaces
        if name == "minecraft:magma_block"
    ]

    for lx, wy, lz in magma_positions:
        # 在岩浆块上方放装饰
        for deco_block, chance in MAGMA_NEIGHBORS_DECORATIONS:
            if rng.random() < chance:
                place_y = wy + 1
                sy, ly = world_y_to_section(place_y)
                section = get_section_by_y(chunk, sy)
                if section is None:
                    continue
                cached = cache_section(section)
                if cached:
                    current = get_block_at_section(section, lx, ly, lz, cached)
                    if current == "minecraft:air":
                        set_block_at_section(section, lx, ly, lz, deco_block)
                        placed += 1
                break

    return placed


# ── 主流程 ────────────────────────────────────────────

def process_world(world_path: str):
    print(f"加载世界: {world_path}")
    world = mc.load(world_path)

    rng = random.Random(SEED)
    total_placed = 0
    total_chunks = 0
    stats = defaultdict(int)

    # 遍历所有 overworld region
    from mcworldlib import Dimension
    regions = world.dimensions[Dimension.OVERWORLD]["region"]

    for region_key, region in regions.items():
        print(f"  处理 region {region_key}...")
        for chunk_key, chunk in region.items():
            try:
                surfaces = find_surface_blocks(chunk)
                n1 = scatter_decorations(chunk, surfaces, rng)
                n2 = decorate_near_magma(chunk, surfaces, rng)
                total_placed += n1 + n2
                total_chunks += 1
                stats["scatter"] += n1
                stats["magma_glow"] += n2
            except Exception as e:
                print(f"    跳过 chunk {chunk_key}: {e}")

    print(f"\n后处理完成:")
    print(f"  处理 chunk 数: {total_chunks}")
    print(f"  放置装饰总数: {total_placed}")
    for k, v in stats.items():
        print(f"    {k}: {v}")

    print("保存世界...")
    world.save()
    print("完成!")


if __name__ == "__main__":
    world_path = sys.argv[1] if len(sys.argv) > 1 else WORLD_PATH
    os.chdir(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    process_world(world_path)
