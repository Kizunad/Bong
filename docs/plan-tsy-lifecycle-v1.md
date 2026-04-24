# TSY 生命周期与道伥 · plan-tsy-lifecycle-v1

> 收官 TSY 核心玩法：遗物 = 骨架、取骨架 → 灵压深化、最后一件 → 塌缩 + race-out、干尸 → 道伥、道伥喷出主世界。完整端到端闭环。
> 交叉引用：`plan-tsy-v1.md`（meta）· `plan-tsy-zone-v1.md §1.3`（TsyPresence）· `plan-tsy-loot-v1.md §4`（CorpseEmbalmed）· `worldview.md §十六.一`（生命周期 5 步）· `worldview.md §十六.六`（干尸 → 道伥）· `worldview.md §十六.七`（天道静观）· `plan-npc-ai-v1.md`（archetype 基础）

---

## §-1 现状（已实装，不重做）

| 层 | 能力 | 位置 |
|----|------|------|
| TsyPresence | 玩家 TSY session 状态 + 入场 snapshot + 入口坐标 | `server/src/world/tsy.rs`（P0） |
| TSY zone 识别 | `Zone::is_tsy/tsy_layer/tsy_family_id` + `tsy_` 前缀约定 | `server/src/world/zone.rs` ext（P0） |
| 负压 drain | `tsy_drain_tick` 基于 `Zone.spirit_qi` × 池^n 抽真元 | `server/src/world/tsy_drain.rs`（P0） |
| 入场/出关传送 | `RiftPortal` component + entry/exit portal system | `server/src/world/tsy_portal.rs`（P0） |
| 入场过滤 | `apply_entry_filter` 剥离高灵质 item | `server/src/world/tsy_filter.rs`（P0） |
| 上古遗物 spawn | TSY 激活时 batch spawn → `DroppedLootRegistry.ownerless` | `server/src/inventory/tsy_loot_spawn.rs`（P1） |
| 秘境死亡分流 | `apply_tsy_death_drop` → 100%/50% 分 + 干尸 | `server/src/inventory/tsy_death_drop.rs`（P1） |
| CorpseEmbalmed | 干尸实体 component + spawn | `server/src/inventory/corpse.rs`（P1） |
| DeathEvent attacker 链路 | `attacker / attacker_player_id` 字段 | `server/src/combat/events.rs`（P1） |
| NPC archetype 框架 | `NpcArchetype` enum + brain / spawn / patrol | `server/src/npc/{brain,lifecycle,spawn,patrol}.rs` |
| ZoneRegistry dynamic add/remove | P0 已扩展 `push/remove` | `server/src/world/zone.rs`（P0） |

**本 plan 要新增**：
- TSY zone state machine（new → active → declining → collapsing → dead）
- 遗物骨架注册与 pickup 追踪
- 骨架松动：动态调整 `Zone.spirit_qi`
- 塌缩事件 + race-out（负压加倍、30 秒倒计时）
- 塌缩完成：zone 移除 + 剩余玩家 force 传送出
- `NpcArchetype::Daoxiang` variant
- 干尸激活成道伥（N tick 后转化）
- 塌缩瞬间部分道伥 spawn 到主世界

---

## §0 设计轴心

1. **塌缩由玩家行为驱动** — 最后一件遗物被取走 = 塌缩触发瞬间。**不挂外部 tick 定时**（§十六.一）
2. **遗物 = 骨架 = 负向真元池锚点** — 失去一件 → zone 结构松动 → `Zone.spirit_qi` 往负方向走（e.g. -0.4 → -0.5 → -0.7 → -1.0）（§十六.一 step 3）
3. **race-out 是 30 秒窗口** — 塌缩开始 → 负压翻倍 → 30 秒后 zone dead，剩余玩家被 force 传送出 + 真元归零可致死
4. **道伥是 NPC archetype，不是特殊玩法规则** — 复用 `NpcArchetype::Daoxiang` variant + brain tree；worldview lore 由 agent narration 赋予
5. **干尸转化有冷却** — 玩家死亡后不是立刻变道伥，而是 tick 累积到阈值（MVP = 6000 tick = 5 分钟）；塌缩发生时未到阈值的干尸也会被激活（塌缩加速）（§十六.六）
6. **塌缩时道伥 "喷出"**：塌缩瞬间，zone 内所有已激活的道伥 **50% 概率** 被传送到主世界裂缝附近；其余随 zone 一起消失（§十六.六）
7. **死坍缩渊不可重进** — zone dead 后从 registry 移除；即便有玩家残留其内，也被 kick 到 entry_portal_pos。之后该 family_id 永久标记为 dead，不可重生同一 family
8. **新 TSY 生成留给后续 plan** — 本 plan 不做自动生成，只处理一个已存在 TSY 的完整生命周期

---

## §1 TSY Zone 状态机

### 1.1 状态定义

