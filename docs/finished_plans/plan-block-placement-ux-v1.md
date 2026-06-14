# plan-block-placement-ux-v1

> 一句话主题：修 P5 方块放置 / 快捷栏绑定的 UX 与 wiring 缺口——**拖拽绑定可放置方块**、**用后图标·数量同步**、**上下文菜单左右键都可点**——让"BlockPicker 取方块 → 绑栏 → 右键放置"这条审阅/建造闭环顺滑可用。

> 立项动机：worldgen-v4 P6 真机审阅时，用户取了 BlockPicker 的 `vanilla:<block>` 方块却右键放不下。逐层排查发现 P5 放置功能在双快捷栏 UI 下存在多处 wiring 断点（见各阶段）。**底层 server 放置链路（`block_item_to_state` vanilla: 直通 + `handle_block_place_requests`）正常**，缺口全在客户端绑定/选中/同步层。

## 阶段总览

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 拖拽绑定可放置方块（统一 quick_slot ↔ SkillBarStore 绑定路径） | ✅ 2026-06-15 |
| P1 | 用后同步：实例消耗/数量扣减 → SkillBar 图标清除或刷新 | ✅ 2026-06-15 |
| P2 | 上下文菜单左右键都可点（"绑定到 N" 等不再只认右键） | ✅ 2026-06-15 |
| P3 | vanilla:<block> 物品图标渲染真 vanilla 物品（不再落火焰占位图） | ✅ 2026-06-15 |

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

## P3 — vanilla:<block> 物品图标渲染真 vanilla 物品

- **现状 bug**：`vanilla:acacia_log` 等 BlockPicker 方块在 SkillBar / 背包槽里显示成**火焰熔岩"未知物品"占位图**，不是真的 acacia_log 木头材质。根因：`InspectScreen.java:3124 drawItemTextureRaw` 用 `GridSlotComponent.textureIdForItem(item)` 取一张**扁平 Bong 自定义贴图**（`ctx.drawTexture`）画图标；`vanilla:` 物品在 `ItemIconRegistry` 无对应 Bong 贴图 → 落 fallback 占位图。SkillBar 绑定也用 `:2509 ItemIconRegistry.itemTexturePath(item.itemId())` 同样落空。
- **关键**：`BlockVanillaIconMap.createVanillaBlockStack(blockShortId)` **已能给真 vanilla `ItemStack`**（BlockPicker 面板就靠它显示真图标，§review-fixes #559 我已让 `createStackFor` 也认 vanilla:）—— 只是 SkillBar/背包槽的图标渲染没走这条。
- **目标**：`vanilla:<short>` 物品的图标 = **用 MC 原生物品渲染**（`DrawContext.drawItem(createVanillaBlockStack(short), x, y)`）而非扁平贴图，与 BlockPicker 面板一致显示真方块图标。
- **可核验交付物**：`textureIdForItem` / `drawItemTextureRaw` / SkillBar `iconTexture` 对 `vanilla:` 前缀分支走真 ItemStack 渲染；测试 / 目检「acacia_log 槽显示木头材质而非占位图」。
- **待决**：是否所有 dev 画廊 vanilla 方块都走原生渲染（含无 BlockItem 的特殊块降级占位）；与 §review-fixes #559 的 `createStackFor`/`createVanillaBlockStack` 复用对齐。

## 审计来源

worldgen-v4 P6 真机审阅期间用户实操放置 surfaced。底层 server 放置链路正常（`server/src/world/block_place.rs::block_item_to_state` 已 vanilla: 直通、`handle_block_place_requests` 已落块），缺口全在 client 绑定/选中/同步层。与 `plan-interaction-intent-cleanup-v1`（感知/截脉自动触发收窄）同属 P6-审阅-surfaced 的 P5 交互 wiring 修复，但主题不同（放置 UX vs 自动触发），故分立。

## §N 开放问题（升 active 前收口）

1. **quick_slot 栏 与 SkillBarStore 栏 是一个还是两个**：现有 `quick_slot_bind`（cast/use）与 `skill_bar_bind_item`（放置读）两条 C2S + 两套 UI 状态。需查 server 侧 quick_slot 与 skill_bar 的存储/语义，定"统一为一栏"还是"两栏各司其职、方块只走 SkillBar"。这是 P0 实现路线的前置。
2. **count 语义**：dev BlockPicker 方块给的是单实例还是带 count 堆叠；放置扣减按实例还是按数量。
3. 是否一并把"拖放/选中/放置"的反馈做轻量提示（如选中槽高亮、放置成功音效），提升可用性——还是只修功能 wiring。

> 收口结论：#1 定为「两栏各司其职、方块拖放额外旁路写 SkillBarStore」——P0 在 quick_slot 拖放落点对可放置方块**额外**调 `bindBlockItemToSkillBar`（同时发 `skill_bar_bind` + `SkillBarStore.updateSlot`），不合并两条 C2S，避免动既有 cast/use 栏语义。#2 定为「按 inventory_snapshot 自洽」——P1 不新增"清槽"协议，客户端用既有 `inventory_snapshot` 校验实例存活，count 走数量显示、消耗到 0 清槽。#3 只修功能 wiring，不做新增反馈提示（留待后续 plan）。

---

## Finish Evidence

### 落地清单（各阶段真实 client 文件/类）

