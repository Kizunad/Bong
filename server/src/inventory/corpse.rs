//! plan-tsy-loot-v1 §4 — 干尸（CorpseEmbalmed）。
//!
//! 玩家在 TSY 内死亡 → 真元抽干 → 干尸。本模块只提供 component 定义；
//! - 实际 spawn 由 `apply_death_drop_on_revive` 在 §3 路径里 `commands.spawn`
//! - 后续 P2 plan-tsy-lifecycle 读 `activated_to_daoxiang` 决定是否激活成"道伥"
//!
//! MVP 干尸不持久化：服务器重启后丢失（与 plan §10 风险表中标记一致）。

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component};

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
        };
        assert_eq!(corpse.family_id, "tsy_lingxu_01");
        assert_eq!(corpse.died_at_tick, 1234);
        assert_eq!(corpse.death_cause, "tsy_drain");
        assert_eq!(corpse.drops, vec![10, 11, 12]);
        assert!(!corpse.activated_to_daoxiang);
    }

    #[test]
    fn corpse_embalmed_serde_round_trip() {
        let corpse = CorpseEmbalmed {
            family_id: "tsy_zongmen_01".into(),
            died_at_tick: 9999,
            death_cause: "attack_intent:offline:Bob".into(),
            drops: vec![1, 2, 3, 4, 5],
            activated_to_daoxiang: true,
        };
        let json = serde_json::to_string(&corpse).expect("serialize");
        let back: CorpseEmbalmed = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(corpse, back);
    }
}
