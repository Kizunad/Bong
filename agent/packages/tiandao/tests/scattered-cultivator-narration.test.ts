import { describe, expect, it, vi } from "vitest";
import { CHANNELS } from "@bong/schema";

import {
  ScatteredCultivatorNarrationRuntime,
  renderNpcIntrusionNarration,
  renderPressureNarration,
  type ScatteredCultivatorNarrationRuntimeClient,
} from "../src/scattered-cultivator-narration.js";

const { AGENT_NARRATE, SOCIAL_NICHE_INTRUSION, ZONE_PRESSURE_CROSSED } = CHANNELS;

class FakePubSub implements ScatteredCultivatorNarrationRuntimeClient {
  public published: Array<{ channel: string; message: string }> = [];
  public subscribedChannels: string[] = [];
  public listeners: Array<(channel: string, message: string) => void> = [];

  async subscribe(channel: string): Promise<void> {
    this.subscribedChannels.push(channel);
  }

  on(_event: string, listener: (channel: string, message: string) => void) {
    this.listeners.push(listener);
    return this;
  }

  off(_event: string, listener: (channel: string, message: string) => void) {
    this.listeners = this.listeners.filter((entry) => entry !== listener);
    return this;
  }

  async unsubscribe(): Promise<void> {}

  disconnect(): void {}

  async publish(channel: string, message: string): Promise<number> {
    this.published.push({ channel, message });
    return 1;
  }
}

const silent = { info: vi.fn(), warn: vi.fn() };

describe("ScatteredCultivatorNarrationRuntime", () => {
  it("subscribes to pressure and niche intrusion channels", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new ScatteredCultivatorNarrationRuntime({ sub, pub, logger: silent });

    await runtime.connect();

    expect(sub.subscribedChannels).toEqual([ZONE_PRESSURE_CROSSED, SOCIAL_NICHE_INTRUSION]);
  });

  it("publishes zone-scoped narration on pressure crossing", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new ScatteredCultivatorNarrationRuntime({ sub, pub, logger: silent });

    await runtime.handlePayload(
      ZONE_PRESSURE_CROSSED,
      JSON.stringify({
        v: 1,
        kind: "zone_pressure_crossed",
        zone: "spawn",
        level: "high",
        raw_pressure: 1.2,
        at_tick: 80,
      }),
    );

    expect(pub.published).toHaveLength(1);
    expect(pub.published[0].channel).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0]).toEqual({
      scope: "zone",
      target: "spawn",
      text: "spawn 散修聚众，地脉已被榨到阈上；此地又一波将逝。",
      style: "narration",
      kind: "npc_farm_pressure",
    });
    expect(runtime.stats.received).toBe(1);
    expect(runtime.stats.published).toBe(1);
  });

  it("publishes NPC-only niche intrusion narration", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new ScatteredCultivatorNarrationRuntime({ sub, pub, logger: silent });

    await runtime.handlePayload(
      SOCIAL_NICHE_INTRUSION,
      JSON.stringify({
        v: 1,
        niche_pos: [1, 64, 2],
        intruder_id: "npc:rogue_1",
        items_taken: [7, 8],
        taint_delta: 0.1,
      }),
    );

    expect(pub.published).toHaveLength(1);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0].kind).toBe("niche_intrusion_by_npc");
    expect(envelope.narrations[0].target).toBe("niche:1,64,2|intruder:npc:rogue_1");
  });

  it("ignores non-NPC niche intruders", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new ScatteredCultivatorNarrationRuntime({ sub, pub, logger: silent });

    await runtime.handlePayload(
      SOCIAL_NICHE_INTRUSION,
      JSON.stringify({
        v: 1,
        niche_pos: [1, 64, 2],
        intruder_id: "char:player",
        items_taken: [],
        taint_delta: 0,
      }),
    );

    expect(pub.published).toHaveLength(0);
    expect(runtime.stats.ignored).toBe(1);
  });

  it("renders deterministic narration helpers", () => {
    expect(
      renderPressureNarration({
        v: 1,
        kind: "zone_pressure_crossed",
        zone: "valley",
        level: "mid",
        raw_pressure: 0.7,
        at_tick: 1,
      }).text,
    ).toContain("田埂人影相续");
    expect(
      renderNpcIntrusionNarration({
        v: 1,
        niche_pos: [3, 64, 4],
        intruder_id: "npc_2",
        items_taken: [1],
        taint_delta: 0.2,
      }).kind,
    ).toBe("niche_intrusion_by_npc");
  });
});
