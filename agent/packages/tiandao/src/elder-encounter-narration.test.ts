/**
 * plan-dying-elder-v1 P3 — ElderEncounterNarrationRuntime 单测。
 *
 * 测契约：
 * - 收 bong:elder_encounter 消息 → 产 AGENT_NARRATE publish（5 种 event_kind 各验）
 * - 无效 JSON / schema 不符 → rejectedContract++，不 publish
 * - appeared 叙事 scope="zone"，target=zone_name
 * - 死亡广播叙事 scope="broadcast"
 * - connect/disconnect 生命周期
 * - stats 初始化 / 计数器自增
 */

import { beforeEach, describe, expect, it, vi } from "vitest";

import { CHANNELS } from "@bong/schema";
import {
  ELDER_ENCOUNTER_DURABLE_WIRING,
  startElderEncounterRuntime,
  type RedisClientConstructor,
} from "./main.js";

import {
  DURABLE_NARRATION_DEDUPE_TTL_SECONDS,
  DURABLE_QUEUE_READ_TIMEOUT_SECONDS,
  ELDER_ENCOUNTER_DURABLE_DEAD_LETTER,
  ELDER_ENCOUNTER_DURABLE_PROCESSING,
  ELDER_ENCOUNTER_RUNTIME_SHUTDOWN_TIMEOUT_MS,
  MAX_DURABLE_RECOVERY_BATCH,
  MAX_SEEN_DURABLE_EVENT_IDS,
  SEEN_DURABLE_EVENT_ID_TTL_MS,
  ElderEncounterNarrationRuntime,
  renderAppearedNarration,
  renderDanReceivedNarration,
  renderDeathBroadcast,
} from "./elder-encounter-narration.js";

const { ELDER_ENCOUNTER, ELDER_ENCOUNTER_DURABLE, AGENT_NARRATE } = CHANNELS;

// ──── mock client ────────────────────────────────────────────────────────────

function makeMockClient() {
  const listeners: Map<string, ((ch: string, msg: string) => void)[]> = new Map();

  return {
    subscribe: vi.fn(async (_channel: string) => {}),
    on: vi.fn((event: string, listener: (ch: string, msg: string) => void) => {
      const arr = listeners.get(event) ?? [];
      arr.push(listener);
      listeners.set(event, arr);
    }),
    off: vi.fn((event: string, listener: (ch: string, msg: string) => void) => {
      const arr = listeners.get(event) ?? [];
      const idx = arr.indexOf(listener);
      if (idx !== -1) arr.splice(idx, 1);
      listeners.set(event, arr);
    }),
    unsubscribe: vi.fn(async () => {}),
    disconnect: vi.fn(() => {}),
    publish: vi.fn(async (_channel: string, _message: string) => 1),
    eval: vi.fn(
      async (
        _script: string,
        _numKeys: number,
        ..._args: Array<string | number>
      ) => 1,
    ),

    emit(channel: string, message: string) {
      (listeners.get("message") ?? []).forEach((l) => l(channel, message));
    },
  };
}

function makeProductionRedisConstructor() {
  const clients: Array<{
    url: string;
    client: ReturnType<typeof makeMockClient>;
  }> = [];
  const Constructor = class {
    constructor(url: string) {
      const client = makeMockClient();
      clients.push({ url, client });
      return client;
    }
  };
  return {
    Constructor: Constructor as unknown as RedisClientConstructor,
    clients,
  };
}

