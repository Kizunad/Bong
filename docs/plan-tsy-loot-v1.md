# TSY 物资与秘境死亡 · plan-tsy-loot-v1

> 给 TSY 搭物资层：99 探索者遗物 + 1 上古遗物的分布规则；秘境所得 100% 掉落；干尸化；Fabric 禁用原生 keepInventory；DeathEvent 扩展 attacker 链路。
> 交叉引用：`plan-tsy-v1.md §2`（横切任务）· `plan-tsy-dimension-v1`（位面基础设施前置）· `plan-tsy-zone-v1.md §1.3`（TsyPresence）· `worldview.md §十六.三`（物资 99+1）· `worldview.md §十六.四`（入场/出关）· `worldview.md §十六.六`（死亡结算）· `plan-inventory-v1.md`（DroppedLoot 基础）· `plan-death-lifecycle-v1.md`（§十二 死亡规则）

> **2026-04-24 架构反转备忘**：TSY 实现为独立位面。本 plan 的"秘境内死亡"判定仍走 `TsyPresence` Component 存在性（不直接依赖位面信息），不受反转影响；但 `CorpseEmbalmed` 和 dropped loot 挂在 **TSY layer** 下（玩家在 TSY dim 内死亡 → 尸体/掉落物 entity 挂 `DimensionLayers.tsy`，不会漏到主世界）。P2 lifecycle 塌缩 cleanup 时这些 entity 随 zone 一起被清理。

---

## §-1 现状（已实装，不重做）

| 层 | 能力 | 位置 |
|----|------|------|
| ItemInstance | `instance_id / template_id / rarity / spirit_quality / durability / freshness` | `server/src/inventory/mod.rs:135-150` |
| ItemRarity | `Common / Uncommon / Rare / Epic / Legendary` | `server/src/inventory/mod.rs`（随 ItemInstance 同文件） |
| PlayerInventory | 多容器 + 装备 + 快捷栏 + 骨币 + 45 kg 上限 | `server/src/inventory/mod.rs:192-200` |
| DroppedLoot | `DroppedLootEntry` + `DroppedLootRegistry`（`HashMap<Entity, Vec<Entry>>`） | `server/src/inventory/mod.rs:1090-1102` |
| 50% 死亡掉落 | `apply_death_drop_on_revive` + `apply_death_drop_to_inventory` | `server/src/inventory/mod.rs:1304-1368` |
| Dropped loot 同步 | client 可视化 payload emit | `server/src/network/dropped_loot_sync_emit.rs` |
| DeathEvent | `{ target, cause, at_tick }` | `server/src/combat/events.rs:82-87` |
| 复活惩罚 | 降一阶 + qi=0 + 关脉 + 虚弱 | `server/src/cultivation/death_hooks.rs:45-75` |
| 运数/劫数 | 3 次运数 + Roll 概率 + 寿元扣除 | plan-death-lifecycle-v1 已实装 |
| TsyPresence | 玩家在 TSY 的 session 状态 + 入场 snapshot | `server/src/world/tsy.rs`（本系列 P0 新增） |

**本 plan 要新增**：
- 上古遗物 item template + 生成机制
- 99/1 按层深的 loot spawn table
- 秘境内死亡结算（区分"秘境所得" 100% vs "原带物" 50%）
- 干尸化（corpse item/entity 特殊 drop bag）
- Fabric keepInventory mixin（横切 2.2）
- `DeathEvent` 扩展 `attacker / attacker_player_id`（横切 2.1）

---

## §0 设计轴心

1. **99/1 比例铁律** — 每一层的 loot 构成都是 99% 凡物 + 1% 上古遗物；1% 倾向深层但不绝对（`worldview §十六.三`）
2. **上古遗物谁都能用，不绑定、不激活** — 捡到即可使用；唯一代价是耐久极低（`worldview §十六.三`）
3. **耐久字段化** — 上古遗物的 "1 次到三五次" 用现有 `ItemInstance.durability` 表达，`durability <= 0` 时物品自动销毁
4. **秘境所得识别靠 `entry_inventory_snapshot`** — 入场时记录的 instance_id 是"原带物"，之后捡到的是"秘境所得"
5. **秘境内死亡分流**：
   - 秘境所得 → **100% 掉在死亡点**（在 TSY zone 里）
   - 原带物 → **50% 掉在死亡点**（和 §十二 规则一致）
   - 玩家重生在灵龛，带走剩下的 50% 原带物（+ 秘境所得清零）
6. **干尸 = 特殊 corpse loot bag** — 不是地面散落物，而是一具可交互的"尸体"实体；后续 P2 lifecycle plan 的"道伥" 转化从这里激活
7. **凡人原生 drop 关掉** — Fabric 端禁用 MC 原生 `dropInventory()` 和 keepInventory 逻辑，所有掉落走 server 端 `DroppedLootRegistry`
8. **DeathEvent 加 attacker** 是前置要求 — 秘境 PVP 掠夺的链路（"谁杀了谁，谁的 loot 给谁看到"）需要攻击者信息

---

## §1 上古遗物数据模型

### 1.1 ItemRarity 扩展

