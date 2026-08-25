//! C2S Production/Alchemy typed dispatch。
//!
//! 顶层 ingress 负责 channel、budget、decode、version 与 live gate；本模块只把
//! 已经通过这些门禁的十个炼丹请求转换为编译期闭合的领域路由，并复用既有
//! `client_request_handler` helper，保持业务校验和副作用顺序不变。

use valence::prelude::{Client, Commands, Entity, Events, Query, Username};

use crate::alchemy::Intervention;
use crate::combat::CombatClock;
use crate::inventory::PlayerInventory;
use crate::schema::client_request::ClientRequestV1;

#[cfg(test)]
use crate::schema::alchemy::AlchemyInterventionV1;

use crate::network::client_request_handler::{
    AlchemyRequestParams, ClientRequestDispatchParams, CombatRequestParams,
    NpcEngagementRequestParams, SkillScrollRequestParams,
};

/// 已通过 schema/version 校验的 Production/Alchemy 请求。
#[derive(Debug, PartialEq)]
pub(crate) enum ProductionRequest {
    OpenFurnace {
        furnace_pos: (i32, i32, i32),
    },
    FeedSlot {
        furnace_pos: (i32, i32, i32),
        slot_idx: u8,
        material: String,
        count: u32,
    },
    TakeBack {
        furnace_pos: (i32, i32, i32),
        slot_idx: u8,
    },
    Ignite {
        furnace_pos: (i32, i32, i32),
        recipe_id: String,
    },
    Intervention {
        furnace_pos: (i32, i32, i32),
        intervention: Intervention,
    },
    TurnPage {
        delta: i32,
    },
    LearnRecipe {
        recipe_id: String,
    },
    LearnRecipeFragment {
        item_instance_id: u64,
    },
    TakePill {
        pill_item_id: String,
    },
    FurnacePlace {
        pos: (i32, i32, i32),
        item_instance_id: u64,
    },
}

