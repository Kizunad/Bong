# TSY Zone 基础框架 · plan-tsy-zone-v1

> 把"活坍缩渊"作为一种特殊 zone 落地到现有 Zone 框架；负压抽真元作为持续压力源；裂缝 POI 双向传送；入场过滤器剥离高灵质物品。P0 基础设施，不涉及 loot 与塌缩。
> 交叉引用：`plan-tsy-v1.md §2`（横切）· `plan-tsy-v1.md §0`（公理）· `worldview.md §十六.一/§十六.二/§十六.四`

---

## §-1 现状（已实装，不重做）

| 层 | 能力 | 位置 |
|----|------|------|
| Zone struct | `name/bounds/spirit_qi/danger_level/patrol_anchors/blocked_tiles/active_events` | `server/src/world/zone.rs:23-31` |
| Zone 查询 | `contains(pos)` / `clamp_position` / `center` / `patrol_target` | `server/src/world/zone.rs:69-105` |
| ZoneRegistry | `find_zone(pos)` / `find_zone_by_name(name)` / `load_from_path` | `server/src/world/zone.rs:109-243` |
| Zone 加载 | 从 `server/zones.json` 启动时加载 | `server/src/world/zone.rs:133` |
| PlayerState | `realm / spirit_qi / spirit_qi_max / karma` | `server/src/player/state.rs:27-34` |
| Item.spirit_quality | [0.0, 1.0] 灵质字段 | `server/src/inventory/mod.rs:135-150` |
| 真元条 HUD | client 侧已渲染 | `client/src/main/java/...`（plan-combat-ui_impl 完成） |

**本 plan 要新增**：TSY zone 类型识别、负压 tick、裂缝 POI、双向传送、入场过滤器、浅/中/深三层灵压采样、`/tsy-spawn` 调试命令。

**本 plan 不处理的地形 / POI 生成**：TSY zone 的内部地貌（4 起源视觉差异、碎骨堆/阵盘残件/骨架/容器/守灵 anchor 自动分布）由独立 plan `docs/plans-skeleton/plan-tsy-worldgen-v1.md` 承载。本 plan 的 `/tsy-spawn` 调试命令 + `zones.json` 手写 3 subzone 是**骨架级兜底**，worldgen plan 落地后替换为 blueprint → manifest.json 驱动（POI 通道已通，见 `worldgen plan §1`）。

**隐形前置依赖**：本 plan §3.1 假设 `ZoneRegistry` 支持运行时动态 add（`/tsy-spawn` 追加 3 subzone）。现有 `ZoneRegistry::apply_runtime_records()` (`server/src/world/zone.rs:195`) 只支持修改已注册 zone 的 `active_events`/`blocked_tiles`，**不支持 add/remove zone**。本 plan 需补 `ZoneRegistry::register_runtime_zone(zone: Zone) -> Result<()>`（幂等、同名 zone 已存在则拒绝、push 到内部 Vec 末尾）。此能力属于 P0 范围，不外推给 worldgen plan。

**架构前置依赖（2026-04-24 反转）**：本 plan 原 §-1 点 5 / §0 轴心 5 约定"传送是同一 MC world 内的坐标传送"，**已推翻**——`worldview.md §十六 世界层实现注` 明确坍缩渊以**独立位面**实现（类 Nether）。相关基础设施由 `docs/plans-skeleton/plan-tsy-dimension-v1.md` 承载：Valence `DimensionType` 注册、TSY `LayerBundle`、跨位面传送 API (`DimensionTransferRequest`)、per-dimension `TerrainProvider`。本 P0 plan 的裂缝/入场/出关系统全部改为消费 dimension plan 提供的跨位面 API，而非自己直 `insert Position`。

---

## §0 设计轴心

1. **Zone 以 name 前缀识别**：`zone.name.starts_with("tsy_")` 即为活坍缩渊。不改 Zone 结构的 shape，只约定命名，降低扩展摩擦（见 `plan-tsy-v1.md §2.3`）
2. **负压机制只读 Zone 现有字段**：`Zone.spirit_qi ∈ [-1.2, -0.3]` 即为 TSY 内部灵压；不新增 `draining_rate` 字段（从 spirit_qi 推导）
3. **内部层深用多个 subzone 表达**：一个"活坍缩渊"在 zones.json 里是**三个相邻 zone**（`tsy_xxx_shallow` / `_mid` / `_deep`），通过 name 后缀联动。好处是复用 Zone 现有几何判定 + 避免"一个 zone 多个灵压"的特殊逻辑
4. **入口 POI 走现有 `active_events` 字段**：用 `"portal_rift"` tag 标记该 zone 靠近边缘的传送点；TSY 的入口 zone 同时拥有 `"tsy_entry"` tag
5. **传送是跨位面（Nether 式 dimension 切换）** — TSY 位面是独立 Valence `LayerBundle`，所有 TSY zone 的 AABB 是 **TSY dim 内部坐标**（不是主世界坐标）；裂缝 POI 锚点存在主世界 layer，触发后走 `DimensionTransferRequest` 切到 TSY layer。基础设施由 `plan-tsy-dimension-v1` 承载，本 plan 只消费接口
6. **入场过滤是入口传送的 on-arrival hook** — 传送完成后扫描玩家 inventory 所有 item，`spirit_quality >= 0.3` 的 item 在入口被"剥离"（set to 0 + spawn 一个 bone/灰烬 item 替代），离场不再恢复

