import { CHANNELS } from "@bong/schema";
import { describe, expect, it, vi } from "vitest";

import { createMockClient } from "../src/llm.js";
import * as runtime from "../src/runtime.js";
import {
  getMockCompletionMarker,
  main,
  runMockTickForTest,
  startAgentUiResponseRuntime,
  type PublishSink,
  type RedisRuntimeClient,
} from "../src/main.js";
import { REALM_GATE_NARRATION_TEXT } from "../src/ui/uiResponseConsumer.js";
import { WorldModel } from "../src/world-model.js";

interface MockRedisRuntimeClient extends RedisRuntimeClient {
  readonly publish: ReturnType<typeof vi.fn>;
  readonly subscribe: ReturnType<typeof vi.fn>;
  readonly on: ReturnType<typeof vi.fn>;
  readonly off: ReturnType<typeof vi.fn>;
  readonly unsubscribe: ReturnType<typeof vi.fn>;
  readonly disconnect: ReturnType<typeof vi.fn>;
  emit(channel: string, message: string): void;
}

function makeMockRedisRuntimeClient(options: { subscribeError?: Error } = {}): MockRedisRuntimeClient {
  const listeners = new Set<(channel: string, message: string) => void>();
  let subscriberMode = false;

  return {
    publish: vi.fn(async (_channel: string, _message: string) => {
      if (subscriberMode) {
        throw new Error("Connection in subscriber mode, only subscriber commands may be used");
      }
      return 1;
    }),
    subscribe: vi.fn(async (_channel: string) => {
      if (options.subscribeError) throw options.subscribeError;
      subscriberMode = true;
    }),
    on: vi.fn((_event: string, listener: (channel: string, message: string) => void) => {
      listeners.add(listener);
    }),
    off: vi.fn((_event: string, listener: (channel: string, message: string) => void) => {
      listeners.delete(listener);
    }),
    unsubscribe: vi.fn(async () => {
      subscriberMode = false;
    }),
    disconnect: vi.fn(() => undefined),
    emit(channel: string, message: string): void {
      for (const listener of listeners) listener(channel, message);
    },
  };
}

const ERA_DECLARATION_RESPONSE = JSON.stringify({
  commands: [],
  narrations: [
    {
      scope: "broadcast",
      text: "天道昭告：灵潮纪已至，诸域灵机渐盛。",
      style: "era_decree",
    },
  ],
  reasoning: "Era declaration for deterministic test",
});

