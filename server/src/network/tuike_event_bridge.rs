//! plan-tuike-v1/v2 — 替尸事件 → Redis 叙事事件。

use valence::prelude::{Entity, EventReader, Query, Res, Username};

use crate::combat::tuike::ShedEvent;
use crate::combat::tuike_v2::events::PermanentTaintAbsorbedEvent;
use crate::combat::tuike_v2::{ContamTransferredEvent, DonFalseSkinEvent, FalseSkinSheddedEvent};
use crate::network::redis_bridge::RedisOutbound;
use crate::network::RedisBridgeResource;
use crate::schema::tuike_v2::{
    FalseSkinTierV1, TuikeSkillEventV1, TuikeSkillIdV1, TuikeSkillVisualContractV1,
};

pub fn publish_tuike_shed_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<ShedEvent>,
) {
    for event in events.read() {
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::TuikeShed(event.payload()));
    }
}

pub fn publish_tuike_v2_skill_events(
    redis: Res<RedisBridgeResource>,
    mut don_events: EventReader<DonFalseSkinEvent>,
    mut shed_events: EventReader<FalseSkinSheddedEvent>,
    mut transfer_events: EventReader<ContamTransferredEvent>,
    mut permanent_taint_events: EventReader<PermanentTaintAbsorbedEvent>,
    usernames: Query<&Username>,
) {
    for event in don_events.read() {
        let payload = base_payload(
            TuikeSkillIdV1::Don,
            event.caster,
            event.tier,
            event.layers_after,
            event.tick,
            &event.visual,
            &usernames,
        );
        send_tuike_v2_payload(&redis, payload);
    }

    for event in shed_events.read() {
        let mut payload = base_payload(
            TuikeSkillIdV1::Shed,
            event.owner,
            event.tier,
            event.layers_after,
            event.tick,
            &event.visual,
            &usernames,
        );
        payload.damage_absorbed = Some(event.damage_absorbed);
        payload.damage_overflow = Some(event.damage_overflow);
        payload.contam_load = Some(event.contam_load);
        payload.permanent_absorbed = event.permanent_taint_load;
        payload.active_shed = Some(event.active);
        send_tuike_v2_payload(&redis, payload);
    }

    for event in transfer_events.read() {
        let mut payload = base_payload(
            TuikeSkillIdV1::TransferTaint,
            event.caster,
            event.tier,
            0,
            event.tick,
            &event.visual,
            &usernames,
        );
        payload.contam_moved_percent = event.contam_moved_percent;
        payload.permanent_absorbed = event.permanent_absorbed;
        payload.qi_cost = event.qi_cost;
        // Transfer events do not carry a post-transfer skin load snapshot. For this
        // narration/HUD event, reuse the moved percent as the compatibility load signal.
        payload.contam_load = Some(event.contam_moved_percent);
        send_tuike_v2_payload(&redis, payload);
    }

    // plan-combat-skill-feedback-bridges-v1 P6 — 永久污浊被皮吸收（守恒叙事：衰败已被皮吸收，真元上限不再缩减）。
    // PermanentQiMaxDecay 已在 skills.rs:232 removed；桥只转发 amount，不触 qi。
    for event in permanent_taint_events.read() {
        let default_visual = crate::combat::tuike_v2::TuikeSkillVisual::for_skill(
            crate::combat::tuike_v2::TuikeSkillId::TransferTaint,
            event.tier == crate::combat::tuike_v2::FalseSkinTier::Ancient,
        );
        let default_visual_payload: crate::combat::tuike_v2::events::TuikeSkillVisualPayload =
            default_visual.into();
        let mut payload = base_payload(
            TuikeSkillIdV1::TransferTaint,
            event.caster,
            event.tier,
            0,
            event.tick,
            &default_visual_payload,
            &usernames,
        );
        // 守恒叙事：衰败已被皮吸收，真元上限不再缩减（PermanentQiMaxDecay already removed at skills.rs:232）
        payload.permanent_absorbed = event.amount;
        send_tuike_v2_payload(&redis, payload);
    }
}

