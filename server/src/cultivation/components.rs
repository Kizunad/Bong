//! P1 cultivation components.
//!
//! Minimal slice: Cultivation / MeridianSystem / QiColor / Karma, plus the
//! enums they depend on. Mutations, breakthrough logic, forging, and color
//! evolution live in later slices; this file only defines state shape and
//! trivially pure helpers.

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component};

/// 修为境界 — see plan §1.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Realm {
    Awaken,   // 醒灵
    Induce,   // 引气
    Condense, // 凝脉
    Solidify, // 固元
    Spirit,   // 通灵
    Void,     // 化虚
}

impl Realm {
    pub fn previous(self) -> Option<Self> {
        match self {
            Realm::Awaken => None,
            Realm::Induce => Some(Realm::Awaken),
            Realm::Condense => Some(Realm::Induce),
            Realm::Solidify => Some(Realm::Condense),
            Realm::Spirit => Some(Realm::Solidify),
            Realm::Void => Some(Realm::Spirit),
        }
    }

    /// 此境界需要已打通的经脉数量（含正经 + 奇经，参考 plan §3.1）。
    pub fn required_meridians(self) -> usize {
        match self {
            Realm::Awaken => 1,
            Realm::Induce => 3,
            Realm::Condense => 6,
            Realm::Solidify => 12,
            Realm::Spirit => 16,
            Realm::Void => 20,
        }
    }
}

/// 20 条经脉（12 正经 + 8 奇经）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MeridianId {
    // 12 正经
    Lung,
    LargeIntestine,
    Stomach,
    Spleen,
    Heart,
    SmallIntestine,
    Bladder,
    Kidney,
    Pericardium,
    TripleEnergizer,
    Gallbladder,
    Liver,
    // 8 奇经
    Ren,
    Du,
    Chong,
    Dai,
    YinQiao,
    YangQiao,
    YinWei,
    YangWei,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeridianFamily {
    Regular,
    Extraordinary,
}

impl MeridianId {
    pub const REGULAR: [MeridianId; 12] = [
        MeridianId::Lung,
        MeridianId::LargeIntestine,
        MeridianId::Stomach,
        MeridianId::Spleen,
        MeridianId::Heart,
        MeridianId::SmallIntestine,
        MeridianId::Bladder,
        MeridianId::Kidney,
        MeridianId::Pericardium,
        MeridianId::TripleEnergizer,
        MeridianId::Gallbladder,
        MeridianId::Liver,
    ];

    pub const EXTRAORDINARY: [MeridianId; 8] = [
        MeridianId::Ren,
        MeridianId::Du,
        MeridianId::Chong,
        MeridianId::Dai,
        MeridianId::YinQiao,
        MeridianId::YangQiao,
        MeridianId::YinWei,
        MeridianId::YangWei,
    ];

    pub const ALL: [MeridianId; 20] = [
        MeridianId::Lung,
        MeridianId::LargeIntestine,
        MeridianId::Stomach,
        MeridianId::Spleen,
        MeridianId::Heart,
        MeridianId::SmallIntestine,
        MeridianId::Bladder,
        MeridianId::Kidney,
        MeridianId::Pericardium,
        MeridianId::TripleEnergizer,
        MeridianId::Gallbladder,
        MeridianId::Liver,
        MeridianId::Ren,
        MeridianId::Du,
        MeridianId::Chong,
        MeridianId::Dai,
        MeridianId::YinQiao,
        MeridianId::YangQiao,
        MeridianId::YinWei,
        MeridianId::YangWei,
    ];

    pub fn family(self) -> MeridianFamily {
        if Self::REGULAR.contains(&self) {
            MeridianFamily::Regular
        } else {
            MeridianFamily::Extraordinary
        }
    }
}

/// 单条经脉。`flow_rate` / `flow_capacity` 是相互独立的锻造轴
/// （plan §3.3）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meridian {
    pub id: MeridianId,
    pub opened: bool,
    pub open_progress: f64, // 0.0..=1.0, 未打通时累积
    pub flow_rate: f64,
    pub flow_capacity: f64,
    pub rate_tier: u8,
    pub capacity_tier: u8,
    pub throughput_current: f64,
    pub integrity: f64, // 0.0..=1.0
    pub cracks: Vec<MeridianCrack>,
    pub opened_at: u64, // tick 时戳，LIFO 排序用
}

