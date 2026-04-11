import Redis from "ioredis";
const IORedis = Redis.default ?? Redis;
import { CHANNELS } from "@bong/schema";
import type {
  WorldStateV1,
  AgentCommandV1,
  NarrationV1,
  ChatMessageV1,
} from "@bong/schema";
import { parseChatMessages } from "./chat-processor.js";
import type { CommandPublishRequest, NarrationPublishRequest } from "./runtime.js";
import type { WorldModelSnapshot } from "./world-model.js";

const { WORLD_STATE, AGENT_COMMAND, AGENT_NARRATE, PLAYER_CHAT } = CHANNELS;

const DEFAULT_CHAT_DRAIN_WINDOW = 128;
const DRAIN_COUNTER_KEY = `${PLAYER_CHAT}:drain_counter`;
export const WORLD_MODEL_STATE_KEY = "bong:tiandao:state";
export const WORLD_MODEL_STATE_FIELDS = Object.freeze({
  currentEra: "current_era",
  zoneHistory: "zone_history",
  lastDecisions: "last_decisions",
  lastTick: "last_tick",
});

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
  private stateCallbacks: Array<(state: WorldStateV1) => void> = [];
  private connected = false;
  private readonly onMessage = (channel: string, message: string): void => {
    if (channel !== WORLD_STATE) {
      return;
    }

    try {
      const state = JSON.parse(message) as WorldStateV1;
      this.latestState = state;
      for (const cb of this.stateCallbacks) {
        cb(state);
      }
    } catch (e) {
      console.warn("[redis-ipc] failed to parse world_state:", e);
    }
  };

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
    this.sub.off?.("message", this.onMessage);
    this.sub.on("message", this.onMessage);
    this.connected = true;
    console.log(`[redis-ipc] subscribed to ${WORLD_STATE}`);
  }

  getLatestState(): WorldStateV1 | null {
    return this.latestState;
  }

  onWorldState(cb: (state: WorldStateV1) => void): void {
    this.stateCallbacks.push(cb);
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

  async loadWorldModelState(logger: Pick<typeof console, "warn"> = console): Promise<
    Partial<WorldModelSnapshot> | null
  > {
    if (!this.pub.hgetall) {
      return null;
    }

    const hash = await this.pub.hgetall(WORLD_MODEL_STATE_KEY);
    if (!hash || Object.keys(hash).length === 0) {
      return null;
    }

    const snapshot: Partial<WorldModelSnapshot> = {};
    const currentEra = parseJsonField(hash[WORLD_MODEL_STATE_FIELDS.currentEra], "current_era", logger);
    if (currentEra !== null) {
      snapshot.currentEra = currentEra as WorldModelSnapshot["currentEra"];
    }

    const zoneHistory = parseJsonField(hash[WORLD_MODEL_STATE_FIELDS.zoneHistory], "zone_history", logger);
    if (zoneHistory !== null) {
      snapshot.zoneHistory = zoneHistory as WorldModelSnapshot["zoneHistory"];
    }

    const lastDecisions = parseJsonField(hash[WORLD_MODEL_STATE_FIELDS.lastDecisions], "last_decisions", logger);
    if (lastDecisions !== null) {
      snapshot.lastDecisions = lastDecisions as WorldModelSnapshot["lastDecisions"];
    }

    const lastTick = parseTickField(
      hash[WORLD_MODEL_STATE_FIELDS.lastTick],
      WORLD_MODEL_STATE_FIELDS.lastTick,
      logger,
    );
    if (lastTick !== null) {
      snapshot.lastTick = lastTick;
    }

    return snapshot;
  }

  async saveWorldModelState(snapshot: WorldModelSnapshot): Promise<void> {
    if (!this.pub.hset) {
      return;
    }

    const values: Record<string, string> = {
      [WORLD_MODEL_STATE_FIELDS.currentEra]: JSON.stringify(snapshot.currentEra),
      [WORLD_MODEL_STATE_FIELDS.zoneHistory]: JSON.stringify(snapshot.zoneHistory),
      [WORLD_MODEL_STATE_FIELDS.lastDecisions]: JSON.stringify(snapshot.lastDecisions),
      [WORLD_MODEL_STATE_FIELDS.lastTick]: String(snapshot.lastTick ?? ""),
    };

    await this.pub.hset(WORLD_MODEL_STATE_KEY, values);
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

function parseJsonField(
  value: string | undefined,
  fieldName: string,
  logger: Pick<typeof console, "warn">,
): unknown | null {
  if (value === undefined) {
    logger.warn(`[redis-ipc] missing ${fieldName} in ${WORLD_MODEL_STATE_KEY}`);
    return null;
  }

  try {
    return JSON.parse(value);
  } catch (error) {
    logger.warn(`[redis-ipc] failed to parse ${fieldName} from ${WORLD_MODEL_STATE_KEY}:`, error);
    return null;
  }
}

function parseTickField(
  value: string | undefined,
  fieldName: string,
  logger: Pick<typeof console, "warn">,
): number | null {
  if (value === undefined) {
    logger.warn(`[redis-ipc] missing ${fieldName} in ${WORLD_MODEL_STATE_KEY}`);
    return null;
  }

  if (value.trim() === "") {
    return null;
  }

  const parsed = Number(value);
  if (!Number.isFinite(parsed)) {
    logger.warn(`[redis-ipc] failed to parse last_tick from ${WORLD_MODEL_STATE_KEY}:`, value);
    return null;
  }

  return parsed;
}
