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

---

## §0 设计轴心

1. **Zone 以 name 前缀识别**：`zone.name.starts_with("tsy_")` 即为活坍缩渊。不改 Zone 结构的 shape，只约定命名，降低扩展摩擦（见 `plan-tsy-v1.md §2.3`）
2. **负压机制只读 Zone 现有字段**：`Zone.spirit_qi ∈ [-1.2, -0.3]` 即为 TSY 内部灵压；不新增 `draining_rate` 字段（从 spirit_qi 推导）
3. **内部层深用多个 subzone 表达**：一个"活坍缩渊"在 zones.json 里是**三个相邻 zone**（`tsy_xxx_shallow` / `_mid` / `_deep`），通过 name 后缀联动。好处是复用 Zone 现有几何判定 + 避免"一个 zone 多个灵压"的特殊逻辑
4. **入口 POI 走现有 `active_events` 字段**：用 `"portal_rift"` tag 标记该 zone 靠近边缘的传送点；TSY 的入口 zone 同时拥有 `"tsy_entry"` tag
5. **传送不是跨 dimension，是同一 MC world 内的坐标传送** — MVP 0.1 不碰 Bevy world / MC dimension 层；所有 TSY zone 在主世界里有物理坐标（ZoneRegistry 的 AABB）
6. **入场过滤是入口传送的 on-arrival hook** — 传送完成后扫描玩家 inventory 所有 item，`spirit_quality >= 0.3` 的 item 在入口被"剥离"（set to 0 + spawn 一个 bone/灰烬 item 替代），离场不再恢复

---

## §1 数据模型

### 1.1 Zone 配置扩展（无 struct 改动，仅约定）

TSY 系列 zone 在 `server/zones.json` 里的模板：

```json
{
  "name": "tsy_lingxu_01_shallow",
  "aabb": { "min": [1800, 40, 2800], "max": [1900, 120, 2900] },
  "spirit_qi": -0.4,
  "danger_level": 4,
  "active_events": ["tsy_entry", "portal_rift"],
  "patrol_anchors": [[1850, 80, 2850]],
  "blocked_tiles": []
},
{
  "name": "tsy_lingxu_01_mid",
  "aabb": { "min": [1800, 0, 2800], "max": [1900, 40, 2900] },
  "spirit_qi": -0.7,
  "danger_level": 5,
  "active_events": [],
  "patrol_anchors": [[1850, 20, 2850]],
  "blocked_tiles": []
},
{
  "name": "tsy_lingxu_01_deep",
  "aabb": { "min": [1800, -40, 2800], "max": [1900, 0, 2900] },
  "spirit_qi": -1.1,
  "danger_level": 5,
  "active_events": [],
  "patrol_anchors": [[1850, -20, 2850]],
  "blocked_tiles": []
}
```

