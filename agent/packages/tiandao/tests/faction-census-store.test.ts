import { describe, expect, it, vi } from "vitest";
import { FactionCensusStore } from "../src/faction-census-store.js";
import type { FactionCensusStoreClient } from "../src/faction-census-store.js";

/**
 * plan-offscreen-war-v1 P5：FactionCensusStore 薄消费层测试。
 *
 * 覆盖：
 *   1. 订阅 bong:faction_state（connect 调 sub.subscribe）
 *   2. 合法 payload 解析并存入 census（最新覆盖语义）
 *   3. 校验拒非法 payload（非法 status / 缺字段）
 *   4. region_descriptor 增量增强入口（getGroupCensus 返回 region_descriptor 供调用方引用）
 */

function makeSub(): FactionCensusStoreClient & {
  emit: (channel: string, msg: string) => void;
} {
  const listeners = new Map<string, Array<(ch: string, msg: string) => void>>();
  return {
    subscribe: vi.fn().mockResolvedValue(undefined),
    unsubscribe: vi.fn().mockResolvedValue(undefined),
    disconnect: vi.fn(),
    on: vi.fn((event, listener) => {
      const arr = listeners.get(event) ?? [];
      arr.push(listener as (ch: string, msg: string) => void);
      listeners.set(event, arr);
    }),
    off: vi.fn((event, listener) => {
      const arr = listeners.get(event) ?? [];
      listeners.set(event, arr.filter((l) => l !== listener));
    }),
    emit(channel, msg) {
      for (const listener of listeners.get("message") ?? []) {
        listener(channel, msg);
      }
    },
  };
}

function makeSample(overrides: Record<string, unknown> = {}): string {
  return JSON.stringify({
    v: 1,
    kind: "faction_state",
    group_id: 2,
    region_descriptor: "rift_valley一带散修",
    population: 7,
    status: "rising",
    dominant_zone: "rift_valley",
    strongest_realm: "Solidify",
    strongest_char_id: "dormant:rogue:3",
    at_tick: 4321,
    ...overrides,
  });
}

