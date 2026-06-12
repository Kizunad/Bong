// Pure data layer — NO three.js. Decodes the P0 span binaries and builds voxel
// surface geometry. Kept dependency-free so it runs under headless vitest and
// the mesh law is pinned by tests (data bytes -> vertices/faces), not by eyeballing.
//
// On-disk contract (worldgen/scripts/terrain_gen/fields.py §8.1 #1):
//   spans_count.bin : u8 per column, 0..=MAX_SPANS (0 = full-void column)
//   spans.bin       : MAX_SPANS slots/col, each slot = (i16 floor_y, i16 ceiling_y)
//                     little-endian, 16 bytes/col stride. Unused slots = sentinel
//                     i16::MAX (32767). col_idx = local_z*tileSize + local_x (row-major).
//   surface = span[0].ceiling_y (the walkable top — §8.1 #2).

import type { ColumnSpans, DecodedTile, Span } from "./types";
import { qiHeatColor, surfaceColorForId, type RGB } from "./palette";

export const MAX_SPANS = 4;
export const SPAN_SENTINEL = 32767;
export const SPAN_BYTES_PER_COLUMN = MAX_SPANS * 2 * 2; // 16
/** World floor (bedrock) Y — SPAN_MIN_Y in fields.py. A span whose floor sits
 *  here touches the world bottom, so its underside is never exposed to air. */
export const SPAN_MIN_Y = -64;

export interface DecodeParams {
  tileSize: number;
  spansCount: Uint8Array;
  spans: ArrayBufferView | ArrayBuffer;
  surfaceId?: Uint8Array;
  waterLevel?: Float32Array;
  qiDensity?: Float32Array;
  floraVariantId?: Uint8Array;
  tileX?: number;
  tileZ?: number;
}

/** Decode one column straight from the byte buffers at `colIdx`. */
export function decodeColumn(
  spansCount: Uint8Array,
  spansView: DataView,
  colIdx: number,
): ColumnSpans {
  const count = spansCount[colIdx];
  if (count === 0) return []; // full-void column — caller must skip
  if (count > MAX_SPANS) {
    throw new Error(
      `column ${colIdx}: span count ${count} exceeds MAX_SPANS=${MAX_SPANS} ` +
        `— corrupt spans_count.bin (each column holds at most ${MAX_SPANS} spans)`,
    );
  }
  const base = colIdx * SPAN_BYTES_PER_COLUMN;
  const out: Span[] = [];
  for (let slot = 0; slot < count; slot++) {
    const off = base + slot * 4;
    // little-endian i16 (the `true` arg) — matches numpy "<i2" on the encode side.
    const floorY = spansView.getInt16(off, true);
    const ceilingY = spansView.getInt16(off + 2, true);
    if (floorY === SPAN_SENTINEL || ceilingY === SPAN_SENTINEL) {
      throw new Error(
        `column ${colIdx} slot ${slot}: sentinel inside count=${count} ` +
          `— spans_count and spans.bin disagree (expected a real (floor,ceiling))`,
      );
    }
    out.push({ floorY, ceilingY });
  }
  return out;
}

/**
 * Decode a whole tile into per-column spans + the semantic arrays the viewer
 * colors with. Validates buffer sizes against tileSize so a truncated download
 * fails loud here, not as silent geometry corruption downstream.
 */
export function decodeTile(p: DecodeParams): DecodedTile {
  const cols = p.tileSize * p.tileSize;
  if (p.spansCount.length !== cols) {
    throw new Error(
      `spans_count length ${p.spansCount.length} != tileSize^2 ${cols} ` +
        `— wrong tile size or truncated spans_count.bin`,
    );
  }
  const spansView =
    p.spans instanceof ArrayBuffer
      ? new DataView(p.spans)
      : new DataView(p.spans.buffer, p.spans.byteOffset, p.spans.byteLength);
  const expectedSpanBytes = cols * SPAN_BYTES_PER_COLUMN;
  if (spansView.byteLength !== expectedSpanBytes) {
    throw new Error(
      `spans byteLength ${spansView.byteLength} != cols*stride ${expectedSpanBytes} ` +
        `— truncated spans.bin (stride is ${SPAN_BYTES_PER_COLUMN} B/col)`,
    );
  }
  // Optional semantic layers are per-column (one value per col_idx), so each
  // must hold exactly tileSize² entries. Validate up front: a truncated layer
  // would otherwise read `undefined` mid-mesh and color/place geometry at NaN
  // (silent corruption) rather than failing loud here.
  const checkLayer = (name: string, arr: { length: number } | undefined): void => {
    if (arr !== undefined && arr.length !== cols) {
      throw new Error(
        `${name} length ${arr.length} != tileSize² ${cols} ` +
          `— truncated ${name}.bin (one value per column expected)`,
      );
    }
  };
  checkLayer("surface_id", p.surfaceId);
  checkLayer("water_level", p.waterLevel);
  checkLayer("qi_density", p.qiDensity);
  checkLayer("flora_variant_id", p.floraVariantId);

  const columns: ColumnSpans[] = new Array(cols);
  for (let c = 0; c < cols; c++) {
    columns[c] = decodeColumn(p.spansCount, spansView, c);
  }
  return {
    tileSize: p.tileSize,
    tileX: p.tileX ?? 0,
    tileZ: p.tileZ ?? 0,
    columns,
    surfaceId: p.surfaceId,
    waterLevel: p.waterLevel,
    qiDensity: p.qiDensity,
    floraVariantId: p.floraVariantId,
  };
}

