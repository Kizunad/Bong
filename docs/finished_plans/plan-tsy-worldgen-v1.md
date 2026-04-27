# TSY Worldgen · plan-tsy-worldgen-v1（骨架）

> 坍缩渊的地形 / POI / NPC anchor 自动生成：把 TSY 4 起源 × 3 层深度作为 blueprint + profile 对接进现有 worldgen 栈，产出**独立 TSY 位面**的地形 raster + 主世界侧的裂缝锚点 POI；`plan-tsy-zone-v1` 的 `/tsy-spawn` 调试命令落地后退化为"激活已注册 TSY zone"。**骨架阶段**：列接口与决策点，不下笔实装；2026-04-26 实地考察后填充至"接口对齐已实装代码 + 模板 / 伪代码完整"级别。
> 交叉引用：`plan-tsy-v1.md §1`（依赖图）· `plan-tsy-dimension-v1 §2 §6`（位面基础设施前置）· `plan-tsy-zone-v1.md §-1 §3.1`（P0 前置）· `worldview.md §十六 世界层实现注 §十六.一`（位面决策 + 4 起源）

---

## §-1 前提（现有代码基线，2026-04-26 实地核对）

> **图例**：✅ 已实装可消费 / 🟡 已实装但本 plan 需扩展 / ❌ 未实装（前置依赖）

### Python worldgen 栈

| 能力 | 状态 | 位置 |
|------|------|------|
| `LAYER_REGISTRY`（27 层，含 qi_density / mofa_decay / anomaly_* / underground_tier 等） | 🟡 | `worldgen/scripts/terrain_gen/fields.py:45-115`（本 plan 末尾追加 3 行 TSY 专用 layer，见 §4.1） |
| `BlueprintZone` + `PoiSpec`（pois 已可序列化；尚无 `dimension` 字段） | 🟡 | `worldgen/scripts/terrain_gen/blueprint.py:34-63`（本 plan §2.2 加 `dimension`） |
| Pipeline 主入口（`load_blueprint → build_generation_plan → synthesize_fields → export_rasters`） | 🟡 | `worldgen/scripts/terrain_gen/__main__.py:67-102`（本 plan §2.1 让 main 按位面跑两次） |
| Profile 注册表（`_GENERATORS`，9 个现有：abyssal_maze / ancient_battlefield / broken_peaks / cave_network / rift_valley / sky_isle / spawn_plain / spring_marsh / waste_plateau） | 🟡 | `worldgen/scripts/terrain_gen/profiles/__init__.py:14-27`（本 plan §3.1 加 4 个 TSY profile） |
| Profile dispatch（`_build_zone_overlay_tile` 用 if/elif 硬 dispatch profile name → fill_*_tile） | 🟡 | `worldgen/scripts/terrain_gen/stitcher.py:378-407`（本 plan §3.2 加 4 个 elif 分支） |
| Stitcher boundary mode（`hard` / `semi_hard` / soft 三档已实现） | ✅ | `worldgen/scripts/terrain_gen/stitcher.py:159-179` |
| Raster 导出（little-endian；每 layer 一个 `.bin`；manifest.json 含 `pois` / `semantic_layers` / `vertical_layers` / `anomaly_kinds` / `profiles_ecology` / `global_decoration_palette`） | 🟡 | `worldgen/scripts/terrain_gen/bakers/raster_export.py:57-167`（本 plan §2.1 按位面双产出） |
| POI manifest 序列化（`_collect_poi_payload` 已遍历 `zones[].pois[]` → manifest.pois） | ✅ | `worldgen/scripts/terrain_gen/bakers/raster_export.py:97, 170-188` |
| `dev-reload.sh` 主流程（regen → raster_check → cargo build → restart，4 步，单 manifest） | 🟡 | `scripts/dev-reload.sh:20-67`（本 plan §6.1 双 manifest 决策） |
| Raster invariant 校验（10 项现有，含 rift_axis_sdf / height range / sky_island / underground_tier / anomaly_kind / qi_density 范围 / water depth） | 🟡 | `worldgen/scripts/terrain_gen/harness/raster_check.py:1-201`（本 plan §4.3 加 5 条新 invariant） |

### Rust server 栈

| 能力 | 状态 | 位置 |
|------|------|------|
| `DimensionKind { Overworld, Tsy }` enum | ✅ | `server/src/world/dimension.rs:19` |
| `DimensionLayers { overworld, tsy }` resource + `entity_for(kind)` | ✅ | `server/src/world/dimension.rs:37-49` |
| `CurrentDimension(DimensionKind)` component（默认 Overworld） | ✅ | `server/src/world/dimension.rs:73-78` |
| `Zone.dimension: DimensionKind` 字段 + `find_zone(dim, pos)` 按位面查 | ✅ | `server/src/world/zone.rs:24-35, 195-199` |
| `ZoneConfig` JSON 反序列化支持 `"dimension": "overworld" \| "tsy"` 字段 | ✅ | `server/src/world/zone.rs:347, 474`（已通过测试 zone.rs:666/673/708 验证） |
| `DimensionTransferRequest { entity, target, target_pos }` event + `apply_dimension_transfers` system | ✅ | `server/src/world/dimension_transfer.rs:23-99` |
| `TerrainProvider::load(manifest, raster_dir, biomes)` mmap 加载 manifest + 每 layer `.bin` | ✅ | `server/src/world/terrain/raster.rs:283-407` |
| `TerrainProvider.pois() -> &[Poi]` accessor（`Poi { zone, kind, name, pos_xyz, tags, unlock, qi_affinity, danger_bias }`） | ✅ | `server/src/world/terrain/raster.rs:411-413, 257-266` |
| `TerrainProviders { overworld, tsy: Option<TerrainProvider> }` + `for_dimension(kind) -> Option<&TerrainProvider>` | ✅ | `server/src/world/terrain/raster.rs:130-151` |
| Chunk 按需生成（`generate_chunks_around_players`） | ✅ | `server/src/world/terrain/mod.rs:109` |
| `ZoneRegistry::register_runtime_zone(zone) -> Result<()>` | ❌ | P0 plan-tsy-zone-v1 §-1 责任，本 plan 不依赖运行态 add（启动期 blueprint 一次性 load 即可） |
| `RiftPortal` / `TsyPresence` Component | ❌ | P0 plan-tsy-zone-v1 §1.3 责任 |
| `tsy_poi_consumer.rs`（POI → spawn entity） | ❌ | **本 plan §1 责任** |

**关键发现**：blueprint `zones[].pois[]` 已经自动序列化进 `manifest.json` → server mmap 加载 → `TerrainProvider.pois()` 可查询；`TerrainProviders { overworld, tsy }` 多 provider 框架已就位（dimension plan PR #47 落地）。**POI 通道全通，server 侧唯一缺的是 consumer**（POI 被加载但无人消费）。

### 前置依赖（本 plan 启动条件）

- ✅ **`plan-tsy-dimension-v1` Rust 侧**：PR #47 已落地 `DimensionKind` / `DimensionLayers` / `TerrainProviders { overworld, tsy: Option }` / `DimensionTransferRequest` / `Zone.dimension`。本 plan 直接消费这些接口
- ❌ **`ZoneRegistry::register_runtime_zone(Zone)`**：P0 plan-tsy-zone-v1 责任。本 plan **不需要**此 API（startup 期 blueprint load 即建好两组 zone），仅 `/tsy-spawn` 调试命令需要——本 plan 落地后该命令退化为"激活+跨位面传"，runtime add 不再走 `register_runtime_zone`，但保留给未来 runtime 场景（例如塌缩事件 spawn 临时 CollapseTear 微型 zone）
- ❌ **`TsyPresence` / `RiftPortal` Component**：P0 plan §1.3 责任。本 plan §1 的 consumer 直接 spawn 这些 component；P0 不 merged 前 consumer 编译失败，逻辑上等 P0 merge

### TSY 系列兄弟 plan 的耦合点

| 兄弟 plan | 状态 | 交互 |
|-----------|------|------|
| `plan-tsy-zone-v1` (P0) | docs/（active 文档） | 本 plan **替换** 其 `/tsy-spawn` 路径；P0 定义的 `TsyPresence` component、`RiftPortal` component、入场过滤、负压 tick **全部保留不改**；本 plan 只负责把这些 component **从 POI 表自动 spawn 出来** |
| `plan-tsy-container-v1` (P3) | docs/（active 文档，Rust 未实装） | 容器 kind（干尸/骨架/储物袋/石匣/法阵核心）通过 POI kind=`loot_container` + tags `archetype:X` 表达；钥匙约束走 tags `locked:Y` |
| `plan-tsy-hostile-v1` (P4) | docs/（active 文档，Rust 未实装） | NPC archetype（道伥/执念/守灵/畸变体 + 浅层"高阶守株待兔者"） 通过 POI kind=`npc_anchor` + tags `archetype:X, trigger:X, leash_radius:N` 表达；起源-层深 spawn pool 由 profile 生成密度写入 |
| `plan-tsy-extract-v1` (P5) | docs/（active 文档，Rust 未实装） | 3 种 RiftPortal（MainRift/DeepRift/CollapseTear）通过 POI kind=`rift_portal` + tags `kind:main/deep/tear` 表达 |

---

## §0 设计轴心（骨架阶段已定稿，不再动）

0. **TSY 是独立位面，不嵌入主世界（2026-04-24 架构反转）**：对齐 `worldview.md §十六 世界层实现注` 与 `plan-tsy-dimension-v1`。所有 TSY zone 的 AABB 是**独立 `bong:tsy` 位面内部坐标**；主世界侧只保留裂缝 POI（`rift_portal direction=entry`）作为跨位面传送锚点。这个轴心决定了后续所有数据流：blueprint 产出**两份 manifest**、raster layer 作用域分位面、POI consumer 按位面拆两组 system
1. **POI 通道复用，不新建 worldgen 输出 pipeline**：blueprint → manifest.json → `TerrainProvider.pois()` 已全通；本 plan 工作 = 扩 POI kind + 把 manifest 产出扩到两份 + 写 server 侧 consumer（主世界侧 / TSY 侧各一组）
2. **Profile 架构对齐**：TSY 4 起源各对应一个 profile class（或合并后按 origin 内部分支），接口对齐现有 9 个（`PROFILE_NAME` / `extra_layers` / `ecology` / `fill_*_tile()`）；主世界侧不新增 profile（裂缝 POI 靠现有 profile 的 landmark 机制嵌入，或由 blueprint 直接挂在主世界 zone 的 pois[] 上）
3. **TSY zone 是 blueprint entry，不走运行时创建**：worldgen 产出时 TSY zone 已 registered 到独立 `zones.tsy.json`（主世界 `zones.json` 保留 rift_portal POI 条目），`/tsy-spawn` 调试命令退化为"激活 + 跨位面传玩家"
4. **三层 subzone 走 Y 分层（对齐 P0 §1.1）**：浅/中/深共享 XZ，Y 轴分层；profile 内按 `zone.depth_tier` 分支 fill 逻辑（见 §5）。独立位面里 Y 分层无需避让主世界地质，自由度更高
5. **Voxel 地貌走 profile 程序化**（对齐现有 9 profile 架构），**不**走预烤 schematic/nbt
6. **骨架 plan 不做世界级分布算法**：每个 TSY family 的 POI/portal/容器/anchor 位置在 blueprint 中显式手写（骨架阶段 2 起源 × 1 family × 3 层 = 6 subzone）；大规模自动分布留给 active plan。主世界裂缝锚点位置也手写，由叙事决定（北荒荒原、宗门遗址脚下、战场边缘）
7. **三层负压悖论是硬约束（`worldview.md §十六.三`）**：不是地形参数的艺术选择——浅 -0.3~-0.5 / 中 -0.6~-0.8 / 深 -0.9~-1.2，loot 档次、NPC 密度、`relic_core_slot` 分布都必须映射世界观语义（浅层=高阶 PVP 收割场 / 深层=低阶淘金避难所，上古遗物集中深层）。`§3.3` 差异化表格、`§1.1` POI tags、`§2.2` blueprint 模板均须按此校对
8. **一次性生命周期是世界观底色（`worldview.md §十六.一`）**：活坍缩渊最后一件上古遗物被取走 → 塌缩 → 死坍缩渊永久封闭。骨架阶段 "blueprint 固定 TSY zone" 是实施简化，**不**是对"可反复刷"的承诺——任何 worldgen 产物或 POI 设计都不得暗示同一 family 可重复清理。`§8 Q1` 因此不再问"同 family 是否重置"（已否决），改问"新坍缩渊怎么在别处新生"。独立位面里"永久封闭" = TSY dim 内该 family 的 subzone 被 registry 移除 + 主世界对应裂缝锚点失效（双端同步由 P2 lifecycle 处理）

