# TSY 容器与搜刮 · plan-tsy-container-v1

> 坍缩渊内的 loot 不是地面散布，而是**被抽干的遗骸 + 保存结构的腔体**构成的容器分层（5 档）。本 plan 落地 `LootContainer` / `SearchProgress` / `ContainerKey` 三大 Component，配合搜刮倒计时、搜刮时真元消耗 1.5× 加速、钥匙单次消耗。P3 在 P0/P1/P2 就位后开工。
> 交叉引用：`plan-tsy-v1.md §0/§2`（公理 / 横切）· `plan-tsy-dimension-v1`（位面基础设施前置）· `plan-tsy-zone-v1.md §1.3`（TsyPresence）· `plan-tsy-loot-v1.md §1-§3`（AncientRelicTemplate / DroppedLootEntry.ownerless）· `plan-tsy-lifecycle-v1.md §2`（TsyZoneState / 骨架计数）· `worldview.md §十六.三 容器与搜刮`（含 5 档分层表）· `worldview.md §十六.一`（传说档 = 骨架）

> **2026-04-24 架构反转备忘**：TSY 实现为独立位面。本 plan 所有"zone AABB 内撒点" / `find_zone` 调用均在 **TSY dim 内部坐标系**下工作（不是主世界坐标），且 spawn 出来的 container 实体要挂到 TSY layer（`plan-tsy-dimension-v1 §1 DimensionLayers.tsy`）。`TsyPresence` 字段名 `entry_portal_pos` 已变为 `return_to: DimensionAnchor`（本 plan 不直接读该字段，仅以 Component 存在作 marker 使用）。

---

## §-1 现状（已实装 / 上游 plan 已锁，本 plan 不重做）

| 层 | 能力 | 位置 |
|----|------|------|
| `ItemInstance` | `template_id` / `rarity` / `durability` / `spirit_quality` / `stack_count` / `freshness` | `server/src/inventory/mod.rs:134-150` |
| `DroppedLootRegistry` | `by_owner: HashMap<Entity, Vec<DroppedLootEntry>>` | `server/src/inventory/mod.rs:1103-1106` |
| `DroppedLootEntry` | `instance_id` / `source_container_id` / `world_pos` / `item` | `server/src/inventory/mod.rs:1093-1101` |
| `PlayerInventory` | `containers` / `equipped` / `hotbar` / `bone_coins` | `server/src/inventory/mod.rs:192-200` |
| `ItemRarity::Ancient` | 上古遗物 rarity + 低耐久约束 | P1 plan §1.1（待实装时落地） |
| `AncientRelicTemplate` | 上古遗物模板池 | P1 plan §2.1（待实装时落地） |
| `DroppedLootRegistry.ownerless` | 世界自然 loot 容器（非玩家 drop） | P1 plan §3（待实装时落地） |
| `TsyPresence` | `family_id` / `entered_at_tick` / `return_to: DimensionAnchor` | P0 plan §1.3 |
| Zone TSY helpers | `is_tsy()` / `tsy_layer()` / `tsy_family_id()` | P0 plan §1.2 |
| `CombatState` / `Wounds` | 中断条件的读侧 | `server/src/combat/components.rs` |
| `Cultivation.spirit_qi` | 真元池（搜刮时加速抽取的对象） | `server/src/cultivation/components.rs` |
| `DeathEvent.attacker_player_id` | PVP 掠夺追溯字段 | P1 plan §2（横切） |

**本 plan 要新增**：`LootContainer` Component（Entity-scoped）+ `ContainerKind` 5 档 enum + `SearchProgress` Component + `ContainerKey` 物品模板族 + `loot_pools.json` 配置文件 + 容器 spawn rule（按 TSY 起源 × 层深）+ 搜刮 tick system + 真元加速 1.5× hook + 钥匙消耗 + 完成事件 `RelicExtracted` 对接 P2 lifecycle + IPC schema `container-interaction-v1` + 客户端搜刮 HUD。

---

## §0 设计轴心（不可违反）

