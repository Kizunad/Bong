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

  /** Server → Agent: 天道狩猎专属叙事请求（plan-tiandao-hunt-v1 P3） */
  TIANDAO_HUNT_NARRATION_REQUEST: "bong:tiandao_hunt_narration_request",

  /** Agent → Server: WorldModel 快照权威上报（Task 5）(Pub/Sub) */
  AGENT_WORLD_MODEL: "bong:agent_world_model",

  /** Agent → Server: 天道灾劫选型 intent（plan-calamity-arsenal-v1） */
  CALAMITY_INTENT: "bong:calamity_intent",

  /** Server → Agent: 节律相位切换事件（plan-jiezeq-v1 P4） */
  SEASON_CHANGED: "bong:season_changed",

  /** Server → Agent: 骨币真元总供给月度快照（plan-economy-v1 P3） */
  BONE_COIN_TICK: "bong:bone_coin_tick",

  /** Server → Agent: 当前价格指数月度快照（plan-economy-v1 P2/P3） */
  PRICE_INDEX: "bong:price_index",

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

  /** Server → Agent: 突破 cinematic 阶段事件（plan-breakthrough-cinematic-v1） */
  BREAKTHROUGH_CINEMATIC: "bong:breakthrough_cinematic",

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

  /** Server → Agent/Client: 死亡电影化阶段编排（plan-death-rebirth-cinematic-v1） */
  DEATH_CINEMATIC: "bong:death_cinematic",

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

  /** Server → Agent: 化虚四类世界级 action 公告（plan-void-actions-v1） */
  VOID_ACTION_SUPPRESS_TSY: "bong:void_action/suppress_tsy",
  VOID_ACTION_EXPLODE_ZONE: "bong:void_action/explode_zone",
  VOID_ACTION_BARRIER: "bong:void_action/barrier",
  VOID_ACTION_LEGACY_ASSIGN: "bong:void_action/legacy_assign",

  /** Server → Agent: NPC 生成事件（plan-npc-ai-v1 §6） */
  NPC_SPAWN: "bong:npc/spawn",

  /** Server → Agent: NPC 死亡事件（plan-npc-ai-v1 §6） */
  NPC_DEATH: "bong:npc/death",

  /** Server → Agent: 派系状态变更事件（plan-npc-ai-v1 §6） */
  FACTION_EVENT: "bong:faction/event",

  /** Server → Agent: 散修群体消长盘面快照（plan-offscreen-war-v1 P5，reframe b） */
  FACTION_STATE: "bong:faction_state",

  /**
   * Server → Agent: 具名势力注册表快照（plan-faction-expansion-v1 P0）。
   *
   * 与 FACTION_STATE（bong:faction_state，emergent group census）完全独立——
   * 不同 key（bong:named_faction_state），不同 payload（NamedFactionStateV1）。
   * 防撞：NamedFactionStateV1 != FactionStateV1，key != bong:faction_state。
   * 下游：social-v2 WarReputation / faction-wars FactionWarEventV1 消费 NamedFactionId。
   * P1: faction-wars consumes NamedFactionId via faction_id_for_war
   */
  NAMED_FACTION_STATE: "bong:named_faction_state",

  /** Server → Agent: 涌现区域冲突生命周期事件（plan-offscreen-war-v1 P6/P9，reframe b） */
  FACTION_WAR: "bong:faction/war",

  /** Server → Agent: 玩家社交暴露事件（plan-social-v1 §7） */
  SOCIAL_EXPOSURE: "bong:social/exposure",

  /** Server → Agent: 玩家盟约建立 / 解除（plan-social-v1 §7） */
  SOCIAL_PACT: "bong:social/pact",

  /** Server → Agent: 玩家死仇建立（plan-social-v1 §7） */
  SOCIAL_FEUD: "bong:social/feud",

  /** Server → Agent: 玩家声名变动（plan-social-v1 §7） */
  SOCIAL_RENOWN_DELTA: "bong:social/renown_delta",

  /** Server → Agent/Client: 灵龛抄家入侵流水（plan-niche-defense-v1 P4） */
  SOCIAL_NICHE_INTRUSION: "bong:social/niche_intrusion",

  /** Server → Agent: 玩家 active identity 声名跨 100/500/1000 阈值 */
  HIGH_RENOWN_MILESTONE: "bong:high_renown_milestone",

  /** Server → Agent: 灵田 zone pressure 跨档事件（plan-lingtian-npc-v1 P5） */
  ZONE_PRESSURE_CROSSED: "bong:zone/pressure_crossed",

  /** Server → Agent / Client: 天气事件起 / 落（plan-lingtian-weather-v1 §3 / §4.4）
   *
   * payload 形态：`WeatherEventUpdateV1`（kind: started / expired / cleared）。
   * 单 zone MVP 用 zone_id="default"，未来扩展时按 zone 分发。 */
  WEATHER_EVENT_UPDATE: "bong:weather_event_update",

  /** Server → Client / Agent: zone-scoped 持续环境状态（plan-zone-environment-v1） */
  ZONE_ENVIRONMENT_UPDATE: "bong:zone_environment_update",

  /** Server → Agent: 噬元鼠局部相变事件（plan-rat-v1 P4） */
  RAT_PHASE_EVENT: "bong:rat_phase_event",

  /** Server → Agent: 战斗实时事件（Task 7）(Pub/Sub) */
  COMBAT_REALTIME: "bong:combat_realtime",

  /** Server → Agent: 战斗聚合摘要（Task 7，200 tick cadence）(Pub/Sub) */
  COMBAT_SUMMARY: "bong:combat_summary",

  /** Server → Agent: 混元 / 流派克制 PVP telemetry（plan-multi-style-v1 P3） */
  STYLE_BALANCE_TELEMETRY: "bong:style_balance_telemetry",

  /** Server → Agent: 反作弊阈值上报（plan-anticheat-v1） */
  ANTICHEAT: "bong:anticheat",

  /** Server → Agent: 护甲耐久变化（plan-armor-v1 §3）(Pub/Sub) */
  ARMOR_DURABILITY_CHANGED: "bong:armor/durability_changed",

  /** Server → Agent: 阵法 v2 deploy / decay / breakthrough / 欺天暴露事件 */
  ZHENFA_V2_EVENT: "bong:zhenfa/v2_event",

  /** Server → Agent: 涡流反噬事件（plan-woliu-v1 §3.2.D） */
  WOLIU_BACKFIRE: "bong:woliu/backfire",

  /** Server → Agent: 涡流抽干投射物真元（plan-woliu-v1 §3.2.D） */
  WOLIU_PROJECTILE_DRAINED: "bong:woliu/projectile_drained",

  /** Server → Client/Agent: 涡流持涡 HUD 状态（plan-woliu-v1 §8） */
  WOLIU_VORTEX_STATE: "bong:woliu/vortex_state",

  /** Server → Agent: 截脉 v2 五招叙事事件 */
  ZHENMAI_SKILL_EVENT: "bong:zhenmai/skill_event",

  /** Server → Agent: 爆脉 v3 五招叙事事件 */
  BAOMAI_V3_SKILL_EVENT: "bong:baomai_v3/skill_event",

  /** Server → Agent: 涡流 v2 五招 cast 流水（plan-woliu-v2 P3） */
  WOLIU_V2_CAST: "bong:woliu_v2/cast",

  /** Server → Agent: 涡流 v2 反噬分级流水（plan-woliu-v2 P3） */
  WOLIU_V2_BACKFIRE: "bong:woliu_v2/backfire",

  /** Server → Agent: 涡流 v2 紊流场生成/叙事流水（plan-woliu-v2 P3） */
  WOLIU_V2_TURBULENCE: "bong:woliu_v2/turbulence",

  /** Server → Agent: 毒蛊经脉侵蚀进度（plan-dugu-v1 P1 agent narration） */
  DUGU_POISON_PROGRESS: "bong:dugu/poison_progress",

  /** Server → Agent: 毒性真元服丹剂量事件（plan-poison-trait-v1） */
  POISON_DOSE_EVENT: "bong:poison/dose",

  /** Server → Agent: 毒性真元消化过载事件（plan-poison-trait-v1） */
  POISON_OVERDOSE_EVENT: "bong:poison/overdose",

  /** Server → Agent: 毒蛊 v2 五招 cast 流水（plan-dugu-v2 P3） */
  DUGU_V2_CAST: "bong:dugu_v2/cast",

  /** Server → Agent: 毒蛊 v2 自蕴进度与形貌异化（plan-dugu-v2 P3） */
  DUGU_V2_SELF_CURE: "bong:dugu_v2/self_cure",

  /** Server → Agent: 毒蛊 v2 倒蚀清算与绝壁劫预兆（plan-dugu-v2 P3） */
  DUGU_V2_REVERSE: "bong:dugu_v2/reverse",

  /** Server → Agent: 暗器载体封元完成（plan-anqi-v1 P2 narration） */
  ANQI_CARRIER_CHARGED: "bong:combat/carrier_charged",

  /** Server → Agent: 暗器载体命中注射（plan-anqi-v1 P2 narration） */
  ANQI_CARRIER_IMPACT: "bong:combat/carrier_impact",

  /** Server → Agent: 暗器投射物射空 / 蒸发（plan-anqi-v1 P2 narration） */
  ANQI_PROJECTILE_DESPAWNED: "bong:combat/projectile_despawned",

  /** Server → Agent: 暗器 v2 多发齐射事件 */
  ANQI_MULTI_SHOT: "bong:anqi/multi_shot",

  /** Server → Agent: 暗器 v2 凝魂 / 破甲 / 单射注射事件 */
  ANQI_QI_INJECTION: "bong:anqi/qi_injection",

  /** Server → Agent: 暗器 v2 化虚诱饵分形事件 */
  ANQI_ECHO_FRACTAL: "bong:anqi/echo_fractal",

  /** Server → Agent: 暗器 v2 容器磨损税事件 */
  ANQI_CARRIER_ABRASION: "bong:anqi/carrier_abrasion",

  /** Server → Agent: 暗器 v2 容器切换事件 */
  ANQI_CONTAINER_SWAP: "bong:anqi/container_swap",

  /** Server → Agent: 替尸 / 蜕壳流脱壳事件（plan-tuike-v1 §P1） */
  TUIKE_SHED: "bong:tuike/shed",

  /** Server → Client/Agent: 伪皮 HUD 状态（plan-tuike-v1 §P0） */
  TUIKE_FALSE_SKIN_STATE: "bong:tuike/false_skin_state",

  /** Server → Agent: 替尸 v2 三招叙事事件 */
  TUIKE_V2_SKILL_EVENT: "bong:tuike_v2/skill_event",

  /** Server → Agent: 蜕壳灰烬入包叙事事件（plan-combat-skill-feedback-bridges-v1 P6） */
  TUIKE_V2_ASH_DECAY: "bong:tuike_v2/ash_decay",

  /** Server → Agent: 医道治疗 / 业力 / 医患结契事件（plan-yidao-v1） */
  YIDAO_EVENT: "bong:yidao/event",

  /** Server → Agent: 伪灵脉活动快照（plan-terrain-pseudo-vein-v1 §6.1） */
  PSEUDO_VEIN_ACTIVE: "bong:pseudo_vein:active",

  /** Server → Agent: 伪灵脉消散事件（plan-terrain-pseudo-vein-v1 §6.1） */
  PSEUDO_VEIN_DISSIPATE: "bong:pseudo_vein:dissipate",

  /** Server → Agent: 九宗故地阵核激活事件（plan-terrain-jiuzong-ruin-v1 §7） */
  ZONG_CORE_ACTIVATED: "bong:zong_core_activated",

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

  /** Server → Agent/Client: 战斗功法残卷研读事件 */
  TECHNIQUE_SCROLL_READ: "bong:technique/scroll_read",

  /** Server → Agent/Client: 战斗功法习得事件 */
  TECHNIQUE_LEARNED: "bong:technique/learned",

  /** Server → Agent/Client: 战斗功法练满事件 */
  TECHNIQUE_MASTERED: "bong:technique/mastered",

  /** Server → Client: 战斗功法熟练度提升事件 */
  TECHNIQUE_PROFICIENCY_UP: "bong:technique/proficiency_up",

  /** Server → Agent: 灵眼迁移观测（plan-spirit-eye-v1 §6） */
  SPIRIT_EYE_MIGRATE: "bong:spirit_eye/migrate",

  /** Server → Agent: 私有灵眼发现观测（plan-spirit-eye-v1 §4） */
  SPIRIT_EYE_DISCOVERED: "bong:spirit_eye/discovered",

  /** Server → Agent: 灵眼内固元突破观测（plan-spirit-eye-v1 §5） */
  SPIRIT_EYE_USED_FOR_BREAKTHROUGH: "bong:spirit_eye/used_for_breakthrough",

  /** Server → Agent: 灵宝器灵对话请求（plan-spirit-treasure-v1 P1） */
  SPIRIT_TREASURE_DIALOGUE_REQUEST: "bong:spirit_treasure_dialogue_request",

  /** Agent → Server: 灵宝器灵对话结果（plan-spirit-treasure-v1 P1） */
  SPIRIT_TREASURE_DIALOGUE: "bong:spirit_treasure_dialogue",

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
  /** Server → Agent: 丹心识别高精度线索 */
  ALCHEMY_INSIGHT: "bong:alchemy_insight",

  // ─── 身份与信誉（plan-identity-v1 §7） ────────────────────────
  /** Server → Agent: 玩家 active identity 反应分级跌入 Wanted (<-75) 后 emit */
  WANTED_PLAYER: "bong:wanted_player",

  // ─── 经脉永久 SEVERED（plan-meridian-severed-v1 §1 P3） ────────────────────────
  /** Server → Agent: 经脉永久 SEVERED 事件流（7 类来源 emit 同一通道） */
  MERIDIAN_SEVERED: "bong:meridian_severed",

  // ─── 丹道变异（plan-dandao-path-v1 §6.4） ────────────────────────────
  /** Server → Agent: 变异阶段推进事件（阶段 3+ 触发天道 narration） */
  MUTATION_EVENT: "bong:mutation_event",

  // ─── 通用手搓（plan-craft-v1 P3） ────────────────────────────
  /** Server → Agent: 出炉结果（成功 / 失败），narration 出炉叙事的 trigger */
  CRAFT_OUTCOME: "bong:craft/outcome",
  /** Server → Agent: 三渠道解锁广播（残卷=首学 / 师承 / 顿悟），narration 三类叙事的 trigger */
  CRAFT_RECIPE_UNLOCKED: "bong:craft/recipe_unlocked",

  // ─── 爆脉 v3 残余事件（plan-combat-skill-feedback-bridges-v1 P2） ──────────
  /** Server → Agent: 山震 AoE 震波事件（baomai_v3 mountain_shake，agent narration） */
  BAOMAI_V3_MOUNTAIN_SHAKE: "bong:baomai_v3/mountain_shake",
  /** Server → Agent: 血燃 HP→真元事件（baomai_v3 blood_burn，agent narration） */
  BAOMAI_V3_BLOOD_BURN: "bong:baomai_v3/blood_burn",
  /** Server → Agent: 超越到期事件（baomai_v3 transcendence expired，agent narration） */
  BAOMAI_V3_TRANSCENDENCE_EXPIRED: "bong:baomai_v3/transcendence_expired",
  /** Server → Agent: 过载涟漪事件（baomai_v3 overload ripple，agent narration） */
  BAOMAI_V3_OVERLOAD_RIPPLE: "bong:baomai_v3/overload_ripple",

  // ─── 天道 UI-as-Data（plan-agent-ui-data-v1 P0） ─────────────────────────────
  /** Agent → Server: 天道 UI 面板指令（含 realm_gate / allowed_button_ids，Pub/Sub） */
  AGENT_UI_CMD: "bong:agent_ui_cmd",

  /** Server → Agent: 天道 UI 面板响应（button_click / dismissed / timeout / replaced / error / parse_error，Pub/Sub） */
  AGENT_UI_RESPONSE: "bong:agent_ui_response",

  // ─── 垂死大能遭遇（plan-dying-elder-v1 P3） ─────────────────────────────────
  /** Server → Agent: 垂死大能遭遇事件（appeared / dan_received / betrayal / dead_*，agent narration） */
  ELDER_ENCOUNTER: "bong:elder_encounter",

  // ─── 我流虚蚀（plan-combat-skill-feedback-bridges-v1 P3） ────────────────────
  /** Server → Agent: 虚蚀阶段推进事件（我流虚蚀整链激活，agent narration） */
  VOID_EROSION_EVENT: "bong:void_erosion_event",

  // ─── 领地霸主叙事（plan-territory-v1 P3） ────────────────────────────────
  /** Server → Agent: 领地霸主叙事请求（Established / Ousted / QiDepleted 三时刻） */
  TERRITORY_NARRATION_REQUEST: "bong:territory_narration_request",

  // ─── 爆脉 v4（plan-combat-skill-feedback-bridges-v1 P1） ────────────────────
  /** Server → Agent: 疤纹回路形成（baomai_v4 scar_circuit formed，agent narration） */
  BAOMAI_V4_SCAR_CIRCUIT_FORMED: "bong:baomai_v4/scar_circuit_formed",
  /** Server → Agent: 疤纹回路断裂（baomai_v4 scar_circuit broken，agent narration） */
  BAOMAI_V4_SCAR_CIRCUIT_BROKEN: "bong:baomai_v4/scar_circuit_broken",
  /** Server → Agent: 活茧阶段提升（baomai_v4 iron_cocoon stage up，agent narration） */
  BAOMAI_V4_IRON_COCOON_STAGE_UP: "bong:baomai_v4/iron_cocoon_stage_up",
  /** Server → Agent: 共振锁定开始（baomai_v4 resonance lock started，agent narration） */
  BAOMAI_V4_RESONANCE_LOCK: "bong:baomai_v4/resonance_lock",
  /** Server → Agent: 共振锁定结束（baomai_v4 resonance lock ended，agent narration） */
  BAOMAI_V4_RESONANCE_LOCK_END: "bong:baomai_v4/resonance_lock_end",
} as const;

