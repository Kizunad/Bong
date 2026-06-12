// Headless pins for the manifest spans_encoding contract guard (api.ts). The
// decoder reads spans.bin at a compiled-in stride / MAX_SPANS / sentinel; if a
// future bake's manifest declares a different layout the whole tile decodes to
// garbage. assertSpansEncoding must catch that mismatch at load time.

import { describe, it, expect } from "vitest";
import { assertSpansEncoding } from "../src/api";
import { MAX_SPANS, SPAN_BYTES_PER_COLUMN, SPAN_SENTINEL } from "../src/decode";
import type { Manifest, SpansEncoding } from "../src/types";

function manifestWith(encoding: Partial<SpansEncoding> | null): Manifest {
  const enc =
    encoding === null
      ? undefined
      : {
          max_spans: MAX_SPANS,
          bytes_per_column: SPAN_BYTES_PER_COLUMN,
          sentinel: SPAN_SENTINEL,
          count_file: "spans_count.bin",
          spans_file: "spans.bin",
          slot_layout: "i16_le(floor,ceiling) × max_spans",
          ...encoding,
        };
  // Only the fields assertSpansEncoding touches matter; cast the rest.
  return { spans_encoding: enc } as unknown as Manifest;
}

describe("assertSpansEncoding — manifest must match the compiled decoder", () => {
  it("accepts an encoding identical to the decoder constants", () => {
    expect(() => assertSpansEncoding(manifestWith({}))).not.toThrow();
  });

  it("throws when spans_encoding is missing entirely", () => {
    expect(() => assertSpansEncoding(manifestWith(null))).toThrow(
      /missing spans_encoding/,
    );
  });

  it("throws on a max_spans mismatch (decoder would mis-stride every column)", () => {
    expect(() =>
      assertSpansEncoding(manifestWith({ max_spans: MAX_SPANS + 1 })),
    ).toThrow(/max_spans/);
  });

  it("throws on a bytes_per_column (stride) mismatch", () => {
    expect(() =>
      assertSpansEncoding(manifestWith({ bytes_per_column: SPAN_BYTES_PER_COLUMN * 2 })),
    ).toThrow(/bytes_per_column/);
  });

  it("throws on a sentinel mismatch (unused-slot marker disagreement)", () => {
    expect(() =>
      assertSpansEncoding(manifestWith({ sentinel: SPAN_SENTINEL - 1 })),
    ).toThrow(/sentinel/);
  });

  it("reports every mismatched field at once, not just the first", () => {
    let msg = "";
    try {
      assertSpansEncoding(
        manifestWith({ max_spans: 99, bytes_per_column: 99, sentinel: 99 }),
      );
    } catch (e) {
      msg = (e as Error).message;
    }
    expect(msg).toMatch(/max_spans/);
    expect(msg).toMatch(/bytes_per_column/);
    expect(msg).toMatch(/sentinel/);
  });
});
