//! 武器 component 与运行时状态（plan-weapon-v1 §1.3 / §6 / §7）。
//!
//! 数据分层:
//! - inventory 侧 [`ItemInstance`](crate::inventory::ItemInstance) 负责占格、堆叠、持久化
//! - 本模块 [`Weapon`] component 负责战斗 runtime:挂在玩家 Entity 上,装备时插入、卸下时移除
//! - 两侧通过 `instance_id` 关联
//!
//! 装备/卸下流程留 W2 与 inventory-v1 `InventoryMoveRequest` 一同接入。本 phase 只完成
//! 数据结构 + 纯函数(伤害乘数 / 耐久扣减 / soul_bond 累加),为后续业务层搭桥。

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component, Entity, Event};

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

/// plan-weapon-v1 §7: 角色-武器磨合关系。同一 character 累计使用该武器升 bond_level。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoulBond {
    /// 绑定的 character_id(= Bong 玩家角色身份,非 Minecraft 账号 UUID)。
    pub character_id: String,
    /// 0=生疏 · 1=磨合 · 2=契合 · 3=神融。超出 [`MAX_LEVEL`](Self::MAX_LEVEL) 无效。
    pub bond_level: u8,
    /// 朝下一级的进度 `[0, 1]`; 达到 1.0 升级并归零; 已达 MAX_LEVEL 则常为 1.0。
    pub bond_progress: f32,
}

impl SoulBond {
    pub const MAX_LEVEL: u8 = 3;

    /// plan §7.2 非绑定者使用惩罚乘数(×0.8)。
    pub const NON_BONDED_MULTIPLIER: f32 = 0.8;

    pub fn new(character_id: String) -> Self {
        Self {
            character_id,
            bond_level: 0,
            bond_progress: 0.0,
        }
    }

    /// 累加进度,溢出时升级并重置。返回本次调用是否触发了**升级**。
    ///
    /// 已达 [`MAX_LEVEL`](Self::MAX_LEVEL) 后 progress 锁在 1.0,返回 `false`。
    pub fn advance(&mut self, delta: f32) -> bool {
        if self.bond_level >= Self::MAX_LEVEL {
            self.bond_progress = 1.0;
            return false;
        }
        self.bond_progress += delta;
        if self.bond_progress < 1.0 {
            return false;
        }
        // 溢出:升级。剩余 progress 丢弃(不累积到下一级),v1 简单规则。
        self.bond_level = (self.bond_level + 1).min(Self::MAX_LEVEL);
        self.bond_progress = if self.bond_level >= Self::MAX_LEVEL {
            1.0
        } else {
            0.0
        };
        true
    }

    /// plan §6.1 soul_bond_multiplier 表: 0→1.00 · 1→1.05 · 2→1.12 · 3→1.25。
    pub fn damage_multiplier(&self) -> f32 {
        match self.bond_level {
            0 => 1.00,
            1 => 1.05,
            2 => 1.12,
            _ => 1.25,
        }
    }
}

/// plan-weapon-v1 §1.3: 玩家 Entity 上的武器 runtime 组件。
///
/// 生命周期(§2.1):装备时插入、卸下 / 死亡 drop / 耐久归零 时移除。
/// 装备/卸下业务在 W2(inventory-v1 `InventoryMoveRequest`)里接入,本 phase 仅数据。
#[derive(Debug, Clone, Component)]
pub struct Weapon {
    /// 对应 [`ItemInstance`](crate::inventory::ItemInstance) 的 instance_id。
    pub instance_id: u64,
    /// 缓存 template_id,避免每次攻击都查 inventory。
    pub template_id: String,
    pub weapon_kind: WeaponKind,
    pub base_attack: f32,
    /// 0=凡铁 · 1=灵器 · 2=法宝 · 3=仙器(plan §0 / §10)。
    pub quality_tier: u8,
    pub durability: f32,
    pub durability_max: f32,
    pub soul_bond: Option<SoulBond>,
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

    /// plan §6.1 完整伤害乘数。攻击方 character_id 用于 soul_bond 匹配。
    pub fn damage_multiplier_for(&self, attacker_character_id: &str) -> f32 {
        let bond_mul = match &self.soul_bond {
            Some(bond) if bond.character_id == attacker_character_id => bond.damage_multiplier(),
            Some(_) => SoulBond::NON_BONDED_MULTIPLIER,
            None => 1.0,
        };
        self.attack_multiplier() * self.quality_multiplier() * self.durability_factor() * bond_mul
    }

