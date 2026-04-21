//! plan-shelflife-v1 §4 / M4 — 神识感知（FreshnessProbeIntent）。
//!
//! 玩家主动查看物品保质期的服务端通道。修为 ≥ 凝脉 才获精确数值；
//! 低阶修为只能通过 client M3b 的 5 档显示位（不走本通道）。
//!
//! M4a scope：events + resolver + realm gate + 空 predicted_event。
//! M4b 延后：predicted_event_ticks 预测算法 + 容器 lookup +
//! ClientRequest wire 触发路径（rights 层）。

use valence::prelude::{bevy_ecs, Entity, Event, EventReader, EventWriter, Query, Res};

use super::compute::{compute_current_qi, compute_track_state};
use super::container::container_storage_multiplier;
use super::registry::DecayProfileRegistry;
use super::types::{ContainerFreshnessBehavior, DecayTrack, TrackState};
use crate::cultivation::components::{Cultivation, Realm};
use crate::inventory::{inventory_item_by_instance_borrow, PlayerInventory};

/// plan §4 — 神识感知查询意图。`issued_at_tick` 由 caller（chat 命令 / 未来
/// ClientRequest wire）在构造时设置当前 server tick。
#[derive(Debug, Clone, Event)]
pub struct FreshnessProbeIntent {
    /// 发起查询的玩家 entity。
    pub player: Entity,
    /// 被查询物品的 ECS instance_id（玩家 inventory 中必须存在）。
    pub instance_id: u64,
    /// 发起时的 server tick — 用于 compute_* 的时间计算。
    pub issued_at_tick: u64,
}

/// plan §4 — 神识感知查询回执。
#[derive(Debug, Clone, Event)]
pub struct FreshnessProbeResponse {
    pub player: Entity,
    pub instance_id: u64,
    pub result: ProbeResult,
}

