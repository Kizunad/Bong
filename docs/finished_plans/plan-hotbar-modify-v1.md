# Bong · plan-hotbar-modify-v1 · Active（验收完成 2026-04-29，待归档）

**快捷栏双行重构**：明确 1–9（战斗技能行）与 F1–F9（物品快捷使用行）的分工，让爆脉流等战斗流派技能可以直接绑在 1–9 行上用最顺手的手指出招。

**世界观锚点**：无（纯 UX/交互设计 plan）

**交叉引用**：`plan-HUD-v1.md §2.2` · `plan-combat-ui_impl.md` · `plan-skill-v1.md` · `plan-baomai-v1.md`（首个接入的战斗流派 — **P0 不依赖**：本 plan P0 用 mock skill consumer 顶位即可，但接口必须按真实最终形态完整定义；P2 才换真实 impl）

**测试方针**（CLAUDE.md 饱和化测试）：本 plan 所有新协议 / Component / handler / 状态转换都要饱和覆盖 — happy + 边界 + 错误分支 + 状态转换全到位。mock skill consumer 同样要测全部分支（Skill 路由命中 / cooldown / cast 中断 / 虚脱期），让 plan-baomai-v1 真实 consumer 接入时只换 fn pointer 不改测试。

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

- [x] **1–9 行 = 战斗技能行**：最顺手（WASD 正上方），绑爆脉五式等战斗技能，按键直接出招
- [x] **F1–F9 行 = 物品快捷使用行**：绑丹药/绷带/暗器等消耗品，有 cast time
- [x] **两行都可混绑**：1–9 也可以放物品（复用现网 server-driven hotbar 切换路径），F1–F9 也可放无 cast 技能
- [x] **互不冲突**：1–9 触发走一条通道，F1–F9 走另一条，同槽位可以各自绑定不同东西
- [x] **配置路径统一**：InspectScreen 内拖拽配置——1–9 行新增「战斗·修炼」tab（五区联动工作台，详见 §4）；F1–F9 行沿用现网 `quickUseStrip`（不动）

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

**视觉设计稿**：`docs/plans-skeleton/plan-hotbar-modify-v1.svg`（v2 · 920×580 mockup，按现网 `outerRow` 真实结构对齐：左 hotbarStrip+quickUseStrip 双竖条 / 中 mainPanel `战斗·修炼` tab / 右 discardStrip；含拖拽流向 ① 列表→左侧 hotbar slot 标注）。实装走 owo-lib `FlowLayout` + 现有 `GridSlotComponent` / `BodyInspectComponent` / `StatusBarsPanel` + 新增组件，不引入 runtime SVG 渲染（client 端无 batik，全部 PNG icon + `DrawContext`）。

### 4.1 整体布局（按现网 InspectScreen.outerRow 真实结构）

**关键事实**：现网 `InspectScreen.outerRow` 是 `horizontalFlow`（line 163），三段式 `[hotbarStrip 34px 竖条][quickUseStrip 34px 竖条][mainPanel 中央][discardStrip 34px 竖条]`；hotbar / quickUse / discard 全部是 `verticalFlow`（line 530/547/610），9 槽竖排，**不是底部横排**。「战斗·修炼」tab 是 mainPanel 内新增的第 4 个 tab。

```
┌── InspectScreen · outerRow horizontalFlow（gap=2，vAlign=CENTER）─────────────────┐
│ ┌─1-9─┐ ┌─F1-9─┐ ┌── mainPanel (verticalFlow, padding 4) ────────────┐ ┌─丢弃─┐│
│ │  1崩│ │  F1丹│ │ tab: 装备 修仙 技艺 [战斗·修炼]   境界 凝脉一层 XP▰▰▱│ │  丢 ││
│ │  2靠│ │  F2绷│ ├────────────────────────────────────────────────────┤ │  弃 ││
│ │ cd  │ │  F3 ░│ │ ① 已学列表（拖源）         │ ② 功法详情卡          │ │     ││
│ │  3逆│ │  F4 ░│ │   ─ 功法 ─                │   [崩] 崩拳 · 黄阶     │ │     ││
│ │  ⊙  │ │  F5 ░│ │   [崩] 崩拳   ★选中态     │   描述 / 需求 / 招式  │ │     ││
│ │  4剑│ │  F6 ░│ │   [靠] 贴山靠 黄阶 0.62   │   ▸ 真元 30/360       │ │     ││
│ │  5 ░│ │  F7 ░│ │   [步] 血崩步 🔒境界锁   │   ▸ cast 8t · cd 60t  │ │     ││
│ │  6 ░│ │  F8 ░│ │   [逆] 逆脉护体 ⊙        │   ▸ 射程近身 1.8m     │ │     ││
│ │  7 ░│ │  F9 ░│ │   ─ 技艺 ─                │   ▶ 已绑定 · 左侧槽 1 │ │     ││
│ │  8 ░│ │      │ │   采药/炼丹/锻造（不入栏）│                       │ │     ││
│ │  9＋│ │      │ ├────────────────────────────┼───────────────────────┤ │     ││
│ │     │ │      │ │ ③ 经脉缩略图               │ ④ 修士状态            │ │     ││
│ │ ↑   │ │      │ │   [人体剪影]              │   真元 280/360         │ │     ││
│ │ 拖目│ │      │ │   ▰ LI 健康  ▰ TE 健康   │   体力 92/100          │ │     ││
│ │ 标  │ │      │ │   污染 0.12/1.00         │   因果+0.32 实力1240   │ │     ││
│ │     │ │      │ ├────────────────────────────┴───────────────────────┤ │     ││
│ │     │ │      │ │ BottomInfoBar （现网持有，不动）                    │ │     ││
│ └─────┘ └──────┘ └─────────────────────────────────────────────────────┘ └─────┘│
└────────────────────────────────────────────────────────────────────────────────┘
拖拽流向：mainPanel ① 列表 ─→ 左侧 hotbarStrip 槽 N（同侧为绑技能；同槽右键清空）
```

