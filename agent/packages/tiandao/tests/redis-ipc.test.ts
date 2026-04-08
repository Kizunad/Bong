import { CHANNELS } from "@bong/schema";
import type { Command } from "@bong/schema";
import { describe, expect, it, vi } from "vitest";

const publishSpy = vi.fn<(...args: unknown[]) => Promise<number>>().mockResolvedValue(0);

vi.mock("ioredis", () => {
  class MockRedis {
    subscribe = vi.fn().mockResolvedValue(undefined);
    publish = publishSpy;
    on = vi.fn();
    unsubscribe = vi.fn().mockResolvedValue(undefined);
    disconnect = vi.fn();
  }

  return {
    default: MockRedis,
  };
});

import { RedisIpc } from "../src/redis-ipc.js";

class FakeSubClient {
  listener: ((channel: string, message: string) => void) | null = null;
  private readonly lifecycleListeners = new Map<string, Array<(...args: unknown[]) => void>>();
  subscribe = vi.fn().mockResolvedValue(1);
  publish = vi.fn().mockResolvedValue(0);
  unsubscribe = vi.fn().mockResolvedValue(undefined);
  disconnect = vi.fn();

  on(event: string, listener: (...args: unknown[]) => void): unknown {
    if (event === "message") {
      this.listener = listener as (channel: string, message: string) => void;
      return undefined;
    }

    const existing = this.lifecycleListeners.get(event) ?? [];
    existing.push(listener);
    this.lifecycleListeners.set(event, existing);
    return undefined;
  }

  async eval(): Promise<unknown> {
    return [];
  }

  emit(channel: string, message: string): void {
    this.listener?.(channel, message);
  }

  emitLifecycle(event: "close" | "reconnecting" | "ready" | "error", ...args: unknown[]): void {
    const listeners = this.lifecycleListeners.get(event) ?? [];
    for (const listener of listeners) {
      listener(...args);
    }
  }
}

class FakePubClient {
  private readonly lifecycleListeners = new Map<string, Array<(...args: unknown[]) => void>>();
  subscribe = vi.fn().mockResolvedValue(1);
  publish = vi.fn().mockResolvedValue(0);
  unsubscribe = vi.fn().mockResolvedValue(undefined);
  disconnect = vi.fn();

  on(event: string, listener: (...args: unknown[]) => void): unknown {
    const existing = this.lifecycleListeners.get(event) ?? [];
    existing.push(listener);
    this.lifecycleListeners.set(event, existing);
    return undefined;
  }

  async eval(): Promise<unknown> {
    return [];
  }

  emitLifecycle(event: "close" | "reconnecting" | "ready" | "error", ...args: unknown[]): void {
    const listeners = this.lifecycleListeners.get(event) ?? [];
    for (const listener of listeners) {
      listener(...args);
    }
  }
}

