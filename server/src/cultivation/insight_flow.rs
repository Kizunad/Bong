//! 顿悟流水线（plan §5.4 / §5.5）— 触发点 → Offer → Chosen → Apply 全闭环。
//!
//! Agent LLM 尚未接入时，本模块使用 `insight_fallback::fallback_for` 作为 offer
//! 来源，对外仍以 `InsightRequest` / `InsightOffer` / `InsightChosen` 事件契约暴露。
//! 当 agent runtime 就绪后，只需把 `process_insight_request` 替换为读 agent 通道，
//! 触发点与 Apply 子系统可完全复用。

use valence::prelude::{
    bevy_ecs, Commands, Component, Entity, EventReader, EventWriter, Position, Query, Res,
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
use super::insight_fallback::{fallback_for, fallback_for_context};
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
/// 从 DTO 无损重建。本函数采取务实策略：**用 `fallback_for(trigger_id)` 作为
/// 实际效果来源**，agent 的产出仅作日志便于后续调试 prompt 质量。待 schema
/// 扩充 `effect_params` 后，可在此处真正解析 agent 决策并落地。
pub fn ingest_agent_insight_offer(
    trigger_id: &str,
    agent_choices: &[crate::schema::cultivation::InsightChoiceV1],
) -> Option<Vec<InsightChoice>> {
    let fallback = fallback_for(trigger_id);
    if fallback.is_empty() {
        tracing::warn!(
            "[bong][cultivation] agent offer for trigger {:?} has no local fallback; dropping ({} agent choices ignored)",
            trigger_id,
            agent_choices.len()
        );
        return None;
    }
    tracing::debug!(
        "[bong][cultivation] agent offer trigger={:?} agent_choices={} -> using fallback ({} choices)",
        trigger_id,
        agent_choices.len(),
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
#[allow(clippy::type_complexity)]
pub fn apply_insight_chosen(
    clock: Res<CultivationClock>,
    mut commands: Commands,
    mut events: EventReader<InsightChosen>,
    mut lifespan_extension_tx: EventWriter<LifespanExtensionIntent>,
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
}