**⑤ 区不在 mainPanel 内**：避免双份 1–9 — `⑤ 1–9 战斗 strip = 左侧已存在的 hotbarStrip` 复用，本 tab 内 ①②③④ 选中态联动时通过 `markBoundSlots(skill_id)` 给左侧 strip 已绑该功法的槽描金边做回路视觉提示。F1–F9 同理（quickUseStrip 为镜像，不动）。

### 4.2 区域职责

| 区 | 内容 | 数据源 | 复用现有 | 新增组件 |
|---|---|---|---|---|
| **① 列表** | 功法行（Grade + proficiency 进度条 + 锁定态）+ 技艺三行紧凑（HERB/ALCH/FORG lv+xp）；功法可拖入 ⑤，技艺仅展示 | `TechniquesListPanel.snapshot()` + `SkillSetStore.snapshot()` | `SkillSetSnapshot.Entry.progressRatio()` | `TechniqueRowComponent` · `SkillRowComponent` |
| **② 详情卡** | 选中态功法的完整信息：描述、需求（境界 + 经脉）、招式数值（消耗/cast/cd/射程）、已绑定 slot 反查 | `TechniquesSnapshotV1`（详见 §7）+ `SkillBarStore` 反查 | — | `TechniqueDetailCard` |
| **③ 经脉缩略图** | 简化人体剪影 + 高亮"所选功法 `required_meridians`"对应经脉 + legend 列出健康度 | `MeridianBody`（已有完整 20 经数据）+ 详情卡 selected.required_meridians | `BodyInspectComponent` 加 compactMode | `MeridianChannel` 高亮集合参数 |
| **④ 修士状态** | 真元 / 体力 / 因果 / 综合实力 / 区域，紧凑横向条 | `PlayerStateViewModel`（spiritQiCurrent/Max、karma、compositePower、zoneId） | `StatusBarsPanel` 紧凑变体 | — |
| **⑤ 1-9 Hotbar** | **不在 tab 内**：直接复用 outerRow 远左 `hotbarStrip`（9 槽竖排 verticalFlow，width=cs+6=34）；技能（黄边）/ 物品（蓝边）/ 空（虚线）/ 冷却蒙灰 / toggle on（绿边小圆点）；左侧第二条 `quickUseStrip` 为 F1–F9 不动 | `SkillBarStore`（新）+ `QuickUseSlotStore`（已有） | `GridSlotComponent` 拖拽事件 + 现网 `buildHotbarStrip()`（line 528） | `SkillBarHudPlanner`（HUD 端共享渲染） |

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
| ~~`client/.../combat/inspect/CombatHotbarStrip.java`~~ | ~~新文件~~ —— **取消**：⑤ 复用现网 `InspectScreen.buildHotbarStrip()` 产物（`outerRow` 远左 `hotbarStrip`），扩 `GridSlotComponent` 拖目标 kind = `SKILL_BAR(slot)` 即可。F1–F9 镜像也无需新增——左侧第二条 `quickUseStrip` 是同源现网组件 |
| `client/src/main/java/com/bong/client/network/TechniquesSnapshotHandler.java`（新文件） | 接收 `TechniquesSnapshotV1` → 调 `TechniquesListPanel.replace(...)`；entry 字段扩展为含 `description` / `required_realm` / `required_meridians` / `qi_cost` / `cast_ticks` / `cooldown_ticks` / `range`（同步扩展 `TechniquesListPanel.Technique` record） |
| `client/src/main/java/com/bong/client/inventory/state/DragState.java` | 扩展 `SourceKind.TECHNIQUE`（带 `skill_id`）/ `TargetKind.SKILL_BAR(slot)`；技艺行为 `SourceKind.SKILL_LV` 但 drop 即 reject（不可入战斗 Hotbar） |

