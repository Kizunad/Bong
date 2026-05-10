import { describe, expect, it, vi } from "vitest";
import { CHANNELS, type PoisonSideEffectTagV1 } from "@bong/schema";

import {
  PoisonTraitNarrationRuntime,
  poisonSideEffectText,
  type PoisonTraitRuntimeClient,
} from "../src/poison-trait-runtime.js";

const { AGENT_NARRATE, POISON_DOSE_EVENT, POISON_OVERDOSE_EVENT } = CHANNELS;

class FakePubSub implements PoisonTraitRuntimeClient {
  public published: Array<{ channel: string; message: string }> = [];
  public subscribedChannels: string[] = [];
  private readonly listenersByEvent = new Map<string, Array<(channel: string, message: string) => void>>();

  async subscribe(channel: string): Promise<void> {
    this.subscribedChannels.push(channel);
  }

  on(event: string, listener: (channel: string, message: string) => void) {
    const listeners = this.listenersByEvent.get(event) ?? [];
    listeners.push(listener);
    this.listenersByEvent.set(event, listeners);
    return this;
  }

  off(event: string, listener: (channel: string, message: string) => void) {
    const listeners = this.listenersByEvent.get(event) ?? [];
    this.listenersByEvent.set(event, listeners.filter((entry) => entry !== listener));
    return this;
  }

  async unsubscribe(): Promise<void> {}

  disconnect(): void {}

  async publish(channel: string, message: string): Promise<number> {
    this.published.push({ channel, message });
    return 1;
  }

  emit(event: string, channel: string, message: string): void {
    for (const listener of this.listenersByEvent.get(event) ?? []) {
      listener(channel, message);
    }
  }
}

const silent = { info: vi.fn(), warn: vi.fn() };

describe("PoisonTraitNarrationRuntime", () => {
  it("subscribes to poison dose and overdose Redis channels", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new PoisonTraitNarrationRuntime({ sub, pub, logger: silent });

    await runtime.connect();

    expect(sub.subscribedChannels).toEqual(
      expect.arrayContaining([POISON_DOSE_EVENT, POISON_OVERDOSE_EVENT]),
    );
    expect(sub.subscribedChannels).toHaveLength(2);
  });

  it("wires Redis message events to poison narration handling", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new PoisonTraitNarrationRuntime({ sub, pub, logger: silent });

    await runtime.connect();
    sub.emit(
      "ignored",
      POISON_DOSE_EVENT,
      JSON.stringify({
        v: 1,
        player_entity_id: 7,
        dose_amount: 5,
        side_effect_tag: "qi_focus_drift_2h",
        poison_level_after: 17,
        digestion_after: 50,
        at_tick: 100,
      }),
    );
    expect(pub.published).toHaveLength(0);

    sub.emit(
      "message",
      POISON_DOSE_EVENT,
      JSON.stringify({
        v: 1,
        player_entity_id: 7,
        dose_amount: 5,
        side_effect_tag: "qi_focus_drift_2h",
        poison_level_after: 17,
        digestion_after: 50,
        at_tick: 100,
      }),
    );

    await vi.waitFor(() => expect(pub.published).toHaveLength(1));
    expect(pub.published[0].channel).toBe(AGENT_NARRATE);
  });

  it("publishes dose narration to agent narration channel", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new PoisonTraitNarrationRuntime({ sub, pub, logger: silent });

    await runtime.handlePayload(
      POISON_DOSE_EVENT,
      JSON.stringify({
        v: 1,
        player_entity_id: 7,
        dose_amount: 5,
        side_effect_tag: "qi_focus_drift_2h",
        poison_level_after: 17,
        digestion_after: 50,
        at_tick: 100,
      }),
    );

    expect(pub.published).toHaveLength(1);
    expect(pub.published[0].channel).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0]).toMatchObject({
      scope: "player",
      target: "poison_dose:7|tick:100",
      style: "narration",
    });
    expect(envelope.narrations[0].text).toContain("毒性真元升至 17");
  });

  it("publishes overdose narration with lifespan cost", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new PoisonTraitNarrationRuntime({ sub, pub, logger: silent });

    await runtime.handlePayload(
      POISON_OVERDOSE_EVENT,
      JSON.stringify({
        v: 1,
        player_entity_id: 7,
        severity: "moderate",
        overflow: 30,
        lifespan_penalty_years: 1,
        micro_tear_probability: 0.1,
        at_tick: 120,
      }),
    );

    expect(pub.published).toHaveLength(1);
    expect(pub.published[0].channel).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0].target).toBe("poison_overdose:7|tick:120");
    expect(envelope.narrations[0].text).toContain("寿元折去 1.0 年");
  });

  it("rejects malformed payloads without publishing", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new PoisonTraitNarrationRuntime({ sub, pub, logger: silent });

    await runtime.handlePayload(
      POISON_DOSE_EVENT,
      JSON.stringify({ v: 1, player_entity_id: Number.MAX_SAFE_INTEGER + 1 }),
    );

    expect(pub.published).toHaveLength(0);
    expect(runtime.stats.rejectedContract).toBe(1);
  });

  it("keeps a fallback side effect line for unknown tags", () => {
    expect(poisonSideEffectText("unknown" as PoisonSideEffectTagV1)).toContain("丹毒");
  });
});
