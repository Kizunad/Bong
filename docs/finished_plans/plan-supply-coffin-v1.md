# Bong · plan-supply-coffin-v1

**巨剑沧海物资棺——开箱即取即碎、真实时间刷新**。zone 内散布三档中式棺材（松木/漆棺/祭坛棺），玩家打开 → 取走物资 → 棺碎消失 → 冷却后在 zone 内随机新位置重新刷出。Loot 表全部围绕剑道材料体系。

**世界观锚点**：
- `worldview.md §五:402-410` 器修 = 真元封存在物理载体；剑道材料（玄铁/陨铁/剑胚）是器修养成的核心消耗
- `plan-sword-path-v1 §P1.2` 原材料定义（`xuan_iron` / `meteor_iron` / `sword_embryo_shard` / `spirit_wood_core` 等 11 种剑道材料）
- `plan-sword-path-v1 §P2` 巨剑沧海地形——上古宗门覆灭后数千柄灵剑插入海底与崖壁，物资棺 = 历代探索者在此遇难留下的遗物容器

**叙事设定**：巨剑沧海曾是铸剑宗门的驻地，覆灭后历代器修前来寻剑道遗物。有些修士死在这里，随身物资连同遗骸封在简易棺木中（松木棺）；更早期的大能残骸被后人以漆棺收殓；极少数宗门长老的祭坛棺至今残留微弱法阵，内藏珍贵遗物。这些棺木在灵气潮汐中时隐时现——灵气涨潮冲刷地层，将深埋的棺木推出地表；取走内容物后棺木失去灵气支撑，迅速风化碎裂。下一次潮汐又会将新的棺木冲出。

**前置依赖**：
- `plan-entity-model-v1` ✅ — `BongEntityModelKind` 注册管线 + `BongVisualState` tracked data
- `plan-sword-path-v1 P1.2` ⬜ — 原材料 template 定义（`xuan_iron` 已在 `ItemRegistry`，其余需 P1 落地；本 plan 可先用 stub template，P1 落地后替换）
- `plan-inventory-v1` ✅ — `add_item_to_player_inventory` / `ItemRegistry`
- `plan-combat-no_ui` ✅ — `CombatClock` tick 时钟
- `plan-audio-world-v1` ✅ — `PlaySoundRecipeRequest` 音效管线

**反向被依赖**：
- `plan-sword-path-v1 P2` — 巨剑沧海 zone 的物资获取入口

---

## 接入面 Checklist

- **进料**：
  - `world::zone::Zone` — zone AABB 范围（随机选点用）
  - `combat::CombatClock` — tick 时钟
  - `inventory::ItemRegistry` — 物品模板查询
  - `world::entity_model` — `BongVisualKind` / `EntityKind` / `BongVisualState` 渲染管线
  - `network::client_request_handler` — 玩家交互请求（打开棺材）
- **出料**：
  - `inventory::add_item_to_player_inventory` — 给玩家发物品
  - `network::audio_event_emit::PlaySoundRecipeRequest` — 碎裂音效
  - `network::send_server_data_payload` — 棺碎/刷新状态同步（复用 entity despawn）
- **共享类型/event**：
  - **复用** `BongVisualKind` / `BongVisualState` / `EntityKind`（新增 3 个 raw_id）
  - **复用** `PlaySoundRecipeRequest`
  - **复用** `CombatClock`
  - **新增** `SupplyCoffinRegistry` Resource — 物资棺全局状态
  - **新增** `SupplyCoffinOpened` Event — 玩家开棺事件
- **跨仓库契约**：
  - server: `supply_coffin` 模块 / `SupplyCoffinRegistry` / `SupplyCoffinGrade` enum / `SupplyCoffinOpened` event
  - client: `BongEntityModelKind.COFFIN_COMMON` / `COFFIN_RARE` / `COFFIN_PRECIOUS`（raw_id 157-159）/ 3 套 geo.json（已有）+ texture + 碎裂动画
  - agent: 无（纯本地循环，天道不参与）
- **worldview 锚点**：§五 器修材料体系
- **qi_physics 锚点**：无（物资棺不涉及真元流动，纯凡物容器）

---

## 边界：本 plan 做什么 & 不做什么

