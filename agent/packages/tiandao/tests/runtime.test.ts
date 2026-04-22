import { describe, expect, it, vi } from "vitest";
import { mkdir, mkdtemp, readdir, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import {
  DEFAULT_MODEL,
  DEFAULT_REDIS_URL,
  NoopTelemetrySink,
  computeLoopBackoffMs,
  createRuntimeClients,
  createRuntimeClient,
  resolveRuntimeConfig,
  runTick,
  runRuntime,
  type RuntimeModelOverrides,
  type CommandPublishRequest,
  type NarrationPublishRequest,
  type RuntimeRedis,
} from "../src/runtime.js";
import { LlmBackoffError, LlmTimeoutError, type LlmClient } from "../src/llm.js";
import type { TelemetrySink } from "../src/telemetry.js";
import { WorldModel, type WorldModelSnapshot } from "../src/world-model.js";
import { FakeAgent, createTestWorldState } from "./support/fakes.js";
import type { AgentWorldModelEnvelopeV1, ChatMessageV1, Command, Narration } from "@bong/schema";

function createStructuredChatResult(content: string, model: string) {
  return {
    content,
    durationMs: 0,
    requestId: "test-request-id",
    model,
  };
}

class StructuredFakeLlmClient implements LlmClient {
  constructor(private readonly response: string) {}

  async chat(model: string) {
    return createStructuredChatResult(this.response, model);
  }
}

class ChatAwareFakeAgent extends FakeAgent {
  public receivedChatSignalsCount = 0;

  setChatSignals(signals: { player: string }[]): void {
    this.receivedChatSignalsCount = signals.length;
  }
}

class SequenceRuntimeRedis implements RuntimeRedis {
  public readonly connect = vi.fn(async () => {});
  public readonly disconnect = vi.fn(async () => {});
  public readonly drainPlayerChat = vi.fn(async (): Promise<ChatMessageV1[]> => []);
  public readonly loadWorldModelState = vi.fn(async (): Promise<WorldModelSnapshot | null> => {
    if (this.worldModelSnapshots.length === 0) {
      return null;
    }

    const picked = this.worldModelSnapshots[Math.min(this.worldModelSnapshotIndex, this.worldModelSnapshots.length - 1)] ?? null;
    this.worldModelSnapshotIndex += 1;
    return picked;
  });
  public readonly publishCommands = vi.fn(async (_request: CommandPublishRequest) => {});
  public readonly publishNarrations = vi.fn(async (_request: NarrationPublishRequest) => {});
  public readonly publishAgentWorldModel = vi.fn(
    async (_request: {
      source: NonNullable<AgentWorldModelEnvelopeV1["source"]>;
      snapshot: AgentWorldModelEnvelopeV1["snapshot"];
      metadata: { sourceTick: number; correlationId: string };
    }) => {},
  );
  private index = 0;
  private worldModelSnapshotIndex = 0;

  constructor(
    private readonly states: Array<ReturnType<typeof createTestWorldState> | null>,
    private readonly worldModelSnapshots: Array<WorldModelSnapshot | null> = [],
  ) {}

  getLatestState() {
    const picked = this.states[Math.min(this.index, this.states.length - 1)] ?? null;
    this.index += 1;
    return picked;
  }
}

function createMirrorSnapshot(overrides: Partial<WorldModelSnapshot> = {}): WorldModelSnapshot {
  return {
    currentEra: null,
    zoneHistory: {},
    lastDecisions: {},
    playerFirstSeenTick: {},
    lastTick: null,
    lastStateTs: null,
    ...overrides,
  };
}

class FailingPublishRuntimeRedis extends SequenceRuntimeRedis {
  public publishAttempts = 0;

  constructor(
    states: Array<ReturnType<typeof createTestWorldState> | null>,
    private readonly failOnAttempt = 1,
  ) {
    super(states);
  }

  override readonly publishCommands = vi.fn(async (_request: CommandPublishRequest) => {
    this.publishAttempts += 1;
    if (this.publishAttempts === this.failOnAttempt) {
      throw new Error("publish command failed");
    }
  });
}

  describe("resolveRuntimeConfig", () => {
  it("uses mock mode and defaults when env is missing", () => {
    const config = resolveRuntimeConfig(["node", "src/main.ts", "--mock"], {});

    expect(config.mockMode).toBe(true);
    expect(config.model).toBe(DEFAULT_MODEL);
    expect(config.redisUrl).toBe(DEFAULT_REDIS_URL);
    expect(config.baseUrl).toBeNull();
    expect(config.apiKey).toBeNull();
  });

describe("computeLoopBackoffMs", () => {
  it("returns base delay for non-positive failure streaks", () => {
    expect(computeLoopBackoffMs(0)).toBe(1_000);
    expect(computeLoopBackoffMs(-1)).toBe(1_000);
  });
});

  it("reads runtime env in non-mock mode", () => {
    const config = resolveRuntimeConfig(["node", "src/main.ts"], {
      LLM_MODEL: DEFAULT_MODEL,
      LLM_MODEL_ANNOTATE: DEFAULT_MODEL,
      LLM_MODEL_CALAMITY: DEFAULT_MODEL,
      LLM_MODEL_MUTATION: DEFAULT_MODEL,
      LLM_MODEL_ERA: "gpt-5.4",
      REDIS_URL: "redis://mock:6379",
      LLM_BASE_URL: "https://llm.example.test/v1",
      LLM_API_KEY: "k_test",
    });

    expect(config.mockMode).toBe(false);
    expect(config.model).toBe(DEFAULT_MODEL);
    expect(config.modelOverrides).toEqual({
      default: DEFAULT_MODEL,
      annotate: DEFAULT_MODEL,
      calamity: DEFAULT_MODEL,
      mutation: DEFAULT_MODEL,
      era: "gpt-5.4",
    });
    expect(config.redisUrl).toBe("redis://mock:6379");
    expect(config.baseUrl).toBe("https://llm.example.test/v1");
    expect(config.apiKey).toBe("k_test");
  });

  it("fails fast when runtime override model is outside allowlist", () => {
    expect(() =>
      resolveRuntimeConfig(["node", "src/main.ts"], {
        LLM_MODEL: DEFAULT_MODEL,
        LLM_MODEL_ERA: "unsupported-model",
      }),
    ).toThrow(/invalid model override for role 'era'/);
  });
});

describe("createRuntimeClient", () => {
  it("uses mock client in mock mode even without env", async () => {
    const chat = vi.fn(async (model: string) => createStructuredChatResult("mock", model));
    const client = createRuntimeClient(
      {
        mockMode: true,
        model: DEFAULT_MODEL,
        redisUrl: DEFAULT_REDIS_URL,
        baseUrl: null,
        apiKey: null,
      },
      {
        createMockClient: () => ({ chat }),
      },
    );

    expect(await client.chat(DEFAULT_MODEL, [])).toEqual(
      createStructuredChatResult("mock", DEFAULT_MODEL),
    );
    expect(chat).toHaveBeenCalledTimes(1);
  });

  it("does not evaluate real-client factory in mock mode", async () => {
    const createClient = vi.fn(() => {
      throw new Error("real client should not be created in mock mode");
    });
    const mockChat = vi.fn(async (model: string) => createStructuredChatResult("mock-only", model));

    const client = createRuntimeClient(
      {
        mockMode: true,
        model: DEFAULT_MODEL,
        redisUrl: DEFAULT_REDIS_URL,
        baseUrl: null,
        apiKey: null,
      },
      {
        createClient,
        createMockClient: () => ({ chat: mockChat }),
      },
    );

    expect(await client.chat(DEFAULT_MODEL, [])).toEqual(
      createStructuredChatResult("mock-only", DEFAULT_MODEL),
    );
    expect(mockChat).toHaveBeenCalledTimes(1);
    expect(createClient).not.toHaveBeenCalled();
  });

  it("throws when non-mock mode lacks LLM env", () => {
    expect(() =>
      createRuntimeClient({
        mockMode: false,
        model: DEFAULT_MODEL,
        redisUrl: DEFAULT_REDIS_URL,
        baseUrl: null,
        apiKey: null,
      }),
    ).toThrow(/Missing LLM_BASE_URL or LLM_API_KEY/);
  });

  it("creates isolated clients for every fixed routing role", () => {
    const createdModels: string[] = [];
    const roleModels = [DEFAULT_MODEL, DEFAULT_MODEL, DEFAULT_MODEL, DEFAULT_MODEL, "gpt-5.4"];
    const createClient = vi.fn(() => {
      const createdModel = roleModels[createdModels.length] ?? DEFAULT_MODEL;
      createdModels.push(createdModel);
      return {
        chat: vi.fn(async (requestedModel: string) =>
          createStructuredChatResult(
            JSON.stringify({ commands: [], narrations: [], reasoning: createdModel }),
            requestedModel,
          ),
        ),
      };
    });

    const clients = createRuntimeClients(
      {
        mockMode: false,
        model: DEFAULT_MODEL,
        modelOverrides: {
          default: DEFAULT_MODEL,
          annotate: DEFAULT_MODEL,
          calamity: DEFAULT_MODEL,
          mutation: DEFAULT_MODEL,
          era: "gpt-5.4",
        },
        redisUrl: DEFAULT_REDIS_URL,
        baseUrl: "https://llm.example.test/v1",
        apiKey: "k_test",
      },
      { createClient },
    );

    expect(createClient).toHaveBeenCalledTimes(5);
    expect(createdModels).toEqual([DEFAULT_MODEL, DEFAULT_MODEL, DEFAULT_MODEL, DEFAULT_MODEL, "gpt-5.4"]);
    expect(new Set(Object.values(clients)).size).toBe(5);
  });

  it("creates isolated mock clients for every fixed routing role", () => {
    const createMockClient = vi.fn(() => ({ chat: vi.fn(async (model: string) => createStructuredChatResult("{}", model)) }));

    const clients = createRuntimeClients(
      {
        mockMode: true,
        model: DEFAULT_MODEL,
        modelOverrides: {
          default: DEFAULT_MODEL,
          annotate: DEFAULT_MODEL,
          calamity: DEFAULT_MODEL,
          mutation: DEFAULT_MODEL,
          era: "gpt-5.4",
        },
        redisUrl: DEFAULT_REDIS_URL,
        baseUrl: null,
        apiKey: null,
      },
      { createMockClient },
    );

    expect(createMockClient).toHaveBeenCalledTimes(5);
    expect(new Set(Object.values(clients)).size).toBe(5);
  });
});

describe("runTick", () => {
  it("publishes one merged command batch and one merged narration batch", async () => {
    const state = createTestWorldState();
    const publishCommands = vi.fn(async () => {});
    const publishNarrations = vi.fn(async () => {});
    const logger = {
      log: vi.fn(),
      error: vi.fn(),
    };

    const command: Command = {
      type: "modify_zone",
      target: "starter_zone",
      params: { spirit_qi_delta: 0.1 },
    };
    const narration: Narration = {
      scope: "zone",
      target: "starter_zone",
      text: "灵气微升",
      style: "narration",
    };

    const result = await runTick(state, {
      agents: [
        new FakeAgent("mutation", {
          commands: [command],
          narrations: [narration],
          reasoning: "test",
        }),
      ],
      llmClient: new StructuredFakeLlmClient("{}"),
      model: DEFAULT_MODEL,
      publishCommands,
      publishNarrations,
      logger,
    });

    expect(publishCommands).toHaveBeenCalledTimes(1);
    expect(publishNarrations).toHaveBeenCalledTimes(1);
    expect(publishCommands).toHaveBeenCalledWith(
      expect.objectContaining({
        source: "arbiter",
        metadata: {
          sourceTick: 123,
          correlationId: "tiandao-tick-123",
        },
        commands: expect.arrayContaining([
          expect.objectContaining({
            type: "modify_zone",
            target: "starter_zone",
          }),
        ]),
      }),
    );
    expect(publishNarrations).toHaveBeenCalledWith({
      narrations: [narration],
      metadata: {
        sourceTick: 123,
        correlationId: "tiandao-tick-123",
      },
    });
    expect(result.metadata).toEqual({
      sourceTick: 123,
      correlationId: "tiandao-tick-123",
    });
    expect(result.totalCommands).toBe(1);
    expect(result.totalNarrations).toBe(1);
  });

  it("does not publish one command batch per sub-agent", async () => {
    const state = createTestWorldState();
    const publishCommands = vi.fn(async () => {});
    const publishNarrations = vi.fn(async () => {});

    await runTick(state, {
      agents: [
        new FakeAgent("calamity", {
          commands: [{ type: "modify_zone", target: "starter_zone", params: { spirit_qi_delta: -0.1 } }],
          narrations: [],
          reasoning: "c",
        }),
        new FakeAgent("mutation", {
          commands: [{ type: "modify_zone", target: "starter_zone", params: { spirit_qi_delta: 0.05 } }],
          narrations: [],
          reasoning: "m",
        }),
        new FakeAgent("era", {
          commands: [{ type: "modify_zone", target: "starter_zone", params: { spirit_qi_delta: 0.05 } }],
          narrations: [],
          reasoning: "e",
        }),
      ],
      llmClient: new StructuredFakeLlmClient("{}"),
      model: DEFAULT_MODEL,
      publishCommands,
      publishNarrations,
      logger: { log: vi.fn(), error: vi.fn() },
    });

    expect(publishCommands).toHaveBeenCalledTimes(1);
    expect(publishCommands).toHaveBeenCalledWith(
      expect.objectContaining({
        source: "arbiter",
        commands: expect.any(Array),
        metadata: expect.objectContaining({ sourceTick: 123 }),
      }),
    );
  });

  it("skips publish when agent returns null", async () => {
    const publishCommands = vi.fn(async () => {});
    const publishNarrations = vi.fn(async () => {});

    await runTick(createTestWorldState(), {
      agents: [new FakeAgent("calamity", null)],
      llmClient: new StructuredFakeLlmClient("{}"),
      model: DEFAULT_MODEL,
      publishCommands,
      publishNarrations,
      logger: { log: vi.fn(), error: vi.fn() },
    });

    expect(publishCommands).not.toHaveBeenCalled();
    expect(publishNarrations).not.toHaveBeenCalled();
  });

  it("injects drained chat signals to agents before ticking", async () => {
    const publishCommands = vi.fn(async () => {});
    const publishNarrations = vi.fn(async () => {});
    const chatAwareAgent = new ChatAwareFakeAgent("calamity", null);

    await runTick(createTestWorldState(), {
      agents: [chatAwareAgent],
      llmClient: new StructuredFakeLlmClient("{}"),
      model: DEFAULT_MODEL,
      chatSignals: [
        {
          player: "offline:Steve",
          raw: "灵气太少了",
          sentiment: -0.6,
          intent: "complaint",
          influence_weight: 0.7,
        },
      ],
      publishCommands,
      publishNarrations,
      logger: { log: vi.fn(), error: vi.fn() },
    });

    expect(chatAwareAgent.receivedChatSignalsCount).toBe(1);
  });

  it("persists current era from arbiter output into the shared world model", async () => {
    const publishCommands = vi.fn(async () => {});
    const publishNarrations = vi.fn(async () => {});
    const worldModel = new WorldModel();

    await runTick(createTestWorldState(), {
      agents: [
        new FakeAgent("era", {
          commands: [
            {
              type: "modify_zone",
              target: "全局",
              params: {
                era_name: "末法纪",
                global_effect: "灵机渐枯，诸域修行更艰",
                spirit_qi_delta: -0.02,
                danger_level_delta: 1,
              },
            },
          ],
          narrations: [
            {
              scope: "broadcast",
              text: "天地风色俱沉，旧脉将歇，新纪将临。",
              style: "era_decree",
            },
          ],
          reasoning: "declare era",
        }),
      ],
      llmClient: new StructuredFakeLlmClient("{}"),
      model: DEFAULT_MODEL,
      worldModel,
      publishCommands,
      publishNarrations,
      logger: { log: vi.fn(), error: vi.fn() },
    });

    expect(worldModel.currentEra).toEqual({
      name: "末法纪",
      sinceTick: 123,
      globalEffect: "灵机渐枯，诸域修行更艰",
    });
    expect(publishCommands).toHaveBeenCalledWith(
      expect.objectContaining({
        source: "arbiter",
        metadata: expect.objectContaining({
          sourceTick: 123,
          correlationId: "tiandao-tick-123",
        }),
        commands: expect.arrayContaining([
          expect.objectContaining({ type: "modify_zone", target: "starter_zone" }),
        ]),
      }),
    );
  });

  it("marks tick metadata even when all agents skip", async () => {
    const result = await runTick(createTestWorldState(), {
      agents: [new FakeAgent("calamity", null)],
      llmClient: new StructuredFakeLlmClient("{}"),
      model: DEFAULT_MODEL,
      publishCommands: vi.fn(async () => {}),
      publishNarrations: vi.fn(async () => {}),
      logger: { log: vi.fn(), error: vi.fn() },
    });

    expect(result.skipped).toBe(true);
    expect(result.metadata).toEqual({
      sourceTick: 123,
      correlationId: "tiandao-tick-123",
    });
  });

  it("returns structured telemetry metrics including parse failures and chat signal count", async () => {
    const parseFailAgent = {
      name: "calamity",
      tick: vi.fn(async () => ({
        commands: [],
        narrations: [],
        reasoning: "parse-failed",
        parseFailures: {
          commands: 1,
          narrations: 0,
          total: 1,
        },
      })),
    };

    const result = await runTick(createTestWorldState(), {
      agents: [parseFailAgent],
      llmClient: new StructuredFakeLlmClient("{}"),
      model: DEFAULT_MODEL,
      publishCommands: vi.fn(async () => {}),
      publishNarrations: vi.fn(async () => {}),
      chatSignals: [
        {
          player: "offline:Steve",
          raw: "灵气枯竭",
          sentiment: -0.8,
          intent: "complaint",
          influence_weight: 0.9,
        },
      ],
      staleStateSkipped: true,
      reconnectCount: 2,
      backoffCount: 1,
      logger: { log: vi.fn(), error: vi.fn() },
    });

    expect(result.metrics).toEqual(
      expect.objectContaining({
        tick: 123,
        mergedCommandCount: 0,
        mergedNarrationCount: 0,
        chatSignalCount: 1,
        eraChanged: false,
        staleStateSkipped: true,
        errorBreakdown: {
          timeout: 0,
          backoff: 1,
          parseFail: 1,
          reconnect: 2,
          dedupeDrop: 0,
        },
      }),
    );
    expect(result.metrics.agentResults).toEqual([
      expect.objectContaining({
        name: "calamity",
        status: "ok",
        commandCount: 0,
        narrationCount: 0,
        model: DEFAULT_MODEL,
        tokensEstimated: 0,
      }),
    ]);
  });

  it("routes per-agent telemetry model using fixed role overrides", async () => {
    const perRoleClients = {
      default: new StructuredFakeLlmClient("{}"),
      annotate: new StructuredFakeLlmClient("{}"),
      calamity: new StructuredFakeLlmClient("{}"),
      mutation: new StructuredFakeLlmClient("{}"),
      era: new StructuredFakeLlmClient("{}"),
    };
    const modelOverrides: RuntimeModelOverrides = {
      default: DEFAULT_MODEL,
      annotate: DEFAULT_MODEL,
      calamity: DEFAULT_MODEL,
      mutation: DEFAULT_MODEL,
      era: "gpt-5.4",
    };

    const result = await runTick(createTestWorldState(), {
      agents: [new FakeAgent("era", { commands: [], narrations: [], reasoning: "era route" })],
      llmClient: perRoleClients.default,
      llmClientsByRole: perRoleClients,
      model: DEFAULT_MODEL,
      modelOverrides,
      publishCommands: vi.fn(async () => {}),
      publishNarrations: vi.fn(async () => {}),
      logger: { log: vi.fn(), error: vi.fn() },
    });

    expect(result.metrics.agentResults).toEqual([
      expect.objectContaining({
        name: "era",
        model: "gpt-5.4",
      }),
    ]);
  });

  it("warns and continues when telemetry recordTick fails", async () => {
    const warn = vi.fn();
    const telemetrySink = {
      recordTick: vi.fn(async () => {
        throw new Error("sink record fail");
      }),
      flush: vi.fn(async () => {}),
    };

    const result = await runTick(createTestWorldState(), {
      agents: [new FakeAgent("calamity", { commands: [], narrations: [], reasoning: "ok" })],
      llmClient: new StructuredFakeLlmClient("{}"),
      model: DEFAULT_MODEL,
      publishCommands: vi.fn(async () => {}),
      publishNarrations: vi.fn(async () => {}),
      telemetrySink,
      telemetryWarnLogger: { warn },
      logger: { log: vi.fn(), error: vi.fn() },
    });

    expect(result.metrics.tick).toBe(123);
    expect(telemetrySink.recordTick).toHaveBeenCalledTimes(1);
    expect(warn).toHaveBeenCalledWith("[tiandao] telemetry recordTick failed:", expect.any(Error));
  });

  it("classifies timeout and llm backoff into telemetry errorBreakdown", async () => {
    const timeoutAgent = {
      name: "calamity",
      tick: vi.fn(async () => {
        throw new LlmTimeoutError(500);
      }),
    };
    const backoffAgent = {
      name: "mutation",
      tick: vi.fn(async () => {
        throw new LlmBackoffError(Date.now() + 1000);
      }),
    };

    const result = await runTick(createTestWorldState(), {
      agents: [timeoutAgent, backoffAgent],
      llmClient: new StructuredFakeLlmClient("{}"),
      model: DEFAULT_MODEL,
      publishCommands: vi.fn(async () => {}),
      publishNarrations: vi.fn(async () => {}),
      logger: { log: vi.fn(), error: vi.fn() },
      backoffCount: 2,
    });

    expect(result.metrics.errorBreakdown.timeout).toBe(1);
    expect(result.metrics.errorBreakdown.backoff).toBe(3);
    expect(result.metrics.errorBreakdown.parseFail).toBe(0);
    expect(result.metrics.errorBreakdown.reconnect).toBe(0);
    expect(result.metrics.errorBreakdown.dedupeDrop).toBe(0);
  });
});

describe("runRuntime", () => {
  async function withIsolatedCwd<T>(run: () => Promise<T>): Promise<T> {
    const tempDir = await mkdtemp(join(tmpdir(), "tiandao-runtime-test-"));
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

  it("completes in mock mode without Redis and without env", async () => {
    const state = createTestWorldState();
    const logger = { log: vi.fn(), error: vi.fn(), warn: vi.fn() };
    const createRedis = vi.fn((): RuntimeRedis => {
      throw new Error("redis should not be created in mock mode");
    });

    await runRuntime(
      {
        mockMode: true,
        model: DEFAULT_MODEL,
        redisUrl: DEFAULT_REDIS_URL,
        baseUrl: null,
        apiKey: null,
      },
      {
        agents: [new FakeAgent("era", { commands: [], narrations: [], reasoning: "mock" })],
        createRedis,
        createMockClient: () => ({
          chat: vi.fn(async (model: string) =>
            createStructuredChatResult(
              JSON.stringify({ commands: [], narrations: [], reasoning: "mock" }),
              model,
            ),
          ),
        }),
        loadMockState: () => state,
        logger,
      },
    );

    expect(createRedis).not.toHaveBeenCalled();
    expect(logger.log).toHaveBeenCalled();
  });

  it("returns after single mock tick without sleep", async () => {
    const sleep = vi.fn(async () => {});
    const createRedis = vi.fn((): RuntimeRedis => {
      throw new Error("redis should not be created in mock mode");
    });
    const agentTick = vi.fn(async () => ({ commands: [], narrations: [], reasoning: "single-tick" }));
    const mockAgent = {
      name: "mock-agent",
      tick: agentTick,
    };

    await runRuntime(
      {
        mockMode: true,
        model: DEFAULT_MODEL,
        redisUrl: DEFAULT_REDIS_URL,
        baseUrl: null,
        apiKey: null,
      },
      {
        agents: [mockAgent],
        createRedis,
        createMockClient: () => ({
          chat: vi.fn(async (model: string) => createStructuredChatResult("{}", model)),
        }),
        loadMockState: () => createTestWorldState(),
        sleep,
        logger: { log: vi.fn(), error: vi.fn(), warn: vi.fn() },
      },
    );

    expect(agentTick).toHaveBeenCalledTimes(1);
    expect(createRedis).not.toHaveBeenCalled();
    expect(sleep).not.toHaveBeenCalled();
  });

  it("skips stale world_state before mutating world model or publishing again", async () => {
    const staleState = createTestWorldState();
    const freshState = createTestWorldState();
    freshState.tick = 124;

    const redis = new SequenceRuntimeRedis([staleState, staleState, freshState]);
    const worldModel = new WorldModel();
    const logger = { log: vi.fn(), error: vi.fn(), warn: vi.fn() };

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
              commands: [{ type: "modify_zone", target: "starter_zone", params: { spirit_qi_delta: 0.02 } }],
              narrations: [],
              reasoning: "cmd",
            }),
          ],
          sleep: vi.fn(async () => {}),
          logger,
          worldModel,
          maxLoopIterations: 3,
        },
      );
    });

    expect(redis.publishCommands).toHaveBeenCalledTimes(2);
    expect(redis.publishCommands.mock.calls[0]?.[0]).toEqual(
      expect.objectContaining({
        metadata: expect.objectContaining({ sourceTick: 123, correlationId: "tiandao-tick-123" }),
      }),
    );
    expect(redis.publishCommands.mock.calls[1]?.[0]).toEqual(
      expect.objectContaining({
        metadata: expect.objectContaining({ sourceTick: 124, correlationId: "tiandao-tick-124" }),
      }),
    );
    expect(logger.log).toHaveBeenCalledWith(
      "[tiandao] stale_state_skip tick=123 last_processed_tick=123",
    );
    expect(worldModel.latestState?.tick).toBe(124);
  });

  it("keeps redis loop alive when telemetry sink throws", async () => {
    const firstState = createTestWorldState();
    const secondState = createTestWorldState();
    secondState.tick = 124;

    const redis = new SequenceRuntimeRedis([firstState, secondState]);
    const logger = { log: vi.fn(), error: vi.fn(), warn: vi.fn() };
    const telemetrySink = {
      recordTick: vi.fn(async () => {
        throw new Error("tick sink down");
      }),
      flush: vi.fn(async () => {}),
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
              reasoning: "ok",
            }),
          ],
          sleep: vi.fn(async () => {}),
          logger,
          telemetrySink,
          maxLoopIterations: 2,
        },
      );
    });

    expect(redis.publishCommands).toHaveBeenCalledTimes(2);
    expect(logger.warn).toHaveBeenCalledWith("[tiandao] telemetry recordTick failed:", expect.any(Error));
  });

  it("warns on telemetry flush failure but still completes runtime shutdown", async () => {
    const state = createTestWorldState();
    const logger = { log: vi.fn(), error: vi.fn(), warn: vi.fn() };
    const telemetrySink = {
      recordTick: vi.fn(async () => {}),
      flush: vi.fn(async () => {
        throw new Error("flush sink down");
      }),
    };

    await runRuntime(
      {
        mockMode: true,
        model: DEFAULT_MODEL,
        redisUrl: DEFAULT_REDIS_URL,
        baseUrl: null,
        apiKey: null,
      },
      {
        agents: [new FakeAgent("era", { commands: [], narrations: [], reasoning: "mock" })],
        createMockClient: () => ({
          chat: vi.fn(async (model: string) => createStructuredChatResult("{}", model)),
        }),
        loadMockState: () => state,
        logger,
        telemetrySink,
      },
    );

    expect(telemetrySink.recordTick).toHaveBeenCalledTimes(1);
    expect(telemetrySink.flush).toHaveBeenCalledTimes(1);
    expect(logger.warn).toHaveBeenCalledWith("[tiandao] telemetry flush failed:", expect.any(Error));
  });

  it("accepts explicit NoopTelemetrySink injection", async () => {
    await runRuntime(
      {
        mockMode: true,
        model: DEFAULT_MODEL,
        redisUrl: DEFAULT_REDIS_URL,
        baseUrl: null,
        apiKey: null,
      },
      {
        agents: [new FakeAgent("era", { commands: [], narrations: [], reasoning: "noop" })],
        createMockClient: () => ({
          chat: vi.fn(async (model: string) => createStructuredChatResult("{}", model)),
        }),
        logger: { log: vi.fn(), error: vi.fn(), warn: vi.fn() },
        telemetrySink: new NoopTelemetrySink(),
      },
    );
  });

  it("counts reconnect and loop backoff into emitted tick metrics", async () => {
    const firstState = createTestWorldState();
    const secondState = createTestWorldState();
    secondState.tick = 124;

    const redis = new SequenceRuntimeRedis([firstState, secondState]);
    let drainAttempts = 0;
    redis.drainPlayerChat.mockImplementation(async () => {
      drainAttempts += 1;
      if (drainAttempts === 2) {
        throw new Error("redis drain failed once");
      }

      return [];
    });

    const captured: TelemetrySink & { ticks: Array<{ tick: number; errorBreakdown: { reconnect: number; backoff: number } }> } = {
      ticks: [],
      async recordTick(metrics) {
        this.ticks.push({
          tick: metrics.tick,
          errorBreakdown: {
            reconnect: metrics.errorBreakdown.reconnect,
            backoff: metrics.errorBreakdown.backoff,
          },
        });
      },
      async flush() {},
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
            chat: vi.fn(async (model: string) =>
              createStructuredChatResult(
                JSON.stringify({ commands: [], narrations: [], reasoning: "ok" }),
                model,
              ),
            ),
          }),
          agents: [new FakeAgent("calamity", { commands: [], narrations: [], reasoning: "ok" })],
          sleep: vi.fn(async () => {}),
          logger: { log: vi.fn(), error: vi.fn(), warn: vi.fn() },
          maxLoopIterations: 3,
          telemetrySink: captured,
        },
      );
    });

    const tick124 = captured.ticks.find((entry) => entry.tick === 124);
    expect(tick124?.errorBreakdown.backoff).toBe(1);
    expect(tick124?.errorBreakdown.reconnect).toBe(1);
  });

  it("uses isolated annotate and per-agent clients with fixed routed models", async () => {
    const state = createTestWorldState();
    const redis = new SequenceRuntimeRedis([state]);
    redis.drainPlayerChat.mockImplementation(async () => [
      {
        v: 1,
        ts: 1711111111,
        player: "offline:Steve",
        raw: "灵气太少了",
        zone: "spawn",
      },
    ]);

    const defaultChat = vi.fn(async (model: string) =>
      createStructuredChatResult(JSON.stringify({ commands: [], narrations: [], reasoning: "default" }), model),
    );
    const annotateChat = vi.fn(async (model: string) =>
      createStructuredChatResult(
        JSON.stringify([
          {
            player: "offline:Steve",
            zone: "spawn",
            raw: "灵气太少了",
            sentiment: -0.7,
            intent: "complaint",
            influence_weight: 0.8,
          },
        ]),
        model,
      ),
    );
    const calamityChat = vi.fn(async (model: string) =>
      createStructuredChatResult(JSON.stringify({ commands: [], narrations: [], reasoning: "calamity" }), model),
    );
    const mutationChat = vi.fn(async (model: string) =>
      createStructuredChatResult(JSON.stringify({ commands: [], narrations: [], reasoning: "mutation" }), model),
    );
    const eraChat = vi.fn(async (model: string) =>
      createStructuredChatResult(JSON.stringify({ commands: [], narrations: [], reasoning: "era" }), model),
    );

    const createdClients: LlmClient[] = [];
    const createClient = vi.fn(() => {
      if (createdClients.length === 0) {
        const client = { chat: defaultChat };
        createdClients.push(client);
        return client;
      }
      if (createdClients.length === 1) {
        const client = { chat: annotateChat };
        createdClients.push(client);
        return client;
      }
      if (createdClients.length === 2) {
        const client = { chat: calamityChat };
        createdClients.push(client);
        return client;
      }
      if (createdClients.length === 3) {
        const client = { chat: mutationChat };
        createdClients.push(client);
        return client;
      }

      const client = { chat: eraChat };
      createdClients.push(client);
      return client;
    });

    await withIsolatedCwd(async () => {
      await runRuntime(
        {
          mockMode: false,
          model: DEFAULT_MODEL,
          modelOverrides: {
            default: DEFAULT_MODEL,
            annotate: DEFAULT_MODEL,
            calamity: DEFAULT_MODEL,
            mutation: DEFAULT_MODEL,
            era: "gpt-5.4",
          },
          redisUrl: DEFAULT_REDIS_URL,
          baseUrl: "https://llm.example.test/v1",
          apiKey: "k_test",
        },
        {
          createRedis: () => redis,
          createClient,
          sleep: vi.fn(async () => {}),
          logger: { log: vi.fn(), error: vi.fn(), warn: vi.fn() },
          maxLoopIterations: 1,
        },
      );
    });

    expect(createClient).toHaveBeenCalledTimes(5);
    expect(new Set(createdClients).size).toBe(5);
    expect(defaultChat).not.toHaveBeenCalled();
    expect(annotateChat).toHaveBeenCalledWith(DEFAULT_MODEL, expect.any(Array));
    expect(calamityChat).toHaveBeenCalledWith(
      DEFAULT_MODEL,
      expect.any(Array),
      expect.objectContaining({
        tools: expect.any(Array),
        toolContext: expect.objectContaining({
          latestState: expect.objectContaining({ tick: state.tick }),
        }),
      }),
    );
    expect(mutationChat).toHaveBeenCalledWith(
      DEFAULT_MODEL,
      expect.any(Array),
      expect.objectContaining({
        tools: expect.any(Array),
        toolContext: expect.objectContaining({
          latestState: expect.objectContaining({ tick: state.tick }),
        }),
      }),
    );
    expect(eraChat).toHaveBeenCalledWith("gpt-5.4", expect.any(Array));
  });

  it("preserves preloaded worldModel state while publishing fresh ticks", async () => {
    const staleState = createTestWorldState();
    staleState.tick = 188;
    const freshState = createTestWorldState();
    freshState.tick = 200;
    const redis = new SequenceRuntimeRedis([staleState, freshState]);
    const logger = { log: vi.fn(), error: vi.fn(), warn: vi.fn() };
    const worldModel = new WorldModel();
    worldModel.restoreFromJSON({
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
          reasoning: "restore",
        },
      },
      playerFirstSeenTick: {
        "offline:test-player": 188,
      },
      lastTick: 188,
      lastStateTs: staleState.ts,
    });

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
        agents: [new FakeAgent("mutation", { commands: [], narrations: [], reasoning: "ok" })],
        sleep: vi.fn(async () => {}),
        logger,
        worldModel,
        maxLoopIterations: 2,
      },
    );

    expect(worldModel.currentEra?.name).toBe("末法纪");
    expect(worldModel.lastTick).toBe(200);
    expect(logger.log).not.toHaveBeenCalledWith("[tiandao] restored state from tick 188, era: 末法纪");
    expect(redis.publishAgentWorldModel).toHaveBeenCalledTimes(2);
    expect(redis.publishAgentWorldModel.mock.calls[0]?.[0]?.snapshot.lastTick).toBe(188);
    expect(redis.publishAgentWorldModel.mock.calls[1]?.[0]?.snapshot.lastTick).toBe(200);
    expect(redis.publishAgentWorldModel.mock.calls[1]?.[0]?.snapshot.lastStateTs).toBe(freshState.ts);
  });

  it("restores world model from redis mirror on startup without using local snapshot files", async () => {
    const logger = { log: vi.fn(), error: vi.fn(), warn: vi.fn() };
    const worldModel = new WorldModel();
    const redis = new SequenceRuntimeRedis([null], [
      createMirrorSnapshot({
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
            reasoning: "restore-from-mirror",
          },
        },
        playerFirstSeenTick: {
          "offline:test-player": 188,
        },
        lastTick: 188,
        lastStateTs: 1_711_111_188,
      }),
    ]);

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
        agents: [new FakeAgent("mutation", { commands: [], narrations: [], reasoning: "idle" })],
        sleep: vi.fn(async () => {}),
        logger,
        worldModel,
        maxLoopIterations: 1,
      },
    );

    expect(worldModel.currentEra?.name).toBe("末法纪");
    expect(worldModel.lastTick).toBe(188);
    expect(redis.publishAgentWorldModel).not.toHaveBeenCalled();
    expect(logger.log).toHaveBeenCalledWith(
      "[tiandao] restored world model from redis mirror tick=188, era: 末法纪 (startup)",
    );
  });

  it("does not restore older redis mirror over fresher in-memory world model", async () => {
    const logger = { log: vi.fn(), error: vi.fn(), warn: vi.fn() };
    const worldModel = new WorldModel();
    worldModel.restoreFromJSON(
      createMirrorSnapshot({
        currentEra: {
          name: "新纪",
          sinceTick: 400,
          globalEffect: "灵脉复涌",
        },
        lastTick: 400,
        lastStateTs: 1_711_444_400,
      }),
    );
    const redis = new SequenceRuntimeRedis([null], [
      createMirrorSnapshot({
        currentEra: {
          name: "末法纪",
          sinceTick: 188,
          globalEffect: "灵机渐枯",
        },
        lastTick: 188,
        lastStateTs: 1_711_111_188,
      }),
    ]);

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
        agents: [new FakeAgent("mutation", { commands: [], narrations: [], reasoning: "idle" })],
        sleep: vi.fn(async () => {}),
        logger,
        worldModel,
        maxLoopIterations: 1,
      },
    );

    expect(worldModel.currentEra?.name).toBe("新纪");
    expect(worldModel.lastTick).toBe(400);
    expect(logger.log).not.toHaveBeenCalledWith(
      expect.stringContaining("restored world model from redis mirror"),
    );
  });

  it("does not auto-restore startup state from local disk snapshot files", async () => {
    const tempDir = await mkdtemp(join(tmpdir(), "tiandao-runtime-snapshot-restore-"));
    const previousCwd = process.cwd();
    const logger = { log: vi.fn(), error: vi.fn(), warn: vi.fn() };

    try {
      process.chdir(tempDir);
      await mkdir(join(tempDir, "data"), { recursive: true });
      await writeFile(
        join(tempDir, "data", "tiandao-snapshot-188.json"),
        `${JSON.stringify({
          currentEra: {
            name: "末法纪",
            sinceTick: 188,
            globalEffect: "灵机渐枯",
          },
          zoneHistory: {},
          lastDecisions: {},
          lastTick: 188,
        }, null, 2)}\n`,
        "utf8",
      );

      const staleState = createTestWorldState();
      staleState.tick = 188;
      const freshState = createTestWorldState();
      freshState.tick = 200;
      const redis = new SequenceRuntimeRedis([staleState, freshState]);

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
              reasoning: "snapshot-restore",
            }),
          ],
          sleep: vi.fn(async () => {}),
          logger,
          maxLoopIterations: 2,
        },
      );

      expect(logger.log).not.toHaveBeenCalledWith("[tiandao] restored state from tick 188, era: 末法纪");
      expect(logger.warn).not.toHaveBeenCalledWith(
        "[tiandao] failed to load snapshot file tiandao-snapshot-188.json:",
        expect.any(Error),
      );
      expect(redis.publishCommands).toHaveBeenCalledTimes(2);
      expect(redis.publishCommands.mock.calls[0]?.[0]).toEqual(
        expect.objectContaining({
          metadata: expect.objectContaining({ sourceTick: 188, correlationId: "tiandao-tick-188" }),
        }),
      );
      expect(redis.publishCommands.mock.calls[1]?.[0]).toEqual(
        expect.objectContaining({
          metadata: expect.objectContaining({ sourceTick: 200, correlationId: "tiandao-tick-200" }),
        }),
      );
      expect(redis.publishAgentWorldModel).toHaveBeenCalledTimes(2);
      expect(redis.publishAgentWorldModel.mock.calls[0]?.[0]?.snapshot.lastTick).toBe(188);
      expect(redis.publishAgentWorldModel.mock.calls[1]?.[0]?.snapshot.lastTick).toBe(200);
      expect(redis.publishAgentWorldModel.mock.calls[1]?.[0]?.snapshot.lastStateTs).toBe(freshState.ts);
    } finally {
      process.chdir(previousCwd);
      await rm(tempDir, { recursive: true, force: true });
    }
  });

  it("does not seed stale cursor from preloaded worldModel on startup", async () => {
    const staleState = createTestWorldState();
    staleState.tick = 188;
    const freshState = createTestWorldState();
    freshState.tick = 200;
    freshState.ts = staleState.ts + 12;
    const redis = new SequenceRuntimeRedis([staleState, freshState]);
    const logger = { log: vi.fn(), error: vi.fn(), warn: vi.fn() };

    const worldModel = new WorldModel();
    worldModel.restoreFromJSON({
      currentEra: {
        name: "末法纪",
        sinceTick: 188,
        globalEffect: "灵机渐枯",
      },
      zoneHistory: {},
      lastDecisions: {},
      playerFirstSeenTick: {
        "offline:test-player": 188,
      },
      lastTick: 188,
      lastStateTs: staleState.ts,
    });

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
            reasoning: "snapshot-cursor",
          }),
        ],
        sleep: vi.fn(async () => {}),
        logger,
        worldModel,
        maxLoopIterations: 2,
      },
    );

    expect(logger.log).not.toHaveBeenCalledWith(
      "[tiandao] stale_state_skip tick=188 last_processed_tick=188",
    );
    expect(redis.publishCommands).toHaveBeenCalledTimes(2);
    expect(redis.publishCommands.mock.calls[0]?.[0]).toEqual(
      expect.objectContaining({
        metadata: expect.objectContaining({ sourceTick: 188, correlationId: "tiandao-tick-188" }),
      }),
    );
    expect(redis.publishCommands.mock.calls[1]?.[0]).toEqual(
      expect.objectContaining({
        metadata: expect.objectContaining({ sourceTick: 200, correlationId: "tiandao-tick-200" }),
      }),
    );
  });

  it("redacts redis credentials from runtime connect logs", async () => {
    const state = createTestWorldState();
    const redis = new SequenceRuntimeRedis([state]);
    const logger = { log: vi.fn(), error: vi.fn(), warn: vi.fn() };

    await runRuntime(
      {
        mockMode: false,
        model: DEFAULT_MODEL,
        redisUrl: "redis://:super-secret@redis.internal:6380/4",
        baseUrl: "https://llm.example.test/v1",
        apiKey: "k_test",
      },
      {
        createRedis: () => redis,
        createClient: () => ({
          chat: vi.fn(async (model: string) => createStructuredChatResult("[]", model)),
        }),
        agents: [new FakeAgent("mutation", { commands: [], narrations: [], reasoning: "ok" })],
        sleep: vi.fn(async () => {}),
        logger,
        maxLoopIterations: 1,
      },
    );

    const logLines = logger.log.mock.calls
      .map((call) => call[0])
      .filter((value): value is string => typeof value === "string");

    expect(logLines).toContain("[tiandao] connected to Redis at redis.internal:6380");
    expect(logLines.join("\n")).not.toContain("super-secret");
    expect(logLines.join("\n")).not.toContain("redis://:super-secret@redis.internal:6380/4");
  });

  it("restores world model history exactly before retrying a failed tick", async () => {
    await withIsolatedCwd(async () => {
      const seedState = createTestWorldState();
      seedState.tick = 123;
      seedState.zones[0] = {
        ...seedState.zones[0],
        spirit_qi: 0.41,
      };

      const failedTickState = createTestWorldState();
      failedTickState.tick = 124;
      failedTickState.zones[0] = {
        ...failedTickState.zones[0],
        spirit_qi: 0.77,
      };

      const nextFreshState = createTestWorldState();
      nextFreshState.tick = 125;
      nextFreshState.zones[0] = {
        ...nextFreshState.zones[0],
        spirit_qi: 0.55,
      };

      const redis = new FailingPublishRuntimeRedis(
        [seedState, failedTickState, failedTickState, nextFreshState],
        2,
      );
      const worldModel = new WorldModel();

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
              reasoning: "retry-after-rollback",
            }),
          ],
          sleep: vi.fn(async () => {}),
          logger: { log: vi.fn(), error: vi.fn(), warn: vi.fn() },
          worldModel,
          maxLoopIterations: 4,
        },
      );

      const starterHistory = worldModel.getZoneHistory("starter_zone");
      expect(starterHistory.map((entry) => entry.spirit_qi)).toEqual([0.41, 0.77, 0.55]);
      expect(worldModel.lastTick).toBe(125);
    });
  });

  it("does not roll back published world model after post-publish failure", async () => {
    const state = createTestWorldState();
    state.tick = 420;
    state.ts = 1_711_777_420;
    const logger = { log: vi.fn(), error: vi.fn(), warn: vi.fn() };
    const redis = new SequenceRuntimeRedis([state]);
    const worldModel = new WorldModel();
    const sleep = vi.fn(async () => {});

    redis.publishAgentWorldModel.mockImplementationOnce(async () => {
      throw new Error("publish world model failed");
    });

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
        agents: [new FakeAgent("mutation", { commands: [], narrations: [], reasoning: "post-publish" })],
        sleep,
        logger,
        worldModel,
        maxLoopIterations: 2,
      },
    );

    expect(worldModel.lastTick).toBe(420);
    expect(worldModel.lastStateTs).toBe(1_711_777_420);
    expect(logger.warn).toHaveBeenCalledWith(
      "[tiandao] failed to publish world model snapshot:",
      expect.any(Error),
    );
  });

  it("prefers snapshot with concrete lastStateTs when ticks tie", async () => {
    const logger = { log: vi.fn(), error: vi.fn(), warn: vi.fn() };
    const worldModel = new WorldModel();
    worldModel.restoreFromJSON(
      createMirrorSnapshot({
        currentEra: {
          name: "无时标旧态",
          sinceTick: 500,
          globalEffect: "仅内存残留",
        },
        lastTick: 500,
        lastStateTs: null,
      }),
    );
    const redis = new SequenceRuntimeRedis([null], [
      createMirrorSnapshot({
        currentEra: {
          name: "镜像权威态",
          sinceTick: 500,
          globalEffect: "带持久化游标",
        },
        lastTick: 500,
        lastStateTs: 1_711_555_500,
      }),
    ]);

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
        agents: [new FakeAgent("mutation", { commands: [], narrations: [], reasoning: "idle" })],
        sleep: vi.fn(async () => {}),
        logger,
        worldModel,
        maxLoopIterations: 1,
      },
    );

    expect(worldModel.currentEra?.name).toBe("镜像权威态");
    expect(worldModel.lastTick).toBe(500);
    expect(worldModel.lastStateTs).toBe(1_711_555_500);
    expect(logger.log).toHaveBeenCalledWith(
      "[tiandao] restored world model from redis mirror tick=500, era: 镜像权威态 (startup)",
    );
  });

  it("does not re-persist world model state on stale tick skip", async () => {
    const staleState = createTestWorldState();
    staleState.tick = 300;
    const freshState = createTestWorldState();
    freshState.tick = 301;
    const redis = new SequenceRuntimeRedis([staleState, staleState, freshState]);

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
              reasoning: "persist",
            }),
          ],
          sleep: vi.fn(async () => {}),
          logger: { log: vi.fn(), error: vi.fn(), warn: vi.fn() },
          maxLoopIterations: 3,
        },
      );
    });

    expect(redis.publishAgentWorldModel).toHaveBeenCalledTimes(2);
    expect(redis.publishAgentWorldModel.mock.calls[0]?.[0]?.snapshot.lastTick).toBe(300);
    expect(redis.publishAgentWorldModel.mock.calls[1]?.[0]?.snapshot.lastTick).toBe(301);
  });

  it("periodically reconciles newer redis mirror state after prolonged stale-state idle", async () => {
    const staleState = createTestWorldState();
    staleState.tick = 300;
    staleState.ts = 1_711_333_300;
    const logger = { log: vi.fn(), error: vi.fn(), warn: vi.fn() };
    const redis = new SequenceRuntimeRedis(
      [staleState],
      [
        null,
        createMirrorSnapshot({
          currentEra: {
            name: "新纪",
            sinceTick: 320,
            globalEffect: "灵脉复涌",
          },
          zoneHistory: {
            starter_zone: [
              {
                name: "starter_zone",
                spirit_qi: 0.66,
                danger_level: 1,
                active_events: [],
                player_count: 2,
              },
            ],
          },
          lastTick: 320,
          lastStateTs: 1_711_333_320,
        }),
      ],
    );
    const worldModel = new WorldModel();

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
        agents: [new FakeAgent("mutation", { commands: [], narrations: [], reasoning: "idle" })],
        sleep: vi.fn(async () => {}),
        logger,
        worldModel,
        maxLoopIterations: 61,
      },
    );

    expect(worldModel.currentEra?.name).toBe("新纪");
    expect(worldModel.lastTick).toBe(320);
    expect(redis.publishAgentWorldModel).toHaveBeenCalledTimes(1);
    expect(logger.log).toHaveBeenCalledWith(
      "[tiandao] restored world model from redis mirror tick=320, era: 新纪 (reconcile)",
    );
  });

  it("ignores corrupted local snapshot file at startup and continues with fresh tick", async () => {
    const tempDir = await mkdtemp(join(tmpdir(), "tiandao-runtime-corrupt-"));
    const previousCwd = process.cwd();
    const logger = { log: vi.fn(), error: vi.fn(), warn: vi.fn() };

    try {
      process.chdir(tempDir);
      await mkdir(join(tempDir, "data"), { recursive: true });
      await writeFile(join(tempDir, "data", "tiandao-snapshot-999.json"), "{broken", "utf8");

      const state = createTestWorldState();
      state.tick = 400;
      const redis = new SequenceRuntimeRedis([state]);

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
          agents: [new FakeAgent("mutation", { commands: [], narrations: [], reasoning: "ok" })],
          sleep: vi.fn(async () => {}),
          logger,
          maxLoopIterations: 1,
        },
      );

      expect(logger.warn).not.toHaveBeenCalledWith(
        "[tiandao] failed to load snapshot file tiandao-snapshot-999.json:",
        expect.any(Error),
      );
      expect(redis.publishCommands).toHaveBeenCalledTimes(0);
      expect(redis.publishAgentWorldModel).toHaveBeenCalledTimes(1);
      expect(redis.publishAgentWorldModel.mock.calls[0]?.[0]?.snapshot.lastTick).toBe(400);
    } finally {
      process.chdir(previousCwd);
      await rm(tempDir, { recursive: true, force: true });
    }
  });

  it("continues with fresh tick even when startup worldModel is malformed", async () => {
    await withIsolatedCwd(async () => {
      const state = createTestWorldState();
      state.tick = 410;
      const redis = new SequenceRuntimeRedis([state]);
      const logger = { log: vi.fn(), error: vi.fn(), warn: vi.fn() };
      const worldModel = new WorldModel();

      worldModel.restoreFromJSON({
        currentEra: {
          name: "broken",
          sinceTick: "bad" as unknown as number,
          globalEffect: "oops",
        },
        zoneHistory: {
          blood_valley: "bad-history" as unknown as never,
        },
        lastDecisions: {
          mutation: {
            commands: "bad" as unknown as never[],
            narrations: [],
            reasoning: "recoverable",
          },
        },
        playerFirstSeenTick: "bad" as unknown as Record<string, number>,
        lastTick: "bad" as unknown as number,
        lastStateTs: "bad" as unknown as number,
      });

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
          agents: [new FakeAgent("mutation", { commands: [], narrations: [], reasoning: "ok" })],
          sleep: vi.fn(async () => {}),
          logger,
        maxLoopIterations: 1,
          worldModel,
        },
      );

      expect(redis.publishCommands).toHaveBeenCalledTimes(0);
      expect(redis.publishAgentWorldModel).toHaveBeenCalledTimes(1);
      expect(redis.publishAgentWorldModel.mock.calls[0]?.[0]?.snapshot.lastTick).toBe(410);
    });
  });

  it("rotates local snapshot files and keeps latest five under data", async () => {
    const tempDir = await mkdtemp(join(tmpdir(), "tiandao-runtime-rotate-"));
    const previousCwd = process.cwd();

    try {
      process.chdir(tempDir);
      const baseState = createTestWorldState();
      const states = Array.from({ length: 8 }, (_unused, index) => {
        const state = {
          ...baseState,
          tick: 100 * (index + 1),
          ts: baseState.ts + 100 * index,
          players: [...baseState.players],
          npcs: [...baseState.npcs],
          zones: baseState.zones.map((zone) => ({ ...zone, active_events: [...zone.active_events] })),
          recent_events: [...baseState.recent_events],
        };
        return state;
      });

      const redis = new SequenceRuntimeRedis(states);

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
          agents: [new FakeAgent("mutation", { commands: [], narrations: [], reasoning: "ok" })],
          sleep: vi.fn(async () => {}),
          logger: { log: vi.fn(), error: vi.fn(), warn: vi.fn() },
          maxLoopIterations: 8,
        },
      );

      const files = (await readdir(join(tempDir, "data")))
        .filter((name) => name.startsWith("tiandao-snapshot-") && name.endsWith(".json"))
        .sort();
      expect(files).toEqual([
        "tiandao-snapshot-400.json",
        "tiandao-snapshot-500.json",
        "tiandao-snapshot-600.json",
        "tiandao-snapshot-700.json",
        "tiandao-snapshot-800.json",
      ]);
    } finally {
      process.chdir(previousCwd);
      await rm(tempDir, { recursive: true, force: true });
    }
  });
});
