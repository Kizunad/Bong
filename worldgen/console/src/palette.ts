// Color tables transcribed 1:1 from the Python preview exporter so the 3D
// viewer matches the PNG previews byte-for-byte.
//
//   surface colors  <- worldgen/scripts/terrain_gen/exporters.py:_surface_color (59-88)
//   zone colors     <- worldgen/scripts/terrain_gen/exporters.py:_zone_preview_color (171-181)
//   hillshade light <- worldgen/scripts/terrain_gen/exporters.py:_hillshade (382-410)
//
// surface_id indices are NOT hardcoded here — they come from manifest.surface_palette
// (id -> block name); this table only maps block name -> RGB, so adding a new
// surface name on the Python side needs only a matching entry here (else fallback).

export type RGB = [number, number, number];

/** block name -> RGB. Mirror of exporters.py `_surface_color` palette dict. */
export const SURFACE_COLORS: Record<string, RGB> = {
  stone: [111, 114, 118],
  smooth_stone: [171, 174, 174],
  dirt: [111, 90, 70],
  coarse_dirt: [120, 106, 90],
  gravel: [132, 128, 122],
  sand: [182, 170, 128],
  red_sandstone: [173, 103, 79],
  terracotta: [154, 110, 92],
  blackstone: [54, 52, 58],
  basalt: [78, 80, 84],
  magma_block: [182, 86, 28],
  crimson_nylium: [118, 36, 54],
  calcite: [229, 231, 228],
  snow_block: [246, 248, 252],
  packed_ice: [164, 198, 235],
  podzol: [103, 77, 57],
  rooted_dirt: [97, 88, 68],
  soul_sand: [88, 70, 58],
  bone_block: [224, 219, 201],
  mud: [91, 81, 72],
  clay: [126, 138, 146],
  grass_block: [93, 136, 82],
  moss_block: [78, 121, 74],
  andesite: [126, 130, 133],
  deepslate: [84, 88, 96],
  dead_bush: [140, 121, 79],
};

/** Magenta fallback (matches exporters.py `palette.get(name, (255, 0, 255))`). */
export const SURFACE_FALLBACK: RGB = [255, 0, 255];

/** zone terrain_profile -> RGB. Mirror of exporters.py `_zone_preview_color`. */
export const ZONE_COLORS: Record<string, RGB> = {
  spawn_plain: [122, 171, 104],
  broken_peaks: [160, 168, 182],
  spring_marsh: [82, 146, 122],
  rift_valley: [191, 96, 74],
  cave_network: [114, 102, 158],
  waste_plateau: [171, 144, 96],
  ash_dead_zone: [128, 126, 118],
};

/** Fallback zone swatch (exporters.py default `(214, 88, 185)`). */
export const ZONE_FALLBACK: RGB = [214, 88, 185];

/** Fixed water tint used by the preview exporter for the water overlay layer. */
export const WATER_COLOR: RGB = [74, 112, 168];

/** Hillshade directional-light vector from exporters.py `_hillshade`. */
export const LIGHT_DIR: RGB = [-0.65, -0.45, 0.62];

/** Map a surface block name to RGB (fallback magenta on unknown name). */
export function surfaceColor(name: string | undefined): RGB {
  if (name === undefined) return SURFACE_FALLBACK;
  return SURFACE_COLORS[name] ?? SURFACE_FALLBACK;
}

/** Map a surface_id index through the manifest palette to RGB. */
export function surfaceColorForId(id: number, palette: string[]): RGB {
  return surfaceColor(palette[id]);
}

/**
 * qi_density heatmap: cold blue (0) -> teal -> warm orange (1).
 * Domain clamped to [0,1] (raster clamps qi_density). Returns 0..1 RGB.
 */
export function qiHeatColor(value: number): RGB {
  const t = Math.max(0, Math.min(1, value));
  // Three-stop ramp: blue (0.20,0.35,0.85) -> teal (0.20,0.80,0.65) -> orange (0.95,0.55,0.15)
  const stops: RGB[] = [
    [0.2, 0.35, 0.85],
    [0.2, 0.8, 0.65],
    [0.95, 0.55, 0.15],
  ];
  const seg = t * (stops.length - 1);
  const i = Math.min(stops.length - 2, Math.floor(seg));
  const f = seg - i;
  const a = stops[i];
  const b = stops[i + 1];
  return [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f, a[2] + (b[2] - a[2]) * f];
}