```
      [New]                                     ◀ Zone 刚被 registry.push 进来，但还没人发现
        │ first player enters
        ↓
      [Active]     ←── 大部分时间处于这里
        │ skeleton_count < initial_count × 0.5（超过一半遗物被取走）
        ↓
     [Declining]   ←── 骨架已明显松动，深层更致命
        │ last skeleton taken
        ↓
    [Collapsing]   ←── 塌缩窗口（30 秒 race-out）
        │ 30 sec timer expires
        ↓
      [Dead]       ←── zone 从 registry 移除，family_id 永久作废
```

### 1.2 Resource：TsyZoneStateRegistry

**位置**：`server/src/world/tsy_lifecycle.rs`（新建）

```rust
use std::collections::HashMap;

#[derive(Resource, Default, Debug)]
pub struct TsyZoneStateRegistry {
    pub by_family: HashMap<String, TsyZoneState>,
}

#[derive(Debug, Clone)]
pub struct TsyZoneState {
    pub family_id: String,
    pub lifecycle: TsyLifecycle,
    pub source_class: AncientRelicSource,       // 来源（大能/宗门/战场）
    pub initial_skeleton: Vec<u64>,              // 初始骨架 instance_ids
    pub remaining_skeleton: HashSet<u64>,        // 还在 zone 里的骨架 instance_ids
    pub created_at_tick: u64,
    pub activated_at_tick: Option<u64>,          // 首次有玩家进入
    pub collapsing_started_at_tick: Option<u64>,
    pub dead_at_tick: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TsyLifecycle {
    New,
    Active,
    Declining,
    Collapsing,
    Dead,
}
```

### 1.3 状态转换触发条件

| 从 | 到 | 触发条件 | 副作用 |
|----|----|---------|--------|
| New | Active | 首次有玩家 TsyPresence 绑到此 family | 记 `activated_at_tick` |
| Active | Declining | `remaining_skeleton.len() < initial_skeleton.len() / 2` | 更新所有 subzone 的 `spirit_qi`（见 §2） |
| Declining | Collapsing | `remaining_skeleton.is_empty()` | 记 `collapsing_started_at_tick`，发 `TsyCollapseStarted` event |
| Collapsing | Dead | `collapsing_started_at_tick + 30 * 20 <= now` | 从 `ZoneRegistry` 移除三个 subzone，force 传送剩余玩家，发 `TsyCollapseCompleted` event |
| 任何 | Dead | （强制：管理员 kill 命令） | 同上 |

### 1.4 Tick system

**位置**：`server/src/world/tsy_lifecycle.rs`

```rust
pub fn tsy_lifecycle_tick(
    mut state_reg: ResMut<TsyZoneStateRegistry>,
    zones: Res<ZoneRegistry>,
    mut commands: Commands,
    presence_q: Query<(Entity, &TsyPresence)>,
    registry: Res<DroppedLootRegistry>,  // 用来检测 ownerless 里还剩多少 Ancient rarity 的遗物
    tick: Res<ServerTick>,
    mut emit_started: EventWriter<TsyCollapseStarted>,
    mut emit_completed: EventWriter<TsyCollapseCompleted>,
) {
    for state in state_reg.by_family.values_mut() {
        match state.lifecycle {
            TsyLifecycle::New => {
                if presence_q.iter().any(|(_, p)| p.family_id == state.family_id) {
                    state.lifecycle = TsyLifecycle::Active;
                    state.activated_at_tick = Some(tick.0);
                }
            }
            TsyLifecycle::Active | TsyLifecycle::Declining => {
                // 更新 remaining_skeleton：扫 DroppedLootRegistry.ownerless 看哪些原 skeleton ids 还在
                let remaining: HashSet<u64> = state.initial_skeleton.iter()
                    .filter(|id| registry.ownerless.iter().any(|e| e.instance_id == **id))
                    .copied()
                    .collect();
                state.remaining_skeleton = remaining;

                if state.remaining_skeleton.is_empty() {
                    state.lifecycle = TsyLifecycle::Collapsing;
                    state.collapsing_started_at_tick = Some(tick.0);
                    emit_started.send(TsyCollapseStarted { family_id: state.family_id.clone() });
                } else if state.remaining_skeleton.len() < state.initial_skeleton.len() / 2 {
                    if state.lifecycle == TsyLifecycle::Active {
                        state.lifecycle = TsyLifecycle::Declining;
                        // Trigger §2.2 灵压深化
                    }
                }
            }
            TsyLifecycle::Collapsing => {
                let elapsed = tick.0.saturating_sub(state.collapsing_started_at_tick.unwrap_or(0));
                if elapsed >= COLLAPSE_DURATION_TICKS {
                    state.lifecycle = TsyLifecycle::Dead;
                    state.dead_at_tick = Some(tick.0);
                    emit_completed.send(TsyCollapseCompleted { family_id: state.family_id.clone() });
                }
            }
            TsyLifecycle::Dead => {
                // no-op；由另一个 system 清理
            }
        }
    }
}

pub const COLLAPSE_DURATION_TICKS: u64 = 30 * 20;  // 30 秒 * 20 tick/s
```

