# TSY Worldgen · plan-tsy-worldgen-v1（骨架）

> 坍缩渊的地形 / POI / NPC anchor 自动生成：把 TSY 4 起源 × 3 层深度作为 blueprint + profile 对接进现有 worldgen 栈，产出**独立 TSY 位面**的地形 raster + 主世界侧的裂缝锚点 POI；`plan-tsy-zone-v1` 的 `/tsy-spawn` 调试命令落地后退化为"激活已注册 TSY zone"。**骨架阶段**：列接口与决策点，不下笔实装。
> 交叉引用：`plan-tsy-v1.md §1`（依赖图）· `plan-tsy-dimension-v1 §2 §6`（位面基础设施前置）· `plan-tsy-zone-v1.md §-1 §3.1`（P0 前置）· `worldview.md §十六 世界层实现注 §十六.一`（位面决策 + 4 起源）

---

## §-1 前提（现有代码基线）

### Python worldgen 栈（已成熟）

| 能力 | 位置 |
|------|------|
| LAYER_REGISTRY（25+ 层，含 qi_density/mofa_decay/anomaly_*） | `worldgen/scripts/terrain_gen/fields.py:45-104` |
| Blueprint JSON（zone + pois[] 已可序列化） | `server/zones.worldview.example.json`（结构参考） |
| Pipeline 主入口（load_blueprint → plan → synthesize → export） | `worldgen/scripts/terrain_gen/__main__.py:67-102` |
| Profile 注册（9 个现有：spawn_plain / broken_peaks / spring_marsh / rift_valley / cave_network / waste_plateau / sky_isle / abyssal_maze / ancient_battlefield） | `worldgen/scripts/terrain_gen/profiles/__init__.py:14-28` |
| Stitcher（zone→wilderness 按 blend_mode） | `worldgen/scripts/terrain_gen/stitcher.py:191-300` |
| Raster 导出（little-endian；每 layer 一个 `.bin`；manifest.json 带 pois[]） | `worldgen/scripts/terrain_gen/bakers/raster_export.py:97, 170-188` |
| dev-reload（regen → raster_check → cargo build → restart） | `scripts/dev-reload.sh:23-68` |
| Raster invariant（8 条现有校验） | `worldgen/scripts/terrain_gen/harness/raster_check.py:1-202` |

### Rust server 栈（POI 通道已通，缺 consumer）

| 能力 | 位置 |
|------|------|
| `TerrainProvider::load()` mmap 加载 manifest + 每 layer `.bin` | `server/src/world/terrain/raster.rs:251,483` |
| `TerrainProvider.pois()` accessor（返回 blueprint POI 列表） | `server/src/world/terrain/raster.rs:302-315,379` |
| Chunk 按需生成（`generate_chunks_around_players`） | `server/src/world/terrain/mod.rs:109` |
| ZoneRegistry 启动时 load | `server/src/world/zone.rs:128-133` |

**关键发现**：blueprint `zones[].pois[]` 已经自动序列化进 `manifest.json` → server mmap 加载 → `TerrainProvider.pois()` 可查询。**POI 通道全通，server 侧唯一缺的是 consumer**（POI 被加载但无人消费）。

### 前置依赖

- **`plan-tsy-dimension-v1`（基础设施前置，2026-04-24 架构反转新增）**：提供 `DimensionKind` enum、TSY `DimensionType` 注册、TSY `LayerBundle` setup、`TerrainProviders { overworld, tsy }` 多 provider routing、`DimensionTransferRequest` 事件。本 plan 消费这些接口：worldgen 产出两份 manifest（主世界 / TSY dim），server 侧分别 mmap 进对应 provider
- **`ZoneRegistry::register_runtime_zone(Zone)`（由 P0 补足）**：P0 plan-tsy-zone-v1 的 `/tsy-spawn` 调试命令依赖此能力（现有 `apply_runtime_records` 只改属性不加 zone）。本 plan 落地后，该能力不再被 dev 调试路径使用（blueprint 启动即 load），但保留给未来 runtime 场景（例如塌缩事件 spawn 临时 CollapseTear 对应的微型 zone）
- **`Zone.dimension` 字段（由 dimension plan Q2 候选 A）**：每个 zone 带位面归属，`ZoneRegistry.find_zone(dim, pos)` 按位面 + 坐标查询；blueprint 产出时必填

### TSY 系列兄弟 plan 的耦合点

