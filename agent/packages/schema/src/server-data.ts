import { Type, type Static } from "@sinclair/typebox";

import {
  AlchemyContaminationLevelV1,
  AlchemyOutcomeBucket,
  AlchemyRecipeEntryV1,
  AlchemyStageHintV1,
  BlockPosV1,
} from "./alchemy.js";
import { BotanyHarvestModeV1 } from "./botany.js";
import {
  SkillBarConfigV1,
  TechniquesSnapshotV1,
} from "./combat-hud.js";
import { CarrierStateV1 } from "./combat-carrier.js";
import { DuguPoisonStateV1 } from "./dugu.js";
import { EventKind, MAX_PAYLOAD_BYTES } from "./common.js";
import { ColorKind, InsightCategory, SkillMilestoneSnapshotV1 } from "./cultivation.js";
import {
  InventoryEventDroppedV1,
  InventoryEventDurabilityChangedV1,
  InventoryEventMovedV1,
  InventoryEventStackChangedV1,
  InventoryItemViewV1,
  InventorySnapshotV1,
} from "./inventory.js";
import { Narration } from "./narration.js";
import { RealmVisionParamsV1 } from "./realm-vision.js";
import {
  ExtractAbortedV1,
  ExtractCompletedV1,
  ExtractFailedV1,
  ExtractProgressV1,
  ExtractStartedV1,
  RiftPortalRemovedV1,
  RiftPortalStateV1,
  TsyCollapseStartedIpcV1,
} from "./extract-v1.js";
import { VortexFieldStateV1 } from "./woliu.js";
import {
  ContainerStateV1,
  SearchAbortedV1,
  SearchCompletedV1,
  SearchProgressV1,
  SearchStartedV1,
} from "./container-interaction.js";
import {
  ForgeBlueprintBookDataV1,
  ForgeOutcomeDataV1,
  ForgeSessionDataV1,
  WeaponForgeStationDataV1,
} from "./forge.js";
import {
  SkillCapChangedPayloadV1,
  SkillLvUpPayloadV1,
  SkillScrollUsedPayloadV1,
  SkillSnapshotPayloadV1,
  SkillXpGainPayloadV1,
} from "./skill.js";
import { PlayerPowerBreakdown, SeasonStateV1, Vec3, ZoneStatusV1 } from "./world-state.js";
import {
  SocialAnonymityPayloadV1,
  SocialExposureEventV1,
  SocialFeudEventV1,
  NicheGuardianBrokenV1,
  NicheGuardianFatigueV1,
  NicheIntrusionEventV1,
  SocialPactEventV1,
  PlayerSocialSnapshotV1,
  SocialRenownDeltaV1,
  SparringInvitePayloadV1,
  TradeOfferPayloadV1,
} from "./social.js";
import { SpiritualSenseTargetsV1 } from "./spiritual-sense.js";
import { FalseSkinStateV1 } from "./tuike.js";
import {
  ServerDataFreshnessUpdateV1,
  ServerDataProcessingSessionV1,
} from "./processing.js";

const MERIDIAN_CHANNEL_COUNT = 20;

const CultivationOpenedArrayV1 = Type.Array(Type.Boolean(), {
  minItems: MERIDIAN_CHANNEL_COUNT,
  maxItems: MERIDIAN_CHANNEL_COUNT,
});

const CultivationFlowArrayV1 = Type.Array(Type.Number({ minimum: 0 }), {
  minItems: MERIDIAN_CHANNEL_COUNT,
  maxItems: MERIDIAN_CHANNEL_COUNT,
});

const CultivationIntegrityArrayV1 = Type.Array(
  Type.Number({ minimum: 0, maximum: 1 }),
  {
    minItems: MERIDIAN_CHANNEL_COUNT,
    maxItems: MERIDIAN_CHANNEL_COUNT,
  },
);

const CultivationProgressArrayV1 = Type.Array(
  Type.Number({ minimum: 0, maximum: 1 }),
  {
    minItems: MERIDIAN_CHANNEL_COUNT,
    maxItems: MERIDIAN_CHANNEL_COUNT,
  },
);

const CultivationCracksArrayV1 = Type.Array(
  Type.Integer({ minimum: 0, maximum: 255 }),
  {
    minItems: MERIDIAN_CHANNEL_COUNT,
    maxItems: MERIDIAN_CHANNEL_COUNT,
  },
);

const LifespanPreviewV1 = Type.Object(
  {
    years_lived: Type.Number({ minimum: 0 }),
    cap_by_realm: Type.Integer({ minimum: 1 }),
    remaining_years: Type.Number({ minimum: 0 }),
    death_penalty_years: Type.Integer({ minimum: 0 }),
    tick_rate_multiplier: Type.Number({ minimum: 0 }),
    is_wind_candle: Type.Boolean(),
  },
  { additionalProperties: false },
);

const DeathScreenStageV1 = Type.Union([
  Type.Literal("fortune"),
  Type.Literal("tribulation"),
]);

const DeathScreenZoneKindV1 = Type.Union([
  Type.Literal("ordinary"),
  Type.Literal("death"),
  Type.Literal("negative"),
]);

