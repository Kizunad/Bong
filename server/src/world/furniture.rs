use std::collections::HashMap;

use valence::prelude::{
    bevy_ecs, App, BlockState, Client, Entity, Events, IntoSystemConfigs, Position, Query, Res,
    ResMut, Resource, Update, Username, With,
};

use crate::combat::components::{Stamina, StaminaState, StatusEffects, TICKS_PER_SECOND};
use crate::combat::events::{ApplyStatusEffectIntent, StatusEffectKind};
use crate::combat::status::has_active_status;
use crate::combat::{status, CombatClock, CombatSystemSet};
use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest};
use crate::network::{gameplay_vfx, vfx_event_emit::VfxEventRequest};
use crate::player::gameplay::PendingGameplayNarrations;
use crate::schema::common::NarrationStyle;

pub const FURNITURE_AURA_RADIUS: i32 = 2;
pub const FURNITURE_AURA_TICK_INTERVAL_TICKS: u64 = TICKS_PER_SECOND;
pub const FURNITURE_AURA_DURATION_TICKS: u64 = 3 * TICKS_PER_SECOND;
pub const BED_REST_VFX_EVENT_ID: &str = "bong:furniture_bed_rest";
pub const MEDITATION_VFX_EVENT_ID: &str = "bong:furniture_meditation";
pub const FURNITURE_AURA_AUDIO_RECIPE_ID: &str = "furniture_aura_hint";

const BED_HEALTH_REGEN_MAGNITUDE: f32 = 0.5;
const MEDITATION_CULTIVATION_MAGNITUDE: f32 = 0.2;
const BED_REST_NARRATION: &str = "你倚着床榻歇下，伤处的钝痛慢慢退去。";
const MEDITATION_NARRATION: &str = "盘膝坐定，气息渐匀，周身灵机似乎流转得快了些。";
const BED_REST_COLOR: &str = "#E8C97A";
const MEDITATION_COLOR: &str = "#BFD8C8";

type FurnitureAuraPlayerItem<'a> = (
    Entity,
    &'a Position,
    &'a Stamina,
    &'a StatusEffects,
    &'a Username,
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FurnitureAura {
    BedRest,
    Meditation,
}

impl FurnitureAura {
    fn status_kind(self) -> StatusEffectKind {
        match self {
            Self::BedRest => StatusEffectKind::HealthRegenBoost,
            Self::Meditation => StatusEffectKind::CultivationAcceleration,
        }
    }

    fn magnitude(self) -> f32 {
        match self {
            Self::BedRest => BED_HEALTH_REGEN_MAGNITUDE,
            Self::Meditation => MEDITATION_CULTIVATION_MAGNITUDE,
        }
    }

    fn vfx_event_id(self) -> &'static str {
        match self {
            Self::BedRest => BED_REST_VFX_EVENT_ID,
            Self::Meditation => MEDITATION_VFX_EVENT_ID,
        }
    }

    fn color(self) -> &'static str {
        match self {
            Self::BedRest => BED_REST_COLOR,
            Self::Meditation => MEDITATION_COLOR,
        }
    }

    fn particle_count(self) -> u32 {
        match self {
            Self::BedRest => 6,
            Self::Meditation => 8,
        }
    }

    fn duration_ticks(self) -> u32 {
        match self {
            Self::BedRest => 12,
            Self::Meditation => 16,
        }
    }

    fn narration(self) -> &'static str {
        match self {
            Self::BedRest => BED_REST_NARRATION,
            Self::Meditation => MEDITATION_NARRATION,
        }
    }
}

