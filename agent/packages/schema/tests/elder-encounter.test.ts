import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import { validateElderEncounterEventV1Contract } from "../src/elder-encounter.js";

const samplesDir = join(dirname(fileURLToPath(import.meta.url)), "..", "samples");

function loadSample(name: string): unknown {
  return JSON.parse(readFileSync(join(samplesDir, name), "utf8"));
}

describe("ElderEncounterEventV1 sample pins", () => {
  it("accepts all five event kinds and pins required fields", () => {
    const samples = loadSample("elder-encounter-event.sample.json");
    expect(Array.isArray(samples)).toBe(true);
    expect(samples).toHaveLength(5);

    const eventKinds = new Set<string>();
    for (const [index, sample] of (samples as unknown[]).entries()) {
      const result = validateElderEncounterEventV1Contract(sample);
      expect(
        result.ok,
        `sample[${index}] should be accepted: ${result.errors.join("; ")}`,
      ).toBe(true);
      const event = sample as Record<string, unknown>;
      eventKinds.add(String(event.event_kind));
      for (const field of [
        "zone_name",
        "elder_entity_id",
        "event_kind",
        "betray_probability",
        "dan_count",
        "offered_skill_id",
        "qi_fraction",
        "server_tick",
      ]) {
        expect(event, `sample[${index}] should include ${field}`).toHaveProperty(field);
      }
    }

    expect([...eventKinds].sort()).toEqual([
      "appeared",
      "betrayal",
      "dan_received",
      "dead_natural",
      "dead_player_kill",
    ]);
  });

  it("allows event_id to be omitted but rejects an empty optional value", () => {
    const samples = loadSample("elder-encounter-event.sample.json") as Record<string, unknown>[];
    expect(samples[0]).not.toHaveProperty("event_id");
    expect(samples[2].event_id).toBe("elder:betrayal:7:720002");

    const invalid = loadSample(
      "elder-encounter-event.invalid-empty-event-id.sample.json",
    );
    expect(validateElderEncounterEventV1Contract(invalid).ok).toBe(false);
  });

  it("rejects missing required fields and invalid discriminants", () => {
    const missing = loadSample(
      "elder-encounter-event.invalid-missing-field.sample.json",
    );
    expect(validateElderEncounterEventV1Contract(missing).ok).toBe(false);

    const unknownKind = loadSample(
      "elder-encounter-event.invalid-unknown-kind.sample.json",
    );
    expect(validateElderEncounterEventV1Contract(unknownKind).ok).toBe(false);
  });

  it("rejects unknown fields", () => {
    const extra = loadSample(
      "elder-encounter-event.invalid-extra-field.sample.json",
    );
    expect(validateElderEncounterEventV1Contract(extra).ok).toBe(false);
  });
});
