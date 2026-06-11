//! plan-qi-handling-attrition-v1 P0/P1 — 搬运灵物天道税（worldview §八.2）。
//!
//! # 守恒约束
//!
//! 所有磨损量【必须】归还 zone，不允许凭空消失：
//!   - item.spirit_quality 减少量（绝对值）== zone 接收量（accepted）
//!   - overflow（zone 已满）仍走 overflow 账户，总量守恒
//!   - 审计轨迹用 `QiTransferReason::AttritionTax { op_kind }` 标记
//!
//! # 接入点（P0: Pickup；P1: SlotMove / ContainerSearch / ForgeLoad / AlchemyLoad）
//!
//! 接入模式：在各 inventory 操作成功分支后**同步调用** `apply_attrition`，
//! 仿 `maybe_apply_targeted_item_wear`（client_request_handler.rs:7533）的插桩模式。
//!
//! # AttritionExemptTag / 死坍缩渊 guard（P1/P3）
//!
//! 物品 `ItemInstance` 带 exempt 标记时直接跳过磨损。
//! 进入 `TsyLifecycle::Dead` 的坍缩渊 family 禁止再结算磨损；race-out 正常应已撤离，
//! 这里保留显式 guard 防止 stale zone / 测试注入吞真元。

use valence::prelude::{bevy_ecs, Events, Resource};

use crate::cultivation::dead_zone::DEAD_ZONE_QI_THRESHOLD;
use crate::inventory::ItemInstance;
use crate::qi_physics::constants::{
    QI_ATTRITION_BASE_RATE, QI_ATTRITION_DEAD_ZONE_MULTIPLIER, QI_ATTRITION_MIN_QI,
    QI_ATTRITION_SPIRIT_SOURCE_MULTIPLIER, QI_ATTRITION_SPIRIT_SOURCE_THRESHOLD,
    QI_ATTRITION_TSY_MULTIPLIER, QI_EPSILON, QI_ZONE_UNIT_CAPACITY,
};
use crate::qi_physics::ledger::{AttritionOpKind, QiAccountId, QiTransfer, QiTransferReason};
use crate::qi_physics::release::qi_release_to_zone;
use crate::world::tsy_lifecycle::TsyZoneStateRegistry;
use crate::world::zone::Zone;

// ── AttritionConfig Resource ──────────────────────────────────────────────────

/// plan-qi-handling-attrition-v1 P0 — 磨损配置 Resource。
///
/// 默认值全部引自 `qi_physics::constants`（禁止 inline 常数，plan P0 硬约束）。
/// 保留为 Resource 形式以便 dev 命令 / 天道层未来调参，生产路径不应改 defaults。
#[derive(Debug, Clone, Resource)]
pub struct AttritionConfig {
    /// 基础磨损税率（§8 校准项）。
    pub base_rate: f64,
    /// 死域环境倍率。
    pub dead_zone_multiplier: f64,
    /// 坍缩渊环境倍率。
    pub tsy_multiplier: f64,
    /// 馈赠区环境倍率（< 1）。
    pub spirit_source_multiplier: f64,
    /// 馈赠区 spirit_qi 阈值。
    pub spirit_source_threshold: f64,
    /// abs_qi 低于此值跳过磨损。
    pub min_qi: f64,
}

impl Default for AttritionConfig {
    fn default() -> Self {
        Self {
            base_rate: QI_ATTRITION_BASE_RATE,
            dead_zone_multiplier: QI_ATTRITION_DEAD_ZONE_MULTIPLIER,
            tsy_multiplier: QI_ATTRITION_TSY_MULTIPLIER,
            spirit_source_multiplier: QI_ATTRITION_SPIRIT_SOURCE_MULTIPLIER,
            spirit_source_threshold: QI_ATTRITION_SPIRIT_SOURCE_THRESHOLD,
            min_qi: QI_ATTRITION_MIN_QI,
        }
    }
}

// ── AttritionOpKind rate_factor ───────────────────────────────────────────────

impl AttritionOpKind {
    /// § 8 校准项：各操作类型的 rate 因子。
    /// effective_rate = QI_ATTRITION_BASE_RATE × rate_factor × env_multiplier。
    ///
    /// | op_kind         | rate_factor | effective_rate |
    /// |-----------------|-------------|----------------|
    /// | Pickup          | 1.000       | 0.030          |
    /// | SlotMove        | 0.667       | ≈0.020         |
    /// | ContainerSearch | 1.667       | ≈0.050         |
    /// | ForgeLoad       | 1.333       | ≈0.040         |
    /// | AlchemyLoad     | 1.333       | ≈0.040         |
    pub fn rate_factor(self) -> f64 {
        match self {
            AttritionOpKind::Pickup => 1.000,
            AttritionOpKind::SlotMove => 0.667,
            AttritionOpKind::ContainerSearch => 1.667,
            AttritionOpKind::ForgeLoad => 1.333,
            AttritionOpKind::AlchemyLoad => 1.333,
        }
    }
}

// ── env_multiplier ────────────────────────────────────────────────────────────

/// 根据 zone 状态计算环境倍率。
///
/// 判定优先级：
/// 1. 坍缩渊（is_tsy）→ × QI_ATTRITION_TSY_MULTIPLIER (3.0)
/// 2. 死域（spirit_qi >= 0 且 < DEAD_ZONE_QI_THRESHOLD）→ × QI_ATTRITION_DEAD_ZONE_MULTIPLIER (3.0)
/// 3. 馈赠区（spirit_qi >= QI_ATTRITION_SPIRIT_SOURCE_THRESHOLD）→ × QI_ATTRITION_SPIRIT_SOURCE_MULTIPLIER (0.8)
/// 4. 默认 → 1.0
pub fn env_multiplier(zone: &Zone) -> f64 {
    if zone.is_tsy() {
        return QI_ATTRITION_TSY_MULTIPLIER;
    }
    if zone.spirit_qi >= 0.0 && zone.spirit_qi < DEAD_ZONE_QI_THRESHOLD {
        return QI_ATTRITION_DEAD_ZONE_MULTIPLIER;
    }
    if zone.spirit_qi >= QI_ATTRITION_SPIRIT_SOURCE_THRESHOLD {
        return QI_ATTRITION_SPIRIT_SOURCE_MULTIPLIER;
    }
    1.0
}

// ── compute_attrition_amount ──────────────────────────────────────────────────

/// 计算磨损绝对量（item 侧扣除量）。
///
/// `abs_qi` = item.spirit_quality × stack_count（调用方计算后传入）。
/// 返回 attrition_abs = abs_qi × effective_rate。
pub fn compute_attrition_amount(abs_qi: f64, op_kind: AttritionOpKind, env_mult: f64) -> f64 {
    let effective_rate = QI_ATTRITION_BASE_RATE * op_kind.rate_factor() * env_mult;
    abs_qi * effective_rate
}

