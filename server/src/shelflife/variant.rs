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
use crate::world::season::Season;

use super::types::{DecayTrack, Freshness, TrackState};
use super::{
    compute::{
        compute_current_qi, compute_current_qi_with_season, compute_track_state,
        compute_track_state_with_season,
    },
    registry::DecayProfileRegistry,
};

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
                let current_qi =
                    compute_current_qi(freshness, profile, now_tick, zone_multiplier.max(0.0));
                if migrate_age_to_spoil(
                    item,
                    profile,
                    spoil_id,
                    item_registry,
                    now_tick,
                    current_qi,
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

pub fn apply_variant_switch_with_season(
    item: &mut ItemInstance,
    profile_registry: &DecayProfileRegistry,
    item_registry: &ItemRegistry,
    now_tick: u64,
    zone_multiplier: f32,
    season: Season,
    entropy_seed: u64,
) -> bool {
    let Some(freshness) = &item.freshness else {
        return false;
    };

    let Some(profile) = profile_registry.get(&freshness.profile) else {
        return false;
    };

    let state = compute_track_state_with_season(
        freshness,
        profile,
        now_tick,
        zone_multiplier.max(0.0),
        season,
        entropy_seed,
    );

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
                let current_qi = compute_current_qi_with_season(
                    freshness,
                    profile,
                    now_tick,
                    zone_multiplier.max(0.0),
                    season,
                    entropy_seed,
                );
                if migrate_age_to_spoil(
                    item,
                    profile,
                    spoil_id,
                    item_registry,
                    now_tick,
                    current_qi,
                ) {
                    return true;
                }
            }
        }
        _ => {}
    }

    false
}