1. **容器是唯一 loot 载体** — TSY 内地面不刷散装物资，**一切 loot 必须从容器搜出**。这让"搜刮风险暴露"成为强制时间成本（§十六.三 容器与搜刮 末段）
2. **5 档容器分层** — 普通（干尸 / 骨架）/ 罕见（储物袋残骸）/ 史诗（石匣 / 玉棺）/ 传说（法阵核心），搜刮时长 3-40 秒（§十六.三 表格）
3. **搜刮期间真元消耗 × 1.5** — 搜刮是主动暴露行为，抽吸速率在 P0 基线上乘 1.5。这让"深度搜刮永远伴随决定：守在旁边的修士快逼近了，要不要放弃半搜完的石匣？"（§十六.三）
4. **中断即归零** — 移动 / 攻击 / 受击 任一条件 → `SearchProgress` 清零。不保留进度、不支持续搜。鼓励**选对时机再开搜刮**的策略
5. **钥匙单次消耗** — 史诗以上容器需钥匙，钥匙在**搜刮完成**时消耗（打断不浪费）；一把钥匙只对应一类容器，不通用（§十六.三 钥匙/令牌）
6. **传说档容器即骨架** — `ContainerKind::RelicCore` 搜空 → 发 `RelicExtracted(family_id)` 事件 → P2 lifecycle 接收 → `TsyZoneState.relics_remaining -= 1` → 取最后一件触发塌缩（§十六.一 step 4 / §十六.三 "传说档容器 = §十六.一 的骨架"）
7. **容器互斥搜刮** — 同一容器一次只能一人在搜；他人想搜 → 拒绝。搜刮者中断 → 容器重新可抢
8. **容器 spawn 在 zone 初始化时一次性注册** — 不随玩家进入刷新，不热 reload。P3 demo 用 `/tsy-spawn` 同时注入容器；正式发布由 worldgen 填充
9. **钥匙掉落规则（来源）**：
   - 秘境守灵 NPC 100% 掉钥匙（见 P4 hostile plan）
   - 道伥低概率（~5%）掉钥匙
   - 历史散布：zone 初始化时按 Poisson 在浅/中层 spawn 少量钥匙作为 `DroppedLootRegistry.ownerless` 条目
10. **不上 NBT 持久化** — 容器状态（已搜刮与否）是 zone-scoped 运行时状态，zone 塌缩一起消失；不需要写 SQLite

---

## §1 数据模型

### 1.1 `LootContainer` Component

**位置**：新建 `server/src/world/tsy_container.rs`，通过 `server/src/world/mod.rs` 暴露

```rust
use bevy_ecs::prelude::Component;
use crate::world::zone::TsyLayer;

/// TSY 容器的运行时状态。挂在 Entity 上，Entity 位置 = 容器位置。
#[derive(Component, Debug)]
pub struct LootContainer {
    pub kind: ContainerKind,
    pub family_id: String,           // TSY 家族 id, e.g. "tsy_lingxu_01"
    pub layer: TsyLayer,             // 所在层深
    pub loot_pool_id: String,        // 指向 loot_pools.json 的 key
    pub locked: Option<KeyKind>,     // Some = 锁着需要对应钥匙
    pub searched_by: Option<bevy_ecs::entity::Entity>,  // 当前搜刮者（互斥）
    pub depleted: bool,              // 已被搜空
    pub spawned_at_tick: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainerKind {
    DryCorpse,     // 干尸（普通）
    Skeleton,      // 骨架（普通）
    StoragePouch,  // 储物袋残骸（罕见）
    StoneCasket,   // 石匣 / 玉棺（史诗，locked）
    RelicCore,     // 法阵核心（传说，locked，= 骨架）
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyKind {
    StoneCasketKey,    // 石匣匙
    JadeCoffinSeal,    // 玉棺纹
    ArrayCoreSigil,    // 阵核钤（RelicCore 专用）
}

impl ContainerKind {
    /// 基础搜刮时长（tick，20 tps）
    pub const fn base_search_ticks(self) -> u32 {
        match self {
            Self::DryCorpse    => 80,   //  4 秒
            Self::Skeleton     => 80,   //  4 秒
            Self::StoragePouch => 200,  // 10 秒
            Self::StoneCasket  => 400,  // 20 秒
            Self::RelicCore    => 600,  // 30 秒
        }
    }

    /// 需要的钥匙类型（None = 不需要锁）
    pub const fn required_key(self) -> Option<KeyKind> {
        match self {
            Self::StoneCasket => Some(KeyKind::StoneCasketKey),  // 石匣 = 石匣匙
            Self::RelicCore   => Some(KeyKind::ArrayCoreSigil),  // 阵核 = 阵核钤
            _                 => None,
        }
    }

    /// 是否为骨架（= 对 zone 结构有支撑作用）
    pub const fn is_skeleton(self) -> bool {
        matches!(self, Self::RelicCore)
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DryCorpse    => "dry_corpse",
            Self::Skeleton     => "skeleton",
            Self::StoragePouch => "storage_pouch",
            Self::StoneCasket  => "stone_casket",
            Self::RelicCore    => "relic_core",
        }
    }
}
```

**注**：`StoragePouch` 未锁（罕见档属于"靠走运 / 靠眼力 / 靠抢先"而非靠钥匙层门），`StoneCasket` / `RelicCore` 锁；具体锁什么由 `required_key()` 决定。后续迭代可把玉棺拆出独立种，现在合并进 `StoneCasket` + `JadeCoffinSeal` 的组合（在 spawn 时按 zone 起源选匙）。

### 1.2 `SearchProgress` Component

挂在**玩家 Entity** 上（不在容器上），表示该玩家当前正在搜刮的容器：

```rust
use bevy_ecs::prelude::{Component, Entity};

#[derive(Component, Debug)]
pub struct SearchProgress {
    pub container: Entity,        // 目标容器
    pub required_ticks: u32,      // = ContainerKind::base_search_ticks
    pub elapsed_ticks: u32,
    pub started_at_tick: u64,
    pub started_pos: [f64; 3],    // 用于中断检测（位置偏移 > 0.5 block 则中断）
    pub key_item_instance_id: Option<u64>,  // 已锁定要消耗的钥匙 instance_id（完成时扣）
}
```

