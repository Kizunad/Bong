import { describe, expect, it, vi } from "vitest";
import { CHANNELS, InsightRequestV1 } from "@bong/schema";

import {
  applyInsightArbiter,
  InsightRuntime,
  type InsightRuntimeClient,
} from "../src/insight-runtime.js";
import type { LlmClient } from "../src/llm.js";

const { INSIGHT_OFFER } = CHANNELS;

class FakePubSub implements InsightRuntimeClient {
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

function sampleRequest(overrides: Partial<InsightRequestV1> = {}): InsightRequestV1 {
  return {
    trigger_id: "first_induce_breakthrough",
    character_id: "Azure",
    realm: "Induce",
    qi_color_state: { main: "Mellow", is_chaotic: false, is_hunyuan: false },
    recent_biography: ["t120:open:Lung", "t240:reach:Induce"],
    composure: 0.7,
    available_categories: ["Meridian", "Qi", "Composure"],
    global_caps: { Meridian: 0.2, Qi: 10, Composure: 0.3 },
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

function choice(overrides: Record<string, unknown> = {}) {
  return {
    category: "Qi" as const,
    effect_kind: "QiRegenFactor",
    magnitude: 0.03,
    flavor_text: "真元回流",
    alignment: "neutral" as const,
    cost_kind: "qi_volatility",
    cost_magnitude: 0.015,
    cost_flavor: "真元挥发加速",
    ...overrides,
  };
}

describe("applyInsightArbiter", () => {
  it("drops out-of-whitelist categories", () => {
    const req = sampleRequest();
    const filtered = applyInsightArbiter(req, {
      offer_id: "o1",
      trigger_id: req.trigger_id,
      character_id: req.character_id,
      choices: [
        choice({ category: "Meridian", effect_kind: "MeridianIntegrityBoost", magnitude: 0.1, cost_magnitude: 0.05, alignment: "converge" }),
        choice({ category: "Style", effect_kind: "StyleUnlock", magnitude: 1, flavor_text: "bad", alignment: "diverge" }),
      ],
    });
    expect(filtered.choices.length).toBe(1);
    expect(filtered.choices[0].category).toBe("Meridian");
  });

  it("drops magnitude over cap", () => {
    const req = sampleRequest();
    const filtered = applyInsightArbiter(req, {
      offer_id: "o1",
      trigger_id: req.trigger_id,
      character_id: req.character_id,
      choices: [
        choice({ category: "Qi", effect_kind: "QiMaxBoost", magnitude: 999, cost_magnitude: 500, flavor_text: "over cap" }),
        choice({ category: "Qi", effect_kind: "QiMaxBoost", magnitude: 8, cost_magnitude: 4, flavor_text: "ok" }),
      ],
    });
    expect(filtered.choices.length).toBe(1);
    expect(filtered.choices[0].magnitude).toBe(8);
  });

  it("drops non-positive magnitude", () => {
    const req = sampleRequest();
    const filtered = applyInsightArbiter(req, {
      offer_id: "o1",
      trigger_id: req.trigger_id,
      character_id: req.character_id,
      choices: [
        choice({ magnitude: 0, cost_magnitude: 0.1, alignment: "converge" }),
        choice({ magnitude: -0.01, cost_magnitude: 0.1, alignment: "neutral" }),
        choice({ magnitude: 0.03, cost_magnitude: 0.015, alignment: "diverge" }),
      ],
    });
    expect(filtered.choices.length).toBe(1);
    expect(filtered.choices[0].alignment).toBe("diverge");
  });

  it("caps at one choice per alignment", () => {
    const req = sampleRequest();
    const many = Array.from({ length: 6 }, (_, i) => ({
      category: "Composure" as const,
      effect_kind: "ComposureRestore",
      magnitude: 0.1 + i * 0.01,
      flavor_text: `#${i}`,
      alignment: undefined,
      cost_kind: "shock_sensitivity",
      cost_magnitude: 0.2,
      cost_flavor: "心境冲击敏感",
    }));
    const filtered = applyInsightArbiter(req, {
      offer_id: "o1",
      trigger_id: req.trigger_id,
      character_id: req.character_id,
      choices: many,
    });
    expect(filtered.choices.length).toBe(3);
  });

  it("fills missing alignment by sequence", () => {
    const req = sampleRequest();
    const filtered = applyInsightArbiter(req, {
      offer_id: "o1",
      trigger_id: req.trigger_id,
      character_id: req.character_id,
      choices: [
        choice({ category: "Meridian", alignment: undefined }),
        choice({ category: "Qi", alignment: undefined }),
        choice({ category: "Composure", alignment: undefined }),
      ],
    });
    expect(filtered.choices.map((c) => c.alignment)).toEqual(["converge", "neutral", "diverge"]);
  });

  it("deduplicates repeated alignment", () => {
    const req = sampleRequest();
    const filtered = applyInsightArbiter(req, {
      offer_id: "o1",
      trigger_id: req.trigger_id,
      character_id: req.character_id,
      choices: [
        choice({ category: "Meridian", alignment: "converge" }),
        choice({ category: "Qi", alignment: "converge" }),
        choice({ category: "Composure", alignment: "diverge" }),
      ],
    });
    expect(filtered.choices.map((c) => c.alignment)).toEqual(["converge", "neutral", "diverge"]);
  });
});

describe("InsightRuntime.handleRequestPayload", () => {
  it("publishes a valid offer on successful LLM output", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const llmContent = JSON.stringify({
      offer_id: "ofr_test_1",
      trigger_id: "first_induce_breakthrough",
      choices: [
        choice({
          category: "Meridian",
          effect_kind: "MeridianIntegrityBoost",
          magnitude: 0.15,
          flavor_text: "经脉微明",
          alignment: "converge",
          cost_kind: "opposite_color_penalty",
          cost_magnitude: 0.08,
          cost_flavor: "对立色效率下降",
        }),
        choice({
          category: "Qi",
          effect_kind: "QiRegenFactor",
          magnitude: 0.03,
          flavor_text: "真元回流",
          alignment: "neutral",
          cost_kind: "qi_volatility",
          cost_magnitude: 0.015,
          cost_flavor: "真元挥发加速",
        }),
        choice({
          category: "Composure",
          effect_kind: "ComposureRestore",
          magnitude: 0.1,
          flavor_text: "心湖回稳",
          alignment: "diverge",
          cost_kind: "shock_sensitivity",
          cost_magnitude: 0.05,
          cost_flavor: "心境冲击敏感",
        }),
      ],
    });
    const rt = new InsightRuntime({
      llm: makeLlm(llmContent),
      model: "mock",
      sub,
      pub,
      logger: silent,
      systemPrompt: "test",
    });
    await rt.handleRequestPayload(JSON.stringify(sampleRequest()));
    expect(pub.published.length).toBe(1);
    expect(pub.published[0].channel).toBe(INSIGHT_OFFER);
    expect(rt.stats.offered).toBe(1);
  });

  it("publishes fallback empty-offer when LLM returns invalid JSON", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const rt = new InsightRuntime({
      llm: makeLlm("not json"),
      model: "mock",
      sub,
      pub,
      logger: silent,
      systemPrompt: "test",
    });
    await rt.handleRequestPayload(JSON.stringify(sampleRequest()));
    expect(pub.published.length).toBe(1);
    const offer = JSON.parse(pub.published[0].message);
    expect(offer.trigger_id).toBe("first_induce_breakthrough");
    expect(Array.isArray(offer.choices)).toBe(true);
  });

  it("publishes fallback empty-offer when arbiter leaves fewer than three choices", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const llmContent = JSON.stringify({
      offer_id: "ofr_short",
      trigger_id: "first_induce_breakthrough",
      choices: [
        choice({
          category: "Meridian",
          effect_kind: "MeridianIntegrityBoost",
          magnitude: 0.15,
          alignment: "converge",
          cost_kind: "opposite_color_penalty",
          cost_magnitude: 0.08,
        }),
      ],
    });
    const rt = new InsightRuntime({
      llm: makeLlm(llmContent),
      model: "mock",
      sub,
      pub,
      logger: silent,
      systemPrompt: "test",
    });
    await rt.handleRequestPayload(JSON.stringify(sampleRequest()));
    expect(pub.published.length).toBe(1);
    const offer = JSON.parse(pub.published[0].message);
    expect(offer.choices).toHaveLength(1);
    expect(offer.choices[0].effect_kind).toBe("NoOp");
    expect(rt.stats.rejectedArbiter).toBe(1);
  });

  it("rejects non-JSON payload without publishing", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const rt = new InsightRuntime({
      llm: makeLlm(""),
      model: "mock",
      sub,
      pub,
      logger: silent,
      systemPrompt: "test",
    });
    await rt.handleRequestPayload("<<garbage>>");
    expect(pub.published.length).toBe(0);
    expect(rt.stats.rejectedContract).toBeGreaterThanOrEqual(1);
  });

  it("publishes fallback when arbiter filters all choices", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const llm = makeLlm(
      JSON.stringify({
        offer_id: "ofr_x",
        trigger_id: "first_induce_breakthrough",
        choices: [
          // all out of caps
          choice({ category: "Qi", effect_kind: "QiMaxBoost", magnitude: 999, cost_magnitude: 500, flavor_text: "a" }),
          choice({ category: "Style", effect_kind: "StyleUnlock", magnitude: 1, cost_magnitude: 1, flavor_text: "b" }),
        ],
      }),
    );
    const rt = new InsightRuntime({
      llm,
      model: "mock",
      sub,
      pub,
      logger: silent,
      systemPrompt: "test",
    });
    await rt.handleRequestPayload(JSON.stringify(sampleRequest()));
    expect(pub.published.length).toBe(1);
    expect(rt.stats.rejectedArbiter).toBe(1);
  });
});
