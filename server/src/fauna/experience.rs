//! Fauna Experience v1：spawn/death VFX 与生物音效触发。

use valence::prelude::{Added, DVec3, Entity, EventReader, EventWriter, Position, Query, With};

use crate::combat::events::{AttackIntent, DeathEvent};
use crate::combat::rat_bite::RatBiteEvent;
use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest};
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::npc::spawn::NpcMarker;
use crate::schema::vfx_event::VfxEventPayloadV1;

use super::components::{BeastKind, FaunaTag};
use super::visual::FaunaVisualKind;

type VisualNpcQuery<'w, 's> =
    Query<'w, 's, (Entity, &'static Position, &'static FaunaVisualKind), With<NpcMarker>>;
type AddedVisualQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Position, &'static FaunaVisualKind),
    (With<NpcMarker>, Added<FaunaVisualKind>),
>;

pub fn emit_fauna_spawn_vfx_system(
    visuals: AddedVisualQuery<'_, '_>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for (position, visual) in &visuals {
        let pos = position.get();
        vfx_events.send(spawn_particle(
            "bong:fauna_spawn_dust",
            pos,
            visual.event_color(),
            0.72,
            8,
            24,
        ));
        if matches!(visual, FaunaVisualKind::AshSpider) {
            vfx_events.send(spawn_particle(
                "bong:spider_shimmer",
                pos + DVec3::new(0.0, 0.35, 0.0),
                "#B8D0C8",
                0.45,
                10,
                28,
            ));
        }
    }
}

pub fn emit_fauna_spawn_ambient_audio_system(
    visuals: AddedVisualQuery<'_, '_>,
    mut audio_events: EventWriter<PlaySoundRecipeRequest>,
) {
    for (position, visual) in &visuals {
        let Some(recipe_id) = ambient_recipe_for_visual(*visual) else {
            continue;
        };
        audio_events.send(play_audio(recipe_id, position.get(), 1.0, 0.0));
    }
}

pub fn emit_fauna_attack_audio_system(
    mut attacks: EventReader<AttackIntent>,
    attackers: Query<(&Position, &FaunaTag), With<NpcMarker>>,
    mut audio_events: EventWriter<PlaySoundRecipeRequest>,
) {
    for attack in attacks.read() {
        let Ok((position, tag)) = attackers.get(attack.attacker) else {
            continue;
        };
        let Some(recipe_id) = attack_recipe_for_beast(tag.beast_kind) else {
            continue;
        };
        audio_events.send(play_audio(recipe_id, position.get(), 1.0, 0.0));
    }
}

pub fn emit_fauna_death_vfx_audio_system(
    mut deaths: EventReader<DeathEvent>,
    visuals: VisualNpcQuery<'_, '_>,
    tags: Query<&FaunaTag>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    mut audio_events: EventWriter<PlaySoundRecipeRequest>,
) {
    for death in deaths.read() {
        let Ok((entity, position, visual)) = visuals.get(death.target) else {
            continue;
        };
        let pos = position.get();
        vfx_events.send(spawn_particle(
            "bong:death_soul_dissipate",
            pos,
            visual.event_color(),
            0.85,
            18,
            38,
        ));
        vfx_events.send(spawn_particle(
            "bong:fauna_bone_shatter",
            pos,
            "#D8C8AA",
            0.75,
            6,
            18,
        ));

        if let Ok(tag) = tags.get(entity) {
            if let Some(recipe_id) = death_recipe_for_beast(tag.beast_kind) {
                audio_events.send(play_audio(recipe_id, pos, 1.0, 0.0));
            }
        }
    }
}

pub fn emit_rat_bite_audio_system(
    mut bites: EventReader<RatBiteEvent>,
    rats: Query<(&Position, &FaunaTag)>,
    mut audio_events: EventWriter<PlaySoundRecipeRequest>,
) {
    for bite in bites.read() {
        let Ok((position, tag)) = rats.get(bite.rat) else {
            continue;
        };
        if !matches!(tag.beast_kind, BeastKind::Rat) {
            continue;
        }
        audio_events.send(play_audio("fauna_rat_squeal", position.get(), 1.0, 0.0));
    }
}

pub fn play_audio(
    recipe_id: &str,
    pos: DVec3,
    volume_mul: f32,
    pitch_shift: f32,
) -> PlaySoundRecipeRequest {
    PlaySoundRecipeRequest {
        recipe_id: recipe_id.to_string(),
        instance_id: 0,
        pos: Some([
            pos.x.floor() as i32,
            pos.y.floor() as i32,
            pos.z.floor() as i32,
        ]),
        flag: None,
        volume_mul,
        pitch_shift,
        recipient: AudioRecipient::Radius {
            origin: pos,
            radius: 64.0,
        },
    }
}

pub fn spawn_particle(
    event_id: &str,
    pos: DVec3,
    color: &str,
    strength: f32,
    count: u16,
    duration_ticks: u16,
) -> VfxEventRequest {
    VfxEventRequest::new(
        pos,
        VfxEventPayloadV1::SpawnParticle {
            event_id: event_id.to_string(),
            origin: [pos.x, pos.y, pos.z],
            direction: None,
            color: Some(color.to_string()),
            strength: Some(strength),
            count: Some(count),
            duration_ticks: Some(duration_ticks),
        },
    )
}

pub const fn attack_recipe_for_beast(kind: BeastKind) -> Option<&'static str> {
    match kind {
        BeastKind::Spider => Some("fauna_ash_spider_attack"),
        BeastKind::HybridBeast => Some("fauna_hybrid_beast_attack"),
        BeastKind::VoidDistorted => Some("fauna_void_distorted_attack"),
        BeastKind::Rat | BeastKind::Whale => None,
    }
}

