# Bong · plan-inspect-cleanup-v1 · 骨架

**Client UI 整改**——砍快捷使用 tab、手搓 tab 按 mockup 重写、清理冗余快捷键（I/K/J）、InsightOffer 接入真实链路。client-only 改动合并一个 plan。

**交叉引用**：
- `plan-craft-v1`（skeleton）：手搓 inventory 标签集成
- `plan-weapon-v1`（finished）：InspectScreen tab 体系 + E 键 Mixin 拦截
- `plan-hotbar-modify-v2`（finished）：功法 tab（TechniquesTabPanel）
- `plan-alchemy-v1`（finished）：AlchemyScreen + 炼丹炉方块交互
- `plan-tribulation-v1`（finished）：server `tribulation_heart_demon_offer_emit.rs` → `HeartDemonOfferV1` 真实链路
- `docs/mockup-craft-ui.html`：手搓 UI 视觉规格（640×340 三栏布局，对标 AlchemyScreen owo-lib 风格）

**前置依赖**：
- `InspectScreen` ✅（`client/.../inventory/InspectScreen.java`，6 tab）
- `CraftTabPanel` ✅（`client/.../inventory/component/CraftTabPanel.java`，当前二栏文字占位）
- `CraftStore` / `CraftSessionStateView` / `CraftRecipe` ✅（数据层已实装）
- `AlchemyScreen` ✅（`client/.../alchemy/AlchemyScreen.java`，三栏 600×340 参考实现）
- `AlchemyScreenBootstrap` ✅（K 键快捷键，`alchemy/AlchemyScreenBootstrap.java`）
- `InspectScreenBootstrap` ✅（I 键快捷键 + E 键 Mixin 拦截，`inventory/InspectScreenBootstrap.java`）
- `InsightOfferScreenBootstrap` ✅（J 键 debug + store listener 自动弹窗，`insight/InsightOfferScreenBootstrap.java`）
- `HeartDemonOfferHandler` ✅（server → client 真实链路，`network/HeartDemonOfferHandler.java`）
- `GridSlotComponent` ✅（通用格子组件）

---

## 接入面 Checklist

- **进料**：`CraftStore`（配方 / session / outcome）/ `InventoryStateStore`（材料库存）/ `QuickUseSlotStore`（快捷使用——删 tab 但保留 store + HUD）/ `InsightOfferStore`（顿悟邀约——删 debug 键保留 store + 真实链路）
- **出料**：Client UI 变更（纯 client，无 server/agent 改动）
- **共享类型**：无新增
- **跨仓库契约**：仅 client
- **worldview 锚点**：无
- **qi_physics 锚点**：无

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | 砍快捷使用 tab + 清理冗余快捷键（I/K/J） | ⬜ |
| P1 | 手搓 tab → 独立 CraftScreen（对齐 mockup 三栏布局） | ⬜ |
| P2 | 饱和测试 | ⬜ |

---

## P0 — 砍快捷使用 tab + 清理冗余快捷键

### A. 砍快捷使用 tab（`InspectScreen.java`）

**删除**：
- `TAB_QUICK_USE = 4` 常量（`:57`）
- `TAB_NAMES` 数组中 "快捷使用" 项（`:60`）
- `quickUseTabContent` 字段（`:89`）+ `quickUseTabSlots[9]`（`:120`）
- `buildQuickUseTabContent()` 方法（`:661-693`）
- `leftCol.child(quickUseTabContent)` 注册（`:492-494`）
- `switchTab()` 内 `TAB_QUICK_USE` 分支（`:819-820`）
- `switchTab()` tabs 数组中对应项（`:804`）
- 拖放 highlight 中 `quickUseTabSlots` 引用（`:2245,2257`）

**重排**：`TAB_CRAFT` 从 5 → 4，`TAB_NAMES` 变为 `{"装备", "修仙", "技艺", "功法", "手搓"}`（5 tab）。

**保留**：
- `QuickUseSlotStore` + `QuickSlotConfig` + `QuickSlotConfigHandler` — 数据层不动
- `QuickBarHudPlanner` — HUD 快捷使用栏正常渲染
- `quickUseStrip`（`:122,219-220`）——InspectScreen 左侧竖条 F1-F9 快捷使用格仍保留（这是装备配置入口，不是 tab）
- `CombatHudBootstrap` F 键触发 — 运行时不受影响

### B. 砍 InspectScreen I 键（`InspectScreenBootstrap.java`）

E 键已通过 `MixinMinecraftClient` 拦截 → `InspectScreenBootstrap.openInspectScreen()`（plan-weapon-v1 §4.4），I 键冗余。