export const ServerDataType = Type.Union([
  Type.Literal("welcome"),
  Type.Literal("heartbeat"),
  Type.Literal("narration"),
  Type.Literal("zone_info"),
  Type.Literal("event_alert"),
  Type.Literal("player_state"),
  Type.Literal("ui_open"),
  Type.Literal("cultivation_detail"),
  Type.Literal("qi_color_observed"),
  Type.Literal("inventory_event"),
  Type.Literal("inventory_snapshot"),
  Type.Literal("dropped_loot_sync"),
  Type.Literal("botany_harvest_progress"),
  Type.Literal("botany_plant_v2_render_profiles"),
  Type.Literal("lumber_progress"),
  Type.Literal("botany_skill"),
  Type.Literal("processing_session"),
  Type.Literal("freshness_update"),
  Type.Literal("alchemy_furnace"),
  Type.Literal("alchemy_session"),
  Type.Literal("alchemy_outcome_forecast"),
  Type.Literal("alchemy_outcome_resolved"),
  Type.Literal("alchemy_recipe_book"),
  Type.Literal("alchemy_contamination"),
  Type.Literal("death_screen"),
  Type.Literal("terminate_screen"),
  Type.Literal("skill_xp_gain"),
  Type.Literal("skill_lv_up"),
  Type.Literal("skill_cap_changed"),
  Type.Literal("skill_scroll_used"),
  Type.Literal("skill_snapshot"),
  Type.Literal("burst_meridian_event"),
  Type.Literal("skillbar_config"),
  Type.Literal("techniques_snapshot"),
  Type.Literal("weapon_equipped"),
  Type.Literal("weapon_broken"),
  Type.Literal("treasure_equipped"),
  Type.Literal("dugu_poison_state"),
  Type.Literal("carrier_state"),
  Type.Literal("rift_portal_state"),
  Type.Literal("rift_portal_removed"),
  Type.Literal("extract_started"),
  Type.Literal("extract_progress"),
  Type.Literal("extract_completed"),
  Type.Literal("extract_aborted"),
  Type.Literal("extract_failed"),
  Type.Literal("tsy_collapse_started_ipc"),
  Type.Literal("container_state"),
  Type.Literal("search_started"),
  Type.Literal("search_progress"),
  Type.Literal("search_completed"),
  Type.Literal("search_aborted"),
  Type.Literal("forge_station"),
  Type.Literal("forge_session"),
  Type.Literal("forge_outcome"),
  Type.Literal("forge_blueprint_book"),
  Type.Literal("tribulation_state"),
  Type.Literal("tribulation_broadcast"),
  Type.Literal("ascension_quota"),
  Type.Literal("heart_demon_offer"),
  Type.Literal("social_anonymity"),
  Type.Literal("social_exposure"),
  Type.Literal("social_pact"),
  Type.Literal("social_feud"),
  Type.Literal("social_renown_delta"),
  Type.Literal("niche_intrusion"),
  Type.Literal("niche_guardian_fatigue"),
  Type.Literal("niche_guardian_broken"),
  Type.Literal("sparring_invite"),
  Type.Literal("trade_offer"),
  Type.Literal("realm_vision_params"),
  Type.Literal("spiritual_sense_targets"),
  Type.Literal("false_skin_state"),
]);
export type ServerDataType = Static<typeof ServerDataType>;

export const ServerDataWelcomeV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("welcome"),
    message: Type.String({ maxLength: MAX_PAYLOAD_BYTES }),
  },
  { additionalProperties: false },
);
export type ServerDataWelcomeV1 = Static<typeof ServerDataWelcomeV1>;

export const ServerDataHeartbeatV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("heartbeat"),
    message: Type.String({ maxLength: MAX_PAYLOAD_BYTES }),
  },
  { additionalProperties: false },
);
export type ServerDataHeartbeatV1 = Static<typeof ServerDataHeartbeatV1>;

export const ServerDataNarrationV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("narration"),
    narrations: Type.Array(Narration),
  },
  { additionalProperties: false },
);
export type ServerDataNarrationV1 = Static<typeof ServerDataNarrationV1>;

export const ServerDataZoneInfoV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("zone_info"),
    zone: Type.String(),
    spirit_qi: Type.Number({ minimum: -1, maximum: 1 }),
    danger_level: Type.Integer({ minimum: 0, maximum: 5 }),
    status: Type.Optional(ZoneStatusV1),
    active_events: Type.Optional(Type.Array(Type.String())),
    perception_text: Type.Optional(Type.String({ minLength: 1 })),
  },
  { additionalProperties: false },
);
export type ServerDataZoneInfoV1 = Static<typeof ServerDataZoneInfoV1>;

export const ServerDataEventAlertV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("event_alert"),
    event: EventKind,
    message: Type.String({ maxLength: 500 }),
    zone: Type.Optional(Type.String()),
    duration_ticks: Type.Optional(Type.Integer({ minimum: 0 })),
  },
  { additionalProperties: false },
);
export type ServerDataEventAlertV1 = Static<typeof ServerDataEventAlertV1>;

export const ServerDataPlayerStateV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("player_state"),
    player: Type.Optional(Type.String()),
    realm: Type.String(),
    spirit_qi: Type.Number({ minimum: 0, maximum: 160 }),
    karma: Type.Number({ minimum: -1, maximum: 1 }),
    composite_power: Type.Number({ minimum: 0, maximum: 1 }),
    breakdown: PlayerPowerBreakdown,
    zone: Type.String(),
    season_state: Type.Optional(SeasonStateV1),
    social: Type.Optional(PlayerSocialSnapshotV1),
  },
  { additionalProperties: false },
);
export type ServerDataPlayerStateV1 = Static<typeof ServerDataPlayerStateV1>;

export const ServerDataUiOpenV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("ui_open"),
    ui: Type.Optional(Type.String({ description: "logical UI key" })),
    xml: Type.String({ maxLength: 10_240 }),
  },
  { additionalProperties: false },
);
export type ServerDataUiOpenV1 = Static<typeof ServerDataUiOpenV1>;