**生命周期**：
- `start_search(player, container)` 成功 → insert `SearchProgress`
- `tick_search` 每 tick 累加 elapsed + 检查中断条件
- 完成 / 中断 → remove `SearchProgress`

### 1.3 钥匙物品：`ContainerKey` 作为 `ItemInstance` 的 template 约定

**不新增 struct**，钥匙复用现有 `ItemInstance`，约定：

- `template_id` 以 `key_` 开头（e.g. `key_stone_casket`、`key_jade_coffin`、`key_array_core`）
- `spirit_quality = 0.0`（钥匙是凡物，入场过滤不会剥）
- `stack_count` 可叠加（一人可持多把）
- `durability = 1.0`（只用一次就失效 = 直接扣 stack_count 一次）
- 物品模板在 `server/assets/items/tsy_keys.toml` 定义

**识别 helper**：

```rust
impl ItemInstance {
    pub fn as_container_key(&self) -> Option<KeyKind> {
        match self.template_id.as_str() {
            "key_stone_casket" => Some(KeyKind::StoneCasketKey),
            "key_jade_coffin"  => Some(KeyKind::JadeCoffinSeal),
            "key_array_core"   => Some(KeyKind::ArrayCoreSigil),
            _ => None,
        }
    }
}
```

### 1.4 `loot_pools.json` 配置文件

**位置**：新建 `server/loot_pools.json`

**schema**（简化，非 JSON Schema，只是示意）：

```json
{
  "pools": {
    "dry_corpse_shallow_common": {
      "entries": [
        { "template_id": "iron_sword_worn",  "weight": 40, "count": [1, 1] },
        { "template_id": "bone_coin_dead",   "weight": 30, "count": [1, 5] },
        { "template_id": "dry_grass_common", "weight": 20, "count": [1, 3] },
        { "template_id": "tattered_robe",    "weight": 10, "count": [1, 1] }
      ],
      "rolls": [1, 3]
    },

    "skeleton_shallow_common": {
      "entries": [
        { "template_id": "bone_shard",           "weight": 50, "count": [1, 3] },
        { "template_id": "bone_coin_dead",       "weight": 30, "count": [1, 3] },
        { "template_id": "iron_knife_worn",      "weight": 20, "count": [1, 1] }
      ],
      "rolls": [1, 2]
    },

    "storage_pouch_mid": {
      "entries": [
        { "template_id": "pill_residue_low",   "weight": 40, "count": [1, 2] },
        { "template_id": "talisman_fragment",  "weight": 25, "count": [1, 1] },
        { "template_id": "manual_fragment",    "weight": 20, "count": [1, 1] },
        { "template_id": "minor_talisman",     "weight": 15, "count": [1, 1] }
      ],
      "rolls": [1, 3]
    },

    "stone_casket_mid": {
      "entries": [
        { "template_id": "mid_tier_sword",     "weight": 35, "count": [1, 1] },
        { "template_id": "mid_tier_robe",      "weight": 25, "count": [1, 1] },
        { "template_id": "sect_token",         "weight": 20, "count": [1, 1] },
        { "template_id": "cultivation_manual", "weight": 20, "count": [1, 1] }
      ],
      "rolls": [1, 2]
    },

    "relic_core_deep": {
      "entries": [
        { "template_id": "__ancient_relic_random__", "weight": 100, "count": [1, 1] }
      ],
      "rolls": [1, 1]
    }
  }
}
```

**特殊 template**：`__ancient_relic_random__` 是一个 sentinel，在 `resolve_loot_pool` 里 detect 到后转发给 P1 plan 的 `AncientRelicTemplate::random_roll()`。

**rolls 字段**：每次搜刮滚多少个 entry（范围 min-max，闭区间，均匀分布）。

### 1.5 容器 spawn 规则（`server/zones.json` 扩展或独立 `tsy_containers.json`）

**选项 A**：在 zones.json 的 TSY zone 条目里加 `container_spec` 字段。
**选项 B**：独立 `server/tsy_containers.json` 按 zone name 索引。

**选 B**，降低 zones.json 复杂度。

```json
{
  "tsy_lingxu_01": {
    "shallow": {
      "containers": [
        { "kind": "dry_corpse",    "count": 12, "loot_pool": "dry_corpse_shallow_common" },
        { "kind": "skeleton",      "count":  8, "loot_pool": "skeleton_shallow_common"   },
        { "kind": "storage_pouch", "count":  2, "loot_pool": "storage_pouch_mid"         }
      ]
    },
    "mid": {
      "containers": [
        { "kind": "dry_corpse",    "count": 6, "loot_pool": "dry_corpse_shallow_common" },
        { "kind": "storage_pouch", "count": 4, "loot_pool": "storage_pouch_mid"         },
        { "kind": "stone_casket",  "count": 2, "loot_pool": "stone_casket_mid"          }
      ]
    },
    "deep": {
      "containers": [
        { "kind": "dry_corpse",    "count": 3, "loot_pool": "dry_corpse_shallow_common" },
        { "kind": "stone_casket",  "count": 1, "loot_pool": "stone_casket_mid"          },
        { "kind": "relic_core",    "count": 3, "loot_pool": "relic_core_deep"           }
      ]
    }
  }
}
```

