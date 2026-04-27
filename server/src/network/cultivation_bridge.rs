//! 修炼事件 → Redis outbound 桥（plan §6.1）。
//!
//! 读取 BreakthroughOutcome / ForgeOutcome / CultivationDeathTrigger Bevy 事件，
//! 构造 V1 DTO 并通过 `RedisBridgeResource.tx_outbound` 推送到对应 channel：
//!   * bong:breakthrough_event
//!   * bong:forge_event
//!   * bong:cultivation_death
//!
//! InsightRequest 由本模块下发 (bong:insight_request)，agent 产出的 InsightOfferV1
//! 经 bong:insight_offer 回传，由 network::process_redis_inbound 解析并挂
//! PendingInsightOffer Component / 发 InsightOffer 事件。

use std::collections::HashMap;

use valence::prelude::{Entity, EventReader, Query, Res, Username};

use super::redis_bridge::RedisOutbound;
use super::RedisBridgeResource;
use crate::cultivation::breakthrough::{BreakthroughError, BreakthroughOutcome};
use crate::cultivation::components::{Cultivation, QiColor};
use crate::cultivation::death_hooks::CultivationDeathTrigger;
use crate::cultivation::forging::{ForgeAxis, ForgeOutcome};
use crate::cultivation::insight::{InsightCategory, InsightQuota, InsightRequest};
use crate::cultivation::life_record::LifeRecord;
use crate::cultivation::lifespan::{AgingEventEmitted, LifespanEventEmitted};
use crate::cultivation::possession::DuoSheEventEmitted;
use crate::schema::cultivation::{
    color_kind_to_string, meridian_id_to_string, realm_to_string, BreakthroughEventV1,
    CultivationDeathV1, ForgeEventV1, InsightRequestV1, QiColorStateV1,
};

pub fn publish_breakthrough_events(
    redis: Res<RedisBridgeResource>,
    mut reader: EventReader<BreakthroughOutcome>,
) {
    for ev in reader.read() {
        let (kind, to_realm, success_rate, severity) = match &ev.result {
            Ok(success) => (
                "Succeeded",
                Some(realm_to_string(success.to).to_string()),
                Some(success.success_rate),
                None,
            ),
            Err(BreakthroughError::RolledFailure { severity }) => {
                ("Failed", None, None, Some(*severity))
            }
            Err(_) => continue,
        };
        let payload = BreakthroughEventV1 {
            kind: kind.to_string(),
            from_realm: realm_to_string(ev.from).to_string(),
            to_realm,
            success_rate,
            severity,
        };
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::BreakthroughEvent(payload));
    }
}

pub fn publish_forge_events(
    redis: Res<RedisBridgeResource>,
    mut reader: EventReader<ForgeOutcome>,
) {
    for ev in reader.read() {
        let axis_s = match ev.axis {
            ForgeAxis::Rate => "Rate",
            ForgeAxis::Capacity => "Capacity",
        };
        let (from_tier, to_tier, success) = match ev.result {
            Ok(new_tier) => (new_tier.saturating_sub(1), new_tier, true),
            Err(_) => (0, 0, false),
        };
        let payload = ForgeEventV1 {
            meridian: meridian_id_to_string(ev.meridian).to_string(),
            axis: axis_s.to_string(),
            from_tier,
            to_tier,
            success,
        };
        let _ = redis.tx_outbound.send(RedisOutbound::ForgeEvent(payload));
    }
}

const INSIGHT_RECENT_BIO_N: usize = 12;

pub fn publish_insight_requests(
    redis: Res<RedisBridgeResource>,
    mut reader: EventReader<InsightRequest>,
    q: Query<(
        Entity,
        &Username,
        &Cultivation,
        &QiColor,
        &LifeRecord,
        &InsightQuota,
    )>,
) {
    for ev in reader.read() {
        let Ok((_, name, cult, color, life, quota)) = q.get(ev.entity) else {
            continue;
        };
        // plan §5.5：把剩余额度上下文显式传给 agent，否则 agent 生成的候选会被
        // applyInsightArbiter() 全量过滤，最终只能走空 offer / fallback。
        let all_cats = [
            InsightCategory::Meridian,
            InsightCategory::Qi,
            InsightCategory::Composure,
            InsightCategory::Coloring,
            InsightCategory::Breakthrough,
            InsightCategory::Style,
            InsightCategory::Perception,
        ];
        let available_categories: Vec<String> = all_cats
            .iter()
            .filter(|cat| {
                let used = quota.cumulative.get(*cat).copied().unwrap_or(0.0);
                used + 1e-9 < (*cat).cumulative_cap()
            })
            .map(|c| format!("{c:?}"))
            .collect();
        let global_caps: HashMap<String, f64> = all_cats
            .iter()
            .map(|c| {
                let used = quota.cumulative.get(c).copied().unwrap_or(0.0);
                let remaining = (c.cumulative_cap() - used).max(0.0);
                (format!("{c:?}"), remaining)
            })
            .collect();
        let payload = InsightRequestV1 {
            trigger_id: ev.trigger_id.clone(),
            character_id: name.0.clone(),
            realm: realm_to_string(ev.realm).to_string(),
            qi_color_state: QiColorStateV1 {
                main: color_kind_to_string(color.main).to_string(),
                secondary: color.secondary.map(|s| color_kind_to_string(s).to_string()),
                is_chaotic: color.is_chaotic,
                is_hunyuan: color.is_hunyuan,
            },
            recent_biography: life
                .recent_summary(INSIGHT_RECENT_BIO_N)
                .iter()
                .map(|e| format!("{e:?}"))
                .collect(),
            composure: cult.composure,
            available_categories,
            global_caps,
        };
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::InsightRequest(payload));
    }
}

pub fn publish_cultivation_death_events(
    redis: Res<RedisBridgeResource>,
    mut reader: EventReader<CultivationDeathTrigger>,
) {
    for ev in reader.read() {
        let payload = CultivationDeathV1 {
            cause: format!("{:?}", ev.cause),
            context: ev.context.clone(),
        };
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::CultivationDeath(payload));
    }
}

pub fn publish_lifespan_events(
    redis: Res<RedisBridgeResource>,
    mut reader: EventReader<LifespanEventEmitted>,
) {
    for ev in reader.read() {
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::LifespanEvent(ev.payload.clone()));
    }
}

pub fn publish_duo_she_events(
    redis: Res<RedisBridgeResource>,
    mut reader: EventReader<DuoSheEventEmitted>,
) {
    for ev in reader.read() {
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::DuoSheEvent(ev.payload.clone()));
    }
}

pub fn publish_aging_events(
    redis: Res<RedisBridgeResource>,
    mut reader: EventReader<AgingEventEmitted>,
) {
    for ev in reader.read() {
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::Aging(ev.payload.clone()));
    }
}
