import { describe, expect, it, vi } from "vitest";
import { RedisIpc } from "../src/redis-ipc.js";
import { CHANNELS } from "@bong/schema";

const { PLAYER_CHAT, WORLD_STATE } = CHANNELS;

interface FakeMultiResult {
  lrange: string[];
  writesDuringExec?: string[];
}

class FakeRedisListClient {
  private readonly lists = new Map<string, string[]>();
  private readonly subscribers = new Map<string, Array<(channel: string, message: string) => void>>();
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
    const listeners = this.subscribers.get(channel) ?? [];
    for (const listener of listeners) {
      listener(channel, message);
    }
    return listeners.length;
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
});