### 1.5 TSY 激活事件（本 plan 新增 + 衔接 P1）

P1 的 `tsy_loot_spawn_on_zone_activate` 读 `TsyZoneActivated` event。本 plan 明确这个 event 的 emitter：

**位置**：`server/src/world/tsy_lifecycle.rs`

```rust
#[derive(Event)]
pub struct TsyZoneActivated {
    pub family_id: String,
    pub source_class: AncientRelicSource,
    pub family_id_shallow: String,  // e.g. "tsy_lingxu_01_shallow" — P1 loot spawn 用
}

/// 对外 API：注册一个新 TSY family 到 lifecycle system
pub fn register_new_tsy(
    state_reg: &mut TsyZoneStateRegistry,
    family_id: String,
    source_class: AncientRelicSource,
    tick: u64,
    mut emit: EventWriter<TsyZoneActivated>,
) {
    state_reg.by_family.insert(family_id.clone(), TsyZoneState {
        family_id: family_id.clone(),
        lifecycle: TsyLifecycle::New,
        source_class,
        initial_skeleton: Vec::new(),     // 由 P1 的 tsy_loot_spawn 填充
        remaining_skeleton: HashSet::new(),
        created_at_tick: tick,
        activated_at_tick: None,
        collapsing_started_at_tick: None,
        dead_at_tick: None,
    });
    emit.send(TsyZoneActivated {
        family_id: family_id.clone(),
        source_class,
        family_id_shallow: format!("{}_shallow", family_id),
    });
}
```

P1 的 `tsy_loot_spawn_on_zone_activate` 在 spawn 完遗物后，回写 `initial_skeleton`：

```rust
// 在 P1 plan-tsy-loot-v1.md §2.2 的末尾（需 merge 后补上）
state_reg.by_family.get_mut(&family_id).unwrap().initial_skeleton = spawned_instance_ids;
```

---

## §2 骨架松动：动态灵压深化

### 2.1 松动曲线

| 剩余骨架比例 | shallow 灵压 | mid 灵压 | deep 灵压 |
|-------------|-------------|---------|----------|
| 100%（初始） | -0.3 | -0.6 | -0.9 |
| 75% | -0.35 | -0.7 | -1.0 |
| 50%（Declining 开始） | -0.45 | -0.8 | -1.05 |
| 25% | -0.5 | -0.85 | -1.1 |
| 0%（Collapsing） | -0.6（× 2 = -1.2 race-out） | -0.95（× 2 = -1.9） | -1.15（× 2 = -2.3） |

**公式**：

```rust
fn compute_layer_spirit_qi(
    layer: TsyLayer,
    skeleton_ratio: f64,    // 0.0 ~ 1.0
    is_collapsing: bool,
) -> f64 {
    let base = match layer {
        TsyLayer::Shallow => -0.3,
        TsyLayer::Mid => -0.6,
        TsyLayer::Deep => -0.9,
    };
    let depth_factor = match layer {
        TsyLayer::Shallow => -0.3,
        TsyLayer::Mid => -0.4,
        TsyLayer::Deep => -0.3,
    };
    let after_decay = base + depth_factor * (1.0 - skeleton_ratio);
    if is_collapsing {
        after_decay * 2.0  // race-out: 双倍
    } else {
        after_decay
    }
}
```

### 2.2 应用到 Zone.spirit_qi

**位置**：`server/src/world/tsy_lifecycle.rs`

```rust
pub fn tsy_lifecycle_apply_spirit_qi(
    state_reg: Res<TsyZoneStateRegistry>,
    mut zones: ResMut<ZoneRegistry>,
) {
    for state in state_reg.by_family.values() {
        if state.lifecycle == TsyLifecycle::Dead { continue; }
        let ratio = if state.initial_skeleton.is_empty() { 1.0 }
                    else { state.remaining_skeleton.len() as f64 / state.initial_skeleton.len() as f64 };
        let is_collapsing = state.lifecycle == TsyLifecycle::Collapsing;

        for layer in [TsyLayer::Shallow, TsyLayer::Mid, TsyLayer::Deep] {
            let suffix = match layer {
                TsyLayer::Shallow => "_shallow",
                TsyLayer::Mid => "_mid",
                TsyLayer::Deep => "_deep",
            };
            let zone_name = format!("{}{}", state.family_id, suffix);
            if let Some(zone) = zones.zones.iter_mut().find(|z| z.name == zone_name) {
                zone.spirit_qi = compute_layer_spirit_qi(layer, ratio, is_collapsing);
            }
        }
    }
}
```

每 tick 跑一次（tick 频率可以 downsample 到每秒一次），将计算后的 spirit_qi 写回 Zone struct。P0 的 `tsy_drain_tick` 读 `Zone.spirit_qi` 得到抽取速率，自然反映骨架松动。

### 2.3 Tick 依赖序（含 P0/P1 已有）

