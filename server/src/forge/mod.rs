//! plan-forge-v1 —— 炼器专项（武器 MVP）。
//!
//! 独立于 cultivation::forging（经脉锻造）。本模块实装 §3 MVP 切片：
//!   * §1.1 BlueprintRegistry（JSON 加载）→ blueprint.rs
//!   * §1.2 WeaponForgeStation Component            → station.rs
//!   * §1.3 ForgeSession 四步进程                   → session.rs + steps.rs
//!   * §1.4 LearnedBlueprints                       → learned.rs
//!   * §4   事件总线                                → events.rs
//!   * §6.P6 flawed_fallback / side_effect_pool     → fallback.rs + history.rs
//!
//! 服务器系统负责把 Event 翻译为 StepState 变化，由 client UI / 未来 agent 驱动 Event 输入。
//!
//! TODO(plan-persistence-v1): forge 持久化需保存的 Resource/Component：
//! `ForgeSessions`（在炉进度）、`BlueprintRegistry`（图谱定义版本/校验）、
//! `LearnedBlueprints`（玩家已学图谱）与 `WeaponForgeStation`（砧方块实体）。

pub mod blueprint;
pub mod events;
pub mod fallback;
pub mod history;
pub mod inventory_bridge;
pub mod learned;
pub mod session;
pub mod skill_hook;
pub mod station;
pub mod steps;

use std::collections::HashMap;

use valence::prelude::{
    App, EventReader, EventWriter, IntoSystemConfigs, Query, Res, ResMut, Update,
};

use self::blueprint::{BlueprintRegistry, StepKind, DEFAULT_BLUEPRINTS_DIR};
use self::events::{
    ConsecrationInject, ForgeBucket, ForgeOutcomeEvent, ForgeStartAccepted,
    InscriptionScrollSubmit, StartForgeRequest, StepAdvance, TemperingHit,
};
use self::history::{ForgeAttempt, ForgeHistory};
use self::learned::LearnedBlueprints;
use self::session::{ForgeSession, ForgeSessions, ForgeStep, StepState};
use self::station::WeaponForgeStation;
use self::steps::{
    advance_step, apply_scroll, apply_tempering_hit, compute_achieved_tier, inject_qi,
    resolve_billet, resolve_consecration, resolve_inscription, resolve_tempering, select_bucket,
    ConsecrationResult, InscriptionResult, TemperingResult,
};
use crate::cultivation::breakthrough::skill_cap_for_realm;
use crate::cultivation::components::{Cultivation, QiColor};
use crate::mineral::{build_default_registry as build_default_mineral_registry, MineralRegistry};
use crate::skill::components::{SkillId, SkillSet};
use crate::skill::curve::effective_lv;
use crate::skill::events::{SkillXpGain, XpGainSource};

type ForgeCasterSkillQueryItem<'a> = (&'a Cultivation, &'a QiColor, &'a SkillSet);

pub fn register(app: &mut App) {
    tracing::info!("[bong][forge] registering plan-forge-v1 systems");

    let mineral_registry = build_default_mineral_registry();
    let registry =
        BlueprintRegistry::load_dir_with_minerals(DEFAULT_BLUEPRINTS_DIR, Some(&mineral_registry))
            .unwrap_or_else(|e| {
                tracing::error!("[bong][forge] blueprint load failed: {e}");
                BlueprintRegistry::new()
            });
    tracing::info!(
        "[bong][forge] loaded {} blueprints: [{}]",
        registry.len(),
        registry.ids().cloned().collect::<Vec<_>>().join(", ")
    );
    app.insert_resource(registry);
    app.insert_resource(ForgeSessions::new());

    app.add_event::<StartForgeRequest>();
    app.add_event::<TemperingHit>();
    app.add_event::<InscriptionScrollSubmit>();
    app.add_event::<ConsecrationInject>();
    app.add_event::<StepAdvance>();
    app.add_event::<ForgeStartAccepted>();
    app.add_event::<ForgeOutcomeEvent>();
    app.add_event::<station::PlaceForgeStationRequest>();

    app.add_systems(
        Update,
        (
            station::handle_place_station_request,
            handle_start_forge_requests,
            crate::network::forge_bridge::publish_forge_start_on_session_create
                .after(handle_start_forge_requests),
            handle_tempering_hits.after(handle_start_forge_requests),
            handle_scroll_submits.after(handle_tempering_hits),
            handle_consecration_injects.after(handle_scroll_submits),
            handle_step_advance.after(handle_consecration_injects),
            inventory_bridge::forge_outcome_to_inventory.after(handle_step_advance),
            crate::network::forge_bridge::publish_forge_outcome.after(handle_step_advance),
        ),
    );
}

