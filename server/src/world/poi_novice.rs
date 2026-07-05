//! plan-poi-novice-v1 — 新手 POI runtime registry / event stub。

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use valence::prelude::{
    bevy_ecs, App, Commands, Component, DVec3, Entity, EntityLayerId, Event, EventReader,
    EventWriter, IntoSystemConfigs, Position, Query, Res, ResMut, Resource, Startup, Update,
};

use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::world::dimension::DimensionLayers;
use crate::world::setup_world;
use crate::world::terrain::{Poi, SurfaceProvider, TerrainProviders};
use crate::world::tsy_container::{ContainerKind, LootContainer};
use crate::world::zone::{TsyDepth, DEFAULT_SPAWN_ZONE_NAME};

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

    /// 增量注册（不清空既有站点）。供 runtime scatter 系统使用——避免清掉
    /// `PoiNoviceLoader::load` 已从 manifest 加载的 11 种既有 novice POI
    /// （plan-surface-stash-runtime-scatter-gap-v1 §P1 交付物 1 / §8.1 #2）。
    pub fn extend(&mut self, sites: Vec<PoiNoviceSite>) {
        self.sites.extend(sites);
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
            Startup,
            scatter_and_spawn_surface_stashes.after(PoiNoviceLoader::load),
        )
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

// ——— plan-surface-stash-runtime-scatter-gap-v1 §P1: 散修遗缴 runtime scatter ———
//
// scatter_surface_stashes 是纯函数（determinism 由 seed 保证），
// scatter_and_spawn_surface_stashes 是接入 Startup 调度的真实生产系统
// （见 register() 里 `.add_systems(Startup, scatter_and_spawn_surface_stashes...)`）。

/// 散修遗缴 Poisson-disk 散布参数。
pub const SURFACE_STASH_COUNT: usize = 12;
pub const SURFACE_STASH_MIN_DIST: f64 = 200.0;
/// 与已有 POI（含教程 POI + 其他 novice POI）的最小距离。
pub const SURFACE_STASH_MIN_POI_DIST: f64 = 100.0;
/// spawn 中心，对齐真实 `spawn` zone AABB 中心（`zones.json` min/max ±750）。
const SPAWN_CENTER_X: f64 = 0.0;
const SPAWN_CENTER_Z: f64 = 0.0;

/// craft stash 半径上限（采样时的最大距离半径）。
const CRAFT_RADIUS: f64 = 1000.0;
/// `spawn` zone AABB 安全边距：半径 750 减 50，防止 CRAFT_RADIUS 采样点沿坐标轴
/// 方向冲出 zone 边界（§8.1 #2 决议 4）。
const SURFACE_STASH_ZONE_SAFE_RADIUS: f64 = 700.0;

/// basic / scroll / craft 目标数量。
const BASIC_COUNT: usize = 5;
const SCROLL_COUNT: usize = 4;
const CRAFT_COUNT: usize = 3;

/// runtime scatter 固定种子。全仓不存在"world seed"权威资源，字面量常数保证
/// 同一构建每次重启散布结果完全一致（§8.1 #1 决议）。
pub(crate) const SURFACE_STASH_SCATTER_SEED: u64 = 0x5343_4159_5F31_3200;

/// 拒绝采样 while-loop 的 max-attempts 兜底（博弈 gate major 修复）。
///
/// self-spacing（`current_min_dist`）每 10_000 次尝试减半，从 `SURFACE_STASH_MIN_DIST`
/// (200.0) 衰减到工程上可忽略的量级（<1e-6，log2(200/1e-6)≈28 轮）需要约
/// 28 * 10_000 = 280_000 次尝试；这是循环里唯一会随尝试次数变化收紧的判据——
/// POI 距离 / zone AABB / 可通行性判据全程不随尝试次数松动。取
/// 500_000（显著高于 280_000 衰减 schedule + shipping spawn_plain 地形实测
/// ~22 次的收敛裕量），保证：
/// - 正常可通行地形下这个上限永远打不到（determinism / 恒产 12 的既有测试不受影响）；
/// - 若未来地形改动让"可通行 ∩ zone AABB 内 ∩ 远离既有 POI"可行域清空，循环在有界
///   时间内退出而不是让 Startup 挂死（见 `scatter_surface_stashes` 尾部的优雅降级）。
pub(crate) const SURFACE_STASH_MAX_SCATTER_ATTEMPTS: u64 = 500_000;

