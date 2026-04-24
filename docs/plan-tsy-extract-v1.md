# TSY 撤离点 · plan-tsy-extract-v1

> 搜打撤的**撤** —— 不能原地登出、不能传送、不能用符箓，必须站在**负压突破点**完成撤离仪式。本 plan 落地 `RiftPortal` Component（3 种 kind：主裂缝 / 深层缝 / 塌缩裂口）+ `ExtractProgress` Component + 撤离 tick + 中断规则 + race-out 模式切换。P5 在 P0/P1/P2 + P3/P4（可选）就位后开工。
> 交叉引用：`plan-tsy-v1.md §0/§2`（公理 / 横切）· `plan-tsy-dimension-v1`（位面基础设施前置）· `plan-tsy-zone-v1.md §1.4-§2`（`tsy_entry` tag / 入场传送 / `RiftPortal.direction` / `TsyPresence.return_to`）· `plan-tsy-lifecycle-v1.md §3-§5`（塌缩事件 / race-out / cleanup）· `plan-tsy-loot-v1.md §4`（秘境内死亡 → 干尸化）· `worldview.md §十六.四 撤离点：负压突破点`（3 种类型 + 撤离规则表）· `worldview.md §十六.一 step 4`（race-out 机制）

> **2026-04-24 架构反转备忘**：TSY 实现为独立位面。本 plan 所有"出关传送"已改为发 `DimensionTransferRequest { target: Overworld, target_pos: presence.return_to.pos }` 跨位面传送，而非 `insert(Position(entry_portal_pos))`。`MainWorldRiftAnchor` Resource 讨论（§2.3）被 `TsyPresence.return_to: DimensionAnchor` 原生表达收编，不再需要独立 Resource。§8 Q "不同入口进出坐标" 的最终决定：**用 `TsyPresence.return_to` 记录的进入锚点**（进 A 出 A），不按撤离 portal 的 family anchor 映射。

---

## §-1 现状（已实装 / 上游 plan 已锁）

| 层 | 能力 | 位置 |
|----|------|------|
| `Zone` / `ZoneRegistry` | zone 识别 / `find_zone(dim, pos)`（架构反转后按位面查） | `server/src/world/zone.rs:23-243` |
| Zone TSY helpers | `is_tsy()` / `tsy_layer()` / `tsy_family_id()` / `is_tsy_entry()` | P0 plan §1.2 |
| `TsyPresence` Component | `family_id` / `entered_at_tick` / `return_to: DimensionAnchor` | P0 plan §1.3 |
| **入口传送**（进 TSY） | 主世界 `rift_portal direction=entry` POI → `DimensionTransferRequest` 跨位面 → TSY dim `_shallow` 中心 | P0 plan §2 + `plan-tsy-dimension-v1 §3` |
| `apply_entry_filter(inv)` | 入场剥离 `spirit_quality >= 0.3` | P0 plan §2 |
| `Cultivation.spirit_qi` | 撤离期间继续消耗 | `server/src/cultivation/components.rs` |
| `CombatState` / `Wounds` | 中断条件的读侧 | `server/src/combat/components.rs` |
| `DeathEvent` | 真元归零 → 死亡事件链 | `server/src/combat/events.rs`（P1 plan 扩展） |
| `TsyLifecycle` | `New` / `Active` / `Declining` / `Collapsing` / `Dead` | P2 plan §3.1 |
| `TsyZoneStateRegistry` | 按 family_id 查 state | P2 plan §3.2 |
| `TsyCollapseStarted` / `TsyCollapseCompleted` 事件 | lifecycle 状态转移信号 | P2 plan §3.3 |
| 秘境内死亡 → 干尸化 | `TsyDeathDropOutcome` + `CorpseEmbalmed` | P1 plan §4 |
| `apply_drain_per_tick` | 已有 drain 主循环，可被 multiplier 叠加 | P0 plan §2 |

**本 plan 要新增**：`RiftPortal` Component + `RiftKind` enum + `ExtractProgress` Component + `StartExtractRequest` / `ExtractCompleted` / `ExtractAborted` / `ExtractFailed` 事件 + 撤离 tick system + 中断检测 + race-out 模式切换（`update_rifts_for_collapse`）+ 塌缩裂口 spawn + 塌缩完成时 portal cleanup + IPC schema `extract-v1` + 客户端撤离 HUD。

---

