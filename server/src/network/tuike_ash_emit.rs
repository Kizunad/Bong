//! plan-combat-skill-feedback-bridges-v1 P6 — 蜕壳灰烬入包 + VFX + Redis 叙事事件。
//!
//! 监听 [`FalseSkinDecayedToAshEvent`]，对每个事件：
//!   1. 调用 `add_item_to_player_inventory` 将灰烬回收给皮原主人（event.output_item_id，不写死）。
//!   2. 发送 VFX 事件 `bong:tuike_ash_burst`（差异化于 AshFootprintTracker 的 "ash_burst"）。
//!   3. 发送 `RedisOutbound::TuikeAshDecay` 通知 agent 叙事。
//!
//! 守恒红线：仅读取 event.output_item_id，不重算物品或扣 qi；add_item 失败时只记 warn，不 panic。
//! Ancient tier 场景：FalseSkinTier::Ancient → output_item_id = FALSE_SKIN_ANCIENT_RELIC_SHARD_ITEM_ID。

use valence::prelude::{DVec3, EventReader, EventWriter, Position, Query, Res, ResMut, Username};

use crate::combat::tuike_v2::events::FalseSkinDecayedToAshEvent;
use crate::combat::tuike_v2::FalseSkinTier;
use crate::inventory::{
    add_item_to_player_inventory, InventoryInstanceIdAllocator, ItemRegistry, PlayerInventory,
};
use crate::network::redis_bridge::RedisOutbound;
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::network::RedisBridgeResource;
use crate::schema::tuike_v2::{FalseSkinTierV1, TuikeAshDecayV1};
use crate::schema::vfx_event::VfxEventPayloadV1;

/// VFX 粒子 id —— 灰烬迸发效果（差异化于 AshFootprintTracker 的 ash_burst）。
pub const TUIKE_ASH_BURST_VFX_ID: &str = "bong:tuike_ash_burst";

#[allow(clippy::too_many_arguments)]
pub fn publish_tuike_ash_events(
    redis: Res<RedisBridgeResource>,
    item_registry: Res<ItemRegistry>,
    mut allocator: ResMut<InventoryInstanceIdAllocator>,
    mut ash_events: EventReader<FalseSkinDecayedToAshEvent>,
    usernames: Query<&Username>,
    mut inventories: Query<&mut PlayerInventory>,
    positions: Query<&Position>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for event in ash_events.read() {
        // 1. 回收灰烬物品到原主人背包（取 event.output_item_id，不写死，Ancient tier = relic shard）
        match inventories.get_mut(event.owner) {
            Ok(mut inventory) => {
                if let Err(err) = add_item_to_player_inventory(
                    &mut inventory,
                    &item_registry,
                    &mut allocator,
                    &event.output_item_id,
                    1,
                    0,
                ) {
                    tracing::warn!(
                        "[bong][tuike-ash] add_item_to_player_inventory failed \
                        for owner={:?} item='{}': {err}",
                        event.owner,
                        event.output_item_id
                    );
                }
            }
            Err(_) => {
                tracing::warn!(
                    "[bong][tuike-ash] owner entity {:?} missing PlayerInventory; \
                    ash item '{}' not granted",
                    event.owner,
                    event.output_item_id
                );
            }
        }

        // 2. 向灰烬原主人位置发 VFX（bong:tuike_ash_burst）
        let origin = positions
            .get(event.owner)
            .map(|p| p.get())
            .unwrap_or(DVec3::ZERO);
        vfx_events.send(VfxEventRequest::new(
            origin,
            VfxEventPayloadV1::SpawnParticle {
                event_id: TUIKE_ASH_BURST_VFX_ID.to_string(),
                origin: [origin.x, origin.y + 1.0, origin.z],
                direction: Some([0.0, 0.1, 0.0]),
                color: None,
                strength: Some(1.0),
                count: Some(1),
                duration_ticks: Some(20),
            },
        ));

        // 3. 发送 Redis 叙事事件（agent 叙事：「假皮化为灰烬，XXX 回收了 <item>」）
        let owner_id = usernames
            .get(event.owner)
            .map(|u| format!("offline:{}", u.0))
            .unwrap_or_else(|_| format!("char:{}", event.owner.to_bits()));

        let redis_event = TuikeAshDecayV1 {
            owner_id,
            output_item_id: event.output_item_id.clone(),
            tier: tier_to_v1(event.tier),
            tick: event.tick,
        };

        if let Err(err) = redis
            .tx_outbound
            .send(RedisOutbound::TuikeAshDecay(redis_event))
        {
            tracing::warn!("[bong][tuike-ash] failed to queue TuikeAshDecay event: {err}");
        }
    }
}

