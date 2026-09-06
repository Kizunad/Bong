use serde_json::json;
use valence::entity::Look;
use valence::prelude::bevy_ecs::system::SystemParam;
use valence::prelude::{
    bevy_ecs, Client, Commands, DVec3, Entity, EventReader, EventWriter, Events, GameMode,
    ParamSet, Position, Query, Res, ResMut, Username, With,
};

use crate::body_plan::{
    humanoid_plan_static, resolve_body_plan, resolve_body_plan_for_target, BodyPlanPurpose,
    BodyPlanRegistry, BodyPlanResolveInputs, RaceRegistry,
};
use crate::combat::anticheat::AntiCheatCounter;
use crate::combat::arm_wound;
use crate::combat::armor::{ArmorProfileRegistry, ARMOR_MITIGATION_CAP};
use crate::combat::baomai_v4::dead_armor::{should_block_contamination, DeadMeridianArmor};
use crate::combat::body_mass::{BodyMass, Stance};
use crate::combat::jiemai::{
    jiemai_apply_effects, jiemai_effectiveness, jiemai_fov_check, jiemai_prep_window,
};
use crate::combat::knockback::{
    compute_combat_knockback, CombatKnockbackInput, KnockbackEvent, DEFAULT_CHAIN_DEPTH,
};
use crate::combat::shield_block::{
    self as shield_block_mod, shield_fov_check, SHIELD_NEAR_BREAK_DURABILITY_THRESHOLD,
};
use crate::combat::status::{body_part_damage_multiplier, has_active_status};
use crate::combat::sword_basics;
use crate::combat::tuike::{tuike_filter_contam, FalseSkin, ShedEvent};
use crate::combat::tuike_v2::physics::naked_defense_damage_multiplier;
use crate::combat::tuike_v2::StackedFalseSkins;
use crate::combat::weapon::{EquipSlot, ShieldBlockHit, ShieldBroken, Weapon, WeaponBroken};
use crate::combat::zhenmai_v2::{
    self, BackfireAmplification, MeridianHardenActive, MultiPointActive,
};
use crate::combat::CombatClock;
use crate::combat::{
    components::{
        BodyPart, CombatState, DerivedAttrs, Lifecycle, LifecycleState, Stamina, StaminaState,
        StatusEffects, Wound, Wounds, HEAD_STUN_DURATION_TICKS, HEAD_STUN_SEVERITY_THRESHOLD,
        LEG_SLOWED_DURATION_TICKS, LEG_SLOWED_SEVERITY_THRESHOLD,
    },
    events::{
        ApplyStatusEffectIntent, AttackIntent, AttackSource, CombatEvent, DeathEvent,
        DefenseIntent, DefenseKind, StatusEffectKind,
    },
    raycast::{self, raycast_humanoid},
};
use crate::cultivation::color::{record_style_practice, PracticeLog};
use crate::cultivation::components::{
    ColorKind, ContamSource, Contamination, CrackCause, Cultivation, MeridianCrack, MeridianSystem,
    QiColor,
};
use crate::cultivation::known_techniques::KnownTechniques;
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::cultivation::tribulation::JueBiLawDisruption;
use crate::inventory::{
    consume_item_instance_once, discard_inventory_item_to_dropped_loot,
    move_equipped_item_to_first_container_slot, set_item_instance_durability, DroppedLootRegistry,
    InventoryDurabilityChangedEvent, ItemRegistry, PlayerInventory, EQUIP_SLOT_CHEST,
    EQUIP_SLOT_FEET, EQUIP_SLOT_HEAD, EQUIP_SLOT_LEGS, EQUIP_SLOT_OFF_HAND,
};
use crate::network::audio_event_emit::{
    AudioRecipient, PlaySoundRecipeRequest, AUDIO_BROADCAST_RADIUS,
};
use crate::network::{gameplay_vfx, vfx_event_emit::VfxEventRequest};
use crate::npc::brain::canonical_npc_id;
use crate::npc::movement::PendingKnockback;
use crate::npc::scenario::PassiveTarget;
use crate::npc::spawn::NpcMarker;
use crate::player::state::canonical_player_id;
use crate::qi_physics::constants::{
    QI_ZHENMAI_CONCUSSION_BLEEDING_PER_SEC, QI_ZHENMAI_PARRY_RECOVERY_TICKS,
};
use crate::qi_physics::{flow_modifier, QiAccountId, QiTransfer};
use crate::schema::anticheat::ViolationKindV1;
use crate::schema::common::{GameEventType, NarrationStyle};
use crate::schema::inventory::{EquipSlotV1, EquipStateV1, InventoryLocationV1};
use crate::schema::world_state::GameEvent;
use crate::skill::components::SkillId;
use crate::skill::events::{SkillXpGain, XpGainSource};
use crate::world::events::ActiveEventsResource;
use crate::world::zone::ZoneRegistry;

const ARMOR_HIT_CONTAMINATION_MULTIPLIER: f64 = 0.1;
const ARMOR_HIT_DURABILITY_COST_POINTS: f64 = 0.5;

/// plan-race-system-v1 P4（决议 §风险#2 修复）—— 把命中落在的 intrinsic 部位折算成
/// 供护甲 `body_coverage`/`defense_profile` 匹配用的 legacy `BodyPart`。
///
/// 未易形（`morph_state = None`）或无可用 `RaceRegistry` 时，行为与 P0-P3 完全一致：
/// 直接 `id_to_legacy_body_part(intrinsic_part)`（非人形 intrinsic 部位无 legacy 对应物
/// 时返回 `None`，护甲减免不生效——这条既有行为不变）。
///
/// 已易形时：`intrinsic_part` 往往是非人形部位（如飞鲸 `tail_fin`），直接转 legacy 会
/// 提前 `None`，静默吞掉玩家实际穿着的"形态外观"护甲减免。本函数改为先经
/// `MorphPairDef.part_mapping`（方向 form_part → intrinsic_part）**逆查**出对应的
/// form 部位（如 human 形态的 "chest"），再对 **form 部位** 转 legacy——form 通常是
/// 人形构型，转换能成功，从而让形态外观护甲的减免继续折算回本体伤害。
fn legacy_part_for_wound_with_morph(
    intrinsic_part: &crate::body_plan::BodyPartId,
    morph_state: Option<&crate::body_plan::MorphState>,
    intrinsic_race: Option<&crate::body_plan::RaceId>,
    races: Option<&RaceRegistry>,
) -> Option<BodyPart> {
    if let (Some(morph), Some(intrinsic_race), Some(races)) = (morph_state, intrinsic_race, races) {
        if let Some(pair) = races.resolve_morph_pair(intrinsic_race, &morph.form) {
            if let Some(form_part) = pair.form_part_for_intrinsic(intrinsic_part) {
                if let Some(legacy) = crate::body_plan::id_to_legacy_body_part(form_part) {
                    return Some(legacy);
                }
            }
        }
    }
    crate::body_plan::id_to_legacy_body_part(intrinsic_part)
}

fn apply_armor_mitigation(
    wound: &mut Wound,
    derived: &DerivedAttrs,
    contam: &mut f64,
    morph_state: Option<&crate::body_plan::MorphState>,
    intrinsic_race: Option<&crate::body_plan::RaceId>,
    races: Option<&RaceRegistry>,
) -> Option<f32> {
    // humanoid-only boundary（P0 决议，本轮不迁移）：`DerivedAttrs.defense_profile` 仍以
    // legacy `BodyPart` 为键（护甲/体修被动减伤矩阵本轮不迁移，P1 批次范围）；非人形
    // 部位 id 没有 legacy 对应物时，视为"该部位没有任何护甲/被动减伤条目"（`None`，
    // 与今天"这个 (part, kind) 组合没配置"的既有语义完全一致），显式 `?` 提前返回。
    // plan-race-system-v1 P4：MorphState 在场时绕过这条静默吞减免的分支，经
    // part_mapping 逆查折算回 form 部位再转 legacy（见 `legacy_part_for_wound_with_morph`）。
    let legacy_part =
        legacy_part_for_wound_with_morph(&wound.location, morph_state, intrinsic_race, races)?;
    let &m = derived.defense_profile.get(&(legacy_part, wound.kind))?;
    if m <= 0.0 {
        return None;
    }

    let m = m.clamp(0.0, ARMOR_MITIGATION_CAP);
    if m <= 0.0 {
        return None;
    }
    wound.severity *= 1.0 - m;
    wound.bleeding_per_sec *= 1.0 - m;
    // plan-armor-v1 §Q10: armor 把 severity 压低 (1-m) -> contam 一阶要随之减少；
    // 然后整体再压 ARMOR_HIT_CONTAMINATION_MULTIPLIER (0.1) 实现 "甲挡住基本不污染"。
    // 两段叠乘是有意为之 —— 1-m 让强弱甲仍有量级区分（顶甲 0.015×、弱甲 0.095×），
    // 0.1 整体闸门保证哪怕弱甲也不会推 contam 失控。改公式必须同步更新
    // `armor_hit_scales_contamination_and_ticks_item_durability` 的 expected_contam。
    *contam *= 1.0 - f64::from(m);
    *contam *= ARMOR_HIT_CONTAMINATION_MULTIPLIER;
    Some(m)
}

const DEBUG_ATTACK_STAMINA_COST: f32 = 12.0;
const DEBUG_ATTACK_CONTAMINATION_FACTOR: f64 = 0.25;
const ATTACKER_EYE_HEIGHT: f64 = 1.62;
const ATTACK_QI_DAMAGE_FACTOR: f32 = 1.0;
const ATTACK_QI_THROUGHPUT_FACTOR: f64 = 1.0;

#[derive(Debug, Clone, Copy)]
struct WoundKindProfile {
    damage_mul: f32,
    bleed_mul: f32,
    contam_mul: f64,
    crack_mul: f64,
}

type CombatClientItem<'a> = (
    Entity,
    &'a Position,
    &'a Username,
    &'a crate::player::state::PlayerState,
);
type CombatClientFilter = With<Client>;
type CombatTargetItem<'a> = (
    &'a mut Wounds,
    &'a mut Stamina,
    &'a mut Contamination,
    &'a mut MeridianSystem,
    Option<&'a mut LifeRecord>,
    Option<&'a Lifecycle>,
    Option<&'a mut CombatState>,
    Option<&'a mut Cultivation>,
    Option<&'a mut FalseSkin>,
    Option<&'a StackedFalseSkins>,
    Option<&'a mut DerivedAttrs>,
    Option<&'a mut PracticeLog>,
    Option<&'a mut MultiPointActive>,
    Option<&'a MeridianHardenActive>,
    // plan-race-system-v1 P4 —— 元组已达 15 元素（WorldQuery 元组上限附近，见其余处
    // 同款注释），新增 `MorphState` 查询嵌套进最后一个元素而非追加顶层第 16 项。
    (
        Option<&'a BackfireAmplification>,
        Option<&'a crate::body_plan::MorphState>,
    ),
);
type CombatAttackerItem<'a> = (
    &'a mut Cultivation,
    &'a mut MeridianSystem,
    Option<&'a DerivedAttrs>,
    Option<&'a mut AntiCheatCounter>,
    Option<&'a CombatState>,
    Option<&'a KnownTechniques>,
    Option<&'a Lifecycle>,
    // plan-combat-hit-location-v1 P1 — 攻方自身臂伤（主手臂伤势）削减自身攻击伤害。
    // 只读，与 CombatTargetItem 的 `&mut Wounds` 同处一个 ParamSet（p0/p1），Bevy 允许。
    Option<&'a Wounds>,
    Option<&'a LifeRecord>,
);
type DefenseResponderItem<'a> = (
    &'a mut CombatState,
    &'a Cultivation,
    Option<&'a PlayerInventory>,
    Option<&'a StatusEffects>,
    Option<&'a FalseSkin>,
);
type PositionLookItem<'a> = (&'a Position, Option<&'a Look>);

/// 事件写出参数合并，避免 Bevy 0.14 顶层 SystemParam 数量上限。
#[derive(SystemParam)]
pub struct CombatResolveEventWriters<'w, 's> {
    status_effect_intents: EventWriter<'w, ApplyStatusEffectIntent>,
    out_events: EventWriter<'w, CombatEvent>,
    qi_transfers: Option<ResMut<'w, Events<QiTransfer>>>,
    /// R5 P0 — jiemai cost 的物理 settlement owner；与 zone/event 一起收进 SystemParam，
    /// 避免超过 Bevy 顶层参数上限。
    qi_ledger: ResMut<'w, crate::qi_physics::WorldQiAccount>,
    multipoint_backfires: Option<ResMut<'w, Events<zhenmai_v2::MultiPointBackfireEvent>>>,
    vfx_events: Option<ResMut<'w, Events<VfxEventRequest>>>,
    audio_events: Option<ResMut<'w, Events<PlaySoundRecipeRequest>>>,
    knockback_events: Option<ResMut<'w, Events<KnockbackEvent>>>,
    death_events: EventWriter<'w, DeathEvent>,
    durability_changed_tx: EventWriter<'w, InventoryDurabilityChangedEvent>,
    /// plan-shield-block-v1 P2 — 盾牌格挡成功触发 PARRY_BLOCK 动画（via emit_defense_animation_triggers）。
    defense_intent_tx: Option<ResMut<'w, Events<DefenseIntent>>>,
    /// plan-shield-block-v1 P4 — player-scope narration（格挡成功 / 近破盾）。
    narrations: Option<ResMut<'w, crate::player::gameplay::PendingGameplayNarrations>>,
    /// bughunt r2 QP-003 — jiemai 格挡真元守恒：扣除的 qi_cost 需回灌到防御方所在 zone。
    zone_registry: Option<ResMut<'w, ZoneRegistry>>,
    /// qi 守恒：查询攻击/防御方当前维度，用于 release_qi_amount_to_zone 定位 zone。
    dimension_q: Query<'w, 's, Option<&'static crate::world::dimension::CurrentDimension>>,
    /// `/npc_scenario passive_target` contract: damage is allowed, forced movement is not.
    passive_targets: Query<'w, 's, (), With<PassiveTarget>>,
}

