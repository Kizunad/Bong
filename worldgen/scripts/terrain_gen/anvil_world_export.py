"""anvil_world_export.py — driver：synthetic / raster height_fn → 多 region 落盘
（plan §2.2）。

为何独立模块（不放 exporters.py）：exporters.py 内部用 `from .blueprint import ...`
等相对 import，被 unittest 当裸模块加载会报 ImportError。anvil 模块设计就是不依赖
worldgen blueprint / fields，纯接受 callable height_fn —— 接口先于实现锁定，real
raster 接入只换 fn body 不改函数签名。

调用：
    from anvil_world_export import export_anvil_world  # 独立 import 路径
"""

from __future__ import annotations

from pathlib import Path
from typing import Callable

# 模块内部相对 import（被当 package 加载时）+ 兼容裸模块加载
try:
    from .anvil_export import chunk_to_nbt_compressed  # type: ignore[import-not-found]
    from .anvil_region_writer import region_for_chunk, write_region  # type: ignore[import-not-found]
except ImportError:  # 单测裸 import 路径
    from anvil_export import chunk_to_nbt_compressed  # type: ignore[no-redef]
    from anvil_region_writer import region_for_chunk, write_region  # type: ignore[no-redef]


def export_anvil_world(
    output_dir: Path,
    chunk_x_min: int,
    chunk_x_max: int,
    chunk_z_min: int,
    chunk_z_max: int,
    height_fn: Callable[[int, int], int],
    *,
    top_block: str = "minecraft:grass_block",
    filler_block: str = "minecraft:stone",
    bottom_block: str = "minecraft:bedrock",
) -> dict:
    """生成 1.20.1 Anvil world 文件树。

    把 chunk 范围按 region (32×32 chunks) 分桶，对每个 chunk 调
    `height_fn(world_x, world_z) -> int` 拿 16×16 高度数组，调
    `chunk_to_nbt_compressed` 编码 + zlib 压缩，最后 `write_region` 把每个 region
    的所有 chunks 写到 `<output_dir>/region/r.X.Z.mca`。

    输出 layout（server BONG_WORLD_PATH 期望）：
        <output_dir>/
            region/
                r.X.Z.mca

    server/src/world/mod.rs::is_anvil_region_file_name 校验文件名 + 至少一个有效
    region 触发 AnvilIfPresent path（不需要 level.dat）。

    Args:
        output_dir: 世界根目录（region/ 子目录会自动建）
        chunk_x_min..chunk_x_max: chunk x 范围（含 max；max-min+1 chunks 一行）
        chunk_z_min..chunk_z_max: chunk z 范围
        height_fn: callable(world_x, world_z) -> int，返回该方块列的 grass top 的
                   y 坐标（[-64, 320)）。**接口先于实现锁定**：测试可传 lambda 端到端，
                   P2/未来 raster 接入时只换 fn body，签名不变
        top_block / filler_block / bottom_block: 方块名

    Returns:
        dict {regions_written, chunks_written, output_dir, region_dir}

    Raises:
        ValueError: chunk_x/z range 翻转 (max < min)
    """
    if chunk_x_max < chunk_x_min or chunk_z_max < chunk_z_min:
        raise ValueError(
            f"chunk range 翻转: x [{chunk_x_min}, {chunk_x_max}] z [{chunk_z_min}, {chunk_z_max}]"
        )

    region_dir = output_dir / "region"
    region_dir.mkdir(parents=True, exist_ok=True)

    chunks_by_region: dict[tuple[int, int], dict[tuple[int, int], bytes]] = {}
    for cx in range(chunk_x_min, chunk_x_max + 1):
        for cz in range(chunk_z_min, chunk_z_max + 1):
            heights = [
                [int(height_fn(cx * 16 + x, cz * 16 + z)) for x in range(16)]
                for z in range(16)
            ]
            compressed = chunk_to_nbt_compressed(
                cx, cz, heights,
                top_block=top_block,
                filler_block=filler_block,
                bottom_block=bottom_block,
            )
            rx, rz = region_for_chunk(cx, cz)
            chunks_by_region.setdefault((rx, rz), {})[(cx, cz)] = compressed

    regions_written = 0
    chunks_written = 0
    for (rx, rz), chunks in chunks_by_region.items():
        write_region(rx, rz, chunks, region_dir)
        regions_written += 1
        chunks_written += len(chunks)

    return {
        "regions_written": regions_written,
        "chunks_written": chunks_written,
        "output_dir": str(output_dir),
        "region_dir": str(region_dir),
    }


# ----- Synthetic height functions (P1 默认；P2/真实接入换 raster reader) -----


def rolling_hills_height_fn(
    base: int = 64,
    amplitude: int = 8,
    period_blocks: int = 64,
) -> Callable[[int, int], int]:
    """生成一个简单的 rolling hills height_fn：sin 波叠加。

    用作 plan §2.2 默认 backend，让 PR #78 的 5 角度 iso 视野有起伏可看（不全平）。
    P2 真实 raster 接入后此函数不再被 pipeline 调用，但留作 fixture / 单测兜底。

    base: 平均高度
    amplitude: 起伏幅度
    period_blocks: 一个完整波长的方块数
    """
    import math

    def fn(world_x: int, world_z: int) -> int:
        h = base + int(
            amplitude * (
                math.sin(world_x / period_blocks * math.pi)
                + math.cos(world_z / period_blocks * math.pi)
            ) / 2
        )
        return max(-63, min(319, h))

    return fn
