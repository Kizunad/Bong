//! C2S scroll/technique-learning typed dispatch.
//!
//! The ingress system owns channel filtering, decoding, version checks and live
//! gates. This module owns only the two learning-scroll variants and keeps the
//! existing craft-first fallback and learning helpers together with their
//! domain route.

use valence::prelude::{Client, Entity, Events, Query, Username};

use crate::cultivation::components::MeridianSystem;
use crate::cultivation::technique_scroll::{
    can_learn_technique, learn_technique_if_allowed, LearnSource, ScrollReadOutcome,
    TechniqueLearnedEvent, TechniqueScrollReadEvent,
};
use crate::inventory::{
    consume_item_instance_once, inventory_item_by_instance_borrow, InventoryMoveRejectReason,
    ItemCategory, ItemRegistry, ItemTemplate, PlayerInventory,
};
use crate::network::client_request_handler::{resync_snapshot, SkillScrollRequestParams};
use crate::network::inventory_move_rejected_emit::emit_inventory_move_rejected;
use crate::network::inventory_snapshot_emit::send_inventory_snapshot_to_client;
use crate::network::techniques_snapshot_emit::send_techniques_snapshot_to_client;
use crate::player::state::{canonical_player_id, PlayerState};
use crate::schema::client_request::ClientRequestV1;
use crate::skill::components::{ScrollId, SkillId};
use crate::skill::events::{SkillScrollUsed, SkillXpGain, XpGainSource};

/// 已通过 schema/version/live gate 的卷轴学习请求。
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ScrollRequest {
    LearnSkillScroll { instance_id: u64 },
    TechniqueScrollUse { instance_id: u64 },
}

/// 从总 C2S enum 提取卷轴/功法学习域；非本域请求原样交还顶层 handler。
pub(crate) fn try_into_scroll_request(
    request: ClientRequestV1,
) -> Result<ScrollRequest, ClientRequestV1> {
    match request {
        ClientRequestV1::LearnSkillScroll { instance_id, .. } => {
            Ok(ScrollRequest::LearnSkillScroll { instance_id })
        }
        ClientRequestV1::TechniqueScrollUse { instance_id, .. } => {
            Ok(ScrollRequest::TechniqueScrollUse { instance_id })
        }
        request => Err(request),
    }
}

/// 分发一个 typed 卷轴学习请求。
///
/// 两个 wire 变体沿用拆分前完全相同的 craft-first fallback：能够解锁丹方
/// 的卷轴先交给 craft consumer，否则再按模板交给技能/功法学习 helper。
#[allow(clippy::too_many_arguments)]
pub(crate) fn dispatch_scroll_request(
    request: ScrollRequest,
    entity: Entity,
    inventories: &mut Query<&mut PlayerInventory>,
    clients: &mut Query<(&Username, &mut Client)>,
    player_states: &Query<&PlayerState>,
    skill_scroll_params: &mut SkillScrollRequestParams<'_, '_>,
    meridians_q: &mut Query<&mut MeridianSystem>,
) {
    let instance_id = match request {
        ScrollRequest::LearnSkillScroll { instance_id }
        | ScrollRequest::TechniqueScrollUse { instance_id } => instance_id,
    };
    if !handle_craft_recipe_scroll(
        entity,
        instance_id,
        inventories,
        clients,
        &skill_scroll_params.item_registry,
        CraftRecipeScrollParams {
            registry: skill_scroll_params.craft_registry.as_deref(),
            unlock_state: skill_scroll_params.craft_unlock_state.as_deref_mut(),
            unlock_tx: skill_scroll_params.craft_unlock_tx.as_deref_mut(),
        },
    ) {
        handle_learn_skill_scroll(
            entity,
            instance_id,
            inventories,
            clients,
            player_states,
            skill_scroll_params,
            meridians_q,
        );
    }
}

struct CraftRecipeScrollParams<'a> {
    registry: Option<&'a crate::craft::CraftRegistry>,
    unlock_state: Option<&'a mut crate::craft::RecipeUnlockState>,
    unlock_tx: Option<&'a mut Events<crate::craft::CraftUnlockIntent>>,
}

