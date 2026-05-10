import { describe, expect, it, vi } from "vitest";
import { CHANNELS } from "@bong/schema";

import {
  BaomaiV3NarrationRuntime,
  renderBaomaiV3Narration,
  type BaomaiV3RuntimeClient,
} from "../src/baomai-v3-runtime.js";

const { AGENT_NARRATE, BAOMAI_V3_SKILL_EVENT } = CHANNELS;

class FakePubSub implements BaomaiV3RuntimeClient {
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

describe("BaomaiV3NarrationRuntime", () => {
  it("subscribes to the baomai-v3 skill-event channel", async () => {
    const sub = new FakePubSub();
    const runtime = new BaomaiV3NarrationRuntime({
      sub,
      pub: new FakePubSub(),
      logger: silent,
    });

    await runtime.connect();

    expect(sub.subscribedChannels).toEqual([BAOMAI_V3_SKILL_EVENT]);
  });

  it("renders disperse as flow-rate overdrive rather than immunity", () => {
    const narration = renderBaomaiV3Narration({
      v: 1,
      type: "baomai_skill_event",
      skill_id: "disperse",
      caster_id: "offline:Azure",
      tick: 120,
      qi_invested: 5350,
      damage: 0,
      blood_multiplier: 1,
      flow_rate_multiplier: 10,
      meridian_ids: ["Ren", "Du"],
    });

    expect(narration?.text).toContain("脉流暴涨十倍");
    expect(narration?.text).toContain("没有一分免伤");
  });

  it("publishes narration for baomai skill events", async () => {
    const pub = new FakePubSub();
    const runtime = new BaomaiV3NarrationRuntime({
      sub: new FakePubSub(),
      pub,
      logger: silent,
    });

    await runtime.handlePayload(BAOMAI_V3_SKILL_EVENT, JSON.stringify({
      v: 1,
      type: "baomai_skill_event",
      skill_id: "blood_burn",
      caster_id: "offline:Azure",
      tick: 121,
      qi_invested: 0,
      damage: 0,
      blood_multiplier: 3,
      flow_rate_multiplier: 1,
      meridian_ids: ["Liver", "Ren", "Du"],
    }));

    expect(pub.published).toHaveLength(1);
    expect(pub.published[0].channel).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0].text).toContain("血雾");
    expect(runtime.stats.published).toBe(1);
  });

  it("rejects malformed payloads without publishing", async () => {
    const pub = new FakePubSub();
    const runtime = new BaomaiV3NarrationRuntime({
      sub: new FakePubSub(),
      pub,
      logger: silent,
    });

    await runtime.handlePayload(BAOMAI_V3_SKILL_EVENT, "{\"type\":\"bad\"}");

    expect(pub.published).toHaveLength(0);
    expect(runtime.stats.rejectedContract).toBe(1);
  });
});
