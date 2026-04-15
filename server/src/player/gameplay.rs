use std::collections::{HashMap, VecDeque};

use serde::{Deserialize, Serialize};
use serde_json::json;
use valence::prelude::{
    App, Client, Entity, EventWriter, IntoSystemConfigs, ParamSet, Position, Query, Res, ResMut,
    Resource, Update, Username, With,
};

use super::state::{canonical_player_id, PlayerState};
use crate::combat::{
    components::WoundKind,
    debug::enqueue_debug_attack_intent,
    events::{AttackIntent, FIST_REACH},
};
use crate::schema::common::{GameEventType, NarrationScope, NarrationStyle};
use crate::schema::narration::Narration;
use crate::schema::world_state::GameEvent;
use crate::world::events::ActiveEventsResource;
use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};

const GATHER_SPIRIT_QI_REWARD: f64 = 14.0;
const GATHER_EXPERIENCE_REWARD: u64 = 90;
const GATHER_INVENTORY_REWARD: f64 = 0.12;
const GATHER_KARMA_REWARD: f64 = 0.06;
const BREAKTHROUGH_RULES: [BreakthroughRule; 3] = [
    BreakthroughRule {
        current_realm: "mortal",
        next_realm: "qi_refining_1",
        required_experience: 120,
        minimum_karma: -0.2,
        required_spirit_qi: 60.0,
        next_spirit_qi_max: 120.0,
    },
    BreakthroughRule {
        current_realm: "qi_refining_1",
        next_realm: "qi_refining_2",
        required_experience: 300,
        minimum_karma: -0.1,
        required_spirit_qi: 90.0,
        next_spirit_qi_max: 140.0,
    },
    BreakthroughRule {
        current_realm: "qi_refining_2",
        next_realm: "qi_refining_3",
        required_experience: 600,
        minimum_karma: 0.0,
        required_spirit_qi: 110.0,
        next_spirit_qi_max: 160.0,
    },
];

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

    fn push_player(&mut self, player: &str, text: impl Into<String>, style: NarrationStyle) {
        self.pending.push(Narration {
            scope: NarrationScope::Player,
            target: Some(player.to_string()),
            text: text.into(),
            style,
        });
    }
}

#[derive(Default)]
pub struct GameplayTick {
    tick: u64,
}

impl Resource for GameplayTick {}

#[derive(Debug, Clone, Copy, PartialEq)]
struct BreakthroughRule {
    current_realm: &'static str,
    next_realm: &'static str,
    required_experience: u64,
    minimum_karma: f64,
    required_spirit_qi: f64,
    next_spirit_qi_max: f64,
}

type GameplayPlayerSetReadItem<'a> = (Entity, &'a Username, &'a Position, &'a PlayerState);
type GameplayPlayerSetReadFilter = With<Client>;
type GameplayPlayerSetWriteItem<'a> = &'a mut PlayerState;
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

