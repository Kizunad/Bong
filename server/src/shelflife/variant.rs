//! plan-shelflife-v1 M6 — item ID 变体切换。
//!
//! 当物品的 `TrackState` 到达 `Dead`（Decay 路径）、`Spoiled`（Spoil 路径 NBT-only）、
//! `AgePostPeakSpoiled`（Age → Spoil 迁移）时，触发 item ID / NBT 切换。
//!
//! 切换决策依据 plan §6.3 表：
//! - Decay `Dead`：ling_shi → dead_mineral_ling_shi_* / fengling_bone_coin → rotten_bone_coin
//! - Spoil `Spoiled`：不切 item ID，走 NBT（本 v1 暂通过 TrackState 隐含标识）
//! - Age → Spoil 迁移：chen_jiu → chen_cu（有文化语义的 ID 切换）
//!
//! 触发点（plan §6.1 第 5/6/7 条）：
//! - item transfer / pickup / discard
//! - server tick boundary 200 sweep

use crate::inventory::{ItemInstance, ItemRegistry};

use super::types::{DecayTrack, Freshness, TrackState};
use super::{compute::compute_track_state, registry::DecayProfileRegistry};

/// 对单个 ItemInstance 做 shelflife 变体切换（如有必要）。
///
/// 返回 `true` 若发生了任何字段变更（调用方据此 bump revision）。
pub fn apply_variant_switch(
    item: &mut ItemInstance,
    profile_registry: &DecayProfileRegistry,
    item_registry: &ItemRegistry,
    now_tick: u64,
    zone_multiplier: f32,
) -> bool {
    let Some(freshness) = &item.freshness else {
        return false;
    };

    let Some(profile) = profile_registry.get(&freshness.profile) else {
        return false;
    };

    // 在 inventory 上下文由 sweep 传入 zone multiplier；容器路径会单独传参。
    let state = compute_track_state(freshness, profile, now_tick, zone_multiplier.max(0.0));

    match state {
        TrackState::Dead => {
            if let Some(dead_id) = dead_variant_mapping(freshness.profile.as_str()) {
                if switch_template(item, dead_id, item_registry) {
                    return true;
                }
            }
        }
        TrackState::AgePostPeakSpoiled => {
            if let Some(spoil_id) = age_spoil_variant_mapping(freshness.profile.as_str()) {
                if migrate_age_to_spoil(
                    item,
                    profile,
                    spoil_id,
                    item_registry,
                    now_tick,
                    zone_multiplier,
                ) {
                    return true;
                }
            }
        }
        // Spoil Spoiled：不切 item ID，由消费侧按 TrackState::Spoiled 做 contam 警告。
        _ => {}
    }

    false
}

/// Decay track `Dead` → dead 变体 template_id 映射。
fn dead_variant_mapping(profile_id: &str) -> Option<&'static str> {
    match profile_id {
        "ling_shi_fan_v1" => Some("dead_mineral_ling_shi_fan"),
        "ling_shi_zhong_v1" => Some("dead_mineral_ling_shi_zhong"),
        "ling_shi_shang_v1" => Some("dead_mineral_ling_shi_shang"),
        "ling_shi_yi_v1" => Some("dead_mineral_ling_shi_yi"),
        "bone_coin_v1" | "bone_coin_5_v1" | "bone_coin_15_v1" | "bone_coin_40_v1" => {
            Some("rotten_bone_coin")
        }
        _ => None,
    }
}

/// Age → Spoil 迁移 → item ID 映射（仅对有文化语义的物品）。
fn age_spoil_variant_mapping(profile_id: &str) -> Option<&'static str> {
    match profile_id {
        "chen_jiu_v1" => Some("chen_cu"),
        _ => None,
    }
}

/// 用目标 template_id 替换 item 的外观字段。
///
/// 从 `ItemRegistry` 查 template，若找不到 → no-op（不静默改 id 但不更新 display）。
fn switch_template(item: &mut ItemInstance, template_id: &str, registry: &ItemRegistry) -> bool {
    let Some(template) = registry.get(template_id) else {
        tracing::warn!(
            target: "bong::shelflife",
            "dead variant template `{template_id}` not found in ItemRegistry — skipping switch for instance {}",
            item.instance_id
        );
        return false;
    };

    item.template_id = template_id.to_string();
    item.display_name = template.display_name.clone();
    item.description = template.description.clone();
    item.rarity = template.rarity;
    // 保留原有 grid_w/h / weight / spirit_quality / durability 不变
    true
}