**位置**：`server/src/inventory/mod.rs`（ItemRarity 定义处）

**改动**：新增一个 variant `Ancient`（上古遗物专用）

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemRarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
    Ancient,  // ← 新增：上古遗物，只由 TSY 生成
}
```

**IPC schema 同步**：`agent/packages/schema/src/inventory.ts` 对应 rarity enum 同步加 `"ancient"`。

### 1.2 上古遗物 template 表

**位置**：新建 `server/src/inventory/ancient_relics.rs`

```rust
/// 上古遗物模板定义
pub struct AncientRelicTemplate {
    pub template_id: String,           // "ancient_relic_sword_01"
    pub display_name: String,          // "无相剑残骸"
    pub kind: AncientRelicKind,
    pub source_class: AncientRelicSource,
    pub strength_tier: u8,             // 1-3，对应"一次 / 三次 / 五次使用"
    pub description: String,
}

pub enum AncientRelicKind {
    Weapon,            // 法宝 / 剑 / 钩
    Scroll,            // 残卷（功法 / 丹方 / 阵图）
    BeastCore,         // 异兽核（突破必需品）
    Pendant,           // 佩物（弟子遗物）
}

pub enum AncientRelicSource {
    DaoLord,           // 大能陨落类
    SectRuins,         // 宗门遗迹类
    BattleSediment,    // 战场沉淀类
}

/// MVP 种子表（P1 只配 ~10 件）
pub fn seed_ancient_relics() -> Vec<AncientRelicTemplate> {
    vec![
        AncientRelicTemplate {
            template_id: "ancient_relic_sword_wuxiang".into(),
            display_name: "无相剑残骸".into(),
            kind: AncientRelicKind::Weapon,
            source_class: AncientRelicSource::DaoLord,
            strength_tier: 3,
            description: "上古剑修无相真人的佩剑残骸。三击即碎。".into(),
        },
        AncientRelicTemplate {
            template_id: "ancient_relic_scroll_kaimai".into(),
            display_name: "《开脉残卷》".into(),
            kind: AncientRelicKind::Scroll,
            source_class: AncientRelicSource::SectRuins,
            strength_tier: 1,
            description: "失落宗门灵墟的开脉功法残页。一次性消耗。".into(),
        },
        AncientRelicTemplate {
            template_id: "ancient_relic_core_yibian".into(),
            display_name: "异变兽核（干涸）".into(),
            kind: AncientRelicKind::BeastCore,
            source_class: AncientRelicSource::BattleSediment,
            strength_tier: 1,
            description: "通灵境突破所需。一次性消耗。".into(),
        },
        // ... 再加 7 件凑 10
    ]
}
```

### 1.3 耐久映射

```rust
impl AncientRelicTemplate {
    pub fn to_item_instance(&self, allocator: &mut ItemIdAllocator) -> ItemInstance {
        let durability = match self.strength_tier {
            1 => 1.0,   // 一次性（用完 durability -> 0）
            2 => 3.0,   // 三次
            3 => 5.0,   // 五次
            _ => 1.0,
        };
        ItemInstance {
            instance_id: allocator.alloc(),
            template_id: self.template_id.clone(),
            display_name: self.display_name.clone(),
            grid_w: 1, grid_h: 2,  // 标准剑型尺寸，残卷 1x1
            weight: 1.5,
            rarity: ItemRarity::Ancient,
            description: self.description.clone(),
            stack_count: 1,
            spirit_quality: 0.0,   // 上古遗物本身带出来就是"无灵" —— 见 worldview §十六.三
            durability,
            freshness: None,
        }
    }
}
```

**使用时耐久扣减**：每次调用（攻击、学习残卷、突破消耗兽核）由对应系统 `durability -= 1.0`；归零即销毁（inventory 内 instance 被 remove）。

### 1.4 IPC schema：ItemInstance rarity 扩展

**位置**：`agent/packages/schema/src/inventory.ts`（现有 rarity enum 处）

```typescript
export const ItemRarityV1 = Type.Union([
  Type.Literal('common'),
  Type.Literal('uncommon'),
  Type.Literal('rare'),
  Type.Literal('epic'),
  Type.Literal('legendary'),
  Type.Literal('ancient'),   // ← 新增
]);
```

---

## §2 99/1 Loot 分布与 Spawn

### 2.1 Loot spawn 时机

上古遗物**不是在玩家取走时生成**，而是在 **TSY zone 首次激活时批量 spawn**（或作为 zone 配置的一部分预放置）。

**MVP 方案（P1 实现）**：
- TSY zone 在 ZoneRegistry 里注册时，同步 spawn N 件遗物到 zone 内的 patrol_anchors 附近
- N 取决于 TSY 的来源（见 §十六.一 生命周期 step 1）：大能陨落 3-5 件、宗门遗迹 5-10 件、战场沉淀 2-4 件
- 实际位置由 patrol_anchors + 随机偏移决定（MVP 用确定性 seed，保证 save/load 一致）

**P2 lifecycle plan 处理**：遗物 = 骨架的关联（取走一件 → zone state 更新）在 P2 里做。P1 只负责"把遗物摆在地上"。

### 2.2 上古遗物 spawn system

**位置**：新建 `server/src/inventory/tsy_loot_spawn.rs`

```rust
pub fn tsy_loot_spawn_on_zone_activate(
    mut commands: Commands,
    mut zone_events: EventReader<TsyZoneActivated>,
    zones: Res<ZoneRegistry>,
    mut registry: ResMut<DroppedLootRegistry>,
    mut allocator: ResMut<ItemIdAllocator>,
    relic_pool: Res<AncientRelicPool>,
) {
    for ev in zone_events.read() {
        let Some(zone) = zones.find_zone_by_name(&ev.family_id_shallow) else { continue };
        // 生成 3-10 件遗物，主要在 mid/deep 层
        let count = relic_count_for_source(&ev.source_class);  // e.g. 5
        let distribution = layer_distribution(count);          // (shallow, mid, deep) = (0, 1, 4)

        for (layer, layer_count) in distribution.iter() {
            for _ in 0..*layer_count {
                let relic_template = relic_pool.sample(&ev.source_class);
                let instance = relic_template.to_item_instance(&mut allocator);
                let pos = sample_position_in_layer(&ev.family_id, *layer);

                // 作为 "world-attached" DroppedLootEntry 注册
                // 注：普通 DroppedLootEntry 是和 owner Entity 绑定的；这里是"无主"的
                // 需要扩展 DroppedLootEntry 支持 ownerless（见 §2.3）
                registry.insert_ownerless(DroppedLootEntry {
                    instance_id: instance.instance_id,
                    source_container_id: "tsy_spawn".into(),
                    source_row: 0, source_col: 0,
                    world_pos: pos.to_array(),
                    item: instance,
                });
            }
        }
    }
}
```

### 2.3 `DroppedLootRegistry` 扩展：ownerless drops

**位置**：`server/src/inventory/mod.rs`（DroppedLootRegistry struct 附近）

**现状**：`DroppedLootRegistry.by_owner: HashMap<Entity, Vec<DroppedLootEntry>>` —— 所有 dropped loot 都 keyed by owner entity（最初掉落它的那个玩家）

**问题**：TSY 自然 spawn 的遗物没有 "owner"（它们是"前人留下的"）

**方案**：扩展 Registry

```rust
pub struct DroppedLootRegistry {
    pub by_owner: HashMap<Entity, Vec<DroppedLootEntry>>,
    pub ownerless: Vec<DroppedLootEntry>,  // ← 新增：无主掉落（TSY 自然 spawn、道伥掉落等）
}

