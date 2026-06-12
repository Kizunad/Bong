"""anvil_export.py — Worldgen raster → 1.20.1 Anvil chunk NBT 编码（plan-worldgen-anvil-export-v1 §1）。

P0 范围：单 chunk 编码。P1/P2 在 anvil_region_writer.py + exporters.py 加 region 拼装。

设计目标（plan §0）：
  - 不依赖 MC 运行时：纯 Python 生成 NBT 字节，跑得快
  - 不引新 pip dep：style 对齐 worldgen/scripts/terrain_gen/exporters.py（仅 stdlib）

NBT 格式参考：https://wiki.vg/NBT
1.20.1 chunk 结构参考：https://minecraft.fandom.com/wiki/Chunk_format

Chunk root（unnamed compound）:
  - DataVersion: Int 3465（1.20.1，钉死常量；改版本号要同步改 tests）
  - xPos / yPos / zPos: Int chunk 坐标（yPos=-4 = 最低 section index）
  - Status: String "minecraft:full"
  - LastUpdate / InhabitedTime: Long 0
  - sections: List<Compound> — 每段 16 个 Y level，sections Y -4..19 共 24 段
  - block_entities: List<Compound> 空
  - Heightmaps: Compound { MOTION_BLOCKING / WORLD_SURFACE: LongArray 37 longs }

Section 结构:
  - Y: Byte (signed)
  - block_states: Compound
    - palette: List<Compound> [{Name: String "minecraft:stone"}]
    - data: LongArray（palette > 1 才有；palette=1 时 MC 视所有 block 为 palette[0]）
  - biomes: Compound
    - palette: List<String> ["minecraft:plains"]
    - data: LongArray（同上）
"""

from __future__ import annotations

import struct
import zlib
from io import BytesIO
from typing import Iterable

# 1.20.1 协议常量（钉死，tests 验证）
DATA_VERSION = 3465
WORLD_MIN_Y = -64
WORLD_MAX_Y = 320  # exclusive (blocks 占 -64..319)
SECTION_HEIGHT = 16
MIN_SECTION_Y = WORLD_MIN_Y // SECTION_HEIGHT  # -4
MAX_SECTION_Y = (WORLD_MAX_Y - 1) // SECTION_HEIGHT  # 19
TOTAL_SECTIONS = MAX_SECTION_Y - MIN_SECTION_Y + 1  # 24
CHUNK_WIDTH = 16  # MC chunk 是 16x16 block 列
HEIGHTMAP_BITS = 9  # 9 bits 容纳 0..384（h - WORLD_MIN_Y + 1）

# NBT TAG ID
TAG_END = 0
TAG_BYTE = 1
TAG_SHORT = 2
TAG_INT = 3
TAG_LONG = 4
TAG_FLOAT = 5
TAG_DOUBLE = 6
TAG_BYTE_ARRAY = 7
TAG_STRING = 8
TAG_LIST = 9
TAG_COMPOUND = 10
TAG_INT_ARRAY = 11
TAG_LONG_ARRAY = 12

# 默认表层方块（P0；P1 接 worldview block palette）
DEFAULT_TOP_BLOCK = "minecraft:grass_block"
DEFAULT_FILLER_BLOCK = "minecraft:stone"
DEFAULT_BOTTOM_BLOCK = "minecraft:bedrock"
AIR = "minecraft:air"


# ----- NBT 字节流编码（big-endian）-----


def _write_byte(buf: BytesIO, v: int) -> None:
    buf.write(struct.pack(">b", v))


def _write_short(buf: BytesIO, v: int) -> None:
    buf.write(struct.pack(">h", v))


def _write_int(buf: BytesIO, v: int) -> None:
    buf.write(struct.pack(">i", v))


def _write_long(buf: BytesIO, v: int) -> None:
    buf.write(struct.pack(">q", v))


def _write_string(buf: BytesIO, s: str) -> None:
    encoded = s.encode("utf-8")
    if len(encoded) > 0xFFFF:
        raise ValueError(f"NBT string too long ({len(encoded)} bytes, max 65535)")
    buf.write(struct.pack(">H", len(encoded)))
    buf.write(encoded)