/// Age → Spoil 路径迁移：更新 freshness + 切换外观模板。
///
/// plan §1.4 规则：
/// - `track` 由 Age 改为 Spoil
/// - `profile` 改为 post_peak_spoil_profile
/// - `created_at_tick` 重置为迁移当下 tick（重新开始 Spoil 衰减计时）
/// - `initial_qi` 重置为当前 current_qi（Spoil 衰减的起点）
fn migrate_age_to_spoil(
    item: &mut ItemInstance,
    age_profile: &super::types::DecayProfile,
    spoil_template_id: &str,
    item_registry: &ItemRegistry,
    now_tick: u64,
    zone_multiplier: f32,
) -> bool {
    let Some(freshness) = &item.freshness else {
        return false;
    };

    let current_qi =
        compute_track_state_current_qi(freshness, age_profile, now_tick, zone_multiplier);

    let spoil_profile_id = match age_profile {
        super::types::DecayProfile::Age {
            post_peak_spoil_profile,
            ..
        } => post_peak_spoil_profile.clone(),
        _ => return false,
    };

    // 更新外观
    if let Some(template) = item_registry.get(spoil_template_id) {
        item.template_id = spoil_template_id.to_string();
        item.display_name = template.display_name.clone();
        item.description = template.description.clone();
        item.rarity = template.rarity;
    }

    // 更新 freshness —— 路径迁移
    item.freshness = Some(Freshness {
        created_at_tick: now_tick,
        initial_qi: current_qi,
        track: DecayTrack::Spoil,
        profile: spoil_profile_id,
        frozen_accumulated: 0,
        frozen_since_tick: None,
    });

    true
}

