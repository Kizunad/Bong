//! plan-poi-novice-v1 — 新手 POI runtime registry / event stub。

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use valence::prelude::{
    bevy_ecs, App, Component, DVec3, Entity, Event, EventReader, EventWriter, IntoSystemConfigs,
    Query, Res, ResMut, Resource, Startup, Update,
};

use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::world::setup_world;
use crate::world::terrain::{Poi, TerrainProviders};

pub const TRADE_REFUSAL_SECONDS: u64 = 7 * 24 * 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PoiNoviceKind {
    ForgeStation,
    AlchemyFurnace,
    RogueVillage,
    MutantNest,
    ScrollHidden,
    SpiritHerbValley,
    HerbPatch,
    QiSpring,
    TradeSpot,
    ShelterSpot,
    WaterSource,
    /// 散修遗缴：地表可见容器（plan-onboarding-loop-v1 P0.3）
    SurfaceStash,
}

impl PoiNoviceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ForgeStation => "forge_station",
            Self::AlchemyFurnace => "alchemy_furnace",
            Self::RogueVillage => "rogue_village",
            Self::MutantNest => "mutant_nest",
            Self::ScrollHidden => "scroll_hidden",
            Self::SpiritHerbValley => "spirit_herb_valley",
            Self::HerbPatch => "herb_patch",
            Self::QiSpring => "qi_spring",
            Self::TradeSpot => "trade_spot",
            Self::ShelterSpot => "shelter_spot",
            Self::WaterSource => "water_source",
            Self::SurfaceStash => "surface_stash",
        }
    }

    pub fn first_action_label(self) -> &'static str {
        match self {
            Self::ForgeStation => "第一次炼器",
            Self::AlchemyFurnace => "第一次炼丹",
            Self::RogueVillage => "第一次社交",
            Self::MutantNest => "第一次猎兽核",
            Self::ScrollHidden => "第一次拾取知识",
            Self::SpiritHerbValley => "第一次采集",
            Self::HerbPatch => "第一次蹲守灵草",
            Self::QiSpring => "第一次借泉修炼",
            Self::TradeSpot => "第一次路口交易",
            Self::ShelterSpot => "第一次归巢休息",
            Self::WaterSource => "第一次取水",
            Self::SurfaceStash => "第一次搜遗缴",
        }
    }
}

impl TryFrom<&str> for PoiNoviceKind {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "forge_station" => Ok(Self::ForgeStation),
            "alchemy_furnace" => Ok(Self::AlchemyFurnace),
            "rogue_village" => Ok(Self::RogueVillage),
            "mutant_nest" => Ok(Self::MutantNest),
            "scroll_hidden" => Ok(Self::ScrollHidden),
            "spirit_herb_valley" => Ok(Self::SpiritHerbValley),
            "herb_patch" => Ok(Self::HerbPatch),
            "qi_spring" => Ok(Self::QiSpring),
            "trade_spot" => Ok(Self::TradeSpot),
            "shelter_spot" => Ok(Self::ShelterSpot),
            "water_source" => Ok(Self::WaterSource),
            "surface_stash" => Ok(Self::SurfaceStash),
            other => Err(format!("unknown novice POI type `{other}`")),
        }
    }
}

#[derive(Debug, Clone, Component, PartialEq)]
pub struct PoiNoviceSite {
    pub id: String,
    pub kind: PoiNoviceKind,
    pub zone: String,
    pub name: String,
    pub pos_xyz: [f32; 3],
    pub selection_strategy: String,
    pub qi_affinity: f32,
    pub danger_bias: i32,
    pub tags: Vec<String>,
}

impl PoiNoviceSite {
    pub fn position_vec(&self) -> DVec3 {
        DVec3::new(
            f64::from(self.pos_xyz[0]),
            f64::from(self.pos_xyz[1]),
            f64::from(self.pos_xyz[2]),
        )
    }
}

#[derive(Debug, Default, Resource)]
pub struct PoiNoviceRegistry {
    sites: Vec<PoiNoviceSite>,
}

impl PoiNoviceRegistry {
    pub fn replace_all(&mut self, sites: Vec<PoiNoviceSite>) {
        self.sites = sites;
    }