describe("FactionCensusStore", () => {
  it("connect subscribes to bong:faction_state", async () => {
    // P5 验收：connect 必须调 sub.subscribe("bong:faction_state")，
    // 确保 redis-ipc 层能从正确 channel 收 FactionStateV1 快照。
    const sub = makeSub();
    const store = new FactionCensusStore({ sub });
    await store.connect();
    expect(sub.subscribe).toHaveBeenCalledWith("bong:faction_state");
  });

  it("connect is idempotent — second call skips subscribe", async () => {
    // 幂等保护：connect 两次只应调用一次 subscribe，避免重复注册 listener。
    const sub = makeSub();
    const store = new FactionCensusStore({ sub });
    await store.connect();
    await store.connect();
    expect(sub.subscribe).toHaveBeenCalledTimes(1);
  });

  it("accepts valid FactionStateV1 and stores latest census", async () => {
    // happy path：合法 payload 必须被接受，census 存入后可通过 getGroupCensus 查询。
    // stats.accepted++ 且 stats.rejected 不变。
    const sub = makeSub();
    const store = new FactionCensusStore({ sub });
    await store.connect();

    sub.emit("bong:faction_state", makeSample({ group_id: 2, population: 7, status: "rising" }));

    expect(store.stats.received).toBe(1);
    expect(store.stats.accepted).toBe(1);
    expect(store.stats.rejected).toBe(0);

    const census = store.getGroupCensus(2);
    expect(census).toBeDefined();
    expect(census?.group_id).toBe(2);
    expect(census?.population).toBe(7);
    expect(census?.status).toBe("rising");
    expect(census?.region_descriptor).toBe("rift_valley一带散修");
  });

  it("latest update overwrites previous census for same group_id", async () => {
    // 最新覆盖语义：同 group_id 的更新应覆盖旧快照（只保留最新状态，不累积历史）。
    // 期望 population 从 7 更新为 12，status 从 rising → waning。
    const sub = makeSub();
    const store = new FactionCensusStore({ sub });
    await store.connect();

    sub.emit("bong:faction_state", makeSample({ group_id: 2, population: 7, status: "rising" }));
    sub.emit("bong:faction_state", makeSample({ group_id: 2, population: 12, status: "waning", at_tick: 5000 }));

    expect(store.stats.accepted).toBe(2);
    const census = store.getGroupCensus(2);
    expect(census?.population).toBe(12);
    expect(census?.status).toBe("waning");
    // at_tick 也应是最新的
    expect(census?.at_tick).toBe(5000);
  });

  it("stores multiple distinct groups independently", async () => {
    // 多 group 独立存储：group_id=2 和 group_id=5 各自保存，互不覆盖。
    const sub = makeSub();
    const store = new FactionCensusStore({ sub });
    await store.connect();

    sub.emit("bong:faction_state", makeSample({ group_id: 2, population: 7, dominant_zone: "rift_valley" }));
    sub.emit("bong:faction_state", makeSample({ group_id: 5, population: 3, dominant_zone: "spawn", region_descriptor: "spawn一带散修" }));

    expect(store.groupCount).toBe(2);
    expect(store.getGroupCensus(2)?.dominant_zone).toBe("rift_valley");
    expect(store.getGroupCensus(5)?.dominant_zone).toBe("spawn");
  });

  it("rejects invalid status string and increments rejected counter", async () => {
    // 反：非法 status（ascending 不属于 rising/stable/waning）必须被拒，
    // stats.rejected++ 且 census 不被污染（group_id=2 无记录）。
    // 与 schema FactionStateV1 server端双端对齐：不接受 enum 之外的 status。
    const warnMock = vi.fn();
    const sub = makeSub();
    const store = new FactionCensusStore({
      sub,
      logger: { info: vi.fn(), warn: warnMock },
    });
    await store.connect();

    sub.emit(
      "bong:faction_state",
      makeSample({ status: "ascending" }),
    );

    expect(store.stats.received).toBe(1);
    expect(store.stats.rejected).toBe(1);
    expect(store.stats.accepted).toBe(0);
    expect(store.getGroupCensus(2)).toBeUndefined();
    expect(warnMock).toHaveBeenCalled();
  });

  it("rejects payload missing required field population", async () => {
    // 反：缺 population 字段必须被拒（FactionStateV1 无 serde default，不能静默归零）。
    // 与 schema faction-state.invalid-missing-field.sample.json 对拍。
    const sub = makeSub();
    const store = new FactionCensusStore({ sub });
    await store.connect();

    const payload = JSON.parse(makeSample()) as Record<string, unknown>;
    delete payload.population;
    sub.emit("bong:faction_state", JSON.stringify(payload));

    expect(store.stats.rejected).toBe(1);
    expect(store.stats.accepted).toBe(0);
  });

  it("rejects non-JSON payload", async () => {
    // 错误分支：非 JSON 字符串必须被拒，不抛异常，只记 stats.rejected++。
    const sub = makeSub();
    const store = new FactionCensusStore({ sub });
    await store.connect();

    sub.emit("bong:faction_state", "not-valid-json{{{");

    expect(store.stats.rejected).toBe(1);
    expect(store.stats.accepted).toBe(0);
  });

  it("ignores messages on other channels", async () => {
    // 过滤：非 bong:faction_state channel 的消息不影响 census（onMessage 守卫）。
    const sub = makeSub();
    const store = new FactionCensusStore({ sub });
    await store.connect();

    sub.emit("bong:npc/death", makeSample());

    expect(store.stats.received).toBe(0);
    expect(store.groupCount).toBe(0);
  });

  it("getGroupCensus returns undefined for unknown group_id", () => {
    // 边界：未知 group_id 必须返回 undefined（不抛异常）。
    const sub = makeSub();
    const store = new FactionCensusStore({ sub });
    expect(store.getGroupCensus(999)).toBeUndefined();
  });

  it("getAllCensus returns a copy — mutations do not affect internal state", async () => {
    // 防御性副本：getAllCensus 返回的 Map 被外部修改不应影响内部 census。
    const sub = makeSub();
    const store = new FactionCensusStore({ sub });
    await store.connect();

    sub.emit("bong:faction_state", makeSample({ group_id: 2 }));

    const copy = store.getAllCensus();
    copy.delete(2);
    // 内部 census 仍保留 group_id=2
    expect(store.getGroupCensus(2)).toBeDefined();
    expect(store.groupCount).toBe(1);
  });

  it("region_descriptor is accessible via getGroupCensus — P5 增量增强入口", async () => {
    // P5 增量增强入口：faction_state 存储的 region_descriptor 可供渲染层引用，
    // 实现匿名散修群体区域描述（纯只读，不触碰 war narration 模板）。
    // 期望：region_descriptor 字段按 wire 值保留，无截断或转换。
    const sub = makeSub();
    const store = new FactionCensusStore({ sub });
    await store.connect();

    sub.emit("bong:faction_state", makeSample({ region_descriptor: "spawn一带散修", group_id: 3 }));

    const census = store.getGroupCensus(3);
    expect(census?.region_descriptor).toBe("spawn一带散修");
  });

  it("all three GroupStatus variants are accepted by the store", async () => {
    // 状态转换饱和：rising / stable / waning 三变体各一条，确认 store 对每个变体都 accept。
    const sub = makeSub();
    const store = new FactionCensusStore({ sub });
    await store.connect();

    for (const [idx, status] of (["rising", "stable", "waning"] as const).entries()) {
      const groupId = 10 + idx;
      sub.emit("bong:faction_state", makeSample({ group_id: groupId, status }));
      const census = store.getGroupCensus(groupId);
      expect(
        census?.status,
        `expected group ${groupId} status="${status}" to be stored, actual ${JSON.stringify(census?.status)}`,
      ).toBe(status);
    }

    expect(store.stats.accepted).toBe(3);
    expect(store.stats.rejected).toBe(0);
  });

  it("disconnect unsubscribes and calls sub.disconnect", async () => {
    // 生命周期：disconnect 必须调 unsubscribe + sub.disconnect，释放资源。
    const sub = makeSub();
    const store = new FactionCensusStore({ sub });
    await store.connect();
    await store.disconnect();
    expect(sub.unsubscribe).toHaveBeenCalledTimes(1);
    expect(sub.disconnect).toHaveBeenCalledTimes(1);
  });
});