/// 算 current_qi（仅用于 Age→Spoil 迁移时的 initial_qi 重置）。
/// 不引入完整 compute_current_qi 以保持 variant 模块不依赖 container multiplier。
fn compute_track_state_current_qi(
    freshness: &Freshness,
    profile: &super::types::DecayProfile,
    now_tick: u64,
    zone_multiplier: f32,
) -> f32 {
    super::compute::compute_current_qi(freshness, profile, now_tick, zone_multiplier.max(0.0))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::super::types::{DecayFormula, DecayProfile, DecayProfileId};
    use super::*;
    use crate::inventory::{ItemCategory, ItemRarity};

    fn dead_template(id: &str) -> crate::inventory::ItemTemplate {
        crate::inventory::ItemTemplate {
            id: id.to_string(),
            display_name: format!("死·{}", id),
            category: ItemCategory::Misc,
            max_stack_count: 1,
            grid_w: 1,
            grid_h: 1,
            base_weight: 0.5,
            rarity: ItemRarity::Common,
            spirit_quality_initial: 0.0,
            description: String::from("dead"),
            effect: None,
            cast_duration_ms: 1500,
            cooldown_ms: 1500,
            weapon_spec: None,
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
        }
    }

    fn make_item_registry() -> ItemRegistry {
        let mut map = HashMap::new();
        for id in [
            "dead_mineral_ling_shi_fan",
            "dead_mineral_ling_shi_zhong",
            "dead_mineral_ling_shi_shang",
            "dead_mineral_ling_shi_yi",
        ] {
            map.insert(id.to_string(), dead_template(id));
        }
        map.insert(
            "rotten_bone_coin".to_string(),
            crate::inventory::ItemTemplate {
                id: "rotten_bone_coin".to_string(),
                display_name: "腐骨币".to_string(),
                category: ItemCategory::BoneCoin,
                max_stack_count: 1,
                grid_w: 1,
                grid_h: 1,
                base_weight: 0.05,
                rarity: ItemRarity::Common,
                spirit_quality_initial: 0.0,
                description: String::from("rotten"),
                effect: None,
                cast_duration_ms: 1500,
                cooldown_ms: 1500,
                weapon_spec: None,
                forge_station_spec: None,
                blueprint_scroll_spec: None,
                inscription_scroll_spec: None,
            },
        );
        map.insert(
            "chen_cu".to_string(),
            crate::inventory::ItemTemplate {
                id: "chen_cu".to_string(),
                display_name: "陈醋".to_string(),
                category: ItemCategory::Misc,
                max_stack_count: 1,
                grid_w: 1,
                grid_h: 1,
                base_weight: 0.5,
                rarity: ItemRarity::Common,
                spirit_quality_initial: 0.0,
                description: String::from("vinegar"),
                effect: None,
                cast_duration_ms: 1500,
                cooldown_ms: 1500,
                weapon_spec: None,
                forge_station_spec: None,
                blueprint_scroll_spec: None,
                inscription_scroll_spec: None,
            },
        );
        ItemRegistry::from_map(map)
    }

    fn dec_profile(id: &str, half_life_days: u64, floor: f32) -> DecayProfile {
        DecayProfile::Decay {
            id: DecayProfileId::new(id),
            formula: DecayFormula::Exponential {
                half_life_ticks: half_life_days * 20 * 60 * 60 * 24,
            },
            floor_qi: floor,
        }
    }

    fn fresh_instance(template_id: &str, profile_id: &str, initial_qi: f32) -> ItemInstance {
        ItemInstance {
            instance_id: 1,
            template_id: template_id.to_string(),
            display_name: "test".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.5,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 0.0,
            durability: 1.0,
            freshness: Some(Freshness {
                created_at_tick: 0,
                initial_qi,
                track: DecayTrack::Decay,
                profile: DecayProfileId::new(profile_id),
                frozen_accumulated: 0,
                frozen_since_tick: None,
            }),
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
        }
    }

    fn make_profile_registry() -> DecayProfileRegistry {
        let mut r = DecayProfileRegistry::new();
        r.insert(dec_profile("ling_shi_fan_v1", 3, 0.0)).unwrap();
        r.insert(dec_profile("ling_shi_zhong_v1", 5, 0.0)).unwrap();
        r.insert(dec_profile("ling_shi_shang_v1", 7, 0.0)).unwrap();
        r.insert(dec_profile("ling_shi_yi_v1", 14, 0.0)).unwrap();
        r.insert(DecayProfile::Decay {
            id: DecayProfileId::new("bone_coin_v1"),
            formula: DecayFormula::Linear {
                decay_per_tick: 100.0 / (20.0 * 60.0 * 60.0 * 24.0 * 365.0),
            },
            floor_qi: 0.0,
        })
        .unwrap();
        // chen_cu_v1 as Spoil profile
        r.insert(DecayProfile::Spoil {
            id: DecayProfileId::new("chen_cu_v1"),
            formula: DecayFormula::Exponential {
                half_life_ticks: 365 * 20 * 60 * 60 * 24,
            },
            spoil_threshold: 10.0,
        })
        .unwrap();
        // chen_jiu_v1 as Age profile
        r.insert(DecayProfile::Age {
            id: DecayProfileId::new("chen_jiu_v1"),
            peak_at_ticks: 1000,
            peak_bonus: 0.5,
            peak_window_ratio: 0.1,
            post_peak_half_life_ticks: 1000,
            post_peak_spoil_threshold: 30.0,
            post_peak_spoil_profile: DecayProfileId::new("chen_cu_v1"),
        })
        .unwrap();
        r
    }

    #[test]
    fn ling_shi_dead_switches_template() {
        let profile_r = make_profile_registry();
        let item_r = make_item_registry();

        // ling_shi_fan with half_life=3 days, floor=0.0, initial=100.0, created at tick 0.
        // After 40 half-lives (120 days), current_qi ≈ 100 * 0.5^40 ≈ 9.1e-11 → ≤ EPSILON → Dead.
        let mut item = fresh_instance("mineral_ling_shi_fan", "ling_shi_fan_v1", 100.0);
        let ticks_per_day: u64 = 20 * 60 * 60 * 24;
        let now = 3 * ticks_per_day * 40; // 120 days

        assert!(apply_variant_switch(
            &mut item, &profile_r, &item_r, now, 1.0
        ));
        assert_eq!(item.template_id, "dead_mineral_ling_shi_fan");
        assert_eq!(item.display_name, "死·dead_mineral_ling_shi_fan");
    }

    #[test]
    fn ling_shi_not_dead_yet_no_switch() {
        let profile_r = make_profile_registry();
        let item_r = make_item_registry();

        let mut item = fresh_instance("mineral_ling_shi_fan", "ling_shi_fan_v1", 100.0);
        let now = 0; // just created — current_qi = 100

        assert!(!apply_variant_switch(
            &mut item, &profile_r, &item_r, now, 1.0
        ));
        assert_eq!(item.template_id, "mineral_ling_shi_fan");
    }

    #[test]
    fn item_without_freshness_no_switch() {
        let profile_r = make_profile_registry();
        let item_r = make_item_registry();

        let mut item = ItemInstance {
            instance_id: 1,
            template_id: "any".to_string(),
            display_name: "any".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.5,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 0.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
        };

        assert!(!apply_variant_switch(
            &mut item,
            &profile_r,
            &item_r,
            999_999_999,
            1.0,
        ));
        assert_eq!(item.template_id, "any");
    }

    #[test]
    fn age_to_spoil_migration_switches_to_chen_cu() {
        let profile_r = make_profile_registry();
        let item_r = make_item_registry();

        // chen_jiu with peak=1000, post_peak_half=1000, post_peak_spoil_threshold=30
        // initial_qi = 100, peak_bonus=0.5 → peak_value = 150
        // After peak (tick 2000), post_peak_half=1000 → at tick 3000: 150 * 0.5^(1000/1000) = 75
        // At tick 4000: 150 * 0.5^(2000/1000) = 37.5
        // At tick 5000: 150 * 0.5^(3000/1000) = 18.75 → below threshold 30 → AgePostPeakSpoiled
        let mut item = ItemInstance {
            instance_id: 1,
            template_id: "chen_jiu".to_string(),
            display_name: "陈酒".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.5,
            rarity: ItemRarity::Rare,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 0.0,
            durability: 1.0,
            freshness: Some(Freshness {
                created_at_tick: 0,
                initial_qi: 100.0,
                track: DecayTrack::Age,
                profile: DecayProfileId::new("chen_jiu_v1"),
                frozen_accumulated: 0,
                frozen_since_tick: None,
            }),
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
        };

        let now = 5000;
        assert!(apply_variant_switch(
            &mut item, &profile_r, &item_r, now, 1.0
        ));
        assert_eq!(item.template_id, "chen_cu");
        assert_eq!(item.display_name, "陈醋");

        // Freshness should be reset to Spoil track with chen_cu_v1 profile
        let f = item.freshness.as_ref().unwrap();
        assert_eq!(f.track, DecayTrack::Spoil);
        assert_eq!(f.profile.as_str(), "chen_cu_v1");
        assert_eq!(f.created_at_tick, now); // reset
        assert_eq!(f.frozen_accumulated, 0);
        assert!(f.frozen_since_tick.is_none());
    }

    #[test]
    fn bone_coin_dead_switches_to_rotten() {
        let profile_r = make_profile_registry();
        let item_r = make_item_registry();

        let mut item = ItemInstance {
            instance_id: 1,
            template_id: "fengling_bone_coin".to_string(),
            display_name: "封灵骨币".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.05,
            rarity: ItemRarity::Uncommon,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 0.0,
            durability: 1.0,
            freshness: Some(Freshness {
                created_at_tick: 0,
                initial_qi: 100.0,
                track: DecayTrack::Decay,
                profile: DecayProfileId::new("bone_coin_v1"),
                frozen_accumulated: 0,
                frozen_since_tick: None,
            }),
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
        };

        // bone_coin Linear decay over ~1y; use 2 years for safety margin to ensure ≤ EPSILON.
        let ticks_per_year: u64 = 365 * 20 * 60 * 60 * 24;
        let now = ticks_per_year * 2;
        assert!(apply_variant_switch(
            &mut item, &profile_r, &item_r, now, 1.0
        ));
        assert_eq!(item.template_id, "rotten_bone_coin");
        assert_eq!(item.display_name, "腐骨币");
    }
}