describe("RedisIpc publish payload", () => {
  it("omits source and strips private _source from public command payload", async () => {
    publishSpy.mockClear();

    const redis = new RedisIpc({ url: "redis://127.0.0.1:6379" });
    await redis.publishCommands("merged", [
      {
        type: "spawn_event",
        target: "blood_valley",
        _source: "calamity",
        params: {
          event: "beast_tide",
          intensity: 0.7,
        },
      },
    ]);

    expect(publishSpy).toHaveBeenCalledOnce();
    const [, jsonPayload] = publishSpy.mock.calls[0] ?? [];
    expect(typeof jsonPayload).toBe("string");

    const parsed = JSON.parse(String(jsonPayload)) as {
      source?: unknown;
      commands: Array<{ _source?: unknown; params: Record<string, unknown> }>;
    };

    expect(parsed.source).toBeUndefined();
    expect(parsed.commands[0]?._source).toBeUndefined();
    expect(parsed.commands[0]?.params?._source).toBeUndefined();
  });

  it("strips nested private _source fields from params objects and arrays", async () => {
    publishSpy.mockClear();

    const redis = new RedisIpc({ url: "redis://127.0.0.1:6379" });
    await redis.publishCommands("merged", [
      {
        type: "spawn_event",
        target: "blood_valley",
        params: {
          event: "beast_tide",
          intensity: 0.8,
          meta: {
            _source: "era",
            nested: {
              _source: "mutation",
              level: 2,
            },
          },
          effects: [
            {
              kind: "storm",
              _source: "calamity",
            },
          ],
        },
      },
    ]);

    const [, jsonPayload] = publishSpy.mock.calls[0] ?? [];
    const parsed = JSON.parse(String(jsonPayload)) as {
      commands: Array<{ params: Record<string, unknown> }>;
    };

    const params = parsed.commands[0]?.params;
    expect(params?._source).toBeUndefined();
    expect((params?.meta as Record<string, unknown>)?._source).toBeUndefined();
    expect(
      ((params?.meta as Record<string, unknown>)?.nested as Record<string, unknown>)?._source,
    ).toBeUndefined();
    const effects = params?.effects as Array<Record<string, unknown>>;
    expect(effects[0]?._source).toBeUndefined();
  });

  it("drops invalid world_state json and schema-invalid payload with structured warnings", async () => {
    const subClient = new FakeSubClient();
    const pubClient = new FakePubClient();
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => undefined);

    try {
      const redis = new RedisIpc({
        url: "redis://fake",
        createClient: (() => {
          let index = 0;
          return () => {
            index += 1;
            return index === 1 ? subClient : pubClient;
          };
        })(),
      });

      const receivedStates: Array<{ v: number; tick: number }> = [];
      redis.onWorldState((state) => {
        receivedStates.push({ v: state.v, tick: state.tick });
      });

      await redis.connect();

      subClient.emit(CHANNELS.WORLD_STATE, "{invalid_json");
      subClient.emit(
        CHANNELS.WORLD_STATE,
        JSON.stringify({
          v: 1,
          ts: 1_700_000_001,
          tick: "not-a-number",
          players: [],
          npcs: [],
          zones: [],
          recent_events: [],
        }),
      );

      const validState = {
        v: 1,
        ts: 1_700_000_002,
        tick: 42,
        players: [],
        npcs: [],
        zones: [],
        recent_events: [],
      };
      subClient.emit(CHANNELS.WORLD_STATE, JSON.stringify(validState));

      expect(receivedStates).toEqual([{ v: 1, tick: 42 }]);

      expect(warnSpy).toHaveBeenCalledWith(
        "[redis-ipc] drop invalid world_state payload",
        expect.objectContaining({ reason: "invalid_json", channel: CHANNELS.WORLD_STATE }),
      );

      expect(warnSpy).toHaveBeenCalledWith(
        "[redis-ipc] drop invalid world_state payload",
        expect.objectContaining({
          reason: "schema_invalid",
          channel: CHANNELS.WORLD_STATE,
          errors: expect.arrayContaining([expect.stringContaining("/tick")]),
        }),
      );
    } finally {
      warnSpy.mockRestore();
    }
  });

  it("skips publishing schema-invalid command payload", async () => {
    publishSpy.mockClear();
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => undefined);

    try {
      const redis = new RedisIpc({ url: "redis://127.0.0.1:6379" });
      const invalidCommands = [
        {
          type: "invalid_command_type",
          target: "blood_valley",
          params: {
            event: "beast_tide",
          },
        } as unknown as Command,
      ];

      await redis.publishCommands("merged", invalidCommands);

      expect(publishSpy).not.toHaveBeenCalled();
      expect(warnSpy).toHaveBeenCalledWith(
        "[redis-ipc] skip invalid agent_command payload",
        expect.objectContaining({
          reason: "schema_invalid",
          command_count: 1,
          errors: expect.arrayContaining([expect.stringContaining("/commands/0/type")]),
        }),
      );
    } finally {
      warnSpy.mockRestore();
    }
  });

  it("skips publishing schema-invalid narration payload", async () => {
    publishSpy.mockClear();
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => undefined);

    try {
      const redis = new RedisIpc({ url: "redis://127.0.0.1:6379" });
      await redis.publishNarrations([
        {
          scope: "zone",
          style: "narration",
          target: "blood_valley",
          text: "x".repeat(600),
        },
      ]);

      expect(publishSpy).not.toHaveBeenCalled();
      expect(warnSpy).toHaveBeenCalledWith(
        "[redis-ipc] skip invalid narration payload",
        expect.objectContaining({
          reason: "schema_invalid",
          narration_count: 1,
          errors: expect.arrayContaining([expect.stringContaining("/narrations/0/text")]),
        }),
      );
    } finally {
      warnSpy.mockRestore();
    }
  });

  it("logs redis lifecycle events for sub and pub clients", async () => {
    const subClient = new FakeSubClient();
    const pubClient = new FakePubClient();
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const logSpy = vi.spyOn(console, "log").mockImplementation(() => undefined);

    try {
      const redis = new RedisIpc({
        url: "redis://fake",
        createClient: (() => {
          let index = 0;
          return () => {
            index += 1;
            return index === 1 ? subClient : pubClient;
          };
        })(),
      });

      await redis.connect();

      subClient.emitLifecycle("ready");
      pubClient.emitLifecycle("ready");
      subClient.emitLifecycle("close");
      pubClient.emitLifecycle("close");
      subClient.emitLifecycle("reconnecting", 250);
      pubClient.emitLifecycle("reconnecting", 500);
      subClient.emitLifecycle("error", new Error("sub boom"));
      pubClient.emitLifecycle("error", new Error("pub boom"));

      expect(logSpy).toHaveBeenCalledWith(
        "[redis-ipc] redis connection ready",
        expect.objectContaining({ client: "sub" }),
      );
      expect(logSpy).toHaveBeenCalledWith(
        "[redis-ipc] redis connection ready",
        expect.objectContaining({ client: "pub" }),
      );

      expect(warnSpy).toHaveBeenCalledWith(
        "[redis-ipc] redis connection closed",
        expect.objectContaining({ client: "sub" }),
      );
      expect(warnSpy).toHaveBeenCalledWith(
        "[redis-ipc] redis connection closed",
        expect.objectContaining({ client: "pub" }),
      );

      expect(warnSpy).toHaveBeenCalledWith(
        "[redis-ipc] redis reconnecting",
        expect.objectContaining({ client: "sub", delay_ms: 250 }),
      );
      expect(warnSpy).toHaveBeenCalledWith(
        "[redis-ipc] redis reconnecting",
        expect.objectContaining({ client: "pub", delay_ms: 500 }),
      );

      expect(warnSpy).toHaveBeenCalledWith(
        "[redis-ipc] redis connection error",
        expect.objectContaining({ client: "sub", error: "sub boom" }),
      );
      expect(warnSpy).toHaveBeenCalledWith(
        "[redis-ipc] redis connection error",
        expect.objectContaining({ client: "pub", error: "pub boom" }),
      );
    } finally {
      warnSpy.mockRestore();
      logSpy.mockRestore();
    }
  });
});