fn send_tuike_v2_payload(redis: &RedisBridgeResource, payload: TuikeSkillEventV1) {
    if let Err(error) = redis
        .tx_outbound
        .send(RedisOutbound::TuikeV2SkillEvent(payload))
    {
        tracing::warn!("[bong][tuike-v2] failed to queue tuike skill event: {error}");
    }
}

fn base_payload(
    skill_id: TuikeSkillIdV1,
    caster: Entity,
    tier: crate::combat::tuike_v2::FalseSkinTier,
    layers_after: u8,
    tick: u64,
    visual: &crate::combat::tuike_v2::events::TuikeSkillVisualPayload,
    usernames: &Query<&Username>,
) -> TuikeSkillEventV1 {
    TuikeSkillEventV1::new(
        entity_wire_id(usernames.get(caster).ok(), caster),
        skill_id,
        tier_payload(tier),
        layers_after,
        tick,
        TuikeSkillVisualContractV1::new(
            visual.animation_id.clone(),
            visual.particle_id.clone(),
            visual.sound_recipe_id.clone(),
            visual.icon_texture.clone(),
        ),
    )
}

fn entity_wire_id(username: Option<&Username>, entity: Entity) -> String {
    username
        .map(|username| format!("offline:{}", username.0))
        .unwrap_or_else(|| format!("char:{}", entity.to_bits()))
}

