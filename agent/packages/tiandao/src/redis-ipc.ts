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

interface MultiExecResult<T = unknown> {
  0: Error | null;
  1: T;
}

interface RedisMultiLike {
  lrange(key: string, start: number, stop: number): RedisMultiLike;
  ltrim(key: string, start: number, stop: number): RedisMultiLike;
  exec(): Promise<Array<MultiExecResult<unknown>> | null>;
}

interface RedisClientLike {
  subscribe(channel: string): Promise<unknown>;
  on(event: "message", listener: (channel: string, message: string) => void): this;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
  publish(channel: string, message: string): Promise<number>;
  multi(): RedisMultiLike;
}

export interface RedisIpcDeps {
  createClient?: (url: string) => RedisClientLike;
}

export interface RedisIpcConfig {
  url: string;
}

export class RedisIpc {
  private sub: RedisClientLike;
  private pub: RedisClientLike;
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

  constructor(config: RedisIpcConfig, deps: RedisIpcDeps = {}) {
    const createClient = deps.createClient ?? ((url: string) => new IORedis(url));
    this.sub = createClient(config.url);
    this.pub = createClient(config.url);
  }

  async connect(): Promise<void> {
    if (this.connected) {
      return;
    }

    await this.sub.subscribe(WORLD_STATE);
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

  private async drainListAtomically(key: string, maxItems: number): Promise<string[]> {
    if (!Number.isFinite(maxItems) || maxItems <= 0) {
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
