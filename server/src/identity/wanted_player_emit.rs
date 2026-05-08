//! plan-identity-v1 P5：玩家 active identity 进入 Wanted (<-75) 后向 agent 发
//! `bong:wanted_player` Redis pub。
//!
//! 触发源：[`crate::identity::events::IdentityReactionChangedEvent`]，仅 `to_tier ==
//! Wanted` 时下发；从 Wanted 退出（如切身份）不下发，由 agent 用退出语义自己处理。

use uuid::Uuid;
use valence::prelude::{App, EventReader, Query, Res, Update, Username, With};

use super::events::IdentityReactionChangedEvent;
use super::reaction::ReactionTier;
use super::PlayerIdentities;
use crate::combat::components::Lifecycle;
use crate::network::redis_bridge::RedisOutbound;
use crate::network::RedisBridgeResource;
use crate::schema::identity::{RevealedTagKindV1, WantedPlayerEventTag, WantedPlayerEventV1};

/// 注册 wanted-player emit 系统。
pub fn register(app: &mut App) {
    app.add_systems(Update, emit_wanted_player_to_redis);
}

/// 根据玩家 active identity 数据 + IdentityReactionChangedEvent 构造 IPC payload。
///
/// 规则：
/// - `event` = "wanted_player"
/// - `player_uuid` = `uuid::Uuid::new_v5(NAMESPACE_OID, char_id)`（与 attach 系统一致）
/// - `primary_tag`：优先取第一条 permanent RevealedTag；否则取第一条 RevealedTag；
///   都没有 → DuguRevealed（保守默认，反应分级跌入 Wanted 通常由毒蛊驱动）
pub fn build_wanted_player_event(
    char_id: &str,
    identities: &PlayerIdentities,
    event: &IdentityReactionChangedEvent,
) -> Option<WantedPlayerEventV1> {
    let active = identities.get(event.identity_id)?;
    let primary_tag = active
        .revealed_tags
        .iter()
        .find(|t| t.permanent)
        .or_else(|| active.revealed_tags.first())
        .map(|t| revealed_tag_kind_to_v1(t.kind))
        .unwrap_or(RevealedTagKindV1::DuguRevealed);

    Some(WantedPlayerEventV1 {
        event: WantedPlayerEventTag::WantedPlayer,
        player_uuid: Uuid::new_v5(&Uuid::NAMESPACE_OID, char_id.as_bytes()).to_string(),
        char_id: char_id.to_string(),
        identity_display_name: active.display_name.clone(),
        identity_id: active.id.0,
        reputation_score: active.reputation_score(),
        primary_tag,
        tick: event.at_tick,
    })
}

fn revealed_tag_kind_to_v1(kind: super::RevealedTagKind) -> RevealedTagKindV1 {
    match kind {
        super::RevealedTagKind::DuguRevealed => RevealedTagKindV1::DuguRevealed,
        super::RevealedTagKind::AnqiMaster => RevealedTagKindV1::AnqiMaster,
        super::RevealedTagKind::ZhenfaMaster => RevealedTagKindV1::ZhenfaMaster,
        super::RevealedTagKind::BaomaiUser => RevealedTagKindV1::BaomaiUser,
        super::RevealedTagKind::TuikeUser => RevealedTagKindV1::TuikeUser,
        super::RevealedTagKind::WoliuMaster => RevealedTagKindV1::WoliuMaster,
        super::RevealedTagKind::ZhenmaiUser => RevealedTagKindV1::ZhenmaiUser,
        super::RevealedTagKind::SwordMaster => RevealedTagKindV1::SwordMaster,
        super::RevealedTagKind::ForgeMaster => RevealedTagKindV1::ForgeMaster,
        super::RevealedTagKind::AlchemyMaster => RevealedTagKindV1::AlchemyMaster,
    }
}

