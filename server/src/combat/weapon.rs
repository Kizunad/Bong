//! 武器 component 与运行时状态（plan-weapon-v1 §1.3 / §6）。
//!
//! 数据分层:
//! - inventory 侧 [`ItemInstance`](crate::inventory::ItemInstance) 负责占格、堆叠、持久化
//! - 本模块 [`Weapon`] component 负责战斗 runtime:挂在玩家 Entity 上,装备时插入、卸下时移除
//! - 两侧通过 `instance_id` 关联
//!
//! 装备/卸下流程留 W2 与 inventory-v1 `InventoryMoveRequest` 一同接入。本 phase 只完成
//! 数据结构 + 纯函数(伤害乘数 / 耐久扣减),为后续业务层搭桥。

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Changed, Commands, Component, Entity, Event, Query, Res};

use crate::inventory::{ItemInstance, ItemRegistry, PlayerInventory, WeaponSpec};

/// plan-weapon-v1 §1.3: 武器大类。影响渲染 transform(§5.4)、攻击动画、技能兼容性。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeaponKind {
    Sword,
    Saber,
    Staff,
    Fist,
    Spear,
    Dagger,
    Bow,
}

/// plan-weapon-v1 §1.3: 玩家 Entity 上的武器 runtime 组件。
///
/// 生命周期(§2.1):装备时插入、卸下 / 死亡 drop / 耐久归零 时移除。
/// 装备/卸下业务在 W2(inventory-v1 `InventoryMoveRequest`)里接入,本 phase 仅数据。
#[derive(Debug, Clone, Component)]
pub struct Weapon {
    pub slot: EquipSlot,
    /// 对应 [`ItemInstance`](crate::inventory::ItemInstance) 的 instance_id。
    pub instance_id: u64,
    /// 缓存 template_id,避免每次攻击都查 inventory。
    pub template_id: String,
    #[allow(dead_code)] // v1 先保留到 runtime component，后续渲染/技能钩子会消费。
    pub weapon_kind: WeaponKind,
    pub base_attack: f32,
    /// 0=凡铁 · 1=灵器 · 2=法宝 · 3=仙器(plan §0 / §10)。
    pub quality_tier: u8,
    pub durability: f32,
    pub durability_max: f32,
}

impl Weapon {
    /// plan §6.3: 每次命中默认扣的耐久。W6 战斗插桩时可按 wound_kind 差异化。
    pub const HIT_DURABILITY_COST: f32 = 0.5;

    /// plan §6.1 quality_multiplier 表: 0→1.00 · 1→1.15 · 2→1.35 · 3+→1.60。
    pub fn quality_multiplier(&self) -> f32 {
        match self.quality_tier {
            0 => 1.00,
            1 => 1.15,
            2 => 1.35,
            _ => 1.60,
        }
    }

    /// plan §6.1 durability_factor: `0.5 + 0.5 × (cur/max)`; 残破武器保底 50% 威力。
    pub fn durability_factor(&self) -> f32 {
        let ratio = if self.durability_max > 0.0 {
            (self.durability / self.durability_max).clamp(0.0, 1.0)
        } else {
            0.0
        };
        0.5 + 0.5 * ratio
    }

    /// plan §6.1 weapon_attack_multiplier 基础档: `max(1.0, base_attack / 10.0)`。
    ///
    /// 赤手空拳(无 [`Weapon`] component)走 combat/resolve 侧的 1.0 缺省路径,不进本函数。
    pub fn attack_multiplier(&self) -> f32 {
        (self.base_attack / 10.0).max(1.0)
    }

    /// plan §6.1 完整伤害乘数: attack × quality × durability。
    pub fn damage_multiplier(&self) -> f32 {
        self.attack_multiplier() * self.quality_multiplier() * self.durability_factor()
    }

    /// 扣减一次耐久; 返回是否归零(调用方据此判定 WeaponBroken)。
    pub fn tick_durability(&mut self) -> bool {
        self.durability = (self.durability - Self::HIT_DURABILITY_COST).max(0.0);
        self.durability <= 0.0
    }

    /// plan §2.3: 从 [`ItemInstance`] 和 [`WeaponSpec`] 派生一个运行时 `Weapon`。
    ///
    /// `ItemInstance.durability` 是 `[0, 1]` 比例；此处乘 `spec.durability_max`
    /// 转为绝对值挂到 component(tick_durability 扣减用绝对值)。
    pub fn from_item_and_spec(item: &ItemInstance, spec: &WeaponSpec, slot: EquipSlot) -> Self {
        let durability = (item.durability as f32) * spec.durability_max;
        Self {
            slot,
            instance_id: item.instance_id,
            template_id: item.template_id.clone(),
            weapon_kind: spec.weapon_kind,
            base_attack: spec.base_attack,
            quality_tier: spec.quality_tier,
            durability,
            durability_max: spec.durability_max,
        }
    }
}