function makeDurableQueueClient(options: { blockReadNumber?: number } = {}) {
  const lists = new Map<string, string[]>();
  let releaseBlockedRead: (() => void) | undefined;
  let readCount = 0;
  const list = (key: string): string[] => {
    const existing = lists.get(key);
    if (existing !== undefined) return existing;
    const created: string[] = [];
    lists.set(key, created);
    return created;
  };
  const source = list(ELDER_ENCOUNTER_DURABLE);
  const processing = list(ELDER_ENCOUNTER_DURABLE_PROCESSING);
  const deadLetter = list(ELDER_ENCOUNTER_DURABLE_DEAD_LETTER);

  return {
    lists,
    source,
    processing,
    deadLetter,
    releaseBlockedRead: () => {
      releaseBlockedRead?.();
      releaseBlockedRead = undefined;
    },
    blmove: vi.fn(
      async (
        sourceKey: string,
        destinationKey: string,
        sourceSide: "LEFT" | "RIGHT",
        destinationSide: "LEFT" | "RIGHT",
        _timeoutSeconds: number,
      ) => {
        readCount += 1;
        if (options.blockReadNumber === readCount) {
          await new Promise<void>((resolve) => {
            releaseBlockedRead = resolve;
          });
        }
        const sourceList = list(sourceKey);
        const destinationList = list(destinationKey);
        const value = sourceSide === "LEFT" ? sourceList.shift() : sourceList.pop();
        if (value === undefined) {
          await new Promise((resolve) => setTimeout(resolve, 0));
          return null;
        }
        if (destinationSide === "LEFT") destinationList.unshift(value);
        else destinationList.push(value);
        return value;
      },
    ),
    lrem: vi.fn(async (key: string, _count: number, value: string) => {
      const target = list(key);
      const index = target.indexOf(value);
      if (index === -1) return 0;
      target.splice(index, 1);
      return 1;
    }),
    lmove: vi.fn(
      async (
        sourceKey: string,
        destinationKey: string,
        sourceSide: "LEFT" | "RIGHT",
        destinationSide: "LEFT" | "RIGHT",
      ) => {
        const sourceList = list(sourceKey);
        const destinationList = list(destinationKey);
        const value = sourceSide === "LEFT" ? sourceList.shift() : sourceList.pop();
        if (value === undefined) return null;
        if (destinationSide === "LEFT") destinationList.unshift(value);
        else destinationList.push(value);
        return value;
      },
    ),
    disconnect: vi.fn(() => {}),
  };
}

async function waitForQueueState(
  predicate: () => boolean,
  message: string,
): Promise<void> {
  await vi.waitFor(() => {
    expect(predicate(), message).toBe(true);
  });
}

function silenceLogger() {
  return { info: vi.fn(), warn: vi.fn() };
}

function queueRuntime(
  sub: ReturnType<typeof makeMockClient>,
  pub: ReturnType<typeof makeMockClient>,
  queue: ReturnType<typeof makeDurableQueueClient>,
) {
  return new ElderEncounterNarrationRuntime({
    sub,
    pub,
    durableQueue: queue,
    logger: silenceLogger(),
  });
}

async function stopQueueRuntime(runtime: ElderEncounterNarrationRuntime): Promise<void> {
  await runtime.disconnect();
}

// ──── payload helpers ────────────────────────────────────────────────────────

type EventKind =
  | "appeared"
  | "dan_received"
  | "betrayal"
  | "dead_natural"
  | "dead_player_kill";

function makePayload(eventKind: EventKind, overrides: Record<string, unknown> = {}): string {
  return JSON.stringify({
    zone_name: "tsy_deep",
    elder_entity_id: 42,
    event_kind: eventKind,
    betray_probability: eventKind === "appeared" ? 0.65 : 0.0,
    dan_count: eventKind === "dan_received" ? 3 : 0,
    offered_skill_id: eventKind === "appeared" ? "woliu.heart" : "",
    // Bug4 修复：schema 新增 qi_fraction 字段（server 必发，additionalProperties:false 要求）
    qi_fraction: ["betrayal", "dead_natural", "dead_player_kill"].includes(eventKind) ? 0.0 : 1.0,
    server_tick: 720000,
    ...overrides,
  });
}

// ──── tests ──────────────────────────────────────────────────────────────────

