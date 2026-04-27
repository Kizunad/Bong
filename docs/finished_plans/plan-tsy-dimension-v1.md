# TSY Dimension 基础设施 · plan-tsy-dimension-v1

> 把"坍缩渊"落成一个**独立位面**而不是主世界的一块 AABB：注册 Valence `DimensionType` + 建 TSY 专用 `LayerBundle` + 跨位面传送 API + per-dimension `TerrainProvider` + client 侧 dimension 切换。本 plan 是 `plan-tsy-zone-v1` (P0) 和 `plan-tsy-worldgen-v1` (worldgen) 的**共同前置**，不涉及负压 / loot / 塌缩业务逻辑。
> 交叉引用：`worldview.md §十六 世界层实现注`（位面决策源）· `plan-tsy-zone-v1.md §-1/§3`（跨 dim 传送消费方）· `plan-tsy-worldgen-v1.md §-1/§1/§2`（TSY dim 地形消费方）

> **2026-04-24 Audit 升级备忘**：骨架→active。Valence pin `2b705351` 源码审计完成（见 §11）；Q2/Q4/Q5/Q10/§3.3-Q 已决并关闭；§1.1 DimensionType 字段修正（加 `height`、`effects` 改 enum、`monster_spawn_light_level` 填值）；新增 §1.3 `CurrentDimension` component；§2.3 增加 `chat_collector.rs` consumer 备注；§8 规模重估 790 → ~1360 行（find_zone 35+ caller 替换为主因）。

---

## §-1 前提（现有代码基线）

### Valence 多 dimension 模型

Valence 的"dimension"本质上是 Bevy ECS 里的一个 `LayerBundle`（`ChunkLayer` + `EntityLayer`）+ 在 `DimensionTypeRegistry` 里注册一个 `DimensionType` 元数据（决定 min_y/height/ambient_light/有无天空盒等）。玩家通过 `Client::respawn(layer_entity, ...)`（或切换 `VisibleChunkLayer` / `VisibleEntityLayers`）进入不同的位面。

| 能力 | 现状 | 位置 |
|------|------|------|
| `DimensionTypeRegistry` 已引入 | ✅ 仅注册主世界一个 DimensionType | `server/src/world/mod.rs:10,76` |
| 主世界 LayerBundle setup | ✅ 单 layer | `server/src/world/mod.rs:361-379` |
| `TerrainProvider` 单例 | ✅ mmap 加载单份 manifest | `server/src/world/terrain/raster.rs:251,483` |
| `generate_chunks_around_players` chunk 按需生成 | ✅ 单 layer 内跑 | `server/src/world/terrain/mod.rs:109` |
| 跨 dimension 传送 API | ❌ 无 | — |
| 多 `TerrainProvider` 或 per-dim provider routing | ❌ 无 | — |
| Client 侧 dimension 切换处理 | ❌ 未验证（MC 1.20.1 协议 763 `play.Respawn` packet 原生支持） | — |

### 上游决策

- `worldview.md §十六 世界层实现注` 已钉："坍缩渊以独立位面实现，由裂缝传送门进入，不挂在主世界坐标上"
- P0 `plan-tsy-zone-v1` `§-1 点 5`"传送不是跨 dimension"已**被本 plan 反转**——P0 plan 将同步修订

---

## §0 设计轴心（已定稿，不再动）

1. **TSY 是一个共享位面，不按 family 切分 dimension**：所有活坍缩渊 family 共享同一个 TSY `DimensionType` + 同一个 `LayerBundle`，在位面内各占一片 XZ 区域（对齐 worldgen plan §2.2 blueprint AABB，但坐标是 TSY dim 独立坐标系，从 (0,0,0) 或约定原点起排）
2. **主世界 ↔ TSY 是唯一的跨位面对**：本 plan 不为其他内容（冥界、梦境等）准备通用"n 位面"框架；如后续出现新位面，该 plan fork 即可
3. **裂缝锚点存在主世界**：主世界的"裂缝" POI 由 worldgen 写入主世界 blueprint；spawn 为主世界 layer 里的 `RiftPortal` 触发器实体；玩家触发 → 跨位面传送到 TSY dim 的对应 family 浅层中心
4. **per-dimension TerrainProvider**：`TerrainProvider` 从单例升级为 `{DimensionKind → TerrainProvider}` map（或直接两个具名字段 `overworld` / `tsy`），consumer 系统按 `DimensionKind` 取对应 provider
5. **玩家 `TsyPresence` 的 `entry_portal_pos` 必须带位面信息**：从 `DVec3` 升级为 `(DimensionKind, DVec3)`（回主世界的锚点），否则出关时无法跨回
6. **DimensionType 参数走 Nether 式**：`has_skylight=false`、`has_ceiling=true`、`ambient_light=0.08`、`piglin_safe=false`、`bed_works=false`、`respawn_anchor_works=false`、`min_y=-64`、`height=256`、`fixed_time=18000`（永夜）、`effects="minecraft:the_nether"`（走 nether 视觉包，客户端原生支持，无需自制资源包）。上述值域在 active plan 前可调；骨架阶段钉"走 Nether 式而不是 Overworld 式"。
7. **玩家出关 = 主动或被动跨位面回到主世界锚点**：不做"走出 TSY AABB 自动出关"——TSY 位面里走到 XZ 边界会撞不可逾越的 world border（或无限延伸的死负压区，物理上走不出去）；出关唯一途径是走回裂缝入口 POI（TSY 侧的 `_shallow` 层中心复用同一个 `RiftPortal` 实体作为双向门，或塌缩事件强制弹出）

---

## §1 DimensionType 注册

### 1.1 新 DimensionType

**位置**：`server/src/world/dimension.rs`（新文件）

