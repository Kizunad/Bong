"""从本地 BongWorldGen 生成一个带野外灵植刷新点的最小 raster 样例。

这个探针不修改 BongWorldGen 仓库。它复用该库已有的地表资源候选，
将植物资源块转换成服务端消费的 ``wild_plant_points.json`` 稀疏锚点。
真实世界生成仍由 BongWorldGen 负责，植物行为和再生账本由 server 负责。
"""

from __future__ import annotations

import argparse
import json
from dataclasses import replace
from pathlib import Path

from bong_worldgen.adapters import to_bong_tile, write_bong_raster
from bong_worldgen.data.zone_recipes import LINGQUAN_TERRAIN
from bong_worldgen.engine import (
    MountainRange,
    Point,
    TerrainRecipe,
    ZoneResourceSettings,
    generate_heightfield,
)


ANCHOR_X = -2240
ANCHOR_Z = 2752
TILE_SIZE = 16
DEFAULT_SEED = 812731
DEFAULT_PLANT_ID = "gu_yuan_gen"


def build_recipe() -> TerrainRecipe:
    """在灵泉湿地的灵草甸附近增加一条仅用于样例的候选山脊。"""

    local_zone = replace(
        LINGQUAN_TERRAIN,
        wetland=None,
        mountains=(
            MountainRange(
                path=(Point(ANCHOR_X - 16, ANCHOR_Z + 8), Point(ANCHOR_X + 32, ANCHOR_Z + 8)),
                width=48.0,
                height=0.0,
                base_elevation=68.0,
                summit_elevation=74.0,
                spine_warp_strength=0.0,
                width_variation=0.0,
            ),
        ),
        resources=ZoneResourceSettings(
            grid_spacing=4,
            ore_probability=0.0,
            plant_probability=1.0,
            ridge_width=64.0,
            off_ridge_plant_multiplier=1.0,
            minimum_elevation=0.0,
            ore_materials=(),
            ore_rarities=(),
            plant_materials=("fern", "moss_carpet"),
            plant_rarities=("多", "中"),
        ),
    )
    return TerrainRecipe(
        name="wild_botany_preview",
        base_height=68.0,
        terrain_zones=(local_zone,),
    )


def generate_preview(output: Path, *, seed: int, plant_id: str) -> tuple[Path, int]:
    recipe = build_recipe()
    field = generate_heightfield(
        recipe,
        width=TILE_SIZE,
        height=TILE_SIZE,
        origin_x=ANCHOR_X,
        origin_z=ANCHOR_Z,
        seed=seed,
    )
    resource_blocks = [
        block
        for block in field.underground_blocks
        if block.material in {"minecraft:fern", "minecraft:moss_carpet"}
    ]
    # 灵泉湿地当前气候样例可能把 16×16 窗口投影到沙质可见层，
    # 旧版 zone resource 过滤器会因此不放置 fern。此时仍从同一个
    # Heightfield 读取可站立地表，保留一个确定的 gameplay 刷新锚点。
    positions = [(block.x, block.y, block.z) for block in resource_blocks]
    if not positions:
        local_x = local_z = TILE_SIZE // 2
        positions.append(
            (
                ANCHOR_X + local_x,
                int(round(field.height[local_z, local_x])) + 1,
                ANCHOR_Z + local_z,
            )
        )

    tile = to_bong_tile(field, sea_level=62.0)
    tile_x, tile_z = ANCHOR_X // TILE_SIZE, ANCHOR_Z // TILE_SIZE
    manifest_path = write_bong_raster(
        tile,
        output,
        tile_x=tile_x,
        tile_z=tile_z,
        world_name=recipe.name,
    )
    points = [
        {
            "id": index,
            "plant_id": plant_id,
            "zone_name": "lingquan_marsh",
            "position": list(position),
            "regen_ticks": 7200,
        }
        for index, position in enumerate(positions, start=1)
    ]
    sidecar = output / "wild_plant_points.json"
    sidecar.write_text(
        json.dumps({"version": 1, "points": points}, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    return manifest_path, len(points)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--seed", type=int, default=DEFAULT_SEED)
    parser.add_argument("--plant-id", default=DEFAULT_PLANT_ID)
    args = parser.parse_args()
    manifest, count = generate_preview(args.output, seed=args.seed, plant_id=args.plant_id)
    print(f"manifest={manifest}")
    print(f"wild_plant_points={count} plant_id={args.plant_id}")


if __name__ == "__main__":
    main()
