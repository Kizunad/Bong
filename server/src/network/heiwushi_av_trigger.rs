//! 黑武士 Boss action → VFX + 音效 emit（plan-sword-path-complete §B）。
//!
//! boss action system 在成功发招那帧 emit `HeiwushiActionVfxEvent`；
//! 本模块两个系统消费该事件，分别发 `VfxEventRequest`（粒子）和 `PlaySoundRecipeRequest`（音效）。
//!
//! boss 是 Marker 实体（无 UniqueId），故 PlayAnim（按玩家 UUID 寻人）走不通；招式的
//! GeckoLib 实体动画改走 `VfxEventPayloadV1::PlayEntityAnim`，按 MC 协议 `entity_id`
//! （Valence `EntityId::get()`）定位客户端 `FaunaEntity` 并播一次性招式动画。

use valence::entity::EntityId;
use valence::prelude::{EventReader, EventWriter, Query};

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

// ── GeckoLib 实体招式动画名（必须与 client heiwushi.animation.json key 精确一致）────
// 仅这几招有对应 GeckoLib 动画；MeleeSlash / Death 无专属动画，不 emit PlayEntityAnim。
const ANIM_HEIWUSHI_DARK_BARRAGE: &str = "animation.bong.heiwushi.dark_barrage";
const ANIM_HEIWUSHI_DARK_VORTEX: &str = "animation.bong.heiwushi.dark_vortex";
const ANIM_HEIWUSHI_TRANSFORM: &str = "animation.bong.heiwushi.transform";
// SwordIntent（boss 飞剑）复用 dark_barrage 挥击动画。
const ANIM_HEIWUSHI_SWORD_INTENT: &str = "animation.bong.heiwushi.dark_barrage";

/// kind → (GeckoLib 动画名, 动画占用 tick)。返回 None 表示该招无对应实体动画（跳过 emit）。
fn heiwushi_entity_anim_for(kind: HeiwushiActionKind) -> Option<(&'static str, u16)> {
    match kind {
        HeiwushiActionKind::DarkBarrage => Some((ANIM_HEIWUSHI_DARK_BARRAGE, 15)),
        HeiwushiActionKind::DarkVortex => Some((ANIM_HEIWUSHI_DARK_VORTEX, 21)),
        HeiwushiActionKind::ShadowTransform => Some((ANIM_HEIWUSHI_TRANSFORM, 16)),
        HeiwushiActionKind::SwordIntent => Some((ANIM_HEIWUSHI_SWORD_INTENT, 15)),
        // 无 GeckoLib 招式动画 → 仅靠粒子 + 音效呈现。
        HeiwushiActionKind::MeleeSlash | HeiwushiActionKind::Death => None,
    }
}

// ── 音效 recipe id（必须与 server/assets/audio/recipes/heiwushi_*.json 对齐）─────
const AUDIO_HEIWUSHI_MELEE_SLASH: &str = "heiwushi_melee_slash";
const AUDIO_HEIWUSHI_DARK_BARRAGE: &str = "heiwushi_dark_barrage";
const AUDIO_HEIWUSHI_DARK_VORTEX: &str = "heiwushi_dark_vortex";
const AUDIO_HEIWUSHI_TRANSFORM: &str = "heiwushi_transform";
const AUDIO_HEIWUSHI_DEATH: &str = "heiwushi_death";
// SwordIntent 复用 sword_manifest_summon（召唤剑意）
const AUDIO_HEIWUSHI_SWORD_INTENT: &str = "sword_manifest_summon";

