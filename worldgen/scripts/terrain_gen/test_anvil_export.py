"""饱和测试 anvil_export — plan-worldgen-anvil-export-v1 §1.3。

跑法（仓库根）：
    python3 -m unittest discover -s worldgen/scripts/terrain_gen -p 'test_anvil_export.py'

覆盖矩阵：
  ① chunk_to_nbt happy：heights=64 整张 → round-trip 解出来 DataVersion/sections/heightmaps 对得上
  ② section Y 全覆盖：24 个 section 都生成，Y -4..19
  ③ 单 palette section（heights=-64 全部 bedrock）→ data 字段省略
  ④ 多 palette section（heights 跨 section 边界，混 stone/grass/air）→ data 长度匹配 bits packing
  ⑤ chunk 坐标负值：xPos/zPos = -25 round-trip 正确（NBT signed int）
  ⑥ Heightmap 编码：256 个 9-bit 值打 37 longs，解码回来与输入一致
  ⑦ heights 边界值：-64（最低）/ 319（最高）round-trip 不报错
  ⑧ DataVersion 钉死 3465（防 MC 版本升级时漏改）
  ⑨ 空 section（heights << section min Y）→ palette=[air]，data 省略
  ⑩ invalid input：heights 维度错 / chunk_x 非 int / heights 超出范围 → 类型/值错误
  ⑪ chunk_to_nbt_compressed 返回 zlib 压缩字节，能被 zlib.decompress 还原成 chunk_to_nbt 输出
"""

from __future__ import annotations

import struct
import sys
import unittest
import zlib
from io import BytesIO
from pathlib import Path

# 让 import 不依赖 scripts/__init__.py
sys.path.insert(0, str(Path(__file__).resolve().parent))

import anvil_export as ax  # noqa: E402


# ----- 简易 NBT 解码器（仅供测试）-----


def _read_byte(buf: BytesIO) -> int:
    return struct.unpack(">b", buf.read(1))[0]


def _read_short(buf: BytesIO) -> int:
    return struct.unpack(">h", buf.read(2))[0]


def _read_int(buf: BytesIO) -> int:
    return struct.unpack(">i", buf.read(4))[0]


def _read_long(buf: BytesIO) -> int:
    return struct.unpack(">q", buf.read(8))[0]


def _read_string(buf: BytesIO) -> str:
    n = struct.unpack(">H", buf.read(2))[0]
    return buf.read(n).decode("utf-8")


def _read_payload(buf: BytesIO, tag_id: int):
    if tag_id == ax.TAG_BYTE:
        return _read_byte(buf)
    if tag_id == ax.TAG_SHORT:
        return _read_short(buf)
    if tag_id == ax.TAG_INT:
        return _read_int(buf)
    if tag_id == ax.TAG_LONG:
        return _read_long(buf)
    if tag_id == ax.TAG_STRING:
        return _read_string(buf)
    if tag_id == ax.TAG_LIST:
        elem_tag = _read_byte(buf)
        count = _read_int(buf)
        return [_read_payload(buf, elem_tag) for _ in range(count)]
    if tag_id == ax.TAG_COMPOUND:
        return _read_compound(buf)
    if tag_id == ax.TAG_LONG_ARRAY:
        n = _read_int(buf)
        return [_read_long(buf) for _ in range(n)]
    if tag_id == ax.TAG_INT_ARRAY:
        n = _read_int(buf)
        return [_read_int(buf) for _ in range(n)]
    raise ValueError(f"unsupported tag {tag_id}")


def _read_compound(buf: BytesIO) -> dict:
    out = {}
    while True:
        tag_id = _read_byte(buf)
        if tag_id == ax.TAG_END:
            return out
        name = _read_string(buf)
        out[name] = _read_payload(buf, tag_id)


def parse_nbt(data: bytes) -> dict:
    """解 chunk_to_nbt 输出。顶层是 (TAG_COMPOUND, "", payload)。"""
    buf = BytesIO(data)
    tag_id = _read_byte(buf)
    if tag_id != ax.TAG_COMPOUND:
        raise ValueError(f"顶层不是 compound, got {tag_id}")
    name = _read_string(buf)
    if name != "":
        raise ValueError(f"顶层 compound 应 unnamed, got {name!r}")
    return _read_compound(buf)


# ----- 测试辅助 -----


def uniform_heights(value: int) -> list[list[int]]:
    return [[value] * ax.CHUNK_WIDTH for _ in range(ax.CHUNK_WIDTH)]


# ----- 测试 -----