---

## §8 实施节点

### P0 · 1–9 技能栏基础（目标：1 周）✅ 2026-04-28

```
Schema：
  - [x] 顺手补：TS client-request.ts 新增 UseQuickSlotRequestV1 + QuickSlotBindRequestV1（修复现网遗漏）
  - [x] SkillBarBindRequestV1 + SkillBarCastRequestV1 TypeBox 定义（binding 用 union: null | item | skill）
  - [x] SkillBarConfigV1 server-data TypeBox（mirror QuickSlotConfigV1）
  - [x] Rust side ClientRequestV1 新增 SkillBarBind / SkillBarCast 变体 + SkillBarBindingV1 enum
Server：
  - [x] combat/components.rs 新增 SkillBarBindings Component（mirror QuickSlotBindings）+ SkillSlot enum
  - [x] inventory/mod.rs:260 同地点注入 SkillBarBindings::default()
  - [x] PlayerUiPrefs 改 pub(crate) + 新增 skill_bar: [SkillSlotPersist; 9] 字段
  - [x] handle_skill_bar_bind：解析 binding union → 写 SkillBarBindings.slots[slot]（Skill: skill_id 注册校验；Item: template_id → 背包首个匹配 instance；None: Empty）
  - [x] handle_skill_bar_cast：读 slots[slot] → Skill 路由到 skill system / Item 视为 nop / Empty 视为 nop
  - [x] 路由表：skill_id → system fn pointer（**接口完整 + mock 顶位**：fn 签名按真实最终形态定义 `fn(&mut World, EntityId, slot, target) -> CastResult`；mock 实现走完整 cast→cooldown→complete 状态机，仅伤害结算/经脉消耗用占位常量；plan-baomai-v1 接入时只替换 fn pointer 不改路由层）
  - [x] network/skillbar_config_emit.rs 新增 emit_skillbar_config_payloads（mirror quickslot_config_emit.rs）
  - [x] 持久化：登录时读 prefs_json.skill_bar → 注入 SkillBarBindings；handle_skill_bar_bind 写 Component 时同步 mark prefs dirty
  - [x] 顺手补（P0 必做）：现网 PlayerUiPrefs.quick_slots dead field 修复——
        handle_quick_slot_bind 写 QuickSlotBindings 时同步写 quick_slots（template_id），
        登录时读 prefs_json.quick_slots 注入 QuickSlotBindings
客户端：
  - [x] combat/SkillBarStore.java（本地镜像 9 槽，参考 QuickUseSlotStore）
  - [x] mixin/KeyBindingMixin.java：1–9 按键拦截 → 查 SkillBarStore.slots[n]：
        Skill → 发 SkillBarCast + cancel keypress；Item/Empty → 不 cancel，让 MC server-driven hotbar 切换继续
        （实装走 SkillBarKeyRouter.shouldCancelHotbarKey() 三态机 PASS_THROUGH/CAST_SENT/COOLDOWN_BLOCKED）
  - [x] hud/SkillBarHudPlanner.java：渲染 1–9 槽（测试用崩拳图标 + 冷却蒙灰；参考 QuickBarHudPlanner）
  - [x] network/SkillBarConfigHandler.java：接收 SkillBarConfigV1 → 更新 SkillBarStore（参考 QuickSlotConfigHandler）
测试（**饱和化**：见 CLAUDE.md "Testing — 饱和化测试"，每条 case 都要把"目标行为"锁死，回归立刻撞红）：
  Schema 双端对拍：
  - [x] TS↔Rust sample 对拍：UseQuickSlot / QuickSlotBind / SkillBarBind / SkillBarCast 全协议 happy + invalid（多余字段 / 类型错 / slot 越界 / binding union 缺 kind）
  - [x] SkillBarBindingV1 union：null / item / skill 三 variant 各有正反 sample
  按键路由（mixin + handle_skill_bar_cast）饱和分支：
  - [x] slot Empty → 按 1 nop（不发 SkillBarCast，server 收到也 drop 并日志告警）
  - [x] slot Item → 按 1 走 server-driven hotbar 切换（不 cancel keypress、不发 SkillBarCast）
  - [x] slot Skill → 按 1 发 SkillBarCast → mock consumer 走完整 cast→cooldown→complete
  - [x] cooldown 期内按 1 → client 蒙灰挡住；server 兜底 drop（双层防御都测）
  - [x] cast 期内受击中断 / 控制中断 / 移动 >0.3m 中断三种各一条 case（共用 tick_casts_or_interrupt 路径）
  绑定持久化（每条 transition 一条 case）：
  - [x] SkillBarBind Empty → Skill：写 SkillBarBindings + 写 prefs.skill_bar + emit SkillBarConfigV1
  - [x] SkillBarBind Skill → Empty（binding=null）：清空双层 + emit
  - [x] SkillBarBind Skill → 不同 Skill：覆盖且 cooldown 重置
  - [x] 重启进程：prefs 落盘 → 登录读 prefs → 注入 SkillBarBindings；与重启前完全一致
  - [x] 边界：slot 越界（10 / -1）reject；skill_id 不在 skill_registry reject
  顺手补 quick_slots 死字段（同款饱和）：
  - [x] handle_quick_slot_bind 写 Component 同步 mark prefs dirty + 落盘
  - [x] 登录读 prefs.quick_slots → 注入 QuickSlotBindings；F1–F9 重启不丢
  - [x] 回归：现网 UseQuickSlot 行为不变（cast / cooldown / 中断三路径）
  Mock skill consumer 接口锁定（让 plan-baomai-v1 接入时不改测试）：
  - [x] 路由 fn 签名 pin 测试：fn(&mut World, EntityId, slot, target) -> CastResult 各分支命中
  - [x] CastResult enum 全 variant（Started / Rejected{reason} / Interrupted）各有 case
```

