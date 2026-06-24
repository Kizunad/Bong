use valence::prelude::{
    bevy_ecs, bevy_ecs::system::SystemParam, Component, DVec3, Entity, Events, Position, Query,
    Res, ResMut, UniqueId,
};

use crate::combat::components::{
    CastSource, Casting, SkillBarBindings, Stamina, StaminaState, StatusEffects, WoundKind,
    TICKS_PER_SECOND,
};
use crate::combat::events::{AttackIntent, AttackReach, AttackSource, StatusEffectKind};
use crate::combat::status::{has_active_status, upsert_status_effect};
use crate::combat::weapon::{Weapon, WeaponKind};
use crate::combat::CombatClock;
use crate::cultivation::components::{ColorKind, Cultivation, QiColor, Realm};
use crate::cultivation::known_techniques::KnownTechniques;
use crate::cultivation::life_record::LifeRecord;
use crate::cultivation::meridian::severed::SkillMeridianDependencies;
use crate::cultivation::skill_registry::{CastRejectReason, CastResult, SkillRegistry};
use crate::network::audio_event_emit::{
    AudioRecipient, PlaySoundRecipeRequest, AUDIO_BROADCAST_RADIUS,
};
use crate::network::cast_emit::current_unix_millis;
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::qi_physics::{
    constants::{QI_EPSILON, QI_ZONE_UNIT_CAPACITY},
    qi_excretion_loss,
    release::qi_release_to_zone,
    ContainerKind, EnvField, QiAccountId, QiTransfer, QiTransferReason,
};
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};

pub const SWORD_CLEAVE_SKILL_ID: &str = "sword.cleave";
pub const SWORD_THRUST_SKILL_ID: &str = "sword.thrust";
pub const SWORD_PARRY_SKILL_ID: &str = "sword.parry";
pub const SWORD_INFUSE_SKILL_ID: &str = "sword.infuse";

const SWORD_INFUSE_MIN_QI: f64 = 5.0;
const SWORD_INFUSE_MAX_FRACTION: f64 = 0.5;
const SWORD_INFUSE_HITS: f64 = 5.0;
const SWORD_INFUSE_DURATION_TICKS: u64 = 60 * TICKS_PER_SECOND;
const SWORD_QI_STORE_TICK_INTERVAL: u64 = TICKS_PER_SECOND;
pub const SWORD_PARRY_STAGGER_TICKS: u64 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwordTechnique {
    Cleave,
    Thrust,
    Parry,
    Infuse,
}

impl SwordTechnique {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Cleave => SWORD_CLEAVE_SKILL_ID,
            Self::Thrust => SWORD_THRUST_SKILL_ID,
            Self::Parry => SWORD_PARRY_SKILL_ID,
            Self::Infuse => SWORD_INFUSE_SKILL_ID,
        }
    }

    const fn base_stamina_cost(self) -> f32 {
        match self {
            Self::Cleave => 8.0,
            Self::Thrust => 4.0,
            Self::Parry => 6.0,
            Self::Infuse => 3.0,
        }
    }
}

#[derive(Debug, Clone, Component, PartialEq)]
pub struct SwordQiStore {
    pub stored_qi: f64,
    pub qi_per_hit: f64,
    pub remaining_ticks: u64,
    pub infuser_color: ColorKind,
    pub weapon_instance_id: u64,
    pub container_account: QiAccountId,
    pub carrier: ContainerKind,
}

#[derive(Debug, Clone, Component, PartialEq)]
pub struct PendingSwordInfuse {
    pub amount: f64,
    pub complete_at_tick: u64,
    pub slot: u8,
    pub weapon_instance_id: u64,
    pub carrier: ContainerKind,
    pub infuser_color: ColorKind,
    pub container_account: QiAccountId,
}

#[derive(SystemParam)]
pub struct SwordInfuseCompletionParams<'w, 's> {
    commands: valence::prelude::Commands<'w, 's>,
    pending: Query<'w, 's, (Entity, &'static PendingSwordInfuse)>,
    weapons: Query<'w, 's, &'static Weapon>,
    cultivations: Query<'w, 's, &'static mut Cultivation>,
    positions: Query<'w, 's, &'static Position>,
    unique_ids: Query<'w, 's, &'static UniqueId>,
    qi_transfers: Option<ResMut<'w, Events<QiTransfer>>>,
    vfx_events: Option<ResMut<'w, Events<VfxEventRequest>>>,
    audio_events: Option<ResMut<'w, Events<PlaySoundRecipeRequest>>>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SwordTechniqueProfile {
    pub stamina_cost: f32,
    pub cast_ticks: u32,
    pub cooldown_ticks: u64,
    pub range: f32,
    pub damage_multiplier: f32,
    pub parry_window_ticks: u64,
    pub block_ratio: f32,
}

pub fn register_skills(registry: &mut SkillRegistry) {
    registry.register(SWORD_CLEAVE_SKILL_ID, cast_sword_cleave);
    registry.register(SWORD_THRUST_SKILL_ID, cast_sword_thrust);
    registry.register(SWORD_PARRY_SKILL_ID, cast_sword_parry);
    registry.register(SWORD_INFUSE_SKILL_ID, cast_sword_infuse);
}

pub fn declare_meridian_dependencies(dependencies: &mut SkillMeridianDependencies) {
    for id in [
        SWORD_CLEAVE_SKILL_ID,
        SWORD_THRUST_SKILL_ID,
        SWORD_PARRY_SKILL_ID,
        SWORD_INFUSE_SKILL_ID,
    ] {
        dependencies.declare(id, Vec::new());
    }
}

pub fn sword_profile(technique: SwordTechnique, proficiency: f32) -> SwordTechniqueProfile {
    let prof = proficiency.clamp(0.0, 1.0);
    match technique {
        SwordTechnique::Cleave => SwordTechniqueProfile {
            stamina_cost: lerp(8.0, 5.0, prof),
            cast_ticks: lerp_round(16.0, 10.0, prof),
            cooldown_ticks: u64::from(lerp_round(30.0, 22.0, prof)),
            range: 3.0,
            damage_multiplier: 1.0 + prof * 0.3,
            parry_window_ticks: 0,
            block_ratio: 0.0,
        },
        SwordTechnique::Thrust => SwordTechniqueProfile {
            stamina_cost: lerp(4.0, 2.0, prof),
            cast_ticks: lerp_round(10.0, 7.0, prof),
            cooldown_ticks: u64::from(lerp_round(20.0, 14.0, prof)),
            range: lerp(3.5, 4.0, prof),
            damage_multiplier: 0.75 + prof * 0.19,
            parry_window_ticks: 0,
            block_ratio: 0.0,
        },
        SwordTechnique::Parry => SwordTechniqueProfile {
            stamina_cost: lerp(6.0, 4.0, prof),
            cast_ticks: 4,
            cooldown_ticks: u64::from(lerp_round(40.0, 30.0, prof)),
            range: 0.0,
            damage_multiplier: 0.0,
            parry_window_ticks: 4 + (prof * 4.0).floor() as u64,
            block_ratio: 0.3 + prof * 0.3,
        },
        SwordTechnique::Infuse => SwordTechniqueProfile {
            stamina_cost: SwordTechnique::Infuse.base_stamina_cost(),
            cast_ticks: 40,
            cooldown_ticks: 100,
            range: 0.0,
            damage_multiplier: 0.0,
            parry_window_ticks: 0,
            block_ratio: 0.0,
        },
    }
}

pub fn sword_proficiency_label(proficiency: f32) -> &'static str {
    let prof = proficiency.clamp(0.0, 1.0);
    if prof < 0.20 {
        "生疏"
    } else if prof < 0.50 {
        "入门"
    } else if prof < 0.80 {
        "熟练"
    } else if prof < 0.95 {
        "精通"
    } else {
        "化境"
    }
}

pub fn sword_proficiency_gain(current: f32, successful: bool, parry_bonus: bool) -> f32 {
    let current = current.clamp(0.0, 1.0);
    let base = if successful {
        if current < 0.50 {
            0.010
        } else if current < 0.80 {
            0.005
        } else if current < 0.95 {
            0.003
        } else {
            0.001
        }
    } else {
        0.002
    };
    if parry_bonus {
        base + 0.005
    } else {
        base
    }
}