/** Walkable surface Y for a column = span[0].ceiling_y, or null for void. */
export function surfaceY(column: ColumnSpans): number | null {
  return column.length === 0 ? null : column[0].ceilingY;
}

export type ColorMode = "terrain" | "qi";

export interface MeshParams {
  /** Sampling stride for LOD: 1 = full res, 2/4/8 = downsample. */
  stride?: number;
  /** terrain (surface palette) or qi (qi_density heatmap). */
  colorMode?: ColorMode;
  /** Vertical exaggeration for readability (1 = true scale). */
  yScale?: number;
}

/** Plain geometry buffers — three.js-free so the mesh law is unit-testable. */
export interface VoxelGeometry {
  positions: Float32Array; // xyz triples
  normals: Float32Array; // xyz triples
  colors: Float32Array; // rgb triples, 0..1
  indices: Uint32Array; // triangle indices
  /** Number of quad faces emitted (4 verts + 6 indices each). */
  faceCount: number;
}

interface Quad {
  // Four corner positions (CCW when viewed from outside) + a face color + normal.
  corners: [number, number, number][];
  normal: [number, number, number];
  color: RGB;
}

/**
 * Build voxel surface geometry from decoded spans.
 *
 * For each sampled column we emit, per solid span, the top quad (span ceiling),
 * the bottom quad (span floor — only when there is air below it, i.e. a cave
 * ceiling or sky-isle underside), and the four side quads facing neighbor
 * columns where the neighbor leaves that height exposed.
 *
 * This is a FACE-CULLED voxel mesher, NOT a greedy mesher: it culls interior
 * faces (a face hidden behind a solid neighbor is never emitted) but it does
 * NOT merge adjacent coplanar same-color quads into larger ones — every exposed
 * column face is its own quad. So a flat plateau costs one top quad PER COLUMN
 * (greedy meshing would collapse those into a single big quad). That keeps the
 * mesher dead simple and the per-face law unit-testable; this is a dev preview
 * tool and LOD stride already caps the worst case. Real greedy quad-merging for
 * the 512² ~1.3M-quad full-res case is deferred to P7 (perf follow-up).
 *
 * Single-span ground, cave (lower span under air), and sky-isle (upper span
 * floating) all fall out of the same per-span loop.
 *
 * Coordinates: world X = tileX*tileSize + localX, world Z = tileZ*tileSize +
 * localZ, with col_idx = localZ*tileSize + localX (row-major, matches Python).
 */
