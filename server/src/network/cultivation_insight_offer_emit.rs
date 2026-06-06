//! 修炼顿悟 S2C 回执发送系统
//!
//! 消费 `InsightOffer` 事件（两路 producer：`insight_flow.rs:212` fallback
//! + `network/mod.rs:2263` agent-fed，共用同一事件队列），将顿悟邀约
//! 序列化为 `ServerDataPayloadV1::InsightOffer` 并仅发给触发者。
//!
//! 模板：`tribulation_heart_demon_offer_emit.rs`（已验证范本）。

use valence::prelude::{Client, Entity, EventReader, Query, Username, With};

use crate::cultivation::insight::{InsightCategory, InsightChoice, InsightEffect, InsightOffer};
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::player::state::canonical_player_id;
use crate::schema::cultivation::{InsightChoiceV1, InsightOfferV1};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

/// 每帧排空 `InsightOffer` 事件队列，仅发给 `ev.entity` 对应玩家，旁观者不收包。
///
/// 两路 producer（fallback + agent-fed）共用同一 InsightOffer 事件队列，
/// 本 reader 一并排空。
pub fn emit_cultivation_insight_offers(
    mut clients: Query<(Entity, &mut Client, &Username), With<Client>>,
    mut offers: EventReader<InsightOffer>,
) {
    for ev in offers.read() {
        if ev.choices.is_empty() {
            tracing::debug!(
                "[bong][cultivation] insight_offer trigger={:?} has empty choices; skipping",
                ev.trigger_id
            );
            continue;
        }

        // 从触发者 entity 反查 character_id（username → canonical_player_id）。
        let character_id = clients
            .iter()
            .find(|(entity, _, _)| *entity == ev.entity)
            .map(|(_, _, username)| canonical_player_id(username.0.as_str()))
            .unwrap_or_else(|| format!("offline:unknown:{}", ev.entity.index()));

        let offer_v1 = InsightOfferV1 {
            offer_id: format!("insight:{}:{}", ev.entity.index(), ev.trigger_id),
            trigger_id: ev.trigger_id.clone(),
            character_id,
            choices: ev.choices.iter().map(insight_choice_to_v1).collect(),
        };

        let payload = ServerDataV1::new(ServerDataPayloadV1::InsightOffer(offer_v1));
        let payload_type = payload_type_label(payload.payload_type());
        let payload_bytes = match serialize_server_data_payload(&payload) {
            Ok(bytes) => bytes,
            Err(error) => {
                log_payload_build_error(payload_type, &error);
                continue;
            }
        };

        for (entity, mut client, _) in &mut clients {
            if entity == ev.entity {
                send_server_data_payload(&mut client, payload_bytes.as_slice());
            }
        }
    }
}

/// `InsightChoice`（服务端内部）→ `InsightChoiceV1`（S2C wire）。
fn insight_choice_to_v1(c: &InsightChoice) -> InsightChoiceV1 {
    InsightChoiceV1 {
        category: insight_category_str(c.category).to_string(),
        effect_kind: insight_effect_kind_str(&c.effect).to_string(),
        magnitude: c.effect.magnitude(),
        flavor_text: c.flavor.clone(),
        narrator_voice: None,
        alignment: Some(c.alignment.code().to_string()),
        cost_kind: Some(c.cost.kind().to_string()),
        cost_magnitude: Some(c.cost.magnitude()),
        cost_flavor: if c.cost_flavor.is_empty() {
            None
        } else {
            Some(c.cost_flavor.clone())
        },
    }
}

/// `InsightCategory` → snake_case 字串（对齐 TS schema InsightCategory 字面量）。
fn insight_category_str(cat: InsightCategory) -> &'static str {
    match cat {
        InsightCategory::Meridian => "Meridian",
        InsightCategory::Qi => "Qi",
        InsightCategory::Composure => "Composure",
        InsightCategory::Coloring => "Coloring",
        InsightCategory::Breakthrough => "Breakthrough",
        InsightCategory::Style => "Style",
        InsightCategory::Perception => "Perception",
    }
}