def _write_long_array(buf: BytesIO, longs: Iterable[int]) -> None:
    longs = list(longs)
    _write_int(buf, len(longs))
    for v in longs:
        _write_long(buf, v)


def _write_named_tag(buf: BytesIO, tag_id: int, name: str) -> None:
    _write_byte(buf, tag_id)
    _write_string(buf, name)


def _write_compound_end(buf: BytesIO) -> None:
    _write_byte(buf, TAG_END)


def _write_list_header(buf: BytesIO, element_tag: int, count: int) -> None:
    _write_byte(buf, element_tag)
    _write_int(buf, count)


# ----- 单 section 编码 -----


def _bits_per_index(palette_size: int) -> int:
    """1.20.1 anvil block_states 包打编码：bits_per_value = max(4, ceil(log2(size)))。"""
    if palette_size <= 1:
        return 0  # palette=1 时不写 data，所有 block = palette[0]
    bits = 1
    while (1 << bits) < palette_size:
        bits += 1
    return max(4, bits)


def _pack_indexes_into_longs(indexes: list[int], bits: int) -> list[int]:
    """1.20.1 packing：每 long 装 floor(64/bits) 个 index，剩余 bits 留空（不跨 long）。"""
    if bits == 0:
        return []
    indices_per_long = 64 // bits
    longs: list[int] = []
    long_value = 0
    in_current = 0
    for idx in indexes:
        if idx >> bits != 0:
            raise ValueError(f"index {idx} 超出 {bits} bits 容量")
        long_value |= (idx & ((1 << bits) - 1)) << (in_current * bits)
        in_current += 1
        if in_current == indices_per_long:
            longs.append(_to_signed_long(long_value))
            long_value = 0
            in_current = 0
    if in_current > 0:
        longs.append(_to_signed_long(long_value))
    return longs


def _to_signed_long(unsigned: int) -> int:
    """把 unsigned 64bit 翻成 signed Java long（NBT 是 signed）。"""
    if unsigned >= (1 << 63):
        return unsigned - (1 << 64)
    return unsigned


def _section_block_palette_and_data(
    section_blocks: list[str],
) -> tuple[list[str], list[int]]:
    """4096 个方块名 → (palette, packed long indexes)。palette=1 时 indexes 空。"""
    if len(section_blocks) != 4096:
        raise ValueError(f"section_blocks 长度必须 4096，实际 {len(section_blocks)}")
    palette: list[str] = []
    name_to_idx: dict[str, int] = {}
    indexes: list[int] = []
    for name in section_blocks:
        if name not in name_to_idx:
            name_to_idx[name] = len(palette)
            palette.append(name)
        indexes.append(name_to_idx[name])
    bits = _bits_per_index(len(palette))
    longs = _pack_indexes_into_longs(indexes, bits)
    return palette, longs


def _write_block_states(buf: BytesIO, palette: list[str], data_longs: list[int]) -> None:
    """写一个 block_states compound（palette + 可选 data）。"""
    _write_named_tag(buf, TAG_LIST, "palette")
    _write_list_header(buf, TAG_COMPOUND, len(palette))
    for name in palette:
        _write_named_tag(buf, TAG_STRING, "Name")
        _write_string(buf, name)
        _write_compound_end(buf)
    if data_longs:
        _write_named_tag(buf, TAG_LONG_ARRAY, "data")
        _write_long_array(buf, data_longs)
    _write_compound_end(buf)


def _write_biomes(buf: BytesIO) -> None:
    """P0 简化：所有 section biomes palette 单条 plains，无 data。"""
    _write_named_tag(buf, TAG_LIST, "palette")
    _write_list_header(buf, TAG_STRING, 1)
    _write_string(buf, "minecraft:plains")
    # palette=1 不需要 data 字段
    _write_compound_end(buf)


