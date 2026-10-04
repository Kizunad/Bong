//! Staged inventory transaction contracts for RF-33 R10 P1 and RF-34 R10 P2a.
//!
//! The transaction keeps the same validation order while its spill path uses the bounded
//! dropped-loot writer introduced by R10 P2a. The terminal-delivery consumer remains a later
//! phase. New callers can exercise the same validation order:
//! clone a staged view, validate every requested change, then commit one inventory revision.
//! A failed operation leaves the caller's inventory and dropped-loot registry untouched.

use crate::schema::inventory::InventoryLocationV1;
use crate::world::dimension::DimensionKind;

use super::capacity::{CapacityError, SpillContext};
use super::{
    attach_at_location, bump_revision, detach_instance, find_first_fit_container_location,
    inventory_item_by_instance_borrow, inventory_item_by_instance_mut,
    inventory_location_by_instance, stack_identity_matches, validate_attach_fits, DroppedLootEntry,
    DroppedLootRegistry, DroppedLootVisibility, DroppedLootWriteError,
    InventoryInstanceIdAllocator, InventoryMoveRejectReason, InventoryRevision, ItemInstance,
    ItemRegistry, ItemTemplate, PlayerInventory,
};

/// One item in a delivery request.  Existing instances are moved without rebuilding their NBT;
/// template deliveries allocate one fresh instance id in the staged allocator.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
pub enum DeliveryItem {
    Template {
        template_id: String,
        stack_count: u32,
        current_tick: u64,
    },
    Existing(ItemInstance),
}

impl DeliveryItem {
    pub fn template(template_id: impl Into<String>, stack_count: u32, current_tick: u64) -> Self {
        Self::Template {
            template_id: template_id.into(),
            stack_count,
            current_tick,
        }
    }

    pub fn existing(item: ItemInstance) -> Self {
        Self::Existing(item)
    }
}

/// Atomic delivery request.  `request_id` is the idempotency/correlation seam owned by R3.
#[derive(Debug, Clone, PartialEq)]
pub struct DeliveryRequest {
    pub request_id: String,
    pub items: Vec<DeliveryItem>,
}

impl DeliveryRequest {
    pub fn new(request_id: impl Into<String>, items: Vec<DeliveryItem>) -> Self {
        Self {
            request_id: request_id.into(),
            items,
        }
    }
}

/// Checked consume request.  A quantity of zero is rejected before touching the staged view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsumeRequest {
    pub request_id: String,
    pub instance_id: u64,
    pub quantity: u32,
}

impl ConsumeRequest {
    pub fn new(request_id: impl Into<String>, instance_id: u64, quantity: u32) -> Self {
        Self {
            request_id: request_id.into(),
            instance_id,
            quantity,
        }
    }
}

/// Pickup request keyed by the authoritative dropped instance id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickupRequest {
    pub request_id: String,
    pub instance_id: u64,
}

impl PickupRequest {
    pub fn new(request_id: impl Into<String>, instance_id: u64) -> Self {
        Self {
            request_id: request_id.into(),
            instance_id,
        }
    }
}

/// Server-resolved pickup facts.  Client coordinates and dimension are never accepted here.
#[derive(Debug, Clone, PartialEq)]
pub struct PickupAuthorization {
    pub player_id: String,
    pub dimension: DimensionKind,
    pub position: [f64; 3],
    pub max_distance: f64,
    /// R4 will fill this from the authoritative metadata projection.  It is kept in the seam
    /// now so owner-only drops cannot later be implemented from client-provided fields.
    pub owner: Option<String>,
    pub server_authorized_private_pickup: bool,
}

