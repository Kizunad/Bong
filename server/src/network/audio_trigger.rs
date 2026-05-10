//! Domain event adapters for audio v1 SoundRecipe triggers.

use std::collections::HashMap;

use valence::prelude::{
    Client, DVec3, Entity, EventReader, EventWriter, Position, Query, ResMut, Resource, With,
};

use crate::alchemy::{AlchemyOutcomeEvent, ResolvedOutcome};
use crate::botany::components::HarvestTerminalEvent;
use crate::combat::components::{Lifecycle, Wounds};
use crate::combat::events::{CombatEvent, DeathEvent};
use crate::combat::tuike_v2::{ContamTransferredEvent, DonFalseSkinEvent, FalseSkinSheddedEvent};
use crate::combat::woliu_v2::VortexCastEvent;
use crate::cultivation::breakthrough::BreakthroughOutcome;
use crate::cultivation::components::Cultivation;
use crate::cultivation::life_record::LifeRecord;
use crate::cultivation::overload::MeridianOverloadEvent;
use crate::cultivation::possession::DuoSheWarningEvent;
use crate::cultivation::qi_zero_decay::RealmRegressed;
use crate::cultivation::tribulation::{
    JueBiTriggeredEvent, TribulationAnnounce, TribulationFailed, TribulationKind, TribulationState,
    TribulationWaveCleared,
};
use crate::forge::blueprint::TemperBeat;
use crate::forge::events::{ForgeBucket, ForgeOutcomeEvent, ForgeStartAccepted, TemperingHit};
use crate::forge::session::{ForgeSessions, ForgeStep};
use crate::lingtian::events::{
    DrainQiCompleted, HarvestCompleted, PlantingCompleted, ReplenishCompleted, TillCompleted,
};
use crate::network::audio_event_emit::{
    recipient_for_attenuation, AudioRecipient, PlaySoundRecipeRequest, AUDIO_BROADCAST_RADIUS,
};
use crate::npc::brain::canonical_npc_id;
use crate::npc::spawn::NpcMarker;
use crate::schema::audio::AudioAttenuation;
use crate::skill::events::{SkillLvUp, SkillScrollUsed, SkillXpGain, XpGainSource};

#[derive(Debug, Default)]
pub struct AudioTriggerState {
    low_hp: HashMap<Entity, bool>,
    low_qi: HashMap<Entity, bool>,
}

impl Resource for AudioTriggerState {}

type PlayerAudioStateItem<'a> = (
    Entity,
    &'a Position,
    Option<&'a Wounds>,
    Option<&'a Cultivation>,
);
type PlayerAudioStateFilter = With<Client>;

pub fn emit_player_state_audio_triggers(
    mut state: ResMut<AudioTriggerState>,
    players: Query<PlayerAudioStateItem<'_>, PlayerAudioStateFilter>,
    mut audio: EventWriter<PlaySoundRecipeRequest>,
) {
    for (entity, position, wounds, cultivation) in &players {
        if let Some(wounds) = wounds {
            let hp_ratio = wounds.health_current / wounds.health_max.max(1.0);
            let low_hp = hp_ratio < 0.3;
            if low_hp && !state.low_hp.get(&entity).copied().unwrap_or(false) {
                emit_play(
                    &mut audio,
                    "heartbeat_low_hp",
                    entity,
                    position.get(),
                    Some("hp_below_30".to_string()),
                    1.0,
                    0.0,
                );
            }
            state.low_hp.insert(entity, low_hp);
        }

        if let Some(cultivation) = cultivation {
            let qi_ratio = (cultivation.qi_current / cultivation.qi_max.max(1.0)) as f32;
            let low_qi = qi_ratio <= 0.05;
            if low_qi && !state.low_qi.get(&entity).copied().unwrap_or(false) {
                emit_play(
                    &mut audio,
                    "qi_depleted_warning",
                    entity,
                    position.get(),
                    None,
                    1.0,
                    0.0,
                );
            }
            state.low_qi.insert(entity, low_qi);
        }
    }
}