def _build_section_blocks(
    section_y: int,
    heights: list[list[int]],
    top_block: str,
    filler_block: str,
    bottom_block: str,
) -> list[str]:
    """生成一个 section 的 4096 方块名列表（顺序：y-major，z 次之，x 最内）。

    每一 (x, z) 列：y == bottom_y(WORLD_MIN_Y) 放 bottom_block；y < height 放 filler；
    y == height 放 top；y > height 放 air。

    这是单段（height）路径；spans 路径见 ``_build_section_blocks_from_spans``。
    """
    blocks: list[str] = []
    section_min_y = section_y * SECTION_HEIGHT
    for local_y in range(SECTION_HEIGHT):
        world_y = section_min_y + local_y
        for z in range(CHUNK_WIDTH):
            for x in range(CHUNK_WIDTH):
                h = heights[z][x]
                if world_y == WORLD_MIN_Y:
                    blocks.append(bottom_block)
                elif world_y < h:
                    blocks.append(filler_block)
                elif world_y == h:
                    blocks.append(top_block)
                else:
                    blocks.append(AIR)
    return blocks


def _column_surface_y(spans: list[tuple[int, int]]) -> int | None:
    """Walkable surface y = ceiling of span[0] (§8.1 #2); None for a void column.

    This is the *navigation* surface (NPC pathing / decoration anchoring), which
    by §8.1 #2 convention is the ground span (``span[0]``) ceiling — deliberately
    NOT the geometrically highest block when a sky-isle floats above.  Block
    placement uses this to decide which y gets ``top_block``.
    """
    if not spans:
        return None
    return spans[0][1]


def _column_highest_solid_y(spans: list[tuple[int, int]]) -> int | None:
    """Highest solid block y in the column = ``max`` ceiling over *all* spans.

    The MC ``WORLD_SURFACE`` / ``MOTION_BLOCKING`` heightmaps encode the topmost
    non-air block of a column (+1), independent of which span is the walkable
    ground.  A sky-isle span (e.g. ``(290, 310)``) sits above ``span[0]`` and its
    blocks *are* written into the sections; the heightmap must report ``310`` so
    client lighting / occlusion / motion-blocking does not treat that range as
    open sky.  Returns ``None`` for a fully void column.
    """
    if not spans:
        return None
    return max(ceiling_y for _floor_y, ceiling_y in spans)


def _block_at_in_spans(
    world_y: int,
    spans: list[tuple[int, int]],
    surface_y: int | None,
    top_block: str,
    filler_block: str,
    bottom_block: str,
) -> str:
    """worldgen-v4 P0 §8.1 #8 — block at *world_y* given a column's spans.

    Fill order (loop over segments):
      * y == WORLD_MIN_Y → bedrock floor
      * y inside any solid span → top_block at the surface span's ceiling, else
        filler_block (caves carved below / isle floors above stay solid)
      * otherwise air (the cave void and the gap below a floating isle)
    """
    if world_y == WORLD_MIN_Y:
        return bottom_block
    for floor_y, ceiling_y in spans:
        if floor_y <= world_y <= ceiling_y:
            if surface_y is not None and world_y == surface_y:
                return top_block
            return filler_block
    return AIR


def _build_section_blocks_from_spans(
    section_y: int,
    spans_grid: list[list[list[tuple[int, int]]]],
    top_block: str,
    filler_block: str,
    bottom_block: str,
) -> list[str]:
    """spans 路径：按段 loop fill 一个 section 的 4096 方块名列表。

    ``spans_grid[z][x]`` = 该列的 solid spans 列表（inclusive (floor_y, ceiling_y)）。
    与 ``_build_section_blocks`` 同 block 顺序（y-major / z / x）。
    """
    blocks: list[str] = []
    section_min_y = section_y * SECTION_HEIGHT
    for local_y in range(SECTION_HEIGHT):
        world_y = section_min_y + local_y
        for z in range(CHUNK_WIDTH):
            for x in range(CHUNK_WIDTH):
                spans = spans_grid[z][x]
                surface_y = _column_surface_y(spans)
                blocks.append(
                    _block_at_in_spans(
                        world_y, spans, surface_y,
                        top_block, filler_block, bottom_block,
                    )
                )
    return blocks


