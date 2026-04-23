import { describe, expect, it, vi } from "vitest";
import { CHANNELS } from "@bong/schema";

import {
  SkillLvUpNarrationRuntime,
  type SkillLvUpNarrationRuntimeClient,
} from "../src/skill-lv-up-runtime.js";
import type { LlmClient } from "../src/llm.js";

const { AGENT_NARRATE } = CHANNELS;

class FakePubSub implements SkillLvUpNarrationRuntimeClient {
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

const silent = { info: vi.fn(), warn: vi.fn(), error: vi.fn() };

describe("SkillLvUpNarrationRuntime", () => {
  it("publishes one player-scoped narration on valid skill lv up event", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new SkillLvUpNarrationRuntime({
      llm: makeLlm(
        JSON.stringify({
          scope: "player",
          text: "你摘辨草木渐熟，今又进一层，已至Lv.4。",
          style: "narration",
        }),
      ),
      model: "mock",
      sub,
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await runtime.handlePayload(
      JSON.stringify({
        v: 1,
        char_id: 101,
        skill: "herbalism",
        new_lv: 4,
      }),
    );

    expect(pub.published).toHaveLength(1);
    expect(pub.published[0].channel).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.v).toBe(1);
    expect(envelope.narrations).toHaveLength(1);
    expect(envelope.narrations[0]).toEqual({
      scope: "player",
      target: "char:101|skill:herbalism|lv:4",
      text: "你摘辨草木渐熟，今又进一层，已至Lv.4。",
      style: "narration",
    });
    expect(runtime.stats.published).toBe(1);
  });

  it("rejects invalid payload without publishing", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const runtime = new SkillLvUpNarrationRuntime({
      llm: makeLlm(""),
      model: "mock",
      sub,
      pub,
      logger: silent,
      systemPrompt: "test",
    });

    await runtime.handlePayload(JSON.stringify({ v: 1, char_id: 101, skill: "herbalism" }));

    expect(pub.published).toHaveLength(0);
    expect(runtime.stats.rejectedContract).toBe(1);
  });

  it("falls back to deterministic narration when llm fails", async () => {
    const pub = new FakePubSub();
    const sub = new FakePubSub();
    const llm: LlmClient = {
      async chat() {
        throw new Error("boom");
      },
    };
    const runtime = new SkillLvUpNarrationRuntime({
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
        char_id: 202,
        skill: "alchemy",
        new_lv: 3,
      }),
    );

    expect(pub.published).toHaveLength(1);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0].target).toBe("char:202|skill:alchemy|lv:3");
    expect(String(envelope.narrations[0].text)).toContain("炼丹");
    expect(String(envelope.narrations[0].text)).toContain("Lv.3");
    expect(runtime.stats.llmFailures).toBe(1);
    expect(runtime.stats.fallbackUsed).toBe(1);
  });
});