// ══════════════════════════════ Systems ══════════════════════════════

#[allow(clippy::too_many_arguments)]
fn handle_start_forge_requests(
    mut ev: EventReader<StartForgeRequest>,
    registry: Res<BlueprintRegistry>,
    minerals: Res<MineralRegistry>,
    mut sessions: ResMut<ForgeSessions>,
    mut stations: Query<&mut WeaponForgeStation>,
    learned: Query<&LearnedBlueprints>,
    mut accepted: EventWriter<ForgeStartAccepted>,
    mut outcomes: EventWriter<ForgeOutcomeEvent>,
) {
    for req in ev.read() {
        let Some(bp) = registry.get(&req.blueprint) else {
            tracing::warn!("[bong][forge] unknown blueprint: {}", req.blueprint);
            continue;
        };
        // 校验图谱已学习
        if let Ok(lb) = learned.get(req.caster) {
            if !lb.knows(&bp.id) {
                tracing::debug!("[bong][forge] caster has not learned {}", bp.id);
                continue;
            }
        }
        // 校验砧 tier
        let Ok(mut station) = stations.get_mut(req.station) else {
            tracing::warn!("[bong][forge] station entity missing");
            continue;
        };
        if !station.can_craft(bp.station_tier_min) {
            tracing::debug!(
                "[bong][forge] station tier {} < required {}",
                station.tier,
                bp.station_tier_min
            );
            continue;
        }

        // 解析 Billet（step[0] 必须是 billet，否则图谱非法）
        let Some(StepKind::Billet) = bp.steps.first().map(|s| s.kind()) else {
            tracing::error!(
                "[bong][forge] blueprint {} must start with billet step",
                bp.id
            );
            continue;
        };
        let billet_profile = match &bp.steps[0] {
            blueprint::StepSpec::Billet { profile } => profile,
            _ => unreachable!(),
        };
        if let Some((material, reason)) = invalid_required_forge_material(billet_profile, &minerals)
        {
            tracing::info!(
                "[bong][forge] rejected blueprint {}: required material `{material}` {reason}",
                bp.id
            );
            continue;
        }

        // 收集投料。optional carrier 允许来自 fauna/spiritwood 等后续专项；required
        // mineral 已在 blueprint load + runtime 双重校验为正典金属。
        let mut inputs: HashMap<String, u32> = HashMap::new();
        for (m, c) in &req.materials {
            *inputs.entry(m.clone()).or_insert(0) += c;
        }
        let billet_res = match resolve_billet(billet_profile, &inputs, bp.tier_cap) {
            Ok(r) => r,
            Err(e) => {
                tracing::info!("[bong][forge] billet waste: {e:?}");
                let id = sessions.allocate_id();
                outcomes.send(ForgeOutcomeEvent {
                    session: id,
                    caster: req.caster,
                    blueprint: bp.id.clone(),
                    bucket: ForgeBucket::Waste,
                    weapon_item: None,
                    quality: 0.0,
                    color: None,
                    side_effects: vec![],
                    achieved_tier: 0,
                });
                continue;
            }
        };

        let id = sessions.allocate_id();
        let mut session = ForgeSession::new(id, bp.id.clone(), req.station, req.caster);
        session.committed_materials = inputs;
        session.step_state = StepState::Billet(billet_res.state.clone());
        session.billet_flawed = billet_res.flawed;
        session.billet_carrier_cap = billet_res.state.resolved_tier_cap;
        session.flawed_marker = billet_res.flawed;
        session.achieved_tier = 1;
        station.session = Some(id);

        tracing::info!(
            "[bong][forge] start session {:?} blueprint={} carrier_cap={}",
            id,
            bp.id,
            billet_res.state.resolved_tier_cap
        );
        sessions.insert(session);
        accepted.send(ForgeStartAccepted {
            session: id,
            station: req.station,
            caster: req.caster,
            blueprint: bp.id.clone(),
            materials: req.materials.clone(),
        });
    }
}

