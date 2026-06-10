use std::collections::HashSet;
use std::fs;
use std::path::Path;

use serde::Deserialize;
use valence::prelude::{App, Commands, DVec3, Resource, Startup};

use super::dimension::DimensionKind;
use super::TEST_AREA_BLOCK_EXTENT;
use crate::persistence::{ZoneOverlayRecord, ZoneRuntimeRecord, ZONE_OVERLAY_PAYLOAD_VERSION};

pub const DEFAULT_ZONES_PATH: &str = "zones.json";
pub const DEFAULT_SPAWN_ZONE_NAME: &str = "spawn";

const DEFAULT_SPAWN_BOUNDS_MIN: [f64; 3] = [-128.0, 64.0, -128.0];
const DEFAULT_SPAWN_BOUNDS_MAX_Y: f64 = 80.0;
const DEFAULT_SPAWN_SPIRIT_QI: f64 = 0.9;
const DEFAULT_SPAWN_PATROL_ANCHORS: [[f64; 3]; 1] = [[14.0, 66.0, 14.0]];
const MAX_ZONE_DANGER_LEVEL: u8 = 7;
const MIN_ZONE_SPIRIT_QI: f64 = -1.0;
const MAX_ZONE_SPIRIT_QI: f64 = 1.0;
const COLLAPSED_ZONE_EVENT_NAME: &str = "realm_collapse";

#[derive(Clone, Debug, PartialEq)]
pub struct Zone {
    pub name: String,
    /// Dimension this zone lives in. Defaults to overworld for backwards compatibility
    /// with `zones.json` snapshots that pre-date the TSY dim.
    pub dimension: DimensionKind,
    pub bounds: (DVec3, DVec3),
    pub spirit_qi: f64,
    pub danger_level: u8,
    pub active_events: Vec<String>,
    pub patrol_anchors: Vec<DVec3>,
    pub blocked_tiles: Vec<(i32, i32)>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BotanyZoneTag {
    Plains,
    Mountain,
    Marsh,
    BloodValley,
    Cave,
    Wastes,
    /// 负灵域（噬脉根 / 浮尘草 生长地）。plan §1.1 特殊路径，不扣 zone spirit_qi。
    /// 判定：zone.spirit_qi < -0.2 即视为负灵域（阈值可调）。
    NegativeField,
    // 注：原计划加 DeathEdge / ResidueAsh / FakeVeinBurn，但调查后发现它们不是 zone 属性：
    //  - DeathEdge 是动态的"灵气衰退锋线"
    //  - ResidueAsh 是 block 级（残灰方块表面）
    //  - FakeVeinBurn 是事件级临时焦土
    // 这些生境对应的植物 (yang_jing_tai / hui_jin_tai / tian_nu_jiao) 全部走 EventTriggered
    // 路径，由专属事件系统（plan-residue / plan-tribulation）触发，不挂 zone tag。
}

impl Zone {
    fn spawn() -> Self {
        Self {
            name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            dimension: DimensionKind::Overworld,
            bounds: default_spawn_bounds(),
            spirit_qi: DEFAULT_SPAWN_SPIRIT_QI,
            danger_level: 0,
            active_events: Vec::new(),
            patrol_anchors: DEFAULT_SPAWN_PATROL_ANCHORS
                .into_iter()
                .map(dvec3_from_array)
                .collect(),
            blocked_tiles: Vec::new(),
        }
    }

    pub fn contains(&self, pos: DVec3) -> bool {
        let (min, max) = self.bounds;

        pos.x >= min.x
            && pos.x <= max.x
            && pos.y >= min.y
            && pos.y <= max.y
            && pos.z >= min.z
            && pos.z <= max.z
    }

    /// AABB 空间体积。嵌套 / 重叠 zone 命中时用于挑选最小（最具体）的 zone，
    /// 让内嵌小 zone（渊口荒丘、九宗故地等）不会被外层大 zone 永久遮蔽。
    pub fn aabb_volume(&self) -> f64 {
        let (min, max) = self.bounds;
        (max.x - min.x).max(0.0) * (max.y - min.y).max(0.0) * (max.z - min.z).max(0.0)
    }

    pub fn clamp_position(&self, pos: DVec3) -> DVec3 {
        let (min, max) = self.bounds;

        DVec3::new(
            pos.x.clamp(min.x, max.x),
            pos.y.clamp(min.y, max.y),
            pos.z.clamp(min.z, max.z),
        )
    }

    pub fn center(&self) -> DVec3 {
        let (min, max) = self.bounds;
        DVec3::new(
            (min.x + max.x) * 0.5,
            (min.y + max.y) * 0.5,
            (min.z + max.z) * 0.5,
        )
    }

    pub fn patrol_target(&self, anchor_index: usize) -> DVec3 {
        if self.patrol_anchors.is_empty() {
            self.center()
        } else {
            self.patrol_anchors[anchor_index % self.patrol_anchors.len()]
        }
    }

    /// plan-tsy-zone-v1 §0 axiom 1 — TSY 系列 zone 通过 `tsy_` 名前缀识别，不改 Zone struct。
    pub fn is_tsy(&self) -> bool {
        self.name.starts_with("tsy_")
    }

    /// plan-tsy-zone-v1 §1.2 — 解析 TSY 层深（None = 不是 TSY 或后缀不规范）。
    ///
    /// 当前 P0 暴露公共 API，由 `/tsy_spawn` 调试命令（plan §3.1）/ worldgen plan /
    /// loot plan 后续消费；P0 自身 drain / portal 不使用层深字段。
    #[allow(dead_code)]
    pub fn tsy_depth(&self) -> Option<TsyDepth> {
        if !self.is_tsy() {
            return None;
        }
        if self.name.ends_with("_shallow") {
            Some(TsyDepth::Shallow)
        } else if self.name.ends_with("_mid") {
            Some(TsyDepth::Mid)
        } else if self.name.ends_with("_deep") {
            Some(TsyDepth::Deep)
        } else {
            None
        }
    }

    /// plan-tsy-zone-v1 §1.2 — TSY 系列 id（"tsy_lingxu_01_shallow" → "tsy_lingxu_01"）。
    ///
    /// 当前 P0 暴露公共 API；消费方为 `/tsy_spawn`（用于 family→3-subzone 检索）
    /// 与后续 worldgen plan。
    #[allow(dead_code)]
    pub fn tsy_family_id(&self) -> Option<String> {
        if !self.is_tsy() {
            return None;
        }
        // 仅当后缀属于已知层深时切除，避免不规范命名错误归一。
        match self.tsy_depth() {
            Some(_) => self.name.rsplit_once('_').map(|(head, _)| head.to_string()),
            None => None,
        }
    }

