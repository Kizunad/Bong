import { describe, expect, it, vi } from "vitest";
import { CHANNELS } from "@bong/schema";

import {
  TribulationNarrationRuntime,
  type TribulationNarrationRuntimeClient,
} from "../src/tribulation-runtime.js";
import type { LlmClient } from "../src/llm.js";

const { AGENT_NARRATE } = CHANNELS;

class FakePubSub implements TribulationNarrationRuntimeClient {
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

const failingLlm: LlmClient = {
  async chat() {
    throw new Error("boom");
  },
};

const silent = { info: vi.fn(), warn: vi.fn(), error: vi.fn() };

describe("TribulationNarrationRuntime", () => {
  it("falls back to cold narration when DuXu ascends", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new TribulationNarrationRuntime({
      llm: failingLlm,
      model: "mock",
      sub,
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await runtime.handlePayload(
      JSON.stringify({
        v: 1,
        kind: "du_xu",
        phase: { kind: "settle" },
        char_id: "offline:Azure",
        actor_name: "Azure",
        result: {
          char_id: "offline:Azure",
          outcome: "ascended",
          waves_survived: 3,
        },
      }),
    );

    expect(pub.published).toHaveLength(1);
    expect(pub.published[0].channel).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0]).toEqual({
      scope: "broadcast",
      target: "tribulation:du_xu|char:offline:Azure|settle",
      text: "Azure 历尽 3 道劫雷，终入化虚，天地并不称贺。",
      style: "narration",
    });
    expect(runtime.stats.llmFailures).toBe(1);
    expect(runtime.stats.fallbackUsed).toBe(1);
  });

  it("uses the plan omen broadcast when DuXu begins", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new TribulationNarrationRuntime({
      llm: failingLlm,
      model: "mock",
      sub,
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await runtime.handlePayload(
      JSON.stringify({
        v: 1,
        kind: "du_xu",
        phase: { kind: "omen" },
        char_id: "offline:Azure",
        actor_name: "Azure",
        epicenter: [400, 66, -200],
      }),
    );

    expect(pub.published).toHaveLength(1);
    expect(pub.published[0].channel).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0]).toEqual({
      scope: "broadcast",
      target: "tribulation:du_xu|char:offline:Azure|omen",
      text: "北风忽起，雷云自聚。又有修士在逆天。",
      style: "narration",
    });
    expect(runtime.stats.llmFailures).toBe(1);
    expect(runtime.stats.fallbackUsed).toBe(1);
  });

