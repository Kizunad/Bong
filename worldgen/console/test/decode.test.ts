// Headless pins for the data->geometry core (decode.ts). No three.js, no DOM.
// These lock the byte layout decode AND the voxel mesh law so a regression in
// either (wrong offset, wrong sentinel handling, dropped face culling, broken
// multi-span cave/sky-isle shapes) turns the suite red.

import { describe, it, expect } from "vitest";
import {
  MAX_SPANS,
  SPAN_BYTES_PER_COLUMN,
  SPAN_SENTINEL,
  decodeColumn,
  decodeTile,
  spansToVoxelGeometry,
  surfaceY,
} from "../src/decode";
import { encodeColumns, fillColumns, type SpanTuple } from "./fixtures";

const PALETTE = ["stone", "grass_block", "dirt", "water"];

function tileFrom(tileSize: number, columns: SpanTuple[][], extra: Partial<{
  surfaceId: Uint8Array;
  qiDensity: Float32Array;
  waterLevel: Float32Array;
}> = {}) {
  const { spansCount, spans } = encodeColumns(tileSize, columns);
  return decodeTile({ tileSize, spansCount, spans, ...extra });
}

// ---------------------------------------------------------------------------
// Byte-layout decode
// ---------------------------------------------------------------------------

describe("decode constants match the P0 contract", () => {
  it("stride is 16 bytes (4 spans * (i16 floor + i16 ceiling))", () => {
    expect(MAX_SPANS).toBe(4);
    expect(SPAN_BYTES_PER_COLUMN).toBe(16);
    expect(SPAN_SENTINEL).toBe(32767);
  });
});

describe("decodeColumn reads (floor,ceiling) i16-LE at col_idx*16", () => {
  it("decodes a single ground span", () => {
    const { spansCount, spans } = encodeColumns(1, [[[-64, 71]]]);
    const view = new DataView(spans);
    const col = decodeColumn(spansCount, view, 0);
    expect(col).toEqual([{ floorY: -64, ceilingY: 71 }]);
  });

  it("decodes a multi-span column in slot order (surface first)", () => {
    // span[0] = ground (surface=80), span[1] = sky isle floating above.
    const { spansCount, spans } = encodeColumns(1, [[[-64, 80], [200, 210]]]);
    const view = new DataView(spans);
    const col = decodeColumn(spansCount, view, 0);
    expect(col).toEqual([
      { floorY: -64, ceilingY: 80 },
      { floorY: 200, ceilingY: 210 },
    ]);
    // §8.1 #2: surface = span[0].ceiling, NOT the geometrically highest span.
    expect(surfaceY(col)).toBe(80);
  });

  it("returns [] for a full-void column (count=0)", () => {
    const { spansCount, spans } = encodeColumns(1, [[]]);
    const view = new DataView(spans);
    expect(decodeColumn(spansCount, view, 0)).toEqual([]);
    expect(surfaceY([])).toBeNull();
  });

  it("reads little-endian: a negative floor_y is not byte-swapped", () => {
    // -64 LE = 0xC0 0xFF; if read big-endian it'd be -16129. Pin the LE law.
    const { spansCount, spans } = encodeColumns(1, [[[-64, -1]]]);
    const bytes = new Uint8Array(spans);
    expect(bytes[0]).toBe(0xc0);
    expect(bytes[1]).toBe(0xff);
    const col = decodeColumn(spansCount, new DataView(spans), 0);
    expect(col[0].floorY).toBe(-64);
    expect(col[0].ceilingY).toBe(-1);
  });

  it("throws when a sentinel sits inside the declared count (count/spans disagree)", () => {
    // Hand-corrupt: count says 2 but slot 1 is sentinel-filled.
    const { spans } = encodeColumns(1, [[[-64, 70]]]);
    const badCount = new Uint8Array([2]);
    expect(() => decodeColumn(badCount, new DataView(spans), 0)).toThrow(/sentinel inside count/);
  });

  it("throws when count exceeds MAX_SPANS", () => {
    const { spans } = encodeColumns(1, [[[-64, 70]]]);
    const badCount = new Uint8Array([5]);
    expect(() => decodeColumn(badCount, new DataView(spans), 0)).toThrow(/exceeds MAX_SPANS/);
  });
});

