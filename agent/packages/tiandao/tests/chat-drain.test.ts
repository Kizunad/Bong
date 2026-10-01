import { describe, expect, it } from "vitest";

import { RedisIpc, type RedisIpcClient } from "../src/redis-ipc.js";

const PLAYER_CHAT_KEY = "bong:player_chat";
const DRAIN_COUNTER_KEY = "bong:player_chat:drain_counter";

class FakeRedisBus {
  readonly lists = new Map<string, string[]>();
  readonly counters = new Map<string, number>();
  concurrentWritesNextDrain: string[] = [];

  pushToList(key: string, value: string): void {
    const current = this.lists.get(key) ?? [];
    this.lists.set(key, [...current, value]);
  }
}

class FakeRedisClient implements RedisIpcClient {
  constructor(private readonly bus: FakeRedisBus) {}

  async subscribe(): Promise<number> {
    return 1;
  }

  on(): unknown {
    return undefined;
  }

  async publish(): Promise<number> {
    return 0;
  }

  async eval(_script: string, _numKeys: number, ...args: string[]): Promise<unknown> {
    const [chatKey, secondArgument, thirdArgument] = args;
    if (thirdArgument !== undefined) {
      const current = this.bus.lists.get(chatKey) ?? [];
      const index = current.findIndex(
        (item) => item.includes(secondArgument) && item.includes(thirdArgument),
      );
      if (index < 0) {
        return false;
      }
      const [matching] = current.splice(index, 1);
      this.bus.lists.set(chatKey, current);
      return matching;
    }

    const counterKey = secondArgument;
    const current = this.bus.lists.get(chatKey);

    if (!current || current.length === 0) {
      return [];
    }

    const nextSuffix = (this.bus.counters.get(counterKey) ?? 0) + 1;
    this.bus.counters.set(counterKey, nextSuffix);

    const drainingKey = `${chatKey}:drain:${nextSuffix}`;

    this.bus.lists.set(drainingKey, current);
    this.bus.lists.delete(chatKey);

    if (this.bus.concurrentWritesNextDrain.length > 0) {
      const writes = [...this.bus.concurrentWritesNextDrain];
      this.bus.concurrentWritesNextDrain = [];
      for (const write of writes) {
        this.bus.pushToList(chatKey, write);
      }
    }

    const drained = [...(this.bus.lists.get(drainingKey) ?? [])];
    this.bus.lists.delete(drainingKey);
    return drained;
  }

  async unsubscribe(): Promise<unknown> {
    return 0;
  }

  disconnect(): void {}
}

describe("RedisIpc atomic player_chat drain", () => {
  it("takes only the requested player token and preserves other chat messages", async () => {
    const bus = new FakeRedisBus();
    const target = JSON.stringify({
      v: 1,
      ts: 1_700_000_010,
      player: "offline:Target",
      raw: "token-42 目标消息",
      zone: "spawn",
    });
    const otherPlayer = JSON.stringify({
      v: 1,
      ts: 1_700_000_011,
      player: "offline:Other",
      raw: "token-99 其他消息",
      zone: "spawn",
    });
    const otherToken = JSON.stringify({
      v: 1,
      ts: 1_700_000_012,
      player: "offline:Target",
      raw: "token-99 目标的另一条消息",
      zone: "spawn",
    });

    bus.pushToList(PLAYER_CHAT_KEY, target);
    bus.pushToList(PLAYER_CHAT_KEY, otherPlayer);
    bus.pushToList(PLAYER_CHAT_KEY, otherToken);

    const redis = new RedisIpc({
      url: "redis://fake",
      createClient: () => new FakeRedisClient(bus),
    });

    const matched = await redis.takeMatchingPlayerChat({
      player: "offline:Target",
      token: "token-42",
      logger: { warn: () => undefined },
    });

    expect(matched?.raw).toBe("token-42 目标消息");
    expect(bus.lists.get(PLAYER_CHAT_KEY)).toEqual([otherPlayer, otherToken]);
  });

  it("preserves concurrent writes for the next drain round", async () => {
    const bus = new FakeRedisBus();
    const firstBatchRawA = JSON.stringify({
      v: 1,
      ts: 1_700_000_010,
      player: "Steve",
      raw: "先到消息 A",
      zone: "spawn",
    });
    const firstBatchRawB = JSON.stringify({
      v: 1,
      ts: 1_700_000_011,
      player: "Alex",
      raw: "先到消息 B",
      zone: "spawn",
    });
    const concurrentRaw = JSON.stringify({
      v: 1,
      ts: 1_700_000_012,
      player: "Eve",
      raw: "并发写入消息",
      zone: "spawn",
    });

    bus.pushToList(PLAYER_CHAT_KEY, firstBatchRawA);
    bus.pushToList(PLAYER_CHAT_KEY, firstBatchRawB);
    bus.concurrentWritesNextDrain = [concurrentRaw];

    const redis = new RedisIpc({
      url: "redis://fake",
      createClient: () => new FakeRedisClient(bus),
    });

    const firstDrain = await redis.drainPlayerChatRaw();
    const secondDrain = await redis.drainPlayerChatRaw();
    const thirdDrain = await redis.drainPlayerChatRaw();

    expect(firstDrain).toEqual([firstBatchRawA, firstBatchRawB]);
    expect(secondDrain).toEqual([concurrentRaw]);
    expect(thirdDrain).toEqual([]);
    expect(bus.counters.get(DRAIN_COUNTER_KEY)).toBe(2);
  });
});