export const QiColorPracticeWeightV1 = Type.Object(
  {
    color: ColorKind,
    weight: Type.Number({ minimum: 0 }),
    ratio: Type.Number({ minimum: 0, maximum: 1 }),
  },
  { additionalProperties: false },
);
export type QiColorPracticeWeightV1 = Static<typeof QiColorPracticeWeightV1>;

export const ServerDataCultivationDetailV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("cultivation_detail"),
    realm: Type.String(),
    opened: CultivationOpenedArrayV1,
    flow_rate: CultivationFlowArrayV1,
    flow_capacity: CultivationFlowArrayV1,
    integrity: CultivationIntegrityArrayV1,
    open_progress: Type.Optional(CultivationProgressArrayV1),
    cracks_count: Type.Optional(CultivationCracksArrayV1),
    contamination_total: Type.Number({ minimum: 0 }),
    lifespan: Type.Optional(LifespanPreviewV1),
    recent_skill_milestones_summary: Type.Optional(Type.String({ maxLength: 4096 })),
    skill_milestones: Type.Optional(Type.Array(SkillMilestoneSnapshotV1)),
    qi_color_main: Type.Optional(ColorKind),
    qi_color_secondary: Type.Optional(ColorKind),
    qi_color_chaotic: Type.Optional(Type.Boolean()),
    qi_color_hunyuan: Type.Optional(Type.Boolean()),
    practice_weights: Type.Optional(Type.Array(QiColorPracticeWeightV1, { maxItems: 10 })),
  },
  { additionalProperties: false },
);
export type ServerDataCultivationDetailV1 = Static<
  typeof ServerDataCultivationDetailV1
>;

export const QiColorObservedV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("qi_color_observed"),
    observer: Type.String({ minLength: 1, maxLength: 128 }),
    observed: Type.String({ minLength: 1, maxLength: 128 }),
    main: ColorKind,
    secondary: Type.Optional(ColorKind),
    is_chaotic: Type.Boolean(),
    is_hunyuan: Type.Boolean(),
    realm_diff: Type.Number(),
  },
  { additionalProperties: false },
);
export type QiColorObservedV1 = Static<typeof QiColorObservedV1>;

export const ServerDataInventorySnapshotV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("inventory_snapshot"),
    ...InventorySnapshotV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataInventorySnapshotV1 = Static<
  typeof ServerDataInventorySnapshotV1
>;

export const DroppedLootEntryV1 = Type.Object(
  {
    instance_id: Type.Integer({ minimum: 0 }),
    source_container_id: Type.String(),
    source_row: Type.Integer({ minimum: 0 }),
    source_col: Type.Integer({ minimum: 0 }),
    world_pos: Vec3,
    item: InventoryItemViewV1,
  },
  { additionalProperties: false },
);
export type DroppedLootEntryV1 = Static<typeof DroppedLootEntryV1>;

export const ServerDataDroppedLootSyncV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("dropped_loot_sync"),
    drops: Type.Array(DroppedLootEntryV1),
  },
  { additionalProperties: false },
);
export type ServerDataDroppedLootSyncV1 = Static<typeof ServerDataDroppedLootSyncV1>;

const ServerDataInventoryEventMovedV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("inventory_event"),
    ...InventoryEventMovedV1.properties,
  },
  { additionalProperties: false },
);

const ServerDataInventoryEventStackChangedV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("inventory_event"),
    ...InventoryEventStackChangedV1.properties,
  },
  { additionalProperties: false },
);

const ServerDataInventoryEventDroppedV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("inventory_event"),
    ...InventoryEventDroppedV1.properties,
  },
  { additionalProperties: false },
);

const ServerDataInventoryEventDurabilityChangedV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("inventory_event"),
    ...InventoryEventDurabilityChangedV1.properties,
  },
  { additionalProperties: false },
);

export const ServerDataInventoryEventV1 = Type.Union([
  ServerDataInventoryEventMovedV1,
  ServerDataInventoryEventDroppedV1,
  ServerDataInventoryEventStackChangedV1,
  ServerDataInventoryEventDurabilityChangedV1,
]);
export type ServerDataInventoryEventV1 = Static<typeof ServerDataInventoryEventV1>;

export const ServerDataBotanyHarvestProgressV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("botany_harvest_progress"),
    session_id: Type.String({ minLength: 1 }),
    target_id: Type.String({ minLength: 1 }),
    target_name: Type.String({ minLength: 1 }),
    plant_kind: Type.String({ minLength: 1 }),
    mode: BotanyHarvestModeV1,
    progress: Type.Number({ minimum: 0, maximum: 1 }),
    auto_selectable: Type.Boolean(),
    request_pending: Type.Boolean(),
    interrupted: Type.Boolean(),
    completed: Type.Boolean(),
    detail: Type.String(),
    hazard_hints: Type.Optional(Type.Array(Type.String({ maxLength: 500 }))),
    // plan §1.3 投影锚定：目标植物世界坐标，client 侧做 world→screen 投影定位浮窗。
    // 省略时 client 回退到准星右侧锚点。
    target_pos: Type.Optional(
      Type.Tuple([Type.Number(), Type.Number(), Type.Number()]),
    ),
  },
  { additionalProperties: false },
);
export type ServerDataBotanyHarvestProgressV1 = Static<
  typeof ServerDataBotanyHarvestProgressV1
>;

export const BotanyModelOverlayV1 = Type.Union([
  Type.Literal("none"),
  Type.Literal("emissive"),
  Type.Literal("dual_phase"),
]);
export type BotanyModelOverlayV1 = Static<typeof BotanyModelOverlayV1>;

