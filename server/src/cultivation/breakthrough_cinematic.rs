//! 突破 cinematic 编排层。
//!
//! 这里不重写粒子 / HUD / 音频底层，只把突破结果拆成 5 个 server-authoritative
//! 阶段，并在阶段切换时推送 server_data、VFX 与 agent narration 事件。

use valence::prelude::{
    bevy_ecs, Client, Commands, Component, DVec3, Entity, Event, EventReader, EventWriter,
    Position, Query, Res, UniqueId, With,
};

use crate::combat::events::CombatEvent;
use crate::cultivation::breakthrough::{next_realm, BreakthroughError, BreakthroughOutcome};
use crate::cultivation::components::Realm;
use crate::cultivation::tick::CultivationClock;
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::gameplay_vfx;
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::cultivation::{realm_to_string, BreakthroughCinematicEventV1};
use crate::schema::server_data::{BreakthroughCinematicS2cV1, ServerDataPayloadV1, ServerDataV1};
use crate::world::dimension::{CurrentDimension, DimensionKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BreakthroughCinematicPhase {
    Prelude,
    Charge,
    Catalyze,
    Apex,
    Aftermath,
}

impl BreakthroughCinematicPhase {
    pub const ALL: [Self; 5] = [
        Self::Prelude,
        Self::Charge,
        Self::Catalyze,
        Self::Apex,
        Self::Aftermath,
    ];

    pub fn wire_name(self) -> &'static str {
        match self {
            Self::Prelude => "prelude",
            Self::Charge => "charge",
            Self::Catalyze => "catalyze",
            Self::Apex => "apex",
            Self::Aftermath => "aftermath",
        }
    }

    fn next(self) -> Option<Self> {
        match self {
            Self::Prelude => Some(Self::Charge),
            Self::Charge => Some(Self::Catalyze),
            Self::Catalyze => Some(Self::Apex),
            Self::Apex => Some(Self::Aftermath),
            Self::Aftermath => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakthroughCinematicResult {
    Pending,
    Success,
    Failure,
    Interrupted,
}

impl BreakthroughCinematicResult {
    pub fn wire_name(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Success => "success",
            Self::Failure => "failure",
            Self::Interrupted => "interrupted",
        }
    }

    fn failed(self) -> bool {
        matches!(self, Self::Failure | Self::Interrupted)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BreakthroughCinematicDurations {
    pub prelude: u32,
    pub charge: u32,
    pub catalyze: u32,
    pub apex: u32,
    pub aftermath: u32,
}

impl BreakthroughCinematicDurations {
    pub const fn new(prelude: u32, charge: u32, catalyze: u32, apex: u32, aftermath: u32) -> Self {
        Self {
            prelude,
            charge,
            catalyze,
            apex,
            aftermath,
        }
    }

    pub fn for_phase(self, phase: BreakthroughCinematicPhase) -> u32 {
        match phase {
            BreakthroughCinematicPhase::Prelude => self.prelude,
            BreakthroughCinematicPhase::Charge => self.charge,
            BreakthroughCinematicPhase::Catalyze => self.catalyze,
            BreakthroughCinematicPhase::Apex => self.apex,
            BreakthroughCinematicPhase::Aftermath => self.aftermath,
        }
        .max(1)
    }

    pub fn total(self) -> u32 {
        self.prelude + self.charge + self.catalyze + self.apex + self.aftermath
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BreakthroughCinematicProfile {
    pub durations: BreakthroughCinematicDurations,
    pub visible_radius_blocks: f64,
    pub particle_density: f32,
    pub apex_intensity: f32,
    pub global: bool,
    pub distant_billboard: bool,
    pub tribulation_overlay: bool,
    pub style: &'static str,
    pub season_overlay: &'static str,
}

impl BreakthroughCinematicProfile {
    pub fn duration(self, phase: BreakthroughCinematicPhase) -> u32 {
        self.durations.for_phase(phase)
    }
}

#[derive(Debug, Clone, Component)]
pub struct BreakthroughCinematic {
    pub realm_from: Realm,
    pub realm_to: Realm,
    pub phase: BreakthroughCinematicPhase,
    pub phase_tick: u32,
    pub started_tick: u64,
    pub result: BreakthroughCinematicResult,
    pub interrupted: bool,
    pub profile: BreakthroughCinematicProfile,
    pub actor_id: String,
}

impl BreakthroughCinematic {
    pub fn new(
        realm_from: Realm,
        realm_to: Realm,
        result: BreakthroughCinematicResult,
        started_tick: u64,
        actor_id: impl Into<String>,
    ) -> Option<Self> {
        let profile = profile_for_transition(realm_from, realm_to)?;
        Some(Self {
            realm_from,
            realm_to,
            phase: BreakthroughCinematicPhase::Prelude,
            phase_tick: 0,
            started_tick,
            result,
            interrupted: false,
            profile,
            actor_id: actor_id.into(),
        })
    }

    pub fn phase_duration_ticks(&self) -> u32 {
        self.profile.duration(self.phase)
    }

    pub fn advance_one_tick(&mut self) -> BreakthroughCinematicAdvance {
        let duration = self.phase_duration_ticks();
        if self.phase_tick + 1 < duration {
            self.phase_tick += 1;
            return BreakthroughCinematicAdvance::NoChange;
        }

        match self.phase.next() {
            Some(next) => {
                self.phase = next;
                self.phase_tick = 0;
                BreakthroughCinematicAdvance::PhaseChanged(next)
            }
            None => BreakthroughCinematicAdvance::Finished,
        }
    }

    pub fn interrupt(&mut self) {
        self.interrupted = true;
        self.result = BreakthroughCinematicResult::Interrupted;
        self.phase = BreakthroughCinematicPhase::Aftermath;
        self.phase_tick = 0;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakthroughCinematicAdvance {
    NoChange,
    PhaseChanged(BreakthroughCinematicPhase),
    Finished,
}

#[derive(Debug, Clone, Event)]
pub struct BreakthroughCinematicAgentEvent {
    pub payload: BreakthroughCinematicEventV1,
}

type CinematicClientQueryItem<'a> = (
    Entity,
    &'a mut Client,
    &'a Position,
    Option<&'a CurrentDimension>,
);

pub fn profile_for_transition(from: Realm, to: Realm) -> Option<BreakthroughCinematicProfile> {
    if next_realm(from) != Some(to) {
        return None;
    }

    Some(match (from, to) {
        (Realm::Awaken, Realm::Induce) => BreakthroughCinematicProfile {
            durations: BreakthroughCinematicDurations::new(60, 200, 100, 40, 120),
            visible_radius_blocks: 256.0,
            particle_density: 1.0,
            apex_intensity: 0.55,
            global: false,
            distant_billboard: false,
            tribulation_overlay: false,
            style: "fresh_spiral",
            season_overlay: "adaptive",
        },
        (Realm::Induce, Realm::Condense) => BreakthroughCinematicProfile {
            durations: BreakthroughCinematicDurations::new(120, 420, 220, 60, 180),
            visible_radius_blocks: 512.0,
            particle_density: 1.5,
            apex_intensity: 0.68,
            global: false,
            distant_billboard: false,
            tribulation_overlay: false,
            style: "meridian_shell",
            season_overlay: "adaptive",
        },
        (Realm::Condense, Realm::Solidify) => BreakthroughCinematicProfile {
            durations: BreakthroughCinematicDurations::new(180, 680, 380, 80, 280),
            visible_radius_blocks: 1024.0,
            particle_density: 2.2,
            apex_intensity: 0.78,
            global: false,
            distant_billboard: true,
            tribulation_overlay: false,
            style: "golden_core",
            season_overlay: "adaptive",
        },
        (Realm::Solidify, Realm::Spirit) => BreakthroughCinematicProfile {
            durations: BreakthroughCinematicDurations::new(260, 1020, 620, 120, 380),
            visible_radius_blocks: 5000.0,
            particle_density: 3.0,
            apex_intensity: 0.9,
            global: true,
            distant_billboard: true,
            tribulation_overlay: false,
            style: "sky_resonance",
            season_overlay: "adaptive",
        },
        (Realm::Spirit, Realm::Void) => BreakthroughCinematicProfile {
            durations: BreakthroughCinematicDurations::new(360, 1440, 980, 180, 640),
            visible_radius_blocks: 10_000.0,
            particle_density: 4.0,
            apex_intensity: 1.0,
            global: true,
            distant_billboard: true,
            tribulation_overlay: true,
            style: "void_tribulation",
            season_overlay: "volatile",
        },
        _ => return None,
    })
}

pub fn start_breakthrough_cinematic_on_outcome(
    mut commands: Commands,
    clock: Res<CultivationClock>,
    mut outcomes: EventReader<BreakthroughOutcome>,
    actor_q: Query<(
        Option<&Position>,
        Option<&CurrentDimension>,
        Option<&UniqueId>,
    )>,
    mut clients: Query<CinematicClientQueryItem<'_>, With<Client>>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    mut agent_events: EventWriter<BreakthroughCinematicAgentEvent>,
) {
    for outcome in outcomes.read() {
        let Some((to, result)) = cinematic_target_and_result(outcome) else {
            continue;
        };
        let actor_id = actor_q
            .get(outcome.entity)
            .ok()
            .and_then(|(_, _, unique_id)| unique_id)
            .map(|id| id.0.to_string())
            .unwrap_or_else(|| format!("entity:{:?}", outcome.entity));
        let Some(cinematic) =
            BreakthroughCinematic::new(outcome.from, to, result, clock.tick, actor_id)
        else {
            continue;
        };
        let (origin, dimension) = actor_origin_and_dimension(outcome.entity, &actor_q);

        emit_cinematic_phase(
            &cinematic,
            origin,
            dimension,
            clock.tick,
            &mut clients,
            &mut vfx_events,
            &mut agent_events,
        );
        commands.entity(outcome.entity).insert(cinematic);
    }
}

pub fn breakthrough_cinematic_phase_tick(
    mut commands: Commands,
    clock: Res<CultivationClock>,
    mut cinematics: Query<(
        Entity,
        &mut BreakthroughCinematic,
        Option<&Position>,
        Option<&CurrentDimension>,
    )>,
    mut clients: Query<CinematicClientQueryItem<'_>, With<Client>>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    mut agent_events: EventWriter<BreakthroughCinematicAgentEvent>,
) {
    for (entity, mut cinematic, position, current_dimension) in &mut cinematics {
        match cinematic.advance_one_tick() {
            BreakthroughCinematicAdvance::NoChange => {}
            BreakthroughCinematicAdvance::PhaseChanged(_) => {
                emit_cinematic_phase(
                    &cinematic,
                    position_to_array(position),
                    dimension_or_default(current_dimension),
                    clock.tick,
                    &mut clients,
                    &mut vfx_events,
                    &mut agent_events,
                );
            }
            BreakthroughCinematicAdvance::Finished => {
                commands.entity(entity).remove::<BreakthroughCinematic>();
            }
        }
    }
}

pub fn interrupt_breakthrough_cinematic_on_hit(
    clock: Res<CultivationClock>,
    mut hits: EventReader<CombatEvent>,
    mut cinematics: Query<(
        &mut BreakthroughCinematic,
        Option<&Position>,
        Option<&CurrentDimension>,
    )>,
    mut clients: Query<CinematicClientQueryItem<'_>, With<Client>>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    mut agent_events: EventWriter<BreakthroughCinematicAgentEvent>,
) {
    for hit in hits.read() {
        if hit.damage <= 0.0 {
            continue;
        }
        let Ok((mut cinematic, position, current_dimension)) = cinematics.get_mut(hit.target)
        else {
            continue;
        };
        if cinematic.interrupted || cinematic.phase == BreakthroughCinematicPhase::Aftermath {
            continue;
        }

        cinematic.interrupt();
        emit_cinematic_phase(
            &cinematic,
            position_to_array(position),
            dimension_or_default(current_dimension),
            clock.tick,
            &mut clients,
            &mut vfx_events,
            &mut agent_events,
        );
    }
}

fn cinematic_target_and_result(
    outcome: &BreakthroughOutcome,
) -> Option<(Realm, BreakthroughCinematicResult)> {
    match outcome.result {
        Ok(success) => Some((success.to, BreakthroughCinematicResult::Success)),
        Err(BreakthroughError::RolledFailure { .. }) => {
            next_realm(outcome.from).map(|to| (to, BreakthroughCinematicResult::Failure))
        }
        Err(_) => None,
    }
}

fn actor_origin_and_dimension(
    entity: Entity,
    actor_q: &Query<(
        Option<&Position>,
        Option<&CurrentDimension>,
        Option<&UniqueId>,
    )>,
) -> ([f64; 3], DimensionKind) {
    actor_q
        .get(entity)
        .ok()
        .map(|(position, dimension, _)| {
            (position_to_array(position), dimension_or_default(dimension))
        })
        .unwrap_or(([0.0, 0.0, 0.0], DimensionKind::Overworld))
}

fn position_to_array(position: Option<&Position>) -> [f64; 3] {
    position
        .map(|position| {
            let p = position.get();
            [p.x, p.y, p.z]
        })
        .unwrap_or([0.0, 0.0, 0.0])
}

fn dimension_or_default(current_dimension: Option<&CurrentDimension>) -> DimensionKind {
    current_dimension
        .map(|dimension| dimension.0)
        .unwrap_or(DimensionKind::Overworld)
}

fn emit_cinematic_phase(
    cinematic: &BreakthroughCinematic,
    origin: [f64; 3],
    dimension: DimensionKind,
    at_tick: u64,
    clients: &mut Query<CinematicClientQueryItem<'_>, With<Client>>,
    vfx_events: &mut EventWriter<VfxEventRequest>,
    agent_events: &mut EventWriter<BreakthroughCinematicAgentEvent>,
) {
    let payload = cinematic.to_s2c_payload(origin, at_tick);
    send_cinematic_payload_to_visible_clients(&payload, dimension, clients);

    for request in cinematic_vfx_requests(cinematic, origin) {
        vfx_events.send(request);
    }

    agent_events.send(BreakthroughCinematicAgentEvent {
        payload: cinematic.to_agent_payload(origin, at_tick),
    });
}

fn send_cinematic_payload_to_visible_clients(
    payload: &BreakthroughCinematicS2cV1,
    dimension: DimensionKind,
    clients: &mut Query<CinematicClientQueryItem<'_>, With<Client>>,
) {
    let envelope = ServerDataV1::new(ServerDataPayloadV1::BreakthroughCinematic(payload.clone()));
    let payload_type = payload_type_label(envelope.payload_type());
    let payload_bytes = match serialize_server_data_payload(&envelope) {
        Ok(bytes) => bytes,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };

    let origin = DVec3::new(
        payload.world_pos[0],
        payload.world_pos[1],
        payload.world_pos[2],
    );
    let radius_sq = payload.visible_radius_blocks * payload.visible_radius_blocks;
    for (_, mut client, position, client_dimension) in clients.iter_mut() {
        if dimension_or_default(client_dimension) != dimension {
            continue;
        }
        let client_pos = position.get();
        if !payload.global {
            let delta_sq = origin.distance_squared(client_pos);
            if delta_sq > radius_sq {
                continue;
            }
        }
        send_server_data_payload(&mut client, payload_bytes.as_slice());
    }
}

impl BreakthroughCinematic {
    fn to_s2c_payload(&self, origin: [f64; 3], at_tick: u64) -> BreakthroughCinematicS2cV1 {
        BreakthroughCinematicS2cV1 {
            actor_id: self.actor_id.clone(),
            phase: self.phase.wire_name().to_string(),
            phase_tick: self.phase_tick,
            phase_duration_ticks: self.phase_duration_ticks(),
            realm_from: realm_to_string(self.realm_from).to_string(),
            realm_to: realm_to_string(self.realm_to).to_string(),
            result: self.result.wire_name().to_string(),
            interrupted: self.interrupted,
            world_pos: origin,
            visible_radius_blocks: self.profile.visible_radius_blocks,
            global: self.profile.global,
            distant_billboard: self.profile.distant_billboard,
            particle_density: self.profile.particle_density,
            intensity: phase_intensity(self),
            season_overlay: self.profile.season_overlay.to_string(),
            style: self.profile.style.to_string(),
            at_tick,
        }
    }

    fn to_agent_payload(&self, origin: [f64; 3], at_tick: u64) -> BreakthroughCinematicEventV1 {
        BreakthroughCinematicEventV1 {
            v: 1,
            actor_id: self.actor_id.clone(),
            phase: self.phase.wire_name().to_string(),
            phase_tick: self.phase_tick,
            phase_duration_ticks: self.phase_duration_ticks(),
            realm_from: realm_to_string(self.realm_from).to_string(),
            realm_to: realm_to_string(self.realm_to).to_string(),
            result: self.result.wire_name().to_string(),
            interrupted: self.interrupted,
            world_pos: origin,
            visible_radius_blocks: self.profile.visible_radius_blocks,
            global: self.profile.global,
            distant_billboard: self.profile.distant_billboard,
            season_overlay: self.profile.season_overlay.to_string(),
            style: self.profile.style.to_string(),
            at_tick,
        }
    }
}

pub fn cinematic_vfx_requests(
    cinematic: &BreakthroughCinematic,
    origin: [f64; 3],
) -> Vec<VfxEventRequest> {
    let origin = DVec3::new(origin[0], origin[1], origin[2]);
    let density = cinematic.profile.particle_density;
    let intensity = phase_intensity(cinematic);
    let result_failed = cinematic.result.failed();
    match cinematic.phase {
        BreakthroughCinematicPhase::Prelude => vec![gameplay_vfx::spawn_request(
            gameplay_vfx::CULTIVATION_ABSORB,
            origin,
            None,
            "#66FFCC",
            intensity,
            scaled_count(8, density),
            40,
        )],
        BreakthroughCinematicPhase::Charge => vec![
            gameplay_vfx::spawn_request(
                gameplay_vfx::CULTIVATION_ABSORB,
                origin,
                None,
                "#88CCDD",
                intensity,
                scaled_count(18, density),
                60,
            ),
            gameplay_vfx::spawn_request(
                gameplay_vfx::MERIDIAN_OPEN,
                origin,
                Some([0.0, 1.0, 0.0]),
                "#88CCDD",
                intensity,
                scaled_count(4, density),
                50,
            ),
        ],
        BreakthroughCinematicPhase::Catalyze => vec![gameplay_vfx::spawn_request(
            gameplay_vfx::BREAKTHROUGH_PILLAR,
            origin,
            None,
            pillar_color(cinematic),
            intensity,
            scaled_count(16, density),
            80,
        )],
        BreakthroughCinematicPhase::Apex => vec![gameplay_vfx::spawn_request(
            gameplay_vfx::BREAKTHROUGH_PILLAR,
            origin,
            None,
            pillar_color(cinematic),
            cinematic.profile.apex_intensity,
            scaled_count(28, density),
            90,
        )],
        BreakthroughCinematicPhase::Aftermath if result_failed => {
            vec![gameplay_vfx::spawn_request(
                gameplay_vfx::BREAKTHROUGH_FAIL,
                origin,
                None,
                "#FF3344",
                0.9,
                scaled_count(if cinematic.interrupted { 32 } else { 20 }, density),
                60,
            )]
        }
        BreakthroughCinematicPhase::Aftermath => vec![gameplay_vfx::spawn_request(
            gameplay_vfx::BREAKTHROUGH_PILLAR,
            origin,
            None,
            "#FFD700",
            0.45,
            scaled_count(10, density),
            120,
        )],
    }
}

fn scaled_count(base: u32, density: f32) -> u32 {
    ((base as f32 * density).round() as u32).clamp(1, 128)
}

fn phase_intensity(cinematic: &BreakthroughCinematic) -> f32 {
    let apex = cinematic.profile.apex_intensity;
    match cinematic.phase {
        BreakthroughCinematicPhase::Prelude => (apex * 0.35).clamp(0.1, 1.0),
        BreakthroughCinematicPhase::Charge => (apex * 0.55).clamp(0.1, 1.0),
        BreakthroughCinematicPhase::Catalyze => (apex * 0.75).clamp(0.1, 1.0),
        BreakthroughCinematicPhase::Apex => apex.clamp(0.1, 1.0),
        BreakthroughCinematicPhase::Aftermath if cinematic.result.failed() => 0.85,
        BreakthroughCinematicPhase::Aftermath => (apex * 0.45).clamp(0.1, 1.0),
    }
}

fn pillar_color(cinematic: &BreakthroughCinematic) -> &'static str {
    match cinematic.realm_to {
        Realm::Induce => "#66FFCC",
        Realm::Condense => "#88CCDD",
        Realm::Solidify => "#FFD700",
        Realm::Spirit => "#FFF3B0",
        Realm::Void => "#B445FF",
        Realm::Awaken => "#66FFCC",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cinematic_profile_duration_matrix_matches_plan() {
        let yinqi = profile_for_transition(Realm::Awaken, Realm::Induce).unwrap();
        assert_eq!(yinqi.durations.total(), 520);
        assert_eq!(yinqi.visible_radius_blocks, 256.0);

        let ningmai = profile_for_transition(Realm::Induce, Realm::Condense).unwrap();
        assert_eq!(ningmai.durations.total(), 1_000);
        assert_eq!(ningmai.visible_radius_blocks, 512.0);

        let guyuan = profile_for_transition(Realm::Condense, Realm::Solidify).unwrap();
        assert_eq!(guyuan.durations.total(), 1_600);
        assert_eq!(guyuan.visible_radius_blocks, 1_024.0);

        let tongling = profile_for_transition(Realm::Solidify, Realm::Spirit).unwrap();
        assert_eq!(tongling.durations.total(), 2_400);
        assert!(tongling.global);
        assert!(tongling.distant_billboard);

        let huaxu = profile_for_transition(Realm::Spirit, Realm::Void).unwrap();
        assert_eq!(huaxu.durations.total(), 3_600);
        assert!(huaxu.tribulation_overlay);
    }

    #[test]
    fn cinematic_phase_progression_advances_in_order() {
        let mut cinematic = BreakthroughCinematic::new(
            Realm::Awaken,
            Realm::Induce,
            BreakthroughCinematicResult::Success,
            10,
            "entity:1",
        )
        .unwrap();

        for _ in 0..59 {
            assert_eq!(
                cinematic.advance_one_tick(),
                BreakthroughCinematicAdvance::NoChange
            );
        }
        assert_eq!(
            cinematic.advance_one_tick(),
            BreakthroughCinematicAdvance::PhaseChanged(BreakthroughCinematicPhase::Charge)
        );
        assert_eq!(cinematic.phase_tick, 0);
        assert_eq!(cinematic.phase, BreakthroughCinematicPhase::Charge);
    }

    #[test]
    fn interrupt_jumps_to_fail_aftermath() {
        let mut cinematic = BreakthroughCinematic::new(
            Realm::Induce,
            Realm::Condense,
            BreakthroughCinematicResult::Success,
            10,
            "entity:1",
        )
        .unwrap();
        cinematic.phase = BreakthroughCinematicPhase::Catalyze;
        cinematic.phase_tick = 25;

        cinematic.interrupt();

        assert_eq!(cinematic.phase, BreakthroughCinematicPhase::Aftermath);
        assert_eq!(cinematic.phase_tick, 0);
        assert_eq!(cinematic.result, BreakthroughCinematicResult::Interrupted);
        assert!(cinematic.interrupted);
    }

    #[test]
    fn cinematic_agent_event_contains_phase_contract() {
        let cinematic = BreakthroughCinematic::new(
            Realm::Solidify,
            Realm::Spirit,
            BreakthroughCinematicResult::Success,
            100,
            "00000000-0000-0000-0000-000000000001",
        )
        .unwrap();

        let payload = cinematic.to_agent_payload([10.0, 64.0, -4.0], 120);

        assert_eq!(payload.v, 1);
        assert_eq!(payload.phase, "prelude");
        assert_eq!(payload.realm_from, "Solidify");
        assert_eq!(payload.realm_to, "Spirit");
        assert!(payload.global);
        assert!(payload.distant_billboard);
    }
}