#[derive(Debug, Clone)]
pub enum ProbeResult {
    /// 查询被拒 — 修为不足 / 物品不存在 / 物品无 freshness / profile 未注册。
    Denied { reason: ProbeDenialReason },
    /// 修为 ≥ 凝脉 通过查询，返回精确当下数值 + 路径机态。
    Precise {
        /// Codex review r#36 P2 — track 与 track_state 必须同时返回：Fresh/Declining 是
        /// path-agnostic 状态，UI 需要 track 才能选正确分档词（Decay "鲜品" vs Spoil
        /// "新鲜" vs Age "青涩"）；避免消费方二次 inventory lookup 的 race 窗口。
        track: DecayTrack,
        current_qi: f32,
        initial_qi: f32,
        track_state: TrackState,
        /// plan §4 — 距 Dead / Spoiled / Peak 还剩多少 tick 的预测。
        /// M4a 暂固定 None（predictor 在 M4b）。
        predicted_event_ticks: Option<u64>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeDenialReason {
    /// 修为 < 凝脉。凡修 / 醒灵 / 引气 不具神识感知能力。
    RealmTooLow,
    /// Inventory 中找不到此 instance_id。
    ItemNotFound,
    /// 物品本身无 freshness NBT（凡俗工具 / 瑶器 等无时间敏感物品）。
    NoFreshness,
    /// `freshness.profile` 未在 `DecayProfileRegistry` 注册。
    ProfileNotRegistered,
}

/// plan §4 最低修为门槛 — 凝脉（rank 2）。
const MIN_PROBE_REALM_RANK: u8 = 2;

/// 修为等级序数化（Awaken=0 ... Void=5）。cultivation::Realm 未 derive PartialOrd，
/// 在此本地定义以避免跨模块耦合。
fn realm_rank(realm: Realm) -> u8 {
    match realm {
        Realm::Awaken => 0,
        Realm::Induce => 1,
        Realm::Condense => 2,
        Realm::Solidify => 3,
        Realm::Spirit => 4,
        Realm::Void => 5,
    }
}

/// resolver — 读 Intent，查 inventory + registry，按修为分档 emit Response。
pub fn resolve_freshness_probe_intents(
    mut intents: EventReader<FreshnessProbeIntent>,
    mut responses: EventWriter<FreshnessProbeResponse>,
    registry: Res<DecayProfileRegistry>,
    players: Query<(&PlayerInventory, &Cultivation)>,
) {
    for intent in intents.read() {
        let reply = resolve_one_probe(intent, &registry, &players);
        responses.send(reply);
    }
}

/// 单次 probe 解析 — 拆出便于单测直接调用（不依赖 Bevy 事件系统）。
fn resolve_one_probe(
    intent: &FreshnessProbeIntent,
    registry: &DecayProfileRegistry,
    players: &Query<(&PlayerInventory, &Cultivation)>,
) -> FreshnessProbeResponse {
    // 1. 玩家查不到 → ItemNotFound（同时覆盖"玩家 entity 失效"）
    let Ok((inventory, cultivation)) = players.get(intent.player) else {
        return denied(intent, ProbeDenialReason::ItemNotFound);
    };

    // 2. 修为 < 凝脉 → RealmTooLow
    if realm_rank(cultivation.realm) < MIN_PROBE_REALM_RANK {
        return denied(intent, ProbeDenialReason::RealmTooLow);
    }

    // 3. Inventory 查 instance_id — 用 borrow 版本避免 clone ~5-6 次 String heap alloc
    let Some(item) = inventory_item_by_instance_borrow(inventory, intent.instance_id) else {
        return denied(intent, ProbeDenialReason::ItemNotFound);
    };

    // 4. Item 无 freshness
    let Some(freshness) = item.freshness.as_ref() else {
        return denied(intent, ProbeDenialReason::NoFreshness);
    };

    // 5. Profile 未注册
    let Some(profile) = registry.get(&freshness.profile) else {
        return denied(intent, ProbeDenialReason::ProfileNotRegistered);
    };

    // 6. 全部通过 — compute + 返回精确结果。
    //    容器行为 M4a 默认 Normal；M4b 接入 item 所在容器 lookup。
    let multiplier = container_storage_multiplier(&ContainerFreshnessBehavior::Normal, profile);
    let current_qi = compute_current_qi(freshness, profile, intent.issued_at_tick, multiplier);
    let track_state = compute_track_state(freshness, profile, intent.issued_at_tick, multiplier);

    FreshnessProbeResponse {
        player: intent.player,
        instance_id: intent.instance_id,
        result: ProbeResult::Precise {
            track: freshness.track,
            current_qi,
            initial_qi: freshness.initial_qi,
            track_state,
            predicted_event_ticks: None,
        },
    }
}

fn denied(intent: &FreshnessProbeIntent, reason: ProbeDenialReason) -> FreshnessProbeResponse {
    FreshnessProbeResponse {
        player: intent.player,
        instance_id: intent.instance_id,
        result: ProbeResult::Denied { reason },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::{Cultivation, Realm};
    use crate::inventory::{
        ContainerState, ItemInstance, ItemRarity, PlacedItemState, PlayerInventory,
    };
    use crate::shelflife::{DecayFormula, DecayProfile, DecayProfileId, Freshness};
    use std::collections::HashMap;
    use valence::prelude::{App, Update};

    fn sample_profile() -> DecayProfile {
        DecayProfile::Decay {
            id: DecayProfileId::new("probe_test_decay"),
            formula: DecayFormula::Exponential {
                half_life_ticks: 1000,
            },
            floor_qi: 0.0,
        }
    }

    fn make_item(instance_id: u64, freshness: Option<Freshness>) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: "ling_shi_fan".to_string(),
            display_name: "凡品灵石".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.5,
            rarity: ItemRarity::Common,
            description: "末法残石".to_string(),
            stack_count: 1,
            spirit_quality: 0.7,
            durability: 1.0,
            freshness,
        }
    }

    fn make_inventory_with_item(item: ItemInstance) -> PlayerInventory {
        let container = ContainerState {
            id: "main_pack".to_string(),
            name: "main_pack".to_string(),
            rows: 4,
            cols: 4,
            items: vec![PlacedItemState {
                row: 0,
                col: 0,
                instance: item,
            }],
        };
        PlayerInventory {
            revision: crate::inventory::InventoryRevision(1),
            containers: vec![container],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 100.0,
        }
    }

    fn make_cultivation(realm: Realm) -> Cultivation {
        Cultivation {
            realm,
            ..Default::default()
        }
    }

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_event::<FreshnessProbeIntent>();
        app.add_event::<FreshnessProbeResponse>();
        app.insert_resource(DecayProfileRegistry::default());
        app.add_systems(Update, resolve_freshness_probe_intents);
        app
    }

