import Redis from "ioredis";
const IORedis = Redis.default ?? Redis;
import {
  CHANNELS,
  type ChannelName,
  validateAlchemyInsightV1Contract,
  validateAlchemySessionEndV1Contract,
  validateFactionEventV1Contract,
  validateNpcDeathV1Contract,
  validateNpcSpawnedV1Contract,
  validatePoiSpawnedEventV1Contract,
  validateTrespassEventV1Contract,
  validateTsyNpcSpawnedV1Contract,
  validateTsySentinelPhaseChangedV1Contract,
} from "@bong/schema";
import type {
  AgentWorldModelEnvelopeV1,
  AgentWorldModelSnapshotV1,
  AgentCommandV1,
  AlchemySessionEndV1,
  AlchemyInsightV1,
  FactionEventV1,
  NarrationV1,
  NpcDeathV1,
  NpcSpawnedV1,
  PoiSpawnedEventV1,
  ChatMessageV1,
  TrespassEventV1,
  TsyNpcSpawnedV1,
  TsySentinelPhaseChangedV1,
  WorldStateV1,
} from "@bong/schema";
import { parseChatMessages } from "./chat-processor.js";
import type { CommandPublishRequest, NarrationPublishRequest } from "./runtime.js";

const {
  WORLD_STATE,
  AGENT_COMMAND,
  AGENT_NARRATE,
  AGENT_WORLD_MODEL,
  PLAYER_CHAT,
  TSY_EVENT,
  NPC_SPAWN,
  NPC_DEATH,
  FACTION_EVENT,
  ALCHEMY_SESSION_END,
  ALCHEMY_INSIGHT,
  BOTANY_ECOLOGY,
  AGING,
  LIFESPAN_EVENT,
  DUO_SHE_EVENT,
  BREAKTHROUGH_EVENT,
  CULTIVATION_DEATH,
  FORGE_EVENT,
  FORGE_START,
  FORGE_OUTCOME,
  SOCIAL_EXPOSURE,
  SOCIAL_PACT,
  SOCIAL_FEUD,
  SOCIAL_RENOWN_DELTA,
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
  POI_NOVICE_EVENT,
  SPIRIT_EYE_MIGRATE,
  SPIRIT_EYE_DISCOVERED,
  SPIRIT_EYE_USED_FOR_BREAKTHROUGH,
} = CHANNELS;

const DEFAULT_CHAT_DRAIN_WINDOW = 128;
const TSY_HOSTILE_EVENT_BUFFER_LIMIT = 128;
const NPC_EVENT_BUFFER_LIMIT = 128;
const ALCHEMY_EVENT_BUFFER_LIMIT = 128;
const POI_NOVICE_EVENT_BUFFER_LIMIT = 128;
const CROSS_SYSTEM_EVENT_BUFFER_LIMIT = 256;
const DRAIN_COUNTER_KEY = `${PLAYER_CHAT}:drain_counter`;
export const WORLD_MODEL_STATE_KEY = "bong:tiandao:state";
export const WORLD_MODEL_STATE_FIELDS = Object.freeze({
  currentEra: "current_era",
  zoneHistory: "zone_history",
  lastDecisions: "last_decisions",
  playerFirstSeenTick: "player_first_seen_tick",
  lastTick: "last_tick",
  lastStateTs: "last_state_ts",
});

export interface PublishAgentWorldModelRequest {
  source: NonNullable<AgentWorldModelEnvelopeV1["source"]>;
  snapshot: AgentWorldModelEnvelopeV1["snapshot"];
  metadata: {
    sourceTick: number;
    correlationId: string;
  };
}

export type TsyHostileEventV1 = TsyNpcSpawnedV1 | TsySentinelPhaseChangedV1;
export type NpcRuntimeEventV1 = NpcSpawnedV1 | NpcDeathV1 | FactionEventV1;
export type AlchemyRuntimeEventV1 = AlchemySessionEndV1 | AlchemyInsightV1;
export type PoiNoviceRuntimeEventV1 = PoiSpawnedEventV1 | TrespassEventV1;
export interface CrossSystemRuntimeEventV1 {
  channel: ChannelName;
  payload: unknown;
}