fn invalid_required_forge_material<'a>(
    billet_profile: &'a blueprint::BilletProfile,
    minerals: &MineralRegistry,
) -> Option<(&'a str, &'static str)> {
    for required in &billet_profile.required {
        let Some(entry) = minerals.get_by_str(required.material.as_str()) else {
            return Some((required.material.as_str(), "is not a registered mineral_id"));
        };
        if entry.forge_tier_min == 0 {
            return Some((required.material.as_str(), "is not a forge metal"));
        }
    }
    None
}

fn handle_tempering_hits(
    mut ev: EventReader<TemperingHit>,
    registry: Res<BlueprintRegistry>,
    mut sessions: ResMut<ForgeSessions>,
    casters: Query<(&Cultivation, &SkillSet)>,
) {
    for hit in ev.read() {
        let Some(session) = sessions.get_mut(hit.session) else {
            continue;
        };
        if session.current_step != ForgeStep::Tempering {
            continue;
        }
        let Some(bp) = registry.get(&session.blueprint) else {
            continue;
        };
        let Some(profile) = bp.steps.get(session.step_index).and_then(|s| match s {
            blueprint::StepSpec::Tempering { profile } => Some(profile),
            _ => None,
        }) else {
            continue;
        };
        let forging_lv = casters
            .get(session.caster)
            .ok()
            .map(|(cultivation, skill_set)| forging_effective_lv(cultivation, skill_set))
            .unwrap_or(0);
        let window_bonus = skill_hook::tempering_window_bonus_ticks(forging_lv);
        if let StepState::Tempering(state) = &mut session.step_state {
            apply_tempering_hit(profile, state, hit.beat, hit.ticks_remaining, window_bonus);
        }
    }
}

fn handle_scroll_submits(
    mut ev: EventReader<InscriptionScrollSubmit>,
    mut sessions: ResMut<ForgeSessions>,
) {
    for submit in ev.read() {
        let Some(session) = sessions.get_mut(submit.session) else {
            continue;
        };
        if session.current_step != ForgeStep::Inscription {
            continue;
        }
        if let StepState::Inscription(state) = &mut session.step_state {
            apply_scroll(state, submit.inscription_id.clone());
        }
    }
}

fn handle_consecration_injects(
    mut ev: EventReader<ConsecrationInject>,
    mut sessions: ResMut<ForgeSessions>,
) {
    for inject in ev.read() {
        let Some(session) = sessions.get_mut(inject.session) else {
            continue;
        };
        if session.current_step != ForgeStep::Consecration {
            continue;
        }
        if let StepState::Consecration(state) = &mut session.step_state {
            inject_qi(state, inject.qi_amount);
        }
    }
}

