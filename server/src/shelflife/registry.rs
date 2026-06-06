//! plan-shelflife-v1 §8 — DecayProfileRegistry。
//!
//! 注册表存所有 DecayProfile，供消费侧（M5 alchemy/forge/修炼吸收）+ snapshot
//! 衍生层（M3a）按 `freshness.profile` ID 查 profile 后调 compute_*。
//!
//! M3a 阶段：建空骨架 + insert/get/len API。M7 时由各 plan（mineral / fauna /
//! botany / alchemy / food / forge）填实际 profile（plan §7 钩子表）。

use std::collections::HashMap;

use valence::prelude::Resource;

use super::types::{DecayFormula, DecayProfile, DecayProfileId};

const TICKS_PER_REAL_DAY: u64 = 20 * 60 * 60 * 24;
const GAME_DAY_TICKS: u64 = 24_000;

/// DecayProfile 全局注册表。**Bevy Resource** — 通过 ECS world 存取。
#[derive(Debug, Default, Clone)]
pub struct DecayProfileRegistry {
    profiles: HashMap<DecayProfileId, DecayProfile>,
}

impl Resource for DecayProfileRegistry {}

impl DecayProfileRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 插入 profile — 先校验 (validate())，失败返 Err 不入表。
    /// 同 ID 重复 insert 覆盖原有（caller 负责 dedupe / load order 决策）。
    pub fn insert(&mut self, profile: DecayProfile) -> Result<(), String> {
        profile.validate()?;
        self.profiles.insert(profile.id().clone(), profile);
        Ok(())
    }

    pub fn get(&self, id: &DecayProfileId) -> Option<&DecayProfile> {
        self.profiles.get(id)
    }

    pub fn contains(&self, id: &DecayProfileId) -> bool {
        self.profiles.contains_key(id)
    }

    pub fn len(&self) -> usize {
        self.profiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }

    /// 列出所有已注册 profile ID — 调试 / 工具用。
    pub fn iter_ids(&self) -> impl Iterator<Item = &DecayProfileId> {
        self.profiles.keys()
    }
}