impl DroppedLootRegistry {
    pub fn insert_ownerless(&mut self, entry: DroppedLootEntry) { ... }
    pub fn pick_up_ownerless(&mut self, instance_id: u64) -> Option<DroppedLootEntry> { ... }
    pub fn all_in_aabb(&self, aabb: (DVec3, DVec3)) -> Vec<&DroppedLootEntry> {
        // 扫 by_owner + ownerless, 返回在 AABB 内的所有 entry
    }
}
```

**拾取逻辑兼容**：现有 `pick_up` 入口（未读，推测在 `server/src/network/dropped_loot_sync_emit.rs` 附近）需要在 `by_owner` 找不到时回退 `ownerless`。

### 2.4 99% 凡物来源：初始空 + 自然累积

**核心点**：99% 不是 system spawn 的，而是**玩家死在 TSY 内留下的凡物**。这些在 P1 自然产生（秘境内死亡→ 秘境所得 100% + 原带物 50% 掉）。

所以 P1 loot 系统的范围是：
- spawn 1%（上古遗物，见 §2.2）
- handle 99% 的自然累积（见 §3 秘境内死亡）

**初始平衡**：P1 不预置任何凡物；早期 playtest 时修士进去还是能看到上古遗物。随着 playtest 进行，死过的玩家会自然留下凡物，99% 累积。**这是世界观级别的正确行为**（见 `worldview §十六.三` 经济循环）。

---

## §3 秘境内死亡结算

### 3.1 入口判定：玩家是否在 TSY 内死亡

**位置**：死亡系统的入口，`server/src/combat/lifecycle.rs` 或相邻 `cultivation/death_hooks.rs`

```rust
/// 判定玩家死亡时是否在 TSY 内
pub fn is_death_in_tsy(
    target: Entity,
    positions: &Query<&Position>,
    presence: &Query<&TsyPresence>,
) -> Option<TsyPresence> {
    presence.get(target).ok().cloned()
}
```

如果返回 `Some(presence)`，走 §3.2 分流逻辑；否则走原 `apply_death_drop_on_revive`（§十二 50% 规则）。

### 3.2 死亡分流算法

**位置**：新增 `server/src/inventory/tsy_death_drop.rs`

```rust
/// 结果：秘境内死亡的完整掉落结果
pub struct TsyDeathDropOutcome {
    pub entry_carry_dropped: Vec<ItemInstance>,   // 原带物的 50% 掉落部分
    pub entry_carry_kept: Vec<ItemInstance>,      // 原带物的 50% 保留（重生带回）
    pub tsy_acquired_dropped: Vec<ItemInstance>,  // 秘境所得全部掉落
    pub corpse_pos: DVec3,
    pub is_embalmed: bool,  // 干尸标记
}