/// StepAdvance 统一收束：根据当前 step 结果推进，若到 Done → 派发 outcome。
#[allow(clippy::too_many_arguments)]
fn handle_step_advance(
    mut ev: EventReader<StepAdvance>,
    registry: Res<BlueprintRegistry>,
    mut sessions: ResMut<ForgeSessions>,
    mut stations: Query<&mut WeaponForgeStation>,
    mut caster_q: Query<ForgeCasterSkillQueryItem>,
    mut history_q: Query<&mut ForgeHistory>,
    mut outcomes: EventWriter<ForgeOutcomeEvent>,
    mut skill_xp_events: EventWriter<SkillXpGain>,
) {
    for advance in ev.read() {
        let Some(session) = sessions.get_mut(advance.session) else {
            continue;
        };
        let Some(bp) = registry.get(&session.blueprint) else {
            continue;
        };

        let prev_step = session.current_step;
        let caster_info =
            caster_q
                .get(session.caster)
                .ok()
                .map(|(cultivation, qi_color, skill_set)| {
                    let forging_lv = forging_effective_lv(cultivation, skill_set);
                    (cultivation.realm, qi_color.main, forging_lv)
                });
        // 对当前步骤做结算。
        let (step_flawed, step_waste) =
            match (&session.step_state, bp.steps.get(session.step_index)) {
                (StepState::Tempering(state), Some(blueprint::StepSpec::Tempering { profile })) => {
                    let miss_bonus = caster_info
                        .map(|(_, _, lv)| skill_hook::allowed_miss_bonus(lv))
                        .unwrap_or(0);
                    let result = resolve_tempering(profile, state, miss_bonus);
                    session.tempering_result = Some(result);
                    (
                        matches!(result, TemperingResult::Flawed | TemperingResult::Good),
                        matches!(result, TemperingResult::Waste),
                    )
                }
                (
                    StepState::Inscription(state),
                    Some(blueprint::StepSpec::Inscription { profile }),
                ) => {
                    let failure_reduction = caster_info
                        .map(|(_, _, lv)| skill_hook::inscription_failure_rate_reduction(lv))
                        .unwrap_or(0.0);
                    let roll =
                        deterministic_step_roll(session.id.0, session.step_index, 0x1bad5eed);
                    let result = resolve_inscription(profile, state, roll, failure_reduction);
                    session.inscription_result = Some(result);
                    (
                        matches!(
                            result,
                            InscriptionResult::Partial | InscriptionResult::Failed
                        ),
                        false,
                    )
                }
                (
                    StepState::Consecration(state),
                    Some(blueprint::StepSpec::Consecration { profile }),
                ) => {
                    let result = caster_info
                        .map(|(realm, color, _)| resolve_consecration(profile, state, color, realm))
                        .unwrap_or(ConsecrationResult::Failed);
                    session.consecration_result = Some(result);
                    (
                        matches!(
                            result,
                            ConsecrationResult::Insufficient | ConsecrationResult::Failed
                        ),
                        false,
                    )
                }
                _ => (false, false),
            };
        if step_waste {
            finalize_outcome(
                session,
                bp,
                ForgeBucket::Waste,
                None,
                &mut stations,
                &mut caster_q,
                &mut history_q,
                &mut outcomes,
                &mut skill_xp_events,
            );
            continue;
        }
        if step_flawed {
            session.flawed_marker = true;
        }

        // 推进 step_index → 下一 step 或 Done
        advance_step(session, bp);

        if prev_step != session.current_step {
            tracing::debug!(
                "[bong][forge] session {:?} advanced {prev_step:?} → {:?}",
                session.id,
                session.current_step
            );
        }

        if session.is_done() {
            // 汇总各步结果 → bucket
            let bucket = finalize_bucket(session, bp);
            finalize_outcome(
                session,
                bp,
                bucket,
                caster_info.map(|(realm, color, _)| (realm, color)),
                &mut stations,
                &mut caster_q,
                &mut history_q,
                &mut outcomes,
                &mut skill_xp_events,
            );
        }
    }
}

