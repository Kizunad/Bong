# Bong · plan-weapon-v1

**武器法宝专项**。从 `plan-combat-no_ui.md §6` 抽离并深化，定义 Bong 从 inventory 占格、装备切换、主手 3D 渲染、到战斗加成的完整链路。一并规定 client 接管原生 MC hotbar / E 背包 UI 的替代策略。

**落位**：
- server：`server/src/combat/weapon.rs`（新）· `server/assets/items/weapons.toml`（扩展）
- client：`client/src/main/java/com/bong/client/weapon/` + `client/.../mixin/` + `client/.../assets/bong/models/`

**交叉引用**：
- 父 plan：`plan-combat-no_ui.md §6`（Weapon/Treasure component 设计原意）
- 依赖：`plan-inventory-v1.md`（背包占格 + ItemInstance/装备槽）
- 协作：`plan-HUD-v1.md §2.2`（战斗快捷栏）· `plan-particle-system-v1.md`（武器 VFX）· `plan-forge-v1.md`（锻造/修复）
- 范围外：**Treasure（法宝）展开为 Entity 的业务**留 `plan-treasure-v1`（本 plan 只占槽位）

---

## §0 设计轴心

| 原则 | 含义 | 反模式 |
|---|---|---|
| **双层模型** | ItemInstance（inventory 占格）+ Weapon Component（combat 运行时） | 只在 inventory 里放，战斗时现查 snapshot |
| **接管而非屏蔽** | 原生 hotbar / E 背包 路径完全替换为 Bong UI | 叠加两套 UI / 切换式显示 |
| **MC 物品系统零入侵** | 不注册 vanilla `Item`，纯 ItemInstance + template_id | 给每把武器注册 `Item::register` |
| **渲染走 Mixin** | `HeldItemRenderer` 拦截 → 按 Weapon 组件画模型 | 把武器绑进 `ItemStack` 的 NBT 绕一圈 |
| **Weapon ≠ Treasure** | 武器（握手上）和法宝（可展开飞出去）两条 flow | 一个 Component 同时表达两者 |
| **Soul-bond 是渐进关系** | 同角色同武器用得越久，bond_level 自然升级 | 一次性"命名绑定"无成长 |
| **赤手可战** | 无武器时伤害基数不为 0，走拳套基线 | 没武器就完全不能打 |

---

## §1 数据模型

### 1.1 ItemTemplate 扩展（weapon 特有字段）

TOML 定义（`server/assets/items/weapons.toml`）：

```toml
[[item]]
template_id = "iron_sword"
kind = "Weapon"
display_name = "铁剑"
grid_w = 1
grid_h = 2
weight = 1.2
stackable = false
weapon_kind = "Sword"       # Sword | Saber | Staff | Fist | Spear | Dagger | Bow
base_attack = 8.0
quality_tier = 0            # 0=凡铁 1=灵器 2=法宝 3=仙器
durability_max = 200.0
qi_cost_mul = 1.0           # 以此武器发动 qi 技能的 qi 消耗倍率
icon = "bong-client:textures/gui/items/iron_sword.png"
model = "bong:models/weapon/iron_sword"   # client 渲染查询 key
```

### 1.2 ItemInstance（复用 inventory-v1 字段 + 武器项）

```rust
struct ItemInstance {
    instance_id: u64,
    template_id: String,
    stack_count: u32,
    spirit_quality: f32,      // [0, 1]
    durability: f32,          // [0, durability_max] — 武器特有
    soul_bond: Option<SoulBond>,  // 武器特有
}

struct SoulBond {
    character_id: String,     // 绑定角色（不是玩家账号，而是 Bong 的 character 身份）
    bond_level: u8,           // 0=生疏 1=磨合 2=契合 3=神融
    bond_progress: f32,       // [0, 1] 朝下一级的进度
}
```

### 1.3 Weapon Component（combat 运行时派生层）

```rust
#[derive(Component)]
pub struct Weapon {
    pub instance_id: u64,       // 回指 inventory 里的 ItemInstance
    pub template_id: String,    // 缓存便于查询
    pub weapon_kind: WeaponKind,
    pub base_attack: f32,
    pub quality_tier: u8,
    pub durability: f32,
    pub durability_max: f32,
    pub soul_bond: Option<SoulBond>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponKind {
    Sword, Saber, Staff, Fist, Spear, Dagger, Bow,
}
```

