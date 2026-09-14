//! plan-craft-v1 §3 — `CraftSession` component + 状态机。
//!
//! §0 设计轴心：
//!   * **单任务**：玩家同时只允许一个 CraftSession 存在，新 start 必须先 cancel
//!   * **in-game 时间推进**：只有 `tick_session` 显式推进时才走，玩家下线
//!     （inventory 关闭）时调用方不调用 tick，自动暂停
//!   * **守恒律**：qi_cost 一次性走 `qi_physics::ledger::transfer_external_qi_to_ledger`
//!     （Crafting reason），把 ECS 真元权威转入持久待分配池
//!
//! §5 决策门：
//!   * #3 = B：取消任务返还材料 70%（向下取整），qi 不退
//!   * #4 = A：玩家死亡 → 走 cancel 路径，PlayerDied 作为 reason
//!   * #6 = B：requirements 软 gate，但 `start_craft` 内做硬校验防作弊

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component, Entity};

use crate::cultivation::components::{ColorKind, Cultivation, QiColor, Realm};
use crate::inventory::{bump_revision, ContainerState, ItemInstance, PlayerInventory};
use crate::qi_physics::ledger::{
    pending_inflow_account, transfer_external_qi_to_ledger, QiAccountId, QiTransferReason,
    WorldQiAccount,
};
use crate::skill::components::SkillSet;
use crate::skill::curve::effective_lv;

use super::events::{CraftCompletedEvent, CraftFailedEvent, CraftFailureReason, CraftStartedEvent};
use super::recipe::{CraftRecipe, RecipeId};
use super::registry::CraftRegistry;
use super::unlock::RecipeUnlockState;

/// §5 决策门 #3 = B：取消返还 70%，30% 损耗惩罚。
pub const CANCEL_REFUND_RATIO: f64 = 0.7;

/// 单次手搓批量上限。client UI 与 schema 同步使用同一个语义上限。
pub const MAX_CRAFT_QUANTITY: u32 = 64;

/// 玩家进行中的手搓任务。
/// 玩家只允许同时挂 1 个 CraftSession（单任务）。`remaining_ticks` 由
/// `tick_session` 在玩家在线时推进；为 0 时调用 `finalize_craft`。
#[derive(Debug, Clone, Component, PartialEq, Serialize, Deserialize)]
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
    /// 技能等级不足。
    SkillTooLow { required: u8, current: u8 },
    /// ledger 内部错误（transfer 失败等）
    LedgerError(String),
    /// 批量数量必须 >= 1。
    InvalidQuantity(u32),
    /// 批量数量超过服务端硬上限。
    QuantityTooLarge { requested: u32, max: u32 },
    /// plan-workbench-recipes-v1 §P2.4：配方需要制作台但玩家 3 格内没有。
    StationOutOfRange,
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
    /// 调用方需要负责真实返还（入包或落地兜底）。
    pub refund_manifest: Vec<(String, u32)>,
}

/// 完成手搓的产出包：产出物 + 完成事件。
#[derive(Debug, Clone)]
pub struct FinalizeCraftOutcome {
    pub event: CraftCompletedEvent,
    /// 产出：(template_id, count)。
    /// 调用方需要负责真实写入 inventory。
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
    let mut consumed_any = false;
    // 先吃 containers
    'containers: for container in inventory.containers.iter_mut() {
        let mut i = 0;
        while i < container.items.len() {
            if container.items[i].instance.template_id == template_id {
                let take = needed.min(container.items[i].instance.stack_count);
                container.items[i].instance.stack_count -= take;
                needed -= take;
                consumed_any |= take > 0;
                if container.items[i].instance.stack_count == 0 {
                    container.items.remove(i);
                    continue;
                }
            }
            i += 1;
            if needed == 0 {
                break 'containers;
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
                consumed_any |= take > 0;
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
    if consumed_any {
        bump_revision(inventory);
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
    /// 玩家技能状态；缺失时所有技能等级按 0 处理。
    pub skill_set: Option<&'a SkillSet>,
    /// plan-workbench-recipes-v1 §P2.4：玩家 3 格内是否有制作台。
    /// `true` = 有（或不需要），`false` = 没有。
    /// 对 station=None 的配方此字段被忽略。
    pub has_nearby_workbench: bool,
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
/// 7. 把 ECS 玩家真元原子转入 durable pending inflow（reason = Crafting），再提交
///    `cultivation.qi_current -= qi_cost`；player ledger 账户不做长期镜像
/// 8. 扣材料
/// 9. 构造 CraftSession + CraftStartedEvent
pub fn start_craft(
    request: StartCraftRequest<'_>,
    deps: StartCraftDeps<'_>,
) -> Result<StartCraftSuccess, StartCraftError> {
    if request.quantity == 0 {
        return Err(StartCraftError::InvalidQuantity(request.quantity));
    }
    if request.quantity > MAX_CRAFT_QUANTITY {
        return Err(StartCraftError::QuantityTooLarge {
            requested: request.quantity,
            max: MAX_CRAFT_QUANTITY,
        });
    }
    let recipe = deps
        .registry
        .get(request.recipe_id)
        .ok_or_else(|| StartCraftError::UnknownRecipe(request.recipe_id.clone()))?;

    // plan-craft-material-discovery：所有配方都必须先解锁才能制作。
    // 无显式 unlock_sources 的基础配方不再"空源即默认解锁"，而是通过
    // "持有任一原料"被动解锁（unlock::unlock_via_material +
    // craft_emit::apply_material_discovery_unlock）；残卷/师承/顿悟门控的
    // 秘传配方仍走对应渠道写入 unlock_state。两者最终都收敛到 is_unlocked。
    // 例外：unlock::BASELINE_RECIPES（制作台自身）恒解锁，由 is_unlocked 单点豁免。
    if !deps.unlock_state.is_unlocked(request.player_id, &recipe.id) {
        return Err(StartCraftError::NotUnlocked(recipe.id.clone()));
    }

    if deps.existing_session.is_some() {
        return Err(StartCraftError::AlreadyHasSession);
    }

    // plan-workbench-recipes-v1 §P2.4：station 校验。
    // station=Some(Workbench) → 玩家 3 格内必须有 WorkbenchBlock。
    // station=None → 手搓，不受制作台距离限制。
    if recipe.station.is_some() && !deps.has_nearby_workbench {
        return Err(StartCraftError::StationOutOfRange);
    }

    if let Some(min) = recipe.requirements.skill_lv_min {
        let current = deps
            .skill_set
            .and_then(|set| set.skills.values().map(|entry| entry.lv).max())
            .map(|real_lv| {
                effective_lv(
                    real_lv,
                    crate::cultivation::breakthrough::skill_cap_for_realm(deps.cultivation.realm),
                )
            })
            .unwrap_or(0);
        if current < min {
            return Err(StartCraftError::SkillTooLow {
                required: min,
                current,
            });
        }
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
    let to = pending_inflow_account();
    if total_qi_cost > 0.0 {
        transfer_external_qi_to_ledger(
            deps.ledger,
            from,
            to,
            total_qi_cost,
            QiTransferReason::Crafting,
        )
        .map_err(|error| StartCraftError::LedgerError(error.to_string()))?;

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
/// 不动 inventory / 不扣 qi；调用方按返还清单执行真实返还。
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
/// 不动 inventory；调用方按 output_manifest 执行真实产出写入。
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
#[path = "session_tests.rs"]
mod tests;
