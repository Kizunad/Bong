use std::collections::{HashMap, VecDeque};

use serde::{Deserialize, Serialize};
use serde_json::json;
use valence::prelude::{
    App, Client, Entity, EventWriter, IntoSystemConfigs, ParamSet, Position, Query, ResMut,
    Resource, Update, Username, With,
};

use super::state::{canonical_player_id, PlayerState};
use crate::botany::components::BotanyHarvestMode;
use crate::botany::components::HarvestSessionStore;
use crate::botany::components::Plant;
use crate::botany::harvest::start_or_resume_harvest;
use crate::botany::registry::canonicalize_herb_id;
use crate::combat::{
    components::WoundKind,
    debug::enqueue_debug_attack_intent,
    events::{AttackIntent, AttackSource, FIST_REACH},
};
use crate::cultivation::breakthrough::BreakthroughRequest;
use crate::cultivation::components::Cultivation;
use crate::qi_physics::constants::{QI_GATHER_REWARD, QI_PER_ZONE_UNIT};
use crate::schema::common::{GameEventType, NarrationScope, NarrationStyle};
use crate::schema::narration::Narration;
use crate::schema::world_state::GameEvent;
use crate::world::dimension::DimensionKind;
use crate::world::events::ActiveEventsResource;
use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};

const GATHER_INVENTORY_REWARD: f64 = 0.12;
const GATHER_KARMA_REWARD: f64 = 0.06;

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CombatAction {
    pub target: String,
    pub qi_invest: f64,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq)]
pub struct GatherAction {
    pub resource: String,
    pub target_entity: Option<Entity>,
    pub mode: Option<BotanyHarvestMode>,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq)]
pub enum GameplayAction {
    Combat(CombatAction),
    Gather(GatherAction),
    AttemptBreakthrough,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QueuedGameplayAction {
    pub player: String,
    pub action: GameplayAction,
}

#[derive(Default)]
pub struct GameplayActionQueue {
    pending: VecDeque<QueuedGameplayAction>,
}

impl Resource for GameplayActionQueue {}

impl GameplayActionQueue {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn enqueue(&mut self, player: impl Into<String>, action: GameplayAction) {
        self.pending.push_back(QueuedGameplayAction {
            player: player.into(),
            action,
        });
    }

    #[cfg(test)]
    pub fn pending_actions_snapshot(&self) -> Vec<QueuedGameplayAction> {
        self.pending.iter().cloned().collect()
    }

    fn pop_front(&mut self) -> Option<QueuedGameplayAction> {
        self.pending.pop_front()
    }
}

#[derive(Default)]
pub struct PendingGameplayNarrations {
    pending: Vec<Narration>,
}

impl Resource for PendingGameplayNarrations {}

impl PendingGameplayNarrations {
    pub fn drain(&mut self) -> Vec<Narration> {
        std::mem::take(&mut self.pending)
    }

    pub fn push_player(&mut self, player: &str, text: impl Into<String>, style: NarrationStyle) {
        self.pending.push(Narration {
            scope: NarrationScope::Player,
            target: Some(player.to_string()),
            text: text.into(),
            style,
            kind: None,
        });
    }