**spawn 位置**：`/tsy-spawn` 命令或 zone 初始化 hook 在 zone AABB 内随机撒点（避开 `blocked_tiles`），给每个 container 挂 `Transform` + `LootContainer`。

**P3 demo 固定** `deep.relic_core.count = 3`（= P2 骨架数），保证跟 lifecycle plan 的 `relics_remaining = 3` 初值对齐。

---

## §2 搜刮系统

### 2.1 开始搜刮：`start_search_container` system / event

**事件**：

```rust
#[derive(Event, Debug)]
pub struct StartSearchRequest {
    pub player: Entity,
    pub container: Entity,
}

#[derive(Event, Debug)]
pub enum StartSearchResult {
    Started { player: Entity, container: Entity, required_ticks: u32 },
    Rejected { player: Entity, container: Entity, reason: SearchRejectionReason },
}

#[derive(Debug, Clone)]
pub enum SearchRejectionReason {
    Depleted,
    OccupiedByOther,        // searched_by = Some(other_entity)
    MissingKey(KeyKind),    // 需要钥匙但 inventory 没有
    AlreadySearching,       // 玩家已经在搜别的容器
    OutOfRange,             // 距离 > 3 block
    InCombat,               // 战斗状态下不允许开搜
}
```

**system 逻辑**：

```rust
fn start_search_container(
    mut events: EventReader<StartSearchRequest>,
    mut results: EventWriter<StartSearchResult>,
    mut containers: Query<(&mut LootContainer, &Transform)>,
    players: Query<(&Transform, &PlayerInventory, &CombatState, Option<&SearchProgress>), With<Player>>,
    mut commands: Commands,
    tick: Res<ServerTick>,
) {
    for req in events.read() {
        let Ok((p_tf, p_inv, p_combat, p_progress)) = players.get(req.player) else { continue };
        let Ok((mut container, c_tf)) = containers.get_mut(req.container) else { continue };

        if p_progress.is_some() { /* 发 AlreadySearching, continue */ }
        if container.depleted   { /* 发 Depleted, continue */ }
        if container.searched_by.is_some() && container.searched_by != Some(req.player) { /* 发 OccupiedByOther */ }
        if distance(p_tf, c_tf) > 3.0 { /* 发 OutOfRange */ }
        if matches!(p_combat, CombatState::Combat { .. }) { /* 发 InCombat */ }

        // 检查钥匙
        let key_id = match container.kind.required_key() {
            Some(kk) => match find_key_in_inventory(p_inv, kk) {
                Some(id) => Some(id),
                None => { /* 发 MissingKey(kk), continue */ return; }
            },
            None => None,
        };

        // 通过所有检查 → 开始搜刮
        container.searched_by = Some(req.player);
        commands.entity(req.player).insert(SearchProgress {
            container: req.container,
            required_ticks: container.kind.base_search_ticks(),
            elapsed_ticks: 0,
            started_at_tick: tick.0,
            started_pos: p_tf.translation.as_array_f64(),
            key_item_instance_id: key_id,
        });

        results.send(StartSearchResult::Started { ... });
    }
}
```

### 2.2 搜刮进度 tick：`tick_search_progress` system

```rust
fn tick_search_progress(
    mut players: Query<(Entity, &Transform, &CombatState, &Wounds, &mut SearchProgress)>,
    mut commands: Commands,
    mut complete_events: EventWriter<SearchCompleted>,
    mut abort_events: EventWriter<SearchAborted>,
) {
    for (player_ent, tf, combat, wounds, mut progress) in players.iter_mut() {
        // 中断条件
        if distance(tf.translation.as_array_f64(), progress.started_pos) > 0.5 { /* 中断 */ }
        if matches!(combat, CombatState::Combat { .. })                        { /* 中断 */ }
        if wounds.damaged_this_tick()                                          { /* 中断 */ }

        progress.elapsed_ticks += 1;

        if progress.elapsed_ticks >= progress.required_ticks {
            complete_events.send(SearchCompleted {
                player: player_ent,
                container: progress.container,
                key_item_instance_id: progress.key_item_instance_id,
            });
            commands.entity(player_ent).remove::<SearchProgress>();
        }
    }
}
```

**中断**：
- remove `SearchProgress`
- 对应容器 `searched_by` 清空
- 发 `SearchAborted { player, container, reason }`

### 2.3 真元加速 × 1.5：`apply_search_drain_multiplier` hook

**接入 P0 plan 的 `compute_drain_per_tick`**：