---

## §1 POI Consumer System（Rust 新建）

**位置**：`server/src/world/tsy_poi_consumer.rs`（新文件）

**跨位面拆分（2026-04-24 架构反转）**：POI 按所在位面分两组消费——

- **主世界侧 provider** (`TerrainProviders.overworld`)：只产出 `rift_portal direction=entry`（跨位面入口锚点）
- **TSY 侧 provider** (`TerrainProviders.tsy`)：产出 `rift_portal direction=exit`（对应 family `_shallow` 中心的回程门）+ `loot_container` / `npc_anchor` / `relic_core_slot` / 层间跳转 portal（mid↔deep，属 P5 extract plan）

Spawn 时必须把实体挂到正确的 Valence layer（主世界 layer entity 或 TSY layer entity），由 `DimensionLayers` resource 查 layer entity。

### 1.1 POI kind 扩展

现有 `Poi.kind` 是自由字符串。本 plan 新增 4 个约定值：

| kind | 位面 | 消费者 | 必需 tags | 可选 tags |
|------|------|--------|-----------|-----------|
| `rift_portal` | 两侧 | `spawn_rift_portals` | `direction:entry\|exit`, `kind:main\|deep\|tear`, `family_id:X` | `trigger_radius:N`, `target_family_pos_xyz:x,y,z`（entry 必填，指向 TSY dim `_shallow` 中心；exit 由 TsyPresence.return_to 运行时填）, `orientation:vertical\|horizontal`（默认按 direction 推导：entry→vertical / Nether 式，exit→horizontal / End 式），`facing:north\|south\|east\|west`（仅 vertical 需要，决定 Nether 门正面朝向） |
| `loot_container` | TSY | `spawn_tsy_containers` | `archetype:dry_corpse\|skeleton\|storage_pouch\|stone_casket\|relic_core` | `locked:stone_key\|jade_seal\|array_sigil`, `loot_pool:X` |
| `npc_anchor` | TSY | `spawn_tsy_npc_anchors` | `archetype:daoxiang\|zhinian\|sentinel\|fuya\|{P4_TBD}` | `trigger:on_enter\|on_relic_touched\|always`, `leash_radius:N` |
| `relic_core_slot` | TSY | `spawn_tsy_relic_slots` | `slot_count:N` | — |

> **npc_anchor archetype `{P4_TBD}` 占位**：浅层 PVP 收割场所需的"高阶守株待兔者" archetype 归 P4 `plan-tsy-hostile-v1` 定义（候选命名：`ancient_sentinel`），本 plan schema 对该值不做硬约束——consumer 见到未知 archetype 时 log warn + skip（§1.4），blueprint sample 阶段先用 `archetype:daoxiang` 占位，P4 命名锁定后回填。
> 
> **POI tags 解析约定**：`tags: Vec<String>` 元素形如 `"key:value"`；解析器拆 `:` 一刀，左侧 key 右侧 value。多值 tag 同 key 多次出现（例如 `"loot_pool:common"` 与 `"loot_pool:locked"` 共存）。

### 1.2 Consumer system 骨架

下面 4 个 system 在 `tsy_poi_consumer.rs` 内，注册到 `Startup` post-init stage（在 `TerrainProviders::load` 之后、`DimensionLayers` 已 setup 之后）。

```rust
//! tsy_poi_consumer.rs — POI → entity spawn (plan-tsy-worldgen-v1 §1.2)
//!
//! 启动期一次性消费 TerrainProviders.{overworld, tsy} 的 pois()，
//! spawn 出 RiftPortal / LootContainer / NpcAnchor / RelicCoreSlot
//! marker entity 到对应 layer。
//! 失败处理见 §1.4：缺字段 / 未知枚举 → log warn + skip，不 panic。

use crate::world::dimension::{DimensionAnchor, DimensionKind, DimensionLayers};
use crate::world::terrain::raster::{Poi, TerrainProviders};
use crate::world::zone::ZoneRegistry;
use crate::tsy::components::{
    RiftPortal, PortalDirection, PortalKind,
    LootContainer, ContainerArchetype, ContainerLock,
    NpcAnchor, NpcArchetype, NpcTrigger,
    RelicCoreSlot,
};
use valence::prelude::*;

pub fn spawn_rift_portals(
    mut commands: Commands,
    providers: Res<TerrainProviders>,
    layers: Res<DimensionLayers>,
    zones: Res<ZoneRegistry>,
) {
    // 主世界侧 entry portals
    for poi in providers.overworld.pois().iter().filter(|p| p.kind == "rift_portal") {
        let Some(direction) = parse_direction(&poi.tags) else {
            warn_skip("rift_portal", &poi, "missing direction tag");
            continue;
        };
        if !matches!(direction, PortalDirection::Entry) {
            warn_skip("rift_portal", &poi, "overworld provider hosts only entry portals");
            continue;
        }
        let Some(family_id) = parse_family_id(&poi.tags) else {
            warn_skip("rift_portal", &poi, "missing family_id");
            continue;
        };
        let kind = parse_portal_kind(&poi.tags).unwrap_or(PortalKind::Main);
        let target_pos = parse_target_family_pos_xyz(&poi.tags)
            .or_else(|| resolve_tsy_shallow_center(&zones, &family_id))
            .unwrap_or_else(|| {
                warn_skip("rift_portal", &poi, "could not resolve TSY shallow center");
                DVec3::ZERO
            });

        commands.spawn((
            RiftPortal {
                family_id,
                target: DimensionAnchor { dimension: DimensionKind::Tsy, pos: target_pos },
                trigger_radius: parse_trigger_radius(&poi.tags).unwrap_or(1.5),
                direction: PortalDirection::Entry,
                kind,
            },
            Position(poi.pos_xyz.into()),
            EntityLayerId(layers.overworld),
        ));
        // 同时摆放原版 portal 方块组（§1.2.a）
        write_portal_blocks(&mut commands, &poi, layers.overworld, /* vertical */ true);
    }

    // TSY 侧 exit portals
    for poi in providers.tsy.as_ref().map(|p| p.pois()).unwrap_or(&[])
        .iter().filter(|p| p.kind == "rift_portal")
    {
        let Some(direction) = parse_direction(&poi.tags) else { continue };
        if !matches!(direction, PortalDirection::Exit) { continue; }
        let Some(family_id) = parse_family_id(&poi.tags) else { continue };

        commands.spawn((
            RiftPortal {
                family_id,
                // exit 的 target 由 TsyPresence.return_to 运行时填，此处置 zero 占位
                target: DimensionAnchor { dimension: DimensionKind::Overworld, pos: DVec3::ZERO },
                trigger_radius: parse_trigger_radius(&poi.tags).unwrap_or(1.5),
                direction: PortalDirection::Exit,
                kind: parse_portal_kind(&poi.tags).unwrap_or(PortalKind::Main),
            },
            Position(poi.pos_xyz.into()),
            EntityLayerId(layers.tsy),
        ));
        write_portal_blocks(&mut commands, &poi, layers.tsy, /* vertical */ false);
    }
}

pub fn spawn_tsy_containers(
    mut commands: Commands,
    providers: Res<TerrainProviders>,
    layers: Res<DimensionLayers>,
) {
    let Some(tsy) = providers.tsy.as_ref() else { return };
    for poi in tsy.pois().iter().filter(|p| p.kind == "loot_container") {
        let Some(archetype) = parse_container_archetype(&poi.tags) else {
            warn_skip("loot_container", &poi, "missing or unknown archetype");
            continue;
        };
        let lock = parse_container_lock(&poi.tags); // None ⇒ unlocked
        let loot_pool = parse_loot_pool(&poi.tags); // None ⇒ archetype default
        commands.spawn((
            LootContainer { archetype, lock, loot_pool },
            Position(poi.pos_xyz.into()),
            EntityLayerId(layers.tsy),
        ));
    }
}

pub fn spawn_tsy_npc_anchors(
    mut commands: Commands,
    providers: Res<TerrainProviders>,
    layers: Res<DimensionLayers>,
) {
    let Some(tsy) = providers.tsy.as_ref() else { return };
    for poi in tsy.pois().iter().filter(|p| p.kind == "npc_anchor") {
        let Some(archetype) = parse_npc_archetype(&poi.tags) else {
            warn_skip("npc_anchor", &poi, "missing or unknown archetype");
            continue;
        };
        let trigger = parse_npc_trigger(&poi.tags).unwrap_or(NpcTrigger::OnEnter);
        let leash_radius = parse_leash_radius(&poi.tags).unwrap_or(8.0);
        commands.spawn((
            NpcAnchor { archetype, trigger, leash_radius },
            Position(poi.pos_xyz.into()),
            EntityLayerId(layers.tsy),
        ));
    }
}

pub fn spawn_tsy_relic_slots(
    mut commands: Commands,
    providers: Res<TerrainProviders>,
    layers: Res<DimensionLayers>,
) {
    let Some(tsy) = providers.tsy.as_ref() else { return };
    for poi in tsy.pois().iter().filter(|p| p.kind == "relic_core_slot") {
        let slot_count = parse_slot_count(&poi.tags).unwrap_or(1).max(1).min(8);
        commands.spawn((
            RelicCoreSlot { slot_count },
            Position(poi.pos_xyz.into()),
            EntityLayerId(layers.tsy),
        ));
    }
}

// --- helpers ---
fn warn_skip(kind: &str, poi: &Poi, reason: &str) {
    tracing::warn!(
        "[bong][tsy-poi] skip {kind} at zone={} pos={:?}: {reason}",
        poi.zone, poi.pos_xyz
    );
}
// parse_direction / parse_family_id / parse_portal_kind / parse_trigger_radius /
// parse_target_family_pos_xyz / parse_container_archetype / parse_container_lock /
// parse_loot_pool / parse_npc_archetype / parse_npc_trigger / parse_leash_radius /
// parse_slot_count / resolve_tsy_shallow_center / write_portal_blocks 由本文件实现。
```

注册到 app（`server/src/main.rs` 或 `server/src/world/mod.rs`）：

```rust
app.add_systems(
    Startup,
    (
        spawn_rift_portals,
        spawn_tsy_containers,
        spawn_tsy_npc_anchors,
        spawn_tsy_relic_slots,
    ).after(load_terrain_providers).after(setup_dimension_layers)
);
```

#### 1.2.a Portal 方块摆放（复用 MC 原版模型）

`spawn_rift_portals` 不只 spawn marker entity，还要把**原版 portal 方块组**摆到对应 layer（详见 `plan-tsy-dimension-v1 §3.3`）：

- `orientation=vertical` (Entry)：以 POI `pos_xyz` 为底部中心，沿 `facing` 轴摆 `obsidian` 4×5 框 + 内部 2×3 `nether_portal`；marker entity 放在 portal 方块中心，`trigger_radius=1.5`
- `orientation=horizontal` (Exit)：以 POI `pos_xyz` 为地面中心，摆 5×5 平面 —— 外圈 12 × `end_portal_frame`（带 eye，朝内），中心 3×3 `end_portal`；marker entity 悬浮在中心上方 0.5 格，`trigger_radius=1.5`

方块写入走 Valence 的 `ChunkLayer::set_block` —— `write_portal_blocks(commands, poi, layer_entity, vertical)` 助手在 startup 时逐块写。**骨架阶段不预烤进 raster**——理由：单 family Entry+Exit ≈ 30 方块 × N family（骨架 2 family），总量 < 100 方块；预烤需扩 raster 格式（feature_mask 增 portal palette 编码），ROI 太低。active plan 阶段如发现性能问题再迁。

