import { describe, expect, it, vi } from "vitest";

import { createMockClient } from "../src/llm.js";
import * as runtime from "../src/runtime.js";
import {
  getMockCompletionMarker,
  main,
  runMockTickForTest,
  type PublishSink,
} from "../src/main.js";
import { WorldModel } from "../src/world-model.js";

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
            scope: "broadcast",
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
      now: () => 1_000_000,
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
      now: () => 1_000_000,
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
      now: () => 1_030_000,
      model: "mock-model",
      sink: null,
    });

    expect(second.skipped).toBe(false);
    expect(worldModel.currentEra).toEqual(sameEra);
    expect(worldModel.lastDecisions.size).toBeGreaterThanOrEqual(1);
  });

  it("forwards redisUrl to runtime in non-mock mode", async () => {
    const runRuntimeSpy = vi.spyOn(runtime, "runRuntime").mockResolvedValue(undefined);

    try {
      await main({
        mockMode: false,
        redisUrl: "redis://unit-test:6380",
        baseUrl: "https://llm.example.test/v1",
        apiKey: "k_test",
        model: "mock-model",
      });

      expect(runRuntimeSpy).toHaveBeenCalledWith({
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
