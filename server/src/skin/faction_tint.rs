use valence::prelude::{
    bevy_ecs, Commands, Component, DVec3, Entity, Equipment, EventWriter, IntoSystemConfigs,
    ItemKind, ItemStack, Position, Query, Res, Update,
};

use crate::cultivation::components::{Cultivation, Realm};
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::npc::faction::{FactionId, FactionMembership, FactionRank};
use crate::npc::lifecycle::{NpcArchetype, NpcLifespan};
use crate::npc::movement::GameTick;
use crate::npc::spawn::NpcMarker;
use crate::schema::vfx_event::VfxEventPayloadV1;

use super::npc_skin_selector::{select_profile_from_components, NpcVisualProfile};

pub const ATTACK_FACTION_RGB: i32 = 0xCC2222;
pub const DEFEND_FACTION_RGB: i32 = 0x4466AA;
pub const ELDER_HAIR_RGB: i32 = 0xCCCCCC;

const RANK_AURA_INTERVAL_TICKS: u64 = 100;
const HIGH_REALM_AURA_INTERVAL_TICKS: u64 = 60;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Component)]
pub struct NpcVisualEquipment;

pub fn register(app: &mut valence::prelude::App) {
    app.add_systems(
        Update,
        (
            sync_npc_visual_profiles_system,
            emit_rank_aura_vfx_system.after(sync_npc_visual_profiles_system),
            emit_high_realm_aura_vfx_system.after(sync_npc_visual_profiles_system),
        ),
    );
}

pub fn visual_equipment(profile: &NpcVisualProfile) -> Equipment {
    let mut equipment = Equipment::default();
    apply_visual_equipment(&mut equipment, profile);
    equipment
}

pub fn apply_visual_equipment(equipment: &mut Equipment, profile: &NpcVisualProfile) {
    equipment.set_chest(faction_chest(profile.faction_id).unwrap_or(ItemStack::EMPTY));
    equipment.set_head(head_marker(profile).unwrap_or(ItemStack::EMPTY));
    equipment.set_main_hand(rank_hand_marker(profile).unwrap_or(ItemStack::EMPTY));
}

pub fn rank_aura_event_id(profile: &NpcVisualProfile) -> Option<&'static str> {
    if !matches!(profile.faction_rank, Some(FactionRank::Leader)) {
        return None;
    }
    if profile.has_high_realm_aura() {
        Some("bong:npc_rank_aura_master")
    } else {
        Some("bong:npc_rank_aura_elder")
    }
}

pub fn high_realm_aura_event_id(profile: &NpcVisualProfile) -> Option<&'static str> {
    profile
        .has_high_realm_aura()
        .then_some("bong:npc_qi_aura_ripple")
}

#[allow(clippy::type_complexity)]
fn sync_npc_visual_profiles_system(
    mut commands: Commands,
    mut npcs: Query<
        (
            Entity,
            &NpcArchetype,
            Option<&Cultivation>,
            Option<&FactionMembership>,
            Option<&NpcLifespan>,
            Option<&mut NpcVisualProfile>,
            Option<&mut Equipment>,
        ),
        bevy_ecs::query::With<NpcMarker>,
    >,
) {
    for (entity, archetype, cultivation, faction, lifespan, profile, equipment) in &mut npcs {
        let desired = select_profile_from_components(
            *archetype,
            cultivation
                .map(|cultivation| cultivation.realm)
                .unwrap_or(Realm::Awaken),
            faction,
            lifespan,
        );

        let mut changed = false;
        match profile {
            Some(mut current) => {
                if *current != desired {
                    *current = desired;
                    changed = true;
                }
            }
            None => {
                commands.entity(entity).insert(desired);
                changed = true;
            }
        }

        match equipment {
            Some(mut equipment) => {
                if changed {
                    apply_visual_equipment(&mut equipment, &desired);
                }
            }
            None => {
                commands
                    .entity(entity)
                    .insert((visual_equipment(&desired), NpcVisualEquipment));
            }
        }
    }
}