**Audit 修正**（2026-04-24）：对照 Valence pin `2b705351` 的 `valence_registry::dimension_type::DimensionType` 真实 struct，修正三处：
- 补 `height: i32` 字段（真实 struct 和 `logical_height` 是**两个独立字段**；`#[serde(deny_unknown_fields)]` 漏字段会 panic）
- `effects` 类型是 `DimensionEffects` enum 不是 `String`
- `monster_spawn_light_level` 填具体值 `MonsterSpawnLightLevel::Int(0)`

```rust
use valence::ident;
use valence_registry::dimension_type::{
    DimensionEffects, DimensionType, DimensionTypeRegistry, MonsterSpawnLightLevel,
};

pub const TSY_DIMENSION_IDENT: &str = "bong:tsy";

pub fn register_tsy_dimension(registry: &mut DimensionTypeRegistry) {
    registry.insert(
        ident!("bong:tsy"),
        DimensionType {
            has_skylight: false,
            has_ceiling: true,
            ultrawarm: false,
            natural: false,         // 床爆炸 / respawn 行为按非自然位面处理
            coordinate_scale: 1.0,  // 不做坐标换算，TSY dim 坐标就是 blueprint 坐标
            piglin_safe: false,
            bed_works: false,
            respawn_anchor_works: false,
            has_raids: false,
            height: 256,            // 物理高度（真实 struct 要求）
            logical_height: 256,    // 玩家可达高度
            min_y: -64,
            infiniburn: "#minecraft:infiniburn_nether".into(),
            effects: DimensionEffects::TheNether,  // enum 变体，不是 String
            ambient_light: 0.08,
            fixed_time: Some(18000),               // 永夜（对齐负压压抑基调）
            monster_spawn_light_level: MonsterSpawnLightLevel::Int(0),
            monster_spawn_block_light_limit: 0,
        },
    );
}
```

**⚠️ 注册时机（Valence 要求）**：`DimensionTypeRegistry` 源文件明确注释"Modifying the dimension type registry after the server has started can break invariants within instances and clients"。`register_tsy_dimension` 必须挂在 `PreStartup`，早于任何 `ChunkLayer` / `LayerBundle::new` 的 spawn（否则 chunk layer 拿 dim type 时 id 还没分配）。

### 1.2 LayerBundle 初始化

**位置**：`server/src/world/mod.rs` 的 setup 系统扩展

```rust
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DimensionKind { Overworld, Tsy }

#[derive(Resource)]
pub struct DimensionLayers {
    pub overworld: Entity,
    pub tsy: Entity,
}

fn setup_dimensions(
    mut commands: Commands,
    server: Res<Server>,
    dimensions: Res<DimensionTypeRegistry>,
    biomes: Res<BiomeRegistry>,
) {
    let overworld = commands.spawn(LayerBundle::new(ident!("minecraft:overworld"), &dimensions, &biomes, &server)).id();
    let tsy       = commands.spawn(LayerBundle::new(ident!("bong:tsy"),          &dimensions, &biomes, &server)).id();
    commands.insert_resource(DimensionLayers { overworld, tsy });
}
```

**Audit 契合**：`LayerBundle::new(N: Into<Ident<String>>, &DimensionTypeRegistry, &BiomeRegistry, &Server)` 签名对齐 Valence pin `2b705351` 的 `valence_server::layer::LayerBundle::new`。现有 `server/src/world/mod.rs:370` / `:384` 已用同签名创建 overworld layer，TSY layer 仅 ident 不同。

### 1.3 `CurrentDimension` component（新增，P0 plan 多处引用的前置）

**位置**：`server/src/world/dimension.rs`（同文件）

```rust
/// 玩家当前所在位面（每 tick 由 apply_dimension_transfers 维护）。
/// - 初始化：Client 首次 spawn 时挂 `CurrentDimension(DimensionKind::Overworld)`
/// - 变更：DimensionTransferRequest 处理后改写 .0（与 VisibleChunkLayer 同步）
/// - 消费：P0 的 tsy_entry_portal_system / tsy_exit_portal_system 用来区分"主世界 → TSY" vs "TSY → 主世界" 触发方向（避免同一 portal 被两侧误触发）
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CurrentDimension(pub DimensionKind);

impl Default for CurrentDimension {
    fn default() -> Self {
        Self(DimensionKind::Overworld)
    }
}
```

**初始化挂载点**：`server/src/player/mod.rs:149` 的 `apply_spawn_defaults` 扩展签名，接受 `current_dim: &mut CurrentDimension` 并在 spawn layer 对应的 `DimensionKind` 处赋值。由 P-1 本 plan 提供 struct 定义 + 初始化系统；P0 `plan-tsy-zone-v1 §3.3/§3.4` 直接 Query `&CurrentDimension`。

---

## §2 per-dimension TerrainProvider

### 2.1 Manifest 布局

Worldgen 侧必须产出两份 raster（见 worldgen plan §2.1 分文件方案）：

```
worldgen_out/
  overworld/
    manifest.json
    tile_xxx_yyy/*.bin
  tsy/
    manifest.json
    tile_xxx_yyy/*.bin
```

### 2.2 Provider 升级

**位置**：`server/src/world/terrain/raster.rs`

```rust
#[derive(Resource)]
pub struct TerrainProviders {
    pub overworld: TerrainProvider,
    pub tsy: TerrainProvider,
}

impl TerrainProviders {
    pub fn load(overworld_root: &Path, tsy_root: &Path) -> Result<Self, LoadError> {
        Ok(Self {
            overworld: TerrainProvider::load(overworld_root)?,
            tsy:       TerrainProvider::load(tsy_root)?,
        })
    }
    pub fn get(&self, kind: DimensionKind) -> &TerrainProvider {
        match kind { DimensionKind::Overworld => &self.overworld, DimensionKind::Tsy => &self.tsy }
    }
}
```