#[allow(unused_variables)]
pub fn emit_wanted_player_to_redis(
    mut events: EventReader<IdentityReactionChangedEvent>,
    players: Query<(&PlayerIdentities, &Lifecycle, &Username), With<valence::prelude::Client>>,
    redis: Option<Res<RedisBridgeResource>>,
) {
    let Some(redis) = redis else {
        events.clear();
        return;
    };
    for event in events.read() {
        if event.to_tier != ReactionTier::Wanted {
            continue;
        }
        let Ok((identities, lifecycle, _username)) = players.get(event.player) else {
            continue;
        };
        let Some(payload) = build_wanted_player_event(&lifecycle.character_id, identities, event)
        else {
            continue;
        };
        let _ = redis.tx_outbound.send(RedisOutbound::WantedPlayer(payload));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::Realm;
    use crate::identity::{IdentityId, RevealedTag, RevealedTagKind};
    use valence::prelude::Entity;

    fn dugu_identities() -> PlayerIdentities {
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        pid.identities[0].renown.notoriety = 30;
        pid.identities[0].revealed_tags.push(RevealedTag {
            kind: RevealedTagKind::DuguRevealed,
            witnessed_at_tick: 50,
            witness_realm: Realm::Spirit,
            permanent: true,
        });
        pid
    }

    fn wanted_event(player: Entity) -> IdentityReactionChangedEvent {
        IdentityReactionChangedEvent {
            player,
            identity_id: IdentityId::DEFAULT,
            from_tier: ReactionTier::Normal,
            to_tier: ReactionTier::Wanted,
            at_tick: 24_000,
        }
    }

    #[test]
    fn build_event_uses_dugu_tag_as_primary() {
        let pid = dugu_identities();
        let payload =
            build_wanted_player_event("offline:kiz", &pid, &wanted_event(Entity::from_raw(1)))
                .expect("payload");

        assert_eq!(payload.event, WantedPlayerEventTag::WantedPlayer);
        assert_eq!(payload.char_id, "offline:kiz");
        assert_eq!(payload.identity_display_name, "kiz");
        assert_eq!(payload.identity_id, 0);
        assert_eq!(payload.primary_tag, RevealedTagKindV1::DuguRevealed);
        assert_eq!(payload.reputation_score, -80);
        assert_eq!(payload.tick, 24_000);
        assert!(!payload.player_uuid.is_empty());
    }

    #[test]
    fn build_event_uses_first_permanent_tag_when_multiple_present() {
        // 即使顺序在前的是非永久 tag，也应优先选 permanent
        let mut pid = dugu_identities();
        pid.identities[0].revealed_tags.insert(
            0,
            RevealedTag {
                kind: RevealedTagKind::AnqiMaster,
                witnessed_at_tick: 10,
                witness_realm: Realm::Solidify,
                permanent: false,
            },
        );
        let payload =
            build_wanted_player_event("offline:kiz", &pid, &wanted_event(Entity::from_raw(1)))
                .expect("payload");
        assert_eq!(payload.primary_tag, RevealedTagKindV1::DuguRevealed);
    }

    #[test]
    fn build_event_falls_back_to_first_tag_when_no_permanent() {
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        pid.identities[0].renown.notoriety = 200;
        pid.identities[0].revealed_tags.push(RevealedTag {
            kind: RevealedTagKind::AnqiMaster,
            witnessed_at_tick: 10,
            witness_realm: Realm::Solidify,
            permanent: false,
        });
        let payload =
            build_wanted_player_event("offline:kiz", &pid, &wanted_event(Entity::from_raw(1)))
                .expect("payload");
        assert_eq!(payload.primary_tag, RevealedTagKindV1::AnqiMaster);
        // notoriety 200 - fame 0 - tag_penalty 0 = -200 (Wanted)
        assert_eq!(payload.reputation_score, -200);
    }

    #[test]
    fn build_event_defaults_primary_tag_to_dugu_when_no_revealed_tags() {
        // 极少见但合法：reputation 进 Wanted（如 -200 notoriety）但无 RevealedTag
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        pid.identities[0].renown.notoriety = 200;
        let payload =
            build_wanted_player_event("offline:kiz", &pid, &wanted_event(Entity::from_raw(1)))
                .expect("payload");
        assert_eq!(payload.primary_tag, RevealedTagKindV1::DuguRevealed);
    }

    #[test]
    fn build_event_returns_none_when_identity_id_unknown() {
        let pid = PlayerIdentities::with_default("kiz", 0);
        let mut event = wanted_event(Entity::from_raw(1));
        event.identity_id = IdentityId(99);
        assert!(build_wanted_player_event("offline:kiz", &pid, &event).is_none());
    }

    #[test]
    fn revealed_tag_kind_to_v1_round_trip_all_variants() {
        for kind in [
            RevealedTagKind::DuguRevealed,
            RevealedTagKind::AnqiMaster,
            RevealedTagKind::ZhenfaMaster,
            RevealedTagKind::BaomaiUser,
            RevealedTagKind::TuikeUser,
            RevealedTagKind::WoliuMaster,
            RevealedTagKind::ZhenmaiUser,
            RevealedTagKind::SwordMaster,
            RevealedTagKind::ForgeMaster,
            RevealedTagKind::AlchemyMaster,
        ] {
            let v1 = revealed_tag_kind_to_v1(kind);
            // 序列化后比对相同的 snake_case 字串
            let server_json = serde_json::to_string(&kind).unwrap();
            let schema_json = serde_json::to_string(&v1).unwrap();
            assert_eq!(server_json, schema_json, "{kind:?} 双端 enum 表达不一致");
        }
    }

    #[test]
    fn player_uuid_v5_deterministic_from_char_id() {
        let pid = dugu_identities();
        let payload_a =
            build_wanted_player_event("offline:kiz", &pid, &wanted_event(Entity::from_raw(1)))
                .expect("payload");
        let payload_b =
            build_wanted_player_event("offline:kiz", &pid, &wanted_event(Entity::from_raw(2)))
                .expect("payload");
        assert_eq!(payload_a.player_uuid, payload_b.player_uuid);

        let payload_c =
            build_wanted_player_event("offline:other", &pid, &wanted_event(Entity::from_raw(1)))
                .expect("payload");
        assert_ne!(payload_a.player_uuid, payload_c.player_uuid);
    }
}