fn handle_craft_recipe_scroll(
    entity: Entity,
    instance_id: u64,
    inventories: &mut Query<&mut PlayerInventory>,
    clients: &mut Query<(&Username, &mut Client)>,
    item_registry: &ItemRegistry,
    craft: CraftRecipeScrollParams<'_>,
) -> bool {
    let (Some(craft_registry), Some(craft_unlock_state), Some(craft_unlock_tx)) =
        (craft.registry, craft.unlock_state, craft.unlock_tx)
    else {
        return false;
    };
    let Some(template_id) = inventories
        .get(entity)
        .ok()
        .and_then(|inventory| inventory_item_by_instance_borrow(inventory, instance_id))
        .map(|instance| instance.template_id.clone())
    else {
        return false;
    };
    let Some(template) = item_registry.get(&template_id) else {
        return false;
    };
    if template.category != ItemCategory::Scroll {
        return false;
    }
    let Ok((username, _)) = clients.get_mut(entity) else {
        return false;
    };
    let player_id = canonical_player_id(username.0.as_str());
    let recipe_ids: Vec<_> =
        crate::craft::unlock::find_recipes_unlockable_by_scroll(craft_registry, &template_id)
            .into_iter()
            .filter(|recipe| craft_unlock_state.reserve_scroll_unlock(&player_id, &recipe.id))
            .map(|recipe| recipe.id.clone())
            .collect();
    if recipe_ids.is_empty() {
        let is_craft_scroll =
            crate::craft::unlock::find_recipes_unlockable_by_scroll(craft_registry, &template_id)
                .into_iter()
                .next()
                .is_some();
        return is_craft_scroll;
    }
    let Ok(mut inventory) = inventories.get_mut(entity) else {
        for recipe_id in &recipe_ids {
            craft_unlock_state.release_scroll_unlock_reservation(&player_id, recipe_id);
        }
        return true;
    };
    if consume_item_instance_once(&mut inventory, instance_id).is_err() {
        for recipe_id in &recipe_ids {
            craft_unlock_state.release_scroll_unlock_reservation(&player_id, recipe_id);
        }
        return true;
    }
    for recipe_id in recipe_ids {
        craft_unlock_tx.send(crate::craft::CraftUnlockIntent {
            caster: entity,
            player_id: player_id.clone(),
            recipe_id,
            source: crate::craft::UnlockEventSource::Scroll {
                item_template: template_id.clone(),
            },
        });
    }
    true
}

