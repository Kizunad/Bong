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
use crate::cultivation::death_hooks::PlayerRevived;
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
    recipient_for_attenuation, AudioRecipient, PlaySoundRecipeRequest, StopSoundRecipeRequest,
    AUDIO_BROADCAST_RADIUS,
};
use crate::npc::brain::canonical_npc_id;
use crate::npc::spawn::NpcMarker;
use crate::schema::tribulation::DuXuOutcomeV1;
use crate::skill::events::{SkillLvUp, SkillScrollUsed, SkillXpGain, XpGainSource};
use crate::social::events::{SocialPactEvent, SocialRenownDeltaEvent};
use crate::sword_path::av_event::{SwordPathSkillCastEvent, SwordPathSkillId};
use crate::tools::ToolKind;

/// **audio-trigger 调度的唯一生产注册入口**（`network::register` 调它，别处不许再散着 `add_systems`）。
///
/// 提取自 `network::mod`（PR #1262 review 意见）：接线门禁测试跑的就是这个函数，于是「某个 emit
/// 系统没被注册进调度」不再是测试照不到的死角——测试不再自己抄一份系统清单，从这里删掉任何一个
/// 系统，`production_wiring_registers_audio_trigger_systems_exactly_once_in_order` 立刻撞红
/// （它跑的是更上一层的生产 `network::register_app_wiring`，还顺带锁了「恰好注册一次」与两条调度边）。
///
/// 调度契约（与提取前逐条一致）：所有 emit 系统 `.after(tick_audio_dedup_clock)`（拿到当帧 dedup
/// 逻辑 tick）`.before(audio_event_emit::emit_audio_play_payloads)`（本系统发出的
/// `PlaySoundRecipeRequest` 同帧投递给客户端）。注意这只约束「emit 系统 → payload 投递」这一跳：
/// cast 逻辑 → emit 系统之间没有显式 order，cast 事件最坏跨 1 tick 才被读到（`EventReader`
/// 双缓冲保证不丢），与重构前 cast 命令 flush 的时序同量级，非本次引入。
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
    // 重生必须收掉低血心跳 loop（`heartbeat_low_hp` 第二层 = entity.player.hurt，client 侧每
    // 20 tick 自行重放，漏 stop 就变成重生后一直响受伤音）。约束与 #1264 逐字一致：排在低血上沿
    // 系统之后（先让血量记账落定，再由重生收尾清账 + 发 stop，否则两系统争 `AudioTriggerState`
    // 是 Bevy ambiguous order），并在 stop payload 投递之前，保证同帧下发。
    // 注意它挂的是 **stop** sink，与上面那组 emit 系统的 `.before(emit_audio_play_payloads)` 不同，
    // 故单独一个 add_systems 而不是并进那个 tuple。
    // add_event 幂等：这里自带一次注册，别让本系统的可运行性依赖 `cmd::dev::revive` 恰好也注册了
    // 同一事件（dev 入口不该是生产路径的前提）。
    app.add_event::<crate::cultivation::death_hooks::PlayerRevived>();
    app.add_systems(
        Update,
        stop_low_hp_heartbeat_on_revive
            .after(emit_player_state_audio_triggers)
            .before(crate::network::audio_event_emit::emit_audio_stop_payloads),
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

/// 低血心跳 loop 的稳定 instance id（同 fauna fuya pressure hum 惯例：`Entity::to_bits`）。
///
/// `heartbeat_low_hp` 是 **loop recipe**（`interval_ticks: 20`，第二层是
/// `minecraft:entity.player.hurt`），client 侧收到后由 `SoundRecipePlayer` 自己每秒重放；
/// 想收掉它必须发 `bong:audio/stop` 带**同一个 instance id**。所以这条 loop 不能再用
/// `instance_id: 0`（让 server 侧 allocator 随机分配、事后无从指认），必须按玩家实体派生
/// 一个稳定 id。
pub(crate) fn low_hp_heartbeat_instance_id(entity: Entity) -> u64 {
    entity.to_bits().max(1)
}

/// 低血心跳的唯一触发判据（严格小于阈值）。抽成函数是为了让「重生血量会不会重新
/// 起心跳」这类不变量能对着**生产判据**断言，而不是在测试里另写一遍比较。
pub(crate) fn is_low_hp_for_heartbeat(hp_ratio: f32) -> bool {
    hp_ratio < LOW_HP_HEARTBEAT_RATIO
}

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
    mut audio_stops: EventWriter<StopSoundRecipeRequest>,
) {
    let mut audio = audio.context();
    for (entity, position, wounds, cultivation) in &players {
        if let Some(wounds) = wounds {
            let hp_ratio = wounds.health_current / wounds.health_max.max(1.0);
            let low_hp = is_low_hp_for_heartbeat(hp_ratio);
            let was_low_hp = state.low_hp.get(&entity).copied().unwrap_or(false);
            if low_hp && !was_low_hp {
                emit_play_loop(
                    &mut audio,
                    "heartbeat_low_hp",
                    entity,
                    position.get(),
                    low_hp_heartbeat_instance_id(entity),
                    Some(LOW_HP_HEARTBEAT_FLAG.to_string()),
                    1.0,
                );
            } else if !low_hp && was_low_hp {
                // 血量回到阈值以上必须显式收 loop：开 loop 的一方负责关（同
                // fauna fuya pressure hum 的 play/stop 配对惯例）。漏关的话 client
                // 侧心跳会一直每秒重放 `entity.player.hurt` 层——实机表现就是
                // 重生（血量回到 REVIVE_HEALTH_FRACTION）之后仍在响受伤音。
                audio_stops.send(stop_low_hp_heartbeat(entity));
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

/// 重生（`PlayerRevived`）时无条件收掉该玩家的低血心跳 loop。
///
/// 血量回到 `REVIVE_HEALTH_FRACTION` 后 `emit_player_state_audio_triggers` 的下沿也会
/// 发一次 stop，但那条路径依赖 `REVIVE_HEALTH_FRACTION >= LOW_HP_HEARTBEAT_RATIO` 这个
/// 常数巧合（见 `revive_health_fraction_never_rearms_low_hp_heartbeat` pin 测试）。重生
/// 必须**干净**：这里按 `PlayerRevived` 显式收一次，并清掉 low_hp 记账，让下一次真掉血
/// 能重新起心跳。stop 是幂等的——client 侧没有该 instance 时 `loops.remove` / `sink.stop`
/// 都是 no-op。
pub fn stop_low_hp_heartbeat_on_revive(
    mut state: ResMut<AudioTriggerState>,
    mut revived: EventReader<PlayerRevived>,
    mut audio_stops: EventWriter<StopSoundRecipeRequest>,
) {
    for event in revived.read() {
        state.low_hp.remove(&event.entity);
        audio_stops.send(stop_low_hp_heartbeat(event.entity));
    }
}

fn stop_low_hp_heartbeat(entity: Entity) -> StopSoundRecipeRequest {
    StopSoundRecipeRequest {
        instance_id: low_hp_heartbeat_instance_id(entity),
        // 心跳是 player_local 的贴耳音，硬停（无淡出）才不会在重生后拖出尾音。
        fade_out_ticks: 0,
        recipient: AudioRecipient::Single(entity),
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
        // plan-gathering-tool-bind-v1 P1：草镰接通本职——持镰收割 vs 徒手割手的差异化 SFX。
        // PR #1293 review 修正：required_tool_used/bare_hand_wound 是"任意 required_tool
        // 草本"的通用布尔值，仓库里已有 DunQiJia/GuaDao/BingJiaShouTao 三类既有 required_tool
        // 草本——必须额外限定 required_tool_kind == CaoLian，否则这些既有草本的持工具/徒手
        // 采集也会错误播放草镰专属 SFX。
        let is_cao_lian = event.required_tool_kind == Some(ToolKind::CaoLian);
        if event.bare_hand_wound && is_cao_lian {
            emit_play(
                &mut audio,
                "botany_bare_hand_wound",
                event.client_entity,
                origin,
                None,
                1.0,
                0.0,
            );
        } else if event.required_tool_used && is_cao_lian {
            emit_play(
                &mut audio,
                "cao_lian_harvest_swing",
                event.client_entity,
                origin,
                None,
                1.0,
                0.0,
            );
        }
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
/// `cast_center`——那是**施法当时**取到的 caster 位置。这里刻意不查实时 `Position`：重构前的内联
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
            event.cast_center,
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
    emit_play_inner(
        audio,
        recipe_id,
        entity,
        origin,
        0,
        flag,
        volume_mul,
        pitch_shift,
    );
}

/// 带稳定 instance id 的 loop recipe 发声——调用方必须在条件结束时用同一 id 发
/// `StopSoundRecipeRequest` 收尾（一次性音效走 `emit_play`，instance 由 server 分配即可）。
fn emit_play_loop(
    audio: &mut AudioEmitContext<'_, '_>,
    recipe_id: impl Into<String>,
    entity: Entity,
    origin: DVec3,
    instance_id: u64,
    flag: Option<String>,
    volume_mul: f32,
) {
    emit_play_inner(
        audio,
        recipe_id,
        entity,
        origin,
        instance_id,
        flag,
        volume_mul,
        0.0,
    );
}

#[allow(clippy::too_many_arguments)]
fn emit_play_inner(
    audio: &mut AudioEmitContext<'_, '_>,
    recipe_id: impl Into<String>,
    entity: Entity,
    origin: DVec3,
    instance_id: u64,
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
        instance_id,
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
/// 若改用 `emit_play`，`dugu_poison_signature` 声明的 `MELEE` 会把**收包半径**从 64 格砍到 8 格
/// （比该招自己 10 格的 `ReverseAftermathCloud` 还小——站在毒雾里都可能收不到包），再叠上世界锚点
/// 的 LINEAR 衰减（L0 volume 0.24，8 格处已衰掉约一半）。近场增益两条路线量级相当，**塌的是
/// 收听范围**——正是 P4 吃过两次的「签名进了资产却听不到」那一类（PR #1262 review 抓出）。
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
    use valence::testing::{create_mock_client, MockClientHelper};

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
        app.add_event::<StopSoundRecipeRequest>();
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

    // ────────────────────────────────────────────────────────────────────
    // 低血心跳 loop 生命周期（重生残留受伤音修复）
    //
    // 实机 bug：死亡→点重生后仍每秒响一次受伤音。抓包实证根因——`heartbeat_low_hp`
    // 是 loop recipe（interval 20 ticks，第二层 `minecraft:entity.player.hurt`），
    // server 只在血量跌破 20% 的上沿发一次 play、**从不发 stop**；client 侧带 flag
    // 的 loop 又把 flag 自注册成 sticky，while_flag 判定永真 → loop 永生。
    // 下面这组用例把「谁开谁关」锁死在 server 侧。
    // ────────────────────────────────────────────────────────────────────

    /// 构造一个跑心跳生命周期的最小 App：两条 audio 事件通道 + 上沿/下沿系统 + 重生系统。
    /// 返回的 `MockClientHelper` 必须由调用方持住（drop 会断开 mock client 连接）。
    fn heartbeat_app(username: &str) -> (App, Entity, MockClientHelper) {
        let mut app = App::new();
        app.init_resource::<AudioTriggerState>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_event::<StopSoundRecipeRequest>();
        app.add_event::<PlayerRevived>();
        app.add_systems(
            Update,
            (
                emit_player_state_audio_triggers,
                stop_low_hp_heartbeat_on_revive,
            ),
        );
        let (mut bundle, helper) = create_mock_client(username);
        bundle.player.position = Position::new([0.0, 64.0, 0.0]);
        let player = app.world_mut().spawn(bundle).id();
        (app, player, helper)
    }

    fn set_health(app: &mut App, player: Entity, health_current: f32) {
        app.world_mut().entity_mut(player).insert(Wounds {
            health_current,
            health_max: 100.0,
            ..Default::default()
        });
    }

    fn drain_plays(app: &mut App) -> Vec<PlaySoundRecipeRequest> {
        app.world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .collect()
    }

    fn drain_stops(app: &mut App) -> Vec<StopSoundRecipeRequest> {
        app.world_mut()
            .resource_mut::<Events<StopSoundRecipeRequest>>()
            .drain()
            .collect()
    }

    /// loop 必须带稳定 instance id（否则事后无从 stop——这是原 bug 的结构性成因）。
    #[test]
    fn low_hp_heartbeat_play_carries_stable_stoppable_instance_id() {
        let (mut app, player, _client) = heartbeat_app("hbid");
        set_health(&mut app, player, 10.0);

        app.update();

        let plays = drain_plays(&mut app);
        assert_eq!(plays.len(), 1, "跌破 20% 应只发一次心跳 loop play");
        assert_eq!(plays[0].recipe_id, "heartbeat_low_hp");
        assert_ne!(
            plays[0].instance_id, 0,
            "期望心跳 loop 带非 0 的稳定 instance id 因为 instance_id=0 会让 server 侧 allocator 现分配、\
             之后无法用同一 id 发 stop（loop 永生 → 重生后仍响受伤音）；实际 0"
        );
        assert_eq!(
            plays[0].instance_id,
            low_hp_heartbeat_instance_id(player),
            "期望 instance id 由玩家实体派生（与 stop 侧同一函数）因为收 loop 要按同一 id 对齐；实际不一致"
        );
    }

    /// 契约 pin：心跳 recipe 必须是 `player_local`（→ 收件人 `Single(player)`），
    /// 因为 stop 侧写死了 `AudioRecipient::Single`。若有人把 attenuation 改成广播类，
    /// play 会到别人客户端、stop 却只回自己 → 别人耳朵里的心跳永生。
    #[test]
    fn low_hp_heartbeat_recipe_is_player_local_so_single_recipient_stop_matches() {
        let registry =
            crate::audio::SoundRecipeRegistry::load_default().expect("默认 audio recipe 应能加载");
        let recipe = registry
            .get("heartbeat_low_hp")
            .expect("heartbeat_low_hp recipe 应存在");
        assert!(
            recipe.loop_cfg.is_some(),
            "期望 heartbeat_low_hp 仍是 loop recipe 因为整套 play/stop 配对治法就是为 loop 设计的；实际没有 loop 段"
        );
        assert_eq!(
            recipe.attenuation,
            crate::schema::audio::AudioAttenuation::PlayerLocal,
            "期望 heartbeat_low_hp 保持 player_local 因为 stop 侧按 Single(player) 定向；\
             改成广播类会让别人客户端收到 play 却收不到 stop（心跳在他们耳里永生）；实际 {:?}",
            recipe.attenuation,
        );
    }

    /// 下沿（血量回到阈值以上）必须发 stop，且是发给该玩家自己。
    #[test]
    fn low_hp_heartbeat_stops_when_health_recovers_above_threshold() {
        let (mut app, player, _client) = heartbeat_app("hbrec");
        set_health(&mut app, player, 10.0);
        app.update();
        assert_eq!(drain_plays(&mut app).len(), 1, "先起心跳");
        assert!(
            drain_stops(&mut app).is_empty(),
            "上沿不该发 stop——loop 刚起来"
        );

        set_health(&mut app, player, 25.0);
        app.update();

        let stops = drain_stops(&mut app);
        assert_eq!(
            stops.len(),
            1,
            "期望血量回到 25%（> 20% 阈值）时发 1 条 stop 因为开 loop 的一方负责关掉它；实际 {} 条",
            stops.len()
        );
        assert_eq!(stops[0].instance_id, low_hp_heartbeat_instance_id(player));
        assert_eq!(
            stops[0].fade_out_ticks, 0,
            "期望硬停（fade_out_ticks=0）因为贴耳心跳淡出会在重生后拖出受伤尾音；实际有淡出"
        );
        assert_eq!(
            stops[0].recipient,
            AudioRecipient::Single(player),
            "期望只发给该玩家因为心跳是 player_local 私有反馈；实际收件人不对"
        );
        assert!(drain_plays(&mut app).is_empty(), "血量回升不该再发 play");
    }

    /// 持续低血只有一条 play、零 stop（心跳不能每 tick 重开，也不能自己断掉）。
    #[test]
    fn low_hp_heartbeat_is_not_restarted_or_stopped_while_health_stays_low() {
        let (mut app, player, _client) = heartbeat_app("hbhold");
        set_health(&mut app, player, 10.0);
        app.update();
        assert_eq!(drain_plays(&mut app).len(), 1);
        drain_stops(&mut app);

        set_health(&mut app, player, 5.0);
        app.update();
        app.update();

        assert!(
            drain_plays(&mut app).is_empty(),
            "期望持续低血不再重发 play 因为 loop 由 client 侧自行重放（上沿触发语义）；实际重发了"
        );
        assert!(
            drain_stops(&mut app).is_empty(),
            "期望持续低血不发 stop 因为条件还成立；实际误停了心跳"
        );
    }

    /// 主线场景：低血 → 死亡 → 重生。重生必须收掉心跳 loop（否则重生后一直响受伤音）。
    #[test]
    fn revive_stops_low_hp_heartbeat_loop() {
        let (mut app, player, _client) = heartbeat_app("hbrev");
        set_health(&mut app, player, 10.0);
        app.update();
        assert_eq!(drain_plays(&mut app).len(), 1, "低血先起心跳");
        drain_stops(&mut app);

        // 死亡（hp=0）期间心跳照旧（条件仍成立），关键是重生这一刻要收掉。
        set_health(&mut app, player, 0.0);
        app.update();
        assert!(drain_stops(&mut app).is_empty(), "死亡本身不触发 stop");

        app.world_mut().send_event(PlayerRevived { entity: player });
        app.update();

        let stops = drain_stops(&mut app);
        assert!(
            stops
                .iter()
                .any(|stop| stop.instance_id == low_hp_heartbeat_instance_id(player)),
            "期望 PlayerRevived 后发出针对该玩家心跳 instance 的 stop 因为重生必须干净、\
             不能残留 heartbeat_low_hp 的 entity.player.hurt 层；实际 stop 列表 {:?}",
            stops
                .iter()
                .map(|stop| stop.instance_id)
                .collect::<Vec<_>>()
        );
    }

    /// 重生后记账要清干净：下一次真掉血能重新起心跳（修复不能把低血反馈永久关死）。
    #[test]
    fn low_hp_heartbeat_rearms_after_revive_when_player_drops_low_again() {
        let (mut app, player, _client) = heartbeat_app("hbrearm");
        set_health(&mut app, player, 10.0);
        app.update();
        drain_plays(&mut app);

        app.world_mut().send_event(PlayerRevived { entity: player });
        // 重生把血量拉回 REVIVE_HEALTH_FRACTION（20%）——正好在阈值上，不该再起心跳。
        set_health(&mut app, player, 20.0);
        app.update();
        drain_stops(&mut app);
        assert!(
            drain_plays(&mut app).is_empty(),
            "期望重生血量（20% = 阈值）不重开心跳因为判据是严格小于阈值；实际重开了"
        );

        // 之后再被打到 15% —— 心跳必须回来（低血反馈没被永久关死）。
        set_health(&mut app, player, 15.0);
        app.update();

        let plays = drain_plays(&mut app);
        assert_eq!(
            plays.len(),
            1,
            "期望重生后再次跌破 20% 能重新起心跳因为重生只清记账、不禁用低血反馈；实际 {} 条 play",
            plays.len()
        );
        assert_eq!(plays[0].recipe_id, "heartbeat_low_hp");
    }

    /// 比常数 pin 更严的同族 pin：**照生产的 f32 算术**把重生血量算出来再过判据。
    ///
    /// 生产链路是 `health_current = (health_max * REVIVE_HEALTH_FRACTION).max(1.0)`
    /// （`combat::lifecycle::revive_lifecycle`）→ `hp_ratio = health_current / health_max.max(1.0)`
    /// （本文件的心跳系统）。f32 舍入让「比例常数 >= 阈值」并不能推出「算出来的商 >= 阈值」：
    /// health_max = 20.5 / 41.0 / 82.0 等取值下商会落到 0.19999999 < 0.2（实算复核过 41.0：
    /// `41 × 0.2f32` 舍入成 8.19999980926，除 41 得 0.199999995），重生那一刻又自动起一条含
    /// `entity.player.hurt` 层的心跳。今天玩家 health_max 恒为 `Wounds::default()` 的 100.0
    /// （100 × 0.2 = 20.0，商恰好 0.2）所以安全。
    ///
    /// **覆盖边界（别过度指望这条）**：它只盯 `Wounds::default()` 这一个来源，所以能挡住
    /// 改 `DEFAULT_HEALTH_MAX` / `REVIVE_HEALTH_FRACTION`。若将来按境界/属性走**运行时赋值**
    /// 改玩家 `health_max`（不动 Default），这条 pin 不会撞红——那种改法必须自己重算这个商。
    #[test]
    fn revive_health_ratio_computed_like_production_never_rearms_heartbeat() {
        let health_max = Wounds::default().health_max;
        let revived_health =
            (health_max * crate::combat::components::REVIVE_HEALTH_FRACTION).max(1.0);
        let revived_ratio = revived_health / health_max.max(1.0);
        assert!(
            !is_low_hp_for_heartbeat(revived_ratio),
            "期望按生产算术算出的重生血量比例 {revived_ratio}（health_max={health_max} → \
             health_current={revived_health}）不触发低血心跳（阈值 {LOW_HP_HEARTBEAT_RATIO}）——\
             f32 舍入一旦让商落到阈值之下，重生瞬间就会自动起一条含 entity.player.hurt 层的\
             心跳 loop；改动 health_max / REVIVE_HEALTH_FRACTION 时必须同时重设计心跳触发",
        );
    }

    #[test]
    fn revive_health_fraction_never_rearms_low_hp_heartbeat() {
        // 对着**生产判据** is_low_hp_for_heartbeat 断言，而不是在测试里重写比较。
        let revive_hp_ratio = crate::combat::components::REVIVE_HEALTH_FRACTION;
        assert!(
            !is_low_hp_for_heartbeat(revive_hp_ratio),
            "期望重生血量比例 {revive_hp_ratio} 不触发低血心跳（阈值 {LOW_HP_HEARTBEAT_RATIO}）——\
             否则重生那一刻就会自动起一条含 entity.player.hurt 层的心跳 loop，玩家听到的就是\
             「重生就有受伤音」；要调低重生血量必须同时重设计心跳触发",
        );
        // 反向对照：略低于重生血量的 hp 必须仍被判为低血（防止有人把判据改成恒 false 让本测试蒙过）。
        assert!(
            is_low_hp_for_heartbeat(revive_hp_ratio - 0.01),
            "期望比重生血量再低一点（{}）仍被判低血，否则判据被改坏成恒 false、低血心跳整体失效",
            revive_hp_ratio - 0.01,
        );
    }

    /// 反向锁：修复只动 loop 生命周期，**真受伤的一次性音效照旧**（含重生之后再被打）。
    #[test]
    fn combat_hit_audio_still_plays_after_revive() {
        let mut app = App::new();
        app.init_resource::<AudioTriggerState>();
        app.init_resource::<AudioImplementationDedup>();
        app.add_event::<CombatEvent>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_event::<StopSoundRecipeRequest>();
        app.add_event::<PlayerRevived>();
        app.add_systems(
            Update,
            (stop_low_hp_heartbeat_on_revive, emit_combat_audio_triggers),
        );
        let attacker = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
        let target = app.world_mut().spawn(Position::new([1.0, 64.0, 0.0])).id();

        app.world_mut().send_event(PlayerRevived { entity: target });
        app.world_mut().send_event(CombatEvent {
            attacker,
            target,
            resolved_at_tick: 7,
            body_part: BodyPart::Chest,
            wound_kind: WoundKind::Blunt,
            source: crate::combat::events::AttackSource::Melee,
            debug_command: false,
            physical_damage: 0.0,
            damage: 12.0,
            contam_delta: 0.0,
            description: "post-revive hit".to_string(),
            defense_kind: None,
            defense_effectiveness: None,
            defense_contam_reduced: None,
            defense_wound_severity: None,
        });

        app.update();

        let recipes: Vec<_> = drain_plays(&mut app)
            .into_iter()
            .map(|request| request.recipe_id)
            .collect();
        assert_eq!(
            recipes,
            vec!["hit_heavy", "wound_inflict"],
            "期望重生后被打（胸部 12 伤害）仍发 hit_heavy + wound_inflict（后者含 entity.player.hurt 层）\
             因为修复只收心跳 loop、不该动真受伤反馈；实际 recipes={recipes:?}"
        );
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

    /// 收集某个 App 的 `Update` 调度里所有系统的 (NodeId, 系统名)。`ScheduleGraph` 在
    /// `add_systems` 时即填充，无需初始化/运行，故不受「缺 Events 资源会 panic」影响。
    fn update_schedule_systems(app: &App) -> Vec<(bevy_ecs::schedule::NodeId, String)> {
        app.get_schedule(Update)
            .expect("Update 调度应存在")
            .graph()
            .systems()
            .map(|(id, system, _)| (id, system.name().to_string()))
            .collect()
    }

    /// 定位某个系统函数对应的 `SystemTypeSet` 节点——`.after(f)` / `.before(f)` 建的依赖边
    /// 连的是该函数的匿名 type set，不是系统节点本身，故顺序断言要拿它对拍。
    fn locate_system_type_set(app: &App, expected: &str) -> bevy_ecs::schedule::NodeId {
        let hits: Vec<_> = app
            .get_schedule(Update)
            .expect("Update 调度应存在")
            .graph()
            .system_sets()
            .filter(|(_, set, _)| format!("{set:?}").contains(expected))
            .map(|(id, _, _)| id)
            .collect();
        assert_eq!(
            hits.len(),
            1,
            "`{expected}` 的 SystemTypeSet 应唯一，实际 {} 个",
            hits.len()
        );
        hits[0]
    }

    /// 在调度里按后缀唯一定位一个系统，返回它的 `NodeId`——**恰好一次**，重复注册也撞红。
    fn locate_exactly_once(
        systems: &[(bevy_ecs::schedule::NodeId, String)],
        expected: &str,
    ) -> bevy_ecs::schedule::NodeId {
        let hits: Vec<_> = systems
            .iter()
            .filter(|(_, name)| name.ends_with(expected))
            .collect();
        assert_eq!(
            hits.len(),
            1,
            "`{expected}` 应在 Update 调度里恰好注册 1 次（0 次 = 运行时永不触发的功能孤岛；\
             ≥2 次 = 重复注册：dedup 逻辑时钟会一帧推进多次、把 2 tick 窗口悄悄改短，emit 也会重发），\
             实际 {} 次",
            hits.len()
        );
        hits[0].0
    }

    /// **接线门禁（调度侧）**：跑**生产**装配入口，断言三条签名链的 emit 系统
    /// ① 恰好注册一次、② `.after(tick_audio_dedup_clock)`、③ `.before(emit_audio_play_payloads)`
    /// 三条依赖边都在图里。
    ///
    /// 走的是生产 `network::register_app_wiring`（`network::register` = Redis bootstrap + 它），
    /// 所以**连顶层那一行 `audio_trigger::register(app)` 委托被删也会撞红**——不是只调
    /// `audio_trigger::register` 自欺欺人（PR #1262 review 要求，已变异验证）。
    #[test]
    fn production_wiring_registers_audio_trigger_systems_exactly_once_in_order() {
        let mut app = App::new();
        crate::network::register_app_wiring(&mut app);

        let systems = update_schedule_systems(&app);
        // 系统本体各注册一次（重复注册撞红）……
        locate_exactly_once(&systems, "tick_audio_dedup_clock");
        locate_exactly_once(&systems, "audio_event_emit::emit_audio_play_payloads");
        // ……顺序边连的是这两个函数的 SystemTypeSet 节点。
        let dedup_clock = locate_system_type_set(&app, "tick_audio_dedup_clock");
        let payload_sink = locate_system_type_set(&app, "emit_audio_play_payloads");
        let dependency = app
            .get_schedule(Update)
            .expect("Update 调度应存在")
            .graph()
            .dependency()
            .graph();

        for expected in [
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
            let node = locate_exactly_once(&systems, expected);
            assert!(
                dependency.contains_edge(dedup_clock, node),
                "`{expected}` 必须 .after(tick_audio_dedup_clock)——少了这条边，emit 会读到上一帧的 \
                 dedup 逻辑 tick，2 tick 去重窗口失准"
            );
            assert!(
                dependency.contains_edge(node, payload_sink),
                "`{expected}` 必须 .before(emit_audio_play_payloads)——少了这条边，本帧发出的 \
                 PlaySoundRecipeRequest 要等下一帧才投递给客户端（本文件 register 的调度契约）"
            );
        }

        // 重生收心跳 loop（#1264）挂的是 **stop** sink，约束与上面那组不同，单独锁一遍：
        // 漏 `.after(emit_player_state_audio_triggers)` 会与低血上沿系统争 `AudioTriggerState`
        // （Bevy ambiguous order）；漏 `.before(emit_audio_stop_payloads)` 则 stop 跨帧才下发。
        let revive_stop = locate_exactly_once(&systems, "stop_low_hp_heartbeat_on_revive");
        let low_hp_state = locate_system_type_set(&app, "emit_player_state_audio_triggers");
        let stop_sink = locate_system_type_set(&app, "emit_audio_stop_payloads");
        assert!(
            dependency.contains_edge(low_hp_state, revive_stop),
            "`stop_low_hp_heartbeat_on_revive` 必须 .after(emit_player_state_audio_triggers)"
        );
        assert!(
            dependency.contains_edge(revive_stop, stop_sink),
            "`stop_low_hp_heartbeat_on_revive` 必须 .before(emit_audio_stop_payloads)"
        );
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

    // ─── dedup 状态转换：P5 后三处站点首次套上 AudioImplementationDedup ───

    /// 蜕壳签名的 **dedup 碰撞 + 窗口恢复**（PR #1262 review：plan 明确接受这个新状态转换，
    /// 那就必须有测试锁住它，否则「接受」是空话）。
    ///
    /// `shed_skin_burst` 被主动施法与维护 / 被动掉壳共用，dedup key = (owner, recipe)、
    /// 窗口 `AUDIO_DEDUP_WINDOW_TICKS` = 2 逻辑 tick。三条状态转换逐个断言：
    /// ① 同 owner 同帧两条（主动 + 被动）→ 只发一条；② 窗口内（下一帧）再来 → 仍被抑制；
    /// ③ 跨过窗口边界 → 恢复发声。外加 ④ 不同 owner 互不抑制。
    #[test]
    fn shed_signature_dedup_collides_within_window_and_recovers_after() {
        use crate::audio::implementation::AUDIO_DEDUP_WINDOW_TICKS;
        use crate::combat::tuike_v2::events::{
            FalseSkinSheddedEvent, TuikeSkillId, TuikeSkillVisual,
        };
        use crate::combat::tuike_v2::FalseSkinTier;

        fn shed_event(owner: Entity, active: bool) -> FalseSkinSheddedEvent {
            FalseSkinSheddedEvent {
                owner,
                attacker: None,
                tier: FalseSkinTier::Light,
                damage_absorbed: 0.0,
                damage_overflow: 0.0,
                contam_load: 0.0,
                permanent_taint_load: 0.0,
                layers_after: 0,
                active,
                tick: 1,
                visual: TuikeSkillVisual::for_skill(TuikeSkillId::Shed, false).into(),
            }
        }

        let mut app = App::new();
        app.init_resource::<AudioImplementationDedup>();
        app.add_event::<crate::combat::tuike_v2::DonFalseSkinEvent>();
        app.add_event::<FalseSkinSheddedEvent>();
        app.add_event::<crate::combat::tuike_v2::ContamTransferredEvent>();
        app.add_event::<PlaySoundRecipeRequest>();
        // 与生产同序：dedup 逻辑时钟先推进，emit 才读它。
        app.add_systems(
            Update,
            (tick_audio_dedup_clock, emit_tuike_v2_audio_triggers).chain(),
        );
        let owner = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
        let other = app.world_mut().spawn(Position::new([40.0, 64.0, 0.0])).id();

        // ① 同 owner 同帧：主动施法 + 维护/被动掉壳各发一条事件 → 只响一次。
        //    ④ 同帧另一个 owner 的蜕壳不受牵连。
        app.world_mut().send_event(shed_event(owner, true));
        app.world_mut().send_event(shed_event(owner, false));
        app.world_mut().send_event(shed_event(other, true));
        app.update();
        let first: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .map(|event| (event.recipient, event.recipe_id))
            .collect();
        assert_eq!(
            first.len(),
            2,
            "同 owner 的两条蜕壳应被 dedup 合成一条、另一 owner 独立发一条（共 2 条），实际 {first:?}"
        );
        let recipients: std::collections::BTreeSet<_> = first
            .iter()
            .map(|(recipient, _)| format!("{recipient:?}"))
            .collect();
        assert_eq!(
            recipients.len(),
            2,
            "这 2 条应分属两个不同 owner（dedup key 含 entity，别人的蜕壳不该被我的抑制），实际 {first:?}"
        );

        // ② 窗口内（下一帧，逻辑 tick 差 1 < 2）再来一条 → 仍被抑制。
        app.world_mut().send_event(shed_event(owner, true));
        app.update();
        let within: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .collect();
        assert!(
            within.is_empty(),
            "dedup 窗口内（差 1 tick < {AUDIO_DEDUP_WINDOW_TICKS}）的同 owner 同 recipe 应被抑制，实际 {} 条",
            within.len()
        );

        // ③ 再推一帧跨过窗口边界（差 2 tick）→ 恢复发声。
        app.world_mut().send_event(shed_event(owner, true));
        app.update();
        let recovered: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .map(|event| event.recipe_id)
            .collect();
        assert_eq!(
            recovered,
            vec![crate::combat::tuike_v2::events::SHED_SKIN_BURST_RECIPE.to_string()],
            "跨过 {AUDIO_DEDUP_WINDOW_TICKS} tick 窗口后应恢复发声，实际 {recovered:?}"
        );
    }

    /// 真脉共用 recipe 的两招（multipoint / harden 都映射 `zhenmai_shield_hum`）同帧连发时，
    /// 同样被 dedup 合成一条；而映射到别的 recipe 的招（parry）不受影响。
    ///
    /// 这条也是 P5 新引入的状态转换（旧内联 emit 不过 dedup），plan 里已声明接受。
    #[test]
    fn zhenmai_shared_recipe_skills_dedup_within_window() {
        use crate::combat::zhenmai_v2::ZhenmaiSkillId;

        assert_eq!(
            ZhenmaiSkillId::MultiPoint.audio_recipe(),
            ZhenmaiSkillId::HardenMeridian.audio_recipe(),
            "本测试的前提是这两招共用 recipe（既有映射）；若哪天各自专属了，本测试应改为断言两条都响"
        );

        let mut app = App::new();
        app.init_resource::<AudioImplementationDedup>();
        app.add_event::<ZhenmaiSkillCastEvent>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(
            Update,
            (tick_audio_dedup_clock, emit_zhenmai_v2_audio_triggers).chain(),
        );
        let caster = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
        for skill in [
            ZhenmaiSkillId::MultiPoint,
            ZhenmaiSkillId::HardenMeridian,
            ZhenmaiSkillId::Parry,
        ] {
            app.world_mut().send_event(ZhenmaiSkillCastEvent {
                caster,
                skill,
                cast_center: DVec3::new(0.0, 64.0, 0.0),
            });
        }
        app.update();

        let mut recipes = drain_recipes(&mut app);
        recipes.sort();
        let mut expected = vec![
            ZhenmaiSkillId::MultiPoint.audio_recipe().to_string(),
            ZhenmaiSkillId::Parry.audio_recipe().to_string(),
        ];
        expected.sort();
        assert_eq!(
            recipes, expected,
            "共用 `zhenmai_shield_hum` 的 multipoint / harden 同帧连发应只响一次，parry 另发一条，\
             实际 {recipes:?}"
        );
    }

    // ─── 真脉五招：emit_zhenmai_v2_audio_triggers（P5 emit 架构统一）───

    fn setup_zhenmai_audio_app() -> App {
        let mut app = App::new();
        app.init_resource::<AudioImplementationDedup>();
        // 装**真实** registry：recipient 路由由 recipe 的 attenuation 推出，不插 registry 会退化成
        // `Single(caster)`，路由断言就成了空转。
        app.insert_resource(
            SoundRecipeRegistry::load_default().expect("default audio recipes should load"),
        );
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
                cast_center: DVec3::new(0.0, 64.0, 0.0),
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
            cast_center: DVec3::new(7.5, 64.0, -3.5),
        });
        app.world_mut().send_event(ZhenmaiSkillCastEvent {
            caster: positionless_caster,
            skill: ZhenmaiSkillId::Parry,
            cast_center: DVec3::new(-20.5, 70.0, 11.5),
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
            "caster 无 Position 时同样用 event.cast_center，不静默丢招式声"
        );
        // 路由锁：收听范围由 recipe 声明的 attenuation（真脉五招全是 world_3d）推出 = 64 格广播，
        // 圆心同为 cast-time center。重构前是内联硬编码的 32 格；这条已在 plan 披露为「只放宽收包」，
        // 此处钉住它——若哪天被改成听者锚点或 MELEE 8 格（dugu 踩过的坑）立刻撞红。
        assert_eq!(
            emitted[0].recipient,
            AudioRecipient::Radius {
                origin: DVec3::new(7.5, 64.0, -3.5),
                radius: AUDIO_BROADCAST_RADIUS,
            },
            "真脉音效收听范围应是以 cast-time center 为圆心的 world_3d 64 格广播"
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

    // ── plan-gathering-tool-bind-v1 P1（PR #1293 review 修正）：草镰专属 SFX 必须严格限定
    // required_tool_kind == CaoLian，不能对任意 required_tool 草本一律播放 ──

    fn botany_harvest_terminal_event(
        client_entity: Entity,
        bare_hand_wound: bool,
        required_tool_used: bool,
        required_tool_kind: Option<ToolKind>,
    ) -> HarvestTerminalEvent {
        HarvestTerminalEvent {
            client_entity,
            session_id: "offline:Azure".to_string(),
            target_id: "plant-1".to_string(),
            target_name: "test_plant".to_string(),
            plant_kind: "test_plant".to_string(),
            mode: crate::botany::components::BotanyHarvestMode::Manual,
            interrupted: false,
            completed: true,
            detail: "采得 1 株".to_string(),
            target_pos: Some([10.0, 64.0, 10.0]),
            spirit_quality: 0.9,
            duration_ticks: 40,
            gathering_quality: None,
            tool_used: None,
            overflow_to_ground: false,
            bare_hand_wound,
            required_tool_used,
            required_tool_kind,
        }
    }

    fn botany_audio_test_app() -> App {
        let mut app = App::new();
        app.add_event::<HarvestTerminalEvent>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_botany_audio_triggers);
        app
    }

    #[test]
    fn cao_lian_harvest_swing_emits_after_cao_lian_tool_used() {
        let mut app = botany_audio_test_app();
        let player = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
        app.world_mut().send_event(botany_harvest_terminal_event(
            player,
            false,
            true,
            Some(ToolKind::CaoLian),
        ));
        app.update();

        let recipes: Vec<_> = drain_audio(&mut app)
            .into_iter()
            .map(|e| e.recipe_id)
            .collect();
        assert_eq!(
            recipes,
            vec!["harvest_pluck", "cao_lian_harvest_swing"],
            "持草镰完成采集应在基础采集声之后追加草镰挥砍声"
        );
    }

    #[test]
    fn botany_bare_hand_wound_emits_after_cao_lian_bare_hand_hit() {
        let mut app = botany_audio_test_app();
        let player = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
        app.world_mut().send_event(botany_harvest_terminal_event(
            player,
            true,
            false,
            Some(ToolKind::CaoLian),
        ));
        app.update();

        let recipes: Vec<_> = drain_audio(&mut app)
            .into_iter()
            .map(|e| e.recipe_id)
            .collect();
        assert_eq!(
            recipes,
            vec!["harvest_pluck", "botany_bare_hand_wound"],
            "草镰目标草本徒手割手应在基础采集声之后追加割手痛呼声"
        );
    }

    /// 回归：既有 DunQiJia 门槛草本（`XuanGenWei` 等）持工具完成采集，不得播放
    /// 草镰专属的 `cao_lian_harvest_swing`——required_tool_used 只是"任意 required_tool
    /// 命中"的通用信号，必须靠 required_tool_kind 甄别。
    #[test]
    fn non_cao_lian_required_tool_harvest_does_not_emit_cao_lian_swing() {
        let mut app = botany_audio_test_app();
        let player = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
        app.world_mut().send_event(botany_harvest_terminal_event(
            player,
            false,
            true,
            Some(ToolKind::DunQiJia),
        ));
        app.update();

        let recipes: Vec<_> = drain_audio(&mut app)
            .into_iter()
            .map(|e| e.recipe_id)
            .collect();
        assert_eq!(
            recipes,
            vec!["harvest_pluck"],
            "既有 DunQiJia 门槛草本不属于本 plan 范围，不应播放草镰专属挥砍声"
        );
    }

    /// 回归：既有 DunQiJia 门槛草本徒手割手，不得播放草镰专属的 `botany_bare_hand_wound`。
    #[test]
    fn non_cao_lian_bare_hand_wound_does_not_emit_botany_bare_hand_wound_recipe() {
        let mut app = botany_audio_test_app();
        let player = app.world_mut().spawn(Position::new([0.0, 64.0, 0.0])).id();
        app.world_mut().send_event(botany_harvest_terminal_event(
            player,
            true,
            false,
            Some(ToolKind::DunQiJia),
        ));
        app.update();

        let recipes: Vec<_> = drain_audio(&mut app)
            .into_iter()
            .map(|e| e.recipe_id)
            .collect();
        assert_eq!(
            recipes,
            vec!["harvest_pluck"],
            "既有 DunQiJia 门槛草本的徒手割手不属于本 plan 范围，不应播放草镰专属割手声"
        );
    }
}
