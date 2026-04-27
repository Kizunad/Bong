//! plan-tsy-worldgen-v1 §1 — POI → entity spawn.
//!
//! Startup-time consumers that read `TerrainProviders.{overworld, tsy}.pois()`
//! and spawn `RiftPortal` / `LootContainer` / `NpcAnchor` / `RelicCoreSlot`
//! marker entities into the appropriate Valence layer. Failure handling
//! (missing tag / unknown enum) follows §1.4: log warn + skip, never panic.
//!
//! POI tag convention (§1.1): `Vec<String>` of `"key:value"` pairs. Multi-value
//! semantics are NOT supported by `parse_tags` — duplicate keys collapse
//! last-write-wins. If P3 introduces multi-`loot_pool` semantics it must
//! migrate this helper to `HashMap<&str, Vec<&str>>` and update callers.
//!
//! Cross-dim split (§架构反转 2026-04-24):
//! - overworld provider hosts only `rift_portal direction=entry`
//! - tsy provider hosts `rift_portal direction=exit` + `loot_container` /
//!   `npc_anchor` / `relic_core_slot`
//!
//! Portal block placement (§1.2.a) is deferred to a follow-up — this commit
//! lands the marker entities only; the vanilla nether_portal / end_portal
//! block frames will be written by `tsy_portal.rs` once it gates on
//! `RiftPortal` markers spawned here.

use crate::world::dimension::DimensionKind;
use crate::world::dimension::DimensionLayers;
use crate::world::setup_world;
use crate::world::terrain::TerrainProviders;
use crate::world::tsy::{
    DimensionAnchor, LootContainer, NpcAnchor, PortalDirection, RelicCoreSlot, RiftPortal,
};
use valence::prelude::{
    App, Commands, DVec3, EntityLayerId, IntoSystemConfigs, Position, Res, Startup,
};

const DEFAULT_TRIGGER_RADIUS: f64 = 1.5;
const DEFAULT_LEASH_RADIUS: f64 = 8.0;
const DEFAULT_SLOT_COUNT: u8 = 1;
const MAX_SLOT_COUNT: u8 = 8;

const KNOWN_CONTAINER_ARCHETYPES: &[&str] = &[
    "dry_corpse",
    "skeleton",
    "storage_pouch",
    "stone_casket",
    "relic_core",
];

const KNOWN_CONTAINER_LOCKS: &[&str] = &["stone_key", "jade_seal", "array_sigil"];

const KNOWN_NPC_ARCHETYPES: &[&str] = &[
    "daoxiang",
    "zhinian",
    "sentinel",
    "fuya",
    // P4 plan-tsy-hostile-v1 will pin the final naming for "高阶守株待兔者";
    // consumer accepts the placeholder pre-emptively so blueprint authors can
    // start using it without churn.
    "ancient_sentinel",
];

const KNOWN_NPC_TRIGGERS: &[&str] = &["on_enter", "on_relic_touched", "always"];

pub fn register(app: &mut App) {
    // Bevy 0.14 默认并行 Startup —— 必须显式 .after(setup_world) 让 consumer
    // 在 TerrainProviders / DimensionLayers 资源插入之后运行；否则
    // Option<Res<...>> 全是 None，4 个系统全 silent skip，看起来"成功跑了 0 个 marker"。
    app.add_systems(
        Startup,
        (
            spawn_rift_portals,
            spawn_tsy_containers,
            spawn_tsy_npc_anchors,
            spawn_tsy_relic_slots,
        )
            .after(setup_world),
    );
}