**删除**：
- `GLFW_KEY_I` KeyBinding 注册（`:50-51`）
- `onEndClientTick` 中 `keyBinding().wasPressed()` 轮询（`:43-44`）
- `keyBinding()` 方法整体（`:48-55`）
- `openScreenKey` 静态字段（`:24`）

**保留**：
- `openInspectScreen(MinecraftClient)` 公开方法 — Mixin 调用入口
- `ClientTickEvents` 注册 — 可能仍用于其他 tick 逻辑（检查后如无则也删）
- `DISCONNECT` listener — 断线清理

### C. 砍 AlchemyScreen K 键（`AlchemyScreenBootstrap.java`）

炼丹炉应只能通过右键炼丹炉方块打开，不该有快捷键。当前 K 键 crosshair 没对准方块就 fallback 到 `BlockPos(0,64,0)` — 错误行为。

**删除**：
- `GLFW_KEY_K` KeyBinding 注册（`:36-38`）
- `onEndClientTick` 中 `keyBinding().wasPressed()` 轮询（`:29-31`）
- `keyBinding()` 方法整体（`:34-41`）
- `openScreenKey` 静态字段（`:17`）
- `ClientTickEvents.END_CLIENT_TICK.register` 注册（`:23`）
- `register()` 中 keyBinding 相关日志（`:24`）

**保留**：
- `requestOpenAlchemyScreen()` 方法 — 改为由右键炼丹炉方块触发（server 发 `alchemy_furnace_open` → client handler 调用）
- `AlchemyScreen` 类本身不动

**TODO**：确认右键炼丹炉的触发链路是否已实装（server 方块交互 → CustomPayload → client 打开 AlchemyScreen）。若未实装则本 P0 仅删 K 键 + 留 `requestOpenAlchemyScreen` 供后续接入。

### D. 砍 InsightOffer J 键 debug mock（`InsightOfferScreenBootstrap.java`）

J 键是调试用，注入 `MockInsightOfferData.firstInduceBreakthrough()` mock 数据。真实链路已通：server `tribulation_heart_demon_offer_emit.rs` → `HeartDemonOfferV1` payload → client `HeartDemonOfferHandler` → `InsightOfferStore.replace()` → `onStoreChanged()` 自动弹窗。

**删除**：
- `GLFW_KEY_J` debug KeyBinding 注册（`:46-48`）
- `onEndClientTick` 中 debug 键轮询（`:56-60`）
- `debugKeyBinding()` 方法整体（`:44-50`）
- `debugKey` 静态字段（`:26`）
- `debugTriggerMockOffer()` 测试 hook（`:87-91`）
- `MockInsightOfferData.java` 整个文件（仅被 debug 键和测试 hook 引用）
- `ClientTickEvents.END_CLIENT_TICK.register` 注册（`:36`）— 无 debug 键后无 tick 逻辑

**保留**：
- `InsightOfferStore.addListener(onStoreChanged)` — 真实链路的 UI 触发入口
- `DISCONNECT` listener — 断线清理
- `onStoreChanged()` / `applyStoreChange()` — 自动弹窗逻辑
- `InsightOfferScreen` 本身不动

---

## P1 — 手搓 UI 对齐 mockup

### 问题

当前 `CraftTabPanel` 挤在 InspectScreen 172px 宽的 leftCol 里，是二栏纯文字占位。mockup（`docs/mockup-craft-ui.html`）设计为 640×340 独立面板，三栏布局——在 172px tab 内无法实现。

### 方案：手搓 tab → 独立 CraftScreen

参考 `AlchemyScreen`（`BaseOwoScreen<FlowLayout>`，600×340，三栏），新建 `CraftScreen`：

- **打开方式**：InspectScreen 手搓 tab 点击时 `client.setScreen(new CraftScreen())`，或直接 C 键打开
- **关闭方式**：Esc / C 键回 InspectScreen 或游戏

### CraftScreen 布局（对齐 mockup）

```
┌─ Header (20px) ──────────────────────────────────────┐
│  手搓台                                C 关闭 · 双击快速制作  │
├──────────┬────────────────────┬───────────┤
│ LEFT 160 │    MID flex        │ RIGHT 200 │
│          │                    │           │
│ 分类按钮   │  ─ 铁甲·胸甲 ─      │  🛡️ 大图标  │
│ 全/甲/兵/  │  3×3 材料格         │  铁甲·胸甲   │
│ 具/丹/杂   │  (GridSlotComponent)│  凡物·铁制   │
│          │                    │           │
│ 配方滚动列表 │  [ 材料需求 ]        │  防御 +8    │
│ 图标+名+数量 │  ✓ 铁矿 ×5         │  耐久 200   │
│          │  ✗ 骨币 ×3         │  重量 6.0   │
│ 🔒 未解锁   │                    │  制作 3.0s  │
│          │  ▓▓▓▓▓░░░ 65%      │           │
│          │                    │  [ 说明 ]   │
├──────────┴────────────────────┴───────────┤
│ Bottom (32px): [一键填充] [−1+] [开始手搓/材料不足] │
└──────────────────────────────────────────┘
Panel: 640×340, Surface.flat(#0D0D15) + outline(#4A4050)
```