impl PickupAuthorization {
    pub fn new(
        player_id: impl Into<String>,
        dimension: DimensionKind,
        position: [f64; 3],
        max_distance: f64,
    ) -> Self {
        Self {
            player_id: player_id.into(),
            dimension,
            position,
            max_distance,
            owner: None,
            server_authorized_private_pickup: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryMergeReceipt {
    pub incoming_instance_id: u64,
    pub target_instance_id: u64,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InventorySpillReceipt {
    pub instance_id: u64,
    pub count: u32,
    pub world_pos: [f64; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct InventoryDeliveryReceipt {
    pub request_id: String,
    pub revision: InventoryRevision,
    pub requested_count: u64,
    pub stored_count: u64,
    pub spilled_count: u64,
    pub created_instance_ids: Vec<u64>,
    pub placed_existing_instance_ids: Vec<u64>,
    pub merges: Vec<InventoryMergeReceipt>,
    pub spills: Vec<InventorySpillReceipt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryConsumeReceipt {
    pub request_id: String,
    pub revision: InventoryRevision,
    pub instance_id: u64,
    pub consumed: u32,
    pub remaining: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InventoryPickupReceipt {
    pub request_id: String,
    pub revision: InventoryRevision,
    pub removed_drop_id: u64,
    pub incoming_instance_id: u64,
    pub incoming_count: u32,
    pub target_instance_id: u64,
    pub target_location: InventoryLocationV1,
    pub merged: bool,
}

/// Failure taxonomy shared by delivery, consume, and pickup.  Every variant is a pre-commit
/// failure except `PersistenceUnavailable`, whose durable seam is still required to be atomic.
#[derive(Debug, Clone, PartialEq)]
pub enum InventoryTxnError {
    InvalidRequest {
        reason: String,
    },
    UnknownTemplate {
        template_id: String,
    },
    UnknownInstance {
        instance_id: u64,
    },
    DuplicateInstanceId {
        instance_id: u64,
    },
    IdentityMismatch {
        instance_id: u64,
    },
    ZeroQuantity,
    Insufficient {
        instance_id: u64,
        available: u32,
        requested: u32,
    },
    InvalidPlacement {
        reason: String,
    },
    Capacity(CapacityError),
    MissingSpillContext,
    MissingDurableSeam,
    Unauthorized,
    WrongDimension,
    OutOfRange {
        distance: f64,
        max_distance: f64,
    },
    PersistenceUnavailable {
        reason: String,
    },
}

impl From<CapacityError> for InventoryTxnError {
    fn from(error: CapacityError) -> Self {
        Self::Capacity(error)
    }
}

/// A staged transaction over one player's inventory.
pub struct InventoryTxn<'a> {
    inventory: &'a mut PlayerInventory,
    registry: &'a ItemRegistry,
    allocator: &'a mut InventoryInstanceIdAllocator,
}

impl<'a> InventoryTxn<'a> {
    pub fn new(
        inventory: &'a mut PlayerInventory,
        registry: &'a ItemRegistry,
        allocator: &'a mut InventoryInstanceIdAllocator,
    ) -> Self {
        Self {
            inventory,
            registry,
            allocator,
        }
    }

    /// Deliver all items atomically.  Existing dynamic fields are moved verbatim; a template is
    /// materialized only in the staged allocator.  If no placement exists, every overflow item
    /// requires a fully populated [`SpillContext`].
    pub fn deliver(
        &mut self,
        request: DeliveryRequest,
        spill: Option<&mut SpillContext<'_>>,
    ) -> Result<InventoryDeliveryReceipt, InventoryTxnError> {
        ensure_request_id(&request.request_id)?;
        if request.items.is_empty() {
            return Err(InventoryTxnError::InvalidRequest {
                reason: "delivery request must contain at least one item".to_string(),
            });
        }

        let mut staged_inventory = self.inventory.clone();
        let mut staged_allocator = self.allocator.clone();
        let mut prepared = Vec::with_capacity(request.items.len());
        let mut created_instance_ids = Vec::new();
        let mut placed_existing_instance_ids = Vec::new();
        let mut requested_count = 0_u64;
        let mut incoming_ids = std::collections::HashSet::new();

        for delivery in request.items {
            let item = match delivery {
                DeliveryItem::Template {
                    template_id,
                    stack_count,
                    current_tick,
                } => {
                    let template = self.registry.get(&template_id).ok_or_else(|| {
                        InventoryTxnError::UnknownTemplate {
                            template_id: template_id.clone(),
                        }
                    })?;
                    validate_template_stack(template, &template_id, stack_count)?;
                    let instance_id = staged_allocator
                        .next_id()
                        .map_err(|reason| InventoryTxnError::InvalidRequest { reason })?;
                    if inventory_item_by_instance_borrow(&staged_inventory, instance_id).is_some() {
                        return Err(InventoryTxnError::DuplicateInstanceId { instance_id });
                    }
                    if !incoming_ids.insert(instance_id) {
                        return Err(InventoryTxnError::DuplicateInstanceId { instance_id });
                    }
                    created_instance_ids.push(instance_id);
                    runtime_instance_from_template_for_txn(
                        template,
                        instance_id,
                        stack_count,
                        current_tick,
                    )
                }
                DeliveryItem::Existing(item) => {
                    validate_existing_item(
                        self.inventory,
                        &staged_inventory,
                        &item,
                        self.registry,
                    )?;
                    if !incoming_ids.insert(item.instance_id) {
                        return Err(InventoryTxnError::DuplicateInstanceId {
                            instance_id: item.instance_id,
                        });
                    }
                    placed_existing_instance_ids.push(item.instance_id);
                    item
                }
            };
            requested_count = requested_count.saturating_add(u64::from(item.stack_count));
            prepared.push(item);
        }

        let mut stored_count = 0_u64;
        let mut spilled_count = 0_u64;
        let mut merges = Vec::new();
        let mut spills = Vec::new();
        let mut spill_entries = Vec::new();

        for item in prepared {
            let item_count = item.stack_count;
            let max_stack_count = self
                .registry
                .get(&item.template_id)
                .map_or(item.stack_count, |template| template.max_stack_count);
            if let Some(merge) = merge_item_if_fits(&mut staged_inventory, &item, max_stack_count) {
                stored_count = stored_count.saturating_add(u64::from(item.stack_count));
                merges.push(merge);
                continue;
            }

            if let Some(location) = find_first_fit_container_location(&staged_inventory, &item) {
                validate_attach_fits(&staged_inventory, &item, &location)
                    .map_err(invalid_placement)?;
                attach_at_location(&mut staged_inventory, item, &location)
                    .map_err(invalid_placement)?;
                stored_count = stored_count.saturating_add(u64::from(item_count));
                continue;
            }

            let Some(spill_context) = spill.as_deref() else {
                return Err(InventoryTxnError::MissingSpillContext);
            };
            let entry = DroppedLootEntry {
                instance_id: item.instance_id,
                source_container_id: spill_context.source_identity.clone(),
                source_row: 0,
                source_col: 0,
                world_pos: spill_context.world_pos,
                dimension: spill_context.dimension,
                owner: None,
                visibility: DroppedLootVisibility::Public,
                item: item.clone(),
            };
            spilled_count = spilled_count.saturating_add(u64::from(item.stack_count));
            spills.push(InventorySpillReceipt {
                instance_id: item.instance_id,
                count: item.stack_count,
                world_pos: spill_context.world_pos,
            });
            spill_entries.push(entry);
        }

        // The staged inventory has already been validated.  Only the durable spill admission
        // remains; it is completed before assigning the staged inventory to the caller.
        if !spill_entries.is_empty() {
            let Some(spill_context) = spill else {
                return Err(InventoryTxnError::MissingSpillContext);
            };
            spill_context
                .validate()
                .map_err(|error| InventoryTxnError::InvalidRequest {
                    reason: format!("invalid spill context: {error:?}"),
                })?;
            let reservation = spill_context
                .capacity
                .reserve(spill_entries.len())
                .map_err(InventoryTxnError::Capacity)?;
            if let Some(duplicate) = spill_entries.iter().find(|entry| {
                spill_context
                    .registry
                    .entries
                    .contains_key(&entry.instance_id)
            }) {
                spill_context.capacity.release(reservation);
                return Err(InventoryTxnError::DuplicateInstanceId {
                    instance_id: duplicate.instance_id,
                });
            }
            let mut staged_registry = spill_context.registry.clone();
            for entry in &spill_entries {
                if let Err(error) = staged_registry.try_insert_public(entry.clone()) {
                    spill_context.capacity.release(reservation);
                    return Err(match error {
                        DroppedLootWriteError::DuplicateInstanceId(instance_id) => {
                            InventoryTxnError::DuplicateInstanceId { instance_id }
                        }
                        DroppedLootWriteError::Capacity(error) => {
                            InventoryTxnError::Capacity(error)
                        }
                        other => InventoryTxnError::PersistenceUnavailable {
                            reason: format!("dropped-loot admission failed: {other:?}"),
                        },
                    });
                }
            }
            if let Err(reason) = spill_context
                .durable
                .persist_batch(&spill_context.transaction_id, &spill_entries)
            {
                spill_context.capacity.release(reservation);
                return Err(InventoryTxnError::PersistenceUnavailable { reason });
            }
            *spill_context.registry = staged_registry;
            spill_context.capacity.commit(reservation);
        }

        if stored_count > 0 {
            bump_revision(&mut staged_inventory);
        }
        *self.inventory = staged_inventory;
        *self.allocator = staged_allocator;

        Ok(InventoryDeliveryReceipt {
            request_id: request.request_id,
            revision: self.inventory.revision,
            requested_count,
            stored_count,
            spilled_count,
            created_instance_ids,
            placed_existing_instance_ids,
            merges,
            spills,
        })
    }

    /// Consume exactly one checked quantity with no partial mutation on failure.
    pub fn consume_checked(
        &mut self,
        request: ConsumeRequest,
    ) -> Result<InventoryConsumeReceipt, InventoryTxnError> {
        ensure_request_id(&request.request_id)?;
        if request.quantity == 0 {
            return Err(InventoryTxnError::ZeroQuantity);
        }
        let mut staged_inventory = self.inventory.clone();
        let available = inventory_item_by_instance_borrow(&staged_inventory, request.instance_id)
            .map(|item| item.stack_count)
            .ok_or(InventoryTxnError::UnknownInstance {
                instance_id: request.instance_id,
            })?;
        if request.quantity > available {
            return Err(InventoryTxnError::Insufficient {
                instance_id: request.instance_id,
                available,
                requested: request.quantity,
            });
        }
        let remaining = {
            let item = inventory_item_by_instance_mut(&mut staged_inventory, request.instance_id)
                .ok_or(InventoryTxnError::UnknownInstance {
                instance_id: request.instance_id,
            })?;
            item.stack_count -= request.quantity;
            item.stack_count
        };
        if remaining == 0 {
            detach_instance(&mut staged_inventory, request.instance_id);
        }
        bump_revision(&mut staged_inventory);
        *self.inventory = staged_inventory;
        Ok(InventoryConsumeReceipt {
            request_id: request.request_id,
            revision: self.inventory.revision,
            instance_id: request.instance_id,
            consumed: request.quantity,
            remaining,
        })
    }

    /// Authorize, stage, attach/merge, and only then remove a dropped entry.
    pub fn pickup_and_merge(
        &mut self,
        request: PickupRequest,
        authorization: PickupAuthorization,
        registry: &mut DroppedLootRegistry,
    ) -> Result<InventoryPickupReceipt, InventoryTxnError> {
        ensure_request_id(&request.request_id)?;
        validate_authorization(&authorization)?;
        let entry = registry.entries.get(&request.instance_id).cloned().ok_or(
            InventoryTxnError::UnknownInstance {
                instance_id: request.instance_id,
            },
        )?;
        if entry.instance_id != request.instance_id {
            return Err(InventoryTxnError::IdentityMismatch {
                instance_id: request.instance_id,
            });
        }
        if entry.dimension != authorization.dimension {
            return Err(InventoryTxnError::WrongDimension);
        }
        let distance = distance_between(entry.world_pos, authorization.position);
        if distance > authorization.max_distance {
            return Err(InventoryTxnError::OutOfRange {
                distance,
                max_distance: authorization.max_distance,
            });
        }
        let template = self.registry.get(&entry.item.template_id).ok_or_else(|| {
            InventoryTxnError::UnknownTemplate {
                template_id: entry.item.template_id.clone(),
            }
        })?;
        validate_template_stack(template, &entry.item.template_id, entry.item.stack_count)?;
        if inventory_item_by_instance_borrow(self.inventory, entry.instance_id).is_some() {
            return Err(InventoryTxnError::IdentityMismatch {
                instance_id: entry.instance_id,
            });
        }

        let mut staged_inventory = self.inventory.clone();
        let (target_instance_id, target_location, merged) = if let Some(target) =
            find_merge_target(&staged_inventory, &entry.item, self.registry)
        {
            let target_instance_id = target.instance_id;
            let location = inventory_location_by_instance(&staged_inventory, target_instance_id)
                .ok_or_else(|| invalid_placement_message("merge target has no location"))?;
            let target_item =
                inventory_item_by_instance_mut(&mut staged_inventory, target_instance_id)
                    .ok_or_else(|| invalid_placement_message("merge target disappeared"))?;
            target_item.stack_count = target_item
                .stack_count
                .saturating_add(entry.item.stack_count);
            (target_instance_id, location, true)
        } else {
            let location = find_first_fit_container_location(&staged_inventory, &entry.item)
                .ok_or_else(|| InventoryTxnError::InvalidPlacement {
                    reason: "no valid inventory placement for dropped item".to_string(),
                })?;
            validate_attach_fits(&staged_inventory, &entry.item, &location)
                .map_err(invalid_placement)?;
            attach_at_location(&mut staged_inventory, entry.item.clone(), &location)
                .map_err(invalid_placement)?;
            (entry.instance_id, location, false)
        };

        bump_revision(&mut staged_inventory);
        registry.entries.remove(&request.instance_id);
        *self.inventory = staged_inventory;
        Ok(InventoryPickupReceipt {
            request_id: request.request_id,
            revision: self.inventory.revision,
            removed_drop_id: request.instance_id,
            incoming_instance_id: entry.instance_id,
            incoming_count: entry.item.stack_count,
            target_instance_id,
            target_location,
            merged,
        })
    }
}

fn ensure_request_id(request_id: &str) -> Result<(), InventoryTxnError> {
    if request_id.trim().is_empty() {
        Err(InventoryTxnError::InvalidRequest {
            reason: "request_id must not be empty".to_string(),
        })
    } else {
        Ok(())
    }
}

fn validate_template_stack(
    template: &ItemTemplate,
    template_id: &str,
    stack_count: u32,
) -> Result<(), InventoryTxnError> {
    if stack_count == 0 {
        return Err(InventoryTxnError::ZeroQuantity);
    }
    if template.max_stack_count == 0 || stack_count > template.max_stack_count {
        return Err(InventoryTxnError::InvalidRequest {
            reason: format!(
                "template `{template_id}` stack_count {stack_count} exceeds max {}",
                template.max_stack_count
            ),
        });
    }
    if template.grid_w == 0 || template.grid_h == 0 {
        return Err(InventoryTxnError::InvalidPlacement {
            reason: format!("template `{template_id}` has a zero footprint"),
        });
    }
    Ok(())
}

fn validate_existing_item(
    original: &PlayerInventory,
    staged: &PlayerInventory,
    item: &ItemInstance,
    registry: &ItemRegistry,
) -> Result<(), InventoryTxnError> {
    if item.instance_id == 0 {
        return Err(InventoryTxnError::InvalidRequest {
            reason: "existing delivery requires a non-zero instance id".to_string(),
        });
    }
    if item.stack_count == 0 {
        return Err(InventoryTxnError::ZeroQuantity);
    }
    let template =
        registry
            .get(&item.template_id)
            .ok_or_else(|| InventoryTxnError::UnknownTemplate {
                template_id: item.template_id.clone(),
            })?;
    validate_template_stack(template, &item.template_id, item.stack_count)?;
    if inventory_item_by_instance_borrow(original, item.instance_id).is_some()
        || inventory_item_by_instance_borrow(staged, item.instance_id).is_some()
    {
        return Err(InventoryTxnError::DuplicateInstanceId {
            instance_id: item.instance_id,
        });
    }
    Ok(())
}

fn merge_item_if_fits(
    inventory: &mut PlayerInventory,
    item: &ItemInstance,
    max_stack_count: u32,
) -> Option<InventoryMergeReceipt> {
    for container in &mut inventory.containers {
        for placed in &mut container.items {
            if stack_identity_matches(&placed.instance, item)
                && placed
                    .instance
                    .stack_count
                    .checked_add(item.stack_count)
                    .is_some_and(|count| count <= max_stack_count)
            {
                let target_instance_id = placed.instance.instance_id;
                placed.instance.stack_count += item.stack_count;
                return Some(InventoryMergeReceipt {
                    incoming_instance_id: item.instance_id,
                    target_instance_id,
                    count: item.stack_count,
                });
            }
        }
    }
    None
}

fn find_merge_target(
    inventory: &PlayerInventory,
    item: &ItemInstance,
    registry: &ItemRegistry,
) -> Option<ItemInstance> {
    let max_stack_count = registry
        .get(&item.template_id)
        .map_or(item.stack_count, |template| template.max_stack_count);
    inventory
        .containers
        .iter()
        .flat_map(|container| container.items.iter())
        .find(|placed| {
            stack_identity_matches(&placed.instance, item)
                && placed
                    .instance
                    .stack_count
                    .checked_add(item.stack_count)
                    .is_some_and(|count| count <= max_stack_count)
        })
        .map(|placed| placed.instance.clone())
}

fn validate_authorization(authorization: &PickupAuthorization) -> Result<(), InventoryTxnError> {
    if authorization.player_id.trim().is_empty()
        || !authorization.max_distance.is_finite()
        || authorization.max_distance < 0.0
        || authorization
            .position
            .iter()
            .any(|value| !value.is_finite())
    {
        return Err(InventoryTxnError::Unauthorized);
    }
    Ok(())
}

fn distance_between(left: [f64; 3], right: [f64; 3]) -> f64 {
    let dx = left[0] - right[0];
    let dy = left[1] - right[1];
    let dz = left[2] - right[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn invalid_placement(reason: InventoryMoveRejectReason) -> InventoryTxnError {
    InventoryTxnError::InvalidPlacement {
        reason: format!("{reason:?}"),
    }
}

fn invalid_placement_message(reason: impl Into<String>) -> InventoryTxnError {
    InventoryTxnError::InvalidPlacement {
        reason: reason.into(),
    }
}

fn runtime_instance_from_template_for_txn(
    template: &ItemTemplate,
    instance_id: u64,
    stack_count: u32,
    current_tick: u64,
) -> ItemInstance {
    super::runtime_instance_from_template(template, instance_id, stack_count, current_tick)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::inventory::{
        capacity::{BoundedCapacityProvider, CapacityProvider, NoopDurableSpill},
        ContainerState, ItemCategory, ItemRarity, PlacedItemState, SlotContents,
    };

    fn item(id: u64, template_id: &str, count: u32) -> ItemInstance {
        ItemInstance {
            instance_id: id,
            template_id: template_id.to_string(),
            display_name: template_id.to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 1.0,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: count,
            spirit_quality: 0.5,
            durability: 0.75,
            freshness: None,
            mineral_id: Some("fan_tie".to_string()),
            charges: Some(3),
            forge_quality: Some(0.4),
            forge_color: None,
            forge_side_effects: vec!["heat".to_string()],
            forge_achieved_tier: Some(2),
            alchemy: None,
            lingering_owner_qi: None,
        }
    }

    fn inventory(rows: u8, cols: u8, items: Vec<PlacedItemState>) -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(7),
            material_preparation: Default::default(),
            containers: vec![ContainerState {
                id: "body_pocket".to_string(),
                name: "暗袋".to_string(),
                rows,
                cols,
                items,
                owner_instance_id: None,
                quick_access: true,
            }],
            equipped: HashMap::<String, SlotContents>::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 15.0,
            triggered_treasures: Vec::new(),
        }
    }

    fn registry() -> ItemRegistry {
        let mut templates = HashMap::new();
        let mut template = ItemTemplate::minimal_for_test("ore");
        template.category = ItemCategory::Mineral;
        template.max_stack_count = 16;
        templates.insert("ore".to_string(), template);
        ItemRegistry::from_map(templates)
    }

    fn spill_context<'a>(
        registry: &'a mut DroppedLootRegistry,
        capacity: &'a mut BoundedCapacityProvider,
        durable: &'a mut NoopDurableSpill,
    ) -> SpillContext<'a> {
        SpillContext {
            source_identity: "player:alice".to_string(),
            source_revision: InventoryRevision(7),
            dimension: DimensionKind::Overworld,
            world_pos: [1.0, 64.0, 2.0],
            registry,
            capacity,
            durable,
            transaction_id: "txn-1".to_string(),
        }
    }

    #[derive(Debug, Default)]
    struct FailingDurableSpill;

    impl super::super::capacity::DurableSpill for FailingDurableSpill {
        fn persist_batch(
            &mut self,
            _transaction_id: &str,
            _entries: &[DroppedLootEntry],
        ) -> Result<(), String> {
            Err("test persistence failure".to_string())
        }
    }

    #[test]
    fn consume_failure_paths_leave_inventory_and_revision_unchanged() {
        let mut inventory = inventory(
            1,
            1,
            vec![PlacedItemState {
                row: 0,
                col: 0,
                instance: item(1, "ore", 2),
            }],
        );
        let before = inventory.clone();
        let registry = registry();
        for request in [
            ConsumeRequest::new("zero", 1, 0),
            ConsumeRequest::new("unknown", 99, 1),
            ConsumeRequest::new("insufficient", 1, 3),
        ] {
            let mut allocator = InventoryInstanceIdAllocator::new(2);
            let failed = {
                let mut txn = InventoryTxn::new(&mut inventory, &registry, &mut allocator);
                txn.consume_checked(request).is_err()
            };
            assert!(failed);
            assert_eq!(inventory.revision, before.revision);
            assert_eq!(inventory.containers, before.containers);
        }
        let mut allocator = InventoryInstanceIdAllocator::new(2);
        let mut txn = InventoryTxn::new(&mut inventory, &registry, &mut allocator);
        let receipt = txn
            .consume_checked(ConsumeRequest::new("ok", 1, 1))
            .expect("one item is consumable");
        assert_eq!(receipt.remaining, 1);
        assert_eq!(inventory.revision, InventoryRevision(8));
    }

    #[test]
    fn deliver_existing_item_preserves_dynamic_identity_and_revision_bumps_once() {
        let mut inventory = inventory(1, 1, Vec::new());
        let registry = registry();
        let mut allocator = InventoryInstanceIdAllocator::new(20);
        let incoming = item(9, "ore", 1);
        let mut txn = InventoryTxn::new(&mut inventory, &registry, &mut allocator);
        let receipt = txn
            .deliver(
                DeliveryRequest::new("deliver", vec![DeliveryItem::existing(incoming.clone())]),
                None,
            )
            .expect("empty slot accepts existing item");
        assert_eq!(receipt.revision, InventoryRevision(8));
        assert_eq!(inventory.containers[0].items[0].instance, incoming);
        assert_eq!(receipt.created_instance_ids, Vec::<u64>::new());
        assert_eq!(receipt.placed_existing_instance_ids, vec![9]);
    }

    #[test]
    fn full_delivery_requires_spill_context_and_then_records_spill() {
        let mut inventory = inventory(
            1,
            1,
            vec![PlacedItemState {
                row: 0,
                col: 0,
                instance: item(1, "ore", 1),
            }],
        );
        let registry = registry();
        let mut allocator = InventoryInstanceIdAllocator::new(20);
        let mut incoming = item(9, "ore", 1);
        incoming.durability = 0.2;
        let before = inventory.clone();
        {
            let mut txn = InventoryTxn::new(&mut inventory, &registry, &mut allocator);
            assert_eq!(
                txn.deliver(
                    DeliveryRequest::new(
                        "no-spill",
                        vec![DeliveryItem::existing(incoming.clone())],
                    ),
                    None,
                ),
                Err(InventoryTxnError::MissingSpillContext)
            );
        }
        assert_eq!(inventory.revision, before.revision);
        assert_eq!(inventory.containers, before.containers);

        let mut drops = DroppedLootRegistry::default();
        let mut capacity = BoundedCapacityProvider::new(0, 4).expect("capacity");
        let mut durable = NoopDurableSpill;
        let mut context = spill_context(&mut drops, &mut capacity, &mut durable);
        let mut txn = InventoryTxn::new(&mut inventory, &registry, &mut allocator);
        let receipt = txn
            .deliver(
                DeliveryRequest::new("spill", vec![DeliveryItem::existing(incoming)]),
                Some(&mut context),
            )
            .expect("durable spill succeeds");
        assert_eq!(
            receipt.stored_count + receipt.spilled_count,
            receipt.requested_count
        );
        assert_eq!(drops.entries.len(), 1);
        assert_eq!(capacity.snapshot().current, 1);
    }

    #[test]
    fn failed_durable_spill_releases_capacity_and_preserves_source_and_registry() {
        let mut inventory = inventory(
            1,
            1,
            vec![PlacedItemState {
                row: 0,
                col: 0,
                instance: item(1, "ore", 1),
            }],
        );
        let before = inventory.clone();
        let registry = registry();
        let mut allocator = InventoryInstanceIdAllocator::new(20);
        let mut drops = DroppedLootRegistry::default();
        let mut capacity = BoundedCapacityProvider::new(0, 4).expect("capacity");
        let mut durable = FailingDurableSpill;
        let mut context = SpillContext {
            source_identity: "player:alice".to_string(),
            source_revision: InventoryRevision(7),
            dimension: DimensionKind::Overworld,
            world_pos: [1.0, 64.0, 2.0],
            registry: &mut drops,
            capacity: &mut capacity,
            durable: &mut durable,
            transaction_id: "txn-failing".to_string(),
        };
        let mut txn = InventoryTxn::new(&mut inventory, &registry, &mut allocator);
        let mut incoming = item(9, "ore", 1);
        incoming.durability = 0.2;
        assert!(matches!(
            txn.deliver(
                DeliveryRequest::new("spill-failure", vec![DeliveryItem::existing(incoming)]),
                Some(&mut context),
            ),
            Err(InventoryTxnError::PersistenceUnavailable { .. })
        ));
        assert_eq!(inventory.revision, before.revision);
        assert_eq!(inventory.containers, before.containers);
        assert!(drops.entries.is_empty());
        assert_eq!(capacity.snapshot().reserved, 0);
        assert_eq!(capacity.snapshot().current, 0);
    }

    #[test]
    fn pickup_rejects_wrong_dimension_without_removing_drop() {
        let mut inventory = inventory(1, 1, Vec::new());
        let registry = registry();
        let mut allocator = InventoryInstanceIdAllocator::new(20);
        let mut drops = DroppedLootRegistry::default();
        drops.entries.insert(
            9,
            DroppedLootEntry {
                instance_id: 9,
                source_container_id: "player:alice".to_string(),
                source_row: 0,
                source_col: 0,
                world_pos: [0.0, 64.0, 0.0],
                dimension: DimensionKind::Tsy,
                owner: None,
                visibility: DroppedLootVisibility::Public,
                item: item(9, "ore", 1),
            },
        );
        let mut txn = InventoryTxn::new(&mut inventory, &registry, &mut allocator);
        let error = txn
            .pickup_and_merge(
                PickupRequest::new("pickup", 9),
                PickupAuthorization::new(
                    "player:alice",
                    DimensionKind::Overworld,
                    [0.0, 64.0, 0.0],
                    2.5,
                ),
                &mut drops,
            )
            .expect_err("cross-dimension pickup is forbidden");
        assert_eq!(error, InventoryTxnError::WrongDimension);
        assert!(drops.entries.contains_key(&9));
    }

    #[test]
    fn pickup_rejects_registry_key_identity_mismatch_without_mutation() {
        let mut inventory = inventory(1, 1, Vec::new());
        let before_inventory = inventory.clone();
        let registry = registry();
        let mut allocator = InventoryInstanceIdAllocator::new(20);
        let mut drops = DroppedLootRegistry::default();
        drops.entries.insert(
            9,
            DroppedLootEntry {
                instance_id: 10,
                source_container_id: "player:alice".to_string(),
                source_row: 0,
                source_col: 0,
                world_pos: [0.0, 64.0, 0.0],
                dimension: DimensionKind::Overworld,
                owner: None,
                visibility: DroppedLootVisibility::Public,
                item: item(10, "ore", 1),
            },
        );
        let before_drops = drops.clone();
        let mut txn = InventoryTxn::new(&mut inventory, &registry, &mut allocator);
        let error = txn
            .pickup_and_merge(
                PickupRequest::new("identity-mismatch", 9),
                PickupAuthorization::new(
                    "player:alice",
                    DimensionKind::Overworld,
                    [0.0, 64.0, 0.0],
                    2.5,
                ),
                &mut drops,
            )
            .expect_err("registry key and entry identity must agree");
        assert_eq!(
            error,
            InventoryTxnError::IdentityMismatch { instance_id: 9 }
        );
        assert_eq!(
            serde_json::to_value(&inventory).expect("inventory should serialize"),
            serde_json::to_value(&before_inventory).expect("inventory snapshot should serialize")
        );
        assert_eq!(drops.entries, before_drops.entries);
    }

    #[test]
    fn pickup_out_of_range_keeps_drop_and_pickup_success_removes_it_after_attach() {
        let mut inventory = inventory(1, 1, Vec::new());
        let registry = registry();
        let mut allocator = InventoryInstanceIdAllocator::new(20);
        let mut drops = DroppedLootRegistry::default();
        drops.entries.insert(
            9,
            DroppedLootEntry {
                instance_id: 9,
                source_container_id: "player:alice".to_string(),
                source_row: 0,
                source_col: 0,
                world_pos: [4.0, 64.0, 0.0],
                dimension: DimensionKind::Overworld,
                owner: None,
                visibility: DroppedLootVisibility::Public,
                item: item(9, "ore", 1),
            },
        );
        let mut txn = InventoryTxn::new(&mut inventory, &registry, &mut allocator);
        let error = txn
            .pickup_and_merge(
                PickupRequest::new("too-far", 9),
                PickupAuthorization::new(
                    "player:alice",
                    DimensionKind::Overworld,
                    [0.0, 64.0, 0.0],
                    2.5,
                ),
                &mut drops,
            )
            .expect_err("out-of-range pickup is forbidden");
        assert!(matches!(error, InventoryTxnError::OutOfRange { .. }));
        assert!(drops.entries.contains_key(&9));
        let receipt = txn
            .pickup_and_merge(
                PickupRequest::new("near", 9),
                PickupAuthorization::new(
                    "player:alice",
                    DimensionKind::Overworld,
                    [4.0, 64.0, 0.0],
                    2.5,
                ),
                &mut drops,
            )
            .expect("same-dimension in-range pickup succeeds");
        assert_eq!(receipt.removed_drop_id, 9);
        assert!(!drops.entries.contains_key(&9));
        assert_eq!(inventory.revision, InventoryRevision(8));
    }
}