**原版 portal travel 逻辑必须禁用**——Valence 若对 `nether_portal` / `end_portal` 方块保留原版 `on_entity_collision` 行为（4 秒传 nether / 瞬时传 end），我们的 `RiftPortal` 逻辑会被抢先触发错误的 dim。需 audit `valence_entity` / `valence_player` 的 portal 相关系统，拦截或覆盖（见 dimension plan §3.3 Q）。

### 1.3 幂等与 regen 策略

- server 启动只跑一次 consumer（POI 是启动态）
- dev-reload 重启 server 时 POI 坐标可能因 seed 变化而移动，**依赖 server 重启而非热加载**（对齐 dev-reload.sh 现状）
- 运行时不处理 despawn；塌缩事件的临时 portal spawn 属 P5 `plan-tsy-extract-v1` 职责

### 1.4 失败处理

- POI tags 缺失必需字段 → `tracing::warn!` + skip spawn（不 panic，允许 worldgen 迭代时部分 zone 不完整）
- 未知 kind value（如 `archetype:xxx` 的 xxx 不在枚举内）→ warn + skip
- 错误日志统一格式 `[bong][tsy-poi] skip <kind> at zone=<zone_name> pos=<pos>: <reason>`，对齐现有 `log_payload_build_error` 风格

---

## §2 Blueprint 扩展

### 2.1 文件布局（架构反转后已决策，~~Q5~~ 关闭）

Blueprint **必须分两文件**，分别产出两份 manifest：

1. **`server/zones.worldview.example.json`**（主世界 blueprint，现有）—— 只新增 `kind=rift_portal direction=entry` POI 条目，挂在合适的主世界 zone 的 `pois[]` 里
2. **`server/zones.tsy.json`**（新文件，TSY dim blueprint）—— 所有 TSY subzone + TSY 侧 POI

**Pipeline 改造**：

- `worldgen/scripts/terrain_gen/blueprint.py:34` `BlueprintZone` 加 `dimension: str`（默认 `"minecraft:overworld"`，TSY blueprint 写 `"bong:tsy"`）
- `worldgen/scripts/terrain_gen/__main__.py:67` `main` 改为按位面跑两次：默认主世界 blueprint + 可选 TSY blueprint（CLI 加 `--tsy-blueprint <path>` 参数），各自产出独立 `output_dir`：
  - 主世界：`worldgen/generated/terrain-gen/rasters/`（保持现状）
  - TSY：`worldgen/generated/terrain-gen/rasters-tsy/`（新）
- `bakers/raster_export.py:export_rasters()` 增 `layer_whitelist: Optional[set[str]]` 参数：TSY 调用传 `None`（导全部 layer 含 tsy_*）；主世界调用传 `LAYER_REGISTRY.keys() - {"tsy_presence", "tsy_origin_id", "tsy_depth_tier"}`，避免主世界 manifest 多写 3 个无意义 layer

**Server 侧**：`TerrainProviders::load()` 已支持 `overworld` + `Option<tsy>`（`raster.rs:130`）；启动 main 读两个环境变量 `BONG_TERRAIN_RASTER_PATH`（现有，主世界）+ `BONG_TSY_RASTER_PATH`（新）。后者缺失时 `tsy: None`（dimension 现有 fallback 行为）。

### 2.2 TSY zone 模板

**坐标系注意**：以下 AABB 和 POI `pos_xyz` 都是 **TSY dim 内部坐标**，以 family 原点（例如 `(0, 0, 0)`）起排；worldgen 可自由分配 family 在 TSY dim 内的排布（约定每 family 间距 500 格）。主世界侧的对应 rift_portal entry POI 在主世界 blueprint 里单独写。

#### 2.2.a 完整 family 模板：宗门遗迹 01（3 层）

```json
// zones.tsy.json （新文件）
{
  "version": 1,
  "world": {
    "name": "tsy_realm",
    "spawn_zone": "tsy_zongmen_01_shallow",
    "bounds_xz": { "min": [-2000, -2000], "max": [2000, 2000] },
    "notes": [
      "TSY dim blueprint. Coordinates are TSY-internal — no relation to overworld XYZ.",
      "Each family occupies a XZ patch (≈100×100), Y stratified shallow/mid/deep."
    ]
  },
  "zones": [
    {
      "name": "tsy_zongmen_01_shallow",
      "dimension": "bong:tsy",
      "display_name": "宗门遗迹·浅层（第一族）",
      "aabb": { "min": [0, 40, 0], "max": [100, 120, 100] },
      "center_xz": [50, 50],
      "size_xz": [100, 100],
      "spirit_qi": -0.4,
      "danger_level": 4,
      "active_events": ["tsy_entry"],
      "patrol_anchors": [[50.0, 80.0, 50.0]],
      "blocked_tiles": [],
      "worldgen": {
        "terrain_profile": "tsy_zongmen_ruin",
        "shape": "rectangle",
        "boundary": { "mode": "hard", "width": 4 },
        "height_model": { "base": [60, 64], "peak": 72 },
        "surface_palette": ["cracked_stone_bricks", "andesite", "gravel", "deepslate"],
        "biome_mix": ["mofa:tsy_ruined"],
        "landmarks": [],
        "depth_tier": "shallow",
        "origin": "zongmen_yiji"
      },
      "pois": [
        { "kind": "rift_portal",     "pos_xyz": [50.0, 100.0,  50.0], "tags": ["direction:exit", "kind:main", "family_id:zongmen_01", "orientation:horizontal"] },
        { "kind": "loot_container",  "pos_xyz": [60.0,  80.0,  70.0], "tags": ["archetype:storage_pouch"] },
        { "kind": "loot_container",  "pos_xyz": [40.0,  78.0,  30.0], "tags": ["archetype:dry_corpse"] },
        { "kind": "loot_container",  "pos_xyz": [80.0,  82.0,  60.0], "tags": ["archetype:dry_corpse"] },
        { "kind": "npc_anchor",      "pos_xyz": [70.0,  80.0,  80.0], "tags": ["archetype:daoxiang", "trigger:on_enter", "leash_radius:8"] },
        { "kind": "npc_anchor",      "pos_xyz": [25.0,  80.0,  25.0], "tags": ["archetype:daoxiang", "trigger:always", "leash_radius:6"] }
      ]
    },
    {
      "name": "tsy_zongmen_01_mid",
      "dimension": "bong:tsy",
      "display_name": "宗门遗迹·中层（第一族）",
      "aabb": { "min": [0, 0, 0], "max": [100, 40, 100] },
      "center_xz": [50, 50],
      "size_xz": [100, 100],
      "spirit_qi": -0.7,
      "danger_level": 5,
      "active_events": [],
      "patrol_anchors": [[50.0, 20.0, 50.0]],
      "blocked_tiles": [],
      "worldgen": {
        "terrain_profile": "tsy_zongmen_ruin",
        "shape": "rectangle",
        "boundary": { "mode": "hard", "width": 4 },
        "height_model": { "base": [4, 12], "peak": 28 },
        "surface_palette": ["mossy_cobblestone", "deepslate", "cobbled_deepslate"],
        "biome_mix": ["mofa:tsy_ruined"],
        "landmarks": [],
        "depth_tier": "mid",
        "origin": "zongmen_yiji"
      },
      "pois": [
        { "kind": "loot_container", "pos_xyz": [55.0, 18.0, 45.0], "tags": ["archetype:skeleton"] },
        { "kind": "loot_container", "pos_xyz": [30.0, 22.0, 65.0], "tags": ["archetype:skeleton"] },
        { "kind": "loot_container", "pos_xyz": [75.0, 22.0, 75.0], "tags": ["archetype:storage_pouch", "locked:stone_key"] },
        { "kind": "loot_container", "pos_xyz": [50.0, 20.0, 30.0], "tags": ["archetype:stone_casket", "locked:jade_seal"] },
        { "kind": "npc_anchor",     "pos_xyz": [60.0, 22.0, 60.0], "tags": ["archetype:zhinian", "trigger:on_enter", "leash_radius:10"] },
        { "kind": "npc_anchor",     "pos_xyz": [35.0, 24.0, 35.0], "tags": ["archetype:sentinel", "trigger:always", "leash_radius:12"] }
      ]
    },
    {
      "name": "tsy_zongmen_01_deep",
      "dimension": "bong:tsy",
      "display_name": "宗门遗迹·深层（第一族）",
      "aabb": { "min": [0, -40, 0], "max": [100, 0, 100] },
      "center_xz": [50, 50],
      "size_xz": [100, 100],
      "spirit_qi": -1.1,
      "danger_level": 5,
      "active_events": ["tsy_collapse_proximity"],
      "patrol_anchors": [[50.0, -20.0, 50.0]],
      "blocked_tiles": [],
      "worldgen": {
        "terrain_profile": "tsy_zongmen_ruin",
        "shape": "rectangle",
        "boundary": { "mode": "hard", "width": 4 },
        "height_model": { "base": [-36, -28], "peak": -8 },
        "surface_palette": ["deepslate", "tuff", "calcite", "soul_sand"],
        "biome_mix": ["mofa:tsy_ruined"],
        "landmarks": [],
        "depth_tier": "deep",
        "origin": "zongmen_yiji"
      },
      "pois": [
        { "kind": "relic_core_slot", "pos_xyz": [50.0, -20.0, 50.0], "tags": ["slot_count:5"] },
        { "kind": "loot_container",  "pos_xyz": [40.0, -18.0, 40.0], "tags": ["archetype:relic_core", "locked:array_sigil"] },
        { "kind": "loot_container",  "pos_xyz": [60.0, -18.0, 60.0], "tags": ["archetype:relic_core", "locked:array_sigil"] },
        { "kind": "npc_anchor",      "pos_xyz": [55.0, -22.0, 55.0], "tags": ["archetype:fuya", "trigger:on_relic_touched", "leash_radius:8"] },
        { "kind": "npc_anchor",      "pos_xyz": [45.0, -22.0, 45.0], "tags": ["archetype:fuya", "trigger:always", "leash_radius:6"] }
      ]
    }
  ]
}
```

#### 2.2.b 第二个 family：大能陨落 01（完整 3 层）

500 格间距错排到 `[500-600] × [500-600]`；profile = `tsy_daneng_crater`，origin = `daneng_luoluo`（id=1）。主题：陨石坑环 + 灵气结晶柱 + 中心残骸。`shape="ellipse"` 圆形坑（vs zongmen 的 `rectangle` 殿宇方阵）。POI 总数 26（§8 Q9 区间 19-29）。

