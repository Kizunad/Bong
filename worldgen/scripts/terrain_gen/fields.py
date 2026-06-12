from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import TYPE_CHECKING, Iterable

import numpy as np

if TYPE_CHECKING:
    from .blueprint import BlueprintZone, ZoneOverlaySpec


# ---------------------------------------------------------------------------
# Column spans — worldgen-v4 P0 §8.1 #1
#
# The vertical structure of a column is a small list of *solid* spans, each an
# inclusive ``(floor_y, ceiling_y)`` block range.  This single representation
# replaces the old height + sky_island_base_y/thickness + cave_mask/
# ceiling_height/entrance_mask/cavern_floor_y patch layers.
#
# Binary layout (decided in §8.1 #1, mmap-friendly fixed stride):
#   spans_count.bin : u8 per column, 0..=MAX_SPANS  (0 = full void column)
#   spans.bin       : MAX_SPANS slots per column, each slot = two little-endian
#                     i16 (floor_y, ceiling_y) = 4 bytes; column stride = 16 B.
#                     Unused slots are filled with the sentinel i16::MAX so the
#                     Rust reader can mmap at ``offset = col_idx * 16`` and skip
#                     trailing sentinels without a separate index.
#
# Y range: world Y ∈ [-64, 432) (MIN_Y=-64, WORLD_HEIGHT=496); i16 has ample
# headroom.  We reject f32 (precision waste, double the size) and variable
# length + index encoding (extra indirection for no payoff).
# ---------------------------------------------------------------------------

MAX_SPANS = 4
SPAN_SENTINEL = 32767  # i16::MAX — marks an unused slot
SPAN_MIN_Y = -64
SPAN_MAX_Y = 431  # inclusive top (world Y < 432)
SPAN_BYTES_PER_COLUMN = MAX_SPANS * 2 * 2  # 4 spans × (i16 floor + i16 ceiling)


@dataclass(frozen=True)
class ColumnSpans:
    """Up to ``MAX_SPANS`` inclusive solid ``(floor_y, ceiling_y)`` ranges.

    Spans are kept sorted bottom-to-top and must be non-overlapping with
    ``floor_y <= ceiling_y`` per span.  Construction validates these invariants
    so an illegal column can never be silently serialized.
    """

    spans: tuple[tuple[int, int], ...]

    def __post_init__(self) -> None:
        if len(self.spans) > MAX_SPANS:
            raise ValueError(
                f"ColumnSpans accepts at most {MAX_SPANS} spans, got {len(self.spans)}"
            )
        prev_ceiling: int | None = None
        for floor_y, ceiling_y in self.spans:
            if not (SPAN_MIN_Y <= floor_y <= SPAN_MAX_Y):
                raise ValueError(
                    f"span floor_y={floor_y} out of world range "
                    f"[{SPAN_MIN_Y}, {SPAN_MAX_Y}]"
                )
            if not (SPAN_MIN_Y <= ceiling_y <= SPAN_MAX_Y):
                raise ValueError(
                    f"span ceiling_y={ceiling_y} out of world range "
                    f"[{SPAN_MIN_Y}, {SPAN_MAX_Y}]"
                )
            if floor_y > ceiling_y:
                raise ValueError(
                    f"span floor_y={floor_y} must be <= ceiling_y={ceiling_y}"
                )
            if prev_ceiling is not None and floor_y <= prev_ceiling:
                raise ValueError(
                    f"spans must be sorted, non-overlapping with a gap: "
                    f"floor_y={floor_y} <= previous ceiling_y={prev_ceiling}"
                )
            prev_ceiling = ceiling_y

    @property
    def count(self) -> int:
        return len(self.spans)

    @property
    def surface_ceiling_y(self) -> int | None:
        """Top face of the lowest solid span (NPC/decoration surface anchor).

        Returns ``None`` for a fully void column.  §8.1 #2: ``query_surface``
        semantics = the lowest solid span's ceiling.
        """
        if not self.spans:
            return None
        return self.spans[0][1]

    def to_slots(self) -> list[int]:
        """Flatten to ``MAX_SPANS * 2`` i16 values, sentinel-padded."""
        slots: list[int] = []
        for floor_y, ceiling_y in self.spans:
            slots.append(floor_y)
            slots.append(ceiling_y)
        while len(slots) < MAX_SPANS * 2:
            slots.append(SPAN_SENTINEL)
        return slots

    @classmethod
    def from_slots(cls, slots: Iterable[int], count: int) -> "ColumnSpans":
        """Rebuild from a flat i16 slot list + a count byte (decode side)."""
        flat = list(slots)
        if len(flat) != MAX_SPANS * 2:
            raise ValueError(
                f"expected {MAX_SPANS * 2} slot values, got {len(flat)}"
            )
        if not (0 <= count <= MAX_SPANS):
            raise ValueError(f"span count={count} out of range [0, {MAX_SPANS}]")
        spans = tuple(
            (flat[2 * i], flat[2 * i + 1]) for i in range(count)
        )
        # Trailing slots beyond count must be the sentinel — guard against
        # corrupt encodings leaking real-looking coordinates.
        for i in range(count, MAX_SPANS):
            if flat[2 * i] != SPAN_SENTINEL or flat[2 * i + 1] != SPAN_SENTINEL:
                raise ValueError(
                    f"slot {i} beyond count={count} is not sentinel-filled: "
                    f"{flat[2 * i]}, {flat[2 * i + 1]}"
                )
        return cls(spans=spans)


