// Glues the API client to the pure decoder: downloads spans + the semantic
// layers a tile carries, then decodes into a DecodedTile. three.js-free so it
// can be reused or tested independently of the renderer.

import { fetchTileLayer } from "./api";
import { decodeTile } from "./decode";
import type { DecodedTile, Manifest, ManifestTile } from "./types";

const SPANS_COUNT = "spans_count.bin";
const SPANS = "spans.bin";

/**
 * Load + decode a single tile. Spans are mandatory (every tile carries them in
 * P0); surface_id / water_level / qi_density / flora_variant_id are optional and
 * are fetched ONLY when `tile.layers` declares the tile actually wrote them.
 *
 * The manifest already tells us which optional layers each tile carries, so
 * requesting the others would just generate a 404 per absent layer per tile — a
 * 404 storm across the whole world on every load. Gating on `tile.layers`
 * fetches exactly what exists (and absent layers stay undefined, the decoder /
 * mesh fall back to safe defaults).
 */
export async function loadTile(
  manifest: Manifest,
  tile: ManifestTile,
): Promise<DecodedTile> {
  const ts = manifest.tile_size;
  // tile.layers holds bare layer names (no ".bin"); the endpoint takes the
  // file name. Only fetch a layer the manifest says this tile wrote.
  const has = (layer: string): boolean => tile.layers.includes(layer);
  const fetchIf = (layer: string): Promise<ArrayBuffer | null> =>
    has(layer)
      ? fetchTileLayer(tile.tile_x, tile.tile_z, `${layer}.bin`)
      : Promise.resolve(null);

  const [countBuf, spansBuf, surfaceBuf, waterBuf, qiBuf, floraBuf] = await Promise.all([
    fetchTileLayer(tile.tile_x, tile.tile_z, SPANS_COUNT),
    fetchTileLayer(tile.tile_x, tile.tile_z, SPANS),
    fetchIf("surface_id"),
    fetchIf("water_level"),
    fetchIf("qi_density"),
    fetchIf("flora_variant_id"),
  ]);

  if (!countBuf || !spansBuf) {
    throw new Error(
      `tile ${tile.dir} is missing span files (count=${!!countBuf}, spans=${!!spansBuf}) ` +
        `— every tile must carry spans in P0`,
    );
  }

  return decodeTile({
    tileSize: ts,
    tileX: tile.tile_x,
    tileZ: tile.tile_z,
    spansCount: new Uint8Array(countBuf),
    spans: spansBuf,
    surfaceId: surfaceBuf ? new Uint8Array(surfaceBuf) : undefined,
    waterLevel: waterBuf ? new Float32Array(waterBuf) : undefined,
    qiDensity: qiBuf ? new Float32Array(qiBuf) : undefined,
    floraVariantId: floraBuf ? new Uint8Array(floraBuf) : undefined,
  });
}

/** Tiles sorted by distance to the spawn (world origin) so the viewer loads the
 * spawn neighborhood first (matches the "fly around spawn" acceptance). */
export function tilesByDistanceToSpawn(manifest: Manifest): ManifestTile[] {
  return [...manifest.tiles].sort((a, b) => {
    const da = a.tile_x * a.tile_x + a.tile_z * a.tile_z;
    const db = b.tile_x * b.tile_x + b.tile_z * b.tile_z;
    return da - db;
  });
}
