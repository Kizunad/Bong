//! 黑武士 Boss action → VFX + 音效 emit（plan-sword-path-complete §B）。
//!
//! boss action system 在成功发招那帧 emit `HeiwushiActionVfxEvent`；
//! 本模块两个系统消费该事件，分别发 `VfxEventRequest`（粒子）和 `PlaySoundRecipeRequest`（音效）。
//!
//! boss 是 Marker 实体（无 UniqueId），故不走 PlayAnim 路径，仅靠粒子 + 音效呈现招式。

use valence::prelude::{EventReader, EventWriter};

use crate::network::audio_event_emit::{
    AudioRecipient, PlaySoundRecipeRequest, AUDIO_BROADCAST_RADIUS,
};
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::npc::heiwushi::{HeiwushiActionKind, HeiwushiActionVfxEvent};
use crate::schema::vfx_event::VfxEventPayloadV1;

// ── 粒子 event id 常量（必须与 client SwordPathVfxPlayer.EVENT_IDS 精确一致）──────
const VFX_HEIWUSHI_MELEE_SLASH: &str = "bong:heiwushi_melee_slash";
const VFX_HEIWUSHI_DARK_BARRAGE: &str = "bong:heiwushi_dark_barrage";
const VFX_HEIWUSHI_DARK_VORTEX: &str = "bong:heiwushi_dark_vortex";
const VFX_HEIWUSHI_TRANSFORM: &str = "bong:heiwushi_transform";
const VFX_HEIWUSHI_DEATH: &str = "bong:heiwushi_death";

// SwordIntent 复用 dark_barrage 视觉（boss 飞剑）
const VFX_HEIWUSHI_SWORD_INTENT: &str = "bong:heiwushi_dark_barrage";

// ── 音效 recipe id（必须与 server/assets/audio/recipes/heiwushi_*.json 对齐）─────
const AUDIO_HEIWUSHI_MELEE_SLASH: &str = "heiwushi_melee_slash";
const AUDIO_HEIWUSHI_DARK_BARRAGE: &str = "heiwushi_dark_barrage";
const AUDIO_HEIWUSHI_DARK_VORTEX: &str = "heiwushi_dark_vortex";
const AUDIO_HEIWUSHI_TRANSFORM: &str = "heiwushi_transform";
const AUDIO_HEIWUSHI_DEATH: &str = "heiwushi_death";
// SwordIntent 复用 sword_manifest_summon（召唤剑意）
const AUDIO_HEIWUSHI_SWORD_INTENT: &str = "sword_manifest_summon";

/// 黑武士 action → 粒子 VFX emit。
pub fn emit_heiwushi_visual_triggers(
    mut events: EventReader<HeiwushiActionVfxEvent>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for event in events.read() {
        let origin = event.origin;
        let dir = event.direction.map(|d| [d.x, d.y, d.z]);

        let (event_id, color, count, duration, strength) = match event.kind {
            HeiwushiActionKind::MeleeSlash => {
                (VFX_HEIWUSHI_MELEE_SLASH, "#334455", 3u16, 8u16, 0.8f32)
            }
            HeiwushiActionKind::DarkBarrage => {
                (VFX_HEIWUSHI_DARK_BARRAGE, "#2A0033", 8u16, 20u16, 0.8f32)
            }
            HeiwushiActionKind::DarkVortex => {
                (VFX_HEIWUSHI_DARK_VORTEX, "#2A0033", 16u16, 30u16, 0.8f32)
            }
            HeiwushiActionKind::ShadowTransform => {
                (VFX_HEIWUSHI_TRANSFORM, "#2A0033", 32u16, 25u16, 0.9f32)
            }
            HeiwushiActionKind::Death => (VFX_HEIWUSHI_DEATH, "#334455", 48u16, 60u16, 0.8f32),
            HeiwushiActionKind::SwordIntent => {
                (VFX_HEIWUSHI_SWORD_INTENT, "#2A0033", 8u16, 20u16, 0.8f32)
            }
        };

        vfx_events.send(VfxEventRequest::new(
            origin,
            VfxEventPayloadV1::SpawnParticle {
                event_id: event_id.to_string(),
                origin: [origin.x, origin.y, origin.z],
                direction: dir,
                color: Some(color.to_string()),
                strength: Some(strength),
                count: Some(count),
                duration_ticks: Some(duration),
            },
        ));
    }
}