def _write_section_blocks(buf: BytesIO, section_y: int, section_blocks: list[str]) -> None:
    """Common section serialization given the already-built 4096 block names."""
    palette, data_longs = _section_block_palette_and_data(section_blocks)

    # Y
    _write_named_tag(buf, TAG_BYTE, "Y")
    _write_byte(buf, section_y)
    # block_states
    _write_named_tag(buf, TAG_COMPOUND, "block_states")
    _write_block_states(buf, palette, data_longs)
    # biomes
    _write_named_tag(buf, TAG_COMPOUND, "biomes")
    _write_biomes(buf)
    _write_compound_end(buf)


def _write_section(
    buf: BytesIO,
    section_y: int,
    heights: list[list[int]],
    top_block: str,
    filler_block: str,
    bottom_block: str,
) -> None:
    section_blocks = _build_section_blocks(
        section_y, heights, top_block, filler_block, bottom_block
    )
    _write_section_blocks(buf, section_y, section_blocks)


def _write_section_from_spans(
    buf: BytesIO,
    section_y: int,
    spans_grid: list[list[list[tuple[int, int]]]],
    top_block: str,
    filler_block: str,
    bottom_block: str,
) -> None:
    section_blocks = _build_section_blocks_from_spans(
        section_y, spans_grid, top_block, filler_block, bottom_block
    )
    _write_section_blocks(buf, section_y, section_blocks)


# ----- Heightmap -----


def _encode_heightmap(heights: list[list[int]]) -> list[int]:
    """1.20.1 heightmap pack：256 个 9-bit 值，每 long 装 7 个（无跨 long），共 37 longs。

    存储值 = world_y - WORLD_MIN_Y + 1（grass top 上方的第一格 air，i.e. height + 1）。
    我们的 height 是 grass 那一格的 y，所以 stored = height + 1 - WORLD_MIN_Y = height + 65。
    """
    encoded_values: list[int] = []
    for z in range(CHUNK_WIDTH):
        for x in range(CHUNK_WIDTH):
            h = heights[z][x]
            stored = h + 1 - WORLD_MIN_Y
            if stored < 0 or stored >= (1 << HEIGHTMAP_BITS):
                raise ValueError(
                    f"heightmap value out of range: height={h} stored={stored} "
                    f"max={(1 << HEIGHTMAP_BITS) - 1}"
                )
            encoded_values.append(stored)

    indices_per_long = 64 // HEIGHTMAP_BITS  # 7
    longs: list[int] = []
    long_value = 0
    in_current = 0
    for v in encoded_values:
        long_value |= (v & ((1 << HEIGHTMAP_BITS) - 1)) << (in_current * HEIGHTMAP_BITS)
        in_current += 1
        if in_current == indices_per_long:
            longs.append(_to_signed_long(long_value))
            long_value = 0
            in_current = 0
    if in_current > 0:
        longs.append(_to_signed_long(long_value))
    return longs


# ----- 顶层 chunk 编码 -----


def _validate_inputs(
    chunk_x: int,
    chunk_z: int,
    heights: list[list[int]],
) -> None:
    if not isinstance(chunk_x, int) or not isinstance(chunk_z, int):
        raise TypeError(
            f"chunk_x / chunk_z 必须 int，实际 {type(chunk_x).__name__} / {type(chunk_z).__name__}"
        )
    if len(heights) != CHUNK_WIDTH:
        raise ValueError(
            f"heights 必须 {CHUNK_WIDTH}x{CHUNK_WIDTH}，外层 row 数 {len(heights)}"
        )
    for z, row in enumerate(heights):
        if len(row) != CHUNK_WIDTH:
            raise ValueError(
                f"heights[{z}] 长度必须 {CHUNK_WIDTH}，实际 {len(row)}"
            )
        for x, h in enumerate(row):
            if not isinstance(h, int):
                raise TypeError(
                    f"heights[{z}][{x}] 必须 int，实际 {type(h).__name__}"
                )
            if h < WORLD_MIN_Y or h >= WORLD_MAX_Y:
                raise ValueError(
                    f"heights[{z}][{x}]={h} 超出 [{WORLD_MIN_Y}, {WORLD_MAX_Y}) 范围"
                )


