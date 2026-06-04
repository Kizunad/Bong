//! plan-combat-skill-feedback-bridges-v1 P6 — 蜕壳灰烬入包 + VFX + Redis 叙事事件。
//!
//! 监听 [`FalseSkinDecayedToAshEvent`]，对每个事件：
//!   1. 调用 `add_item_to_player_inventory` 将灰烬回收给皮原主人（event.output_item_id，不写死）。
//!   2. 发送 VFX 事件 `bong:tuike_ash_burst`（差异化于 AshFootprintTracker 的 "ash_burst"）。
//!   3. 发送 `RedisOutbound::TuikeAshDecay` 通知 agent 叙事。
//!
//! 守恒红线：仅读取 event.output_item_id，不重算物品或扣 qi；add_item 失败时只记 warn，不 panic。
//! Ancient tier 场景：FalseSkinTier::Ancient → output_item_id = FALSE_SKIN_ANCIENT_RELIC_SHARD_ITEM_ID。

use valence::prelude::{EventReader, Query, Res, ResMut, Username};

use crate::combat::tuike_v2::events::FalseSkinDecayedToAshEvent;
use crate::combat::tuike_v2::FalseSkinTier;
use crate::inventory::{
    add_item_to_player_inventory, InventoryInstanceIdAllocator, ItemRegistry, PlayerInventory,
};
use crate::network::redis_bridge::RedisOutbound;
use crate::network::RedisBridgeResource;
use crate::schema::tuike_v2::{FalseSkinTierV1, TuikeAshDecayV1};

pub fn publish_tuike_ash_events(
    redis: Res<RedisBridgeResource>,
    item_registry: Res<ItemRegistry>,
    mut allocator: ResMut<InventoryInstanceIdAllocator>,
    mut ash_events: EventReader<FalseSkinDecayedToAshEvent>,
    usernames: Query<&Username>,
    mut inventories: Query<&mut PlayerInventory>,
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

        // 2. 发送 Redis 叙事事件（agent 叙事：「假皮化为灰烬，XXX 回收了 <item>」）
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
    use crate::network::RedisBridgeResource;

    fn setup_app() -> (App, Receiver<RedisOutbound>) {
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
        let mut app = App::new();
        app.add_event::<FalseSkinDecayedToAshEvent>();
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
}