/// `InsightEffect` 变体 → snake_case 字串（作为 effect_kind wire 值）。
fn insight_effect_kind_str(effect: &InsightEffect) -> &'static str {
    match effect {
        InsightEffect::MeridianRate { .. } => "meridian_rate",
        InsightEffect::MeridianForgeDiscount { .. } => "meridian_forge_discount",
        InsightEffect::MeridianOverloadTolerance { .. } => "meridian_overload_tolerance",
        InsightEffect::QiRegenFactor { .. } => "qi_regen_factor",
        InsightEffect::PurgeEfficiency { .. } => "purge_efficiency",
        InsightEffect::UnfreezeQiMax { .. } => "unfreeze_qi_max",
        InsightEffect::ComposureRecover { .. } => "composure_recover",
        InsightEffect::ComposureShockDiscount { .. } => "composure_shock_discount",
        InsightEffect::ComposureImmuneDuringBreakthrough => {
            "composure_immune_during_breakthrough"
        }
        InsightEffect::ColorCapAdd { .. } => "color_cap_add",
        InsightEffect::ChaoticTolerance { .. } => "chaotic_tolerance",
        InsightEffect::HunyuanThreshold { .. } => "hunyuan_threshold",
        InsightEffect::NextBreakthroughBonus { .. } => "next_breakthrough_bonus",
        InsightEffect::BreakthroughEventConditionDrop { .. } => {
            "breakthrough_event_condition_drop"
        }
        InsightEffect::TribulationPredictionWindow => "tribulation_prediction_window",
        InsightEffect::DualForgeDiscount { .. } => "dual_forge_discount",
        InsightEffect::ColorMaterialAffinity { .. } => "color_material_affinity",
        InsightEffect::ZhenfaConcealment { .. } => "zhenfa_concealment",
        InsightEffect::ZhenfaDisenchant { .. } => "zhenfa_disenchant",
        InsightEffect::VortexBackfireResist { .. } => "vortex_backfire_resist",
        InsightEffect::VortexDeltaBonus { .. } => "vortex_delta_bonus",
        InsightEffect::VortexFlowSpeed { .. } => "vortex_flow_speed",
        InsightEffect::UnlockPractice { .. } => "unlock_practice",
        InsightEffect::UnlockPerception { .. } => "unlock_perception",
        InsightEffect::LifespanExtensionEnlightenment => "lifespan_extension_enlightenment",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::cultivation::components::{ColorKind, MeridianId, Realm};
    use crate::cultivation::insight::{InsightAlignment, InsightCost, InsightEffect};
    use crate::network::agent_bridge::SERVER_DATA_CHANNEL;
    use crate::schema::server_data::ServerDataPayloadV1;
    use valence::prelude::{App, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    fn spawn_mock_client(app: &mut App, name: &str) -> (Entity, MockClientHelper) {
        let (bundle, helper) = create_mock_client(name);
        let entity = app.world_mut().spawn(bundle).id();
        (entity, helper)
    }

    fn flush_all_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();
        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush");
        }
    }

    fn collect_insight_offers(helper: &mut MockClientHelper) -> Vec<InsightOfferV1> {
        let mut offers = Vec::new();
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                continue;
            }
            let payload: ServerDataV1 = serde_json::from_slice(packet.data.0 .0)
                .expect("server data payload should decode");
            if let ServerDataPayloadV1::InsightOffer(offer) = payload.payload {
                offers.push(offer);
            }
        }
        offers
    }

    fn make_simple_choice() -> InsightChoice {
        InsightChoice {
            category: InsightCategory::Composure,
            alignment: InsightAlignment::Neutral,
            effect: InsightEffect::ComposureRecover { mul: 1.05 },
            flavor: "守本心，气机稳。".to_string(),
            cost: InsightCost::QiVolatility { add: 0.015 },
            cost_magnitude: 0.015,
            cost_flavor: "灵气更活，战斗中真元挥发加速。".to_string(),
            target_color: None,
        }
    }

    fn make_breakthrough_choice() -> InsightChoice {
        InsightChoice {
            category: InsightCategory::Breakthrough,
            alignment: InsightAlignment::Converge,
            effect: InsightEffect::NextBreakthroughBonus { add: 0.05 },
            flavor: "下次冲关必有所得。".to_string(),
            cost: InsightCost::ShockSensitivity { add: 0.02 },
            cost_magnitude: 0.02,
            cost_flavor: "心神更易受扰。".to_string(),
            target_color: None,
        }
    }

    fn make_meridian_choice() -> InsightChoice {
        InsightChoice {
            category: InsightCategory::Meridian,
            alignment: InsightAlignment::Diverge,
            effect: InsightEffect::MeridianRate {
                id: MeridianId::Lung,
                mul: 1.03,
            },
            flavor: "经脉通畅，气速提升。".to_string(),
            cost: InsightCost::MeridianHealSlowdown { mul: 0.95 },
            cost_magnitude: 0.05,
            cost_flavor: "经脉自愈减慢。".to_string(),
            target_color: None,
        }
    }

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_event::<InsightOffer>();
        app.add_systems(Update, emit_cultivation_insight_offers);
        app
    }

    // ─────────────────────────────────────────────────────────────
    // P2 测试矩阵：≥6 条
    // ─────────────────────────────────────────────────────────────

    /// Happy path：单 offer → InsightOfferV1 字段对拍。
    #[test]
    fn single_offer_fields_correct() {
        let mut app = setup_app();
        let (player, mut helper) = spawn_mock_client(&mut app, "Azure");

        app.world_mut().send_event(InsightOffer {
            entity: player,
            trigger_id: "bt:1".to_string(),
            choices: vec![make_simple_choice()],
        });

        app.update();
        flush_all_client_packets(&mut app);

        let offers = collect_insight_offers(&mut helper);
        assert_eq!(offers.len(), 1, "期望收到 1 个 InsightOffer payload");
        let o = &offers[0];
        assert_eq!(o.trigger_id, "bt:1", "trigger_id 应透传");
        assert!(!o.offer_id.is_empty(), "offer_id 不应为空");
        assert!(!o.character_id.is_empty(), "character_id 不应为空");
        assert_eq!(o.choices.len(), 1, "单选 offer 应有 1 个 choice");
        let c = &o.choices[0];
        assert_eq!(c.category, "Composure", "category 应为 Composure");
        assert_eq!(
            c.effect_kind, "composure_recover",
            "effect_kind 应为 composure_recover"
        );
        assert!(c.magnitude > 0.0, "magnitude 应 >0");
        assert!(!c.flavor_text.is_empty(), "flavor_text 不应为空");
    }

    /// 多 choice round-trip：含 cost_* 可选字段。
    #[test]
    fn multi_choice_with_cost_fields_round_trip() {
        let mut app = setup_app();
        let (player, mut helper) = spawn_mock_client(&mut app, "Kiz");

        app.world_mut().send_event(InsightOffer {
            entity: player,
            trigger_id: "bt:2".to_string(),
            choices: vec![
                make_simple_choice(),
                make_breakthrough_choice(),
                make_meridian_choice(),
            ],
        });

        app.update();
        flush_all_client_packets(&mut app);

        let offers = collect_insight_offers(&mut helper);
        assert_eq!(offers.len(), 1);
        let o = &offers[0];
        assert_eq!(o.choices.len(), 3, "3 个 choice 应全部保留");

        // cost 字段验证
        let choice_0 = &o.choices[0];
        assert!(choice_0.cost_kind.is_some(), "cost_kind 应存在");
        assert_eq!(
            choice_0.cost_kind.as_deref(),
            Some("qi_volatility"),
            "cost_kind 应为 qi_volatility"
        );
        assert!(choice_0.cost_magnitude.is_some(), "cost_magnitude 应存在");
        assert!(choice_0.cost_flavor.is_some(), "cost_flavor 应存在");

        // alignment 字段验证
        assert_eq!(
            choice_0.alignment.as_deref(),
            Some("neutral"),
            "Neutral alignment"
        );
        let choice_1 = &o.choices[1];
        assert_eq!(
            choice_1.alignment.as_deref(),
            Some("converge"),
            "Converge alignment"
        );
        let choice_2 = &o.choices[2];
        assert_eq!(
            choice_2.alignment.as_deref(),
            Some("diverge"),
            "Diverge alignment"
        );
    }

    /// 只发给触发者，旁观者 0 包。
    #[test]
    fn offer_sent_only_to_triggering_player() {
        let mut app = setup_app();
        let (player, mut player_helper) = spawn_mock_client(&mut app, "Azure");
        let (_spectator, mut spectator_helper) = spawn_mock_client(&mut app, "Spectator");

        app.world_mut().send_event(InsightOffer {
            entity: player,
            trigger_id: "bt:3".to_string(),
            choices: vec![make_simple_choice()],
        });

        app.update();
        flush_all_client_packets(&mut app);

        let player_offers = collect_insight_offers(&mut player_helper);
        let spectator_offers = collect_insight_offers(&mut spectator_helper);
        assert_eq!(player_offers.len(), 1, "触发玩家应收到 1 包");
        assert_eq!(spectator_offers.len(), 0, "旁观者不应收到任何包");
    }

    /// 空 choices 守卫：空 choices 时不发包。
    #[test]
    fn empty_choices_guard_no_packet_sent() {
        let mut app = setup_app();
        let (player, mut helper) = spawn_mock_client(&mut app, "Azure");

        app.world_mut().send_event(InsightOffer {
            entity: player,
            trigger_id: "bt:empty".to_string(),
            choices: vec![],
        });

        app.update();
        flush_all_client_packets(&mut app);

        let offers = collect_insight_offers(&mut helper);
        assert_eq!(
            offers.len(),
            0,
            "空 choices 不应发 InsightOffer payload（守卫丢弃）"
        );
    }

    /// fallback 路径（直接从 InsightOffer 事件）和 agent-fed 路径均由同一 reader 排空。
    #[test]
    fn two_offers_same_player_both_drained() {
        let mut app = setup_app();
        let (player, mut helper) = spawn_mock_client(&mut app, "Azure");

        // 模拟 fallback 路径产生的 offer
        app.world_mut().send_event(InsightOffer {
            entity: player,
            trigger_id: "fallback:1".to_string(),
            choices: vec![make_simple_choice()],
        });
        // 模拟 agent-fed 路径产生的 offer（同一玩家连续两次 offer）
        app.world_mut().send_event(InsightOffer {
            entity: player,
            trigger_id: "agent:1".to_string(),
            choices: vec![make_breakthrough_choice()],
        });

        app.update();
        flush_all_client_packets(&mut app);

        let offers = collect_insight_offers(&mut helper);
        assert_eq!(
            offers.len(),
            2,
            "两路 producer 的 offer 均应被同一 reader 排空并发送"
        );
        let trigger_ids: Vec<&str> = offers.iter().map(|o| o.trigger_id.as_str()).collect();
        assert!(
            trigger_ids.contains(&"fallback:1"),
            "fallback offer 应被发送"
        );
        assert!(trigger_ids.contains(&"agent:1"), "agent offer 应被发送");
    }

    /// 类型串隔离 pin：insight_offer ≠ heart_demon_offer。
    #[test]
    fn insight_offer_type_label_pin_not_heart_demon() {
        use crate::network::agent_bridge::payload_type_label;
        use crate::schema::server_data::ServerDataType;

        let insight_label = payload_type_label(ServerDataType::InsightOffer);
        let heart_demon_label = payload_type_label(ServerDataType::HeartDemonOffer);

        assert_eq!(
            insight_label, "insight_offer",
            "InsightOffer 类型串应为 insight_offer，实际 {insight_label}"
        );
        assert_ne!(
            insight_label, heart_demon_label,
            "insight_offer 和 heart_demon_offer 类型串绝不能相同（串台会导致错误 handler）"
        );
    }

    /// effect_kind 映射覆盖：所有内置变体产出 snake_case 非空串。
    #[test]
    fn effect_kind_str_all_variants_non_empty() {
        let effects = vec![
            InsightEffect::MeridianRate { id: MeridianId::Lung, mul: 1.0 },
            InsightEffect::MeridianForgeDiscount { id: MeridianId::Lung, discount: 0.1 },
            InsightEffect::MeridianOverloadTolerance { id: MeridianId::Lung, add: 0.01 },
            InsightEffect::QiRegenFactor { mul: 1.1 },
            InsightEffect::PurgeEfficiency { color: ColorKind::Sharp, mul: 1.1 },
            InsightEffect::UnfreezeQiMax { mul: 0.9 },
            InsightEffect::ComposureRecover { mul: 1.05 },
            InsightEffect::ComposureShockDiscount { event: "tribulation".to_string(), mul: 0.9 },
            InsightEffect::ComposureImmuneDuringBreakthrough,
            InsightEffect::ColorCapAdd { color: ColorKind::Sharp, add: 0.05 },
            InsightEffect::ChaoticTolerance { add: 0.03 },
            InsightEffect::HunyuanThreshold { mul: 0.95 },
            InsightEffect::NextBreakthroughBonus { add: 0.05 },
            InsightEffect::BreakthroughEventConditionDrop { realm: Realm::Induce },
            InsightEffect::TribulationPredictionWindow,
            InsightEffect::DualForgeDiscount { id: MeridianId::Lung, mul: 0.9 },
            InsightEffect::ColorMaterialAffinity {
                color: ColorKind::Sharp,
                material: "jade".to_string(),
                add: 0.05,
            },
            InsightEffect::ZhenfaConcealment { add: 0.1 },
            InsightEffect::ZhenfaDisenchant { add: 0.1 },
            InsightEffect::VortexBackfireResist { mul: 0.9 },
            InsightEffect::VortexDeltaBonus { add: 0.05 },
            InsightEffect::VortexFlowSpeed { mul: 1.1 },
            InsightEffect::UnlockPractice { name: "wind_step".to_string() },
            InsightEffect::UnlockPerception { kind: "mineral_sense".to_string() },
            InsightEffect::LifespanExtensionEnlightenment,
        ];
        for effect in &effects {
            let kind = insight_effect_kind_str(effect);
            assert!(
                !kind.is_empty(),
                "effect_kind_str 不应为空，effect = {effect:?}"
            );
            assert!(
                !kind.chars().any(|c| c.is_uppercase()),
                "effect_kind_str 应全小写 snake_case，实际 {kind}"
            );
        }
    }
}
