/**
 * 天道 world model：保存跨 tick 的世界摘要、趋势和持久化快照。
 *
 * 本模块只负责内存状态、快照复制与恢复，以及供 Agent 读取的摘要；Redis
 * mirror 的 snake_case 编解码在 `redis-ipc.ts`，tick 调度和发布在 `runtime.ts`。
 * 所有 getter 都返回副本，调用方不能通过展示数据反向修改模型。
 */
import type {
  BotanyEcologySnapshotV1,
  BotanyZoneEcologyV1,
  FaunaEcologySnapshotV1,
  FaunaZoneEcologyV1,
  NpcSnapshot,
  PlayerProfile,
  WorldStateV1,
  ZoneSnapshot,
} from "@bong/schema";
import { NEWBIE_POWER_THRESHOLD } from "@bong/schema";
import type { AgentDecision } from "./parse.js";
import { summarizeBalance, type BalanceSummary } from "./balance.js";

const MAX_ZONE_HISTORY = 10;
const MAX_BOTANY_ECOLOGY_SNAPSHOTS = 5;
const MAX_FAUNA_ECOLOGY_SNAPSHOTS = 5;
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

/** 由连续快照灵气变化计算出的趋势方向。 */
export type TrendDirection = "rising" | "stable" | "falling";

/** 单个 zone 的趋势摘要。 */
export interface ZoneTrendSummary {
  name: string;
  previousSpiritQi: number;
  currentSpiritQi: number;
  delta: number;
  trend: TrendDirection;
}

/** 全部 zone 汇总后的世界趋势。 */
export interface WorldTrendSummary {
  zones: ZoneTrendSummary[];
  previousSpiritQi: number;
  currentSpiritQi: number;
  delta: number;
  trend: TrendDirection;
}

/** 天道当前时代的可持久化表示。 */
export interface CurrentEra {
  name: string;
  sinceTick: number;
  globalEffect: string;
}

/** 供某个 Agent 读取的最近决策摘要。 */
export interface PeerDecisionSummary {
  agentName: string;
  displayName: string;
  summary: string;
  reasoning: string;
  commandCount: number;
  narrationCount: number;
}

/** 最近一条叙事的展示摘要。 */
export interface RecentNarrationSummary {
  agentName: string;
  displayName: string;
  scope: AgentDecision["narrations"][number]["scope"];
  target?: string;
  style: AgentDecision["narrations"][number]["style"];
  text: string;
}

/** 供 UI/叙事选择使用的关键玩家摘要。 */
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

/** WorldModel 可持久化的内部 camelCase 快照。 */
export interface WorldModelSnapshot {
  currentEra: CurrentEra | null;
  zoneHistory: Record<string, ZoneSnapshot[]>;
  lastDecisions: Record<string, AgentDecision>;
  playerFirstSeenTick: Record<string, number>;
  negDomainPendingTribulations: Record<string, NegDomainPendingTribulation>;
  negDomainEscapeTelemetry: NegDomainEscapeTelemetrySnapshot;
  negDomainEscapeSessions: Record<string, NegDomainEscapeSession>;
  lastTick: number | null;
  lastStateTs: number | null;
}

/** 负域中被抑制的劫数记录。 */
export interface NegDomainPendingTribulation {
  playerUuid: string;
  playerName: string;
  zone: string;
  enteredAtTick: number;
  lastSuppressedTick: number;
  reason: "negative_domain_tribulation_exempt";
}

/** 负域逃逸遥测的持久化摘要。 */
export interface NegDomainEscapeTelemetrySnapshot {
  escapeEntryCount: number;
  postEscapeRealmDropCount: number;
  successfulTribulationAvoidanceCount: number;
  activeEscapeSessionCount: number;
  postEscapeRealmDropRate: number;
}

/** 单个玩家的负域逃逸会话。 */
export interface NegDomainEscapeSession {
  playerUuid: string;
  playerName: string;
  zone: string;
  enteredAtTick: number;
  entryRealmRank: number;
}

/** 低灵气高密度植物区的压力标记。 */
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

/** 植物生态异常窗口中的统计记录。 */
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

