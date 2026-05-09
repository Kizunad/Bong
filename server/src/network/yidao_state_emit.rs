use std::collections::HashMap;

use valence::prelude::{Client, Entity, Local, Query, Username};

use crate::combat::components::Casting;
use crate::combat::yidao::{
    entity_wire_id, healer_npc_decision, HealerNpcAction, HealerProfile, HealingMastery,
    KarmaCounter, CONTAM_PURGE_SKILL_ID, EMERGENCY_RESUSCITATE_SKILL_ID, LIFE_EXTENSION_SKILL_ID,
    MASS_MERIDIAN_REPAIR_SKILL_ID, MERIDIAN_REPAIR_SKILL_ID,
};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use crate::schema::yidao::{HealerNpcAiStateV1, YidaoHudStateV1, YidaoSkillIdV1};

type YidaoHudEmitItem<'a> = (
    Entity,
    &'a mut Client,
    &'a Username,
    Option<&'a HealerProfile>,
    Option<&'a HealingMastery>,
    Option<&'a KarmaCounter>,
    Option<&'a Casting>,
);

pub fn emit_yidao_hud_state_payloads(
    mut clients: Query<YidaoHudEmitItem<'_>>,
    mut previous: Local<HashMap<Entity, YidaoHudStateV1>>,
) {
    for (entity, mut client, username, profile, mastery, karma, casting) in &mut clients {
        let Some(state) = build_yidao_hud_state(entity, profile, mastery, karma, casting) else {
            continue;
        };
        if previous.get(&entity) == Some(&state) {
            continue;
        }
        previous.insert(entity, state.clone());
        send_yidao_payload(
            &mut client,
            username,
            ServerDataV1::new(ServerDataPayloadV1::YidaoHudState(state)),
        );
    }
}

pub fn emit_healer_npc_ai_state_payloads(
    healers: Query<(Entity, &HealerProfile, Option<&Casting>)>,
    mut clients: Query<(&mut Client, &Username)>,
    mut previous: Local<HashMap<Entity, HealerNpcAiStateV1>>,
) {
    for (healer, profile, casting) in &healers {
        let state = build_healer_npc_ai_state(healer, profile, casting);
        if previous.get(&healer) == Some(&state) {
            continue;
        }
        previous.insert(healer, state.clone());
        let payload = ServerDataV1::new(ServerDataPayloadV1::HealerNpcAiState(state));
        for (mut client, username) in &mut clients {
            send_yidao_payload(&mut client, username, payload.clone());
        }
    }
}

fn build_yidao_hud_state(
    entity: Entity,
    profile: Option<&HealerProfile>,
    mastery: Option<&HealingMastery>,
    karma: Option<&KarmaCounter>,
    casting: Option<&Casting>,
) -> Option<YidaoHudStateV1> {
    if profile.is_none()
        && mastery.is_none()
        && karma.is_none()
        && active_skill_from_casting(casting).is_none()
    {
        return None;
    }
    let patient_ids = profile
        .map(|profile| {
            profile
                .contracts
                .iter()
                .take(16)
                .map(|contract| contract.patient_id.clone())
                .collect()
        })
        .unwrap_or_default();
    Some(YidaoHudStateV1 {
        healer_id: entity_wire_id(entity),
        reputation: profile
            .map(|profile| profile.reputation)
            .unwrap_or_default(),
        peace_mastery: mastery.map(peace_mastery_score).unwrap_or_default(),
        karma: karma
            .map(|karma| karma.yidao_karma.max(0.0))
            .unwrap_or_default(),
        active_skill: active_skill_from_casting(casting),
        patient_ids,
        patient_hp_percent: None,
        patient_contam_total: None,
        severed_meridian_count: 0,
        contract_count: profile
            .map(|profile| profile.contracts.len().min(u32::MAX as usize) as u32)
            .unwrap_or_default(),
        mass_preview_count: 0,
    })
}

fn build_healer_npc_ai_state(
    healer: Entity,
    profile: &HealerProfile,
    casting: Option<&Casting>,
) -> HealerNpcAiStateV1 {
    let action = active_skill_from_casting(casting)
        .map(healer_action_from_skill)
        .unwrap_or_else(|| healer_npc_decision(1.0, 0, 0.0, false, false, false).action);
    HealerNpcAiStateV1 {
        healer_id: entity_wire_id(healer),
        active_action: healer_action_label(action).to_string(),
        queue_len: profile.contracts.len().min(u32::MAX as usize) as u32,
        reputation: profile.reputation,
        retreating: action == HealerNpcAction::Retreat,
    }
}

fn peace_mastery_score(mastery: &HealingMastery) -> f32 {
    mastery
        .meridian_repair
        .max(mastery.contam_purge)
        .max(mastery.emergency_resuscitate)
        .max(mastery.life_extension)
        .max(mastery.mass_meridian_repair)
        .clamp(0.0, 100.0) as f32
}