def chunk_to_nbt(
    chunk_x: int,
    chunk_z: int,
    heights: list[list[int]],
    top_block: str = DEFAULT_TOP_BLOCK,
    filler_block: str = DEFAULT_FILLER_BLOCK,
    bottom_block: str = DEFAULT_BOTTOM_BLOCK,
) -> bytes:
    """生成 1.20.1 anvil chunk NBT 字节（**未压缩**；region writer 负责 zlib 压缩）。

    Args:
        chunk_x / chunk_z: chunk 坐标（block_x // 16，可负）
        heights: 16x16 二维数组，heights[z][x] = 那一列 grass top 的 y 坐标
                 （y 范围 [-64, 320)；最高实心方块）
        top_block / filler_block / bottom_block: 方块名（minecraft:* 形式）

    Returns:
        NBT 字节串（big-endian，未压缩）。region writer 写入时再 zlib 压缩。

    Raises:
        TypeError / ValueError: 输入不合法
    """
    _validate_inputs(chunk_x, chunk_z, heights)

    buf = BytesIO()
    # 顶层是 unnamed compound。NBT 标准：先 tag id + 名（空字符串）+ payload
    _write_named_tag(buf, TAG_COMPOUND, "")

    # DataVersion
    _write_named_tag(buf, TAG_INT, "DataVersion")
    _write_int(buf, DATA_VERSION)

    # xPos / zPos（两个必填）+ yPos（最低 section index）
    _write_named_tag(buf, TAG_INT, "xPos")
    _write_int(buf, chunk_x)
    _write_named_tag(buf, TAG_INT, "yPos")
    _write_int(buf, MIN_SECTION_Y)
    _write_named_tag(buf, TAG_INT, "zPos")
    _write_int(buf, chunk_z)

    # Status
    _write_named_tag(buf, TAG_STRING, "Status")
    _write_string(buf, "minecraft:full")

    # LastUpdate / InhabitedTime（必填，0 即可）
    _write_named_tag(buf, TAG_LONG, "LastUpdate")
    _write_long(buf, 0)
    _write_named_tag(buf, TAG_LONG, "InhabitedTime")
    _write_long(buf, 0)

    # sections list
    _write_named_tag(buf, TAG_LIST, "sections")
    _write_list_header(buf, TAG_COMPOUND, TOTAL_SECTIONS)
    for section_y in range(MIN_SECTION_Y, MAX_SECTION_Y + 1):
        _write_section(
            buf, section_y, heights, top_block, filler_block, bottom_block
        )

    # block_entities（空 list；元素 tag 必须是 TAG_END 当 count=0）
    _write_named_tag(buf, TAG_LIST, "block_entities")
    _write_list_header(buf, TAG_END, 0)

    # Heightmaps
    _write_named_tag(buf, TAG_COMPOUND, "Heightmaps")
    heightmap_longs = _encode_heightmap(heights)
    _write_named_tag(buf, TAG_LONG_ARRAY, "MOTION_BLOCKING")
    _write_long_array(buf, heightmap_longs)
    _write_named_tag(buf, TAG_LONG_ARRAY, "WORLD_SURFACE")
    _write_long_array(buf, heightmap_longs)
    _write_compound_end(buf)

    # 顶层 compound 收尾
    _write_compound_end(buf)

    return buf.getvalue()


def chunk_to_nbt_compressed(
    chunk_x: int,
    chunk_z: int,
    heights: list[list[int]],
    **kwargs,
) -> bytes:
    """chunk_to_nbt 的 zlib 压缩版本，region writer 直接写入。"""
    raw = chunk_to_nbt(chunk_x, chunk_z, heights, **kwargs)
    return zlib.compress(raw, level=6)


