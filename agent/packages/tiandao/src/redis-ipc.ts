/**
 * 天道 Redis IPC 适配层：订阅 server 事件、校验 payload，并提供有界 drain 队列。
 *
 * 本模块只处理 Redis 连接、频道分发和 wire payload 的容错解析；tick 编排在
 * `runtime.ts`，WorldModel 的内部 camelCase 快照在 `world-model.ts`。所有
 * snake_case ↔ camelCase 映射集中在 mirror parser，避免业务代码散落协议细节。
 */
import Redis from "ioredis";
const IORedis = Redis.default ?? Redis;
import {
  CHANNELS,
  type ChannelName,
  validateAlchemyInsightV1Contract,
  validateAlchemySessionEndV1Contract,
  validateBoneCoinTickV1Contract,
  validateBotanyEcologySnapshotV1Contract,
  validateFaunaEcologySnapshotV1Contract,
  validateDeathCinematicS2cV1Contract,
  validateFactionEventV1Contract,
  validateNpcDeathV1Contract,
  validateNpcSpawnedV1Contract,
  validatePoiSpawnedEventV1Contract,
  validatePriceIndexV1Contract,
  validateRatPhaseChangeEventV1Contract,
  validateTrespassEventV1Contract,
  validateTsyNpcSpawnedV1Contract,
  validateTsySentinelPhaseChangedV1Contract,
  validateTsyEnterEventV1Contract,
  validateTsyExitEventV1Contract,
  validateTsyZoneActivatedV1Contract,
  validateWeatherEventUpdateV1Contract,
} from "@bong/schema";
import type {
  AgentWorldModelEnvelopeV1,
  AgentWorldModelSnapshotV1,
  AgentCommandV1,
  AlchemySessionEndV1,
  AlchemyInsightV1,
  BoneCoinTickV1,
  BotanyEcologySnapshotV1,
  FaunaEcologySnapshotV1,
  FactionEventV1,
  NarrationV1,
  NpcDeathV1,
  NpcSpawnedV1,
  PoiSpawnedEventV1,
  PriceIndexV1,
  RatPhaseChangeEventV1,
  ChatMessageV1,
  TrespassEventV1,
  TsyNpcSpawnedV1,
  TsySentinelPhaseChangedV1,
  TsyEnterEventV1,
  TsyExitEventV1,
  TsyZoneActivatedV1,
  WeatherEventUpdateV1,
  WorldStateV1,
} from "@bong/schema";
import { parseChatMessages } from "./chat-processor.js";
import type { CommandPublishRequest, NarrationPublishRequest } from "./runtime.js";
import type { WorldModelSnapshot } from "./world-model.js";

const {
  WORLD_STATE,
  AGENT_COMMAND,
  AGENT_NARRATE,
  AGENT_WORLD_MODEL,
  PLAYER_CHAT,
  PRICE_INDEX,
  BONE_COIN_TICK,
  TSY_EVENT,
  NPC_SPAWN,
  NPC_DEATH,
  FACTION_EVENT,
  ALCHEMY_SESSION_END,
  ALCHEMY_INSIGHT,
  BOTANY_ECOLOGY,
  FAUNA_ECOLOGY,
  ZONE_ENVIRONMENT_UPDATE,
  RAT_PHASE_EVENT,
  WEATHER_EVENT_UPDATE,
  AGING,
  LIFESPAN_EVENT,
  DUO_SHE_EVENT,
  BREAKTHROUGH_EVENT,
  CULTIVATION_DEATH,
  DEATH_CINEMATIC,
  FORGE_EVENT,
  FORGE_START,
  FORGE_OUTCOME,
  SOCIAL_EXPOSURE,
  SOCIAL_PACT,
  SOCIAL_FEUD,
  SOCIAL_RENOWN_DELTA,
  SOCIAL_NICHE_INTRUSION,
  COMBAT_REALTIME,
  COMBAT_SUMMARY,
  ARMOR_DURABILITY_CHANGED,
  PSEUDO_VEIN_ACTIVE,
  PSEUDO_VEIN_DISSIPATE,
  REBIRTH,
  SKILL_XP_GAIN,
  SKILL_LV_UP,
  SKILL_CAP_CHANGED,
  SKILL_SCROLL_USED,
  MUTATION_EVENT,
  POI_NOVICE_EVENT,
  SPIRIT_EYE_MIGRATE,
  SPIRIT_EYE_DISCOVERED,
  SPIRIT_EYE_USED_FOR_BREAKTHROUGH,
  SEASON_CHANGED,
} = CHANNELS;

const DEFAULT_CHAT_DRAIN_WINDOW = 128;
const TSY_HOSTILE_EVENT_BUFFER_LIMIT = 128;
const TSY_RUNTIME_EVENT_BUFFER_LIMIT = 128;
const TSY_ZONE_ACTIVATED_BUFFER_LIMIT = 64;
const NPC_EVENT_BUFFER_LIMIT = 128;
const ALCHEMY_EVENT_BUFFER_LIMIT = 128;
const POI_NOVICE_EVENT_BUFFER_LIMIT = 128;
const CROSS_SYSTEM_EVENT_BUFFER_LIMIT = 256;
const ECONOMY_EVENT_BUFFER_LIMIT = 64;
const WEATHER_EVENT_BUFFER_LIMIT = 128;
const DRAIN_COUNTER_KEY = `${PLAYER_CHAT}:drain_counter`;
/** Redis 中持久化 world model mirror 的 hash key。 */
export const WORLD_MODEL_STATE_KEY = "bong:tiandao:state";
/** world model mirror 的稳定 snake_case 字段名。 */
export const WORLD_MODEL_STATE_FIELDS = Object.freeze({
  currentEra: "current_era",
  zoneHistory: "zone_history",
  lastDecisions: "last_decisions",
  playerFirstSeenTick: "player_first_seen_tick",
  negDomainPendingTribulations: "neg_domain_pending_tribulations",
  negDomainEscapeTelemetry: "neg_domain_escape_telemetry",
  negDomainEscapeSessions: "neg_domain_escape_sessions",
  lastTick: "last_tick",
  lastStateTs: "last_state_ts",
});

const REQUIRED_WORLD_MODEL_STATE_FIELDS = Object.freeze([
  WORLD_MODEL_STATE_FIELDS.currentEra,
  WORLD_MODEL_STATE_FIELDS.zoneHistory,
  WORLD_MODEL_STATE_FIELDS.lastDecisions,
  WORLD_MODEL_STATE_FIELDS.playerFirstSeenTick,
  WORLD_MODEL_STATE_FIELDS.lastTick,
  WORLD_MODEL_STATE_FIELDS.lastStateTs,
]);

/** 发布 world model mirror 时使用的 envelope 与关联信息。 */
export interface PublishAgentWorldModelRequest {
  source: NonNullable<AgentWorldModelEnvelopeV1["source"]>;
  snapshot: AgentWorldModelEnvelopeV1["snapshot"];
  metadata: {
    sourceTick: number;
    correlationId: string;
  };
}

/** TSY 敌对事件的受支持联合类型。 */
export type TsyHostileEventV1 = TsyNpcSpawnedV1 | TsySentinelPhaseChangedV1;
/** TSY 进入/退出运行时事件的受支持联合类型。 */
export type TsyRuntimeEventV1 = TsyEnterEventV1 | TsyExitEventV1;
/** NPC 运行时事件的受支持联合类型。 */
export type NpcRuntimeEventV1 = NpcSpawnedV1 | NpcDeathV1 | FactionEventV1;
/** 炼丹运行时事件的受支持联合类型。 */
export type AlchemyRuntimeEventV1 = AlchemySessionEndV1 | AlchemyInsightV1;
/** POI 新手事件的受支持联合类型。 */
export type PoiNoviceRuntimeEventV1 = PoiSpawnedEventV1 | TrespassEventV1;
/** 尚未有专用 runtime handler 的跨系统事件封装。 */
export interface CrossSystemRuntimeEventV1 {
  channel: ChannelName;
  payload: unknown;
}