/** 跨 tick 保存世界摘要，并提供快照/恢复边界。 */
export class WorldModel {
  private latestStateValue: WorldStateV1 | null = null;
  private currentEraValue: CurrentEra | null = null;
  private lastStateTsValue: number | null = null;
  readonly zoneHistory = new Map<string, ZoneSnapshot[]>();
  readonly lastDecisions = new Map<string, AgentDecision>();
  private readonly playerFirstSeenTick = new Map<string, number>();
  private readonly negDomainPendingTribulations = new Map<string, NegDomainPendingTribulation>();
  private readonly negDomainEscapeSessions = new Map<string, NegDomainEscapeSession>();
  private negDomainEscapeEntryCount = 0;
  private negDomainPostEscapeRealmDropCount = 0;
  private negDomainSuccessfulTribulationAvoidanceCount = 0;
  private botanyEcologyValue: BotanyEcologySnapshotV1 | null = null;
  private readonly botanyEcologySnapshots: BotanyEcologySnapshotV1[] = [];
  readonly botanyEcologyHistory = new Map<string, BotanyZoneEcologyV1[]>();
  private faunaEcologyValue: FaunaEcologySnapshotV1 | null = null;
  private readonly faunaEcologySnapshots: FaunaEcologySnapshotV1[] = [];
  readonly faunaEcologyHistory = new Map<string, FaunaZoneEcologyV1[]>();
  readonly zoneStressFlags = new Map<string, ZoneStressFlag>();
  readonly zoneAnomalyHistory = new Map<string, ZoneAnomalyLog[]>();
  private newPlayersThisTick = new Set<string>();
  private suppressNewPlayersThisTickOnNextUpdate = false;

  /** 用首个 server world state 初始化模型。 */
  static fromState(state: WorldStateV1): WorldModel {
    const model = new WorldModel();
    model.updateState(state);
    return model;
  }

  /** 从可能不完整或来自旧版本的快照恢复模型。 */
  static fromJSON(snapshot: Partial<WorldModelSnapshot> | null | undefined): WorldModel {
    const model = new WorldModel();
    model.restoreFromJSON(snapshot);
    return model;
  }

  /** 以宽容规则替换持久化状态；无效字段会被丢弃为安全默认值。 */
  restoreFromJSON(snapshot: Partial<WorldModelSnapshot> | null | undefined): void {
    this.applySnapshot(snapshot ?? {});
  }

  /** 返回最近一次 server 状态的只读语义副本。 */
  get latestState(): WorldStateV1 | null {
    return this.latestStateValue;
  }

  /** 返回当前时代副本；没有时代时返回 null。 */
  get currentEra(): CurrentEra | null {
    return cloneCurrentEra(this.currentEraValue);
  }

  /** 返回模型最近状态的 tick。 */
  get lastTick(): number | null {
    return this.latestStateValue?.tick ?? null;
  }

  /** 返回模型最近状态的 server 时间戳。 */
  get lastStateTs(): number | null {
    return this.lastStateTsValue;
  }

  /** 返回最近植物生态快照副本，字段名保持既有 Agent context 契约。 */
  get botany_ecology(): BotanyEcologySnapshotV1 | null {
    return this.botanyEcologyValue ? cloneBotanyEcologySnapshot(this.botanyEcologyValue) : null;
  }

  /** 返回最近凡兽生态快照副本，字段名保持既有 Agent context 契约。 */
  get fauna_ecology(): FaunaEcologySnapshotV1 | null {
    return this.faunaEcologyValue ? cloneFaunaEcologySnapshot(this.faunaEcologyValue) : null;
  }