describe("ElderEncounterNarrationRuntime", () => {
  let sub: ReturnType<typeof makeMockClient>;
  let pub: ReturnType<typeof makeMockClient>;
  let runtime: ElderEncounterNarrationRuntime;

  beforeEach(async () => {
    sub = makeMockClient();
    pub = makeMockClient();
    runtime = new ElderEncounterNarrationRuntime({ sub, pub });
    await runtime.connect();
  });

  it("connect 订阅 CHANNELS.ELDER_ENCOUNTER", () => {
    expect(
      sub.subscribe,
      `connect() 应订阅 ${ELDER_ENCOUNTER} 频道以接收遭遇事件`,
    ).toHaveBeenCalledWith(ELDER_ENCOUNTER);
  });

  it("生产 bootstrap 明确只启动 Pub/Sub，durable consumer 保持未接线", async () => {
    const { Constructor, clients } = makeProductionRedisConstructor();
    const cleanup = await startElderEncounterRuntime(
      { redisUrl: "redis://production-bootstrap-test" },
      Constructor,
    );

    try {
      await vi.waitFor(() => {
        expect(clients[0]?.client.subscribe).toHaveBeenCalledWith(ELDER_ENCOUNTER);
      });
      expect(ELDER_ENCOUNTER_DURABLE_WIRING.status).toBe("contract-first-unwired");
      expect(ELDER_ENCOUNTER_DURABLE_WIRING.owner).toContain(
        "plan-agent-narration-pipeline-v1.md",
      );
      expect(clients, "生产 elder bootstrap 应只创建 sub + pub 两个 Redis 客户端").toHaveLength(2);
      expect(clients.map(({ url }) => url)).toEqual([
        "redis://production-bootstrap-test",
        "redis://production-bootstrap-test",
      ]);
      expect(
        clients.every(({ client }) => !("blmove" in client)),
        "生产断开态不得把 durable queue client 注入 runtime",
      ).toBe(true);

      const [productionSub, productionPub] = clients;
      expect(productionSub?.client.subscribe).toHaveBeenCalledTimes(1);
      expect(productionSub?.client.subscribe).not.toHaveBeenCalledWith(ELDER_ENCOUNTER_DURABLE);
      productionSub?.client.emit(ELDER_ENCOUNTER, makePayload("appeared"));
      await vi.waitFor(() => {
        expect(productionPub?.client.publish).toHaveBeenCalledWith(
          AGENT_NARRATE,
          expect.any(String),
        );
      });
    } finally {
      await cleanup();
    }

    expect(clients[0]?.client.disconnect).toHaveBeenCalledOnce();
    expect(clients[1]?.client.disconnect).toHaveBeenCalledOnce();
  });

  it("stats 初始为 0", () => {
    const r2 = new ElderEncounterNarrationRuntime({ sub, pub });
    expect(r2.stats).toEqual({
      received: 0,
      published: 0,
      rejectedContract: 0,
      ignored: 0,
    });
  });

  // ── appeared 事件 ──────────────────────────────────────────────────────────

  it("appeared → 产出 AGENT_NARRATE，scope=zone，target=zone_name", async () => {
    await runtime.handlePayload(makePayload("appeared"));
    expect(pub.publish, "appeared 事件应触发恰好一次 publish").toHaveBeenCalledOnce();
    const [channel, msg] = pub.publish.mock.calls[0] as [string, string];
    expect(channel, `叙事应发往 ${AGENT_NARRATE}`).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(msg) as { v: number; narrations: Array<{ scope: string; target: string }> };
    expect(envelope.v).toBe(1);
    expect(envelope.narrations).toHaveLength(1);
    expect(envelope.narrations[0]?.scope, "appeared 叙事 scope 应为 zone").toBe("zone");
    expect(
      envelope.narrations[0]?.target,
      "appeared 叙事 target 应为 zone_name",
    ).toBe("tsy_deep");
    expect(runtime.stats.received).toBe(1);
    expect(runtime.stats.published).toBe(1);
  });

  // ── dan_received 事件 ──────────────────────────────────────────────────────

  it("dan_received → 产出 zone scope 叙事", async () => {
    await runtime.handlePayload(makePayload("dan_received"));
    expect(pub.publish).toHaveBeenCalledOnce();
    const [channel, msg] = pub.publish.mock.calls[0] as [string, string];
    expect(channel).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(msg) as { narrations: Array<{ scope: string }> };
    expect(envelope.narrations[0]?.scope, "dan_received 叙事 scope 应为 zone").toBe("zone");
  });

  it("dan_received 叙事文本包含 dan_count", async () => {
    await runtime.handlePayload(makePayload("dan_received", { dan_count: 3 }));
    const [, msg] = pub.publish.mock.calls[0] as [string, string];
    const envelope = JSON.parse(msg) as { narrations: Array<{ text: string }> };
    const text = envelope.narrations[0]?.text ?? "";
    expect(text, "叙事应包含给丹数量 '3'").toContain("3");
  });

  // ── betrayal 事件 ──────────────────────────────────────────────────────────

  it("betrayal → 产出 broadcast scope 叙事", async () => {
    await runtime.handlePayload(makePayload("betrayal"));
    expect(pub.publish).toHaveBeenCalledOnce();
    const [, msg] = pub.publish.mock.calls[0] as [string, string];
    const envelope = JSON.parse(msg) as { narrations: Array<{ scope: string }> };
    expect(
      envelope.narrations[0]?.scope,
      "betrayal 死亡广播 scope 应为 broadcast",
    ).toBe("broadcast");
    expect(runtime.stats.received).toBe(1);
  });

  it("betrayal 叙事 kind 为 dying_elder_betrayal", async () => {
    await runtime.handlePayload(makePayload("betrayal"));
    const [, msg] = pub.publish.mock.calls[0] as [string, string];
    const envelope = JSON.parse(msg) as { narrations: Array<{ kind: string }> };
    expect(envelope.narrations[0]?.kind).toBe("dying_elder_betrayal");
  });

  // ── dead_natural 事件 ──────────────────────────────────────────────────────

  it("dead_natural → 产出 broadcast scope 叙事", async () => {
    await runtime.handlePayload(makePayload("dead_natural"));
    expect(pub.publish).toHaveBeenCalledOnce();
    const [, msg] = pub.publish.mock.calls[0] as [string, string];
    const envelope = JSON.parse(msg) as { narrations: Array<{ scope: string; kind: string }> };
    expect(envelope.narrations[0]?.scope).toBe("broadcast");
    expect(envelope.narrations[0]?.kind).toBe("dying_elder_dead_natural");
  });

  // ── dead_player_kill 事件 ──────────────────────────────────────────────────

  it("dead_player_kill → 产出 broadcast scope 叙事", async () => {
    await runtime.handlePayload(makePayload("dead_player_kill"));
    expect(pub.publish).toHaveBeenCalledOnce();
    const [, msg] = pub.publish.mock.calls[0] as [string, string];
    const envelope = JSON.parse(msg) as { narrations: Array<{ scope: string; kind: string }> };
    expect(envelope.narrations[0]?.scope).toBe("broadcast");
    expect(envelope.narrations[0]?.kind).toBe("dying_elder_dead_player_kill");
  });

  // ── 错误路径 ───────────────────────────────────────────────────────────────

  it("无效 JSON → rejectedContract++，不 publish", async () => {
    await runtime.handlePayload("not-json");
    expect(pub.publish, "非法 JSON 不应触发 publish").not.toHaveBeenCalled();
    expect(
      runtime.stats.rejectedContract,
      "无效 JSON 应令 rejectedContract=1",
    ).toBe(1);
  });

  it("schema 不符（缺 zone_name）→ rejectedContract++，不 publish", async () => {
    const bad = JSON.stringify({
      elder_entity_id: 1,
      event_kind: "appeared",
      betray_probability: 0.5,
      dan_count: 0,
      offered_skill_id: "",
      server_tick: 100,
      // zone_name 缺失
    });
    await runtime.handlePayload(bad);
    expect(pub.publish, "缺 zone_name 的 payload 应被 schema 校验拒绝").not.toHaveBeenCalled();
    expect(runtime.stats.rejectedContract).toBe(1);
  });

  it("schema 不符（event_kind 非法值）→ rejectedContract++，不 publish", async () => {
    const bad = JSON.stringify({
      zone_name: "tsy_deep",
      elder_entity_id: 1,
      event_kind: "unknown_kind",
      betray_probability: 0.5,
      dan_count: 0,
      offered_skill_id: "",
      server_tick: 100,
    });
    await runtime.handlePayload(bad);
    expect(pub.publish).not.toHaveBeenCalled();
    expect(runtime.stats.rejectedContract).toBe(1);
  });

  it("非 ELDER_ENCOUNTER channel 消息通过 onMessage 路由被忽略", async () => {
    sub.emit("bong:other_channel", makePayload("appeared"));
    await new Promise((r) => setTimeout(r, 0));
    expect(pub.publish, "其他 channel 消息不应触发 publish").not.toHaveBeenCalled();
  });

  it("ELDER_ENCOUNTER channel 消息通过 onMessage emit 走完整路径", async () => {
    sub.emit(ELDER_ENCOUNTER, makePayload("appeared"));
    await new Promise((r) => setTimeout(r, 0));
    expect(pub.publish, "ELDER_ENCOUNTER emit 应触发 publish").toHaveBeenCalledOnce();
  });

  // ── durable event_id 去重 ────────────────────────────────────────────────────

  it("durable event_id 通过 Redis 原子 claim+publish，重复输入不再发布", async () => {
    const payload = makePayload("dead_natural", { event_id: "terminal:npc:42:720000" });

    await runtime.handlePayload(payload);
    await runtime.handlePayload(payload);

    expect(pub.publish, "durable payload 不得走非原子的普通 PUBLISH").not.toHaveBeenCalled();
    expect(pub.eval, "首次 durable payload 应走 Redis 原子去重脚本").toHaveBeenCalledOnce();
    const [, keyCount, dedupeKey, channel] = pub.eval.mock.calls[0] as [
      string,
      number,
      string,
      string,
      string,
    ];
    expect(keyCount, "Redis 去重脚本应只声明一个 key，避免 claim 与发布状态分裂").toBe(1);
    expect(dedupeKey, "去重 key 应绑定 durable event_id，避免不同遭遇互相抑制").toBe(
      "bong:dedupe:elder_encounter:terminal:npc:42:720000",
    );
    expect(channel, "原子脚本应发布到 AGENT_NARRATE，实际发布端点必须保持稳定").toBe(AGENT_NARRATE);
    expect(
      pub.eval.mock.calls[0]?.[5],
      `去重 claim 应带 ${DURABLE_NARRATION_DEDUPE_TTL_SECONDS}s TTL，避免 Redis key 永久泄漏`,
    ).toBe(DURABLE_NARRATION_DEDUPE_TTL_SECONDS);
    expect(runtime.stats.published).toBe(1);
    expect(runtime.stats.ignored).toBe(1);
  });

  it("Redis 已持有 durable event_id 时计 ignored 且不重复发布", async () => {
    pub.eval.mockResolvedValueOnce(0);

    await runtime.handlePayload(
      makePayload("betrayal", { event_id: "terminal:npc:42:duplicate" }),
    );

    expect(pub.eval).toHaveBeenCalledOnce();
    expect(pub.publish).not.toHaveBeenCalled();
    expect(runtime.stats.received).toBe(1);
    expect(runtime.stats.published).toBe(0);
    expect(runtime.stats.ignored).toBe(1);
  });

  it("durable 原子发布失败不在本地记 seen，同 ID 可重试", async () => {
    pub.eval.mockRejectedValueOnce(new Error("redis unavailable")).mockResolvedValueOnce(1);
    const payload = makePayload("dead_player_kill", {
      event_id: "terminal:npc:42:retry",
    });

    await runtime.handlePayload(payload);
    await runtime.handlePayload(payload);

    expect(pub.eval, "失败后同一 durable ID 必须再次尝试 Redis 原子脚本").toHaveBeenCalledTimes(2);
    expect(runtime.stats.received).toBe(2);
    expect(runtime.stats.published).toBe(1);
    expect(runtime.stats.ignored).toBe(0);
  });

  it("durable 去重缓存按 TTL 淘汰后允许完整消息路径再次处理", async () => {
    vi.useFakeTimers();
    try {
      const payload = makePayload("dead_natural", { event_id: "terminal:npc:42:ttl" });
      await runtime.handlePayload(payload);
      vi.advanceTimersByTime(SEEN_DURABLE_EVENT_ID_TTL_MS + 1);
      pub.eval.mockResolvedValueOnce(1);
      await runtime.handlePayload(payload);

      expect(
        pub.eval,
        "本地去重缓存过期后应重新执行 Redis claim，原因是短 TTL 只抑制重复突发而非永久丢弃事件",
      ).toHaveBeenCalledTimes(2);
      expect(runtime.stats.published, "去重缓存过期后应再次发布，实际发布次数必须为 2").toBe(2);
    } finally {
      vi.useRealTimers();
    }
  });

  it("durable 去重缓存超过上限时淘汰最旧 ID，避免进程内存无界增长", async () => {
    for (let index = 0; index < MAX_SEEN_DURABLE_EVENT_IDS + 1; index += 1) {
      await runtime.handlePayload(
        makePayload("dead_natural", { event_id: `terminal:npc:42:lru:${index}` }),
      );
    }
    pub.eval.mockResolvedValueOnce(1);
    await runtime.handlePayload(makePayload("dead_natural", { event_id: "terminal:npc:42:lru:0" }));

    expect(
      pub.eval,
      "超过本地 LRU 上限后最旧 ID 应可再次走 Redis claim，避免 seen 集合无界增长",
    ).toHaveBeenCalledTimes(MAX_SEEN_DURABLE_EVENT_IDS + 2);
    expect(runtime.stats.published, "最旧 ID 被淘汰后应重新发布，实际发布次数应包含重试").toBe(
      MAX_SEEN_DURABLE_EVENT_IDS + 2,
    );
  });

  it("durable 消息通过订阅 emit 走完整去重状态转换", async () => {
    const payload = makePayload("dead_natural", { event_id: "terminal:npc:42:emit" });
    sub.emit(ELDER_ENCOUNTER, payload);
    await vi.waitFor(() => expect(pub.eval).toHaveBeenCalledOnce());
    sub.emit(ELDER_ENCOUNTER, payload);
    await vi.waitFor(() => expect(runtime.stats.ignored).toBe(1));

    expect(pub.publish, "durable 订阅消息应只走原子 eval，不应旁路普通 publish").not.toHaveBeenCalled();
    expect(runtime.stats.published, "重复 durable 订阅消息应只发布一次，实际发布次数应为 1").toBe(1);
  });

  it("durable 去重脚本返回非 1 时不污染本地 seen，后续状态可重试", async () => {
    pub.eval.mockResolvedValueOnce(0).mockResolvedValueOnce(1);
    const payload = makePayload("dead_natural", { event_id: "terminal:npc:42:claim-retry" });

    await runtime.handlePayload(payload);
    await runtime.handlePayload(payload);

    expect(pub.eval, "Redis claim 未成功时后续同 ID 必须可重试，不能被本地 seen 错误吞掉").toHaveBeenCalledTimes(2);
    expect(runtime.stats.published, "第二次 claim 成功后应发布一次，实际发布次数应为 1").toBe(1);
  });

  it("durable event_id 通过 Redis 原子 claim+publish，重复输入不再发布", async () => {
    const payload = makePayload("dead_natural", { event_id: "terminal:npc:42:duplicate-cache" });

    await runtime.handlePayload(payload);
    await runtime.handlePayload(payload);

    expect(runtime.stats.ignored, "成功发布后本地 seen 应抑制短时间重复输入，实际 ignored 应为 1").toBe(1);
  });

  it("durable event_id 的 Redis TTL 与本地 TTL 都是有界策略", () => {
    expect(
      DURABLE_NARRATION_DEDUPE_TTL_SECONDS,
      "Redis 去重键必须有正 TTL，避免跨 runtime 的永久 key 泄漏",
    ).toBeGreaterThan(0);
    expect(
      SEEN_DURABLE_EVENT_ID_TTL_MS,
      "本地去重缓存必须有正 TTL，避免进程内重复 ID 永久占用",
    ).toBeGreaterThan(0);
  });

  it("durable payload 缺少 EVAL 能力时 fail closed，不退化为普通 PUBLISH", async () => {
    const pubWithoutEval = makeMockClient();
    delete (pubWithoutEval as Partial<typeof pubWithoutEval>).eval;
    const failClosedRuntime = new ElderEncounterNarrationRuntime({
      sub,
      pub: pubWithoutEval,
    });

    await failClosedRuntime.handlePayload(
      makePayload("dead_natural", { event_id: "terminal:npc:42:no-eval" }),
    );

    expect(pubWithoutEval.publish).not.toHaveBeenCalled();
    expect(failClosedRuntime.stats.received).toBe(1);
    expect(failClosedRuntime.stats.published).toBe(0);
  });

  it("无 event_id 的非 durable 遭遇保持普通 PUBLISH 语义", async () => {
    await runtime.handlePayload(makePayload("appeared"));

    expect(pub.publish).toHaveBeenCalledOnce();
    expect(pub.eval).not.toHaveBeenCalled();
  });

  it("durable queue 只在原子发布成功后返回可 ACK", async () => {
    const payload = makePayload("dead_natural", {
      event_id: "terminal:npc:42:queue-ack",
    });
    const durableRuntime = new ElderEncounterNarrationRuntime({ sub, pub });

    expect(await durableRuntime.processDurablePayload(payload)).toBe(true);

    expect(pub.eval).toHaveBeenCalledOnce();
  });

  it("durable queue 发布失败返回不可 ACK", async () => {
    const payload = makePayload("betrayal", {
      event_id: "terminal:npc:42:queue-retry",
    });
    pub.eval.mockRejectedValueOnce(new Error("redis unavailable"));
    const durableRuntime = new ElderEncounterNarrationRuntime({ sub, pub });

    expect(await durableRuntime.processDurablePayload(payload)).toBe(false);

    expect(pub.eval).toHaveBeenCalledOnce();
  });

  it("durable queue 拒绝缺 event_id 的 payload，避免无去重 ACK", async () => {
    const durableRuntime = new ElderEncounterNarrationRuntime({ sub, pub });

    expect(await durableRuntime.processDurablePayload(makePayload("dead_natural"))).toBe(false);

    expect(pub.publish).not.toHaveBeenCalled();
    expect(pub.eval).not.toHaveBeenCalled();
    expect(durableRuntime.stats.rejectedContract).toBe(1);
  });

  it("durable connect 只恢复有界数量，并保持 processing 右端到 source 左端的 FIFO", async () => {
    const queue = makeDurableQueueClient({ blockReadNumber: 1 });
    queue.processing.push(...Array.from({ length: MAX_DURABLE_RECOVERY_BATCH + 1 }, (_, index) => `pending-${index}`));
    const durableRuntime = queueRuntime(makeMockClient(), makeMockClient(), queue);

    await durableRuntime.connect();

    expect(queue.processing, "恢复达到上限时应保留剩余 processing，避免启动阶段无界搬运").toHaveLength(1);
    expect(queue.source.slice(0, 3), "恢复应从 processing 右端移到 source 左端，保持最早消息先重试").toEqual([
      "pending-1",
      "pending-2",
      "pending-3",
    ]);
    expect(queue.lmove.mock.calls[0], "恢复应使用 processing RIGHT→source LEFT，实际参数必须体现 FIFO 方向").toEqual([
      ELDER_ENCOUNTER_DURABLE_PROCESSING,
      ELDER_ENCOUNTER_DURABLE,
      "RIGHT",
      "LEFT",
    ]);
    const disconnectPromise = durableRuntime.disconnect();
    queue.releaseBlockedRead();
    await disconnectPromise;
  });

  it("durable worker retry 将 processing 右端移回 source 左端，而不是反向吞掉 payload", async () => {
    const queue = makeDurableQueueClient({ blockReadNumber: 2 });
    const payload = makePayload("dead_natural", { event_id: "terminal:npc:42:worker-retry" });
    queue.source.push(payload);
    const workerRuntime = queueRuntime(sub, pub, queue);
    pub.eval.mockRejectedValueOnce(new Error("redis unavailable"));

    await workerRuntime.connect();
    await waitForQueueState(
      () => queue.source.includes(payload) && queue.processing.length === 0,
      "Redis 暂时失败后 payload 应从 processing 回到 source，原因是失败可重试且 processing 必须清空",
    );
    expect(queue.source, "retry 后 source 应重新拥有原 payload，实际队列不能丢消息").toContain(payload);
    expect(queue.lmove.mock.calls.some((call) => call[0] === ELDER_ENCOUNTER_DURABLE_PROCESSING && call[1] === ELDER_ENCOUNTER_DURABLE && call[2] === "RIGHT" && call[3] === "LEFT"), "retry 应使用 processing RIGHT→source LEFT，避免错误端点导致重复/丢失").toBe(true);

    const disconnectPromise = workerRuntime.disconnect();
    queue.releaseBlockedRead();
    await disconnectPromise;
  });

  it("durable worker 将不可恢复 poison pill 移入 dead-letter，不进行无限 retry", async () => {
    const queue = makeDurableQueueClient();
    const payload = makePayload("dead_natural", { event_id: "terminal:npc:42:dead-letter" });
    queue.source.push(payload);
    const pubWithoutEval = makeMockClient();
    delete (pubWithoutEval as Partial<typeof pubWithoutEval>).eval;
    const workerRuntime = queueRuntime(sub, pubWithoutEval, queue);

    await workerRuntime.connect();
    await waitForQueueState(
      () => queue.deadLetter.includes(payload) && queue.processing.length === 0,
      "缺少 EVAL 的 durable payload 应进入 dead-letter，原因是该契约缺陷不可通过重试修复",
    );
    await workerRuntime.disconnect();

    expect(queue.deadLetter, "poison pill 应只进入 dead-letter 一次，实际不能留在 source 重试").toEqual([payload]);
    expect(queue.source, "dead-letter 后 source 应为空，避免 poison pill 无限回灌").toEqual([]);
  });

  it("durable worker 的 BLMove 读取超时与 shutdown timeout 有严格边界", async () => {
    const queue = makeDurableQueueClient({ blockReadNumber: 1 });
    const workerRuntime = queueRuntime(sub, pub, queue);

    await workerRuntime.connect();
    expect(queue.blmove.mock.calls[0]?.[4], "worker 应以 1 秒阻塞读取，实际 timeout 必须固定为契约值").toBe(
      DURABLE_QUEUE_READ_TIMEOUT_SECONDS,
    );
    expect(
      ELDER_ENCOUNTER_RUNTIME_SHUTDOWN_TIMEOUT_MS,
      "shutdown timeout 必须严格大于最长 BLMove 阻塞，避免 500ms 提前截断 worker 清理",
    ).toBeGreaterThan(DURABLE_QUEUE_READ_TIMEOUT_SECONDS * 1000);
    queue.releaseBlockedRead();
    await workerRuntime.disconnect();
  });

  // ── disconnect ─────────────────────────────────────────────────────────────

  it("disconnect 清理 listener + unsubscribe", async () => {
    await runtime.disconnect();
    expect(
      sub.off,
      "disconnect() 应调用 sub.off 移除 onMessage 监听器",
    ).toHaveBeenCalled();
    expect(
      sub.unsubscribe,
      "disconnect() 应调用 sub.unsubscribe 退订频道",
    ).toHaveBeenCalled();
  });

  it("disconnect 后消息不再被处理", async () => {
    await runtime.disconnect();
    sub.emit(ELDER_ENCOUNTER, makePayload("appeared"));
    await new Promise((r) => setTimeout(r, 0));
    expect(
      pub.publish,
      "disconnect() 后收到的消息不应触发任何 publish",
    ).not.toHaveBeenCalled();
  });
});

