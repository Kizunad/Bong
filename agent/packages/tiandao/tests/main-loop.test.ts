import { describe, expect, it, vi } from "vitest";
import { mkdtemp, mkdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  DEFAULT_MODEL,
  DEFAULT_REDIS_URL,
  computeLoopBackoffMs,
  runRuntime,
  type CommandPublishRequest,
  type NarrationPublishRequest,
  type RuntimeRedis,
} from "../src/runtime.js";
import { FakeAgent, createTestWorldState } from "./support/fakes.js";
import type { ChatMessageV1 } from "@bong/schema";

function createStructuredChatResult(content: string, model: string) {
  return {
    content,
    durationMs: 0,
    requestId: "loop-test-request-id",
    model,
  };
}

class FlakyRuntimeRedis implements RuntimeRedis {
  private connected = false;
  private latestState = createTestWorldState();
  private readonly connectFailuresBeforeSuccess: number;
  private readonly drainFailuresBeforeSuccess: number;
  private readonly publishFailuresBeforeSuccess: number;
  private connectAttempts = 0;
  private drainAttempts = 0;
  private publishAttempts = 0;
  public readonly publishedCommandTicks: number[] = [];

  constructor(args: {
    connectFailuresBeforeSuccess?: number;
    drainFailuresBeforeSuccess?: number;
    publishFailuresBeforeSuccess?: number;
  } = {}) {
    this.connectFailuresBeforeSuccess = args.connectFailuresBeforeSuccess ?? 0;
    this.drainFailuresBeforeSuccess = args.drainFailuresBeforeSuccess ?? 0;
    this.publishFailuresBeforeSuccess = args.publishFailuresBeforeSuccess ?? 0;
  }

  async connect(): Promise<void> {
    this.connectAttempts += 1;
    if (this.connectAttempts <= this.connectFailuresBeforeSuccess) {
      throw new Error("redis connect failed");
    }
    this.connected = true;
  }

  getLatestState() {
    return this.connected ? this.latestState : null;
  }

  async drainPlayerChat(): Promise<ChatMessageV1[]> {
    this.drainAttempts += 1;
    if (this.drainAttempts <= this.drainFailuresBeforeSuccess) {
      throw new Error("redis drain failed");
    }
    return [
      {
        v: 1,
        ts: 1711111111,
        player: "offline:Steve",
        raw: "灵气太少了",
        zone: "spawn",
      },
    ];
  }

  async publishCommands(request: CommandPublishRequest): Promise<void> {
    this.publishAttempts += 1;
    this.publishedCommandTicks.push(request.metadata.sourceTick);
    if (this.publishAttempts <= this.publishFailuresBeforeSuccess) {
      throw new Error("publish command failed");
    }
  }

  async publishNarrations(_request: NarrationPublishRequest): Promise<void> {}

  async disconnect(): Promise<void> {
    this.connected = false;
  }
}