    fn drain_responses(app: &mut App) -> Vec<FreshnessProbeResponse> {
        let world = app.world_mut();
        let mut events = world.resource_mut::<valence::prelude::Events<FreshnessProbeResponse>>();
        events.drain().collect()
    }

    // =========== Denied paths ===========

    #[test]
    fn denied_when_player_entity_does_not_exist() {
        let mut app = setup_app();
        let ghost = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(FreshnessProbeIntent {
            player: ghost,
            instance_id: 99,
            issued_at_tick: 0,
        });
        app.update();
        let res = drain_responses(&mut app);
        assert_eq!(res.len(), 1);
        match res[0].result {
            ProbeResult::Denied {
                reason: ProbeDenialReason::ItemNotFound,
            } => {}
            ref other => panic!("expected ItemNotFound, got {other:?}"),
        }
    }

    #[test]
    fn denied_realm_below_condense() {
        let mut app = setup_app();
        let profile = sample_profile();
        app.world_mut()
            .resource_mut::<DecayProfileRegistry>()
            .insert(profile.clone())
            .unwrap();

        for realm in [Realm::Awaken, Realm::Induce] {
            let freshness = Freshness::new(0, 100.0, &profile);
            let item = make_item(1, Some(freshness));
            let inv = make_inventory_with_item(item);
            let player = app.world_mut().spawn((inv, make_cultivation(realm))).id();

            app.world_mut().send_event(FreshnessProbeIntent {
                player,
                instance_id: 1,
                issued_at_tick: 500,
            });
            app.update();

            let res = drain_responses(&mut app);
            assert_eq!(res.len(), 1, "realm {realm:?}");
            match res[0].result {
                ProbeResult::Denied {
                    reason: ProbeDenialReason::RealmTooLow,
                } => {}
                ref other => panic!("realm {realm:?} expected RealmTooLow, got {other:?}"),
            }
        }
    }

    #[test]
    fn denied_item_not_found_when_instance_id_missing() {
        let mut app = setup_app();
        let profile = sample_profile();
        app.world_mut()
            .resource_mut::<DecayProfileRegistry>()
            .insert(profile)
            .unwrap();

        let inv = make_inventory_with_item(make_item(1, None));
        let player = app
            .world_mut()
            .spawn((inv, make_cultivation(Realm::Condense)))
            .id();

        app.world_mut().send_event(FreshnessProbeIntent {
            player,
            instance_id: 9999, // 不存在
            issued_at_tick: 0,
        });
        app.update();

        let res = drain_responses(&mut app);
        assert_eq!(res.len(), 1);
        match res[0].result {
            ProbeResult::Denied {
                reason: ProbeDenialReason::ItemNotFound,
            } => {}
            ref other => panic!("expected ItemNotFound, got {other:?}"),
        }
    }

