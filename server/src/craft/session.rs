//! plan-craft-v1 §3 — `CraftSession` component + 状态机。
//!
//! §0 设计轴心：
//!   * **单任务**：玩家同时只允许一个 CraftSession 存在，新 start 必须先 cancel
//!   * **in-game 时间推进**：只有 `tick_session` 显式推进时才走，玩家下线
//!     （inventory 关闭）时调用方不调用 tick，自动暂停
//!   * **守恒律**：qi_cost 一次性走 `qi_physics::ledger::QiTransfer`
//!     （Crafting reason），**禁止** `cultivation.qi_current -= cost` 直接扣
//!
//! §5 决策门：
//!   * #3 = B：取消任务返还材料 70%（向下取整），qi 不退
//!   * #4 = A：玩家死亡 → 走 cancel 路径，PlayerDied 作为 reason
//!   * #6 = B：requirements 软 gate，但 `start_craft` 内做硬校验防作弊

use valence::prelude::{bevy_ecs, Component, Entity};

use crate::cultivation::components::{ColorKind, Cultivation, QiColor, Realm};
use crate::inventory::{ContainerState, ItemInstance, PlayerInventory};
use crate::qi_physics::ledger::{QiAccountId, QiTransfer, QiTransferReason, WorldQiAccount};
use crate::qi_physics::QiPhysicsError;

use super::events::{CraftCompletedEvent, CraftFailedEvent, CraftFailureReason, CraftStartedEvent};
use super::recipe::{CraftRecipe, RecipeId};
use super::registry::CraftRegistry;
use super::unlock::RecipeUnlockState;

/// §5 决策门 #3 = B：取消返还 70%，30% 损耗惩罚。
pub const CANCEL_REFUND_RATIO: f64 = 0.7;

/// `start_craft` 的"ledger 与 cultivation 视图失同步"严格判定阈值。
/// 浮点容差 — 1e-9 远大于 transfer 路径任何累积误差，但小到能捕获语义性 desync。
const QI_SYNC_EPSILON: f64 = 1e-9;

/// 玩家进行中的手搓任务。
/// 玩家只允许同时挂 1 个 CraftSession（单任务）。`remaining_ticks` 由
/// `tick_session` 在玩家在线时推进；为 0 时调用 `finalize_craft`。
#[derive(Debug, Clone, Component, PartialEq)]
pub struct CraftSession {
    pub recipe_id: RecipeId,
    /// 起手 tick 时戳（统计 / UI 显示用）
    pub started_at_tick: u64,
    /// 剩余 in-game tick；0 时表示完成
    pub remaining_ticks: u64,
    /// 起手时的 `total_ticks`（用于 UI 进度条），与 recipe 的 time_ticks 等价
    pub total_ticks: u64,
    /// 玩家 canonical id（"offline:Alice"），用于 unlock state / refund 等查找
    pub owner_player_id: String,
    /// 起手时实际扣除的 qi（守恒律观察值，必须与 ledger 中 transfer 的 amount 等同）
    pub qi_paid: f64,
    /// 本 session 总制作件数。1 表示普通单件制作。
    pub quantity_total: u32,
    /// 已经完成并发放到背包的件数。
    pub completed_count: u32,
}

/// `start_craft` 的失败原因。所有 reject 路径都不会写 ledger / 不会扣材料 / 不会改 inventory。
#[derive(Debug, Clone, PartialEq)]
pub enum StartCraftError {
    /// 配方 id 在 registry 内不存在
    UnknownRecipe(RecipeId),
    /// 配方未对该玩家解锁
    NotUnlocked(RecipeId),
    /// 玩家已有正在进行的 session
    AlreadyHasSession,
    /// 缺料：包含缺失清单 (template_id, have, need)
    MissingMaterials(Vec<MaterialDeficit>),
    /// 真元不足：have < need
    InsufficientQi { have: f64, need: f64 },
    /// 境界不足：要求 vs 当前
    RealmTooLow { required: Realm, current: Realm },
    /// 真元色不满足（main color 不匹配 kind）
    QiColorMismatch {
        required: ColorKind,
        current: ColorKind,
    },
    /// **ledger 与 cultivation state view 失同步** — 调用方应先调用
    /// `qi_physics` 的 sync system 把 player 账户镜像到 cultivation.qi_current
    /// 后再 retry。当前 craft 模块不主动 set_balance（避免 inflate ledger
    /// 总数），改由调用方负责状态同步以保守恒律。
    LedgerOutOfSync {
        player_balance: f64,
        cultivation_qi_current: f64,
        required: f64,
    },
    /// ledger 内部错误（transfer 失败等）
    LedgerError(String),
    /// 批量数量必须 >= 1。
    InvalidQuantity(u32),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MaterialDeficit {
    pub template_id: String,
    pub have: u32,
    pub need: u32,
}

/// `start_craft` 成功结果包：调用方应：
///   1. 把 `session` insert 到 caster entity
///   2. 把 `event` 通过 EventWriter 广播
///   3. 把 `consumed_templates` 调用 `consume_materials_from_inventory` 真正扣减
///      （已在内部完成 — 此字段仅供 trace / 日志）
#[derive(Debug, Clone)]
pub struct StartCraftSuccess {
    pub session: CraftSession,
    pub event: CraftStartedEvent,
    pub consumed: Vec<(String, u32)>,
}

/// 取消手搓的产出包：返还清单 + 失败事件。
#[derive(Debug, Clone)]
pub struct CancelCraftOutcome {
    pub event: CraftFailedEvent,
    /// 70% 返还材料：(template_id, refund_count)。0 数量不写入。
    /// 调用方需要 `inventory::add_item_to_player_inventory` 真实加回 inventory。
    pub refund_manifest: Vec<(String, u32)>,
}

/// 完成手搓的产出包：产出物 + 完成事件。
#[derive(Debug, Clone)]
pub struct FinalizeCraftOutcome {
    pub event: CraftCompletedEvent,
    /// 产出：(template_id, count)。
    /// 调用方需要 `inventory::add_item_to_player_inventory` 真实加进 inventory。
    pub output_manifest: (String, u32),
}

/// 服务端按 template_id 统计玩家 inventory 内某物品的总数（含 containers + hotbar，
/// 不含 equipped — 装备槽里的东西不该当材料）。
pub fn count_template_in_inventory(inventory: &PlayerInventory, template_id: &str) -> u32 {
    let from_containers: u32 = inventory
        .containers
        .iter()
        .flat_map(|c: &ContainerState| c.items.iter())
        .filter(|p| p.instance.template_id == template_id)
        .map(|p| p.instance.stack_count)
        .sum();
    let from_hotbar: u32 = inventory
        .hotbar
        .iter()
        .filter_map(|s| s.as_ref())
        .filter(|i: &&ItemInstance| i.template_id == template_id)
        .map(|i| i.stack_count)
        .sum();
    from_containers + from_hotbar
}

/// 从 inventory 扣减 `count` 个 `template_id` —— containers 优先，hotbar 兜底。
/// 找到的 stack 按 stack_count 衰减，归零的 placed item 立刻移除。
///
/// 调用方应在 `count_template_in_inventory` 已确认充足后再调；本函数若发现
/// 实际不足会**部分扣完后返回 Err**（调用方需要 rollback 的话需要先 snapshot）。
pub fn consume_materials_from_inventory(
    inventory: &mut PlayerInventory,
    template_id: &str,
    mut needed: u32,
) -> Result<(), MaterialDeficit> {
    if needed == 0 {
        return Ok(());
    }
    // 先吃 containers
    for container in inventory.containers.iter_mut() {
        let mut i = 0;
        while i < container.items.len() {
            if container.items[i].instance.template_id == template_id {
                let take = needed.min(container.items[i].instance.stack_count);
                container.items[i].instance.stack_count -= take;
                needed -= take;
                if container.items[i].instance.stack_count == 0 {
                    container.items.remove(i);
                    continue;
                }
            }
            i += 1;
            if needed == 0 {
                return Ok(());
            }
        }
    }
    // hotbar 兜底
    for slot in inventory.hotbar.iter_mut() {
        if needed == 0 {
            break;
        }
        let drop_slot = if let Some(item) = slot.as_mut() {
            if item.template_id == template_id {
                let take = needed.min(item.stack_count);
                item.stack_count -= take;
                needed -= take;
                item.stack_count == 0
            } else {
                false
            }
        } else {
            false
        };
        if drop_slot {
            *slot = None;
        }
    }
    if needed == 0 {
        Ok(())
    } else {
        Err(MaterialDeficit {
            template_id: template_id.to_string(),
            have: 0, // 已经吃完所有，没办法回填精确 have；调用方应在 count_* 阶段就拒绝
            need: needed,
        })
    }
}

/// 起手参数包 — 桥接 caller 提供的所有外部状态。
pub struct StartCraftRequest<'a> {
    pub caster: Entity,
    pub player_id: &'a str,
    pub recipe_id: &'a RecipeId,
    pub current_tick: u64,
    pub zone_id: &'a str,
    pub quantity: u32,
}