---

## §1 数据模型

### 1.1 Zone 配置扩展（无 struct 改动，仅约定）

**坐标系注意**：TSY 系列 zone 的 AABB 全部是 **TSY dim 内部坐标**（由 `plan-tsy-dimension-v1` 注册的独立 Valence layer），不占主世界坐标。示例中 XZ 以 (0,0) 为 family 原点起排，由 worldgen blueprint 统一分配。

TSY 系列 zone 在 `server/zones.tsy.json`（独立文件，`plan-tsy-worldgen-v1 §2.1` 决策）里的模板：

```json
{
  "name": "tsy_lingxu_01_shallow",
  "dimension": "bong:tsy",
  "aabb": { "min": [0, 40, 0], "max": [100, 120, 100] },
  "spirit_qi": -0.4,
  "danger_level": 4,
  "active_events": ["tsy_entry", "portal_rift"],
  "patrol_anchors": [[50, 80, 50]],
  "blocked_tiles": []
},
{
  "name": "tsy_lingxu_01_mid",
  "dimension": "bong:tsy",
  "aabb": { "min": [0, 0, 0], "max": [100, 40, 100] },
  "spirit_qi": -0.7,
  "danger_level": 5,
  "active_events": [],
  "patrol_anchors": [[50, 20, 50]],
  "blocked_tiles": []
},
{
  "name": "tsy_lingxu_01_deep",
  "dimension": "bong:tsy",
  "aabb": { "min": [0, -40, 0], "max": [100, 0, 100] },
  "spirit_qi": -1.1,
  "danger_level": 5,
  "active_events": [],
  "patrol_anchors": [[50, -20, 50]],
  "blocked_tiles": []
}
```

**约定**：
- 新增 `dimension` 字段（`plan-tsy-dimension-v1 §6` Q2 候选 A：单 registry + Zone.dimension gating）；主世界 zone 填 `"minecraft:overworld"`，TSY zone 填 `"bong:tsy"`
- 三个 subzone **共享 XZ bounds**，Y 轴垂直分层（浅层顶上、深层底下）
- 玩家在 TSY 内走动时通过 Y 坐标自然跨层，`ZoneRegistry.find_zone(dim, pos)` 按当前位面 + 坐标返回对应层
- **命名前缀** `tsy_<来源>_<序号>_<层深>`；`<层深>` ∈ `{shallow, mid, deep}`
- 裂缝入口 POI（`portal_rift` tag）**登记在主世界 zone**，不在 TSY dim 内部；本 subzone 的 `"tsy_entry"` tag 现在表示"该 TSY subzone 是跨位面后的着陆层"，语义比原版略窄

### 1.2 识别 helper（新增）

**位置**：`server/src/world/zone.rs` 末尾添加

```rust
impl Zone {
    /// TSY 系列 zone 的识别
    pub fn is_tsy(&self) -> bool {
        self.name.starts_with("tsy_")
    }

    /// TSY 层深（None = 不是 TSY）
    pub fn tsy_layer(&self) -> Option<TsyLayer> {
        if !self.is_tsy() { return None; }
        if self.name.ends_with("_shallow") { Some(TsyLayer::Shallow) }
        else if self.name.ends_with("_mid") { Some(TsyLayer::Mid) }
        else if self.name.ends_with("_deep") { Some(TsyLayer::Deep) }
        else { None }
    }

    /// TSY 系列 id（"tsy_lingxu_01_shallow" → "tsy_lingxu_01"）
    pub fn tsy_family_id(&self) -> Option<String> {
        if !self.is_tsy() { return None; }
        self.name.rsplit_once('_').map(|(head, _)| head.to_string())
    }

    /// 是否为入口层（有 tsy_entry tag）
    pub fn is_tsy_entry(&self) -> bool {
        self.active_events.iter().any(|e| e == "tsy_entry")
    }
}

/// 坍缩渊层深
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TsyLayer { Shallow, Mid, Deep }
```

### 1.3 新增 Component：`TsyPresence`

**位置**：新建 `server/src/world/tsy.rs`，在 `server/src/world/mod.rs` 暴露

```rust
/// 玩家在 TSY 内的状态（Entity-scoped）
#[derive(Component, Debug)]
pub struct TsyPresence {
    /// 玩家所在的 TSY 系列 id（如 "tsy_lingxu_01"）
    pub family_id: String,
    /// 进入 tick
    pub entered_at_tick: u64,
    /// 入场时的 inventory 快照 instance_ids —— 用于秘境死亡结算区分"秘境所得" vs "原带物"
    pub entry_inventory_snapshot: Vec<u64>,
    /// 出关锚点：回到哪个位面 + 哪个坐标（架构反转后必带位面信息）
    /// - 通常 = `(DimensionKind::Overworld, 触发裂缝的主世界坐标 + (0,1,0))`
    /// - 塌缩时若主世界锚点已失效（如对应 RiftPortal 被 despawn），由 P2 lifecycle 决定 fallback（出生点 / 灵龛 / 随机安全点）
    pub return_to: DimensionAnchor,
}

/// 位面锚点（`plan-tsy-dimension-v1 §3` 定义）
pub struct DimensionAnchor {
    pub dimension: DimensionKind,  // 来自 plan-tsy-dimension-v1
    pub pos: DVec3,
}
```