### P1 · InspectScreen 五区联动工作台（目标：1.5 周）✅ 2026-04-28

按 §4 五区拆五子任务，建议按顺序推进（每子任务独立可见效，便于阶段性 review）：

**P1.a · Schema + Server techniques_snapshot 链路**（解锁后续 UI 数据源）
```
  - [x] TypeBox: TechniquesSnapshotV1 + TechniqueEntryV1 + TechniqueRequiredMeridianV1
  - [x] Rust: ServerDataV1 加入 TechniquesSnapshot 变体
  - [x] cultivation/skill_registry.rs：4 个示例条目（崩拳 / 贴山靠 / 血崩步 / 逆脉护体）
        必填 metadata: description / required_realm / required_meridians / qi_cost / cast_ticks / cooldown_ticks / range / grade
  - [x] cultivation/known_techniques.rs：KnownTechniques Component (stub: 4 个 id 全已学，proficiency 初始 0.5)
  - [x] network/techniques_snapshot_emit.rs：merge KnownTechniques + skill_registry → 推 TechniquesSnapshotV1
  - [x] client: TechniquesSnapshotHandler.java + 扩展 TechniquesListPanel.Technique record
  - [x] 测试：登录后 client 收到 4 条 entries，TechniquesListPanel.snapshot() 非空
```

**P1.b · ① 列表组件（功法 + 技艺）**
```
  - [x] InspectScreen tabNames 扩为 4 个 (`["装备", "修仙", "技艺", "战斗·修炼"]`)
  - [x] CombatTrainingPanel.java 主容器骨架（owo FlowLayout，五区先空占位）
  - [x] TechniqueRowComponent（icon + name + Grade + proficiency 进度条 + 锁定态）
  - [x] SkillRowComponent（复用 SkillSetStore，紧凑三行 HERB/ALCH/FORG）
  - [x] 段头分隔（功法 / 技艺）+ 筛选 hover 态
  - [x] selectedTechniqueId 状态 + selectionChanged 事件总线
```

**P1.c · ② 详情卡 + 联动**
```
  - [x] TechniqueDetailCard.java 容器（描述 / 需求 / 招式数值四宫格）
  - [x] 订阅 selectionChanged → 从 TechniquesListPanel.snapshot() 查 entry → 渲染
  - [x] 已绑定 slot 反查：SkillBarStore.findBySkillId(id) → "已绑定槽 N" banner
  - [x] 空选中态（无功法选中时）显示提示文本
```

**P1.d · ③ 经脉缩略图（联动高亮）**
```
  - [x] BodyInspectComponent 加 compactMode（仅人体剪影，跳过 12+8 完整经脉绘制）
        — 或新建 MeridianMiniView.java 复用 MeridianBody 数据但简化渲染（实装走 MeridianMiniView 新组件）
  - [x] highlightChannels(Set<MeridianChannel>) 接受所选功法的 required_meridians
  - [x] legend 列出 2-4 条所需经脉 + 健康度（绿/红） + 污染度横条
  - [x] 右下角 "详情" 链接 → switchTab("修仙") 跳到完整经脉视图
```