### 关键组件

| 区域 | 组件 | 对标 mockup 元素 |
|------|------|----------------|
| Header | `LabelComponent` 标题 + hint | `.header-title` + `.header-hint` |
| 左栏 | 分类按钮行（6 个 `ButtonComponent`）+ 配方滚动列表（行=图标+名称+数量/🔒） | `.category-row` + `.recipe-scroll` |
| 中栏 | 配方标题 + 3×3 `GridSlotComponent` 材料格（ok/missing/empty 三态边框色）+ 材料需求文字清单 + 进度条 `FlowLayout` | `.mat-grid` + `.req-section` + `.prog-bar` |
| 右栏 | 产物大图标（128px `ItemIconComponent`）+ 属性表（防御/耐久/重量/制作时间）+ 说明文字 | `.output-icon` + `.stat-list` + `.req-section[说明]` |
| 底部 | [一键填充] + 数量 ±1 + [开始手搓]/[材料不足] `ButtonComponent` | `.bottom-bar` |

### 颜色表（mockup 正典）

```java
PANEL_BG     = 0xFF0D0D15
PANEL_BORDER = 0xFF4A4050
HEADER_BG    = 0xFF12121C
LEFT_BG      = 0xFF1A1814
LEFT_BORDER  = 0xFF4A4030
CAT_ACTIVE   = 0xFF3A3020  // border #8A7040, text #FFD080
CAT_INACTIVE = 0xFF2A2318  // border #5A4A30, text #AA9060
SELECTED_ROW = 0xFF2A3528  // border #5A8A50
SLOT_OK      = 0xFF3A6A3A
SLOT_MISSING = 0xFF6A3A3A
SLOT_EMPTY   = 0xFF222222
RIGHT_BG     = 0xFF14141C
RIGHT_BORDER = 0xFF3A3A50
CRAFT_BTN    = 0xFF2A4A2A  // border #4A8A4A, text #AAFFAA
CRAFT_DISABLED = 0xFF1A1A1A // border #3A3A3A, text #555555
```

### InspectScreen 手搓 tab 改造

InspectScreen 的 `TAB_CRAFT` 保留（作为入口），但 tab content 不再内嵌 `CraftTabPanel`，改为：
- 展示一个简要面板："C 打开手搓台" + 当前任务状态（从 `CraftStore.sessionState()` 读）
- 点击或按 C → `client.setScreen(new CraftScreen())`

### 清理

- 删除 `CraftTabPanel.java`（被 `CraftScreen` 替代）
- InspectScreen 中 `craftTab` 字段 + `dispose()` 调用简化

---

## P2 — 饱和测试

### P0A tab 删除（1-4）
1. InspectScreen tab 数量 = 5（装备/修仙/技艺/功法/手搓）
2. tab 切换 0→1→2→3→4 循环无越界
3. 快捷使用 HUD 正常渲染（QuickBarHudPlanner）
4. F 键触发快捷使用正常

### P0B I 键删除（5-7）
5. I 键按下无响应（不打开 InspectScreen）
6. E 键仍正常打开 InspectScreen（Mixin 拦截链路）
7. 断线后 store 清理正常

### P0C K 键删除（8-10）
8. K 键按下无响应（不打开 AlchemyScreen）
9. 右键炼丹炉方块仍能打开 AlchemyScreen（若已实装；否则标记 TODO）
10. AlchemyScreen 内部功能不受影响

### P0D J 键删除（11-14）
11. J 键按下无响应（不弹 mock 邀约）
12. server 发送 `HeartDemonOfferV1` → client 自动弹出 InsightOfferScreen（真实链路）
13. 断线后 InsightOfferStore 正确清理
14. `MockInsightOfferData.java` 不存在（已删）

