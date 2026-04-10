#!/usr/bin/env python3
"""末法残土 Phase B — Anvil 地形后处理

在 Phase A (Datapack + Chunky) 生成的 .mca 世界上做方块级装饰增强。
通过 mcworldlib 读写 1.20.1 Anvil 格式，不需要启动 MC 服务端。

当前框架能力：
  1. 支持读取 world blueprint（默认: ../server/zones.worldview.example.json）
  2. 按 blueprint zone 的 XZ 范围筛选需要处理的 chunk
  3. 在命中的 chunk 上执行地表装饰增强

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
  python3 scripts/postprocess.py server/mofa-world --blueprint ../server/zones.worldview.example.json
  python3 scripts/postprocess.py server/mofa-world --no-blueprint

默认 world_path: server/mofa-world
"""

import argparse
import json
import math
import os
import random
import sys
from collections import defaultdict

import mcworldlib as mc
from nbtlib import Compound, List, Long, String


# ── 常量 ──────────────────────────────────────────────

WORLD_PATH = "server/mofa-world"
DEFAULT_BLUEPRINT_PATH = os.path.join("..", "server", "zones.worldview.example.json")
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
            (
                block("soul_lantern", {"hanging": "false", "waterlogged": "false"}),
                0.001,
            ),
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
            (
                block(
                    "polished_blackstone_button",
                    {"face": "floor", "facing": "north", "powered": "false"},
                ),
                0.003,
            ),
            (block("cobweb"), 0.002),
        ],
    },
    # 低谷（Y 30-80）：发光地衣、sculk
    "low_valley": {
        "y_min": 30,
        "y_max": 80,
        "surface_scatter": [
            (
                block(
                    "sculk_sensor",
                    {
                        "sculk_sensor_phase": "inactive",
                        "power": "0",
                        "waterlogged": "false",
                    },
                ),
                0.002,
            ),
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
            (
                block(
                    "sculk_sensor",
                    {
                        "sculk_sensor_phase": "inactive",
                        "power": "0",
                        "waterlogged": "false",
                    },
                ),
                0.003,
            ),
            (
                block(
                    "sculk_vein",
                    {
                        "down": "true",
                        "east": "false",
                        "north": "false",
                        "south": "false",
                        "up": "false",
                        "waterlogged": "false",
                        "west": "false",
                    },
                ),
                0.005,
            ),
        ],
    },
}

# 岩浆块周围的发光装饰
MAGMA_NEIGHBORS_DECORATIONS = [
    (
        block(
            "glow_lichen",
            {
                "down": "true",
                "east": "false",
                "north": "false",
                "south": "false",
                "up": "false",
                "waterlogged": "false",
                "west": "false",
            },
        ),
        0.3,
    ),
    (block("shroomlight"), 0.05),
]

# 固体方块集合（用于判断是否为地表）
SOLID_BLOCKS = {
    "minecraft:netherrack",
    "minecraft:blackstone",
    "minecraft:basalt",
    "minecraft:smooth_basalt",
    "minecraft:deepslate",
    "minecraft:magma_block",
    "minecraft:nether_wart_block",
    "minecraft:red_nether_bricks",
    "minecraft:crimson_planks",
    "minecraft:polished_blackstone",
    "minecraft:polished_blackstone_bricks",
    "minecraft:dripstone_block",
}

AIR_BLOCK = block("air")


# ── Blueprint 读取 ─────────────────────────────────────


def chunk_range_for_aabb(aabb: dict) -> tuple[int, int, int, int]:
    """AABB -> 覆盖到的 chunk 范围（XZ）"""
    min_x, _, min_z = aabb["min"]
    max_x, _, max_z = aabb["max"]

    return (
        math.floor(min_x / 16),
        math.floor(max_x / 16),
        math.floor(min_z / 16),
        math.floor(max_z / 16),
    )


def normalize_blueprint_zone(zone: dict) -> dict:
    """补充 zone 后处理所需的派生字段"""
    zone_copy = dict(zone)
    zone_copy["chunk_bounds"] = chunk_range_for_aabb(zone_copy["aabb"])
    return zone_copy


def load_blueprint(blueprint_path: str | None) -> dict | None:
    """读取 world blueprint；返回 None 表示禁用 blueprint 过滤"""
    if blueprint_path is None:
        return None

    with open(blueprint_path, encoding="utf-8") as blueprint_file:
        raw = json.load(blueprint_file)

    zones = [normalize_blueprint_zone(zone) for zone in raw.get("zones", [])]
    return {
        "path": blueprint_path,
        "world": raw.get("world", {}),
        "zones": zones,
    }


