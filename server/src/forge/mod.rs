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

pub mod artifact_color;
pub mod artifact_meridian;
pub mod blueprint;
pub mod events;
pub mod fallback;
pub mod history;
pub mod inventory_bridge;
pub mod learned;
pub mod processing_mode;
pub mod resonance;
pub mod session;
pub mod skill_hook;
pub mod station;
pub mod steps;

use std::collections::HashMap;

use valence::prelude::{
    App, DVec3, EventReader, EventWriter, Events, IntoSystemConfigs, Query, Res, ResMut, Update,
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
use crate::mineral::MineralFeedbackEvent;
use crate::mineral::{build_default_registry as build_default_mineral_registry, MineralRegistry};
use crate::network::{gameplay_vfx, vfx_event_emit::VfxEventRequest};
use crate::qi_physics::ledger::{QiAccountId, QiTransfer, QiTransferReason, WorldQiAccount};
use crate::skill::components::{SkillId, SkillSet};
use crate::skill::curve::effective_lv;
use crate::skill::events::{SkillXpGain, XpGainSource};
use crate::world::dimension::DimensionKind;
use crate::world::events::EVENT_REALM_COLLAPSE;
use crate::world::zone::ZoneRegistry;

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
    app.add_event::<artifact_meridian::ArtifactMeridianDepthChanged>();
    app.add_event::<artifact_meridian::ArtifactMeridianCracked>();
    app.add_event::<artifact_meridian::ArtifactTierEvolved>();
    app.add_event::<station::PlaceForgeStationRequest>();
    app.add_event::<processing_mode::StartForgeProcessingRequest>();
    app.add_event::<processing_mode::ForgeProcessingAccepted>();

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
            processing_mode::forge_processing_mode_handler,
        ),
    );

    app.add_systems(
        Update,
        (
            artifact_meridian::artifact_meridian_deepen_on_use
                .in_set(crate::combat::CombatSystemSet::Emit)
                .after(crate::combat::resolve::resolve_attack_intents),
            artifact_meridian::artifact_tier_evolved_narration
                .in_set(crate::combat::CombatSystemSet::Emit)
                .after(artifact_meridian::artifact_meridian_deepen_on_use),
            artifact_meridian::artifact_color_evolve_tick
                .in_set(crate::combat::CombatSystemSet::Emit),
            artifact_meridian::artifact_meridian_maintenance_tick
                .in_set(crate::combat::CombatSystemSet::Emit),
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
    mut feedback: EventWriter<MineralFeedbackEvent>,
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

        if let Err(error) = bp.validate_with(&minerals, station.tier) {
            match error {
                blueprint::ForgeValidationError::TierMismatch {
                    material_name,
                    required_tier,
                    ..
                } => {
                    feedback.send(MineralFeedbackEvent::forge_tier_mismatch(
                        req.caster,
                        forge_station_tier_name(station.tier),
                        material_name,
                        required_tier,
                    ));
                }
                blueprint::ForgeValidationError::UnknownMaterial { .. } => {
                    feedback.send(MineralFeedbackEvent::unknown_for_forge(req.caster));
                }
                blueprint::ForgeValidationError::NotForgeMetal { material } => {
                    feedback.send(MineralFeedbackEvent::invalid_for_forge(
                        req.caster, material,
                    ));
                }
            }
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
                    consecration_qi_amount: 0.0,
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
            if blueprint::is_allowed_item_material(required.material.as_str()) {
                continue;
            }
            return Some((
                required.material.as_str(),
                "is not a registered mineral_id or forge item material",
            ));
        };
        if entry.forge_tier_min == 0 {
            return Some((required.material.as_str(), "is not a forge metal"));
        }
    }
    None
}

fn forge_station_tier_name(tier: u8) -> &'static str {
    match tier {
        1 => "凡铁炉",
        2 => "灵铁炉",
        3 => "稀铁炉",
        4..=u8::MAX => "道炉",
        0 => "无炉",
    }
}