### 2.3 Chunk 生成路由

`generate_chunks_around_players` 按玩家当前 dimension 选 provider。只两档，match 硬编；增位面时改 match。

**Audit 发现的全部 TerrainProvider consumer（~16 处）**，都要按 `DimensionKind` 路由：

| 文件 | 用途 |
|------|------|
| `server/src/world/terrain/mod.rs` × 5 | chunk gen / surface / setup |
| `server/src/world/terrain/raster.rs` | 本体 |
| `server/src/world/terrain/biome.rs` | biome 查询 |
| `server/src/world/terrain/flora.rs` | 植被 |
| `server/src/world/terrain/mega_tree.rs` × 3 | 巨树 |
| `server/src/world/terrain/decoration.rs` | 装饰物 |
| **`server/src/network/chat_collector.rs`** ⭐ | narration 生成时读地形信息 |

⭐ **chat_collector.rs 必须同步路由**，否则 agent narration 会在 TSY dim 里错用主世界地貌描述（或反之）—— 玩家感知层面的立即 bug。

---

## §3 跨位面传送 API

### 3.1 接口

**位置**：`server/src/world/dimension_transfer.rs`（新文件）

```rust
pub struct DimensionTransferRequest {
    pub entity: Entity,
    pub target: DimensionKind,
    pub target_pos: DVec3,
}

pub fn apply_dimension_transfers(
    mut commands: Commands,
    layers: Res<DimensionLayers>,
    mut clients: Query<(&mut Client, &mut Position, &mut VisibleChunkLayer, &mut VisibleEntityLayers)>,
    mut requests: EventReader<DimensionTransferRequest>,
) { /* 逐请求应用：换 VisibleChunkLayer.0、VisibleEntityLayers.0、Position、发 respawn */ }
```

调用方（P0 的 `tsy_entry_portal_system` / `tsy_exit_portal_system`）不直 `insert Position`，改成发 `DimensionTransferRequest` event。

### 3.2 幂等 / 保护

- 同一 entity 同一 tick 多次 request → 仅应用最后一次
- 目标位面未 load → log error + skip（不 panic）
- 传送前 unload entity 在原 layer 的 chunk 订阅（Valence 侧 auto，但要验）

### 3.3 Portal 视觉形态（复用 MC 原版模型，不自制资源）

**决策**：不做自定义 portal 方块 / 粒子 / mesh，直接复用 MC 1.20.1 原版的两种 portal 方块 + 框架，客户端原生渲染零成本。形态按**方向语义**分两类：

| 方向 | 原版模型 | 方块组 | 世界观解释 |
|------|---------|--------|----------|
| **Entry**（主世界 → TSY） | **竖式 Nether 门** | `obsidian` 4×5 框 + 内部 `nether_portal` | "地壳上一道凝结负灵气的竖直裂缝"，玩家钻进去；对齐 `worldview §十六` 原话"地壳上一道看似普通的裂缝" |
| **Exit**（TSY → 主世界） | **横式 End 门** | 12 × `end_portal_frame`（带 eye）围一圈，中心 3×3 填 `end_portal` | "TSY 深处阵盘残件 / 法阵核心托起的回程阵，踏上去负压反吐"；对齐 `worldview §十六.一` "阵盘残件 / 法阵核心" 的描述 |

视觉心智："**竖 = 入，横 = 出**"。玩家一眼就能认出哪个是进哪个是出，不需要 UI 提示。

**P5 撤离点（DeepRift / CollapseTear）的形态分配由 `plan-tsy-extract-v1` 定，建议同样用横式 End 门表达"从 TSY 向外"的所有方向**（包括深层缝和塌缩裂口）。竖式 Nether 门只用于主世界侧 Entry，保持一致性。

#### 触发方式

**不复用原版的触发逻辑**——
- Nether 门原版是"站在 portal 方块内 4 秒才传送"，我们要**瞬时**
- End 门原版目标 dimension 写死是 End，我们要路由到 TSY

**做法**：在 portal 方块上方（Entry）或中心（Exit）spawn 一个**不可见标记 entity**（armor stand 或 Valence 原生 marker），挂 `RiftPortal` + `Position`；触发完全走我们的 `trigger_radius` + AABB 检测 + 发 `DimensionTransferRequest`。portal 方块只是**视觉/听觉皮肤**，玩家看到的是原版门的紫色/紫黑效果 + 原版 ambient sound。

#### 方块构造（Worldgen 产出）

Entry / Exit portal 的方块实体组在 `manifest.json` 里作为 POI 的附带 block patch（或由 consumer system 在 spawn 时主动摆放 2×3/4×5 obsidian 框 + portal 内部方块）：

| Portal | 尺寸 | 占位 | 朝向 |
|--------|------|------|------|
| Entry（Nether 竖式） | 4 宽 × 5 高 × 1 深 | XZ 平面任一朝向（blueprint 指定） | 玩家可从任一面进入 |
| Exit（End 横式） | 5 × 1 × 5（含 3×3 portal 中心） | 地面朝上 | 玩家从上方落入 / 踏上 |

Worldgen POI tag 新增 `orientation:vertical\|horizontal`（可选；按 `direction` 推导默认值：entry→vertical, exit→horizontal）。

#### Portal auto-travel 风险（2026-04-24 audit 已关闭）

~~原 Q："Valence 是否保留 vanilla `nether_portal` / `end_portal` 的 auto-travel 行为，如保留必须拦截"~~ **已决**：

