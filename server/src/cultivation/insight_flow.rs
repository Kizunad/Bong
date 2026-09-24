//! 顿悟流水线（plan §5.4 / §5.5）— 触发点 → Offer → Chosen → Apply 全闭环。
//!
//! Agent LLM 尚未接入时，本模块使用 `insight_fallback::fallback_for` 作为 offer
//! 来源，对外仍以 `InsightRequest` / `InsightOffer` / `InsightChosen` 事件契约暴露。
//! 当 agent runtime 就绪后，只需把 `process_insight_request` 替换为读 agent 通道，
//! 触发点与 Apply 子系统可完全复用。

use valence::prelude::{
    bevy_ecs, Commands, Component, Entity, EventReader, EventWriter, Position, Query, Res, UniqueId,
};

use crate::network::vfx_event_emit::VfxEventRequest;
use crate::schema::vfx_event::VfxEventPayloadV1;

use super::breakthrough::{BreakthroughError, BreakthroughOutcome};
use super::color::PracticeLog;
use super::components::{Cultivation, MeridianSystem, QiColor, Realm};
use super::forging::{ForgeAxis, ForgeOutcome, P1_MAX_TIER};
use super::insight::{
    validate_offer, InsightChoice, InsightChosen, InsightEffect, InsightOffer, InsightQuota,
    InsightRequest,
};
use super::insight_apply::{apply_choice, InsightModifiers, UnlockedPerceptions};
use super::insight_fallback::fallback_for_context;
use super::life_record::LifeRecord;
use super::lifespan::{LifespanComponent, LifespanExtensionIntent};
use super::tick::CultivationClock;

/// 服务器缓存的顿悟 offer（component 形式挂在玩家实体上）。
///
/// 由 `process_insight_request` 填入，由 `apply_insight_chosen` 消费并移除。
#[derive(Debug, Clone, Component)]
pub struct PendingInsightOffer {
    pub trigger_id: String,
    pub choices: Vec<InsightChoice>,
}

/// Agent offer 落地时可用的玩家上下文。
pub type InsightOfferContext<'a> = (&'a QiColor, &'a PracticeLog, &'a InsightQuota, Realm);

/// 突破成功/失败 → InsightRequest。
///
/// * 成功：首次抵达某新境界 → `first_breakthrough_to_<Realm>`（InsightQuota
///   跟踪 `fired_triggers` 防重复）
/// * 失败（RolledFailure 且 severity < 0.5）→ `breakthrough_failed_recovered`
pub fn insight_trigger_on_breakthrough(
    mut outcomes: EventReader<BreakthroughOutcome>,
    mut requests: EventWriter<InsightRequest>,
    mut players: Query<(&Cultivation, &mut InsightQuota)>,
    positions: Query<&Position>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for ev in outcomes.read() {
        let Ok((cultivation, mut quota)) = players.get_mut(ev.entity) else {
            continue;
        };
        match &ev.result {
            Ok(success) => {
                let trigger = format!("first_breakthrough_to_{}", realm_tag(success.to));
                if !quota.fired_triggers.iter().any(|t| t == &trigger) {
                    quota.fired_triggers.push(trigger.clone());
                    requests.send(InsightRequest {
                        entity: ev.entity,
                        trigger_id: trigger,
                        realm: cultivation.realm,
                    });
                    // plan-particle-system-v1 §4.4：首次达成新境界 → enlightenment_aura 顿悟光辉。
                    if let Ok(pos) = positions.get(ev.entity) {
                        let p = pos.get();
                        vfx_events.send(VfxEventRequest::new(
                            p,
                            VfxEventPayloadV1::SpawnParticle {
                                event_id: "bong:enlightenment_aura".to_string(),
                                origin: [p.x, p.y, p.z],
                                direction: None,
                                color: Some("#FFE8B0".to_string()),
                                strength: Some(0.9),
                                count: Some(24),
                                duration_ticks: Some(50),
                            },
                        ));
                    }
                }
            }
            Err(BreakthroughError::RolledFailure { severity }) if *severity < 0.5 => {
                requests.send(InsightRequest {
                    entity: ev.entity,
                    trigger_id: "breakthrough_failed_recovered".to_string(),
                    realm: cultivation.realm,
                });
            }
            Err(_) => {}
        }
    }
}