| 兄弟 plan | 交互 |
|-----------|------|
| `plan-tsy-zone-v1` (P0) | 本 plan **替换** 其 `/tsy-spawn` 路径；P0 定义的 `TsyPresence` component、`RiftPortal` component、入场过滤、负压 tick **全部保留不改**；本 plan 只负责把这些 component **从 POI 表自动 spawn 出来** |
| `plan-tsy-container-v1` (P3) | 容器 kind（干尸/骨架/储物袋/石匣/法阵核心）通过 POI kind=`loot_container` + tags `archetype:X` 表达；钥匙约束走 tags `locked:Y` |
| `plan-tsy-hostile-v1` (P4) | NPC archetype（道伥/执念/守灵/畸变体）通过 POI kind=`npc_anchor` + tags `archetype:X, trigger:X, leash_radius:N` 表达；起源-层深 spawn pool 由 profile 生成密度写入 |
| `plan-tsy-extract-v1` (P5) | 3 种 RiftPortal（MainRift/DeepRift/CollapseTear）通过 POI kind=`rift_portal` + tags `kind:main/deep/tear` 表达 |

---

## §0 设计轴心（骨架阶段已定稿，不再动）

0. **TSY 是独立位面，不嵌入主世界（2026-04-24 架构反转）**：对齐 `worldview.md §十六 世界层实现注` 与 `plan-tsy-dimension-v1`。所有 TSY zone 的 AABB 是**独立 `bong:tsy` 位面内部坐标**；主世界侧只保留裂缝 POI（`rift_portal direction=entry`）作为跨位面传送锚点。这个轴心决定了后续所有数据流：blueprint 产出**两份 manifest**、raster layer 作用域分位面、POI consumer 按位面拆两组 system
1. **POI 通道复用，不新建 worldgen 输出 pipeline**：blueprint → manifest.json → `TerrainProvider.pois()` 已全通；本 plan 工作 = 扩 POI kind + 把 manifest 产出扩到两份 + 写 server 侧 consumer（主世界侧 / TSY 侧各一组）
2. **Profile 架构对齐**：TSY 4 起源各对应一个 profile class（或合并后按 origin 内部分支），接口对齐现有 9 个（`PROFILE_NAME` / `EXTRA_LAYERS` / `ECOLOGY` / `fill_*_tile()`）；主世界侧不新增 profile（裂缝 POI 靠现有 profile 的 landmark 机制嵌入，或由 blueprint 直接挂在主世界 zone 的 pois[] 上）
3. **TSY zone 是 blueprint entry，不走运行时创建**：worldgen 产出时 TSY zone 已 registered 到独立 `zones.tsy.json`（主世界 `zones.json` 保留 rift_portal POI 条目），`/tsy-spawn` 调试命令退化为"激活 + 跨位面传玩家"
4. **三层 subzone 走 Y 分层（对齐 P0 §1.1）**：浅/中/深共享 XZ，Y 轴分层；profile 内按 `zone.depth_tier` 分支 fill 逻辑（见 §5 Q2）。独立位面里 Y 分层无需避让主世界地质，自由度更高
5. **Voxel 地貌走 profile 程序化**（对齐现有 9 profile 架构），**不**走预烤 schematic/nbt
6. **骨架 plan 不做世界级分布算法**：每个 TSY family 的 POI/portal/容器/anchor 位置在 blueprint 中显式手写（骨架阶段 2-4 个样本 family）；大规模自动分布留给 active plan。主世界裂缝锚点位置也手写，由叙事决定（北荒荒原、宗门遗址脚下、战场边缘）
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
| `rift_portal` | 两侧 | `spawn_rift_portals` | `direction:entry\|exit`, `kind:main\|deep\|tear`, `family_id:X` | `trigger_radius:N`, `target_family_pos_xyz:[x,y,z]`（entry 必填，指向 TSY dim `_shallow` 中心；exit 由 TsyPresence.return_to 运行时填）, `orientation:vertical\|horizontal`（默认按 direction 推导：entry→vertical / Nether 式，exit→horizontal / End 式），`facing:north\|south\|east\|west`（仅 vertical 需要，决定 Nether 门正面朝向） |
| `loot_container` | TSY | `spawn_tsy_containers` | `archetype:dry_corpse\|skeleton\|storage_pouch\|stone_casket\|relic_core` | `locked:stone_key\|jade_seal\|array_sigil`, `loot_pool:X` |
| `npc_anchor` | TSY | `spawn_tsy_npc_anchors` | `archetype:daoxiang\|zhinian\|sentinel\|fuya` | `trigger:on_enter\|on_relic_touched\|always`, `leash_radius:N` |
| `relic_core_slot` | TSY | `spawn_tsy_relic_slots` | `slot_count:N` | — |

