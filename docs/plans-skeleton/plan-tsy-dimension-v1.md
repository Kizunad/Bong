# TSY Dimension 基础设施 · plan-tsy-dimension-v1（骨架）

> 把"坍缩渊"落成一个**独立位面**而不是主世界的一块 AABB：注册 Valence `DimensionType` + 建 TSY 专用 `LayerBundle` + 跨位面传送 API + per-dimension `TerrainProvider` + client 侧 dimension 切换。本 plan 是 `plan-tsy-zone-v1` (P0) 和 `plan-tsy-worldgen-v1` (worldgen) 的**共同前置**，不涉及负压 / loot / 塌缩业务逻辑。**骨架阶段**：钉决策与接口，不下笔实装。
> 交叉引用：`worldview.md §十六 世界层实现注`（位面决策源）· `plan-tsy-zone-v1.md §-1/§3`（跨 dim 传送消费方）· `plan-tsy-worldgen-v1.md §-1/§1/§2`（TSY dim 地形消费方）

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

## §0 设计轴心（骨架阶段已定稿，不再动）

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

```rust
pub const TSY_DIMENSION_IDENT: &str = "bong:tsy";

pub fn register_tsy_dimension(registry: &mut DimensionTypeRegistry) {
    registry.insert(
        ident!("bong:tsy"),
        DimensionType {
            has_skylight: false,
            has_ceiling: true,
            ultrawarm: false,
            natural: false,        // 床爆炸 / respawn 行为按非自然位面处理
            coordinate_scale: 1.0, // 不做坐标换算，TSY dim 坐标就是 blueprint 坐标
            piglin_safe: false,
            bed_works: false,
            respawn_anchor_works: false,
            has_raids: false,
            logical_height: 256,
            min_y: -64,
            infiniburn: "#minecraft:infiniburn_nether".into(),
            effects: "minecraft:the_nether".into(),
            ambient_light: 0.08,
            fixed_time: Some(18000),  // 永夜（对齐负压压抑基调）
            monster_spawn_light_level: ...,
            monster_spawn_block_light_limit: 0,
        },
    );
}
```

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

`generate_chunks_around_players` 按玩家当前 dimension 选 provider。骨架阶段只两档，match 硬编；增位面时改 match。

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

---

## §4 Client 侧 dimension 切换

### 4.1 协议路径

MC 1.20.1 协议 763 的 `play.Respawn` packet 原生支持 dimension 切换，客户端只要 dimension type 在 `registry_codec`（login time sent）里有注册，就能渲染。Valence 在 `Client::respawn` 里已经发了这个 packet。

### 4.2 Fabric 微端需要改吗？

**初判不需要**（原版 MC client 就支持 dimension 切换），但要验：
- HUD overlay（真元条等）在 dimension 切换时是否 unload / 重载状态
- 任何写死 "Overworld" 或硬编 dimension 检查的 mixin

**active plan 前的 QA 点**：在本地 client 做一次 `/tsy-spawn` → 进 TSY 位面，检查 HUD 残留 / 音效切换 / 粒子 / tick 率是否异常。

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

## §7 开放问题清单（骨架阶段不答，active 前收敛）