pub const fn death_recipe_for_beast(kind: BeastKind) -> Option<&'static str> {
    match kind {
        BeastKind::Rat => Some("fauna_rat_death"),
        BeastKind::Spider => Some("fauna_ash_spider_death"),
        BeastKind::HybridBeast => Some("fauna_hybrid_beast_death"),
        BeastKind::VoidDistorted => Some("fauna_void_distorted_death"),
        BeastKind::Whale => None,
    }
}

pub const fn ambient_recipe_for_visual(visual: FaunaVisualKind) -> Option<&'static str> {
    match visual {
        FaunaVisualKind::AshSpider => Some("fauna_ash_spider_ambient"),
        FaunaVisualKind::HybridBeast => Some("fauna_hybrid_beast_ambient"),
        FaunaVisualKind::VoidDistorted => Some("fauna_void_distorted_ambient"),
        FaunaVisualKind::DevourRat
        | FaunaVisualKind::Daoxiang
        | FaunaVisualKind::Zhinian
        | FaunaVisualKind::TsySentinel
        | FaunaVisualKind::Fuya => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::components::WoundKind;
    use crate::combat::events::AttackReach;
    use valence::prelude::{App, Events, Update};

    #[test]
    fn spawn_vfx_emits_dust_for_custom_fauna_visual() {
        let mut app = App::new();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_fauna_spawn_vfx_system);
        app.world_mut().spawn((
            NpcMarker,
            Position::new([1.0, 64.0, 2.0]),
            FaunaVisualKind::DevourRat,
        ));

        app.update();

        let events = app.world().resource::<Events<VfxEventRequest>>();
        let first = events
            .iter_current_update_events()
            .next()
            .expect("fauna visual spawn should emit dust VFX");
        assert!(matches!(
            &first.payload,
            VfxEventPayloadV1::SpawnParticle { event_id, .. } if event_id == "bong:fauna_spawn_dust"
        ));
    }

    #[test]
    fn rat_bite_audio_uses_rat_squeal_recipe() {
        let mut app = App::new();
        app.add_event::<RatBiteEvent>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_rat_bite_audio_system);
        let rat = app
            .world_mut()
            .spawn((
                Position::new([0.0, 64.0, 0.0]),
                FaunaTag::new(BeastKind::Rat),
            ))
            .id();
        let target = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(RatBiteEvent {
            rat,
            target,
            qi_steal: 1,
        });

        app.update();

        let events = app.world().resource::<Events<PlaySoundRecipeRequest>>();
        let event = events
            .iter_current_update_events()
            .next()
            .expect("rat bite should emit squeal audio");
        assert_eq!(event.recipe_id, "fauna_rat_squeal");
    }

    #[test]
    fn rat_bite_audio_skips_non_rat_entities() {
        let mut app = App::new();
        app.add_event::<RatBiteEvent>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_rat_bite_audio_system);
        let spider = app
            .world_mut()
            .spawn((
                Position::new([0.0, 64.0, 0.0]),
                FaunaTag::new(BeastKind::Spider),
            ))
            .id();
        let target = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(RatBiteEvent {
            rat: spider,
            target,
            qi_steal: 1,
        });

        app.update();

        let events = app.world().resource::<Events<PlaySoundRecipeRequest>>();
        assert_eq!(events.iter_current_update_events().count(), 0);
    }

    #[test]
    fn spider_attack_audio_uses_planned_attack_recipe() {
        let mut app = App::new();
        app.add_event::<AttackIntent>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_fauna_attack_audio_system);
        let spider = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 64.0, 0.0]),
                FaunaTag::new(BeastKind::Spider),
            ))
            .id();
        app.world_mut().send_event(AttackIntent {
            attacker: spider,
            target: None,
            issued_at_tick: 1,
            reach: AttackReach::new(0.9, 0.0),
            qi_invest: 0.0,
            wound_kind: WoundKind::Pierce,
            source: Default::default(),
            debug_command: None,
        });

        app.update();

        let events = app.world().resource::<Events<PlaySoundRecipeRequest>>();
        let event = events
            .iter_current_update_events()
            .next()
            .expect("spider attack should emit its fauna attack audio recipe");
        assert_eq!(event.recipe_id, "fauna_ash_spider_attack");
    }

    #[test]
    fn hybrid_spawn_audio_uses_planned_ambient_recipe() {
        let mut app = App::new();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_fauna_spawn_ambient_audio_system);
        app.world_mut().spawn((
            NpcMarker,
            Position::new([1.0, 64.0, 2.0]),
            FaunaVisualKind::HybridBeast,
        ));

        app.update();

        let events = app.world().resource::<Events<PlaySoundRecipeRequest>>();
        let event = events
            .iter_current_update_events()
            .next()
            .expect("hybrid beast spawn should emit its ambient audio recipe");
        assert_eq!(event.recipe_id, "fauna_hybrid_beast_ambient");
    }

    #[test]
    fn void_distorted_death_audio_uses_planned_death_recipe() {
        let mut app = App::new();
        app.add_event::<DeathEvent>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_fauna_death_vfx_audio_system);
        let beast = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([2.0, 64.0, 3.0]),
                FaunaVisualKind::VoidDistorted,
                FaunaTag::new(BeastKind::VoidDistorted),
            ))
            .id();
        app.world_mut().send_event(DeathEvent {
            target: beast,
            cause: "test".to_string(),
            attacker: None,
            attacker_player_id: None,
            at_tick: 3,
        });

        app.update();

        let events = app.world().resource::<Events<PlaySoundRecipeRequest>>();
        let event = events
            .iter_current_update_events()
            .next()
            .expect("void distorted death should emit its fauna death audio recipe");
        assert_eq!(event.recipe_id, "fauna_void_distorted_death");
    }
}