> **npc_anchor archetype 扩展位（轴心 7 连带）**：当前 4 档 archetype（道伥 / 执念 / 守灵 / 畸变体）主要覆盖 mid/deep 层生态。浅层 PVP 收割场所需的"高阶守株待兔者" archetype 归 P4 `plan-tsy-hostile-v1` 定义（候选命名：`ancient_sentinel`），本 plan 的 schema 须留值域扩展位，不把 archetype 枚举写死。

**Q3**：kind 是否从 `String` 升级为 enum？见 §8。

### 1.2 Consumer system 骨架

```rust
pub fn spawn_rift_portals_from_pois(
    mut commands: Commands,
    providers: Res<TerrainProviders>,       // plan-tsy-dimension-v1 §2.2
    layers: Res<DimensionLayers>,
    zones: Res<ZoneRegistry>,
) {
    // 主世界侧：Entry portals（跨位面传 → TSY）
    for poi in providers.overworld.pois().iter().filter(|p| p.kind == "rift_portal") {
        let Some(direction) = parse_direction(&poi.tags) else { continue };
        if !matches!(direction, PortalDirection::Entry) { continue; }
        let family_id = extract_family_id(&poi.tags);
        let target_pos = parse_target_family_pos(&poi.tags)
            .unwrap_or_else(|| resolve_tsy_shallow_center(&zones, &family_id));
        commands.spawn((
            RiftPortal {
                family_id,
                target: DimensionAnchor { dimension: DimensionKind::Tsy, pos: target_pos },
                trigger_radius: 1.5,
                direction: PortalDirection::Entry,
            },
            Position(poi.pos_xyz.into()),
            // 挂到主世界 layer
            Layer(layers.overworld),
        ));
    }

    // TSY 侧：Exit portals（target 的 dimension 是 Overworld，pos 在运行时从 TsyPresence 取；
    // 此处只 spawn 占位 entity 作为触发区，实际 target 由 tsy_exit_portal_system 读 Presence）
    for poi in providers.tsy.pois().iter().filter(|p| p.kind == "rift_portal") {
        // 同构，direction=Exit，Layer(layers.tsy)
    }
}
```

同构拆出 4 个 system：rift_portals / containers / npc_anchors / relic_slots。container/npc_anchor/relic_slot 只消费 `providers.tsy.pois()` 并挂到 TSY layer。注册到 `Startup` post-init stage（在 `TerrainProviders::load` 之后）。

#### 1.2.a Portal 方块摆放（复用 MC 原版模型）

`spawn_rift_portals` 不只 spawn marker entity，还要把**原版 portal 方块组**摆到对应 layer（详见 `plan-tsy-dimension-v1 §3.3`）：

- `orientation=vertical` (Entry)：以 POI `pos_xyz` 为底部中心，沿 `facing` 轴摆 `obsidian` 4×5 框 + 内部 2×3 `nether_portal`；marker entity 放在 portal 方块中心，`trigger_radius=1.5`
- `orientation=horizontal` (Exit)：以 POI `pos_xyz` 为地面中心，摆 5×5 平面 —— 外圈 12 × `end_portal_frame`（带 eye，朝内），中心 3×3 `end_portal`；marker entity 悬浮在中心上方 0.5 格，`trigger_radius=1.5`

方块写入既可走 Valence 的 `ChunkLayer::set_block`（startup 时逐块写，zone mask 通过后），也可在 worldgen 侧预烤进 raster（profile fill_tile 时写入 `feature_mask` + 方块 palette，`plan-tsy-dimension-v1` 决策）。**骨架阶段倾向 startup 写**（简化 raster 格式），active 前复核性能（每 family 至少 1 Entry + 1 Exit = ~30 方块，量很小）。

**原版 portal travel 逻辑必须禁用**——Valence 若对 `nether_portal` / `end_portal` 方块保留原版 `on_entity_collision` 行为（4 秒传 nether / 瞬时传 end），我们的 `RiftPortal` 逻辑会被抢先触发错误的 dim。需 audit `valence_entity` / `valence_player` 的 portal 相关系统，拦截或覆盖（见 dimension plan §3.3 Q）。

### 1.3 幂等与 regen 策略

- server 启动只跑一次 consumer（POI 是启动态）
- dev-reload 重启 server 时 POI 坐标可能因 seed 变化而移动，**依赖 server 重启而非热加载**（对齐 dev-reload.sh 现状）
- 运行时不处理 despawn；塌缩事件的临时 portal spawn 属 P5 `plan-tsy-extract-v1` 职责

### 1.4 失败处理

- POI tags 缺失必需字段 → log warn + skip spawn（不 panic，允许 worldgen 迭代时部分 zone 不完整）
- 未知 kind value（如 `archetype:xxx` 的 xxx 不在枚举内）→ log warn + skip
- 整合到现有 `log_payload_build_error` 风格