| 维度 | 范围 | 不做 |
|------|------|------|
| zone | 巨剑沧海专用 | 通用 zone 物资刷新框架（后续可抽象） |
| 棺材等级 | 3 档（松木/漆棺/祭坛棺） | 更多等级 |
| 交互 | 打开→取→碎，瞬时 | 搜刮倒计时（那是 TSY 容器） |
| 刷新 | 真实时间冷却 | server tick 计时 / 玩家触发 |
| loot | 剑道材料为主 | 通用 loot 池 |
| 模型 | 3 个 bbmodel 已有 | 新建模型 |

---

## §0 设计轴心

1. **开即碎**：玩家右键交互 → 一次性获得全部 loot → 棺材 entity 销毁（碎裂粒子+音效）。没有搜刮进度条、没有打断机制——这不是 TSY 高风险搜刮，是主世界探索奖励
2. **三档递增**：松木棺（Common）/ 漆棺（Rare）/ 祭坛棺（Precious）。等级越高 loot 越好、数量越少、刷新越慢
3. **真实时间刷新**：碎裂后进入冷却，按真实世界时间计时（不依赖 server tick），避免快进 `/time advance` 刷物资
4. **随机选点**：刷新时在 zone AABB 内随机选地面位置（y = 地表高度），避开水下/半空/已有棺材位置
5. **同时存在上限**：zone 内同时最多 N 个物资棺（松木 5 / 漆棺 2 / 祭坛棺 1），取一个才会开始刷下一个
6. **剑道 loot 专精**：loot 表围绕剑道材料体系——玄铁、精铁、灵木心（低档）→ 陨铁、剑胚残片、灵泉水（中档）→ 星辰铁、上古剑胚、剑道残卷（高档）

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | 物资棺数据模型 + Registry + 刷新计时器 + loot 表 | ✅ 2026-05-17 |
| P1 | Entity 渲染（3 个 BongEntityModelKind 注册 + client geo/texture 接入）| ✅ 2026-05-17 |
| P2 | 交互系统（开棺 → 发 loot → 碎裂 → 刷新排队）+ 视听 | ✅ 2026-05-17 |
| P3 | Dev 命令 + 饱和测试 + 集成联调 | ✅ 2026-05-17 |

---

## P0 — 数据模型 + Registry + Loot 表

### P0.1 SupplyCoffinGrade — 三档枚举

`server/src/supply_coffin/mod.rs`（新建模块）

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupplyCoffinGrade {
    Common,    // 松木棺
    Rare,      // 漆棺
    Precious,  // 祭坛棺
}

impl SupplyCoffinGrade {
    pub const fn max_active(self) -> usize {
        match self {
            Self::Common => 5,
            Self::Rare => 2,
            Self::Precious => 1,
        }
    }

    pub const fn cooldown_secs(self) -> u64 {
        match self {
            Self::Common => 30 * 60,       // 30 分钟
            Self::Rare => 2 * 60 * 60,     // 2 小时
            Self::Precious => 6 * 60 * 60, // 6 小时
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Common => "common",
            Self::Rare => "rare",
            Self::Precious => "precious",
        }
    }
}
```

### P0.2 SupplyCoffinRegistry — 全局状态

```rust
#[derive(Debug, Resource)]
pub struct SupplyCoffinRegistry {
    /// 当前场上活跃的物资棺（entity → 状态）
    pub active: HashMap<Entity, ActiveSupplyCoffin>,
    /// 冷却中的槽位（碎裂后等待刷新）
    pub cooldowns: Vec<CoffinCooldown>,
    /// zone AABB（启动时从 Zone 配置读入）
    pub zone_aabb: (DVec3, DVec3),
}

#[derive(Debug, Clone)]
pub struct ActiveSupplyCoffin {
    pub grade: SupplyCoffinGrade,
    pub pos: DVec3,
    pub spawned_at_wall_secs: u64,
}