## §0 设计轴心（不可违反）

1. **撤离强制定点** — 在 TSY 内不能主动登出，不能传送，不能用符；玩家 disconnect 在服务器视角等同于**在原地继续挨抽**。撤离必须站在 `RiftPortal` 附近按键启动（§十六.四 撤离点）
2. **撤离 = 倒计时 + 中断**：7-10 / 10-15 / 3 秒三档倒计时，任何移动 / 战斗 / 受击都中断归零
3. **撤离期间真元照抽** — 没有"倒计时保护"，真元归零 = 当场失败（干尸化走 P1 的 `TsyDeathDropOutcome`）
4. **撤离完成 = 玩家 + 全部物品传到主世界裂缝外**（含秘境所得）
5. **三档 portal 职能互斥**：主裂缝双向；深层缝单向（只出不入）；塌缩裂口临时（只在 race-out 阶段存在 + 时长压缩到 3 秒）
6. **race-out 模式切换由 P2 触发** — 本 plan **只监听**`TsyCollapseStarted` 事件做模式切换，不自行判定塌缩条件
7. **塌缩完成 → 所有未撤离玩家"化灰"** — 按 §十六.六 走死亡路径（调用 P1 `TsyDeathDropOutcome`），本 plan 触发，不重复定义死亡结算
8. **撤离不免战斗** — PVP 可以打断撤离；战斗 AOE 也可以打断。撤离中被杀 = 和普通秘境死相同结算
9. **Portal 是 Entity**（而非 Zone 字段 / 特殊块），有 `Transform`，可以空间查询 / 撤离近接
10. **Portal 数量由 zone 元数据硬编码**，不是运行时随机 —— `/tsy-spawn` 命令同步创建；塌缩裂口是唯一的运行时动态 spawn
11. **不做对话 / 菜单交互** — 按 E（`USE` 键）即启动；按 ESC 即取消；不需要"确认撤离？"弹窗

---

## §1 数据模型

### 1.1 `RiftPortal` Component + `RiftKind` enum

**位置**：新建 `server/src/world/rift_portal.rs`，在 `server/src/world/mod.rs` 暴露

```rust
use bevy_ecs::prelude::Component;

/// TSY 的撤离点。挂在 Entity 上，Entity 位置 = portal 位置。
#[derive(Component, Debug)]
pub struct RiftPortal {
    pub kind: RiftKind,
    pub family_id: String,             // 所属 TSY 家族
    pub current_extract_ticks: u32,    // 当前生效的撤离时长（tick，20 tps）
    pub activation_window: Option<TickWindow>,  // None = 永久生效；Some = 限定存在区间
}

#[derive(Debug, Clone, Copy)]
pub struct TickWindow {
    pub start_at_tick: u64,
    pub end_at_tick: u64,  // end_at_tick 到达 → despawn
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiftKind {
    MainRift,       // 主裂缝 —— 双向（= P0 入口 + 出口），浅层 / 外缘
    DeepRift,       // 深层缝 —— 单向出，深层或深-中层交界
    CollapseTear,   // 塌缩裂口 —— race-out 时动态 spawn，时长压缩
}

impl RiftKind {
    pub const fn base_extract_ticks(self) -> u32 {
        match self {
            Self::MainRift     => 160,   //  8 秒（worldview 7-10 秒的中点）
            Self::DeepRift     => 240,   // 12 秒
            Self::CollapseTear =>  60,   //  3 秒
        }
    }

    pub const fn allows_entry(self) -> bool {
        matches!(self, Self::MainRift)  // 只有主裂缝可以进
    }

    pub const fn allows_exit(self) -> bool {
        true   // 三种都可以出
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MainRift     => "main_rift",
            Self::DeepRift     => "deep_rift",
            Self::CollapseTear => "collapse_tear",
        }
    }
}
```

**半径约定**：玩家距离 portal < `PORTAL_INTERACT_RADIUS`（默认 2.0 block）即可按 E 启动撤离。

### 1.2 `ExtractProgress` Component

挂在**玩家 Entity**（不在 portal 上，一个玩家同时只能撤离一个）：

```rust
#[derive(Component, Debug)]
pub struct ExtractProgress {
    pub portal: Entity,                 // 目标 portal
    pub required_ticks: u32,            // 快照启动时的 current_extract_ticks
    pub elapsed_ticks: u32,
    pub started_at_tick: u64,
    pub started_pos: [f64; 3],          // 中断检测基准
}
```

