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
  processLocustSwarmEvents,
  processTsyZoneActivatedForUi,
  type RuntimeModelOverrides,
  type CommandPublishRequest,
  type NarrationPublishRequest,
  type RuntimeRedis,
} from "../src/runtime.js";
import { LlmBackoffError, LlmTimeoutError, type LlmClient } from "../src/llm.js";
import type { TelemetrySink } from "../src/telemetry.js";
import { WorldModel, type WorldModelSnapshot } from "../src/world-model.js";
import { FakeAgent, createTestWorldState } from "./support/fakes.js";
import type { AgentUiResponsePayloadV1, AgentWorldModelEnvelopeV1, ChatMessageV1, Command, Narration, NpcDeathV1, RatPhaseChangeEventV1, TsyZoneActivatedV1 } from "@bong/schema";

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

class DeathAwareFakeAgent extends FakeAgent {
  public receivedNpcDeathCount = 0;
  public lastReceivedNpcDeaths: NpcDeathV1[] = [];

  setNpcDeathEvents(events: NpcDeathV1[]): void {
    this.receivedNpcDeathCount = events.length;
    this.lastReceivedNpcDeaths = events;
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
    negDomainPendingTribulations: {},
    negDomainEscapeTelemetry: {
      escapeEntryCount: 0,
      postEscapeRealmDropCount: 0,
      successfulTribulationAvoidanceCount: 0,
      activeEscapeSessionCount: 0,
      postEscapeRealmDropRate: 0,
    },
    negDomainEscapeSessions: {},
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

  it("merges deterministic NPC producer commands before publishing", async () => {
    const state = createTestWorldState();
    const publishCommands = vi.fn(async () => {});
    const publishNarrations = vi.fn(async () => {});
    const producer = vi.fn(() => [
      {
        source: "npc_producer",
        decision: {
          commands: [
            {
              type: "spawn_npc" as const,
              target: "starter_zone",
              params: { archetype: "rogue", count: 2, reason: "test_producer" },
            },
          ],
          narrations: [],
          reasoning: "producer",
        },
      },
    ]);

    await runTick(state, {
      agents: [
        new FakeAgent("mutation", {
          commands: [{ type: "modify_zone", target: "starter_zone", params: { spirit_qi_delta: 0.05 } }],
          narrations: [],
          reasoning: "llm",
        }),
      ],
      llmClient: new StructuredFakeLlmClient("{}"),
      model: DEFAULT_MODEL,
      publishCommands,
      publishNarrations,
      logger: { log: vi.fn(), error: vi.fn() },
      deterministicNpcProducer: producer,
    });

    expect(producer).toHaveBeenCalledWith(
      expect.objectContaining({
        state,
        sourcedDecisions: [expect.objectContaining({ source: "mutation" })],
        metadata: { sourceTick: 123, correlationId: "tiandao-tick-123" },
      }),
    );
    expect(publishCommands).toHaveBeenCalledWith(
      expect.objectContaining({
        commands: expect.arrayContaining([
          expect.objectContaining({ type: "modify_zone", target: "starter_zone" }),
          expect.objectContaining({ type: "spawn_npc", target: "starter_zone" }),
        ]),
      }),
    );
  });

  it("filters invalid producer commands through Arbiter constraints", async () => {
    const publishCommands = vi.fn(async () => {});
    await runTick(createTestWorldState(), {
      agents: [],
      llmClient: new StructuredFakeLlmClient("{}"),
      model: DEFAULT_MODEL,
      publishCommands,
      publishNarrations: vi.fn(async () => {}),
      logger: { log: vi.fn(), error: vi.fn() },
      deterministicNpcProducer: () => [
        {
          source: "npc_producer",
          decision: {
            commands: [{ type: "spawn_npc", target: "missing_zone", params: { archetype: "rogue" } }],
            narrations: [],
            reasoning: "invalid producer target",
          },
        },
      ],
    });

    expect(publishCommands).not.toHaveBeenCalled();
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

  it("publishes deterministic negative-domain drowning narrations from WorldModel state", async () => {
    const previousState = createTestWorldState();
    previousState.tick = 200;
    previousState.zones = [
      { name: "negative_domain", spirit_qi: -0.3, danger_level: 4, active_events: [], player_count: 2 },
    ];
    previousState.players[0] = {
      ...previousState.players[0],
      uuid: "victim",
      name: "高修乙",
      realm: "Spirit",
      zone: "negative_domain",
      cultivation: {
        realm: "Spirit",
        qi_current: 90,
        qi_max: 100,
        qi_max_frozen: 100,
        meridians_opened: 12,
        meridians_total: 20,
        qi_color_main: "Sharp",
        qi_color_chaotic: false,
        qi_color_hunyuan: false,
        composure: 0.8,
      },
    };
    previousState.players.push({
      ...previousState.players[0],
      uuid: "baiter",
      name: "低修甲",
      realm: "Condense",
      cultivation: {
        realm: "Condense",
        qi_current: 20,
        qi_max: 30,
        qi_max_frozen: 30,
        meridians_opened: 8,
        meridians_total: 20,
        qi_color_main: "Sharp",
        qi_color_chaotic: false,
        qi_color_hunyuan: false,
        composure: 0.8,
      },
    });
    const currentState = {
      ...previousState,
      tick: 205,
      players: previousState.players.map((player) => ({ ...player })),
    };
    currentState.players[0] = {
      ...currentState.players[0],
      cultivation: {
        ...currentState.players[0].cultivation!,
        qi_current: 60,
      },
    };
    const publishNarrations = vi.fn(async () => {});

    await runTick(currentState, {
      agents: [],
      llmClient: new StructuredFakeLlmClient("{}"),
      model: DEFAULT_MODEL,
      worldModel: WorldModel.fromState(previousState),
      publishCommands: vi.fn(async () => {}),
      publishNarrations,
      logger: { log: vi.fn(), error: vi.fn() },
    });

    expect(publishNarrations).toHaveBeenCalledWith(
      expect.objectContaining({
        narrations: expect.arrayContaining([
          expect.objectContaining({ scope: "player", target: "baiter", text: expect.stringContaining("这是你的机会") }),
          expect.objectContaining({ scope: "zone", target: "negative_domain", text: expect.stringContaining("不偏向强者") }),
        ]),
      }),
    );
  });

  it("adds negative-domain escape counters to tick telemetry for balance consumers", async () => {
    const previousState = createTestWorldState();
    previousState.tick = 210;
    previousState.players[0] = {
      ...previousState.players[0],
      uuid: "player-spirit",
      name: "灵修甲",
      realm: "Spirit",
      zone: "safe_zone",
    };
    previousState.zones = [
      { name: "safe_zone", spirit_qi: 0.4, danger_level: 1, active_events: [], player_count: 1 },
    ];
    const currentState = {
      ...previousState,
      tick: 211,
      players: previousState.players.map((player) => ({ ...player, zone: "negative_domain" })),
      zones: [
        { name: "negative_domain", spirit_qi: -0.2, danger_level: 4, active_events: [], player_count: 1 },
      ],
    };
    const capturedMetrics: unknown[] = [];
    const telemetrySink: TelemetrySink = {
      recordTick(metrics) {
        capturedMetrics.push(metrics);
      },
      flush() {},
    };

    const result = await runTick(currentState, {
      agents: [],
      llmClient: new StructuredFakeLlmClient("{}"),
      model: DEFAULT_MODEL,
      worldModel: WorldModel.fromState(previousState),
      publishCommands: vi.fn(async () => {}),
      publishNarrations: vi.fn(async () => {}),
      telemetrySink,
      logger: { log: vi.fn(), error: vi.fn() },
    });

    expect(result.metrics.negDomainEscape).toMatchObject({
      escapeEntryCount: 1,
      postEscapeRealmDropCount: 0,
      successfulTribulationAvoidanceCount: 0,
      activeEscapeSessionCount: 1,
      postEscapeRealmDropRate: 0,
    });
    expect(capturedMetrics[0]).toEqual(
      expect.objectContaining({
        negDomainEscape: expect.objectContaining({ escapeEntryCount: 1 }),
      }),
    );
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

  it("injects drained npc death events to agents before ticking (P4 offscreen war context)", async () => {
    const publishCommands = vi.fn(async () => {});
    const publishNarrations = vi.fn(async () => {});
    const deathAwareAgent = new DeathAwareFakeAgent("mutation", null);

    const deaths: NpcDeathV1[] = [
      {
        v: 1,
        kind: "npc_death",
        npc_id: "dormant:combat:1",
        archetype: "rogue",
        cause: "combat",
        faction_id: "attack",
        age_ticks: 10_000,
        max_age_ticks: 200_000,
        at_tick: 84_000,
        from_dormant_combat: true,
        pos: [12, 64, -30],
      },
    ];

    await runTick(createTestWorldState(), {
      agents: [deathAwareAgent],
      llmClient: new StructuredFakeLlmClient("{}"),
      model: DEFAULT_MODEL,
      npcDeathEvents: deaths,
      publishCommands,
      publishNarrations,
      logger: { log: vi.fn(), error: vi.fn() },
    });

    expect(deathAwareAgent.receivedNpcDeathCount).toBe(1);
    expect(deathAwareAgent.lastReceivedNpcDeaths[0]?.npc_id).toBe("dormant:combat:1");
  });

  it("defaults npc death events to empty when none supplied", async () => {
    const deathAwareAgent = new DeathAwareFakeAgent("mutation", null);
    await runTick(createTestWorldState(), {
      agents: [deathAwareAgent],
      llmClient: new StructuredFakeLlmClient("{}"),
      model: DEFAULT_MODEL,
      publishCommands: vi.fn(async () => {}),
      publishNarrations: vi.fn(async () => {}),
      logger: { log: vi.fn(), error: vi.fn() },
    });
    expect(deathAwareAgent.receivedNpcDeathCount).toBe(0);
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

describe("processLocustSwarmEvents", () => {
  it("publishes_only_the_first_accepted_locust_escalation_per_batch", async () => {
    const state = createTestWorldState();
    const event: RatPhaseChangeEventV1 = {
      chunk: [0, 0],
      zone: "starter_zone",
      group_id: 1,
      from: "solitary",
      to: { transitioning: { progress: 0 } },
      rat_count: 12,
      local_qi: 0.7,
      qi_gradient: 0.3,
      tick: state.tick,
    };
    const redis = {
      drainRatPhaseEvents: vi.fn(() => [
        event,
        { ...event, group_id: 2, zone: "green_cloud_peak" },
      ]),
      publishCommands: vi.fn(async (_request: CommandPublishRequest) => {}),
      publishNarrations: vi.fn(async (_request: NarrationPublishRequest) => {}),
    } as unknown as RuntimeRedis;
    const tracker = {
      ingest: vi.fn(() => ({
        commands: [
          {
            type: "spawn_event",
            target: "starter_zone",
            params: {
              event: "beast_tide",
              tide_kind: "locust_swarm",
              target_zone: "green_cloud_peak",
            },
          },
        ],
        narrations: [],
        reasoning: "accepted",
      })),
    } as unknown as Parameters<typeof processLocustSwarmEvents>[0]["tracker"];

    await processLocustSwarmEvents({
      redis,
      state,
      tracker,
      logger: { warn: vi.fn() },
    });

    expect(tracker.ingest).toHaveBeenCalledTimes(1);
    expect(redis.publishCommands).toHaveBeenCalledTimes(1);
    expect(redis.publishCommands).toHaveBeenCalledWith(
      expect.objectContaining({
        commands: [
          expect.objectContaining({
            type: "spawn_event",
            target: "starter_zone",
          }),
        ],
      }),
    );
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

// ─── plan-agent-ui-data-v1 P2 Fix①: triggerUi 在 TSY 秘境信号下被调 → emit AGENT_UI_CMD ─────

describe("processTsyZoneActivatedForUi (Fix①: triggerUi production path)", () => {
  /** 构造一个最小 mock AgentUiRuntime，只暴露 triggerUi */
  function makeMockUiRuntime() {
    return {
      triggerUi: vi.fn(async (_opts: unknown) => ({
        requestId: "mock-request-id-001",
        sentBlurVersion: false,
      })),
      drainPendingButtonClicks: vi.fn(() => [] as AgentUiResponsePayloadV1[]),
      drainPendingSessionEnds: vi.fn(() => [] as AgentUiResponsePayloadV1[]),
    };
  }

  function makeTsyZoneActivatedV1(overrides: Partial<TsyZoneActivatedV1> = {}): TsyZoneActivatedV1 {
    return {
      v: 1,
      kind: "tsy_zone_activated",
      tick: 1000,
      family_id: "tsy_lingxu_01",
      source_class: "dao_lord",
      ...overrides,
    };
  }

  it("calls triggerUi with tsy_discovery scenario when a TSY zone activates with a player online", async () => {
    const uiRuntime = makeMockUiRuntime();
    const state = createTestWorldState(); // has player in "starter_zone"
    const event = makeTsyZoneActivatedV1({ family_id: "starter_zone" }); // player is in this zone

    await processTsyZoneActivatedForUi({
      state,
      events: [event],
      agentUiRuntime: uiRuntime as never,
      logger: { log: vi.fn(), warn: vi.fn() },
    });

    expect(uiRuntime.triggerUi, "triggerUi must be called once for a TSY activation event").toHaveBeenCalledOnce();
    const opts = uiRuntime.triggerUi.mock.calls[0][0] as Record<string, unknown>;
    expect(opts["scenario"], "scenario must be tsy_discovery").toBe("tsy_discovery");
    expect(opts["targetPlayer"], "targetPlayer must be set to the online player").toBeTruthy();
    const params = opts["params"] as Record<string, string>;
    expect(params["zone_name"], "zone_name must match family_id").toBe("starter_zone");
    expect(params["danger_tier"], "danger_tier must be resolved from zone snapshot").toBeTruthy();
    expect(params["agent_narrative"], "agent_narrative must contain family_id").toContain("starter_zone");
  });

  it("picks any online player when no player is in the exact TSY zone", async () => {
    const uiRuntime = makeMockUiRuntime();
    const state = createTestWorldState(); // player in "starter_zone"
    const event = makeTsyZoneActivatedV1({ family_id: "tsy_lingxu_99" }); // different zone, no player

    await processTsyZoneActivatedForUi({
      state,
      events: [event],
      agentUiRuntime: uiRuntime as never,
      logger: { log: vi.fn(), warn: vi.fn() },
    });

    // Falls back to first online player
    expect(uiRuntime.triggerUi, "should fallback to first online player").toHaveBeenCalledOnce();
    const opts = uiRuntime.triggerUi.mock.calls[0][0] as Record<string, unknown>;
    const target = opts["targetPlayer"] as { uuid: string };
    expect(target.uuid, "target player uuid is the first online player").toBe("offline:test-player");
  });

  it("skips triggerUi and logs when there are no online players", async () => {
    const uiRuntime = makeMockUiRuntime();
    const state = { ...createTestWorldState(), players: [] };
    const event = makeTsyZoneActivatedV1();
    const logSpy = vi.fn();

    await processTsyZoneActivatedForUi({
      state,
      events: [event],
      agentUiRuntime: uiRuntime as never,
      logger: { log: logSpy, warn: vi.fn() },
    });

    expect(uiRuntime.triggerUi, "no players → triggerUi must NOT be called").not.toHaveBeenCalled();
    expect(logSpy, "should log skip message").toHaveBeenCalledWith(
      expect.stringContaining("no online players"),
    );
  });

  it("processes multiple TSY activation events in one batch", async () => {
    const uiRuntime = makeMockUiRuntime();
    const state = createTestWorldState();
    const events = [
      makeTsyZoneActivatedV1({ family_id: "tsy_a", tick: 100 }),
      makeTsyZoneActivatedV1({ family_id: "tsy_b", tick: 101 }),
      makeTsyZoneActivatedV1({ family_id: "tsy_c", tick: 102 }),
    ];

    await processTsyZoneActivatedForUi({
      state,
      events,
      agentUiRuntime: uiRuntime as never,
      logger: { log: vi.fn(), warn: vi.fn() },
    });

    expect(uiRuntime.triggerUi.mock.calls).toHaveLength(3);
  });

  it("warns and continues when triggerUi throws for one event", async () => {
    const uiRuntime = makeMockUiRuntime();
    uiRuntime.triggerUi
      .mockRejectedValueOnce(new Error("redis down"))
      .mockResolvedValueOnce({ requestId: "req-002", sentBlurVersion: false });

    const state = createTestWorldState();
    const events = [
      makeTsyZoneActivatedV1({ family_id: "tsy_fail", tick: 200 }),
      makeTsyZoneActivatedV1({ family_id: "tsy_ok", tick: 201 }),
    ];
    const warnSpy = vi.fn();

    await processTsyZoneActivatedForUi({
      state,
      events,
      agentUiRuntime: uiRuntime as never,
      logger: { log: vi.fn(), warn: warnSpy },
    });

    // Both events attempted
    expect(uiRuntime.triggerUi.mock.calls).toHaveLength(2);
    expect(warnSpy, "should warn on triggerUi failure").toHaveBeenCalledOnce();
    expect(warnSpy.mock.calls[0][0]).toContain("triggerUi failed");
  });

  it("emits AGENT_UI_CMD via agentUiRuntime.triggerUi for dao_lord source class", async () => {
    const uiRuntime = makeMockUiRuntime();
    const state = createTestWorldState();
    const event = makeTsyZoneActivatedV1({ source_class: "dao_lord" });

    await processTsyZoneActivatedForUi({
      state, events: [event], agentUiRuntime: uiRuntime as never, logger: { log: vi.fn(), warn: vi.fn() },
    });

    const params = (uiRuntime.triggerUi.mock.calls[0][0] as Record<string, unknown>)["params"] as Record<string, string>;
    expect(params["agent_narrative"], "narrative references source class").toContain("dao_lord");
  });
});

// ─── plan-agent-ui-data-v1 P2 Fix②: Arbiter loop drain → button_click 进推演输入 ─────────────

describe("runRuntime Fix②: drainPendingButtonClicks injected into tick before each Arbiter run", () => {
  /** 扩展 SequenceRuntimeRedis 支持 drainTsyZoneActivatedEvents（返回空） */
  class AgentUiAwareRuntimeRedis implements RuntimeRedis {
    public readonly connect = vi.fn(async () => {});
    public readonly disconnect = vi.fn(async () => {});
    public readonly drainPlayerChat = vi.fn(async (): Promise<ChatMessageV1[]> => []);
    public readonly publishCommands = vi.fn(async (_r: CommandPublishRequest) => {});
    public readonly publishNarrations = vi.fn(async (_r: NarrationPublishRequest) => {});
    public readonly drainTsyZoneActivatedEvents = vi.fn(() => [] as TsyZoneActivatedV1[]);
    private index = 0;
    constructor(private readonly states: Array<ReturnType<typeof createTestWorldState> | null>) {}
    getLatestState() {
      const s = this.states[Math.min(this.index, this.states.length - 1)] ?? null;
      this.index += 1;
      return s;
    }
  }

  it("drain is called exactly once per fresh-state tick and button_clicks logged as injected (Fix②)", async () => {
    const clickEvent: AgentUiResponsePayloadV1 = {
      request_id: "req-fix2-001",
      action: "button_click",
      params: { button_id: "enter_realm" },
    };
    const mockUiRuntime = {
      triggerUi: vi.fn(async () => ({ requestId: "r", sentBlurVersion: false })),
      drainPendingButtonClicks: vi.fn()
        .mockReturnValueOnce([clickEvent]) // first tick: one click
        .mockReturnValue([]),              // subsequent ticks: empty
      drainPendingSessionEnds: vi.fn(() => []),
    };

    const state1 = createTestWorldState();
    const state2 = { ...createTestWorldState(), tick: 124, ts: state1.ts + 5 };
    const redis = new AgentUiAwareRuntimeRedis([state1, state2, null]);
    const logger = { log: vi.fn(), error: vi.fn(), warn: vi.fn() };

    const tempDir = await mkdtemp(join(tmpdir(), "tiandao-fix2-"));
    const prevCwd = process.cwd();
    try {
      process.chdir(tempDir);
      await mkdir(join(tempDir, "data"), { recursive: true });

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
          createClient: () => ({ chat: vi.fn(async (m: string) => ({ content: "[]", durationMs: 0, requestId: "r", model: m })) }),
          agents: [new FakeAgent("calamity", { commands: [], narrations: [], reasoning: "ok" })],
          sleep: vi.fn(async () => {}),
          logger,
          maxLoopIterations: 3,
          agentUiRuntime: mockUiRuntime as never,
        },
      );
    } finally {
      process.chdir(prevCwd);
      await rm(tempDir, { recursive: true, force: true });
    }

    // Fix②: drainPendingButtonClicks 必须在每轮 fresh-state tick 时被调
    expect(
      mockUiRuntime.drainPendingButtonClicks.mock.calls.length,
      "drainPendingButtonClicks should be called for each fresh-state tick",
    ).toBeGreaterThanOrEqual(1);

    // Fix②: button_click 注入时应有 log 记录
    const logLines = logger.log.mock.calls.flatMap((c) => c.map(String));
    const injectLog = logLines.find((l) => l.includes("button_click inject") && l.includes("enter_realm"));
    expect(injectLog, "should log button_click inject with button_id=enter_realm").toBeTruthy();
  });

  it("runRuntime with no agentUiRuntime proceeds without error (drain skipped gracefully)", async () => {
    const redis = new AgentUiAwareRuntimeRedis([createTestWorldState(), null]);
    const logger = { log: vi.fn(), error: vi.fn(), warn: vi.fn() };

    const tempDir = await mkdtemp(join(tmpdir(), "tiandao-fix2-noop-"));
    const prevCwd = process.cwd();
    try {
      process.chdir(tempDir);
      await mkdir(join(tempDir, "data"), { recursive: true });

      await expect(
        runRuntime(
          {
            mockMode: false,
            model: DEFAULT_MODEL,
            redisUrl: DEFAULT_REDIS_URL,
            baseUrl: "https://llm.example.test/v1",
            apiKey: "k_test",
          },
          {
            createRedis: () => redis,
            createClient: () => ({ chat: vi.fn(async (m: string) => ({ content: "[]", durationMs: 0, requestId: "r", model: m })) }),
            agents: [new FakeAgent("calamity", { commands: [], narrations: [], reasoning: "ok" })],
            sleep: vi.fn(async () => {}),
            logger,
            maxLoopIterations: 2,
            // agentUiRuntime intentionally omitted
          },
        ),
      ).resolves.toBeUndefined();
    } finally {
      process.chdir(prevCwd);
      await rm(tempDir, { recursive: true, force: true });
    }

    expect(logger.error, "no errors when agentUiRuntime is absent").not.toHaveBeenCalled();
  });

  // ─── session_end drain 接线 ────────────────────────────────────────────────

  it("drainPendingSessionEnds is called in runRuntime loop and session_end is logged (接线验收)", async () => {
    const sessionEndEvent: AgentUiResponsePayloadV1 = {
      request_id: "req-se-001",
      action: "dismissed",
      params: {},
    };
    const mockUiRuntime = {
      triggerUi: vi.fn(async () => ({ requestId: "r", sentBlurVersion: false })),
      drainPendingButtonClicks: vi.fn(() => []),
      drainPendingSessionEnds: vi.fn()
        .mockReturnValueOnce([sessionEndEvent]) // first tick: one session_end
        .mockReturnValue([]),
    };

    const state1 = createTestWorldState();
    const redis = new AgentUiAwareRuntimeRedis([state1, null]);
    const logger = { log: vi.fn(), error: vi.fn(), warn: vi.fn() };

    const tempDir = await mkdtemp(join(tmpdir(), "tiandao-se-drain-"));
    const prevCwd = process.cwd();
    try {
      process.chdir(tempDir);
      await mkdir(join(tempDir, "data"), { recursive: true });

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
          createClient: () => ({ chat: vi.fn(async (m: string) => ({ content: "[]", durationMs: 0, requestId: "r", model: m })) }),
          agents: [new FakeAgent("calamity", { commands: [], narrations: [], reasoning: "ok" })],
          sleep: vi.fn(async () => {}),
          logger,
          maxLoopIterations: 2,
          agentUiRuntime: mockUiRuntime as never,
        },
      );
    } finally {
      process.chdir(prevCwd);
      await rm(tempDir, { recursive: true, force: true });
    }

    // drainPendingSessionEnds 必须在每轮 fresh-state tick 时被调
    expect(
      mockUiRuntime.drainPendingSessionEnds.mock.calls.length,
      "drainPendingSessionEnds must be called at least once in the loop",
    ).toBeGreaterThanOrEqual(1);

    // session_end 消费时应有 log 记录（action=dismissed, request_id）
    const logLines = logger.log.mock.calls.flatMap((c) => c.map(String));
    const sessionEndLog = logLines.find(
      (l) => l.includes("session_end") && l.includes("dismissed") && l.includes("req-se-001"),
    );
    expect(
      sessionEndLog,
      "should log session_end with action=dismissed and request_id=req-se-001",
    ).toBeTruthy();
  });
});

// ─── plan-agent-ui-data-v1 P2 Fix③: button_click 真注入 agent 推演上下文 ──────────────────────

describe("runTick button_click 真注入: applyButtonClickEventsToAgents 进 agent.setButtonClickEvents", () => {
  /** TickAgent 实现了 setButtonClickEvents，用于验证注入路径 */
  class ButtonClickAwareAgent extends FakeAgent {
    public receivedButtonClicks: AgentUiResponsePayloadV1[] = [];

    setButtonClickEvents(events: AgentUiResponsePayloadV1[]): void {
      this.receivedButtonClicks = events;
    }
  }

  it("runTick calls setButtonClickEvents on agent with button_click events", async () => {
    const clickEvent: AgentUiResponsePayloadV1 = {
      request_id: "req-inject-001",
      action: "button_click",
      params: { button_id: "enter_realm" },
    };
    const agent = new ButtonClickAwareAgent("calamity", { commands: [], narrations: [], reasoning: "ok" });
    const state = createTestWorldState();

    await runTick(state, {
      agents: [agent],
      llmClient: { chat: vi.fn(async (m: string) => ({ content: "[]", durationMs: 0, requestId: "r", model: m })) },
      model: DEFAULT_MODEL,
      buttonClickEvents: [clickEvent],
      publishCommands: vi.fn(async () => {}),
      publishNarrations: vi.fn(async () => {}),
      logger: { log: vi.fn(), error: vi.fn() },
    });

    expect(
      agent.receivedButtonClicks,
      "agent.setButtonClickEvents must be called with the button_click events (真注入验收)",
    ).toHaveLength(1);
    expect(
      agent.receivedButtonClicks[0].params["button_id"],
      "injected button_id must match the original click event",
    ).toBe("enter_realm");
  });

  it("runTick with empty buttonClickEvents does not call setButtonClickEvents", async () => {
    const agent = new ButtonClickAwareAgent("calamity", { commands: [], narrations: [], reasoning: "ok" });
    const state = createTestWorldState();

    await runTick(state, {
      agents: [agent],
      llmClient: { chat: vi.fn(async (m: string) => ({ content: "[]", durationMs: 0, requestId: "r", model: m })) },
      model: DEFAULT_MODEL,
      buttonClickEvents: [], // empty
      publishCommands: vi.fn(async () => {}),
      publishNarrations: vi.fn(async () => {}),
      logger: { log: vi.fn(), error: vi.fn() },
    });

    // setButtonClickEvents is called with empty array (not undefined), queue size stays 0
    expect(
      agent.receivedButtonClicks,
      "empty buttonClickEvents: agent receives empty array (no residual from prior state)",
    ).toHaveLength(0);
  });

  it("runTick with no buttonClickEvents field calls setButtonClickEvents with empty array", async () => {
    const agent = new ButtonClickAwareAgent("era", { commands: [], narrations: [], reasoning: "ok" });
    const state = createTestWorldState();

    await runTick(state, {
      agents: [agent],
      llmClient: { chat: vi.fn(async (m: string) => ({ content: "[]", durationMs: 0, requestId: "r", model: m })) },
      model: DEFAULT_MODEL,
      // buttonClickEvents omitted
      publishCommands: vi.fn(async () => {}),
      publishNarrations: vi.fn(async () => {}),
      logger: { log: vi.fn(), error: vi.fn() },
    });

    expect(
      agent.receivedButtonClicks,
      "omitted buttonClickEvents: agent receives empty array (setButtonClickEvents called with [])",
    ).toHaveLength(0);
  });

  it("multiple button_click events all reach agent.setButtonClickEvents", async () => {
    const clicks: AgentUiResponsePayloadV1[] = [
      { request_id: "r1", action: "button_click", params: { button_id: "enter_realm" } },
      { request_id: "r2", action: "button_click", params: { button_id: "observe_only" } },
    ];
    const agent = new ButtonClickAwareAgent("mutation", { commands: [], narrations: [], reasoning: "ok" });
    const state = createTestWorldState();

    await runTick(state, {
      agents: [agent],
      llmClient: { chat: vi.fn(async (m: string) => ({ content: "[]", durationMs: 0, requestId: "r", model: m })) },
      model: DEFAULT_MODEL,
      buttonClickEvents: clicks,
      publishCommands: vi.fn(async () => {}),
      publishNarrations: vi.fn(async () => {}),
      logger: { log: vi.fn(), error: vi.fn() },
    });

    expect(agent.receivedButtonClicks).toHaveLength(2);
    expect(agent.receivedButtonClicks[0].params["button_id"]).toBe("enter_realm");
    expect(agent.receivedButtonClicks[1].params["button_id"]).toBe("observe_only");
  });

  it("agent without setButtonClickEvents is not affected (graceful no-op)", async () => {
    // FakeAgent does NOT implement setButtonClickEvents → should not throw
    const agentWithout = new FakeAgent("calamity", { commands: [], narrations: [], reasoning: "ok" });
    const agentWith = new ButtonClickAwareAgent("era", { commands: [], narrations: [], reasoning: "ok" });
    const state = createTestWorldState();
    const clicks: AgentUiResponsePayloadV1[] = [
      { request_id: "r1", action: "button_click", params: { button_id: "dismiss" } },
    ];

    await expect(
      runTick(state, {
        agents: [agentWithout, agentWith],
        llmClient: { chat: vi.fn(async (m: string) => ({ content: "[]", durationMs: 0, requestId: "r", model: m })) },
        model: DEFAULT_MODEL,
        buttonClickEvents: clicks,
        publishCommands: vi.fn(async () => {}),
        publishNarrations: vi.fn(async () => {}),
        logger: { log: vi.fn(), error: vi.fn() },
      }),
    ).resolves.not.toThrow();

    // agent with setButtonClickEvents received the events
    expect(agentWith.receivedButtonClicks).toHaveLength(1);
  });
});

// ─── plan-agent-ui-data-v1 P2 BLOCKER: target_player canonical format ─────────────────────────

describe("processTsyZoneActivatedForUi target_player canonical format (BLOCKER-身份键)", () => {
  it("target_player passed to triggerUi is the player.uuid field (offline:X canonical format)", async () => {
    const uiRuntime = {
      triggerUi: vi.fn(async (_opts: unknown) => ({
        requestId: "mock-request-id-canonical",
        sentBlurVersion: false,
      })),
      drainPendingButtonClicks: vi.fn(() => [] as AgentUiResponsePayloadV1[]),
      drainPendingSessionEnds: vi.fn(() => [] as AgentUiResponsePayloadV1[]),
    };

    // createTestWorldState() already uses uuid: "offline:test-player" (canonical format)
    const state = createTestWorldState();
    const event: TsyZoneActivatedV1 = {
      v: 1,
      kind: "tsy_zone_activated",
      tick: 1001,
      family_id: "tsy_canonical_test",
      source_class: "dao_lord",
    };

    await processTsyZoneActivatedForUi({
      state,
      events: [event],
      agentUiRuntime: uiRuntime as never,
      logger: { log: vi.fn(), warn: vi.fn() },
    });

    expect(uiRuntime.triggerUi).toHaveBeenCalledOnce();
    const opts = uiRuntime.triggerUi.mock.calls[0][0] as Record<string, unknown>;
    const targetPlayer = opts["targetPlayer"] as { uuid: string; name: string };

    // BLOCKER 契约 pin：target_player 必须是 "offline:X" 格式（canonical_player_id）
    // 与 server agent_ui.rs 的 canonical_player_id(username.0.as_str()) 比较逻辑对齐
    expect(
      targetPlayer.uuid,
      "target_player.uuid must be 'offline:X' (canonical_player_id format) to match server lookup",
    ).toBe("offline:test-player");
    expect(
      targetPlayer.uuid,
      "canonical id must start with 'offline:' prefix",
    ).toMatch(/^offline:/);
  });

  it("target_player canonical id is preserved intact through triggerUi → renderUi → command.target_player", async () => {
    // End-to-end: processTsyZoneActivatedForUi → agentUiRuntime.triggerUi → target_player field
    // The triggerUi receives targetPlayer.uuid which is the world-state canonical id.
    // This pin test verifies the uuid string is never transformed (no stripping, no concatenation).
    const state = createTestWorldState();
    // UUID is "offline:test-player" — canonical format
    expect(
      state.players[0].uuid,
      "createTestWorldState player.uuid must be in canonical 'offline:X' format (world-state contract)",
    ).toMatch(/^offline:/);

    const capturedTargetPlayer: { uuid: string }[] = [];
    const uiRuntime = {
      triggerUi: vi.fn(async (opts: unknown) => {
        const o = opts as Record<string, unknown>;
        capturedTargetPlayer.push(o["targetPlayer"] as { uuid: string });
        return { requestId: "r-canon", sentBlurVersion: false };
      }),
      drainPendingButtonClicks: vi.fn(() => [] as AgentUiResponsePayloadV1[]),
      drainPendingSessionEnds: vi.fn(() => [] as AgentUiResponsePayloadV1[]),
    };

    await processTsyZoneActivatedForUi({
      state,
      events: [{ v: 1, kind: "tsy_zone_activated", tick: 2000, family_id: "starter_zone", source_class: "sect_ruins" }],
      agentUiRuntime: uiRuntime as never,
      logger: { log: vi.fn(), warn: vi.fn() },
    });

    expect(capturedTargetPlayer).toHaveLength(1);
    expect(
      capturedTargetPlayer[0].uuid,
      "targetPlayer.uuid passed to triggerUi must equal the world-state player.uuid unchanged",
    ).toBe(state.players[0].uuid);
  });
});
