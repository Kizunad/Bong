import { describe, expect, it, vi } from "vitest";
import { CHANNELS, type HeartDemonPregenRequestV1 } from "@bong/schema";

import {
  applyHeartDemonArbiter,
  fallbackHeartDemonOffer,
  HeartDemonRuntime,
  type HeartDemonRuntimeClient,
} from "../src/heart-demon-runtime.js";
import type { LlmClient } from "../src/llm.js";

const { HEART_DEMON_OFFER } = CHANNELS;

class FakePubSub implements HeartDemonRuntimeClient {
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

function sampleRequest(overrides: Partial<HeartDemonPregenRequestV1> = {}): HeartDemonPregenRequestV1 {
  return {
    trigger_id: "heart_demon:1:1000",
    character_id: "offline:Azure",
    actor_name: "Azure",
    realm: "Spirit",
    qi_color_state: { main: "Mellow", is_chaotic: false, is_hunyuan: false },
    recent_biography: ["t120:open:Lung", "t240:reach:Spirit"],
    composure: 0.7,
    started_tick: 1000,
    waves_total: 5,
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

describe("fallbackHeartDemonOffer", () => {
  it("keeps the canonical steadfast choice reachable", () => {
    const offer = fallbackHeartDemonOffer(sampleRequest(), () => 10_000);
    expect(offer.trigger_id).toBe("heart_demon:1:1000");
    expect(offer.choices.map((choice) => choice.choice_id)).toEqual([
      "heart_demon_choice_0",
      "heart_demon_choice_1",
      "heart_demon_choice_2",
    ]);
    expect(offer.choices[0].category).toBe("Composure");
    expect(offer.choices[0].title).toBe("守本心");
  });
});

describe("applyHeartDemonArbiter", () => {
  it("normalizes LLM choice ids and preserves server outcome ordering", () => {
    const req = sampleRequest();
    const normalized = applyHeartDemonArbiter(req, {
      offer_id: "custom",
      trigger_id: req.trigger_id,
      trigger_label: "心魔照见",
      realm_label: "渡虚劫 · 心魔",
      composure: 0.2,
      quota_remaining: 99,
      quota_total: 99,
      expires_at_ms: 1,
      choices: [
        {
          choice_id: "heart_demon_choice_2",
          category: "Qi",
          title: "承认无门",
          effect_summary: "wrong",
          flavor: "此路本无门。",
          style_hint: "冷",
        },
      ],
    });
    expect(normalized.choices.map((choice) => choice.choice_id)).toEqual([
      "heart_demon_choice_0",
      "heart_demon_choice_1",
      "heart_demon_choice_2",
    ]);
    expect(normalized.choices[2].category).toBe("Perception");
    expect(normalized.choices[2].effect_summary).toContain("不得增益");
    expect(normalized.quota_remaining).toBe(1);
    expect(normalized.expires_at_ms).toBeGreaterThan(10_000);
  });

  it("does not trust LLM text that makes the steadfast choice a trap", () => {
    const req = sampleRequest();
    const normalized = applyHeartDemonArbiter(req, {
      offer_id: "custom",
      trigger_id: req.trigger_id,
      trigger_label: "心魔照见",
      realm_label: "渡虚劫 · 心魔",
      composure: 0.2,
      quota_remaining: 1,
      quota_total: 1,
      expires_at_ms: 1,
      choices: [
        {
          choice_id: "heart_demon_choice_0",
          category: "Composure",
          title: "守本心",
          effect_summary: "稳住心神，回复少量当前真元",
          flavor: "这是一条陷阱，选了便被心魔吞没。",
          style_hint: "不可达",
        },
      ],
    }, () => 10_000);

    const fallback = fallbackHeartDemonOffer(req, () => 10_000);
    expect(normalized.choices[0].flavor).toBe(fallback.choices[0].flavor);
    expect(normalized.choices[0].style_hint).toBe(fallback.choices[0].style_hint);
  });
});

describe("HeartDemonRuntime.handleRequestPayload", () => {
  it("publishes a valid canonical offer on successful LLM output", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const req = sampleRequest();
    const llmContent = JSON.stringify({
      offer_id: "ofr-heart-demon",
      trigger_id: req.trigger_id,
      trigger_label: "心魔照见",
      realm_label: "渡虚劫 · 心魔",
      composure: 0.6,
      quota_remaining: 1,
      quota_total: 1,
      expires_at_ms: 123,
      choices: [
        {
          choice_id: "heart_demon_choice_0",
          category: "Composure",
          title: "记起开脉时",
          effect_summary: "稳住心神，回复少量当前真元",
          flavor: "你记起 t120:open:Lung，不再追逐影子。",
          style_hint: "稳妥",
        },
        {
          choice_id: "heart_demon_choice_1",
          category: "Breakthrough",
          title: "斩旧影",
          effect_summary: "若斩错心魔，将损当前真元并强化下一道开天雷",
          flavor: "刀锋照见自己的影。",
          style_hint: "冒险",
        },
        {
          choice_id: "heart_demon_choice_2",
          category: "Perception",
          title: "无解",
          effect_summary: "承认无解，不得增益也不受真元惩罚",
          flavor: "此题无门。",
          style_hint: "止损",
        },
      ],
    });
    const rt = new HeartDemonRuntime({
      llm: makeLlm(llmContent),
      model: "mock",
      sub,
      pub,
      logger: silent,
      systemPrompt: "test",
      now: () => 10_000,
    });
    await rt.handleRequestPayload(JSON.stringify(req));

    expect(pub.published.length).toBe(1);
    expect(pub.published[0].channel).toBe(HEART_DEMON_OFFER);
    const offer = JSON.parse(pub.published[0].message);
    expect(offer.trigger_id).toBe(req.trigger_id);
    expect(offer.expires_at_ms).toBe(40_000);
    expect(offer.choices[0].choice_id).toBe("heart_demon_choice_0");
    expect(offer.choices[0].category).toBe("Composure");
  });

  it("publishes fallback offer when LLM output is invalid", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const rt = new HeartDemonRuntime({
      llm: makeLlm("not json"),
      model: "mock",
      sub,
      pub,
      logger: silent,
      systemPrompt: "test",
      now: () => 10_000,
    });
    await rt.handleRequestPayload(JSON.stringify(sampleRequest()));

    expect(pub.published.length).toBe(1);
    const offer = JSON.parse(pub.published[0].message);
    expect(offer.choices.map((choice: { choice_id: string }) => choice.choice_id)).toEqual([
      "heart_demon_choice_0",
      "heart_demon_choice_1",
      "heart_demon_choice_2",
    ]);
    expect(rt.stats.fallbackUsed).toBe(1);
  });
});
