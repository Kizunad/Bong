# Bong · plan-hotbar-modify-v1 · 骨架

**快捷栏双行重构**：明确 1–9（战斗技能行）与 F1–F9（物品快捷使用行）的分工，让爆脉流等战斗流派技能可以直接绑在 1–9 行上用最顺手的手指出招。

**世界观锚点**：无（纯 UX/交互设计 plan）

**交叉引用**：`plan-HUD-v1.md §2.2` · `plan-combat-ui_impl.md` · `plan-skill-v1.md` · `plan-baomai-v1.md`（首个接入的战斗流派）

**依赖现有实现**（重点：现网 F1–F9 是 *运行时* 已实装但 *持久化* 未接通）：
- 运行时绑定：`server/src/combat/components.rs:303` `QuickSlotBindings` Component（`slots: [Option<u64>; 9]` instance_id + `cooldown_until_tick: [u64; 9]`）
- 持久化字段：`server/src/player/state.rs:53` `PlayerUiPrefs.quick_slots: [Option<String>; 9]` —— 当前是 **dead field**：`handle_quick_slot_bind` 不写回，登录也不恢复 `QuickSlotBindings`，玩家重连绑定丢失（P0 顺手补，详见 §8）
- Rust 协议：`server/src/schema/client_request.rs:141/147` `UseQuickSlot {v, slot}` / `QuickSlotBind {v, slot, item_id: Option<String>}`
- TS 协议：`agent/packages/schema/src/client-request.ts` 的 `ClientRequestV1` Union **缺失** `UseQuickSlot` / `QuickSlotBind`（client 直发 JSON 无双端校验，P0 顺手补）
- 服务端消费：`server/src/network/client_request_handler.rs:1139` `handle_use_quick_slot` / `:1303` `handle_quick_slot_bind`（后者把 template_id → 背包内首个匹配 instance_id）
- Cast 管线：`server/src/combat/components.rs:262` `Casting` Component + `server/src/network/cast_emit.rs:54` `tick_casts_or_interrupt` system（含受击中断 / 控制中断 / 移动中断阈值 0.3m）
- 冷却同步：`server/src/network/quickslot_config_emit.rs:32` `emit_quickslot_config_payloads` 监听 `Changed<QuickSlotBindings>` 推 `QuickSlotConfigV1`（含 `cooldown_until_ms`）
- 1–9 hotbar 存储：`server/src/inventory/mod.rs:113` `PlayerInventory.hotbar: [Option<ItemInstance>; 9]` —— **server-driven 而非 MC 原生 `Inventory.selected`**；client 端 `InspectScreen.hotbarStrip` 是镜像
- Client F1–F9 实现：`client/.../hud/QuickBarHudPlanner.java`（HUD 渲染）+ `combat/QuickUseSlotStore.java`（store）+ `network/QuickSlotConfigHandler.java`（payload handler）+ `inventory/InspectScreen.java:545` `buildQuickUseStrip()`（配置 strip，**不是独立 tab** —— 现网 InspectScreen tab 只有 `["装备", "修仙", "技艺"]`，strip 常驻所有 tab 之上）
- 已学功法 UI：`client/.../combat/inspect/TechniquesListPanel.java` —— **空骨架已存在**但未挂上 InspectScreen，无 server payload 数据流

---

## §0 设计轴心

- [ ] **1–9 行 = 战斗技能行**：最顺手（WASD 正上方），绑爆脉五式等战斗技能，按键直接出招
- [ ] **F1–F9 行 = 物品快捷使用行**：绑丹药/绷带/暗器等消耗品，有 cast time
- [ ] **两行都可混绑**：1–9 也可以放物品（复用现网 server-driven hotbar 切换路径），F1–F9 也可放无 cast 技能
- [ ] **互不冲突**：1–9 触发走一条通道，F1–F9 走另一条，同槽位可以各自绑定不同东西
- [ ] **配置路径统一**：InspectScreen 内拖拽配置——1–9 行新增「战斗·修炼」tab（五区联动工作台，详见 §4）；F1–F9 行沿用现网 `quickUseStrip`（不动）

---

## §1 当前状态审计

### 1.1 已实装

| 行 | 键 | 用途 | 配置方式 | 协议 | 存储 |
|---|---|---|---|---|---|
| **下层** | 1–9 | server-driven hotbar：武器/物品切换（client `Inventory.selected` 上报 server，*非纯 MC 原生*） | E 键背包拖拽（现网） | `inventory_move_intent`（hotbar location） | `PlayerInventory.hotbar: [Option<ItemInstance>; 9]`（inventory/mod.rs:113） |
| **上层** | F1–F9 | 快捷使用：丹药/绷带等 consumable，含 cast time | InspectScreen 内 `quickUseStrip` 左侧 strip（**非独立 tab**） | `QuickSlotBind` / `UseQuickSlot`（Rust 已实装、TS schema 缺失） | 运行时 `QuickSlotBindings` Component（持久化 `PlayerUiPrefs.quick_slots` 是 dead field） |

### 1.2 缺失

**绑定通路**：

- 1–9 行**不支持绑定技能**（只能放物品 instance）
- F1–F9 行**只支持 consumable**，不支持技能绑定
- 两行之间**没有技能绑定通路**
- 没有「从已学功法列表拖技能到快捷栏」的 InspectScreen tab

**信息整合（本次升级补齐）**：

- **没有功法详情卡**：列表项只显示 Grade + 名称，描述/真元消耗/cast/cooldown/经脉需求等运行时数据无 UI 承接（玩家盲拖）
- **经脉视图与功法选中无联动**：现网 `BodyInspectComponent` 仅在「修仙」tab 独立展示完整 12 正经 + 8 奇经，不能"根据当前选中功法高亮所需经脉"——配置技能时玩家无法直观看到"这招走哪条经脉、这条经脉是不是 SEVERED"
- **技艺（HERB/ALCH/FORG）与战斗 Hotbar 视图分离**：技艺虽不入战斗 Hotbar，但配置战斗技能时仍需共显（如未来"锻造影响某武器流派可绑性"）；当前 `SkillSetSnapshot` 只在「技艺」tab 独立 lv+xp 渲染，无统一工作台
- **境界 / XP / 真元等全局状态在配置面板上不可见**：玩家拖功法时若不切回「修仙」tab，看不到当前真元够不够、境界达没达——需求决策与状态反馈分离

