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
    const [chatKey, counterKey] = args;
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