pub fn register(app: &mut App) {
    app.init_resource::<FurnitureRegistry>();
    app.add_systems(
        Update,
        furniture_aura_tick
            .in_set(CombatSystemSet::Intent)
            .before(status::status_effect_apply_tick),
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FurnitureKind {
    SimpleBed,
    MeditationMat,
    MoistureBase,
    SpiritStoneRack,
}

impl FurnitureKind {
    #[cfg(test)]
    pub const ALL: [Self; 4] = [
        Self::SimpleBed,
        Self::MeditationMat,
        Self::MoistureBase,
        Self::SpiritStoneRack,
    ];

    #[cfg(test)]
    pub fn template_id(self) -> &'static str {
        match self {
            Self::SimpleBed => "simple_bed",
            Self::MeditationMat => "meditation_mat",
            Self::MoistureBase => "moisture_base",
            Self::SpiritStoneRack => "spirit_stone_rack",
        }
    }

    #[cfg(test)]
    pub fn block_state(self) -> BlockState {
        match self {
            Self::SimpleBed => BlockState::BONG_SIMPLE_BED,
            Self::MeditationMat => BlockState::BONG_MEDITATION_MAT,
            Self::MoistureBase => BlockState::BONG_MOISTURE_BASE,
            Self::SpiritStoneRack => BlockState::BONG_SPIRIT_STONE_RACK,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Resource)]
pub struct FurnitureRegistry {
    by_pos: HashMap<[i32; 3], FurnitureKind>,
}

impl FurnitureRegistry {
    pub fn register(&mut self, pos: [i32; 3], kind: FurnitureKind) -> Option<FurnitureKind> {
        self.by_pos.insert(pos, kind)
    }

    pub fn remove(&mut self, pos: [i32; 3]) -> Option<FurnitureKind> {
        self.by_pos.remove(&pos)
    }

    #[cfg(test)]
    pub fn kind_at(&self, pos: [i32; 3]) -> Option<FurnitureKind> {
        self.by_pos.get(&pos).copied()
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.by_pos.len()
    }

    #[allow(dead_code)]
    pub fn rebuild_from_blocks<I>(&mut self, blocks: I)
    where
        I: IntoIterator<Item = ([i32; 3], BlockState)>,
    {
        self.by_pos.clear();
        for (pos, state) in blocks {
            if let Some(kind) = furniture_kind_for_block_state(state) {
                self.register(pos, kind);
            }
        }
    }

    pub fn kinds_in_range(
        &self,
        center: [i32; 3],
        radius: i32,
    ) -> impl Iterator<Item = ([i32; 3], FurnitureKind)> + '_ {
        self.by_pos.iter().filter_map(move |(pos, kind)| {
            let distance = chebyshev_distance(*pos, center);
            (radius >= 0 && distance <= radius).then_some((*pos, *kind))
        })
    }
}

pub fn furniture_kind_for_template_id(template_id: &str) -> Option<FurnitureKind> {
    match template_id {
        "simple_bed" => Some(FurnitureKind::SimpleBed),
        "meditation_mat" => Some(FurnitureKind::MeditationMat),
        "moisture_base" => Some(FurnitureKind::MoistureBase),
        "spirit_stone_rack" => Some(FurnitureKind::SpiritStoneRack),
        _ => None,
    }
}

#[allow(dead_code)]
pub fn furniture_kind_for_block_state(state: BlockState) -> Option<FurnitureKind> {
    match state {
        BlockState::BONG_SIMPLE_BED => Some(FurnitureKind::SimpleBed),
        BlockState::BONG_MEDITATION_MAT => Some(FurnitureKind::MeditationMat),
        BlockState::BONG_MOISTURE_BASE => Some(FurnitureKind::MoistureBase),
        BlockState::BONG_SPIRIT_STONE_RACK => Some(FurnitureKind::SpiritStoneRack),
        _ => None,
    }
}

fn chebyshev_distance(left: [i32; 3], right: [i32; 3]) -> i32 {
    (left[0] - right[0])
        .abs()
        .max((left[1] - right[1]).abs())
        .max((left[2] - right[2]).abs())
}

#[allow(clippy::too_many_arguments)]
fn furniture_aura_tick(
    clock: Res<CombatClock>,
    registry: Res<FurnitureRegistry>,
    players: Query<FurnitureAuraPlayerItem<'_>, With<Client>>,
    mut status_events: Option<ResMut<Events<ApplyStatusEffectIntent>>>,
    mut vfx_events: Option<ResMut<Events<VfxEventRequest>>>,
    mut audio_events: Option<ResMut<Events<PlaySoundRecipeRequest>>>,
    mut narrations: Option<ResMut<PendingGameplayNarrations>>,
) {
    if !clock
        .tick
        .is_multiple_of(FURNITURE_AURA_TICK_INTERVAL_TICKS)
    {
        return;
    }
    let Some(status_events) = status_events.as_deref_mut() else {
        return;
    };

    for (entity, position, stamina, status_effects, username) in &players {
        if stamina.state != StaminaState::Idle {
            continue;
        }

        let player_block = block_pos_from_position(position);
        let mut has_bed = false;
        let mut has_meditation_mat = false;
        for (_, kind) in registry.kinds_in_range(player_block, FURNITURE_AURA_RADIUS) {
            match kind {
                FurnitureKind::SimpleBed => has_bed = true,
                FurnitureKind::MeditationMat => has_meditation_mat = true,
                FurnitureKind::MoistureBase | FurnitureKind::SpiritStoneRack => {}
            }
        }

        if has_bed {
            emit_furniture_aura(
                FurnitureAura::BedRest,
                entity,
                position,
                status_effects,
                username,
                clock.tick,
                status_events,
                vfx_events.as_deref_mut(),
                audio_events.as_deref_mut(),
                narrations.as_deref_mut(),
            );
        }
        if has_meditation_mat {
            emit_furniture_aura(
                FurnitureAura::Meditation,
                entity,
                position,
                status_effects,
                username,
                clock.tick,
                status_events,
                vfx_events.as_deref_mut(),
                audio_events.as_deref_mut(),
                narrations.as_deref_mut(),
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_furniture_aura(
    aura: FurnitureAura,
    entity: Entity,
    position: &Position,
    status_effects: &StatusEffects,
    username: &Username,
    issued_at_tick: u64,
    status_events: &mut Events<ApplyStatusEffectIntent>,
    vfx_events: Option<&mut Events<VfxEventRequest>>,
    audio_events: Option<&mut Events<PlaySoundRecipeRequest>>,
    narrations: Option<&mut PendingGameplayNarrations>,
) {
    let status_kind = aura.status_kind();
    let was_active = has_active_status(status_effects, status_kind.clone());
    status_events.send(ApplyStatusEffectIntent {
        target: entity,
        kind: status_kind,
        magnitude: aura.magnitude(),
        duration_ticks: FURNITURE_AURA_DURATION_TICKS,
        issued_at_tick,
    });

    if was_active {
        return;
    }

    let origin = position.get();
    if let Some(vfx_events) = vfx_events {
        gameplay_vfx::send_spawn(
            vfx_events,
            gameplay_vfx::spawn_request(
                aura.vfx_event_id(),
                origin,
                None,
                aura.color(),
                0.75,
                aura.particle_count(),
                aura.duration_ticks(),
            ),
        );
    }
    if let Some(audio_events) = audio_events {
        audio_events.send(PlaySoundRecipeRequest {
            recipe_id: FURNITURE_AURA_AUDIO_RECIPE_ID.to_string(),
            instance_id: 0,
            pos: Some([
                origin.x.floor() as i32,
                origin.y.floor() as i32,
                origin.z.floor() as i32,
            ]),
            flag: None,
            volume_mul: 1.0,
            pitch_shift: 0.0,
            recipient: AudioRecipient::Single(entity),
        });
    }
    if let Some(narrations) = narrations {
        narrations.push_player(
            username.0.as_str(),
            aura.narration(),
            NarrationStyle::Perception,
        );
    }
}

fn block_pos_from_position(position: &Position) -> [i32; 3] {
    let pos = position.get();
    [
        pos.x.floor() as i32,
        pos.y.floor() as i32,
        pos.z.floor() as i32,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    use valence::prelude::{App, Entity, Events, IntoSystemConfigs, Position, Update};
    use valence::testing::create_mock_client;

    use crate::combat::components::{
        ActiveStatusEffect, Stamina, StatusEffects, STATUS_EFFECT_TICK_INTERVAL_TICKS,
    };
    use crate::combat::status::{status_effect_apply_tick, status_effect_tick};
    use crate::combat::CombatClock;
    use crate::cultivation::tick::cultivation_acceleration_multiplier;
    use crate::network::audio_event_emit::PlaySoundRecipeRequest;
    use crate::network::vfx_event_emit::VfxEventRequest;
    use crate::player::gameplay::PendingGameplayNarrations;
    use crate::schema::vfx_event::VfxEventPayloadV1;

    #[test]
    fn template_id_mapping_pins_all_furniture_variants() {
        for kind in FurnitureKind::ALL {
            assert_eq!(
                furniture_kind_for_template_id(kind.template_id()),
                Some(kind),
                "template_id `{}` must map to {kind:?}",
                kind.template_id()
            );
        }
        assert_eq!(furniture_kind_for_template_id("torch_item"), None);
        assert_eq!(furniture_kind_for_template_id("missing_furniture"), None);
    }

    #[test]
    fn block_state_mapping_pins_all_furniture_variants() {
        for kind in FurnitureKind::ALL {
            assert_eq!(
                furniture_kind_for_block_state(kind.block_state()),
                Some(kind),
                "block state {:?} must map to {kind:?}",
                kind.block_state()
            );
        }
        assert_eq!(furniture_kind_for_block_state(BlockState::DIRT), None);
        assert_eq!(
            furniture_kind_for_block_state(BlockState::BONG_ZHENFA_NODE),
            None
        );
    }

    #[test]
    fn register_query_and_remove_are_consistent() {
        let mut registry = FurnitureRegistry::default();
        let pos = [10, 64, -3];

        assert_eq!(registry.register(pos, FurnitureKind::SimpleBed), None);
        assert_eq!(registry.kind_at(pos), Some(FurnitureKind::SimpleBed));
        assert_eq!(
            registry.kinds_in_range([11, 64, -4], 1).collect::<Vec<_>>(),
            vec![(pos, FurnitureKind::SimpleBed)]
        );
        assert_eq!(registry.remove(pos), Some(FurnitureKind::SimpleBed));
        assert_eq!(registry.kind_at(pos), None);
        assert!(registry.kinds_in_range([11, 64, -4], 1).next().is_none());
    }

    #[test]
    fn same_position_register_replaces_then_can_re_register() {
        let mut registry = FurnitureRegistry::default();
        let pos = [0, 65, 0];

        assert_eq!(registry.register(pos, FurnitureKind::SimpleBed), None);
        assert_eq!(
            registry.register(pos, FurnitureKind::MeditationMat),
            Some(FurnitureKind::SimpleBed)
        );
        assert_eq!(registry.kind_at(pos), Some(FurnitureKind::MeditationMat));
        assert_eq!(registry.remove(pos), Some(FurnitureKind::MeditationMat));
        assert_eq!(registry.register(pos, FurnitureKind::MoistureBase), None);
        assert_eq!(registry.kind_at(pos), Some(FurnitureKind::MoistureBase));
    }

    #[test]
    fn range_query_uses_chebyshev_boundary() {
        let mut registry = FurnitureRegistry::default();
        registry.register([2, 64, 0], FurnitureKind::SimpleBed);
        registry.register([3, 64, 0], FurnitureKind::MeditationMat);
        registry.register([0, 67, 0], FurnitureKind::MoistureBase);
        registry.register([0, 64, -2], FurnitureKind::SpiritStoneRack);

        let mut hits = registry
            .kinds_in_range([0, 64, 0], 2)
            .map(|(_, kind)| kind)
            .collect::<Vec<_>>();
        hits.sort_by_key(|kind| *kind as u8);

        assert_eq!(
            hits,
            vec![FurnitureKind::SimpleBed, FurnitureKind::SpiritStoneRack],
            "radius=2 should include boundary distance 2 and exclude distance 3"
        );
        assert!(registry.kinds_in_range([0, 64, 0], -1).next().is_none());
    }

    #[test]
    fn rebuild_from_blocks_keeps_only_furniture_states() {
        let mut registry = FurnitureRegistry::default();
        registry.register([99, 1, 99], FurnitureKind::SimpleBed);

        registry.rebuild_from_blocks([
            ([1, 64, 1], BlockState::BONG_SIMPLE_BED),
            ([2, 64, 1], BlockState::DIRT),
            ([3, 64, 1], BlockState::BONG_ZHENFA_NODE),
            ([4, 64, 1], BlockState::BONG_MEDITATION_MAT),
        ]);

        assert_eq!(registry.len(), 2);
        assert_eq!(registry.kind_at([1, 64, 1]), Some(FurnitureKind::SimpleBed));
        assert_eq!(
            registry.kind_at([4, 64, 1]),
            Some(FurnitureKind::MeditationMat)
        );
        assert_eq!(registry.kind_at([99, 1, 99]), None);
    }

    fn aura_app() -> App {
        let mut app = App::new();
        app.insert_resource(CombatClock {
            tick: FURNITURE_AURA_TICK_INTERVAL_TICKS,
        });
        app.insert_resource(FurnitureRegistry::default());
        app.insert_resource(PendingGameplayNarrations::default());
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(
            Update,
            (
                furniture_aura_tick,
                status_effect_apply_tick,
                status_effect_tick,
            )
                .chain(),
        );
        app
    }

    fn spawn_aura_player(app: &mut App, name: &str, pos: [f64; 3]) -> Entity {
        let (mut client_bundle, _helper) = create_mock_client(name);
        client_bundle.player.position = Position::new(pos);
        app.world_mut()
            .spawn((client_bundle, Stamina::default(), StatusEffects::default()))
            .id()
    }

    fn set_clock(app: &mut App, tick: u64) {
        app.world_mut().resource_mut::<CombatClock>().tick = tick;
    }

    fn status_intents(app: &App) -> Vec<ApplyStatusEffectIntent> {
        app.world()
            .resource::<Events<ApplyStatusEffectIntent>>()
            .iter_current_update_events()
            .cloned()
            .collect()
    }

    fn vfx_event_ids(app: &App) -> Vec<String> {
        app.world()
            .resource::<Events<VfxEventRequest>>()
            .iter_current_update_events()
            .filter_map(|event| match &event.payload {
                VfxEventPayloadV1::SpawnParticle { event_id, .. } => Some(event_id.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn bed_aura_hits_chebyshev_radius_two_and_excludes_three() {
        let mut app = aura_app();
        app.world_mut()
            .resource_mut::<FurnitureRegistry>()
            .register([2, 64, 0], FurnitureKind::SimpleBed);
        let inside = spawn_aura_player(&mut app, "Inside", [0.5, 64.0, 0.5]);
        let outside = spawn_aura_player(&mut app, "Outside", [5.0, 64.0, 0.0]);

        app.update();

        let intents = status_intents(&app);
        assert_eq!(
            intents.len(),
            1,
            "only the player within Chebyshev radius 2 should receive bed aura; got {intents:?}"
        );
        assert_eq!(intents[0].target, inside);
        assert_ne!(intents[0].target, outside);
        assert_eq!(intents[0].kind, StatusEffectKind::HealthRegenBoost);
        assert!((intents[0].magnitude - BED_HEALTH_REGEN_MAGNITUDE).abs() < f32::EPSILON);
    }

    #[test]
    fn aura_deduplicates_two_beds_without_magnitude_stacking() {
        let mut app = aura_app();
        {
            let mut registry = app.world_mut().resource_mut::<FurnitureRegistry>();
            registry.register([1, 64, 0], FurnitureKind::SimpleBed);
            registry.register([0, 64, 1], FurnitureKind::SimpleBed);
        }
        let player = spawn_aura_player(&mut app, "Resting", [0.5, 64.0, 0.5]);

        app.update();

        let status_effects = app.world().get::<StatusEffects>(player).unwrap();
        let active = status_effects
            .active
            .iter()
            .filter(|effect| effect.kind == StatusEffectKind::HealthRegenBoost)
            .collect::<Vec<_>>();
        assert_eq!(
            active.len(),
            1,
            "two beds in range must still produce one HealthRegenBoost status, got {:?}",
            status_effects.active
        );
        assert!(
            (active[0].magnitude - BED_HEALTH_REGEN_MAGNITUDE).abs() < f32::EPSILON,
            "bed aura magnitude must stay at +50%, not stack with duplicate beds"
        );
        assert_eq!(
            status_intents(&app)
                .iter()
                .filter(|intent| intent.kind == StatusEffectKind::HealthRegenBoost)
                .count(),
            1,
            "aura side must dedupe same-kind furniture before status.rs upsert"
        );
    }

    #[test]
    fn aura_requires_idle_stamina_state() {
        let mut app = aura_app();
        app.world_mut()
            .resource_mut::<FurnitureRegistry>()
            .register([0, 64, 0], FurnitureKind::SimpleBed);
        let player = spawn_aura_player(&mut app, "Runner", [0.5, 64.0, 0.5]);
        app.world_mut().get_mut::<Stamina>(player).unwrap().state = StaminaState::Jogging;

        app.update();

        assert!(
            status_intents(&app).is_empty(),
            "non-Idle players must not receive furniture aura renewal"
        );
        assert!(app
            .world()
            .get::<StatusEffects>(player)
            .unwrap()
            .active
            .is_empty());
    }

    #[test]
    fn moving_player_does_not_renew_and_existing_buff_expires() {
        let mut app = aura_app();
        app.world_mut()
            .resource_mut::<FurnitureRegistry>()
            .register([0, 64, 0], FurnitureKind::SimpleBed);
        let player = spawn_aura_player(&mut app, "Moving", [0.5, 64.0, 0.5]);
        app.world_mut().entity_mut(player).insert(StatusEffects {
            active: vec![ActiveStatusEffect {
                kind: StatusEffectKind::HealthRegenBoost,
                magnitude: BED_HEALTH_REGEN_MAGNITUDE,
                remaining_ticks: STATUS_EFFECT_TICK_INTERVAL_TICKS,
                source_pill: None,
            }],
        });
        app.world_mut().get_mut::<Stamina>(player).unwrap().state = StaminaState::Walking;
        set_clock(&mut app, FURNITURE_AURA_TICK_INTERVAL_TICKS);

        app.update();

        assert!(
            status_intents(&app).is_empty(),
            "moving player should receive no refresh intent, so the short existing buff can expire"
        );
        assert!(
            app.world()
                .get::<StatusEffects>(player)
                .unwrap()
                .active
                .is_empty(),
            "existing furniture buff must expire naturally once player leaves Idle state"
        );
    }

    #[test]
    fn active_status_refreshes_without_replaying_feedback() {
        let mut app = aura_app();
        app.world_mut()
            .resource_mut::<FurnitureRegistry>()
            .register([0, 64, 0], FurnitureKind::SimpleBed);
        let player = spawn_aura_player(&mut app, "QuietRefresh", [0.5, 64.0, 0.5]);
        app.world_mut().entity_mut(player).insert(StatusEffects {
            active: vec![ActiveStatusEffect {
                kind: StatusEffectKind::HealthRegenBoost,
                magnitude: BED_HEALTH_REGEN_MAGNITUDE,
                remaining_ticks: STATUS_EFFECT_TICK_INTERVAL_TICKS,
                source_pill: None,
            }],
        });

        app.update();

        let active = &app.world().get::<StatusEffects>(player).unwrap().active;
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].kind, StatusEffectKind::HealthRegenBoost);
        assert!(
            active[0].remaining_ticks
                >= FURNITURE_AURA_DURATION_TICKS - STATUS_EFFECT_TICK_INTERVAL_TICKS,
            "active bed aura should refresh duration without stacking; got {:?}",
            active
        );
        assert!(
            vfx_event_ids(&app).is_empty(),
            "refresh ticks must not replay one-shot VFX"
        );
        assert!(
            app.world()
                .resource::<Events<PlaySoundRecipeRequest>>()
                .iter_current_update_events()
                .next()
                .is_none(),
            "refresh ticks must not replay one-shot audio"
        );
        assert!(
            app.world_mut()
                .resource_mut::<PendingGameplayNarrations>()
                .drain()
                .is_empty(),
            "refresh ticks must not replay narration"
        );
    }

    #[test]
    fn bed_and_meditation_emit_distinct_status_vfx_audio_and_narration_on_first_apply() {
        let mut app = aura_app();
        {
            let mut registry = app.world_mut().resource_mut::<FurnitureRegistry>();
            registry.register([0, 64, 0], FurnitureKind::SimpleBed);
            registry.register([1, 64, 0], FurnitureKind::MeditationMat);
        }
        let player = spawn_aura_player(&mut app, "RestAndSit", [0.5, 64.0, 0.5]);

        app.update();

        let intents = status_intents(&app);
        assert_eq!(intents.len(), 2);
        assert!(intents.iter().any(|intent| intent.target == player
            && intent.kind == StatusEffectKind::HealthRegenBoost
            && (intent.magnitude - BED_HEALTH_REGEN_MAGNITUDE).abs() < f32::EPSILON));
        assert!(intents.iter().any(|intent| intent.target == player
            && intent.kind == StatusEffectKind::CultivationAcceleration
            && (intent.magnitude - MEDITATION_CULTIVATION_MAGNITUDE).abs() < f32::EPSILON));

        let status_effects = app.world().get::<StatusEffects>(player).unwrap();
        assert!(
            (cultivation_acceleration_multiplier(status_effects) - 1.2).abs() < 0.000_001,
            "meditation mat must feed existing CultivationAcceleration multiplier, got {:?}",
            status_effects.active
        );

        let mut vfx_ids = vfx_event_ids(&app);
        vfx_ids.sort();
        assert_eq!(
            vfx_ids,
            vec![
                BED_REST_VFX_EVENT_ID.to_string(),
                MEDITATION_VFX_EVENT_ID.to_string()
            ]
        );

        let audio = app
            .world()
            .resource::<Events<PlaySoundRecipeRequest>>()
            .iter_current_update_events()
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(audio.len(), 2);
        assert!(audio.iter().all(|event| {
            event.recipe_id == FURNITURE_AURA_AUDIO_RECIPE_ID
                && event.recipient == AudioRecipient::Single(player)
        }));

        let narrations = app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();
        assert_eq!(narrations.len(), 2);
        assert!(narrations
            .iter()
            .any(|narration| narration.text == BED_REST_NARRATION));
        assert!(narrations
            .iter()
            .any(|narration| narration.text == MEDITATION_NARRATION));
    }

    #[test]
    fn moisture_base_and_spirit_stone_rack_are_decorative_for_aura() {
        let mut app = aura_app();
        {
            let mut registry = app.world_mut().resource_mut::<FurnitureRegistry>();
            registry.register([0, 64, 0], FurnitureKind::MoistureBase);
            registry.register([1, 64, 0], FurnitureKind::SpiritStoneRack);
        }
        let _player = spawn_aura_player(&mut app, "NoAura", [0.5, 64.0, 0.5]);

        app.update();

        assert!(
            status_intents(&app).is_empty(),
            "moisture_base and spirit_stone_rack must not emit bed/mat aura intents"
        );
        assert!(
            vfx_event_ids(&app).is_empty(),
            "decorative furniture must not emit aura VFX"
        );
    }
}
