from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import TYPE_CHECKING, Iterable

import numpy as np

if TYPE_CHECKING:
    from .blueprint import BlueprintZone

DEFAULT_FIELD_LAYERS = (
    "height",
    "surface_id",
    "subsurface_id",
    "water_level",
    "biome_id",
    "feature_mask",
    "boundary_weight",
)


# ---------------------------------------------------------------------------
# Layer registry — single source of truth for defaults, blend modes, and
# export types.  Every layer used anywhere in the pipeline must appear here.
#
#   safe_default: value for columns with no zone data.  Must match the Rust
#                 wilderness.rs / column.rs "no effect" semantics.
#   blend_mode:   how the stitcher combines base (wilderness) with zone overlay.
#                 "maximum"  — higher value = stronger effect (masks, weights)
#                 "minimum"  — lower value = stronger effect (SDF distances)
#                 "lerp"     — linear interpolation by boundary weight
#                 "swap"     — discrete swap at dithered threshold
#                 "special"  — handled by dedicated stitcher code (height, water…)
#   export_type:  "float32" or "uint8" for raster binary serialization.
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class LayerSpec:
    safe_default: float
    blend_mode: str   # maximum | minimum | lerp | swap | special
    export_type: str  # float32 | uint8


LAYER_REGISTRY: dict[str, LayerSpec] = {
    # --- core layers (blended by dedicated stitcher code) ---
    "height":           LayerSpec(safe_default=0.0,  blend_mode="special",  export_type="float32"),
    "surface_id":       LayerSpec(safe_default=0.0,  blend_mode="swap",     export_type="uint8"),
    "subsurface_id":    LayerSpec(safe_default=0.0,  blend_mode="swap",     export_type="uint8"),
    "water_level":      LayerSpec(safe_default=-1.0, blend_mode="special",  export_type="float32"),
    "biome_id":         LayerSpec(safe_default=0.0,  blend_mode="swap",     export_type="uint8"),
    "feature_mask":     LayerSpec(safe_default=0.0,  blend_mode="special",  export_type="float32"),
    "boundary_weight":  LayerSpec(safe_default=0.0,  blend_mode="special",  export_type="float32"),
    # --- zone-specific layers (blended by extra-layer loop) ---
    "rift_axis_sdf":    LayerSpec(safe_default=99.0, blend_mode="minimum",  export_type="float32"),
    "rim_edge_mask":    LayerSpec(safe_default=0.0,  blend_mode="maximum",  export_type="float32"),
    "fracture_mask":    LayerSpec(safe_default=0.0,  blend_mode="maximum",  export_type="float32"),
    "cave_mask":        LayerSpec(safe_default=0.0,  blend_mode="maximum",  export_type="float32"),
    "ceiling_height":   LayerSpec(safe_default=0.0,  blend_mode="maximum",  export_type="float32"),
    "entrance_mask":    LayerSpec(safe_default=0.0,  blend_mode="maximum",  export_type="float32"),
    "neg_pressure":     LayerSpec(safe_default=0.0,  blend_mode="maximum",  export_type="float32"),
    "ruin_density":     LayerSpec(safe_default=0.0,  blend_mode="maximum",  export_type="float32"),
    # --- xianxia / mofa semantic layers ---
    # qi_density: 灵气浓度 (0~1). Baseline of mofa world is "thin qi"; zones like
    #   spring_marsh lift it, waste_plateau flatlines it. `lerp` lets overlays
    #   raise OR lower the base value smoothly across zone boundaries.
    # mofa_decay: 末法腐朽度 (0~1). Conceptual dual of qi_density — a region can
    #   have low qi but not yet decayed (pristine but silent), or be fully
    #   decayed with residual qi (cursed land). Also `lerp` blended.
    # qi_vein_flow: 灵脉流向强度 (0~1). Sparse linear structure; `maximum` so
    #   overlays only add veins, never erase nearby zone's vein trails.
    "qi_density":       LayerSpec(safe_default=0.12, blend_mode="lerp",     export_type="float32"),
    "mofa_decay":       LayerSpec(safe_default=0.40, blend_mode="lerp",     export_type="float32"),
    "qi_vein_flow":     LayerSpec(safe_default=0.0,  blend_mode="maximum",  export_type="float32"),
}


def layer_storage_dtype(layer_name: str) -> np.dtype:
    """Internal storage dtype for a layer.

    Discrete-id layers (export_type == "uint8") live in memory as uint8 so they
    survive np.where / blending without silent up-casts to int32/64.  Continuous
    layers stay in float64 to preserve mid-pipeline precision; the raster baker
    downcasts to float32 only at the final write boundary.
    """
    spec = LAYER_REGISTRY.get(layer_name)
    if spec is None:
        return np.dtype(np.float64)
    if spec.export_type == "uint8":
        return np.dtype(np.uint8)
    return np.dtype(np.float64)


@dataclass(frozen=True)
class Bounds2D:
    min_x: int
    max_x: int
    min_z: int
    max_z: int

    @property
    def width(self) -> int:
        return self.max_x - self.min_x + 1

    @property
    def depth(self) -> int:
        return self.max_z - self.min_z + 1

    def tile_range(self, tile_size: int) -> tuple[int, int, int, int]:
        return (
            self.min_x // tile_size,
            self.max_x // tile_size,
            self.min_z // tile_size,
            self.max_z // tile_size,
        )

    def expanded(self, margin: int) -> "Bounds2D":
        return Bounds2D(
            min_x=self.min_x - margin,
            max_x=self.max_x + margin,
            min_z=self.min_z - margin,
            max_z=self.max_z + margin,
        )

    def intersects(self, other: "Bounds2D") -> bool:
        return not (
            self.max_x < other.min_x
            or self.min_x > other.max_x
            or self.max_z < other.min_z
            or self.min_z > other.max_z
        )