fn handle_learn_skill_scroll(
    entity: Entity,
    instance_id: u64,
    inventories: &mut Query<&mut PlayerInventory>,
    clients: &mut Query<(&Username, &mut Client)>,
    player_states: &Query<&PlayerState>,
    skill_scroll_params: &mut SkillScrollRequestParams<'_, '_>,
    meridians_q: &mut Query<&mut MeridianSystem>,
) {
    let Some(template_id) = ({
        let inventory = match inventories.get(entity) {
            Ok(inv) => inv,
            Err(_) => return,
        };
        inventory_item_by_instance_borrow(inventory, instance_id)
            .map(|instance| instance.template_id.clone())
    }) else {
        return;
    };

    if let Some(template) = skill_scroll_params
        .item_registry
        .get(template_id.as_str())
        .cloned()
        .filter(|template| template.technique_scroll_spec.is_some())
    {
        handle_learn_technique_scroll(
            entity,
            instance_id,
            inventories,
            clients,
            player_states,
            skill_scroll_params,
            meridians_q,
            &template,
        );
        return;
    }

    let Some((skill, scroll_id, xp_grant)) = ({
        skill_scroll_spec(template_id.as_str())
            .map(|(skill, xp_grant)| (skill, ScrollId::new(template_id.clone()), xp_grant))
    }) else {
        tracing::warn!(
            "[bong][network][skill] learn_skill_scroll rejected: instance_id={} is not a known skill scroll",
            instance_id
        );
        return;
    };

    let is_duplicate = match skill_scroll_params.skill_sets.get(entity) {
        Ok(skill_set) => skill_set.consumed_scrolls.contains(&scroll_id),
        Err(_) => return,
    };

    if is_duplicate {
        if let Some(skill_scroll_used_tx) = skill_scroll_params.skill_scroll_used_tx.as_deref_mut()
        {
            skill_scroll_used_tx.send(SkillScrollUsed {
                char_entity: entity,
                scroll_id,
                skill,
                xp_granted: 0,
                was_duplicate: true,
            });
        }
        if let Ok(inventory) = inventories.get(entity) {
            resync_snapshot(
                entity,
                inventory,
                clients,
                player_states,
                &skill_scroll_params.cultivations,
                "skill_scroll_duplicate",
            );
        }
        if let Ok((username, mut client)) = clients.get_mut(entity) {
            if let (Ok(skill_set), Ok(cultivation)) = (
                skill_scroll_params.skill_sets.get(entity),
                skill_scroll_params.cultivations.get(entity),
            ) {
                crate::network::skill_snapshot_emit::send_skill_snapshot_to_client(
                    entity,
                    &mut client,
                    username.0.as_str(),
                    skill_set,
                    cultivation,
                    "skill_scroll_duplicate",
                );
            }
        }
        return;
    }

    {
        let Ok(mut inventory) = inventories.get_mut(entity) else {
            return;
        };
        if consume_item_instance_once(&mut inventory, instance_id).is_err() {
            return;
        }
    }

    if let Ok(mut skill_set) = skill_scroll_params.skill_sets.get_mut(entity) {
        skill_set.consumed_scrolls.insert(scroll_id.clone());
    } else {
        return;
    }

    if let Some(skill_xp_tx) = skill_scroll_params.skill_xp_tx.as_deref_mut() {
        skill_xp_tx.send(SkillXpGain {
            char_entity: entity,
            skill,
            amount: xp_grant,
            source: XpGainSource::Scroll {
                scroll_id: scroll_id.clone(),
                xp_grant,
            },
        });
    }
    if let Some(skill_scroll_used_tx) = skill_scroll_params.skill_scroll_used_tx.as_deref_mut() {
        skill_scroll_used_tx.send(SkillScrollUsed {
            char_entity: entity,
            scroll_id,
            skill,
            xp_granted: xp_grant,
            was_duplicate: false,
        });
    }

    let Ok(player_state) = player_states.get(entity) else {
        return;
    };
    let Ok(cultivation) = skill_scroll_params.cultivations.get(entity) else {
        return;
    };
    if let Ok((username, mut client)) = clients.get_mut(entity) {
        if let Ok(inventory) = inventories.get(entity) {
            send_inventory_snapshot_to_client(
                entity,
                &mut client,
                username.0.as_str(),
                inventory,
                player_state,
                cultivation,
                "skill_scroll_consumed",
            );
        }
        if let Ok(skill_set) = skill_scroll_params.skill_sets.get(entity) {
            crate::network::skill_snapshot_emit::send_skill_snapshot_to_client(
                entity,
                &mut client,
                username.0.as_str(),
                skill_set,
                cultivation,
                "skill_scroll_consumed",
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_learn_technique_scroll(
    entity: Entity,
    instance_id: u64,
    inventories: &mut Query<&mut PlayerInventory>,
    clients: &mut Query<(&Username, &mut Client)>,
    player_states: &Query<&PlayerState>,
    skill_scroll_params: &mut SkillScrollRequestParams<'_, '_>,
    meridians_q: &mut Query<&mut MeridianSystem>,
    template: &ItemTemplate,
) {
    let Some(spec) = template.technique_scroll_spec.as_ref() else {
        return;
    };
    let technique_id = spec.skill_id.clone();
    let outcome = {
        let Ok(known) = skill_scroll_params.known_techniques.get(entity) else {
            return;
        };
        let Ok(cultivation) = skill_scroll_params.cultivations.get(entity) else {
            return;
        };
        let Ok(meridians) = meridians_q.get_mut(entity) else {
            return;
        };
        let severed = skill_scroll_params
            .severed_meridians
            .get(entity)
            .ok()
            .flatten();
        let intrinsic_plan = crate::body_plan::resolve_body_plan_for_target(
            entity,
            crate::body_plan::BodyPlanPurpose::Intrinsic,
            crate::body_plan::BodyPlanResolveInputs {
                cultivation: Some(cultivation),
                beast_kind: None,
                morph_state: None,
            },
            skill_scroll_params.body_plans.as_deref(),
            skill_scroll_params.race_registry.as_deref(),
        );
        can_learn_technique(
            &skill_scroll_params.technique_registry,
            known,
            cultivation,
            &meridians,
            severed,
            technique_id.as_str(),
            intrinsic_plan.is_humanoid,
            intrinsic_plan.meridian_profile.as_ref(),
        )
    };

    if matches!(outcome, ScrollReadOutcome::Learned) {
        {
            let Ok(mut inventory) = inventories.get_mut(entity) else {
                return;
            };
            if consume_item_instance_once(&mut inventory, instance_id).is_err() {
                return;
            }
        }

        let learned = {
            let Ok(mut known) = skill_scroll_params.known_techniques.get_mut(entity) else {
                return;
            };
            let Ok(cultivation) = skill_scroll_params.cultivations.get(entity) else {
                return;
            };
            let Ok(meridians) = meridians_q.get_mut(entity) else {
                return;
            };
            let severed = skill_scroll_params
                .severed_meridians
                .get(entity)
                .ok()
                .flatten();
            let intrinsic_plan = crate::body_plan::resolve_body_plan_for_target(
                entity,
                crate::body_plan::BodyPlanPurpose::Intrinsic,
                crate::body_plan::BodyPlanResolveInputs {
                    cultivation: Some(cultivation),
                    beast_kind: None,
                    morph_state: None,
                },
                skill_scroll_params.body_plans.as_deref(),
                skill_scroll_params.race_registry.as_deref(),
            );
            matches!(
                learn_technique_if_allowed(
                    &skill_scroll_params.technique_registry,
                    &mut known,
                    cultivation,
                    &meridians,
                    severed,
                    technique_id.as_str(),
                    0.0,
                    intrinsic_plan.is_humanoid,
                    intrinsic_plan.meridian_profile.as_ref(),
                ),
                ScrollReadOutcome::Learned
            )
        };
        if learned {
            if let Some(tx) = skill_scroll_params.technique_learned_tx.as_deref_mut() {
                tx.send(TechniqueLearnedEvent {
                    player: entity,
                    technique_id: technique_id.clone(),
                    source: LearnSource::Scroll {
                        item_id: template.id.clone(),
                    },
                });
            }
        }
    }

    if let Some(tx) = skill_scroll_params.technique_scroll_read_tx.as_deref_mut() {
        tx.send(TechniqueScrollReadEvent {
            player: entity,
            technique_id: technique_id.clone(),
            source_item: template.id.clone(),
            outcome: outcome.clone(),
        });
    }

    // central-review 2012 #3：拒绝原因必须在 wire 上可观察——只下发不变快照时，
    // client 无法区分「RealmTooLow 拒绝」与「静默忽略/错误原因拒绝」。非习得拒绝
    // 走既有 `InventoryMoveRejectedV1` 契约（reason=realm_too_low / race_mismatch，
    // RealmTooLow 带 required_realm），bot 场景据 reason 断言具体原因。
    if let Some(reject_reason) = match &outcome {
        ScrollReadOutcome::RealmTooLow { required, .. } => {
            Some(InventoryMoveRejectReason::RealmTooLow {
                required_realm: crate::schema::cultivation::realm_to_string(*required).to_string(),
            })
        }
        ScrollReadOutcome::RaceMismatch => Some(InventoryMoveRejectReason::RaceMismatch),
        ScrollReadOutcome::Learned
        | ScrollReadOutcome::AlreadyKnown
        | ScrollReadOutcome::MeridianSevered { .. }
        | ScrollReadOutcome::MeridianMissing { .. }
        | ScrollReadOutcome::FormAnchorClosed
        | ScrollReadOutcome::InvalidScroll => None,
    } {
        emit_inventory_move_rejected(entity, &reject_reason, clients);
    }

    resync_technique_scroll_use(
        entity,
        inventories,
        clients,
        player_states,
        skill_scroll_params,
        match outcome {
            ScrollReadOutcome::Learned => "technique_scroll_learned",
            ScrollReadOutcome::AlreadyKnown => "technique_scroll_already_known",
            ScrollReadOutcome::RealmTooLow { .. } => "technique_scroll_realm_too_low",
            ScrollReadOutcome::RaceMismatch => "technique_scroll_race_mismatch",
            ScrollReadOutcome::MeridianSevered { .. } => "technique_scroll_meridian_severed",
            ScrollReadOutcome::MeridianMissing { .. } => "technique_scroll_meridian_missing",
            ScrollReadOutcome::FormAnchorClosed => "technique_scroll_form_anchor_closed",
            ScrollReadOutcome::InvalidScroll => "technique_scroll_invalid",
        },
    );
}

fn resync_technique_scroll_use(
    entity: Entity,
    inventories: &Query<&mut PlayerInventory>,
    clients: &mut Query<(&Username, &mut Client)>,
    player_states: &Query<&PlayerState>,
    skill_scroll_params: &SkillScrollRequestParams<'_, '_>,
    reason: &str,
) {
    let Ok(player_state) = player_states.get(entity) else {
        return;
    };
    let Ok(cultivation) = skill_scroll_params.cultivations.get(entity) else {
        return;
    };
    let Ok((username, mut client)) = clients.get_mut(entity) else {
        return;
    };
    if let Ok(inventory) = inventories.get(entity) {
        send_inventory_snapshot_to_client(
            entity,
            &mut client,
            username.0.as_str(),
            inventory,
            player_state,
            cultivation,
            reason,
        );
    }
    if let Ok(known) = skill_scroll_params.known_techniques.get(entity) {
        send_techniques_snapshot_to_client(
            &skill_scroll_params.technique_registry,
            entity,
            &mut client,
            username.0.as_str(),
            known,
        );
    }
}

fn skill_scroll_spec(template_id: &str) -> Option<(SkillId, u32)> {
    match template_id {
        "skill_scroll_herbalism_baicao_can" => Some((SkillId::Herbalism, 500)),
        "skill_scroll_alchemy_danhuo_can" => Some((SkillId::Alchemy, 500)),
        "skill_scroll_forging_duantie_can" => Some((SkillId::Forging, 500)),
        _ => None,
    }
}