**约定**：
- 三个 subzone **共享 XZ bounds**，Y 轴垂直分层（浅层顶上、深层底下）
- 玩家在 TSY 内走动时通过 Y 坐标自然跨层，`ZoneRegistry.find_zone(pos)` 返回对应层
- **命名前缀** `tsy_<来源>_<序号>_<层深>`；`<层深>` ∈ `{shallow, mid, deep}`

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
    /// 入口坐标（用于出关传送回对应位置）
    pub entry_portal_pos: DVec3,
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
  entry_portal_pos: Type.Array(Type.Number(), { minItems: 3, maxItems: 3 }),
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
pub fn tsy_drain_tick(
    mut players: Query<(Entity, &mut PlayerState, &Position, &TsyPresence)>,
    zones: Res<ZoneRegistry>,
    tick: Res<ServerTick>,
    mut death_events: EventWriter<DeathEvent>,
) {
    for (entity, mut state, pos, _presence) in &mut players {
        let Some(zone) = zones.find_zone(pos.0) else { continue };
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

## §3 裂缝 POI 与双向传送

### 3.1 裂缝入口

裂缝 = 主世界的某个坐标，靠近时触发传送进 TSY。

**MVP 实现**：调试命令 `/tsy-spawn <family_id>` 手动在当前位置放置一个裂缝 + 对应的 TSY 三个 subzone（动态追加到 ZoneRegistry）。正式发布要和 worldgen 对接（`worldgen` Python 脚本生成 POI，此为后续 plan）。

### 3.2 Rift POI Component

**位置**：`server/src/world/tsy.rs`

```rust
/// 裂缝 POI（附着在世界某个坐标的空气方块或标记实体）
#[derive(Component, Debug, Clone)]
pub struct RiftPortal {
    /// 对应 TSY family id（如 "tsy_lingxu_01"）
    pub family_id: String,
    /// 入口传送目标：对应 family 的 _shallow 层的 center
    pub entry_destination: DVec3,
    /// 激活半径（玩家靠近时触发传送）
    pub trigger_radius: f64,  // MVP = 1.5 格
}
```

裂缝本身**不是实体**，是附着在世界坐标的 marker component（用 EntityMap 查找）。MVP 用一个隐形的 armor stand 载体，后续可以换成 particle / 特殊方块。

### 3.3 Entry 传送 System

**位置**：`server/src/world/tsy_portal.rs`，注册到 `FixedUpdate`

```rust
pub fn tsy_entry_portal_system(
    mut commands: Commands,
    players: Query<(Entity, &Position, &PlayerState, &PlayerInventory), Without<TsyPresence>>,
    portals: Query<(&Position, &RiftPortal)>,
    zones: Res<ZoneRegistry>,
    tick: Res<ServerTick>,
    mut emit: EventWriter<TsyEnterEmit>,
) {
    for (player_entity, player_pos, state, inv) in &players {
        for (portal_pos, portal) in &portals {
            if player_pos.0.distance(portal_pos.0) <= portal.trigger_radius {
                // Step 1: 入场过滤（见 §4）
                let filtered = apply_entry_filter(inv);

                // Step 2: attach TsyPresence
                commands.entity(player_entity).insert(TsyPresence {
                    family_id: portal.family_id.clone(),
                    entered_at_tick: tick.0,
                    entry_inventory_snapshot: inv.all_instance_ids(),
                    entry_portal_pos: portal_pos.0,
                });

                // Step 3: 传送到 shallow 层 center
                commands.entity(player_entity).insert(Position(portal.entry_destination));

                // Step 4: emit event
                emit.send(TsyEnterEmit { player_entity, family_id: portal.family_id.clone(), filtered });

                break;  // 一个玩家一 tick 只能进一个 portal
            }
        }
    }
}
```

### 3.4 Exit 传送

**设计决策**：出关 = 玩家从 TSY subzone 走出 AABB（例如走到 `_shallow` 的 XZ 边界外）。不需要特殊的"出口方块"——一走出 zone，就传送回入口坐标。

**原因**：
- 简化 UX（玩家不用找特定出口）
- 和负压机制对齐：撤退就是往外走，走到 zone 边界自然出关
- 死坍缩渊（塌缩后，P2 plan）时，zone 被 registry 移除，玩家自动出界 → 自动传送出（但那时真元可能已经被 race-out 抽干，出来就是死）

**实现**：

```rust
pub fn tsy_exit_portal_system(
    mut commands: Commands,
    players: Query<(Entity, &Position, &TsyPresence, &PlayerState)>,
    zones: Res<ZoneRegistry>,
    tick: Res<ServerTick>,
    mut emit: EventWriter<TsyExitEmit>,
) {
    for (entity, pos, presence, state) in &players {
        let zone = zones.find_zone(pos.0);
        let in_tsy = zone.map_or(false, |z| z.is_tsy() && z.tsy_family_id().as_deref() == Some(&presence.family_id));

        if !in_tsy {
            // 玩家走出了 TSY subzones（或当前 zone 是别的 TSY，走错了）
            // 传送回入口坐标
            commands.entity(entity).insert(Position(presence.entry_portal_pos));
            commands.entity(entity).remove::<TsyPresence>();

            emit.send(TsyExitEmit {
                player_entity: entity,
                family_id: presence.family_id.clone(),
                duration_ticks: tick.0 - presence.entered_at_tick,
            });
        }
    }
}
```

**注**：`in_tsy` 判定考虑了"走到**另一个** TSY"的奇葩场景（两个 TSY zones 紧邻）。本 plan 假设 zone load 时校验 TSY 互不相交（见 §5 测试）。

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
- [ ] TSY zones 之间 AABB 不相交（除了同一 family 的三层在 Y 轴上重叠是正常）
- [ ] TSY zones 不和非 TSY zone 相交（否则 panic 或 warn，至少 log）

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
  - [ ] 走出 XZ bounds → 被传送回裂缝附近（entry_portal_pos）
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
| MixinPlayerEntityHeldItem 等现有 client mixin 是否影响入场传送 | 低 | 传送完全在 server 端，client mixin 不参与逻辑 |
| 过滤器漏项（某些 item 字段没清 → 作弊带入） | 中 | 过滤器只看 `spirit_quality`，所有物品统一通过这个字段 gating；新增物品类型时必须 review |
| 出关传送把玩家送到掉崖 / 卡方块的位置 | 中 | MVP：`entry_portal_pos + [0, 1, 0]` 保证浮空 1 格；后续 check `is_air(pos + offset)` |
| `ZoneRegistry` 不支持运行时 hot-add | 高 | `/tsy-spawn` 命令需要扩 `ZoneRegistry::add(zone)` + `remove(name)`；原 registry 是启动时 load，目前只有 `load_from_path`；新增 `fn push(&mut self, zone: Zone)` |

### 未决设计问题（本 plan 不解决，标记给后续）

- **多人同时进同一裂缝**：并发安全 — 需要确认 `TsyPresence` 的 attach 是否 safe（Bevy command 是 deferred，应该 OK；但要测）
- **裂缝 POI 持久化**：`/tsy-spawn` 放的 portal 断线后是否保留？MVP 不保留（只是调试工具）；正式发布走 worldgen 自动生成
- **TSY zone 里 NPC 行为**：现有 NPC patrol 逻辑对 TSY 友好吗？MVP 不在 TSY 里放 NPC；P2 lifecycle plan 引入道伥时再处理

---

## §8 后续 / 相关

**依赖本 plan 完成后才能启动**：

- `plan-tsy-loot-v1.md`（P1）— 依赖 `TsyPresence` 做"秘境内死亡"的条件判定 + `entry_inventory_snapshot` 区分秘境所得
- `plan-tsy-lifecycle-v1.md`（P2）— 依赖 ZoneRegistry 动态 add/remove + 负压 tick system 的 hook

**文件清单**（本 plan 新增）：

- `server/src/world/tsy.rs`（Component + Layer enum + helpers）
- `server/src/world/tsy_drain.rs`（drain tick）
- `server/src/world/tsy_portal.rs`（entry / exit portal systems + RiftPortal component）
- `server/src/world/tsy_filter.rs`（entry filter）
- `server/src/world/zone.rs`（+ 识别 helpers） — 修改
- `server/src/world/mod.rs`（注册子模块 + systems） — 修改
- `server/zones.json`（+ 3 个 sample TSY subzone） — 修改
- `agent/packages/schema/src/tsy.ts`（新 schema）
- `agent/packages/schema/src/index.ts` — 修改（导出）
- `server/tests/tsy_zone_integration.rs`（新 integration test）
- `scripts/smoke-tsy-zone.sh`（新 smoke script）

**不改的文件**（明确避免作用域蔓延）：

- `server/src/inventory/mod.rs` — loot 相关改动归 P1
- `server/src/combat/events.rs` — `DeathEvent` 扩展归 P1
- `client/src/main/resources/bong-client.mixins.json` — keepInventory mixin 归 P1
- `server/src/npc/**` — 道伥 archetype 归 P2
- `worldgen/**` — worldgen 接入是独立 plan

---

## §9 实施边界

此 plan 单次 `/consume-plan tsy-zone` 应该能在一次 worktree 里吃完。预估：

- Rust 新增代码：~800-1200 行（含 tests）
- TS schema：~80 行（新文件 + index 导出）
- zones.json：~60 行（3 个 sample subzone）
- Integration test：~150 行
- Smoke script：~30 行

规模 ≈ `plan-inventory-v1` 的 30%（因为复用了 Zone 框架而非重建）。
