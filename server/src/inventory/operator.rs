//! OP 背包复用 body_pocket。存档中的尺寸不是授权：新登录先校准，在线玩家在
//! PreUpdate 校准，均早于库存操作。装备重建只保留已校准的容器，不自行判断权限。

use std::io;

use valence::prelude::bevy_ecs::system::SystemParam;
use valence::prelude::{App, Client, Commands, Entity, PreUpdate, Query, Res, ResMut, Username};

use super::*;
use crate::cmd::dev::DevCommandPermissions;
use crate::player::state::{save_player_inventory_with_drops, PlayerStatePersistence};

const OP_POCKET_NAME: &str = "OP 背包";
const OP_POCKET_ROWS: u8 = 12;
const OP_POCKET_COLS: u8 = 12;
const OP_CARRY_CAPACITY: f64 = 1000.0;

pub(super) fn register(app: &mut App) {
    app.add_systems(PreUpdate, sync_operator_inventory);
}

pub(super) fn base_carry_capacity(inventory: &PlayerInventory) -> f64 {
    if inventory.containers.iter().any(|container| {
        container.id == BODY_POCKET_CONTAINER_ID
            && container.name == OP_POCKET_NAME
            && container.rows == OP_POCKET_ROWS
            && container.cols == OP_POCKET_COLS
    }) {
        OP_CARRY_CAPACITY
    } else {
        BASE_CARRY_CAPACITY
    }
}

pub(super) fn expand_pocket(inventory: &mut PlayerInventory, registry: &ItemRegistry) {
    if let Some(pocket) = inventory
        .containers
        .iter_mut()
        .find(|container| container.id == BODY_POCKET_CONTAINER_ID)
    {
        pocket.name = OP_POCKET_NAME.to_string();
        pocket.rows = OP_POCKET_ROWS;
        pocket.cols = OP_POCKET_COLS;
        inventory.max_weight = compute_max_weight(inventory, registry);
        bump_revision(inventory);
    }
}

/// Optional 资源兼容只装配部分系统的测试；需要变更时缺少必要资源则拒绝操作。
#[derive(SystemParam)]
pub(crate) struct OperatorInventoryAccess<'w> {
    permissions: Option<Res<'w, DevCommandPermissions>>,
    registry: Option<Res<'w, ItemRegistry>>,
    loadout: Option<Res<'w, DefaultLoadout>>,
    persistence: Option<Res<'w, PlayerStatePersistence>>,
    dropped: Option<ResMut<'w, DroppedLootRegistry>>,
}

impl OperatorInventoryAccess<'_> {
    pub(crate) fn reconcile(
        &mut self,
        username: &str,
        inventory: &PlayerInventory,
        position: [f64; 3],
        dimension: DimensionKind,
    ) -> io::Result<Option<PlayerInventory>> {
        let operator = self
            .permissions
            .as_ref()
            .is_some_and(|permissions| permissions.is_operator(username));
        let ordinary_pocket = self.loadout.as_ref().and_then(|loadout| {
            loadout
                .0
                .containers
                .iter()
                .find(|container| container.id == BODY_POCKET_CONTAINER_ID)
        });
        let (rows, cols, name) = if operator {
            (OP_POCKET_ROWS, OP_POCKET_COLS, OP_POCKET_NAME)
        } else {
            ordinary_pocket.map_or(
                (BODY_POCKET_ROWS, BODY_POCKET_COLS, "贴身口袋"),
                |pocket| (pocket.rows, pocket.cols, pocket.name.as_str()),
            )
        };
        let pocket = inventory
            .containers
            .iter()
            .find(|container| container.id == BODY_POCKET_CONTAINER_ID);
        if pocket.is_some_and(|pocket| {
            pocket.rows == rows
                && pocket.cols == cols
                && if operator {
                    pocket.name == name
                } else {
                    pocket.name != OP_POCKET_NAME
                }
        }) || (!operator && pocket.is_none())
        {
            return Ok(None);
        }

        let registry = self
            .registry
            .as_deref()
            .ok_or_else(|| io::Error::other("OP 背包缺少物品注册表"))?;
        let mut staged = inventory.clone();
        let overflow = resize_pocket(&mut staged, registry, rows, cols, name);
        staged.revision.0 = inventory.revision.0.saturating_add(1);
        if !overflow.is_empty() {
            let persistence = self
                .persistence
                .as_deref()
                .ok_or_else(|| io::Error::other("撤权溢出缺少持久化资源"))?;
            let dropped = self
                .dropped
                .as_deref_mut()
                .ok_or_else(|| io::Error::other("撤权溢出缺少掉落物注册表"))?;
            let entries: Vec<_> = overflow
                .into_iter()
                .map(|item| DroppedLootEntry {
                    instance_id: item.instance_id,
                    source_container_id: BODY_POCKET_CONTAINER_ID.to_string(),
                    source_row: 0,
                    source_col: 0,
                    world_pos: [position[0], position[1] + 0.5, position[2]],
                    dimension,
                    owner: None,
                    visibility: DroppedLootVisibility::Public,
                    item,
                })
                .collect();
            let mut staged_dropped = dropped.clone();
            staged_dropped
                .try_insert_public_batch(entries.clone())
                .map_err(|error| io::Error::other(format!("掉落物 admission 失败: {error:?}")))?;
            // 库存减物和掉落物增物必须同事务。失败时不发布缩容结果，也不写运行时掉落表。
            save_player_inventory_with_drops(persistence, username, &staged, &entries)?;
            *dropped = staged_dropped;
        }
        Ok(Some(staged))
    }
}