/// 从总 C2S enum 提取 Production/Alchemy 域；非本域请求原样交还顶层 handler。
pub(crate) fn try_into_production_request(
    request: ClientRequestV1,
) -> Result<ProductionRequest, ClientRequestV1> {
    match request {
        ClientRequestV1::AlchemyOpenFurnace { furnace_pos, .. } => {
            Ok(ProductionRequest::OpenFurnace { furnace_pos })
        }
        ClientRequestV1::AlchemyFeedSlot {
            furnace_pos,
            slot_idx,
            material,
            count,
            ..
        } => Ok(ProductionRequest::FeedSlot {
            furnace_pos,
            slot_idx,
            material,
            count,
        }),
        ClientRequestV1::AlchemyTakeBack {
            furnace_pos,
            slot_idx,
            ..
        } => Ok(ProductionRequest::TakeBack {
            furnace_pos,
            slot_idx,
        }),
        ClientRequestV1::AlchemyIgnite {
            furnace_pos,
            recipe_id,
            ..
        } => Ok(ProductionRequest::Ignite {
            furnace_pos,
            recipe_id,
        }),
        ClientRequestV1::AlchemyIntervention {
            furnace_pos,
            intervention,
            ..
        } => Ok(ProductionRequest::Intervention {
            furnace_pos,
            intervention: Intervention::from(intervention),
        }),
        ClientRequestV1::AlchemyTurnPage { delta, .. } => Ok(ProductionRequest::TurnPage { delta }),
        ClientRequestV1::AlchemyLearnRecipe { recipe_id, .. } => {
            Ok(ProductionRequest::LearnRecipe { recipe_id })
        }
        ClientRequestV1::AlchemyLearnRecipeFragment {
            item_instance_id, ..
        } => Ok(ProductionRequest::LearnRecipeFragment { item_instance_id }),
        ClientRequestV1::AlchemyTakePill { pill_item_id, .. } => {
            Ok(ProductionRequest::TakePill { pill_item_id })
        }
        ClientRequestV1::AlchemyFurnacePlace {
            x,
            y,
            z,
            item_instance_id,
            ..
        } => Ok(ProductionRequest::FurnacePlace {
            pos: (x, y, z),
            item_instance_id,
        }),
        request => Err(request),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProductionDispatchOutcome {
    Dispatched,
    DroppedMissingEventResource,
}

fn emit_learn_recipe_fragment(
    player: Entity,
    item_instance_id: u64,
    tx: Option<&mut Events<crate::alchemy::LearnRecipeFragmentIntent>>,
) -> ProductionDispatchOutcome {
    let Some(tx) = tx else {
        tracing::warn!(
            "[bong][network][alchemy] dropped learn_recipe_fragment because LearnRecipeFragmentIntent event resource is missing"
        );
        return ProductionDispatchOutcome::DroppedMissingEventResource;
    };
    tracing::info!(
        "[bong][network][alchemy] learn_recipe_fragment entity={player:?} item_instance_id={item_instance_id}"
    );
    tx.send(crate::alchemy::LearnRecipeFragmentIntent {
        player,
        item_instance_id,
    });
    ProductionDispatchOutcome::Dispatched
}

fn emit_furnace_place(
    player: Entity,
    pos: valence::prelude::BlockPos,
    item_instance_id: u64,
    tx: Option<&mut Events<crate::alchemy::PlaceFurnaceRequest>>,
) -> ProductionDispatchOutcome {
    let Some(tx) = tx else {
        tracing::warn!(
            "[bong][network][alchemy] dropped furnace_place because PlaceFurnaceRequest event resource is missing"
        );
        return ProductionDispatchOutcome::DroppedMissingEventResource;
    };
    tracing::info!(
        "[bong][network][alchemy] furnace_place entity={player:?} pos=[{},{},{}] instance={item_instance_id}",
        pos.x,
        pos.y,
        pos.z,
    );
    tx.send(crate::alchemy::PlaceFurnaceRequest {
        player,
        pos,
        item_instance_id,
    });
    ProductionDispatchOutcome::Dispatched
}

/// 分发一个 typed Production/Alchemy 请求。
///
/// 参数全部是既有 handler 的窄 ECS 查询/resource 面；没有动态 registry 或全局
/// `World` 逃逸。helper 的调用顺序与拆分前保持一致。
#[allow(clippy::too_many_arguments)]
pub(crate) fn dispatch_production_request<
    'alchemy_w,
    'alchemy_s,
    'combat_w,
    'combat_s,
    'dispatch_w,
    'npc_w,
    'npc_s,
    'skill_w,
    'skill_s,
>(
    request: ProductionRequest,
    player: Entity,
    combat_clock: &CombatClock,
    alchemy: &mut AlchemyRequestParams<'alchemy_w, 'alchemy_s>,
    combat: &mut CombatRequestParams<'combat_w, 'combat_s>,
    dispatch: &mut ClientRequestDispatchParams<'dispatch_w>,
    npc: &mut NpcEngagementRequestParams<'npc_w, 'npc_s>,
    skill_scroll: &mut SkillScrollRequestParams<'skill_w, 'skill_s>,
    commands: &mut Commands,
    clients: &mut Query<(&Username, &mut Client)>,
    inventories: &mut Query<&mut PlayerInventory>,
    player_states: &Query<&crate::player::state::PlayerState>,
) -> ProductionDispatchOutcome {
    match request {
        ProductionRequest::OpenFurnace { furnace_pos } => {
            crate::network::client_request_handler::handle_alchemy_open_furnace(
                player,
                furnace_pos,
                clients,
                &mut alchemy.furnaces,
                &mut alchemy.learned,
                &alchemy.recipe_registry,
            );
        }
        ProductionRequest::FeedSlot {
            furnace_pos,
            slot_idx,
            material,
            count,
        } => {
            crate::network::client_request_handler::handle_alchemy_feed_slot(
                player,
                furnace_pos,
                slot_idx,
                material,
                count,
                clients,
                &mut alchemy.furnaces,
                &alchemy.recipe_registry,
                inventories,
                player_states,
                &skill_scroll.cultivations,
                alchemy.zones.as_deref_mut(),
                alchemy.attrition_qi_transfers.as_deref_mut(),
                alchemy.attrition_applied_events.as_deref_mut(),
                alchemy.tsy_lifecycle.as_deref(),
            );
        }
        ProductionRequest::TakeBack {
            furnace_pos,
            slot_idx,
        } => {
            crate::network::client_request_handler::handle_alchemy_take_back(
                player,
                furnace_pos,
                slot_idx,
                combat_clock.tick,
                clients,
                &mut alchemy.furnaces,
                &alchemy.recipe_registry,
                &mut alchemy.outcome_tx,
                inventories,
                player_states,
                &skill_scroll.cultivations,
                &alchemy.item_registry,
                alchemy.instance_allocator.as_deref_mut(),
                alchemy.vfx_events.as_deref_mut(),
            );
        }
        ProductionRequest::Ignite {
            furnace_pos,
            recipe_id,
        } => {
            crate::network::client_request_handler::handle_alchemy_ignite(
                player,
                furnace_pos,
                recipe_id,
                clients,
                &mut alchemy.furnaces,
                &alchemy.recipe_registry,
                alchemy.zones.as_deref(),
                alchemy.redis.as_deref(),
                alchemy.vfx_events.as_deref_mut(),
            );
        }
        ProductionRequest::Intervention {
            furnace_pos,
            intervention,
        } => {
            crate::network::client_request_handler::handle_alchemy_intervention(
                player,
                furnace_pos,
                intervention,
                clients,
                &combat.unique_ids,
                &mut alchemy.furnaces,
                &alchemy.recipe_registry,
                alchemy.zones.as_deref(),
                alchemy.redis.as_deref(),
                alchemy.vfx_events.as_deref_mut(),
            );
        }
        ProductionRequest::TurnPage { delta } => {
            crate::network::client_request_handler::handle_alchemy_turn_page(
                player,
                delta,
                clients,
                &mut alchemy.learned,
                &mut alchemy.state,
            );
        }
        ProductionRequest::LearnRecipe { recipe_id } => {
            crate::network::client_request_handler::handle_alchemy_learn(
                player,
                recipe_id,
                clients,
                &mut alchemy.learned,
                &alchemy.recipe_registry,
            );
        }
        ProductionRequest::LearnRecipeFragment { item_instance_id } => {
            return emit_learn_recipe_fragment(
                player,
                item_instance_id,
                alchemy.learn_fragment_tx.as_deref_mut(),
            );
        }
        ProductionRequest::TakePill { pill_item_id } => {
            crate::network::client_request_handler::handle_alchemy_take_pill(
                player,
                &pill_item_id,
                None,
                commands,
                combat_clock,
                inventories,
                clients,
                player_states,
                &skill_scroll.cultivations,
                combat,
                &mut dispatch.lifespan_extension_tx,
                alchemy.vfx_events.as_deref_mut(),
                &mut npc.audio_events,
                alchemy.hallucination_events.as_deref_mut(),
                alchemy.pending_narrations.as_deref_mut(),
            );
        }
        ProductionRequest::FurnacePlace {
            pos: (x, y, z),
            item_instance_id,
        } => {
            let pos = valence::prelude::BlockPos::new(x, y, z);
            return emit_furnace_place(
                player,
                pos,
                item_instance_id,
                alchemy.place_furnace_tx.as_deref_mut(),
            );
        }
    }
    ProductionDispatchOutcome::Dispatched
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_conversion_preserves_all_alchemy_payloads_and_boundaries() {
        assert_eq!(
            try_into_production_request(ClientRequestV1::AlchemyOpenFurnace {
                v: 1,
                furnace_pos: (i32::MIN, 64, i32::MAX),
            })
            .ok(),
            Some(ProductionRequest::OpenFurnace {
                furnace_pos: (i32::MIN, 64, i32::MAX),
            })
        );
        assert_eq!(
            try_into_production_request(ClientRequestV1::AlchemyFeedSlot {
                v: 1,
                furnace_pos: (1, 2, 3),
                slot_idx: u8::MAX,
                material: String::new(),
                count: u32::MAX,
            })
            .ok(),
            Some(ProductionRequest::FeedSlot {
                furnace_pos: (1, 2, 3),
                slot_idx: u8::MAX,
                material: String::new(),
                count: u32::MAX,
            })
        );
        assert_eq!(
            try_into_production_request(ClientRequestV1::AlchemyTakeBack {
                v: 1,
                furnace_pos: (-1, -2, -3),
                slot_idx: 0,
            })
            .ok(),
            Some(ProductionRequest::TakeBack {
                furnace_pos: (-1, -2, -3),
                slot_idx: 0,
            })
        );
        assert_eq!(
            try_into_production_request(ClientRequestV1::AlchemyIgnite {
                v: 1,
                furnace_pos: (1, 2, 3),
                recipe_id: String::new(),
            })
            .ok(),
            Some(ProductionRequest::Ignite {
                furnace_pos: (1, 2, 3),
                recipe_id: String::new(),
            })
        );

        let intervention = try_into_production_request(ClientRequestV1::AlchemyIntervention {
            v: 1,
            furnace_pos: (4, 5, 6),
            intervention: AlchemyInterventionV1::AdjustTemp { temp: f64::NAN },
        })
        .expect("AlchemyIntervention must enter the typed route");
        assert!(matches!(
            intervention,
            ProductionRequest::Intervention {
                furnace_pos: (4, 5, 6),
                intervention: Intervention::AdjustTemp(value),
            } if value.is_nan()
        ));

        assert_eq!(
            try_into_production_request(ClientRequestV1::AlchemyTurnPage {
                v: 1,
                delta: i32::MIN,
            })
            .ok(),
            Some(ProductionRequest::TurnPage { delta: i32::MIN })
        );
        assert_eq!(
            try_into_production_request(ClientRequestV1::AlchemyLearnRecipe {
                v: 1,
                recipe_id: String::new(),
            })
            .ok(),
            Some(ProductionRequest::LearnRecipe {
                recipe_id: String::new(),
            })
        );
        assert_eq!(
            try_into_production_request(ClientRequestV1::AlchemyLearnRecipeFragment {
                v: 1,
                item_instance_id: u64::MAX,
            })
            .ok(),
            Some(ProductionRequest::LearnRecipeFragment {
                item_instance_id: u64::MAX,
            })
        );
        assert_eq!(
            try_into_production_request(ClientRequestV1::AlchemyTakePill {
                v: 1,
                pill_item_id: String::new(),
            })
            .ok(),
            Some(ProductionRequest::TakePill {
                pill_item_id: String::new(),
            })
        );
        assert_eq!(
            try_into_production_request(ClientRequestV1::AlchemyFurnacePlace {
                v: 1,
                x: i32::MIN,
                y: 0,
                z: i32::MAX,
                item_instance_id: u64::MAX,
            })
            .ok(),
            Some(ProductionRequest::FurnacePlace {
                pos: (i32::MIN, 0, i32::MAX),
                item_instance_id: u64::MAX,
            })
        );
    }

    #[test]
    fn non_alchemy_request_is_returned_unchanged() {
        let request = ClientRequestV1::BreakthroughRequest { v: 1 };
        assert!(matches!(
            try_into_production_request(request),
            Err(ClientRequestV1::BreakthroughRequest { v: 1 })
        ));
    }

    #[test]
    fn event_payloads_preserve_player_fields_and_fail_closed_without_resources() {
        let player = Entity::from_raw(17);

        let mut fragment_events = Events::<crate::alchemy::LearnRecipeFragmentIntent>::default();
        assert_eq!(
            emit_learn_recipe_fragment(player, u64::MAX, Some(&mut fragment_events)),
            ProductionDispatchOutcome::Dispatched
        );
        let fragments: Vec<_> = fragment_events.drain().collect();
        assert_eq!(fragments.len(), 1, "fragment dispatch must emit one intent");
        assert_eq!(fragments[0].player, player);
        assert_eq!(fragments[0].item_instance_id, u64::MAX);

        let mut missing_fragment_events =
            Events::<crate::alchemy::LearnRecipeFragmentIntent>::default();
        assert_eq!(
            emit_learn_recipe_fragment(player, 1, None),
            ProductionDispatchOutcome::DroppedMissingEventResource
        );
        assert!(
            missing_fragment_events.drain().next().is_none(),
            "missing fragment event resource must not create a fallback event"
        );

        let pos = valence::prelude::BlockPos::new(i32::MIN, 0, i32::MAX);
        let mut furnace_events = Events::<crate::alchemy::PlaceFurnaceRequest>::default();
        assert_eq!(
            emit_furnace_place(player, pos, u64::MAX, Some(&mut furnace_events)),
            ProductionDispatchOutcome::Dispatched
        );
        let furnaces: Vec<_> = furnace_events.drain().collect();
        assert_eq!(furnaces.len(), 1, "furnace place must emit one request");
        assert_eq!(furnaces[0].player, player);
        assert_eq!(furnaces[0].pos, pos);
        assert_eq!(furnaces[0].item_instance_id, u64::MAX);

        let mut missing_furnace_events = Events::<crate::alchemy::PlaceFurnaceRequest>::default();
        assert_eq!(
            emit_furnace_place(player, pos, 1, None),
            ProductionDispatchOutcome::DroppedMissingEventResource
        );
        assert!(
            missing_furnace_events.drain().next().is_none(),
            "missing furnace event resource must not create a fallback event"
        );
    }
}