- **Valence pin `2b705351` 源码审计**：`crates/valence_server` + `crates/valence_entity` 全 tree 零命中 `nether_portal` / `end_portal` / `portal_travel` / `PortalBlock` 相关 system。Valence 作为 headless Bevy server **完全不实现** vanilla 的 portal travel 逻辑。
- 唯一"portal"关联字段是 `PortalCooldown`（`client.rs:141`），仅作为 GameJoin packet 的 client-side 提示值送给客户端，默认 0，和 server 行为无关。
- Client 侧（Fabric vanilla）踩到 `nether_portal` / `end_portal` 方块只会渲染紫色/紫黑 overlay + ambient sound；**不主动切 dim**（切 dim 要 server 发 `PlayerRespawnS2c`）。
- 玩家相机/屏幕在 portal 方块内的紫色 overlay 效果保留（MC 原生视觉），audit 中视为**正面反馈**（增强"正在穿越"的沉浸感），无需拦截。

**结论**：方案 A 安全，`BlockState::NETHER_PORTAL` / `END_PORTAL` 放进 `ChunkLayer::set_block` 即可，marker entity + `DimensionTransferRequest` 独立触发，zero 原版干扰。

---

## §4 Client 侧 dimension 切换

### 4.1 协议路径

MC 1.20.1 协议 763 的 `play.Respawn` packet 原生支持 dimension 切换，客户端只要 dimension type 在 `registry_codec`（login time sent）里有注册，就能渲染。Valence 在 `Client::respawn` 里已经发了这个 packet。

### 4.2 Fabric 微端需要改吗？

**2026-04-24 Audit 结论：零改动**（见 §11）。

- **Server 端 Valence**：`Changed<VisibleChunkLayer>` 触发 `crates/valence_server/src/spawn.rs:144` 的 `respawn` system 自动发 `PlayerRespawnS2c`（含新 `dimension_type_name`）；玩家首次 join 时 `GameJoinS2c` 含完整 `registry_codec`，内含我们注册的 `bong:tsy`。
- **Client 端 Fabric vanilla**：MC 1.20.1 原生支持 dimension 切换（Respawn packet → 场景重载）；owo-lib HUD 是 screen-space overlay，dimension 切换不 unload 其 component。
- **未命中硬编 dim 的 mixin**：现有 Bong mixin（camera / game_renderer 等 6 个）全部和 dimension 无关；`grep "Overworld\|OVERWORLD\|DimensionTypes\."` on `client/src/main/java/` 若需 double-check 可 active 阶段顺便跑一次（零风险）。

原 Q10（"手动 audit"）**关闭**，不再作为升级前置。

---

## §5 测试 / smoke

### 5.1 Rust unit tests

- `TerrainProviders::load` 两份 manifest 都加载成功
- `DimensionTransferRequest` 应用后 `VisibleChunkLayer.0` 和 `Position` 同步改
- 同 tick 双请求幂等

### 5.2 Integration

`server/tests/dimension_transfer_integration.rs`：
- 玩家起于 Overworld，发 `Transfer(Tsy, (0, 80, 0))` → 验 Position + VisibleChunkLayer 都换了
- 再发 `Transfer(Overworld, origin)` → 验回切
- 连续 10 次往返，验无 chunk leak / entity leak

### 5.3 Smoke `scripts/smoke-tsy-dimension.sh`（新）

1. regen 两份 manifest
2. 启 server
3. 用 test client 连接 → `/tsy-spawn` → 验客户端收到 Respawn packet + 场景重载
4. 回传 → 验回主世界

---

## §6 对下游 plan 的接口契约

### 给 P0 `plan-tsy-zone-v1`

- P0 的 `tsy_entry_portal_system` 不再自己 `insert Position`；改成发 `DimensionTransferRequest` event
- `TsyPresence.entry_portal_pos: DVec3` 升级为 `entry_anchor: (DimensionKind, DVec3)`（或语义更明确的 `return_to: DimensionAnchor`）
- P0 的 `is_tsy()` / `tsy_layer()` / `tsy_family_id()` 逻辑保留不变（仍基于 zone.name 前缀），只是这些 zone 注册到 TSY dim 内的 `ZoneRegistry` 实例（或 zone.dimension 字段 gating，见 Q2）

### 给 worldgen plan

- Blueprint 产出 **两套 manifest**：主世界 + TSY dim
- 主世界 manifest 的 POI 含 `kind=rift_portal` 条目（TSY 入口锚点，tag 带 `family_id:X`）
- TSY manifest 的 POI 含 `loot_container` / `npc_anchor` / `relic_core_slot` / 深层 portal
- worldgen plan §4 新增的 `tsy_presence` / `tsy_origin_id` / `tsy_depth_tier` layer **只在 TSY manifest 里存在**（Q7 自然关闭——非 TSY 位面里根本没这些 layer）

---

## §7 开放问题清单（active 实装中小步决策，不阻塞开工）

**已关闭（audit 2026-04-24）**：
- ~~**Q2**~~ ZoneRegistry 位面归属 → **候选 A**：单 registry + `Zone.dimension` 字段 + `find_zone(dim, pos)` 签名（P0 plan §1.1 已默认，audit 确认 ZoneRegistry 结构 `Vec<Zone>` 可直接加字段）
- ~~**Q4**~~ 多人 instance 化 → **共享 TSY dim**（符合 §十六 supply chain 设计；搜打撤同一片土地）
- ~~**Q5**~~ 出关几何 → **走回 Exit portal 主动交互**（P0 plan §3.4 已决；独立位面里"走出 AABB 自动出关"几何不成立）
- ~~**Q10**~~ Client 侧 dim 切换 audit → **零改动**（§4.2：Valence 自动发 `PlayerRespawnS2c`，MC 1.20.1 原生支持，owo-lib HUD 不受影响）
- ~~**§3.3-Q**~~ Portal auto-travel → **Valence 源码零命中**，方案 A 安全（详见 §3.3）

