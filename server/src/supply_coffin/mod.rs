//! plan-supply-coffin-v1 — 巨剑沧海物资棺：开箱即取即碎、真实时间刷新。
//!
//! P0 — 数据模型 + Registry + Loot 表（本文件 + `loot.rs`）。
//! P2 — 交互 + 刷新 tick（`interact.rs` / `refresh.rs`，由 `register()` 接入 Update）。
//!
//! 世界观锚点：`worldview.md §五:402-410` 器修材料；`plan-sword-path-v1` 11 种剑道材料。
//! 物资棺 = 历代探索者死在巨剑沧海的遗物容器；灵气潮汐把深埋棺木冲出地表，
//! 取走内容物后棺木失去灵气支撑迅速风化碎裂。

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, App, DVec3, Entity, Resource};

pub mod loot;

#[cfg(test)]
mod tests;

#[allow(unused_imports)]
pub use loot::{loot_table, roll_count_range, roll_loot, SupplyCoffinLootEntry};

/// 物资棺三档（plan §0 设计轴心 6 + P0.1）。
///
/// 等级越高 loot 越好、`max_active` 越少、`cooldown_secs` 越长。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupplyCoffinGrade {
    /// 松木棺 — 低阶器修消耗品，量大管饱。
    Common,
    /// 漆棺 — 中阶器修关键材料。
    Rare,
    /// 祭坛棺 — 高阶器修珍稀材料；可极低概率含 broken_sword_soul。
    Precious,
}

impl SupplyCoffinGrade {
    /// 三 variant 完整列表，迭代用。
    pub const ALL: [Self; 3] = [Self::Common, Self::Rare, Self::Precious];

    /// zone 内同档活跃上限（plan P0.1: 5 / 2 / 1）。
    pub const fn max_active(self) -> usize {
        match self {
            Self::Common => 5,
            Self::Rare => 2,
            Self::Precious => 1,
        }
    }

    /// 真实世界时间冷却（秒，plan P0.1: 30min / 2h / 6h）。
    ///
    /// 用 wall clock 不用 server tick——避免 `/time advance` 刷物资。
    pub const fn cooldown_secs(self) -> u64 {
        match self {
            Self::Common => 30 * 60,
            Self::Rare => 2 * 60 * 60,
            Self::Precious => 6 * 60 * 60,
        }
    }

    /// schema / 日志 / dev 命令用的 snake_case 名。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Common => "common",
            Self::Rare => "rare",
            Self::Precious => "precious",
        }
    }

    /// dev 命令解析用的反向映射，未识别返回 `None`。
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "common" => Some(Self::Common),
            "rare" => Some(Self::Rare),
            "precious" => Some(Self::Precious),
            _ => None,
        }
    }
}

/// 当前活跃物资棺的运行时记录（entity → 状态）。
#[derive(Debug, Clone, PartialEq)]
pub struct ActiveSupplyCoffin {
    pub grade: SupplyCoffinGrade,
    pub pos: DVec3,
    /// spawn 时的 wall-clock 秒（`current_wall_clock_secs()`）。
    pub spawned_at_wall_secs: u64,
}

/// 冷却中的槽位（碎裂后等待 `cooldown_secs` 才能再次刷新）。
#[derive(Debug, Clone, PartialEq)]
pub struct CoffinCooldown {
    pub grade: SupplyCoffinGrade,
    /// 碎裂时的 wall-clock 秒。`broken_at + grade.cooldown_secs() <= now` 视为到期。
    pub broken_at_wall_secs: u64,
}

impl CoffinCooldown {
    /// 冷却到期判定（plan §P3.2 测试 6：边界为 `broken_at + cooldown <= now`）。
    pub fn is_ready(&self, now_secs: u64) -> bool {
        now_secs
            >= self
                .broken_at_wall_secs
                .saturating_add(self.grade.cooldown_secs())
    }
}

/// 物资棺全局状态（plan P0.2）。Bevy Resource 单例。
#[derive(Debug, Resource)]
pub struct SupplyCoffinRegistry {
    /// 当前场上活跃的物资棺。key = entity id；value 含 grade/pos/spawn 时刻。
    pub active: HashMap<Entity, ActiveSupplyCoffin>,
    /// 冷却队列。FIFO 按 grade 取出最早到期的一个。
    pub cooldowns: Vec<CoffinCooldown>,
    /// 刷新选点的 zone AABB（含 y 高度区间，y 用于将来 ChunkLayer 接入时的搜索上下界）。
    pub zone_aabb: (DVec3, DVec3),
    /// 选点 fallback y 高度（ChunkLayer 集成前的默认地表 y）。
    pub spawn_y: f64,
    /// splitmix64 内部状态——刷新选点 / loot 抽样用，确保单测可重放。
    pub rng_state: u64,
}