fn emit_rank_aura_vfx_system(
    game_tick: Option<Res<GameTick>>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    npcs: Query<(&NpcVisualProfile, &Position), bevy_ecs::query::With<NpcMarker>>,
) {
    let Some(tick) = game_tick.map(|tick| u64::from(tick.0)) else {
        return;
    };
    if tick % RANK_AURA_INTERVAL_TICKS != 0 {
        return;
    }

    for (profile, position) in &npcs {
        let Some(event_id) = rank_aura_event_id(profile) else {
            continue;
        };
        send_visual_particle(
            &mut vfx_events,
            event_id,
            position.get(),
            Some("#F2D16B"),
            0.55,
            8,
            40,
        );
    }
}

fn emit_high_realm_aura_vfx_system(
    game_tick: Option<Res<GameTick>>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    npcs: Query<(&NpcVisualProfile, &Position), bevy_ecs::query::With<NpcMarker>>,
) {
    let Some(tick) = game_tick.map(|tick| u64::from(tick.0)) else {
        return;
    };
    if tick % HIGH_REALM_AURA_INTERVAL_TICKS != 0 {
        return;
    }

    for (profile, position) in &npcs {
        let Some(event_id) = high_realm_aura_event_id(profile) else {
            continue;
        };
        send_visual_particle(
            &mut vfx_events,
            event_id,
            position.get(),
            Some("#8FE6B8"),
            0.35,
            4,
            35,
        );
    }
}

pub fn npc_death_smoke_request(origin: DVec3) -> VfxEventRequest {
    VfxEventRequest::new(
        origin,
        VfxEventPayloadV1::SpawnParticle {
            event_id: "bong:npc_death_smoke".to_string(),
            origin: [origin.x, origin.y, origin.z],
            direction: None,
            color: Some("#B8B8B8".to_string()),
            strength: Some(0.45),
            count: Some(16),
            duration_ticks: Some(60),
        },
    )
}

pub fn npc_death_qi_burst_request(
    origin: DVec3,
    profile: Option<&NpcVisualProfile>,
) -> Option<VfxEventRequest> {
    profile?.has_high_realm_aura().then(|| {
        VfxEventRequest::new(
            origin,
            VfxEventPayloadV1::SpawnParticle {
                event_id: "bong:npc_death_qi_burst".to_string(),
                origin: [origin.x, origin.y + 0.8, origin.z],
                direction: None,
                color: Some("#8FE6B8".to_string()),
                strength: Some(0.75),
                count: Some(8),
                duration_ticks: Some(12),
            },
        )
    })
}

fn send_visual_particle(
    vfx_events: &mut EventWriter<VfxEventRequest>,
    event_id: &str,
    origin: DVec3,
    color: Option<&str>,
    strength: f32,
    count: u16,
    duration_ticks: u16,
) {
    vfx_events.send(VfxEventRequest::new(
        origin,
        VfxEventPayloadV1::SpawnParticle {
            event_id: event_id.to_string(),
            origin: [origin.x, origin.y, origin.z],
            direction: None,
            color: color.map(ToString::to_string),
            strength: Some(strength),
            count: Some(count),
            duration_ticks: Some(duration_ticks),
        },
    ));
}

fn faction_chest(faction_id: Option<FactionId>) -> Option<ItemStack> {
    match faction_id {
        Some(FactionId::Attack) => Some(dyed_leather(
            ItemKind::LeatherChestplate,
            ATTACK_FACTION_RGB,
        )),
        Some(FactionId::Defend) => Some(dyed_leather(
            ItemKind::LeatherChestplate,
            DEFEND_FACTION_RGB,
        )),
        Some(FactionId::Neutral) | None => None,
    }
}

fn head_marker(profile: &NpcVisualProfile) -> Option<ItemStack> {
    if matches!(profile.faction_rank, Some(FactionRank::Leader)) {
        return Some(ItemStack::new(ItemKind::DiamondHelmet, 1, None));
    }
    if profile.age_band.is_elderly() {
        return Some(dyed_leather(ItemKind::LeatherHelmet, ELDER_HAIR_RGB));
    }
    None
}

fn rank_hand_marker(profile: &NpcVisualProfile) -> Option<ItemStack> {
    match profile.faction_rank {
        Some(FactionRank::Ally) => Some(ItemStack::new(ItemKind::Emerald, 1, None)),
        _ => None,
    }
}