/// 经脉裂痕（plan §1.1）。严重度 0..=1。`healing_progress` 达到 severity
/// 时移除。成因区分过载 / 被攻击 / 走火 / 淬炼失败。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeridianCrack {
    pub severity: f64,
    pub healing_progress: f64,
    pub cause: CrackCause,
    pub created_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CrackCause {
    Overload,
    Attack,
    Backfire,        // 走火入魔
    ForgeFailure,    // 淬炼失败
    VoluntarySever,  // plan-meridian-severed-v1 §4 #1：zhenmai ⑤ 主动绝脉
    TribulationFail, // plan-meridian-severed-v1 §4 #5：渡劫失败爆脉
    DuguDistortion,  // plan-meridian-severed-v1 §4 #6：dugu 阴诡色形貌异化侵蚀
}

impl Meridian {
    pub fn new(id: MeridianId) -> Self {
        Self {
            id,
            opened: false,
            open_progress: 0.0,
            flow_rate: 1.0,
            flow_capacity: 10.0,
            rate_tier: 0,
            capacity_tier: 0,
            throughput_current: 0.0,
            integrity: 1.0,
            cracks: Vec::new(),
            opened_at: 0,
        }
    }

    /// plan §3.3.2 渐进非线性 flow_rate 曲线。
    pub fn rate_for_tier(tier: u8) -> f64 {
        const CURVE: [f64; 11] = [1.0, 2.0, 3.0, 5.0, 8.0, 12.0, 17.0, 23.0, 30.0, 40.0, 55.0];
        CURVE[(tier as usize).min(10)]
    }

    /// plan §3.3.2 flow_capacity 曲线（以 10 为打通基准，tier 0 即基础容量）。
    pub fn capacity_for_tier(tier: u8) -> f64 {
        const CURVE: [f64; 11] = [
            10.0, 20.0, 30.0, 40.0, 60.0, 90.0, 130.0, 180.0, 250.0, 350.0, 500.0,
        ];
        CURVE[(tier as usize).min(10)]
    }
}

/// 异种真元污染源（plan §1.1）。由战斗 plan 写入，本 plan 负责排异演化。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContamSource {
    pub amount: f64,
    pub color: ColorKind,
    #[serde(default)]
    pub attacker_id: Option<String>,
    pub introduced_at: u64,
}

/// 污染 Component。`entries` 为空 = 纯净。
#[derive(Debug, Clone, Default, Component, Serialize, Deserialize)]
pub struct Contamination {
    pub entries: Vec<ContamSource>,
}

/// 玩家 20 经脉系统。固定长度数组，顺序与 `MeridianId::REGULAR` /
/// `MeridianId::EXTRAORDINARY` 一致。
#[derive(Debug, Clone, Component, Serialize, Deserialize)]
pub struct MeridianSystem {
    pub regular: [Meridian; 12],
    pub extraordinary: [Meridian; 8],
}

impl Default for MeridianSystem {
    fn default() -> Self {
        Self {
            regular: MeridianId::REGULAR.map(Meridian::new),
            extraordinary: MeridianId::EXTRAORDINARY.map(Meridian::new),
        }
    }
}

