import { describe, expect, it, vi } from "vitest";
import { CHANNELS } from "@bong/schema";

import {
  ZhenfaV2NarrationRuntime,
  type ZhenfaV2NarrationRuntimeClient,
} from "../src/zhenfa-v2-runtime.js";

const { AGENT_NARRATE } = CHANNELS;

class FakePubSub implements ZhenfaV2NarrationRuntimeClient {
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

describe("ZhenfaV2NarrationRuntime", () => {
  it("publishes warning narration for exposed deceive-heaven arrays", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new ZhenfaV2NarrationRuntime({ sub, pub, logger: silent });

    await runtime.handlePayload(
      JSON.stringify({
        v: 1,
        event: "deceive_heaven_exposed",
        array_id: 8,
        kind: "deceive_heaven",
        owner: "offline:Azure",
        x: 1,
        y: 64,
        z: -2,
        tick: 200,
        reveal_chance_per_tick: 0.002,
      }),
    );

    expect(pub.published).toHaveLength(1);
    expect(pub.published[0].channel).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0]).toMatchObject({
      scope: "broadcast",
      target: "zhenfa:deceive_heaven_exposed|deceive_heaven|id:8|tick:200",
      style: "system_warning",
    });
    expect(envelope.narrations[0].text).toContain("假账");
    expect(runtime.stats.published).toBe(1);
  });

  it("rejects malformed events without publishing", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new ZhenfaV2NarrationRuntime({ sub, pub, logger: silent });

    await runtime.handlePayload(JSON.stringify({ event: "deploy" }));

    expect(pub.published).toHaveLength(0);
    expect(runtime.stats.rejectedContract).toBe(1);
  });

  it("broadcasts non-exposure events when no zone metadata exists", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new ZhenfaV2NarrationRuntime({ sub, pub, logger: silent });

    await runtime.handlePayload(
      JSON.stringify({
        v: 1,
        event: "deploy",
        array_id: 9,
        kind: "lingju",
        owner: "offline:Azure",
        x: 1,
        y: 64,
        z: -2,
        tick: 201,
        density_multiplier: 1.5,
      }),
    );

    expect(pub.published).toHaveLength(1);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0]).toMatchObject({
      scope: "broadcast",
      target: "zhenfa:deploy|lingju|id:9|tick:201",
      style: "narration",
    });
  });

  it("routes events with explicit zone metadata to that zone", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new ZhenfaV2NarrationRuntime({ sub, pub, logger: silent });

    await runtime.handlePayload(
      JSON.stringify({
        v: 1,
        event: "deploy",
        array_id: 10,
        kind: "shrine_ward",
        owner: "offline:Azure",
        zone: "blood_valley",
        x: 1,
        y: 64,
        z: -2,
        tick: 202,
        radius: 5,
      }),
    );

    expect(pub.published).toHaveLength(1);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0]).toMatchObject({
      scope: "zone",
      target: "blood_valley",
      style: "narration",
    });
  });

  it("names the breaker on breakthrough events", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new ZhenfaV2NarrationRuntime({ sub, pub, logger: silent });

    await runtime.handlePayload(
      JSON.stringify({
        v: 1,
        event: "breakthrough",
        array_id: 11,
        kind: "illusion",
        owner: "offline:Azure",
        breaker: "offline:Breaker",
        x: 1,
        y: 64,
        z: -2,
        tick: 203,
        force_break: true,
      }),
    );

    expect(pub.published).toHaveLength(1);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0].text).toContain("Breaker 硬破了Azure");
  });
});
