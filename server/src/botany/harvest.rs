use std::collections::HashSet;

use valence::prelude::{Entity, EventReader, EventWriter, Position, Query, Res, ResMut, With};

use crate::combat::events::CombatEvent;
use crate::cultivation::breakthrough::skill_cap_for_realm;
use crate::cultivation::components::Cultivation;
use crate::inventory::{
    add_item_to_player_inventory, InventoryInstanceIdAllocator, ItemRegistry, PlayerInventory,
};
use crate::player::state::canonical_player_id;
use crate::skill::components::{SkillId, SkillSet};
use crate::skill::curve::effective_lv;
use crate::skill::events::{SkillXpGain, XpGainSource};

use super::components::{
    BotanyHarvestMode, BotanyPhase, BotanySkillChangedEvent, BotanyTrampleRoll, HarvestSession,
    HarvestSessionStore, HarvestTerminalEvent, InventorySnapshotRequestEvent, Plant,
    PlantProximityTracker, PlantStaticPointStore,
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
) {
    let player_id = canonical_player_id(player_name);
    if store.session_for(player_id.as_str()).is_some() {
        return;
    }

    let duration_ticks = match mode {
        BotanyHarvestMode::Manual => MANUAL_DURATION_TICKS,
        BotanyHarvestMode::Auto => AUTO_DURATION_TICKS,
    };

    store.upsert_session(HarvestSession {
        player_id,
        client_entity,
        target_entity,
        target_plant,
        mode,
        started_at_tick: now_tick,
        duration_ticks,
        phase: BotanyPhase::InProgress,
        last_progress: 0.0,
        origin_position,
    });
}