pub fn apply_tsy_death_drop(
    inventory: &mut PlayerInventory,
    presence: &TsyPresence,
    death_pos: DVec3,
    seed: u64,
) -> TsyDeathDropOutcome {
    // Step 1: 分流
    let snapshot: HashSet<u64> = presence.entry_inventory_snapshot.iter().copied().collect();

    let mut entry_carry: Vec<ItemInstance> = Vec::new();   // 入场时 snapshot 里的
    let mut tsy_acquired: Vec<ItemInstance> = Vec::new();  // 之后新增的

    for container in inventory.containers.iter() {
        for item_opt in container.slots.iter() {
            if let Some(item) = item_opt {
                if snapshot.contains(&item.instance_id) {
                    entry_carry.push(item.clone());
                } else {
                    tsy_acquired.push(item.clone());
                }
            }
        }
    }
    // 装备槽 / hotbar 同样分流

    // Step 2: 原带物按 50% 规则随机掉落
    let mut rng = seed_rng(seed);
    let (entry_dropped, entry_kept) = split_50pct(entry_carry, &mut rng);

    // Step 3: 秘境所得 100% 掉落
    let tsy_dropped = tsy_acquired;

    // Step 4: 从 inventory 里移除所有掉落的 item
    for item in entry_dropped.iter().chain(tsy_dropped.iter()) {
        inventory.remove_by_instance_id(item.instance_id);
    }

    TsyDeathDropOutcome {
        entry_carry_dropped: entry_dropped,
        entry_carry_kept: entry_kept,
        tsy_acquired_dropped: tsy_dropped,
        corpse_pos: death_pos,
        is_embalmed: true,
    }
}
```

### 3.3 Drop bag 放置

```rust
pub fn spawn_death_drops(
    outcome: &TsyDeathDropOutcome,
    registry: &mut DroppedLootRegistry,
) {
    // 原带物的 50% 掉落 → 以死者为 owner 的 DroppedLootEntry
    for item in &outcome.entry_carry_dropped {
        registry.by_owner.entry(death_entity).or_default().push(DroppedLootEntry {
            instance_id: item.instance_id,
            source_container_id: "tsy_death_entry_carry".into(),
            source_row: 0, source_col: 0,
            world_pos: outcome.corpse_pos.to_array(),
            item: item.clone(),
        });
    }
    // 秘境所得 → 也 owner=死者（方便追踪 "这是谁留下的"），但**不走 50% 逻辑**
    for item in &outcome.tsy_acquired_dropped {
        registry.by_owner.entry(death_entity).or_default().push(DroppedLootEntry {
            instance_id: item.instance_id,
            source_container_id: "tsy_death_acquired".into(),
            source_row: 0, source_col: 0,
            world_pos: outcome.corpse_pos.to_array(),
            item: item.clone(),
        });
    }
}
```

**注**：死者自己的 Entity 在 lifecycle 死亡时会被 despawn，导致 `DroppedLootRegistry.by_owner[dead_entity]` 变成孤儿。需要：
- 选项 A：在 despawn 前把 by_owner 转移到 ownerless
- 选项 B：despawn 后留一个"遗物守护" Entity 持有这些 drop，直到被捡完或 despawn 过期

MVP 选 A：简单。

### 3.4 接线到现有死亡流水线

**位置**：修改 `server/src/inventory/mod.rs:1304` `apply_death_drop_on_revive`

```rust
pub fn apply_death_drop_on_revive(
    mut revived: EventReader<PlayerRevived>,
    mut inventories: Query<&mut PlayerInventory>,
    presence: Query<&TsyPresence>,   // ← 新增
    positions: Query<&Position>,     // ← 新增
    mut registry: ResMut<DroppedLootRegistry>,
    // ... existing ...
) {
    for ev in revived.read() {
        // ← 新增分流
        if let Ok(p) = presence.get(ev.player_entity) {
            let pos = positions.get(ev.player_entity).map(|p| p.0).unwrap_or_default();
            if let Ok(mut inv) = inventories.get_mut(ev.player_entity) {
                let outcome = apply_tsy_death_drop(&mut inv, p, pos, ev.seed);
                spawn_death_drops(&outcome, &mut registry);
                // 发 DroppedLootSync event
                // 发 CorpseEmbalmedSpawn event（供 P2 转化道伥）
                continue;  // skip 原 50% 逻辑
            }
        }

        // 原 50% 逻辑保持不变
        if let Ok(mut inv) = inventories.get_mut(ev.player_entity) {
            apply_death_drop_to_inventory(&mut inv, ev.seed);
            // ...
        }
    }
}
```

---

## §4 干尸化（Embalmed Corpse）

### 4.1 概念

按 `worldview §十六.六`：秘境内死亡 → 真元抽干 + 血肉被抽干 → 干尸。
干尸不是普通 drop bag，它**作为一个可交互实体**留在死亡点：

- 玩家可接近查看（显示死者名字 / 死亡时间 / 死因）
- 玩家可拾取其中的 loot（和其他 drop 一样通过 pickup 交互）
- 过一段时间（P2 plan 接手）会被负压激活成道伥

### 4.2 CorpseEmbalmed component

**位置**：新建 `server/src/inventory/corpse.rs`

```rust
/// 干尸实体
#[derive(Component, Debug, Clone)]
pub struct CorpseEmbalmed {
    pub original_player_id: Uuid,
    pub original_display_name: String,
    pub died_at_tick: u64,
    pub death_cause: String,
    pub family_id: String,        // 所在的 TSY family
    pub drops: Vec<u64>,          // instance_ids in DroppedLootRegistry
    /// 是否已被 P2 lifecycle 激活成道伥（MVP = false）
    pub activated_to_daoxiang: bool,
}
```

### 4.3 Spawn corpse

```rust
pub fn spawn_embalmed_corpse(
    commands: &mut Commands,
    outcome: &TsyDeathDropOutcome,
    player_info: &PlayerPublicInfo,
    tick: u64,
    cause: &str,
    family_id: &str,
) -> Entity {
    let drops = outcome.entry_carry_dropped.iter()
        .chain(outcome.tsy_acquired_dropped.iter())
        .map(|i| i.instance_id)
        .collect();
    commands.spawn((
        Position(outcome.corpse_pos),
        CorpseEmbalmed {
            original_player_id: player_info.uuid,
            original_display_name: player_info.display_name.clone(),
            died_at_tick: tick,
            death_cause: cause.to_string(),
            family_id: family_id.to_string(),
            drops,
            activated_to_daoxiang: false,
        },
        // Valence 端显示用的 visual marker（MVP 用一个静止的 zombie entity 改皮肤，后续 P3 polish）
    )).id()
}
```

### 4.4 Corpse 的客户端可视化

**MVP**：server 端 spawn 的 CorpseEmbalmed entity 走现有 Valence entity sync 机制，client 看到是一个 "fallen villager / zombie" mob（静止、不动、不攻击）。

**IPC schema**：`agent/packages/schema/src/tsy.ts` 新增

```typescript
export const TsyCorpseSpawnEventV1 = Type.Object({
  v: Type.Literal(1),
  kind: Type.Literal('tsy_corpse_spawn'),
  tick: Type.Number(),
  corpse_entity_id: Type.String(),
  original_player_id: Type.String(),
  original_display_name: Type.String(),
  family_id: Type.String(),
  death_cause: Type.String(),
  pos: Type.Array(Type.Number(), { minItems: 3, maxItems: 3 }),
});
```

---

## §5 Fabric keepInventory Mixin（横切 2.2）

### 5.1 目标

禁用 Minecraft 原生的死亡掉落与 keepInventory 游戏规则，所有掉落走 server 端 DroppedLootRegistry + sync event。

### 5.2 实现

**新建**：`client/src/main/java/com/bong/client/mixin/MixinPlayerEntityDrop.java`

```java
package com.bong.client.mixin;