/// plan-food-v1 MAJOR1 — 与 `apply_variant_switch_with_season` 相同，
/// 但额外接受 `ContainerFreshnessBehavior`。
///
/// 若当前 item 所在容器包含 ice_cellar（SpoilOnly { rate: 0.3 }），
/// 则在 zone_multiplier 基础上再乘 `container_storage_multiplier`，
/// 使腐败速率差异 ≥70% 的容器效果在 sweep 中真实生效。
#[allow(clippy::too_many_arguments)]
pub fn apply_variant_switch_with_season_and_container(
    item: &mut ItemInstance,
    profile_registry: &DecayProfileRegistry,
    item_registry: &ItemRegistry,
    now_tick: u64,
    zone_multiplier: f32,
    season: Season,
    entropy_seed: u64,
    container_behavior: &super::types::ContainerFreshnessBehavior,
) -> bool {
    let Some(freshness) = &item.freshness else {
        return false;
    };

    let Some(profile) = profile_registry.get(&freshness.profile) else {
        return false;
    };

    // 组合容器行为乘子与 zone_multiplier。
    let container_mul = super::container::container_storage_multiplier(container_behavior, profile);
    let effective_multiplier = (zone_multiplier * container_mul).max(0.0);

    let state = compute_track_state_with_season(
        freshness,
        profile,
        now_tick,
        effective_multiplier,
        season,
        entropy_seed,
    );

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
                let current_qi = compute_current_qi_with_season(
                    freshness,
                    profile,
                    now_tick,
                    effective_multiplier,
                    season,
                    entropy_seed,
                );
                if migrate_age_to_spoil(
                    item,
                    profile,
                    spoil_id,
                    item_registry,
                    now_tick,
                    current_qi,
                ) {
                    return true;
                }
            }
        }
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
///
/// plan-food-v1 P1：chen_jiu_v1 映射到 `food.spirit_wine.chen_cu`（food.toml 注册的完整 template ID）。
fn age_spoil_variant_mapping(profile_id: &str) -> Option<&'static str> {
    match profile_id {
        "chen_jiu_v1" => Some("food.spirit_wine.chen_cu"),
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
    current_qi: f32,
) -> bool {
    if item.freshness.is_none() {
        return false;
    }

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
            technique_scroll_spec: None,
            recipe_fragment_spec: None,
            container_spec: None,
            shelflife_profile: None,
            shelflife_track: None,
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
                technique_scroll_spec: None,
                recipe_fragment_spec: None,
                container_spec: None,
                shelflife_profile: None,
                shelflife_track: None,
            },
        );
        // plan-food-v1 P1：food.spirit_wine.chen_cu 是 age_spoil_variant_mapping 的目标 ID。
        map.insert(
            "food.spirit_wine.chen_cu".to_string(),
            crate::inventory::ItemTemplate {
                id: "food.spirit_wine.chen_cu".to_string(),
                display_name: "陈醋".to_string(),
                category: ItemCategory::Food,
                max_stack_count: 16,
                grid_w: 1,
                grid_h: 1,
                base_weight: 0.5,
                rarity: ItemRarity::Common,
                spirit_quality_initial: 0.65,
                description: String::from("vinegar after aging"),
                effect: None,
                cast_duration_ms: 1500,
                cooldown_ms: 1500,
                weapon_spec: None,
                forge_station_spec: None,
                blueprint_scroll_spec: None,
                inscription_scroll_spec: None,
                technique_scroll_spec: None,
                recipe_fragment_spec: None,
                container_spec: None,
                shelflife_profile: Some("chen_cu_v1".to_string()),
                shelflife_track: Some(crate::shelflife::DecayTrack::Spoil),
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
            alchemy: None,
            lingering_owner_qi: None,
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
            alchemy: None,
            lingering_owner_qi: None,
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
            alchemy: None,
            lingering_owner_qi: None,
        };

        let now = 5000;
        assert!(
            apply_variant_switch(&mut item, &profile_r, &item_r, now, 1.0),
            "chen_jiu past post_peak_spoil_threshold should trigger variant switch to food.spirit_wine.chen_cu"
        );
        // plan-food-v1 P1：item ID 切换到完整 food namespace 的 chen_cu template。
        assert_eq!(
            item.template_id, "food.spirit_wine.chen_cu",
            "template_id should switch to food.spirit_wine.chen_cu (not legacy 'chen_cu') \
             because age_spoil_variant_mapping maps chen_jiu_v1 → food.spirit_wine.chen_cu"
        );
        assert_eq!(
            item.display_name, "陈醋",
            "display_name should reflect the food.spirit_wine.chen_cu template display_name"
        );

        // Freshness should be reset to Spoil track with chen_cu_v1 profile
        let f = item.freshness.as_ref().unwrap();
        assert_eq!(
            f.track,
            DecayTrack::Spoil,
            "after Age→Spoil migration, track should become Spoil"
        );
        assert_eq!(
            f.profile.as_str(),
            "chen_cu_v1",
            "post-migration profile should be chen_cu_v1 (from Age profile's post_peak_spoil_profile)"
        );
        assert_eq!(
            f.created_at_tick, now,
            "created_at_tick reset to migration tick"
        ); // reset
        assert_eq!(
            f.frozen_accumulated, 0,
            "frozen_accumulated reset to 0 after migration"
        );
        assert!(
            f.frozen_since_tick.is_none(),
            "frozen_since_tick cleared after migration"
        );
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
            alchemy: None,
            lingering_owner_qi: None,
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

    // ── plan-food-v1 P1 — 陈酒陈化路径完整链路测试 ──

    fn make_chen_jiu_item(initial_qi: f32, created_at: u64) -> ItemInstance {
        ItemInstance {
            instance_id: 42,
            template_id: "food.spirit_wine.chen_jiu".to_string(),
            display_name: "陈酒".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.5,
            rarity: ItemRarity::Uncommon,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 0.0,
            durability: 1.0,
            freshness: Some(Freshness {
                created_at_tick: created_at,
                initial_qi,
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
            alchemy: None,
            lingering_owner_qi: None,
        }
    }

    /// P1 happy path: 陈酒在峰值窗口内，age_peak_check 返回 Peaking，apply_variant_switch 不切换。
    #[test]
    fn chen_jiu_in_peak_window_no_switch() {
        use super::super::consume::{age_peak_check, AgePeakCheck};
        let profile_r = make_profile_registry();
        let item_r = make_item_registry();

        // chen_jiu_v1: peak_at_ticks=1000, peak_window_ratio=0.1
        // 窗口 = [1000*(1-0.1), 1000*(1+0.1)] = [900, 1100]
        // initial_qi=100, peak_bonus=0.5 → Peaking 时 current_qi ≈ 150
        let mut item = make_chen_jiu_item(100.0, 0);
        let profile = profile_r
            .get(&DecayProfileId::new("chen_jiu_v1"))
            .expect("chen_jiu_v1 must be in test registry");

        // 在峰值中心 tick=1000 处检查 age_peak_check
        let freshness = item.freshness.as_ref().unwrap();
        let peak_result = age_peak_check(freshness, profile, 1000, 1.0);
        assert!(
            matches!(peak_result, AgePeakCheck::Peaking { bonus_strength } if (bonus_strength - 0.5).abs() < 1e-3),
            "chen_jiu at tick=1000 (peak center) should return Peaking with bonus_strength=0.5; \
             got {peak_result:?}"
        );

        // apply_variant_switch 在峰值窗口内不切换 item ID。
        let switched = apply_variant_switch(&mut item, &profile_r, &item_r, 1000, 1.0);
        assert!(
            !switched,
            "chen_jiu in peak window should NOT trigger variant switch; it's still aging well"
        );
        assert_eq!(
            item.template_id, "food.spirit_wine.chen_jiu",
            "template_id must remain food.spirit_wine.chen_jiu during peak window"
        );
    }

    /// P1: 峰值窗口前沿（tick=900），也算 Peaking。
    #[test]
    fn chen_jiu_at_peak_window_start_is_peaking() {
        use super::super::consume::{age_peak_check, AgePeakCheck};
        let profile_r = make_profile_registry();

        let item = make_chen_jiu_item(100.0, 0);
        let profile = profile_r.get(&DecayProfileId::new("chen_jiu_v1")).unwrap();
        let freshness = item.freshness.as_ref().unwrap();

        // 窗口下沿 tick=900（包含端点）
        let result_at_900 = age_peak_check(freshness, profile, 900, 1.0);
        assert!(
            matches!(result_at_900, AgePeakCheck::Peaking { .. }),
            "at peak window lower boundary (tick=900) should be Peaking; got {result_at_900:?}"
        );
    }

    /// P1: 在峰值窗口前（tick=500），age_peak_check 返回 NotPeaking。
    #[test]
    fn chen_jiu_before_peak_window_not_peaking() {
        use super::super::consume::{age_peak_check, AgePeakCheck};
        let profile_r = make_profile_registry();

        let item = make_chen_jiu_item(100.0, 0);
        let profile = profile_r.get(&DecayProfileId::new("chen_jiu_v1")).unwrap();
        let freshness = item.freshness.as_ref().unwrap();

        // 峰值窗口下沿 900 之前
        let result = age_peak_check(freshness, profile, 500, 1.0);
        assert_eq!(
            result,
            AgePeakCheck::NotPeaking,
            "at tick=500 (before peak window [900,1100]) should be NotPeaking; got {result:?}"
        );
    }

    /// P1: 过峰后（tick=5000），apply_variant_switch 触发 item ID 切换为 food.spirit_wine.chen_cu。
    #[test]
    fn chen_jiu_past_post_peak_threshold_switches_to_food_spirit_wine_chen_cu() {
        let profile_r = make_profile_registry();
        let item_r = make_item_registry();

        // chen_jiu_v1: peak=1000, post_peak_half=1000, post_peak_spoil_threshold=30
        // initial_qi=100, peak_bonus=0.5 → peak_value=150
        // post_peak 衰减：at tick 5000 → 150 * 0.5^((5000-1000)/1000) = 150 * 0.0625 = 9.375 < 30
        let mut item = make_chen_jiu_item(100.0, 0);

        let switched = apply_variant_switch(&mut item, &profile_r, &item_r, 5000, 1.0);
        assert!(
            switched,
            "chen_jiu at tick=5000 (current_qi≈9.4 < post_peak_spoil_threshold=30) \
             should trigger Age→Spoil migration; no switch indicates bug in variant mapping"
        );
        assert_eq!(
            item.template_id, "food.spirit_wine.chen_cu",
            "after Age→Spoil migration, template_id MUST be food.spirit_wine.chen_cu \
             (plan-food-v1 P1 fix: age_spoil_variant_mapping maps chen_jiu_v1 → food.spirit_wine.chen_cu)"
        );
        assert_eq!(
            item.display_name, "陈醋",
            "display_name after migration should match food.spirit_wine.chen_cu template"
        );
        let f = item
            .freshness
            .as_ref()
            .expect("freshness must survive migration");
        assert_eq!(
            f.track,
            DecayTrack::Spoil,
            "after Age→Spoil migration, track must be Spoil because chen_jiu passed post_peak_spoil_threshold"
        );
        assert_eq!(
            f.profile.as_str(),
            "chen_cu_v1",
            "freshness profile after migration must be chen_cu_v1 (post_peak_spoil_profile field)"
        );
        assert_eq!(
            f.created_at_tick, 5000,
            "created_at_tick reset to migration tick=5000 for new Spoil countdown"
        );
    }

    /// P1 边界: post_peak_spoil_threshold 恰好满足时（current == threshold）不切换（严格 <）。
    #[test]
    fn chen_jiu_exactly_at_post_peak_threshold_does_not_switch() {
        let profile_r = make_profile_registry();
        let item_r = make_item_registry();

        // 需要找到 current == 30 的时间点。
        // peak_at=1000, post_peak_half=1000, peak_bonus=0.5, initial=100 → peak_value=150
        // current(t) = 150 * 0.5^((t-1000)/1000) = 30 → 0.5^x = 0.2 → x = log2(5) ≈ 2.322
        // t = 1000 + 2322 = 3322
        // 用 t=3320 先确认 current > threshold（不切换），t=3400 确认 < threshold（切换）。
        let mut item_near = make_chen_jiu_item(100.0, 0);

        // 3320 ticks: 0.5^(2320/1000) ≈ 0.5^2.32 ≈ 0.201 → 150 * 0.201 ≈ 30.2 > 30 → 不切换
        let switched_near = apply_variant_switch(&mut item_near, &profile_r, &item_r, 3320, 1.0);
        assert!(
            !switched_near,
            "at tick=3320 current≈30.2 which is > post_peak_spoil_threshold=30 — should NOT switch yet"
        );
    }

    /// P1 边界：apply_variant_switch 对非 Age track item（pure Spoil）不触发陈化切换。
    #[test]
    fn spoil_track_food_does_not_trigger_age_spoil_switch() {
        let profile_r = make_profile_registry();
        let item_r = make_item_registry();

        // 使用 Spoil profile（chen_cu_v1）的物品，不应触发 age_spoil_variant_mapping。
        let mut item = ItemInstance {
            instance_id: 99,
            template_id: "food.spirit_wine.chen_cu".to_string(),
            display_name: "陈醋".to_string(),
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
                initial_qi: 100.0,
                track: DecayTrack::Spoil,
                profile: DecayProfileId::new("chen_cu_v1"),
                frozen_accumulated: 0,
                frozen_since_tick: None,
            }),
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        };

        // 在 Spoil Spoiled 状态（远过 spoil_threshold=10）：不切 item ID（plan §9.4 Spoil Spoiled 不走 ID 变体）。
        let switched = apply_variant_switch(&mut item, &profile_r, &item_r, 10_000_000, 1.0);
        assert!(
            !switched,
            "Spoil-track food item should NOT trigger age_spoil_variant_mapping — \
             only Age→Spoil migration path cuts item ID"
        );
        assert_eq!(
            item.template_id, "food.spirit_wine.chen_cu",
            "template_id must remain food.spirit_wine.chen_cu for Spoil-track items"
        );
    }

    /// P1: apply_variant_switch_with_season 路径也正确切换 chen_jiu → food.spirit_wine.chen_cu。
    #[test]
    fn chen_jiu_with_season_path_also_switches_to_correct_id() {
        use crate::world::season::Season;
        let profile_r = make_profile_registry();
        let item_r = make_item_registry();

        let mut item = make_chen_jiu_item(100.0, 0);
        // tick=5000: summer（不影响 Age track，只影响 Decay/Spoil 的季节系数）
        let switched = apply_variant_switch_with_season(
            &mut item,
            &profile_r,
            &item_r,
            5000,
            1.0,
            Season::Summer,
            42,
        );
        assert!(
            switched,
            "apply_variant_switch_with_season: chen_jiu at tick=5000 should switch"
        );
        assert_eq!(
            item.template_id, "food.spirit_wine.chen_cu",
            "apply_variant_switch_with_season must also use food.spirit_wine.chen_cu not legacy 'chen_cu'"
        );
    }
}