/// plan-weapon-v1 §2.3: 每 tick 同步 `PlayerInventory.equipped.{main/off/two}` ↔ `Weapon` component。
///
/// 使用 `Changed<PlayerInventory>` 过滤：只处理本 tick 有变动的玩家 Entity（含 revision 变化）。
/// 选择顺序：main_hand > two_hand > off_hand。v1 的实际战斗结算只吃当前一个 `Weapon`
/// component，但网络与 HUD 仍可单独收到各槽位 snapshot。
pub fn sync_weapon_component_from_equipped(
    mut commands: Commands,
    registry: Res<ItemRegistry>,
    inventories: Query<(Entity, &PlayerInventory), Changed<PlayerInventory>>,
    existing_weapons: Query<&Weapon>,
) {
    for (entity, inv) in &inventories {
        let current_item = current_weapon_from_inventory(inv);
        let existing = existing_weapons.get(entity).ok();

        match (current_item, existing) {
            (Some((item, slot)), None) => {
                // 新装备
                if let Some(tpl) = registry.get(&item.template_id) {
                    if let Some(spec) = &tpl.weapon_spec {
                        commands
                            .entity(entity)
                            .insert(Weapon::from_item_and_spec(item, spec, slot));
                    }
                }
            }
            (None, Some(_)) => {
                // 卸下
                commands.entity(entity).remove::<Weapon>();
            }
            (Some((item, slot)), Some(w))
                if item.instance_id != w.instance_id || slot != w.slot =>
            {
                // 切换不同武器
                if let Some(tpl) = registry.get(&item.template_id) {
                    match &tpl.weapon_spec {
                        Some(spec) => {
                            commands
                                .entity(entity)
                                .insert(Weapon::from_item_and_spec(item, spec, slot));
                        }
                        None => {
                            // equipped slot 放入了非武器(理论上 inventory 会拒但兜底)
                            commands.entity(entity).remove::<Weapon>();
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn current_weapon_from_inventory(inv: &PlayerInventory) -> Option<(&ItemInstance, EquipSlot)> {
    inv.equipped
        .get("main_hand")
        .map(|item| (item, EquipSlot::MainHand))
        .or_else(|| {
            inv.equipped
                .get("two_hand")
                .map(|item| (item, EquipSlot::TwoHand))
        })
        .or_else(|| {
            inv.equipped
                .get("off_hand")
                .map(|item| (item, EquipSlot::OffHand))
        })
}

/// plan-weapon-v1 §6.3: 武器损坏事件。`Weapon` component 已被调用方移除,ItemInstance 仍在。
#[derive(Debug, Clone, Event)]
pub struct WeaponBroken {
    pub entity: Entity,
    pub instance_id: u64,
    pub template_id: String,
}

/// plan-weapon-v1 §2.3: 装备武器意图。
///
/// 当前架构走 `sync_weapon_component_from_equipped`:直接从 `PlayerInventory.equipped`
/// 派生 Weapon component,不走 Intent。本结构体保留给未来"不经过 inventory move 的
/// 快捷装备键"(例如双持快速切换)使用。
#[allow(dead_code)]
#[derive(Debug, Clone, Event)]
pub struct EquipWeaponIntent {
    pub entity: Entity,
    pub instance_id: u64,
    pub slot: EquipSlot,
}

/// plan-weapon-v1 §2.4: 卸下武器意图。同 [`EquipWeaponIntent`] 暂未接入。
#[allow(dead_code)]
#[derive(Debug, Clone, Event)]
pub struct UnequipWeaponIntent {
    pub entity: Entity,
    pub slot: EquipSlot,
}

/// 装备槽枚举(plan §2.2)。跟 inventory-v1 `EquipSlotV1` 对齐,仅限武器相关。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(clippy::enum_variant_names)] // 三个都带 Hand 后缀属于语义(plan §2.2 槽位命名)
pub enum EquipSlot {
    MainHand,
    OffHand,
    TwoHand,
}

// ============================================================================
// tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{
        ContainerState, InventoryRevision, ItemCategory, ItemRarity, ItemRegistry, ItemTemplate,
    };
    use std::collections::HashMap;
    use valence::prelude::{App, Update};

    fn sample_weapon(tier: u8, durability: f32) -> Weapon {
        Weapon {
            slot: EquipSlot::MainHand,
            instance_id: 1,
            template_id: "iron_sword".into(),
            weapon_kind: WeaponKind::Sword,
            base_attack: 8.0,
            quality_tier: tier,
            durability,
            durability_max: 200.0,
        }
    }

    #[test]
    fn quality_multiplier_covers_four_tiers() {
        assert_eq!(sample_weapon(0, 200.0).quality_multiplier(), 1.00);
        assert_eq!(sample_weapon(1, 200.0).quality_multiplier(), 1.15);
        assert_eq!(sample_weapon(2, 200.0).quality_multiplier(), 1.35);
        assert_eq!(sample_weapon(3, 200.0).quality_multiplier(), 1.60);
        // 溢出 tier 走 3+ 档
        assert_eq!(sample_weapon(9, 200.0).quality_multiplier(), 1.60);
    }

    #[test]
    fn durability_factor_scales_with_wear() {
        let full = sample_weapon(0, 200.0);
        assert!((full.durability_factor() - 1.0).abs() < 1e-6);
        let half = sample_weapon(0, 100.0);
        assert!((half.durability_factor() - 0.75).abs() < 1e-6);
        let zero = sample_weapon(0, 0.0);
        assert!((zero.durability_factor() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn attack_multiplier_floors_at_one() {
        let mut low = sample_weapon(0, 200.0);
        low.base_attack = 3.0; // 3/10=0.3 → floor 1.0
        assert_eq!(low.attack_multiplier(), 1.0);
        let mid = sample_weapon(0, 200.0); // 8/10=0.8 → 1.0
        assert_eq!(mid.attack_multiplier(), 1.0);
        let mut hi = sample_weapon(0, 200.0);
        hi.base_attack = 22.0; // 2.2
        assert!((hi.attack_multiplier() - 2.2).abs() < 1e-6);
    }

    #[test]
    fn damage_multiplier_uses_all_factors() {
        let w = Weapon {
            slot: EquipSlot::MainHand,
            instance_id: 1,
            template_id: "spirit_sword".into(),
            weapon_kind: WeaponKind::Sword,
            base_attack: 14.0, // attack_mul 1.4
            quality_tier: 1,   // qual 1.15
            durability: 200.0,
            durability_max: 400.0, // factor 0.75
        };
        let mul = w.damage_multiplier();
        let expected = 1.4 * 1.15 * 0.75;
        assert!(
            (mul - expected).abs() < 1e-5,
            "got {mul} expected {expected}"
        );
    }

    #[test]
    fn tick_durability_broken_after_enough_hits() {
        let mut w = sample_weapon(0, 1.0);
        assert!(!w.tick_durability()); // 1.0 → 0.5,未坏
        assert_eq!(w.durability, 0.5);
        assert!(w.tick_durability()); // 0.5 → 0.0,坏了
        assert_eq!(w.durability, 0.0);
    }

    #[test]
    fn tick_durability_does_not_go_negative() {
        let mut w = sample_weapon(0, 0.3);
        assert!(w.tick_durability()); // 0.3 - 0.5 → max(0, -0.2) = 0
        assert_eq!(w.durability, 0.0);
    }

    // ──────────────────────────────────────────────────────────────────────
    // 集成测试:sync_weapon_component_from_equipped
    // ──────────────────────────────────────────────────────────────────────

    fn test_registry_with_iron_sword() -> ItemRegistry {
        let mut templates = HashMap::new();
        templates.insert(
            "iron_sword".to_string(),
            ItemTemplate {
                id: "iron_sword".to_string(),
                display_name: "铁剑".to_string(),
                category: ItemCategory::Weapon,
                grid_w: 1,
                grid_h: 2,
                base_weight: 1.2,
                rarity: ItemRarity::Common,
                spirit_quality_initial: 1.0,
                description: "test".to_string(),
                effect: None,
                cast_duration_ms: 0,
                cooldown_ms: 0,
                weapon_spec: Some(WeaponSpec {
                    weapon_kind: WeaponKind::Sword,
                    base_attack: 8.0,
                    quality_tier: 0,
                    durability_max: 200.0,
                    qi_cost_mul: 1.0,
                }),
            },
        );
        templates.insert(
            "spirit_saber".to_string(),
            ItemTemplate {
                id: "spirit_saber".to_string(),
                display_name: "灵刀".to_string(),
                category: ItemCategory::Weapon,
                grid_w: 1,
                grid_h: 2,
                base_weight: 1.0,
                rarity: ItemRarity::Uncommon,
                spirit_quality_initial: 1.0,
                description: "test".to_string(),
                effect: None,
                cast_duration_ms: 0,
                cooldown_ms: 0,
                weapon_spec: Some(WeaponSpec {
                    weapon_kind: WeaponKind::Saber,
                    base_attack: 14.0,
                    quality_tier: 1,
                    durability_max: 400.0,
                    qi_cost_mul: 1.0,
                }),
            },
        );
        ItemRegistry::from_map(templates)
    }

    fn empty_inventory() -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(1),
            containers: vec![ContainerState {
                id: "main_pack".to_string(),
                name: "主背包".to_string(),
                rows: 5,
                cols: 7,
                items: vec![],
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 100.0,
        }
    }

    fn make_item(instance_id: u64, template_id: &str) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: template_id.to_string(),
            display_name: "t".to_string(),
            grid_w: 1,
            grid_h: 2,
            weight: 1.0,
            rarity: ItemRarity::Common,
            description: "".to_string(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability: 1.0,
            freshness: None,
        }
    }

    fn bump(inv: &mut PlayerInventory) {
        inv.revision = InventoryRevision(inv.revision.0 + 1);
    }

    #[test]
    fn sync_inserts_weapon_component_on_equip() {
        let mut app = App::new();
        app.insert_resource(test_registry_with_iron_sword());
        app.add_systems(Update, sync_weapon_component_from_equipped);
        let entity = app.world_mut().spawn(empty_inventory()).id();
        app.update();
        assert!(app.world().entity(entity).get::<Weapon>().is_none());

        // 装备 iron_sword
        {
            let mut inv = app.world_mut().get_mut::<PlayerInventory>(entity).unwrap();
            inv.equipped
                .insert("main_hand".to_string(), make_item(42, "iron_sword"));
            bump(&mut inv);
        }
        app.update();

        let w = app
            .world()
            .entity(entity)
            .get::<Weapon>()
            .expect("Weapon inserted");
        assert_eq!(w.instance_id, 42);
        assert_eq!(w.weapon_kind, WeaponKind::Sword);
        assert!((w.base_attack - 8.0).abs() < 1e-6);
        assert_eq!(w.quality_tier, 0);
        assert!((w.durability_max - 200.0).abs() < 1e-6);
        assert!((w.durability - 200.0).abs() < 1e-6); // ratio 1.0 × max
    }

    #[test]
    fn sync_removes_weapon_component_on_unequip() {
        let mut app = App::new();
        app.insert_resource(test_registry_with_iron_sword());
        app.add_systems(Update, sync_weapon_component_from_equipped);
        let mut inv = empty_inventory();
        inv.equipped
            .insert("main_hand".to_string(), make_item(42, "iron_sword"));
        let entity = app.world_mut().spawn(inv).id();
        app.update();
        assert!(app.world().entity(entity).get::<Weapon>().is_some());

        // 卸下
        {
            let mut inv = app.world_mut().get_mut::<PlayerInventory>(entity).unwrap();
            inv.equipped.remove("main_hand");
            bump(&mut inv);
        }
        app.update();
        assert!(app.world().entity(entity).get::<Weapon>().is_none());
    }

    #[test]
    fn sync_switches_weapon_component_on_swap() {
        let mut app = App::new();
        app.insert_resource(test_registry_with_iron_sword());
        app.add_systems(Update, sync_weapon_component_from_equipped);
        let mut inv = empty_inventory();
        inv.equipped
            .insert("main_hand".to_string(), make_item(42, "iron_sword"));
        let entity = app.world_mut().spawn(inv).id();
        app.update();
        assert_eq!(
            app.world()
                .entity(entity)
                .get::<Weapon>()
                .unwrap()
                .instance_id,
            42
        );

        // 换装 spirit_saber
        {
            let mut inv = app.world_mut().get_mut::<PlayerInventory>(entity).unwrap();
            inv.equipped.clear();
            inv.equipped
                .insert("main_hand".to_string(), make_item(77, "spirit_saber"));
            bump(&mut inv);
        }
        app.update();

        let w = app.world().entity(entity).get::<Weapon>().unwrap();
        assert_eq!(w.instance_id, 77);
        assert_eq!(w.weapon_kind, WeaponKind::Saber);
        assert_eq!(w.quality_tier, 1);
    }
}
