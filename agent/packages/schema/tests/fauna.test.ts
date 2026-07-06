import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import { CHANNELS, REDIS_V1_CHANNELS } from "../src/channels.js";
import { validateFaunaEcologySnapshotV1Contract } from "../src/fauna.js";

const __dirname = dirname(fileURLToPath(import.meta.url));
const samplesDir = join(__dirname, "..", "samples");

function loadSample(name: string): unknown {
  return JSON.parse(readFileSync(join(samplesDir, name), "utf-8"));
}

describe("FaunaEcologySnapshotV1 contract", () => {
  it("accepts the committed sample", () => {
    const result = validateFaunaEcologySnapshotV1Contract(
      loadSample("fauna-ecology-snapshot.sample.json"),
    );
    expect(result.ok).toBe(true);
    expect(result.errors).toEqual([]);
  });

  it("accepts an empty-zones snapshot (boundary)", () => {
    expect(
      validateFaunaEcologySnapshotV1Contract({ v: 1, tick: 0, zones: [] }).ok,
    ).toBe(true);
  });

  it("accepts a zone with empty species_counts (all fauna gone)", () => {
    expect(
      validateFaunaEcologySnapshotV1Contract({
        v: 1,
        tick: 10,
        zones: [{ zone: "blood_valley", spirit_qi: -0.2, species_counts: [] }],
      }).ok,
    ).toBe(true);
  });

  it("rejects a wrong version literal", () => {
    expect(
      validateFaunaEcologySnapshotV1Contract({ v: 2, tick: 0, zones: [] }).ok,
    ).toBe(false);
  });

  it("rejects spirit_qi outside [-1, 1]", () => {
    expect(
      validateFaunaEcologySnapshotV1Contract({
        v: 1,
        tick: 5,
        zones: [{ zone: "spawn", spirit_qi: 1.5, species_counts: [] }],
      }).ok,
    ).toBe(false);
  });

  it("rejects a negative species count", () => {
    expect(
      validateFaunaEcologySnapshotV1Contract({
        v: 1,
        tick: 5,
        zones: [
          { zone: "spawn", spirit_qi: 0.3, species_counts: [{ kind: "rabbit", count: -1 }] },
        ],
      }).ok,
    ).toBe(false);
  });

  it("rejects an empty species kind", () => {
    expect(
      validateFaunaEcologySnapshotV1Contract({
        v: 1,
        tick: 5,
        zones: [{ zone: "spawn", spirit_qi: 0.3, species_counts: [{ kind: "", count: 1 }] }],
      }).ok,
    ).toBe(false);
  });

  it("rejects unknown top-level fields (additionalProperties false)", () => {
    expect(
      validateFaunaEcologySnapshotV1Contract({ v: 1, tick: 0, zones: [], extra: true }).ok,
    ).toBe(false);
  });

  it("registers the fauna ecology channel", () => {
    expect(CHANNELS.FAUNA_ECOLOGY).toBe("bong:fauna/ecology");
    expect(REDIS_V1_CHANNELS).toContain(CHANNELS.FAUNA_ECOLOGY);
  });
});