/// Production defaults owned by active plans.
///
/// plan-mineral-v1 §1.4 / §3: four `ling_shi_*` fuel-layer profiles are
/// Exponential decay resources, distinct from slow/freezable bone-coin profiles.
pub fn build_default_registry() -> DecayProfileRegistry {
    let mut registry = DecayProfileRegistry::new();

    register_production_profiles(&mut registry);

    // plan-shelflife-v1 M6: 骨币 Linear Decay（~1y 完全衰减）
    registry
        .insert(DecayProfile::Decay {
            id: DecayProfileId::new("bone_coin_v1"),
            formula: DecayFormula::Linear {
                decay_per_tick: 100.0 / (TICKS_PER_REAL_DAY as f32 * 365.0),
            },
            floor_qi: 0.0,
        })
        .expect("built-in bone_coin profile should validate");

    // plan-fauna-v1 §4：封灵骨币面值档。面值越高，骨材越好，半衰期越长。
    for profile in [
        fauna_decay_profile("bone_coin_5_v1", 3, 0.0),
        fauna_decay_profile("bone_coin_15_v1", 7, 0.0),
        fauna_decay_profile("bone_coin_40_v1", 14, 0.0),
    ] {
        registry
            .insert(profile)
            .expect("built-in fauna bone coin profile should validate");
    }

    // plan-fauna-v1 §7 P3：骨骼也会失去封真元能力，击杀掉落即挂 freshness。
    for profile in [
        fauna_decay_profile("fauna_bone_shu_gu_v1", 1, 0.0),
        fauna_decay_profile("fauna_bone_zhu_gu_v1", 3, 0.0),
        fauna_decay_profile("fauna_bone_yi_shou_gu_v1", 5, 0.0),
        fauna_decay_profile("fauna_bone_feng_he_gu_v1", 7, 0.0),
    ] {
        registry
            .insert(profile)
            .expect("built-in fauna bone profile should validate");
    }

    // plan-food-v1 P0: 普通熟肉 Spoil（3 游戏日腐败）
    registry
        .insert(DecayProfile::Spoil {
            id: DecayProfileId::new("food_spoil_mundane_meat_v1"),
            formula: DecayFormula::Linear {
                decay_per_tick: 1.0 / (GAME_DAY_TICKS as f32 * 3.0),
            },
            spoil_threshold: 0.01,
        })
        .expect("built-in food_spoil_mundane_meat profile should validate");

    // plan-food-v1 P0: 灵果 Spoil（2 游戏日腐败，食物中腐败最快）
    registry
        .insert(DecayProfile::Spoil {
            id: DecayProfileId::new("food_spoil_ling_guo_v1"),
            formula: DecayFormula::Linear {
                decay_per_tick: 1.0 / (GAME_DAY_TICKS as f32 * 2.0),
            },
            spoil_threshold: 0.01,
        })
        .expect("built-in food_spoil_ling_guo profile should validate");

    // plan-food-v1 MAJOR3: 干饼 Spoil（30 游戏日；陈饼是晒干品，远比生肉耐储）
    registry
        .insert(DecayProfile::Spoil {
            id: DecayProfileId::new("food_spoil_mundane_dry_v1"),
            formula: DecayFormula::Linear {
                decay_per_tick: 1.0 / (GAME_DAY_TICKS as f32 * 30.0),
            },
            spoil_threshold: 0.01,
        })
        .expect("built-in food_spoil_mundane_dry profile should validate");

    // plan-shelflife-v1 M6: 陈酒 Age PeakAndFall → 过峰迁 Spoil（chen_cu_v1）
    // chen_cu_v1 作为 Spoil profile 先注册，chen_jiu_v1 引用它
    registry
        .insert(DecayProfile::Spoil {
            id: DecayProfileId::new("chen_cu_v1"),
            formula: DecayFormula::Exponential {
                half_life_ticks: 365 * TICKS_PER_REAL_DAY,
            },
            spoil_threshold: 10.0,
        })
        .expect("built-in chen_cu profile should validate");

    registry
        .insert(DecayProfile::Age {
            id: DecayProfileId::new("chen_jiu_v1"),
            peak_at_ticks: 365 * TICKS_PER_REAL_DAY, // 1 real-year
            peak_bonus: 0.5,
            peak_window_ratio: 0.1,
            post_peak_half_life_ticks: 365 * TICKS_PER_REAL_DAY,
            post_peak_spoil_threshold: 30.0,
            post_peak_spoil_profile: DecayProfileId::new("chen_cu_v1"),
        })
        .expect("built-in chen_jiu profile should validate");

    registry
}

pub fn register_production_profiles(registry: &mut DecayProfileRegistry) {
    // plan-mineral-v1 §1.4: 四档灵石 Exponential Decay
    for profile in [
        ling_shi_profile("ling_shi_fan_v1", 3),
        ling_shi_profile("ling_shi_zhong_v1", 5),
        ling_shi_profile("ling_shi_shang_v1", 7),
        ling_shi_profile("ling_shi_yi_v1", 14),
        ling_mu_gun_profile(),
        fresh_herb_profile(),
        processing_linear_profile("forging_alchemy_v1", 30),
        processing_exp_profile("drying_v1", 14),
        processing_exp_profile("grinding_v1", 7),
        processing_exp_profile("extraction_v1", 3),
    ] {
        registry
            .insert(profile)
            .expect("built-in production shelflife profile should validate");
    }
}

fn ling_shi_profile(id: &'static str, half_life_days: u64) -> DecayProfile {
    fauna_decay_profile(id, half_life_days, 0.0)
}

fn fauna_decay_profile(id: &'static str, half_life_days: u64, floor_qi: f32) -> DecayProfile {
    DecayProfile::Decay {
        id: DecayProfileId::new(id),
        formula: DecayFormula::Exponential {
            half_life_ticks: half_life_days * TICKS_PER_REAL_DAY,
        },
        floor_qi,
    }
}

fn ling_mu_gun_profile() -> DecayProfile {
    DecayProfile::Decay {
        id: DecayProfileId::new("ling_mu_gun_v1"),
        formula: DecayFormula::Exponential {
            half_life_ticks: TICKS_PER_REAL_DAY,
        },
        floor_qi: 0.0,
    }
}

fn fresh_herb_profile() -> DecayProfile {
    DecayProfile::Spoil {
        id: DecayProfileId::new("fresh_herb_v1"),
        formula: DecayFormula::Linear {
            decay_per_tick: 1.0 / (GAME_DAY_TICKS as f32 * 3.0),
        },
        spoil_threshold: 0.01,
    }
}