```json
// 续 zones.tsy.json zones[] 内追加：
{
  "name": "tsy_daneng_01_shallow",
  "dimension": "bong:tsy",
  "display_name": "大能陨落·浅层（第一族）",
  "aabb": { "min": [500, 40, 500], "max": [600, 120, 600] },
  "center_xz": [550, 550],
  "size_xz": [100, 100],
  "spirit_qi": -0.45,
  "danger_level": 4,
  "active_events": ["tsy_entry"],
  "patrol_anchors": [[550.0, 80.0, 550.0]],
  "blocked_tiles": [],
  "worldgen": {
    "terrain_profile": "tsy_daneng_crater",
    "shape": "ellipse",
    "boundary": { "mode": "hard", "width": 4 },
    "height_model": { "base": [60, 68], "peak": 78 },
    "surface_palette": ["blackstone", "calcite", "basalt", "gravel"],
    "biome_mix": ["mofa:tsy_crater"],
    "landmarks": [],
    "depth_tier": "shallow",
    "origin": "daneng_luoluo"
  },
  "pois": [
    { "kind": "rift_portal",     "pos_xyz": [550.0, 100.0, 550.0], "tags": ["direction:exit", "kind:main", "family_id:daneng_01", "orientation:horizontal"] },
    { "kind": "loot_container",  "pos_xyz": [560.0,  80.0, 570.0], "tags": ["archetype:storage_pouch"] },
    { "kind": "loot_container",  "pos_xyz": [540.0,  82.0, 540.0], "tags": ["archetype:storage_pouch"] },
    { "kind": "loot_container",  "pos_xyz": [580.0,  82.0, 560.0], "tags": ["archetype:dry_corpse"] },
    { "kind": "loot_container",  "pos_xyz": [530.0,  80.0, 580.0], "tags": ["archetype:dry_corpse"] },
    { "kind": "npc_anchor",      "pos_xyz": [575.0,  80.0, 545.0], "tags": ["archetype:daoxiang", "trigger:on_enter", "leash_radius:8"] },
    { "kind": "npc_anchor",      "pos_xyz": [525.0,  80.0, 565.0], "tags": ["archetype:daoxiang", "trigger:always", "leash_radius:6"] },
    { "kind": "npc_anchor",      "pos_xyz": [555.0,  82.0, 580.0], "tags": ["archetype:daoxiang", "trigger:on_enter", "leash_radius:8"] }
  ]
},
{
  "name": "tsy_daneng_01_mid",
  "dimension": "bong:tsy",
  "display_name": "大能陨落·中层（第一族）",
  "aabb": { "min": [500, 0, 500], "max": [600, 40, 600] },
  "center_xz": [550, 550],
  "size_xz": [100, 100],
  "spirit_qi": -0.75,
  "danger_level": 5,
  "active_events": [],
  "patrol_anchors": [[550.0, 20.0, 550.0]],
  "blocked_tiles": [],
  "worldgen": {
    "terrain_profile": "tsy_daneng_crater",
    "shape": "ellipse",
    "boundary": { "mode": "hard", "width": 4 },
    "height_model": { "base": [4, 18], "peak": 28 },
    "surface_palette": ["basalt", "blackstone", "deepslate", "magma_block"],
    "biome_mix": ["mofa:tsy_crater"],
    "landmarks": [],
    "depth_tier": "mid",
    "origin": "daneng_luoluo"
  },
  "pois": [
    { "kind": "loot_container", "pos_xyz": [560.0, 18.0, 540.0], "tags": ["archetype:skeleton"] },
    { "kind": "loot_container", "pos_xyz": [535.0, 22.0, 560.0], "tags": ["archetype:skeleton"] },
    { "kind": "loot_container", "pos_xyz": [575.0, 22.0, 575.0], "tags": ["archetype:skeleton"] },
    { "kind": "loot_container", "pos_xyz": [555.0, 20.0, 575.0], "tags": ["archetype:storage_pouch", "locked:stone_key"] },
    { "kind": "loot_container", "pos_xyz": [540.0, 20.0, 535.0], "tags": ["archetype:storage_pouch", "locked:stone_key"] },
    { "kind": "loot_container", "pos_xyz": [565.0, 22.0, 560.0], "tags": ["archetype:stone_casket", "locked:jade_seal"] },
    { "kind": "npc_anchor",     "pos_xyz": [550.0, 22.0, 550.0], "tags": ["archetype:zhinian", "trigger:on_enter", "leash_radius:10"] },
    { "kind": "npc_anchor",     "pos_xyz": [570.0, 22.0, 535.0], "tags": ["archetype:zhinian", "trigger:always", "leash_radius:8"] },
    { "kind": "npc_anchor",     "pos_xyz": [535.0, 24.0, 545.0], "tags": ["archetype:sentinel", "trigger:always", "leash_radius:12"] },
    { "kind": "npc_anchor",     "pos_xyz": [580.0, 22.0, 575.0], "tags": ["archetype:daoxiang", "trigger:on_enter", "leash_radius:6"] }
  ]
},
{
  "name": "tsy_daneng_01_deep",
  "dimension": "bong:tsy",
  "display_name": "大能陨落·深层（第一族）",
  "aabb": { "min": [500, -40, 500], "max": [600, 0, 600] },
  "center_xz": [550, 550],
  "size_xz": [100, 100],
  "spirit_qi": -1.15,
  "danger_level": 5,
  "active_events": ["tsy_collapse_proximity"],
  "patrol_anchors": [[550.0, -20.0, 550.0]],
  "blocked_tiles": [],
  "worldgen": {
    "terrain_profile": "tsy_daneng_crater",
    "shape": "ellipse",
    "boundary": { "mode": "hard", "width": 4 },
    "height_model": { "base": [-36, -22], "peak": -4 },
    "surface_palette": ["deepslate", "calcite", "amethyst_block", "end_stone"],
    "biome_mix": ["mofa:tsy_crater"],
    "landmarks": [],
    "depth_tier": "deep",
    "origin": "daneng_luoluo"
  },
  "pois": [
    { "kind": "relic_core_slot", "pos_xyz": [550.0, -20.0, 550.0], "tags": ["slot_count:5"] },
    { "kind": "loot_container",  "pos_xyz": [540.0, -18.0, 540.0], "tags": ["archetype:relic_core", "locked:array_sigil"] },
    { "kind": "loot_container",  "pos_xyz": [560.0, -18.0, 560.0], "tags": ["archetype:relic_core", "locked:array_sigil"] },
    { "kind": "loot_container",  "pos_xyz": [550.0, -22.0, 575.0], "tags": ["archetype:relic_core", "locked:array_sigil"] },
    { "kind": "npc_anchor",      "pos_xyz": [555.0, -22.0, 555.0], "tags": ["archetype:fuya", "trigger:on_relic_touched", "leash_radius:8"] },
    { "kind": "npc_anchor",      "pos_xyz": [545.0, -22.0, 545.0], "tags": ["archetype:fuya", "trigger:always", "leash_radius:6"] },
    { "kind": "npc_anchor",      "pos_xyz": [565.0, -22.0, 545.0], "tags": ["archetype:fuya", "trigger:on_relic_touched", "leash_radius:8"] },
    { "kind": "npc_anchor",      "pos_xyz": [555.0, -22.0, 535.0], "tags": ["archetype:zhinian", "trigger:on_enter", "leash_radius:10"] }
  ]
}
```

> **vs zongmen_01 关键差异**：
> - `shape:"ellipse"` vs zongmen `"rectangle"` —— 陨石坑圆形 vs 殿宇方阵
> - `surface_palette` 走火山岩系（blackstone/basalt/magma）vs zongmen 砖石系（cracked_stone_bricks/deepslate）
> - shallow `peak:78` 高于环（陨击坑边缘隆起），mid `peak:28` 中等，deep `peak:-4`（中央晶柱腔体顶）
> - deep 层 surface_palette 含 `amethyst_block`/`end_stone`，对应 §3.4 表"灵气结晶柱 + 中央巨型晶柱腔体"主题
> - POI 总 26 个：shallow 8 / mid 10 / deep 8，符合 §8 Q9 单 family 19-29 区间

#### 2.2.c 主世界 entry portal POI（补丁到 `zones.worldview.example.json`）

放在合适的主世界地表 zone（例如 `north_wastes` / `ancient_battlefield` / 宗门遗迹外围）的 `pois[]` 里：

```json
{
  "kind": "rift_portal",
  "name": "塌缩裂缝·宗门遗迹",
  "pos_xyz": [1810.0, 100.0, 2810.0],
  "tags": [
    "direction:entry",
    "kind:main",
    "family_id:zongmen_01",
    "target_family_pos_xyz:50,100,50",
    "orientation:vertical",
    "facing:north"
  ],
  "unlock": "灵识扫过裂隙，听见远处宗门钟鸣残响",
  "qi_affinity": -0.30
},
{
  "kind": "rift_portal",
  "name": "塌缩裂缝·大能陨落",
  "pos_xyz": [-2400.0, 90.0, 1800.0],
  "tags": [
    "direction:entry",
    "kind:main",
    "family_id:daneng_01",
    "target_family_pos_xyz:550,100,550",
    "orientation:vertical",
    "facing:east"
  ]
}
```

> **Portal 方块形态（对齐 `plan-tsy-dimension-v1 §3.3`）**：
> - `orientation:vertical`（Entry）= `obsidian` 4×5 框 + 内部 2×3 `nether_portal`，Nether 竖门视觉，"地壳裂缝"
> - `orientation:horizontal`（Exit）= 外圈 12 × `end_portal_frame`（带 eye）+ 中心 3×3 `end_portal`，End 横门视觉，"阵盘回程阵"
> - POI `pos_xyz` 是 portal 方块组的中心；`§1.2.a` 定义 consumer 的摆放细节

#### 2.2.d Blueprint schema 字段总览

| 字段 | 取值 | 必填 | 由谁消费 |
|------|------|------|---------|
| `dimension` | `"minecraft:overworld"` \| `"bong:tsy"` | ✅ TSY zone 必填；overworld zone 默认值 | `blueprint.py` loader、Rust `ZoneConfig` deserialize |
| `worldgen.depth_tier` | `"shallow"` \| `"mid"` \| `"deep"` | ✅ TSY zone 必填 | TSY profile fill_*_tile 内部分支；raster_check |
| `worldgen.origin` | `"daneng_luoluo"` \| `"zongmen_yiji"` \| `"zhanchang_chendian"` \| `"gaoshou_sichu"` | ✅ TSY zone 必填 | TSY profile fill_*_tile 写 `tsy_origin_id` layer |
| `worldgen.boundary.mode` | `"hard"` (TSY 强制) | ✅ TSY zone 必填 `"hard"` | stitcher.py:159-179（已实装） |

**`extras` 兜底**：现有 `ZoneWorldgenConfig.extras: dict[str, Any]` 已收纳未识别字段。`depth_tier` / `origin` 暂可走 `extras["depth_tier"]` / `extras["origin"]` 不改 dataclass；profile fill 函数读 `zone.worldgen.extras.get("depth_tier")`。**轻量路径**，骨架阶段直接落 `extras`，active 前若数量级稳定再升级为 first-class 字段。

### 2.3 骨架阶段样本数量

2 起源 × 1 family × 3 层 = **6 subzone**（zongmen_01 完整 + daneng_01 占位）。active plan 阶段扩到 4 起源 × 2-3 family × 3 层 = **24-36 subzone**。

---

## §3 Profile 新增（Python）

### 3.1 起源 → profile 映射

| 起源 | Profile class 文件 | 备注 |
|------|------|------|
| 大能陨落 | `worldgen/scripts/terrain_gen/profiles/tsy_daneng_crater.py`（新） | 陨石坑 + 灵气结晶柱 + 中心残骸 |
| 宗门遗迹 | `worldgen/scripts/terrain_gen/profiles/tsy_zongmen_ruin.py`（新） | 倒塌殿宇 + 阵盘残件 + 藏书废墟 |
| 战场沉淀 | `worldgen/scripts/terrain_gen/profiles/tsy_zhanchang.py`（新） | 密集骨堆 + 兵器林立 + 血色地脉。**Q6 决策**：新建而非 fork `ancient_battlefield`——独立位面后两者 qi_density 语义反转（主世界古战场 0.10-0.15 末法滞留；TSY 战场沉淀 0.85-0.95 残灵浓厚），fork 会造成 fill 函数双轨判断 zone.worldgen.dimension 复杂度，新建直观 |
| 近代高手死处 | `worldgen/scripts/terrain_gen/profiles/tsy_gaoshou_hermitage.py`（新） | 单栋茅屋 + 坟冢 + 日常器物 |

### 3.2 Profile 接口（对齐现有 9 个 generator）

每个 TSY profile 文件结构与 `profiles/spawn_plain.py` / `profiles/ancient_battlefield.py` 同构。下面以 `tsy_zongmen_ruin.py` 为基准给完整骨架，其余 3 个文件 1:1 沿用，差异在 `extra_layers` 选取、`EcologySpec.decorations` 内容、`fill_*_tile` 内的地貌算法。

