import { describe, expect, it } from "vitest";

import { validateZonePressureCrossedV1Contract } from "../src/zone-pressure.js";

describe("ZonePressureCrossedV1", () => {
  it("accepts a valid pressure crossing event", () => {
    const result = validateZonePressureCrossedV1Contract({
      v: 1,
      kind: "zone_pressure_crossed",
      zone: "spawn",
      level: "high",
      raw_pressure: 1.25,
      at_tick: 42,
    });

    expect(result.ok).toBe(true);
  });

  it("rejects unsupported pressure level", () => {
    const result = validateZonePressureCrossedV1Contract({
      v: 1,
      kind: "zone_pressure_crossed",
      zone: "spawn",
      level: "none",
      raw_pressure: 0,
      at_tick: 42,
    });

    expect(result.ok).toBe(false);
  });
});