/// 运行时 Poisson-disk 采样 12 个散修遗缴点。
///
/// 使用 seed × index 做 PRNG seed 保证 determinism。
/// 分配 pool：按距 spawn 中心距离排序后，前 5 个 = basic，接下来 4 个 = scroll，
/// 最后 3 个 = craft（deterministic quota slice）。
///
/// 若 10000 次尝试未凑满 12 点，自动将 min_dist 减半继续尝试，保证最终
/// 产出恰好 12 个点（正常可通行地形下）。避水 / 避岩浆 / 避让既有 POI / zone AABB
/// 判据全部在本函数内部的拒绝采样循环里完成——不能留给调用方对返回值事后过滤，
/// 否则被拒点没有补采机制，产出会 < 12（§8.1 #2 决议 1）。
///
/// 循环有 `SURFACE_STASH_MAX_SCATTER_ATTEMPTS` 兜底：若"可通行 ∩ zone AABB 内 ∩
/// 远离既有 POI"可行域为空（地形改动导致），耗尽后**优雅降级**——`tracing::warn!`
/// 打警告后返回已累积的点（可能 < 12），而不是无限挂死（博弈 gate major：Startup
/// 调度上挂死会让服务器起不来）。
pub fn scatter_surface_stashes(
    seed: u64,
    existing_poi_xz: &[(f64, f64)],
    is_passable: &dyn Fn(f64, f64) -> bool,
) -> Vec<ScatteredStash> {
    debug_assert_eq!(
        BASIC_COUNT + SCROLL_COUNT + CRAFT_COUNT,
        SURFACE_STASH_COUNT,
        "quota 常数必须精确覆盖 SURFACE_STASH_COUNT，否则末段 quota slice 会漏算/越界"
    );
    let mut points = Vec::with_capacity(SURFACE_STASH_COUNT);
    let mut current_min_dist = SURFACE_STASH_MIN_DIST;
    let mut attempt = 0u64;

    while points.len() < SURFACE_STASH_COUNT && attempt < SURFACE_STASH_MAX_SCATTER_ATTEMPTS {
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
        // 距离检查：与既有 POI（教程 POI + 其他 novice POI，manifest 全集）的最小距离
        let too_close_to_existing_poi = existing_poi_xz.iter().any(|&(px, pz)| {
            let dx = px - x;
            let dz = pz - z;
            (dx * dx + dz * dz).sqrt() < SURFACE_STASH_MIN_POI_DIST
        });
        if too_close_to_existing_poi {
            continue;
        }
        // zone AABB 安全边距校验
        if x.abs() > SURFACE_STASH_ZONE_SAFE_RADIUS || z.abs() > SURFACE_STASH_ZONE_SAFE_RADIUS {
            continue;
        }
        // 避水 / 避岩浆
        if !is_passable(x, z) {
            continue;
        }
        points.push(ScatteredStash {
            x,
            z,
            pool_id: String::new(), // 后面分配
            index: points.len(),
        });
    }

    if points.len() < SURFACE_STASH_COUNT {
        tracing::warn!(
            "[bong][poi-novice] scatter_surface_stashes exhausted {} attempts, only placed {}/{} \
             surface_stash point(s); likely cause: passable ∩ zone-AABB ∩ far-from-existing-POI \
             feasible region is too small or empty (terrain changed?) — degrading gracefully \
             instead of hanging Startup",
            SURFACE_STASH_MAX_SCATTER_ATTEMPTS,
            points.len(),
            SURFACE_STASH_COUNT
        );
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

/// Startup 一次性生产系统：真实把 `scatter_surface_stashes` 的纯函数产出接进
/// `PoiNoviceRegistry` + `LootContainer` 实体（plan §P1 交付物 3）。
///
/// `providers`/`layers` 任一 `None` 时直接 return——与 `PoiNoviceLoader::load`
/// 同一容错模式，测试 App 不装载完整世界插件时不 panic。
pub fn scatter_and_spawn_surface_stashes(
    mut commands: Commands,
    providers: Option<Res<TerrainProviders>>,
    layers: Option<Res<DimensionLayers>>,
    mut registry: ResMut<PoiNoviceRegistry>,
    mut spawned: EventWriter<PoiSpawned>,
) {
    let (Some(providers), Some(layers)) = (providers, layers) else {
        return;
    };

    // existing_poi_xz 取自 providers.overworld.pois() 原始 manifest 全集（不是
    // registry.sites()）——教程 POI（spawn_tutorial_coffin/tutorial_chest/...）
    // 不带 poi_novice tag，不会进入 registry；只查 registry 会漏挡，遗缴会刷到
    // 教程 POI 脸上（§8.1 #2 决议 3）。
    let existing_poi_xz: Vec<(f64, f64)> = providers
        .overworld
        .pois()
        .iter()
        .map(|poi| (f64::from(poi.pos_xyz[0]), f64::from(poi.pos_xyz[2])))
        .collect();
    let is_passable = |x: f64, z: f64| -> bool {
        providers
            .overworld
            .query_surface(x.floor() as i32, z.floor() as i32)
            .passable
    };
    // 避水/避岩浆/避让既有 POI/zone AABB 全部已在 scatter_surface_stashes 内部
    // 的拒绝采样循环里跑完——正常可通行地形下返回值恰好 12 个、且全部落在可通行
    // 列，wrapper 不需要再对返回值做任何跳过/事后过滤（§8.1 #2 决议 1）。若可行域
    // 清空触发 SURFACE_STASH_MAX_SCATTER_ATTEMPTS 兜底，返回值可能 < 12（已在
    // scatter_surface_stashes 内部 tracing::warn! 打过日志）——这里按实际长度
    // 逐个生产，不假设固定 12。
    let stashes =
        scatter_surface_stashes(SURFACE_STASH_SCATTER_SEED, &existing_poi_xz, &is_passable);

    let mut spawned_count = 0usize;
    for stash in stashes {
        let info = providers
            .overworld
            .query_surface(stash.x.floor() as i32, stash.z.floor() as i32);
        let y = info.y + 1;
        let pos = DVec3::new(stash.x, f64::from(y), stash.z);
        let pos_xyz = [stash.x as f32, y as f32, stash.z as f32];
        let site = PoiNoviceSite {
            id: format!(
                "{}:surface_stash:{}",
                DEFAULT_SPAWN_ZONE_NAME,
                position_id_token(pos_xyz)
            ),
            kind: PoiNoviceKind::SurfaceStash,
            zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            name: "散修遗缴".to_string(),
            pos_xyz,
            selection_strategy: stash.pool_id.clone(),
            qi_affinity: 0.0,
            danger_bias: 0,
            tags: vec![
                "poi_novice".to_string(),
                "poi_type:surface_stash".to_string(),
            ],
        };
        registry.extend(vec![site.clone()]);
        spawned.send(PoiSpawned { site });
        // 与 spawn_tutorial.rs 的 tutorial_chest 分支同模式；不新增任何方块摆放
        // 函数——sync_tsy_container_visuals（entity_model.rs）已通用处理任何
        // LootContainer+Position+EntityLayerId 实体的外观。
        commands.spawn((
            LootContainer::new(
                ContainerKind::SurfaceStash,
                DEFAULT_SPAWN_ZONE_NAME.to_string(),
                TsyDepth::Shallow,
                stash.pool_id,
                0,
            ),
            Position(pos),
            EntityLayerId(layers.overworld),
        ));
        spawned_count += 1;
    }

    tracing::info!(
        "[bong][poi-novice] scattered {spawned_count} surface_stash site(s) from seed={:#x}",
        SURFACE_STASH_SCATTER_SEED
    );
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScatteredStash {
    pub x: f64,
    pub z: f64,
    pub pool_id: String,
    pub index: usize,
}

/// 简单 splitmix64 PRNG（deterministic, 无状态）。
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
        let stashes = super::scatter_surface_stashes(42, &[], &|_, _| true);
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
        let stashes = super::scatter_surface_stashes(123456, &[], &|_, _| true);
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
        let a = super::scatter_surface_stashes(999, &[], &|_, _| true);
        let b = super::scatter_surface_stashes(999, &[], &|_, _| true);
        assert_eq!(a, b, "同 seed 的 scatter 结果应完全一致");
    }

    #[test]
    fn scatter_surface_stashes_quota_5_4_3() {
        let stashes = super::scatter_surface_stashes(42, &[], &|_, _| true);
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

    // ——— plan-onboarding-loop-v1 P4: 校准测试 ———

    #[test]
    fn craft_stashes_within_spawn_radius() {
        let stashes = super::scatter_surface_stashes(42, &[], &|_, _| true);
        let craft_stashes: Vec<_> = stashes
            .iter()
            .filter(|s| s.pool_id == "surface_stash_craft")
            .collect();
        assert_eq!(
            craft_stashes.len(),
            super::CRAFT_COUNT,
            "craft stash 数量应为 {}; got {}",
            super::CRAFT_COUNT,
            craft_stashes.len()
        );
        for s in &craft_stashes {
            let dist = ((s.x - super::SPAWN_CENTER_X).powi(2)
                + (s.z - super::SPAWN_CENTER_Z).powi(2))
            .sqrt();
            assert!(
                dist <= super::CRAFT_RADIUS + 1.0,
                "craft 遗缴 #{} 距 spawn 中心 {:.0} 格，应在 CRAFT_RADIUS {} 格内",
                s.index,
                dist,
                super::CRAFT_RADIUS
            );
        }
    }

    // ——— plan-surface-stash-runtime-scatter-gap-v1 §P1 交付物 2/4: 拒绝采样判据测试 ———

    #[test]
    fn scatter_surface_stashes_avoids_existing_poi_within_min_poi_dist() {
        // 一个既有 POI 落在 spawn 中心；所有产出遗缴都必须与它保持
        // >= SURFACE_STASH_MIN_POI_DIST 的距离（§8.1 #2 决议）。
        let existing = [(0.0, 0.0)];
        let stashes = super::scatter_surface_stashes(42, &existing, &|_, _| true);
        assert_eq!(stashes.len(), super::SURFACE_STASH_COUNT);
        for s in &stashes {
            let dist = (s.x.powi(2) + s.z.powi(2)).sqrt();
            assert!(
                dist >= super::SURFACE_STASH_MIN_POI_DIST - 1.0,
                "遗缴 #{} 距既有 POI (0,0) {:.1} 格，应 >= SURFACE_STASH_MIN_POI_DIST {} 格（避让判据必须在拒绝采样循环内生效）",
                s.index,
                dist,
                super::SURFACE_STASH_MIN_POI_DIST
            );
        }
    }

    #[test]
    fn scatter_surface_stashes_respects_zone_safe_radius() {
        // 无既有 POI / 恒可通行场景下，所有产出点必须落在 zone AABB 安全半径内
        // （CRAFT_RADIUS=1000 会系统性冲出 zone AABB 750 的 bug，§8.1 #2 决议 4）。
        let stashes = super::scatter_surface_stashes(7, &[], &|_, _| true);
        assert_eq!(stashes.len(), super::SURFACE_STASH_COUNT);
        for s in &stashes {
            assert!(
                s.x.abs() <= super::SURFACE_STASH_ZONE_SAFE_RADIUS + 1.0,
                "遗缴 #{} x={:.1} 超出 zone 安全半径 {}",
                s.index,
                s.x,
                super::SURFACE_STASH_ZONE_SAFE_RADIUS
            );
            assert!(
                s.z.abs() <= super::SURFACE_STASH_ZONE_SAFE_RADIUS + 1.0,
                "遗缴 #{} z={:.1} 超出 zone 安全半径 {}",
                s.index,
                s.z,
                super::SURFACE_STASH_ZONE_SAFE_RADIUS
            );
        }
    }

    #[test]
    fn scatter_surface_stashes_avoids_impassable_columns() {
        // 避水/避岩浆判据必须在纯函数内部的拒绝采样循环里生效——不是调用方
        // 事后过滤（否则产出会 < 12，击穿"恰好 12"不变量）。这里把 x<0 判定为
        // 不可通行（模拟一片水域/岩浆），验证所有产出点都落在可通行侧且总数仍为 12。
        let is_passable = |x: f64, _z: f64| x >= 0.0;
        let stashes = super::scatter_surface_stashes(99, &[], &is_passable);
        assert_eq!(
            stashes.len(),
            super::SURFACE_STASH_COUNT,
            "即便一半地形不可通行，拒绝采样也必须补采凑满 {} 个点",
            super::SURFACE_STASH_COUNT
        );
        for s in &stashes {
            assert!(
                s.x >= 0.0,
                "遗缴 #{} x={:.1} 落在标记为不可通行的一侧，避水/避岩浆判据未在循环内生效",
                s.index,
                s.x
            );
        }
    }

    // ——— 博弈 gate major：拒绝采样 while-loop max-attempts 兜底回归测试 ———

    #[test]
    fn scatter_surface_stashes_terminates_under_fully_blocked_terrain() {
        // is_passable 恒 false 模拟"可行域为空"的极端地形（全水域/全岩浆）。
        // 在加 SURFACE_STASH_MAX_SCATTER_ATTEMPTS 兜底之前，这个 while-loop 没有
        // 任何随尝试次数收紧的终止条件能应对这种输入——会无限循环，若这段代码跑在
        // Startup 调度里，服务器永远起不来。这条测试是该挂起路径唯一的 CI 覆盖。
        let start = std::time::Instant::now();
        let stashes = super::scatter_surface_stashes(42, &[], &|_, _| false);
        let elapsed = start.elapsed();

        assert!(
            elapsed < std::time::Duration::from_secs(10),
            "scatter_surface_stashes 在全不可通行地形下耗时 {elapsed:?} 才返回——\
             max-attempts 兜底未生效或上限设得过大，Startup 调度上等价于挂死"
        );
        assert!(
            stashes.len() < super::SURFACE_STASH_COUNT,
            "全不可通行地形下可行域为空，scatter_surface_stashes 应耗尽 \
             SURFACE_STASH_MAX_SCATTER_ATTEMPTS 后优雅降级、返回 <{} 个点，实际 {} 个\
             （如果仍是 {}，说明 max-attempts 兜底没有真正介入循环终止条件）",
            super::SURFACE_STASH_COUNT,
            stashes.len(),
            super::SURFACE_STASH_COUNT
        );
    }

    #[test]
    fn scatter_surface_stashes_terminates_when_existing_poi_blankets_the_aabb() {
        // 另一种可行域清空场景：既有 POI 密集铺满 ±700 AABB，使网格内任意点都
        // < SURFACE_STASH_MIN_POI_DIST(100) 远离一个既有 POI。50 格步长的密铺
        // 保证网格内任一点到最近 POI 距离 <= 50*sqrt(2)/2 ≈ 35.4 < 100，可行域为空。
        let mut existing = Vec::new();
        let mut coord = -700i32;
        while coord <= 700 {
            let mut other = -700i32;
            while other <= 700 {
                existing.push((f64::from(coord), f64::from(other)));
                other += 50;
            }
            coord += 50;
        }

        let start = std::time::Instant::now();
        let stashes = super::scatter_surface_stashes(7, &existing, &|_, _| true);
        let elapsed = start.elapsed();

        assert!(
            elapsed < std::time::Duration::from_secs(10),
            "既有 POI 铺满可行域时 scatter_surface_stashes 耗时 {elapsed:?} 才返回——\
             max-attempts 兜底未生效"
        );
        assert!(
            stashes.len() < super::SURFACE_STASH_COUNT,
            "既有 POI 密铺 AABB 使可行域清空，应优雅降级返回 <{} 个点，实际 {} 个",
            super::SURFACE_STASH_COUNT,
            stashes.len()
        );
    }

    // ——— minor pin：extend 不应清空 loader 已加载的既有 novice site ———

    #[test]
    fn scatter_and_spawn_surface_stashes_extends_without_clearing_existing_novice_sites() {
        use crate::world::terrain::TerrainProvider;

        let mut app = App::new();
        app.init_resource::<PoiNoviceRegistry>();
        app.add_event::<PoiSpawned>();
        let overworld = app.world_mut().spawn_empty().id();
        let tsy = app.world_mut().spawn_empty().id();
        app.insert_resource(DimensionLayers { overworld, tsy });
        app.insert_resource(TerrainProviders {
            overworld: TerrainProvider::empty_for_tests(),
            tsy: None,
        });

        // 模拟 PoiNoviceLoader::load 已经先跑过、从 manifest 加载了既有 novice site
        // （forge_station 等 11 种）——这一步必须发生在 scatter_and_spawn_surface_stashes
        // 之前，且此时 registry 非空，才能真正锁住"extend 不清空既有站点"这条契约。
        // 若未来有人把 registry.extend(...) 静默重构回 replace_all(...)，这条既有
        // 站点会在 scatter 跑完后消失，下面的 by_id 断言会撞红。
        app.world_mut()
            .resource_mut::<PoiNoviceRegistry>()
            .extend(vec![PoiNoviceSite {
                id: "spawn:forge_station:x304_y71_z208".to_string(),
                kind: PoiNoviceKind::ForgeStation,
                zone: "spawn".to_string(),
                name: "破败炼器台".to_string(),
                pos_xyz: [304.0, 71.0, 208.0],
                selection_strategy: "strict_radius_1500".to_string(),
                qi_affinity: 0.15,
                danger_bias: 0,
                tags: vec![
                    "poi_novice".to_string(),
                    "poi_type:forge_station".to_string(),
                ],
            }]);

        app.add_systems(Startup, scatter_and_spawn_surface_stashes);
        app.update();

        let registry = app.world().resource::<PoiNoviceRegistry>();
        assert!(
            registry
                .by_id("spawn:forge_station:x304_y71_z208")
                .is_some(),
            "既有 loader 加载的 forge_station novice site 在 scatter_and_spawn_surface_stashes \
             跑完后必须依旧存在——如果 registry.extend 被重构回 replace_all，这条既有站点会被\
             静默抹掉且没有任何测试能发现"
        );
        let surface_stash_count = registry.by_kind(PoiNoviceKind::SurfaceStash).count();
        assert_eq!(
            surface_stash_count, SURFACE_STASH_COUNT,
            "scatter 仍应产出 {} 个 SurfaceStash 站点，实际 {}",
            SURFACE_STASH_COUNT, surface_stash_count
        );
        assert_eq!(
            registry.sites().len(),
            1 + SURFACE_STASH_COUNT,
            "registry 总数应 = 既有 1 个 + 新增 {} 个 = {}，实际 {}\
             （如果 extend 被换成 replace_all，这里会只剩 {} 个，说明既有站点被清空了）",
            SURFACE_STASH_COUNT,
            1 + SURFACE_STASH_COUNT,
            registry.sites().len(),
            SURFACE_STASH_COUNT
        );
    }

    #[test]
    fn scatter_and_spawn_surface_stashes_is_deterministic_across_restarts() {
        use crate::world::terrain::TerrainProvider;

        fn make_app() -> App {
            let mut app = App::new();
            app.init_resource::<PoiNoviceRegistry>();
            app.add_event::<PoiSpawned>();
            let overworld = app.world_mut().spawn_empty().id();
            let tsy = app.world_mut().spawn_empty().id();
            app.insert_resource(DimensionLayers { overworld, tsy });
            app.insert_resource(TerrainProviders {
                overworld: TerrainProvider::empty_for_tests(),
                tsy: None,
            });
            app.add_systems(Startup, scatter_and_spawn_surface_stashes);
            app
        }

        let mut app_a = make_app();
        app_a.update();
        let mut app_b = make_app();
        app_b.update();

        let sites_a: Vec<[f32; 3]> = app_a
            .world()
            .resource::<PoiNoviceRegistry>()
            .by_kind(PoiNoviceKind::SurfaceStash)
            .map(|site| site.pos_xyz)
            .collect();
        let sites_b: Vec<[f32; 3]> = app_b
            .world()
            .resource::<PoiNoviceRegistry>()
            .by_kind(PoiNoviceKind::SurfaceStash)
            .map(|site| site.pos_xyz)
            .collect();

        assert_eq!(
            sites_a.len(),
            SURFACE_STASH_COUNT,
            "首次跑 Startup 应产出 {} 个 SurfaceStash 站点，实际 {}",
            SURFACE_STASH_COUNT,
            sites_a.len()
        );
        assert_eq!(
            sites_a, sites_b,
            "同 SURFACE_STASH_SCATTER_SEED + 同 mock provider 两次独立重启，12 个站点坐标必须逐一相等（determinism 契约）"
        );
    }
}