pub fn is_sword_attack_source(source: AttackSource) -> bool {
    matches!(
        source,
        AttackSource::SwordCleave
            | AttackSource::SwordThrust
            | AttackSource::SwordPathCondenseEdge
            | AttackSource::SwordPathQiSlash
            | AttackSource::SwordPathResonance
            | AttackSource::SwordPathManifest
            | AttackSource::SwordPathHeavenGate
    )
}

pub fn source_to_technique(source: AttackSource) -> Option<SwordTechnique> {
    match source {
        AttackSource::SwordCleave => Some(SwordTechnique::Cleave),
        AttackSource::SwordThrust => Some(SwordTechnique::Thrust),
        _ => None,
    }
}

pub fn record_sword_parry_success(world: &mut bevy_ecs::world::World, defender: Entity) {
    apply_known_gain(world, defender, SwordTechnique::Parry, true, true);
}

pub fn track_sword_proficiency_from_hits(
    mut events: valence::prelude::EventReader<crate::combat::events::CombatEvent>,
    mut players: Query<&mut KnownTechniques>,
) {
    for event in events.read() {
        let Some(technique) = source_to_technique(event.source) else {
            continue;
        };
        if event.damage <= 0.0 && event.physical_damage <= 0.0 {
            continue;
        }
        let Ok(mut known) = players.get_mut(event.attacker) else {
            continue;
        };
        let Some(entry) = known
            .entries
            .iter_mut()
            .find(|entry| entry.id == technique.id())
        else {
            continue;
        };
        let gain = sword_proficiency_gain(entry.proficiency, true, false);
        entry.proficiency = (entry.proficiency + gain).clamp(0.0, 1.0);
    }
}

pub fn sword_qi_store_tick(
    clock: Res<CombatClock>,
    mut commands: valence::prelude::Commands,
    mut stores: Query<(Entity, &mut SwordQiStore)>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut qi_transfers: Option<ResMut<Events<QiTransfer>>>,
) {
    if !clock.tick.is_multiple_of(SWORD_QI_STORE_TICK_INTERVAL) {
        return;
    }
    for (entity, mut store) in &mut stores {
        if store.remaining_ticks == 0 || store.stored_qi <= f64::EPSILON {
            // Flush any remaining stored_qi to zone before removal.
            let remainder = store.stored_qi;
            if remainder > f64::EPSILON {
                credit_qi_to_zone(
                    zones.as_deref_mut(),
                    qi_transfers.as_deref_mut(),
                    store.container_account.clone(),
                    remainder,
                    QiTransferReason::ReleaseToZone,
                );
            }
            commands.entity(entity).remove::<SwordQiStore>();
            continue;
        }
        let elapsed_secs = SWORD_QI_STORE_TICK_INTERVAL as f64 / TICKS_PER_SECOND as f64;
        let loss = qi_excretion_loss(
            store.stored_qi,
            store.carrier,
            elapsed_secs,
            EnvField::new(0.0),
        )
        .min(store.stored_qi);
        if loss > f64::EPSILON {
            store.stored_qi = (store.stored_qi - loss).max(0.0);
            credit_qi_to_zone(
                zones.as_deref_mut(),
                qi_transfers.as_deref_mut(),
                store.container_account.clone(),
                loss,
                QiTransferReason::Excretion,
            );
        }
        store.remaining_ticks = store
            .remaining_ticks
            .saturating_sub(SWORD_QI_STORE_TICK_INTERVAL);
        if store.remaining_ticks == 0 || store.stored_qi <= f64::EPSILON {
            // Flush remaining stored_qi to zone on expiry.
            let remainder = store.stored_qi;
            if remainder > f64::EPSILON {
                credit_qi_to_zone(
                    zones.as_deref_mut(),
                    qi_transfers.as_deref_mut(),
                    store.container_account.clone(),
                    remainder,
                    QiTransferReason::ReleaseToZone,
                );
                store.stored_qi = 0.0;
            }
            commands.entity(entity).remove::<SwordQiStore>();
        }
    }
}

pub fn sword_infuse_completion_tick(
    clock: Res<CombatClock>,
    mut params: SwordInfuseCompletionParams,
) {
    for (entity, pending) in &mut params.pending {
        if clock.tick < pending.complete_at_tick {
            continue;
        }
        let valid_weapon = params.weapons.get(entity).is_ok_and(|weapon| {
            weapon.weapon_kind == WeaponKind::Sword
                && weapon.instance_id == pending.weapon_instance_id
        });
        if !valid_weapon {
            params
                .commands
                .entity(entity)
                .remove::<PendingSwordInfuse>();
            continue;
        }
        let Ok(mut cultivation) = params.cultivations.get_mut(entity) else {
            params
                .commands
                .entity(entity)
                .remove::<PendingSwordInfuse>();
            continue;
        };
        if cultivation.qi_current + f64::EPSILON < pending.amount {
            params
                .commands
                .entity(entity)
                .remove::<PendingSwordInfuse>();
            continue;
        }
        cultivation.qi_current =
            (cultivation.qi_current - pending.amount).clamp(0.0, cultivation.qi_max);
        emit_qi_transfer(
            params.qi_transfers.as_deref_mut(),
            player_account_id_for_entity(entity, None),
            pending.container_account.clone(),
            pending.amount,
            QiTransferReason::Channeling,
        );
        params.commands.entity(entity).insert(SwordQiStore {
            stored_qi: pending.amount,
            qi_per_hit: pending.amount / SWORD_INFUSE_HITS,
            remaining_ticks: SWORD_INFUSE_DURATION_TICKS,
            infuser_color: pending.infuser_color,
            weapon_instance_id: pending.weapon_instance_id,
            container_account: pending.container_account.clone(),
            carrier: pending.carrier,
        });
        if let (Some(events), Ok(position)) = (
            params.vfx_events.as_deref_mut(),
            params.positions.get(entity),
        ) {
            emit_particle(
                events,
                position.get() + DVec3::new(0.0, 1.0, 0.0),
                "bong:sword_infuse_glow",
                color_hex(pending.infuser_color),
                0.85,
                8,
                40,
            );
            if let Ok(unique_id) = params.unique_ids.get(entity) {
                events.send(VfxEventRequest::new(
                    position.get(),
                    VfxEventPayloadV1::PlayAnim {
                        target_player: unique_id.0.to_string(),
                        anim_id: "bong:sword_infuse".to_string(),
                        priority: 1200,
                        fade_in_ticks: Some(2),
                    },
                ));
            }
        }
        if let (Some(events), Ok(position)) = (
            params.audio_events.as_deref_mut(),
            params.positions.get(entity),
        ) {
            emit_audio(events, "sword_infuse", entity, position.get());
        }
        params
            .commands
            .entity(entity)
            .remove::<PendingSwordInfuse>();
    }
}

pub fn drain_sword_qi_for_hit(world: &mut bevy_ecs::world::World, caster: Entity) -> f32 {
    let (spent, container_account) = {
        let Some(mut store) = world.get_mut::<SwordQiStore>(caster) else {
            return 0.0;
        };
        if store.stored_qi <= f64::EPSILON || store.remaining_ticks == 0 {
            return 0.0;
        }
        let spent = store.qi_per_hit.min(store.stored_qi).max(0.0);
        store.stored_qi = (store.stored_qi - spent).max(0.0);
        (spent, store.container_account.clone())
    };
    // Credit zone directly — QiTransfer is audit-only and has no EventReader.
    // 守恒（#701）：与 tick 路径同源逻辑（apply_zone_credit + emit_zone_credit_transfers），zone 饱和/
    // 缺失的截断部分路由 overflow 账户，不蒸发。World 单借用限制 → 分两步借 ZoneRegistry / Events。
    let (accepted, overflow) = {
        let mut zones = world.get_resource_mut::<ZoneRegistry>();
        apply_zone_credit(zones.as_deref_mut(), &container_account, spent)
    };
    let mut qi_transfers = world.get_resource_mut::<Events<QiTransfer>>();
    emit_zone_credit_transfers(
        qi_transfers.as_deref_mut(),
        container_account,
        accepted,
        overflow,
        QiTransferReason::ReleaseToZone,
    );
    spent as f32
}

