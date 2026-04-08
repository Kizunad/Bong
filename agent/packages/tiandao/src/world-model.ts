import type { PlayerProfile, WorldStateV1, ZoneSnapshot } from "@bong/schema";
import type { AgentDecision } from "./parse.js";
import { summarizeBalance, type BalanceSummary } from "./balance.js";

const MAX_ZONE_HISTORY = 10;
const TREND_WINDOW = 3;
const TREND_EPSILON = 0.02;
const KEY_PLAYER_LIMIT = 4;

const AGENT_ORDER = ["calamity", "mutation", "era"] as const;

const AGENT_DISPLAY_NAMES: Record<string, string> = {
  calamity: "灾劫 Agent",
  mutation: "变化 Agent",
  era: "演绎时代 Agent",
};

export type TrendDirection = "rising" | "stable" | "falling";

export interface ZoneTrendSummary {
  name: string;
  previousSpiritQi: number;
  currentSpiritQi: number;
  delta: number;
  trend: TrendDirection;
}

export interface WorldTrendSummary {
  zones: ZoneTrendSummary[];
  previousSpiritQi: number;
  currentSpiritQi: number;
  delta: number;
  trend: TrendDirection;
}

export interface EraGlobalEffect {
  description: string;
  spiritQiDelta: number;
  dangerLevelDelta: number;
}

export interface CurrentEra {
  name: string;
  sinceTick: number;
  globalEffect: EraGlobalEffect;
}

export interface PeerDecisionSummary {
  agentName: string;
  displayName: string;
  summary: string;
  reasoning: string;
  commandCount: number;
  narrationCount: number;
}

export interface KeyPlayerSummary {
  uuid: string;
  name: string;
  zone: string;
  compositePower: number;
  karma: number;
  recentKills: number;
  recentDeaths: number;
  reasons: string[];
  note: string;
}

interface MutableKeyPlayerSummary {
  player: PlayerProfile;
  reasons: string[];
}

export class WorldModel {
  private latestStateValue: WorldStateV1 | null = null;
  private currentEraValue: CurrentEra | null = null;
  readonly zoneHistory = new Map<string, ZoneSnapshot[]>();
  readonly lastDecisions = new Map<string, AgentDecision>();
  private readonly playerFirstSeenTick = new Map<string, number>();
  private newPlayersThisTick = new Set<string>();

  static fromState(state: WorldStateV1): WorldModel {
    const model = new WorldModel();
    model.updateState(state);
    return model;
  }

  get latestState(): WorldStateV1 | null {
    return this.latestStateValue;
  }

  get currentEra(): CurrentEra | null {
    return cloneCurrentEra(this.currentEraValue);
  }

  updateState(state: WorldStateV1): void {
    const clonedState = cloneWorldState(state);
    const hadPreviousState = this.latestStateValue !== null;
    this.latestStateValue = clonedState;
    this.newPlayersThisTick = new Set<string>();

    for (const zone of clonedState.zones) {
      const history = this.zoneHistory.get(zone.name) ?? [];
      history.push(cloneZoneSnapshot(zone));
      if (history.length > MAX_ZONE_HISTORY) {
        history.shift();
      }
      this.zoneHistory.set(zone.name, history);
    }

    for (const player of clonedState.players) {
      if (!this.playerFirstSeenTick.has(player.uuid)) {
        this.playerFirstSeenTick.set(player.uuid, clonedState.tick);
        if (hadPreviousState) {
          this.newPlayersThisTick.add(player.uuid);
        }
      }
    }
  }

  recordDecision(agentName: string, decision: AgentDecision): void {
    this.lastDecisions.set(agentName, cloneDecision(decision));
  }

  setCurrentEra(currentEra: CurrentEra): void {
    this.currentEraValue = cloneCurrentEra(currentEra);
  }

  getZoneHistory(zoneName: string): ZoneSnapshot[] {
    return (this.zoneHistory.get(zoneName) ?? []).map(cloneZoneSnapshot);
  }