/// 守恒律调用器：传入对真元 ledger 的 mut 引用。
pub struct StartCraftDeps<'a> {
    pub registry: &'a CraftRegistry,
    pub unlock_state: &'a RecipeUnlockState,
    pub inventory: &'a mut PlayerInventory,
    pub cultivation: &'a mut Cultivation,
    pub qi_color: &'a QiColor,
    pub ledger: &'a mut WorldQiAccount,
    pub existing_session: Option<&'a CraftSession>,
}

/// §3 主入口 — 起手手搓。
///
/// 校验顺序（**任一失败立即 Err，无副作用，可放心 retry**）：
/// 1. recipe 必须存在
/// 2. 玩家 unlock state 必须包含 recipe
/// 3. 玩家未占用其他 session
/// 4. requirements: realm / qi_color 满足
/// 5. 材料足够（count_template_in_inventory）
/// 6. qi 足够（cultivation.qi_current ≥ qi_cost）
///
/// 副作用阶段（成功必经）：
/// 7. ledger transfer player → zone（reason = Crafting），同时
///    `cultivation.qi_current -= qi_cost`（守恒律一致性）
/// 8. 扣材料
/// 9. 构造 CraftSession + CraftStartedEvent
pub fn start_craft(
    request: StartCraftRequest<'_>,
    deps: StartCraftDeps<'_>,
) -> Result<StartCraftSuccess, StartCraftError> {
    if request.quantity == 0 {
        return Err(StartCraftError::InvalidQuantity(request.quantity));
    }
    let recipe = deps
        .registry
        .get(request.recipe_id)
        .ok_or_else(|| StartCraftError::UnknownRecipe(request.recipe_id.clone()))?;

    if !deps.unlock_state.is_unlocked(request.player_id, &recipe.id) {
        return Err(StartCraftError::NotUnlocked(recipe.id.clone()));
    }

    if deps.existing_session.is_some() {
        return Err(StartCraftError::AlreadyHasSession);
    }

    if let Some(min) = recipe.requirements.realm_min {
        let cur = deps.cultivation.realm;
        if (cur as u8) < (min as u8) {
            return Err(StartCraftError::RealmTooLow {
                required: min,
                current: cur,
            });
        }
    }

    if let Some((kind, _share)) = recipe.requirements.qi_color_min {
        // P1 阶段简化：main color 命中即视为满足；share 阈值留 P2 接入
        // qi_color 评估系统时再细化（plan-qi-physics-v2 / qi_color/color.rs）。
        if deps.qi_color.main != kind {
            return Err(StartCraftError::QiColorMismatch {
                required: kind,
                current: deps.qi_color.main,
            });
        }
    }

    // 材料充足校验
    let mut deficits = Vec::new();
    for (template, need) in &recipe.materials {
        let total_need = need.saturating_mul(request.quantity);
        let have = count_template_in_inventory(deps.inventory, template);
        if have < total_need {
            deficits.push(MaterialDeficit {
                template_id: template.clone(),
                have,
                need: total_need,
            });
        }
    }
    if !deficits.is_empty() {
        return Err(StartCraftError::MissingMaterials(deficits));
    }

    let total_qi_cost = recipe.qi_cost * f64::from(request.quantity);
    if deps.cultivation.qi_current < total_qi_cost {
        return Err(StartCraftError::InsufficientQi {
            have: deps.cultivation.qi_current,
            need: total_qi_cost,
        });
    }

    // ===== 副作用阶段 =====
    let from = QiAccountId::player(request.player_id);
    let to = QiAccountId::zone(request.zone_id);
    if total_qi_cost > 0.0 {
        // 守恒律：调用方必须先把 cultivation.qi_current **严格** sync 到
        // ledger.player(id)（待 qi_physics::sync_player_qi_to_ledger system
        // 接入后由 ECS hook 自动同步）。本函数**不**主动 set_balance，避免
        // ad-hoc 注入导致 sum(ledger) inflate 破坏全局守恒律。
        //
        // 如果 ledger.balance(player) ≠ cultivation.qi_current，说明视图失同步：
        // - balance < cult：出过 cultivation 增量没镜像到 ledger（如 regen）
        // - balance > cult：ledger 收到了 cultivation 没扣的 outflow
        // 两种情况都属 desync，调用方需要先 sync 再 retry。
        let player_balance = deps.ledger.balance(&from);
        if (player_balance - deps.cultivation.qi_current).abs() > QI_SYNC_EPSILON {
            return Err(StartCraftError::LedgerOutOfSync {
                player_balance,
                cultivation_qi_current: deps.cultivation.qi_current,
                required: total_qi_cost,
            });
        }
        // 视图严格一致后，验证余额够付（外层 cultivation_qi_current >= qi_cost
        // 已校验，sync 一致后 player_balance 也保证 >= qi_cost；fail-safe）
        if player_balance < total_qi_cost {
            return Err(StartCraftError::LedgerOutOfSync {
                player_balance,
                cultivation_qi_current: deps.cultivation.qi_current,
                required: total_qi_cost,
            });
        }

        let transfer = QiTransfer::new(
            from.clone(),
            to.clone(),
            total_qi_cost,
            QiTransferReason::Crafting,
        )
        .map_err(|e: QiPhysicsError| StartCraftError::LedgerError(e.to_string()))?;
        deps.ledger
            .transfer(transfer)
            .map_err(|e: QiPhysicsError| StartCraftError::LedgerError(e.to_string()))?;

        deps.cultivation.qi_current -= total_qi_cost;
        if deps.cultivation.qi_current < 0.0 {
            // 上面已校验充足，这里不该走到；fail-safe clamp
            deps.cultivation.qi_current = 0.0;
        }
    }

    // 扣材料（不可回滚 — 上面已确认充足）
    let mut consumed = Vec::with_capacity(recipe.materials.len());
    for (template, need) in &recipe.materials {
        let total_need = need.saturating_mul(request.quantity);
        consume_materials_from_inventory(deps.inventory, template, total_need)
            .expect("materials checked above");
        consumed.push((template.clone(), total_need));
    }

    let session = CraftSession {
        recipe_id: recipe.id.clone(),
        started_at_tick: request.current_tick,
        remaining_ticks: recipe.time_ticks,
        total_ticks: recipe.time_ticks,
        owner_player_id: request.player_id.to_string(),
        qi_paid: total_qi_cost,
        quantity_total: request.quantity,
        completed_count: 0,
    };

    let event = CraftStartedEvent {
        caster: request.caster,
        recipe_id: recipe.id.clone(),
        started_at_tick: request.current_tick,
        total_ticks: recipe.time_ticks,
        qi_paid: total_qi_cost,
    };

    Ok(StartCraftSuccess {
        session,
        event,
        consumed,
    })
}