import net.minecraft.entity.player.PlayerEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(PlayerEntity.class)
public abstract class MixinPlayerEntityDrop {

    /**
     * 拦截 PlayerEntity.dropInventory() —— 取消 vanilla 的死亡物品掉落。
     * 所有掉落统一由 Bong server 通过 DroppedLootRegistry 处理。
     */
    @Inject(method = "dropInventory", at = @At("HEAD"), cancellable = true)
    private void bong$cancelVanillaDrop(CallbackInfo ci) {
        ci.cancel();
    }
}
```

**注册**：`client/src/main/resources/bong-client.mixins.json`

```json
{
  "client": [
    "mixin.MixinCamera",
    "mixin.MixinGameRenderer",
    "mixin.MixinHeldItemRenderer",
    "mixin.MixinInGameHud",
    "mixin.MixinMinecraftClient",
    "mixin.MixinMouse",
    "mixin.MixinPlayerEntityHeldItem",
    "mixin.MixinPlayerEntityDrop"
  ]
}
```

### 5.3 副作用审查

- MC 原生拾取逻辑不受影响（只拦截 drop 不拦截 pickup）
- PvE 场景下 mob 死亡掉落 MC 原生 loot **也被这个 mixin 影响**？—— **不影响**，`dropInventory` 是 PlayerEntity 的方法；mob 是 `MobEntity.dropLoot`，不同路径
- /give 命令的物品加入仍然走 vanilla 路径（server 端 Bong 不用 /give 添加 inventory，用 `add_item_to_player_inventory`）

### 5.4 server 端的 keepInventory gamerule

**同步改动**：server 启动时设 gamerule keepInventory=true（让 MC 原生"保留 inventory"，避免和我们 mixin 冲突）

**位置**：`server/src/main.rs` 或 world init

```rust
// 在 world creation 时
world.set_gamerule("keepInventory", true);
```

这样:
- client mixin 拦截 dropInventory（double insurance）
- server gamerule 告诉 MC "不掉"
- 实际掉落走 Bong DroppedLootRegistry

---

## §6 DeathEvent 扩展（横切 2.1）

### 6.1 struct 改动

**位置**：`server/src/combat/events.rs:82-87`

```rust
#[derive(Event, Debug, Clone)]
pub struct DeathEvent {
    pub target: Entity,
    pub cause: String,
    pub attacker: Option<Entity>,
    pub attacker_player_id: Option<Uuid>,
    pub at_tick: u64,
}
```

### 6.2 所有 DeathEvent 发出点的改动

用 `grep -rn "DeathEvent {" server/src/` 找到所有发出点，逐一补 `attacker` / `attacker_player_id`：

- `server/src/combat/lifecycle.rs` (wound_bleed_tick) — cause="bleed_out"：attacker 从 Wounds 源取；如果是 PVP → player_id，否则 None
- `server/src/cultivation/death_hooks.rs`（若有）— cause="cultivation:xxx"：attacker=None
- `server/src/world/tsy_drain.rs`（P0 新增）— cause="tsy_drain"：attacker=None
- 其他

### 6.3 IPC schema

`agent/packages/schema/src/combat-event.ts` 的 `CombatRealtimeEventV1.attacker_id` 已是 Optional<string>（见 §-1 现状），无需改。
确保 Rust serde 写 `attacker_player_id: Option<Uuid>` 对应 TS `attacker_id?: string`。

### 6.4 Attacker 链路完整性测试

- [ ] 玩家 A 揍死玩家 B → `DeathEvent.attacker_player_id = Some(A.uuid)`
- [ ] 玩家 A 被 NPC 揍死 → `DeathEvent.attacker = Some(npc_entity)`, `attacker_player_id = None`
- [ ] 玩家 A 被负压抽死 → `attacker = None`, `attacker_player_id = None`（无人凶手）
- [ ] 玩家 A 修炼走火自爆 → `attacker = None`, `attacker_player_id = None`

---

## §7 封灵匣（保养容器）— 本 plan 不做

按 `plan-tsy-v1.md §3` 非目标表，封灵匣是 P3 后续工作，本 plan 不落地。**此处仅占位**：data model 先不定，等 P3 plan-tsy-polish 或 plan-forge-v2 时再设计。

---

## §8 测试策略

### 8.1 Rust unit tests

**位置**：各新增文件的 `#[cfg(test)]` 模块

