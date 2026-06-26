//! Rust → Proto 变换层。
//!
//! `impl From<&ServerDataPayloadV1> for bong::server_data_envelope::Payload`
//! 把运行时内部 enum 映射到 prost 生成的 proto oneof。
//!
//! 设计原则：
//! - 只做值拷贝 / 类型转换，不做业务逻辑
//! - 所有 Rust enum → proto i32 用显式 match 而非 `as i32` 以避免 ABI 漂移
//! - 复合嵌套（InventorySnapshot 等）做 helper 函数，保持 match arm 行数可控

use super::proto_gen::bong;
use super::server_data::ServerDataPayloadV1;

// ═══════════════════════════════════════════════════════════════
// helper: Rust enum → proto i32
// ═══════════════════════════════════════════════════════════════

fn color_kind_to_proto(c: &crate::cultivation::components::ColorKind) -> i32 {
    use crate::cultivation::components::ColorKind;
    match c {
        ColorKind::Sharp => bong::ColorKind::Sharp as i32,
        ColorKind::Heavy => bong::ColorKind::Heavy as i32,
        ColorKind::Mellow => bong::ColorKind::Mellow as i32,
        ColorKind::Solid => bong::ColorKind::Solid as i32,
        ColorKind::Light => bong::ColorKind::Light as i32,
        ColorKind::Intricate => bong::ColorKind::Intricate as i32,
        ColorKind::Gentle => bong::ColorKind::Gentle as i32,
        ColorKind::Insidious => bong::ColorKind::Insidious as i32,
        ColorKind::Violent => bong::ColorKind::Violent as i32,
        ColorKind::Turbid => bong::ColorKind::Turbid as i32,
    }
}

fn realm_str_to_proto(s: &str) -> i32 {
    match s {
        "Awaken" => bong::Realm::Awaken as i32,
        "Induce" => bong::Realm::Induce as i32,
        "Condense" => bong::Realm::Condense as i32,
        "Solidify" => bong::Realm::Solidify as i32,
        "Spirit" => bong::Realm::Spirit as i32,
        "Void" => bong::Realm::Void as i32,
        _ => bong::Realm::Unspecified as i32,
    }
}

fn realm_enum_to_proto(r: &crate::cultivation::components::Realm) -> i32 {
    use crate::cultivation::components::Realm;
    match r {
        Realm::Awaken => bong::Realm::Awaken as i32,
        Realm::Induce => bong::Realm::Induce as i32,
        Realm::Condense => bong::Realm::Condense as i32,
        Realm::Solidify => bong::Realm::Solidify as i32,
        Realm::Spirit => bong::Realm::Spirit as i32,
        Realm::Void => bong::Realm::Void as i32,
    }
}

fn item_rarity_to_proto_str(r: &super::inventory::ItemRarityV1) -> String {
    use super::inventory::ItemRarityV1;
    match r {
        ItemRarityV1::Common => "common".to_string(),
        ItemRarityV1::Uncommon => "uncommon".to_string(),
        ItemRarityV1::Rare => "rare".to_string(),
        ItemRarityV1::Epic => "epic".to_string(),
        ItemRarityV1::Legendary => "legendary".to_string(),
        ItemRarityV1::Ancient => "ancient".to_string(),
    }
}

fn event_kind_to_proto(e: &super::common::EventKind) -> i32 {
    use super::common::EventKind;
    match e {
        EventKind::ThunderTribulation => bong::EventKind::ThunderTribulation as i32,
        EventKind::BeastTide => bong::EventKind::BeastTide as i32,
        EventKind::RealmCollapse => bong::EventKind::RealmCollapse as i32,
        EventKind::KarmaBacklash => bong::EventKind::KarmaBacklash as i32,
        EventKind::PoisonMiasma => bong::EventKind::PoisonMiasma as i32,
        EventKind::MeridianSeal => bong::EventKind::MeridianSeal as i32,
        EventKind::DaoxiangWave => bong::EventKind::DaoxiangWave as i32,
        EventKind::HeavenlyFire => bong::EventKind::HeavenlyFire as i32,
        EventKind::PressureInvert => bong::EventKind::PressureInvert as i32,
        EventKind::AllWither => bong::EventKind::AllWither as i32,
        // plan-exploration-probe-return-v1 P1 — 通用提示。
        EventKind::Generic => bong::EventKind::Generic as i32,
    }
}

fn zone_status_to_string(s: &super::world_state::ZoneStatusV1) -> String {
    use super::world_state::ZoneStatusV1;
    match s {
        ZoneStatusV1::Normal => "Normal".into(),
        ZoneStatusV1::RaceOut => "RaceOut".into(),
        ZoneStatusV1::Collapsed => "Collapsed".into(),
    }
}

fn season_to_proto(s: &super::world_state::SeasonV1) -> i32 {
    use super::world_state::SeasonV1;
    match s {
        SeasonV1::Summer => bong::Season::Summer as i32,
        SeasonV1::SummerToWinter => bong::Season::SummerToWinter as i32,
        SeasonV1::Winter => bong::Season::Winter as i32,
        SeasonV1::WinterToSummer => bong::Season::WinterToSummer as i32,
    }
}

fn relationship_kind_to_proto(k: &super::social::RelationshipKindV1) -> i32 {
    use super::social::RelationshipKindV1;
    match k {
        RelationshipKindV1::Master => bong::RelationshipKind::Master as i32,
        RelationshipKindV1::Disciple => bong::RelationshipKind::Disciple as i32,
        RelationshipKindV1::Companion => bong::RelationshipKind::Companion as i32,
        RelationshipKindV1::Pact => bong::RelationshipKind::Pact as i32,
        RelationshipKindV1::Feud => bong::RelationshipKind::Feud as i32,
    }
}

fn meridian_idx_to_proto(idx: u8) -> i32 {
    // idx 0..19 直接映射到 MeridianId 1..20（+1 偏移因 0 = UNSPECIFIED）
    (idx as i32) + 1
}

fn skill_id_to_proto(s: &super::skill::SkillIdV1) -> i32 {
    use super::skill::SkillIdV1;
    match s {
        SkillIdV1::Herbalism => bong::SkillId::Herbalism as i32,
        SkillIdV1::Alchemy => bong::SkillId::Alchemy as i32,
        SkillIdV1::Forging => bong::SkillId::Forging as i32,
        SkillIdV1::Combat => bong::SkillId::Combat as i32,
        SkillIdV1::Mineral => bong::SkillId::Mineral as i32,
        SkillIdV1::Cultivation => bong::SkillId::Cultivation as i32,
    }
}

fn exposure_kind_to_proto(k: &super::social::ExposureKindV1) -> i32 {
    use super::social::ExposureKindV1;
    match k {
        ExposureKindV1::Chat => bong::ExposureKind::Chat as i32,
        ExposureKindV1::Trade => bong::ExposureKind::Trade as i32,
        ExposureKindV1::Divine => bong::ExposureKind::Divine as i32,
        ExposureKindV1::Death => bong::ExposureKind::Death as i32,
    }
}

fn guardian_kind_to_proto(k: &super::social::GuardianKindV1) -> i32 {
    use super::social::GuardianKindV1;
    match k {
        GuardianKindV1::Puppet => bong::GuardianKind::Puppet as i32,
        GuardianKindV1::ZhenfaTrap => bong::GuardianKind::ZhenfaTrap as i32,
        GuardianKindV1::BondedDaoxiang => bong::GuardianKind::BondedDaoxiang as i32,
    }
}

fn gathering_target_type_to_proto(t: &super::server_data::GatheringTargetTypeV1) -> i32 {
    use super::server_data::GatheringTargetTypeV1;
    match t {
        GatheringTargetTypeV1::Herb => bong::GatheringTargetType::Herb as i32,
        GatheringTargetTypeV1::Ore => bong::GatheringTargetType::Ore as i32,
        GatheringTargetTypeV1::Wood => bong::GatheringTargetType::Wood as i32,
    }
}

fn gathering_quality_hint_to_proto(q: &super::server_data::GatheringQualityHintV1) -> i32 {
    use super::server_data::GatheringQualityHintV1;
    match q {
        GatheringQualityHintV1::Normal => bong::GatheringQualityHint::Normal as i32,
        GatheringQualityHintV1::FineLikely => bong::GatheringQualityHint::FineLikely as i32,
        GatheringQualityHintV1::PerfectPossible => {
            bong::GatheringQualityHint::PerfectPossible as i32
        }
        GatheringQualityHintV1::Fine => bong::GatheringQualityHint::Fine as i32,
        GatheringQualityHintV1::Perfect => bong::GatheringQualityHint::Perfect as i32,
    }
}

fn death_screen_stage_to_proto(s: &super::server_data::DeathScreenStageV1) -> i32 {
    use super::server_data::DeathScreenStageV1;
    match s {
        DeathScreenStageV1::Fortune => bong::DeathScreenStage::Fortune as i32,
        DeathScreenStageV1::Tribulation => bong::DeathScreenStage::Tribulation as i32,
    }
}

fn death_screen_zone_kind_to_proto(k: &super::server_data::DeathScreenZoneKindV1) -> i32 {
    use super::server_data::DeathScreenZoneKindV1;
    match k {
        DeathScreenZoneKindV1::Ordinary => bong::DeathScreenZoneKind::Ordinary as i32,
        DeathScreenZoneKindV1::Death => bong::DeathScreenZoneKind::Death as i32,
        DeathScreenZoneKindV1::Negative => bong::DeathScreenZoneKind::Negative as i32,
    }
}

fn rift_portal_kind_to_proto(k: &super::server_data::RiftPortalKindV1) -> i32 {
    use super::server_data::RiftPortalKindV1;
    match k {
        RiftPortalKindV1::MainRift => bong::RiftPortalKind::MainRift as i32,
        RiftPortalKindV1::DeepRift => bong::RiftPortalKind::DeepRift as i32,
        RiftPortalKindV1::CollapseTear => bong::RiftPortalKind::CollapseTear as i32,
    }
}

fn rift_portal_direction_to_proto(d: &super::server_data::RiftPortalDirectionV1) -> i32 {
    use super::server_data::RiftPortalDirectionV1;
    match d {
        RiftPortalDirectionV1::Entry => bong::RiftPortalDirection::Entry as i32,
        RiftPortalDirectionV1::Exit => bong::RiftPortalDirection::Exit as i32,
    }
}

fn extract_aborted_reason_to_proto(r: &super::server_data::ExtractAbortedReasonV1) -> i32 {
    use super::server_data::ExtractAbortedReasonV1;
    match r {
        ExtractAbortedReasonV1::Moved => bong::ExtractAbortedReason::Moved as i32,
        ExtractAbortedReasonV1::Combat => bong::ExtractAbortedReason::Combat as i32,
        ExtractAbortedReasonV1::Damaged => bong::ExtractAbortedReason::Damaged as i32,
        ExtractAbortedReasonV1::Cancelled => bong::ExtractAbortedReason::Cancelled as i32,
        ExtractAbortedReasonV1::PortalExpired => bong::ExtractAbortedReason::PortalExpired as i32,
        ExtractAbortedReasonV1::OutOfRange => bong::ExtractAbortedReason::OutOfRange as i32,
        ExtractAbortedReasonV1::NotInTsy => bong::ExtractAbortedReason::NotInTsy as i32,
        ExtractAbortedReasonV1::AlreadyBusy => bong::ExtractAbortedReason::AlreadyBusy as i32,
        ExtractAbortedReasonV1::PortalOccupied => bong::ExtractAbortedReason::PortalOccupied as i32,
        ExtractAbortedReasonV1::CannotExit => bong::ExtractAbortedReason::CannotExit as i32,
    }
}

fn extract_failed_reason_to_proto(r: &super::server_data::ExtractFailedReasonV1) -> i32 {
    use super::server_data::ExtractFailedReasonV1;
    match r {
        ExtractFailedReasonV1::SpiritQiDrained => bong::ExtractFailedReason::SpiritQiDrained as i32,
    }
}

fn container_kind_to_proto(k: &super::server_data::ContainerKindV1) -> i32 {
    use super::server_data::ContainerKindV1;
    match k {
        ContainerKindV1::DryCorpse => bong::ContainerKind::DryCorpse as i32,
        ContainerKindV1::Skeleton => bong::ContainerKind::Skeleton as i32,
        ContainerKindV1::StoragePouch => bong::ContainerKind::StoragePouch as i32,
        ContainerKindV1::StoneCasket => bong::ContainerKind::StoneCasket as i32,
        ContainerKindV1::RelicCore => bong::ContainerKind::RelicCore as i32,
        ContainerKindV1::SurfaceStash => bong::ContainerKind::SurfaceStash as i32,
    }
}

fn key_kind_to_proto(k: &super::server_data::KeyKindV1) -> i32 {
    use super::server_data::KeyKindV1;
    match k {
        KeyKindV1::StoneCasketKey => bong::KeyKind::StoneCasketKey as i32,
        KeyKindV1::JadeCoffinSeal => bong::KeyKind::JadeCoffinSeal as i32,
        KeyKindV1::ArrayCoreSigil => bong::KeyKind::ArrayCoreSigil as i32,
    }
}

fn search_abort_reason_to_proto(r: &super::server_data::SearchAbortReasonV1) -> i32 {
    use super::server_data::SearchAbortReasonV1;
    match r {
        SearchAbortReasonV1::Moved => bong::SearchAbortReason::Moved as i32,
        SearchAbortReasonV1::Combat => bong::SearchAbortReason::Combat as i32,
        SearchAbortReasonV1::Damaged => bong::SearchAbortReason::Damaged as i32,
        SearchAbortReasonV1::Cancelled => bong::SearchAbortReason::Cancelled as i32,
    }
}

// ═══════════════════════════════════════════════════════════════
// helper: 复合嵌套类型转换
// ═══════════════════════════════════════════════════════════════

fn lifespan_to_proto(l: &super::server_data::LifespanPreviewV1) -> bong::LifespanPreview {
    bong::LifespanPreview {
        years_lived: l.years_lived,
        cap_by_realm: l.cap_by_realm,
        remaining_years: l.remaining_years,
        death_penalty_years: l.death_penalty_years,
        tick_rate_multiplier: l.tick_rate_multiplier,
        is_wind_candle: l.is_wind_candle,
    }
}

fn breakdown_to_proto(b: &super::world_state::PlayerPowerBreakdown) -> bong::PlayerPowerBreakdown {
    bong::PlayerPowerBreakdown {
        combat: b.combat,
        wealth: b.wealth,
        social: b.social,
        karma: b.karma,
        territory: b.territory,
    }
}

fn season_state_to_proto(s: &super::world_state::SeasonStateV1) -> bong::SeasonState {
    bong::SeasonState {
        season: season_to_proto(&s.season),
        tick_into_phase: s.tick_into_phase,
        phase_total_ticks: s.phase_total_ticks,
        year_index: s.year_index,
    }
}

fn renown_tag_to_proto(t: &super::social::RenownTagV1) -> bong::RenownTag {
    bong::RenownTag {
        tag: t.tag.clone(),
        weight: t.weight,
        last_seen_tick: t.last_seen_tick,
        permanent: t.permanent,
    }
}

fn renown_snapshot_to_proto(r: &super::social::RenownSnapshotV1) -> bong::RenownSnapshot {
    bong::RenownSnapshot {
        fame: r.fame,
        notoriety: r.notoriety,
        top_tags: r.top_tags.iter().map(renown_tag_to_proto).collect(),
    }
}

fn relationship_to_proto(r: &super::social::RelationshipSnapshotV1) -> bong::RelationshipSnapshot {
    bong::RelationshipSnapshot {
        kind: relationship_kind_to_proto(&r.kind),
        peer: r.peer.clone(),
        since_tick: r.since_tick,
        metadata: r.metadata.to_string(),
    }
}

fn faction_to_proto(
    f: &super::social::FactionMembershipSnapshotV1,
) -> bong::FactionMembershipSnapshot {
    bong::FactionMembershipSnapshot {
        faction: f.faction.clone(),
        rank: f.rank as u32,
        loyalty: f.loyalty,
        betrayal_count: f.betrayal_count as u32,
        invite_block_until_tick: f.invite_block_until_tick,
        permanently_refused: f.permanently_refused,
    }
}

fn social_snapshot_to_proto(
    s: &super::social::PlayerSocialSnapshotV1,
) -> bong::PlayerSocialSnapshot {
    bong::PlayerSocialSnapshot {
        renown: Some(renown_snapshot_to_proto(&s.renown)),
        relationships: s.relationships.iter().map(relationship_to_proto).collect(),
        exposed_to_count: s.exposed_to_count,
        faction_membership: s.faction_membership.as_ref().map(faction_to_proto),
    }
}

fn inventory_item_view_to_proto(
    v: &super::inventory::InventoryItemViewV1,
) -> bong::InventoryItemView {
    bong::InventoryItemView {
        instance_id: v.instance_id,
        item_id: v.item_id.clone(),
        display_name: v.display_name.clone(),
        grid_width: v.grid_width as u32,
        grid_height: v.grid_height as u32,
        weight: v.weight,
        rarity: item_rarity_to_proto_str(&v.rarity),
        description: v.description.clone(),
        stack_count: v.stack_count,
        spirit_quality: v.spirit_quality,
        durability: v.durability,
        mineral_id: v.mineral_id.clone(),
        scroll_kind: v.scroll_kind.clone(),
        scroll_skill_id: v.scroll_skill_id.clone(),
        scroll_xp_grant: v.scroll_xp_grant,
        charges: v.charges,
        forge_quality: v.forge_quality,
        forge_color: v.forge_color.as_ref().map(color_kind_to_proto),
        forge_side_effects: v.forge_side_effects.clone(),
        forge_achieved_tier: v.forge_achieved_tier.map(|t| t as u32),
    }
}

fn inventory_location_to_proto(
    loc: &super::inventory::InventoryLocationV1,
) -> bong::InventoryLocation {
    use super::inventory::InventoryLocationV1;
    match loc {
        InventoryLocationV1::Container {
            container_id,
            row,
            col,
        } => bong::InventoryLocation {
            location: Some(bong::inventory_location::Location::Container(
                bong::InventoryLocationContainer {
                    container_id: container_id.clone(),
                    row: *row,
                    col: *col,
                },
            )),
        },
        InventoryLocationV1::Equip { slot, state } => bong::InventoryLocation {
            location: Some(bong::inventory_location::Location::Equip(
                bong::InventoryLocationEquip {
                    slot: equip_slot_to_proto(slot),
                    state: equip_state_to_proto(state),
                },
            )),
        },
        InventoryLocationV1::Hotbar { index } => bong::InventoryLocation {
            location: Some(bong::inventory_location::Location::Hotbar(
                bong::InventoryLocationHotbar {
                    index: *index as u32,
                },
            )),
        },
    }
}

fn equip_slot_to_proto(s: &super::inventory::EquipSlotV1) -> i32 {
    use super::inventory::EquipSlotV1;
    match s {
        EquipSlotV1::Head => bong::EquipSlot::Head as i32,
        EquipSlotV1::Chest => bong::EquipSlot::Chest as i32,
        EquipSlotV1::Legs => bong::EquipSlot::Legs as i32,
        EquipSlotV1::Feet => bong::EquipSlot::Feet as i32,
        EquipSlotV1::MainHand => bong::EquipSlot::MainHand as i32,
        EquipSlotV1::OffHand => bong::EquipSlot::OffHand as i32,
        EquipSlotV1::ExtraHand0 => bong::EquipSlot::ExtraHand0 as i32,
        EquipSlotV1::ExtraHand1 => bong::EquipSlot::ExtraHand1 as i32,
    }
}

fn equip_state_to_proto(s: &super::inventory::EquipStateV1) -> i32 {
    use super::inventory::EquipStateV1;
    match s {
        EquipStateV1::Worn => bong::EquipState::Worn as i32,
        EquipStateV1::Held => bong::EquipState::Held as i32,
    }
}

// ═══════════════════════════════════════════════════════════════
// 主体 From impl
// ═══════════════════════════════════════════════════════════════

type Payload = bong::server_data_envelope::Payload;

/// CoffinGradeV1 → wire string（与 TS schema 和 DB 存储一致，全小写 snake_case）。
fn coffin_grade_v1_to_proto_str(g: super::server_data::CoffinGradeV1) -> &'static str {
    match g {
        super::server_data::CoffinGradeV1::Mundane => "mundane",
        super::server_data::CoffinGradeV1::Jade => "jade",
        super::server_data::CoffinGradeV1::Stone => "stone",
        super::server_data::CoffinGradeV1::Bronze => "bronze",
    }
}

/// wire string → CoffinGradeV1（未知值 fallback Mundane，与 serde default 一致）。
fn coffin_grade_v1_from_proto_str(s: &str) -> super::server_data::CoffinGradeV1 {
    match s {
        "jade" => super::server_data::CoffinGradeV1::Jade,
        "stone" => super::server_data::CoffinGradeV1::Stone,
        "bronze" => super::server_data::CoffinGradeV1::Bronze,
        _ => super::server_data::CoffinGradeV1::Mundane,
    }
}

/// Public entry point: convert a `ServerDataPayloadV1` to the prost oneof payload.
pub fn server_data_to_proto_payload(p: &ServerDataPayloadV1) -> Payload {
    Payload::from(p)
}

