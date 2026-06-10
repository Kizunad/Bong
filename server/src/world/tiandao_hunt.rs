use crate::combat::components::{CombatState, Lifecycle};
use crate::cultivation::components::{Cultivation, Realm};
use crate::cultivation::tick::{CultivationClock, CultivationSessionPracticeAccumulator};
use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest};
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::network::{redis_bridge::RedisOutbound, RedisBridgeResource};
use crate::player::state::canonical_player_id;
use crate::qi_physics::{
    constants::{
        QI_TIANDAO_MOVING_ESCAPE_DECAY_MULTIPLIER, QI_TIANDAO_WATCH_ZONE_DRAIN_PER_MINUTE,
    },
    QiAccountId, QiTransfer, QiTransferReason, WorldQiAccount,
};
use crate::schema::agent_command::Command;
use crate::schema::common::CommandType;
use crate::schema::cultivation::realm_to_string;
use crate::schema::tiandao_hunt_narration::{
    TiandaoHuntNarrationRequestV1, TiandaoHuntResponseLevelV1,
};
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::social::{position_is_within_own_active_spirit_niche, SpiritNicheRegistry};
use crate::world::calamity::{EVENT_BEAST_TIDE, EVENT_REALM_COLLAPSE, EVENT_THUNDER_TRIBULATION};
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::events::ActiveEventsResource;
use crate::world::season::{Season, WorldSeasonState};
use crate::world::zone::ZoneRegistry;
use crate::zhenfa::{DeceiveHeavenEvent, DeceiveHeavenExposedEvent, ZhenfaSystemSet};
use serde_json::json;
use std::collections::HashMap;
use valence::prelude::{
    bevy_ecs, bevy_ecs::system::SystemParam, ident, App, Client, Commands, Component, DVec3,
    Entity, Events, IntoSystemConfigs, Local, Position, Query, Res, ResMut, Resource, Update,
    Username, With, Without,
};

const ATTENTION_MIN: f64 = 0.0;
const ATTENTION_MAX: f64 = 100.0;
pub const TIANDAO_HUNT_EVAL_INTERVAL_TICKS: u64 = 10 * 20;
const TIANDAO_PRESENCE_SPIRIT_REALM_MIN_RANK: u8 = 4;
const TIANDAO_PRESSURE_EVENT_INTERVAL_TICKS: u64 = 3 * 60 * 20;
const TIANDAO_TRIBULATION_EVENT_INTERVAL_TICKS: u64 = 5 * 60 * 20;
const TIANDAO_ANNIHILATE_EVENT_INTERVAL_TICKS: u64 = 2 * 60 * 20;
const TIANDAO_AUDIO_INSTANCE_BASE: u64 = 710_000;
pub const DECEIVE_HEAVEN_DECOY_MIN_DISTANCE_BLOCKS: f64 = 500.0;
pub const DECEIVE_HEAVEN_DECOY_DURATION_TICKS: u64 = 30 * 60 * 20;
pub const DECEIVE_HEAVEN_DECOY_DECAY_MULTIPLIER: f64 = 4.0;
pub const DECEIVE_HEAVEN_REVEAL_PENALTY: f64 = 20.0;
const TIANDAO_MOVING_DISTANCE_EPSILON_BLOCKS: f64 = 0.1;

#[derive(Debug, Clone, Component, PartialEq)]
pub struct TiandaoAttention {
    pub level: f64,
    pub response: TiandaoResponseLevel,
    pub last_eval_tick: u64,
    pub accumulation_rate: f64,
    pub peak_level: f64,
    pub last_response_tick: u64,
    pub last_emitted_response: TiandaoResponseLevel,
    pub narration_count: u32,
}

