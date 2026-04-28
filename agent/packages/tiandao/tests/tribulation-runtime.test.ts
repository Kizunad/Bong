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
});
