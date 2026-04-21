//! plan-shelflife-v1 §8 — DecayProfileRegistry。
//!
//! 注册表存所有 DecayProfile，供消费侧（M5 alchemy/forge/修炼吸收）+ snapshot
//! 衍生层（M3a）按 `freshness.profile` ID 查 profile 后调 compute_*。
//!
//! M3a 阶段：建空骨架 + insert/get/len API。M7 时由各 plan（mineral / fauna /
//! botany / alchemy / food / forge）填实际 profile（plan §7 钩子表）。

use std::collections::HashMap;

use valence::prelude::Resource;

use super::types::{DecayProfile, DecayProfileId};

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
}
