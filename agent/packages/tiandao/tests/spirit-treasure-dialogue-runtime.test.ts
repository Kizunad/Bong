import { describe, expect, it, vi } from "vitest";
import {
  CHANNELS,
  type SpiritTreasureDialogueRequestV1,
  validateSpiritTreasureDialogueV1Contract,
} from "@bong/schema";

import {
  SpiritTreasureDialogueRuntime,
  type SpiritTreasureDialogueRuntimeClient,
} from "../src/spirit-treasure-dialogue-runtime.js";
import type { LlmClient } from "../src/llm.js";

const { SPIRIT_TREASURE_DIALOGUE_REQUEST, SPIRIT_TREASURE_DIALOGUE } = CHANNELS;

class FakePubSub implements SpiritTreasureDialogueRuntimeClient {
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
    this.listeners = this.listeners.filter((candidate) => candidate !== listener);
    return this;
  }

  async unsubscribe(): Promise<void> {}
  disconnect(): void {}

  async publish(channel: string, message: string): Promise<number> {
    this.published.push({ channel, message });
    return 1;
  }
}

function sampleRequest(
  overrides: Partial<SpiritTreasureDialogueRequestV1> = {},
): SpiritTreasureDialogueRequestV1 {
  return {
    v: 1,
    request_id: "spirit_treasure:7:840",
    character_id: "offline:Azure",
    treasure_id: "spirit_treasure_jizhaojing",
    trigger: "player",
    player_message: "镜中可见什么？",
    context: {
      realm: "Condense",
      qi_percent: 0.72,
      zone: "spawn",
      recent_events: ["刚从清风宗遗迹深层取出寂照镜"],
      affinity: 0.5,
      dialogue_history: [{ speaker: "player", content: "镜中可见什么？" }],
      equipped: true,
    },
    ...overrides,
  };
}

function makeLlm(content: string): LlmClient {
  return {
    async chat(model: string) {
      return { content, durationMs: 0, requestId: null, model };
    },
  };
}

const silent = { info: vi.fn(), warn: vi.fn(), error: vi.fn() };

describe("SpiritTreasureDialogueRuntime", () => {
  it("subscribes to the dialogue request channel", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new SpiritTreasureDialogueRuntime({
      llm: makeLlm(""),
      model: "mock",
      sub,
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await runtime.connect();

    expect(sub.subscribedChannels).toEqual([SPIRIT_TREASURE_DIALOGUE_REQUEST]);
  });

  it("publishes validated dialogue from LLM JSON", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new SpiritTreasureDialogueRuntime({
      llm: makeLlm(
        JSON.stringify({
          text: "镜中不见你，只见你脚下灵脉往西北偏。",
          tone: "curious",
          affinity_delta: 0.03,
        }),
      ),
      model: "mock",
      sub,
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await runtime.handleRequestPayload(JSON.stringify(sampleRequest()));

    expect(pub.published).toHaveLength(1);
    expect(pub.published[0]?.channel).toBe(SPIRIT_TREASURE_DIALOGUE);
    const payload = JSON.parse(pub.published[0]?.message ?? "{}");
    expect(validateSpiritTreasureDialogueV1Contract(payload).ok).toBe(true);
    expect(payload).toMatchObject({
      request_id: "spirit_treasure:7:840",
      character_id: "offline:Azure",
      treasure_id: "spirit_treasure_jizhaojing",
      text: "镜中不见你，只见你脚下灵脉往西北偏。",
      tone: "curious",
      affinity_delta: 0.03,
    });
    expect(runtime.stats.replied).toBe(1);
  });

  it("falls back deterministically when LLM output is invalid", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new SpiritTreasureDialogueRuntime({
      llm: makeLlm("not json"),
      model: "mock",
      sub,
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await runtime.handleRequestPayload(JSON.stringify(sampleRequest()));

    const payload = JSON.parse(pub.published[0]?.message ?? "{}");
    expect(payload.text).toContain("镜面微冷");
    expect(payload.tone).toBe("curious");
    expect(payload.affinity_delta).toBe(0.01);
    expect(runtime.stats.fallbackUsed).toBe(1);
  });

  it("rejects invalid request contracts without publishing", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new SpiritTreasureDialogueRuntime({
      llm: makeLlm(""),
      model: "mock",
      sub,
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await runtime.handleRequestPayload(JSON.stringify({ ...sampleRequest(), v: 2 }));

    expect(pub.published).toHaveLength(0);
    expect(runtime.stats.rejectedContract).toBe(1);
  });
});