fn tier_payload(tier: crate::combat::tuike_v2::FalseSkinTier) -> FalseSkinTierV1 {
    match tier {
        crate::combat::tuike_v2::FalseSkinTier::Fan => FalseSkinTierV1::Fan,
        crate::combat::tuike_v2::FalseSkinTier::Light => FalseSkinTierV1::Light,
        crate::combat::tuike_v2::FalseSkinTier::Mid => FalseSkinTierV1::Mid,
        crate::combat::tuike_v2::FalseSkinTier::Heavy => FalseSkinTierV1::Heavy,
        crate::combat::tuike_v2::FalseSkinTier::Ancient => FalseSkinTierV1::Ancient,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::Receiver;
    use valence::prelude::{App, Events, Update};

    fn setup_app() -> (App, Receiver<RedisOutbound>) {
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
        let mut app = App::new();
        app.add_event::<ShedEvent>();
        app.add_event::<DonFalseSkinEvent>();
        app.add_event::<FalseSkinSheddedEvent>();
        app.add_event::<ContamTransferredEvent>();
        app.add_event::<PermanentTaintAbsorbedEvent>();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_systems(Update, publish_tuike_shed_events);
        app.add_systems(Update, publish_tuike_v2_skill_events);
        (app, rx_outbound)
    }

    #[test]
    fn publishes_shed_event_payload() {
        let (mut app, rx) = setup_app();
        let target = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<Events<ShedEvent>>()
            .send(ShedEvent {
                target,
                attacker: None,
                target_id: "offline:Azure".to_string(),
                attacker_id: None,
                kind: crate::combat::tuike::FalseSkinKind::SpiderSilk,
                layers_shed: 1,
                layers_remaining: 0,
                contam_absorbed: 10.0,
                contam_overflow: 2.0,
                tick: 8,
            });

        app.update();

        let outbound = rx.try_recv().expect("expected outbound shed event");
        let RedisOutbound::TuikeShed(payload) = outbound else {
            panic!("expected TuikeShed, got {outbound:?}");
        };
        assert_eq!(payload.target_id, "offline:Azure");
        assert_eq!(payload.layers_shed, 1);
        assert_eq!(payload.contam_overflow, 2.0);
    }

    #[test]
    fn publishes_v2_transfer_payload_with_visual_contract() {
        let (mut app, rx) = setup_app();
        let caster = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<Events<ContamTransferredEvent>>()
            .send(ContamTransferredEvent {
                caster,
                tier: crate::combat::tuike_v2::FalseSkinTier::Ancient,
                contam_moved_percent: 15.0,
                backflow_percent: 0.0,
                permanent_absorbed: 0.4,
                qi_cost: 105.0,
                tick: 42,
                visual: crate::combat::tuike_v2::TuikeSkillVisual::for_skill(
                    crate::combat::tuike_v2::TuikeSkillId::TransferTaint,
                    true,
                )
                .into(),
            });

        app.update();

        let outbound = rx
            .try_iter()
            .find(|event| matches!(event, RedisOutbound::TuikeV2SkillEvent(_)))
            .expect("expected outbound tuike v2 event");
        let RedisOutbound::TuikeV2SkillEvent(payload) = outbound else {
            panic!("expected TuikeV2SkillEvent");
        };
        assert_eq!(payload.skill_id, TuikeSkillIdV1::TransferTaint);
        assert_eq!(payload.tier, FalseSkinTierV1::Ancient);
        assert_eq!(payload.permanent_absorbed, 0.4);
        assert_eq!(payload.particle_id, "bong:ancient_skin_glow");
        assert!(payload.caster_id.starts_with("char:"));
    }

    /// plan-combat-skill-feedback-bridges-v1 P6 — PermanentTaintAbsorbedEvent 补 reader 测试。
    /// 验证：emit → TuikeV2SkillEvent(skill_id=TransferTaint, permanent_absorbed=X) 1 条。
    /// 守恒叙事：PermanentQiMaxDecay already removed at skills.rs:232；桥只转发 amount，不触 qi。
    #[test]
    fn publishes_permanent_taint_absorbed_payload() {
        let (mut app, rx) = setup_app();
        let caster = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<Events<PermanentTaintAbsorbedEvent>>()
            .send(PermanentTaintAbsorbedEvent {
                caster,
                amount: 0.33,
                tier: crate::combat::tuike_v2::FalseSkinTier::Mid,
                tick: 77,
            });

        app.update();

        let outbound = rx
            .try_iter()
            .find(|event| matches!(event, RedisOutbound::TuikeV2SkillEvent(_)))
            .expect(
                "期望 TuikeV2SkillEvent；PermanentTaintAbsorbedEvent 应产出 TuikeV2SkillEvent \
                 (守恒叙事：衰败已被皮吸收，真元上限不再缩减)",
            );
        let RedisOutbound::TuikeV2SkillEvent(payload) = outbound else {
            panic!("应为 TuikeV2SkillEvent");
        };
        assert_eq!(
            payload.skill_id,
            TuikeSkillIdV1::TransferTaint,
            "PermanentTaintAbsorbed 应复用 TransferTaint skill_id"
        );
        assert_eq!(payload.tier, FalseSkinTierV1::Mid, "tier 应正确传递");
        assert!(
            (payload.permanent_absorbed - 0.33).abs() < 1e-9,
            "permanent_absorbed 应为 0.33（守恒：只转发 amount，不重算）；实际={}",
            payload.permanent_absorbed
        );
        assert_eq!(payload.tick, 77);
    }

    #[test]
    fn permanent_taint_ancient_uses_ancient_particle() {
        // Ancient tier PermanentTaintAbsorbedEvent 应使用 ancient_skin_glow 粒子（差异化叙事）
        let (mut app, rx) = setup_app();
        let caster = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<Events<PermanentTaintAbsorbedEvent>>()
            .send(PermanentTaintAbsorbedEvent {
                caster,
                amount: 1.0,
                tier: crate::combat::tuike_v2::FalseSkinTier::Ancient,
                tick: 88,
            });

        app.update();

        let outbound = rx
            .try_iter()
            .find(|event| matches!(event, RedisOutbound::TuikeV2SkillEvent(_)))
            .expect("期望 TuikeV2SkillEvent；Ancient PermanentTaintAbsorbed");
        let RedisOutbound::TuikeV2SkillEvent(payload) = outbound else {
            panic!("应为 TuikeV2SkillEvent");
        };
        assert_eq!(
            payload.particle_id, "bong:ancient_skin_glow",
            "Ancient tier 应使用 ancient_skin_glow（差异化 VFX）"
        );
        assert_eq!(payload.tier, FalseSkinTierV1::Ancient);
    }
}