### 1.3 计划中的但未实装

- `InspectScreen` 「已学功法」tab — **`TechniquesListPanel.java` 空骨架已存在**（`combat/inspect/`，含 `Grade` enum 占位），但 InspectScreen 内无引用、无数据流、`techniques_snapshot` server payload 也未定义
- 功法列表渲染 + 拖拽到快捷栏（plan-HUD-v1 §10）
- F1–F9 持久化（现网 `PlayerUiPrefs.quick_slots` 是 dead field — P0 顺手补，见 §8）

---

## §2 双行新分工

```
         ┌─────────────────────────────────────────┐
         │ [F1 回血丹] [F2 回真元] [F3 解毒] ...    │ ← 上层 F1–F9 · 物品快捷使用
         │ [ 1 崩拳 ] [ 2 贴山靠] [ 3 血崩步] ...  │ ← 下层 1–9 · 战斗技能行
         └─────────────────────────────────────────┘
         手指位置:         WASD 正上方！
         1-2-3-4-5 无需离开移动区
```

### 2.1 下层 1–9：战斗技能行（主战场）

| 槽位 | 绑定类型 | 按键行为 | cast time |
|---|---|---|---|
| 物品（武器/工具/方块）| 按 X = 切到手持（server-driven hotbar，§3.3）| 无 |
| **技能**（崩拳/贴山靠/…）| 按 X = **直接出招** | 有（招式前摇 anim） |
| 空 | 按 X = 无操作 | — |

**关键**：技能槽不经过「切手持」——按 1 直接打崩拳，不需要先切换到某个状态再左键。这是和现网 1–9 物品 hotbar 最大的区别。

### 2.2 上层 F1–F9：物品快捷使用行（消耗品/工具）

保持现有逻辑不变：
- 绑丹药/绷带/暗器/道具 → 按 F-key 触发 `UseQuickSlot`，有 cast time + cooldown
- 也可以绑无 cast 的技能（如逆脉护体 toggle）
- 配置方式仍为 InspectScreen 拖拽

### 2.3 两行对比

| 属性 | 下层 1–9 | 上层 F1–F9 |
|---|---|---|
| 核心定位 | 战斗技能 + 武器切换 | 消耗品快捷使用 |
| 按键位置 | 最顺手（WASD 上）| 次顺手（需伸手）|
| 技能出招 | ✅ 直接出招 | ✅ 也可绑技能 |
| 物品使用 | ✅ 切手持（server-driven hotbar）| ✅ 有 cast time |
| cast time | 有（技能前摇）| 有（物品读条）|
| 冷却显示 | 槽位蒙灰 + 倒计时 | 槽位蒙灰 + 倒计时 |
| 配置方式 | 新增 InspectScreen「战斗·修炼」tab（五区联动工作台：已学列表+详情卡+经脉缩略+状态条+1–9 槽，详见 §4） | 现网 `quickUseStrip`（左侧 strip，**非独立 tab**） |
| 存储 | 运行时 `SkillBarBindings` Component（新增；mirror `QuickSlotBindings`）+ `PlayerUiPrefs.skill_bar`（持久化新增） | 运行时 `QuickSlotBindings` Component（已有）+ `PlayerUiPrefs.quick_slots`（**当前 dead field**，P0 顺手补） |

---

## §3 数据模型

### 3.1 1–9 技能栏存储

**双层结构**（mirror 现网 `QuickSlotBindings` + `PlayerUiPrefs.quick_slots` 应有的双层模型）：

```rust
// === 运行时（ECS Component，参考 combat/components.rs:303 QuickSlotBindings） ===
// server/src/combat/components.rs 新增
#[derive(Debug, Clone, Component, Default)]
pub struct SkillBarBindings {
    pub slots: [SkillSlot; 9],
    /// 每个 slot 下次可用的 server tick；0 表示无冷却（与 QuickSlotBindings 一致）。
    pub cooldown_until_tick: [u64; 9],
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum SkillSlot {
    /// 空槽
    #[default]
    Empty,
    /// 绑定背包物品；按 1–9 走现网 server-driven hotbar 切换路径（不发 SkillBarCast）
    Item { instance_id: u64 },
    /// 绑定战斗技能；按 1–9 直接发 SkillBarCast 出招
    Skill { skill_id: String },  // e.g. "burst_meridian.beng_quan"
}

// === 持久化（DB → ECS，登录恢复） ===
// server/src/player/state.rs PlayerUiPrefs 扩展
// 注意：现网 PlayerUiPrefs 是 *private struct*（state.rs:51 无 pub）；
//   新增 skill_bar 字段需要：
//   (1) 改 pub(crate) 或暴露 getter，让 player::mod 登录路径可读
//   (2) 同步路径：handle_skill_bar_bind 写 SkillBarBindings → 落 player_ui_prefs.prefs_json
//   (3) on_login 读 prefs_json → 注入 SkillBarBindings Component（同地点 mirror QuickSlotBindings 注入）
//   (4) P0 顺手修复 quick_slots dead field（同款 sync 路径）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub(crate) struct PlayerUiPrefs {
    pub quick_slots: [Option<String>; 9],         // 已有；P0 补 sync 路径
    pub skill_bar: [SkillSlotPersist; 9],          // 新增
}

/// 持久化变体：Item 存 template_id（不存 instance_id —— 重连后 mirror
/// handle_quick_slot_bind 同款逻辑，从背包内首个匹配 template 解析 instance）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum SkillSlotPersist {
    #[default]
    Empty,
    Item { template_id: String },
    Skill { skill_id: String },
}
```

### 3.2 协议扩展

**TS schema 同时补齐现网遗漏**：本 plan 顺手把现网 `UseQuickSlot` / `QuickSlotBind`（仅 Rust 侧实装、TS 缺失）一并加到 `ClientRequestV1` Union，确保双端校验恢复（plan-HUD-v1 遗留）。

**ClientRequestV1 新增**：