**生命周期**：
- 启动 → insert `ExtractProgress`
- 每 tick 累加 `elapsed_ticks`
- 完成 / 中断 / 失败 → remove

### 1.3 Zone 元数据扩展：`portal_specs`

**位置**：独立 `server/tsy_portals.json`（和 `tsy_containers.json` 平行，降低 zones.json 复杂度）

```json
{
  "tsy_lingxu_01": {
    "shallow": [
      { "kind": "main_rift", "pos": [1850, 80, 2850] },
      { "kind": "main_rift", "pos": [1810, 78, 2895] }
    ],
    "mid": [],
    "deep": [
      { "kind": "deep_rift", "pos": [1820, -10, 2810] }
    ]
  }
}
```

**读逻辑**：`load_tsy_portals() -> TsyPortalRegistry`（启动加载）。

---

## §2 撤离系统

### 2.1 启动撤离：`start_extract_request` system / event

**事件**：

```rust
#[derive(Event, Debug)]
pub struct StartExtractRequest {
    pub player: Entity,
    pub portal: Entity,
}

#[derive(Event, Debug)]
pub enum StartExtractResult {
    Started { player: Entity, portal: Entity, required_ticks: u32 },
    Rejected { player: Entity, portal: Entity, reason: ExtractRejectionReason },
}

#[derive(Debug, Clone)]
pub enum ExtractRejectionReason {
    OutOfRange,         // 距离 > 2 block
    AlreadyExtracting,  // 已挂 ExtractProgress
    InCombat,           // CombatState::Combat
    NotInTsy,           // 玩家不在 TSY 内（无 TsyPresence）
    PortalExpired,      // activation_window 已过期（CollapseTear 已关闭）
    CannotExit,         // 当前 zone 和 portal 不对应？理论不可能
    PortalCollapsed,    // TSY 已进入 Dead 状态
}
```

**system 逻辑**：

```rust
fn start_extract_request(
    mut events: EventReader<StartExtractRequest>,
    mut results: EventWriter<StartExtractResult>,
    portals: Query<(&RiftPortal, &Transform)>,
    players: Query<
        (&Transform, &TsyPresence, &CombatState, Option<&ExtractProgress>),
        With<Player>
    >,
    mut commands: Commands,
    tick: Res<ServerTick>,
    lifecycle_registry: Res<TsyZoneStateRegistry>,
) {
    for req in events.read() {
        let Ok((p_tf, p_presence, p_combat, p_extracting)) = players.get(req.player) else { continue };
        let Ok((portal, portal_tf)) = portals.get(req.portal) else { continue };

        if p_extracting.is_some() { /* AlreadyExtracting */ }
        if matches!(p_combat, CombatState::Combat { .. }) { /* InCombat */ }
        if distance(p_tf, portal_tf) > 2.0 { /* OutOfRange */ }

        if p_presence.family_id != portal.family_id { /* 不在同一 TSY，拒 */ }
        if let Some(win) = portal.activation_window {
            if tick.0 > win.end_at_tick { /* PortalExpired */ }
        }
        if let Some(state) = lifecycle_registry.get(&portal.family_id) {
            if matches!(state.lifecycle, TsyLifecycle::Dead) { /* PortalCollapsed */ }
        }

        // 通过 → 启动
        commands.entity(req.player).insert(ExtractProgress {
            portal: req.portal,
            required_ticks: portal.current_extract_ticks,
            elapsed_ticks: 0,
            started_at_tick: tick.0,
            started_pos: p_tf.translation.as_array_f64(),
        });

        results.send(StartExtractResult::Started { ... });
    }
}
```

### 2.2 撤离进度 tick：`tick_extract_progress` system