pub fn emit_combat_audio_triggers(
    mut combat_events: EventReader<CombatEvent>,
    positions: Query<&Position>,
    npc_markers: Query<(), With<NpcMarker>>,
    mut audio: EventWriter<PlaySoundRecipeRequest>,
) {
    for event in combat_events.read() {
        let Ok(position) = positions.get(event.target) else {
            continue;
        };
        let origin = position.get();
        let recipe_id = if event.description.contains("jiemai=true") {
            "parry_clang"
        } else if event.damage >= 0.5 {
            "meridian_crack"
        } else if npc_markers.get(event.target).is_ok() && event.damage > 0.0 {
            "npc_hurt"
        } else if npc_markers.get(event.attacker).is_ok() && event.damage > 0.0 {
            "npc_aggro"
        } else {
            continue;
        };
        emit_play(&mut audio, recipe_id, event.target, origin, None, 1.0, 0.0);
    }
}

pub fn emit_npc_death_audio_triggers(
    mut death_events: EventReader<DeathEvent>,
    positions: Query<&Position>,
    npc_markers: Query<(), With<NpcMarker>>,
    mut audio: EventWriter<PlaySoundRecipeRequest>,
) {
    for event in death_events.read() {
        if npc_markers.get(event.target).is_err() {
            continue;
        }
        let Ok(position) = positions.get(event.target) else {
            continue;
        };
        emit_play(
            &mut audio,
            "npc_death",
            event.target,
            position.get(),
            None,
            1.0,
            0.0,
        );
    }
}

pub fn emit_cultivation_audio_triggers(
    mut breakthroughs: EventReader<BreakthroughOutcome>,
    mut regressions: EventReader<RealmRegressed>,
    mut overloads: EventReader<MeridianOverloadEvent>,
    positions: Query<&Position>,
    mut audio: EventWriter<PlaySoundRecipeRequest>,
) {
    for event in breakthroughs.read() {
        let Ok(position) = positions.get(event.entity) else {
            continue;
        };
        let origin = position.get();
        let recipe_id = if event.result.is_ok() {
            "realm_breakthrough"
        } else {
            "meridian_crack"
        };
        emit_play(&mut audio, recipe_id, event.entity, origin, None, 1.0, 0.0);
    }

    for event in regressions.read() {
        let Ok(position) = positions.get(event.entity) else {
            continue;
        };
        emit_play(
            &mut audio,
            "realm_regression",
            event.entity,
            position.get(),
            None,
            1.0,
            0.0,
        );
    }

    for event in overloads.read() {
        let Ok(position) = positions.get(event.entity) else {
            continue;
        };
        emit_play(
            &mut audio,
            "meridian_crack",
            event.entity,
            position.get(),
            None,
            severity_volume(event.severity),
            0.0,
        );
    }
}

pub fn emit_tribulation_audio_triggers(
    mut announces: EventReader<TribulationAnnounce>,
    mut juebi_triggered: EventReader<JueBiTriggeredEvent>,
    mut waves: EventReader<TribulationWaveCleared>,
    mut failures: EventReader<TribulationFailed>,
    positions: Query<&Position>,
    states: Query<&TribulationState>,
    mut audio: EventWriter<PlaySoundRecipeRequest>,
) {
    for event in announces.read() {
        let Ok(position) = positions.get(event.entity) else {
            continue;
        };
        emit_play(
            &mut audio,
            "tribulation_thunder_distant",
            event.entity,
            position.get(),
            None,
            1.0,
            0.0,
        );
    }

    for event in juebi_triggered.read() {
        let Ok(position) = positions.get(event.entity) else {
            continue;
        };
        emit_play(
            &mut audio,
            "ground_crack_rumble",
            event.entity,
            position.get(),
            None,
            1.0,
            0.0,
        );
    }

    for event in waves.read() {
        let Ok(position) = positions.get(event.entity) else {
            continue;
        };
        let recipe = if states
            .get(event.entity)
            .is_ok_and(|state| state.kind == TribulationKind::JueBi)
        {
            match event.wave {
                1 => "pressure_collapse_whoosh",
                2 => "ground_crack_rumble",
                _ => "pillar_eruption_boom",
            }
        } else {
            "tribulation_wave_impact"
        };
        emit_play(
            &mut audio,
            recipe,
            event.entity,
            position.get(),
            None,
            1.0,
            0.0,
        );
    }

    for event in failures.read() {
        let Ok(position) = positions.get(event.entity) else {
            continue;
        };
        emit_play(
            &mut audio,
            "realm_regression",
            event.entity,
            position.get(),
            None,
            1.0,
            0.0,
        );
    }
}