/// 锻造达 tier 里程碑（P1: tier == P1_MAX_TIER） → `meridian_forge_tier_milestone`。
pub fn insight_trigger_on_forge(
    mut outcomes: EventReader<ForgeOutcome>,
    mut requests: EventWriter<InsightRequest>,
    mut players: Query<(&Cultivation, &mut InsightQuota)>,
) {
    for ev in outcomes.read() {
        let Ok((cultivation, mut quota)) = players.get_mut(ev.entity) else {
            continue;
        };
        let Ok(tier) = &ev.result else { continue };
        if *tier != P1_MAX_TIER {
            continue;
        }
        let axis_tag = match ev.axis {
            ForgeAxis::Rate => "rate",
            ForgeAxis::Capacity => "cap",
        };
        // 同经脉同轴只触发一次
        let fired_key = format!("forge_milestone:{:?}:{axis_tag}", ev.meridian);
        if quota.fired_triggers.iter().any(|t| t == &fired_key) {
            continue;
        }
        quota.fired_triggers.push(fired_key);
        requests.send(InsightRequest {
            entity: ev.entity,
            trigger_id: "meridian_forge_tier_milestone".to_string(),
            realm: cultivation.realm,
        });
    }
}

pub fn insight_trigger_on_wind_candle(
    mut requests: EventWriter<InsightRequest>,
    mut players: Query<(Entity, &Cultivation, &LifespanComponent, &mut InsightQuota)>,
) {
    let trigger = "wind_candle_lifespan_extension";
    for (entity, cultivation, lifespan, mut quota) in &mut players {
        if !lifespan.is_wind_candle()
            || !quota.has_quota(cultivation.realm)
            || quota.fired_triggers.iter().any(|seen| seen == trigger)
        {
            continue;
        }
        quota.fired_triggers.push(trigger.to_string());
        requests.send(InsightRequest {
            entity,
            trigger_id: trigger.to_string(),
            realm: cultivation.realm,
        });
    }
}

/// Agent 端经 Redis 下发的 offer → 服务器 PendingInsightOffer 的桥。
///
/// 当前 DTO (`InsightChoiceV1`) 仅携带 `effect_kind` + `magnitude`，而服务器
/// `InsightEffect` 变体大多还需要 `id` / `color` / `material` 等上下文，无法
/// 从 DTO 无损重建。本函数采取务实策略：**用当前玩家上下文驱动的
/// `fallback_for_context` 作为实际效果来源**，agent 的产出仅作日志便于后续调试
/// prompt 质量。待 schema 扩充 `effect_params` 后，可在此处真正解析 agent 决策
/// 并落地。
pub fn ingest_agent_insight_offer(
    trigger_id: &str,
    agent_choices: &[crate::schema::cultivation::InsightChoiceV1],
    context: Option<InsightOfferContext<'_>>,
) -> Option<Vec<InsightChoice>> {
    let has_context = context.is_some();
    let (fallback_color, fallback_log, fallback_quota);
    let (qi_color, practice_log, quota, realm) = match context {
        Some(context) => context,
        None => {
            fallback_color = QiColor::default();
            fallback_log = PracticeLog::default();
            fallback_quota = InsightQuota::default();
            (
                &fallback_color,
                &fallback_log,
                &fallback_quota,
                Realm::Induce,
            )
        }
    };
    let fallback = fallback_for_context(trigger_id, qi_color, practice_log, quota, realm);
    if fallback.is_empty() {
        tracing::warn!(
            "[bong][cultivation] agent offer for trigger {:?} has no local fallback; dropping ({} agent choices ignored)",
            trigger_id,
            agent_choices.len()
        );
        return None;
    }
    tracing::debug!(
        "[bong][cultivation] agent offer trigger={:?} agent_choices={} context={} main_color={:?} realm={:?} -> using contextual fallback ({} choices)",
        trigger_id,
        agent_choices.len(),
        if has_context { "entity" } else { "default" },
        qi_color.main,
        realm,
        fallback.len()
    );
    Some(fallback)
}