fn processing_linear_profile(id: &'static str, total_game_days: u64) -> DecayProfile {
    DecayProfile::Decay {
        id: DecayProfileId::new(id),
        formula: DecayFormula::Linear {
            decay_per_tick: 1.0 / (GAME_DAY_TICKS as f32 * total_game_days as f32),
        },
        floor_qi: 0.0,
    }
}

fn processing_exp_profile(id: &'static str, half_life_game_days: u64) -> DecayProfile {
    DecayProfile::Decay {
        id: DecayProfileId::new(id),
        formula: DecayFormula::Exponential {
            half_life_ticks: GAME_DAY_TICKS * half_life_game_days,
        },
        floor_qi: 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::super::types::{DecayFormula, DecayProfile, DecayProfileId};
    use super::*;

    fn sample_decay() -> DecayProfile {
        DecayProfile::Decay {
            id: DecayProfileId::new("ling_shi_fan_v1"),
            formula: DecayFormula::Exponential {
                half_life_ticks: 5_184_000, // 3 real-days @ 20 TPS
            },
            floor_qi: 1.0,
        }
    }

    fn sample_age() -> DecayProfile {
        DecayProfile::Age {
            id: DecayProfileId::new("chen_jiu_v1"),
            peak_at_ticks: 6_307_200_000, // 1 real-year @ 20 TPS
            peak_bonus: 0.5,
            peak_window_ratio: 0.1,
            post_peak_half_life_ticks: 5_184_000,
            post_peak_spoil_threshold: 30.0,
            post_peak_spoil_profile: DecayProfileId::new("chen_cu_v1"),
        }
    }

    #[test]
    fn empty_registry_is_empty() {
        let r = DecayProfileRegistry::new();
        assert!(r.is_empty());
        assert_eq!(r.len(), 0);
        assert!(r.get(&DecayProfileId::new("anything")).is_none());
    }

    #[test]
    fn insert_valid_profile_and_lookup() {
        let mut r = DecayProfileRegistry::new();
        r.insert(sample_decay()).expect("valid profile insert");
        assert_eq!(r.len(), 1);
        assert!(r.contains(&DecayProfileId::new("ling_shi_fan_v1")));
        let got = r.get(&DecayProfileId::new("ling_shi_fan_v1")).unwrap();
        assert_eq!(got.id().as_str(), "ling_shi_fan_v1");
    }

    #[test]
    fn insert_invalid_profile_rejected_and_not_inserted() {
        let mut r = DecayProfileRegistry::new();
        let bad = DecayProfile::Age {
            id: DecayProfileId::new("bad_age"),
            peak_at_ticks: 0, // validate() rejects zero
            peak_bonus: 0.5,
            peak_window_ratio: 0.1,
            post_peak_half_life_ticks: 100,
            post_peak_spoil_threshold: 1.0,
            post_peak_spoil_profile: DecayProfileId::new("anything"),
        };
        assert!(r.insert(bad).is_err());
        assert!(r.is_empty());
    }

    #[test]
    fn insert_overwrites_same_id() {
        let mut r = DecayProfileRegistry::new();
        r.insert(sample_decay()).unwrap();
        let updated = DecayProfile::Decay {
            id: DecayProfileId::new("ling_shi_fan_v1"),
            formula: DecayFormula::Linear {
                decay_per_tick: 0.001,
            },
            floor_qi: 0.5,
        };
        r.insert(updated).unwrap();
        assert_eq!(r.len(), 1);
        let got = r.get(&DecayProfileId::new("ling_shi_fan_v1")).unwrap();
        assert!(matches!(
            got,
            DecayProfile::Decay {
                formula: DecayFormula::Linear { .. },
                ..
            }
        ));
    }

    #[test]
    fn iter_ids_lists_all_inserted() {
        let mut r = DecayProfileRegistry::new();
        r.insert(sample_decay()).unwrap();
        r.insert(sample_age()).unwrap();
        let ids: Vec<&str> = r.iter_ids().map(|id| id.as_str()).collect();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"ling_shi_fan_v1"));
        assert!(ids.contains(&"chen_jiu_v1"));
    }

    #[test]
    fn default_registry_registers_all_ling_shi_profiles() {
        let r = build_default_registry();
        for id in [
            "ling_shi_fan_v1",
            "ling_shi_zhong_v1",
            "ling_shi_shang_v1",
            "ling_shi_yi_v1",
        ] {
            assert!(r.contains(&DecayProfileId::new(id)), "missing {id}");
        }
        for id in [
            "ling_mu_gun_v1",
            "fresh_herb_v1",
            "drying_v1",
            "grinding_v1",
            "forging_alchemy_v1",
            "extraction_v1",
            "bone_coin_v1",
            "bone_coin_5_v1",
            "bone_coin_15_v1",
            "bone_coin_40_v1",
            "fauna_bone_shu_gu_v1",
            "fauna_bone_zhu_gu_v1",
            "fauna_bone_yi_shou_gu_v1",
            "fauna_bone_feng_he_gu_v1",
            "chen_cu_v1",
            "chen_jiu_v1",
            // plan-food-v1 P0/MAJOR3 — 食物 Spoil profiles
            "food_spoil_mundane_meat_v1",
            "food_spoil_ling_guo_v1",
            "food_spoil_mundane_dry_v1",
        ] {
            assert!(r.contains(&DecayProfileId::new(id)), "missing {id}");
        }
        assert_eq!(r.len(), 23, "expected 23 profiles: 20 existing + 3 food spoil profiles (food_spoil_mundane_meat_v1, food_spoil_ling_guo_v1, food_spoil_mundane_dry_v1)");
    }

    #[test]
    fn register_production_profiles_registers_ling_shi_spiritwood_and_processing_profiles() {
        let mut r = DecayProfileRegistry::new();
        register_production_profiles(&mut r);
        assert_eq!(r.len(), 10);
        assert!(r.contains(&DecayProfileId::new("ling_shi_fan_v1")));
        assert!(r.contains(&DecayProfileId::new("ling_shi_yi_v1")));
        assert!(r.contains(&DecayProfileId::new("ling_mu_gun_v1")));
        assert!(r.contains(&DecayProfileId::new("drying_v1")));
        assert!(r.contains(&DecayProfileId::new("extraction_v1")));
    }

    #[test]
    fn ling_shi_half_lives_match_plan_table() {
        let r = build_default_registry();
        let cases = [
            ("ling_shi_fan_v1", 3),
            ("ling_shi_zhong_v1", 5),
            ("ling_shi_shang_v1", 7),
            ("ling_shi_yi_v1", 14),
        ];
        for (id, days) in cases {
            let profile = r.get(&DecayProfileId::new(id)).expect("profile exists");
            assert!(matches!(
                profile,
                DecayProfile::Decay {
                    formula: DecayFormula::Exponential { half_life_ticks },
                    floor_qi: 0.0,
                    ..
                } if *half_life_ticks == days * TICKS_PER_REAL_DAY
            ));
        }
    }

    #[test]
    fn ling_mu_gun_half_life_is_one_real_day() {
        let r = build_default_registry();
        let profile = r
            .get(&DecayProfileId::new("ling_mu_gun_v1"))
            .expect("ling_mu_gun profile exists");
        assert!(matches!(
            profile,
            DecayProfile::Decay {
                formula: DecayFormula::Exponential { half_life_ticks },
                floor_qi: 0.0,
                ..
            } if *half_life_ticks == TICKS_PER_REAL_DAY
        ));
    }

    // ── plan-food-v1 P0 — 食物 Spoil profile 测试 ──

    #[test]
    fn food_spoil_mundane_meat_profile_is_spoil_track() {
        let r = build_default_registry();
        let profile = r
            .get(&DecayProfileId::new("food_spoil_mundane_meat_v1"))
            .expect("food_spoil_mundane_meat_v1 should be in build_default_registry — check shelflife/registry.rs food-v1 P0 block");
        assert!(
            matches!(profile, DecayProfile::Spoil { .. }),
            "food_spoil_mundane_meat_v1 should be DecayProfile::Spoil track because mundane meat spoils"
        );
        assert_eq!(
            profile.track(),
            super::super::types::DecayTrack::Spoil,
            "track() should return Spoil for food_spoil_mundane_meat_v1"
        );
    }

    #[test]
    fn food_spoil_mundane_meat_profile_spoil_threshold_near_zero() {
        let r = build_default_registry();
        let profile = r
            .get(&DecayProfileId::new("food_spoil_mundane_meat_v1"))
            .expect("food_spoil_mundane_meat_v1 must be registered");
        let DecayProfile::Spoil {
            spoil_threshold,
            formula: DecayFormula::Linear { decay_per_tick },
            ..
        } = profile
        else {
            panic!("expected Spoil+Linear profile for mundane meat");
        };
        assert!(
            (*spoil_threshold - 0.01).abs() < 1e-6,
            "mundane meat spoil_threshold should be 0.01, got {spoil_threshold}"
        );
        // 3 game days to full decay: decay_per_tick = 1.0 / (GAME_DAY_TICKS * 3)
        let expected = 1.0 / (GAME_DAY_TICKS as f32 * 3.0);
        assert!(
            (*decay_per_tick - expected).abs() < 1e-12,
            "mundane meat decay_per_tick should be {expected} (1 / 3 game days), got {decay_per_tick}"
        );
    }

    #[test]
    fn food_spoil_mundane_meat_spoils_after_three_game_days() {
        use super::super::consume::{spoil_check, SpoilCheckOutcome};
        use super::super::types::{DecayFormula, DecayProfile, DecayProfileId, Freshness};
        let profile = DecayProfile::Spoil {
            id: DecayProfileId::new("food_spoil_mundane_meat_v1"),
            formula: DecayFormula::Linear {
                decay_per_tick: 1.0 / (GAME_DAY_TICKS as f32 * 3.0),
            },
            spoil_threshold: 0.01,
        };
        let freshness = Freshness::new(0, 1.0, &profile);

        // At tick 0 (just created): Safe — qi == 1.0 >= 0.01
        assert!(
            matches!(
                spoil_check(&freshness, &profile, 0, 1.0),
                SpoilCheckOutcome::Safe { .. }
            ),
            "at tick 0 mundane meat should be Safe because qi=1.0 >> spoil_threshold=0.01"
        );

        // At exactly 3 game days: qi ≈ 0.0 → should be CriticalBlock (< 0.1 * 0.01)
        let after_three_days = GAME_DAY_TICKS * 3;
        let outcome = spoil_check(&freshness, &profile, after_three_days, 1.0);
        assert!(
            matches!(outcome, SpoilCheckOutcome::CriticalBlock { .. }),
            "at 3 game days mundane meat should be CriticalBlock because qi depleted; got {outcome:?}"
        );
    }

    #[test]
    fn food_spoil_ling_guo_profile_is_spoil_track_with_two_game_day_formula() {
        let r = build_default_registry();
        let profile = r
            .get(&DecayProfileId::new("food_spoil_ling_guo_v1"))
            .expect("food_spoil_ling_guo_v1 should be in build_default_registry — check shelflife/registry.rs food-v1 P0 block");
        let DecayProfile::Spoil {
            spoil_threshold,
            formula: DecayFormula::Linear { decay_per_tick },
            ..
        } = profile
        else {
            panic!("expected Spoil+Linear for ling_guo profile, got {profile:?}");
        };
        assert!(
            (*spoil_threshold - 0.01).abs() < 1e-6,
            "ling_guo spoil_threshold should be 0.01, got {spoil_threshold}"
        );
        // 2 game days to full decay: decay_per_tick = 1.0 / (GAME_DAY_TICKS * 2)
        let expected = 1.0 / (GAME_DAY_TICKS as f32 * 2.0);
        assert!(
            (*decay_per_tick - expected).abs() < 1e-12,
            "ling_guo decay_per_tick should be {expected} (1 / 2 game days), got {decay_per_tick}"
        );
    }

    #[test]
    fn ling_guo_decays_faster_than_mundane_meat() {
        // ling_guo 2 game days < mundane_meat 3 game days
        let r = build_default_registry();
        let ling_guo = r
            .get(&DecayProfileId::new("food_spoil_ling_guo_v1"))
            .unwrap();
        let meat = r
            .get(&DecayProfileId::new("food_spoil_mundane_meat_v1"))
            .unwrap();
        let (
            DecayProfile::Spoil {
                formula:
                    DecayFormula::Linear {
                        decay_per_tick: ling_guo_rate,
                    },
                ..
            },
            DecayProfile::Spoil {
                formula:
                    DecayFormula::Linear {
                        decay_per_tick: meat_rate,
                    },
                ..
            },
        ) = (ling_guo, meat)
        else {
            panic!("both profiles must be Spoil+Linear");
        };
        assert!(
            ling_guo_rate > meat_rate,
            "ling_guo ({ling_guo_rate}) should decay faster than mundane_meat ({meat_rate}) \
             because fruit is more perishable (2 game days vs 3)"
        );
    }

    #[test]
    fn food_spoil_profiles_validate_ok() {
        // All food profiles must pass validate()
        for (id, profile) in [
            (
                "food_spoil_mundane_meat_v1",
                DecayProfile::Spoil {
                    id: DecayProfileId::new("food_spoil_mundane_meat_v1"),
                    formula: DecayFormula::Linear {
                        decay_per_tick: 1.0 / (GAME_DAY_TICKS as f32 * 3.0),
                    },
                    spoil_threshold: 0.01,
                },
            ),
            (
                "food_spoil_ling_guo_v1",
                DecayProfile::Spoil {
                    id: DecayProfileId::new("food_spoil_ling_guo_v1"),
                    formula: DecayFormula::Linear {
                        decay_per_tick: 1.0 / (GAME_DAY_TICKS as f32 * 2.0),
                    },
                    spoil_threshold: 0.01,
                },
            ),
        ] {
            profile.validate().unwrap_or_else(|e| {
                panic!("{id} profile failed validate(): {e}");
            });
        }
    }

    #[test]
    fn food_spoil_check_safe_at_creation() {
        use super::super::consume::{spoil_check, SpoilCheckOutcome};
        use super::super::types::{DecayFormula, DecayProfile, DecayProfileId, Freshness};
        let profile = DecayProfile::Spoil {
            id: DecayProfileId::new("food_spoil_ling_guo_v1"),
            formula: DecayFormula::Linear {
                decay_per_tick: 1.0 / (GAME_DAY_TICKS as f32 * 2.0),
            },
            spoil_threshold: 0.01,
        };
        let freshness = Freshness::new(0, 1.0, &profile);
        match spoil_check(&freshness, &profile, 0, 1.0) {
            SpoilCheckOutcome::Safe { current_qi } => {
                assert!(
                    (current_qi - 1.0).abs() < 1e-4,
                    "ling_guo at tick 0 should have current_qi≈1.0, got {current_qi}"
                );
            }
            other => panic!("expected Safe at tick 0, got {other:?}"),
        }
    }

    #[test]
    fn food_spoil_check_warn_approaching_spoil() {
        use super::super::consume::{spoil_check, SpoilCheckOutcome};
        use super::super::types::{DecayFormula, DecayProfile, DecayProfileId, Freshness};
        // decay_per_tick=1.0: depletes 1 qi per tick from initial=100.
        // spoil_threshold=10.0 → Warn when 1.0 <= current < 10.0.
        // At tick 95: current = 100 - 1*95 = 5.0 → Warn (1.0 <= 5.0 < 10.0).
        let profile = DecayProfile::Spoil {
            id: DecayProfileId::new("food_spoil_ling_guo_v1"),
            formula: DecayFormula::Linear {
                decay_per_tick: 1.0,
            },
            spoil_threshold: 10.0,
        };
        let freshness = Freshness::new(0, 100.0, &profile);

        let outcome = spoil_check(&freshness, &profile, 95, 1.0);
        assert!(
            matches!(outcome, SpoilCheckOutcome::Warn { .. }),
            "at tick 95 current=5.0 should be Warn because 0.1*threshold(=1.0) <= 5.0 < threshold(=10.0); got {outcome:?}"
        );
    }

    #[test]
    fn food_spoil_check_critical_block_fully_spoiled() {
        use super::super::consume::{spoil_check, SpoilCheckOutcome};
        use super::super::types::{DecayFormula, DecayProfile, DecayProfileId, Freshness};
        // decay_per_tick=1.0, initial=100, spoil_threshold=10.0.
        // At tick 100: current = 100 - 1*100 = 0 < 0.1*10.0=1.0 → CriticalBlock.
        let profile = DecayProfile::Spoil {
            id: DecayProfileId::new("food_spoil_mundane_meat_v1"),
            formula: DecayFormula::Linear {
                decay_per_tick: 1.0,
            },
            spoil_threshold: 10.0,
        };
        let freshness = Freshness::new(0, 100.0, &profile);

        let outcome = spoil_check(&freshness, &profile, 100, 1.0);
        assert!(
            matches!(outcome, SpoilCheckOutcome::CriticalBlock { .. }),
            "at tick 100 current=0 should be CriticalBlock because 0 < 0.1*threshold(=1.0); got {outcome:?}"
        );
    }
}