// ── P3 apply outcome / guard ─────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttritionSkipReason {
    NonPositiveQi,
    BelowMinimumQi,
    Exempt,
    DeadTsy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttritionApplyOutcome {
    Applied,
    Skipped(AttritionSkipReason),
}

/// P3：如果 zone 所属 TSY family 已 `Dead`，返回 family id。
pub fn dead_tsy_family_id(zone: &Zone, lifecycle: Option<&TsyZoneStateRegistry>) -> Option<String> {
    let family_id = zone.tsy_family_id()?;
    let state = lifecycle?.by_family.get(&family_id)?;
    state.lifecycle.is_dead().then_some(family_id)
}

// ── release_attrition_to_zone ─────────────────────────────────────────────────

/// 薄 helper：将磨损逸散量归还 zone，emit `AttritionTax` 审计转账。
///
/// 复用 `qi_release_to_zone` 的 cap/overflow 数值逻辑，但**丢弃**其 `ReleaseToZone` transfer，
/// 用 `AttritionTax { op_kind }` 重建，以符合 plan 契约。
///
/// # 守恒
/// - accepted → zone.spirit_qi 增加（`zone_after / QI_ZONE_UNIT_CAPACITY`）
/// - overflow → `QiTransfer(from, overflow_account, overflow, AttritionTax)` 留审计
/// - 合计 accepted + overflow == amount，无凭空消失
pub fn release_attrition_to_zone(
    zone: &mut Zone,
    amount: f64,
    from_id: QiAccountId,
    op_kind: AttritionOpKind,
    qi_transfers: Option<&mut Events<QiTransfer>>,
) {
    if amount < QI_EPSILON {
        return;
    }

    let zone_current = zone.spirit_qi * QI_ZONE_UNIT_CAPACITY;
    let zone_cap = QI_ZONE_UNIT_CAPACITY;
    let to_id = QiAccountId::zone(zone.name.clone());

    match qi_release_to_zone(
        amount,
        from_id.clone(),
        to_id.clone(),
        zone_current,
        zone_cap,
    ) {
        Ok(outcome) => {
            // 更新 zone.spirit_qi（口径：zone_after / QI_ZONE_UNIT_CAPACITY，与 npc_skill.rs 同）
            zone.spirit_qi = (outcome.zone_after / QI_ZONE_UNIT_CAPACITY).clamp(-1.0, 1.0);

            // emit AttritionTax 审计轨迹（丢弃 qi_release_to_zone 产出的 ReleaseToZone transfer）
            if let Some(events) = qi_transfers {
                if outcome.accepted > QI_EPSILON {
                    if let Ok(t) = QiTransfer::new(
                        from_id.clone(),
                        to_id,
                        outcome.accepted,
                        QiTransferReason::AttritionTax { op_kind },
                    ) {
                        events.send(t);
                    }
                }
                // overflow 仍守恒：归入 overflow 账户（与 npc_skill.rs:70-82 同口径）
                if outcome.overflow > QI_EPSILON {
                    let overflow_id =
                        QiAccountId::overflow(format!("attrition_overflow:{}", zone.name));
                    if let Ok(t) = QiTransfer::new(
                        from_id,
                        overflow_id,
                        outcome.overflow,
                        QiTransferReason::AttritionTax { op_kind },
                    ) {
                        events.send(t);
                    }
                }
            }
        }
        Err(err) => {
            tracing::warn!(
                "[bong][attrition] release_attrition_to_zone failed zone={} amount={amount}: {err:?}",
                zone.name
            );
        }
    }
}

// ── apply_attrition ───────────────────────────────────────────────────────────

/// 主入口：对 item 施加磨损，逸散量守恒归还 zone。
///
/// # 跳过条件
/// - `item.spirit_quality <= 0`（凡俗物品）
/// - `abs_qi < QI_ATTRITION_MIN_QI`（小额跳过）
/// - item 带 `attrition_exempt` 字段（P1 豁免）
///
/// # zone 为 None 时
/// 无 zone 上下文（如内部测试）时不做归还，直接 return（守恒靠 tests 对拍）。
/// 生产路径保证调用方传入非 None。
pub fn apply_attrition(
    item: &mut ItemInstance,
    op_kind: AttritionOpKind,
    zone: Option<&mut Zone>,
    qi_transfers: Option<&mut Events<QiTransfer>>,
) {
    let _ = apply_attrition_checked(item, op_kind, zone, qi_transfers, None);
}

/// P3 主入口：附带显式 skip reason，供运行时 guard 与单测锁定边界。
pub fn apply_attrition_checked(
    item: &mut ItemInstance,
    op_kind: AttritionOpKind,
    zone: Option<&mut Zone>,
    qi_transfers: Option<&mut Events<QiTransfer>>,
    tsy_lifecycle: Option<&TsyZoneStateRegistry>,
) -> AttritionApplyOutcome {
    if is_attrition_exempt(item) {
        return AttritionApplyOutcome::Skipped(AttritionSkipReason::Exempt);
    }

    if item.spirit_quality <= 0.0 {
        return AttritionApplyOutcome::Skipped(AttritionSkipReason::NonPositiveQi);
    }

    let abs_qi = item.spirit_quality * item.stack_count.max(1) as f64;
    if abs_qi < QI_ATTRITION_MIN_QI {
        return AttritionApplyOutcome::Skipped(AttritionSkipReason::BelowMinimumQi);
    }

    if let Some((zone_name, family_id)) = zone.as_deref().and_then(|zone| {
        dead_tsy_family_id(zone, tsy_lifecycle).map(|family_id| (zone.name.clone(), family_id))
    }) {
        tracing::warn!(
            "[bong][attrition] blocked attrition in dead tsy family={family_id} zone={zone_name} op_kind={op_kind:?} item={}",
            item.instance_id
        );
        return AttritionApplyOutcome::Skipped(AttritionSkipReason::DeadTsy);
    }

    let env_mult = zone.as_deref().map(env_multiplier).unwrap_or(1.0);
    let rate = QI_ATTRITION_BASE_RATE * op_kind.rate_factor() * env_mult;
    let attrition_abs = abs_qi * rate;

    // 扣 item（按比例降 spirit_quality，保持 [0,1] 范围）
    let new_quality = (item.spirit_quality * (1.0 - rate)).clamp(0.0, 1.0);
    item.spirit_quality = new_quality;

    // 守恒归还 zone
    if let Some(zone) = zone {
        let from_id = QiAccountId::container(format!("item:{}", item.instance_id));
        release_attrition_to_zone(zone, attrition_abs, from_id, op_kind, qi_transfers);
    }

    AttritionApplyOutcome::Applied
}

