import { describe, expect, it, vi } from "vitest";
import {
  CHANNELS,
  type TiandaoHuntNarrationRequestV1,
  validateNarrationV1Contract,
} from "@bong/schema";

import {
  TiandaoHuntNarrationRuntime,
  type TiandaoHuntNarrationRuntimeClient,
} from "../src/tiandao-hunt-narration-runtime.js";
import type { LlmClient } from "../src/llm.js";

const { AGENT_NARRATE, TIANDAO_HUNT_NARRATION_REQUEST } = CHANNELS;

class FakePubSub implements TiandaoHuntNarrationRuntimeClient {
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

  emit(channel: string, message: string): void {
    for (const listener of this.listeners) {
      listener(channel, message);
    }
  }
}

function sampleRequest(
  overrides: Partial<TiandaoHuntNarrationRequestV1> = {},
): TiandaoHuntNarrationRequestV1 {
  return {
    v: 1,
    character_id: "player-001",
    realm: "Condense",
    attention_level: 42,
    response_level: "pressure",
    zone: "spawn",
    recent_actions: [
      "activity:realm_breakthrough",
      "countermeasure:none",
      "zone_qi:0.58",
    ],
    narration_count: 1,
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

async function flushMicrotasks(): Promise<void> {
  await new Promise<void>((resolve) => setImmediate(resolve));
}

const silent = { info: vi.fn(), warn: vi.fn(), error: vi.fn() };

describe("TiandaoHuntNarrationRuntime", () => {
  it.each(["watch", "pressure"] as const)(
    "publishes a validated player-scoped narration for %s requests",
    async (responseLevel) => {
      const pub = new FakePubSub();
      const sub = new FakePubSub();
      const runtime = new TiandaoHuntNarrationRuntime({
        llm: makeLlm(
          JSON.stringify({
            text: "你背后忽然发冷，像有灰尘在记你的脚步。",
            style: "perception",
          }),
        ),
        model: "mock",
        sub,
        pub,
        logger: silent,
        systemPrompt: "test",
      });

      await runtime.handleRequestPayload(
        JSON.stringify(sampleRequest({ response_level: responseLevel })),
      );

      expect(pub.published).toHaveLength(1);
      expect(pub.published[0]?.channel).toBe(AGENT_NARRATE);
      const envelope = JSON.parse(pub.published[0]?.message ?? "{}");
      expect(validateNarrationV1Contract(envelope).ok).toBe(true);
      expect(envelope).toEqual({
        v: 1,
        narrations: [
          {
            scope: "player",
            target: "player-001",
            text: "你背后忽然发冷，像有灰尘在记你的脚步。",
            style: "perception",
          },
        ],
      });
      expect(runtime.stats.received).toBe(1);
      expect(runtime.stats.published).toBe(1);
    },
  );

  it.each(["tribulation", "annihilate"] as const)(
    "publishes broadcast narration for %s requests",
    async (responseLevel) => {
      const pub = new FakePubSub();
      const sub = new FakePubSub();
      const runtime = new TiandaoHuntNarrationRuntime({
        llm: makeLlm(
          JSON.stringify({
            text: "荒地上空压下一道闷雷，尘光往低处倒卷。",
            style: "system_warning",
          }),
        ),
        model: "mock",
        sub,
        pub,
        logger: silent,
        systemPrompt: "test",
      });

      await runtime.handleRequestPayload(
        JSON.stringify(sampleRequest({ response_level: responseLevel })),
      );

      const envelope = JSON.parse(pub.published[0]?.message ?? "{}");
      expect(validateNarrationV1Contract(envelope).ok).toBe(true);
      expect(envelope.narrations[0]).toEqual({
        scope: "broadcast",
        text: "荒地上空压下一道闷雷，尘光往低处倒卷。",
        style: "system_warning",
      });
    },
  );

  it("rejects invalid request contracts without publishing", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new TiandaoHuntNarrationRuntime({
      llm: makeLlm(""),
      model: "mock",
      sub,
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await runtime.handleRequestPayload(
      JSON.stringify({ ...sampleRequest(), response_level: "none" }),
    );

    expect(pub.published).toHaveLength(0);
    expect(runtime.stats.rejectedContract).toBe(1);
  });

  it("falls back deterministically when LLM fails", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const llm: LlmClient = {
      async chat() {
        throw new Error("boom");
      },
    };
    const runtime = new TiandaoHuntNarrationRuntime({
      llm,
      model: "mock",
      sub,
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await runtime.handleRequestPayload(
      JSON.stringify(
        sampleRequest({ response_level: "pressure", zone: "north_wastes" }),
      ),
    );

    const envelope = JSON.parse(pub.published[0]?.message ?? "{}");
    expect(validateNarrationV1Contract(envelope).ok).toBe(true);
    expect(envelope.narrations[0]).toMatchObject({
      scope: "player",
      target: "player-001",
      style: "system_warning",
    });
    expect(String(envelope.narrations[0].text)).toContain("north_wastes");
    expect(runtime.stats.llmFailures).toBe(1);
    expect(runtime.stats.fallbackUsed).toBe(1);
  });

  it("subscribes to request channel and routes messages on connect", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new TiandaoHuntNarrationRuntime({
      llm: makeLlm(
        JSON.stringify({
          text: "天色低了一寸，脚下尘土无声发紧。",
          style: "perception",
        }),
      ),
      model: "mock",
      sub,
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await runtime.connect();
    sub.emit("other", JSON.stringify(sampleRequest()));
    sub.emit(
      TIANDAO_HUNT_NARRATION_REQUEST,
      JSON.stringify(sampleRequest({ response_level: "watch" })),
    );
    await flushMicrotasks();

    expect(sub.subscribedChannels).toEqual([TIANDAO_HUNT_NARRATION_REQUEST]);
    expect(pub.published).toHaveLength(1);
    expect(JSON.parse(pub.published[0]?.message ?? "{}").narrations[0].scope).toBe(
      "player",
    );
  });
});