    pub fn sites(&self) -> &[PoiNoviceSite] {
        &self.sites
    }

    pub fn by_kind(&self, kind: PoiNoviceKind) -> impl Iterator<Item = &PoiNoviceSite> {
        self.sites.iter().filter(move |site| site.kind == kind)
    }

    pub fn by_id(&self, id: &str) -> Option<&PoiNoviceSite> {
        self.sites.iter().find(|site| site.id == id)
    }

    pub fn nearest_by_kinds(
        &self,
        origin: DVec3,
        kinds: &[PoiNoviceKind],
        radius: f64,
    ) -> Option<&PoiNoviceSite> {
        let radius_sq = radius.max(0.0) * radius.max(0.0);
        self.sites
            .iter()
            .filter(|site| kinds.contains(&site.kind))
            .filter_map(|site| {
                let pos = site.position_vec();
                let dx = pos.x - origin.x;
                let dz = pos.z - origin.z;
                let distance_sq = dx * dx + dz * dz;
                (distance_sq <= radius_sq).then_some((site, distance_sq))
            })
            .min_by(|left, right| left.1.total_cmp(&right.1))
            .map(|(site, _)| site)
    }
}

#[derive(Debug, Clone, Event)]
pub struct PoiSpawned {
    pub site: PoiNoviceSite,
}

#[derive(Debug, Clone, Event)]
pub struct TrespassEvent {
    pub village_id: String,
    pub player: Entity,
    pub killed_npc_count: u32,
}

#[derive(Debug, Clone, Event)]
pub struct PoiFirstActionEvent {
    pub player: Entity,
    pub kind: PoiNoviceKind,
    pub tick: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TradeRefusal {
    pub player_debug_id: String,
    pub refusal_until_wall_clock_secs: u64,
    pub killed_npc_count: u32,
}

#[derive(Debug, Default, Resource)]
pub struct PoiTradeRefusalStore {
    by_village: HashMap<String, Vec<TradeRefusal>>,
}

impl PoiTradeRefusalStore {
    pub fn apply_trespass(
        &mut self,
        village_id: impl Into<String>,
        player_debug_id: impl Into<String>,
        killed_npc_count: u32,
        now_wall_clock_secs: u64,
    ) -> u64 {
        let until = now_wall_clock_secs.saturating_add(TRADE_REFUSAL_SECONDS);
        let village_id = village_id.into();
        let player_debug_id = player_debug_id.into();
        let entries = self.by_village.entry(village_id).or_default();
        if let Some(existing) = entries
            .iter_mut()
            .find(|entry| entry.player_debug_id == player_debug_id)
        {
            existing.refusal_until_wall_clock_secs = until;
            existing.killed_npc_count = killed_npc_count;
        } else {
            entries.push(TradeRefusal {
                player_debug_id,
                refusal_until_wall_clock_secs: until,
                killed_npc_count,
            });
        }
        until
    }

