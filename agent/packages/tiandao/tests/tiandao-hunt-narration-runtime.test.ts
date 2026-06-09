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
  public unsubscribeCalls = 0;
  public disconnectCalls = 0;

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

  async unsubscribe(): Promise<void> {
    this.unsubscribeCalls += 1;
  }

  disconnect(): void {
    this.disconnectCalls += 1;
  }

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

function parseOnlyPublishedEnvelope(pub: FakePubSub): unknown {
  expect(
    pub.published.length,
    `expected exactly one publish because a valid narration should be emitted; actual pub.published=${JSON.stringify(pub.published)}; repair: inspect request contract and publish channel`,
  ).toBe(1);
  expect(
    pub.published[0]?.channel,
    `expected publish channel ${AGENT_NARRATE}; actual=${pub.published[0]?.channel}; repair: check AGENT_NARRATE routing`,
  ).toBe(AGENT_NARRATE);
  return JSON.parse(pub.published[0]?.message ?? "{}");
}

function assertNarrationEnvelope(
  envelope: unknown,
  expected: Record<string, unknown>,
): void {
  const validation = validateNarrationV1Contract(envelope);
  expect(
    validation.ok,
    `expected validateNarrationV1Contract(envelope) to pass; actual=${JSON.stringify(validation)}; repair: check NarrationV1 shape`,
  ).toBe(true);
  expect(
    envelope,
    `expected envelope to match narration contract fixture; actual=${JSON.stringify(envelope)}; repair: check scope/target/style mapping`,
  ).toEqual(expected);
}

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

      const envelope = parseOnlyPublishedEnvelope(pub);
      assertNarrationEnvelope(envelope, {
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
      expect(
        runtime.stats.received,
        `expected runtime.stats.received=1 after one valid request; actual=${runtime.stats.received}; repair: increment after request validation`,
      ).toBe(1);
      expect(
        runtime.stats.published,
        `expected runtime.stats.published=1 after one successful publish; actual=${runtime.stats.published}; repair: increment after pub.publish`,
      ).toBe(1);
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

      const envelope = parseOnlyPublishedEnvelope(pub) as {
        narrations: Array<Record<string, unknown>>;
      };
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

    expect(
      pub.published,
      `expected no publish for rejected request contract; actual=${JSON.stringify(pub.published)}; repair: return before pub.publish on contract failure`,
    ).toHaveLength(0);
    expect(
      runtime.stats.rejectedContract,
      `expected runtime.stats.rejectedContract=1 for invalid response_level; actual=${runtime.stats.rejectedContract}; repair: increment rejectedContract on schema failure`,
    ).toBe(1);
  });

  it.each([
    ["LLM throws", async () => {
      throw new Error("boom");
    }, 1],
    ["LLM returns non-JSON", async (model: string) => ({
      content: "天道沉默",
      durationMs: 0,
      requestId: null,
      model,
    }), 0],
    ["LLM returns invalid style", async (model: string) => ({
      content: JSON.stringify({ text: "尘土发冷。", style: "bad_style" }),
      durationMs: 0,
      requestId: null,
      model,
    }), 0],
  ] as const)("falls back deterministically when %s", async (_name, chat, failures) => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const llm: LlmClient = { chat };
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

    const envelope = parseOnlyPublishedEnvelope(pub) as {
      narrations: Array<Record<string, unknown>>;
    };
    expect(validateNarrationV1Contract(envelope).ok).toBe(true);
    expect(envelope.narrations[0]).toMatchObject({
      scope: "player",
      target: "player-001",
      style: "system_warning",
    });
    expect(String(envelope.narrations[0].text)).toContain("north_wastes");
    expect(runtime.stats.llmFailures).toBe(failures);
    expect(runtime.stats.fallbackUsed).toBe(1);
  });

  it("rejects non-JSON payloads without publishing", async () => {
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

    await runtime.handleRequestPayload("{not-json");

    expect(
      pub.published,
      `expected no publish for non-JSON payload; actual=${JSON.stringify(pub.published)}; repair: keep JSON.parse failure on rejectedContract path`,
    ).toHaveLength(0);
    expect(runtime.stats.rejectedContract).toBe(1);
    expect(runtime.stats.received).toBe(0);
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
    const envelope = parseOnlyPublishedEnvelope(pub) as {
      narrations: Array<Record<string, unknown>>;
    };
    expect(envelope.narrations[0]?.scope).toBe("player");
  });

  it("keeps connect idempotent and disconnect removes the message listener", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new TiandaoHuntNarrationRuntime({
      llm: makeLlm(
        JSON.stringify({
          text: "灰风压低，尘粒贴着脚面走。",
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
    await runtime.connect();

    expect(
      sub.subscribedChannels,
      `expected idempotent connect to subscribe once to ${TIANDAO_HUNT_NARRATION_REQUEST}; actual=${JSON.stringify(sub.subscribedChannels)}; repair: keep connected guard before subscribe`,
    ).toEqual([TIANDAO_HUNT_NARRATION_REQUEST]);
    expect(sub.listeners).toHaveLength(1);

    await runtime.disconnect();
    sub.emit(TIANDAO_HUNT_NARRATION_REQUEST, JSON.stringify(sampleRequest()));
    await flushMicrotasks();

    expect(
      pub.published,
      `expected no publish after disconnect removed listener; actual=${JSON.stringify(pub.published)}; repair: call off(\"message\", onMessage) during disconnect`,
    ).toHaveLength(0);
    expect(sub.unsubscribeCalls).toBe(1);
    expect(sub.disconnectCalls).toBe(1);
    expect(pub.disconnectCalls).toBe(1);
  });
});