export const BotanyPlantV2RenderProfileV1 = Type.Object(
  {
    plant_id: Type.String({ minLength: 1 }),
    base_mesh_ref: Type.String({ minLength: 1 }),
    tint_rgb: Type.Integer({ minimum: 0, maximum: 0xffffff }),
    tint_rgb_secondary: Type.Optional(Type.Integer({ minimum: 0, maximum: 0xffffff })),
    model_overlay: BotanyModelOverlayV1,
  },
  { additionalProperties: false },
);
export type BotanyPlantV2RenderProfileV1 = Static<
  typeof BotanyPlantV2RenderProfileV1
>;

export const ServerDataBotanyPlantV2RenderProfilesV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("botany_plant_v2_render_profiles"),
    profiles: Type.Array(BotanyPlantV2RenderProfileV1),
  },
  { additionalProperties: false },
);
export type ServerDataBotanyPlantV2RenderProfilesV1 = Static<
  typeof ServerDataBotanyPlantV2RenderProfilesV1
>;

export const ServerDataLumberProgressV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("lumber_progress"),
    session_id: Type.String({ minLength: 1 }),
    log_pos: BlockPosV1,
    progress: Type.Number({ minimum: 0, maximum: 1 }),
    interrupted: Type.Boolean(),
    completed: Type.Boolean(),
    detail: Type.String(),
  },
  { additionalProperties: false },
);
export type ServerDataLumberProgressV1 = Static<
  typeof ServerDataLumberProgressV1
>;

export const ServerDataBotanySkillV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("botany_skill"),
    level: Type.Integer({ minimum: 0 }),
    xp: Type.Integer({ minimum: 0 }),
    xp_to_next_level: Type.Integer({ minimum: 1 }),
    auto_unlock_level: Type.Integer({ minimum: 1 }),
  },
  { additionalProperties: false },
);
export type ServerDataBotanySkillV1 = Static<typeof ServerDataBotanySkillV1>;

// ─── 炼丹推送（plan-alchemy-v1 §4） ────────────────────────────────────────

export const ServerDataAlchemyFurnaceV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("alchemy_furnace"),
    pos: Type.Optional(BlockPosV1),
    tier: Type.Integer({ minimum: 1, maximum: 9 }),
    integrity: Type.Number({ minimum: 0 }),
    integrity_max: Type.Number({ minimum: 0 }),
    owner_name: Type.String(),
    has_session: Type.Boolean(),
  },
  { additionalProperties: false },
);
export type ServerDataAlchemyFurnaceV1 = Static<typeof ServerDataAlchemyFurnaceV1>;

export const ServerDataAlchemySessionV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("alchemy_session"),
    /** null = 未起炉。 */
    recipe_id: Type.Union([Type.String(), Type.Null()]),
    active: Type.Boolean(),
    elapsed_ticks: Type.Integer({ minimum: 0 }),
    target_ticks: Type.Integer({ minimum: 0 }),
    temp_current: Type.Number({ minimum: 0, maximum: 1 }),
    temp_target: Type.Number({ minimum: 0, maximum: 1 }),
    temp_band: Type.Number({ minimum: 0 }),
    qi_injected: Type.Number({ minimum: 0 }),
    qi_target: Type.Number({ minimum: 0 }),
    status_label: Type.String(),
    stages: Type.Array(AlchemyStageHintV1),
    /** 服务端预格式化后给 client 直接显示（含色码）。 */
    interventions_recent: Type.Array(Type.String(), { maxItems: 8 }),
  },
  { additionalProperties: false },
);
export type ServerDataAlchemySessionV1 = Static<typeof ServerDataAlchemySessionV1>;

export const ServerDataAlchemyOutcomeForecastV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("alchemy_outcome_forecast"),
    perfect_pct: Type.Number({ minimum: 0, maximum: 100 }),
    good_pct: Type.Number({ minimum: 0, maximum: 100 }),
    flawed_pct: Type.Number({ minimum: 0, maximum: 100 }),
    waste_pct: Type.Number({ minimum: 0, maximum: 100 }),
    explode_pct: Type.Number({ minimum: 0, maximum: 100 }),
    perfect_note: Type.String(),
    good_note: Type.String(),
    flawed_note: Type.String(),
  },
  { additionalProperties: false },
);
export type ServerDataAlchemyOutcomeForecastV1 = Static<
  typeof ServerDataAlchemyOutcomeForecastV1
>;

export const ServerDataAlchemyOutcomeResolvedV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("alchemy_outcome_resolved"),
    bucket: AlchemyOutcomeBucket,
    recipe_id: Type.Union([Type.String(), Type.Null()]),
    pill: Type.Optional(Type.String()),
    quality: Type.Optional(Type.Number({ minimum: 0, maximum: 1 })),
    toxin_amount: Type.Optional(Type.Number({ minimum: 0 })),
    toxin_color: Type.Optional(ColorKind),
    qi_gain: Type.Optional(Type.Number({ minimum: 0 })),
    side_effect_tag: Type.Optional(Type.String()),
    flawed_path: Type.Boolean(),
    damage: Type.Optional(Type.Number({ minimum: 0 })),
    meridian_crack: Type.Optional(Type.Number({ minimum: 0 })),
  },
  { additionalProperties: false },
);
export type ServerDataAlchemyOutcomeResolvedV1 = Static<
  typeof ServerDataAlchemyOutcomeResolvedV1
>;

export const ServerDataAlchemyRecipeBookV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("alchemy_recipe_book"),
    learned: Type.Array(AlchemyRecipeEntryV1),
    current_index: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type ServerDataAlchemyRecipeBookV1 = Static<typeof ServerDataAlchemyRecipeBookV1>;

export const ServerDataAlchemyContaminationV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("alchemy_contamination"),
    /** 通常 mellow + violent 各一条；可扩展更多色。 */
    levels: Type.Array(AlchemyContaminationLevelV1, { minItems: 0, maxItems: 10 }),
    metabolism_note: Type.String(),
  },
  { additionalProperties: false },
);
export type ServerDataAlchemyContaminationV1 = Static<
  typeof ServerDataAlchemyContaminationV1