```
FixedUpdate:
  combat_resolve
  ↓ after
  tsy_loot_spawn_on_zone_activate     (P1 - spawn skeleton)
  ↓ after
  tsy_lifecycle_tick                  (P2 - 状态机，更新 remaining_skeleton)
  ↓ after
  tsy_lifecycle_apply_spirit_qi       (P2 - 写回 Zone.spirit_qi)
  ↓ after
  tsy_drain_tick                      (P0 - 读 Zone.spirit_qi 抽真元)
  ↓ after
  lifecycle_death_process
  ↓ after
  tsy_corpse_to_daoxiang_tick         (P2 - 干尸转化，见 §4)
  ↓ after
  tsy_collapse_completed_cleanup      (P2 - 处理 Dead 状态，见 §3)
```

---

## §3 塌缩事件：Collapsing / Dead 处理

### 3.1 TsyCollapseStarted 事件

**发出点**：`tsy_lifecycle_tick` 从 Declining → Collapsing

**消费者**：

- `agent` 端监听 → narration "某 TSY 进入塌缩，30 秒后关闭"（世界频道广播或仅该 zone 内玩家收到）
- 所有在该 family_id 内的玩家 → client-side UI 倒计时（"塌缩剩余 30 秒"）
- `tsy_lifecycle_apply_spirit_qi` 的下一个 tick 立刻把 spirit_qi × 2

### 3.2 Race-out 体感

玩家在 Collapsing 状态下：

- 真元抽速翻倍（shallow -0.3 → -0.6，引气玩家每秒被抽 1.56 点 → 3.12 点）
- HUD 显示"塌缩倒计时 29... 28... "（client 自行倒数，不依赖 server push）
- 所有 TSY 内 mob（包括已激活的道伥）**停止攻击玩家**（它们也在逃？设计决策：**不停止**，反而更激进，提高 race-out 难度）

### 3.3 Dead 完成的清理

**位置**：`server/src/world/tsy_lifecycle.rs`

```rust
pub fn tsy_collapse_completed_cleanup(
    mut commands: Commands,
    mut emit: EventReader<TsyCollapseCompleted>,
    mut zones: ResMut<ZoneRegistry>,
    mut state_reg: ResMut<TsyZoneStateRegistry>,
    presence_q: Query<(Entity, &TsyPresence, &PlayerState)>,
    mut player_states: Query<&mut PlayerState>,
    positions_q: Query<&Position>,
    mut loot_registry: ResMut<DroppedLootRegistry>,
    mut daoxiang_spawn: EventWriter<DaoxiangEjectSpawn>,
) {
    for ev in emit.read() {
        // Step 1: 处理所有还在 zone 内的玩家
        for (entity, presence, _) in &presence_q {
            if presence.family_id == ev.family_id {
                // Force 传送回 entry_portal_pos，真元可能已 ≤ 0 → 死亡流水线
                commands.entity(entity).insert(Position(presence.entry_portal_pos));
                commands.entity(entity).remove::<TsyPresence>();
                // 如果真元还 > 0，虽然出来了但半死状态
                // 如果真元 ≤ 0，下一 tick DeathEvent 正常发出（cause="tsy_drain"）
            }
        }

        // Step 2: 处理 zone 内剩余的 ownerless loot（凡物）→ 随 zone 消失
        //         上古遗物 skeleton 此时已经全被取走（ownerless 中不存在）
        //         留在 zone 内的是玩家死亡留下的凡物和秘境所得
        let family_zones: Vec<String> = vec![
            format!("{}_shallow", ev.family_id),
            format!("{}_mid", ev.family_id),
            format!("{}_deep", ev.family_id),
        ];
        let aabbs: Vec<(DVec3, DVec3)> = family_zones.iter()
            .filter_map(|n| zones.find_zone_by_name(n).map(|z| z.bounds))
            .collect();

        loot_registry.ownerless.retain(|entry| {
            let pos = DVec3::from(entry.world_pos);
            !aabbs.iter().any(|(min, max)|
                pos.x >= min.x && pos.x <= max.x &&
                pos.y >= min.y && pos.y <= max.y &&
                pos.z >= min.z && pos.z <= max.z
            )
        });
        // by_owner 同理

        // Step 3: 处理 zone 内的干尸 → 激活成道伥，50% 喷出
        // 见 §4

        // Step 4: 从 ZoneRegistry 移除三个 subzone
        for zone_name in &family_zones {
            zones.remove_by_name(zone_name);
        }

        // Step 5: 标记 TsyZoneState 为 Dead（留记录；不要 remove，方便其他系统查询）
        if let Some(s) = state_reg.by_family.get_mut(&ev.family_id) {
            s.lifecycle = TsyLifecycle::Dead;
        }
    }
}
```

### 3.4 Dead 状态的永久性

Dead family_id 永远留在 `TsyZoneStateRegistry.by_family` 里（MVP 不清理 — 足够低的内存占用；即使 10 年后这个 server 有几千个 dead family，几百 KB）。用途：