impl Default for TiandaoAttention {
    fn default() -> Self {
        Self {
            level: 0.0,
            response: TiandaoResponseLevel::None,
            last_eval_tick: 0,
            accumulation_rate: 0.0,
            peak_level: 0.0,
            last_response_tick: 0,
            last_emitted_response: TiandaoResponseLevel::None,
            narration_count: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TiandaoResponseLevel {
    None,
    Watch,
    Pressure,
    Tribulation,
    Annihilate,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TiandaoActivity {
    Meditating,
    Combat,
    Moving,
    Standing,
    InNiche,
}

type AttentionAttachFilter = (With<Client>, With<Cultivation>, Without<TiandaoAttention>);
type TiandaoPlayerQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Cultivation,
        &'static Position,
        &'static Username,
        &'static mut Client,
        Option<&'static CurrentDimension>,
        Option<&'static CombatState>,
        Option<&'static Lifecycle>,
        &'static mut TiandaoAttention,
    ),
    With<Client>,
>;

#[derive(SystemParam)]
pub struct TiandaoHuntResources<'w, 's> {
    clock: Option<Res<'w, CultivationClock>>,
    practice_accumulator: Option<Res<'w, CultivationSessionPracticeAccumulator>>,
    spirit_niches: Option<Res<'w, SpiritNicheRegistry>>,
    activity: ResMut<'w, TiandaoActivityRuntimeState>,
    deceive_heaven: DeceiveHeavenRuntimeParams<'w, 's>,
    zones: Option<ResMut<'w, ZoneRegistry>>,
    season: Option<Res<'w, WorldSeasonState>>,
    active_events: Option<ResMut<'w, ActiveEventsResource>>,
    vfx_events: Option<ResMut<'w, Events<VfxEventRequest>>>,
    audio_events: Option<ResMut<'w, Events<PlaySoundRecipeRequest>>>,
    redis: Option<Res<'w, RedisBridgeResource>>,
    qi_ledger: Option<ResMut<'w, WorldQiAccount>>,
}

#[derive(SystemParam)]
pub struct DeceiveHeavenRuntimeParams<'w, 's> {
    state: ResMut<'w, DeceiveHeavenRuntimeState>,
    deceive_events: Option<Res<'w, Events<DeceiveHeavenEvent>>>,
    deceive_exposed_events: Option<Res<'w, Events<DeceiveHeavenExposedEvent>>>,
    deceive_event_reader: Local<'s, bevy_ecs::event::ManualEventReader<DeceiveHeavenEvent>>,
    deceive_exposed_event_reader:
        Local<'s, bevy_ecs::event::ManualEventReader<DeceiveHeavenExposedEvent>>,
}

struct TiandaoResponseSinks<'a> {
    zones: Option<&'a mut ZoneRegistry>,
    active_events: Option<&'a mut ActiveEventsResource>,
    vfx_events: Option<&'a mut Events<VfxEventRequest>>,
    audio_events: Option<&'a mut Events<PlaySoundRecipeRequest>>,
    redis: Option<&'a RedisBridgeResource>,
    qi_ledger: Option<&'a mut WorldQiAccount>,
}

#[derive(Debug, Clone, Default, Resource)]
pub struct TiandaoActivityRuntimeState {
    last_eval_position_by_entity: HashMap<Entity, DVec3>,
}

impl TiandaoActivityRuntimeState {
    fn activity_for_eval(&mut self, input: TiandaoActivityRuntimeInput<'_>) -> TiandaoActivity {
        let moved_since_last_eval = self
            .last_eval_position_by_entity
            .insert(input.entity, input.position)
            .is_some_and(|previous| {
                (input.position - previous).length() > TIANDAO_MOVING_DISTANCE_EPSILON_BLOCKS
            });

        if input
            .combat
            .and_then(|combat| combat.in_combat_until_tick)
            .is_some_and(|until_tick| until_tick > input.now_tick)
        {
            return TiandaoActivity::Combat;
        }

        if let (Some(lifecycle), Some(spirit_niches)) = (input.lifecycle, input.spirit_niches) {
            if position_is_within_own_active_spirit_niche(
                lifecycle.character_id.as_str(),
                input.position,
                spirit_niches,
            ) {
                return TiandaoActivity::InNiche;
            }
        }

        if input.practice_accumulator.is_some_and(|accumulator| {
            accumulator.is_recently_practicing(input.entity, input.now_tick)
        }) {
            return TiandaoActivity::Meditating;
        }

        if moved_since_last_eval {
            TiandaoActivity::Moving
        } else {
            TiandaoActivity::Standing
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct TiandaoActivityRuntimeInput<'a> {
    entity: Entity,
    position: DVec3,
    combat: Option<&'a CombatState>,
    lifecycle: Option<&'a Lifecycle>,
    practice_accumulator: Option<&'a CultivationSessionPracticeAccumulator>,
    spirit_niches: Option<&'a SpiritNicheRegistry>,
    now_tick: u64,
}

impl Default for TiandaoActivity {
    fn default() -> Self {
        Self::Standing
    }
}

impl TiandaoActivity {
    pub const fn as_wire(self) -> &'static str {
        match self {
            Self::Meditating => "meditating",
            Self::Combat => "combat",
            Self::Moving => "moving",
            Self::Standing => "standing",
            Self::InNiche => "in_niche",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TiandaoAttentionInput {
    pub realm: Realm,
    pub zone_spirit_qi: f64,
    pub activity: TiandaoActivity,
    pub season: Season,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TiandaoCountermeasureInput {
    pub deceive_heaven_decoy: Option<DeceiveHeavenDecoyInput>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeceiveHeavenDecoyInput {
    pub placed_tick: u64,
    pub distance_blocks: f64,
    pub exposed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeceiveHeavenOutcome {
    None,
    TooClose,
    Expired,
    Diverted,
    Revealed,
}

impl DeceiveHeavenOutcome {
    pub const fn as_wire(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::TooClose => "too_close",
            Self::Expired => "expired",
            Self::Diverted => "diverted",
            Self::Revealed => "revealed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TiandaoCountermeasureOutcome {
    pub deceive_heaven: DeceiveHeavenOutcome,
    pub decay_multiplier: f64,
    pub attention_penalty: f64,
}

impl Default for TiandaoCountermeasureOutcome {
    fn default() -> Self {
        Self {
            deceive_heaven: DeceiveHeavenOutcome::None,
            decay_multiplier: 1.0,
            attention_penalty: 0.0,
        }
    }
}

#[derive(Debug, Clone, Default, Resource)]
pub struct DeceiveHeavenRuntimeState {
    deployments: HashMap<u64, DeceiveHeavenRuntimeDeployment>,
}

#[derive(Debug, Clone, PartialEq)]
struct DeceiveHeavenRuntimeDeployment {
    owner_player_id: String,
    pos: [i32; 3],
    placed_at_tick: u64,
    exposed_at_tick: Option<u64>,
    exposure_penalty_pending: bool,
}

impl DeceiveHeavenRuntimeState {
    fn record_events(
        &mut self,
        deceive_events: Option<&Events<DeceiveHeavenEvent>>,
        deceive_exposed_events: Option<&Events<DeceiveHeavenExposedEvent>>,
        deceive_event_reader: &mut bevy_ecs::event::ManualEventReader<DeceiveHeavenEvent>,
        deceive_exposed_event_reader: &mut bevy_ecs::event::ManualEventReader<
            DeceiveHeavenExposedEvent,
        >,
    ) {
        if let Some(events) = deceive_events {
            for event in deceive_event_reader.read(events) {
                self.record_deployment(event);
            }
        }
        if let Some(events) = deceive_exposed_events {
            for event in deceive_exposed_event_reader.read(events) {
                self.record_exposure(event);
            }
        }
    }

    fn record_deployment(&mut self, event: &DeceiveHeavenEvent) {
        self.deployments.insert(
            event.array_id,
            DeceiveHeavenRuntimeDeployment {
                owner_player_id: event.owner_player_id.clone(),
                pos: event.pos,
                placed_at_tick: event.placed_at_tick,
                exposed_at_tick: None,
                exposure_penalty_pending: false,
            },
        );
    }

    fn record_exposure(&mut self, event: &DeceiveHeavenExposedEvent) {
        self.deployments
            .entry(event.array_id)
            .and_modify(|deployment| {
                deployment.exposed_at_tick = Some(event.exposed_at_tick);
                deployment.exposure_penalty_pending = true;
            })
            .or_insert_with(|| DeceiveHeavenRuntimeDeployment {
                owner_player_id: event.owner_player_id.clone(),
                pos: event.pos,
                placed_at_tick: event.exposed_at_tick,
                exposed_at_tick: Some(event.exposed_at_tick),
                exposure_penalty_pending: true,
            });
    }

    fn prune_expired(&mut self, now_tick: u64) {
        self.deployments.retain(|_, deployment| {
            now_tick.saturating_sub(deployment.placed_at_tick) < DECEIVE_HEAVEN_DECOY_DURATION_TICKS
        });
    }

    fn countermeasure_input(
        &self,
        owner_player_id: &str,
        player_pos: DVec3,
        now_tick: u64,
    ) -> TiandaoCountermeasureInput {
        let mut exposed_candidate = None;
        let mut diverted_candidate = None;
        let mut inactive_candidate = None;

        for deployment in self.deployments.values() {
            if deployment.owner_player_id != owner_player_id {
                continue;
            }

            let decoy = deployment.decoy_input(player_pos);
            match deceive_heaven_decoy_outcome(decoy, now_tick) {
                DeceiveHeavenOutcome::Revealed if deployment.exposure_penalty_pending => {
                    exposed_candidate = Some(decoy);
                    break;
                }
                DeceiveHeavenOutcome::Diverted if diverted_candidate.is_none() => {
                    diverted_candidate = Some(decoy);
                }
                DeceiveHeavenOutcome::TooClose | DeceiveHeavenOutcome::Expired
                    if inactive_candidate.is_none() =>
                {
                    inactive_candidate = Some(decoy);
                }
                DeceiveHeavenOutcome::None
                | DeceiveHeavenOutcome::TooClose
                | DeceiveHeavenOutcome::Expired
                | DeceiveHeavenOutcome::Diverted
                | DeceiveHeavenOutcome::Revealed => {}
            }
        }

        if let Some(decoy) = exposed_candidate {
            return TiandaoCountermeasureInput {
                deceive_heaven_decoy: Some(decoy),
            };
        }

        TiandaoCountermeasureInput {
            deceive_heaven_decoy: diverted_candidate.or(inactive_candidate),
        }
    }

    fn mark_countermeasure_applied(
        &mut self,
        owner_player_id: &str,
        player_pos: DVec3,
        now_tick: u64,
        outcome: TiandaoCountermeasureOutcome,
    ) {
        if outcome.deceive_heaven != DeceiveHeavenOutcome::Revealed {
            return;
        }

        let applied_array_id = self.deployments.iter().find_map(|(array_id, deployment)| {
            if deployment.owner_player_id != owner_player_id || !deployment.exposure_penalty_pending
            {
                return None;
            }

            if deceive_heaven_decoy_outcome(deployment.decoy_input(player_pos), now_tick)
                == DeceiveHeavenOutcome::Revealed
            {
                Some(*array_id)
            } else {
                None
            }
        });

        if let Some(array_id) = applied_array_id {
            self.deployments.remove(&array_id);
        }
    }
}

impl DeceiveHeavenRuntimeDeployment {
    fn decoy_input(&self, player_pos: DVec3) -> DeceiveHeavenDecoyInput {
        DeceiveHeavenDecoyInput {
            placed_tick: self.placed_at_tick,
            distance_blocks: distance_to_array(player_pos, self.pos),
            exposed: self.exposure_penalty_pending,
        }
    }
}

impl DeceiveHeavenRuntimeParams<'_, '_> {
    fn sync_events(&mut self, now_tick: u64) {
        self.state.record_events(
            self.deceive_events.as_deref(),
            self.deceive_exposed_events.as_deref(),
            &mut self.deceive_event_reader,
            &mut self.deceive_exposed_event_reader,
        );
        self.state.prune_expired(now_tick);
    }

    fn countermeasure_input(
        &self,
        owner_player_id: &str,
        player_pos: DVec3,
        now_tick: u64,
    ) -> TiandaoCountermeasureInput {
        self.state
            .countermeasure_input(owner_player_id, player_pos, now_tick)
    }

    fn mark_countermeasure_applied(
        &mut self,
        owner_player_id: &str,
        player_pos: DVec3,
        now_tick: u64,
        outcome: TiandaoCountermeasureOutcome,
    ) {
        self.state
            .mark_countermeasure_applied(owner_player_id, player_pos, now_tick, outcome);
    }
}

pub fn register(app: &mut App) {
    app.init_resource::<DeceiveHeavenRuntimeState>();
    app.init_resource::<TiandaoActivityRuntimeState>();
    app.add_systems(
        Update,
        (
            attach_tiandao_attention,
            tiandao_hunt_tick.after(ZhenfaSystemSet::Runtime),
        ),
    );
}

pub fn attach_tiandao_attention(
    mut commands: Commands,
    players: Query<Entity, AttentionAttachFilter>,
) {
    for entity in players.iter() {
        commands.entity(entity).insert(TiandaoAttention::default());
    }
}

pub fn tiandao_hunt_tick(
    mut resources: TiandaoHuntResources<'_, '_>,
    mut players: TiandaoPlayerQuery<'_, '_>,
) {
    let Some(clock) = resources.clock.as_deref() else {
        return;
    };
    let now_tick = clock.tick;
    resources.deceive_heaven.sync_events(now_tick);
    let season = resources
        .season
        .as_deref()
        .map(|state| state.current.season)
        .unwrap_or_default();
    for (
        player_entity,
        cultivation,
        position,
        username,
        mut client,
        dimension,
        combat,
        lifecycle,
        mut attention,
    ) in players.iter_mut()
    {
        if !should_evaluate_attention(now_tick, attention.last_eval_tick) {
            continue;
        }
        let activity = resources
            .activity
            .activity_for_eval(TiandaoActivityRuntimeInput {
                entity: player_entity,
                position: position.0,
                combat,
                lifecycle,
                practice_accumulator: resources.practice_accumulator.as_deref(),
                spirit_niches: resources.spirit_niches.as_deref(),
                now_tick,
            });
        let owner_player_id = canonical_player_id(username.0.as_str());
        let countermeasures =
            resources
                .deceive_heaven
                .countermeasure_input(&owner_player_id, position.0, now_tick);
        let eval = apply_attention_eval(
            cultivation,
            position,
            &mut attention,
            TiandaoEvalContext {
                dimension,
                zones: resources.zones.as_deref(),
                season,
                activity,
                countermeasures,
                now_tick,
            },
        );
        let Some(eval) = eval else {
            continue;
        };
        resources.deceive_heaven.mark_countermeasure_applied(
            &owner_player_id,
            position.0,
            now_tick,
            eval.countermeasure,
        );

        emit_tiandao_presence_payload(
            &mut client,
            cultivation.realm,
            &attention,
            eval.zone_name.as_deref().unwrap_or("unknown"),
            eval.zone_spirit_qi,
            now_tick,
        );

        apply_tiandao_response_chain(
            &mut attention,
            eval,
            player_entity,
            username.0.as_str(),
            TiandaoResponseSinks {
                zones: resources.zones.as_deref_mut(),
                active_events: resources.active_events.as_deref_mut(),
                vfx_events: resources.vfx_events.as_deref_mut(),
                audio_events: resources.audio_events.as_deref_mut(),
                redis: resources.redis.as_deref(),
                qi_ledger: resources.qi_ledger.as_deref_mut(),
            },
            now_tick,
        );
    }
}

pub fn apply_attention_eval(
    cultivation: &Cultivation,
    position: &Position,
    attention: &mut TiandaoAttention,
    context: TiandaoEvalContext<'_>,
) -> Option<TiandaoEvalSnapshot> {
    let dimension = context
        .dimension
        .map(|dim| dim.0)
        .unwrap_or(DimensionKind::Overworld);
    let zone = context
        .zones
        .and_then(|registry| registry.find_zone(dimension, position.0))
        .map(|zone| (zone.name.clone(), zone.spirit_qi));
    let zone_spirit_qi = zone
        .as_ref()
        .map(|(_, spirit_qi)| *spirit_qi)
        .unwrap_or_default();
    let input = TiandaoAttentionInput {
        realm: cultivation.realm,
        zone_spirit_qi,
        activity: context.activity,
        season: context.season,
    };
    if should_evaluate_attention(context.now_tick, attention.last_eval_tick) {
        let countermeasure = advance_attention_with_countermeasures(
            attention,
            input,
            context.countermeasures,
            context.now_tick,
        );
        Some(TiandaoEvalSnapshot {
            position: position.0,
            zone_name: zone.map(|(name, _)| name),
            zone_spirit_qi,
            realm: cultivation.realm,
            activity: context.activity,
            response: attention.response,
            level: attention.level,
            countermeasure,
        })
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TiandaoEvalContext<'a> {
    pub dimension: Option<&'a CurrentDimension>,
    pub zones: Option<&'a ZoneRegistry>,
    pub season: Season,
    pub activity: TiandaoActivity,
    pub countermeasures: TiandaoCountermeasureInput,
    pub now_tick: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TiandaoEvalSnapshot {
    pub position: DVec3,
    pub zone_name: Option<String>,
    pub zone_spirit_qi: f64,
    pub realm: Realm,
    pub activity: TiandaoActivity,
    pub response: TiandaoResponseLevel,
    pub level: f64,
    pub countermeasure: TiandaoCountermeasureOutcome,
}

pub fn should_evaluate_attention(now_tick: u64, last_eval_tick: u64) -> bool {
    now_tick >= last_eval_tick
        && now_tick.saturating_sub(last_eval_tick) >= TIANDAO_HUNT_EVAL_INTERVAL_TICKS
}

#[cfg(test)]
pub fn advance_attention(
    attention: &mut TiandaoAttention,
    input: TiandaoAttentionInput,
    eval_tick: u64,
) {
    advance_attention_with_countermeasures(
        attention,
        input,
        TiandaoCountermeasureInput::default(),
        eval_tick,
    );
}

pub fn advance_attention_with_countermeasures(
    attention: &mut TiandaoAttention,
    input: TiandaoAttentionInput,
    countermeasures: TiandaoCountermeasureInput,
    eval_tick: u64,
) -> TiandaoCountermeasureOutcome {
    let previous_response = attention.response;
    let accumulation_rate = accumulation_rate(input);
    let outcome = countermeasure_outcome(countermeasures, eval_tick);
    let decay = attention_decay_for_eval(previous_response, input, accumulation_rate)
        * outcome.decay_multiplier;
    let next_level = (attention.level + accumulation_rate - decay + outcome.attention_penalty)
        .clamp(ATTENTION_MIN, ATTENTION_MAX);

    attention.level = next_level;
    attention.accumulation_rate = accumulation_rate;
    attention.peak_level = attention.peak_level.max(next_level);
    attention.response = response_for_level(previous_response, next_level);
    attention.last_eval_tick = eval_tick;
    outcome
}

pub fn accumulation_rate(input: TiandaoAttentionInput) -> f64 {
    realm_base_rate(input.realm)
        * zone_qi_factor(input.zone_spirit_qi)
        * activity_factor(input.activity)
        * season_factor(input.season)
}

pub const fn realm_base_rate(realm: Realm) -> f64 {
    match realm {
        Realm::Awaken | Realm::Induce => 0.0,
        Realm::Condense => 0.01,
        Realm::Solidify => 0.05,
        Realm::Spirit => 0.15,
        Realm::Void => 0.40,
    }
}

pub fn zone_qi_factor(spirit_qi: f64) -> f64 {
    if spirit_qi <= 0.1 {
        0.3
    } else if spirit_qi <= 0.3 {
        0.6
    } else if spirit_qi <= 0.6 {
        1.0
    } else {
        1.8
    }
}

pub const fn activity_factor(activity: TiandaoActivity) -> f64 {
    match activity {
        TiandaoActivity::Meditating => 1.5,
        TiandaoActivity::Combat => 1.2,
        TiandaoActivity::Moving => 0.8,
        TiandaoActivity::Standing => 1.0,
        TiandaoActivity::InNiche => 0.5,
    }
}

pub const fn season_factor(season: Season) -> f64 {
    if season.is_xizhuan() {
        1.5
    } else {
        1.0
    }
}

pub fn decay_rate(response: TiandaoResponseLevel, zone_spirit_qi: f64) -> f64 {
    let base = match response {
        TiandaoResponseLevel::None => 0.08,
        TiandaoResponseLevel::Watch => 0.05,
        TiandaoResponseLevel::Pressure => 0.03,
        TiandaoResponseLevel::Tribulation => 0.01,
        TiandaoResponseLevel::Annihilate => 0.0,
    };
    base * zone_decay_multiplier(zone_spirit_qi)
}

fn attention_decay_for_eval(
    response: TiandaoResponseLevel,
    input: TiandaoAttentionInput,
    accumulation_rate: f64,
) -> f64 {
    if response == TiandaoResponseLevel::None
        && accumulation_rate > 0.0
        && input.activity != TiandaoActivity::Moving
    {
        0.0
    } else if input.activity == TiandaoActivity::Moving
        && matches!(
            response,
            TiandaoResponseLevel::Pressure | TiandaoResponseLevel::Tribulation
        )
    {
        decay_rate(response, input.zone_spirit_qi) * QI_TIANDAO_MOVING_ESCAPE_DECAY_MULTIPLIER
    } else {
        decay_rate(response, input.zone_spirit_qi)
    }
}

pub fn countermeasure_outcome(
    input: TiandaoCountermeasureInput,
    eval_tick: u64,
) -> TiandaoCountermeasureOutcome {
    let Some(decoy) = input.deceive_heaven_decoy else {
        return TiandaoCountermeasureOutcome::default();
    };
    let deceive_heaven = deceive_heaven_decoy_outcome(decoy, eval_tick);
    match deceive_heaven {
        DeceiveHeavenOutcome::Diverted => TiandaoCountermeasureOutcome {
            deceive_heaven,
            decay_multiplier: DECEIVE_HEAVEN_DECOY_DECAY_MULTIPLIER,
            attention_penalty: 0.0,
        },
        DeceiveHeavenOutcome::Revealed => TiandaoCountermeasureOutcome {
            deceive_heaven,
            decay_multiplier: 1.0,
            attention_penalty: DECEIVE_HEAVEN_REVEAL_PENALTY,
        },
        DeceiveHeavenOutcome::None
        | DeceiveHeavenOutcome::TooClose
        | DeceiveHeavenOutcome::Expired => TiandaoCountermeasureOutcome {
            deceive_heaven,
            ..TiandaoCountermeasureOutcome::default()
        },
    }
}

pub fn deceive_heaven_decoy_outcome(
    decoy: DeceiveHeavenDecoyInput,
    eval_tick: u64,
) -> DeceiveHeavenOutcome {
    if decoy.exposed {
        return DeceiveHeavenOutcome::Revealed;
    }
    if !decoy.distance_blocks.is_finite()
        || decoy.distance_blocks < DECEIVE_HEAVEN_DECOY_MIN_DISTANCE_BLOCKS
    {
        return DeceiveHeavenOutcome::TooClose;
    }
    if eval_tick.saturating_sub(decoy.placed_tick) >= DECEIVE_HEAVEN_DECOY_DURATION_TICKS {
        return DeceiveHeavenOutcome::Expired;
    }
    DeceiveHeavenOutcome::Diverted
}

fn distance_to_array(player_pos: DVec3, array_pos: [i32; 3]) -> f64 {
    let array_center = DVec3::new(
        f64::from(array_pos[0]) + 0.5,
        f64::from(array_pos[1]),
        f64::from(array_pos[2]) + 0.5,
    );
    (player_pos - array_center).length()
}

pub fn zone_decay_multiplier(spirit_qi: f64) -> f64 {
    if spirit_qi < 0.0 {
        5.0
    } else if spirit_qi == 0.0 {
        3.0
    } else {
        1.0
    }
}

pub fn response_for_level(previous: TiandaoResponseLevel, level: f64) -> TiandaoResponseLevel {
    match previous {
        TiandaoResponseLevel::None => {
            if level >= 15.0 {
                TiandaoResponseLevel::Watch
            } else {
                TiandaoResponseLevel::None
            }
        }
        TiandaoResponseLevel::Watch => {
            if level >= 40.0 {
                TiandaoResponseLevel::Pressure
            } else if level < 10.0 {
                TiandaoResponseLevel::None
            } else {
                TiandaoResponseLevel::Watch
            }
        }
        TiandaoResponseLevel::Pressure => {
            if level >= 70.0 {
                TiandaoResponseLevel::Tribulation
            } else if level < 30.0 {
                TiandaoResponseLevel::Watch
            } else {
                TiandaoResponseLevel::Pressure
            }
        }
        TiandaoResponseLevel::Tribulation => {
            if level >= 90.0 {
                TiandaoResponseLevel::Annihilate
            } else if level < 60.0 {
                TiandaoResponseLevel::Pressure
            } else {
                TiandaoResponseLevel::Tribulation
            }
        }
        TiandaoResponseLevel::Annihilate => {
            if level < 80.0 {
                TiandaoResponseLevel::Tribulation
            } else {
                TiandaoResponseLevel::Annihilate
            }
        }
    }
}

fn apply_tiandao_response_chain(
    attention: &mut TiandaoAttention,
    eval: TiandaoEvalSnapshot,
    player_entity: Entity,
    username: &str,
    sinks: TiandaoResponseSinks<'_>,
    now_tick: u64,
) {
    let Some(profile) = tiandao_response_profile(eval.response) else {
        attention.last_response_tick = 0;
        attention.last_emitted_response = TiandaoResponseLevel::None;
        attention.narration_count = 0;
        return;
    };

    if attention.last_emitted_response == eval.response
        && attention.last_response_tick != 0
        && now_tick.saturating_sub(attention.last_response_tick) < profile.interval_ticks
    {
        return;
    }
    let narration_count = if attention.last_emitted_response == eval.response {
        attention.narration_count = attention.narration_count.saturating_add(1);
        attention.narration_count
    } else {
        attention.narration_count = 0;
        0
    };
    attention.last_response_tick = now_tick;
    attention.last_emitted_response = eval.response;

    if let Some(audio_events) = sinks.audio_events {
        audio_events.send(tiandao_audio_request(&eval, profile, player_entity));
    }
    if let (Some(vfx_events), Some(request)) =
        (sinks.vfx_events, tiandao_vfx_request(&eval, profile))
    {
        vfx_events.send(request);
    }
    if let (Some(redis), Some(request)) = (
        sinks.redis,
        tiandao_narration_request(&eval, username, narration_count),
    ) {
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::TiandaoHuntNarrationRequest(request));
    }

    if eval.response == TiandaoResponseLevel::Watch {
        if let (Some(zone_name), Some(zones), Some(qi_ledger)) =
            (eval.zone_name.as_deref(), sinks.zones, sinks.qi_ledger)
        {
            apply_watch_zone_qi_drain(zone_name, profile.interval_ticks, zones, qi_ledger);
        }
        return;
    }

    let Some(event_name) = profile.event_name else {
        return;
    };
    let (Some(zone_name), Some(active_events), Some(zones)) =
        (eval.zone_name.as_deref(), sinks.active_events, sinks.zones)
    else {
        return;
    };
    let target_player = canonical_player_id(username);
    let command = tiandao_spawn_event_command(
        zone_name,
        event_name,
        profile.duration_ticks,
        profile.intensity,
        target_player.as_str(),
        eval.response,
    );
    active_events.enqueue_from_spawn_command_with_karma(&command, Some(zones), None, None);
}

fn apply_watch_zone_qi_drain(
    zone_name: &str,
    interval_ticks: u64,
    zones: &mut ZoneRegistry,
    qi_ledger: &mut WorldQiAccount,
) -> Option<QiTransfer> {
    let minutes = interval_ticks as f64 / 20.0 / 60.0;
    let requested = QI_TIANDAO_WATCH_ZONE_DRAIN_PER_MINUTE * minutes;
    if requested <= 0.0 {
        return None;
    }
    let zone = zones.find_zone_mut(zone_name)?;
    let amount = zone.spirit_qi.max(0.0).min(requested);
    if amount <= 0.0 {
        return None;
    }
    let transfer = QiTransfer::new(
        QiAccountId::zone(zone_name),
        QiAccountId::tiandao(),
        amount,
        QiTransferReason::TiandaoWatchDrain,
    )
    .ok()?;
    let source_balance = qi_ledger.balance(&transfer.from);
    qi_ledger
        .set_balance(transfer.from.clone(), source_balance + amount)
        .ok()?;
    qi_ledger.transfer(transfer.clone()).ok()?;
    zone.spirit_qi = (zone.spirit_qi - amount).max(0.0);
    Some(transfer)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TiandaoResponseProfile {
    pub vignette_rgb: u32,
    pub vignette_alpha: f64,
    pub shake_intensity: f64,
    pub saturation: f64,
    pub audio_recipe_id: &'static str,
    pub vfx_event_id: Option<&'static str>,
    pub vfx_color: &'static str,
    pub event_name: Option<&'static str>,
    pub interval_ticks: u64,
    pub duration_ticks: u64,
    pub intensity: f64,
}

pub fn tiandao_response_profile(response: TiandaoResponseLevel) -> Option<TiandaoResponseProfile> {
    match response {
        TiandaoResponseLevel::None => None,
        TiandaoResponseLevel::Watch => Some(TiandaoResponseProfile {
            vignette_rgb: 0x400800,
            vignette_alpha: 0.03,
            shake_intensity: 0.0,
            saturation: 1.0,
            audio_recipe_id: "tiandao_watch_ambient",
            vfx_event_id: None,
            vfx_color: "#400800",
            event_name: None,
            interval_ticks: 5 * 60 * 20,
            duration_ticks: 60 * 20,
            intensity: 0.15,
        }),
        TiandaoResponseLevel::Pressure => Some(TiandaoResponseProfile {
            vignette_rgb: 0x400800,
            vignette_alpha: 0.08,
            shake_intensity: 0.35,
            saturation: 0.95,
            audio_recipe_id: "tiandao_pressure_ambient",
            vfx_event_id: Some("bong:tiandao_beast_spawn"),
            vfx_color: "#604020",
            event_name: Some(EVENT_BEAST_TIDE),
            interval_ticks: TIANDAO_PRESSURE_EVENT_INTERVAL_TICKS,
            duration_ticks: 60 * 20,
            intensity: 0.35,
        }),
        TiandaoResponseLevel::Tribulation => Some(TiandaoResponseProfile {
            vignette_rgb: 0x601000,
            vignette_alpha: 0.15,
            shake_intensity: 0.65,
            saturation: 0.85,
            audio_recipe_id: "tiandao_tribulation_ambient",
            vfx_event_id: Some("bong:tiandao_directed_thunder"),
            vfx_color: "#E0E8FF",
            event_name: Some(EVENT_THUNDER_TRIBULATION),
            interval_ticks: TIANDAO_TRIBULATION_EVENT_INTERVAL_TICKS,
            duration_ticks: 60 * 20,
            intensity: 0.75,
        }),
        TiandaoResponseLevel::Annihilate => Some(TiandaoResponseProfile {
            vignette_rgb: 0x801000,
            vignette_alpha: 0.25,
            shake_intensity: 1.0,
            saturation: 0.7,
            audio_recipe_id: "tiandao_annihilate_ambient",
            vfx_event_id: Some("bong:realm_collapse_boundary"),
            vfx_color: "#601000",
            event_name: Some(EVENT_REALM_COLLAPSE),
            interval_ticks: TIANDAO_ANNIHILATE_EVENT_INTERVAL_TICKS,
            duration_ticks: 30 * 20,
            intensity: 1.0,
        }),
    }
}

fn tiandao_audio_request(
    eval: &TiandaoEvalSnapshot,
    profile: TiandaoResponseProfile,
    player_entity: Entity,
) -> PlaySoundRecipeRequest {
    let pos = eval.position;
    PlaySoundRecipeRequest {
        recipe_id: profile.audio_recipe_id.to_string(),
        instance_id: TIANDAO_AUDIO_INSTANCE_BASE + u64::from(eval.response.rank()),
        pos: Some([
            pos.x.floor() as i32,
            pos.y.floor() as i32,
            pos.z.floor() as i32,
        ]),
        flag: Some(format!("tiandao:{}", eval.response.as_wire())),
        volume_mul: 1.0,
        pitch_shift: 0.0,
        recipient: AudioRecipient::Single(player_entity),
    }
}

fn tiandao_vfx_request(
    eval: &TiandaoEvalSnapshot,
    profile: TiandaoResponseProfile,
) -> Option<VfxEventRequest> {
    let event_id = profile.vfx_event_id?;
    Some(VfxEventRequest::new(
        eval.position,
        VfxEventPayloadV1::SpawnParticle {
            event_id: event_id.to_string(),
            origin: [eval.position.x, eval.position.y, eval.position.z],
            direction: Some([6.0, 1.0, 6.0]),
            color: Some(profile.vfx_color.to_string()),
            strength: Some(profile.intensity.clamp(0.0, 1.0) as f32),
            count: Some(match eval.response {
                TiandaoResponseLevel::Watch => 1,
                TiandaoResponseLevel::Pressure => 6,
                TiandaoResponseLevel::Tribulation => 18,
                TiandaoResponseLevel::Annihilate => 32,
                TiandaoResponseLevel::None => 1,
            }),
            duration_ticks: Some(match eval.response {
                TiandaoResponseLevel::Watch => 80,
                TiandaoResponseLevel::Pressure => 100,
                TiandaoResponseLevel::Tribulation => 80,
                TiandaoResponseLevel::Annihilate => 160,
                TiandaoResponseLevel::None => 20,
            }),
        },
    ))
}

fn tiandao_spawn_event_command(
    zone_name: &str,
    event_name: &str,
    duration_ticks: u64,
    intensity: f64,
    target_player: &str,
    response: TiandaoResponseLevel,
) -> Command {
    Command {
        command_type: CommandType::SpawnEvent,
        target: zone_name.to_string(),
        params: HashMap::from([
            ("event".to_string(), json!(event_name)),
            ("duration_ticks".to_string(), json!(duration_ticks)),
            ("intensity".to_string(), json!(intensity.clamp(0.0, 1.0))),
            ("target_player".to_string(), json!(target_player)),
            ("attention_level".to_string(), json!(response.as_wire())),
            ("reason".to_string(), json!("tiandao_hunt_p1")),
        ]),
    }
}

fn tiandao_narration_request(
    eval: &TiandaoEvalSnapshot,
    username: &str,
    narration_count: u32,
) -> Option<TiandaoHuntNarrationRequestV1> {
    let response_level = TiandaoHuntResponseLevelV1::try_from(eval.response).ok()?;
    Some(TiandaoHuntNarrationRequestV1::new(
        canonical_player_id(username),
        realm_to_string(eval.realm),
        eval.level,
        response_level,
        eval.zone_name.as_deref().unwrap_or("unknown"),
        tiandao_recent_actions(eval),
        narration_count,
    ))
}

fn tiandao_recent_actions(eval: &TiandaoEvalSnapshot) -> Vec<String> {
    let mut actions = vec![
        format!("activity:{}", eval.activity.as_wire()),
        format!("zone_qi:{:.3}", eval.zone_spirit_qi),
    ];
    if eval.countermeasure.deceive_heaven != DeceiveHeavenOutcome::None {
        actions.push(format!(
            "countermeasure:deceive_heaven_{}",
            eval.countermeasure.deceive_heaven.as_wire()
        ));
    }
    actions
}

fn emit_tiandao_presence_payload(
    client: &mut Client,
    realm: Realm,
    attention: &TiandaoAttention,
    zone_name: &str,
    zone_spirit_qi: f64,
    now_tick: u64,
) {
    let profile = (realm_rank(realm) >= TIANDAO_PRESENCE_SPIRIT_REALM_MIN_RANK)
        .then(|| tiandao_response_profile(attention.response))
        .flatten();
    let payload = json!({
        "v": 1,
        "type": "tiandao_presence",
        "level": attention.level,
        "response": attention.response.as_wire(),
        "zone": zone_name,
        "zone_spirit_qi": zone_spirit_qi,
        "vignette_rgb": profile.map(|profile| profile.vignette_rgb).unwrap_or(0),
        "vignette_alpha": profile.map(|profile| profile.vignette_alpha).unwrap_or(0.0),
        "shake_intensity": profile.map(|profile| profile.shake_intensity).unwrap_or(0.0),
        "saturation": profile.map(|profile| profile.saturation).unwrap_or(1.0),
        "tick": now_tick,
    });
    match serde_json::to_vec(&payload) {
        Ok(bytes) => {
            client.send_custom_payload(ident!("bong:tiandao_presence"), &bytes);
        }
        Err(error) => {
            tracing::warn!("[bong][tiandao_hunt] failed to serialize presence payload: {error}");
        }
    }
}

pub const fn realm_rank(realm: Realm) -> u8 {
    match realm {
        Realm::Awaken => 0,
        Realm::Induce => 1,
        Realm::Condense => 2,
        Realm::Solidify => 3,
        Realm::Spirit => 4,
        Realm::Void => 5,
    }
}

impl TiandaoResponseLevel {
    pub const fn as_wire(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Watch => "watch",
            Self::Pressure => "pressure",
            Self::Tribulation => "tribulation",
            Self::Annihilate => "annihilate",
        }
    }

    pub const fn rank(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Watch => 1,
            Self::Pressure => 2,
            Self::Tribulation => 3,
            Self::Annihilate => 4,
        }
    }
}

impl TryFrom<TiandaoResponseLevel> for TiandaoHuntResponseLevelV1 {
    type Error = ();

    fn try_from(value: TiandaoResponseLevel) -> Result<Self, Self::Error> {
        match value {
            TiandaoResponseLevel::None => Err(()),
            TiandaoResponseLevel::Watch => Ok(Self::Watch),
            TiandaoResponseLevel::Pressure => Ok(Self::Pressure),
            TiandaoResponseLevel::Tribulation => Ok(Self::Tribulation),
            TiandaoResponseLevel::Annihilate => Ok(Self::Annihilate),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::components::Wounds;
    use crate::combat::events::{ApplyStatusEffectIntent, CombatEvent, DeathEvent};
    use crate::combat::CombatClock;
    use crate::cultivation::color::PracticeLog;
    use crate::cultivation::components::{
        Contamination, Cultivation, MeridianSystem, QiColor, Realm,
    };
    use crate::cultivation::negative_zone::siphon_amount;
    use crate::cultivation::tribulation::JueBiTriggerEvent;
    use crate::inventory::{
        ContainerState, InventoryRevision, ItemInstance, ItemRarity, PlayerInventory,
        EQUIP_SLOT_MAIN_HAND, MAIN_PACK_CONTAINER_ID,
    };
    use crate::player::gameplay::PendingGameplayNarrations;
    use crate::social::components::SpiritNiche;
    use crate::world::dimension::{CurrentDimension, DimensionKind};
    use crate::world::season::Season;
    use crate::world::zone::Zone;
    use crate::zhenfa::{ZhenfaCarrierKind, ZhenfaKind, ZhenfaPlaceRequest, ZhenfaRegistry};
    use valence::prelude::{ChunkLayer, DVec3, UnloadedChunk};
    use valence::testing::{create_mock_client, ScenarioSingleClient};

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "expected {expected}, got {actual}"
        );
    }

    fn input(realm: Realm) -> TiandaoAttentionInput {
        TiandaoAttentionInput {
            realm,
            zone_spirit_qi: 0.6,
            activity: TiandaoActivity::Standing,
            season: Season::Summer,
        }
    }

    fn sync_deceive_heaven_runtime(
        state: &mut DeceiveHeavenRuntimeState,
        deploy_event: Option<DeceiveHeavenEvent>,
        exposed_event: Option<DeceiveHeavenExposedEvent>,
    ) {
        let mut deploy_events = Events::default();
        let mut exposed_events = Events::default();
        let mut deploy_reader = bevy_ecs::event::ManualEventReader::default();
        let mut exposed_reader = bevy_ecs::event::ManualEventReader::default();

        if let Some(event) = deploy_event {
            deploy_events.send(event);
        }
        if let Some(event) = exposed_event {
            exposed_events.send(event);
        }

        state.record_events(
            Some(&deploy_events),
            Some(&exposed_events),
            &mut deploy_reader,
            &mut exposed_reader,
        );
    }

    fn deceive_heaven_deploy_event(
        array_id: u64,
        owner_player_id: &str,
        pos: [i32; 3],
        placed_at_tick: u64,
    ) -> DeceiveHeavenEvent {
        DeceiveHeavenEvent {
            owner: Entity::from_raw(1),
            owner_player_id: owner_player_id.to_string(),
            array_id,
            pos,
            self_weight_multiplier: 0.5,
            target_weight_multiplier: 1.5,
            reveal_chance: 0.10,
            placed_at_tick,
        }
    }

    fn deceive_heaven_exposed_event(
        array_id: u64,
        owner_player_id: &str,
        pos: [i32; 3],
        exposed_at_tick: u64,
    ) -> DeceiveHeavenExposedEvent {
        DeceiveHeavenExposedEvent {
            owner: Entity::from_raw(1),
            owner_player_id: owner_player_id.to_string(),
            array_id,
            pos,
            self_weight_multiplier: 0.5,
            target_weight_multiplier: 1.5,
            reveal_chance: 0.10,
            exposed_at_tick,
        }
    }

    fn negative_zone_escape_qi_cost_per_eval_for_tests(zone_spirit_qi: f64, qi_max: f64) -> f64 {
        siphon_amount(zone_spirit_qi, qi_max) * TIANDAO_HUNT_EVAL_INTERVAL_TICKS as f64
    }

    fn tiandao_runtime_app() -> (App, Entity) {
        let mut app = App::new();
        app.add_event::<DeceiveHeavenEvent>();
        app.add_event::<DeceiveHeavenExposedEvent>();
        app.insert_resource(CultivationClock { tick: 0 });
        app.init_resource::<DeceiveHeavenRuntimeState>();
        app.init_resource::<TiandaoActivityRuntimeState>();
        app.add_systems(Update, tiandao_hunt_tick);

        let (mut bundle, _helper) = create_mock_client("Alice");
        bundle.player.position = Position::new([0.0, 64.0, 0.0]);
        let player = app
            .world_mut()
            .spawn((
                bundle,
                Cultivation {
                    realm: Realm::Awaken,
                    qi_current: 100.0,
                    qi_max: 100.0,
                    ..Cultivation::default()
                },
                TiandaoAttention {
                    level: 20.0,
                    response: TiandaoResponseLevel::Watch,
                    ..TiandaoAttention::default()
                },
            ))
            .id();
        (app, player)
    }

    fn tiandao_zhenfa_production_app() -> (App, Entity) {
        let scenario = ScenarioSingleClient::new();
        let mut app = scenario.app;
        crate::world::dimension::mark_test_layer_as_overworld(&mut app);
        app.world_mut()
            .get_mut::<ChunkLayer>(scenario.layer)
            .expect("test layer should carry ChunkLayer")
            .insert_chunk([37, 0], UnloadedChunk::new());
        app.insert_resource(CultivationClock { tick: 0 });
        app.insert_resource(CombatClock::default());
        app.insert_resource(PendingGameplayNarrations::default());
        app.add_event::<JueBiTriggerEvent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<ApplyStatusEffectIntent>();
        crate::zhenfa::register(&mut app);
        super::register(&mut app);

        let (mut bundle, _helper) = create_mock_client("Alice");
        bundle.player.position = Position::new([0.0, 64.0, 0.0]);
        let player = app
            .world_mut()
            .spawn((
                bundle,
                Cultivation {
                    realm: Realm::Solidify,
                    qi_current: 100.0,
                    qi_max: 100.0,
                    ..Cultivation::default()
                },
                QiColor::default(),
                PracticeLog::default(),
                Wounds::default(),
                Contamination::default(),
                MeridianSystem::default(),
                TiandaoAttention {
                    level: 20.0,
                    response: TiandaoResponseLevel::Watch,
                    ..TiandaoAttention::default()
                },
                deceive_heaven_test_inventory(),
            ))
            .id();
        (app, player)
    }

    fn deceive_heaven_test_inventory() -> PlayerInventory {
        const ZHENFA_FLAG_ITEM_ID_FOR_TEST: &str = "array_flag";
        let mut inventory = PlayerInventory {
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                id: MAIN_PACK_CONTAINER_ID.to_string(),
                name: "main".to_string(),
                rows: 4,
                cols: 6,
                items: Vec::new(),
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 10,
            max_weight: 45.0,
        };
        inventory.equipped.insert(
            EQUIP_SLOT_MAIN_HAND.to_string(),
            test_item(9200, ZHENFA_FLAG_ITEM_ID_FOR_TEST, 1),
        );
        inventory.containers[0]
            .items
            .push(crate::inventory::PlacedItemState {
                row: 0,
                col: 0,
                instance: test_item(9201, "ling_mu_ban", 2),
            });
        inventory.containers[0]
            .items
            .push(crate::inventory::PlacedItemState {
                row: 0,
                col: 1,
                instance: test_item(9202, "yi_shou_gu", 4),
            });
        inventory
    }

    fn capture_production_deceive_heaven_exposure(
        app: &mut App,
        player: Entity,
    ) -> DeceiveHeavenExposedEvent {
        const MAX_EXPOSURE_CANDIDATES: i32 = 200;

        let mut requested_at_tick = 200;
        for offset in 0..MAX_EXPOSURE_CANDIDATES {
            let pos = [600 + offset, 64, 0];
            app.world_mut().resource_mut::<CultivationClock>().tick = requested_at_tick;
            app.world_mut().resource_mut::<CombatClock>().tick = requested_at_tick;
            app.world_mut()
                .entity_mut(player)
                .insert(deceive_heaven_test_inventory());
            app.world_mut()
                .get_mut::<Cultivation>(player)
                .unwrap()
                .qi_current = 100.0;
            app.world_mut().send_event(ZhenfaPlaceRequest {
                player,
                pos,
                kind: ZhenfaKind::DeceiveHeaven,
                carrier: ZhenfaCarrierKind::BeastCoreInlaid,
                qi_invest_ratio: 0.10,
                trigger: None,
                item_instance_id: None,
                target_face: None,
                requested_at_tick,
            });
            app.update();

            let instance = app
                .world()
                .resource::<ZhenfaRegistry>()
                .find_at(pos)
                .expect("production placement should create a deceive heaven instance")
                .clone();
            let probe_tick = instance.expires_at_tick.saturating_sub(1);
            {
                let mut attention = app.world_mut().get_mut::<TiandaoAttention>(player).unwrap();
                attention.level = 21.0;
                attention.response = TiandaoResponseLevel::Watch;
                attention.last_eval_tick = instance.placed_at_tick;
            }
            app.world_mut().resource_mut::<CultivationClock>().tick = probe_tick;
            app.world_mut().resource_mut::<CombatClock>().tick = probe_tick;
            app.update();

            let exposed = app
                .world()
                .resource::<Events<DeceiveHeavenExposedEvent>>()
                .iter_current_update_events()
                .find(|event| event.array_id == instance.id)
                .cloned();
            if let Some(event) = exposed {
                assert_eq!(event.owner_player_id, "offline:Alice");
                assert_eq!(event.pos, pos);
                assert_eq!(event.exposed_at_tick, probe_tick);
                return event;
            }

            requested_at_tick = instance.expires_at_tick.saturating_add(1);
        }

        panic!("production zhenfa path did not emit a deceive heaven exposure event");
    }

    fn test_item(instance_id: u64, template_id: &str, stack_count: u32) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: template_id.to_string(),
            display_name: template_id.to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.1,
            rarity: ItemRarity::Common,
            description: template_id.to_string(),
            stack_count,
            spirit_quality: 1.0,
            durability: 1.0,
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

    fn single_zone_registry(name: &str, spirit_qi: f64) -> ZoneRegistry {
        ZoneRegistry {
            zones: vec![Zone {
                name: name.to_string(),
                dimension: DimensionKind::Overworld,
                bounds: (DVec3::new(-16.0, 0.0, -16.0), DVec3::new(16.0, 128.0, 16.0)),
                spirit_qi,
                danger_level: 0,
                active_events: Vec::new(),
                patrol_anchors: Vec::new(),
                blocked_tiles: Vec::new(),
            }],
        }
    }

    fn evaluate_deceive_runtime(
        state: &mut DeceiveHeavenRuntimeState,
        player_pos: DVec3,
        now_tick: u64,
        start_level: f64,
    ) -> (TiandaoEvalSnapshot, TiandaoAttention) {
        let cultivation = Cultivation {
            realm: Realm::Awaken,
            ..Cultivation::default()
        };
        let position = Position(player_pos);
        let mut attention = TiandaoAttention {
            level: start_level,
            response: TiandaoResponseLevel::Watch,
            ..TiandaoAttention::default()
        };
        let countermeasures = state.countermeasure_input("offline:Alice", position.0, now_tick);
        let snapshot = apply_attention_eval(
            &cultivation,
            &position,
            &mut attention,
            TiandaoEvalContext {
                dimension: None,
                zones: None,
                season: Season::Summer,
                activity: TiandaoActivity::Standing,
                countermeasures,
                now_tick,
            },
        )
        .expect("ten-second tiandao_hunt eval should run");
        state.mark_countermeasure_applied(
            "offline:Alice",
            position.0,
            now_tick,
            snapshot.countermeasure,
        );

        (snapshot, attention)
    }

    fn narration_test_snapshot(response: TiandaoResponseLevel, level: f64) -> TiandaoEvalSnapshot {
        TiandaoEvalSnapshot {
            position: DVec3::new(1.2, 64.0, -3.4),
            zone_name: Some("spawn".to_string()),
            zone_spirit_qi: 0.6,
            realm: Realm::Spirit,
            activity: TiandaoActivity::Meditating,
            response,
            level,
            countermeasure: TiandaoCountermeasureOutcome::default(),
        }
    }

    fn narration_test_bridge() -> (
        RedisBridgeResource,
        crossbeam_channel::Receiver<RedisOutbound>,
    ) {
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
        (
            RedisBridgeResource {
                tx_outbound,
                rx_inbound,
            },
            rx_outbound,
        )
    }

    fn expect_tiandao_narration_request(
        rx: &crossbeam_channel::Receiver<RedisOutbound>,
    ) -> TiandaoHuntNarrationRequestV1 {
        match rx
            .try_recv()
            .expect("expected one tiandao narration outbound")
        {
            RedisOutbound::TiandaoHuntNarrationRequest(payload) => payload,
            other => panic!("expected TiandaoHuntNarrationRequest, got {other:?}"),
        }
    }

    fn narration_test_sinks(redis: &RedisBridgeResource) -> TiandaoResponseSinks<'_> {
        TiandaoResponseSinks {
            zones: None,
            active_events: None,
            vfx_events: None,
            audio_events: None,
            redis: Some(redis),
            qi_ledger: None,
        }
    }

    fn advance_for_minutes(
        attention: &mut TiandaoAttention,
        input: TiandaoAttentionInput,
        minutes: u64,
        eval_index: &mut u64,
    ) {
        for _ in 0..(minutes * 60 * 20 / TIANDAO_HUNT_EVAL_INTERVAL_TICKS) {
            *eval_index += 1;
            advance_attention(
                attention,
                input,
                *eval_index * TIANDAO_HUNT_EVAL_INTERVAL_TICKS,
            );
        }
    }

    fn emit_response_chain_for_test(
        attention: &mut TiandaoAttention,
        response: TiandaoResponseLevel,
        level: f64,
        now_tick: u64,
    ) -> (
        ActiveEventsResource,
        ZoneRegistry,
        Events<VfxEventRequest>,
        Events<PlaySoundRecipeRequest>,
        crossbeam_channel::Receiver<RedisOutbound>,
    ) {
        let (redis, rx) = narration_test_bridge();
        let mut active_events = ActiveEventsResource::default();
        let mut zones = single_zone_registry("spawn", 0.6);
        let mut vfx_events = Events::<VfxEventRequest>::default();
        let mut audio_events = Events::<PlaySoundRecipeRequest>::default();

        apply_tiandao_response_chain(
            attention,
            narration_test_snapshot(response, level),
            Entity::from_raw(42),
            "Alice",
            TiandaoResponseSinks {
                zones: Some(&mut zones),
                active_events: Some(&mut active_events),
                vfx_events: Some(&mut vfx_events),
                audio_events: Some(&mut audio_events),
                redis: Some(&redis),
                qi_ledger: None,
            },
            now_tick,
        );

        (active_events, zones, vfx_events, audio_events, rx)
    }

    fn qi_snapshot(zone_qi: f64, ledger: &WorldQiAccount) -> crate::qi_physics::WorldQiSnapshot {
        crate::qi_physics::WorldQiSnapshot {
            player_qi: 0.0,
            zone_qi,
            container_qi: 0.0,
            ledger_qi: ledger.total(),
            era_decay_accum: 0.0,
            budget_initial_total: crate::qi_physics::constants::DEFAULT_SPIRIT_QI_TOTAL,
            budget_current_total: crate::qi_physics::constants::DEFAULT_SPIRIT_QI_TOTAL,
        }
    }

    #[test]
    fn realm_base_rates_match_plan_scale() {
        assert_eq!(realm_base_rate(Realm::Awaken), 0.0);
        assert_eq!(realm_base_rate(Realm::Induce), 0.0);
        assert_eq!(realm_base_rate(Realm::Condense), 0.01);
        assert_eq!(realm_base_rate(Realm::Solidify), 0.05);
        assert_eq!(realm_base_rate(Realm::Spirit), 0.15);
        assert_eq!(realm_base_rate(Realm::Void), 0.40);
    }

    #[test]
    fn runtime_activity_tracks_movement_since_last_eval() {
        let entity = Entity::from_raw(7);
        let mut state = TiandaoActivityRuntimeState::default();

        let first = state.activity_for_eval(TiandaoActivityRuntimeInput {
            entity,
            position: DVec3::new(0.0, 64.0, 0.0),
            combat: None,
            lifecycle: None,
            practice_accumulator: None,
            spirit_niches: None,
            now_tick: 200,
        });
        let second = state.activity_for_eval(TiandaoActivityRuntimeInput {
            entity,
            position: DVec3::new(1.0, 64.0, 0.0),
            combat: None,
            lifecycle: None,
            practice_accumulator: None,
            spirit_niches: None,
            now_tick: 400,
        });

        assert_eq!(first, TiandaoActivity::Standing);
        assert_eq!(second, TiandaoActivity::Moving);
    }

    #[test]
    fn runtime_activity_uses_combat_before_meditation_or_movement() {
        let entity = Entity::from_raw(8);
        let mut state = TiandaoActivityRuntimeState::default();
        let mut practice = CultivationSessionPracticeAccumulator::default();
        practice.note_practice_tick_for_tests(entity, 198);
        let combat = CombatState {
            in_combat_until_tick: Some(300),
            ..CombatState::default()
        };

        let activity = state.activity_for_eval(TiandaoActivityRuntimeInput {
            entity,
            position: DVec3::new(4.0, 64.0, 0.0),
            combat: Some(&combat),
            lifecycle: None,
            practice_accumulator: Some(&practice),
            spirit_niches: None,
            now_tick: 200,
        });

        assert_eq!(activity, TiandaoActivity::Combat);
    }

    #[test]
    fn runtime_activity_uses_own_active_niche_before_meditation() {
        let entity = Entity::from_raw(9);
        let mut state = TiandaoActivityRuntimeState::default();
        let mut practice = CultivationSessionPracticeAccumulator::default();
        practice.note_practice_tick_for_tests(entity, 198);
        let lifecycle = Lifecycle {
            character_id: "offline:Alice".to_string(),
            ..Lifecycle::default()
        };
        let mut spirit_niches = SpiritNicheRegistry::default();
        spirit_niches.upsert(SpiritNiche {
            owner: "offline:Alice".to_string(),
            pos: [10, 64, 10],
            placed_at_tick: 1,
            revealed: false,
            revealed_by: None,
            is_damaged: false,
            guardians: Vec::new(),
        });

        let activity = state.activity_for_eval(TiandaoActivityRuntimeInput {
            entity,
            position: DVec3::new(10.5, 64.5, 10.5),
            combat: None,
            lifecycle: Some(&lifecycle),
            practice_accumulator: Some(&practice),
            spirit_niches: Some(&spirit_niches),
            now_tick: 200,
        });

        assert_eq!(activity, TiandaoActivity::InNiche);
    }

    #[test]
    fn low_realms_never_accumulate_attention() {
        for realm in [Realm::Awaken, Realm::Induce] {
            let mut attention = TiandaoAttention {
                level: 10.0,
                ..TiandaoAttention::default()
            };
            let mut eval_index = 0;
            advance_for_minutes(
                &mut attention,
                TiandaoAttentionInput {
                    zone_spirit_qi: 0.9,
                    activity: TiandaoActivity::Meditating,
                    ..input(realm)
                },
                24 * 60,
                &mut eval_index,
            );
            assert_eq!(attention.level, 0.0);
            assert_eq!(attention.accumulation_rate, 0.0);
            assert_eq!(attention.response, TiandaoResponseLevel::None);
        }
    }

    #[test]
    fn zone_qi_factor_has_all_plan_thresholds() {
        assert_eq!(zone_qi_factor(-0.5), 0.3);
        assert_eq!(zone_qi_factor(0.1), 0.3);
        assert_eq!(zone_qi_factor(0.1001), 0.6);
        assert_eq!(zone_qi_factor(0.3), 0.6);
        assert_eq!(zone_qi_factor(0.3001), 1.0);
        assert_eq!(zone_qi_factor(0.6), 1.0);
        assert_eq!(zone_qi_factor(0.6001), 1.8);
    }

    #[test]
    fn activity_factor_matches_plan_actions() {
        assert_eq!(activity_factor(TiandaoActivity::Meditating), 1.5);
        assert_eq!(activity_factor(TiandaoActivity::Combat), 1.2);
        assert_eq!(activity_factor(TiandaoActivity::Moving), 0.8);
        assert_eq!(activity_factor(TiandaoActivity::Standing), 1.0);
        assert_eq!(activity_factor(TiandaoActivity::InNiche), 0.5);
    }

    #[test]
    fn xizhuan_season_increases_accumulation() {
        let normal = accumulation_rate(TiandaoAttentionInput {
            season: Season::Summer,
            ..input(Realm::Void)
        });
        let xizhuan = accumulation_rate(TiandaoAttentionInput {
            season: Season::SummerToWinter,
            ..input(Realm::Void)
        });
        assert!((xizhuan - normal * 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn response_upgrade_thresholds_are_inclusive() {
        assert_eq!(
            response_for_level(TiandaoResponseLevel::None, 15.0),
            TiandaoResponseLevel::Watch
        );
        assert_eq!(
            response_for_level(TiandaoResponseLevel::Watch, 40.0),
            TiandaoResponseLevel::Pressure
        );
        assert_eq!(
            response_for_level(TiandaoResponseLevel::Pressure, 70.0),
            TiandaoResponseLevel::Tribulation
        );
        assert_eq!(
            response_for_level(TiandaoResponseLevel::Tribulation, 90.0),
            TiandaoResponseLevel::Annihilate
        );
    }

    #[test]
    fn response_downgrade_uses_hysteresis() {
        assert_eq!(
            response_for_level(TiandaoResponseLevel::Watch, 10.0),
            TiandaoResponseLevel::Watch
        );
        assert_eq!(
            response_for_level(TiandaoResponseLevel::Watch, 9.99),
            TiandaoResponseLevel::None
        );
        assert_eq!(
            response_for_level(TiandaoResponseLevel::Pressure, 30.0),
            TiandaoResponseLevel::Pressure
        );
        assert_eq!(
            response_for_level(TiandaoResponseLevel::Pressure, 29.99),
            TiandaoResponseLevel::Watch
        );
        assert_eq!(
            response_for_level(TiandaoResponseLevel::Tribulation, 60.0),
            TiandaoResponseLevel::Tribulation
        );
        assert_eq!(
            response_for_level(TiandaoResponseLevel::Tribulation, 59.99),
            TiandaoResponseLevel::Pressure
        );
        assert_eq!(
            response_for_level(TiandaoResponseLevel::Annihilate, 80.0),
            TiandaoResponseLevel::Annihilate
        );
        assert_eq!(
            response_for_level(TiandaoResponseLevel::Annihilate, 79.99),
            TiandaoResponseLevel::Tribulation
        );
    }

    #[test]
    fn advance_attention_clamps_and_tracks_peak() {
        let mut attention = TiandaoAttention {
            level: 99.9,
            response: TiandaoResponseLevel::Annihilate,
            last_eval_tick: u64::MAX,
            accumulation_rate: 0.0,
            peak_level: 50.0,
            ..TiandaoAttention::default()
        };
        advance_attention(
            &mut attention,
            TiandaoAttentionInput {
                realm: Realm::Void,
                zone_spirit_qi: 1.0,
                activity: TiandaoActivity::Meditating,
                season: Season::SummerToWinter,
            },
            u64::MAX,
        );
        assert_eq!(attention.level, 100.0);
        assert_eq!(attention.peak_level, 100.0);
        assert_eq!(attention.last_eval_tick, u64::MAX);
        assert_eq!(attention.response, TiandaoResponseLevel::Annihilate);
    }

    #[test]
    fn decay_never_underflows_attention() {
        let mut attention = TiandaoAttention {
            level: 0.01,
            response: TiandaoResponseLevel::None,
            ..TiandaoAttention::default()
        };
        advance_attention(
            &mut attention,
            TiandaoAttentionInput {
                realm: Realm::Awaken,
                zone_spirit_qi: -0.5,
                activity: TiandaoActivity::Standing,
                season: Season::Summer,
            },
            200,
        );
        assert_eq!(attention.level, 0.0);
    }

    #[test]
    fn dead_and_negative_zones_accelerate_decay_without_qi_transfer() {
        assert_eq!(zone_decay_multiplier(0.0), 3.0);
        assert_eq!(zone_decay_multiplier(-0.01), 5.0);
        assert_eq!(zone_decay_multiplier(0.01), 1.0);
        assert_close(decay_rate(TiandaoResponseLevel::Watch, 0.0), 0.15);
        assert_close(decay_rate(TiandaoResponseLevel::Watch, -0.1), 0.25);
    }

    #[test]
    fn none_stage_allows_attention_to_build_before_watch() {
        let input = TiandaoAttentionInput {
            realm: Realm::Condense,
            zone_spirit_qi: 0.9,
            activity: TiandaoActivity::Meditating,
            season: Season::Summer,
        };
        let mut attention = TiandaoAttention::default();
        advance_attention(&mut attention, input, 200);

        assert!(
            attention.level > 0.0,
            "凝脉以上在高灵气打坐时必须能从 None 建立注意力，否则 P4 曲线永远到不了 Watch"
        );
        assert_eq!(
            attention_decay_for_eval(TiandaoResponseLevel::None, input, accumulation_rate(input)),
            0.0
        );
    }

    #[test]
    fn condense_high_qi_meditation_reaches_watch_then_moving_escapes_to_none() {
        let mut attention = TiandaoAttention::default();
        let mut eval_index = 0;
        let high_qi_meditation = TiandaoAttentionInput {
            realm: Realm::Condense,
            zone_spirit_qi: 0.9,
            activity: TiandaoActivity::Meditating,
            season: Season::Summer,
        };
        advance_for_minutes(&mut attention, high_qi_meditation, 250, &mut eval_index);
        assert_eq!(attention.response, TiandaoResponseLevel::Watch);
        assert!(
            attention.level >= 10.0 && attention.level < 15.0,
            "凝脉高灵气打坐应只擦到 Watch 边缘并受滞后保护，actual={}",
            attention.level
        );

        advance_for_minutes(
            &mut attention,
            TiandaoAttentionInput {
                activity: TiandaoActivity::Moving,
                ..high_qi_meditation
            },
            30,
            &mut eval_index,
        );
        assert_eq!(attention.response, TiandaoResponseLevel::None);
        assert!(
            attention.level < 10.0,
            "凝脉 Watch 边缘跑路 30 分钟应脱离天道注视，actual={}",
            attention.level
        );
    }

    #[test]
    fn solidify_attention_reaches_watch_pressure_then_moving_downgrades() {
        let mut attention = TiandaoAttention::default();
        let mut eval_index = 0;
        let standing = input(Realm::Solidify);
        advance_for_minutes(&mut attention, standing, 50, &mut eval_index);
        assert_eq!(attention.response, TiandaoResponseLevel::Watch);
        assert!(
            attention.level >= 15.0 && attention.level < 40.0,
            "固元 50 分钟应进入 Watch 但未到 Pressure，actual={}",
            attention.level
        );

        let high_qi_meditation = TiandaoAttentionInput {
            realm: Realm::Solidify,
            zone_spirit_qi: 0.9,
            activity: TiandaoActivity::Meditating,
            season: Season::Summer,
        };
        advance_for_minutes(&mut attention, high_qi_meditation, 70, &mut eval_index);
        assert_eq!(attention.response, TiandaoResponseLevel::Pressure);

        let before_move = attention.level;
        advance_for_minutes(
            &mut attention,
            TiandaoAttentionInput {
                activity: TiandaoActivity::Moving,
                ..standing
            },
            15,
            &mut eval_index,
        );
        assert!(
            attention.level < before_move,
            "固元移动应降低 Pressure 注意力，before={before_move} after={}",
            attention.level
        );
        assert!(
            matches!(
                attention.response,
                TiandaoResponseLevel::Pressure | TiandaoResponseLevel::Watch
            ),
            "固元移动后只能保持 Pressure 或降回 Watch，actual={:?}",
            attention.response
        );
    }

    #[test]
    fn spirit_attention_reaches_watch_pressure_tribulation_in_order() {
        let mut attention = TiandaoAttention::default();
        let mut eval_index = 0;
        let standing = input(Realm::Spirit);

        advance_for_minutes(&mut attention, standing, 17, &mut eval_index);
        assert_eq!(attention.response, TiandaoResponseLevel::Watch);

        advance_for_minutes(&mut attention, standing, 73, &mut eval_index);
        assert_eq!(attention.response, TiandaoResponseLevel::Pressure);

        advance_for_minutes(&mut attention, standing, 30, &mut eval_index);
        assert_eq!(attention.response, TiandaoResponseLevel::Tribulation);
        assert!(
            attention.peak_level >= 70.0,
            "通灵曲线必须按 Watch→Pressure→Tribulation 升级，peak={}",
            attention.peak_level
        );
    }

    #[test]
    fn void_attention_reaches_watch_tribulation_annihilate_in_order() {
        let mut attention = TiandaoAttention::default();
        let mut eval_index = 0;
        let standing = input(Realm::Void);

        advance_for_minutes(&mut attention, standing, 7, &mut eval_index);
        assert_eq!(attention.response, TiandaoResponseLevel::Watch);

        advance_for_minutes(&mut attention, standing, 31, &mut eval_index);
        assert_eq!(attention.response, TiandaoResponseLevel::Tribulation);

        advance_for_minutes(&mut attention, standing, 12, &mut eval_index);
        assert_eq!(attention.response, TiandaoResponseLevel::Annihilate);
        assert_eq!(
            decay_rate(TiandaoResponseLevel::Annihilate, standing.zone_spirit_qi),
            0.0,
            "Annihilate 级注意力不自然衰减"
        );
    }

    #[test]
    fn deceive_heaven_decoy_diverts_attention_when_far_active_and_not_revealed() {
        let start_level = 20.0;
        let mut attention = TiandaoAttention {
            level: start_level,
            response: TiandaoResponseLevel::Watch,
            ..TiandaoAttention::default()
        };
        let outcome = advance_attention_with_countermeasures(
            &mut attention,
            TiandaoAttentionInput {
                realm: Realm::Awaken,
                zone_spirit_qi: 0.6,
                activity: TiandaoActivity::Standing,
                season: Season::Summer,
            },
            TiandaoCountermeasureInput {
                deceive_heaven_decoy: Some(DeceiveHeavenDecoyInput {
                    placed_tick: 200,
                    distance_blocks: DECEIVE_HEAVEN_DECOY_MIN_DISTANCE_BLOCKS,
                    exposed: false,
                }),
            },
            400,
        );

        assert_eq!(outcome.deceive_heaven, DeceiveHeavenOutcome::Diverted);
        assert_close(
            outcome.decay_multiplier,
            DECEIVE_HEAVEN_DECOY_DECAY_MULTIPLIER,
        );
        assert_close(
            attention.level,
            start_level - 0.05 * DECEIVE_HEAVEN_DECOY_DECAY_MULTIPLIER,
        );
        assert_eq!(attention.response, TiandaoResponseLevel::Watch);
    }

    #[test]
    fn deceive_heaven_decoy_revealed_adds_twenty_attention_without_decay_bonus() {
        let start_level = 21.0;
        let mut attention = TiandaoAttention {
            level: start_level,
            response: TiandaoResponseLevel::Watch,
            ..TiandaoAttention::default()
        };
        let outcome = advance_attention_with_countermeasures(
            &mut attention,
            TiandaoAttentionInput {
                realm: Realm::Awaken,
                zone_spirit_qi: 0.6,
                activity: TiandaoActivity::Standing,
                season: Season::Summer,
            },
            TiandaoCountermeasureInput {
                deceive_heaven_decoy: Some(DeceiveHeavenDecoyInput {
                    placed_tick: 0,
                    distance_blocks: 800.0,
                    exposed: true,
                }),
            },
            400,
        );

        assert_eq!(outcome.deceive_heaven, DeceiveHeavenOutcome::Revealed);
        assert_close(outcome.attention_penalty, DECEIVE_HEAVEN_REVEAL_PENALTY);
        assert_close(
            attention.level,
            start_level - 0.05 + DECEIVE_HEAVEN_REVEAL_PENALTY,
        );
        assert_eq!(attention.response, TiandaoResponseLevel::Pressure);
    }

    #[test]
    fn deceive_heaven_decoy_has_distance_reveal_and_expiry_boundaries() {
        let active = DeceiveHeavenDecoyInput {
            placed_tick: 100,
            distance_blocks: 500.0,
            exposed: false,
        };
        assert_eq!(
            deceive_heaven_decoy_outcome(active, 100 + DECEIVE_HEAVEN_DECOY_DURATION_TICKS - 1),
            DeceiveHeavenOutcome::Diverted
        );
        assert_eq!(
            deceive_heaven_decoy_outcome(
                DeceiveHeavenDecoyInput {
                    distance_blocks: 499.99,
                    ..active
                },
                200
            ),
            DeceiveHeavenOutcome::TooClose
        );
        assert_eq!(
            deceive_heaven_decoy_outcome(
                DeceiveHeavenDecoyInput {
                    exposed: true,
                    distance_blocks: 499.99,
                    ..active
                },
                200
            ),
            DeceiveHeavenOutcome::Revealed
        );
        assert_eq!(
            deceive_heaven_decoy_outcome(active, 100 + DECEIVE_HEAVEN_DECOY_DURATION_TICKS),
            DeceiveHeavenOutcome::Expired
        );
    }

    #[test]
    fn deceive_heaven_exposure_penalty_wins_over_distance_boundary() {
        let outcome = countermeasure_outcome(
            TiandaoCountermeasureInput {
                deceive_heaven_decoy: Some(DeceiveHeavenDecoyInput {
                    placed_tick: 100,
                    distance_blocks: 1.0,
                    exposed: true,
                }),
            },
            200,
        );

        assert_eq!(outcome.deceive_heaven, DeceiveHeavenOutcome::Revealed);
        assert_eq!(outcome.decay_multiplier, 1.0);
        assert_close(outcome.attention_penalty, DECEIVE_HEAVEN_REVEAL_PENALTY);
    }

    #[test]
    fn deceive_heaven_deploy_event_enters_tiandao_hunt_runtime_and_decays_x4() {
        let mut state = DeceiveHeavenRuntimeState::default();
        sync_deceive_heaven_runtime(
            &mut state,
            Some(deceive_heaven_deploy_event(
                7,
                "offline:Alice",
                [600, 64, 0],
                100,
            )),
            None,
        );

        let (snapshot, attention) =
            evaluate_deceive_runtime(&mut state, DVec3::new(0.0, 64.0, 0.0), 300, 20.0);

        assert_eq!(
            snapshot.countermeasure.deceive_heaven,
            DeceiveHeavenOutcome::Diverted
        );
        assert_close(
            snapshot.countermeasure.decay_multiplier,
            DECEIVE_HEAVEN_DECOY_DECAY_MULTIPLIER,
        );
        assert_close(
            attention.level,
            20.0 - decay_rate(TiandaoResponseLevel::Watch, 0.0)
                * DECEIVE_HEAVEN_DECOY_DECAY_MULTIPLIER,
        );
    }

    #[test]
    fn deceive_heaven_runtime_rejects_too_close_and_expired_deployments() {
        let mut too_close_state = DeceiveHeavenRuntimeState::default();
        sync_deceive_heaven_runtime(
            &mut too_close_state,
            Some(deceive_heaven_deploy_event(
                8,
                "offline:Alice",
                [499, 64, 0],
                100,
            )),
            None,
        );

        let (too_close, too_close_attention) =
            evaluate_deceive_runtime(&mut too_close_state, DVec3::new(0.0, 64.0, 0.0), 300, 20.0);

        assert_eq!(
            too_close.countermeasure.deceive_heaven,
            DeceiveHeavenOutcome::TooClose
        );
        assert_close(
            too_close_attention.level,
            20.0 - decay_rate(TiandaoResponseLevel::Watch, 0.0),
        );

        let mut expired_state = DeceiveHeavenRuntimeState::default();
        sync_deceive_heaven_runtime(
            &mut expired_state,
            Some(deceive_heaven_deploy_event(
                9,
                "offline:Alice",
                [600, 64, 0],
                100,
            )),
            None,
        );

        let (expired, expired_attention) = evaluate_deceive_runtime(
            &mut expired_state,
            DVec3::new(0.0, 64.0, 0.0),
            100 + DECEIVE_HEAVEN_DECOY_DURATION_TICKS,
            20.0,
        );

        assert_eq!(
            expired.countermeasure.deceive_heaven,
            DeceiveHeavenOutcome::Expired
        );
        assert_close(
            expired_attention.level,
            20.0 - decay_rate(TiandaoResponseLevel::Watch, 0.0),
        );
    }

    #[test]
    fn deceive_heaven_exposed_event_adds_tiandao_hunt_attention_penalty() {
        let mut state = DeceiveHeavenRuntimeState::default();
        sync_deceive_heaven_runtime(
            &mut state,
            Some(deceive_heaven_deploy_event(
                10,
                "offline:Alice",
                [600, 64, 0],
                100,
            )),
            None,
        );
        sync_deceive_heaven_runtime(
            &mut state,
            None,
            Some(deceive_heaven_exposed_event(
                10,
                "offline:Alice",
                [600, 64, 0],
                250,
            )),
        );

        let (snapshot, attention) =
            evaluate_deceive_runtime(&mut state, DVec3::new(0.0, 64.0, 0.0), 300, 21.0);

        assert_eq!(
            snapshot.countermeasure.deceive_heaven,
            DeceiveHeavenOutcome::Revealed
        );
        assert_close(
            snapshot.countermeasure.attention_penalty,
            DECEIVE_HEAVEN_REVEAL_PENALTY,
        );
        assert_close(
            attention.level,
            21.0 - decay_rate(TiandaoResponseLevel::Watch, 0.0) + DECEIVE_HEAVEN_REVEAL_PENALTY,
        );
        assert_eq!(attention.response, TiandaoResponseLevel::Pressure);
    }

    #[test]
    fn tiandao_hunt_tick_consumes_deceive_heaven_deploy_event_and_decays_x4() {
        let (mut app, player) = tiandao_runtime_app();
        app.world_mut().send_event(deceive_heaven_deploy_event(
            11,
            "offline:Alice",
            [600, 64, 0],
            200,
        ));
        app.world_mut().resource_mut::<CultivationClock>().tick = 200;

        app.update();

        let attention = app.world().get::<TiandaoAttention>(player).unwrap();
        assert_close(
            attention.level,
            20.0 - decay_rate(TiandaoResponseLevel::Watch, 0.0)
                * DECEIVE_HEAVEN_DECOY_DECAY_MULTIPLIER,
        );
        assert_eq!(attention.last_eval_tick, 200);
    }

    #[test]
    fn production_zhenfa_deceive_heaven_place_feeds_tiandao_hunt_same_update() {
        let (mut app, player) = tiandao_zhenfa_production_app();
        app.world_mut().send_event(ZhenfaPlaceRequest {
            player,
            pos: [600, 64, 0],
            kind: ZhenfaKind::DeceiveHeaven,
            carrier: ZhenfaCarrierKind::BeastCoreInlaid,
            qi_invest_ratio: 0.80,
            trigger: None,
            item_instance_id: None,
            target_face: None,
            requested_at_tick: 200,
        });
        app.world_mut().resource_mut::<CultivationClock>().tick = 200;

        app.update();

        let attention = app.world().get::<TiandaoAttention>(player).unwrap();
        assert_close(
            attention.level,
            20.0 + accumulation_rate(TiandaoAttentionInput {
                realm: Realm::Solidify,
                zone_spirit_qi: 0.0,
                activity: TiandaoActivity::Standing,
                season: Season::default(),
            }) - decay_rate(TiandaoResponseLevel::Watch, 0.0)
                * DECEIVE_HEAVEN_DECOY_DECAY_MULTIPLIER,
        );
        assert_eq!(attention.last_eval_tick, 200);
    }

    #[test]
    fn production_zhenfa_exposure_feeds_tiandao_hunt_penalty_once() {
        let (mut app, player) = tiandao_zhenfa_production_app();
        let exposed = capture_production_deceive_heaven_exposure(&mut app, player);

        let first = app.world().get::<TiandaoAttention>(player).unwrap().clone();
        assert_eq!(first.last_eval_tick, exposed.exposed_at_tick);
        assert_close(
            first.level,
            21.0 + first.accumulation_rate - decay_rate(TiandaoResponseLevel::Watch, 0.0)
                + DECEIVE_HEAVEN_REVEAL_PENALTY,
        );
        assert_eq!(first.response, TiandaoResponseLevel::Pressure);

        let next_eval_tick = first
            .last_eval_tick
            .saturating_add(TIANDAO_HUNT_EVAL_INTERVAL_TICKS);
        app.world_mut().resource_mut::<CultivationClock>().tick = next_eval_tick;
        app.world_mut().resource_mut::<CombatClock>().tick = next_eval_tick;
        app.update();

        let second = app.world().get::<TiandaoAttention>(player).unwrap();
        assert_close(
            second.level,
            first.level + second.accumulation_rate
                - decay_rate(TiandaoResponseLevel::Pressure, 0.0),
        );
        assert_eq!(second.last_eval_tick, next_eval_tick);
    }

    #[test]
    fn tiandao_hunt_tick_applies_deceive_heaven_exposed_penalty_once() {
        let (mut app, player) = tiandao_runtime_app();
        app.world_mut()
            .get_mut::<TiandaoAttention>(player)
            .unwrap()
            .level = 21.0;
        app.world_mut().send_event(deceive_heaven_deploy_event(
            12,
            "offline:Alice",
            [600, 64, 0],
            200,
        ));
        app.world_mut().send_event(deceive_heaven_exposed_event(
            12,
            "offline:Alice",
            [600, 64, 0],
            200,
        ));
        app.world_mut().resource_mut::<CultivationClock>().tick = 200;

        app.update();

        let first = app.world().get::<TiandaoAttention>(player).unwrap().clone();
        assert_close(
            first.level,
            21.0 - decay_rate(TiandaoResponseLevel::Watch, 0.0) + DECEIVE_HEAVEN_REVEAL_PENALTY,
        );
        assert_eq!(first.response, TiandaoResponseLevel::Pressure);

        app.world_mut().resource_mut::<CultivationClock>().tick = 400;
        app.update();

        let second = app.world().get::<TiandaoAttention>(player).unwrap();
        assert_close(
            second.level,
            first.level - decay_rate(TiandaoResponseLevel::Pressure, 0.0),
        );
        assert_eq!(second.last_eval_tick, 400);
    }

    #[test]
    fn negative_zone_escape_applies_decay_x5_and_reports_existing_siphon_cost() {
        let mut attention = TiandaoAttention {
            level: 50.0,
            response: TiandaoResponseLevel::Pressure,
            ..TiandaoAttention::default()
        };
        advance_attention(
            &mut attention,
            TiandaoAttentionInput {
                realm: Realm::Awaken,
                zone_spirit_qi: -0.5,
                activity: TiandaoActivity::Standing,
                season: Season::Summer,
            },
            200,
        );
        assert_close(attention.level, 50.0 - 0.03 * 5.0);
        assert_close(
            negative_zone_escape_qi_cost_per_eval_for_tests(-0.5, 1000.0),
            siphon_amount(-0.5, 1000.0) * TIANDAO_HUNT_EVAL_INTERVAL_TICKS as f64,
        );
        assert!(
            negative_zone_escape_qi_cost_per_eval_for_tests(-0.5, 1000.0)
                > negative_zone_escape_qi_cost_per_eval_for_tests(-0.5, 100.0),
            "负灵域反制代价必须随 qi_max 放大，让高境承担更高真元消耗"
        );
    }

    #[test]
    fn tiandao_hunt_tick_uses_negative_zone_registry_after_ten_second_interval() {
        let (mut app, player) = tiandao_runtime_app();
        app.insert_resource(single_zone_registry("test_negative_field", -0.3));

        app.world_mut().resource_mut::<CultivationClock>().tick = 199;
        app.update();
        let before = app.world().get::<TiandaoAttention>(player).unwrap();
        assert_eq!(before.last_eval_tick, 0);
        assert_close(before.level, 20.0);

        app.world_mut().resource_mut::<CultivationClock>().tick = 200;
        app.update();

        let after = app.world().get::<TiandaoAttention>(player).unwrap();
        assert_eq!(after.last_eval_tick, 200);
        assert_close(
            after.level,
            20.0 - decay_rate(TiandaoResponseLevel::Watch, -0.3),
        );
    }

    #[test]
    fn nomadic_meditation_cycle_does_not_enter_pressure() {
        let mut attention = TiandaoAttention {
            level: 30.0,
            response: TiandaoResponseLevel::Watch,
            peak_level: 30.0,
            ..TiandaoAttention::default()
        };
        let mut eval_index = 1;
        for _cycle in 0..4 {
            for _ in 0..60 {
                advance_attention(
                    &mut attention,
                    TiandaoAttentionInput {
                        activity: TiandaoActivity::Meditating,
                        ..input(Realm::Solidify)
                    },
                    eval_index * TIANDAO_HUNT_EVAL_INTERVAL_TICKS,
                );
                eval_index += 1;
            }
            for _ in 0..30 {
                advance_attention(
                    &mut attention,
                    TiandaoAttentionInput {
                        activity: TiandaoActivity::Moving,
                        ..input(Realm::Solidify)
                    },
                    eval_index * TIANDAO_HUNT_EVAL_INTERVAL_TICKS,
                );
                eval_index += 1;
            }
        }

        assert!(
            attention.level < 40.0,
            "固元 4 轮游牧打坐/转移应停在 Pressure 阈值下，actual={}",
            attention.level
        );
        assert_eq!(attention.response, TiandaoResponseLevel::Watch);
    }

    #[test]
    fn realm_regression_recomputes_accumulation_rate_from_new_realm() {
        let (mut app, player) = tiandao_runtime_app();
        app.insert_resource(single_zone_registry("test_plain_field", 0.6));
        app.world_mut()
            .get_mut::<TiandaoAttention>(player)
            .unwrap()
            .level = 0.0;
        app.world_mut()
            .get_mut::<Cultivation>(player)
            .unwrap()
            .realm = Realm::Spirit;

        app.world_mut().resource_mut::<CultivationClock>().tick = 200;
        app.update();
        let first = app.world().get::<TiandaoAttention>(player).unwrap();
        assert_close(first.accumulation_rate, 0.15);

        app.world_mut()
            .get_mut::<Cultivation>(player)
            .unwrap()
            .realm = Realm::Solidify;
        app.world_mut().resource_mut::<CultivationClock>().tick = 400;
        app.update();
        let second = app.world().get::<TiandaoAttention>(player).unwrap();
        assert_close(second.accumulation_rate, 0.05);
    }

    #[test]
    fn void_high_qi_meditation_reaches_watch_in_plan_timeframe() {
        let mut attention = TiandaoAttention::default();
        let input = TiandaoAttentionInput {
            realm: Realm::Void,
            zone_spirit_qi: 0.9,
            activity: TiandaoActivity::Meditating,
            season: Season::Summer,
        };
        for _ in 0..28 {
            advance_attention(&mut attention, input, 200);
        }
        assert_eq!(attention.response, TiandaoResponseLevel::Watch);
        assert!(attention.level >= 15.0);
    }

    #[test]
    fn evaluation_respects_ten_second_interval() {
        let cultivation = Cultivation {
            realm: Realm::Void,
            ..Cultivation::default()
        };
        let position = Position(DVec3::new(0.0, 65.0, 0.0));
        let dimension = CurrentDimension(DimensionKind::Overworld);
        let zones = ZoneRegistry::fallback();
        let mut attention = TiandaoAttention::default();

        apply_attention_eval(
            &cultivation,
            &position,
            &mut attention,
            TiandaoEvalContext {
                dimension: Some(&dimension),
                zones: Some(&zones),
                season: Season::Summer,
                activity: TiandaoActivity::Standing,
                countermeasures: TiandaoCountermeasureInput::default(),
                now_tick: 199,
            },
        );
        assert_eq!(attention.last_eval_tick, 0);
        assert_eq!(attention.level, 0.0);

        apply_attention_eval(
            &cultivation,
            &position,
            &mut attention,
            TiandaoEvalContext {
                dimension: Some(&dimension),
                zones: Some(&zones),
                season: Season::Summer,
                activity: TiandaoActivity::Standing,
                countermeasures: TiandaoCountermeasureInput::default(),
                now_tick: 200,
            },
        );
        assert_eq!(attention.last_eval_tick, 200);
        assert!(attention.level > 0.0);
    }

    #[test]
    fn response_profile_matches_four_levels() {
        assert!(tiandao_response_profile(TiandaoResponseLevel::None).is_none());

        let watch = tiandao_response_profile(TiandaoResponseLevel::Watch).unwrap();
        assert_eq!(watch.audio_recipe_id, "tiandao_watch_ambient");
        assert_eq!(watch.vfx_event_id, None);
        assert_eq!(watch.event_name, None);

        let pressure = tiandao_response_profile(TiandaoResponseLevel::Pressure).unwrap();
        assert_eq!(pressure.audio_recipe_id, "tiandao_pressure_ambient");
        assert_eq!(pressure.vfx_event_id, Some("bong:tiandao_beast_spawn"));
        assert_eq!(pressure.event_name, Some(EVENT_BEAST_TIDE));

        let tribulation = tiandao_response_profile(TiandaoResponseLevel::Tribulation).unwrap();
        assert_eq!(tribulation.audio_recipe_id, "tiandao_tribulation_ambient");
        assert_eq!(
            tribulation.vfx_event_id,
            Some("bong:tiandao_directed_thunder")
        );
        assert_eq!(tribulation.event_name, Some(EVENT_THUNDER_TRIBULATION));

        let annihilate = tiandao_response_profile(TiandaoResponseLevel::Annihilate).unwrap();
        assert_eq!(annihilate.audio_recipe_id, "tiandao_annihilate_ambient");
        assert_eq!(
            annihilate.vfx_event_id,
            Some("bong:realm_collapse_boundary")
        );
        assert_eq!(annihilate.event_name, Some(EVENT_REALM_COLLAPSE));
    }

    #[test]
    fn response_profiles_pin_plan_intervals() {
        assert_eq!(
            tiandao_response_profile(TiandaoResponseLevel::Watch)
                .unwrap()
                .interval_ticks,
            5 * 60 * 20
        );
        assert_eq!(
            tiandao_response_profile(TiandaoResponseLevel::Pressure)
                .unwrap()
                .interval_ticks,
            TIANDAO_PRESSURE_EVENT_INTERVAL_TICKS
        );
        assert_eq!(
            tiandao_response_profile(TiandaoResponseLevel::Tribulation)
                .unwrap()
                .interval_ticks,
            TIANDAO_TRIBULATION_EVENT_INTERVAL_TICKS
        );
        assert_eq!(
            tiandao_response_profile(TiandaoResponseLevel::Annihilate)
                .unwrap()
                .interval_ticks,
            TIANDAO_ANNIHILATE_EVENT_INTERVAL_TICKS
        );
    }

    #[test]
    fn realm_rank_gates_presence_payload_to_spirit_and_above() {
        assert_eq!(realm_rank(Realm::Awaken), 0);
        assert_eq!(realm_rank(Realm::Induce), 1);
        assert_eq!(realm_rank(Realm::Condense), 2);
        assert_eq!(realm_rank(Realm::Solidify), 3);
        assert_eq!(realm_rank(Realm::Spirit), 4);
        assert_eq!(realm_rank(Realm::Void), 5);
    }

    #[test]
    fn tiandao_spawn_command_uses_target_and_attention_wire_value() {
        let command = tiandao_spawn_event_command(
            "spawn",
            EVENT_THUNDER_TRIBULATION,
            600,
            1.2,
            "player-1",
            TiandaoResponseLevel::Tribulation,
        );
        assert_eq!(command.command_type, CommandType::SpawnEvent);
        assert_eq!(command.target, "spawn");
        assert_eq!(
            command.params.get("event").and_then(|value| value.as_str()),
            Some(EVENT_THUNDER_TRIBULATION)
        );
        assert_eq!(
            command
                .params
                .get("target_player")
                .and_then(|value| value.as_str()),
            Some("player-1")
        );
        assert_eq!(
            command
                .params
                .get("attention_level")
                .and_then(|value| value.as_str()),
            Some("tribulation")
        );
        assert_eq!(
            command
                .params
                .get("reason")
                .and_then(|value| value.as_str()),
            Some("tiandao_hunt_p1")
        );
    }

    #[test]
    fn pressure_response_enqueues_beast_tide_and_emits_multimodal_feedback() {
        let mut attention = TiandaoAttention::default();
        let (active_events, _zones, vfx_events, audio_events, rx) =
            emit_response_chain_for_test(&mut attention, TiandaoResponseLevel::Pressure, 45.0, 200);

        assert!(active_events.contains("spawn", EVENT_BEAST_TIDE));
        assert_eq!(
            active_events.count_by_zone_and_event("spawn", EVENT_BEAST_TIDE),
            1,
            "Pressure 响应必须只入队一次定向兽潮"
        );
        assert_eq!(vfx_events.iter_current_update_events().count(), 1);
        assert_eq!(audio_events.iter_current_update_events().count(), 1);
        let narration = expect_tiandao_narration_request(&rx);
        assert_eq!(
            narration.response_level,
            TiandaoHuntResponseLevelV1::Pressure
        );
    }

    #[test]
    fn watch_response_drains_zone_qi_through_conserving_ledger_transfer() {
        let mut zones = single_zone_registry("spawn", 0.6);
        let mut ledger = WorldQiAccount::default();
        let before = qi_snapshot(0.6, &ledger);

        let transfer = apply_watch_zone_qi_drain(
            "spawn",
            tiandao_response_profile(TiandaoResponseLevel::Watch)
                .unwrap()
                .interval_ticks,
            &mut zones,
            &mut ledger,
        )
        .expect("Watch 级必须发出守恒 QiTransfer");

        assert_eq!(transfer.from, QiAccountId::zone("spawn"));
        assert_eq!(transfer.to, QiAccountId::tiandao());
        assert_eq!(transfer.reason, QiTransferReason::TiandaoWatchDrain);
        assert_close(transfer.amount, 0.05);
        assert_close(zones.find_zone_by_name("spawn").unwrap().spirit_qi, 0.55);
        assert_close(ledger.balance(&QiAccountId::zone("spawn")), 0.0);
        assert_close(ledger.balance(&QiAccountId::tiandao()), 0.05);
        crate::qi_physics::assert_conservation(
            &before,
            &qi_snapshot(zones.find_zone_by_name("spawn").unwrap().spirit_qi, &ledger),
            0.0,
        )
        .expect("Watch 级 zone qi 微调必须在 zone_qi + ledger_qi 口径守恒");
    }

    #[test]
    fn watch_zone_qi_drain_is_noop_when_zone_is_missing() {
        let mut zones = single_zone_registry("spawn", 0.6);
        let mut ledger = WorldQiAccount::default();

        let transfer = apply_watch_zone_qi_drain(
            "missing",
            tiandao_response_profile(TiandaoResponseLevel::Watch)
                .unwrap()
                .interval_ticks,
            &mut zones,
            &mut ledger,
        );

        assert!(
            transfer.is_none(),
            "未知 zone 不应生成 Watch 级 QiTransfer，got={transfer:?}"
        );
        assert_close(zones.find_zone_by_name("spawn").unwrap().spirit_qi, 0.6);
        assert_close(ledger.total(), 0.0);
    }

    #[test]
    fn watch_zone_qi_drain_is_noop_when_zone_qi_is_empty_or_negative() {
        for zone_qi in [0.0, -0.1] {
            let mut zones = single_zone_registry("spawn", zone_qi);
            let mut ledger = WorldQiAccount::default();

            let transfer = apply_watch_zone_qi_drain(
                "spawn",
                tiandao_response_profile(TiandaoResponseLevel::Watch)
                    .unwrap()
                    .interval_ticks,
                &mut zones,
                &mut ledger,
            );

            assert!(
                transfer.is_none(),
                "zone_qi={zone_qi} 时 Watch 级不应从空/负真元区抽取，got={transfer:?}"
            );
            assert_close(zones.find_zone_by_name("spawn").unwrap().spirit_qi, zone_qi);
            assert_close(ledger.total(), 0.0);
        }
    }

    #[test]
    fn watch_zone_qi_drain_partial_transfer_preserves_conservation() {
        let mut zones = single_zone_registry("spawn", 0.02);
        let mut ledger = WorldQiAccount::default();
        let before = qi_snapshot(0.02, &ledger);

        let transfer = apply_watch_zone_qi_drain(
            "spawn",
            tiandao_response_profile(TiandaoResponseLevel::Watch)
                .unwrap()
                .interval_ticks,
            &mut zones,
            &mut ledger,
        )
        .expect("低于单次抽取量但大于 0 的 zone qi 必须被部分转移");

        assert_close(transfer.amount, 0.02);
        assert_close(zones.find_zone_by_name("spawn").unwrap().spirit_qi, 0.0);
        assert_close(ledger.balance(&QiAccountId::tiandao()), 0.02);
        crate::qi_physics::assert_conservation(
            &before,
            &qi_snapshot(zones.find_zone_by_name("spawn").unwrap().spirit_qi, &ledger),
            0.0,
        )
        .expect("Watch 级部分抽取必须保持 zone_qi + ledger_qi 守恒");
    }

    #[test]
    fn watch_zone_qi_drain_zero_interval_is_noop() {
        let mut zones = single_zone_registry("spawn", 0.6);
        let mut ledger = WorldQiAccount::default();

        let transfer = apply_watch_zone_qi_drain("spawn", 0, &mut zones, &mut ledger);

        assert!(
            transfer.is_none(),
            "interval_ticks=0 时 Watch 级不应生成 QiTransfer，got={transfer:?}"
        );
        assert_close(zones.find_zone_by_name("spawn").unwrap().spirit_qi, 0.6);
        assert_close(ledger.total(), 0.0);
    }

    #[test]
    fn tribulation_response_enqueues_thunder_with_targeted_feedback() {
        let mut attention = TiandaoAttention::default();
        let (active_events, _zones, vfx_events, audio_events, rx) = emit_response_chain_for_test(
            &mut attention,
            TiandaoResponseLevel::Tribulation,
            75.0,
            200,
        );

        assert!(active_events.contains("spawn", EVENT_THUNDER_TRIBULATION));
        assert_eq!(
            active_events.thunder_target_for_zone("spawn").as_deref(),
            Some("offline:Alice"),
            "Tribulation 雷劫必须带 target_player，避免退化成普通区域天灾"
        );
        assert_eq!(vfx_events.iter_current_update_events().count(), 1);
        assert_eq!(audio_events.iter_current_update_events().count(), 1);
        let narration = expect_tiandao_narration_request(&rx);
        assert_eq!(
            narration.response_level,
            TiandaoHuntResponseLevelV1::Tribulation
        );
    }

    #[test]
    fn annihilate_response_enqueues_realm_collapse_and_keeps_attention_sticky() {
        let mut attention = TiandaoAttention {
            level: 95.0,
            response: TiandaoResponseLevel::Annihilate,
            ..TiandaoAttention::default()
        };
        let (active_events, _zones, vfx_events, audio_events, rx) = emit_response_chain_for_test(
            &mut attention,
            TiandaoResponseLevel::Annihilate,
            95.0,
            200,
        );

        assert!(active_events.contains("spawn", EVENT_REALM_COLLAPSE));
        assert_eq!(vfx_events.iter_current_update_events().count(), 1);
        assert_eq!(audio_events.iter_current_update_events().count(), 1);
        let narration = expect_tiandao_narration_request(&rx);
        assert_eq!(
            narration.response_level,
            TiandaoHuntResponseLevelV1::Annihilate
        );

        advance_attention(&mut attention, input(Realm::Void), 400);
        assert_eq!(
            attention.response,
            TiandaoResponseLevel::Annihilate,
            "Annihilate 级不会靠自然衰减解除，必须死亡或负灵域反制"
        );
    }

    #[test]
    fn watch_profile_has_audio_but_no_particle_event() {
        let eval = TiandaoEvalSnapshot {
            position: DVec3::new(1.2, 64.0, -3.4),
            zone_name: Some("spawn".to_string()),
            zone_spirit_qi: 0.6,
            realm: Realm::Spirit,
            activity: TiandaoActivity::Standing,
            response: TiandaoResponseLevel::Watch,
            level: 20.0,
            countermeasure: TiandaoCountermeasureOutcome::default(),
        };
        let profile = tiandao_response_profile(TiandaoResponseLevel::Watch).unwrap();

        let player = Entity::from_raw(42);
        let audio = tiandao_audio_request(&eval, profile, player);

        assert_eq!(audio.recipe_id, "tiandao_watch_ambient");
        assert_eq!(audio.instance_id, TIANDAO_AUDIO_INSTANCE_BASE + 1);
        assert_eq!(audio.flag.as_deref(), Some("tiandao:watch"));
        assert!(matches!(audio.recipient, AudioRecipient::Single(entity) if entity == player));
        assert!(
            tiandao_vfx_request(&eval, profile).is_none(),
            "Watch 级按 plan 只给 HUD/音效氛围，不产生可见粒子"
        );
    }

    #[test]
    fn watch_response_publishes_narration_request_before_event_name_gate() {
        let (redis, rx) = narration_test_bridge();
        let mut attention = TiandaoAttention::default();

        apply_tiandao_response_chain(
            &mut attention,
            narration_test_snapshot(TiandaoResponseLevel::Watch, 20.0),
            Entity::from_raw(42),
            "Alice",
            TiandaoResponseSinks {
                zones: None,
                active_events: None,
                vfx_events: None,
                audio_events: None,
                redis: Some(&redis),
                qi_ledger: None,
            },
            200,
        );

        let payload = expect_tiandao_narration_request(&rx);
        assert_eq!(payload.character_id, "offline:Alice");
        assert_eq!(payload.realm, "Spirit");
        assert_eq!(payload.attention_level, 20.0);
        assert_eq!(payload.response_level, TiandaoHuntResponseLevelV1::Watch);
        assert_eq!(payload.zone, "spawn");
        assert_eq!(payload.narration_count, 0);
        assert!(
            payload
                .recent_actions
                .iter()
                .any(|entry| entry == "activity:meditating"),
            "recent_actions must carry the activity context used by the agent prompt"
        );
    }

    #[test]
    fn narration_request_respects_interval_and_increments_same_response_count() {
        let (redis, rx) = narration_test_bridge();
        let mut attention = TiandaoAttention::default();

        apply_tiandao_response_chain(
            &mut attention,
            narration_test_snapshot(TiandaoResponseLevel::Watch, 20.0),
            Entity::from_raw(42),
            "Alice",
            narration_test_sinks(&redis),
            200,
        );
        assert_eq!(expect_tiandao_narration_request(&rx).narration_count, 0);

        apply_tiandao_response_chain(
            &mut attention,
            narration_test_snapshot(TiandaoResponseLevel::Watch, 21.0),
            Entity::from_raw(42),
            "Alice",
            narration_test_sinks(&redis),
            200 + tiandao_response_profile(TiandaoResponseLevel::Watch)
                .unwrap()
                .interval_ticks
                - 1,
        );
        assert!(
            rx.try_recv().is_err(),
            "same response inside interval must not publish duplicate narration"
        );

        apply_tiandao_response_chain(
            &mut attention,
            narration_test_snapshot(TiandaoResponseLevel::Watch, 22.0),
            Entity::from_raw(42),
            "Alice",
            narration_test_sinks(&redis),
            200 + tiandao_response_profile(TiandaoResponseLevel::Watch)
                .unwrap()
                .interval_ticks,
        );
        assert_eq!(expect_tiandao_narration_request(&rx).narration_count, 1);
    }

    #[test]
    fn narration_count_resets_when_response_level_upgrades() {
        let (redis, rx) = narration_test_bridge();
        let mut attention = TiandaoAttention::default();

        apply_tiandao_response_chain(
            &mut attention,
            narration_test_snapshot(TiandaoResponseLevel::Watch, 20.0),
            Entity::from_raw(42),
            "Alice",
            narration_test_sinks(&redis),
            200,
        );
        let _ = expect_tiandao_narration_request(&rx);

        apply_tiandao_response_chain(
            &mut attention,
            narration_test_snapshot(TiandaoResponseLevel::Watch, 22.0),
            Entity::from_raw(42),
            "Alice",
            narration_test_sinks(&redis),
            200 + tiandao_response_profile(TiandaoResponseLevel::Watch)
                .unwrap()
                .interval_ticks,
        );
        assert_eq!(expect_tiandao_narration_request(&rx).narration_count, 1);

        apply_tiandao_response_chain(
            &mut attention,
            narration_test_snapshot(TiandaoResponseLevel::Pressure, 41.0),
            Entity::from_raw(42),
            "Alice",
            narration_test_sinks(&redis),
            200 + tiandao_response_profile(TiandaoResponseLevel::Watch)
                .unwrap()
                .interval_ticks
                + 1,
        );
        let payload = expect_tiandao_narration_request(&rx);
        assert_eq!(payload.response_level, TiandaoHuntResponseLevelV1::Pressure);
        assert_eq!(payload.narration_count, 0);
    }

    #[test]
    fn narration_count_resets_after_response_drops_to_none() {
        let (redis, rx) = narration_test_bridge();
        let mut attention = TiandaoAttention::default();
        let interval = tiandao_response_profile(TiandaoResponseLevel::Watch)
            .unwrap()
            .interval_ticks;

        apply_tiandao_response_chain(
            &mut attention,
            narration_test_snapshot(TiandaoResponseLevel::Watch, 20.0),
            Entity::from_raw(42),
            "Alice",
            narration_test_sinks(&redis),
            200,
        );
        assert_eq!(expect_tiandao_narration_request(&rx).narration_count, 0);

        apply_tiandao_response_chain(
            &mut attention,
            narration_test_snapshot(TiandaoResponseLevel::Watch, 22.0),
            Entity::from_raw(42),
            "Alice",
            narration_test_sinks(&redis),
            200 + interval,
        );
        assert_eq!(expect_tiandao_narration_request(&rx).narration_count, 1);

        apply_tiandao_response_chain(
            &mut attention,
            narration_test_snapshot(TiandaoResponseLevel::None, 8.0),
            Entity::from_raw(42),
            "Alice",
            narration_test_sinks(&redis),
            200 + interval + 1,
        );
        assert_eq!(attention.last_emitted_response, TiandaoResponseLevel::None);
        assert_eq!(attention.last_response_tick, 0);
        assert_eq!(attention.narration_count, 0);
        assert!(
            rx.try_recv().is_err(),
            "None response must not publish narration while resetting response state"
        );

        apply_tiandao_response_chain(
            &mut attention,
            narration_test_snapshot(TiandaoResponseLevel::Watch, 19.0),
            Entity::from_raw(42),
            "Alice",
            narration_test_sinks(&redis),
            200 + interval + 2,
        );
        assert_eq!(expect_tiandao_narration_request(&rx).narration_count, 0);
    }
}