**生命周期**：
- 玩家进裂缝传送 → `TsyPresence` attach 到 Entity
- 玩家出关传送 → `TsyPresence` detach（+ 清理）
- 玩家在 TSY 内死亡 → detach（死亡结算由 `plan-tsy-loot-v1.md` 处理）

### 1.4 IPC schema 新增

**位置**：`agent/packages/schema/src/tsy.ts`（新建）

```typescript
import { Type, Static } from '@sinclair/typebox';

/** 玩家进入 TSY */
export const TsyEnterEventV1 = Type.Object({
  v: Type.Literal(1),
  kind: Type.Literal('tsy_enter'),
  tick: Type.Number(),
  player_id: Type.String(),
  family_id: Type.String(),
  // 出关锚点（架构反转后带位面信息）：回到哪个 dim + 哪个坐标
  return_to: Type.Object({
    dimension: Type.String(),      // e.g. "minecraft:overworld"
    pos: Type.Array(Type.Number(), { minItems: 3, maxItems: 3 }),
  }),
  filtered_items: Type.Array(Type.Object({
    instance_id: Type.Number(),
    template_id: Type.String(),
    reason: Type.Literal('spirit_quality_too_high'),
  })),
});

/** 玩家出关 */
export const TsyExitEventV1 = Type.Object({
  v: Type.Literal(1),
  kind: Type.Literal('tsy_exit'),
  tick: Type.Number(),
  player_id: Type.String(),
  family_id: Type.String(),
  duration_ticks: Type.Number(),
  qi_drained_total: Type.Number(),
});

export type TsyEnterEvent = Static<typeof TsyEnterEventV1>;
export type TsyExitEvent = Static<typeof TsyExitEventV1>;
```

在 `agent/packages/schema/src/index.ts` 导出；generate.ts 自动产生 JSON schema 和 Rust serde 结构。

---

## §2 负压公式与 tick 系统

### 2.1 抽取速率公式

**公理**：抽取率非线性，与真元池大小平方关系（见 `plan-tsy-v1.md §0` 公理 2）。

```rust
/// 每 tick 抽取真元量（单位：点）
/// rate = |spirit_qi| × (池 / base)^n × base_rate
/// 其中 spirit_qi ∈ [-1.2, -0.3]（深层到浅层）
/// 池 = player.spirit_qi_max（不是 current！）
/// n = NONLINEAR_EXPONENT（调参，默认 1.5）
/// base = REFERENCE_POOL（默认 100，即引气满池基准）
/// base_rate = BASE_DRAIN_PER_TICK（调参，默认 0.5 点/tick）
pub fn compute_drain_per_tick(zone: &Zone, player: &PlayerState) -> f64 {
    if !zone.is_tsy() { return 0.0; }
    let pool_ratio = player.spirit_qi_max / REFERENCE_POOL;
    let nonlinear = pool_ratio.powf(NONLINEAR_EXPONENT);
    zone.spirit_qi.abs() * nonlinear * BASE_DRAIN_PER_TICK
}

const REFERENCE_POOL: f64 = 100.0;
const NONLINEAR_EXPONENT: f64 = 1.5;
const BASE_DRAIN_PER_TICK: f64 = 0.5;  // server tick = 20Hz，0.5/tick = 10/sec
```

**参考数值**（20Hz tick，默认参数）：

| 境界 | spirit_qi_max | 灵压 | 抽速（秒） | 理论撑时 |
|------|---------------|------|-----------|---------|
| 引气 | 30 | -0.3（浅） | 0.78 点/秒 | 38 秒不回，**若静坐可回**则长时间 |
| 引气 | 30 | -1.1（深） | 2.85 点/秒 | 10.5 秒满池榨空 |
| 化虚 | 500 | -0.3（浅） | **41 点/秒** | 12 秒 |
| 化虚 | 500 | -1.1（深） | 150 点/秒 | 3.3 秒 |

**注**：上面"理论撑时"假设玩家完全不回真元。worldview §二 说"真元回复**只能靠灵气浓度高的地方**"——TSY 内灵气是负数，**不回**，唯一回复方式是消耗丹药。

实际表现：引气苟深层 "20-30 分钟" 的体感（`worldview §十六.二 表格`）= 玩家慢速移动 + 偶尔嗑药 + 战斗闪避，综合下来在引气小池基础上能撑到 20-30 分钟。化虚深层 "数秒即被秒" 精确对齐 3.3 秒测算。

**参数调节**：`server/src/world/tsy.rs` 顶部用 `pub const`，方便 playtest 调。

### 2.2 Drain tick system

**位置**：新建 `server/src/world/tsy_drain.rs`，在 `server/src/world/mod.rs` 注册到 `FixedUpdate`