impl MeridianSystem {
    pub fn iter(&self) -> impl Iterator<Item = &Meridian> {
        self.regular.iter().chain(self.extraordinary.iter())
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Meridian> {
        self.regular.iter_mut().chain(self.extraordinary.iter_mut())
    }

    pub fn opened_count(&self) -> usize {
        self.iter().filter(|m| m.opened).count()
    }

    pub fn regular_opened_count(&self) -> usize {
        self.regular.iter().filter(|m| m.opened).count()
    }

    pub fn extraordinary_opened_count(&self) -> usize {
        self.extraordinary.iter().filter(|m| m.opened).count()
    }

    pub fn sum_capacity(&self) -> f64 {
        self.iter()
            .filter(|m| m.opened)
            .map(|m| m.flow_capacity)
            .sum()
    }

    pub fn sum_rate(&self) -> f64 {
        self.iter().filter(|m| m.opened).map(|m| m.flow_rate).sum()
    }

    pub fn get(&self, id: MeridianId) -> &Meridian {
        if let Some(idx) = MeridianId::REGULAR.iter().position(|x| *x == id) {
            &self.regular[idx]
        } else {
            let idx = MeridianId::EXTRAORDINARY
                .iter()
                .position(|x| *x == id)
                .expect("MeridianId must be regular or extraordinary");
            &self.extraordinary[idx]
        }
    }

    pub fn get_mut(&mut self, id: MeridianId) -> &mut Meridian {
        if let Some(idx) = MeridianId::REGULAR.iter().position(|x| *x == id) {
            &mut self.regular[idx]
        } else {
            let idx = MeridianId::EXTRAORDINARY
                .iter()
                .position(|x| *x == id)
                .expect("MeridianId must be regular or extraordinary");
            &mut self.extraordinary[idx]
        }
    }
}

/// 10 种真元色（plan §1.1 / worldview §六）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ColorKind {
    Sharp,     // 锐
    Heavy,     // 厚
    Mellow,    // 醇
    Solid,     // 实
    Light,     // 轻
    Intricate, // 巧
    Gentle,    // 柔
    Insidious, // 阴
    Violent,   // 烈
    Turbid,    // 浊
}

#[derive(Debug, Clone, Component, Serialize, Deserialize)]
pub struct QiColor {
    pub main: ColorKind,
    pub secondary: Option<ColorKind>,
    pub is_chaotic: bool,
    pub is_hunyuan: bool,
}

impl Default for QiColor {
    fn default() -> Self {
        Self {
            main: ColorKind::Mellow,
            secondary: None,
            is_chaotic: false,
            is_hunyuan: false,
        }
    }
}

/// 修为主组件。`qi_max_frozen` 用于 QiZeroDecay 窗口期（plan §2）。
#[derive(Debug, Clone, Component, Serialize, Deserialize)]
pub struct Cultivation {
    pub realm: Realm,
    pub qi_current: f64,
    pub qi_max: f64,
    pub qi_max_frozen: Option<f64>,
    pub last_qi_zero_at: Option<u64>, // tick 计数
    pub pending_material_bonus: f64,
    pub composure: f64, // 0.0..=1.0
    pub composure_recover_rate: f64,
}

impl Default for Cultivation {
    fn default() -> Self {
        Self {
            realm: Realm::Awaken,
            qi_current: 0.0,
            qi_max: 10.0,
            qi_max_frozen: None,
            last_qi_zero_at: None,
            pending_material_bonus: 0.0,
            composure: 1.0,
            composure_recover_rate: 0.001,
        }
    }
}

pub fn recover_current_qi(cultivation: &mut Cultivation, amount: f64) -> f64 {
    let amount = if amount.is_finite() {
        amount.max(0.0)
    } else {
        0.0
    };
    let effective_qi_max = (cultivation.qi_max - cultivation.qi_max_frozen.unwrap_or(0.0)).max(0.0);
    let before = cultivation.qi_current;
    cultivation.qi_current = (cultivation.qi_current + amount).clamp(0.0, effective_qi_max);
    (cultivation.qi_current - before).max(0.0)
}

/// 因果 / 业力（plan §1.1）— P1 仅存储权重，推演留给后续切片。
#[derive(Debug, Clone, Default, Component, Serialize, Deserialize)]
pub struct Karma {
    pub weight: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashSet;

    use crate::npc::brain::canonical_npc_id;
    use crate::player::state::canonical_player_id;
    use valence::prelude::App;

    #[test]
    fn realm_previous_chain() {
        assert_eq!(Realm::Awaken.previous(), None);
        assert_eq!(Realm::Void.previous(), Some(Realm::Spirit));
    }

    #[test]
    fn realm_meridian_requirements_monotonic() {
        let chain = [
            Realm::Awaken,
            Realm::Induce,
            Realm::Condense,
            Realm::Solidify,
            Realm::Spirit,
            Realm::Void,
        ];
        let mut prev = 0;
        for r in chain {
            let n = r.required_meridians();
            assert!(n > prev, "required_meridians must strictly increase");
            prev = n;
        }
        assert_eq!(chain.map(Realm::required_meridians), [1, 3, 6, 12, 16, 20]);
        assert_eq!(Realm::Void.required_meridians(), 20);
    }

