//! Domain event adapters for audio v1 SoundRecipe triggers.

use std::collections::HashMap;

use valence::prelude::bevy_ecs::system::SystemParam;
use valence::prelude::{
    bevy_ecs, App, Client, DVec3, Entity, EventReader, EventWriter, IntoSystemConfigs, Position,
    Query, Res, ResMut, Resource, Update, With,
};

use crate::alchemy::{AlchemyOutcomeEvent, ResolvedOutcome, StartAlchemyRequest};
use crate::audio::implementation::{
    breakthrough_recipe, combat_hit_recipe_for_body_part, forge_hammer_recipe, parry_recipe,
    school_hit_recipe, AudioImplementationDedup,
};
use crate::audio::SoundRecipeRegistry;
use crate::botany::components::HarvestTerminalEvent;
use crate::combat::anqi_v2::{
    AnqiSkillId, ArmorPierceEvent, DecoyDeployEvent, MultiShotEvent, QiInjectionEvent,
};
use crate::combat::baomai_v3::{BaomaiSkillEvent, BaomaiSkillId};
use crate::combat::carrier::CarrierChargedEvent;
use crate::combat::components::{Lifecycle, Wounds};
use crate::combat::dugu_v2::skills::DUGU_POISON_SIGNATURE_RECIPE;
use crate::combat::dugu_v2::ReverseTriggeredEvent;
use crate::combat::events::{AttackSource, CombatEvent, DeathEvent, DefenseKind};
use crate::combat::needle::QiNeedleChargedEvent;
use crate::combat::tuike_v2::{ContamTransferredEvent, DonFalseSkinEvent, FalseSkinSheddedEvent};
use crate::combat::woliu::{VortexBackfireEvent, VortexField};
use crate::combat::woliu_v2::VortexCastEvent;
use crate::combat::zhenmai_v2::ZhenmaiSkillCastEvent;
use crate::cultivation::breakthrough::BreakthroughOutcome;
use crate::cultivation::components::Cultivation;
use crate::cultivation::dugu::DuguObfuscationDisruptedEvent;
use crate::cultivation::life_record::LifeRecord;
use crate::cultivation::meridian_open::MeridianOpenedEvent;
use crate::cultivation::overload::MeridianOverloadEvent;
use crate::cultivation::possession::DuoSheWarningEvent;
use crate::cultivation::qi_zero_decay::RealmRegressed;
use crate::cultivation::tribulation::{
    JueBiTriggeredEvent, TribulationAnnounce, TribulationFailed, TribulationKind,
    TribulationSettled, TribulationState, TribulationWaveCleared,
};
use crate::forge::blueprint::TemperBeat;
use crate::forge::events::{ForgeBucket, ForgeOutcomeEvent, ForgeStartAccepted, TemperingHit};
use crate::forge::session::{ForgeSessions, ForgeStep};
use crate::lingtian::events::{
    DrainQiCompleted, HarvestCompleted, PlantingCompleted, RenewCompleted, ReplenishCompleted,
    TillCompleted,
};
use crate::network::audio_event_emit::{
    recipient_for_attenuation, AudioRecipient, PlaySoundRecipeRequest, AUDIO_BROADCAST_RADIUS,
};
use crate::npc::brain::canonical_npc_id;
use crate::npc::spawn::NpcMarker;
use crate::schema::tribulation::DuXuOutcomeV1;
use crate::skill::events::{SkillLvUp, SkillScrollUsed, SkillXpGain, XpGainSource};
use crate::social::events::{SocialPactEvent, SocialRenownDeltaEvent};
use crate::sword_path::av_event::{SwordPathSkillCastEvent, SwordPathSkillId};

/// **audio-trigger 调度的唯一生产注册入口**（`network::register` 调它，别处不许再散着 `add_systems`）。
///
/// 提取自 `network::mod`（PR #1262 review 意见）：接线门禁测试跑的就是这个函数，于是「某个 emit
/// 系统没被注册进调度」不再是测试照不到的死角——测试不再自己抄一份系统清单，从这里删掉任何一个
/// 系统，`register_wires_all_audio_trigger_systems` 立刻撞红。
///
/// 调度契约（与提取前逐条一致）：所有 emit 系统 `.after(tick_audio_dedup_clock)`（拿到当帧 dedup
/// 逻辑 tick）`.before(audio_event_emit::emit_audio_play_payloads)`（同帧把 `PlaySoundRecipeRequest`
/// 投递给客户端，不跨帧延迟）。
pub fn register(app: &mut App) {
    app.add_systems(Update, tick_audio_dedup_clock);
    app.add_systems(
        Update,
        (
            emit_combat_audio_triggers.after(crate::combat::resolve::resolve_attack_intents),
            emit_npc_death_audio_triggers.after(crate::combat::resolve::resolve_attack_intents),
            emit_cultivation_audio_triggers,
            emit_tribulation_audio_triggers,
            emit_alchemy_audio_triggers,
            emit_forge_audio_triggers,
            emit_botany_audio_triggers,
            emit_lingtian_audio_triggers,
            emit_woliu_v2_audio_triggers,
            // 绝灵涡流（woliu v1）开涡 / 反噬 → 音效（lifecycle 驱动，复用现有 recipe）。
            emit_woliu_v1_vortex_audio_triggers,
            emit_baomai_v3_audio_triggers,
            emit_tuike_v2_audio_triggers,
            // 真脉五招 cast（`ZhenmaiSkillCastEvent`）→ 逐招专属音效（含 sever_chain 签名）。
            emit_zhenmai_v2_audio_triggers,
            emit_sword_path_audio_triggers,
            // 暗器六招 cast → 专属音效（纯 cosmetic，复用 vanilla 音色 recipe）。
            emit_anqi_audio_triggers,
            // 蛊道（凝针 / 灌毒蛊 / 倒蚀签名）cast → 专属音效（纯 cosmetic）。
            emit_dugu_v2_audio_triggers,
            emit_skill_audio_triggers,
            emit_social_audio_triggers
                .after(crate::cultivation::possession::process_duo_she_requests),
            emit_player_state_audio_triggers,
        )
            .after(tick_audio_dedup_clock)
            .before(crate::network::audio_event_emit::emit_audio_play_payloads),
    );
}

#[derive(Debug, Default)]
pub struct AudioTriggerState {
    low_hp: HashMap<Entity, bool>,
    low_qi: HashMap<Entity, bool>,
}

impl Resource for AudioTriggerState {}