fn cast_sword_cleave(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    cast_sword_attack(world, caster, slot, target, SwordTechnique::Cleave)
}

fn cast_sword_thrust(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    cast_sword_attack(world, caster, slot, target, SwordTechnique::Thrust)
}

fn cast_sword_attack(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
    technique: SwordTechnique,
) -> CastResult {
    // 去掉"目标无效"门禁：劈/刺是近战挥击，准星没对准实体也照常挥出（动画 / 粒子 / 体力 /
    // 冷却照走）。命中走 AttackIntent —— target=Some 时正常命中，None 时 resolver 跳过即空挥。
    // 不自动锁定、不改战斗解析、无误伤（用户选 Option B：只拆门禁）。
    let now_tick = current_tick(world);
    if is_on_cooldown(world, caster, slot, now_tick) {
        return rejected(CastRejectReason::OnCooldown);
    }
    if !has_sword(world, caster) {
        return rejected(CastRejectReason::NoWeapon);
    }
    if exhausted(world, caster) {
        return rejected(CastRejectReason::InRecovery);
    }
    let Some(proficiency) = known_active_proficiency(world, caster, technique) else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    let profile = sword_profile(technique, proficiency);
    spend_stamina(world, caster, profile.stamina_cost);
    set_cooldown(
        world,
        caster,
        slot,
        now_tick.saturating_add(profile.cooldown_ticks),
    );
    let qi_invest = drain_sword_qi_for_hit(world, caster);
    world.send_event(AttackIntent {
        attacker: caster,
        // 直接透传 Option：有目标则命中，无目标则 resolver 跳过 = 空挥（不再拦截）。
        target,
        issued_at_tick: now_tick,
        reach: AttackReach::new(profile.range, 0.0),
        qi_invest,
        wound_kind: match technique {
            SwordTechnique::Cleave => WoundKind::Cut,
            SwordTechnique::Thrust => WoundKind::Pierce,
            _ => WoundKind::Cut,
        },
        source: match technique {
            SwordTechnique::Cleave => AttackSource::SwordCleave,
            SwordTechnique::Thrust => AttackSource::SwordThrust,
            _ => AttackSource::Melee,
        },
        debug_command: None,
    });
    // 纯 cosmetic：劈 / 刺命中挥出时各发自己的专属粒子（client `SwordBasicsVfxPlayer`
    // 早已注册 `bong:sword_cleave_trail` / `bong:sword_thrust_hit`，此前 server 从不
    // emit）。动画仍走 AttackIntent → `emit_attack_animation_triggers`，故此处只发粒子，
    // 避免与基础剑斩动画双重触发。
    emit_attack_particle(world, caster, technique);
    CastResult::Started {
        cooldown_ticks: profile.cooldown_ticks,
        anim_duration_ticks: profile.cast_ticks,
    }
}

/// 劈 / 刺各自的客户端已注册粒子 event_id —— 与 `SwordBasicsVfxPlayer` 逐字符对齐。
fn cleave_thrust_particle_id(technique: SwordTechnique) -> Option<&'static str> {
    match technique {
        SwordTechnique::Cleave => Some("bong:sword_cleave_trail"),
        SwordTechnique::Thrust => Some("bong:sword_thrust_hit"),
        // 格挡 / 灌注走各自 cast 路径的 emit_self_visuals，不经此函数。
        SwordTechnique::Parry | SwordTechnique::Infuse => None,
    }
}

/// 劈 / 刺粒子的色彩，对齐 client `SwordBasicsVfxPlayer.fallbackRgb`
/// （CLEAVE=0xC0C0C8 银白、THRUST=0xC03030 暗红）。
fn cleave_thrust_particle_color(technique: SwordTechnique) -> Option<&'static str> {
    match technique {
        SwordTechnique::Cleave => Some("#C0C0C8"),
        SwordTechnique::Thrust => Some("#C03030"),
        // 格挡 / 灌注不经此函数（与 cleave_thrust_particle_id 同契约）。
        SwordTechnique::Parry | SwordTechnique::Infuse => None,
    }
}

/// 在 caster 位置 emit 劈 / 刺的专属粒子。caster 无 `Position` 或无 VfxEventRequest
/// 资源（无头测试）时静默跳过。
fn emit_attack_particle(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    technique: SwordTechnique,
) {
    let Some(event_id) = cleave_thrust_particle_id(technique) else {
        return;
    };
    let Some(color) = cleave_thrust_particle_color(technique) else {
        return;
    };
    let Some(origin) = world.get::<Position>(caster).map(|position| position.get()) else {
        return;
    };
    // 劈是横向剑势 → 略宽弧线、稍长尾迹；刺是点状穿刺 → 集中、短促。
    let (count, duration) = match technique {
        SwordTechnique::Cleave => (10u16, 18u16),
        SwordTechnique::Thrust => (8u16, 14u16),
        // id/color guard 已排除，此处不可达；用 return 而非假值。
        SwordTechnique::Parry | SwordTechnique::Infuse => return,
    };
    if let Some(mut events) = world.get_resource_mut::<Events<VfxEventRequest>>() {
        emit_particle(
            &mut events,
            origin + DVec3::new(0.0, 1.0, 0.0),
            event_id,
            color,
            0.85,
            count,
            duration,
        );
    }
}

fn cast_sword_parry(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    _target: Option<Entity>,
) -> CastResult {
    let now_tick = current_tick(world);
    if is_on_cooldown(world, caster, slot, now_tick) {
        return rejected(CastRejectReason::OnCooldown);
    }
    if !has_sword(world, caster) {
        return rejected(CastRejectReason::NoWeapon);
    }
    if exhausted(world, caster) {
        return rejected(CastRejectReason::InRecovery);
    }
    let Some(proficiency) = known_active_proficiency(world, caster, SwordTechnique::Parry) else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    let profile = sword_profile(SwordTechnique::Parry, proficiency);
    spend_stamina(world, caster, profile.stamina_cost);
    set_cooldown(
        world,
        caster,
        slot,
        now_tick.saturating_add(profile.cooldown_ticks),
    );
    if let Some(mut statuses) = world.get_mut::<StatusEffects>(caster) {
        upsert_status_effect(
            &mut statuses,
            crate::combat::components::ActiveStatusEffect {
                kind: StatusEffectKind::SwordParrying,
                magnitude: profile.block_ratio,
                remaining_ticks: profile.parry_window_ticks,
                source_pill: None,
            },
        );
    }
    apply_known_gain(world, caster, SwordTechnique::Parry, false, false);
    emit_self_visuals(
        world,
        caster,
        "bong:sword_parry",
        "bong:sword_parry_spark",
        "#FFD080",
        1200,
    );
    CastResult::Started {
        cooldown_ticks: profile.cooldown_ticks,
        anim_duration_ticks: profile.cast_ticks,
    }
}

fn cast_sword_infuse(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    _target: Option<Entity>,
) -> CastResult {
    let now_tick = current_tick(world);
    if is_on_cooldown(world, caster, slot, now_tick) {
        return rejected(CastRejectReason::OnCooldown);
    }
    let Some(cultivation) = world.get::<Cultivation>(caster).cloned() else {
        return rejected(CastRejectReason::RealmTooLow);
    };
    if cultivation.realm == Realm::Awaken {
        return rejected(CastRejectReason::RealmTooLow);
    }
    let Some(weapon) = world
        .get::<Weapon>(caster)
        .cloned()
        .filter(|weapon| weapon.weapon_kind == WeaponKind::Sword)
    else {
        return rejected(CastRejectReason::NoWeapon);
    };
    if exhausted(world, caster) {
        return rejected(CastRejectReason::InRecovery);
    }
    let Some(proficiency) = known_active_proficiency(world, caster, SwordTechnique::Infuse) else {
        return rejected(CastRejectReason::InvalidTarget);
    };
    let profile = sword_profile(SwordTechnique::Infuse, proficiency);
    let amount = (cultivation.qi_current * SWORD_INFUSE_MAX_FRACTION)
        .max(0.0)
        .min(cultivation.qi_current);
    if amount < SWORD_INFUSE_MIN_QI {
        return rejected(CastRejectReason::QiInsufficient);
    }
    spend_stamina(world, caster, profile.stamina_cost);
    set_cooldown(
        world,
        caster,
        slot,
        now_tick.saturating_add(profile.cooldown_ticks),
    );
    insert_casting(
        world,
        caster,
        slot,
        SWORD_INFUSE_SKILL_ID,
        profile,
        now_tick,
    );
    let color = world
        .get::<QiColor>(caster)
        .map(|color| color.main)
        .unwrap_or(ColorKind::Mellow);
    world.entity_mut(caster).insert(PendingSwordInfuse {
        amount,
        complete_at_tick: now_tick.saturating_add(u64::from(profile.cast_ticks)),
        slot,
        weapon_instance_id: weapon.instance_id,
        carrier: carrier_for_quality(weapon.quality_tier),
        infuser_color: color,
        container_account: QiAccountId::container(format!(
            "sword_qi_store:{caster:?}:{}",
            weapon.instance_id
        )),
    });
    emit_self_visuals(
        world,
        caster,
        "bong:sword_infuse",
        "bong:sword_infuse_glow",
        color_hex(color),
        1200,
    );
    CastResult::Started {
        cooldown_ticks: profile.cooldown_ticks,
        anim_duration_ticks: profile.cast_ticks,
    }
}