**剩余 Q（active 实装阶段决策，不阻塞开工）**：
- **Q1** `DimensionType` 参数：ambient_light=0.08 vs 0.0？effects 走 `TheNether` vs 自定义（audit 确认默认走 TheNether，playtest 后按氛围调）
- **Q3** TSY dim 的 world border：硬墙 vs 无限延伸死负压区？A 实现简单，B 叙事更酷。实装时先走 A（Valence `WorldBorder` 设小），B 留给 polish
- **Q6** 跨 dim 传送保留 leash/坐骑？约定**全断**（MC 原版 nether 传送也断），实装时在 `apply_dimension_transfers` 里 despawn 所有 leashed entity
- **Q7** TSY dim 的 seed：**独立 seed**（两套 manifest 互不干扰，worldgen plan 侧 follow）
- **Q8** 塌缩事件跨 dim zone 生命周期管理：P2 lifecycle 发 `TsyCollapseCompleted` event → 本 plan 提供 `despawn_zones_for_family(family_id)` helper + 本 plan 暴露的 `apply_dimension_transfers` 强制弹出仍在内的玩家
- **Q9** mmap 成本：两份 `TerrainProvider` 地址空间翻倍但 resident memory 按 page 懒加载；玩家只在一个 dim 内活动，另一份冷数据不驻 RAM。实装完跑 `ps -o rss` 记录基线

---

## §8 实施规模预估（2026-04-24 audit 重估）

原估 ~790 行严重低估 `find_zone` caller 改面（35+ 处）和 TerrainProvider consumer 数量（~16 处）。真实规模：

| 模块 | 原估 | 新估 | 备注 |
|------|------|------|------|
| `server/src/world/dimension.rs`（`DimensionType` 注册 + `DimensionKind` enum + `DimensionLayers` resource + **`CurrentDimension` component §1.3**） | 120 | 160 | 加 CurrentDimension + 真实字段 |
| `server/src/world/dimension_transfer.rs`（event + apply system + CurrentDimension 维护） | 150 | 180 | CurrentDimension 同步 + Changed 监听 |
| `server/src/world/mod.rs` setup 扩展（双 LayerBundle） | 60 | 100 | 兼容现有 overworld spawn 代码（`:370` / `:384` 两处） |
| `server/src/world/terrain/raster.rs` `TerrainProviders` 升级 + **16 处 consumer 改签名** | 120 | 240 | biome/flora/mega_tree×3/decoration/mod×5 + chat_collector |
| `server/src/world/zone.rs` `Zone.dimension` 字段 + `find_zone(dim, pos)` 签名改 + **35+ caller 替换** | 80 | **280** | `player/gameplay.rs` / `network/mod.rs` / `command_executor.rs` × 10 tests / `world/events.rs` × 4 / `world/mod.rs` × 2 / `zone.rs` tests × 5 |
| `server/src/player/mod.rs` CurrentDimension 初始化（`apply_spawn_defaults` 扩签名） | — | 40 | 新增 |
| Client mixin / HUD audit | ?（未决） | **0** | Audit §4.2 证实零改动 |
| Rust tests（unit + integration + 双 dim 往返 + `find_zone` 全测试修） | 200 | 300 | 测试代码改面大 |
| Smoke `scripts/smoke-tsy-dimension.sh` | 40 | 60 | 双 manifest 验证 + 跨 dim 往返 |
| `scripts/dev-reload.sh` 双 manifest 支持 | 20 | 40 | 按位面 regen + validate |
| **合计** | **~790** | **~1400** | +77% |

**建议拆 2 commit**（同一 worktree 吃完，但分 commit 降低 review 震荡）：
1. **Commit 1**：Dimension 基础设施 ~820 行（DimensionType / DimensionKind / DimensionLayers / DimensionTransferRequest / CurrentDimension / TerrainProviders 升级 + consumer 改签名 + setup + tests）
2. **Commit 2**：Zone 升级 ~320 行（Zone.dimension 字段 + find_zone(dim, pos) + 35 caller 批量替换）
3. 其他 ~260 行（smoke / dev-reload / player 初始化）按逻辑归入对应 commit

---

## §9 升级条件（✅ 全部满足，2026-04-24 升级完成）

本 plan 从 `docs/plans-skeleton/` 移到 `docs/`：

1. ✅ Worldview §十六 的位面决策已钉
2. ✅ P0 `plan-tsy-zone-v1` 反转架构后的骨架修订完成（§-1 点 5、§1.3 TsyPresence、§3 entry/exit 跨 dim 改写）
3. ✅ Worldgen plan `plan-tsy-worldgen-v1` 反转架构后的骨架修订完成（§2 blueprint 分文件、§4 layer 作用域收窄）
4. ✅ Q2 `ZoneRegistry` 位面归属方案 → 候选 A（`Zone.dimension` + `find_zone(dim, pos)`）
5. ✅ Q4 多人 instance 策略 → 共享 TSY dim
6. ✅ Q5 出关几何方案 → 走回 Exit portal 主动交互
7. ✅ Q10 client 侧 dim 切换 → 零改动（Valence + MC 1.20.1 原生支持，owo-lib HUD 不受影响；源码 audit 替代手动跑）
8. ✅ Valence pin `2b705351` 源码审计：`DimensionType` 真实字段 / `LayerBundle::new` 签名 / `PlayerRespawnS2c` auto-emit / portal auto-travel zero 风险 全部确认（§11）

---

## §10 不改的文件（明确避免作用域蔓延）

