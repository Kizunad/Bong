use std::collections::HashSet;
use std::fs;
use std::path::Path;

use serde::Deserialize;
use valence::prelude::{
    bevy_ecs, App, Commands, DVec3, IntoSystemConfigs, Resource, Startup, SystemSet,
};

use super::dimension::DimensionKind;
use super::TEST_AREA_BLOCK_EXTENT;
use crate::persistence::{ZoneOverlayRecord, ZoneRuntimeRecord, ZONE_OVERLAY_PAYLOAD_VERSION};

pub const DEFAULT_ZONES_PATH: &str = "zones.json";
pub const DEFAULT_TSY_ZONES_PATH: &str = "zones.tsy.json";
pub const DEFAULT_SPAWN_ZONE_NAME: &str = "spawn";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub(crate) struct ZoneRegistryStartupSet;

const DEFAULT_SPAWN_BOUNDS_MIN: [f64; 3] = [-128.0, 64.0, -128.0];
const DEFAULT_SPAWN_BOUNDS_MAX_Y: f64 = 80.0;
const DEFAULT_SPAWN_SPIRIT_QI: f64 = 0.9;
const DEFAULT_SPAWN_PATROL_ANCHORS: [[f64; 3]; 1] = [[14.0, 66.0, 14.0]];
const MAX_ZONE_DANGER_LEVEL: u8 = 7;
const MIN_ZONE_SPIRIT_QI: f64 = -1.0;
const MAX_ZONE_SPIRIT_QI: f64 = 1.0;
const COLLAPSED_ZONE_EVENT_NAME: &str = "realm_collapse";