const CROSS_SYSTEM_EVENT_CHANNELS: readonly ChannelName[] = [
  BOTANY_ECOLOGY,
  FAUNA_ECOLOGY,
  ZONE_ENVIRONMENT_UPDATE,
  AGING,
  LIFESPAN_EVENT,
  DUO_SHE_EVENT,
  BREAKTHROUGH_EVENT,
  CULTIVATION_DEATH,
  DEATH_CINEMATIC,
  FORGE_EVENT,
  FORGE_START,
  FORGE_OUTCOME,
  SOCIAL_EXPOSURE,
  SOCIAL_PACT,
  SOCIAL_FEUD,
  SOCIAL_RENOWN_DELTA,
  SOCIAL_NICHE_INTRUSION,
  COMBAT_REALTIME,
  COMBAT_SUMMARY,
  ARMOR_DURABILITY_CHANGED,
  PSEUDO_VEIN_ACTIVE,
  PSEUDO_VEIN_DISSIPATE,
  REBIRTH,
  SKILL_XP_GAIN,
  SKILL_LV_UP,
  SKILL_CAP_CHANGED,
  SKILL_SCROLL_USED,
  SPIRIT_EYE_MIGRATE,
  SPIRIT_EYE_DISCOVERED,
  SPIRIT_EYE_USED_FOR_BREAKTHROUGH,
  MUTATION_EVENT,
  SEASON_CHANGED,
  PRICE_INDEX,
  BONE_COIN_TICK,
  WEATHER_EVENT_UPDATE,
];
const CROSS_SYSTEM_EVENT_CHANNEL_SET = new Set<string>(CROSS_SYSTEM_EVENT_CHANNELS);

const DRAIN_SCRIPT = `
local items = redis.call('lrange', ARGV[1], 0, -1)
if #items == 0 then return {} end
local counter = redis.call('incr', ARGV[2])
local drainKey = ARGV[1] .. ':drain:' .. counter
redis.call('rename', ARGV[1], drainKey)
local result = redis.call('lrange', drainKey, 0, -1)
redis.call('del', drainKey)
return result
`;

// Selective consumption keeps other consumers' chat messages in the shared list.
// The scan and removal run in one Redis script, so a second consumer cannot take
// the same matching message between the two operations.
const TAKE_MATCHING_PLAYER_CHAT_SCRIPT = `
local items = redis.call('lrange', KEYS[1], 0, -1)
for _, item in ipairs(items) do
  local ok, message = pcall(cjson.decode, item)
  if ok and type(message) == 'table'
      and message.player == ARGV[1]
      and type(message.raw) == 'string'
      and string.find(message.raw, ARGV[2], 1, true) then
    redis.call('lrem', KEYS[1], 1, item)
    return item
  end
end
return false
`;

interface MultiExecResult<T = unknown> {
  0: Error | null;
  1: T;
}

interface RedisMultiLike {
  lrange(key: string, start: number, stop: number): RedisMultiLike;
  ltrim(key: string, start: number, stop: number): RedisMultiLike;
  exec(): Promise<Array<MultiExecResult<unknown>> | null>;
}

/** RedisIpc 实际使用的最小 Redis client 接口，便于注入测试 fake。 */
export interface RedisIpcClient {
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off?(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
  hgetall?(key: string): Promise<Record<string, string>>;
  hset?(key: string, values: Record<string, string>): Promise<number>;
  multi?(): RedisMultiLike;
  eval?(script: string, numKeys: number, ...args: string[]): Promise<unknown>;
}

/** RedisIpc 连接地址与可选 client 工厂。 */
export interface RedisIpcConfig {
  url: string;
  createClient?: (url: string) => RedisIpcClient;
}

/** RedisIpc 的可替换依赖。 */
export interface RedisIpcDeps {
  createClient?: (url: string) => RedisIpcClient;
}

/** 管理订阅、事件缓冲、原子 drain 和 agent outbound 发布的 Redis 适配器。 */
export class RedisIpc {
  private sub: RedisIpcClient;
  private pub: RedisIpcClient;
  private latestState: WorldStateV1 | null = null;
  private latestTsyHostileEvents: TsyHostileEventV1[] = [];
  /** TSY enter/exit drain queue: the shared channel must not silently discard valid runtime signals. */
  private latestTsyRuntimeEvents: TsyRuntimeEventV1[] = [];
  /** plan-agent-ui-data-v1 P2 — tsy_zone_activated drain 队列，供 triggerUi 参考生产路径使用。 */
  private latestTsyZoneActivatedEvents: TsyZoneActivatedV1[] = [];
  private latestNpcEvents: NpcRuntimeEventV1[] = [];
  // plan-offscreen-war-v1 P4：仅 death 事件的 drain 队列（喂 offscreenWarBlock context）。
  // 与 latestNpcEvents（spawn+death+faction 混合，供 getLatestNpcEvents/callback）分开，
  // drain 语义不互相清空。
  private latestNpcDeathEvents: NpcDeathV1[] = [];
  private latestAlchemyEvents: AlchemyRuntimeEventV1[] = [];
  private latestPoiNoviceEvents: PoiNoviceRuntimeEventV1[] = [];
  private latestRatPhaseEvents: RatPhaseChangeEventV1[] = [];
  private latestPriceIndexEvents: PriceIndexV1[] = [];
  private latestBoneCoinTickEvents: BoneCoinTickV1[] = [];
  private latestWeatherEventUpdates: WeatherEventUpdateV1[] = [];
  private latestCrossSystemEvents: CrossSystemRuntimeEventV1[] = [];
  private latestBotanyEcologyEvents: BotanyEcologySnapshotV1[] = [];
  private latestFaunaEcologyEvents: FaunaEcologySnapshotV1[] = [];
  private pendingTsyRuntimeOverflowDropped = 0;
  private stateCallbacks: Array<(state: WorldStateV1) => void> = [];
  private tsyHostileCallbacks: Array<(event: TsyHostileEventV1) => void> = [];
  private tsyRuntimeCallbacks: Array<(event: TsyRuntimeEventV1) => void> = [];
  private npcEventCallbacks: Array<(event: NpcRuntimeEventV1) => void> = [];
  private alchemyEventCallbacks: Array<(event: AlchemyRuntimeEventV1) => void> = [];
  private poiNoviceEventCallbacks: Array<(event: PoiNoviceRuntimeEventV1) => void> = [];
  private ratPhaseEventCallbacks: Array<(event: RatPhaseChangeEventV1) => void> = [];
  private priceIndexCallbacks: Array<(event: PriceIndexV1) => void> = [];
  private boneCoinTickCallbacks: Array<(event: BoneCoinTickV1) => void> = [];
  private weatherEventCallbacks: Array<(event: WeatherEventUpdateV1) => void> = [];
  private crossSystemEventCallbacks: Array<(event: CrossSystemRuntimeEventV1) => void> = [];
  private botanyEcologyCallbacks: Array<(event: BotanyEcologySnapshotV1) => void> = [];
  private faunaEcologyCallbacks: Array<(event: FaunaEcologySnapshotV1) => void> = [];
  private connected = false;
  private readonly onMessage = (channel: string, message: string): void => {
    if (channel === WORLD_STATE) {
      this.handleWorldStateMessage(message);
      return;
    }

    if (channel === TSY_EVENT) {
      this.handleTsyEventMessage(message);
      return;
    }

    if (channel === NPC_SPAWN || channel === NPC_DEATH || channel === FACTION_EVENT) {
      this.handleNpcRuntimeEventMessage(channel, message);
      return;
    }

    if (channel === ALCHEMY_SESSION_END || channel === ALCHEMY_INSIGHT) {
      this.handleAlchemyRuntimeEventMessage(channel, message);
      return;
    }

    if (channel === POI_NOVICE_EVENT) {
      this.handlePoiNoviceEventMessage(message);
      return;
    }

    if (channel === BOTANY_ECOLOGY) {
      this.handleBotanyEcologyMessage(message);
      return;
    }

    if (channel === FAUNA_ECOLOGY) {
      this.handleFaunaEcologyMessage(message);
      return;
    }

    if (channel === RAT_PHASE_EVENT) {
      this.handleRatPhaseEventMessage(message);
      return;
    }

    if (channel === PRICE_INDEX) {
      this.handlePriceIndexMessage(message);
      return;
    }

    if (channel === BONE_COIN_TICK) {
      this.handleBoneCoinTickMessage(message);
      return;
    }

    if (channel === WEATHER_EVENT_UPDATE) {
      this.handleWeatherEventUpdateMessage(message);
      return;
    }

    if (channel === DEATH_CINEMATIC) {
      this.handleDeathCinematicMessage(message);
      return;
    }

    if (CROSS_SYSTEM_EVENT_CHANNEL_SET.has(channel)) {
      this.handleCrossSystemEventMessage(channel as ChannelName, message);
    }
  };