---

## §2 Blueprint 扩展

### 2.1 文件布局（架构反转后已决策，~~Q5~~ 关闭）

Blueprint **必须分两文件**，分别产出两份 manifest：

1. **`server/zones.worldview.example.json`**（主世界 blueprint，现有）—— 只新增 `kind=rift_portal direction=entry` POI 条目，指向对应 TSY family 的 `_shallow` 中心
2. **`server/zones.tsy.json`**（新文件，TSY dim blueprint）—— 所有 TSY subzone + TSY 侧 POI

Blueprint loader 改造：`load_blueprint` 升级为"按位面加载"，产出两份 `GeneratedFieldSet` 和两份 manifest；或同一 loader 多次 invoke。raster_export 按位面写两个输出目录：`worldgen_out/overworld/` + `worldgen_out/tsy/`（对齐 `plan-tsy-dimension-v1 §2.1`）。

### 2.2 TSY zone 模板

**坐标系注意**：以下 AABB 和 POI `pos_xyz` 都是 **TSY dim 内部坐标**，以 family 原点（例如 (0, 0, 0)）起排；worldgen 可自由分配 family 在 TSY dim 内的排布（例如每 family 间距 500 格）。主世界侧的对应 rift_portal entry POI 在主世界 blueprint 里单独写。

```json
// TSY dim blueprint （zones.tsy.json）
{
  "name": "tsy_zongmen_01_shallow",
  "dimension": "bong:tsy",
  "display_name": "宗门遗迹·浅层（第一族）",
  "aabb": { "min": [0, 40, 0], "max": [100, 120, 100] },
  "spirit_qi": -0.4,
  "danger_level": 4,
  "active_events": ["tsy_entry"],
  "patrol_anchors": [[50, 80, 50]],
  "worldgen": {
    "terrain_profile": "tsy_zongmen_ruin",
    "shape": "rectangle",
    "boundary": { "mode": "hard" },
    "height_model": { "base": [60, 64], "peak": 72 },
    "surface_palette": "tsy_ruin_stone",
    "biome_mix": { "tsy_ruined": 1.0 },
    "landmarks": [],
    "depth_tier": "shallow",
    "origin": "zongmen_yiji"
  },
  "pois": [
    { "kind": "rift_portal",    "pos_xyz": [ 50, 100,  50], "tags": ["direction:exit", "kind:main", "family_id:zongmen_01", "orientation:horizontal"] },
    { "kind": "loot_container", "pos_xyz": [ 60,  80,  70], "tags": ["archetype:stone_casket", "locked:jade_seal"] },
    { "kind": "npc_anchor",     "pos_xyz": [ 70,  80,  80], "tags": ["archetype:zhinian", "trigger:on_enter", "leash_radius:8"] },
    { "kind": "relic_core_slot","pos_xyz": [ 50,  50,  50], "tags": ["slot_count:5"] }
  ]
}
```

```json
// 主世界 blueprint（zones.worldview.example.json 补丁）
// 放在对应 "北荒" 或 "宗门遗迹" 地表 zone 的 pois[] 里
{
  "kind": "rift_portal",
  "pos_xyz": [1810, 100, 2810],
  "tags": [
    "direction:entry",
    "kind:main",
    "family_id:zongmen_01",
    "target_family_pos_xyz:50,100,50",   // TSY dim 内 _shallow 中心
    "orientation:vertical",              // Nether 式竖门
    "facing:north"                       // 门正面朝向
  ]
}
```

> **Portal 方块形态（对齐 `plan-tsy-dimension-v1 §3.3`）**：
> - `orientation:vertical`（Entry）= `obsidian` 4×5 框 + 内部 2×3 `nether_portal`，Nether 竖门视觉，"地壳裂缝"
> - `orientation:horizontal`（Exit）= 外圈 12 × `end_portal_frame`（带 eye）+ 中心 3×3 `end_portal`，End 横门视觉，"阵盘回程阵"
> - POI `pos_xyz` 是 portal 方块组的中心；`§1.2.a` 定义 consumer 的摆放细节

> **mid / deep 模板（骨架阶段不列全文，active plan 补齐）**：同 shallow 同 family，按轴心 7 调整——
> - `name`：`tsy_zongmen_01_mid` / `tsy_zongmen_01_deep`
> - `spirit_qi`：`-0.7`（mid）/ `-1.0`（deep），严守轴心 7 区间
> - `aabb`：复用相同 XZ（轴心 4 Y 分层），仅改 Y 区间（例如 mid `y∈[0,40]`、deep `y∈[-40,0]`）
> - `pois`：按 `§3.3` loot/NPC 分布差异填——mid 以道伥 anchor + 储物袋容器为主；deep 提高 `relic_core_slot` 密度（1 family 的"骨架"上古遗物全部集中深层，对齐 `worldview.md §十六.四`）+ 畸变体 anchor；mid/deep 不放 exit portal（出关只在 shallow 中心）
> - `active_events`：mid/deep 按需追加（例如 deep 层可增 `tsy_collapse_proximity` 预警事件，留给 P2 lifecycle）

