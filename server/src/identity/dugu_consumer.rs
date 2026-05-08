//! [`DuguRevealedEvent`] consumer：把"毒蛊师暴露"写为 `RevealedTag::DuguRevealed`
//! 落到玩家当前 active identity（plan-identity-v1 P2）。
//!
//! P4 后本模块的 system 改用 [`super::revealed::consume_revealed_event`] 泛型版本
//! 注册，但仍保留 [`write_dugu_tag_if_absent`] 作为面向 dugu 单一 tag 的 helper /
//! 测试 fixture。dedup / persistence 行为完全一致（worldview §十一 -50 单值锚点）。

use valence::prelude::App;

use super::PlayerIdentities;

/// 注册 dugu consumer：实质上是 [`super::revealed::consume_revealed_event`] 的
/// `DuguRevealedEvent` 单态化（在 `revealed::register` 里一次性注册了；本函数保留
/// 为 P3 之前调用方的 no-op，避免回归改 main.rs 注册顺序）。
pub fn register(_app: &mut App) {
    // 实际系统在 revealed::register 注册（consume_revealed_event::<DuguRevealedEvent>）。
}

/// 写 DuguRevealed tag 的便捷 helper（dedup by kind + permanent=true）。
///
/// 仅 dugu 一个 RevealedTagKind 永久；通用 helper 见
/// [`super::revealed::write_revealed_tag_if_absent`]。
pub fn write_dugu_tag_if_absent(
    identities: &mut PlayerIdentities,
    witness_realm: crate::cultivation::components::Realm,
    at_tick: u64,
) -> bool {
    super::revealed::write_revealed_tag_if_absent(
        identities,
        super::RevealedTagKind::DuguRevealed,
        witness_realm,
        true,
        at_tick,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::Realm;
    use crate::identity::reputation_score;
    use crate::identity::{IdentityId, IdentityProfile, RevealedTagKind};

    #[test]
    fn write_dugu_tag_first_call_returns_true() {
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        assert!(write_dugu_tag_if_absent(&mut pid, Realm::Spirit, 100));
        let active = pid.active().unwrap();
        assert_eq!(active.revealed_tags.len(), 1);
        assert_eq!(active.revealed_tags[0].kind, RevealedTagKind::DuguRevealed);
        assert_eq!(active.revealed_tags[0].witnessed_at_tick, 100);
        assert_eq!(active.revealed_tags[0].witness_realm, Realm::Spirit);
        assert!(active.revealed_tags[0].permanent);
    }

    #[test]
    fn write_dugu_tag_idempotent_returns_false_on_repeat() {
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        assert!(write_dugu_tag_if_absent(&mut pid, Realm::Spirit, 100));
        assert!(!write_dugu_tag_if_absent(&mut pid, Realm::Solidify, 200));
        // 仍然只有一份 tag，witnessed_at_tick 保留首次值
        let active = pid.active().unwrap();
        assert_eq!(active.revealed_tags.len(), 1);
        assert_eq!(active.revealed_tags[0].witnessed_at_tick, 100);
        assert_eq!(active.revealed_tags[0].witness_realm, Realm::Spirit);
    }

    #[test]
    fn write_dugu_tag_only_affects_active_identity() {
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        pid.identities
            .push(IdentityProfile::new(IdentityId(1), "alt", 0));
        // 当前 active = 0
        assert!(write_dugu_tag_if_absent(&mut pid, Realm::Spirit, 100));
        // alt identity 不应被污染
        let alt = pid.get(IdentityId(1)).unwrap();
        assert!(alt.revealed_tags.is_empty());
        // 默认 identity 才有 tag
        let default = pid.get(IdentityId(0)).unwrap();
        assert_eq!(default.revealed_tags.len(), 1);
    }

    #[test]
    fn dugu_tag_lowers_reputation_to_negative_50() {
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        write_dugu_tag_if_absent(&mut pid, Realm::Spirit, 100);
        assert_eq!(reputation_score(pid.active().unwrap()), -50);
    }

    #[test]
    fn dugu_tag_persists_after_freeze_and_unfreeze() {
        // 切到新 identity 后 dugu identity 应保留 tag；切回去 reputation 仍 -50。
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        write_dugu_tag_if_absent(&mut pid, Realm::Spirit, 100);
        // 模拟 P1 switch 流程
        pid.identities[0].frozen = true;
        pid.identities
            .push(IdentityProfile::new(IdentityId(1), "alt", 200));
        pid.active_identity_id = IdentityId(1);
        pid.last_switch_tick = 200;

        // 旧 identity 仍带 tag
        let dugu = pid.get(IdentityId(0)).unwrap();
        assert_eq!(dugu.revealed_tags.len(), 1);
        assert!(dugu.frozen);
        // 新 active 干净
        assert_eq!(reputation_score(pid.active().unwrap()), 0);

        // 切回去 unfreeze
        pid.identities[0].frozen = false;
        pid.identities[1].frozen = true;
        pid.active_identity_id = IdentityId(0);
        // reputation 重新含 -50
        assert_eq!(reputation_score(pid.active().unwrap()), -50);
    }

    #[test]
    fn switching_back_to_dugu_revealed_identity_restores_negative_50() {
        // worldview §十一 "我又把毒蛊师外套穿上了 NPC 又翻脸了"
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        // 给默认 identity 加点 fame，再触发 dugu
        pid.identities[0].renown.fame = 30;
        write_dugu_tag_if_absent(&mut pid, Realm::Solidify, 50);
        // 默认 reputation = 30 - 0 - 50 = -20
        assert_eq!(reputation_score(pid.active().unwrap()), -20);

        // 切到新 identity，reputation 重置
        pid.identities[0].frozen = true;
        pid.identities
            .push(IdentityProfile::new(IdentityId(1), "alt", 100));
        pid.active_identity_id = IdentityId(1);
        assert_eq!(reputation_score(pid.active().unwrap()), 0);

        // 切回去
        pid.identities[1].frozen = true;
        pid.identities[0].frozen = false;
        pid.active_identity_id = IdentityId(0);
        assert_eq!(reputation_score(pid.active().unwrap()), -20);
    }

    #[test]
    fn write_dugu_tag_returns_false_when_no_active() {
        // 极端情况：active_identity_id 指向不存在的 id
        let mut pid = PlayerIdentities::default();
        assert!(!write_dugu_tag_if_absent(&mut pid, Realm::Awaken, 0));
    }

    #[test]
    fn write_dugu_tag_with_each_realm_variant() {
        for realm in [
            Realm::Awaken,
            Realm::Induce,
            Realm::Condense,
            Realm::Solidify,
            Realm::Spirit,
            Realm::Void,
        ] {
            let mut pid = PlayerIdentities::with_default("kiz", 0);
            assert!(write_dugu_tag_if_absent(&mut pid, realm, 100));
            assert_eq!(pid.active().unwrap().revealed_tags[0].witness_realm, realm);
        }
    }
}