/// 消费 `InsightRequest` → 读取 fallback 池 → 发 `InsightOffer` + 挂 `PendingInsightOffer` Component。
///
/// agent runtime 接入后，把这里换成从 Redis/agent 通道读取 offer 即可。
pub fn process_insight_request(
    mut commands: Commands,
    mut reqs: EventReader<InsightRequest>,
    mut offers: EventWriter<InsightOffer>,
    players: Query<(&QiColor, &PracticeLog, &InsightQuota)>,
) {
    for req in reqs.read() {
        let (fallback_color, fallback_log, fallback_quota);
        let (qi_color, practice_log, quota) =
            if let Ok((qi_color, practice_log, quota)) = players.get(req.entity) {
                (qi_color, practice_log, quota)
            } else {
                fallback_color = QiColor::default();
                fallback_log = PracticeLog::default();
                fallback_quota = InsightQuota::default();
                (&fallback_color, &fallback_log, &fallback_quota)
            };
        let choices =
            fallback_for_context(&req.trigger_id, qi_color, practice_log, quota, req.realm);
        if choices.is_empty() {
            tracing::warn!(
                "[bong][cultivation] no fallback for trigger {:?}; skipping offer",
                req.trigger_id
            );
            continue;
        }
        let pending = PendingInsightOffer {
            trigger_id: req.trigger_id.clone(),
            choices: choices.clone(),
        };
        if let Some(mut e) = commands.get_entity(req.entity) {
            e.insert(pending);
        }
        offers.send(InsightOffer {
            entity: req.entity,
            trigger_id: req.trigger_id.clone(),
            choices,
        });
    }
}

/// 消费 `InsightChosen` → 查 `PendingInsightOffer` → Arbiter 校验 → `apply_choice` + 记 Quota 累积。
///
/// plan-skill-av-relink-v1 P1：三重校验（pending 对齐 / choice 合法 / arbiter 配额）
/// 全部通过、`apply_choice` 生效后，向抉择者发 `enlightenment_pose` 顿悟姿态动画
/// （`anim_targets` 缺 Position/UniqueId 时静默 skip——离线/测试实体不发）。
/// emit 必须在校验通过分支：提前发会在 stale/无效/被拒抉择上误播。
#[allow(clippy::type_complexity)]
pub fn apply_insight_chosen(
    clock: Res<CultivationClock>,
    mut commands: Commands,
    mut events: EventReader<InsightChosen>,
    mut lifespan_extension_tx: EventWriter<LifespanExtensionIntent>,
    anim_targets: Query<(&Position, &UniqueId)>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    mut players: Query<(
        &PendingInsightOffer,
        &mut Cultivation,
        &mut MeridianSystem,
        &mut QiColor,
        &mut PracticeLog,
        &mut UnlockedPerceptions,
        &mut InsightModifiers,
        &mut LifeRecord,
        &mut InsightQuota,
    )>,
) {
    let now = clock.tick;
    for ev in events.read() {
        let Ok((
            pending,
            mut cultivation,
            mut meridians,
            mut qi_color,
            mut practice_log,
            mut perc,
            mut mods,
            mut life,
            mut quota,
        )) = players.get_mut(ev.entity)
        else {
            continue;
        };
        // stale/malformed client decision 校验：客户端回传的 trigger_id 必须与当前挂着的
        // PendingInsightOffer 对齐。否则说明 offer 已被置换（例如又触发了新 offer），
        // 直接丢弃以免把旧选择应用到新 offer 上。
        if ev.trigger_id != pending.trigger_id {
            tracing::warn!(
                "[bong][cultivation] {:?} insight decision mismatch: client sent {:?} but pending is {:?}; ignoring",
                ev.entity,
                ev.trigger_id,
                pending.trigger_id
            );
            continue;
        }
        let Some(idx) = ev.choice_idx else {
            tracing::info!(
                "[bong][cultivation] {:?} rejected insight offer {:?}",
                ev.entity,
                pending.trigger_id
            );
            if let Some(mut e) = commands.get_entity(ev.entity) {
                e.remove::<PendingInsightOffer>();
            }
            continue;
        };
        let Some(choice) = pending.choices.get(idx) else {
            tracing::warn!(
                "[bong][cultivation] {:?} chose invalid idx {idx} for offer {:?}",
                ev.entity,
                pending.trigger_id
            );
            if let Some(mut e) = commands.get_entity(ev.entity) {
                e.remove::<PendingInsightOffer>();
            }
            continue;
        };

        if let Err(err) = validate_offer(&quota, choice, cultivation.realm) {
            tracing::warn!(
                "[bong][cultivation] {:?} insight {:?} rejected by arbiter: {err:?}",
                ev.entity,
                pending.trigger_id
            );
            if let Some(mut e) = commands.get_entity(ev.entity) {
                e.remove::<PendingInsightOffer>();
            }
            continue;
        }

        apply_choice(
            choice,
            &mut cultivation,
            &mut meridians,
            &mut qi_color,
            Some(&mut practice_log),
            &mut perc,
            &mut mods,
            &mut life,
            &pending.trigger_id,
            now,
        );
        if matches!(choice.effect, InsightEffect::LifespanExtensionEnlightenment) {
            lifespan_extension_tx.send(LifespanExtensionIntent {
                entity: ev.entity,
                requested_years: 0,
                source: "enlightenment_extension".to_string(),
            });
        }
        // plan-skill-av-relink-v1 P1 — 顿悟抉择被接受并生效 → enlightenment_pose。
        if let Ok((position, unique_id)) = anim_targets.get(ev.entity) {
            let origin = position.get();
            vfx_events.send(VfxEventRequest::new(
                origin,
                VfxEventPayloadV1::PlayAnim {
                    target_player: unique_id.0.to_string(),
                    anim_id: crate::network::vfx_animation_trigger::ANIM_ENLIGHTENMENT_POSE
                        .to_string(),
                    priority: crate::network::vfx_animation_trigger::STORY_PRIORITY,
                    fade_in_ticks: Some(3),
                },
            ));
        }
        quota.apply_accumulation(choice);

        if let Some(mut e) = commands.get_entity(ev.entity) {
            e.remove::<PendingInsightOffer>();
        }
    }
}