describe("main-loop runtime resilience", () => {
  async function withIsolatedCwd<T>(run: () => Promise<T>): Promise<T> {
    const tempDir = await mkdtemp(join(tmpdir(), "tiandao-main-loop-"));
    const previousCwd = process.cwd();

    try {
      process.chdir(tempDir);
      await mkdir(join(tempDir, "data"), { recursive: true });
      return await run();
    } finally {
      process.chdir(previousCwd);
      await rm(tempDir, { recursive: true, force: true });
    }
  }

  it("retries with bounded exponential backoff", () => {
    expect(computeLoopBackoffMs(1)).toBe(1000);
    expect(computeLoopBackoffMs(2)).toBe(2000);
    expect(computeLoopBackoffMs(3)).toBe(4000);
    expect(computeLoopBackoffMs(10)).toBe(30000);
  });

  it("survives transient redis connect failures and then recovers", async () => {
    const sleep = vi.fn(async () => {});
    const logger = { log: vi.fn(), error: vi.fn(), warn: vi.fn() };
    const redis = new FlakyRuntimeRedis({ connectFailuresBeforeSuccess: 2 });

    await withIsolatedCwd(async () => {
      await runRuntime(
        {
          mockMode: false,
          model: DEFAULT_MODEL,
          redisUrl: DEFAULT_REDIS_URL,
          baseUrl: "https://llm.example.test/v1",
          apiKey: "k_test",
        },
        {
          createRedis: () => redis,
          createClient: () => ({
            chat: vi.fn(async (model: string) => createStructuredChatResult("[]", model)),
          }),
          agents: [new FakeAgent("calamity", { commands: [], narrations: [], reasoning: "ok" })],
          sleep,
          logger,
          maxLoopIterations: 4,
        },
      );
    });

    expect(logger.warn).toHaveBeenCalled();
    expect(logger.log).toHaveBeenCalledWith(expect.stringContaining("connected to Redis"));
    expect(logger.log).toHaveBeenCalledWith(expect.stringContaining("recovered after"));
  });

  it("survives chat-annotation failures without crashing loop", async () => {
    const sleep = vi.fn(async () => {});
    const logger = { log: vi.fn(), error: vi.fn(), warn: vi.fn() };
    const redis = new FlakyRuntimeRedis();

    await withIsolatedCwd(async () => {
      await runRuntime(
        {
          mockMode: false,
          model: DEFAULT_MODEL,
          redisUrl: DEFAULT_REDIS_URL,
          baseUrl: "https://llm.example.test/v1",
          apiKey: "k_test",
        },
        {
          createRedis: () => redis,
          createClient: () => ({
            chat: vi.fn(async () => {
              throw new Error("llm 500");
            }),
          }),
          agents: [new FakeAgent("calamity", { commands: [], narrations: [], reasoning: "ok" })],
          sleep,
          logger,
          maxLoopIterations: 2,
        },
      );
    });

    expect(logger.warn).toHaveBeenCalledWith(
      "[tiandao] chat signal processing failed, keeping previous snapshot:",
      expect.any(Error),
    );
    expect(logger.log).toHaveBeenCalledWith("[tiandao] stopped");
  });

  it("keeps loop alive when runTick path throws due to publish failure", async () => {
    const sleep = vi.fn(async () => {});
    const logger = { log: vi.fn(), error: vi.fn(), warn: vi.fn() };
    const redis = new FlakyRuntimeRedis({ publishFailuresBeforeSuccess: 1 });

    const command = {
      type: "modify_zone" as const,
      target: "starter_zone",
      params: { spirit_qi_delta: 0.01 },
    };

    await withIsolatedCwd(async () => {
      await runRuntime(
        {
          mockMode: false,
          model: DEFAULT_MODEL,
          redisUrl: DEFAULT_REDIS_URL,
          baseUrl: "https://llm.example.test/v1",
          apiKey: "k_test",
        },
        {
          createRedis: () => redis,
          createClient: () => ({
            chat: vi.fn(async (model: string) => createStructuredChatResult("[]", model)),
          }),
          agents: [new FakeAgent("mutation", { commands: [command], narrations: [], reasoning: "cmd" })],
          sleep,
          logger,
          maxLoopIterations: 2,
        },
      );
    });

    expect(logger.warn).toHaveBeenCalledWith(
      expect.stringContaining("transient loop failure #1"),
      expect.any(Error),
    );
    expect(redis.publishedCommandTicks).toEqual([123, 123]);
    expect(logger.log).not.toHaveBeenCalledWith(
      "[tiandao] stale_state_skip tick=123 last_processed_tick=123",
    );
    expect(logger.log).toHaveBeenCalledWith(expect.stringContaining("recovered after"));
  });

  it("consumes stale ticks only once and logs stale_state_skip before republish", async () => {
    const sleep = vi.fn(async () => {});
    const logger = { log: vi.fn(), error: vi.fn(), warn: vi.fn() };
    const repeatedState = createTestWorldState();
    let reads = 0;

    const redis: RuntimeRedis = {
      connect: async () => {},
      getLatestState() {
        reads += 1;
        if (reads < 3) {
          return repeatedState;
        }

        return {
          ...repeatedState,
          tick: 124,
          ts: repeatedState.ts + 1,
        };
      },
      drainPlayerChat: async () => [],
      publishCommands: vi.fn(async (_request: CommandPublishRequest) => {}),
      publishNarrations: vi.fn(async (_request: NarrationPublishRequest) => {}),
      disconnect: async () => {},
    };

    await withIsolatedCwd(async () => {
      await runRuntime(
        {
          mockMode: false,
          model: DEFAULT_MODEL,
          redisUrl: DEFAULT_REDIS_URL,
          baseUrl: "https://llm.example.test/v1",
          apiKey: "k_test",
        },
        {
          createRedis: () => redis,
          createClient: () => ({
            chat: vi.fn(async (model: string) => createStructuredChatResult("[]", model)),
          }),
          agents: [
            new FakeAgent("mutation", {
              commands: [{ type: "modify_zone", target: "starter_zone", params: { spirit_qi_delta: 0.01 } }],
              narrations: [],
              reasoning: "tick",
            }),
          ],
          sleep,
          logger,
          maxLoopIterations: 3,
        },
      );
    });

    expect(redis.publishCommands).toHaveBeenCalledTimes(2);
    expect(redis.publishCommands).toHaveBeenNthCalledWith(
      1,
      expect.objectContaining({
        metadata: { sourceTick: 123, correlationId: "tiandao-tick-123" },
      }),
    );
    expect(redis.publishCommands).toHaveBeenNthCalledWith(
      2,
      expect.objectContaining({
        metadata: { sourceTick: 124, correlationId: "tiandao-tick-124" },
      }),
    );
    expect(logger.log).toHaveBeenCalledWith(
      "[tiandao] stale_state_skip tick=123 last_processed_tick=123",
    );
  });
});
