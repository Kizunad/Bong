use bevy_ecs::system::SystemParam;
use valence::message::SendMessage;
use valence::prelude::{
    bevy_ecs, Client, DVec3, Entity, Events, Position, Query, ResMut, Username, With,
};

use crate::combat::components::{Lifecycle, LifecycleState};
use crate::cultivation::components::{Cultivation, Realm};
use crate::identity::PlayerIdentities;
use crate::inventory::{
    add_item_to_player_inventory, InventoryInstanceIdAllocator, ItemRegistry, PlayerInventory,
};
use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest};
use crate::network::client_request_handler::CombatRequestParams;
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
use crate::player::state::PlayerState;
use crate::reach::DistanceRule;
use crate::schema::client_request::ClientRequestV1;
use crate::social::components::{faction_for_zone, FactionReputation, FactionReputationTier};
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::zone::ZoneRegistry;

type NpcEngagementItem = (
    &'static Position,
    &'static NpcArchetype,
    Option<&'static FactionMembership>,
    Option<&'static Cultivation>,
    Option<&'static Lifecycle>,
    // plan-territory-v1 P1: per-NPC per-player 信誉度。
    Option<&'static NpcPlayerReputation>,
);

#[derive(SystemParam)]
pub(crate) struct NpcEngagementRequestParams<'w, 's> {
    pub(crate) npcs: Query<'w, 's, NpcEngagementItem, With<NpcMarker>>,
    pub(crate) trade_inventories: Query<'w, 's, &'static NpcTradeInventory, With<NpcMarker>>,
    pub(crate) lifecycles: Query<'w, 's, &'static Lifecycle>,
    pub(crate) memories: Query<
        'w,
        's,
        &'static mut crate::npc::interaction_memory::NpcMemoryComponent,
        With<NpcMarker>,
    >,
    pub(crate) positions: Query<'w, 's, &'static Position>,
    pub(crate) dimensions: Query<'w, 's, &'static CurrentDimension>,
    pub(crate) identities: Query<'w, 's, &'static PlayerIdentities, With<Client>>,
    pub(crate) faction_reputations: Query<'w, 's, &'static FactionReputation, With<Client>>,
    pub(crate) audio_events: Option<ResMut<'w, Events<PlaySoundRecipeRequest>>>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn dispatch<'combat_w, 'combat_s, 'npc_w, 'npc_s, 'allocator_w>(
    request: &ClientRequestV1,
    player: Entity,
    tick: u64,
    combat_params: &CombatRequestParams<'combat_w, 'combat_s>,
    npc_params: &mut NpcEngagementRequestParams<'npc_w, 'npc_s>,
    zone_registry: Option<&ZoneRegistry>,
    clients: &mut Query<(&Username, &mut Client)>,
    inventories: &mut Query<&mut PlayerInventory>,
    player_states: &Query<&PlayerState>,
    cultivations: &Query<&Cultivation>,
    item_registry: &ItemRegistry,
    instance_allocator: &mut Option<ResMut<'allocator_w, InventoryInstanceIdAllocator>>,
) {
    match request {
        ClientRequestV1::NpcInspectRequest { npc_entity_id, .. } => {
            let Some(target) = resolve_npc_engagement_target(
                player,
                *npc_entity_id,
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
        ClientRequestV1::NpcDialogueChoice {
            npc_entity_id,
            option_id,
            ..
        } => {
            let Some(target) = resolve_npc_engagement_target(
                player,
                *npc_entity_id,
                combat_params,
                npc_params,
                zone_registry,
            ) else {
                send_npc_interaction_feedback(player, clients, "[NPC] 目标已不在附近，无法交谈。");
                return;
            };
            match option_id.trim() {
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
        ClientRequestV1::NpcTradeRequest {
            npc_entity_id,
            offered_items,
            requested_item_id,
            ..
        } => {
            handle_trade(
                player,
                *npc_entity_id,
                offered_items,
                requested_item_id,
                tick,
                combat_params,
                npc_params,
                zone_registry,
                clients,
                inventories,
                player_states,
                cultivations,
                item_registry,
                instance_allocator,
            );
        }
        _ => unreachable!("NPC typed route received a non-NPC request"),
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_trade<'combat_w, 'combat_s, 'npc_w, 'npc_s, 'allocator_w>(
    player: Entity,
    npc_entity_id: i32,
    offered_items: &[u64],
    requested_item_id: &str,
    tick: u64,
    combat_params: &CombatRequestParams<'combat_w, 'combat_s>,
    npc_params: &mut NpcEngagementRequestParams<'npc_w, 'npc_s>,
    zone_registry: Option<&ZoneRegistry>,
    clients: &mut Query<(&Username, &mut Client)>,
    inventories: &mut Query<&mut PlayerInventory>,
    player_states: &Query<&PlayerState>,
    cultivations: &Query<&Cultivation>,
    item_registry: &ItemRegistry,
    instance_allocator: &mut Option<ResMut<'allocator_w, InventoryInstanceIdAllocator>>,
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
        npc_trade_catalog_entry(target.archetype, requested_item_id)
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
    let faction_rep_f32 = ((target.reputation_to_player as f32 + 100.0) / 200.0).clamp(0.0, 1.0);
    let npc_rep_delta = target
        .npc_player_rep
        .as_ref()
        .map(|rep| {
            let player_id = clients
                .get(player)
                .map(|(username, _)| canonical_player_id(username.0.as_str()))
                .unwrap_or_default();
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
        send_npc_interaction_feedback(player, clients, "§c[NPC] 你的行囊尚未就绪，交易失败。");
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
    let Some(instance_allocator) = instance_allocator.as_deref_mut() else {
        send_npc_interaction_feedback(player, clients, "[NPC] 交易账本未就绪。");
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
pub(crate) struct NpcEngagementTarget {
    pub(crate) entity: Entity,
    pub(crate) archetype: NpcArchetype,
    pub(crate) reputation_to_player: i32,
    pub(crate) faction_reputation_tier: FactionReputationTier,
    pub(crate) display_name: String,
    pub(crate) greeting_text: String,
    pub(crate) position: DVec3,
    /// plan-territory-v1 P1: per-NPC per-player 信誉组件。
    pub(crate) npc_player_rep: Option<NpcPlayerReputation>,
}

impl NpcEngagementTarget {
    pub(crate) fn can_trade(&self) -> bool {
        matches!(self.archetype, NpcArchetype::Rogue | NpcArchetype::Commoner)
            && self.faction_reputation_tier != FactionReputationTier::Wanted
            && self.reputation_to_player >= -30
    }
}

pub(crate) fn resolve_npc_engagement_target(
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
    if !DistanceRule::nearby_interact().allows(player_position, npc_position) {
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
        npc_player_rep: npc_player_rep.cloned(),
    })
}

fn dimension_kind_for(dimensions: &Query<&CurrentDimension>, entity: Entity) -> DimensionKind {
    dimensions
        .get(entity)
        .map(|dimension| dimension.0)
        .unwrap_or_default()
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

pub(crate) fn is_rarity_refused_at_low_rep(rarity: crate::inventory::ItemRarity) -> bool {
    matches!(
        rarity,
        crate::inventory::ItemRarity::Rare
            | crate::inventory::ItemRarity::Epic
            | crate::inventory::ItemRarity::Legendary
            | crate::inventory::ItemRarity::Ancient
    )
}

fn canonical_player_id(username: &str) -> String {
    crate::player::state::canonical_player_id(username)
}