pub(crate) fn apply_queued_gameplay_actions(
    mut queue: ResMut<GameplayActionQueue>,
    mut gameplay_tick: ResMut<GameplayTick>,
    zone_registry: Option<Res<ZoneRegistry>>,
    mut active_events: Option<ResMut<ActiveEventsResource>>,
    mut pending_narrations: ResMut<PendingGameplayNarrations>,
    mut attack_intents: EventWriter<AttackIntent>,
    mut player_sets: ParamSet<GameplayPlayerSetParams<'_, '_>>,
) {
    gameplay_tick.tick = gameplay_tick.tick.saturating_add(1);

    let zone_registry = effective_zone_registry(zone_registry.as_deref());

    while let Some(request) = queue.pop_front() {
        let player_context = {
            let read_players = player_sets.p0();
            read_players
                .iter()
                .find_map(|(entity, username, position, player_state)| {
                    player_matches_request(request.player.as_str(), username.0.as_str()).then(
                        || {
                            let validation = match &request.action {
                                GameplayAction::Combat(_) => Ok(None),
                                GameplayAction::Gather(_) => Ok(None),
                                GameplayAction::AttemptBreakthrough => {
                                    validate_breakthrough(player_state).map(Some)
                                }
                            };

                            (
                                entity,
                                canonical_player_id(username.0.as_str()),
                                zone_name_for_position(&zone_registry, position.get()),
                                validation,
                            )
                        },
                    )
                })
        };

        let Some((player_entity, canonical_player, zone_name, validation)) = player_context else {
            tracing::warn!(
                "[bong][gameplay] dropped queued action for unknown player `{}`: {:?}",
                request.player,
                request.action
            );
            continue;
        };

        match validation {
            Err(rejection) => pending_narrations.push_player(
                canonical_player.as_str(),
                rejection,
                NarrationStyle::SystemWarning,
            ),
            Ok(rule) => {
                let event_tick = gameplay_tick.tick;

                match request.action {
                    GameplayAction::Combat(action) => bridge_debug_combat_action(
                        player_entity,
                        event_tick,
                        action,
                        &mut attack_intents,
                    ),
                    GameplayAction::Gather(action) => {
                        let mut mutable_players = player_sets.p1();
                        let mut player_state = mutable_players.get_mut(player_entity).expect(
                            "validated gameplay target should still have mutable PlayerState",
                        );

                        apply_gather_action(
                            canonical_player.as_str(),
                            zone_name.as_str(),
                            event_tick,
                            &action,
                            &mut player_state,
                            active_events.as_deref_mut(),
                            &mut pending_narrations,
                        )
                    }
                    GameplayAction::AttemptBreakthrough => {
                        let mut mutable_players = player_sets.p1();
                        let mut player_state = mutable_players.get_mut(player_entity).expect(
                            "validated gameplay target should still have mutable PlayerState",
                        );

                        apply_breakthrough_action(
                            canonical_player.as_str(),
                            zone_name.as_str(),
                            event_tick,
                            rule.expect("breakthrough action should carry a rule after validation"),
                            &mut player_state,
                            active_events.as_deref_mut(),
                            &mut pending_narrations,
                        )
                    }
                }
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
            debug_command: Some(action),
        },
    );
}

fn apply_gather_action(
    canonical_player: &str,
    zone_name: &str,
    event_tick: u64,
    action: &GatherAction,
    player_state: &mut PlayerState,
    active_events: Option<&mut ActiveEventsResource>,
    pending_narrations: &mut PendingGameplayNarrations,
) {
    let resource_name = empty_target_fallback(action.resource.as_str());

    player_state.spirit_qi =
        (player_state.spirit_qi + GATHER_SPIRIT_QI_REWARD).clamp(0.0, player_state.spirit_qi_max);
    player_state.experience = player_state
        .experience
        .saturating_add(GATHER_EXPERIENCE_REWARD);
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
                (
                    "experience_gain".to_string(),
                    json!(GATHER_EXPERIENCE_REWARD),
                ),
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

fn apply_breakthrough_action(
    canonical_player: &str,
    zone_name: &str,
    event_tick: u64,
    rule: &BreakthroughRule,
    player_state: &mut PlayerState,
    active_events: Option<&mut ActiveEventsResource>,
    pending_narrations: &mut PendingGameplayNarrations,
) {
    let from_realm = player_state.realm.clone();
    player_state.realm = rule.next_realm.to_string();
    player_state.spirit_qi_max = rule.next_spirit_qi_max;
    player_state.spirit_qi = rule.next_spirit_qi_max;
    player_state.karma = (player_state.karma + 0.08).clamp(-1.0, 1.0);

    if let Some(active_events) = active_events {
        active_events.record_recent_event(GameEvent {
            event_type: GameEventType::EventTriggered,
            tick: event_tick,
            player: Some(canonical_player.to_string()),
            target: Some(rule.next_realm.to_string()),
            zone: Some(zone_name.to_string()),
            details: Some(HashMap::from([
                ("action".to_string(), json!("realm_breakthrough")),
                ("from_realm".to_string(), json!(from_realm)),
                ("to_realm".to_string(), json!(rule.next_realm)),
                (
                    "required_experience".to_string(),
                    json!(rule.required_experience),
                ),
            ])),
        });
    }

    pending_narrations.push_player(
        canonical_player,
        format!(
            "你已突破至 {}，灵海扩张至 {:.0}/{:.0}。",
            realm_display_name(rule.next_realm),
            player_state.spirit_qi,
            player_state.spirit_qi_max
        ),
        NarrationStyle::SystemWarning,
    );
}

fn validate_breakthrough(player_state: &PlayerState) -> Result<&'static BreakthroughRule, String> {
    let Some(rule) = breakthrough_rule(player_state.realm.as_str()) else {
        return Err(format!(
            "{} 暂无进一步的最小验证突破路径。",
            realm_display_name(player_state.realm.as_str())
        ));
    };

    if player_state.experience < rule.required_experience {
        return Err(format!(
            "突破未成：{} 需要至少 {} 点经验。",
            realm_display_name(rule.next_realm),
            rule.required_experience
        ));
    }

    if player_state.karma < rule.minimum_karma {
        return Err(format!(
            "突破未成：心境未稳，因果需不低于 {:.2}。",
            rule.minimum_karma
        ));
    }

    if player_state.spirit_qi < rule.required_spirit_qi {
        return Err(format!(
            "突破未成：灵气尚浅，需至少 {:.0}/{:.0}。",
            rule.required_spirit_qi, player_state.spirit_qi_max
        ));
    }

    Ok(rule)
}

fn effective_zone_registry(zone_registry: Option<&ZoneRegistry>) -> ZoneRegistry {
    match zone_registry {
        Some(zone_registry) if !zone_registry.zones.is_empty() => zone_registry.clone(),
        _ => ZoneRegistry::fallback(),
    }
}

fn zone_name_for_position(
    zone_registry: &ZoneRegistry,
    position: valence::prelude::DVec3,
) -> String {
    zone_registry
        .find_zone(position)
        .map(|zone| zone.name.clone())
        .unwrap_or_else(|| DEFAULT_SPAWN_ZONE_NAME.to_string())
}

fn player_matches_request(requested_player: &str, username: &str) -> bool {
    requested_player.eq_ignore_ascii_case(username)
        || requested_player.eq_ignore_ascii_case(canonical_player_id(username).as_str())
}

fn breakthrough_rule(current_realm: &str) -> Option<&'static BreakthroughRule> {
    let current_realm = current_realm.trim();
    BREAKTHROUGH_RULES
        .iter()
        .find(|rule| rule.current_realm.eq_ignore_ascii_case(current_realm))
}

fn realm_display_name(realm: &str) -> &'static str {
    match realm.trim().to_ascii_lowercase().as_str() {
        "mortal" => "凡体",
        "qi_refining_1" => "炼气一层",
        "qi_refining_2" => "炼气二层",
        "qi_refining_3" => "炼气三层",
        _ => "未知境界",
    }
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
        app.add_systems(
            Update,
            (
                apply_queued_gameplay_actions,
                capture_attack_intents.after(apply_queued_gameplay_actions),
            ),
        );

        let initial_state = PlayerState {
            realm: "qi_refining_1".to_string(),
            spirit_qi: 70.0,
            spirit_qi_max: 100.0,
            karma: 0.05,
            experience: 200,
            inventory_score: 0.10,
        };
        let (mut client_bundle, _helper) = create_mock_client("Azure");
        client_bundle.player.position = Position::new([8.0, 66.0, 8.0]);
        let entity = app
            .world_mut()
            .spawn((client_bundle, initial_state.clone()))
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
}