impl From<&ServerDataPayloadV1> for Payload {
    fn from(v: &ServerDataPayloadV1) -> Self {
        match v {
            ServerDataPayloadV1::Welcome { message } => Payload::Welcome(bong::Welcome {
                message: message.clone(),
            }),
            ServerDataPayloadV1::Heartbeat { message } => Payload::Heartbeat(bong::Heartbeat {
                message: message.clone(),
            }),
            ServerDataPayloadV1::Narration { narrations } => {
                Payload::Narration(bong::NarrationBatch {
                    narrations: narrations
                        .iter()
                        .map(|n| bong::NarrationEntry {
                            text: n.text.clone(),
                            scope: format!("{:?}", n.scope).to_ascii_lowercase(),
                            style: serde_json::to_value(&n.style)
                                .ok()
                                .and_then(|v| v.as_str().map(String::from))
                                .unwrap_or_default(),
                            target: n.target.clone(),
                            kind: n.kind.as_ref().map(|k| {
                                serde_json::to_value(k)
                                    .ok()
                                    .and_then(|v| v.as_str().map(String::from))
                                    .unwrap_or_default()
                            }),
                        })
                        .collect(),
                })
            }
            ServerDataPayloadV1::ZoneInfo {
                zone,
                spirit_qi,
                danger_level,
                status,
                active_events,
                perception_text,
            } => Payload::ZoneInfo(bong::ZoneInfo {
                zone: zone.clone(),
                spirit_qi: *spirit_qi,
                danger_level: *danger_level as u32,
                status: zone_status_to_string(status),
                active_events: active_events.clone().unwrap_or_default(),
                perception_text: perception_text.clone(),
            }),
            ServerDataPayloadV1::EventAlert {
                event,
                message,
                zone,
                duration_ticks,
            } => Payload::EventAlert(bong::EventAlert {
                event: event_kind_to_proto(event),
                message: message.clone(),
                zone: zone.clone(),
                duration_ticks: *duration_ticks,
            }),
            ServerDataPayloadV1::PlayerState {
                player,
                realm,
                spirit_qi,
                spirit_qi_max,
                karma,
                composite_power,
                breakdown,
                zone,
                local_neg_pressure,
                season_state,
                social,
            } => Payload::PlayerState(bong::PlayerState {
                player: player.clone(),
                realm: realm_str_to_proto(realm),
                spirit_qi: *spirit_qi,
                spirit_qi_max: *spirit_qi_max,
                karma: *karma,
                composite_power: *composite_power,
                zone: zone.clone(),
                local_neg_pressure: *local_neg_pressure,
                breakdown: Some(breakdown_to_proto(breakdown)),
                season_state: season_state.as_ref().map(season_state_to_proto),
                social: social.as_ref().map(social_snapshot_to_proto),
            }),
            ServerDataPayloadV1::CoffinState(s) => Payload::CoffinState(bong::CoffinState {
                in_coffin: s.in_coffin,
                lifespan_rate_multiplier: s.lifespan_rate_multiplier,
                // coffin_grade: optional string；None → 省略（接收方按 mundane 兜底）
                coffin_grade: s
                    .coffin_grade
                    .map(|g| coffin_grade_v1_to_proto_str(g).to_string()),
            }),
            ServerDataPayloadV1::UiOpen { ui, xml } => Payload::UiOpen(bong::UiOpen {
                ui: ui.clone(),
                xml: xml.clone(),
            }),
            ServerDataPayloadV1::CultivationDetail {
                realm,
                opened,
                flow_rate,
                flow_capacity,
                integrity,
                open_progress,
                cracks_count,
                contamination_total,
                lifespan,
                recent_skill_milestones_summary,
                skill_milestones,
                qi_color_main,
                qi_color_secondary,
                qi_color_chaotic,
                qi_color_hunyuan,
                practice_weights,
                target_meridian,
            } => {
                // SoA → AoS: 并行数组打包成 repeated MeridianState
                let meridians: Vec<bong::MeridianState> = (0..20)
                    .map(|i| bong::MeridianState {
                        id: meridian_idx_to_proto(i as u8),
                        opened: opened.get(i).copied().unwrap_or(false),
                        flow_rate: flow_rate.get(i).copied().unwrap_or(0.0),
                        flow_capacity: flow_capacity.get(i).copied().unwrap_or(0.0),
                        integrity: integrity.get(i).copied().unwrap_or(1.0),
                        open_progress: open_progress.get(i).copied().unwrap_or(0.0),
                        cracks_count: cracks_count.get(i).copied().unwrap_or(0) as u32,
                    })
                    .collect();
                Payload::CultivationDetail(bong::CultivationDetail {
                    realm: realm_str_to_proto(realm),
                    meridians,
                    target_meridian: target_meridian.map(meridian_idx_to_proto),
                    contamination_total: *contamination_total,
                    lifespan: lifespan.as_ref().map(lifespan_to_proto),
                    recent_skill_milestones_summary: recent_skill_milestones_summary.clone(),
                    skill_milestones: skill_milestones
                        .iter()
                        .map(|m| bong::SkillMilestoneSnapshot {
                            skill: m.skill.clone(),
                            new_lv: m.new_lv as u32,
                            achieved_at: m.achieved_at,
                            narration: m.narration.clone(),
                            total_xp_at: m.total_xp_at,
                        })
                        .collect(),
                    qi_color_main: color_kind_to_proto(qi_color_main),
                    qi_color_secondary: qi_color_secondary.as_ref().map(color_kind_to_proto),
                    qi_color_chaotic: *qi_color_chaotic,
                    qi_color_hunyuan: *qi_color_hunyuan,
                    practice_weights: practice_weights
                        .iter()
                        .map(|pw| bong::PracticeWeight {
                            color: color_kind_to_proto(&pw.color),
                            weight: pw.weight,
                            ratio: pw.ratio,
                        })
                        .collect(),
                })
            }
            ServerDataPayloadV1::QiColorObserved(o) => {
                Payload::QiColorObserved(bong::QiColorObserved {
                    observer: o.observer.clone(),
                    observed: o.observed.clone(),
                    main: color_kind_to_proto(&o.main),
                    secondary: o.secondary.as_ref().map(color_kind_to_proto),
                    is_chaotic: o.is_chaotic,
                    is_hunyuan: o.is_hunyuan,
                    realm_diff: o.realm_diff,
                })
            }
            ServerDataPayloadV1::InventorySnapshot(snap) => {
                Payload::InventorySnapshot(inventory_snapshot_to_proto(snap))
            }
            ServerDataPayloadV1::InventoryEvent(evt) => {
                Payload::InventoryEvent(inventory_event_to_proto(evt))
            }
            ServerDataPayloadV1::DroppedLootSync(drops) => {
                Payload::DroppedLootSync(bong::DroppedLootSync {
                    drops: drops
                        .iter()
                        .map(|d| bong::DroppedLootEntry {
                            instance_id: d.instance_id,
                            source_container_id: d.source_container_id.clone(),
                            source_row: d.source_row,
                            source_col: d.source_col,
                            world_pos_x: d.world_pos[0],
                            world_pos_y: d.world_pos[1],
                            world_pos_z: d.world_pos[2],
                            item: Some(inventory_item_view_to_proto(&d.item)),
                        })
                        .collect(),
                })
            }
            ServerDataPayloadV1::BotanyHarvestProgress {
                session_id,
                target_id,
                target_name,
                plant_kind,
                mode,
                progress,
                auto_selectable,
                request_pending,
                interrupted,
                completed,
                detail,
                hazard_hints,
                target_pos,
            } => Payload::BotanyHarvestProgress(bong::BotanyHarvestProgress {
                session_id: session_id.clone(),
                target_id: target_id.clone(),
                target_name: target_name.clone(),
                plant_kind: plant_kind.clone(),
                mode: mode.clone(),
                progress: *progress,
                auto_selectable: *auto_selectable,
                request_pending: *request_pending,
                interrupted: *interrupted,
                completed: *completed,
                detail: detail.clone(),
                hazard_hints: hazard_hints.clone(),
                target_pos_x: target_pos.map(|p| p[0]),
                target_pos_y: target_pos.map(|p| p[1]),
                target_pos_z: target_pos.map(|p| p[2]),
            }),
            ServerDataPayloadV1::BotanyPlantV2RenderProfiles(profiles) => {
                Payload::BotanyPlantV2RenderProfiles(bong::BotanyPlantV2RenderProfiles {
                    profiles: profiles
                        .iter()
                        .map(|p| bong::BotanyPlantV2RenderProfile {
                            plant_id: p.plant_id.clone(),
                            base_mesh_ref: p.base_mesh_ref.clone(),
                            tint_rgb: p.tint_rgb,
                            tint_rgb_secondary: p.tint_rgb_secondary,
                            model_overlay: botany_model_overlay_to_proto(&p.model_overlay),
                        })
                        .collect(),
                })
            }
            ServerDataPayloadV1::MiningProgress {
                session_id,
                ore_pos,
                progress,
                interrupted,
                completed,
            } => Payload::MiningProgress(bong::MiningProgress {
                session_id: session_id.clone(),
                ore_pos_x: ore_pos[0],
                ore_pos_y: ore_pos[1],
                ore_pos_z: ore_pos[2],
                progress: *progress,
                interrupted: *interrupted,
                completed: *completed,
            }),
            ServerDataPayloadV1::LumberProgress {
                session_id,
                log_pos,
                progress,
                interrupted,
                completed,
                detail,
            } => Payload::LumberProgress(bong::LumberProgress {
                session_id: session_id.clone(),
                log_pos_x: log_pos[0],
                log_pos_y: log_pos[1],
                log_pos_z: log_pos[2],
                progress: *progress,
                interrupted: *interrupted,
                completed: *completed,
                detail: detail.clone(),
            }),
            ServerDataPayloadV1::GatheringSession {
                session_id,
                progress_ticks,
                total_ticks,
                target_name,
                target_type,
                quality_hint,
                tool_used,
                interrupted,
                completed,
            } => Payload::GatheringSession(bong::GatheringSession {
                session_id: session_id.clone(),
                progress_ticks: *progress_ticks,
                total_ticks: *total_ticks,
                target_name: target_name.clone(),
                target_type: gathering_target_type_to_proto(target_type),
                quality_hint: gathering_quality_hint_to_proto(quality_hint),
                tool_used: tool_used.clone(),
                interrupted: *interrupted,
                completed: *completed,
            }),
            ServerDataPayloadV1::BotanySkill {
                level,
                xp,
                xp_to_next_level,
                auto_unlock_level,
            } => Payload::BotanySkill(bong::BotanySkill {
                level: *level,
                xp: *xp,
                xp_to_next_level: *xp_to_next_level,
                auto_unlock_level: *auto_unlock_level,
            }),
            ServerDataPayloadV1::AlchemyFurnace(d) => {
                Payload::AlchemyFurnace(alchemy_furnace_to_proto(d))
            }
            ServerDataPayloadV1::AlchemySession(d) => {
                Payload::AlchemySession(alchemy_session_to_proto(d))
            }
            ServerDataPayloadV1::AlchemyOutcomeForecast(d) => {
                Payload::AlchemyOutcomeForecast(alchemy_outcome_forecast_to_proto(d))
            }
            ServerDataPayloadV1::AlchemyOutcomeResolved(d) => {
                Payload::AlchemyOutcomeResolved(alchemy_outcome_resolved_to_proto(d))
            }
            ServerDataPayloadV1::AlchemyRecipeBook(d) => {
                Payload::AlchemyRecipeBook(alchemy_recipe_book_to_proto(d))
            }
            ServerDataPayloadV1::AlchemyContamination(d) => {
                Payload::AlchemyContamination(alchemy_contamination_to_proto(d))
            }
            ServerDataPayloadV1::CombatHudState(s) => {
                Payload::CombatHudState(combat_hud_state_to_proto(s))
            }
            ServerDataPayloadV1::WoundsSnapshot(s) => {
                Payload::WoundsSnapshot(wounds_snapshot_to_proto(s))
            }
            ServerDataPayloadV1::DefenseWindow(w) => {
                Payload::DefenseWindow(defense_window_to_proto(w))
            }
            ServerDataPayloadV1::CastSync(s) => Payload::CastSync(cast_sync_to_proto(s)),
            ServerDataPayloadV1::QuickSlotConfig(c) => {
                Payload::QuickSlotConfig(quick_slot_config_to_proto(c))
            }
            ServerDataPayloadV1::SkillBarConfig(c) => {
                Payload::SkillBarConfig(skill_bar_config_to_proto(c))
            }
            ServerDataPayloadV1::TechniquesSnapshot(s) => {
                Payload::TechniquesSnapshot(techniques_snapshot_to_proto(s))
            }
            ServerDataPayloadV1::SkillConfigSnapshot(s) => {
                Payload::SkillConfigSnapshot(skill_config_snapshot_to_proto(s))
            }
            ServerDataPayloadV1::UnlocksSync(u) => Payload::UnlocksSync(bong::UnlocksSync {
                jiemai: u.jiemai,
                tishi: u.tishi,
                jueling: u.jueling,
            }),
            ServerDataPayloadV1::DerivedAttrsSync(a) => {
                Payload::DerivedAttrsSync(derived_attrs_to_proto(a))
            }
            ServerDataPayloadV1::EventStreamPush(e) => {
                Payload::EventStreamPush(event_stream_push_to_proto(e))
            }
            ServerDataPayloadV1::WeaponEquipped(w) => {
                Payload::WeaponEquipped(weapon_equipped_to_proto(w))
            }
            ServerDataPayloadV1::WeaponBroken(w) => Payload::WeaponBroken(bong::WeaponBroken {
                instance_id: w.instance_id,
                template_id: w.template_id.clone(),
            }),
            ServerDataPayloadV1::ShieldBroken(s) => Payload::ShieldBroken(bong::ShieldBroken {
                instance_id: s.instance_id,
                template_id: s.template_id.clone(),
            }),
            ServerDataPayloadV1::ShieldBlockHit(h) => {
                Payload::ShieldBlockHit(bong::ShieldBlockHit {
                    template_id: h.template_id.clone(),
                })
            }
            ServerDataPayloadV1::TreasureEquipped(t) => {
                Payload::TreasureEquipped(treasure_equipped_to_proto(t))
            }
            ServerDataPayloadV1::VortexState(s) => Payload::VortexState(vortex_state_to_proto(s)),
            ServerDataPayloadV1::DuguPoisonState(s) => {
                Payload::DuguPoisonState(dugu_poison_state_to_proto(s))
            }
            ServerDataPayloadV1::PoisonDoseEvent(e) => {
                Payload::PoisonDoseEvent(poison_dose_event_to_proto(e))
            }
            ServerDataPayloadV1::PoisonOverdoseEvent(e) => {
                Payload::PoisonOverdoseEvent(poison_overdose_event_to_proto(e))
            }
            ServerDataPayloadV1::PoisonTraitState(s) => {
                Payload::PoisonTraitState(poison_trait_state_to_proto(s))
            }
            ServerDataPayloadV1::CarrierState(s) => {
                Payload::CarrierState(carrier_state_to_proto(s))
            }
            ServerDataPayloadV1::FalseSkinState(s) => {
                Payload::FalseSkinState(false_skin_state_to_proto(s))
            }
            ServerDataPayloadV1::LingtianSession(s) => {
                Payload::LingtianSession(lingtian_session_to_proto(s))
            }
            ServerDataPayloadV1::DeathScreen {
                visible,
                cause,
                luck_remaining,
                final_words,
                countdown_until_ms,
                can_reincarnate,
                can_terminate,
                stage,
                death_number,
                zone_kind,
                lifespan,
                cinematic,
            } => Payload::DeathScreen(bong::DeathScreen {
                visible: *visible,
                cause: cause.clone(),
                luck_remaining: *luck_remaining,
                final_words: final_words.clone(),
                countdown_until_ms: *countdown_until_ms,
                can_reincarnate: *can_reincarnate,
                can_terminate: *can_terminate,
                stage: stage.as_ref().map(death_screen_stage_to_proto),
                death_number: *death_number,
                zone_kind: zone_kind.as_ref().map(death_screen_zone_kind_to_proto),
                lifespan: lifespan.as_ref().map(lifespan_to_proto),
                cinematic: cinematic.as_ref().map(death_cinematic_to_proto),
            }),
            ServerDataPayloadV1::TerminateScreen {
                visible,
                final_words,
                epilogue,
                archetype_suggestion,
            } => Payload::TerminateScreen(bong::TerminateScreen {
                visible: *visible,
                final_words: final_words.clone(),
                epilogue: epilogue.clone(),
                archetype_suggestion: archetype_suggestion.clone(),
            }),
            ServerDataPayloadV1::RiftPortalState(s) => {
                Payload::RiftPortalState(bong::RiftPortalState {
                    entity_id: s.entity_id,
                    kind: rift_portal_kind_to_proto(&s.kind),
                    direction: rift_portal_direction_to_proto(&s.direction),
                    family_id: s.family_id.clone(),
                    world_pos_x: s.world_pos[0],
                    world_pos_y: s.world_pos[1],
                    world_pos_z: s.world_pos[2],
                    trigger_radius: s.trigger_radius,
                    current_extract_ticks: s.current_extract_ticks,
                    activation_window_end: s.activation_window_end,
                })
            }
            ServerDataPayloadV1::RiftPortalRemoved(r) => {
                Payload::RiftPortalRemoved(bong::RiftPortalRemoved {
                    entity_id: r.entity_id,
                })
            }
            ServerDataPayloadV1::ExtractStarted(s) => {
                Payload::ExtractStarted(bong::ExtractStarted {
                    player_id: s.player_id.clone(),
                    portal_entity_id: s.portal_entity_id,
                    portal_kind: rift_portal_kind_to_proto(&s.portal_kind),
                    required_ticks: s.required_ticks,
                    at_tick: s.at_tick,
                })
            }
            ServerDataPayloadV1::ExtractProgress(p) => {
                Payload::ExtractProgress(bong::ExtractProgress {
                    player_id: p.player_id.clone(),
                    portal_entity_id: p.portal_entity_id,
                    elapsed_ticks: p.elapsed_ticks,
                    required_ticks: p.required_ticks,
                })
            }
            ServerDataPayloadV1::ExtractCompleted(c) => {
                Payload::ExtractCompleted(bong::ExtractCompleted {
                    player_id: c.player_id.clone(),
                    portal_kind: rift_portal_kind_to_proto(&c.portal_kind),
                    family_id: c.family_id.clone(),
                    exit_pos_x: c.exit_world_pos[0],
                    exit_pos_y: c.exit_world_pos[1],
                    exit_pos_z: c.exit_world_pos[2],
                    at_tick: c.at_tick,
                })
            }
            ServerDataPayloadV1::ExtractAborted(a) => {
                Payload::ExtractAborted(bong::ExtractAborted {
                    player_id: a.player_id.clone(),
                    reason: extract_aborted_reason_to_proto(&a.reason),
                })
            }
            ServerDataPayloadV1::ExtractFailed(f) => Payload::ExtractFailed(bong::ExtractFailed {
                player_id: f.player_id.clone(),
                reason: extract_failed_reason_to_proto(&f.reason),
            }),
            ServerDataPayloadV1::TsyCollapseStartedIpc(t) => {
                Payload::TsyCollapseStarted(bong::TsyCollapseStartedIpc {
                    family_id: t.family_id.clone(),
                    at_tick: t.at_tick,
                    remaining_ticks: t.remaining_ticks,
                    collapse_tear_entity_ids: t.collapse_tear_entity_ids.clone(),
                })
            }
            ServerDataPayloadV1::ContainerState(s) => {
                Payload::ContainerState(bong::ContainerStateProto {
                    entity_id: s.entity_id,
                    kind: container_kind_to_proto(&s.kind),
                    family_id: s.family_id.clone(),
                    world_pos_x: s.world_pos[0],
                    world_pos_y: s.world_pos[1],
                    world_pos_z: s.world_pos[2],
                    locked: s.locked.as_ref().map(key_kind_to_proto),
                    depleted: s.depleted,
                    searched_by_player_id: s.searched_by_player_id.clone(),
                })
            }
            ServerDataPayloadV1::SearchStarted(s) => Payload::SearchStarted(bong::SearchStarted {
                player_id: s.player_id.clone(),
                container_entity_id: s.container_entity_id,
                required_ticks: s.required_ticks,
                at_tick: s.at_tick,
            }),
            ServerDataPayloadV1::SearchProgress(p) => {
                Payload::SearchProgress(bong::SearchProgress {
                    player_id: p.player_id.clone(),
                    container_entity_id: p.container_entity_id,
                    elapsed_ticks: p.elapsed_ticks,
                    required_ticks: p.required_ticks,
                })
            }
            ServerDataPayloadV1::SearchCompleted(c) => {
                Payload::SearchCompleted(bong::SearchCompleted {
                    player_id: c.player_id.clone(),
                    container_entity_id: c.container_entity_id,
                    family_id: c.family_id.clone(),
                    loot_preview: c
                        .loot_preview
                        .iter()
                        .map(|l| bong::LootPreviewItem {
                            template_id: l.template_id.clone(),
                            display_name: l.display_name.clone(),
                            stack_count: l.stack_count,
                        })
                        .collect(),
                    at_tick: c.at_tick,
                })
            }
            ServerDataPayloadV1::SearchAborted(a) => Payload::SearchAborted(bong::SearchAborted {
                player_id: a.player_id.clone(),
                container_entity_id: a.container_entity_id,
                reason: search_abort_reason_to_proto(&a.reason),
                at_tick: a.at_tick,
            }),
            ServerDataPayloadV1::SkillXpGain(s) => Payload::SkillXpGain(skill_xp_gain_to_proto(s)),
            ServerDataPayloadV1::SkillLvUp(s) => Payload::SkillLvUp(bong::SkillLvUp {
                char_id: s.char_id,
                skill: skill_id_to_proto(&s.skill),
                new_lv: s.new_lv as u32,
            }),
            ServerDataPayloadV1::SkillCapChanged(s) => {
                Payload::SkillCapChanged(bong::SkillCapChanged {
                    char_id: s.char_id,
                    skill: skill_id_to_proto(&s.skill),
                    new_cap: s.new_cap as u32,
                })
            }
            ServerDataPayloadV1::SkillScrollUsed(s) => {
                Payload::SkillScrollUsed(bong::SkillScrollUsed {
                    char_id: s.char_id,
                    scroll_id: s.scroll_id.clone(),
                    skill: skill_id_to_proto(&s.skill),
                    xp_granted: s.xp_granted,
                    was_duplicate: s.was_duplicate,
                })
            }
            ServerDataPayloadV1::SkillSnapshot(s) => {
                Payload::SkillSnapshot(skill_snapshot_to_proto(s))
            }
            ServerDataPayloadV1::ForgeStation(d) => {
                Payload::ForgeStation(forge_station_to_proto(d))
            }
            ServerDataPayloadV1::ForgeSession(d) => {
                Payload::ForgeSession(forge_session_to_proto(d))
            }
            ServerDataPayloadV1::ForgeOutcome(d) => {
                Payload::ForgeOutcome(forge_outcome_to_proto(d))
            }
            ServerDataPayloadV1::ForgeBlueprintBook(d) => {
                Payload::ForgeBlueprintBook(forge_blueprint_book_to_proto(d))
            }
            ServerDataPayloadV1::TribulationState(s) => {
                Payload::TribulationState(tribulation_state_to_proto(s))
            }
            ServerDataPayloadV1::TribulationBroadcast(b) => {
                Payload::TribulationBroadcast(bong::TribulationBroadcast {
                    active: b.active,
                    actor_name: b.actor_name.clone(),
                    stage: b.stage.clone(),
                    world_x: b.world_x,
                    world_z: b.world_z,
                    expires_at_ms: b.expires_at_ms,
                    spectate_invite: b.spectate_invite,
                    spectate_distance: b.spectate_distance,
                })
            }
            ServerDataPayloadV1::AscensionQuota(a) => {
                Payload::AscensionQuota(bong::AscensionQuota {
                    occupied_slots: a.occupied_slots,
                    quota_limit: a.quota_limit,
                    available_slots: a.available_slots,
                    total_world_qi: a.total_world_qi,
                    quota_k: a.quota_k,
                    quota_basis: a.quota_basis.clone(),
                })
            }
            ServerDataPayloadV1::HeartDemonOffer(h) => {
                Payload::HeartDemonOffer(bong::HeartDemonOffer {
                    offer_id: h.offer_id.clone(),
                    trigger_id: h.trigger_id.clone(),
                    trigger_label: h.trigger_label.clone(),
                    realm_label: h.realm_label.clone(),
                    composure: h.composure,
                    quota_remaining: h.quota_remaining,
                    quota_total: h.quota_total,
                    expires_at_ms: h.expires_at_ms,
                    choices: h
                        .choices
                        .iter()
                        .map(|c| bong::HeartDemonOfferChoice {
                            choice_id: c.choice_id.clone(),
                            category: c.category.clone(),
                            title: c.title.clone(),
                            effect_summary: c.effect_summary.clone(),
                            flavor: c.flavor.clone(),
                            style_hint: c.style_hint.clone(),
                        })
                        .collect(),
                })
            }
            ServerDataPayloadV1::BurstMeridianEvent(e) => {
                Payload::BurstMeridianEvent(bong::BurstMeridianEvent {
                    skill: e.skill.clone(),
                    caster: e.caster.clone(),
                    target: e.target.clone(),
                    tick: e.tick,
                    overload_ratio: e.overload_ratio,
                    integrity_snapshot: e.integrity_snapshot,
                })
            }
            ServerDataPayloadV1::BreakthroughCinematic(c) => {
                Payload::BreakthroughCinematic(bong::BreakthroughCinematic {
                    actor_id: c.actor_id.clone(),
                    phase: c.phase.clone(),
                    phase_tick: c.phase_tick,
                    phase_duration_ticks: c.phase_duration_ticks,
                    realm_from: c.realm_from.clone(),
                    realm_to: c.realm_to.clone(),
                    result: c.result.clone(),
                    interrupted: c.interrupted,
                    world_pos_x: c.world_pos[0],
                    world_pos_y: c.world_pos[1],
                    world_pos_z: c.world_pos[2],
                    visible_radius_blocks: c.visible_radius_blocks,
                    global: c.global,
                    distant_billboard: c.distant_billboard,
                    particle_density: c.particle_density,
                    intensity: c.intensity,
                    season_overlay: c.season_overlay.clone(),
                    style: c.style.clone(),
                    at_tick: c.at_tick,
                })
            }
            ServerDataPayloadV1::FullPowerChargingState(s) => {
                Payload::FullPowerCharging(bong::FullPowerChargingState {
                    caster_uuid: s.caster_uuid.clone(),
                    active: s.active,
                    qi_committed: s.qi_committed,
                    target_qi: s.target_qi,
                    started_tick: s.started_tick,
                })
            }
            ServerDataPayloadV1::FullPowerRelease(r) => {
                Payload::FullPowerRelease(bong::FullPowerRelease {
                    caster_uuid: r.caster_uuid.clone(),
                    target_uuid: r.target_uuid.clone(),
                    qi_released: r.qi_released,
                    tick: r.tick,
                    hit_pos_x: r.hit_position.map(|p| p[0]),
                    hit_pos_y: r.hit_position.map(|p| p[1]),
                    hit_pos_z: r.hit_position.map(|p| p[2]),
                })
            }
            ServerDataPayloadV1::FullPowerExhaustedState(s) => {
                Payload::FullPowerExhausted(bong::FullPowerExhaustedState {
                    caster_uuid: s.caster_uuid.clone(),
                    active: s.active,
                    started_tick: s.started_tick,
                    recovery_at_tick: s.recovery_at_tick,
                })
            }
            ServerDataPayloadV1::SocialAnonymity(a) => {
                Payload::SocialAnonymity(bong::SocialAnonymity {
                    viewer: a.viewer.clone(),
                    remotes: a
                        .remotes
                        .iter()
                        .map(|r| bong::SocialRemoteIdentity {
                            player_uuid: r.player_uuid.clone(),
                            anonymous: r.anonymous,
                            display_name: r.display_name.clone(),
                            realm_band: r.realm_band.clone(),
                            breath_hint: r.breath_hint.clone(),
                            renown_tags: r.renown_tags.clone(),
                        })
                        .collect(),
                })
            }
            ServerDataPayloadV1::SocialExposure(e) => {
                Payload::SocialExposure(bong::SocialExposure {
                    actor: e.actor.clone(),
                    kind: exposure_kind_to_proto(&e.kind),
                    witnesses: e.witnesses.clone(),
                    tick: e.tick,
                    zone: e.zone.clone(),
                })
            }
            ServerDataPayloadV1::SocialPact(p) => Payload::SocialPact(bong::SocialPact {
                left: p.left.clone(),
                right: p.right.clone(),
                terms: p.terms.clone(),
                tick: p.tick,
                broken: p.broken,
            }),
            ServerDataPayloadV1::SocialFeud(f) => Payload::SocialFeud(bong::SocialFeud {
                left: f.left.clone(),
                right: f.right.clone(),
                tick: f.tick,
                place: f.place.clone(),
            }),
            ServerDataPayloadV1::SocialRenownDelta(d) => {
                Payload::SocialRenownDelta(bong::SocialRenownDelta {
                    char_id: d.char_id.clone(),
                    fame_delta: d.fame_delta,
                    notoriety_delta: d.notoriety_delta,
                    tags_added: d.tags_added.iter().map(renown_tag_to_proto).collect(),
                    tick: d.tick,
                    reason: d.reason.clone(),
                })
            }
            ServerDataPayloadV1::IdentityPanelState(s) => {
                Payload::IdentityPanelState(identity_panel_state_to_proto(s))
            }
            ServerDataPayloadV1::NicheIntrusion(e) => {
                Payload::NicheIntrusion(bong::NicheIntrusion {
                    niche_pos_x: e.niche_pos[0],
                    niche_pos_y: e.niche_pos[1],
                    niche_pos_z: e.niche_pos[2],
                    intruder_id: e.intruder_id.clone(),
                    items_taken: e.items_taken.clone(),
                    taint_delta: e.taint_delta,
                })
            }
            ServerDataPayloadV1::NicheGuardianFatigue(f) => {
                Payload::NicheGuardianFatigue(bong::NicheGuardianFatigue {
                    guardian_kind: guardian_kind_to_proto(&f.guardian_kind),
                    charges_remaining: f.charges_remaining as u32,
                })
            }
            ServerDataPayloadV1::NicheGuardianBroken(b) => {
                Payload::NicheGuardianBroken(bong::NicheGuardianBroken {
                    guardian_kind: guardian_kind_to_proto(&b.guardian_kind),
                    intruder_id: b.intruder_id.clone(),
                })
            }
            ServerDataPayloadV1::SparringInvite(i) => {
                Payload::SparringInvite(sparring_invite_to_proto(i))
            }
            ServerDataPayloadV1::TradeOffer(o) => Payload::TradeOffer(trade_offer_to_proto(o)),
            ServerDataPayloadV1::RealmVisionParams(p) => {
                Payload::RealmVisionParams(realm_vision_params_to_proto(p))
            }
            ServerDataPayloadV1::SpiritualSenseTargets(t) => {
                Payload::SpiritualSenseTargets(spiritual_sense_targets_to_proto(t))
            }
            ServerDataPayloadV1::HealerNpcAiState(s) => {
                Payload::HealerNpcAiState(healer_npc_ai_state_to_proto(s))
            }
            ServerDataPayloadV1::YidaoHudState(s) => {
                Payload::YidaoHudState(yidao_hud_state_to_proto(s))
            }
            ServerDataPayloadV1::MovementState(s) => {
                Payload::MovementState(movement_state_to_proto(s))
            }
            ServerDataPayloadV1::SpiritTreasureState(s) => {
                Payload::SpiritTreasureState(spirit_treasure_state_to_proto(s))
            }
            ServerDataPayloadV1::SpiritTreasureDialogue(d) => {
                Payload::SpiritTreasureDialogue(spirit_treasure_dialogue_to_proto(d))
            }
            ServerDataPayloadV1::CraftRecipeList(l) => {
                Payload::CraftRecipeList(craft_recipe_list_to_proto(l))
            }
            ServerDataPayloadV1::CraftSessionState(s) => {
                Payload::CraftSessionState(craft_session_state_to_proto(s))
            }
            ServerDataPayloadV1::CraftOutcome(o) => {
                Payload::CraftOutcome(craft_outcome_to_proto(o))
            }
            ServerDataPayloadV1::RecipeUnlocked(r) => {
                Payload::RecipeUnlocked(recipe_unlocked_to_proto(r))
            }
            ServerDataPayloadV1::CombatEventFloater(f) => {
                Payload::CombatEventFloater(bong::CombatEventFloater {
                    events: f
                        .events
                        .iter()
                        .map(|e| bong::CombatEventFloaterEntry {
                            kind: e.kind.clone(),
                            amount: e.amount,
                            text: e.text.clone(),
                            x: e.x,
                            y: e.y,
                            z: e.z,
                        })
                        .collect(),
                })
            }
            ServerDataPayloadV1::KnockbackSync(s) => Payload::KnockbackSync(bong::KnockbackSync {
                distance_blocks: s.distance_blocks,
                velocity_blocks_per_tick: s.velocity_blocks_per_tick,
                duration_ticks: s.duration_ticks,
                kinetic_energy: s.kinetic_energy,
                collision_damage: s.collision_damage,
                chain_depth: s.chain_depth as u32,
                block_broken: s.block_broken,
            }),
            ServerDataPayloadV1::TechniqueProficiencyUpdate(u) => {
                Payload::TechniqueProficiencyUpdate(bong::TechniqueProficiencyUpdate {
                    technique_id: u.technique_id.clone(),
                    proficiency: u.proficiency,
                    gain: u.gain,
                })
            }
            ServerDataPayloadV1::PillBuffStatus(s) => {
                Payload::PillBuffStatus(bong::PillBuffStatus {
                    buff_id: s.buff_id.clone(),
                    remaining_ticks: s.remaining_ticks,
                    effect_multiplier: s.effect_multiplier,
                })
            }
            // ─── plan-supply-coffin-loot-ui P1：外部容器 ────────────
            ServerDataPayloadV1::LootContainerOpen(o) => {
                Payload::LootContainerOpen(bong::LootContainerOpen {
                    session_id: o.session_id,
                    source_kind: serde_json::to_string(&o.source_kind).unwrap_or_default(),
                    rows: o.rows as u32,
                    cols: o.cols as u32,
                    placed_items: o.placed_items.iter().map(placed_item_to_proto).collect(),
                    timeout_wall_secs: o.timeout_wall_secs,
                })
            }
            ServerDataPayloadV1::LootContainerUpdate(u) => {
                Payload::LootContainerUpdate(bong::LootContainerUpdate {
                    session_id: u.session_id,
                    placed_items: u.placed_items.iter().map(placed_item_to_proto).collect(),
                })
            }
            ServerDataPayloadV1::LootContainerClose(c) => {
                Payload::LootContainerClose(bong::LootContainerClose {
                    session_id: c.session_id,
                    reason: loot_container_close_reason_to_proto(&c.reason),
                })
            }
            // plan-offscreen-war-v1 P9：历史战事状态 payload（守恒红线：零真元）
            ServerDataPayloadV1::FactionWarState(s) => {
                Payload::FactionWarState(bong::FactionWarState {
                    war_id: s.war_id,
                    zone: s.zone.clone(),
                    region_descriptor: s.region_descriptor.clone(),
                    phase: s.phase.clone(),
                    groups: s.groups.iter().map(|g| u32::from(*g)).collect(),
                    enlist_count: s.enlist_count,
                    mercenary_count: s.mercenary_count,
                    intercept_count: s.intercept_count,
                    spectate_count: s.spectate_count,
                    winner_group: s.winner_group.map(i32::from).unwrap_or(-1),
                    loser_group: s.loser_group.map(i32::from).unwrap_or(-1),
                })
            }
            // plan-combat-skill-feedback-bridges-v1 P4：暗器 HUD（守恒红线：只读事件字段）
            ServerDataPayloadV1::AnqiHud(s) => Payload::AnqiHud(bong::AnqiHud {
                kind: s.kind.clone(),
                echo_count: s.echo_count,
                aim_progress: s.aim_progress,
                charge_progress: s.charge_progress,
                abrasion_container: s.abrasion_container.clone(),
                abrasion_qi_payload: s.abrasion_qi_payload,
                tick: s.tick,
            }),
            // plan-combat-skill-feedback-bridges-v1 P5：毒蛊 v2 HUD S2C（守恒红线：只读已扣量）
            ServerDataPayloadV1::DuguV2SkillCast(s) => {
                Payload::DuguV2SkillCast(bong::DuguV2SkillCast {
                    kind: s.kind.clone(),
                    caster: s.caster.clone(),
                    target: s.target.clone().unwrap_or_default(),
                    taint_tier: s.taint_tier.clone().unwrap_or_default(),
                    reveal_probability: s.reveal_probability,
                    tick: s.tick,
                })
            }
            ServerDataPayloadV1::DuguV2SelfCure(s) => {
                Payload::DuguV2SelfCure(bong::DuguV2SelfCure {
                    caster: s.caster.clone(),
                    gain_percent: s.gain_percent,
                    self_revealed: s.self_revealed,
                    tick: s.tick,
                })
            }
            ServerDataPayloadV1::DuguV2ShroudActive(s) => {
                Payload::DuguV2ShroudActive(bong::DuguV2ShroudActive {
                    caster: s.caster.clone(),
                    strength: s.strength,
                    expires_at_tick: s.expires_at_tick,
                    tick: s.tick,
                })
            }
            ServerDataPayloadV1::PermanentQiMaxDecayApplied(s) => {
                Payload::PermanentQiMaxDecayApplied(bong::PermanentQiMaxDecayApplied {
                    target: s.target.clone(),
                    loss: s.loss,
                    qi_max_after: s.qi_max_after,
                    tick: s.tick,
                })
            }
            // plan-combat-skill-feedback-bridges-v1 P6：剑道人剑共生 HUD（守恒红线：stored_qi 只读）
            ServerDataPayloadV1::SwordBondHudState(s) => {
                Payload::SwordBondHudState(bong::SwordBondHud {
                    active: s.active,
                    grade_index: s.grade_index as i32,
                    grade_name: s.grade_name.clone(),
                    stored_qi_ratio: s.stored_qi_ratio,
                    bond_strength: s.bond_strength,
                    heaven_gate_ready: s.heaven_gate_ready,
                })
            }
            // 震脉 v2 HUD S2C（守恒红线：只读事件字段，不重算真元/经脉）
            ServerDataPayloadV1::ZhenmaiHud(s) => Payload::ZhenmaiHud(bong::ZhenmaiHud {
                skill_id: s.skill_id.clone(),
                meridian_id: s.meridian_id.clone(),
                contam_removed: s.contam_removed,
                remaining_points: s.remaining_points,
                damage_reduction: s.damage_reduction,
                k_drain: s.k_drain,
                duration_ms: s.duration_ms,
                tick: s.tick,
            }),
            // plan-exploration-probe-return-v1 P0：神识感知矿脉 S2C（只读传输，不涉及 qi_physics）
            ServerDataPayloadV1::MineralProbeResult(m) => {
                Payload::MineralProbeResult(bong::MineralProbeResult {
                    kind: m.kind.clone(),
                    mineral_id: m.mineral_id.clone(),
                    remaining_units: m.remaining_units,
                    display_name_zh: m.display_name_zh.clone(),
                    denial_reason: m.denial_reason.clone(),
                })
            }
            // plan-exploration-probe-return-v1 P1：神识感知保鲜 S2C（只读传输，复用 freshness_update 类型串）
            ServerDataPayloadV1::FreshnessUpdate(u) => {
                Payload::FreshnessUpdate(bong::FreshnessUpdate {
                    item_uuid: u.item_uuid.clone(),
                    freshness: u.freshness,
                    profile_name: u.profile_name.clone(),
                })
            }
            // plan-exploration-probe-return-v1 P2：修炼顿悟 S2C（只读传输）
            ServerDataPayloadV1::InsightOffer(offer) => Payload::InsightOffer(bong::InsightOffer {
                offer_id: offer.offer_id.clone(),
                trigger_id: offer.trigger_id.clone(),
                character_id: offer.character_id.clone(),
                choices: offer
                    .choices
                    .iter()
                    .map(|c| bong::InsightChoice {
                        category: c.category.clone(),
                        effect_kind: c.effect_kind.clone(),
                        magnitude: c.magnitude,
                        flavor_text: c.flavor_text.clone(),
                        narrator_voice: c.narrator_voice.clone(),
                        alignment: c.alignment.clone(),
                        cost_kind: c.cost_kind.clone(),
                        cost_magnitude: c.cost_magnitude,
                        cost_flavor: c.cost_flavor.clone(),
                    })
                    .collect(),
            }),
            ServerDataPayloadV1::WorkbenchOpen {
                entity_id,
                position,
            } => Payload::WorkbenchOpen(bong::WorkbenchOpen {
                entity_id: *entity_id,
                x: position[0],
                y: position[1],
                z: position[2],
            }),
            // ─── plan-agent-ui-data-v1 P0：天道 UI-as-Data（无 proto 定义，JSON 旁路）
            // AgentUiRequest / AgentUiClose 通过 JSON CustomPayload 发送给 client，
            // 不走 proto 编码路径。若此分支被触发，说明调用方走错了路径。
            ServerDataPayloadV1::AgentUiRequest(_) | ServerDataPayloadV1::AgentUiClose(_) => {
                unreachable!(
                    "AgentUiRequest/AgentUiClose 经由 JSON CustomPayload 发送，不走 proto 编码路径；\
                    此分支不可达——若触发说明调用方绕过了 JSON bypass 契约"
                )
            }
            // ─── plan-halfstep-rechallenge-integration-v1 P0：HalfStepRechallenge（JSON 旁路）
            // HalfStepRechallenge 通过 bong:server_data JSON CustomPayload 发送，不走 proto 路径。
            ServerDataPayloadV1::HalfStepRechallenge(_) => {
                unreachable!(
                    "HalfStepRechallenge 经由 JSON CustomPayload 发送，不走 proto 编码路径；\
                    此分支不可达——若触发说明调用方绕过了 JSON bypass 契约"
                )
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════
// 复杂嵌套 helper 函数
// ═══════════════════════════════════════════════════════════════

fn botany_model_overlay_to_proto(o: &super::botany::BotanyModelOverlayV1) -> i32 {
    use super::botany::BotanyModelOverlayV1;
    match o {
        BotanyModelOverlayV1::None => bong::BotanyModelOverlay::None as i32,
        BotanyModelOverlayV1::Emissive => bong::BotanyModelOverlay::Emissive as i32,
        BotanyModelOverlayV1::DualPhase => bong::BotanyModelOverlay::DualPhase as i32,
    }
}

fn placed_item_to_proto(pi: &super::inventory::PlacedInventoryItemV1) -> bong::PlacedInventoryItem {
    bong::PlacedInventoryItem {
        container_id: pi.container_id.clone(),
        row: pi.row,
        col: pi.col,
        item: Some(inventory_item_view_to_proto(&pi.item)),
    }
}

fn loot_container_close_reason_to_proto(
    r: &super::server_data::LootContainerCloseReasonV1,
) -> String {
    use super::server_data::LootContainerCloseReasonV1;
    match r {
        LootContainerCloseReasonV1::Timeout => "timeout",
        LootContainerCloseReasonV1::Distance => "distance",
        LootContainerCloseReasonV1::PlayerClosed => "player_closed",
        LootContainerCloseReasonV1::CoffinDestroyed => "coffin_destroyed",
        LootContainerCloseReasonV1::ContainerDestroyed => "container_destroyed",
    }
    .to_string()
}

fn inventory_snapshot_to_proto(
    s: &super::inventory::InventorySnapshotV1,
) -> bong::InventorySnapshot {
    bong::InventorySnapshot {
        revision: s.revision,
        containers: s
            .containers
            .iter()
            .map(|c| bong::ContainerSnapshot {
                id: c.id.clone(),
                name: c.name.clone(),
                rows: c.rows as u32,
                cols: c.cols as u32,
            })
            .collect(),
        placed_items: s.placed_items.iter().map(placed_item_to_proto).collect(),
        equipped: Some(equipped_to_proto(&s.equipped)),
        hotbar: s
            .hotbar
            .iter()
            .map(|slot| bong::HotbarSlot {
                item: slot.as_ref().map(inventory_item_view_to_proto),
            })
            .collect(),
        bone_coins: s.bone_coins,
        weight: Some(bong::InventoryWeight {
            current: s.weight.current,
            max: s.weight.max,
        }),
        realm: s.realm.clone(),
        qi_current: s.qi_current,
        qi_max: s.qi_max,
        body_level: s.body_level,
    }
}

fn equipped_to_proto(
    e: &super::inventory::EquippedInventorySnapshotV1,
) -> bong::EquippedInventorySnapshot {
    let worn_to_proto = |worn: &[super::inventory::InventoryItemViewV1]| {
        worn.iter().map(inventory_item_view_to_proto).collect()
    };
    bong::EquippedInventorySnapshot {
        head_worn: worn_to_proto(&e.head_worn),
        head_held: e.head_held.as_ref().map(inventory_item_view_to_proto),
        chest_worn: worn_to_proto(&e.chest_worn),
        chest_held: e.chest_held.as_ref().map(inventory_item_view_to_proto),
        legs_worn: worn_to_proto(&e.legs_worn),
        legs_held: e.legs_held.as_ref().map(inventory_item_view_to_proto),
        feet_worn: worn_to_proto(&e.feet_worn),
        feet_held: e.feet_held.as_ref().map(inventory_item_view_to_proto),
        main_hand_worn: worn_to_proto(&e.main_hand_worn),
        main_hand_held: e.main_hand_held.as_ref().map(inventory_item_view_to_proto),
        off_hand_worn: worn_to_proto(&e.off_hand_worn),
        off_hand_held: e.off_hand_held.as_ref().map(inventory_item_view_to_proto),
        extra_hand_0_worn: worn_to_proto(&e.extra_hand_0_worn),
        extra_hand_0_held: e
            .extra_hand_0_held
            .as_ref()
            .map(inventory_item_view_to_proto),
        extra_hand_1_worn: worn_to_proto(&e.extra_hand_1_worn),
        extra_hand_1_held: e
            .extra_hand_1_held
            .as_ref()
            .map(inventory_item_view_to_proto),
    }
}

fn inventory_event_to_proto(e: &super::inventory::InventoryEventV1) -> bong::InventoryEvent {
    use super::inventory::InventoryEventV1;
    match e {
        InventoryEventV1::Moved {
            revision,
            instance_id,
            from,
            to,
        } => bong::InventoryEvent {
            event: Some(bong::inventory_event::Event::Moved(
                bong::InventoryEventMoved {
                    revision: *revision,
                    instance_id: *instance_id,
                    from: Some(inventory_location_to_proto(from)),
                    to: Some(inventory_location_to_proto(to)),
                },
            )),
        },
        InventoryEventV1::Dropped {
            revision,
            instance_id,
            from,
            world_pos,
            item,
        } => bong::InventoryEvent {
            event: Some(bong::inventory_event::Event::Dropped(
                bong::InventoryEventDropped {
                    revision: *revision,
                    instance_id: *instance_id,
                    from: Some(inventory_location_to_proto(from)),
                    world_pos_x: world_pos[0],
                    world_pos_y: world_pos[1],
                    world_pos_z: world_pos[2],
                    item: Some(inventory_item_view_to_proto(item)),
                },
            )),
        },
        InventoryEventV1::StackChanged {
            revision,
            instance_id,
            stack_count,
        } => bong::InventoryEvent {
            event: Some(bong::inventory_event::Event::StackChanged(
                bong::InventoryEventStackChanged {
                    revision: *revision,
                    instance_id: *instance_id,
                    stack_count: *stack_count,
                },
            )),
        },
        InventoryEventV1::DurabilityChanged {
            revision,
            instance_id,
            durability,
        } => bong::InventoryEvent {
            event: Some(bong::inventory_event::Event::DurabilityChanged(
                bong::InventoryEventDurabilityChanged {
                    revision: *revision,
                    instance_id: *instance_id,
                    durability: *durability,
                },
            )),
        },
    }
}

fn alchemy_furnace_to_proto(d: &super::alchemy::AlchemyFurnaceDataV1) -> bong::AlchemyFurnace {
    bong::AlchemyFurnace {
        pos_x: d.pos.map(|p| p.0),
        pos_y: d.pos.map(|p| p.1),
        pos_z: d.pos.map(|p| p.2),
        tier: d.tier as u32,
        integrity: d.integrity,
        integrity_max: d.integrity_max,
        owner_name: d.owner_name.clone(),
        has_session: d.has_session,
    }
}

fn alchemy_session_to_proto(d: &super::alchemy::AlchemySessionDataV1) -> bong::AlchemySession {
    bong::AlchemySession {
        recipe_id: d.recipe_id.clone(),
        active: d.active,
        elapsed_ticks: d.elapsed_ticks,
        target_ticks: d.target_ticks,
        temp_current: d.temp_current,
        temp_target: d.temp_target,
        temp_band: d.temp_band,
        qi_injected: d.qi_injected,
        qi_target: d.qi_target,
        status_label: d.status_label.clone(),
        stages: d
            .stages
            .iter()
            .map(|s| bong::AlchemyStageHint {
                at_tick: s.at_tick,
                window: s.window,
                summary: s.summary.clone(),
                completed: s.completed,
                missed: s.missed,
            })
            .collect(),
        interventions_recent: d.interventions_recent.clone(),
    }
}

fn alchemy_outcome_forecast_to_proto(
    d: &super::alchemy::AlchemyOutcomeForecastDataV1,
) -> bong::AlchemyOutcomeForecast {
    bong::AlchemyOutcomeForecast {
        perfect_pct: d.perfect_pct,
        good_pct: d.good_pct,
        flawed_pct: d.flawed_pct,
        waste_pct: d.waste_pct,
        explode_pct: d.explode_pct,
        perfect_note: d.perfect_note.clone(),
        good_note: d.good_note.clone(),
        flawed_note: d.flawed_note.clone(),
    }
}

fn alchemy_outcome_bucket_to_proto(b: &super::alchemy::AlchemyOutcomeBucketV1) -> i32 {
    use super::alchemy::AlchemyOutcomeBucketV1;
    match b {
        AlchemyOutcomeBucketV1::Perfect => bong::AlchemyOutcomeBucket::Perfect as i32,
        AlchemyOutcomeBucketV1::Good => bong::AlchemyOutcomeBucket::Good as i32,
        AlchemyOutcomeBucketV1::Flawed => bong::AlchemyOutcomeBucket::Flawed as i32,
        AlchemyOutcomeBucketV1::Waste => bong::AlchemyOutcomeBucket::Waste as i32,
        AlchemyOutcomeBucketV1::Explode => bong::AlchemyOutcomeBucket::Explode as i32,
    }
}

fn alchemy_outcome_resolved_to_proto(
    d: &super::alchemy::AlchemyOutcomeResolvedDataV1,
) -> bong::AlchemyOutcomeResolved {
    bong::AlchemyOutcomeResolved {
        bucket: alchemy_outcome_bucket_to_proto(&d.bucket),
        recipe_id: d.recipe_id.clone(),
        pill: d.pill.clone(),
        quality: d.quality,
        toxin_amount: d.toxin_amount,
        toxin_color: d.toxin_color.as_ref().map(color_kind_to_proto),
        qi_gain: d.qi_gain,
        side_effect_tag: d.side_effect_tag.clone(),
        flawed_path: d.flawed_path,
        damage: d.damage,
        meridian_crack: d.meridian_crack,
    }
}

fn alchemy_recipe_book_to_proto(
    d: &super::alchemy::AlchemyRecipeBookDataV1,
) -> bong::AlchemyRecipeBook {
    bong::AlchemyRecipeBook {
        learned: d
            .learned
            .iter()
            .map(|e| bong::AlchemyRecipeEntry {
                id: e.id.clone(),
                display_name: e.display_name.clone(),
                body_text: e.body_text.clone(),
                author: e.author.clone(),
                era: e.era.clone(),
                max_known: e.max_known,
            })
            .collect(),
        current_index: d.current_index,
    }
}

fn alchemy_contamination_to_proto(
    d: &super::alchemy::AlchemyContaminationDataV1,
) -> bong::AlchemyContamination {
    bong::AlchemyContamination {
        levels: d
            .levels
            .iter()
            .map(|l| bong::AlchemyContaminationLevel {
                color: color_kind_to_proto(&l.color),
                current: l.current,
                max: l.max,
                ok: l.ok,
            })
            .collect(),
        metabolism_note: d.metabolism_note.clone(),
    }
}

fn combat_hud_state_to_proto(s: &super::combat_hud::CombatHudStateV1) -> bong::CombatHudState {
    bong::CombatHudState {
        hp_percent: s.hp_percent,
        qi_percent: s.qi_percent,
        stamina_percent: s.stamina_percent,
        derived: Some(bong::DerivedAttrFlags {
            flying: s.derived.flying,
            phasing: s.derived.phasing,
            tribulation_locked: s.derived.tribulation_locked,
        }),
    }
}

fn wounds_snapshot_to_proto(s: &super::combat_hud::WoundsSnapshotV1) -> bong::WoundsSnapshot {
    bong::WoundsSnapshot {
        wounds: s
            .wounds
            .iter()
            .map(|w| bong::WoundEntry {
                part: w.part.clone(),
                kind: w.kind.clone(),
                severity: w.severity,
                state: w.state.clone(),
                infection: w.infection,
                scar: w.scar,
                updated_at_ms: w.updated_at_ms,
            })
            .collect(),
    }
}

fn defense_window_to_proto(w: &super::combat_hud::DefenseWindowV1) -> bong::DefenseWindow {
    bong::DefenseWindow {
        duration_ms: w.duration_ms,
        started_at_ms: w.started_at_ms,
        expires_at_ms: w.expires_at_ms,
    }
}

fn cast_phase_to_proto(p: &super::combat_hud::CastPhaseV1) -> i32 {
    use super::combat_hud::CastPhaseV1;
    match p {
        CastPhaseV1::Idle => bong::CastPhase::Idle as i32,
        CastPhaseV1::Casting => bong::CastPhase::Casting as i32,
        CastPhaseV1::Complete => bong::CastPhase::Complete as i32,
        CastPhaseV1::Interrupt => bong::CastPhase::Interrupt as i32,
    }
}

fn cast_outcome_to_proto(o: &super::combat_hud::CastOutcomeV1) -> i32 {
    use super::combat_hud::CastOutcomeV1;
    match o {
        CastOutcomeV1::None => bong::CastOutcome::None as i32,
        CastOutcomeV1::Completed => bong::CastOutcome::Completed as i32,
        CastOutcomeV1::InterruptMovement => bong::CastOutcome::InterruptMovement as i32,
        CastOutcomeV1::InterruptContam => bong::CastOutcome::InterruptContam as i32,
        CastOutcomeV1::InterruptControl => bong::CastOutcome::InterruptControl as i32,
        CastOutcomeV1::UserCancel => bong::CastOutcome::UserCancel as i32,
        CastOutcomeV1::Death => bong::CastOutcome::Death as i32,
        // plan-bug-qc-p1 §skill-cast P0：经脉门控拒绝 → MeridianGated proto 变体。
        CastOutcomeV1::MeridianGated => bong::CastOutcome::MeridianGated as i32,
        // 通用技能警示（plan-skill-warn-hud）：resolver 拒绝原因 → proto 变体。
        CastOutcomeV1::RejectQiInsufficient => bong::CastOutcome::RejectQiInsufficient as i32,
        CastOutcomeV1::RejectOnCooldown => bong::CastOutcome::RejectOnCooldown as i32,
        CastOutcomeV1::RejectInvalidTarget => bong::CastOutcome::RejectInvalidTarget as i32,
        CastOutcomeV1::RejectInRecovery => bong::CastOutcome::RejectInRecovery as i32,
        CastOutcomeV1::RejectRealmTooLow => bong::CastOutcome::RejectRealmTooLow as i32,
        CastOutcomeV1::RejectNoWeapon => bong::CastOutcome::RejectNoWeapon as i32,
    }
}

fn cast_sync_to_proto(s: &super::combat_hud::CastSyncV1) -> bong::CastSync {
    bong::CastSync {
        phase: cast_phase_to_proto(&s.phase),
        slot: s.slot as u32,
        duration_ms: s.duration_ms,
        started_at_ms: s.started_at_ms,
        outcome: cast_outcome_to_proto(&s.outcome),
    }
}

fn quick_slot_config_to_proto(c: &super::combat_hud::QuickSlotConfigV1) -> bong::QuickSlotConfig {
    bong::QuickSlotConfig {
        slots: c
            .slots
            .iter()
            .map(|opt| bong::OptionalQuickSlotEntry {
                entry: opt.as_ref().map(|e| bong::QuickSlotEntry {
                    item_id: e.item_id.clone(),
                    display_name: e.display_name.clone(),
                    cast_duration_ms: e.cast_duration_ms,
                    cooldown_ms: e.cooldown_ms,
                    icon_texture: e.icon_texture.clone(),
                }),
            })
            .collect(),
        cooldown_until_ms: c.cooldown_until_ms.to_vec(),
    }
}

fn skill_bar_config_to_proto(c: &super::combat_hud::SkillBarConfigV1) -> bong::SkillBarConfig {
    bong::SkillBarConfig {
        slots: c
            .slots
            .iter()
            .map(|opt| bong::OptionalSkillBarEntry {
                entry: opt.as_ref().map(|e| {
                    use super::combat_hud::SkillBarEntryV1;
                    match e {
                        SkillBarEntryV1::Item {
                            template_id,
                            display_name,
                            cast_duration_ms,
                            cooldown_ms,
                            icon_texture,
                        } => bong::SkillBarEntry {
                            kind: Some(bong::skill_bar_entry::Kind::Item(
                                bong::SkillBarEntryItem {
                                    template_id: template_id.clone(),
                                    display_name: display_name.clone(),
                                    cast_duration_ms: *cast_duration_ms,
                                    cooldown_ms: *cooldown_ms,
                                    icon_texture: icon_texture.clone(),
                                },
                            )),
                        },
                        SkillBarEntryV1::Skill {
                            skill_id,
                            display_name,
                            cast_duration_ms,
                            cooldown_ms,
                            icon_texture,
                        } => bong::SkillBarEntry {
                            kind: Some(bong::skill_bar_entry::Kind::Skill(
                                bong::SkillBarEntrySkill {
                                    skill_id: skill_id.clone(),
                                    display_name: display_name.clone(),
                                    cast_duration_ms: *cast_duration_ms,
                                    cooldown_ms: *cooldown_ms,
                                    icon_texture: icon_texture.clone(),
                                },
                            )),
                        },
                    }
                }),
            })
            .collect(),
        cooldown_until_ms: c.cooldown_until_ms.to_vec(),
    }
}

fn techniques_snapshot_to_proto(
    s: &super::combat_hud::TechniquesSnapshotV1,
) -> bong::TechniquesSnapshot {
    bong::TechniquesSnapshot {
        entries: s
            .entries
            .iter()
            .map(|e| bong::TechniqueEntry {
                id: e.id.clone(),
                display_name: e.display_name.clone(),
                grade: e.grade.clone(),
                proficiency: e.proficiency,
                proficiency_label: e.proficiency_label.clone(),
                active: e.active,
                description: e.description.clone(),
                required_realm: e.required_realm.clone(),
                required_meridians: e
                    .required_meridians
                    .iter()
                    .map(|m| bong::TechniqueRequiredMeridian {
                        channel: m.channel.clone(),
                        min_health: m.min_health,
                    })
                    .collect(),
                qi_cost: e.qi_cost,
                stamina_cost: e.stamina_cost,
                cast_ticks: e.cast_ticks,
                cooldown_ticks: e.cooldown_ticks,
                range: e.range,
            })
            .collect(),
    }
}

fn skill_config_snapshot_to_proto(
    s: &crate::skill::config::SkillConfigSnapshot,
) -> bong::SkillConfigSnapshotProto {
    bong::SkillConfigSnapshotProto {
        configs: s
            .configs
            .iter()
            .map(|(skill_id, config)| bong::SkillConfigEntry {
                skill_id: skill_id.clone(),
                json_config: serde_json::to_string(&config.fields).unwrap_or_default(),
            })
            .collect(),
    }
}

fn derived_attrs_to_proto(a: &super::combat_hud::DerivedAttrsSyncV1) -> bong::DerivedAttrsSync {
    bong::DerivedAttrsSync {
        flying: a.flying,
        flying_qi_remaining: a.flying_qi_remaining,
        flying_force_descent_at_ms: a.flying_force_descent_at_ms,
        phasing: a.phasing,
        phasing_until_ms: a.phasing_until_ms,
        tribulation_locked: a.tribulation_locked,
        tribulation_stage: a.tribulation_stage.clone(),
        throughput_peak_norm: a.throughput_peak_norm,
        tuike_layers: a.tuike_layers as u32,
        vortex_active: a.vortex_active,
    }
}

fn event_channel_to_proto(c: &super::combat_hud::EventChannelV1) -> i32 {
    use super::combat_hud::EventChannelV1;
    match c {
        EventChannelV1::Combat => bong::EventChannel::Combat as i32,
        EventChannelV1::Cultivation => bong::EventChannel::Cultivation as i32,
        EventChannelV1::World => bong::EventChannel::World as i32,
        EventChannelV1::Social => bong::EventChannel::Social as i32,
        EventChannelV1::System => bong::EventChannel::System as i32,
    }
}

fn event_priority_to_proto(p: &super::combat_hud::EventPriorityV1) -> i32 {
    use super::combat_hud::EventPriorityV1;
    match p {
        EventPriorityV1::P0Critical => bong::EventPriority::P0Critical as i32,
        EventPriorityV1::P1Important => bong::EventPriority::P1Important as i32,
        EventPriorityV1::P2Normal => bong::EventPriority::P2Normal as i32,
        EventPriorityV1::P3Verbose => bong::EventPriority::P3Verbose as i32,
    }
}

fn event_stream_push_to_proto(e: &super::combat_hud::EventStreamPushV1) -> bong::EventStreamPush {
    bong::EventStreamPush {
        channel: event_channel_to_proto(&e.channel),
        priority: event_priority_to_proto(&e.priority),
        source_tag: e.source_tag.clone(),
        text: e.text.clone(),
        color: e.color,
        created_at_ms: e.created_at_ms,
    }
}

fn weapon_equipped_to_proto(w: &super::combat_hud::WeaponEquippedV1) -> bong::WeaponEquipped {
    bong::WeaponEquipped {
        slot: w.slot.clone(),
        weapon: w.weapon.as_ref().map(|wv| bong::WeaponView {
            instance_id: wv.instance_id,
            template_id: wv.template_id.clone(),
            weapon_kind: wv.weapon_kind.clone(),
            durability_current: wv.durability_current,
            durability_max: wv.durability_max,
            quality_tier: wv.quality_tier as u32,
        }),
    }
}

fn treasure_equipped_to_proto(t: &super::combat_hud::TreasureEquippedV1) -> bong::TreasureEquipped {
    bong::TreasureEquipped {
        slot: t.slot.clone(),
        treasure: t.treasure.as_ref().map(|tv| bong::TreasureView {
            instance_id: tv.instance_id,
            template_id: tv.template_id.clone(),
            display_name: tv.display_name.clone(),
        }),
    }
}

fn vortex_state_to_proto(s: &super::woliu::VortexFieldStateV1) -> bong::VortexFieldState {
    bong::VortexFieldState {
        caster: s.caster.clone(),
        active: s.active,
        center_x: s.center[0],
        center_y: s.center[1],
        center_z: s.center[2],
        radius: s.radius,
        delta: s.delta,
        env_qi_at_cast: s.env_qi_at_cast,
        maintain_remaining_ticks: s.maintain_remaining_ticks,
        intercepted_count: s.intercepted_count,
        active_skill_id: s.active_skill_id.clone(),
        charge_progress: s.charge_progress,
        cooldown_until_ms: s.cooldown_until_ms,
        backfire_level: s.backfire_level.clone(),
        turbulence_radius: s.turbulence_radius,
        turbulence_intensity: s.turbulence_intensity,
        turbulence_until_ms: s.turbulence_until_ms,
    }
}

fn dugu_poison_state_to_proto(s: &super::dugu::DuguPoisonStateV1) -> bong::DuguPoisonState {
    bong::DuguPoisonState {
        target: s.target.clone(),
        active: s.active,
        meridian_id: s.meridian_id.clone(),
        attacker: s.attacker.clone(),
        attached_at_tick: s.attached_at_tick,
        poisoner_realm_tier: s.poisoner_realm_tier as u32,
        loss_per_tick: s.loss_per_tick,
        flow_capacity_after: s.flow_capacity_after,
        qi_max_after: s.qi_max_after,
        server_tick: s.server_tick,
    }
}

fn poison_side_effect_tag_to_proto(t: &super::poison_trait::PoisonSideEffectTagV1) -> i32 {
    use super::poison_trait::PoisonSideEffectTagV1;
    match t {
        PoisonSideEffectTagV1::QiFocusDrift2h => bong::PoisonSideEffectTag::QiFocusDrift2h as i32,
        PoisonSideEffectTagV1::RageBurst30m => bong::PoisonSideEffectTag::RageBurst30m as i32,
        PoisonSideEffectTagV1::HallucinTint6h => bong::PoisonSideEffectTag::HallucinTint6h as i32,
        PoisonSideEffectTagV1::DigestLock6h => bong::PoisonSideEffectTag::DigestLock6h as i32,
        PoisonSideEffectTagV1::ToxicityTierUnlock => {
            bong::PoisonSideEffectTag::ToxicityTierUnlock as i32
        }
    }
}

fn poison_dose_event_to_proto(e: &super::poison_trait::PoisonDoseEventV1) -> bong::PoisonDoseEvent {
    bong::PoisonDoseEvent {
        player_entity_id: e.player_entity_id,
        dose_amount: e.dose_amount,
        side_effect_tag: poison_side_effect_tag_to_proto(&e.side_effect_tag),
        poison_level_after: e.poison_level_after,
        digestion_after: e.digestion_after,
        at_tick: e.at_tick,
    }
}

fn poison_overdose_severity_to_proto(s: &super::poison_trait::PoisonOverdoseSeverityV1) -> i32 {
    use super::poison_trait::PoisonOverdoseSeverityV1;
    match s {
        PoisonOverdoseSeverityV1::Mild => bong::PoisonOverdoseSeverity::Mild as i32,
        PoisonOverdoseSeverityV1::Moderate => bong::PoisonOverdoseSeverity::Moderate as i32,
        PoisonOverdoseSeverityV1::Severe => bong::PoisonOverdoseSeverity::Severe as i32,
    }
}

fn poison_overdose_event_to_proto(
    e: &super::poison_trait::PoisonOverdoseEventV1,
) -> bong::PoisonOverdoseEvent {
    bong::PoisonOverdoseEvent {
        player_entity_id: e.player_entity_id,
        severity: poison_overdose_severity_to_proto(&e.severity),
        overflow: e.overflow,
        lifespan_penalty_years: e.lifespan_penalty_years,
        micro_tear_probability: e.micro_tear_probability,
        at_tick: e.at_tick,
    }
}

fn poison_trait_state_to_proto(
    s: &super::poison_trait::PoisonTraitStateV1,
) -> bong::PoisonTraitState {
    bong::PoisonTraitState {
        player_entity_id: s.player_entity_id,
        poison_toxicity: s.poison_toxicity,
        digestion_current: s.digestion_current,
        digestion_capacity: s.digestion_capacity,
        toxicity_tier_unlocked: s.toxicity_tier_unlocked,
    }
}

fn carrier_charge_phase_to_proto(p: &super::combat_carrier::CarrierChargePhaseV1) -> i32 {
    use super::combat_carrier::CarrierChargePhaseV1;
    match p {
        CarrierChargePhaseV1::Idle => bong::CarrierChargePhase::Idle as i32,
        CarrierChargePhaseV1::Charging => bong::CarrierChargePhase::Charging as i32,
        CarrierChargePhaseV1::Charged => bong::CarrierChargePhase::Charged as i32,
    }
}

fn carrier_state_to_proto(s: &super::combat_carrier::CarrierStateV1) -> bong::CarrierState {
    bong::CarrierState {
        carrier: s.carrier.clone(),
        phase: carrier_charge_phase_to_proto(&s.phase),
        progress: s.progress,
        sealed_qi: s.sealed_qi,
        sealed_qi_initial: s.sealed_qi_initial,
        half_life_remaining_ticks: s.half_life_remaining_ticks,
        item_instance_id: s.item_instance_id,
    }
}

fn false_skin_kind_to_proto(k: &super::tuike::FalseSkinKindV1) -> i32 {
    use super::tuike::FalseSkinKindV1;
    match k {
        FalseSkinKindV1::SpiderSilk => bong::FalseSkinKind::SpiderSilk as i32,
        FalseSkinKindV1::RottenWoodArmor => bong::FalseSkinKind::RottenWoodArmor as i32,
    }
}

fn false_skin_tier_to_proto(t: &super::tuike_v2::FalseSkinTierV1) -> i32 {
    use super::tuike_v2::FalseSkinTierV1;
    match t {
        FalseSkinTierV1::Fan => bong::FalseSkinTier::Fan as i32,
        FalseSkinTierV1::Light => bong::FalseSkinTier::Light as i32,
        FalseSkinTierV1::Mid => bong::FalseSkinTier::Mid as i32,
        FalseSkinTierV1::Heavy => bong::FalseSkinTier::Heavy as i32,
        FalseSkinTierV1::Ancient => bong::FalseSkinTier::Ancient as i32,
    }
}

fn false_skin_state_to_proto(s: &super::tuike::FalseSkinStateV1) -> bong::FalseSkinState {
    bong::FalseSkinState {
        target_id: s.target_id.clone(),
        kind: s.kind.as_ref().map(false_skin_kind_to_proto),
        layers_remaining: s.layers_remaining as u32,
        contam_capacity_per_layer: s.contam_capacity_per_layer,
        absorbed_contam: s.absorbed_contam,
        equipped_at_tick: s.equipped_at_tick,
        layers: s
            .layers
            .iter()
            .map(|l| bong::FalseSkinLayerState {
                tier: false_skin_tier_to_proto(&l.tier),
                spirit_quality: l.spirit_quality,
                damage_capacity: l.damage_capacity,
                contam_load: l.contam_load,
                permanent_taint_load: l.permanent_taint_load,
            })
            .collect(),
    }
}

fn lingtian_session_kind_to_proto(k: &super::lingtian::LingtianSessionKindV1) -> i32 {
    use super::lingtian::LingtianSessionKindV1;
    match k {
        LingtianSessionKindV1::Till => bong::LingtianSessionKind::Till as i32,
        LingtianSessionKindV1::Renew => bong::LingtianSessionKind::Renew as i32,
        LingtianSessionKindV1::Planting => bong::LingtianSessionKind::Planting as i32,
        LingtianSessionKindV1::Harvest => bong::LingtianSessionKind::Harvest as i32,
        LingtianSessionKindV1::Replenish => bong::LingtianSessionKind::Replenish as i32,
        LingtianSessionKindV1::DrainQi => bong::LingtianSessionKind::DrainQi as i32,
    }
}

fn lingtian_session_to_proto(
    s: &super::lingtian::LingtianSessionDataV1,
) -> bong::LingtianSessionData {
    bong::LingtianSessionData {
        active: s.active,
        kind: lingtian_session_kind_to_proto(&s.kind),
        pos_x: s.pos[0],
        pos_y: s.pos[1],
        pos_z: s.pos[2],
        elapsed_ticks: s.elapsed_ticks,
        target_ticks: s.target_ticks,
        plant_id: s.plant_id.clone(),
        source: s.source.clone(),
        dye_contamination: s.dye_contamination,
        dye_contamination_warning: s.dye_contamination_warning,
    }
}

fn death_cinematic_phase_to_proto(p: &super::death_cinematic::DeathCinematicPhaseV1) -> i32 {
    use super::death_cinematic::DeathCinematicPhaseV1;
    match p {
        DeathCinematicPhaseV1::Predeath => bong::DeathCinematicPhase::Predeath as i32,
        DeathCinematicPhaseV1::DeathMoment => bong::DeathCinematicPhase::DeathMoment as i32,
        DeathCinematicPhaseV1::Roll => bong::DeathCinematicPhase::Roll as i32,
        DeathCinematicPhaseV1::InsightOverlay => bong::DeathCinematicPhase::InsightOverlay as i32,
        DeathCinematicPhaseV1::Darkness => bong::DeathCinematicPhase::Darkness as i32,
        DeathCinematicPhaseV1::Rebirth => bong::DeathCinematicPhase::Rebirth as i32,
    }
}

fn death_roll_result_to_proto(r: &super::death_cinematic::DeathRollResultV1) -> i32 {
    use super::death_cinematic::DeathRollResultV1;
    match r {
        DeathRollResultV1::Pending => bong::DeathRollResult::Pending as i32,
        DeathRollResultV1::Survive => bong::DeathRollResult::Survive as i32,
        DeathRollResultV1::Fall => bong::DeathRollResult::Fall as i32,
        DeathRollResultV1::Final => bong::DeathRollResult::Final as i32,
    }
}

fn death_cinematic_zone_kind_to_proto(k: &super::death_cinematic::DeathCinematicZoneKindV1) -> i32 {
    use super::death_cinematic::DeathCinematicZoneKindV1;
    match k {
        DeathCinematicZoneKindV1::Ordinary => bong::DeathCinematicZoneKind::Ordinary as i32,
        DeathCinematicZoneKindV1::Death => bong::DeathCinematicZoneKind::Death as i32,
        DeathCinematicZoneKindV1::Negative => bong::DeathCinematicZoneKind::Negative as i32,
    }
}

fn death_cinematic_to_proto(
    c: &super::death_cinematic::DeathCinematicS2cV1,
) -> bong::DeathCinematicData {
    bong::DeathCinematicData {
        v: c.v as u32,
        character_id: c.character_id.clone(),
        phase: death_cinematic_phase_to_proto(&c.phase),
        phase_tick: c.phase_tick,
        phase_duration_ticks: c.phase_duration_ticks,
        total_elapsed_ticks: c.total_elapsed_ticks,
        total_duration_ticks: c.total_duration_ticks,
        roll: Some(bong::DeathCinematicRoll {
            probability: c.roll.probability,
            threshold: c.roll.threshold,
            luck_value: c.roll.luck_value,
            result: death_roll_result_to_proto(&c.roll.result),
        }),
        insight_text: c.insight_text.clone(),
        is_final: c.is_final,
        death_number: c.death_number,
        zone_kind: death_cinematic_zone_kind_to_proto(&c.zone_kind),
        tsy_death: c.tsy_death,
        rebirth_weakened_ticks: c.rebirth_weakened_ticks,
        skip_predeath: c.skip_predeath,
    }
}

fn tribulation_state_to_proto(
    s: &super::server_data::TribulationStateV1,
) -> bong::TribulationState {
    bong::TribulationState {
        active: s.active,
        char_id: s.char_id.clone(),
        actor_name: s.actor_name.clone(),
        kind: s.kind.clone(),
        phase: s.phase.clone(),
        world_x: s.world_x,
        world_z: s.world_z,
        wave_current: s.wave_current,
        wave_total: s.wave_total,
        started_tick: s.started_tick,
        phase_started_tick: s.phase_started_tick,
        next_wave_tick: s.next_wave_tick,
        failed: s.failed,
        half_step_on_success: s.half_step_on_success,
        participants: s.participants.clone(),
        result: s.result.clone(),
    }
}

fn skill_xp_gain_to_proto(s: &super::skill::SkillXpGainPayloadV1) -> bong::SkillXpGain {
    use super::skill::XpGainSourceV1;
    let mut proto = bong::SkillXpGain {
        char_id: s.char_id,
        skill: skill_id_to_proto(&s.skill),
        amount: s.amount,
        source: None,
        source_realm_breakthrough: false,
    };
    match &s.source {
        XpGainSourceV1::Action { plan_id, action } => {
            proto.source = Some(bong::skill_xp_gain::Source::ActionSource(
                bong::XpGainSourceAction {
                    plan_id: plan_id.clone(),
                    action: action.clone(),
                },
            ));
        }
        XpGainSourceV1::Scroll {
            scroll_id,
            xp_grant,
        } => {
            proto.source = Some(bong::skill_xp_gain::Source::ScrollSource(
                bong::XpGainSourceScroll {
                    scroll_id: scroll_id.clone(),
                    xp_grant: *xp_grant,
                },
            ));
        }
        XpGainSourceV1::Mentor { mentor_char } => {
            proto.source = Some(bong::skill_xp_gain::Source::MentorSource(
                bong::XpGainSourceMentor {
                    mentor_char: *mentor_char,
                },
            ));
        }
        XpGainSourceV1::RealmBreakthrough => {
            proto.source_realm_breakthrough = true;
        }
    }
    proto
}

fn skill_snapshot_to_proto(s: &super::skill::SkillSnapshotPayloadV1) -> bong::SkillSnapshotProto {
    bong::SkillSnapshotProto {
        char_id: s.char_id,
        skills: s
            .skills
            .iter()
            .map(|(name, entry)| bong::SkillSnapshotEntry {
                skill_name: name.clone(),
                entry: Some(bong::SkillEntrySnapshot {
                    lv: entry.lv as u32,
                    xp: entry.xp,
                    xp_to_next: entry.xp_to_next,
                    total_xp: entry.total_xp,
                    cap: entry.cap as u32,
                    recent_gain_xp: entry.recent_gain_xp,
                }),
            })
            .collect(),
        consumed_scrolls: s.consumed_scrolls.clone(),
    }
}

fn forge_station_to_proto(d: &super::forge::WeaponForgeStationDataV1) -> bong::ForgeStation {
    bong::ForgeStation {
        station_id: d.station_id.clone(),
        tier: d.tier as u32,
        integrity: d.integrity,
        owner_name: d.owner_name.clone(),
        has_session: d.has_session,
    }
}

fn forge_step_to_proto(s: &super::forge::ForgeStepV1) -> i32 {
    use super::forge::ForgeStepV1;
    match s {
        ForgeStepV1::Billet => bong::ForgeStep::Billet as i32,
        ForgeStepV1::Tempering => bong::ForgeStep::Tempering as i32,
        ForgeStepV1::Inscription => bong::ForgeStep::Inscription as i32,
        ForgeStepV1::Consecration => bong::ForgeStep::Consecration as i32,
        ForgeStepV1::Done => bong::ForgeStep::Done as i32,
    }
}

fn temper_beat_to_proto(b: &super::forge::TemperBeatV1) -> i32 {
    use super::forge::TemperBeatV1;
    match b {
        TemperBeatV1::Light => bong::TemperBeat::Light as i32,
        TemperBeatV1::Heavy => bong::TemperBeat::Heavy as i32,
        TemperBeatV1::Fold => bong::TemperBeat::Fold as i32,
    }
}

fn forge_step_state_to_proto(s: &super::forge::ForgeStepStateDataV1) -> bong::ForgeStepState {
    use super::forge::ForgeStepStateDataV1;
    match s {
        ForgeStepStateDataV1::Billet {
            materials_in,
            active_carrier,
            resolved_tier_cap,
        } => bong::ForgeStepState {
            state: Some(bong::forge_step_state::State::Billet(
                bong::ForgeStepStateBillet {
                    materials_in: materials_in
                        .iter()
                        .map(|(m, c)| bong::ForgeMaterialPair {
                            material: m.clone(),
                            count: *c,
                        })
                        .collect(),
                    active_carrier: active_carrier.clone(),
                    resolved_tier_cap: *resolved_tier_cap,
                },
            )),
        },
        ForgeStepStateDataV1::Tempering {
            pattern,
            beat_cursor,
            hits,
            misses,
            deviation,
            qi_spent,
        } => bong::ForgeStepState {
            state: Some(bong::forge_step_state::State::Tempering(
                bong::ForgeStepStateTempering {
                    pattern: pattern.iter().map(temper_beat_to_proto).collect(),
                    beat_cursor: *beat_cursor,
                    hits: *hits,
                    misses: *misses,
                    deviation: *deviation,
                    qi_spent: *qi_spent,
                },
            )),
        },
        ForgeStepStateDataV1::Inscription {
            filled_slots,
            max_slots,
            failed,
        } => bong::ForgeStepState {
            state: Some(bong::forge_step_state::State::Inscription(
                bong::ForgeStepStateInscription {
                    filled_slots: *filled_slots,
                    max_slots: *max_slots,
                    failed: *failed,
                },
            )),
        },
        ForgeStepStateDataV1::Consecration {
            qi_injected,
            qi_required,
            color_imprint,
        } => bong::ForgeStepState {
            state: Some(bong::forge_step_state::State::Consecration(
                bong::ForgeStepStateConsecration {
                    qi_injected: *qi_injected,
                    qi_required: *qi_required,
                    color_imprint: color_imprint.as_ref().map(color_kind_to_proto),
                },
            )),
        },
        ForgeStepStateDataV1::None => bong::ForgeStepState {
            state: Some(bong::forge_step_state::State::NoneState(true)),
        },
    }
}

fn forge_session_to_proto(d: &super::forge::ForgeSessionDataV1) -> bong::ForgeSessionData {
    bong::ForgeSessionData {
        session_id: d.session_id,
        blueprint_id: d.blueprint_id.clone(),
        blueprint_name: d.blueprint_name.clone(),
        active: d.active,
        current_step: forge_step_to_proto(&d.current_step),
        step_index: d.step_index,
        achieved_tier: d.achieved_tier,
        step_state: Some(forge_step_state_to_proto(&d.step_state)),
    }
}

fn forge_outcome_bucket_to_proto(b: &super::forge::ForgeOutcomeBucketV1) -> i32 {
    use super::forge::ForgeOutcomeBucketV1;
    match b {
        ForgeOutcomeBucketV1::Perfect => bong::ForgeOutcomeBucket::Perfect as i32,
        ForgeOutcomeBucketV1::Good => bong::ForgeOutcomeBucket::Good as i32,
        ForgeOutcomeBucketV1::Flawed => bong::ForgeOutcomeBucket::Flawed as i32,
        ForgeOutcomeBucketV1::Waste => bong::ForgeOutcomeBucket::Waste as i32,
        ForgeOutcomeBucketV1::Explode => bong::ForgeOutcomeBucket::Explode as i32,
    }
}

fn forge_outcome_to_proto(d: &super::forge::ForgeOutcomeDataV1) -> bong::ForgeOutcome {
    bong::ForgeOutcome {
        session_id: d.session_id,
        blueprint_id: d.blueprint_id.clone(),
        bucket: forge_outcome_bucket_to_proto(&d.bucket),
        weapon_item: d.weapon_item.clone(),
        quality: d.quality,
        color: d.color.as_ref().map(color_kind_to_proto),
        side_effects: d.side_effects.clone(),
        achieved_tier: d.achieved_tier,
        flawed_path: d.flawed_path,
    }
}

fn forge_blueprint_book_to_proto(
    d: &super::forge::ForgeBlueprintBookDataV1,
) -> bong::ForgeBlueprintBook {
    bong::ForgeBlueprintBook {
        learned: d
            .learned
            .iter()
            .map(|e| bong::ForgeBlueprintEntry {
                id: e.id.clone(),
                display_name: e.display_name.clone(),
                tier_cap: e.tier_cap as u32,
                step_count: e.step_count,
            })
            .collect(),
        current_index: d.current_index,
    }
}

fn identity_panel_state_to_proto(
    s: &super::identity::IdentityPanelStateV1,
) -> bong::IdentityPanelState {
    bong::IdentityPanelState {
        active_identity_id: s.active_identity_id,
        last_switch_tick: s.last_switch_tick,
        cooldown_remaining_ticks: s.cooldown_remaining_ticks,
        identities: s
            .identities
            .iter()
            .map(|e| bong::IdentityPanelEntry {
                identity_id: e.identity_id,
                display_name: e.display_name.clone(),
                reputation_score: e.reputation_score,
                frozen: e.frozen,
                revealed_tag_kinds: e
                    .revealed_tag_kinds
                    .iter()
                    .map(revealed_tag_kind_to_proto)
                    .collect(),
            })
            .collect(),
    }
}

fn revealed_tag_kind_to_proto(k: &super::identity::RevealedTagKindV1) -> i32 {
    use super::identity::RevealedTagKindV1;
    match k {
        RevealedTagKindV1::DuguRevealed => bong::RevealedTagKind::DuguRevealed as i32,
        RevealedTagKindV1::AnqiMaster => bong::RevealedTagKind::AnqiMaster as i32,
        RevealedTagKindV1::ZhenfaMaster => bong::RevealedTagKind::ZhenfaMaster as i32,
        RevealedTagKindV1::BaomaiUser => bong::RevealedTagKind::BaomaiUser as i32,
        RevealedTagKindV1::TuikeUser => bong::RevealedTagKind::TuikeUser as i32,
        RevealedTagKindV1::WoliuMaster => bong::RevealedTagKind::WoliuMaster as i32,
        RevealedTagKindV1::ZhenmaiUser => bong::RevealedTagKind::ZhenmaiUser as i32,
        RevealedTagKindV1::SwordMaster => bong::RevealedTagKind::SwordMaster as i32,
        RevealedTagKindV1::ForgeMaster => bong::RevealedTagKind::ForgeMaster as i32,
        RevealedTagKindV1::AlchemyMaster => bong::RevealedTagKind::AlchemyMaster as i32,
    }
}

fn sparring_invite_to_proto(i: &super::social::SparringInvitePayloadV1) -> bong::SparringInvite {
    bong::SparringInvite {
        invite_id: i.invite_id.clone(),
        initiator: i.initiator.clone(),
        target: i.target.clone(),
        realm_band: i.realm_band.clone(),
        breath_hint: i.breath_hint.clone(),
        terms: i.terms.clone(),
        expires_at_ms: i.expires_at_ms,
    }
}

fn trade_offer_to_proto(o: &super::social::TradeOfferPayloadV1) -> bong::TradeOffer {
    bong::TradeOffer {
        offer_id: o.offer_id.clone(),
        initiator: o.initiator.clone(),
        target: o.target.clone(),
        offered_item: Some(bong::TradeItemSummary {
            instance_id: o.offered_item.instance_id,
            item_id: o.offered_item.item_id.clone(),
            display_name: o.offered_item.display_name.clone(),
            stack_count: o.offered_item.stack_count,
        }),
        requested_items: o
            .requested_items
            .iter()
            .map(|ri| bong::TradeItemSummary {
                instance_id: ri.instance_id,
                item_id: ri.item_id.clone(),
                display_name: ri.display_name.clone(),
                stack_count: ri.stack_count,
            })
            .collect(),
        expires_at_ms: o.expires_at_ms,
    }
}

fn fog_shape_to_proto(s: &super::realm_vision::FogShapeV1) -> i32 {
    use super::realm_vision::FogShapeV1;
    match s {
        FogShapeV1::Cylinder => bong::FogShape::Cylinder as i32,
        FogShapeV1::Sphere => bong::FogShape::Sphere as i32,
    }
}

fn realm_vision_params_to_proto(
    p: &super::realm_vision::RealmVisionParamsV1,
) -> bong::RealmVisionParams {
    bong::RealmVisionParams {
        fog_start: p.fog_start,
        fog_end: p.fog_end,
        fog_color_rgb: p.fog_color_rgb,
        fog_shape: fog_shape_to_proto(&p.fog_shape),
        vignette_alpha: p.vignette_alpha,
        tint_color_argb: p.tint_color_argb,
        particle_density: p.particle_density,
        transition_ticks: p.transition_ticks,
        server_view_distance_chunks: p.server_view_distance_chunks as u32,
        post_fx_sharpen: p.post_fx_sharpen,
    }
}

fn sense_kind_to_proto(k: &super::realm_vision::SenseKindV1) -> i32 {
    use super::realm_vision::SenseKindV1;
    match k {
        SenseKindV1::LivingQi => bong::SenseKind::LivingQi as i32,
        SenseKindV1::AmbientLeyline => bong::SenseKind::AmbientLeyline as i32,
        SenseKindV1::CultivatorRealm => bong::SenseKind::CultivatorRealm as i32,
        SenseKindV1::HeavenlyGaze => bong::SenseKind::HeavenlyGaze as i32,
        SenseKindV1::CrisisPremonition => bong::SenseKind::CrisisPremonition as i32,
        SenseKindV1::ZhenfaArray => bong::SenseKind::ZhenfaArray as i32,
        SenseKindV1::ZhenfaWardAlert => bong::SenseKind::ZhenfaWardAlert as i32,
        SenseKindV1::SpiritEye => bong::SenseKind::SpiritEye as i32,
        SenseKindV1::NicheIntrusionTrace => bong::SenseKind::NicheIntrusionTrace as i32,
        // plan-fauna-mimic-spider-v1 P2
        SenseKindV1::DisguisedSpider => bong::SenseKind::DisguisedSpider as i32,
        // plan-daozhan-v1 P3
        SenseKindV1::DisguisedDaoZhang => bong::SenseKind::DisguisedDaoZhang as i32,
        // plan-dying-elder-v1 P3
        SenseKindV1::DyingElderQi => bong::SenseKind::DyingElderQi as i32,
    }
}

fn spiritual_sense_targets_to_proto(
    t: &super::realm_vision::SpiritualSenseTargetsV1,
) -> bong::SpiritualSenseTargets {
    bong::SpiritualSenseTargets {
        entries: t
            .entries
            .iter()
            .map(|e| bong::SenseEntry {
                kind: sense_kind_to_proto(&e.kind),
                x: e.x,
                y: e.y,
                z: e.z,
                intensity: e.intensity,
            })
            .collect(),
        generation: t.generation,
    }
}

fn healer_npc_ai_state_to_proto(s: &super::yidao::HealerNpcAiStateV1) -> bong::HealerNpcAiState {
    bong::HealerNpcAiState {
        healer_id: s.healer_id.clone(),
        active_action: s.active_action.clone(),
        queue_len: s.queue_len,
        reputation: s.reputation,
        retreating: s.retreating,
    }
}

fn yidao_skill_id_to_proto(s: &super::yidao::YidaoSkillIdV1) -> i32 {
    use super::yidao::YidaoSkillIdV1;
    match s {
        YidaoSkillIdV1::MeridianRepair => bong::YidaoSkillId::MeridianRepair as i32,
        YidaoSkillIdV1::ContamPurge => bong::YidaoSkillId::ContamPurge as i32,
        YidaoSkillIdV1::EmergencyResuscitate => bong::YidaoSkillId::EmergencyResuscitate as i32,
        YidaoSkillIdV1::LifeExtension => bong::YidaoSkillId::LifeExtension as i32,
        YidaoSkillIdV1::MassMeridianRepair => bong::YidaoSkillId::MassMeridianRepair as i32,
    }
}

fn yidao_hud_state_to_proto(s: &super::yidao::YidaoHudStateV1) -> bong::YidaoHudState {
    bong::YidaoHudState {
        healer_id: s.healer_id.clone(),
        reputation: s.reputation,
        peace_mastery: s.peace_mastery,
        karma: s.karma,
        active_skill: s.active_skill.as_ref().map(yidao_skill_id_to_proto),
        patient_ids: s.patient_ids.clone(),
        patient_hp_percent: s.patient_hp_percent,
        patient_contam_total: s.patient_contam_total,
        severed_meridian_count: s.severed_meridian_count,
        contract_count: s.contract_count,
        mass_preview_count: s.mass_preview_count,
    }
}

fn movement_action_to_proto(a: &super::movement::MovementActionV1) -> i32 {
    use super::movement::MovementActionV1;
    match a {
        MovementActionV1::None => bong::MovementAction::None as i32,
        MovementActionV1::Dashing => bong::MovementAction::Dashing as i32,
    }
}

fn movement_zone_kind_to_proto(k: &super::movement::MovementZoneKindV1) -> i32 {
    use super::movement::MovementZoneKindV1;
    match k {
        MovementZoneKindV1::Normal => bong::MovementZoneKind::Normal as i32,
        MovementZoneKindV1::Dead => bong::MovementZoneKind::Dead as i32,
        MovementZoneKindV1::Negative => bong::MovementZoneKind::Negative as i32,
        MovementZoneKindV1::ResidueAsh => bong::MovementZoneKind::ResidueAsh as i32,
    }
}

fn movement_action_request_kind_to_proto(k: &super::movement::MovementActionRequestV1) -> i32 {
    use super::movement::MovementActionRequestV1;
    match k {
        MovementActionRequestV1::Dash => bong::MovementActionRequestKind::Dash as i32,
    }
}

fn movement_state_to_proto(s: &super::movement::MovementStateV1) -> bong::MovementStateProto {
    bong::MovementStateProto {
        current_speed_multiplier: s.current_speed_multiplier,
        stamina_cost_active: s.stamina_cost_active,
        movement_action: movement_action_to_proto(&s.movement_action),
        zone_kind: movement_zone_kind_to_proto(&s.zone_kind),
        dash_cooldown_remaining_ticks: s.dash_cooldown_remaining_ticks,
        hitbox_height_blocks: s.hitbox_height_blocks,
        stamina_current: s.stamina_current,
        stamina_max: s.stamina_max,
        low_stamina: s.low_stamina,
        last_action_tick: s.last_action_tick,
        rejected_action: s
            .rejected_action
            .as_ref()
            .map(movement_action_request_kind_to_proto),
    }
}

fn spirit_treasure_dialogue_tone_to_proto(
    t: &super::spirit_treasure::SpiritTreasureDialogueToneV1,
) -> i32 {
    use super::spirit_treasure::SpiritTreasureDialogueToneV1;
    match t {
        SpiritTreasureDialogueToneV1::Cold => bong::SpiritTreasureDialogueTone::Cold as i32,
        SpiritTreasureDialogueToneV1::Curious => bong::SpiritTreasureDialogueTone::Curious as i32,
        SpiritTreasureDialogueToneV1::Warning => bong::SpiritTreasureDialogueTone::Warning as i32,
        SpiritTreasureDialogueToneV1::Amused => bong::SpiritTreasureDialogueTone::Amused as i32,
        SpiritTreasureDialogueToneV1::Silent => bong::SpiritTreasureDialogueTone::Silent as i32,
    }
}

fn spirit_treasure_state_to_proto(
    s: &super::spirit_treasure::SpiritTreasureStatePayloadV1,
) -> bong::SpiritTreasureStateProto {
    bong::SpiritTreasureStateProto {
        treasures: s
            .treasures
            .iter()
            .map(|t| bong::SpiritTreasureClientState {
                template_id: t.template_id.clone(),
                display_name: t.display_name.clone(),
                instance_id: t.instance_id,
                equipped: t.equipped,
                passive_active: t.passive_active,
                affinity: t.affinity,
                sleeping: t.sleeping,
                source_sect: t.source_sect.clone(),
                icon_texture: t.icon_texture.clone(),
                passive_effects: t
                    .passive_effects
                    .iter()
                    .map(|pe| bong::SpiritTreasurePassive {
                        kind: pe.kind.clone(),
                        value: pe.value,
                        description: pe.description.clone(),
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn spirit_treasure_dialogue_to_proto(
    d: &super::spirit_treasure::SpiritTreasureDialoguePayloadV1,
) -> bong::SpiritTreasureDialogueProto {
    bong::SpiritTreasureDialogueProto {
        dialogue: Some(bong::SpiritTreasureDialogueData {
            request_id: d.dialogue.request_id.clone(),
            character_id: d.dialogue.character_id.clone(),
            treasure_id: d.dialogue.treasure_id.clone(),
            text: d.dialogue.text.clone(),
            tone: spirit_treasure_dialogue_tone_to_proto(&d.dialogue.tone),
            affinity_delta: d.dialogue.affinity_delta,
        }),
        display_name: d.display_name.clone(),
        zone: d.zone.clone(),
    }
}

fn craft_category_to_proto(c: &super::craft::CraftCategoryV1) -> i32 {
    use super::craft::CraftCategoryV1;
    match c {
        CraftCategoryV1::AnqiCarrier => bong::CraftCategory::AnqiCarrier as i32,
        CraftCategoryV1::DuguPotion => bong::CraftCategory::DuguPotion as i32,
        CraftCategoryV1::TuikeSkin => bong::CraftCategory::TuikeSkin as i32,
        CraftCategoryV1::ZhenfaTrap => bong::CraftCategory::ZhenfaTrap as i32,
        CraftCategoryV1::Tool => bong::CraftCategory::Tool as i32,
        CraftCategoryV1::ArmorCraft => bong::CraftCategory::ArmorCraft as i32,
        CraftCategoryV1::Container => bong::CraftCategory::Container as i32,
        CraftCategoryV1::PoisonPowder => bong::CraftCategory::PoisonPowder as i32,
        CraftCategoryV1::Misc => bong::CraftCategory::Misc as i32,
    }
}

fn craft_recipe_list_to_proto(l: &super::craft::RecipeListV1) -> bong::CraftRecipeList {
    bong::CraftRecipeList {
        v: l.v as u32,
        player_id: l.player_id.clone(),
        recipes: l
            .recipes
            .iter()
            .map(|r| bong::CraftRecipeEntry {
                id: r.id.clone(),
                category: craft_category_to_proto(&r.category),
                display_name: r.display_name.clone(),
                materials: r
                    .materials
                    .iter()
                    .map(|(t, c)| bong::CraftMaterialPair {
                        template_id: t.clone(),
                        count: *c,
                    })
                    .collect(),
                qi_cost: r.qi_cost,
                time_ticks: r.time_ticks,
                output: Some(bong::CraftOutputPair {
                    template_id: r.output.0.clone(),
                    count: r.output.1,
                }),
                requirements: Some(bong::CraftRequirements {
                    realm_min: r.requirements.realm_min.as_ref().map(realm_enum_to_proto),
                    qi_color_min: r.requirements.qi_color_min.as_ref().map(|(c, s)| {
                        bong::QiColorMinPair {
                            color: color_kind_to_proto(c),
                            min_share: *s,
                        }
                    }),
                    skill_lv_min: r.requirements.skill_lv_min.map(|v| v as u32),
                }),
                unlocked: r.unlocked,
            })
            .collect(),
        ts: l.ts,
    }
}

fn craft_session_state_to_proto(s: &super::craft::CraftSessionStateV1) -> bong::CraftSessionState {
    bong::CraftSessionState {
        v: s.v as u32,
        player_id: s.player_id.clone(),
        active: s.active,
        recipe_id: s.recipe_id.clone(),
        elapsed_ticks: s.elapsed_ticks,
        total_ticks: s.total_ticks,
        completed_count: s.completed_count,
        total_count: s.total_count,
        ts: s.ts,
    }
}

fn craft_failure_reason_to_proto(r: &super::craft::CraftFailureReasonV1) -> i32 {
    use super::craft::CraftFailureReasonV1;
    match r {
        CraftFailureReasonV1::PlayerCancelled => bong::CraftFailureReason::PlayerCancelled as i32,
        CraftFailureReasonV1::PlayerDied => bong::CraftFailureReason::PlayerDied as i32,
        CraftFailureReasonV1::InternalError => bong::CraftFailureReason::InternalError as i32,
    }
}

fn craft_outcome_to_proto(o: &super::craft::CraftOutcomeV1) -> bong::CraftOutcome {
    use super::craft::CraftOutcomeV1;
    match o {
        CraftOutcomeV1::Completed {
            v,
            player_id,
            recipe_id,
            output_template,
            output_count,
            completed_at_tick,
            ts,
        } => bong::CraftOutcome {
            outcome: Some(bong::craft_outcome::Outcome::Completed(
                bong::CraftOutcomeCompleted {
                    v: *v as u32,
                    player_id: player_id.clone(),
                    recipe_id: recipe_id.clone(),
                    output_template: output_template.clone(),
                    output_count: *output_count,
                    completed_at_tick: *completed_at_tick,
                    ts: *ts,
                },
            )),
        },
        CraftOutcomeV1::Failed {
            v,
            player_id,
            recipe_id,
            reason,
            material_returned,
            qi_refunded,
            ts,
        } => bong::CraftOutcome {
            outcome: Some(bong::craft_outcome::Outcome::Failed(
                bong::CraftOutcomeFailed {
                    v: *v as u32,
                    player_id: player_id.clone(),
                    recipe_id: recipe_id.clone(),
                    reason: craft_failure_reason_to_proto(reason),
                    material_returned: *material_returned,
                    qi_refunded: *qi_refunded,
                    ts: *ts,
                },
            )),
        },
    }
}

fn insight_trigger_to_proto(t: &super::craft::InsightTriggerV1) -> i32 {
    use super::craft::InsightTriggerV1;
    match t {
        InsightTriggerV1::Breakthrough => bong::InsightTrigger::Breakthrough as i32,
        InsightTriggerV1::NearDeath => bong::InsightTrigger::NearDeath as i32,
        InsightTriggerV1::DefeatStronger => bong::InsightTrigger::DefeatStronger as i32,
    }
}

fn recipe_unlocked_to_proto(r: &super::craft::RecipeUnlockedV1) -> bong::RecipeUnlocked {
    use super::craft::UnlockEventSourceV1;
    bong::RecipeUnlocked {
        v: r.v as u32,
        player_id: r.player_id.clone(),
        recipe_id: r.recipe_id.clone(),
        source: Some(match &r.source {
            UnlockEventSourceV1::Scroll { item_template } => bong::UnlockEventSource {
                source: Some(bong::unlock_event_source::Source::ScrollItemTemplate(
                    item_template.clone(),
                )),
            },
            UnlockEventSourceV1::Mentor { npc_archetype } => bong::UnlockEventSource {
                source: Some(bong::unlock_event_source::Source::MentorNpcArchetype(
                    npc_archetype.clone(),
                )),
            },
            UnlockEventSourceV1::Insight { trigger } => bong::UnlockEventSource {
                source: Some(bong::unlock_event_source::Source::InsightTrigger(
                    insight_trigger_to_proto(trigger),
                )),
            },
        }),
        unlocked_at_tick: r.unlocked_at_tick,
        ts: r.ts,
    }
}

// ═══════════════════════════════════════════════════════════════
// C2S: ClientRequestV1 → bong::client_request_envelope::Payload
// ═══════════════════════════════════════════════════════════════

fn meridian_id_to_proto(m: &crate::cultivation::components::MeridianId) -> i32 {
    use crate::cultivation::components::MeridianId;
    match m {
        MeridianId::Lung => bong::MeridianId::Lung as i32,
        MeridianId::LargeIntestine => bong::MeridianId::LargeIntestine as i32,
        MeridianId::Stomach => bong::MeridianId::Stomach as i32,
        MeridianId::Spleen => bong::MeridianId::Spleen as i32,
        MeridianId::Heart => bong::MeridianId::Heart as i32,
        MeridianId::SmallIntestine => bong::MeridianId::SmallIntestine as i32,
        MeridianId::Bladder => bong::MeridianId::Bladder as i32,
        MeridianId::Kidney => bong::MeridianId::Kidney as i32,
        MeridianId::Pericardium => bong::MeridianId::Pericardium as i32,
        MeridianId::TripleEnergizer => bong::MeridianId::TripleEnergizer as i32,
        MeridianId::Gallbladder => bong::MeridianId::Gallbladder as i32,
        MeridianId::Liver => bong::MeridianId::Liver as i32,
        MeridianId::Ren => bong::MeridianId::Ren as i32,
        MeridianId::Du => bong::MeridianId::Du as i32,
        MeridianId::Chong => bong::MeridianId::Chong as i32,
        MeridianId::Dai => bong::MeridianId::Dai as i32,
        MeridianId::YinQiao => bong::MeridianId::YinQiao as i32,
        MeridianId::YangQiao => bong::MeridianId::YangQiao as i32,
        MeridianId::YinWei => bong::MeridianId::YinWei as i32,
        MeridianId::YangWei => bong::MeridianId::YangWei as i32,
    }
}

fn forge_axis_to_proto(a: &crate::cultivation::forging::ForgeAxis) -> i32 {
    use crate::cultivation::forging::ForgeAxis;
    match a {
        ForgeAxis::Rate => bong::ForgeAxis::Rate as i32,
        ForgeAxis::Capacity => bong::ForgeAxis::Capacity as i32,
    }
}

fn zhenfa_kind_to_proto(k: &crate::zhenfa::ZhenfaKind) -> i32 {
    use crate::zhenfa::ZhenfaKind;
    match k {
        ZhenfaKind::Trap => bong::ZhenfaKind::Trap as i32,
        ZhenfaKind::Ward => bong::ZhenfaKind::Ward as i32,
        ZhenfaKind::WarningTrap => bong::ZhenfaKind::WarningTrap as i32,
        ZhenfaKind::BlastTrap => bong::ZhenfaKind::BlastTrap as i32,
        ZhenfaKind::SlowTrap => bong::ZhenfaKind::SlowTrap as i32,
        ZhenfaKind::ShrineWard => bong::ZhenfaKind::ShrineWard as i32,
        ZhenfaKind::Lingju => bong::ZhenfaKind::Lingju as i32,
        ZhenfaKind::DeceiveHeaven => bong::ZhenfaKind::DeceiveHeaven as i32,
        ZhenfaKind::Illusion => bong::ZhenfaKind::Illusion as i32,
        ZhenfaKind::NetworkArray => bong::ZhenfaKind::NetworkArray as i32,
        ZhenfaKind::BeastTrap => bong::ZhenfaKind::BeastTrap as i32,
        ZhenfaKind::TripWire => bong::ZhenfaKind::TripWire as i32,
        ZhenfaKind::DecoyStake => bong::ZhenfaKind::DecoyStake as i32,
    }
}

fn zhenfa_carrier_kind_to_proto(k: &crate::zhenfa::ZhenfaCarrierKind) -> i32 {
    use crate::zhenfa::ZhenfaCarrierKind;
    match k {
        ZhenfaCarrierKind::CommonStone => bong::ZhenfaCarrierKind::CommonStone as i32,
        ZhenfaCarrierKind::LingqiBlock => bong::ZhenfaCarrierKind::LingqiBlock as i32,
        ZhenfaCarrierKind::NightWitheredVine => bong::ZhenfaCarrierKind::NightWitheredVine as i32,
        ZhenfaCarrierKind::BeastCoreInlaid => bong::ZhenfaCarrierKind::BeastCoreInlaid as i32,
    }
}

fn zhenfa_disarm_mode_to_proto(m: &crate::zhenfa::ZhenfaDisarmMode) -> i32 {
    use crate::zhenfa::ZhenfaDisarmMode;
    match m {
        ZhenfaDisarmMode::Disarm => bong::ZhenfaDisarmMode::Disarm as i32,
        ZhenfaDisarmMode::ForceBreak => bong::ZhenfaDisarmMode::ForceBreak as i32,
    }
}

fn trap_target_face_to_proto(f: &crate::zhenfa::trap_content::TrapTargetFace) -> i32 {
    use crate::zhenfa::trap_content::TrapTargetFace;
    match f {
        TrapTargetFace::Top => bong::TrapTargetFace::Top as i32,
        TrapTargetFace::Bottom => bong::TrapTargetFace::Bottom as i32,
        TrapTargetFace::North => bong::TrapTargetFace::North as i32,
        TrapTargetFace::South => bong::TrapTargetFace::South as i32,
        TrapTargetFace::East => bong::TrapTargetFace::East as i32,
        TrapTargetFace::West => bong::TrapTargetFace::West as i32,
    }
}

fn anqi_carrier_slot_to_proto(s: &super::client_request::AnqiCarrierSlotV1) -> i32 {
    use super::client_request::AnqiCarrierSlotV1;
    match s {
        AnqiCarrierSlotV1::MainHand => bong::AnqiCarrierSlot::MainHand as i32,
        AnqiCarrierSlotV1::OffHand => bong::AnqiCarrierSlot::OffHand as i32,
    }
}

fn anqi_container_kind_to_proto(k: &super::combat_carrier::AnqiContainerKindV1) -> i32 {
    use super::combat_carrier::AnqiContainerKindV1;
    match k {
        AnqiContainerKindV1::HandSlot => bong::AnqiContainerKind::HandSlot as i32,
        AnqiContainerKindV1::Quiver => bong::AnqiContainerKind::Quiver as i32,
        AnqiContainerKindV1::PocketPouch => bong::AnqiContainerKind::PocketPouch as i32,
        AnqiContainerKindV1::Fenglinghe => bong::AnqiContainerKind::Fenglinghe as i32,
    }
}

fn botany_harvest_mode_to_proto(m: &super::botany::BotanyHarvestModeV1) -> i32 {
    use super::botany::BotanyHarvestModeV1;
    match m {
        BotanyHarvestModeV1::Manual => bong::BotanyHarvestMode::Manual as i32,
        BotanyHarvestModeV1::Auto => bong::BotanyHarvestMode::Auto as i32,
    }
}

fn void_action_request_to_proto(r: &super::void_actions::VoidActionRequestV1) -> bong::VoidAction {
    use super::void_actions::VoidActionRequestV1;
    match r {
        VoidActionRequestV1::SuppressTsy { zone_id } => bong::VoidAction {
            request: Some(bong::void_action::Request::SuppressTsy(
                bong::VoidActionSuppressTsy {
                    zone_id: zone_id.clone(),
                },
            )),
        },
        VoidActionRequestV1::ExplodeZone { zone_id } => bong::VoidAction {
            request: Some(bong::void_action::Request::ExplodeZone(
                bong::VoidActionExplodeZone {
                    zone_id: zone_id.clone(),
                },
            )),
        },
        VoidActionRequestV1::Barrier { zone_id, geometry } => {
            use crate::cultivation::void::components::BarrierGeometry;
            let BarrierGeometry::Circle { center, radius } = geometry;
            bong::VoidAction {
                request: Some(bong::void_action::Request::Barrier(
                    bong::VoidActionBarrier {
                        zone_id: zone_id.clone(),
                        center_x: center[0],
                        center_y: center[1],
                        center_z: center[2],
                        radius: *radius,
                    },
                )),
            }
        }
        VoidActionRequestV1::LegacyAssign {
            inheritor_id,
            item_instance_ids,
            message,
        } => bong::VoidAction {
            request: Some(bong::void_action::Request::LegacyAssign(
                bong::VoidActionLegacyAssign {
                    inheritor_id: inheritor_id.clone(),
                    item_instance_ids: item_instance_ids.clone(),
                    message: message.clone(),
                },
            )),
        },
    }
}

fn alchemy_intervention_to_proto(
    i: &super::alchemy::AlchemyInterventionV1,
) -> bong::AlchemyInterventionProto {
    use super::alchemy::AlchemyInterventionV1;
    match i {
        AlchemyInterventionV1::AdjustTemp { temp } => bong::AlchemyInterventionProto {
            kind: Some(bong::alchemy_intervention_proto::Kind::AdjustTemp(*temp)),
        },
        AlchemyInterventionV1::InjectQi { qi } => bong::AlchemyInterventionProto {
            kind: Some(bong::alchemy_intervention_proto::Kind::InjectQi(*qi)),
        },
        AlchemyInterventionV1::AutoProfile { profile_id } => bong::AlchemyInterventionProto {
            kind: Some(bong::alchemy_intervention_proto::Kind::AutoProfileId(
                profile_id.clone(),
            )),
        },
    }
}

fn apply_pill_target_to_proto(t: &super::client_request::ApplyPillTargetV1) -> (i32, Option<i32>) {
    use super::client_request::ApplyPillTargetV1;
    match t {
        ApplyPillTargetV1::SelfTarget => (bong::ApplyPillTargetKind::Self_ as i32, None),
        ApplyPillTargetV1::Meridian { meridian_id } => (
            bong::ApplyPillTargetKind::Meridian as i32,
            Some(meridian_id_to_proto(meridian_id)),
        ),
    }
}

fn skill_bar_binding_to_proto(
    b: &super::client_request::SkillBarBindingV1,
) -> bong::SkillBarBinding {
    use super::client_request::SkillBarBindingV1;
    match b {
        SkillBarBindingV1::Item { template_id } => bong::SkillBarBinding {
            kind: Some(bong::skill_bar_binding::Kind::Item(
                bong::SkillBarBindingItem {
                    template_id: template_id.clone(),
                },
            )),
        },
        SkillBarBindingV1::Skill { skill_id } => bong::SkillBarBinding {
            kind: Some(bong::skill_bar_binding::Kind::Skill(
                bong::SkillBarBindingSkill {
                    skill_id: skill_id.clone(),
                },
            )),
        },
    }
}

impl From<&super::client_request::ClientRequestV1> for bong::client_request_envelope::Payload {
    fn from(req: &super::client_request::ClientRequestV1) -> Self {
        use super::client_request::ClientRequestV1;
        use bong::client_request_envelope::Payload;

        match req {
            ClientRequestV1::SetMeridianTarget { meridian, .. } => {
                Payload::SetMeridianTarget(bong::SetMeridianTarget {
                    meridian: meridian_id_to_proto(meridian),
                })
            }
            ClientRequestV1::BreakthroughRequest { .. } => {
                Payload::BreakthroughRequest(bong::BreakthroughRequest {})
            }
            ClientRequestV1::StartDuXu { .. } => Payload::StartDuXu(bong::StartDuXu {}),
            ClientRequestV1::VoidAction { request, .. } => {
                Payload::VoidAction(void_action_request_to_proto(request))
            }
            ClientRequestV1::MovementAction {
                action,
                yaw_degrees,
                ..
            } => Payload::MovementAction(bong::MovementActionRequest {
                action: movement_action_request_kind_to_proto(action),
                yaw_degrees: *yaw_degrees,
            }),
            ClientRequestV1::AbortTribulation { .. } => {
                Payload::AbortTribulation(bong::AbortTribulation {})
            }
            ClientRequestV1::HeartDemonDecision { choice_idx, .. } => {
                Payload::HeartDemonDecision(bong::HeartDemonDecision {
                    choice_idx: *choice_idx,
                })
            }
            ClientRequestV1::ForgeRequest { meridian, axis, .. } => {
                Payload::ForgeRequest(bong::ForgeRequestC2s {
                    meridian: meridian_id_to_proto(meridian),
                    axis: forge_axis_to_proto(axis),
                })
            }
            ClientRequestV1::InsightDecision {
                trigger_id,
                choice_idx,
                ..
            } => Payload::InsightDecision(bong::InsightDecision {
                trigger_id: trigger_id.clone(),
                choice_idx: *choice_idx,
            }),
            ClientRequestV1::BotanyHarvestRequest {
                session_id, mode, ..
            } => Payload::BotanyHarvestRequest(bong::BotanyHarvestRequest {
                session_id: session_id.clone(),
                mode: botany_harvest_mode_to_proto(mode),
            }),
            // ─── 炼丹 C2S ────────────────────────────────────────
            ClientRequestV1::AlchemyOpenFurnace { furnace_pos, .. } => {
                Payload::AlchemyOpenFurnace(bong::AlchemyOpenFurnace {
                    furnace_pos_x: furnace_pos.0,
                    furnace_pos_y: furnace_pos.1,
                    furnace_pos_z: furnace_pos.2,
                })
            }
            ClientRequestV1::AlchemyFeedSlot {
                furnace_pos,
                slot_idx,
                material,
                count,
                ..
            } => Payload::AlchemyFeedSlot(bong::AlchemyFeedSlot {
                furnace_pos_x: furnace_pos.0,
                furnace_pos_y: furnace_pos.1,
                furnace_pos_z: furnace_pos.2,
                slot_idx: *slot_idx as u32,
                material: material.clone(),
                count: *count,
            }),
            ClientRequestV1::AlchemyTakeBack {
                furnace_pos,
                slot_idx,
                ..
            } => Payload::AlchemyTakeBack(bong::AlchemyTakeBack {
                furnace_pos_x: furnace_pos.0,
                furnace_pos_y: furnace_pos.1,
                furnace_pos_z: furnace_pos.2,
                slot_idx: *slot_idx as u32,
            }),
            ClientRequestV1::AlchemyIgnite {
                furnace_pos,
                recipe_id,
                ..
            } => Payload::AlchemyIgnite(bong::AlchemyIgnite {
                furnace_pos_x: furnace_pos.0,
                furnace_pos_y: furnace_pos.1,
                furnace_pos_z: furnace_pos.2,
                recipe_id: recipe_id.clone(),
            }),
            ClientRequestV1::AlchemyIntervention {
                furnace_pos,
                intervention,
                ..
            } => Payload::AlchemyIntervention(bong::AlchemyIntervention {
                furnace_pos_x: furnace_pos.0,
                furnace_pos_y: furnace_pos.1,
                furnace_pos_z: furnace_pos.2,
                intervention: Some(alchemy_intervention_to_proto(intervention)),
            }),
            ClientRequestV1::AlchemyTurnPage { delta, .. } => {
                Payload::AlchemyTurnPage(bong::AlchemyTurnPage { delta: *delta })
            }
            ClientRequestV1::AlchemyLearnRecipe { recipe_id, .. } => {
                Payload::AlchemyLearnRecipe(bong::AlchemyLearnRecipe {
                    recipe_id: recipe_id.clone(),
                })
            }
            ClientRequestV1::AlchemyLearnRecipeFragment {
                item_instance_id, ..
            } => Payload::AlchemyLearnRecipeFragment(bong::AlchemyLearnRecipeFragment {
                item_instance_id: *item_instance_id,
            }),
            ClientRequestV1::AlchemyTakePill { pill_item_id, .. } => {
                Payload::AlchemyTakePill(bong::AlchemyTakePill {
                    pill_item_id: pill_item_id.clone(),
                })
            }
            ClientRequestV1::AlchemyFurnacePlace {
                x,
                y,
                z,
                item_instance_id,
                ..
            } => Payload::AlchemyFurnacePlace(bong::AlchemyFurnacePlace {
                x: *x,
                y: *y,
                z: *z,
                item_instance_id: *item_instance_id,
            }),
            // ─── 棺材 C2S ────────────────────────────────────────
            ClientRequestV1::CoffinOpen { x, y, z, .. } => Payload::CoffinOpen(bong::CoffinOpen {
                x: *x,
                y: *y,
                z: *z,
            }),
            ClientRequestV1::CoffinPlace {
                x,
                y,
                z,
                item_instance_id,
                ..
            } => Payload::CoffinPlace(bong::CoffinPlace {
                x: *x,
                y: *y,
                z: *z,
                item_instance_id: *item_instance_id,
            }),
            ClientRequestV1::BlockPlace {
                x,
                y,
                z,
                item_instance_id,
                target_face,
                ..
            } => Payload::BlockPlace(bong::BlockPlace {
                x: *x,
                y: *y,
                z: *z,
                item_instance_id: *item_instance_id,
                target_face: trap_target_face_to_proto(target_face),
            }),
            ClientRequestV1::CoffinEnter { x, y, z, .. } => {
                Payload::CoffinEnter(bong::CoffinEnter {
                    x: *x,
                    y: *y,
                    z: *z,
                })
            }
            ClientRequestV1::CoffinLeave { .. } => Payload::CoffinLeave(bong::CoffinLeave {}),
            // plan-coffin-tiers-v1 P3 — 延寿棺 marker 交互 C2S。
            ClientRequestV1::CoffinBreak { x, y, z, .. } => {
                Payload::CoffinBreak(bong::CoffinBreak {
                    x: *x,
                    y: *y,
                    z: *z,
                })
            }
            ClientRequestV1::CoffinMenuReclaim { x, y, z, .. } => {
                Payload::CoffinMenuReclaim(bong::CoffinMenuReclaim {
                    x: *x,
                    y: *y,
                    z: *z,
                })
            }
            // ─── 灵龛 / 社交 C2S ──────────────────────────────────
            ClientRequestV1::SpiritNichePlace {
                x,
                y,
                z,
                item_instance_id,
                ..
            } => Payload::SpiritNichePlace(bong::SpiritNichePlace {
                x: *x,
                y: *y,
                z: *z,
                item_instance_id: *item_instance_id,
            }),
            ClientRequestV1::SpiritNicheRepair {
                x,
                y,
                z,
                item_instance_id,
                ..
            } => Payload::SpiritNicheRepair(bong::SpiritNicheRepair {
                x: *x,
                y: *y,
                z: *z,
                item_instance_id: *item_instance_id,
            }),
            ClientRequestV1::SpiritNicheGaze { x, y, z, .. } => {
                Payload::SpiritNicheGaze(bong::SpiritNicheGaze {
                    x: *x,
                    y: *y,
                    z: *z,
                })
            }
            ClientRequestV1::SpiritNicheMarkCoordinate { x, y, z, .. } => {
                Payload::SpiritNicheMarkCoordinate(bong::SpiritNicheMarkCoordinate {
                    x: *x,
                    y: *y,
                    z: *z,
                })
            }
            ClientRequestV1::SpiritNicheActivateGuardian {
                niche_pos,
                guardian_kind,
                materials,
                ..
            } => Payload::SpiritNicheActivateGuardian(bong::SpiritNicheActivateGuardian {
                niche_pos_x: niche_pos[0],
                niche_pos_y: niche_pos[1],
                niche_pos_z: niche_pos[2],
                guardian_kind: guardian_kind_to_proto(guardian_kind),
                materials: materials.clone(),
            }),
            ClientRequestV1::SparringInviteResponse {
                invite_id,
                accepted,
                timed_out,
                ..
            } => Payload::SparringInviteResponse(bong::SparringInviteResponse {
                invite_id: invite_id.clone(),
                accepted: *accepted,
                timed_out: *timed_out,
            }),
            ClientRequestV1::TradeOfferRequest {
                target,
                offered_instance_id,
                ..
            } => Payload::TradeOfferRequest(bong::TradeOfferRequest {
                target: target.clone(),
                offered_instance_id: *offered_instance_id,
            }),
            ClientRequestV1::TradeOfferResponse {
                offer_id,
                accepted,
                requested_instance_id,
                ..
            } => Payload::TradeOfferResponse(bong::TradeOfferResponse {
                offer_id: offer_id.clone(),
                accepted: *accepted,
                requested_instance_id: *requested_instance_id,
            }),
            // ─── NPC C2S ──────────────────────────────────────────
            ClientRequestV1::NpcInspectRequest { npc_entity_id, .. } => {
                Payload::NpcInspectRequest(bong::NpcInspectRequest {
                    npc_entity_id: *npc_entity_id,
                })
            }
            ClientRequestV1::NpcDialogueChoice {
                npc_entity_id,
                option_id,
                ..
            } => Payload::NpcDialogueChoice(bong::NpcDialogueChoice {
                npc_entity_id: *npc_entity_id,
                option_id: option_id.clone(),
            }),
            ClientRequestV1::NpcTradeRequest {
                npc_entity_id,
                offered_items,
                requested_item_id,
                ..
            } => Payload::NpcTradeRequest(bong::NpcTradeRequest {
                npc_entity_id: *npc_entity_id,
                offered_items: offered_items.clone(),
                requested_item_id: requested_item_id.clone(),
            }),
            // ─── 阵法 C2S ────────────────────────────────────────
            ClientRequestV1::ZhenfaPlace {
                x,
                y,
                z,
                kind,
                carrier,
                qi_invest_ratio,
                trigger,
                item_instance_id,
                target_face,
                ..
            } => Payload::ZhenfaPlace(bong::ZhenfaPlace {
                x: *x,
                y: *y,
                z: *z,
                kind: zhenfa_kind_to_proto(kind),
                carrier: carrier.as_ref().map(zhenfa_carrier_kind_to_proto),
                qi_invest_ratio: *qi_invest_ratio,
                trigger: trigger.clone(),
                item_instance_id: *item_instance_id,
                target_face: target_face.as_ref().map(trap_target_face_to_proto),
            }),
            ClientRequestV1::ZhenfaTrigger { instance_id, .. } => {
                Payload::ZhenfaTrigger(bong::ZhenfaTrigger {
                    instance_id: *instance_id,
                })
            }
            ClientRequestV1::ZhenfaDisarm { x, y, z, mode, .. } => {
                Payload::ZhenfaDisarm(bong::ZhenfaDisarm {
                    x: *x,
                    y: *y,
                    z: *z,
                    mode: zhenfa_disarm_mode_to_proto(mode),
                })
            }
            ClientRequestV1::QiScatterBeadUse {
                item_instance_id,
                x,
                y,
                z,
                ..
            } => Payload::QiScatterBeadUse(bong::QiScatterBeadUse {
                item_instance_id: *item_instance_id,
                x: *x,
                y: *y,
                z: *z,
            }),
            // ─── 学习 / 功法 C2S ──────────────────────────────────
            ClientRequestV1::LearnSkillScroll { instance_id, .. } => {
                Payload::LearnSkillScroll(bong::LearnSkillScroll {
                    instance_id: *instance_id,
                })
            }
            ClientRequestV1::TechniqueScrollUse { instance_id, .. } => {
                Payload::TechniqueScrollUse(bong::TechniqueScrollUse {
                    instance_id: *instance_id,
                })
            }
            // ─── 背包 C2S ────────────────────────────────────────
            ClientRequestV1::InventoryMoveIntent {
                instance_id,
                from,
                to,
                ..
            } => Payload::InventoryMoveIntent(bong::InventoryMoveIntent {
                instance_id: *instance_id,
                from: Some(inventory_location_to_proto(from)),
                to: Some(inventory_location_to_proto(to)),
            }),
            ClientRequestV1::EquipFalseSkin {
                slot,
                item_instance_id,
                ..
            } => Payload::EquipFalseSkin(bong::EquipFalseSkin {
                slot: equip_slot_to_proto(slot),
                item_instance_id: *item_instance_id,
            }),
            ClientRequestV1::ForgeFalseSkin { kind, .. } => {
                Payload::ForgeFalseSkin(bong::ForgeFalseSkinC2s {
                    kind: false_skin_kind_to_proto(kind),
                })
            }
            ClientRequestV1::InventoryDiscardItem {
                instance_id, from, ..
            } => Payload::InventoryDiscardItem(bong::InventoryDiscardItem {
                instance_id: *instance_id,
                from: Some(inventory_location_to_proto(from)),
            }),
            ClientRequestV1::TreasureActivate {
                instance_id,
                activate,
                ..
            } => Payload::TreasureActivate(bong::TreasureActivate {
                instance_id: *instance_id,
                activate: *activate,
            }),
            ClientRequestV1::DropWeaponIntent {
                instance_id, from, ..
            } => Payload::DropWeaponIntent(bong::DropWeaponIntent {
                instance_id: *instance_id,
                from: Some(inventory_location_to_proto(from)),
            }),
            ClientRequestV1::RepairWeaponIntent {
                instance_id,
                station_pos,
                ..
            } => Payload::RepairWeaponIntent(bong::RepairWeaponIntent {
                instance_id: *instance_id,
                station_pos_x: station_pos[0],
                station_pos_y: station_pos[1],
                station_pos_z: station_pos[2],
            }),
            ClientRequestV1::PickupDroppedItem { instance_id, .. } => {
                Payload::PickupDroppedItem(bong::PickupDroppedItem {
                    instance_id: *instance_id,
                })
            }
            // ─── 矿石 C2S ────────────────────────────────────────
            ClientRequestV1::MineralProbe { x, y, z, .. } => {
                Payload::MineralProbe(bong::MineralProbe {
                    x: *x,
                    y: *y,
                    z: *z,
                })
            }
            // ─── plan-exploration-probe-return-v1 P1：神识感知保鲜 C2S ────────
            ClientRequestV1::FreshnessProbe { instance_id, .. } => {
                Payload::FreshnessProbe(bong::FreshnessProbe {
                    instance_id: *instance_id,
                })
            }
            // ─── 丹药 C2S ────────────────────────────────────────
            ClientRequestV1::ApplyPill {
                instance_id,
                target,
                ..
            } => {
                let (target_kind, meridian_id) = apply_pill_target_to_proto(target);
                Payload::ApplyPill(bong::ApplyPill {
                    instance_id: *instance_id,
                    target_kind,
                    meridian_id,
                })
            }
            ClientRequestV1::SelfAntidote { instance_id, .. } => {
                Payload::SelfAntidote(bong::SelfAntidote {
                    instance_id: *instance_id,
                })
            }
            // ─── 夺舍 / 气色 / 命核 C2S ──────────────────────────
            ClientRequestV1::DuoSheRequest { target_id, .. } => {
                Payload::DuoSheRequest(bong::DuoSheRequest {
                    target_id: target_id.clone(),
                })
            }
            ClientRequestV1::QiColorInspect { observed, .. } => {
                Payload::QiColorInspect(bong::QiColorInspect {
                    observed: observed.clone(),
                })
            }
            ClientRequestV1::UseLifeCore { instance_id, .. } => {
                Payload::UseLifeCore(bong::UseLifeCore {
                    instance_id: *instance_id,
                })
            }
            // ─── 截脉 / 暗器 / 技能栏 C2S ────────────────────────
            ClientRequestV1::Jiemai { .. } => Payload::Jiemai(bong::Jiemai {}),
            ClientRequestV1::ChargeCarrier {
                slot, qi_target, ..
            } => Payload::ChargeCarrier(bong::ChargeCarrier {
                slot: slot.as_ref().map(anqi_carrier_slot_to_proto),
                qi_target: *qi_target,
            }),
            ClientRequestV1::ThrowCarrier {
                slot,
                dir_unit,
                power,
                ..
            } => Payload::ThrowCarrier(bong::ThrowCarrier {
                slot: anqi_carrier_slot_to_proto(slot),
                dir_x: dir_unit[0],
                dir_y: dir_unit[1],
                dir_z: dir_unit[2],
                power: *power,
            }),
            ClientRequestV1::AnqiContainerSwitch { to, .. } => {
                Payload::AnqiContainerSwitch(bong::AnqiContainerSwitch {
                    to: to.as_ref().map(anqi_container_kind_to_proto),
                })
            }
            ClientRequestV1::UseQuickSlot { slot, .. } => {
                Payload::UseQuickSlot(bong::UseQuickSlot { slot: *slot as u32 })
            }
            ClientRequestV1::QuickSlotBind { slot, item_id, .. } => {
                Payload::QuickSlotBind(bong::QuickSlotBind {
                    slot: *slot as u32,
                    item_id: item_id.clone(),
                })
            }
            ClientRequestV1::SkillBarCast { slot, target, .. } => {
                Payload::SkillBarCast(bong::SkillBarCast {
                    slot: *slot as u32,
                    target: target.clone(),
                })
            }
            ClientRequestV1::SkillBarBind { slot, binding, .. } => {
                Payload::SkillBarBind(bong::SkillBarBind {
                    slot: *slot as u32,
                    binding: binding.as_ref().map(skill_bar_binding_to_proto),
                })
            }
            ClientRequestV1::SkillConfigIntent {
                skill_id, config, ..
            } => Payload::SkillConfigIntent(bong::SkillConfigIntent {
                skill_id: skill_id.clone(),
                json_config: serde_json::to_string(config).unwrap_or_default(),
            }),
            // ─── 战斗生命周期 C2S ─────────────────────────────────
            ClientRequestV1::CombatReincarnate { .. } => {
                Payload::CombatReincarnate(bong::CombatReincarnate {})
            }
            ClientRequestV1::CombatTerminate { .. } => {
                Payload::CombatTerminate(bong::CombatTerminate {})
            }
            ClientRequestV1::CombatCreateNewCharacter { .. } => {
                Payload::CombatCreateNewCharacter(bong::CombatCreateNewCharacter {})
            }
            // ─── 裂隙 C2S ────────────────────────────────────────
            ClientRequestV1::StartExtractRequest {
                portal_entity_id, ..
            } => Payload::StartExtractRequest(bong::StartExtractRequest {
                portal_entity_id: *portal_entity_id,
            }),
            ClientRequestV1::CancelExtractRequest { .. } => {
                Payload::CancelExtractRequest(bong::CancelExtractRequest {})
            }
            ClientRequestV1::StartSearch {
                container_entity_id,
                ..
            } => Payload::StartSearch(bong::StartSearch {
                container_entity_id: *container_entity_id,
            }),
            ClientRequestV1::CancelSearch { .. } => Payload::CancelSearch(bong::CancelSearch {}),
            // ─── 灵田 C2S ────────────────────────────────────────
            ClientRequestV1::LingtianStartTill {
                x,
                y,
                z,
                hoe_instance_id,
                mode,
                ..
            } => Payload::LingtianStartTill(bong::LingtianStartTill {
                x: *x,
                y: *y,
                z: *z,
                hoe_instance_id: *hoe_instance_id,
                mode: mode.clone(),
            }),
            ClientRequestV1::LingtianStartRenew {
                x,
                y,
                z,
                hoe_instance_id,
                ..
            } => Payload::LingtianStartRenew(bong::LingtianStartRenew {
                x: *x,
                y: *y,
                z: *z,
                hoe_instance_id: *hoe_instance_id,
            }),
            ClientRequestV1::LingtianStartPlanting {
                x, y, z, plant_id, ..
            } => Payload::LingtianStartPlanting(bong::LingtianStartPlanting {
                x: *x,
                y: *y,
                z: *z,
                plant_id: plant_id.clone(),
            }),
            ClientRequestV1::LingtianStartHarvest { x, y, z, mode, .. } => {
                Payload::LingtianStartHarvest(bong::LingtianStartHarvest {
                    x: *x,
                    y: *y,
                    z: *z,
                    mode: mode.clone(),
                })
            }
            ClientRequestV1::LingtianStartReplenish {
                x, y, z, source, ..
            } => Payload::LingtianStartReplenish(bong::LingtianStartReplenish {
                x: *x,
                y: *y,
                z: *z,
                source: source.clone(),
            }),
            ClientRequestV1::LingtianStartDrainQi { x, y, z, .. } => {
                Payload::LingtianStartDrainQi(bong::LingtianStartDrainQi {
                    x: *x,
                    y: *y,
                    z: *z,
                })
            }
            // ─── 锻造 C2S ────────────────────────────────────────
            ClientRequestV1::ForgeStartSession {
                station_id,
                blueprint_id,
                materials,
                ..
            } => Payload::ForgeStartSession(bong::ForgeStartSession {
                station_id: station_id.clone(),
                blueprint_id: blueprint_id.clone(),
                materials: materials
                    .iter()
                    .map(|(id, count)| bong::ForgeMaterialPair {
                        material: id.clone(),
                        count: *count,
                    })
                    .collect(),
            }),
            ClientRequestV1::ForgeTemperingHit {
                session_id,
                beat,
                ticks_remaining,
                ..
            } => Payload::ForgeTemperingHit(bong::ForgeTemperingHit {
                session_id: *session_id,
                beat: beat.clone(),
                ticks_remaining: *ticks_remaining,
            }),
            ClientRequestV1::ForgeInscriptionScroll {
                session_id,
                inscription_id,
                ..
            } => Payload::ForgeInscriptionScroll(bong::ForgeInscriptionScroll {
                session_id: *session_id,
                inscription_id: inscription_id.clone(),
            }),
            ClientRequestV1::ForgeConsecrationInject {
                session_id,
                qi_amount,
                ..
            } => Payload::ForgeConsecrationInject(bong::ForgeConsecrationInject {
                session_id: *session_id,
                qi_amount: *qi_amount,
            }),
            ClientRequestV1::ForgeStepAdvance { session_id, .. } => {
                Payload::ForgeStepAdvance(bong::ForgeStepAdvance {
                    session_id: *session_id,
                })
            }
            ClientRequestV1::ForgeBlueprintTurnPage { delta, .. } => {
                Payload::ForgeBlueprintTurnPage(bong::ForgeBlueprintTurnPage { delta: *delta })
            }
            ClientRequestV1::ForgeLearnBlueprint { blueprint_id, .. } => {
                Payload::ForgeLearnBlueprint(bong::ForgeLearnBlueprint {
                    blueprint_id: blueprint_id.clone(),
                })
            }
            ClientRequestV1::ForgeStationPlace {
                x,
                y,
                z,
                item_instance_id,
                station_tier,
                ..
            } => Payload::ForgeStationPlace(bong::ForgeStationPlace {
                x: *x,
                y: *y,
                z: *z,
                item_instance_id: *item_instance_id,
                station_tier: *station_tier as u32,
            }),
            // ─── 工坊手搓 C2S ─────────────────────────────────────
            ClientRequestV1::CraftStart {
                recipe_id,
                quantity,
                ..
            } => Payload::CraftStart(bong::CraftStart {
                recipe_id: recipe_id.clone(),
                quantity: *quantity,
            }),
            ClientRequestV1::CraftCancel { .. } => Payload::CraftCancel(bong::CraftCancel {}),
            // ─── plan-supply-coffin-loot-ui P1：外部容器 C2S ────────
            ClientRequestV1::ExternalContainerMove {
                session_id,
                instance_id,
                from,
                to,
                ..
            } => Payload::ExternalContainerMove(bong::ExternalContainerMove {
                session_id: *session_id,
                instance_id: *instance_id,
                from: Some(inventory_location_to_proto(from)),
                to: Some(inventory_location_to_proto(to)),
            }),
            ClientRequestV1::ExternalContainerClose { session_id, .. } => {
                Payload::ExternalContainerClose(bong::ExternalContainerCloseReq {
                    session_id: *session_id,
                })
            }
            ClientRequestV1::SupplyCoffinOpen { entity_id, .. } => {
                Payload::SupplyCoffinOpen(bong::SupplyCoffinOpenReq {
                    entity_id: *entity_id,
                })
            }
            ClientRequestV1::ContainerOpen { entity_id, .. } => {
                Payload::ContainerOpen(bong::ContainerOpenReq {
                    entity_id: *entity_id,
                })
            }
            // ── plan-dying-elder-v1 P1：垂死大能给丹 C2S ─────────────────────
            ClientRequestV1::GiveDanToElder {
                pill_instance_id,
                elder_entity_id,
                ..
            } => Payload::GiveDanToElder(bong::GiveDanToElderReq {
                pill_instance_id: *pill_instance_id,
                elder_entity_id: *elder_entity_id,
            }),
            ClientRequestV1::WorkbenchOpen { entity_id, .. } => {
                Payload::WorkbenchOpen(bong::WorkbenchOpenReq {
                    entity_id: *entity_id,
                })
            }
            // plan-shield-block-v1 P1 — 持续举盾 / 放盾 C2S
            ClientRequestV1::RaiseShield { .. } => Payload::RaiseShield(bong::RaiseShield {}),
            ClientRequestV1::LowerShield { .. } => Payload::LowerShield(bong::LowerShield {}),
            // ─── plan-agent-ui-data-v1 P0：天道 UI 响应（JSON CustomPayload，无 proto 定义）
            // AgentUiResponse 通过 JSON CustomPayload 接收，不走 proto 编码路径。
            // 若此分支被触发，说明调用方走错了路径（proto bypass 契约被破坏）。
            ClientRequestV1::AgentUiResponse { .. } => {
                unreachable!(
                    "AgentUiResponse 经由 JSON CustomPayload 接收，不走 proto 编码路径；\
                    此分支不可达——若触发说明调用方绕过了 JSON bypass 契约"
                )
            }
            ClientRequestV1::BlockPickerGive { .. } => {
                // plan-worldgen-v4 P5 §8.1#5 — 画廊 dev-only give-block 仅在 server
                // 本地 dev handler 处理（gamemode 门控），不转发给天道 agent IPC，
                // 因此不需要 proto 编码路径。
                unreachable!(
                    "BlockPickerGive 为 dev-only 本地请求，不走 proto 编码路径；\
                    此分支不可达——若触发说明调用方误把 dev 请求转发到了 agent IPC"
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;

    /// S2C: encode -> decode roundtrip preserves payload variant.
    fn s2c_encode_decode_roundtrip(payload: ServerDataPayloadV1) {
        let proto_payload = server_data_to_proto_payload(&payload);
        let envelope = bong::ServerDataEnvelope {
            payload: Some(proto_payload),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = bong::ServerDataEnvelope::decode(bytes.as_slice())
            .expect("S2C proto decode should succeed");
        assert!(
            decoded.payload.is_some(),
            "decoded S2C envelope should contain a payload"
        );
    }

    #[test]
    fn s2c_welcome_roundtrip() {
        s2c_encode_decode_roundtrip(ServerDataPayloadV1::Welcome {
            message: "hello".to_string(),
        });
    }

    #[test]
    fn s2c_heartbeat_roundtrip() {
        s2c_encode_decode_roundtrip(ServerDataPayloadV1::Heartbeat {
            message: "ping".to_string(),
        });
    }

    #[test]
    fn s2c_narration_roundtrip() {
        s2c_encode_decode_roundtrip(ServerDataPayloadV1::Narration {
            narrations: vec![super::super::narration::Narration {
                text: "test".to_string(),
                scope: super::super::common::NarrationScope::Broadcast,
                style: super::super::common::NarrationStyle::Narration,
                kind: None,
                target: None,
            }],
        });
    }

    #[test]
    fn s2c_event_alert_roundtrip() {
        s2c_encode_decode_roundtrip(ServerDataPayloadV1::EventAlert {
            event: super::super::common::EventKind::ThunderTribulation,
            message: "watch out".to_string(),
            zone: Some("spawn".to_string()),
            duration_ticks: Some(100),
        });
    }

    #[test]
    fn s2c_coffin_state_roundtrip() {
        use super::super::server_data::{CoffinGradeV1, CoffinStateV1};
        // mundane grade (baseline)
        s2c_encode_decode_roundtrip(ServerDataPayloadV1::CoffinState(CoffinStateV1 {
            in_coffin: true,
            lifespan_rate_multiplier: 1.0,
            coffin_grade: Some(CoffinGradeV1::Mundane),
        }));
    }

    /// Proto wire roundtrip must NOT drop coffin_grade for non-mundane grades.
    /// Regression guard for blocker #1/#6：生产 wire 走 proto，grade 必须全程传递。
    #[test]
    fn s2c_coffin_state_grade_roundtrip_non_mundane() {
        use super::super::server_data::{CoffinGradeV1, CoffinStateV1};
        use bong::server_data_envelope::Payload;

        let cases: &[(CoffinGradeV1, &str)] = &[
            (CoffinGradeV1::Jade, "jade"),
            (CoffinGradeV1::Stone, "stone"),
            (CoffinGradeV1::Bronze, "bronze"),
        ];
        for (grade, expected_str) in cases {
            let payload = ServerDataPayloadV1::CoffinState(CoffinStateV1 {
                in_coffin: true,
                lifespan_rate_multiplier: 0.5,
                coffin_grade: Some(*grade),
            });
            let proto_payload = server_data_to_proto_payload(&payload);
            let envelope = bong::ServerDataEnvelope {
                payload: Some(proto_payload),
            };
            let bytes = envelope.encode_to_vec();
            let decoded = bong::ServerDataEnvelope::decode(bytes.as_slice())
                .expect("S2C proto decode should succeed");
            match decoded.payload {
                Some(Payload::CoffinState(ref c)) => {
                    assert_eq!(
                        c.coffin_grade.as_deref(),
                        Some(*expected_str),
                        "grade={expected_str} should survive proto encode/decode roundtrip, \
                         but decoded coffin_grade={:?}",
                        c.coffin_grade
                    );
                }
                other => panic!("expected CoffinState payload, got {other:?}"),
            }
        }
    }

    /// Proto wire roundtrip: grade=None（出棺）→ coffin_grade 字段省略（不写入 wire）。
    #[test]
    fn s2c_coffin_state_grade_none_not_in_wire() {
        use super::super::server_data::CoffinStateV1;
        use bong::server_data_envelope::Payload;

        let payload = ServerDataPayloadV1::CoffinState(CoffinStateV1 {
            in_coffin: false,
            lifespan_rate_multiplier: 1.0,
            coffin_grade: None,
        });
        let proto_payload = server_data_to_proto_payload(&payload);
        let envelope = bong::ServerDataEnvelope {
            payload: Some(proto_payload),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = bong::ServerDataEnvelope::decode(bytes.as_slice())
            .expect("S2C proto decode should succeed");
        match decoded.payload {
            Some(Payload::CoffinState(ref c)) => {
                assert!(
                    c.coffin_grade.is_none(),
                    "grade=None (出棺) should not be present in decoded proto, \
                     but decoded coffin_grade={:?}",
                    c.coffin_grade
                );
            }
            other => panic!("expected CoffinState payload, got {other:?}"),
        }
    }

    // ─── plan-exploration-probe-return-v1 S2C proto round-trip ─────────────────

    /// P0: MineralProbeResult found — wire round-trip + 逐字段保真。
    #[test]
    fn s2c_mineral_probe_result_found_proto_roundtrip() {
        use bong::server_data_envelope::Payload;
        use prost::Message;
        let data = super::super::server_data::MineralProbeResultV1 {
            kind: "found".to_string(),
            mineral_id: Some("cu_tie".to_string()),
            remaining_units: Some(42),
            display_name_zh: Some("赤铜矿脉".to_string()),
            denial_reason: None,
        };
        let payload = ServerDataPayloadV1::MineralProbeResult(data);
        let proto = server_data_to_proto_payload(&payload);
        let envelope = bong::ServerDataEnvelope {
            payload: Some(proto),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = bong::ServerDataEnvelope::decode(bytes.as_slice())
            .expect("MineralProbeResult proto decode");
        match decoded.payload {
            Some(Payload::MineralProbeResult(r)) => {
                assert_eq!(r.kind, "found", "kind should be 'found'");
                assert_eq!(
                    r.mineral_id.as_deref(),
                    Some("cu_tie"),
                    "mineral_id should round-trip"
                );
                assert_eq!(
                    r.remaining_units,
                    Some(42),
                    "remaining_units should round-trip"
                );
                assert_eq!(
                    r.display_name_zh.as_deref(),
                    Some("赤铜矿脉"),
                    "display_name_zh should round-trip"
                );
                assert!(
                    r.denial_reason.is_none(),
                    "denial_reason should be None for found"
                );
            }
            other => panic!("expected MineralProbeResult payload, got {other:?}"),
        }
    }

    /// P0: MineralProbeResult denied — denial_reason 字段保真。
    #[test]
    fn s2c_mineral_probe_result_denied_proto_roundtrip() {
        use bong::server_data_envelope::Payload;
        use prost::Message;
        let data = super::super::server_data::MineralProbeResultV1 {
            kind: "denied".to_string(),
            mineral_id: None,
            remaining_units: None,
            display_name_zh: None,
            denial_reason: Some("realm_too_low".to_string()),
        };
        let payload = ServerDataPayloadV1::MineralProbeResult(data);
        let proto = server_data_to_proto_payload(&payload);
        let envelope = bong::ServerDataEnvelope {
            payload: Some(proto),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = bong::ServerDataEnvelope::decode(bytes.as_slice())
            .expect("MineralProbeResult denied proto decode");
        match decoded.payload {
            Some(Payload::MineralProbeResult(r)) => {
                assert_eq!(r.kind, "denied", "kind should be 'denied'");
                assert_eq!(
                    r.denial_reason.as_deref(),
                    Some("realm_too_low"),
                    "denial_reason should round-trip"
                );
                assert!(
                    r.mineral_id.is_none(),
                    "mineral_id should be None for denied"
                );
            }
            other => panic!("expected MineralProbeResult payload, got {other:?}"),
        }
    }

    /// P1: FreshnessUpdate — item_uuid + freshness + profile_name 字段保真。
    #[test]
    fn s2c_freshness_update_proto_roundtrip() {
        use super::super::processing::FreshnessUpdateV1;
        s2c_encode_decode_roundtrip(ServerDataPayloadV1::FreshnessUpdate(FreshnessUpdateV1 {
            item_uuid: "99".to_string(),
            freshness: 0.65,
            profile_name: "ling_cao".to_string(),
        }));
    }

    /// P2: InsightOffer — round-trip via helper（含 choice fields）。
    #[test]
    fn s2c_insight_offer_proto_roundtrip() {
        use super::super::cultivation::{InsightChoiceV1, InsightOfferV1};
        s2c_encode_decode_roundtrip(ServerDataPayloadV1::InsightOffer(InsightOfferV1 {
            offer_id: "insight:1:100".to_string(),
            trigger_id: "insight:1:100".to_string(),
            character_id: "offline:Kiz".to_string(),
            choices: vec![InsightChoiceV1 {
                category: "Qi".to_string(),
                effect_kind: "qi_max".to_string(),
                magnitude: 0.05,
                flavor_text: "气海微扩张。".to_string(),
                narrator_voice: None,
                alignment: None,
                cost_kind: None,
                cost_magnitude: None,
                cost_flavor: None,
            }],
        }));
    }

    #[test]
    fn s2c_workbench_open_proto_roundtrip() {
        use bong::server_data_envelope::Payload;
        use prost::Message;
        let payload = ServerDataPayloadV1::WorkbenchOpen {
            entity_id: 42,
            position: [1, 64, -2],
        };
        let proto = server_data_to_proto_payload(&payload);
        let envelope = bong::ServerDataEnvelope {
            payload: Some(proto),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            bong::ServerDataEnvelope::decode(bytes.as_slice()).expect("WorkbenchOpen proto decode");
        match decoded.payload {
            Some(Payload::WorkbenchOpen(open)) => {
                assert_eq!(open.entity_id, 42);
                assert_eq!([open.x, open.y, open.z], [1, 64, -2]);
            }
            other => panic!("expected WorkbenchOpen payload, got {other:?}"),
        }
    }

    #[test]
    fn s2c_ui_open_roundtrip() {
        s2c_encode_decode_roundtrip(ServerDataPayloadV1::UiOpen {
            ui: Some("inspect".to_string()),
            xml: String::new(),
        });
    }

    #[test]
    fn s2c_to_proto_bytes_method_encodes_successfully() {
        use super::super::server_data::ServerDataV1;
        let payload = ServerDataV1::welcome("hello world");
        let bytes = payload.to_proto_bytes();
        assert!(!bytes.is_empty(), "proto bytes should not be empty");
        let decoded = bong::ServerDataEnvelope::decode(bytes.as_slice())
            .expect("to_proto_bytes output should decode as ServerDataEnvelope");
        assert!(decoded.payload.is_some());
        match decoded.payload.unwrap() {
            bong::server_data_envelope::Payload::Welcome(w) => {
                assert_eq!(w.message, "hello world");
            }
            other => panic!("expected Welcome payload, got {other:?}"),
        }
    }

    /// C2S: encode -> decode roundtrip.
    fn c2s_encode_decode_roundtrip(req: super::super::client_request::ClientRequestV1) {
        let proto_payload = bong::client_request_envelope::Payload::from(&req);
        let envelope = bong::ClientRequestEnvelope {
            payload: Some(proto_payload),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = bong::ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("C2S proto decode should succeed");
        assert!(
            decoded.payload.is_some(),
            "decoded C2S envelope should contain a payload for {req:?}"
        );
    }

    #[test]
    fn c2s_set_meridian_target_roundtrip() {
        c2s_encode_decode_roundtrip(
            super::super::client_request::ClientRequestV1::SetMeridianTarget {
                v: 1,
                meridian: crate::cultivation::components::MeridianId::Lung,
            },
        );
    }

    #[test]
    fn c2s_breakthrough_request_roundtrip() {
        c2s_encode_decode_roundtrip(
            super::super::client_request::ClientRequestV1::BreakthroughRequest { v: 1 },
        );
    }

    #[test]
    fn c2s_alchemy_open_furnace_roundtrip() {
        c2s_encode_decode_roundtrip(
            super::super::client_request::ClientRequestV1::AlchemyOpenFurnace {
                v: 1,
                furnace_pos: (10, 64, -20),
            },
        );
    }

    #[test]
    fn c2s_craft_start_roundtrip() {
        c2s_encode_decode_roundtrip(super::super::client_request::ClientRequestV1::CraftStart {
            v: 1,
            recipe_id: "bone_needle".to_string(),
            quantity: 3,
        });
    }

    #[test]
    fn c2s_zhenfa_place_roundtrip() {
        c2s_encode_decode_roundtrip(super::super::client_request::ClientRequestV1::ZhenfaPlace {
            v: 1,
            x: 100,
            y: 64,
            z: -50,
            kind: crate::zhenfa::ZhenfaKind::Trap,
            carrier: Some(crate::zhenfa::ZhenfaCarrierKind::CommonStone),
            qi_invest_ratio: 0.5,
            trigger: None,
            item_instance_id: Some(999),
            target_face: Some(crate::zhenfa::trap_content::TrapTargetFace::Top),
        });
    }

    #[test]
    fn c2s_zhenfa_kind_proto_pins_include_trap_runtime_variants() {
        use crate::zhenfa::ZhenfaKind;
        let cases = [
            (ZhenfaKind::Trap, bong::ZhenfaKind::Trap),
            (ZhenfaKind::Ward, bong::ZhenfaKind::Ward),
            (ZhenfaKind::WarningTrap, bong::ZhenfaKind::WarningTrap),
            (ZhenfaKind::BlastTrap, bong::ZhenfaKind::BlastTrap),
            (ZhenfaKind::SlowTrap, bong::ZhenfaKind::SlowTrap),
            (ZhenfaKind::ShrineWard, bong::ZhenfaKind::ShrineWard),
            (ZhenfaKind::Lingju, bong::ZhenfaKind::Lingju),
            (ZhenfaKind::DeceiveHeaven, bong::ZhenfaKind::DeceiveHeaven),
            (ZhenfaKind::Illusion, bong::ZhenfaKind::Illusion),
            (ZhenfaKind::NetworkArray, bong::ZhenfaKind::NetworkArray),
            (ZhenfaKind::BeastTrap, bong::ZhenfaKind::BeastTrap),
            (ZhenfaKind::TripWire, bong::ZhenfaKind::TripWire),
            (ZhenfaKind::DecoyStake, bong::ZhenfaKind::DecoyStake),
        ];
        for (rust, proto) in cases {
            assert_eq!(
                zhenfa_kind_to_proto(&rust),
                proto as i32,
                "ZhenfaKind::{rust:?} must map to protobuf {proto:?}"
            );
            assert_ne!(
                zhenfa_kind_to_proto(&rust),
                bong::ZhenfaKind::Unspecified as i32,
                "ZhenfaKind::{rust:?} must not map to UNSPECIFIED"
            );
        }
        assert_eq!(
            cases.len(),
            13,
            "ZhenfaKind proto coverage must include the 13 P0 variants"
        );
    }

    #[test]
    fn c2s_block_place_roundtrip() {
        c2s_encode_decode_roundtrip(super::super::client_request::ClientRequestV1::BlockPlace {
            v: 1,
            x: 8,
            y: 64,
            z: -8,
            item_instance_id: 4242,
            target_face: crate::zhenfa::trap_content::TrapTargetFace::North,
        });
    }

    #[test]
    fn c2s_void_action_barrier_roundtrip() {
        use crate::cultivation::void::components::BarrierGeometry;
        c2s_encode_decode_roundtrip(super::super::client_request::ClientRequestV1::VoidAction {
            v: 1,
            request: super::super::void_actions::VoidActionRequestV1::Barrier {
                zone_id: "spawn".to_string(),
                geometry: BarrierGeometry::Circle {
                    center: [100.0, 64.0, -50.0],
                    radius: 10.0,
                },
            },
        });
    }

    #[test]
    fn c2s_movement_action_roundtrip() {
        c2s_encode_decode_roundtrip(
            super::super::client_request::ClientRequestV1::MovementAction {
                v: 1,
                action: super::super::movement::MovementActionRequestV1::Dash,
                yaw_degrees: Some(90.0),
            },
        );
    }

    #[test]
    fn c2s_skill_config_intent_roundtrip() {
        use std::collections::BTreeMap;
        let mut config = BTreeMap::new();
        config.insert(
            "range".to_string(),
            serde_json::Value::Number(serde_json::Number::from(5)),
        );
        c2s_encode_decode_roundtrip(
            super::super::client_request::ClientRequestV1::SkillConfigIntent {
                v: 1,
                skill_id: "fireball".to_string(),
                config,
            },
        );
    }

    #[test]
    fn c2s_apply_pill_self_roundtrip() {
        c2s_encode_decode_roundtrip(super::super::client_request::ClientRequestV1::ApplyPill {
            v: 1,
            instance_id: 42,
            target: super::super::client_request::ApplyPillTargetV1::SelfTarget,
        });
    }

    #[test]
    fn c2s_apply_pill_meridian_roundtrip() {
        c2s_encode_decode_roundtrip(super::super::client_request::ClientRequestV1::ApplyPill {
            v: 1,
            instance_id: 42,
            target: super::super::client_request::ApplyPillTargetV1::Meridian {
                meridian_id: crate::cultivation::components::MeridianId::Heart,
            },
        });
    }

    // ─── plan-supply-coffin-loot-ui P1：外部容器 proto roundtrip ────

    #[test]
    fn s2c_loot_container_open_roundtrip() {
        let payload = ServerDataPayloadV1::LootContainerOpen(
            super::super::server_data::LootContainerOpenV1 {
                session_id: 42,
                source_kind: super::super::server_data::LootContainerSourceKindV1::SupplyCoffin {
                    grade: "common".to_string(),
                },
                rows: 3,
                cols: 4,
                placed_items: vec![],
                timeout_wall_secs: 1716872400,
            },
        );
        s2c_encode_decode_roundtrip(payload.clone());

        // Field-level verification on the decoded proto payload
        let proto_payload = server_data_to_proto_payload(&payload);
        let envelope = bong::ServerDataEnvelope {
            payload: Some(proto_payload),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = bong::ServerDataEnvelope::decode(bytes.as_slice())
            .expect("S2C proto decode should succeed");
        match decoded.payload {
            Some(bong::server_data_envelope::Payload::LootContainerOpen(open)) => {
                assert_eq!(open.session_id, 42, "session_id should survive roundtrip");
                assert_eq!(open.rows, 3, "rows should survive roundtrip");
                assert_eq!(open.cols, 4, "cols should survive roundtrip");
                assert!(
                    open.placed_items.is_empty(),
                    "empty placed_items should survive roundtrip"
                );
                assert_eq!(
                    open.timeout_wall_secs, 1716872400,
                    "timeout_wall_secs should survive roundtrip"
                );
                assert!(
                    open.source_kind.contains("supply_coffin"),
                    "source_kind should contain supply_coffin, got: {}",
                    open.source_kind
                );
            }
            other => panic!("expected LootContainerOpen payload, got {other:?}"),
        }
    }

    #[test]
    fn s2c_loot_container_update_roundtrip() {
        let payload = ServerDataPayloadV1::LootContainerUpdate(
            super::super::server_data::LootContainerUpdateV1 {
                session_id: 7,
                placed_items: vec![],
            },
        );
        s2c_encode_decode_roundtrip(payload.clone());

        // Field-level verification on the decoded proto payload
        let proto_payload = server_data_to_proto_payload(&payload);
        let envelope = bong::ServerDataEnvelope {
            payload: Some(proto_payload),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = bong::ServerDataEnvelope::decode(bytes.as_slice())
            .expect("S2C proto decode should succeed");
        match decoded.payload {
            Some(bong::server_data_envelope::Payload::LootContainerUpdate(upd)) => {
                assert_eq!(upd.session_id, 7, "session_id should survive roundtrip");
                assert!(
                    upd.placed_items.is_empty(),
                    "empty placed_items should survive roundtrip"
                );
            }
            other => panic!("expected LootContainerUpdate payload, got {other:?}"),
        }
    }

    #[test]
    fn s2c_loot_container_close_roundtrip() {
        use super::super::server_data::{LootContainerCloseReasonV1, LootContainerCloseV1};

        let cases = [
            (LootContainerCloseReasonV1::Timeout, "timeout"),
            (LootContainerCloseReasonV1::Distance, "distance"),
            (LootContainerCloseReasonV1::PlayerClosed, "player_closed"),
            (
                LootContainerCloseReasonV1::CoffinDestroyed,
                "coffin_destroyed",
            ),
            (
                LootContainerCloseReasonV1::ContainerDestroyed,
                "container_destroyed",
            ),
        ];

        for (reason, expected_wire) in cases {
            let payload = ServerDataPayloadV1::LootContainerClose(LootContainerCloseV1 {
                session_id: 99,
                reason: reason.clone(),
            });
            s2c_encode_decode_roundtrip(payload.clone());

            // Field-level verification on the decoded proto payload
            let proto_payload = server_data_to_proto_payload(&payload);
            let envelope = bong::ServerDataEnvelope {
                payload: Some(proto_payload),
            };
            let bytes = envelope.encode_to_vec();
            let decoded = bong::ServerDataEnvelope::decode(bytes.as_slice())
                .expect("S2C proto decode should succeed");
            match decoded.payload {
                Some(bong::server_data_envelope::Payload::LootContainerClose(close)) => {
                    assert_eq!(
                        close.session_id, 99,
                        "session_id should survive roundtrip for {reason:?}"
                    );
                    assert_eq!(
                        close.reason, expected_wire,
                        "expected close reason wire `{expected_wire}` for {reason:?}, actual `{}`",
                        close.reason
                    );
                }
                other => panic!("expected LootContainerClose payload, got {other:?}"),
            }
        }
    }

    #[test]
    fn c2s_external_container_move_roundtrip() {
        c2s_encode_decode_roundtrip(
            super::super::client_request::ClientRequestV1::ExternalContainerMove {
                v: 1,
                session_id: 42,
                instance_id: 100,
                from: super::super::inventory::InventoryLocationV1::Container {
                    container_id: "ext_42".to_string(),
                    row: 0,
                    col: 1,
                },
                to: super::super::inventory::InventoryLocationV1::Container {
                    container_id: "body_pocket".to_string(),
                    row: 1,
                    col: 0,
                },
            },
        );
    }

    #[test]
    fn c2s_external_container_close_roundtrip() {
        c2s_encode_decode_roundtrip(
            super::super::client_request::ClientRequestV1::ExternalContainerClose {
                v: 1,
                session_id: 99,
            },
        );
    }

    #[test]
    fn c2s_supply_coffin_open_roundtrip() {
        c2s_encode_decode_roundtrip(
            super::super::client_request::ClientRequestV1::SupplyCoffinOpen {
                v: 1,
                entity_id: 42,
            },
        );
    }

    #[test]
    fn c2s_supply_coffin_open_zero_entity_id_roundtrip() {
        c2s_encode_decode_roundtrip(
            super::super::client_request::ClientRequestV1::SupplyCoffinOpen { v: 1, entity_id: 0 },
        );
    }

    #[test]
    fn c2s_supply_coffin_open_max_entity_id_roundtrip() {
        c2s_encode_decode_roundtrip(
            super::super::client_request::ClientRequestV1::SupplyCoffinOpen {
                v: 1,
                entity_id: i32::MAX,
            },
        );
    }

    #[test]
    fn c2s_container_open_roundtrip() {
        c2s_encode_decode_roundtrip(
            super::super::client_request::ClientRequestV1::ContainerOpen {
                v: 1,
                entity_id: 42,
            },
        );
    }

    #[test]
    fn c2s_container_open_zero_entity_id_roundtrip() {
        c2s_encode_decode_roundtrip(
            super::super::client_request::ClientRequestV1::ContainerOpen { v: 1, entity_id: 0 },
        );
    }

    #[test]
    fn c2s_container_open_max_entity_id_roundtrip() {
        c2s_encode_decode_roundtrip(
            super::super::client_request::ClientRequestV1::ContainerOpen {
                v: 1,
                entity_id: i32::MAX,
            },
        );
    }

    #[test]
    fn c2s_container_open_proto_roundtrip_asserts_variant_and_entity_id() {
        use prost::Message;
        let req = super::super::client_request::ClientRequestV1::ContainerOpen {
            v: 1,
            entity_id: 165,
        };
        let proto_payload = bong::client_request_envelope::Payload::from(&req);
        let envelope = bong::ClientRequestEnvelope {
            payload: Some(proto_payload),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = bong::ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("C2S ContainerOpen proto decode should succeed");
        match decoded.payload {
            Some(bong::client_request_envelope::Payload::ContainerOpen(open)) => {
                assert_eq!(
                    open.entity_id, 165,
                    "ContainerOpen.entity_id should survive proto roundtrip"
                );
            }
            other => panic!("expected ContainerOpen payload, got {other:?}"),
        }
    }

    #[test]
    fn c2s_workbench_open_roundtrip() {
        c2s_encode_decode_roundtrip(
            super::super::client_request::ClientRequestV1::WorkbenchOpen {
                v: 1,
                entity_id: 42,
            },
        );
    }

    #[test]
    fn c2s_workbench_open_proto_roundtrip_asserts_variant_and_entity_id() {
        use prost::Message;
        let req = super::super::client_request::ClientRequestV1::WorkbenchOpen {
            v: 1,
            entity_id: 165,
        };
        let proto_payload = bong::client_request_envelope::Payload::from(&req);
        let envelope = bong::ClientRequestEnvelope {
            payload: Some(proto_payload),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = bong::ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("C2S WorkbenchOpen proto decode should succeed");
        match decoded.payload {
            Some(bong::client_request_envelope::Payload::WorkbenchOpen(open)) => {
                assert_eq!(
                    open.entity_id, 165,
                    "WorkbenchOpen.entity_id should survive proto roundtrip"
                );
            }
            other => panic!("expected WorkbenchOpen payload, got {other:?}"),
        }
    }

    // ─── plan-coffin-tiers-v1 P3：延寿棺 C2S proto roundtrip ────────────────

    #[test]
    fn c2s_coffin_break_roundtrip() {
        c2s_encode_decode_roundtrip(super::super::client_request::ClientRequestV1::CoffinBreak {
            v: 1,
            x: 10,
            y: 64,
            z: -5,
        });
    }

    /// CodeRabbit major D: oneof variant + 坐标逐字段保真断言（非零/负坐标）。
    #[test]
    fn c2s_coffin_break_proto_roundtrip_asserts_variant_and_coords() {
        use prost::Message;
        let req = super::super::client_request::ClientRequestV1::CoffinBreak {
            v: 1,
            x: 10,
            y: 64,
            z: -5,
        };
        let proto_payload = bong::client_request_envelope::Payload::from(&req);
        let envelope = bong::ClientRequestEnvelope {
            payload: Some(proto_payload),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = bong::ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("C2S CoffinBreak proto decode should succeed");
        match decoded.payload {
            Some(bong::client_request_envelope::Payload::CoffinBreak(b)) => {
                assert_eq!(
                    b.x, 10,
                    "CoffinBreak.x should survive roundtrip; expected 10, got {}",
                    b.x
                );
                assert_eq!(
                    b.y, 64,
                    "CoffinBreak.y should survive roundtrip; expected 64, got {}",
                    b.y
                );
                assert_eq!(
                    b.z, -5,
                    "CoffinBreak.z (negative) should survive roundtrip; expected -5, got {}",
                    b.z
                );
            }
            other => panic!(
                "expected CoffinBreak oneof variant after roundtrip, got {other:?}; \
                 confirms the payload is not silently mapped to another variant"
            ),
        }
    }

    #[test]
    fn c2s_coffin_menu_reclaim_roundtrip() {
        c2s_encode_decode_roundtrip(
            super::super::client_request::ClientRequestV1::CoffinMenuReclaim {
                v: 1,
                x: 3,
                y: 65,
                z: 7,
            },
        );
    }

    // ─── plan-exploration-probe-return-v1 P0/P1 C2S proto round-trip ─────────

    /// P0: MineralProbe C2S proto round-trip + 坐标逐字段保真（含负坐标边界）。
    #[test]
    fn c2s_mineral_probe_proto_roundtrip_asserts_variant_and_coords() {
        use prost::Message;
        let req = super::super::client_request::ClientRequestV1::MineralProbe {
            v: 1,
            x: -16,
            y: 48,
            z: 255,
        };
        let proto_payload = bong::client_request_envelope::Payload::from(&req);
        let envelope = bong::ClientRequestEnvelope {
            payload: Some(proto_payload),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = bong::ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("C2S MineralProbe proto decode should succeed");
        match decoded.payload {
            Some(bong::client_request_envelope::Payload::MineralProbe(r)) => {
                assert_eq!(
                    r.x, -16,
                    "MineralProbe.x (negative) should survive roundtrip; expected -16, got {}",
                    r.x
                );
                assert_eq!(
                    r.y, 48,
                    "MineralProbe.y should survive roundtrip; expected 48, got {}",
                    r.y
                );
                assert_eq!(
                    r.z, 255,
                    "MineralProbe.z should survive roundtrip; expected 255, got {}",
                    r.z
                );
            }
            other => panic!("expected MineralProbe oneof variant after roundtrip, got {other:?}"),
        }
    }

    /// P1: FreshnessProbe C2S proto round-trip + instance_id 逐字段保真（含边界值）。
    #[test]
    fn c2s_freshness_probe_proto_roundtrip_asserts_variant_and_instance_id() {
        use prost::Message;
        let req = super::super::client_request::ClientRequestV1::FreshnessProbe {
            v: 1,
            instance_id: 9_999_999_999u64,
        };
        let proto_payload = bong::client_request_envelope::Payload::from(&req);
        let envelope = bong::ClientRequestEnvelope {
            payload: Some(proto_payload),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = bong::ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("C2S FreshnessProbe proto decode should succeed");
        match decoded.payload {
            Some(bong::client_request_envelope::Payload::FreshnessProbe(r)) => {
                assert_eq!(
                    r.instance_id, 9_999_999_999u64,
                    "FreshnessProbe.instance_id large value should survive roundtrip; expected 9_999_999_999, got {}",
                    r.instance_id
                );
            }
            other => panic!("expected FreshnessProbe oneof variant after roundtrip, got {other:?}"),
        }
    }

    /// P1: FreshnessProbe instance_id=0 边界（最小合法值）。
    #[test]
    fn c2s_freshness_probe_zero_instance_id_roundtrip() {
        c2s_encode_decode_roundtrip(
            super::super::client_request::ClientRequestV1::FreshnessProbe {
                v: 1,
                instance_id: 0,
            },
        );
    }

    /// P0: MineralProbe basic round-trip（全走 helper）。
    #[test]
    fn c2s_mineral_probe_basic_roundtrip() {
        c2s_encode_decode_roundtrip(
            super::super::client_request::ClientRequestV1::MineralProbe {
                v: 1,
                x: 10,
                y: 64,
                z: -8,
            },
        );
    }

    /// CodeRabbit major D: oneof variant + 坐标逐字段保真断言（含负坐标）。
    #[test]
    fn c2s_coffin_menu_reclaim_proto_roundtrip_asserts_variant_and_coords() {
        use prost::Message;
        let req = super::super::client_request::ClientRequestV1::CoffinMenuReclaim {
            v: 1,
            x: -8,
            y: 65,
            z: 3,
        };
        let proto_payload = bong::client_request_envelope::Payload::from(&req);
        let envelope = bong::ClientRequestEnvelope {
            payload: Some(proto_payload),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = bong::ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("C2S CoffinMenuReclaim proto decode should succeed");
        match decoded.payload {
            Some(bong::client_request_envelope::Payload::CoffinMenuReclaim(r)) => {
                assert_eq!(
                    r.x, -8,
                    "CoffinMenuReclaim.x (negative) should survive roundtrip; expected -8, got {}",
                    r.x
                );
                assert_eq!(
                    r.y, 65,
                    "CoffinMenuReclaim.y should survive roundtrip; expected 65, got {}",
                    r.y
                );
                assert_eq!(
                    r.z, 3,
                    "CoffinMenuReclaim.z should survive roundtrip; expected 3, got {}",
                    r.z
                );
            }
            other => panic!(
                "expected CoffinMenuReclaim oneof variant after roundtrip, got {other:?}; \
                 confirms the payload is not silently mapped to another variant"
            ),
        }
    }

    // ─── plan-agent-ui-data-v1 P0：AgentUi JSON-bypass 协议契约 pin 测试 ───────

    /// AgentUiResponse 协议契约：JSON CustomPayload 路径有效，proto 路径不可达。
    ///
    /// 此测试锁死两条重要约束：
    /// 1. AgentUiResponsePayloadV1 JSON 正样本 roundtrip 正确（JSON 路径真实可用）。
    /// 2. C2S proto 转换函数中 AgentUiResponse 分支标记为 unreachable（检查代码注释+结构）。
    ///
    /// 保护目的：防止将来切换 proto 编码时无意触发此分支（footgun 消除 pin）。
    #[test]
    fn agent_ui_response_json_bypass_contract_pin() {
        use crate::schema::agent_ui::{AgentUiActionType, AgentUiResponsePayloadV1};
        use std::collections::HashMap;

        // JSON 路径正样本 roundtrip（验证 JSON bypass 路径真实可用）
        let response = AgentUiResponsePayloadV1 {
            request_id: "pin-test-req".to_string(),
            action: AgentUiActionType::ButtonClick,
            params: {
                let mut m = HashMap::new();
                m.insert("button_id".to_string(), "confirm".to_string());
                m
            },
        };
        let json =
            serde_json::to_string(&response).expect("AgentUiResponsePayloadV1 JSON 序列化不应失败");
        let decoded: AgentUiResponsePayloadV1 =
            serde_json::from_str(&json).expect("AgentUiResponsePayloadV1 JSON 反序列化不应失败");

        assert_eq!(
            decoded.request_id, "pin-test-req",
            "AgentUiResponsePayloadV1 request_id 应在 JSON roundtrip 后保持，期望 pin-test-req，实为 {}",
            decoded.request_id
        );
        assert_eq!(
            decoded.action,
            AgentUiActionType::ButtonClick,
            "AgentUiResponsePayloadV1 action 应在 JSON roundtrip 后保持 ButtonClick"
        );
        assert_eq!(
            decoded.params.get("button_id").map(String::as_str),
            Some("confirm"),
            "AgentUiResponsePayloadV1 params.button_id 应在 JSON roundtrip 后保持 confirm"
        );

        // S2C AgentUiRequest/Close 同样走 JSON bypass，不走 proto。
        // 以下验证 JSON bypass 路径对 AgentUiRequestPayloadV1 也正确。
        use crate::schema::agent_ui::AgentUiRequestPayloadV1;
        let request = AgentUiRequestPayloadV1 {
            request_id: "pin-req".to_string(),
            target_player: "pin-player".to_string(),
            xml: "<owo-ui><components><label>test</label></components></owo-ui>".to_string(),
            timeout_ticks: 600,
        };
        let json2 =
            serde_json::to_string(&request).expect("AgentUiRequestPayloadV1 JSON 序列化不应失败");
        let decoded2: AgentUiRequestPayloadV1 =
            serde_json::from_str(&json2).expect("AgentUiRequestPayloadV1 JSON 反序列化不应失败");
        assert_eq!(
            decoded2.timeout_ticks, 600,
            "AgentUiRequestPayloadV1 timeout_ticks 应在 JSON roundtrip 后保持 600"
        );
    }

    #[test]
    #[should_panic(expected = "AgentUiRequest/AgentUiClose 经由 JSON CustomPayload 发送")]
    fn agent_ui_request_panics_if_proto_path_is_used() {
        use crate::schema::agent_ui::AgentUiRequestPayloadV1;

        let payload = ServerDataPayloadV1::AgentUiRequest(AgentUiRequestPayloadV1 {
            request_id: "pin-req".to_string(),
            target_player: "offline:Kiz".to_string(),
            xml: "<owo-ui><components><label>test</label></components></owo-ui>".to_string(),
            timeout_ticks: 600,
        });
        let _: bong::server_data_envelope::Payload = (&payload).into();
    }

    #[test]
    #[should_panic(expected = "AgentUiRequest/AgentUiClose 经由 JSON CustomPayload 发送")]
    fn agent_ui_close_panics_if_proto_path_is_used() {
        use crate::schema::agent_ui::AgentUiClosePayloadV1;

        let payload = ServerDataPayloadV1::AgentUiClose(AgentUiClosePayloadV1 {
            request_id: "pin-req".to_string(),
            reason: Some("session_expired".to_string()),
        });
        let _: bong::server_data_envelope::Payload = (&payload).into();
    }

    #[test]
    #[should_panic(expected = "AgentUiResponse 经由 JSON CustomPayload 接收")]
    fn agent_ui_response_panics_if_proto_path_is_used() {
        use crate::schema::agent_ui::AgentUiActionType;
        use crate::schema::client_request::ClientRequestV1;

        let req = ClientRequestV1::AgentUiResponse {
            v: 1,
            request_id: "pin-req".to_string(),
            action: AgentUiActionType::ButtonClick,
            params: std::collections::HashMap::from([(
                "button_id".to_string(),
                "enter_realm".to_string(),
            )]),
        };
        let _: bong::client_request_envelope::Payload = (&req).into();
    }

    // ─── plan-test-coverage-guards-v1 P0：HalfStepRechallenge JSON-bypass pin ──

    /// HalfStepRechallenge is a JSON-bypass variant — it must never reach the proto path.
    ///
    /// MUTATION GUARD: If `HalfStepRechallenge` is accidentally removed from the JSON-bypass
    /// list (`is_json_bypass`) and treated as a proto variant without adding a proto arm,
    /// `to_proto_bytes()` will call the `From` impl which hits `unreachable!()` — this test
    /// will change from #[should_panic] to an actual panic in the wrong place. Keep this test
    /// pinned to detect such misclassifications.
    #[test]
    #[should_panic(expected = "HalfStepRechallenge 经由 JSON CustomPayload 发送")]
    fn halfstep_rechallenge_panics_if_proto_path_is_used() {
        use super::super::server_data::HalfStepRechallengeV1;

        let payload = ServerDataPayloadV1::HalfStepRechallenge(HalfStepRechallengeV1 {
            active: true,
            char_id: "offline:Kiz".to_string(),
            rechallenge_window_until: 50_000,
            at_tick: 1_000,
        });
        let _: bong::server_data_envelope::Payload = (&payload).into();
    }

    // ─── plan-test-coverage-guards-v1 P0：exhaustive proto encoding guard ────────

    /// Returns a minimum-viable fixture for every `ServerDataPayloadV1` variant (125 total).
    ///
    /// **Why a list and not an exhaustive match?**
    /// The list itself cannot be made compile-time exhaustive — Rust cannot iterate enum variants.
    /// Exhaustiveness is enforced by TWO complementary mechanisms:
    ///   1. `ServerDataPayloadV1::is_json_bypass(&self)` has NO `_` catch-all; adding a new
    ///      variant without updating it causes rustc E0004 (compile failure).
    ///   2. `s2c_fixture_count_matches_variant_count()` cross-checks the fixture list length
    ///      against the number of `ServerDataType` discriminants derived from `payload_type()`.
    ///
    /// Each element carries `(fixture, expected_bypass: bool)` so the guard test can route it
    /// either to the proto-encode assertion or to the should-panic branch.
    ///
    /// MUTATION GUARDS (documented for manual mutation testing):
    ///   - Delete a proto arm in `From<&ServerDataPayloadV1>` → the `From` match is exhaustive
    ///     with NO catch-all, so removing an arm is rustc E0004 (compile failure), caught even
    ///     earlier than a test. The `catch_unwind` in the guard test is defense-in-depth: if a
    ///     future refactor adds a `_ => unreachable!()` catch-all, a deleted arm would fall
    ///     through to it at runtime and `s2c_all_proto_variants_encode_without_panic` reds instead.
    ///   - Flip a bypass variant in `is_json_bypass` from `true` to `false` → guard calls
    ///     `to_proto_bytes()` → the `From` impl hits the bypass `unreachable!()` arm → RED.
    ///   - Add a new variant without updating `is_json_bypass` → rustc E0004 → compile failure.
    ///   - Add a new variant without adding a fixture here → `s2c_fixture_count_matches_variant_count` → RED.
    fn s2c_all_fixtures() -> Vec<(ServerDataPayloadV1, bool)> {
        use super::super::agent_ui::{AgentUiClosePayloadV1, AgentUiRequestPayloadV1};
        use super::super::alchemy::{
            AlchemyContaminationDataV1, AlchemyFurnaceDataV1, AlchemyOutcomeBucketV1,
            AlchemyOutcomeForecastDataV1, AlchemyOutcomeResolvedDataV1, AlchemyRecipeBookDataV1,
            AlchemySessionDataV1,
        };
        use super::super::combat_carrier::CarrierStateV1;
        use super::super::combat_hud::*;
        use super::super::craft::{
            CraftOutcomeV1, CraftSessionStateV1, RecipeListV1, RecipeUnlockedV1,
        };
        use super::super::cultivation::InsightOfferV1;
        use super::super::dugu::DuguPoisonStateV1;
        use super::super::forge::{
            ForgeBlueprintBookDataV1, ForgeOutcomeBucketV1, ForgeOutcomeDataV1, ForgeSessionDataV1,
            ForgeStepStateDataV1, ForgeStepV1, WeaponForgeStationDataV1,
        };
        use super::super::identity::IdentityPanelStateV1;
        use super::super::inventory::{EquippedInventorySnapshotV1, InventoryWeightV1};
        use super::super::lingtian::LingtianSessionDataV1;
        use super::super::movement::MovementStateV1;
        use super::super::movement::{MovementActionV1, MovementZoneKindV1};
        use super::super::poison_trait::{
            PoisonDoseEventV1, PoisonOverdoseEventV1, PoisonOverdoseSeverityV1,
            PoisonSideEffectTagV1, PoisonTraitStateV1,
        };
        use super::super::processing::FreshnessUpdateV1;
        use super::super::realm_vision::{
            FogShapeV1, RealmVisionParamsV1, SpiritualSenseTargetsV1,
        };
        use super::super::server_data::{
            AnqiHudV1, AscensionQuotaV1, BreakthroughCinematicS2cV1, BurstMeridianEventV1,
            CoffinStateV1, CombatEventFloaterV1, ContainerKindV1, ContainerStateV1,
            DuguV2HudQiDecayV1, DuguV2HudSelfCureV1, DuguV2HudShroudActiveV1, DuguV2HudSkillCastV1,
            ExtractAbortedReasonV1, ExtractAbortedV1, ExtractCompletedV1, ExtractFailedReasonV1,
            ExtractFailedV1, ExtractProgressV1, ExtractStartedV1, FactionWarStateV1,
            FullPowerChargingStateV1, FullPowerExhaustedStateV1, FullPowerReleaseV1,
            GatheringQualityHintV1, GatheringTargetTypeV1, HalfStepRechallengeV1,
            HeartDemonOfferV1, KnockbackSyncV1, LootContainerCloseReasonV1, LootContainerCloseV1,
            LootContainerOpenV1, LootContainerSourceKindV1, LootContainerUpdateV1,
            MineralProbeResultV1, PillBuffStatusV1, QiColorObservedV1, RiftPortalDirectionV1,
            RiftPortalKindV1, RiftPortalRemovedV1, RiftPortalStateV1, SearchAbortReasonV1,
            SearchAbortedV1, SearchCompletedV1, SearchProgressV1, SearchStartedV1,
            SwordBondHudStateV1, TechniqueProficiencyUpdateV1, TribulationBroadcastV1,
            TribulationStateV1, TsyCollapseStartedIpcV1, ZhenmaiHudV1,
        };
        use super::super::skill::{
            SkillCapChangedPayloadV1, SkillIdV1, SkillLvUpPayloadV1, SkillScrollUsedPayloadV1,
            SkillSnapshotPayloadV1, SkillXpGainPayloadV1, XpGainSourceV1,
        };
        use super::super::social::{
            ExposureKindV1, GuardianKindV1, NicheGuardianBrokenV1, NicheGuardianFatigueV1,
            NicheIntrusionEventV1, SocialAnonymityPayloadV1, SocialExposureEventV1,
            SocialFeudEventV1, SocialPactEventV1, SocialRenownDeltaV1, SparringInvitePayloadV1,
            TradeItemSummaryV1, TradeOfferPayloadV1,
        };
        use super::super::spirit_treasure::{
            SpiritTreasureDialoguePayloadV1, SpiritTreasureDialogueToneV1,
            SpiritTreasureDialogueV1, SpiritTreasureStatePayloadV1,
        };
        use super::super::tuike::FalseSkinStateV1;
        use super::super::woliu::VortexFieldStateV1;
        use super::super::world_state::PlayerPowerBreakdown;
        use super::super::yidao::{HealerNpcAiStateV1, YidaoHudStateV1};
        use crate::cultivation::components::ColorKind;
        use crate::skill::config::SkillConfigSnapshot;

        macro_rules! fix {
            ($v:expr) => {
                ($v, $v.is_json_bypass())
            };
        }

        // Note: the bypass field is derived from is_json_bypass() on each fixture, so any
        // misclassification in is_json_bypass() will cause the proto path to panic or the
        // should-panic branch to succeed unexpectedly.
        vec![
            fix!(ServerDataPayloadV1::Welcome {
                message: "hello".to_string()
            }),
            fix!(ServerDataPayloadV1::Heartbeat {
                message: "ping".to_string()
            }),
            fix!(ServerDataPayloadV1::Narration {
                narrations: vec![super::super::narration::Narration {
                    text: "test".to_string(),
                    scope: super::super::common::NarrationScope::Broadcast,
                    style: super::super::common::NarrationStyle::Narration,
                    kind: None,
                    target: None,
                }],
            }),
            fix!(ServerDataPayloadV1::ZoneInfo {
                zone: "spawn".to_string(),
                spirit_qi: 1.0,
                danger_level: 2,
                status: super::super::world_state::ZoneStatusV1::Normal,
                active_events: None,
                perception_text: None,
            }),
            fix!(ServerDataPayloadV1::EventAlert {
                event: super::super::common::EventKind::ThunderTribulation,
                message: "watch out".to_string(),
                zone: Some("spawn".to_string()),
                duration_ticks: Some(100),
            }),
            fix!(ServerDataPayloadV1::PlayerState {
                player: None,
                realm: "Induce".to_string(),
                spirit_qi: 1.0,
                spirit_qi_max: 1.0,
                karma: 0.0,
                composite_power: 1.0,
                breakdown: PlayerPowerBreakdown {
                    combat: 1.0,
                    wealth: 0.0,
                    social: 0.0,
                    karma: 0.0,
                    territory: 0.0,
                },
                zone: "spawn".to_string(),
                local_neg_pressure: None,
                season_state: None,
                social: None,
            }),
            fix!(ServerDataPayloadV1::CoffinState(CoffinStateV1 {
                in_coffin: false,
                lifespan_rate_multiplier: 1.0,
                coffin_grade: None,
            })),
            fix!(ServerDataPayloadV1::UiOpen {
                ui: None,
                xml: "<root/>".to_string(),
            }),
            fix!(ServerDataPayloadV1::CultivationDetail {
                realm: "Induce".to_string(),
                opened: vec![false; 20],
                flow_rate: vec![0.0; 20],
                flow_capacity: vec![10.0; 20],
                integrity: vec![1.0; 20],
                open_progress: vec![0.0; 20],
                cracks_count: vec![0; 20],
                contamination_total: 0.0,
                lifespan: None,
                recent_skill_milestones_summary: String::new(),
                skill_milestones: vec![],
                qi_color_main: ColorKind::Intricate,
                qi_color_secondary: None,
                qi_color_chaotic: false,
                qi_color_hunyuan: false,
                practice_weights: vec![],
                target_meridian: None,
            }),
            fix!(ServerDataPayloadV1::QiColorObserved(QiColorObservedV1 {
                observer: "offline:Kiz".to_string(),
                observed: "offline:Azure".to_string(),
                main: ColorKind::Intricate,
                secondary: None,
                is_chaotic: false,
                is_hunyuan: false,
                realm_diff: 0,
            })),
            fix!(ServerDataPayloadV1::InventorySnapshot(Box::new(
                super::super::inventory::InventorySnapshotV1 {
                    revision: 1,
                    containers: vec![],
                    placed_items: vec![],
                    equipped: EquippedInventorySnapshotV1::default(),
                    hotbar: vec![],
                    bone_coins: 0,
                    weight: InventoryWeightV1 {
                        current: 0.0,
                        max: 100.0
                    },
                    realm: "Induce".to_string(),
                    qi_current: 0.0,
                    qi_max: 100.0,
                    body_level: 0.0,
                }
            ))),
            fix!(ServerDataPayloadV1::InventoryEvent(Box::new(
                super::super::inventory::InventoryEventV1::StackChanged {
                    revision: 1,
                    instance_id: 42,
                    stack_count: 5,
                }
            ))),
            fix!(ServerDataPayloadV1::DroppedLootSync(vec![])),
            fix!(ServerDataPayloadV1::BotanyHarvestProgress {
                session_id: "ses:1".to_string(),
                target_id: "entity:1".to_string(),
                target_name: "草".to_string(),
                plant_kind: "herb".to_string(),
                mode: "standard".to_string(),
                progress: 0.5,
                auto_selectable: false,
                request_pending: false,
                interrupted: false,
                completed: false,
                detail: String::new(),
                hazard_hints: vec![],
                target_pos: None,
            }),
            fix!(ServerDataPayloadV1::BotanyPlantV2RenderProfiles(vec![])),
            fix!(ServerDataPayloadV1::MiningProgress {
                session_id: "ses:2".to_string(),
                ore_pos: [0, 64, 0],
                progress: 0.2,
                interrupted: false,
                completed: false,
            }),
            fix!(ServerDataPayloadV1::LumberProgress {
                session_id: "ses:3".to_string(),
                log_pos: [0, 64, 0],
                progress: 0.3,
                interrupted: false,
                completed: false,
                detail: String::new(),
            }),
            fix!(ServerDataPayloadV1::GatheringSession {
                session_id: "ses:4".to_string(),
                progress_ticks: 10,
                total_ticks: 40,
                target_name: "草".to_string(),
                target_type: GatheringTargetTypeV1::Herb,
                quality_hint: GatheringQualityHintV1::Normal,
                tool_used: None,
                interrupted: false,
                completed: false,
            }),
            fix!(ServerDataPayloadV1::BotanySkill {
                level: 1,
                xp: 0,
                xp_to_next_level: 100,
                auto_unlock_level: 3,
            }),
            fix!(ServerDataPayloadV1::AlchemyFurnace(Box::new(
                AlchemyFurnaceDataV1 {
                    pos: None,
                    tier: 1,
                    integrity: 1.0,
                    integrity_max: 1.0,
                    owner_name: String::new(),
                    has_session: false,
                }
            ))),
            fix!(ServerDataPayloadV1::AlchemySession(Box::new(
                AlchemySessionDataV1 {
                    recipe_id: None,
                    active: false,
                    elapsed_ticks: 0,
                    target_ticks: 0,
                    temp_current: 0.0,
                    temp_target: 0.0,
                    temp_band: 0.0,
                    qi_injected: 0.0,
                    qi_target: 0.0,
                    status_label: String::new(),
                    stages: vec![],
                    interventions_recent: vec![],
                }
            ))),
            fix!(ServerDataPayloadV1::AlchemyOutcomeForecast(Box::new(
                AlchemyOutcomeForecastDataV1 {
                    perfect_pct: 0.0,
                    good_pct: 0.0,
                    flawed_pct: 0.0,
                    waste_pct: 1.0,
                    explode_pct: 0.0,
                    perfect_note: String::new(),
                    good_note: String::new(),
                    flawed_note: String::new(),
                }
            ))),
            fix!(ServerDataPayloadV1::AlchemyOutcomeResolved(Box::new(
                AlchemyOutcomeResolvedDataV1 {
                    bucket: AlchemyOutcomeBucketV1::Waste,
                    recipe_id: None,
                    pill: None,
                    quality: None,
                    toxin_amount: None,
                    toxin_color: None,
                    qi_gain: None,
                    side_effect_tag: None,
                    flawed_path: false,
                    damage: None,
                    meridian_crack: None,
                }
            ))),
            fix!(ServerDataPayloadV1::AlchemyRecipeBook(Box::new(
                AlchemyRecipeBookDataV1 {
                    learned: vec![],
                    current_index: 0,
                }
            ))),
            fix!(ServerDataPayloadV1::AlchemyContamination(Box::new(
                AlchemyContaminationDataV1 {
                    levels: vec![],
                    metabolism_note: String::new(),
                }
            ))),
            fix!(ServerDataPayloadV1::CombatHudState(CombatHudStateV1 {
                hp_percent: 1.0,
                qi_percent: 1.0,
                stamina_percent: 1.0,
                derived: DerivedAttrFlagsV1::default(),
            })),
            fix!(ServerDataPayloadV1::WoundsSnapshot(WoundsSnapshotV1 {
                wounds: vec![]
            })),
            fix!(ServerDataPayloadV1::DefenseWindow(DefenseWindowV1 {
                duration_ms: 200,
                started_at_ms: 0,
                expires_at_ms: 200,
            })),
            fix!(ServerDataPayloadV1::CastSync(CastSyncV1 {
                phase: CastPhaseV1::Idle,
                slot: 0,
                duration_ms: 0,
                started_at_ms: 0,
                outcome: CastOutcomeV1::None,
            })),
            fix!(ServerDataPayloadV1::QuickSlotConfig(QuickSlotConfigV1 {
                slots: vec![None; 9],
                cooldown_until_ms: vec![0; 9],
            })),
            fix!(ServerDataPayloadV1::SkillBarConfig(SkillBarConfigV1 {
                slots: vec![None; 9],
                cooldown_until_ms: vec![0; 9],
            })),
            fix!(ServerDataPayloadV1::TechniquesSnapshot(
                TechniquesSnapshotV1 { entries: vec![] }
            )),
            fix!(ServerDataPayloadV1::SkillConfigSnapshot(
                SkillConfigSnapshot {
                    configs: Default::default(),
                }
            )),
            fix!(ServerDataPayloadV1::UnlocksSync(UnlocksSyncV1::default())),
            fix!(ServerDataPayloadV1::DerivedAttrsSync(DerivedAttrsSyncV1 {
                flying: false,
                flying_qi_remaining: 0.0,
                flying_force_descent_at_ms: 0,
                phasing: false,
                phasing_until_ms: 0,
                tribulation_locked: false,
                tribulation_stage: String::new(),
                throughput_peak_norm: 0.0,
                tuike_layers: 0,
                vortex_active: false,
            })),
            fix!(ServerDataPayloadV1::EventStreamPush(EventStreamPushV1 {
                channel: EventChannelV1::Combat,
                priority: EventPriorityV1::P1Important,
                source_tag: String::new(),
                text: "test".to_string(),
                color: 0,
                created_at_ms: 0,
            })),
            fix!(ServerDataPayloadV1::WeaponEquipped(WeaponEquippedV1 {
                slot: "main_hand".to_string(),
                weapon: None,
            })),
            fix!(ServerDataPayloadV1::WeaponBroken(WeaponBrokenV1 {
                instance_id: 1,
                template_id: "sword_iron".to_string(),
            })),
            fix!(ServerDataPayloadV1::ShieldBroken(ShieldBrokenV1 {
                instance_id: 2,
                template_id: "shield_wood".to_string(),
            })),
            fix!(ServerDataPayloadV1::ShieldBlockHit(ShieldBlockHitV1 {
                template_id: "shield_wood".to_string(),
            })),
            fix!(ServerDataPayloadV1::TreasureEquipped(TreasureEquippedV1 {
                slot: "treasure_belt_0".to_string(),
                treasure: None,
            })),
            fix!(ServerDataPayloadV1::VortexState(VortexFieldStateV1 {
                caster: "entity:1".to_string(),
                active: false,
                center: [0.0, 64.0, 0.0],
                radius: 1.5,
                delta: 0.25,
                env_qi_at_cast: 0.9,
                maintain_remaining_ticks: 0,
                intercepted_count: 0,
                active_skill_id: "woliu.hold".to_string(),
                charge_progress: 0.0,
                cooldown_until_ms: 0,
                backfire_level: String::new(),
                turbulence_radius: 1.0,
                turbulence_intensity: 0.0,
                turbulence_until_ms: 0,
            })),
            fix!(ServerDataPayloadV1::DuguPoisonState(DuguPoisonStateV1 {
                target: "offline:Kiz".to_string(),
                active: false,
                meridian_id: "lung".to_string(),
                attacker: "offline:Azure".to_string(),
                attached_at_tick: 0,
                poisoner_realm_tier: 1,
                loss_per_tick: 0.0,
                flow_capacity_after: 10.0,
                qi_max_after: 100.0,
                server_tick: 0,
            })),
            fix!(ServerDataPayloadV1::PoisonDoseEvent(PoisonDoseEventV1 {
                v: 1,
                player_entity_id: 1,
                dose_amount: 1.0,
                side_effect_tag: PoisonSideEffectTagV1::QiFocusDrift2h,
                poison_level_after: 1.0,
                digestion_after: 0.0,
                at_tick: 1,
            })),
            fix!(ServerDataPayloadV1::PoisonOverdoseEvent(
                PoisonOverdoseEventV1 {
                    v: 1,
                    player_entity_id: 1,
                    severity: PoisonOverdoseSeverityV1::Mild,
                    overflow: 1.0,
                    lifespan_penalty_years: 0.0,
                    micro_tear_probability: 0.0,
                    at_tick: 1,
                }
            )),
            fix!(ServerDataPayloadV1::PoisonTraitState(PoisonTraitStateV1 {
                v: 1,
                player_entity_id: 1,
                poison_toxicity: 0.0,
                digestion_current: 0.0,
                digestion_capacity: 100.0,
                toxicity_tier_unlocked: false,
            })),
            fix!(ServerDataPayloadV1::CarrierState(CarrierStateV1 {
                carrier: "none".to_string(),
                phase: super::super::combat_carrier::CarrierChargePhaseV1::Idle,
                progress: 0.0,
                sealed_qi: 0.0,
                sealed_qi_initial: 0.0,
                half_life_remaining_ticks: 0,
                item_instance_id: None,
            })),
            fix!(ServerDataPayloadV1::FalseSkinState(FalseSkinStateV1 {
                target_id: "offline:Kiz".to_string(),
                kind: None,
                layers_remaining: 0,
                contam_capacity_per_layer: 0.0,
                absorbed_contam: 0.0,
                equipped_at_tick: 0,
                layers: vec![],
            })),
            fix!(ServerDataPayloadV1::LingtianSession(Box::new(
                LingtianSessionDataV1 {
                    active: false,
                    kind: super::super::lingtian::LingtianSessionKindV1::Till,
                    pos: [0, 64, 0],
                    elapsed_ticks: 0,
                    target_ticks: 100,
                    plant_id: None,
                    source: None,
                    dye_contamination: None,
                    dye_contamination_warning: false,
                }
            ))),
            fix!(ServerDataPayloadV1::DeathScreen {
                visible: false,
                cause: "fall".to_string(),
                luck_remaining: 1.0,
                final_words: vec![],
                countdown_until_ms: 0,
                can_reincarnate: true,
                can_terminate: false,
                stage: None,
                death_number: None,
                zone_kind: None,
                lifespan: None,
                cinematic: None,
            }),
            fix!(ServerDataPayloadV1::TerminateScreen {
                visible: false,
                final_words: String::new(),
                epilogue: String::new(),
                archetype_suggestion: String::new(),
            }),
            fix!(ServerDataPayloadV1::RiftPortalState(RiftPortalStateV1 {
                entity_id: 1,
                kind: RiftPortalKindV1::MainRift,
                direction: RiftPortalDirectionV1::Exit,
                family_id: "tsy_01".to_string(),
                world_pos: [0.0, 64.0, 0.0],
                trigger_radius: 2.0,
                current_extract_ticks: 0,
                activation_window_end: None,
            })),
            fix!(ServerDataPayloadV1::RiftPortalRemoved(
                RiftPortalRemovedV1 { entity_id: 1 }
            )),
            fix!(ServerDataPayloadV1::ExtractStarted(ExtractStartedV1 {
                player_id: "offline:Kiz".to_string(),
                portal_entity_id: 1,
                portal_kind: RiftPortalKindV1::MainRift,
                required_ticks: 160,
                at_tick: 1,
            })),
            fix!(ServerDataPayloadV1::ExtractProgress(ExtractProgressV1 {
                player_id: "offline:Kiz".to_string(),
                portal_entity_id: 1,
                elapsed_ticks: 1,
                required_ticks: 160,
            })),
            fix!(ServerDataPayloadV1::ExtractCompleted(ExtractCompletedV1 {
                player_id: "offline:Kiz".to_string(),
                portal_kind: RiftPortalKindV1::MainRift,
                family_id: "tsy_01".to_string(),
                exit_world_pos: [0.0, 64.0, 0.0],
                at_tick: 161,
            })),
            fix!(ServerDataPayloadV1::ExtractAborted(ExtractAbortedV1 {
                player_id: "offline:Kiz".to_string(),
                reason: ExtractAbortedReasonV1::PortalOccupied,
            })),
            fix!(ServerDataPayloadV1::ExtractFailed(ExtractFailedV1 {
                player_id: "offline:Kiz".to_string(),
                reason: ExtractFailedReasonV1::SpiritQiDrained,
            })),
            fix!(ServerDataPayloadV1::TsyCollapseStartedIpc(
                TsyCollapseStartedIpcV1 {
                    family_id: "tsy_01".to_string(),
                    at_tick: 1,
                    remaining_ticks: 100,
                    collapse_tear_entity_ids: vec![],
                }
            )),
            fix!(ServerDataPayloadV1::ContainerState(ContainerStateV1 {
                entity_id: 1,
                kind: ContainerKindV1::StoragePouch,
                family_id: "tsy_01".to_string(),
                world_pos: [0.0, 64.0, 0.0],
                locked: None,
                depleted: false,
                searched_by_player_id: None,
            })),
            fix!(ServerDataPayloadV1::SearchStarted(SearchStartedV1 {
                player_id: "offline:Kiz".to_string(),
                container_entity_id: 1,
                required_ticks: 100,
                at_tick: 1,
            })),
            fix!(ServerDataPayloadV1::SearchProgress(SearchProgressV1 {
                player_id: "offline:Kiz".to_string(),
                container_entity_id: 1,
                elapsed_ticks: 10,
                required_ticks: 100,
            })),
            fix!(ServerDataPayloadV1::SearchCompleted(SearchCompletedV1 {
                player_id: "offline:Kiz".to_string(),
                container_entity_id: 1,
                family_id: "tsy_01".to_string(),
                loot_preview: vec![],
                at_tick: 101,
            })),
            fix!(ServerDataPayloadV1::SearchAborted(SearchAbortedV1 {
                player_id: "offline:Kiz".to_string(),
                container_entity_id: 1,
                reason: SearchAbortReasonV1::Cancelled,
                at_tick: 50,
            })),
            fix!(ServerDataPayloadV1::SkillXpGain(Box::new(
                SkillXpGainPayloadV1::new(
                    1,
                    SkillIdV1::Herbalism,
                    10,
                    XpGainSourceV1::Action {
                        plan_id: "p0".to_string(),
                        action: "harvest".to_string(),
                    },
                )
            ))),
            fix!(ServerDataPayloadV1::SkillLvUp(SkillLvUpPayloadV1::new(
                1,
                SkillIdV1::Herbalism,
                2
            ))),
            fix!(ServerDataPayloadV1::SkillCapChanged(
                SkillCapChangedPayloadV1::new(1, SkillIdV1::Herbalism, 5)
            )),
            fix!(ServerDataPayloadV1::SkillScrollUsed(Box::new(
                SkillScrollUsedPayloadV1::new(1, "scroll:herb:1", SkillIdV1::Herbalism, 50, false)
            ))),
            fix!(ServerDataPayloadV1::SkillSnapshot(Box::new(
                SkillSnapshotPayloadV1 {
                    v: 1,
                    char_id: 1,
                    skills: Default::default(),
                    consumed_scrolls: vec![],
                }
            ))),
            fix!(ServerDataPayloadV1::ForgeStation(Box::new(
                WeaponForgeStationDataV1 {
                    station_id: "forge:1".to_string(),
                    tier: 1,
                    integrity: 1.0,
                    owner_name: String::new(),
                    has_session: false,
                }
            ))),
            fix!(ServerDataPayloadV1::ForgeSession(Box::new(
                ForgeSessionDataV1 {
                    session_id: 1,
                    blueprint_id: "bp:sword".to_string(),
                    blueprint_name: "凡铁剑".to_string(),
                    active: false,
                    current_step: ForgeStepV1::Billet,
                    step_index: 0,
                    achieved_tier: 0,
                    step_state: ForgeStepStateDataV1::None,
                }
            ))),
            fix!(ServerDataPayloadV1::ForgeOutcome(Box::new(
                ForgeOutcomeDataV1 {
                    session_id: 1,
                    blueprint_id: "bp:sword".to_string(),
                    bucket: ForgeOutcomeBucketV1::Good,
                    weapon_item: None,
                    quality: 0.5,
                    color: None,
                    side_effects: vec![],
                    achieved_tier: 1,
                    flawed_path: false,
                }
            ))),
            fix!(ServerDataPayloadV1::ForgeBlueprintBook(Box::new(
                ForgeBlueprintBookDataV1 {
                    learned: vec![],
                    current_index: 0,
                }
            ))),
            fix!(ServerDataPayloadV1::TribulationState(
                TribulationStateV1::clear()
            )),
            fix!(ServerDataPayloadV1::TribulationBroadcast(
                TribulationBroadcastV1::clear()
            )),
            fix!(ServerDataPayloadV1::AscensionQuota(AscensionQuotaV1::new(
                0, 3
            ))),
            fix!(ServerDataPayloadV1::HeartDemonOffer(HeartDemonOfferV1 {
                offer_id: "hd:1".to_string(),
                trigger_id: "hd:1".to_string(),
                trigger_label: "心魔".to_string(),
                realm_label: "渡虚".to_string(),
                composure: 0.5,
                quota_remaining: 1,
                quota_total: 1,
                expires_at_ms: 9_999_999_999_999,
                choices: vec![],
            })),
            fix!(ServerDataPayloadV1::BurstMeridianEvent(
                BurstMeridianEventV1 {
                    skill: "beng_quan".to_string(),
                    caster: "offline:Kiz".to_string(),
                    target: None,
                    tick: 1,
                    overload_ratio: 1.0,
                    integrity_snapshot: 1.0,
                }
            )),
            fix!(ServerDataPayloadV1::BreakthroughCinematic(
                BreakthroughCinematicS2cV1 {
                    actor_id: "offline:Kiz".to_string(),
                    phase: "apex".to_string(),
                    phase_tick: 0,
                    phase_duration_ticks: 80,
                    realm_from: "Condense".to_string(),
                    realm_to: "Solidify".to_string(),
                    result: "success".to_string(),
                    interrupted: false,
                    world_pos: [0.0, 64.0, 0.0],
                    visible_radius_blocks: 64.0,
                    global: false,
                    distant_billboard: false,
                    particle_density: 1.0,
                    intensity: 1.0,
                    season_overlay: "adaptive".to_string(),
                    style: "golden_core".to_string(),
                    at_tick: 1,
                }
            )),
            fix!(ServerDataPayloadV1::FullPowerChargingState(
                FullPowerChargingStateV1 {
                    caster_uuid: "00000000-0000-0000-0000-000000000001".to_string(),
                    active: false,
                    qi_committed: 0.0,
                    target_qi: 100.0,
                    started_tick: 0,
                }
            )),
            fix!(ServerDataPayloadV1::FullPowerRelease(FullPowerReleaseV1 {
                caster_uuid: "00000000-0000-0000-0000-000000000001".to_string(),
                target_uuid: None,
                qi_released: 100.0,
                tick: 1,
                hit_position: None,
            })),
            fix!(ServerDataPayloadV1::FullPowerExhaustedState(
                FullPowerExhaustedStateV1 {
                    caster_uuid: "00000000-0000-0000-0000-000000000001".to_string(),
                    active: false,
                    started_tick: 0,
                    recovery_at_tick: 0,
                }
            )),
            fix!(ServerDataPayloadV1::SocialAnonymity(
                SocialAnonymityPayloadV1 {
                    viewer: "offline:Kiz".to_string(),
                    remotes: vec![],
                }
            )),
            fix!(ServerDataPayloadV1::SocialExposure(SocialExposureEventV1 {
                v: 1,
                actor: "char:kiz".to_string(),
                kind: ExposureKindV1::Chat,
                witnesses: vec![],
                tick: 1,
                zone: None,
            })),
            fix!(ServerDataPayloadV1::SocialPact(SocialPactEventV1 {
                v: 1,
                left: "char:a".to_string(),
                right: "char:b".to_string(),
                terms: "mutual".to_string(),
                tick: 1,
                broken: false,
            })),
            fix!(ServerDataPayloadV1::SocialFeud(SocialFeudEventV1 {
                v: 1,
                left: "char:a".to_string(),
                right: "char:b".to_string(),
                tick: 1,
                place: None,
            })),
            fix!(ServerDataPayloadV1::SocialRenownDelta(
                SocialRenownDeltaV1 {
                    v: 1,
                    char_id: "char:kiz".to_string(),
                    fame_delta: 1,
                    notoriety_delta: 0,
                    tags_added: vec![],
                    tick: 1,
                    reason: "pact".to_string(),
                }
            )),
            fix!(ServerDataPayloadV1::IdentityPanelState(
                IdentityPanelStateV1 {
                    active_identity_id: 0,
                    last_switch_tick: 0,
                    cooldown_remaining_ticks: 0,
                    identities: vec![],
                }
            )),
            fix!(ServerDataPayloadV1::NicheIntrusion(NicheIntrusionEventV1 {
                v: 1,
                niche_pos: [0, 64, 0],
                intruder_id: "char:thief".to_string(),
                items_taken: vec![],
                taint_delta: 0.0,
            })),
            fix!(ServerDataPayloadV1::NicheGuardianFatigue(
                NicheGuardianFatigueV1 {
                    v: 1,
                    guardian_kind: GuardianKindV1::Puppet,
                    charges_remaining: 1,
                }
            )),
            fix!(ServerDataPayloadV1::NicheGuardianBroken(
                NicheGuardianBrokenV1 {
                    v: 1,
                    guardian_kind: GuardianKindV1::Puppet,
                    intruder_id: "char:thief".to_string(),
                }
            )),
            fix!(ServerDataPayloadV1::SparringInvite(
                SparringInvitePayloadV1 {
                    invite_id: "inv:1".to_string(),
                    initiator: "char:a".to_string(),
                    target: "char:b".to_string(),
                    realm_band: "condense".to_string(),
                    breath_hint: "匀".to_string(),
                    terms: "standard".to_string(),
                    expires_at_ms: 9_999_999_999_999,
                }
            )),
            fix!(ServerDataPayloadV1::TradeOffer(TradeOfferPayloadV1 {
                offer_id: "trade:1".to_string(),
                initiator: "char:a".to_string(),
                target: "char:b".to_string(),
                offered_item: TradeItemSummaryV1 {
                    instance_id: 1,
                    item_id: "bone_coin".to_string(),
                    display_name: "骨币".to_string(),
                    stack_count: 10,
                },
                requested_items: vec![],
                expires_at_ms: 9_999_999_999_999,
            })),
            fix!(ServerDataPayloadV1::RealmVisionParams(
                RealmVisionParamsV1 {
                    fog_start: 30.0,
                    fog_end: 60.0,
                    fog_color_rgb: 0,
                    fog_shape: FogShapeV1::Cylinder,
                    vignette_alpha: 0.0,
                    tint_color_argb: 0,
                    particle_density: 0.0,
                    transition_ticks: 0,
                    server_view_distance_chunks: 4,
                    post_fx_sharpen: 0.0,
                }
            )),
            fix!(ServerDataPayloadV1::SpiritualSenseTargets(
                SpiritualSenseTargetsV1 {
                    generation: 0,
                    entries: vec![],
                }
            )),
            fix!(ServerDataPayloadV1::HealerNpcAiState(HealerNpcAiStateV1 {
                healer_id: "npc:doc".to_string(),
                active_action: "idle".to_string(),
                queue_len: 0,
                reputation: 0,
                retreating: false,
            })),
            fix!(ServerDataPayloadV1::YidaoHudState(YidaoHudStateV1 {
                healer_id: "npc:doc".to_string(),
                reputation: 0,
                peace_mastery: 0.0,
                karma: 0.0,
                active_skill: None,
                patient_ids: vec![],
                patient_hp_percent: None,
                patient_contam_total: None,
                severed_meridian_count: 0,
                contract_count: 0,
                mass_preview_count: 0,
            })),
            fix!(ServerDataPayloadV1::MovementState(MovementStateV1 {
                current_speed_multiplier: 1.0,
                stamina_cost_active: false,
                movement_action: MovementActionV1::None,
                zone_kind: MovementZoneKindV1::Normal,
                dash_cooldown_remaining_ticks: 0,
                hitbox_height_blocks: 1.8,
                stamina_current: 100.0,
                stamina_max: 100.0,
                low_stamina: false,
                last_action_tick: None,
                rejected_action: None,
            })),
            fix!(ServerDataPayloadV1::SpiritTreasureState(
                SpiritTreasureStatePayloadV1 { treasures: vec![] }
            )),
            fix!(ServerDataPayloadV1::SpiritTreasureDialogue(
                SpiritTreasureDialoguePayloadV1 {
                    dialogue: SpiritTreasureDialogueV1 {
                        v: 1,
                        request_id: "req:1".to_string(),
                        character_id: "offline:Kiz".to_string(),
                        treasure_id: "jade_pendant".to_string(),
                        text: "汝何求".to_string(),
                        tone: SpiritTreasureDialogueToneV1::Cold,
                        affinity_delta: 0.0,
                    },
                    display_name: "翠玉佩".to_string(),
                    zone: "spawn".to_string(),
                }
            )),
            fix!(ServerDataPayloadV1::CraftRecipeList(Box::new(
                RecipeListV1 {
                    v: 1,
                    player_id: "offline:Kiz".to_string(),
                    recipes: vec![],
                    ts: 0,
                }
            ))),
            fix!(ServerDataPayloadV1::CraftSessionState(
                CraftSessionStateV1 {
                    v: 1,
                    player_id: "offline:Kiz".to_string(),
                    active: false,
                    recipe_id: None,
                    elapsed_ticks: 0,
                    total_ticks: 0,
                    completed_count: 0,
                    total_count: 0,
                    ts: 0,
                }
            )),
            fix!(ServerDataPayloadV1::CraftOutcome(
                CraftOutcomeV1::Completed {
                    v: 1,
                    player_id: "offline:Kiz".to_string(),
                    recipe_id: "craft.example".to_string(),
                    output_template: "example_item".to_string(),
                    output_count: 1,
                    completed_at_tick: 1,
                    ts: 0,
                }
            )),
            fix!(ServerDataPayloadV1::RecipeUnlocked(RecipeUnlockedV1 {
                v: 1,
                player_id: "offline:Kiz".to_string(),
                recipe_id: "craft.example".to_string(),
                source: super::super::craft::UnlockEventSourceV1::Insight {
                    trigger: super::super::craft::InsightTriggerV1::NearDeath,
                },
                unlocked_at_tick: 1,
                ts: 0,
            })),
            fix!(ServerDataPayloadV1::WorkbenchOpen {
                entity_id: 1,
                position: [0, 64, 0],
            }),
            fix!(ServerDataPayloadV1::CombatEventFloater(
                CombatEventFloaterV1 { events: vec![] }
            )),
            fix!(ServerDataPayloadV1::KnockbackSync(KnockbackSyncV1 {
                distance_blocks: 1.0,
                velocity_blocks_per_tick: 0.2,
                duration_ticks: 5,
                kinetic_energy: 5.0,
                collision_damage: None,
                chain_depth: 0,
                block_broken: false,
            })),
            fix!(ServerDataPayloadV1::TechniqueProficiencyUpdate(
                TechniqueProficiencyUpdateV1 {
                    technique_id: "sword.cleave".to_string(),
                    proficiency: 0.1,
                    gain: 0.001,
                }
            )),
            fix!(ServerDataPayloadV1::PillBuffStatus(PillBuffStatusV1 {
                buff_id: "huo_xue_dan".to_string(),
                remaining_ticks: 100,
                effect_multiplier: 1.0,
            })),
            fix!(ServerDataPayloadV1::LootContainerOpen(
                LootContainerOpenV1 {
                    session_id: 1,
                    source_kind: LootContainerSourceKindV1::SupplyCoffin {
                        grade: "common".to_string(),
                    },
                    rows: 3,
                    cols: 4,
                    placed_items: vec![],
                    timeout_wall_secs: 9_999_999,
                }
            )),
            fix!(ServerDataPayloadV1::LootContainerUpdate(
                LootContainerUpdateV1 {
                    session_id: 1,
                    placed_items: vec![],
                }
            )),
            fix!(ServerDataPayloadV1::LootContainerClose(
                LootContainerCloseV1 {
                    session_id: 1,
                    reason: LootContainerCloseReasonV1::Timeout,
                }
            )),
            fix!(ServerDataPayloadV1::FactionWarState(FactionWarStateV1 {
                war_id: 1,
                zone: "spawn".to_string(),
                region_descriptor: "东部荒野".to_string(),
                phase: "emerging".to_string(),
                groups: vec![],
                enlist_count: 0,
                mercenary_count: 0,
                intercept_count: 0,
                spectate_count: 0,
                winner_group: None,
                loser_group: None,
            })),
            fix!(ServerDataPayloadV1::AnqiHud(AnqiHudV1 {
                kind: "echo".to_string(),
                echo_count: 0,
                aim_progress: 0.0,
                charge_progress: 0.0,
                abrasion_container: "none".to_string(),
                abrasion_qi_payload: 0.0,
                tick: 1,
            })),
            fix!(ServerDataPayloadV1::DuguV2SkillCast(DuguV2HudSkillCastV1 {
                kind: "eclipse".to_string(),
                caster: "offline:Kiz".to_string(),
                target: None,
                taint_tier: None,
                reveal_probability: 0.0,
                tick: 1,
            })),
            fix!(ServerDataPayloadV1::DuguV2SelfCure(DuguV2HudSelfCureV1 {
                caster: "offline:Kiz".to_string(),
                gain_percent: 0.0,
                self_revealed: false,
                tick: 1,
            })),
            fix!(ServerDataPayloadV1::DuguV2ShroudActive(
                DuguV2HudShroudActiveV1 {
                    caster: "offline:Kiz".to_string(),
                    strength: 0.0,
                    expires_at_tick: 0,
                    tick: 1,
                }
            )),
            fix!(ServerDataPayloadV1::PermanentQiMaxDecayApplied(
                DuguV2HudQiDecayV1 {
                    target: "offline:Kiz".to_string(),
                    loss: 1.0,
                    qi_max_after: 99.0,
                    tick: 1,
                }
            )),
            fix!(ServerDataPayloadV1::SwordBondHudState(
                SwordBondHudStateV1 {
                    active: false,
                    grade_index: 0,
                    grade_name: "凡铁".to_string(),
                    stored_qi_ratio: 0.0,
                    bond_strength: 0.0,
                    heaven_gate_ready: false,
                }
            )),
            fix!(ServerDataPayloadV1::ZhenmaiHud(ZhenmaiHudV1 {
                skill_id: "sever_chain".to_string(),
                meridian_id: "Heart".to_string(),
                contam_removed: 0.0,
                remaining_points: 0,
                damage_reduction: 0.0,
                k_drain: 1.5,
                duration_ms: 60_000,
                tick: 1,
            })),
            fix!(ServerDataPayloadV1::MineralProbeResult(
                MineralProbeResultV1 {
                    kind: "denied".to_string(),
                    mineral_id: None,
                    remaining_units: None,
                    display_name_zh: None,
                    denial_reason: Some("no_skill".to_string()),
                }
            )),
            fix!(ServerDataPayloadV1::FreshnessUpdate(FreshnessUpdateV1 {
                item_uuid: "42".to_string(),
                freshness: 1.0,
                profile_name: "default".to_string(),
            })),
            fix!(ServerDataPayloadV1::InsightOffer(InsightOfferV1 {
                offer_id: "ins:1".to_string(),
                trigger_id: "ins:1".to_string(),
                character_id: "offline:Kiz".to_string(),
                choices: vec![],
            })),
            // ─── JSON-bypass variants (3) — is_json_bypass() returns true ───────────
            fix!(ServerDataPayloadV1::AgentUiRequest(
                AgentUiRequestPayloadV1 {
                    request_id: "req:1".to_string(),
                    target_player: "offline:Kiz".to_string(),
                    xml: "<owo-ui/>".to_string(),
                    timeout_ticks: 600,
                }
            )),
            fix!(ServerDataPayloadV1::AgentUiClose(AgentUiClosePayloadV1 {
                request_id: "req:1".to_string(),
                reason: None,
            })),
            fix!(ServerDataPayloadV1::HalfStepRechallenge(
                HalfStepRechallengeV1 {
                    active: false,
                    char_id: "offline:Kiz".to_string(),
                    rechallenge_window_until: 0,
                    at_tick: 0,
                }
            )),
        ]
    }

    /// Verifies that the fixture list covers every `ServerDataPayloadV1` variant (125 total).
    ///
    /// This cross-checks the fixture count against `ServerDataType` discriminant count derived
    /// from `payload_type()`. If a new variant is added and a fixture is not added to
    /// `s2c_all_fixtures()`, this test will fail with a clear count mismatch message.
    ///
    /// NOTE: This test uses the `ServerDataType` enum variant count as the ground truth.
    /// That count is maintained by `payload_type()` which is also an exhaustive match.
    #[test]
    fn s2c_fixture_count_matches_variant_count() {
        use super::super::server_data::ServerDataType;
        use std::collections::HashSet;
        let fixtures = s2c_all_fixtures();
        // Collect all discriminants from the fixture list
        let fixture_discriminants: HashSet<ServerDataType> =
            fixtures.iter().map(|(f, _)| f.payload_type()).collect();

        // The authoritative variant count is determined by exhaustively listing all ServerDataType variants.
        // This list must be kept in sync with the ServerDataType enum.
        // If a new variant is added to ServerDataType but not here, this test will red on the
        // discriminant mismatch below — not here. The point of THIS assertion is to catch
        // duplicates (same variant appearing twice in fixtures, reducing coverage).
        let all_types: Vec<ServerDataType> = vec![
            ServerDataType::Welcome,
            ServerDataType::Heartbeat,
            ServerDataType::Narration,
            ServerDataType::ZoneInfo,
            ServerDataType::EventAlert,
            ServerDataType::PlayerState,
            ServerDataType::CoffinState,
            ServerDataType::UiOpen,
            ServerDataType::CultivationDetail,
            ServerDataType::QiColorObserved,
            ServerDataType::InventorySnapshot,
            ServerDataType::InventoryEvent,
            ServerDataType::DroppedLootSync,
            ServerDataType::BotanyHarvestProgress,
            ServerDataType::BotanyPlantV2RenderProfiles,
            ServerDataType::MiningProgress,
            ServerDataType::LumberProgress,
            ServerDataType::GatheringSession,
            ServerDataType::BotanySkill,
            ServerDataType::AlchemyFurnace,
            ServerDataType::AlchemySession,
            ServerDataType::AlchemyOutcomeForecast,
            ServerDataType::AlchemyOutcomeResolved,
            ServerDataType::AlchemyRecipeBook,
            ServerDataType::AlchemyContamination,
            ServerDataType::CombatHudState,
            ServerDataType::WoundsSnapshot,
            ServerDataType::DefenseWindow,
            ServerDataType::CastSync,
            ServerDataType::QuickSlotConfig,
            ServerDataType::SkillBarConfig,
            ServerDataType::TechniquesSnapshot,
            ServerDataType::SkillConfigSnapshot,
            ServerDataType::UnlocksSync,
            ServerDataType::DerivedAttrsSync,
            ServerDataType::EventStreamPush,
            ServerDataType::WeaponEquipped,
            ServerDataType::WeaponBroken,
            ServerDataType::ShieldBroken,
            ServerDataType::ShieldBlockHit,
            ServerDataType::TreasureEquipped,
            ServerDataType::VortexState,
            ServerDataType::DuguPoisonState,
            ServerDataType::PoisonDoseEvent,
            ServerDataType::PoisonOverdoseEvent,
            ServerDataType::PoisonTraitState,
            ServerDataType::CarrierState,
            ServerDataType::FalseSkinState,
            ServerDataType::LingtianSession,
            ServerDataType::DeathScreen,
            ServerDataType::TerminateScreen,
            ServerDataType::RiftPortalState,
            ServerDataType::RiftPortalRemoved,
            ServerDataType::ExtractStarted,
            ServerDataType::ExtractProgress,
            ServerDataType::ExtractCompleted,
            ServerDataType::ExtractAborted,
            ServerDataType::ExtractFailed,
            ServerDataType::TsyCollapseStartedIpc,
            ServerDataType::ContainerState,
            ServerDataType::SearchStarted,
            ServerDataType::SearchProgress,
            ServerDataType::SearchCompleted,
            ServerDataType::SearchAborted,
            ServerDataType::SkillXpGain,
            ServerDataType::SkillLvUp,
            ServerDataType::SkillCapChanged,
            ServerDataType::SkillScrollUsed,
            ServerDataType::SkillSnapshot,
            ServerDataType::ForgeStation,
            ServerDataType::ForgeSession,
            ServerDataType::ForgeOutcome,
            ServerDataType::ForgeBlueprintBook,
            ServerDataType::TribulationState,
            ServerDataType::TribulationBroadcast,
            ServerDataType::AscensionQuota,
            ServerDataType::HeartDemonOffer,
            ServerDataType::BurstMeridianEvent,
            ServerDataType::BreakthroughCinematic,
            ServerDataType::FullPowerChargingState,
            ServerDataType::FullPowerRelease,
            ServerDataType::FullPowerExhaustedState,
            ServerDataType::SocialAnonymity,
            ServerDataType::SocialExposure,
            ServerDataType::SocialPact,
            ServerDataType::SocialFeud,
            ServerDataType::SocialRenownDelta,
            ServerDataType::IdentityPanelState,
            ServerDataType::NicheIntrusion,
            ServerDataType::NicheGuardianFatigue,
            ServerDataType::NicheGuardianBroken,
            ServerDataType::SparringInvite,
            ServerDataType::TradeOffer,
            ServerDataType::RealmVisionParams,
            ServerDataType::SpiritualSenseTargets,
            ServerDataType::HealerNpcAiState,
            ServerDataType::YidaoHudState,
            ServerDataType::MovementState,
            ServerDataType::SpiritTreasureState,
            ServerDataType::SpiritTreasureDialogue,
            ServerDataType::CraftRecipeList,
            ServerDataType::CraftSessionState,
            ServerDataType::CraftOutcome,
            ServerDataType::RecipeUnlocked,
            ServerDataType::WorkbenchOpen,
            ServerDataType::CombatEventFloater,
            ServerDataType::KnockbackSync,
            ServerDataType::TechniqueProficiencyUpdate,
            ServerDataType::PillBuffStatus,
            ServerDataType::LootContainerOpen,
            ServerDataType::LootContainerUpdate,
            ServerDataType::LootContainerClose,
            ServerDataType::FactionWarState,
            ServerDataType::AnqiHud,
            ServerDataType::DuguV2SkillCast,
            ServerDataType::DuguV2SelfCure,
            ServerDataType::DuguV2ShroudActive,
            ServerDataType::PermanentQiMaxDecayApplied,
            ServerDataType::SwordBondHudState,
            ServerDataType::ZhenmaiHud,
            ServerDataType::MineralProbeResult,
            ServerDataType::FreshnessUpdate,
            ServerDataType::InsightOffer,
            ServerDataType::AgentUiRequest,
            ServerDataType::AgentUiClose,
            ServerDataType::HalfStepRechallenge,
        ];

        let total_variants = all_types.len();
        assert_eq!(
            fixtures.len(),
            total_variants,
            "Fixture list has {} entries but ServerDataType has {} variants. \
             Add a fixture for every new variant in s2c_all_fixtures(). \
             Current fixture discriminants: {:?}",
            fixtures.len(),
            total_variants,
            fixture_discriminants.iter().collect::<Vec<_>>()
        );

        let all_types_set: HashSet<ServerDataType> = all_types.into_iter().collect();
        let missing: Vec<&ServerDataType> = all_types_set
            .iter()
            .filter(|t| !fixture_discriminants.contains(t))
            .collect();
        assert!(
            missing.is_empty(),
            "These ServerDataType variants are missing from the fixture list: {:?}. \
             Add a fixture for each in s2c_all_fixtures().",
            missing
        );

        let extra: Vec<ServerDataType> = fixture_discriminants
            .iter()
            .filter(|t| !all_types_set.contains(t))
            .cloned()
            .collect();
        assert!(
            extra.is_empty(),
            "These discriminants appear in fixtures but not in the authoritative all_types list: {:?}. \
             Update the all_types list in this test.",
            extra
        );
    }

    /// Exhaustive proto encoding guard for all 126 `ServerDataPayloadV1` variants.
    ///
    /// For each of the 123 proto-encodable variants (is_json_bypass=false):
    ///   - Calls `ServerDataV1::new(variant).to_proto_bytes()`.
    ///   - Asserts the bytes are non-empty (proto envelope was built).
    ///   - Decodes and asserts the envelope contains a payload (proto arm exists in From impl).
    ///
    /// For each of the 3 JSON-bypass variants (is_json_bypass=true):
    ///   - Asserts that `to_proto_bytes()` panics with the expected `unreachable!()` message.
    ///   - This is done via `std::panic::catch_unwind` to avoid aborting the test process.
    ///
    /// MUTATION GUARDS:
    ///   - Delete any proto arm in `From<&ServerDataPayloadV1>` → `to_proto_bytes()` calls
    ///     `unreachable!()` → this test panics with a clear variant name → RED.
    ///   - Flip a bypass variant in `is_json_bypass` from `true` to `false` → this test will
    ///     try to call `to_proto_bytes()` and hit `unreachable!()` → RED.
    ///   - Correctly labeled new bypass variant (true) → `catch_unwind` catches the panic → GREEN.
    #[test]
    fn s2c_all_proto_variants_encode_without_panic() {
        use super::super::server_data::ServerDataV1;
        use prost::Message;

        let fixtures = s2c_all_fixtures();
        let mut proto_count = 0usize;
        let mut bypass_count = 0usize;

        for (variant, is_bypass) in fixtures {
            let type_name = format!("{:?}", variant.payload_type());

            if is_bypass {
                // Bypass variants must panic on the proto path — verify with catch_unwind.
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let envelope = ServerDataV1::new(variant);
                    let _ = envelope.to_proto_bytes();
                }));
                assert!(
                    result.is_err(),
                    "Bypass variant {type_name} should panic on to_proto_bytes() because it is \
                     a JSON-bypass variant with no proto definition. If this assertion fails, \
                     either is_json_bypass() is misclassified (should be true) or a proto arm \
                     was accidentally added."
                );
                bypass_count += 1;
            } else {
                // Proto-encodable variant: must encode to non-empty bytes and decode successfully.
                let envelope = ServerDataV1::new(variant);
                let bytes = envelope.to_proto_bytes();
                assert!(
                    !bytes.is_empty(),
                    "Proto-encodable variant {type_name} produced empty bytes from to_proto_bytes(). \
                     This indicates the proto arm in From<&ServerDataPayloadV1> is returning an \
                     empty payload or the encode step is broken."
                );
                let decoded = bong::ServerDataEnvelope::decode(bytes.as_slice())
                    .unwrap_or_else(|e| panic!(
                        "Proto-encodable variant {type_name} produced bytes that failed to decode: {e}. \
                         The proto arm may be constructing a malformed proto message."
                    ));
                assert!(
                    decoded.payload.is_some(),
                    "Proto-encodable variant {type_name} decoded to an envelope with no payload. \
                     The proto arm in From<&ServerDataPayloadV1> may be returning an empty oneof."
                );
                proto_count += 1;
            }
        }

        assert_eq!(
            proto_count, 123,
            "Expected 123 proto-encodable S2C variants, got {proto_count}. \
             The fixture list or is_json_bypass classification may have changed."
        );
        assert_eq!(
            bypass_count, 3,
            "Expected 3 JSON-bypass S2C variants, got {bypass_count}. \
             Check is_json_bypass() and s2c_all_fixtures()."
        );
    }

    // ─── plan-test-coverage-guards-v1 P0：C2S exhaustive proto encoding guard ──

    /// Returns a minimum-viable fixture for every `ClientRequestV1` variant (99 total).
    ///
    /// The same exhaustiveness strategy as `s2c_all_fixtures()` applies:
    ///   - No compile-time list exhaustiveness, but `c2s_fixture_count_matches_variant_count()`
    ///     cross-checks list length against a manually maintained variant set.
    ///   - `AgentUiResponse` is the only C2S JSON-bypass variant (is_c2s_json_bypass=true).
    ///
    /// MUTATION GUARDS (see `c2s_all_proto_variants_encode_without_panic`):
    ///   - Delete a proto arm → fixture hits `unreachable!()` → test panics → RED.
    ///   - Add a new C2S variant without fixture → count assertion → RED.
    fn c2s_all_fixtures() -> Vec<(super::super::client_request::ClientRequestV1, bool)> {
        use super::super::agent_ui::AgentUiActionType;
        use super::super::client_request::{
            AnqiCarrierSlotV1, ApplyPillTargetV1, ClientRequestV1, SkillBarBindingV1,
        };
        use super::super::inventory::InventoryLocationV1;
        use super::super::movement::MovementActionRequestV1;
        use super::super::void_actions::VoidActionRequestV1;
        use crate::cultivation::components::MeridianId;
        use crate::cultivation::forging::ForgeAxis;
        use crate::cultivation::void::components::BarrierGeometry;
        use crate::schema::inventory::EquipSlotV1;
        use crate::zhenfa::trap_content::TrapTargetFace;
        use crate::zhenfa::{ZhenfaCarrierKind, ZhenfaDisarmMode, ZhenfaKind};

        // `is_c2s_bypass(req)` — returns true only for AgentUiResponse (the sole C2S bypass).
        // There is no `const fn is_c2s_json_bypass` on ClientRequestV1 because there is only
        // 1 bypass variant, and a separate exhaustive fn would duplicate maintenance burden.
        // Instead, we track it via a closure here.
        let is_bypass =
            |req: &ClientRequestV1| matches!(req, ClientRequestV1::AgentUiResponse { .. });

        let build = |req: ClientRequestV1| -> (ClientRequestV1, bool) {
            let b = is_bypass(&req);
            (req, b)
        };

        vec![
            build(ClientRequestV1::SetMeridianTarget {
                v: 1,
                meridian: MeridianId::Lung,
            }),
            build(ClientRequestV1::BreakthroughRequest { v: 1 }),
            build(ClientRequestV1::StartDuXu { v: 1 }),
            build(ClientRequestV1::VoidAction {
                v: 1,
                request: VoidActionRequestV1::Barrier {
                    zone_id: "spawn".to_string(),
                    geometry: BarrierGeometry::Circle {
                        center: [0.0, 64.0, 0.0],
                        radius: 5.0,
                    },
                },
            }),
            build(ClientRequestV1::MovementAction {
                v: 1,
                action: MovementActionRequestV1::Dash,
                yaw_degrees: None,
            }),
            build(ClientRequestV1::AbortTribulation { v: 1 }),
            build(ClientRequestV1::HeartDemonDecision {
                v: 1,
                choice_idx: None,
            }),
            build(ClientRequestV1::ForgeRequest {
                v: 1,
                meridian: MeridianId::Lung,
                axis: ForgeAxis::Rate,
            }),
            build(ClientRequestV1::InsightDecision {
                v: 1,
                trigger_id: "ins:1".to_string(),
                choice_idx: None,
            }),
            build(ClientRequestV1::BotanyHarvestRequest {
                v: 1,
                session_id: "ses:1".to_string(),
                mode: super::super::botany::BotanyHarvestModeV1::Manual,
            }),
            build(ClientRequestV1::AlchemyOpenFurnace {
                v: 1,
                furnace_pos: (0, 64, 0),
            }),
            build(ClientRequestV1::AlchemyFeedSlot {
                v: 1,
                furnace_pos: (0, 64, 0),
                slot_idx: 0,
                material: "herb_a".to_string(),
                count: 1,
            }),
            build(ClientRequestV1::AlchemyTakeBack {
                v: 1,
                furnace_pos: (0, 64, 0),
                slot_idx: 0,
            }),
            build(ClientRequestV1::AlchemyIgnite {
                v: 1,
                furnace_pos: (0, 64, 0),
                recipe_id: "recipe:1".to_string(),
            }),
            build(ClientRequestV1::AlchemyIntervention {
                v: 1,
                furnace_pos: (0, 64, 0),
                intervention: super::super::alchemy::AlchemyInterventionV1::AdjustTemp {
                    temp: 100.0,
                },
            }),
            build(ClientRequestV1::AlchemyTurnPage { v: 1, delta: 1 }),
            build(ClientRequestV1::AlchemyLearnRecipe {
                v: 1,
                recipe_id: "recipe:1".to_string(),
            }),
            build(ClientRequestV1::AlchemyLearnRecipeFragment {
                v: 1,
                item_instance_id: 1,
            }),
            build(ClientRequestV1::AlchemyTakePill {
                v: 1,
                pill_item_id: "pill:1".to_string(),
            }),
            build(ClientRequestV1::AlchemyFurnacePlace {
                v: 1,
                x: 0,
                y: 64,
                z: 0,
                item_instance_id: 1,
            }),
            build(ClientRequestV1::CoffinOpen {
                v: 1,
                x: 0,
                y: 64,
                z: 0,
            }),
            build(ClientRequestV1::CoffinPlace {
                v: 1,
                x: 0,
                y: 64,
                z: 0,
                item_instance_id: 1,
            }),
            build(ClientRequestV1::BlockPlace {
                v: 1,
                x: 0,
                y: 64,
                z: 0,
                item_instance_id: 1,
                target_face: TrapTargetFace::Top,
            }),
            build(ClientRequestV1::CoffinEnter {
                v: 1,
                x: 0,
                y: 64,
                z: 0,
            }),
            build(ClientRequestV1::CoffinLeave { v: 1 }),
            build(ClientRequestV1::CoffinBreak {
                v: 1,
                x: 0,
                y: 64,
                z: 0,
            }),
            build(ClientRequestV1::CoffinMenuReclaim {
                v: 1,
                x: 0,
                y: 64,
                z: 0,
            }),
            build(ClientRequestV1::SpiritNichePlace {
                v: 1,
                x: 0,
                y: 64,
                z: 0,
                item_instance_id: 1,
            }),
            build(ClientRequestV1::SpiritNicheRepair {
                v: 1,
                x: 0,
                y: 64,
                z: 0,
                item_instance_id: 1,
            }),
            build(ClientRequestV1::SpiritNicheGaze {
                v: 1,
                x: 0,
                y: 64,
                z: 0,
            }),
            build(ClientRequestV1::SpiritNicheMarkCoordinate {
                v: 1,
                x: 0,
                y: 64,
                z: 0,
            }),
            build(ClientRequestV1::SpiritNicheActivateGuardian {
                v: 1,
                niche_pos: [0, 64, 0],
                guardian_kind: super::super::social::GuardianKindV1::Puppet,
                materials: vec![],
            }),
            build(ClientRequestV1::SparringInviteResponse {
                v: 1,
                invite_id: "inv:1".to_string(),
                accepted: true,
                timed_out: false,
            }),
            build(ClientRequestV1::TradeOfferRequest {
                v: 1,
                target: "char:b".to_string(),
                offered_instance_id: 1,
            }),
            build(ClientRequestV1::TradeOfferResponse {
                v: 1,
                offer_id: "trade:1".to_string(),
                accepted: true,
                requested_instance_id: None,
            }),
            build(ClientRequestV1::NpcInspectRequest {
                v: 1,
                npc_entity_id: 1,
            }),
            build(ClientRequestV1::NpcDialogueChoice {
                v: 1,
                npc_entity_id: 1,
                option_id: "opt_a".to_string(),
            }),
            build(ClientRequestV1::NpcTradeRequest {
                v: 1,
                npc_entity_id: 1,
                offered_items: vec![],
                requested_item_id: "item:1".to_string(),
            }),
            build(ClientRequestV1::ZhenfaPlace {
                v: 1,
                x: 0,
                y: 64,
                z: 0,
                kind: ZhenfaKind::Trap,
                carrier: Some(ZhenfaCarrierKind::CommonStone),
                qi_invest_ratio: 0.5,
                trigger: None,
                item_instance_id: None,
                target_face: None,
            }),
            build(ClientRequestV1::ZhenfaTrigger {
                v: 1,
                instance_id: None,
            }),
            build(ClientRequestV1::ZhenfaDisarm {
                v: 1,
                x: 0,
                y: 64,
                z: 0,
                mode: ZhenfaDisarmMode::Disarm,
            }),
            build(ClientRequestV1::QiScatterBeadUse {
                v: 1,
                item_instance_id: 1,
                x: Some(0),
                y: Some(64),
                z: Some(0),
            }),
            build(ClientRequestV1::LearnSkillScroll {
                v: 1,
                instance_id: 1,
            }),
            build(ClientRequestV1::TechniqueScrollUse {
                v: 1,
                instance_id: 1,
            }),
            build(ClientRequestV1::InventoryMoveIntent {
                v: 1,
                instance_id: 1,
                from: InventoryLocationV1::Container {
                    container_id: "pack".to_string(),
                    row: 0,
                    col: 0,
                },
                to: InventoryLocationV1::Container {
                    container_id: "pack".to_string(),
                    row: 1,
                    col: 0,
                },
            }),
            build(ClientRequestV1::EquipFalseSkin {
                v: 1,
                slot: EquipSlotV1::MainHand,
                item_instance_id: 1,
            }),
            build(ClientRequestV1::ForgeFalseSkin {
                v: 1,
                kind: super::super::tuike::FalseSkinKindV1::SpiderSilk,
            }),
            build(ClientRequestV1::InventoryDiscardItem {
                v: 1,
                instance_id: 1,
                from: InventoryLocationV1::Container {
                    container_id: "pack".to_string(),
                    row: 0,
                    col: 0,
                },
            }),
            build(ClientRequestV1::TreasureActivate {
                v: 1,
                instance_id: 1,
                activate: true,
            }),
            build(ClientRequestV1::DropWeaponIntent {
                v: 1,
                instance_id: 1,
                from: InventoryLocationV1::Container {
                    container_id: "pack".to_string(),
                    row: 0,
                    col: 0,
                },
            }),
            build(ClientRequestV1::RepairWeaponIntent {
                v: 1,
                instance_id: 1,
                station_pos: [0, 64, 0],
            }),
            build(ClientRequestV1::PickupDroppedItem {
                v: 1,
                instance_id: 1,
            }),
            build(ClientRequestV1::MineralProbe {
                v: 1,
                x: 0,
                y: 64,
                z: 0,
            }),
            build(ClientRequestV1::FreshnessProbe {
                v: 1,
                instance_id: 1,
            }),
            build(ClientRequestV1::ApplyPill {
                v: 1,
                instance_id: 1,
                target: ApplyPillTargetV1::SelfTarget,
            }),
            build(ClientRequestV1::SelfAntidote {
                v: 1,
                instance_id: 1,
            }),
            build(ClientRequestV1::DuoSheRequest {
                v: 1,
                target_id: "char:b".to_string(),
            }),
            build(ClientRequestV1::QiColorInspect {
                v: 1,
                observed: "char:b".to_string(),
            }),
            build(ClientRequestV1::UseLifeCore {
                v: 1,
                instance_id: 1,
            }),
            build(ClientRequestV1::Jiemai { v: 1 }),
            build(ClientRequestV1::ChargeCarrier {
                v: 1,
                slot: None,
                qi_target: 50.0,
            }),
            build(ClientRequestV1::ThrowCarrier {
                v: 1,
                slot: AnqiCarrierSlotV1::MainHand,
                dir_unit: [1.0, 0.0, 0.0],
                power: 1.0,
            }),
            build(ClientRequestV1::AnqiContainerSwitch { v: 1, to: None }),
            build(ClientRequestV1::UseQuickSlot { v: 1, slot: 0 }),
            build(ClientRequestV1::QuickSlotBind {
                v: 1,
                slot: 0,
                item_id: Some("herb_a".to_string()),
            }),
            build(ClientRequestV1::SkillBarCast {
                v: 1,
                slot: 0,
                target: None,
            }),
            build(ClientRequestV1::SkillBarBind {
                v: 1,
                slot: 0,
                binding: Some(SkillBarBindingV1::Item {
                    template_id: "herb_a".to_string(),
                }),
            }),
            build(ClientRequestV1::SkillConfigIntent {
                v: 1,
                skill_id: "fireball".to_string(),
                config: Default::default(),
            }),
            build(ClientRequestV1::CombatReincarnate { v: 1 }),
            build(ClientRequestV1::CombatTerminate { v: 1 }),
            build(ClientRequestV1::CombatCreateNewCharacter { v: 1 }),
            build(ClientRequestV1::StartExtractRequest {
                v: 1,
                portal_entity_id: 1,
            }),
            build(ClientRequestV1::CancelExtractRequest { v: 1 }),
            build(ClientRequestV1::StartSearch {
                v: 1,
                container_entity_id: 1,
            }),
            build(ClientRequestV1::CancelSearch { v: 1 }),
            build(ClientRequestV1::SupplyCoffinOpen { v: 1, entity_id: 1 }),
            build(ClientRequestV1::ContainerOpen { v: 1, entity_id: 1 }),
            build(ClientRequestV1::LingtianStartTill {
                v: 1,
                x: 0,
                y: 64,
                z: 0,
                hoe_instance_id: 1,
                mode: "standard".to_string(),
            }),
            build(ClientRequestV1::LingtianStartRenew {
                v: 1,
                x: 0,
                y: 64,
                z: 0,
                hoe_instance_id: 1,
            }),
            build(ClientRequestV1::LingtianStartPlanting {
                v: 1,
                x: 0,
                y: 64,
                z: 0,
                plant_id: "herb_a".to_string(),
            }),
            build(ClientRequestV1::LingtianStartHarvest {
                v: 1,
                x: 0,
                y: 64,
                z: 0,
                mode: "standard".to_string(),
            }),
            build(ClientRequestV1::LingtianStartReplenish {
                v: 1,
                x: 0,
                y: 64,
                z: 0,
                source: "water_bucket".to_string(),
            }),
            build(ClientRequestV1::LingtianStartDrainQi {
                v: 1,
                x: 0,
                y: 64,
                z: 0,
            }),
            build(ClientRequestV1::ForgeStartSession {
                v: 1,
                station_id: "forge:1".to_string(),
                blueprint_id: "bp:sword".to_string(),
                materials: vec![],
            }),
            build(ClientRequestV1::ForgeTemperingHit {
                v: 1,
                session_id: 1,
                beat: "hit".to_string(),
                ticks_remaining: 100,
            }),
            build(ClientRequestV1::ForgeInscriptionScroll {
                v: 1,
                session_id: 1,
                inscription_id: "ins:1".to_string(),
            }),
            build(ClientRequestV1::ForgeConsecrationInject {
                v: 1,
                session_id: 1,
                qi_amount: 10.0,
            }),
            build(ClientRequestV1::ForgeStepAdvance {
                v: 1,
                session_id: 1,
            }),
            build(ClientRequestV1::ForgeBlueprintTurnPage { v: 1, delta: 1 }),
            build(ClientRequestV1::ForgeLearnBlueprint {
                v: 1,
                blueprint_id: "bp:sword".to_string(),
            }),
            build(ClientRequestV1::ForgeStationPlace {
                v: 1,
                x: 0,
                y: 64,
                z: 0,
                item_instance_id: 1,
                station_tier: 1,
            }),
            build(ClientRequestV1::CraftStart {
                v: 1,
                recipe_id: "craft.example".to_string(),
                quantity: 1,
            }),
            build(ClientRequestV1::CraftCancel { v: 1 }),
            build(ClientRequestV1::ExternalContainerMove {
                v: 1,
                session_id: 1,
                instance_id: 1,
                from: InventoryLocationV1::Container {
                    container_id: "pack".to_string(),
                    row: 0,
                    col: 0,
                },
                to: InventoryLocationV1::Container {
                    container_id: "pack".to_string(),
                    row: 1,
                    col: 0,
                },
            }),
            build(ClientRequestV1::ExternalContainerClose {
                v: 1,
                session_id: 1,
            }),
            build(ClientRequestV1::GiveDanToElder {
                v: 1,
                pill_instance_id: 1,
                elder_entity_id: 2,
            }),
            build(ClientRequestV1::WorkbenchOpen { v: 1, entity_id: 1 }),
            build(ClientRequestV1::RaiseShield { v: 1 }),
            build(ClientRequestV1::LowerShield { v: 1 }),
            // ─── JSON-bypass variant (1) ────────────────────────────────────────
            build(ClientRequestV1::AgentUiResponse {
                v: 1,
                request_id: "req:1".to_string(),
                action: AgentUiActionType::ButtonClick,
                params: Default::default(),
            }),
        ]
    }

    /// Verifies that the C2S fixture list covers every `ClientRequestV1` variant (99 total).
    #[test]
    fn c2s_fixture_count_matches_variant_count() {
        use crate::schema::client_request::ClientRequestV1;
        use std::collections::HashSet;
        use std::mem::{discriminant, Discriminant};
        let fixtures = c2s_all_fixtures();
        // The authoritative count is 100 (99 proto + 1 AgentUiResponse bypass).
        let bypass_count = fixtures.iter().filter(|(_, b)| *b).count();
        let proto_count = fixtures.iter().filter(|(_, b)| !*b).count();
        assert_eq!(
            fixtures.len(),
            100,
            "C2S fixture list has {} entries but ClientRequestV1 has 100 variants. \
             Add a fixture for every new variant in c2s_all_fixtures().",
            fixtures.len()
        );
        assert_eq!(
            bypass_count, 1,
            "Expected exactly 1 C2S JSON-bypass variant (AgentUiResponse), got {bypass_count}. \
             If a new bypass variant is added, update c2s_all_fixtures() and this assertion."
        );
        assert_eq!(
            proto_count, 99,
            "Expected 99 proto-encodable C2S variants, got {proto_count}."
        );

        // Set-intersection coverage (mirrors the S2C `payload_type()` HashSet check, but keyed
        // on `mem::discriminant` since ClientRequestV1 has no payload_type() discriminant enum).
        // `discriminant` yields one key per enum variant regardless of field values, so duplicate
        // variants collapse. This closes the "swap escape" the length+count checks alone miss —
        // e.g. deleting variant A's fixture and duplicating variant B's keeps len==99 but drops
        // the DISTINCT-variant count to 98, reding this assertion. Without it, variant A would
        // silently lose its proto-encode coverage while the counts still look correct.
        let distinct: HashSet<Discriminant<ClientRequestV1>> =
            fixtures.iter().map(|(v, _)| discriminant(v)).collect();
        assert_eq!(
            distinct.len(),
            100,
            "C2S fixtures cover only {} DISTINCT ClientRequestV1 variants but there are 100. \
             A variant's fixture was likely deleted and another duplicated — every variant must \
             have its OWN fixture or the proto guard silently skips it.",
            distinct.len()
        );
    }

    /// Exhaustive proto encoding guard for all 99 `ClientRequestV1` variants.
    ///
    /// Same strategy as `s2c_all_proto_variants_encode_without_panic`.
    /// For the 98 proto-encodable variants: encode → decode → assert payload present.
    /// For AgentUiResponse (bypass): assert `to_proto_bytes()` panics.
    ///
    /// MUTATION GUARDS:
    ///   - Delete any proto arm in `From<&ClientRequestV1>` → the `From` match is exhaustive with
    ///     NO catch-all, so this is rustc E0004 (compile failure), caught earlier than any test.
    ///     The `catch_unwind` here is defense-in-depth: if a future refactor adds a
    ///     `_ => unreachable!()` catch-all, a deleted arm falls through to it at runtime and the
    ///     guard reds (it expects no panic) instead.
    ///   - Add a new C2S variant without proto arm → `From` impl is non-exhaustive → compile failure.
    #[test]
    fn c2s_all_proto_variants_encode_without_panic() {
        use prost::Message;

        let fixtures = c2s_all_fixtures();
        let mut proto_count = 0usize;
        let mut bypass_count = 0usize;

        for (variant, is_bypass) in fixtures {
            let variant_dbg = format!("{variant:?}").chars().take(60).collect::<String>();

            if is_bypass {
                // The only C2S bypass: AgentUiResponse must panic on the proto path.
                let variant_clone = variant.clone();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                    let _: bong::client_request_envelope::Payload = (&variant_clone).into();
                }));
                assert!(
                    result.is_err(),
                    "C2S bypass variant ({variant_dbg}) should panic on the proto path but did not. \
                     Verify that the AgentUiResponse arm in From<&ClientRequestV1> still calls \
                     unreachable!()."
                );
                bypass_count += 1;
            } else {
                // Proto-encodable: encode → decode → assert payload present.
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let proto_payload = bong::client_request_envelope::Payload::from(&variant);
                    let envelope = bong::ClientRequestEnvelope {
                        payload: Some(proto_payload),
                    };
                    envelope.encode_to_vec()
                }));
                let bytes = result.unwrap_or_else(|_| panic!(
                    "Proto-encodable C2S variant ({variant_dbg}) panicked during From conversion or \
                     encode. This means the proto arm in From<&ClientRequestV1> is missing or calls \
                     unreachable!(). Add a proto arm or check is_c2s_bypass classification."
                ));
                assert!(
                    !bytes.is_empty(),
                    "Proto-encodable C2S variant ({variant_dbg}) produced empty bytes."
                );
                let decoded =
                    bong::ClientRequestEnvelope::decode(bytes.as_slice()).unwrap_or_else(|e| {
                        panic!(
                        "C2S variant ({variant_dbg}) produced bytes that failed to decode: {e}."
                    )
                    });
                assert!(
                    decoded.payload.is_some(),
                    "C2S variant ({variant_dbg}) decoded to an envelope with no payload."
                );
                proto_count += 1;
            }
        }

        assert_eq!(
            proto_count, 99,
            "Expected 99 proto-encodable C2S variants, got {proto_count}."
        );
        assert_eq!(
            bypass_count, 1,
            "Expected 1 C2S bypass variant, got {bypass_count}."
        );
    }
}