- **P0 拖拽绑定可放置方块**：`client/src/main/java/com/bong/client/inventory/InspectScreen.java`
  - `isBlockQuickBarBindable(InventoryItem)`（:2565）—— 谓词，复用 `BlockVanillaIconMap.isKnownBlockItem`
  - quick_slot 拖放落点（:898）对可放置方块**额外**走 `bindBlockItemToSkillBar(index, item)`（:899），使拖放与右键菜单"绑定到 N"同效（同时 `sendSkillBarBindItem` + `SkillBarStore.updateSlot`）
  - 测试 `InspectScreenSkillBarBindItemTest`（7 cases）：拖放方块后 `SkillBarStore.snapshot().slot(slot)` 为 ITEM 且 `selectedBlockPlaceIntent` 非空
- **P1 用后同步（实例消耗 → 槽自洽刷新）**：`InspectScreen.java`
  - inventory_snapshot 到达后按 instance 存活 reconcile SkillBar 槽（:822-835，`SkillBarStore.updateSlot(i, null)` 清失效槽；count>1 由 `GridSlotComponent.drawItemOverlays` 绘数量；`!model.isEmpty()` 守卫防开屏首帧误清）
  - 测试 `InspectScreenSkillBarHydrateTest`（8 cases）：消耗清槽 / 仍存活保留 / count 显示 / 空 model 守卫 / SKILL 类不动 / 多槽独立 reconcile
- **P2 上下文菜单左右键都可点**：`InspectScreen.java`
  - `handleContextMenuClick(mouseX, mouseY, button)`（:1797）—— action 行命中对 button==0 与 button==1 都触发（`triggerPillMenuAction` / `triggerSkillBarMenuAction` / `triggerWeaponMenuAction` 三处一致）；行外侧仅右键关菜单，左键点空白不关
  - 测试 `InspectScreenContextMenuClickTest`（8 cases）：button==0 命中 action 触发、三菜单一致、左键空白不关、右键空白关
- **P3 vanilla:<block> 真物品图标渲染**：`client/src/main/java/com/bong/client/block/BlockVanillaIconMap.java` + `InspectScreen.java` + `client/src/main/java/com/bong/client/inventory/component/GridSlotComponent.java` + `client/src/main/java/com/bong/client/BongHud.java`
  - `BlockVanillaIconMap`：新增 `isKnownBlockItem(String)`（:65）、`usesVanillaItemIcon(String)`（:127）分流谓词 + `drawVanillaIcon(DrawContext, itemId, dx, dy, size)`（:150）统一原生渲染（`createStackFor`/`createVanillaBlockStack` → `DrawContext.drawItem`），无 BlockItem 的特殊块优雅降级回扁平占位贴图
  - 四处渲染路径接入：`GridSlotComponent` 单格槽、`InspectScreen.drawItemTextureRaw`（:3223）多格 overlay + 拖拽跟随、`BongHud.drawItemTexture` HUD 快捷栏
  - `bindBlockItemToSkillBar` 对 vanilla 物品留空 iconTexture（blank），让 HUD 走 itemTexture 命令分流到原生方块图标分支（:2599-2601）
  - 测试 `BlockVanillaIconMapTest`（11 cases）：`usesVanillaItemIcon` 全分支 + `drawVanillaIcon` 早退分支 + `createStackFor`/`createVanillaBlockStack` vanilla: 解析

### 关键 commit

- `8db5df8d5`（2026-06-15）chore(plan)：骨架→active（消费实现）
- `c1b6ebfc7`（2026-06-15）P0+P2：拖放方块入快捷栏绑定 SkillBar + 菜单左右键命中
- `7356fd3ad`（2026-06-15）P3+P1：vanilla 方块原生图标 + 放置后槽位自洽

### 测试结果

- `cd client && JAVA_HOME=.../java-17-openjdk-amd64 ./gradlew test build` → **BUILD SUCCESSFUL**，全仓 **2789 tests，0 failures / 0 errors**
- 本 plan 新增/扩充测试：`InspectScreenSkillBarBindItemTest`（7）、`InspectScreenSkillBarHydrateTest`（8，新建）、`InspectScreenContextMenuClickTest`（8，新建）、`BlockVanillaIconMapTest`（11）

### 跨仓库核验

- **client**（本 plan 主战场）：`InspectScreen.isBlockQuickBarBindable` / `InspectScreen.bindBlockItemToSkillBar` / `InspectScreen.handleContextMenuClick` / `BlockVanillaIconMap.isKnownBlockItem` / `BlockVanillaIconMap.usesVanillaItemIcon` / `BlockVanillaIconMap.drawVanillaIcon` / `GridSlotComponent`(vanilla 分流) / `BongHud.drawItemTexture`(vanilla 分流) 均命中
- **server**：**不变**——底层放置链路（`server/src/world/block_place.rs::block_item_to_state` vanilla: 直通、`handle_block_place_requests` 落 `Vanilla{state}`）本已正常，本 plan 不改放置协议与落块逻辑
- **agent**：不涉及
- **schema**：不变——P0 复用既有 `skill_bar_bind` C2S，P1 复用既有 `inventory_snapshot`，无新增协议/无 sample 改动

### 遗留 / 后续

- §N #3 的"拖放/选中/放置"轻量反馈（选中槽高亮、放置成功音效）未做，按收口结论留待后续 plan。
- BlockPicker / 方块放置整链是 dev-only 画廊审阅工具链，无 worldview gameplay 锚点；若未来要转生产建造体验，需另立 plan 接 worldview 经济/采集语义。