# ----- spans 路径（worldgen-v4 P0 §8.1 #8）-----


def _encode_heightmap_from_spans(
    spans_grid: list[list[list[tuple[int, int]]]],
) -> list[int]:
    """Heightmap from per-column spans: stored value = top_solid_y + 1 - WORLD_MIN_Y.

    MC ``WORLD_SURFACE`` / ``MOTION_BLOCKING`` encode the *highest* solid block of
    the column, not the walkable ground.  When a sky-isle span floats above
    ``span[0]`` its blocks are written, so the heightmap must report that isle top
    (``max`` ceiling over all spans) — otherwise the client treats the isle range
    as open sky (broken lighting / occlusion).  A void column stores WORLD_MIN_Y
    so the heightmap stays in range (no solid above bedrock).
    """
    encoded_values: list[int] = []
    for z in range(CHUNK_WIDTH):
        for x in range(CHUNK_WIDTH):
            spans = spans_grid[z][x]
            top_solid_y = _column_highest_solid_y(spans)
            h = top_solid_y if top_solid_y is not None else WORLD_MIN_Y
            stored = h + 1 - WORLD_MIN_Y
            if stored < 0 or stored >= (1 << HEIGHTMAP_BITS):
                raise ValueError(
                    f"heightmap value out of range: surface={h} stored={stored} "
                    f"max={(1 << HEIGHTMAP_BITS) - 1}"
                )
            encoded_values.append(stored)

    indices_per_long = 64 // HEIGHTMAP_BITS
    longs: list[int] = []
    long_value = 0
    in_current = 0
    for v in encoded_values:
        long_value |= (v & ((1 << HEIGHTMAP_BITS) - 1)) << (in_current * HEIGHTMAP_BITS)
        in_current += 1
        if in_current == indices_per_long:
            longs.append(_to_signed_long(long_value))
            long_value = 0
            in_current = 0
    if in_current > 0:
        longs.append(_to_signed_long(long_value))
    return longs


def _validate_spans_grid(spans_grid: list[list[list[tuple[int, int]]]]) -> None:
    """Validate a 16x16 raw spans grid against what the renderer actually consumes.

    The renderer reads spans in **raw order**: ``_block_at_in_spans`` fills every
    span (solidity is order-independent) and ``_column_surface_y`` privileges
    ``spans[0]`` as the walkable surface (§8.1 #2).  Per the ``ColumnSpans`` spec
    (fields.py), ``spans[0]`` is the surface *by convention* — it need not be the
    geometrically lowest segment (a carved cave remnant may sit below it, a
    sky-isle above), so validation must NOT impose a positional ordering on it.

    To keep validation and consumption in the same idiom we:
      1. check per-span bounds / ``floor <= ceiling`` in **raw order** (the order
         the renderer iterates), so the failing span index matches what fills;
      2. check pairwise non-overlap with an air gap — this is an order-independent
         invariant, so a ``sorted`` view is the only correct way to verify it and
         the sort here carries no surface-ordering meaning.
    """
    if len(spans_grid) != CHUNK_WIDTH:
        raise ValueError(
            f"spans_grid 必须 {CHUNK_WIDTH}x{CHUNK_WIDTH}，外层 row 数 {len(spans_grid)}"
        )
    for z, row in enumerate(spans_grid):
        if len(row) != CHUNK_WIDTH:
            raise ValueError(f"spans_grid[{z}] 长度必须 {CHUNK_WIDTH}，实际 {len(row)}")
        for x, spans in enumerate(row):
            # Pass 1 — per-span bounds / floor<=ceiling in the consumer's raw order.
            for floor_y, ceiling_y in spans:
                if floor_y < WORLD_MIN_Y or ceiling_y >= WORLD_MAX_Y:
                    raise ValueError(
                        f"spans_grid[{z}][{x}] span ({floor_y},{ceiling_y}) 超出 "
                        f"[{WORLD_MIN_Y}, {WORLD_MAX_Y})"
                    )
                if floor_y > ceiling_y:
                    raise ValueError(
                        f"spans_grid[{z}][{x}] floor_y={floor_y} > ceiling_y={ceiling_y}"
                    )
            # Pass 2 — pairwise non-overlap with an air gap (order-independent).
            ordered = sorted(spans)
            prev_ceiling: int | None = None
            for floor_y, ceiling_y in ordered:
                if prev_ceiling is not None and floor_y <= prev_ceiling + 1:
                    raise ValueError(
                        f"spans_grid[{z}][{x}] 段重叠/相邻: floor_y={floor_y} "
                        f"<= prev ceiling_y={prev_ceiling} + 1"
                    )
                prev_ceiling = ceiling_y


