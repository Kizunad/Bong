use valence::prelude::{DVec3, EventReader, EventWriter};

use super::quality::GatheringQuality;
use super::session::{GatheringCompleteEvent, GatheringProgressFrame};
use super::tools::GatheringTargetKind;
use crate::network::audio_event_emit::{
    AudioRecipient, PlaySoundRecipeRequest, AUDIO_BROADCAST_RADIUS,
};
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::schema::vfx_event::VfxEventPayloadV1;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GatheringFeedbackCue {
    pub sound_recipe: &'static str,
    pub particle_event_id: &'static str,
    pub color: &'static str,
    pub count: u16,
    pub duration_ticks: u16,
}

pub fn tick_feedback_cue(target: GatheringTargetKind) -> GatheringFeedbackCue {
    match target {
        GatheringTargetKind::Herb => GatheringFeedbackCue {
            sound_recipe: "gather_herb_tick",
            particle_event_id: "bong:gather_herb_tick",
            color: "#6FAF55",
            count: 2,
            duration_ticks: 24,
        },
        GatheringTargetKind::Ore => GatheringFeedbackCue {
            sound_recipe: "gather_mine_tick",
            particle_event_id: "bong:gather_mine_tick",
            color: "#8A8A8A",
            count: 3,
            duration_ticks: 18,
        },
        GatheringTargetKind::Wood => GatheringFeedbackCue {
            sound_recipe: "gather_chop_tick",
            particle_event_id: "bong:gather_chop_tick",
            color: "#A06A3C",
            count: 2,
            duration_ticks: 20,
        },
    }
}

pub fn completion_feedback_cue(quality: GatheringQuality) -> GatheringFeedbackCue {
    match quality {
        GatheringQuality::Perfect => GatheringFeedbackCue {
            sound_recipe: "gather_perfect",
            particle_event_id: "bong:gather_perfect",
            color: "#FFD35A",
            count: 12,
            duration_ticks: 28,
        },
        GatheringQuality::Fine | GatheringQuality::Normal => GatheringFeedbackCue {
            sound_recipe: "gather_complete",
            particle_event_id: "bong:gather_complete",
            color: if quality == GatheringQuality::Fine {
                "#62E67A"
            } else {
                "#DDE7F2"
            },
            count: 8,
            duration_ticks: 22,
        },
    }
}

pub fn emit_gathering_feedback(
    mut frames: EventReader<GatheringProgressFrame>,
    mut completions: EventReader<GatheringCompleteEvent>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    mut audio_events: EventWriter<PlaySoundRecipeRequest>,
) {
    for frame in frames.read() {
        if frame.completed || frame.interrupted {
            continue;
        }
        emit_cue(
            frame.origin_position,
            tick_feedback_cue(frame.target_type),
            &mut vfx_events,
            &mut audio_events,
        );
    }

    for completion in completions.read() {
        emit_cue(
            completion.origin_position,
            completion_feedback_cue(completion.quality),
            &mut vfx_events,
            &mut audio_events,
        );
    }
}

fn emit_cue(
    origin: [f64; 3],
    cue: GatheringFeedbackCue,
    vfx_events: &mut EventWriter<VfxEventRequest>,
    audio_events: &mut EventWriter<PlaySoundRecipeRequest>,
) {
    let origin_vec = DVec3::new(origin[0], origin[1], origin[2]);
    vfx_events.send(VfxEventRequest::new(
        origin_vec,
        VfxEventPayloadV1::SpawnParticle {
            event_id: cue.particle_event_id.to_string(),
            origin,
            direction: Some([0.0, 1.0, 0.0]),
            color: Some(cue.color.to_string()),
            strength: Some(0.8),
            count: Some(cue.count),
            duration_ticks: Some(cue.duration_ticks),
        },
    ));
    audio_events.send(PlaySoundRecipeRequest {
        recipe_id: cue.sound_recipe.to_string(),
        instance_id: 0,
        pos: Some([
            origin[0].round() as i32,
            origin[1].round() as i32,
            origin[2].round() as i32,
        ]),
        flag: None,
        volume_mul: 1.0,
        pitch_shift: 0.0,
        recipient: AudioRecipient::Radius {
            origin: origin_vec,
            radius: AUDIO_BROADCAST_RADIUS,
        },
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_feedback_selects_plan_tick_cues() {
        assert_eq!(
            tick_feedback_cue(GatheringTargetKind::Herb).sound_recipe,
            "gather_herb_tick"
        );
        assert_eq!(
            tick_feedback_cue(GatheringTargetKind::Ore).particle_event_id,
            "bong:gather_mine_tick"
        );
        assert_eq!(
            tick_feedback_cue(GatheringTargetKind::Wood).color,
            "#A06A3C"
        );
    }

    #[test]
    fn perfect_completion_uses_rare_feedback_cue() {
        let normal = completion_feedback_cue(GatheringQuality::Normal);
        let perfect = completion_feedback_cue(GatheringQuality::Perfect);

        assert_eq!(normal.sound_recipe, "gather_complete");
        assert_eq!(perfect.sound_recipe, "gather_perfect");
        assert!(perfect.count > normal.count);
    }
}