  private handleWorldStateMessage(message: string): void {
    try {
      const state = JSON.parse(message) as WorldStateV1;
      this.latestState = state;
      for (const cb of this.stateCallbacks) {
        cb(state);
      }
    } catch (e) {
      console.warn("[redis-ipc] failed to parse world_state:", e);
    }
  }

  private handleTsyEventMessage(message: string): void {
    try {
      const data = JSON.parse(message) as unknown;
      if (!isObjectRecord(data) || typeof data.kind !== "string") {
        console.warn("[redis-ipc] invalid tsy_event payload: missing string kind");
        return;
      }

      if (data.kind === "tsy_enter") {
        const result = validateTsyEnterEventV1Contract(data);
        if (!result.ok) {
          console.warn("[redis-ipc] invalid tsy_enter event:", result.errors.join("; "));
          return;
        }
        this.recordTsyRuntimeEvent(data as TsyEnterEventV1);
        return;
      }

      if (data.kind === "tsy_exit") {
        const result = validateTsyExitEventV1Contract(data);
        if (!result.ok) {
          console.warn("[redis-ipc] invalid tsy_exit event:", result.errors.join("; "));
          return;
        }
        this.recordTsyRuntimeEvent(data as TsyExitEventV1);
        return;
      }

      if (data.kind === "tsy_npc_spawned") {
        const result = validateTsyNpcSpawnedV1Contract(data);
        if (!result.ok) {
          console.warn("[redis-ipc] invalid tsy_npc_spawned event:", result.errors.join("; "));
          return;
        }
        this.recordTsyHostileEvent(data as TsyNpcSpawnedV1);
        return;
      }

      if (data.kind === "tsy_sentinel_phase_changed") {
        const result = validateTsySentinelPhaseChangedV1Contract(data);
        if (!result.ok) {
          console.warn(
            "[redis-ipc] invalid tsy_sentinel_phase_changed event:",
            result.errors.join("; "),
          );
          return;
        }
        this.recordTsyHostileEvent(data as TsySentinelPhaseChangedV1);
        return;
      }

      // plan-agent-ui-data-v1 P2：秘境激活事件 → drain 队列供 triggerUi 参考生产路径消费。
      if (data.kind === "tsy_zone_activated") {
        const result = validateTsyZoneActivatedV1Contract(data);
        if (!result.ok) {
          console.warn(
            "[redis-ipc] invalid tsy_zone_activated event:",
            result.errors.join("; "),
          );
          return;
        }
        this.recordTsyZoneActivatedEvent(data as TsyZoneActivatedV1);
        return;
      }

      console.warn(`[redis-ipc] unknown tsy_event kind: ${data.kind}`);
    } catch (e) {
      console.warn("[redis-ipc] failed to parse tsy_event:", e);
    }
  }

  private recordTsyRuntimeEvent(event: TsyRuntimeEventV1): void {
    this.latestTsyRuntimeEvents.push(event);
    if (this.latestTsyRuntimeEvents.length > TSY_RUNTIME_EVENT_BUFFER_LIMIT) {
      const droppedCount = this.latestTsyRuntimeEvents.length - TSY_RUNTIME_EVENT_BUFFER_LIMIT;
      this.pendingTsyRuntimeOverflowDropped += droppedCount;
      this.latestTsyRuntimeEvents = this.latestTsyRuntimeEvents.slice(-TSY_RUNTIME_EVENT_BUFFER_LIMIT);
    }
    for (const cb of this.tsyRuntimeCallbacks) {
      cb(event);
    }
  }

  private recordTsyHostileEvent(event: TsyHostileEventV1): void {
    this.latestTsyHostileEvents.push(event);
    if (this.latestTsyHostileEvents.length > TSY_HOSTILE_EVENT_BUFFER_LIMIT) {
      this.latestTsyHostileEvents = this.latestTsyHostileEvents.slice(-TSY_HOSTILE_EVENT_BUFFER_LIMIT);
    }
    for (const cb of this.tsyHostileCallbacks) {
      cb(event);
    }
  }

  private recordTsyZoneActivatedEvent(event: TsyZoneActivatedV1): void {
    this.latestTsyZoneActivatedEvents.push(event);
    if (this.latestTsyZoneActivatedEvents.length > TSY_ZONE_ACTIVATED_BUFFER_LIMIT) {
      this.latestTsyZoneActivatedEvents =
        this.latestTsyZoneActivatedEvents.slice(-TSY_ZONE_ACTIVATED_BUFFER_LIMIT);
    }
  }

  private handlePoiNoviceEventMessage(message: string): void {
    try {
      const data = JSON.parse(message) as unknown;
      if (!isObjectRecord(data) || typeof data.kind !== "string") {
        return;
      }

      if (data.kind === "poi_spawned") {
        const result = validatePoiSpawnedEventV1Contract(data);
        if (!result.ok) {
          console.warn("[redis-ipc] invalid poi_spawned event:", result.errors.join("; "));
          return;
        }
        this.recordPoiNoviceEvent(data as PoiSpawnedEventV1);
        return;
      }

      if (data.kind === "trespass") {
        const result = validateTrespassEventV1Contract(data);
        if (!result.ok) {
          console.warn("[redis-ipc] invalid trespass event:", result.errors.join("; "));
          return;
        }
        this.recordPoiNoviceEvent(data as TrespassEventV1);
      }
    } catch (e) {
      console.warn("[redis-ipc] failed to parse poi novice event:", e);
    }
  }

  private recordPoiNoviceEvent(event: PoiNoviceRuntimeEventV1): void {
    this.latestPoiNoviceEvents.push(event);
    if (this.latestPoiNoviceEvents.length > POI_NOVICE_EVENT_BUFFER_LIMIT) {
      this.latestPoiNoviceEvents = this.latestPoiNoviceEvents.slice(-POI_NOVICE_EVENT_BUFFER_LIMIT);
    }
    for (const cb of this.poiNoviceEventCallbacks) {
      cb(event);
    }
  }