class ChunkRoundTripTests(unittest.TestCase):
    def test_happy_path_heights_64(self):
        heights = uniform_heights(64)
        nbt = ax.chunk_to_nbt(0, 0, heights)
        chunk = parse_nbt(nbt)
        self.assertEqual(chunk["DataVersion"], 3465)
        self.assertEqual(chunk["xPos"], 0)
        self.assertEqual(chunk["zPos"], 0)
        self.assertEqual(chunk["yPos"], -4)
        self.assertEqual(chunk["Status"], "minecraft:full")
        self.assertEqual(len(chunk["sections"]), 24, "expected 24 sections (Y -4..19)")

    def test_section_y_range(self):
        chunk = parse_nbt(ax.chunk_to_nbt(0, 0, uniform_heights(64)))
        ys = sorted(s["Y"] for s in chunk["sections"])
        self.assertEqual(ys, list(range(-4, 20)))

    def test_single_palette_section_no_data(self):
        # 全 air section（section_y=10 时 world_y 160..175 都 > height=64）
        chunk = parse_nbt(ax.chunk_to_nbt(0, 0, uniform_heights(64)))
        s10 = next(s for s in chunk["sections"] if s["Y"] == 10)
        palette = s10["block_states"]["palette"]
        self.assertEqual(len(palette), 1)
        self.assertEqual(palette[0]["Name"], "minecraft:air")
        self.assertNotIn(
            "data", s10["block_states"], "palette=1 时 data 字段应省略"
        )

    def test_multi_palette_section_data_size(self):
        # 含 grass top 的 section（section_y=4 时 world_y 64..79，height=64 → grass at row 0, air rows 1..15）
        chunk = parse_nbt(ax.chunk_to_nbt(0, 0, uniform_heights(64)))
        s4 = next(s for s in chunk["sections"] if s["Y"] == 4)
        palette_names = [e["Name"] for e in s4["block_states"]["palette"]]
        # palette = [grass_block, air] 顺序按出现先后
        self.assertIn("minecraft:grass_block", palette_names)
        self.assertIn("minecraft:air", palette_names)
        # palette size 2 → bits=4 → indices_per_long=16 → 4096/16 = 256 longs
        self.assertEqual(len(s4["block_states"]["data"]), 256)

    def test_negative_chunk_coords(self):
        nbt = ax.chunk_to_nbt(-25, -25, uniform_heights(64))
        chunk = parse_nbt(nbt)
        self.assertEqual(chunk["xPos"], -25)
        self.assertEqual(chunk["zPos"], -25)

    def test_heightmap_packing(self):
        # 8 个不同 height 值（每列）→ heightmap 应能解出来
        heights = [[(z + x) % 50 + 50 for x in range(16)] for z in range(16)]
        chunk = parse_nbt(ax.chunk_to_nbt(0, 0, heights))
        # 256 heights × 9 bits / (64/9 = 7 per long) → 37 longs
        self.assertEqual(len(chunk["Heightmaps"]["MOTION_BLOCKING"]), 37)
        # 解 packing 验证第一个 height（z=0,x=0 → heights[0][0]=50 → stored 50+1-(-64)=115）
        first_long = chunk["Heightmaps"]["MOTION_BLOCKING"][0]
        if first_long < 0:
            first_long += 1 << 64  # signed → unsigned
        first_decoded = first_long & ((1 << 9) - 1)
        expected = heights[0][0] + 1 - ax.WORLD_MIN_Y
        self.assertEqual(first_decoded, expected)
        # 不同位置（z=0,x=1）
        second_decoded = (first_long >> 9) & ((1 << 9) - 1)
        self.assertEqual(second_decoded, heights[0][1] + 1 - ax.WORLD_MIN_Y)

    def test_height_boundary_low(self):
        # heights = -64（世界最底）→ 整 chunk 全 bedrock 在 y=-64，但 height=-64
        # 表示 grass_block 也在 y=-64？语义上奇怪但应不报错
        heights = uniform_heights(-64)
        nbt = ax.chunk_to_nbt(0, 0, heights)
        chunk = parse_nbt(nbt)
        self.assertEqual(len(chunk["sections"]), 24)

    def test_height_boundary_high(self):
        # heights = 319（最高）→ section 19 顶部有 grass
        heights = uniform_heights(319)
        chunk = parse_nbt(ax.chunk_to_nbt(0, 0, heights))
        s19 = next(s for s in chunk["sections"] if s["Y"] == 19)
        names = [e["Name"] for e in s19["block_states"]["palette"]]
        self.assertIn("minecraft:grass_block", names)
        self.assertNotIn("minecraft:air", names, "height=319 时 section 19 顶层都是 stone/grass，无 air")

    def test_data_version_pinned(self):
        # 钉死常量；改版本号要同步改测试
        self.assertEqual(ax.DATA_VERSION, 3465, "1.20.1 DataVersion 钉死")
        chunk = parse_nbt(ax.chunk_to_nbt(0, 0, uniform_heights(64)))
        self.assertEqual(chunk["DataVersion"], 3465)