```rust
fn tick_extract_progress(
    mut players: Query<(Entity, &Transform, &CombatState, &Wounds, &Cultivation, &mut ExtractProgress)>,
    portals: Query<&RiftPortal>,
    mut commands: Commands,
    mut complete_events: EventWriter<ExtractCompleted>,
    mut abort_events: EventWriter<ExtractAborted>,
    mut fail_events: EventWriter<ExtractFailed>,
) {
    for (player_ent, tf, combat, wounds, cult, mut progress) in players.iter_mut() {
        // 真元归零 → 失败（干尸化）
        if cult.spirit_qi <= 0.0 {
            fail_events.send(ExtractFailed {
                player: player_ent,
                portal: progress.portal,
                reason: ExtractFailureReason::SpiritQiDrained,
            });
            commands.entity(player_ent).remove::<ExtractProgress>();
            continue;
        }

        // 中断条件
        if distance(tf.translation.as_array_f64(), progress.started_pos) > 0.5 {
            abort_events.send(ExtractAborted { player: player_ent, portal: progress.portal, reason: Moved });
            commands.entity(player_ent).remove::<ExtractProgress>();
            continue;
        }
        if matches!(combat, CombatState::Combat { .. }) {
            abort_events.send(ExtractAborted { reason: Combat, .. });
            commands.entity(player_ent).remove::<ExtractProgress>();
            continue;
        }
        if wounds.damaged_this_tick() {
            abort_events.send(ExtractAborted { reason: Damaged, .. });
            commands.entity(player_ent).remove::<ExtractProgress>();
            continue;
        }

        progress.elapsed_ticks += 1;

        if progress.elapsed_ticks >= progress.required_ticks {
            complete_events.send(ExtractCompleted { player: player_ent, portal: progress.portal });
            commands.entity(player_ent).remove::<ExtractProgress>();
        }
    }
}
```

**注**：真元依然由 P0 的 `apply_drain_per_tick` system 独立抽取——本 plan 不碰 drain 公式，只在撤离失败时读最终值。

### 2.3 撤离完成：`handle_extract_completed` system

```rust
fn handle_extract_completed(
    mut events: EventReader<ExtractCompleted>,
    mut commands: Commands,
    presences: Query<&TsyPresence>,
    mut dim_transfer: EventWriter<DimensionTransferRequest>,
) {
    for e in events.read() {
        let Ok(presence) = presences.get(e.player) else { continue };

        // 1. 跨位面传回主世界入场锚点
        //    layer 切换 + Position 更新 + Respawn packet 由
        //    `plan-tsy-dimension-v1 §3` 的 `apply_dimension_transfers` 统一处理
        dim_transfer.send(DimensionTransferRequest {
            entity: e.player,
            target: presence.return_to.dimension,   // = DimensionKind::Overworld
            target_pos: presence.return_to.pos,     // = 进入裂缝时记录的主世界锚点
        });

        // 2. 移除 TsyPresence（玩家离开秘境会话）
        // 注：inventory 内容保留——上古遗物 / 凡物都带出
        // 注：drain 自然停止（PlayerPressureSystem 看不到 TsyPresence）
        commands.entity(e.player).remove::<TsyPresence>();

        // 3. 发 agent IPC（ExtractCompletedV1）给天道 narration
        // （narration 侧独立 plan 消费）
    }
}
```

**主世界锚点**：~~新增 `MainWorldRiftAnchor` Resource~~（2026-04-24 架构反转后不再需要）。直接读 `TsyPresence.return_to: DimensionAnchor`，它记录了进入 TSY 时的主世界锚点（dimension + pos）。出关 = 发 `DimensionTransferRequest { target: return_to.dimension, target_pos: return_to.pos }` 跨位面传回主世界。

### 2.4 撤离失败：`handle_extract_failed` system

```rust
fn handle_extract_failed(
    mut events: EventReader<ExtractFailed>,
    mut death_events: EventWriter<DeathEvent>,
    tick: Res<ServerTick>,
) {
    for e in events.read() {
        // 失败 = 死亡，走 P1 plan 的 TsyDeathDropOutcome 路径
        death_events.send(DeathEvent {
            target: e.player,
            cause: format!("tsy_extract_failed:{}", match e.reason {
                ExtractFailureReason::SpiritQiDrained => "spirit_qi_drained",
            }),
            attacker: None,
            attacker_player_id: None,
            at_tick: tick.0,
        });
        // P1 plan 的 apply_tsy_death_drop 接手：100% 秘境所得掉 + 干尸化
    }
}
```

---

## §3 Race-out 模式切换

### 3.1 监听 P2 `TsyCollapseStarted` 事件

