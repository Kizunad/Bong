//! NPC inspect/dialogue/trade C2S 请求分发。
//!
//! 顶层 ingress 负责 decode、版本、预算与 live gate；本模块只接收已经通过这些
//! 门禁的 typed NPC request，并保留原有目标解析、反馈和交易状态检查顺序。
//! 不使用动态 registry、反射、字符串路由或 `&mut World`。

use bevy_ecs::system::SystemParam;
use valence::message::SendMessage;
use valence::prelude::{bevy_ecs, Client, DVec3, Entity, Events, Query, ResMut, Username, With};

use crate::combat::components::{Lifecycle, LifecycleState};
use crate::cultivation::components::{Cultivation, Realm};
use crate::identity::PlayerIdentities;
use crate::inventory::{
    add_item_to_player_inventory, InventoryInstanceIdAllocator, ItemRegistry, PlayerInventory,
};
use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest};
use crate::network::client_request_handler::{dimension_kind_for, CombatRequestParams};
use crate::network::inventory_snapshot_emit::send_inventory_snapshot_to_client;
use crate::network::npc_metadata::{
    display_name as npc_display_name, greeting_text_for_archetype,
    reputation_to_player_score_for_client,
};
use crate::npc::faction::FactionMembership;
use crate::npc::interaction_memory::{
    record_player_npc_interaction, NpcInteractionOutcome, NpcInteractionType,
};
use crate::npc::lifecycle::NpcArchetype;
use crate::npc::spawn::NpcMarker;
use crate::npc::trade::{NpcPlayerReputation, NpcTradeInventory};
use crate::player::state::{canonical_player_id, PlayerState};
use crate::schema::client_request::ClientRequestV1;
use crate::social::components::{faction_for_zone, FactionReputation, FactionReputationTier};
use crate::world::dimension::CurrentDimension;
use crate::world::zone::ZoneRegistry;

type NpcEngagementItem = (
    &'static valence::prelude::Position,
    &'static NpcArchetype,
    Option<&'static FactionMembership>,
    Option<&'static Cultivation>,
    Option<&'static Lifecycle>,
    // plan-territory-v1 P1: per-NPC per-player 信誉度（霸主驻守 rep 加成写入此组件，
    // 这里读取后叠加到 faction baseline，让 dominance rep 真正影响交易价格）。
    Option<&'static NpcPlayerReputation>,
);

#[derive(SystemParam)]
pub(crate) struct NpcEngagementRequestParams<'w, 's> {
    pub npcs: Query<'w, 's, NpcEngagementItem, With<NpcMarker>>,
    pub trade_inventories: Query<'w, 's, &'static NpcTradeInventory, With<NpcMarker>>,
    pub lifecycles: Query<'w, 's, &'static Lifecycle>,
    pub memories: Query<
        'w,
        's,
        &'static mut crate::npc::interaction_memory::NpcMemoryComponent,
        With<NpcMarker>,
    >,
    pub positions: Query<'w, 's, &'static valence::prelude::Position>,
    pub dimensions: Query<'w, 's, &'static CurrentDimension>,
    pub identities: Query<'w, 's, &'static PlayerIdentities, With<Client>>,
    pub faction_reputations: Query<'w, 's, &'static FactionReputation, With<Client>>,
    pub audio_events: Option<ResMut<'w, Events<PlaySoundRecipeRequest>>>,
}

/// 已经通过总 schema、version 和 live gate 的 NPC 请求。
///
/// 这是编译期路由面：每个 schema variant 有唯一 Rust variant，不存在字符串 handler
/// registry、反射或动态路由。
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum NpcRequest {
    Inspect {
        npc_entity_id: i32,
    },
    DialogueChoice {
        npc_entity_id: i32,
        option_id: String,
    },
    Trade {
        npc_entity_id: i32,
        offered_items: Vec<u64>,
        requested_item_id: String,
    },
}

/// 从总的 C2S schema enum 中取出 NPC 域；非 NPC 请求原样交还顶层 handler。
pub(crate) fn try_into_npc_request(
    request: ClientRequestV1,
) -> Result<NpcRequest, ClientRequestV1> {
    match request {
        ClientRequestV1::NpcInspectRequest { npc_entity_id, .. } => {
            Ok(NpcRequest::Inspect { npc_entity_id })
        }
        ClientRequestV1::NpcDialogueChoice {
            npc_entity_id,
            option_id,
            ..
        } => Ok(NpcRequest::DialogueChoice {
            npc_entity_id,
            option_id,
        }),
        ClientRequestV1::NpcTradeRequest {
            npc_entity_id,
            offered_items,
            requested_item_id,
            ..
        } => Ok(NpcRequest::Trade {
            npc_entity_id,
            offered_items,
            requested_item_id,
        }),
        request => Err(request),
    }
}