def zones_for_chunk(blueprint: dict | None, chunk_x: int, chunk_z: int) -> list[dict]:
    """找出与给定 chunk 相交的所有 blueprint zones"""
    if blueprint is None:
        return []

    matched = []
    for zone in blueprint["zones"]:
        min_cx, max_cx, min_cz, max_cz = zone["chunk_bounds"]
        if min_cx <= chunk_x <= max_cx and min_cz <= chunk_z <= max_cz:
            matched.append(zone)

    return matched


# ── 方块读写工具 ─────────────────────────────────────


def decode_packed_indices(
    data_longs: list, bits_per_entry: int, total: int = 4096
) -> list[int]:
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
            val -= 1 << 64
        longs.append(Long(val))

    return longs


def block_name(compound) -> str:
    """从 palette compound 中提取方块名"""
    return str(compound.get("Name", "minecraft:air"))


def block_signature(compound: Compound) -> tuple[str, tuple[tuple[str, str], ...]]:
    """生成可哈希的方块签名，用于 palette 查找"""
    props = compound.get("Properties")
    if props is None:
        return str(compound.get("Name", "minecraft:air")), ()

    return (
        str(compound.get("Name", "minecraft:air")),
        tuple(sorted((str(key), str(value)) for key, value in props.items())),
    )


def build_palette_lookup(
    palette: list[Compound],
) -> dict[tuple[str, tuple[tuple[str, str], ...]], int]:
    """为 palette 建立 block -> index 索引"""
    return {block_signature(block): idx for idx, block in enumerate(palette)}


def section_y_to_world_y(section_y: int) -> int:
    """Section Y index → 世界最低 Y 坐标"""
    return section_y * 16


def world_y_to_section(world_y: int) -> tuple[int, int]:
    """世界 Y → (section_y, local_y 0-15)"""
    section_y = (
        world_y >> 4
        if world_y >= 0
        else -(-world_y >> 4) - (1 if world_y % 16 != 0 else 0)
    )
    local_y = world_y - section_y * 16
    return section_y, local_y


def get_block_at_cached_section(section_cache, lx: int, ly: int, lz: int) -> str:
    """读取 section cache 内某个方块的名称"""
    palette = section_cache["palette"]
    indices = section_cache["indices"]
    idx = ly * 256 + lz * 16 + lx  # YZX order
    return block_name(palette[indices[idx]])


def cache_section(section) -> dict | None:
    """预解码 section，供 chunk 内多次读写复用"""
    bs = section.get("block_states")
    if not bs or "palette" not in bs:
        return None
    palette = list(bs["palette"])
    if "data" not in bs:
        indices = [0] * 4096
    else:
        bits = max(4, math.ceil(math.log2(max(len(palette), 1))))
        indices = decode_packed_indices(list(bs["data"]), bits)

    return {
        "section": section,
        "palette": palette,
        "palette_lookup": build_palette_lookup(palette),
        "indices": indices,
        "dirty": False,
    }


def cache_chunk_sections(chunk) -> dict[int, dict]:
    """为单个 chunk 建立 section cache，避免重复解码"""
    section_caches = {}
    for section in chunk["sections"]:
        section_y = int(section["Y"])
        section_cache = cache_section(section)
        if section_cache is not None:
            section_caches[section_y] = section_cache

    return section_caches


def flush_section_cache(section_cache) -> bool:
    """将脏 section cache 回写到 NBT"""
    if not section_cache["dirty"]:
        return False

    section = section_cache["section"]
    bs = section["block_states"]
    palette = section_cache["palette"]
    bits = max(4, math.ceil(math.log2(max(len(palette), 1))))
    bs["palette"] = List[Compound](palette)
    bs["data"] = encode_packed_indices(section_cache["indices"], bits)
    section_cache["dirty"] = False
    return True


def flush_chunk_sections(section_caches: dict[int, dict]) -> int:
    """回写当前 chunk 中所有脏 section，返回提交数量"""
    flushed = 0
    for section_cache in section_caches.values():
        if flush_section_cache(section_cache):
            flushed += 1

    return flushed


def set_block_at_cached_section(
    section_cache, lx: int, ly: int, lz: int, new_block: Compound
) -> bool:
    """在 section cache 内设置一个方块，延迟到 chunk 末尾统一编码"""
    palette = section_cache["palette"]
    palette_lookup = section_cache["palette_lookup"]
    indices = section_cache["indices"]
    new_signature = block_signature(new_block)
    new_idx = palette_lookup.get(new_signature)

    if new_idx is None:
        new_idx = len(palette)
        palette.append(new_block)
        palette_lookup[new_signature] = new_idx

    pos = ly * 256 + lz * 16 + lx
    if indices[pos] == new_idx:
        return False

    indices[pos] = new_idx
    section_cache["dirty"] = True
    return True


# ── 地形分析 ──────────────────────────────────────────