- 避免 `/tsy-spawn <dead_family_id>` 重复创建同 family
- agent narration 可以查"此地曾经有何秘境"
- 未来"亡者博物馆"（library-web）可以展示过去的秘境历史

---

## §4 道伥：NpcArchetype::Daoxiang

### 4.1 Archetype 定义

**位置**：`server/src/npc/mod.rs` 或 `server/src/npc/archetype.rs`（依现有结构）

**新增 variant**：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NpcArchetype {
    // ... existing variants ...
    Daoxiang,  // ← 新增
}
```

**属性表**：

- base_health: 80（略低于引气玩家，方便杀，但模仿难辨认）
- base_speed: 0.7（偏慢）
- realm_tier: 引气 3 ~ 凝脉 2（具体看 "前主" — 死时境界）
- aggressive_threshold: 当玩家真元 < 20% 或 被从背后接近时转入 aggressive
- disguise_actions: 砍树 / 挖矿 / 蹲伏（mimic behavior）

### 4.2 Brain tree 分支

**位置**：`server/src/npc/brain.rs`（扩展）

```rust
// 伪代码 —— 具体接入方式看现有 big-brain Scorer/Action 模式
pub fn register_daoxiang_brain(app: &mut App) {
    // Scorer:
    //   DisguiseScorer — 玩家距离 > 15 → 高分 → 进 Disguise action
    //   AggroScorer — 玩家距离 < 5 或 背对 或 真元<20% → 高分
    //
    // Action:
    //   DisguiseAction — 播放砍树/挖矿动画，不动
    //   AggroAction — 瞬间突进 + 上古术法（定义为 ranged dash + high-damage slash）
}
```

### 4.3 Spawn 入口

**位置**：`server/src/npc/spawn.rs`（扩展）

```rust
/// 将一个 CorpseEmbalmed 激活成道伥
pub fn spawn_daoxiang_from_corpse(
    commands: &mut Commands,
    corpse: &CorpseEmbalmed,
    pos: DVec3,
    tick: u64,
) -> Entity {
    commands.spawn((
        NpcArchetype::Daoxiang,
        Position(pos),
        NpcState { ... },
        DaoxiangOrigin {
            from_corpse: corpse.original_player_id,
            from_family: corpse.family_id.clone(),
            activated_at_tick: tick,
        },
        // big-brain thinker with Daoxiang scorers
    )).id()
}
```

### 4.4 DaoxiangOrigin component

```rust
#[derive(Component, Debug)]
pub struct DaoxiangOrigin {
    pub from_corpse: Uuid,        // 原主角色 ID
    pub from_family: String,       // TSY family
    pub activated_at_tick: u64,
}
```

用途：

- inspect 能看到 "这是某某的遗骸"
- 生平卷记录："某某死后其遗骸被激活为道伥，游荡于世"（agent 写 narration）
- 击杀道伥时查这个 component 判定"玩家击杀前同伴的道伥"的特殊事件

---

## §5 干尸 → 道伥 转化

### 5.1 触发条件

两种路径：

1. **自然累积**：CorpseEmbalmed 存在 ≥ `DAOXIANG_NATURAL_TICKS`（MVP = 6000 tick = 5 分钟）
2. **塌缩加速**：zone 进入 Collapsing 时，该 zone 内所有 CorpseEmbalmed 立即激活

```rust
pub const DAOXIANG_NATURAL_TICKS: u64 = 6000;
```

### 5.2 Tick system

**位置**：`server/src/world/tsy_lifecycle.rs`

```rust
pub fn tsy_corpse_to_daoxiang_tick(
    mut commands: Commands,
    corpse_q: Query<(Entity, &Position, &CorpseEmbalmed)>,
    state_reg: Res<TsyZoneStateRegistry>,
    tick: Res<ServerTick>,
) {
    for (entity, pos, corpse) in &corpse_q {
        if corpse.activated_to_daoxiang { continue; }

        let family_state = state_reg.by_family.get(&corpse.family_id);

        let should_activate = match family_state {
            Some(s) if s.lifecycle == TsyLifecycle::Collapsing => true,  // 塌缩加速
            _ => tick.0.saturating_sub(corpse.died_at_tick) >= DAOXIANG_NATURAL_TICKS,
        };

        if should_activate {
            // 1. Spawn 道伥 at corpse pos
            spawn_daoxiang_from_corpse(&mut commands, corpse, pos.0, tick.0);
            // 2. 标记 corpse.activated_to_daoxiang = true（或 despawn corpse entity）
            commands.entity(entity).despawn();
        }
    }
}
```

### 5.3 一具干尸只激活一次

`CorpseEmbalmed.activated_to_daoxiang` 标记幂等性；或者直接 despawn corpse（简单）。

### 5.4 道伥的 loot 继承

道伥被击杀时，应该 drop 原干尸里的物品（而非凭空生成）。实现：

```rust
pub fn daoxiang_death_drops(
    origin: &DaoxiangOrigin,
    registry: &mut DroppedLootRegistry,
    pos: DVec3,
) {
    // 从 registry.by_owner 里找 corpse 原 player 的遗物（通过 from_corpse uuid 反查 entity）
    // 把它们转移到 ownerless
    // （具体看 player_id ↔ entity 的映射机制）
}
```

**注**：MVP 可简化 — 道伥死亡随机生成一件破旧凡物（"锈剑"、"残页"）+ 偶尔残卷。完整 loot 继承推到 P3。

---

## §6 塌缩时的道伥 "喷出"

按 `worldview §十六.六`："一部分在坍缩渊塌缩的瞬间被挤出到主世界"。

### 6.1 规则

塌缩完成（Collapsing → Dead）瞬间：

- zone 内所有道伥（已激活的 + 刚被塌缩激活的）—— 50% 概率**传送到主世界裂缝附近**（±10 格随机偏移）
- 另外 50% 随 zone 消失

### 6.2 实现

**位置**：`server/src/world/tsy_lifecycle.rs` `tsy_collapse_completed_cleanup` 的 Step 3

```rust
// Step 3 (expanded): 处理道伥喷出
let family_zones_aabbs: Vec<(DVec3, DVec3)> = /* as before */;
let family_id = &ev.family_id;

