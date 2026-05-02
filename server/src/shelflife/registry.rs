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
        ] {
            assert!(r.contains(&DecayProfileId::new(id)), "missing {id}");
        }
        assert_eq!(r.len(), 15);
    }

    #[test]
    fn register_production_profiles_registers_ling_shi_ladder_and_spiritwood() {
        let mut r = DecayProfileRegistry::new();
        register_production_profiles(&mut r);
        assert_eq!(r.len(), 5);
        assert!(r.contains(&DecayProfileId::new("ling_shi_fan_v1")));
        assert!(r.contains(&DecayProfileId::new("ling_shi_yi_v1")));
        assert!(r.contains(&DecayProfileId::new("ling_mu_gun_v1")));
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
}