fn known_active_proficiency(
    world: &bevy_ecs::world::World,
    caster: Entity,
    technique: SwordTechnique,
) -> Option<f32> {
    world
        .get::<KnownTechniques>(caster)?
        .entries
        .iter()
        .find(|entry| entry.id == technique.id() && entry.active)
        .map(|entry| entry.proficiency.clamp(0.0, 1.0))
}

fn apply_known_gain(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    technique: SwordTechnique,
    successful: bool,
    parry_bonus: bool,
) {
    let Some(mut known) = world.get_mut::<KnownTechniques>(caster) else {
        return;
    };
    let Some(entry) = known
        .entries
        .iter_mut()
        .find(|entry| entry.id == technique.id())
    else {
        return;
    };
    let gain = sword_proficiency_gain(entry.proficiency, successful, parry_bonus);
    entry.proficiency = (entry.proficiency + gain).clamp(0.0, 1.0);
}

fn has_sword(world: &bevy_ecs::world::World, caster: Entity) -> bool {
    world
        .get::<Weapon>(caster)
        .is_some_and(|weapon| weapon.weapon_kind == WeaponKind::Sword)
}

fn exhausted(world: &bevy_ecs::world::World, caster: Entity) -> bool {
    world
        .get::<Stamina>(caster)
        .is_some_and(|stamina| stamina.state == StaminaState::Exhausted || stamina.current <= 0.0)
        || world
            .get::<StatusEffects>(caster)
            .is_some_and(|statuses| has_active_status(statuses, StatusEffectKind::Stunned))
}

fn spend_stamina(world: &mut bevy_ecs::world::World, caster: Entity, amount: f32) {
    let now_tick = current_tick(world);
    let Some(mut stamina) = world.get_mut::<Stamina>(caster) else {
        return;
    };
    stamina.current = (stamina.current - amount.max(0.0)).clamp(0.0, stamina.max);
    stamina.state = if stamina.current <= 0.0 {
        StaminaState::Exhausted
    } else {
        StaminaState::Combat
    };
    stamina.last_drain_tick = Some(now_tick);
}

fn is_on_cooldown(world: &bevy_ecs::world::World, caster: Entity, slot: u8, now_tick: u64) -> bool {
    world
        .get::<SkillBarBindings>(caster)
        .is_some_and(|bindings| bindings.is_on_cooldown(slot, now_tick))
}

fn set_cooldown(world: &mut bevy_ecs::world::World, caster: Entity, slot: u8, until_tick: u64) {
    if let Some(mut bindings) = world.get_mut::<SkillBarBindings>(caster) {
        bindings.set_cooldown(slot, until_tick);
    }
}

fn insert_casting(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    skill_id: &str,
    profile: SwordTechniqueProfile,
    now_tick: u64,
) {
    let start_position = world
        .get::<Position>(caster)
        .map(|position| position.get())
        .unwrap_or(DVec3::ZERO);
    world.entity_mut(caster).insert(Casting {
        source: CastSource::SkillBar,
        slot,
        started_at_tick: now_tick,
        duration_ticks: u64::from(profile.cast_ticks),
        started_at_ms: current_unix_millis(),
        duration_ms: profile
            .cast_ticks
            .saturating_mul(crate::time::MILLIS_PER_TICK as u32),
        bound_instance_id: None,
        start_position,
        complete_cooldown_ticks: profile.cooldown_ticks,
        skill_id: Some(skill_id.to_string()),
        skill_config: None,
    });
}

fn current_tick(world: &bevy_ecs::world::World) -> u64 {
    world
        .get_resource::<CombatClock>()
        .map(|clock| clock.tick)
        .unwrap_or_default()
}

fn carrier_for_quality(quality_tier: u8) -> ContainerKind {
    match quality_tier {
        0 => ContainerKind::WieldedInWeapon,
        1 => ContainerKind::SealedInBone,
        _ => ContainerKind::SealedAncientRelic,
    }
}

fn player_account_id_for_entity(entity: Entity, life_record: Option<&LifeRecord>) -> QiAccountId {
    if let Some(life_record) = life_record {
        return QiAccountId::player(life_record.character_id.clone());
    }
    QiAccountId::player(format!("entity:{entity:?}"))
}

/// 把 `amount` 入账 spawn zone（直接写 `zone.spirit_qi`，承重项——QiTransfer 全仓 audit-only 无
/// EventReader），返回 `(accepted, overflow)`。守恒（CodeRabbit #701 Critical）：用
/// `qi_release_to_zone` 算 accepted/overflow，zone 饱和被截断 / 缺 ZoneRegistry|spawn zone 时
/// **截断/缺失的部分作为 overflow 返回**（由 caller 路由 overflow 账户），不凭空蒸发。
/// 裸 `spirit_qi*CAP`（不 .max(0.0)，负灵域守恒）。
fn apply_zone_credit(
    zones: Option<&mut ZoneRegistry>,
    from: &QiAccountId,
    amount: f64,
) -> (f64, f64) {
    match zones {
        Some(zones) => match zones.find_zone_mut(DEFAULT_SPAWN_ZONE_NAME) {
            Some(zone) => {
                let zone_current = zone.spirit_qi * QI_ZONE_UNIT_CAPACITY;
                match qi_release_to_zone(
                    amount,
                    from.clone(),
                    QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME),
                    zone_current,
                    QI_ZONE_UNIT_CAPACITY,
                ) {
                    Ok(outcome) => {
                        zone.spirit_qi =
                            (outcome.zone_after / QI_ZONE_UNIT_CAPACITY).clamp(-1.0, 1.0);
                        (outcome.accepted, outcome.overflow)
                    }
                    Err(_) => (0.0, amount), // invalid input：全额 overflow，不蒸发
                }
            }
            None => (0.0, amount), // 无 spawn zone → 全额 overflow
        },
        None => (0.0, amount), // 无 ZoneRegistry → 全额 overflow
    }
}

/// emit accepted→zone + overflow→overflow 账户两条审计 transfer（同 reason），accepted+overflow==amount。
fn emit_zone_credit_transfers(
    events: Option<&mut Events<QiTransfer>>,
    from: QiAccountId,
    accepted: f64,
    overflow: f64,
    reason: QiTransferReason,
) {
    let Some(events) = events else {
        return;
    };
    if accepted > QI_EPSILON {
        if let Ok(transfer) = QiTransfer::new(
            from.clone(),
            QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME),
            accepted,
            reason,
        ) {
            events.send(transfer);
        }
    }
    if overflow > QI_EPSILON {
        let overflow_to = QiAccountId::overflow("sword_qi_overflow".to_string());
        if let Ok(transfer) = QiTransfer::new(from, overflow_to, overflow, reason) {
            events.send(transfer);
        }
    }
}