class EmptySectionTests(unittest.TestCase):
    def test_section_above_height_is_pure_air(self):
        # height=80 → section 5 (Y 80..95) 应是 stone+air mix; section 6+ 全 air
        heights = uniform_heights(80)
        chunk = parse_nbt(ax.chunk_to_nbt(0, 0, heights))
        for s in chunk["sections"]:
            if s["Y"] > 5:
                palette = [e["Name"] for e in s["block_states"]["palette"]]
                self.assertEqual(
                    palette, ["minecraft:air"],
                    f"section Y={s['Y']} 应只含 air，实际 {palette}"
                )
                self.assertNotIn("data", s["block_states"])


class InvalidInputTests(unittest.TestCase):
    def test_heights_wrong_outer_dim(self):
        bad = [[64] * 16] * 15
        with self.assertRaises(ValueError):
            ax.chunk_to_nbt(0, 0, bad)

    def test_heights_wrong_inner_dim(self):
        bad = [[64] * 15 for _ in range(16)]
        with self.assertRaises(ValueError):
            ax.chunk_to_nbt(0, 0, bad)

    def test_heights_non_int(self):
        bad = [[64.5] * 16 for _ in range(16)]
        with self.assertRaises(TypeError):
            ax.chunk_to_nbt(0, 0, bad)

    def test_heights_out_of_range(self):
        bad = uniform_heights(400)  # > WORLD_MAX_Y
        with self.assertRaises(ValueError):
            ax.chunk_to_nbt(0, 0, bad)

    def test_heights_negative_oob(self):
        bad = uniform_heights(-100)  # < WORLD_MIN_Y
        with self.assertRaises(ValueError):
            ax.chunk_to_nbt(0, 0, bad)

    def test_chunk_x_non_int(self):
        with self.assertRaises(TypeError):
            ax.chunk_to_nbt(0.5, 0, uniform_heights(64))

    def test_chunk_z_non_int(self):
        with self.assertRaises(TypeError):
            ax.chunk_to_nbt(0, "abc", uniform_heights(64))


class CompressedOutputTests(unittest.TestCase):
    def test_compressed_round_trip(self):
        heights = uniform_heights(64)
        compressed = ax.chunk_to_nbt_compressed(0, 0, heights)
        # zlib 头：78 9c 是 default level 6
        self.assertEqual(compressed[0:1], b"\x78", "zlib header byte 1")
        decompressed = zlib.decompress(compressed)
        raw = ax.chunk_to_nbt(0, 0, heights)
        self.assertEqual(decompressed, raw, "decompress(compressed) 应等于 raw")

    def test_compressed_smaller_than_raw(self):
        heights = uniform_heights(64)
        raw = ax.chunk_to_nbt(0, 0, heights)
        compressed = ax.chunk_to_nbt_compressed(0, 0, heights)
        # 大量重复 stone/air 应高压缩比
        self.assertLess(
            len(compressed), len(raw) // 2,
            f"compressed {len(compressed)} 应 < raw {len(raw)} / 2"
        )


class BitsPerIndexTests(unittest.TestCase):
    """_bits_per_index 是 packing 核心，单独覆盖边界。"""

    def test_palette_one_returns_zero(self):
        self.assertEqual(ax._bits_per_index(1), 0)

    def test_palette_two_to_sixteen_uses_min_four(self):
        for size in [2, 4, 8, 16]:
            with self.subTest(size=size):
                self.assertEqual(
                    ax._bits_per_index(size), 4,
                    f"palette {size} 应 4 bits（min 4）"
                )

    def test_palette_seventeen_to_thirty_two(self):
        for size in [17, 32]:
            with self.subTest(size=size):
                self.assertEqual(ax._bits_per_index(size), 5)

    def test_palette_sixty_five_to_one_twenty_eight(self):
        self.assertEqual(ax._bits_per_index(65), 7)
        self.assertEqual(ax._bits_per_index(128), 7)


if __name__ == "__main__":
    unittest.main()
