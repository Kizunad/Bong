import { describe, expect, it, vi } from "vitest";
import { RedisIpc, WORLD_MODEL_STATE_FIELDS, WORLD_MODEL_STATE_KEY } from "../src/redis-ipc.js";
import { CHANNELS } from "@bong/schema";

const {
  AGENT_COMMAND,
  AGENT_NARRATE,
  AGENT_WORLD_MODEL,
  ALCHEMY_INSIGHT,
  ALCHEMY_SESSION_END,
  FACTION_EVENT,
  AGING,
  BOTANY_ECOLOGY,
  BREAKTHROUGH_EVENT,
  COMBAT_REALTIME,
  ZONE_PRESSURE_CROSSED,
  PSEUDO_VEIN_ACTIVE,
  PSEUDO_VEIN_DISSIPATE,
  FORGE_OUTCOME,
  REBIRTH,
  SKILL_XP_GAIN,
  SOCIAL_FEUD,
  SOCIAL_NICHE_INTRUSION,
  SPIRIT_EYE_DISCOVERED,
  SPIRIT_EYE_MIGRATE,
  SPIRIT_EYE_USED_FOR_BREAKTHROUGH,
  NPC_DEATH,
  NPC_SPAWN,
  PLAYER_CHAT,
  POI_NOVICE_EVENT,
  PRICE_INDEX,
  TSY_EVENT,
  WORLD_STATE,
} = CHANNELS;

interface FakeMultiResult {
  lrange: string[];
  writesDuringExec?: string[];
}

class FakeRedisListClient {
  private readonly lists = new Map<string, string[]>();
  private readonly subscribers = new Map<string, Array<(channel: string, message: string) => void>>();
  private readonly hashes = new Map<string, Record<string, string>>();
  private readonly published: Array<{ channel: string; message: string }> = [];
  private nextMultiResult: FakeMultiResult | null = null;

  async subscribe(channel: string): Promise<number> {
    if (!this.subscribers.has(channel)) {
      this.subscribers.set(channel, []);
    }
    return 1;
  }

  on(event: "message", listener: (channel: string, message: string) => void): this {
    if (event !== "message") {
      return this;
    }
    for (const channel of this.subscribers.keys()) {
      const listeners = this.subscribers.get(channel) ?? [];
      listeners.push(listener);
      this.subscribers.set(channel, listeners);
    }
    return this;
  }

  off(event: "message", listener: (channel: string, message: string) => void): this {
    if (event !== "message") {
      return this;
    }
    for (const channel of this.subscribers.keys()) {
      const listeners = this.subscribers.get(channel) ?? [];
      this.subscribers.set(
        channel,
        listeners.filter((current) => current !== listener),
      );
    }
    return this;
  }

  async unsubscribe(): Promise<number> {
    this.subscribers.clear();
    return 0;
  }

  disconnect(): void {}

  async publish(channel: string, message: string): Promise<number> {
    this.published.push({ channel, message });
    const listeners = this.subscribers.get(channel) ?? [];
    for (const listener of listeners) {
      listener(channel, message);
    }
    return listeners.length;
  }

  async hgetall(key: string): Promise<Record<string, string>> {
    return { ...(this.hashes.get(key) ?? {}) };
  }

  async hset(key: string, values: Record<string, string>): Promise<number> {
    this.hashes.set(key, { ...values });
    return Object.keys(values).length;
  }

  multi(): {
    lrange: (key: string, start: number, stop: number) => ReturnType<FakeRedisListClient["multi"]>;
    ltrim: (key: string, start: number, stop: number) => ReturnType<FakeRedisListClient["multi"]>;
    exec: () => Promise<[[null, string[]], [null, "OK"]]>;
  } {
    let lrangeKey = "";
    let lrangeStart = 0;
    let lrangeStop = -1;
    let ltrimKey = "";
    let ltrimStart = 0;

    const chain = {
      lrange: (key: string, start: number, stop: number) => {
        lrangeKey = key;
        lrangeStart = start;
        lrangeStop = stop;
        return chain;
      },
      ltrim: (key: string, start: number, _stop: number) => {
        ltrimKey = key;
        ltrimStart = start;
        return chain;
      },
      exec: async () => {
        const state = this.nextMultiResult;
        this.nextMultiResult = null;

        const list = [...(this.lists.get(lrangeKey) ?? [])];
        const stop = lrangeStop < 0 ? list.length - 1 : lrangeStop;
        const selected = list.slice(lrangeStart, stop + 1);

        const writers = state?.writesDuringExec ?? [];
        if (writers.length > 0) {
          const current = this.lists.get(lrangeKey) ?? [];
          this.lists.set(lrangeKey, [...current, ...writers]);
        }

        const latest = this.lists.get(ltrimKey) ?? [];
        this.lists.set(ltrimKey, latest.slice(ltrimStart));

        return [[null, state?.lrange ?? selected], [null, "OK"]] as [[null, string[]], [null, "OK"]];
      },
    };

    return chain;
  }