    /// plan-tsy-zone-v1 §1.1 — 入口层标记（active_events 含 `tsy_entry` tag）。
    ///
    /// 当前 P0 暴露公共 API；消费方为 `/tsy_spawn` 调试命令（plan §3.1）+ worldgen plan
    /// 用于"哪一层是着陆点"的查询。
    #[allow(dead_code)]
    pub fn is_tsy_entry(&self) -> bool {
        self.active_events.iter().any(|e| e == "tsy_entry")
    }
}

/// plan-tsy-zone-v1 §1.2 — 坍缩渊层深枚举。
///
/// 命名为 `TsyDepth` 而非 plan 文档原文的 `TsyLayer`，避免与
/// `world::dimension::TsyLayer`（marker component for the bong:tsy `LayerBundle`）冲突。
#[allow(dead_code)] // P0 仅由测试 + 公共 API 消费；运行时使用方在后续 plan 接入。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TsyDepth {
    Shallow,
    Mid,
    Deep,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ZoneRegistry {
    pub zones: Vec<Zone>,
}

impl Resource for ZoneRegistry {}

impl Default for ZoneRegistry {
    fn default() -> Self {
        Self::fallback()
    }
}

impl ZoneRegistry {
    pub fn fallback() -> Self {
        Self {
            zones: vec![Zone::spawn()],
        }
    }

    pub fn load() -> Self {
        let manifest_dir_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_ZONES_PATH);
        Self::load_from_path(manifest_dir_path)
    }