#[allow(clippy::too_many_arguments)]
pub fn complete_harvest_for_player(
    store: &mut HarvestSessionStore,
    player_id: &str,
    plant_query: &mut Query<&mut Plant, With<Plant>>,
    inventory_query: &mut Query<&mut PlayerInventory, With<valence::prelude::Client>>,
    harvesters: &Query<(Option<&Cultivation>, Option<&SkillSet>), With<valence::prelude::Client>>,
    kind_registry: &BotanyKindRegistry,
    item_registry: &ItemRegistry,
    allocator: &mut InventoryInstanceIdAllocator,
    snapshot_events: &mut EventWriter<InventorySnapshotRequestEvent>,
    static_points: &mut PlantStaticPointStore,
    terminal_events: &mut EventWriter<HarvestTerminalEvent>,
    skill_events: &mut EventWriter<BotanySkillChangedEvent>,
    skill_xp_events: &mut EventWriter<SkillXpGain>,
    now_tick: u64,
) -> Result<(), String> {
    let session = store
        .remove_session(player_id)
        .ok_or_else(|| format!("missing harvest session for `{player_id}`"))?;

    let mut target_pos: Option<[f64; 3]> = None;
    let mut variant = PlantVariant::None;
    if let Some(target_entity) = session.target_entity {
        if let Ok(mut plant) = plant_query.get_mut(target_entity) {
            target_pos = Some(plant.position);
            variant = plant.variant;
            if let Some(source_point) = plant.source_point {
                if let Some(point) = static_points.get_mut(source_point) {
                    point.bound_entity = None;
                    point.last_spawn_tick = Some(now_tick);
                }
            }
            plant.harvested = true;
        }
    }

    let kind = kind_registry
        .get(session.target_plant)
        .ok_or_else(|| format!("missing kind for `{}`", session.target_plant.as_str()))?;

    let mut inventory = inventory_query
        .get_mut(session.client_entity)
        .map_err(|_| {
            format!(
                "player inventory missing on entity {:?}",
                session.client_entity
            )
        })?;

    let receipt =
        add_item_to_player_inventory(&mut inventory, item_registry, allocator, kind.item_id, 1)?;

    let herbalism_quality_bonus = harvesters
        .get(session.client_entity)
        .ok()
        .map(|(cultivation, skill_set)| {
            super::skill_hook::spirit_quality_bonus(herbalism_effective_lv(cultivation, skill_set))
        })
        .unwrap_or(0.0);

    // plan-skill-v1 §6.1 品质偏移先投影到连续 spirit_quality，再叠加 botany 变种修饰。
    if variant != PlantVariant::None || herbalism_quality_bonus > 0.0 {
        apply_harvest_modifiers_to_instance(
            &mut inventory,
            receipt.instance_id,
            variant,
            herbalism_quality_bonus,
        );
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
    terminal_events.send(HarvestTerminalEvent {
        client_entity: session.client_entity,
        session_id: session.player_id.clone(),
        target_id: format_target_id(session.target_entity),
        target_name: target_name_with_variant.clone(),
        plant_kind: session.target_plant.as_str().to_string(),
        mode: session.mode,
        interrupted: false,
        completed: true,
        detail: format!("采得 1 株 · 灵气流出 {:.3}", kind.growth_cost),
        target_pos,
    });
    Ok(())
}

/// 对刚 push 进 main pack 的 ItemInstance 应用 herb skill / variant 品质修饰与显示名前缀。
fn apply_harvest_modifiers_to_instance(
    inventory: &mut PlayerInventory,
    instance_id: u64,
    variant: PlantVariant,
    herbalism_quality_bonus: f64,
) {
    for container in inventory.containers.iter_mut() {
        for placed in container.items.iter_mut() {
            if placed.instance.instance_id != instance_id {
                continue;
            }
            let q = placed.instance.spirit_quality
                + herbalism_quality_bonus
                + variant.quality_modifier();
            placed.instance.spirit_quality = q.clamp(0.0, 1.0);
            if let Some(prefix) = variant.display_prefix() {
                placed.instance.display_name =
                    format!("{} · {}", prefix, placed.instance.display_name);
            }
            return;
        }
    }
}

fn herbalism_effective_lv(cultivation: Option<&Cultivation>, skill_set: Option<&SkillSet>) -> u8 {
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
        let reason: &'static str = if hit { "受击打断" } else { "移动打断" };
        to_interrupt.push(InterruptTarget {
            player_id: session.player_id.clone(),
            client_entity: session.client_entity,
            target_entity: session.target_entity,
            target_plant: session.target_plant,
            mode: session.mode,
            reason,
            trampled,
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
    z % u64::from(chance_inverse) == 0
}

#[allow(clippy::too_many_arguments)]
pub fn tick_harvest_sessions(
    gameplay_tick: Option<Res<crate::player::gameplay::GameplayTick>>,
    mut store: ResMut<HarvestSessionStore>,
    mut plants: Query<&mut Plant, With<Plant>>,
    mut inventories: Query<&mut PlayerInventory, With<valence::prelude::Client>>,
    harvesters: Query<(Option<&Cultivation>, Option<&SkillSet>), With<valence::prelude::Client>>,
    kind_registry: Res<BotanyKindRegistry>,
    item_registry: Res<ItemRegistry>,
    mut allocator: ResMut<InventoryInstanceIdAllocator>,
    mut snapshot_events: EventWriter<InventorySnapshotRequestEvent>,
    mut static_points: ResMut<PlantStaticPointStore>,
    mut terminal_events: EventWriter<HarvestTerminalEvent>,
    mut skill_events: EventWriter<BotanySkillChangedEvent>,
    mut skill_xp_events: EventWriter<SkillXpGain>,
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
        let _ = complete_harvest_for_player(
            &mut store,
            player_id.as_str(),
            &mut plants,
            &mut inventories,
            &harvesters,
            kind_registry.as_ref(),
            item_registry.as_ref(),
            &mut allocator,
            &mut snapshot_events,
            &mut static_points,
            &mut terminal_events,
            &mut skill_events,
            &mut skill_xp_events,
            now,
        );
    }
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
    use crate::combat::components::{BodyPart, WoundKind};
    use crate::cultivation::components::{Cultivation, Realm};
    use crate::inventory::{
        load_item_registry, ContainerState, InventoryInstanceIdAllocator, InventoryRevision,
        PlayerInventory, MAIN_PACK_CONTAINER_ID,
    };
    use crate::player::gameplay::GameplayTick;
    use crate::skill::components::{SkillEntry, SkillSet};
    use crate::world::zone::ZoneRegistry;
    use std::collections::HashMap;
    use valence::prelude::{App, Events, Update};
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
        app.world_mut()
            .spawn(Plant {
                id: BotanyPlantId::CiSheHao,
                zone_name: zone_name.to_string(),
                position: [10.0, 64.0, 10.0],
                planted_at_tick: 0,
                wither_progress: 0,
                source_point: None,
                harvested: false,
                trampled: false,
                variant: crate::botany::registry::PlantVariant::None,
            })
            .id()
    }

    fn empty_inventory_8x8() -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                id: MAIN_PACK_CONTAINER_ID.into(),
                name: "main".into(),
                rows: 8,
                cols: 8,
                items: Vec::new(),
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 999.0,
        }
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
        app.add_event::<HarvestTerminalEvent>();
        app.add_event::<BotanySkillChangedEvent>();
        app.add_event::<SkillXpGain>();
        app
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
                damage: 4.0,
                contam_delta: 0.0,
                description: "test".to_string(),
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
                damage: 4.0,
                contam_delta: 0.0,
                description: "test".to_string(),
            });
        }

        app.update();

        let store = app.world().resource::<HarvestSessionStore>();
        assert!(
            store.session_for("offline:Azure").is_none(),
            "Auto session should break on hit"
        );
    }
}