```typescript
// agent/packages/schema/src/client-request.ts

// (1) 顺手补齐现网遗漏
export const UseQuickSlotRequestV1 = Type.Object({
  v: Type.Literal(1),
  type: Type.Literal("use_quick_slot"),
  slot: Type.Integer({ minimum: 0, maximum: 8 }),
}, { additionalProperties: false });

export const QuickSlotBindRequestV1 = Type.Object({
  v: Type.Literal(1),
  type: Type.Literal("quick_slot_bind"),
  slot: Type.Integer({ minimum: 0, maximum: 8 }),
  /// 与 Rust `Option<String>` 对齐：null = 清空，string = template_id
  item_id: Type.Union([Type.Null(), Type.String()]),
}, { additionalProperties: false });

// (2) 本 plan 新增
export const SkillBarCastRequestV1 = Type.Object({
  v: Type.Literal(1),
  type: Type.Literal("skill_bar_cast"),
  slot: Type.Integer({ minimum: 0, maximum: 8 }),
  /// 可选：崩拳/贴山靠等需要指定 target entity_id
  target: Type.Optional(Type.String()),
}, { additionalProperties: false });

/// binding 用 union 而不是字符串前缀（与现网 `QuickSlotBind.item_id: Option<String>`
/// 的 None-as-empty 模式对齐扩展：null=清空 / item kind / skill kind）。
export const SkillBarBindingV1 = Type.Union([
  Type.Null(),
  Type.Object({ kind: Type.Literal("item"), template_id: Type.String() }, { additionalProperties: false }),
  Type.Object({ kind: Type.Literal("skill"), skill_id: Type.String() }, { additionalProperties: false }),
]);

export const SkillBarBindRequestV1 = Type.Object({
  v: Type.Literal(1),
  type: Type.Literal("skill_bar_bind"),
  slot: Type.Integer({ minimum: 0, maximum: 8 }),
  binding: SkillBarBindingV1,
}, { additionalProperties: false });
```

**Rust 侧对应**：

```rust
// server/src/schema/client_request.rs ClientRequestV1 新增
SkillBarCast { v: u8, slot: u8, target: Option<String> },
SkillBarBind { v: u8, slot: u8, binding: SkillBarBindingV1 },

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SkillBarBindingV1 {
    Item { template_id: String },
    Skill { skill_id: String },
    // None = 清空（serde Option<SkillBarBindingV1> 在 ClientRequestV1.binding 用 Option 包裹）
}
// 实际上 ClientRequestV1::SkillBarBind.binding 用 Option<SkillBarBindingV1>
// 与 QuickSlotBind.item_id: Option<String> 对称。
```

### 3.3 按键映射

| 客户端按键 | 发送的协议 |
|---|---|
| 1–9 | `SkillBarCast { slot: 0..8 }` |
| F1–F9 | `UseQuickSlot { slot: 0..8 }`（现有，不变）|

**兼容**：当 1–9 槽位绑的是物品（`SkillSlot::Item`）时，按 1–9 退化回**现网 server-driven hotbar 切换路径**——client 端 mixin 不 cancel 该 keypress，让 MC `Inventory.selected` 改变事件继续走 `inventory_move_intent`（hotbar location）路径，server 的 `PlayerInventory.hotbar`（inventory/mod.rs:113）按现状响应；不发送 `SkillBarCast`。注意现网 1–9 hotbar **不是纯 MC 客户端原生**，而是 server-driven（client `InspectScreen.hotbarStrip` 是镜像）。

---

## §4 配置流程（InspectScreen ·「战斗 · 修炼」工作台）

### 4.0 设计目标

**单 tab 五区联动工作台**，避免"配技能时要在 修仙/技艺/战斗 多 tab 间反复切"的视图割裂。所有与「绑战斗技能」相关的信息——已学功法、技艺等级、当前选中功法所需经脉、修士全局状态、目标 Hotbar——同框可见。

**视觉设计稿**：`docs/plans-skeleton/plan-hotbar-modify-v1.svg`（800×540 mockup，含五区 + 拖拽流向标注）。实装走 owo-lib `FlowLayout` + 现有 `GridSlotComponent` / `BodyInspectComponent` / `StatusBarsPanel` + 新增组件，不引入 runtime SVG 渲染（client 端无 batik，全部 PNG icon + `DrawContext`）。

### 4.1 五区布局

```
┌── InspectScreen · 「战斗 · 修炼」 tab ─────────────────────────────┐
│ tab: 装备 | 修仙 | 技艺 | [战斗·修炼]              境界 凝脉一层 XP▰▰▱│
├──────────────────────────────────┬────────────────────────────────┤
│ ① 已学列表（拖源）                │ ② 功法详情卡                    │
│   ─ 功法 ─                       │   [icon] 崩拳 · 黄阶 · 0.85    │
│   [崩] 崩拳   黄阶 ▰▰▰▰▰▰▰░ 0.85│   描述 / 需求 / 招式数值        │
│   [靠] 贴山靠 黄阶 ▰▰▰▰▰▰░░ 0.62│   ▸ 真元 30/360 · cast 8t      │
│   [步] 血崩步 玄阶 ░░ 🔒 锁     │   ▸ cd 60t · 射程 1.8m         │
│   [逆] 逆脉护体 ⊙ 维持中          │   ─ 已绑定槽 1（→ 出招）        │
│   ─ 技艺 ─                       │                                │
│   采药 lv2 30%  炼丹 lv4 60%     ├────────────────────────────────┤
│   锻造 lv1 0%  (不可入战斗 Hotbar)│ ④ 修士状态                      │
│                                  │   真元 ▰▰▰▰▰▰▰░ 280/360         │
├──────────────────────────────────┤   体力 ▰▰▰▰▰▰▰▰ 92/100          │
│ ③ 经脉视图（联动选中功法高亮）     │   因果 +0.32 · 综合 1240        │
│   [人体剪影 + 高亮所需经脉]       │   区域 青云脉                    │
│   ▰ 手阳明大肠经 LI · 健康        │                                │
│   ▰ 手少阳三焦经 TE · 健康        │                                │
│   污染 0.12/1.00                 │                                │
├──────────────────────────────────┴────────────────────────────────┤
│ ⑤ 1–9 战斗 Hotbar （拖目标 / 当前编辑层）                          │
│ [1崩][2靠 cd2.1s][3逆 ⊙][4剑][5空][6空][7空][8空][9空+] | F1丹 F2绷│
│ 拖入功法 = 绑技能 · 拖入物品 = server-driven 切手持 · 右键 = 清空   │
└────────────────────────────────────────────────────────────────────┘
```