    pub fn refusal_until(&self, village_id: &str, player_debug_id: &str) -> Option<u64> {
        self.by_village
            .get(village_id)?
            .iter()
            .find(|entry| entry.player_debug_id == player_debug_id)
            .map(|entry| entry.refusal_until_wall_clock_secs)
    }
}

pub struct PoiNoviceLoader;

impl PoiNoviceLoader {
    pub fn load(
        providers: Option<Res<TerrainProviders>>,
        mut registry: ResMut<PoiNoviceRegistry>,
        mut spawned: EventWriter<PoiSpawned>,
    ) {
        let Some(providers) = providers else {
            return;
        };
        let sites = providers
            .overworld
            .pois()
            .iter()
            .filter_map(site_from_manifest_poi)
            .collect::<Vec<_>>();
        for site in &sites {
            spawned.send(PoiSpawned { site: site.clone() });
        }
        if !sites.is_empty() {
            tracing::info!(
                "[bong][poi-novice] loaded {} novice POIs from terrain manifest",
                sites.len()
            );
        }
        registry.replace_all(sites);
        for site in registry.sites() {
            debug_assert!(registry.by_id(site.id.as_str()).is_some());
        }
        for kind in novice_kinds() {
            tracing::debug!(
                "[bong][poi-novice] kind={} loaded_count={}",
                kind.as_str(),
                registry.by_kind(kind).count()
            );
        }
    }
}

pub fn register(app: &mut App) {
    app.init_resource::<PoiNoviceRegistry>()
        .init_resource::<PoiTradeRefusalStore>()
        .add_event::<PoiSpawned>()
        .add_event::<TrespassEvent>()
        .add_event::<PoiFirstActionEvent>()
        .add_systems(Startup, PoiNoviceLoader::load.after(setup_world))
        .add_systems(
            Update,
            (
                record_trespass_trade_refusal_stub,
                record_first_poi_action_events,
            ),
        );
}

pub fn record_trespass_trade_refusal_stub(
    mut events: EventReader<TrespassEvent>,
    mut store: ResMut<PoiTradeRefusalStore>,
) {
    let now = current_wall_clock_secs();
    for event in events.read() {
        let player_debug_id = format!("{:?}", event.player);
        let until = store.apply_trespass(
            event.village_id.clone(),
            player_debug_id.as_str(),
            event.killed_npc_count,
            now,
        );
        debug_assert_eq!(
            store.refusal_until(&event.village_id, player_debug_id.as_str()),
            Some(until)
        );
        tracing::info!(
            "[bong][poi-novice] village={} refuses player={} until={} after killed_npc_count={}",
            event.village_id,
            player_debug_id,
            until,
            event.killed_npc_count
        );
    }
}

pub fn record_first_poi_action_events(
    mut events: EventReader<PoiFirstActionEvent>,
    mut records: Query<&mut LifeRecord>,
) {
    for event in events.read() {
        let Ok(mut life_record) = records.get_mut(event.player) else {
            tracing::warn!(
                "[bong][poi-novice] first action ignored; missing LifeRecord for player={:?}",
                event.player
            );
            continue;
        };
        record_first_poi_action(&mut life_record, event.kind, event.tick);
    }
}

pub fn record_first_poi_action(life_record: &mut LifeRecord, kind: PoiNoviceKind, tick: u64) {
    let trigger = format!("poi_novice:{}", kind.as_str());
    if life_record.biography.iter().any(|entry| {
        matches!(
            entry,
            BiographyEntry::InsightTaken {
                trigger: existing,
                ..
            } if existing == &trigger
        )
    }) {
        return;
    }
    life_record.push(BiographyEntry::InsightTaken {
        trigger,
        choice: kind.first_action_label().to_string(),
        alignment: None,
        cost_kind: None,
        tick,
    });
}

pub fn site_from_manifest_poi(poi: &Poi) -> Option<PoiNoviceSite> {
    if !poi.tags.iter().any(|tag| tag == "poi_novice") {
        return None;
    }
    let tags = parse_tags(&poi.tags);
    let kind = tags
        .get("poi_type")
        .and_then(|value| PoiNoviceKind::try_from(*value).ok())?;
    let selection_strategy = tags
        .get("selection")
        .copied()
        .unwrap_or("unknown")
        .to_string();
    Some(PoiNoviceSite {
        id: stable_site_id(poi, kind, &tags),
        kind,
        zone: poi.zone.clone(),
        name: poi.name.clone(),
        pos_xyz: poi.pos_xyz,
        selection_strategy,
        qi_affinity: poi.qi_affinity,
        danger_bias: poi.danger_bias,
        tags: poi.tags.clone(),
    })
}

fn stable_site_id(poi: &Poi, kind: PoiNoviceKind, tags: &HashMap<&str, &str>) -> String {
    let token = tags
        .get("poi_id")
        .or_else(|| tags.get("id"))
        .map(|raw| stable_id_token(raw))
        .unwrap_or_else(|| position_id_token(poi.pos_xyz));
    format!("{}:{}:{}", poi.zone, kind.as_str(), token)
}

fn position_id_token(pos_xyz: [f32; 3]) -> String {
    format!(
        "x{}_y{}_z{}",
        pos_xyz[0].round() as i32,
        pos_xyz[1].round() as i32,
        pos_xyz[2].round() as i32
    )
}

fn stable_id_token(raw: &str) -> String {
    let token = raw
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if token.is_empty() {
        "site".to_string()
    } else {
        token
    }
}

pub fn parse_tags(tags: &[String]) -> HashMap<&str, &str> {
    let mut parsed = HashMap::new();
    for tag in tags {
        let Some((key, value)) = tag.split_once(':') else {
            continue;
        };
        parsed.insert(key, value);
    }
    parsed
}

fn novice_kinds() -> [PoiNoviceKind; 12] {
    [
        PoiNoviceKind::ForgeStation,
        PoiNoviceKind::AlchemyFurnace,
        PoiNoviceKind::RogueVillage,
        PoiNoviceKind::MutantNest,
        PoiNoviceKind::ScrollHidden,
        PoiNoviceKind::SpiritHerbValley,
        PoiNoviceKind::HerbPatch,
        PoiNoviceKind::QiSpring,
        PoiNoviceKind::TradeSpot,
        PoiNoviceKind::ShelterSpot,
        PoiNoviceKind::WaterSource,
        PoiNoviceKind::SurfaceStash,
    ]
}

fn current_wall_clock_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

// ——— plan-onboarding-loop-v1 P0.3: 散修遗缴 runtime scatter ———
//
// 以下函数 / 常数是 P0.3 scatter 系统的 building block，Startup system
// 接入在后续集成。本 PR 先建立纯函数 + 测试锁住契约。

/// 散修遗缴 Poisson-disk 散布参数。
#[allow(dead_code)]
pub const SURFACE_STASH_COUNT: usize = 12;
#[allow(dead_code)]
pub const SURFACE_STASH_MIN_DIST: f64 = 200.0;
/// 与已有 POI 的最小距离。
#[allow(dead_code)]
pub const SURFACE_STASH_MIN_POI_DIST: f64 = 100.0;
/// spawn 中心（fallback flat world 为 128,128）。
#[allow(dead_code)]
const SPAWN_CENTER_X: f64 = 128.0;
#[allow(dead_code)]
const SPAWN_CENTER_Z: f64 = 128.0;

/// basic stash 半径上限（距 spawn 中心）。
#[allow(dead_code)]
const BASIC_RADIUS: f64 = 500.0;
/// scroll stash 半径上限。
#[allow(dead_code)]
const SCROLL_RADIUS: f64 = 800.0;
/// craft stash 半径上限。
#[allow(dead_code)]
const CRAFT_RADIUS: f64 = 1000.0;

/// basic / scroll / craft 目标数量。
#[allow(dead_code)]
const BASIC_COUNT: usize = 5;
#[allow(dead_code)]
const SCROLL_COUNT: usize = 4;
#[allow(dead_code)]
const CRAFT_COUNT: usize = 3;

/// 运行时 Poisson-disk 采样 12 个散修遗缴点。
///
/// 使用 world_seed × index 做 PRNG seed 保证 determinism。
/// 分配 pool：按距 spawn 中心距离排序后，前 5 个 = basic，接下来 4 个 = scroll，
/// 最后 3 个 = craft（deterministic quota slice）。
///
/// 若 10000 次尝试未凑满 12 点，自动将 min_dist 减半继续尝试，保证最终
/// 产出恰好 12 个点。
#[allow(dead_code)]
pub fn scatter_surface_stashes(seed: u64) -> Vec<ScatteredStash> {
    let mut points = Vec::with_capacity(SURFACE_STASH_COUNT);
    let mut current_min_dist = SURFACE_STASH_MIN_DIST;
    let mut attempt = 0u64;

    while points.len() < SURFACE_STASH_COUNT {
        if attempt > 0 && attempt % 10000 == 0 {
            // 放宽 min_dist 继续尝试
            current_min_dist *= 0.5;
        }
        let rng = splitmix64(
            seed.wrapping_mul(0x9E37_79B9_7F4A_7C15)
                .wrapping_add(attempt),
        );
        let angle = (rng & 0xFFFF) as f64 / 65536.0 * std::f64::consts::TAU;
        let rng2 = splitmix64(rng);
        let dist = (rng2 & 0xFFFF) as f64 / 65536.0 * CRAFT_RADIUS;
        let x = SPAWN_CENTER_X + angle.cos() * dist;
        let z = SPAWN_CENTER_Z + angle.sin() * dist;
        attempt += 1;

        // 距离检查：与已有遗缴点的 min_dist
        let too_close = points.iter().any(|p: &ScatteredStash| {
            let dx = p.x - x;
            let dz = p.z - z;
            (dx * dx + dz * dz).sqrt() < current_min_dist
        });
        if too_close {
            continue;
        }
        points.push(ScatteredStash {
            x,
            z,
            pool_id: String::new(), // 后面分配
            index: points.len(),
        });
    }

    // 按距 spawn 中心距离排序
    points.sort_by(|a, b| {
        let da = ((a.x - SPAWN_CENTER_X).powi(2) + (a.z - SPAWN_CENTER_Z).powi(2)).sqrt();
        let db = ((b.x - SPAWN_CENTER_X).powi(2) + (b.z - SPAWN_CENTER_Z).powi(2)).sqrt();
        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
    });

    // Deterministic quota slice：前 5 = basic，接下来 4 = scroll，最后 3 = craft
    for (i, point) in points.iter_mut().enumerate() {
        point.pool_id = if i < BASIC_COUNT {
            "surface_stash_basic".to_string()
        } else if i < BASIC_COUNT + SCROLL_COUNT {
            "surface_stash_scroll".to_string()
        } else {
            "surface_stash_craft".to_string()
        };
    }

    points
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub struct ScatteredStash {
    pub x: f64,
    pub z: f64,
    pub pool_id: String,
    pub index: usize,
}

/// 简单 splitmix64 PRNG（deterministic, 无状态）。
#[allow(dead_code)]
fn splitmix64(seed: u64) -> u64 {
    let z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    let z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::terrain::Poi;
    use valence::prelude::Entity;

    fn novice_poi() -> Poi {
        Poi {
            zone: "spawn".to_string(),
            kind: "novice_forge_station".to_string(),
            name: "破败炼器台".to_string(),
            pos_xyz: [304.0, 71.0, 208.0],
            tags: vec![
                "poi_novice".to_string(),
                "poi_type:forge_station".to_string(),
                "selection:strict_radius_1500".to_string(),
            ],
            unlock: "引气期可用".to_string(),
            qi_affinity: 0.15,
            danger_bias: 0,
        }
    }

    #[test]
    fn manifest_poi_tag_parses_into_runtime_site() {
        let site = site_from_manifest_poi(&novice_poi()).expect("novice poi should parse");
        assert_eq!(site.id, "spawn:forge_station:x304_y71_z208");
        assert_eq!(site.kind, PoiNoviceKind::ForgeStation);
        assert_eq!(site.selection_strategy, "strict_radius_1500");
        assert_eq!(site.pos_xyz, [304.0, 71.0, 208.0]);
    }

    #[test]
    fn manifest_poi_ids_are_unique_for_same_kind_instances() {
        let mut first = novice_poi();
        first.tags[1] = "poi_type:herb_patch".to_string();
        first.pos_xyz = [10.0, 66.0, 10.0];
        let mut second = first.clone();
        second.pos_xyz = [12.0, 66.0, 10.0];

        let first = site_from_manifest_poi(&first).expect("first herb patch should parse");
        let second = site_from_manifest_poi(&second).expect("second herb patch should parse");

        assert_ne!(first.id, second.id);
        assert_eq!(first.id, "spawn:herb_patch:x10_y66_z10");
        assert_eq!(second.id, "spawn:herb_patch:x12_y66_z10");
    }

    #[test]
    fn manifest_poi_id_tag_takes_priority_and_is_sanitized() {
        let mut poi = novice_poi();
        poi.tags.extend([
            "id:fallback".to_string(),
            "poi_id:herb patch/01".to_string(),
        ]);
        poi.tags[1] = "poi_type:herb_patch".to_string();

        let site = site_from_manifest_poi(&poi).expect("tagged herb patch should parse");

        assert_eq!(site.id, "spawn:herb_patch:herb_patch_01");
    }

    #[test]
    fn nearest_by_kinds_handles_empty_kinds_and_radius_boundaries() {
        let mut registry = PoiNoviceRegistry::default();
        registry.replace_all(vec![
            PoiNoviceSite {
                id: "spawn:origin_spring".to_string(),
                kind: PoiNoviceKind::QiSpring,
                zone: "spawn".to_string(),
                name: "原点灵泉".to_string(),
                pos_xyz: [0.0, 66.0, 0.0],
                selection_strategy: "test".to_string(),
                qi_affinity: 0.2,
                danger_bias: 0,
                tags: Vec::new(),
            },
            PoiNoviceSite {
                id: "spawn:near_spring".to_string(),
                kind: PoiNoviceKind::QiSpring,
                zone: "spawn".to_string(),
                name: "近处灵泉".to_string(),
                pos_xyz: [4.0, 66.0, 0.0],
                selection_strategy: "test".to_string(),
                qi_affinity: 0.2,
                danger_bias: 0,
                tags: Vec::new(),
            },
        ]);

        assert!(registry.nearest_by_kinds(DVec3::ZERO, &[], 64.0).is_none());
        assert_eq!(
            registry
                .nearest_by_kinds(DVec3::ZERO, &[PoiNoviceKind::QiSpring], -1.0)
                .map(|site| site.id.as_str()),
            Some("spawn:origin_spring")
        );
        assert_eq!(
            registry
                .nearest_by_kinds(DVec3::ZERO, &[PoiNoviceKind::QiSpring], 3.0)
                .map(|site| site.id.as_str()),
            Some("spawn:origin_spring")
        );
    }

    #[test]
    fn daily_life_poi_kind_tags_parse_and_find_nearest() {
        let mut registry = PoiNoviceRegistry::default();
        registry.replace_all(vec![
            PoiNoviceSite {
                id: "spawn:far_herb".to_string(),
                kind: PoiNoviceKind::HerbPatch,
                zone: "spawn".to_string(),
                name: "远处灵草".to_string(),
                pos_xyz: [50.0, 66.0, 0.0],
                selection_strategy: "test".to_string(),
                qi_affinity: 0.2,
                danger_bias: 0,
                tags: Vec::new(),
            },
            PoiNoviceSite {
                id: "spawn:near_herb".to_string(),
                kind: PoiNoviceKind::HerbPatch,
                zone: "spawn".to_string(),
                name: "近处灵草".to_string(),
                pos_xyz: [8.0, 66.0, 0.0],
                selection_strategy: "test".to_string(),
                qi_affinity: 0.2,
                danger_bias: 0,
                tags: Vec::new(),
            },
        ]);

        let nearest = registry
            .nearest_by_kinds(DVec3::ZERO, &[PoiNoviceKind::HerbPatch], 64.0)
            .expect("nearest herb patch should be found");
        assert_eq!(nearest.id, "spawn:near_herb");
        assert_eq!(
            PoiNoviceKind::try_from("qi_spring"),
            Ok(PoiNoviceKind::QiSpring)
        );
    }

    #[test]
    fn trespass_refusal_extends_one_week_from_current_wall_clock() {
        let mut store = PoiTradeRefusalStore::default();
        let until = store.apply_trespass("spawn:rogue_village", "offline:Azure", 3, 100);
        assert_eq!(until, 100 + TRADE_REFUSAL_SECONDS);
        assert_eq!(
            store.refusal_until("spawn:rogue_village", "offline:Azure"),
            Some(until)
        );
    }

    #[test]
    fn life_record_first_poi_action_is_idempotent() {
        let mut life = LifeRecord::new("offline:Azure");
        record_first_poi_action(&mut life, PoiNoviceKind::ForgeStation, 12);
        record_first_poi_action(&mut life, PoiNoviceKind::ForgeStation, 99);
        assert_eq!(life.biography.len(), 1);
        assert!(matches!(
            &life.biography[0],
            BiographyEntry::InsightTaken { trigger, choice, tick, .. }
                if trigger == "poi_novice:forge_station"
                    && choice == "第一次炼器"
                    && *tick == 12
        ));
    }

    #[test]
    fn trespass_event_keeps_plan_contract_fields() {
        let event = TrespassEvent {
            village_id: "spawn:rogue_village".to_string(),
            player: Entity::from_raw(7),
            killed_npc_count: 2,
        };
        assert_eq!(event.village_id, "spawn:rogue_village");
        assert_eq!(event.killed_npc_count, 2);
    }

    // ——— plan-onboarding-loop-v1 P0.3: scatter_surface_stashes 测试 ———

    #[test]
    fn scatter_surface_stashes_produces_12_in_spawn_1000() {
        let stashes = super::scatter_surface_stashes(42);
        assert_eq!(
            stashes.len(),
            super::SURFACE_STASH_COUNT,
            "scatter 应产出 {} 个遗缴，实际 {}",
            super::SURFACE_STASH_COUNT,
            stashes.len()
        );
        // 所有点都在 spawn ±1000 范围内
        for s in &stashes {
            let dist = ((s.x - super::SPAWN_CENTER_X).powi(2)
                + (s.z - super::SPAWN_CENTER_Z).powi(2))
            .sqrt();
            assert!(
                dist <= super::CRAFT_RADIUS + 1.0, // +1 浮点容差
                "遗缴 #{} 距 spawn 中心 {:.0} 超过 {} 格上限",
                s.index,
                dist,
                super::CRAFT_RADIUS
            );
        }
        // pool_id 都是合法值
        for s in &stashes {
            assert!(
                [
                    "surface_stash_basic",
                    "surface_stash_scroll",
                    "surface_stash_craft"
                ]
                .contains(&s.pool_id.as_str()),
                "遗缴 #{} pool_id \"{}\" 不是合法值",
                s.index,
                s.pool_id
            );
        }
    }

    #[test]
    fn scatter_surface_stashes_min_spacing_200() {
        let stashes = super::scatter_surface_stashes(123456);
        for (i, a) in stashes.iter().enumerate() {
            for (j, b) in stashes.iter().enumerate() {
                if i == j {
                    continue;
                }
                let dist = ((a.x - b.x).powi(2) + (a.z - b.z).powi(2)).sqrt();
                assert!(
                    dist >= super::SURFACE_STASH_MIN_DIST - 1.0, // -1 浮点容差
                    "遗缴 #{} 与 #{} 间距 {:.1} < 最小间距 {}",
                    i,
                    j,
                    dist,
                    super::SURFACE_STASH_MIN_DIST
                );
            }
        }
    }

    #[test]
    fn scatter_surface_stashes_deterministic() {
        let a = super::scatter_surface_stashes(999);
        let b = super::scatter_surface_stashes(999);
        assert_eq!(a, b, "同 seed 的 scatter 结果应完全一致");
    }

    #[test]
    fn scatter_surface_stashes_quota_5_4_3() {
        let stashes = super::scatter_surface_stashes(42);
        let basic = stashes
            .iter()
            .filter(|s| s.pool_id == "surface_stash_basic")
            .count();
        let scroll = stashes
            .iter()
            .filter(|s| s.pool_id == "surface_stash_scroll")
            .count();
        let craft = stashes
            .iter()
            .filter(|s| s.pool_id == "surface_stash_craft")
            .count();
        assert_eq!(
            basic,
            super::BASIC_COUNT,
            "basic 数量应为 {}，实际为 {}",
            super::BASIC_COUNT,
            basic
        );
        assert_eq!(
            scroll,
            super::SCROLL_COUNT,
            "scroll 数量应为 {}，实际为 {}",
            super::SCROLL_COUNT,
            scroll
        );
        assert_eq!(
            craft,
            super::CRAFT_COUNT,
            "craft 数量应为 {}，实际为 {}",
            super::CRAFT_COUNT,
            craft
        );
    }

    #[test]
    fn surface_stash_poi_kind_str_roundtrip() {
        assert_eq!(PoiNoviceKind::SurfaceStash.as_str(), "surface_stash");
        assert_eq!(
            PoiNoviceKind::try_from("surface_stash"),
            Ok(PoiNoviceKind::SurfaceStash)
        );
        assert_eq!(
            PoiNoviceKind::SurfaceStash.first_action_label(),
            "第一次搜遗缴"
        );
    }
}
