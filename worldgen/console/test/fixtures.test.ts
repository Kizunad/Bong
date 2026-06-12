// Dedicated pins for the byte-level fixture builders (test/fixtures.ts). These
// mirror the Python span encoder; if they drift, every decode test that feeds
// off them silently tests the wrong bytes. So lock the encoder's own layout and
// guard branches directly, not just transitively through the decoder.

import { describe, it, expect } from "vitest";
import { MAX_SPANS, SPAN_BYTES_PER_COLUMN, SPAN_SENTINEL } from "../src/decode";
import { encodeColumns, fillColumns, type SpanTuple } from "./fixtures";

describe("encodeColumns — on-disk byte layout (mirrors the Python encoder)", () => {
  it("writes count + a single (floor,ceiling) i16-LE pair, sentinel-padding the rest", () => {
    const { spansCount, spans } = encodeColumns(1, [[[-64, 71]]]);
    expect(spansCount.length).toBe(1);
    expect(spansCount[0]).toBe(1);
    expect(spans.byteLength).toBe(SPAN_BYTES_PER_COLUMN); // 1 col * 16 B

    const view = new DataView(spans);
    // slot 0 = the real span, little-endian.
    expect(view.getInt16(0, true)).toBe(-64);
    expect(view.getInt16(2, true)).toBe(71);
    // slots 1..MAX_SPANS-1 = sentinel padding (both floor and ceiling).
    for (let slot = 1; slot < MAX_SPANS; slot++) {
      expect(view.getInt16(slot * 4, true)).toBe(SPAN_SENTINEL);
      expect(view.getInt16(slot * 4 + 2, true)).toBe(SPAN_SENTINEL);
    }
  });

  it("encodes little-endian (a negative floor is not byte-swapped)", () => {
    const { spans } = encodeColumns(1, [[[-64, -1]]]);
    const bytes = new Uint8Array(spans);
    // -64 LE = 0xC0 0xFF (big-endian would be 0xFF 0xC0).
    expect(bytes[0]).toBe(0xc0);
    expect(bytes[1]).toBe(0xff);
  });

  it("packs a full MAX_SPANS column with no sentinel padding", () => {
    const four: SpanTuple[] = [
      [-64, 10],
      [20, 30],
      [40, 50],
      [60, 70],
    ];
    const { spansCount, spans } = encodeColumns(1, [four]);
    expect(spansCount[0]).toBe(MAX_SPANS);
    const view = new DataView(spans);
    for (let slot = 0; slot < MAX_SPANS; slot++) {
      expect(view.getInt16(slot * 4, true)).toBe(four[slot][0]);
      expect(view.getInt16(slot * 4 + 2, true)).toBe(four[slot][1]);
    }
  });

  it("encodes a void column as count=0 with an all-sentinel slot block", () => {
    const { spansCount, spans } = encodeColumns(1, [[]]);
    expect(spansCount[0]).toBe(0);
    const view = new DataView(spans);
    for (let slot = 0; slot < MAX_SPANS; slot++) {
      expect(view.getInt16(slot * 4, true)).toBe(SPAN_SENTINEL);
      expect(view.getInt16(slot * 4 + 2, true)).toBe(SPAN_SENTINEL);
    }
  });

  it("lays columns out row-major (col_idx = z*tileSize + x)", () => {
    // 2×2: distinct ceilings so we can read back the index order.
    const cols: SpanTuple[][] = [
      [[-64, 60]], // (x0,z0) -> idx 0
      [[-64, 61]], // (x1,z0) -> idx 1
      [[-64, 62]], // (x0,z1) -> idx 2
      [[-64, 63]], // (x1,z1) -> idx 3
    ];
    const { spans } = encodeColumns(2, cols);
    const view = new DataView(spans);
    for (let i = 0; i < 4; i++) {
      expect(view.getInt16(i * SPAN_BYTES_PER_COLUMN + 2, true)).toBe(60 + i);
    }
  });

  it("throws when the column count != tileSize²", () => {
    expect(() => encodeColumns(2, [[[-64, 60]]])).toThrow(/expected 4 columns, got 1/);
  });

  it("throws when a column carries more than MAX_SPANS spans", () => {
    const tooMany: SpanTuple[] = [
      [-64, 1],
      [2, 3],
      [4, 5],
      [6, 7],
      [8, 9],
    ];
    expect(() => encodeColumns(1, [tooMany])).toThrow(/exceeds MAX_SPANS/);
  });
});

describe("fillColumns — uniform column grid", () => {
  it("produces tileSize² independent copies of the span list", () => {
    const grid = fillColumns(2, [[-64, 64]]);
    expect(grid.length).toBe(4); // 2×2
    for (const col of grid) {
      expect(col).toEqual([[-64, 64]]);
    }
    // Copies are independent: mutating one column must not touch the others
    // (each row gets a fresh nested array, not a shared reference).
    grid[0][0][1] = 999;
    expect(grid[1][0][1]).toBe(64);
  });

  it("fills with multi-span lists too", () => {
    const grid = fillColumns(1, [
      [-64, 40],
      [100, 110],
    ]);
    expect(grid).toEqual([
      [
        [-64, 40],
        [100, 110],
      ],
    ]);
  });
});