/// 黑武士 action → 音效 emit。
pub fn emit_heiwushi_audio_triggers(
    mut events: EventReader<HeiwushiActionVfxEvent>,
    mut audio: EventWriter<PlaySoundRecipeRequest>,
) {
    for event in events.read() {
        let origin = event.origin;
        let recipe_id = match event.kind {
            HeiwushiActionKind::MeleeSlash => AUDIO_HEIWUSHI_MELEE_SLASH,
            HeiwushiActionKind::DarkBarrage => AUDIO_HEIWUSHI_DARK_BARRAGE,
            HeiwushiActionKind::DarkVortex => AUDIO_HEIWUSHI_DARK_VORTEX,
            HeiwushiActionKind::ShadowTransform => AUDIO_HEIWUSHI_TRANSFORM,
            HeiwushiActionKind::Death => AUDIO_HEIWUSHI_DEATH,
            HeiwushiActionKind::SwordIntent => AUDIO_HEIWUSHI_SWORD_INTENT,
        };

        audio.send(PlaySoundRecipeRequest {
            recipe_id: recipe_id.to_string(),
            instance_id: 0,
            pos: Some([
                origin.x.round() as i32,
                origin.y.round() as i32,
                origin.z.round() as i32,
            ]),
            flag: Some(format!("heiwushi_{}", recipe_id)),
            volume_mul: 1.0,
            pitch_shift: 0.0,
            recipient: AudioRecipient::Radius {
                origin,
                radius: AUDIO_BROADCAST_RADIUS,
            },
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::npc::heiwushi::{HeiwushiActionKind, HeiwushiActionVfxEvent};
    use valence::prelude::{App, DVec3, Entity, Events, Update};

    fn make_app() -> App {
        let mut app = App::new();
        app.add_event::<HeiwushiActionVfxEvent>()
            .add_event::<VfxEventRequest>()
            .add_event::<PlaySoundRecipeRequest>()
            .add_systems(
                Update,
                (emit_heiwushi_visual_triggers, emit_heiwushi_audio_triggers),
            );
        app
    }

    fn send_action(app: &mut App, kind: HeiwushiActionKind, dir: Option<DVec3>) {
        app.world_mut().send_event(HeiwushiActionVfxEvent {
            boss: Entity::PLACEHOLDER,
            kind,
            origin: DVec3::new(100.0, 64.0, 100.0),
            direction: dir,
            tick: 42,
        });
    }

    fn drain_vfx(app: &App) -> Vec<String> {
        app.world()
            .resource::<Events<VfxEventRequest>>()
            .iter_current_update_events()
            .filter_map(|e| {
                if let VfxEventPayloadV1::SpawnParticle { event_id, .. } = &e.payload {
                    Some(event_id.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    fn drain_audio(app: &App) -> Vec<String> {
        app.world()
            .resource::<Events<PlaySoundRecipeRequest>>()
            .iter_current_update_events()
            .map(|e| e.recipe_id.clone())
            .collect()
    }

    #[test]
    fn melee_slash_emits_correct_vfx_and_audio() {
        let mut app = make_app();
        send_action(&mut app, HeiwushiActionKind::MeleeSlash, Some(DVec3::X));
        app.update();
        assert_eq!(
            drain_vfx(&app),
            vec![VFX_HEIWUSHI_MELEE_SLASH],
            "MeleeSlash 应 emit bong:heiwushi_melee_slash 粒子"
        );
        assert_eq!(
            drain_audio(&app),
            vec![AUDIO_HEIWUSHI_MELEE_SLASH],
            "MeleeSlash 应 emit heiwushi_melee_slash 音效"
        );
    }

    #[test]
    fn dark_barrage_emits_correct_vfx_and_audio() {
        let mut app = make_app();
        send_action(&mut app, HeiwushiActionKind::DarkBarrage, Some(DVec3::Z));
        app.update();
        assert_eq!(drain_vfx(&app), vec![VFX_HEIWUSHI_DARK_BARRAGE]);
        assert_eq!(drain_audio(&app), vec![AUDIO_HEIWUSHI_DARK_BARRAGE]);
    }

    #[test]
    fn dark_vortex_emits_correct_vfx_and_audio() {
        let mut app = make_app();
        send_action(&mut app, HeiwushiActionKind::DarkVortex, None);
        app.update();
        assert_eq!(drain_vfx(&app), vec![VFX_HEIWUSHI_DARK_VORTEX]);
        assert_eq!(drain_audio(&app), vec![AUDIO_HEIWUSHI_DARK_VORTEX]);
    }

    #[test]
    fn shadow_transform_emits_correct_vfx_and_audio() {
        let mut app = make_app();
        send_action(&mut app, HeiwushiActionKind::ShadowTransform, None);
        app.update();
        assert_eq!(drain_vfx(&app), vec![VFX_HEIWUSHI_TRANSFORM]);
        assert_eq!(drain_audio(&app), vec![AUDIO_HEIWUSHI_TRANSFORM]);
    }

    #[test]
    fn death_emits_correct_vfx_and_audio() {
        let mut app = make_app();
        send_action(&mut app, HeiwushiActionKind::Death, None);
        app.update();
        assert_eq!(drain_vfx(&app), vec![VFX_HEIWUSHI_DEATH]);
        assert_eq!(drain_audio(&app), vec![AUDIO_HEIWUSHI_DEATH]);
    }

    #[test]
    fn sword_intent_emits_correct_vfx_and_audio() {
        let mut app = make_app();
        send_action(&mut app, HeiwushiActionKind::SwordIntent, Some(DVec3::X));
        app.update();
        // SwordIntent 复用 dark_barrage 视觉
        assert_eq!(drain_vfx(&app), vec![VFX_HEIWUSHI_SWORD_INTENT]);
        assert_eq!(drain_audio(&app), vec![AUDIO_HEIWUSHI_SWORD_INTENT]);
    }
}