export const REDIS_V1_CHANNELS = [
  CHANNELS.WORLD_STATE,
  CHANNELS.PLAYER_CHAT,
  CHANNELS.AGENT_COMMAND,
  CHANNELS.AGENT_NARRATE,
  CHANNELS.TIANDAO_HUNT_NARRATION_REQUEST,
  CHANNELS.AGENT_WORLD_MODEL,
  CHANNELS.CALAMITY_INTENT,
  CHANNELS.SEASON_CHANGED,
  CHANNELS.BONE_COIN_TICK,
  CHANNELS.PRICE_INDEX,
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
  CHANNELS.DEATH_CINEMATIC,
  CHANNELS.AGING,
  CHANNELS.LIFESPAN_EVENT,
  CHANNELS.DUO_SHE_EVENT,
  CHANNELS.TRIBULATION,
  CHANNELS.TRIBULATION_OMEN,
  CHANNELS.TRIBULATION_LOCK,
  CHANNELS.TRIBULATION_WAVE,
  CHANNELS.TRIBULATION_SETTLE,
  CHANNELS.TRIBULATION_COLLAPSE,
  CHANNELS.VOID_ACTION_SUPPRESS_TSY,
  CHANNELS.VOID_ACTION_EXPLODE_ZONE,
  CHANNELS.VOID_ACTION_BARRIER,
  CHANNELS.VOID_ACTION_LEGACY_ASSIGN,
  CHANNELS.NPC_SPAWN,
  CHANNELS.NPC_DEATH,
  CHANNELS.FACTION_EVENT,
  CHANNELS.FACTION_STATE,
  CHANNELS.FACTION_WAR,
  CHANNELS.SOCIAL_EXPOSURE,
  CHANNELS.SOCIAL_PACT,
  CHANNELS.SOCIAL_FEUD,
  CHANNELS.SOCIAL_RENOWN_DELTA,
  CHANNELS.SOCIAL_NICHE_INTRUSION,
  CHANNELS.HIGH_RENOWN_MILESTONE,
  CHANNELS.ZONE_PRESSURE_CROSSED,
  CHANNELS.WEATHER_EVENT_UPDATE,
  CHANNELS.ZONE_ENVIRONMENT_UPDATE,
  CHANNELS.RAT_PHASE_EVENT,
  CHANNELS.COMBAT_REALTIME,
  CHANNELS.COMBAT_SUMMARY,
  CHANNELS.STYLE_BALANCE_TELEMETRY,
  CHANNELS.ANTICHEAT,
  CHANNELS.ARMOR_DURABILITY_CHANGED,
  CHANNELS.WOLIU_BACKFIRE,
  CHANNELS.WOLIU_PROJECTILE_DRAINED,
  CHANNELS.WOLIU_VORTEX_STATE,
  CHANNELS.WOLIU_V2_CAST,
  CHANNELS.WOLIU_V2_BACKFIRE,
  CHANNELS.WOLIU_V2_TURBULENCE,
  CHANNELS.ZHENFA_V2_EVENT,
  CHANNELS.TUIKE_V2_SKILL_EVENT,
  CHANNELS.YIDAO_EVENT,
  CHANNELS.PSEUDO_VEIN_ACTIVE,
  CHANNELS.PSEUDO_VEIN_DISSIPATE,
  CHANNELS.ZONG_CORE_ACTIVATED,
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
  CHANNELS.SPIRIT_TREASURE_DIALOGUE_REQUEST,
  CHANNELS.SPIRIT_TREASURE_DIALOGUE,
  CHANNELS.TSY_EVENT,
  CHANNELS.POI_NOVICE_EVENT,
  CHANNELS.FORGE_START,
  CHANNELS.FORGE_OUTCOME,
  CHANNELS.ALCHEMY_SESSION_START,
  CHANNELS.ALCHEMY_SESSION_END,
  CHANNELS.ALCHEMY_INTERVENTION_RESULT,
  CHANNELS.ALCHEMY_INSIGHT,
  CHANNELS.WANTED_PLAYER,
  CHANNELS.MERIDIAN_SEVERED,
  CHANNELS.POISON_DOSE_EVENT,
  CHANNELS.POISON_OVERDOSE_EVENT,
  CHANNELS.ZHENMAI_SKILL_EVENT,
  CHANNELS.BAOMAI_V3_SKILL_EVENT,
  CHANNELS.CRAFT_OUTCOME,
  CHANNELS.CRAFT_RECIPE_UNLOCKED,
  CHANNELS.MUTATION_EVENT,
  // plan-dying-elder-v1 P3 — 垂死大能遭遇叙事通道
  CHANNELS.ELDER_ENCOUNTER,
  // plan-combat-skill-feedback-bridges-v1 P3 — 我流虚蚀阶段推进叙事通道
  CHANNELS.VOID_EROSION_EVENT,
  // plan-combat-skill-feedback-bridges-v1 P6 — 蜕壳灰烬入包叙事通道
  CHANNELS.TUIKE_V2_ASH_DECAY,
  // plan-combat-skill-feedback-bridges-v1 P1 — 爆脉 v4 叙事通道
  CHANNELS.BAOMAI_V4_SCAR_CIRCUIT_FORMED,
  CHANNELS.BAOMAI_V4_SCAR_CIRCUIT_BROKEN,
  CHANNELS.BAOMAI_V4_IRON_COCOON_STAGE_UP,
  CHANNELS.BAOMAI_V4_RESONANCE_LOCK,
  CHANNELS.BAOMAI_V4_RESONANCE_LOCK_END,
  // plan-combat-skill-feedback-bridges-v1 P2 — 爆脉 v3 残余事件通道
  CHANNELS.BAOMAI_V3_MOUNTAIN_SHAKE,
  CHANNELS.BAOMAI_V3_BLOOD_BURN,
  CHANNELS.BAOMAI_V3_TRANSCENDENCE_EXPIRED,
  CHANNELS.BAOMAI_V3_OVERLOAD_RIPPLE,
  // plan-territory-v1 P3 — 领地霸主叙事通道
  CHANNELS.TERRITORY_NARRATION_REQUEST,
  // plan-faction-expansion-v1 P0 — 具名势力注册表快照通道
  CHANNELS.NAMED_FACTION_STATE,
  // plan-agent-ui-data-v1 P0 — 天道 UI 面板 IPC 通道
  CHANNELS.AGENT_UI_CMD,
  CHANNELS.AGENT_UI_RESPONSE,
] as const;

export type ChannelName = (typeof CHANNELS)[keyof typeof CHANNELS];