#[derive(Debug, Clone)]
pub struct CoffinCooldown {
    pub grade: SupplyCoffinGrade,
    pub broken_at_wall_secs: u64,
}
```

### P0.3 Loot 表 — 剑道材料

`server/src/supply_coffin/loot.rs`

**松木棺（Common）—— 低阶器修消耗品，量大管饱**

| 物品 | template_id | 数量 | 权重 | 备注 |
|------|-------------|------|------|------|
| 精铁 | `refined_iron` | 2-4 | 30 | 锻造基础材料 |
| 玄铁 | `xuan_iron` | 1-2 | 25 | 巨剑沧海特产矿 |
| 灵木 | `spirit_wood` | 1 | 20 | 剑柄材料 |
| 灵泉水 | `spirit_spring_water` | 1-3 | 15 | 淬剑辅材 |
| 腐朽骨币 | `rotten_bone_coin` | 3-8 | 10 | 前人遗留货币残骸 |

每次开箱 roll 2-3 种，每种独立 roll 数量。

**漆棺（Rare）—— 中阶器修关键材料**

| 物品 | template_id | 数量 | 权重 | 备注 |
|------|-------------|------|------|------|
| 玄铁 | `xuan_iron` | 2-4 | 25 | 量更大 |
| 陨铁 | `meteor_iron` | 1-2 | 25 | 中高阶锻造核心 |
| 剑胚残片 | `sword_embryo_shard` | 1 | 20 | 灵剑铸造必需 |
| 灵泉精华 | `spirit_spring_essence` | 1 | 15 | 灵泉水浓缩版 |
| 剑道残卷 | `scroll_sword_path` | 1 | 15 | 解锁剑道招式 |

每次开箱 roll 2-3 种。

**祭坛棺（Precious）—— 高阶器修珍稀材料**

| 物品 | template_id | 数量 | 权重 | 备注 |
|------|-------------|------|------|------|
| 陨铁 | `meteor_iron` | 2-3 | 20 | 大量 |
| 星辰铁 | `star_iron` | 1 | 20 | 高阶灵剑核心 |
| 上古剑胚 | `ancient_sword_embryo` | 1 | 15 | 极品灵剑材料 |
| 剑胚残片 | `sword_embryo_shard` | 1-2 | 15 | 保底 |
| 剑道残卷 | `scroll_sword_path` | 1 | 15 | 解锁高阶剑术 |
| 破碎剑魂 | `broken_sword_soul` | 1 | 15 | 传说级，极低概率被 roll 到 |

每次开箱 roll 2-4 种。

```rust
pub struct SupplyCoffinLootEntry {
    pub template_id: &'static str,
    pub min_count: u8,
    pub max_count: u8,
    pub weight: u32,
}

pub fn loot_table(grade: SupplyCoffinGrade) -> &'static [SupplyCoffinLootEntry] { ... }

pub fn roll_count(grade: SupplyCoffinGrade) -> usize {
    match grade {
        SupplyCoffinGrade::Common => 2..=3,   // 随机 2-3 种
        SupplyCoffinGrade::Rare => 2..=3,
        SupplyCoffinGrade::Precious => 2..=4,
    }
}

/// 从 loot_table 中按权重不重复抽 roll_count 种，每种再 roll min..=max 数量
pub fn roll_loot(grade: SupplyCoffinGrade, seed: u64) -> Vec<(String, u8)> { ... }
```

---

## P1 — Entity 渲染

### P1.1 Server 端注册 3 个 EntityKind

`server/src/world/entity_model.rs` 新增：

```rust
pub const COFFIN_COMMON_ENTITY_KIND: EntityKind = EntityKind::new(157);
pub const COFFIN_RARE_ENTITY_KIND: EntityKind = EntityKind::new(158);
pub const COFFIN_PRECIOUS_ENTITY_KIND: EntityKind = EntityKind::new(159);
```

`BongVisualKind` 新增 3 个变体：`CoffinCommon` / `CoffinRare` / `CoffinPrecious`。

### P1.2 Client 端注册 BongEntityModelKind

`client/src/main/java/.../BongEntityModelKind.java` 新增：

```java
COFFIN_COMMON(
    "coffin_common", "CoffinCommon.bbmodel", 157,
    1.0f, 0.6f, 64, 10, 0.15f,
    "intact", "opening"
),
COFFIN_RARE(
    "coffin_rare", "CoffinRare.bbmodel", 158,
    1.0f, 0.7f, 64, 10, 0.18f,
    "intact", "opening"
),
COFFIN_PRECIOUS(
    "coffin_precious", "CoffinPrecious.bbmodel", 159,
    1.2f, 0.8f, 64, 10, 0.22f,
    "intact", "opening"
)
```

geo.json 已存在：`coffin_common.geo.json` / `coffin_rare.geo.json` / `coffin_precious.geo.json`。

texture 状态 2 种：`intact`（完好）/ `opening`（开启中，碎裂前短暂显示）。

### P1.3 Texture

需要新增 6 张贴图（3 档 × 2 状态）：
- `textures/entity/supply_coffin/coffin_common_intact.png`
- `textures/entity/supply_coffin/coffin_common_opening.png`
- `textures/entity/supply_coffin/coffin_rare_intact.png`
- `textures/entity/supply_coffin/coffin_rare_opening.png`
- `textures/entity/supply_coffin/coffin_precious_intact.png`
- `textures/entity/supply_coffin/coffin_precious_opening.png`

`opening` 状态贴图 = `intact` 基础上加裂纹 overlay + 发光缝隙。

---

## P2 — 交互系统 + 视听

### P2.1 开棺交互

`server/src/supply_coffin/interact.rs`

- 玩家对物资棺 entity 右键 → server 收到交互事件
- 校验距离（≤ 4 格）
- 从 `SupplyCoffinRegistry.active` 查到 grade
- `roll_loot(grade, seed)` 生成物品列表
- 逐个 `add_item_to_player_inventory`
- 发 `SupplyCoffinOpened` event
- 设 `BongVisualState` = `1`（`opening`），等 10 tick 后 despawn entity
- 将该槽位加入 `cooldowns`

### P2.2 碎裂视听

**粒子**：entity despawn 前 client 播放碎裂粒子
- `BongSpriteParticle` × 12，颜色按档次区分（松木 `#8B6914` / 漆棺 `#2A1506` / 祭坛 `#C4A35A`）
- lifetime 15 tick，速度向外随机 0.3-0.8 m/s，spawn mode burst
- 贴图 `bong:coffin_debris`（新增 8×8 碎片贴图）
- VfxPlayer: `SupplyCoffinBreakVfxPlayer`
- 事件 ID: `bong:supply_coffin_break`