pub fn spawn_rift_portals(
    mut commands: Commands,
    providers: Option<Res<TerrainProviders>>,
    layers: Option<Res<DimensionLayers>>,
) {
    let (Some(providers), Some(layers)) = (providers, layers) else {
        return;
    };

    let mut entry_count = 0usize;
    let mut exit_count = 0usize;

    // Overworld → entry portals
    for poi in providers
        .overworld
        .pois()
        .iter()
        .filter(|p| p.kind == "rift_portal")
    {
        let tags = parse_tags(&poi.tags);
        let Some(direction) = parse_direction(&tags) else {
            warn_skip(
                "rift_portal",
                &poi.zone,
                poi.pos_xyz,
                "missing direction tag",
            );
            continue;
        };
        if !matches!(direction, PortalDirection::Entry) {
            warn_skip(
                "rift_portal",
                &poi.zone,
                poi.pos_xyz,
                "overworld provider hosts only entry portals",
            );
            continue;
        }
        let Some(family_id) = tags.get("family_id").map(|s| s.to_string()) else {
            warn_skip("rift_portal", &poi.zone, poi.pos_xyz, "missing family_id");
            continue;
        };
        // Entry portal must resolve to an explicit TSY-side spawn coord.
        // Fallback to DVec3::ZERO would silently teleport players into the
        // void; §1.4 mandates warn+skip on incomplete data.
        // raster_check invariant §4.3 #6 already enforces this tag at validate
        // time, but defend at runtime too — a stale manifest must not fall back
        // to (0,0,0) and silently void the player.
        let Some(target_pos) = parse_target_family_pos_xyz(&tags) else {
            warn_skip(
                "rift_portal",
                &poi.zone,
                poi.pos_xyz,
                "missing target_family_pos_xyz tag (entry portals require explicit TSY coord)",
            );
            continue;
        };

        commands.spawn((
            RiftPortal {
                family_id,
                target: DimensionAnchor {
                    dimension: DimensionKind::Tsy,
                    pos: target_pos,
                },
                trigger_radius: parse_f64(&tags, "trigger_radius")
                    .unwrap_or(DEFAULT_TRIGGER_RADIUS),
                direction: PortalDirection::Entry,
            },
            Position(poi_pos_dvec3(poi.pos_xyz)),
            EntityLayerId(layers.overworld),
        ));
        entry_count += 1;
    }

    // TSY → exit portals
    let Some(tsy) = providers.tsy.as_ref() else {
        tracing::info!(
            "[bong][tsy-poi] spawn_rift_portals: spawned {entry_count} entry portals; \
             tsy provider not loaded, skipping exit portals"
        );
        return;
    };
    for poi in tsy.pois().iter().filter(|p| p.kind == "rift_portal") {
        let tags = parse_tags(&poi.tags);
        let Some(direction) = parse_direction(&tags) else {
            warn_skip(
                "rift_portal",
                &poi.zone,
                poi.pos_xyz,
                "missing direction tag",
            );
            continue;
        };
        if !matches!(direction, PortalDirection::Exit) {
            warn_skip(
                "rift_portal",
                &poi.zone,
                poi.pos_xyz,
                "tsy provider hosts only exit portals",
            );
            continue;
        }
        let Some(family_id) = tags.get("family_id").map(|s| s.to_string()) else {
            warn_skip("rift_portal", &poi.zone, poi.pos_xyz, "missing family_id");
            continue;
        };

        commands.spawn((
            RiftPortal {
                family_id,
                // exit's target.pos is filled at runtime from TsyPresence.return_to;
                // placeholder zero is replaced by tsy_portal.rs when the player
                // touches the marker.
                target: DimensionAnchor {
                    dimension: DimensionKind::Overworld,
                    pos: DVec3::ZERO,
                },
                trigger_radius: parse_f64(&tags, "trigger_radius")
                    .unwrap_or(DEFAULT_TRIGGER_RADIUS),
                direction: PortalDirection::Exit,
            },
            Position(poi_pos_dvec3(poi.pos_xyz)),
            EntityLayerId(layers.tsy),
        ));
        exit_count += 1;
    }

    tracing::info!(
        "[bong][tsy-poi] spawn_rift_portals: spawned {entry_count} entry / {exit_count} exit portals"
    );
}

pub fn spawn_tsy_containers(
    mut commands: Commands,
    providers: Option<Res<TerrainProviders>>,
    layers: Option<Res<DimensionLayers>>,
) {
    let (Some(providers), Some(layers)) = (providers, layers) else {
        return;
    };
    let Some(tsy) = providers.tsy.as_ref() else {
        return;
    };

    let mut count = 0usize;
    for poi in tsy.pois().iter().filter(|p| p.kind == "loot_container") {
        let tags = parse_tags(&poi.tags);
        let Some(archetype) = parse_known(&tags, "archetype", KNOWN_CONTAINER_ARCHETYPES) else {
            warn_skip(
                "loot_container",
                &poi.zone,
                poi.pos_xyz,
                "missing or unknown archetype tag",
            );
            continue;
        };
        // 缺 `locked:` tag 视为 unlocked；写了但值未知（typo 风险）需要 warn
        // 而非默默 unlocked，否则 `locked:bone_key` 拼写错就让箱子直接打开。
        let lock = match tags.get("locked").copied() {
            Some(value) if KNOWN_CONTAINER_LOCKS.contains(&value) => Some(value.to_string()),
            Some(value) => {
                tracing::warn!(
                    "[bong][tsy-poi] loot_container at zone={} pos=({:.1},{:.1},{:.1}): \
                     unknown locked value {value:?}, treating as unlocked",
                    poi.zone,
                    poi.pos_xyz[0],
                    poi.pos_xyz[1],
                    poi.pos_xyz[2]
                );
                None
            }
            None => None,
        };
        let loot_pool = tags.get("loot_pool").map(|s| s.to_string());

        commands.spawn((
            LootContainer {
                archetype,
                lock,
                loot_pool,
            },
            Position(poi_pos_dvec3(poi.pos_xyz)),
            EntityLayerId(layers.tsy),
        ));
        count += 1;
    }

    tracing::info!("[bong][tsy-poi] spawn_tsy_containers: spawned {count} markers");
}