def find_surface_blocks(
    section_caches: dict[int, dict],
) -> list[tuple[int, int, int, str]]:
    """找出 chunk 中所有地表方块位置（固体方块且上方是空气）

    返回 [(lx, world_y, lz, block_name), ...]
    """
    surfaces = []
    section_ys_desc = sorted(section_caches.keys(), reverse=True)

    for lx in range(16):
        for lz in range(16):
            # 从最高 section 往下扫描
            found_air = True  # 顶部算空气
            for sy in section_ys_desc:
                section_cache = section_caches[sy]
                palette = section_cache["palette"]
                indices = section_cache["indices"]
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


def scatter_decorations(
    section_caches: dict[int, dict], surfaces: list, rng: random.Random
) -> int:
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
                        section_cache = section_caches.get(sy)
                        if section_cache is None:
                            continue

                        # 确认目标位置是空气
                        current = get_block_at_cached_section(section_cache, lx, ly, lz)
                        if current == "minecraft:air" and set_block_at_cached_section(
                            section_cache, lx, ly, lz, deco_block
                        ):
                            placed += 1
                        break  # 每个地表位置最多放一个装饰
                break  # 只匹配第一个 Y 层

    return placed


def decorate_near_magma(
    section_caches: dict[int, dict], surfaces: list, rng: random.Random
) -> int:
    """岩浆块附近放发光装饰"""
    placed = 0

    magma_positions = [
        (lx, wy, lz) for lx, wy, lz, name in surfaces if name == "minecraft:magma_block"
    ]

    for lx, wy, lz in magma_positions:
        # 在岩浆块上方放装饰
        for deco_block, chance in MAGMA_NEIGHBORS_DECORATIONS:
            if rng.random() < chance:
                place_y = wy + 1
                sy, ly = world_y_to_section(place_y)
                section_cache = section_caches.get(sy)
                if section_cache is None:
                    continue

                current = get_block_at_cached_section(section_cache, lx, ly, lz)
                if current == "minecraft:air" and set_block_at_cached_section(
                    section_cache, lx, ly, lz, deco_block
                ):
                    placed += 1
                break

    return placed


# ── 主流程 ────────────────────────────────────────────


def process_world(world_path: str, blueprint: dict | None = None):
    print(f"加载世界: {world_path}")
    if blueprint is not None:
        print(f"加载蓝图: {blueprint['path']} (zones={len(blueprint['zones'])})")
    else:
        print("未启用蓝图过滤：将处理所有 overworld chunks")

    world = mc.load(world_path)

    rng = random.Random(SEED)
    total_placed = 0
    total_chunks = 0
    total_flushed_sections = 0
    total_skipped_chunks = 0
    stats = defaultdict(int)

    # 遍历所有 overworld region
    from mcworldlib import Dimension

    regions = world.dimensions[Dimension.OVERWORLD]["region"]

    for region_key, region in regions.items():
        print(f"  处理 region {region_key}...")
        for chunk_key, chunk in region.items():
            try:
                chunk_x, chunk_z = chunk_key
                matched_zones = zones_for_chunk(blueprint, chunk_x, chunk_z)
                if blueprint is not None and not matched_zones:
                    total_skipped_chunks += 1
                    continue

                section_caches = cache_chunk_sections(chunk)
                surfaces = find_surface_blocks(section_caches)
                n1 = scatter_decorations(section_caches, surfaces, rng)
                n2 = decorate_near_magma(section_caches, surfaces, rng)
                flushed_sections = flush_chunk_sections(section_caches)
                total_placed += n1 + n2
                total_chunks += 1
                total_flushed_sections += flushed_sections
                stats["scatter"] += n1
                stats["magma_glow"] += n2
                if matched_zones:
                    stats["blueprint_chunk_hits"] += 1
            except Exception as e:
                print(f"    跳过 chunk {chunk_key}: {e}")

    print(f"\n后处理完成:")
    print(f"  处理 chunk 数: {total_chunks}")
    print(f"  跳过 chunk 数: {total_skipped_chunks}")
    print(f"  放置装饰总数: {total_placed}")
    print(f"  回写 section 数: {total_flushed_sections}")
    for k, v in stats.items():
        print(f"    {k}: {v}")

    print("保存世界...")
    world.save()
    print("完成!")


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="末法残土世界后处理")
    parser.add_argument(
        "world_path",
        nargs="?",
        default=WORLD_PATH,
        help="要处理的世界目录，默认 server/mofa-world",
    )
    parser.add_argument(
        "--blueprint",
        default=DEFAULT_BLUEPRINT_PATH,
        help="world blueprint JSON 路径，默认 ../server/zones.worldview.example.json",
    )
    parser.add_argument(
        "--no-blueprint",
        action="store_true",
        help="禁用 blueprint 过滤，处理整个世界",
    )
    return parser.parse_args(argv)


if __name__ == "__main__":
    os.chdir(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    args = parse_args(sys.argv[1:])
    blueprint = None if args.no_blueprint else load_blueprint(args.blueprint)
    process_world(args.world_path, blueprint)
