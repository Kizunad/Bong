//! 通用 `RevealedEvent` trait + 泛型 consumer（plan-identity-v1 P4）。
//!
//! 6 流派 vN+1 各自的 "被识破" 事件（anqi / zhenfa / baomai / tuike / woliu /
//! zhenmai）都按本 trait 实现，便于：
//!
//! 1. 统一 RevealedTag 写入逻辑（[`consume_revealed_event`] 泛型 system）
//! 2. agent / client 下游订阅可基于 trait abstraction 处理
//! 3. plan-identity-v1 自身的 dugu consumer 在 P2 用了直接实现，本 P 把它升级为
//!    trait-based 实现（行为完全一致，仅多一层抽象）
//!
//! ## 接入示例（流派 vN+1）
//!
//! ```ignore
//! impl RevealedEvent for AnqiRevealedEvent {
//!     fn revealed_player(&self) -> Entity { self.revealed_player }
//!     fn witness(&self) -> Entity { self.witness }
//!     fn witness_realm(&self) -> Realm { self.witness_realm }
//!     fn revealed_tag_kind(&self) -> RevealedTagKind { RevealedTagKind::AnqiMaster }
//!     fn is_permanent(&self) -> bool { false }   // 暗器流可衰减，dugu 才永久
//!     fn at_tick(&self) -> u64 { self.at_tick }
//!     fn at_position(&self) -> [f64; 3] { self.at_position }
//! }
//!
//! // 注册：
//! app.add_systems(Update, consume_revealed_event::<AnqiRevealedEvent>);
//! ```

use valence::prelude::{Entity, Event, EventReader, Query, Res, With};

use super::{PlayerIdentities, RevealedTag, RevealedTagKind};
use crate::combat::components::Lifecycle;
use crate::cultivation::components::Realm;
use crate::cultivation::dugu::DuguRevealedEvent;
use crate::persistence::identity as identity_db;
use crate::persistence::PersistenceSettings;

/// 招式 / 流派 "被识破" 事件抽象。
///
/// 实现该 trait 的事件可被 [`consume_revealed_event`] 泛型 system 消费，
/// 自动写入对应玩家 active identity 的 [`RevealedTag`]。
pub trait RevealedEvent: Event + Send + Sync + Clone {
    fn revealed_player(&self) -> Entity;
    fn witness(&self) -> Entity;
    fn witness_realm(&self) -> Realm;
    fn revealed_tag_kind(&self) -> RevealedTagKind;
    fn is_permanent(&self) -> bool;
    fn at_tick(&self) -> u64;
    fn at_position(&self) -> [f64; 3];
}

impl RevealedEvent for DuguRevealedEvent {
    fn revealed_player(&self) -> Entity {
        self.revealed_player
    }
    fn witness(&self) -> Entity {
        self.witness
    }
    fn witness_realm(&self) -> Realm {
        self.witness_realm
    }
    fn revealed_tag_kind(&self) -> RevealedTagKind {
        RevealedTagKind::DuguRevealed
    }
    fn is_permanent(&self) -> bool {
        true
    }
    fn at_tick(&self) -> u64 {
        self.at_tick
    }
    fn at_position(&self) -> [f64; 3] {
        self.at_position
    }
}

/// 泛型 consumer：消费任意 [`RevealedEvent`] 写入 active identity 的 RevealedTag。
///
/// dedup by kind：同 active identity 内同 kind 的 tag 只保留首次写入（避免重复扣分）。
/// 持久化在写入成功（首次）时同步保存。
pub fn consume_revealed_event<E: RevealedEvent>(
    mut events: EventReader<E>,
    mut players: Query<(&mut PlayerIdentities, &Lifecycle), With<valence::prelude::Client>>,
    persistence: Option<Res<PersistenceSettings>>,
) {
    for event in events.read() {
        let Ok((mut identities, lifecycle)) = players.get_mut(event.revealed_player()) else {
            continue;
        };
        let char_id = lifecycle.character_id.clone();
        let written = write_revealed_tag_if_absent(
            &mut identities,
            event.revealed_tag_kind(),
            event.witness_realm(),
            event.is_permanent(),
            event.at_tick(),
        );
        if written {
            if let Some(settings) = persistence.as_deref() {
                if let Err(error) =
                    identity_db::save_player_identities(settings, &char_id, &identities)
                {
                    tracing::warn!(
                        ?error,
                        char_id,
                        kind = ?event.revealed_tag_kind(),
                        "[bong][identity] revealed-event consumer persistence save failed"
                    );
                }
            }
        }
    }
}