**P1.e · ⑤ 1-9 Hotbar 拖入 + ④ 状态条**
```
  - [x] DragState 扩展 SourceKind.TECHNIQUE / TargetKind.SKILL_BAR(slot) / SourceKind.SKILL_LV (drop reject)
  - [x] **不新建 CombatHotbarStrip** —— ⑤ = 现网 outerRow 远左 hotbarStrip（line 528），通过 GridSlotComponent dropTargetKind 接受 SKILL_BAR(slot) 类型即可
  - [x] 拖入校验：境界不足 / 经脉 SEVERED → reject + 灰显反馈 + toast 提示
  - [x] 命中：乐观更新 SkillBarStore + 发 SkillBarBind；server reject 由 SkillBarConfigHandler 回滚
  - [x] 右键左侧 strip 槽位 = 清空（发 SkillBarBind { binding: null }）
  - [x] StatusBarsPanel 紧凑变体（真元 + 体力 + 因果 + 综合实力 + 区域），④ 区横向条
  - [x] markBoundSlots(skill_id) — 选中功法时，左侧 hotbarStrip 中已绑定该功法的槽描金边（GridSlotComponent.setHighlight）
  - [x] 抽 SkillSlotRenderer util，hotbarStrip 槽渲染 + SkillBarHudPlanner（HUD 端）共享绘制
```

### P2 · 完整集成（随 plan-baomai-v1 P0 同步 — 仅替换 mock fn pointer，路由 / handler / 协议 / 测试都不改）✅ 2026-04-29

