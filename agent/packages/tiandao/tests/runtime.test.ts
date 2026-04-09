import { describe, expect, it, vi } from "vitest";
import {
  DEFAULT_MODEL,
  DEFAULT_REDIS_URL,
  resolveRuntimeConfig,
  createRuntimeClient,
  runTick,
  runRuntime,
  type RuntimeRedis,
} from "../src/runtime.js";
import { WorldModel } from "../src/world-model.js";
import { FakeAgent, FakeLlmClient, createTestWorldState } from "./support/fakes.js";
import type { Command, Narration } from "@bong/schema";

class ChatAwareFakeAgent extends FakeAgent {
  public receivedChatSignalsCount = 0;

  setChatSignals(signals: { player: string }[]): void {
    this.receivedChatSignalsCount = signals.length;
  }
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

  it("reads runtime env in non-mock mode", () => {
    const config = resolveRuntimeConfig(["node", "src/main.ts"], {
      LLM_MODEL: "test-model",
      REDIS_URL: "redis://mock:6379",
      LLM_BASE_URL: "https://llm.example.test/v1",
      LLM_API_KEY: "k_test",
    });

    expect(config.mockMode).toBe(false);
    expect(config.model).toBe("test-model");
    expect(config.redisUrl).toBe("redis://mock:6379");
    expect(config.baseUrl).toBe("https://llm.example.test/v1");
    expect(config.apiKey).toBe("k_test");
  });
});

describe("createRuntimeClient", () => {
  it("uses mock client in mock mode even without env", async () => {
    const chat = vi.fn(async () => "mock");
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

    expect(await client.chat([], DEFAULT_MODEL)).toBe("mock");
    expect(chat).toHaveBeenCalledTimes(1);
  });

  it("does not evaluate real-client factory in mock mode", async () => {
    const createClient = vi.fn(() => {
      throw new Error("real client should not be created in mock mode");
    });
    const mockChat = vi.fn(async () => "mock-only");

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

    expect(await client.chat([], DEFAULT_MODEL)).toBe("mock-only");
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

    await runTick(state, {
      agents: [
        new FakeAgent("mutation", {
          commands: [command],
          narrations: [narration],
          reasoning: "test",
        }),
      ],
      llmClient: new FakeLlmClient("{}"),
      model: DEFAULT_MODEL,
      publishCommands,
      publishNarrations,
      logger,
    });

    expect(publishCommands).toHaveBeenCalledTimes(1);
    expect(publishNarrations).toHaveBeenCalledTimes(1);
    expect(publishCommands).toHaveBeenCalledWith(
      "arbiter",
      expect.arrayContaining([
        expect.objectContaining({
          type: "modify_zone",
          target: "starter_zone",
        }),
      ]),
    );
    expect(publishNarrations).toHaveBeenCalledWith([narration]);
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
      llmClient: new FakeLlmClient("{}"),
      model: DEFAULT_MODEL,
      publishCommands,
      publishNarrations,
      logger: { log: vi.fn(), error: vi.fn() },
    });

    expect(publishCommands).toHaveBeenCalledTimes(1);
    expect(publishCommands).toHaveBeenCalledWith("arbiter", expect.any(Array));
  });

  it("skips publish when agent returns null", async () => {
    const publishCommands = vi.fn(async () => {});
    const publishNarrations = vi.fn(async () => {});

    await runTick(createTestWorldState(), {
      agents: [new FakeAgent("calamity", null)],
      llmClient: new FakeLlmClient("{}"),
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
      llmClient: new FakeLlmClient("{}"),
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
      llmClient: new FakeLlmClient("{}"),
      model: DEFAULT_MODEL,
      worldModel,
      publishCommands,
      publishNarrations,
      logger: { log: vi.fn(), error: vi.fn() },
    });

    expect(worldModel.currentEra).toEqual({
      name: "末法纪",
      sinceTick: 123,
      globalEffect: {
        description: "灵机渐枯，诸域修行更艰",
        spiritQiDelta: -0.02,
        dangerLevelDelta: 1,
      },
    });
    expect(publishCommands).toHaveBeenCalledWith(
      "arbiter",
      expect.arrayContaining([
        expect.objectContaining({ type: "modify_zone", target: "starter_zone" }),
      ]),
    );
  });
});

describe("runRuntime", () => {
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
          chat: vi.fn(async () => JSON.stringify({ commands: [], narrations: [], reasoning: "mock" })),
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
        createMockClient: () => ({ chat: vi.fn(async () => "{}") }),
        loadMockState: () => createTestWorldState(),
        sleep,
        logger: { log: vi.fn(), error: vi.fn(), warn: vi.fn() },
      },
    );

    expect(agentTick).toHaveBeenCalledTimes(1);
    expect(createRedis).not.toHaveBeenCalled();
    expect(sleep).not.toHaveBeenCalled();
  });
});
