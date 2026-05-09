//! plan-identity-v1 P5：server → client `identity_panel_state` 同步。

use valence::prelude::{Client, DetectChanges, Query, Ref, Res, With};

use crate::identity::{PlayerIdentities, RevealedTagKind, IDENTITY_SWITCH_COOLDOWN_TICKS};
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::npc::movement::GameTick;
use crate::schema::identity::{IdentityPanelEntryV1, IdentityPanelStateV1, RevealedTagKindV1};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

const IDENTITY_PANEL_COOLDOWN_REFRESH_INTERVAL_TICKS: u64 = 20;

type IdentityPanelClientQueryItem<'a> = (Ref<'a, PlayerIdentities>, &'a mut Client);

pub fn emit_identity_panel_state_payloads(
    mut clients: Query<IdentityPanelClientQueryItem<'_>, With<Client>>,
    game_tick: Option<Res<GameTick>>,
) {
    let cooldown_refresh_enabled = game_tick.is_some();
    let now_tick = game_tick.as_deref().map(|tick| tick.0 as u64).unwrap_or(0);
    for (identities, mut client) in clients.iter_mut() {
        let identity_changed = identities.is_added() || identities.is_changed();
        if !should_emit_identity_panel_state(
            &identities,
            now_tick,
            identity_changed,
            cooldown_refresh_enabled,
        ) {
            continue;
        }
        let state = build_identity_panel_state(&identities, now_tick);
        let payload = ServerDataV1::new(ServerDataPayloadV1::IdentityPanelState(state));
        let payload_type = payload_type_label(payload.payload_type());
        let payload_bytes = match serialize_server_data_payload(&payload) {
            Ok(bytes) => bytes,
            Err(error) => {
                log_payload_build_error(payload_type, &error);
                continue;
            }
        };
        send_server_data_payload(&mut client, payload_bytes.as_slice());
    }
}

fn should_emit_identity_panel_state(
    identities: &PlayerIdentities,
    now_tick: u64,
    identity_changed: bool,
    cooldown_refresh_enabled: bool,
) -> bool {
    if identity_changed {
        return true;
    }
    if !cooldown_refresh_enabled || identities.last_switch_tick == 0 {
        return false;
    }
    if now_tick % IDENTITY_PANEL_COOLDOWN_REFRESH_INTERVAL_TICKS != 0 {
        return false;
    }
    let cooldown_refresh_deadline = identities
        .last_switch_tick
        .saturating_add(IDENTITY_SWITCH_COOLDOWN_TICKS)
        .saturating_add(IDENTITY_PANEL_COOLDOWN_REFRESH_INTERVAL_TICKS);
    now_tick <= cooldown_refresh_deadline
}

pub fn build_identity_panel_state(
    identities: &PlayerIdentities,
    now_tick: u64,
) -> IdentityPanelStateV1 {
    IdentityPanelStateV1 {
        active_identity_id: identities.active_identity_id.0,
        last_switch_tick: identities.last_switch_tick,
        cooldown_remaining_ticks: identities.cooldown_remaining(now_tick),
        identities: identities
            .identities
            .iter()
            .map(|profile| IdentityPanelEntryV1 {
                identity_id: profile.id.0,
                display_name: profile.display_name.clone(),
                reputation_score: profile.reputation_score(),
                frozen: profile.frozen,
                revealed_tag_kinds: profile
                    .revealed_tags
                    .iter()
                    .map(|tag| revealed_tag_kind_to_v1(tag.kind))
                    .collect(),
            })
            .collect(),
    }
}

