# Bong · plan-inspect-cleanup-v1 · 骨架

**InspectScreen 整改**——砍快捷使用 tab + 手搓 tab 按 mockup 重写。两件独立但同模块的 client-only 改动合并一个 plan。

**交叉引用**：
- `plan-craft-v1`（skeleton）：手搓 inventory 标签集成
- `plan-weapon-v1`（finished）：InspectScreen tab 体系
- `plan-hotbar-modify-v2`（finished）：功法 tab（TechniquesTabPanel）
- `docs/mockup-craft-ui.html`：手搓 UI 视觉规格（640×340 三栏布局，对标 AlchemyScreen owo-lib 风格）

**前置依赖**：
- `InspectScreen` ✅（`client/.../inventory/InspectScreen.java`，6 tab）
- `CraftTabPanel` ✅（`client/.../inventory/component/CraftTabPanel.java`，当前二栏文字占位）
- `CraftStore` / `CraftSessionStateView` / `CraftRecipe` ✅（数据层已实装）
- `AlchemyScreen` ✅（`client/.../alchemy/AlchemyScreen.java`，三栏 600×340 参考实现）
- `GridSlotComponent` ✅（通用格子组件）

---

## 接入面 Checklist

- **进料**：`CraftStore`（配方 / session / outcome）/ `InventoryStateStore`（材料库存）/ `QuickUseSlotStore`（快捷使用——删 tab 但保留 store + HUD）
- **出料**：InspectScreen UI 变更（纯 client，无 server/agent 改动）
- **共享类型**：无新增
- **跨仓库契约**：仅 client
- **worldview 锚点**：无
- **qi_physics 锚点**：无

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | 砍快捷使用 tab | ⬜ |
| P1 | 手搓 tab → 独立 CraftScreen（对齐 mockup 三栏布局） | ⬜ |
| P2 | 饱和测试 | ⬜ |

---

## P0 — 砍快捷使用 tab

### InspectScreen（`InspectScreen.java`）

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

### 测试

- InspectScreen tab 数量 6 → 5
- tab 切换循环无越界
- 快捷使用 HUD + F 键触发不受影响

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

### P0 tab 删除（1-4）
1. InspectScreen tab 数量 = 5（装备/修仙/技艺/功法/手搓）
2. tab 切换 0→1→2→3→4 循环无越界
3. 快捷使用 HUD 正常渲染（QuickBarHudPlanner）
4. F 键触发快捷使用正常

### P1 CraftScreen（5-15）
5. C 键打开 CraftScreen / Esc 关闭
6. InspectScreen 手搓 tab 点击打开 CraftScreen
7. 左栏分类过滤（全部 → 甲 → 兵 → 具 → 丹 → 杂）正确筛选
8. 配方列表 unlocked 显示图标+名+数量 / locked 显示 🔒+解锁条件
9. 选中配方 → 中栏 3×3 材料格填充（ok=绿边 / missing=红边 / empty=暗边）
10. 中栏材料需求清单 ✓/✗ 实时反映背包库存
11. 点击 [开始手搓] → `sendCraftStart` → 进度条走动
12. 点击 [取消任务] → `sendCraftCancel` → 材料返还显示
13. 右栏产物预览：图标 + 属性表（防御/耐久/重量/制作时间）+ 说明
14. [材料不足] 按钮灰态 + tooltip
15. CraftScreen 关闭后 CraftStore listener 正确解绑（无泄漏）

### 回归（16-17）
16. AlchemyScreen 不受影响（独立 screen）
17. InspectScreen 其他 4 tab（装备/修仙/技艺/功法）功能不变