/// 把 [`RevealedTag`] 写到 active identity，dedup by kind。
///
/// 与 [`crate::identity::dugu_consumer::write_dugu_tag_if_absent`] 同语义但泛化到
/// 任意 [`RevealedTagKind`]。
pub fn write_revealed_tag_if_absent(
    identities: &mut PlayerIdentities,
    kind: RevealedTagKind,
    witness_realm: Realm,
    permanent: bool,
    at_tick: u64,
) -> bool {
    let Some(active) = identities.active_mut() else {
        return false;
    };
    if active.has_tag(kind) {
        return false;
    }
    active.revealed_tags.push(RevealedTag {
        kind,
        witnessed_at_tick: at_tick,
        witness_realm,
        permanent,
    });
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::{IdentityId, IdentityProfile, RevealedTagKind};

    fn sample_dugu_event(player: Entity, witness: Entity) -> DuguRevealedEvent {
        DuguRevealedEvent {
            revealed_player: player,
            witness,
            witness_realm: Realm::Spirit,
            at_position: [10.0, 64.0, 10.0],
            at_tick: 100,
        }
    }

    #[test]
    fn dugu_revealed_event_implements_revealed_event_trait() {
        let player = Entity::from_raw(1);
        let witness = Entity::from_raw(2);
        let event = sample_dugu_event(player, witness);
        // 通过 trait 调用每个 accessor，确保 impl 命中
        let event: &dyn RevealedEventDyn = &event;
        assert_eq!(event.revealed_player(), player);
        assert_eq!(event.witness(), witness);
        assert_eq!(event.witness_realm(), Realm::Spirit);
        assert_eq!(event.revealed_tag_kind(), RevealedTagKind::DuguRevealed);
        assert!(event.is_permanent());
        assert_eq!(event.at_tick(), 100);
        assert_eq!(event.at_position(), [10.0, 64.0, 10.0]);
    }

    /// 因为 [`RevealedEvent`] 自身有 `Event + Send + Sync + Clone` bound，单独包成
    /// dyn-safe 子 trait 用于上面这类 trait-object 测试。生产代码不需要这个。
    trait RevealedEventDyn {
        fn revealed_player(&self) -> Entity;
        fn witness(&self) -> Entity;
        fn witness_realm(&self) -> Realm;
        fn revealed_tag_kind(&self) -> RevealedTagKind;
        fn is_permanent(&self) -> bool;
        fn at_tick(&self) -> u64;
        fn at_position(&self) -> [f64; 3];
    }
    impl<E: RevealedEvent> RevealedEventDyn for E {
        fn revealed_player(&self) -> Entity {
            <E as RevealedEvent>::revealed_player(self)
        }
        fn witness(&self) -> Entity {
            <E as RevealedEvent>::witness(self)
        }
        fn witness_realm(&self) -> Realm {
            <E as RevealedEvent>::witness_realm(self)
        }
        fn revealed_tag_kind(&self) -> RevealedTagKind {
            <E as RevealedEvent>::revealed_tag_kind(self)
        }
        fn is_permanent(&self) -> bool {
            <E as RevealedEvent>::is_permanent(self)
        }
        fn at_tick(&self) -> u64 {
            <E as RevealedEvent>::at_tick(self)
        }
        fn at_position(&self) -> [f64; 3] {
            <E as RevealedEvent>::at_position(self)
        }
    }

    #[test]
    fn write_revealed_tag_first_call_returns_true() {
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        assert!(write_revealed_tag_if_absent(
            &mut pid,
            RevealedTagKind::AnqiMaster,
            Realm::Solidify,
            false,
            200
        ));
        let active = pid.active().unwrap();
        assert_eq!(active.revealed_tags.len(), 1);
        assert_eq!(active.revealed_tags[0].kind, RevealedTagKind::AnqiMaster);
        assert!(!active.revealed_tags[0].permanent);
    }

    #[test]
    fn write_revealed_tag_idempotent_per_kind() {
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        assert!(write_revealed_tag_if_absent(
            &mut pid,
            RevealedTagKind::ZhenfaMaster,
            Realm::Spirit,
            false,
            100
        ));
        assert!(!write_revealed_tag_if_absent(
            &mut pid,
            RevealedTagKind::ZhenfaMaster,
            Realm::Void,
            false,
            200
        ));
        // 仅 1 条 tag，witnessed_at_tick 保留首次值
        assert_eq!(pid.active().unwrap().revealed_tags.len(), 1);
        assert_eq!(
            pid.active().unwrap().revealed_tags[0].witnessed_at_tick,
            100
        );
    }

    #[test]
    fn write_revealed_tag_different_kinds_coexist() {
        // 同玩家可以同时被识破多个流派——每个 kind 独立 dedup
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        for kind in [
            RevealedTagKind::AnqiMaster,
            RevealedTagKind::ZhenfaMaster,
            RevealedTagKind::DuguRevealed,
        ] {
            assert!(write_revealed_tag_if_absent(
                &mut pid,
                kind,
                Realm::Spirit,
                false,
                100
            ));
        }
        assert_eq!(pid.active().unwrap().revealed_tags.len(), 3);
    }

    #[test]
    fn write_revealed_tag_returns_false_when_no_active() {
        let mut pid = PlayerIdentities::default();
        assert!(!write_revealed_tag_if_absent(
            &mut pid,
            RevealedTagKind::DuguRevealed,
            Realm::Awaken,
            true,
            0
        ));
    }

    #[test]
    fn write_revealed_tag_only_affects_active_identity() {
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        pid.identities
            .push(IdentityProfile::new(IdentityId(1), "alt", 0));
        assert!(write_revealed_tag_if_absent(
            &mut pid,
            RevealedTagKind::ZhenmaiUser,
            Realm::Solidify,
            false,
            100
        ));
        let alt = pid.get(IdentityId(1)).unwrap();
        assert!(alt.revealed_tags.is_empty(), "alt 不应被污染");
        let default = pid.get(IdentityId(0)).unwrap();
        assert_eq!(default.revealed_tags.len(), 1);
    }

    #[test]
    fn revealed_tag_kind_baseline_penalty_is_correct_per_kind() {
        // 完整覆盖 RevealedTagKind 全枚举（防新增变体后忘加分）
        let cases: &[(RevealedTagKind, i32)] = &[
            (RevealedTagKind::DuguRevealed, 50),
            (RevealedTagKind::AnqiMaster, 0),
            (RevealedTagKind::ZhenfaMaster, 0),
            (RevealedTagKind::BaomaiUser, 0),
            (RevealedTagKind::TuikeUser, 0),
            (RevealedTagKind::WoliuMaster, 0),
            (RevealedTagKind::ZhenmaiUser, 0),
            (RevealedTagKind::SwordMaster, 0),
            (RevealedTagKind::ForgeMaster, 0),
            (RevealedTagKind::AlchemyMaster, 0),
        ];
        for (kind, expected) in cases {
            assert_eq!(
                kind.baseline_penalty(),
                *expected,
                "kind={kind:?}; expected baseline_penalty={expected}"
            );
        }
    }

    #[test]
    fn consume_revealed_event_generic_works_for_dugu_event() {
        // App-level 集成测试：DuguRevealedEvent 通过通用 consumer 路径写入 RevealedTag。
        use valence::prelude::{App, Update};
        use valence::testing::create_mock_client;

        let mut app = App::new();
        app.add_event::<DuguRevealedEvent>();
        app.add_systems(Update, consume_revealed_event::<DuguRevealedEvent>);
        app.finish();
        app.cleanup();

        let (client_bundle, _helper) = create_mock_client("RevealedTester");
        let player = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(player).insert((
            PlayerIdentities::with_default("RevealedTester", 0),
            Lifecycle {
                character_id: "offline:RevealedTester".to_string(),
                ..Default::default()
            },
        ));
        let witness = app.world_mut().spawn_empty().id();

        app.world_mut()
            .send_event(sample_dugu_event(player, witness));
        app.update();

        let identities = app
            .world()
            .entity(player)
            .get::<PlayerIdentities>()
            .expect("PlayerIdentities");
        let active = identities.active().unwrap();
        assert_eq!(active.revealed_tags.len(), 1);
        assert_eq!(active.revealed_tags[0].kind, RevealedTagKind::DuguRevealed);
        assert!(active.revealed_tags[0].permanent);
        assert_eq!(active.revealed_tags[0].witnessed_at_tick, 100);
    }

    #[test]
    fn consume_revealed_event_handles_repeated_emit_idempotently() {
        use valence::prelude::{App, Update};
        use valence::testing::create_mock_client;

        let mut app = App::new();
        app.add_event::<DuguRevealedEvent>();
        app.add_systems(Update, consume_revealed_event::<DuguRevealedEvent>);
        app.finish();
        app.cleanup();

        let (client_bundle, _helper) = create_mock_client("RevealedTester");
        let player = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(player).insert((
            PlayerIdentities::with_default("RevealedTester", 0),
            Lifecycle {
                character_id: "offline:RevealedTester".to_string(),
                ..Default::default()
            },
        ));
        let witness = app.world_mut().spawn_empty().id();

        // 同 tick 发 3 次同事件
        for _ in 0..3 {
            app.world_mut()
                .send_event(sample_dugu_event(player, witness));
        }
        app.update();

        let identities = app
            .world()
            .entity(player)
            .get::<PlayerIdentities>()
            .expect("PlayerIdentities");
        let active = identities.active().unwrap();
        assert_eq!(active.revealed_tags.len(), 1, "dedup 应仅保留一条");
    }
}