describe("main mock execution", () => {
  it("keeps mock client metadata deterministic for smoke assertions", async () => {
    const result = await createMockClient().chat("mock-model", [
      { role: "system", content: "system" },
      { role: "user", content: "user" },
    ]);

    expect(result.durationMs).toBe(0);
    expect(result.requestId).toBeNull();
    expect(result.model).toBe("mock-model");
  });

  it("runs single mock tick without env and emits stable marker", async () => {
    const logSpy = vi.spyOn(console, "log").mockImplementation(() => undefined);
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => undefined);

    try {
      await expect(
        main({
          mockMode: true,
          baseUrl: undefined,
          apiKey: undefined,
          model: "mock-model",
        }),
      ).resolves.toBeUndefined();

      const logs = logSpy.mock.calls.flatMap((call) => call.map((item) => String(item)));
      expect(logs).toContain(getMockCompletionMarker());
      expect(errorSpy).not.toHaveBeenCalledWith("Missing LLM_BASE_URL or LLM_API_KEY in .env");
    } finally {
      logSpy.mockRestore();
      errorSpy.mockRestore();
    }
  });

  it("publishes deterministic command and narration through injected sink", async () => {
    const commandPublishes: Array<{ source: string; commands: unknown[] }> = [];
    const narrationPublishes: unknown[][] = [];
    const sink: PublishSink = {
      async publishCommands(source, commands) {
        commandPublishes.push({ source, commands });
      },
      async publishNarrations(narrations) {
        narrationPublishes.push(narrations);
      },
    };

    const llm = createMockClient(
      JSON.stringify({
        commands: [
          {
            type: "spawn_event",
            target: "blood_valley",
            params: { event: "beast_tide", intensity: 0.7, duration_ticks: 200 },
          },
        ],
        narrations: [
          {
            scope: "zone",
            target: "blood_valley",
            text: "天象突变，兽潮将至。",
            style: "system_warning",
          },
        ],
        reasoning: "deterministic mock publish",
      }),
    );

    const summary = await runMockTickForTest({
      llmClient: llm,
      sink,
      now: () => 50_000_000,
      model: "mock-model",
    });

    expect(summary.totalCommands).toBe(3);
    expect(summary.totalNarrations).toBe(3);
    expect(summary.chatSignalCount).toBe(0);
    expect(summary.skipped).toBe(false);
    expect(summary.durationMs).toBeGreaterThanOrEqual(0);
    expect(commandPublishes).toHaveLength(1);
    expect(commandPublishes[0]).toEqual({
      source: "merged",
      commands: [
        {
          type: "spawn_event",
          target: "blood_valley",
          params: { event: "beast_tide", intensity: 0.7, duration_ticks: 200 },
        },
      ],
    });
    expect(narrationPublishes).toHaveLength(1);
    expect(narrationPublishes[0]).toHaveLength(3);
  });

  it("keeps world model persistent and updates currentEra deterministically from era narration", async () => {
    const worldModel = new WorldModel();
    const llm = createMockClient(ERA_DECLARATION_RESPONSE);

    const first = await runMockTickForTest({
      llmClient: llm,
      worldModel,
      now: () => 50_000_000,
      model: "mock-model",
      sink: null,
    });

    expect(first.skipped).toBe(false);
    expect(worldModel.latestState?.tick).toBe(84_000);
    expect(worldModel.currentEra).toEqual(
      expect.objectContaining({
        name: "灵潮纪",
        sinceTick: 84_000,
      }),
    );
    expect(worldModel.currentEra?.globalEffect).toContain("灵潮纪已至");

    const sameEra = worldModel.currentEra;

    const second = await runMockTickForTest({
      llmClient: llm,
      worldModel,
      now: () => 50_030_000,
      model: "mock-model",
      sink: null,
    });

    expect(second.skipped).toBe(false);
    expect(worldModel.currentEra).toEqual(sameEra);
    expect(worldModel.lastDecisions.size).toBeGreaterThanOrEqual(1);
  });

  it("forwards redisUrl to runtime in non-mock mode", async () => {
    const runRuntimeSpy = vi.spyOn(runtime, "runRuntime").mockResolvedValue(undefined);
    // plan-agent-ui-data-v1 P2：auxiliaryRuntimeStarter 现在返回 AuxiliaryRuntimeResult
    const auxiliaryRuntimeStarter = vi.fn().mockResolvedValue({ cleanupFns: [], agentUiRuntime: undefined });

    try {
      await main({
        mockMode: false,
        redisUrl: "redis://unit-test:6380",
        baseUrl: "https://llm.example.test/v1",
        apiKey: "k_test",
        model: "mock-model",
        auxiliaryRuntimeStarter,
      });

      // runRuntime 现在接收 config 和 deps（含 agentUiRuntime）
      expect(runRuntimeSpy).toHaveBeenCalledWith(
        {
          mockMode: false,
          redisUrl: "redis://unit-test:6380",
          baseUrl: "https://llm.example.test/v1",
          apiKey: "k_test",
          model: "mock-model",
        },
        { agentUiRuntime: undefined },
      );
      expect(auxiliaryRuntimeStarter).toHaveBeenCalledWith({
        mockMode: false,
        redisUrl: "redis://unit-test:6380",
        baseUrl: "https://llm.example.test/v1",
        apiKey: "k_test",
        model: "mock-model",
      });
    } finally {
      runRuntimeSpy.mockRestore();
    }
  });
});

