// Byte-level fixture builders mirroring the Python encoder
// (worldgen/scripts/terrain_gen/fields.py encode_spans_arrays / to_slots).
// Tests feed these into the decoder/mesher to pin the data->geometry law.

import { MAX_SPANS, SPAN_BYTES_PER_COLUMN, SPAN_SENTINEL } from "../src/decode";

export type SpanTuple = [floorY: number, ceilingY: number];

/**
 * Encode a grid of columns (row-major: columns[z][x]) into the two on-disk
 * buffers exactly as the Python side does:
 *   spans_count: u8 per column
 *   spans:       MAX_SPANS slots/col, (i16 floor, i16 ceiling) LE, sentinel pad.
 */
export function encodeColumns(
  tileSize: number,
  columns: SpanTuple[][],
): { spansCount: Uint8Array; spans: ArrayBuffer } {
  const cols = tileSize * tileSize;
  if (columns.length !== cols) {
    throw new Error(`expected ${cols} columns, got ${columns.length}`);
  }
  const spansCount = new Uint8Array(cols);
  const spans = new ArrayBuffer(cols * SPAN_BYTES_PER_COLUMN);
  const view = new DataView(spans);
  for (let c = 0; c < cols; c++) {
    const colSpans = columns[c];
    if (colSpans.length > MAX_SPANS) {
      throw new Error(`column ${c}: ${colSpans.length} spans exceeds MAX_SPANS`);
    }
    spansCount[c] = colSpans.length;
    const base = c * SPAN_BYTES_PER_COLUMN;
    for (let slot = 0; slot < MAX_SPANS; slot++) {
      const off = base + slot * 4;
      if (slot < colSpans.length) {
        view.setInt16(off, colSpans[slot][0], true);
        view.setInt16(off + 2, colSpans[slot][1], true);
      } else {
        view.setInt16(off, SPAN_SENTINEL, true);
        view.setInt16(off + 2, SPAN_SENTINEL, true);
      }
    }
  }
  return { spansCount, spans };
}

/** Build a flat columns array filling every column with the same span list. */
export function fillColumns(tileSize: number, spans: SpanTuple[]): SpanTuple[][] {
  const cols = tileSize * tileSize;
  return Array.from({ length: cols }, () => spans.map((s) => [...s] as SpanTuple));
}