/// Credit `amount` into spawn zone + emit audit transfer(s). zone/tick 路径共用（hit 路径因 World
/// 单借用限制走 apply_zone_credit + emit_zone_credit_transfers 两步，但逻辑同源）。
fn credit_qi_to_zone(
    zones: Option<&mut ZoneRegistry>,
    qi_transfers: Option<&mut Events<QiTransfer>>,
    from: QiAccountId,
    amount: f64,
    reason: QiTransferReason,
) {
    if amount <= QI_EPSILON {
        return;
    }
    let (accepted, overflow) = apply_zone_credit(zones, &from, amount);
    emit_zone_credit_transfers(qi_transfers, from, accepted, overflow, reason);
}

fn emit_qi_transfer(
    events: Option<&mut Events<QiTransfer>>,
    from: QiAccountId,
    to: QiAccountId,
    amount: f64,
    reason: QiTransferReason,
) {
    let Some(events) = events else {
        return;
    };
    if let Ok(transfer) = QiTransfer::new(from, to, amount, reason) {
        events.send(transfer);
    }
}

fn emit_self_visuals(
    world: &mut bevy_ecs::world::World,
    entity: Entity,
    anim_id: &str,
    particle_id: &str,
    color: &str,
    priority: u16,
) {
    let origin = world
        .get::<Position>(entity)
        .map(|position| position.get())
        .unwrap_or(DVec3::ZERO);
    let unique_id = world.get::<UniqueId>(entity).map(|id| id.0.to_string());
    if let Some(mut events) = world.get_resource_mut::<Events<VfxEventRequest>>() {
        if let Some(target_player) = unique_id {
            events.send(VfxEventRequest::new(
                origin,
                VfxEventPayloadV1::PlayAnim {
                    target_player,
                    anim_id: anim_id.to_string(),
                    priority,
                    fade_in_ticks: Some(2),
                },
            ));
        }
        emit_particle(
            &mut events,
            origin + DVec3::new(0.0, 1.0, 0.0),
            particle_id,
            color,
            0.8,
            8,
            24,
        );
    }
    if let Some(mut events) = world.get_resource_mut::<Events<PlaySoundRecipeRequest>>() {
        let recipe = match particle_id {
            "bong:sword_parry_spark" => "sword_parry",
            "bong:sword_infuse_glow" => "sword_infuse",
            _ => return,
        };
        emit_audio(&mut events, recipe, entity, origin);
    }
}

fn emit_particle(
    events: &mut Events<VfxEventRequest>,
    origin: DVec3,
    event_id: &str,
    color: &str,
    strength: f32,
    count: u16,
    duration_ticks: u16,
) {
    events.send(VfxEventRequest::new(
        origin,
        VfxEventPayloadV1::SpawnParticle {
            event_id: event_id.to_string(),
            origin: [origin.x, origin.y, origin.z],
            direction: None,
            color: Some(color.to_string()),
            strength: Some(strength.clamp(0.0, 1.0)),
            count: Some(count),
            duration_ticks: Some(duration_ticks),
        },
    ));
}

fn emit_audio(
    events: &mut Events<PlaySoundRecipeRequest>,
    recipe: &str,
    _entity: Entity,
    origin: DVec3,
) {
    events.send(PlaySoundRecipeRequest {
        recipe_id: recipe.to_string(),
        instance_id: 0,
        pos: None,
        flag: None,
        volume_mul: 1.0,
        pitch_shift: 0.0,
        recipient: AudioRecipient::Radius {
            origin,
            radius: AUDIO_BROADCAST_RADIUS,
        },
    });
}

fn color_hex(color: ColorKind) -> &'static str {
    match color {
        ColorKind::Sharp => "#C0C8E8",
        ColorKind::Heavy => "#8A6A44",
        ColorKind::Mellow => "#B0E0C0",
        ColorKind::Solid => "#B8B8B8",
        ColorKind::Light => "#E8F6FF",
        ColorKind::Intricate => "#7B63D8",
        ColorKind::Gentle => "#BDE8D0",
        ColorKind::Insidious => "#7A4AA0",
        ColorKind::Violent => "#D84830",
        ColorKind::Turbid => "#807060",
    }
}

fn lerp(start: f32, end: f32, t: f32) -> f32 {
    start + (end - start) * t.clamp(0.0, 1.0)
}

fn lerp_round(start: f32, end: f32, t: f32) -> u32 {
    lerp(start, end, t).round().max(1.0) as u32
}