  private handleNpcRuntimeEventMessage(channel: string, message: string): void {
    try {
      const data = JSON.parse(message) as unknown;
      const result =
        channel === NPC_SPAWN
          ? validateNpcSpawnedV1Contract(data)
          : channel === NPC_DEATH
            ? validateNpcDeathV1Contract(data)
            : validateFactionEventV1Contract(data);
      if (!result.ok) {
        console.warn(`[redis-ipc] invalid NPC runtime event on ${channel}:`, result.errors.join("; "));
        return;
      }
      this.recordNpcRuntimeEvent(data as NpcRuntimeEventV1);
    } catch (e) {
      console.warn(`[redis-ipc] failed to parse NPC runtime event on ${channel}:`, e);
    }
  }

  private recordNpcRuntimeEvent(event: NpcRuntimeEventV1): void {
    this.latestNpcEvents.push(event);
    if (this.latestNpcEvents.length > NPC_EVENT_BUFFER_LIMIT) {
      this.latestNpcEvents = this.latestNpcEvents.slice(-NPC_EVENT_BUFFER_LIMIT);
    }
    // plan-offscreen-war-v1 P4：death 事件单独入 drain 队列喂 offscreenWarBlock。
    if (event.kind === "npc_death") {
      this.latestNpcDeathEvents.push(event);
      if (this.latestNpcDeathEvents.length > NPC_EVENT_BUFFER_LIMIT) {
        this.latestNpcDeathEvents = this.latestNpcDeathEvents.slice(-NPC_EVENT_BUFFER_LIMIT);
      }
    }
    for (const cb of this.npcEventCallbacks) {
      cb(event);
    }
  }

  private handleAlchemyRuntimeEventMessage(channel: string, message: string): void {
    try {
      const data = JSON.parse(message) as unknown;
      const result = channel === ALCHEMY_INSIGHT
        ? validateAlchemyInsightV1Contract(data)
        : validateAlchemySessionEndV1Contract(data);
      if (!result.ok) {
        console.warn(`[redis-ipc] invalid alchemy event on ${channel}:`, result.errors.join("; "));
        return;
      }
      this.recordAlchemyRuntimeEvent(data as AlchemyRuntimeEventV1);
    } catch (e) {
      console.warn(`[redis-ipc] failed to parse alchemy event on ${channel}:`, e);
    }
  }

  private recordAlchemyRuntimeEvent(event: AlchemyRuntimeEventV1): void {
    this.latestAlchemyEvents.push(event);
    if (this.latestAlchemyEvents.length > ALCHEMY_EVENT_BUFFER_LIMIT) {
      this.latestAlchemyEvents = this.latestAlchemyEvents.slice(-ALCHEMY_EVENT_BUFFER_LIMIT);
    }
    for (const cb of this.alchemyEventCallbacks) {
      cb(event);
    }
  }

  private handleBotanyEcologyMessage(message: string): void {
    try {
      const data = JSON.parse(message) as unknown;
      const result = validateBotanyEcologySnapshotV1Contract(data);
      if (!result.ok) {
        console.warn("[redis-ipc] invalid botany ecology snapshot:", result.errors.join("; "));
        return;
      }
      this.recordBotanyEcologyEvent(data as BotanyEcologySnapshotV1);
      this.recordCrossSystemEvent({ channel: BOTANY_ECOLOGY, payload: data });
    } catch (e) {
      console.warn("[redis-ipc] failed to parse botany ecology snapshot:", e);
    }
  }

  private handleFaunaEcologyMessage(message: string): void {
    try {
      const data = JSON.parse(message) as unknown;
      const result = validateFaunaEcologySnapshotV1Contract(data);
      if (!result.ok) {
        console.warn("[redis-ipc] invalid fauna ecology snapshot:", result.errors.join("; "));
        return;
      }
      this.recordFaunaEcologyEvent(data as FaunaEcologySnapshotV1);
      this.recordCrossSystemEvent({ channel: FAUNA_ECOLOGY, payload: data });
    } catch (e) {
      console.warn("[redis-ipc] failed to parse fauna ecology snapshot:", e);
    }
  }

  private recordFaunaEcologyEvent(event: FaunaEcologySnapshotV1): void {
    this.latestFaunaEcologyEvents.push(event);
    if (this.latestFaunaEcologyEvents.length > CROSS_SYSTEM_EVENT_BUFFER_LIMIT) {
      this.latestFaunaEcologyEvents =
        this.latestFaunaEcologyEvents.slice(-CROSS_SYSTEM_EVENT_BUFFER_LIMIT);
    }
    for (const cb of this.faunaEcologyCallbacks) {
      cb(event);
    }
  }

  private recordBotanyEcologyEvent(event: BotanyEcologySnapshotV1): void {
    this.latestBotanyEcologyEvents.push(event);
    if (this.latestBotanyEcologyEvents.length > CROSS_SYSTEM_EVENT_BUFFER_LIMIT) {
      this.latestBotanyEcologyEvents =
        this.latestBotanyEcologyEvents.slice(-CROSS_SYSTEM_EVENT_BUFFER_LIMIT);
    }
    for (const cb of this.botanyEcologyCallbacks) {
      cb(event);
    }
  }

  private handleRatPhaseEventMessage(message: string): void {
    try {
      const data = JSON.parse(message) as unknown;
      const result = validateRatPhaseChangeEventV1Contract(data);
      if (!result.ok) {
        console.warn("[redis-ipc] invalid rat phase event:", result.errors.join("; "));
        return;
      }
      this.recordRatPhaseEvent(data as RatPhaseChangeEventV1);
    } catch (e) {
      console.warn("[redis-ipc] failed to parse rat phase event:", e);
    }
  }

  private recordRatPhaseEvent(event: RatPhaseChangeEventV1): void {
    this.latestRatPhaseEvents.push(event);
    if (this.latestRatPhaseEvents.length > CROSS_SYSTEM_EVENT_BUFFER_LIMIT) {
      this.latestRatPhaseEvents = this.latestRatPhaseEvents.slice(-CROSS_SYSTEM_EVENT_BUFFER_LIMIT);
    }
    for (const cb of this.ratPhaseEventCallbacks) {
      cb(event);
    }
  }

  private handlePriceIndexMessage(message: string): void {
    try {
      const data = JSON.parse(message) as unknown;
      const result = validatePriceIndexV1Contract(data);
      if (!result.ok) {
        console.warn("[redis-ipc] invalid price index event:", result.errors.join("; "));
        return;
      }
      this.recordPriceIndexEvent(data as PriceIndexV1);
      this.recordCrossSystemEvent({ channel: PRICE_INDEX, payload: data });
    } catch (e) {
      console.warn("[redis-ipc] failed to parse price index event:", e);
    }
  }

  private recordPriceIndexEvent(event: PriceIndexV1): void {
    this.latestPriceIndexEvents.push(event);
    if (this.latestPriceIndexEvents.length > ECONOMY_EVENT_BUFFER_LIMIT) {
      this.latestPriceIndexEvents = this.latestPriceIndexEvents.slice(-ECONOMY_EVENT_BUFFER_LIMIT);
    }
    for (const cb of this.priceIndexCallbacks) {
      cb(event);
    }
  }

  private handleBoneCoinTickMessage(message: string): void {
    try {
      const data = JSON.parse(message) as unknown;
      const result = validateBoneCoinTickV1Contract(data);
      if (!result.ok) {
        console.warn("[redis-ipc] invalid bone coin tick event:", result.errors.join("; "));
        return;
      }
      this.recordBoneCoinTickEvent(data as BoneCoinTickV1);
      this.recordCrossSystemEvent({ channel: BONE_COIN_TICK, payload: data });
    } catch (e) {
      console.warn("[redis-ipc] failed to parse bone coin tick event:", e);
    }
  }