def encode_spans_arrays(
    columns: list[ColumnSpans],
) -> tuple[np.ndarray, np.ndarray]:
    """Encode a column list into (spans_count uint8, spans int16) arrays.

    The returned ``spans`` array is shape ``(len(columns) * MAX_SPANS * 2,)``
    little-endian int16; ``spans_count`` is shape ``(len(columns),)`` uint8.
    Both are C-contiguous so ``.tobytes()`` yields the on-disk layout directly.
    """
    count = np.empty(len(columns), dtype=np.uint8)
    slots = np.empty(len(columns) * MAX_SPANS * 2, dtype="<i2")
    for idx, column in enumerate(columns):
        count[idx] = column.count
        base = idx * MAX_SPANS * 2
        slots[base : base + MAX_SPANS * 2] = column.to_slots()
    return count, slots


def decode_spans_column(
    count_bytes: bytes, spans_bytes: bytes, col_idx: int
) -> ColumnSpans:
    """Decode a single column from the raw binary buffers (mmap-style read)."""
    count = count_bytes[col_idx]
    base = col_idx * SPAN_BYTES_PER_COLUMN
    raw = spans_bytes[base : base + SPAN_BYTES_PER_COLUMN]
    flat = list(np.frombuffer(raw, dtype="<i2"))
    return ColumnSpans.from_slots(flat, count)

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
    density_maskable: bool = False  # zeroed inside layout-flatten radius


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
    "portal_anchor_sdf": LayerSpec(safe_default=999.0, blend_mode="minimum", export_type="float32"),
    "rim_edge_mask":    LayerSpec(safe_default=0.0,  blend_mode="maximum",  export_type="float32"),
    "fracture_mask":    LayerSpec(safe_default=0.0,  blend_mode="maximum",  export_type="float32"),
    # worldgen-v4 P0 §8.1 #1: cave_mask / ceiling_height / entrance_mask are
    # collapsed into the span representation (the cave carve is now an explicit
    # gap between two solid spans) and no longer exported as patch layers.
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
    "spirit_eye_candidates": LayerSpec(safe_default=0.0, blend_mode="maximum", export_type="uint8"),
    # realm_collapse_mask: 1 means the runtime zone overlay marked this area as
    #   ZoneStatus::Collapsed.  Consumers should preserve physical structures
    #   but disable qi-dependent functionality inside the marked columns.
    "realm_collapse_mask": LayerSpec(safe_default=0.0, blend_mode="maximum", export_type="uint8"),
    # --- vertical-dimension semantic layers (kept; §8.1 #12) ---
    # The geometric base_y/thickness/cavern_floor_y patch layers were folded
    # into spans (§8.1 #1).  sky_island_mask and underground_tier survive as
    # independent SEMANTIC layers because the 5 灵草 environment locks key off
    # them directly; deriving them from raw span geometry would be lossy.
    # sky_island_mask: 该列上空是否存在浮岛 (0~1). profile 写入浮岛核心强度，
    #   stitcher `maximum`+weight 让边界自然消退 → 浮岛视觉上边缘逐渐变薄。
    #   env_sky_island() 改由 span 高位悬空段派生 base_y/thickness（Rust 侧）。
    # underground_tier: 0=地表，1=浅洞，2=中洞，3=深渊. uint8 maximum blend.
    "sky_island_mask":      LayerSpec(safe_default=0.0,    blend_mode="maximum", export_type="float32"),
    "underground_tier":     LayerSpec(safe_default=0.0,    blend_mode="maximum", export_type="uint8"),
    # --- ecology layers ---
    # flora_density: [0,1] likelihood a decoration occupies this column.
    #   Rust consumer samples it per-chunk and rolls against per-variant rarity.
    # flora_variant_id: uint8 index into the zone profile's EcologySpec.decorations
    #   tuple (or into a merged palette — manifest declares both). 0 = none.
    "flora_density":        LayerSpec(safe_default=0.0,    blend_mode="maximum", export_type="float32", density_maskable=True),
    "flora_variant_id":     LayerSpec(safe_default=0.0,    blend_mode="swap",    export_type="uint8",   density_maskable=True),
    # ground_cover_density / ground_cover_id: 与 flora_* 平行的"地表植被层"
    # （短草/花/蕨/枯木）。flora 是稀疏特征装饰（树/灌木/巨石），ground_cover
    # 是密集地表覆盖。两层独立 swap，让一列既能长树又能长花，不再二选一。
    # 同样指向全局 GLOBAL_DECORATION_PALETTE（kind="flower" 的 spec）。
    "ground_cover_density": LayerSpec(safe_default=0.0,    blend_mode="maximum", export_type="float32", density_maskable=True),
    "ground_cover_id":      LayerSpec(safe_default=0.0,    blend_mode="swap",    export_type="uint8",   density_maskable=True),
    # zongmen_origin_id: overworld 九宗故地 origin discriminator.
    #   1=血溪 / 2=北陵 / 3=南渊 / 4=赤霞 / 5=玄水 / 6=太初 / 7=幽暗 / 0=none.
    "zongmen_origin_id":    LayerSpec(safe_default=0.0,    blend_mode="swap",    export_type="uint8"),
    # --- mineral layers (plan-mineral-v1 §2.1) ---
    # mineral_density: [0,1] likelihood a mineral ore-block occupies this column.
    #   Rust consumer samples per-chunk and rolls against per-tier rarity (品阶反比 —
    #   sui_tie / can_tie / ku_jin 极稀). `maximum` blend so zone overlays can ADD
    #   mineral pockets but never erase neighbour zone's veins.
    # mineral_kind: uint8 index into the zone profile's MineralPalette tuple
    #   (0 = none, 1..N = mineral_id from MineralRegistry order). `swap` so per-cell
    #   the dominant zone wins; 同 vanilla block 多矿 (e.g. ling_tie / dan_sha 共
    #   redstone_ore) 由此 kind 在 server 区分。
    "mineral_density":      LayerSpec(safe_default=0.0,    blend_mode="maximum", export_type="float32"),
    "mineral_kind":         LayerSpec(safe_default=0.0,    blend_mode="swap",    export_type="uint8"),
    # --- structure layers ---
    # fossil_bbox: 0 none, 1 whalefall outer ribs/periphery, 2 mineral-rich core.
    # The manifest also exports fossil_bboxes metadata so Rust can materialize
    # center/periphery mineral anchors without re-parsing blueprint POIs.
    "fossil_bbox":          LayerSpec(safe_default=0.0,    blend_mode="maximum", export_type="uint8"),
    # --- anomaly layers (event hooks for Agent / blood moon / rift systems) ---
    # anomaly_intensity: [0,1] strength of local reality-warp. Agent / event
    #   system spawns themed mobs / visual FX when intensity > threshold.
    # anomaly_kind: uint8 enum —
    #   0 none, 1 spacetime_rift, 2 qi_turbulence, 3 blood_moon_anchor,
    #   4 cursed_echo, 5 wild_formation. Declared in manifest.anomaly_kinds.
    "anomaly_intensity":    LayerSpec(safe_default=0.0,    blend_mode="maximum", export_type="float32"),
    "anomaly_kind":         LayerSpec(safe_default=0.0,    blend_mode="swap",    export_type="uint8"),
    # --- TSY-specific layers (plan-tsy-worldgen-v1 §4.1) ---
    # 仅在 TSY dim manifest 中产出；主世界 manifest 通过 raster_export.layer_whitelist 过滤。
    # tsy_presence: 1 表示 TSY family 区域内（Rust hot-path mask 查询）；
    #   maximum blend 让 family 边界外保持 0 不被覆盖。
    # tsy_origin_id: 1=daneng_luoluo / 2=zongmen_yiji / 3=zhanchang_chendian /
    #   4=gaoshou_sichu / 0=none.
    # tsy_depth_tier: 1=shallow / 2=mid / 3=deep / 0=none.
    "tsy_presence":         LayerSpec(safe_default=0.0,    blend_mode="maximum", export_type="uint8"),
    "tsy_origin_id":        LayerSpec(safe_default=0.0,    blend_mode="swap",    export_type="uint8"),
    "tsy_depth_tier":       LayerSpec(safe_default=0.0,    blend_mode="swap",    export_type="uint8"),
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


def layer_compact_dtype(layer_name: str) -> np.dtype:
    """Final in-memory dtype after a tile has finished all blend math."""
    spec = LAYER_REGISTRY.get(layer_name)
    if spec is None:
        return np.dtype(np.float64)
    if spec.export_type == "uint8":
        return np.dtype(np.uint8)
    if spec.export_type == "float32":
        return np.dtype(np.float32)
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
    # plan-terrain-wiring-v1 P0 M2 — layout fields passthrough from BlueprintZone.
    # Used by stitcher (flatten + density mask) and __main__ export pass.
    architectural_layout: str | None = None
    compound_flatten_radius: int | None = None


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
    zone_overlays: list["ZoneOverlaySpec"]
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

    def compact_layers(self) -> None:
        """Reduce finalized tile memory without changing exported raster dtypes."""
        for layer_name, values in list(self.layers.items()):
            target_dtype = layer_compact_dtype(layer_name)
            if values.dtype != target_dtype or not values.flags.c_contiguous:
                self.layers[layer_name] = np.ascontiguousarray(
                    values, dtype=target_dtype
                )


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