// 获取入口 portal 坐标
let entry_pos = state_reg.by_family.get(family_id)
    .and_then(|_| /* 查 RiftPortal 的 position */)
    .unwrap_or(DVec3::ZERO);

// 扫所有 Daoxiang + DaoxiangOrigin 位于 family_zones 内的
let mut rng = /* deterministic rng from tick */;
for (daoxiang_entity, daoxiang_pos) in daoxiang_q.iter() {
    if is_in_any_aabb(daoxiang_pos.0, &family_zones_aabbs) {
        if rng.gen_bool(0.5) {
            // 50% 喷出：传送到主世界裂缝附近
            let offset = DVec3::new(
                rng.gen_range(-10.0..10.0), 0.0, rng.gen_range(-10.0..10.0)
            );
            commands.entity(daoxiang_entity).insert(Position(entry_pos + offset));
        } else {
            // 50% 随 zone 消失
            commands.entity(daoxiang_entity).despawn();
        }
    }
}
```

---

## §7 测试策略

### 7.1 Rust unit tests

**位置**：各新增文件的 `#[cfg(test)]` 模块

- [ ] `tsy_lifecycle.rs`:
  - 状态机转换：New → Active → Declining → Collapsing → Dead（各触发条件）
  - `compute_layer_spirit_qi` 各 ratio + collapsing flag 组合
  - DAOXIANG_NATURAL_TICKS 自然转化
  - Collapsing 加速激活所有 corpse
- [ ] NPC archetype 注册：Daoxiang variant serde round-trip
- [ ] `DaoxiangOrigin` 字段 round-trip

**最少 15 tests**。

### 7.2 集成测试

**位置**：`server/tests/tsy_lifecycle_integration.rs`（新建）

- [ ] Happy path：
  - 注册一个 TSY family 5 件骨架 → Active
  - 取走 3 件 → 比例 40% < 50% → Declining → Zone.spirit_qi 加深
  - 取走最后 2 件 → Collapsing → spirit_qi 翻倍
  - 30 秒 tick 推进 → Dead → zone 从 registry 移除
  - 期间有 1 个玩家残留 → 传送出
- [ ] 干尸转化：
  - 玩家在 TSY 内死亡 → CorpseEmbalmed spawn
  - tick 推进 6000 → 道伥 spawn + corpse despawn
- [ ] 塌缩加速：
  - 玩家死亡 → CorpseEmbalmed at 100 tick
  - TSY 其他遗物全被取走 → Collapsing
  - 期望：CorpseEmbalmed 立刻激活成道伥（不等 6000 tick）
- [ ] 道伥喷出：
  - 5 个道伥在 TSY 内
  - 塌缩完成 → 期望 2-3 个被传送到主世界（50% ± 噪音），其余 despawn

### 7.3 NPC AI smoke

- [ ] 道伥 entity spawn 后，server log 显示 brain 注册成功
- [ ] 玩家接近 ≥ 15 格 → 道伥 "disguise" 播放
- [ ] 玩家接近 ≤ 5 格或背对 → 道伥 "aggressive" 突进

### 7.4 Schema test

- [ ] `TsyCollapseStartedV1` + `TsyCollapseCompletedV1` round-trip
- [ ] `DaoxiangSpawnedV1` round-trip

---

## §8 验收标准

### Automated

执行 `bash scripts/smoke-tsy-lifecycle.sh`（新建）：