**生命周期**：
- `EquipWeaponIntent` 处理时 server 插入 `Weapon` component 到玩家 Entity
- `UnequipWeaponIntent` 或死亡 / drop 时移除
- durability <= 0 时自动 unequip + 发 `WeaponBroken` 事件

### 1.4 Treasure Component（骨架，留 plan-treasure-v1 深化）

```rust
#[derive(Component)]
pub struct Treasure {
    pub instance_id: u64,
    pub template_id: String,
    pub treasure_kind: TreasureKind,  // FlyingSword | Talisman | Formation | FleshGolem
    pub energy: f32,
    pub cooldown_remaining: f32,
    // deploy_state 字段留 plan-treasure-v1
}
```

本 plan：仅在 inventory 里**占槽**并允许装备到 `treasure_belt`（腰带），**不实现展开飞出**。

---

## §2 装备/卸下状态机

### 2.1 状态图

```
   ┌────────┐   EquipWeaponIntent      ┌──────────┐
   │ Stowed │ ───────────────────────▶ │ Equipped │
   │ (bag)  │ ◀─────────────────────── │ (main_hd)│
   └────────┘   UnequipWeaponIntent    └──────────┘
       ▲                                     │
       │ revive hook                         │ durability<=0
       │                                     ▼
       │                               ┌──────────┐
       └─────────────────────────────  │  Broken  │
              player death              └──────────┘
                                             │
                                             │ RepairWeaponIntent(plan-forge-v1)
                                             ▼
                                        Equipped(恢复 durability)
```

### 2.2 装备槽规则（扩展 inventory-v1 `equipped`）

| 槽位 | 允许 kind | 冲突规则 |
|---|---|---|
| `main_hand` | Weapon（任一 kind） | 排他 |
| `off_hand` | Weapon（single-handed kinds：Dagger / Fist） 或 Treasure | 与 `two_hand` 冲突 |
| `two_hand` | Weapon（两手武器：Spear / Staff） | 占据 main+off |
| `treasure_belt_0..3` | Treasure | 每槽一件 |

### 2.3 装备流程

1. 客户端拖拽 ItemInstance 到 `main_hand` UI 槽
2. 发 `InventoryMoveRequestV1 { instance_id, to: Equipped(main_hand) }`
3. server 校验：
   - ItemTemplate.kind == Weapon
   - 无 two_hand 冲突
   - 角色未 stunned / silenced
4. 通过 → `PlayerInventory.equipped.main_hand = Some(instance_id)` + 插入 `Weapon` component
5. 推 `InventoryEventV1::Moved` + 新 channel `bong:combat/weapon_equipped`（§8.1）
6. 客户端更新 `InventoryStateStore` + `WeaponEquippedStore` → 触发 HeldItemRenderer 重绘

### 2.4 卸下流程

**玩家主动**：
- 拖 ItemInstance 从 main_hand 回背包 → `InventoryMoveRequestV1 { to: Container(...) }`
- server 移除 `Weapon` component + 更新 equipped

**被动**（死亡 drop）：
- 在 `PlayerTerminated` 事件里处理：所有 equipped.* 的 ItemInstance 按 drop table 决定保留 / 掉落
- 默认规则：Weapon durability >= 50% 保留，< 50% 掉落

---

## §3 Inventory 集成

### 3.1 背包格占用

复用 inventory-v1 §1 设计（3 容器 56 格 + 7 装备槽 + 9 hotbar），无需修改容器结构。

武器占格策略：
- 单手武器：1×2（通常竖放）
- 双手武器：2×2（大剑 / 法杖占更多）
- 匕首：1×1
- Treasure：1×1（压缩态，展开时不占格）

### 3.2 hotbar 特殊地位

inventory-v1 的 `hotbar[9]` 对应 MC 原生 1-9 快捷栏。本 plan：
- hotbar[0..8] 放 **消耗品 + 技能卷轴**（plan-HUD-v1 §2.2 上层 F1-F9 快捷使用栏）
- **不放武器** —— 武器走 equipped.main_hand 唯一通道
- MC 原生 `Inventory.selected`（1-9 切换）**保留**，但只换"当前 active 消耗品 / 技能"，不换武器

决策说明：武器切换**不是即时的**（战斗中换剑要 animation），而 MC 原生 1-9 是瞬时的 —— 语义不匹配，所以武器与 hotbar 解耦。

### 3.3 Schema 扩展

`InventorySnapshotV1.equipped` 已有 `main_hand?/off_hand?/two_hand?` 字段（inventory-v1 §3），本 plan 不改 schema，只扩充 ItemInstance 新字段：