fn finalize_bucket(session: &ForgeSession, bp: &blueprint::Blueprint) -> ForgeBucket {
    let billet_ok = session.achieved_tier >= 1;
    let billet_flawed = session.billet_flawed;
    let tempering = if bp.has_step(StepKind::Tempering) {
        session.tempering_result
    } else {
        None
    };
    let inscription = if bp.has_step(StepKind::Inscription) {
        session.inscription_result
    } else {
        None
    };
    let consecration = if bp.has_step(StepKind::Consecration) {
        session.consecration_result
    } else {
        None
    };
    select_bucket(
        billet_ok,
        billet_flawed,
        tempering,
        inscription,
        consecration,
    )
}

#[allow(clippy::too_many_arguments)]
fn finalize_outcome(
    session: &mut ForgeSession,
    bp: &blueprint::Blueprint,
    bucket: ForgeBucket,
    caster_info: Option<(
        crate::cultivation::components::Realm,
        crate::cultivation::components::ColorKind,
    )>,
    stations: &mut Query<&mut WeaponForgeStation>,
    _caster_q: &mut Query<ForgeCasterSkillQueryItem>,
    history_q: &mut Query<&mut ForgeHistory>,
    outcomes: &mut EventWriter<ForgeOutcomeEvent>,
    skill_xp_events: &mut EventWriter<SkillXpGain>,
) {
    // 读取 outcome spec
    let (weapon_item, quality) = match &bucket {
        ForgeBucket::Perfect => bp
            .outcomes
            .perfect
            .as_ref()
            .map(|o| (Some(o.weapon.clone()), o.quality))
            .unwrap_or((None, 0.0)),
        ForgeBucket::Good => bp
            .outcomes
            .good
            .as_ref()
            .map(|o| (Some(o.weapon.clone()), o.quality))
            .unwrap_or((None, 0.0)),
        ForgeBucket::Flawed => {
            if let Some(fb) = &bp.flawed_fallback {
                let base = bp
                    .outcomes
                    .flawed
                    .as_ref()
                    .map(|o| o.quality)
                    .unwrap_or(0.5);
                (Some(fb.weapon.clone()), fallback::flawed_quality(fb, base))
            } else {
                bp.outcomes
                    .flawed
                    .as_ref()
                    .map(|o| (Some(o.weapon.clone()), o.quality))
                    .unwrap_or((None, 0.0))
            }
        }
        ForgeBucket::Waste => (None, 0.0),
        ForgeBucket::Explode => (None, 0.0),
    };

    // side effects（仅 flawed 抽取）
    let mut side_effects = Vec::new();
    if matches!(bucket, ForgeBucket::Flawed) {
        if let Some(fb) = &bp.flawed_fallback {
            // 简易决定性：用 session_id 低位当 roll 种子
            let roll = (session.id.0 & 0xffff) as u32;
            if let Some(entry) = fallback::weighted_pick(&fb.side_effect_pool, roll) {
                side_effects.push(entry.tag.clone());
            }
        }
    }

    // 爆炉 → 扣 station integrity
    if matches!(bucket, ForgeBucket::Explode) {
        if let Ok(mut s) = stations.get_mut(session.station) {
            if let Some(ex) = &bp.outcomes.explode {
                s.apply_wear(ex.station_wear);
            }
        }
    }
    // 清 station.session
    if let Ok(mut s) = stations.get_mut(session.station) {
        s.session = None;
    }

    // color：仅 consecration 成功才染色
    let color = if bp.has_step(StepKind::Consecration)
        && matches!(bucket, ForgeBucket::Perfect | ForgeBucket::Good)
    {
        caster_info.map(|(_, c)| c)
    } else {
        None
    };

    let achieved_tier = compute_achieved_tier(
        bp,
        matches!(
            bucket,
            ForgeBucket::Perfect | ForgeBucket::Good | ForgeBucket::Flawed
        ),
        session
            .tempering_result
            .map(|r| !matches!(r, TemperingResult::Flawed | TemperingResult::Waste)),
        session
            .inscription_result
            .map(|r| matches!(r, InscriptionResult::Filled)),
        session
            .consecration_result
            .map(|r| matches!(r, ConsecrationResult::Succeeded { .. })),
        session.billet_carrier_cap,
    );

    // Append LifeRecord / ForgeHistory
    if let Ok(mut h) = history_q.get_mut(session.caster) {
        h.push(ForgeAttempt {
            tick: 0,
            blueprint: bp.id.clone(),
            bucket_tag: ForgeAttempt::from_bucket(&bucket),
            achieved_tier,
            weapon_item: weapon_item.clone(),
            quality,
            color,
            side_effects: side_effects.clone(),
        });
    }

    session.current_step = ForgeStep::Done;

    // plan-skill-v1 §10 forge 钩子：按分步累加算 XP 发 SkillXpGain（Forging）。
    // 数值 source-of-truth 见 `forge::skill_hook::xp_for_outcome`（plan §7.3）。
    let xp = skill_hook::xp_for_outcome(
        bucket,
        bp.has_step(StepKind::Tempering),
        bp.has_step(StepKind::Inscription),
        bp.has_step(StepKind::Consecration),
    );
    skill_xp_events.send(SkillXpGain {
        char_entity: session.caster,
        skill: SkillId::Forging,
        amount: xp,
        source: XpGainSource::Action {
            plan_id: "forge",
            action: forge_action_for_bucket(bucket),
        },
    });

    outcomes.send(ForgeOutcomeEvent {
        session: session.id,
        caster: session.caster,
        blueprint: bp.id.clone(),
        bucket,
        weapon_item,
        quality,
        color,
        side_effects,
        achieved_tier,
    });
}