### P1 CraftScreen（15-25）
15. C 键打开 CraftScreen / Esc 关闭
16. InspectScreen 手搓 tab 点击打开 CraftScreen
17. 左栏分类过滤（全部 → 甲 → 兵 → 具 → 丹 → 杂）正确筛选
18. 配方列表 unlocked 显示图标+名+数量 / locked 显示 🔒+解锁条件
19. 选中配方 → 中栏 3×3 材料格填充（ok=绿边 / missing=红边 / empty=暗边）
20. 中栏材料需求清单 ✓/✗ 实时反映背包库存
21. 点击 [开始手搓] → `sendCraftStart` → 进度条走动
22. 点击 [取消任务] → `sendCraftCancel` → 材料返还显示
23. 右栏产物预览：图标 + 属性表（防御/耐久/重量/制作时间）+ 说明
24. [材料不足] 按钮灰态 + tooltip
25. CraftScreen 关闭后 CraftStore listener 正确解绑（无泄漏）

### 回归（26-28）
26. AlchemyScreen 独立 screen 不受影响
27. InspectScreen 其他 4 tab（装备/修仙/技艺/功法）功能不变
28. InsightOfferScreen 真实链路弹窗不受影响

## Finish Evidence

### 落地清单

- P0A：`client/src/main/java/com/bong/client/inventory/InspectScreen.java` 移除 `TAB_QUICK_USE`、`quickUseTabContent`、`quickUseTabSlots` 和旧 tab highlight 路径；`QuickUseSlotStore` / `QuickBarHudPlanner` / F1-F9 `quickUseStrip` 保留。
- P0B：`client/src/main/java/com/bong/client/inventory/InspectScreenBootstrap.java` 移除 I 键注册与 tick 轮询；`openInspectScreen(MinecraftClient)` 仍作为 E 键 mixin 入口。
- P0C：`client/src/main/java/com/bong/client/alchemy/AlchemyScreenBootstrap.java` 移除 K 键注册与 fallback `BlockPos(0,64,0)`；`client/src/main/java/com/bong/client/mixin/MixinClientPlayerInteractionManagerAlchemy.java` 继续只在已知炼丹炉右键时打开 `AlchemyScreen`。
- P0D：`client/src/main/java/com/bong/client/insight/InsightOfferScreenBootstrap.java` 移除 J 键 mock 注入与测试 hook；`MockInsightOfferData` 从 main source 删除，测试数据迁入 `client/src/test/java/com/bong/client/insight/InsightOfferFixtures.java`；真实 `InsightOfferStore` listener 保留。
- P1：`client/src/main/java/com/bong/client/inventory/InspectScreen.java` 的“手搓”tab 改为 `CraftScreen` 入口；`client/src/main/java/com/bong/client/inventory/component/CraftTabPanel.java` 删除；`client/src/main/java/com/bong/client/craft/CraftScreenLayout.java` / `CraftMaterialGrid.java` 固定 640x340、三栏、3x3 材料格契约。
- P2：`InspectScreenQuickUseTabTest`、`CraftUxViewModelTest`、`InsightOffer*Test` 和 `MixinClientPlayerInteractionManagerAlchemyTest` 覆盖 tab 数量、3x3 手搓布局、真实顿悟 fixture、store/dispatcher 和炼丹炉右键规则。

### 关键 commit

- `a06881e6c` · 2026-05-13 · `refactor(client): 精简检视手搓入口`
- `a3f94cce3` · 2026-05-13 · `refactor(client): 移除冗余检视炼丹快捷键`
- `43d53aeba` · 2026-05-13 · `refactor(client): 移除顿悟 mock 调试入口`

### 测试结果

- `JAVA_HOME="$HOME/.sdkman/candidates/java/17.0.18-amzn" ./gradlew test --tests "com.bong.client.inventory.InspectScreenQuickUseTabTest" --tests "com.bong.client.craft.CraftUxViewModelTest" --tests "com.bong.client.insight.*" --tests "com.bong.client.mixin.MixinClientPlayerInteractionManagerAlchemyTest"` — BUILD SUCCESSFUL。
- `JAVA_HOME="$HOME/.sdkman/candidates/java/17.0.18-amzn" ./gradlew test build` — BUILD SUCCESSFUL。
- `git diff --check` — 通过。

### 跨仓库核验

- server：无改动；炼丹炉右键仍通过既有 `alchemy_open_furnace` client request。
- agent：无改动。
- client：命中 `InspectScreen::tabNamesForTests`、`CraftScreen`、`CraftScreenLayout.MATERIAL_ROWS/COLUMNS`、`AlchemyScreenBootstrap.requestOpenAlchemyScreen`、`InsightOfferScreenBootstrap.applyStoreChange`、`HeartDemonOfferHandler` / `InsightOfferStore.replace` 真实链路。

### 遗留 / 后续

- 本 plan 不新增 server/agent 协议；手搓配方内容与 craft skeleton/后续 plan 继续按既有 `CraftStore` / `ClientRequestSender.sendCraftStart` 接入。