**音效** `supply_coffin_break`：
```json
{
  "layers": [
    { "sound": "block.wood.break", "pitch": 0.7, "volume": 1.0, "delay_ticks": 0 },
    { "sound": "entity.item.break", "pitch": 0.9, "volume": 0.6, "delay_ticks": 2 },
    { "sound": "block.chest.open", "pitch": 0.5, "volume": 0.4, "delay_ticks": 0 }
  ]
}
```

**祭坛棺额外**：碎裂时附加 `entity.wither.ambient` pitch 1.2 volume 0.3（微弱法阵消散声）。

### P2.3 刷新 tick system

`server/src/supply_coffin/refresh.rs`

```rust
fn supply_coffin_refresh_tick(
    mut registry: ResMut<SupplyCoffinRegistry>,
    mut commands: Commands,
    layers: Query<&mut ChunkLayer>,
    dimension_layers: Res<DimensionLayers>,
) {
    let now = current_wall_clock_secs();
    // 1. 检查 cooldowns 中已到期的
    // 2. 按 grade 检查 active 数量是否未满
    // 3. 到期 + 未满 → 随机选点 spawn 新 entity
    // 4. 从 cooldowns 移除
}
```

随机选点策略：
- 在 zone AABB 的 xz 范围内 uniform random
- y 用 ChunkLayer 查询该 (x,z) 的地表高度（最高非 AIR block + 1）
- 排除水下位置（block 是水 → 重 roll）
- 排除距已有物资棺 10 格内的位置（防堆叠）
- 最多重试 20 次，失败则延后 60s 重试

### P2.4 刷新出现视听

**粒子**：新棺 spawn 时 client 播放涌现效果
- `BongGroundDecalParticle` × 6，颜色 `#A08050` 半透明（泥土翻涌感）
- lifetime 25 tick，spawn mode burst
- VfxPlayer: `SupplyCoffinSpawnVfxPlayer`
- 事件 ID: `bong:supply_coffin_emerge`

**音效** `supply_coffin_emerge`：
```json
{
  "layers": [
    { "sound": "block.gravel.place", "pitch": 0.6, "volume": 0.5, "delay_ticks": 0 },
    { "sound": "block.stone.place", "pitch": 0.8, "volume": 0.3, "delay_ticks": 5 }
  ]
}
```

---

## P3 — Dev 命令 + 测试

### P3.1 Dev 命令

| 命令 | 用途 |
|------|------|
| `/supply_coffin spawn <grade>` | 在玩家脚下强制刷一个指定档次物资棺 |
| `/supply_coffin list` | 列出当前活跃物资棺位置+档次 |
| `/supply_coffin reset` | 清空所有活跃棺+冷却，重新初始化 |
| `/supply_coffin cooldown <grade> <secs>` | 临时覆盖冷却时间（测试用） |