@dataclass(frozen=True)
class WorldTile:
    tile_x: int
    tile_z: int
    min_x: int
    max_x: int
    min_z: int
    max_z: int

    @property
    def tile_id(self) -> str:
        return f"tile_{self.tile_x}_{self.tile_z}"

    @property
    def bounds(self) -> Bounds2D:
        return Bounds2D(
            min_x=self.min_x,
            max_x=self.max_x,
            min_z=self.min_z,
            max_z=self.max_z,
        )


def build_world_tiles(bounds: Bounds2D, tile_size: int) -> list[WorldTile]:
    min_tx, max_tx, min_tz, max_tz = bounds.tile_range(tile_size)
    tiles: list[WorldTile] = []
    for tile_z in range(min_tz, max_tz + 1):
        for tile_x in range(min_tx, max_tx + 1):
            min_x = tile_x * tile_size
            min_z = tile_z * tile_size
            tiles.append(
                WorldTile(
                    tile_x=tile_x,
                    tile_z=tile_z,
                    min_x=min_x,
                    max_x=min_x + tile_size - 1,
                    min_z=min_z,
                    max_z=min_z + tile_size - 1,
                )
            )
    return tiles


@dataclass
class WildernessFieldPlan:
    profile_name: str
    bounds_xz: Bounds2D
    required_layers: tuple[str, ...]
    notes: tuple[str, ...] = ()


@dataclass
class ZoneFieldPlan:
    zone_name: str
    display_name: str
    profile_name: str
    generator_name: str
    shape: str
    bounds_xz: Bounds2D
    boundary_mode: str
    boundary_width: int
    required_layers: tuple[str, ...]
    extra_layers: tuple[str, ...] = ()
    landmarks: tuple[str, ...] = ()
    notes: tuple[str, ...] = ()


@dataclass
class BakePlan:
    backend: str
    output_dir: Path
    artifacts: dict[str, Path]
    notes: tuple[str, ...] = ()


@dataclass
class TerrainGenerationPlan:
    world_name: str
    blueprint_path: Path
    profiles_path: Path
    output_dir: Path
    world_bounds: Bounds2D
    tile_size: int
    tiles: list[WorldTile]
    wilderness: WildernessFieldPlan
    blueprint_zones: list["BlueprintZone"]
    zone_plans: list[ZoneFieldPlan]
    stitch_strategy: str
    bake_plan: BakePlan | None = None
    notes: tuple[str, ...] = ()

    @property
    def tile_count(self) -> int:
        return len(self.tiles)


@dataclass
class SurfacePalette:
    names: list[str] = field(default_factory=list)
    ids_by_name: dict[str, int] = field(default_factory=dict)

    def ensure(self, surface_name: str) -> int:
        if surface_name not in self.ids_by_name:
            surface_id = len(self.names)
            self.names.append(surface_name)
            self.ids_by_name[surface_name] = surface_id
        return self.ids_by_name[surface_name]

    def extend(self, names: Iterable[str]) -> None:
        for name in names:
            self.ensure(name)


@dataclass
class TileFieldBuffer:
    tile: WorldTile
    tile_size: int
    layers: dict[str, np.ndarray]
    contributing_zones: list[str] = field(default_factory=list)

    @classmethod
    def create(
        cls, tile: WorldTile, tile_size: int, layer_names: Iterable[str]
    ) -> "TileFieldBuffer":
        area = tile_size * tile_size
        layers: dict[str, np.ndarray] = {}
        for name in layer_names:
            spec = LAYER_REGISTRY.get(name)
            default = spec.safe_default if spec is not None else 0.0
            layers[name] = np.full(area, default, dtype=layer_storage_dtype(name))
        return cls(tile=tile, tile_size=tile_size, layers=layers)

    def index(self, local_x: int, local_z: int) -> int:
        return local_z * self.tile_size + local_x

    def set_value(
        self, layer_name: str, local_x: int, local_z: int, value: float | int
    ) -> None:
        self.layers[layer_name][self.index(local_x, local_z)] = value

    def get_value(self, layer_name: str, local_x: int, local_z: int) -> float | int:
        return self.layers[layer_name][self.index(local_x, local_z)].item()

    def set_index_value(self, layer_name: str, index: int, value: float | int) -> None:
        self.layers[layer_name][index] = value

    def get_index_value(self, layer_name: str, index: int) -> float | int:
        return self.layers[layer_name][index].item()

    def layer_stats(self, layer_name: str) -> tuple[float | int, float | int]:
        arr = self.layers[layer_name]
        if arr.size == 0:
            return 0, 0
        return arr.min().item(), arr.max().item()


@dataclass(frozen=True)
class GeneratedTileSummary:
    tile_id: str
    zone_names: tuple[str, ...]
    layer_stats: dict[str, tuple[float | int, float | int]]


@dataclass
class GeneratedFieldSet:
    tile_size: int
    surface_palette: SurfacePalette
    layers: tuple[str, ...]
    tiles: list[TileFieldBuffer]
    notes: tuple[str, ...] = ()

    def summaries(self) -> list[GeneratedTileSummary]:
        summaries: list[GeneratedTileSummary] = []
        for tile in self.tiles:
            summaries.append(
                GeneratedTileSummary(
                    tile_id=tile.tile.tile_id,
                    zone_names=tuple(tile.contributing_zones),
                    layer_stats={
                        layer_name: tile.layer_stats(layer_name)
                        for layer_name in self.layers
                    },
                )
            )
        return summaries
