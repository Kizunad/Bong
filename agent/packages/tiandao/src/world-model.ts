import type { ChatSignal, WorldStateV1, ZoneSnapshot } from "@bong/schema";

import { balanceAdvice, type BalanceAnalysis } from "./balance.js";
import type { AgentDecision } from "./parse.js";

export type ZoneTrend = "rising" | "stable" | "falling";

export const WORLD_MODEL_MAX_ZONE_HISTORY = 10;
export const WORLD_MODEL_RECENT_WINDOW_SIZE = 3;

const DEFAULT_STABLE_DELTA_THRESHOLD = 0.015;
const DEFAULT_MAX_CHAT_SIGNALS = 25;

export interface WorldModelOptions {
  maxZoneHistory?: number;
  recentWindowSize?: number;
  stableDeltaThreshold?: number;
  maxChatSignals?: number;
}

export interface ZoneTrendSnapshot {
  zone: string;
  previousAverageSpiritQi: number;
  recentAverageSpiritQi: number;
  delta: number;
  trend: ZoneTrend;
  samples: number;
}

export interface CurrentEraSnapshot {
  name: string;
  sinceTick: number;
  globalEffect: string;
}

export interface BalanceSnapshot {
  tick: number;
  playerCount: number;
  summary: string;
  analysis: BalanceAnalysis;
}

function cloneValue<T>(value: T): T {
  if (typeof globalThis.structuredClone === "function") {
    return globalThis.structuredClone(value);
  }

  return JSON.parse(JSON.stringify(value)) as T;
}

function average(values: number[]): number {
  if (values.length === 0) return 0;
  return values.reduce((sum, value) => sum + value, 0) / values.length;
}

export class WorldModel {
  latestState: WorldStateV1 | null = null;
  chatSignals: ChatSignal[] = [];
  currentEra: CurrentEraSnapshot | null = null;
  balanceSnapshot: BalanceSnapshot | null = null;
  readonly lastDecisions = new Map<string, AgentDecision>();
  readonly zoneHistory = new Map<string, ZoneSnapshot[]>();

  private readonly maxZoneHistory: number;
  private readonly recentWindowSize: number;
  private readonly stableDeltaThreshold: number;
  private readonly maxChatSignals: number;

  constructor(options: WorldModelOptions = {}) {
    this.maxZoneHistory = options.maxZoneHistory ?? WORLD_MODEL_MAX_ZONE_HISTORY;
    this.recentWindowSize = options.recentWindowSize ?? WORLD_MODEL_RECENT_WINDOW_SIZE;
    this.stableDeltaThreshold = options.stableDeltaThreshold ?? DEFAULT_STABLE_DELTA_THRESHOLD;
    this.maxChatSignals = options.maxChatSignals ?? DEFAULT_MAX_CHAT_SIGNALS;
  }

  updateState(state: WorldStateV1): void {
    this.latestState = cloneValue(state);
    this.balanceSnapshot = this.createBalanceSnapshot(state);

    for (const zone of state.zones) {
      const history = this.zoneHistory.get(zone.name) ?? [];
      history.push(cloneValue(zone));

      if (history.length > this.maxZoneHistory) {
        history.shift();
      }

      this.zoneHistory.set(zone.name, history);
    }
  }

  rememberChatSignals(signals: ChatSignal[]): void {
    const combined = [...this.chatSignals, ...cloneValue(signals)];
    this.chatSignals = combined.slice(-this.maxChatSignals);
  }

  rememberDecision(agentName: string, decision: AgentDecision): void {
    this.lastDecisions.set(agentName, cloneValue(decision));
  }

  rememberCurrentEra(currentEra: CurrentEraSnapshot | null): void {
    this.currentEra = currentEra ? cloneValue(currentEra) : null;
  }

  rememberDecisions(decisions: Iterable<[string, AgentDecision]>): void {
    for (const [agentName, decision] of decisions) {
      this.rememberDecision(agentName, decision);
    }
  }

  getZoneHistory(name: string): ZoneSnapshot[] {
    return cloneValue(this.zoneHistory.get(name) ?? []);
  }

  getZoneTrend(name: string, fallbackZone?: ZoneSnapshot): ZoneTrend {
    return this.getZoneTrendSnapshot(name, fallbackZone).trend;
  }

  getZoneTrendSnapshot(name: string, fallbackZone?: ZoneSnapshot): ZoneTrendSnapshot {
    const recordedHistory = this.zoneHistory.get(name) ?? [];
    const history = recordedHistory.length > 0 ? recordedHistory : fallbackZone ? [fallbackZone] : [];
    const recent = history.slice(-this.recentWindowSize);
    const previous = history.slice(-this.recentWindowSize * 2, -this.recentWindowSize);
    const baseline = previous.length > 0 ? previous : recent;
    const recentAverageSpiritQi = average(recent.map((zone) => zone.spirit_qi));
    const previousAverageSpiritQi = average(baseline.map((zone) => zone.spirit_qi));
    const delta = recentAverageSpiritQi - previousAverageSpiritQi;

    let trend: ZoneTrend = "stable";
    if (delta > this.stableDeltaThreshold) {
      trend = "rising";
    } else if (delta < -this.stableDeltaThreshold) {
      trend = "falling";
    }

    return {
      zone: name,
      previousAverageSpiritQi,
      recentAverageSpiritQi,
      delta,
      trend,
      samples: history.length,
    };
  }

  getWorldTrendSnapshots(zones: readonly ZoneSnapshot[]): ZoneTrendSnapshot[] {
    return zones.map((zone) => this.getZoneTrendSnapshot(zone.name, zone));
  }

  getBalanceSnapshot(): BalanceSnapshot | null {
    return this.balanceSnapshot ? cloneValue(this.balanceSnapshot) : null;
  }

  private createBalanceSnapshot(state: WorldStateV1): BalanceSnapshot {
    const analysis = balanceAdvice(state.players);

    return {
      tick: state.tick,
      playerCount: state.players.length,
      summary: this.summarizeBalance(analysis),
      analysis,
    };
  }

  private summarizeBalance(analysis: BalanceAnalysis): string {
    const leadAdvice = analysis.recommendations[0]?.summary ?? "维持当前平衡";
    return `Gini ${analysis.gini.toFixed(2)} (${analysis.severity}); ${leadAdvice}`;
  }
}