### 4.2 区域职责

| 区 | 内容 | 数据源 | 复用现有 | 新增组件 |
|---|---|---|---|---|
| **① 列表** | 功法行（Grade + proficiency 进度条 + 锁定态）+ 技艺三行紧凑（HERB/ALCH/FORG lv+xp）；功法可拖入 ⑤，技艺仅展示 | `TechniquesListPanel.snapshot()` + `SkillSetStore.snapshot()` | `SkillSetSnapshot.Entry.progressRatio()` | `TechniqueRowComponent` · `SkillRowComponent` |
| **② 详情卡** | 选中态功法的完整信息：描述、需求（境界 + 经脉）、招式数值（消耗/cast/cd/射程）、已绑定 slot 反查 | `TechniquesSnapshotV1`（详见 §7）+ `SkillBarStore` 反查 | — | `TechniqueDetailCard` |
| **③ 经脉缩略图** | 简化人体剪影 + 高亮"所选功法 `required_meridians`"对应经脉 + legend 列出健康度 | `MeridianBody`（已有完整 20 经数据）+ 详情卡 selected.required_meridians | `BodyInspectComponent` 加 compactMode | `MeridianChannel` 高亮集合参数 |
| **④ 修士状态** | 真元 / 体力 / 因果 / 综合实力 / 区域，紧凑横向条 | `PlayerStateViewModel`（spiritQiCurrent/Max、karma、compositePower、zoneId） | `StatusBarsPanel` 紧凑变体 | — |
| **⑤ 1-9 Hotbar** | 9 槽：技能（黄边）/ 物品（蓝边）/ 空（虚线）/ 冷却蒙灰 / toggle on（绿边小圆点）；右侧只读镜像 F1-F9 | `SkillBarStore`（新）+ `QuickUseSlotStore`（已有） | `GridSlotComponent` 拖拽事件 | `CombatHotbarStrip` · 与 `SkillBarHudPlanner` 共享渲染 |

### 4.3 选中态联动

**关键交互**：① 列表中点击/键盘选中一项 → ② 详情卡刷新 + ③ 经脉图高亮变更——这是单 tab 工作台相对原"加新 tab + 仅列表"方案的核心增益。

```
client 端选中状态机：
  CombatTrainingPanel.selectedTechniqueId: String  // 仅 client 本地，不走网络

  选中变化时：
    fire selectionChanged(techniqueId)
      → TechniqueDetailCard.refresh(snapshot.findById(id))
      → MeridianMiniView.highlightChannels(detail.required_meridians.map(::channel))
      → CombatHotbarStrip.markBoundSlots(SkillBarStore.findBySkillId(id))
        // 让 ⑤ 中已绑定该功法的槽位也描金边，回路视觉提示
```

无需新 server payload —— `selectedTechniqueId` 是纯 client UI 状态。

### 4.4 拖拽流程

```
源：① 列表中拖起一项
  - 功法行 → DragState.source = TECHNIQUE(skill_id)
  - 技艺行 → DragState.source = SKILL_LV（不可拖：drop 即 reject，灰显反馈）
  - 锁定态功法（境界不足 / 经脉 SEVERED）→ drop 即 reject，弹"境界不足"提示

目标：⑤ 1-9 Hotbar 槽（DragState.target = SKILL_BAR(slot)）
  - 命中：本地 SkillBarStore.slots[slot] = Skill { skill_id }（乐观更新）
  - 发 SkillBarBind { slot, binding: { kind: "skill", skill_id } }
  - server 验证（详见 §3.2）→ 推 SkillBarConfigV1 → SkillBarConfigHandler 同步 / 回滚

物品来源：背包 GridSlot 拖入 ⑤
  - 退化回现网 server-driven hotbar 切换路径（§3.3，发 InventoryMoveIntent）
  - SkillBarBind 仅承载 skill 绑定；物品绑定走 InventoryMoveIntent 而非 SkillBarBind
    （这一点与原 §3.1 SkillSlot::Item 设计一致：Item 槽位只是 mirror 现网 hotbar）

右键槽位：清空
  - 发 SkillBarBind { slot, binding: null } → server 写 Empty
```

### 4.5 与现网 InspectScreen 结构的关系

**现状**：tab 列表 `["装备", "修仙", "技艺"]`（`InspectScreen.java:192`）；F1–F9 走 `buildQuickUseStrip()`（line 545）常驻左侧 strip，所有 tab 共享。

**新方案**：
- tab 列表扩为 `["装备", "修仙", "技艺", "战斗·修炼"]`
- 新 tab 内容 = §4.1 五区联动工作台
- 「修仙」tab 完整经脉视图保留不动（玩家点 ③ 缩略图右下角"详情"链接可跳转过去）
- 「技艺」tab 保留不动（① 列表底段只是缩略复读，详情仍在原 tab）
- F1–F9 strip 不迁移、不动；⑤ 区底部仅做镜像展示（强调"两套独立"）

**为什么不合并 修仙/技艺/战斗 三 tab**：
- 「修仙」tab 的 `BodyInspectComponent` 是双层（体表/经脉）+ 12+8 完整经脉 + 滤镜（手经/足经/奇经），UI 占用大；战斗工作台只需要"针对当前功法的小图"
- 「技艺」tab 含残卷消费记录、recent +XP 衰减动画等独立交互，合并会让战斗 tab 过载
- 结论：战斗 tab 复用上述两 tab 的**数据**（store / snapshot），但**视图**简化重组——不是把整个 tab 内容塞过来

---

## §5 按键触发流程

### 5.1 玩家按 1