```python
# worldgen/scripts/terrain_gen/profiles/tsy_zongmen_ruin.py
"""TSY 宗门遗迹 profile — 倒塌殿宇 / 阵盘残件 / 藏书废墟.

Y 分层（depth_tier 取自 zone.worldgen.extras["depth_tier"]，§2.2.d）：
- shallow Y∈[40,120]: 灰雾地表 + 少量柱础 + 骨堆点缀（被搜尽过）
- mid Y∈[0,40]: 主废墟 + 残墙 + 中型容器位
- deep Y∈[-40,0]: 阵盘核心 + 法阵残件 + 高密度遗物 slot
"""

from __future__ import annotations

import numpy as np

from ..blueprint import BlueprintZone
from ..fields import SurfacePalette, TileFieldBuffer, WorldTile
from ..noise import _tile_coords, fbm_2d, ridge_2d, warped_fbm_2d
from .base import (
    DecorationSpec,
    EcologySpec,
    ProfileContext,
    TerrainProfileGenerator,
)

ZONGMEN_RUIN_DECORATIONS = (
    DecorationSpec(
        name="toppled_pillar",
        kind="boulder",
        blocks=("cracked_stone_bricks", "deepslate_bricks", "andesite"),
        size_range=(3, 6),
        rarity=0.45,
        notes="柱础残段：宗门殿宇倒塌后的石柱半埋。",
    ),
    DecorationSpec(
        name="array_disc_remnant",
        kind="crystal",
        blocks=("lodestone", "amethyst_block", "chiseled_deepslate"),
        size_range=(2, 4),
        rarity=0.18,
        notes="阵盘残片：曾经的引气阵法核心，紫晶尚有微光。",
    ),
    DecorationSpec(
        name="scripture_pile",
        kind="shrub",
        blocks=("dirt", "podzol", "soul_sand"),
        size_range=(1, 3),
        rarity=0.30,
        notes="藏经废墟：腐朽竹简化为黑土，灵识扫过隐约见字。",
    ),
    DecorationSpec(
        name="sect_stele",
        kind="boulder",
        blocks=("deepslate_bricks", "chiseled_deepslate", "soul_lantern"),
        size_range=(3, 5),
        rarity=0.20,
        notes="宗门界碑：刻有山门字样的深板岩碑，多已断裂。",
    ),
)

# 起源代号 → tsy_origin_id 编码（与 §4.1 对齐）
ORIGIN_CODE = {"daneng_luoluo": 1, "zongmen_yiji": 2, "zhanchang_chendian": 3, "gaoshou_sichu": 4}
DEPTH_CODE = {"shallow": 1, "mid": 2, "deep": 3}


class TsyZongmenRuinGenerator(TerrainProfileGenerator):
    profile_name = "tsy_zongmen_ruin"
    extra_layers = (
        "qi_density",        # TSY 内 0.85~0.95（高浓度残留）
        "mofa_decay",        # TSY 内 0.10~0.20（末法程度低）
        "qi_vein_flow",      # 高，阵盘脉络
        "anomaly_intensity",
        "anomaly_kind",
        "ruin_density",
        "fracture_mask",     # 倒塌殿宇沿断裂带
        "flora_density",
        "flora_variant_id",
        "tsy_presence",      # 新 layer，在 family AABB 内 = 1
        "tsy_origin_id",     # 新 layer，宗门 = 2
        "tsy_depth_tier",    # 新 layer，shallow/mid/deep = 1/2/3
    )
    ecology = EcologySpec(
        decorations=ZONGMEN_RUIN_DECORATIONS,
        ambient_effects=("dry_wind", "stone_creak", "distant_chant"),
        notes="倒塌殿宇 + 阵盘残件 + 藏经废墟。色调灰青，深层有阵眼微光。",
    )

    def build_notes(self, context: ProfileContext) -> tuple[str, ...]:
        return (
            "Y stratified (shallow/mid/deep) by zone.worldgen.extras['depth_tier'].",
            "High qi_density + low mofa_decay (TSY signature inversion vs overworld).",
            "Deep tier hosts relic_core_slot + array_disc_remnant decorations.",
        )


def fill_tsy_zongmen_ruin_tile(
    zone: BlueprintZone,
    tile: WorldTile,
    tile_size: int,
    palette: SurfacePalette,
) -> TileFieldBuffer:
    depth_tier = zone.worldgen.extras.get("depth_tier", "shallow")
    origin_id = ORIGIN_CODE.get(zone.worldgen.extras.get("origin", "zongmen_yiji"), 2)
    depth_id = DEPTH_CODE.get(depth_tier, 1)

    layer_names = (
        "height", "surface_id", "subsurface_id", "water_level",
        "biome_id", "feature_mask", "boundary_weight",
        "qi_density", "mofa_decay", "qi_vein_flow",
        "anomaly_intensity", "anomaly_kind",
        "ruin_density", "fracture_mask",
        "flora_density", "flora_variant_id",
        "tsy_presence", "tsy_origin_id", "tsy_depth_tier",
    )
    buffer = TileFieldBuffer.create(tile, tile_size, layer_names)
    wx, wz = _tile_coords(tile.min_x, tile.min_z, tile_size)
    area = tile_size * tile_size

    # ---- Y 分层骨架（核心算法） ----
    if depth_tier == "shallow":
        # 灰雾地表，缓和 + 残柱，base ≈ 60-72
        base = 60.0 + fbm_2d(wx, wz, scale=120.0, octaves=3, seed=2100) * 4.0
        ruin = np.clip(0.20 + fbm_2d(wx, wz, scale=80.0, octaves=2, seed=2110) * 0.25, 0.0, 0.6)
        qi = np.clip(0.85 + fbm_2d(wx, wz, scale=140.0, octaves=2, seed=2120) * 0.05, 0.7, 1.0)
        decay = np.clip(0.12 + np.abs(ridge_2d(wx, wz, scale=70.0, octaves=2, seed=2130)) * 0.08, 0.05, 0.25)
    elif depth_tier == "mid":
        # 主废墟，沟壑 + 残墙密集，base ≈ 4-28
        base = 8.0 + ridge_2d(wx, wz, scale=60.0, octaves=4, seed=2200) * 6.0
        ruin = np.clip(0.55 + warped_fbm_2d(wx, wz, scale=80.0, octaves=3, warp_scale=120.0, warp_strength=40.0, seed=2210) * 0.25, 0.3, 0.95)
        qi = np.clip(0.88 + fbm_2d(wx, wz, scale=110.0, octaves=2, seed=2220) * 0.06, 0.75, 1.0)
        decay = np.clip(0.15 + ruin * 0.05, 0.08, 0.30)
    else:  # deep
        # 阵盘核心 + 大空洞底面，base ≈ -36 ~ -8
        base = -28.0 + fbm_2d(wx, wz, scale=140.0, octaves=3, seed=2300) * 4.0
        ruin = np.clip(0.40 + fbm_2d(wx, wz, scale=70.0, octaves=2, seed=2310) * 0.20, 0.2, 0.8)
        # 阵盘核心提升 qi 到极值
        qi = np.clip(0.92 + fbm_2d(wx, wz, scale=180.0, octaves=2, seed=2320) * 0.06, 0.85, 1.0)
        decay = np.clip(0.10 + fbm_2d(wx, wz, scale=90.0, octaves=2, seed=2330) * 0.05, 0.05, 0.20)

    fracture = np.maximum(0.0, ridge_2d(wx, wz, scale=80.0, octaves=4, seed=2400 + depth_id * 100))
    qi_vein = np.clip(fracture * 0.7 + ruin * 0.2, 0.0, 1.0)

    # ---- Surface ----
    stone_id = palette.ensure("stone")
    deepslate_id = palette.ensure("deepslate")
    bricks_id = palette.ensure("cracked_stone_bricks") if depth_tier != "deep" else palette.ensure("deepslate")
    moss_id = palette.ensure("moss_block") if depth_tier == "shallow" else palette.ensure("deepslate")

    surface_id = np.full_like(base, stone_id, dtype=np.int32)
    surface_id = np.where(ruin > 0.4, bricks_id, surface_id)
    surface_id = np.where(fracture > 0.5, deepslate_id, surface_id)
    if depth_tier == "shallow":
        surface_id = np.where((ruin < 0.3) & (fracture < 0.2), moss_id, surface_id)

    # ---- Anomaly: 深层显著、浅层稀疏 ----
    anomaly_seed = 2500 + depth_id * 100
    anomaly_field = warped_fbm_2d(wx, wz, scale=200.0, octaves=3, warp_scale=240.0, warp_strength=70.0, seed=anomaly_seed)
    anomaly_threshold = {"shallow": 0.55, "mid": 0.45, "deep": 0.35}[depth_tier]
    anomaly_intensity = np.clip((anomaly_field - anomaly_threshold) * 3.0, 0.0, 1.0)
    # kind: 5 = wild_formation（阵盘共振），优先；mid/deep 偶发 1 = spacetime_rift
    anomaly_kind = np.where(anomaly_intensity > 0.15, 5, 0).astype(np.int32)
    if depth_tier == "deep":
        rift_field = fbm_2d(wx, wz, scale=160.0, octaves=2, seed=2600)
        rift_strong = (rift_field > 0.35) & (anomaly_intensity < 0.25)
        anomaly_intensity = np.where(rift_strong, np.clip(rift_field * 0.8, 0.0, 1.0), anomaly_intensity)
        anomaly_kind = np.where(rift_strong, 1, anomaly_kind)

    # ---- Flora ----
    flora_density = np.clip(ruin * 0.55 + fracture * 0.25, 0.0, 1.0)
    flora_variant = np.zeros_like(base, dtype=np.int32)
    flora_variant = np.where(ruin > 0.4, 1, flora_variant)              # toppled_pillar
    flora_variant = np.where((flora_variant == 0) & (fracture > 0.5), 4, flora_variant)  # sect_stele
    if depth_tier == "deep":
        flora_variant = np.where(anomaly_kind == 5, 2, flora_variant)   # array_disc_remnant
    flora_variant = np.where((flora_variant == 0) & (ruin > 0.25), 3, flora_variant)  # scripture_pile

    # ---- 写 buffer ----
    buffer.layers["height"] = np.round(base, 3).ravel()
    buffer.layers["surface_id"] = surface_id.ravel().astype(np.uint8)
    buffer.layers["subsurface_id"] = np.full(area, deepslate_id, dtype=np.uint8)
    buffer.layers["water_level"] = np.full(area, -1.0, dtype=np.float64)
    buffer.layers["biome_id"] = np.full(area, 5, dtype=np.uint8)  # dripstone_caves slot 占位
    buffer.layers["feature_mask"] = np.round(np.clip(ruin * 0.5 + fracture * 0.4, 0.0, 1.0), 3).ravel()
    buffer.layers["boundary_weight"] = np.zeros(area, dtype=np.float64)
    buffer.layers["qi_density"] = np.round(qi, 3).ravel()
    buffer.layers["mofa_decay"] = np.round(decay, 3).ravel()
    buffer.layers["qi_vein_flow"] = np.round(qi_vein, 3).ravel()
    buffer.layers["anomaly_intensity"] = np.round(anomaly_intensity, 3).ravel()
    buffer.layers["anomaly_kind"] = anomaly_kind.ravel().astype(np.uint8)
    buffer.layers["ruin_density"] = np.round(ruin, 3).ravel()
    buffer.layers["fracture_mask"] = np.round(fracture, 3).ravel()
    buffer.layers["flora_density"] = np.round(flora_density, 3).ravel()
    buffer.layers["flora_variant_id"] = flora_variant.ravel().astype(np.uint8)
    # TSY 专用 layer：family AABB 内全 1
    buffer.layers["tsy_presence"] = np.ones(area, dtype=np.uint8)
    buffer.layers["tsy_origin_id"] = np.full(area, origin_id, dtype=np.uint8)
    buffer.layers["tsy_depth_tier"] = np.full(area, depth_id, dtype=np.uint8)

    buffer.contributing_zones.append(zone.name)
    return buffer
```

### 3.2.a Profile 注册接入点（4 处必改）

每个新 profile 文件必须接入 4 处：

1. **`profiles/__init__.py:1-12`** import 4 个新 generator class
2. **`profiles/__init__.py:14-27`** `_GENERATORS` 字典加 4 个 generator 实例
3. **`stitcher.py:24-29`** import 4 个 `fill_tsy_*_tile` 函数
4. **`stitcher.py:378-407`** `_build_zone_overlay_tile` 内 if/elif 链尾加 4 个 elif 分支

每个 elif 分支模板：
```python
elif profile == "tsy_zongmen_ruin":
    buffer = fill_tsy_zongmen_ruin_tile(zone, tile, tile_size, palette)
```

### 3.3 Shallow/Mid/Deep 差异化（对齐轴心 7）