pub fn emit_alchemy_audio_triggers(
    mut outcomes: EventReader<AlchemyOutcomeEvent>,
    positions: Query<&Position>,
    mut audio: EventWriter<PlaySoundRecipeRequest>,
) {
    for event in outcomes.read() {
        let origin = positions
            .get(event.furnace)
            .map(|position| position.get())
            .unwrap_or(DVec3::ZERO);
        let (recipe_id, volume_mul) = match event.outcome {
            ResolvedOutcome::Pill { .. } => ("pill_consume", 0.75),
            ResolvedOutcome::Explode { .. } => ("furnace_boom", 1.0),
            ResolvedOutcome::Waste { .. } | ResolvedOutcome::Mismatch => continue,
        };
        emit_play(
            &mut audio,
            recipe_id,
            event.furnace,
            origin,
            None,
            volume_mul,
            0.0,
        );
    }
}

pub fn emit_forge_audio_triggers(
    mut starts: EventReader<ForgeStartAccepted>,
    mut hits: EventReader<TemperingHit>,
    mut outcomes: EventReader<ForgeOutcomeEvent>,
    sessions: Option<valence::prelude::Res<ForgeSessions>>,
    positions: Query<&Position>,
    mut audio: EventWriter<PlaySoundRecipeRequest>,
) {
    for event in starts.read() {
        if let Ok(position) = positions.get(event.station) {
            emit_play(
                &mut audio,
                "stance_switch",
                event.caster,
                position.get(),
                None,
                0.7,
                0.0,
            );
        }
    }

    for event in hits.read() {
        let Some(sessions) = sessions.as_deref() else {
            continue;
        };
        let Some(session) = sessions.get(event.session) else {
            continue;
        };
        if session.current_step != ForgeStep::Tempering {
            continue;
        }
        let recipe_id = match event.beat {
            TemperBeat::Light => "hammer_strike_light",
            TemperBeat::Heavy => "hammer_strike_heavy",
            TemperBeat::Fold => "hammer_strike_fold",
        };
        let origin = positions
            .get(session.station)
            .map(|position| position.get())
            .or_else(|_| positions.get(session.caster).map(|position| position.get()))
            .unwrap_or(DVec3::ZERO);
        emit_play(
            &mut audio,
            recipe_id,
            session.caster,
            origin,
            None,
            1.0,
            0.0,
        );
    }

    for event in outcomes.read() {
        let recipe_id = match event.bucket {
            ForgeBucket::Explode => "furnace_boom",
            ForgeBucket::Perfect | ForgeBucket::Good | ForgeBucket::Flawed => "skill_lv_up",
            ForgeBucket::Waste => continue,
        };
        let Ok(position) = positions.get(event.caster) else {
            continue;
        };
        emit_play(
            &mut audio,
            recipe_id,
            event.caster,
            position.get(),
            None,
            0.8,
            0.0,
        );
    }
}

pub fn emit_botany_audio_triggers(
    mut terminal: EventReader<HarvestTerminalEvent>,
    positions: Query<&Position>,
    mut audio: EventWriter<PlaySoundRecipeRequest>,
) {
    for event in terminal.read() {
        if !event.completed || event.interrupted {
            continue;
        }
        let origin = event
            .target_pos
            .map(|pos| DVec3::new(pos[0], pos[1], pos[2]))
            .or_else(|| positions.get(event.client_entity).ok().map(|p| p.get()))
            .unwrap_or(DVec3::ZERO);
        emit_play(
            &mut audio,
            "harvest_pluck",
            event.client_entity,
            origin,
            None,
            1.0,
            0.0,
        );
    }
}

pub fn emit_lingtian_audio_triggers(
    mut tills: EventReader<TillCompleted>,
    mut plantings: EventReader<PlantingCompleted>,
    mut harvests: EventReader<HarvestCompleted>,
    mut replenishes: EventReader<ReplenishCompleted>,
    mut drains: EventReader<DrainQiCompleted>,
    mut audio: EventWriter<PlaySoundRecipeRequest>,
) {
    for event in tills.read() {
        emit_play_at_block(&mut audio, "till_soil", event.player, event.pos, 1.0);
    }
    for event in plantings.read() {
        emit_play_at_block(
            &mut audio,
            "lingtian_plant_seed",
            event.player,
            event.pos,
            0.9,
        );
    }
    for event in harvests.read() {
        emit_play_at_block(&mut audio, "harvest_pluck", event.player, event.pos, 1.0);
    }
    for event in replenishes.read() {
        emit_play_at_block(&mut audio, "plot_replenish", event.player, event.pos, 1.0);
    }
    for event in drains.read() {
        emit_play_at_block(&mut audio, "lingtian_drain", event.player, event.pos, 0.85);
    }
}

