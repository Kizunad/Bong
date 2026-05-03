import { describe, expect, it, vi } from "vitest";
import {
  CHANNELS,
  type DeathInsightRequestV1,
  validateNarrationV1Contract,
} from "@bong/schema";

import {
  DeathInsightRuntime,
  type DeathInsightRuntimeClient,
} from "../src/death-insight-runtime.js";
import type { LlmClient } from "../src/llm.js";

const { AGENT_NARRATE, DEATH_INSIGHT } = CHANNELS;

class FakePubSub implements DeathInsightRuntimeClient {
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
    this.listeners = this.listeners.filter((l) => l !== listener);
    return this;
  }

  async unsubscribe(): Promise<void> {}
  disconnect(): void {}

  async publish(channel: string, message: string): Promise<number> {
    this.published.push({ channel, message });
    return 1;
  }
}

function sampleRequest(overrides: Partial<DeathInsightRequestV1> = {}): DeathInsightRequestV1 {
  return {
    v: 1,
    request_id: "death_insight:offline:Azure:84000:3",
    character_id: "offline:Azure",
    at_tick: 84_000,
    cause: "combat:mutant_beast",
    category: "combat",
    realm: "Condense",
    player_realm: "qi_refining_6",
    zone_kind: "ordinary",
    death_count: 3,
    rebirth_chance: 1,
    recent_biography: ["t83100:reach:Condense", "t83980:near_death:combat"],
    position: { x: 8, y: 150, z: 8 },
    context: { will_terminate: false, fortune_remaining: 0 },
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

describe("DeathInsightRuntime", () => {
  it("subscribes to the death insight channel", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const rt = new DeathInsightRuntime({
      llm: makeLlm(""),
      model: "mock",
      sub,
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await rt.connect();
    expect(sub.subscribedChannels).toEqual([DEATH_INSIGHT]);
  });

  it("publishes player-targeted narration from valid LLM text", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const rt = new DeathInsightRuntime({
      llm: makeLlm(JSON.stringify({ text: "你死前看见血谷东侧有灵气回流。", style: "perception" })),
      model: "mock",
      sub,
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await rt.handleRequestPayload(JSON.stringify(sampleRequest()));

    expect(pub.published).toHaveLength(1);
    expect(pub.published[0]?.channel).toBe(AGENT_NARRATE);
    const payload = JSON.parse(pub.published[0]?.message ?? "{}");
    expect(validateNarrationV1Contract(payload).ok).toBe(true);
    expect(payload.narrations[0]).toMatchObject({
      scope: "player",
      target: "offline:Azure",
      text: "你死前看见血谷东侧有灵气回流。",
      style: "perception",
      kind: "death_insight",
    });
    expect(rt.stats.narrated).toBe(1);
  });

  it("falls back to natural death life review when LLM output is invalid", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const rt = new DeathInsightRuntime({
      llm: makeLlm("not json"),
      model: "mock",
      sub,
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await rt.handleRequestPayload(
      JSON.stringify(
        sampleRequest({
          cause: "cultivation:NaturalAging",
          category: "natural",
          lifespan_remaining_years: 0,
          recent_biography: [
            "t100:open:Lung",
            "t900:reach:Induce",
            "t83100:reach:Condense",
            "t83980:near_death:cultivation:NaturalAging",
          ],
        }),
      ),
    );

    const payload = JSON.parse(pub.published[0]?.message ?? "{}");
    const text = String(payload.narrations[0]?.text ?? "");
    expect(text).toContain("寿火已尽");
    expect(text).toContain("生平残页");
    expect(text).toContain("t83980:near_death:cultivation:NaturalAging");
    expect(payload.narrations[0]?.target).toBe("offline:Azure");
  });

  it("adds tribulation chance and final words for terminal tribulation death", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const rt = new DeathInsightRuntime({
      llm: makeLlm(""),
      model: "mock",
      sub,
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await rt.handleRequestPayload(
      JSON.stringify(
        sampleRequest({
          cause: "cultivation:TribulationFailure",
          category: "tribulation",
          realm: "Spirit",
          death_count: 4,
          rebirth_chance: 0.35,
          context: { will_terminate: true },
        }),
      ),
    );

    const payload = JSON.parse(pub.published[0]?.message ?? "{}");
    expect(payload.narrations[0]?.text).toContain("此次运数：35%");
    expect(payload.narrations[0]?.text).toContain("终焉之言");
    expect(payload.narrations[0]?.style).toBe("era_decree");
  });

  it("keeps known spirit eye coordinates in fallback death insight", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const rt = new DeathInsightRuntime({
      llm: makeLlm(""),
      model: "mock",
      sub,
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await rt.handleRequestPayload(
      JSON.stringify(
        sampleRequest({
          realm: "Solidify",
          known_spirit_eyes: [
            {
              eye_id: "eye_spawn_0",
              zone: "qingyun_peaks",
              pos: { x: 920, y: 88, z: -640 },
              qi_concentration: 1,
            },
          ],
        }),
      ),
    );

    const payload = JSON.parse(pub.published[0]?.message ?? "{}");
    const text = String(payload.narrations[0]?.text ?? "");
    expect(text).toContain("灵眼 eye_spawn_0");
    expect(text).toContain("(920, 88, -640)");
  });

  it("keeps low-realm combat fallback short", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const rt = new DeathInsightRuntime({
      llm: makeLlm(""),
      model: "mock",
      sub,
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await rt.handleRequestPayload(
      JSON.stringify(sampleRequest({ realm: "Induce", death_count: 1, rebirth_chance: 1 })),
    );

    const payload = JSON.parse(pub.published[0]?.message ?? "{}");
    expect([...String(payload.narrations[0]?.text ?? "")]).toHaveLength(20);
  });

  it("rejects malformed payload without publishing", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const rt = new DeathInsightRuntime({
      llm: makeLlm(""),
      model: "mock",
      sub,
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await rt.handleRequestPayload("<<garbage>>");
    await rt.handleRequestPayload(JSON.stringify({ v: 1, request_id: "missing-fields" }));

    expect(pub.published).toHaveLength(0);
    expect(rt.stats.rejectedContract).toBe(2);
  });
});