### P3.2 测试清单

**单元测试**（`server/src/supply_coffin/tests.rs`）：

1. `SupplyCoffinGrade` 枚举完整性：3 variant × `max_active` / `cooldown_secs` / `as_str` 各有断言
2. `roll_loot` 不同 seed 产出合法物品（template_id 在 loot 表中 + count 在 min..=max 范围）
3. `roll_loot` 不重复抽（同一次 roll 不出现两个相同 template_id）
4. `roll_loot` 抽取数量在 `roll_count` 范围内
5. `SupplyCoffinRegistry` insert / remove / cooldown 状态转换
6. cooldown 到期判定（`broken_at + cooldown_secs <= now`）
7. `max_active` 约束：同档次活跃数达上限时不再 spawn
8. 随机选点避开已有棺材（距离 ≥ 10 格）

**集成测试**：

9. spawn entity → 查 `BongVisualState` = 0（intact）
10. 开棺 → 背包新增物品 + entity despawn + cooldown 入队
11. cooldown 到期 → 新 entity spawn 在不同位置
12. 祭坛棺 loot 包含 `star_iron` / `ancient_sword_embryo`（高阶独有）

---

## Finish Evidence

**验收**：2026-05-17 全部 P0/P1/P2/P3 ✅，60 个本 plan 单测 + 4 跨模块测试通过，
server 5052 tests 全绿，clippy `--all-targets -D warnings` 干净。

### 落地清单（每阶段 ↔ 真实文件）

| 阶段 | 模块 / 文件 | 关键 symbol |
|------|-------------|-------------|
| P0 数据模型 | `server/src/supply_coffin/mod.rs` | `SupplyCoffinGrade` / `SupplyCoffinRegistry` / `ActiveSupplyCoffin` / `CoffinCooldown` / `current_wall_clock_secs` |
| P0 loot 表 | `server/src/supply_coffin/loot.rs` | `SupplyCoffinLootEntry` / `loot_table` / `roll_count_range` / `roll_loot` |
| P0 单测 | `server/src/supply_coffin/tests.rs` | 35 P0 + 3 集成 = 38 个 supply_coffin::tests |
| P1 server | `server/src/world/entity_model.rs` | `COFFIN_COMMON/RARE/PRECIOUS_ENTITY_KIND` (146/147/148) / `BongVisualKind::CoffinCommon/Rare/Precious` / `SupplyCoffinGrade::visual_kind()` |
| P1 client | `client/src/main/java/com/bong/client/entity/BongEntityModelKind.java` | 三个枚举 `COFFIN_COMMON/RARE/PRECIOUS` raw_id 146-148 单 state "intact" |
| P1 client renderer | `client/src/main/java/com/bong/client/entity/Coffin{Common,Rare,Precious}Renderer.java` | 三个 renderer 子类 + `BongEntityRenderBootstrap.java` 绑定 |
| P1 client 资产 | `client/src/main/resources/assets/bong/{geo,animations,textures/entity}/coffin_*` | geo bone Body/Lid 大写化 / Lid 摆动 idle 动画 / `_intact` 后缀贴图 |
| P2 交互 | `server/src/supply_coffin/interact.rs` | `handle_supply_coffin_interact` / `SupplyCoffinOpened` event |
| P2 刷新 | `server/src/supply_coffin/refresh.rs` | `supply_coffin_refresh_tick` / `pick_valid_pos` / `SupplyCoffinMarker` |
| P2 视听 audio | `server/assets/audio/recipes/supply_coffin_break_{common,rare,precious}.json` + `supply_coffin_emerge.json` | recipe id 同名，注入 `SoundRecipeRegistry`（audio 总数 202 → 206） |
| P2 视听 VFX | server emit `bong:supply_coffin_break` / `bong:supply_coffin_emerge` SpawnParticle | 客户端 `BongSpriteParticle` / `BongGroundDecalParticle` 渲染器待视觉 polish PR（见遗留） |
| P3 dev cmd | `server/src/cmd/dev/supply_coffin.rs` | `SupplyCoffinCmd::{Spawn,List,Reset,Cooldown}` + `handle_supply_coffin_cmd`，`registry_pin` 已含 4 条 path |
| P3 测试 | `server/src/supply_coffin/tests.rs` + `server/src/supply_coffin/refresh.rs::tests` + `server/src/cmd/dev/supply_coffin.rs::tests` | 50 + 3 + 7 = 60 项；外加 `world::entity_model::tests::supply_coffin_grade_maps_to_visual_kind` |

