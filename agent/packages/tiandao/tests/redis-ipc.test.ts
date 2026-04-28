import { describe, expect, it, vi } from "vitest";
import { RedisIpc, WORLD_MODEL_STATE_FIELDS, WORLD_MODEL_STATE_KEY } from "../src/redis-ipc.js";
import { CHANNELS } from "@bong/schema";

const {
  AGENT_COMMAND,
  AGENT_NARRATE,
  AGENT_WORLD_MODEL,
  FACTION_EVENT,
  NPC_DEATH,
  NPC_SPAWN,
  PLAYER_CHAT,
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
