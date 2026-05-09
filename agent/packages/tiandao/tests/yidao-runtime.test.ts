import { describe, expect, it, vi } from "vitest";
import { CHANNELS, type YidaoEventV1 } from "@bong/schema";

import {
  renderYidaoNarration,
  YidaoNarrationRuntime,
  type YidaoNarrationRuntimeClient,
} from "../src/yidao-runtime.js";

const { AGENT_NARRATE, YIDAO_EVENT } = CHANNELS;

class FakePubSub implements YidaoNarrationRuntimeClient {
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

const silent = { info: vi.fn(), warn: vi.fn() };

const baseEvent: YidaoEventV1 = {
  v: 1,
  kind: "meridian_heal",
  tick: 42,
  medic_id: "offline:Doctor",
  patient_ids: ["offline:Patient"],
  skill: "meridian_repair",
  meridian_id: "Lung",
  success_count: 1,
  failure_count: 0,
  qi_transferred: 80,
  contam_reduced: 0,
  hp_restored: 0,
  karma_delta: 0,
  medic_qi_max_delta: 0,
  patient_qi_max_delta: 0,
  contract_state: "patient",
  detail: "meridian repair",
};

describe("YidaoNarrationRuntime", () => {
  it("subscribes to yidao events", async () => {
    const sub = new FakePubSub();
    const runtime = new YidaoNarrationRuntime({
      sub,
      pub: new FakePubSub(),
      logger: silent,
    });

    await runtime.connect();

    expect(sub.subscribedChannels).toEqual([YIDAO_EVENT]);
  });

  it("renders five skill narration with contract state", () => {
    const narration = renderYidaoNarration(baseEvent);

    expect(narration?.target).toBe(
      "yidao:meridian_heal|medic:offline:Doctor|tick:42",
    );
    expect(narration?.text).toContain("接回 Lung 经");
    expect(narration?.text).toContain("医患关系转为 患者");
  });

  it("publishes narration and rejects invalid payloads", async () => {
    const pub = new FakePubSub();
    const runtime = new YidaoNarrationRuntime({
      sub: new FakePubSub(),
      pub,
      logger: silent,
    });

    await runtime.handlePayload(JSON.stringify({
      ...baseEvent,
      kind: "life_extension",
      skill: "life_extension",
      karma_delta: 4.5,
      medic_qi_max_delta: -0.1,
      patient_qi_max_delta: -0.1,
      detail: "life extension",
    }));
    await runtime.handlePayload(JSON.stringify({ ...baseEvent, skill: "bad" }));

    expect(pub.published).toHaveLength(1);
    expect(pub.published[0].channel).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0].style).toBe("system_warning");
    expect(envelope.narrations[0].text).toContain("续命术");
    expect(runtime.stats.published).toBe(1);
    expect(runtime.stats.rejectedContract).toBe(1);
  });
});