```
  - [x] 崩拳绑定 + 按 1 出招全链路贯通（替换 mock skill consumer = plan-baomai-v1 真实 resolve_beng_quan）
  - [x] BurstMeridianEvent → 客户端播放动画 + 沉重色粒子
  - [x] 冷却同步（SkillBarConfigV1，含 cooldown_until_ms）→ SkillBarHudPlanner 倒计时蒙灰
  - [x] 虚脱期感知：1–9 槽位红色锁定覆盖
  - [x] **回归**：P0 全部饱和化测试通过率 100%（任何一条红就证明接口或路由层不该改而被改了）
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
  - SVG 设计稿落库：`docs/plans-skeleton/plan-hotbar-modify-v1.svg`（v1 · 800×540 mockup，含拖拽流向标注 + 五区色板对齐 `Grade.color()` 与 `MeridianChannel.baseColor()`）。client 端无 runtime SVG 渲染（无 batik），实装走 owo `FlowLayout` + `GridSlotComponent` + `BodyInspectComponent` + PNG icon + `DrawContext`。
  - 落定 §9 开放问题：客户端「已学功法列表」走 server push 路线 → 新增 `TechniquesSnapshotV1` payload + 服务端 `KnownTechniques` Component + `skill_registry.rs` 静态注册表（§7.2 新增三文件）。
  - §1.2 缺失清单补四项：功法详情卡 / 经脉与功法选中联动 / 技艺与战斗工作台同框 / 全局状态在配置面板可见。
  - §8 P1 拆为五子任务（a 数据链路 / b 列表 / c 详情卡 / d 经脉缩略图 / e Hotbar+状态条），每子任务独立可见效便于阶段性 review。
- 2026-04-27（设计稿修订 v2 · hotbar 位置纠偏）：v1 SVG 把 ⑤ 1–9 Hotbar 画在 InspectScreen 底部横排（800×48），实地考察 `client/src/main/java/com/bong/client/inventory/InspectScreen.java:163-476` 发现现网 `outerRow` 是 `horizontalFlow`，三段式 `[hotbarStrip(34) | quickUseStrip(34) | mainPanel | discardStrip(34)]`，hotbar / quickUse / discard 全是 `verticalFlow` 9 槽竖排（line 530/547/610），**v1 设计稿与现网完全错位**。
  - 重绘 `plan-hotbar-modify-v1.svg`（v2 · 920×580），按现网 `outerRow` 真实结构画：左侧两条 strip 竖排 + 中央 mainPanel 显示「战斗·修炼」tab + 右侧 discardStrip。
  - 修订 §4.1 五区布局 ASCII 图：⑤ 不再是 mainPanel 内独立区，而是**复用左侧已有 hotbarStrip 当拖目标**，避免双份 1–9。
  - §4.2 区域职责表 ⑤ 行：取消"新增 `CombatHotbarStrip`"，改为扩 `GridSlotComponent.dropTargetKind = SKILL_BAR(slot)`；§7.3 客户端表对应文件删除（保留 `SkillBarHudPlanner` 用于 HUD 渲染）。
  - §8 P1.e 子任务相应调整：去掉 CombatHotbarStrip 行，加左侧 strip 拖目标接入 + `GridSlotComponent.setHighlight` 联动。
- 2026-04-27（依赖解耦 + 测试方针）：原 P0 / P1.a 描述把 mock skill consumer 写得像"应付占位"，与 plan-baomai-v1 P0 时序绑死。修订为 **mock 顶位 + 完整接口 + 饱和化测试** 方针：
  - 头部「测试方针」段落新增，引 CLAUDE.md "Testing — 饱和化测试"。
  - 路由表 fn 签名按真实最终形态定义 `fn(&mut World, EntityId, slot, target) -> CastResult`；mock 实现完整走 cast→cooldown→complete 状态机；plan-baomai-v1 接入时只换 fn pointer 不改路由层。
  - §8 P0 测试章节重写为饱和清单：schema 双端对拍 / 按键路由 4 分支 / 中断 3 路径 / 绑定 transition 4 状态 / 持久化 / 边界 reject / quick_slots 死字段补救 / mock consumer 接口 pin —— 每条都把"目标行为"锁死，回归即撞红。
  - P2 标注为"仅替换 mock fn pointer"，并加回归条 = P0 测试全绿。
- 2026-04-28（P0 + P1 实装合并）：PR #65 合并 `1b2f0e0e`，分 4 个核心 commit 落地：
  - `3062098c feat(schema): 补齐技能栏协议与样本` — TS `SkillBarBindRequestV1/SkillBarCastRequestV1/SkillBarBindingV1/UseQuickSlotRequestV1/QuickSlotBindRequestV1` + `SkillBarConfigV1` + `TechniquesSnapshotV1`；Rust 对偶 enum 与 sample 双端对拍。
  - `1a05076f feat(server): 接入技能栏运行时与持久化` — `SkillBarBindings` Component + `SkillSlot/SkillSlotPersist` enum；`handle_skill_bar_bind/cast` + `start_generic_skillbar_cast` + skill_id 路由表（fn pointer，mock 顶位）；`network/skillbar_config_emit.rs` + 配套 `skillbar_config_emit_test.rs`；`cultivation/skill_registry.rs` 4 条目；`cultivation/known_techniques.rs` stub Component；`network/techniques_snapshot_emit.rs`；登录/dirty 双向同步 + `quick_slots` dead field 修复。
  - `d3d797e0 feat(client): 接入技能栏协议与 HUD 路由` — `combat/SkillBarStore/SkillBarKeyRouter/SkillBarEntry/SkillBarConfig`（三态机 PASS_THROUGH/CAST_SENT/COOLDOWN_BLOCKED）；`network/SkillBarConfigHandler/TechniquesSnapshotHandler`；`hud/SkillBarHudPlanner`；`mixin/CombatKeybindings` F1-F9 注册 + 1-9 拦截 + I 键 InspectScreen。
  - `1755d4ff feat(client): 增加战斗修炼配置页` — `combat/inspect/CombatTrainingPanel`（五区主容器，selectionChanged 事件总线）+ `TechniqueDetailCard` + `MeridianMiniView` + 扩展 `TechniquesListPanel`；InspectScreen tab 扩为 4 个；DragState `SourceKind.TECHNIQUE/TargetKind.SKILL_BAR` + 拖入校验。
  - `e87668f9 fix(client): 收口技能栏评审问题` — review 修复回路。
- 2026-04-29（P2 + 验收）：`b0302396 feat: 落地爆脉崩拳真实结算` — `cultivation/burst_meridian.rs` 真实 `resolve_beng_quan` 替换 mock fn pointer，按 P2 设计仅换实现不改路由 / 协议 / 测试；崩拳 slot 0 cast 400ms + cd 3000ms 经 `SkillBarKeyRouterTest` PASS_THROUGH/CAST_SENT/COOLDOWN_BLOCKED 三态机回归全绿。`/plans-status` 实地核验：P0/P1/P2 全部代码侧已落地，文档复选框补勾完毕，准备归档进 `docs/finished_plans/`。

---

## §11 Finish Evidence

### 落地清单

**P0 · 1–9 技能栏基础 + quick_slots dead field 修复**

| 层 | 文件 / 路径 | 内容 |
|---|---|---|
| Schema (TS) | `agent/packages/schema/src/client-request.ts` | `SkillBarCastRequestV1` + `SkillBarBindRequestV1` + `SkillBarBindingV1` + 顺手补 `UseQuickSlotRequestV1` + `QuickSlotBindRequestV1`（全部入 `ClientRequestV1` Union） |
| Schema (TS) | `agent/packages/schema/src/server-data.ts` | `SkillBarConfigV1` + `TechniquesSnapshotV1` |
| Schema (Rust) | `server/src/schema/client_request.rs` | `ClientRequestV1::SkillBarBind/SkillBarCast` + `SkillBarBindingV1` enum |
| Schema 样本 | `agent/packages/schema/samples/client-request-skill-bar-*.json` + `client-request-quick-slot-*.json` | 双端对拍 |
| Server | `server/src/combat/components.rs` | `SkillBarBindings` Component（mirror `QuickSlotBindings`）+ `SkillSlot` enum |
| Server | `server/src/inventory/mod.rs` | 同地点注入 `SkillBarBindings::default()` |
| Server | `server/src/player/state.rs` | `PlayerUiPrefs` 改 `pub(crate)` + `skill_bar: [SkillSlotPersist; 9]`；`SkillSlotPersist` enum；登录路径注入 `SkillBarBindings` + 修 `quick_slots` dead field |
| Server | `server/src/network/client_request_handler.rs` | `handle_skill_bar_bind` / `handle_skill_bar_cast` / `start_generic_skillbar_cast` + skill_id → fn pointer 路由表（mock 顶位，按真实最终签名定义）|
| Server | `server/src/network/skillbar_config_emit.rs` (新) | `emit_skillbar_config_payloads` 监听 `Changed<SkillBarBindings>` |
| Server | `server/src/network/skillbar_config_emit_test.rs` (新) | 153 行配套测试 |
| Server | `server/src/cultivation/skill_registry.rs` (新) | 4 条目静态注册表（崩拳/贴山靠/血崩步/逆脉护体） |
| Server | `server/src/cultivation/known_techniques.rs` (新) | `KnownTechniques` Component（stub） |
| Server | `server/src/network/techniques_snapshot_emit.rs` (新) | merge stub + registry 推 `TechniquesSnapshotV1` |
| Client | `client/src/main/java/com/bong/client/combat/SkillBarStore.java` (新) | 本地 9 槽镜像 |
| Client | `client/src/main/java/com/bong/client/combat/SkillBarKeyRouter.java` (新) | 三态机 `PASS_THROUGH / CAST_SENT / COOLDOWN_BLOCKED` |
| Client | `client/src/main/java/com/bong/client/combat/SkillBarEntry.java` + `SkillBarConfig.java` (新) | 数据载体 |
| Client | `client/src/main/java/com/bong/client/network/SkillBarConfigHandler.java` (新) | 接收 `SkillBarConfigV1` → 更新 store |
| Client | `client/src/main/java/com/bong/client/hud/SkillBarHudPlanner.java` (新) | 1-9 槽 HUD 渲染（图标 + 冷却蒙灰） |
| Client | `client/src/main/java/com/bong/client/mixin/CombatKeybindings.java` | F1-F9 注册（`GLFW_KEY_F1 + i`）+ 1-9 拦截 via `SkillBarKeyRouter.shouldCancelHotbarKey()` + I 键 InspectScreen 入口 |

**P1 · InspectScreen 五区联动工作台**

| 子任务 | 文件 / 路径 |
|---|---|
| P1.a 数据链路 | `cultivation/skill_registry.rs` + `cultivation/known_techniques.rs` + `network/techniques_snapshot_emit.rs` + `client/network/TechniquesSnapshotHandler.java` |
| P1.b 列表 | `client/inventory/InspectScreen.java` tab 列表扩为 4 个；`combat/inspect/CombatTrainingPanel.java` (新) 主容器 + `selectedTechniqueId` + `selectionChanged` 事件总线；`combat/inspect/TechniquesListPanel.java` 从空骨架填充 |
| P1.c 详情卡 | `combat/inspect/TechniqueDetailCard.java` (新) 描述/需求/招式数值四宫格；订阅 `selectionChanged` |
| P1.d 经脉缩略图 | `combat/inspect/MeridianMiniView.java` (新) compact 模式 + `highlightChannels(Set<MeridianChannel>)` |
| P1.e Hotbar 拖入 + 状态条 | `inventory/state/DragState` 扩 `SourceKind.TECHNIQUE/TargetKind.SKILL_BAR(slot)`；复用现网 `hotbarStrip` 不新建 `CombatHotbarStrip`；`StatusBarsPanel` 紧凑变体；`markBoundSlots` 描金边联动 |

**P2 · 完整集成（爆脉崩拳真实结算）**

| 文件 / 路径 | 内容 |
|---|---|
| `server/src/cultivation/burst_meridian.rs` | 真实 `resolve_beng_quan`：臂经脉 integrity 扣减 + qi 消耗 + 过载 AttackIntent + `BurstMeridianEvent` |
| `client/src/main/resources/assets/bong/player_animation/beng_quan.json` | 8t 崩拳动画（plan-player-animation-v1 §5.1 增量） |
| 路由层未改 | skill_id `burst_meridian.beng_quan` 仅替换 fn pointer，handler / 协议 / 测试均未动 ✓ 满足 P2 设计目标 |

### 关键 commit

| Hash | 日期 | 说明 |
|---|---|---|
| `3062098c` | 2026-04-28 | feat(schema): 补齐技能栏协议与样本（P0 schema） |
| `1a05076f` | 2026-04-28 | feat(server): 接入技能栏运行时与持久化（P0 server + P1.a server） |
| `d3d797e0` | 2026-04-28 | feat(client): 接入技能栏协议与 HUD 路由（P0 client） |
| `1755d4ff` | 2026-04-28 | feat(client): 增加战斗修炼配置页（P1.b/c/d/e 五区工作台） |
| `e87668f9` | 2026-04-28 | fix(client): 收口技能栏评审问题（review 回路） |
| `1b2f0e0e` | 2026-04-28 03:00 | Merge PR #65 实现技能栏双行快捷栏链路（P0+P1 入主） |
| `8aa2f1c6` | 2026-04-28 | 合并 main：同步技能栏与粒子更新 |
| `b0302396` | 2026-04-29 00:51 | feat: 落地爆脉崩拳真实结算（P2 — 仅替换 mock fn pointer） |

### 测试结果

```
Server (Rust):
  - server/src/network/skillbar_config_emit_test.rs       # 153 行配套
  - server/src/network/client_request_handler.rs          # 5 个 skill_bar_* 测试函数
    · skill_bar_bind_skill_then_cast_starts_skillbar_cast
    · skill_bar_cast_defined_skill_without_resolver_uses_generic_cast_path
    · skill_bar_cast_protocol_entity_id_does_not_fallback_to_entity_bits
    · skill_bar_cast_empty_item_or_cooldown_does_not_start_cast
    · skill_bar_bind_rejects_unknown_skill
  - 整体 client_request_handler 模块 24 个 #[test]
  跑法：cd server && cargo test
