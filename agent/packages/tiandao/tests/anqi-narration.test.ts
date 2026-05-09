import { describe, expect, it, vi } from "vitest";
import { CHANNELS } from "@bong/schema";

import { AnqiNarrationRuntime, type AnqiNarrationRuntimeClient } from "../src/anqi-narration.js";
import type { LlmClient } from "../src/llm.js";

const {
  AGENT_NARRATE,
  ANQI_CARRIER_IMPACT,
  ANQI_PROJECTILE_DESPAWNED,
  ANQI_MULTI_SHOT,
  ANQI_QI_INJECTION,
  ANQI_ECHO_FRACTAL,
  ANQI_CARRIER_ABRASION,
  ANQI_CONTAINER_SWAP,
} = CHANNELS;

class FakePubSub implements AnqiNarrationRuntimeClient {
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

function makeLlm(content: string): LlmClient {
  return {
    async chat(model: string) {
      return { content, durationMs: 0, requestId: null, model };
    },
  };
}

const silent = { info: vi.fn(), warn: vi.fn() };

describe("AnqiNarrationRuntime", () => {
  it("subscribes to both anqi event channels", async () => {
    const sub = new FakePubSub();
    const runtime = new AnqiNarrationRuntime({
      llm: makeLlm(""),
      model: "mock",
      sub,
      pub: new FakePubSub(),
      logger: silent,
      systemPrompt: "test",
    });

    await runtime.connect();

    expect(sub.subscribedChannels).toEqual([
      ANQI_CARRIER_IMPACT,
      ANQI_PROJECTILE_DESPAWNED,
      ANQI_MULTI_SHOT,
      ANQI_QI_INJECTION,
      ANQI_ECHO_FRACTAL,
      ANQI_CARRIER_ABRASION,
      ANQI_CONTAINER_SWAP,
    ]);
  });

  it("publishes LLM narration for a carrier impact", async () => {
    const pub = new FakePubSub();
    const runtime = new AnqiNarrationRuntime({
      llm: makeLlm(JSON.stringify({ text: "骨刺碎在远处，异色真元一线入脉。", style: "narration" })),
      model: "mock",
      sub: new FakePubSub(),
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await runtime.handlePayload(
      ANQI_CARRIER_IMPACT,
      JSON.stringify({
        attacker: "entity:archer",
        target: "entity:target",
        carrier_kind: "yibian_shougu",
        hit_distance: 37,
        sealed_qi_initial: 30,
        hit_qi: 19,
        wound_damage: 9.5,
        contam_amount: 9.5,
        tick: 120,
      }),
    );

    expect(pub.published).toHaveLength(1);
    expect(pub.published[0].channel).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0]).toEqual({
      scope: "broadcast",
      target: "anqi:impact|attacker:entity:archer|target:entity:target|tick:120",
      text: "骨刺碎在远处，异色真元一线入脉。",
      style: "narration",
    });
  });

  it("falls back for projectile despawn when LLM fails", async () => {
    const pub = new FakePubSub();
    const runtime = new AnqiNarrationRuntime({
      llm: {
        async chat() {
          throw new Error("boom");
        },
      },
      model: "mock",
      sub: new FakePubSub(),
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await runtime.handlePayload(
      ANQI_PROJECTILE_DESPAWNED,
      JSON.stringify({
        owner: "entity:archer",
        projectile: "entity:bone",
        reason: "out_of_range",
        distance: 83,
        qi_evaporated: 11,
        residual_qi: 4,
        pos: [12, 64, -9],
        tick: 180,
      }),
    );

    expect(pub.published).toHaveLength(1);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0].target).toBe("anqi:miss|projectile:entity:bone|tick:180");
    expect(String(envelope.narrations[0].text)).toContain("射空");
    expect(runtime.stats.llmFailures).toBe(1);
    expect(runtime.stats.fallbackUsed).toBe(1);
  });

  it("falls back for anqi-v2 echo fractal events", async () => {
    const pub = new FakePubSub();
    const runtime = new AnqiNarrationRuntime({
      llm: {
        async chat() {
          throw new Error("offline");
        },
      },
      model: "mock",
      sub: new FakePubSub(),
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await runtime.handlePayload(
      ANQI_ECHO_FRACTAL,
      JSON.stringify({
        caster: "entity:void",
        carrier_kind: "shanggu_bone",
        local_qi_density: 9,
        threshold: 0.3,
        echo_count: 30,
        damage_per_echo: 2,
        tick: 240,
      }),
    );

    expect(pub.published).toHaveLength(1);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0].target).toBe("anqi:echo|caster:entity:void|tick:240");
    expect(String(envelope.narrations[0].text)).toContain("30 支 echo");
  });

  it("falls back with a valid narration contract for container abrasion", async () => {
    const pub = new FakePubSub();
    const runtime = new AnqiNarrationRuntime({
      llm: {
        async chat() {
          throw new Error("offline");
        },
      },
      model: "mock",
      sub: new FakePubSub(),
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await runtime.handlePayload(
      ANQI_CARRIER_ABRASION,
      JSON.stringify({
        carrier: "entity:needle",
        container: "quiver",
        direction: "store",
        lost_qi: 2.5,
        after_qi: 47.5,
        tick: 260,
      }),
    );

    expect(pub.published).toHaveLength(1);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0]).toMatchObject({
      scope: "broadcast",
      target: "anqi:abrasion|carrier:entity:needle|tick:260",
      style: "system_warning",
    });
  });

  it("rejects malformed anqi-v2 payloads", async () => {
    const pub = new FakePubSub();
    const runtime = new AnqiNarrationRuntime({
      llm: makeLlm(""),
      model: "mock",
      sub: new FakePubSub(),
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await runtime.handlePayload(
      ANQI_QI_INJECTION,
      JSON.stringify({
        caster: "entity:a",
        skill: "unknown",
      }),
    );

    expect(pub.published).toHaveLength(0);
    expect(runtime.stats.rejectedContract).toBe(1);
  });
});