```rust
fn on_tsy_collapse_started(
    mut events: EventReader<TsyCollapseStarted>,
    mut portals: Query<(Entity, &mut RiftPortal)>,
    mut commands: Commands,
    zone_registry: Res<ZoneRegistry>,
    tick: Res<ServerTick>,
) {
    for e in events.read() {
        // 1. 现有 portal（MainRift + DeepRift）的 current_extract_ticks 强制缩到 3 秒（= 60 tick）
        for (_, mut portal) in portals.iter_mut() {
            if portal.family_id == e.family_id {
                portal.current_extract_ticks = 60;
            }
        }

        // 2. spawn 3-5 个 CollapseTear 临时 portal
        spawn_collapse_tears(&mut commands, &zone_registry, &e.family_id, tick.0);
    }
}
```

**spawn_collapse_tears**：

```rust
fn spawn_collapse_tears(
    commands: &mut Commands,
    zone_registry: &ZoneRegistry,
    family_id: &str,
    now_tick: u64,
) {
    let count = rand::random_range(3..=5);
    let family_zones: Vec<&Zone> = zone_registry.all()
        .filter(|z| z.tsy_family_id().as_deref() == Some(family_id))
        .collect();

    for _ in 0..count {
        let zone = family_zones.choose(&mut rng).unwrap();
        let pos = random_point_in_aabb(&zone.bounds);

        commands.spawn((
            Transform::from_xyz(pos[0], pos[1], pos[2]),
            RiftPortal {
                kind: RiftKind::CollapseTear,
                family_id: family_id.to_string(),
                current_extract_ticks: 60,   // 3 秒
                activation_window: Some(TickWindow {
                    start_at_tick: now_tick,
                    end_at_tick: now_tick + COLLAPSE_DURATION_TICKS,  // 来自 P2
                }),
            },
        ));
    }
}
```

`COLLAPSE_DURATION_TICKS` 来自 P2 plan §3.4（默认 `30 * 20 = 600` tick = 30s）。CollapseTear 存续到塌缩完成时一并消失。

### 3.2 `TsyCollapseCompleted` 时 cleanup

```rust
fn on_tsy_collapse_completed(
    mut events: EventReader<TsyCollapseCompleted>,
    portals: Query<(Entity, &RiftPortal)>,
    players_in_tsy: Query<(Entity, &TsyPresence, Option<&ExtractProgress>)>,
    mut death_events: EventWriter<DeathEvent>,
    mut commands: Commands,
    tick: Res<ServerTick>,
) {
    for e in events.read() {
        // 1. 所有属于这个 family_id 的 portal 全部 despawn
        for (portal_ent, portal) in portals.iter() {
            if portal.family_id == e.family_id {
                commands.entity(portal_ent).despawn();
            }
        }

        // 2. 所有还在 TSY 内的玩家（有 TsyPresence 且未完成撤离）—— 化灰
        for (player_ent, presence, extracting) in players_in_tsy.iter() {
            if presence.family_id == e.family_id {
                // 包括正在撤离中（elapsed < required）的玩家——死
                death_events.send(DeathEvent {
                    target: player_ent,
                    cause: String::from("tsy_collapsed"),
                    attacker: None,
                    attacker_player_id: None,
                    at_tick: tick.0,
                });
                // P1 plan 的 apply_tsy_death_drop 走——但因为 zone 同时被 P2 清理，所以 drops 会消失（"100% 掉落但随坍缩渊化灰"—— §十六.六 原文）
            }
        }
    }
}
```

**`TsyCollapseCompleted` 的定义权**：属于 P2 plan。本 plan 只消费。

### 3.3 CollapseTear 过期自动消失

```rust
fn despawn_expired_portals(
    portals: Query<(Entity, &RiftPortal)>,
    mut commands: Commands,
    tick: Res<ServerTick>,
) {
    for (ent, portal) in portals.iter() {
        if let Some(win) = portal.activation_window {
            if tick.0 > win.end_at_tick {
                commands.entity(ent).despawn();
            }
        }
    }
}
```

---

## §4 客户端同步

### 4.1 IPC schema：`extract-v1`

新建 `agent/packages/schema/src/extract-v1.ts`：