```typescript
// extends agent/packages/schema/src/inventory.ts
InventoryItemV1 {
  ...existing
  durability?: number          // weapon 特有
  soul_bond?: {
    character_id: string
    bond_level: 0 | 1 | 2 | 3
    bond_progress: number      // [0, 1]
  }
}
```

---

## §4 原生 UI 接管

### 4.1 三个 Mixin 清单

| 原生路径 | 替代 | Mixin 类 |
|---|---|---|
| `InGameHud.renderHotbar` | `BongHotbarHudPlanner`（见 §4.3） | `mixin/MixinInGameHud.java` |
| 按 E 打开 `InventoryScreen` | `InspectScreen`（背包 tab） | `mixin/MixinKeyboardInputOrClientPlayer.java` |
| `HeldItemRenderer.renderItem`（第一人称 / 第三人称持握） | 按 Weapon component 查自定义 model | `mixin/MixinHeldItemRenderer.java` |

### 4.2 关闭原生 hotbar

```java
@Mixin(InGameHud.class)
public class MixinInGameHud {
    @Inject(method = "renderHotbar", at = @At("HEAD"), cancellable = true)
    private void bong$replaceHotbar(float tickDelta, DrawContext context, CallbackInfo ci) {
        ci.cancel();  // Bong 自己的 BongHotbarHudPlanner 接管
    }
}
```

注册到 `bong-client.mixins.json`。

### 4.3 新 `BongHotbarHudPlanner`（取代原生底部栏）

沿用 `plan-HUD-v1.md §2.2` 规范：
- 下层（MC `Inventory.selected` 1-9）：画消耗品/技能栏（9 格 × 60px）
- 上层（F1-F9）：plan-HUD-v1 已有 `QuickBarHudPlanner`
- **新增**：左端 `main_hand` + `off_hand` 武器/法宝持握槽（60×120px），显示当前装备图标 + durability 环 + soul_bond 等级徽章

视觉布局：

```
┌──[main_hand]─┐ ┌──F1──F2──F3──F4──F5──F6──F7──F8──F9──┐
│ 铁剑 ⚔ 85%  │ │ [消耗品/技能快捷使用栏]               │
│ soul: ♦♦    │ └──────────────────────────────────────┘
├──[off_hand]─┤ ┌──1───2───3───4───5───6───7───8───9──┐
│  符箓 𝕊     │ │ [MC Inventory.selected 消耗品栏]   │
└─────────────┘ └──────────────────────────────────────┘
```

### 4.4 关闭原生 E 键 InventoryScreen

两个可选路径：

**方案 A**（推荐）：Mixin `ClientPlayerEntity.openInventory`

```java
@Mixin(ClientPlayerEntity.class)
public class MixinClientPlayerEntity {
    @Inject(method = "openInventory", at = @At("HEAD"), cancellable = true)
    private void bong$routeToInspect(CallbackInfo ci) {
        MinecraftClient.getInstance().setScreen(new InspectScreen(InspectTab.Inventory));
        ci.cancel();
    }
}
```

**方案 B**：Mixin `KeyboardInput` 或 `MinecraftClient.handleInputEvents`（更广泛但风险高）

用 A。保留 ESC 关闭、容器方块（箱子 / 炼丹炉）的 ScreenHandler 机制 —— 只拦**自身 inventory**打开。

### 4.5 保留的原生行为

- `Inventory.selected` 1-9 的模型选中逻辑（只替换视觉，不换机制）
- 容器方块（chest）打开 ScreenHandler
- Hotkey A/D/W/S 走位
- F3 debug

---

## §5 主手 3D 渲染

### 5.1 第一人称（`HeldItemRenderer`）

```java
@Mixin(HeldItemRenderer.class)
public class MixinHeldItemRenderer {
    @Inject(
        method = "renderItem(Lnet/minecraft/entity/LivingEntity;Lnet/minecraft/item/ItemStack;...)V",
        at = @At("HEAD"),
        cancellable = true
    )
    private void bong$customWeaponModel(
        LivingEntity entity, ItemStack stack, ModelTransformationMode mode,
        boolean leftHanded, MatrixStack matrices, VertexConsumerProvider vertexConsumers,
        int light, CallbackInfo ci
    ) {
        if (!(entity instanceof PlayerEntity)) return;
        UUID uuid = entity.getUuid();
        WeaponSnapshot weapon = WeaponEquippedStore.instance().weaponFor(uuid);
        if (weapon == null) return;  // 让原生 fallback（赤手 / 吃东西 / vanilla 物品）
        BongWeaponRenderer.render(weapon, mode, leftHanded, matrices, vertexConsumers, light);
        ci.cancel();
    }
}
```