```rust
// P0 原函数
fn compute_drain_per_tick(zone: &Zone, player: &PlayerState, presence: &TsyPresence) -> f64 {
    let base = BASE_DRAIN_PER_TICK;
    let scaled = base * zone.spirit_qi.abs() * (player.spirit_qi / REFERENCE_POOL).powf(NONLINEAR_EXPONENT);
    scaled
}

// P3 扩展：读玩家是否在搜刮
fn compute_drain_per_tick_p3(
    zone: &Zone,
    player: &PlayerState,
    presence: &TsyPresence,
    searching: Option<&SearchProgress>,  // ← 新参数
) -> f64 {
    let base = compute_drain_per_tick(zone, player, presence);
    if searching.is_some() { base * 1.5 } else { base }
}
```

**替换方式**：P3 plan 修改 P0 plan 的 drain system 签名，让它 join `Option<&SearchProgress>` query。P0 实装时直接写成支持 multiplier 的版本，P3 挂 hook 即可；或者 P0 先实装不带 SearchProgress，P3 再打 patch。后者需要修改 system signature，破坏 P0 现有测试，P3 要一起补 test。

### 2.4 搜刮完成：`handle_search_completed` system

```rust
fn handle_search_completed(
    mut events: EventReader<SearchCompleted>,
    mut containers: Query<&mut LootContainer>,
    mut inventories: Query<&mut PlayerInventory>,
    mut relic_extracted_events: EventWriter<RelicExtracted>,
    loot_pools: Res<LootPoolRegistry>,
    mut allocator: ResMut<InventoryInstanceIdAllocator>,
    tick: Res<ServerTick>,
) {
    for e in events.read() {
        let Ok(mut container) = containers.get_mut(e.container) else { continue };

        // 1. 滚 loot
        let items = roll_loot_pool(&loot_pools, &container.loot_pool_id, &mut allocator);

        // 2. 把 loot 塞玩家背包（溢出丢到 DroppedLoot.ownerless at container pos）
        let Ok(mut inv) = inventories.get_mut(e.player) else { continue };
        for item in items {
            if !try_insert_into_inventory(&mut inv, item.clone()) {
                spawn_ownerless_drop(&mut dropped_registry, item, container_world_pos);
            }
        }

        // 3. 消耗钥匙
        if let Some(key_id) = e.key_item_instance_id {
            consume_one_stack(&mut inv, key_id);
        }

        // 4. 标记 depleted
        container.searched_by = None;
        container.depleted = true;

        // 5. 如果是 RelicCore → 发 RelicExtracted 给 P2 lifecycle
        if container.kind.is_skeleton() {
            relic_extracted_events.send(RelicExtracted {
                family_id: container.family_id.clone(),
                at_tick: tick.0,
            });
        }
    }
}
```

---

## §3 钥匙系统详细

### 3.1 钥匙物品模板（`server/assets/items/tsy_keys.toml`）

```toml
[[items]]
template_id = "key_stone_casket"
display_name = "石匣匙"
grid_w = 1
grid_h = 1
weight = 0.1
rarity = "common"
description = "插入石匣匙孔即开。用过即化灰。"
spirit_quality = 0.0
durability = 1.0

[[items]]
template_id = "key_jade_coffin"
display_name = "玉棺纹"
grid_w = 1
grid_h = 1
weight = 0.2
rarity = "rare"
description = "按手即亮，玉棺封印瞬解。次即暗。"
spirit_quality = 0.0
durability = 1.0

[[items]]
template_id = "key_array_core"
display_name = "阵核钤"
grid_w = 1
grid_h = 1
weight = 0.3
rarity = "epic"
description = "阵核钤印，能在阵眼上敲出一个缺口——仅此一次。"
spirit_quality = 0.0
durability = 1.0
```

### 3.2 钥匙匹配：`find_key_in_inventory`

```rust
fn find_key_in_inventory(inv: &PlayerInventory, kind: KeyKind) -> Option<u64> {
    let target_template = match kind {
        KeyKind::StoneCasketKey => "key_stone_casket",
        KeyKind::JadeCoffinSeal => "key_jade_coffin",
        KeyKind::ArrayCoreSigil => "key_array_core",
    };
    for container in &inv.containers {
        for slot in &container.slots {
            if let Some(item) = slot {
                if item.template_id == target_template { return Some(item.instance_id); }
            }
        }
    }
    None
}
```

### 3.3 钥匙的**来源**三路径

| 来源 | 触发 | 设计意图 |
|------|------|---------|
| **秘境守灵 NPC** | drop on kill（100%） | 高门槛 boss reward；详见 P4 hostile plan |
| **道伥 NPC** | drop on kill（5%） | 低概率稳定来源，配合 farm 道伥节奏 |
| **历史散布** | zone 初始化时按 Poisson 撒落在浅 / 中层表面 | 给走位好的玩家一条"白嫖钥匙"路径，不需要强制打 NPC |

**P3 demo 先实装第 3 条**（zone 初始化撒落），NPC drop 在 P4 plan 实装后接入。

---

## §4 Spawn 规则

### 4.1 容器 spawn 时机