// ──── 纯函数叙事模板测试 ─────────────────────────────────────────────────────

describe("renderAppearedNarration", () => {
  it("scope 为 zone，target 为 zone_name", () => {
    const result = renderAppearedNarration({
      zone_name: "tsy_abyss",
      elder_entity_id: 5,
      event_kind: "appeared",
      betray_probability: 0.65,
      dan_count: 0,
      offered_skill_id: "woliu.heart",
      qi_fraction: 1.0,
      server_tick: 1000,
    });
    expect(result.scope).toBe("zone");
    expect(result.target).toBe("tsy_abyss");
    expect(result.kind).toBe("dying_elder_appeared");
    expect(typeof result.text).toBe("string");
    expect(result.text.length, "叙事文本不应为空").toBeGreaterThan(0);
  });

  it("高 betray_probability 叙事包含危险暗示", () => {
    const result = renderAppearedNarration({
      zone_name: "tsy_deep",
      elder_entity_id: 1,
      event_kind: "appeared",
      betray_probability: 0.9,
      dan_count: 0,
      offered_skill_id: "woliu.turbulence_burst",
      qi_fraction: 1.0,
      server_tick: 0,
    });
    expect(result.text, "高危险度应包含'险机'暗示").toContain("险机");
  });

  it("低 betray_probability 叙事不含'险机'", () => {
    const result = renderAppearedNarration({
      zone_name: "tsy_shallow",
      elder_entity_id: 2,
      event_kind: "appeared",
      betray_probability: 0.35,
      dan_count: 0,
      offered_skill_id: "anqi.echo_fractal",
      qi_fraction: 1.0,
      server_tick: 0,
    });
    expect(result.text, "低危险度不应包含'险机'").not.toContain("险机");
  });
});