  /** 导出与 Redis/file mirror 解耦的内部 camelCase 快照副本。 */
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
      negDomainPendingTribulations: Object.fromEntries(this.negDomainPendingTribulations.entries()),
      negDomainEscapeTelemetry: this.getNegDomainEscapeTelemetrySnapshot(),
      negDomainEscapeSessions: Object.fromEntries(this.negDomainEscapeSessions.entries()),
      lastTick: this.lastTick,
      lastStateTs: this.lastStateTs,
    };
  }

  /** 接收一个新的 server 快照并更新历史窗口与新玩家集合。 */
  updateState(state: WorldStateV1): void {
    const clonedState = cloneWorldState(state);
    const hadPreviousState = this.latestStateValue !== null;
    const suppressNewPlayersThisTick = this.suppressNewPlayersThisTickOnNextUpdate;
    this.suppressNewPlayersThisTickOnNextUpdate = false;
    this.latestStateValue = clonedState;
    this.lastStateTsValue = clonedState.ts;
    this.newPlayersThisTick = new Set<string>();

    // plan-era-state-v1 P2：从 WorldStateV1.era 同步时代状态到 currentEraValue。
    // agent era.md skill 可在决策前通过 worldModel.currentEra 获取当前时代，避免重复宣告。
    if (clonedState.era) {
      const eraTypeToName: Record<string, string> = {
        calamity: "calamity",
        change: "mutation",
        deduction: "deduction",
        unknown: "unknown",
      };
      const eraName = eraTypeToName[clonedState.era.era_type] ?? clonedState.era.era_type;
      this.currentEraValue = {
        name: eraName,
        sinceTick: clonedState.era.onset_tick,
        globalEffect: `era_type:${clonedState.era.era_type}|intensity:${clonedState.era.intensity.toFixed(2)}`,
      };
    }
    // 注意：era=undefined 时不清除 currentEraValue（保留 agent 自己记录的时代状态，
    // 避免 server 未回填 era 字段时丢失 agent 内部推演结果）。

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

  /** 记录 Agent 最近一次决策，供 peer context 和快照使用。 */
  recordDecision(agentName: string, decision: AgentDecision): void {
    this.lastDecisions.set(agentName, cloneDecision(decision));
  }

  /** 记录植物生态快照，并更新压力/异常窗口。 */
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

  /** 返回植物生态快照历史窗口副本。 */
  getRecentBotanyEcologySnapshots(): BotanyEcologySnapshotV1[] {
    return this.botanyEcologySnapshots.map(cloneBotanyEcologySnapshot);
  }

  /** 返回指定 zone 的植物生态历史窗口副本。 */
  getBotanyEcologyHistory(zoneName: string): BotanyZoneEcologyV1[] {
    return (this.botanyEcologyHistory.get(zoneName) ?? []).map(cloneBotanyZoneEcology);
  }

  // plan-mundane-fauna-v1 P3：凡兽生态快照存量 + 每 zone 历史（narration 信号，非决策输入）。
  // 只做 value/snapshots/history 三存量，不复用 botany 的 zoneStressFlag/zoneAnomaly 机制
  // （那是 botany variant tainted/thunder 专属）。
  /** 记录凡兽生态快照；它不参与植物专属压力标记。 */
  ingestFaunaEcology(snapshot: FaunaEcologySnapshotV1): void {
    const clonedSnapshot = cloneFaunaEcologySnapshot(snapshot);
    this.faunaEcologyValue = clonedSnapshot;
    this.faunaEcologySnapshots.push(cloneFaunaEcologySnapshot(clonedSnapshot));
    if (this.faunaEcologySnapshots.length > MAX_FAUNA_ECOLOGY_SNAPSHOTS) {
      this.faunaEcologySnapshots.shift();
    }

    for (const zone of clonedSnapshot.zones) {
      const history = this.faunaEcologyHistory.get(zone.zone) ?? [];
      history.push(cloneFaunaZoneEcology(zone));
      if (history.length > MAX_FAUNA_ECOLOGY_SNAPSHOTS) {
        history.shift();
      }
      this.faunaEcologyHistory.set(zone.zone, history);
    }
  }

  /** 返回凡兽生态快照历史窗口副本。 */
  getRecentFaunaEcologySnapshots(): FaunaEcologySnapshotV1[] {
    return this.faunaEcologySnapshots.map(cloneFaunaEcologySnapshot);
  }

  /** 返回指定 zone 的凡兽生态历史窗口副本。 */
  getFaunaEcologyHistory(zoneName: string): FaunaZoneEcologyV1[] {
    return (this.faunaEcologyHistory.get(zoneName) ?? []).map(cloneFaunaZoneEcology);
  }

  /** 返回当前仍处于压力阈值的 zone 标记副本。 */
  getZoneStressFlags(): ZoneStressFlag[] {
    return [...this.zoneStressFlags.values()].map((flag) => ({ ...flag }));
  }

  /** 返回指定 zone 最近的植物异常窗口。 */
  getZoneAnomalyWindow(zoneName: string): ZoneAnomalyLog[] {
    return (this.zoneAnomalyHistory.get(zoneName) ?? []).map((entry) => ({ ...entry }));
  }

  /** 由仲裁结果显式设置当前时代。 */
  setCurrentEra(currentEra: CurrentEra): void {
    this.currentEraValue = cloneCurrentEra(currentEra);
  }

  /** 兼容旧调用方的时代记忆别名，语义等同于 setCurrentEra。 */
  rememberCurrentEra(currentEra: CurrentEra): void {
    this.setCurrentEra(currentEra);
  }

  /** 返回指定 zone 的历史快照副本。 */
  getZoneHistory(zoneName: string): ZoneSnapshot[] {
    return (this.zoneHistory.get(zoneName) ?? []).map(cloneZoneSnapshot);
  }

  /** 返回指定 zone 的趋势方向；没有历史时按 stable 处理。 */
  getZoneTrend(zoneName: string): TrendDirection {
    return this.getZoneTrendSummary(zoneName)?.trend ?? "stable";
  }

  /** 返回指定 zone 的完整趋势摘要。 */
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

  /** 汇总当前世界所有 zone 的趋势。 */
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

  /** 计算当前在线玩家的功德/力量分布摘要。 */
  getBalanceSummary(): BalanceSummary {
    return summarizeBalance(this.latestStateValue?.players ?? []);
  }

  /** 记录负域中被抑制、等待后续处理的劫数。 */
  recordNegDomainPendingTribulation(args: {
    playerUuid: string;
    playerName: string;
    zone: string;
    tick: number;
    reason: NegDomainPendingTribulation["reason"];
  }): void {
    const existing = this.negDomainPendingTribulations.get(args.playerUuid);
    this.negDomainPendingTribulations.set(args.playerUuid, {
      playerUuid: args.playerUuid,
      playerName: args.playerName,
      zone: args.zone,
      enteredAtTick: existing?.enteredAtTick ?? args.tick,
      lastSuppressedTick: args.tick,
      reason: args.reason,
    });
  }

  /** 判断玩家是否有待处理的负域劫数。 */
  hasNegDomainPendingTribulation(playerUuid: string): boolean {
    return this.negDomainPendingTribulations.has(playerUuid);
  }

  /** 返回玩家待处理的负域劫数副本。 */
  getNegDomainPendingTribulation(playerUuid: string): NegDomainPendingTribulation | null {
    const pending = this.negDomainPendingTribulations.get(playerUuid);
    return pending ? { ...pending } : null;
  }

  /** 清除玩家待处理的负域劫数。 */
  clearNegDomainPendingTribulation(playerUuid: string): void {
    this.negDomainPendingTribulations.delete(playerUuid);
  }

  /** 读取并同时清除玩家待处理的负域劫数。 */
  consumeNegDomainPendingTribulation(playerUuid: string): NegDomainPendingTribulation | null {
    const pending = this.getNegDomainPendingTribulation(playerUuid);
    this.clearNegDomainPendingTribulation(playerUuid);
    return pending;
  }

  /** 记录玩家进入负域并开启逃逸会话。 */
  recordNegDomainEscapeEntry(args: {
    playerUuid: string;
    playerName: string;
    zone: string;
    tick: number;
    entryRealmRank: number;
  }): void {
    if (this.negDomainEscapeSessions.has(args.playerUuid)) {
      return;
    }

    this.negDomainEscapeEntryCount += 1;
    this.negDomainEscapeSessions.set(args.playerUuid, {
      playerUuid: args.playerUuid,
      playerName: args.playerName,
      zone: args.zone,
      enteredAtTick: args.tick,
      entryRealmRank: args.entryRealmRank,
    });
  }

  /** 记录玩家离开负域，并统计境界下降。 */
  recordNegDomainEscapeExit(args: { playerUuid: string; exitRealmRank: number }): void {
    const session = this.negDomainEscapeSessions.get(args.playerUuid);
    if (!session) {
      return;
    }

    if (args.exitRealmRank < session.entryRealmRank) {
      this.negDomainPostEscapeRealmDropCount += 1;
    }
    this.negDomainEscapeSessions.delete(args.playerUuid);
  }

  /** 记录一次成功避免负域劫数的结果。 */
  recordSuccessfulNegDomainTribulationAvoidance(): void {
    this.negDomainSuccessfulTribulationAvoidanceCount += 1;
  }

  /** 返回负域逃逸计数和比例的快照副本。 */
  getNegDomainEscapeTelemetrySnapshot(): NegDomainEscapeTelemetrySnapshot {
    return {
      escapeEntryCount: this.negDomainEscapeEntryCount,
      postEscapeRealmDropCount: this.negDomainPostEscapeRealmDropCount,
      successfulTribulationAvoidanceCount: this.negDomainSuccessfulTribulationAvoidanceCount,
      activeEscapeSessionCount: this.negDomainEscapeSessions.size,
      postEscapeRealmDropRate:
        this.negDomainEscapeEntryCount > 0
          ? this.negDomainPostEscapeRealmDropCount / this.negDomainEscapeEntryCount
          : 0,
    };
  }

  /** 按力量、因果和近期事件选出有限数量的关键玩家。 */
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

  /** 返回除指定 Agent 外的最近决策摘要。 */
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

  /** 返回最近的叙事摘要，limit 会被规整为非负整数。 */
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

    this.negDomainPendingTribulations.clear();
    const pendingTribulations = sanitizeNegDomainPendingTribulations(
      snapshot.negDomainPendingTribulations,
    );
    for (const [playerId, pending] of Object.entries(pendingTribulations)) {
      this.negDomainPendingTribulations.set(playerId, pending);
    }

    this.negDomainEscapeSessions.clear();
    const escapeSessions = sanitizeNegDomainEscapeSessions(snapshot.negDomainEscapeSessions);
    for (const [playerId, session] of Object.entries(escapeSessions)) {
      this.negDomainEscapeSessions.set(playerId, session);
    }

    const escapeTelemetry = sanitizeNegDomainEscapeTelemetry(snapshot.negDomainEscapeTelemetry);
    this.negDomainEscapeEntryCount = escapeTelemetry.escapeEntryCount;
    this.negDomainPostEscapeRealmDropCount = escapeTelemetry.postEscapeRealmDropCount;
    this.negDomainSuccessfulTribulationAvoidanceCount =
      escapeTelemetry.successfulTribulationAvoidanceCount;

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
    // plan-era-state-v1 P2: 携带时代状态字段
    era: state.era ? { ...state.era } : undefined,
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

function cloneFaunaEcologySnapshot(snapshot: FaunaEcologySnapshotV1): FaunaEcologySnapshotV1 {
  return {
    v: snapshot.v,
    tick: snapshot.tick,
    zones: snapshot.zones.map(cloneFaunaZoneEcology),
  };
}

function cloneFaunaZoneEcology(zone: FaunaZoneEcologyV1): FaunaZoneEcologyV1 {
  return {
    zone: zone.zone,
    spirit_qi: zone.spirit_qi,
    species_counts: zone.species_counts.map((entry) => ({ ...entry })),
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

function sanitizeNegDomainPendingTribulations(
  pendingTribulations: unknown,
): Record<string, NegDomainPendingTribulation> {
  if (!isRecord(pendingTribulations)) {
    return {};
  }

  const normalized: Record<string, NegDomainPendingTribulation> = {};
  for (const [playerId, pending] of Object.entries(pendingTribulations)) {
    if (!isRecord(pending)) {
      continue;
    }

    const playerUuid = typeof pending.playerUuid === "string" ? pending.playerUuid : playerId;
    const playerName = typeof pending.playerName === "string" ? pending.playerName : playerUuid;
    const zone = typeof pending.zone === "string" ? pending.zone : "";
    const enteredAtTick = sanitizeFiniteNumber(pending.enteredAtTick);
    const lastSuppressedTick = sanitizeFiniteNumber(pending.lastSuppressedTick);
    if (zone.length === 0 || enteredAtTick === null || lastSuppressedTick === null) {
      continue;
    }

    normalized[playerUuid] = {
      playerUuid,
      playerName,
      zone,
      enteredAtTick,
      lastSuppressedTick,
      reason: "negative_domain_tribulation_exempt",
    };
  }

  return normalized;
}

function sanitizeNegDomainEscapeTelemetry(
  telemetry: unknown,
): Omit<NegDomainEscapeTelemetrySnapshot, "activeEscapeSessionCount" | "postEscapeRealmDropRate"> {
  if (!isRecord(telemetry)) {
    return {
      escapeEntryCount: 0,
      postEscapeRealmDropCount: 0,
      successfulTribulationAvoidanceCount: 0,
    };
  }

  return {
    escapeEntryCount: sanitizeNonNegativeInteger(telemetry.escapeEntryCount),
    postEscapeRealmDropCount: sanitizeNonNegativeInteger(telemetry.postEscapeRealmDropCount),
    successfulTribulationAvoidanceCount: sanitizeNonNegativeInteger(
      telemetry.successfulTribulationAvoidanceCount,
    ),
  };
}

function sanitizeNegDomainEscapeSessions(
  sessions: unknown,
): Record<string, NegDomainEscapeSession> {
  if (!isRecord(sessions)) {
    return {};
  }

  const normalized: Record<string, NegDomainEscapeSession> = {};
  for (const [playerId, session] of Object.entries(sessions)) {
    if (!isRecord(session)) {
      continue;
    }

    const playerUuid = typeof session.playerUuid === "string" ? session.playerUuid : playerId;
    const playerName = typeof session.playerName === "string" ? session.playerName : playerUuid;
    const zone = typeof session.zone === "string" ? session.zone : "";
    const enteredAtTick = sanitizeFiniteNumber(session.enteredAtTick);
    const entryRealmRank = sanitizeFiniteNumber(session.entryRealmRank);
    if (zone.length === 0 || enteredAtTick === null || entryRealmRank === null) {
      continue;
    }

    normalized[playerUuid] = {
      playerUuid,
      playerName,
      zone,
      enteredAtTick,
      entryRealmRank,
    };
  }

  return normalized;
}

function sanitizeFiniteNumber(value: unknown): number | null {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return null;
  }

  return value;
}

function sanitizeNonNegativeInteger(value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0) {
    return 0;
  }

  return Math.floor(value);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