```rust
/// 系统：每 tick 扫描有 TsyPresence 的玩家，按当前 zone 抽真元
///
/// 架构反转后：`find_zone` 需要位面参数。system 已被 `TsyPresence` gate 过滤，
/// 理论上玩家都在 TSY dim，直接传 `DimensionKind::Tsy` 即可；若出现
/// `TsyPresence` 存在但 `CurrentDimension != Tsy` 的 inconsistent state，
/// `find_zone(Tsy, pos)` 会返回 None 自然 skip（暴露 bug 而非静默错传）。
pub fn tsy_drain_tick(
    mut players: Query<(Entity, &mut PlayerState, &Position, &TsyPresence)>,
    zones: Res<ZoneRegistry>,
    tick: Res<ServerTick>,
    mut death_events: EventWriter<DeathEvent>,
) {
    for (entity, mut state, pos, _presence) in &mut players {
        let Some(zone) = zones.find_zone(DimensionKind::Tsy, pos.0) else { continue };
        if !zone.is_tsy() { continue; }

        let drain = compute_drain_per_tick(zone, &state);
        state.spirit_qi -= drain;

        if state.spirit_qi <= 0.0 {
            // 真元归零 → 血肉开始被抽（下个 plan-tsy-loot 处理干尸化）
            // 先发 DeathEvent，cause="tsy_drain"
            death_events.send(DeathEvent {
                target: entity,
                cause: "tsy_drain".to_string(),
                attacker: None,
                attacker_player_id: None,  // attacker 字段由 plan-tsy-loot 加
                at_tick: tick.0,
            });
        }
    }
}
```

**注**：`DeathEvent.attacker` / `attacker_player_id` 由 P1 `plan-tsy-loot-v1` 正式加，P0 先用占位 `None` 不会破坏现有代码（若那里没这些字段，P0 的版本就不带这两字段）。

### 2.3 Tick 依赖序

```
FixedUpdate:
  combat_resolve          (写 Wounds → DeathEvent on bleed_out)
  ↓ after
  tsy_drain_tick          (写 PlayerState.spirit_qi → DeathEvent on qi=0)
  ↓ after
  lifecycle_death_process (读 DeathEvent → 执行死亡)
```

放在 combat 之后、lifecycle 之前，保证同一 tick 内两种致死原因都能被同一个 lifecycle 统一处理。

---

## §3 裂缝 POI 与跨位面传送

### 3.1 裂缝入口

裂缝 = **主世界** layer 内某个坐标的 `RiftPortal` 实体，靠近时触发**跨位面传送**到 TSY dim 内对应 family 的 `_shallow` 中心。

**MVP 实现**：调试命令 `/tsy-spawn <family_id>` 手动在当前位置（主世界）放置一个裂缝 + 对应的 TSY 三个 subzone（动态追加到 ZoneRegistry TSY dim 分组，依赖新增的 `ZoneRegistry::register_runtime_zone()`，见 §-1 隐形前置依赖）。正式发布走 `plan-tsy-worldgen-v1`：Python 侧 blueprint 分两文件产出——主世界 manifest 含 rift_portal POI（TSY 入口锚点）+ TSY dim manifest 含三层 subzone / loot / npc_anchor；`dev-reload.sh` regen 后 server 启动时两份 manifest 分别 mmap 进对应 `TerrainProvider`（`plan-tsy-dimension-v1 §2`），`/tsy-spawn` 调试命令退化为"强制激活已注册 TSY zone + 跨位面传玩家"。

### 3.2 Rift POI Component

**位置**：`server/src/world/tsy.rs`

```rust
/// 裂缝 POI：两种实体共用同一 component 定义
/// - **主世界侧**：附着在主世界 layer 某坐标，玩家触发后跨位面传 → TSY
/// - **TSY dim 侧**：同 family 的 `_shallow` 中心也放一个，玩家触发后跨位面传回主世界
/// 两侧实例通过 `direction` 区分
#[derive(Component, Debug, Clone)]
pub struct RiftPortal {
    /// 对应 TSY family id（如 "tsy_lingxu_01"）
    pub family_id: String,
    /// 跨位面传送目标：目标 dim + 目标坐标
    pub target: DimensionAnchor,
    /// 激活半径（玩家靠近时触发传送）
    pub trigger_radius: f64,  // MVP = 1.5 格
    /// 方向：Entry（主世界 → TSY）或 Exit（TSY → 主世界）
    pub direction: PortalDirection,
}

pub enum PortalDirection { Entry, Exit }
```

**视觉形态（复用 MC 原版 portal 模型，零资源成本）**：详见 `plan-tsy-dimension-v1 §3.3`。

- **Entry**（主世界裂缝）= **竖式 Nether 门**：`obsidian` 4×5 框 + 内部 `nether_portal` 方块；"地壳上一道凝结负灵气的竖直裂缝"
- **Exit**（TSY `_shallow` 中心回程阵）= **横式 End 门**：12 × `end_portal_frame`（带 eye）围一圈 + 中心 3×3 `end_portal`；"阵盘残件托起的回程阵，踏上去负压反吐"