fn handle_tempering_hits(
    mut ev: EventReader<TemperingHit>,
    registry: Res<BlueprintRegistry>,
    mut sessions: ResMut<ForgeSessions>,
    casters: Query<(&Cultivation, &SkillSet)>,
    stations: Query<&WeaponForgeStation>,
    mut vfx_events: Option<ResMut<Events<VfxEventRequest>>>,
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
            if let (Some(events), Ok(station)) =
                (vfx_events.as_deref_mut(), stations.get(session.station))
            {
                if let Some(origin) = forge_station_origin(station) {
                    gameplay_vfx::send_spawn(
                        events,
                        gameplay_vfx::spawn_request(
                            gameplay_vfx::FORGE_HAMMER_STRIKE,
                            origin,
                            Some([0.0, 0.8, 0.0]),
                            "#FF8800",
                            0.8,
                            8,
                            20,
                        ),
                    );
                }
            }
        }
    }
}

fn handle_scroll_submits(
    mut ev: EventReader<InscriptionScrollSubmit>,
    mut sessions: ResMut<ForgeSessions>,
    stations: Query<&WeaponForgeStation>,
    mut vfx_events: Option<ResMut<Events<VfxEventRequest>>>,
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
            if let (Some(events), Ok(station)) =
                (vfx_events.as_deref_mut(), stations.get(session.station))
            {
                if let Some(origin) = forge_station_origin(station) {
                    gameplay_vfx::send_spawn(
                        events,
                        gameplay_vfx::spawn_request(
                            gameplay_vfx::FORGE_INSCRIPTION,
                            origin,
                            None,
                            "#4488FF",
                            0.8,
                            1,
                            20,
                        ),
                    );
                }
            }
        }
    }
}

