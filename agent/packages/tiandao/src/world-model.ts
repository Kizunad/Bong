import type {
  BotanyEcologySnapshotV1,
  BotanyZoneEcologyV1,
  NpcSnapshot,
  PlayerProfile,
  WorldStateV1,
  ZonePressureCrossedV1,
  ZoneSnapshot,
} from "@bong/schema";
import { NEWBIE_POWER_THRESHOLD } from "@bong/schema";
import type { AgentDecision } from "./parse.js";
import { summarizeBalance, type BalanceSummary } from "./balance.js";

const MAX_ZONE_HISTORY = 10;
const MAX_BOTANY_ECOLOGY_SNAPSHOTS = 5;
const MAX_ZONE_ANOMALY_HISTORY = 5;
const TREND_WINDOW = 3;
const TREND_EPSILON = 0.02;
const KEY_PLAYER_LIMIT = 3;
const ZONE_STRESS_MIN_PLANTS = 10;
const ZONE_STRESS_QI_THRESHOLD = 0.2;

const AGENT_ORDER = ["calamity", "mutation", "era", "npc_producer"] as const;

const AGENT_DISPLAY_NAMES: Record<string, string> = {
  calamity: "灾劫 Agent",
  mutation: "变化 Agent",
  era: "演绎时代 Agent",
  npc_producer: "NPC 推演器",
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

export interface CurrentEra {
  name: string;
  sinceTick: number;
  globalEffect: string;
}

export interface PeerDecisionSummary {
  agentName: string;
  displayName: string;
  summary: string;
  reasoning: string;
  commandCount: number;
  narrationCount: number;
}

export interface RecentNarrationSummary {
  agentName: string;
  displayName: string;
  scope: AgentDecision["narrations"][number]["scope"];
  target?: string;
  style: AgentDecision["narrations"][number]["style"];
  text: string;
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

export interface WorldModelSnapshot {
  currentEra: CurrentEra | null;
  zoneHistory: Record<string, ZoneSnapshot[]>;
  lastDecisions: Record<string, AgentDecision>;
  playerFirstSeenTick: Record<string, number>;
  lastTick: number | null;
  lastStateTs: number | null;
}

export interface ZoneStressFlag {
  zone: string;
  tick: number;
  spiritQi: number;
  plantCount: number;
  qiUtilization: number;
  plantCountDelta: number;
  spiritQiDelta: number;
  reason: "low_qi_high_density";
}

export interface ZoneAnomalyLog {
  zone: string;
  tick: number;
  taintedCount: number;
  thunderCount: number;
  taintedThresholdExceeded: boolean;
  thunderThresholdExceeded: boolean;
  thunderSpikeRatio: number | null;
}

interface MutableKeyPlayerSummary {
  player: PlayerProfile;
  reasons: string[];
}

export class WorldModel {
  private latestStateValue: WorldStateV1 | null = null;
  private currentEraValue: CurrentEra | null = null;
  private lastStateTsValue: number | null = null;
  readonly zoneHistory = new Map<string, ZoneSnapshot[]>();
  readonly lastDecisions = new Map<string, AgentDecision>();
  private readonly playerFirstSeenTick = new Map<string, number>();
  private botanyEcologyValue: BotanyEcologySnapshotV1 | null = null;
  private readonly botanyEcologySnapshots: BotanyEcologySnapshotV1[] = [];
  readonly botanyEcologyHistory = new Map<string, BotanyZoneEcologyV1[]>();
  readonly zoneStressFlags = new Map<string, ZoneStressFlag>();
  readonly zoneAnomalyHistory = new Map<string, ZoneAnomalyLog[]>();
  readonly latestZonePressureCrossed = new Map<string, ZonePressureCrossedV1>();
  private newPlayersThisTick = new Set<string>();
  private suppressNewPlayersThisTickOnNextUpdate = false;

  static fromState(state: WorldStateV1): WorldModel {
    const model = new WorldModel();
    model.updateState(state);
    return model;
  }

  static fromJSON(snapshot: Partial<WorldModelSnapshot> | null | undefined): WorldModel {
    const model = new WorldModel();
    model.restoreFromJSON(snapshot);
    return model;
  }

  restoreFromJSON(snapshot: Partial<WorldModelSnapshot> | null | undefined): void {
    this.applySnapshot(snapshot ?? {});
  }

  get latestState(): WorldStateV1 | null {
    return this.latestStateValue;
  }

  get currentEra(): CurrentEra | null {
    return cloneCurrentEra(this.currentEraValue);
  }

  get lastTick(): number | null {
    return this.latestStateValue?.tick ?? null;
  }

  get lastStateTs(): number | null {
    return this.lastStateTsValue;
  }

  get botany_ecology(): BotanyEcologySnapshotV1 | null {
    return this.botanyEcologyValue ? cloneBotanyEcologySnapshot(this.botanyEcologyValue) : null;
  }

  toJSON(): WorldModelSnapshot {
    const zoneHistory: Record<string, ZoneSnapshot[]> = {};
    for (const [zoneName, history] of this.zoneHistory.entries()) {
      zoneHistory[zoneName] = history.map(cloneZoneSnapshot);
    }

    const lastDecisions: Record<string, AgentDecision> = {};
    for (const [agentName, decision] of this.lastDecisions.entries()) {
      lastDecisions[agentName] = cloneDecision(decision);
    }

    return {
      currentEra: cloneCurrentEra(this.currentEraValue),
      zoneHistory,
      lastDecisions,
      playerFirstSeenTick: Object.fromEntries(this.playerFirstSeenTick.entries()),
      lastTick: this.lastTick,
      lastStateTs: this.lastStateTs,
    };
  }

  updateState(state: WorldStateV1): void {
    const clonedState = cloneWorldState(state);
    const hadPreviousState = this.latestStateValue !== null;
    const suppressNewPlayersThisTick = this.suppressNewPlayersThisTickOnNextUpdate;
    this.suppressNewPlayersThisTickOnNextUpdate = false;
    this.latestStateValue = clonedState;
    this.lastStateTsValue = clonedState.ts;
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
        if (hadPreviousState && !suppressNewPlayersThisTick) {
          this.newPlayersThisTick.add(player.uuid);
        }
      }
    }
  }

  recordDecision(agentName: string, decision: AgentDecision): void {
    this.lastDecisions.set(agentName, cloneDecision(decision));
  }

  ingestBotanyEcology(snapshot: BotanyEcologySnapshotV1): void {
    const clonedSnapshot = cloneBotanyEcologySnapshot(snapshot);
    this.botanyEcologyValue = clonedSnapshot;
    this.botanyEcologySnapshots.push(cloneBotanyEcologySnapshot(clonedSnapshot));
    if (this.botanyEcologySnapshots.length > MAX_BOTANY_ECOLOGY_SNAPSHOTS) {
      this.botanyEcologySnapshots.shift();
    }

    for (const zone of clonedSnapshot.zones) {
      const history = this.botanyEcologyHistory.get(zone.zone) ?? [];
      const previous = history.at(-1) ?? null;
      history.push(cloneBotanyZoneEcology(zone));
      if (history.length > MAX_BOTANY_ECOLOGY_SNAPSHOTS) {
        history.shift();
      }
      this.botanyEcologyHistory.set(zone.zone, history);

      this.updateZoneStressFlag(clonedSnapshot.tick, zone, previous);
      this.recordZoneAnomaly(clonedSnapshot.tick, zone, previous);
    }
  }

  ingestZonePressureCrossed(event: ZonePressureCrossedV1): void {
    this.latestZonePressureCrossed.set(event.zone, { ...event });
  }

  getRecentBotanyEcologySnapshots(): BotanyEcologySnapshotV1[] {
    return this.botanyEcologySnapshots.map(cloneBotanyEcologySnapshot);
  }

  getBotanyEcologyHistory(zoneName: string): BotanyZoneEcologyV1[] {
    return (this.botanyEcologyHistory.get(zoneName) ?? []).map(cloneBotanyZoneEcology);
  }

  getZoneStressFlags(): ZoneStressFlag[] {
    return [...this.zoneStressFlags.values()].map((flag) => ({ ...flag }));
  }

  getZoneAnomalyWindow(zoneName: string): ZoneAnomalyLog[] {
    return (this.zoneAnomalyHistory.get(zoneName) ?? []).map((entry) => ({ ...entry }));
  }

  getLatestZonePressureCrossed(zoneName: string): ZonePressureCrossedV1 | null {
    const event = this.latestZonePressureCrossed.get(zoneName);
    return event ? { ...event } : null;
  }

  setCurrentEra(currentEra: CurrentEra): void {
    this.currentEraValue = cloneCurrentEra(currentEra);
  }

  rememberCurrentEra(currentEra: CurrentEra): void {
    this.setCurrentEra(currentEra);
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
    if (
      weakest &&
      (!strongest || weakest.uuid !== strongest.uuid) &&
      weakest.composite_power >= NEWBIE_POWER_THRESHOLD
    ) {
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

      const latestSkillMilestone = player.life_record?.skill_milestones.at(-1);
      if (latestSkillMilestone) {
        const skillLabel = describeSkill(latestSkillMilestone.skill);
        addKeyPlayerReason(
          tracked,
          player,
          `技艺突破 ${skillLabel} Lv.${latestSkillMilestone.new_lv}`,
        );
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

  getRecentNarrations(limit = 6): RecentNarrationSummary[] {
    const boundedLimit = Math.max(0, Math.trunc(limit));
    if (boundedLimit === 0) {
      return [];
    }

    const entries: RecentNarrationSummary[] = [];
    for (const [agentName, decision] of [...this.lastDecisions.entries()].sort(([left], [right]) =>
      compareAgentNames(left, right),
    )) {
      for (const narration of decision.narrations) {
        entries.push({
          agentName,
          displayName: AGENT_DISPLAY_NAMES[agentName] ?? `${agentName} Agent`,
          scope: narration.scope,
          target: narration.target,
          style: narration.style,
          text: narration.text,
        });
      }
    }

    return entries.slice(-boundedLimit);
  }

  private applySnapshot(snapshot: Partial<WorldModelSnapshot>): void {
    this.currentEraValue = sanitizeCurrentEra(snapshot.currentEra);

    this.zoneHistory.clear();
    const zoneHistory = sanitizeZoneHistory(snapshot.zoneHistory);
    for (const [zoneName, history] of Object.entries(zoneHistory)) {
      this.zoneHistory.set(zoneName, history);
    }

    this.lastDecisions.clear();
    const lastDecisions = sanitizeLastDecisions(snapshot.lastDecisions);
    for (const [agentName, decision] of Object.entries(lastDecisions)) {
      this.lastDecisions.set(agentName, decision);
    }

    this.playerFirstSeenTick.clear();
    const playerFirstSeenTick = sanitizePlayerFirstSeenTick(snapshot.playerFirstSeenTick);
    for (const [playerId, firstSeenTick] of Object.entries(playerFirstSeenTick)) {
      this.playerFirstSeenTick.set(playerId, firstSeenTick);
    }

    const normalizedLastTick = sanitizeLastTick(snapshot.lastTick);
    const normalizedLastStateTs = sanitizeLastStateTs(snapshot.lastStateTs);
    this.lastStateTsValue = normalizedLastStateTs;
    this.suppressNewPlayersThisTickOnNextUpdate =
      normalizedLastTick !== null && !isRecord(snapshot.playerFirstSeenTick);
    if (normalizedLastTick === null) {
      this.latestStateValue = null;
      this.newPlayersThisTick = new Set<string>();
      return;
    }

    this.latestStateValue = {
      v: 1,
      ts: normalizedLastStateTs ?? 0,
      tick: normalizedLastTick,
      season_state: {
        season: "summer",
        tick_into_phase: normalizedLastTick,
        phase_total_ticks: 1_382_400,
        year_index: 0,
      },
      players: [],
      npcs: [],
      rat_density_heatmap: {
        zones: {},
      },
      zones: [],
      recent_events: [],
    };
    this.newPlayersThisTick = new Set<string>();
  }

  private updateZoneStressFlag(
    tick: number,
    zone: BotanyZoneEcologyV1,
    previous: BotanyZoneEcologyV1 | null,
  ): void {
    const plantCount = totalPlantCount(zone);
    const previousPlantCount = previous ? totalPlantCount(previous) : plantCount;
    const plantCountDelta = plantCount - previousPlantCount;
    const spiritQiDelta = previous ? zone.spirit_qi - previous.spirit_qi : 0;
    const qiUtilization = plantCount / Math.max(zone.spirit_qi, 0.01);

    if (zone.spirit_qi < ZONE_STRESS_QI_THRESHOLD && plantCount >= ZONE_STRESS_MIN_PLANTS) {
      this.zoneStressFlags.set(zone.zone, {
        zone: zone.zone,
        tick,
        spiritQi: zone.spirit_qi,
        plantCount,
        qiUtilization,
        plantCountDelta,
        spiritQiDelta,
        reason: "low_qi_high_density",
      });
      return;
    }

    this.zoneStressFlags.delete(zone.zone);
  }

  private recordZoneAnomaly(
    tick: number,
    zone: BotanyZoneEcologyV1,
    previous: BotanyZoneEcologyV1 | null,
  ): void {
    const taintedCount = variantCount(zone, "tainted");
    const thunderCount = variantCount(zone, "thunder");
    const previousThunderCount = previous ? variantCount(previous, "thunder") : 0;
    const thunderSpikeRatio =
      previousThunderCount > 0 ? thunderCount / previousThunderCount : null;

    const history = this.zoneAnomalyHistory.get(zone.zone) ?? [];
    history.push({
      zone: zone.zone,
      tick,
      taintedCount,
      thunderCount,
      taintedThresholdExceeded: taintedCount > 3,
      thunderThresholdExceeded: thunderCount > 5,
      thunderSpikeRatio,
    });
    if (history.length > MAX_ZONE_ANOMALY_HISTORY) {
      history.shift();
    }
    this.zoneAnomalyHistory.set(zone.zone, history);
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

  const skillBreakthroughReason = reasons.find((reason) => reason.startsWith("技艺突破 "));
  if (skillBreakthroughReason) {
    return `${skillBreakthroughReason}，手艺有成`;
  }

  if (reasons.some((reason) => reason.startsWith("新入世") || reason.startsWith("综合最弱"))) {
    return "天道可扶";
  }

  if (player.breakdown.karma >= 0.3) {
    return "可为秩序锚点";
  }

  return "局势所系";
}

function describeSkill(skill: string): string {
  switch (skill) {
    case "herbalism":
      return "采药";
    case "alchemy":
      return "炼丹";
    case "forging":
      return "锻造";
    default:
      return skill;
  }
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
  const clonedNpcs: NpcSnapshot[] = state.npcs.map((npc): NpcSnapshot => ({
    id: npc.id,
    kind: npc.kind,
    zone: npc.zone,
    pos: [...npc.pos],
    state: npc.state,
    blackboard: { ...npc.blackboard },
    digest: npc.digest
      ? {
          ...npc.digest,
          disciple: npc.digest.disciple
            ? {
                ...npc.digest.disciple,
                lineage: npc.digest.disciple.lineage ? { ...npc.digest.disciple.lineage } : undefined,
                mission_queue: npc.digest.disciple.mission_queue
                  ? { ...npc.digest.disciple.mission_queue }
                  : undefined,
              }
            : undefined,
        }
      : undefined,
  }));

  return {
    v: state.v,
    ts: state.ts,
    tick: state.tick,
    season_state: { ...state.season_state },
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
      cultivation: player.cultivation ? { ...player.cultivation } : undefined,
      life_record: player.life_record
        ? {
            ...player.life_record,
            skill_milestones: player.life_record.skill_milestones.map((milestone) => ({
              ...milestone,
            })),
          }
        : undefined,
      social: player.social
        ? {
            renown: {
              ...player.social.renown,
              top_tags: player.social.renown.top_tags.map((tag) => ({ ...tag })),
            },
            relationships: player.social.relationships.map((relationship) => ({
              ...relationship,
              metadata: cloneJsonValue(relationship.metadata),
            })),
            exposed_to_count: player.social.exposed_to_count,
            faction_membership: player.social.faction_membership
              ? { ...player.social.faction_membership }
              : undefined,
          }
        : undefined,
    })),
    npcs: clonedNpcs,
    factions: state.factions?.map((faction): NonNullable<WorldStateV1["factions"]>[number] => ({
      ...faction,
      leader_lineage: faction.leader_lineage ? { ...faction.leader_lineage } : undefined,
      mission_queue: faction.mission_queue ? { ...faction.mission_queue } : undefined,
    })),
    rat_density_heatmap: {
      zones: Object.fromEntries(
        Object.entries(state.rat_density_heatmap.zones).map(([zoneName, snapshot]) => [
          zoneName,
          { ...snapshot },
        ]),
      ),
    },
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

function cloneBotanyEcologySnapshot(snapshot: BotanyEcologySnapshotV1): BotanyEcologySnapshotV1 {
  return {
    v: snapshot.v,
    tick: snapshot.tick,
    zones: snapshot.zones.map(cloneBotanyZoneEcology),
  };
}

function cloneBotanyZoneEcology(zone: BotanyZoneEcologyV1): BotanyZoneEcologyV1 {
  return {
    zone: zone.zone,
    spirit_qi: zone.spirit_qi,
    plant_counts: zone.plant_counts.map((entry) => ({ ...entry })),
    variant_counts: zone.variant_counts.map((entry) => ({ ...entry })),
  };
}

function totalPlantCount(zone: BotanyZoneEcologyV1): number {
  return zone.plant_counts.reduce((total, entry) => total + entry.count, 0);
}

function variantCount(zone: BotanyZoneEcologyV1, variant: "tainted" | "thunder"): number {
  return zone.variant_counts
    .filter((entry) => entry.variant === variant)
    .reduce((total, entry) => total + entry.count, 0);
}

function cloneJsonValue<T>(value: T): T {
  if (value === undefined || value === null) return value;
  return JSON.parse(JSON.stringify(value)) as T;
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
      kind: narration.kind,
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
    globalEffect: currentEra.globalEffect,
  };
}

function sanitizeCurrentEra(currentEra: unknown): CurrentEra | null {
  if (!isRecord(currentEra)) {
    return null;
  }

  const name = currentEra.name;
  const globalEffect = currentEra.globalEffect;
  const sinceTick = currentEra.sinceTick;

  if (
    typeof name !== "string" ||
    typeof globalEffect !== "string" ||
    typeof sinceTick !== "number" ||
    !Number.isFinite(sinceTick)
  ) {
    return null;
  }

  return {
    name,
    sinceTick,
    globalEffect,
  };
}

function sanitizeZoneHistory(zoneHistory: unknown): Record<string, ZoneSnapshot[]> {
  if (!isRecord(zoneHistory)) {
    return {};
  }

  const normalized: Record<string, ZoneSnapshot[]> = {};
  for (const [zoneName, history] of Object.entries(zoneHistory)) {
    if (!Array.isArray(history)) {
      continue;
    }

    const snapshots: ZoneSnapshot[] = [];
    for (const snapshot of history) {
      const normalizedSnapshot = sanitizeZoneSnapshot(snapshot);
      if (normalizedSnapshot) {
        snapshots.push(normalizedSnapshot);
      }
    }

    if (snapshots.length > 0) {
      normalized[zoneName] = snapshots.slice(-MAX_ZONE_HISTORY);
    }
  }

  return normalized;
}

function sanitizeZoneSnapshot(snapshot: unknown): ZoneSnapshot | null {
  if (!isRecord(snapshot)) {
    return null;
  }

  const name = snapshot.name;
  const spiritQi = sanitizeFiniteNumber(snapshot.spirit_qi);
  const dangerLevel = sanitizeFiniteNumber(snapshot.danger_level);
  const activeEvents = snapshot.active_events;
  const playerCount = sanitizeFiniteNumber(snapshot.player_count);

  if (
    typeof name !== "string" ||
    spiritQi === null ||
    dangerLevel === null ||
    !Array.isArray(activeEvents) ||
    playerCount === null
  ) {
    return null;
  }

  const normalizedActiveEvents = activeEvents.filter(
    (entry): entry is string => typeof entry === "string",
  );

  return {
    name,
    spirit_qi: spiritQi,
    danger_level: dangerLevel,
    active_events: normalizedActiveEvents,
    player_count: playerCount,
  };
}

function sanitizeLastDecisions(lastDecisions: unknown): Record<string, AgentDecision> {
  if (!isRecord(lastDecisions)) {
    return {};
  }

  const normalized: Record<string, AgentDecision> = {};
  for (const [agentName, decision] of Object.entries(lastDecisions)) {
    const normalizedDecision = sanitizeDecision(decision);
    if (normalizedDecision) {
      normalized[agentName] = normalizedDecision;
    }
  }

  return normalized;
}

function sanitizeDecision(decision: unknown): AgentDecision | null {
  if (!isRecord(decision)) {
    return null;
  }

  const commands = decision.commands;
  const narrations = decision.narrations;
  const reasoning = decision.reasoning;

  if (!Array.isArray(commands) || !Array.isArray(narrations) || typeof reasoning !== "string") {
    return null;
  }

  const normalizedCommands: AgentDecision["commands"] = [];
  for (const command of commands) {
    if (!isRecord(command)) {
      continue;
    }

    const type = command.type;
    const target = command.target;
    const params = command.params;
    if (typeof type !== "string" || typeof target !== "string" || !isRecord(params)) {
      continue;
    }

    normalizedCommands.push({
      type: type as AgentDecision["commands"][number]["type"],
      target,
      params: { ...params },
    });
  }

  const normalizedNarrations: AgentDecision["narrations"] = [];
  for (const narration of narrations) {
    if (!isRecord(narration)) {
      continue;
    }

    const scope = narration.scope;
    const target = narration.target;
    const text = narration.text;
    const style = narration.style;
    const kind = narration.kind;
    if (
      typeof scope !== "string" ||
      (target !== undefined && typeof target !== "string") ||
      typeof text !== "string" ||
      typeof style !== "string" ||
      (kind !== undefined && typeof kind !== "string")
    ) {
      continue;
    }

    normalizedNarrations.push({
      scope: scope as AgentDecision["narrations"][number]["scope"],
      target,
      text,
      style: style as AgentDecision["narrations"][number]["style"],
      kind: kind as AgentDecision["narrations"][number]["kind"],
    });
  }

  return cloneDecision({
    commands: normalizedCommands,
    narrations: normalizedNarrations,
    reasoning,
  });
}

function sanitizeLastTick(lastTick: unknown): number | null {
  const normalizedLastTick = sanitizeFiniteNumber(lastTick);
  if (normalizedLastTick === null) {
    return null;
  }

  return normalizedLastTick;
}

function sanitizeLastStateTs(lastStateTs: unknown): number | null {
  const normalizedLastStateTs = sanitizeFiniteNumber(lastStateTs);
  if (normalizedLastStateTs === null) {
    return null;
  }

  return normalizedLastStateTs;
}

function sanitizePlayerFirstSeenTick(playerFirstSeenTick: unknown): Record<string, number> {
  if (!isRecord(playerFirstSeenTick)) {
    return {};
  }

  const normalized: Record<string, number> = {};
  for (const [playerId, firstSeenTick] of Object.entries(playerFirstSeenTick)) {
    const normalizedFirstSeenTick = sanitizeFiniteNumber(firstSeenTick);
    if (normalizedFirstSeenTick !== null) {
      normalized[playerId] = normalizedFirstSeenTick;
    }
  }

  return normalized;
}

function sanitizeFiniteNumber(value: unknown): number | null {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return null;
  }

  return value;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