- `agent/**` — 跨位面与天道 Agent 无直接交互；TSY 事件仍通过现有 IPC schema（P0 `tsy_enter_event`）走
- `server/src/npc/**` — NPC 在 TSY 内的行为归 P4 `plan-tsy-hostile-v1`；本 plan 只保证 NPC entity 能在 TSY layer 内存在
- `server/src/inventory/**` — 跨位面物品携带规则（Q6）由 P0 入场过滤器处理
- 所有现有 zones.json / zones.worldview.example.json 内容结构 — 本 plan 可能加 `dimension` 字段（Q2 候选 A），但不改既有 zone 语义

---

## §11 2026-04-24 Audit 报告（升级 active 的证据链）

本 plan 从 skeleton 升 active 前跑的全方位 audit，涵盖 Valence pin 源码 + 本仓库代码影响面 + 命名冲突 + manifest 现状 9 项（A-I）。完整结论固化于此，避免未来重审。

### A. Valence pin `2b705351` API 对齐

| 审计点 | 结论 |
|--------|------|
| `DimensionType` struct 字段 | 真实 17 字段 vs plan 初稿 15；修正：加 `height`，`effects` 改 enum `DimensionEffects::TheNether`，`monster_spawn_light_level` 填 `MonsterSpawnLightLevel::Int(0)`（见 §1.1） |
| `#[serde(deny_unknown_fields)]` | 真实 struct 带此 attr，漏字段 runtime panic — §1.1 修正后安全 |
| `LayerBundle::new` 签名 | `(N: Into<Ident<String>>, &DimensionTypeRegistry, &BiomeRegistry, &Server)` 与 plan §1.2 完全对齐（现有 `world/mod.rs:370/384` 已用同签名创建 overworld） |
| `PlayerRespawnS2c` 自动发送 | Valence `spawn.rs:144` 的 `respawn` system 监听 `Changed<VisibleChunkLayer>`，plan §3.1 "改 `VisibleChunkLayer.0`" 思路行得通 |
| `DimensionTypeRegistry` 注册时机 | Valence 源文件明确警告注册要早于 client spawn —— §1.1 补注 `PreStartup` 时机要求 |
| **HACK 风险**：Valence `load_default_dimension_types` 把所有默认 dim 的 `ambient_light` 改成 1.0 | **不影响我们**：HACK 只迭代 `RegistryCodec` 里的默认 dim（overworld/nether/end），自定义 `bong:tsy` 用 `reg.insert(...)` 后注册，不被覆盖，保持 `ambient_light=0.08` |

### B. TerrainProvider consumer 影响面

16 处 consumer（见 §2.3 表格）。所有签名从 `&TerrainProvider` → `&TerrainProviders` + `DimensionKind` 参数。**关键发现**：`network/chat_collector.rs` 也是 consumer，plan 原稿未提，加进 §2.3。

### C. find_zone caller 影响面（规模重估主因）

**35+ caller**。生产 6 + 测试 29：

| 文件 | 调用数 | 类型 |
|------|------|------|
| `player/gameplay.rs:463` | 1 | 生产 |
| `network/mod.rs:822` | 1 | 生产 |
| `world/mod.rs:427,441` | 2 | 生产 |
| `world/events.rs:845,903` | 2 | 生产 |
| `network/command_executor.rs` | 10+ | 测试（硬编码 `DVec3::new(8.0, 66.0, 8.0)`） |
| `world/zone.rs` tests | 5 | 测试 |
| `world/events.rs` tests | 2 | 测试 |
| `network/mod.rs:898` + 其他 | 多 | `find_zone_by_name`（签名不变，但若需要 dim-scoped 查询，另加新 API） |

机械替换 + 测试 fixture 加 dim 参数，预计 ~280 行（+200 行 vs 原估 80）。

### D. zones.json 现状

- `server/zones.json`（99 行，6 zone）+ `server/zones.worldview.example.json`（659 行，世界观样板）
- 零 `dimension` 字段，所有现有 zone 默认 `minecraft:overworld`
- Plan §1.1 新增 `dimension` 字段：6 条 JSON entry 补丁 + loader 加 `#[serde(default = "overworld_ident")]` 向后兼容旧 snapshot
- 新 `server/zones.tsy.json` 由 P0/worldgen plan 建，本 plan 不建

### E. CurrentDimension 现状

**零实装**。P0 plan §3.3/§3.4 多处引用 `&CurrentDimension` 但 P-1 原 skeleton 未定义。**已在本次升级中补进 §1.3** —— struct + Default + 初始化挂载点。

### F. 命名冲突扫描

```
DimensionTransferRequest / bong:tsy / TSY_DIMENSION_IDENT
DimensionKind / DimensionLayers / DimensionAnchor / CurrentDimension
```

全部零命中 — 开工无 collision 风险。

### G. `ZoneRegistry::register_runtime_zone` 现状

**不存在**。现有 `apply_runtime_records`（`zone.rs:195`）只改已注册 zone 的 `spirit_qi/danger_level`，不加 zone。P0 `plan-tsy-zone-v1 §-1` 显式声明"由 P0 补"，本 P-1 不管。`ZoneRegistry` 结构简单（`pub struct ZoneRegistry { pub zones: Vec<Zone> }`），push+dedup 不超过 10 行。

### H. Portal auto-travel 风险

**Valence 源码零命中**（详见 §3.3），Portal 方案 A 完全安全。MC 1.20.1 vanilla client 对 `nether_portal`/`end_portal` 方块仅做渲染 overlay + ambient sound，不主动切 dim；切 dim 唯一触发是 server 端 `PlayerRespawnS2c`，我们完全控制。

### I. Manifest mmap 成本