  it("loads the worldview tone guide into the default tribulation prompt", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    let systemPrompt = "";
    const promptLlm: LlmClient = {
      async chat(_model, messages) {
        systemPrompt = String(messages[0]?.content ?? "");
        return JSON.stringify({
          text: "血谷灵脉又枯了三分。仍有蠢人在那里打坐。",
          style: "narration",
        });
      },
    };
    const runtime = new TribulationNarrationRuntime({
      llm: promptLlm,
      model: "mock",
      sub,
      pub,
      logger: silent,
    });

    await runtime.handlePayload(
      JSON.stringify({
        v: 1,
        kind: "du_xu",
        phase: { kind: "omen" },
        char_id: "offline:Azure",
      }),
    );

    expect(systemPrompt).toContain("天道不是帮你的，也不是害你的");
    expect(systemPrompt).toContain("不要把事件写成奖励或惩罚");
    expect(systemPrompt).toContain("不好的叙事");
    expect(pub.published).toHaveLength(1);
  });

  it("falls back to cold sarcasm when DuXu is intercepted and killed", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new TribulationNarrationRuntime({
      llm: failingLlm,
      model: "mock",
      sub,
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await runtime.handlePayload(
      JSON.stringify({
        v: 1,
        kind: "du_xu",
        phase: { kind: "settle" },
        char_id: "offline:Victim",
        actor_name: "Victim",
        result: {
          char_id: "offline:Victim",
          outcome: "killed",
          killer: "offline:Killer",
          waves_survived: 2,
        },
      }),
    );

    expect(pub.published).toHaveLength(1);
    expect(pub.published[0].channel).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.v).toBe(1);
    expect(envelope.narrations).toHaveLength(1);
    expect(envelope.narrations[0]).toEqual({
      scope: "broadcast",
      target: "tribulation:du_xu|char:offline:Victim|settle",
      text: "Victim 死于劫中截胡，杀者 offline:Killer 得其遗物；天雷不辨勇怯，只记损益。",
      style: "narration",
    });
    expect(runtime.stats.llmFailures).toBe(1);
    expect(runtime.stats.fallbackUsed).toBe(1);
  });

  it("falls back to evacuation warning when zone collapse is not settled", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new TribulationNarrationRuntime({
      llm: failingLlm,
      model: "mock",
      sub,
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await runtime.handlePayload(
      JSON.stringify({
        v: 1,
        kind: "zone_collapse",
        phase: { kind: "lock" },
        zone: "spawn",
      }),
    );

    expect(pub.published).toHaveLength(1);
    expect(pub.published[0].channel).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0]).toEqual({
      scope: "broadcast",
      target: "tribulation:zone_collapse|zone:spawn|lock",
      text: "spawn 灵气低伏，灰风先起，此地将崩，尚有片刻可退。",
      style: "narration",
    });
    expect(runtime.stats.llmFailures).toBe(1);
    expect(runtime.stats.fallbackUsed).toBe(1);
  });

  it("falls back to final lament when zone collapse settles", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new TribulationNarrationRuntime({
      llm: failingLlm,
      model: "mock",
      sub,
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await runtime.handlePayload(
      JSON.stringify({
        v: 1,
        kind: "zone_collapse",
        phase: { kind: "settle" },
        zone: "spawn",
      }),
    );

    expect(pub.published).toHaveLength(1);
    expect(pub.published[0].channel).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0]).toEqual({
      scope: "broadcast",
      target: "tribulation:zone_collapse|zone:spawn|settle",
      text: "spawn 灵机断绝，域崩已成，未退者皆归死寂。",
      style: "narration",
    });
    expect(runtime.stats.llmFailures).toBe(1);
    expect(runtime.stats.fallbackUsed).toBe(1);
  });

  it("broadcasts cold narration when an ascension quota slot opens", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new TribulationNarrationRuntime({
      llm: failingLlm,
      model: "mock",
      sub,
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await runtime.handlePayload(
      JSON.stringify({
        v: 1,
        kind: "ascension_quota_open",
        phase: { kind: "settle" },
        occupied_slots: 0,
      }),
    );

    expect(pub.published).toHaveLength(1);
    expect(pub.published[0].channel).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.v).toBe(1);
    expect(envelope.narrations).toHaveLength(1);
    expect(envelope.narrations[0]).toEqual({
      scope: "broadcast",
      target: "tribulation:ascension_quota_open|settle",
      text: "化虚有位，叩关者可往；天道只空出座次，不替任何人铺路。",
      style: "narration",
    });
    expect(runtime.stats.llmFailures).toBe(1);
    expect(runtime.stats.fallbackUsed).toBe(1);
  });

  it("falls back to zone-scoped hint for hidden targeted calamity", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new TribulationNarrationRuntime({
      llm: failingLlm,
      model: "mock",
      sub,
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await runtime.handlePayload(
      JSON.stringify({
        v: 1,
        kind: "targeted",
        phase: { kind: "omen" },
        zone: "spawn",
        epicenter: [8, 66, 8],
      }),
    );

    expect(pub.published).toHaveLength(1);
    expect(pub.published[0].channel).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.v).toBe(1);
    expect(envelope.narrations).toHaveLength(1);
    expect(envelope.narrations[0]).toEqual({
      scope: "zone",
      target: "spawn",
      text: "spawn 近日运道不佳，灵机一动便多一分折耗。",
      style: "narration",
    });
    expect(runtime.stats.llmFailures).toBe(1);
    expect(runtime.stats.fallbackUsed).toBe(1);
  });

  it("keeps LLM targeted calamity narration zone-scoped", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const llm: LlmClient = {
      async chat() {
        return JSON.stringify({
          text: "spawn 近来灰云低伏，丹火无端多灭了两次。",
          style: "narration",
        });
      },
    };
    const runtime = new TribulationNarrationRuntime({
      llm,
      model: "mock",
      sub,
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await runtime.handlePayload(
      JSON.stringify({
        v: 1,
        kind: "targeted",
        phase: { kind: "omen" },
        zone: "spawn",
        epicenter: [8, 66, 8],
      }),
    );

    expect(pub.published).toHaveLength(1);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0]).toEqual({
      scope: "zone",
      target: "spawn",
      text: "spawn 近来灰云低伏，丹火无端多灭了两次。",
      style: "narration",
    });
    expect(runtime.stats.llmFailures).toBe(0);
    expect(runtime.stats.fallbackUsed).toBe(0);
  });

  it("falls back when LLM targeted calamity narration leaks hidden mechanics", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const llm: LlmClient = {
      async chat() {
        return JSON.stringify({
          text: "spawn 的劫气权重升高，定向天罚概率已经改变。",
          style: "narration",
        });
      },
    };
    const runtime = new TribulationNarrationRuntime({
      llm,
      model: "mock",
      sub,
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await runtime.handlePayload(
      JSON.stringify({
        v: 1,
        kind: "targeted",
        phase: { kind: "omen" },
        zone: "spawn",
        epicenter: [8, 66, 8],
      }),
    );

    expect(pub.published).toHaveLength(1);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0]).toEqual({
      scope: "zone",
      target: "spawn",
      text: "spawn 近日运道不佳，灵机一动便多一分折耗。",
      style: "narration",
    });
    expect(runtime.stats.llmFailures).toBe(0);
    expect(runtime.stats.fallbackUsed).toBe(1);
  });
});