- **Q1** `DimensionType` 参数：永夜 vs 动态塌缩光（塌缩事件时光线骤暗）？ambient_light=0.08 vs 0.0？effects 走 `minecraft:the_nether` vs 自定义？——本 plan 初稿走 nether 复用，active plan 前 playtest 看氛围
- **Q2** `ZoneRegistry` 如何区分位面归属？候选 A：单 registry + `Zone.dimension: DimensionKind` 字段 gating；候选 B：两个 registry `overworld_zones` / `tsy_zones`。A 改动面小但查询 hot path 多一个 filter；B 数据分离干净但所有 zone 消费者要路由。倾向 A
- **Q3** TSY dim 的 world border：硬墙（走到撞不可逾越的壁）vs 无限延伸死负压区（走到被负压抽干变道伥）？叙事上 B 更酷，实现上 A 更简单。骨架阶段不决
- **Q4** 多人 instance 化：每队独立 TSY dim instance（需要 per-instance layer + 动态 layer 生成）vs world-level 共享 TSY（默认，搜打撤在同一片土地上）？骨架阶段**倾向后者**（共享更符合 §十六 supply chain 设计），但 Q 留给 active 复核
- **Q5** 出关几何：P0 原设"走出 AABB 自动出关"在独立位面里不成立（走出去是 world border 或无物区）。改成"走回裂缝（`_shallow` 层的 RiftPortal）主动交互"？还是"走到 `_shallow` 的 XZ 中心一定半径内自动触发"？P0 plan §3.4 要配合改
- **Q6** 入场携带物品 / 生物体积：跨 dim 传送是否保留挂在身上的 leash 实体 / 坐骑 / 拖拽物？MC 原版传 nether 会断 leash。骨架阶段约定**全断**，但要显式记录
- **Q7** TSY dim 的 seed / 地形 bake 是否与主世界独立？worldgen plan 的 `world_seed` 对 TSY 是否另开一个？倾向**独立 seed**（不同 seed 才能保证两套 manifest 的地形互不干扰）
- **Q8** `plan-tsy-lifecycle-v1`（P2）的塌缩事件：死坍缩渊 = 移除 TSY dim 内的 subzone + 失效主世界裂缝 POI（双端同步）。消息通道是什么？跨 dim 的 zone 生命周期管理需要本 plan 预留 hook
- **Q9** 性能：两份 mmap TerrainProvider 占内存是单份的 2x；但玩家一般同时只在一个 dim 内活动，另一份可 lazy mmap。骨架阶段不决
- **Q10** Fabric 客户端侧跨 dimension 的 HUD / mixin 行为验证清单（见 §4.2）何时做？active 开工前必须至少手动走一遍

---

## §8 实施规模预估（骨架，active 开工时修正）

| 模块 | 新增行数 |
|------|------|
| `server/src/world/dimension.rs`（DimensionType 注册 + DimensionKind enum + DimensionLayers resource） | ~120 |
| `server/src/world/dimension_transfer.rs`（event + apply system） | ~150 |
| `server/src/world/mod.rs` setup 扩展 | ~60 |
| `server/src/world/terrain/raster.rs` `TerrainProviders` 升级 + chunk 生成路由 | ~120 |
| `server/src/world/zone.rs` `Zone.dimension` 字段 + registry gating（Q2 候选 A） | ~80 |
| Client mixin / HUD audit（若需改） | ~? 待 §4.2 QA 后定 |
| Rust tests（unit + integration） | ~200 |
| Smoke `scripts/smoke-tsy-dimension.sh` | ~40 |
| `scripts/dev-reload.sh` 双 manifest 支持 | ~20 |
| **合计** | **~790（client 侧另计）** |

骨架一次 worktree 吃完；若 §4.2 审计出需改 client mixin，client 侧单独起一个小 worktree。

---

## §9 升级条件（骨架 → active）

本 plan 从 `docs/plans-skeleton/` 移到 `docs/` 的触发：

1. Worldview §十六 的位面决策已钉（✅ 本次连带修订完成）
2. P0 `plan-tsy-zone-v1` 反转架构后的骨架修订完成（§-1 点 5、§1.3 TsyPresence、§3 entry/exit 跨 dim 改写）
3. Worldgen plan `plan-tsy-worldgen-v1` 反转架构后的骨架修订完成（§2 blueprint 分文件、§4 layer 作用域收窄）
4. Q2 `ZoneRegistry` 位面归属方案收敛（A vs B）
5. Q4 多人 instance 策略收敛
6. Q5 出关几何方案收敛（P0 联动）
7. Q10 client 侧 dimension 切换的一次手动 audit 已跑过，列出具体改动清单（或确认 zero change）

---

## §10 不改的文件（明确避免作用域蔓延）

- `agent/**` — 跨位面与天道 Agent 无直接交互；TSY 事件仍通过现有 IPC schema（P0 `tsy_enter_event`）走
- `server/src/npc/**` — NPC 在 TSY 内的行为归 P4 `plan-tsy-hostile-v1`；本 plan 只保证 NPC entity 能在 TSY layer 内存在
- `server/src/inventory/**` — 跨位面物品携带规则（Q6）由 P0 入场过滤器处理
- 所有现有 zones.json / zones.worldview.example.json 内容结构 — 本 plan 可能加 `dimension` 字段（Q2 候选 A），但不改既有 zone 语义

---

**下一步**：等 P0 和 worldgen plan 骨架同步反转完成（依赖本 plan 的决策）→ 回答 Q2/Q4/Q5/Q10 → 骨架升级为 active plan（移出 `plans-skeleton/`），`/consume-plan tsy-dimension` 启动。本 plan **必须先于** P0 和 worldgen plan 的 active 阶段落地（基础设施前置）。