**实现**：portal 方块由 worldgen blueprint / `/tsy-spawn` 直接摆放到对应 layer；中心位置 spawn 一个**不可见 marker entity**（armor stand 等），挂 `Position` + `RiftPortal`。玩家靠近 → AABB 命中 marker → 走本 plan §3.3 的 `DimensionTransferRequest` 路径。portal 方块本身只是皮肤，**不复用原版的 portal travel 逻辑**（原版 Nether 4s 延迟、End 目标 dim 写死 End，都不适用于跨位面到 TSY）。需要验证 Valence 是否对这些 vanilla 方块有 auto-travel 行为（见 dimension plan §3.3 Q）。

### 3.3 Entry 传送 System

**位置**：`server/src/world/tsy_portal.rs`，注册到 `FixedUpdate`

**关键变化（架构反转后）**：不再自己 `insert Position`，改为发 `DimensionTransferRequest` event 让 `plan-tsy-dimension-v1 §3` 的 `apply_dimension_transfers` 系统统一处理 layer 切换 + Position 更新 + Respawn packet。

```rust
pub fn tsy_entry_portal_system(
    mut commands: Commands,
    players: Query<(Entity, &Position, &PlayerState, &PlayerInventory, &CurrentDimension), Without<TsyPresence>>,
    portals: Query<(&Position, &RiftPortal)>,
    tick: Res<ServerTick>,
    mut dim_transfer: EventWriter<DimensionTransferRequest>,
    mut emit: EventWriter<TsyEnterEmit>,
) {
    for (player_entity, player_pos, state, inv, cur_dim) in &players {
        // 玩家必须在主世界才能触发 Entry portal
        if cur_dim.0 != DimensionKind::Overworld { continue; }

        for (portal_pos, portal) in &portals {
            if !matches!(portal.direction, PortalDirection::Entry) { continue; }
            if player_pos.0.distance(portal_pos.0) <= portal.trigger_radius {
                // Step 1: 入场过滤（见 §4）
                let filtered = apply_entry_filter(inv);

                // Step 2: attach TsyPresence（出关锚点 = 触发点 + 抬 1 格防卡）
                commands.entity(player_entity).insert(TsyPresence {
                    family_id: portal.family_id.clone(),
                    entered_at_tick: tick.0,
                    entry_inventory_snapshot: inv.all_instance_ids(),
                    return_to: DimensionAnchor {
                        dimension: DimensionKind::Overworld,
                        pos: portal_pos.0 + DVec3::Y,
                    },
                });

                // Step 3: 发跨位面传送请求（layer 切换 + Position 更新 + Respawn packet 统一处理）
                dim_transfer.send(DimensionTransferRequest {
                    entity: player_entity,
                    target: portal.target.dimension,   // = DimensionKind::Tsy
                    target_pos: portal.target.pos,     // = TSY dim 内 family 的 _shallow center
                });

                // Step 4: emit event
                emit.send(TsyEnterEmit { player_entity, family_id: portal.family_id.clone(), filtered });

                break;  // 一个玩家一 tick 只能进一个 portal
            }
        }
    }
}
```

### 3.4 Exit 传送

**设计决策（架构反转后）**：出关 = 玩家**走回 `_shallow` 中心的 Exit portal 实体**（与主世界入口 RiftPortal 双向对应）。不再做"走出 AABB 自动出关"——在独立 TSY 位面里走出 family AABB 要么撞 world border、要么落入无地的死负压区（见 dimension plan Q3），都不是可预期的 UX。

**原因**：
- 独立位面没有"走出去 = 出关"的几何基础
- 改成 Exit portal 实体更符合 MC 原版跨位面心智（nether portal 也是实体触发）
- 死坍缩渊（P2 lifecycle）时 Exit portal 被 despawn + TSY subzone 被 registry 移除，由 P2 负责把仍在内部的玩家强制弹回主世界（或按 race-out 失败处理）

**实现**：Exit portal 与 Entry portal 共用 `RiftPortal` component，只是 `direction = PortalDirection::Exit` 且 `target.dimension = Overworld`、`target.pos` 从 `TsyPresence.return_to` 取。

```rust
pub fn tsy_exit_portal_system(
    mut commands: Commands,
    players: Query<(Entity, &Position, &TsyPresence, &CurrentDimension)>,
    portals: Query<(&Position, &RiftPortal)>,
    tick: Res<ServerTick>,
    mut dim_transfer: EventWriter<DimensionTransferRequest>,
    mut emit: EventWriter<TsyExitEmit>,
) {
    for (entity, pos, presence, cur_dim) in &players {
        if cur_dim.0 != DimensionKind::Tsy { continue; }

        for (portal_pos, portal) in &portals {
            if !matches!(portal.direction, PortalDirection::Exit) { continue; }
            if portal.family_id != presence.family_id { continue; }
            if pos.0.distance(portal_pos.0) > portal.trigger_radius { continue; }

            // 走回对应 family 的 Exit portal → 跨位面回主世界锚点
            dim_transfer.send(DimensionTransferRequest {
                entity,
                target: presence.return_to.dimension,
                target_pos: presence.return_to.pos,
            });
            commands.entity(entity).remove::<TsyPresence>();

            emit.send(TsyExitEmit {
                player_entity: entity,
                family_id: presence.family_id.clone(),
                duration_ticks: tick.0 - presence.entered_at_tick,
            });
            break;
        }
    }
}
```