describe("Agent UI production startup factory", () => {
  it("creates three dedicated Redis connections and privately publishes gate rejection", async () => {
    const clients: MockRedisRuntimeClient[] = [];
    const createRedisClient = vi.fn((_url: string) => {
      const client = makeMockRedisRuntimeClient();
      clients.push(client);
      return client;
    });
    const logSpy = vi.spyOn(console, "log").mockImplementation(() => undefined);

    try {
      const { cleanup, runtime, ready } = await startAgentUiResponseRuntime({
        redisUrl: "redis://factory-test:6380",
        createRedisClient,
      });

      await ready;
      expect(clients[1]?.subscribe).toHaveBeenCalledWith(CHANNELS.AGENT_UI_RESPONSE);
      expect(createRedisClient).toHaveBeenCalledTimes(3);
      expect(createRedisClient).toHaveBeenNthCalledWith(1, "redis://factory-test:6380");
      expect(createRedisClient).toHaveBeenNthCalledWith(2, "redis://factory-test:6380");
      expect(createRedisClient).toHaveBeenNthCalledWith(3, "redis://factory-test:6380");
      expect(new Set(clients).size, "command pub, response sub, and narration pub must be distinct").toBe(3);

      clients[1].emit(
        CHANNELS.AGENT_UI_RESPONSE,
        JSON.stringify({
          request_id: "factory-private-narration",
          action: "error",
          target_player: "offline:TargetOnly",
          params: { reason: "realm_gate_rejected", player_realm: "1", required_realm: "5" },
        }),
      );

      await vi.waitFor(() => expect(clients[2].publish).toHaveBeenCalledOnce());
      expect(clients[1].publish, "subscriber-mode connection must never publish").not.toHaveBeenCalled();
      expect(clients[0].publish, "command publisher must not carry narration").not.toHaveBeenCalled();

      const [channel, rawPayload] = clients[2].publish.mock.calls[0] as [string, string];
      expect(channel).toBe(CHANNELS.AGENT_NARRATE);
      expect(JSON.parse(rawPayload)).toEqual({
        v: 1,
        narrations: [
          {
            scope: "player",
            target: "offline:TargetOnly",
            style: "system_warning",
            text: REALM_GATE_NARRATION_TEXT,
          },
        ],
      });
      expect(rawPayload).not.toContain('"scope":"broadcast"');
      expect(runtime.stats.realmGateRejected).toBe(1);
      expect(runtime.stats.narrationPublished).toBe(1);

      await cleanup();
      for (const client of clients) {
        expect(client.disconnect, "every production factory connection must close on shutdown").toHaveBeenCalled();
      }
    } finally {
      logSpy.mockRestore();
    }
  });

  it("rejects a startup factory that aliases narration publisher to subscriber", async () => {
    const commandPub = makeMockRedisRuntimeClient();
    const responseSub = makeMockRedisRuntimeClient();
    const createRedisClient = vi
      .fn<(url: string) => RedisRuntimeClient>()
      .mockReturnValueOnce(commandPub)
      .mockReturnValueOnce(responseSub)
      .mockReturnValueOnce(responseSub);

    await expect(
      startAgentUiResponseRuntime({ redisUrl: "redis://alias-test:6380", createRedisClient }),
    ).rejects.toThrow(/narration publisher must be distinct from subscriber/);

    expect(createRedisClient).toHaveBeenCalledTimes(3);
    expect(responseSub.subscribe, "alias must fail before entering subscriber mode").not.toHaveBeenCalled();
    expect(commandPub.disconnect).toHaveBeenCalledOnce();
    expect(responseSub.disconnect).toHaveBeenCalledOnce();
  });

  it("closes all three factory connections when response subscription fails", async () => {
    const clients = [
      makeMockRedisRuntimeClient(),
      makeMockRedisRuntimeClient({ subscribeError: new Error("subscribe failed") }),
      makeMockRedisRuntimeClient(),
    ];
    const createRedisClient = vi
      .fn<(url: string) => RedisRuntimeClient>()
      .mockReturnValueOnce(clients[0])
      .mockReturnValueOnce(clients[1])
      .mockReturnValueOnce(clients[2]);
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => undefined);

    try {
      const { cleanup, ready } = await startAgentUiResponseRuntime({
        redisUrl: "redis://subscribe-error:6380",
        createRedisClient,
      });

      await expect(ready).rejects.toThrow("subscribe failed");
      await vi.waitFor(() => {
        for (const client of clients) expect(client.disconnect).toHaveBeenCalled();
      });
      expect(clients[0].publish).not.toHaveBeenCalled();
      expect(clients[1].publish, "failed subscriber connection must never publish").not.toHaveBeenCalled();
      expect(clients[2].publish).not.toHaveBeenCalled();
      expect(warnSpy).toHaveBeenCalledWith(
        "[tiandao] agent ui runtime failed to start:",
        expect.objectContaining({ message: "subscribe failed" }),
      );

      await expect(cleanup(), "cleanup remains idempotent after startup failure").resolves.toBeUndefined();
    } finally {
      warnSpy.mockRestore();
    }
  });
});
