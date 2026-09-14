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
use crate::combat::components::{Lifecycle, LifecycleState, Wounds};
use crate::combat::dugu_v2::skills::DUGU_POISON_SIGNATURE_RECIPE;
use crate::combat::dugu_v2::ReverseTriggeredEvent;
use crate::combat::events::{AttackSource, CombatEvent, DefenseKind};
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
use crate::npc::lifecycle::{NpcTerminalSettlementSucceeded, NpcTerminalSystemSet};
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
            emit_npc_death_audio_triggers
                .in_set(NpcTerminalSystemSet::PostCommit)
                .after(crate::combat::resolve::resolve_attack_intents),
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
    // 重生必须收掉低血心跳 loop（client 侧每 20 tick 自行重放，旧配方曾含受伤叫声）。排在低血上沿
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
/// `heartbeat_low_hp` 是 **loop recipe**（`interval_ticks: 20`），
/// client 侧收到后由 `SoundRecipePlayer` 自己每秒重放；
/// 想收掉它必须发 `bong:audio/stop` 带**同一个 instance id**。所以这条 loop 不能再用
/// `instance_id: 0`（让 server 侧 allocator 随机分配、事后无从指认），必须按玩家实体派生
/// 一个稳定 id。
pub(crate) fn low_hp_heartbeat_instance_id(entity: Entity) -> u64 {
    entity.to_bits().max(1)
}

/// 低血心跳的血量判据（大于零且严格小于阈值）。抽成函数是为了让「重生血量会不会重新
/// 起心跳」这类不变量能对着**生产判据**断言，而不是在测试里另写一遍比较。
pub(crate) fn is_low_hp_for_heartbeat(hp_ratio: f32) -> bool {
    hp_ratio > 0.0 && hp_ratio < LOW_HP_HEARTBEAT_RATIO
}

type PlayerAudioStateItem<'a> = (
    Entity,
    &'a Position,
    Option<&'a Wounds>,
    Option<&'a Cultivation>,
    Option<&'a Lifecycle>,
);
type PlayerAudioStateFilter = With<Client>;

pub fn emit_player_state_audio_triggers(
    mut state: ResMut<AudioTriggerState>,
    players: Query<PlayerAudioStateItem<'_>, PlayerAudioStateFilter>,
    mut audio: AudioEmitWriter,
    mut audio_stops: EventWriter<StopSoundRecipeRequest>,
) {
    let mut audio = audio.context();
    for (entity, position, wounds, cultivation, lifecycle) in &players {
        if let Some(wounds) = wounds {
            let hp_ratio = wounds.health_current / wounds.health_max.max(1.0);
            // 等待复活裁决和终结均不是存活低血，不能继续播放心跳。
            let alive = !lifecycle.is_some_and(|life| life.state != LifecycleState::Alive);
            let low_hp = alive && is_low_hp_for_heartbeat(hp_ratio);
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
                // 退出存活低血就显式收 loop，不能等复活后再停止死亡界面里的循环。
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
    mut settlements: EventReader<NpcTerminalSettlementSucceeded>,
    positions: Query<&Position>,
    mut audio: AudioEmitWriter,
) {
    let mut audio = audio.context();
    for settlement in settlements.read() {
        let Ok(position) = positions.get(settlement.entity) else {
            continue;
        };
        emit_play(
            &mut audio,
            "npc_death",
            settlement.entity,
            position.get(),
            None,
            1.0,
            0.0,
        );
        if let Some(attacker) = settlement.attacker {
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
#[path = "audio_trigger_tests.rs"]
mod tests;