fn revealed_tag_kind_to_v1(kind: RevealedTagKind) -> RevealedTagKindV1 {
    match kind {
        RevealedTagKind::DuguRevealed => RevealedTagKindV1::DuguRevealed,
        RevealedTagKind::AnqiMaster => RevealedTagKindV1::AnqiMaster,
        RevealedTagKind::ZhenfaMaster => RevealedTagKindV1::ZhenfaMaster,
        RevealedTagKind::BaomaiUser => RevealedTagKindV1::BaomaiUser,
        RevealedTagKind::TuikeUser => RevealedTagKindV1::TuikeUser,
        RevealedTagKind::WoliuMaster => RevealedTagKindV1::WoliuMaster,
        RevealedTagKind::ZhenmaiUser => RevealedTagKindV1::ZhenmaiUser,
        RevealedTagKind::SwordMaster => RevealedTagKindV1::SwordMaster,
        RevealedTagKind::ForgeMaster => RevealedTagKindV1::ForgeMaster,
        RevealedTagKind::AlchemyMaster => RevealedTagKindV1::AlchemyMaster,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::Realm;
    use crate::identity::{IdentityId, IdentityProfile, RevealedTag};

    #[test]
    fn panel_state_contains_active_id_cooldown_and_entries() {
        let mut identities = PlayerIdentities::with_default("旧名", 10);
        identities.identities[0].frozen = true;
        identities.identities[0].revealed_tags.push(RevealedTag {
            kind: RevealedTagKind::DuguRevealed,
            witnessed_at_tick: 20,
            witness_realm: Realm::Spirit,
            permanent: true,
        });
        identities
            .identities
            .push(IdentityProfile::new(IdentityId(1), "新名", 100));
        identities.active_identity_id = IdentityId(1);
        identities.last_switch_tick = 100;

        let state = build_identity_panel_state(&identities, 12_100);

        assert_eq!(state.active_identity_id, 1);
        assert_eq!(state.last_switch_tick, 100);
        assert_eq!(state.cooldown_remaining_ticks, 12_000);
        assert_eq!(state.identities.len(), 2);
        assert_eq!(state.identities[0].display_name, "旧名");
        assert!(state.identities[0].frozen);
        assert_eq!(
            state.identities[0].revealed_tag_kinds,
            vec![RevealedTagKindV1::DuguRevealed]
        );
        assert_eq!(state.identities[1].display_name, "新名");
        assert!(!state.identities[1].frozen);
    }

    #[test]
    fn panel_state_refreshes_during_cooldown_and_once_after_end() {
        let mut identities = PlayerIdentities::with_default("kiz", 0);
        identities.last_switch_tick = 101;

        assert!(should_emit_identity_panel_state(
            &identities,
            120,
            false,
            true
        ));
        assert!(!should_emit_identity_panel_state(
            &identities,
            119,
            false,
            true
        ));
        assert!(should_emit_identity_panel_state(
            &identities,
            24_120,
            false,
            true
        ));
        assert!(!should_emit_identity_panel_state(
            &identities,
            24_140,
            false,
            true
        ));
        assert!(!should_emit_identity_panel_state(
            &identities,
            120,
            false,
            false
        ));
    }

    #[test]
    fn panel_state_still_emits_on_identity_change() {
        let identities = PlayerIdentities::with_default("kiz", 0);

        assert!(should_emit_identity_panel_state(
            &identities,
            119,
            true,
            false
        ));
    }

    #[test]
    fn panel_state_serializes_as_server_data_identity_panel_state() {
        let identities = PlayerIdentities::with_default("kiz", 0);
        let payload = ServerDataV1::new(ServerDataPayloadV1::IdentityPanelState(
            build_identity_panel_state(&identities, 0),
        ));

        let json = serde_json::to_value(&payload).expect("server_data should serialize");
        assert_eq!(json["v"], 1);
        assert_eq!(json["type"], "identity_panel_state");
        assert_eq!(json["active_identity_id"], 0);

        let round_trip: ServerDataV1 =
            serde_json::from_value(json).expect("server_data should deserialize");
        assert_eq!(
            round_trip.payload_type(),
            crate::schema::server_data::ServerDataType::IdentityPanelState
        );
    }
}