```
客户端：
  读 SkillBarStore.slots[0] →
    SkillSlot::Skill { skill_id }:
      发 SkillBarCast { slot: 0, target: Some(crosshair_entity_id) }
      cancel 1 键 keypress 防止 MC hotbar 切换
      锁定 1 键输入直到 cast 完成（防止双击；client 端本地 cooldown_until_ms 蒙灰）
      槽位蒙灰 + cast bar 渲染
    SkillSlot::Item { instance_id }:
      不 cancel 1 键，让现网 server-driven hotbar 切换继续（§3.3）
      不发 SkillBarCast
    SkillSlot::Empty: nop

服务端：
  收到 SkillBarCast { slot: 0 } →
    读 SkillBarBindings.slots[0]（运行时 ECS Component）→
      SkillSlot::Skill { skill_id: "burst_meridian.beng_quan" } →
        路由到 burst_meridian consumer（实装由 plan-baomai-v1 提供）：
          验证境界 / 经脉 / 虚脱 / cooldown_until_tick → 通过则
            插 Casting Component（启 cast bar）→
            执行崩拳结算 → 写 cooldown_until_tick →
            回推 BurstMeridianEvent { skill: "beng_quan", anim_duration_ticks: 8, ... }
          cooldown 中：drop（client 端蒙灰已挡按键，无需 server reject payload）
      SkillSlot::Item / SkillSlot::Empty: drop（client mixin 不应发；server 兜底日志告警）
```

### 5.2 玩家按 F1（现有逻辑不变）

```
客户端：
  读 QuickUseSlotStore.slots[0] →
    如果是丹药 → 发 UseQuickSlot { slot: 0 }
    如果绑的是技能 → 同样走 UseQuickSlot（server 侧路由到 skill handler）
```

### 5.3 cast time + cooldown 统一处理

**复用现网两条管线**（不新建第三条）：
- `Casting` Component（combat/components.rs:262）+ `tick_casts_or_interrupt` system（cast_emit.rs:54）：cast 状态机，phase ∈ `Casting | Interrupt | Complete`，含受击中断 / 控制中断 / 移动中断阈值 0.3m
- `Changed<QuickSlotBindings>` 触发 `emit_quickslot_config_payloads`（quickslot_config_emit.rs:32）→ 推 `QuickSlotConfigV1`（含 `cooldown_until_ms`）

1–9 SkillBar 接入方式：
- **出招前摇**：插 `Casting` Component（与 F1–F9 同款），client 端通过 `CastSyncV1` 渲染 cast bar
- **招式后摇 / 冷却**：`tick_casts_or_interrupt` 完成时写 `SkillBarBindings.cooldown_until_tick[slot]`（和现网写 `QuickSlotBindings` 同款），触发 `Changed<SkillBarBindings>`
- **推送**：新增 `network/skillbar_config_emit.rs::emit_skillbar_config_payloads`，mirror `emit_quickslot_config_payloads`，推 `SkillBarConfigV1`（含 `slots` + `cooldown_until_ms`）。**不再单独引入 `SkillBarSyncV1`** —— `SkillBarConfigV1` 一条覆盖 bind/cooldown/login 全场景

---

## §6 与爆脉流的对接

见 `plan-baomai-v1.md §3`。核心路径：

```
InspectScreen「战斗·修炼」tab（§4 五区工作台）
  → 从 ① 已学列表 选中崩拳 → ② 详情卡刷新 + ③ 经脉缩略图高亮 LI/TE
  → 拖崩拳到 ⑤ 1 号槽
    → SkillBarBind {
         slot: 0,
         binding: { kind: "skill", skill_id: "burst_meridian.beng_quan" },
       }

战斗中按 1
  → SkillBarCast { slot: 0, target: Some(crosshair_entity_id) }
    → server 读 SkillBarBindings.slots[0] = SkillSlot::Skill { skill_id: "burst_meridian.beng_quan" }
      → 路由到 burst_meridian consumer（plan-baomai-v1 提供实装）
        → resolve_beng_quan()
          → 臂经脉 integrity 扣减 + qi 消耗
          → BurstMeridianIntent → AttackIntent(过载)
          → BurstMeridianEvent → 客户端播动画+粒子
```

**与现有 `GameplayAction` 的关系**：不需要新增 `GameplayAction` 分支。`SkillBarCast` 在 `client_request_handler` 中直接路由到对应的 skill system（burst_meridian / 暗器 / 阵法 / …），不经过 `GameplayActionQueue` —— 对齐现网 `UseQuickSlot` 处理方式（直接插 `Casting` Component 而非入 queue）。

---

## §7 数据契约

### 7.1 Schema 端

| 文件 | 改动 |
|---|---|
| `agent/packages/schema/src/client-request.ts` | 新增 `SkillBarBindRequestV1` + `SkillBarCastRequestV1`；**同时补齐现网遗漏**的 `UseQuickSlotRequestV1` + `QuickSlotBindRequestV1`；全部加入 `ClientRequestV1` union |
| `agent/packages/schema/src/server-data.ts` | 新增 `SkillBarConfigV1`（mirror 现网 `QuickSlotConfigV1`：`slots` + `cooldown_until_ms`，绑定/冷却/登录全量都走这条；不再引入 `SkillBarSyncV1`，与 §5.3 对齐） |

| `agent/packages/schema/src/server-data.ts`（续） | 新增 `TechniquesSnapshotV1`（§4 详情卡 ② + 经脉缩略图 ③ 的数据来源；entries 含 `id` / `display_name` / `grade` / `proficiency` / `active` / `description` / `required_realm` / `required_meridians: [{channel, min_health}]` / `qi_cost` / `cast_ticks` / `cooldown_ticks` / `range`） |

### 7.2 Server 端

| 文件 | 改动 |
|---|---|
| `server/src/combat/components.rs` | 新增 `SkillBarBindings` Component（mirror `QuickSlotBindings`：`slots: [SkillSlot; 9]` + `cooldown_until_tick: [u64; 9]`）+ `SkillSlot` enum |
| `server/src/inventory/mod.rs:260` | 同地点注入 `SkillBarBindings::default()`（与现网 `QuickSlotBindings` 注入对称）|
| `server/src/player/state.rs` | `PlayerUiPrefs` 改 `pub(crate)`，新增 `skill_bar: [SkillSlotPersist; 9]`；`SkillSlotPersist` 定义；登录时 `prefs_json.skill_bar` → 注入 `SkillBarBindings`；**P0 顺手补**：`prefs_json.quick_slots` → 注入 `QuickSlotBindings`（修复 dead field）|
| `server/src/schema/client_request.rs` | 新增 `SkillBarBind` / `SkillBarCast` 变体 + `SkillBarBindingV1` enum |
| `server/src/network/client_request_handler.rs` | 新增 `handle_skill_bar_bind`（解析 binding union → 写 `SkillBarBindings`）/ `handle_skill_bar_cast`（读 slots[slot]：Skill 路由 / Item 与 Empty 视为 nop）；skill 路由表（skill_id → system fn pointer，初期 mock，真实 `burst_meridian.beng_quan` 由 plan-baomai-v1 P0 接入）；**P0 顺手补**：`handle_quick_slot_bind` 写 Component 时同步 mark prefs dirty |
| `server/src/network/skillbar_config_emit.rs`（新文件）| `emit_skillbar_config_payloads` 监听 `Changed<SkillBarBindings>` → 推 `SkillBarConfigV1`（mirror `quickslot_config_emit.rs`） |