    pub fn push_broadcast(&mut self, text: impl Into<String>, style: NarrationStyle) {
        self.pending.push(Narration {
            scope: NarrationScope::Broadcast,
            target: None,
            text: text.into(),
            style,
            kind: None,
        });
    }
}

#[derive(Default)]
pub struct GameplayTick {
    tick: u64,
}

impl Resource for GameplayTick {}

impl GameplayTick {
    pub fn current_tick(&self) -> u64 {
        self.tick
    }
}

type GameplayPlayerSetReadItem<'a> = (Entity, &'a Username, &'a Position);
type GameplayPlayerSetReadFilter = With<Client>;
type GameplayPlayerSetWriteItem<'a> = (&'a mut PlayerState, &'a mut Cultivation);
type GameplayPlayerSetWriteFilter = With<Client>;
type GameplayPlayerSetParams<'w, 's> = (
    Query<'w, 's, GameplayPlayerSetReadItem<'w>, GameplayPlayerSetReadFilter>,
    Query<'w, 's, GameplayPlayerSetWriteItem<'w>, GameplayPlayerSetWriteFilter>,
);

pub fn register(app: &mut App) {
    app.insert_resource(GameplayActionQueue::default());
    app.insert_resource(PendingGameplayNarrations::default());
    app.insert_resource(GameplayTick::default());
    app.add_systems(
        Update,
        apply_queued_gameplay_actions.after(super::attach_player_state_to_joined_clients),
    );
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_queued_gameplay_actions(
    mut queue: ResMut<GameplayActionQueue>,
    mut gameplay_tick: ResMut<GameplayTick>,
    mut zone_registry: Option<ResMut<ZoneRegistry>>,
    mut active_events: Option<ResMut<ActiveEventsResource>>,
    mut pending_narrations: ResMut<PendingGameplayNarrations>,
    mut harvest_sessions: Option<ResMut<HarvestSessionStore>>,
    mut attack_intents: EventWriter<AttackIntent>,
    mut breakthrough_requests: EventWriter<BreakthroughRequest>,
    plants: Query<(Entity, &Plant)>,
    mut player_sets: ParamSet<GameplayPlayerSetParams<'_, '_>>,
) {
    gameplay_tick.tick = gameplay_tick.tick.saturating_add(1);

    while let Some(request) = queue.pop_front() {
        let player_context = {
            let read_players = player_sets.p0();
            read_players
                .iter()
                .find_map(|(entity, username, position)| {
                    player_matches_request(request.player.as_str(), username.0.as_str()).then(
                        || {
                            (
                                entity,
                                canonical_player_id(username.0.as_str()),
                                position.get(),
                                zone_name_for_position(zone_registry.as_deref(), position.get()),
                            )
                        },
                    )
                })
        };

        let Some((player_entity, canonical_player, player_position, zone_name)) = player_context
        else {
            tracing::warn!(
                "[bong][gameplay] dropped queued action for unknown player `{}`: {:?}",
                request.player,
                request.action
            );
            continue;
        };

        let event_tick = gameplay_tick.tick;

        match request.action {
            GameplayAction::Combat(action) => {
                bridge_debug_combat_action(player_entity, event_tick, action, &mut attack_intents)
            }
            GameplayAction::Gather(action) => {
                let mut mutable_players = player_sets.p1();
                let (mut player_state, mut cultivation) = mutable_players
                    .get_mut(player_entity)
                    .expect("gameplay target should still have mutable PlayerState + Cultivation");

                apply_gather_action(
                    canonical_player.as_str(),
                    player_entity,
                    player_position,
                    zone_name.as_str(),
                    event_tick,
                    &action,
                    &mut player_state,
                    &mut cultivation,
                    harvest_sessions.as_deref_mut(),
                    &plants,
                    zone_registry.as_deref_mut(),
                    active_events.as_deref_mut(),
                    &mut pending_narrations,
                )
            }
            GameplayAction::AttemptBreakthrough => {
                // Single source of truth: cultivation system consumes the breakthrough request.
                // Validation and outcomes are handled in `cultivation::breakthrough_system`.
                breakthrough_requests.send(BreakthroughRequest {
                    entity: player_entity,
                    material_bonus: 0.0,
                });
            }
        }
    }
}

fn bridge_debug_combat_action(
    attacker: Entity,
    event_tick: u64,
    action: CombatAction,
    attack_intents: &mut EventWriter<AttackIntent>,
) {
    enqueue_debug_attack_intent(
        attack_intents,
        AttackIntent {
            attacker,
            target: None,
            issued_at_tick: event_tick,
            reach: FIST_REACH,
            qi_invest: action.qi_invest.max(0.0) as f32,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: Some(action),
        },
    );
}

#[allow(clippy::too_many_arguments)]
fn apply_gather_action(
    canonical_player: &str,
    player_entity: Entity,
    player_position: valence::prelude::DVec3,
    zone_name: &str,
    event_tick: u64,
    action: &GatherAction,
    player_state: &mut PlayerState,
    cultivation: &mut Cultivation,
    harvest_sessions: Option<&mut HarvestSessionStore>,
    plants: &Query<(Entity, &Plant)>,
    zone_registry: Option<&mut ZoneRegistry>,
    active_events: Option<&mut ActiveEventsResource>,
    pending_narrations: &mut PendingGameplayNarrations,
) {
    let resource_name = empty_target_fallback(action.resource.as_str());

    if let Some(harvest_sessions) = harvest_sessions {
        if let Ok(plant_id) = canonicalize_herb_id(resource_name) {
            let target_entity = action.target_entity.or_else(|| {
                resolve_nearest_harvestable_plant(plants, plant_id, zone_name, player_position)
            });
            start_or_resume_harvest(
                harvest_sessions,
                canonical_player.trim_start_matches("offline:"),
                player_entity,
                target_entity,
                plant_id,
                action.mode.unwrap_or(BotanyHarvestMode::Manual),
                [player_position.x, player_position.y, player_position.z],
                event_tick,
            );
        }
    }

    let qi_gain = gather_qi_from_zone(zone_registry, zone_name, cultivation);
    player_state.inventory_score =
        (player_state.inventory_score + GATHER_INVENTORY_REWARD).clamp(0.0, 1.0);
    player_state.karma = (player_state.karma + GATHER_KARMA_REWARD).clamp(-1.0, 1.0);

    if let Some(active_events) = active_events {
        active_events.record_recent_event(GameEvent {
            event_type: GameEventType::ZoneQiChange,
            tick: event_tick,
            player: Some(canonical_player.to_string()),
            target: Some(resource_name.to_string()),
            zone: Some(zone_name.to_string()),
            details: Some(HashMap::from([
                ("action".to_string(), json!("gather")),
                ("resource".to_string(), json!(resource_name)),
                ("spirit_qi_gain".to_string(), json!(qi_gain)),
                ("inventory_gain".to_string(), json!(GATHER_INVENTORY_REWARD)),
            ])),
        });
    }

    pending_narrations.push_player(
        canonical_player,
        format!("你采得 {}，储物与阅历皆有所增长。", resource_name),
        NarrationStyle::Narration,
    );
}

fn gather_qi_from_zone(
    zone_registry: Option<&mut ZoneRegistry>,
    zone_name: &str,
    cultivation: &mut Cultivation,
) -> f64 {
    let Some(zone_registry) = zone_registry else {
        return 0.0;
    };
    let Some(zone) = zone_registry.find_zone_mut(zone_name) else {
        return 0.0;
    };
    let room = (cultivation.qi_max.max(1.0) - cultivation.qi_current).max(0.0);
    let available = (zone.spirit_qi.max(0.0) * QI_PER_ZONE_UNIT).max(0.0);
    let gain = QI_GATHER_REWARD.min(room).min(available);
    if gain <= 0.0 {
        return 0.0;
    }

    cultivation.qi_current += gain;
    zone.spirit_qi = (zone.spirit_qi - gain / QI_PER_ZONE_UNIT).max(0.0);
    gain
}

fn resolve_nearest_harvestable_plant(
    plants: &Query<(Entity, &Plant)>,
    plant_id: crate::botany::registry::BotanyPlantId,
    zone_name: &str,
    player_position: valence::prelude::DVec3,
) -> Option<Entity> {
    const MAX_HARVEST_DISTANCE_SQ: f64 = 6.0 * 6.0;
    plants
        .iter()
        .filter(|(_, plant)| {
            plant.id == plant_id
                && plant.zone_name == zone_name
                && !plant.harvested
                && !plant.trampled
        })
        .filter_map(|(entity, plant)| {
            let dx = player_position.x - plant.position[0];
            let dy = player_position.y - plant.position[1];
            let dz = player_position.z - plant.position[2];
            let dist_sq = dx * dx + dy * dy + dz * dz;
            (dist_sq <= MAX_HARVEST_DISTANCE_SQ).then_some((entity, dist_sq))
        })
        .min_by(|(_, a), (_, b)| a.total_cmp(b))
        .map(|(entity, _)| entity)
}

fn zone_name_for_position(
    zone_registry: Option<&ZoneRegistry>,
    position: valence::prelude::DVec3,
) -> String {
    zone_registry
        .filter(|registry| !registry.zones.is_empty())
        .and_then(|registry| registry.find_zone(DimensionKind::Overworld, position))
        .map(|zone| zone.name.clone())
        .unwrap_or_else(|| DEFAULT_SPAWN_ZONE_NAME.to_string())
}

fn player_matches_request(requested_player: &str, username: &str) -> bool {
    requested_player.eq_ignore_ascii_case(username)
        || requested_player.eq_ignore_ascii_case(canonical_player_id(username).as_str())
}

fn empty_target_fallback(value: &str) -> &str {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "无名之物"
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use valence::prelude::{App, EventReader, Position, ResMut, Update};
    use valence::testing::create_mock_client;

    #[derive(Default)]
    struct CapturedAttackIntents(Vec<AttackIntent>);

    impl valence::prelude::Resource for CapturedAttackIntents {}

    fn capture_attack_intents(
        mut events: EventReader<AttackIntent>,
        mut captured: ResMut<CapturedAttackIntents>,
    ) {
        captured.0.extend(events.read().cloned());
    }

    #[test]
    fn combat_actions_bridge_to_attack_intent_without_mutating_player_state() {
        let mut app = App::new();
        app.insert_resource(GameplayActionQueue::default());
        app.insert_resource(PendingGameplayNarrations::default());
        app.insert_resource(GameplayTick::default());
        app.insert_resource(ZoneRegistry::fallback());
        app.insert_resource(CapturedAttackIntents::default());
        app.add_event::<AttackIntent>();
        app.add_event::<BreakthroughRequest>();
        app.add_systems(
            Update,
            (
                apply_queued_gameplay_actions,
                capture_attack_intents.after(apply_queued_gameplay_actions),
            ),
        );

        let initial_state = PlayerState {
            karma: 0.05,
            inventory_score: 0.10,
        };
        let (mut client_bundle, _helper) = create_mock_client("Azure");
        client_bundle.player.position = Position::new([8.0, 66.0, 8.0]);
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                Cultivation {
                    qi_current: 70.0,
                    qi_max: 100.0,
                    ..Cultivation::default()
                },
                initial_state.clone(),
            ))
            .id();

        app.world_mut()
            .resource_mut::<GameplayActionQueue>()
            .enqueue(
                "offline:Azure",
                GameplayAction::Combat(CombatAction {
                    target: "Crimson".to_string(),
                    qi_invest: 18.0,
                }),
            );

        app.update();

        let captured = &app.world().resource::<CapturedAttackIntents>().0;
        assert_eq!(
            captured.len(),
            1,
            "combat queue should bridge into AttackIntent"
        );
        assert_eq!(captured[0].attacker, entity);
        assert_eq!(captured[0].target, None);
        assert_eq!(captured[0].issued_at_tick, 1);
        assert_eq!(captured[0].reach, FIST_REACH);
        assert_eq!(captured[0].qi_invest, 18.0);
        assert_eq!(
            captured[0].debug_command,
            Some(CombatAction {
                target: "Crimson".to_string(),
                qi_invest: 18.0,
            })
        );

        let player_state = app
            .world()
            .entity(entity)
            .get::<PlayerState>()
            .expect("player state should remain attached after bridge");
        assert_eq!(player_state, &initial_state);
    }