describe("renderDanReceivedNarration", () => {
  it("scope 为 zone，text 包含 dan_count", () => {
    const result = renderDanReceivedNarration({
      zone_name: "tsy_deep",
      elder_entity_id: 10,
      event_kind: "dan_received",
      betray_probability: 0.0,
      dan_count: 4,
      offered_skill_id: "sword_path.heaven_gate",
      qi_fraction: 0.7,
      server_tick: 500,
    });
    expect(result.scope).toBe("zone");
    expect(result.kind).toBe("dying_elder_dan_received");
    expect(result.text).toContain("4");
  });
});

describe("renderDeathBroadcast", () => {
  it("betrayal → scope=broadcast, kind=dying_elder_betrayal", () => {
    const result = renderDeathBroadcast(
      {
        zone_name: "tsy_deep",
        elder_entity_id: 7,
        event_kind: "betrayal",
        betray_probability: 0.0,
        dan_count: 5,
        offered_skill_id: "",
        qi_fraction: 0.0,
        server_tick: 999,
      },
      "betrayal",
    );
    expect(result.scope).toBe("broadcast");
    expect(result.kind).toBe("dying_elder_betrayal");
    expect(result.text).toContain("tsy_deep");
  });

  it("dead_natural → scope=broadcast, kind=dying_elder_dead_natural", () => {
    const result = renderDeathBroadcast(
      {
        zone_name: "tsy_abyss",
        elder_entity_id: 3,
        event_kind: "dead_natural",
        betray_probability: 0.0,
        dan_count: 0,
        offered_skill_id: "",
        qi_fraction: 0.0,
        server_tick: 100,
      },
      "dead_natural",
    );
    expect(result.scope).toBe("broadcast");
    expect(result.kind).toBe("dying_elder_dead_natural");
  });

  it("dead_player_kill → scope=broadcast, kind=dying_elder_dead_player_kill", () => {
    const result = renderDeathBroadcast(
      {
        zone_name: "tsy_deep",
        elder_entity_id: 1,
        event_kind: "dead_player_kill",
        betray_probability: 0.0,
        dan_count: 0,
        offered_skill_id: "",
        qi_fraction: 0.0,
        server_tick: 200,
      },
      "dead_player_kill",
    );
    expect(result.scope).toBe("broadcast");
    expect(result.kind).toBe("dying_elder_dead_player_kill");
    expect(result.text, "死亡广播文本应提及 zone_name").toContain("tsy_deep");
  });

  it("target 格式为 zone:{zone_name}|elder:{idx}", () => {
    const result = renderDeathBroadcast(
      {
        zone_name: "tsy_test",
        elder_entity_id: 99,
        event_kind: "dead_natural",
        betray_probability: 0.0,
        dan_count: 0,
        offered_skill_id: "",
        qi_fraction: 0.0,
        server_tick: 0,
      },
      "dead_natural",
    );
    expect(result.target).toBe("zone:tsy_test|elder:99");
  });
});