/// in-game 推进 session.remaining_ticks。`amount` 为消耗的 in-game tick 数。
/// 返回 true 表示这一推进让 session 完成（调用方紧接着应调 finalize_craft）。
///
/// 注意：本函数仅做计数推进。**调用方负责在线状态判定**——下线时不要调用本函数。
pub fn tick_session(session: &mut CraftSession, amount: u64) -> bool {
    if session.remaining_ticks == 0 {
        return true;
    }
    session.remaining_ticks = session.remaining_ticks.saturating_sub(amount);
    session.remaining_ticks == 0
}

/// 计算取消时的返还清单（材料 70% 向下取整）。
/// 不动 inventory / 不扣 qi；调用方按返还清单执行 `add_item_to_player_inventory`。
pub fn cancel_craft(
    session: &CraftSession,
    recipe: &CraftRecipe,
    caster: Entity,
    reason: CraftFailureReason,
) -> CancelCraftOutcome {
    debug_assert_eq!(
        session.recipe_id, recipe.id,
        "cancel_craft: session/recipe id mismatch"
    );
    let refund_manifest: Vec<(String, u32)> = recipe
        .materials
        .iter()
        .map(|(template, need)| {
            let remaining_count = session
                .quantity_total
                .saturating_sub(session.completed_count);
            let reserved_need = need.saturating_mul(remaining_count);
            let refund = ((reserved_need as f64) * CANCEL_REFUND_RATIO).floor() as u32;
            (template.clone(), refund)
        })
        .filter(|(_, refund)| *refund > 0)
        .collect();
    let total_returned: u32 = refund_manifest.iter().map(|(_, n)| *n).sum();

    let event = CraftFailedEvent {
        caster,
        recipe_id: recipe.id.clone(),
        reason,
        material_returned: total_returned,
        qi_refunded: 0.0, // §5 决策门 #3：qi 不退
    };
    CancelCraftOutcome {
        event,
        refund_manifest,
    }
}