**新增字段**：
- `dimension` ∈ `{"minecraft:overworld", "bong:tsy"}`（由 `plan-tsy-dimension-v1 §6 Q2` 候选 A 引入，所有 zone 统一加）
- `worldgen.depth_tier` ∈ `{shallow, mid, deep}`（profile 内分支用）
- `worldgen.origin` ∈ `{daneng_luoluo, zongmen_yiji, zhanchang_chendian, gaoshou_sichu}`
- `worldgen.boundary.mode: "hard"` — TSY 边界硬切，stitcher 不做 blend（对齐 §0 轴心 4；stitcher 已支持，见 `stitcher.py:159-177` 的 `hard`/`semi_hard` 分支）；独立位面里边界外是 world border / 无地区（见 dimension plan Q3）

### 2.3 骨架阶段样本数量

2 起源 × 3 层 = 6 subzone（1 family 每起源）。active plan 阶段扩到 4 起源 × 2-3 family × 3 层 = 24-36 subzone。

---

## §3 Profile 新增（Python）

### 3.1 起源 → profile 映射

| 起源 | Profile class 文件 | 备注 |
|------|------|------|
| 大能陨落 | `profiles/tsy_daneng_crater.py`（新） | 陨石坑 + 灵气结晶柱 + 中心残骸 |
| 宗门遗迹 | `profiles/tsy_zongmen_ruin.py`（新） | 倒塌殿宇 + 阵盘残件 + 藏书废墟 |
| 战场沉淀 | `profiles/tsy_zhanchang.py`（新 or fork `ancient_battlefield`，Q6） | 密集骨堆 + 兵器林立 + 血色地脉 |
| 近代高手死处 | `profiles/tsy_gaoshou_hermitage.py`（新） | 单栋茅屋 + 坟冢 + 日常器物 |

### 3.2 Profile 接口（对齐现有）

```python
# profiles/tsy_zongmen_ruin.py
PROFILE_NAME = "tsy_zongmen_ruin"
EXTRA_LAYERS = (
    "qi_density",        # 复用，TSY 内 ≈ 0.9（高浓度残留）
    "mofa_decay",        # 复用，TSY 内 ≈ 0.15（末法程度低）
    "anomaly_intensity", # 复用，ruin 区 ≈ 0.4-0.7
    "anomaly_kind",      # 复用，TSY 特有 kind 值待定（见 Q 新增）
    "tsy_presence",      # 新 layer，mask=1
    "tsy_origin_id",     # 新 layer，= 2（zongmen）
    "tsy_depth_tier",    # 新 layer，= 1/2/3
)
ECOLOGY = EcologySpec(decorations=(...), ambient_effects=(...), notes="倒塌殿宇…")

def fill_tsy_zongmen_ruin_tile(zone, tile, tile_size, palette):
    depth = zone.worldgen.depth_tier  # shallow/mid/deep 分支
    for cell in tile:
        fill_height(cell, depth)
        fill_surface(cell, depth, palette)
        fill_tsy_layers(cell, origin_id=2, depth_tier=depth)
        # 宗门特征：柱础、碎墙、藏经架（shallow 多；deep 只剩阵盘核心）
```

### 3.3 Shallow/Mid/Deep 差异化（骨架示意，对齐轴心 7）

| 深度 | 主题 | 地貌特征 | spirit_qi | anomaly_intensity | loot 档次 | 主要 NPC |
|------|------|---------|-----------|-------------------|----------|--------|
| shallow | 入口带 / **PVP 死地** | 灰雾弥漫、低骨堆、少量容器 | -0.3 ~ -0.5 | 0.2-0.4 | 凡铁、磨损装备（历代搜尽） | **高阶守株待兔者（PVP 收割，P4 定义）** |
| mid | 主废墟 / 冲突带 | 密集遗骸、中型容器 | -0.6 ~ -0.8 | 0.4-0.7 | 残卷、轻型法器 | 道伥、中型守灵 |
| deep | 核心 / 低阶避难所 | 阵盘核心、法阵残件 | -0.9 ~ -1.2 | 0.7-1.0 | **上古遗物（集中）** | **畸变体（低阶可 20-30 分钟苟）** |

