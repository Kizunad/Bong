import type { WeatherEventUpdateV1 } from "@bong/schema";
import { describe, expect, it } from "vitest";
import { renderWeatherNarration } from "../src/runtime.js";

function weather(overrides: Partial<WeatherEventUpdateV1> = {}): WeatherEventUpdateV1 {
  return {
    v: 1,
    kind: "started",
    data: {
      v: 1,
      zone_id: "blood_valley_east_scorch",
      kind: "thunderstorm",
      started_at_lingtian_tick: 1440,
      expires_at_lingtian_tick: 1620,
      remaining_ticks: 180,
    },
    ...overrides,
  };
}

describe("zone weather narration", () => {
  it("renders scorch thunderstorm as zone scoped narration", () => {
    expect(renderWeatherNarration(weather())).toEqual(
      expect.objectContaining({
        scope: "zone",
        target: "zone:blood_valley_east_scorch",
        style: "narration",
        text: expect.stringContaining("焦土"),
      }),
    );
  });

  it("ignores expired events to avoid duplicate narration", () => {
    expect(renderWeatherNarration(weather({ kind: "expired" }))).toBeNull();
  });
});