  setList(key: string, values: string[]): void {
    this.lists.set(key, [...values]);
  }

  getList(key: string): string[] {
    return [...(this.lists.get(key) ?? [])];
  }

  setNextMultiResult(result: FakeMultiResult): void {
    this.nextMultiResult = result;
  }

  setHash(key: string, value: Record<string, string>): void {
    this.hashes.set(key, { ...value });
  }

  getHash(key: string): Record<string, string> {
    return { ...(this.hashes.get(key) ?? {}) };
  }

  getPublished(channel?: string): Array<{ channel: string; message: string }> {
    if (channel === undefined) {
      return [...this.published];
    }
    return this.published.filter((entry) => entry.channel === channel);
  }

  getSubscribedChannels(): string[] {
    return [...this.subscribers.keys()];
  }
}

describe("redis-ipc", () => {
  it("drains player chat via atomic LRANGE/LTRIM window", async () => {
    const pub = new FakeRedisListClient();
    const sub = new FakeRedisListClient();

    pub.setList(PLAYER_CHAT, [
      JSON.stringify({ v: 1, ts: 1, player: "offline:Steve", raw: "a", zone: "spawn" }),
      JSON.stringify({ v: 1, ts: 2, player: "offline:Alex", raw: "b", zone: "spawn" }),
      JSON.stringify({ v: 1, ts: 3, player: "offline:Eve", raw: "c", zone: "spawn" }),
    ]);

    const createClient = vi
      .fn<(url: string) => FakeRedisListClient>()
      .mockReturnValueOnce(sub)
      .mockReturnValueOnce(pub);

    const ipc = new RedisIpc(
      { url: "redis://fake" },
      {
        createClient,
      },
    );

    await ipc.connect();
    const drained = await ipc.drainPlayerChat({ maxItems: 2, logger: { warn: vi.fn() } });

    expect(drained).toHaveLength(2);
    expect(drained.map((m) => m.raw)).toEqual(["a", "b"]);
    expect(pub.getList(PLAYER_CHAT).map((s) => JSON.parse(s).raw)).toEqual(["c"]);
  });

  it("does not lose concurrent writes happening during drain transaction", async () => {
    const pub = new FakeRedisListClient();
    const sub = new FakeRedisListClient();

    pub.setList(PLAYER_CHAT, [
      JSON.stringify({ v: 1, ts: 1, player: "offline:Steve", raw: "first", zone: "spawn" }),
      JSON.stringify({ v: 1, ts: 2, player: "offline:Alex", raw: "second", zone: "spawn" }),
    ]);

    pub.setNextMultiResult({
      lrange: [
        JSON.stringify({ v: 1, ts: 1, player: "offline:Steve", raw: "first", zone: "spawn" }),
      ],
      writesDuringExec: [
        JSON.stringify({ v: 1, ts: 3, player: "offline:Eve", raw: "concurrent", zone: "spawn" }),
      ],
    });

    const createClient = vi
      .fn<(url: string) => FakeRedisListClient>()
      .mockReturnValueOnce(sub)
      .mockReturnValueOnce(pub);

    const ipc = new RedisIpc(
      { url: "redis://fake" },
      {
        createClient,
      },
    );

    await ipc.connect();
    const drained = await ipc.drainPlayerChat({ maxItems: 1, logger: { warn: vi.fn() } });

    expect(drained).toHaveLength(1);
    expect(drained[0]?.raw).toBe("first");

    const remaining = pub.getList(PLAYER_CHAT).map((s) => JSON.parse(s).raw);
    expect(remaining).toEqual(["second", "concurrent"]);
  });

  it("updates latest state from world_state channel", async () => {
    const pub = new FakeRedisListClient();
    const sub = new FakeRedisListClient();

    const createClient = vi
      .fn<(url: string) => FakeRedisListClient>()
      .mockReturnValueOnce(sub)
      .mockReturnValueOnce(pub);

    const ipc = new RedisIpc(
      { url: "redis://fake" },
      {
        createClient,
      },
    );

    await ipc.connect();

    await sub.publish(
      WORLD_STATE,
      JSON.stringify({
        v: 1,
        ts: 1,
        tick: 1,
        players: [],
        npcs: [],
        zones: [],
        recent_events: [],
      }),
    );

    expect(ipc.getLatestState()?.tick).toBe(1);
  });

  it("observes TSY hostile events from the shared TSY event channel", async () => {
    const pub = new FakeRedisListClient();
    const sub = new FakeRedisListClient();

    const createClient = vi
      .fn<(url: string) => FakeRedisListClient>()
      .mockReturnValueOnce(sub)
      .mockReturnValueOnce(pub);

    const ipc = new RedisIpc(
      { url: "redis://fake" },
      {
        createClient,
      },
    );
    const callback = vi.fn();
    ipc.onTsyHostileEvent(callback);

    await ipc.connect();
    await sub.publish(
      TSY_EVENT,
      JSON.stringify({
        v: 1,
        kind: "tsy_npc_spawned",
        family_id: "tsy_zongmen_yiji_01",
        archetype: "guardian_relic_sentinel",
        count: 3,
        at_tick: 12000,
      }),
    );
    await sub.publish(
      TSY_EVENT,
      JSON.stringify({
        v: 1,
        kind: "tsy_sentinel_phase_changed",
        family_id: "tsy_zongmen_yiji_01",
        container_entity_id: 42,
        phase: 1,
        max_phase: 3,
        at_tick: 12345,
      }),
    );

    expect(callback).toHaveBeenCalledTimes(2);
    expect(ipc.getLatestTsyHostileEvents()).toEqual([
      expect.objectContaining({ kind: "tsy_npc_spawned", count: 3 }),
      expect.objectContaining({ kind: "tsy_sentinel_phase_changed", phase: 1 }),
    ]);
  });

  it("observes NPC runtime events from dedicated channels", async () => {
    const pub = new FakeRedisListClient();
    const sub = new FakeRedisListClient();

    const createClient = vi
      .fn<(url: string) => FakeRedisListClient>()
      .mockReturnValueOnce(sub)
      .mockReturnValueOnce(pub);

    const ipc = new RedisIpc(
      { url: "redis://fake" },
      {
        createClient,
      },
    );
    const callback = vi.fn();
    ipc.onNpcRuntimeEvent(callback);

    await ipc.connect();
    await sub.publish(
      NPC_SPAWN,
      JSON.stringify({
        v: 1,
        kind: "npc_spawned",
        npc_id: "npc_1v1",
        archetype: "rogue",
        source: "agent_command",
        zone: "spawn",
        pos: [1, 66, 2],
        initial_age_ticks: 0,
        at_tick: 0,
      }),
    );
    await sub.publish(
      NPC_DEATH,
      JSON.stringify({
        v: 1,
        kind: "npc_death",
        npc_id: "npc_1v1",
        archetype: "rogue",
        cause: "combat",
        age_ticks: 1,
        max_age_ticks: 100,
        at_tick: 1,
      }),
    );
    await sub.publish(
      FACTION_EVENT,
      JSON.stringify({
        v: 1,
        kind: "faction_event",
        faction_id: "attack",
        event_kind: "adjust_loyalty_bias",
        loyalty_bias: 0.6,
        mission_queue_size: 1,
        at_tick: 2,
      }),
    );

    expect(callback).toHaveBeenCalledTimes(3);
    expect(ipc.getLatestNpcEvents()).toEqual([
      expect.objectContaining({ kind: "npc_spawned", npc_id: "npc_1v1" }),
      expect.objectContaining({ kind: "npc_death", cause: "combat" }),
      expect.objectContaining({ kind: "faction_event", faction_id: "attack" }),
    ]);
  });

  it("observes alchemy session_end events for narration triggers", async () => {
    const pub = new FakeRedisListClient();
    const sub = new FakeRedisListClient();

    const createClient = vi
      .fn<(url: string) => FakeRedisListClient>()
      .mockReturnValueOnce(sub)
      .mockReturnValueOnce(pub);

    const ipc = new RedisIpc(
      { url: "redis://fake" },
      {
        createClient,
      },
    );
    const callback = vi.fn();
    ipc.onAlchemyRuntimeEvent(callback);

    await ipc.connect();
    await sub.publish(
      ALCHEMY_SESSION_END,
      JSON.stringify({
        v: 1,
        session_id: "alchemy:-12:64:38:offline:Azure:kai_mai_pill_v0",
        recipe_id: "kai_mai_pill_v0",
        furnace_pos: [-12, 64, 38],
        furnace_tier: 1,
        caster_id: "offline:Azure",
        bucket: "explode",
        damage: 12,
        meridian_crack: 0.2,
        elapsed_ticks: 120,
        ts: 84120,
      }),
    );

    expect(callback).toHaveBeenCalledTimes(1);
    expect(ipc.getLatestAlchemyEvents()).toEqual([
      expect.objectContaining({
        bucket: "explode",
        caster_id: "offline:Azure",
        damage: 12,
      }),
    ]);
  });

  it("observes alchemy insight events for narration triggers", async () => {
    const pub = new FakeRedisListClient();
    const sub = new FakeRedisListClient();

    const createClient = vi
      .fn<(url: string) => FakeRedisListClient>()
      .mockReturnValueOnce(sub)
      .mockReturnValueOnce(pub);

    const ipc = new RedisIpc(
      { url: "redis://fake" },
      {
        createClient,
      },
    );
    const callback = vi.fn();
    ipc.onAlchemyRuntimeEvent(callback);

    await ipc.connect();
    await sub.publish(
      ALCHEMY_INSIGHT,
      JSON.stringify({
        v: 1,
        player_id: "offline:Azure",
        source_pill: "hui_yuan_pill",
        recipe_id: "hui_yuan_pill_v0",
        accuracy: 0.86,
        ingredients: ["ling_grass", "qingxin_leaf"],
      }),
    );

    expect(callback).toHaveBeenCalledTimes(1);
    expect(ipc.getLatestAlchemyEvents()).toEqual([
      expect.objectContaining({
        player_id: "offline:Azure",
        accuracy: 0.86,
      }),
    ]);
  });

  it("observes valid botany ecology snapshots and skips invalid payloads", async () => {
    const pub = new FakeRedisListClient();
    const sub = new FakeRedisListClient();
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);

    const createClient = vi
      .fn<(url: string) => FakeRedisListClient>()
      .mockReturnValueOnce(sub)
      .mockReturnValueOnce(pub);

    const ipc = new RedisIpc(
      { url: "redis://fake" },
      {
        createClient,
      },
    );
    const callback = vi.fn();
    ipc.onBotanyEcology(callback);

    await ipc.connect();
    await sub.publish(
      BOTANY_ECOLOGY,
      JSON.stringify({
        v: 1,
        tick: 84,
        zones: [
          {
            zone: "starter_zone",
            spirit_qi: 0.12,
            plant_counts: [{ kind: "ning_mai_cao", count: 12 }],
            variant_counts: [{ variant: "tainted", count: 4 }],
          },
        ],
      }),
    );
    await sub.publish(
      BOTANY_ECOLOGY,
      JSON.stringify({ v: 1, tick: 85, zones: [{ zone: "bad", spirit_qi: 2 }] }),
    );

    expect(callback).toHaveBeenCalledTimes(1);
    expect(ipc.drainBotanyEcologyEvents()).toEqual([
      expect.objectContaining({
        tick: 84,
        zones: [expect.objectContaining({ zone: "starter_zone" })],
      }),
    ]);
    expect(ipc.drainBotanyEcologyEvents()).toEqual([]);
    expect(warn).toHaveBeenCalledWith(
      "[redis-ipc] invalid botany ecology snapshot:",
      expect.stringContaining("spirit_qi"),
    );

    warn.mockRestore();
  });

  it("observes price index events from the economy channel", async () => {
    const pub = new FakeRedisListClient();
    const sub = new FakeRedisListClient();
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});

    const createClient = vi
      .fn<(url: string) => FakeRedisListClient>()
      .mockReturnValueOnce(sub)
      .mockReturnValueOnce(pub);

    const ipc = new RedisIpc(
      { url: "redis://fake" },
      {
        createClient,
      },
    );
    const callback = vi.fn();
    ipc.onPriceIndex(callback);

    await ipc.connect();
    await sub.publish(
      PRICE_INDEX,
      JSON.stringify({
        v: 1,
        tick: 720_000,
        season: "summer_to_winter",
        supply_spirit_qi: 27.5,
        demand_spirit_qi: 50,
        rhythm_multiplier: 1.1,
        market_factor: 0.9,
        price_multiplier: 0.99,
        sample_prices: [{ item_id: "common_good", base_price: 4, final_price: 4 }],
      }),
    );
    await sub.publish(
      PRICE_INDEX,
      JSON.stringify({ v: 1, tick: 1, season: "bad", supply_spirit_qi: 1 }),
    );

    expect(callback).toHaveBeenCalledTimes(1);
    expect(ipc.drainPriceIndexEvents()).toEqual([
      expect.objectContaining({
        tick: 720_000,
        season: "summer_to_winter",
      }),
    ]);
    expect(ipc.drainPriceIndexEvents()).toEqual([]);
    expect(warn).toHaveBeenCalledWith(
      "[redis-ipc] invalid price index event:",
      expect.stringContaining("season"),
    );

    warn.mockRestore();
  });

  it("observes zone pressure crossed events from the dedicated channel", async () => {
    const pub = new FakeRedisListClient();
    const sub = new FakeRedisListClient();

    const createClient = vi
      .fn<(url: string) => FakeRedisListClient>()
      .mockReturnValueOnce(sub)
      .mockReturnValueOnce(pub);

    const ipc = new RedisIpc(
      { url: "redis://fake" },
      {
        createClient,
      },
    );
    const callback = vi.fn();
    ipc.onZonePressureCrossed(callback);

    await ipc.connect();
    await sub.publish(
      ZONE_PRESSURE_CROSSED,
      JSON.stringify({
        v: 1,
        kind: "zone_pressure_crossed",
        zone: "starter_zone",
        level: "high",
        raw_pressure: 1.1,
        at_tick: 1440,
      }),
    );

    expect(callback).toHaveBeenCalledTimes(1);
    expect(ipc.drainZonePressureCrossedEvents()).toEqual([
      expect.objectContaining({ zone: "starter_zone", level: "high", raw_pressure: 1.1 }),
    ]);
    expect(ipc.drainZonePressureCrossedEvents()).toEqual([]);
  });

  it("observes novice POI events for narration triggers", async () => {
    const pub = new FakeRedisListClient();
    const sub = new FakeRedisListClient();

    const createClient = vi
      .fn<(url: string) => FakeRedisListClient>()
      .mockReturnValueOnce(sub)
      .mockReturnValueOnce(pub);

    const ipc = new RedisIpc(
      { url: "redis://fake" },
      {
        createClient,
      },
    );
    const callback = vi.fn();
    ipc.onPoiNoviceEvent(callback);

    await ipc.connect();
    await sub.publish(
      POI_NOVICE_EVENT,
      JSON.stringify({
        v: 1,
        kind: "poi_spawned",
        poi_id: "spawn:forge_station",
        poi_type: "forge_station",
        zone: "spawn",
        pos: [304, 71, 208],
        selection_strategy: "strict_radius_1500",
        qi_affinity: 0.15,
        danger_bias: 0,
      }),
    );
    await sub.publish(
      POI_NOVICE_EVENT,
      JSON.stringify({
        v: 1,
        kind: "trespass",
        village_id: "spawn:rogue_village",
        player_id: "offline:Azure",
        killed_npc_count: 3,
        refusal_until_wall_clock_secs: 1770000000,
      }),
    );

    expect(callback).toHaveBeenCalledTimes(2);
    expect(ipc.getLatestPoiNoviceEvents()).toEqual([
      expect.objectContaining({ kind: "poi_spawned", poi_type: "forge_station" }),
      expect.objectContaining({ kind: "trespass", village_id: "spawn:rogue_village" }),
    ]);
  });

  it("subscribes and buffers cross-system runtime events", async () => {
    const pub = new FakeRedisListClient();
    const sub = new FakeRedisListClient();

    const createClient = vi
      .fn<(url: string) => FakeRedisListClient>()
      .mockReturnValueOnce(sub)
      .mockReturnValueOnce(pub);

    const ipc = new RedisIpc(
      { url: "redis://fake" },
      {
        createClient,
      },
    );
    const callback = vi.fn();
    ipc.onCrossSystemEvent(callback);

    await ipc.connect();

    expect(sub.getSubscribedChannels()).toEqual(
      expect.arrayContaining([
        BOTANY_ECOLOGY,
        ZONE_PRESSURE_CROSSED,
        AGING,
        BREAKTHROUGH_EVENT,
        SOCIAL_FEUD,
        SOCIAL_NICHE_INTRUSION,
        COMBAT_REALTIME,
        PSEUDO_VEIN_ACTIVE,
        PSEUDO_VEIN_DISSIPATE,
        FORGE_OUTCOME,
        REBIRTH,
        SKILL_XP_GAIN,
        SPIRIT_EYE_MIGRATE,
        SPIRIT_EYE_DISCOVERED,
        SPIRIT_EYE_USED_FOR_BREAKTHROUGH,
      ]),
    );

    await sub.publish(
      BOTANY_ECOLOGY,
      JSON.stringify({
        v: 1,
        tick: 84,
        zones: [{ zone: "spawn", spirit_qi: 0.2, plant_counts: [], variant_counts: [] }],
      }),
    );
    await sub.publish(
      AGING,
      JSON.stringify({ v: 1, character_id: "offline:Azure", at_tick: 85, kind: "tick_rate" }),
    );
    await sub.publish(SOCIAL_FEUD, JSON.stringify({ v: 1, left: "char:a", right: "char:b", tick: 86 }));
    await sub.publish(
      SOCIAL_NICHE_INTRUSION,
      JSON.stringify({
        v: 1,
        type: "niche_intrusion",
        niche_pos: [1, 64, 2],
        intruder_id: "char:raider",
        items_taken: [41],
        taint_delta: 0.2,
      }),
    );
    await sub.publish(SKILL_XP_GAIN, JSON.stringify({ v: 1, char_id: 1, skill: "herbalism", amount: 2 }));
    await sub.publish(
      PSEUDO_VEIN_ACTIVE,
      JSON.stringify({
        v: 1,
        id: "pseudo_vein_42",
        center_xz: [1280, -640],
        spirit_qi_current: 0.3,
        occupants: ["offline:Azure"],
        spawned_at_tick: 1,
        estimated_decay_at_tick: 2,
        season_at_spawn: "summer_to_winter",
      }),
    );
    await sub.publish(
      PSEUDO_VEIN_DISSIPATE,
      JSON.stringify({
        v: 1,
        id: "pseudo_vein_42",
        center_xz: [1280, -640],
        storm_anchors: [[1380, -650]],
        storm_duration_ticks: 9000,
        qi_redistribution: { refill_to_hungry_ring: 0.7, collected_by_tiandao: 0.3 },
      }),
    );
    await sub.publish(
      SPIRIT_EYE_MIGRATE,
      JSON.stringify({
        v: 1,
        eye_id: "eye_spawn_0",
        from: { x: 120, y: 80, z: -30 },
        to: { x: 920, y: 88, z: -640 },
        reason: "usage_pressure",
        usage_pressure: 1.1,
        tick: 90,
      }),
    );
    await sub.publish(
      SPIRIT_EYE_DISCOVERED,
      JSON.stringify({
        v: 1,
        eye_id: "eye_spawn_0",
        character_id: "offline:Azure",
        pos: { x: 920, y: 88, z: -640 },
        zone: "qingyun_peaks",
        qi_concentration: 1,
        discovered_at_tick: 91,
      }),
    );
    await sub.publish(
      SPIRIT_EYE_USED_FOR_BREAKTHROUGH,
      JSON.stringify({
        v: 1,
        eye_id: "eye_spawn_0",
        character_id: "offline:Azure",
        realm_from: "Condense",
        realm_to: "Foundation",
        usage_pressure: 0.2,
        tick: 92,
      }),
    );

    expect(callback).toHaveBeenCalledTimes(10);
    expect(ipc.getLatestCrossSystemEvents()).toEqual([
      expect.objectContaining({ channel: BOTANY_ECOLOGY, payload: expect.objectContaining({ tick: 84 }) }),
      expect.objectContaining({ channel: AGING, payload: expect.objectContaining({ character_id: "offline:Azure" }) }),
      expect.objectContaining({ channel: SOCIAL_FEUD, payload: expect.objectContaining({ left: "char:a" }) }),
      expect.objectContaining({
        channel: SOCIAL_NICHE_INTRUSION,
        payload: expect.objectContaining({ type: "niche_intrusion", intruder_id: "char:raider" }),
      }),
      expect.objectContaining({ channel: SKILL_XP_GAIN, payload: expect.objectContaining({ skill: "herbalism" }) }),
      expect.objectContaining({ channel: PSEUDO_VEIN_ACTIVE, payload: expect.objectContaining({ id: "pseudo_vein_42" }) }),
      expect.objectContaining({
        channel: PSEUDO_VEIN_DISSIPATE,
        payload: expect.objectContaining({ storm_duration_ticks: 9000 }),
      }),
      expect.objectContaining({ channel: SPIRIT_EYE_MIGRATE, payload: expect.objectContaining({ eye_id: "eye_spawn_0" }) }),
      expect.objectContaining({
        channel: SPIRIT_EYE_DISCOVERED,
        payload: expect.objectContaining({ character_id: "offline:Azure" }),
      }),
      expect.objectContaining({
        channel: SPIRIT_EYE_USED_FOR_BREAKTHROUGH,
        payload: expect.objectContaining({ realm_to: "Foundation" }),
      }),
    ]);
  });

  it("publishes spawn_npc commands through the existing agent command channel", async () => {
    const pub = new FakeRedisListClient();
    const sub = new FakeRedisListClient();

    const createClient = vi
      .fn<(url: string) => FakeRedisListClient>()
      .mockReturnValueOnce(sub)
      .mockReturnValueOnce(pub);

    const ipc = new RedisIpc(
      { url: "redis://fake" },
      {
        createClient,
      },
    );

    await ipc.publishCommands({
      source: "arbiter",
      commands: [{ type: "spawn_npc", target: "starter_zone", params: { archetype: "zombie" } }],
      metadata: { sourceTick: 123, correlationId: "corr_spawn_npc" },
    });

    const published = pub.getPublished();
    expect(published).toHaveLength(1);
    const [publishedBatch] = published;
    expect(publishedBatch?.channel).toBe(CHANNELS.AGENT_COMMAND);
    expect(JSON.parse(publishedBatch?.message ?? "{}")).toMatchObject({
      v: 1,
      source: "arbiter",
      commands: [{ type: "spawn_npc", target: "starter_zone", params: { archetype: "zombie" } }],
    });
  });

  it("keeps world_state callback execution single even if connect is retried without teardown", async () => {
    const pub = new FakeRedisListClient();
    const sub = new FakeRedisListClient();

    const createClient = vi
      .fn<(url: string) => FakeRedisListClient>()
      .mockReturnValueOnce(sub)
      .mockReturnValueOnce(pub);

    const ipc = new RedisIpc(
      { url: "redis://fake" },
      {
        createClient,
      },
    );

    const callback = vi.fn();
    ipc.onWorldState(callback);

    await ipc.connect();
    await sub.publish(
      WORLD_STATE,
      JSON.stringify({
        v: 1,
        ts: 1,
        tick: 1,
        players: [],
        npcs: [],
        zones: [],
        recent_events: [],
      }),
    );

    (ipc as unknown as { connected: boolean }).connected = false;
    await ipc.connect();
    await sub.publish(
      WORLD_STATE,
      JSON.stringify({
        v: 1,
        ts: 2,
        tick: 2,
        players: [],
        npcs: [],
        zones: [],
        recent_events: [],
      }),
    );

    expect(callback).toHaveBeenCalledTimes(2);
    expect(ipc.getLatestState()?.tick).toBe(2);
  });

  it("keeps agent command/narration channels unchanged", async () => {
    const pub = new FakeRedisListClient();
    const sub = new FakeRedisListClient();

    const createClient = vi
      .fn<(url: string) => FakeRedisListClient>()
      .mockReturnValueOnce(sub)
      .mockReturnValueOnce(pub);

    const ipc = new RedisIpc(
      { url: "redis://fake" },
      {
        createClient,
      },
    );

    await ipc.publishCommands({
      source: "arbiter",
      commands: [
        {
          type: "modify_zone",
          target: "starter_zone",
          params: { spirit_qi_delta: 0.02 },
        },
      ],
      metadata: { sourceTick: 10, correlationId: "tick-10" },
    });
    await ipc.publishNarrations({
      narrations: [
        {
          scope: "broadcast",
          text: "天地异动",
          style: "narration",
        },
      ],
      metadata: { sourceTick: 10, correlationId: "tick-10" },
    });

    expect(pub.getPublished(AGENT_COMMAND)).toHaveLength(1);
    expect(pub.getPublished(AGENT_NARRATE)).toHaveLength(1);
    expect(pub.getPublished(AGENT_WORLD_MODEL)).toHaveLength(0);
  });

  it("publishes world model on dedicated bong:agent_world_model channel", async () => {
    const pub = new FakeRedisListClient();
    const sub = new FakeRedisListClient();

    const createClient = vi
      .fn<(url: string) => FakeRedisListClient>()
      .mockReturnValueOnce(sub)
      .mockReturnValueOnce(pub);

    const ipc = new RedisIpc(
      { url: "redis://fake" },
      {
        createClient,
      },
    );

    await ipc.publishAgentWorldModel({
      source: "arbiter",
      metadata: { sourceTick: 123, correlationId: "tiandao-tick-123" },
      snapshot: {
        currentEra: {
          name: "末法纪",
          sinceTick: 120,
          globalEffect: "灵机渐枯",
        },
        zoneHistory: {
          blood_valley: [
            {
              name: "blood_valley",
              spirit_qi: 0.4,
              danger_level: 2,
              active_events: ["tribulation"],
              player_count: 3,
            },
          ],
        },
        lastDecisions: {
          mutation: {
            commands: [],
            narrations: [],
            reasoning: "ok",
          },
        },
        playerFirstSeenTick: {
          "offline:Elder": 88,
        },
        lastTick: 123,
        lastStateTs: 1710000100,
      },
    });

    const published = pub.getPublished(AGENT_WORLD_MODEL);
    expect(published).toHaveLength(1);
    expect(pub.getPublished(AGENT_COMMAND)).toHaveLength(0);
    expect(pub.getPublished(AGENT_NARRATE)).toHaveLength(0);

    const envelope = JSON.parse(published[0]?.message ?? "{}") as {
      v: number;
      id: string;
      source: string;
      snapshot: { lastTick: number | null; lastStateTs: number | null };
    };
    expect(envelope.v).toBe(1);
    expect(envelope.source).toBe("arbiter");
    expect(envelope.id).toContain("world_model_t123_arbiter_");
    expect(envelope.snapshot.lastTick).toBe(123);
    expect(envelope.snapshot.lastStateTs).toBe(1710000100);
  });

  it("loads world model snapshot from redis mirror hash", async () => {
    const pub = new FakeRedisListClient();
    const sub = new FakeRedisListClient();
    const logger = { warn: vi.fn() };

    pub.setHash(WORLD_MODEL_STATE_KEY, {
      [WORLD_MODEL_STATE_FIELDS.currentEra]: JSON.stringify({
        name: "末法纪",
        sinceTick: 188,
        globalEffect: "灵机渐枯",
      }),
      [WORLD_MODEL_STATE_FIELDS.zoneHistory]: JSON.stringify({
        blood_valley: [
          {
            name: "blood_valley",
            spirit_qi: 0.45,
            danger_level: 2,
            active_events: ["tribulation"],
            player_count: 3,
          },
        ],
      }),
      [WORLD_MODEL_STATE_FIELDS.lastDecisions]: JSON.stringify({
        mutation: {
          commands: [],
          narrations: [],
          reasoning: "ok",
        },
      }),
      [WORLD_MODEL_STATE_FIELDS.playerFirstSeenTick]: JSON.stringify({
        "offline:test-player": 188,
      }),
      [WORLD_MODEL_STATE_FIELDS.lastTick]: "188",
      [WORLD_MODEL_STATE_FIELDS.lastStateTs]: "",
    });

    const createClient = vi
      .fn<(url: string) => FakeRedisListClient>()
      .mockReturnValueOnce(sub)
      .mockReturnValueOnce(pub);

    const ipc = new RedisIpc(
      { url: "redis://fake" },
      {
        createClient,
      },
    );

    const snapshot = await ipc.loadWorldModelState({ logger });

    expect(snapshot).toEqual({
      currentEra: {
        name: "末法纪",
        sinceTick: 188,
        globalEffect: "灵机渐枯",
      },
      zoneHistory: {
        blood_valley: [
          {
            name: "blood_valley",
            spirit_qi: 0.45,
            danger_level: 2,
            active_events: ["tribulation"],
            player_count: 3,
          },
        ],
      },
      lastDecisions: {
        mutation: {
          commands: [],
          narrations: [],
          reasoning: "ok",
        },
      },
      playerFirstSeenTick: {
        "offline:test-player": 188,
      },
      lastTick: 188,
      lastStateTs: null,
    });
    expect(logger.warn).not.toHaveBeenCalled();
  });

  it("returns null when world model mirror hash is missing", async () => {
    const pub = new FakeRedisListClient();
    const sub = new FakeRedisListClient();
    const logger = { warn: vi.fn() };

    const createClient = vi
      .fn<(url: string) => FakeRedisListClient>()
      .mockReturnValueOnce(sub)
      .mockReturnValueOnce(pub);

    const ipc = new RedisIpc(
      { url: "redis://fake" },
      {
        createClient,
      },
    );

    const snapshot = await ipc.loadWorldModelState({ logger });
    expect(snapshot).toBeNull();
    expect(logger.warn).not.toHaveBeenCalled();
  });

  it("fails soft when world model mirror has malformed json", async () => {
    const pub = new FakeRedisListClient();
    const sub = new FakeRedisListClient();
    const logger = { warn: vi.fn() };

    pub.setHash(WORLD_MODEL_STATE_KEY, {
      [WORLD_MODEL_STATE_FIELDS.currentEra]: "{broken",
      [WORLD_MODEL_STATE_FIELDS.zoneHistory]: JSON.stringify({}),
      [WORLD_MODEL_STATE_FIELDS.lastDecisions]: JSON.stringify({}),
      [WORLD_MODEL_STATE_FIELDS.playerFirstSeenTick]: JSON.stringify({}),
      [WORLD_MODEL_STATE_FIELDS.lastTick]: "188",
      [WORLD_MODEL_STATE_FIELDS.lastStateTs]: "1711111188",
    });

    const createClient = vi
      .fn<(url: string) => FakeRedisListClient>()
      .mockReturnValueOnce(sub)
      .mockReturnValueOnce(pub);

    const ipc = new RedisIpc(
      { url: "redis://fake" },
      {
        createClient,
      },
    );

    const snapshot = await ipc.loadWorldModelState({ logger });
    expect(snapshot).toBeNull();
    expect(logger.warn).toHaveBeenCalled();
  });

  it("fails soft when world model mirror is missing required fields", async () => {
    const pub = new FakeRedisListClient();
    const sub = new FakeRedisListClient();
    const logger = { warn: vi.fn() };

    pub.setHash(WORLD_MODEL_STATE_KEY, {
      [WORLD_MODEL_STATE_FIELDS.currentEra]: "null",
      [WORLD_MODEL_STATE_FIELDS.zoneHistory]: JSON.stringify({}),
      [WORLD_MODEL_STATE_FIELDS.lastDecisions]: JSON.stringify({}),
      [WORLD_MODEL_STATE_FIELDS.lastTick]: "188",
      [WORLD_MODEL_STATE_FIELDS.lastStateTs]: "1711111188",
    });

    const createClient = vi
      .fn<(url: string) => FakeRedisListClient>()
      .mockReturnValueOnce(sub)
      .mockReturnValueOnce(pub);

    const ipc = new RedisIpc(
      { url: "redis://fake" },
      {
        createClient,
      },
    );

    const snapshot = await ipc.loadWorldModelState({ logger });
    expect(snapshot).toBeNull();
    expect(logger.warn).toHaveBeenCalledWith(
      expect.stringContaining("missing world model mirror fields"),
    );
  });
});
