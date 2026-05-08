import { describe, expect, it, vi } from "vitest";
import { CHANNELS } from "@bong/schema";

import {
  CraftNarrationRuntime,
  type CraftNarrationRuntimeClient,
} from "../src/craft-runtime.js";
import type { LlmClient } from "../src/llm.js";

const { AGENT_NARRATE, CRAFT_OUTCOME, CRAFT_RECIPE_UNLOCKED } = CHANNELS;

class FakePubSub implements CraftNarrationRuntimeClient {
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

const completedOutcome = {
  kind: "completed",
  v: 1,
  player_id: "offline:Alice",
  recipe_id: "craft.example.eclipse_needle.iron",
  output_template: "eclipse_needle_iron",
  output_count: 3,
  completed_at_tick: 5000,
  ts: 1234567,
} as const;

const scrollUnlock = {
  v: 1,
  player_id: "offline:Alice",
  recipe_id: "craft.example.eclipse_needle.iron",
  source: { kind: "scroll", item_template: "scroll_eclipse_needle_iron" },
  unlocked_at_tick: 4000,
  ts: 1234567,
} as const;

const mentorUnlock = {
  v: 1,
  player_id: "offline:Alice",
  recipe_id: "craft.example.poison_decoction.fan",
  source: { kind: "mentor", npc_archetype: "poison_master" },
  unlocked_at_tick: 4000,
  ts: 1234567,
} as const;

const insightUnlock = {
  v: 1,
  player_id: "offline:Alice",
  recipe_id: "craft.example.fake_skin.light",
  source: { kind: "insight", trigger: "near_death" },
  unlocked_at_tick: 4000,
  ts: 1234567,
} as const;

describe("CraftNarrationRuntime", () => {
  it("subscribes to both craft channels on connect", async () => {
    const sub = new FakePubSub();
    const runtime = new CraftNarrationRuntime({
      llm: makeLlm(""),
      model: "mock",
      sub,
      pub: new FakePubSub(),
      logger: silent,
      systemPrompt: "test",
    });
    await runtime.connect();
    expect(sub.subscribedChannels.sort()).toEqual([CRAFT_OUTCOME, CRAFT_RECIPE_UNLOCKED].sort());
  });

  it("publishes completed-category narration on craft_outcome", async () => {
    const pub = new FakePubSub();
    const runtime = new CraftNarrationRuntime({
      llm: makeLlm(
        JSON.stringify({
          text: "蚀针成型，针尾涔涔渗着冷意。",
          style: "narration",
        }),
      ),
      model: "mock",
      sub: new FakePubSub(),
      pub,
      logger: silent,
      systemPrompt: "test",
    });
    await runtime.handleOutcome(JSON.stringify(completedOutcome));
    expect(pub.published).toHaveLength(1);
    expect(pub.published[0].channel).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0].text).toBe("蚀针成型，针尾涔涔渗着冷意。");
    expect(runtime.stats.byCategory.completed).toBe(1);
  });

  it("ignores failed outcome (no narration emitted)", async () => {
    const pub = new FakePubSub();
    const runtime = new CraftNarrationRuntime({
      llm: makeLlm("any"),
      model: "mock",
      sub: new FakePubSub(),
      pub,
      logger: silent,
      systemPrompt: "test",
    });
    await runtime.handleOutcome(
      JSON.stringify({
        kind: "failed",
        v: 1,
        player_id: "offline:Alice",
        recipe_id: "x",
        reason: "player_cancelled",
        material_returned: 1,
        qi_refunded: 0,
        ts: 1,
      }),
    );
    expect(pub.published).toHaveLength(0);
  });

  it("dispatches scroll unlock to first_learn category", async () => {
    const pub = new FakePubSub();
    const runtime = new CraftNarrationRuntime({
      llm: makeLlm(
        JSON.stringify({ text: "灯下半夜，残卷字脚里读出门道。", style: "narration" }),
      ),
      model: "mock",
      sub: new FakePubSub(),
      pub,
      logger: silent,
      systemPrompt: "test",
    });
    await runtime.handleUnlocked(JSON.stringify(scrollUnlock));
    expect(runtime.stats.byCategory.firstLearn).toBe(1);
    expect(runtime.stats.byCategory.mentor).toBe(0);
    expect(runtime.stats.byCategory.insight).toBe(0);
    expect(pub.published).toHaveLength(1);
  });