export function spansToVoxelGeometry(
  tile: DecodedTile,
  manifestSurfacePalette: string[],
  params: MeshParams = {},
): VoxelGeometry {
  const stride = Math.max(1, Math.floor(params.stride ?? 1));
  const colorMode = params.colorMode ?? "terrain";
  const yScale = params.yScale ?? 1;
  const ts = tile.tileSize;
  const cols = tile.columns;

  const idx = (lx: number, lz: number) => lz * ts + lx;
  const baseX = tile.tileX * ts;
  const baseZ = tile.tileZ * ts;

  // Per-column color in 0..255 (assembleQuads divides by 255). qiHeatColor
  // returns 0..1, so scale it up to keep one convention end-to-end.
  const colorOf = (colIdx: number): RGB => {
    if (colorMode === "qi" && tile.qiDensity) {
      const [r, g, b] = qiHeatColor(tile.qiDensity[colIdx]);
      return [r * 255, g * 255, b * 255];
    }
    const sid = tile.surfaceId ? tile.surfaceId[colIdx] : 0;
    return surfaceColorForId(sid, manifestSurfacePalette);
  };

  // Does the neighbor column (one stride away) cover world-Y range [floor,ceil]?
  // If it has no span overlapping [floor, ceil], the side face is exposed.
  const neighborCovers = (
    nx: number,
    nz: number,
    floorY: number,
    ceilingY: number,
  ): boolean => {
    if (nx < 0 || nz < 0 || nx >= ts || nz >= ts) return false; // tile edge: expose
    const nspans = cols[idx(nx, nz)];
    for (const s of nspans) {
      if (s.floorY <= floorY && s.ceilingY >= ceilingY) return true;
    }
    return false;
  };

  const quads: Quad[] = [];

  for (let lz = 0; lz < ts; lz += stride) {
    for (let lx = 0; lx < ts; lx += stride) {
      const cIdx = idx(lx, lz);
      const column = cols[cIdx];
      if (column.length === 0) continue; // void column — nothing to draw
      const color = colorOf(cIdx);

      const x0 = baseX + lx;
      const x1 = x0 + stride;
      const z0 = baseZ + lz;
      const z1 = z0 + stride;

      for (let si = 0; si < column.length; si++) {
        const span = column[si];
        const top = (span.ceilingY + 1) * yScale; // block ceiling occupies up to ceiling+1
        const bot = span.floorY * yScale;

        // Top face (+Y) — always visible: nothing solid sits above a span top.
        quads.push({
          normal: [0, 1, 0],
          color,
          corners: [
            [x0, top, z0],
            [x1, top, z0],
            [x1, top, z1],
            [x0, top, z1],
          ],
        });

        // Bottom face (-Y) — exposed whenever this span's floor does NOT touch
        // the world bottom (bedrock). A span resting on SPAN_MIN_Y is solid all
        // the way down, so its underside is hidden; anything floating above the
        // floor (a cave CEILING viewed from inside, a sky-isle underside, a
        // floating platform) has air below and must show its bottom.
        //
        // The old `si === 0` heuristic wrongly hid EVERY span[0] underside,
        // erasing cave ceilings (where span[0] is the carved roof region above
        // air) and floating-platform bottoms. floor_y > SPAN_MIN_Y is the
        // correct, position-based signal and is independent of span ordering.
        const touchesWorldFloor = span.floorY <= SPAN_MIN_Y;
        if (!touchesWorldFloor) {
          quads.push({
            normal: [0, -1, 0],
            color,
            corners: [
              [x0, bot, z1],
              [x1, bot, z1],
              [x1, bot, z0],
              [x0, bot, z0],
            ],
          });
        }

        // Side faces — exposed where the neighbor column doesn't cover [floor,ceil].
        // -X
        if (!neighborCovers(lx - stride, lz, span.floorY, span.ceilingY)) {
          quads.push({
            normal: [-1, 0, 0],
            color,
            corners: [
              [x0, bot, z0],
              [x0, bot, z1],
              [x0, top, z1],
              [x0, top, z0],
            ],
          });
        }
        // +X
        if (!neighborCovers(lx + stride, lz, span.floorY, span.ceilingY)) {
          quads.push({
            normal: [1, 0, 0],
            color,
            corners: [
              [x1, bot, z1],
              [x1, bot, z0],
              [x1, top, z0],
              [x1, top, z1],
            ],
          });
        }
        // -Z
        if (!neighborCovers(lx, lz - stride, span.floorY, span.ceilingY)) {
          quads.push({
            normal: [0, 0, -1],
            color,
            corners: [
              [x1, bot, z0],
              [x0, bot, z0],
              [x0, top, z0],
              [x1, top, z0],
            ],
          });
        }
        // +Z
        if (!neighborCovers(lx, lz + stride, span.floorY, span.ceilingY)) {
          quads.push({
            normal: [0, 0, 1],
            color,
            corners: [
              [x0, bot, z1],
              [x1, bot, z1],
              [x1, top, z1],
              [x0, top, z1],
            ],
          });
        }
      }
    }
  }

  return assembleQuads(quads);
}

/** Flatten quads into interleaved typed arrays (two triangles per quad). */
function assembleQuads(quads: Quad[]): VoxelGeometry {
  const faceCount = quads.length;
  const positions = new Float32Array(faceCount * 4 * 3);
  const normals = new Float32Array(faceCount * 4 * 3);
  const colors = new Float32Array(faceCount * 4 * 3);
  const indices = new Uint32Array(faceCount * 6);

  let vp = 0;
  let ii = 0;
  for (let q = 0; q < faceCount; q++) {
    const quad = quads[q];
    const baseVert = q * 4;
    for (let k = 0; k < 4; k++) {
      const c = quad.corners[k];
      positions[vp] = c[0];
      positions[vp + 1] = c[1];
      positions[vp + 2] = c[2];
      normals[vp] = quad.normal[0];
      normals[vp + 1] = quad.normal[1];
      normals[vp + 2] = quad.normal[2];
      colors[vp] = quad.color[0] / 255;
      colors[vp + 1] = quad.color[1] / 255;
      colors[vp + 2] = quad.color[2] / 255;
      vp += 3;
    }
    indices[ii] = baseVert;
    indices[ii + 1] = baseVert + 1;
    indices[ii + 2] = baseVert + 2;
    indices[ii + 3] = baseVert;
    indices[ii + 4] = baseVert + 2;
    indices[ii + 5] = baseVert + 3;
    ii += 6;
  }

  return { positions, normals, colors, indices, faceCount };
}