**注**：走到**另一个** TSY family 的 Exit portal 要阻止（`portal.family_id != presence.family_id` 那行）。P2 lifecycle 处理塌缩时的强制弹出是独立 system，不走这个 portal 路径。

---

## §4 入场过滤器

### 4.1 规则

按 `worldview.md §十六.四`：

- 物品 `spirit_quality >= 0.3` → **视为有附着真元**
- 入口瞬间负压抽走物品真元 → **`spirit_quality` 置 0** + **item 名改为「xxx 骨壳」/「xxx 枯枝」等凡物名**
- 物品其他属性（grid 尺寸、重量、稀有度）保留；只是"灵质"被洗掉
- 常见例子：
  - 满灵骨币（spirit_quality = 0.8）→ 退活骨壳（spirit_quality = 0, name = "枯骨残片"）
  - 鲜采灵草（spirit_quality = 0.6, freshness = Fresh）→ 枯灵草残（spirit_quality = 0, freshness = Withered）
  - 附灵剑（spirit_quality = 0.5）→ 凡铁剑（spirit_quality = 0, display_name = "锈迹斑斑的铁剑"）

### 4.2 实现

**位置**：`server/src/world/tsy_filter.rs`

```rust
pub const ENTRY_FILTER_THRESHOLD: f64 = 0.3;

pub struct FilteredItem {
    pub instance_id: u64,
    pub before_name: String,
    pub before_spirit_quality: f64,
}

/// 扫描 inventory，将所有 spirit_quality >= threshold 的 item 剥离真元
pub fn apply_entry_filter(inv: &mut PlayerInventory) -> Vec<FilteredItem> {
    let mut filtered = Vec::new();
    for container in inv.containers.iter_mut() {
        for item_opt in container.slots.iter_mut() {
            if let Some(item) = item_opt {
                if item.spirit_quality >= ENTRY_FILTER_THRESHOLD {
                    filtered.push(FilteredItem {
                        instance_id: item.instance_id,
                        before_name: item.display_name.clone(),
                        before_spirit_quality: item.spirit_quality,
                    });
                    apply_spirit_strip(item);
                }
            }
        }
    }
    // 同理扫描 equipped / hotbar
    for (_slot, item) in inv.equipped.iter_mut() {
        if item.spirit_quality >= ENTRY_FILTER_THRESHOLD {
            filtered.push(FilteredItem { .. });
            apply_spirit_strip(item);
        }
    }
    filtered
}

fn apply_spirit_strip(item: &mut ItemInstance) {
    item.spirit_quality = 0.0;
    // 更名逻辑（查表或动态前缀）
    item.display_name = strip_name(&item.template_id, &item.display_name);
    // 灵草 freshness 过滤
    if let Some(freshness) = item.freshness.as_mut() {
        *freshness = Freshness::Withered;
    }
}

fn strip_name(template_id: &str, original: &str) -> String {
    match template_id {
        id if id.starts_with("spirit_herb_") => format!("{}（枯）", original),
        "bone_coin" => "枯骨残片".to_string(),
        id if id.starts_with("weapon_") => format!("{}（失灵）", original),
        _ => format!("{}（无灵）", original),
    }
}
```

### 4.3 入场过滤的边界情况

- **空 inventory**：直接 return empty vec，不 panic
- **装备槽里的物品**：也被过滤（玩家穿着附灵铠甲进 → 铠甲灵纹瞬间失效，变成普通铠甲）
- **hotbar**：同上
- **堆叠物品**（stack_count > 1）：整堆一起过滤（丹药一次性几颗全废）
- **持久化**：过滤后的 inventory 通过 `save_player_core_slice` 落盘

---

## §5 测试策略

### 5.1 Rust unit tests（位置）

**新增测试文件**：

- `server/src/world/tsy.rs` 内置 `#[cfg(test)] mod tests` — 测 `is_tsy / tsy_layer / tsy_family_id`
- `server/src/world/tsy_drain.rs` 内置 tests — 测 `compute_drain_per_tick`（参数化，至少覆盖 "引气浅"/"引气深"/"化虚浅"/"化虚深" 四个点）
- `server/src/world/tsy_filter.rs` 内置 tests — 测 `apply_entry_filter` 各种 item 类型

**覆盖点**（最少 15 tests）：

- [ ] `is_tsy()`: 3 case（yes / no / empty name）
- [ ] `tsy_layer()`: 5 case（shallow / mid / deep / 非 tsy / 格式错）
- [ ] `tsy_family_id()`: 3 case
- [ ] `compute_drain_per_tick()`: 4 case（as §2.1 table）+ 非 TSY zone（0 drain）+ 池 = 0 边界
- [ ] `apply_entry_filter()`: 至少 5 case（满灵骨币 / 鲜灵草 / 附灵武器 / 低灵质 pass / 空 inventory）

### 5.2 集成测试

**位置**：`server/tests/tsy_zone_integration.rs`（新建）

用 Valence test harness 起一个临时 server，加 `/tsy-spawn` 命令 + 几个 hardcoded TSY zone，模拟：