- [ ] `ancient_relics.rs`：seed pool size、每个 source_class 至少 2 件
- [ ] `tsy_loot_spawn.rs`：`relic_count_for_source` 匹配表、`layer_distribution` 分布总和正确
- [ ] `tsy_death_drop.rs`：
  - entry_carry_snapshot 空时（入场空 → 全部是秘境所得）
  - entry_carry 有 10 件、tsy_acquired 5 件 → 50%/5 件掉 + 100%/5 件掉
  - 装备槽里的秘境所得（理论上不可能，因为装备一般不是秘境里捡的；但边界要测）
- [ ] `corpse.rs`：`spawn_embalmed_corpse` 字段赋值正确

**最少 20 tests**。

### 8.2 集成测试

**位置**：`server/tests/tsy_loot_integration.rs`（新建）

- [ ] 玩家进 TSY → 捡一件上古遗物 → 死亡（e.g. 真元归零）→ outcome 分流正确
- [ ] 玩家进 TSY → 捡 3 件遗物 → 原带 1 件凡剑 → 被 NPC 揍死 → 3 件秘境所得 100% 掉、凡剑 50% Roll
- [ ] 两个玩家进同一 TSY → 玩家 A 捡的遗物 A 死了掉了 → 玩家 B 能捡起来 → B 出关后背包有 A 捡过的遗物

### 8.3 Mixin test（Java）

**位置**：`client/src/test/java/com/bong/client/mixin/MixinPlayerEntityDropTest.java`（若有 Fabric test framework）或 manual smoke

- [ ] vanilla death 场景下 player inventory 不 drop（配合 server 端 keepInventory=true gamerule）
- [ ] MC /kill 命令触发死亡 → client 不显示物品掉落

### 8.4 Schema test

- [ ] `ItemRarityV1` 支持 'ancient'
- [ ] `TsyCorpseSpawnEventV1` round-trip
- [ ] TS → Rust serde artifact 跑通

### 8.5 Attacker chain test

独立 test file `server/tests/death_event_attacker_chain.rs`：

- [ ] PVP 死亡 → `attacker_player_id` 有值
- [ ] PVE 死亡 → `attacker_player_id` 无值，`attacker` 有值
- [ ] 环境死亡（tsy_drain / bleed_out without source）→ 两者都 None

---

## §9 验收标准

### Automated

执行 `bash scripts/smoke-tsy-loot.sh`（新建）：