### 5.2 第三人称（他人持握）

MVP：**不改**。他人看玩家的武器仍走 vanilla `ItemStack` 渲染（要求武器 ItemInstance → fake ItemStack 转换，picked up as plan-forge-v2 的事）。
后续：另一 Mixin 拦 `PlayerEntityRenderer` 的手部物品 feature layer。

### 5.3 武器模型资源

- 位置：`client/src/main/resources/assets/bong/models/weapon/*.json`
- 格式：Blockbench 导出的 MC model JSON（沿用 Fabric 标准加载器）
- 命名：按 template_id：`iron_sword.json` / `spirit_saber.json` / ...
- 加载：`BongWeaponModelRegistry` 启动扫描 + mc texture manager

### 5.4 第一人称持握 transform

按 WeaponKind 分类的默认 transform（可被 model JSON 覆盖）：

| WeaponKind | 手部位置 | 朝向 | 大小 |
|---|---|---|---|
| Sword | 拳心 | 剑锋朝前上 | 1.0 |
| Saber | 拳心 | 刀锋朝前下 | 1.0 |
| Staff | 拳心偏下 | 杖头朝上 | 1.3 |
| Spear | 拳心偏后 | 枪尖朝前 | 1.5 |
| Dagger | 反握拳心 | 刃朝下 | 0.6 |
| Fist | 拳套外包 | — | 1.0 |
| Bow | 拳心 | 弓身垂直 | 1.2 |

---

## §6 Combat 联动

### 6.1 伤害加成公式

```
final_damage = base_damage
             × weapon_attack_multiplier(weapon)
             × quality_multiplier(weapon.quality_tier)
             × durability_factor(weapon.durability / weapon.durability_max)
             × soul_bond_multiplier(weapon.soul_bond)
```

| 因子 | 公式 | 说明 |
|---|---|---|
| `weapon_attack_multiplier` | `max(1.0, weapon.base_attack / 10.0)` | 无武器（Weapon=None）= 1.0（拳套基线） |
| `quality_multiplier` | tier 0→1.0 · 1→1.15 · 2→1.35 · 3→1.6 | 品阶四档 |
| `durability_factor` | `0.5 + 0.5 × (dur / dur_max)` | 残破武器保底 50% 威力 |
| `soul_bond_multiplier` | level 0→1.0 · 1→1.05 · 2→1.12 · 3→1.25 | 未绑定 1.0 |

### 6.2 插桩位置

`server/src/combat/resolve.rs::resolve_attack_intents` 里查 attacker 的 `Weapon` component：

```rust
let weapon = weapons.get(intent.attacker).ok();
let weapon_mul = weapon.map(weapon_damage_multiplier).unwrap_or(1.0);
let damage = base_damage * weapon_mul * ...;

// 命中后扣 durability
if let Some(mut w) = weapons.get_mut(intent.attacker).ok() {
    w.durability = (w.durability - WEAPON_HIT_DURABILITY_COST).max(0.0);
    if w.durability <= 0.0 {
        weapon_broken_events.send(WeaponBroken { entity: intent.attacker, instance_id: w.instance_id });
    }
}
```

### 6.3 WeaponBroken 事件处理

- `Weapon` component 被移除
- `PlayerInventory.equipped.main_hand = None`（ItemInstance 仍在，durability=0，可送修）
- 推 `WeaponBrokenV1` payload → 客户端弹 HUD 通知 + 边缘红闪一次
- 后续修复走 `plan-forge-v1.md` 的 `RepairWeaponIntent`

---

## §7 SoulBond（灵魂绑定）

### 7.1 触发时机

- 玩家用该武器造成伤害 ≥ 100 点累计 → bond_progress += 0.01
- 玩家用该武器完成一次突破（breakthrough success） → bond_progress += 0.15
- 玩家用该武器击杀一个高境界敌人 → bond_progress += 0.10
- bond_progress 达 1.0 → bond_level += 1，progress 归零

### 7.2 未绑定 → 首次绑定

玩家拾起未绑定武器后，第一次用它造成伤害 → soul_bond = Some({ character_id: self, level: 0, progress: 0.0 })。此后其他角色不能完全发挥该武器威力（非绑定者使用 × 0.8 乘数）。

