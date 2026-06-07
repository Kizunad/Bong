//! plan-tsy-loot-v1 §4 — 干尸（CorpseEmbalmed）。
//!
//! 玩家在 TSY 内死亡 → 真元抽干 → 干尸。本模块只提供 component 定义；
//! - 实际 spawn 由 `apply_death_drop_on_revive` 在 §3 路径里 `commands.spawn`
//! - 后续 P2 plan-tsy-lifecycle 读 `activated_to_daoxiang` 决定是否激活成"道伥"
//!
//! MVP 干尸不持久化：服务器重启后丢失（与 plan §10 风险表中标记一致）。

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component};

use crate::cultivation::components::Realm;

/// 干尸实体的 marker component。
///
/// `drops` 是死亡点散落的 instance_id 列表（既包括 entry_carry 50% 也包括 tsy_acquired
/// 100%），方便 P2 lifecycle / P3 polish 系统反向查 "这具干尸提供了哪些 loot"。
/// 真正的 loot 拾取仍走 `DroppedLootRegistry`。
#[derive(Component, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CorpseEmbalmed {
    /// 所在 TSY family（继承自 `TsyPresence.family_id`）。塌缩时 cleanup 用。
    pub family_id: String,
    /// 死亡 tick（MVP 暂用 entered_at_tick 占位；真 death tick 待 §6 attacker chain 后填）。
    pub died_at_tick: u64,
    /// 死亡原因（`"tsy_drain"` / `"attack_intent:offline:Foo"` / `"bleed_out"` ...）。
    pub death_cause: String,
    /// 死亡点散落的 instance_id（仅作引用，真正掉落物在 DroppedLootRegistry 里）。
    pub drops: Vec<u64>,
    /// 是否已被 P2 lifecycle 激活成道伥。MVP 默认 false。
    pub activated_to_daoxiang: bool,
    /// plan-daozhan-v1 P0 — 死者境界（来自死亡时刻的 Cultivation.realm）。
    ///
    /// 用于道伥 spawn 概率门控（化虚 80% / 通灵 50% / 固元 20%）及 loot 分档。
    /// 历史干尸（升级前创建）无此字段时反序列化为 None，视为低境处理（spawn 概率 0%）。
    #[serde(default)]
    pub origin_realm: Option<Realm>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corpse_embalmed_default_state() {
        let corpse = CorpseEmbalmed {
            family_id: "tsy_lingxu_01".into(),
            died_at_tick: 1234,
            death_cause: "tsy_drain".into(),
            drops: vec![10, 11, 12],
            activated_to_daoxiang: false,
            origin_realm: None,
        };
        assert_eq!(corpse.family_id, "tsy_lingxu_01");
        assert_eq!(corpse.died_at_tick, 1234);
        assert_eq!(corpse.death_cause, "tsy_drain");
        assert_eq!(corpse.drops, vec![10, 11, 12]);
        assert!(!corpse.activated_to_daoxiang);
        assert!(
            corpse.origin_realm.is_none(),
            "默认 origin_realm 应为 None（向后兼容）"
        );
    }

    #[test]
    fn corpse_embalmed_serde_round_trip() {
        let corpse = CorpseEmbalmed {
            family_id: "tsy_zongmen_01".into(),
            died_at_tick: 9999,
            death_cause: "attack_intent:offline:Bob".into(),
            drops: vec![1, 2, 3, 4, 5],
            activated_to_daoxiang: true,
            origin_realm: Some(Realm::Void),
        };
        let json = serde_json::to_string(&corpse).expect("serialize");
        let back: CorpseEmbalmed = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(corpse, back);
    }

    #[test]
    fn corpse_embalmed_serde_backward_compat_no_origin_realm() {
        // 历史 JSON（无 origin_realm 字段）应能反序列化，origin_realm 默认 None
        let legacy_json = r#"{"family_id":"tsy_old","died_at_tick":100,"death_cause":"tsy_drain","drops":[],"activated_to_daoxiang":false}"#;
        let corpse: CorpseEmbalmed = serde_json::from_str(legacy_json)
            .expect("历史 JSON 应能反序列化（origin_realm 有 #[serde(default)]）");
        assert!(
            corpse.origin_realm.is_none(),
            "旧版 JSON 无 origin_realm 字段，应反序列化为 None"
        );
    }

    #[test]
    fn corpse_embalmed_void_realm() {
        let corpse = CorpseEmbalmed {
            family_id: "tsy_qingyun_01".into(),
            died_at_tick: 5000,
            death_cause: "tsy_collapsed".into(),
            drops: vec![],
            activated_to_daoxiang: false,
            origin_realm: Some(Realm::Void),
        };
        assert_eq!(
            corpse.origin_realm,
            Some(Realm::Void),
            "化虚境界干尸应记录 Realm::Void"
        );
    }
}