    #[test]
    fn denied_no_freshness_when_item_untracked() {
        let mut app = setup_app();
        let profile = sample_profile();
        app.world_mut()
            .resource_mut::<DecayProfileRegistry>()
            .insert(profile)
            .unwrap();

        let inv = make_inventory_with_item(make_item(1, None)); // 无 freshness
        let player = app
            .world_mut()
            .spawn((inv, make_cultivation(Realm::Condense)))
            .id();

        app.world_mut().send_event(FreshnessProbeIntent {
            player,
            instance_id: 1,
            issued_at_tick: 0,
        });
        app.update();

        let res = drain_responses(&mut app);
        match res[0].result {
            ProbeResult::Denied {
                reason: ProbeDenialReason::NoFreshness,
            } => {}
            ref other => panic!("expected NoFreshness, got {other:?}"),
        }
    }

    #[test]
    fn denied_profile_not_registered() {
        let mut app = setup_app();
        // 空 registry — profile 不在

        let profile = sample_profile();
        let freshness = Freshness::new(0, 100.0, &profile);
        let inv = make_inventory_with_item(make_item(1, Some(freshness)));
        let player = app
            .world_mut()
            .spawn((inv, make_cultivation(Realm::Condense)))
            .id();

        app.world_mut().send_event(FreshnessProbeIntent {
            player,
            instance_id: 1,
            issued_at_tick: 100,
        });
        app.update();

        let res = drain_responses(&mut app);
        match res[0].result {
            ProbeResult::Denied {
                reason: ProbeDenialReason::ProfileNotRegistered,
            } => {}
            ref other => panic!("expected ProfileNotRegistered, got {other:?}"),
        }
    }

    // =========== Precise paths ===========

    #[test]
    fn precise_at_condense_returns_current_and_state() {
        let mut app = setup_app();
        let profile = sample_profile();
        app.world_mut()
            .resource_mut::<DecayProfileRegistry>()
            .insert(profile.clone())
            .unwrap();

        let freshness = Freshness::new(0, 100.0, &profile);
        let inv = make_inventory_with_item(make_item(1, Some(freshness)));
        let player = app
            .world_mut()
            .spawn((inv, make_cultivation(Realm::Condense)))
            .id();

        // tick 1000 = 1 half_life → current = 50
        app.world_mut().send_event(FreshnessProbeIntent {
            player,
            instance_id: 1,
            issued_at_tick: 1000,
        });
        app.update();

        let res = drain_responses(&mut app);
        match res[0].result {
            ProbeResult::Precise {
                track,
                current_qi,
                initial_qi,
                track_state,
                predicted_event_ticks,
            } => {
                assert_eq!(track, DecayTrack::Decay);
                assert!((current_qi - 50.0).abs() < 1e-3);
                assert!((initial_qi - 100.0).abs() < 1e-3);
                assert_eq!(track_state, TrackState::Declining);
                assert!(predicted_event_ticks.is_none(), "M4a predictor not wired");
            }
            ref other => panic!("expected Precise, got {other:?}"),
        }
    }

    #[test]
    fn precise_at_void_realm_returns_result() {
        // Void (rank 5) >> Condense gate (rank 2) — 通过
        let mut app = setup_app();
        let profile = sample_profile();
        app.world_mut()
            .resource_mut::<DecayProfileRegistry>()
            .insert(profile.clone())
            .unwrap();

        let freshness = Freshness::new(0, 100.0, &profile);
        let inv = make_inventory_with_item(make_item(1, Some(freshness)));
        let player = app
            .world_mut()
            .spawn((inv, make_cultivation(Realm::Void)))
            .id();

        app.world_mut().send_event(FreshnessProbeIntent {
            player,
            instance_id: 1,
            issued_at_tick: 0,
        });
        app.update();

        let res = drain_responses(&mut app);
        assert!(matches!(res[0].result, ProbeResult::Precise { .. }));
    }

    #[test]
    fn realm_rank_orders_all_six_realms() {
        assert_eq!(realm_rank(Realm::Awaken), 0);
        assert_eq!(realm_rank(Realm::Induce), 1);
        assert_eq!(realm_rank(Realm::Condense), 2);
        assert_eq!(realm_rank(Realm::Solidify), 3);
        assert_eq!(realm_rank(Realm::Spirit), 4);
        assert_eq!(realm_rank(Realm::Void), 5);
    }

