/**
 * Redis channel names — single source of truth.
 * Rust 侧硬编码对应常量。
 */
export const CHANNELS = {
  /** Server → Agent: 世界状态快照 (Pub/Sub) */
  WORLD_STATE: "bong:world_state",

  /** Server → Agent: 玩家聊天消息 (Redis List, RPUSH/BLPOP) */
  PLAYER_CHAT: "bong:player_chat",

  /** Agent → Server: 天道指令 (Pub/Sub) */
  AGENT_COMMAND: "bong:agent_command",

  /** Agent → Server: 叙事文本，转发给客户端 (Pub/Sub) */
  AGENT_NARRATE: "bong:agent_narrate",
} as const;

export const REDIS_V1_CHANNELS = [
  CHANNELS.WORLD_STATE,
  CHANNELS.PLAYER_CHAT,
  CHANNELS.AGENT_COMMAND,
  CHANNELS.AGENT_NARRATE,
] as const;

export type ChannelName = (typeof CHANNELS)[keyof typeof CHANNELS];