/// 完成手搓 — 计算产出 manifest + 完成事件。
/// 不动 inventory；调用方按 output_manifest 执行 `add_item_to_player_inventory`。
pub fn finalize_craft(
    session: &CraftSession,
    recipe: &CraftRecipe,
    caster: Entity,
    current_tick: u64,
) -> FinalizeCraftOutcome {
    debug_assert_eq!(
        session.recipe_id, recipe.id,
        "finalize_craft: session/recipe id mismatch"
    );
    let event = CraftCompletedEvent {
        caster,
        recipe_id: recipe.id.clone(),
        completed_at_tick: current_tick,
        output_template: recipe.output.0.clone(),
        output_count: recipe.output.1,
    };
    FinalizeCraftOutcome {
        event,
        output_manifest: recipe.output.clone(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::super::events::{InsightTrigger, UnlockEventSource};
    use super::super::recipe::{CraftCategory, CraftRequirements, UnlockSource};
    use super::*;
    use crate::cultivation::components::Cultivation;
    use crate::inventory::{
        ContainerState, InventoryRevision, ItemInstance, ItemRarity, PlacedItemState,
    };
    use crate::qi_physics::ledger::QiAccountId;
    use valence::prelude::App;

    fn make_inventory(items: &[(&str, u32)]) -> PlayerInventory {
        let placed: Vec<PlacedItemState> = items
            .iter()
            .enumerate()
            .map(|(idx, (template, n))| PlacedItemState {
                row: idx as u8,
                col: 0,
                instance: ItemInstance {
                    instance_id: idx as u64 + 1,
                    template_id: (*template).into(),
                    display_name: (*template).into(),
                    grid_w: 1,
                    grid_h: 1,
                    weight: 1.0,
                    rarity: ItemRarity::Common,
                    description: String::new(),
                    stack_count: *n,
                    spirit_quality: 0.0,
                    durability: 1.0,
                    freshness: None,
                    mineral_id: None,
                    charges: None,
                    forge_quality: None,
                    forge_color: None,
                    forge_side_effects: Vec::new(),
                    forge_achieved_tier: None,
                    alchemy: None,
                    lingering_owner_qi: None,
                },
            })
            .collect();
        PlayerInventory {
            revision: InventoryRevision(1),
            containers: vec![ContainerState {
                id: "main_pack".into(),
                name: "main".into(),
                rows: 16,
                cols: 1,
                items: placed,
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 100.0,
        }
    }

    fn simple_recipe(id: &str) -> CraftRecipe {
        CraftRecipe {
            id: RecipeId::new(id),
            category: CraftCategory::Misc,
            display_name: id.into(),
            materials: vec![("herb_a".into(), 2), ("iron_needle".into(), 3)],
            qi_cost: 5.0,
            time_ticks: 100,
            output: ("test_pill".into(), 1),
            requirements: CraftRequirements::default(),
            unlock_sources: vec![UnlockSource::Scroll {
                item_template: "scroll_x".into(),
            }],
        }
    }

    fn ok_deps_for_player<'a>(
        registry: &'a CraftRegistry,
        unlock: &'a RecipeUnlockState,
        inventory: &'a mut PlayerInventory,
        cultivation: &'a mut Cultivation,
        color: &'a QiColor,
        ledger: &'a mut WorldQiAccount,
    ) -> StartCraftDeps<'a> {
        StartCraftDeps {
            registry,
            unlock_state: unlock,
            inventory,
            cultivation,
            qi_color: color,
            ledger,
            existing_session: None,
        }
    }

    fn make_world() -> (
        CraftRegistry,
        RecipeUnlockState,
        Cultivation,
        QiColor,
        WorldQiAccount,
    ) {
        let mut registry = CraftRegistry::new();
        registry.register(simple_recipe("a")).unwrap();
        let mut unlock = RecipeUnlockState::new();
        unlock.unlock("offline:Alice", RecipeId::new("a"));
        let cultivation = Cultivation {
            qi_current: 50.0,
            qi_max: 80.0,
            ..Default::default()
        };
        let color = QiColor::default();
        // 模拟未来 qi_physics::sync_player_qi_to_ledger system —— 把
        // cultivation.qi_current 镜像到 ledger.player 账户后才能 start_craft
        let mut ledger = WorldQiAccount::default();
        ledger
            .set_balance(QiAccountId::player("offline:Alice"), cultivation.qi_current)
            .unwrap();
        (registry, unlock, cultivation, color, ledger)
    }

    fn caster_entity() -> Entity {
        // 在测试 App 内 spawn empty 拿 entity id（其他 fn 不需要真 App）
        let mut app = App::new();
        app.world_mut().spawn_empty().id()
    }

    // ============= 材料统计 =============

    #[test]
    fn count_template_aggregates_containers_and_hotbar() {
        let mut inv = make_inventory(&[("herb_a", 5), ("herb_a", 3), ("iron_needle", 2)]);
        // hotbar 内再放 4 个 herb_a
        inv.hotbar[0] = Some(ItemInstance {
            instance_id: 99,
            template_id: "herb_a".into(),
            display_name: "herb_a".into(),
            grid_w: 1,
            grid_h: 1,
            weight: 1.0,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 4,
            spirit_quality: 0.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        });
        assert_eq!(count_template_in_inventory(&inv, "herb_a"), 5 + 3 + 4);
        assert_eq!(count_template_in_inventory(&inv, "iron_needle"), 2);
        assert_eq!(count_template_in_inventory(&inv, "absent"), 0);
    }

    #[test]
    fn consume_materials_drains_in_order_and_drops_empty_stacks() {
        let mut inv = make_inventory(&[("herb_a", 5), ("herb_a", 3)]);
        consume_materials_from_inventory(&mut inv, "herb_a", 6).unwrap();
        // 第一个 stack 被吃完移除，第二个剩 2
        let remaining: Vec<_> = inv.containers[0]
            .items
            .iter()
            .map(|p| p.instance.stack_count)
            .collect();
        assert_eq!(remaining, vec![2]);
    }

    #[test]
    fn consume_materials_zero_count_is_noop() {
        let mut inv = make_inventory(&[("herb_a", 5)]);
        consume_materials_from_inventory(&mut inv, "herb_a", 0).unwrap();
        assert_eq!(count_template_in_inventory(&inv, "herb_a"), 5);
    }

    #[test]
    fn consume_materials_returns_err_on_underflow() {
        let mut inv = make_inventory(&[("herb_a", 1)]);
        let err = consume_materials_from_inventory(&mut inv, "herb_a", 5).unwrap_err();
        assert_eq!(err.template_id, "herb_a");
        assert_eq!(err.need, 4);
    }

    // ============= start_craft =============

    #[test]
    fn start_craft_happy_path_writes_ledger_and_session() {
        let (registry, unlock, mut cult, color, mut ledger) = make_world();
        let mut inv = make_inventory(&[("herb_a", 5), ("iron_needle", 5)]);
        let caster = caster_entity();

        let result = start_craft(
            StartCraftRequest {
                caster,
                player_id: "offline:Alice",
                recipe_id: &RecipeId::new("a"),
                current_tick: 1000,
                zone_id: "spawn",
                quantity: 1,
            },
            ok_deps_for_player(&registry, &unlock, &mut inv, &mut cult, &color, &mut ledger),
        )
        .unwrap();

        // session 形态
        assert_eq!(result.session.recipe_id.as_str(), "a");
        assert_eq!(result.session.started_at_tick, 1000);
        assert_eq!(result.session.remaining_ticks, 100);
        assert_eq!(result.session.qi_paid, 5.0);

        // 材料扣减
        assert_eq!(count_template_in_inventory(&inv, "herb_a"), 3);
        assert_eq!(count_template_in_inventory(&inv, "iron_needle"), 2);

        // qi 守恒：cultivation 扣 5，ledger zone 余额 +5
        assert_eq!(cult.qi_current, 45.0);
        let zone_balance = ledger.balance(&QiAccountId::zone("spawn"));
        assert_eq!(zone_balance, 5.0);

        // 守恒律观察：qi_paid 与 ledger transfer 等同
        assert_eq!(result.session.qi_paid, 5.0);
        assert_eq!(result.event.qi_paid, 5.0);
    }

    #[test]
    fn start_craft_batch_reserves_all_materials_and_qi_upfront() {
        let (registry, unlock, mut cult, color, mut ledger) = make_world();
        let mut inv = make_inventory(&[("herb_a", 8), ("iron_needle", 10)]);
        let result = start_craft(
            StartCraftRequest {
                caster: caster_entity(),
                player_id: "offline:Alice",
                recipe_id: &RecipeId::new("a"),
                current_tick: 1000,
                zone_id: "spawn",
                quantity: 3,
            },
            ok_deps_for_player(&registry, &unlock, &mut inv, &mut cult, &color, &mut ledger),
        )
        .unwrap();

        assert_eq!(result.session.quantity_total, 3);
        assert_eq!(result.session.completed_count, 0);
        assert_eq!(result.session.qi_paid, 15.0);
        assert_eq!(count_template_in_inventory(&inv, "herb_a"), 2);
        assert_eq!(count_template_in_inventory(&inv, "iron_needle"), 1);
        assert_eq!(cult.qi_current, 35.0);
        assert_eq!(ledger.balance(&QiAccountId::zone("spawn")), 15.0);
    }

    #[test]
    fn start_craft_rejects_unknown_recipe() {
        let (registry, unlock, mut cult, color, mut ledger) = make_world();
        let mut inv = make_inventory(&[("herb_a", 5), ("iron_needle", 5)]);
        let err = start_craft(
            StartCraftRequest {
                caster: caster_entity(),
                player_id: "offline:Alice",
                recipe_id: &RecipeId::new("missing"),
                current_tick: 0,
                zone_id: "spawn",
                quantity: 1,
            },
            ok_deps_for_player(&registry, &unlock, &mut inv, &mut cult, &color, &mut ledger),
        )
        .unwrap_err();
        assert!(matches!(err, StartCraftError::UnknownRecipe(_)));
    }

    #[test]
    fn start_craft_rejects_locked_recipe() {
        let (registry, _unlock, mut cult, color, mut ledger) = make_world();
        let unlock = RecipeUnlockState::new(); // 空 unlock state
        let mut inv = make_inventory(&[("herb_a", 5), ("iron_needle", 5)]);
        let err = start_craft(
            StartCraftRequest {
                caster: caster_entity(),
                player_id: "offline:Alice",
                recipe_id: &RecipeId::new("a"),
                current_tick: 0,
                zone_id: "spawn",
                quantity: 1,
            },
            ok_deps_for_player(&registry, &unlock, &mut inv, &mut cult, &color, &mut ledger),
        )
        .unwrap_err();
        assert!(matches!(err, StartCraftError::NotUnlocked(_)));
        // 失败时无副作用：材料仍在
        assert_eq!(count_template_in_inventory(&inv, "herb_a"), 5);
    }

    #[test]
    fn start_craft_rejects_when_session_already_exists() {
        let (registry, unlock, mut cult, color, mut ledger) = make_world();
        let mut inv = make_inventory(&[("herb_a", 5), ("iron_needle", 5)]);
        let existing = CraftSession {
            recipe_id: RecipeId::new("a"),
            started_at_tick: 0,
            remaining_ticks: 50,
            total_ticks: 100,
            owner_player_id: "offline:Alice".into(),
            qi_paid: 5.0,
            quantity_total: 1,
            completed_count: 0,
        };
        let mut deps =
            ok_deps_for_player(&registry, &unlock, &mut inv, &mut cult, &color, &mut ledger);
        deps.existing_session = Some(&existing);
        let err = start_craft(
            StartCraftRequest {
                caster: caster_entity(),
                player_id: "offline:Alice",
                recipe_id: &RecipeId::new("a"),
                current_tick: 0,
                zone_id: "spawn",
                quantity: 1,
            },
            deps,
        )
        .unwrap_err();
        assert_eq!(err, StartCraftError::AlreadyHasSession);
    }

    #[test]
    fn start_craft_rejects_missing_materials_with_full_deficit_list() {
        let (registry, unlock, mut cult, color, mut ledger) = make_world();
        let mut inv = make_inventory(&[("herb_a", 1)]); // need 2 + iron_needle 3
        let err = start_craft(
            StartCraftRequest {
                caster: caster_entity(),
                player_id: "offline:Alice",
                recipe_id: &RecipeId::new("a"),
                current_tick: 0,
                zone_id: "spawn",
                quantity: 1,
            },
            ok_deps_for_player(&registry, &unlock, &mut inv, &mut cult, &color, &mut ledger),
        )
        .unwrap_err();
        match err {
            StartCraftError::MissingMaterials(deficits) => {
                assert_eq!(deficits.len(), 2);
                let herb = deficits.iter().find(|d| d.template_id == "herb_a").unwrap();
                assert_eq!(herb.have, 1);
                assert_eq!(herb.need, 2);
                let iron = deficits
                    .iter()
                    .find(|d| d.template_id == "iron_needle")
                    .unwrap();
                assert_eq!(iron.have, 0);
                assert_eq!(iron.need, 3);
            }
            other => panic!("expected MissingMaterials, got {other:?}"),
        }
        // 失败时不扣材料
        assert_eq!(count_template_in_inventory(&inv, "herb_a"), 1);
    }

    #[test]
    fn start_craft_rejects_insufficient_qi() {
        let (registry, unlock, mut _ignored, color, mut ledger) = make_world();
        let mut cult = Cultivation {
            qi_current: 2.0, // recipe 要 5
            qi_max: 80.0,
            ..Default::default()
        };
        let mut inv = make_inventory(&[("herb_a", 5), ("iron_needle", 5)]);
        let err = start_craft(
            StartCraftRequest {
                caster: caster_entity(),
                player_id: "offline:Alice",
                recipe_id: &RecipeId::new("a"),
                current_tick: 0,
                zone_id: "spawn",
                quantity: 1,
            },
            ok_deps_for_player(&registry, &unlock, &mut inv, &mut cult, &color, &mut ledger),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            StartCraftError::InsufficientQi {
                have: 2.0,
                need: 5.0
            }
        ));
        // 失败时不扣材料
        assert_eq!(count_template_in_inventory(&inv, "herb_a"), 5);
    }

    #[test]
    fn start_craft_rejects_realm_too_low() {
        let mut registry = CraftRegistry::new();
        let mut recipe = simple_recipe("a");
        recipe.requirements.realm_min = Some(Realm::Solidify);
        registry.register(recipe).unwrap();

        let mut unlock = RecipeUnlockState::new();
        unlock.unlock("offline:Alice", RecipeId::new("a"));
        let mut cult = Cultivation {
            qi_current: 50.0,
            qi_max: 80.0,
            realm: Realm::Awaken,
            ..Default::default()
        };
        let color = QiColor::default();
        let mut ledger = WorldQiAccount::default();
        let mut inv = make_inventory(&[("herb_a", 5), ("iron_needle", 5)]);

        let err = start_craft(
            StartCraftRequest {
                caster: caster_entity(),
                player_id: "offline:Alice",
                recipe_id: &RecipeId::new("a"),
                current_tick: 0,
                zone_id: "spawn",
                quantity: 1,
            },
            ok_deps_for_player(&registry, &unlock, &mut inv, &mut cult, &color, &mut ledger),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            StartCraftError::RealmTooLow {
                required: Realm::Solidify,
                current: Realm::Awaken
            }
        ));
    }

    #[test]
    fn start_craft_rejects_qi_color_mismatch() {
        let mut registry = CraftRegistry::new();
        let mut recipe = simple_recipe("a");
        recipe.requirements.qi_color_min = Some((ColorKind::Insidious, 0.05));
        registry.register(recipe).unwrap();
        let mut unlock = RecipeUnlockState::new();
        unlock.unlock("offline:Alice", RecipeId::new("a"));
        let mut cult = Cultivation {
            qi_current: 50.0,
            qi_max: 80.0,
            ..Default::default()
        };
        let color = QiColor {
            main: ColorKind::Mellow, // 不是 Insidious
            ..Default::default()
        };
        let mut ledger = WorldQiAccount::default();
        let mut inv = make_inventory(&[("herb_a", 5), ("iron_needle", 5)]);
        let err = start_craft(
            StartCraftRequest {
                caster: caster_entity(),
                player_id: "offline:Alice",
                recipe_id: &RecipeId::new("a"),
                current_tick: 0,
                zone_id: "spawn",
                quantity: 1,
            },
            ok_deps_for_player(&registry, &unlock, &mut inv, &mut cult, &color, &mut ledger),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            StartCraftError::QiColorMismatch {
                required: ColorKind::Insidious,
                current: ColorKind::Mellow
            }
        ));
    }

    #[test]
    fn start_craft_zero_qi_recipe_skips_ledger_transfer() {
        let mut registry = CraftRegistry::new();
        let mut recipe = simple_recipe("a");
        recipe.qi_cost = 0.0;
        registry.register(recipe).unwrap();
        let mut unlock = RecipeUnlockState::new();
        unlock.unlock("offline:Alice", RecipeId::new("a"));
        let mut cult = Cultivation {
            qi_current: 0.0, // 零 qi 也能起手
            qi_max: 80.0,
            ..Default::default()
        };
        let color = QiColor::default();
        let mut ledger = WorldQiAccount::default();
        let mut inv = make_inventory(&[("herb_a", 5), ("iron_needle", 5)]);
        let result = start_craft(
            StartCraftRequest {
                caster: caster_entity(),
                player_id: "offline:Alice",
                recipe_id: &RecipeId::new("a"),
                current_tick: 0,
                zone_id: "spawn",
                quantity: 1,
            },
            ok_deps_for_player(&registry, &unlock, &mut inv, &mut cult, &color, &mut ledger),
        )
        .unwrap();
        assert_eq!(result.session.qi_paid, 0.0);
        // ledger 无 transfer 落地
        assert_eq!(ledger.transfers().len(), 0);
        assert_eq!(cult.qi_current, 0.0);
    }

    // ============= tick_session =============

    #[test]
    fn tick_session_decrements_remaining() {
        let mut session = CraftSession {
            recipe_id: RecipeId::new("a"),
            started_at_tick: 0,
            remaining_ticks: 100,
            total_ticks: 100,
            owner_player_id: "offline:Alice".into(),
            qi_paid: 5.0,
            quantity_total: 1,
            completed_count: 0,
        };
        let done = tick_session(&mut session, 30);
        assert!(!done);
        assert_eq!(session.remaining_ticks, 70);
    }

    #[test]
    fn tick_session_completes_at_zero() {
        let mut session = CraftSession {
            recipe_id: RecipeId::new("a"),
            started_at_tick: 0,
            remaining_ticks: 5,
            total_ticks: 100,
            owner_player_id: "offline:Alice".into(),
            qi_paid: 0.0,
            quantity_total: 1,
            completed_count: 0,
        };
        let done = tick_session(&mut session, 5);
        assert!(done);
        assert_eq!(session.remaining_ticks, 0);
    }

    #[test]
    fn tick_session_overshoot_clamps_to_zero() {
        let mut session = CraftSession {
            recipe_id: RecipeId::new("a"),
            started_at_tick: 0,
            remaining_ticks: 5,
            total_ticks: 100,
            owner_player_id: "offline:Alice".into(),
            qi_paid: 0.0,
            quantity_total: 1,
            completed_count: 0,
        };
        let done = tick_session(&mut session, 100);
        assert!(done);
        assert_eq!(session.remaining_ticks, 0);
    }

    #[test]
    fn tick_session_with_zero_amount_is_noop() {
        let mut session = CraftSession {
            recipe_id: RecipeId::new("a"),
            started_at_tick: 0,
            remaining_ticks: 50,
            total_ticks: 100,
            owner_player_id: "offline:Alice".into(),
            qi_paid: 0.0,
            quantity_total: 1,
            completed_count: 0,
        };
        let done = tick_session(&mut session, 0);
        assert!(!done);
        assert_eq!(session.remaining_ticks, 50);
    }

    #[test]
    fn tick_session_already_complete_is_idempotent() {
        let mut session = CraftSession {
            recipe_id: RecipeId::new("a"),
            started_at_tick: 0,
            remaining_ticks: 0,
            total_ticks: 100,
            owner_player_id: "offline:Alice".into(),
            qi_paid: 0.0,
            quantity_total: 1,
            completed_count: 0,
        };
        let done = tick_session(&mut session, 50);
        assert!(done);
        assert_eq!(session.remaining_ticks, 0);
    }

    // ============= cancel_craft =============

    #[test]
    fn cancel_craft_returns_70pct_floor() {
        let recipe = simple_recipe("a"); // herb_a×2, iron_needle×3
        let session = CraftSession {
            recipe_id: RecipeId::new("a"),
            started_at_tick: 0,
            remaining_ticks: 50,
            total_ticks: 100,
            owner_player_id: "offline:Alice".into(),
            qi_paid: 5.0,
            quantity_total: 1,
            completed_count: 0,
        };
        let outcome = cancel_craft(
            &session,
            &recipe,
            caster_entity(),
            CraftFailureReason::PlayerCancelled,
        );
        // herb_a: floor(2 * 0.7) = 1
        // iron_needle: floor(3 * 0.7) = 2
        let map: HashMap<&str, u32> = outcome
            .refund_manifest
            .iter()
            .map(|(t, n)| (t.as_str(), *n))
            .collect();
        assert_eq!(map.get("herb_a"), Some(&1));
        assert_eq!(map.get("iron_needle"), Some(&2));
        assert_eq!(outcome.event.material_returned, 3);
        assert_eq!(outcome.event.qi_refunded, 0.0); // §5 决策门 #3
    }

    #[test]
    fn cancel_craft_filters_zero_refund_entries() {
        let mut recipe = simple_recipe("a");
        recipe.materials = vec![("herb_a".into(), 1)]; // floor(1 * 0.7) = 0
        let session = CraftSession {
            recipe_id: RecipeId::new("a"),
            started_at_tick: 0,
            remaining_ticks: 50,
            total_ticks: 100,
            owner_player_id: "offline:Alice".into(),
            qi_paid: 0.0,
            quantity_total: 1,
            completed_count: 0,
        };
        let outcome = cancel_craft(
            &session,
            &recipe,
            caster_entity(),
            CraftFailureReason::PlayerCancelled,
        );
        assert!(outcome.refund_manifest.is_empty());
        assert_eq!(outcome.event.material_returned, 0);
    }

    #[test]
    fn cancel_craft_batch_refunds_unfinished_quantity() {
        let recipe = simple_recipe("a"); // herb_a×2, iron_needle×3
        let session = CraftSession {
            recipe_id: RecipeId::new("a"),
            started_at_tick: 0,
            remaining_ticks: 50,
            total_ticks: 100,
            owner_player_id: "offline:Alice".into(),
            qi_paid: 15.0,
            quantity_total: 3,
            completed_count: 1,
        };
        let outcome = cancel_craft(
            &session,
            &recipe,
            caster_entity(),
            CraftFailureReason::PlayerCancelled,
        );
        let map: HashMap<&str, u32> = outcome
            .refund_manifest
            .iter()
            .map(|(t, n)| (t.as_str(), *n))
            .collect();
        // 剩余 2 件：herb_a floor(2*2*0.7)=2；iron_needle floor(3*2*0.7)=4
        assert_eq!(map.get("herb_a"), Some(&2));
        assert_eq!(map.get("iron_needle"), Some(&4));
        assert_eq!(outcome.event.material_returned, 6);
    }

    #[test]
    fn cancel_craft_propagates_player_died_reason() {
        let recipe = simple_recipe("a");
        let session = CraftSession {
            recipe_id: RecipeId::new("a"),
            started_at_tick: 0,
            remaining_ticks: 50,
            total_ticks: 100,
            owner_player_id: "offline:Alice".into(),
            qi_paid: 5.0,
            quantity_total: 1,
            completed_count: 0,
        };
        let outcome = cancel_craft(
            &session,
            &recipe,
            caster_entity(),
            CraftFailureReason::PlayerDied,
        );
        assert_eq!(outcome.event.reason, CraftFailureReason::PlayerDied);
    }

    #[test]
    fn cancel_craft_propagates_internal_error_reason() {
        let recipe = simple_recipe("a");
        let session = CraftSession {
            recipe_id: RecipeId::new("a"),
            started_at_tick: 0,
            remaining_ticks: 50,
            total_ticks: 100,
            owner_player_id: "offline:Alice".into(),
            qi_paid: 0.0,
            quantity_total: 1,
            completed_count: 0,
        };
        let outcome = cancel_craft(
            &session,
            &recipe,
            caster_entity(),
            CraftFailureReason::InternalError,
        );
        assert_eq!(outcome.event.reason, CraftFailureReason::InternalError);
    }

    // ============= finalize_craft =============

    #[test]
    fn finalize_craft_returns_output_manifest() {
        let mut recipe = simple_recipe("a");
        recipe.output = ("eclipse_needle_iron".into(), 3);
        let session = CraftSession {
            recipe_id: RecipeId::new("a"),
            started_at_tick: 100,
            remaining_ticks: 0,
            total_ticks: 100,
            owner_player_id: "offline:Alice".into(),
            qi_paid: 5.0,
            quantity_total: 1,
            completed_count: 0,
        };
        let outcome = finalize_craft(&session, &recipe, caster_entity(), 200);
        assert_eq!(outcome.event.completed_at_tick, 200);
        assert_eq!(outcome.event.output_template, "eclipse_needle_iron");
        assert_eq!(outcome.event.output_count, 3);
        assert_eq!(outcome.output_manifest, ("eclipse_needle_iron".into(), 3));
    }

    // ============= 守恒律端到端 =============

    #[test]
    fn start_craft_ledger_amount_matches_session_qi_paid() {
        // 守恒律观察值断言 — qi_paid 必须等同 ledger transfer amount
        let (registry, unlock, mut cult, color, mut ledger) = make_world();
        let mut inv = make_inventory(&[("herb_a", 5), ("iron_needle", 5)]);

        let result = start_craft(
            StartCraftRequest {
                caster: caster_entity(),
                player_id: "offline:Alice",
                recipe_id: &RecipeId::new("a"),
                current_tick: 0,
                zone_id: "spawn",
                quantity: 1,
            },
            ok_deps_for_player(&registry, &unlock, &mut inv, &mut cult, &color, &mut ledger),
        )
        .unwrap();

        // 找最近一次 transfer
        let last_transfer = ledger
            .transfers()
            .last()
            .expect("ledger should have transfer");
        assert_eq!(last_transfer.amount, result.session.qi_paid);
        assert_eq!(last_transfer.reason, QiTransferReason::Crafting);
        assert_eq!(last_transfer.from, QiAccountId::player("offline:Alice"));
        assert_eq!(last_transfer.to, QiAccountId::zone("spawn"));
    }

    #[test]
    fn start_craft_unlock_via_insight_then_run() {
        // 集成：先用 insight 解锁，然后 start_craft 跑通
        let mut registry = CraftRegistry::new();
        let mut recipe = simple_recipe("a");
        recipe.unlock_sources = vec![UnlockSource::Insight {
            trigger: InsightTrigger::Breakthrough,
        }];
        registry.register(recipe).unwrap();
        let mut unlock = RecipeUnlockState::new();
        let recipe_ref = registry.get(&RecipeId::new("a")).unwrap();
        let outcome = super::super::unlock::unlock_via_insight(
            &mut unlock,
            "offline:Alice",
            recipe_ref,
            InsightTrigger::Breakthrough,
        );
        assert!(matches!(
            outcome,
            super::super::unlock::UnlockOutcome::Newly {
                source: UnlockEventSource::Insight {
                    trigger: InsightTrigger::Breakthrough
                }
            }
        ));

        let mut cult = Cultivation {
            qi_current: 50.0,
            qi_max: 80.0,
            ..Default::default()
        };
        let color = QiColor::default();
        let mut ledger = WorldQiAccount::default();
        // sync ledger to cultivation（模拟 sync system 行为）
        ledger
            .set_balance(QiAccountId::player("offline:Alice"), cult.qi_current)
            .unwrap();
        let mut inv = make_inventory(&[("herb_a", 5), ("iron_needle", 5)]);
        let success = start_craft(
            StartCraftRequest {
                caster: caster_entity(),
                player_id: "offline:Alice",
                recipe_id: &RecipeId::new("a"),
                current_tick: 0,
                zone_id: "spawn",
                quantity: 1,
            },
            ok_deps_for_player(&registry, &unlock, &mut inv, &mut cult, &color, &mut ledger),
        )
        .unwrap();
        assert_eq!(success.session.qi_paid, 5.0);
    }

    // ============= 守恒 / ledger sync 不变量 =============

    #[test]
    fn ledger_player_balance_aligned_with_cultivation_after_start() {
        // 不变量：start_craft 完成后，player 账户的 ledger 余额 ==
        // cultivation.qi_current_post（即扣完后的 state view）。
        // 前提：调用方已 sync 过 ledger.player(id) = cultivation.qi_current
        // （make_world helper 已在 setup 阶段执行）。
        let (registry, unlock, mut cult, color, mut ledger) = make_world();
        let mut inv = make_inventory(&[("herb_a", 5), ("iron_needle", 5)]);
        let qi_before = cult.qi_current;
        let zone_before = ledger.balance(&QiAccountId::zone("spawn"));

        start_craft(
            StartCraftRequest {
                caster: caster_entity(),
                player_id: "offline:Alice",
                recipe_id: &RecipeId::new("a"),
                current_tick: 0,
                zone_id: "spawn",
                quantity: 1,
            },
            ok_deps_for_player(&registry, &unlock, &mut inv, &mut cult, &color, &mut ledger),
        )
        .unwrap();

        // recipe.qi_cost = 5.0（make_world / simple_recipe）
        let qi_paid = 5.0_f64;
        assert_eq!(cult.qi_current, qi_before - qi_paid);
        let player_after = ledger.balance(&QiAccountId::player("offline:Alice"));
        assert_eq!(
            player_after, cult.qi_current,
            "player ledger balance must mirror cultivation.qi_current after transfer"
        );
        let zone_after = ledger.balance(&QiAccountId::zone("spawn"));
        assert_eq!(
            zone_after,
            zone_before + qi_paid,
            "zone account must gain exactly qi_cost"
        );
    }

    #[test]
    fn start_craft_with_synced_ledger_does_not_inflate_player_balance() {
        // 不变量：当调用方先把 ledger.player(id) 同步到 cultivation.qi_current 后，
        // start_craft **不会**额外注入余额到 player 账户（防 set_balance leak）。
        // post 状态：player_balance == cult.qi_current_post == pre - qi_cost。
        let (registry, unlock, mut cult, color, mut ledger) = make_world();
        let mut inv = make_inventory(&[("herb_a", 5), ("iron_needle", 5)]);
        let player_pre = ledger.balance(&QiAccountId::player("offline:Alice"));
        assert_eq!(player_pre, 50.0, "make_world should sync ledger to 50.0");

        start_craft(
            StartCraftRequest {
                caster: caster_entity(),
                player_id: "offline:Alice",
                recipe_id: &RecipeId::new("a"),
                current_tick: 0,
                zone_id: "spawn",
                quantity: 1,
            },
            ok_deps_for_player(&registry, &unlock, &mut inv, &mut cult, &color, &mut ledger),
        )
        .unwrap();

        let player_post = ledger.balance(&QiAccountId::player("offline:Alice"));
        // post == pre - qi_cost（5.0）
        assert!((player_pre - player_post - 5.0).abs() < 1e-9);
        assert_eq!(player_post, cult.qi_current);
    }

    #[test]
    fn ledger_total_conservation_after_start_craft() {
        // 守恒律：ledger 内部总量在 start_craft 前后相等
        // （player → zone 的 transfer 是账内移动，不增减总数）。
        // cultivation.qi_current 是 ledger.player 的 view，不参与 ledger.total()。
        let (registry, unlock, mut cult, color, mut ledger) = make_world();
        let mut inv = make_inventory(&[("herb_a", 5), ("iron_needle", 5)]);
        let ledger_total_before = ledger.total();

        start_craft(
            StartCraftRequest {
                caster: caster_entity(),
                player_id: "offline:Alice",
                recipe_id: &RecipeId::new("a"),
                current_tick: 0,
                zone_id: "spawn",
                quantity: 1,
            },
            ok_deps_for_player(&registry, &unlock, &mut inv, &mut cult, &color, &mut ledger),
        )
        .unwrap();

        let ledger_total_after = ledger.total();
        assert!(
            (ledger_total_before - ledger_total_after).abs() < 1e-9,
            "ledger.total() before {ledger_total_before} must equal after {ledger_total_after}"
        );
    }

    #[test]
    fn start_craft_rejects_when_ledger_out_of_sync() {
        // 守恒律强制：调用方未 sync ledger.player 到 cultivation.qi_current 时，
        // start_craft 必须 fail-fast（避免 ad-hoc set_balance 注入）。
        let mut registry = CraftRegistry::new();
        registry.register(simple_recipe("a")).unwrap();
        let mut unlock = RecipeUnlockState::new();
        unlock.unlock("offline:Alice", RecipeId::new("a"));
        let mut cult = Cultivation {
            qi_current: 50.0,
            qi_max: 80.0,
            ..Default::default()
        };
        let color = QiColor::default();
        // 故意**不** sync ledger — player 账户余额 0
        let mut ledger = WorldQiAccount::default();
        let mut inv = make_inventory(&[("herb_a", 5), ("iron_needle", 5)]);

        let err = start_craft(
            StartCraftRequest {
                caster: caster_entity(),
                player_id: "offline:Alice",
                recipe_id: &RecipeId::new("a"),
                current_tick: 0,
                zone_id: "spawn",
                quantity: 1,
            },
            ok_deps_for_player(&registry, &unlock, &mut inv, &mut cult, &color, &mut ledger),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            StartCraftError::LedgerOutOfSync {
                player_balance: 0.0,
                cultivation_qi_current: 50.0,
                required: 5.0,
            }
        ));
        // 失败时无副作用：cultivation 不动 / 材料不动
        assert_eq!(cult.qi_current, 50.0);
        assert_eq!(count_template_in_inventory(&inv, "herb_a"), 5);
    }

    #[test]
    fn start_craft_rejects_ledger_overshoot_relative_to_cultivation() {
        // 严格 sync 校验：即使 ledger.balance(player) > qi_cost 但 ≠
        // cultivation.qi_current，也属于 desync 必须 reject。
        // 防止"ledger 凭空多 200 但 cultivation 只 50"误算守恒。
        let mut registry = CraftRegistry::new();
        registry.register(simple_recipe("a")).unwrap();
        let mut unlock = RecipeUnlockState::new();
        unlock.unlock("offline:Alice", RecipeId::new("a"));
        let mut cult = Cultivation {
            qi_current: 50.0,
            qi_max: 80.0,
            ..Default::default()
        };
        let color = QiColor::default();
        // ledger 余额 200 > cultivation 50：明显 desync（不应当通过）
        let mut ledger = WorldQiAccount::default();
        ledger
            .set_balance(QiAccountId::player("offline:Alice"), 200.0)
            .unwrap();
        let mut inv = make_inventory(&[("herb_a", 5), ("iron_needle", 5)]);

        let err = start_craft(
            StartCraftRequest {
                caster: caster_entity(),
                player_id: "offline:Alice",
                recipe_id: &RecipeId::new("a"),
                current_tick: 0,
                zone_id: "spawn",
                quantity: 1,
            },
            ok_deps_for_player(&registry, &unlock, &mut inv, &mut cult, &color, &mut ledger),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            StartCraftError::LedgerOutOfSync {
                player_balance: 200.0,
                cultivation_qi_current: 50.0,
                required: 5.0,
            }
        ));
        // 失败时无副作用：余额 / 材料 / cultivation 不动
        assert_eq!(cult.qi_current, 50.0);
        assert_eq!(ledger.balance(&QiAccountId::player("offline:Alice")), 200.0);
        assert_eq!(count_template_in_inventory(&inv, "herb_a"), 5);
    }
}