- [ ] 玩家走到 portal 半径内 → `TsyPresence` 附着 + 传送到 shallow center
- [ ] 入场时 inventory 里的高灵质 item 被剥离（验证过滤 event）
- [ ] 5 秒后 `spirit_qi` 被抽一定量
- [ ] 玩家在 shallow 移动到 XZ 边界外 → `TsyPresence` 移除 + 传送回入口

### 5.3 Schema test

**位置**：`agent/packages/schema/src/tsy.spec.ts`（新建）

- [ ] TsyEnterEventV1 round-trip（TypeBox → JSON → parse）
- [ ] TsyExitEventV1 round-trip
- [ ] 跑 `npm run schema:export` 后 Rust 端能 serde deserialize 对应 JSON

### 5.4 Zone load 校验

**位置**：`server/src/world/zone.rs` 的 `ZoneRegistry::load_from_path` 扩展

加启动时校验：

- [ ] 所有 TSY subzone（`_shallow/_mid/_deep`）必须是三组存在（否则 warn）
- [ ] 同一位面内 zone 之间 AABB 不相交（同一 family 的三层共享 XZ + Y 分层是正常例外）
- [ ] 每个 TSY subzone 的 `zone.dimension == "bong:tsy"`；主世界的 `rift_portal` POI 所在 zone `zone.dimension == "minecraft:overworld"`（跨位面 zone 一致性）
- [ ] ~~TSY zones 不和非 TSY zone 相交~~（架构反转后自然满足：两类 zone 不在同一位面，不可能相交）

---

## §6 验收标准

### Automated

执行 `bash scripts/smoke-tsy-zone.sh`（新建）：