describe("decodeTile validates buffer sizes", () => {
  it("decodes every column row-major (col_idx = z*ts + x)", () => {
    // 2x2 tile: distinct surface per column so we can verify indexing.
    const cols: SpanTuple[][] = [
      [[-64, 60]], // (x0,z0)
      [[-64, 61]], // (x1,z0)
      [[-64, 62]], // (x0,z1)
      [[-64, 63]], // (x1,z1)
    ];
    const tile = tileFrom(2, cols);
    expect(tile.columns.length).toBe(4);
    expect(surfaceY(tile.columns[0 * 2 + 0])).toBe(60);
    expect(surfaceY(tile.columns[0 * 2 + 1])).toBe(61);
    expect(surfaceY(tile.columns[1 * 2 + 0])).toBe(62);
    expect(surfaceY(tile.columns[1 * 2 + 1])).toBe(63);
  });

  it("rejects a truncated spans_count buffer", () => {
    const { spans } = encodeColumns(2, fillColumns(2, [[-64, 60]]));
    const shortCount = new Uint8Array(3); // should be 4
    expect(() => decodeTile({ tileSize: 2, spansCount: shortCount, spans })).toThrow(
      /spans_count length/,
    );
  });

  it("rejects a truncated spans buffer", () => {
    const { spansCount } = encodeColumns(2, fillColumns(2, [[-64, 60]]));
    const shortSpans = new ArrayBuffer(3 * SPAN_BYTES_PER_COLUMN); // should be 4
    expect(() =>
      decodeTile({ tileSize: 2, spansCount, spans: shortSpans }),
    ).toThrow(/byteLength/);
  });
});

// ---------------------------------------------------------------------------
// Voxel mesh law
// ---------------------------------------------------------------------------

function quadCount(geo: { faceCount: number; indices: Uint32Array; positions: Float32Array }): number {
  // Each quad = 4 verts + 6 indices. Cross-check the bookkeeping.
  expect(geo.indices.length).toBe(geo.faceCount * 6);
  expect(geo.positions.length).toBe(geo.faceCount * 4 * 3);
  return geo.faceCount;
}

describe("spansToVoxelGeometry — single ground column (1x1 tile)", () => {
  it("emits a top + 4 exposed sides, no bottom (ground underside hidden)", () => {
    const tile = tileFrom(1, [[[-64, 71]]]);
    const geo = spansToVoxelGeometry(tile, PALETTE);
    // 1 top + 0 bottom (ground) + 4 sides = 5 faces.
    expect(quadCount(geo)).toBe(5);
    // Non-empty, real vertices.
    expect(geo.positions.length).toBeGreaterThan(0);
    // Top face Y = (ceiling+1) = 72.
    const ys = Array.from(geo.positions).filter((_, i) => i % 3 === 1);
    expect(Math.max(...ys)).toBe(72);
    expect(Math.min(...ys)).toBe(-64);
  });

  it("colors vertices from the surface palette via surface_id", () => {
    const tile = tileFrom(1, [[[-64, 71]]], { surfaceId: new Uint8Array([1]) }); // grass_block
    const geo = spansToVoxelGeometry(tile, PALETTE, { colorMode: "terrain" });
    // grass_block = (93,136,82) -> /255.
    expect(geo.colors[0]).toBeCloseTo(93 / 255, 5);
    expect(geo.colors[1]).toBeCloseTo(136 / 255, 5);
    expect(geo.colors[2]).toBeCloseTo(82 / 255, 5);
  });

  it("qi color mode maps qi_density through the heatmap, not the surface palette", () => {
    const tile = tileFrom(1, [[[-64, 71]]], {
      surfaceId: new Uint8Array([1]),
      qiDensity: new Float32Array([1.0]), // max -> warm orange end of ramp
    });
    const geo = spansToVoxelGeometry(tile, PALETTE, { colorMode: "qi" });
    // qiHeatColor(1) = (0.95, 0.55, 0.15); the orange channel dominates.
    expect(geo.colors[0]).toBeGreaterThan(geo.colors[2]);
    expect(geo.colors[0]).toBeCloseTo(0.95, 2);
  });
});

