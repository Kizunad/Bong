import { describe, expect, it, vi } from "vitest";
import {
  createZeroErrorBreakdown,
  JsonLogSink,
  NoopTelemetrySink,
  RollingSummarySink,
  type TickMetrics,
} from "../src/telemetry.js";

function buildMetrics(tick: number, overrides: Partial<TickMetrics> = {}): TickMetrics {
  return {
    tick,
    timestamp: 1_700_000_000_000 + tick,
    durationMs: 120,
    agentResults: [
      {
        name: "calamity",
        status: "ok",
        durationMs: 40,
        commandCount: 1,
        narrationCount: 0,
        tokensEstimated: 128,
        model: "gpt-5.4-mini",
      },
      {
        name: "mutation",
        status: "skipped",
        durationMs: 5,
        commandCount: 0,
        narrationCount: 0,
        tokensEstimated: 0,
        model: "gpt-5.4-mini",
      },
      {
        name: "era",
        status: "error",
        durationMs: 12,
        commandCount: 0,
        narrationCount: 0,
        tokensEstimated: 0,
        model: "gpt-5.4",
      },
    ],
    mergedCommandCount: 2,
    mergedNarrationCount: 1,
    chatSignalCount: 3,
    eraChanged: false,
    errorBreakdown: {
      ...createZeroErrorBreakdown(),
      timeout: 1,
      backoff: 0,
      parseFail: 2,
      reconnect: 1,
      dedupeDrop: 0,
    },
    staleStateSkipped: false,
    ...overrides,
  };
}

describe("telemetry sinks", () => {
  it("JsonLogSink emits fixed prefix with serialized payload", () => {
    const log = vi.fn();
    const sink = new JsonLogSink({ logger: { log } });

    sink.recordTick(buildMetrics(123));

    expect(log).toHaveBeenCalledTimes(1);
    expect(log).toHaveBeenCalledWith(
      expect.stringMatching(/^\[tiandao:tick\]\s\{.+\}$/),
    );
  });

  it("RollingSummarySink prints deterministic [tiandao:stats] summary every N ticks", () => {
    const log = vi.fn();
    const sink = new RollingSummarySink({ logger: { log }, intervalTicks: 10 });

    for (let tick = 101; tick <= 110; tick += 1) {
      sink.recordTick(buildMetrics(tick));
    }

    expect(log).toHaveBeenCalledTimes(1);
    expect(log).toHaveBeenCalledWith(
      "[tiandao:stats] ticks=10 tick_range=101-110 avg_ms=120.00 commands=20 narrations=10 llm_ok=10/20 timeout=10 backoff=0 parse_fail=20 reconnect=10 dedupe_drop=0 stale_skip=0",
    );
  });

  it("RollingSummarySink flush emits remaining partial window", () => {
    const log = vi.fn();
    const sink = new RollingSummarySink({ logger: { log }, intervalTicks: 10 });

    sink.recordTick(buildMetrics(1));
    sink.recordTick(buildMetrics(2));
    sink.flush();

    expect(log).toHaveBeenCalledTimes(1);
    expect(log).toHaveBeenCalledWith(
      "[tiandao:stats] ticks=2 tick_range=1-2 avg_ms=120.00 commands=4 narrations=2 llm_ok=2/4 timeout=2 backoff=0 parse_fail=4 reconnect=2 dedupe_drop=0 stale_skip=0",
    );
  });

  it("NoopTelemetrySink accepts record/flush without side effects", () => {
    const sink = new NoopTelemetrySink();

    expect(() => sink.recordTick(buildMetrics(99))).not.toThrow();
    expect(() => sink.flush()).not.toThrow();
  });
});