**选项 A**（P3 demo）：`/tsy-spawn <family_id>` 调试命令调用 `spawn_tsy_containers(family_id)`，读取 `tsy_containers.json` 的配置，在对应 zone AABB 内按 `count` 随机撒点。

**选项 B**（正式发布）：Worldgen 生成 zone 时同步写 container 配置；或 zone 首次有玩家进入时 lazy spawn。

P3 plan 范围限定 A，B 列为非目标。

### 4.2 按 TSY 起源调整 count

Meta plan §2.3 约定 zone name 编码起源（`tsy_<origin>_<id>_<layer>`）。P3 plan 在 spawn 时根据 origin 应用乘数：

| 起源 | `dry_corpse` × | `storage_pouch` × | `stone_casket` × | `relic_core` × |
|------|---------------|-------------------|------------------|----------------|
| `tankuozun`（大能陨落） | 0.7 | 0.8 | 0.5 | **1.3**（陨落处遗物浓度高） |
| `zongmen_*`（宗门遗迹） | 1.0 | 1.2 | **1.5** | 1.0 |
| `zhanchang_*`（战场沉淀） | **1.3** | 0.5 | 0.4 | 0.6 |
| `gaoshou_*`（近代高手死处） | 1.0 | 1.0 | 0.7 | 0.5 |

P3 plan 把这个映射放在 `TsyOriginModifier` resource 里，常量表形式：

```rust
#[derive(Resource)]
pub struct TsyOriginModifier {
    pub table: HashMap<String, OriginMultiplier>,
}

pub struct OriginMultiplier {
    pub dry_corpse_x: f32,
    pub storage_pouch_x: f32,
    pub stone_casket_x: f32,
    pub relic_core_x: f32,
}
```

### 4.3 与 P2 lifecycle `TsyZoneState.relics_remaining` 对齐

**规则**：zone 初始化时 `relics_remaining = count(relic_core in deep layer)`。P2 plan 的 `init_tsy_zone_state` hook 在 container spawn 后读取 `LootContainer` query 统计 `kind == RelicCore && !depleted`，设为初值。

P3 plan 要在 container spawn 系统运行完后触发 `TsyZoneInitialized { family_id }` 事件，P2 plan 的 lifecycle init 监听此事件做 `relics_remaining` 计数。

---

## §5 客户端同步

### 5.1 IPC schema：`container-interaction-v1`

新建 `agent/packages/schema/src/container-interaction.ts`：

```typescript
import { Type } from '@sinclair/typebox';

export const ContainerStateV1 = Type.Object({
  entity_id: Type.Number(),         // container Entity id
  kind: Type.Union([
    Type.Literal('dry_corpse'),
    Type.Literal('skeleton'),
    Type.Literal('storage_pouch'),
    Type.Literal('stone_casket'),
    Type.Literal('relic_core'),
  ]),
  world_pos: Type.Tuple([Type.Number(), Type.Number(), Type.Number()]),
  locked: Type.Optional(Type.Union([
    Type.Literal('stone_casket_key'),
    Type.Literal('jade_coffin_seal'),
    Type.Literal('array_core_sigil'),
  ])),
  depleted: Type.Boolean(),
  searched_by_player_id: Type.Optional(Type.String()),  // uuid
});

export const SearchStartedV1 = Type.Object({
  player_id: Type.String(),
  container_entity_id: Type.Number(),
  required_ticks: Type.Number(),
  at_tick: Type.Number(),
});

export const SearchProgressV1 = Type.Object({
  player_id: Type.String(),
  container_entity_id: Type.Number(),
  elapsed_ticks: Type.Number(),
  required_ticks: Type.Number(),
});

export const SearchCompletedV1 = Type.Object({
  player_id: Type.String(),
  container_entity_id: Type.Number(),
  loot_preview: Type.Array(Type.Object({
    template_id: Type.String(),
    display_name: Type.String(),
    stack_count: Type.Number(),
  })),
});

export const SearchAbortedV1 = Type.Object({
  player_id: Type.String(),
  container_entity_id: Type.Number(),
  reason: Type.Union([
    Type.Literal('moved'),
    Type.Literal('combat'),
    Type.Literal('damaged'),
    Type.Literal('cancelled'),
  ]),
});
```

**触发时机**：
- `SearchStartedV1` — `start_search_container` 成功时发
- `SearchProgressV1` — 每 5 tick 广播一次（避免高频网络抖动）
- `SearchCompletedV1` — `handle_search_completed` 完成后发
- `SearchAbortedV1` — `tick_search_progress` 中断时发

**接入 Rust 端**：`server/src/schema/container.rs`（新建）通过 `typebox-to-serde` 流水线生成 struct + 反序列化。

### 5.2 客户端 HUD

**位置**：新建 `client/src/main/java/com/bong/client/tsy/SearchProgressHud.java`