`worldgen/generated/` 目录当前不存在（artifact gitignored，regen 产出）。双份 mmap 的真实内存开销未基线化。Audit 结论：**延到 active 实装后用 `ps -o rss` 记录基线**（Q9 剩余开放）。理论上 mmap 按 page 懒加载，单玩家一次只在一个 dim，冷 dim 不驻 RAM。

---

**下一步**：`/consume-plan tsy-dimension` 启动。本 plan **必须先于** P0 和 worldgen plan 的 active 阶段落地（基础设施前置）。

---

## §12 进度日志

- **2026-04-25**：实装零落地。`server/src/` grep `DimensionKind` / `DimensionLayers` / `DimensionTransferRequest` / `TerrainProviders` / `CurrentDimension` / `register_tsy_dimension` / `bong:tsy` / `TSY_DIMENSION_IDENT` 全部零命中；`world/mod.rs:370,384` + `world/terrain/mod.rs:104` 仍是单 `LayerBundle::new(ident!("overworld"), …)`；`world/zone.rs:187` `find_zone(pos)` 仍单 dim 签名，`Zone` struct 无 `dimension` 字段。本 plan 当前为 active 设计稿（§1–§11 已 audit 定稿，1360 行规模评估完成，骨架已升 active），但代码实施尚未开始 —— `/consume-plan tsy-dimension` 仍是下一步。
- **2026-04-26**：PR #47（merge commit 579fc67e）落地 §1.1–§3.2 + §4.2/§11-A + Q2 候选 A 完整链路，1252 单测全绿（server 36 文件 +1293/-267）：
  - **§1.1**：新 `world/dimension.rs` → `DimensionKind` enum + `DimensionLayers` resource + `CurrentDimension` 组件 + `register_tsy_dimension`（按 Valence pin `2b705351` 17 字段注册 `bong:tsy`，Nether 风格视觉/永夜/无天空）+ 6 单测
  - **§1.2**：`setup_world` 双 spawn overworld + TSY layer，`OverworldLayer` / `TsyLayer` 标记组件，9 处单层 query 加 `With<OverworldLayer>` 过滤；`spawn_anvil_world` / `spawn_fallback_flat_world` / `spawn_raster_world` 改返回 overworld Entity
  - **§1.3**：`init_clients` 插入 `CurrentDimension::default()`；`attach_player_state_to_joined_clients` 按 persisted `last_dimension` 重写 `EntityLayerId` / `VisibleChunkLayer` / `VisibleEntityLayers`（DB migration v13 加 `player_slow.last_dimension`，重连恢复闭环）
  - **§2.2/§2.3**：新 `TerrainProviders { overworld, tsy: Option<TerrainProvider> }`（`world/terrain/raster.rs`），3 处 resource consumer 升级（`world/terrain/mod.rs` + `network/chat_collector.rs` + `npc/navigator.rs`）；TSY provider `None` 占位等 `plan-tsy-worldgen-v1` 产 manifest
  - **§3.1/§3.2**：新 `world/dimension_transfer.rs` → `DimensionTransferRequest` event + `apply_dimension_transfers` system（同 tick 同 entity HashMap dedup / 缺组件 `tracing::warn` 跳过 / 缺 resource 安静 drain），5 单测
  - **§4.2/§11-A**：跨位面传送由 Valence `respawn` 系统监听 `Changed<VisibleChunkLayer>` 自动 `PlayerRespawnS2c`，client MC 1.20.1 vanilla 零改动
  - **Q2 候选 A**：`Zone` struct + `ZoneConfig` 加 `dimension: DimensionKind` 字段（`#[serde(default)]` 向后兼容旧 zones.json），`find_zone(pos)` → `find_zone(dim, pos)` 签名升级；50+ 处生产/测试 caller 升级，3 新单测覆盖旧 snapshot/显式 tsy/dim 隔离；`find_zone_by_name` 保留全局（plan §11.C / Q3）
  - **Codex P1 修**：`cultivation/tick.rs::qi_regen_and_zone_drain_tick` 改读 `CurrentDimension` 真实位面，杜绝 TSY 玩家被错查 overworld zone 倒扣 spirit_qi（commit 链尾）
  - **未覆盖**：§5/§6/§7（worldgen routing 留给 `plan-tsy-worldgen-v1`），§11.B 16 处 helper 仍传 overworld provider 单参（helper 自身不感知 dim，调用方决定）
  - **下游解冻**：P0 `plan-tsy-zone-v1` / P1 `plan-tsy-loot-v1` / `plan-tsy-worldgen-v1` 现可基于本基础设施开工

---

## Finish Evidence

### 落地清单

- **§1.1 DimensionType 注册**：
  - `server/src/world/dimension.rs` — `TSY_DIMENSION_IDENT="bong:tsy"` / `register_tsy_dimension(&mut DimensionTypeRegistry)` / `register_tsy_dimension_system` (PreStartup) / `DimensionKind` enum (Overworld/Tsy)
- **§1.2 LayerBundle 初始化**：
  - `server/src/world/dimension.rs` — `DimensionLayers { overworld, tsy }` resource + `OverworldLayer` / `TsyLayer` 标记组件
  - `server/src/world/mod.rs:34,174,447,479` — 双 spawn `(layer, OverworldLayer)` / `(layer, TsyLayer)`
- **§1.3 CurrentDimension component**：
  - `server/src/world/dimension.rs:82` — `pub struct CurrentDimension(pub DimensionKind)` + `Default`
  - `server/src/player/mod.rs:145,194` — `init_clients` 挂 `CurrentDimension::default()`，`attach_player_state_to_joined_clients` 按 `last_dimension` 重写
  - `server/src/player/state.rs:51,65,409,420,452,562,630,638,647,768,782` — DB v13 `player_slow.last_dimension` 持久化与读回