    /// 扣减一次耐久; 返回是否归零(调用方据此判定 WeaponBroken)。
    pub fn tick_durability(&mut self) -> bool {
        self.durability = (self.durability - Self::HIT_DURABILITY_COST).max(0.0);
        self.durability <= 0.0
    }

    /// plan §7: 首次使用未绑定武器 → 绑到 character。
    /// 返回是否触发了**首次绑定**(已绑定则不变)。
    pub fn ensure_bond(&mut self, character_id: &str) -> bool {
        if self.soul_bond.is_some() {
            return false;
        }
        self.soul_bond = Some(SoulBond::new(character_id.to_string()));
        true
    }
}

/// plan-weapon-v1 §6.3: 武器损坏事件。`Weapon` component 已被调用方移除,ItemInstance 仍在。
#[derive(Debug, Clone, Event)]
pub struct WeaponBroken {
    pub entity: Entity,
    pub instance_id: u64,
    pub template_id: String,
}

/// plan-weapon-v1 §2.3: 装备武器意图。W2 会消费并插入 Weapon component。
#[derive(Debug, Clone, Event)]
pub struct EquipWeaponIntent {
    pub entity: Entity,
    pub instance_id: u64,
    pub slot: EquipSlot,
}

/// plan-weapon-v1 §2.4: 卸下武器意图。
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

    fn sample_weapon(tier: u8, durability: f32, bond: Option<SoulBond>) -> Weapon {
        Weapon {
            instance_id: 1,
            template_id: "iron_sword".into(),
            weapon_kind: WeaponKind::Sword,
            base_attack: 8.0,
            quality_tier: tier,
            durability,
            durability_max: 200.0,
            soul_bond: bond,
        }
    }

    #[test]
    fn soul_bond_new_starts_at_zero() {
        let b = SoulBond::new("char_a".into());
        assert_eq!(b.bond_level, 0);
        assert_eq!(b.bond_progress, 0.0);
        assert_eq!(b.damage_multiplier(), 1.0);
    }

    #[test]
    fn soul_bond_advance_progresses() {
        let mut b = SoulBond::new("char_a".into());
        assert!(!b.advance(0.3));
        assert_eq!(b.bond_level, 0);
        assert!((b.bond_progress - 0.3).abs() < 1e-6);
    }

    #[test]
    fn soul_bond_advance_levels_up_on_overflow() {
        let mut b = SoulBond::new("char_a".into());
        assert!(b.advance(1.5)); // 溢出触发升级
        assert_eq!(b.bond_level, 1);
        assert_eq!(b.bond_progress, 0.0);
        assert_eq!(b.damage_multiplier(), 1.05);
    }

    #[test]
    fn soul_bond_saturates_at_max_level() {
        let mut b = SoulBond {
            character_id: "char_a".into(),
            bond_level: SoulBond::MAX_LEVEL,
            bond_progress: 1.0,
        };
        assert!(!b.advance(5.0));
        assert_eq!(b.bond_level, SoulBond::MAX_LEVEL);
        assert_eq!(b.bond_progress, 1.0);
        assert_eq!(b.damage_multiplier(), 1.25);
    }

    #[test]
    fn soul_bond_three_consecutive_levels() {
        let mut b = SoulBond::new("char_a".into());
        assert!(b.advance(1.0)); // -> lvl 1
        assert!(b.advance(1.0)); // -> lvl 2
        assert!(b.advance(1.0)); // -> lvl 3
        assert_eq!(b.bond_level, SoulBond::MAX_LEVEL);
        assert_eq!(b.bond_progress, 1.0);
    }

    #[test]
    fn quality_multiplier_covers_four_tiers() {
        assert_eq!(sample_weapon(0, 200.0, None).quality_multiplier(), 1.00);
        assert_eq!(sample_weapon(1, 200.0, None).quality_multiplier(), 1.15);
        assert_eq!(sample_weapon(2, 200.0, None).quality_multiplier(), 1.35);
        assert_eq!(sample_weapon(3, 200.0, None).quality_multiplier(), 1.60);
        // 溢出 tier 走 3+ 档
        assert_eq!(sample_weapon(9, 200.0, None).quality_multiplier(), 1.60);
    }

    #[test]
    fn durability_factor_scales_with_wear() {
        let full = sample_weapon(0, 200.0, None);
        assert!((full.durability_factor() - 1.0).abs() < 1e-6);
        let half = sample_weapon(0, 100.0, None);
        assert!((half.durability_factor() - 0.75).abs() < 1e-6);
        let zero = sample_weapon(0, 0.0, None);
        assert!((zero.durability_factor() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn attack_multiplier_floors_at_one() {
        let mut low = sample_weapon(0, 200.0, None);
        low.base_attack = 3.0; // 3/10=0.3 → floor 1.0
        assert_eq!(low.attack_multiplier(), 1.0);
        let mid = sample_weapon(0, 200.0, None); // 8/10=0.8 → 1.0
        assert_eq!(mid.attack_multiplier(), 1.0);
        let mut hi = sample_weapon(0, 200.0, None);
        hi.base_attack = 22.0; // 2.2
        assert!((hi.attack_multiplier() - 2.2).abs() < 1e-6);
    }

    #[test]
    fn damage_multiplier_uses_all_factors() {
        let bond = SoulBond {
            character_id: "char_a".into(),
            bond_level: 2,
            bond_progress: 0.5,
        };
        let w = Weapon {
            instance_id: 1,
            template_id: "spirit_sword".into(),
            weapon_kind: WeaponKind::Sword,
            base_attack: 14.0, // attack_mul 1.4
            quality_tier: 1,   // qual 1.15
            durability: 200.0,
            durability_max: 400.0, // factor 0.75
            soul_bond: Some(bond),
        };
        let mul = w.damage_multiplier_for("char_a"); // bond 1.12
        let expected = 1.4 * 1.15 * 0.75 * 1.12;
        assert!(
            (mul - expected).abs() < 1e-5,
            "got {mul} expected {expected}"
        );
    }

    #[test]
    fn non_bonded_attacker_uses_penalty() {
        let bond = SoulBond {
            character_id: "char_owner".into(),
            bond_level: 3,
            bond_progress: 1.0,
        };
        let w = sample_weapon(0, 200.0, Some(bond));
        let mine = w.damage_multiplier_for("char_owner");
        let other = w.damage_multiplier_for("char_stranger");
        assert!(mine > other, "owner {mine} should exceed stranger {other}");
        // stranger 走 NON_BONDED_MULTIPLIER(0.8)
        let expected_other = 1.0 * 1.0 * 1.0 * SoulBond::NON_BONDED_MULTIPLIER;
        assert!((other - expected_other).abs() < 1e-5);
    }

    #[test]
    fn unbonded_weapon_has_no_bond_multiplier() {
        let w = sample_weapon(0, 200.0, None);
        assert_eq!(w.damage_multiplier_for("anyone"), 1.0);
    }

    #[test]
    fn tick_durability_broken_after_enough_hits() {
        let mut w = sample_weapon(0, 1.0, None);
        assert!(!w.tick_durability()); // 1.0 → 0.5,未坏
        assert_eq!(w.durability, 0.5);
        assert!(w.tick_durability()); // 0.5 → 0.0,坏了
        assert_eq!(w.durability, 0.0);
    }

    #[test]
    fn tick_durability_does_not_go_negative() {
        let mut w = sample_weapon(0, 0.3, None);
        assert!(w.tick_durability()); // 0.3 - 0.5 → max(0, -0.2) = 0
        assert_eq!(w.durability, 0.0);
    }

    #[test]
    fn ensure_bond_attaches_on_first_use() {
        let mut w = sample_weapon(0, 200.0, None);
        assert!(w.ensure_bond("char_a"));
        assert!(w.soul_bond.is_some());
        let bond = w.soul_bond.as_ref().unwrap();
        assert_eq!(bond.character_id, "char_a");
        assert_eq!(bond.bond_level, 0);
        // 二次调用不覆盖
        assert!(!w.ensure_bond("char_b"));
        assert_eq!(w.soul_bond.as_ref().unwrap().character_id, "char_a");
    }
}
