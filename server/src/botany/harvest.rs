use std::collections::HashSet;

use valence::prelude::{
    Entity, EventReader, EventWriter, Position, Query, RemovedComponents, Res, ResMut, With,
};

use crate::combat::components::Wounds;
use crate::combat::events::CombatEvent;
use crate::cultivation::breakthrough::skill_cap_for_realm;
use crate::cultivation::components::{Contamination, Cultivation, Realm};
use crate::gathering::quality::roll_quality;
use crate::gathering::tools::{equipped_gathering_tool, GatheringTargetKind};
use crate::inventory::{
    add_item_to_player_inventory_or_ground, DroppedLootRegistry, GrantOrGroundOutcome,
    InventoryDurabilityChangedEvent, InventoryInstanceIdAllocator, ItemInstance, ItemRegistry,
    PlayerInventory,
};
use crate::player::state::canonical_player_id;
use crate::skill::components::{SkillId, SkillSet};
use crate::skill::curve::effective_lv;
use crate::skill::events::{SkillXpGain, XpGainSource};
use crate::world::dimension::DimensionKind;

use super::components::{
    BotanyAttractsMobsEvent, BotanyHarvestMode, BotanyPhase, BotanySkillChangedEvent,
    BotanyTrampleRoll, HarvestSession, HarvestSessionStore, HarvestTerminalEvent,
    InventorySnapshotRequestEvent, Plant, PlantProximityTracker, PlantStaticPointStore,
};
use super::registry::{BotanyKindRegistry, BotanyPlantId, PlantVariant};

const MANUAL_DURATION_TICKS: u64 = 40;
const AUTO_DURATION_TICKS: u64 = 120;
/// plan-skill-v1 §7.1：野外采集 手动 +2 · 自动 +5。
const MANUAL_SKILL_XP: u64 = 2;
const AUTO_SKILL_XP: u64 = 5;
const MOVEMENT_BREAK_DISTANCE_SQ: f64 = 0.3 * 0.3;
/// plan §1.3 路径踩踏半径：玩家水平距离 < 0.7 块（约一个方块 footprint）视为踩到。
const TRAMPLE_RADIUS_SQ: f64 = 0.7 * 0.7;
/// 垂直距离 > 2 块认为跟植物不在同一层（平台/洞穴分层），不触发踩踏。
const TRAMPLE_VERTICAL_MAX: f64 = 2.0;

pub(crate) fn harvest_duration_ticks_for(mode: BotanyHarvestMode) -> u64 {
    match mode {
        BotanyHarvestMode::Manual => MANUAL_DURATION_TICKS,
        BotanyHarvestMode::Auto => AUTO_DURATION_TICKS,
    }
}

type HarvestHazardQuery<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static mut Cultivation>,
        Option<&'static SkillSet>,
        Option<&'static mut Contamination>,
        Option<&'static mut Wounds>,
    ),
    With<valence::prelude::Client>,
>;

#[allow(clippy::too_many_arguments)]
pub fn start_or_resume_harvest(
    store: &mut HarvestSessionStore,
    player_name: &str,
    client_entity: Entity,
    target_entity: Option<Entity>,
    target_plant: BotanyPlantId,
    mode: BotanyHarvestMode,
    origin_position: [f64; 3],
    now_tick: u64,
) -> bool {
    let player_id = canonical_player_id(player_name);
    if store.session_for(player_id.as_str()).is_some() {
        return false;
    }

    store
        .try_insert_session(HarvestSession {
            player_id,
            client_entity,
            target_entity,
            target_plant,
            mode,
            started_at_tick: now_tick,
            duration_ticks: harvest_duration_ticks_for(mode),
            phase: BotanyPhase::InProgress,
            last_progress: 0.0,
            origin_position,
        })
        .is_ok()
}

pub(crate) fn request_harvest_mode(
    store: &mut HarvestSessionStore,
    session_id: &str,
    client_entity: Entity,
    mode: BotanyHarvestMode,
    now_tick: u64,
) -> Result<(), String> {
    let session = store
        .session_for_mut(session_id)
        .ok_or_else(|| format!("missing harvest session `{session_id}`"))?;
    if session.client_entity != client_entity {
        return Err(format!(
            "harvest session `{session_id}` belongs to {:?}, not {:?}",
            session.client_entity, client_entity
        ));
    }

    session.mode = mode;
    session.started_at_tick = now_tick;
    session.duration_ticks = harvest_duration_ticks_for(mode);
    session.phase = BotanyPhase::InProgress;
    session.last_progress = 0.0;
    Ok(())
}

/// plan-bughunt-botany-disconnect-session P1：结构性前置校验失败（缺 kind /
/// 缺 Client+PlayerInventory）时补发 `interrupted=true` 终结帧。session 在完成路径
/// 入口就已移除，不发帧客户端会永远等不到收口。grant 阶段的结构性失败**不**在此列——
/// 那条路径的"无终结帧"语义由 plan-botany-harvest-full-inventory-loss-v1 §8.1 已 pin，
/// 本 plan 不翻案。
fn send_structural_cancel_terminal(
    session: &HarvestSession,
    terminal_events: &mut EventWriter<HarvestTerminalEvent>,
) {
    terminal_events.send(HarvestTerminalEvent {
        client_entity: session.client_entity,
        session_id: session.player_id.clone(),
        target_id: format_target_id(session.target_entity),
        target_name: session.target_plant.as_str().to_string(),
        plant_kind: session.target_plant.as_str().to_string(),
        mode: session.mode,
        interrupted: true,
        completed: false,
        detail: "结算异常打断".to_string(),
        target_pos: None,
        spirit_quality: 0.0,
        duration_ticks: session.duration_ticks,
        gathering_quality: None,
        tool_used: None,
        overflow_to_ground: false,
        bare_hand_wound: false,
        required_tool_used: false,
        required_tool_kind: None,
    });
}

#[allow(clippy::too_many_arguments)]
pub fn complete_harvest_for_player(
    store: &mut HarvestSessionStore,
    player_id: &str,
    plant_query: &mut Query<&mut Plant, With<Plant>>,
    inventory_query: &mut Query<&mut PlayerInventory, With<valence::prelude::Client>>,
    harvest_hazards: &mut HarvestHazardQuery<'_, '_>,
    kind_registry: &BotanyKindRegistry,
    item_registry: &ItemRegistry,
    allocator: &mut InventoryInstanceIdAllocator,
    snapshot_events: &mut EventWriter<InventorySnapshotRequestEvent>,
    static_points: &mut PlantStaticPointStore,
    terminal_events: &mut EventWriter<HarvestTerminalEvent>,
    skill_events: &mut EventWriter<BotanySkillChangedEvent>,
    skill_xp_events: &mut EventWriter<SkillXpGain>,
    durability_events: &mut EventWriter<InventoryDurabilityChangedEvent>,
    mob_attraction_events: &mut EventWriter<BotanyAttractsMobsEvent>,
    now_tick: u64,
    dropped_loot: Option<&mut DroppedLootRegistry>,
) -> Result<(), String> {
    let pending_session = store
        .session_for(player_id)
        .cloned()
        .ok_or_else(|| format!("missing harvest session for `{player_id}`"))?;
    let target_entity = match pending_session.target_entity {
        Some(target) if store.owns_target(player_id, target) => target,
        _ => {
            let session = store
                .remove_session(player_id)
                .expect("session existed before reservation validation");
            send_structural_cancel_terminal(&session, terminal_events);
            return Err(format!(
                "harvest session for `{player_id}` does not own a live target reservation"
            ));
        }
    };
    let valid_target = plant_query.get(target_entity).is_ok_and(|plant| {
        plant.id == pending_session.target_plant && !plant.harvested && !plant.trampled
    });
    if !valid_target {
        let session = store
            .remove_session(player_id)
            .expect("session existed before target validation");
        send_structural_cancel_terminal(&session, terminal_events);
        return Err(format!(
            "harvest target {target_entity:?} is missing, consumed, or no longer matches `{}`",
            pending_session.target_plant.as_str()
        ));
    }

    let session = store
        .remove_session(player_id)
        .expect("validated harvest session must still exist");

    // plan-botany-harvest-full-inventory-loss-v1 §8.1 决议 #2：结构性校验必须挪到
    // `plant.harvested = true` 这段不可逆副作用之前——否则 kind/inventory 缺失时植物已被
    // 标记收获，随后 lifecycle tick 把它当 wither 回收，玩家却什么都没拿到。任一前置
    // 校验失败，下面的 grant 调用就不会执行，plant.harvested 保持 false 可重收。
    //
    // plan-bughunt-botany-disconnect-session P1：前置校验失败走显式取消语义——session
    // 已被上面移除，若不发终结帧，客户端 HUD 会停在进度满格等一个永远不来的 terminal。
    // 缺 Client 的情形理论上已被 release_disconnected_harvest_sessions 在同帧更早拦截
    // （见 botany/mod.rs 的 .chain() 顺序），这里是权威侧最后一道兜底。
    let kind = match kind_registry.get(session.target_plant) {
        Some(kind) => kind,
        None => {
            send_structural_cancel_terminal(&session, terminal_events);
            return Err(format!(
                "missing kind for `{}`",
                session.target_plant.as_str()
            ));
        }
    };

    let mut inventory = match inventory_query.get_mut(session.client_entity) {
        Ok(inventory) => inventory,
        Err(_) => {
            send_structural_cancel_terminal(&session, terminal_events);
            return Err(format!(
                "player inventory missing on entity {:?}",
                session.client_entity
            ));
        }
    };

    // 博弈 gate major 修复（同根因彻底兑现）：这里只读取 grant / 品质计算需要的字段
    // （position / zone_name / variant），**不**在此处做任何不可逆写入。旧实现在这里就把
    // `plant.harvested = true` 且解绑 static_point——一旦下面的 grant 调用对结构性错误
    // （non-"inventory full:" 的 unknown template / stack_count 0 / no containers /
    // allocator 耗尽 / 无 DroppedLootRegistry 兜底等）`?` 提前返回，植物已被标记收获却什么
    // 都没拿到，与本 PR 要修的原 bug 同形状地静默丢产出。不可逆副作用现在推迟到 grant
    // 成功（`Ok`，含 `DroppedToGround`）之后才执行，见下方对应块。
    let mut target_pos: Option<[f64; 3]> = None;
    let mut target_zone_name: Option<String> = None;
    let mut variant = PlantVariant::None;
    if let Some(target_entity) = session.target_entity {
        if let Ok(plant) = plant_query.get(target_entity) {
            target_pos = Some(plant.position);
            target_zone_name = Some(plant.zone_name.clone());
            variant = plant.variant;
        }
    }

    let actual_tool = crate::tools::main_hand_tool_in_inventory(&inventory);
    let gathering_tool = equipped_gathering_tool(&inventory)
        .filter(|tool| tool.matches_target(GatheringTargetKind::Herb));
    let mut herbalism_quality_bonus = 0.0;
    let mut player_realm = Realm::Awaken;
    if let Ok((cultivation, skill_set, _, _)) = harvest_hazards.get_mut(session.client_entity) {
        player_realm = cultivation
            .as_deref()
            .map(|cultivation| cultivation.realm)
            .unwrap_or(Realm::Awaken);
        herbalism_quality_bonus = super::skill_hook::spirit_quality_bonus(herbalism_effective_lv(
            cultivation.as_deref(),
            skill_set,
        ));
    }
    let gathering_quality_seed = now_tick
        ^ session
            .client_entity
            .to_bits()
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ session.duration_ticks.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    let gathering_quality = roll_quality(
        gathering_quality_seed,
        gathering_tool.map(|tool| tool.material),
        player_realm,
    );

    let harvest_spirit_quality = item_registry
        .get(kind.item_id)
        .map(|template| {
            template.spirit_quality_initial + herbalism_quality_bonus + variant.quality_modifier()
        })
        .unwrap_or(herbalism_quality_bonus + variant.quality_modifier())
        .clamp(0.0, 1.0) as f32;
    let has_instance_modifier = variant != PlantVariant::None || herbalism_quality_bonus > 0.0;
    // plan-botany-harvest-full-inventory-loss-v1 §8.1 决议 #1：原子"入包或掉地"——
    // 满包不再是这个函数的 Err 来源，产物要么进背包要么落地面，永不静默消失。
    let ground_pos = target_pos.unwrap_or(session.origin_position);
    let outcome = add_item_to_player_inventory_or_ground(
        &mut inventory,
        item_registry,
        allocator,
        dropped_loot,
        kind.item_id,
        1,
        now_tick,
        ground_pos,
        DimensionKind::Overworld,
        has_instance_modifier.then_some(
            &(|instance: &mut ItemInstance| {
                apply_harvest_modifiers_to_item(instance, variant, herbalism_quality_bonus)
            }) as &dyn Fn(&mut ItemInstance),
        ),
    )?;
    let overflow_to_ground = matches!(outcome, GrantOrGroundOutcome::DroppedToGround(_));

    // grant 已成功（上面的 `?` 已经通过，含 `DroppedToGround` 兜底）——现在才执行不可逆
    // 副作用：static_point 解绑 + `plant.harvested = true`。任何结构性 `?` 提前返回都不会
    // 走到这里，植物保持 harvested = false，可重收（tripwire 见
    // harvest_completion_grant_structural_failure_leaves_plant_unharvested）。
    if let Some(target_entity) = session.target_entity {
        if let Ok(mut plant) = plant_query.get_mut(target_entity) {
            if let Some(source_point) = plant.source_point {
                if let Some(point) = static_points.get_mut(source_point) {
                    point.bound_entity = None;
                    point.last_spawn_tick = Some(now_tick);
                }
            }
            plant.harvested = true;
        }
    }

    // plan-gathering-tool-bind-v1 P1：required_tool_used 与 tool_used（下方 gathering_tool
    // 派生）是两套正交系统——required_tool 管受伤/耐久，gathering_tool 管采集速度/品质
    // （§8.1 决议 #3）。这里只判定"required_tool 是否命中"，供 HUD/AV 消费。
    let session_required_tool = required_tool_for(session.target_plant, kind_registry);
    let required_tool_matched =
        session_required_tool.is_some_and(|required_tool| actual_tool == Some(required_tool));
    if let Some(required_tool) = session_required_tool {
        if required_tool_matched && gathering_tool.is_none() {
            crate::tools::damage_main_hand_tool(
                session.client_entity,
                &mut inventory,
                durability_events,
                required_tool.durability_cost_ratio_per_use(),
            );
        }
    }

    let mut bare_hand_wound = false;
    if let Ok((cultivation, _skill_set, contamination, wounds)) =
        harvest_hazards.get_mut(session.client_entity)
    {
        let mut cultivation = cultivation;
        let mut contamination = contamination;
        let mut wounds = wounds;
        bare_hand_wound = super::hazard::apply_completion_hazards(
            session.target_plant,
            kind_registry,
            cultivation.as_deref_mut(),
            contamination.as_deref_mut(),
            wounds.as_deref_mut(),
            actual_tool,
            now_tick,
        );
    }

    if let (Some(target_pos), Some(zone_name)) = (target_pos, target_zone_name.as_deref()) {
        for (mob_kind, min_count, max_count) in
            super::hazard::attracts_mobs_hazards_for_kind(session.target_plant, kind_registry)
        {
            mob_attraction_events.send(BotanyAttractsMobsEvent {
                client_entity: session.client_entity,
                plant_kind: session.target_plant,
                zone_name: zone_name.to_string(),
                target_pos,
                mob_kind,
                min_count,
                max_count,
                issued_at_tick: now_tick,
            });
        }
    }

    let base_xp = match session.mode {
        BotanyHarvestMode::Manual => MANUAL_SKILL_XP,
        BotanyHarvestMode::Auto => AUTO_SKILL_XP,
    };
    let xp = base_xp.saturating_add_signed(variant.xp_delta());
    let new_skill = store.add_skill_xp(player_id, xp);
    skill_events.send(BotanySkillChangedEvent {
        client_entity: session.client_entity,
        state: new_skill,
    });
    // plan-skill-v1 §10 botany 钩子：同一笔 XP 同步入 SkillSet（herbalism）。
    // BotanySkillChangedEvent 仍保留给 client 派生视图（plan §5.1 P7 完全退役）。
    let action = match session.mode {
        BotanyHarvestMode::Manual => "harvest_manual",
        BotanyHarvestMode::Auto => "harvest_auto",
    };
    skill_xp_events.send(SkillXpGain {
        char_entity: session.client_entity,
        skill: SkillId::Herbalism,
        amount: xp as u32,
        source: XpGainSource::Action {
            plan_id: "botany",
            action,
        },
    });

    snapshot_events.send(InventorySnapshotRequestEvent {
        client_entity: session.client_entity,
    });
    let target_name_with_variant = variant
        .display_prefix()
        .map(|p| format!("{} · {}", p, session.target_plant.as_str()))
        .unwrap_or_else(|| session.target_plant.as_str().to_string());
    let mut detail = if overflow_to_ground {
        format!(
            "采得 1 株 · 背包已满，已放置于地面 · 灵气流出 {:.3}",
            kind.growth_cost
        )
    } else {
        format!("采得 1 株 · 灵气流出 {:.3}", kind.growth_cost)
    };
    if bare_hand_wound {
        detail.push_str(" · 叶缘割手");
    }
    terminal_events.send(HarvestTerminalEvent {
        client_entity: session.client_entity,
        session_id: session.player_id.clone(),
        target_id: format_target_id(session.target_entity),
        target_name: target_name_with_variant.clone(),
        plant_kind: session.target_plant.as_str().to_string(),
        mode: session.mode,
        interrupted: false,
        completed: true,
        detail,
        target_pos,
        spirit_quality: harvest_spirit_quality,
        duration_ticks: session.duration_ticks,
        gathering_quality: Some(gathering_quality),
        tool_used: gathering_tool.map(|tool| tool.item_id.to_string()),
        overflow_to_ground,
        bare_hand_wound,
        required_tool_used: required_tool_matched,
        required_tool_kind: session_required_tool,
    });
    Ok(())
}

