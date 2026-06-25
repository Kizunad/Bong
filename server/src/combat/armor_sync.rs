//! plan-armor-v1 §1.3 — 装备护甲 → DerivedAttrs.defense_profile 同步。

use std::collections::HashMap;

use valence::prelude::{Changed, Query, Res};

use crate::combat::armor::{ArmorProfileRegistry, ARMOR_MITIGATION_CAP};
use crate::combat::components::{BodyPart, DerivedAttrs, WoundKind};
use crate::inventory::{
    PlayerInventory, EQUIP_SLOT_CHEST, EQUIP_SLOT_FEET, EQUIP_SLOT_HEAD, EQUIP_SLOT_LEGS,
};

pub(crate) fn build_defense_profile_from_inventory(
    inv: &PlayerInventory,
    armor_profiles: &ArmorProfileRegistry,
) -> HashMap<(BodyPart, WoundKind), f32> {
    let mut profile: HashMap<(BodyPart, WoundKind), f32> = HashMap::new();

    // plan-layered-equip-v1 P3 公式1/2/5（混装各自生效，加性聚合）— 读四个护甲身体槽的
    // worn 全层（栈所有层）。非护甲件（背包 / 伪皮）无 ArmorProfile，被 armor_profiles.get
    // 自然跳过（公式6 由 body_mass 负责自重过滤，此处防御聚合天然只命中有 ArmorProfile 的件）。
    //
    // 聚合规则（公式1/2/5）：
    //   defense[(body,kind)] = Σ_{worn 件 i}( kind_mitigation_i × durability_mul_i )，写入前对累加值 .min(CAP)。
    // - 同一身体部位多件加性（`.max()` → `+=`，不取最高）。
    // - 不同身体部位独立累加（按 (body,kind) entry 天然隔离）。
    // - 逐件 NOT 单独 clamp——破甲 0.3 缩放后裸值进 sum，只在写 entry 时 .min(CAP)（公式2）。
    //   矩阵存已 clamped 值（非裸 sum 1.0）；resolve.rs 的 .clamp 为最终唯一兜底（公式3）。
    for slot in [
        EQUIP_SLOT_HEAD,
        EQUIP_SLOT_CHEST,
        EQUIP_SLOT_LEGS,
        EQUIP_SLOT_FEET,
    ] {
        let Some(contents) = inv.equipped.get(slot) else {
            continue;
        };
        for item in contents.worn.iter() {
            let Some(ap) = armor_profiles.get(item.template_id.as_str()) else {
                continue;
            };

            // 公式5：durability ≤ 0 → broken_multiplier(0.3)，否则 1.0；先各自 × effective_mul。
            let effective_mul = ap.effective_multiplier_for_durability_ratio(item.durability);
            for body in &ap.body_coverage {
                for (kind, mitigation) in &ap.kind_mitigation {
                    // 公式2：各件裸贡献（不单独 clamp）累加进 (body,kind) entry。
                    let contribution = mitigation * effective_mul;
                    if contribution <= 0.0 {
                        continue;
                    }
                    *profile.entry((*body, *kind)).or_insert(0.0) += contribution;
                }
            }
        }
    }

    // 公式2：写入前对每个 (body,kind) 累加值 .min(CAP)，矩阵存已 clamped 值（非裸 sum）。
    for value in profile.values_mut() {
        *value = value.min(ARMOR_MITIGATION_CAP);
    }

    profile
}

#[allow(dead_code)]
pub(crate) fn effective_durability(
    profile: &crate::combat::armor::ArmorProfile,
    item: &crate::inventory::ItemInstance,
) -> f32 {
    let base = profile.durability_max as f32;
    let quality_mult = item
        .forge_quality
        .filter(|q| q.is_finite())
        .unwrap_or(0.5)
        .clamp(0.0, 1.0);
    base * (0.7 + 0.6 * quality_mult)
}