| `server/src/cultivation/skill_registry.rs`（新文件 / 或归 `combat/`）| 静态功法注册表：`skill_id` → metadata（`description`、`required_realm`、`required_meridians`、`qi_cost`、`cast_ticks`、`cooldown_ticks`、`range`、`grade`）；初期 hardcode 4 个示例条目（崩拳 / 贴山靠 / 血崩步 / 逆脉护体），后续由各流派 plan（baomai / anqi / …）owns |
| `server/src/cultivation/known_techniques.rs`（新文件 / 或归 `combat/`）| `KnownTechniques` Component（玩家学过的功法 ids + proficiency）；初期 stub 固定返回 4 个示例 id，由 plan-baomai-v1 P1 接入真实学习/掌握度逻辑 |
| `server/src/network/techniques_snapshot_emit.rs`（新文件）| `emit_techniques_snapshot_payloads` 监听 `Changed<KnownTechniques>` → 推 `TechniquesSnapshotV1`（合并 `KnownTechniques` 的运行时态 + `skill_registry` 的静态 metadata） |

### 7.3 Client 端

| 文件 | 改动 |
|---|---|
| `client/src/main/java/com/bong/client/combat/SkillBarStore.java`（新文件）| 本地镜像 1–9 技能栏（参考 `combat/QuickUseSlotStore.java`） |
| `client/src/main/java/com/bong/client/inventory/InspectScreen.java` | 改 tab 列表为 `{"装备", "修仙", "技艺", "战斗·修炼"}`；新 tab 内容 = `CombatTrainingPanel`（§4 五区工作台）；F1–F9 strip 不动 |
| `client/src/main/java/com/bong/client/combat/inspect/TechniquesListPanel.java` | 改：从空骨架填充 — 接入 `techniques_snapshot` payload + 列表项渲染 |
| `client/src/main/java/com/bong/client/hud/SkillBarHudPlanner.java`（新文件）| 渲染 1–9 行技能栏（槽位图标 + 冷却蒙灰 + 境界锁定灰；参考 `hud/QuickBarHudPlanner.java`）|
| `client/src/main/java/com/bong/client/mixin/KeyBindingMixin.java`（新文件或扩展现有）| 1–9 按键拦截 → 查 `SkillBarStore.slots[n]`：Skill → 发 `SkillBarCast` + cancel keypress；Item / Empty → 不 cancel，让 MC server-driven hotbar 切换继续 |
| `client/src/main/java/com/bong/client/network/SkillBarConfigHandler.java`（新文件）| 接收 `SkillBarConfigV1` → 更新 `SkillBarStore`（参考 `network/QuickSlotConfigHandler.java`）|

| `client/src/main/java/com/bong/client/combat/inspect/CombatTrainingPanel.java`（新文件） | §4 五区工作台主容器（owo `FlowLayout`）；持有 `selectedTechniqueId` 状态 + `selectionChanged` 事件总线；负责 ① 列表 / ② 详情 / ③ 经脉 / ④ 状态 / ⑤ Hotbar 五区拼装与跨区联动 |
| `client/src/main/java/com/bong/client/combat/inspect/TechniqueDetailCard.java`（新文件） | ② 详情卡（描述 / 需求 / 招式数值四宫格 / 已绑定提示），订阅 `selectionChanged` 刷新；从 `TechniquesSnapshotV1` entry 渲染 |
| `client/src/main/java/com/bong/client/combat/inspect/MeridianMiniView.java`（新文件 / 或扩展 `BodyInspectComponent.compactMode`） | ③ 经脉缩略图（compact 模式）：人体剪影 + `highlightChannels(Set<MeridianChannel>)` 接受联动参数；右下角"详情"链接跳转到「修仙」tab 完整经脉视图 |
| `client/src/main/java/com/bong/client/combat/inspect/CombatHotbarStrip.java`（新文件） | ⑤ 1-9 槽渲染 + 镜像 F1-F9（只读）+ 拖目标判定；与 `SkillBarHudPlanner` 共享一份槽位绘制逻辑（抽到 `SkillSlotRenderer` util） |
| `client/src/main/java/com/bong/client/network/TechniquesSnapshotHandler.java`（新文件） | 接收 `TechniquesSnapshotV1` → 调 `TechniquesListPanel.replace(...)`；entry 字段扩展为含 `description` / `required_realm` / `required_meridians` / `qi_cost` / `cast_ticks` / `cooldown_ticks` / `range`（同步扩展 `TechniquesListPanel.Technique` record） |
| `client/src/main/java/com/bong/client/inventory/state/DragState.java` | 扩展 `SourceKind.TECHNIQUE`（带 `skill_id`）/ `TargetKind.SKILL_BAR(slot)`；技艺行为 `SourceKind.SKILL_LV` 但 drop 即 reject（不可入战斗 Hotbar） |

---

## §8 实施节点

### P0 · 1–9 技能栏基础（目标：1 周）