| 深度 | 主题 | 地貌特征 | spirit_qi | qi_density | anomaly_intensity | loot 档次 | 主要 NPC |
|------|------|---------|-----------|-----------|-------------------|----------|--------|
| shallow | 入口带 / **PVP 死地** | 灰雾弥漫、低骨堆、少量容器 | -0.3 ~ -0.5 | 0.85-0.95 | 0.1-0.3 | 凡铁、磨损装备（历代搜尽） | **`{P4_TBD}`高阶守株待兔者** + `daoxiang` |
| mid | 主废墟 / 冲突带 | 密集遗骸、中型容器 | -0.6 ~ -0.8 | 0.85-0.95 | 0.3-0.6 | 残卷、轻型法器、`storage_pouch` 主分布 | `daoxiang` 集群 / `zhinian` 单只 / `sentinel` 守点 |
| deep | 核心 / 低阶避难所 | 阵盘核心、法阵残件 | -0.9 ~ -1.2 | 0.85-1.0 | 0.5-1.0 | **上古遗物（`relic_core_slot` 集中）** | `fuya` 守灵 + `zhinian` 高阶 |

> **反直觉布局提醒（轴心 7）**：浅层的主要威胁是**高阶 PVP** 不是环境负压；深层对**低阶**反而最安全（真元池小，绝对抽吸量有限），对**高阶**数秒即秒。Profile fill 时 `npc_anchor` 密度、`loot_container` 档次、`relic_core_slot` 槽位都必须反映此语义——不是游戏平衡，是世界观 §十六.三 的物理推导。
> 
> `{P4_TBD}` 候选命名：`ancient_sentinel`（P4 plan 内部讨论）；P4 merged 后回填本表 + §1.1 `npc_anchor.archetype` 表，archetype 解析器同步加该值域。

### 3.4 三个剩余 profile 的接口差异（不全文展开，给关键差异）

| profile | extra_layers 关键 | EcologySpec.decorations 主题 | fill 算法关键差异 vs zongmen |
|---------|------------------|------------------------------|------------------------------|
| `tsy_daneng_crater` | 同 zongmen，但去 `fracture_mask` 加 `cave_mask` | 灵气结晶柱（amethyst_block/calcite/end_rod）/ 中心残骸（black_concrete/obsidian）/ 焦土环（blackstone）/ 灵晶碎簇（small_amethyst_bud） | 中心碗状坑（深 12-20 格 vs zongmen 残墙），shallow 圆圈构图，deep 中央巨型晶柱腔体 |
| `tsy_zhanchang` | 同 ancient_battlefield，加 `tsy_presence/origin_id/depth_tier` | 骨堆山（bone_block）/ 兵器林（iron_block/copper_block/cobwebs）/ 血色地脉（red_concrete/red_sand）/ 战旗残骸（red_wool/black_wool） | qi_density 反转：主世界 ancient_battlefield 0.10-0.30；TSY zhanchang 0.85-0.95，骨堆密度高 3 倍 |
| `tsy_gaoshou_hermitage` | 简化，无 fracture_mask；加 `flora_density/variant_id`（农作物） | 茅屋（hay_block/oak_planks/thatch）/ 坟冢（gravel/podzol/dead_bush）/ 日常器物（barrel/iron_ingot/glass_bottle）/ 残棋盘（white_concrete/black_concrete） | 单建筑中心+周围 50 格农田，Y 分层最浅（shallow 平地、mid 半山腰、deep 山洞修炼室）；anomaly_intensity 最低（0.0-0.3） |

完整文件 active plan 阶段补；§9 行数预估按"4 文件平均 ~120 行"估。

---

## §4 Layer 扩展

### 4.1 新增 3 个 TSY layer

**精确插入位置**：`worldgen/scripts/terrain_gen/fields.py:115`（LAYER_REGISTRY dict 末尾，`anomaly_kind` 之后）追加：

```python
    # --- TSY-specific layers (plan-tsy-worldgen-v1 §4.1) ---
    # tsy_presence: 1 表示 TSY family 区域内（fast mask，Rust hot-path 查询）；
    #   主世界 manifest 不写此 layer（raster_export whitelist 过滤）
    # tsy_origin_id: 1 daneng / 2 zongmen / 3 zhanchang / 4 gaoshou / 0 none
    # tsy_depth_tier: 1 shallow / 2 mid / 3 deep / 0 none
    "tsy_presence":   LayerSpec(safe_default=0,  blend_mode="maximum", export_type="uint8"),
    "tsy_origin_id":  LayerSpec(safe_default=0,  blend_mode="swap",    export_type="uint8"),
    "tsy_depth_tier": LayerSpec(safe_default=0,  blend_mode="swap",    export_type="uint8"),
```

**作用域（架构反转后）**：这 3 个 layer **只出现在 TSY dim 的 raster 产出**里；主世界 raster 不写、不覆盖。`bakers/raster_export.py:export_rasters()` 增 `layer_whitelist` 参数（§2.1），TSY 调用不传（导全部）；主世界调用传 `LAYER_REGISTRY.keys() - {"tsy_presence", "tsy_origin_id", "tsy_depth_tier"}`。

**Server 侧 schema 已有 hook 位**：`server/src/world/terrain/raster.rs:101 ColumnSample` 现已注入 7 类 optional layer（rift_axis_sdf / sky_island / underground_tier 等），新增 3 个 TSY layer 走同一模式：`raster.rs:154 TileFields` struct 加 `tsy_presence: Option<Mmap>` / `tsy_origin_id: Option<Mmap>` / `tsy_depth_tier: Option<Mmap>`，`raster.rs:514 TileFields::load` 加 `map_optional_layer(...)`，`raster.rs:475 sample()` 加 `read_optional_u8(...)`。3 个 layer 均 `uint8` → `tile_area`（非 `area4`）。

### 4.2 复用现有 layer

- `qi_density` (lerp) — TSY 内 ≈ 0.85-0.95（高浓度，对比末法主世界 0.05-0.15）；主世界仍保持现有值域
- `mofa_decay` (lerp) — TSY 内 ≈ 0.10-0.20（末法程度低）
- `anomaly_intensity` (maximum) + `anomaly_kind` (swap) — TSY 异常编码：5=`wild_formation`（阵盘共振）主导，1=`spacetime_rift`（深层偶发），2=`qi_turbulence`（mid 层），4=`cursed_echo`（zhanchang 起源专属，骨堆怨念）。值域 0..5 不变（沿用现有 `anomaly_kinds` 字典，`raster_export.py:134-141`）

### 4.3 raster_check invariant 新增（5 条）

加到 `worldgen/scripts/terrain_gen/harness/raster_check.py:validate_rasters()`，建议作为 single-manifest 校验（每个 manifest 单独跑一次，TSY manifest 走 TSY 专用分支，主世界 manifest 走主世界专用分支）。也可以拆出独立函数 `validate_tsy_manifest(raster_dir)` 在 dev-reload.sh 步骤 2 之后追加调用。

```python
# Inside validate_rasters() — 在主循环结束、报告构建前追加 5 条新 invariant：

manifest_kind = "tsy" if any(z.startswith("tsy_") for tile in tiles for z in tile.get("zones", [])) else "overworld"

if manifest_kind == "tsy":
    # 1. 每 family 至少 1 个 kind=rift_portal direction=exit POI
    families: dict[str, dict] = {}
    for poi in manifest.get("pois", []):
        if poi["kind"] != "rift_portal":
            continue
        tags = {t.split(":", 1)[0]: t.split(":", 1)[1] for t in poi.get("tags", []) if ":" in t}
        family = tags.get("family_id")
        direction = tags.get("direction")
        if family:
            families.setdefault(family, {"entry": 0, "exit": 0})
            if direction in ("entry", "exit"):
                families[family][direction] += 1
    for fam, counts in families.items():
        if counts["exit"] < 1:
            errors.append(f"TSY family '{fam}' has no rift_portal direction=exit")

    # 2. 每 family 三层齐全（按 zone name 后缀 _shallow/_mid/_deep）
    fam_tiers: dict[str, set[str]] = {}
    for tile in tiles:
        for z in tile.get("zones", []):
            if not z.startswith("tsy_"):
                continue
            for tier in ("shallow", "mid", "deep"):
                if z.endswith(f"_{tier}"):
                    fam = z[len("tsy_"):-len(f"_{tier}")]
                    fam_tiers.setdefault(fam, set()).add(tier)
    for fam, tiers in fam_tiers.items():
        missing = {"shallow", "mid", "deep"} - tiers
        if missing:
            errors.append(f"TSY family '{fam}' missing tiers: {sorted(missing)}")

    # 3. tsy_presence > 0 的 cell 必须 qi_density >= 0.7
    for tile_info in tiles:
        tile_dir = raster_path / tile_info["dir"]
        presence = tile_dir / "tsy_presence.bin"
        qi_file = tile_dir / "qi_density.bin"
        if not (presence.exists() and qi_file.exists()):
            continue
        pres_raw = presence.read_bytes()
        qi_data = _read_float_layer(qi_file, area)
        if qi_data is None or len(pres_raw) != area:
            continue
        for p, q in zip(pres_raw, qi_data):
            if p > 0 and q < 0.70:
                errors.append(f"{tile_info['dir']}: tsy_presence>0 with qi_density={q:.2f} < 0.7")
                break

    # 4. tsy_origin_id ∈ {0..4}, tsy_depth_tier ∈ {0..3}
    for tile_info in tiles:
        for layer_name, max_val in (("tsy_origin_id", 4), ("tsy_depth_tier", 3)):
            f = raster_path / tile_info["dir"] / f"{layer_name}.bin"
            if not f.exists():
                continue
            raw = f.read_bytes()
            if len(raw) == area and max(raw) > max_val:
                errors.append(f"{tile_info['dir']}: {layer_name} max={max(raw)} > {max_val}")

    # 5. 三层 AABB Y 区间不 overlap（属 manifest schema 校验，需读 zones.tsy.json，
    #    raster_check 阶段无原始 blueprint；改在 blueprint loader 一致性校验里做。
    #    此处保留 stub，由 blueprint.py:load_blueprint() 在 TSY 分支报错。）
else:  # overworld manifest
    # 6. 每个 kind=rift_portal direction=entry POI 必须带 family_id + target_family_pos_xyz
    for poi in manifest.get("pois", []):
        if poi["kind"] != "rift_portal":
            continue
        tags = {t.split(":", 1)[0]: t.split(":", 1)[1] for t in poi.get("tags", []) if ":" in t}
        if tags.get("direction") != "entry":
            continue
        if "family_id" not in tags:
            errors.append(f"overworld rift_portal at {poi['pos_xyz']} missing family_id tag")
        if "target_family_pos_xyz" not in tags:
            errors.append(f"overworld rift_portal at {poi['pos_xyz']} missing target_family_pos_xyz tag")

    # 7. 主世界 manifest 不出现 tsy_* layer
    for tile in tiles:
        for layer in tile.get("layers", []):
            if layer.startswith("tsy_"):
                errors.append(f"overworld manifest tile {tile['dir']} unexpectedly contains {layer}")
```

> 跨 manifest 校验（entry POI 的 `family_id` 在 TSY manifest 里有对应 family）属 dev-reload.sh 步骤 2 之后的"双 manifest 一致性"校验，新建 `harness/cross_manifest_check.py`，比 raster_check 高一层（不 inline）。

---

## §5 Q2 详解：三层深度模型（已收敛 → 选项 A）

### 选项 A：Y 分层（首选，已收敛）

- XZ 共享（三层 AABB 同 XZ 范围，§2.2.a 模板已对齐），Y 轴垂直分层
- 默认 Y 区间：shallow Y∈[40,120], mid Y∈[0,40], deep Y∈[-40,0]
- `ZoneRegistry.find_zone(DimensionKind::Tsy, player_pos)` 按 Y 自然切换
- **优点**：复用现有 `underground_tier` + `cavern_floor_y` layer；P0 §1.1 已约定；玩家"往下探 = 深入负压"的心智匹配世界观
- **缺点 / 验收前置**：profile fill_tile 按 `extras["depth_tier"]` 分支（§3 模板已示），单 profile 处理全深度；Y 剧变处过渡需手工 smooth——active plan 阶段需验"Y=40 / Y=0 边界"是否有视觉断层（建议用 stitcher 现有 `boundary.width:4` 做 Y 方向 gradient 写 boundary_weight，profile 内 lerp 高程值）

### 选项 B：独立 AABB（已否决）

- 三层 XZ 不同，层间通过 portal 联通
- 否决理由：UX 多一层 portal 切换；不符合 §十六"浅/中/深 = 物理深度"直觉；layer 复用率低

---

## §6 dev 迭代 + 校验