> **反直觉布局提醒（轴心 7）**：浅层的主要威胁是**高阶 PVP** 不是环境负压；深层对**低阶**反而最安全（真元池小，绝对抽吸量有限），对**高阶**数秒即秒。Profile fill 时 `npc_anchor` 密度、`loot_container` 档次、`relic_core_slot` 槽位都必须反映此语义——不是游戏平衡，是世界观 §十六.三 的物理推导。

---

## §4 Layer 扩展

### 4.1 新增 3 个 TSY layer（加到 `LAYER_REGISTRY` @ `fields.py:45-104`）

**作用域（架构反转后）**：这 3 个 layer **只出现在 TSY dim 的 raster 产出**里；主世界 raster 不写、不覆盖、根本没有这些 layer。原 Q7 "全世界覆盖 vs 稀疏存储" 自然关闭。

```python
LayerSpec("tsy_presence",   safe_default=0,   blend_mode="maximum", export_type="uint8"),
LayerSpec("tsy_origin_id",  safe_default=0,   blend_mode="swap",    export_type="uint8"),
LayerSpec("tsy_depth_tier", safe_default=0,   blend_mode="swap",    export_type="uint8"),
```

编码：
- `tsy_presence`: 0=TSY dim 内非 family 区（void / world border），1=TSY family 区域内（fast mask，Rust 查询 hot path）
- `tsy_origin_id`: 0=none, 1=daneng, 2=zongmen, 3=zhanchang, 4=gaoshou
- `tsy_depth_tier`: 0=none, 1=shallow, 2=mid, 3=deep

**实现提示**：raster_export 按位面分两次调用时，传一个"哪些 layer 属于本位面"的 whitelist。TSY 产出带这 3 个 layer，主世界产出不带。

### 4.2 复用现有 layer

- `qi_density` (lerp) — TSY 内 ≈ 0.9（高浓度，对比末法主世界 0.05-0.15）；主世界仍保持现有值域
- `mofa_decay` (lerp) — TSY 内 ≈ 0.15（末法程度低）
- `anomaly_intensity` (maximum) + `anomaly_kind` (swap) — TSY 异常编码：塌缩中 / 扭曲 / 幻象 / 时间停滞（值域扩展需 coordinate with 现有 anomaly 定义）

### 4.3 raster_check invariant 新增

针对 TSY manifest：
- 每 TSY family（通过 zone name 前缀 `tsy_<origin>_<N>_*`）至少 1 个 `kind=rift_portal direction=exit` POI
- 每 TSY family 三层齐全（shallow + mid + deep，按命名后缀识别）
- `tsy_presence > 0` 的 cell 必须 `qi_density >= 0.7`
- 三层的 AABB Y 区间不 overlap（选项 A 要求）
- `tsy_origin_id` ∈ {0..4}、`tsy_depth_tier` ∈ {0..3}

针对主世界 manifest：
- 每个 `kind=rift_portal direction=entry` POI 必须带 `family_id` + `target_family_pos_xyz` tags，且 `family_id` 在 TSY manifest 里有对应 subzone 组
- 主世界 manifest 不出现 `tsy_presence` / `tsy_origin_id` / `tsy_depth_tier` layer

---

## §5 Q2 详解：三层深度模型

（架构反转后，两个选项都在**独立位面**内实现，不必再与主世界地质冲突——但 Y 分层依然是首选）

### 选项 A：Y 分层（对齐 P0 §1.1 现状，首选）

- XZ 共享（三层 AABB 同 XZ 范围），Y 轴垂直分层
- 示例：shallow Y∈[40,120], mid Y∈[0,40], deep Y∈[-40,0]
- `ZoneRegistry.find_zone(DimensionKind::Tsy, player_pos)` 按 Y 自然切换
- **优点**：复用现有 `underground_tier` + `cavern_floor_y` layer；P0 §1.1 已约定；玩家"往下探 = 深入负压"的心智匹配世界观
- **缺点**：profile fill_tile 按 Y 区间分支，单 profile 处理全深度；Y 剧变处过渡需手工 smooth

### 选项 B：独立 AABB

- 三层 XZ 可不同（例如 shallow 在 TSY dim 的某 XZ 区、mid 在另一区，彼此通过层间 portal 联通）
- 每层独立 profile 或同 profile 不同参数
- **优点**：每 profile fill 逻辑单一；三层气氛可以差异更大
- **缺点**：玩家"向下走"需 portal 切换（层间 rift_portal kind:deep），多一层 UX；不符合 §十六 "浅/中/深 = 物理深度" 直觉

**倾向 A**（active plan 阶段需验 Y 剧变处过渡可控）。

---

## §6 dev 迭代 + 校验