impl SupplyCoffinRegistry {
    pub fn new(zone_aabb: (DVec3, DVec3), spawn_y: f64, rng_seed: u64) -> Self {
        Self {
            active: HashMap::new(),
            cooldowns: Vec::new(),
            zone_aabb,
            spawn_y,
            rng_state: rng_seed,
        }
    }

    pub fn active_count(&self, grade: SupplyCoffinGrade) -> usize {
        self.active.values().filter(|a| a.grade == grade).count()
    }

    pub fn insert_active(
        &mut self,
        entity: Entity,
        grade: SupplyCoffinGrade,
        pos: DVec3,
        now_secs: u64,
    ) {
        self.active.insert(
            entity,
            ActiveSupplyCoffin {
                grade,
                pos,
                spawned_at_wall_secs: now_secs,
            },
        );
    }

    pub fn remove_active(&mut self, entity: Entity) -> Option<ActiveSupplyCoffin> {
        self.active.remove(&entity)
    }

    pub fn enqueue_cooldown(&mut self, grade: SupplyCoffinGrade, now_secs: u64) {
        self.cooldowns.push(CoffinCooldown {
            grade,
            broken_at_wall_secs: now_secs,
        });
    }

    /// 弹出第一个到期的指定 grade 冷却槽位；返回是否弹出成功。
    pub fn pop_ready_cooldown(&mut self, grade: SupplyCoffinGrade, now_secs: u64) -> bool {
        let Some(idx) = self
            .cooldowns
            .iter()
            .position(|c| c.grade == grade && c.is_ready(now_secs))
        else {
            return false;
        };
        self.cooldowns.remove(idx);
        true
    }

    /// 把第一个匹配 grade 的冷却槽 `broken_at` 推迟 `delay_secs`。
    /// 用于刷新选点 20 次都失败后避免下 tick 立刻重试（plan §P2.3）。
    pub fn delay_oldest_cooldown(&mut self, grade: SupplyCoffinGrade, delay_secs: u64) {
        if let Some(c) = self.cooldowns.iter_mut().find(|c| c.grade == grade) {
            c.broken_at_wall_secs = c.broken_at_wall_secs.saturating_add(delay_secs);
        }
    }

    /// 给定一个候选 pos，返回它与最近活跃棺的欧式距离。
    /// 空 registry 返回 `f64::INFINITY`（任何距离都比它近，等价于"无限制"）。
    pub fn min_distance_to_active(&self, pos: DVec3) -> f64 {
        self.active
            .values()
            .map(|a| a.pos.distance(pos))
            .fold(f64::INFINITY, f64::min)
    }

    /// splitmix64 advance —— 返回下一个 u64 随机值。
    pub fn next_rand_u64(&mut self) -> u64 {
        self.rng_state = self.rng_state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.rng_state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// 返回当前 wall-clock 秒（since UNIX_EPOCH）。
///
/// 失败（时钟回拨到 epoch 之前）返回 0——只会让冷却"早一点到期"，无安全后果。
pub fn current_wall_clock_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 注册 Resource —— P2 systems 待 plan P2 阶段挂入 Update schedule。
///
/// 当前只 insert resource，让 P0 测试在主 binary 编译路径下也能运行。
pub fn register(app: &mut App) {
    use crate::world::terrain::sword_sea_xz_bounds;

    let ((min_x, min_z), (max_x, max_z)) = sword_sea_xz_bounds();
    // y 区间：sea_level 上下各留 16 格 buffer，给将来 ChunkLayer ground-height
    // 查询用；当前刷新选点用 `spawn_y` 单 y。
    let zone_aabb = (
        DVec3::new(f64::from(min_x), 48.0, f64::from(min_z)),
        DVec3::new(f64::from(max_x), 96.0, f64::from(max_z)),
    );
    let registry = SupplyCoffinRegistry::new(
        zone_aabb,
        // sea level + 1，对应海面/岸边大致地表；ChunkLayer 接入后会被精确查询取代。
        65.0,
        // 启动种子混入当前 wall clock，多次启服不会撞同一抽序。
        0x5C0F_F1C0_FFEE_u64 ^ current_wall_clock_secs(),
    );
    tracing::info!(
        "[bong][supply_coffin] registered grade caps Common={} Rare={} Precious={} \
         zone_aabb=({:.0},{:.0})..=({:.0},{:.0})",
        SupplyCoffinGrade::Common.max_active(),
        SupplyCoffinGrade::Rare.max_active(),
        SupplyCoffinGrade::Precious.max_active(),
        zone_aabb.0.x,
        zone_aabb.0.z,
        zone_aabb.1.x,
        zone_aabb.1.z,
    );
    app.insert_resource(registry);
}
