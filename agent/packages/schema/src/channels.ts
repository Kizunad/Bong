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

  /** Server → Agent: 心魔劫预生成请求（plan-tribulation §2.4） (Pub/Sub) */
  HEART_DEMON_REQUEST: "bong:heart_demon_request",

  /** Agent → Server: 心魔劫预生成选项（plan-tribulation §2.4） (Pub/Sub) */
  HEART_DEMON_OFFER: "bong:heart_demon_offer",

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

  /** Server → Agent: 天劫统一事件流（plan-tribulation-v1 §6） */
  TRIBULATION: "bong:tribulation",
  TRIBULATION_OMEN: "bong:tribulation/omen",
  TRIBULATION_LOCK: "bong:tribulation/lock",
  TRIBULATION_WAVE: "bong:tribulation/wave",
  TRIBULATION_SETTLE: "bong:tribulation/settle",
  TRIBULATION_COLLAPSE: "bong:tribulation/collapse",

  /** Server → Agent: NPC 生成事件（plan-npc-ai-v1 §6） */
  NPC_SPAWN: "bong:npc/spawn",

  /** Server → Agent: NPC 死亡事件（plan-npc-ai-v1 §6） */
  NPC_DEATH: "bong:npc/death",

  /** Server → Agent: 派系状态变更事件（plan-npc-ai-v1 §6） */
  FACTION_EVENT: "bong:faction/event",

  /** Server → Agent: 玩家社交暴露事件（plan-social-v1 §7） */
  SOCIAL_EXPOSURE: "bong:social/exposure",

  /** Server → Agent: 玩家盟约建立 / 解除（plan-social-v1 §7） */
  SOCIAL_PACT: "bong:social/pact",

  /** Server → Agent: 玩家死仇建立（plan-social-v1 §7） */
  SOCIAL_FEUD: "bong:social/feud",

  /** Server → Agent: 玩家声名变动（plan-social-v1 §7） */
  SOCIAL_RENOWN_DELTA: "bong:social/renown_delta",

  /** Server → Agent: 战斗实时事件（Task 7）(Pub/Sub) */
  COMBAT_REALTIME: "bong:combat_realtime",

  /** Server → Agent: 战斗聚合摘要（Task 7，200 tick cadence）(Pub/Sub) */
  COMBAT_SUMMARY: "bong:combat_summary",

  /** Server → Agent: 反作弊阈值上报（plan-anticheat-v1） */
  ANTICHEAT: "bong:anticheat",

  /** Server → Agent: 护甲耐久变化（plan-armor-v1 §3）(Pub/Sub) */
  ARMOR_DURABILITY_CHANGED: "bong:armor/durability_changed",

  /** Server → Agent: 涡流反噬事件（plan-woliu-v1 §3.2.D） */
  WOLIU_BACKFIRE: "bong:woliu/backfire",

  /** Server → Agent: 涡流抽干投射物真元（plan-woliu-v1 §3.2.D） */
  WOLIU_PROJECTILE_DRAINED: "bong:woliu/projectile_drained",

  /** Server → Client/Agent: 涡流持涡 HUD 状态（plan-woliu-v1 §8） */
  WOLIU_VORTEX_STATE: "bong:woliu/vortex_state",

  /** Server → Agent: 伪灵脉活动快照（plan-terrain-pseudo-vein-v1 §6.1） */
  PSEUDO_VEIN_ACTIVE: "bong:pseudo_vein:active",

  /** Server → Agent: 伪灵脉消散事件（plan-terrain-pseudo-vein-v1 §6.1） */
  PSEUDO_VEIN_DISSIPATE: "bong:pseudo_vein:dissipate",

  /** Server → Agent: botany 采集进度观测（server-agent · 玩家维度） */
  BOTANY_HARVEST_PROGRESS: "bong:botany/harvest_progress",

  /** Server → Agent: botany 生态快照 (plan-botany-v1 §7 · 定时聚合 zone spirit_qi + 植物密度 + variant 分布) */
  BOTANY_ECOLOGY: "bong:botany/ecology",

  /** Server → Client/Agent: 灵木伐木进度（plan-spiritwood-v1 §3） */
  LUMBER_PROGRESS: "bong:lumber_progress",

  /** Server → Agent: skill XP 进账 (plan-skill-v1 §8) — 做中学 / 顿悟 / 突破 / 师承 四路来源 */
  SKILL_XP_GAIN: "bong:skill/xp_gain",

  /** Server → Agent: skill 升级事件 (plan-skill-v1 §8) — agent P5 据此生成冷漠古意 narration */
  SKILL_LV_UP: "bong:skill/lv_up",

  /** Server → Agent: skill cap 变更 (plan-skill-v1 §4) — 境界突破上调 / 跌落下修 */
  SKILL_CAP_CHANGED: "bong:skill/cap_changed",

  /** Server → Agent: 残卷使用结算 (plan-skill-v1 §3.2) — `was_duplicate=true` 时 `xp_granted=0` */
  SKILL_SCROLL_USED: "bong:skill/scroll_used",

  /** Server → Agent: 灵眼迁移观测（plan-spirit-eye-v1 §6） */
  SPIRIT_EYE_MIGRATE: "bong:spirit_eye/migrate",

  /** Server → Agent: 私有灵眼发现观测（plan-spirit-eye-v1 §4） */
  SPIRIT_EYE_DISCOVERED: "bong:spirit_eye/discovered",

  /** Server → Agent: 灵眼内固元突破观测（plan-spirit-eye-v1 §5） */
  SPIRIT_EYE_USED_FOR_BREAKTHROUGH: "bong:spirit_eye/used_for_breakthrough",

  /** Server → Agent: 玩家踏进 / 走出活坍缩渊 (plan-tsy-zone-followup-v1 §2.4)
   *
   * Entry / exit 共享同一频道，consumer 按 payload `kind` 字段（`tsy_enter` / `tsy_exit`）dispatch。 */
  TSY_EVENT: "bong:tsy_event",

  /** Server → Agent: 新手 POI 生成 / 屠村事件（plan-poi-novice-v1 §P2） */
  POI_NOVICE_EVENT: "bong:poi_novice/event",

  // ─── 炼器（武器）（plan-forge-v1 §4） ───────────────────
  /** Server → Agent: 锻造起炉（玩家起炉时推，供 agent 生成观察叙事） */
  FORGE_START: "bong:forge/start",
  /** Server → Agent: 锻造结果（结算推，供 agent 记录/叙事） */
  FORGE_OUTCOME: "bong:forge/outcome",

  // ─── 炼丹（plan-alchemy-client-v1 §6 / P4） ───────────────────
  /** Server → Agent: 炼丹起炉 */
  ALCHEMY_SESSION_START: "bong:alchemy/session_start",
  /** Server → Agent: 炼丹结算（含炸炉） */
  ALCHEMY_SESSION_END: "bong:alchemy/session_end",
  /** Server → Agent: 炼丹干预结果 */
  ALCHEMY_INTERVENTION_RESULT: "bong:alchemy/intervention_result",
} as const;