/// plan-zone-qi-economy-v1 P1 — `qi_equilibrium` 下限。0.0 = 不参与回流（向后兼容默认值）；
/// 非零值必须落在 `[0.0, MAX_ZONE_SPIRIT_QI]` 内，与 `spirit_qi` 同一浓度量纲。
const MIN_ZONE_QI_EQUILIBRIUM: f64 = 0.0;

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
    /// plan-zone-qi-economy-v1 P1 — 平衡浓度钳位点（`spirit_qi` 同量纲，0..=1）。
    /// `0.0` = 不回流（向后兼容默认值，pre-P1 `zones.json` 快照/测试 fixture 无此字段）。
    /// heartbeat 回流 system 只把 `spirit_qi` 补到这个值即停，绝不过冲。
    pub qi_equilibrium: f64,
    /// plan-zone-qi-economy-v1 P1 — 回流速率（绝对灵气点/分钟，`QI_ZONE_UNIT_CAPACITY`
    /// 换算前的原始单位）。`0.0` = 不回流（向后兼容默认值）。来源是独立待分配池
    /// （`qi_physics::ledger::pending_inflow_account`），绝不凭空创生。
    pub qi_inflow_per_min: f64,
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
            // 这是 zones.json 缺失/校验失败时的硬编码兜底，不是生产配置来源；
            // 保持 P1 向后兼容默认值 0.0（不回流）。真实 spawn 回流参数在 zones.json。
            qi_equilibrium: 0.0,
            qi_inflow_per_min: 0.0,
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
    /// fix-spec-1901-v2 §7.1 — 空间版本号。
    ///
    /// 只在 zone membership、dimension、AABB bounds 或其它 `find_zone` 几何
    /// 输入变化时递增；zone spirit qi、pressure、其它非几何字段更新不得
    /// 递增。`auto_set_plot_zone` 以它作为 pending plot retry 触发器，替代
    /// 粗粒度 `is_changed()`（后者被 heartbeat qi tick 的每 tick mutable
    /// borrow 污染）。
    pub spatial_revision: u64,
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
            spatial_revision: 0,
        }
    }

    pub fn load() -> Self {
        let manifest_dir_path = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut registry = Self::load_from_path(manifest_dir_path.join(DEFAULT_ZONES_PATH));

        let tsy_path = manifest_dir_path.join(DEFAULT_TSY_ZONES_PATH);
        match registry.merge_tsy_blueprint_from_path(&tsy_path) {
            Ok(loaded) if loaded > 0 => {
                tracing::info!(
                    "[bong][world] merged {loaded} TSY blueprint zone(s) from {}",
                    tsy_path.display()
                );
            }
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(
                    "[bong][world] failed to merge TSY blueprint zones from {}: {error}",
                    tsy_path.display()
                );
            }
        }

        registry
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

    /// 按位置查找可变 zone（overworld），返回最小（最具体）AABB 命中的 zone 的可变引用。
    ///
    /// 用于 qi 入账需要直接修改 zone.spirit_qi 的场景（如毒蛊脏真元散回、BossDrain 后续等）。
    /// `dim` 参数指定目标维度；调用方应从 `CurrentDimension` 组件读取实体所在维度后传入，
    /// 而非硬编码 `DimensionKind::Overworld`（否则 Tsy 维度下 zone 查找永远返回 None）。
    pub fn find_zone_mut_by_pos(
        &mut self,
        dim: crate::world::dimension::DimensionKind,
        pos: valence::prelude::DVec3,
    ) -> Option<&mut Zone> {
        // 先找最小命中 zone 的名字（只读阶段），再通过名字拿 &mut（避免借用冲突）
        let name = self
            .zones
            .iter()
            .filter(|zone| zone.dimension == dim && zone.contains(pos))
            .min_by(|a, b| {
                a.aabb_volume()
                    .partial_cmp(&b.aabb_volume())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|zone| zone.name.clone())?;
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
        // fix-spec-1901-v2 §7.1 — membership 变化递增空间 revision。
        self.spatial_revision = self.spatial_revision.wrapping_add(1);
        Ok(())
    }

    fn merge_tsy_blueprint_from_path(&mut self, path: impl AsRef<Path>) -> Result<usize, String> {
        let path = path.as_ref();
        let contents = match fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                tracing::info!(
                    "[bong][world] no TSY blueprint zones config at {}, skipping TSY zone merge",
                    path.display()
                );
                return Ok(0);
            }
            Err(error) => {
                return Err(format!(
                    "failed to read TSY blueprint zones config: {error}"
                ));
            }
        };

        let config: ZonesFileConfig = serde_json::from_str(&contents)
            .map_err(|error| format!("failed to parse TSY blueprint zones config: {error}"))?;
        let supplemental = ZoneRegistry::try_from_config(
            config,
            ZoneConfigPolicy {
                require_spawn: false,
                spirit_qi_floor: SpiritQiFloor::TsyBlueprint,
            },
        )?;
        let loaded = supplemental.zones.len();

        if let Some(zone) = supplemental
            .zones
            .iter()
            .find(|zone| !zone.is_tsy() || zone.dimension != DimensionKind::Tsy)
        {
            return Err(format!(
                "zone `{}` is not a TSY blueprint zone; supplemental merge rejected",
                zone.name
            ));
        }

        if let Some(zone) = supplemental
            .zones
            .iter()
            .find(|zone| self.zones.iter().any(|existing| existing.name == zone.name))
        {
            return Err(format!(
                "zone `{}` already registered; TSY blueprint merge rejected",
                zone.name
            ));
        }

        self.zones.extend(supplemental.zones);
        // fix-spec-1901-v2 §7.1 — TSY blueprint 批量并入是 membership 变化。
        if loaded > 0 {
            self.spatial_revision = self.spatial_revision.wrapping_add(1);
        }

        Ok(loaded)
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
    /// plan-zone-qi-economy-v1 P1 — `#[serde(default)]` = 0.0（向后兼容，pre-P1
    /// `zones.json` 快照缺此字段时不回流）。
    #[serde(default)]
    qi_equilibrium: f64,
    #[serde(default)]
    qi_inflow_per_min: f64,
}

#[derive(Debug, Deserialize)]
struct ZoneAabbConfig {
    min: [f64; 3],
    max: [f64; 3],
}

impl TryFrom<ZonesFileConfig> for ZoneRegistry {
    type Error = String;

    fn try_from(config: ZonesFileConfig) -> Result<Self, Self::Error> {
        Self::try_from_config(
            config,
            ZoneConfigPolicy {
                require_spawn: true,
                spirit_qi_floor: SpiritQiFloor::RuntimeBounded,
            },
        )
    }
}

#[derive(Debug, Clone, Copy)]
struct ZoneConfigPolicy {
    require_spawn: bool,
    spirit_qi_floor: SpiritQiFloor,
}

