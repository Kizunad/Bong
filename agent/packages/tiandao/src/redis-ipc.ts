import Redis from "ioredis";
const IORedis = Redis.default ?? Redis;
import { CHANNELS } from "@bong/schema";
import type {
  WorldStateV1,
  AgentCommandV1,
  NarrationV1,
  Command,
  Narration,
  ChatMessageV1,
} from "@bong/schema";
import { parseChatMessages } from "./chat-processor.js";

const { WORLD_STATE, AGENT_COMMAND, AGENT_NARRATE, PLAYER_CHAT } = CHANNELS;

const DEFAULT_CHAT_DRAIN_WINDOW = 128;
const DRAIN_COUNTER_KEY = `${PLAYER_CHAT}:drain_counter`;

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

  async publishCommands(
    source: "arbiter",
    commands: Command[],
  ): Promise<void> {
    if (commands.length === 0) return;

    const msg: AgentCommandV1 = {
      v: 1,
      id: `cmd_${Date.now()}_${source}`,
      source,
      commands,
    };

    const json = JSON.stringify(msg);
    const subscribers = await this.pub.publish(AGENT_COMMAND, json);
    console.log(
      `[redis-ipc] published ${commands.length} commands to ${AGENT_COMMAND} (${subscribers} subscribers)`,
    );
  }

  async publishNarrations(narrations: Narration[]): Promise<void> {
    if (narrations.length === 0) return;

    const msg: NarrationV1 = {
      v: 1,
      narrations,
    };

    const json = JSON.stringify(msg);
    const subscribers = await this.pub.publish(AGENT_NARRATE, json);
    console.log(
      `[redis-ipc] published ${narrations.length} narrations to ${AGENT_NARRATE} (${subscribers} subscribers)`,
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
