"""anvil_region_writer.py — 1.20.1 Anvil region 文件写入（plan §2.1）。

Region 文件格式（每 region = 32×32 chunks = 512×512 blocks）：
  bytes 0..4095     location table (1024 entries × 4 bytes)
                    每 entry: 3 bytes BE = sector offset，1 byte = sector count
                    sector_offset=0 表示该 chunk 不存在
  bytes 4096..8191  timestamp table (1024 × 4-byte BE int)
  bytes 8192+       chunk payloads，每个对齐到 4096-byte sector 边界
                    payload 头：4 bytes BE length + 1 byte compression(2=zlib) + bytes

Chunk 在 region 内的索引：idx = (chunk_z_local << 5) | chunk_x_local
其中 local 是 chunk 相对 region 的偏移：chunk_x_local = chunk_x & 31。

server/src/world/mod.rs::is_anvil_region_file_name 用 `r\.<int>\.<int>\.mca` 模式
识别 region 文件，本模块产出文件名严格按此格式（如 `r.0.0.mca`、`r.-1.-1.mca`）。
"""

from __future__ import annotations

import struct
import time
from pathlib import Path

SECTOR_SIZE = 4096
LOCATION_TABLE_BYTES = 4096
TIMESTAMP_TABLE_BYTES = 4096
HEADER_BYTES = LOCATION_TABLE_BYTES + TIMESTAMP_TABLE_BYTES  # 8192
COMPRESSION_ZLIB = 2

CHUNKS_PER_SIDE = 32  # 一个 region 32×32 chunks
CHUNKS_PER_REGION = CHUNKS_PER_SIDE * CHUNKS_PER_SIDE  # 1024


def chunk_index_in_region(chunk_x: int, chunk_z: int) -> int:
    """1.20.1 region location table 索引：(z & 31) * 32 + (x & 31)。"""
    return ((chunk_z & 31) << 5) | (chunk_x & 31)


def region_for_chunk(chunk_x: int, chunk_z: int) -> tuple[int, int]:
    """chunk 坐标 → 该 chunk 所在 region 坐标（floorDiv 32）。"""
    return (chunk_x >> 5, chunk_z >> 5)


def region_file_name(region_x: int, region_z: int) -> str:
    return f"r.{region_x}.{region_z}.mca"


def _encode_chunk_payload(compressed: bytes) -> bytes:
    """5-byte 头 + zlib 字节 + padding 到 SECTOR_SIZE 倍数。

    长度字段 = compression byte (1) + compressed payload bytes，**不含**长度字段
    自身 4 字节。
    """
    payload_with_header = struct.pack(">I", len(compressed) + 1) + bytes(
        [COMPRESSION_ZLIB]
    ) + compressed
    pad = (-len(payload_with_header)) % SECTOR_SIZE
    return payload_with_header + (b"\x00" * pad)


def write_region(
    region_x: int,
    region_z: int,
    chunks: dict[tuple[int, int], bytes],
    output_dir: Path,
    timestamp: int | None = None,
) -> Path:
    """写一个 r.X.Z.mca 文件。

    Args:
        region_x / region_z: region 坐标（chunk_x >> 5）
        chunks: dict[(chunk_x_global, chunk_z_global)] → 已 zlib 压缩的 chunk NBT 字节。
                所有 chunk 必须落在 (region_x, region_z) 这个 region 内，否则报错。
        output_dir: 输出目录（一般是 <world>/region/）
        timestamp: 写入 timestamp 字段（unix 秒）；None = 用当前时间

    Returns:
        实际写入的文件路径
    """
    output_dir.mkdir(parents=True, exist_ok=True)
    out_path = output_dir / region_file_name(region_x, region_z)

    if timestamp is None:
        timestamp = int(time.time())

    # 校验所有 chunk 落在本 region
    for (cx, cz) in chunks:
        rx, rz = region_for_chunk(cx, cz)
        if rx != region_x or rz != region_z:
            raise ValueError(
                f"chunk ({cx}, {cz}) 不在 region ({region_x}, {region_z}) 内 "
                f"(其 region = ({rx}, {rz}))"
            )

    locations = bytearray(LOCATION_TABLE_BYTES)
    timestamps = bytearray(TIMESTAMP_TABLE_BYTES)
    chunk_payloads: list[bytes] = []
    next_sector = HEADER_BYTES // SECTOR_SIZE  # = 2

    # 按 location index 升序写，让文件 layout 与 index 顺序一致（便于 debug）
    for (cx, cz) in sorted(chunks.keys(), key=lambda k: chunk_index_in_region(*k)):
        idx = chunk_index_in_region(cx, cz)
        compressed = chunks[(cx, cz)]
        if not compressed:
            raise ValueError(f"chunk ({cx}, {cz}) 压缩字节为空")
        payload = _encode_chunk_payload(compressed)
        sector_count = len(payload) // SECTOR_SIZE
        if sector_count == 0 or sector_count > 0xFF:
            raise ValueError(
                f"chunk ({cx}, {cz}) 压缩后占 {sector_count} sectors，超 [1, 255]"
            )

        # 写 location entry：3 bytes BE offset + 1 byte count
        offset_bytes = next_sector.to_bytes(3, "big", signed=False)
        locations[idx * 4 : idx * 4 + 3] = offset_bytes
        locations[idx * 4 + 3] = sector_count

        # 写 timestamp entry
        struct.pack_into(">i", timestamps, idx * 4, timestamp)

        chunk_payloads.append(payload)
        next_sector += sector_count

    with out_path.open("wb") as f:
        f.write(bytes(locations))
        f.write(bytes(timestamps))
        for payload in chunk_payloads:
            f.write(payload)

    return out_path


def write_empty_region(
    region_x: int,
    region_z: int,
    output_dir: Path,
) -> Path:
    """写一个空 region 文件（只有 8KB 头，所有 chunks 缺席）。"""
    return write_region(region_x, region_z, {}, output_dir)