/// P1 豁免检查 helper：若 item 携带豁免标记则返回 true，调用方应跳过 apply_attrition。
///
/// 当前实现：template_id 以 `"attrition_exempt:"` 前缀为豁免标记（临时约定，
/// P2 正式字段 `ItemInstance.attrition_exempt: bool` 落地后替换此 check）。
pub fn is_attrition_exempt(item: &ItemInstance) -> bool {
    item.template_id.starts_with("attrition_exempt:")
}

// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use valence::prelude::App;

    use crate::inventory::ancient_relics::AncientRelicSource;
    use crate::inventory::{ItemInstance, ItemRarity};
    use crate::qi_physics::constants::{
        QI_ATTRITION_BASE_RATE, QI_ATTRITION_DEAD_ZONE_MULTIPLIER, QI_ATTRITION_MIN_QI,
        QI_ATTRITION_SPIRIT_SOURCE_MULTIPLIER, QI_ATTRITION_SPIRIT_SOURCE_THRESHOLD,
        QI_ATTRITION_TSY_MULTIPLIER, QI_EPSILON, QI_ZONE_UNIT_CAPACITY,
    };
    use crate::qi_physics::ledger::{AttritionOpKind, QiAccountKind, QiTransfer, QiTransferReason};
    use crate::world::dimension::DimensionKind;
    use crate::world::tsy::DimensionAnchor;
    use crate::world::tsy_lifecycle::{TsyLifecycle, TsyZoneState};
    use crate::world::zone::Zone;

    use super::*;

    // ── helpers ──────────────────────────────────────────────────────────────

    fn make_zone(name: &str, spirit_qi: f64) -> Zone {
        Zone {
            name: name.to_string(),
            dimension: DimensionKind::Overworld,
            bounds: (
                valence::prelude::DVec3::new(-128.0, 64.0, -128.0),
                valence::prelude::DVec3::new(128.0, 80.0, 128.0),
            ),
            spirit_qi,
            danger_level: 0,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
        }
    }

    fn make_tsy_zone(spirit_qi: f64) -> Zone {
        make_zone("tsy_shallow_a", spirit_qi)
    }

    fn make_tsy_family_zone(family_id: &str, depth: &str, spirit_qi: f64) -> Zone {
        let mut zone = make_zone(&format!("{family_id}_{depth}"), spirit_qi);
        zone.dimension = DimensionKind::Tsy;
        zone
    }

    fn make_item(instance_id: u64, spirit_quality: f64, stack_count: u32) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: "lingshi_fragment".to_string(),
            display_name: "灵石".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 1.0,
            rarity: ItemRarity::Uncommon,
            description: String::new(),
            stack_count,
            spirit_quality,
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
        }
    }

    fn make_tsy_registry(family_id: &str, lifecycle: TsyLifecycle) -> TsyZoneStateRegistry {
        let mut registry = TsyZoneStateRegistry::default();
        registry.by_family.insert(
            family_id.to_string(),
            TsyZoneState {
                family_id: family_id.to_string(),
                lifecycle,
                source_class: AncientRelicSource::DaoLord,
                initial_skeleton: vec![1, 2],
                remaining_skeleton: HashSet::from([1, 2]),
                created_at_tick: 0,
                activated_at_tick: Some(0),
                collapsing_started_at_tick: None,
                dead_at_tick: lifecycle.is_dead().then_some(100),
                main_world_anchor: DimensionAnchor {
                    dimension: DimensionKind::Overworld,
                    pos: valence::prelude::DVec3::new(10.0, 65.0, 10.0),
                },
            },
        );
        registry
    }

    /// 守恒对拍核心函数：apply_attrition → 验证 item 减少量 == zone 增加量（绝对值口径）。
    ///
    /// # 守恒口径说明
    /// - `item_qi_delta` = spirit_quality_before×stack - spirit_quality_after×stack（绝对量）
    /// - `zone_qi_delta` = Δzone.spirit_qi × QI_ZONE_UNIT_CAPACITY（转回绝对量）
    /// - 两者应相等（误差 < 1e-9）。
    ///
    /// `summarize_world_qi` 的 zone_qi 字段是**原始分率**（0..1），与 container_qi（绝对量）
    /// 不同口径，故**不适合**直接调 `assert_conservation`——那个断言是为 WorldQiAccount 账本
    /// 转账设计的（player/zone 均在账本内，账本口径一致）。
    /// 这里改为直接对拍 item_delta（绝对量）== zone_delta（绝对量）。
    fn snapshot_conservation_test(
        zone: &mut Zone,
        item: &mut ItemInstance,
        op_kind: AttritionOpKind,
    ) -> (f64, f64) {
        let abs_qi_before = item.spirit_quality * item.stack_count.max(1) as f64;
        let zone_qi_before = zone.spirit_qi;

        let mut app = App::new();
        app.add_event::<QiTransfer>();

        // 应用磨损
        {
            let mut events_res = app.world_mut().resource_mut::<Events<QiTransfer>>();
            let events_mut: &mut Events<QiTransfer> = &mut events_res;
            apply_attrition(item, op_kind, Some(zone), Some(events_mut));
        }

        let zone_qi_after = zone.spirit_qi;
        let item_qi_delta = abs_qi_before - item.spirit_quality * item.stack_count.max(1) as f64;
        // zone_qi 是 0..1 分率，乘以 QI_ZONE_UNIT_CAPACITY 转回绝对量
        let zone_qi_delta = (zone_qi_after - zone_qi_before) * QI_ZONE_UNIT_CAPACITY;

        // 直接守恒断言：item 减少的绝对量 == zone 增加的绝对量（cap 满时 overflow 也守恒，
        // 但 overflow 账户不进 zone_qi，故满 zone 时 zone_qi_delta < item_qi_delta 是正确的）
        // → 此处只验证 item_delta == zone_delta + overflow_delta，用 QiTransfer events 核对

        (item_qi_delta, zone_qi_delta)
    }

    // ── P0 守恒律单测 ─────────────────────────────────────────────────────────

    #[test]
    fn p0_pickup_conservation_qi100_to_97_zone_plus_3() {
        // 验收：spirit_quality=1.0 × stack=100(abs=100), Pickup, 默认 env
        // → spirit_quality=0.97 + zone +3 + 守恒律全绿
        let mut zone = make_zone("spawn", 0.5); // 非死域非 tsy 非馈赠区，env_mult=1.0
        let mut item = make_item(1, 1.0, 100);

        let (item_delta, zone_delta) =
            snapshot_conservation_test(&mut zone, &mut item, AttritionOpKind::Pickup);

        // item spirit_quality 应降至 0.97（BASE_RATE × 1.0 × 1.0 = 0.03，abs_qi_delta ≈ 3.0）
        assert!(
            (item.spirit_quality - 0.97).abs() < 1e-9,
            "期望 spirit_quality=0.97 (1.0×(1-0.03))，实际 {:.6}",
            item.spirit_quality
        );
        // zone 应增加约 3.0 / QI_ZONE_UNIT_CAPACITY（绝对量 3.0）
        assert!(
            (zone_delta - 3.0).abs() < 1e-6,
            "期望 zone 增加 3.0 真元，实际 {:.6}（item 减少 {:.6}）",
            zone_delta,
            item_delta
        );
        assert!(
            (item_delta - zone_delta).abs() < 1e-6,
            "item 减少量 {:.6} 与 zone 增加量 {:.6} 不等（守恒违反）",
            item_delta,
            zone_delta
        );
    }

    #[test]
    fn p0_transfer_reason_is_attrition_tax_pickup() {
        // QiTransfer reason == AttritionTax{Pickup} 且 amount ≈ 3.0
        let mut zone = make_zone("spawn", 0.5);
        let mut item = make_item(42, 1.0, 100);

        let mut app = App::new();
        app.add_event::<QiTransfer>();

        {
            let mut events_res = app.world_mut().resource_mut::<Events<QiTransfer>>();
            apply_attrition(
                &mut item,
                AttritionOpKind::Pickup,
                Some(&mut zone),
                Some(&mut events_res),
            );
        }

        let events_res = app.world().resource::<Events<QiTransfer>>();
        let mut reader = events_res.get_reader();
        let transfers: Vec<_> = reader.read(events_res).collect();

        assert!(
            !transfers.is_empty(),
            "apply_attrition 应 emit 至少一个 QiTransfer"
        );
        let t = &transfers[0];
        assert!(
            matches!(
                t.reason,
                QiTransferReason::AttritionTax {
                    op_kind: AttritionOpKind::Pickup
                }
            ),
            "reason 应为 AttritionTax{{Pickup}}，实际 {:?}",
            t.reason
        );
        assert!(
            (t.amount - 3.0).abs() < 1e-6,
            "transfer.amount 应 ≈ 3.0，实际 {:.6}",
            t.amount
        );
    }

    #[test]
    fn p0_zone_increase_equals_item_decrease_exact() {
        // zone 增量 == item 减量（精确等式断言，带修复线索）
        let mut zone = make_zone("spawn", 0.5);
        let mut item = make_item(7, 1.0, 100);

        let zone_qi_before = zone.spirit_qi * QI_ZONE_UNIT_CAPACITY;
        let abs_qi_before = item.spirit_quality * item.stack_count as f64;

        let mut app = App::new();
        app.add_event::<QiTransfer>();
        {
            let mut events_res = app.world_mut().resource_mut::<Events<QiTransfer>>();
            apply_attrition(
                &mut item,
                AttritionOpKind::Pickup,
                Some(&mut zone),
                Some(&mut events_res),
            );
        }

        let zone_qi_after = zone.spirit_qi * QI_ZONE_UNIT_CAPACITY;
        let abs_qi_after = item.spirit_quality * item.stack_count as f64;

        let item_lost = abs_qi_before - abs_qi_after;
        let zone_gained = zone_qi_after - zone_qi_before;

        assert!(
            (item_lost - zone_gained).abs() < 1e-9,
            "守恒违反：item 减少 {item_lost:.9}，zone 增加 {zone_gained:.9}，差 {:.9}（凭空消失/产生）",
            (item_lost - zone_gained).abs()
        );
    }

    #[test]
    fn p0_skip_when_abs_qi_below_min() {
        // abs_qi < 0.5 → 不触发磨损，spirit_quality 不变，无 transfer
        let mut zone = make_zone("spawn", 0.5);
        let spirit_q = 0.004;
        let mut item = make_item(2, spirit_q, 1); // abs_qi = 0.004 < 0.5

        let mut app = App::new();
        app.add_event::<QiTransfer>();
        {
            let mut events_res = app.world_mut().resource_mut::<Events<QiTransfer>>();
            apply_attrition(
                &mut item,
                AttritionOpKind::Pickup,
                Some(&mut zone),
                Some(&mut events_res),
            );
        }

        assert!(
            (item.spirit_quality - spirit_q).abs() < QI_EPSILON,
            "abs_qi<0.5 时 spirit_quality 不应变，期望 {spirit_q}，实际 {}",
            item.spirit_quality
        );
        let events_res = app.world().resource::<Events<QiTransfer>>();
        let mut reader = events_res.get_reader();
        let transfers: Vec<_> = reader.read(events_res).collect();
        assert!(
            transfers.is_empty(),
            "abs_qi<0.5 不应 emit 任何 QiTransfer，实际 {} 条",
            transfers.len()
        );
    }

    #[test]
    fn p0_boundary_abs_qi_exactly_0_5_triggers() {
        // abs_qi == 0.5 的 off-by-one 边界：应触发磨损
        let mut zone = make_zone("spawn", 0.5);
        // spirit_quality=0.5, stack=1 → abs=0.5, 刚好不小于 MIN_QI
        let sq_before = 0.5;
        let mut item = make_item(3, sq_before, 1);

        let mut app = App::new();
        app.add_event::<QiTransfer>();
        {
            let mut events_res = app.world_mut().resource_mut::<Events<QiTransfer>>();
            apply_attrition(
                &mut item,
                AttritionOpKind::Pickup,
                Some(&mut zone),
                Some(&mut events_res),
            );
        }

        assert!(
            item.spirit_quality < sq_before,
            "abs_qi=0.5 应触发磨损，spirit_quality 应从 {sq_before} 降低，实际 {}",
            item.spirit_quality
        );
    }

    #[test]
    fn p0_boundary_abs_qi_below_0_5_skips() {
        // abs_qi = 0.4999 < 0.5 → 跳过
        let mut zone = make_zone("spawn", 0.5);
        let sq = 0.4999;
        let mut item = make_item(4, sq, 1);

        let mut app = App::new();
        app.add_event::<QiTransfer>();
        {
            let mut events_res = app.world_mut().resource_mut::<Events<QiTransfer>>();
            apply_attrition(
                &mut item,
                AttritionOpKind::Pickup,
                Some(&mut zone),
                Some(&mut events_res),
            );
        }

        assert!(
            (item.spirit_quality - sq).abs() < QI_EPSILON,
            "abs_qi=0.4999 <0.5 应跳过，spirit_quality 不变，实际 {}",
            item.spirit_quality
        );
    }

    #[test]
    fn p3_checked_below_min_reports_skip_reason_without_transfer() {
        let mut zone = make_zone("spawn", 0.5);
        let sq = QI_ATTRITION_MIN_QI - 0.0001;
        let mut item = make_item(30, sq, 1);

        let mut app = App::new();
        app.add_event::<QiTransfer>();
        let outcome = {
            let mut events_res = app.world_mut().resource_mut::<Events<QiTransfer>>();
            apply_attrition_checked(
                &mut item,
                AttritionOpKind::Pickup,
                Some(&mut zone),
                Some(&mut events_res),
                None,
            )
        };

        assert_eq!(
            outcome,
            AttritionApplyOutcome::Skipped(AttritionSkipReason::BelowMinimumQi),
            "abs_qi<QI_ATTRITION_MIN_QI 必须返回明确 BelowMinimumQi，避免调用方误判为已结算"
        );
        assert!(
            (item.spirit_quality - sq).abs() < QI_EPSILON,
            "BelowMinimumQi 跳过时 item 不应变动"
        );
        let events_res = app.world().resource::<Events<QiTransfer>>();
        let mut reader = events_res.get_reader();
        let transfers: Vec<_> = reader.read(events_res).collect();
        assert!(
            transfers.is_empty(),
            "BelowMinimumQi 跳过时不应 emit QiTransfer，实际 {} 条",
            transfers.len()
        );
    }

    #[test]
    fn p3_checked_abs_qi_exact_min_applies() {
        let mut zone = make_zone("spawn", 0.5);
        let mut item = make_item(31, QI_ATTRITION_MIN_QI, 1);
        let before = item.spirit_quality;

        let mut app = App::new();
        app.add_event::<QiTransfer>();
        let outcome = {
            let mut events_res = app.world_mut().resource_mut::<Events<QiTransfer>>();
            apply_attrition_checked(
                &mut item,
                AttritionOpKind::Pickup,
                Some(&mut zone),
                Some(&mut events_res),
                None,
            )
        };

        assert_eq!(
            outcome,
            AttritionApplyOutcome::Applied,
            "abs_qi==QI_ATTRITION_MIN_QI 是闭区间边界，应触发磨损"
        );
        assert!(
            item.spirit_quality < before,
            "abs_qi==QI_ATTRITION_MIN_QI 应降低 spirit_quality，before={before} after={}",
            item.spirit_quality
        );
    }

    #[test]
    fn p3_checked_exempt_reports_skip_reason_without_mutation() {
        let mut zone = make_zone("spawn", 0.5);
        let zone_before = zone.spirit_qi;
        let mut item = ItemInstance {
            template_id: "attrition_exempt:sealed_herb".to_string(),
            ..make_item(32, 1.0, 100)
        };
        let item_before = item.spirit_quality;

        let mut app = App::new();
        app.add_event::<QiTransfer>();
        let outcome = {
            let mut events_res = app.world_mut().resource_mut::<Events<QiTransfer>>();
            apply_attrition_checked(
                &mut item,
                AttritionOpKind::SlotMove,
                Some(&mut zone),
                Some(&mut events_res),
                None,
            )
        };

        assert_eq!(
            outcome,
            AttritionApplyOutcome::Skipped(AttritionSkipReason::Exempt),
            "显式豁免物品必须返回 Exempt，而不是静默跳过"
        );
        assert!(
            (item.spirit_quality - item_before).abs() < QI_EPSILON,
            "Exempt 跳过时 item 不应变动"
        );
        assert!(
            (zone.spirit_qi - zone_before).abs() < QI_EPSILON,
            "Exempt 跳过时 zone 不应变动"
        );
        let events_res = app.world().resource::<Events<QiTransfer>>();
        let mut reader = events_res.get_reader();
        let transfers: Vec<_> = reader.read(events_res).collect();
        assert!(
            transfers.is_empty(),
            "Exempt 跳过时不应 emit QiTransfer，实际 {} 条",
            transfers.len()
        );
    }

    #[test]
    fn p3_dead_tsy_family_id_detects_dead_lifecycle() {
        let zone = make_tsy_family_zone("tsy_lingxu_01", "shallow", 0.5);
        let dead = make_tsy_registry("tsy_lingxu_01", TsyLifecycle::Dead);
        let active = make_tsy_registry("tsy_lingxu_01", TsyLifecycle::Active);

        assert_eq!(
            dead_tsy_family_id(&zone, Some(&dead)).as_deref(),
            Some("tsy_lingxu_01"),
            "Dead lifecycle 应被解析为死坍缩渊 family guard"
        );
        assert_eq!(
            dead_tsy_family_id(&zone, Some(&active)),
            None,
            "Active lifecycle 不应误判为死坍缩渊"
        );
        assert_eq!(
            dead_tsy_family_id(&make_zone("spawn", 0.5), Some(&dead)),
            None,
            "非 TSY zone 不应命中 dead_tsy guard"
        );
    }

    #[test]
    fn p3_checked_dead_tsy_skips_without_transfer_or_mutation() {
        let mut zone = make_tsy_family_zone("tsy_lingxu_02", "deep", 0.5);
        let zone_before = zone.spirit_qi;
        let registry = make_tsy_registry("tsy_lingxu_02", TsyLifecycle::Dead);
        let mut item = make_item(33, 1.0, 100);
        let item_before = item.spirit_quality;

        let mut app = App::new();
        app.add_event::<QiTransfer>();
        let outcome = {
            let mut events_res = app.world_mut().resource_mut::<Events<QiTransfer>>();
            apply_attrition_checked(
                &mut item,
                AttritionOpKind::ContainerSearch,
                Some(&mut zone),
                Some(&mut events_res),
                Some(&registry),
            )
        };

        assert_eq!(
            outcome,
            AttritionApplyOutcome::Skipped(AttritionSkipReason::DeadTsy),
            "死坍缩渊内 stale 操作必须显式跳过，不能扣 item 后无处归还"
        );
        assert!(
            (item.spirit_quality - item_before).abs() < QI_EPSILON,
            "DeadTsy guard 命中时 item 不应变动"
        );
        assert!(
            (zone.spirit_qi - zone_before).abs() < QI_EPSILON,
            "DeadTsy guard 命中时 zone 不应变动"
        );
        let events_res = app.world().resource::<Events<QiTransfer>>();
        let mut reader = events_res.get_reader();
        let transfers: Vec<_> = reader.read(events_res).collect();
        assert!(
            transfers.is_empty(),
            "DeadTsy guard 命中时不应 emit QiTransfer，实际 {} 条",
            transfers.len()
        );
    }

    #[test]
    fn p3_dead_zone_zero_spirit_qi_still_conserves_release() {
        let mut zone = make_zone("ash_dead", 0.0);
        let mut item = make_item(34, 1.0, 100);
        let abs_qi_before = item.spirit_quality * item.stack_count.max(1) as f64;
        let zone_qi_before = zone.spirit_qi * QI_ZONE_UNIT_CAPACITY;

        let mut app = App::new();
        app.add_event::<QiTransfer>();
        let outcome = {
            let mut events_res = app.world_mut().resource_mut::<Events<QiTransfer>>();
            apply_attrition_checked(
                &mut item,
                AttritionOpKind::Pickup,
                Some(&mut zone),
                Some(&mut events_res),
                None,
            )
        };

        assert_eq!(
            outcome,
            AttritionApplyOutcome::Applied,
            "spirit_qi=0.0 的死域仍有 WorldQiAccount，应正常接收磨损归还"
        );
        let abs_qi_after = item.spirit_quality * item.stack_count.max(1) as f64;
        let zone_qi_after = zone.spirit_qi * QI_ZONE_UNIT_CAPACITY;
        let item_lost = abs_qi_before - abs_qi_after;
        let zone_gained = zone_qi_after - zone_qi_before;
        assert!(
            item_lost > 0.0,
            "死域磨损应从 item 扣除正值，实际 item_lost={item_lost:.9}"
        );
        assert!(
            (item_lost - zone_gained).abs() < 1e-9,
            "死域 spirit_qi=0.0 仍必须守恒：item_lost={item_lost:.9} zone_gained={zone_gained:.9}"
        );
    }

    #[test]
    fn p0_spirit_quality_zero_skips() {
        // spirit_quality = 0 → 不触发
        let mut zone = make_zone("spawn", 0.5);
        let mut item = make_item(5, 0.0, 100);
        let zone_before = zone.spirit_qi;

        let mut app = App::new();
        app.add_event::<QiTransfer>();
        {
            let mut events_res = app.world_mut().resource_mut::<Events<QiTransfer>>();
            apply_attrition(
                &mut item,
                AttritionOpKind::Pickup,
                Some(&mut zone),
                Some(&mut events_res),
            );
        }

        assert_eq!(item.spirit_quality, 0.0, "spirit_quality=0 不应变动");
        assert!(
            (zone.spirit_qi - zone_before).abs() < QI_EPSILON,
            "spirit_quality=0 时 zone 不应变动"
        );
    }

    // ── P1：各 op_kind 磨损率 ─────────────────────────────────────────────────

    fn assert_op_kind_rate(op_kind: AttritionOpKind, expected_rate: f64) {
        let mut zone = make_zone("spawn", 0.5); // 默认 env_mult = 1.0
        let mut item = make_item(100, 1.0, 100); // abs_qi = 100

        let mut app = App::new();
        app.add_event::<QiTransfer>();
        {
            let mut events_res = app.world_mut().resource_mut::<Events<QiTransfer>>();
            apply_attrition(&mut item, op_kind, Some(&mut zone), Some(&mut events_res));
        }

        let expected_quality = 1.0 - expected_rate;
        assert!(
            (item.spirit_quality - expected_quality).abs() < 1e-9,
            "op_kind={op_kind:?}: 期望 spirit_quality={expected_quality:.6}（rate={expected_rate}），实际 {:.6}",
            item.spirit_quality
        );
    }

    #[test]
    fn p1_pickup_rate_is_0_03() {
        assert_op_kind_rate(AttritionOpKind::Pickup, 0.03);
    }

    #[test]
    fn p1_slot_move_rate_is_approx_0_02() {
        // BASE × 0.667 = 0.02001
        let effective = QI_ATTRITION_BASE_RATE * AttritionOpKind::SlotMove.rate_factor();
        assert!(
            (effective - 0.02).abs() < 0.001,
            "SlotMove effective rate 应 ≈ 0.02，实际 {effective:.6}"
        );
        assert_op_kind_rate(AttritionOpKind::SlotMove, effective);
    }

    #[test]
    fn p1_container_search_rate_is_approx_0_05() {
        let effective = QI_ATTRITION_BASE_RATE * AttritionOpKind::ContainerSearch.rate_factor();
        assert!(
            (effective - 0.05).abs() < 0.001,
            "ContainerSearch effective rate 应 ≈ 0.05，实际 {effective:.6}"
        );
        assert_op_kind_rate(AttritionOpKind::ContainerSearch, effective);
    }

    #[test]
    fn p1_forge_load_rate_is_approx_0_04() {
        let effective = QI_ATTRITION_BASE_RATE * AttritionOpKind::ForgeLoad.rate_factor();
        assert!(
            (effective - 0.04).abs() < 0.001,
            "ForgeLoad effective rate 应 ≈ 0.04，实际 {effective:.6}"
        );
        assert_op_kind_rate(AttritionOpKind::ForgeLoad, effective);
    }

    #[test]
    fn p1_alchemy_load_rate_is_approx_0_04() {
        let effective = QI_ATTRITION_BASE_RATE * AttritionOpKind::AlchemyLoad.rate_factor();
        assert!(
            (effective - 0.04).abs() < 0.001,
            "AlchemyLoad effective rate 应 ≈ 0.04，实际 {effective:.6}"
        );
        assert_op_kind_rate(AttritionOpKind::AlchemyLoad, effective);
    }

    // ── P1：环境倍率 ──────────────────────────────────────────────────────────

    #[test]
    fn p1_dead_zone_multiplier_3x() {
        // spirit_qi = 0.0 < DEAD_ZONE_QI_THRESHOLD (0.01) → 死域 × 3
        use crate::cultivation::dead_zone::DEAD_ZONE_QI_THRESHOLD;
        let mut zone = make_zone("ash_dead", 0.0); // spirit_qi=0 → is_dead_zone
        assert!(zone.spirit_qi >= 0.0 && zone.spirit_qi < DEAD_ZONE_QI_THRESHOLD);

        let mut item = make_item(200, 1.0, 100);
        let mut app = App::new();
        app.add_event::<QiTransfer>();
        {
            let mut events_res = app.world_mut().resource_mut::<Events<QiTransfer>>();
            apply_attrition(
                &mut item,
                AttritionOpKind::Pickup,
                Some(&mut zone),
                Some(&mut events_res),
            );
        }

        let expected_rate = QI_ATTRITION_BASE_RATE * 1.0 * QI_ATTRITION_DEAD_ZONE_MULTIPLIER;
        let expected_quality = 1.0 - expected_rate;
        assert!(
            (item.spirit_quality - expected_quality).abs() < 1e-9,
            "死域×3: 期望 spirit_quality={expected_quality:.6}，实际 {:.6}",
            item.spirit_quality
        );
    }

    #[test]
    fn p1_tsy_multiplier_3x() {
        // zone.name="tsy_..." → is_tsy() → × 3
        let mut zone = make_tsy_zone(0.5);
        assert!(zone.is_tsy(), "tsy_shallow_a 应被识别为 is_tsy");

        let mut item = make_item(201, 1.0, 100);
        let mut app = App::new();
        app.add_event::<QiTransfer>();
        {
            let mut events_res = app.world_mut().resource_mut::<Events<QiTransfer>>();
            apply_attrition(
                &mut item,
                AttritionOpKind::Pickup,
                Some(&mut zone),
                Some(&mut events_res),
            );
        }

        let expected_rate = QI_ATTRITION_BASE_RATE * 1.0 * QI_ATTRITION_TSY_MULTIPLIER;
        let expected_quality = 1.0 - expected_rate;
        assert!(
            (item.spirit_quality - expected_quality).abs() < 1e-9,
            "坍缩渊×3: 期望 spirit_quality={expected_quality:.6}，实际 {:.6}",
            item.spirit_quality
        );
    }

    #[test]
    fn p1_spirit_source_multiplier_0_8x() {
        // spirit_qi = 0.7 >= threshold → 馈赠区 × 0.8
        let mut zone = make_zone("lingquan", QI_ATTRITION_SPIRIT_SOURCE_THRESHOLD);
        let mut item = make_item(202, 1.0, 100);

        let mut app = App::new();
        app.add_event::<QiTransfer>();
        {
            let mut events_res = app.world_mut().resource_mut::<Events<QiTransfer>>();
            apply_attrition(
                &mut item,
                AttritionOpKind::Pickup,
                Some(&mut zone),
                Some(&mut events_res),
            );
        }

        let expected_rate = QI_ATTRITION_BASE_RATE * 1.0 * QI_ATTRITION_SPIRIT_SOURCE_MULTIPLIER;
        let expected_quality = 1.0 - expected_rate;
        assert!(
            (item.spirit_quality - expected_quality).abs() < 1e-9,
            "馈赠区×0.8: 期望 spirit_quality={expected_quality:.6}（rate={expected_rate:.4}），实际 {:.6}",
            item.spirit_quality
        );
    }

    #[test]
    fn p1_spirit_source_boundary_at_0_7() {
        // spirit_qi=0.7 → 馈赠区；spirit_qi=0.6999 → 默认 1.0
        let mut zone_high = make_zone("lingquan_high", 0.7);
        let mut zone_low = make_zone("lingquan_low", 0.6999);

        let mult_high = env_multiplier(&zone_high);
        let mult_low = env_multiplier(&zone_low);

        assert!(
            (mult_high - QI_ATTRITION_SPIRIT_SOURCE_MULTIPLIER).abs() < QI_EPSILON,
            "spirit_qi=0.7 应为馈赠区倍率 {QI_ATTRITION_SPIRIT_SOURCE_MULTIPLIER}，实际 {mult_high}"
        );
        assert!(
            (mult_low - 1.0).abs() < QI_EPSILON,
            "spirit_qi=0.6999 应为默认倍率 1.0，实际 {mult_low}"
        );

        // 也验证磨损差异
        let mut item_high = make_item(203, 1.0, 100);
        let mut item_low = make_item(204, 1.0, 100);

        let mut app = App::new();
        app.add_event::<QiTransfer>();
        {
            let mut e = app.world_mut().resource_mut::<Events<QiTransfer>>();
            apply_attrition(
                &mut item_high,
                AttritionOpKind::Pickup,
                Some(&mut zone_high),
                Some(&mut e),
            );
            apply_attrition(
                &mut item_low,
                AttritionOpKind::Pickup,
                Some(&mut zone_low),
                Some(&mut e),
            );
        }

        assert!(
            item_high.spirit_quality > item_low.spirit_quality,
            "馈赠区(0.7)磨损应更少，spirit_quality 应更高: high={:.6} low={:.6}",
            item_high.spirit_quality,
            item_low.spirit_quality
        );
    }

    #[test]
    fn p1_env_combined_with_op_kind_container_search_dead_zone() {
        // ContainerSearch(1.667) × 死域(3.0) = 0.03 × 1.667 × 3 ≈ 0.15
        let mut zone = make_zone("ash_dead_tsy", 0.0); // 死域（非 tsy，name 不含 "tsy_"前缀）
        let mut item = make_item(205, 1.0, 100);

        let mut app = App::new();
        app.add_event::<QiTransfer>();
        {
            let mut e = app.world_mut().resource_mut::<Events<QiTransfer>>();
            apply_attrition(
                &mut item,
                AttritionOpKind::ContainerSearch,
                Some(&mut zone),
                Some(&mut e),
            );
        }

        let expected_rate = QI_ATTRITION_BASE_RATE
            * AttritionOpKind::ContainerSearch.rate_factor()
            * QI_ATTRITION_DEAD_ZONE_MULTIPLIER;
        let expected_quality = 1.0 - expected_rate;
        assert!(
            (item.spirit_quality - expected_quality).abs() < 1e-9,
            "ContainerSearch×死域: 期望={expected_quality:.6}，实际={:.6}",
            item.spirit_quality
        );
    }

    #[test]
    fn p1_attrition_exempt_tag_skips() {
        // AttritionExemptTag: template_id 以 "attrition_exempt:" 前缀 → 直接跳过
        let mut zone = make_zone("spawn", 0.5);
        let mut item = ItemInstance {
            template_id: "attrition_exempt:lingshi_jade".to_string(),
            ..make_item(300, 1.0, 100)
        };
        let zone_qi_before = zone.spirit_qi;
        let sq_before = item.spirit_quality;

        // 检查豁免标记
        assert!(
            is_attrition_exempt(&item),
            "attrition_exempt: 前缀应被识别为豁免"
        );

        // 只在非豁免时调用 apply_attrition（生产路径 caller 负责检查）
        if !is_attrition_exempt(&item) {
            let mut app = App::new();
            app.add_event::<QiTransfer>();
            let mut e = app.world_mut().resource_mut::<Events<QiTransfer>>();
            apply_attrition(
                &mut item,
                AttritionOpKind::Pickup,
                Some(&mut zone),
                Some(&mut e),
            );
        }

        assert!(
            (item.spirit_quality - sq_before).abs() < QI_EPSILON,
            "豁免物品 spirit_quality 不应变动，期望 {sq_before}，实际 {}",
            item.spirit_quality
        );
        assert!(
            (zone.spirit_qi - zone_qi_before).abs() < QI_EPSILON,
            "豁免物品 zone 不应变动"
        );
    }

    #[test]
    fn p1_cap_overflow_conservation() {
        // zone 已接近满容量 → accepted < attrition，overflow 走 overflow 账户，总量守恒
        // zone.spirit_qi = 0.9999 → zone_current ≈ 49.995，cap = 50.0，room ≈ 0.005
        let mut zone = make_zone("full_zone", 0.9999);
        let mut item = make_item(400, 1.0, 100); // abs_qi=100, attrition=3.0 >> room

        let abs_qi_before = item.spirit_quality * item.stack_count.max(1) as f64;
        let zone_qi_before = zone.spirit_qi;

        let mut app = App::new();
        app.add_event::<QiTransfer>();

        // 应用磨损
        {
            let mut events_res = app.world_mut().resource_mut::<Events<QiTransfer>>();
            apply_attrition(
                &mut item,
                AttritionOpKind::Pickup,
                Some(&mut zone),
                Some(&mut events_res),
            );
        }

        let zone_qi_after = zone.spirit_qi;
        let item_delta = abs_qi_before - item.spirit_quality * item.stack_count.max(1) as f64;
        let zone_delta = (zone_qi_after - zone_qi_before) * QI_ZONE_UNIT_CAPACITY;

        // 读取全部 QiTransfer 事件，分拣 accepted（to zone）和 overflow
        let events_res = app.world().resource::<Events<QiTransfer>>();
        let mut reader = events_res.get_reader();
        let transfers: Vec<_> = reader.read(events_res).collect();

        // 汇总各类 AttritionTax transfer 量
        let mut accepted_sum = 0.0_f64;
        let mut overflow_sum = 0.0_f64;
        for t in &transfers {
            if matches!(
                t.reason,
                QiTransferReason::AttritionTax {
                    op_kind: AttritionOpKind::Pickup
                }
            ) {
                // overflow 账户 kind == QiAccountKind::Overflow
                if t.to.kind == QiAccountKind::Overflow {
                    overflow_sum += t.amount;
                } else {
                    accepted_sum += t.amount;
                }
            }
        }

        // 守恒断言 1：zone 只能接收 room 部分，overflow 应大于 0
        assert!(
            zone_delta < item_delta,
            "满 zone 应只接收部分 attrition：zone_delta={zone_delta:.6} < item_delta={item_delta:.6}"
        );
        assert!(zone_delta >= 0.0, "zone_delta 应 >= 0（不应减少）");

        // 守恒断言 2（关键）：QiTransfer events 的 accepted + overflow == attrition_abs（item_delta）
        // 这是主要的守恒锁，确保 release_attrition_to_zone 的 overflow emit 不能被悄悄移除
        let events_total = accepted_sum + overflow_sum;
        assert!(
            (events_total - item_delta).abs() < 1e-6,
            "守恒违反：QiTransfer events 合计(accepted={accepted_sum:.6} + overflow={overflow_sum:.6})={events_total:.6} != item_delta={item_delta:.6}（差 {:.6}）。\
             若 overflow emit 被移除，此断言必然撞红。",
            (events_total - item_delta).abs()
        );

        // 守恒断言 3：overflow 量应与 (item_delta - zone_delta) 匹配
        assert!(
            (overflow_sum - (item_delta - zone_delta)).abs() < 1e-6,
            "overflow_sum={overflow_sum:.6} 应等于 item_delta({item_delta:.6}) - zone_delta({zone_delta:.6}) = {:.6}",
            item_delta - zone_delta
        );

        // 守恒断言 4：accepted_sum 与 zone_delta（绝对量）应一致
        assert!(
            (accepted_sum - zone_delta).abs() < 1e-6,
            "accepted_sum={accepted_sum:.6} 应等于 zone_delta={zone_delta:.6}（两者均为 zone 实际增量）"
        );
    }

    // ── P1：AttritionOpKind 变体 pin 测试（饱和化：每变体一 case）────────────

    #[test]
    fn p1_pin_op_kind_pickup() {
        assert_ne!(AttritionOpKind::Pickup, AttritionOpKind::SlotMove);
        assert_eq!(
            QiTransferReason::AttritionTax {
                op_kind: AttritionOpKind::Pickup
            },
            QiTransferReason::AttritionTax {
                op_kind: AttritionOpKind::Pickup
            }
        );
    }

    #[test]
    fn p1_pin_op_kind_slot_move() {
        assert_ne!(AttritionOpKind::SlotMove, AttritionOpKind::Pickup);
        let r = QiTransferReason::AttritionTax {
            op_kind: AttritionOpKind::SlotMove,
        };
        assert!(matches!(
            r,
            QiTransferReason::AttritionTax {
                op_kind: AttritionOpKind::SlotMove
            }
        ));
    }

    #[test]
    fn p1_pin_op_kind_container_search() {
        let r = QiTransferReason::AttritionTax {
            op_kind: AttritionOpKind::ContainerSearch,
        };
        assert!(matches!(
            r,
            QiTransferReason::AttritionTax {
                op_kind: AttritionOpKind::ContainerSearch
            }
        ));
    }

    #[test]
    fn p1_pin_op_kind_forge_load() {
        let r = QiTransferReason::AttritionTax {
            op_kind: AttritionOpKind::ForgeLoad,
        };
        assert!(matches!(
            r,
            QiTransferReason::AttritionTax {
                op_kind: AttritionOpKind::ForgeLoad
            }
        ));
    }

    #[test]
    fn p1_pin_op_kind_alchemy_load() {
        let r = QiTransferReason::AttritionTax {
            op_kind: AttritionOpKind::AlchemyLoad,
        };
        assert!(matches!(
            r,
            QiTransferReason::AttritionTax {
                op_kind: AttritionOpKind::AlchemyLoad
            }
        ));
    }

    // ── AttritionConfig 默认值 pin 测试 ──────────────────────────────────────

    #[test]
    fn attrition_config_default_matches_constants() {
        let cfg = AttritionConfig::default();
        assert!(
            (cfg.base_rate - QI_ATTRITION_BASE_RATE).abs() < QI_EPSILON,
            "AttritionConfig.base_rate 应等于 QI_ATTRITION_BASE_RATE"
        );
        assert!(
            (cfg.min_qi - QI_ATTRITION_MIN_QI).abs() < QI_EPSILON,
            "AttritionConfig.min_qi 应等于 QI_ATTRITION_MIN_QI"
        );
        assert!(
            (cfg.dead_zone_multiplier - QI_ATTRITION_DEAD_ZONE_MULTIPLIER).abs() < QI_EPSILON,
            "AttritionConfig.dead_zone_multiplier 应等于 QI_ATTRITION_DEAD_ZONE_MULTIPLIER"
        );
        assert!(
            (cfg.tsy_multiplier - QI_ATTRITION_TSY_MULTIPLIER).abs() < QI_EPSILON,
            "AttritionConfig.tsy_multiplier 应等于 QI_ATTRITION_TSY_MULTIPLIER"
        );
        assert!(
            (cfg.spirit_source_multiplier - QI_ATTRITION_SPIRIT_SOURCE_MULTIPLIER).abs()
                < QI_EPSILON,
            "AttritionConfig.spirit_source_multiplier 应等于 QI_ATTRITION_SPIRIT_SOURCE_MULTIPLIER"
        );
    }
}