    #[test]
    fn meridian_all_matches_partition_without_duplicates() {
        let merged: Vec<MeridianId> = MeridianId::REGULAR
            .iter()
            .chain(MeridianId::EXTRAORDINARY.iter())
            .copied()
            .collect();
        assert_eq!(merged.as_slice(), MeridianId::ALL.as_slice());

        let unique: HashSet<MeridianId> = MeridianId::ALL.iter().copied().collect();
        assert_eq!(unique.len(), MeridianId::ALL.len());
    }

    #[test]
    fn recover_current_qi_clamps_to_effective_max_without_raising_cap() {
        let mut cultivation = Cultivation {
            qi_current: 160.0,
            qi_max: 210.0,
            qi_max_frozen: Some(30.0),
            ..Default::default()
        };

        let recovered = recover_current_qi(&mut cultivation, 80.0);

        assert_eq!(recovered, 20.0);
        assert_eq!(cultivation.qi_current, 180.0);
        assert_eq!(cultivation.qi_max, 210.0);
        assert_eq!(cultivation.qi_max_frozen, Some(30.0));
    }

    #[test]
    fn meridian_system_defaults_20_closed() {
        let ms = MeridianSystem::default();
        assert_eq!(ms.iter().count(), 20);
        assert_eq!(ms.opened_count(), 0);
    }

    #[test]
    fn meridian_family_partition() {
        for id in MeridianId::REGULAR {
            assert_eq!(id.family(), MeridianFamily::Regular);
        }
        for id in MeridianId::EXTRAORDINARY {
            assert_eq!(id.family(), MeridianFamily::Extraordinary);
        }
    }

    #[test]
    fn meridian_get_mut_roundtrip() {
        let mut ms = MeridianSystem::default();
        ms.get_mut(MeridianId::Lung).opened = true;
        assert!(ms.get(MeridianId::Lung).opened);
        assert_eq!(ms.opened_count(), 1);
        assert_eq!(ms.regular_opened_count(), 1);
        assert_eq!(ms.extraordinary_opened_count(), 0);
    }

    #[test]
    fn meridian_family_opened_counts_are_separate() {
        let mut ms = MeridianSystem::default();
        ms.get_mut(MeridianId::Lung).opened = true;
        ms.get_mut(MeridianId::Ren).opened = true;
        ms.get_mut(MeridianId::Du).opened = true;

        assert_eq!(ms.opened_count(), 3);
        assert_eq!(ms.regular_opened_count(), 1);
        assert_eq!(ms.extraordinary_opened_count(), 2);
    }

    #[test]
    fn contam_source_serde_roundtrip_preserves_attacker_id() {
        let source = ContamSource {
            amount: 2.5,
            color: ColorKind::Violent,
            attacker_id: Some(canonical_player_id("Alice")),
            introduced_at: 77,
        };

        let json = serde_json::to_string(&source).expect("contam source should serialize");
        let decoded: ContamSource =
            serde_json::from_str(&json).expect("contam source should deserialize");

        assert_eq!(decoded.attacker_id.as_deref(), Some("offline:Alice"));
        assert_eq!(decoded.introduced_at, 77);
    }

    #[test]
    fn contam_source_serde_defaults_missing_attacker_id_for_legacy_payloads() {
        let legacy = serde_json::json!({
            "amount": 1.0,
            "color": "Sharp",
            "introduced_at": 9,
        });

        let decoded: ContamSource =
            serde_json::from_value(legacy).expect("legacy contam payload should deserialize");

        assert_eq!(decoded.attacker_id, None);
        assert_eq!(decoded.introduced_at, 9);
    }

    #[test]
    fn canonical_identity_anchors_are_string_based_not_transient_ids() {
        let mut app = App::new();
        let entity = app.world_mut().spawn_empty().id();
        let player_id = canonical_player_id("Wanderer");
        let npc_id = canonical_npc_id(entity);

        assert_eq!(player_id, "offline:Wanderer");
        assert!(npc_id.starts_with(&format!("npc_{}", entity.index())));
        assert!(npc_id.ends_with(&format!("v{}", entity.generation())));
        assert_ne!(npc_id, format!("npc_{}", entity.index()));
        assert_ne!(npc_id, entity.index().to_string());
    }
}