### 7.3 解绑

死亡不解绑。手动"祭炼"消耗资源解绑（plan-forge-v1 范围）。

---

## §8 数据契约 + Channel

### 8.1 新增 Channel

| Channel | 方向 | Payload | 频率 |
|---|---|---|---|
| `bong:combat/weapon_equipped` | server → client | `WeaponEquippedV1` | equip/unequip/durability 变化时 |
| `bong:combat/weapon_broken` | server → client | `WeaponBrokenV1` | 事件驱动 |
| `bong:combat/treasure_equipped` | server → client | `TreasureEquippedV1`（骨架） | 变更时 |

### 8.2 Payload

```rust
pub struct WeaponEquippedV1 {
    pub slot: EquipSlot,           // MainHand | OffHand | TwoHand
    pub instance_id: u64,
    pub template_id: String,
    pub weapon_kind: WeaponKind,
    pub durability_current: f32,
    pub durability_max: f32,
    pub quality_tier: u8,
    pub soul_bond: Option<SoulBond>,
}

pub struct WeaponBrokenV1 {
    pub instance_id: u64,
    pub template_id: String,
}
```

### 8.3 新 Intent（C2S）

扩展 inventory-v1 `InventoryMoveRequestV1` 足够覆盖装备 / 卸下（to 字段指 `Equipped(main_hand)` 即视为装备）。

新 Intent：

| Intent | 触发 | Payload |
|---|---|---|
| `DropWeaponIntent` | 玩家 Q 键（扔武器到地上） | `{ instance_id }` |
| `RepairWeaponIntent` | plan-forge-v1 工作站 | `{ instance_id, station_pos }` |

---

## §9 新增 Store（client）

| Store | 职责 | Channel |
|---|---|---|
| `WeaponEquippedStore` | 当前 main_hand / off_hand / two_hand 武器快照 | `bong:combat/weapon_equipped` |
| `TreasureEquippedStore` | 腰带 4 槽法宝 | `bong:combat/treasure_equipped` |
| `BongWeaponModelRegistry` | template_id → 加载的 BakedModel 映射 | 本地资源扫描 |

---

## §10 初版武器物品清单（MVP 7 把）

| template_id | 显示名 | kind | tier | base_attack | durability | 占格 | 说明 |
|---|---|---|---|---|---|---|---|
| `iron_sword` | 铁剑 | Sword | 0 | 8.0 | 200 | 1×2 | 起手凡铁 |
| `bronze_saber` | 青铜刀 | Saber | 0 | 9.0 | 180 | 1×2 | 起手凡铁 |
| `wooden_staff` | 木杖 | Staff | 0 | 5.0 | 150 | 1×3 | qi 技能加成 |
| `bone_dagger` | 骨刀 | Dagger | 0 | 6.0 | 120 | 1×1 | 轻武，速攻 |
| `hand_wrap` | 拳套 | Fist | 0 | 3.0 | 300 | 1×1 | 补丁式 |
| `spirit_sword` | 灵剑 | Sword | 1 | 14.0 | 400 | 1×2 | 第一件灵器 |
| `flying_sword_feixuan` | 飞玄剑 | Sword | 2 | 22.0 | 600 | 1×2 | 可绑定后"出窍"（骨架，展开业务留 plan-treasure-v1） |

贴图：7 张（AI 生成，参考 `local_images/generation_guide.md` 的物品图规范）。
模型：7 个 Blockbench JSON。

---

## §11 实施节点（W 阶段）

| Phase | 内容 | 工作量（天） | 依赖 |
|---|---|---|---|
| **W0 plan 文档** | 本文档 | 1 | — |
| **W1 Weapon component + schema** | §1.3 + §8.2 `WeaponEquippedV1` + `WeaponBroken` 事件 + TOML 扩展 | 1 | — |
| **W2 装备/卸下 gameplay** | inventory-v1 `InventoryMoveRequest` 处理 filling + §2 状态机 | 1.5 | W1 + inv-v1 P2 |
| **W3 Mixin 关原生 UI** | §4.2 + §4.4 两个 Mixin + InspectScreen 背包 tab 接管 | 1 | — |
| **W4 BongHotbarHudPlanner** | §4.3 自定义 hotbar 渲染替代原生 | 1.5 | W3 |
| **W5 主手 3D 渲染** | §5 HeldItemRenderer Mixin + 2-3 把武器模型 | 2.5 | W1 |
| **W6 战斗加成 + 耐久** | §6 resolve 插桩 + §6.3 WeaponBroken 处理 | 1 | W1 + W5 |
| **W7 SoulBond gameplay** | §7 触发累加 + 等级跃迁 + 跨人使用惩罚 | 1 | W6 |
| **W8 武器物品清单 + 资源** | §10 7 把武器 TOML + 7 贴图 + 3-7 Blockbench 模型 | 2 | W5 |