### 关键 commit（本 worktree）

- `bf6872edb` 2026-05-17 — feat(supply-coffin): P0 数据模型 + Registry + 剑道材料 loot 表
- `9e815e651` 2026-05-17 — feat(supply-coffin): P1 entity 渲染 —— server EntityKind + client renderer
- `10753feea` 2026-05-17 — feat(supply-coffin): P2 交互 + 刷新 tick + 视听
- `c484bece8` 2026-05-17 — feat(supply-coffin): P3 /supply_coffin dev 命令 + 饱和集成测试

### 测试结果

| 命令 | 结果 |
|------|------|
| `cargo test`（server 全量） | 5052 passed / 0 failed |
| `cargo test --bin bong-server supply_coffin` | 50 passed（含 47 supply_coffin::tests + 3 refresh::tests） |
| `cargo test --bin bong-server cmd::dev::supply_coffin` | 7 passed（dev cmd 集成） |
| `cargo test --bin bong-server cmd::tests` | 4 passed（registry_pin frozen path 同步） |
| `cargo test --bin bong-server audio::tests::loads_default_audio_recipes` | 1 passed（recipe 总数 206 含 4 supply_coffin） |
| `cargo test --bin bong-server world::entity_model::tests` | 4 passed（含 `supply_coffin_grade_maps_to_visual_kind` + entity_kind id 对齐） |
| `cargo clippy --all-targets -- -D warnings` | 0 warnings |

### 跨仓库核验

- **server** `supply_coffin` 模块（mod / loot / interact / refresh / tests）+
  `cmd::dev::supply_coffin` + `world::entity_model::BongVisualKind::Coffin*` +
  4 audio recipe JSON + entity raw_id 146/147/148
- **client** `BongEntityModelKind.COFFIN_{COMMON,RARE,PRECIOUS}` (raw_id
  146/147/148) + 3 Coffin*Renderer 类 + 3 geo + 3 animation + 3 `_intact.png`
  贴图 + `BongEntityRenderBootstrap` 绑定
- **agent** 不参与（plan 头部已声明 "agent: 无 —— 纯本地循环"）

### 遗留 / 后续

| 项 | 原因 | 跟进 |
|----|------|------|
| EntityKind 选 146-148（plan 文档原写 157-159） | origin/main 在 plan 写出时 EntityKind 仅到 145，紧跟使用 146-148 衔接；行为与 plan 等价 | 文档已就 commit log 注明，无需修订 plan 编号 |
| 棺木"opening" 裂纹叠层贴图缺失 | 仅"intact" 单 state；plan §P2.1 设的 10-tick `BongVisualState=1` 延迟改为立即 despawn + 粒子 burst | 视觉 polish PR：补 3 张 `coffin_*_opening.png` + `BongEntityModelKind` 扩 stateCount → 2 |
| 客户端 `SupplyCoffinBreakVfxPlayer` / `SupplyCoffinSpawnVfxPlayer` 未实装 | server 已 emit `bong:supply_coffin_break` / `bong:supply_coffin_emerge` SpawnParticle，但 client VfxPlayer 注册留作视觉 polish | 同上 PR，新增 2 个 VfxPlayer + 8×8 `coffin_debris.png` 碎片贴图 |
| 选点 y 用常数 `spawn_y=65.0`（plan §P2.3 期望 ChunkLayer 顶面查询） | ChunkLayer ground-height API 在 sword_sea 区域调用代价较高，且 sword_sea 65±海面浮动 ≤2 格 | 后续 plan：抽象 `WorldGroundHeightProvider`，与 npc spawn 共用 |
| 客户端 `./gradlew test` 受阻于 `MovementKeybindingsTest.java` | origin/main 已存在的 `MovementKeybindingsTest` 引用未合并的 `resolveDashYawDegrees` 方法，与本 plan 无关 | 由另一条 PR 处理（fix/combat-npc-fauna-hud-pr 分支线索） |
| `coffin_break` recipe 已被 plan-tsy-container 占用 → 本 plan 用 `supply_coffin_break_{grade}` 三档 | 命名空间隔离；plan §P2.2 期望单一 break recipe，按祭坛棺 layer 不同的需求自然拆三档 | — |