```
Schema：
  - [ ] 顺手补：TS client-request.ts 新增 UseQuickSlotRequestV1 + QuickSlotBindRequestV1（修复现网遗漏）
  - [ ] SkillBarBindRequestV1 + SkillBarCastRequestV1 TypeBox 定义（binding 用 union: null | item | skill）
  - [ ] SkillBarConfigV1 server-data TypeBox（mirror QuickSlotConfigV1）
  - [ ] Rust side ClientRequestV1 新增 SkillBarBind / SkillBarCast 变体 + SkillBarBindingV1 enum
Server：
  - [ ] combat/components.rs 新增 SkillBarBindings Component（mirror QuickSlotBindings）+ SkillSlot enum
  - [ ] inventory/mod.rs:260 同地点注入 SkillBarBindings::default()
  - [ ] PlayerUiPrefs 改 pub(crate) + 新增 skill_bar: [SkillSlotPersist; 9] 字段
  - [ ] handle_skill_bar_bind：解析 binding union → 写 SkillBarBindings.slots[slot]（Skill: skill_id 注册校验；Item: template_id → 背包首个匹配 instance；None: Empty）
  - [ ] handle_skill_bar_cast：读 slots[slot] → Skill 路由到 skill system / Item 视为 nop / Empty 视为 nop
  - [ ] 路由表：skill_id → system fn pointer（初期 mock，真实 burst_meridian.beng_quan 由 plan-baomai-v1 P0 接入）
  - [ ] network/skillbar_config_emit.rs 新增 emit_skillbar_config_payloads（mirror quickslot_config_emit.rs）
  - [ ] 持久化：登录时读 prefs_json.skill_bar → 注入 SkillBarBindings；handle_skill_bar_bind 写 Component 时同步 mark prefs dirty
  - [ ] 顺手补（P0 必做）：现网 PlayerUiPrefs.quick_slots dead field 修复——
        handle_quick_slot_bind 写 QuickSlotBindings 时同步写 quick_slots（template_id），
        登录时读 prefs_json.quick_slots 注入 QuickSlotBindings
客户端：
  - [ ] combat/SkillBarStore.java（本地镜像 9 槽，参考 QuickUseSlotStore）
  - [ ] mixin/KeyBindingMixin.java：1–9 按键拦截 → 查 SkillBarStore.slots[n]：
        Skill → 发 SkillBarCast + cancel keypress；Item/Empty → 不 cancel，让 MC server-driven hotbar 切换继续
  - [ ] hud/SkillBarHudPlanner.java：渲染 1–9 槽（测试用崩拳图标 + 冷却蒙灰；参考 QuickBarHudPlanner）
  - [ ] network/SkillBarConfigHandler.java：接收 SkillBarConfigV1 → 更新 SkillBarStore（参考 QuickSlotConfigHandler）
测试：
  - [ ] 按 1 → client 发 SkillBarCast { slot: 0 } → server 路由到 mock handler
  - [ ] SkillBarBind { slot: 0, binding: { kind: "skill", skill_id: "..." } } → 重启进程不丢（先验证 prefs 持久化，再验证登录恢复 SkillBarBindings）
  - [ ] 回归：F1–F9 quick slot 重启后不丢（验证顺手补的 quick_slots 持久化）
  - [ ] TS↔Rust schema 对称：UseQuickSlot/QuickSlotBind/SkillBarBind/SkillBarCast 双端 sample 对拍
```

### P1 · InspectScreen 五区联动工作台（目标：1.5 周）

按 §4 五区拆五子任务，建议按顺序推进（每子任务独立可见效，便于阶段性 review）：

**P1.a · Schema + Server techniques_snapshot 链路**（解锁后续 UI 数据源）
```
  - [ ] TypeBox: TechniquesSnapshotV1 + TechniqueEntryV1 + TechniqueRequiredMeridianV1
  - [ ] Rust: ServerDataV1 加入 TechniquesSnapshot 变体
  - [ ] cultivation/skill_registry.rs：4 个示例条目（崩拳 / 贴山靠 / 血崩步 / 逆脉护体）
        必填 metadata: description / required_realm / required_meridians / qi_cost / cast_ticks / cooldown_ticks / range / grade
  - [ ] cultivation/known_techniques.rs：KnownTechniques Component (stub: 4 个 id 全已学，proficiency 初始 0.5)
  - [ ] network/techniques_snapshot_emit.rs：merge KnownTechniques + skill_registry → 推 TechniquesSnapshotV1
  - [ ] client: TechniquesSnapshotHandler.java + 扩展 TechniquesListPanel.Technique record
  - [ ] 测试：登录后 client 收到 4 条 entries，TechniquesListPanel.snapshot() 非空
```

**P1.b · ① 列表组件（功法 + 技艺）**
```
  - [ ] InspectScreen tabNames 扩为 4 个 (`["装备", "修仙", "技艺", "战斗·修炼"]`)
  - [ ] CombatTrainingPanel.java 主容器骨架（owo FlowLayout，五区先空占位）
  - [ ] TechniqueRowComponent（icon + name + Grade + proficiency 进度条 + 锁定态）
  - [ ] SkillRowComponent（复用 SkillSetStore，紧凑三行 HERB/ALCH/FORG）
  - [ ] 段头分隔（功法 / 技艺）+ 筛选 hover 态
  - [ ] selectedTechniqueId 状态 + selectionChanged 事件总线
```

**P1.c · ② 详情卡 + 联动**
```
  - [ ] TechniqueDetailCard.java 容器（描述 / 需求 / 招式数值四宫格）
  - [ ] 订阅 selectionChanged → 从 TechniquesListPanel.snapshot() 查 entry → 渲染
  - [ ] 已绑定 slot 反查：SkillBarStore.findBySkillId(id) → "已绑定槽 N" banner
  - [ ] 空选中态（无功法选中时）显示提示文本
```

**P1.d · ③ 经脉缩略图（联动高亮）**
```
  - [ ] BodyInspectComponent 加 compactMode（仅人体剪影，跳过 12+8 完整经脉绘制）
        — 或新建 MeridianMiniView.java 复用 MeridianBody 数据但简化渲染
  - [ ] highlightChannels(Set<MeridianChannel>) 接受所选功法的 required_meridians
  - [ ] legend 列出 2-4 条所需经脉 + 健康度（绿/红） + 污染度横条
  - [ ] 右下角 "详情" 链接 → switchTab("修仙") 跳到完整经脉视图
```

