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

  /** Server → Agent: 死亡触发（plan-death-lifecycle-v1 §7） */
  DEATH: "bong:death",

  /** Server → Agent: 重生结算（plan-death-lifecycle-v1 §7） */
  REBIRTH: "bong:rebirth",

  /** Server → Agent: 遗念生成请求（plan-death-lifecycle-v1 §7） */
  DEATH_INSIGHT: "bong:death_insight",

  /** Server → Agent: 老化 / 风烛 / tick rate 变化（plan-death-lifecycle-v1 §7） */
  AGING: "bong:aging",

  /** Server → Agent: 寿元事件公开流水（plan-death-lifecycle-v1 §7） */
  LIFESPAN_EVENT: "bong:lifespan_event",

  /** Server → Agent: 夺舍公开流水（plan-death-lifecycle-v1 §7） */
  DUO_SHE_EVENT: "bong:duo_she_event",

  /** Server → Agent: 战斗实时事件（Task 7）(Pub/Sub) */
  COMBAT_REALTIME: "bong:combat_realtime",

  /** Server → Agent: 战斗聚合摘要（Task 7，200 tick cadence）(Pub/Sub) */
  COMBAT_SUMMARY: "bong:combat_summary",

  /** Server → Agent: 护甲耐久变化（plan-armor-v1 §3）(Pub/Sub) */
  ARMOR_DURABILITY_CHANGED: "bong:armor/durability_changed",

  /** Server → Agent: botany 采集进度观测（server-agent · 玩家维度） */
  BOTANY_HARVEST_PROGRESS: "bong:botany/harvest_progress",

  /** Server → Agent: botany 生态快照 (plan-botany-v1 §7 · 定时聚合 zone spirit_qi + 植物密度 + variant 分布) */
  BOTANY_ECOLOGY: "bong:botany/ecology",

  /** Server → Agent: skill XP 进账 (plan-skill-v1 §8) — 做中学 / 顿悟 / 突破 / 师承 四路来源 */
  SKILL_XP_GAIN: "bong:skill/xp_gain",

  /** Server → Agent: skill 升级事件 (plan-skill-v1 §8) — agent P5 据此生成冷漠古意 narration */
  SKILL_LV_UP: "bong:skill/lv_up",

  /** Server → Agent: skill cap 变更 (plan-skill-v1 §4) — 境界突破上调 / 跌落下修 */
  SKILL_CAP_CHANGED: "bong:skill/cap_changed",

  /** Server → Agent: 残卷使用结算 (plan-skill-v1 §3.2) — `was_duplicate=true` 时 `xp_granted=0` */
  SKILL_SCROLL_USED: "bong:skill/scroll_used",

  /** Server → Agent: 玩家踏进 / 走出活坍缩渊 (plan-tsy-zone-followup-v1 §2.4)
   *
   * Entry / exit 共享同一频道，consumer 按 payload `kind` 字段（`tsy_enter` / `tsy_exit`）dispatch。 */
  TSY_EVENT: "bong:tsy_event",
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
  CHANNELS.DEATH,
  CHANNELS.REBIRTH,
  CHANNELS.DEATH_INSIGHT,
  CHANNELS.AGING,
  CHANNELS.LIFESPAN_EVENT,
  CHANNELS.DUO_SHE_EVENT,
  CHANNELS.COMBAT_REALTIME,
  CHANNELS.COMBAT_SUMMARY,
  CHANNELS.ARMOR_DURABILITY_CHANGED,
  CHANNELS.BOTANY_HARVEST_PROGRESS,
  CHANNELS.BOTANY_ECOLOGY,
  CHANNELS.SKILL_XP_GAIN,
  CHANNELS.SKILL_LV_UP,
  CHANNELS.SKILL_CAP_CHANGED,
  CHANNELS.SKILL_SCROLL_USED,
  CHANNELS.TSY_EVENT,
] as const;

export type ChannelName = (typeof CHANNELS)[keyof typeof CHANNELS];