/// plan §7.3 action 名对齐（供 agent narration 按结局区分）。
fn forge_action_for_bucket(bucket: ForgeBucket) -> &'static str {
    match bucket {
        ForgeBucket::Perfect => "craft_perfect",
        ForgeBucket::Good => "craft_good",
        ForgeBucket::Flawed => "craft_flawed",
        ForgeBucket::Waste => "craft_waste",
        ForgeBucket::Explode => "craft_explode",
    }
}

fn forging_effective_lv(cultivation: &Cultivation, skill_set: &SkillSet) -> u8 {
    let real_lv = skill_set
        .skills
        .get(&SkillId::Forging)
        .map(|entry| entry.lv)
        .unwrap_or(0);
    effective_lv(real_lv, skill_cap_for_realm(cultivation.realm))
}

fn deterministic_step_roll(session_seed: u64, step_index: usize, salt: u64) -> f32 {
    let mut x = session_seed ^ ((step_index as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)) ^ salt;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    (x as f64 / u64::MAX as f64).clamp(0.0, 0.999_999) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forge::blueprint::{BilletProfile, BilletTolerance, CarrierSpec, MaterialStack};

    #[test]
    fn runtime_required_material_accepts_forge_metal() {
        let minerals = build_default_mineral_registry();
        let profile = BilletProfile {
            required: vec![MaterialStack {
                material: "fan_tie".into(),
                count: 3,
            }],
            optional_carriers: vec![CarrierSpec {
                material: "ling_wood".into(),
                unlocks_tier: 3,
            }],
            tolerance: BilletTolerance::default(),
        };

        assert_eq!(invalid_required_forge_material(&profile, &minerals), None);
    }

    #[test]
    fn runtime_required_material_rejects_non_metal_mineral() {
        let minerals = build_default_mineral_registry();
        let profile = BilletProfile {
            required: vec![MaterialStack {
                material: "dan_sha".into(),
                count: 1,
            }],
            optional_carriers: vec![],
            tolerance: BilletTolerance::default(),
        };

        assert_eq!(
            invalid_required_forge_material(&profile, &minerals),
            Some(("dan_sha", "is not a forge metal"))
        );
    }
}