/// 对本次采集产物应用 herb skill / variant 品质修饰与显示名前缀。
fn apply_harvest_modifiers_to_item(
    instance: &mut ItemInstance,
    variant: PlantVariant,
    herbalism_quality_bonus: f64,
) {
    let q = instance.spirit_quality + herbalism_quality_bonus + variant.quality_modifier();
    instance.spirit_quality = q.clamp(0.0, 1.0);
    if let Some(prefix) = variant.display_prefix() {
        instance.display_name = format!("{} · {}", prefix, instance.display_name);
    }
}

// F23 — `pub(crate)` (not private) so `lingtian::systems::handle_start_harvest` can reuse the
// same herbalism-level resolution to gate `SessionMode::Auto` server-side (see botany/components.rs
// `BotanySkillState::auto_unlock_level`). Botany's own harvest flow already used this locally.
pub(crate) fn herbalism_effective_lv(
    cultivation: Option<&Cultivation>,
    skill_set: Option<&SkillSet>,
) -> u8 {
    let real_lv = skill_set
        .and_then(|skill_set| {
            skill_set
                .skills
                .get(&SkillId::Herbalism)
                .map(|entry| entry.lv)
        })
        .unwrap_or(0);
    let cap = cultivation
        .map(|cultivation| skill_cap_for_realm(cultivation.realm))
        .unwrap_or(crate::skill::curve::SKILL_MAX_LEVEL);
    effective_lv(real_lv, cap)
}

fn format_target_id(target_entity: Option<Entity>) -> String {
    target_entity
        .map(|e| format!("plant-{}", e.to_bits()))
        .unwrap_or_default()
}

#[allow(dead_code)]
pub fn queue_harvest_inventory_snapshot(
    events: &mut EventWriter<InventorySnapshotRequestEvent>,
    client_entity: Entity,
) {
    events.send(InventorySnapshotRequestEvent { client_entity });
}

/// plan §1.3 打断 + 踩踏：移动（仅 Manual）或受击 → Session 中止；
/// 中止时按 `BotanyTrampleRoll`（默认 5%）决定目标植物是否被踩死，走 lifecycle 的归还路径。
#[allow(clippy::too_many_arguments)]
pub fn enforce_harvest_session_constraints(
    gameplay_tick: Option<Res<crate::player::gameplay::GameplayTick>>,
    mut store: ResMut<HarvestSessionStore>,
    mut plants: Query<&mut Plant, With<Plant>>,
    client_positions: Query<(Entity, &Position), With<valence::prelude::Client>>,
    kind_registry: Res<BotanyKindRegistry>,
    mut combat_events: EventReader<CombatEvent>,
    trample_roll: Res<BotanyTrampleRoll>,
    mut terminal_events: EventWriter<HarvestTerminalEvent>,
) {
    let Some(gameplay_tick) = gameplay_tick else {
        return;
    };
    let now = gameplay_tick.current_tick();

    let hit_entities: HashSet<Entity> = combat_events.read().map(|ev| ev.target).collect();

    struct InterruptTarget {
        player_id: String,
        client_entity: Entity,
        target_entity: Option<Entity>,
        target_plant: BotanyPlantId,
        mode: BotanyHarvestMode,
        duration_ticks: u64,
        reason: &'static str,
        trampled: bool,
    }

    let mut to_interrupt: Vec<InterruptTarget> = Vec::new();
    for session in store.iter() {
        let hit = hit_entities.contains(&session.client_entity);
        let moved = match session.mode {
            BotanyHarvestMode::Manual => client_positions
                .get(session.client_entity)
                .map(|(_, position)| {
                    let cur = position.get();
                    let [ox, oy, oz] = session.origin_position;
                    let dx = cur.x - ox;
                    let dy = cur.y - oy;
                    let dz = cur.z - oz;
                    dx * dx + dy * dy + dz * dz > MOVEMENT_BREAK_DISTANCE_SQ
                })
                .unwrap_or(false),
            BotanyHarvestMode::Auto => false,
        };

        if !hit && !moved {
            continue;
        }

        let trample_seed = trample_seed_for(
            now,
            session.player_id.as_str(),
            session.target_entity,
            hit,
            moved,
        );
        let trampled = should_trample(trample_seed, trample_roll.chance_inverse);
        let dispersed = super::hazard::should_disperse_on_fail(
            trample_seed ^ 0xD1B5_4A32_D192_ED03,
            super::hazard::failure_dispersal_chance(session.target_plant, kind_registry.as_ref()),
        );
        let reason: &'static str = if hit { "受击打断" } else { "移动打断" };
        to_interrupt.push(InterruptTarget {
            player_id: session.player_id.clone(),
            client_entity: session.client_entity,
            target_entity: session.target_entity,
            target_plant: session.target_plant,
            mode: session.mode,
            duration_ticks: session.duration_ticks,
            reason,
            trampled: trampled || dispersed,
        });
    }

    for target in to_interrupt {
        store.remove_session(target.player_id.as_str());
        let mut target_pos: Option<[f64; 3]> = None;
        if let Some(plant_entity) = target.target_entity {
            if let Ok(mut plant) = plants.get_mut(plant_entity) {
                target_pos = Some(plant.position);
                if target.trampled {
                    plant.trampled = true;
                }
            }
        }
        let detail = if target.trampled {
            format!("{} · 目标被踩死", target.reason)
        } else {
            target.reason.to_string()
        };
        terminal_events.send(HarvestTerminalEvent {
            client_entity: target.client_entity,
            session_id: target.player_id.clone(),
            target_id: format_target_id(target.target_entity),
            target_name: target.target_plant.as_str().to_string(),
            plant_kind: target.target_plant.as_str().to_string(),
            mode: target.mode,
            interrupted: true,
            completed: false,
            detail,
            target_pos,
            spirit_quality: 0.0,
            duration_ticks: target.duration_ticks,
            gathering_quality: None,
            tool_used: None,
            overflow_to_ground: false,
            bare_hand_wound: false,
            required_tool_used: false,
            required_tool_kind: None,
        });
    }
}

/// plan-bughunt-botany-disconnect-session P0 方案 A：断线即取消 botany 采集 session。
///
/// 消费 `RemovedComponents<Client>`——valence 在客户端连接丢失时移除该组件；
/// `player::despawn_disconnected_clients` 也读同一信号做玩家持久化，两个系统各自持有
/// 独立的 reader cursor 互不影响（范式同 `world::container_open::release_disconnected_container_locks`，
/// 该系统已用相同机制清理断线容器占用锁）。
///
/// 必须排在 `tick_harvest_sessions` 之前跑（见 `botany/mod.rs` 的 `.chain()` 顺序）：
/// 否则断线当帧若 session 恰好到达完成 tick，`complete_harvest_for_player` 会先
/// `remove_session` 再因旧实体缺 `Client`/`PlayerInventory` 失败，静默吞掉玩家已等待的
/// 采集进度——这是本 bug 的核心触发路径。这里抢先移除 session 并发送
/// `interrupted=true` 的终结事件，让 `tick_harvest_sessions` 在同一 tick 内再也看不到
/// 该 session；同时清掉旧 `client_entity` 对该 `player_id` 的占位，玩家重连后
/// `start_or_resume_harvest` 能立刻用新实体重新开始，不会被旧 session 卡住。
///
/// 不清理 `HarvestSessionStore::skills_by_player`——断线只取消进行中的采集动作，
/// 已经获得的采集熟练度 XP 是玩家的既得进度，不随断线清零。
pub fn release_disconnected_harvest_sessions(
    mut disconnected_clients: RemovedComponents<valence::prelude::Client>,
    mut store: ResMut<HarvestSessionStore>,
    mut terminal_events: EventWriter<HarvestTerminalEvent>,
) {
    for entity in disconnected_clients.read() {
        let Some(player_id) = store
            .iter()
            .find(|session| session.client_entity == entity)
            .map(|session| session.player_id.clone())
        else {
            continue;
        };

        let Some(session) = store.remove_session(player_id.as_str()) else {
            continue;
        };

        tracing::info!(
            "[bong][botany] cancelling harvest session for `{player_id}` — client {entity:?} disconnected mid-harvest"
        );

        terminal_events.send(HarvestTerminalEvent {
            client_entity: session.client_entity,
            session_id: session.player_id.clone(),
            target_id: format_target_id(session.target_entity),
            target_name: session.target_plant.as_str().to_string(),
            plant_kind: session.target_plant.as_str().to_string(),
            mode: session.mode,
            interrupted: true,
            completed: false,
            detail: "断线打断".to_string(),
            target_pos: None,
            spirit_quality: 0.0,
            duration_ticks: session.duration_ticks,
            gathering_quality: None,
            tool_used: None,
            overflow_to_ground: false,
            bare_hand_wound: false,
            required_tool_used: false,
            required_tool_kind: None,
        });
    }
}