>;

export const ServerDataDeathScreenV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("death_screen"),
    visible: Type.Boolean(),
    cause: Type.String(),
    luck_remaining: Type.Number({ minimum: 0, maximum: 1 }),
    final_words: Type.Array(Type.String()),
    countdown_until_ms: Type.Integer({ minimum: 0 }),
    can_reincarnate: Type.Boolean(),
    can_terminate: Type.Boolean(),
    stage: Type.Optional(DeathScreenStageV1),
    death_number: Type.Optional(Type.Integer({ minimum: 1 })),
    zone_kind: Type.Optional(DeathScreenZoneKindV1),
    lifespan: Type.Optional(LifespanPreviewV1),
  },
  { additionalProperties: false },
);
export type ServerDataDeathScreenV1 = Static<typeof ServerDataDeathScreenV1>;

export const ServerDataTerminateScreenV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("terminate_screen"),
    visible: Type.Boolean(),
    final_words: Type.String(),
    epilogue: Type.String(),
    archetype_suggestion: Type.String(),
  },
  { additionalProperties: false },
);
export type ServerDataTerminateScreenV1 = Static<typeof ServerDataTerminateScreenV1>;

export const ServerDataTribulationBroadcastV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("tribulation_broadcast"),
    active: Type.Boolean(),
    actor_name: Type.String(),
    stage: Type.String(),
    world_x: Type.Number(),
    world_z: Type.Number(),
    expires_at_ms: Type.Integer({ minimum: 0 }),
    spectate_invite: Type.Boolean(),
    spectate_distance: Type.Number({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type ServerDataTribulationBroadcastV1 = Static<
  typeof ServerDataTribulationBroadcastV1
>;

export const ServerDataTribulationStateV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("tribulation_state"),
    active: Type.Boolean(),
    char_id: Type.String(),
    actor_name: Type.String(),
    kind: Type.Union([
      Type.Literal("du_xu"),
      Type.Literal("zone_collapse"),
      Type.Literal("targeted"),
    ]),
    phase: Type.Union([
      Type.Literal("omen"),
      Type.Literal("lock"),
      Type.Literal("wave"),
      Type.Literal("heart_demon"),
      Type.Literal("settle"),
    ]),
    world_x: Type.Number(),
    world_z: Type.Number(),
    wave_current: Type.Integer({ minimum: 0 }),
    wave_total: Type.Integer({ minimum: 0 }),
    started_tick: Type.Integer({ minimum: 0 }),
    phase_started_tick: Type.Integer({ minimum: 0 }),
    next_wave_tick: Type.Integer({ minimum: 0 }),
    failed: Type.Boolean(),
    half_step_on_success: Type.Boolean(),
    participants: Type.Array(Type.String()),
    result: Type.Optional(Type.Union([
      Type.Literal("ascended"),
      Type.Literal("half_step"),
      Type.Literal("failed"),
      Type.Literal("killed"),
      Type.Literal("fled"),
      Type.Null(),
    ])),
  },
  { additionalProperties: false },
);
export type ServerDataTribulationStateV1 = Static<
  typeof ServerDataTribulationStateV1
>;

export const ServerDataAscensionQuotaV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("ascension_quota"),
    occupied_slots: Type.Integer({ minimum: 0 }),
    quota_limit: Type.Integer({ minimum: 0 }),
    available_slots: Type.Integer({ minimum: 0 }),
    total_world_qi: Type.Number({ minimum: 0 }),
    quota_k: Type.Number({ minimum: 0 }),
    quota_basis: Type.String({ minLength: 1 }),
  },
  { additionalProperties: false },
);
export type ServerDataAscensionQuotaV1 = Static<
  typeof ServerDataAscensionQuotaV1
>;

export const HeartDemonOfferChoiceV1 = Type.Object(
  {
    choice_id: Type.String({ minLength: 1, maxLength: 128 }),
    category: InsightCategory,
    title: Type.String({ minLength: 1, maxLength: 64 }),
    effect_summary: Type.String({ minLength: 1, maxLength: 256 }),
    flavor: Type.String({ minLength: 1, maxLength: 500 }),
    style_hint: Type.String({ maxLength: 64 }),
  },
  { additionalProperties: false },
);
export type HeartDemonOfferChoiceV1 = Static<typeof HeartDemonOfferChoiceV1>;

export const ServerDataHeartDemonOfferV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("heart_demon_offer"),
    offer_id: Type.String({ minLength: 1, maxLength: 128 }),
    trigger_id: Type.String({ minLength: 1, maxLength: 128 }),
    trigger_label: Type.String({ minLength: 1, maxLength: 128 }),
    realm_label: Type.String({ minLength: 1, maxLength: 128 }),
    composure: Type.Number({ minimum: 0, maximum: 1 }),
    quota_remaining: Type.Integer({ minimum: 0 }),
    quota_total: Type.Integer({ minimum: 1 }),
    expires_at_ms: Type.Integer({ minimum: 0 }),
    choices: Type.Array(HeartDemonOfferChoiceV1, { minItems: 1, maxItems: 4 }),
  },
  { additionalProperties: false },
);
export type ServerDataHeartDemonOfferV1 = Static<typeof ServerDataHeartDemonOfferV1>;