  getZoneTrend(zoneName: string): TrendDirection {
    return this.getZoneTrendSummary(zoneName)?.trend ?? "stable";
  }

  getZoneTrendSummary(zoneName: string): ZoneTrendSummary | null {
    const history = this.zoneHistory.get(zoneName) ?? [];
    if (history.length === 0) {
      return null;
    }

    const values = history.map((zone) => zone.spirit_qi);
    const { previousAverage, currentAverage } = splitTrendWindows(values);
    const delta = currentAverage - previousAverage;

    return {
      name: zoneName,
      previousSpiritQi: previousAverage,
      currentSpiritQi: currentAverage,
      delta,
      trend: classifyTrend(delta),
    };
  }

  getWorldTrendSummary(): WorldTrendSummary | null {
    const state = this.latestStateValue;
    if (!state || state.zones.length === 0) {
      return null;
    }

    const zones: ZoneTrendSummary[] = [];
    for (const zone of state.zones) {
      const summary = this.getZoneTrendSummary(zone.name);
      if (summary) {
        zones.push(summary);
      }
    }

    if (zones.length === 0) {
      return null;
    }

    const previousSpiritQi = average(zones.map((zone) => zone.previousSpiritQi));
    const currentSpiritQi = average(zones.map((zone) => zone.currentSpiritQi));
    const delta = currentSpiritQi - previousSpiritQi;

    return {
      zones,
      previousSpiritQi,
      currentSpiritQi,
      delta,
      trend: classifyTrend(delta),
    };
  }

  getBalanceSummary(): BalanceSummary {
    return summarizeBalance(this.latestStateValue?.players ?? []);
  }

  getKeyPlayers(): KeyPlayerSummary[] {
    const state = this.latestStateValue;
    if (!state || state.players.length === 0) {
      return [];
    }

    const tracked = new Map<string, MutableKeyPlayerSummary>();
    const byPowerDesc = [...state.players].sort((a, b) => b.composite_power - a.composite_power);
    const byPowerAsc = [...state.players].sort((a, b) => a.composite_power - b.composite_power);
    const byAbsKarmaDesc = [...state.players].sort(
      (a, b) => Math.abs(b.breakdown.karma) - Math.abs(a.breakdown.karma),
    );

    const strongest = byPowerDesc[0];
    if (strongest) {
      addKeyPlayerReason(tracked, strongest, `综合最强(${strongest.composite_power.toFixed(2)})`);
    }

    const weakest = byPowerAsc[0];
    if (weakest && (!strongest || weakest.uuid !== strongest.uuid)) {
      addKeyPlayerReason(tracked, weakest, `综合最弱(${weakest.composite_power.toFixed(2)})`);
    }

    const karmaExtremist = byAbsKarmaDesc[0];
    if (karmaExtremist && Math.abs(karmaExtremist.breakdown.karma) >= 0.25) {
      const karmaLabel = karmaExtremist.breakdown.karma >= 0 ? "karma 偏正" : "karma 偏负";
      addKeyPlayerReason(
        tracked,
        karmaExtremist,
        `${karmaLabel}(${karmaExtremist.breakdown.karma.toFixed(2)})`,
      );
    }

    for (const player of state.players) {
      if (player.recent_kills >= 3) {
        addKeyPlayerReason(tracked, player, `连续击杀 ${player.recent_kills} 次`);
      }

      if (this.newPlayersThisTick.has(player.uuid)) {
        addKeyPlayerReason(tracked, player, `新入世(${player.composite_power.toFixed(2)})`);
      }
    }

    return [...tracked.values()]
      .map(({ player, reasons }) => ({
        uuid: player.uuid,
        name: player.name,
        zone: player.zone,
        compositePower: player.composite_power,
        karma: player.breakdown.karma,
        recentKills: player.recent_kills,
        recentDeaths: player.recent_deaths,
        reasons,
        note: summarizeKeyPlayerNote(player, reasons),
      }))
      .sort((a, b) => {
        if (b.reasons.length !== a.reasons.length) {
          return b.reasons.length - a.reasons.length;
        }

        if (b.compositePower !== a.compositePower) {
          return b.compositePower - a.compositePower;
        }

        return a.name.localeCompare(b.name);
      })
      .slice(0, KEY_PLAYER_LIMIT);
  }