pub fn apply_defense_intents(
    mut defenses: EventReader<DefenseIntent>,
    mut defenders: Query<DefenseResponderItem<'_>>,
    mut status_effect_intents: EventWriter<ApplyStatusEffectIntent>,
) {
    for defense in defenses.read() {
        let Ok((mut combat_state, cultivation, inventory, status_effects, false_skin)) =
            defenders.get_mut(defense.defender)
        else {
            continue;
        };

        if status_effects.is_some_and(|se| {
            has_active_status(se, StatusEffectKind::Stunned)
                || has_active_status(se, StatusEffectKind::VortexCasting)
                || has_active_status(se, StatusEffectKind::ParryRecovery)
                || has_active_status(se, StatusEffectKind::VoidCoreActive)
        }) {
            continue;
        }
        // plan-shield-block-v1 P2 — 盾格挡 emit 的 DefenseIntent 仅用于动画触发，
        // 不应开截脉 incoming_window 也不应施加 per-block ParryRecovery。
        // 盾格挡语义：凡人盾，无境界加成，不耦合 jiemai/zhenmai。
        if status_effects.is_some_and(|se| has_active_status(se, StatusEffectKind::ShieldBlocking))
        {
            continue;
        }
        if zhenmai_v2::parry_qi_cost_for_realm(cultivation.realm).is_none() {
            continue;
        }

        let mut window = jiemai_prep_window(inventory, defense.issued_at_tick);
        if let Some(skin) = false_skin {
            window.duration_ms = ((window.duration_ms as f32) * skin.kind.jiemai_window_modifier())
                .round()
                .max(1.0) as u32;
        }
        combat_state.incoming_window = Some(window);
        status_effect_intents.send(ApplyStatusEffectIntent {
            target: defense.defender,
            kind: StatusEffectKind::ParryRecovery,
            magnitude: 1.0,
            duration_ticks: QI_ZHENMAI_PARRY_RECOVERY_TICKS,
            issued_at_tick: defense.issued_at_tick,
        });
    }
}
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn resolve_attack_intents(
    clock: Res<CombatClock>,
    armor_profiles: Option<Res<ArmorProfileRegistry>>,
    mut intents: EventReader<AttackIntent>,
    mut active_events: Option<ResMut<ActiveEventsResource>>,
    clients: Query<CombatClientItem<'_>, CombatClientFilter>,
    positions: Query<PositionLookItem<'_>>,
    npc_markers: Query<(), With<NpcMarker>>,
    npc_positions: Query<(Entity, &Position), With<NpcMarker>>,
    statuses: Query<&StatusEffects>,
    juebi_law_disruptions: Query<&JueBiLawDisruption>,
    sparring_sessions: Query<&crate::social::components::SparringState>,
    mut combatants: ParamSet<(
        Query<CombatAttackerItem<'_>>,
        Query<CombatTargetItem<'_>>,
        Query<&GameMode>,
    )>,
    body_masses: Query<&BodyMass>,
    stances: Query<&Stance>,
    mut event_writers: CombatResolveEventWriters,
    // plan-weapon-v1 §6：武器加成 + 耐久扣减
    // plan-shield-block-v1 P3：shield_broken 事件写出 + ItemRegistry（读 ShieldSpec）
    // plan-shield-block-v1 P4：defender KnownTechniques（shield_block_profile 缩放）+ shield_block_hit emit
    // plan-baomai-v4 P0：dead_armor 免疫区拦截污染（第 13 位，≤16 上限安全）
    // plan-race-system-v1 P0b：body_part_multipliers 改查目标实体解析出的 BodyPlan（第
    // 14/15 位，仍 ≤16 上限）——`resolve_attack_intents` 已在函数顶层参数用满 Bevy
    // SystemParam 元组 16 元素上限（见本函数其余参数 + `CombatResolveEventWriters` 的
    // "避免 Bevy 0.14 顶层 SystemParam 数量上限" 注释），新增资源只能塞进这个既有 bucket
    // tuple。`Option<Res<...>>` 与顶层 `armor_profiles: Option<Res<ArmorProfileRegistry>>`
    // 同款——大量既有单测未插入这两个资源，缺失时 `body_part_multipliers` 优雅退化到
    // `body_plan::humanoid_plan_static()`（生产环境 `body_plan::register()` 恒装载，
    // 这条退化分支不会在真实部署触发）。
    // plan-race-system-v1 P0 review 修复（BLOCKING-2）：`mutation_states`（第 16 位）
    // 是 dandao `mutation_slot_mapping` 的真实消费点——顶层新增一个 system param 会撞
    // `resolve_attack_intents` 自身函数元数的 16 上限（实测：加了会直接编译失败,
    // `SystemParamFunction` 未对 17 元实现），只能继续塞进这个 bucket tuple。
    weapon_break: (
        Query<&mut Weapon>,
        EventWriter<WeaponBroken>,
        EventWriter<ShieldBroken>,
        EventWriter<ShieldBlockHit>,
        Commands,
        Query<&mut PlayerInventory>,
        Query<&QiColor>,
        Option<ResMut<DroppedLootRegistry>>,
        Option<ResMut<Events<SkillXpGain>>>,
        Option<ResMut<Events<ShedEvent>>>,
        Option<Res<ItemRegistry>>,
        Query<Option<&KnownTechniques>>,
        Query<Option<&DeadMeridianArmor>>,
        Option<Res<BodyPlanRegistry>>,
        Option<Res<RaceRegistry>>,
        Query<&crate::dandao::mutation::MutationState>,
    ),
) {
    let (
        mut weapons,
        mut weapon_broken_events,
        mut shield_broken_events,
        mut shield_block_hit_events,
        mut commands,
        mut inventories,
        qi_colors,
        mut dropped_loot_registry,
        mut skill_xp_events,
        mut shed_events,
        item_registry,
        defender_known_q,
        dead_armor_q,
        body_plan_registry,
        race_registry,
        mutation_states,
    ) = weapon_break;

    for intent in intents.read() {
        // Attacker blocked by VoidCoreActive (cannot attack while in void core collapse)
        if statuses.get(intent.attacker).is_ok_and(|se| {
            has_active_status(se, StatusEffectKind::Stunned)
                || has_active_status(se, StatusEffectKind::VortexCasting)
                || has_active_status(se, StatusEffectKind::ParryRecovery)
                || has_active_status(se, StatusEffectKind::VoidCoreActive)
        }) {
            continue;
        }

        let Some((attacker_position, attacker_id, target_entity, target_position, target_id)) =
            resolve_intent_entities(intent, &clients, &positions, &npc_markers, &npc_positions)
        else {
            continue;
        };
        let target_damageable = {
            let game_modes = combatants.p2();
            crate::combat::is_damageable(target_entity, &game_modes)
        };
        if !target_damageable {
            continue;
        }

        // Target blocked by VoidCoreActive (cannot be hit while in void core collapse)
        if statuses
            .get(target_entity)
            .is_ok_and(|se| has_active_status(se, StatusEffectKind::VoidCoreActive))
        {
            continue;
        }

        let target_lifecycle_blocks_attack = {
            let mut target_query = combatants.p1();
            let Ok((_, _, _, _, _, lifecycle, _, _, _, _, _, _, _, _, _)) =
                target_query.get_mut(target_entity)
            else {
                continue;
            };
            lifecycle.is_some_and(|lifecycle| {
                matches!(
                    lifecycle.state,
                    LifecycleState::NearDeath
                        | LifecycleState::AwaitingRevival
                        | LifecycleState::Terminated
                )
            })
        };
        if target_lifecycle_blocks_attack {
            continue;
        }

        let qi_invest = f64::from(intent.qi_invest.max(0.0));
        let juebi_law_env = juebi_law_disruptions
            .get(intent.attacker)
            .ok()
            .map(|disruption| disruption.env_field())
            .unwrap_or_default();

        {
            let mut attacker_query = combatants.p0();
            let Ok((
                mut attacker_cultivation,
                _,
                _,
                mut anticheat_counter,
                attacker_combat_state,
                _,
                attacker_lifecycle,
                _,
                attacker_life_record,
            )) = attacker_query.get_mut(intent.attacker)
            else {
                continue;
            };

            // Attacker is dead / dying — skip this intent.
            if attacker_lifecycle.is_some_and(|lc| {
                matches!(
                    lc.state,
                    LifecycleState::NearDeath
                        | LifecycleState::AwaitingRevival
                        | LifecycleState::Terminated
                )
            }) {
                continue;
            }

            if intent.source == AttackSource::Melee
                && intent.debug_command.is_none()
                && attacker_combat_state
                    .and_then(|state| state.last_attack_at_tick)
                    .is_some_and(|last_attack_at_tick| intent.issued_at_tick <= last_attack_at_tick)
            {
                record_anticheat_violation(
                    anticheat_counter.as_deref_mut(),
                    ViolationKindV1::CooldownBypassed,
                    format!(
                        "cooldown: issued_at_tick={} last_attack_at_tick={}",
                        intent.issued_at_tick,
                        attacker_combat_state
                            .and_then(|state| state.last_attack_at_tick)
                            .unwrap_or_default()
                    ),
                );
            }

            if qi_invest > f64::EPSILON
                && !source_uses_prepaid_qi(intent.source)
                && attacker_cultivation.qi_current + f64::EPSILON < qi_invest
            {
                if intent.debug_command.is_none() {
                    record_anticheat_violation(
                        anticheat_counter.as_deref_mut(),
                        ViolationKindV1::QiInvestExceeded,
                        format!(
                            "qi_invest: requested={:.3} available={:.3}",
                            qi_invest, attacker_cultivation.qi_current
                        ),
                    );
                }
                continue;
            }

            if qi_invest > f64::EPSILON && !source_uses_prepaid_qi(intent.source) {
                // 守恒红线：现场扣费在命中判定前完成，命中与未命中都必须结算同一笔
                // qi_invest。既有 release facade 原子更新 Cultivation、zone/overflow
                // 与完整 QiTransfer 审计；任何落账失败都 fail closed，不进入伤害路径。
                let attacker_dim = event_writers
                    .dimension_q
                    .get(intent.attacker)
                    .ok()
                    .flatten();
                let attacker_pos = positions
                    .get(intent.attacker)
                    .ok()
                    .map(|(position, _)| position);
                if let Err(error) = crate::cultivation::death_hooks::release_qi_amount_to_zone(
                    &mut attacker_cultivation,
                    qi_invest,
                    attacker_pos,
                    attacker_dim,
                    attacker_life_record,
                    event_writers.zone_registry.as_deref_mut(),
                    &mut event_writers.qi_ledger,
                    event_writers.qi_transfers.as_deref_mut(),
                    "combat_attack_qi_invest",
                ) {
                    tracing::warn!(
                        ?error,
                        attacker = ?intent.attacker,
                        "[bong][combat] attack qi investment release failed closed"
                    );
                    continue;
                }
            }
        }

        let attacker_eye_position = attacker_position + DVec3::new(0.0, ATTACKER_EYE_HEIGHT, 0.0);
        // §8.1 #1/#3 决议 — 攻方瞄准方向：废除 raycast_humanoid 内置恒定胸心 fallback。
        // NPC 恒定走"指向目标几何中心 + 确定性高斯 jitter"（不使用其自身 Look 组件，
        // 不做物种战术瞄准，见 §8.1 #3）；玩家走真实 Look 方向，不叠加人工 jitter
        // （Look 本身已含真实瞄准误差）。`Look::default()`（yaw=0/pitch=0）在 server 端
        // 等价于"刚出生、从未上报过任何 rotation 包的玩家"——与缺失 Look 组件同等对待，
        // 退化为几何中心瞄准：真实玩家一进入游戏就持续上报视角，精确撞上这一哨兵值的
        // 概率可忽略不计，这也是保留既有测试 fixture（未显式设置 Look）行为不变的现实路径。
        let aim_direction = if npc_markers.get(intent.attacker).is_ok() {
            let seed = raycast::npc_aim_seed(&attacker_id, intent.issued_at_tick);
            let sigma_scale = raycast::weapon_aim_jitter_scale(intent.reach);
            raycast::npc_aim_direction(attacker_eye_position, target_position, seed, sigma_scale)
        } else {
            let attacker_look = positions
                .get(intent.attacker)
                .ok()
                .and_then(|(_, look)| look.copied());
            match attacker_look.filter(|look| *look != Look::default()) {
                Some(look) => {
                    let look_vec = look.vec();
                    DVec3::new(
                        f64::from(look_vec.x),
                        f64::from(look_vec.y),
                        f64::from(look_vec.z),
                    )
                }
                None => raycast::chest_aim_direction(attacker_eye_position, target_position),
            }
        };

        // plan-race-system-v1 P0c —— raycast_humanoid 命中几何按**目标实体**分派：
        // 借道 `combatants.p1()` 峰值读一次目标 `Cultivation.race`（只读即可，用完这个
        // block 就释放这次借用——后面 549 行附近还会再借一次拿完整目标数据）。查询链
        // 与 `body_part_multipliers`（本文件下方）同款：`resolve_body_plan_for_target`
        // 资源齐全时走 `resolve_body_plan`（`BodyPlanPurpose::Intrinsic`），资源缺失或
        // 解析失败（未知 race，理论上不会发生——持久化加载路径早已拒绝未知 race 落地为
        // 组件）时退化到 `humanoid_plan_static()`。P0b 未在 `CombatTargetItem` 里查
        // `BeastKind`（同 `body_part_multipliers` 注释：该 15 元素元组已逼近 Bevy
        // `WorldQuery` 元组上限，且 `races.json` 现阶段所有 `BeastKind` 派生种族的
        // `body_plan_id` 均为 "humanoid"）——`beast_kind: None` 落进 Tier2/Tier3 分支，
        // 与"真的查了 BeastKind"得到完全相同的 humanoid 解析结果，bit-for-bit 不受影响。
        let target_body_plan = {
            let mut target_query = combatants.p1();
            let defender_cultivation_for_plan = target_query.get_mut(target_entity).ok().and_then(
                |(
                    _wounds,
                    _stamina,
                    _contamination,
                    _meridians,
                    _life_record,
                    _lifecycle,
                    _combat_state,
                    defender_cultivation,
                    _false_skin,
                    _tuike_v2_stack,
                    _defender_attrs,
                    _defender_practice_log,
                    _multipoint_active,
                    _harden_active,
                    _backfire_amplification,
                )| defender_cultivation,
            );
            resolve_body_plan_for_target(
                target_entity,
                BodyPlanPurpose::Intrinsic,
                BodyPlanResolveInputs {
                    cultivation: defender_cultivation_for_plan.as_deref(),
                    beast_kind: None,
                    morph_state: None,
                },
                body_plan_registry.as_deref(),
                race_registry.as_deref(),
            )
        };

        // plan-race-system-v1 P0 review r2（BLOCKING-1 收口）—— `PartBoxes` 分支需要目标
        // 朝向把世界系射线变换到局部系；`HeightBands` 分支忽略这个参数（既有人形几何不
        // 依赖朝向）。缺失 `Look`（NPC 常见）时退化到 yaw=0——只影响非人形 `PartBoxes`
        // 构型的命中判定，人形 `HeightBands` 行为不受影响（bit-for-bit 不变）。
        let target_yaw_radians = positions
            .get(target_entity)
            .ok()
            .and_then(|(_, look)| look)
            .map(|look| f64::from(look.yaw).to_radians())
            .unwrap_or(0.0);

        let Some(hit_probe) = raycast_humanoid(
            target_body_plan,
            attacker_eye_position,
            target_position,
            target_yaw_radians,
            f64::from(
                intent.reach.max
                    / (juebi_law_env.law_disruption_distance_multiplier() as f32).max(1.0),
            ),
            aim_direction,
        ) else {
            if intent.debug_command.is_none() {
                let mut attacker_query = combatants.p0();
                if let Ok((_, _, _, mut anticheat_counter, _, _, _, _, _)) =
                    attacker_query.get_mut(intent.attacker)
                {
                    record_anticheat_violation(
                        anticheat_counter.as_deref_mut(),
                        ViolationKindV1::ReachExceeded,
                        format!(
                            "reach: target_distance={:.3} server_max={:.3}",
                            target_position.distance(attacker_position),
                            intent.reach.max
                        ),
                    );
                }
            }
            continue;
        };
        let distance = hit_probe.distance as f32;

        let (attacker_damage_multiplier, attacker_body_mass, sword_damage_multiplier) = {
            let mut attacker_query = combatants.p0();
            let Ok((
                attacker_cultivation,
                mut attacker_meridians,
                attacker_attrs,
                _,
                _,
                attacker_known_techniques,
                _,
                attacker_wounds,
                _,
            )) = attacker_query.get_mut(intent.attacker)
            else {
                continue;
            };
            // plan-race-system-v1 P0 review 修复（BLOCKING-1）—— 攻方臂伤倍率查询改为
            // 按攻方实体解析出的 BodyPlan 分发（`BodyPlanPurpose::Intrinsic`），不再固定
            // 读 `humanoid_plan_static()`；资源缺失/未知 race 时 `resolve_body_plan_for_target`
            // 自身已内建退化到 humanoid，行为 bit-for-bit 不变（同 `body_part_multipliers`/
            // `target_body_plan` 消费点同款模式）。`beast_kind: None` 简化同上方 `target_body_plan`
            // 注释的既有先例（races.json 现阶段所有 BeastKind 派生种族均落 humanoid）。
            let attacker_body_plan = resolve_body_plan_for_target(
                intent.attacker,
                BodyPlanPurpose::Intrinsic,
                BodyPlanResolveInputs {
                    cultivation: Some(&*attacker_cultivation),
                    beast_kind: None,
                    morph_state: None,
                },
                body_plan_registry.as_deref(),
                race_registry.as_deref(),
            );
            // plan-combat-hit-location-v1 P1（决议 §8.1 #2）— 攻方主手臂伤势削减自身攻击伤害。
            // Bruise×0.95/Abrasion×0.90/Laceration×0.80/Fracture×0.60/Severed×0.40。
            let attacker_arm_wound_damage_multiplier =
                arm_wound::combined_factor_from_optional(attacker_wounds, attacker_body_plan)
                    .attack_damage_multiplier;

            if qi_invest > f64::EPSILON && !sword_basics::is_sword_attack_source(intent.source) {
                if let Some(primary_meridian) =
                    first_open_or_fallback_meridian(&mut attacker_meridians)
                {
                    primary_meridian.throughput_current += qi_invest
                        * ATTACK_QI_THROUGHPUT_FACTOR
                        * juebi_law_env.law_disruption_channeling_multiplier();
                }
            }
            (
                attacker_attrs
                    .map(|attrs| attrs.attack_power)
                    .unwrap_or(1.0)
                    * attacker_arm_wound_damage_multiplier,
                body_masses.get(intent.attacker).ok().copied(),
                sword_basics::source_to_technique(intent.source)
                    .and_then(|technique| {
                        attacker_known_techniques.and_then(|known| {
                            known
                                .entries
                                .iter()
                                .find(|entry| entry.id == technique.id())
                                .map(|entry| {
                                    sword_basics::sword_profile(technique, entry.proficiency)
                                        .damage_multiplier
                                })
                        })
                    })
                    .unwrap_or(1.0),
            )
        };

        let mut target_query = combatants.p1();
        let Ok((
            mut wounds,
            mut stamina,
            mut contamination,
            mut meridians,
            life_record,
            lifecycle,
            combat_state,
            defender_cultivation,
            false_skin,
            tuike_v2_stack,
            defender_attrs,
            defender_practice_log,
            mut multipoint_active,
            harden_active,
            (backfire_amplification, defender_morph_state),
        )) = target_query.get_mut(target_entity)
        else {
            continue;
        };
        // plan-race-system-v1 P4 —— 提前克隆一份本体 race 快照（`defender_cultivation`
        // 在下方 `!is_physical_hit` 分支会被按值移动进临时 `if let` 元组，之后不再可借用）；
        // 护甲折算（`apply_armor_mitigation` / 耐久扣减分支）需要在移动点之后仍能读取
        // 本体 race，故这里先克隆一份 owned 快照，不依赖后续借用存活。
        let defender_race_snapshot: Option<crate::body_plan::RaceId> =
            defender_cultivation.as_deref().map(|c| c.race.clone());
        // plan-combat-hit-location-v1 P1（决议 §8.1 #2）— 防御方副手臂（持盾侧）伤势削减
        // 格挡/招架减伤效果：Laceration ×0.80(-20%)/Fracture·Severed ×0.60(-40%)。
        // 读取本次命中造成的伤口写入 wounds.entries 之前的既有伤势状态。
        // plan-race-system-v1 P0 review 修复（BLOCKING-1）—— 复用上方已按目标实体解析
        // 出的 `target_body_plan`（同一个 `target_entity`），不再固定读 humanoid_plan_static()。
        let defender_off_arm_block_multiplier =
            arm_wound::combined_factor(&wounds, target_body_plan).block_multiplier;
        let decay = ((intent.reach.max - distance) / intent.reach.max.max(0.001)).clamp(0.0, 1.0);
        let hit_qi = (intent.qi_invest * decay).max(0.0);
        let is_physical_hit = intent.qi_invest <= f32::EPSILON;
        let (body_damage_multiplier, contam_multiplier, bleed_multiplier) = body_part_multipliers(
            target_entity,
            defender_cultivation.as_deref(),
            body_plan_registry.as_deref(),
            race_registry.as_deref(),
            &hit_probe.part_id,
        );
        let wound_profile = wound_kind_profile(intent.wound_kind);
        let defender_damage_multiplier = defender_attrs
            .as_ref()
            .map(|attrs| attrs.defense_power)
            .unwrap_or(1.0)
            * naked_defense_damage_multiplier(tuike_v2_stack, clock.tick);
        // 正式武器走 Weapon component；凡器不挂 Weapon，但主手使用时按低倍率临时武器结算。
        let mut hit_tool: Option<crate::tools::ToolKind> = None;
        let mut weapon_kind_for_knockback = None;
        let (weapon_base_damage, weapon_multiplier): (f32, f32) = match weapons.get(intent.attacker)
        {
            Ok(weapon) => {
                weapon_kind_for_knockback = Some(weapon.weapon_kind);
                let resonance = inventories.get(intent.attacker).ok().and_then(|inventory| {
                    crate::forge::artifact_meridian::artifact_resonance_for_inventory(
                        inventory,
                        weapon.instance_id,
                        qi_colors.get(intent.attacker).ok(),
                    )
                });
                let multiplier = resonance
                    .map(|value| weapon.damage_multiplier_with_resonance(value))
                    .unwrap_or_else(|| weapon.damage_multiplier());
                (weapon.base_attack.max(1.0), multiplier)
            }
            Err(_) => {
                hit_tool = inventories
                    .get(intent.attacker)
                    .ok()
                    .and_then(crate::tools::main_hand_tool_in_inventory);
                let multiplier = hit_tool
                    .map(crate::tools::ToolKind::combat_damage_multiplier)
                    .unwrap_or(1.0);
                (1.0, multiplier)
            }
        };
        let defender_stance = stances
            .get(target_entity)
            .ok()
            .copied()
            .unwrap_or_else(|| Stance::from_runtime(&stamina, combat_state.as_deref()));
        let attacker_mass = attacker_body_mass.unwrap_or_default();
        let defender_mass = body_masses
            .get(target_entity)
            .ok()
            .copied()
            .unwrap_or_default();
        let zhenmai_attack_kind =
            zhenmai_v2::attack_kind_for_source(intent.source, intent.wound_kind);
        let harden_damage_multiplier = if is_physical_hit {
            1.0
        } else {
            harden_active
                .map(|active| flow_modifier(1.0, active.damage_multiplier))
                .unwrap_or(1.0)
        };
        let backfire_incoming_damage_multiplier = if is_physical_hit {
            1.0
        } else {
            backfire_amplification
                .filter(|active| active.active_for(zhenmai_attack_kind, clock.tick))
                .map(|active| active.incoming_damage_multiplier)
                .unwrap_or(1.0)
        };
        let base_damage = if is_physical_hit {
            weapon_base_damage
                * body_damage_multiplier
                * attacker_damage_multiplier
                * defender_damage_multiplier
                * weapon_multiplier
                * wound_profile.damage_mul
                * sword_damage_multiplier
        } else {
            hit_qi
                * ATTACK_QI_DAMAGE_FACTOR
                * body_damage_multiplier
                * attacker_damage_multiplier
                * defender_damage_multiplier
                * weapon_multiplier
                * harden_damage_multiplier
                * backfire_incoming_damage_multiplier
                * sword_damage_multiplier
        };
        let juebi_backfire_fraction = if is_physical_hit {
            0.0
        } else {
            juebi_law_env.law_disruption_backfire_fraction() as f32
        };
        let damage = (base_damage * (1.0 - juebi_backfire_fraction)).max(1.0);
        let juebi_backfire_damage = (base_damage * juebi_backfire_fraction).max(0.0);
        let was_alive = wounds.health_current > 0.0;
        if !event_writers.passive_targets.contains(target_entity) {
            if let Ok(knockback) = compute_combat_knockback(CombatKnockbackInput {
                physical_damage: damage,
                qi_invest: hit_qi,
                attacker_mass: Some(&attacker_mass),
                target_mass: Some(&defender_mass),
                target_stance: Some(&defender_stance),
                target_cultivation: defender_cultivation.as_deref(),
                weapon_kind: weapon_kind_for_knockback,
                source: intent.source,
            }) {
                if knockback.is_actionable() {
                    let direction = target_position - attacker_position;
                    if direction.length() > f64::EPSILON {
                        commands
                            .entity(target_entity)
                            .insert(PendingKnockback::from_result(
                                intent.attacker,
                                intent.source,
                                direction,
                                knockback,
                                DEFAULT_CHAIN_DEPTH,
                            ));
                        if let Some(events) = event_writers.knockback_events.as_deref_mut() {
                            events.send(KnockbackEvent {
                                attacker: intent.attacker,
                                target: target_entity,
                                source: intent.source,
                                distance_blocks: knockback.distance_blocks,
                                velocity_blocks_per_tick: knockback.velocity_blocks_per_tick,
                                duration_ticks: knockback.duration_ticks,
                                kinetic_energy: knockback.kinetic_energy,
                                collision_damage: None,
                                chain_depth: DEFAULT_CHAIN_DEPTH,
                                block_broken: false,
                            });
                        }
                    }
                }
            }
        }
        let mut pending_reflected_qi = 0.0_f64;
        if !is_physical_hit {
            if let Some(active) = multipoint_active.as_deref_mut() {
                let reflected =
                    zhenmai_v2::multipoint_contact(active, f64::from(hit_qi), zhenmai_attack_kind);
                pending_reflected_qi += reflected;
                if let Some(events) = event_writers.multipoint_backfires.as_deref_mut() {
                    events.send(zhenmai_v2::MultiPointBackfireEvent {
                        defender: target_entity,
                        attacker: Some(intent.attacker),
                        attack_kind: zhenmai_attack_kind,
                        contact_index: active.contact_count,
                        reflected_qi: reflected,
                        remaining_points: u32::from(active.points)
                            .saturating_sub(active.contact_count),
                        expires_at_tick: active.expires_at_tick,
                        tick: clock.tick,
                    });
                }
                zhenmai_v2::apply_self_damage(&mut wounds, active.self_damage_per_contact);
            }
        }
        if !is_physical_hit {
            if let Some(active) = backfire_amplification
                .filter(|active| active.active_for(zhenmai_attack_kind, clock.tick))
            {
                pending_reflected_qi += zhenmai_v2::reflected_qi(
                    f64::from(hit_qi),
                    active.k_drain,
                    zhenmai_attack_kind,
                );
            }
        }
        if pending_reflected_qi > f64::EPSILON {
            if let Some(transfer) = zhenmai_v2::backfire_transfer(
                QiAccountId::player(attacker_id.clone()),
                QiAccountId::player(target_id.clone()),
                pending_reflected_qi,
            ) {
                if let Some(events) = event_writers.qi_transfers.as_deref_mut() {
                    events.send(transfer);
                }
            }
            let attacker = intent.attacker;
            commands.add(
                move |world: &mut valence::prelude::bevy_ecs::world::World| {
                    zhenmai_v2::apply_reflected_qi(world, attacker, pending_reflected_qi);
                },
            );
        }
        if juebi_backfire_damage > f32::EPSILON {
            let attacker = intent.attacker;
            commands.add(
                move |world: &mut valence::prelude::bevy_ecs::world::World| {
                    zhenmai_v2::apply_self_damage_to_entity(world, attacker, juebi_backfire_damage);
                },
            );
        }

        // plan-weapon-v1 §6.3：命中一次 → 耐久扣减。
        // 若耐久归零收集 broken info,下面统一 commands 操作(避免与 mut borrow 冲突)。
        let broken_weapon: Option<(u64, String)> = if let Ok(mut weapon) =
            weapons.get_mut(intent.attacker)
        {
            if weapon.tick_durability() {
                Some((weapon.instance_id, weapon.template_id.clone()))
            } else {
                if let Ok(mut inventory) = inventories.get_mut(intent.attacker) {
                    let durability_ratio = if weapon.durability_max > 0.0 {
                        f64::from((weapon.durability / weapon.durability_max).clamp(0.0, 1.0))
                    } else {
                        0.0
                    };
                    if let Err(error) = set_item_instance_durability(
                        &mut inventory,
                        weapon.instance_id,
                        durability_ratio,
                    ) {
                        tracing::warn!(
                                "[bong][combat][weapon] failed to persist durability for instance {}: {}",
                                weapon.instance_id,
                                error
                            );
                    }
                }
                None
            }
        } else {
            None
        };
        if let Some((instance_id, template_id)) = broken_weapon {
            let mut broken_dislodged = false;
            if let Ok(mut inventory) = inventories.get_mut(intent.attacker) {
                let broken_slot = inventory.equipped.iter().find_map(|(slot, contents)| {
                    contents
                        .iter_all()
                        .find(|item| item.instance_id == instance_id)
                        .map(|_| match slot.as_str() {
                            crate::inventory::EQUIP_SLOT_MAIN_HAND => EquipSlotV1::MainHand,
                            crate::inventory::EQUIP_SLOT_OFF_HAND => EquipSlotV1::OffHand,
                            _ => EquipSlotV1::MainHand,
                        })
                });
                if let Err(error) = set_item_instance_durability(&mut inventory, instance_id, 0.0) {
                    tracing::warn!(
                        "[bong][combat][weapon] failed to persist broken durability for instance {}: {}",
                        instance_id,
                        error
                    );
                }
                match move_equipped_item_to_first_container_slot(&mut inventory, instance_id) {
                    Ok(_) => {
                        broken_dislodged = true;
                    }
                    Err(error) => {
                        tracing::warn!(
                            "[bong][combat][weapon] failed to move broken weapon instance {} into container: {}",
                            instance_id,
                            error
                        );
                        if let Some(slot) = broken_slot {
                            if let Some(dropped_loot_registry) = dropped_loot_registry.as_mut() {
                                let dropped = discard_inventory_item_to_dropped_loot(
                                    &mut inventory,
                                    dropped_loot_registry,
                                    [
                                        attacker_position.x,
                                        attacker_position.y,
                                        attacker_position.z,
                                    ],
                                    crate::world::dimension::DimensionKind::Overworld,
                                    instance_id,
                                    &InventoryLocationV1::Equip {
                                        slot,
                                        state: EquipStateV1::Held,
                                    },
                                );
                                match dropped {
                                    Ok(_) => {
                                        broken_dislodged = true;
                                    }
                                    Err(drop_error) => {
                                        tracing::warn!(
                                            "[bong][combat][weapon] failed to drop broken weapon instance {} after container fallback failed: {}",
                                            instance_id,
                                            drop_error
                                        );
                                    }
                                }
                            } else {
                                tracing::warn!(
                                    "[bong][combat][weapon] broken weapon instance {} cannot fall back to dropped loot because DroppedLootRegistry is unavailable",
                                    instance_id
                                );
                            }
                        } else {
                            tracing::warn!(
                                "[bong][combat][weapon] broken weapon instance {} no longer has an equipped slot",
                                instance_id
                            );
                        }
                    }
                }
            }
            if broken_dislodged {
                commands.entity(intent.attacker).remove::<Weapon>();
                weapon_broken_events.send(WeaponBroken {
                    entity: intent.attacker,
                    instance_id,
                    template_id,
                });
            }
        }

        if let Some(tool) = hit_tool {
            if let Ok(mut inventory) = inventories.get_mut(intent.attacker) {
                crate::tools::damage_main_hand_tool(
                    intent.attacker,
                    &mut inventory,
                    &mut event_writers.durability_changed_tx,
                    tool.durability_cost_ratio_per_use(),
                );
            }
        }

        let mut emitted_contam_delta = if is_physical_hit {
            0.0
        } else {
            f64::from(damage)
                * DEBUG_ATTACK_CONTAMINATION_FACTOR
                * f64::from(contam_multiplier)
                * wound_profile.contam_mul
        };
        let mut jiemai_success = false;
        let mut jiemai_effectiveness_value = None;
        let mut jiemai_contam_reduced = None;
        let mut jiemai_wound_severity = None;
        let mut sword_parry_success = false;
        let mut sword_parry_block_ratio = None;
        let mut sword_parry_contam_reduced = None;
        let mut sword_parry_reflected_damage = None;
        // plan-shield-block-v1 P2 — 盾牌格挡结果（无反伤）
        let mut shield_block_success = false;
        let mut shield_block_ratio: Option<f32> = None;
        let mut shield_block_contam_reduced: Option<f64> = None;
        let mut shield_blocked_damage: Option<f32> = None;
        let mut false_skin = false_skin;
        let mut defender_attrs = defender_attrs;

        // 污染结算顺序：截脉先改污染量，护甲再削污染，伪皮最后截胡剩余污染。

        stamina.current =
            (stamina.current - DEBUG_ATTACK_STAMINA_COST * decay).clamp(0.0, stamina.max);
        stamina.last_drain_tick = Some(clock.tick);
        stamina.state = if stamina.current <= 0.0 {
            StaminaState::Exhausted
        } else {
            StaminaState::Combat
        };

        if !is_physical_hit {
            if let (Some(mut combat_state), Some(mut defender_cultivation)) =
                (combat_state, defender_cultivation)
            {
                let window_open = combat_state
                    .incoming_window
                    .as_ref()
                    .is_some_and(|window| clock.tick < window.expires_at_tick());

                let qi_cost = zhenmai_v2::parry_qi_cost_for_realm(defender_cultivation.realm);
                let fov_ok = jiemai_fov_check(
                    attacker_position,
                    target_position,
                    positions
                        .get(target_entity)
                        .ok()
                        .and_then(|(_position, look)| look),
                    defender_cultivation.realm,
                );
                if window_open
                    && qi_cost
                        .is_some_and(|cost| defender_cultivation.qi_current + f64::EPSILON >= cost)
                    && fov_ok
                {
                    // bughunt r2 QP-003 — 守恒：格挡真元费用通过 typed transaction
                    // 原子扣除并回灌防御方所在 zone；失败时不开格挡结果。
                    {
                        let defender_dim =
                            event_writers.dimension_q.get(target_entity).ok().flatten();
                        let defender_pos = positions.get(target_entity).ok().map(|(pos, _)| pos);
                        let release = crate::cultivation::death_hooks::release_qi_amount_to_zone(
                            &mut defender_cultivation,
                            qi_cost.expect("window guard requires a realm parry qi cost"),
                            defender_pos,
                            defender_dim,
                            life_record.as_deref(),
                            event_writers.zone_registry.as_deref_mut(),
                            &mut event_writers.qi_ledger,
                            event_writers.qi_transfers.as_deref_mut(),
                            "jiemai_parry",
                        );
                        if let Err(error) = release {
                            tracing::warn!(
                                ?error,
                                "[bong][combat] jiemai parry qi release failed closed"
                            );
                            continue;
                        }
                    }

                    let before = emitted_contam_delta;
                    let effectiveness = jiemai_effectiveness(distance);
                    let mut concussion_severity =
                        zhenmai_v2::parry_self_damage_for_realm(defender_cultivation.realm);
                    jiemai_apply_effects(
                        effectiveness,
                        &mut emitted_contam_delta,
                        &mut concussion_severity,
                    );
                    jiemai_effectiveness_value = Some(effectiveness);
                    jiemai_contam_reduced = Some((before - emitted_contam_delta).max(0.0));
                    jiemai_wound_severity = Some(concussion_severity);

                    wounds.entries.push(Wound {
                        location: hit_probe.part_id.clone(),
                        kind: crate::combat::components::WoundKind::Concussion,
                        severity: concussion_severity,
                        bleeding_per_sec: QI_ZHENMAI_CONCUSSION_BLEEDING_PER_SEC,
                        created_at_tick: clock.tick,
                        inflicted_by: Some(attacker_id.clone()),
                    });
                    if let Some(mut practice_log) = defender_practice_log {
                        // qi_colors 已存在于 weapon_break tuple（只读），直接复用
                        let defender_qi_color = qi_colors.get(target_entity).ok();
                        record_style_practice(
                            &mut practice_log,
                            ColorKind::Violent,
                            defender_qi_color,
                        );
                    }
                    jiemai_success = true;
                }

                combat_state.incoming_window = None;
            }
        }

        let mut wound = Wound {
            location: hit_probe.part_id.clone(),
            kind: intent.wound_kind,
            severity: damage,
            bleeding_per_sec: damage * 0.05 * bleed_multiplier * wound_profile.bleed_mul,
            created_at_tick: clock.tick,
            inflicted_by: Some(attacker_id.clone()),
        };

        let defender_status_effects = statuses.get(target_entity).ok();
        if let Some(block_ratio) =
            active_status_magnitude(defender_status_effects, StatusEffectKind::SwordParrying)
        {
            // plan-combat-hit-location-v1 P1 — 副手臂伤势打折招架减伤效果（决议 §8.1 #2）。
            let block_ratio = (block_ratio * defender_off_arm_block_multiplier).clamp(0.0, 0.95);
            let before_severity = wound.severity;
            let before_contam = emitted_contam_delta;
            wound.severity *= 1.0 - block_ratio;
            wound.bleeding_per_sec *= 1.0 - block_ratio;
            emitted_contam_delta *= f64::from(1.0 - block_ratio);
            let blocked_damage = (before_severity - wound.severity).max(0.0);
            let reflected_damage = blocked_damage * 0.15;
            sword_parry_success = true;
            sword_parry_block_ratio = Some(block_ratio);
            sword_parry_contam_reduced = Some((before_contam - emitted_contam_delta).max(0.0));
            sword_parry_reflected_damage = Some(reflected_damage);
            let attacker = intent.attacker;
            let reflected_by = target_id.clone();
            let reflected_at_tick = clock.tick;
            if reflected_damage > f32::EPSILON {
                commands.add(
                    move |world: &mut valence::prelude::bevy_ecs::world::World| {
                        if let Some(mut attacker_wounds) = world.get_mut::<Wounds>(attacker) {
                            attacker_wounds.health_current = (attacker_wounds.health_current
                                - reflected_damage)
                                .clamp(0.0, attacker_wounds.health_max);
                            attacker_wounds.entries.push(Wound {
                                // plan-combat-hit-location-v1 P2（决议 §8.1 旁路桶 #1）——
                                // 剑招招架反伤打的是攻方持械的那只手：格挡时兵刃互击的
                                // 冲击沿武器传回持械臂，物理上不该落在恒定的胸口。
                                // humanoid-only boundary（plan-race-system-v1 P0 决议，
                                // 本轮不迁移）：MAIN_ARM 是编译期 legacy BodyPart 字面量
                                // （见 arm_wound.rs 模块文档"P0b"节），经 legacy_body_part_to_id
                                // 全双射转换为 BodyPartId。
                                location: crate::body_plan::legacy_body_part_to_id(
                                    crate::combat::arm_wound::MAIN_ARM,
                                ),
                                kind: crate::combat::components::WoundKind::Blunt,
                                severity: reflected_damage,
                                bleeding_per_sec: 0.0,
                                created_at_tick: reflected_at_tick,
                                inflicted_by: Some(reflected_by),
                            });
                        }
                    },
                );
            }
            event_writers
                .status_effect_intents
                .send(ApplyStatusEffectIntent {
                    target: intent.attacker,
                    kind: StatusEffectKind::Staggered,
                    magnitude: 0.3,
                    duration_ticks: sword_basics::SWORD_PARRY_STAGGER_TICKS,
                    issued_at_tick: clock.tick,
                });
            let defender = target_entity;
            commands.add(
                move |world: &mut valence::prelude::bevy_ecs::world::World| {
                    sword_basics::record_sword_parry_success(world, defender);
                },
            );
        }

        // plan-shield-block-v1 P2 / P4 — 盾牌格挡减伤分支（独立于 SwordParrying，无反伤）。
        // P4: block_ratio 经 shield_block_profile(proficiency) 缩放（替代 P2 固定 spec.block_ratio）。
        // 正面 FOV 判定：须先过 shield_fov_check，背面不减伤。
        if let Some(_raw_ratio) =
            active_status_magnitude(defender_status_effects, StatusEffectKind::ShieldBlocking)
        {
            // plan-shield-block-v1 P4 — 读取 defender 的 shield_block proficiency 并应用 profile。
            // shield_block_profile 根据 template_id 给出各盾的上下限（木盾 0.5→0.6，骨盾 0.65→0.72）。
            let (shield_template_id, shield_proficiency) = inventories
                .get(target_entity)
                .ok()
                .and_then(|inv| {
                    inv.equipped
                        .get(EQUIP_SLOT_OFF_HAND)
                        .and_then(|s| s.held.clone())
                })
                .map(|item| {
                    let proficiency = defender_known_q
                        .get(target_entity)
                        .ok()
                        .flatten()
                        .and_then(|known| {
                            known
                                .entries
                                .iter()
                                .find(|e| e.id == shield_block_mod::SHIELD_BLOCK_TECHNIQUE_ID)
                                .map(|e| e.proficiency)
                        })
                        .unwrap_or(0.0);
                    (item.template_id, proficiency)
                })
                .unwrap_or_else(|| ("wooden_shield".to_string(), 0.0));
            let profile =
                shield_block_mod::shield_block_profile(&shield_template_id, shield_proficiency);
            // plan-combat-hit-location-v1 P1 — 副手臂伤势打折盾牌格挡减伤效果（决议 §8.1 #2）。
            let ratio = (profile.block_ratio * defender_off_arm_block_multiplier).clamp(0.0, 0.95);
            // 正面 FOV 判定（±120°，dot ≥ -0.5）
            let fov_ok = shield_fov_check(
                attacker_position,
                target_position,
                positions
                    .get(target_entity)
                    .ok()
                    .and_then(|(_pos, look)| look),
            );
            if fov_ok && ratio > 0.0 {
                let before_severity = wound.severity;
                let before_contam = emitted_contam_delta;
                wound.severity *= 1.0 - ratio;
                wound.bleeding_per_sec *= 1.0 - ratio;
                emitted_contam_delta *= f64::from(1.0 - ratio);
                let blocked = (before_severity - wound.severity).max(0.0);
                shield_block_success = true;
                shield_block_ratio = Some(ratio);
                shield_block_contam_reduced = Some((before_contam - emitted_contam_delta).max(0.0));
                shield_blocked_damage = Some(blocked);

                // plan-shield-block-v1 P4 — 近破盾 narration 文本（在 get_mut 块内计算，在块外 emit）。
                let mut near_break_narration_text: Option<String> = None;
                // plan-shield-block-v1 P4 — shield_block_hit template_id（在 get_mut 块内捕获，在块外 emit）。
                let mut shield_block_hit_template_id: Option<String> = None;

                // plan-shield-block-v1 P3 — 盾牌耐久扣减。
                // 语义：durability_max = 次满伤格挡。每次满伤格挡扣 1 单位（ratio 减 1/durability_max）。
                // 盾的 ItemInstance.durability 以 0..1 ratio 存储，与 set_item_instance_durability 契约一致。
                // SwordParrying+ShieldBlocking 互斥裁定：
                //   同帧两者同时 active 时，上方 SwordParrying 分支（lines ~912-963）先执行，
                //   ShieldBlocking 分支（当前）后执行，两者串行削减伤害（不叠加同一伤害来源）。
                //   SwordParrying 格挡已改变 wound.severity，ShieldBlocking 从改变后的值继续削，
                //   产生双重减伤但分属独立机制，不存在同一伤害被两侧重叠归零的算术失衡。
                //   防御者通过境界解锁 SwordParrying（需截脉窗口 + 真元），ShieldBlocking 仅需举盾——
                //   同帧双激活在实际游戏流程中极罕见（需同 tick 同时满足截脉窗口和举盾）；
                //   不施加互斥守护，两者独立减伤符合设计意图（盾减物理 + 截脉减真元污染）。
                // 注意：block_ratio clamp to 0.95 确保不会除零。
                if let Ok(mut inventory) = inventories.get_mut(target_entity) {
                    if let Some(item) = inventory
                        .equipped
                        .get(EQUIP_SLOT_OFF_HAND)
                        .and_then(|s| s.held.as_ref())
                    {
                        let instance_id = item.instance_id;
                        let template_id_snap = item.template_id.clone();
                        // plan-shield-block-v1 P4：记录盾 template_id 供格挡命中通知。
                        shield_block_hit_template_id = Some(template_id_snap.clone());
                        let cur_ratio = item.durability;
                        let durability_max = item_registry
                            .as_deref()
                            .and_then(|reg| reg.get(&template_id_snap))
                            .and_then(|tpl| tpl.shield_spec.as_ref())
                            .map(|spec| spec.durability_max)
                            .unwrap_or(40.0); // 无 registry（单测环境）fallback 木盾值

                        if durability_max > 0.0 && cur_ratio > 0.0 {
                            // blocked_damage / durability_max 扣减比例
                            // 满伤格挡（=1次格挡整体damage=1.0 severity units） → 扣 1/durability_max ratio
                            // 部分伤害按比例线性扣
                            let cur_abs = cur_ratio * durability_max;
                            let cost = f64::from(blocked); // blocked 是本次 f32 伤害单位数
                            let next_abs = (cur_abs - cost).max(0.0);
                            let next_ratio = (next_abs / durability_max).clamp(0.0, 1.0);

                            if next_ratio < cur_ratio {
                                match set_item_instance_durability(
                                    &mut inventory,
                                    instance_id,
                                    next_ratio,
                                ) {
                                    Ok(update) => {
                                        event_writers.durability_changed_tx.send(
                                            InventoryDurabilityChangedEvent {
                                                entity: target_entity,
                                                revision: update.revision,
                                                instance_id: update.instance_id,
                                                durability: update.durability,
                                            },
                                        );
                                        // 耐久归零 → emit ShieldBroken + 移除盾物品（盾销毁）
                                        if next_ratio <= 0.0 {
                                            // 从 off_hand 移除盾（盾销毁，不保留在背包）
                                            let _ = consume_item_instance_once(
                                                &mut inventory,
                                                instance_id,
                                            );
                                            shield_broken_events.send(ShieldBroken {
                                                entity: target_entity,
                                                instance_id,
                                                template_id: template_id_snap.clone(),
                                            });
                                        }
                                        // plan-shield-block-v1 P4 — 近破盾 narration 文本。
                                        // next_ratio > 0: 盾未销毁但已低于预警阈值。
                                        if next_ratio > 0.0
                                            && next_ratio < SHIELD_NEAR_BREAK_DURABILITY_THRESHOLD
                                            && near_break_narration_text.is_none()
                                        {
                                            near_break_narration_text =
                                                Some(if template_id_snap == "bone_shield" {
                                                    "骨盾发出一声脆响，裂纹爬上盾沿。".to_string()
                                                } else {
                                                    "盾面传来裂木之声，这盾快撑不住了。".to_string()
                                                });
                                        }
                                    }
                                    Err(error) => {
                                        tracing::warn!(
                                            "[bong][combat][shield] failed to persist durability for shield instance {}: {}",
                                            instance_id,
                                            error
                                        );
                                    }
                                }
                            }
                        }
                    }
                }

                // 格挡成功：emit DefenseIntent 触发 PARRY_BLOCK 动画（兼容现有路径，不覆盖）。
                if let Some(defense_tx) = event_writers.defense_intent_tx.as_deref_mut() {
                    defense_tx.send(DefenseIntent {
                        defender: target_entity,
                        issued_at_tick: clock.tick,
                    });
                }

                // plan-shield-block-v1 P4 — 格挡成功 → 熟练度增益（接线点）。
                // 严格镜像 resolve.rs:963-965 SwordParry 的 commands.add 接线写法。
                let defender_for_proficiency = target_entity;
                commands.add(
                    move |world: &mut valence::prelude::bevy_ecs::world::World| {
                        shield_block_mod::record_shield_block_success(
                            world,
                            defender_for_proficiency,
                        );
                    },
                );

                // plan-shield-block-v1 P4 — 格挡成功 narration（scope=Player, style=Perception）。
                if let Some(narrations) = event_writers.narrations.as_deref_mut() {
                    // target_id 是 canonical_player_id（NPC 无 Username 时返回 offline: 前缀）
                    if !target_id.starts_with("offline:") && !target_id.starts_with("npc:") {
                        narrations.push_player(
                            &target_id,
                            "盾面一震，那一下被卸开了大半。",
                            NarrationStyle::Perception,
                        );
                    }
                }

                // plan-shield-block-v1 P4 — 近破盾 narration（耐久低于阈值时）。
                // near_break_narration_text 在 inventories.get_mut 块内计算（避免重借），
                // 在该块外 emit（narrations 在事件写出参数中）。
                if let Some(text) = near_break_narration_text.as_deref() {
                    if !target_id.starts_with("offline:") && !target_id.starts_with("npc:") {
                        if let Some(narrations) = event_writers.narrations.as_deref_mut() {
                            narrations.push_player(&target_id, text, NarrationStyle::Perception);
                        }
                    }
                }

                // plan-shield-block-v1 P4 — 格挡命中 → emit ShieldBlockHit（携带 template_id）。
                // shield_block_hit_template_id 在 inventories.get_mut 块内捕获（避免重借），
                // 在该块外 emit。client ShieldBlockHitHandler 按 template_id 触发材质差异化粒子+音效。
                if let Some(ref tmpl) = shield_block_hit_template_id {
                    shield_block_hit_events.send(ShieldBlockHit {
                        entity: target_entity,
                        template_id: tmpl.clone(),
                    });
                }
            }
        }

        // plan-armor-v1 §4.1：护甲减免在截脉判定之后应用。
        // 截脉当前只影响污染与额外 concussion，不直接改变本次伤口 severity。
        if let Some(attrs) = defender_attrs.as_deref() {
            let armor_mitigation = apply_armor_mitigation(
                &mut wound,
                attrs,
                &mut emitted_contam_delta,
                defender_morph_state,
                defender_race_snapshot.as_ref(),
                race_registry.as_deref(),
            );

            // 护甲命中：扣减装备耐久（少量）。
            if let (Some(_m), Some(armor_profiles)) = (armor_mitigation, armor_profiles.as_deref())
            {
                if let Ok(mut inventory) = inventories.get_mut(target_entity) {
                    // plan-layered-equip-v1 P0.2（桶②）— 遍历护甲身体槽 worn 全层，取减伤最高一件扣耐久。
                    let best: Option<(u64, u32, f64, f32)> = [
                        EQUIP_SLOT_HEAD,
                        EQUIP_SLOT_CHEST,
                        EQUIP_SLOT_LEGS,
                        EQUIP_SLOT_FEET,
                    ]
                    .into_iter()
                    .filter_map(|slot| inventory.equipped.get(slot))
                    .flat_map(|contents| contents.worn.iter())
                    .filter_map(|item| {
                        let ap = armor_profiles.get(item.template_id.as_str())?;
                        // humanoid-only boundary（P0 决议，本轮不迁移）：`ArmorProfile.
                        // body_coverage` 仍以 legacy `BodyPart` 8 段为键（护甲系统本轮不
                        // 迁移，P1 批次范围）；非人形部位 id 没有 legacy 对应物时，视为
                        // "这件护甲不可能覆盖该部位"（护甲系统本就假设人形躯体），显式
                        // 提前返回而非静默吞掉。plan-race-system-v1 P4：MorphState 在场
                        // 时经 part_mapping 逆查折算回 form 部位再转 legacy（同上
                        // `apply_armor_mitigation` 调用点的同款折算，见
                        // `legacy_part_for_wound_with_morph`），耐久扣减目标与实际吃到
                        // 减免的护甲件保持一致。
                        let legacy_part = legacy_part_for_wound_with_morph(
                            &hit_probe.part_id,
                            defender_morph_state,
                            defender_race_snapshot.as_ref(),
                            race_registry.as_deref(),
                        )?;
                        if !ap.body_coverage.contains(&legacy_part) {
                            return None;
                        }
                        let base_m = *ap.kind_mitigation.get(&intent.wound_kind).unwrap_or(&0.0);
                        if base_m <= 0.0 {
                            return None;
                        }
                        let effective_mul =
                            ap.effective_multiplier_for_durability_ratio(item.durability);
                        let effective_m = (base_m * effective_mul).clamp(0.0, ARMOR_MITIGATION_CAP);
                        if effective_m <= 0.0 {
                            return None;
                        }
                        Some((
                            item.instance_id,
                            ap.durability_max,
                            item.durability,
                            effective_m,
                        ))
                    })
                    .max_by(|a, b| a.3.total_cmp(&b.3));

                    if let Some((instance_id, durability_max, cur_ratio, _effective_m)) = best {
                        if durability_max > 0 && cur_ratio > 0.0 {
                            let durability_max = f64::from(durability_max);
                            let cur_abs = (cur_ratio * durability_max).max(0.0);
                            let next_abs = (cur_abs - ARMOR_HIT_DURABILITY_COST_POINTS).max(0.0);
                            let next_ratio = (next_abs / durability_max).clamp(0.0, 1.0);
                            if next_ratio < cur_ratio {
                                let broke_now = next_ratio <= 0.0 && cur_ratio > 0.0;
                                match set_item_instance_durability(
                                    &mut inventory,
                                    instance_id,
                                    next_ratio,
                                ) {
                                    Ok(update) => {
                                        event_writers.durability_changed_tx.send(
                                            InventoryDurabilityChangedEvent {
                                                entity: target_entity,
                                                revision: update.revision,
                                                instance_id: update.instance_id,
                                                durability: update.durability,
                                            },
                                        );
                                        if broke_now {
                                            if let Some(audio_events) =
                                                event_writers.audio_events.as_deref_mut()
                                            {
                                                audio_events.send(PlaySoundRecipeRequest {
                                                    recipe_id: "armor_break".to_string(),
                                                    instance_id: 0,
                                                    pos: None,
                                                    flag: None,
                                                    volume_mul: 1.0,
                                                    pitch_shift: 0.0,
                                                    recipient: AudioRecipient::Radius {
                                                        origin: target_position,
                                                        radius: AUDIO_BROADCAST_RADIUS,
                                                    },
                                                });
                                            }
                                        }
                                    }
                                    Err(error) => {
                                        tracing::warn!(
                                            "[bong][combat][armor] failed to persist durability for instance {}: {}",
                                            instance_id,
                                            error
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        // plan-race-system-v1 P0 review 修复（BLOCKING-2）—— dandao 变异（如脊突
        // SpineSpurs 的背部 DamageReduction）经 `mutation_slot_mapping` 解析出的
        // 目标实体部位，与丹药/状态效果驱动的 `body_part_damage_multiplier` 同一处
        // 叠乘生效；`target_body_plan` 已在本轮命中判定时按目标实体解析（见上方
        // "raycast_humanoid 命中几何按目标实体分派"注释）。
        let defender_mutation_state = mutation_states.get(target_entity).ok();
        // humanoid-only boundary（P0 决议，本轮不迁移）：丹药 BodyPartResist/BodyPartWeaken
        // 状态效果（`combat::status::body_part_damage_multiplier`）与 dandao 变异伤害倍率
        // （`dandao::mutation::mutation_damage_multiplier_for_part`）两个函数都仍以 legacy
        // `BodyPart` 为参数类型（P1 批次范围）；非人形部位 id 没有 legacy 对应物时，视为
        // "这两套系统都不对该部位生效"，中性倍率 1.0（显式，不是静默吞掉）。
        let pill_part_multiplier = crate::body_plan::id_to_legacy_body_part(&hit_probe.part_id)
            .map(|legacy_part| {
                body_part_damage_multiplier(defender_status_effects, legacy_part)
                    * crate::dandao::mutation::mutation_damage_multiplier_for_part(
                        defender_mutation_state,
                        target_body_plan,
                        legacy_part,
                    )
            })
            .unwrap_or(1.0);
        if (pill_part_multiplier - 1.0).abs() > f32::EPSILON {
            wound.severity *= pill_part_multiplier;
            wound.bleeding_per_sec *= pill_part_multiplier;
            emitted_contam_delta *= f64::from(pill_part_multiplier);
        }

        let false_skin_kind_before = false_skin.as_ref().map(|skin| skin.kind);
        let filter_result = tuike_filter_contam(emitted_contam_delta, false_skin.as_deref_mut());
        let layers_remaining = false_skin
            .as_ref()
            .map(|skin| skin.layers_remaining)
            .unwrap_or(0);
        if let Some(attrs) = defender_attrs.as_deref_mut() {
            attrs.tuike_layers = layers_remaining;
        }
        if filter_result.shed_layers > 0 {
            if let Some(kind) = false_skin_kind_before {
                if let Some(events) = shed_events.as_deref_mut() {
                    events.send(ShedEvent {
                        target: target_entity,
                        attacker: Some(intent.attacker),
                        target_id: target_id.clone(),
                        attacker_id: Some(attacker_id.clone()),
                        kind,
                        layers_shed: filter_result.shed_layers,
                        layers_remaining,
                        contam_absorbed: filter_result.contam_absorbed,
                        contam_overflow: filter_result.passes_through,
                        tick: clock.tick,
                    });
                }
            }
        }
        if filter_result.depleted && false_skin_kind_before.is_some() {
            commands.entity(target_entity).remove::<FalseSkin>();
            if let Ok(mut inventory) = inventories.get_mut(target_entity) {
                if let Some(item) = inventory.equipped.get(EQUIP_SLOT_CHEST).and_then(|s| {
                    s.worn.iter().rev().find(|i| {
                        crate::combat::tuike::false_skin_kind_for_item(&i.template_id).is_some()
                    })
                }) {
                    let instance_id = item.instance_id;
                    let _ = consume_item_instance_once(&mut inventory, instance_id);
                }
            }
        }
        emitted_contam_delta = filter_result.passes_through;
        // plan-baomai-v4 P0 — 死脉甲免疫区拦截污染。
        // 守恒决议：drop_no_release。污染量由伤害公式凭空派生（非攻方真元转移），
        // 拦截后直接丢弃，不调用 qi_release_to_zone（否则通胀）。
        // humanoid-only boundary（P0 决议，本轮不迁移）：`DeadMeridianArmor.immune_regions`
        // 仍以 legacy `BodyPart` 为键（baomai_v4 主动绝脉机制本轮不迁移，P1 批次范围）；
        // 非人形部位 id 没有 legacy 对应物时，视为"未被授予免疫"（false），显式短路而非
        // panic/静默吞掉。
        let dead_armor_blocks =
            dead_armor_q
                .get(target_entity)
                .ok()
                .flatten()
                .is_some_and(|armor| {
                    crate::body_plan::id_to_legacy_body_part(&hit_probe.part_id)
                        .is_some_and(|legacy_part| should_block_contamination(armor, legacy_part))
                });
        if dead_armor_blocks {
            emitted_contam_delta = 0.0;
        }
        if emitted_contam_delta > 0.0 {
            contamination.entries.push(ContamSource {
                amount: emitted_contam_delta,
                color: ColorKind::Mellow,
                // plan-race-system-v1 P6b review BLOCKER 收口：经脉污染路由改走通用
                // `body_plan::dugu_injection_channel(target_body_plan, body_part)`（不再
                // 固定读 legacy `dugu::body_part_to_meridian` 私表），`ContamSource.meridian_id`
                // 本身已换轨为 `MeridianChannelId`（见其字段文档），直接持有解析出的
                // channel——不再经 `MeridianChannelId::to_meridian_id()` 把已经拿到的
                // 通用 channel 又压回 legacy `MeridianId` 枚举（换轨前的实现这么做会让
                // 非 humanoid 专属 channel，如 P5 飞鲸的 `tail_core`，必然找不到 legacy
                // 对应物而被强制归零成 `None`——专属 channel 因此实际不可被
                // `contamination_tick`/`resolve_crack_target` 消费，是本轮修的
                // BLOCKER）。humanoid 命中数值 bit-for-bit 不变（`MeridianChannelId` 的
                // snake_case 字符串就是 `humanoid.json dugu_injection` 表原样）。非
                // humanoid 目标（`target_body_plan` 未声明该部位映射）时仍是显式
                // `None`（污染仍计入总量，只是不挂靠某条经脉——显式，非静默吞掉）。
                meridian_id: crate::body_plan::dugu_injection_channel(
                    target_body_plan,
                    &hit_probe.part_id,
                ),
                attacker_id: Some(attacker_id.clone()),
                introduced_at: clock.tick,
            });
        }

        wounds.health_current =
            (wounds.health_current - wound.severity).clamp(0.0, wounds.health_max);
        let wound_bleeding = wound.bleeding_per_sec;
        let wound_severity = wound.severity;
        wounds.entries.push(wound);

        if wound_bleeding > 0.0 {
            event_writers
                .status_effect_intents
                .send(ApplyStatusEffectIntent {
                    target: target_entity,
                    kind: StatusEffectKind::Bleeding,
                    magnitude: wound_bleeding,
                    duration_ticks: u64::MAX,
                    issued_at_tick: clock.tick,
                });
        }

        // plan-race-system-v1 P0 review r2（BLOCKING-2 收口）—— 断臂脱手 / 腿伤减速 /
        // 头伤眩晕三条部位功能性后果统一改为按**目标实体解析出的 `target_body_plan`**
        // 查询命中部位的 `PartConsequence`（`Manipulator{main_hand}` / `Locomotion` /
        // `Sensory`），取代此前分别反压三次 legacy `BodyPart`（`MAIN_ARM` 字面量比较 /
        // `LegL|LegR` 匹配 / `Head` 比较）的写法——任意 `BodyPlan`（含非人形构型，如
        // 未来 whale 的 `tail_fin`=Locomotion）都能触发正确的功能性后果，不再要求
        // "命中部位必须是这 8 个 legacy 变体之一"这个人形专属前提。**决策**（该做
        // 什么）与**副作用**（怎么做）拆成两步：[`dispatch_part_consequence`] 是纯
        // 函数，直接单测锁死"未知 part id 显式无后果"这条在当前几何架构下无法通过
        // 完整攻击管线触达的分支（`hit_probe.part_id` 恒来自同一份已校验 plan 的
        // `hit_geometry`，与 `plan.parts` 失配理论上不会发生——这是纵深防御，不是
        // 死代码，见该函数文档）。
        match dispatch_part_consequence(target_body_plan, &hit_probe.part_id, wound_severity) {
            PartConsequenceOutcome::SeverMainHandManipulator => {
                // plan-combat-hit-location-v1 P4（决议 §8.1 #2 Severed 行为级后果 #1 —
                // 消除 arm_wound::ArmWoundFactors.main_arm_severed 零消费孤岛）——
                // 主手臂本次命中直接判定为 Severed 分级 → 该侧持械立即脱手落地。
                // 与"武器耐久归零脱手"（本文件上方 broken_weapon 分支）刻意不同：耐久
                // 归零时武器仍完整，优先塞回随身容器；断臂时持械的手已经不在了，没有
                // "塞回背包"这个物理动作可言，直接走 dropped_loot 世界掉落（既有链路，
                // `discard_inventory_item_to_dropped_loot` + `DroppedLootRegistry`，与
                // §4664 剑招/耐久脱手同一套 API）。只处理 `EquipSlot::MainHand`：
                // `sync_weapon_component_from_equipped` 的选择顺序是
                // main_hand.held > off_hand.held，若目标此刻的 `Weapon` component
                // 追踪的其实是副手武器（主手空手/主手非武器），断主手臂不应误删副手件。
                if let Ok(severed_weapon) = weapons.get(target_entity) {
                    if severed_weapon.slot == EquipSlot::MainHand {
                        let dropped_instance_id = severed_weapon.instance_id;
                        if let Ok(mut target_inventory) = inventories.get_mut(target_entity) {
                            if let Some(dropped_loot_registry) = dropped_loot_registry.as_mut() {
                                match discard_inventory_item_to_dropped_loot(
                                    &mut target_inventory,
                                    dropped_loot_registry,
                                    [target_position.x, target_position.y, target_position.z],
                                    crate::world::dimension::DimensionKind::Overworld,
                                    dropped_instance_id,
                                    &InventoryLocationV1::Equip {
                                        slot: EquipSlotV1::MainHand,
                                        state: EquipStateV1::Held,
                                    },
                                ) {
                                    Ok(_) => {
                                        commands.entity(target_entity).remove::<Weapon>();
                                    }
                                    Err(drop_error) => {
                                        tracing::warn!(
                                            "[bong][combat][arm_wound] main arm severed but failed to drop weapon instance {} for target: {}",
                                            dropped_instance_id,
                                            drop_error
                                        );
                                    }
                                }
                            } else {
                                tracing::warn!(
                                    "[bong][combat][arm_wound] main arm severed weapon instance {} cannot fall back to dropped loot because DroppedLootRegistry is unavailable",
                                    dropped_instance_id
                                );
                            }
                        }
                    }
                }
            }
            PartConsequenceOutcome::ApplyLegSlow => {
                event_writers
                    .status_effect_intents
                    .send(ApplyStatusEffectIntent {
                        target: target_entity,
                        kind: StatusEffectKind::Slowed,
                        magnitude: 0.4,
                        duration_ticks: LEG_SLOWED_DURATION_TICKS,
                        issued_at_tick: clock.tick,
                    });
                // plan-combat-hit-location-v1 P3 — 腿伤减速触发时目标脚下血渍 decal
                // （复用 client BongGroundDecalParticle 基类，lifetime 100t，无新贴图）。
                if let Some(events) = event_writers.vfx_events.as_deref_mut() {
                    gameplay_vfx::send_spawn(
                        events,
                        gameplay_vfx::spawn_request(
                            gameplay_vfx::COMBAT_LEG_WOUND_DECAL,
                            target_position,
                            None,
                            "#8C1F1F",
                            (wound_severity / 20.0).clamp(0.3, 1.0),
                            1,
                            100,
                        ),
                    );
                }
            }
            PartConsequenceOutcome::ApplyHeadStun => {
                event_writers
                    .status_effect_intents
                    .send(ApplyStatusEffectIntent {
                        target: target_entity,
                        kind: StatusEffectKind::Stunned,
                        magnitude: 1.0,
                        duration_ticks: HEAD_STUN_DURATION_TICKS,
                        issued_at_tick: clock.tick,
                    });
            }
            PartConsequenceOutcome::NoConsequence => {
                // 已知 consequence，但本次命中无外部可观察后果（Core 命中 / 未达阈值的
                // Locomotion·Sensory / Manipulator{main_hand:false} / 未达 Severed 的
                // Manipulator{main_hand:true}）——显式空分支，不是遗漏。
            }
            PartConsequenceOutcome::UnknownPart => {
                // plan-race-system-v1 P0 review r2（BLOCKING-2 收口）—— 未知 part id
                // （命中部位不在 `target_body_plan.parts` 里，理论上不会发生：命中几何
                // 与部位定义同出一份 `BodyPlan`）：显式 warn + 无功能性后果，不静默吞掉。
                tracing::warn!(
                    "[bong][body_plan] resolve_attack_intents: hit part id {} not found in \
                     resolved BodyPlan {} parts — no locomotion/sensory/manipulator \
                     consequence dispatched (explicit no-op, not a silent skip)",
                    hit_probe.part_id,
                    target_body_plan.id
                );
            }
        }

        if !is_physical_hit {
            if let Some(primary_meridian) = first_open_or_fallback_meridian(&mut meridians) {
                primary_meridian.throughput_current += qi_invest * f64::from(decay);
                primary_meridian.cracks.push(MeridianCrack {
                    severity: f64::from(wound_severity) * 0.02 * wound_profile.crack_mul,
                    healing_progress: 0.0,
                    cause: CrackCause::Attack,
                    created_at: clock.tick,
                });
            }
        }

        if let Some(mut life_record) = life_record {
            life_record.push(BiographyEntry::CombatHit {
                attacker_id: attacker_id.clone(),
                // `BiographyEntry::CombatHit.body_part` 是自由格式 String（LifeRecord 持久化
                // 为不透明 JSON blob，无 schema 强绑定）——直接用 part_id 的 Display（如
                // "head"）而非 legacy Debug（如 "Head"），任意部位 id（含非人形）都能记录，
                // 不需要经过 legacy 转换。
                body_part: hit_probe.part_id.to_string(),
                wound_kind: format!("{:?}", intent.wound_kind),
                damage: wound_severity,
                tick: clock.tick,
            });
            if let Some(effectiveness) = jiemai_effectiveness_value {
                life_record.push(BiographyEntry::JiemaiParry {
                    attacker_id: attacker_id.clone(),
                    effectiveness,
                    tick: clock.tick,
                });
            }
        }

        let action_label = if intent.debug_command.is_some() {
            "debug_attack_intent"
        } else {
            attack_source_label(intent.source)
        };
        let qi_damage = if is_physical_hit { 0.0 } else { wound_severity };
        let physical_damage = if is_physical_hit { wound_severity } else { 0.0 };
        let description = format!(
            "{} {} -> {} hit {} with {:?} for {:.1} qi / {:.1} physical damage (hit_qi {:.1}, jiemai={} sword_parry={} shield_block={} eff={:.2}) at {:.2} reach decay",
            action_label,
            attacker_id,
            target_id,
            hit_probe.part_id,
            intent.wound_kind,
            qi_damage,
            physical_damage,
            hit_qi,
            jiemai_success,
            sword_parry_success,
            shield_block_success,
            jiemai_effectiveness_value
                .or(sword_parry_block_ratio)
                .or(shield_block_ratio)
                .unwrap_or(0.0),
            decay
        );

        event_writers.out_events.send(CombatEvent {
            attacker: intent.attacker,
            target: target_entity,
            resolved_at_tick: clock.tick,
            // humanoid-only boundary（P0 决议，边界①）：`CombatEvent.body_part`（走
            // Redis/JSON 的 `CombatBodyPartV1` 8 值枚举，`network::combat_bridge::
            // map_body_part`）本轮不开放化（P1 经脉/wire 批次范围）。非人形部位 id 没有
            // legacy 对应物时，`CombatBodyPartV1` 无法表达该部位——显式记 warn + 落一个
            // 占位值（`BodyPart::Chest`，与 dead_armor/dugu 等模块的躯干核心占位惯例
            // 一致），不是静默默认；wire 精确度的开放化留给 P1。
            //
            // bughunt minor：这条 warn 此前无节流——非人形 `PartBoxes` 构型一旦上线，
            // 每次命中该类目标都会打一条 warn，高频战斗场景（连续普攻/AOE）会刷屏。
            // 按 `combat::shield_block::shield_low_stamina_narration_tick` 的既有 tick
            // 取样节流惯例改成同一 tick 只警一次；只节流日志，返回值（占位
            // `BodyPart::Chest`）不受影响。
            body_part: crate::body_plan::id_to_legacy_body_part(&hit_probe.part_id).unwrap_or_else(
                || {
                    const UNMAPPED_BODY_PART_WARN_INTERVAL_TICKS: u64 = 80;
                    if clock
                        .tick
                        .is_multiple_of(UNMAPPED_BODY_PART_WARN_INTERVAL_TICKS)
                    {
                        tracing::warn!(
                            "[bong][body_plan] CombatEvent wire: part id {} has no legacy \
                             BodyPart mapping — CombatBodyPartV1 is humanoid-only until P1 \
                             opens it up; emitting BodyPart::Chest as an explicit placeholder \
                             (not a silent default); further occurrences within the next {} \
                             ticks are throttled",
                            hit_probe.part_id,
                            UNMAPPED_BODY_PART_WARN_INTERVAL_TICKS,
                        );
                    }
                    BodyPart::Chest
                },
            ),
            wound_kind: intent.wound_kind,
            source: intent.source,
            debug_command: intent.debug_command.is_some(),
            physical_damage,
            damage: qi_damage,
            contam_delta: emitted_contam_delta,
            description,
            defense_kind: if sword_parry_success {
                Some(DefenseKind::SwordParry)
            } else if shield_block_success {
                Some(DefenseKind::ShieldBlock)
            } else {
                jiemai_success.then_some(DefenseKind::JieMai)
            },
            defense_effectiveness: jiemai_effectiveness_value
                .or(sword_parry_block_ratio)
                .or(shield_block_ratio),
            defense_contam_reduced: jiemai_contam_reduced
                .or(sword_parry_contam_reduced)
                .or(shield_block_contam_reduced),
            defense_wound_severity: jiemai_wound_severity
                .or(sword_parry_reflected_damage)
                .or(shield_blocked_damage),
        });
        if let Some(events) = event_writers.vfx_events.as_deref_mut() {
            let hit_origin = target_position + DVec3::new(0.0, 1.0, 0.0);
            let hit_dir = [
                target_position.x - attacker_position.x,
                target_position.y - attacker_position.y,
                target_position.z - attacker_position.z,
            ];
            let hit_len =
                (hit_dir[0] * hit_dir[0] + hit_dir[1] * hit_dir[1] + hit_dir[2] * hit_dir[2])
                    .sqrt();
            let hit_dir = if hit_len > 1e-6 {
                [
                    hit_dir[0] / hit_len,
                    hit_dir[1] / hit_len,
                    hit_dir[2] / hit_len,
                ]
            } else {
                [0.0, 0.0, 0.0]
            };
            // plan-combat-hit-location-v1 P3 — 部位差异视听反馈：头部命中暴击星形 burst，
            // 四肢命中血色三线沿命中法线；胸/腹/背命中维持既有 COMBAT_HIT 不变。
            // humanoid-only boundary（P0 决议，本轮不迁移）：命中 VFX 分级本身按 legacy
            // `BodyPart` 三档分类（crit/limb/torso），非人形部位 id 没有 legacy 对应物时
            // 落回 torso 档默认 `COMBAT_HIT`（既不算 crit 也不算 limb 特化）——显式兜底，
            // 不是遗漏分支。
            match crate::body_plan::id_to_legacy_body_part(&hit_probe.part_id) {
                Some(BodyPart::Head) => {
                    gameplay_vfx::send_spawn(
                        events,
                        gameplay_vfx::spawn_request(
                            gameplay_vfx::COMBAT_HIT_HEAD_CRIT,
                            hit_origin,
                            Some(hit_dir),
                            "#FFE9A0",
                            (wound_severity / 20.0).clamp(0.25, 1.0),
                            6,
                            8,
                        ),
                    );
                }
                Some(BodyPart::ArmL | BodyPart::ArmR | BodyPart::LegL | BodyPart::LegR) => {
                    gameplay_vfx::send_spawn(
                        events,
                        gameplay_vfx::spawn_request(
                            gameplay_vfx::COMBAT_HIT_LIMB,
                            hit_origin,
                            Some(hit_dir),
                            "#8C1F1F",
                            (wound_severity / 20.0).clamp(0.25, 1.0),
                            3,
                            6,
                        ),
                    );
                }
                Some(BodyPart::Chest | BodyPart::Abdomen | BodyPart::Back) | None => {
                    gameplay_vfx::send_spawn(
                        events,
                        gameplay_vfx::spawn_request(
                            gameplay_vfx::COMBAT_HIT,
                            hit_origin,
                            Some(hit_dir),
                            "#FF3344",
                            (wound_severity / 20.0).clamp(0.25, 1.0),
                            6,
                            12,
                        ),
                    );
                }
            }
            if jiemai_success || sword_parry_success || shield_block_success {
                gameplay_vfx::send_spawn(
                    events,
                    gameplay_vfx::spawn_request(
                        gameplay_vfx::COMBAT_PARRY,
                        hit_origin,
                        Some([-hit_dir[0], -hit_dir[1], -hit_dir[2]]),
                        if sword_parry_success {
                            "#FFD080"
                        } else if shield_block_success {
                            "#A0C8FF"
                        } else {
                            "#4488FF"
                        },
                        jiemai_effectiveness_value
                            .or(sword_parry_block_ratio)
                            .or(shield_block_ratio)
                            .unwrap_or(0.6)
                            .clamp(0.3, 1.0),
                        8,
                        16,
                    ),
                );
            }
        }

        if let Some(active_events) = active_events.as_deref_mut() {
            active_events.record_recent_event(GameEvent {
                event_type: GameEventType::EventTriggered,
                tick: clock.tick,
                player: Some(attacker_id.clone()),
                target: Some(target_id),
                zone: None,
                details: Some(std::collections::HashMap::from([
                    ("action".to_string(), json!(action_label)),
                    (
                        // GameEvent.details 是自由格式 JSON map，直接用 part_id 的原始
                        // 字符串（如 "head"），任意部位 id（含非人形）都能记录。
                        "body_part".to_string(),
                        json!(hit_probe.part_id.as_str()),
                    ),
                    (
                        "wound_kind".to_string(),
                        json!(format!("{:?}", intent.wound_kind)),
                    ),
                    ("damage".to_string(), json!(wound_severity)),
                    ("physical_damage".to_string(), json!(physical_damage)),
                    ("contam_delta".to_string(), json!(emitted_contam_delta)),
                    ("qi_invest".to_string(), json!(intent.qi_invest)),
                    ("hit_qi".to_string(), json!(hit_qi)),
                    ("jiemai_success".to_string(), json!(jiemai_success)),
                    (
                        "sword_parry_success".to_string(),
                        json!(sword_parry_success),
                    ),
                    (
                        "jiemai_effectiveness".to_string(),
                        json!(jiemai_effectiveness_value),
                    ),
                    (
                        "jiemai_contam_reduced".to_string(),
                        json!(jiemai_contam_reduced),
                    ),
                    (
                        "jiemai_wound_severity".to_string(),
                        json!(jiemai_wound_severity),
                    ),
                    ("reach_decay".to_string(), json!(decay)),
                ])),
            });
        }

        let active_sparring = crate::social::active_sparring_between(
            &sparring_sessions,
            intent.attacker,
            target_entity,
        );
        if let Some(sparring) = active_sparring.as_ref() {
            if clock.tick <= sparring.expires_at_tick && was_alive && wounds.health_current <= 0.0 {
                wounds.health_current = (wounds.health_max.max(1.0) * 0.05).max(1.0);
                crate::social::conclude_sparring_defeat(
                    &mut commands,
                    &mut event_writers.status_effect_intents,
                    target_entity,
                    intent.attacker,
                    clock.tick,
                );
                continue;
            }
        }

        if was_alive
            && wounds.health_current <= 0.0
            && !lifecycle.is_some_and(|lifecycle| {
                matches!(
                    lifecycle.state,
                    LifecycleState::NearDeath | LifecycleState::Terminated
                )
            })
        {
            // plan-tsy-loot-v1 §6 — 攻击链路：attacker entity 来自 intent；
            // attacker_player_id 仅在攻击者是 player 时填（canonical id 形如
            // "offline:Foo"），NPC 攻击者保留 None。
            let attacker_player_id = attacker_id
                .starts_with("offline:")
                .then(|| attacker_id.clone());
            event_writers.death_events.send(DeathEvent {
                target: target_entity,
                cause: format!("{action_label}:{attacker_id}"),
                attacker: Some(intent.attacker),
                attacker_player_id,
                at_tick: clock.tick,
            });
            if let (true, Some(skill_xp_events)) = (
                attacker_id.starts_with("offline:"),
                skill_xp_events.as_deref_mut(),
            ) {
                skill_xp_events.send(SkillXpGain {
                    char_entity: intent.attacker,
                    skill: SkillId::Combat,
                    amount: 4,
                    source: XpGainSource::Action {
                        plan_id: "combat",
                        action: "kill_npc",
                    },
                });
            }
        }
    }
}

fn attack_source_label(source: AttackSource) -> &'static str {
    match source {
        AttackSource::Melee => "attack_intent",
        AttackSource::NpcMelee => "npc_melee_attack",
        AttackSource::BurstMeridian => "burst_meridian_attack",
        AttackSource::QiNeedle => "qi_needle",
        AttackSource::FullPower => "full_power_strike",
        AttackSource::SwordCleave => "sword_cleave",
        AttackSource::SwordThrust => "sword_thrust",
        AttackSource::SwordPathCondenseEdge => "sword_path_condense_edge",
        AttackSource::SwordPathQiSlash => "sword_path_qi_slash",
        AttackSource::SwordPathResonance => "sword_path_resonance",
        AttackSource::SwordPathManifest => "sword_path_manifest",
        AttackSource::SwordPathHeavenGate => "sword_path_heaven_gate",
    }
}

fn source_uses_prepaid_qi(source: AttackSource) -> bool {
    matches!(
        source,
        AttackSource::BurstMeridian
            | AttackSource::FullPower
            | AttackSource::SwordCleave
            | AttackSource::SwordThrust
            // plan-sword-path-v2 §P1: 五招都在 cast 阶段通过 apply_cast_costs 已扣
            // 真元 / 体力。若漏掉 prepaid 白名单，resolver 的反作弊会因
            // qi_invest > qi_current 拒绝结算（QiInvestExceeded），结果是所有
            // sword_path 攻击都被错误拦截。
            | AttackSource::SwordPathCondenseEdge
            | AttackSource::SwordPathQiSlash
            | AttackSource::SwordPathResonance
            | AttackSource::SwordPathManifest
            | AttackSource::SwordPathHeavenGate
            // bug-hunt-1: QiNeedle 在 cast 阶段（needle.rs:87）已无条件预扣
            // QI_NEEDLE_QI_COST(=1.0) 自 qi_current，与 BurstMeridian 同构地 emit
            // AttackIntent{qi_invest:1.0}。漏掉 prepaid 白名单会让 resolver 再扣一次
            // qi_invest(=1.0)，每发气针命中后净扣 2.0 真元（double-spend）。
            | AttackSource::QiNeedle
            // bug: Daoxiang TSY NPCs have Cultivation{qi_current:0.0, qi_max:10.0} by
            // default (NpcRuntimeBundle), but emit AttackIntent{qi_invest:25.0}.
            // Spirit-qi < -0.4 in TSY zones means the regen branch never fires, so
            // qi_current stays 0.0 permanently. qi_max=10.0 < qi_invest=25.0 means the
            // gate can never be cleared. NPC attacks are server-side-authoritative and
            // need no player qi conservation accounting.
            | AttackSource::NpcMelee
    )
}

fn active_status_magnitude(
    statuses: Option<&StatusEffects>,
    kind: StatusEffectKind,
) -> Option<f32> {
    statuses?
        .active
        .iter()
        .find(|effect| effect.kind == kind && effect.remaining_ticks > 0)
        .map(|effect| effect.magnitude)
}

fn record_anticheat_violation(
    counter: Option<&mut AntiCheatCounter>,
    kind: ViolationKindV1,
    details: String,
) {
    let Some(counter) = counter else {
        return;
    };
    counter.record_violation(kind, details);
}

/// plan-race-system-v1 P0 review r2（BLOCKING-2 收口）—— 断臂脱手 / 腿伤减速 / 头伤
/// 眩晕三条部位功能性后果的**决策**（不含副作用：不 emit 事件、不碰 ECS World/
/// Commands）。拆成纯函数是为了让"命中部位 id 不在目标 plan.parts 里"这条分支能被
/// 直接单元测试锁住而不必构造一整条会触发几何求交失配的生产管线——在当前架构下，
/// `resolve_attack_intents` 里传入的 `part_id` 恒来自 `raycast_humanoid` 对同一份
/// `plan.hit_geometry` 的求交结果，与 `plan.parts` 失配理论上不会发生（
/// `validate_body_plan` 在 registry 加载期就拒绝 `PartBoxes`/`HeightBands` 引用悬空
/// 部位 id），这条分支是纵深防御，不是遗忘的死代码。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PartConsequenceOutcome {
    /// 命中部位是 `Manipulator{main_hand:true}` 且本次伤势判定为 `Severed`——目标该侧
    /// 持械应立即脱手落地。
    SeverMainHandManipulator,
    /// 命中部位是 `Locomotion` 且伤势达到 `LEG_SLOWED_SEVERITY_THRESHOLD`。
    ApplyLegSlow,
    /// 命中部位是 `Sensory` 且伤势达到 `HEAD_STUN_SEVERITY_THRESHOLD`。
    ApplyHeadStun,
    /// 命中部位有已知 `PartConsequence`，但本次命中不触发任何外部可观察后果——
    /// `Core` 命中 / 未达阈值的 `Locomotion`·`Sensory` / `Manipulator{main_hand:false}`
    /// / 未达 `Severed` 的 `Manipulator{main_hand:true}` 均落在这一档。
    NoConsequence,
    /// 命中部位 id 不在目标 `BodyPlan.parts` 里——显式未知，调用方必须 warn 而非静默。
    UnknownPart,
}

fn dispatch_part_consequence(
    plan: &crate::body_plan::BodyPlan,
    part_id: &crate::body_plan::BodyPartId,
    wound_severity: f32,
) -> PartConsequenceOutcome {
    match plan.consequence_for(part_id) {
        Some(crate::body_plan::PartConsequence::Manipulator { main_hand: true }) => {
            if arm_wound::is_severed(arm_wound::wound_severity_to_grade(wound_severity)) {
                PartConsequenceOutcome::SeverMainHandManipulator
            } else {
                PartConsequenceOutcome::NoConsequence
            }
        }
        // 副手臂断裂当前无独立"行为级"后果（脱手判定只认主手，副手断裂只通过
        // `arm_wound::combined_factor().block_multiplier` 影响格挡减伤）——显式归入
        // NoConsequence，不是遗漏。
        Some(crate::body_plan::PartConsequence::Manipulator { main_hand: false }) => {
            PartConsequenceOutcome::NoConsequence
        }
        Some(crate::body_plan::PartConsequence::Locomotion) => {
            if wound_severity >= LEG_SLOWED_SEVERITY_THRESHOLD {
                PartConsequenceOutcome::ApplyLegSlow
            } else {
                PartConsequenceOutcome::NoConsequence
            }
        }
        Some(crate::body_plan::PartConsequence::Sensory) => {
            if wound_severity >= HEAD_STUN_SEVERITY_THRESHOLD {
                PartConsequenceOutcome::ApplyHeadStun
            } else {
                PartConsequenceOutcome::NoConsequence
            }
        }
        // 躯干核心命中对"肢体功能性后果"（脱手/减速/眩晕）无影响——显式归入
        // NoConsequence，不是遗漏。
        Some(crate::body_plan::PartConsequence::Core) => PartConsequenceOutcome::NoConsequence,
        None => PartConsequenceOutcome::UnknownPart,
    }
}

/// plan-race-system-v1 P0b —— 部位倍率不再是本文件的硬编 8 分支 match，改查目标实体
/// 解析出的 [`crate::body_plan::BodyPlan`]（经 [`resolve_body_plan`]）的
/// `BodyPartDef.{damage_mul,contam_mul,bleed_mul}`。查询链：
/// 1. `body_plans`/`races` 均存在（生产环境恒真，`body_plan::register()` 启动期装载）
///    → 走 `resolve_body_plan`（玩家走 `Cultivation.race`，见 `BodyPlanPurpose::Intrinsic`
///    语义）；解析成功即用其结果查表。
/// 2. 解析失败（未知 race，理论上不会发生——`persistence` 层反序列化早已拒载未知
///    race）或任一资源缺失（大量既有单测未插入这两个资源，见 `weapon_break` 元组
///    P0b 注释）→ 退化到 [`humanoid_plan_static`]（与 registry 加载同一份
///    `humanoid.json`，数值 bit-for-bit 相同，不是第二份硬编码表）。
///
/// P0b 未在 `CombatTargetItem` 查询里加 `Option<&BeastKind>`（该 15 元素元组已逼近
/// Bevy `WorldQuery` 元组 16 元素上限，见 `resolve_attack_intents` 顶层参数同款注释）——
/// 这在行为上是安全简化：`races.json` 现阶段所有 `BeastKind` 派生种族的
/// `body_plan_id` 均为 `"humanoid"`（P5 才会引入 whale 等非人形 plan），
/// `BodyPlanResolveInputs{beast_kind:None}` 落进 `resolve_body_plan` 的 Tier2/Tier3
/// 分支，与"真的查了 BeastKind"得到完全相同的 `humanoid` 解析结果——bit-for-bit
/// 不受影响。P5 若要给非人形 NPC 引入差异化命中倍率，需要在此处补上 BeastKind 查询
/// （届时 `CombatTargetItem` 元组可能需要拆分或改走 `#[derive(SystemParam)]` 结构体）。
fn body_part_multipliers(
    target_entity: Entity,
    defender_cultivation: Option<&Cultivation>,
    body_plans: Option<&BodyPlanRegistry>,
    races: Option<&RaceRegistry>,
    part_id: &crate::body_plan::BodyPartId,
) -> (f32, f32, f32) {
    let plan = match (body_plans, races) {
        (Some(body_plans), Some(races)) => match resolve_body_plan(
            target_entity,
            BodyPlanPurpose::Intrinsic,
            BodyPlanResolveInputs {
                cultivation: defender_cultivation,
                beast_kind: None,
                morph_state: None,
            },
            body_plans,
            races,
        ) {
            Ok(plan) => plan,
            Err(error) => {
                tracing::error!(
                    "[bong][body_plan] body_part_multipliers: {error} — falling back to humanoid"
                );
                humanoid_plan_static()
            }
        },
        _ => humanoid_plan_static(),
    };

    plan.parts
        .iter()
        .find(|def| &def.id == part_id)
        .map(|def| (def.damage_mul, def.contam_mul, def.bleed_mul))
        .unwrap_or_else(|| {
            tracing::error!(
                "[bong][body_plan] body plan {} has no part definition for part id {part_id} \
                 — using neutral 1.0 multipliers",
                plan.id
            );
            (1.0, 1.0, 1.0)
        })
}

/// P0b 之前的硬编 8 分支 match（保留供 `legacy_body_part_multipliers_matches_data_driven_defaults`
/// pin 测试逐项对拍，证明数据驱动路径与旧表 bit-for-bit 一致；生产代码不再调用）。
#[cfg(test)]
fn legacy_body_part_multipliers(body_part: BodyPart) -> (f32, f32, f32) {
    match body_part {
        BodyPart::Head => (2.0, 1.5, 1.5),
        BodyPart::Chest => (1.0, 1.0, 1.0),
        BodyPart::Back => (0.9, 1.0, 1.0),
        BodyPart::Abdomen => (0.9, 1.2, 1.3),
        BodyPart::ArmL | BodyPart::ArmR => (0.7, 0.8, 0.8),
        BodyPart::LegL | BodyPart::LegR => (0.6, 0.7, 1.0),
    }
}

fn wound_kind_profile(kind: crate::combat::components::WoundKind) -> WoundKindProfile {
    match kind {
        crate::combat::components::WoundKind::Cut => WoundKindProfile {
            damage_mul: 1.0,
            bleed_mul: 1.4,
            contam_mul: 1.0,
            crack_mul: 1.0,
        },
        crate::combat::components::WoundKind::Blunt => WoundKindProfile {
            damage_mul: 1.0,
            bleed_mul: 0.7,
            contam_mul: 0.8,
            crack_mul: 1.3,
        },
        crate::combat::components::WoundKind::Pierce => WoundKindProfile {
            damage_mul: 1.0,
            bleed_mul: 1.0,
            contam_mul: 1.2,
            crack_mul: 1.1,
        },
        crate::combat::components::WoundKind::Burn => WoundKindProfile {
            damage_mul: 1.0,
            bleed_mul: 0.2,
            contam_mul: 1.3,
            crack_mul: 0.7,
        },
        crate::combat::components::WoundKind::Concussion => WoundKindProfile {
            damage_mul: 1.0,
            bleed_mul: 0.1,
            contam_mul: 0.6,
            crack_mul: 1.4,
        },
    }
}

type ResolvedIntent = (DVec3, String, Entity, DVec3, String);

fn resolve_intent_entities(
    intent: &AttackIntent,
    clients: &Query<CombatClientItem<'_>, CombatClientFilter>,
    positions: &Query<PositionLookItem<'_>>,
    npc_markers: &Query<(), With<NpcMarker>>,
    npc_positions: &Query<(Entity, &Position), With<NpcMarker>>,
) -> Option<ResolvedIntent> {
    let (attacker_position, attacker_id) =
        resolve_combat_actor(intent.attacker, clients, positions, npc_markers)?;

    if let Some(action) = intent.debug_command.as_ref() {
        let (target_entity, target_position, _target_hint_qi_max, target_id) =
            resolve_debug_target(
                intent,
                action,
                clients,
                positions,
                npc_markers,
                npc_positions,
            )?;
        return Some((
            attacker_position,
            attacker_id,
            target_entity,
            target_position,
            target_id,
        ));
    }

    let target_entity = intent.target?;
    if target_entity == intent.attacker {
        return None;
    }
    let (target_position, target_id) =
        resolve_combat_actor(target_entity, clients, positions, npc_markers)?;
    Some((
        attacker_position,
        attacker_id,
        target_entity,
        target_position,
        target_id,
    ))
}

fn resolve_combat_actor(
    entity: Entity,
    clients: &Query<CombatClientItem<'_>, CombatClientFilter>,
    positions: &Query<PositionLookItem<'_>>,
    npc_markers: &Query<(), With<NpcMarker>>,
) -> Option<(DVec3, String)> {
    if let Ok((_, position, username, _)) = clients.get(entity) {
        return Some((position.get(), canonical_player_id(username.0.as_str())));
    }
    if npc_markers.get(entity).is_ok() {
        let position = positions.get(entity).ok()?.0.get();
        return Some((position, canonical_npc_id(entity)));
    }
    None
}

fn resolve_debug_target(
    intent: &AttackIntent,
    action: &crate::player::gameplay::CombatAction,
    clients: &Query<CombatClientItem<'_>, CombatClientFilter>,
    positions: &Query<PositionLookItem<'_>>,
    npc_markers: &Query<(), With<NpcMarker>>,
    npc_positions: &Query<(Entity, &Position), With<NpcMarker>>,
) -> Option<(Entity, DVec3, f64, String)> {
    if let Some(target) = intent.target {
        if let Ok((_, position, username, _player_state)) = clients.get(target) {
            return Some((
                target,
                position.get(),
                0.0,
                canonical_player_id(username.0.as_str()),
            ));
        }

        if npc_markers.get(target).is_ok() {
            let position = positions.get(target).ok()?.0.get();
            return Some((target, position, 0.0, canonical_npc_id(target)));
        }

        return None;
    }

    let target_name = action.target.trim();
    if target_name.is_empty() {
        return None;
    }

    if let Some(player_match) =
        clients
            .iter()
            .find_map(|(entity, position, username, _player_state)| {
                if entity == intent.attacker {
                    return None;
                }

                let canonical = canonical_player_id(username.0.as_str());
                (username.0.eq_ignore_ascii_case(target_name)
                    || canonical.eq_ignore_ascii_case(target_name))
                .then_some((entity, position.get(), 0.0, canonical))
            })
    {
        return Some(player_match);
    }

    npc_positions.iter().find_map(|(entity, position)| {
        if entity == intent.attacker {
            return None;
        }

        let canonical = canonical_npc_id(entity);
        canonical.eq_ignore_ascii_case(target_name).then_some((
            entity,
            position.get(),
            0.0,
            canonical,
        ))
    })
}

fn first_open_or_fallback_meridian(
    meridians: &mut MeridianSystem,
) -> Option<&mut crate::cultivation::components::Meridian> {
    if let Some(index) = meridians
        .regular
        .iter()
        .position(|meridian| meridian.opened)
    {
        return meridians.regular.get_mut(index);
    }

    meridians.regular.get_mut(0)
}

#[cfg(test)]
#[path = "resolve_tests.rs"]
mod tests;