```typescript
import { Type } from '@sinclair/typebox';

export const RiftPortalStateV1 = Type.Object({
  entity_id: Type.Number(),
  kind: Type.Union([
    Type.Literal('main_rift'),
    Type.Literal('deep_rift'),
    Type.Literal('collapse_tear'),
  ]),
  family_id: Type.String(),
  world_pos: Type.Tuple([Type.Number(), Type.Number(), Type.Number()]),
  current_extract_ticks: Type.Number(),
  activation_window_end: Type.Optional(Type.Number()),  // CollapseTear 的 end_at_tick
});

export const ExtractStartedV1 = Type.Object({
  player_id: Type.String(),
  portal_entity_id: Type.Number(),
  portal_kind: Type.String(),
  required_ticks: Type.Number(),
  at_tick: Type.Number(),
});

export const ExtractProgressV1 = Type.Object({
  player_id: Type.String(),
  portal_entity_id: Type.Number(),
  elapsed_ticks: Type.Number(),
  required_ticks: Type.Number(),
});

export const ExtractCompletedV1 = Type.Object({
  player_id: Type.String(),
  portal_kind: Type.String(),
  family_id: Type.String(),
  exit_world_pos: Type.Tuple([Type.Number(), Type.Number(), Type.Number()]),
  at_tick: Type.Number(),
});

export const ExtractAbortedV1 = Type.Object({
  player_id: Type.String(),
  reason: Type.Union([
    Type.Literal('moved'),
    Type.Literal('combat'),
    Type.Literal('damaged'),
    Type.Literal('cancelled'),
  ]),
});

export const ExtractFailedV1 = Type.Object({
  player_id: Type.String(),
  reason: Type.Union([
    Type.Literal('spirit_qi_drained'),
  ]),
});

export const TsyCollapseStartedIpcV1 = Type.Object({
  family_id: Type.String(),
  at_tick: Type.Number(),
  remaining_ticks: Type.Number(),     // 塌缩倒计时（COLLAPSE_DURATION_TICKS）
  collapse_tear_entity_ids: Type.Array(Type.Number()),
});
```

**触发时机**：
- `RiftPortalStateV1` — zone 初始化时广播 portal 列表 + 塌缩开始时广播 CollapseTear 新增
- `ExtractStartedV1` / `AbortedV1` / `CompletedV1` / `FailedV1` — 对应 system 触发时发
- `ExtractProgressV1` — 每 5 tick 广播一次（和 P3 容器搜刮频率一致）
- `TsyCollapseStartedIpcV1` — race-out 开始时广播（agent narration 用）

### 4.2 客户端 HUD

**位置**：新建 `client/src/main/java/com/bong/client/tsy/ExtractProgressHud.java`

**显示**：
- 玩家站在 portal 近接范围内 → HUD 底部弹"按 E 开始撤离 [<time_total>s]"提示（portal 类型做前缀："主裂缝 / 深层缝 / 塌缩裂口"）
- 启动后 → 进度条（和真元条对称，下方 + 显示"撤离中 [<remaining>s]"）
- 期间真元继续下降（已有真元条），两条同屏滚动
- 中断 → 闪红 + 原因文字
- 完成 → 黑屏渐入渐出 + 显示"已撤出：<family_id>"
- 失败（真元归零）→ 死亡 sequence（客户端走原有 death screen）

**race-out HUD**：
- 监听 `TsyCollapseStartedIpcV1` → 全屏闪红 + 顶部中央常驻倒计时"坍缩倒计时 <s>s"
- 倒计时到 0 → 全屏白光一秒 → 死亡 / 传出（由 server 端决定）

### 4.3 客户端交互

- 玩家走到 portal 半径 2 block 内 → 按 `USE` 键（默认 E）→ 发 `StartExtractRequest` IPC
- 撤离期间按 `ESC` → 发 `CancelExtractRequest`（server 发 `ExtractAborted { reason: cancelled }`）
- 移动 WASD / 攻击键 / 受击均由 server 判定中断，client 只渲染结果

---

## §5 横切 / 依赖整合

### 5.1 对 P0 plan 的依赖

- `TsyPresence` Component：必须有此 Component 才能撤离（NotInTsy 拒）
- `TsyPresence.return_to: DimensionAnchor`：撤离完成后发 `DimensionTransferRequest` 跨位面传回此锚点（架构反转前为 `entry_portal_pos: DVec3`）
- `apply_drain_per_tick`：撤离期间真元继续消耗

### 5.2 对 P1 plan 的依赖

- `DeathEvent.attacker / attacker_player_id`：撤离失败 → DeathEvent（attacker 为 None，除非是 PVP 被击中断撤离）
- `TsyDeathDropOutcome`：真元归零或塌缩被清死 → 走 P1 的 100% 秘境所得掉 + 干尸化

### 5.3 对 P2 plan 的依赖 / 反依赖