/// 分发一个 typed NPC 请求。
///
/// 参数只暴露必要的 query/resource 写面；交易库存 mutation 仍在所有目标、生命期、
/// 目录、信誉、库存与骨币检查之后执行，且没有 `&mut World` 逃逸。
#[allow(clippy::too_many_arguments)]
pub(crate) fn dispatch_npc_request(
    request: NpcRequest,
    player: Entity,
    combat_params: &CombatRequestParams<'_, '_>,
    npc_params: &mut NpcEngagementRequestParams<'_, '_>,
    zone_registry: Option<&ZoneRegistry>,
    clients: &mut Query<(&Username, &mut Client)>,
    inventories: &mut Query<&mut PlayerInventory>,
    player_states: &Query<&PlayerState>,
    cultivations: &Query<&Cultivation>,
    item_registry: &ItemRegistry,
    instance_allocator: Option<&mut InventoryInstanceIdAllocator>,
    tick: u64,
) {
    match request {
        NpcRequest::Inspect { npc_entity_id } => {
            let Some(target) = resolve_npc_engagement_target(
                player,
                npc_entity_id,
                combat_params,
                npc_params,
                zone_registry,
            ) else {
                send_npc_interaction_feedback(player, clients, "[NPC] 目标已不在附近，无法查看。");
                return;
            };
            if target.reputation_to_player < -30 {
                emit_npc_refuse_audio(&mut npc_params.audio_events, player, target.position);
            }
            send_npc_interaction_feedback(
                player,
                clients,
                format!("§7[NPC] {}：{}", target.display_name, target.greeting_text),
            );
        }
        NpcRequest::DialogueChoice {
            npc_entity_id,
            option_id,
        } => {
            let Some(target) = resolve_npc_engagement_target(
                player,
                npc_entity_id,
                combat_params,
                npc_params,
                zone_registry,
            ) else {
                send_npc_interaction_feedback(player, clients, "[NPC] 目标已不在附近，无法交谈。");
                return;
            };
            let option = option_id.trim();
            match option {
                "inspect" => send_npc_interaction_feedback(
                    player,
                    clients,
                    format!("§7[NPC] 你端详了一眼 {}。", target.display_name),
                ),
                "trade" if target.can_trade() => send_npc_interaction_feedback(
                    player,
                    clients,
                    format!("§7[NPC] {} 摊开了随身货物。", target.display_name),
                ),
                "leave" => {}
                _ => {
                    emit_npc_refuse_audio(&mut npc_params.audio_events, player, target.position);
                    send_npc_interaction_feedback(
                        player,
                        clients,
                        format!("§c[NPC] {} 不愿回应这个选择。", target.display_name),
                    );
                }
            }
        }
        NpcRequest::Trade {
            npc_entity_id,
            offered_items,
            requested_item_id,
        } => {
            dispatch_npc_trade_request(
                player,
                npc_entity_id,
                offered_items,
                requested_item_id,
                combat_params,
                npc_params,
                zone_registry,
                clients,
                inventories,
                player_states,
                cultivations,
                item_registry,
                instance_allocator,
                tick,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn dispatch_npc_trade_request(
    player: Entity,
    npc_entity_id: i32,
    offered_items: Vec<u64>,
    requested_item_id: String,
    combat_params: &CombatRequestParams<'_, '_>,
    npc_params: &mut NpcEngagementRequestParams<'_, '_>,
    zone_registry: Option<&ZoneRegistry>,
    clients: &mut Query<(&Username, &mut Client)>,
    inventories: &mut Query<&mut PlayerInventory>,
    player_states: &Query<&PlayerState>,
    cultivations: &Query<&Cultivation>,
    item_registry: &ItemRegistry,
    instance_allocator: Option<&mut InventoryInstanceIdAllocator>,
    tick: u64,
) {
    let Some(target) = resolve_npc_engagement_target(
        player,
        npc_entity_id,
        combat_params,
        npc_params,
        zone_registry,
    ) else {
        send_npc_interaction_feedback(player, clients, "[NPC] 目标已不在附近，无法交易。");
        return;
    };
    if !offered_items.is_empty() {
        emit_npc_refuse_audio(&mut npc_params.audio_events, player, target.position);
        send_npc_interaction_feedback(player, clients, "§c[NPC] 当前交易只支持骨币结算。");
        return;
    }
    let Some((template_id, _catalogue_price)) =
        npc_trade_catalog_entry(target.archetype, &requested_item_id)
    else {
        emit_npc_refuse_audio(&mut npc_params.audio_events, player, target.position);
        send_npc_interaction_feedback(
            player,
            clients,
            format!("§c[NPC] {} 没有这件货。", target.display_name),
        );
        return;
    };
    if !target.can_trade() {
        emit_npc_refuse_audio(&mut npc_params.audio_events, player, target.position);
        send_npc_interaction_feedback(
            player,
            clients,
            format!("§c[NPC] {} 不做买卖。", target.display_name),
        );
        return;
    }
    let Ok(trade_inventory) = npc_params.trade_inventories.get(target.entity) else {
        emit_npc_refuse_audio(&mut npc_params.audio_events, player, target.position);
        send_npc_interaction_feedback(
            player,
            clients,
            format!("§c[NPC] {} 当前没有可成交的货物。", target.display_name),
        );
        return;
    };
    let Some(offer) = trade_inventory
        .offers
        .iter()
        .find(|offer| offer.template_id == template_id)
        .cloned()
    else {
        emit_npc_refuse_audio(&mut npc_params.audio_events, player, target.position);
        send_npc_interaction_feedback(
            player,
            clients,
            format!("§c[NPC] {} 当前没有这件货。", target.display_name),
        );
        return;
    };
    let base_price = u64::from(offer.price_bone_coins);
    // P3: 将旧 i32 信誉转为 0.0-1.0 范围用于新定价系统。
    // plan-territory-v1 P1: 叠加 NpcPlayerReputation（霸主驻守 rep 加成写入此组件）。
    // 叠加策略：先取 FactionMembership baseline (i32 → [0,1])，
    // 再加 NpcPlayerReputation 的偏移量（默认 0.5 对应"中立=0 偏移"），
    // 即 delta = npc_rep_score - 0.5，faction_baseline + delta，再 clamp。
    let faction_rep_f32 = ((target.reputation_to_player as f32 + 100.0) / 200.0).clamp(0.0, 1.0);
    let npc_rep_delta = target
        .npc_player_rep
        .as_ref()
        .map(|rep| {
            let player_id = clients
                .get(player)
                .map(|(username, _)| canonical_player_id(username.0.as_str()))
                .unwrap_or_default();
            // NpcPlayerReputation.get() 默认 0.5（中立），
            // 霸主驻守后逼近 0.7+（High tier）。
            // delta = score - 0.5（正 = 比中立好，负 = 比中立差）。
            rep.get(player_id.as_str()) - 0.5
        })
        .unwrap_or(0.0);
    let rep_f32 = (faction_rep_f32 + npc_rep_delta).clamp(0.0, 1.0);
    let rep_tier = crate::npc::trade::RepTier::from_score(rep_f32);
    let eligibility = crate::npc::trade::check_trade_eligibility(rep_tier);
    let price = match eligibility {
        crate::npc::trade::TradeEligibility::Refused => {
            let attack_hint = if rep_f32 <= 0.05 {
                "，已经起了杀心"
            } else {
                ""
            };
            emit_npc_refuse_audio(&mut npc_params.audio_events, player, target.position);
            send_npc_interaction_feedback(
                player,
                clients,
                format!(
                    "§c[NPC] {} 对你充满敌意，拒绝交易{attack_hint}。",
                    target.display_name
                ),
            );
            return;
        }
        crate::npc::trade::TradeEligibility::RefuseRare => {
            // Low 信誉：Rare+（含 Rare/Epic/Legendary/Ancient）直接拒绝；
            // Common/Uncommon 允许，但加 1.3x markup。
            // 阈值注释见 trade.rs RepTier::Low（"加价 + 拒绝稀有品"）。
            let item_rarity = item_registry
                .get(template_id)
                .map(|template| template.rarity)
                .unwrap_or(crate::inventory::ItemRarity::Common);
            if is_rarity_refused_at_low_rep(item_rarity) {
                emit_npc_refuse_audio(&mut npc_params.audio_events, player, target.position);
                send_npc_interaction_feedback(
                    player,
                    clients,
                    format!("§c[NPC] {} 不愿将此物卖给你。", target.display_name),
                );
                return;
            }
            // Common/Uncommon：允许，1.3x 加价
            let config = crate::npc::trade::TradePricingConfig::default();
            (base_price as f64 * config.rep_low_markup as f64)
                .ceil()
                .max(1.0) as u64
        }
        crate::npc::trade::TradeEligibility::Allowed { price_modifier } => {
            (base_price as f64 * price_modifier as f64).ceil().max(1.0) as u64
        }
    };
    let Ok(mut inventory) = inventories.get_mut(player) else {
        send_npc_interaction_feedback(player, clients, "[NPC] 你的行囊尚未就绪，交易失败。");
        return;
    };
    if inventory.bone_coins < price {
        emit_npc_refuse_audio(&mut npc_params.audio_events, player, target.position);
        send_npc_interaction_feedback(
            player,
            clients,
            format!("§c[NPC] 骨币不足，需要 {price} 枚。"),
        );
        return;
    }
    let Some(instance_allocator) = instance_allocator else {
        send_npc_interaction_feedback(player, clients, "§c[NPC] 交易账本未就绪。");
        return;
    };
    if let Err(error) = add_item_to_player_inventory(
        &mut inventory,
        item_registry,
        instance_allocator,
        template_id,
        offer.count,
        tick,
    ) {
        send_npc_interaction_feedback(player, clients, format!("§c[NPC] 交易失败：{error}"));
        return;
    }
    inventory.bone_coins = inventory.bone_coins.saturating_sub(price);
    inventory.revision.0 = inventory.revision.0.saturating_add(1);
    let Ok((username, mut client)) = clients.get_mut(player) else {
        return;
    };
    client.send_chat_message(format!(
        "§a[NPC] 你用 {price} 枚骨币从 {} 手中买下 {} x{}。",
        target.display_name, offer.display_name, offer.count
    ));
    record_player_npc_interaction(
        &mut npc_params.memories,
        &npc_params.lifecycles,
        target.entity,
        player,
        NpcInteractionType::Trade,
        NpcInteractionOutcome::Friendly,
        tick,
    );
    if let (Ok(player_state), Ok(cultivation)) =
        (player_states.get(player), cultivations.get(player))
    {
        send_inventory_snapshot_to_client(
            player,
            &mut client,
            username.0.as_str(),
            &inventory,
            player_state,
            cultivation,
            "npc_trade",
        );
    }
}

#[derive(Debug, Clone)]
struct NpcEngagementTarget {
    entity: Entity,
    archetype: NpcArchetype,
    reputation_to_player: i32,
    faction_reputation_tier: FactionReputationTier,
    display_name: String,
    greeting_text: String,
    position: DVec3,
    /// plan-territory-v1 P1: per-NPC per-player 信誉组件（Optional clone）。
    /// trade handler 读取时传入 player 的 canonical_player_id 叠加到 rep_f32。
    npc_player_rep: Option<NpcPlayerReputation>,
}

impl NpcEngagementTarget {
    fn can_trade(&self) -> bool {
        matches!(self.archetype, NpcArchetype::Rogue | NpcArchetype::Commoner)
            && self.faction_reputation_tier != FactionReputationTier::Wanted
            && self.reputation_to_player >= -30
    }
}

fn resolve_npc_engagement_target(
    player: Entity,
    npc_entity_id: i32,
    combat_params: &CombatRequestParams<'_, '_>,
    npc_params: &NpcEngagementRequestParams<'_, '_>,
    zone_registry: Option<&ZoneRegistry>,
) -> Option<NpcEngagementTarget> {
    let npc = combat_params
        .entity_manager
        .as_deref()
        .and_then(|manager| manager.get_by_id(npc_entity_id))?;
    if dimension_kind_for(&npc_params.dimensions, player)
        != dimension_kind_for(&npc_params.dimensions, npc)
    {
        return None;
    }
    let player_position = npc_params.positions.get(player).ok()?.get();
    let (npc_position, archetype, membership, cultivation, lifecycle, npc_player_rep) =
        npc_params.npcs.get(npc).ok()?;
    if lifecycle.is_some_and(|lifecycle| lifecycle.state == LifecycleState::Terminated) {
        return None;
    }
    let npc_position = npc_position.get();
    const NPC_INTERACTION_MAX_DISTANCE: f64 = 6.0;
    if player_position.distance_squared(npc_position)
        > NPC_INTERACTION_MAX_DISTANCE * NPC_INTERACTION_MAX_DISTANCE
    {
        return None;
    }
    let player_identities = npc_params.identities.get(player).ok();
    let player_faction_reputation = npc_params.faction_reputations.get(player).ok();
    let realm = cultivation
        .map(|cultivation| cultivation.realm)
        .unwrap_or(Realm::Awaken);
    let npc_dimension = dimension_kind_for(&npc_params.dimensions, npc);
    let npc_zone_name = zone_registry
        .and_then(|zones| zones.find_zone(npc_dimension, npc_position))
        .map(|zone| zone.name.as_str());
    let faction_reputation_tier = player_faction_reputation
        .and_then(|reputation| npc_zone_name.map(|zone| reputation.tier_for_zone(zone)))
        .unwrap_or(FactionReputationTier::Normal);
    Some(NpcEngagementTarget {
        entity: npc,
        archetype: *archetype,
        reputation_to_player: reputation_to_player_score_for_npc_zone(
            membership,
            player_identities,
            player_faction_reputation,
            npc_zone_name,
        ),
        faction_reputation_tier,
        display_name: npc_display_name(*archetype, realm, membership),
        greeting_text: greeting_text_for_archetype(*archetype).to_string(),
        position: npc_position,
        // plan-territory-v1 P1: clone 可选信誉组件，trade handler 中叠加霸主 rep 加成。
        npc_player_rep: npc_player_rep.cloned(),
    })
}

pub(crate) fn reputation_to_player_score_for_npc_zone(
    membership: Option<&FactionMembership>,
    player_identities: Option<&PlayerIdentities>,
    faction_reputation: Option<&FactionReputation>,
    zone_name: Option<&str>,
) -> i32 {
    let Some(faction_score) = faction_reputation.and_then(|reputation| {
        zone_name
            .and_then(faction_for_zone)
            .map(|faction| reputation.score(faction))
    }) else {
        return reputation_to_player_score_for_client(membership, player_identities);
    };
    let faction_baseline = membership
        .map(crate::network::npc_metadata::reputation_to_player_score)
        .unwrap_or_default();
    faction_baseline
        .saturating_add(faction_score)
        .clamp(-100, 100)
}

pub(crate) fn npc_trade_catalog_entry(
    archetype: NpcArchetype,
    requested_item_id: &str,
) -> Option<(&'static str, u64)> {
    match (archetype, requested_item_id.trim()) {
        (NpcArchetype::Commoner, "lingcao" | "spirit_grass") => Some(("spirit_grass", 10)),
        (NpcArchetype::Rogue, "lingcao" | "spirit_grass") => Some(("spirit_grass", 10)),
        (NpcArchetype::Rogue, "fragment_scroll" | "broken_artifact_scroll") => {
            Some(("broken_artifact_scroll", 40))
        }
        (NpcArchetype::Rogue, "skill_scroll_herbalism_baicao_can") => {
            Some(("skill_scroll_herbalism_baicao_can", 30))
        }
        // plan-cultivation-pacing-v1 P2.2：NPC 售卖低品质修炼丹药。
        // Commoner/Rogue 均可购买次品灵息丸（8 骨币）和次品聚灵丹（15 骨币），
        // 效果 ×0.6，引导玩家自炼正品。
        (
            NpcArchetype::Commoner | NpcArchetype::Rogue,
            "ling_xi_wan_flawed" | "ling_xi_wan_次品",
        ) => Some(("ling_xi_wan_flawed", 8)),
        (
            NpcArchetype::Commoner | NpcArchetype::Rogue,
            "ju_ling_dan_flawed" | "ju_ling_dan_次品",
        ) => Some(("ju_ling_dan_flawed", 15)),
        _ => None,
    }
}

pub(crate) fn is_rarity_refused_at_low_rep(rarity: crate::inventory::ItemRarity) -> bool {
    matches!(
        rarity,
        crate::inventory::ItemRarity::Rare
            | crate::inventory::ItemRarity::Epic
            | crate::inventory::ItemRarity::Legendary
            | crate::inventory::ItemRarity::Ancient
    )
}

fn send_npc_interaction_feedback(
    player: Entity,
    clients: &mut Query<(&Username, &mut Client)>,
    message: impl Into<String>,
) {
    let Ok((_, mut client)) = clients.get_mut(player) else {
        return;
    };
    client.send_chat_message(message.into());
}

fn emit_npc_refuse_audio(
    audio_events: &mut Option<ResMut<Events<PlaySoundRecipeRequest>>>,
    player: Entity,
    position: DVec3,
) {
    let Some(audio_events) = audio_events.as_mut() else {
        return;
    };
    audio_events.send(PlaySoundRecipeRequest {
        recipe_id: "npc_refuse".to_string(),
        instance_id: 0,
        pos: Some([
            position.x.floor() as i32,
            position.y.floor() as i32,
            position.z.floor() as i32,
        ]),
        flag: None,
        volume_mul: 1.0,
        pitch_shift: 0.0,
        recipient: AudioRecipient::Single(player),
    });
}

#[cfg(test)]
mod typed_route_tests {
    use super::*;

    #[test]
    fn typed_route_preserves_all_npc_request_fields() {
        let inspect = try_into_npc_request(ClientRequestV1::NpcInspectRequest {
            v: 1,
            npc_entity_id: 42,
        });
        assert_eq!(
            inspect.ok(),
            Some(NpcRequest::Inspect { npc_entity_id: 42 }),
            "inspect 应进入编译期 NPC route 并保留 protocol entity id"
        );

        let dialogue = try_into_npc_request(ClientRequestV1::NpcDialogueChoice {
            v: 1,
            npc_entity_id: 42,
            option_id: " trade ".to_string(),
        });
        assert_eq!(
            dialogue.ok(),
            Some(NpcRequest::DialogueChoice {
                npc_entity_id: 42,
                option_id: " trade ".to_string(),
            }),
            "dialogue typed route 应保留原始 option_id，trim 语义仍由 dispatcher 决定"
        );

        let trade = try_into_npc_request(ClientRequestV1::NpcTradeRequest {
            v: 1,
            npc_entity_id: 42,
            offered_items: vec![1001, 1002],
            requested_item_id: "spirit_grass".to_string(),
        });
        assert_eq!(
            trade.ok(),
            Some(NpcRequest::Trade {
                npc_entity_id: 42,
                offered_items: vec![1001, 1002],
                requested_item_id: "spirit_grass".to_string(),
            }),
            "trade typed route 应完整传递 offered_items 与 requested_item_id"
        );
    }

    #[test]
    fn typed_route_returns_non_npc_request_to_parent() {
        let request = ClientRequestV1::BreakthroughRequest { v: 1 };
        assert!(
            matches!(
                try_into_npc_request(request),
                Err(ClientRequestV1::BreakthroughRequest { v: 1 })
            ),
            "非 NPC 请求必须原样交还顶层 handler，不能被字符串或动态 registry 吞掉"
        );
    }
}

#[cfg(test)]
mod named_faction_reputation_tests {
    use super::*;
    use crate::npc::faction::{FactionId, FactionRank, MissionQueue, NamedFactionId, Reputation};

    fn membership_with_loyalty(loyalty: f64) -> FactionMembership {
        FactionMembership {
            faction_id: FactionId::Neutral,
            rank: FactionRank::Disciple,
            reputation: Reputation { loyalty },
            lineage: None,
            mission_queue: MissionQueue::default(),
        }
    }

    #[test]
    fn npc_zone_faction_reputation_replaces_global_identity_renown() {
        let mut identities = PlayerIdentities::with_default("Azure", 0);
        identities.active_mut().unwrap().renown.notoriety = 80;
        let mut faction_reputation = FactionReputation::default();
        faction_reputation.apply_delta(NamedFactionId::QingyunHunters, 60);

        let score = reputation_to_player_score_for_npc_zone(
            None,
            Some(&identities),
            Some(&faction_reputation),
            Some("qingyun_peaks"),
        );

        assert_eq!(
            score, 60,
            "青云 zone NPC 应读取 QingyunHunters per_faction 信誉，而不是全局 identity Renown"
        );
    }

    #[test]
    fn npc_zone_faction_reputation_falls_back_to_identity_for_unknown_zone() {
        let mut identities = PlayerIdentities::with_default("Azure", 0);
        identities.active_mut().unwrap().renown.notoriety = 80;
        let mut faction_reputation = FactionReputation::default();
        faction_reputation.apply_delta(NamedFactionId::QingyunHunters, 60);

        let score = reputation_to_player_score_for_npc_zone(
            None,
            Some(&identities),
            Some(&faction_reputation),
            Some("spawn"),
        );

        assert_eq!(
            score, -80,
            "未映射到具名势力的 zone 应保持 legacy identity Renown fallback"
        );
    }

    #[test]
    fn npc_zone_faction_reputation_falls_back_when_zone_or_reputation_missing() {
        let mut identities = PlayerIdentities::with_default("Azure", 0);
        identities.active_mut().unwrap().renown.notoriety = 40;
        let mut faction_reputation = FactionReputation::default();
        faction_reputation.apply_delta(NamedFactionId::QingyunHunters, 60);

        let missing_zone_score = reputation_to_player_score_for_npc_zone(
            None,
            Some(&identities),
            Some(&faction_reputation),
            None,
        );
        let missing_reputation_score = reputation_to_player_score_for_npc_zone(
            None,
            Some(&identities),
            None,
            Some("qingyun_peaks"),
        );
        let empty_score = reputation_to_player_score_for_npc_zone(None, None, None, None);

        assert_eq!(
            missing_zone_score, -40,
            "zone_name=None 时必须回退 legacy identity reputation，避免误读具名势力信誉"
        );
        assert_eq!(
            missing_reputation_score, -40,
            "玩家缺少 FactionReputation 组件时必须回退 legacy identity reputation"
        );
        assert_eq!(
            empty_score, 0,
            "缺少 membership/identity/faction reputation 的空输入应保持中立 0"
        );
    }

    #[test]
    fn npc_zone_faction_reputation_clamps_membership_plus_faction_score() {
        let high_membership = membership_with_loyalty(1.0);
        let low_membership = membership_with_loyalty(0.0);
        let medium_membership = membership_with_loyalty(0.245);
        let mut high_reputation = FactionReputation::default();
        high_reputation.apply_delta(NamedFactionId::QingyunHunters, 1);
        let mut low_reputation = FactionReputation::default();
        low_reputation.apply_delta(NamedFactionId::QingyunHunters, -1);
        let mut off_by_one_reputation = FactionReputation::default();
        off_by_one_reputation.apply_delta(NamedFactionId::QingyunHunters, 50);

        let upper = reputation_to_player_score_for_npc_zone(
            Some(&high_membership),
            None,
            Some(&high_reputation),
            Some("qingyun_peaks"),
        );
        let lower = reputation_to_player_score_for_npc_zone(
            Some(&low_membership),
            None,
            Some(&low_reputation),
            Some("qingyun_peaks"),
        );
        let off_by_one = reputation_to_player_score_for_npc_zone(
            Some(&medium_membership),
            None,
            Some(&off_by_one_reputation),
            Some("qingyun_peaks"),
        );

        assert_eq!(
            upper, 100,
            "membership baseline + faction score 超过上界时必须 clamp 到 100"
        );
        assert_eq!(
            lower, -100,
            "membership baseline + faction score 低于下界时必须 clamp 到 -100"
        );
        assert_eq!(
            off_by_one, -1,
            "未触及边界的 membership baseline + faction score 不应被误 clamp"
        );
    }

    #[test]
    fn wanted_tier_blocks_trade_even_when_score_would_otherwise_allow() {
        let target = NpcEngagementTarget {
            entity: Entity::PLACEHOLDER,
            archetype: NpcArchetype::Commoner,
            reputation_to_player: 100,
            faction_reputation_tier: FactionReputationTier::Wanted,
            display_name: "青云残峰散修".to_string(),
            greeting_text: String::new(),
            position: DVec3::ZERO,
            npc_player_rep: None,
        };

        assert!(
            !target.can_trade(),
            "Wanted tier 必须优先阻断交易，即使 reputation_to_player 分数本身足够高"
        );
    }
}

#[cfg(test)]
mod npc_flawed_pill_trade_tests {
    use super::*;
    use crate::npc::lifecycle::NpcArchetype;

    #[test]
    fn commoner_sells_flawed_ling_xi_wan_at_8_bones() {
        let result = npc_trade_catalog_entry(NpcArchetype::Commoner, "ling_xi_wan_flawed");
        assert_eq!(
            result,
            Some(("ling_xi_wan_flawed", 8)),
            "Commoner 应以 8 骨币售卖次品灵息丸"
        );
    }

    #[test]
    fn commoner_sells_flawed_ju_ling_dan_at_15_bones() {
        let result = npc_trade_catalog_entry(NpcArchetype::Commoner, "ju_ling_dan_flawed");
        assert_eq!(
            result,
            Some(("ju_ling_dan_flawed", 15)),
            "Commoner 应以 15 骨币售卖次品聚灵丹"
        );
    }

    #[test]
    fn rogue_sells_flawed_ling_xi_wan_at_8_bones() {
        let result = npc_trade_catalog_entry(NpcArchetype::Rogue, "ling_xi_wan_flawed");
        assert_eq!(
            result,
            Some(("ling_xi_wan_flawed", 8)),
            "Rogue 也应以 8 骨币售卖次品灵息丸"
        );
    }

    #[test]
    fn rogue_sells_flawed_ju_ling_dan_at_15_bones() {
        let result = npc_trade_catalog_entry(NpcArchetype::Rogue, "ju_ling_dan_flawed");
        assert_eq!(
            result,
            Some(("ju_ling_dan_flawed", 15)),
            "Rogue 也应以 15 骨币售卖次品聚灵丹"
        );
    }

    #[test]
    fn chinese_alias_also_resolves_for_commoner() {
        assert_eq!(
            npc_trade_catalog_entry(NpcArchetype::Commoner, "ling_xi_wan_次品"),
            Some(("ling_xi_wan_flawed", 8)),
            "中文别名 ling_xi_wan_次品 应解析到同一物品"
        );
        assert_eq!(
            npc_trade_catalog_entry(NpcArchetype::Commoner, "ju_ling_dan_次品"),
            Some(("ju_ling_dan_flawed", 15)),
            "中文别名 ju_ling_dan_次品 应解析到同一物品"
        );
    }

    #[test]
    fn beast_does_not_sell_flawed_pills() {
        assert!(
            npc_trade_catalog_entry(NpcArchetype::Beast, "ling_xi_wan_flawed").is_none(),
            "Beast 不应售卖次品丹药"
        );
    }

    #[test]
    fn zombie_does_not_sell_flawed_pills() {
        assert!(
            npc_trade_catalog_entry(NpcArchetype::Zombie, "ling_xi_wan_flawed").is_none(),
            "Zombie 不应售卖次品丹药"
        );
    }

    #[test]
    fn normal_pills_not_in_npc_catalog() {
        assert!(
            npc_trade_catalog_entry(NpcArchetype::Commoner, "ling_xi_wan").is_none(),
            "正品灵息丸不应在 NPC 交易目录中"
        );
        assert!(
            npc_trade_catalog_entry(NpcArchetype::Commoner, "ju_ling_dan").is_none(),
            "正品聚灵丹不应在 NPC 交易目录中"
        );
    }

    #[test]
    fn higher_pills_not_in_npc_catalog() {
        assert!(
            npc_trade_catalog_entry(NpcArchetype::Commoner, "tong_mai_san_flawed").is_none(),
            "通脉散以上 NPC 不售卖"
        );
        assert!(
            npc_trade_catalog_entry(NpcArchetype::Rogue, "xi_sui_ye_flawed").is_none(),
            "洗髓液以上 NPC 不售卖"
        );
    }

    #[test]
    fn buy_path_spirit_grass_price_10() {
        let result = npc_trade_catalog_entry(NpcArchetype::Commoner, "spirit_grass");
        assert_eq!(
            result,
            Some(("spirit_grass", 10)),
            "买路 spirit_grass 应以 10 骨币售卖（与 TRADE_CATALOGUE 对齐），期望: Some((\"spirit_grass\", 10))，实际: {:?}",
            result
        );
    }

    #[test]
    fn buy_path_broken_artifact_scroll_price_40() {
        let result = npc_trade_catalog_entry(NpcArchetype::Rogue, "broken_artifact_scroll");
        assert_eq!(
            result,
            Some(("broken_artifact_scroll", 40)),
            "买路 broken_artifact_scroll 应以 40 骨币售卖（与 TRADE_CATALOGUE 对齐），期望: Some((\"broken_artifact_scroll\", 40))，实际: {:?}",
            result
        );
    }
}

#[cfg(test)]
mod refuse_rare_rarity_gate_tests {
    use super::*;
    use crate::inventory::ItemRarity;

    #[test]
    fn rare_rarity_is_refused_for_low_rep() {
        assert!(
            is_rarity_refused_at_low_rep(ItemRarity::Rare),
            "ItemRarity::Rare 应触发 RefuseRare 拒绝门控，期望为 true"
        );
    }

    #[test]
    fn common_rarity_allowed_for_low_rep_with_markup() {
        assert!(
            !is_rarity_refused_at_low_rep(ItemRarity::Common),
            "ItemRarity::Common 不应触发 RefuseRare 门控，期望为 false"
        );
    }

    #[test]
    fn uncommon_rarity_is_allowed_off_by_one_boundary() {
        assert!(
            !is_rarity_refused_at_low_rep(ItemRarity::Uncommon),
            "ItemRarity::Uncommon 是 Rare 阈值 off-by-one 边界，应允许 1.3x markup"
        );
    }

    #[test]
    fn epic_legendary_ancient_all_refused() {
        assert!(is_rarity_refused_at_low_rep(ItemRarity::Epic));
        assert!(is_rarity_refused_at_low_rep(ItemRarity::Legendary));
        assert!(is_rarity_refused_at_low_rep(ItemRarity::Ancient));
    }

    #[test]
    fn high_mid_rep_not_refused_by_eligibility() {
        use crate::npc::trade::{check_trade_eligibility, RepTier, TradeEligibility};
        assert!(matches!(
            check_trade_eligibility(RepTier::High),
            TradeEligibility::Allowed { .. }
        ));
        assert!(matches!(
            check_trade_eligibility(RepTier::Mid),
            TradeEligibility::Allowed { .. }
        ));
    }

    #[test]
    fn hostile_rep_is_fully_refused_not_rare_gated() {
        use crate::npc::trade::{check_trade_eligibility, RepTier, TradeEligibility};
        assert_eq!(
            check_trade_eligibility(RepTier::Hostile),
            TradeEligibility::Refused
        );
    }

    #[test]
    fn low_rep_eligibility_is_refuse_rare() {
        use crate::npc::trade::{check_trade_eligibility, RepTier, TradeEligibility};
        assert_eq!(
            check_trade_eligibility(RepTier::Low),
            TradeEligibility::RefuseRare
        );
    }

    #[test]
    fn full_refuse_rare_chain_rare_item_low_rep_refused() {
        use crate::npc::trade::{check_trade_eligibility, RepTier, TradeEligibility};
        let eligibility = check_trade_eligibility(RepTier::Low);
        assert_eq!(eligibility, TradeEligibility::RefuseRare);
        assert!(is_rarity_refused_at_low_rep(ItemRarity::Rare));
    }

    #[test]
    fn full_refuse_rare_chain_common_item_low_rep_allowed() {
        use crate::npc::trade::{check_trade_eligibility, RepTier, TradeEligibility};
        let eligibility = check_trade_eligibility(RepTier::Low);
        assert_eq!(eligibility, TradeEligibility::RefuseRare);
        assert!(!is_rarity_refused_at_low_rep(ItemRarity::Common));
        let config = crate::npc::trade::TradePricingConfig::default();
        let final_price = (10_f64 * config.rep_low_markup as f64).ceil().max(1.0) as u64;
        assert_eq!(final_price, 13);
    }
}