fn dyed_leather(item: ItemKind, rgb: i32) -> ItemStack {
    let mut display = valence::nbt::Compound::new();
    display.insert("color", rgb);
    let mut root = valence::nbt::Compound::new();
    root.insert("display", display);
    ItemStack::new(item, 1, Some(root))
}

#[cfg(test)]
mod tests {
    use super::super::npc_skin_selector::{select_npc_visual_profile, NpcAgeBand};
    use super::*;
    use valence::nbt::Value;

    fn profile(
        faction_id: Option<FactionId>,
        faction_rank: Option<FactionRank>,
        age_band: NpcAgeBand,
        realm: Realm,
    ) -> NpcVisualProfile {
        select_npc_visual_profile(
            NpcArchetype::Disciple,
            realm,
            faction_id,
            faction_rank,
            age_ratio_for_band(age_band),
        )
    }

    fn age_ratio_for_band(age_band: NpcAgeBand) -> f64 {
        match age_band {
            NpcAgeBand::Young => 0.2,
            NpcAgeBand::Adult => 0.5,
            NpcAgeBand::Elder => 0.75,
            NpcAgeBand::Fading => 0.95,
        }
    }

    fn leather_color(stack: &ItemStack) -> Option<i32> {
        let display = match stack.nbt.as_ref()?.get("display")? {
            Value::Compound(display) => display,
            _ => return None,
        };
        match display.get("color")? {
            Value::Int(color) => Some(*color),
            _ => None,
        }
    }

    #[test]
    fn faction_tint_applies() {
        let attack = profile(
            Some(FactionId::Attack),
            Some(FactionRank::Disciple),
            NpcAgeBand::Adult,
            Realm::Awaken,
        );
        let defend = profile(
            Some(FactionId::Defend),
            Some(FactionRank::Disciple),
            NpcAgeBand::Adult,
            Realm::Awaken,
        );
        let neutral = profile(None, None, NpcAgeBand::Adult, Realm::Awaken);

        let attack_equipment = visual_equipment(&attack);
        let defend_equipment = visual_equipment(&defend);
        let neutral_equipment = visual_equipment(&neutral);

        assert_eq!(attack_equipment.chest().item, ItemKind::LeatherChestplate);
        assert_eq!(
            leather_color(attack_equipment.chest()),
            Some(ATTACK_FACTION_RGB)
        );
        assert_eq!(defend_equipment.chest().item, ItemKind::LeatherChestplate);
        assert_eq!(
            leather_color(defend_equipment.chest()),
            Some(DEFEND_FACTION_RGB)
        );
        assert!(neutral_equipment.chest().is_empty());
    }

    #[test]
    fn age_and_rank_markers_do_not_conflict() {
        let elder = profile(None, None, NpcAgeBand::Elder, Realm::Condense);
        let leader = profile(
            Some(FactionId::Attack),
            Some(FactionRank::Leader),
            NpcAgeBand::Fading,
            Realm::Spirit,
        );

        let elder_equipment = visual_equipment(&elder);
        let leader_equipment = visual_equipment(&leader);

        assert_eq!(elder_equipment.head().item, ItemKind::LeatherHelmet);
        assert_eq!(leather_color(elder_equipment.head()), Some(ELDER_HAIR_RGB));
        assert_eq!(leader_equipment.head().item, ItemKind::DiamondHelmet);
        assert_eq!(
            rank_aura_event_id(&leader),
            Some("bong:npc_rank_aura_master")
        );
    }

    #[test]
    fn high_realm_death_burst_only_emits_for_high_realm_profile() {
        let high = profile(None, None, NpcAgeBand::Adult, Realm::Spirit);
        let low = profile(None, None, NpcAgeBand::Adult, Realm::Induce);

        let request = npc_death_qi_burst_request(DVec3::new(1.0, 2.0, 3.0), Some(&high))
            .expect("high realm NPC death should emit qi burst");
        assert!(matches!(
            request.payload,
            VfxEventPayloadV1::SpawnParticle { ref event_id, .. } if event_id == "bong:npc_death_qi_burst"
        ));
        assert!(npc_death_qi_burst_request(DVec3::ZERO, Some(&low)).is_none());
        assert!(npc_death_qi_burst_request(DVec3::ZERO, None).is_none());
    }
}
