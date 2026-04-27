//! 定期向 Fabric 客户端推送 `cultivation_detail` CustomPayload（`bong:server_data`）。
//!
//! payload 使用 SoA(parallel arrays) 布局，顺序为
//! `MeridianId::REGULAR` (0..12) 紧接 `MeridianId::EXTRAORDINARY` (12..20)，
//! 与 `MeridianId` 判别式一致；客户端解析时按索引还原。
//!
//! 节流：每 20 tick 最多发一次（~1s @ 20TPS）。

use valence::prelude::{bevy_ecs, Client, Entity, Position, Query, Res, ResMut, Resource, With};

use crate::cultivation::components::{Contamination, Cultivation, MeridianSystem};
use crate::cultivation::life_record::LifeRecord;
use crate::cultivation::lifespan::{
    lifespan_tick_rate_multiplier, LifespanCapTable, LifespanComponent,
};
use crate::cultivation::tick::CultivationClock;
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::schema::cultivation::SkillMilestoneSnapshotV1;
use crate::schema::server_data::{LifespanPreviewV1, ServerDataPayloadV1, ServerDataV1};
use crate::world::zone::ZoneRegistry;

const EMIT_INTERVAL_TICKS: u64 = 20;

#[derive(Default, Resource)]
pub struct CultivationDetailEmitState {
    last_emit_tick: u64,
}

type CultivationDetailEmitQueryItem<'a> = (
    Entity,
    &'a mut Client,
    &'a MeridianSystem,
    &'a Cultivation,
    Option<&'a Contamination>,
    Option<&'a LifespanComponent>,
    Option<&'a Position>,
    Option<&'a LifeRecord>,
);

pub fn emit_cultivation_detail_payloads(
    clock: Res<CultivationClock>,
    zones: Option<Res<ZoneRegistry>>,
    mut state: ResMut<CultivationDetailEmitState>,
    mut clients: Query<CultivationDetailEmitQueryItem<'_>, With<Client>>,
) {
    if clock.tick.saturating_sub(state.last_emit_tick) < EMIT_INTERVAL_TICKS {
        return;
    }
    state.last_emit_tick = clock.tick;

    let zones = zones.as_deref();

    for (
        entity,
        mut client,
        meridians,
        cultivation,
        contamination,
        lifespan,
        position,
        life_record,
    ) in &mut clients
    {
        let mut opened = Vec::with_capacity(20);
        let mut flow_rate = Vec::with_capacity(20);
        let mut flow_capacity = Vec::with_capacity(20);
        let mut integrity = Vec::with_capacity(20);
        let mut open_progress = Vec::with_capacity(20);
        let mut cracks_count = Vec::with_capacity(20);
        for m in meridians
            .regular
            .iter()
            .chain(meridians.extraordinary.iter())
        {
            opened.push(m.opened);
            flow_rate.push(m.flow_rate);
            flow_capacity.push(m.flow_capacity);
            integrity.push(m.integrity);
            open_progress.push(if m.opened { 1.0 } else { m.open_progress });
            cracks_count.push(u8::try_from(m.cracks.len()).unwrap_or(u8::MAX));
        }

        let contamination_total = contamination
            .map(|c| c.entries.iter().map(|e| e.amount).sum::<f64>())
            .unwrap_or(0.0);
        let lifespan = lifespan.map(|lifespan| LifespanPreviewV1 {
            years_lived: lifespan.years_lived,
            cap_by_realm: lifespan.cap_by_realm,
            remaining_years: lifespan.remaining_years(),
            death_penalty_years: LifespanCapTable::death_penalty_years_for_cap(
                lifespan.cap_by_realm,
            ),
            tick_rate_multiplier: lifespan_tick_rate_multiplier(position, zones),
            is_wind_candle: lifespan.is_wind_candle(),
        });
        let recent_skill_milestones_summary = life_record
            .map(|life| life.recent_skill_milestones_summary_text(3))
            .unwrap_or_default();
        let skill_milestones = life_record
            .map(|life| {
                let len = life.skill_milestones.len();
                let start = len.saturating_sub(6);
                life.skill_milestones[start..]
                    .iter()
                    .map(SkillMilestoneSnapshotV1::from_runtime)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let payload = ServerDataV1::new(ServerDataPayloadV1::CultivationDetail {
            realm: format!("{:?}", cultivation.realm),
            opened,
            flow_rate,
            flow_capacity,
            integrity,
            open_progress,
            cracks_count,
            contamination_total,
            lifespan,
            recent_skill_milestones_summary,
            skill_milestones,
        });
        let label = payload_type_label(payload.payload_type());
        let bytes = match serialize_server_data_payload(&payload) {
            Ok(b) => b,
            Err(err) => {
                tracing::warn!(
                    "[bong][network] failed to serialize {label} for {entity:?}: {err:?}"
                );
                continue;
            }
        };

        use valence::ident;
        let _ = SERVER_DATA_CHANNEL; // channel constant, matches ident! literal below
        client.send_custom_payload(ident!("bong:server_data"), &bytes);
    }
}
