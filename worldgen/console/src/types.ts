// Shared types mirroring the manifest v2 contract emitted by
// worldgen/scripts/terrain_gen/bakers/raster_export.py (§8.1 #1 / #7).
//
// Only the fields the viewer actually consumes are typed; the manifest carries
// more (anomaly_kinds, profiles_ecology, …) which we leave as an index signature.

export interface SpansEncoding {
  /** MAX_SPANS — slots per column (4). */
  max_spans: number;
  /** SPAN_BYTES_PER_COLUMN — stride in bytes per column (16). */
  bytes_per_column: number;
  /** i16::MAX (32767) — marks an unused slot. */
  sentinel: number;
  count_file: string;
  spans_file: string;
  slot_layout: string;
}

export interface WorldBounds {
  min_x: number;
  max_x: number;
  min_z: number;
  max_z: number;
}

export interface ManifestTile {
  tile_x: number;
  tile_z: number;
  dir: string;
  /** Zone names contributing to this tile. */
  zones: string[];
  /** Semantic / vertical layer names actually written for this tile. */
  layers: string[];
  /** Always true in P0 — every tile carries spans_count.bin + spans.bin. */
  spans: boolean;
}

export interface ManifestPoi {
  zone: string;
  kind: string;
  name: string;
  pos_xyz: [number, number, number];
  tags: string[];
  unlock: string;
  qi_affinity: number;
  danger_bias: number;
}

export interface Manifest {
  version: number;
  backend: string;
  world_name: string;
  tile_size: number;
  spans_encoding: SpansEncoding;
  world_bounds: WorldBounds;
  /** surface_id index -> block name; the viewer maps name -> RGB. */
  surface_palette: string[];
  biome_palette: string[];
  tiles: ManifestTile[];
  pois: ManifestPoi[];
  semantic_layers: string[];
  vertical_layers: string[];
  [key: string]: unknown;
}

/** A single solid vertical range in a column: blocks [floor_y, ceiling_y] inclusive. */
export interface Span {
  floorY: number;
  ceilingY: number;
}

/** Decoded vertical structure of one column. Empty array = full-void column. */
export type ColumnSpans = Span[];

/** A decoded tile: per-column spans + the surface palette indices for coloring. */
export interface DecodedTile {
  tileSize: number;
  tileX: number;
  tileZ: number;
  /** Length tileSize*tileSize, row-major (col_idx = local_z*tileSize + local_x). */
  columns: ColumnSpans[];
  /** Optional surface_id per column (uint8), same indexing as columns. */
  surfaceId?: Uint8Array;
  /** Optional water_level per column (float32); < 0 means no water. */
  waterLevel?: Float32Array;
  /** Optional qi_density per column (float32), clamped [0,1]. */
  qiDensity?: Float32Array;
  /** Optional flora_variant_id per column (uint8); 0 = no flora. */
  floraVariantId?: Uint8Array;
}
