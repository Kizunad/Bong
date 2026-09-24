//! plan-forge-v1 §4 `LifeRecord.forge_attempts` —— 本 plan 独立维护锻造史。
//!
//! 亡者博物馆未来会合并至 cultivation LifeRecord，本切片先用独立 Component，
//! 避免强耦合修炼 plan 的 biography enum。

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component};

use super::blueprint::BlueprintId;
use super::events::ForgeBucket;
use crate::cultivation::components::ColorKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeAttempt {
    pub tick: u64,
    pub blueprint: BlueprintId,
    pub bucket_tag: String,
    pub achieved_tier: u8,
    pub weapon_item: Option<String>,
    pub quality: f32,
    pub color: Option<ColorKind>,
    pub side_effects: Vec<String>,
}

impl ForgeAttempt {
    pub fn from_bucket(bucket: &ForgeBucket) -> String {
        match bucket {
            ForgeBucket::Perfect => "perfect",
            ForgeBucket::Good => "good",
            ForgeBucket::Flawed => "flawed",
            ForgeBucket::Waste => "waste",
            ForgeBucket::Explode => "explode",
        }
        .to_string()
    }
}

#[derive(Debug, Clone, Default, Component, Serialize, Deserialize)]
pub struct ForgeHistory {
    pub attempts: Vec<ForgeAttempt>,
}

impl ForgeHistory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, attempt: ForgeAttempt) {
        self.attempts.push(attempt);
    }

    pub fn recent(&self, n: usize) -> &[ForgeAttempt] {
        let start = self.attempts.len().saturating_sub(n);
        &self.attempts[start..]
    }
}
