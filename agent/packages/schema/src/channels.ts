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

  /** Agent → Server: WorldModel 快照权威上报（Task 5）(Pub/Sub) */
  AGENT_WORLD_MODEL: "bong:agent_world_model",

  /** Server → Agent: 顿悟请求（plan-cultivation §5.5） (Pub/Sub) */
  INSIGHT_REQUEST: "bong:insight_request",

  /** Agent → Server: 顿悟候选（plan-cultivation §5.5） (Pub/Sub) */
  INSIGHT_OFFER: "bong:insight_offer",

  /** Server → Agent: 突破事件（plan-cultivation §6.1） (Pub/Sub) */
  BREAKTHROUGH_EVENT: "bong:breakthrough_event",

  /** Server → Agent: 锻造事件（plan-cultivation §6.1） (Pub/Sub) */
  FORGE_EVENT: "bong:forge_event",

  /** Server → Agent: 修炼侧致死触发（plan-cultivation §4） (Pub/Sub) */
  CULTIVATION_DEATH: "bong:cultivation_death",

  /** Server → Agent: 战斗实时事件（Task 7）(Pub/Sub) */
  COMBAT_REALTIME: "bong:combat_realtime",

  /** Server → Agent: 战斗聚合摘要（Task 7，200 tick cadence）(Pub/Sub) */
  COMBAT_SUMMARY: "bong:combat_summary",
} as const;

export const REDIS_V1_CHANNELS = [
  CHANNELS.WORLD_STATE,
  CHANNELS.PLAYER_CHAT,
  CHANNELS.AGENT_COMMAND,
  CHANNELS.AGENT_NARRATE,
  CHANNELS.AGENT_WORLD_MODEL,
  CHANNELS.INSIGHT_REQUEST,
  CHANNELS.INSIGHT_OFFER,
  CHANNELS.BREAKTHROUGH_EVENT,
  CHANNELS.FORGE_EVENT,
  CHANNELS.CULTIVATION_DEATH,
  CHANNELS.COMBAT_REALTIME,
  CHANNELS.COMBAT_SUMMARY,
] as const;

export type ChannelName = (typeof CHANNELS)[keyof typeof CHANNELS];