  private recordBoneCoinTickEvent(event: BoneCoinTickV1): void {
    this.latestBoneCoinTickEvents.push(event);
    if (this.latestBoneCoinTickEvents.length > ECONOMY_EVENT_BUFFER_LIMIT) {
      this.latestBoneCoinTickEvents =
        this.latestBoneCoinTickEvents.slice(-ECONOMY_EVENT_BUFFER_LIMIT);
    }
    for (const cb of this.boneCoinTickCallbacks) {
      cb(event);
    }
  }

  private handleWeatherEventUpdateMessage(message: string): void {
    try {
      const data = JSON.parse(message) as unknown;
      const result = validateWeatherEventUpdateV1Contract(data);
      if (!result.ok) {
        console.warn("[redis-ipc] invalid weather event update:", result.errors.join("; "));
        return;
      }
      this.recordWeatherEventUpdate(data as WeatherEventUpdateV1);
      this.recordCrossSystemEvent({ channel: WEATHER_EVENT_UPDATE, payload: data });
    } catch (e) {
      console.warn("[redis-ipc] failed to parse weather event update:", e);
    }
  }

  private handleDeathCinematicMessage(message: string): void {
    try {
      const data = JSON.parse(message) as unknown;
      const result = validateDeathCinematicS2cV1Contract(data);
      if (!result.ok) {
        console.warn("[redis-ipc] invalid death cinematic:", result.errors.join("; "));
        return;
      }
      this.recordCrossSystemEvent({ channel: DEATH_CINEMATIC, payload: data });
    } catch (e) {
      console.warn("[redis-ipc] failed to parse death cinematic:", e);
    }
  }

  private recordWeatherEventUpdate(event: WeatherEventUpdateV1): void {
    this.latestWeatherEventUpdates.push(event);
    if (this.latestWeatherEventUpdates.length > WEATHER_EVENT_BUFFER_LIMIT) {
      this.latestWeatherEventUpdates =
        this.latestWeatherEventUpdates.slice(-WEATHER_EVENT_BUFFER_LIMIT);
    }
    for (const cb of this.weatherEventCallbacks) {
      cb(event);
    }
  }

  private handleCrossSystemEventMessage(channel: ChannelName, message: string): void {
    try {
      this.recordCrossSystemEvent({ channel, payload: JSON.parse(message) as unknown });
    } catch (e) {
      console.warn(`[redis-ipc] failed to parse cross-system event on ${channel}:`, e);
    }
  }

  private recordCrossSystemEvent(event: CrossSystemRuntimeEventV1): void {
    this.latestCrossSystemEvents.push(event);
    if (this.latestCrossSystemEvents.length > CROSS_SYSTEM_EVENT_BUFFER_LIMIT) {
      this.latestCrossSystemEvents = this.latestCrossSystemEvents.slice(-CROSS_SYSTEM_EVENT_BUFFER_LIMIT);
    }
    for (const cb of this.crossSystemEventCallbacks) {
      cb(event);
    }
  }

  /** 创建订阅与发布两个独立 Redis client，避免 pub/sub 状态互相影响。 */
  constructor(config: RedisIpcConfig, deps?: RedisIpcDeps) {
    const createClient =
      config.createClient ??
      deps?.createClient ??
      ((url: string) => new IORedis(url) as unknown as RedisIpcClient);
    this.sub = createClient(config.url);
    this.pub = createClient(config.url);
  }

  /** 建立订阅并安装单一 message dispatcher；重复调用保持幂等。 */
  async connect(): Promise<void> {
    if (this.connected) {
      return;
    }

    await this.sub.subscribe(WORLD_STATE);
    await this.sub.subscribe(TSY_EVENT);
    await this.sub.subscribe(NPC_SPAWN);
    await this.sub.subscribe(NPC_DEATH);
    await this.sub.subscribe(FACTION_EVENT);
    await this.sub.subscribe(ALCHEMY_SESSION_END);
    await this.sub.subscribe(ALCHEMY_INSIGHT);
    await this.sub.subscribe(POI_NOVICE_EVENT);
    await this.sub.subscribe(RAT_PHASE_EVENT);
    for (const channel of CROSS_SYSTEM_EVENT_CHANNELS) {
      await this.sub.subscribe(channel);
    }
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.connected = true;
    console.log(
      `[redis-ipc] subscribed to ${[WORLD_STATE, TSY_EVENT, NPC_SPAWN, NPC_DEATH, FACTION_EVENT, ALCHEMY_SESSION_END, ALCHEMY_INSIGHT, POI_NOVICE_EVENT, RAT_PHASE_EVENT, ...CROSS_SYSTEM_EVENT_CHANNELS].join(", ")}`,
    );
  }

  /** 返回最近一次收到的 world_state 快照。 */
  getLatestState(): WorldStateV1 | null {
    return this.latestState;
  }

  /** 注册 world_state 观察者；观察者异常由调用方自行处理。 */
  onWorldState(cb: (state: WorldStateV1) => void): void {
    this.stateCallbacks.push(cb);
  }

  /** 返回最近缓冲的 TSY 敌对事件副本。 */
  getLatestTsyHostileEvents(): TsyHostileEventV1[] {
    return [...this.latestTsyHostileEvents];
  }

  /** 注册 TSY 敌对事件观察者。 */
  onTsyHostileEvent(cb: (event: TsyHostileEventV1) => void): void {
    this.tsyHostileCallbacks.push(cb);
  }

  /** 返回尚未 drain 的 TSY enter/exit 事件副本。 */
  getLatestTsyRuntimeEvents(): TsyRuntimeEventV1[] {
    return [...this.latestTsyRuntimeEvents];
  }

  /** 注册 TSY enter/exit 事件观察者。 */
  onTsyRuntimeEvent(cb: (event: TsyRuntimeEventV1) => void): void {
    this.tsyRuntimeCallbacks.push(cb);
  }

  /** 原子语义地取出并清空供单一 Tiandao runtime consumer 使用的 TSY enter/exit 队列。 */
  drainTsyRuntimeEvents(): TsyRuntimeEventV1[] {
    const events = [...this.latestTsyRuntimeEvents];
    if (this.pendingTsyRuntimeOverflowDropped > 0) {
      console.warn(
        `[redis-ipc] tsy runtime event buffer overflow: dropped oldest events ` +
        `(count=${this.pendingTsyRuntimeOverflowDropped}, retained=${TSY_RUNTIME_EVENT_BUFFER_LIMIT})`,
      );
      this.pendingTsyRuntimeOverflowDropped = 0;
    }
    this.latestTsyRuntimeEvents = [];
    return events;
  }

  /** 取出并清空 TSY zone activated 队列，供 UI runtime 每轮触发发现面板。 */
  drainTsyZoneActivatedEvents(): TsyZoneActivatedV1[] {
    const events = [...this.latestTsyZoneActivatedEvents];
    this.latestTsyZoneActivatedEvents = [];
    return events;
  }

  /** 返回最近缓冲的 NPC spawn/death/faction 事件副本。 */
  getLatestNpcEvents(): NpcRuntimeEventV1[] {
    return [...this.latestNpcEvents];
  }

  /** 注册 NPC 运行时事件观察者。 */
  onNpcRuntimeEvent(cb: (event: NpcRuntimeEventV1) => void): void {
    this.npcEventCallbacks.push(cb);
  }

  /** 取出并清空供离屏战斗上下文使用的 death 队列（非 combat 由下游过滤）。 */
  drainNpcDeathEvents(): NpcDeathV1[] {
    const events = [...this.latestNpcDeathEvents];
    this.latestNpcDeathEvents = [];
    return events;
  }