  getPeerDecisions(agentName?: string): PeerDecisionSummary[] {
    return [...this.lastDecisions.entries()]
      .filter(([name]) => name !== agentName)
      .sort(([left], [right]) => compareAgentNames(left, right))
      .map(([name, decision]) => ({
        agentName: name,
        displayName: AGENT_DISPLAY_NAMES[name] ?? `${name} Agent`,
        summary: summarizeDecision(decision),
        reasoning: decision.reasoning,
        commandCount: decision.commands.length,
        narrationCount: decision.narrations.length,
      }));
  }
}

function splitTrendWindows(values: number[]): {
  previousAverage: number;
  currentAverage: number;
} {
  if (values.length === 0) {
    return { previousAverage: 0, currentAverage: 0 };
  }

  const currentWindow = values.slice(-TREND_WINDOW);
  const previousWindow = values.slice(-(TREND_WINDOW * 2), -TREND_WINDOW);
  const fallbackPreviousWindow = previousWindow.length > 0 ? previousWindow : values.slice(0, values.length - 1);

  const currentAverage = average(currentWindow);
  const previousAverage = average(
    fallbackPreviousWindow.length > 0 ? fallbackPreviousWindow : currentWindow,
  );

  return {
    previousAverage,
    currentAverage,
  };
}

function classifyTrend(delta: number): TrendDirection {
  if (delta >= TREND_EPSILON) {
    return "rising";
  }

  if (delta <= -TREND_EPSILON) {
    return "falling";
  }

  return "stable";
}

function average(values: number[]): number {
  if (values.length === 0) {
    return 0;
  }

  return values.reduce((acc, value) => acc + value, 0) / values.length;
}

function addKeyPlayerReason(
  tracked: Map<string, MutableKeyPlayerSummary>,
  player: PlayerProfile,
  reason: string,
): void {
  const existing = tracked.get(player.uuid);
  if (existing) {
    if (!existing.reasons.includes(reason)) {
      existing.reasons.push(reason);
    }
    return;
  }

  tracked.set(player.uuid, {
    player,
    reasons: [reason],
  });
}

function summarizeKeyPlayerNote(player: PlayerProfile, reasons: string[]): string {
  if (reasons.some((reason) => reason.startsWith("karma 偏负") || reason.startsWith("连续击杀"))) {
    return "因果将至";
  }

  if (reasons.some((reason) => reason.startsWith("新入世") || reason.startsWith("综合最弱"))) {
    return "天道可扶";
  }

  if (player.breakdown.karma >= 0.3) {
    return "可为秩序锚点";
  }

  return "局势所系";
}

function compareAgentNames(left: string, right: string): number {
  const leftIndex = AGENT_ORDER.indexOf(left as (typeof AGENT_ORDER)[number]);
  const rightIndex = AGENT_ORDER.indexOf(right as (typeof AGENT_ORDER)[number]);
  const normalizedLeftIndex = leftIndex === -1 ? Number.POSITIVE_INFINITY : leftIndex;
  const normalizedRightIndex = rightIndex === -1 ? Number.POSITIVE_INFINITY : rightIndex;

  if (normalizedLeftIndex !== normalizedRightIndex) {
    return normalizedLeftIndex - normalizedRightIndex;
  }

  return left.localeCompare(right);
}

function summarizeDecision(decision: AgentDecision): string {
  if (decision.commands.length === 0) {
    return decision.narrations.length > 0 ? `仅叙事 ${decision.narrations.length} 条` : "无行动";
  }

  return decision.commands.map(describeCommand).join("；");
}