- [ ] `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
- [ ] `cd agent/packages/schema && npm test`
- [ ] `cd client && ./gradlew test build`（验证 mixin 加载）
- [ ] Rust / TS schema artifact 一致
- [ ] `grep -rn "DeathEvent {" server/src/ | xargs -I{} echo "check {}"`  — 确保所有 DeathEvent 发出点都带 `attacker` 字段（否则编译会失败）

### Manual QA

**前置**：P0 plan-tsy-zone 已 merged；进游戏 `/tsy-spawn tsy_lingxu_01`；手动 give 玩家一些物品（含高灵质 + 低灵质 + 附灵武器）

- [ ] **A. 上古遗物 spawn**
  - [ ] 进入新生成的 TSY → shallow 层几乎看不到遗物
  - [ ] 走到 mid 层 → 1-2 件遗物地上可见
  - [ ] 走到 deep 层 → 3-5 件遗物聚集
  - [ ] 拾取一件遗物 → 背包里能看到，rarity = ancient，durability = 1/3/5

- [ ] **B. 使用上古遗物（耐久扣减）**
  - [ ] 装备 `ancient_relic_sword_wuxiang` → 攻击 3 次 → 第 4 次提示"碎裂散尽"→ 物品消失
  - [ ] 学习 `ancient_relic_scroll_kaimai` → 经脉 +1 → 残卷消失（durability=1 一次性）
  - [ ] 消耗 `ancient_relic_core_yibian` → 突破助力 → 兽核消失

- [ ] **C. 秘境死亡分流**
  - [ ] 玩家进 TSY 前背包有 10 件凡物（原带）
  - [ ] 在 TSY 里捡 3 件遗物（秘境所得）
  - [ ] 让真元归零死亡 → 检查死亡点掉落：
    - [ ] 3 件遗物 100% 在地上
    - [ ] 10 件原带物 ≈ 5 件在地上（50% Roll，误差 ±2）
    - [ ] 死亡点有 **干尸实体**（server log "CorpseEmbalmed spawned"）
    - [ ] 干尸处能交互 / 显示死者名字
  - [ ] 重生在灵龛 → 背包有剩下的 ≈ 5 件原带物，无秘境所得

- [ ] **D. PVP attacker 链路**
  - [ ] 玩家 A（引气）进 TSY
  - [ ] 玩家 B（固元）进同一 TSY，找到 A，揍死 A
  - [ ] A 的 `DeathEvent.attacker_player_id = B.uuid`（查 server log）
  - [ ] A 的掉落 owner 信息标明 "killed_by=B"（agent 或 log 可看到）

- [ ] **E. 主世界死亡（回归测试）**
  - [ ] 玩家在主世界正常死亡 → 走原 50% 规则，**不** spawn 干尸，不受本 plan 影响

- [ ] **F. Fabric mixin**
  - [ ] 玩家死亡 → MC 原生"物品掉落动画"**不出现**
  - [ ] 所有地面物品都是 Bong 自己的 DroppedLoot sync 渲染

### Acceptance bar

- [ ] 所有自动化 test 通过
- [ ] Manual QA A-F 全绿
- [ ] `grep -rn "DeathEvent {" server/` 不漏网

---

## §10 风险 / 未决

| 风险 | 级别 | 缓解 |
|------|------|------|
| `DeathEvent` 改动影响范围大，可能漏掉某个发出点 | 高 | strict grep + `cargo check`（编译器会 catch missing field） |
| Fabric mixin 在 Dev client 和 Production client 行为不同 | 中 | 两个环境都测（`./gradlew runClient` 和 `./gradlew build` jar 都验证） |
| 干尸实体用什么 Valence entity 类型 | 中 | MVP 用 zombie（静止，不 AI）；后续 P3 改 custom entity |
| 秘境所得 instance_id 和原带 instance_id 冲突 | 低 | `ItemIdAllocator` 全局 unique 保证；有测试 |
| 玩家在 TSY 外捡到秘境所得（通过别人死掉掉出来）再进 TSY → 被当作"原带" | 低 | entry_inventory_snapshot 是进门时拍的；后续捡到的都不算入 snapshot，所以这种物品在 TSY 内死亡时仍会被当作"秘境所得"（因为没在原 snapshot 里）— 行为正确 |
| 干尸持久化（断线 / 重启） | 中 | MVP 不持久化干尸实体（Bevy scene save 暂未包含）；重启后死在 TSY 的玩家留下的干尸消失；P2 lifecycle 若要"N 分钟变道伥"需要 tick 计时，断线后会丢失 — 标记为未决，P2 plan 决定 |
| `DroppedLootRegistry.by_owner[dead_entity]` 在 despawn 后孤儿 | 中 | despawn 前 transfer 到 ownerless；加一个系统做这件事（`server/src/inventory/` 新增 `transfer_owner_on_despawn.rs`） |

### 未决设计问题

- **上古遗物耐久的展示** — client 端需要改 item tooltip UI 以显示 durability（"3/5 次可用"）；本 plan 不做（推给 plan-HUD-v1 扩展），inventory 内部有字段就行
- **秘境所得堆叠**：如果玩家捡了 5 颗相同的灵草，他们堆在一个 stack 里，instance_id 只有一个——但 `entry_inventory_snapshot` 是按 instance_id 记录的。边界：若原带也有相同 template 的草，合并时要谨慎。MVP：堆叠只对 `spirit_quality = 0` 的凡物 merge；不同 snapshot 来源的 item 不自动堆叠（强制分列）
- **道伥 spawn 从干尸激活**：P2 lifecycle plan 处理；本 plan 只留 `CorpseEmbalmed.activated_to_daoxiang` 字段占位

---

## §11 依赖与文件清单

### 前置依赖

- ✅ `plan-tsy-zone-v1.md` 必须先 merge（需要 `TsyPresence` 和 TSY zone 识别）

### 本 plan 新增/修改文件

**Rust**（server）：

- `server/src/inventory/ancient_relics.rs`（新建，~150 行）
- `server/src/inventory/tsy_loot_spawn.rs`（新建，~200 行）
- `server/src/inventory/tsy_death_drop.rs`（新建，~250 行）
- `server/src/inventory/corpse.rs`（新建，~100 行）
- `server/src/inventory/mod.rs`（修改：ItemRarity 扩 Ancient, DroppedLootRegistry 扩 ownerless, apply_death_drop_on_revive 分流）
- `server/src/combat/events.rs`（修改：DeathEvent 扩 attacker 字段）
- `server/src/combat/lifecycle.rs`（修改：wound_bleed_tick 填 attacker）
- `server/src/cultivation/death_hooks.rs`（修改：所有 DeathEvent 发出点对齐）
- `server/src/world/tsy_drain.rs`（修改：drain-induced DeathEvent 对齐）
- `server/tests/tsy_loot_integration.rs`（新建，~250 行）
- `server/tests/death_event_attacker_chain.rs`（新建，~120 行）

**TS**（schema）：

- `agent/packages/schema/src/tsy.ts`（修改：加 `TsyCorpseSpawnEventV1`）
- `agent/packages/schema/src/inventory.ts`（修改：rarity 加 'ancient'）
- `agent/packages/schema/src/tsy.spec.ts`（修改 / 新建 round-trip test）

**Java**（client）：

- `client/src/main/java/com/bong/client/mixin/MixinPlayerEntityDrop.java`（新建，~30 行）
- `client/src/main/resources/bong-client.mixins.json`（修改：加 mixin 条目）

**脚本**：

- `scripts/smoke-tsy-loot.sh`（新建，~40 行）

### 规模估算

- Rust 新增 / 修改：~1500-1800 行
- TS schema：~100 行
- Java mixin：~30 行
- 测试：~400 行
- 总计：**~2000 行**，单次 /consume-plan 应能消化

---

## §12 后续 / 相关

- **P2 plan-tsy-lifecycle-v1** — 接管：
  - 干尸 → 道伥的激活机制（读 `CorpseEmbalmed.activated_to_daoxiang`）
  - 遗物骨架 state machine（读 `ownerless` loot 里的 Ancient rarity 数量）
  - 塌缩事件 + race-out
- **P3 plan-tsy-polish-v1** — 封灵匣 / UI tooltip / 入口感知 HUD
- **plan-HUD-v1 扩展** — 耐久可视化（tooltip 显示 durability）
- **plan-shelflife-v1** — 凡物的"灵气流失税"在 TSY 内外的行为（MVP 不变，后续看）

---

## §13 进度日志

- 2026-04-25：本 plan 仍为纯设计骨架，server 端 0 行落地。核查实际代码：
  - `ItemRarity` 仅 5 档（Common/Uncommon/Rare/Epic/Legendary），未扩 `Ancient`（§1.1 未做）
  - `server/src/inventory/` 下无 `ancient_relics.rs` / `tsy_loot_spawn.rs` / `tsy_death_drop.rs` / `corpse.rs`（§1–§4 未做）
  - `DroppedLootRegistry` 无 `ownerless` 字段（§2.3 未做）
  - `DeathEvent` 仍是 `{ target, cause, at_tick }`，未扩 `attacker / attacker_player_id`（§6 未做）
  - `TsyPresence` 在 server 端 grep 无命中，§-1 现状表中标注的"P0 新增"实际尚未落地，本 plan 的前置依赖（plan-tsy-zone-v1）未 merge
  - client mixin 列表无 `MixinPlayerEntityDrop`，仅有 Camera/GameRenderer/HeldItemRenderer/InGameHud/MinecraftClient/Mouse/PlayerEntityHeldItem（§5 未做）
  - 全部 `[ ]` 维持未勾选状态。
- **2026-04-26**：**P-1 解冻** — `plan-tsy-dimension-v1` 已 PR #47（merge 579fc67e）合并，跨位面 API / `Zone.dimension` / `CurrentDimension` 全部就位。本 plan 仍 blocking on **P0 `tsy-zone`**（`TsyPresence` component 由 P0 引入），需 P0 demoable 后才能开 `/consume-plan tsy-loot`。横切依赖 `plan-death-lifecycle-v1 §6 DeathEvent.attacker` 仍未启动。
- **2026-04-27**：**主体 merged** — PR #53（merge 9fb8d2b7）已合并。代码核对：`server/src/inventory/` 下 `ancient_relics.rs` / `tsy_loot_spawn.rs` / `tsy_death_drop.rs` / `corpse.rs` 全部确认；`ItemRarity::Ancient` variant 已加；`DeathEvent.attacker` / `attacker_player_id` 已扩；`DroppedLootRegistry.ownerless` 字段已建；`tsy_loot_integration_test.rs` 存在；client `MixinPlayerEntityDrop` 已加。横切 `plan-death-lifecycle-v1 §6 DeathEvent.attacker` 已被本 plan 顺手实装。剩余 ~8%：smoke 全链路 e2e 与文档自审清单全勾未跑。dashboard percent 92%。