pub fn emit_woliu_v2_audio_triggers(
    mut casts: EventReader<VortexCastEvent>,
    positions: Query<&Position>,
    mut audio: EventWriter<PlaySoundRecipeRequest>,
) {
    for event in casts.read() {
        let origin = positions
            .get(event.caster)
            .map(|position| position.get())
            .unwrap_or(event.center);
        emit_play(
            &mut audio,
            event.visual.sound_recipe_id,
            event.caster,
            origin,
            Some(event.skill.as_str().to_string()),
            1.0,
            0.0,
        );
    }
}

pub fn emit_tuike_v2_audio_triggers(
    mut don_events: EventReader<DonFalseSkinEvent>,
    mut shed_events: EventReader<FalseSkinSheddedEvent>,
    mut transfer_events: EventReader<ContamTransferredEvent>,
    positions: Query<&Position>,
    mut audio: EventWriter<PlaySoundRecipeRequest>,
) {
    for event in don_events.read() {
        let origin = positions
            .get(event.caster)
            .map(|position| position.get())
            .unwrap_or(DVec3::ZERO);
        emit_play(
            &mut audio,
            event.visual.sound_recipe_id.as_str(),
            event.caster,
            origin,
            Some("tuike_don".to_string()),
            1.0,
            0.0,
        );
    }
    for event in shed_events.read() {
        let origin = positions
            .get(event.owner)
            .map(|position| position.get())
            .unwrap_or(DVec3::ZERO);
        emit_play(
            &mut audio,
            event.visual.sound_recipe_id.as_str(),
            event.owner,
            origin,
            Some("tuike_shed".to_string()),
            1.0,
            if event.permanent_taint_load > 0.0 {
                0.08
            } else {
                0.0
            },
        );
    }
    for event in transfer_events.read() {
        let origin = positions
            .get(event.caster)
            .map(|position| position.get())
            .unwrap_or(DVec3::ZERO);
        emit_play(
            &mut audio,
            event.visual.sound_recipe_id.as_str(),
            event.caster,
            origin,
            Some("tuike_transfer_taint".to_string()),
            1.0,
            if event.permanent_absorbed > 0.0 {
                0.12
            } else {
                0.0
            },
        );
    }
}

pub fn emit_skill_audio_triggers(
    mut xp: EventReader<SkillXpGain>,
    mut lv_up: EventReader<SkillLvUp>,
    mut scrolls: EventReader<SkillScrollUsed>,
    positions: Query<&Position>,
    mut audio: EventWriter<PlaySoundRecipeRequest>,
) {
    for event in xp.read() {
        if !matches!(
            &event.source,
            XpGainSource::Action {
                plan_id: "combat" | "cultivation",
                ..
            }
        ) {
            continue;
        }
        let Ok(position) = positions.get(event.char_entity) else {
            continue;
        };
        emit_play(
            &mut audio,
            "stance_switch",
            event.char_entity,
            position.get(),
            None,
            0.7,
            0.0,
        );
    }

    for event in lv_up.read() {
        let Ok(position) = positions.get(event.char_entity) else {
            continue;
        };
        emit_play(
            &mut audio,
            "skill_lv_up",
            event.char_entity,
            position.get(),
            None,
            1.0,
            0.0,
        );
    }

    for event in scrolls.read() {
        if event.was_duplicate {
            continue;
        }
        let Ok(position) = positions.get(event.char_entity) else {
            continue;
        };
        emit_play(
            &mut audio,
            "exposure_name",
            event.char_entity,
            position.get(),
            None,
            0.8,
            0.0,
        );
    }
}

pub fn emit_social_audio_triggers(
    mut duo_she_warnings: EventReader<DuoSheWarningEvent>,
    targets: Query<(Entity, &Position, Option<&LifeRecord>, Option<&Lifecycle>)>,
    mut audio: EventWriter<PlaySoundRecipeRequest>,
) {
    for warning in duo_she_warnings.read() {
        let Some((entity, position)) = resolve_audio_target(warning.target_id.as_str(), &targets)
        else {
            continue;
        };
        emit_play(
            &mut audio,
            "exposure_name",
            entity,
            position,
            None,
            1.0,
            0.0,
        );
    }
}