fn tier_to_v1(tier: FalseSkinTier) -> FalseSkinTierV1 {
    match tier {
        FalseSkinTier::Fan => FalseSkinTierV1::Fan,
        FalseSkinTier::Light => FalseSkinTierV1::Light,
        FalseSkinTier::Mid => FalseSkinTierV1::Mid,
        FalseSkinTier::Heavy => FalseSkinTierV1::Heavy,
        FalseSkinTier::Ancient => FalseSkinTierV1::Ancient,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::Receiver;
    use valence::prelude::{App, Events, Update};

    use crate::combat::tuike_v2::events::FalseSkinDecayedToAshEvent;
    use crate::combat::tuike_v2::state::{
        FALSE_SKIN_ANCIENT_RELIC_SHARD_ITEM_ID, FALSE_SKIN_ASH_ITEM_ID,
    };
    use crate::combat::tuike_v2::FalseSkinTier;
    use crate::network::redis_bridge::RedisOutbound;
    use crate::network::vfx_event_emit::VfxEventRequest;
    use crate::network::RedisBridgeResource;

    fn setup_app() -> (App, Receiver<RedisOutbound>) {
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
        let mut app = App::new();
        app.add_event::<FalseSkinDecayedToAshEvent>();
        app.add_event::<VfxEventRequest>();
        app.insert_resource(crate::inventory::ItemRegistry::default());
        app.insert_resource(crate::inventory::InventoryInstanceIdAllocator::default());
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_systems(Update, publish_tuike_ash_events);
        (app, rx_outbound)
    }

    // ── tier_to_v1 pin tests ──────────────────────────────────────────────

    #[test]
    fn tier_to_v1_all_variants() {
        assert_eq!(tier_to_v1(FalseSkinTier::Fan), FalseSkinTierV1::Fan);
        assert_eq!(tier_to_v1(FalseSkinTier::Light), FalseSkinTierV1::Light);
        assert_eq!(tier_to_v1(FalseSkinTier::Mid), FalseSkinTierV1::Mid);
        assert_eq!(tier_to_v1(FalseSkinTier::Heavy), FalseSkinTierV1::Heavy);
        assert_eq!(tier_to_v1(FalseSkinTier::Ancient), FalseSkinTierV1::Ancient);
    }

    // ── Redis publish tests ───────────────────────────────────────────────

    #[test]
    fn mid_tier_publishes_redis_event_with_ash_item_id() {
        let (mut app, rx) = setup_app();
        let owner = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<Events<FalseSkinDecayedToAshEvent>>()
            .send(FalseSkinDecayedToAshEvent {
                owner,
                tier: FalseSkinTier::Mid,
                output_item_id: FALSE_SKIN_ASH_ITEM_ID.to_string(),
                tick: 100,
            });

        app.update();

        let outbound = rx
            .try_iter()
            .find(|e| matches!(e, RedisOutbound::TuikeAshDecay(_)))
            .expect("期望 TuikeAshDecay Redis 事件；Mid tier 应产出 ash item");
        let RedisOutbound::TuikeAshDecay(payload) = outbound else {
            panic!("应为 TuikeAshDecay");
        };
        assert_eq!(
            payload.output_item_id, FALSE_SKIN_ASH_ITEM_ID,
            "Mid tier 蜕壳灰烬 output_item_id 应为普通灰烬 id（取 event.output_item_id，不写死）"
        );
        assert_eq!(payload.tier, FalseSkinTierV1::Mid);
        assert_eq!(payload.tick, 100);
    }

    #[test]
    fn ancient_tier_publishes_relic_shard_item_id() {
        // 边界 case：Ancient tier → FALSE_SKIN_ANCIENT_RELIC_SHARD_ITEM_ID（不是普通灰烬）
        let (mut app, rx) = setup_app();
        let owner = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<Events<FalseSkinDecayedToAshEvent>>()
            .send(FalseSkinDecayedToAshEvent {
                owner,
                tier: FalseSkinTier::Ancient,
                output_item_id: FALSE_SKIN_ANCIENT_RELIC_SHARD_ITEM_ID.to_string(),
                tick: 200,
            });

        app.update();

        let outbound = rx
            .try_iter()
            .find(|e| matches!(e, RedisOutbound::TuikeAshDecay(_)))
            .expect("期望 TuikeAshDecay Redis 事件；Ancient tier 应产出 relic shard");
        let RedisOutbound::TuikeAshDecay(payload) = outbound else {
            panic!("应为 TuikeAshDecay");
        };
        assert_eq!(
            payload.output_item_id, FALSE_SKIN_ANCIENT_RELIC_SHARD_ITEM_ID,
            "Ancient tier 蜕壳 output_item_id 应为 relic shard（边界 case：写死 ash_id 会误产普通灰烬）"
        );
        assert_eq!(
            payload.tier,
            FalseSkinTierV1::Ancient,
            "Ancient tier 标识必须正确传递给 agent 叙事"
        );
    }

    #[test]
    fn no_events_means_zero_redis_publish() {
        let (mut app, rx) = setup_app();
        app.update();
        let ash_events: Vec<_> = rx
            .try_iter()
            .filter(|e| matches!(e, RedisOutbound::TuikeAshDecay(_)))
            .collect();
        assert!(
            ash_events.is_empty(),
            "无 FalseSkinDecayedToAshEvent 时不应发出任何 TuikeAshDecay；发出了 {} 条",
            ash_events.len()
        );
    }

    #[test]
    fn fan_tier_publishes_ash_item_id() {
        let (mut app, rx) = setup_app();
        let owner = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<Events<FalseSkinDecayedToAshEvent>>()
            .send(FalseSkinDecayedToAshEvent {
                owner,
                tier: FalseSkinTier::Fan,
                output_item_id: FALSE_SKIN_ASH_ITEM_ID.to_string(),
                tick: 50,
            });

        app.update();

        let outbound = rx
            .try_iter()
            .find(|e| matches!(e, RedisOutbound::TuikeAshDecay(_)))
            .expect("期望 TuikeAshDecay Redis 事件；Fan tier");
        let RedisOutbound::TuikeAshDecay(payload) = outbound else {
            panic!("应为 TuikeAshDecay");
        };
        assert_eq!(payload.output_item_id, FALSE_SKIN_ASH_ITEM_ID);
        assert_eq!(payload.tier, FalseSkinTierV1::Fan);
    }

    // ── TuikeAshDecayV1 serde roundtrip ──────────────────────────────────

    #[test]
    fn tuike_ash_decay_v1_serde_roundtrip() {
        use crate::schema::tuike_v2::TuikeAshDecayV1;

        let original = TuikeAshDecayV1 {
            owner_id: "offline:Azure".to_string(),
            output_item_id: FALSE_SKIN_ASH_ITEM_ID.to_string(),
            tier: FalseSkinTierV1::Mid,
            tick: 333,
        };
        let json = serde_json::to_string(&original).expect("TuikeAshDecayV1 应能序列化");
        let back: TuikeAshDecayV1 =
            serde_json::from_str(&json).expect("TuikeAshDecayV1 应能反序列化");
        assert_eq!(
            original, back,
            "TuikeAshDecayV1 JSON roundtrip 必须无损；JSON={json}"
        );
    }

    #[test]
    fn tuike_ash_decay_v1_ancient_serde_roundtrip() {
        use crate::schema::tuike_v2::TuikeAshDecayV1;

        let original = TuikeAshDecayV1 {
            owner_id: "offline:Azure".to_string(),
            output_item_id: FALSE_SKIN_ANCIENT_RELIC_SHARD_ITEM_ID.to_string(),
            tier: FalseSkinTierV1::Ancient,
            tick: 999,
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let back: TuikeAshDecayV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            original, back,
            "Ancient tier TuikeAshDecayV1 roundtrip 必须无损"
        );
    }

    // ── VFX emit 路径测试（red-when-reverted）────────────────────────────
    //
    // 本测试验证 publish_tuike_ash_events 在灰烬事件发生时必须 emit VfxEventRequest。
    // 若删掉 system 中 vfx_events.send(...) 那行，此测试即红。

    #[test]
    fn vfx_event_emitted_on_ash_decay() {
        let (mut app, _rx) = setup_app();
        let owner = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<Events<FalseSkinDecayedToAshEvent>>()
            .send(FalseSkinDecayedToAshEvent {
                owner,
                tier: FalseSkinTier::Mid,
                output_item_id: FALSE_SKIN_ASH_ITEM_ID.to_string(),
                tick: 42,
            });

        app.update();

        let vfx_events = app.world().resource::<Events<VfxEventRequest>>();
        let count = vfx_events.len();
        assert_eq!(
            count, 1,
            "FalseSkinDecayedToAshEvent 必须触发恰好 1 条 VfxEventRequest(bong:tuike_ash_burst)；\
            实际发出 {count} 条。删掉 vfx_events.send(...) 会让此测试红。"
        );

        let mut reader = vfx_events.get_reader();
        let vfx = reader
            .read(vfx_events)
            .next()
            .expect("VFX event should exist");
        match &vfx.payload {
            VfxEventPayloadV1::SpawnParticle { event_id, .. } => {
                assert_eq!(
                    event_id, TUIKE_ASH_BURST_VFX_ID,
                    "VFX event_id 必须为 bong:tuike_ash_burst，实际为 {event_id}"
                );
            }
            other => panic!("期望 SpawnParticle，实际 payload 类型={other:?}"),
        }
    }

    #[test]
    fn vfx_event_uses_owner_position() {
        // 有 Position 组件的 owner → VFX origin 与 Position 对齐
        let (mut app, _rx) = setup_app();
        use valence::prelude::DVec3;
        let pos = valence::prelude::Position::new(DVec3::new(10.0, 64.0, -5.0));
        let owner = app.world_mut().spawn(pos).id();
        app.world_mut()
            .resource_mut::<Events<FalseSkinDecayedToAshEvent>>()
            .send(FalseSkinDecayedToAshEvent {
                owner,
                tier: FalseSkinTier::Fan,
                output_item_id: FALSE_SKIN_ASH_ITEM_ID.to_string(),
                tick: 10,
            });

        app.update();

        let vfx_events = app.world().resource::<Events<VfxEventRequest>>();
        let mut reader = vfx_events.get_reader();
        let vfx = reader.read(vfx_events).next().expect("VFX event");
        // origin 字段应与 owner Position 对齐（DVec3 level）
        assert!(
            (vfx.origin.x - 10.0_f64).abs() < 1e-9 && (vfx.origin.z - -5.0_f64).abs() < 1e-9,
            "VFX origin 应与 owner Position 对齐；期望 x≈10 z≈-5，实际 origin={:?}",
            vfx.origin
        );
    }

    #[test]
    fn no_events_means_zero_vfx() {
        let (mut app, _rx) = setup_app();
        app.update();
        let vfx_events = app.world().resource::<Events<VfxEventRequest>>();
        assert_eq!(
            vfx_events.len(),
            0,
            "无 FalseSkinDecayedToAshEvent 时不应发出任何 VfxEventRequest"
        );
    }

    #[test]
    fn ancient_tier_also_emits_vfx() {
        // Ancient tier 同样必须 emit VFX（non-tier-conditional）
        let (mut app, _rx) = setup_app();
        let owner = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<Events<FalseSkinDecayedToAshEvent>>()
            .send(FalseSkinDecayedToAshEvent {
                owner,
                tier: FalseSkinTier::Ancient,
                output_item_id: FALSE_SKIN_ANCIENT_RELIC_SHARD_ITEM_ID.to_string(),
                tick: 500,
            });

        app.update();

        let vfx_events = app.world().resource::<Events<VfxEventRequest>>();
        assert_eq!(
            vfx_events.len(),
            1,
            "Ancient tier 蜕壳灰烬也必须发 VfxEventRequest；实际发出 {} 条",
            vfx_events.len()
        );
    }
}
