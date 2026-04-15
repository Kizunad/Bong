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

pub mod blueprint;
pub mod events;
pub mod fallback;
pub mod history;
pub mod learned;
pub mod session;
pub mod station;
pub mod steps;

use std::collections::HashMap;

use valence::prelude::{
    App, EventReader, EventWriter, IntoSystemConfigs, Query, Res, ResMut, Update,
};

use self::blueprint::{BlueprintRegistry, StepKind, DEFAULT_BLUEPRINTS_DIR};
use self::events::{
    ConsecrationInject, ForgeBucket, ForgeOutcomeEvent, InscriptionScrollSubmit, StartForgeRequest,
    StepAdvance, TemperingHit,
};
use self::history::{ForgeAttempt, ForgeHistory};
use self::learned::LearnedBlueprints;
use self::session::{ForgeSession, ForgeSessions, ForgeStep, StepState};
use self::station::WeaponForgeStation;
use self::steps::{
    advance_step, apply_scroll, apply_tempering_hit, compute_achieved_tier, inject_qi,
    resolve_billet, resolve_tempering, select_bucket, ConsecrationResult, InscriptionResult,
    TemperingResult,
};
use crate::cultivation::components::{Cultivation, QiColor};

pub fn register(app: &mut App) {
    tracing::info!("[bong][forge] registering plan-forge-v1 systems");

    let registry = BlueprintRegistry::load_dir(DEFAULT_BLUEPRINTS_DIR).unwrap_or_else(|e| {
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
    app.add_event::<ForgeOutcomeEvent>();

    app.add_systems(
        Update,
        (
            handle_start_forge_requests,
            handle_tempering_hits.after(handle_start_forge_requests),
            handle_scroll_submits.after(handle_tempering_hits),
            handle_consecration_injects.after(handle_scroll_submits),
            handle_step_advance.after(handle_consecration_injects),
        ),
    );
}

// ══════════════════════════════ Systems ══════════════════════════════

fn handle_start_forge_requests(
    mut ev: EventReader<StartForgeRequest>,
    registry: Res<BlueprintRegistry>,
    mut sessions: ResMut<ForgeSessions>,
    mut stations: Query<&mut WeaponForgeStation>,
    learned: Query<&LearnedBlueprints>,
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

        // 收集投料
        let mut inputs: HashMap<String, u32> = HashMap::new();
        for (m, c) in &req.materials {
            *inputs.entry(m.clone()).or_insert(0) += c;
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
        let billet_res = match resolve_billet(billet_profile, &inputs, bp.tier_cap) {
            Ok(r) => r,
            Err(e) => {
                tracing::info!("[bong][forge] billet waste: {e:?}");
                let id = sessions.allocate_id();
                outcomes.send(ForgeOutcomeEvent {
                    session: id,
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
    }
}

fn handle_tempering_hits(
    mut ev: EventReader<TemperingHit>,
    registry: Res<BlueprintRegistry>,
    mut sessions: ResMut<ForgeSessions>,
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
        if let StepState::Tempering(state) = &mut session.step_state {
            apply_tempering_hit(profile, state, hit.beat, hit.ticks_remaining);
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
fn handle_step_advance(
    mut ev: EventReader<StepAdvance>,
    registry: Res<BlueprintRegistry>,
    mut sessions: ResMut<ForgeSessions>,
    mut stations: Query<&mut WeaponForgeStation>,
    mut caster_q: Query<(&Cultivation, &QiColor)>,
    mut history_q: Query<&mut ForgeHistory>,
    mut outcomes: EventWriter<ForgeOutcomeEvent>,
) {
    for advance in ev.read() {
        let Some(session) = sessions.get_mut(advance.session) else {
            continue;
        };
        let Some(bp) = registry.get(&session.blueprint) else {
            continue;
        };

        let prev_step = session.current_step;
        // 对当前步骤做结算。
        let (tempering_flawed, tempering_waste) =
            match (&session.step_state, bp.steps.get(session.step_index)) {
                (StepState::Tempering(state), Some(blueprint::StepSpec::Tempering { profile })) => {
                    let r = resolve_tempering(profile, state);
                    (
                        matches!(r, TemperingResult::Flawed | TemperingResult::Good),
                        matches!(r, TemperingResult::Waste),
                    )
                }
                _ => (false, false),
            };
        if tempering_waste {
            finalize_outcome(
                session,
                bp,
                ForgeBucket::Waste,
                None,
                &mut stations,
                &mut caster_q,
                &mut history_q,
                &mut outcomes,
            );
            continue;
        }
        if tempering_flawed {
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
            // 从 caster 读 color/realm 用于 consecration（若未来存 step_state 中更好，这里简化）
            let caster_info = caster_q
                .get(session.caster)
                .ok()
                .map(|(c, qc)| (c.realm, qc.main));
            finalize_outcome(
                session,
                bp,
                bucket,
                caster_info,
                &mut stations,
                &mut caster_q,
                &mut history_q,
                &mut outcomes,
            );
        }
    }
}

fn finalize_bucket(session: &ForgeSession, bp: &blueprint::Blueprint) -> ForgeBucket {
    // 简化：当前实现中非 tempering 步骤的 resolution 需要调用方传 roll —— 这里走最保守判定
    // （真正 roll 应在 step_advance 分步落盘，本 MVP 用 flawed_marker + 步骤缺失兜底）。
    let billet_ok = session.achieved_tier >= 1;
    let billet_flawed = session.flawed_marker;
    let tempering = if bp.has_step(StepKind::Tempering) {
        Some(if session.flawed_marker {
            TemperingResult::Flawed
        } else {
            TemperingResult::Perfect
        })
    } else {
        None
    };
    let inscription = if bp.has_step(StepKind::Inscription) {
        Some(InscriptionResult::Filled)
    } else {
        None
    };
    let consecration = if bp.has_step(StepKind::Consecration) {
        Some(ConsecrationResult::Succeeded {
            color: crate::cultivation::components::ColorKind::Mellow,
        })
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
    _caster_q: &mut Query<(&Cultivation, &QiColor)>,
    history_q: &mut Query<&mut ForgeHistory>,
    outcomes: &mut EventWriter<ForgeOutcomeEvent>,
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
        if bp.has_step(StepKind::Tempering) {
            Some(!matches!(bucket, ForgeBucket::Flawed | ForgeBucket::Waste))
        } else {
            None
        },
        if bp.has_step(StepKind::Inscription) {
            Some(!matches!(bucket, ForgeBucket::Flawed | ForgeBucket::Waste))
        } else {
            None
        },
        if bp.has_step(StepKind::Consecration) {
            Some(matches!(bucket, ForgeBucket::Perfect | ForgeBucket::Good))
        } else {
            None
        },
        match &session.step_state {
            StepState::Billet(b) => b.resolved_tier_cap,
            _ => bp.tier_cap,
        },
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

    outcomes.send(ForgeOutcomeEvent {
        session: session.id,
        blueprint: bp.id.clone(),
        bucket,
        weapon_item,
        quality,
        color,
        side_effects,
        achieved_tier,
    });
}