- **§2 per-dimension TerrainProvider**：
  - `server/src/world/terrain/raster.rs:139-142` — `pub struct TerrainProviders { pub overworld: TerrainProvider, pub tsy: Option<TerrainProvider> }`
  - `server/src/world/terrain/mod.rs:25,114,162,204` — re-export + setup insert + chunk gen consumer
  - `server/src/network/chat_collector.rs:21,43` — narration 路由
  - `server/src/npc/navigator.rs:48,261` — NPC 寻路 consumer
- **§3 跨位面传送 API**：
  - `server/src/world/dimension_transfer.rs:23,34,89` — `DimensionTransferRequest` event + `apply_dimension_transfers` system + `DimensionTransferSet` SystemSet
  - `server/src/world/mod.rs:105` — set ordering（Update 内排在传送 set 前/后）
  - `server/src/world/tsy_lifecycle.rs:52,398,472,502` — entry/exit portal 直发 `DimensionTransferRequest`，不再 insert Position
- **§Q2 候选 A — Zone.dimension + find_zone(dim, pos)**：
  - `server/src/world/zone.rs:28` — `Zone { ..., pub dimension: DimensionKind }`
  - `server/src/world/zone.rs:257` — `pub fn find_zone(&self, dim: DimensionKind, pos: DVec3) -> Option<&Zone>`
  - `server/src/world/zone.rs:769` — `find_zone_filters_by_dimension` 单测
  - `server/src/network/command_executor.rs:1173,1199,1233,1262,1295,1314,1468,1525` 等 35+ caller 升级
- **§4 Client 侧 dimension 切换**：零改动（Valence `respawn` 系统监听 `Changed<VisibleChunkLayer>` 自动发 `PlayerRespawnS2c`，client/Fabric vanilla 原生支持）

### 关键 commit

- `cef33e81` (2026-04-25) — feat(world): 新增 dimension 模块（DimensionKind/DimensionLayers/CurrentDimension/register_tsy_dimension）
- `bffcf7ff` (2026-04-25) — feat(world): 双 LayerBundle（overworld + bong:tsy）+ DimensionLayers resource + OverworldLayer 标记
- `4ffa23be` (2026-04-25) — feat(world): 跨位面传送 — DimensionTransferRequest event + apply_dimension_transfers system
- `9204ff2a` (2026-04-25) — feat(zone): Zone.dimension 字段 + find_zone(dim, pos) 签名 + 全测试 fixture 升级
- `579fc67e` (2026-04-26) — plan-tsy-dimension-v1: TSY 位面基础设施（DimensionType + 跨位面传送 + Zone.dimension）(#47, merge)

### 测试结果

- `server/src/world/dimension.rs` — 6 `#[test]`（`TSY_DIMENSION_IDENT` 常量 / `register_tsy_dimension_inserts_bong_tsy` / `register_tsy_dimension_uses_nether_visuals` 等）
- `server/src/world/dimension_transfer.rs` — 5 `#[test]`（同 tick 双请求幂等 / 缺组件 warn 跳过 / 缺 resource 安静 drain / VisibleChunkLayer 与 Position 同步切换 等）
- `server/src/world/zone.rs` — 16 `#[test]`（含 `find_zone_filters_by_dimension` / 旧 snapshot 默认 overworld / 显式 tsy / dim 隔离）
- `server/src/player/state.rs` — 12 `#[test]`（含 `last_dimension` v13 migration 与持久化往返）
- `server/src/world/tsy_lifecycle_integration_test.rs` + `server/src/world/tsy_integration_test.rs` — 端到端跨位面 entry/exit 与 DimensionTransferRequest 验证
- `cd server && cargo test` — 全仓库 1252 单测通过（PR #47 merge 前验收，进度日志 2026-04-26 记录）

### 跨仓库核验

- **server**：`DimensionKind` / `DimensionLayers` / `CurrentDimension` / `OverworldLayer` / `TsyLayer` / `register_tsy_dimension` / `TSY_DIMENSION_IDENT` / `DimensionTransferRequest` / `apply_dimension_transfers` / `DimensionTransferSet` / `TerrainProviders` / `Zone.dimension` / `find_zone(dim, pos)` 全部命中（`server/src/world/dimension.rs` / `dimension_transfer.rs` / `terrain/raster.rs` / `zone.rs` / `player/mod.rs` / `player/state.rs` / `tsy_lifecycle.rs` / `network/chat_collector.rs` / `npc/navigator.rs`）
- **agent**：（不涉及；§10 显式排除 `agent/**`，跨位面与天道 Agent 无直接交互）
- **client**：（零改动；§4.2 + §11-A 已论证 Valence 自动发 `PlayerRespawnS2c` + MC 1.20.1 协议 763 + Fabric vanilla 原生支持；`grep DimensionKind|TerrainProviders|DimensionTransferRequest|CurrentDimension client/` 零命中）
- **worldgen**：（不涉及；TSY manifest 由 `plan-tsy-worldgen-v1` 产，本 plan `TerrainProviders.tsy` 留 `Option<TerrainProvider>` 占位）

### 遗留 / 后续

- `TerrainProviders.tsy` 为 `Option<TerrainProvider>`，待 `plan-tsy-worldgen-v1` 产 TSY manifest 后填实；§5.3 smoke `scripts/smoke-tsy-dimension.sh` 与 §11.B 16 处 helper 的 dim 路由完善留给后续 plan（helper 自身不感知 dim，调用方决定，已被 §12 进度日志 2026-04-26 标注为"未覆盖")。Q9 mmap 内存基线（`ps -o rss`）待 TSY manifest 实装后测量。