fn active_skill_from_casting(casting: Option<&Casting>) -> Option<YidaoSkillIdV1> {
    match casting.and_then(|casting| casting.skill_id.as_deref()) {
        Some(MERIDIAN_REPAIR_SKILL_ID) => Some(YidaoSkillIdV1::MeridianRepair),
        Some(CONTAM_PURGE_SKILL_ID) => Some(YidaoSkillIdV1::ContamPurge),
        Some(EMERGENCY_RESUSCITATE_SKILL_ID) => Some(YidaoSkillIdV1::EmergencyResuscitate),
        Some(LIFE_EXTENSION_SKILL_ID) => Some(YidaoSkillIdV1::LifeExtension),
        Some(MASS_MERIDIAN_REPAIR_SKILL_ID) => Some(YidaoSkillIdV1::MassMeridianRepair),
        _ => None,
    }
}

fn healer_action_from_skill(skill: YidaoSkillIdV1) -> HealerNpcAction {
    match skill {
        YidaoSkillIdV1::MeridianRepair | YidaoSkillIdV1::MassMeridianRepair => {
            HealerNpcAction::MeridianRepair
        }
        YidaoSkillIdV1::ContamPurge => HealerNpcAction::ContamPurge,
        YidaoSkillIdV1::EmergencyResuscitate => HealerNpcAction::EmergencyResuscitate,
        YidaoSkillIdV1::LifeExtension => HealerNpcAction::LifeExtension,
    }
}

fn healer_action_label(action: HealerNpcAction) -> &'static str {
    match action {
        HealerNpcAction::EmergencyResuscitate => "emergency_resuscitate",
        HealerNpcAction::LifeExtension => "life_extension",
        HealerNpcAction::ContamPurge => "contam_purge",
        HealerNpcAction::MeridianRepair => "meridian_repair",
        HealerNpcAction::Retreat => "retreat",
        HealerNpcAction::Idle => "idle",
    }
}

fn send_yidao_payload(client: &mut Client, username: &Username, payload: ServerDataV1) {
    let payload_type = payload_type_label(payload.payload_type());
    let payload_bytes = match serialize_server_data_payload(&payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };
    send_server_data_payload(client, payload_bytes.as_slice());
    tracing::debug!(
        "[bong][network] sent {} {} payload for `{}`",
        SERVER_DATA_CHANNEL,
        payload_type,
        username.0,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::components::CastSource;
    use crate::combat::yidao::{MedicalContract, MedicalContractState};
    use valence::prelude::DVec3;

    #[test]
    fn hud_state_projects_healer_profile_and_casting() {
        let healer = Entity::from_raw(7);
        let profile = HealerProfile {
            reputation: 12,
            contracts: vec![MedicalContract {
                patient_id: "offline:Kiz".to_string(),
                state: MedicalContractState::Patient,
                treatment_count: 1,
                first_treatment_tick: 10,
                last_treatment_tick: 20,
            }],
        };
        let mastery = HealingMastery {
            meridian_repair: 48.0,
            contam_purge: 8.0,
            emergency_resuscitate: 4.0,
            life_extension: 1.0,
            mass_meridian_repair: 0.0,
        };
        let karma = KarmaCounter {
            yidao_karma: 3.5,
            tribulation_weight: 0.0,
        };
        let casting = Casting {
            source: CastSource::SkillBar,
            slot: 3,
            started_at_tick: 1,
            duration_ticks: 20,
            started_at_ms: 0,
            duration_ms: 1000,
            bound_instance_id: None,
            start_position: DVec3::ZERO,
            complete_cooldown_ticks: 40,
            skill_id: Some(MERIDIAN_REPAIR_SKILL_ID.to_string()),
            skill_config: None,
        };

        let state = build_yidao_hud_state(
            healer,
            Some(&profile),
            Some(&mastery),
            Some(&karma),
            Some(&casting),
        )
        .expect("hud state");

        assert_eq!(state.reputation, 12);
        assert_eq!(state.peace_mastery, 48.0);
        assert_eq!(state.karma, 3.5);
        assert_eq!(state.active_skill, Some(YidaoSkillIdV1::MeridianRepair));
        assert_eq!(state.patient_ids, vec!["offline:Kiz"]);
        assert_eq!(state.contract_count, 1);
    }

    #[test]
    fn healer_ai_state_uses_active_yidao_cast_label() {
        let healer = Entity::from_raw(9);
        let profile = HealerProfile {
            reputation: 5,
            contracts: Vec::new(),
        };
        let casting = Casting {
            source: CastSource::SkillBar,
            slot: 4,
            started_at_tick: 1,
            duration_ticks: 20,
            started_at_ms: 0,
            duration_ms: 1000,
            bound_instance_id: None,
            start_position: DVec3::ZERO,
            complete_cooldown_ticks: 40,
            skill_id: Some(LIFE_EXTENSION_SKILL_ID.to_string()),
            skill_config: None,
        };

        let state = build_healer_npc_ai_state(healer, &profile, Some(&casting));

        assert_eq!(state.reputation, 5);
        assert_eq!(state.active_action, "life_extension");
        assert!(!state.retreating);
    }
}
