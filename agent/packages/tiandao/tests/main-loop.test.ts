import { describe, expect, it, vi } from "vitest";
import { DEFAULT_MODEL, DEFAULT_REDIS_URL, computeLoopBackoffMs, runRuntime, type RuntimeRedis } from "../src/runtime.js";
import { FakeAgent, createTestWorldState } from "./support/fakes.js";
import type { ChatMessageV1, Command, Narration } from "@bong/schema";

class FlakyRuntimeRedis implements RuntimeRedis {
  private connected = false;
  private latestState = createTestWorldState();
  private readonly connectFailuresBeforeSuccess: number;
  private readonly drainFailuresBeforeSuccess: number;
  private readonly publishFailuresBeforeSuccess: number;
  private connectAttempts = 0;
  private drainAttempts = 0;
  private publishAttempts = 0;

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

  async publishCommands(_source: "arbiter", _commands: Command[]): Promise<void> {
    this.publishAttempts += 1;
    if (this.publishAttempts <= this.publishFailuresBeforeSuccess) {
      throw new Error("publish command failed");
    }
  }

  async publishNarrations(_narrations: Narration[]): Promise<void> {}

  async disconnect(): Promise<void> {
    this.connected = false;
  }
}

describe("main-loop runtime resilience", () => {
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
        createClient: () => ({ chat: vi.fn(async () => "[]") }),
        agents: [new FakeAgent("calamity", { commands: [], narrations: [], reasoning: "ok" })],
        sleep,
        logger,
        maxLoopIterations: 4,
      },
    );

    expect(logger.warn).toHaveBeenCalled();
    expect(logger.log).toHaveBeenCalledWith(expect.stringContaining("connected to Redis"));
    expect(logger.log).toHaveBeenCalledWith(expect.stringContaining("recovered after"));
  });

  it("survives chat-annotation failures without crashing loop", async () => {
    const sleep = vi.fn(async () => {});
    const logger = { log: vi.fn(), error: vi.fn(), warn: vi.fn() };
    const redis = new FlakyRuntimeRedis();

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
        createClient: () => ({ chat: vi.fn(async () => "[]") }),
        agents: [new FakeAgent("mutation", { commands: [command], narrations: [], reasoning: "cmd" })],
        sleep,
        logger,
        maxLoopIterations: 3,
      },
    );

    expect(logger.warn).toHaveBeenCalledWith(
      expect.stringContaining("transient loop failure #1"),
      expect.any(Error),
    );
    expect(logger.log).toHaveBeenCalledWith(expect.stringContaining("recovered after"));
  });
});