fn resize_pocket(
    inventory: &mut PlayerInventory,
    registry: &ItemRegistry,
    rows: u8,
    cols: u8,
    name: &str,
) -> Vec<ItemInstance> {
    let pocket = match inventory
        .containers
        .iter()
        .position(|container| container.id == BODY_POCKET_CONTAINER_ID)
    {
        Some(index) => &mut inventory.containers[index],
        None => {
            inventory.containers.push(ContainerState {
                id: BODY_POCKET_CONTAINER_ID.to_string(),
                name: name.to_string(),
                rows,
                cols,
                items: Vec::new(),
                owner_instance_id: None,
                quick_access: false,
            });
            inventory
                .containers
                .last_mut()
                .expect("刚创建的口袋必须存在")
        }
    };
    pocket.rows = rows;
    pocket.cols = cols;
    pocket.name = name.to_string();
    let mut displaced = Vec::new();
    pocket.items.retain(|placed| {
        let fits = u16::from(placed.row) + u16::from(placed.instance.grid_h) <= u16::from(rows)
            && u16::from(placed.col) + u16::from(placed.instance.grid_w) <= u16::from(cols);
        if !fits {
            displaced.push(placed.instance.clone());
        }
        fits
    });
    // 先留在暗袋，尽量保住直接携带背包件派生的容器身份。
    let mut overflow = Vec::new();
    for item in displaced {
        let pocket = inventory
            .containers
            .iter_mut()
            .find(|container| container.id == BODY_POCKET_CONTAINER_ID)
            .unwrap();
        if let Some((row, col)) = find_free_slot(pocket, item.grid_w, item.grid_h) {
            pocket.items.push(PlacedItemState {
                row,
                col,
                instance: item,
            });
        } else if let Some(location) = find_first_fit_container_location(inventory, &item) {
            // 权限撤销不会销毁物品；即使现有总重超过普通上限，也允许搬到已有容器。
            if attach_at_location(inventory, item.clone(), &location).is_err() {
                overflow.push(item);
            }
        } else {
            overflow.push(item);
        }
    }
    // 若背包件移入其它背包或溢出，它派生的容器必须随之回收，并保住内含物。
    overflow.extend(rebuild_containers_from_equipment(inventory, registry));
    overflow
}

type OperatorInventoryQuery<'a> = (
    Entity,
    &'a Username,
    &'a Position,
    Option<&'a CurrentDimension>,
    &'a mut PlayerInventory,
);

fn sync_operator_inventory(
    mut commands: Commands,
    mut access: OperatorInventoryAccess<'_>,
    mut players: Query<OperatorInventoryQuery<'_>, With<Client>>,
) {
    for (entity, username, position, dimension, mut inventory) in &mut players {
        match access.reconcile(
            &username.0,
            &inventory,
            position.get().to_array(),
            dimension.map_or(DimensionKind::default(), |value| value.0),
        ) {
            Ok(Some(updated)) => *inventory = updated,
            Ok(None) => {}
            Err(error) => {
                tracing::error!(
                    "[bong][inventory] cannot safely reconcile OP backpack for {}: {error}",
                    username.0
                );
                // 断开后普通保存仍持有原库存；重连必须重新通过权限校准，不会开放旧扩容。
                commands.entity(entity).remove::<Client>();
            }
        }
    }
}

#[cfg(test)]
mod tests;