- **监听** `TsyCollapseStarted`（由 P2 发）→ 切 race-out 模式
- **监听** `TsyCollapseCompleted`（由 P2 发）→ despawn 所有 portal + 清内部玩家
- P2 的 `TsyLifecycle::Collapsing` / `Dead` 状态：本 plan 在 `StartExtractRequest` 检查时读

### 5.4 对 P3 plan 的依赖

- P3 容器搜刮期间如果玩家启动 `StartExtractRequest` → 拒（`AlreadyExtracting` 改名为 `AlreadyBusy` 更贴切），或自动取消搜刮
- **决定**：按 `AlreadyBusy` 拒——玩家必须先 ESC 取消搜刮，再启动撤离。这让玩家明确决策，避免"一键撤退"省略搜刮放弃的显式行为

**实施对齐**：`ExtractRejectionReason::AlreadyBusy` 覆盖 `AlreadyExtracting` + `IsSearching` 两种情况。P3 plan 的 `SearchRejectionReason::AlreadyBusy` 类似覆盖"正在撤离"。Meta plan §2 横切追加 `2.6 忙态互斥`（由**本 P5 plan 接纳**，提供共享 enum `PlayerBusyState`）。

### 5.5 对 P4 plan 的依赖

- P4 Fuya aura 在撤离期间**仍然生效** —— 站在 Fuya 光环里撤离 = 真元 drain × 1.5，撤不完就死。设计即预期，不做特殊规避
- P4 守灵 / 道伥在 portal 附近 spawn 概率按现有 spawn pool 随机——可能守关也可能不守，玩家要接受这个 RNG

---

## §6 验收 demo

**E2E 场景**（P5 单阶段）：

1. `/tsy-spawn tsy_lingxu_01` 生成一个 TSY，内含 2 个 MainRift（浅层）+ 1 个 DeepRift（深层）
2. 玩家 A 走到 MainRift 附近 → HUD 显示"按 E 开始撤离 [8s]"
3. A 按 E → 进度条启动，真元条继续下降；等 8 秒完成 → 传出到主世界 MainRift 附近坐标
4. 玩家 B 也进 TSY，走到 DeepRift 附近 → HUD 显示"按 E 开始撤离 [12s]"
5. B 按 E → 进度条启动；启动后 3 秒 B 受击（附近道伥追上来）→ 撤离中断，HUD 闪"受击，撤离中断"
6. B 回头杀道伥 → 再次按 E → 再等 12 秒 → 传出
7. 玩家 C 进 TSY，下深层搜空最后一个 RelicCore → **P2 触发 `TsyCollapseStarted`**
8. 本 plan 监听 → 所有 portal `current_extract_ticks = 60`（3 秒）+ spawn 3-5 个 CollapseTear
9. C 已经在 relic_core 旁，最近的 CollapseTear 距离 8 block → C 冲过去 → 按 E → 3 秒完成 → 传出
10. 玩家 D 在中层被 Fuya 拖住 → 跑不到任何 portal → 真元归零 → `ExtractFailed { SpiritQiDrained }` → 干尸化（100% 秘境所得掉 at 死亡位置）
11. 塌缩倒计时结束 → `TsyCollapseCompleted` → 本 plan despawn 所有 portal；任何还在 TSY 内的玩家（D 已死不算；E 如果还在里面）立即死亡 → 化灰
12. zone 注册表对 `tsy_lingxu_01` 全部清除（P2 plan 侧 cleanup）

**自动化测试**：

- `server/src/world/rift_portal.rs::tests`：
  - `RiftKind::base_extract_ticks` 表校验
  - `allows_entry` / `allows_exit` 表校验
- `server/src/world/extract_system.rs::tests`：
  - `start_extract_request` 所有 Rejected 路径
  - `tick_extract_progress` 中断条件（移动 / 战斗 / 受击）
  - 真元归零 → `ExtractFailed`
  - 完成 → 玩家 `CurrentDimension == Overworld` 且 `Position == presence.return_to.pos`
  - `on_tsy_collapse_started` → 所有 portal current_extract_ticks = 60 + spawn 3-5 CollapseTear
  - `on_tsy_collapse_completed` → portal 全 despawn + 内部玩家 DeathEvent
- 集成：`cargo test tsy_extract` 通过

---

## §7 非目标（推迟 / 独立 plan）