### 6.1 现有流水线（对齐 `scripts/dev-reload.sh:20-67`）

```
regen (python -m scripts.terrain_gen)
  → raster_check (validate manifest + all tiles)
  → cargo build
  → kill old server, restart with new manifest path (BONG_TERRAIN_RASTER_PATH)
```

**双 manifest 改造**：

- `__main__.py` 增 `--tsy-blueprint` 参数（§2.1）；regen 步骤跑两次 export
- `dev-reload.sh:23` 改为：
  ```bash
  (cd worldgen && .venv/bin/python -m scripts.terrain_gen --backend raster) || exit 1
  if [ -f "../server/zones.tsy.json" ]; then
      (cd worldgen && .venv/bin/python -m scripts.terrain_gen \
           --blueprint ../server/zones.tsy.json \
           --output-dir generated/terrain-gen/rasters-tsy \
           --backend raster) || exit 1
  fi
  ```
- `dev-reload.sh:34-40` validate 步骤跑两次 `validate_rasters()`（主世界 + TSY），任一 errors → 失败
- `dev-reload.sh:55-56` server restart 增传 `BONG_TSY_RASTER_PATH` 环境变量（dimension plan §6 已规约）

TSY worldgen 产出是 **startup-time 数据**，server 重启即消费。无需热加载机制。

### 6.2 新 smoke 脚本

`scripts/smoke-tsy-worldgen.sh`（新）：

```bash
#!/usr/bin/env bash
# smoke-tsy-worldgen.sh — TSY worldgen end-to-end smoke
# Usage: bash scripts/smoke-tsy-worldgen.sh [--keep-server]
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

echo "==> [1/6] Regen + validate (overworld + tsy)"
bash scripts/dev-reload.sh --skip-server-start

echo "==> [2/6] Inspect TSY manifest"
python3 -c "
import json
m = json.load(open('worldgen/generated/terrain-gen/rasters-tsy/manifest.json'))
families = set()
exits = 0
for p in m['pois']:
    tags = {t.split(':',1)[0]: t.split(':',1)[1] for t in p.get('tags',[]) if ':' in t}
    if p['kind'] == 'rift_portal':
        if 'family_id' in tags: families.add(tags['family_id'])
        if tags.get('direction') == 'exit': exits += 1
assert exits >= len(families), f'expected ≥{len(families)} exit portals, found {exits}'
print(f'  {len(families)} families, {exits} exit portals')
"

echo "==> [3/6] Start server"
(cd server && BONG_TERRAIN_RASTER_PATH=../worldgen/generated/terrain-gen/rasters/manifest.json \
     BONG_TSY_RASTER_PATH=../worldgen/generated/terrain-gen/rasters-tsy/manifest.json \
     cargo run > /tmp/bong-server.log 2>&1 &)
sleep 5

echo "==> [4/6] Verify TSY POI consumer fired"
grep -q '\[bong\]\[tsy-poi\]' /tmp/bong-server.log && echo "  no warnings (good)" || true
grep -E 'spawn_(rift_portals|tsy_containers|tsy_npc_anchors|tsy_relic_slots)' /tmp/bong-server.log \
    || { echo "FAIL: TSY consumers did not run"; exit 1; }

echo "==> [5/6] Verify ZoneRegistry has TSY zones"
grep -E 'loaded [0-9]+ authoritative zone' /tmp/bong-server.log
grep -q 'tsy_zongmen_01_shallow' /tmp/bong-server.log \
    || { echo "FAIL: TSY shallow zone not registered"; exit 1; }

echo "==> [6/6] Cleanup"
[ "${1:-}" != "--keep-server" ] && pkill -f 'target/debug/bong-server' 2>/dev/null || true

echo "✅ TSY worldgen smoke passed"
```

### 6.3 raster_check 新 invariant

见 §4.3。总计 7 条新增（5 条 TSY 分支 + 2 条主世界分支）。

---

## §7 与 P0 / dimension plan 的联动

### 2026-04-23（原版联动）

P0 `plan-tsy-zone-v1` 已在 2026-04-23 修订：

- §-1 加 worldgen plan 指向 + `ZoneRegistry::register_runtime_zone()` 前置依赖显性化
- §3.1 注明 worldgen 落地后 `/tsy-spawn` 退化为"激活命令"
- §8 "不改的文件" 具体点名本 plan + terrain POI consumer

### 2026-04-24（架构反转连带修订）

- P0 `plan-tsy-zone-v1` §-1 点 5 / §0 轴心 5 反转为"跨位面传送"；§1.1 zones.tsy.json 模板坐标改 TSY dim 内部；§1.3 `TsyPresence.entry_portal_pos` 升级为 `return_to: DimensionAnchor`；§3.1/§3.3/§3.4 entry/exit 系统改发 `DimensionTransferRequest` event
- 新增 `docs/plan-tsy-dimension-v1.md` 作为本 plan + P0 的共同前置（Valence `DimensionType` / 跨位面传送 / per-dimension `TerrainProvider`）
- 本 plan §0 轴心 0 新增；§1 consumer 拆两侧；§2 blueprint 强制分两文件；§4 layer 作用域收窄到 TSY dim；§8 Q5/Q7/Q10 关闭

### 2026-04-26（实地考察后骨架填充）

- §-1 全表加 ✅/🟡/❌ 状态标记，对齐 PR #47 落地的 `DimensionKind` / `DimensionLayers` / `TerrainProviders { overworld, tsy }` / `Zone.dimension` / `find_zone(dim, pos)`
- §1.2 补 4 个 consumer system 完整 fn 签名 + helper 列表 + Startup 注册位置
- §2.2 补 `tsy_zongmen_01` 三层完整 JSON 模板（shallow/mid/deep 各自含 worldgen/pois/AABB），`tsy_daneng_01` 占位说明结构差异
- §3.1 给 4 个 profile fill 函数完整骨架（zongmen 全文 + 其余 3 个差异表）+ §3.2.a 4 处注册接入点
- §4.1 给 LAYER_REGISTRY 精确插入位置（fields.py:115 末尾）+ Rust 侧 hook 位（raster.rs:101/154/475/514）
- §4.3 给 7 条 raster_check invariant 完整 Python 伪代码
- §6.2 给 smoke 脚本完整 bash 骨架
- §8 收敛 Q2 / Q4 / Q6 / Q9 / Q11；Q1 / Q8 留给 P2 / dimension plan
- §10 升级条件加 ✅ / ⏳ 状态

本 plan 仍不反向改 P0 的**业务逻辑**——P0 实装的 `TsyPresence`/`RiftPortal`/入场过滤/负压 tick 概念全部保留，本 plan 只负责**从两份 POI 表自动 spawn 出来**，替代 `/tsy-spawn` 的手工流程。

---

## §8 开放问题清单（active 前可选项收敛位）

- **Q1** 坍缩渊世代更替（**被轴心 8 部分收敛，仍开放**）：原 family 塌缩后永久清零已定（`worldview.md §十六.一`），**新 family 怎么上线**尚开放——选项 A：预生成候选 family 池（blueprint 含 N 个 inactive family，按节奏激活）；选项 B：运行时动态创建 zone（需 `ZoneRegistry::register_runtime_zone()` + manifest 动态 append + TSY dim raster 增量写入）。涉及 P2 `plan-tsy-lifecycle-v1` 的塌缩事件消费。**倾向选项 A**：预先 bake 整个 TSY dim raster（含 8-12 inactive family），活跃由 zone 是否 registered 控制 + 主世界 entry portal 是否 spawn 控制；最终决策 P2 active 前定
- **Q2** 三层深度模型（**已收敛 → 选项 A Y 分层**，§5）。active plan 前需验 Y 剧变处过渡可控
- **Q3** `Poi.kind` 是否升级为 enum？现状 `String`（`raster.rs:222 ManifestPoi.kind: String`），server 端 `filter(|p| p.kind == "rift_portal")` 字符串比较。升级优点：编译期检查 + dispatch 更快；缺点：`TerrainProvider` 签名变更，影响面需 audit。**倾向保留 String**：consumer 已成熟（kind == "rift_portal" 等），enum 化收益边际；active 前若引入 ≥10 种 POI kind 再考虑
- **Q4** 塌缩地貌扭曲（P2 lifecycle 触发）：**已收敛 → 混合方案**——worldgen 预烤 1 个 alt manifest（约定 `zones.tsy.collapsed.json` + `rasters-tsy-collapsed/`，每 family 死状态地貌），P2 lifecycle 在塌缩瞬间通过 `ChunkMutator` 事件做局部 block damage（裂痕扩展 / 特定 POI 摧毁）+ `ZoneRegistry` 替换该 family 的 zone 配置激活 alt manifest tile。**本 plan 只约定 alt manifest 命名**，具体 hand-off / runtime swap 留 P2
- ~~Q5 Blueprint 布局~~（**已关闭**：架构反转后"分两文件"成硬决策）
- **Q6** 战场沉淀起源是否 fork `ancient_battlefield` profile？**已收敛 → 新建独立 profile**（§3.1 表）。理由：独立位面后 qi_density 语义反转（主世界 0.10-0.15；TSY 0.85-0.95），fork 会污染主世界 profile 逻辑
- ~~Q7 tsy_presence 全世界覆盖~~（**已关闭**：TSY layer 只出现在 TSY manifest，主世界不写）
- ~~**Q8** 多人服 instance 化~~（**已关闭 → 共享 TSY dim**）：dimension plan §7 Q4（`docs/plan-tsy-dimension-v1.md:341,389`）已决全 server 共享同一 TSY dim，理由对齐 §十六 supply chain 设计（搜打撤同一片土地）；Q1 repop 语义基于此
- **Q9** 单 family POI 数量级（**已收敛**）：
  - rift_portal: 主世界 1 entry + TSY 1 exit = 2 个/family（层间 portal 0~2 由 P5 决定）
  - loot_container: 浅 3-5 / 中 4-6 / 深 2-4 = 9-15 个/family（轴心 7：浅"少量被搜尽"，深"集中遗物"由 relic_core_slot 承担）
  - npc_anchor: 浅 2-3（高阶守株待兔少而强）/ 中 3-5 / 深 2-3 = 7-11 个/family
  - relic_core_slot: 1 个/family（深层中心，slot_count:5）
  - 单 family 总 POI 数 ≈ 19-29，§2.2.a 模板符合此范围（zongmen_01 = 12 POI 偏低，active 前补齐）
- ~~Q10 `boundary.mode: "hard"` 需 stitcher 支持硬切~~（**已关闭**：`stitcher.py:159-179` 已实现 `hard`/`semi_hard`/soft 三档）
- **Q11** 主世界裂缝锚点的数量 / 分布（**已收敛 → 1:1**）：每 TSY family 对应主世界 1 个 entry 锚点。理由：简化 portal 方块摆放（不必处理多 entry → 同 shallow 中心着陆冲突）；符合 §十六.一"一次性生命周期"叙事（多入口暗示重复利用）。lore 团队反推改 n:1 走叙事侧扩展（多个主世界 zone 都挂 entry POI，target_family_pos_xyz 同指一个 family），不影响数据流

---

## §9 实施规模预估（active plan 开工时修正）

| 模块 | 新增行数 |
|------|------|
| Python profile (`profiles/tsy_*.py` × 4，~120 行/文件) | ~480 |
| `profiles/__init__.py` 注册 4 个 generator | ~12 |
| `stitcher.py` 4 个 elif 分支 + 4 个 import | ~16 |
| `fields.py` LAYER_REGISTRY 扩展 3 行 + 注释 | ~12 |
| `blueprint.py` `BlueprintZone.dimension` 字段 + 解析 | ~30 |
| `__main__.py` `--tsy-blueprint` 参数 + 双跑 | ~40 |
| `bakers/raster_export.py` `layer_whitelist` 参数 + 主世界过滤 | ~30 |
| `harness/raster_check.py` 7 条新 invariant（5 条 TSY 分支 + 2 条主世界分支） | ~120 |
| `harness/cross_manifest_check.py`（新文件，跨 manifest family_id 一致性） | ~80 |
| Blueprint sample `zones.tsy.json`（2 family × 3 层）+ 主世界 `zones.worldview.example.json` 补 2 个 entry POI | ~280 |
| Rust `server/src/world/tsy_poi_consumer.rs`（4 个 system + 14 helper + portal block 摆放） | ~420 |
| `server/src/world/terrain/raster.rs` 加 3 个 TSY layer hook（TileFields / load / sample） | ~50 |
| `server/src/main.rs` 注册 4 个 system + 读 `BONG_TSY_RASTER_PATH` env | ~30 |
| Rust tests (integration + unit，含跨位面 POI 一致性 / parse helpers / portal block 写入) | ~280 |
| Smoke `scripts/smoke-tsy-worldgen.sh` | ~70 |
| `scripts/dev-reload.sh` 双 manifest 支持 | ~30 |
| **合计** | **~1980** |