### 6.1 现有流水线（对齐 `scripts/dev-reload.sh`）

```
regen (python -m scripts.terrain_gen)
  → raster_check (validate manifest + all tiles)
  → cargo build
  → kill old server, restart with new manifest path
```

TSY worldgen 产出是 **startup-time 数据**，server 重启即消费。无需热加载机制。

### 6.2 新 smoke 脚本

`scripts/smoke-tsy-worldgen.sh`（新）：
1. regen + validate
2. 启 server
3. 走到第一个 TSY family 的 rift_portal 坐标，验证 RiftPortal component 已 spawn
4. 进 zone，验证 LootContainer / NpcAnchor 已 spawn（数量匹配 blueprint）
5. 走到 deep 层，验证 `tsy_depth_tier=3`
6. 停 server，退出码 0/1

### 6.3 raster_check 新 invariant

见 §4.3。总计 5 条新增。

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

本 plan 仍不反向改 P0 的**业务逻辑**——P0 实装的 `TsyPresence`/`RiftPortal`/入场过滤/负压 tick 概念全部保留，本 plan 只负责**从两份 POI 表自动 spawn 出来**，替代 `/tsy-spawn` 的手工流程。

---

## §8 开放问题清单（骨架阶段不答，进 active plan 前收敛）

- **Q1** 坍缩渊世代更替（**被轴心 8 部分收敛**）：原 family 塌缩后永久清零已定（`worldview.md §十六.一`），**新 family 怎么上线**尚开放——选项 A：预生成候选 family 池（blueprint 含 N 个 inactive family，按节奏激活）；选项 B：运行时动态创建 zone（需 `ZoneRegistry::register_runtime_zone()` + manifest 动态 append + TSY dim raster 增量写入）。涉及 P2 `plan-tsy-lifecycle-v1` 的塌缩事件消费。独立位面下选项 A 更顺（预先 bake 好整个 TSY dim raster，活跃 family 通过 zone 是否 registered 控制）
- **Q2** 三层深度模型（选项 A Y 分层 vs B 独立 AABB）— 倾向 A（§5），但 active plan 前需验 Y 剧变处过渡可控
- **Q3** `Poi.kind` 是否升级为 enum？现状 `String`，server 端 `filter(|p| p.kind == "rift_portal")` 字符串比较。升级优点：编译期检查 + dispatch 更快；缺点：`TerrainProvider` 签名变更，影响面需 audit
- **Q4** 塌缩地貌扭曲（P2 lifecycle 触发）：预烤多版 voxel（`tsy_zongmen_01_collapsed` blueprint alt）vs 运行时 block damage（Rust 侧 ChunkMutator）？预烤简单但不可逆，运行时复杂但支持反复塌缩
- ~~Q5 Blueprint 布局~~（**已关闭**：架构反转后"分两文件"成硬决策，`zones.worldview.example.json` + `zones.tsy.json` + blueprint loader 按位面分次加载）
- **Q6** 战场沉淀起源是否 fork `ancient_battlefield` profile？如 fork，主世界古战场 vs TSY 战场沉淀的地貌语义需分离（前者 qi_density 极低，后者极高）。独立位面后两者的 profile 可以完全分离，冲突风险低
- ~~Q7 tsy_presence 全世界覆盖~~（**已关闭**：TSY layer 只出现在 TSY manifest，主世界不写）
- **Q8** 多人服 instance 化：全 server 共享同一 TSY dim vs per-party instance TSY dim？骨架阶段**倾向共享**（符合 §十六 supply chain 设计），Q1 repop 的语义也基于共享。由 `plan-tsy-dimension-v1 §7 Q4` 承接最终决策
- **Q9** 单 family POI 数量级：骨架阶段约定 2-5 rift_portal（1 主世界 entry + 1 TSY exit + 可选 1-3 层间 portal） + 10-30 loot_container + 5-15 npc_anchor + 1 relic_core_slot；active plan 前需 playtest 校准
- ~~Q10 `boundary.mode: "hard"` 需 stitcher 支持硬切~~（**已关闭**：`stitcher.py:159-177` 已实现 `hard`/`semi_hard`/soft 三档 boundary mode，TSY zone 直接 `"boundary": {"mode": "hard"}` 即可，无需 stitcher 改动）
- **Q11**（新增）主世界裂缝锚点的数量 / 分布：每 TSY family 固定 1 个 entry 锚点 vs 多个主世界锚点都通向同一 TSY family？后者更符合"秘境入口散落各地"叙事，但需要决定"从不同 entry 进同一 family 着陆到同一 shallow 中心还是不同点"。骨架阶段约定 1:1，active 前复核

---

