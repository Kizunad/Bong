import { describe, expect, it, vi } from "vitest";

import { LlmBackoffError, createMockClient } from "../src/llm.js";
import { runMockTickForTest, type PublishSink } from "../src/main.js";

describe("runtime tick guards", () => {
  it("keeps tick alive when LLM is in backoff and logs skipped metrics", async () => {
    const retryAt = 1_700_000_000;
    const guardedClient = {
      async chat(): Promise<string> {
        throw new LlmBackoffError(retryAt);
      },
      getGuardState() {
        return {
          consecutiveFailures: 3,
          backoffUntil: retryAt,
        };
      },
    };

    const sink: PublishSink = {
      async publishCommands(): Promise<void> {},
      async publishNarrations(): Promise<void> {},
    };

    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const logSpy = vi.spyOn(console, "log").mockImplementation(() => undefined);

    try {
      const summary = await runMockTickForTest({
        llmClient: guardedClient,
        sink,
        now: () => 1_000_000,
        model: "mock-model",
      });

      expect(summary.totalCommands).toBe(0);
      expect(summary.totalNarrations).toBe(0);
      expect(summary.chatSignalCount).toBe(0);
      expect(summary.skipped).toBe(true);

      expect(warnSpy).toHaveBeenCalledWith(
        "[tiandao][calamity] skipped (llm backoff)",
        expect.objectContaining({ retry_at: retryAt }),
      );
      expect(warnSpy).toHaveBeenCalledWith(
        "[tiandao][mutation] skipped (llm backoff)",
        expect.objectContaining({ retry_at: retryAt }),
      );
      expect(warnSpy).toHaveBeenCalledWith(
        "[tiandao][era] skipped (llm backoff)",
        expect.objectContaining({ retry_at: retryAt }),
      );

      expect(logSpy).toHaveBeenCalledWith(
        "[tiandao][tick-metrics]",
        expect.objectContaining({
          command_count: 0,
          narration_count: 0,
          chat_signal_count: 0,
          skipped: true,
          skipped_by_backoff: 3,
        }),
      );
    } finally {
      warnSpy.mockRestore();
      logSpy.mockRestore();
    }
  });

  it("logs deterministic tick metrics for successful mock run", async () => {
    const sink: PublishSink = {
      async publishCommands(): Promise<void> {},
      async publishNarrations(): Promise<void> {},
    };

    const logSpy = vi.spyOn(console, "log").mockImplementation(() => undefined);

    try {
      const summary = await runMockTickForTest({
        llmClient: createMockClient(),
        sink,
        now: () => 1_000_000,
        model: "mock-model",
      });

      expect(summary.chatSignalCount).toBe(0);
      expect(summary.skipped).toBe(false);
      expect(summary.durationMs).toBeGreaterThanOrEqual(0);

      expect(logSpy).toHaveBeenCalledWith(
        "[tiandao][tick-metrics]",
        expect.objectContaining({
          command_count: 0,
          narration_count: 0,
          chat_signal_count: 0,
          skipped: false,
        }),
      );
    } finally {
      logSpy.mockRestore();
    }
  });
});