    // Codex review r#36 P2 — Precise payload 必须携带 DecayTrack，避免消费方二次查询。
    #[test]
    fn precise_returns_decay_track_for_spoil_profile() {
        let mut app = setup_app();
        let spoil_profile = DecayProfile::Spoil {
            id: DecayProfileId::new("probe_test_spoil"),
            formula: DecayFormula::Exponential {
                half_life_ticks: 1000,
            },
            spoil_threshold: 10.0,
        };
        app.world_mut()
            .resource_mut::<DecayProfileRegistry>()
            .insert(spoil_profile.clone())
            .unwrap();

        let freshness = Freshness::new(0, 100.0, &spoil_profile);
        let inv = make_inventory_with_item(make_item(1, Some(freshness)));
        let player = app
            .world_mut()
            .spawn((inv, make_cultivation(Realm::Condense)))
            .id();

        app.world_mut().send_event(FreshnessProbeIntent {
            player,
            instance_id: 1,
            issued_at_tick: 0,
        });
        app.update();

        let res = drain_responses(&mut app);
        match res[0].result {
            ProbeResult::Precise {
                track, track_state, ..
            } => {
                assert_eq!(
                    track,
                    DecayTrack::Spoil,
                    "Spoil profile must yield Spoil track"
                );
                assert_eq!(track_state, TrackState::Fresh);
            }
            ref other => panic!("expected Precise, got {other:?}"),
        }
    }

    #[test]
    fn precise_returns_decay_track_for_age_profile() {
        let mut app = setup_app();
        let age_profile = DecayProfile::Age {
            id: DecayProfileId::new("probe_test_age"),
            peak_at_ticks: 1000,
            peak_bonus: 0.5,
            peak_window_ratio: 0.1,
            post_peak_half_life_ticks: 500,
            post_peak_spoil_threshold: 30.0,
            post_peak_spoil_profile: DecayProfileId::new("probe_test_age_spoil"),
        };
        app.world_mut()
            .resource_mut::<DecayProfileRegistry>()
            .insert(age_profile.clone())
            .unwrap();

        let freshness = Freshness::new(0, 100.0, &age_profile);
        let inv = make_inventory_with_item(make_item(1, Some(freshness)));
        let player = app
            .world_mut()
            .spawn((inv, make_cultivation(Realm::Condense)))
            .id();

        app.world_mut().send_event(FreshnessProbeIntent {
            player,
            instance_id: 1,
            issued_at_tick: 0,
        });
        app.update();

        let res = drain_responses(&mut app);
        match res[0].result {
            ProbeResult::Precise { track, .. } => {
                assert_eq!(track, DecayTrack::Age, "Age profile must yield Age track");
            }
            ref other => panic!("expected Precise, got {other:?}"),
        }
    }

    #[test]
    fn probe_at_exact_gate_realm_condense_is_allowed() {
        // 边界值：修为 == MIN_PROBE_REALM_RANK（凝脉）应通过，不 deny
        let mut app = setup_app();
        let profile = sample_profile();
        app.world_mut()
            .resource_mut::<DecayProfileRegistry>()
            .insert(profile.clone())
            .unwrap();

        let freshness = Freshness::new(0, 100.0, &profile);
        let inv = make_inventory_with_item(make_item(1, Some(freshness)));
        let player = app
            .world_mut()
            .spawn((inv, make_cultivation(Realm::Condense)))
            .id();

        app.world_mut().send_event(FreshnessProbeIntent {
            player,
            instance_id: 1,
            issued_at_tick: 0,
        });
        app.update();

        let res = drain_responses(&mut app);
        assert!(
            matches!(res[0].result, ProbeResult::Precise { .. }),
            "凝脉 应通过 gate（边界包含），got {:?}",
            res[0].result
        );
    }
}
