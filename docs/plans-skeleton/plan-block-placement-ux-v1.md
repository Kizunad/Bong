# plan-block-placement-ux-v1（骨架）

> **骨架（skeleton / 草案）**。一句话主题：修 P5 方块放置 / 快捷栏绑定的 UX 与 wiring 缺口——**拖拽绑定可放置方块**、**用后图标·数量同步**、**上下文菜单左右键都可点**——让"BlockPicker 取方块 → 绑栏 → 右键放置"这条审阅/建造闭环顺滑可用。

> 立项动机：worldgen-v4 P6 真机审阅时，用户取了 BlockPicker 的 `vanilla:<block>` 方块却右键放不下。逐层排查发现 P5 放置功能在双快捷栏 UI 下存在多处 wiring 断点（见各阶段）。**底层 server 放置链路（`block_item_to_state` vanilla: 直通 + `handle_block_place_requests`）正常**，缺口全在客户端绑定/选中/同步层。

## 阶段总览

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 拖拽绑定可放置方块（统一 quick_slot ↔ SkillBarStore 绑定路径） | ⬜ |
| P1 | 用后同步：实例消耗/数量扣减 → SkillBar 图标清除或刷新 | ⬜ |
| P2 | 上下文菜单左右键都可点（"绑定到 N" 等不再只认右键） | ⬜ |

## 接入面 checklist（防孤岛——升 active 前据 docs/CLAUDE.md §二 核实）

- **进料**：复用 P5 `BlockPicker`（`vanilla:<block>` 物品）+ `BlockVanillaIconMap.isKnownBlockItem`（已认 vanilla: 物品，见 review-fixes #559）+ `InventoryStateStore` / `SkillBarStore` / `BlockPlaceIntentResolver`。
- **出料**：客户端 `ClientRequestSender.sendBlockPlace`（已存在）→ server `handle_block_place_requests`（已放 `Vanilla{state}`，**不动**）。本 plan 只修客户端绑定/选中/同步，不改放置协议与 server 落块逻辑。
- **共享类型 / event**：`SkillBarEntry.Kind.ITEM` / `SkillBarStore` / `quick_slot_bind` vs `skill_bar_bind_item` 两条 C2S（**核心矛盾**，见 §N）。
- **跨仓库契约**：client 绑定层为主；P1 若需 server 在消耗实例后回执"清槽"信号，确认走既有 inventory_snapshot / instance 同步，不新增协议。
- **worldview 锚点**：纯 dev/creative 画廊审阅工具链（BlockPicker 是 dev-only），无 worldview gameplay 锚点；但放置 UX 一致性服务于建造体验。
- **qi_physics 锚点**：无（纯放置/UI）。

## P0 — 拖拽绑定可放置方块（统一绑定路径）

- **现状 bug**：把 `vanilla:<block>` 方块**拖**到快捷栏槽 → 走 `InspectScreen.java:860 sendQuickSlotBind`（更新服务端 quick_slot + `quickUseSlots` UI），**但不写 `SkillBarStore`**。而放置读的是 `BlockPlaceIntentResolver.java:19 SkillBarStore.snapshot().slot(selectedSlot)` —— 两条路不通 → 拖进去的方块"看得见、放不了"（服务器日志全 `quick_slot_bind`、零 `block_place` 实证）。能放的是另一条：右键物品 → `InspectScreen.java:2479 openSkillBarContextMenu` → "绑定到 N" → `:2505 bindBlockItemToSkillBar` → `sendSkillBarBindItem` + `SkillBarStore.updateSlot`。
- **目标**：**拖放可放置方块到快捷栏 = 绑进 SkillBarStore**（与右键菜单同效），让拖拽这一最自然的动作直接可放。
- **可核验交付物**：`InspectScreen` 拖放落点对 `isBlockQuickBarBindable(item)` 的方块走 `bindBlockItemToSkillBar`（或等价：拖放同时 `sendSkillBarBindItem` + `SkillBarStore.updateSlot`）；测试「拖放方块后 `SkillBarStore.snapshot().slot(slot)` 为 ITEM 且 `selectedBlockPlaceIntent` 非空」。
- **待决**：`quick_slot`（cast/use 栏）与 `SkillBarStore`（放置读的栏）是否本该是同一个栏（见 §N #1）——若是，统一两条绑定 C2S；若不是，方块只允许绑 SkillBar 栏、quick_slot 栏对方块禁用拖放。

## P1 — 用后同步：图标清除 / 数量扣减

- **现状 bug**：放置成功后 server 消耗实例（用户实测「不能再使用」= 已消耗），**但 SkillBar 图标仍在**（`SkillBarEntry` 按 itemId 留存，实例消耗后未刷新）。期望：放一个扣一个，数量到 0 清空槽位图标。
- **目标**：实例被放置消耗 / 数量变化时，同步刷新或清除对应 SkillBar 槽图标。
- **可核验交付物**：放置后 inventory 同步触发 SkillBar 槽按 instance 存活校验（参 `SkillBarStore.java:48-50` 已有 selectedEntry 失效清 selectedSlot 的模式，扩到槽图标）；count>1 显示数量、=0 清槽；测试「实例消耗后槽 entry 清空」。
- **待决**：BlockPicker 方块是单实例还是可堆叠 count（"扣除一定数量"语义）——决定是逐个消耗还是按 count 扣减。

## P2 — 上下文菜单左右键都可点

- **现状 bug**：`InspectScreen.java:1740 if (button == 1)` —— "绑定到 N" / 丹药 / 武器等上下文菜单的**动作点击只认右键**（button==1），用户左键点无反应。菜单本身右键/长按打开（`:1818/:1832`），选动作还要右键，反直觉。
- **目标**：菜单动作点击**左右键皆可**（`skillBarContextMenu` / `pillContextMenu` / `weaponContextMenu` 三处一致）。
- **可核验交付物**：菜单 action 命中逻辑从 `if (button == 1)` 放宽为左右键都处理（保留右键/长按打开）；测试「button==0 点 action 命中 `triggerSkillBarMenuAction`」。

## 审计来源

worldgen-v4 P6 真机审阅期间用户实操放置 surfaced。底层 server 放置链路正常（`server/src/world/block_place.rs::block_item_to_state` 已 vanilla: 直通、`handle_block_place_requests` 已落块），缺口全在 client 绑定/选中/同步层。与 `plan-interaction-intent-cleanup-v1`（感知/截脉自动触发收窄）同属 P6-审阅-surfaced 的 P5 交互 wiring 修复，但主题不同（放置 UX vs 自动触发），故分立。

## §N 开放问题（升 active 前收口）

1. **quick_slot 栏 与 SkillBarStore 栏 是一个还是两个**：现有 `quick_slot_bind`（cast/use）与 `skill_bar_bind_item`（放置读）两条 C2S + 两套 UI 状态。需查 server 侧 quick_slot 与 skill_bar 的存储/语义，定"统一为一栏"还是"两栏各司其职、方块只走 SkillBar"。这是 P0 实现路线的前置。
2. **count 语义**：dev BlockPicker 方块给的是单实例还是带 count 堆叠；放置扣减按实例还是按数量。
3. 是否一并把"拖放/选中/放置"的反馈做轻量提示（如选中槽高亮、放置成功音效），提升可用性——还是只修功能 wiring。