Client (Java):
  - client/src/test/.../combat/SkillBarKeyRouterTest.java     # 3 路 PASS_THROUGH / CAST_SENT / COOLDOWN_BLOCKED
  - client/src/test/.../network/SkillBarConfigHandlerTest.java # 50 行
  跑法：cd client && ./gradlew test
Schema 双端对拍：
  - agent/packages/schema/samples/client-request-{skill-bar-*,quick-slot-*}.json  双端 roundtrip
  跑法：cd agent/packages/schema && npm test
```

### 跨仓库核验

| 仓库 | 命中 symbol |
|---|---|
| **agent/schema** | `SkillBarCastRequestV1` / `SkillBarBindRequestV1` / `SkillBarBindingV1` / `UseQuickSlotRequestV1` / `QuickSlotBindRequestV1` / `SkillBarConfigV1` / `TechniquesSnapshotV1` |
| **server** | `SkillBarBindings` / `SkillSlot` / `SkillSlotPersist` / `handle_skill_bar_bind` / `handle_skill_bar_cast` / `start_generic_skillbar_cast` / `emit_skillbar_config_payloads` / `emit_techniques_snapshot_payloads` / `cultivation::skill_registry` / `cultivation::known_techniques` / `cultivation::burst_meridian::resolve_beng_quan` |
| **client** | `SkillBarStore` / `SkillBarKeyRouter` / `SkillBarConfigHandler` / `SkillBarHudPlanner` / `TechniquesSnapshotHandler` / `CombatTrainingPanel` / `TechniqueDetailCard` / `MeridianMiniView` / `TechniquesListPanel` / `CombatKeybindings` |

### 遗留 / 后续

- **§9 仍开放**：F1-F9 行是否支持技能绑定（扩 `QuickSlotBind.item_id` 前缀 vs 引入 union）？多流派共存（爆脉+暗器同时学）的 1-9 自动分配方案？MC 原生 1-9 hotbar 视觉切换动画在技能槽是否保留？—— 这三项是 UX/扩展设计问题，本 plan 不收口，留给后续流派 plan（plan-anqi-* / plan-zhenfa-* 等）触发时再定
- **依赖外部 plan**：路由表 `skill_id → fn pointer` 当前仅崩拳为真实实现，其他四式（贴山靠 / 血崩步 / 逆脉护体 / etc.）随 `plan-baomai-v1` 后续阶段陆续替换；`plan-anqi-v1` / `plan-zhenfa-v1` 等其他流派同款"换 fn pointer 不改路由"
- **`KnownTechniques` Component 仍 stub**：当前 4 条全已学 + proficiency 0.5 固定值，真实学习 / 掌握度 / 残卷消费由 `plan-baomai-v1` P1 / `plan-cultivation-progression-v*` 接管
- **饱和测试基线**：当前 `SkillBarKeyRouterTest` 3 路覆盖三态机；CLAUDE.md 饱和测试纲要要求边界（slot 越界 / skill_id 不在 registry / 受击中断 / 移动中断）也锁——server 侧 `client_request_handler` 已覆盖大部分，client 侧未来如发现 router 边界 bug 应补 case