pub fn emit_player_local_audio(
    audio: &mut EventWriter<PlaySoundRecipeRequest>,
    recipe_id: impl Into<String>,
    entity: Entity,
    origin: DVec3,
    flag: Option<String>,
    volume_mul: f32,
) {
    audio.send(PlaySoundRecipeRequest {
        recipe_id: recipe_id.into(),
        instance_id: 0,
        pos: Some(block_pos(origin)),
        flag,
        volume_mul,
        pitch_shift: 0.0,
        recipient: AudioRecipient::Single(entity),
    });
}

pub fn emit_recipe_audio(
    audio: &mut EventWriter<PlaySoundRecipeRequest>,
    recipe_id: impl Into<String>,
    entity: Entity,
    origin: DVec3,
    flag: Option<String>,
    volume_mul: f32,
) {
    emit_play(audio, recipe_id, entity, origin, flag, volume_mul, 0.0);
}

fn emit_play(
    audio: &mut EventWriter<PlaySoundRecipeRequest>,
    recipe_id: impl Into<String>,
    entity: Entity,
    origin: DVec3,
    flag: Option<String>,
    volume_mul: f32,
    pitch_shift: f32,
) {
    let recipe_id = recipe_id.into();
    let attenuation = attenuation_for_recipe(&recipe_id);
    audio.send(PlaySoundRecipeRequest {
        recipe_id,
        instance_id: 0,
        pos: Some(block_pos(origin)),
        flag,
        volume_mul,
        pitch_shift,
        recipient: recipient_for_attenuation(attenuation, entity, origin),
    });
}

fn emit_play_at_block(
    audio: &mut EventWriter<PlaySoundRecipeRequest>,
    recipe_id: impl Into<String>,
    entity: Entity,
    pos: valence::prelude::BlockPos,
    volume_mul: f32,
) {
    let origin = DVec3::new(f64::from(pos.x), f64::from(pos.y), f64::from(pos.z));
    let recipe_id = recipe_id.into();
    audio.send(PlaySoundRecipeRequest {
        recipe_id: recipe_id.clone(),
        instance_id: 0,
        pos: Some([pos.x, pos.y, pos.z]),
        flag: None,
        volume_mul,
        pitch_shift: 0.0,
        recipient: recipient_for_attenuation(attenuation_for_recipe(&recipe_id), entity, origin),
    });
}

fn attenuation_for_recipe(recipe_id: &str) -> AudioAttenuation {
    match recipe_id {
        "heartbeat_low_hp"
        | "qi_depleted_warning"
        | "realm_regression"
        | "meridian_crack"
        | "pill_consume"
        | "niche_breach"
        | "exposure_name"
        | "narration_cue"
        | "skill_lv_up"
        | "npc_refuse"
        | "npc_greeting_cultivator"
        | "npc_greeting_commoner"
        | "stance_switch" => AudioAttenuation::PlayerLocal,
        "tribulation_thunder_distant" => AudioAttenuation::GlobalHint,
        "realm_breakthrough" => AudioAttenuation::ZoneBroadcast,
        _ => AudioAttenuation::World3d,
    }
}

fn block_pos(origin: DVec3) -> [i32; 3] {
    [
        origin.x.floor() as i32,
        origin.y.floor() as i32,
        origin.z.floor() as i32,
    ]
}

fn severity_volume(severity: f64) -> f32 {
    (0.6 + severity as f32).clamp(0.6, 1.5)
}

fn resolve_audio_target(
    target_id: &str,
    targets: &Query<(Entity, &Position, Option<&LifeRecord>, Option<&Lifecycle>)>,
) -> Option<(Entity, DVec3)> {
    let char_entity_bits = target_id
        .strip_prefix("char:")
        .and_then(|bits| bits.parse::<u64>().ok());

    targets
        .iter()
        .find(|(entity, _, life_record, lifecycle)| {
            char_entity_bits.is_some_and(|bits| entity.to_bits() == bits)
                || life_record.is_some_and(|record| record.character_id == target_id)
                || lifecycle.is_some_and(|lifecycle| lifecycle.character_id == target_id)
                || canonical_npc_id(*entity) == target_id
        })
        .map(|(entity, position, _, _)| (entity, position.get()))
}