pub fn spawn_tsy_npc_anchors(
    mut commands: Commands,
    providers: Option<Res<TerrainProviders>>,
    layers: Option<Res<DimensionLayers>>,
) {
    let (Some(providers), Some(layers)) = (providers, layers) else {
        return;
    };
    let Some(tsy) = providers.tsy.as_ref() else {
        return;
    };

    let mut count = 0usize;
    for poi in tsy.pois().iter().filter(|p| p.kind == "npc_anchor") {
        let tags = parse_tags(&poi.tags);
        let Some(archetype) = parse_known(&tags, "archetype", KNOWN_NPC_ARCHETYPES) else {
            warn_skip(
                "npc_anchor",
                &poi.zone,
                poi.pos_xyz,
                "missing or unknown archetype tag",
            );
            continue;
        };
        let trigger = parse_known(&tags, "trigger", KNOWN_NPC_TRIGGERS)
            .unwrap_or_else(|| "on_enter".to_string());
        let leash_radius = parse_f64(&tags, "leash_radius").unwrap_or(DEFAULT_LEASH_RADIUS);

        commands.spawn((
            NpcAnchor {
                archetype,
                trigger,
                leash_radius,
            },
            Position(poi_pos_dvec3(poi.pos_xyz)),
            EntityLayerId(layers.tsy),
        ));
        count += 1;
    }

    tracing::info!("[bong][tsy-poi] spawn_tsy_npc_anchors: spawned {count} markers");
}

pub fn spawn_tsy_relic_slots(
    mut commands: Commands,
    providers: Option<Res<TerrainProviders>>,
    layers: Option<Res<DimensionLayers>>,
) {
    let (Some(providers), Some(layers)) = (providers, layers) else {
        return;
    };
    let Some(tsy) = providers.tsy.as_ref() else {
        return;
    };

    let mut count = 0usize;
    for poi in tsy.pois().iter().filter(|p| p.kind == "relic_core_slot") {
        let tags = parse_tags(&poi.tags);
        let slot_count = parse_u8(&tags, "slot_count")
            .unwrap_or(DEFAULT_SLOT_COUNT)
            .clamp(1, MAX_SLOT_COUNT);

        commands.spawn((
            RelicCoreSlot { slot_count },
            Position(poi_pos_dvec3(poi.pos_xyz)),
            EntityLayerId(layers.tsy),
        ));
        count += 1;
    }

    tracing::info!("[bong][tsy-poi] spawn_tsy_relic_slots: spawned {count} markers");
}

// ---------- Tag parsing helpers ----------

/// Parse `tags: Vec<String>` (each element shaped `"key:value"`) into a flat
/// HashMap. Duplicate keys collapse last-write-wins — multi-value semantics
/// are explicitly NOT supported (see module doc).
fn parse_tags(raw: &[String]) -> std::collections::HashMap<&str, &str> {
    let mut out = std::collections::HashMap::new();
    for tag in raw {
        if let Some((key, value)) = tag.split_once(':') {
            out.insert(key, value);
        }
    }
    out
}

fn parse_direction(tags: &std::collections::HashMap<&str, &str>) -> Option<PortalDirection> {
    match tags.get("direction").copied()? {
        "entry" => Some(PortalDirection::Entry),
        "exit" => Some(PortalDirection::Exit),
        _ => None,
    }
}

fn parse_known(
    tags: &std::collections::HashMap<&str, &str>,
    key: &str,
    whitelist: &[&str],
) -> Option<String> {
    let value = tags.get(key).copied()?;
    if whitelist.contains(&value) {
        Some(value.to_string())
    } else {
        None
    }
}