export const ServerDataSkillXpGainV1 = Type.Object(
  {
    type: Type.Literal("skill_xp_gain"),
    ...SkillXpGainPayloadV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataSkillXpGainV1 = Static<typeof ServerDataSkillXpGainV1>;

export const ServerDataSkillLvUpV1 = Type.Object(
  {
    type: Type.Literal("skill_lv_up"),
    ...SkillLvUpPayloadV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataSkillLvUpV1 = Static<typeof ServerDataSkillLvUpV1>;

export const ServerDataSkillCapChangedV1 = Type.Object(
  {
    type: Type.Literal("skill_cap_changed"),
    ...SkillCapChangedPayloadV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataSkillCapChangedV1 = Static<
  typeof ServerDataSkillCapChangedV1
>;

export const ServerDataSkillScrollUsedV1 = Type.Object(
  {
    type: Type.Literal("skill_scroll_used"),
    ...SkillScrollUsedPayloadV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataSkillScrollUsedV1 = Static<
  typeof ServerDataSkillScrollUsedV1
>;

export const ServerDataSkillSnapshotV1 = Type.Object(
  {
    type: Type.Literal("skill_snapshot"),
    ...SkillSnapshotPayloadV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataSkillSnapshotV1 = Static<typeof ServerDataSkillSnapshotV1>;

export const BurstMeridianEventV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("burst_meridian_event"),
    skill: Type.String({ minLength: 1 }),
    caster: Type.String({ minLength: 1 }),
    target: Type.Optional(Type.String({ minLength: 1 })),
    tick: Type.Integer({ minimum: 0 }),
    overload_ratio: Type.Number({ minimum: 0 }),
    integrity_snapshot: Type.Number({ minimum: 0, maximum: 1 }),
  },
  { additionalProperties: false },
);
export type BurstMeridianEventV1 = Static<typeof BurstMeridianEventV1>;

export const ServerDataSkillBarConfigV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("skillbar_config"),
    ...SkillBarConfigV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataSkillBarConfigV1 = Static<typeof ServerDataSkillBarConfigV1>;

export const ServerDataTechniquesSnapshotV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("techniques_snapshot"),
    ...TechniquesSnapshotV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataTechniquesSnapshotV1 = Static<typeof ServerDataTechniquesSnapshotV1>;

export const ServerDataVortexStateV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("vortex_state"),
    ...VortexFieldStateV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataVortexStateV1 = Static<typeof ServerDataVortexStateV1>;

export const ServerDataDuguPoisonStateV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("dugu_poison_state"),
    ...DuguPoisonStateV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataDuguPoisonStateV1 = Static<typeof ServerDataDuguPoisonStateV1>;

export const ServerDataCarrierStateV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("carrier_state"),
    ...CarrierStateV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataCarrierStateV1 = Static<typeof ServerDataCarrierStateV1>;

export const ServerDataFalseSkinStateV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("false_skin_state"),
    ...FalseSkinStateV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataFalseSkinStateV1 = Static<typeof ServerDataFalseSkinStateV1>;

// plan-weapon-v1 §8.2：装备槽推送走 bong:server_data + type 分发。
export const WeaponViewV1 = Type.Object(
  {
    instance_id: Type.Integer({ minimum: 0 }),
    template_id: Type.String({ minLength: 1, maxLength: 128 }),
    weapon_kind: Type.String({ minLength: 1, maxLength: 64 }),
    durability_current: Type.Number({ minimum: 0 }),
    durability_max: Type.Number({ minimum: 0 }),
    quality_tier: Type.Integer({ minimum: 0, maximum: 255 }),
  },
  { additionalProperties: false },
);
export type WeaponViewV1 = Static<typeof WeaponViewV1>;

export const WeaponEquippedV1 = Type.Object(
  {
    slot: Type.String({ minLength: 1, maxLength: 64 }),
    weapon: Type.Optional(Type.Union([WeaponViewV1, Type.Null()])),
  },
  { additionalProperties: false },
);
export type WeaponEquippedV1 = Static<typeof WeaponEquippedV1>;

export const WeaponBrokenV1 = Type.Object(
  {
    instance_id: Type.Integer({ minimum: 0 }),
    template_id: Type.String({ minLength: 1, maxLength: 128 }),
  },
  { additionalProperties: false },
);
export type WeaponBrokenV1 = Static<typeof WeaponBrokenV1>;

export const TreasureViewV1 = Type.Object(
  {
    instance_id: Type.Integer({ minimum: 0 }),
    template_id: Type.String({ minLength: 1, maxLength: 128 }),
    display_name: Type.String({ minLength: 1, maxLength: 256 }),
  },
  { additionalProperties: false },
);
export type TreasureViewV1 = Static<typeof TreasureViewV1>;

export const TreasureEquippedV1 = Type.Object(
  {
    slot: Type.String({ minLength: 1, maxLength: 64 }),
    treasure: Type.Optional(Type.Union([TreasureViewV1, Type.Null()])),
  },
  { additionalProperties: false },
);
export type TreasureEquippedV1 = Static<typeof TreasureEquippedV1>;

export const ServerDataWeaponEquippedV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("weapon_equipped"),
    ...WeaponEquippedV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataWeaponEquippedV1 = Static<
  typeof ServerDataWeaponEquippedV1
>;

export const ServerDataWeaponBrokenV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("weapon_broken"),
    ...WeaponBrokenV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataWeaponBrokenV1 = Static<typeof ServerDataWeaponBrokenV1>;

export const ServerDataTreasureEquippedV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("treasure_equipped"),
    ...TreasureEquippedV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataTreasureEquippedV1 = Static<
  typeof ServerDataTreasureEquippedV1
>;

export const ServerDataRiftPortalStateV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("rift_portal_state"),
    ...RiftPortalStateV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataRiftPortalStateV1 = Static<typeof ServerDataRiftPortalStateV1>;

export const ServerDataRiftPortalRemovedV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("rift_portal_removed"),
    ...RiftPortalRemovedV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataRiftPortalRemovedV1 = Static<
  typeof ServerDataRiftPortalRemovedV1
>;

export const ServerDataExtractStartedV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("extract_started"),
    ...ExtractStartedV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataExtractStartedV1 = Static<typeof ServerDataExtractStartedV1>;

export const ServerDataExtractProgressV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("extract_progress"),
    ...ExtractProgressV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataExtractProgressV1 = Static<typeof ServerDataExtractProgressV1>;

export const ServerDataExtractCompletedV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("extract_completed"),
    ...ExtractCompletedV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataExtractCompletedV1 = Static<typeof ServerDataExtractCompletedV1>;

export const ServerDataExtractAbortedV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("extract_aborted"),
    ...ExtractAbortedV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataExtractAbortedV1 = Static<typeof ServerDataExtractAbortedV1>;

export const ServerDataExtractFailedV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("extract_failed"),
    ...ExtractFailedV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataExtractFailedV1 = Static<typeof ServerDataExtractFailedV1>;

export const ServerDataTsyCollapseStartedIpcV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("tsy_collapse_started_ipc"),
    ...TsyCollapseStartedIpcV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataTsyCollapseStartedIpcV1 = Static<
  typeof ServerDataTsyCollapseStartedIpcV1
>;

// ─── 炼器（武器）（plan-forge-v1 §4） ───────────────────────
export const ServerDataForgeStationV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("forge_station"),
    ...WeaponForgeStationDataV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataForgeStationV1 = Static<typeof ServerDataForgeStationV1>;

export const ServerDataForgeSessionV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("forge_session"),
    ...ForgeSessionDataV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataForgeSessionV1 = Static<typeof ServerDataForgeSessionV1>;

export const ServerDataForgeOutcomeV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("forge_outcome"),
    ...ForgeOutcomeDataV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataForgeOutcomeV1 = Static<typeof ServerDataForgeOutcomeV1>;

export const ServerDataForgeBlueprintBookV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("forge_blueprint_book"),
    ...ForgeBlueprintBookDataV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataForgeBlueprintBookV1 = Static<
  typeof ServerDataForgeBlueprintBookV1
>;

// ─── 玩家社交（plan-social-v1 §7） ───────────────────────
export const ServerDataSocialAnonymityV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("social_anonymity"),
    ...SocialAnonymityPayloadV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataSocialAnonymityV1 = Static<
  typeof ServerDataSocialAnonymityV1
>;

export const ServerDataSocialExposureV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("social_exposure"),
    actor: SocialExposureEventV1.properties.actor,
    kind: SocialExposureEventV1.properties.kind,
    witnesses: SocialExposureEventV1.properties.witnesses,
    tick: SocialExposureEventV1.properties.tick,
    zone: SocialExposureEventV1.properties.zone,
  },
  { additionalProperties: false },
);
export type ServerDataSocialExposureV1 = Static<
  typeof ServerDataSocialExposureV1
>;

export const ServerDataSocialPactV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("social_pact"),
    left: SocialPactEventV1.properties.left,
    right: SocialPactEventV1.properties.right,
    terms: SocialPactEventV1.properties.terms,
    tick: SocialPactEventV1.properties.tick,
    broken: SocialPactEventV1.properties.broken,
  },
  { additionalProperties: false },
);
export type ServerDataSocialPactV1 = Static<typeof ServerDataSocialPactV1>;

export const ServerDataSocialFeudV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("social_feud"),
    left: SocialFeudEventV1.properties.left,
    right: SocialFeudEventV1.properties.right,
    tick: SocialFeudEventV1.properties.tick,
    place: SocialFeudEventV1.properties.place,
  },
  { additionalProperties: false },
);
export type ServerDataSocialFeudV1 = Static<typeof ServerDataSocialFeudV1>;

export const ServerDataSocialRenownDeltaV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("social_renown_delta"),
    char_id: SocialRenownDeltaV1.properties.char_id,
    fame_delta: SocialRenownDeltaV1.properties.fame_delta,
    notoriety_delta: SocialRenownDeltaV1.properties.notoriety_delta,
    tags_added: SocialRenownDeltaV1.properties.tags_added,
    tick: SocialRenownDeltaV1.properties.tick,
    reason: SocialRenownDeltaV1.properties.reason,
  },
  { additionalProperties: false },
);
export type ServerDataSocialRenownDeltaV1 = Static<
  typeof ServerDataSocialRenownDeltaV1