fn handle_consecration_injects(
    mut ev: EventReader<ConsecrationInject>,
    mut sessions: ResMut<ForgeSessions>,
    stations: Query<&WeaponForgeStation>,
    zone_registry: Option<Res<ZoneRegistry>>,
    mut vfx_events: Option<ResMut<Events<VfxEventRequest>>>,
    mut qi_account: Option<ResMut<WorldQiAccount>>,
    mut cultivations: Query<&mut Cultivation>,
) {
    for inject in ev.read() {
        let Some(session) = sessions.get_mut(inject.session) else {
            continue;
        };
        if session.current_step != ForgeStep::Consecration {
            continue;
        }
        if stations
            .get(session.station)
            .ok()
            .is_some_and(|station| station_zone_is_collapsed(station, zone_registry.as_deref()))
        {
            tracing::debug!(
                "[bong][forge] consecration inject ignored: station={:?} is in collapsed zone",
                session.station
            );
            continue;
        }

        // 守恒：从玩家真元扣减，转入 zone ledger。
        // 玩家真元在 Cultivation.qi_current（ECS），不在 WorldQiAccount balances，
        // 所以不走 WorldQiAccount::transfer（会检查不存在的 player ledger 余额）。
        // 照搬 dandao/boss_spawn.rs:386-410 BossDrain 模式。
        let caster = session.caster;
        let station_entity = session.station;

        let injected = if let Ok(mut cultivation) = cultivations.get_mut(caster) {
            // 守恒原则：「扣玩家真元」与「ledger 记账」必须原子绑定。
            // 若 WorldQiAccount 资源缺失，跳过整段注入（返回 0.0）——绝不出现
            // 「qi_current 已扣但 ledger 未记账」的孤立路径。
            // 参照 dandao/boss_spawn.rs BossDrain 早返回模式。
            let Some(ref mut account) = qi_account else {
                tracing::warn!(
                    "[bong][forge] consecration inject skipped: WorldQiAccount resource missing (caster={:?})",
                    caster
                );
                continue;
            };

            // 钳制：绝不信任 client 上报的 qi_amount，以 ECS qi_current 为准。
            // 显式拒绝非有限值（NaN/Inf）——NaN.min(x) 在 Rust 返回 x，会绕过钳制
            // 变成全额注入。上游 handler 已校验 finite，此处守恒结算点再守一道
            // （防御纵深，防未来其它 ConsecrationInject 生产者绕过校验）。
            let available = if cultivation.qi_current.is_finite() {
                cultivation.qi_current.max(0.0)
            } else {
                0.0
            };
            let requested = if inject.qi_amount.is_finite() {
                inject.qi_amount.max(0.0)
            } else {
                0.0
            };
            let clamped = requested.min(available);

            // 守恒：先写 ledger（带错误检查），账本写成功后才扣玩家真元。
            // 块求值为**实际注入量**：ledger 写失败或 clamped<=0 时为 0.0，
            // 使 inject_qi/consecration_qi_injected 只反映真实搬运的真元。
            if clamped > 0.0 {
                // 解析 station 所属 zone 名
                let zone_id = stations
                    .get(station_entity)
                    .ok()
                    .and_then(|station| station.pos)
                    .and_then(|(x, y, z)| {
                        zone_registry.as_deref().and_then(|zr| {
                            zr.find_zone(
                                DimensionKind::Overworld,
                                DVec3::new(f64::from(x) + 0.5, f64::from(y), f64::from(z) + 0.5),
                            )
                            .map(|zone| QiAccountId::zone(zone.name.clone()))
                        })
                    })
                    .unwrap_or_else(|| {
                        // zone 不可解析：fallback 到 overflow，真元绝不凭空消失
                        QiAccountId::overflow(format!(
                            "forge_consecration_no_zone:{}",
                            station_entity.to_bits()
                        ))
                    });

                // 确保目标账户存在
                if !account.has_account(&zone_id) {
                    let _ = account.set_balance(zone_id.clone(), 0.0);
                }

                // 守恒原子性：先写 ledger（带错误检查），成功后才扣玩家真元。
                // 若 set_balance 失败（理论上不会——clamped/zone_balance 均有限非负），
                // 不扣 qi_current、不记审计，守恒不破（什么都没发生），注入量计 0。
                let zone_balance = account.balance(&zone_id);
                match account.set_balance(zone_id.clone(), zone_balance + clamped) {
                    Ok(()) => {
                        cultivation.qi_current = (cultivation.qi_current - clamped).max(0.0);
                        let player_id = QiAccountId::player(format!("entity:{}", caster.to_bits()));
                        account.push_transfer_audit(QiTransfer {
                            from: player_id,
                            to: zone_id,
                            amount: clamped,
                            reason: QiTransferReason::Crafting,
                        });
                        clamped
                    }
                    Err(e) => {
                        tracing::warn!(
                            "[bong][forge] consecration inject skipped: ledger credit failed ({e}), player qi preserved (caster={:?})",
                            caster
                        );
                        0.0
                    }
                }
            } else {
                0.0
            }
        } else {
            // caster 无 Cultivation 组件（理论上不应发生）→ 跳过注入
            tracing::debug!(
                "[bong][forge] consecration inject skipped: caster={:?} has no Cultivation component",
                caster
            );
            continue;
        };

        if let StepState::Consecration(state) = &mut session.step_state {
            // 使用钳制后的量，保证 consecration_qi_injected 反映真实注入真元
            inject_qi(state, injected);
            if let (Some(events), Ok(station)) =
                (vfx_events.as_deref_mut(), stations.get(session.station))
            {
                if let Some(origin) = forge_station_origin(station) {
                    gameplay_vfx::send_spawn(
                        events,
                        gameplay_vfx::spawn_request(
                            gameplay_vfx::FORGE_CONSECRATION,
                            origin,
                            Some([0.0, 1.0, 0.0]),
                            "#FFFFFF",
                            0.9,
                            10,
                            18,
                        ),
                    );
                }
            }
        }
    }
}

fn forge_station_origin(station: &WeaponForgeStation) -> Option<DVec3> {
    station
        .pos
        .map(|(x, y, z)| DVec3::new(f64::from(x) + 0.5, f64::from(y) + 0.8, f64::from(z) + 0.5))
}

