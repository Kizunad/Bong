import type { NarrationEvaluation } from "./narration-eval.js";

export interface TickErrorBreakdown {
  timeout: number;
  backoff: number;
  parseFail: number;
  reconnect: number;
  dedupeDrop: number;
}

export interface TickAgentMetrics {
  name: string;
  status: "ok" | "skipped" | "error";
  durationMs: number;
  commandCount: number;
  narrationCount: number;
  tokensEstimated: number;
  model: string | null;
}

export interface TickMetrics {
  tick: number;
  timestamp: number;
  durationMs: number;
  agentResults: TickAgentMetrics[];
  mergedCommandCount: number;
  mergedNarrationCount: number;
  chatSignalCount: number;
  eraChanged: boolean;
  errorBreakdown: TickErrorBreakdown;
  staleStateSkipped: boolean;
  narrationScores?: NarrationEvaluation[];
  narrationLowScoreCount?: number;
  narrationAverageScore?: number;
}

export interface TelemetrySink {
  recordTick(metrics: TickMetrics): void | Promise<void>;
  flush(): void | Promise<void>;
}

export type TickAgentResult = TickAgentMetrics;

export class NoopTelemetrySink implements TelemetrySink {
  recordTick(_metrics: TickMetrics): void {}

  flush(): void {}
}

export interface JsonLogSinkOptions {
  logger?: Pick<typeof console, "log">;
}

export class JsonLogSink implements TelemetrySink {
  private readonly logger: Pick<typeof console, "log">;

  constructor(options: JsonLogSinkOptions = {}) {
    this.logger = options.logger ?? console;
  }

  recordTick(metrics: TickMetrics): void {
    this.logger.log(`[tiandao:tick] ${JSON.stringify(metrics)}`);
  }

  flush(): void {}
}

export interface RollingSummarySinkOptions {
  logger?: Pick<typeof console, "log">;
  intervalTicks?: number;
  everyTicks?: number;
}

export class RollingSummarySink implements TelemetrySink {
  private readonly logger: Pick<typeof console, "log">;
  private readonly everyTicks: number;
  private readonly buffer: TickMetrics[] = [];

  constructor(options: RollingSummarySinkOptions = {}) {
    this.logger = options.logger ?? console;
    this.everyTicks = Math.max(1, options.intervalTicks ?? options.everyTicks ?? 10);
  }

  recordTick(metrics: TickMetrics): void {
    this.buffer.push(metrics);

    while (this.buffer.length >= this.everyTicks) {
      this.emitSummary(this.buffer.splice(0, this.everyTicks));
    }
  }

  flush(): void {
    if (this.buffer.length === 0) {
      return;
    }

    this.emitSummary(this.buffer.splice(0, this.buffer.length));
  }

  private emitSummary(metricsBatch: TickMetrics[]): void {
    const ticks = metricsBatch.length;
    const tickStart = metricsBatch[0]?.tick ?? 0;
    const tickEnd = metricsBatch[metricsBatch.length - 1]?.tick ?? tickStart;
    const totalDurationMs = metricsBatch.reduce((sum, metrics) => sum + metrics.durationMs, 0);
    const totalCommands = metricsBatch.reduce((sum, metrics) => sum + metrics.mergedCommandCount, 0);
    const totalNarrations = metricsBatch.reduce((sum, metrics) => sum + metrics.mergedNarrationCount, 0);

    const llmOk = metricsBatch.reduce(
      (sum, metrics) =>
        sum + metrics.agentResults.filter((result) => result.status === "ok").length,
      0,
    );
    const llmAttempted = metricsBatch.reduce(
      (sum, metrics) =>
        sum + metrics.agentResults.filter((result) => result.status !== "skipped").length,
      0,
    );

    const timeout = metricsBatch.reduce(
      (sum, metrics) => sum + metrics.errorBreakdown.timeout,
      0,
    );
    const backoff = metricsBatch.reduce(
      (sum, metrics) => sum + metrics.errorBreakdown.backoff,
      0,
    );
    const parseFail = metricsBatch.reduce(
      (sum, metrics) => sum + metrics.errorBreakdown.parseFail,
      0,
    );
    const reconnect = metricsBatch.reduce(
      (sum, metrics) => sum + metrics.errorBreakdown.reconnect,
      0,
    );
    const dedupeDrop = metricsBatch.reduce(
      (sum, metrics) => sum + metrics.errorBreakdown.dedupeDrop,
      0,
    );
    const staleSkip = metricsBatch.reduce(
      (sum, metrics) => sum + (metrics.staleStateSkipped ? 1 : 0),
      0,
    );

    const avgMs = ticks > 0 ? totalDurationMs / ticks : 0;
    this.logger.log(
      `[tiandao:stats] ticks=${ticks} tick_range=${tickStart}-${tickEnd} avg_ms=${avgMs.toFixed(2)} commands=${totalCommands} narrations=${totalNarrations} llm_ok=${llmOk}/${llmAttempted} timeout=${timeout} backoff=${backoff} parse_fail=${parseFail} reconnect=${reconnect} dedupe_drop=${dedupeDrop} stale_skip=${staleSkip}`,
    );
  }
}

export function createZeroErrorBreakdown(): TickErrorBreakdown {
  return {
    timeout: 0,
    backoff: 0,
    parseFail: 0,
    reconnect: 0,
    dedupeDrop: 0,
  };
}

export function emptyErrorBreakdown(): TickErrorBreakdown {
  return createZeroErrorBreakdown();
}