/// 黑武士 action → 粒子 VFX + GeckoLib 实体招式动画 emit。
///
/// `boss_ids` 用于取 boss 的 MC 协议 `entity_id`（Valence `EntityId::get()`），
/// 供 `PlayEntityAnim` 在客户端按 `world.getEntityById` 定位 `FaunaEntity`。
pub fn emit_heiwushi_visual_triggers(
    mut events: EventReader<HeiwushiActionVfxEvent>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    boss_ids: Query<&EntityId>,
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

        // 招式 GeckoLib 实体动画：仅对有对应动画的招式 emit；boss 无 EntityId（尚未分配
        // 协议 id / 已 despawn）则跳过——客户端反正定位不到实体。
        if let Some((anim, duration_ticks)) = heiwushi_entity_anim_for(event.kind) {
            if let Ok(entity_id) = boss_ids.get(event.boss) {
                vfx_events.send(VfxEventRequest::new(
                    origin,
                    VfxEventPayloadV1::PlayEntityAnim {
                        entity_id: entity_id.get(),
                        anim: anim.to_string(),
                        duration_ticks,
                    },
                ));
            }
        }
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
    use valence::entity::EntityId;
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

    /// 生成一个带 `EntityId` 组件的 boss 实体（模拟已分配协议 id 的 marker boss）。
    /// 注意：valence `EntityId` 字段私有，外部只能造 `EntityId::default()`（= -1，
    /// 运行时由 valence 改写为正值）；测试断言"emit 的 entity_id == 该实体 EntityId.get()"，
    /// 锁住"读对了实体"这条接线契约（与具体数值无关）。
    fn spawn_boss_with_entity_id(app: &mut App) -> Entity {
        app.world_mut().spawn(EntityId::default()).id()
    }

    fn send_action_from(app: &mut App, boss: Entity, kind: HeiwushiActionKind) {
        app.world_mut().send_event(HeiwushiActionVfxEvent {
            boss,
            kind,
            origin: DVec3::new(100.0, 64.0, 100.0),
            direction: Some(DVec3::X),
            tick: 42,
        });
    }

    /// 抽出本 tick emit 的所有 PlayEntityAnim → (entity_id, anim, duration_ticks)。
    fn drain_entity_anim(app: &App) -> Vec<(i32, String, u16)> {
        app.world()
            .resource::<Events<VfxEventRequest>>()
            .iter_current_update_events()
            .filter_map(|e| {
                if let VfxEventPayloadV1::PlayEntityAnim {
                    entity_id,
                    anim,
                    duration_ticks,
                } = &e.payload
                {
                    Some((*entity_id, anim.clone(), *duration_ticks))
                } else {
                    None
                }
            })
            .collect()
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

    // ── PlayEntityAnim：黑武士招式 → GeckoLib 实体动画 ─────────────────────────

    /// 表驱动：每个有 GeckoLib 动画的招式 emit 正确 (anim, duration)，且 entity_id 取自
    /// boss 的 `EntityId.get()`。MeleeSlash / Death 在下一条用例验证"不 emit"。
    #[test]
    fn action_emits_entity_anim_with_correct_anim_duration_and_entity_id() {
        let cases = [
            (
                HeiwushiActionKind::DarkBarrage,
                "animation.bong.heiwushi.dark_barrage",
                15u16,
            ),
            (
                HeiwushiActionKind::DarkVortex,
                "animation.bong.heiwushi.dark_vortex",
                21,
            ),
            (
                HeiwushiActionKind::ShadowTransform,
                "animation.bong.heiwushi.transform",
                16,
            ),
            (
                // SwordIntent 复用 dark_barrage 挥击动画。
                HeiwushiActionKind::SwordIntent,
                "animation.bong.heiwushi.dark_barrage",
                15,
            ),
        ];
        for (kind, expected_anim, expected_duration) in cases {
            let mut app = make_app();
            let boss = spawn_boss_with_entity_id(&mut app);
            let expected_id = app.world().get::<EntityId>(boss).unwrap().get();
            send_action_from(&mut app, boss, kind);
            app.update();

            let emitted = drain_entity_anim(&app);
            assert_eq!(
                emitted.len(),
                1,
                "{kind:?} 应 emit 恰好 1 条 PlayEntityAnim，实际: {emitted:?}"
            );
            let (entity_id, anim, duration) = &emitted[0];
            assert_eq!(
                *entity_id, expected_id,
                "{kind:?} 的 PlayEntityAnim.entity_id 应取自 boss 的 EntityId.get()={expected_id}，实际 {entity_id}"
            );
            assert_eq!(
                anim, expected_anim,
                "{kind:?} 的 anim 应为 {expected_anim}，实际 {anim}"
            );
            assert_eq!(
                *duration, expected_duration,
                "{kind:?} 的 duration_ticks 应为 {expected_duration}，实际 {duration}"
            );
        }
    }

    #[test]
    fn melee_and_death_do_not_emit_entity_anim() {
        for kind in [HeiwushiActionKind::MeleeSlash, HeiwushiActionKind::Death] {
            let mut app = make_app();
            let boss = spawn_boss_with_entity_id(&mut app);
            send_action_from(&mut app, boss, kind);
            app.update();
            assert!(
                drain_entity_anim(&app).is_empty(),
                "{kind:?} 无 GeckoLib 招式动画，不应 emit PlayEntityAnim，实际: {:?}",
                drain_entity_anim(&app)
            );
            // 但仍应 emit 粒子（确认只是没接动画，不是整条没跑）。
            assert!(!drain_vfx(&app).is_empty(), "{kind:?} 仍应 emit 粒子 VFX");
        }
    }

    #[test]
    fn entity_anim_skipped_when_boss_has_no_entity_id() {
        // boss 实体无 EntityId 组件（Entity::PLACEHOLDER）→ 即便招式有动画也跳过 PlayEntityAnim，
        // 否则会 emit 一条客户端定位不到实体的死事件。
        let mut app = make_app();
        send_action(&mut app, HeiwushiActionKind::DarkBarrage, Some(DVec3::X));
        app.update();
        assert!(
            drain_entity_anim(&app).is_empty(),
            "boss 无 EntityId 时不应 emit PlayEntityAnim，实际: {:?}",
            drain_entity_anim(&app)
        );
        // 粒子仍应 emit（不依赖 EntityId）。
        assert_eq!(drain_vfx(&app), vec![VFX_HEIWUSHI_DARK_BARRAGE]);
    }
}