  /** 返回最近缓冲的炼丹事件副本。 */
  getLatestAlchemyEvents(): AlchemyRuntimeEventV1[] {
    return [...this.latestAlchemyEvents];
  }

  /** 注册炼丹事件观察者。 */
  onAlchemyRuntimeEvent(cb: (event: AlchemyRuntimeEventV1) => void): void {
    this.alchemyEventCallbacks.push(cb);
  }

  /** 返回最近缓冲的 POI 新手事件副本。 */
  getLatestPoiNoviceEvents(): PoiNoviceRuntimeEventV1[] {
    return [...this.latestPoiNoviceEvents];
  }

  /** 注册 POI 新手事件观察者。 */
  onPoiNoviceEvent(cb: (event: PoiNoviceRuntimeEventV1) => void): void {
    this.poiNoviceEventCallbacks.push(cb);
  }

  /** 取出并清空蝗灾阶段事件队列。 */
  drainRatPhaseEvents(): RatPhaseChangeEventV1[] {
    const events = [...this.latestRatPhaseEvents];
    this.latestRatPhaseEvents = [];
    return events;
  }

  /** 注册蝗灾阶段事件观察者。 */
  onRatPhaseEvent(cb: (event: RatPhaseChangeEventV1) => void): void {
    this.ratPhaseEventCallbacks.push(cb);
  }

  /** 取出并清空价格指数事件队列。 */
  drainPriceIndexEvents(): PriceIndexV1[] {
    const events = [...this.latestPriceIndexEvents];
    this.latestPriceIndexEvents = [];
    return events;
  }

  /** 注册价格指数事件观察者。 */
  onPriceIndex(cb: (event: PriceIndexV1) => void): void {
    this.priceIndexCallbacks.push(cb);
  }

  /** 取出并清空骨币 tick 事件队列。 */
  drainBoneCoinTickEvents(): BoneCoinTickV1[] {
    const events = [...this.latestBoneCoinTickEvents];
    this.latestBoneCoinTickEvents = [];
    return events;
  }

  /** 注册骨币 tick 事件观察者。 */
  onBoneCoinTick(cb: (event: BoneCoinTickV1) => void): void {
    this.boneCoinTickCallbacks.push(cb);
  }

  /** 取出并清空天气更新事件队列。 */
  drainWeatherEventUpdates(): WeatherEventUpdateV1[] {
    const events = [...this.latestWeatherEventUpdates];
    this.latestWeatherEventUpdates = [];
    return events;
  }

  /** 注册天气更新事件观察者。 */
  onWeatherEventUpdate(cb: (event: WeatherEventUpdateV1) => void): void {
    this.weatherEventCallbacks.push(cb);
  }

  /** 返回最近缓冲的跨系统事件副本。 */
  getLatestCrossSystemEvents(): CrossSystemRuntimeEventV1[] {
    return [...this.latestCrossSystemEvents];
  }

  /** 注册跨系统事件观察者。 */
  onCrossSystemEvent(cb: (event: CrossSystemRuntimeEventV1) => void): void {
    this.crossSystemEventCallbacks.push(cb);
  }

  /** 取出并清空植物生态快照队列。 */
  drainBotanyEcologyEvents(): BotanyEcologySnapshotV1[] {
    const events = [...this.latestBotanyEcologyEvents];
    this.latestBotanyEcologyEvents = [];
    return events;
  }

  /** 注册植物生态快照观察者。 */
  onBotanyEcology(cb: (event: BotanyEcologySnapshotV1) => void): void {
    this.botanyEcologyCallbacks.push(cb);
  }

  /** 取出并清空凡兽生态快照队列。 */
  drainFaunaEcologyEvents(): FaunaEcologySnapshotV1[] {
    const events = [...this.latestFaunaEcologyEvents];
    this.latestFaunaEcologyEvents = [];
    return events;
  }

  /** 注册凡兽生态快照观察者。 */
  onFaunaEcology(cb: (event: FaunaEcologySnapshotV1) => void): void {
    this.faunaEcologyCallbacks.push(cb);
  }

  /** 将仲裁后的命令发布到 agent command 频道。 */
  async publishCommands(request: CommandPublishRequest): Promise<void> {
    const { source, commands, metadata } = request;
    if (commands.length === 0) return;

    const msg: AgentCommandV1 = {
      v: 1,
      id: `cmd_t${metadata.sourceTick}_${source}_${Date.now()}`,
      source,
      commands,
    };

    const json = JSON.stringify(msg);
    const subscribers = await this.pub.publish(AGENT_COMMAND, json);
    console.log(
      `[redis-ipc] published ${commands.length} commands to ${AGENT_COMMAND} (${subscribers} subscribers, source_tick=${metadata.sourceTick}, correlation_id=${metadata.correlationId})`,
    );
  }

  /** 将叙事发布到 agent narration 频道。 */
  async publishNarrations(request: NarrationPublishRequest): Promise<void> {
    const { narrations, metadata } = request;
    if (narrations.length === 0) return;

    const msg: NarrationV1 = {
      v: 1,
      narrations,
    };

    const json = JSON.stringify(msg);
    const subscribers = await this.pub.publish(AGENT_NARRATE, json);
    console.log(
      `[redis-ipc] published ${narrations.length} narrations to ${AGENT_NARRATE} (${subscribers} subscribers, source_tick=${metadata.sourceTick}, correlation_id=${metadata.correlationId})`,
    );
  }

  /** 移除 dispatcher、取消订阅并关闭两个 Redis client。 */
  async disconnect(): Promise<void> {
    this.connected = false;
    this.sub.off?.("message", this.onMessage);
    await this.sub.unsubscribe();
    this.sub.disconnect();
    this.pub.disconnect();
    console.log("[redis-ipc] disconnected");
  }

  /** 将 world model 的 wire 快照发布到 agent world model 频道。 */
  async publishAgentWorldModel(request: PublishAgentWorldModelRequest): Promise<void> {
    const { source, snapshot, metadata } = request;

    const message: AgentWorldModelEnvelopeV1 = {
      v: 1,
      id: `world_model_t${metadata.sourceTick}_${source}_${Date.now()}`,
      source,
      snapshot,
    };

    const json = JSON.stringify(message);
    const subscribers = await this.pub.publish(AGENT_WORLD_MODEL, json);
    console.log(
      `[redis-ipc] published world model to ${AGENT_WORLD_MODEL} (${subscribers} subscribers, source_tick=${metadata.sourceTick}, correlation_id=${metadata.correlationId})`,
    );
  }

  /** 从 Redis mirror 读取并校验 world model；坏 mirror 返回 null。 */
  async loadWorldModelState(options: { logger?: Pick<typeof console, "warn"> } = {}): Promise<WorldModelSnapshot | null> {
    if (!this.pub.hgetall) {
      return null;
    }

    const logger = options.logger ?? console;
    const mirror = await this.pub.hgetall(WORLD_MODEL_STATE_KEY);
    if (Object.keys(mirror).length === 0) {
      return null;
    }

    return parseWorldModelStateMirror(mirror, logger);
  }

  /** 原子取出一批玩家聊天并解析为 schema 消息。 */
  async drainPlayerChat(options: { maxItems?: number; logger?: Pick<typeof console, "warn"> } = {}): Promise<ChatMessageV1[]> {
    const maxItems = options.maxItems ?? DEFAULT_CHAT_DRAIN_WINDOW;
    const logger = options.logger ?? console;
    const raw = await this.drainListAtomically(PLAYER_CHAT, maxItems);
    if (raw.length === 0) {
      return [];
    }
    return parseChatMessages(raw, logger);
  }