**MVP 路径**（W1 + W2 + W3 + W5（仅 1 把占位模型）+ W6）：≈ 7 天

**完整路径**（W1-W8）：≈ 12-13 天

---

## §12 已定案 / 开放问题

### 12.1 已定案

1. **双层模型**：ItemInstance + Weapon Component（§1.2 / §1.3）
2. **Weapon 与 Treasure 分两 flow**：Weapon 握手、Treasure 腰带+展开（§0）
3. **3 个 Mixin 接管原生**：hotbar / inventory key / HeldItemRenderer（§4）
4. **武器不上 MC Item 注册**：纯 ItemInstance + template_id（§0）
5. **hotbar 不放武器**：武器走 equipped.main_hand 唯一通道（§3.2）
6. **第三人称持握 MVP 不做**：仅第一人称，他人看走 vanilla fallback（§5.2）
7. **赤手可战**：Weapon=None 时伤害 × 1.0（§6.1）
8. **Treasure 业务留 plan-treasure-v1**：本 plan 只占槽（§1.4）

### 12.2 开放问题

1. **武器模型格式**：Blockbench JSON vs OBJ？**倾向 Blockbench JSON**（与 MC 原生加载器兼容，Blockbench 直出）
2. **武器图标来源**：AI 生成 vs 手绘？**沿用 `local_images/generation_guide.md` AI 流程**
3. **耐久归零能否修复**：**能**，但需完整耐久度 30% 以上，且需 plan-forge-v1 工作站（细节待 forge plan）
4. **SoulBond 跨服持久化**：character_id 是服务端 UUID，跨会话保留（依赖 plan-persistence-v1）
5. **掉落分布**：death drop table 具体规则（durability ≥ 50% 保留）在 plan-death-lifecycle-v1 确认
6. **武器技能**：tier 2+ 武器是否内置 Technique？留 plan-skill-v1
7. **双武器并持**（双刀流）：是否允许同时 main_hand + off_hand 都是 Weapon？**v1 允许**（仅 Dagger / Fist 占 off_hand）
8. **Bow 弹药**：Bow 吃箭 Item 吗？**v1 不做 ranged**，Bow 只做骨架

---

## §13 验收标准

- ✓ 铁剑 ItemInstance 在起手 loadout 中，背包打开能看到
- ✓ 拖到 main_hand 后，`Weapon` component 被插入玩家 Entity
- ✓ 拖回背包后，component 被移除
- ✓ 按 E 不再打开 `InventoryScreen`，而是 `InspectScreen(背包 tab)`
- ✓ 原生底部 hotbar 完全不渲染，`BongHotbarHudPlanner` 接管
- ✓ 主手装备后第一人称看到 3D 武器模型
- ✓ 无武器时第一人称看到手
- ✓ 左键攻击带武器加成（铁剑对比赤手：伤害 × 1.2 以上）
- ✓ 连击 400 次（200 耐久 × 0.5/击）后武器损坏，HUD 弹通知
- ✓ 同玩家用同武器累计 100 伤害后 bond_progress +0.01（可查 InspectScreen · 武器详情）
- ✓ 跨会话保留装备（重进游戏仍主手有剑）

---

## §14 交叉引用

- 父：`plan-combat-no_ui.md §6`（Weapon/Treasure 原始设计）
- 依赖：`plan-inventory-v1.md §1 / §2 / §3`（ItemInstance / 装备槽 / snapshot）
- 协作：`plan-HUD-v1.md §2.2`（战斗快捷栏 · 本 plan §4.3 扩展）
- 协作：`plan-particle-system-v1.md §4.1`（武器命中 VFX · 未来绑定 weapon_kind → event_id）
- 协作：`plan-forge-v1.md`（修复、锻造、祭炼解绑）
- 后续：`plan-treasure-v1.md`（飞剑展开 Entity / 符箓投掷 / 阵法布置）
- 后续：`plan-skill-v1.md`（武器内置 Technique）