## §9 实施规模预估（骨架，active plan 开工时修正）

| 模块 | 新增行数 |
|------|------|
| Python profile (`profiles/tsy_*.py` × 4) | ~400 |
| `fields.py` LAYER_REGISTRY 扩展 | ~30 |
| `raster_check.py` 新 invariant（跨 manifest 校验） | ~90 |
| Blueprint loader 按位面分次加载 + raster_export 按位面产出两份 manifest | ~120 |
| Blueprint sample `zones.tsy.json`（2 family × 3 层 × 1 origin）+ 主世界裂缝锚点补丁 | ~240 |
| Rust `server/src/world/tsy_poi_consumer.rs`（按位面拆两组 consumer + Layer 绑定） | ~380 |
| `server/src/world/terrain/raster.rs` POI kind 扩展 + 按 provider routing | ~80 |
| Rust tests (integration + unit，含跨位面 POI 一致性测试) | ~260 |
| Smoke `scripts/smoke-tsy-worldgen.sh`（含跨位面走流程） | ~60 |
| `scripts/dev-reload.sh` 双 manifest 支持（**可能归 dimension plan**） | ~20 |
| **合计** | **~1680** |

架构反转后规模较原 ~1280 增加约 400 行，主要是 blueprint loader / raster_export 双位面改造 + consumer 跨位面拆分 + 跨 manifest 一致性校验。仍在一次 worktree 可吃完范围。

---

## §10 升级条件（骨架 → active）

本 plan 从 `docs/plans-skeleton/` 移到 `docs/` 的触发：

1. **`plan-tsy-dimension-v1` active 且 merged**（新增前置）— 必须先有 `DimensionKind` enum / `TerrainProviders` 多 provider / `DimensionTransferRequest` event / `Zone.dimension` 字段 + registry gating
2. P0 `plan-tsy-zone-v1` active 且 merged，且 `ZoneRegistry::register_runtime_zone()` 能力落地
3. P3/P4/P5 plan（container/hostile/extract）至少 1 个开工，需要真实 POI 数据驱动
4. Q1/Q4/Q8/Q11 收敛（至少给出 active 阶段决策方向）
5. **轴心 7/8 连带修订完成**（active 开工前的前置校对清单）：
   - `§3.3` 差异化表的 `spirit_qi` 与 loot/NPC 列已按 worldview §十六.三 实填（骨架已初稿，active 前按 P4 最终 archetype 命名回填）
   - `§2.2` blueprint 补 mid/deep 完整 JSON 模板（骨架只给文字摘要）
   - `§1.1` `npc_anchor.archetype` 值域与 P4 `plan-tsy-hostile-v1` 对齐后实填，不再停留在"扩展位预留"
   - `§8 Q1` 的"新 family 怎么上线"（轴心 8 剩余开放部分）给出选项收敛

---

**下一步**：等 dimension plan merged + P0 merged + P3 或 P4 开工后，回答 Q1/Q2/Q4/Q8/Q11，骨架升级为 active plan（移出 `plans-skeleton/`），`/consume-plan tsy-worldgen` 启动。

---

## §11 进度日志

- **2026-04-25**：骨架现状校核——无任何 TSY 实装。`worldgen/scripts/terrain_gen/profiles/` 仅 9 个现有 profile（abyssal_maze / ancient_battlefield / broken_peaks / cave_network / rift_valley / sky_isle / spawn_plain / spring_marsh / waste_plateau），未见 `tsy_*.py`；`fields.py:LAYER_REGISTRY` 未注册 `tsy_presence` / `tsy_origin_id` / `tsy_depth_tier`；`server/zones.tsy.json` 不存在，`zones.worldview.example.json` 未含 `kind=rift_portal` POI；`server/src/world/` 无 `tsy_poi_consumer.rs`。POI 通道前置确认已通：`blueprint.py:156-184` 已序列化 `pois[]`，待 dimension plan + P0 落 Rust 后开工。本 plan 仍处骨架阶段，未触发 §10 升级条件（dimension plan 文档已 active 但 Rust 侧 `DimensionKind` / `TerrainProviders` 未实装）。
- **2026-04-26**：**dimension plan Rust 侧解冻** — PR #47（merge 579fc67e）落地 `DimensionKind` / `DimensionLayers` / `TerrainProviders { overworld, tsy: Option }` / `DimensionTransferRequest` / `Zone.dimension` 全套基础设施。本 plan §10 升级条件之一（dimension Rust 侧落地）已满足，仍欠 **P0 `tsy-zone` merged**（升级条件之二）；可与 P0 active 阶段并行起步骨架→active 升级。POI 通道（`blueprint.py:156-184`）已就位等 consumer 接入。