  async takeMatchingPlayerChat(options: {
    player: string;
    token: string;
    logger?: Pick<typeof console, "warn">;
  }): Promise<ChatMessageV1 | undefined> {
    if (!this.pub.eval || options.player.length === 0 || options.token.length === 0) {
      return undefined;
    }

    const logger = options.logger ?? console;
    const result = await this.pub.eval(
      TAKE_MATCHING_PLAYER_CHAT_SCRIPT,
      1,
      PLAYER_CHAT,
      options.player,
      options.token,
    );
    if (typeof result !== "string") {
      return undefined;
    }

    return parseChatMessages([result], logger)[0];
  }

  async drainPlayerChatRaw(): Promise<string[]> {
    if (!this.pub.eval) {
      return [];
    }
    const result = await this.pub.eval(DRAIN_SCRIPT, 0, PLAYER_CHAT, DRAIN_COUNTER_KEY);
    return Array.isArray(result) ? (result as string[]) : [];
  }

  private async drainListAtomically(key: string, maxItems: number): Promise<string[]> {
    if (!Number.isFinite(maxItems) || maxItems <= 0) {
      return [];
    }

    if (!this.pub.multi) {
      return [];
    }

    const endIndex = maxItems - 1;
    const trimStart = maxItems;

    const pipeline = this.pub.multi().lrange(key, 0, endIndex).ltrim(key, trimStart, -1);
    const result = await pipeline.exec();
    if (!result) {
      return [];
    }

    const [lrangeResult, ltrimResult] = result as [MultiExecResult<string[]>, MultiExecResult<unknown>];
    if (lrangeResult[0]) {
      throw lrangeResult[0];
    }
    if (ltrimResult[0]) {
      throw ltrimResult[0];
    }

    const rows = lrangeResult[1];
    return Array.isArray(rows) ? rows : [];
  }
}

function parseWorldModelStateMirror(
  mirror: Record<string, string>,
  logger: Pick<typeof console, "warn">,
): WorldModelSnapshot | null {
  const missingFields = REQUIRED_WORLD_MODEL_STATE_FIELDS.filter((field) => !(field in mirror));
  if (missingFields.length > 0) {
    logger.warn(`[redis-ipc] missing world model mirror fields: ${missingFields.join(", ")}`);
    return null;
  }

  // NOTE: server 写入 bong:tiandao:state 的 value 是 serde snake_case JSON
  // （对齐 AgentWorldModelSnapshotV1 wire 形状），所以这里先按 wire 形状解析校验，
  // 再映射回 agent 内部 camelCase 的 WorldModelSnapshot。
  const currentEra = parseJsonField(
    mirror[WORLD_MODEL_STATE_FIELDS.currentEra],
    WORLD_MODEL_STATE_FIELDS.currentEra,
    logger,
    isCurrentEra,
  );
  const zoneHistory = parseJsonField(
    mirror[WORLD_MODEL_STATE_FIELDS.zoneHistory],
    WORLD_MODEL_STATE_FIELDS.zoneHistory,
    logger,
    isZoneHistory,
  );
  const lastDecisions = parseJsonField(
    mirror[WORLD_MODEL_STATE_FIELDS.lastDecisions],
    WORLD_MODEL_STATE_FIELDS.lastDecisions,
    logger,
    isLastDecisions,
  );
  const playerFirstSeenTick = parseJsonField(
    mirror[WORLD_MODEL_STATE_FIELDS.playerFirstSeenTick],
    WORLD_MODEL_STATE_FIELDS.playerFirstSeenTick,
    logger,
    isPlayerFirstSeenTick,
  );
  const negDomainPendingTribulations = parseJsonField(
    mirror[WORLD_MODEL_STATE_FIELDS.negDomainPendingTribulations] ?? "{}",
    WORLD_MODEL_STATE_FIELDS.negDomainPendingTribulations,
    logger,
    isNegDomainPendingTribulations,
  );
  const negDomainEscapeTelemetry = parseJsonField(
    mirror[WORLD_MODEL_STATE_FIELDS.negDomainEscapeTelemetry] ??
      '{"escape_entry_count":0,"post_escape_realm_drop_count":0,"successful_tribulation_avoidance_count":0,"active_escape_session_count":0,"post_escape_realm_drop_rate":0}',
    WORLD_MODEL_STATE_FIELDS.negDomainEscapeTelemetry,
    logger,
    isNegDomainEscapeTelemetry,
  );
  const negDomainEscapeSessions = parseJsonField(
    mirror[WORLD_MODEL_STATE_FIELDS.negDomainEscapeSessions] ?? "{}",
    WORLD_MODEL_STATE_FIELDS.negDomainEscapeSessions,
    logger,
    isNegDomainEscapeSessions,
  );
  const lastTick = parseOptionalIntegerField(
    mirror[WORLD_MODEL_STATE_FIELDS.lastTick],
    WORLD_MODEL_STATE_FIELDS.lastTick,
    logger,
  );
  const lastStateTs = parseOptionalIntegerField(
    mirror[WORLD_MODEL_STATE_FIELDS.lastStateTs],
    WORLD_MODEL_STATE_FIELDS.lastStateTs,
    logger,
  );

  if (
    currentEra === INVALID_MIRROR_FIELD ||
    zoneHistory === INVALID_MIRROR_FIELD ||
    lastDecisions === INVALID_MIRROR_FIELD ||
    playerFirstSeenTick === INVALID_MIRROR_FIELD ||
    negDomainPendingTribulations === INVALID_MIRROR_FIELD ||
    negDomainEscapeTelemetry === INVALID_MIRROR_FIELD ||
    negDomainEscapeSessions === INVALID_MIRROR_FIELD ||
    lastTick === INVALID_MIRROR_FIELD ||
    lastStateTs === INVALID_MIRROR_FIELD
  ) {
    return null;
  }

  return mapWorldModelWireSnapshot({
    current_era: currentEra,
    zone_history: zoneHistory,
    last_decisions: lastDecisions,
    player_first_seen_tick: playerFirstSeenTick,
    neg_domain_pending_tribulations: negDomainPendingTribulations,
    neg_domain_escape_telemetry: negDomainEscapeTelemetry,
    neg_domain_escape_sessions: negDomainEscapeSessions,
    last_tick: lastTick,
    last_state_ts: lastStateTs,
  });
}

const INVALID_MIRROR_FIELD = Symbol("invalid-world-model-mirror-field");

/** 将已校验的 Redis snake_case 快照转换为 WorldModel 的内部形状。 */
function mapWorldModelWireSnapshot(
  snapshot: AgentWorldModelSnapshotV1,
): WorldModelSnapshot {
  return {
    currentEra: snapshot.current_era
      ? {
          name: snapshot.current_era.name,
          sinceTick: snapshot.current_era.since_tick,
          globalEffect: snapshot.current_era.global_effect,
        }
      : null,
    zoneHistory: snapshot.zone_history,
    lastDecisions: snapshot.last_decisions,
    playerFirstSeenTick: snapshot.player_first_seen_tick,
    negDomainPendingTribulations: Object.fromEntries(
      Object.entries(snapshot.neg_domain_pending_tribulations).map(([playerId, pending]) => [
        playerId,
        {
          playerUuid: pending.player_uuid,
          playerName: pending.player_name,
          zone: pending.zone,
          enteredAtTick: pending.entered_at_tick,
          lastSuppressedTick: pending.last_suppressed_tick,
          reason: pending.reason,
        },
      ]),
    ),
    negDomainEscapeTelemetry: {
      escapeEntryCount: snapshot.neg_domain_escape_telemetry.escape_entry_count,
      postEscapeRealmDropCount: snapshot.neg_domain_escape_telemetry.post_escape_realm_drop_count,
      successfulTribulationAvoidanceCount:
        snapshot.neg_domain_escape_telemetry.successful_tribulation_avoidance_count,
      activeEscapeSessionCount: snapshot.neg_domain_escape_telemetry.active_escape_session_count,
      postEscapeRealmDropRate: snapshot.neg_domain_escape_telemetry.post_escape_realm_drop_rate,
    },
    negDomainEscapeSessions: Object.fromEntries(
      Object.entries(snapshot.neg_domain_escape_sessions).map(([playerId, session]) => [
        playerId,
        {
          playerUuid: session.player_uuid,
          playerName: session.player_name,
          zone: session.zone,
          enteredAtTick: session.entered_at_tick,
          entryRealmRank: session.entry_realm_rank,
        },
      ]),
    ),
    lastTick: snapshot.last_tick,
    lastStateTs: snapshot.last_state_ts,
  };
}

function parseJsonField<T>(
  rawValue: string | undefined,
  fieldName: string,
  logger: Pick<typeof console, "warn">,
  validator: (value: unknown) => value is T,
): T | typeof INVALID_MIRROR_FIELD {
  if (rawValue === undefined) {
    logger.warn(`[redis-ipc] missing world model mirror field ${fieldName}`);
    return INVALID_MIRROR_FIELD;
  }

  try {
    const parsed = JSON.parse(rawValue);
    if (!validator(parsed)) {
      logger.warn(`[redis-ipc] invalid world model mirror field ${fieldName}`);
      return INVALID_MIRROR_FIELD;
    }
    return parsed;
  } catch (error) {
    logger.warn(`[redis-ipc] failed to parse world model mirror field ${fieldName}:`, error);
    return INVALID_MIRROR_FIELD;
  }
}

function parseOptionalIntegerField(
  rawValue: string | undefined,
  fieldName: string,
  logger: Pick<typeof console, "warn">,
): number | null | typeof INVALID_MIRROR_FIELD {
  if (rawValue === undefined) {
    logger.warn(`[redis-ipc] missing world model mirror field ${fieldName}`);
    return INVALID_MIRROR_FIELD;
  }

  const trimmed = rawValue.trim();
  if (trimmed.length === 0) {
    return null;
  }

  if (!/^-?\d+$/.test(trimmed)) {
    logger.warn(`[redis-ipc] invalid world model mirror integer field ${fieldName}: ${rawValue}`);
    return INVALID_MIRROR_FIELD;
  }

  const parsed = Number.parseInt(trimmed, 10);
  if (!Number.isSafeInteger(parsed)) {
    logger.warn(`[redis-ipc] out-of-range world model mirror integer field ${fieldName}: ${rawValue}`);
    return INVALID_MIRROR_FIELD;
  }

  return parsed;
}

function isCurrentEra(value: unknown): value is AgentWorldModelSnapshotV1["current_era"] {
  if (value === null) {
    return true;
  }

  if (!isObjectRecord(value)) {
    return false;
  }

  return (
    typeof value.name === "string" &&
    typeof value.since_tick === "number" &&
    Number.isFinite(value.since_tick) &&
    typeof value.global_effect === "string"
  );
}

function isZoneHistory(value: unknown): value is AgentWorldModelSnapshotV1["zone_history"] {
  if (!isObjectRecord(value)) {
    return false;
  }

  return Object.values(value).every((history) => {
    return (
      Array.isArray(history) &&
      history.every((entry) => {
        return (
          isObjectRecord(entry) &&
          typeof entry.name === "string" &&
          typeof entry.spirit_qi === "number" &&
          Number.isFinite(entry.spirit_qi) &&
          typeof entry.danger_level === "number" &&
          Number.isFinite(entry.danger_level) &&
          Array.isArray(entry.active_events) &&
          entry.active_events.every((activeEvent) => typeof activeEvent === "string") &&
          typeof entry.player_count === "number" &&
          Number.isFinite(entry.player_count)
        );
      })
    );
  });
}

function isLastDecisions(value: unknown): value is AgentWorldModelSnapshotV1["last_decisions"] {
  if (!isObjectRecord(value)) {
    return false;
  }

  return Object.values(value).every((decision) => {
    return (
      isObjectRecord(decision) &&
      Array.isArray(decision.commands) &&
      decision.commands.every((command) => {
        return (
          isObjectRecord(command) &&
          typeof command.type === "string" &&
          typeof command.target === "string" &&
          isObjectRecord(command.params)
        );
      }) &&
      Array.isArray(decision.narrations) &&
      decision.narrations.every((narration) => {
        return (
          isObjectRecord(narration) &&
          typeof narration.scope === "string" &&
          (narration.target === undefined || typeof narration.target === "string") &&
          typeof narration.text === "string" &&
          typeof narration.style === "string"
        );
      }) &&
      typeof decision.reasoning === "string"
    );
  });
}

function isPlayerFirstSeenTick(value: unknown): value is AgentWorldModelSnapshotV1["player_first_seen_tick"] {
  if (!isObjectRecord(value)) {
    return false;
  }

  return Object.values(value).every((firstSeenTick) => {
    return typeof firstSeenTick === "number" && Number.isFinite(firstSeenTick);
  });
}

function isNegDomainPendingTribulations(
  value: unknown,
): value is AgentWorldModelSnapshotV1["neg_domain_pending_tribulations"] {
  if (!isObjectRecord(value)) {
    return false;
  }

  return Object.values(value).every((pending) => {
    return (
      isObjectRecord(pending) &&
      typeof pending.player_uuid === "string" &&
      typeof pending.player_name === "string" &&
      typeof pending.zone === "string" &&
      typeof pending.entered_at_tick === "number" &&
      Number.isFinite(pending.entered_at_tick) &&
      typeof pending.last_suppressed_tick === "number" &&
      Number.isFinite(pending.last_suppressed_tick) &&
      pending.reason === "negative_domain_tribulation_exempt"
    );
  });
}

function isNegDomainEscapeTelemetry(
  value: unknown,
): value is AgentWorldModelSnapshotV1["neg_domain_escape_telemetry"] {
  return (
    isObjectRecord(value) &&
    isNonNegativeFiniteNumber(value.escape_entry_count) &&
    Number.isInteger(value.escape_entry_count) &&
    isNonNegativeFiniteNumber(value.post_escape_realm_drop_count) &&
    Number.isInteger(value.post_escape_realm_drop_count) &&
    isNonNegativeFiniteNumber(value.successful_tribulation_avoidance_count) &&
    Number.isInteger(value.successful_tribulation_avoidance_count) &&
    isNonNegativeFiniteNumber(value.active_escape_session_count) &&
    Number.isInteger(value.active_escape_session_count) &&
    isNonNegativeFiniteNumber(value.post_escape_realm_drop_rate)
  );
}

function isNegDomainEscapeSessions(
  value: unknown,
): value is AgentWorldModelSnapshotV1["neg_domain_escape_sessions"] {
  if (!isObjectRecord(value)) {
    return false;
  }

  return Object.values(value).every((session) => {
    return (
      isObjectRecord(session) &&
      typeof session.player_uuid === "string" &&
      typeof session.player_name === "string" &&
      typeof session.zone === "string" &&
      typeof session.entered_at_tick === "number" &&
      Number.isInteger(session.entered_at_tick) &&
      typeof session.entry_realm_rank === "number" &&
      Number.isFinite(session.entry_realm_rank)
    );
  });
}

function isNonNegativeFiniteNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value) && value >= 0;
}

function isObjectRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
