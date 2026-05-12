import { describe, expect, it, vi } from "vitest";
import { CHANNELS } from "@bong/schema";

import {
  POLITICAL_EVENT_CHANNELS,
  POLITICAL_THROTTLE_MS,
  PoliticalNarrationRuntime,
  PoliticalNarrationThrottleStore,
  renderPoliticalNarration,
  type PoliticalNarrationRuntimeClient,
} from "../src/political-narration.js";

const {
  AGENT_NARRATE,
  SOCIAL_FEUD,
  SOCIAL_PACT,
  SOCIAL_RENOWN_DELTA,
  SOCIAL_NICHE_INTRUSION,
  WANTED_PLAYER,
  HIGH_RENOWN_MILESTONE,
} = CHANNELS;

class FakePubSub implements PoliticalNarrationRuntimeClient {
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

class FlakyPubSub extends FakePubSub {
  public failNextPublish = true;

  override async publish(channel: string, message: string): Promise<number> {
    if (this.failNextPublish) {
      this.failNextPublish = false;
      throw new Error("transient publish failure");
    }
    return super.publish(channel, message);
  }
}

const silent = { info: vi.fn(), warn: vi.fn() };

describe("PoliticalNarrationRuntime", () => {
  it("subscribes to the political event channels", async () => {
    const runtime = new PoliticalNarrationRuntime({
      sub: new FakePubSub(),
      pub: new FakePubSub(),
      logger: silent,
    });
    const sub = (runtime as unknown as { sub: FakePubSub }).sub;

    await runtime.connect();

    expect(sub.subscribedChannels).toEqual([...POLITICAL_EVENT_CHANNELS]);
  });

  it("publishes feud narration with jianghu style and zone scope", async () => {
    const pub = new FakePubSub();
    const runtime = new PoliticalNarrationRuntime({
      sub: new FakePubSub(),
      pub,
      logger: silent,
      now: () => 1000,
    });

    await runtime.handlePayload(
      SOCIAL_FEUD,
      JSON.stringify({
        v: 1,
        left: "char:one",
        right: "char:two",
        tick: 42,
        place: "blood_valley",
      }),
    );

    expect(pub.published).toHaveLength(1);
    expect(pub.published[0].channel).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(pub.published[0].message);
    expect(envelope.narrations[0]).toEqual(
      expect.objectContaining({
        scope: "zone",
        target: "blood_valley",
        style: "political_jianghu",
        kind: "political_jianghu",
      }),
    );
    expect(envelope.narrations[0].text).toMatch(/江湖|传|市井|山中/);
  });

  it("consumes pact, niche, wanted, and high renown milestone events", async () => {
    let now = 10_000;
    const pub = new FakePubSub();
    const runtime = new PoliticalNarrationRuntime({
      sub: new FakePubSub(),
      pub,
      logger: silent,
      now: () => now,
    });

    await runtime.handlePayload(
      SOCIAL_PACT,
      JSON.stringify({ v: 1, left: "char:a", right: "char:b", terms: "同行", tick: 1, broken: false }),
    );
    now += POLITICAL_THROTTLE_MS + 1;
    await runtime.handlePayload(
      SOCIAL_NICHE_INTRUSION,
      JSON.stringify({ v: 1, niche_pos: [1, 64, 2], intruder_id: "char:raider", items_taken: [7], taint_delta: 0.1 }),
    );
    await runtime.handlePayload(
      WANTED_PLAYER,
      JSON.stringify({
        event: "wanted_player",
        player_uuid: "7a8f80c2-82ad-5d7c-a0dd-b3c1b7d2e1a1",
        char_id: "offline:kiz",
        identity_display_name: "玄锋",
        identity_id: 0,
        reputation_score: -100,
        primary_tag: "dugu_revealed",
        tick: 3,
      }),
    );
    await runtime.handlePayload(
      HIGH_RENOWN_MILESTONE,
      JSON.stringify({
        v: 1,
        event: "high_renown_milestone",
        player_uuid: "7a8f80c2-82ad-5d7c-a0dd-b3c1b7d2e1a1",
        char_id: "offline:kiz",
        identity_id: 0,
        identity_display_name: "玄锋",
        fame: 1000,
        milestone: 1000,
        identity_exposed: true,
        tick: 4,
        zone: "spawn",
      }),
    );

    expect(pub.published).toHaveLength(4);
    const narrations = pub.published.map((entry) => JSON.parse(entry.message).narrations[0]);
    expect(narrations[0].target).toBe("spawn");
    expect(narrations[1].target).toBe("spawn");
    expect(narrations[1].text).not.toContain("niche:");
    expect(narrations[2].scope).toBe("broadcast");
    expect(narrations[2].text).toContain("玄锋");
    expect(narrations[3].scope).toBe("broadcast");
  });

  it("turns pvp betrayal renown deltas into anonymous jianghu narration", async () => {
    const pub = new FakePubSub();
    const runtime = new PoliticalNarrationRuntime({
      sub: new FakePubSub(),
      pub,
      logger: silent,
    });

    await runtime.handlePayload(
      SOCIAL_RENOWN_DELTA,
      JSON.stringify({
        v: 1,
        char_id: "char:bob",
        fame_delta: 0,
        notoriety_delta: 30,
        tags_added: [{ tag: "背信者", weight: 30, last_seen_tick: 77, permanent: false }],
        tick: 77,
        reason: "pvp_betrayal",
      }),
    );

    expect(pub.published).toHaveLength(1);
    const narration = JSON.parse(pub.published[0].message).narrations[0];
    expect(narration.scope).toBe("zone");
    expect(narration.style).toBe("political_jianghu");
    expect(narration.text).toContain("背弃同伴");
    expect(narration.text).not.toContain("char:bob");
  });

  it("ignores broken pact events", async () => {
    const pub = new FakePubSub();
    const runtime = new PoliticalNarrationRuntime({
      sub: new FakePubSub(),
      pub,
      logger: silent,
    });

    await runtime.handlePayload(
      SOCIAL_PACT,
      JSON.stringify({ v: 1, left: "char:a", right: "char:b", terms: "同行", tick: 1, broken: true }),
    );

    expect(pub.published).toHaveLength(0);
    expect(runtime.stats.ignored).toBe(1);
  });

  it("throttles same-zone ordinary events but lets bypass events through", async () => {
    const pub = new FakePubSub();
    const store = new PoliticalNarrationThrottleStore();
    const runtime = new PoliticalNarrationRuntime({
      sub: new FakePubSub(),
      pub,
      logger: silent,
      now: () => 50_000,
      throttleStore: store,
    });
    const feud = {
      v: 1,
      left: "char:one",
      right: "char:two",
      tick: 42,
      place: "blood_valley",
    };

    await runtime.handlePayload(SOCIAL_FEUD, JSON.stringify(feud));
    await runtime.handlePayload(SOCIAL_FEUD, JSON.stringify({ ...feud, tick: 43 }));
    await runtime.handlePayload(
      SOCIAL_NICHE_INTRUSION,
      JSON.stringify({ v: 1, niche_pos: [1, 64, 2], intruder_id: "char:raider", items_taken: [], taint_delta: 0.1 }),
    );

    expect(pub.published).toHaveLength(2);
    expect(runtime.stats.throttled).toBe(1);
  });

  it("reserves throttle before the first publish completes", async () => {
    const pub = new FakePubSub();
    const runtime = new PoliticalNarrationRuntime({
      sub: new FakePubSub(),
      pub,
      logger: silent,
      now: () => 50_000,
    });
    const feud = {
      v: 1,
      left: "char:one",
      right: "char:two",
      tick: 42,
      place: "blood_valley",
    };

    const first = runtime.handlePayload(SOCIAL_FEUD, JSON.stringify(feud));
    await runtime.handlePayload(SOCIAL_FEUD, JSON.stringify({ ...feud, tick: 43 }));
    await first;

    expect(pub.published).toHaveLength(1);
    expect(runtime.stats.throttled).toBe(1);
  });

  it("lets higher severity same-zone events supersede pending ordinary narration", async () => {
    const pub = new FakePubSub();
    const runtime = new PoliticalNarrationRuntime({
      sub: new FakePubSub(),
      pub,
      logger: silent,
      now: () => 50_000,
    });
    const feud = {
      v: 1,
      left: "char:one",
      right: "char:two",
      tick: 42,
      place: "blood_valley",
    };

    const first = runtime.handlePayload(SOCIAL_FEUD, JSON.stringify(feud));
    await runtime.handlePayload(
      HIGH_RENOWN_MILESTONE,
      JSON.stringify({
        v: 1,
        event: "high_renown_milestone",
        player_uuid: "7a8f80c2-82ad-5d7c-a0dd-b3c1b7d2e1a1",
        char_id: "offline:kiz",
        identity_id: 0,
        identity_display_name: "玄锋",
        fame: 500,
        milestone: 500,
        identity_exposed: true,
        tick: 43,
        zone: "blood_valley",
      }),
    );
    await first;

    expect(pub.published).toHaveLength(1);
    const narration = JSON.parse(pub.published[0].message).narrations[0];
    expect(narration.target).toBe("blood_valley");
    expect(narration.text).toContain("500");
    expect(runtime.stats.throttled).toBe(1);
  });

  it("allows ordinary zone narration after five minutes", async () => {
    let now = 1_000;
    const pub = new FakePubSub();
    const runtime = new PoliticalNarrationRuntime({
      sub: new FakePubSub(),
      pub,
      logger: silent,
      now: () => now,
    });

    const payload = JSON.stringify({
      v: 1,
      left: "char:one",
      right: "char:two",
      tick: 42,
      place: "blood_valley",
    });
    await runtime.handlePayload(SOCIAL_FEUD, payload);
    now += POLITICAL_THROTTLE_MS;
    await runtime.handlePayload(SOCIAL_FEUD, payload);

    expect(pub.published).toHaveLength(2);
  });

  it("keeps unexposed high renown milestone anonymous in fallback", () => {
    const narration = renderPoliticalNarration({
      eventType: "high_renown_milestone",
      scope: "zone",
      target: "spawn",
      zone: "spawn",
      severity: 2,
      bypassThrottle: false,
      identityExposed: false,
      exposedIdentities: [],
      unexposedIdentities: ["玄锋"],
      payload: {
        v: 1,
        event: "high_renown_milestone",
        player_uuid: "7a8f80c2-82ad-5d7c-a0dd-b3c1b7d2e1a1",
        char_id: "offline:kiz",
        identity_id: 0,
        identity_display_name: "玄锋",
        fame: 100,
        milestone: 100,
        identity_exposed: false,
        tick: 4,
      },
    });

    expect(narration.text).toContain("某修士");
    expect(narration.text).not.toContain("玄锋");
  });

  it("falls back when LLM output names an unexposed identity", async () => {
    const pub = new FakePubSub();
    const runtime = new PoliticalNarrationRuntime({
      sub: new FakePubSub(),
      pub,
      logger: silent,
      llm: {
        chat: vi.fn(async () =>
          JSON.stringify({
            text: "江湖有传，玄锋之名已在市井流传，旧账未清，后势仍藏灯下。",
            scope: "zone",
            target: "spawn",
            style: "political_jianghu",
            kind: "political_jianghu",
          }),
        ),
      },
    });

    await runtime.handlePayload(
      HIGH_RENOWN_MILESTONE,
      JSON.stringify({
        v: 1,
        event: "high_renown_milestone",
        player_uuid: "7a8f80c2-82ad-5d7c-a0dd-b3c1b7d2e1a1",
        char_id: "offline:kiz",
        identity_id: 0,
        identity_display_name: "玄锋",
        fame: 100,
        milestone: 100,
        identity_exposed: false,
        tick: 4,
        zone: "spawn",
      }),
    );

    expect(pub.published).toHaveLength(1);
    const narration = JSON.parse(pub.published[0].message).narrations[0];
    expect(narration.text).toContain("某修士");
    expect(narration.text).not.toContain("玄锋");
    expect(runtime.stats.fallbackUsed).toBe(1);
  });

  it("falls back when LLM output uses modern political terms", async () => {
    const pub = new FakePubSub();
    const runtime = new PoliticalNarrationRuntime({
      sub: new FakePubSub(),
      pub,
      logger: silent,
      llm: {
        chat: vi.fn(async () =>
          JSON.stringify({
            text: "江湖有传，玄锋在山中立政府与议会，市井消息一夜传开。",
            scope: "broadcast",
            style: "political_jianghu",
            kind: "political_jianghu",
          }),
        ),
      },
    });

    await runtime.handlePayload(
      HIGH_RENOWN_MILESTONE,
      JSON.stringify({
        v: 1,
        event: "high_renown_milestone",
        player_uuid: "7a8f80c2-82ad-5d7c-a0dd-b3c1b7d2e1a1",
        char_id: "offline:kiz",
        identity_id: 0,
        identity_display_name: "玄锋",
        fame: 1000,
        milestone: 1000,
        identity_exposed: true,
        tick: 4,
        zone: "spawn",
      }),
    );

    expect(pub.published).toHaveLength(1);
    const narration = JSON.parse(pub.published[0].message).narrations[0];
    expect(narration.text).not.toContain("政府");
    expect(narration.text).not.toContain("议会");
    expect(runtime.stats.fallbackUsed).toBe(1);
  });

  it("falls back when LLM output lacks jianghu voice", async () => {
    const pub = new FakePubSub();
    const runtime = new PoliticalNarrationRuntime({
      sub: new FakePubSub(),
      pub,
      logger: silent,
      llm: {
        chat: vi.fn(async () =>
          JSON.stringify({
            text: "两名修士建立了长期合作关系，事件影响范围有限，后续发展仍需观察。",
            scope: "zone",
            target: "jianghu",
            style: "political_jianghu",
            kind: "political_jianghu",
          }),
        ),
      },
    });

    await runtime.handlePayload(
      SOCIAL_PACT,
      JSON.stringify({ v: 1, left: "char:a", right: "char:b", terms: "同行", tick: 1, broken: false }),
    );

    expect(pub.published).toHaveLength(1);
    const narration = JSON.parse(pub.published[0].message).narrations[0];
    expect(narration.text).toMatch(/江湖|市井|山中|传/);
    expect(narration.text).not.toContain("长期合作关系");
    expect(runtime.stats.fallbackUsed).toBe(1);
  });

  it("does not record throttle when publish fails", async () => {
    const pub = new FlakyPubSub();
    const runtime = new PoliticalNarrationRuntime({
      sub: new FakePubSub(),
      pub,
      logger: silent,
      now: () => 50_000,
    });
    const feud = {
      v: 1,
      left: "char:one",
      right: "char:two",
      tick: 42,
      place: "blood_valley",
    };

    await runtime.handlePayload(SOCIAL_FEUD, JSON.stringify(feud));
    await runtime.handlePayload(SOCIAL_FEUD, JSON.stringify({ ...feud, tick: 43 }));

    expect(pub.published).toHaveLength(1);
    expect(runtime.stats.published).toBe(1);
    expect(runtime.stats.throttled).toBe(0);
  });
});