fn realm_tag(r: Realm) -> &'static str {
    match r {
        Realm::Awaken => "Awaken",
        Realm::Induce => "Induce",
        Realm::Condense => "Condense",
        Realm::Solidify => "Solidify",
        Realm::Spirit => "Spirit",
        Realm::Void => "Void",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::ColorKind;
    use crate::cultivation::insight::{InsightAlignment, InsightEffect};
    use crate::cultivation::insight_fallback::fallback_for;
    use crate::schema::cultivation::InsightChoiceV1;

    fn agent_choices() -> Vec<InsightChoiceV1> {
        vec![
            InsightChoiceV1 {
                category: "qi".to_string(),
                effect_kind: "qi_regen_factor".to_string(),
                magnitude: 0.1,
                flavor_text: "agent choice should not decide local effect params yet".to_string(),
                narrator_voice: None,
                alignment: Some("converge".to_string()),
                cost_kind: Some("opposite_color_penalty".to_string()),
                cost_magnitude: Some(0.05),
                cost_flavor: Some("agent cost".to_string()),
            },
            InsightChoiceV1 {
                category: "composure".to_string(),
                effect_kind: "composure_recover".to_string(),
                magnitude: 0.1,
                flavor_text: "agent neutral".to_string(),
                narrator_voice: None,
                alignment: Some("neutral".to_string()),
                cost_kind: Some("shock_sensitivity".to_string()),
                cost_magnitude: Some(0.03),
                cost_flavor: Some("agent cost".to_string()),
            },
            InsightChoiceV1 {
                category: "color".to_string(),
                effect_kind: "color_cap_add".to_string(),
                magnitude: 0.04,
                flavor_text: "agent diverge".to_string(),
                narrator_voice: None,
                alignment: Some("diverge".to_string()),
                cost_kind: Some("main_color_penalty".to_string()),
                cost_magnitude: Some(0.1),
                cost_flavor: Some("agent cost".to_string()),
            },
        ]
    }

    #[test]
    fn realm_tag_is_stable() {
        assert_eq!(realm_tag(Realm::Induce), "Induce");
        assert_eq!(realm_tag(Realm::Void), "Void");
    }

    #[test]
    fn fallback_for_first_induce_nonempty() {
        let v = fallback_for("first_breakthrough_to_Induce");
        assert!(!v.is_empty());
    }

    #[test]
    fn agent_offer_uses_same_contextual_fallback_as_local_request() {
        let qi = QiColor {
            main: ColorKind::Sharp,
            ..QiColor::default()
        };
        let mut log = PracticeLog::default();
        log.add(ColorKind::Sharp, 10.0);
        let quota = InsightQuota::default();
        let expected = fallback_for_context(
            "first_breakthrough_to_Induce",
            &qi,
            &log,
            &quota,
            Realm::Induce,
        );

        let actual = ingest_agent_insight_offer(
            "first_breakthrough_to_Induce",
            &agent_choices(),
            Some((&qi, &log, &quota, Realm::Induce)),
        )
        .expect("known trigger should map to contextual fallback");

        assert_eq!(
            actual
                .iter()
                .map(|choice| (choice.alignment, choice.target_color, choice.flavor.clone()))
                .collect::<Vec<_>>(),
            expected
                .iter()
                .map(|choice| (choice.alignment, choice.target_color, choice.flavor.clone()))
                .collect::<Vec<_>>(),
            "agent-fed fallback 必须与本地 InsightRequest 在同一上下文下的三轨语义一致"
        );
    }

    #[test]
    fn agent_offer_for_sharp_context_does_not_degrade_to_default_mellow() {
        let qi = QiColor {
            main: ColorKind::Sharp,
            ..QiColor::default()
        };
        let quota = InsightQuota::default();

        let choices = ingest_agent_insight_offer(
            "first_breakthrough_to_Induce",
            &agent_choices(),
            Some((&qi, &PracticeLog::default(), &quota, Realm::Induce)),
        )
        .expect("known trigger should map to contextual fallback");

        assert!(
            choices.iter().any(|choice| choice.flavor.contains("锋锐")),
            "Sharp 主色的 agent-fed offer 不应退化成默认 Mellow 文案，实际 choices={choices:?}"
        );
        assert!(
            !choices.iter().all(|choice| choice.flavor.contains("醇")),
            "agent-fed offer 不应全部呈现默认醇色模板，实际 choices={choices:?}"
        );
    }

    #[test]
    fn agent_offer_for_hunyuan_context_keeps_hunyuan_specific_choice() {
        let qi = QiColor {
            is_hunyuan: true,
            ..QiColor::default()
        };
        let mut log = PracticeLog::default();
        log.add(ColorKind::Turbid, 40.0);
        let quota = InsightQuota::default();

        let choices = ingest_agent_insight_offer(
            "chaotic_to_hunyuan_pivot",
            &agent_choices(),
            Some((&qi, &log, &quota, Realm::Induce)),
        )
        .expect("known trigger should map to contextual fallback");

        assert!(
            choices
                .iter()
                .any(|choice| matches!(choice.effect, InsightEffect::HunyuanThreshold { .. })),
            "混元上下文必须保留混元专属选项，不能退回默认 Mellow 三轨，实际 choices={choices:?}"
        );
        assert!(
            choices.iter().any(|choice| {
                choice.alignment == InsightAlignment::Diverge
                    && choice.target_color == Some(ColorKind::Turbid)
            }),
            "混元 diverge 槽应根据 PracticeLog 选出最强色 Turbid，实际 choices={choices:?}"
        );
    }

    #[test]
    fn agent_offer_for_chaotic_context_keeps_chaotic_specific_choice() {
        let qi = QiColor {
            main: ColorKind::Violent,
            is_chaotic: true,
            ..QiColor::default()
        };
        let mut log = PracticeLog::default();
        log.add(ColorKind::Violent, 30.0);
        let quota = InsightQuota::default();

        let choices = ingest_agent_insight_offer(
            "first_breakthrough_to_Induce",
            &agent_choices(),
            Some((&qi, &log, &quota, Realm::Induce)),
        )
        .expect("known trigger should map to contextual fallback");

        assert!(
            choices
                .iter()
                .any(|choice| matches!(choice.effect, InsightEffect::ChaoticTolerance { .. })),
            "杂色上下文必须保留杂色专属收束选项，不能退回默认 Mellow 三轨，实际 choices={choices:?}"
        );
        assert!(
            choices.iter().any(|choice| {
                choice.alignment == InsightAlignment::Diverge
                    && choice.target_color == Some(ColorKind::Violent)
            }),
            "杂色 diverge 槽应根据 PracticeLog 选出最高权重主线 Violent，实际 choices={choices:?}"
        );
    }

    // ── plan-skill-av-relink-v1 P3 —— enlightenment_pose 内联 emit pin ───────────

    use valence::prelude::{App, Events, Update};

    const TEST_TRIGGER_ID: &str = "first_breakthrough_to_Induce";

    fn setup_apply_chosen_app() -> App {
        let mut app = App::new();
        app.insert_resource(CultivationClock::default());
        app.add_event::<InsightChosen>();
        app.add_event::<LifespanExtensionIntent>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, apply_insight_chosen);
        app
    }

    /// 挂着有效 PendingInsightOffer 的抉择者；`with_anim_target=false` 模拟离线/
    /// 已清理实体（缺 Position/UniqueId）。
    fn spawn_choosing_player(app: &mut App, with_anim_target: bool, quota: InsightQuota) -> Entity {
        let choices = fallback_for(TEST_TRIGGER_ID);
        assert!(
            !choices.is_empty(),
            "前置条件破坏：fallback offer 必须非空（本测试要走真实抉择路径）"
        );
        let mut entity = app.world_mut().spawn((
            PendingInsightOffer {
                trigger_id: TEST_TRIGGER_ID.to_string(),
                choices,
            },
            Cultivation {
                realm: Realm::Induce,
                ..Default::default()
            },
            MeridianSystem::default(),
            QiColor::default(),
            PracticeLog::default(),
            UnlockedPerceptions::default(),
            InsightModifiers::default(),
            LifeRecord::default(),
            quota,
        ));
        if with_anim_target {
            entity.insert((Position::new([0.0, 64.0, 0.0]), UniqueId::default()));
        }
        entity.id()
    }

    fn drain_enlightenment_anims(app: &mut App) -> Vec<(String, u16)> {
        app.world_mut()
            .resource_mut::<Events<VfxEventRequest>>()
            .drain()
            .filter_map(|request| match request.payload {
                VfxEventPayloadV1::PlayAnim {
                    target_player,
                    anim_id,
                    priority,
                    ..
                } if anim_id == crate::network::vfx_animation_trigger::ANIM_ENLIGHTENMENT_POSE => {
                    Some((target_player, priority))
                }
                _ => None,
            })
            .collect()
    }

    fn send_chosen(app: &mut App, entity: Entity, trigger_id: &str, choice_idx: Option<usize>) {
        app.world_mut().send_event(InsightChosen {
            entity,
            trigger_id: trigger_id.to_string(),
            choice_idx,
        });
    }

    /// happy path：三重校验通过、抉择生效 → 恰发一条 enlightenment_pose 顿悟姿态，
    /// target = 抉择者本人 uuid、优先级叙事档；offer 消费移除、quota 记账。
    #[test]
    fn accepted_insight_choice_emits_enlightenment_pose() {
        let mut app = setup_apply_chosen_app();
        let player = spawn_choosing_player(&mut app, true, InsightQuota::default());
        let player_uuid = app.world().get::<UniqueId>(player).unwrap().0.to_string();

        send_chosen(&mut app, player, TEST_TRIGGER_ID, Some(0));
        app.update();

        let anims = drain_enlightenment_anims(&mut app);
        assert_eq!(
            anims.len(),
            1,
            "顿悟抉择生效应恰发一条 enlightenment_pose，实际 {anims:?}"
        );
        assert_eq!(
            anims[0].0, player_uuid,
            "enlightenment_pose 应发给抉择者本人（target_player = 抉择者 uuid）"
        );
        assert_eq!(
            anims[0].1,
            crate::network::vfx_animation_trigger::STORY_PRIORITY,
            "enlightenment_pose 优先级应为叙事档"
        );
        assert!(
            app.world().get::<PendingInsightOffer>(player).is_none(),
            "生效后 PendingInsightOffer 应被消费移除"
        );
        assert_eq!(
            app.world()
                .get::<InsightQuota>(player)
                .unwrap()
                .used_this_realm,
            1,
            "生效后 quota 应记账一次（证明动画确实发在 apply 生效路径上）"
        );
    }

    /// 错误分支：stale trigger_id（offer 已被置换）→ 抉择被丢弃不发姿态。
    #[test]
    fn stale_trigger_id_mismatch_does_not_emit_pose() {
        let mut app = setup_apply_chosen_app();
        let player = spawn_choosing_player(&mut app, true, InsightQuota::default());

        send_chosen(&mut app, player, "some_replaced_trigger", Some(0));
        app.update();

        assert!(
            drain_enlightenment_anims(&mut app).is_empty(),
            "stale trigger_id 的抉择被丢弃时不应发 enlightenment_pose"
        );
        assert!(
            app.world().get::<PendingInsightOffer>(player).is_some(),
            "mismatch 分支不应消费当前 offer"
        );
    }

    /// 错误分支：玩家拒绝 offer（choice_idx=None）→ 无顿悟不发姿态。
    #[test]
    fn rejected_offer_does_not_emit_pose() {
        let mut app = setup_apply_chosen_app();
        let player = spawn_choosing_player(&mut app, true, InsightQuota::default());

        send_chosen(&mut app, player, TEST_TRIGGER_ID, None);
        app.update();

        assert!(
            drain_enlightenment_anims(&mut app).is_empty(),
            "拒绝 offer 时不应发 enlightenment_pose"
        );
        assert!(
            app.world().get::<PendingInsightOffer>(player).is_none(),
            "拒绝后 offer 应被移除"
        );
    }

    /// 错误分支：非法 choice_idx（越界）→ 抉择无效不发姿态。
    #[test]
    fn invalid_choice_idx_does_not_emit_pose() {
        let mut app = setup_apply_chosen_app();
        let player = spawn_choosing_player(&mut app, true, InsightQuota::default());

        send_chosen(&mut app, player, TEST_TRIGGER_ID, Some(99));
        app.update();

        assert!(
            drain_enlightenment_anims(&mut app).is_empty(),
            "越界 choice_idx 的抉择无效时不应发 enlightenment_pose"
        );
    }

    /// 错误分支：arbiter 配额耗尽（validate_offer 拒绝）→ 抉择被拒不发姿态。
    #[test]
    fn arbiter_rejected_choice_does_not_emit_pose() {
        use crate::cultivation::insight::realm_quota;
        let mut app = setup_apply_chosen_app();
        let exhausted = InsightQuota {
            used_this_realm: realm_quota(Realm::Induce),
            ..Default::default()
        };
        let player = spawn_choosing_player(&mut app, true, exhausted);

        send_chosen(&mut app, player, TEST_TRIGGER_ID, Some(0));
        app.update();

        assert!(
            drain_enlightenment_anims(&mut app).is_empty(),
            "arbiter 配额耗尽拒绝抉择时不应发 enlightenment_pose"
        );
    }

    /// 状态转换分支：抉择者缺 Position/UniqueId（离线/测试实体）→ 效果照常生效、
    /// 动画静默 skip（不因缺渲染目标阻断顿悟本体）。
    #[test]
    fn accepted_choice_without_anim_target_applies_but_does_not_emit() {
        let mut app = setup_apply_chosen_app();
        let player = spawn_choosing_player(&mut app, false, InsightQuota::default());

        send_chosen(&mut app, player, TEST_TRIGGER_ID, Some(0));
        app.update();

        assert!(
            drain_enlightenment_anims(&mut app).is_empty(),
            "缺 Position/UniqueId 的抉择者不应发 enlightenment_pose"
        );
        assert_eq!(
            app.world()
                .get::<InsightQuota>(player)
                .unwrap()
                .used_this_realm,
            1,
            "动画 skip 不得阻断顿悟本体：quota 仍应记账"
        );
        assert!(
            app.world().get::<PendingInsightOffer>(player).is_none(),
            "动画 skip 不得阻断顿悟本体：offer 仍应被消费移除"
        );
    }

    /// 重复触发幂等：生效后 offer 已移除，客户端重放同一抉择不再生效、不再发姿态。
    #[test]
    fn replayed_decision_after_success_does_not_emit_again() {
        let mut app = setup_apply_chosen_app();
        let player = spawn_choosing_player(&mut app, true, InsightQuota::default());

        send_chosen(&mut app, player, TEST_TRIGGER_ID, Some(0));
        app.update();
        assert_eq!(
            drain_enlightenment_anims(&mut app).len(),
            1,
            "前置条件破坏：首次抉择应生效并发姿态"
        );

        send_chosen(&mut app, player, TEST_TRIGGER_ID, Some(0));
        app.update();

        assert!(
            drain_enlightenment_anims(&mut app).is_empty(),
            "offer 已消费后的重放抉择不应再发 enlightenment_pose"
        );
        assert_eq!(
            app.world()
                .get::<InsightQuota>(player)
                .unwrap()
                .used_this_realm,
            1,
            "重放不得二次记账（幂等）"
        );
    }
}