#[derive(Debug, Clone, Copy)]
enum SpiritQiFloor {
    RuntimeBounded,
    TsyBlueprint,
}

impl ZoneRegistry {
    fn try_from_config(config: ZonesFileConfig, policy: ZoneConfigPolicy) -> Result<Self, String> {
        if config.zones.is_empty() {
            return Err("zones list cannot be empty".to_string());
        }

        let mut seen_names = HashSet::new();
        let mut saw_spawn = false;
        let mut zones = Vec::with_capacity(config.zones.len());

        for zone_config in config.zones {
            let zone = validate_zone(zone_config, &mut seen_names, policy.spirit_qi_floor)?;
            if zone.name == DEFAULT_SPAWN_ZONE_NAME {
                saw_spawn = true;
            }
            zones.push(zone);
        }

        if policy.require_spawn && !saw_spawn {
            return Err(format!(
                "zones config must include a `{DEFAULT_SPAWN_ZONE_NAME}` zone to preserve spawn fallback semantics"
            ));
        }

        Ok(Self {
            zones,
            // fix-spec-1901-v2 §7.1 — 从配置全新加载视为 revision 0（基线）。
            spatial_revision: 0,
        })
    }
}

fn validate_zone(
    zone: ZoneConfig,
    seen_names: &mut HashSet<String>,
    spirit_qi_floor: SpiritQiFloor,
) -> Result<Zone, String> {
    let name = zone.name.trim();
    if name.is_empty() {
        return Err("zone name cannot be empty".to_string());
    }

    if !seen_names.insert(name.to_string()) {
        return Err(format!("duplicate zone name `{name}`"));
    }

    let below_floor = match spirit_qi_floor {
        SpiritQiFloor::RuntimeBounded => zone.spirit_qi < MIN_ZONE_SPIRIT_QI,
        // `zones.tsy.json` is a TSY worldgen/dev blueprint, not the overworld runtime
        // `zones.json` authority. Existing TSY drain design uses values like -1.1/-1.15
        // as collapse pressure inputs, so this supplemental loader only requires finite
        // values and the shared upper bound.
        SpiritQiFloor::TsyBlueprint => false,
    };
    if !zone.spirit_qi.is_finite() || below_floor || zone.spirit_qi > MAX_ZONE_SPIRIT_QI {
        let floor_label = match spirit_qi_floor {
            SpiritQiFloor::RuntimeBounded => MIN_ZONE_SPIRIT_QI.to_string(),
            SpiritQiFloor::TsyBlueprint => "-inf".to_string(),
        };
        return Err(format!(
            "zone `{name}` spirit_qi must be a finite value within [{floor_label}, {MAX_ZONE_SPIRIT_QI}]"
        ));
    }

    // plan-zone-qi-economy-v1 P1 — `qi_equilibrium` 与 `spirit_qi` 同量纲（0..=1），
    // 0.0 是显式"不回流"（向后兼容默认值），非零值必须落在合法浓度区间内。
    if !zone.qi_equilibrium.is_finite()
        || !(MIN_ZONE_QI_EQUILIBRIUM..=MAX_ZONE_SPIRIT_QI).contains(&zone.qi_equilibrium)
    {
        return Err(format!(
            "zone `{name}` qi_equilibrium must be a finite value within [{MIN_ZONE_QI_EQUILIBRIUM}, {MAX_ZONE_SPIRIT_QI}]"
        ));
    }

    // plan-zone-qi-economy-v1 P1 — `qi_inflow_per_min` 是绝对灵气点/分钟，只需非负有限，
    // 无上界（标定值由 zones.json 配置，heartbeat 回流 system 自身按待分配池余额缩量）。
    if !zone.qi_inflow_per_min.is_finite() || zone.qi_inflow_per_min < 0.0 {
        return Err(format!(
            "zone `{name}` qi_inflow_per_min must be a finite non-negative value"
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
        qi_equilibrium: zone.qi_equilibrium,
        qi_inflow_per_min: zone.qi_inflow_per_min,
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
    app.add_systems(
        Startup,
        initialize_zone_registry.in_set(ZoneRegistryStartupSet),
    );
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
#[path = "zone_tests.rs"]
mod zone_tests;