fn trample_seed_for(
    now_tick: u64,
    player_id: &str,
    target_entity: Option<Entity>,
    hit: bool,
    moved: bool,
) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    player_id.hash(&mut hasher);
    let player_hash = hasher.finish();

    let target_bits = target_entity.map(|e| e.to_bits()).unwrap_or(0);
    let cause_bit = (u64::from(hit)) | (u64::from(moved) << 1);

    now_tick.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ player_hash.wrapping_mul(0xBF58_476D_1CE4_E5B9)
        ^ target_bits.wrapping_mul(0x94D0_49BB_1331_11EB)
        ^ cause_bit
}

fn should_trample(seed: u64, chance_inverse: u32) -> bool {
    if chance_inverse == 0 {
        return false;
    }
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    z.is_multiple_of(u64::from(chance_inverse))
}

#[allow(clippy::too_many_arguments)]
pub fn tick_harvest_sessions(
    gameplay_tick: Option<Res<crate::player::gameplay::GameplayTick>>,
    mut store: ResMut<HarvestSessionStore>,
    mut plants: Query<&mut Plant, With<Plant>>,
    mut inventories: Query<&mut PlayerInventory, With<valence::prelude::Client>>,
    mut harvest_hazards: HarvestHazardQuery<'_, '_>,
    kind_registry: Res<BotanyKindRegistry>,
    item_registry: Res<ItemRegistry>,
    mut allocator: ResMut<InventoryInstanceIdAllocator>,
    mut snapshot_events: EventWriter<InventorySnapshotRequestEvent>,
    mut static_points: ResMut<PlantStaticPointStore>,
    mut terminal_events: EventWriter<HarvestTerminalEvent>,
    mut skill_events: EventWriter<BotanySkillChangedEvent>,
    mut skill_xp_events: EventWriter<SkillXpGain>,
    mut durability_events: EventWriter<InventoryDurabilityChangedEvent>,
    mut mob_attraction_events: EventWriter<BotanyAttractsMobsEvent>,
    mut dropped_loot: Option<ResMut<DroppedLootRegistry>>,
) {
    let Some(gameplay_tick) = gameplay_tick else {
        return;
    };

    let now = gameplay_tick.current_tick();
    let completed = store
        .iter()
        .filter(|session| session.progress_at(now) >= 1.0)
        .map(|session| session.player_id.clone())
        .collect::<Vec<_>>();

    for player_id in completed {
        if let Err(err) = complete_harvest_for_player(
            &mut store,
            player_id.as_str(),
            &mut plants,
            &mut inventories,
            &mut harvest_hazards,
            kind_registry.as_ref(),
            item_registry.as_ref(),
            &mut allocator,
            &mut snapshot_events,
            &mut static_points,
            &mut terminal_events,
            &mut skill_events,
            &mut skill_xp_events,
            &mut durability_events,
            &mut mob_attraction_events,
            now,
            dropped_loot.as_deref_mut(),
        ) {
            // plan-bughunt-botany-disconnect-session P1：文案与实际状态一致——session
            // 已取消（不会自动 retry），植物保持未收获，玩家需重新发起采集。
            tracing::warn!(
                "[bong][botany] harvest completion failed for `{player_id}`: {err} — \
                 session cancelled, plant left un-harvested; player must restart the harvest"
            );
        }
    }
}

fn required_tool_for(
    plant_id: BotanyPlantId,
    registry: &BotanyKindRegistry,
) -> Option<crate::tools::ToolKind> {
    let kind = registry.get(plant_id)?;
    let spec = kind.v2_spec()?;
    // 当前 required_tool 只存在于 WoundOnBareHand；新增带工具要求的 hazard variant 时必须扩展这里。
    spec.harvest_hazards.iter().find_map(|hazard| match hazard {
        super::registry::HarvestHazard::WoundOnBareHand { required_tool, .. } => *required_tool,
        _ => None,
    })
}