function describeCommand(decisionCommand: AgentDecision["commands"][number]): string {
  if (decisionCommand.type === "modify_zone") {
    const parts: string[] = [];
    const spiritQiDelta = getNumericParam(decisionCommand.params, "spirit_qi_delta");
    const dangerDelta = getNumericParam(decisionCommand.params, "danger_level_delta");

    if (spiritQiDelta !== null) {
      parts.push(`灵气 ${formatSigned(spiritQiDelta)}`);
    }
    if (dangerDelta !== null) {
      parts.push(`危险 ${formatSigned(dangerDelta)}`);
    }

    return `${decisionCommand.target} ${parts.join("，")}`.trim();
  }

  if (decisionCommand.type === "spawn_event") {
    const eventName = getStringParam(decisionCommand.params, "event") ?? "异象";
    const intensity = getNumericParam(decisionCommand.params, "intensity");
    const intensitySuffix = intensity === null ? "" : ` (intensity ${intensity.toFixed(2)})`;
    return `在 ${decisionCommand.target} 降 ${eventName}${intensitySuffix}`;
  }

  if (decisionCommand.type === "npc_behavior") {
    const params = Object.entries(decisionCommand.params)
      .map(([key, value]) => `${key}=${String(value)}`)
      .join(", ");
    return `调整 ${decisionCommand.target} NPC 行为${params ? ` (${params})` : ""}`;
  }

  return `${decisionCommand.target} 执行 ${decisionCommand.type}`;
}

function getNumericParam(params: Record<string, unknown>, key: string): number | null {
  const value = params[key];
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return null;
  }
  return value;
}

function getStringParam(params: Record<string, unknown>, key: string): string | null {
  const value = params[key];
  if (typeof value !== "string") {
    return null;
  }
  return value;
}

function formatSigned(value: number): string {
  const normalized = Math.abs(value) < 0.005 ? 0 : value;
  return `${normalized >= 0 ? "+" : ""}${normalized.toFixed(2)}`;
}

function cloneWorldState(state: WorldStateV1): WorldStateV1 {
  return {
    v: state.v,
    ts: state.ts,
    tick: state.tick,
    players: state.players.map((player) => ({
      uuid: player.uuid,
      name: player.name,
      realm: player.realm,
      composite_power: player.composite_power,
      breakdown: { ...player.breakdown },
      trend: player.trend,
      active_hours: player.active_hours,
      zone: player.zone,
      pos: [...player.pos],
      recent_kills: player.recent_kills,
      recent_deaths: player.recent_deaths,
    })),
    npcs: state.npcs.map((npc) => ({
      id: npc.id,
      kind: npc.kind,
      pos: [...npc.pos],
      state: npc.state,
      blackboard: { ...npc.blackboard },
    })),
    zones: state.zones.map(cloneZoneSnapshot),
    recent_events: state.recent_events.map((event) => ({
      type: event.type,
      tick: event.tick,
      player: event.player,
      target: event.target,
      zone: event.zone,
      details: event.details ? { ...event.details } : undefined,
    })),
  };
}

function cloneZoneSnapshot(zone: ZoneSnapshot): ZoneSnapshot {
  return {
    name: zone.name,
    spirit_qi: zone.spirit_qi,
    danger_level: zone.danger_level,
    active_events: [...zone.active_events],
    player_count: zone.player_count,
  };
}

function cloneDecision(decision: AgentDecision): AgentDecision {
  return {
    commands: decision.commands.map((command) => ({
      type: command.type,
      target: command.target,
      params: { ...command.params },
    })),
    narrations: decision.narrations.map((narration) => ({
      scope: narration.scope,
      target: narration.target,
      text: narration.text,
      style: narration.style,
    })),
    reasoning: decision.reasoning,
  };
}

function cloneCurrentEra(currentEra: CurrentEra | null): CurrentEra | null {
  if (!currentEra) {
    return null;
  }

  return {
    name: currentEra.name,
    sinceTick: currentEra.sinceTick,
    globalEffect: {
      description: currentEra.globalEffect.description,
      spiritQiDelta: currentEra.globalEffect.spiritQiDelta,
      dangerLevelDelta: currentEra.globalEffect.dangerLevelDelta,
    },
  };
}