  it("dispatches mentor unlock to mentor category", async () => {
    const pub = new FakePubSub();
    const runtime = new CraftNarrationRuntime({
      llm: makeLlm(
        JSON.stringify({ text: "记下口诀，那位长辈已经走远了。", style: "narration" }),
      ),
      model: "mock",
      sub: new FakePubSub(),
      pub,
      logger: silent,
      systemPrompt: "test",
    });
    await runtime.handleUnlocked(JSON.stringify(mentorUnlock));
    expect(runtime.stats.byCategory.mentor).toBe(1);
  });

  it("dispatches insight unlock to insight category", async () => {
    const pub = new FakePubSub();
    const runtime = new CraftNarrationRuntime({
      llm: makeLlm(
        JSON.stringify({ text: "血光退到指尖，那一线开窍便记下了。", style: "narration" }),
      ),
      model: "mock",
      sub: new FakePubSub(),
      pub,
      logger: silent,
      systemPrompt: "test",
    });
    await runtime.handleUnlocked(JSON.stringify(insightUnlock));
    expect(runtime.stats.byCategory.insight).toBe(1);
  });

  it("rejects malformed JSON without throwing", async () => {
    const pub = new FakePubSub();
    const runtime = new CraftNarrationRuntime({
      llm: makeLlm("any"),
      model: "mock",
      sub: new FakePubSub(),
      pub,
      logger: silent,
      systemPrompt: "test",
    });
    await runtime.handleUnlocked("not json {");
    expect(runtime.stats.rejectedContract).toBe(1);
    expect(pub.published).toHaveLength(0);
  });

  it("rejects payloads that fail TypeBox contract", async () => {
    const pub = new FakePubSub();
    const runtime = new CraftNarrationRuntime({
      llm: makeLlm("any"),
      model: "mock",
      sub: new FakePubSub(),
      pub,
      logger: silent,
      systemPrompt: "test",
    });
    await runtime.handleOutcome(
      JSON.stringify({
        // missing required `kind`
        v: 1,
        player_id: "x",
        recipe_id: "y",
        ts: 1,
      }),
    );
    expect(runtime.stats.rejectedContract).toBe(1);
    expect(pub.published).toHaveLength(0);
  });

  it("falls back to deterministic narration when LLM returns invalid content", async () => {
    const pub = new FakePubSub();
    const runtime = new CraftNarrationRuntime({
      llm: makeLlm("not-json"),
      model: "mock",
      sub: new FakePubSub(),
      pub,
      logger: silent,
      systemPrompt: "test",
    });
    await runtime.handleOutcome(JSON.stringify(completedOutcome));
    expect(pub.published).toHaveLength(1);
    expect(runtime.stats.fallbackUsed).toBe(1);
    const envelope = JSON.parse(pub.published[0].message);
    // Fallback 必须是合法 narration（非空 text + scope=player）
    expect(envelope.narrations[0].scope).toBe("player");
    expect(envelope.narrations[0].text.length).toBeGreaterThan(0);
  });

  it("handles LLM throw with fallback path", async () => {
    const pub = new FakePubSub();
    const failingLlm: LlmClient = {
      async chat() {
        throw new Error("oops");
      },
    };
    const runtime = new CraftNarrationRuntime({
      llm: failingLlm,
      model: "mock",
      sub: new FakePubSub(),
      pub,
      logger: silent,
      systemPrompt: "test",
    });
    await runtime.handleUnlocked(JSON.stringify(scrollUnlock));
    expect(runtime.stats.llmFailures).toBe(1);
    expect(runtime.stats.fallbackUsed).toBe(1);
    expect(pub.published).toHaveLength(1);
  });
});