>;

export const ServerDataNicheIntrusionV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("niche_intrusion"),
    niche_pos: NicheIntrusionEventV1.properties.niche_pos,
    intruder_id: NicheIntrusionEventV1.properties.intruder_id,
    items_taken: NicheIntrusionEventV1.properties.items_taken,
    taint_delta: NicheIntrusionEventV1.properties.taint_delta,
  },
  { additionalProperties: false },
);
export type ServerDataNicheIntrusionV1 = Static<typeof ServerDataNicheIntrusionV1>;

export const ServerDataNicheGuardianFatigueV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("niche_guardian_fatigue"),
    guardian_kind: NicheGuardianFatigueV1.properties.guardian_kind,
    charges_remaining: NicheGuardianFatigueV1.properties.charges_remaining,
  },
  { additionalProperties: false },
);
export type ServerDataNicheGuardianFatigueV1 = Static<
  typeof ServerDataNicheGuardianFatigueV1
>;

export const ServerDataNicheGuardianBrokenV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("niche_guardian_broken"),
    guardian_kind: NicheGuardianBrokenV1.properties.guardian_kind,
    intruder_id: NicheGuardianBrokenV1.properties.intruder_id,
  },
  { additionalProperties: false },
);
export type ServerDataNicheGuardianBrokenV1 = Static<
  typeof ServerDataNicheGuardianBrokenV1