export const REDIS_V1_CHANNELS = [
  CHANNELS.WORLD_STATE,
  CHANNELS.PLAYER_CHAT,
  CHANNELS.AGENT_COMMAND,
  CHANNELS.AGENT_NARRATE,
  CHANNELS.AGENT_WORLD_MODEL,
  CHANNELS.INSIGHT_REQUEST,
  CHANNELS.INSIGHT_OFFER,
  CHANNELS.HEART_DEMON_REQUEST,
  CHANNELS.HEART_DEMON_OFFER,
  CHANNELS.BREAKTHROUGH_EVENT,
  CHANNELS.FORGE_EVENT,
  CHANNELS.CULTIVATION_DEATH,
  CHANNELS.DEATH,
  CHANNELS.REBIRTH,
  CHANNELS.DEATH_INSIGHT,
  CHANNELS.AGING,
  CHANNELS.LIFESPAN_EVENT,
  CHANNELS.DUO_SHE_EVENT,
  CHANNELS.TRIBULATION,
  CHANNELS.TRIBULATION_OMEN,
  CHANNELS.TRIBULATION_LOCK,
  CHANNELS.TRIBULATION_WAVE,
  CHANNELS.TRIBULATION_SETTLE,
  CHANNELS.TRIBULATION_COLLAPSE,
  CHANNELS.NPC_SPAWN,
  CHANNELS.NPC_DEATH,
  CHANNELS.FACTION_EVENT,
  CHANNELS.SOCIAL_EXPOSURE,
  CHANNELS.SOCIAL_PACT,
  CHANNELS.SOCIAL_FEUD,
  CHANNELS.SOCIAL_RENOWN_DELTA,
  CHANNELS.COMBAT_REALTIME,
  CHANNELS.COMBAT_SUMMARY,
  CHANNELS.ANTICHEAT,
  CHANNELS.ARMOR_DURABILITY_CHANGED,
  CHANNELS.WOLIU_BACKFIRE,
  CHANNELS.WOLIU_PROJECTILE_DRAINED,
  CHANNELS.WOLIU_VORTEX_STATE,
  CHANNELS.PSEUDO_VEIN_ACTIVE,
  CHANNELS.PSEUDO_VEIN_DISSIPATE,
  CHANNELS.BOTANY_HARVEST_PROGRESS,
  CHANNELS.BOTANY_ECOLOGY,
  CHANNELS.LUMBER_PROGRESS,
  CHANNELS.SKILL_XP_GAIN,
  CHANNELS.SKILL_LV_UP,
  CHANNELS.SKILL_CAP_CHANGED,
  CHANNELS.SKILL_SCROLL_USED,
  CHANNELS.SPIRIT_EYE_MIGRATE,
  CHANNELS.SPIRIT_EYE_DISCOVERED,
  CHANNELS.SPIRIT_EYE_USED_FOR_BREAKTHROUGH,
  CHANNELS.TSY_EVENT,
  CHANNELS.POI_NOVICE_EVENT,
  CHANNELS.FORGE_START,
  CHANNELS.FORGE_OUTCOME,
  CHANNELS.ALCHEMY_SESSION_START,
  CHANNELS.ALCHEMY_SESSION_END,
  CHANNELS.ALCHEMY_INTERVENTION_RESULT,
] as const;

export type ChannelName = (typeof CHANNELS)[keyof typeof CHANNELS];