- [ ] `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
- [ ] `cd agent/packages/schema && npm test`
- [ ] `zones.json` sample 加载通过
- [ ] Rust / TS schema artifact 一致

### Manual QA

**前置**：server + client 联跑，offline mode 进游戏；确保 zones.json 包含至少一组 TSY subzone（如 `tsy_lingxu_01_*`）；用 `/tsy-spawn tsy_lingxu_01` 在玩家当前位置放一个测试裂缝。

- [ ] **A. 入场流**
  - [ ] 走到裂缝半径内 → 被传送到 shallow 层
  - [ ] 聊天栏看到 `[Server] 你踏进了裂缝。气息骤变……`（narration 由 agent 写，MVP 可 hardcode）
  - [ ] 进入前高灵质物品（如提前给一个 spirit_quality=0.7 的测试剑），入场后变成 "xxx（失灵）"
  - [ ] 低灵质物品不受影响
  - [ ] 真元条开始持续下降

- [ ] **B. 分层流**
  - [ ] 走到中层（Y 降到 40 以下）→ 真元下降速度加快
  - [ ] 走到深层（Y 降到 0 以下）→ 下降更猛
  - [ ] 反方向走回浅层 → 速度慢下来
  - [ ] 引气境在深层能苟 20 分钟以上（粗略 check，允许 ±50%）
  - [ ] （手动 Give 境界提升到化虚）→ 深层真元几秒归零

- [ ] **C. 出关流**
  - [ ] 走回 `_shallow` 中心 Exit portal 半径内 → 跨位面传回主世界 `return_to.pos`（+1 格浮空）
  - [ ] 跨位面 Respawn packet 发出，客户端场景重载（从 TSY 位面回到 `minecraft:overworld`）
  - [ ] `TsyPresence` component 被移除（/debug 命令显示）
  - [ ] 出关后真元条**不重置**（保持离开时的值；回复靠回到正灵气区静坐或嗑丹药）

- [ ] **D. 边界 / 错误处理**
  - [ ] 在 TSY 内死亡（真元归零）→ `DeathEvent` 发出 cause="tsy_drain"（死亡结算由 P1 plan 处理，P0 只验证事件发出）
  - [ ] `/tsy-spawn` 两次同 family id → 第二次报错不重复创建
  - [ ] `/tsy-spawn unknown_family` → 报错找不到 zones.json 里的对应 family

- [ ] **E. 持久化 / 断线**
  - [ ] 在 TSY 里断线重连 → `TsyPresence` 是否保留？**MVP 假设丢失**（下线 = 强制出关，inventory 持久化已处理）；如果玩家在 TSY 死了但没重连，运数/劫数仍按 §十二 走
  - [ ] 重连玩家坐标传送到灵龛（走 §十一 原规则）

### Acceptance bar

- [ ] 所有自动化 test 通过
- [ ] Manual QA A-D 全绿（E 允许一项未实现但要 log 清楚）
- [ ] `scripts/dev-reload.sh` 一键重启后能立刻 QA 而无需手动 seed

---

## §7 风险 / 未决

| 风险 | 级别 | 缓解 |
|------|------|------|
| 非线性指数 `n=1.5` 是否 balance | 高 | 先 hardcode，playtest 后调；预留 const 方便调 |
| 三个 subzone 共享 XZ 是否造成 `find_zone` 歧义 | 中 | 写一个 "Y 优先"的查询：按 Y 坐标落在哪层；测 § 5 zone load 校验 |
| 跨位面传送在 Fabric 客户端有 mixin / HUD 残留 | 中 | 见 `plan-tsy-dimension-v1 §4.2`；active 开工前手动 audit 一遍；MixinPlayerEntityHeldItem 等现有 mixin 初判无影响（传送完全 server 端），但实机复验 |
| 过滤器漏项（某些 item 字段没清 → 作弊带入） | 中 | 过滤器只看 `spirit_quality`，所有物品统一通过这个字段 gating；新增物品类型时必须 review |
| 出关传送把玩家送到掉崖 / 卡方块的位置 | 中 | MVP：`return_to.pos = 触发点 + (0, 1, 0)` 保证浮空 1 格；后续 check `is_air(pos + offset)` |
| `ZoneRegistry` 不支持运行时 hot-add | 高 | `/tsy-spawn` 命令需要扩 `ZoneRegistry::add(zone)` + `remove(name)`；原 registry 是启动时 load，目前只有 `load_from_path`；新增 `fn push(&mut self, zone: Zone)` |
| `plan-tsy-dimension-v1` 未落地前 P0 无法实装 | 高 | 本 plan active 阶段开工必须晚于 dimension plan active；骨架阶段可并行完成（本修订已完成） |

### 未决设计问题（本 plan 不解决，标记给后续）

- **多人同时进同一裂缝**：并发安全 — 需要确认 `TsyPresence` 的 attach 是否 safe（Bevy command 是 deferred，应该 OK；但要测）
- **裂缝 POI 持久化**：`/tsy-spawn` 放的 portal 断线后是否保留？MVP 不保留（只是调试工具）；正式发布走 worldgen 自动生成
- **TSY zone 里 NPC 行为**：现有 NPC patrol 逻辑对 TSY 友好吗？MVP 不在 TSY 里放 NPC；P2 lifecycle plan 引入道伥时再处理

---

## §8 后续 / 相关

**本 plan 依赖**（必须先落地）：

- `plan-tsy-dimension-v1.md`（基础设施前置）— 提供 `DimensionKind` enum、`DimensionAnchor` struct、`DimensionLayers` resource、`DimensionTransferRequest` event、`TerrainProviders` 多 provider routing、`CurrentDimension` component（玩家所在位面）

**依赖本 plan 完成后才能启动**：

- `plan-tsy-loot-v1.md`（P1）— 依赖 `TsyPresence` 做"秘境内死亡"的条件判定 + `entry_inventory_snapshot` 区分秘境所得
- `plan-tsy-lifecycle-v1.md`（P2）— 依赖 ZoneRegistry 动态 add/remove + 负压 tick system 的 hook + dimension plan 的锚点失效同步

**文件清单**（本 plan 新增）：

- `server/src/world/tsy.rs`（Component + Layer enum + helpers）
- `server/src/world/tsy_drain.rs`（drain tick）
- `server/src/world/tsy_portal.rs`（entry / exit portal systems + RiftPortal component）
- `server/src/world/tsy_filter.rs`（entry filter）
- `server/src/world/zone.rs`（+ 识别 helpers） — 修改
- `server/src/world/mod.rs`（注册子模块 + systems） — 修改
- `server/zones.tsy.json`（新文件，3 个 sample TSY subzone，详见 `plan-tsy-worldgen-v1 §2.1` 分文件决策；主世界 `zones.json` 可能同步加 `"dimension"` 字段）— 修改
- `server/zones.json` — 可能新增 `"dimension"` 字段默认值补注（Q2 候选 A：单 registry + Zone.dimension gating，见 `plan-tsy-dimension-v1 §6`）
- `agent/packages/schema/src/tsy.ts`（新 schema）
- `agent/packages/schema/src/index.ts` — 修改（导出）
- `server/tests/tsy_zone_integration.rs`（新 integration test）
- `scripts/smoke-tsy-zone.sh`（新 smoke script）

**不改的文件**（明确避免作用域蔓延）：

- `server/src/inventory/mod.rs` — loot 相关改动归 P1
- `server/src/combat/events.rs` — `DeathEvent` 扩展归 P1
- `client/src/main/resources/bong-client.mixins.json` — keepInventory mixin 归 P1
- `server/src/npc/**` — 道伥 archetype 归 P2
- `worldgen/**` + blueprint JSON（`zones.worldview.example.json` + 新增 `zones.tsy.json` 的 autogen 版本） — 归 **`plan-tsy-worldgen-v1`**（skeleton，地形/POI 自动生成 + POI Consumer System）
- `server/src/world/terrain/raster.rs` POI consumer — 归 `plan-tsy-worldgen-v1`
- Valence `DimensionTypeRegistry` 注册 / `LayerBundle` setup / `DimensionTransferRequest` 实现 / `TerrainProviders` 多 provider routing — 归 **`plan-tsy-dimension-v1`**（基础设施前置）

---

## §9 实施边界

此 plan 单次 `/consume-plan tsy-zone` 应该能在一次 worktree 里吃完。预估：

- Rust 新增代码：~800-1200 行（含 tests）
- TS schema：~80 行（新文件 + index 导出）
- zones.json：~60 行（3 个 sample subzone）
- Integration test：~150 行
- Smoke script：~30 行

规模 ≈ `plan-inventory-v1` 的 30%（因为复用了 Zone 框架而非重建）。