**显示**：
- 玩家 HUD 下方中央渲染进度条（和现有真元条风格一致）
- 文本：「正在搜刮：干尸 [4s]」`"正在搜刮：<container_kind_zh> [<time_left>s]"`
- 进度满 → flash 收回 → SearchCompleted 后显示 loot preview（3 秒）
- 中断 → flash 红色 + 原因文字（"位置偏移"/"进入战斗"/"受击"）

**实现**：
- `SearchProgressHud` 挂到 `HudRenderEvent`
- 监听 `SearchStartedV1` → start 本地倒计时（服务器每 5 tick 下发矫正）
- 监听 `SearchAbortedV1` / `SearchCompletedV1` → 清除 HUD

### 5.3 交互 binding

- 玩家左键 / `USE` 键点击容器 → 触发 client 发 `StartSearchRequest` IPC 到 server
- 搜刮期间按 `ESC` / 任意动作键 → 发 `CancelSearchRequest`（→ server 发 `SearchAborted { reason: cancelled }`）

---

## §6 对接 P1/P2 plan

### 6.1 P1 plan.loot 借用

- P1 定义了 `AncientRelicTemplate::random_roll()` 随机上古遗物模板；本 plan `resolve_loot_pool` 读到 `__ancient_relic_random__` sentinel 时转发给它
- P1 的 `DroppedLootRegistry.ownerless` 用于：a) 容器溢出时把 loot 丢地面，b) zone 初始化时撒落钥匙

### 6.2 P2 plan.lifecycle 事件联动

新建 `server/src/world/tsy_events.rs`：

```rust
#[derive(Event, Debug)]
pub struct RelicExtracted {
    pub family_id: String,
    pub at_tick: u64,
}

#[derive(Event, Debug)]
pub struct TsyZoneInitialized {
    pub family_id: String,
    pub relic_count: u32,
}
```

- P3 plan 发 `RelicExtracted`，P2 plan 消费（`relics_remaining -= 1`）
- P3 plan 发 `TsyZoneInitialized { relic_count }`，P2 plan 消费（设初值）

事件的"谁定义谁使用"原则：Meta plan §2 新增横切条目 **`2.4 TSY 跨 plan 事件`** —— 由本 P3 plan 接纳（因为事件 producer 在 P3，consumer 在 P2，语义更属于 P3 → P2 的 outbound）。

### 6.3 P4 plan.hostile NPC drop 接入

P4 plan 的 `NpcDrops` 配置文件要能写 `key_stone_casket` 等钥匙 template；本 plan 不管 drop 的触发，只保证钥匙 template 和 inventory 能吃下。

---

## §7 验收 demo

**E2E 场景**（P3 单阶段）：

1. 运行 `/tsy-spawn tsy_lingxu_01` 生成一个 TSY，内含 5 个干尸 + 2 个储物袋 + 1 个石匣 + 3 个法阵核心（deep 层）
2. 玩家 A（引气 3，进来之前身上带一把 `key_stone_casket`）走到一个干尸旁，按 E → 搜刮进度条 4 秒
3. 搜刮期间真元消耗速率从 baseline 0.3/s 提升到 0.45/s（× 1.5）— HUD 观察
4. 搜刮完成 → 背包多出 1 把凡铁剑 + 2 枚死骨币；HUD 闪 loot preview 3 秒
5. 走到石匣旁按 E → 钥匙被 detect → 进度条 20 秒；
6. 玩家 B 过来攻击 A → A 的 `Wounds.damaged_this_tick = true` → `SearchAborted { reason: damaged }`；钥匙**未消耗**（completing 才消耗）；石匣 `searched_by` 清空
7. A 杀退 B，再次按 E 开始石匣搜刮 → 再等 20 秒完成 → 钥匙扣 1，背包多出中阶法器 1 件；石匣 `depleted = true`，不能再搜
8. 下到深层找到一个 `relic_core`，发现没钥匙（阵核钤没带），按 E → `Rejected { MissingKey(ArrayCoreSigil) }`，HUD 提示"需要阵核钤"
9. 找附近道伥打死，5% 掉率 —— 打 20 个才掉一把钥匙；带着钥匙回来
10. 搜 `relic_core` 30 秒完成 → 背包多出上古遗物 1 件；**发 `RelicExtracted`**；P2 plan 的 `relics_remaining` 从 3 减到 2
11. 再搜两个 `relic_core` 直到 `relics_remaining = 0` → **P2 plan 触发塌缩事件**（本 plan 不测这个，交 P2 验收）

**自动化测试**：

- `server/src/world/tsy_container.rs::tests`：
  - `base_search_ticks` 表校验
  - `ContainerKind::required_key` 映射校验
  - `ItemInstance::as_container_key` 识别校验
- `server/src/world/tsy_container_search.rs::tests`：
  - start_search 所有 Rejected 路径
  - tick_search 中断条件
  - complete_search 钥匙消耗 + RelicExtracted 事件发出（mock）
  - `compute_drain_per_tick_p3` 加速 × 1.5 校验
- 集成：`cargo test tsy_container` 跑完通过

---

## §8 非目标（推迟到 P3 后续或独立 plan）