describe("spansToVoxelGeometry — void column produces no geometry", () => {
  it("skips count=0 columns entirely", () => {
    const tile = tileFrom(1, [[]]);
    const geo = spansToVoxelGeometry(tile, PALETTE);
    expect(geo.faceCount).toBe(0);
    expect(geo.positions.length).toBe(0);
    expect(geo.indices.length).toBe(0);
  });
});

describe("spansToVoxelGeometry — sky isle (multi-span, floating) ", () => {
  it("the upper span emits a top AND a bottom (its underside is exposed air)", () => {
    // span[0] ground (surface 80), span[1] floating isle [200,210].
    const tile = tileFrom(1, [[[-64, 80], [200, 210]]]);
    const geo = spansToVoxelGeometry(tile, PALETTE);
    // ground span: top(1)+4 sides = 5 ; isle span: top(1)+bottom(1)+4 sides = 6.
    expect(quadCount(geo)).toBe(11);
    const ys = Array.from(geo.positions).filter((_, i) => i % 3 === 1);
    // Isle top = 211, isle bottom = 200 both present.
    expect(ys).toContain(211);
    expect(ys).toContain(200);
    // Surface query still returns the ground ceiling, not the isle.
    expect(surfaceY(tile.columns[0])).toBe(80);
  });
});

describe("spansToVoxelGeometry — cave (lower span carved under air)", () => {
  it("a column with a roof span + floor span exposes the cave's inner faces", () => {
    // span[0] = walkable surface/roof region [40, 80] (surface=80),
    // span[1] = a cave floor remnant below [-64, 20] with air between.
    const tile = tileFrom(1, [[[40, 80], [-64, 20]]]);
    const geo = spansToVoxelGeometry(tile, PALETTE);
    // span[0] roof: top(1) + 4 sides = 5 (it's span[0] so no bottom emitted...
    //   the carved ceiling underside is the cave roof — modeled as the lower
    //   span's top, see below).
    // span[1] floor: top(1) + bottom(1) + 4 sides = 6.
    expect(quadCount(geo)).toBe(11);
    const ys = Array.from(geo.positions).filter((_, i) => i % 3 === 1);
    // Cave floor top = 21 (the surface NPCs would stand on inside the cave).
    expect(ys).toContain(21);
    // Roof top = 81.
    expect(ys).toContain(81);
    // surface (walkable outer top) is span[0].ceiling = 80.
    expect(surfaceY(tile.columns[0])).toBe(80);
  });
});

describe("spansToVoxelGeometry — face culling between neighbors", () => {
  it("a flat 2x2 plateau culls all interior side faces", () => {
    const tile = tileFrom(2, fillColumns(2, [[-64, 64]]));
    const geo = spansToVoxelGeometry(tile, PALETTE);
    // 4 columns: each top(1) = 4 tops. Sides: only the outer perimeter is
    // exposed. A 2x2 block has 4 columns * 2 outer edges each = 8 side faces
    // (each column sits at a corner, so 2 of its 4 sides face outside).
    // Bottoms: ground span -> none. Total = 4 tops + 8 sides = 12.
    expect(quadCount(geo)).toBe(12);
  });

  it("a step (one column taller) exposes the step riser between neighbors", () => {
    // (x0,z0) tall [−64,70], the other three short [−64,60].
    const cols: SpanTuple[][] = [
      [[-64, 70]],
      [[-64, 60]],
      [[-64, 60]],
      [[-64, 60]],
    ];
    const tile = tileFrom(2, cols);
    const geo = spansToVoxelGeometry(tile, PALETTE);
    // The tall column's +X and +Z sides now face SHORTER neighbors (gap above
    // their ceiling), so those risers ARE emitted — more faces than the flat
    // plateau (12).
    expect(quadCount(geo)).toBeGreaterThan(12);
  });
});

describe("spansToVoxelGeometry — LOD stride downsamples", () => {
  it("stride=2 on a 4x4 tile samples every other column", () => {
    const full = tileFrom(4, fillColumns(4, [[-64, 64]]));
    const dense = spansToVoxelGeometry(full, PALETTE, { stride: 1 });
    const lod = spansToVoxelGeometry(full, PALETTE, { stride: 2 });
    // Strided mesh must be strictly cheaper but still non-empty.
    expect(lod.faceCount).toBeGreaterThan(0);
    expect(lod.faceCount).toBeLessThan(dense.faceCount);
  });
});