const CROSS_SYSTEM_EVENT_CHANNELS: readonly ChannelName[] = [
  BOTANY_ECOLOGY,
  AGING,
  LIFESPAN_EVENT,
  DUO_SHE_EVENT,
  BREAKTHROUGH_EVENT,
  CULTIVATION_DEATH,
  FORGE_EVENT,
  FORGE_START,
  FORGE_OUTCOME,
  SOCIAL_EXPOSURE,
  SOCIAL_PACT,
  SOCIAL_FEUD,
  SOCIAL_RENOWN_DELTA,
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

interface MultiExecResult<T = unknown> {
  0: Error | null;
  1: T;
}

interface RedisMultiLike {
  lrange(key: string, start: number, stop: number): RedisMultiLike;
  ltrim(key: string, start: number, stop: number): RedisMultiLike;
  exec(): Promise<Array<MultiExecResult<unknown>> | null>;
}

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

export interface RedisIpcConfig {
  url: string;
  createClient?: (url: string) => RedisIpcClient;
}

export interface RedisIpcDeps {
  createClient?: (url: string) => RedisIpcClient;
}

export class RedisIpc {
  private sub: RedisIpcClient;
  private pub: RedisIpcClient;
  private latestState: WorldStateV1 | null = null;
  private latestTsyHostileEvents: TsyHostileEventV1[] = [];
  private latestNpcEvents: NpcRuntimeEventV1[] = [];
  private latestAlchemyEvents: AlchemyRuntimeEventV1[] = [];
  private latestPoiNoviceEvents: PoiNoviceRuntimeEventV1[] = [];
  private latestCrossSystemEvents: CrossSystemRuntimeEventV1[] = [];
  private stateCallbacks: Array<(state: WorldStateV1) => void> = [];
  private tsyHostileCallbacks: Array<(event: TsyHostileEventV1) => void> = [];
  private npcEventCallbacks: Array<(event: NpcRuntimeEventV1) => void> = [];
  private alchemyEventCallbacks: Array<(event: AlchemyRuntimeEventV1) => void> = [];
  private poiNoviceEventCallbacks: Array<(event: PoiNoviceRuntimeEventV1) => void> = [];
  private crossSystemEventCallbacks: Array<(event: CrossSystemRuntimeEventV1) => void> = [];
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
      }
    } catch (e) {
      console.warn("[redis-ipc] failed to parse tsy_event:", e);
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

  constructor(config: RedisIpcConfig, deps?: RedisIpcDeps) {
    const createClient =
      config.createClient ??
      deps?.createClient ??
      ((url: string) => new IORedis(url) as unknown as RedisIpcClient);
    this.sub = createClient(config.url);
    this.pub = createClient(config.url);
  }

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
    for (const channel of CROSS_SYSTEM_EVENT_CHANNELS) {
      await this.sub.subscribe(channel);
    }
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.connected = true;
    console.log(
      `[redis-ipc] subscribed to ${[WORLD_STATE, TSY_EVENT, NPC_SPAWN, NPC_DEATH, FACTION_EVENT, ALCHEMY_SESSION_END, ALCHEMY_INSIGHT, POI_NOVICE_EVENT, ...CROSS_SYSTEM_EVENT_CHANNELS].join(", ")}`,
    );
  }

  getLatestState(): WorldStateV1 | null {
    return this.latestState;
  }

  onWorldState(cb: (state: WorldStateV1) => void): void {
    this.stateCallbacks.push(cb);
  }

  getLatestTsyHostileEvents(): TsyHostileEventV1[] {
    return [...this.latestTsyHostileEvents];
  }

  onTsyHostileEvent(cb: (event: TsyHostileEventV1) => void): void {
    this.tsyHostileCallbacks.push(cb);
  }

  getLatestNpcEvents(): NpcRuntimeEventV1[] {
    return [...this.latestNpcEvents];
  }

  onNpcRuntimeEvent(cb: (event: NpcRuntimeEventV1) => void): void {
    this.npcEventCallbacks.push(cb);
  }

  getLatestAlchemyEvents(): AlchemyRuntimeEventV1[] {
    return [...this.latestAlchemyEvents];
  }

  onAlchemyRuntimeEvent(cb: (event: AlchemyRuntimeEventV1) => void): void {
    this.alchemyEventCallbacks.push(cb);
  }

  getLatestPoiNoviceEvents(): PoiNoviceRuntimeEventV1[] {
    return [...this.latestPoiNoviceEvents];
  }

  onPoiNoviceEvent(cb: (event: PoiNoviceRuntimeEventV1) => void): void {
    this.poiNoviceEventCallbacks.push(cb);
  }

  getLatestCrossSystemEvents(): CrossSystemRuntimeEventV1[] {
    return [...this.latestCrossSystemEvents];
  }

  onCrossSystemEvent(cb: (event: CrossSystemRuntimeEventV1) => void): void {
    this.crossSystemEventCallbacks.push(cb);
  }

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

  async disconnect(): Promise<void> {
    this.connected = false;
    this.sub.off?.("message", this.onMessage);
    await this.sub.unsubscribe();
    this.sub.disconnect();
    this.pub.disconnect();
    console.log("[redis-ipc] disconnected");
  }

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

  async loadWorldModelState(options: { logger?: Pick<typeof console, "warn"> } = {}): Promise<AgentWorldModelEnvelopeV1["snapshot"] | null> {
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

  async drainPlayerChat(options: { maxItems?: number; logger?: Pick<typeof console, "warn"> } = {}): Promise<ChatMessageV1[]> {
    const maxItems = options.maxItems ?? DEFAULT_CHAT_DRAIN_WINDOW;
    const logger = options.logger ?? console;
    const raw = await this.drainListAtomically(PLAYER_CHAT, maxItems);
    if (raw.length === 0) {
      return [];
    }
    return parseChatMessages(raw, logger);
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
): AgentWorldModelEnvelopeV1["snapshot"] | null {
  const missingFields = Object.values(WORLD_MODEL_STATE_FIELDS).filter((field) => !(field in mirror));
  if (missingFields.length > 0) {
    logger.warn(`[redis-ipc] missing world model mirror fields: ${missingFields.join(", ")}`);
    return null;
  }

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
    lastTick === INVALID_MIRROR_FIELD ||
    lastStateTs === INVALID_MIRROR_FIELD
  ) {
    return null;
  }

  return {
    currentEra,
    zoneHistory,
    lastDecisions,
    playerFirstSeenTick,
    lastTick,
    lastStateTs,
  };
}

const INVALID_MIRROR_FIELD = Symbol("invalid-world-model-mirror-field");

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

function isCurrentEra(value: unknown): value is AgentWorldModelSnapshotV1["currentEra"] {
  if (value === null) {
    return true;
  }

  if (!isObjectRecord(value)) {
    return false;
  }

  return (
    typeof value.name === "string" &&
    typeof value.sinceTick === "number" &&
    Number.isFinite(value.sinceTick) &&
    typeof value.globalEffect === "string"
  );
}

function isZoneHistory(value: unknown): value is AgentWorldModelSnapshotV1["zoneHistory"] {
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

function isLastDecisions(value: unknown): value is AgentWorldModelSnapshotV1["lastDecisions"] {
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

function isPlayerFirstSeenTick(value: unknown): value is AgentWorldModelSnapshotV1["playerFirstSeenTick"] {
  if (!isObjectRecord(value)) {
    return false;
  }

  return Object.values(value).every((firstSeenTick) => {
    return typeof firstSeenTick === "number" && Number.isFinite(firstSeenTick);
  });
}

function isObjectRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