/// plan §1.3 踩踏主规则：玩家（Client entity）水平靠近活体植物时，每次"进入"近邻范围
/// 掷一次骰子（edge-triggered），命中则 plant.trampled = true，下一 lifecycle tick 自然凋零并归还 spirit_qi。
///
/// Edge-triggered 的关键是 `PlantProximityTracker.in_range` —— 仅当 `(client, plant)`
/// 对本 tick 首次出现在近邻集合里才掷骰；停留在植物上并不会连掷。
pub fn detect_non_session_trample(
    gameplay_tick: Option<Res<crate::player::gameplay::GameplayTick>>,
    trample_roll: Res<BotanyTrampleRoll>,
    mut tracker: ResMut<PlantProximityTracker>,
    mut plants: Query<(Entity, &mut Plant)>,
    clients: Query<(Entity, &Position), With<valence::prelude::Client>>,
) {
    let Some(gameplay_tick) = gameplay_tick else {
        return;
    };
    let now = gameplay_tick.current_tick();

    let mut current: HashSet<(Entity, Entity)> = HashSet::new();
    let mut to_trample: Vec<Entity> = Vec::new();

    // 快照植物坐标避免借用冲突
    let plant_snapshots: Vec<(Entity, [f64; 3], bool, bool)> = plants
        .iter()
        .map(|(entity, plant)| (entity, plant.position, plant.harvested, plant.trampled))
        .collect();

    for (client_entity, client_pos) in clients.iter() {
        let cp = client_pos.get();
        for &(plant_entity, pos, harvested, already_trampled) in &plant_snapshots {
            if harvested || already_trampled {
                continue;
            }
            let dx = cp.x - pos[0];
            let dy = cp.y - pos[1];
            let dz = cp.z - pos[2];
            if dy.abs() > TRAMPLE_VERTICAL_MAX {
                continue;
            }
            if dx * dx + dz * dz > TRAMPLE_RADIUS_SQ {
                continue;
            }
            let pair = (client_entity, plant_entity);
            let is_new = !tracker.in_range.contains(&pair);
            current.insert(pair);
            if !is_new {
                continue;
            }
            let seed = trample_seed_for(now, "", Some(plant_entity), false, true)
                ^ client_entity.to_bits().wrapping_mul(0xCBF2_9CE4_8422_2325);
            if should_trample(seed, trample_roll.chance_inverse) {
                to_trample.push(plant_entity);
            }
        }
    }

    for plant_entity in to_trample {
        if let Ok((_, mut plant)) = plants.get_mut(plant_entity) {
            plant.trampled = true;
        }
    }

    tracker.in_range = current;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::botany::components::PlantLifecycleClock;
    use crate::combat::components::{BodyPart, WoundKind, Wounds};
    use crate::cultivation::components::{Contamination, Cultivation, Realm};
    use crate::inventory::{
        add_item_to_player_inventory, dropped_loot_snapshot, load_item_registry, ContainerState,
        InventoryInstanceIdAllocator, InventoryRevision, ItemInstance, ItemRarity, PlacedItemState,
        PlayerInventory, EQUIP_SLOT_MAIN_HAND, MAIN_PACK_CONTAINER_ID,
    };
    use crate::player::gameplay::GameplayTick;
    use crate::skill::components::{SkillEntry, SkillSet};
    use crate::world::zone::ZoneRegistry;
    use std::collections::HashMap;
    use valence::prelude::{App, Events, IntoSystemConfigs, Update};
    use valence::testing::create_mock_client;

    /// plan-skill-v1 §7.1 botany 行 XP 数值锚点：野外采集 手动 +2 · 自动 +5。
    /// 若此测试挂掉意味着有人偷偷改了 skill source-of-truth 数值。
    #[test]
    fn harvest_xp_constants_match_skill_plan_section_seven_one() {
        assert_eq!(
            MANUAL_SKILL_XP, 2,
            "野外采集 手动 须 = 2（plan-skill §7.1）"
        );
        assert_eq!(AUTO_SKILL_XP, 5, "野外采集 自动 须 = 5（plan-skill §7.1）");
    }

    fn plant_entity(app: &mut App, zone_name: &str) -> Entity {
        plant_entity_with_id_and_variant(
            app,
            zone_name,
            BotanyPlantId::CiSheHao,
            PlantVariant::None,
        )
    }

    fn plant_entity_with_variant(app: &mut App, zone_name: &str, variant: PlantVariant) -> Entity {
        plant_entity_with_id_and_variant(app, zone_name, BotanyPlantId::CiSheHao, variant)
    }

    fn plant_entity_with_id(app: &mut App, zone_name: &str, plant_id: BotanyPlantId) -> Entity {
        plant_entity_with_id_and_variant(app, zone_name, plant_id, PlantVariant::None)
    }

    fn plant_entity_with_id_and_variant(
        app: &mut App,
        zone_name: &str,
        plant_id: BotanyPlantId,
        variant: PlantVariant,
    ) -> Entity {
        app.world_mut()
            .spawn(Plant {
                id: plant_id,
                zone_name: zone_name.to_string(),
                position: [10.0, 64.0, 10.0],
                planted_at_tick: 0,
                wither_progress: 0,
                source_point: None,
                harvested: false,
                trampled: false,
                variant,
            })
            .id()
    }

    fn empty_inventory_8x8() -> PlayerInventory {
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                quick_access: false,
                id: MAIN_PACK_CONTAINER_ID.into(),
                name: "main".into(),
                rows: 8,
                cols: 8,
                items: Vec::new(),
                owner_instance_id: None,
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 999.0,
        }
    }

    fn tool_item(template_id: &str, durability: f64) -> ItemInstance {
        ItemInstance {
            instance_id: 9_001,
            template_id: template_id.to_string(),
            display_name: template_id.to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.1,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 0.0,
            durability,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        }
    }

    /// plan-botany-harvest-full-inventory-loss-v1 P0 测试专用：1x1 容器，唯一格已被
    /// 别的物品占满，保证后续任何 grant 都会走
    /// `add_item_to_player_inventory_or_ground` 的地面 fallback 分支。
    fn full_1x1_inventory_blocking(occupant_template_id: &str) -> PlayerInventory {
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                quick_access: false,
                id: MAIN_PACK_CONTAINER_ID.into(),
                name: "main".into(),
                rows: 1,
                cols: 1,
                items: vec![PlacedItemState {
                    row: 0,
                    col: 0,
                    instance: tool_item(occupant_template_id, 1.0),
                }],
                owner_instance_id: None,
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 999.0,
        }
    }

    fn inventory_with_main_hand_tool(template_id: Option<&str>) -> PlayerInventory {
        inventory_with_main_hand_tool_durability(template_id, 1.0)
    }

    fn inventory_with_main_hand_tool_durability(
        template_id: Option<&str>,
        durability: f64,
    ) -> PlayerInventory {
        let mut inventory = empty_inventory_8x8();
        if let Some(template_id) = template_id {
            inventory.equipped.insert(
                EQUIP_SLOT_MAIN_HAND.to_string(),
                crate::inventory::SlotContents::held_single(tool_item(template_id, durability)),
            );
        }
        inventory
    }

    fn make_app_with_combat_events() -> App {
        let mut app = App::new();
        app.insert_resource(BotanyKindRegistry::default());
        app.insert_resource(PlantStaticPointStore::default());
        app.insert_resource(PlantLifecycleClock::default());
        app.insert_resource(HarvestSessionStore::default());
        app.insert_resource(PlantProximityTracker::default());
        app.insert_resource(BotanyTrampleRoll { chance_inverse: 1 }); // 100% trample
        app.insert_resource(GameplayTick::default());
        app.insert_resource(ZoneRegistry::fallback());
        app.add_event::<CombatEvent>();
        app.add_event::<InventorySnapshotRequestEvent>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_event::<HarvestTerminalEvent>();
        app.add_event::<BotanySkillChangedEvent>();
        app.add_event::<BotanyAttractsMobsEvent>();
        app.add_event::<SkillXpGain>();
        app
    }

    fn queue_completed_ci_she_harvest(app: &mut App, client_entity: Entity, target: Entity) {
        queue_completed_ci_she_harvest_with_mode(
            app,
            client_entity,
            target,
            BotanyHarvestMode::Manual,
        );
    }

    /// plan-botany-harvest-full-inventory-loss-v1 P2：Auto/Manual 各自触发满包 fallback 对照。
    fn queue_completed_ci_she_harvest_with_mode(
        app: &mut App,
        client_entity: Entity,
        target: Entity,
        mode: BotanyHarvestMode,
    ) {
        app.world_mut()
            .resource_mut::<HarvestSessionStore>()
            .upsert_session(HarvestSession {
                player_id: "offline:Azure".to_string(),
                client_entity,
                target_entity: Some(target),
                target_plant: BotanyPlantId::CiSheHao,
                mode,
                started_at_tick: 0,
                duration_ticks: 0,
                phase: BotanyPhase::InProgress,
                last_progress: 0.0,
                origin_position: [10.0, 64.0, 10.0],
            });
    }

    #[test]
    fn session_progress_completes_after_duration() {
        let mut store = HarvestSessionStore::default();
        start_or_resume_harvest(
            &mut store,
            "Azure",
            Entity::from_raw(1),
            Some(Entity::from_raw(2)),
            BotanyPlantId::CiSheHao,
            BotanyHarvestMode::Manual,
            [0.0, 0.0, 0.0],
            10,
        );

        let session = store.session_for("offline:Azure").unwrap();
        assert!(session.progress_at(51) >= 1.0);
    }

    #[test]
    fn mode_request_updates_existing_session_duration_and_progress_origin() {
        let mut store = HarvestSessionStore::default();
        start_or_resume_harvest(
            &mut store,
            "Azure",
            Entity::from_raw(1),
            Some(Entity::from_raw(2)),
            BotanyPlantId::CiSheHao,
            BotanyHarvestMode::Manual,
            [0.0, 0.0, 0.0],
            10,
        );

        request_harvest_mode(
            &mut store,
            "offline:Azure",
            Entity::from_raw(1),
            BotanyHarvestMode::Auto,
            25,
        )
        .expect("existing session should accept mode request");

        let session = store.session_for("offline:Azure").unwrap();
        assert_eq!(session.mode, BotanyHarvestMode::Auto);
        assert_eq!(
            session.duration_ticks,
            harvest_duration_ticks_for(BotanyHarvestMode::Auto)
        );
        assert_eq!(session.started_at_tick, 25);
        assert_eq!(session.last_progress, 0.0);
        assert_eq!(session.phase, BotanyPhase::InProgress);
    }

    #[test]
    fn mode_request_rejects_session_from_different_client_entity() {
        let mut store = HarvestSessionStore::default();
        start_or_resume_harvest(
            &mut store,
            "Azure",
            Entity::from_raw(1),
            Some(Entity::from_raw(2)),
            BotanyPlantId::CiSheHao,
            BotanyHarvestMode::Manual,
            [0.0, 0.0, 0.0],
            10,
        );

        let err = request_harvest_mode(
            &mut store,
            "offline:Azure",
            Entity::from_raw(99),
            BotanyHarvestMode::Auto,
            25,
        )
        .expect_err("session_id from another client must be rejected");

        assert!(err.contains("belongs to"));
        let session = store.session_for("offline:Azure").unwrap();
        assert_eq!(session.mode, BotanyHarvestMode::Manual);
        assert_eq!(
            session.duration_ticks,
            harvest_duration_ticks_for(BotanyHarvestMode::Manual)
        );
        assert_eq!(session.started_at_tick, 10);
    }

    #[test]
    fn mode_request_rejects_missing_session_without_creating_one() {
        let mut store = HarvestSessionStore::default();

        let err = request_harvest_mode(
            &mut store,
            "offline:Azure",
            Entity::from_raw(1),
            BotanyHarvestMode::Auto,
            25,
        )
        .expect_err("missing session_id must be rejected");

        assert!(err.contains("missing harvest session"));
        assert!(
            store.session_for("offline:Azure").is_none(),
            "mode request must not create a new harvest session"
        );
        assert_eq!(
            store.iter().count(),
            0,
            "missing mode request must leave the session store empty"
        );
    }

    #[test]
    fn completed_harvest_applies_herbalism_quality_bonus_using_effective_level() {
        let mut app = make_app_with_combat_events();
        app.insert_resource(load_item_registry().expect("item registry should load"));
        app.insert_resource(InventoryInstanceIdAllocator::default());
        app.add_systems(Update, tick_harvest_sessions);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let mut skill_set = SkillSet::default();
        skill_set.skills.insert(
            SkillId::Herbalism,
            SkillEntry {
                lv: 7,
                ..Default::default()
            },
        );
        let client_entity = app
            .world_mut()
            .spawn(client_bundle)
            .insert(empty_inventory_8x8())
            .insert(Cultivation {
                realm: Realm::Awaken,
                ..Default::default()
            })
            .insert(Contamination::default())
            .insert(Wounds::default())
            .insert(skill_set)
            .id();
        let target = plant_entity(&mut app, "spawn");

        {
            let mut store = app.world_mut().resource_mut::<HarvestSessionStore>();
            store.upsert_session(HarvestSession {
                player_id: "offline:Azure".to_string(),
                client_entity,
                target_entity: Some(target),
                target_plant: BotanyPlantId::CiSheHao,
                mode: BotanyHarvestMode::Manual,
                started_at_tick: 0,
                duration_ticks: 0,
                phase: BotanyPhase::InProgress,
                last_progress: 0.0,
                origin_position: [10.0, 64.0, 10.0],
            });
        }

        app.update();

        let base_quality = app
            .world()
            .resource::<ItemRegistry>()
            .get("ci_she_hao")
            .expect("ci_she_hao template should exist")
            .spirit_quality_initial;
        let inventory = app
            .world()
            .entity(client_entity)
            .get::<PlayerInventory>()
            .expect("client should have inventory");
        let harvested = inventory
            .containers
            .iter()
            .find(|container| container.id == MAIN_PACK_CONTAINER_ID)
            .and_then(|container| {
                container
                    .items
                    .iter()
                    .find(|placed| placed.instance.template_id == "ci_she_hao")
            })
            .expect("harvested herb should be inserted into main pack");

        let expected = base_quality + crate::botany::skill_hook::spirit_quality_bonus(3);
        assert!(
            (harvested.instance.spirit_quality - expected).abs() < 1e-6,
            "harvested spirit_quality should use effective herbalism Lv.3, got {} expected {}",
            harvested.instance.spirit_quality,
            expected
        );
    }

    #[test]
    fn variant_harvest_merges_only_matching_modified_stacks() {
        let mut app = make_app_with_combat_events();
        app.insert_resource(load_item_registry().expect("item registry should load"));
        app.insert_resource(InventoryInstanceIdAllocator::default());
        app.add_systems(Update, tick_harvest_sessions);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let client_entity = app
            .world_mut()
            .spawn(client_bundle)
            .insert(empty_inventory_8x8())
            .insert(Cultivation::default())
            .insert(Contamination::default())
            .insert(Wounds::default())
            .id();

        for variant in [
            PlantVariant::Thunder,
            PlantVariant::Thunder,
            PlantVariant::Tainted,
        ] {
            let target = plant_entity_with_variant(&mut app, "spawn", variant);
            queue_completed_ci_she_harvest(&mut app, client_entity, target);
            app.update();
        }

        let base_quality = app
            .world()
            .resource::<ItemRegistry>()
            .get("ci_she_hao")
            .expect("ci_she_hao template should exist")
            .spirit_quality_initial;
        let inventory = app
            .world()
            .entity(client_entity)
            .get::<PlayerInventory>()
            .expect("client should have inventory");
        let main_pack = inventory
            .containers
            .iter()
            .find(|container| container.id == MAIN_PACK_CONTAINER_ID)
            .expect("main pack should exist");
        let herbs: Vec<_> = main_pack
            .items
            .iter()
            .filter(|placed| placed.instance.template_id == "ci_she_hao")
            .collect();

        assert_eq!(herbs.len(), 2);
        let thunder = herbs
            .iter()
            .find(|placed| placed.instance.display_name.starts_with("雷 · "))
            .expect("thunder herbs should share one modified stack");
        assert_eq!(thunder.instance.stack_count, 2);
        assert_eq!(thunder.instance.display_name.matches("雷 ·").count(), 1);
        assert!(
            (thunder.instance.spirit_quality - (base_quality + 0.10).clamp(0.0, 1.0)).abs() < 1e-6,
            "thunder stack quality should apply its modifier once"
        );

        let tainted = herbs
            .iter()
            .find(|placed| placed.instance.display_name.starts_with("黑 · "))
            .expect("tainted herb should stay isolated from thunder stack");
        assert_eq!(tainted.instance.stack_count, 1);
        assert_eq!(tainted.instance.display_name.matches("黑 ·").count(), 1);
        assert!(
            (tainted.instance.spirit_quality - (base_quality - 0.15).clamp(0.0, 1.0)).abs() < 1e-6,
            "tainted stack quality should apply its modifier once"
        );
    }

    // ─────────────────────────────────────────────────────────────────
    // plan-botany-harvest-full-inventory-loss-v1 §P0/§P2 — 满包不丢产出
    // ─────────────────────────────────────────────────────────────────

    #[test]
    fn harvest_completion_overflow_drops_to_ground_when_inventory_full() {
        let mut app = make_app_with_combat_events();
        app.insert_resource(load_item_registry().expect("item registry should load"));
        app.insert_resource(InventoryInstanceIdAllocator::default());
        app.insert_resource(DroppedLootRegistry::default());
        app.add_systems(Update, tick_harvest_sessions);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let client_entity = app
            .world_mut()
            .spawn(client_bundle)
            .insert(full_1x1_inventory_blocking("filler"))
            .insert(Cultivation::default())
            .insert(Contamination::default())
            .insert(Wounds::default())
            .id();
        let target = plant_entity(&mut app, "spawn");
        queue_completed_ci_she_harvest(&mut app, client_entity, target);

        app.update();

        let dropped = app.world().resource::<DroppedLootRegistry>();
        assert_eq!(
            dropped.entries.len(),
            1,
            "overflow product must land in DroppedLootRegistry instead of vanishing when the pack is full"
        );
        let entry = dropped.entries.values().next().expect("one overflow entry");
        assert_eq!(entry.item.template_id, "ci_she_hao");

        let frames: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<HarvestTerminalEvent>>()
            .drain()
            .collect();
        assert_eq!(
            frames.len(),
            1,
            "exactly one terminal event for the completed harvest"
        );
        assert!(
            frames[0].overflow_to_ground,
            "terminal event should flag overflow_to_ground when the grant fell back to ground"
        );
        assert!(
            frames[0].detail.contains("背包已满"),
            "detail text should mention 背包已满 so the player understands why nothing landed in the pack, got {:?}",
            frames[0].detail
        );

        let plant = app
            .world()
            .entity(target)
            .get::<Plant>()
            .expect("plant entity should still exist");
        assert!(
            plant.harvested,
            "plant is consumed even though the product went to ground — §8.1 决议 #1: full-but-successful grant still counts as a completed harvest, unlike a structural failure"
        );
    }

    #[test]
    fn harvest_completion_non_full_inventory_grants_normally_no_overflow() {
        let mut app = make_app_with_combat_events();
        app.insert_resource(load_item_registry().expect("item registry should load"));
        app.insert_resource(InventoryInstanceIdAllocator::default());
        app.insert_resource(DroppedLootRegistry::default());
        app.add_systems(Update, tick_harvest_sessions);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let client_entity = app
            .world_mut()
            .spawn(client_bundle)
            .insert(empty_inventory_8x8())
            .insert(Cultivation::default())
            .insert(Contamination::default())
            .insert(Wounds::default())
            .id();
        let target = plant_entity(&mut app, "spawn");
        queue_completed_ci_she_harvest(&mut app, client_entity, target);

        app.update();

        let dropped = app.world().resource::<DroppedLootRegistry>();
        assert!(
            dropped.entries.is_empty(),
            "control group: a non-full inventory must never spill to ground"
        );

        let frames: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<HarvestTerminalEvent>>()
            .drain()
            .collect();
        assert_eq!(frames.len(), 1);
        assert!(
            !frames[0].overflow_to_ground,
            "control group terminal event must not flag overflow_to_ground"
        );
        assert!(
            !frames[0].detail.contains("背包已满"),
            "non-overflow detail text must not mention 背包已满, got {:?}",
            frames[0].detail
        );

        let inventory = app
            .world()
            .get::<PlayerInventory>(client_entity)
            .expect("client should have inventory");
        let has_item = inventory.containers.iter().any(|c| {
            c.items
                .iter()
                .any(|p| p.instance.template_id == "ci_she_hao")
        });
        assert!(
            has_item,
            "product should land in the player's inventory as usual"
        );
    }

    #[test]
    fn harvest_completion_missing_kind_registry_leaves_plant_unharvested() {
        let mut app = make_app_with_combat_events();
        // 覆盖 make_app_with_combat_events 里的默认注册表——制造 kind_registry.get 失败分支。
        app.insert_resource(BotanyKindRegistry::empty());
        app.insert_resource(load_item_registry().expect("item registry should load"));
        app.insert_resource(InventoryInstanceIdAllocator::default());
        app.insert_resource(DroppedLootRegistry::default());
        app.add_systems(Update, tick_harvest_sessions);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let client_entity = app
            .world_mut()
            .spawn(client_bundle)
            .insert(empty_inventory_8x8())
            .insert(Cultivation::default())
            .insert(Contamination::default())
            .insert(Wounds::default())
            .id();
        let target = plant_entity(&mut app, "spawn");
        queue_completed_ci_she_harvest(&mut app, client_entity, target);

        app.update();

        let plant = app
            .world()
            .entity(target)
            .get::<Plant>()
            .expect("plant entity should still exist");
        assert!(
            !plant.harvested,
            "structural failure (missing kind) must happen before plant.harvested is set, \
             so the plant stays re-harvestable — §8.1 决议 #2"
        );

        let store = app.world().resource::<HarvestSessionStore>();
        assert!(
            store.session_for("offline:Azure").is_none(),
            "session should still be cleared even on structural failure (light rollback, not data loss)"
        );

        let dropped = app.world().resource::<DroppedLootRegistry>();
        assert!(
            dropped.entries.is_empty(),
            "no product should ever be created when kind lookup fails before any grant is attempted"
        );

        // plan-bughunt-botany-disconnect-session P1 显式取消语义：missing-kind 与
        // missing-inventory 同属结构性前置校验失败，必须补发 interrupted 终结帧。
        let frames: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<HarvestTerminalEvent>>()
            .drain()
            .collect();
        assert_eq!(
            frames.len(),
            1,
            "missing-kind failure must emit exactly one cancellation terminal frame \
             (explicit cancel, not silent swallow), got {frames:?}"
        );
        assert!(
            frames[0].interrupted && !frames[0].completed,
            "must be an interrupt frame because nothing was granted, \
             got interrupted={} completed={}",
            frames[0].interrupted,
            frames[0].completed
        );
        assert_eq!(
            frames[0].detail, "结算异常打断",
            "detail must state the structural-failure cancellation reason"
        );
    }

    #[test]
    fn harvest_completion_missing_player_inventory_leaves_plant_unharvested() {
        // plan-bughunt-botany-disconnect-session P2：本测试**不再**覆盖断线场景——断线
        // 语义由 release_disconnected_harvest_sessions 的专属测试组锁定。这里 pin 的是
        // 纯结构性装配缺陷（实体带 Client 但缺 PlayerInventory）下完成路径的显式取消
        // 语义：植物不收获、session 清掉、且必须补发 interrupted 终结帧而非静默失败。
        let mut app = make_app_with_combat_events();
        app.insert_resource(load_item_registry().expect("item registry should load"));
        app.insert_resource(InventoryInstanceIdAllocator::default());
        app.insert_resource(DroppedLootRegistry::default());
        app.add_systems(Update, tick_harvest_sessions);

        let (client_bundle, _helper) = create_mock_client("Azure");
        // 故意不 insert PlayerInventory —— 模拟系统装配缺陷 / 实体生命周期竞态。
        let client_entity = app
            .world_mut()
            .spawn(client_bundle)
            .insert(Cultivation::default())
            .insert(Contamination::default())
            .insert(Wounds::default())
            .id();
        let target = plant_entity(&mut app, "spawn");
        queue_completed_ci_she_harvest(&mut app, client_entity, target);

        app.update();

        let plant = app
            .world()
            .entity(target)
            .get::<Plant>()
            .expect("plant entity should still exist");
        assert!(
            !plant.harvested,
            "structural failure (missing inventory) must happen before plant.harvested is set"
        );

        let store = app.world().resource::<HarvestSessionStore>();
        assert!(
            store.session_for("offline:Azure").is_none(),
            "session must be cancelled (removed) on structural precheck failure — \
             keeping it would retry-loop the completion path every tick"
        );

        // P1 显式取消语义：结构性前置校验失败必须补发 interrupted 终结帧，
        // 否则客户端 HUD 停在进度满格永远等不到收口。
        let frames: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<HarvestTerminalEvent>>()
            .drain()
            .collect();
        assert_eq!(
            frames.len(),
            1,
            "structural precheck failure must emit exactly one cancellation terminal frame \
             (explicit cancel, not silent swallow), got {frames:?}"
        );
        let frame = &frames[0];
        assert!(
            frame.interrupted && !frame.completed,
            "the terminal frame must be an interrupt (interrupted=true, completed=false) \
             because nothing was granted, got interrupted={} completed={}",
            frame.interrupted,
            frame.completed
        );
        assert_eq!(
            frame.detail, "结算异常打断",
            "detail must state the structural-failure cancellation reason"
        );
        assert_eq!(frame.session_id, "offline:Azure");
        assert_eq!(frame.client_entity, client_entity);
    }

    /// 博弈 gate major 的 tripwire：满包 + 故意不 insert `DroppedLootRegistry`（让
    /// `tick_harvest_sessions` 的 `Option<ResMut<DroppedLootRegistry>>` 解析为
    /// `None`）让 `add_item_to_player_inventory_or_ground` 走"背包已满但无
    /// `DroppedLootRegistry` 可兜底"的结构性 `Err` 分支（见 inventory/mod.rs：
    /// `dropped_loot` 为 `None` 时把 `"inventory full:"` 错误原样 wrap 成
    /// `Err("inventory full and no DroppedLootRegistry available to fall back: ...")`，
    /// 而不是吸收成 `Ok(DroppedToGround)`）。
    ///
    /// 重排前（§8.1 决议 #2 落地前），`plant.harvested = true` 发生在这次 grant 调用**之前**，
    /// 所以这条路径会在植物已被标记收获之后才失败——产物没给玩家，植物却已注定被
    /// lifecycle 当已收获回收，是与本 PR 修的原 bug 同形状的静默丢产出。重排后 grant 在
    /// `plant.harvested` 写入之前执行，这条测试断言它必须仍是 `false`。
    #[test]
    fn harvest_completion_grant_structural_failure_leaves_plant_unharvested() {
        let mut app = make_app_with_combat_events();
        app.insert_resource(load_item_registry().expect("item registry should load"));
        app.insert_resource(InventoryInstanceIdAllocator::default());
        // 故意不 app.insert_resource(DroppedLootRegistry::default()) —— 这是本测试制造
        // 结构性失败的关键：满包 + 无 DroppedLootRegistry 兜底 = grant 结构性 Err。
        app.add_systems(Update, tick_harvest_sessions);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let client_entity = app
            .world_mut()
            .spawn(client_bundle)
            .insert(full_1x1_inventory_blocking("filler"))
            .insert(Cultivation::default())
            .insert(Contamination::default())
            .insert(Wounds::default())
            .id();
        let target = plant_entity(&mut app, "spawn");
        queue_completed_ci_she_harvest(&mut app, client_entity, target);

        app.update();

        let plant = app
            .world()
            .entity(target)
            .get::<Plant>()
            .expect("plant entity should still exist");
        assert!(
            !plant.harvested,
            "grant hit a structural Err (full pack, no DroppedLootRegistry to fall back to) — \
             plant.harvested must still be false so the plant stays re-harvestable. Before the \
             §8.1 #2 reorder this assertion would fail (harvested was flipped to true before the \
             grant call ran) — that regression is exactly what this tripwire pins."
        );

        let store = app.world().resource::<HarvestSessionStore>();
        assert!(
            store.session_for("offline:Azure").is_none(),
            "session should still be cleared even on structural failure (light rollback, not data loss)"
        );

        let frames: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<HarvestTerminalEvent>>()
            .drain()
            .collect();
        assert!(
            frames.is_empty(),
            "no terminal event should fire when the grant itself failed structurally, got {frames:?}"
        );
    }

    #[test]
    fn harvest_completion_variant_and_quality_modifiers_survive_overflow() {
        let mut app = make_app_with_combat_events();
        app.insert_resource(load_item_registry().expect("item registry should load"));
        app.insert_resource(InventoryInstanceIdAllocator::default());
        app.insert_resource(DroppedLootRegistry::default());
        app.add_systems(Update, tick_harvest_sessions);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let mut skill_set = SkillSet::default();
        skill_set.skills.insert(
            SkillId::Herbalism,
            SkillEntry {
                lv: 7,
                ..Default::default()
            },
        );
        let client_entity = app
            .world_mut()
            .spawn(client_bundle)
            .insert(full_1x1_inventory_blocking("filler"))
            .insert(Cultivation {
                realm: Realm::Awaken,
                ..Default::default()
            })
            .insert(Contamination::default())
            .insert(Wounds::default())
            .insert(skill_set)
            .id();
        let target = plant_entity_with_variant(&mut app, "spawn", PlantVariant::Thunder);
        queue_completed_ci_she_harvest(&mut app, client_entity, target);

        app.update();

        let base_quality = app
            .world()
            .resource::<ItemRegistry>()
            .get("ci_she_hao")
            .expect("ci_she_hao template should exist")
            .spirit_quality_initial;
        // 与 completed_harvest_applies_herbalism_quality_bonus_using_effective_level 相同锚点：
        // 原始 Lv.7 → effective Lv.3。
        let herbalism_bonus = crate::botany::skill_hook::spirit_quality_bonus(3);
        let expected_quality =
            (base_quality + herbalism_bonus + PlantVariant::Thunder.quality_modifier())
                .clamp(0.0, 1.0);

        let dropped = app.world().resource::<DroppedLootRegistry>();
        assert_eq!(dropped.entries.len(), 1);
        let entry = dropped.entries.values().next().expect("one overflow entry");
        assert!(
            (entry.item.spirit_quality - expected_quality).abs() < 1e-6,
            "overflow ground drop should carry the same variant+skill quality bonus as a normal grant, got {} expected {}",
            entry.item.spirit_quality,
            expected_quality
        );
        assert!(
            entry.item.display_name.starts_with("雷 · "),
            "overflow drop should keep the variant display prefix, got {:?}",
            entry.item.display_name
        );
    }

    #[test]
    fn harvest_completion_stack_boundary_max_stack_count_still_overflows_correctly() {
        let mut app = make_app_with_combat_events();
        let item_registry = load_item_registry().expect("item registry should load");
        let max_stack = item_registry
            .get("ci_she_hao")
            .expect("ci_she_hao template should exist")
            .max_stack_count;
        app.insert_resource(item_registry);
        app.insert_resource(InventoryInstanceIdAllocator::default());
        app.insert_resource(DroppedLootRegistry::default());
        app.add_systems(Update, tick_harvest_sessions);

        // 1x1 容器：先用公开 API 把唯一格子塞满同模板 max_stack_count 堆叠——这条路径
        // 与正常收获走同一套 add_item_to_player_inventory_inner，保证 merge 字段完全对齐,
        // 从而真正测的是"已满且无法再合并"而不是"模板不同没法合并"。
        let mut inventory = PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                quick_access: false,
                id: MAIN_PACK_CONTAINER_ID.into(),
                name: "main".into(),
                rows: 1,
                cols: 1,
                items: Vec::new(),
                owner_instance_id: None,
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 999.0,
        };
        {
            let registry = app.world().resource::<ItemRegistry>();
            let mut allocator = InventoryInstanceIdAllocator::new(500);
            add_item_to_player_inventory(
                &mut inventory,
                registry,
                &mut allocator,
                "ci_she_hao",
                max_stack,
                0,
            )
            .expect("filling the single 1x1 cell to max_stack_count should succeed");
        }

        let (client_bundle, _helper) = create_mock_client("Azure");
        let client_entity = app
            .world_mut()
            .spawn(client_bundle)
            .insert(inventory)
            .insert(Cultivation::default())
            .insert(Contamination::default())
            .insert(Wounds::default())
            .id();
        let target = plant_entity(&mut app, "spawn");
        queue_completed_ci_she_harvest(&mut app, client_entity, target);

        app.update();

        let dropped = app.world().resource::<DroppedLootRegistry>();
        assert_eq!(
            dropped.entries.len(),
            1,
            "a matching stack already at max_stack_count with no other free cell must still overflow to ground, not silently discard"
        );

        let inv = app
            .world()
            .get::<PlayerInventory>(client_entity)
            .expect("client should have inventory");
        let main = inv
            .containers
            .iter()
            .find(|c| c.id == MAIN_PACK_CONTAINER_ID)
            .expect("main pack should exist");
        assert_eq!(
            main.items.len(),
            1,
            "pre-existing maxed stack must be untouched, not duplicated"
        );
        assert_eq!(
            main.items[0].instance.stack_count, max_stack,
            "existing stack count must stay at max_stack_count, overflow must not silently bump it past the limit"
        );
    }

    #[test]
    fn harvest_completion_overflow_triggers_for_both_manual_and_auto_modes() {
        for mode in [BotanyHarvestMode::Manual, BotanyHarvestMode::Auto] {
            let mut app = make_app_with_combat_events();
            app.insert_resource(load_item_registry().expect("item registry should load"));
            app.insert_resource(InventoryInstanceIdAllocator::default());
            app.insert_resource(DroppedLootRegistry::default());
            app.add_systems(Update, tick_harvest_sessions);

            let (client_bundle, _helper) = create_mock_client("Azure");
            let client_entity = app
                .world_mut()
                .spawn(client_bundle)
                .insert(full_1x1_inventory_blocking("filler"))
                .insert(Cultivation::default())
                .insert(Contamination::default())
                .insert(Wounds::default())
                .id();
            let target = plant_entity(&mut app, "spawn");
            queue_completed_ci_she_harvest_with_mode(&mut app, client_entity, target, mode);

            app.update();

            let dropped = app.world().resource::<DroppedLootRegistry>();
            assert_eq!(
                dropped.entries.len(),
                1,
                "{mode:?} mode should also overflow to ground when the inventory is full"
            );

            let frames: Vec<_> = app
                .world_mut()
                .resource_mut::<Events<HarvestTerminalEvent>>()
                .drain()
                .collect();
            assert_eq!(
                frames.len(),
                1,
                "{mode:?} should send exactly one terminal event"
            );
            assert!(
                frames[0].overflow_to_ground,
                "{mode:?} terminal event should flag overflow_to_ground"
            );
            assert_eq!(frames[0].mode, mode);
        }
    }

    #[test]
    fn harvest_completion_rejects_consumed_target_without_second_grant() {
        let mut app = make_app_with_combat_events();
        app.insert_resource(load_item_registry().expect("item registry should load"));
        app.insert_resource(InventoryInstanceIdAllocator::default());
        app.insert_resource(DroppedLootRegistry::default());
        app.add_systems(Update, tick_harvest_sessions);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let client_entity = app
            .world_mut()
            .spawn(client_bundle)
            .insert(empty_inventory_8x8())
            .insert(Cultivation::default())
            .insert(Contamination::default())
            .insert(Wounds::default())
            .id();
        let target = plant_entity(&mut app, "spawn");

        queue_completed_ci_she_harvest(&mut app, client_entity, target);
        app.update();
        let revision_after_first = app
            .world()
            .entity(client_entity)
            .get::<PlayerInventory>()
            .expect("client keeps inventory")
            .revision;
        let xp_after_first = app
            .world()
            .resource::<HarvestSessionStore>()
            .skill_for("offline:Azure")
            .xp;

        queue_completed_ci_she_harvest(&mut app, client_entity, target);
        app.update();

        let plant = app.world().entity(target).get::<Plant>().unwrap();
        assert!(plant.harvested, "first completion consumes the live plant");
        let inventory = app
            .world()
            .entity(client_entity)
            .get::<PlayerInventory>()
            .expect("client keeps inventory");
        let harvested_count = inventory
            .containers
            .iter()
            .flat_map(|container| &container.items)
            .filter(|placed| placed.instance.template_id == "ci_she_hao")
            .map(|placed| placed.instance.stack_count)
            .sum::<u32>();
        assert_eq!(harvested_count, 1, "a consumed plant grants exactly once");
        assert_eq!(
            inventory.revision, revision_after_first,
            "rejected duplicate completion must not revise inventory"
        );
        assert_eq!(
            app.world()
                .resource::<HarvestSessionStore>()
                .skill_for("offline:Azure")
                .xp,
            xp_after_first,
            "rejected duplicate completion must not award skill XP"
        );
        assert!(
            app.world()
                .resource::<DroppedLootRegistry>()
                .entries
                .is_empty(),
            "rejected duplicate completion must not create overflow loot"
        );
        let frames: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<HarvestTerminalEvent>>()
            .drain()
            .collect();
        assert_eq!(
            frames.len(),
            2,
            "success and rejection each close their session"
        );
        assert!(frames[0].completed && !frames[0].interrupted);
        assert!(frames[1].interrupted && !frames[1].completed);
    }

    #[test]
    fn harvest_completion_overflow_entry_is_visible_via_dropped_loot_snapshot() {
        let mut app = make_app_with_combat_events();
        app.insert_resource(load_item_registry().expect("item registry should load"));
        app.insert_resource(InventoryInstanceIdAllocator::default());
        app.insert_resource(DroppedLootRegistry::default());
        app.add_systems(Update, tick_harvest_sessions);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let client_entity = app
            .world_mut()
            .spawn(client_bundle)
            .insert(full_1x1_inventory_blocking("filler"))
            .insert(Cultivation::default())
            .insert(Contamination::default())
            .insert(Wounds::default())
            .id();
        let target = plant_entity(&mut app, "spawn");
        queue_completed_ci_she_harvest(&mut app, client_entity, target);

        app.update();

        let registry = app.world().resource::<DroppedLootRegistry>();
        let snapshot = dropped_loot_snapshot(registry);
        assert_eq!(
            snapshot.len(),
            1,
            "overflow drop must be discoverable through the same snapshot fn the sync broadcast \
             pipeline uses (dropped_loot_sync_emit), not just the internal entries HashMap"
        );
        assert_eq!(snapshot[0].item.template_id, "ci_she_hao");
    }

    #[test]
    fn required_tool_harvest_avoids_bare_hand_wound() {
        let mut app = make_app_with_combat_events();
        app.insert_resource(load_item_registry().expect("item registry should load"));
        app.insert_resource(InventoryInstanceIdAllocator::default());
        app.add_systems(Update, tick_harvest_sessions);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let client_entity = app
            .world_mut()
            .spawn(client_bundle)
            .insert(inventory_with_main_hand_tool(Some("dun_qi_jia")))
            .insert(Cultivation::default())
            .insert(Contamination::default())
            .insert(Wounds::default())
            .id();
        let target = plant_entity_with_id(&mut app, "spawn", BotanyPlantId::JiaoMaiTeng);

        app.world_mut()
            .resource_mut::<HarvestSessionStore>()
            .upsert_session(HarvestSession {
                player_id: "offline:Azure".to_string(),
                client_entity,
                target_entity: Some(target),
                target_plant: BotanyPlantId::JiaoMaiTeng,
                mode: BotanyHarvestMode::Manual,
                started_at_tick: 0,
                duration_ticks: 0,
                phase: BotanyPhase::InProgress,
                last_progress: 0.0,
                origin_position: [10.0, 64.0, 10.0],
            });

        app.update();

        let wounds = app.world().get::<Wounds>(client_entity).unwrap();
        let contamination = app.world().get::<Contamination>(client_entity).unwrap();
        assert!(wounds.entries.is_empty());
        assert!(contamination.entries.is_empty());
    }

    #[test]
    fn required_tool_harvest_ticks_tool_durability() {
        for (plant_id, tool_id) in [
            (BotanyPlantId::XuanGenWei, "dun_qi_jia"),
            (BotanyPlantId::XuanRongTai, "gua_dao"),
            (BotanyPlantId::XuePoLian, "bing_jia_shou_tao"),
            (BotanyPlantId::JiaoMaiTeng, "dun_qi_jia"),
            (BotanyPlantId::LingJingXu, "gua_dao"),
            // plan-gathering-tool-bind-v1 P1 §8.1 决议 #4：草镰接通本职——持镰免伤+耐久递减。
            (BotanyPlantId::DuanJiCi, "cao_lian"),
            (BotanyPlantId::XueSeMaiCao, "cao_lian"),
        ] {
            let mut app = make_app_with_combat_events();
            app.insert_resource(load_item_registry().expect("item registry should load"));
            app.insert_resource(InventoryInstanceIdAllocator::default());
            app.add_systems(Update, tick_harvest_sessions);

            let (client_bundle, _helper) = create_mock_client("Azure");
            let client_entity = app
                .world_mut()
                .spawn(client_bundle)
                .insert(inventory_with_main_hand_tool(Some(tool_id)))
                .insert(Cultivation::default())
                .insert(Contamination::default())
                .insert(Wounds::default())
                .id();
            let target = plant_entity_with_id(&mut app, "spawn", plant_id);

            app.world_mut()
                .resource_mut::<HarvestSessionStore>()
                .upsert_session(HarvestSession {
                    player_id: "offline:Azure".to_string(),
                    client_entity,
                    target_entity: Some(target),
                    target_plant: plant_id,
                    mode: BotanyHarvestMode::Manual,
                    started_at_tick: 0,
                    duration_ticks: 0,
                    phase: BotanyPhase::InProgress,
                    last_progress: 0.0,
                    origin_position: [10.0, 64.0, 10.0],
                });

            app.update();

            let wounds = app.world().get::<Wounds>(client_entity).unwrap();
            let contamination = app.world().get::<Contamination>(client_entity).unwrap();
            assert!(wounds.entries.is_empty(), "{plant_id:?} should avoid wound");
            assert!(
                contamination.entries.is_empty(),
                "{plant_id:?} should avoid contamination"
            );
            let inventory = app.world().get::<PlayerInventory>(client_entity).unwrap();
            let tool = inventory
                .equipped
                .get(EQUIP_SLOT_MAIN_HAND)
                .and_then(|s| s.held.as_ref())
                .unwrap();
            assert!((tool.durability - 0.99).abs() < 1e-9);

            let durability_events = app
                .world()
                .resource::<Events<InventoryDurabilityChangedEvent>>();
            let events: Vec<_> = durability_events.iter_current_update_events().collect();
            assert_eq!(events.len(), 1, "{plant_id:?} should tick tool durability");
            assert_eq!(events[0].entity, client_entity);
            assert_eq!(events[0].instance_id, 9_001);
            assert!((events[0].durability - 0.99).abs() < 1e-9);
        }
    }

    #[test]
    fn broken_required_tool_counts_as_bare_hand_and_does_not_tick_durability() {
        let mut app = make_app_with_combat_events();
        app.insert_resource(load_item_registry().expect("item registry should load"));
        app.insert_resource(InventoryInstanceIdAllocator::default());
        app.add_systems(Update, tick_harvest_sessions);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let client_entity = app
            .world_mut()
            .spawn(client_bundle)
            .insert(inventory_with_main_hand_tool_durability(
                Some("dun_qi_jia"),
                0.0,
            ))
            .insert(Cultivation::default())
            .insert(Contamination::default())
            .insert(Wounds::default())
            .id();
        let target = plant_entity_with_id(&mut app, "spawn", BotanyPlantId::JiaoMaiTeng);

        app.world_mut()
            .resource_mut::<HarvestSessionStore>()
            .upsert_session(HarvestSession {
                player_id: "offline:Azure".to_string(),
                client_entity,
                target_entity: Some(target),
                target_plant: BotanyPlantId::JiaoMaiTeng,
                mode: BotanyHarvestMode::Manual,
                started_at_tick: 0,
                duration_ticks: 0,
                phase: BotanyPhase::InProgress,
                last_progress: 0.0,
                origin_position: [10.0, 64.0, 10.0],
            });

        app.update();

        let wounds = app.world().get::<Wounds>(client_entity).unwrap();
        let contamination = app.world().get::<Contamination>(client_entity).unwrap();
        assert_eq!(wounds.entries.len(), 1);
        assert_eq!(contamination.entries.len(), 1);

        let inventory = app.world().get::<PlayerInventory>(client_entity).unwrap();
        let tool = inventory
            .equipped
            .get(EQUIP_SLOT_MAIN_HAND)
            .and_then(|s| s.held.as_ref())
            .unwrap();
        assert_eq!(tool.durability, 0.0);

        let durability_events = app
            .world()
            .resource::<Events<InventoryDurabilityChangedEvent>>();
        assert_eq!(durability_events.iter_current_update_events().count(), 0);
    }

    #[test]
    fn wrong_tool_harvest_triggers_bare_hand_wound_and_contamination() {
        let mut app = make_app_with_combat_events();
        app.insert_resource(load_item_registry().expect("item registry should load"));
        app.insert_resource(InventoryInstanceIdAllocator::default());
        app.add_systems(Update, tick_harvest_sessions);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let client_entity = app
            .world_mut()
            .spawn(client_bundle)
            .insert(inventory_with_main_hand_tool(Some("cai_yao_dao")))
            .insert(Cultivation::default())
            .insert(Contamination::default())
            .insert(Wounds::default())
            .id();
        let target = plant_entity_with_id(&mut app, "spawn", BotanyPlantId::JiaoMaiTeng);

        app.world_mut()
            .resource_mut::<HarvestSessionStore>()
            .upsert_session(HarvestSession {
                player_id: "offline:Azure".to_string(),
                client_entity,
                target_entity: Some(target),
                target_plant: BotanyPlantId::JiaoMaiTeng,
                mode: BotanyHarvestMode::Manual,
                started_at_tick: 0,
                duration_ticks: 0,
                phase: BotanyPhase::InProgress,
                last_progress: 0.0,
                origin_position: [10.0, 64.0, 10.0],
            });

        app.update();

        let wounds = app.world().get::<Wounds>(client_entity).unwrap();
        let contamination = app.world().get::<Contamination>(client_entity).unwrap();
        assert_eq!(wounds.entries.len(), 1);
        assert_eq!(wounds.entries[0].kind, WoundKind::Concussion);
        assert_eq!(contamination.entries.len(), 1);
        assert_eq!(
            contamination.entries[0].attacker_id.as_deref(),
            Some("botany_v2_hazard")
        );
    }

    #[test]
    fn bare_hand_harvest_of_cao_lian_gated_plants_causes_laceration_wound() {
        // plan-gathering-tool-bind-v1 P1："徒手 Laceration 命中"——DuanJiCi / XueSeMaiCao
        // 徒手采集应各自命中一次 Cut(Laceration) 伤 + 对应 contamination，每株专属用例。
        for plant_id in [BotanyPlantId::DuanJiCi, BotanyPlantId::XueSeMaiCao] {
            let mut app = make_app_with_combat_events();
            app.insert_resource(load_item_registry().expect("item registry should load"));
            app.insert_resource(InventoryInstanceIdAllocator::default());
            app.add_systems(Update, tick_harvest_sessions);

            let (client_bundle, _helper) = create_mock_client("Azure");
            let client_entity = app
                .world_mut()
                .spawn(client_bundle)
                .insert(inventory_with_main_hand_tool(None))
                .insert(Cultivation::default())
                .insert(Contamination::default())
                .insert(Wounds::default())
                .id();
            let target = plant_entity_with_id(&mut app, "spawn", plant_id);

            app.world_mut()
                .resource_mut::<HarvestSessionStore>()
                .upsert_session(HarvestSession {
                    player_id: "offline:Azure".to_string(),
                    client_entity,
                    target_entity: Some(target),
                    target_plant: plant_id,
                    mode: BotanyHarvestMode::Manual,
                    started_at_tick: 0,
                    duration_ticks: 0,
                    phase: BotanyPhase::InProgress,
                    last_progress: 0.0,
                    origin_position: [10.0, 64.0, 10.0],
                });

            app.update();

            let wounds = app.world().get::<Wounds>(client_entity).unwrap();
            let contamination = app.world().get::<Contamination>(client_entity).unwrap();
            assert_eq!(
                wounds.entries.len(),
                1,
                "{plant_id:?} 徒手采集应命中恰好 1 条伤（WoundOnBareHand），实际 {}",
                wounds.entries.len()
            );
            assert_eq!(
                wounds.entries[0].kind,
                WoundKind::Cut,
                "{plant_id:?} 徒手采集的伤类型应为 Cut（对应 WoundLevel::Laceration），实际 {:?}",
                wounds.entries[0].kind
            );
            assert!(
                (wounds.entries[0].severity - 0.28).abs() < 1e-6,
                "{plant_id:?} Laceration severity 应为 0.28，实际 {}",
                wounds.entries[0].severity
            );
            assert_eq!(
                contamination.entries.len(),
                1,
                "{plant_id:?} 徒手采集应触发恰好 1 条 contamination"
            );
            assert!(
                (contamination.entries[0].amount - 0.2).abs() < 1e-9,
                "{plant_id:?} Laceration contamination amount 应为 0.2，实际 {}",
                contamination.entries[0].amount
            );
        }
    }

    #[test]
    fn cao_lian_broken_durability_counts_as_bare_hand_for_gated_plants() {
        // plan-gathering-tool-bind-v1 P1："镰耐久归零后等同徒手"——durability=0.0 的草镰
        // 应被 main_hand_tool_in_inventory 判定为 None，行为与不持工具一致：受伤 + 不再扣耐久。
        let mut app = make_app_with_combat_events();
        app.insert_resource(load_item_registry().expect("item registry should load"));
        app.insert_resource(InventoryInstanceIdAllocator::default());
        app.add_systems(Update, tick_harvest_sessions);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let client_entity = app
            .world_mut()
            .spawn(client_bundle)
            .insert(inventory_with_main_hand_tool_durability(
                Some("cao_lian"),
                0.0,
            ))
            .insert(Cultivation::default())
            .insert(Contamination::default())
            .insert(Wounds::default())
            .id();
        let target = plant_entity_with_id(&mut app, "spawn", BotanyPlantId::XueSeMaiCao);

        app.world_mut()
            .resource_mut::<HarvestSessionStore>()
            .upsert_session(HarvestSession {
                player_id: "offline:Azure".to_string(),
                client_entity,
                target_entity: Some(target),
                target_plant: BotanyPlantId::XueSeMaiCao,
                mode: BotanyHarvestMode::Manual,
                started_at_tick: 0,
                duration_ticks: 0,
                phase: BotanyPhase::InProgress,
                last_progress: 0.0,
                origin_position: [10.0, 64.0, 10.0],
            });

        app.update();

        let wounds = app.world().get::<Wounds>(client_entity).unwrap();
        assert_eq!(
            wounds.entries.len(),
            1,
            "耐久归零的草镰应等同徒手，触发 1 条伤"
        );
        assert_eq!(wounds.entries[0].kind, WoundKind::Cut);

        let inventory = app.world().get::<PlayerInventory>(client_entity).unwrap();
        let tool = inventory
            .equipped
            .get(EQUIP_SLOT_MAIN_HAND)
            .and_then(|s| s.held.as_ref())
            .unwrap();
        assert_eq!(tool.durability, 0.0, "已经归零的耐久不应变负或被重置");

        let durability_events = app
            .world()
            .resource::<Events<InventoryDurabilityChangedEvent>>();
        assert_eq!(
            durability_events.iter_current_update_events().count(),
            0,
            "耐久已归零的工具不应再触发 InventoryDurabilityChangedEvent"
        );
    }

    #[test]
    fn bare_hand_harvest_of_non_gated_plant_causes_no_wound_regression() {
        // plan-gathering-tool-bind-v1 P1 回归锁："目标植物外徒手不受伤"——CiSheHao 是无
        // v2_spec 的 v1 植物（required_tool_for 返回 None），加了 DuanJiCi/XueSeMaiCao 的
        // required_tool 门槛之后，其余植物的徒手流程必须保持完全不受影响。
        let mut app = make_app_with_combat_events();
        app.insert_resource(load_item_registry().expect("item registry should load"));
        app.insert_resource(InventoryInstanceIdAllocator::default());
        app.add_systems(Update, tick_harvest_sessions);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let client_entity = app
            .world_mut()
            .spawn(client_bundle)
            .insert(inventory_with_main_hand_tool(None))
            .insert(Cultivation::default())
            .insert(Contamination::default())
            .insert(Wounds::default())
            .id();
        let target = plant_entity(&mut app, "spawn");
        queue_completed_ci_she_harvest(&mut app, client_entity, target);

        app.update();

        let wounds = app.world().get::<Wounds>(client_entity).unwrap();
        let contamination = app.world().get::<Contamination>(client_entity).unwrap();
        assert!(
            wounds.entries.is_empty(),
            "CiSheHao 徒手采集不应受伤（无 required_tool hazard），实际 {:?}",
            wounds.entries
        );
        assert!(
            contamination.entries.is_empty(),
            "CiSheHao 徒手采集不应触发 contamination，实际 {:?}",
            contamination.entries
        );
    }

    #[test]
    fn interrupt_populates_terminal_queue_with_reason() {
        let mut app = make_app_with_combat_events();
        app.insert_resource(BotanyTrampleRoll { chance_inverse: 0 });
        app.add_systems(Update, enforce_harvest_session_constraints);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let client_entity = app.world_mut().spawn(client_bundle).id();
        let target = plant_entity(&mut app, "spawn");

        {
            let mut store = app.world_mut().resource_mut::<HarvestSessionStore>();
            start_or_resume_harvest(
                &mut store,
                "Azure",
                client_entity,
                Some(target),
                BotanyPlantId::CiSheHao,
                BotanyHarvestMode::Auto,
                [10.0, 64.0, 10.0],
                1,
            );
        }

        // 受击 → 打断
        app.world_mut()
            .resource_mut::<Events<CombatEvent>>()
            .send(CombatEvent {
                attacker: Entity::from_raw(999),
                target: client_entity,
                resolved_at_tick: 1,
                body_part: BodyPart::Chest,
                wound_kind: WoundKind::Blunt,
                source: crate::combat::events::AttackSource::Melee,
                debug_command: false,
                physical_damage: 0.0,
                damage: 4.0,
                contam_delta: 0.0,
                description: "test".to_string(),
                defense_kind: None,
                defense_effectiveness: None,
                defense_contam_reduced: None,
                defense_wound_severity: None,
            });

        app.update();

        use valence::prelude::Events;
        let frames: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<HarvestTerminalEvent>>()
            .drain()
            .collect();
        assert_eq!(
            frames.len(),
            1,
            "interrupt should send one HarvestTerminalEvent"
        );
        let frame = &frames[0];
        assert!(frame.interrupted && !frame.completed);
        assert!(
            frame.detail.contains("受击打断"),
            "detail should mention `受击打断`, got {:?}",
            frame.detail
        );
    }

    #[test]
    fn manual_session_interrupts_when_player_moves_past_threshold() {
        let mut app = make_app_with_combat_events();
        app.add_systems(Update, enforce_harvest_session_constraints);

        let (mut client_bundle, _helper) = create_mock_client("Azure");
        client_bundle.player.position = Position::new([10.0, 64.0, 10.0]);
        let client_entity = app.world_mut().spawn(client_bundle).id();
        let target = plant_entity(&mut app, "spawn");

        {
            let mut store = app.world_mut().resource_mut::<HarvestSessionStore>();
            start_or_resume_harvest(
                &mut store,
                "Azure",
                client_entity,
                Some(target),
                BotanyPlantId::CiSheHao,
                BotanyHarvestMode::Manual,
                [10.0, 64.0, 10.0],
                1,
            );
        }

        // 移动超过 0.3 块
        app.world_mut()
            .entity_mut(client_entity)
            .get_mut::<Position>()
            .expect("client should have Position")
            .set([12.0, 64.0, 10.0]);

        app.update();

        let store = app.world().resource::<HarvestSessionStore>();
        assert!(store.session_for("offline:Azure").is_none());

        let plant = app
            .world()
            .entity(target)
            .get::<Plant>()
            .expect("plant entity should still exist");
        assert!(plant.trampled, "chance_inverse=1 should always trample");
    }

    #[test]
    fn auto_session_tolerates_movement() {
        let mut app = make_app_with_combat_events();
        app.insert_resource(BotanyTrampleRoll { chance_inverse: 0 }); // never trample
        app.add_systems(Update, enforce_harvest_session_constraints);

        let (mut client_bundle, _helper) = create_mock_client("Azure");
        client_bundle.player.position = Position::new([10.0, 64.0, 10.0]);
        let client_entity = app.world_mut().spawn(client_bundle).id();
        let target = plant_entity(&mut app, "spawn");

        {
            let mut store = app.world_mut().resource_mut::<HarvestSessionStore>();
            start_or_resume_harvest(
                &mut store,
                "Azure",
                client_entity,
                Some(target),
                BotanyPlantId::CiSheHao,
                BotanyHarvestMode::Auto,
                [10.0, 64.0, 10.0],
                1,
            );
        }

        app.world_mut()
            .entity_mut(client_entity)
            .get_mut::<Position>()
            .expect("client should have Position")
            .set([15.0, 64.0, 10.0]);

        app.update();

        let store = app.world().resource::<HarvestSessionStore>();
        assert!(
            store.session_for("offline:Azure").is_some(),
            "Auto session should tolerate movement"
        );
    }

    #[test]
    fn non_session_trample_fires_on_first_proximity_tick_only() {
        let mut app = make_app_with_combat_events();
        app.add_systems(Update, detect_non_session_trample);

        let (mut client_bundle, _helper) = create_mock_client("Azure");
        client_bundle.player.position = Position::new([10.0, 64.0, 10.0]);
        let _client_entity = app.world_mut().spawn(client_bundle).id();

        // 植物离玩家 0.2 块（在 0.7 半径内）
        let target = app
            .world_mut()
            .spawn(Plant {
                id: BotanyPlantId::CiSheHao,
                zone_name: "spawn".to_string(),
                position: [10.2, 64.0, 10.0],
                planted_at_tick: 0,
                wither_progress: 0,
                source_point: None,
                harvested: false,
                trampled: false,
                variant: crate::botany::registry::PlantVariant::None,
            })
            .id();

        // tick1：首次进入近邻，chance_inverse=1 → 必踩死
        app.update();
        let plant = app
            .world()
            .entity(target)
            .get::<Plant>()
            .expect("plant should persist");
        assert!(plant.trampled, "first proximity tick should roll trample");

        // 清掉 trampled，确保第二 tick 不会二次掷骰
        app.world_mut()
            .entity_mut(target)
            .get_mut::<Plant>()
            .unwrap()
            .trampled = false;
        app.update();
        let plant = app
            .world()
            .entity(target)
            .get::<Plant>()
            .expect("plant should persist");
        assert!(
            !plant.trampled,
            "stationary proximity should not re-roll while tracker still holds the pair"
        );
    }

    #[test]
    fn non_session_trample_skips_plants_beyond_radius() {
        let mut app = make_app_with_combat_events();
        app.insert_resource(BotanyTrampleRoll { chance_inverse: 1 });
        app.add_systems(Update, detect_non_session_trample);

        let (mut client_bundle, _helper) = create_mock_client("Azure");
        client_bundle.player.position = Position::new([10.0, 64.0, 10.0]);
        let client_entity = app.world_mut().spawn(client_bundle).id();

        // 水平远 (>0.7) 但在同一 y 层
        let far = app
            .world_mut()
            .spawn(Plant {
                id: BotanyPlantId::CiSheHao,
                zone_name: "spawn".to_string(),
                position: [12.0, 64.0, 12.0],
                planted_at_tick: 0,
                wither_progress: 0,
                source_point: None,
                harvested: false,
                trampled: false,
                variant: crate::botany::registry::PlantVariant::None,
            })
            .id();

        // 近但不同层（dy=5）
        let different_floor = app
            .world_mut()
            .spawn(Plant {
                id: BotanyPlantId::CiSheHao,
                zone_name: "spawn".to_string(),
                position: [10.1, 69.0, 10.0],
                planted_at_tick: 0,
                wither_progress: 0,
                source_point: None,
                harvested: false,
                trampled: false,
                variant: crate::botany::registry::PlantVariant::None,
            })
            .id();

        let _ = client_entity; // 保持未使用警告抑制
        app.update();

        let far_plant = app.world().entity(far).get::<Plant>().unwrap();
        let other_floor = app.world().entity(different_floor).get::<Plant>().unwrap();
        assert!(
            !far_plant.trampled,
            "plant outside horizontal radius should not be trampled"
        );
        assert!(
            !other_floor.trampled,
            "plant on a different vertical layer should not be trampled"
        );
    }

    #[test]
    fn combat_hit_interrupts_auto_session() {
        let mut app = make_app_with_combat_events();
        app.insert_resource(BotanyTrampleRoll { chance_inverse: 0 });
        app.add_systems(Update, enforce_harvest_session_constraints);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let client_entity = app.world_mut().spawn(client_bundle).id();
        let target = plant_entity(&mut app, "spawn");

        {
            let mut store = app.world_mut().resource_mut::<HarvestSessionStore>();
            start_or_resume_harvest(
                &mut store,
                "Azure",
                client_entity,
                Some(target),
                BotanyPlantId::CiSheHao,
                BotanyHarvestMode::Auto,
                [10.0, 64.0, 10.0],
                1,
            );
        }

        {
            let mut events = app.world_mut().resource_mut::<Events<CombatEvent>>();
            events.send(CombatEvent {
                attacker: Entity::from_raw(999),
                target: client_entity,
                resolved_at_tick: 1,
                body_part: BodyPart::Chest,
                wound_kind: WoundKind::Blunt,
                source: crate::combat::events::AttackSource::Melee,
                debug_command: false,
                physical_damage: 0.0,
                damage: 4.0,
                contam_delta: 0.0,
                description: "test".to_string(),
                defense_kind: None,
                defense_effectiveness: None,
                defense_contam_reduced: None,
                defense_wound_severity: None,
            });
        }

        app.update();

        let store = app.world().resource::<HarvestSessionStore>();
        assert!(
            store.session_for("offline:Azure").is_none(),
            "Auto session should break on hit"
        );
    }

    // ---- plan-bughunt-botany-disconnect-session: release_disconnected_harvest_sessions ----

    #[test]
    fn disconnect_cancels_session_and_emits_interrupted_terminal_event() {
        let mut app = make_app_with_combat_events();
        app.add_systems(Update, release_disconnected_harvest_sessions);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let client_entity = app.world_mut().spawn(client_bundle).id();
        let target = plant_entity(&mut app, "spawn");

        {
            let mut store = app.world_mut().resource_mut::<HarvestSessionStore>();
            start_or_resume_harvest(
                &mut store,
                "Azure",
                client_entity,
                Some(target),
                BotanyPlantId::CiSheHao,
                BotanyHarvestMode::Manual,
                [10.0, 64.0, 10.0],
                1,
            );
        }

        // 模拟断线：valence 在连接丢失时移除 Client 组件。
        app.world_mut()
            .entity_mut(client_entity)
            .remove::<valence::prelude::Client>();

        app.update();

        let store = app.world().resource::<HarvestSessionStore>();
        assert!(
            store.session_for("offline:Azure").is_none(),
            "disconnect must cancel the in-progress session immediately, not leave it \
             dangling for a later completion tick to silently swallow"
        );

        let frames: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<HarvestTerminalEvent>>()
            .drain()
            .collect();
        assert_eq!(
            frames.len(),
            1,
            "disconnect must send exactly one terminal event for the cancelled session"
        );
        let frame = &frames[0];
        assert!(
            frame.interrupted && !frame.completed,
            "disconnect cancellation must be an explicit interrupt, not a silent completion — \
             got interrupted={} completed={}",
            frame.interrupted,
            frame.completed
        );
        assert!(
            frame.detail.contains("断线"),
            "detail must clearly state the cancellation reason is disconnect, got {:?}",
            frame.detail
        );
        assert_eq!(frame.session_id, "offline:Azure");
        assert_eq!(frame.client_entity, client_entity);
    }

    #[test]
    fn disconnect_at_completion_tick_does_not_reach_completion_path() {
        // 复现 skeleton 的第二条触发路径：session 恰好在断线当帧达到完成进度。
        // release_disconnected_harvest_sessions 必须在 tick_harvest_sessions 之前拦截，
        // 否则 complete_harvest_for_player 会先 remove_session 再因旧实体缺 Client
        // 查库存失败，静默吞掉玩家已等待完成的采集产出。
        let mut app = make_app_with_combat_events();
        app.insert_resource(load_item_registry().expect("item registry should load"));
        app.insert_resource(InventoryInstanceIdAllocator::default());
        app.insert_resource(DroppedLootRegistry::default());
        app.add_systems(
            Update,
            (release_disconnected_harvest_sessions, tick_harvest_sessions).chain(),
        );

        let (client_bundle, _helper) = create_mock_client("Azure");
        let client_entity = app
            .world_mut()
            .spawn(client_bundle)
            .insert(empty_inventory_8x8())
            .insert(Cultivation::default())
            .insert(Contamination::default())
            .insert(Wounds::default())
            .id();
        let target = plant_entity(&mut app, "spawn");
        // duration_ticks=0 => progress_at(any tick) >= 1.0 immediately, same as the
        // existing `queue_completed_ci_she_harvest` helper's "already complete" setup.
        queue_completed_ci_she_harvest(&mut app, client_entity, target);

        app.world_mut()
            .entity_mut(client_entity)
            .remove::<valence::prelude::Client>();

        app.update();

        let store = app.world().resource::<HarvestSessionStore>();
        assert!(
            store.session_for("offline:Azure").is_none(),
            "session must be gone after the tick regardless of path taken"
        );

        let plant = app
            .world()
            .entity(target)
            .get::<Plant>()
            .expect("plant entity should still exist");
        assert!(
            !plant.harvested,
            "the disconnect path must win the race — plant must NOT be marked harvested via \
             complete_harvest_for_player, which would require a live Client/PlayerInventory \
             that no longer exists on the disconnected entity"
        );

        let frames: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<HarvestTerminalEvent>>()
            .drain()
            .collect();
        assert_eq!(
            frames.len(),
            1,
            "exactly one terminal event must fire — the disconnect cancellation, not a \
             completion event from complete_harvest_for_player"
        );
        assert!(
            frames[0].interrupted && !frames[0].completed,
            "must be the disconnect interrupt, not a completion — got interrupted={} completed={}",
            frames[0].interrupted,
            frames[0].completed
        );

        let dropped = app.world().resource::<DroppedLootRegistry>();
        assert!(
            dropped.entries.is_empty(),
            "no product should ever be granted or dropped for a session cancelled by disconnect"
        );
    }

    #[test]
    fn reconnect_after_disconnect_can_start_a_fresh_session() {
        let mut app = make_app_with_combat_events();
        app.add_systems(Update, release_disconnected_harvest_sessions);

        let (old_bundle, _old_helper) = create_mock_client("Azure");
        let old_client_entity = app.world_mut().spawn(old_bundle).id();
        let target = plant_entity(&mut app, "spawn");

        {
            let mut store = app.world_mut().resource_mut::<HarvestSessionStore>();
            start_or_resume_harvest(
                &mut store,
                "Azure",
                old_client_entity,
                Some(target),
                BotanyPlantId::CiSheHao,
                BotanyHarvestMode::Manual,
                [10.0, 64.0, 10.0],
                1,
            );
        }

        app.world_mut()
            .entity_mut(old_client_entity)
            .remove::<valence::prelude::Client>();
        app.update();

        // 重连：新 client_entity，同一 player_id。
        let (new_bundle, _new_helper) = create_mock_client("Azure");
        let new_client_entity = app.world_mut().spawn(new_bundle).id();
        assert_ne!(
            old_client_entity, new_client_entity,
            "reconnect must produce a fresh ECS entity distinct from the disconnected one"
        );

        {
            let mut store = app.world_mut().resource_mut::<HarvestSessionStore>();
            start_or_resume_harvest(
                &mut store,
                "Azure",
                new_client_entity,
                Some(target),
                BotanyPlantId::CiSheHao,
                BotanyHarvestMode::Manual,
                [10.0, 64.0, 10.0],
                5,
            );
        }

        let store = app.world().resource::<HarvestSessionStore>();
        let session = store
            .session_for("offline:Azure")
            .expect("reconnecting player must be able to start a brand new session");
        assert_eq!(
            session.client_entity, new_client_entity,
            "the new session must be bound to the new client entity, not blocked or \
             misrouted by any residue from the disconnected old session"
        );
        assert_eq!(session.started_at_tick, 5);
    }

    #[test]
    fn disconnect_without_active_session_is_a_no_op() {
        let mut app = make_app_with_combat_events();
        app.add_systems(Update, release_disconnected_harvest_sessions);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let client_entity = app.world_mut().spawn(client_bundle).id();

        // 没有为该玩家创建任何 HarvestSession —— 断线时该系统必须安全地什么都不做。
        app.world_mut()
            .entity_mut(client_entity)
            .remove::<valence::prelude::Client>();

        app.update();

        let frames: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<HarvestTerminalEvent>>()
            .drain()
            .collect();
        assert!(
            frames.is_empty(),
            "disconnecting a client with no active harvest session must not fabricate a \
             terminal event, got {frames:?}"
        );
        assert_eq!(
            app.world().resource::<HarvestSessionStore>().iter().count(),
            0
        );
    }

    #[test]
    fn disconnect_only_cancels_the_matching_players_session() {
        let mut app = make_app_with_combat_events();
        app.add_systems(Update, release_disconnected_harvest_sessions);

        let (azure_bundle, _azure_helper) = create_mock_client("Azure");
        let azure_entity = app.world_mut().spawn(azure_bundle).id();
        let (breeze_bundle, _breeze_helper) = create_mock_client("Breeze");
        let breeze_entity = app.world_mut().spawn(breeze_bundle).id();
        let target_a = plant_entity(&mut app, "spawn");
        let target_b = plant_entity(&mut app, "spawn");

        {
            let mut store = app.world_mut().resource_mut::<HarvestSessionStore>();
            start_or_resume_harvest(
                &mut store,
                "Azure",
                azure_entity,
                Some(target_a),
                BotanyPlantId::CiSheHao,
                BotanyHarvestMode::Manual,
                [10.0, 64.0, 10.0],
                1,
            );
            start_or_resume_harvest(
                &mut store,
                "Breeze",
                breeze_entity,
                Some(target_b),
                BotanyPlantId::CiSheHao,
                BotanyHarvestMode::Auto,
                [20.0, 64.0, 20.0],
                1,
            );
        }

        // 只断线 Azure。
        app.world_mut()
            .entity_mut(azure_entity)
            .remove::<valence::prelude::Client>();

        app.update();

        let store = app.world().resource::<HarvestSessionStore>();
        assert!(
            store.session_for("offline:Azure").is_none(),
            "Azure's session must be cancelled"
        );
        let breeze_session = store
            .session_for("offline:Breeze")
            .expect("Breeze stayed connected — session must be untouched");
        assert_eq!(breeze_session.client_entity, breeze_entity);
        assert_eq!(breeze_session.mode, BotanyHarvestMode::Auto);

        let frames: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<HarvestTerminalEvent>>()
            .drain()
            .collect();
        assert_eq!(
            frames.len(),
            1,
            "only one terminal event for the one disconnected player"
        );
        assert_eq!(frames[0].session_id, "offline:Azure");
    }

    #[test]
    fn disconnect_cancellation_preserves_gathering_skill_xp() {
        let mut app = make_app_with_combat_events();
        app.add_systems(Update, release_disconnected_harvest_sessions);

        let (client_bundle, _helper) = create_mock_client("Azure");
        let client_entity = app.world_mut().spawn(client_bundle).id();
        let target = plant_entity(&mut app, "spawn");

        {
            let mut store = app.world_mut().resource_mut::<HarvestSessionStore>();
            store.add_skill_xp("offline:Azure", 40);
            start_or_resume_harvest(
                &mut store,
                "Azure",
                client_entity,
                Some(target),
                BotanyPlantId::CiSheHao,
                BotanyHarvestMode::Manual,
                [10.0, 64.0, 10.0],
                1,
            );
        }

        app.world_mut()
            .entity_mut(client_entity)
            .remove::<valence::prelude::Client>();
        app.update();

        let store = app.world().resource::<HarvestSessionStore>();
        assert!(store.session_for("offline:Azure").is_none());
        assert_eq!(
            store.skill_for("offline:Azure").xp,
            40,
            "disconnect must only cancel the in-progress session, never touch \
             skills_by_player — earned gathering XP is persistent player progress"
        );
    }
}