/// plan-armor-v1 §1.3 / plan-layered-equip-v1 P3：每当装备变化，重新聚合护甲二维矩阵。
///
/// 聚合规则（P3 公式1/2/5）：同 `(BodyPart, WoundKind)` 多件**加性累加**（不再取最高），
/// 各件 durability 缩放后裸值进 sum，最终对每 entry `.min(ARMOR_MITIGATION_CAP)`。
pub fn sync_armor_to_derived_attrs(
    mut query: Query<(&PlayerInventory, &mut DerivedAttrs), Changed<PlayerInventory>>,
    armor_profiles: Res<ArmorProfileRegistry>,
) {
    for (inv, mut derived) in &mut query {
        derived.defense_profile =
            build_defense_profile_from_inventory(inv, armor_profiles.as_ref());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::combat::armor::ArmorProfile;
    use crate::inventory::{InventoryRevision, ItemInstance, ItemRarity};
    use crate::schema::inventory::EquipSlotV1;
    use valence::prelude::{App, Update};

    fn make_item(instance_id: u64) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: "fake_spirit_hide".to_string(),
            display_name: "fake_spirit_hide".to_string(),
            grid_w: 2,
            grid_h: 2,
            weight: 1.8,
            rarity: ItemRarity::Rare,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 0.8,
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

    #[test]
    fn sync_sets_defense_profile_for_equipped_armor() {
        let mut app = App::new();
        app.insert_resource(ArmorProfileRegistry::from_map(HashMap::from([(
            "fake_spirit_hide".to_string(),
            ArmorProfile {
                slot: EquipSlotV1::Chest,
                body_coverage: vec![BodyPart::Chest, BodyPart::Abdomen],
                kind_mitigation: HashMap::from([(WoundKind::Cut, 0.25)]),
                durability_max: 10,
                broken_multiplier: 0.3,
            },
        )])));
        app.add_systems(Update, sync_armor_to_derived_attrs);

        let mut equipped = HashMap::new();
        equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            crate::inventory::SlotContents::worn_single(make_item(42)),
        );
        let entity = app
            .world_mut()
            .spawn((
                PlayerInventory {
                    revision: InventoryRevision(0),
                    containers: vec![],
                    equipped,
                    hotbar: Default::default(),
                    bone_coins: 0,
                    max_weight: 45.0,
                },
                DerivedAttrs::default(),
            ))
            .id();

        // Changed<PlayerInventory> 需要一次 mutation 才触发。
        {
            let world = app.world_mut();
            let mut entity_mut = world.entity_mut(entity);
            let mut inv = entity_mut.get_mut::<PlayerInventory>().unwrap();
            inv.revision = InventoryRevision(inv.revision.0.saturating_add(1));
        }
        app.update();

        let attrs = app.world().entity(entity).get::<DerivedAttrs>().unwrap();
        assert_eq!(
            attrs
                .defense_profile
                .get(&(BodyPart::Chest, WoundKind::Cut)),
            Some(&0.25)
        );
        assert_eq!(
            attrs
                .defense_profile
                .get(&(BodyPart::Abdomen, WoundKind::Cut)),
            Some(&0.25)
        );
    }

    #[test]
    fn sync_applies_broken_multiplier_when_item_durability_zero() {
        let mut app = App::new();
        app.insert_resource(ArmorProfileRegistry::from_map(HashMap::from([(
            "fake_spirit_hide".to_string(),
            ArmorProfile {
                slot: EquipSlotV1::Chest,
                body_coverage: vec![BodyPart::Chest],
                kind_mitigation: HashMap::from([(WoundKind::Cut, 0.5)]),
                durability_max: 10,
                broken_multiplier: 0.3,
            },
        )])));
        app.add_systems(Update, sync_armor_to_derived_attrs);

        let mut item = make_item(7);
        item.durability = 0.0;
        let mut equipped = HashMap::new();
        equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            crate::inventory::SlotContents::worn_single(item),
        );
        let entity = app
            .world_mut()
            .spawn((
                PlayerInventory {
                    revision: InventoryRevision(0),
                    containers: vec![],
                    equipped,
                    hotbar: Default::default(),
                    bone_coins: 0,
                    max_weight: 45.0,
                },
                DerivedAttrs::default(),
            ))
            .id();

        {
            let world = app.world_mut();
            let mut entity_mut = world.entity_mut(entity);
            let mut inv = entity_mut.get_mut::<PlayerInventory>().unwrap();
            inv.revision = InventoryRevision(inv.revision.0.saturating_add(1));
        }
        app.update();

        let attrs = app.world().entity(entity).get::<DerivedAttrs>().unwrap();
        // 0.5 mitigation × 0.3 broken_multiplier
        assert_eq!(
            attrs
                .defense_profile
                .get(&(BodyPart::Chest, WoundKind::Cut)),
            Some(&0.15)
        );
    }

    fn make_armor_profile(durability_max: u32) -> ArmorProfile {
        ArmorProfile {
            slot: EquipSlotV1::Head,
            body_coverage: vec![BodyPart::Head],
            kind_mitigation: HashMap::from([(WoundKind::Cut, 0.45)]),
            durability_max,
            broken_multiplier: 0.3,
        }
    }

    #[test]
    fn effective_durability_scales_with_forge_quality() {
        let profile = make_armor_profile(280);
        let mut item = make_item(100);
        item.forge_quality = Some(0.5);
        let eff = super::effective_durability(&profile, &item);
        let expected = 280.0 * (0.7 + 0.6 * 0.5);
        assert!(
            (eff - expected).abs() < 0.01,
            "quality=0.5 → effective_durability should be {expected}, got {eff}"
        );
    }

    #[test]
    fn effective_durability_quality_0_gives_0_7x() {
        let profile = make_armor_profile(280);
        let mut item = make_item(101);
        item.forge_quality = Some(0.0);
        let eff = super::effective_durability(&profile, &item);
        let expected = 280.0 * 0.7;
        assert!(
            (eff - expected).abs() < 0.01,
            "quality=0.0 → 0.7× base → {expected}, got {eff}"
        );
    }

    #[test]
    fn effective_durability_quality_1_gives_1_3x() {
        let profile = make_armor_profile(280);
        let mut item = make_item(102);
        item.forge_quality = Some(1.0);
        let eff = super::effective_durability(&profile, &item);
        let expected = 280.0 * 1.3;
        assert!(
            (eff - expected).abs() < 0.01,
            "quality=1.0 → 1.3× base → {expected}, got {eff}"
        );
    }

    #[test]
    fn effective_durability_none_quality_defaults_to_0_5() {
        let profile = make_armor_profile(280);
        let item = make_item(103);
        assert!(item.forge_quality.is_none());
        let eff = super::effective_durability(&profile, &item);
        let expected = 280.0 * (0.7 + 0.6 * 0.5);
        assert!(
            (eff - expected).abs() < 0.01,
            "None forge_quality defaults to 0.5 → {expected}, got {eff}"
        );
    }

    #[test]
    fn effective_durability_clamps_negative_quality() {
        let profile = make_armor_profile(280);
        let mut item = make_item(104);
        item.forge_quality = Some(-0.1);
        let eff = super::effective_durability(&profile, &item);
        let expected = 280.0 * 0.7; // clamped to 0.0
        assert!(
            (eff - expected).abs() < 0.01,
            "negative quality should clamp to 0.0 → 0.7× → {expected}, got {eff}"
        );
    }

    #[test]
    fn effective_durability_clamps_overflow_quality() {
        let profile = make_armor_profile(280);
        let mut item = make_item(105);
        item.forge_quality = Some(1.2);
        let eff = super::effective_durability(&profile, &item);
        let expected = 280.0 * 1.3; // clamped to 1.0
        assert!(
            (eff - expected).abs() < 0.01,
            "quality >1.0 should clamp to 1.0 → 1.3× → {expected}, got {eff}"
        );
    }

    #[test]
    fn effective_durability_nan_defaults_to_0_5() {
        let profile = make_armor_profile(280);
        let mut item = make_item(106);
        item.forge_quality = Some(f32::NAN);
        let eff = super::effective_durability(&profile, &item);
        let expected = 280.0 * (0.7 + 0.6 * 0.5);
        assert!(
            (eff - expected).abs() < 0.01,
            "NaN forge_quality should fallback to default 0.5 → {expected}, got {eff}"
        );
    }

    // ── plan-layered-equip-v1 P3 公式1/2/5：加性聚合 + clamp + durability 缩放 ──

    /// 构造带指定 template_id / cut mitigation / 覆盖部位的 ArmorProfile。
    fn cut_profile(coverage: Vec<BodyPart>, cut_mitigation: f32) -> ArmorProfile {
        ArmorProfile {
            slot: EquipSlotV1::Chest,
            body_coverage: coverage,
            kind_mitigation: HashMap::from([(WoundKind::Cut, cut_mitigation)]),
            durability_max: 10,
            broken_multiplier: 0.3,
        }
    }

    /// 构造带指定 template_id / durability 的 worn 件。
    fn armor_item(instance_id: u64, template_id: &str, durability: f64) -> ItemInstance {
        let mut it = make_item(instance_id);
        it.template_id = template_id.to_string();
        it.durability = durability;
        it
    }

    #[test]
    fn two_armor_pieces_same_slot_aggregate_additively() {
        // 公式1：同槽 worn 两件 0.3 cut → 0.3+0.3=0.6（加性，非取最高 0.3）。
        let registry = ArmorProfileRegistry::from_map(HashMap::from([(
            "light_hide".to_string(),
            cut_profile(vec![BodyPart::Chest], 0.3),
        )]));
        let mut inv = empty_inv();
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            crate::inventory::SlotContents {
                worn: vec![
                    armor_item(1, "light_hide", 1.0),
                    armor_item(2, "light_hide", 1.0),
                ],
                held: None,
            },
        );

        let profile = build_defense_profile_from_inventory(&inv, &registry);
        let v = profile.get(&(BodyPart::Chest, WoundKind::Cut)).copied();
        assert_eq!(
            v,
            Some(0.6),
            "同槽两件 0.3 cut 应加性累加为 0.6（不取最高 0.3）"
        );
    }

    #[test]
    fn additive_sum_clamps_to_cap_not_per_item() {
        // 公式2 boundary：两件 0.5 cut（裸 sum 1.0）→ 矩阵值 = ARMOR_MITIGATION_CAP(0.85)，非 1.0、非 0.5。
        let registry = ArmorProfileRegistry::from_map(HashMap::from([(
            "heavy_hide".to_string(),
            cut_profile(vec![BodyPart::Chest], 0.5),
        )]));
        let mut inv = empty_inv();
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            crate::inventory::SlotContents {
                worn: vec![
                    armor_item(1, "heavy_hide", 1.0),
                    armor_item(2, "heavy_hide", 1.0),
                ],
                held: None,
            },
        );

        let profile = build_defense_profile_from_inventory(&inv, &registry);
        let v = profile.get(&(BodyPart::Chest, WoundKind::Cut)).copied();
        assert_eq!(
            v,
            Some(ARMOR_MITIGATION_CAP),
            "裸 sum 1.0 应被 .min(CAP=0.85) clamp 为 0.85（矩阵存 clamped 值，非裸 1.0）"
        );
    }

    #[test]
    fn broken_piece_scaled_before_additive_sum() {
        // 公式5：破甲(durability=0)件 0.5 × broken_mul(0.3)=0.15，满耐件 0.5 × 1.0=0.5；
        // 加性 sum = 0.65（< CAP，不被 clamp）。验证各件先缩放再纳入 sum。
        let registry = ArmorProfileRegistry::from_map(HashMap::from([(
            "heavy_hide".to_string(),
            cut_profile(vec![BodyPart::Chest], 0.5),
        )]));
        let mut inv = empty_inv();
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            crate::inventory::SlotContents {
                worn: vec![
                    armor_item(1, "heavy_hide", 0.0), // 破甲 → ×0.3 = 0.15
                    armor_item(2, "heavy_hide", 1.0), // 满耐 → ×1.0 = 0.5
                ],
                held: None,
            },
        );

        let profile = build_defense_profile_from_inventory(&inv, &registry);
        let v = profile.get(&(BodyPart::Chest, WoundKind::Cut)).copied();
        assert_eq!(
            v,
            Some(0.65),
            "破甲件先 ×0.3=0.15，满耐件 ×1.0=0.5，加性 sum=0.65（< CAP 不 clamp）"
        );
    }

    #[test]
    fn cross_body_part_accumulates_independently() {
        // 公式1 跨部位：chest 件覆盖 Chest，legs 件覆盖 Legs → 两 entry 各自独立 0.4，不互相 max/sum。
        let registry = ArmorProfileRegistry::from_map(HashMap::from([
            (
                "chest_hide".to_string(),
                cut_profile(vec![BodyPart::Chest], 0.4),
            ),
            (
                "legs_hide".to_string(),
                cut_profile(vec![BodyPart::LegL], 0.4),
            ),
        ]));
        let mut inv = empty_inv();
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            crate::inventory::SlotContents::worn_single(armor_item(1, "chest_hide", 1.0)),
        );
        inv.equipped.insert(
            EQUIP_SLOT_LEGS.to_string(),
            crate::inventory::SlotContents::worn_single(armor_item(2, "legs_hide", 1.0)),
        );

        let profile = build_defense_profile_from_inventory(&inv, &registry);
        assert_eq!(
            profile.get(&(BodyPart::Chest, WoundKind::Cut)).copied(),
            Some(0.4),
            "Chest entry 独立 = 0.4"
        );
        assert_eq!(
            profile.get(&(BodyPart::LegL, WoundKind::Cut)).copied(),
            Some(0.4),
            "LegL entry 独立 = 0.4（不与 Chest 互相累加/取最高）"
        );
    }

    #[test]
    fn non_armor_worn_items_skip_defense_profile() {
        // 背包 / 伪皮（无 ArmorProfile）同槽 worn 不贡献防御。
        let registry = ArmorProfileRegistry::from_map(HashMap::from([(
            "chest_hide".to_string(),
            cut_profile(vec![BodyPart::Chest], 0.4),
        )]));
        let mut inv = empty_inv();
        inv.equipped.insert(
            EQUIP_SLOT_CHEST.to_string(),
            crate::inventory::SlotContents {
                worn: vec![
                    armor_item(1, "chest_hide", 1.0), // 有 ArmorProfile
                    armor_item(2, "backpack", 1.0),   // 无 ArmorProfile → 跳过
                ],
                held: None,
            },
        );

        let profile = build_defense_profile_from_inventory(&inv, &registry);
        assert_eq!(
            profile.get(&(BodyPart::Chest, WoundKind::Cut)).copied(),
            Some(0.4),
            "只有护甲件贡献防御，背包件被 armor_profiles.get 跳过"
        );
    }

    fn empty_inv() -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(0),
            containers: vec![],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 45.0,
        }
    }
}
