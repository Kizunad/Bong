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
import { CHANNELS } from "@bong/schema";
import type { WorldStateV1, AgentCommandV1, NarrationV1, Command, Narration } from "@bong/schema";

const { WORLD_STATE, AGENT_COMMAND, AGENT_NARRATE } = CHANNELS;

export interface RedisIpcConfig {
  url: string;
}

export class RedisIpc {
  private sub: InstanceType<typeof IORedis>;
  private pub: InstanceType<typeof IORedis>;
  private latestState: WorldStateV1 | null = null;
  private stateCallbacks: Array<(state: WorldStateV1) => void> = [];

  constructor(config: RedisIpcConfig) {
    this.sub = new IORedis(config.url);
    this.pub = new IORedis(config.url);
  }

  async connect(): Promise<void> {
    await this.sub.subscribe(WORLD_STATE);
    console.log(`[redis-ipc] subscribed to ${WORLD_STATE}`);

    this.sub.on("message", (channel: string, message: string) => {
      if (channel === WORLD_STATE) {
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

    const msg: AgentCommandV1 = {
      v: 1,
      id: `cmd_${Date.now()}_${source}`,
      source: source as AgentCommandV1["source"],
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
    await this.sub.unsubscribe();
    this.sub.disconnect();
    this.pub.disconnect();
    console.log("[redis-ipc] disconnected");
  }
}