fn rejected(reason: CastRejectReason) -> CastResult {
    CastResult::Rejected { reason }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::known_techniques::KnownTechnique;
    use valence::prelude::{App, Update};

    #[test]
    fn proficiency_labels_cover_five_visible_bands() {
        assert_eq!(sword_proficiency_label(0.0), "生疏");
        assert_eq!(sword_proficiency_label(0.2), "入门");
        assert_eq!(sword_proficiency_label(0.5), "熟练");
        assert_eq!(sword_proficiency_label(0.8), "精通");
        assert_eq!(sword_proficiency_label(0.95), "化境");
    }

    #[test]
    fn profiles_scale_core_sword_knobs() {
        let novice = sword_profile(SwordTechnique::Cleave, 0.0);
        let master = sword_profile(SwordTechnique::Cleave, 1.0);
        assert_eq!(novice.stamina_cost, 8.0);
        assert_eq!(master.stamina_cost, 5.0);
        assert_eq!(novice.cast_ticks, 16);
        assert_eq!(master.cast_ticks, 10);
        assert!((master.damage_multiplier - 1.3).abs() < f32::EPSILON);

        let parry = sword_profile(SwordTechnique::Parry, 1.0);
        assert_eq!(parry.parry_window_ticks, 8);
        assert!((parry.block_ratio - 0.6).abs() < f32::EPSILON);
    }

    fn make_zone_registry_empty() -> ZoneRegistry {
        use crate::world::zone::ZoneRegistry;
        let mut registry = ZoneRegistry::fallback();
        // Reset to 0.0 so the zone has full room; avoids overflow/split in assertions.
        registry
            .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
            .unwrap()
            .spirit_qi = 0.0;
        registry
    }

    #[test]
    fn sword_qi_store_leaks_to_zone_and_expires() {
        let mut app = App::new();
        app.insert_resource(CombatClock {
            tick: SWORD_QI_STORE_TICK_INTERVAL,
        });
        app.insert_resource(make_zone_registry_empty());
        app.add_event::<QiTransfer>();
        app.add_systems(Update, sword_qi_store_tick);
        let entity = app
            .world_mut()
            .spawn(SwordQiStore {
                stored_qi: 10.0,
                qi_per_hit: 2.0,
                remaining_ticks: SWORD_QI_STORE_TICK_INTERVAL,
                infuser_color: ColorKind::Mellow,
                weapon_instance_id: 1,
                container_account: QiAccountId::container("test_sword"),
                carrier: ContainerKind::WieldedInWeapon,
            })
            .id();

        app.update();

        assert!(app.world().get::<SwordQiStore>(entity).is_none());
        let transfers = app.world().resource::<Events<QiTransfer>>();
        assert!(!transfers.is_empty());
    }

    /// On expiry the full remaining stored_qi must flow to zone.spirit_qi (not disappear).
    #[test]
    fn sword_qi_store_expiry_credits_remaining_qi_to_zone() {
        let stored = 10.0_f64;
        let mut app = App::new();
        // Set clock to the last tick (remaining_ticks will hit 0 after one interval).
        app.insert_resource(CombatClock {
            tick: SWORD_QI_STORE_TICK_INTERVAL,
        });
        app.insert_resource(make_zone_registry_empty());
        app.add_event::<QiTransfer>();
        app.add_systems(Update, sword_qi_store_tick);
        let entity = app
            .world_mut()
            .spawn(SwordQiStore {
                stored_qi: stored,
                qi_per_hit: 2.0,
                remaining_ticks: SWORD_QI_STORE_TICK_INTERVAL,
                infuser_color: ColorKind::Mellow,
                weapon_instance_id: 1,
                container_account: QiAccountId::container("test_sword_expiry"),
                carrier: ContainerKind::WieldedInWeapon,
            })
            .id();

        app.update();

        // Component must be removed.
        assert!(
            app.world().get::<SwordQiStore>(entity).is_none(),
            "SwordQiStore must be removed on expiry"
        );
        // Zone must have received the qi (excretion loss is tiny over one interval;
        // the remainder flush brings it back).
        let zone_spirit_qi = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .unwrap()
            .spirit_qi;
        assert!(
            zone_spirit_qi > 0.0,
            "zone.spirit_qi must increase after SwordQiStore expiry: got {zone_spirit_qi}"
        );
        // The total credited to zone must equal original stored_qi (excretion + remainder).
        let expected_zone_delta = stored / QI_ZONE_UNIT_CAPACITY;
        assert!(
            (zone_spirit_qi - expected_zone_delta).abs() < 1e-9,
            "zone delta should equal stored/QI_ZONE_UNIT_CAPACITY={expected_zone_delta:.6}, \
             got zone_spirit_qi={zone_spirit_qi:.6}"
        );
    }

    /// On a hit, qi deducted from SwordQiStore must immediately raise zone.spirit_qi.
    #[test]
    fn sword_hit_credits_qi_to_zone() {
        let mut app = App::new();
        app.add_event::<QiTransfer>();
        app.insert_resource(make_zone_registry_empty());
        let container_account = QiAccountId::container("test_sword_zone_credit");
        let entity = app
            .world_mut()
            .spawn(SwordQiStore {
                stored_qi: 10.0,
                qi_per_hit: 2.0,
                remaining_ticks: SWORD_QI_STORE_TICK_INTERVAL,
                infuser_color: ColorKind::Mellow,
                weapon_instance_id: 1,
                container_account: container_account.clone(),
                carrier: ContainerKind::WieldedInWeapon,
            })
            .id();

        let spent = drain_sword_qi_for_hit(app.world_mut(), entity);

        // Spent amount matches qi_per_hit.
        assert!(
            (spent - 2.0).abs() < f32::EPSILON,
            "spent should be qi_per_hit=2.0, got {spent}"
        );
        // Zone spirit_qi must be directly credited with spent/QI_ZONE_UNIT_CAPACITY.
        let expected_zone_delta = 2.0_f64 / QI_ZONE_UNIT_CAPACITY;
        let zone_spirit_qi = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .unwrap()
            .spirit_qi;
        assert!(
            (zone_spirit_qi - expected_zone_delta).abs() < 1e-9,
            "zone.spirit_qi should increase by spent/QI_ZONE_UNIT_CAPACITY={expected_zone_delta:.6}, \
             got {zone_spirit_qi:.6} (bug: qi was not credited to zone on hit)"
        );
        // Audit event must also be emitted.
        let events = app.world().resource::<Events<QiTransfer>>();
        let mut reader = events.get_reader();
        let transfers: Vec<_> = reader.read(events).collect();
        assert_eq!(
            transfers.len(),
            1,
            "one QiTransfer audit event expected per hit"
        );
        let transfer = transfers[0];
        assert_eq!(
            transfer.from, container_account,
            "审计 transfer.from 应 == 命中扣减的 container_account（可溯源到来源容器）"
        );
        assert_eq!(
            transfer.to,
            QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME),
            "非饱和命中 qi 全额入 spawn zone 账户，故 to 应为该 zone 账户"
        );
        assert_eq!(
            transfer.reason,
            QiTransferReason::ReleaseToZone,
            "剑气回灌走 ReleaseToZone（区域回灌审计语义）"
        );
        assert!(
            (transfer.amount - 2.0).abs() < f64::EPSILON,
            "transfer amount should be qi_per_hit=2.0, got {}",
            transfer.amount
        );
    }

    /// 守恒最大边界（CodeRabbit #701 Critical）：spawn zone 接近饱和（spirit_qi=0.96 → room=2 raw）
    /// 时命中消耗 spent=5 → zone 只吃 room=2 饱和到 1.0，截断的 3 必须路由 overflow 账户而非蒸发；
    /// zone+overflow 之和 == spent（守恒，旧 += clamp 实现会丢 3）。
    #[test]
    fn sword_hit_saturated_zone_routes_overflow_no_evaporation() {
        use crate::qi_physics::ledger::QiAccountKind;
        let mut app = App::new();
        app.add_event::<QiTransfer>();
        // zone 接近满：spirit_qi=0.96 → zone_current=48, room=(50-48)=2
        let mut reg = make_zone_registry_empty();
        reg.find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
            .unwrap()
            .spirit_qi = 0.96;
        app.insert_resource(reg);
        let container_account = QiAccountId::container("test_sword_overflow");
        let entity = app
            .world_mut()
            .spawn(SwordQiStore {
                stored_qi: 100.0,
                qi_per_hit: 5.0, // spent=5 > room=2 → 截断
                remaining_ticks: SWORD_QI_STORE_TICK_INTERVAL,
                infuser_color: ColorKind::Mellow,
                weapon_instance_id: 1,
                container_account,
                carrier: ContainerKind::WieldedInWeapon,
            })
            .id();

        let spent = drain_sword_qi_for_hit(app.world_mut(), entity);
        assert!(
            (spent - 5.0).abs() < f32::EPSILON,
            "spent 应 == qi_per_hit=5（扣自 store），实际 {spent}"
        );

        let zone_after = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .unwrap()
            .spirit_qi;
        assert!(
            (zone_after - 1.0).abs() < 1e-9,
            "zone 应饱和到 1.0（吃 room=2 raw），实际 {zone_after}"
        );

        let events = app.world().resource::<Events<QiTransfer>>();
        let mut reader = events.get_reader();
        let releases: Vec<_> = reader
            .read(events)
            .filter(|t| t.reason == QiTransferReason::ReleaseToZone)
            .collect();
        let zone_credit: f64 = releases
            .iter()
            .filter(|t| t.to.kind == QiAccountKind::Zone)
            .map(|t| t.amount)
            .sum();
        let overflow_credit: f64 = releases
            .iter()
            .filter(|t| t.to.kind == QiAccountKind::Overflow)
            .map(|t| t.amount)
            .sum();
        assert!(
            (zone_credit - 2.0).abs() < 1e-9,
            "进 zone 账户应 == room=2，实际 {zone_credit}"
        );
        assert!(
            (overflow_credit - 3.0).abs() < 1e-9,
            "zone 饱和截断的 3 必须入 overflow 账户而非蒸发，实际 {overflow_credit}（#701 Critical）"
        );
        assert!(
            ((zone_credit + overflow_credit) - 5.0).abs() < 1e-9,
            "守恒：zone({zone_credit}) + overflow({overflow_credit}) 应 == spent 5，实际 {}",
            zone_credit + overflow_credit
        );
    }

    #[test]
    fn sword_hit_releases_spent_stored_qi_to_zone() {
        let mut app = App::new();
        app.add_event::<QiTransfer>();
        app.insert_resource(make_zone_registry_empty());
        let container_account = QiAccountId::container("test_sword_hit");
        let entity = app
            .world_mut()
            .spawn(SwordQiStore {
                stored_qi: 10.0,
                qi_per_hit: 2.0,
                remaining_ticks: SWORD_QI_STORE_TICK_INTERVAL,
                infuser_color: ColorKind::Mellow,
                weapon_instance_id: 1,
                container_account: container_account.clone(),
                carrier: ContainerKind::WieldedInWeapon,
            })
            .id();

        let spent = drain_sword_qi_for_hit(app.world_mut(), entity);

        assert!((spent - 2.0).abs() < f32::EPSILON);
        let store = app.world().get::<SwordQiStore>(entity).unwrap();
        assert!((store.stored_qi - 8.0).abs() < f64::EPSILON);
        let events = app.world().resource::<Events<QiTransfer>>();
        let mut reader = events.get_reader();
        let transfers: Vec<_> = reader.read(events).collect();
        assert_eq!(transfers.len(), 1);
        let transfer = transfers[0];
        assert_eq!(transfer.from, container_account);
        assert_eq!(transfer.to, QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME));
        assert_eq!(transfer.reason, QiTransferReason::ReleaseToZone);
        assert!((transfer.amount - 2.0).abs() < f64::EPSILON);
    }

    /// drain_sword_qi_for_hit returns 0.0 when there is no SwordQiStore on the entity.
    #[test]
    fn drain_hit_returns_zero_without_store() {
        let mut app = App::new();
        app.add_event::<QiTransfer>();
        app.insert_resource(make_zone_registry_empty());
        let entity = app.world_mut().spawn_empty().id();

        let spent = drain_sword_qi_for_hit(app.world_mut(), entity);

        assert_eq!(spent, 0.0, "no SwordQiStore → spent must be 0.0");
        let events = app.world().resource::<Events<QiTransfer>>();
        let mut reader = events.get_reader();
        assert!(
            reader.read(events).next().is_none(),
            "no QiTransfer should be emitted when there is no store"
        );
        let zone_qi = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .unwrap()
            .spirit_qi;
        assert_eq!(
            zone_qi, 0.0,
            "zone.spirit_qi must not change when nothing was spent"
        );
    }

    /// Excretion tick credits zone for the loss amount (not just emits an event).
    #[test]
    fn sword_qi_store_excretion_credits_zone() {
        let mut app = App::new();
        app.insert_resource(CombatClock {
            tick: SWORD_QI_STORE_TICK_INTERVAL,
        });
        let mut registry = make_zone_registry_empty();
        // Store has enough remaining ticks to survive this tick (2 intervals → only 1 elapse).
        app.insert_resource({
            registry
                .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
                .unwrap()
                .spirit_qi = 0.0;
            registry
        });
        app.add_event::<QiTransfer>();
        app.add_systems(Update, sword_qi_store_tick);
        let _entity = app
            .world_mut()
            .spawn(SwordQiStore {
                stored_qi: 50.0,
                qi_per_hit: 5.0,
                // 2 intervals remaining so it won't expire this tick.
                remaining_ticks: 2 * SWORD_QI_STORE_TICK_INTERVAL,
                infuser_color: ColorKind::Mellow,
                weapon_instance_id: 1,
                container_account: QiAccountId::container("test_excretion"),
                carrier: ContainerKind::WieldedInWeapon,
            })
            .id();

        app.update();

        // After one excretion tick, zone must have received some qi.
        let zone_qi = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .unwrap()
            .spirit_qi;
        assert!(
            zone_qi > 0.0,
            "zone.spirit_qi must increase after excretion tick, got {zone_qi} \
             (bug: credit was not applied directly to zone)"
        );
    }

    #[test]
    fn sword_proficiency_gain_diminishes() {
        assert!(
            sword_proficiency_gain(0.0, true, false) > sword_proficiency_gain(0.9, true, false)
        );
        assert!(sword_proficiency_gain(0.1, true, true) > sword_proficiency_gain(0.1, true, false));
        assert_eq!(sword_proficiency_gain(0.1, false, false), 0.002);
    }

    fn known(id: &str, proficiency: f32) -> KnownTechniques {
        KnownTechniques {
            entries: vec![KnownTechnique {
                id: id.to_string(),
                proficiency,
                active: true,
            }],
        }
    }

    #[test]
    fn hit_events_raise_matching_sword_proficiency() {
        let mut app = App::new();
        app.add_event::<crate::combat::events::CombatEvent>();
        app.add_systems(Update, track_sword_proficiency_from_hits);
        let attacker = app
            .world_mut()
            .spawn(known(SWORD_CLEAVE_SKILL_ID, 0.0))
            .id();
        let target = app.world_mut().spawn_empty().id();
        app.world_mut()
            .send_event(crate::combat::events::CombatEvent {
                attacker,
                target,
                resolved_at_tick: 1,
                body_part: crate::combat::components::BodyPart::Chest,
                wound_kind: WoundKind::Cut,
                source: AttackSource::SwordCleave,
                debug_command: false,
                physical_damage: 3.0,
                damage: 0.0,
                contam_delta: 0.0,
                description: "hit".to_string(),
                defense_kind: None,
                defense_effectiveness: None,
                defense_contam_reduced: None,
                defense_wound_severity: None,
            });

        app.update();

        let known = app.world().get::<KnownTechniques>(attacker).unwrap();
        assert!(known.entries[0].proficiency > 0.0);
    }

    // ── 缺武器拒绝（plan-skill-warn-hud）：手里没剑施放剑技 → NoWeapon ────────────

    #[test]
    fn cleave_without_sword_rejects_with_no_weapon_not_invalid_target() {
        // 剑技需手持剑。caster 有目标 + 激活劈技但无 Weapon component → has_sword=false
        // → 应拒绝 NoWeapon（专用「缺武器」原因），而非旧的笼统 InvalidTarget。
        // 让通用警示 HUD 能显示「缺少武器」区别于「目标无效」。
        let mut app = App::new();
        let target = app.world_mut().spawn_empty().id();
        let caster = app
            .world_mut()
            .spawn(known(SWORD_CLEAVE_SKILL_ID, 0.5))
            .id();

        let result = cast_sword_cleave(app.world_mut(), caster, 0, Some(target));

        assert_eq!(
            result,
            CastResult::Rejected {
                reason: CastRejectReason::NoWeapon
            },
            "手持无剑施放劈砍应拒绝 NoWeapon（缺武器），实际 {result:?}"
        );
        assert_ne!(
            result,
            CastResult::Rejected {
                reason: CastRejectReason::InvalidTarget
            },
            "缺武器不应再笼统报 InvalidTarget——否则警示 HUD 会错显「目标无效」"
        );
    }

    #[test]
    fn cleave_without_target_no_longer_rejects_invalid_target() {
        // Option B（去掉"目标无效"门禁）：无目标的劈砍不再报 InvalidTarget。
        // 此 caster 无剑 → 现在落到更后的 NoWeapon 门，证明 no-target 门已彻底移除
        // （而非仍把"准星没对准"当目标无效拦下）。
        let mut app = App::new();
        let caster = app
            .world_mut()
            .spawn(known(SWORD_CLEAVE_SKILL_ID, 0.5))
            .id();

        let result = cast_sword_cleave(app.world_mut(), caster, 0, None);

        assert_ne!(
            result,
            CastResult::Rejected {
                reason: CastRejectReason::InvalidTarget
            },
            "无目标近战不应再被'目标无效'拦截（Option B 拆门禁），实际 {result:?}"
        );
        assert_eq!(
            result,
            CastResult::Rejected {
                reason: CastRejectReason::NoWeapon
            },
            "无剑时应落到 NoWeapon 门（no-target 门已移除，后续门照常），实际 {result:?}"
        );
    }

    #[test]
    fn sword_swing_without_target_air_swings_started_not_rejected() {
        // Option B 核心行为：持剑挥击但准星没对准实体（target=None）→ 照常挥出（Started），
        // 动画 / 体力 / 冷却照走、AttackIntent 以 None 目标发出（resolver 跳过 = 空挥，不命中、
        // 不误伤）。不再被"目标无效"拦。
        use crate::combat::weapon::{EquipSlot, Weapon, WeaponKind};

        let mut app = App::new();
        app.add_event::<AttackIntent>();
        let caster = app
            .world_mut()
            .spawn((
                known(SWORD_CLEAVE_SKILL_ID, 0.5),
                Weapon {
                    slot: EquipSlot::MainHand,
                    instance_id: 1,
                    template_id: "test_sword".to_string(),
                    weapon_kind: WeaponKind::Sword,
                    base_attack: 10.0,
                    quality_tier: 0,
                    durability: 100.0,
                    durability_max: 100.0,
                },
            ))
            .id();

        let result = cast_sword_cleave(app.world_mut(), caster, 0, None);

        assert!(
            matches!(result, CastResult::Started { .. }),
            "持剑无目标挥击应照常挥出 Started（空挥），实际 {result:?}"
        );
        // 确实发了一条 AttackIntent，且 target=None（空挥：resolver 会跳过、不命中）。
        let events = app.world().resource::<Events<AttackIntent>>();
        let intent = events
            .iter_current_update_events()
            .next()
            .expect("无目标挥击仍应发 AttackIntent（命中走 resolver，None 则空挥）");
        assert_eq!(intent.attacker, caster);
        assert_eq!(
            intent.target, None,
            "空挥的 AttackIntent.target 必须是 None（resolver 跳过即不命中，无误伤）"
        );
    }

    #[test]
    fn parry_without_sword_rejects_no_weapon() {
        // 格挡同样需手持剑：gate 顺序 OnCooldown → has_sword。fresh caster 无冷却、
        // 无 Weapon → 直达 NoWeapon。pin 住「无剑格挡 → NoWeapon 而非笼统拒绝」，
        // 让通用警示 HUD 对格挡也显示「缺少武器」。
        let mut app = App::new();
        let caster = app.world_mut().spawn(known(SWORD_PARRY_SKILL_ID, 0.5)).id();

        let result = cast_sword_parry(app.world_mut(), caster, 0, None);

        assert_eq!(
            result,
            CastResult::Rejected {
                reason: CastRejectReason::NoWeapon
            },
            "无剑格挡应拒绝 NoWeapon（缺武器），实际 {result:?}"
        );
        assert_ne!(
            result,
            CastResult::Rejected {
                reason: CastRejectReason::InvalidTarget
            },
            "缺武器格挡不应报 InvalidTarget——否则警示 HUD 错显「目标无效」"
        );
    }

    #[test]
    fn infuse_without_sword_rejects_no_weapon() {
        // 灌注 gate 顺序 OnCooldown → 境界(RealmTooLow) → has_sword。需先满足境界门
        // （Cultivation 存在且 realm≠Awaken）才能抵达 has_sword，故给 Condense 境界 +
        // 不挂 Weapon → 直达 NoWeapon。pin 住「够境界但无剑 → NoWeapon 而非 RealmTooLow」。
        let mut app = App::new();
        let caster = app
            .world_mut()
            .spawn((
                known(SWORD_INFUSE_SKILL_ID, 0.5),
                Cultivation {
                    realm: Realm::Condense,
                    ..Default::default()
                },
            ))
            .id();

        let result = cast_sword_infuse(app.world_mut(), caster, 0, None);

        assert_eq!(
            result,
            CastResult::Rejected {
                reason: CastRejectReason::NoWeapon
            },
            "够境界但无剑灌注应拒绝 NoWeapon（缺武器），实际 {result:?}"
        );
        assert_ne!(
            result,
            CastResult::Rejected {
                reason: CastRejectReason::RealmTooLow
            },
            "境界已达标，缺武器不应误报 RealmTooLow——否则警示 HUD 错显「境界不足」"
        );
    }

    // ── 劈 / 刺专属粒子（client SwordBasicsVfxPlayer 早注册，此前 server 不 emit）──

    #[test]
    fn cleave_thrust_particle_ids_match_client_registry() {
        // 期望：劈 / 刺各映射到 client `SwordBasicsVfxPlayer` 已注册的 event_id，
        // 逐字符对齐，否则 client 收到 payload 找不到 player → 静默吞掉粒子。
        assert_eq!(
            cleave_thrust_particle_id(SwordTechnique::Cleave),
            Some("bong:sword_cleave_trail"),
            "劈应发 client CLEAVE_TRAIL（SwordBasicsVfxPlayer.CLEAVE_TRAIL = bong:sword_cleave_trail）"
        );
        assert_eq!(
            cleave_thrust_particle_id(SwordTechnique::Thrust),
            Some("bong:sword_thrust_hit"),
            "刺应发 client THRUST_HIT（SwordBasicsVfxPlayer.THRUST_HIT = bong:sword_thrust_hit）"
        );
        // 格挡 / 灌注走各自 cast 的 emit_self_visuals，不经此函数 → None，避免重复发粒子。
        assert_eq!(
            cleave_thrust_particle_id(SwordTechnique::Parry),
            None,
            "格挡不经 emit_attack_particle（它在 cast_sword_parry 里发 sword_parry_spark）"
        );
        assert_eq!(
            cleave_thrust_particle_id(SwordTechnique::Infuse),
            None,
            "灌注不经 emit_attack_particle（它在 cast_sword_infuse 里发 sword_infuse_glow）"
        );
    }

    #[test]
    fn cleave_thrust_particle_colors_match_client_fallback_rgb() {
        // 期望：与 client `SwordBasicsVfxPlayer.fallbackRgb` 同色，避免 server / client
        // 调色不一致（CLEAVE=0xC0C0C8 银白、THRUST=0xC03030 暗红）。
        assert_eq!(
            cleave_thrust_particle_color(SwordTechnique::Cleave),
            Some("#C0C0C8")
        );
        assert_eq!(
            cleave_thrust_particle_color(SwordTechnique::Thrust),
            Some("#C03030")
        );
        // 格挡 / 灌注不经此函数 → None（与 id 契约一致）。
        assert_eq!(cleave_thrust_particle_color(SwordTechnique::Parry), None);
        assert_eq!(cleave_thrust_particle_color(SwordTechnique::Infuse), None);
    }

    fn emitted_particles(app: &App) -> Vec<VfxEventPayloadV1> {
        let events = app.world().resource::<Events<VfxEventRequest>>();
        let mut reader = events.get_reader();
        reader
            .read(events)
            .map(|request| request.payload.clone())
            .collect()
    }

    #[test]
    fn emit_attack_particle_sends_cleave_trail_at_caster() {
        let mut app = App::new();
        app.add_event::<VfxEventRequest>();
        let caster = app
            .world_mut()
            .spawn(Position::new(DVec3::new(10.0, 64.0, -3.0)))
            .id();

        emit_attack_particle(app.world_mut(), caster, SwordTechnique::Cleave);

        let particles = emitted_particles(&app);
        assert_eq!(
            particles.len(),
            1,
            "劈应只发 1 条粒子（动画走 AttackIntent 路径，此处不重复发）"
        );
        match &particles[0] {
            VfxEventPayloadV1::SpawnParticle {
                event_id,
                origin,
                color,
                count,
                ..
            } => {
                assert_eq!(
                    event_id, "bong:sword_cleave_trail",
                    "劈的粒子 event_id 必须是 client 注册的 CLEAVE_TRAIL"
                );
                assert_eq!(
                    *origin,
                    [10.0, 65.0, -3.0],
                    "粒子原点应抬到 caster 头胸高（y+1），而非脚底"
                );
                assert_eq!(color.as_deref(), Some("#C0C0C8"));
                assert_eq!(*count, Some(10));
            }
            other => panic!("期望 SpawnParticle，实际 {other:?}"),
        }
    }

    #[test]
    fn emit_attack_particle_sends_thrust_hit_at_caster() {
        let mut app = App::new();
        app.add_event::<VfxEventRequest>();
        let caster = app
            .world_mut()
            .spawn(Position::new(DVec3::new(0.0, 70.0, 0.0)))
            .id();

        emit_attack_particle(app.world_mut(), caster, SwordTechnique::Thrust);

        let particles = emitted_particles(&app);
        assert_eq!(particles.len(), 1, "刺应只发 1 条粒子");
        match &particles[0] {
            VfxEventPayloadV1::SpawnParticle {
                event_id, color, ..
            } => {
                assert_eq!(
                    event_id, "bong:sword_thrust_hit",
                    "刺的粒子 event_id 必须是 client 注册的 THRUST_HIT"
                );
                assert_eq!(color.as_deref(), Some("#C03030"));
            }
            other => panic!("期望 SpawnParticle，实际 {other:?}"),
        }
    }

    #[test]
    fn emit_attack_particle_skips_when_caster_has_no_position() {
        let mut app = App::new();
        app.add_event::<VfxEventRequest>();
        let caster = app.world_mut().spawn_empty().id();

        emit_attack_particle(app.world_mut(), caster, SwordTechnique::Cleave);

        assert!(
            emitted_particles(&app).is_empty(),
            "无 Position 的 caster 不应发粒子（静默跳过，不 panic）"
        );
    }

    #[test]
    fn emit_attack_particle_skips_parry_and_infuse() {
        // 防回归：若误把格挡 / 灌注路由进 emit_attack_particle，会与各自 cast 的
        // emit_self_visuals 双重发粒子。此函数对这两招必须不发。
        for technique in [SwordTechnique::Parry, SwordTechnique::Infuse] {
            let mut app = App::new();
            app.add_event::<VfxEventRequest>();
            let caster = app
                .world_mut()
                .spawn(Position::new(DVec3::new(1.0, 64.0, 1.0)))
                .id();

            emit_attack_particle(app.world_mut(), caster, technique);

            assert!(
                emitted_particles(&app).is_empty(),
                "{technique:?} 不应经 emit_attack_particle 发粒子（它有自己的 emit_self_visuals）"
            );
        }
    }
}