| 功能 | 状态 | 说明 |
|------|------|------|
| Portal 视觉 / 粒子 / sound | client polish | 本 plan 只实装 HUD text + 进度条 |
| 撤离**被打断**时有小概率保留进度（"强韧" trait） | 独立 plan | 涉及修士个性系统，非 TSY 核心玩法 |
| 多人同时撤离 | 不做 | portal 不限玩家数，谁启动谁撤；互不影响 |
| 撤离被中断后短时"眩晕" | P5 后续 | MVP 只做中断归零 |
| Portal 被**玩家破坏**（PVP 阻截策略） | 不做 | Portal 是世界规则不是实物 |
| 入场流程（进 TSY）实装 | **P0 plan 负责** | 本 plan 只负责出关 |
| 出关后在主世界的 narration hook | 独立 plan | 由 agent 层响应 `ExtractCompletedV1` 生成叙事 |
| 撤离中 AFK / disconnect 的处理 | P5 后续 | MVP：disconnect = 玩家 entity 保留在 TSY 内继续挨抽直到真元归零死 |
| 塌缩裂口 spawn 的**视野保证**（不在墙里） | P5 后续 | MVP 随机 spawn；后续加 raycast 检查 |

---

## §8 风险 / 未决

| 风险 | 级别 | 缓解 |
|------|------|------|
| `on_tsy_collapse_started` 和 `tick_extract_progress` 的执行顺序 | **高** | 必须：`on_tsy_collapse_started` 先于 `tick_extract_progress` 每帧运行。用 `SystemSet` 强制顺序；否则玩家可能在"已塌缩但本 tick 未切 rifts"的窗口完成撤离 |
| CollapseTear spawn 在 `blocked_tiles` 或墙里 | 中 | Rejection sampling 最多 20 次；撞 block 就换位置 |
| 玩家同时满足"站在 portal 旁"和"在战斗"→ start 被拒但 HUD 不提示 | 中 | `ExtractRejectionReason::InCombat` 显式发 IPC；client 显示 rejection 原因 1 秒 |
| 撤离中玩家被拉拽（e.g. 敌人技能拖拽）→ 位置变化触发中断 | 低 | 设计即预期；玩家要选"无人打扰"的时机开撤离 |
| 玩家从**入口 A** 进、想从**入口 B** 出 → 出关坐标是哪个？ | 中 | **已决（2026-04-24）**：用 `TsyPresence.return_to` 记录的进入锚点（即 A）。架构反转后 return_to 天生带 dimension + pos 信息，不需要独立 `MainWorldRiftAnchor` Resource。语义上也更合理：你从 A 钻进去，意识就栓在 A 上，从哪个撤离点出都回 A |
| 塌缩完成时清死玩家 → DeathEvent 发了但 zone 也被 cleanup → `CorpseEmbalmed` 挂不上谁 | 中 | 执行顺序：先 DeathEvent（P1 消费 → 干尸 Component 挂玩家 corpse entity）→ 再 zone cleanup；P1 的 `CorpseEmbalmed` 被带着一起 despawn（符合"随坍缩渊化灰"） |
| 撤离时真元继续抽 + 如果 Fuya aura 同时生效 → 真元速率 × 1.5 可能撑不到 8 秒 | 低 | 设计即预期；玩家要主动走离 Fuya aura 再开撤离 |
| `current_extract_ticks = 60` 批量改动时并发 | 低 | `Query<&mut RiftPortal>` 自动串行；无并发问题 |
| 塌缩开始但玩家已经在 MainRift 撤离一半（5 秒已过，还剩 3 秒）→ 是按新的 3 秒算还是旧的？ | **高** | 决定：**按新的从现在开始 3 秒**。实施：`on_tsy_collapse_started` 同时把 `ExtractProgress.required_ticks = 60` 和 `elapsed_ticks = 0` 重置。Race-out 是"最公平的秒表重开"，已经撤了一半的玩家和新开始的一样有 3 秒 |

---

## §9 命名与版本

- 本 plan 文件：`plan-tsy-extract-v1.md`
- 实施后归档：`docs/finished_plans/plan-tsy-extract-v1.md`
- v2 触发条件：Portal 破坏机制 / AFK 处理策略 / 撤离视觉 plan

---

**下一步**：P0/P1/P2（P3/P4 可选）就位后，`/consume-plan tsy-extract` 启动 P5。