const LOW_HP_HEARTBEAT_RATIO: f32 = 0.2;
const LOW_HP_HEARTBEAT_FLAG: &str = "hp_below_20";

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
    mut audio: AudioEmitWriter,
) {
    let mut audio = audio.context();
    for (entity, position, wounds, cultivation) in &players {
        if let Some(wounds) = wounds {
            let hp_ratio = wounds.health_current / wounds.health_max.max(1.0);
            let low_hp = hp_ratio < LOW_HP_HEARTBEAT_RATIO;
            if low_hp && !state.low_hp.get(&entity).copied().unwrap_or(false) {
                emit_play(
                    &mut audio,
                    "heartbeat_low_hp",
                    entity,
                    position.get(),
                    Some(LOW_HP_HEARTBEAT_FLAG.to_string()),
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
    mut audio: AudioEmitWriter,
) {
    let mut audio = audio.context();
    for event in combat_events.read() {
        let Ok(position) = positions.get(event.target) else {
            continue;
        };
        let origin = position.get();
        let total_damage = event.damage + event.physical_damage;
        let recipe_id = if event.defense_kind == Some(DefenseKind::JieMai) {
            parry_recipe(event.defense_effectiveness.unwrap_or(0.6))
        } else if event.defense_kind == Some(DefenseKind::SwordParry) {
            "sword_parry"
        } else if let Some(effectiveness) = event.defense_effectiveness {
            parry_recipe(effectiveness)
        } else if total_damage >= 0.5 {
            let critical = matches!(event.body_part, crate::combat::components::BodyPart::Head);
            match event.source {
                AttackSource::BurstMeridian | AttackSource::FullPower => {
                    school_hit_recipe("baomai", event.damage, critical)
                }
                AttackSource::QiNeedle => school_hit_recipe("dugu", event.damage, critical),
                AttackSource::SwordCleave => "sword_cleave",
                AttackSource::SwordThrust => "sword_thrust",
                // plan-sword-path-v2 §P4：这里是**命中冲击**音效（伤害落地一刻），
                // 沿用基础剑斩配方保证打击感。各招的**施法**专属音效（凝锋 / 剑气 /
                // 剑鸣 / 化形 / 天门）走 emit_sword_path_audio_triggers 读
                // SwordPathSkillCastEvent 独立 emit，与命中音效互补分层。
                AttackSource::SwordPathCondenseEdge => "sword_cleave",
                AttackSource::SwordPathQiSlash => "sword_thrust",
                AttackSource::SwordPathResonance => "sword_cleave",
                AttackSource::SwordPathManifest => "sword_cleave",
                AttackSource::SwordPathHeavenGate => "sword_cleave",
                AttackSource::Melee | AttackSource::NpcMelee => {
                    combat_hit_recipe_for_body_part(event.body_part, total_damage, critical)
                }
            }
        } else if npc_markers.get(event.target).is_ok() && total_damage > 0.0 {
            "npc_hurt"
        } else if npc_markers.get(event.attacker).is_ok() && total_damage > 0.0 {
            "npc_aggro"
        } else {
            continue;
        };
        emit_play(&mut audio, recipe_id, event.target, origin, None, 1.0, 0.0);
        if total_damage >= 8.0 {
            emit_play(
                &mut audio,
                "wound_inflict",
                event.target,
                origin,
                None,
                0.85,
                0.0,
            );
        }
    }
}

pub fn emit_npc_death_audio_triggers(
    mut death_events: EventReader<DeathEvent>,
    positions: Query<&Position>,
    npc_markers: Query<(), With<NpcMarker>>,
    mut audio: AudioEmitWriter,
) {
    let mut audio = audio.context();
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
        if let Some(attacker) = event.attacker {
            emit_play(
                &mut audio,
                "kill_confirm",
                attacker,
                position.get(),
                None,
                1.0,
                0.0,
            );
        }
    }
}

pub fn emit_cultivation_audio_triggers(
    mut breakthroughs: EventReader<BreakthroughOutcome>,
    mut meridian_opened: EventReader<MeridianOpenedEvent>,
    mut regressions: EventReader<RealmRegressed>,
    mut overloads: EventReader<MeridianOverloadEvent>,
    positions: Query<&Position>,
    mut audio: AudioEmitWriter,
) {
    let mut audio = audio.context();
    for event in meridian_opened.read() {
        emit_play(
            &mut audio,
            "meridian_open",
            event.entity,
            event.origin,
            None,
            1.0,
            0.0,
        );
    }

    for event in breakthroughs.read() {
        let Ok(position) = positions.get(event.entity) else {
            continue;
        };
        let origin = position.get();
        let recipe_id = match &event.result {
            Ok(success) => breakthrough_recipe(success.to),
            Err(_) => "breakthrough_fail",
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
            "overload_tear",
            event.entity,
            position.get(),
            None,
            severity_volume(event.severity),
            0.0,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub fn emit_tribulation_audio_triggers(
    mut announces: EventReader<TribulationAnnounce>,
    mut juebi_triggered: EventReader<JueBiTriggeredEvent>,
    mut waves: EventReader<TribulationWaveCleared>,
    mut failures: EventReader<TribulationFailed>,
    mut settled: EventReader<TribulationSettled>,
    positions: Query<&Position>,
    states: Query<&TribulationState>,
    mut audio: AudioEmitWriter,
) {
    let mut audio = audio.context();
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

    for event in settled.read() {
        let recipe = match event.result.outcome {
            DuXuOutcomeV1::Ascended | DuXuOutcomeV1::HalfStep => "tribulation_ascend_success",
            _ => continue,
        };
        let Ok(position) = positions.get(event.entity) else {
            continue;
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
}

pub fn emit_alchemy_audio_triggers(
    mut starts: EventReader<StartAlchemyRequest>,
    mut outcomes: EventReader<AlchemyOutcomeEvent>,
    positions: Query<&Position>,
    mut audio: AudioEmitWriter,
) {
    let mut audio = audio.context();
    for event in starts.read() {
        let origin = positions
            .get(event.furnace)
            .map(|position| position.get())
            .unwrap_or(DVec3::ZERO);
        emit_play(
            &mut audio,
            "alchemy_bubble",
            event.furnace,
            origin,
            Some("alchemy_brewing".to_string()),
            0.8,
            0.0,
        );
    }

    for event in outcomes.read() {
        let origin = positions
            .get(event.furnace)
            .map(|position| position.get())
            .unwrap_or(DVec3::ZERO);
        let (recipe_id, volume_mul) = match event.outcome {
            ResolvedOutcome::Pill { .. } => ("alchemy_complete", 0.9),
            ResolvedOutcome::Explode { .. } => ("alchemy_fail", 1.0),
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
    mut audio: AudioEmitWriter,
) {
    let mut audio = audio.context();
    for event in starts.read() {
        if let Ok(position) = positions.get(event.station) {
            emit_play(
                &mut audio,
                "forge_consecrate",
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
        let recipe_id = forge_hammer_recipe(matches!(event.beat, TemperBeat::Heavy));
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
            ForgeBucket::Explode => "alchemy_fail",
            ForgeBucket::Perfect | ForgeBucket::Good | ForgeBucket::Flawed => "forge_complete",
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
    mut audio: AudioEmitWriter,
) {
    let mut audio = audio.context();
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
    mut renews: EventReader<RenewCompleted>,
    mut audio: AudioEmitWriter,
) {
    let mut audio = audio.context();
    for event in tills.read() {
        emit_play_at_block(&mut audio, "lingtian_till", event.player, event.pos, 1.0);
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
        emit_play_at_block(&mut audio, "lingtian_harvest", event.player, event.pos, 1.0);
    }
    for event in replenishes.read() {
        emit_play_at_block(
            &mut audio,
            "lingtian_replenish",
            event.player,
            event.pos,
            1.0,
        );
    }
    for event in drains.read() {
        emit_play_at_block(&mut audio, "lingtian_drain", event.player, event.pos, 0.85);
    }
    for event in renews.read() {
        emit_play_at_block(
            &mut audio,
            "lingtian_replenish",
            event.player,
            event.pos,
            1.0,
        );
    }
}

pub fn emit_woliu_v2_audio_triggers(
    mut casts: EventReader<VortexCastEvent>,
    positions: Query<&Position>,
    mut audio: AudioEmitWriter,
) {
    let mut audio = audio.context();
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

/// 绝灵涡流（woliu v1 `woliu.vortex`）→ 音效（纯 cosmetic）。
///
/// v1 是长驻领域（`VortexField` component）无 cast 事件，开涡走 lifecycle 检测
/// （与 vfx 侧 `emit_woliu_v1_vortex_visual_triggers` 同模式、状态各自独立）：
/// - field 出现 → `woliu_cast`（复用现有 recipe，零新资产）
/// - 反噬 `VortexBackfireEvent` → `woliu_burst_pop`（爆裂声，断经反噬语义）
pub fn emit_woliu_v1_vortex_audio_triggers(
    mut active_fields: bevy_ecs::prelude::Local<std::collections::HashSet<Entity>>,
    fields: Query<(Entity, &VortexField)>,
    mut backfires: EventReader<VortexBackfireEvent>,
    positions: Query<&Position>,
    mut audio: AudioEmitWriter,
) {
    let mut audio = audio.context();
    let mut seen = std::collections::HashSet::new();
    for (entity, field) in &fields {
        seen.insert(entity);
        if !active_fields.contains(&entity) {
            emit_play(
                &mut audio,
                "woliu_cast",
                field.caster,
                field.center,
                Some("woliu.vortex".to_string()),
                1.0,
                0.0,
            );
        }
    }
    *active_fields = seen;

    for event in backfires.read() {
        // caster 断 Position（断线瞬间）时兜底到领域中心——反噬是重要负反馈，不能静默丢。
        let Ok(origin) = positions
            .get(event.caster)
            .map(|p| p.get())
            .or_else(|_| fields.get(event.caster).map(|(_, field)| field.center))
        else {
            continue;
        };
        emit_play(
            &mut audio,
            "woliu_burst_pop",
            event.caster,
            origin,
            Some("woliu.vortex.backfire".to_string()),
            1.0,
            -0.2,
        );
    }
}

pub fn emit_baomai_v3_audio_triggers(
    mut events: EventReader<BaomaiSkillEvent>,
    positions: Query<&Position>,
    mut audio: AudioEmitWriter,
) {
    let mut audio = audio.context();
    for event in events.read() {
        let Ok(position) = positions.get(event.caster) else {
            continue;
        };
        emit_play(
            &mut audio,
            baomai_recipe_for_skill(event.skill),
            event.caster,
            position.get(),
            Some(event.skill.wire_kind().to_string()),
            1.0,
            0.0,
        );
    }
}

pub(crate) fn baomai_recipe_for_skill(skill: BaomaiSkillId) -> &'static str {
    match skill {
        // 崩拳走专属配方（穿透爆发的形意拳直拳），不再借用通用 baomai_hit_heavy 槽。
        BaomaiSkillId::BengQuan => "beng_quan",
        BaomaiSkillId::FullPowerCharge => "baomai_cast",
        BaomaiSkillId::FullPowerRelease => "baomai_signature",
        BaomaiSkillId::MountainShake => "baomai_hit_critical",
        BaomaiSkillId::BloodBurn => "baomai_hit_light",
        BaomaiSkillId::Disperse => "baomai_signature",
    }
}

/// plan-sword-path-v2 P4 — 剑道五招 cast → 各招专属音效配方。
///
/// 读 `SwordPathSkillCastEvent`，按招式发 `PlaySoundRecipeRequest`，引用客户端已
/// 注册的 `audio_recipes/sword_*.json`。caster 无 `Position` 时落到 cast center
/// （AV 事件自带），保证施法者断 Position 也能出招声。
///
/// **纯 cosmetic**：只发音效，不读 / 改任何战斗 / 真元状态。
pub fn emit_sword_path_audio_triggers(
    mut casts: EventReader<SwordPathSkillCastEvent>,
    positions: Query<&Position>,
    mut audio: AudioEmitWriter,
) {
    let mut audio = audio.context();
    for event in casts.read() {
        let origin = positions
            .get(event.caster)
            .map(|position| position.get())
            .unwrap_or(event.center);
        emit_play(
            &mut audio,
            sword_path_recipe_for_skill(event.skill),
            event.caster,
            origin,
            Some(sword_path_audio_flag(event.skill).to_string()),
            1.0,
            0.0,
        );
    }
}

pub(crate) fn sword_path_recipe_for_skill(skill: SwordPathSkillId) -> &'static str {
    match skill {
        SwordPathSkillId::CondenseEdge => "sword_condense_edge",
        SwordPathSkillId::QiSlash => "sword_qi_slash",
        SwordPathSkillId::Resonance => "sword_resonance",
        SwordPathSkillId::Manifest => "sword_manifest_summon",
        // 蓄力走专属 `heaven_gate_charge`（复用 release 的 `bong:skill.sword_path.heaven_gate`
        // 签名 ogg 作 pitch 0.72/volume 0.4 前兆层 + amethyst 铺底）；释放用 manifest_strike
        // （开天劈击）。**不能复用共享的 `sword_infuse`**——那被基础剑招（sword_basics）也消费，
        // 塞签名会泄漏到普通注剑。
        SwordPathSkillId::HeavenGateCharge => "heaven_gate_charge",
        SwordPathSkillId::HeavenGateRelease => "sword_manifest_strike",
    }
}

fn sword_path_audio_flag(skill: SwordPathSkillId) -> &'static str {
    match skill {
        SwordPathSkillId::CondenseEdge => "sword_path_condense_edge",
        SwordPathSkillId::QiSlash => "sword_path_qi_slash",
        SwordPathSkillId::Resonance => "sword_path_resonance",
        SwordPathSkillId::Manifest => "sword_path_manifest",
        SwordPathSkillId::HeavenGateCharge => "sword_path_heaven_gate_charge",
        SwordPathSkillId::HeavenGateRelease => "sword_path_heaven_gate_release",
    }
}

/// 诱饵分形（回声）签名 recipe——单一真源，供生产 emit 与 `audio::each_signature_skill_*`
/// 运行时消费契约测试共同引用，避免测试另抄一份 recipe id 造成映射漂移假绿。
pub(crate) const ANQI_ECHO_FRACTAL_RECIPE: &str = "anqi_echo_fractal";

/// 暗器六招 cast → `PlaySoundRecipeRequest`，引用 `audio_recipes/anqi_*.json`
/// （全部复用 vanilla 音色分层，无新音频文件）。
///
/// 招式 → 事件源 / recipe 映射：
/// - 封骨（充能）`CarrierChargedEvent` → `anqi_charge_seal`
/// - 单射狙击 `QiInjectionEvent{SingleSnipe}` → `anqi_single_snipe`
/// - 凝魂注射 `QiInjectionEvent{SoulInject}` → `anqi_soul_inject`
/// - 多发齐射 `MultiShotEvent` → `anqi_multi_shot`
/// - 破甲注射 `ArmorPierceEvent` → `anqi_armor_pierce`
/// - 诱饵分形 `DecoyDeployEvent` → `anqi_echo_fractal`
///
/// **纯 cosmetic**：只发音效，不读 / 改任何战斗 / 真元状态。
#[allow(clippy::too_many_arguments)]
pub fn emit_anqi_audio_triggers(
    mut charges: EventReader<CarrierChargedEvent>,
    mut injections: EventReader<QiInjectionEvent>,
    mut multi_shots: EventReader<MultiShotEvent>,
    mut armor_pierces: EventReader<ArmorPierceEvent>,
    mut echoes: EventReader<DecoyDeployEvent>,
    positions: Query<&Position>,
    mut audio: AudioEmitWriter,
) {
    let mut audio = audio.context();

    for event in charges.read() {
        let origin = positions
            .get(event.carrier)
            .map(|p| p.get())
            .unwrap_or_default();
        emit_play(
            &mut audio,
            "anqi_charge_seal",
            event.carrier,
            origin,
            Some("anqi_charge_seal".to_string()),
            1.0,
            0.0,
        );
    }

    for event in injections.read() {
        let origin = positions
            .get(event.caster)
            .map(|p| p.get())
            .unwrap_or_default();
        let (recipe, flag) = match event.skill {
            AnqiSkillId::SingleSnipe => ("anqi_single_snipe", "anqi_single_snipe"),
            AnqiSkillId::SoulInject => ("anqi_soul_inject", "anqi_soul_inject"),
            // MultiShot / ArmorPierce / EchoFractal 走各自 EventReader，不发 QiInjectionEvent。
            _ => continue,
        };
        emit_play(
            &mut audio,
            recipe,
            event.caster,
            origin,
            Some(flag.to_string()),
            1.0,
            0.0,
        );
    }

    for event in multi_shots.read() {
        let origin = positions
            .get(event.caster)
            .map(|p| p.get())
            .unwrap_or_default();
        emit_play(
            &mut audio,
            "anqi_multi_shot",
            event.caster,
            origin,
            Some("anqi_multi_shot".to_string()),
            1.0,
            0.0,
        );
    }

    for event in armor_pierces.read() {
        let origin = positions
            .get(event.caster)
            .map(|p| p.get())
            .unwrap_or_default();
        emit_play(
            &mut audio,
            "anqi_armor_pierce",
            event.caster,
            origin,
            Some("anqi_armor_pierce".to_string()),
            1.0,
            0.0,
        );
    }

    for event in echoes.read() {
        let origin = positions
            .get(event.caster)
            .map(|p| p.get())
            .unwrap_or_default();
        emit_play(
            &mut audio,
            ANQI_ECHO_FRACTAL_RECIPE,
            event.caster,
            origin,
            Some(ANQI_ECHO_FRACTAL_RECIPE.to_string()),
            1.0,
            0.0,
        );
    }
}

/// 蛊道（独孤毒流）cast → `PlaySoundRecipeRequest`，引用 `audio_recipes/dugu_*.json`
/// （除签名 `dugu_poison_signature` 外全部复用 vanilla 音色分层）。
///
/// - 凝针 `QiNeedleChargedEvent` → `dugu_cast`（arrow.shoot：真元凝针远距直刺）
/// - 灌毒蛊 `DuguObfuscationDisruptedEvent` → `dugu_poison_cast`（bee aggressive：失谐真元覆毒）
/// - 倒蚀 `ReverseTriggeredEvent` → `DUGU_POISON_SIGNATURE_RECIPE`（蛊道签名 `bong:skill.dugu.infuse_poison`）
///
/// 倒蚀签名原先内联在 `dugu_v2::skills::apply_reverse`（Pattern B）里发，
/// plan-fpv-cast-av-v1 P5 改为读 cast 事件的独立系统（Pattern A），使 emit-path 可测。
///
/// **纯 cosmetic**：只发音效，不读 / 改任何战斗 / 真元状态。
pub fn emit_dugu_v2_audio_triggers(
    mut needles: EventReader<QiNeedleChargedEvent>,
    mut infusions: EventReader<DuguObfuscationDisruptedEvent>,
    mut reverses: EventReader<ReverseTriggeredEvent>,
    positions: Query<&Position>,
    mut audio: AudioEmitWriter,
) {
    let mut audio = audio.context();

    for event in needles.read() {
        let origin = positions
            .get(event.shooter)
            .map(|position| position.get())
            .unwrap_or(DVec3::ZERO);
        emit_play(
            &mut audio,
            "dugu_cast",
            event.shooter,
            origin,
            Some("dugu_shoot_needle".to_string()),
            1.0,
            0.0,
        );
    }

    for event in infusions.read() {
        let origin = positions
            .get(event.infuser)
            .map(|position| position.get())
            .unwrap_or(DVec3::ZERO);
        emit_play(
            &mut audio,
            "dugu_poison_cast",
            event.infuser,
            origin,
            Some("dugu_infuse_poison".to_string()),
            1.0,
            0.0,
        );
    }

    // 倒蚀签名：**可听字段**（pos / recipient / volume / pitch）与重构前内联 emit 一致——
    // 听者位置发声（`pos: None`，几乎无空间衰减）
    // + 以爆发中心 `event.center` 为圆心的 64 格广播（`emit_play_listener_anchored_broadcast`
    // 的 doc 写了为什么这里不能走 recipe attenuation）。`event.center` = 目标位置，无目标时退到
    // 施法者位置，由 cast 侧算好。
    for event in reverses.read() {
        emit_play_listener_anchored_broadcast(
            &mut audio,
            DUGU_POISON_SIGNATURE_RECIPE,
            event.caster,
            event.center,
            Some("dugu_reverse".to_string()),
        );
    }
}

/// 真脉五招 cast → 各招专属音效配方（`ZhenmaiSkillId::audio_recipe` 单一真源映射）。
///
/// 读 `ZhenmaiSkillCastEvent`（cast 侧 `emit_skill_feedback` 发），音源**无条件**取事件自带的
/// `center`——那是**施法当时**取到的 caster 位置。这里刻意不查实时 `Position`：重构前的内联
/// emit 锁的就是 cast-time 位置，若改读消费时位置，音源会随「事件跨帧才被读到」与「施法后玩家
/// 移动 / 传送」漂移，且依赖未声明的 ECS 生产者-消费者顺序（PR #1262 review 指出）。
///
/// plan-fpv-cast-av-v1 P5：原先音效内联在 `zhenmai_v2::emit_skill_feedback`（Pattern B），
/// 改为本独立系统后「招式实际发出哪条 recipe」可被 emit-path 集成测试锁住。
///
/// **纯 cosmetic**：只发音效，不读 / 改任何战斗 / 真元状态。
pub fn emit_zhenmai_v2_audio_triggers(
    mut casts: EventReader<ZhenmaiSkillCastEvent>,
    mut audio: AudioEmitWriter,
) {
    let mut audio = audio.context();
    for event in casts.read() {
        emit_play(
            &mut audio,
            event.skill.audio_recipe(),
            event.caster,
            event.center,
            None,
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
    mut audio: AudioEmitWriter,
) {
    let mut audio = audio.context();
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
    mut audio: AudioEmitWriter,
) {
    let mut audio = audio.context();
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
    mut pacts: EventReader<SocialPactEvent>,
    mut renown: EventReader<SocialRenownDeltaEvent>,
    mut duo_she_warnings: EventReader<DuoSheWarningEvent>,
    targets: Query<(Entity, &Position, Option<&LifeRecord>, Option<&Lifecycle>)>,
    mut audio: AudioEmitWriter,
) {
    let mut audio = audio.context();
    for event in pacts.read() {
        if event.broken {
            continue;
        }
        let Some((entity, position)) = resolve_audio_target(event.left.as_str(), &targets) else {
            continue;
        };
        emit_play(&mut audio, "pact_bind", entity, position, None, 1.0, 0.0);
    }

    for event in renown.read() {
        let Some((entity, position)) = resolve_audio_target(event.char_id.as_str(), &targets)
        else {
            continue;
        };
        let recipe_id = if event.fame_delta + event.notoriety_delta >= 0 {
            "renown_gain"
        } else {
            "renown_loss"
        };
        emit_play(&mut audio, recipe_id, entity, position, None, 1.0, 0.0);
    }

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

pub fn tick_audio_dedup_clock(dedup: Option<ResMut<AudioImplementationDedup>>) {
    if let Some(mut dedup) = dedup {
        dedup.advance_tick();
    }
}

#[derive(SystemParam)]
pub(crate) struct AudioEmitWriter<'w> {
    audio: EventWriter<'w, PlaySoundRecipeRequest>,
    registry: Option<Res<'w, SoundRecipeRegistry>>,
    dedup: Option<ResMut<'w, AudioImplementationDedup>>,
}

impl<'w> AudioEmitWriter<'w> {
    pub(crate) fn context(&mut self) -> AudioEmitContext<'_, 'w> {
        AudioEmitContext::new(
            &mut self.audio,
            self.registry.as_deref(),
            self.dedup.as_deref_mut(),
        )
    }
}

pub(crate) struct AudioEmitContext<'a, 'w> {
    audio: &'a mut EventWriter<'w, PlaySoundRecipeRequest>,
    registry: Option<&'a SoundRecipeRegistry>,
    dedup: Option<&'a mut AudioImplementationDedup>,
    tick: u64,
}

impl<'a, 'w> AudioEmitContext<'a, 'w> {
    pub(crate) fn new(
        audio: &'a mut EventWriter<'w, PlaySoundRecipeRequest>,
        registry: Option<&'a SoundRecipeRegistry>,
        dedup: Option<&'a mut AudioImplementationDedup>,
    ) -> Self {
        let tick = dedup.as_ref().map_or(0, |dedup| dedup.current_tick());
        Self {
            audio,
            registry,
            dedup,
            tick,
        }
    }

    fn should_emit(&mut self, entity: Entity, recipe_id: &str) -> bool {
        match self.dedup.as_deref_mut() {
            Some(dedup) => dedup.should_emit(entity, recipe_id, self.tick),
            None => true,
        }
    }

    fn recipient(&self, recipe_id: &str, entity: Entity, origin: DVec3) -> AudioRecipient {
        let Some(registry) = self.registry else {
            tracing::warn!(
                "[bong][audio] recipe registry missing while routing recipe `{recipe_id}`"
            );
            return AudioRecipient::Single(entity);
        };
        let Some(recipe) = registry.get(recipe_id) else {
            tracing::warn!(
                "[bong][audio] unknown sound recipe `{recipe_id}` while routing trigger"
            );
            return AudioRecipient::Single(entity);
        };
        recipient_for_attenuation(recipe.attenuation, entity, origin)
    }

    fn send(&mut self, request: PlaySoundRecipeRequest) {
        self.audio.send(request);
    }
}

pub(crate) fn emit_recipe_audio_with_context(
    audio: &mut AudioEmitContext<'_, '_>,
    recipe_id: impl Into<String>,
    entity: Entity,
    origin: DVec3,
    flag: Option<String>,
    volume_mul: f32,
) {
    emit_play(audio, recipe_id, entity, origin, flag, volume_mul, 0.0);
}

fn emit_play(
    audio: &mut AudioEmitContext<'_, '_>,
    recipe_id: impl Into<String>,
    entity: Entity,
    origin: DVec3,
    flag: Option<String>,
    volume_mul: f32,
    pitch_shift: f32,
) {
    let recipe_id = recipe_id.into();
    if !audio.should_emit(entity, &recipe_id) {
        return;
    }
    let recipient = audio.recipient(&recipe_id, entity, origin);
    audio.send(PlaySoundRecipeRequest {
        recipe_id,
        instance_id: 0,
        pos: Some(block_pos(origin)),
        flag,
        volume_mul,
        pitch_shift,
        recipient,
    });
}

fn emit_play_at_block(
    audio: &mut AudioEmitContext<'_, '_>,
    recipe_id: impl Into<String>,
    entity: Entity,
    pos: valence::prelude::BlockPos,
    volume_mul: f32,
) {
    let origin = DVec3::new(f64::from(pos.x), f64::from(pos.y), f64::from(pos.z));
    let recipe_id = recipe_id.into();
    if !audio.should_emit(entity, &recipe_id) {
        return;
    }
    let recipient = audio.recipient(&recipe_id, entity, origin);
    audio.send(PlaySoundRecipeRequest {
        recipe_id,
        instance_id: 0,
        pos: Some([pos.x, pos.y, pos.z]),
        flag: None,
        volume_mul,
        pitch_shift: 0.0,
        recipient,
    });
}

/// 「听者位置 + 广播半径」发声——**只给重构前本就这么发的站点用**，用于把内联 emit 迁到
/// Pattern A 时保持路由逐字段不变。
///
/// 与 `emit_play` 的差别（都不是随便选的）：
/// - `pos: None` → client 把音源放在**听者自己脚下**（`MinecraftSoundSink` 的 fallback，
///   `relative=false` + LINEAR，实际距离仅方块角到耳朵的 1~2 格）⇒ 近满音量、不随距离掉；
///   `emit_play` 的 `Some(block_pos)` 则是世界锚点 + LINEAR 衰减（音量决定可听半径）。
/// - 另有两处非可听差异：新增 `flag`（调试标记；client 只在带 `loop` 的 recipe 上消费，
///   `dugu_poison_signature` 无 loop ⇒ no-op）与 dedup 门（同 entity+recipe 2 tick 内不重发）。
/// - recipient 用固定 `AUDIO_BROADCAST_RADIUS`，不查 recipe 的 `attenuation`。
///
/// 为什么倒蚀签名要走这条：重构前 `dugu_v2::skills::emit_audio` 就是 `pos: None` + 64 格广播；
/// 若改用 `emit_play`，`dugu_poison_signature` 声明的 `MELEE` 会把收听范围砍到 8 格（比该招自己
/// 10 格的 `ReverseAftermathCloud` 还小），再叠上世界锚点的距离衰减与 L0 volume 0.24，实机几乎
/// 听不见——正是 P4 已经吃过两次的「签名进了资产却听不到」（PR #1262 review 抓出）。
/// 要不要把倒蚀改成空间化签名（需同步调 recipe 的 attenuation/volume）留 P5 盲听回归再定。
fn emit_play_listener_anchored_broadcast(
    audio: &mut AudioEmitContext<'_, '_>,
    recipe_id: impl Into<String>,
    entity: Entity,
    origin: DVec3,
    flag: Option<String>,
) {
    let recipe_id = recipe_id.into();
    if !audio.should_emit(entity, &recipe_id) {
        return;
    }
    audio.send(PlaySoundRecipeRequest {
        recipe_id,
        instance_id: 0,
        pos: None,
        flag,
        volume_mul: 1.0,
        pitch_shift: 0.0,
        recipient: AudioRecipient::Radius {
            origin,
            radius: AUDIO_BROADCAST_RADIUS,
        },
    });
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
    use crate::combat::components::{BodyPart, WoundKind, Wounds};
    use crate::combat::events::{CombatEvent, DeathEvent};
    use crate::forge::session::{ForgeSession, ForgeSessionId};
    use valence::prelude::{App, Events, Update};
    use valence::testing::create_mock_client;

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
            debug_command: false,
            physical_damage: 0.0,
            damage: 0.4,
            contam_delta: 0.0,
            description: "test jiemai=true".to_string(),
            defense_kind: Some(DefenseKind::JieMai),
            defense_effectiveness: Some(0.9),
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
        assert_eq!(emitted[0].recipe_id, "parry_perfect");
    }

    #[test]
    fn combat_hit_event_emits_tiered_recipe_and_wound() {
        let mut app = App::new();
        app.init_resource::<AudioImplementationDedup>();
        app.add_event::<CombatEvent>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_combat_audio_triggers);
        let attacker = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
        let target = app.world_mut().spawn(Position::new([1.0, 64.0, 0.0])).id();
        for _ in 0..2 {
            app.world_mut().send_event(CombatEvent {
                attacker,
                target,
                resolved_at_tick: 5,
                body_part: BodyPart::Chest,
                wound_kind: WoundKind::Blunt,
                source: crate::combat::events::AttackSource::Melee,
                debug_command: false,
                physical_damage: 0.0,
                damage: 12.0,
                contam_delta: 0.0,
                description: "test hit tier".to_string(),
                defense_kind: None,
                defense_effectiveness: None,
                defense_contam_reduced: None,
                defense_wound_severity: None,
            });
        }

        app.update();

        let emitted: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .collect();
        let recipes: Vec<_> = emitted.into_iter().map(|event| event.recipe_id).collect();
        assert_eq!(recipes, vec!["hit_heavy", "wound_inflict"]);
    }

    /// plan-combat-hit-location-v1 P3 — 部位差异视听反馈：头部命中即便伤害轻微（低于
    /// hit_heavy 的 10.0 分级线）也要走专属 combat_hit_head_crit，而非退化成 hit_light。
    #[test]
    fn head_hit_event_emits_head_crit_recipe_regardless_of_damage_tier() {
        let mut app = App::new();
        app.init_resource::<AudioImplementationDedup>();
        app.add_event::<CombatEvent>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_combat_audio_triggers);
        let attacker = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
        let target = app.world_mut().spawn(Position::new([1.0, 64.0, 0.0])).id();
        app.world_mut().send_event(CombatEvent {
            attacker,
            target,
            resolved_at_tick: 5,
            body_part: BodyPart::Head,
            wound_kind: WoundKind::Blunt,
            source: crate::combat::events::AttackSource::Melee,
            debug_command: false,
            physical_damage: 0.0,
            // 故意选一个低于 hit_heavy(10.0)/hit_critical(24.0) 分级线的伤害，
            // 证明部位路由优先于伤害分级——不这样测就无法区分"恰好碰上高伤害"的假阳性。
            damage: 2.0,
            contam_delta: 0.0,
            description: "test head crit routes before damage tier".to_string(),
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
        let recipes: Vec<_> = emitted.into_iter().map(|event| event.recipe_id).collect();
        assert_eq!(
            recipes,
            vec!["combat_hit_head_crit"],
            "轻伤头部命中应仍走专属 combat_hit_head_crit，而不是按伤害分级落回 hit_light \
             （命中要害的反馈不该被伤害数值淹没）"
        );
    }

    /// plan-combat-hit-location-v1 P3 — 四肢命中应统一走更闷的 combat_hit_limb，
    /// 四个部位变体（ArmL/ArmR/LegL/LegR）都要命中同一条 recipe。
    #[test]
    fn limb_hit_events_emit_limb_recipe_for_all_four_variants() {
        for limb in [
            BodyPart::ArmL,
            BodyPart::ArmR,
            BodyPart::LegL,
            BodyPart::LegR,
        ] {
            let mut app = App::new();
            app.init_resource::<AudioImplementationDedup>();
            app.add_event::<CombatEvent>();
            app.add_event::<PlaySoundRecipeRequest>();
            app.add_systems(Update, emit_combat_audio_triggers);
            let attacker = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
            let target = app.world_mut().spawn(Position::new([1.0, 64.0, 0.0])).id();
            app.world_mut().send_event(CombatEvent {
                attacker,
                target,
                resolved_at_tick: 5,
                body_part: limb,
                wound_kind: WoundKind::Blunt,
                source: crate::combat::events::AttackSource::Melee,
                debug_command: false,
                physical_damage: 0.0,
                damage: 12.0,
                contam_delta: 0.0,
                description: format!("test limb hit routes to combat_hit_limb for {limb:?}"),
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
            let recipes: Vec<_> = emitted.into_iter().map(|event| event.recipe_id).collect();
            assert_eq!(
                recipes,
                vec!["combat_hit_limb", "wound_inflict"],
                "四肢部位 {limb:?} 命中应路由到 combat_hit_limb（damage=12.0 仍越过 \
                 wound_inflict 的 8.0 阈值，两条 recipe 都应出现）"
            );
        }
    }

    /// 胸/腹/背命中不应受本次部位差异改动影响，维持既有伤害分级 recipe 选择。
    #[test]
    fn torso_hits_still_use_damage_tier_recipe_unaffected_by_body_part_routing() {
        for (part, damage, expected) in [
            (BodyPart::Chest, 3.0, "hit_light"),
            (BodyPart::Abdomen, 12.0, "hit_heavy"),
            (BodyPart::Back, 30.0, "hit_critical"),
        ] {
            let mut app = App::new();
            app.init_resource::<AudioImplementationDedup>();
            app.add_event::<CombatEvent>();
            app.add_event::<PlaySoundRecipeRequest>();
            app.add_systems(Update, emit_combat_audio_triggers);
            let attacker = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
            let target = app.world_mut().spawn(Position::new([1.0, 64.0, 0.0])).id();
            app.world_mut().send_event(CombatEvent {
                attacker,
                target,
                resolved_at_tick: 5,
                body_part: part,
                wound_kind: WoundKind::Blunt,
                source: crate::combat::events::AttackSource::Melee,
                debug_command: false,
                physical_damage: 0.0,
                damage,
                contam_delta: 0.0,
                description: format!("test torso hit {part:?} keeps damage tier recipe"),
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
            assert_eq!(
                emitted[0].recipe_id, expected,
                "{part:?} 命中 damage={damage} 应维持既有分级 recipe {expected}，不受部位差异改动影响"
            );
        }
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
    fn blood_burn_audio_is_player_local() {
        use crate::schema::audio::AudioAttenuation;

        let registry = SoundRecipeRegistry::load_default().expect("default recipes should load");
        assert_eq!(
            registry
                .get("blood_burn_sizzle")
                .expect("blood burn recipe exists")
                .attenuation,
            AudioAttenuation::PlayerLocal,
        );
    }

    fn make_settled(
        entity: valence::prelude::Entity,
        outcome: crate::schema::tribulation::DuXuOutcomeV1,
    ) -> TribulationSettled {
        use crate::cultivation::tribulation::TribulationKind;
        use crate::schema::tribulation::DuXuResultV1;
        TribulationSettled {
            entity,
            kind: TribulationKind::DuXu,
            source: None,
            result: DuXuResultV1 {
                char_id: "test_char".to_string(),
                outcome,
                killer: None,
                waves_survived: 3,
                reason: None,
            },
        }
    }

    #[test]
    fn tribulation_ascended_settled_emits_success_recipe() {
        // Expect: outcome=Ascended → emit_tribulation_audio_triggers emits "tribulation_ascend_success".
        let mut app = App::new();
        app.add_event::<TribulationAnnounce>();
        app.add_event::<JueBiTriggeredEvent>();
        app.add_event::<TribulationWaveCleared>();
        app.add_event::<TribulationFailed>();
        app.add_event::<TribulationSettled>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_tribulation_audio_triggers);
        let entity = app
            .world_mut()
            .spawn(Position::new([10.0, 64.0, 10.0]))
            .id();

        app.world_mut().send_event(make_settled(
            entity,
            crate::schema::tribulation::DuXuOutcomeV1::Ascended,
        ));
        app.update();

        let emitted: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .collect();
        assert_eq!(
            emitted.len(),
            1,
            "expected exactly one PlaySoundRecipeRequest for Ascended outcome, got {}",
            emitted.len()
        );
        assert_eq!(
            emitted[0].recipe_id, "tribulation_ascend_success",
            "Ascended outcome must emit tribulation_ascend_success recipe, got {:?}",
            emitted[0].recipe_id
        );
    }

    #[test]
    fn tribulation_halfstep_settled_emits_success_recipe() {
        // Expect: outcome=HalfStep → same recipe as Ascended (存活通过亦应有成功 AV).
        let mut app = App::new();
        app.add_event::<TribulationAnnounce>();
        app.add_event::<JueBiTriggeredEvent>();
        app.add_event::<TribulationWaveCleared>();
        app.add_event::<TribulationFailed>();
        app.add_event::<TribulationSettled>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_tribulation_audio_triggers);
        let entity = app
            .world_mut()
            .spawn(Position::new([10.0, 64.0, 10.0]))
            .id();

        app.world_mut().send_event(make_settled(
            entity,
            crate::schema::tribulation::DuXuOutcomeV1::HalfStep,
        ));
        app.update();

        let emitted: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .collect();
        assert_eq!(
            emitted.len(),
            1,
            "expected exactly one PlaySoundRecipeRequest for HalfStep outcome, got {}",
            emitted.len()
        );
        assert_eq!(
            emitted[0].recipe_id, "tribulation_ascend_success",
            "HalfStep outcome must emit tribulation_ascend_success recipe, got {:?}",
            emitted[0].recipe_id
        );
    }

    #[test]
    fn tribulation_failed_settled_does_not_emit_success_recipe() {
        // Expect: outcome=Failed → no success AV (failed path handled by TribulationFailed reader).
        let mut app = App::new();
        app.add_event::<TribulationAnnounce>();
        app.add_event::<JueBiTriggeredEvent>();
        app.add_event::<TribulationWaveCleared>();
        app.add_event::<TribulationFailed>();
        app.add_event::<TribulationSettled>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_tribulation_audio_triggers);
        let entity = app
            .world_mut()
            .spawn(Position::new([10.0, 64.0, 10.0]))
            .id();

        app.world_mut().send_event(make_settled(
            entity,
            crate::schema::tribulation::DuXuOutcomeV1::Failed,
        ));
        app.update();

        let emitted: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .collect();
        assert!(
            emitted.is_empty(),
            "Failed outcome must not emit success recipe (handled by TribulationFailed reader), got {:?}",
            emitted.iter().map(|e| &e.recipe_id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn tribulation_killed_and_fled_settled_do_not_emit_success_recipe() {
        // Expect: outcome=Killed/Fled → no success AV (skipped, not gameplay-meaningful success).
        let mut app = App::new();
        app.add_event::<TribulationAnnounce>();
        app.add_event::<JueBiTriggeredEvent>();
        app.add_event::<TribulationWaveCleared>();
        app.add_event::<TribulationFailed>();
        app.add_event::<TribulationSettled>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_tribulation_audio_triggers);
        let entity = app
            .world_mut()
            .spawn(Position::new([10.0, 64.0, 10.0]))
            .id();

        app.world_mut().send_event(make_settled(
            entity,
            crate::schema::tribulation::DuXuOutcomeV1::Killed,
        ));
        app.world_mut().send_event(make_settled(
            entity,
            crate::schema::tribulation::DuXuOutcomeV1::Fled,
        ));
        app.update();

        let emitted: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .collect();
        assert!(
            emitted.is_empty(),
            "Killed/Fled outcomes must not emit success recipe, got {:?}",
            emitted.iter().map(|e| &e.recipe_id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn existing_tribulation_failure_recipe_unaffected_by_settled_reader() {
        // Regression: TribulationFailed still emits realm_regression, adding settled reader
        // must not break the failure audio path.
        let mut app = App::new();
        app.add_event::<TribulationAnnounce>();
        app.add_event::<JueBiTriggeredEvent>();
        app.add_event::<TribulationWaveCleared>();
        app.add_event::<TribulationFailed>();
        app.add_event::<TribulationSettled>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_tribulation_audio_triggers);
        let entity = app
            .world_mut()
            .spawn(Position::new([10.0, 64.0, 10.0]))
            .id();

        use crate::cultivation::tribulation::TribulationFailed;
        app.world_mut()
            .send_event(TribulationFailed { entity, wave: 1 });
        app.update();

        let emitted: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .collect();
        assert_eq!(
            emitted.len(),
            1,
            "TribulationFailed should still emit exactly one recipe after adding settled reader, got {}",
            emitted.len()
        );
        assert_eq!(
            emitted[0].recipe_id, "realm_regression",
            "TribulationFailed must emit realm_regression (regression guard), got {:?}",
            emitted[0].recipe_id
        );
    }

    #[test]
    fn lingtian_actions_emit_dedicated_recipes() {
        let mut app = App::new();
        app.init_resource::<AudioImplementationDedup>();
        app.add_event::<TillCompleted>();
        app.add_event::<PlantingCompleted>();
        app.add_event::<HarvestCompleted>();
        app.add_event::<ReplenishCompleted>();
        app.add_event::<DrainQiCompleted>();
        app.add_event::<RenewCompleted>();
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
        app.world_mut().send_event(TillCompleted {
            player,
            pos,
            hoe: crate::lingtian::hoe::HoeKind::Iron,
            hoe_instance_id: 2,
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
                "lingtian_till",
                "lingtian_plant_seed",
                "lingtian_harvest",
                "lingtian_replenish",
                "lingtian_drain"
            ],
            "regression: existing 5 lingtian sound recipes must not be broken by RenewCompleted addition"
        );
    }

    /// RenewCompleted emits lingtian_replenish audio cue (recipe reuse per scope decision).
    ///
    /// Expectation: emit RenewCompleted → emit_lingtian_audio_triggers →
    /// PlaySoundRecipeRequest with recipe_id="lingtian_replenish" at the correct pos.
    #[test]
    fn renew_completed_emits_lingtian_replenish_audio() {
        let mut app = App::new();
        app.init_resource::<AudioImplementationDedup>();
        app.add_event::<TillCompleted>();
        app.add_event::<PlantingCompleted>();
        app.add_event::<HarvestCompleted>();
        app.add_event::<ReplenishCompleted>();
        app.add_event::<DrainQiCompleted>();
        app.add_event::<RenewCompleted>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_lingtian_audio_triggers);

        let player = app.world_mut().spawn_empty().id();
        let pos = valence::prelude::BlockPos::new(10, 65, -3);

        app.world_mut().send_event(RenewCompleted {
            player,
            pos,
            hoe: crate::lingtian::hoe::HoeKind::Iron,
            hoe_instance_id: 77,
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
            vec!["lingtian_replenish"],
            "expected exactly one lingtian_replenish cue for RenewCompleted \
             (reuses replenish recipe per r2-P1 scope decision), got {recipes:?}"
        );
    }

    /// RenewCompleted: two distinct player entities each emit one lingtian_replenish cue.
    ///
    /// AudioImplementationDedup deduplicates on (entity, recipe_id) per tick window.
    /// Using distinct players confirms both events propagate independently.
    /// (Same-player same-recipe same-tick dedup is tested implicitly by the dedup unit tests.)
    #[test]
    fn multiple_renew_completed_different_players_each_emit_cue() {
        let mut app = App::new();
        app.init_resource::<AudioImplementationDedup>();
        app.add_event::<TillCompleted>();
        app.add_event::<PlantingCompleted>();
        app.add_event::<HarvestCompleted>();
        app.add_event::<ReplenishCompleted>();
        app.add_event::<DrainQiCompleted>();
        app.add_event::<RenewCompleted>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_lingtian_audio_triggers);

        let player_a = app.world_mut().spawn_empty().id();
        let player_b = app.world_mut().spawn_empty().id();
        let pos = valence::prelude::BlockPos::new(0, 64, 0);

        app.world_mut().send_event(RenewCompleted {
            player: player_a,
            pos,
            hoe: crate::lingtian::hoe::HoeKind::Iron,
            hoe_instance_id: 1,
        });
        app.world_mut().send_event(RenewCompleted {
            player: player_b,
            pos,
            hoe: crate::lingtian::hoe::HoeKind::Iron,
            hoe_instance_id: 2,
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
            vec!["lingtian_replenish", "lingtian_replenish"],
            "expected one lingtian_replenish cue per distinct player entity, got {recipes:?}"
        );
    }

    #[test]
    fn alchemy_events_emit_dedicated_recipes() {
        let mut app = App::new();
        app.add_event::<StartAlchemyRequest>();
        app.add_event::<AlchemyOutcomeEvent>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_alchemy_audio_triggers);
        let furnace = app.world_mut().spawn(Position::new([3.0, 64.0, -2.0])).id();

        app.world_mut().send_event(StartAlchemyRequest {
            furnace,
            recipe_id: "hui_yuan_pill_v0".to_string(),
            caster_id: "offline:Azure".to_string(),
        });
        app.world_mut().send_event(AlchemyOutcomeEvent {
            furnace,
            caster_id: "offline:Azure".to_string(),
            recipe_id: Some("hui_yuan_pill_v0".to_string()),
            bucket: crate::alchemy::outcome::OutcomeBucket::Perfect,
            outcome: ResolvedOutcome::Pill {
                recipe_id: "hui_yuan_pill_v0".to_string(),
                pill: "hui_yuan_pill".to_string(),
                quality: 1.0,
                toxin_amount: 0.0,
                toxin_color: crate::cultivation::components::ColorKind::Mellow,
                qi_gain: Some(24.0),
                quality_tier: 3,
                effect_multiplier: 1.0,
                consecrated: true,
                side_effect: None,
                flawed_path: false,
            },
            elapsed_ticks: 120,
        });

        app.update();

        let recipes: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .map(|request| request.recipe_id)
            .collect();
        assert_eq!(recipes, vec!["alchemy_bubble", "alchemy_complete"]);
    }

    #[test]
    fn forge_events_emit_dedicated_recipes() {
        let mut app = App::new();
        app.add_event::<ForgeStartAccepted>();
        app.add_event::<TemperingHit>();
        app.add_event::<ForgeOutcomeEvent>();
        app.init_resource::<ForgeSessions>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_forge_audio_triggers);
        let station = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
        let caster = app.world_mut().spawn(Position::new([1.0, 64.0, 0.0])).id();
        let session_id = ForgeSessionId(1);
        let mut session = ForgeSession::new(session_id, "forge_test".to_string(), station, caster);
        session.current_step = ForgeStep::Tempering;
        app.world_mut()
            .resource_mut::<ForgeSessions>()
            .insert(session);

        app.world_mut().send_event(ForgeStartAccepted {
            session: session_id,
            station,
            caster,
            blueprint: "forge_test".to_string(),
            materials: vec![],
        });
        app.world_mut().send_event(TemperingHit {
            session: session_id,
            beat: TemperBeat::Heavy,
            ticks_remaining: 2,
        });
        app.world_mut().send_event(ForgeOutcomeEvent {
            session: session_id,
            caster,
            blueprint: "forge_test".to_string(),
            bucket: ForgeBucket::Perfect,
            weapon_item: None,
            quality: 1.0,
            color: None,
            side_effects: vec![],
            achieved_tier: 3,
            consecration_qi_amount: 0.0,
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
            vec!["forge_consecrate", "forge_hammer_heavy", "forge_complete"]
        );
    }

    #[test]
    fn player_state_audio_uses_twenty_percent_low_hp_threshold() {
        let mut app = App::new();
        app.init_resource::<AudioTriggerState>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_player_state_audio_triggers);
        let (mut bundle, _helper) = create_mock_client("low_hp");
        bundle.player.position = Position::new([0.0, 64.0, 0.0]);
        let player = app.world_mut().spawn(bundle).id();
        app.world_mut().entity_mut(player).insert(Wounds {
            health_current: 25.0,
            health_max: 100.0,
            ..Default::default()
        });

        app.update();
        let first: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .collect();
        assert!(
            first.is_empty(),
            "25% HP should not trigger the audio-world heartbeat"
        );

        app.world_mut().entity_mut(player).insert(Wounds {
            health_current: 19.0,
            health_max: 100.0,
            ..Default::default()
        });
        app.update();

        let emitted: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].recipe_id, "heartbeat_low_hp");
        assert_eq!(emitted[0].flag.as_deref(), Some("hp_below_20"));
    }

    #[test]
    fn meridian_open_event_emits_chime_recipe() {
        let mut app = App::new();
        app.add_event::<BreakthroughOutcome>();
        app.add_event::<MeridianOpenedEvent>();
        app.add_event::<RealmRegressed>();
        app.add_event::<MeridianOverloadEvent>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_cultivation_audio_triggers);
        let player = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(MeridianOpenedEvent {
            entity: player,
            origin: DVec3::new(3.0, 64.0, -2.0),
        });

        app.update();

        let emitted: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].recipe_id, "meridian_open");
        assert!(matches!(emitted[0].recipient, AudioRecipient::Single(entity) if entity == player));
    }

    #[test]
    fn duo_she_warning_matches_life_record_target() {
        let mut app = App::new();
        app.add_event::<SocialPactEvent>();
        app.add_event::<SocialRenownDeltaEvent>();
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

    #[test]
    fn social_pact_and_renown_emit_audio() {
        let mut app = App::new();
        app.insert_resource(SoundRecipeRegistry::load_default().expect("default recipes load"));
        app.add_event::<SocialPactEvent>();
        app.add_event::<SocialRenownDeltaEvent>();
        app.add_event::<DuoSheWarningEvent>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_social_audio_triggers);
        let target = app
            .world_mut()
            .spawn((
                Position::new([3.0, 64.0, 3.0]),
                LifeRecord::new("offline:Azure"),
            ))
            .id();

        app.world_mut().send_event(SocialPactEvent {
            left: "offline:Azure".to_string(),
            right: "offline:Night".to_string(),
            terms: "teach me the bind".to_string(),
            tick: 1,
            broken: false,
            breaker: None,
            witnesses: vec![],
        });
        app.world_mut().send_event(SocialRenownDeltaEvent {
            char_id: "offline:Azure".to_string(),
            identity_id: None,
            fame_delta: 2,
            notoriety_delta: 0,
            tags_added: vec![],
            tick: 2,
            reason: "test".to_string(),
        });

        app.update();

        let emitted: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .collect();
        let recipes: Vec<_> = emitted
            .iter()
            .map(|request| request.recipe_id.as_str())
            .collect();
        assert_eq!(recipes, vec!["pact_bind", "renown_gain"]);
        assert!(matches!(
            emitted[0].recipient,
            AudioRecipient::Radius { origin, .. } if origin == Position::new([3.0, 64.0, 3.0]).get()
        ));
        assert!(matches!(
            emitted[1].recipient,
            AudioRecipient::Single(entity) if entity == target
        ));
    }

    // ─── plan-sword-path-v2 P4：emit_sword_path_audio_triggers ───

    fn sword_path_cast_event(
        skill: SwordPathSkillId,
        caster: Entity,
        center: DVec3,
    ) -> SwordPathSkillCastEvent {
        SwordPathSkillCastEvent {
            skill,
            caster,
            center,
            direction: None,
            tick: 10,
        }
    }

    fn setup_sword_path_audio_app() -> App {
        let mut app = App::new();
        app.init_resource::<AudioImplementationDedup>();
        app.add_event::<SwordPathSkillCastEvent>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_sword_path_audio_triggers);
        app
    }

    /// 五招各 emit 其专属音效配方 + 专属 flag（按招式 dedup）。
    #[test]
    fn sword_path_skills_emit_dedicated_recipes() {
        let mut app = setup_sword_path_audio_app();
        let caster = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
        let center = DVec3::new(0.0, 64.0, 0.0);

        for skill in [
            SwordPathSkillId::CondenseEdge,
            SwordPathSkillId::QiSlash,
            SwordPathSkillId::Resonance,
            SwordPathSkillId::Manifest,
            SwordPathSkillId::HeavenGateCharge,
            SwordPathSkillId::HeavenGateRelease,
        ] {
            app.world_mut()
                .send_event(sword_path_cast_event(skill, caster, center));
        }
        app.update();

        let emitted: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .collect();
        let recipes: Vec<_> = emitted.iter().map(|e| e.recipe_id.as_str()).collect();
        assert_eq!(
            recipes,
            vec![
                "sword_condense_edge",
                "sword_qi_slash",
                "sword_resonance",
                "sword_manifest_summon",
                "heaven_gate_charge",
                "sword_manifest_strike",
            ],
            "六个 cast 阶段（含天门 charge/release）各应 emit 其专属配方"
        );
        // flag 也按招式区分，便于 client HUD / dedup
        let flags: Vec<_> = emitted
            .iter()
            .map(|e| e.flag.as_deref().unwrap_or(""))
            .collect();
        assert_eq!(
            flags,
            vec![
                "sword_path_condense_edge",
                "sword_path_qi_slash",
                "sword_path_resonance",
                "sword_path_manifest",
                "sword_path_heaven_gate_charge",
                "sword_path_heaven_gate_release",
            ]
        );
    }

    /// 所有引用的剑道配方都必须在 server SoundRecipeRegistry 注册（否则路由 fallback 报 warn）。
    #[test]
    fn all_referenced_sword_path_recipes_exist_in_registry() {
        let registry = SoundRecipeRegistry::load_default().expect("default recipes should load");
        for skill in [
            SwordPathSkillId::CondenseEdge,
            SwordPathSkillId::QiSlash,
            SwordPathSkillId::Resonance,
            SwordPathSkillId::Manifest,
            SwordPathSkillId::HeavenGateCharge,
            SwordPathSkillId::HeavenGateRelease,
        ] {
            let recipe_id = super::sword_path_recipe_for_skill(skill);
            assert!(
                registry.get(recipe_id).is_some(),
                "剑道配方 `{recipe_id}`（招式 {skill:?}）必须在 server registry 注册，\
                 否则 recipient() fallback 到 Single 并 warn——server 与 client 音效资产脱节"
            );
        }
    }

    /// caster 无 Position → 落到 event.center，仍 emit 音效（不哑）。
    #[test]
    fn sword_path_audio_falls_back_to_center_without_position() {
        let mut app = setup_sword_path_audio_app();
        // caster 没有 Position component
        let caster = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(sword_path_cast_event(
            SwordPathSkillId::QiSlash,
            caster,
            DVec3::new(7.0, 64.0, 7.0),
        ));
        app.update();

        let emitted: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .collect();
        assert_eq!(emitted.len(), 1, "无 Position 也应出招声（落到 center）");
        assert_eq!(emitted[0].recipe_id, "sword_qi_slash");
        assert_eq!(
            emitted[0].pos,
            Some([7, 64, 7]),
            "无 Position 时音源应落到 event.center"
        );
    }

    // ─── 崩脉签名：emit_baomai_v3_audio_triggers（运行时消费 emit-path 覆盖）───

    fn setup_baomai_audio_app() -> App {
        let mut app = App::new();
        app.init_resource::<AudioImplementationDedup>();
        app.add_event::<BaomaiSkillEvent>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_baomai_v3_audio_triggers);
        app
    }

    /// 崩脉签名 `full_power_release` 经**真实 emit 系统** emit `baomai_signature`。
    /// 「运行时消费」emit-path 门：跑 `emit_baomai_v3_audio_triggers` 读 `BaomaiSkillEvent`
    /// → `baomai_recipe_for_skill` → 发 `PlaySoundRecipeRequest`；删掉发声调用 / 改坏 skill→recipe
    /// 映射都会撞红（补 `audio::each_signature_skill_*` 静态 pin 之外的 emit 断链覆盖）。
    #[test]
    fn baomai_full_power_release_emits_signature_recipe() {
        let mut app = setup_baomai_audio_app();
        let caster = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
        app.world_mut().send_event(BaomaiSkillEvent {
            skill: BaomaiSkillId::FullPowerRelease,
            caster,
            target: None,
            tick: 1,
            qi_invested: 0.0,
            damage: 0.0,
            radius_blocks: None,
            blood_multiplier: 0.0,
            flow_rate_multiplier: 0.0,
            meridian_dependencies: Vec::new(),
        });
        app.update();

        let emitted: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .collect();
        let recipes: Vec<_> = emitted.iter().map(|e| e.recipe_id.as_str()).collect();
        assert_eq!(
            recipes,
            vec!["baomai_signature"],
            "崩脉 full_power_release 应经 emit 系统实发 baomai_signature，实际 {recipes:?}"
        );
    }

    // ─── 涡流签名：emit_woliu_v2_audio_triggers（运行时消费 emit-path 覆盖）───

    fn setup_woliu_audio_app() -> App {
        let mut app = App::new();
        app.init_resource::<AudioImplementationDedup>();
        app.add_event::<VortexCastEvent>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_woliu_v2_audio_triggers);
        app
    }

    /// 涡流签名 `void_core` 经**真实 emit 系统** emit `woliu_void_core`（经 `event.visual.sound_recipe_id`）。
    /// 「运行时消费」emit-path 门：跑 `emit_woliu_v2_audio_triggers` 读 `VortexCastEvent` → 发
    /// `PlaySoundRecipeRequest`；删掉发声调用 / visual→recipe 映射漂移都会撞红。
    #[test]
    fn woliu_void_core_emits_signature_recipe() {
        use crate::combat::woliu_v2::skills::visual_for;
        use crate::combat::woliu_v2::WoliuSkillId;

        let mut app = setup_woliu_audio_app();
        let caster = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
        app.world_mut().send_event(VortexCastEvent {
            caster,
            skill: WoliuSkillId::VoidCore,
            tick: 1,
            center: DVec3::new(0.0, 64.0, 0.0),
            lethal_radius: 0.0,
            influence_radius: 0.0,
            turbulence_radius: 0.0,
            absorbed_qi: 0.0,
            swirl_qi: 0.0,
            backfire_level: None,
            visual: visual_for(WoliuSkillId::VoidCore),
        });
        app.update();

        let emitted: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .collect();
        let recipes: Vec<_> = emitted.iter().map(|e| e.recipe_id.as_str()).collect();
        assert_eq!(
            recipes,
            vec!["woliu_void_core"],
            "涡流 void_core 应经 emit 系统实发 woliu_void_core（event.visual.sound_recipe_id），实际 {recipes:?}"
        );
    }

    // ─── 蜕壳签名（被动路径）：emit_tuike_v2_audio_triggers（运行时消费 emit-path 覆盖）───

    fn setup_tuike_audio_app() -> App {
        use crate::combat::tuike_v2::events::{
            ContamTransferredEvent, DonFalseSkinEvent, FalseSkinSheddedEvent,
        };
        let mut app = App::new();
        app.init_resource::<AudioImplementationDedup>();
        app.add_event::<DonFalseSkinEvent>();
        app.add_event::<FalseSkinSheddedEvent>();
        app.add_event::<ContamTransferredEvent>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_tuike_v2_audio_triggers);
        app
    }

    /// tuike.shed **被动蜕壳**（`FalseSkinSheddedEvent`）经真实 emit 系统实发签名 `shed_skin_burst`
    /// （经 `event.visual.sound_recipe_id`）。「运行时消费」emit-path 门（Pattern A 侧）；主动施法
    /// `cast_shed`（Pattern B 内联在 cast 逻辑）待 P5 重构为 Pattern A 后补。
    #[test]
    fn tuike_shed_passive_emits_signature_recipe() {
        use crate::combat::tuike_v2::events::{
            FalseSkinSheddedEvent, TuikeSkillId, TuikeSkillVisual,
        };
        use crate::combat::tuike_v2::FalseSkinTier;

        let mut app = setup_tuike_audio_app();
        let owner = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
        app.world_mut().send_event(FalseSkinSheddedEvent {
            owner,
            attacker: None,
            tier: FalseSkinTier::Light,
            damage_absorbed: 0.0,
            damage_overflow: 0.0,
            contam_load: 0.0,
            permanent_taint_load: 0.0,
            layers_after: 0,
            active: true,
            tick: 1,
            visual: TuikeSkillVisual::for_skill(TuikeSkillId::Shed, false).into(),
        });
        app.update();

        let emitted: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .collect();
        let recipes: Vec<_> = emitted.iter().map(|e| e.recipe_id.as_str()).collect();
        assert_eq!(
            recipes,
            vec!["shed_skin_burst"],
            "tuike 被动蜕壳应经 emit 系统实发 shed_skin_burst（event.visual.sound_recipe_id），实际 {recipes:?}"
        );
    }

    // ─── 生产接线门禁：跑真实注册入口，不在测试里另抄系统清单 ───

    /// 收集某个 App 的 `Update` 调度里所有系统名（`ScheduleGraph` 在 `add_systems` 时即填充，
    /// 无需初始化/运行，故不受「缺 Events 资源会 panic」影响）。
    fn update_schedule_system_names(app: &App) -> Vec<String> {
        app.get_schedule(Update)
            .expect("Update 调度应存在")
            .graph()
            .systems()
            .map(|(_, system, _)| system.name().to_string())
            .collect()
    }

    /// **接线门禁**：跑**生产**注册入口 `audio_trigger::register`，断言三条签名链的 emit 系统
    /// 真的进了 `Update` 调度（外加 dedup 时钟与既有 Pattern A 家族）。
    ///
    /// 这是「系统没接线也撞红」的那道门：本测试不自己 `add_systems` 被测函数，而是调生产函数，
    /// 从 `register` 里删掉任一系统即撞红（已变异验证）。残余接缝：`network::register` 里对
    /// 本函数的那一行调用无法在单测覆盖——调它会拉起 Redis bridge 线程。
    #[test]
    fn register_wires_all_audio_trigger_systems() {
        let mut app = App::new();
        super::register(&mut app);

        let names = update_schedule_system_names(&app);
        for expected in [
            "tick_audio_dedup_clock",
            // P5 emit 架构统一的三条签名链
            "emit_zhenmai_v2_audio_triggers",
            "emit_dugu_v2_audio_triggers",
            "emit_tuike_v2_audio_triggers",
            // 既有 Pattern A 家族（同一注册入口，防提取时漏搬）
            "emit_sword_path_audio_triggers",
            "emit_baomai_v3_audio_triggers",
            "emit_woliu_v2_audio_triggers",
            "emit_woliu_v1_vortex_audio_triggers",
            "emit_anqi_audio_triggers",
            "emit_combat_audio_triggers",
            "emit_npc_death_audio_triggers",
            "emit_cultivation_audio_triggers",
            "emit_tribulation_audio_triggers",
            "emit_alchemy_audio_triggers",
            "emit_forge_audio_triggers",
            "emit_botany_audio_triggers",
            "emit_lingtian_audio_triggers",
            "emit_skill_audio_triggers",
            "emit_social_audio_triggers",
            "emit_player_state_audio_triggers",
        ] {
            assert!(
                names.iter().any(|name| name.ends_with(expected)),
                "生产 audio_trigger::register 应把 `{expected}` 注册进 Update 调度——\
                 没进调度 = 运行时永不触发（功能孤岛），实际注册了 {names:?}"
            );
        }
    }

    /// **接线门禁（事件侧）**：三条签名链读的事件必须由**各自模块的生产 `register`** 装进 World，
    /// 否则 cast 侧 `world.send_event` 会静默丢弃 → 实机零签名音。
    ///
    /// 测试调生产 register 而非自己 `add_event`：从生产 register 里删掉 `add_event` 即撞红
    /// （已变异验证）。
    #[test]
    fn production_module_registers_install_signature_cast_events() {
        use crate::combat::dugu_v2::ReverseTriggeredEvent;
        use crate::combat::tuike_v2::FalseSkinSheddedEvent;
        use crate::combat::zhenmai_v2::ZhenmaiSkillCastEvent;

        let mut app = App::new();
        crate::combat::zhenmai_v2::register(&mut app);
        assert!(
            app.world()
                .contains_resource::<Events<ZhenmaiSkillCastEvent>>(),
            "zhenmai_v2::register 必须 add_event::<ZhenmaiSkillCastEvent>()——\
             缺它则 emit_skill_feedback 的 send_event 被静默丢弃，实机零招式音"
        );

        let mut app = App::new();
        crate::combat::dugu_v2::register(&mut app);
        assert!(
            app.world()
                .contains_resource::<Events<ReverseTriggeredEvent>>(),
            "dugu_v2::register 必须 add_event::<ReverseTriggeredEvent>()——缺它则倒蚀签名音无事件可读"
        );

        let mut app = App::new();
        crate::combat::tuike_v2::register(&mut app);
        assert!(
            app.world()
                .contains_resource::<Events<FalseSkinSheddedEvent>>(),
            "tuike_v2::register 必须 add_event::<FalseSkinSheddedEvent>()——\
             缺它则主动 / 被动蜕壳签名音都无事件可读"
        );
    }

    // ─── 真脉五招：emit_zhenmai_v2_audio_triggers（P5 emit 架构统一）───

    fn setup_zhenmai_audio_app() -> App {
        let mut app = App::new();
        app.init_resource::<AudioImplementationDedup>();
        app.add_event::<ZhenmaiSkillCastEvent>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_zhenmai_v2_audio_triggers);
        app
    }

    fn drain_recipes(app: &mut App) -> Vec<String> {
        app.world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .map(|event| event.recipe_id)
            .collect()
    }

    /// 真脉五招逐招经**真实 emit 系统**实发自己的 recipe（期望值调生产映射
    /// `ZhenmaiSkillId::audio_recipe` 得到，测试内不另抄表）。
    ///
    /// 「运行时消费」emit-path 门：跑 `emit_zhenmai_v2_audio_triggers` 读 `ZhenmaiSkillCastEvent`
    /// → 发 `PlaySoundRecipeRequest`。锁的是「事件被吃掉 / 串到别招的 recipe / 多发漏发」；
    /// **映射表本身写错**（如 sever_chain 指向别的 recipe）不由本测试锁——期望值与生产读同一张表，
    /// 那条由 `audio::each_signature_skill_actually_emitted_recipe_swaps_l0_to_its_bong_event`
    /// 的 registry 内容 pin 覆盖。含签名招 `sever_chain`（`zhenmai_sever_crack`）。
    #[test]
    fn zhenmai_skills_emit_their_mapped_recipes() {
        use crate::combat::zhenmai_v2::ZhenmaiSkillId;

        for skill in [
            ZhenmaiSkillId::Parry,
            ZhenmaiSkillId::Neutralize,
            ZhenmaiSkillId::MultiPoint,
            ZhenmaiSkillId::HardenMeridian,
            ZhenmaiSkillId::SeverChain,
        ] {
            let mut app = setup_zhenmai_audio_app();
            let caster = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
            app.world_mut().send_event(ZhenmaiSkillCastEvent {
                caster,
                skill,
                center: DVec3::new(0.0, 64.0, 0.0),
            });
            app.update();

            let recipes = drain_recipes(&mut app);
            assert_eq!(
                recipes,
                vec![skill.audio_recipe().to_string()],
                "真脉 {skill:?} 应经 emit 系统实发 `{}`（生产映射 ZhenmaiSkillId::audio_recipe），实际 {recipes:?}",
                skill.audio_recipe()
            );
        }
    }

    /// 真脉音源锁 **cast-time 语义**：音源恒为事件自带的 `center`（施法当时的位置），与重构前
    /// 内联 emit 一致——不受「事件跨帧才被读到」「施法后玩家移动 / 传送」影响，也不依赖未声明的
    /// ECS 生产者-消费者顺序（PR #1262 review 指出的行为回归，本测试即其回归门）。
    ///
    /// 两条边界一起锁：① 事件发出后 caster 传送到远处，音源仍是施法点；
    /// ② caster 根本没有 `Position`（断线 / 未同步）时照样发声、音源仍是事件 center。
    #[test]
    fn zhenmai_audio_uses_cast_time_center_not_live_position() {
        use crate::combat::zhenmai_v2::ZhenmaiSkillId;

        let mut app = setup_zhenmai_audio_app();
        let moved_caster = app.world_mut().spawn(Position::new([7.5, 64.0, -3.5])).id();
        let positionless_caster = app.world_mut().spawn(()).id();
        app.world_mut().send_event(ZhenmaiSkillCastEvent {
            caster: moved_caster,
            skill: ZhenmaiSkillId::SeverChain,
            center: DVec3::new(7.5, 64.0, -3.5),
        });
        app.world_mut().send_event(ZhenmaiSkillCastEvent {
            caster: positionless_caster,
            skill: ZhenmaiSkillId::Parry,
            center: DVec3::new(-20.5, 70.0, 11.5),
        });
        // 施法之后、音效系统跑之前，caster 传送到很远处（跨帧消费 / 玩家继续跑动的模型）。
        app.world_mut()
            .entity_mut(moved_caster)
            .insert(Position::new([900.0, 12.0, -900.0]));
        app.update();

        let emitted: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .collect();
        assert_eq!(emitted.len(), 2, "两条 cast 事件应各发一条音效");
        assert_eq!(
            emitted[0].recipe_id,
            ZhenmaiSkillId::SeverChain.audio_recipe()
        );
        assert_eq!(
            emitted[0].pos,
            Some([7, 64, -4]),
            "音源必须锁 cast-time center（施法点），不得跟着 caster 移动后的实时 Position 漂移"
        );
        assert_eq!(emitted[1].recipe_id, ZhenmaiSkillId::Parry.audio_recipe());
        assert_eq!(
            emitted[1].pos,
            Some([-21, 70, 11]),
            "caster 无 Position 时同样用 event.center，不静默丢招式声"
        );
    }

    // ─── 蛊道倒蚀签名：emit_dugu_v2_audio_triggers（P5 emit 架构统一）───

    fn setup_dugu_audio_app() -> App {
        let mut app = App::new();
        app.init_resource::<AudioImplementationDedup>();
        app.add_event::<QiNeedleChargedEvent>();
        app.add_event::<DuguObfuscationDisruptedEvent>();
        app.add_event::<ReverseTriggeredEvent>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_dugu_v2_audio_triggers);
        app
    }

    fn reverse_event(caster: Entity, center: DVec3) -> ReverseTriggeredEvent {
        use crate::combat::dugu_v2::events::DuguSkillId;
        use crate::combat::dugu_v2::skills::visual_for;

        ReverseTriggeredEvent {
            caster,
            affected_targets: 1,
            burst_damage: 12.0,
            returned_zone_qi: 1.0,
            juebi_delay_ticks: None,
            tick: 1,
            center,
            visual: visual_for(DuguSkillId::Reverse),
        }
    }

    /// 蛊道签名（倒蚀 `ReverseTriggeredEvent`）经**真实 emit 系统**实发 `dugu_poison_signature`
    /// （recipe id 引生产 const `DUGU_POISON_SIGNATURE_RECIPE` 单一真源）。
    ///
    /// 「运行时消费」emit-path 门：原先内联在 `apply_reverse`（Pattern B）无法独立驱动，
    /// P5 改为本系统读 cast 事件后可锁——删掉发声调用 / 系统没接线都撞红。
    #[test]
    fn dugu_reverse_emits_signature_recipe() {
        let mut app = setup_dugu_audio_app();
        let caster = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
        app.world_mut()
            .send_event(reverse_event(caster, DVec3::new(3.0, 64.0, 4.0)));
        app.update();

        let emitted: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .collect();
        let recipes: Vec<_> = emitted.iter().map(|e| e.recipe_id.as_str()).collect();
        assert_eq!(
            recipes,
            vec![DUGU_POISON_SIGNATURE_RECIPE],
            "蛊道倒蚀应经 emit 系统实发 {DUGU_POISON_SIGNATURE_RECIPE}，实际 {recipes:?}"
        );
        // 路由锁（PR #1262 review）：重构前该站点是「听者位置发声 + 以爆发中心为圆心的 64 格广播」。
        // 若哪天「顺手统一」成 emit_play，recipe 声明的 MELEE 会把收听范围砍到 8 格、再叠上世界锚点
        // 的距离衰减（L0 volume 0.24）→ 实机几乎听不见，本断言即撞红。
        assert_eq!(
            emitted[0].pos, None,
            "倒蚀签名应 pos=None（听者位置、无空间衰减），与重构前内联 emit 一致"
        );
        assert_eq!(
            emitted[0].recipient,
            AudioRecipient::Radius {
                origin: DVec3::new(3.0, 64.0, 4.0),
                radius: AUDIO_BROADCAST_RADIUS,
            },
            "倒蚀签名收听范围应是以爆发中心为圆心的 64 格广播（重构前语义），\
             不得退化成 recipe attenuation 的 MELEE 8 格"
        );
    }

    /// 蛊道三条 reader（凝针 / 灌毒蛊 / 倒蚀签名）同帧全触发时互不串味：各发各的 recipe，
    /// 不多不少三条。
    #[test]
    fn dugu_three_readers_do_not_cross_talk() {
        let mut app = setup_dugu_audio_app();
        let caster = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
        app.world_mut().send_event(QiNeedleChargedEvent {
            shooter: caster,
            target: None,
            tick: 1,
        });
        app.world_mut().send_event(DuguObfuscationDisruptedEvent {
            infuser: caster,
            until_tick: 40,
        });
        app.world_mut()
            .send_event(reverse_event(caster, DVec3::new(0.0, 64.0, 0.0)));
        app.update();

        let mut recipes = drain_recipes(&mut app);
        recipes.sort();
        let mut expected = vec![
            "dugu_cast".to_string(),
            "dugu_poison_cast".to_string(),
            DUGU_POISON_SIGNATURE_RECIPE.to_string(),
        ];
        expected.sort();
        assert_eq!(
            recipes, expected,
            "凝针 → dugu_cast、灌毒蛊 → dugu_poison_cast、倒蚀 → 签名 \
             {DUGU_POISON_SIGNATURE_RECIPE}，三条各一不缺不重（顺序非契约，按 recipe 排序对拍），\
             实际 {recipes:?}"
        );
    }

    // ─── 暗器六招：emit_anqi_audio_triggers ───────────────────────

    fn setup_anqi_audio_app() -> App {
        use crate::combat::anqi_v2::{ArmorPierceEvent, MultiShotEvent, QiInjectionEvent};
        use crate::combat::carrier::CarrierChargedEvent;

        let mut app = App::new();
        app.init_resource::<AudioImplementationDedup>();
        app.add_event::<CarrierChargedEvent>();
        app.add_event::<QiInjectionEvent>();
        app.add_event::<MultiShotEvent>();
        app.add_event::<ArmorPierceEvent>();
        app.add_event::<crate::combat::anqi_v2::DecoyDeployEvent>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_anqi_audio_triggers);
        app
    }

    fn injection_outcome() -> crate::qi_physics::HighDensityInjectionOutcome {
        crate::qi_physics::HighDensityInjectionOutcome {
            payload_qi: 50.0,
            wound_qi: 40.0,
            contamination_qi: 5.0,
            overload_ratio: 0.5,
            triggers_overload_tear: false,
        }
    }

    fn armor_outcome() -> crate::qi_physics::ArmorPenetrationOutcome {
        crate::qi_physics::ArmorPenetrationOutcome {
            base_damage: 60.0,
            ignored_defense_ratio: 0.6,
            effective_damage: 70.0,
            carrier_shatter_probability: 0.2,
        }
    }

    /// 暗器六招各 emit 其专属 recipe（封骨/狙击/齐射/魂注/破甲/分形）。
    #[test]
    fn anqi_skills_emit_dedicated_recipes() {
        use crate::combat::anqi_v2::{
            AnqiSkillId, ArmorPierceEvent, DecoyDeployEvent, MultiShotEvent, QiInjectionEvent,
        };
        use crate::combat::carrier::{CarrierChargedEvent, CarrierKind};
        use crate::cultivation::components::ColorKind;

        let mut app = setup_anqi_audio_app();
        let caster = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();

        app.world_mut().send_event(CarrierChargedEvent {
            carrier: caster,
            instance_id: 1,
            qi_amount: 25.0,
            qi_color: ColorKind::Solid,
            full_charge: true,
            tick: 10,
        });
        app.world_mut().send_event(QiInjectionEvent {
            caster,
            target: None,
            skill: AnqiSkillId::SingleSnipe,
            carrier_kind: CarrierKind::YibianShougu,
            outcome: injection_outcome(),
            tick: 11,
        });
        app.world_mut().send_event(QiInjectionEvent {
            caster,
            target: None,
            skill: AnqiSkillId::SoulInject,
            carrier_kind: CarrierKind::DyedBone,
            outcome: injection_outcome(),
            tick: 12,
        });
        app.world_mut().send_event(MultiShotEvent {
            caster,
            projectile_count: 5,
            carrier_kind: CarrierKind::LingmuArrow,
            shots: Vec::new(),
            tick: 13,
        });
        app.world_mut().send_event(ArmorPierceEvent {
            caster,
            target: None,
            carrier_kind: CarrierKind::FenglingheBone,
            outcome: armor_outcome(),
            tick: 14,
        });
        app.world_mut().send_event(DecoyDeployEvent {
            caster,
            echo_count: 3,
            tick: 15,
        });
        app.update();

        let emitted: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .collect();
        let mut recipes: Vec<_> = emitted.iter().map(|e| e.recipe_id.as_str()).collect();
        recipes.sort_unstable();
        let mut expected = vec![
            "anqi_charge_seal",
            "anqi_single_snipe",
            "anqi_soul_inject",
            "anqi_multi_shot",
            "anqi_armor_pierce",
            "anqi_echo_fractal",
        ];
        expected.sort_unstable();
        assert_eq!(
            recipes, expected,
            "暗器六招各应 emit 其专属 recipe，实际 {recipes:?}"
        );
    }

    /// QiInjectionEvent 的 MultiShot/ArmorPierce/EchoFractal 分支不发声（走各自 EventReader）。
    #[test]
    fn anqi_injection_only_handles_snipe_and_soul() {
        use crate::combat::anqi_v2::{AnqiSkillId, QiInjectionEvent};
        use crate::combat::carrier::CarrierKind;

        let mut app = setup_anqi_audio_app();
        let caster = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
        // 故意发一个 MultiShot 标签的 QiInjectionEvent（实际生产不会，但守语义边界）
        app.world_mut().send_event(QiInjectionEvent {
            caster,
            target: None,
            skill: AnqiSkillId::MultiShot,
            carrier_kind: CarrierKind::LingmuArrow,
            outcome: injection_outcome(),
            tick: 11,
        });
        app.update();

        let emitted: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .collect();
        assert!(
            emitted.is_empty(),
            "QiInjectionEvent 的非 Snipe/Soul 分支不应在 audio 系统发声（走 MultiShot/ArmorPierce 专属 EventReader），实际 {} 条",
            emitted.len()
        );
    }

    // ========== 绝灵涡流 woliu v1（emit_woliu_v1_vortex_audio_triggers） ==========

    fn setup_woliu_v1_audio_app() -> App {
        let mut app = App::new();
        app.add_event::<VortexBackfireEvent>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_woliu_v1_vortex_audio_triggers);
        app
    }

    fn drain_audio(app: &mut App) -> Vec<PlaySoundRecipeRequest> {
        app.world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .collect()
    }

    /// field 出现 → woliu_cast 一次；存续 tick 不重复发声。
    #[test]
    fn woliu_v1_field_appear_plays_cast_recipe_once() {
        use crate::combat::woliu::VortexField;
        use valence::prelude::DVec3;
        let mut app = setup_woliu_v1_audio_app();
        let caster = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
        app.world_mut().entity_mut(caster).insert(VortexField {
            center: DVec3::new(0.0, 64.0, 0.0),
            radius: 8.0,
            delta: 4.0,
            cast_at_tick: 100,
            maintain_max_ticks: 1200,
            caster,
            env_qi_at_cast: 50.0,
            last_maintain_tick: 100,
        });

        app.update();
        let emitted = drain_audio(&mut app);
        assert_eq!(
            emitted.len(),
            1,
            "开涡应发 1 条音效，实际 {} 条",
            emitted.len()
        );
        assert_eq!(
            emitted[0].recipe_id, "woliu_cast",
            "开涡应复用 woliu_cast recipe（零新资产），实际 {}",
            emitted[0].recipe_id
        );

        app.update();
        assert!(
            drain_audio(&mut app).is_empty(),
            "field 存续期间不应重复发开涡音效"
        );
    }

    /// 反噬 → woliu_burst_pop 爆裂声。
    #[test]
    fn woliu_v1_backfire_plays_burst_pop() {
        use crate::combat::woliu::{BackfireCause, VortexBackfireEvent};
        let mut app = setup_woliu_v1_audio_app();
        let caster = app.world_mut().spawn(Position::new([2.0, 64.0, 2.0])).id();
        app.world_mut().send_event(VortexBackfireEvent {
            caster,
            cause: BackfireCause::EnvQiTooLow,
            meridian_severed: crate::cultivation::components::MeridianId::Lung,
            tick: 300,
            env_qi: 1.0,
            delta: 4.0,
            resisted: false,
        });
        app.update();
        let emitted = drain_audio(&mut app);
        assert_eq!(
            emitted.len(),
            1,
            "反噬应发 1 条爆裂声，实际 {} 条",
            emitted.len()
        );
        assert_eq!(
            emitted[0].recipe_id, "woliu_burst_pop",
            "反噬应发爆裂声 recipe，实际 {}",
            emitted[0].recipe_id
        );
    }

    /// 反噬 caster 断 Position 但领域仍在 → 回落 field.center 仍发声（重要负反馈不静默丢）。
    #[test]
    fn woliu_v1_backfire_falls_back_to_field_center_when_caster_positionless() {
        use crate::combat::woliu::{BackfireCause, VortexBackfireEvent, VortexField};
        use valence::prelude::DVec3;
        let mut app = setup_woliu_v1_audio_app();
        let caster = app.world_mut().spawn_empty().id();
        app.world_mut().entity_mut(caster).insert(VortexField {
            center: DVec3::new(4.0, 64.0, -2.0),
            radius: 8.0,
            delta: 4.0,
            cast_at_tick: 100,
            maintain_max_ticks: 1200,
            caster,
            env_qi_at_cast: 50.0,
            last_maintain_tick: 100,
        });
        app.update();
        drain_audio(&mut app); // 吃掉开涡音效

        app.world_mut().send_event(VortexBackfireEvent {
            caster,
            cause: BackfireCause::ExceedMaintainMax,
            meridian_severed: crate::cultivation::components::MeridianId::Lung,
            tick: 120,
            env_qi: 10.0,
            delta: 4.0,
            resisted: false,
        });
        app.update();
        let emitted = drain_audio(&mut app);
        assert_eq!(
            emitted.len(),
            1,
            "caster 无 Position 但领域仍在时，反噬音效应回落 field.center 发出，实际 {} 条",
            emitted.len()
        );
        assert_eq!(
            emitted[0].recipe_id, "woliu_burst_pop",
            "回落路径也必须是爆裂声 recipe，实际 {}",
            emitted[0].recipe_id
        );
    }
}