    pub fn load_from_path(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();

        let contents = match fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                tracing::info!(
                    "[bong][world] no zones config at {}, using fallback spawn zone",
                    path.display()
                );
                return Self::fallback();
            }
            Err(error) => {
                tracing::warn!(
                    "[bong][world] failed to read {} as zones config, using fallback spawn zone: {error}",
                    path.display()
                );
                return Self::fallback();
            }
        };

        let registry = match serde_json::from_str::<ZonesFileConfig>(&contents) {
            Ok(config) => match ZoneRegistry::try_from(config) {
                Ok(registry) => registry,
                Err(error) => {
                    tracing::warn!(
                        "[bong][world] invalid zones config at {}, using fallback spawn zone: {error}",
                        path.display()
                    );
                    return Self::fallback();
                }
            },
            Err(error) => {
                tracing::warn!(
                    "[bong][world] failed to parse {} as zones config, using fallback spawn zone: {error}",
                    path.display()
                );
                return Self::fallback();
            }
        };

        tracing::info!(
            "[bong][world] loaded {} authoritative zone(s) from {}",
            registry.zones.len(),
            path.display()
        );

        registry
    }

    pub fn find_zone_by_name(&self, name: &str) -> Option<&Zone> {
        self.zones.iter().find(|zone| zone.name == name)
    }

    /// Find the zone at `pos` within `dim`. Zones registered to other dimensions
    /// are skipped even if their AABB happens to overlap on the same XYZ in their
    /// own coordinate system.
    pub fn find_zone(&self, dim: DimensionKind, pos: DVec3) -> Option<&Zone> {
        // 嵌套 / 重叠 zone 命中时返回 AABB 体积最小（最具体）的那个：
        // 例如玩家站在嵌入 blood_valley 的 rift_mouth_blood_001 内，应解析为渊口
        // 而非外层血谷。否则按注册顺序取第一个命中者，内嵌小 zone 会被遮蔽。
        self.zones
            .iter()
            .filter(|zone| zone.dimension == dim && zone.contains(pos))
            .min_by(|a, b| {
                a.aabb_volume()
                    .partial_cmp(&b.aabb_volume())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    pub fn find_zone_mut(&mut self, name: &str) -> Option<&mut Zone> {
        self.zones.iter_mut().find(|zone| zone.name == name)
    }

    /// plan-territory-v1 P3 — 判断两个 zone 是否相邻。
    ///
    /// 将两个 zone 的 AABB 各在 XZ 平面扩展 `margin` 格后检查 XZ 投影重叠。
    /// Y 轴不参与（地面游戏，高度不影响相邻语义）。
    ///
    /// `margin` 建议取 100～200 格，过大会导致跨整图误传。
    /// 两个 zone 名相同视为相同 zone，返回 false（无意义的「自相邻」）。
    pub fn zones_are_adjacent(&self, name_a: &str, name_b: &str, margin: f64) -> bool {
        if name_a == name_b {
            return false;
        }
        let Some(a) = self.find_zone_by_name(name_a) else {
            return false;
        };
        let Some(b) = self.find_zone_by_name(name_b) else {
            return false;
        };
        let (a_min, a_max) = a.bounds;
        let (b_min, b_max) = b.bounds;
        // XZ 平面扩展 margin 后 overlap 检测
        let ax_min = a_min.x - margin;
        let ax_max = a_max.x + margin;
        let az_min = a_min.z - margin;
        let az_max = a_max.z + margin;
        let overlap_x = ax_min <= b_max.x && ax_max >= b_min.x;
        let overlap_z = az_min <= b_max.z && az_max >= b_min.z;
        overlap_x && overlap_z
    }

    /// plan-territory-v1 P3 — 返回与 `name` zone 相邻的所有 zone 名称列表。
    #[allow(dead_code)]
    pub fn adjacent_zone_names(&self, name: &str, margin: f64) -> Vec<String> {
        self.zones
            .iter()
            .filter(|z| z.name != name && self.zones_are_adjacent(name, &z.name, margin))
            .map(|z| z.name.clone())
            .collect()
    }

    /// plan-tsy-zone-v1 §-1 隐形前置 — 运行时动态 add 一个 zone（如 `/tsy_spawn`
    /// 调试命令追加 TSY subzone）。同名 zone 已存在则拒绝（idempotent guard）。
    /// 不做 AABB 相交校验：调用方负责保证语义正确（同 family 三层共享 XZ 是合法例外）。
    pub fn register_runtime_zone(&mut self, zone: Zone) -> Result<(), String> {
        if self.zones.iter().any(|existing| existing.name == zone.name) {
            return Err(format!(
                "zone `{}` already registered; runtime add rejected",
                zone.name
            ));
        }
        self.zones.push(zone);
        Ok(())
    }

    pub fn apply_runtime_records(&mut self, runtime_records: &[ZoneRuntimeRecord]) {
        for runtime_record in runtime_records {
            if let Some(zone) = self.find_zone_mut(runtime_record.zone_id.as_str()) {
                zone.spirit_qi = runtime_record.spirit_qi;
                zone.danger_level = runtime_record.danger_level;
            }
        }
    }

    pub fn apply_overlay_records(
        &mut self,
        overlay_records: &[ZoneOverlayRecord],
    ) -> Result<(), String> {
        for overlay_record in overlay_records {
            if overlay_record.payload_version > ZONE_OVERLAY_PAYLOAD_VERSION {
                continue;
            }
            let Some(zone) = self.find_zone_mut(overlay_record.zone_id.as_str()) else {
                continue;
            };

            match overlay_record.overlay_kind.as_str() {
                "collapsed" => {
                    let payload: CollapsedOverlayPayload =
                        serde_json::from_str(&overlay_record.payload_json).map_err(|error| {
                            format!("invalid collapsed overlay payload: {error}")
                        })?;
                    zone.spirit_qi = 0.0;
                    zone.danger_level = payload.danger_level;
                    merge_overlay_events(
                        &mut zone.active_events,
                        vec![COLLAPSED_ZONE_EVENT_NAME.to_string()],
                    );
                    merge_overlay_events(&mut zone.active_events, payload.active_events);
                    merge_overlay_blocked_tiles(&mut zone.blocked_tiles, payload.blocked_tiles);
                }
                "qi_eye_formed" => {
                    let payload: QiEyeOverlayPayload =
                        serde_json::from_str(&overlay_record.payload_json).map_err(|error| {
                            format!("invalid qi_eye_formed overlay payload: {error}")
                        })?;
                    merge_overlay_events(&mut zone.active_events, payload.active_events);
                }
                "ruins_discovered" => {
                    let payload: RuinsDiscoveredOverlayPayload =
                        serde_json::from_str(&overlay_record.payload_json).map_err(|error| {
                            format!("invalid ruins_discovered overlay payload: {error}")
                        })?;
                    merge_overlay_events(&mut zone.active_events, payload.active_events);
                    merge_overlay_blocked_tiles(&mut zone.blocked_tiles, payload.blocked_tiles);
                }
                _ => {}
            }
        }

        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct CollapsedOverlayPayload {
    danger_level: u8,
    #[serde(default)]
    active_events: Vec<String>,
    #[serde(default)]
    blocked_tiles: Vec<[i32; 2]>,
}

#[derive(Debug, Deserialize)]
struct QiEyeOverlayPayload {
    #[serde(default)]
    active_events: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RuinsDiscoveredOverlayPayload {
    #[serde(default)]
    active_events: Vec<String>,
    #[serde(default)]
    blocked_tiles: Vec<[i32; 2]>,
}

fn merge_overlay_events(target: &mut Vec<String>, additions: Vec<String>) {
    for event_name in additions {
        if !target.iter().any(|existing| existing == &event_name) {
            target.push(event_name);
        }
    }
}

fn merge_overlay_blocked_tiles(target: &mut Vec<(i32, i32)>, additions: Vec<[i32; 2]>) {
    for [x, z] in additions {
        let tile = (x, z);
        if !target.iter().any(|existing| existing == &tile) {
            target.push(tile);
        }
    }
}

impl Zone {
    pub fn botany_tags(&self) -> Vec<BotanyZoneTag> {
        let mut tags = Vec::new();
        if self.name.eq_ignore_ascii_case("spawn") {
            tags.push(BotanyZoneTag::Plains);
        }
        if self.name.eq_ignore_ascii_case("qingyun_peaks") {
            tags.push(BotanyZoneTag::Mountain);
        }
        if self.name.eq_ignore_ascii_case("lingquan_marsh") {
            tags.push(BotanyZoneTag::Marsh);
        }
        if self.name.eq_ignore_ascii_case("blood_valley") {
            tags.push(BotanyZoneTag::BloodValley);
        }
        if self.name.eq_ignore_ascii_case("youan_depths") {
            tags.push(BotanyZoneTag::Cave);
        }
        if self.name.eq_ignore_ascii_case("north_wastes") {
            tags.push(BotanyZoneTag::Wastes);
        }

        // 负灵域判定（plan §1.1 特殊路径）：spirit_qi 持续低于 -0.2 即视为负灵域，
        // 可让 shi_mai_gen / fu_chen_cao / zhong_yan_teng 等 NegativeField 植物
        // 走 ZoneRefresh / StaticPoint 生长链（event-triggered 植物不受此影响）。
        if self.spirit_qi < -0.2 {
            tags.push(BotanyZoneTag::NegativeField);
        }

        if tags.is_empty() {
            tags.push(BotanyZoneTag::Plains);
        }

        tags
    }

    pub fn supports_botany_tag(&self, tag: BotanyZoneTag) -> bool {
        self.botany_tags().contains(&tag)
    }
}

#[derive(Debug, Deserialize)]
struct ZonesFileConfig {
    zones: Vec<ZoneConfig>,
}

#[derive(Debug, Deserialize)]
struct ZoneConfig {
    name: String,
    /// Dimension; defaults to overworld for backwards-compat with pre-TSY snapshots.
    #[serde(default)]
    dimension: DimensionKind,
    aabb: ZoneAabbConfig,
    spirit_qi: f64,
    danger_level: u8,
    #[serde(default)]
    active_events: Vec<String>,
    #[serde(default)]
    patrol_anchors: Vec<[f64; 3]>,
    #[serde(default)]
    blocked_tiles: Vec<[i32; 2]>,
}

#[derive(Debug, Deserialize)]
struct ZoneAabbConfig {
    min: [f64; 3],
    max: [f64; 3],
}

impl TryFrom<ZonesFileConfig> for ZoneRegistry {
    type Error = String;

    fn try_from(config: ZonesFileConfig) -> Result<Self, Self::Error> {
        if config.zones.is_empty() {
            return Err("zones list cannot be empty".to_string());
        }

        let mut seen_names = HashSet::new();
        let mut saw_spawn = false;
        let mut zones = Vec::with_capacity(config.zones.len());

        for zone_config in config.zones {
            let zone = validate_zone(zone_config, &mut seen_names)?;
            if zone.name == DEFAULT_SPAWN_ZONE_NAME {
                saw_spawn = true;
            }
            zones.push(zone);
        }

        if !saw_spawn {
            return Err(format!(
                "zones config must include a `{DEFAULT_SPAWN_ZONE_NAME}` zone to preserve spawn fallback semantics"
            ));
        }

        Ok(Self { zones })
    }
}

fn validate_zone(zone: ZoneConfig, seen_names: &mut HashSet<String>) -> Result<Zone, String> {
    let name = zone.name.trim();
    if name.is_empty() {
        return Err("zone name cannot be empty".to_string());
    }

    if !seen_names.insert(name.to_string()) {
        return Err(format!("duplicate zone name `{name}`"));
    }

    if !zone.spirit_qi.is_finite()
        || !(MIN_ZONE_SPIRIT_QI..=MAX_ZONE_SPIRIT_QI).contains(&zone.spirit_qi)
    {
        return Err(format!(
            "zone `{name}` spirit_qi must be a finite value within [{MIN_ZONE_SPIRIT_QI}, {MAX_ZONE_SPIRIT_QI}]"
        ));
    }

    if zone.danger_level > MAX_ZONE_DANGER_LEVEL {
        return Err(format!(
            "zone `{name}` danger_level must be within [0, {MAX_ZONE_DANGER_LEVEL}]"
        ));
    }

    let min = validate_dvec3(zone.aabb.min, format!("zone `{name}` aabb.min"))?;
    let max = validate_dvec3(zone.aabb.max, format!("zone `{name}` aabb.max"))?;
    if min.x > max.x || min.y > max.y || min.z > max.z {
        return Err(format!(
            "zone `{name}` has invalid aabb bounds: min must not exceed max"
        ));
    }

    for event_name in &zone.active_events {
        if event_name.trim().is_empty() {
            return Err(format!("zone `{name}` contains an empty active event name"));
        }
    }

    let mut patrol_anchors = Vec::with_capacity(zone.patrol_anchors.len());
    for (index, anchor) in zone.patrol_anchors.into_iter().enumerate() {
        let anchor = validate_dvec3(anchor, format!("zone `{name}` patrol_anchors[{index}]"))?;
        if !contains_bounds((min, max), anchor) {
            return Err(format!(
                "zone `{name}` patrol_anchors[{index}] must stay within the zone aabb"
            ));
        }
        patrol_anchors.push(anchor);
    }

    let mut seen_blocked_tiles = HashSet::new();
    let mut blocked_tiles = Vec::with_capacity(zone.blocked_tiles.len());
    for (index, [x, z]) in zone.blocked_tiles.into_iter().enumerate() {
        if !contains_horizontal_bounds((min, max), x, z) {
            return Err(format!(
                "zone `{name}` blocked_tiles[{index}] must stay within the zone aabb"
            ));
        }

        let tile = (x, z);
        if !seen_blocked_tiles.insert(tile) {
            return Err(format!(
                "zone `{name}` contains duplicate blocked_tiles entry ({x}, {z})"
            ));
        }

        blocked_tiles.push(tile);
    }

    for (index, anchor) in patrol_anchors.iter().enumerate() {
        let anchor_tile = (anchor.x.floor() as i32, anchor.z.floor() as i32);
        if seen_blocked_tiles.contains(&anchor_tile) {
            return Err(format!(
                "zone `{name}` patrol_anchors[{index}] must not overlap blocked_tiles"
            ));
        }
    }

    Ok(Zone {
        name: name.to_string(),
        dimension: zone.dimension,
        bounds: (min, max),
        spirit_qi: zone.spirit_qi,
        danger_level: zone.danger_level,
        active_events: zone.active_events,
        patrol_anchors,
        blocked_tiles,
    })
}

fn validate_dvec3(value: [f64; 3], field_name: String) -> Result<DVec3, String> {
    if !value.into_iter().all(f64::is_finite) {
        return Err(format!("{field_name} must contain only finite numbers"));
    }

    Ok(dvec3_from_array(value))
}

fn contains_bounds(bounds: (DVec3, DVec3), pos: DVec3) -> bool {
    let (min, max) = bounds;

    pos.x >= min.x
        && pos.x <= max.x
        && pos.y >= min.y
        && pos.y <= max.y
        && pos.z >= min.z
        && pos.z <= max.z
}

fn contains_horizontal_bounds(bounds: (DVec3, DVec3), x: i32, z: i32) -> bool {
    let (min, max) = bounds;

    f64::from(x) >= min.x && f64::from(x) <= max.x && f64::from(z) >= min.z && f64::from(z) <= max.z
}

pub fn default_spawn_bounds() -> (DVec3, DVec3) {
    (
        dvec3_from_array(DEFAULT_SPAWN_BOUNDS_MIN),
        DVec3::new(
            f64::from(TEST_AREA_BLOCK_EXTENT / 2),
            DEFAULT_SPAWN_BOUNDS_MAX_Y,
            f64::from(TEST_AREA_BLOCK_EXTENT / 2),
        ),
    )
}

pub fn register(app: &mut App) {
    tracing::info!("[bong][world] registering zone registry startup system");
    app.add_systems(Startup, initialize_zone_registry);
}

fn initialize_zone_registry(mut commands: Commands) {
    let registry = ZoneRegistry::load();

    tracing::info!(
        "[bong][world] initialized zone registry with {} zone(s)",
        registry.zones.len()
    );

    commands.insert_resource(registry);
}
fn dvec3_from_array(value: [f64; 3]) -> DVec3 {
    DVec3::new(value[0], value[1], value[2])
}

#[cfg(test)]
mod zone_tests {
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};
    use crate::persistence::{ZoneOverlayRecord, ZoneRuntimeRecord, ZONE_OVERLAY_PAYLOAD_VERSION};
    use valence::prelude::DVec3;

    #[test]
    fn loads_zones_json_with_fallback() {
        let valid_path = unique_temp_path("bong-zones-valid", ".json");
        fs::write(
            &valid_path,
            r#"{
  "zones": [
    {
      "name": "spawn",
      "aabb": {
        "min": [0.0, 64.0, 0.0],
        "max": [32.0, 80.0, 32.0]
      },
      "spirit_qi": 0.9,
      "danger_level": 0,
      "active_events": [],
      "patrol_anchors": [
        [14.0, 66.0, 14.0],
        [18.0, 66.0, 18.0]
      ],
      "blocked_tiles": [
        [15, 14],
        [16, 14]
      ]
    },
    {
      "name": "blood_valley",
      "aabb": {
        "min": [100.0, 64.0, 100.0],
        "max": [120.0, 80.0, 120.0]
      },
      "spirit_qi": -0.35,
      "danger_level": 4,
      "active_events": ["beast_tide"],
      "patrol_anchors": [
        [104.0, 66.0, 104.0]
      ],
      "blocked_tiles": [
        [106, 104]
      ]
    }
  ]
}"#,
        )
        .expect("valid zones.json fixture should be writable");

        let registry = ZoneRegistry::load_from_path(&valid_path);
        let spawn = registry
            .find_zone(
                crate::world::dimension::DimensionKind::Overworld,
                DVec3::new(14.0, 66.0, 14.0),
            )
            .expect("valid config should load spawn zone");
        let blood_valley = registry
            .find_zone(
                crate::world::dimension::DimensionKind::Overworld,
                DVec3::new(110.0, 66.0, 110.0),
            )
            .expect("valid config should load blood_valley zone");

        assert_eq!(registry.zones.len(), 2);
        assert_eq!(spawn.name, DEFAULT_SPAWN_ZONE_NAME);
        assert_eq!(spawn.patrol_anchors.len(), 2);
        assert_eq!(spawn.patrol_anchors[0], DVec3::new(14.0, 66.0, 14.0));
        assert_eq!(spawn.blocked_tiles, vec![(15, 14), (16, 14)]);
        assert_eq!(blood_valley.name, "blood_valley");
        assert_eq!(blood_valley.spirit_qi, -0.35);
        assert_eq!(blood_valley.danger_level, 4);
        assert_eq!(blood_valley.active_events, vec!["beast_tide".to_string()]);
        assert_eq!(
            blood_valley.patrol_anchors,
            vec![DVec3::new(104.0, 66.0, 104.0)]
        );
        assert_eq!(blood_valley.blocked_tiles, vec![(106, 104)]);

        let fallback_path = unique_temp_path("bong-zones-missing", ".json");
        let fallback_registry = ZoneRegistry::load_from_path(&fallback_path);
        assert_eq!(fallback_registry.zones.len(), 1);
        assert_eq!(fallback_registry.zones[0].name, DEFAULT_SPAWN_ZONE_NAME);
    }

    #[test]
    fn zones_json_without_dimension_field_defaults_to_overworld() {
        // Backwards-compat: pre-TSY zones.json snapshots have no `dimension` key.
        // `#[serde(default)]` on `ZoneConfig::dimension` must yield Overworld.
        use crate::world::dimension::DimensionKind;
        let path = unique_temp_path("bong-zones-default-dim", ".json");
        fs::write(
            &path,
            r#"{
  "zones": [
    {
      "name": "spawn",
      "aabb": { "min": [0.0, 64.0, 0.0], "max": [32.0, 80.0, 32.0] },
      "spirit_qi": 0.9,
      "danger_level": 0
    }
  ]
}"#,
        )
        .expect("fixture should be writable");
        let registry = ZoneRegistry::load_from_path(&path);
        assert_eq!(registry.zones.len(), 1);
        assert_eq!(registry.zones[0].dimension, DimensionKind::Overworld);
    }

    #[test]
    fn zones_json_with_explicit_tsy_dimension_loads_correctly() {
        use crate::world::dimension::DimensionKind;
        let path = unique_temp_path("bong-zones-explicit-tsy", ".json");
        fs::write(
            &path,
            r#"{
  "zones": [
    {
      "name": "spawn",
      "dimension": "overworld",
      "aabb": { "min": [0.0, 64.0, 0.0], "max": [32.0, 80.0, 32.0] },
      "spirit_qi": 0.9,
      "danger_level": 0
    },
    {
      "name": "tsy_test",
      "dimension": "tsy",
      "aabb": { "min": [-100.0, 0.0, -100.0], "max": [100.0, 128.0, 100.0] },
      "spirit_qi": -0.5,
      "danger_level": 5
    }
  ]
}"#,
        )
        .expect("fixture should be writable");
        let registry = ZoneRegistry::load_from_path(&path);
        assert_eq!(registry.zones.len(), 2);
        let tsy_zone = registry
            .zones
            .iter()
            .find(|z| z.name == "tsy_test")
            .expect("tsy_test zone should be present");
        assert_eq!(tsy_zone.dimension, DimensionKind::Tsy);
    }

    #[test]
    fn find_zone_filters_by_dimension() {
        use crate::world::dimension::DimensionKind;
        let path = unique_temp_path("bong-zones-find-by-dim", ".json");
        fs::write(
            &path,
            r#"{
  "zones": [
    {
      "name": "spawn",
      "aabb": { "min": [0.0, 64.0, 0.0], "max": [32.0, 80.0, 32.0] },
      "spirit_qi": 0.9,
      "danger_level": 0
    },
    {
      "name": "tsy_overlap",
      "dimension": "tsy",
      "aabb": { "min": [0.0, 64.0, 0.0], "max": [32.0, 80.0, 32.0] },
      "spirit_qi": -0.5,
      "danger_level": 5
    }
  ]
}"#,
        )
        .expect("fixture should be writable");
        let registry = ZoneRegistry::load_from_path(&path);
        // Same XYZ, but `find_zone` must return only the matching dimension.
        let pos = DVec3::new(8.0, 66.0, 8.0);
        let overworld_match = registry
            .find_zone(DimensionKind::Overworld, pos)
            .expect("overworld query should find spawn");
        let tsy_match = registry
            .find_zone(DimensionKind::Tsy, pos)
            .expect("tsy query should find tsy_overlap");
        assert_eq!(overworld_match.name, "spawn");
        assert_eq!(tsy_match.name, "tsy_overlap");
    }

    #[test]
    fn accepts_zone_spirit_qi_at_full_negative_bound() {
        let valid_path = unique_temp_path("bong-zones-negative-bound", ".json");
        fs::write(
            &valid_path,
            r#"{
  "zones": [
    {
      "name": "spawn",
      "aabb": {
        "min": [0.0, 64.0, 0.0],
        "max": [32.0, 80.0, 32.0]
      },
      "spirit_qi": -1.0,
      "danger_level": 0,
      "active_events": [],
      "patrol_anchors": [],
      "blocked_tiles": []
    }
  ]
}"#,
        )
        .expect("negative bound zones.json fixture should be writable");

        let registry = ZoneRegistry::load_from_path(&valid_path);
        assert_eq!(registry.zones.len(), 1);
        assert_eq!(registry.zones[0].spirit_qi, -1.0);
    }

    #[test]
    fn rejects_zone_spirit_qi_below_negative_bound() {
        let invalid_path = unique_temp_path("bong-zones-below-negative-bound", ".json");
        fs::write(
            &invalid_path,
            r#"{
  "zones": [
    {
      "name": "spawn",
      "aabb": {
        "min": [0.0, 64.0, 0.0],
        "max": [32.0, 80.0, 32.0]
      },
      "spirit_qi": -1.01,
      "danger_level": 0,
      "active_events": [],
      "patrol_anchors": [],
      "blocked_tiles": []
    }
  ]
}"#,
        )
        .expect("invalid negative bound zones.json fixture should be writable");

        let registry = ZoneRegistry::load_from_path(&invalid_path);
        assert_eq!(registry.zones.len(), 1);
        assert_eq!(registry.zones[0].name, DEFAULT_SPAWN_ZONE_NAME);
        assert_eq!(registry.zones[0].spirit_qi, super::DEFAULT_SPAWN_SPIRIT_QI);
    }

    #[test]
    fn accepts_zone_spirit_qi_at_positive_bound() {
        let valid_path = unique_temp_path("bong-zones-positive-bound", ".json");
        fs::write(
            &valid_path,
            r#"{
  "zones": [
    {
      "name": "spawn",
      "aabb": {
        "min": [0.0, 64.0, 0.0],
        "max": [32.0, 80.0, 32.0]
      },
      "spirit_qi": 1.0,
      "danger_level": 0,
      "active_events": [],
      "patrol_anchors": [],
      "blocked_tiles": []
    }
  ]
}"#,
        )
        .expect("positive bound zones.json fixture should be writable");

        let registry = ZoneRegistry::load_from_path(&valid_path);
        assert_eq!(registry.zones.len(), 1);
        assert_eq!(registry.zones[0].spirit_qi, 1.0);
    }

    #[test]
    fn apply_runtime_records_overrides_only_known_zones() {
        let mut registry = ZoneRegistry::fallback();
        registry.apply_runtime_records(&[
            ZoneRuntimeRecord {
                zone_id: DEFAULT_SPAWN_ZONE_NAME.to_string(),
                spirit_qi: -0.2,
                danger_level: 3,
            },
            ZoneRuntimeRecord {
                zone_id: "missing".to_string(),
                spirit_qi: 0.8,
                danger_level: 5,
            },
        ]);

        assert_eq!(registry.zones.len(), 1);
        assert_eq!(registry.zones[0].name, DEFAULT_SPAWN_ZONE_NAME);
        assert_eq!(registry.zones[0].spirit_qi, -0.2);
        assert_eq!(registry.zones[0].danger_level, 3);
    }

    #[test]
    fn apply_overlay_records_merges_supported_overlay_payloads() {
        let mut registry = ZoneRegistry::fallback();
        registry.zones[0].spirit_qi = 0.8;
        registry
            .apply_overlay_records(&[
                ZoneOverlayRecord {
                    zone_id: DEFAULT_SPAWN_ZONE_NAME.to_string(),
                    overlay_kind: "collapsed".to_string(),
                    payload_json: serde_json::json!({
                        "danger_level": 4,
                        "zone_status": "collapsed",
                        "blocked_tiles": [[1, 2], [3, 4]],
                    })
                    .to_string(),
                    payload_version: ZONE_OVERLAY_PAYLOAD_VERSION,
                    since_wall: 100,
                },
                ZoneOverlayRecord {
                    zone_id: DEFAULT_SPAWN_ZONE_NAME.to_string(),
                    overlay_kind: "qi_eye_formed".to_string(),
                    payload_json: serde_json::json!({
                        "active_events": ["qi_eye_formed"],
                    })
                    .to_string(),
                    payload_version: ZONE_OVERLAY_PAYLOAD_VERSION,
                    since_wall: 101,
                },
                ZoneOverlayRecord {
                    zone_id: DEFAULT_SPAWN_ZONE_NAME.to_string(),
                    overlay_kind: "ruins_discovered".to_string(),
                    payload_json: serde_json::json!({
                        "active_events": ["ruins_discovered"],
                        "blocked_tiles": [[5, 6]],
                    })
                    .to_string(),
                    payload_version: ZONE_OVERLAY_PAYLOAD_VERSION,
                    since_wall: 102,
                },
            ])
            .expect("overlay application should succeed");

        assert_eq!(registry.zones[0].spirit_qi, 0.0);
        assert_eq!(registry.zones[0].danger_level, 4);
        assert_eq!(
            registry.zones[0].active_events,
            vec![
                "realm_collapse".to_string(),
                "qi_eye_formed".to_string(),
                "ruins_discovered".to_string(),
            ]
        );
        assert_eq!(
            registry.zones[0].blocked_tiles,
            vec![(1, 2), (3, 4), (5, 6)]
        );
    }

    #[test]
    fn botany_tags_are_derived_from_zone_name_without_biome_field() {
        let registry =
            ZoneRegistry::load_from_path(Path::new(env!("CARGO_MANIFEST_DIR")).join("zones.json"));

        let spawn = registry
            .find_zone_by_name("spawn")
            .expect("spawn zone should exist");
        assert!(spawn.supports_botany_tag(super::BotanyZoneTag::Plains));

        let marsh = registry
            .find_zone_by_name("lingquan_marsh")
            .expect("lingquan_marsh should exist");
        assert!(marsh.supports_botany_tag(super::BotanyZoneTag::Marsh));

        let blood = registry
            .find_zone_by_name("blood_valley")
            .expect("blood_valley should exist");
        assert!(blood.supports_botany_tag(super::BotanyZoneTag::BloodValley));
    }

    fn unique_temp_path(prefix: &str, suffix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("current time should be after unix epoch")
            .as_nanos();

        std::env::temp_dir().join(format!("{prefix}-{nanos}{suffix}"))
    }

    // ----- plan-woliu-path-v1 §7.2 — wangyintai zone pin tests -----

    #[test]
    fn wangyintai_zone_loads_from_zones_json() {
        let registry =
            ZoneRegistry::load_from_path(Path::new(env!("CARGO_MANIFEST_DIR")).join("zones.json"));
        let zone = registry
            .find_zone_by_name("wangyintai")
            .expect("wangyintai zone should exist in zones.json");

        assert_eq!(zone.name, "wangyintai", "zone name must be wangyintai");
        assert_eq!(
            zone.dimension,
            crate::world::dimension::DimensionKind::Overworld,
            "wangyintai is an overworld zone"
        );
    }

    #[test]
    fn wangyintai_spirit_qi_is_negative() {
        let registry =
            ZoneRegistry::load_from_path(Path::new(env!("CARGO_MANIFEST_DIR")).join("zones.json"));
        let zone = registry
            .find_zone_by_name("wangyintai")
            .expect("wangyintai zone should exist");

        assert!(
            (zone.spirit_qi - (-0.15)).abs() < f64::EPSILON,
            "wangyintai spirit_qi should be -0.15 (negative qi edge), got {}",
            zone.spirit_qi
        );
    }

    #[test]
    fn wangyintai_danger_level_is_3() {
        let registry =
            ZoneRegistry::load_from_path(Path::new(env!("CARGO_MANIFEST_DIR")).join("zones.json"));
        let zone = registry
            .find_zone_by_name("wangyintai")
            .expect("wangyintai zone should exist");

        assert_eq!(
            zone.danger_level, 3,
            "wangyintai danger_level should be 3 (medium risk), got {}",
            zone.danger_level
        );
    }

    #[test]
    fn wangyintai_bounds_match_plan_spec() {
        let registry =
            ZoneRegistry::load_from_path(Path::new(env!("CARGO_MANIFEST_DIR")).join("zones.json"));
        let zone = registry
            .find_zone_by_name("wangyintai")
            .expect("wangyintai zone should exist");

        // plan-zone-registry-coverage-v1 §P2 — moved off the blood_valley AABB overlap.
        let (min, max) = zone.bounds;
        assert!(
            (min.x - 3500.0).abs() < f64::EPSILON
                && (min.y - 40.0).abs() < f64::EPSILON
                && (min.z - (-2150.0)).abs() < f64::EPSILON,
            "wangyintai min bounds should be (3500, 40, -2150), got ({}, {}, {})",
            min.x,
            min.y,
            min.z
        );
        assert!(
            (max.x - 4500.0).abs() < f64::EPSILON
                && (max.y - 200.0).abs() < f64::EPSILON
                && (max.z - (-1150.0)).abs() < f64::EPSILON,
            "wangyintai max bounds should be (4500, 200, -1150), got ({}, {}, {})",
            max.x,
            max.y,
            max.z
        );
    }

    #[test]
    fn wangyintai_patrol_anchors_within_bounds() {
        let registry =
            ZoneRegistry::load_from_path(Path::new(env!("CARGO_MANIFEST_DIR")).join("zones.json"));
        let zone = registry
            .find_zone_by_name("wangyintai")
            .expect("wangyintai zone should exist");

        assert_eq!(
            zone.patrol_anchors.len(),
            2,
            "wangyintai should have 2 patrol anchors per plan spec, got {}",
            zone.patrol_anchors.len()
        );
        for (i, anchor) in zone.patrol_anchors.iter().enumerate() {
            assert!(
                zone.contains(*anchor),
                "wangyintai patrol_anchor[{}] at ({}, {}, {}) must be within zone bounds",
                i,
                anchor.x,
                anchor.y,
                anchor.z
            );
        }
    }

    #[test]
    fn wangyintai_center_resolves_to_zone() {
        let registry =
            ZoneRegistry::load_from_path(Path::new(env!("CARGO_MANIFEST_DIR")).join("zones.json"));

        // Center of the zone AABB (moved off blood_valley overlap, plan-zone-registry-coverage-v1).
        let center = DVec3::new(4000.0, 120.0, -1650.0);
        let found = registry
            .find_zone(crate::world::dimension::DimensionKind::Overworld, center)
            .expect("center point should resolve to wangyintai zone");
        assert_eq!(found.name, "wangyintai");
    }

    #[test]
    fn wangyintai_is_not_tsy() {
        let registry =
            ZoneRegistry::load_from_path(Path::new(env!("CARGO_MANIFEST_DIR")).join("zones.json"));
        let zone = registry
            .find_zone_by_name("wangyintai")
            .expect("wangyintai zone should exist");
        assert!(!zone.is_tsy(), "wangyintai is an overworld zone, not TSY");
    }

    // ----- plan-zone-registry-coverage-v1 — coverage + nested-zone resolution -----

    /// zones.json 必须覆盖全部 overworld 蓝图 zone。任一缺失即撞红，提醒同步
    /// `server/zones.worldview.example.json` 的漂移（运行时权威源是 zones.json）。
    #[test]
    fn zones_json_covers_all_overworld_blueprint_zones() {
        let registry =
            ZoneRegistry::load_from_path(Path::new(env!("CARGO_MANIFEST_DIR")).join("zones.json"));
        let expected = [
            "spawn",
            "qingyun_peaks",
            "lingquan_marsh",
            "blood_valley",
            "youan_depths",
            "north_wastes",
            "giant_sword_sea",
            "dan_zong_yi_yuan",
            "wangyintai",
            "baolongwang_cavern_deep",
            "celestial_isles",
            "south_ash_dead_zone",
            "zhanhun_plain",
            "wuxing_abyss",
            "blood_valley_east_scorch",
            "north_waste_east_scorch",
            "drift_scorch_001",
            "rift_mouth_north_001",
            "rift_mouth_north_002",
            "rift_mouth_blood_001",
            "rift_mouth_west_001",
            "jiuzong_bloodstream_ruin",
            "jiuzong_beiling_ruin",
            "jiuzong_nanyuan_ruin",
            "jiuzong_chixia_ruin",
            "jiuzong_xuanshui_ruin",
            "jiuzong_taichu_ruin",
            "jiuzong_youan_ruin",
        ];
        for name in expected {
            assert!(
                registry.find_zone_by_name(name).is_some(),
                "zones.json must contain overworld zone `{name}` (regen from blueprint if missing)"
            );
        }
        assert_eq!(
            registry.zones.len(),
            expected.len(),
            "zones.json overworld zone count should match the pinned blueprint set"
        );
    }

    /// find_zone 在嵌套 / 重叠 zone 命中时返回 AABB 体积最小（最具体）的 zone。
    #[test]
    fn find_zone_returns_smallest_containing_zone() {
        use super::Zone;
        use crate::world::dimension::DimensionKind;
        let big = Zone {
            name: "big".to_string(),
            dimension: DimensionKind::Overworld,
            bounds: (DVec3::new(0.0, 0.0, 0.0), DVec3::new(100.0, 100.0, 100.0)),
            spirit_qi: 0.0,
            danger_level: 1,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
        };
        let small = Zone {
            name: "small".to_string(),
            dimension: DimensionKind::Overworld,
            bounds: (DVec3::new(40.0, 40.0, 40.0), DVec3::new(60.0, 60.0, 60.0)),
            spirit_qi: 0.0,
            danger_level: 1,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
        };
        // `big` is registered first; without smallest-AABB selection, find_zone
        // would return it for points inside the nested `small` zone.
        let registry = ZoneRegistry {
            zones: vec![big, small],
        };
        assert_eq!(
            registry
                .find_zone(DimensionKind::Overworld, DVec3::new(50.0, 50.0, 50.0))
                .map(|z| z.name.as_str()),
            Some("small"),
            "nested overlap must resolve to the smaller (more specific) zone"
        );
        assert_eq!(
            registry
                .find_zone(DimensionKind::Overworld, DVec3::new(10.0, 10.0, 10.0))
                .map(|z| z.name.as_str()),
            Some("big"),
            "a point only inside the larger zone must resolve to it"
        );
    }

    // ----- plan-tsy-zone-v1 §1.2 / §-1 helper unit tests -----

    fn make_zone(name: &str, dim: crate::world::dimension::DimensionKind) -> super::Zone {
        super::Zone {
            name: name.to_string(),
            dimension: dim,
            bounds: (DVec3::new(0.0, 0.0, 0.0), DVec3::new(10.0, 10.0, 10.0)),
            spirit_qi: 0.0,
            danger_level: 0,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
        }
    }

    #[test]
    fn is_tsy_recognises_prefix() {
        assert!(make_zone(
            "tsy_lingxu_01_shallow",
            crate::world::dimension::DimensionKind::Tsy
        )
        .is_tsy());
        assert!(!make_zone(
            "blood_valley",
            crate::world::dimension::DimensionKind::Overworld
        )
        .is_tsy());
        assert!(!make_zone("", crate::world::dimension::DimensionKind::Overworld).is_tsy());
    }

    #[test]
    fn tsy_depth_parses_layer_suffix() {
        use super::TsyDepth;
        let dim = crate::world::dimension::DimensionKind::Tsy;
        assert_eq!(
            make_zone("tsy_lingxu_01_shallow", dim).tsy_depth(),
            Some(TsyDepth::Shallow)
        );
        assert_eq!(
            make_zone("tsy_lingxu_01_mid", dim).tsy_depth(),
            Some(TsyDepth::Mid)
        );
        assert_eq!(
            make_zone("tsy_lingxu_01_deep", dim).tsy_depth(),
            Some(TsyDepth::Deep)
        );
        // Non-tsy zone returns None even if suffix matches.
        assert_eq!(
            make_zone(
                "foo_shallow",
                crate::world::dimension::DimensionKind::Overworld
            )
            .tsy_depth(),
            None
        );
        // Malformed depth suffix returns None.
        assert_eq!(make_zone("tsy_lingxu_01_abyss", dim).tsy_depth(), None);
    }

    #[test]
    fn tsy_family_id_strips_depth_suffix() {
        let dim = crate::world::dimension::DimensionKind::Tsy;
        assert_eq!(
            make_zone("tsy_lingxu_01_shallow", dim).tsy_family_id(),
            Some("tsy_lingxu_01".to_string())
        );
        assert_eq!(
            make_zone("tsy_a_b_c_deep", dim).tsy_family_id(),
            Some("tsy_a_b_c".to_string())
        );
        // Malformed suffix → None (we refuse to chop arbitrary trailing tokens).
        assert_eq!(make_zone("tsy_lingxu_01_abyss", dim).tsy_family_id(), None);
    }

    #[test]
    fn is_tsy_entry_checks_active_events() {
        let mut z = make_zone(
            "tsy_lingxu_01_shallow",
            crate::world::dimension::DimensionKind::Tsy,
        );
        assert!(!z.is_tsy_entry());
        z.active_events.push("tsy_entry".to_string());
        assert!(z.is_tsy_entry());
    }

    #[test]
    fn register_runtime_zone_appends_unique_zone() {
        let mut registry = ZoneRegistry::fallback();
        let initial_len = registry.zones.len();
        let zone = make_zone(
            "tsy_lingxu_01_shallow",
            crate::world::dimension::DimensionKind::Tsy,
        );
        registry.register_runtime_zone(zone).expect("first add ok");
        assert_eq!(registry.zones.len(), initial_len + 1);
        assert!(registry
            .find_zone_by_name("tsy_lingxu_01_shallow")
            .is_some());
    }

    // ----- plan-territory-v1 P3 — zones_are_adjacent util tests -----

    fn make_zone_at(name: &str, x_min: f64, z_min: f64, x_max: f64, z_max: f64) -> super::Zone {
        super::Zone {
            name: name.to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
            bounds: (
                DVec3::new(x_min, 60.0, z_min),
                DVec3::new(x_max, 80.0, z_max),
            ),
            spirit_qi: 0.5,
            danger_level: 0,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
        }
    }

    #[test]
    fn zones_are_adjacent_touching_within_margin() {
        // zone A: x=[0,100], zone B: x=[200,300], margin=150 → gap=100 < margin → 相邻
        let registry = ZoneRegistry {
            zones: vec![
                make_zone_at("zone_a", 0.0, 0.0, 100.0, 100.0),
                make_zone_at("zone_b", 200.0, 0.0, 300.0, 100.0),
            ],
        };
        assert!(
            registry.zones_are_adjacent("zone_a", "zone_b", 150.0),
            "gap=100 < margin=150 应相邻"
        );
    }

    #[test]
    fn zones_are_adjacent_far_apart_not_adjacent() {
        // zone A: x=[0,100], zone B: x=[400,500], margin=100 → gap=300 > margin → 不相邻
        let registry = ZoneRegistry {
            zones: vec![
                make_zone_at("zone_a", 0.0, 0.0, 100.0, 100.0),
                make_zone_at("zone_c", 400.0, 0.0, 500.0, 100.0),
            ],
        };
        assert!(
            !registry.zones_are_adjacent("zone_a", "zone_c", 100.0),
            "gap=300 > margin=100 不应相邻"
        );
    }

    #[test]
    fn zones_are_adjacent_overlapping() {
        // 两个 zone AABB 直接重叠 → 相邻
        let registry = ZoneRegistry {
            zones: vec![
                make_zone_at("zone_a", 0.0, 0.0, 200.0, 200.0),
                make_zone_at("zone_b", 100.0, 0.0, 300.0, 200.0),
            ],
        };
        assert!(
            registry.zones_are_adjacent("zone_a", "zone_b", 0.0),
            "AABB 重叠（margin=0）应相邻"
        );
    }

    #[test]
    fn zones_are_adjacent_same_name_returns_false() {
        let registry = ZoneRegistry {
            zones: vec![make_zone_at("spawn", 0.0, 0.0, 100.0, 100.0)],
        };
        assert!(
            !registry.zones_are_adjacent("spawn", "spawn", 999.0),
            "相同 zone 名称不应视为相邻（自相邻无意义）"
        );
    }

    #[test]
    fn zones_are_adjacent_missing_zone_returns_false() {
        let registry = ZoneRegistry {
            zones: vec![make_zone_at("zone_a", 0.0, 0.0, 100.0, 100.0)],
        };
        assert!(
            !registry.zones_are_adjacent("zone_a", "nonexistent", 9999.0),
            "找不到 zone 应返回 false"
        );
    }

    #[test]
    fn adjacent_zone_names_returns_neighbors() {
        // A 与 B 相邻（gap=50 < margin=100），A 与 C 不相邻（gap=500 > margin=100）
        let registry = ZoneRegistry {
            zones: vec![
                make_zone_at("zone_a", 0.0, 0.0, 100.0, 100.0),
                make_zone_at("zone_b", 150.0, 0.0, 250.0, 100.0), // gap=50
                make_zone_at("zone_c", 700.0, 0.0, 800.0, 100.0), // gap=600
            ],
        };
        let neighbors = registry.adjacent_zone_names("zone_a", 100.0);
        assert!(
            neighbors.contains(&"zone_b".to_string()),
            "zone_b 应在 zone_a 的相邻列表中"
        );
        assert!(
            !neighbors.contains(&"zone_c".to_string()),
            "zone_c 不应在 zone_a 的相邻列表中"
        );
    }

    #[test]
    fn register_runtime_zone_rejects_duplicate_name() {
        let mut registry = ZoneRegistry::fallback();
        let zone = make_zone(
            "tsy_lingxu_01_shallow",
            crate::world::dimension::DimensionKind::Tsy,
        );
        registry
            .register_runtime_zone(zone.clone())
            .expect("first add ok");
        let err = registry
            .register_runtime_zone(zone)
            .expect_err("duplicate name should be rejected");
        assert!(err.contains("already registered"), "got: {err}");
    }
}
