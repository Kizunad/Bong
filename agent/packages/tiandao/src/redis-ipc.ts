/**
 * Redis IPC — connects 天道 Agent to the Valence server
 *
 * Channels:
 *   bong:world_state  (subscribe) — server publishes world snapshots
 *   bong:agent_command (publish)  — agent sends commands to server
 *   bong:agent_narrate (publish)  — agent sends narrations to server
 */

import Redis from "ioredis";
const IORedis = Redis.default ?? Redis;
import {
  AgentCommandV1 as AgentCommandV1Schema,
  CHANNELS,
  NarrationV1 as NarrationV1Schema,
  WorldStateV1 as WorldStateV1Schema,
  validate,
} from "@bong/schema";
import type { WorldStateV1, AgentCommandV1, NarrationV1, Command, Narration } from "@bong/schema";

const { WORLD_STATE, PLAYER_CHAT, AGENT_COMMAND, AGENT_NARRATE } = CHANNELS;

const DRAIN_PLAYER_CHAT_ATOMIC_LUA = `
local chatKey = KEYS[1]
local counterKey = KEYS[2]

if redis.call("EXISTS", chatKey) == 0 then
  return {}
end

local suffix = redis.call("INCR", counterKey)
local drainingKey = chatKey .. ":drain:" .. suffix

redis.call("RENAME", chatKey, drainingKey)
local drained = redis.call("LRANGE", drainingKey, 0, -1)
redis.call("DEL", drainingKey)

return drained
`;

const PLAYER_CHAT_DRAIN_COUNTER_KEY = `${PLAYER_CHAT}:drain_counter`;

type RedisMessageListener = (channel: string, message: string) => void;
type RedisLifecycleEvent = "close" | "reconnecting" | "ready" | "error";

function formatError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}

function stripPrivateSource(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map((item) => stripPrivateSource(item));
  }

  if (value && typeof value === "object") {
    const output: Record<string, unknown> = {};
    for (const [key, child] of Object.entries(value as Record<string, unknown>)) {
      if (key === "_source") continue;
      output[key] = stripPrivateSource(child);
    }
    return output;
  }

  return value;
}

export interface RedisIpcClient {
  subscribe(channel: string): Promise<number>;
  on(event: "message", listener: RedisMessageListener): unknown;
  on(event: RedisLifecycleEvent, listener: (...args: unknown[]) => void): unknown;
  publish(channel: string, message: string): Promise<number>;
  eval(script: string, numKeys: number, ...args: string[]): Promise<unknown>;
  unsubscribe(...channels: string[]): Promise<unknown>;
  disconnect(): void;
}

function createDefaultClient(url: string): RedisIpcClient {
  return new IORedis(url) as unknown as RedisIpcClient;
}

export interface RedisIpcConfig {
  url: string;
  createClient?: (url: string) => RedisIpcClient;
}

export class RedisIpc {
  private sub: RedisIpcClient;
  private pub: RedisIpcClient;
  private latestState: WorldStateV1 | null = null;
  private stateCallbacks: Array<(state: WorldStateV1) => void> = [];

  constructor(config: RedisIpcConfig) {
    const createClient = config.createClient ?? createDefaultClient;
    this.sub = createClient(config.url);
    this.pub = createClient(config.url);
  }

  private registerLifecycleLogs(client: RedisIpcClient, role: "sub" | "pub"): void {
    client.on("close", () => {
      console.warn("[redis-ipc] redis connection closed", {
        client: role,
      });
    });

    client.on("reconnecting", (delay: unknown) => {
      console.warn("[redis-ipc] redis reconnecting", {
        client: role,
        delay_ms: typeof delay === "number" ? delay : undefined,
      });
    });

    client.on("ready", () => {
      console.log("[redis-ipc] redis connection ready", {
        client: role,
      });
    });

    client.on("error", (error: unknown) => {
      console.warn("[redis-ipc] redis connection error", {
        client: role,
        error: formatError(error),
      });
    });
  }

  async connect(): Promise<void> {
    this.registerLifecycleLogs(this.sub, "sub");
    this.registerLifecycleLogs(this.pub, "pub");

    await this.sub.subscribe(WORLD_STATE);
    console.log(`[redis-ipc] subscribed to ${WORLD_STATE}`);

    this.sub.on("message", (channel: string, message: string) => {
      if (channel === WORLD_STATE) {
        let parsed: unknown;
        try {
          parsed = JSON.parse(message);
        } catch (error) {
          console.warn("[redis-ipc] drop invalid world_state payload", {
            channel,
            reason: "invalid_json",
            error: formatError(error),
            payload_size: message.length,
          });
          return;
        }

        const validation = validate(WorldStateV1Schema, parsed);
        if (!validation.ok) {
          console.warn("[redis-ipc] drop invalid world_state payload", {
            channel,
            reason: "schema_invalid",
            errors: validation.errors,
            payload_size: message.length,
          });
          return;
        }

        const state = parsed as WorldStateV1;
        this.latestState = state;
        for (const cb of this.stateCallbacks) {
          cb(state);
        }
      }
    });
  }

  getLatestState(): WorldStateV1 | null {
    return this.latestState;
  }

  onWorldState(cb: (state: WorldStateV1) => void): void {
    this.stateCallbacks.push(cb);
  }

  async publishCommands(
    source: string,
    commands: Command[],
  ): Promise<void> {
    if (commands.length === 0) return;

    void source;

    const publicCommands: Command[] = commands.map((command) => {
      const publicParams =
        command.params && typeof command.params === "object"
          ? (stripPrivateSource(command.params) as Record<string, unknown>)
          : {};
      return {
        type: command.type,
        target: command.target,
        params: {
          ...publicParams,
        },
      };
    });

    const msg: AgentCommandV1 = {
      v: 1,
      id: `cmd_${Date.now()}`,
      commands: publicCommands,
    };

    const validation = validate(AgentCommandV1Schema, msg);
    if (!validation.ok) {
      console.warn("[redis-ipc] skip invalid agent_command payload", {
        reason: "schema_invalid",
        command_count: publicCommands.length,
        errors: validation.errors,
      });
      return;
    }

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

    const validation = validate(NarrationV1Schema, msg);
    if (!validation.ok) {
      console.warn("[redis-ipc] skip invalid narration payload", {
        reason: "schema_invalid",
        narration_count: narrations.length,
        errors: validation.errors,
      });
      return;
    }

    const json = JSON.stringify(msg);
    const subscribers = await this.pub.publish(AGENT_NARRATE, json);
    console.log(
      `[redis-ipc] published ${narrations.length} narrations to ${AGENT_NARRATE} (${subscribers} subscribers)`,
    );
  }

  async drainPlayerChatRaw(): Promise<string[]> {
    const drained = await this.pub.eval(
      DRAIN_PLAYER_CHAT_ATOMIC_LUA,
      2,
      PLAYER_CHAT,
      PLAYER_CHAT_DRAIN_COUNTER_KEY,
    );

    if (!Array.isArray(drained)) {
      console.warn("[redis-ipc] player_chat drain returned unexpected value", {
        valueType: typeof drained,
      });
      return [];
    }

    const rawEntries: string[] = [];
    for (const item of drained) {
      if (typeof item === "string") {
        rawEntries.push(item);
        continue;
      }

      console.warn("[redis-ipc] player_chat drain dropped non-string item", {
        valueType: typeof item,
      });
    }

    return rawEntries;
  }

  async disconnect(): Promise<void> {
    await this.sub.unsubscribe();
    this.sub.disconnect();
    this.pub.disconnect();
    console.log("[redis-ipc] disconnected");
  }
}