def chunk_to_nbt_from_spans(
    chunk_x: int,
    chunk_z: int,
    spans_grid: list[list[list[tuple[int, int]]]],
    top_block: str = DEFAULT_TOP_BLOCK,
    filler_block: str = DEFAULT_FILLER_BLOCK,
    bottom_block: str = DEFAULT_BOTTOM_BLOCK,
) -> bytes:
    """spans 版 chunk NBT 编码：按段 loop fill 而非单 height（§8.1 #8）。

    Args:
        spans_grid: 16x16，``spans_grid[z][x]`` = 该列 inclusive (floor_y, ceiling_y)
                    solid spans 列表（来自 spans.bin 解码）。空列 = []（全虚空）。
    其余同 ``chunk_to_nbt``。
    """
    if not isinstance(chunk_x, int) or not isinstance(chunk_z, int):
        raise TypeError(
            f"chunk_x / chunk_z 必须 int，实际 {type(chunk_x).__name__} / {type(chunk_z).__name__}"
        )
    _validate_spans_grid(spans_grid)

    buf = BytesIO()
    _write_named_tag(buf, TAG_COMPOUND, "")

    _write_named_tag(buf, TAG_INT, "DataVersion")
    _write_int(buf, DATA_VERSION)
    _write_named_tag(buf, TAG_INT, "xPos")
    _write_int(buf, chunk_x)
    _write_named_tag(buf, TAG_INT, "yPos")
    _write_int(buf, MIN_SECTION_Y)
    _write_named_tag(buf, TAG_INT, "zPos")
    _write_int(buf, chunk_z)
    _write_named_tag(buf, TAG_STRING, "Status")
    _write_string(buf, "minecraft:full")
    _write_named_tag(buf, TAG_LONG, "LastUpdate")
    _write_long(buf, 0)
    _write_named_tag(buf, TAG_LONG, "InhabitedTime")
    _write_long(buf, 0)

    _write_named_tag(buf, TAG_LIST, "sections")
    _write_list_header(buf, TAG_COMPOUND, TOTAL_SECTIONS)
    for section_y in range(MIN_SECTION_Y, MAX_SECTION_Y + 1):
        _write_section_from_spans(
            buf, section_y, spans_grid, top_block, filler_block, bottom_block
        )

    _write_named_tag(buf, TAG_LIST, "block_entities")
    _write_list_header(buf, TAG_END, 0)

    _write_named_tag(buf, TAG_COMPOUND, "Heightmaps")
    heightmap_longs = _encode_heightmap_from_spans(spans_grid)
    _write_named_tag(buf, TAG_LONG_ARRAY, "MOTION_BLOCKING")
    _write_long_array(buf, heightmap_longs)
    _write_named_tag(buf, TAG_LONG_ARRAY, "WORLD_SURFACE")
    _write_long_array(buf, heightmap_longs)
    _write_compound_end(buf)

    _write_compound_end(buf)
    return buf.getvalue()


def chunk_to_nbt_from_spans_compressed(
    chunk_x: int,
    chunk_z: int,
    spans_grid: list[list[list[tuple[int, int]]]],
    **kwargs,
) -> bytes:
    """chunk_to_nbt_from_spans 的 zlib 压缩版本。"""
    raw = chunk_to_nbt_from_spans(chunk_x, chunk_z, spans_grid, **kwargs)
    return zlib.compress(raw, level=6)