| 功能 | 状态 | 说明 |
|------|------|------|
| 容器 worldgen 自动生成 | 独立 plan | P3 demo 只用 `/tsy-spawn` 命令 |
| 封灵匣 / 负灵袋（保养容器） | P3 后续 | 见 Meta §3 非目标；器修流派专属 |
| 容器贴图 / 粒子特效 | client 视觉 polish | 本 plan 只实装 HUD 进度条 + text；贴图交 client 独立 plan |
| loot pool 权重参数化调优 | 运营阶段 | 先用硬编码值，playtest 后调 |
| 多人同容器排队 | 不做 | `OccupiedByOther` 直接拒；等前者放弃 |
| 容器被破坏 / 爆炸造成 loot 掉一地 | 不做 | 保持"搜刮才出货" |
| 历史散布钥匙的时空分布曲线 | P3 后续 | MVP 只做 Poisson 随机撒 |
| 搜刮期间被击 → 短暂"眩晕" 后进度延后恢复 | P3 后续 | MVP 只做"中断归零" |

---

## §9 风险 / 未决

| 风险 | 级别 | 缓解 |
|------|------|------|
| 容器 Entity 数量过多导致 query 开销 | 中 | 每个 TSY ~30 个容器，5-10 个 active TSY = 150-300 Entity，远低于 NPC 数量；无风险 |
| 搜刮 progress bar 和玩家实际真元消耗不同步（网络延迟） | 中 | 服务器每 5 tick 下发矫正；客户端本地插值 |
| 钥匙在 inventory 里被玩家丢地面 → 搜刮时找不到 | 低 | `find_key_in_inventory` 只扫 `inv.containers` 不扫 `DroppedLoot`，设计即如此 |
| 塌缩事件发之前搜刮还在 tick → 可能产生奇怪状态 | 中 | P2 lifecycle `Collapsing` 阶段强制 `SearchAborted { reason: collapse }` 所有玩家；P3 plan 暴露一个 `abort_all_searches(family_id)` fn 给 P2 调用 |
| `searched_by` 在玩家 disconnect 时不释放 | 中 | 玩家退出 session 的 cleanup hook 里 remove `SearchProgress` + 清 `container.searched_by` |
| `__ancient_relic_random__` sentinel 和 P1 plan 的 `AncientRelicTemplate` 接口对不齐 | 中 | P3 开工前确认 P1 plan 已实装 `AncientRelicTemplate::random_roll() -> ItemInstance` 接口；若未实装，P3 plan 的 loot_pool resolver 提前 stub 一个假实现 |
| 容器 spawn 撒点撞 `blocked_tiles` | 低 | spawn rule 要 rejection sample（最多 20 次重试），撞 block 就换位置 |

---

## §10 命名与版本

- 本 plan 文件：`plan-tsy-container-v1.md`
- 实施后归档：`docs/finished_plans/plan-tsy-container-v1.md`
- v2 触发条件：上 worldgen 自动 spawn 容器 → 开 v2；或容器破坏 / 封灵匣器物上线 → 开 v2

---

## §11 进度日志

- 2026-04-25：本 plan 仍为纯设计骨架，未开工。`server/src/world/` 下仅有 `events.rs` / `mod.rs` / `terrain/` / `zone.rs`，未见 `tsy_container.rs` / `tsy_events.rs`；全仓 grep `LootContainer` / `ContainerKind` / `SearchProgress` / `RelicExtracted` / `TsyZoneInitialized` 均无命中；`server/loot_pools.json` 与 `server/tsy_containers.json` 不存在；`server/assets/items/tsy_keys.toml` 不存在；现有 `server/src/npc/loot.rs` 仅服务 NPC kill drop，与本 plan 5 档容器无关。等 P0/P1/P2 demoable 后再 `/consume-plan tsy-container`。
- **2026-04-26**：**P-1 解冻** — `plan-tsy-dimension-v1` 已 PR #47（merge 579fc67e）合并，跨位面基础设施就位（含 `DimensionLayers.tsy` 用于挂 container 实体到 TSY layer）。本 plan 仍 blocking on **P0/P1/P2 串行前置**。
- **2026-04-27**：**主体 merged** — PR #55 已合（fast-forward c331850e）。代码核对：`server/src/world/tsy_container.rs` + `tsy_container_search.rs` + `tsy_container_spawn.rs` 全落，`ContainerKind` / `SearchProgress` / `RelicExtracted` / `LootContainer` 五大组件确认；`server/assets/items/tsy_keys.toml`（37 行）+ `server/loot_pools.json` + `server/tsy_containers.json` 全部生成；client 侧 `SearchHudState.java` + `SearchProgressHudPlanner.java`（含单测 94 行）落地；agent `container-interaction.ts` schema + 4 个 `client-request-*.json` + 4 个 `search-*-v1.json` 已 export 到 generated/。剩余 ~5%：实机搜刮联调与文档自审清单未跑。dashboard percent 95%。

---

**下一步**：P0/P1/P2 全部 demoable 后，`/consume-plan tsy-container` 启动 P3。