**P1.e · ⑤ 1-9 Hotbar 拖入 + ④ 状态条**
```
  - [ ] DragState 扩展 SourceKind.TECHNIQUE / TargetKind.SKILL_BAR(slot) / SourceKind.SKILL_LV (drop reject)
  - [ ] CombatHotbarStrip.java（9 槽 + 镜像 F1-F9 只读）
  - [ ] 拖入校验：境界不足 / 经脉 SEVERED → reject + 灰显反馈 + toast 提示
  - [ ] 命中：乐观更新 SkillBarStore + 发 SkillBarBind；server reject 由 SkillBarConfigHandler 回滚
  - [ ] 右键槽位 = 清空（发 SkillBarBind { binding: null }）
  - [ ] StatusBarsPanel 紧凑变体（真元 + 体力 + 因果 + 综合实力 + 区域），④ 区横向条
  - [ ] markBoundSlots(skill_id) — 选中功法时，⑤ 中已绑定该功法的槽描金边联动
  - [ ] 抽 SkillSlotRenderer util，CombatHotbarStrip + SkillBarHudPlanner 共享绘制
```

### P2 · 完整集成（随 plan-baomai-v1 P0 同步）

```
  - [ ] 崩拳绑定 + 按 1 出招全链路贯通
  - [ ] BurstMeridianEvent → 客户端播放动画 + 沉重色粒子
  - [ ] 冷却同步（SkillBarConfigV1，含 cooldown_until_ms）→ SkillBarHudPlanner 倒计时蒙灰
  - [ ] 虚脱期感知：1–9 槽位红色锁定覆盖
```

---

## §9 开放问题

- [x] ~~1–9 槽绑定物品时，是否需要走 `SkillBarCast` 协议？~~ → §3.3 已定：不发 `SkillBarCast`，client 端 mixin 不 cancel keypress，让现网 server-driven hotbar 切换继续
- [x] ~~技能冷却和物品冷却是否共享同一个 `Casting` component + `tick_casts` system？~~ → §5.3 已定：共享 `Casting` Component + `tick_casts_or_interrupt`，`cooldown_until_tick` 分别记在各自的 `SkillBarBindings` / `QuickSlotBindings` Component 上
- [ ] F1–F9 行是否也支持技能绑定？若支持，扩展现网 `QuickSlotBind.item_id` 前缀（如 `sk:...`）还是引入并列 `binding` 字段（与 `SkillBarBind` 同 union 结构）？
- [x] ~~客户端如何获取「已学功法列表」——是 server push 一个 `KnownTechniquesV1` / `techniques_snapshot` payload 还是客户端从 CultivationDetail 派生？~~ → §7.2 已定：server push `TechniquesSnapshotV1`，由 `KnownTechniques` Component + `skill_registry.rs` 静态 metadata 合并，`emit_techniques_snapshot_payloads` 监听 `Changed<KnownTechniques>` 推送
- [ ] 多流派共存（一个修士同时会爆脉流和暗器流）时，1–9 行如何分配？自由排列还是系统推荐？
- [ ] MC 原生 1–9 hotbar 的视觉切换动画（物品浮起）在技能槽上是否保留？

---

## §10 进度日志

- 2026-04-26：骨架创建。审计现有 F1–F9 快捷使用栏 + MC 原生 1–9 hotbar 实现，明确双行新分工：1–9 = 战斗技能行（最顺手），F1–F9 = 物品快捷使用行（消耗品 cast time）。设计 SkillSlot 数据模型 + SkillBarBind/SkillBarCast 协议 + InspectScreen「战斗技能」tab 配置流程。
- 2026-04-26（修订）：完整代码比对发现 6 处与现网不符的"依赖现有实现"声明，落正：
  (1) F1–F9 运行时存储是 `combat/components.rs:303 QuickSlotBindings` Component（instance_id），不是 `PlayerUiPrefs.quick_slots`（template_id，dead field）；后者从未被读写，重连绑定丢失，P0 顺手补持久化路径
  (2) TS schema `client-request.ts` 缺失 `UseQuickSlot` / `QuickSlotBind`，client 直发 JSON 无双端校验；P0 顺手补
  (3) cast 系统名是 `tick_casts_or_interrupt` 不是 `tick_casts`
  (4) client 端 `QuickUseSlotRenderer` 不存在，实际是 `QuickBarHudPlanner` + `QuickUseSlotStore` + `QuickSlotConfigHandler`
  (5) InspectScreen 没有"快捷使用 tab"，是 `buildQuickUseStrip()` 常驻左侧 strip；新方案选择 B = 加新 tab "战斗技能"
  (6) `TechniquesListPanel.java` 空骨架已存在但未挂上 InspectScreen
  同时统一 cooldown 同步通道为 `SkillBarConfigV1`（不再引入 `SkillBarSyncV1`），binding 用 union 而非字符串前缀。补全所有依赖文件行号。
- 2026-04-26（UI 优化 v2）：把 §4 配置流程从"加新 tab + 仅功法列表 + 1-9 槽"升级为「**战斗 · 修炼**」**五区联动工作台**——一个 tab 内同框：① 已学列表（功法 + 技艺 lv+xp）/ ② 功法详情卡（描述 + 需求 + 招式数值）/ ③ 经脉缩略图（高亮所选功法所需经脉）/ ④ 修士状态条（真元/体力/因果/综合）/ ⑤ 1-9 战斗 Hotbar（拖目标 + 镜像 F1-F9）。核心增益是**选中态联动**：① 选项 → ② 详情刷新 + ③ 经脉高亮变更 + ⑤ 已绑槽位描金边。
  - SVG 设计稿落库：`docs/plans-skeleton/plan-hotbar-modify-v1.svg`（800×540 mockup，含拖拽流向标注 + 五区色板对齐 `Grade.color()` 与 `MeridianChannel.baseColor()`）。client 端无 runtime SVG 渲染（无 batik），实装走 owo `FlowLayout` + `GridSlotComponent` + `BodyInspectComponent` + PNG icon + `DrawContext`。
  - 落定 §9 开放问题：客户端「已学功法列表」走 server push 路线 → 新增 `TechniquesSnapshotV1` payload + 服务端 `KnownTechniques` Component + `skill_registry.rs` 静态注册表（§7.2 新增三文件）。
  - §1.2 缺失清单补四项：功法详情卡 / 经脉与功法选中联动 / 技艺与战斗工作台同框 / 全局状态在配置面板可见。
  - §8 P1 拆为五子任务（a 数据链路 / b 列表 / c 详情卡 / d 经脉缩略图 / e Hotbar+状态条），每子任务独立可见效便于阶段性 review。