- [ ] `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
- [ ] `cd agent/packages/schema && npm test`
- [ ] Integration tests 通过

### Manual QA

**前置**：P0 + P1 已 merged；`/tsy-spawn tsy_lingxu_01` 放一个带 5 件遗物的 TSY；手动 give 玩家远超所需的凡物让他能长期驻留

- [ ] **A. 状态机 happy path**
  - [ ] 进 TSY → server log `TsyLifecycle: New → Active`
  - [ ] 捡走 2 件遗物 → 40% 剩余 → `Active → Declining`
  - [ ] Zone.spirit_qi 变深（查 /zone 命令或 debug UI）
  - [ ] 真元抽速感知到加快
  - [ ] 捡走最后 3 件 → `Declining → Collapsing`
  - [ ] HUD 倒计时显示（若 P3 HUD 未做，查 server log 即可）
  - [ ] 负压明显翻倍
  - [ ] 30 秒到 → `Collapsing → Dead`
  - [ ] TSY zone 从 registry 消失
  - [ ] 玩家被传送回裂缝外坐标

- [ ] **B. 干尸 → 道伥（自然）**
  - [ ] 玩家 A 进 TSY → 故意死亡（真元归零 / /kill）
  - [ ] CorpseEmbalmed entity 可见（MVP：静止 zombie skin）
  - [ ] 等 5 分钟（手动推 tick，或使用 `/tsy-dbg advance-tick 6000`）
  - [ ] corpse 消失 → 原位置 spawn 道伥
  - [ ] 玩家 B 进 TSY → 靠近 → 道伥 disguise 行为（砍空气 / 蹲下）
  - [ ] 背对道伥 → 道伥 aggressive 突进
  - [ ] 击杀道伥 → 掉一件破凡物（MVP 简化版 loot）

- [ ] **C. 塌缩加速激活**
  - [ ] 玩家 A 死在 TSY → corpse 还没到 5 分钟
  - [ ] 玩家 B 进入 → 取走所有骨架 → Collapsing
  - [ ] corpse 立刻激活成道伥（不等自然 tick）
  - [ ] B 面对 race-out + 道伥围攻

- [ ] **D. 道伥喷出主世界**
  - [ ] 设置 TSY 里有 3-4 个已激活道伥
  - [ ] 塌缩完成 → 期望 1-2 个出现在主世界裂缝附近
  - [ ] 玩家回到裂缝位置能看到游荡的道伥

- [ ] **E. Dead family 不可重用**
  - [ ] TSY tsy_lingxu_01 Dead
  - [ ] 再次 `/tsy-spawn tsy_lingxu_01` → 报错 "family dead"
  - [ ] 需换新 family_id：`/tsy-spawn tsy_lingxu_02`

- [ ] **F. 集成 P0/P1**
  - [ ] 负压抽真元（P0）在 lifecycle 深化后加速
  - [ ] 秘境死亡分流（P1）在 lifecycle 任何阶段都正确
  - [ ] 入场过滤（P0）不受 lifecycle 状态影响
  - [ ] Collapsing 状态下还能进？MVP 禁止：rift portal 在 Collapsing 阶段拒绝传送（返回玩家原坐标 + 聊天栏提示"裂缝正在塌缩，无法进入"）

### Acceptance bar

- [ ] 所有自动化 test 通过
- [ ] Manual QA A-F 全绿
- [ ] Full E2E 复现 `plan-tsy-v1.md §6` 的 12 步流程

---

## §9 风险 / 未决

| 风险 | 级别 | 缓解 |
|------|------|------|
| Lifecycle tick 频率过高导致 `find_zone_by_name` 性能退化 | 中 | Downsample 到每秒 1 次；Zone lookup cache |
| Collapsing 30 秒窗口被 exploit（玩家频繁来回 zone 边界） | 中 | 一旦 Collapsing，`rift_entry_portal` 拒绝进入；离开玩家被强制传送 |
| 道伥 brain tree 初次接入 big-brain 可能遇到兼容问题 | 高 | MVP 先用简化 behavior（距离+timer），不强依赖 big-brain 的 Scorer/Action 复杂模式 |
| TSY family Dead 状态占用内存无限增长 | 低 | v2 加"归档到 sqlite，从内存清"（本 plan 不做） |
| `remaining_skeleton` 查询基于 `DroppedLootRegistry.ownerless` 扫描 O(N) | 中 | 如果 registry 很大会慢；用 `HashMap<instance_id, Entry>` 反查缓存 |
| 玩家在 Collapsing 中死亡 → dropped loot 会随 zone 消失 | 中 | 这是 **设计行为**（§十六.六"可能随坍缩渊塌缩而化灰"），明确在 manual QA 验证 |
| 道伥 AI 和 NPC AI 框架冲突 | 中 | P2 先做最小可用的 archetype，深度整合推 P3 |
| tick 推进在测试中难以模拟 | 中 | 加 `/tsy-dbg advance-tick <N>` debug 命令；integration test 用 `ServerTick` resource 直接改 |
| 塌缩事件未持久化 — server 重启会丢状态 | 中 | MVP 不管（TSY 都是临时的）；worldgen 上线后需要考虑持久化活 TSY |

### 未决设计

- **道伥出 TSY 后的去向**：在主世界游荡多久？会不会无限增长？MVP：设 despawn 超时（现实 24 小时 server 时间）；长期 balance 推 P3
- **多玩家同时触发最后一件的归属**：race condition —— 两个玩家同时取最后一件（一件物品，先到先得，另一个没拿到）→ 谁拿到谁触发塌缩，另一个目前无伤。无特殊规则
- **跨 TSY 的骨架"漂流"**：如果玩家拿着骨架遗物走出一个 TSY、进另一个 TSY → 是否影响两边 lifecycle？**不影响**：骨架注册是"zone 初始 spawn 时的 instance_id 集合"，其他 zone 不知道这些 id 的存在
- **塌缩时机 —— agent 是否能预警**：agent 是否能在 Collapsing 前 narration 提前警告？本 plan 不处理，agent narration 接入独立做

---

## §10 依赖与文件清单

### 前置依赖

- ✅ `plan-tsy-zone-v1.md` merged
- ✅ `plan-tsy-loot-v1.md` merged

### 本 plan 新增/修改文件

**Rust**（server）：

- `server/src/world/tsy_lifecycle.rs`（新建，~500 行）
  - `TsyZoneStateRegistry` / `TsyZoneState` / `TsyLifecycle`
  - `tsy_lifecycle_tick`
  - `tsy_lifecycle_apply_spirit_qi`
  - `tsy_collapse_completed_cleanup`
  - `tsy_corpse_to_daoxiang_tick`
  - `register_new_tsy` API
- `server/src/world/mod.rs`（修改：注册新 system）
- `server/src/world/tsy_portal.rs`（修改：Collapsing 状态拒绝入场）
- `server/src/inventory/tsy_loot_spawn.rs`（修改：回写 `initial_skeleton`）
- `server/src/npc/archetype.rs` 或 `mod.rs`（修改：加 `Daoxiang` variant）
- `server/src/npc/brain.rs`（修改：注册 daoxiang brain）
- `server/src/npc/spawn.rs`（修改：加 `spawn_daoxiang_from_corpse`）
- `server/src/npc/daoxiang.rs`（新建，~150 行：`DaoxiangOrigin` + brain actions）
- `server/tests/tsy_lifecycle_integration.rs`（新建，~400 行）

**TS**（schema）：

- `agent/packages/schema/src/tsy.ts`（修改：加 `TsyCollapseStartedV1` + `TsyCollapseCompletedV1` + `DaoxiangSpawnedV1`）

**脚本**：

- `scripts/smoke-tsy-lifecycle.sh`（新建，~40 行）

### 规模估算

- Rust 新增 / 修改：~1800-2200 行
- TS schema：~80 行
- 测试：~500 行
- 总计：**~2500 行**，可能是三个 plan 里最大的；单次 /consume-plan 需要留充足时间，考虑一下拆

### 拆分备选方案

如果本 plan 单次消费过大，可拆：

- `plan-tsy-lifecycle-v1-part-a.md`：状态机 + 骨架松动 + 塌缩（§1-§3）
- `plan-tsy-lifecycle-v1-part-b.md`：道伥 + 干尸转化 + 喷出（§4-§6）

MVP 建议不拆，整吃一次。

---

## §11 实施边界

此 plan 的 `/consume-plan` 预期：

- Rust 新增代码：~1800-2200 行（含 tests）
- 测试：~500 行
- 脚本：~40 行

单次 worktree 吃完压力较大，但不超过 `plan-combat-no_ui` 的规模（2973 行 plan spec）。

**建议实施节奏**：
- Day 1-2：§1 状态机 + §2 骨架松动
- Day 3：§3 塌缩事件 + cleanup
- Day 4：§4 道伥 archetype
- Day 5：§5 干尸转化 + §6 喷出
- Day 6：测试 + E2E QA

---

## §12 后续

本 plan 完成后，TSY 系列**核心玩法闭环**。v1 归档。

后续独立 plan（不在本系列）：

- `plan-tsy-polish-v1.md` — 浪潮 PVE / 入口感知 HUD / inspect 特效 / 封灵匣 / 耐久 tooltip
- `plan-tsy-worldgen-v1.md` — 自动生成活 TSY（宗门遗迹浮现 / 高手陨落触发 / 天道 narrative 接入）
- `plan-tsy-persistence-v1.md` — TSY 生命周期持久化 + "亡者博物馆"中展示过往 TSY 的历史
- `plan-tsy-agent-narrative-v1.md` — 天道 agent 对 TSY 事件的叙事（塌缩倒计时 / 道伥出没 / 新 TSY 出现）
- `plan-daoxiang-ecology-v1.md` — 道伥在主世界的生态（长期 balance、击杀奖励、与其他 NPC 的互动）

所有后续 plan 复用 `plan-tsy-v1.md` meta 里的术语表和设计轴心，避免设定漂移。
