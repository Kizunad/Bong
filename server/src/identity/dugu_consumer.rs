//! [`DuguRevealedEvent`] consumer：把"毒蛊师暴露"写为 `RevealedTag::DuguRevealed`
//! 落到玩家当前 active identity（plan-identity-v1 P2）。
//!
//! 设计要点：
//!
//! - **dedup by kind**：同一 active identity 内 `DuguRevealed` 只保留一份，重复事件
//!   不重复扣 -50（worldview §十一 毒蛊师 -50 baseline 是单值锚点）。
//! - **永久 tag**：[`RevealedTag::permanent = true`]；切身份冻结旧 identity 仍带 tag，
//!   未来切回再触发 -50（plan §0 设计轴心 + §4 切身份消除段）。
//! - **active identity 写入**：事件命中的是当前外貌身份；旧 frozen identity 不被改。
//! - **persistence**：写完同步保存，避免 server 崩了 tag 丢失。

use valence::prelude::{App, EventReader, Query, Res, Update, With};

use super::{PlayerIdentities, RevealedTag, RevealedTagKind};
use crate::combat::components::Lifecycle;
use crate::cultivation::dugu::DuguRevealedEvent;
use crate::persistence::identity as identity_db;
use crate::persistence::PersistenceSettings;

/// 注册 dugu consumer system（在 cultivation::dugu emit 后跑）。
pub fn register(app: &mut App) {
    app.add_systems(Update, consume_dugu_revealed_to_identity_tag);
}

pub fn consume_dugu_revealed_to_identity_tag(
    mut events: EventReader<DuguRevealedEvent>,
    mut players: Query<(&mut PlayerIdentities, &Lifecycle), With<valence::prelude::Client>>,
    persistence: Option<Res<PersistenceSettings>>,
) {
    for event in events.read() {
        let Ok((mut identities, lifecycle)) = players.get_mut(event.revealed_player) else {
            continue;
        };
        let char_id = lifecycle.character_id.clone();
        let written = write_dugu_tag_if_absent(&mut identities, event.witness_realm, event.at_tick);
        if written {
            if let Some(settings) = persistence.as_deref() {
                if let Err(error) =
                    identity_db::save_player_identities(settings, &char_id, &identities)
                {
                    tracing::warn!(
                        ?error,
                        char_id,
                        "[bong][identity] dugu consumer persistence save failed"
                    );
                }
            }
        }
    }
}

/// 把 DuguRevealed tag 写到 active identity，dedup by kind。
///
/// 返回 `true` 表示**确实新增了一条 tag**（首次触发）；`false` 表示已经有该 tag，
/// 调用方据此决定是否触发持久化 / 下游事件。
pub fn write_dugu_tag_if_absent(
    identities: &mut PlayerIdentities,
    witness_realm: crate::cultivation::components::Realm,
    at_tick: u64,
) -> bool {
    let Some(active) = identities.active_mut() else {
        return false;
    };
    if active.has_tag(RevealedTagKind::DuguRevealed) {
        return false;
    }
    active.revealed_tags.push(RevealedTag {
        kind: RevealedTagKind::DuguRevealed,
        witnessed_at_tick: at_tick,
        witness_realm,
        permanent: true,
    });
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::Realm;
    use crate::identity::reputation_score;
    use crate::identity::{IdentityId, IdentityProfile};

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
