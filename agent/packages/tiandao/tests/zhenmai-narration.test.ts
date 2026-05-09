import { describe, expect, it, vi } from "vitest";
import { CHANNELS } from "@bong/schema";

import {
  renderZhenmaiNarration,
  renderZhenmaiSkillNarration,
  ZhenmaiNarrationRuntime,
  type ZhenmaiNarrationRuntimeClient,
} from "../src/zhenmai-narration.js";

const { AGENT_NARRATE, COMBAT_REALTIME, ZHENMAI_SKILL_EVENT } = CHANNELS;

class FakePubSub implements ZhenmaiNarrationRuntimeClient {
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

describe("ZhenmaiNarrationRuntime", () => {
  it("subscribes to combat realtime events", async () => {
    const sub = new FakePubSub();
    const runtime = new ZhenmaiNarrationRuntime({
      sub,
      pub: new FakePubSub(),
      logger: silent,
    });

    await runtime.connect();

    expect(sub.subscribedChannels).toEqual([COMBAT_REALTIME, ZHENMAI_SKILL_EVENT]);
  });

  it("renders effectiveness-tiered jiemai narration", () => {
    const narration = renderZhenmaiNarration({
      v: 1,
      kind: "combat_event",
      tick: 42,
      target_id: "offline:Crimson",
      attacker_id: "offline:Azure",
      body_part: "chest",
      wound_kind: "blunt",
      damage: 12,
      contam_delta: 2,
      description: "jiemai=true eff=0.30",
      defense_kind: "jie_mai",
      defense_effectiveness: 0.3,
      defense_contam_reduced: 6,
      defense_wound_severity: 1,
    });

    expect(narration).toEqual({
      scope: "player",
      target: "offline:Crimson",
      text: "Crimson 被逼到贴身处才震爆，经脉护住了些，反冲却全压回血肉里。",
      style: "narration",
    });
  });

  it("publishes narration for jiemai combat events and ignores ordinary hits", async () => {
    const pub = new FakePubSub();
    const runtime = new ZhenmaiNarrationRuntime({
      sub: new FakePubSub(),
      pub,
      logger: silent,
    });

    await runtime.handlePayload(JSON.stringify({
      v: 1,
      kind: "combat_event",
      tick: 100,
      target_id: "offline:Crimson",
      damage: 8,
      contam_delta: 1,
      defense_kind: "jie_mai",
      defense_effectiveness: 0.85,
    }));
    await runtime.handlePayload(JSON.stringify({
      v: 1,
      kind: "combat_event",
      tick: 101,
      target_id: "offline:Crimson",
      damage: 8,
      contam_delta: 1,
    }));

    expect(pub.published).toHaveLength(1);
    expect(pub.published[0].channel).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0].text).toContain("异音未及入脉");
    expect(runtime.stats.published).toBe(1);
    expect(runtime.stats.ignored).toBe(1);
  });

  it("renders all five zhenmai-v2 skill templates", () => {
    const skills = [
      { skill_id: "parry", needle: "短盾" },
      { skill_id: "neutralize", needle: "磨散" },
      { skill_id: "multipoint", needle: "皮下齐震" },
      { skill_id: "harden_meridian", needle: "绷硬" },
      { skill_id: "sever_chain", needle: "按断" },
    ] as const;

    for (const skill of skills) {
      const narration = renderZhenmaiSkillNarration({
        v: 1,
        type: "zhenmai_skill_event",
        skill_id: skill.skill_id,
        caster_id: "offline:Crimson",
        meridian_id: "Lung",
        attack_kind: "physical_carrier",
        grants_amplification: true,
        tick: 99,
      });
      expect(narration?.text).toContain(skill.needle);
      expect(narration?.target).toBe("offline:Crimson");
    }
  });

  it("publishes narration for zhenmai skill-event channel", async () => {
    const pub = new FakePubSub();
    const runtime = new ZhenmaiNarrationRuntime({
      sub: new FakePubSub(),
      pub,
      logger: silent,
    });

    await runtime.handlePayload(ZHENMAI_SKILL_EVENT, JSON.stringify({
      v: 1,
      type: "zhenmai_skill_event",
      skill_id: "sever_chain",
      caster_id: "offline:Crimson",
      meridian_id: "Du",
      attack_kind: "array",
      grants_amplification: false,
      tick: 120,
    }));

    expect(pub.published).toHaveLength(1);
    expect(pub.published[0].channel).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0].target).toBe("offline:Crimson");
    expect(envelope.narrations[0].text).toContain("没有引来足够反震");
  });
});