>;

export const ServerDataSparringInviteV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("sparring_invite"),
    ...SparringInvitePayloadV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataSparringInviteV1 = Static<
  typeof ServerDataSparringInviteV1
>;

export const ServerDataTradeOfferV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("trade_offer"),
    ...TradeOfferPayloadV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataTradeOfferV1 = Static<typeof ServerDataTradeOfferV1>;

export const ServerDataRealmVisionParamsV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("realm_vision_params"),
    ...RealmVisionParamsV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataRealmVisionParamsV1 = Static<
  typeof ServerDataRealmVisionParamsV1
>;

export const ServerDataSpiritualSenseTargetsV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("spiritual_sense_targets"),
    ...SpiritualSenseTargetsV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataSpiritualSenseTargetsV1 = Static<
  typeof ServerDataSpiritualSenseTargetsV1
>;

export const ServerDataContainerStateV1 = Type.Object(
  {
    type: Type.Literal("container_state"),
    ...ContainerStateV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataContainerStateV1 = Static<typeof ServerDataContainerStateV1>;

export const ServerDataSearchStartedV1 = Type.Object(
  {
    type: Type.Literal("search_started"),
    ...SearchStartedV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataSearchStartedV1 = Static<typeof ServerDataSearchStartedV1>;

export const ServerDataSearchProgressV1 = Type.Object(
  {
    type: Type.Literal("search_progress"),
    ...SearchProgressV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataSearchProgressV1 = Static<typeof ServerDataSearchProgressV1>;

export const ServerDataSearchCompletedV1 = Type.Object(
  {
    type: Type.Literal("search_completed"),
    ...SearchCompletedV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataSearchCompletedV1 = Static<typeof ServerDataSearchCompletedV1>;

export const ServerDataSearchAbortedV1 = Type.Object(
  {
    type: Type.Literal("search_aborted"),
    ...SearchAbortedV1.properties,
  },
  { additionalProperties: false },
);
export type ServerDataSearchAbortedV1 = Static<typeof ServerDataSearchAbortedV1>;

export const ServerDataV1 = Type.Union([
  ServerDataWelcomeV1,
  ServerDataHeartbeatV1,
  ServerDataNarrationV1,
  ServerDataZoneInfoV1,
  ServerDataEventAlertV1,
  ServerDataPlayerStateV1,
  ServerDataUiOpenV1,
  ServerDataCultivationDetailV1,
  QiColorObservedV1,
  ServerDataInventorySnapshotV1,
  ServerDataInventoryEventV1,
  ServerDataDroppedLootSyncV1,
  ServerDataBotanyHarvestProgressV1,
  ServerDataBotanyPlantV2RenderProfilesV1,
  ServerDataLumberProgressV1,
  ServerDataBotanySkillV1,
  ServerDataProcessingSessionV1,
  ServerDataFreshnessUpdateV1,
  ServerDataAlchemyFurnaceV1,
  ServerDataAlchemySessionV1,
  ServerDataAlchemyOutcomeForecastV1,
  ServerDataAlchemyOutcomeResolvedV1,
  ServerDataAlchemyRecipeBookV1,
  ServerDataAlchemyContaminationV1,
  ServerDataDeathScreenV1,
  ServerDataTerminateScreenV1,
  ServerDataHeartDemonOfferV1,
  ServerDataSkillXpGainV1,
  ServerDataSkillLvUpV1,
  ServerDataSkillCapChangedV1,
  ServerDataSkillScrollUsedV1,
  ServerDataSkillSnapshotV1,
  BurstMeridianEventV1,
  ServerDataSkillBarConfigV1,
  ServerDataTechniquesSnapshotV1,
  ServerDataVortexStateV1,
  ServerDataDuguPoisonStateV1,
  ServerDataCarrierStateV1,
  ServerDataFalseSkinStateV1,
  ServerDataWeaponEquippedV1,
  ServerDataWeaponBrokenV1,
  ServerDataTreasureEquippedV1,
  ServerDataRiftPortalStateV1,
  ServerDataRiftPortalRemovedV1,
  ServerDataExtractStartedV1,
  ServerDataExtractProgressV1,
  ServerDataExtractCompletedV1,
  ServerDataExtractAbortedV1,
  ServerDataExtractFailedV1,
  ServerDataTsyCollapseStartedIpcV1,
  ServerDataContainerStateV1,
  ServerDataSearchStartedV1,
  ServerDataSearchProgressV1,
  ServerDataSearchCompletedV1,
  ServerDataSearchAbortedV1,
  ServerDataForgeStationV1,
  ServerDataForgeSessionV1,
  ServerDataForgeOutcomeV1,
  ServerDataForgeBlueprintBookV1,
  ServerDataTribulationStateV1,
  ServerDataTribulationBroadcastV1,
  ServerDataAscensionQuotaV1,
  ServerDataSocialAnonymityV1,
  ServerDataSocialExposureV1,
  ServerDataSocialPactV1,
  ServerDataSocialFeudV1,
  ServerDataSocialRenownDeltaV1,
  ServerDataNicheIntrusionV1,
  ServerDataNicheGuardianFatigueV1,
  ServerDataNicheGuardianBrokenV1,
  ServerDataSparringInviteV1,
  ServerDataTradeOfferV1,
  ServerDataRealmVisionParamsV1,
  ServerDataSpiritualSenseTargetsV1,
]);
export type ServerDataV1 = Static<typeof ServerDataV1>;