#[allow(dead_code)]
fn nearby_recipient(origin: DVec3) -> AudioRecipient {
    AudioRecipient::Radius {
        origin,
        radius: AUDIO_BROADCAST_RADIUS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::components::{BodyPart, WoundKind};
    use crate::combat::events::{CombatEvent, DeathEvent};
    use valence::prelude::{App, Events, Update};

    #[test]
    fn jiemai_combat_event_emits_parry_recipe() {
        let mut app = App::new();
        app.add_event::<CombatEvent>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_combat_audio_triggers);
        let attacker = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
        let target = app.world_mut().spawn(Position::new([1.0, 64.0, 0.0])).id();
        app.world_mut().send_event(CombatEvent {
            attacker,
            target,
            resolved_at_tick: 1,
            body_part: BodyPart::Chest,
            wound_kind: WoundKind::Blunt,
            source: crate::combat::events::AttackSource::Melee,
            damage: 0.4,
            contam_delta: 0.0,
            description: "test jiemai=true".to_string(),
            defense_kind: None,
            defense_effectiveness: None,
            defense_contam_reduced: None,
            defense_wound_severity: None,
        });

        app.update();

        let emitted: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].recipe_id, "parry_clang");
    }

    #[test]
    fn npc_death_emits_audio() {
        let mut app = App::new();
        app.add_event::<DeathEvent>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_npc_death_audio_triggers);
        let npc = app
            .world_mut()
            .spawn((NpcMarker, Position::new([1.0, 64.0, 0.0])))
            .id();
        app.world_mut().send_event(DeathEvent {
            target: npc,
            cause: "test".to_string(),
            attacker: None,
            attacker_player_id: None,
            at_tick: 1,
        });

        app.update();

        let emitted: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].recipe_id, "npc_death");
    }

    #[test]
    fn skill_lv_up_emits_player_local_recipe() {
        let mut app = App::new();
        app.add_event::<SkillXpGain>();
        app.add_event::<SkillLvUp>();
        app.add_event::<SkillScrollUsed>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_skill_audio_triggers);
        let player = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
        app.world_mut().send_event(SkillLvUp {
            char_entity: player,
            skill: crate::skill::components::SkillId::Herbalism,
            new_lv: 2,
        });

        app.update();

        let emitted: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].recipe_id, "skill_lv_up");
        assert!(matches!(emitted[0].recipient, AudioRecipient::Single(entity) if entity == player));
    }

    #[test]
    fn lingtian_actions_emit_dedicated_recipes() {
        let mut app = App::new();
        app.add_event::<TillCompleted>();
        app.add_event::<PlantingCompleted>();
        app.add_event::<HarvestCompleted>();
        app.add_event::<ReplenishCompleted>();
        app.add_event::<DrainQiCompleted>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_lingtian_audio_triggers);
        let player = app.world_mut().spawn_empty().id();
        let pos = valence::prelude::BlockPos::new(3, 64, 5);

        app.world_mut().send_event(TillCompleted {
            player,
            pos,
            hoe: crate::lingtian::hoe::HoeKind::Iron,
            hoe_instance_id: 1,
        });
        app.world_mut().send_event(PlantingCompleted {
            player,
            pos,
            plant_id: "ci_she_hao".to_string(),
        });
        app.world_mut().send_event(HarvestCompleted {
            player,
            pos,
            plant_id: "ci_she_hao".to_string(),
            seed_dropped: false,
        });
        app.world_mut().send_event(ReplenishCompleted {
            player,
            pos,
            source: crate::lingtian::session::ReplenishSource::Zone,
            plot_qi_added: 0.2,
            overflow_to_zone: 0.0,
        });
        app.world_mut().send_event(DrainQiCompleted {
            player,
            pos,
            plot_qi_drained: 0.3,
            qi_to_player: 0.24,
            qi_to_zone: 0.06,
        });

        app.update();

        let recipes: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .map(|request| request.recipe_id)
            .collect();
        assert_eq!(
            recipes,
            vec![
                "till_soil",
                "lingtian_plant_seed",
                "harvest_pluck",
                "plot_replenish",
                "lingtian_drain"
            ]
        );
    }

    #[test]
    fn duo_she_warning_matches_life_record_target() {
        let mut app = App::new();
        app.add_event::<DuoSheWarningEvent>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_social_audio_triggers);
        let target = app
            .world_mut()
            .spawn((
                Position::new([3.0, 64.0, 3.0]),
                LifeRecord::new("offline:Target"),
            ))
            .id();
        app.world_mut().send_event(DuoSheWarningEvent {
            host_id: "offline:Host".to_string(),
            target_id: "offline:Target".to_string(),
            at_tick: 1,
        });

        app.update();

        let emitted: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].recipe_id, "exposure_name");
        assert!(matches!(emitted[0].recipient, AudioRecipient::Single(entity) if entity == target));
    }
}