    #[test]
    fn gather_reward_drains_matching_zone_qi() {
        let mut zones = ZoneRegistry::fallback();
        let zone_before = zones
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .expect("fallback zone exists")
            .spirit_qi;
        let mut cultivation = Cultivation {
            qi_current: 70.0,
            qi_max: 100.0,
            ..Cultivation::default()
        };

        let gained =
            gather_qi_from_zone(Some(&mut zones), DEFAULT_SPAWN_ZONE_NAME, &mut cultivation);

        let zone_after = zones
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .expect("fallback zone exists")
            .spirit_qi;
        assert_eq!(gained, QI_GATHER_REWARD);
        assert_eq!(cultivation.qi_current, 84.0);
        assert!((zone_before - zone_after - gained / QI_PER_ZONE_UNIT).abs() < 1e-9);
    }

    #[test]
    fn gather_reward_caps_to_available_zone_qi() {
        let mut zones = ZoneRegistry::fallback();
        zones
            .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
            .expect("fallback zone exists")
            .spirit_qi = 0.1;
        let mut cultivation = Cultivation {
            qi_current: 70.0,
            qi_max: 100.0,
            ..Cultivation::default()
        };

        let gained =
            gather_qi_from_zone(Some(&mut zones), DEFAULT_SPAWN_ZONE_NAME, &mut cultivation);

        let zone_after = zones
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .expect("fallback zone exists")
            .spirit_qi;
        assert_eq!(gained, 0.1 * QI_PER_ZONE_UNIT);
        assert_eq!(cultivation.qi_current, 75.0);
        assert_eq!(zone_after, 0.0);
    }
}