fn parse_target_family_pos_xyz(tags: &std::collections::HashMap<&str, &str>) -> Option<DVec3> {
    let raw = tags.get("target_family_pos_xyz").copied()?;
    let mut parts = raw.split(',');
    let x: f64 = parts.next()?.trim().parse().ok()?;
    let y: f64 = parts.next()?.trim().parse().ok()?;
    let z: f64 = parts.next()?.trim().parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(DVec3::new(x, y, z))
}

fn parse_f64(tags: &std::collections::HashMap<&str, &str>, key: &str) -> Option<f64> {
    tags.get(key).copied()?.parse::<f64>().ok()
}

fn parse_u8(tags: &std::collections::HashMap<&str, &str>, key: &str) -> Option<u8> {
    tags.get(key).copied()?.parse::<u8>().ok()
}

fn poi_pos_dvec3(pos: [f32; 3]) -> DVec3 {
    DVec3::new(pos[0] as f64, pos[1] as f64, pos[2] as f64)
}

fn warn_skip(kind: &str, zone: &str, pos: [f32; 3], reason: &str) {
    tracing::warn!(
        "[bong][tsy-poi] skip {kind} at zone={zone} pos=({:.1},{:.1},{:.1}): {reason}",
        pos[0],
        pos[1],
        pos[2]
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag_vec(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parse_tags_basic() {
        let raw = tag_vec(&["direction:entry", "kind:main", "family_id:zongmen_01"]);
        let parsed = parse_tags(&raw);
        assert_eq!(parsed.get("direction").copied(), Some("entry"));
        assert_eq!(parsed.get("family_id").copied(), Some("zongmen_01"));
    }

    #[test]
    fn parse_tags_drops_malformed() {
        let raw = tag_vec(&["no_colon", "good:value"]);
        let parsed = parse_tags(&raw);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed.get("good").copied(), Some("value"));
    }

    #[test]
    fn parse_direction_returns_entry_or_exit() {
        let entry_raw = tag_vec(&["direction:entry"]);
        let entry = parse_tags(&entry_raw);
        assert_eq!(parse_direction(&entry), Some(PortalDirection::Entry));
        let exit_raw = tag_vec(&["direction:exit"]);
        let exit = parse_tags(&exit_raw);
        assert_eq!(parse_direction(&exit), Some(PortalDirection::Exit));
        let bad_raw = tag_vec(&["direction:sideways"]);
        let bad = parse_tags(&bad_raw);
        assert_eq!(parse_direction(&bad), None);
    }

    #[test]
    fn parse_known_filters_whitelist() {
        let raw = tag_vec(&["archetype:daoxiang"]);
        let tags = parse_tags(&raw);
        assert_eq!(
            parse_known(&tags, "archetype", KNOWN_NPC_ARCHETYPES),
            Some("daoxiang".to_string())
        );
        let bogus_raw = tag_vec(&["archetype:made_up"]);
        let bogus = parse_tags(&bogus_raw);
        assert_eq!(parse_known(&bogus, "archetype", KNOWN_NPC_ARCHETYPES), None);
    }

    #[test]
    fn parse_target_family_pos_xyz_three_floats() {
        let raw = tag_vec(&["target_family_pos_xyz:50,80,-25.5"]);
        let tags = parse_tags(&raw);
        assert_eq!(
            parse_target_family_pos_xyz(&tags),
            Some(DVec3::new(50.0, 80.0, -25.5))
        );
    }

    #[test]
    fn parse_target_family_pos_xyz_rejects_too_few() {
        let raw = tag_vec(&["target_family_pos_xyz:50,80"]);
        let tags = parse_tags(&raw);
        assert_eq!(parse_target_family_pos_xyz(&tags), None);
    }

    #[test]
    fn parse_f64_and_u8_parsing() {
        let raw = tag_vec(&["trigger_radius:2.5", "slot_count:5"]);
        let tags = parse_tags(&raw);
        assert_eq!(parse_f64(&tags, "trigger_radius"), Some(2.5));
        assert_eq!(parse_u8(&tags, "slot_count"), Some(5));
        assert_eq!(parse_u8(&tags, "missing"), None);
    }

    #[test]
    fn ancient_sentinel_archetype_accepted() {
        let raw = tag_vec(&["archetype:ancient_sentinel"]);
        let tags = parse_tags(&raw);
        assert_eq!(
            parse_known(&tags, "archetype", KNOWN_NPC_ARCHETYPES),
            Some("ancient_sentinel".to_string())
        );
    }
}