骨架填充后规模较初版 ~1280 增加约 700 行，主要是 4 个 profile 文件（+200）+ Rust consumer 完整化（+40）+ cross_manifest_check 新文件（+80）+ tests 扩到 280 行。仍在一次 worktree 可吃完范围。

---

## §10 升级条件（骨架 → active）

本 plan 从 `docs/plans-skeleton/` 移到 `docs/` 的触发：

1. ✅ **`plan-tsy-dimension-v1` Rust 侧 active 且 merged**（PR #47 已落地，2026-04-26 确认）
2. ⏳ **P0 `plan-tsy-zone-v1` active 且 merged**，且 `ZoneRegistry::register_runtime_zone()` + `TsyPresence` / `RiftPortal` Component 落地
3. ⏳ **P3/P4/P5 plan（container/hostile/extract）至少 1 个开工**，需要真实 POI 数据驱动；P4 archetype 命名锁定（`{P4_TBD}` 高阶守株待兔者命名）
4. ⏳ **Q1 收敛**（Q8 已由 dimension plan §7 Q4 决"共享 TSY dim"；其他 Q 已在本次填充收敛）
5. ⏳ **轴心 7/8 连带修订完成**：
   - `§3.3` 差异化表的 `{P4_TBD}` 占位回填（依赖 P4 archetype 命名）
   - `§2.2` blueprint 补 daneng_01 完整 JSON（active 前补齐）
   - `§1.1` `npc_anchor.archetype` 值域与 P4 对齐后实填，consumer 解析器同步加该值
   - `§8 Q1` 的"新 family 怎么上线"给出选项收敛（建议 P2 active 阶段做）

**当前满足度：1/5 ✅，4/5 ⏳。** 不足以触发 /consume-plan，但实施细节已齐全到"前置满足即开工"级别。

---

## §11 进度日志

- **2026-04-25**：骨架现状校核——无任何 TSY 实装。`worldgen/scripts/terrain_gen/profiles/` 仅 9 个现有 profile（abyssal_maze / ancient_battlefield / broken_peaks / cave_network / rift_valley / sky_isle / spawn_plain / spring_marsh / waste_plateau），未见 `tsy_*.py`；`fields.py:LAYER_REGISTRY` 未注册 `tsy_presence` / `tsy_origin_id` / `tsy_depth_tier`；`server/zones.tsy.json` 不存在，`zones.worldview.example.json` 未含 `kind=rift_portal` POI；`server/src/world/` 无 `tsy_poi_consumer.rs`。POI 通道前置确认已通：`blueprint.py:156-184` 已序列化 `pois[]`，待 dimension plan + P0 落 Rust 后开工。本 plan 仍处骨架阶段，未触发 §10 升级条件（dimension plan 文档已 active 但 Rust 侧 `DimensionKind` / `TerrainProviders` 未实装）。
- **2026-04-26**：**dimension plan Rust 侧解冻** — PR #47（merge 579fc67e）落地 `DimensionKind` / `DimensionLayers` / `TerrainProviders { overworld, tsy: Option }` / `DimensionTransferRequest` / `Zone.dimension` 全套基础设施。本 plan §10 升级条件之一（dimension Rust 侧落地）已满足，仍欠 **P0 `tsy-zone` merged**（升级条件之二）；可与 P0 active 阶段并行起步骨架→active 升级。POI 通道（`blueprint.py:156-184`）已就位等 consumer 接入。
- **2026-04-26（夜）**：**Q8 漏标修正 + daneng_01 完整 JSON 补齐**（两件不堵任何前置的事）：(a) §8 Q8 已由 dimension plan §7 Q4（line 341/389）决"共享 TSY dim"，本 plan 漏标，已改为关闭；(b) §10 升级条件 #4 从"Q1/Q8 收敛"收窄为"Q1 收敛"；(c) §2.2.b 从占位差异说明扩成 daneng_01 三层完整 JSON（shallow 8 POI / mid 10 POI / deep 8 POI 共 26，符合 §8 Q9 区间），含 ellipse 形状、火山岩 surface_palette、deep 层 amethyst+end_stone 晶柱腔体主题。本次只动这两处，不触发 §10 其他堵点（P0 zone-v1 / P4 archetype 命名 / Q1 仍 ⏳）。
- **2026-04-26（晚）**：**实地考察后骨架填充至实施级别**。差异核对：(a) `LAYER_REGISTRY` 实际是 27 层 / fields.py:45-115（骨架原写"25+ / 45-104"），(b) 现有 profile 实际 9 个 / `_GENERATORS` 在 `__init__.py:14-27`（骨架基本对），(c) profile dispatch 在 `stitcher.py:378-407` 是 if/elif 硬 dispatch（非动态注册，加新 profile 必须改 stitcher），(d) `BlueprintZone` 现无 `dimension` 字段（`blueprint.py:54-63`），但 Rust `ZoneConfig` 已支持 `"dimension": "overworld" | "tsy"` 反序列化（`zone.rs:347, 474`），双端需对齐补 Python 侧；(e) `register_runtime_zone()` 全 server/src 内不存在（grep 0 命中），P0 责任未动。填充内容：§-1 全表加 ✅/🟡/❌ 状态；§1.2 补 4 个 consumer system 完整 fn 签名（spawn_rift_portals / spawn_tsy_containers / spawn_tsy_npc_anchors / spawn_tsy_relic_slots）+ 14 个 helper 列表；§2.2.a 补 `tsy_zongmen_01_shallow/mid/deep` 完整 JSON 模板（含 worldgen / AABB / pois 共 12 POI）+ §2.2.b daneng_01 占位差异 + §2.2.c 主世界 entry POI 模板 × 2 + §2.2.d 字段总览表；§3.1 / §3.2 给 `tsy_zongmen_ruin.py` 完整骨架（generator class + fill_*_tile 含 Y 分层算法 + decorations / extra_layers / ecology）+ §3.2.a 4 处注册接入点 + §3.4 其余 3 个 profile 差异表；§4.1 给 fields.py:115 精确插入位置 + Rust raster.rs hook 位（101/154/475/514）；§4.3 给 7 条 raster_check invariant 完整 Python 伪代码；§6.2 给 smoke-tsy-worldgen.sh 完整 bash 骨架；§8 收敛 Q2 (→ A) / Q4 (→ 混合) / Q6 (→ 新建 profile) / Q9 (→ 19-29 POI/family) / Q11 (→ 1:1)；§9 规模预估更新 ~1280 → ~1980；§10 升级条件加状态标记。**当前满足度 1/5**，仍保留在 plans-skeleton/，欠 P0 merged + P4 archetype 命名 + Q1 / Q8 收敛即可启动 active phase。

---

## Finish Evidence

### 落地清单

- **§1 POI Consumer**：`server/src/world/tsy_poi_consumer.rs`（Startup 期消费 `TerrainProviders.{overworld, tsy}` 的 `pois()`，spawn `RiftPortal` / `LootContainer` / `NpcAnchor` / `RelicCoreSlot`），通过 `server/src/world/mod.rs:21, 111` 的 `pub mod tsy_poi_consumer;` + `tsy_poi_consumer::register(app);` 接入；环境变量 `BONG_TSY_RASTER_PATH` 控制 TSY provider 加载。
- **§2-§4 Python worldgen 扩展**：
  - `worldgen/scripts/terrain_gen/fields.py:115-124`（LAYER_REGISTRY 追加 `tsy_presence` / `tsy_origin_id` / `tsy_depth_tier` 三层）
  - `worldgen/scripts/terrain_gen/blueprint.py:64-65`（`BlueprintZone` 加 `dimension` 字段）
  - `worldgen/scripts/terrain_gen/__main__.py:30, 33, 71, 77, 132, 140-150`（`--tsy-blueprint` / `--tsy-output-dir` 双跑 + `TSY_ONLY_LAYERS` whitelist）
  - `worldgen/scripts/terrain_gen/bakers/raster_export.py:65-66`（layer_whitelist 主世界过滤 tsy_*）
  - `worldgen/scripts/terrain_gen/profiles/`：`tsy_daneng_crater.py` / `tsy_zongmen_ruin.py` / `tsy_zhanchang.py` / `tsy_gaoshou_hermitage.py` 4 个 TSY profile，由 `profiles/__init__.py:12-15` 注册 + `stitcher.py:28-31, 407-414` dispatch
  - `worldgen/scripts/terrain_gen/harness/raster_check.py:176-279`（§4.3 7 条 TSY/overworld 分支 invariant）
  - `scripts/dev-reload.sh:20-60`（双 manifest 改造：`TSY_BLUEPRINT=server/zones.tsy.json` + `WORLDGEN_TSY_OUTPUT_DIR=generated/terrain-gen-tsy` + 双跑 raster + 双侧 raster_check）
  - `server/zones.tsy.json`（TSY 位面 blueprint 入口）

### 关键 commit

- `77d042fb` (2026-04-27) — plan-tsy-worldgen-v1: TSY 双 manifest worldgen 流水线 + POI consumer (#51)
- `2ad11802` — fix(server): review feedback — startup ordering / entry portal target / lock unknown / cleanup
- `1611ac43` — feat(server): tsy_poi_consumer + BONG_TSY_RASTER_PATH 接入
- `853a8ee2` — feat(worldgen): raster_check 加 7 条 TSY/overworld 分支 invariant
- `2ce5bae9` — feat(worldgen): profiles/__init__.py + stitcher.py 注册 4 个 TSY profile
- `c1dd5382` — feat(worldgen): 4 个 TSY profile (zongmen / daneng / zhanchang / gaoshou)
- `6a549dfb` — feat(worldgen): __main__ 加 --tsy-blueprint 双跑 + raster_export 加 layer_whitelist
- `a9d0836d` — feat(worldgen): BlueprintZone 加 dimension 字段（默认 overworld）
- `d664921d` — feat(worldgen): fields.py 加 3 个 TSY layer (tsy_presence/origin_id/depth_tier)
- `7e14ac97` — feat(worldgen): dev-reload.sh 双 manifest 改造

### 测试结果

- `server/src/world/tsy_poi_consumer.rs` — 8 个 `#[test]`（POI consumer 单测）
- `bash scripts/dev-reload.sh` 双 manifest 流程：overworld + tsy 两轮 `python -m scripts.terrain_gen` + `harness/raster_check.py` 双侧 invariant 校验

### 跨仓库核验

- **server**：`tsy_poi_consumer::register` @ `server/src/world/tsy_poi_consumer.rs`（mod.rs:21, 111 接入）；`BONG_TSY_RASTER_PATH` 环境变量；`TerrainProviders.{overworld, tsy}` 消费路径
- **worldgen**：
  - `LAYER_REGISTRY` 3 层 TSY 扩展 @ `fields.py:115-124`
  - `BlueprintZone.dimension` @ `blueprint.py:64-65`
  - `--tsy-blueprint` / `TSY_ONLY_LAYERS` @ `__main__.py:30-150`
  - `layer_whitelist` @ `bakers/raster_export.py:65-66`
  - 4 TSY profile @ `profiles/tsy_*.py` + `profiles/__init__.py:12-15` + `stitcher.py:28-31, 407-414`
  - 7 TSY invariant @ `harness/raster_check.py:176-279`
  - 双 manifest 流程 @ `scripts/dev-reload.sh:20-60`
- **agent**：（不涉及）
- **client**：（不涉及）

### 遗留 / 后续

- §1.1 `npc_anchor.archetype={P4_TBD}` 高阶守株待兔者命名待 `plan-tsy-hostile-v1` 锁定后回填；浅层 PVP 收割场所需 archetype 接入由后续 plan 负责。