fn station_zone_is_collapsed(
    station: &WeaponForgeStation,
    zone_registry: Option<&ZoneRegistry>,
) -> bool {
    let Some(zone_registry) = zone_registry else {
        return false;
    };
    let Some((x, y, z)) = station.pos else {
        return false;
    };
    let station_pos = DVec3::new(x as f64 + 0.5, y as f64, z as f64 + 0.5);
    zone_registry
        .find_zone(DimensionKind::Overworld, station_pos)
        .is_some_and(|zone| {
            zone.active_events
                .iter()
                .any(|event| event == EVENT_REALM_COLLAPSE)
        })
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
        let mut consecration_qi_injected = None;
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
                    consecration_qi_injected = Some(state.qi_injected);
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
        if let Some(qi_injected) = consecration_qi_injected {
            session.consecration_qi_injected = qi_injected;
        }
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
        consecration_qi_amount: if bp.has_step(StepKind::Consecration) {
            session.consecration_qi_injected
        } else {
            0.0
        },
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
    use crate::forge::blueprint::{
        BilletProfile, BilletTolerance, CarrierSpec, MaterialStack, TemperBeat,
    };
    use crate::forge::session::ForgeSessionId;
    use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};
    use valence::prelude::{App, BlockPos, Update};

    #[test]
    fn runtime_required_material_accepts_forge_metal() {
        let minerals = build_default_mineral_registry();
        let profile = BilletProfile {
            required: vec![MaterialStack {
                material: "fan_tie".into(),
                count: 3,
            }],
            optional_carriers: vec![CarrierSpec {
                material: "ling_mu_ban".into(),
                unlocks_tier: 3,
            }],
            tolerance: BilletTolerance::default(),
        };

        assert_eq!(invalid_required_forge_material(&profile, &minerals), None);
    }

    #[test]
    fn runtime_required_material_accepts_spiritwood_item_materials() {
        let minerals = build_default_mineral_registry();
        let profile = BilletProfile {
            required: vec![
                MaterialStack {
                    material: "ling_mu_ban".into(),
                    count: 2,
                },
                MaterialStack {
                    material: "feng_he_gu".into(),
                    count: 2,
                },
            ],
            optional_carriers: vec![],
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

    #[test]
    fn collapsed_zone_blocks_consecration_qi_injection() {
        let mut app = App::new();
        app.add_event::<ConsecrationInject>();
        app.add_systems(Update, handle_consecration_injects);

        let mut zones = ZoneRegistry::fallback();
        zones
            .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
            .unwrap()
            .active_events
            .push(EVENT_REALM_COLLAPSE.to_string());
        app.insert_resource(zones);

        let station = app
            .world_mut()
            .spawn(WeaponForgeStation::placed(
                BlockPos::new(8, 66, 8),
                1,
                valence::prelude::Entity::PLACEHOLDER,
            ))
            .id();
        let session_id = ForgeSessionId(7);
        let mut sessions = ForgeSessions::new();
        let mut session = ForgeSession::new(
            session_id,
            "qing_feng_v0".to_string(),
            station,
            valence::prelude::Entity::PLACEHOLDER,
        );
        session.current_step = ForgeStep::Consecration;
        session.step_state = StepState::Consecration(Default::default());
        sessions.insert(session);
        app.insert_resource(sessions);

        app.world_mut().send_event(ConsecrationInject {
            session: session_id,
            qi_amount: 5.0,
        });
        app.update();

        let sessions = app.world().resource::<ForgeSessions>();
        let session = sessions.get(session_id).unwrap();
        match &session.step_state {
            StepState::Consecration(state) => assert_eq!(state.qi_injected, 0.0),
            other => panic!("expected consecration state, got {other:?}"),
        }
        assert!(app.world().entity(station).contains::<WeaponForgeStation>());
    }

    // ── helper: build a minimal App for consecration inject tests ──────────────────
    fn consecration_app_with_zone() -> (App, valence::prelude::Entity, ForgeSessionId) {
        let mut app = App::new();
        app.add_event::<ConsecrationInject>();
        app.insert_resource(WorldQiAccount::default());
        app.insert_resource(ZoneRegistry::fallback());
        app.add_systems(Update, handle_consecration_injects);

        let station = app
            .world_mut()
            .spawn(WeaponForgeStation::placed(
                BlockPos::new(8, 66, 8),
                1,
                valence::prelude::Entity::PLACEHOLDER,
            ))
            .id();
        (app, station, ForgeSessionId(42))
    }

    fn insert_session_with_caster(
        app: &mut App,
        session_id: ForgeSessionId,
        station: valence::prelude::Entity,
        caster: valence::prelude::Entity,
    ) {
        let mut sessions = ForgeSessions::new();
        let mut session =
            ForgeSession::new(session_id, "qing_feng_v0".to_string(), station, caster);
        session.current_step = ForgeStep::Consecration;
        session.step_state = StepState::Consecration(Default::default());
        sessions.insert(session);
        app.insert_resource(sessions);
    }

    // ── plan-qi-conservation-leaks-v1 P1 — 守恒测试 ──────────────────────────────

    #[test]
    fn consecration_inject_no_zone_falls_back_to_overflow() {
        // 期望：station 在所有 zone 之外 → find_zone None → 记入 Overflow 账户
        // （真元绝不凭空消失）。守恒：玩家减少量 == overflow 账户增加量。
        let mut app = App::new();
        app.add_event::<ConsecrationInject>();
        app.insert_resource(WorldQiAccount::default());
        app.insert_resource(ZoneRegistry::fallback());
        app.add_systems(Update, handle_consecration_injects);
        // fallback 所有 zone 都在 ~[-128,128] 内；放到远处确保 find_zone 返回 None
        let station = app
            .world_mut()
            .spawn(WeaponForgeStation::placed(
                BlockPos::new(1_000_000, 66, 1_000_000),
                1,
                valence::prelude::Entity::PLACEHOLDER,
            ))
            .id();
        let session_id = ForgeSessionId(7);
        let caster = app
            .world_mut()
            .spawn(Cultivation {
                qi_current: 40.0,
                qi_max: 100.0,
                ..Default::default()
            })
            .id();
        insert_session_with_caster(&mut app, session_id, station, caster);

        let inject_amount = 12.0_f64;
        app.world_mut().send_event(ConsecrationInject {
            session: session_id,
            qi_amount: inject_amount,
        });
        app.update();

        // 玩家真元应减少 inject_amount（即使落 overflow，扣减照常）
        let qi_after = app
            .world()
            .entity(caster)
            .get::<Cultivation>()
            .unwrap()
            .qi_current;
        assert!(
            (qi_after - (40.0 - inject_amount)).abs() < 1e-9,
            "期望 caster qi_current={}（40-12），实际={qi_after}",
            40.0 - inject_amount
        );

        let account = app.world().resource::<WorldQiAccount>();
        let transfers = account.transfers();
        assert_eq!(
            transfers.len(),
            1,
            "期望恰好一条审计记录，实际={}",
            transfers.len()
        );
        let t = &transfers[0];
        assert_eq!(
            t.to.kind,
            crate::qi_physics::ledger::QiAccountKind::Overflow,
            "期望 zone 不可解析时 to.kind=Overflow（真元不消失），实际={:?}",
            t.to.kind
        );
        assert!(
            (t.amount - inject_amount).abs() < 1e-9,
            "期望 overflow transfer.amount={inject_amount}，实际={}",
            t.amount
        );
        // 守恒：overflow 账户余额 == 玩家减少量
        let overflow_balance = account.balance(&t.to);
        assert!(
            (overflow_balance - inject_amount).abs() < 1e-9,
            "期望 overflow 账户余额={inject_amount}（== 玩家减少量，守恒），实际={overflow_balance}"
        );
    }

    #[test]
    fn consecration_inject_conserves_world_qi() {
        // 期望：注入前后 (player_qi + zone_ledger_qi) 总量不变；player 减少量 == zone 增加量。
        let (mut app, station, session_id) = consecration_app_with_zone();
        let caster = app
            .world_mut()
            .spawn(Cultivation {
                qi_current: 30.0,
                qi_max: 100.0,
                ..Default::default()
            })
            .id();
        insert_session_with_caster(&mut app, session_id, station, caster);

        let player_qi_before = 30.0_f64;
        let inject_request = 10.0_f64;

        app.world_mut().send_event(ConsecrationInject {
            session: session_id,
            qi_amount: inject_request,
        });
        app.update();

        let player_qi_after = app
            .world()
            .get::<Cultivation>(caster)
            .expect("caster should still have Cultivation")
            .qi_current;

        let account = app.world().resource::<WorldQiAccount>();
        let zone_id = QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME.to_string());
        let zone_balance = account.balance(&zone_id);

        let player_delta = player_qi_before - player_qi_after;
        assert!(
            (player_delta - zone_balance).abs() < 1e-9,
            "守恒失败：player 减少 {player_delta}，zone 增加 {zone_balance}（二者应相等）"
        );
        assert!(
            player_delta > 0.0,
            "期望 player qi 减少 {inject_request}，实际 player_delta={player_delta}"
        );

        let sessions = app.world().resource::<ForgeSessions>();
        let state = sessions.get(session_id).unwrap();
        let qi_injected = match &state.step_state {
            StepState::Consecration(s) => s.qi_injected,
            _ => panic!("session should be in Consecration step state"),
        };
        assert!(
            (qi_injected - player_delta).abs() < 1e-9,
            "consecration_qi_injected={qi_injected} 应等于 player 实际减少量 {player_delta}"
        );
    }

    #[test]
    fn consecration_inject_clamps_to_player_balance() {
        // 期望：请求量 > qi_current → 只注入 qi_current；qi_current 落 0；无通胀。
        let (mut app, station, session_id) = consecration_app_with_zone();
        let initial_qi = 5.0_f64;
        let caster = app
            .world_mut()
            .spawn(Cultivation {
                qi_current: initial_qi,
                qi_max: 100.0,
                ..Default::default()
            })
            .id();
        insert_session_with_caster(&mut app, session_id, station, caster);

        app.world_mut().send_event(ConsecrationInject {
            session: session_id,
            qi_amount: 999.0, // client 上报的虚假大值
        });
        app.update();

        let qi_after = app.world().get::<Cultivation>(caster).unwrap().qi_current;
        assert!(
            qi_after.abs() < 1e-9,
            "期望 qi_current==0（钳制后全扣），实际={qi_after}（仍有通胀）"
        );

        let account = app.world().resource::<WorldQiAccount>();
        let zone_id = QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME.to_string());
        let zone_balance = account.balance(&zone_id);
        assert!(
            (zone_balance - initial_qi).abs() < 1e-9,
            "zone 应增加 {initial_qi}（钳制后），实际 zone_balance={zone_balance}"
        );

        let sessions = app.world().resource::<ForgeSessions>();
        let qi_injected = match &sessions.get(session_id).unwrap().step_state {
            StepState::Consecration(s) => s.qi_injected,
            _ => panic!("session should be in Consecration step"),
        };
        assert!(
            (qi_injected - initial_qi).abs() < 1e-9,
            "consecration_qi_injected={qi_injected} 应等于初始 qi_current={initial_qi}，不是请求的 999"
        );
    }

    #[test]
    fn consecration_inject_exact_balance() {
        // 期望：请求量 == qi_current → 全注入，qi_current=0，zone 增加 full amount。
        let (mut app, station, session_id) = consecration_app_with_zone();
        let qi = 20.0_f64;
        let caster = app
            .world_mut()
            .spawn(Cultivation {
                qi_current: qi,
                qi_max: 100.0,
                ..Default::default()
            })
            .id();
        insert_session_with_caster(&mut app, session_id, station, caster);

        app.world_mut().send_event(ConsecrationInject {
            session: session_id,
            qi_amount: qi,
        });
        app.update();

        let qi_after = app.world().get::<Cultivation>(caster).unwrap().qi_current;
        assert!(
            qi_after.abs() < 1e-9,
            "期望 qi_current=0（精确全注入），实际={qi_after}"
        );

        let account = app.world().resource::<WorldQiAccount>();
        let zone_id = QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME.to_string());
        let zone_balance = account.balance(&zone_id);
        assert!(
            (zone_balance - qi).abs() < 1e-9,
            "zone 应增加 {qi}，实际={zone_balance}"
        );
    }

    #[test]
    fn consecration_inject_no_cultivation_is_noop() {
        // 期望：caster 无 Cultivation 组件 → 跳过注入，qi_injected 仍为 0，zone balance 不变。
        let (mut app, station, session_id) = consecration_app_with_zone();
        // spawn caster WITHOUT Cultivation component
        let caster = app.world_mut().spawn(()).id();
        insert_session_with_caster(&mut app, session_id, station, caster);

        app.world_mut().send_event(ConsecrationInject {
            session: session_id,
            qi_amount: 10.0,
        });
        app.update();

        let sessions = app.world().resource::<ForgeSessions>();
        let qi_injected = match &sessions.get(session_id).unwrap().step_state {
            StepState::Consecration(s) => s.qi_injected,
            _ => panic!("session should still be in Consecration step"),
        };
        assert!(
            qi_injected.abs() < 1e-9,
            "期望 qi_injected=0（无 Cultivation），实际={qi_injected}"
        );

        let account = app.world().resource::<WorldQiAccount>();
        let zone_id = QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME.to_string());
        let zone_balance = account.balance(&zone_id);
        assert!(
            zone_balance.abs() < 1e-9,
            "期望 zone_balance=0（noop），实际={zone_balance}"
        );
    }

    #[test]
    fn consecration_inject_audit_trail_records_transfer() {
        // 期望：transfers() 中包含对应的 QiTransfer(player→zone, amount, Crafting)。
        let (mut app, station, session_id) = consecration_app_with_zone();
        let caster = app
            .world_mut()
            .spawn(Cultivation {
                qi_current: 50.0,
                qi_max: 100.0,
                ..Default::default()
            })
            .id();
        insert_session_with_caster(&mut app, session_id, station, caster);

        let inject_amount = 15.0_f64;
        app.world_mut().send_event(ConsecrationInject {
            session: session_id,
            qi_amount: inject_amount,
        });
        app.update();

        let account = app.world().resource::<WorldQiAccount>();
        let transfers = account.transfers();
        assert!(
            !transfers.is_empty(),
            "期望 transfers 非空（应有一条 Crafting 审计记录），实际为空"
        );
        let t = &transfers[0];
        assert_eq!(
            t.reason,
            QiTransferReason::Crafting,
            "期望 reason=Crafting，实际={:?}",
            t.reason
        );
        assert!(
            (t.amount - inject_amount).abs() < 1e-9,
            "期望 transfer.amount={inject_amount}，实际={}",
            t.amount
        );
        assert_eq!(
            t.from.kind,
            crate::qi_physics::ledger::QiAccountKind::Player,
            "期望 from.kind=Player，实际={:?}",
            t.from.kind
        );
        assert_eq!(
            t.to.kind,
            crate::qi_physics::ledger::QiAccountKind::Zone,
            "期望 to.kind=Zone（station 所属 zone），实际={:?}",
            t.to.kind
        );
        assert_eq!(
            t.to.id, DEFAULT_SPAWN_ZONE_NAME,
            "期望 to.id=\"{}\"（spawn zone），实际=\"{}\"",
            DEFAULT_SPAWN_ZONE_NAME, t.to.id
        );
    }

    #[test]
    fn consecration_inject_no_qi_account_is_noop() {
        // 守恒守卫：当 WorldQiAccount 资源不存在时，玩家真元绝不被静默扣除。
        // 这是「qi_current 扣减」与「ledger 记账」原子绑定的守门测试——
        // 若代码在无 ledger 情况下仍扣 qi，此测试必须撞红。
        let mut app = App::new();
        app.add_event::<ConsecrationInject>();
        // 故意不 insert WorldQiAccount 资源
        app.insert_resource(ZoneRegistry::fallback());
        app.add_systems(Update, handle_consecration_injects);

        let station = app
            .world_mut()
            .spawn(WeaponForgeStation::placed(
                BlockPos::new(8, 66, 8),
                1,
                valence::prelude::Entity::PLACEHOLDER,
            ))
            .id();

        let initial_qi = 25.0_f64;
        let caster = app
            .world_mut()
            .spawn(Cultivation {
                qi_current: initial_qi,
                qi_max: 100.0,
                ..Default::default()
            })
            .id();

        let session_id = ForgeSessionId(99);
        insert_session_with_caster(&mut app, session_id, station, caster);

        app.world_mut().send_event(ConsecrationInject {
            session: session_id,
            qi_amount: 10.0,
        });
        app.update();

        let qi_after = app
            .world()
            .get::<Cultivation>(caster)
            .expect("caster Cultivation 组件应仍存在")
            .qi_current;
        assert!(
            (qi_after - initial_qi).abs() < 1e-9,
            "期望 qi_current 不变（无 WorldQiAccount 时跳过注入），\
             实际 before={initial_qi} after={qi_after}（真元被静默扣除！守恒漏洞）"
        );

        let sessions = app.world().resource::<ForgeSessions>();
        let qi_injected = match &sessions.get(session_id).unwrap().step_state {
            StepState::Consecration(s) => s.qi_injected,
            _ => panic!("session 应仍处于 Consecration 步骤"),
        };
        assert!(
            qi_injected.abs() < 1e-9,
            "期望 qi_injected=0（注入已跳过），实际={qi_injected}"
        );
    }

    #[test]
    fn hammer_step_emits_vfx() {
        let mut app = App::new();
        let minerals = build_default_mineral_registry();
        let registry =
            BlueprintRegistry::load_dir_with_minerals(DEFAULT_BLUEPRINTS_DIR, Some(&minerals))
                .expect("default forge blueprints should load");
        let blueprint_id = registry
            .ids()
            .find(|id| {
                registry
                    .get(id.as_str())
                    .is_some_and(|blueprint| blueprint.has_step(StepKind::Tempering))
            })
            .expect("default blueprints should include tempering")
            .clone();
        let step_index = registry
            .get(blueprint_id.as_str())
            .unwrap()
            .steps
            .iter()
            .position(|step| step.kind() == StepKind::Tempering)
            .expect("tempering step");
        app.insert_resource(registry);
        app.add_event::<TemperingHit>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, handle_tempering_hits);

        let station = app
            .world_mut()
            .spawn(WeaponForgeStation::placed(
                BlockPos::new(8, 66, 8),
                1,
                valence::prelude::Entity::PLACEHOLDER,
            ))
            .id();
        let caster = app
            .world_mut()
            .spawn((Cultivation::default(), SkillSet::default()))
            .id();
        let session_id = ForgeSessionId(9);
        let mut sessions = ForgeSessions::new();
        let mut session = ForgeSession::new(session_id, blueprint_id, station, caster);
        session.current_step = ForgeStep::Tempering;
        session.step_index = step_index;
        session.step_state = StepState::Tempering(Default::default());
        sessions.insert(session);
        app.insert_resource(sessions);

        app.world_mut().send_event(TemperingHit {
            session: session_id,
            beat: TemperBeat::Light,
            ticks_remaining: 4,
        });
        app.update();

        let events = app.world().resource::<Events<VfxEventRequest>>();
        let emitted = events
            .iter_current_update_events()
            .next()
            .expect("tempering hit should emit vfx");
        match &emitted.payload {
            crate::schema::vfx_event::VfxEventPayloadV1::SpawnParticle { event_id, .. } => {
                assert_eq!(event_id, gameplay_vfx::FORGE_HAMMER_STRIKE);
            }
            other => panic!("expected SpawnParticle, got {other:?}"),
        }
    }

    #[test]
    fn forge_station_tier_name_matches_chat_templates() {
        assert_eq!(forge_station_tier_name(1), "凡铁炉");
        assert_eq!(forge_station_tier_name(2), "灵铁炉");
        assert_eq!(forge_station_tier_name(3), "稀铁炉");
        assert_eq!(forge_station_tier_name(4), "道炉");
    }
}
