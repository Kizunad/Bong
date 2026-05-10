# Bong · plan-craft-ux-v1 · 手搓系统 Client UI

手搓系统 client UI 重做。当前 `plan-craft-v1` ✅ finished 建立了 server 侧 CraftRegistry + CraftRecipe + 手搓逻辑，但 **client 侧 UI 是临时骨架**——没有配方列表、材料匹配不直观、进度不可见。本 plan 做一个完整的 craft screen：配方列表（左栏）+ 材料放置（中栏）+ 产物预览（右栏），UI 高度与现有所有 tab 对齐，宽度可以更宽，交互杠做好。

**世界观锚点**：`worldview.md §九` 以物易物经济 → 手搓是凡物获取的主要途径 · `§十` 资源匮乏 → UI 要清晰显示材料缺多少、有多少，避免浪费

**前置依赖**：
- `plan-craft-v1` ✅ → CraftRegistry / CraftRecipe / CraftSession / server 逻辑全套
- `plan-inventory-v2` ✅ → 物品栏系统 / ItemStack
- `plan-HUD-v1` ✅ → BongHudOrchestrator / owo-lib UI 框架
- `plan-input-binding-v1` ✅ → keybind 系统
- `plan-item-visual-v1` ✅ → 物品 icon / tooltip / 稀有度色

**反向被依赖**：
- `plan-armor-visual-v1` 🆕 → 盔甲 craft 配方需要本 plan 的 UI 来展示
- `plan-poison-trait-v1` 🆕 → 毒丹研磨配方走本 UI

---

## 接入面 Checklist

- **进料**：`craft::CraftRegistry`（所有配方列表）/ `craft::CraftRecipe { id, category, inputs, output, craft_time_ticks }`/ `craft::CraftSession`（进行中的手搓会话）/ `inventory::PlayerInventory`（材料检查）/ MC `HandledScreen` 框架 / owo-lib UI components
- **出料**：`CraftScreen.java`（全功能 craft UI）+ `CraftRecipeListWidget`（左栏配方列表 + 搜索 + 分类 tab）+ `CraftMaterialGrid`（中栏材料放置 + 自动填充）+ `CraftProgressBar`（进度条 + 音效 tick）+ `CraftOutputPreview`（右栏产物预览 + 属性）
- **跨仓库契约**：server `CraftRecipeListS2c` packet（配方列表同步）+ `CraftSessionS2c`（进度同步）→ client `CraftScreen` 消费

---

## §0 设计轴心

- [ ] **高度对齐**：CraftScreen 高度与 InventoryScreen / ForgeScreen / AlchemyScreen 完全一致（统一 tab 系统切换体验）
- [ ] **宽度更大**：比标准 inventory screen 宽 40%（三栏布局需要空间）—— 左 25% / 中 45% / 右 30%
- [ ] **交互杠**：底部常驻操作栏——「开始制作」按钮 + 数量选择 + 一键填充材料
- [ ] **配方可发现**：左栏列出所有已知配方（未知配方灰色 "???" + 需要解锁条件提示）
- [ ] **材料缺失清晰**：材料格子红色高亮缺失的 + 绿色高亮满足的 + tooltip 显示"需要 ×5，拥有 ×3"
- [ ] **进度可见**：制作中有进度条 + tick 音效 + 完成闪光

---

## UI 布局设计

```
┌──────────────────────────────────────────────────────────────────────┐
│  [搜索栏🔍]                                          [X 关闭]        │
├──────────┬────────────────────────────────┬──────────────────────────┤
│ 配方列表  │       材料放置区               │     产物预览             │
│          │                                │                          │
│ [全部]    │  ┌──┐ ┌──┐ ┌──┐              │  ┌────────┐              │
│ [盔甲]    │  │材│ │材│ │材│              │  │ 产物图 │              │
│ [武器]    │  │料│ │料│ │料│              │  │  标    │              │
│ [丹药]    │  │1 │ │2 │ │3 │              │  └────────┘              │
│ [工具]    │  └──┘ └──┘ └──┘              │  铁甲·胸甲               │
│ [杂项]    │  ┌──┐ ┌──┐ ┌──┐              │  防御: +8                │
│          │  │材│ │材│ │材│              │  耐久: 200               │
│ ▸ 骨甲   │  │料│ │料│ │料│              │  材质: 凡物·铁制          │
│ ▸ 兽皮甲 │  │4 │ │5 │ │6 │              │                          │
│ ▸ 铁甲 ← │  └──┘ └──┘ └──┘              │  [材料需求]              │
│ ▸ 铜甲   │                                │  铁矿 ×5 (✓拥有7)      │
│ ▸ ...    │       → → → [进度条] → → →    │  骨币 ×3 (✗拥有1)      │
│          │                                │                          │
├──────────┴────────────────────────────────┴──────────────────────────┤
│  [一键填充]        数量: [- 1 +]         [开始制作]                  │
└──────────────────────────────────────────────────────────────────────┘
```

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | CraftScreen 框架 + 三栏布局 + 配方列表 server 同步 + 配方分类 tab | ⬜ |
| P1 | 材料放置区（自动填充 + 手动放置 + 缺失高亮）+ 产物预览（属性显示） | ⬜ |
| P2 | 交互杠（开始制作 + 数量选择 + 进度条 + 完成动画/音效） | ⬜ |
| P3 | 配方解锁系统 UI（未知配方 "???" + 解锁条件 tooltip）+ 搜索栏 | ⬜ |
| P4 | 全配方 × 材料充足/不足 × 批量制作 饱和化测试 + tab 对齐验证 | ⬜ |

---

## P0 — CraftScreen 框架 + 配方列表 ⬜

### 交付物

1. **`CraftScreen.java`**（`client/src/main/java/com/bong/client/craft/CraftScreen.java`）
   - 继承 `HandledScreen<CraftScreenHandler>`
   - 尺寸：宽 280px / 高与 InventoryScreen 一致（166px standard + 按需扩展）
   - 三栏 FlowLayout（owo-lib）：左 70px / 中 126px / 右 84px
   - keybind 触发：默认 `C` 键打开（注册到 input-binding）
   - 背景贴图：深灰木质纹理（与 forge/alchemy screen 同系列，统一 UI 风格）

2. **`CraftRecipeListWidget`**（左栏）
   - 从 `CraftRecipeListS2c` packet 加载所有配方
   - 分类 tab（顶部横向）：全部 / 盔甲 / 武器 / 丹药 / 工具 / 杂项
   - 每个配方一行：icon(16×16) + 名称 + 可制作数量（绿色数字）/ 不可制作（灰色）
   - 点击选中 → 右侧刷新材料+产物
   - 滚动列表（owo-lib ScrollContainer）

3. **`CraftRecipeListS2c` 协议**（`server/src/network/craft_recipe_list.rs`）
   - 玩家打开 CraftScreen 时 server 发送所有已解锁配方列表
   - `CraftRecipeListS2c { recipes: Vec<CraftRecipePreview> }`
   - `CraftRecipePreview { id, name, category, output_icon_id, can_craft: bool }`

4. **tab 高度对齐验证**
   - CraftScreen 打开后与 InventoryScreen / ForgeScreen 切换 → 高度一致 → 无跳动

### 验收抓手
- 测试：`client::craft::tests::screen_dimensions_match_inventory` / `server::craft::tests::recipe_list_packet_serializes` / `client::craft::tests::category_tab_filters`
- 手动：按 C → CraftScreen 打开 → 左栏配方列表 → 点分类 tab 切换 → 滚动

---

## P1 — 材料放置 + 产物预览 ⬜

### 交付物

1. **`CraftMaterialGrid`**（中栏）
   - 配方选中后：显示所需材料 slot（最多 3×3 = 9 格）
   - 每格显示：需要的物品 icon + 需要数量 + 拥有数量
   - 颜色编码：材料充足 = 绿色边框 / 不足 = 红色边框 + "×3/×5" 红色文字
   - 「一键填充」按钮：从 inventory 自动填入所有可用材料

2. **`CraftOutputPreview`**（右栏）
   - 产物大图标（32×32）+ 名称 + 属性列表
   - 属性按物品类型动态：
     - 盔甲：防御值 / 耐久 / 材质
     - 武器：伤害 / 攻速 / 材质
     - 丹药：效果描述 / 持续时间
     - 通用：名称 / 稀有度 / 重量
   - 下方材料需求清单：每行 = icon + "×5 (✓拥有7)" 绿色 / "×3 (✗拥有1)" 红色

3. **材料缺失 tooltip**
   - hover 红色材料格 → tooltip "需要铁矿 ×5，当前拥有 ×1，还差 ×4"
   - hover 产物 → 完整属性 tooltip（复用 item-visual-v1 tooltip 系统）

### 验收抓手
- 测试：`client::craft::tests::material_grid_green_when_sufficient` / `client::craft::tests::material_grid_red_when_missing` / `client::craft::tests::auto_fill_from_inventory`
- 手动：选铁甲配方 → 中栏显示铁矿×5+骨币×3 → 有铁矿7个→绿边 / 骨币1个→红边 → 一键填充 → 有的自动进去

---

## P2 — 交互杠 + 制作进度 ⬜

### 交付物

1. **底部交互杠**（`CraftActionBar.java`）
   - 固定在 screen 底部，高 24px
   - 左侧：「一键填充」按钮（从 inventory 填材料）
   - 中间：数量选择器 `[- 1 +]`（批量制作，最大 = min(各材料可做数)）
   - 右侧：「开始制作」按钮（绿色，材料不足时灰色不可点 + tooltip "材料不足"）

2. **制作进度条**
   - 点击「开始制作」→ 中栏出现水平进度条（材料格下方）
   - 进度 = `CraftSession.progress_ticks / recipe.craft_time_ticks`
   - 进度条颜色：填充 #44AA44 绿色 → 到达 100% 闪金光
   - 进度 tick 期间：每 20 tick 播一次制作音效（`minecraft:block.anvil.use`(pitch 1.5, volume 0.1) — 轻微敲击声）

3. **制作完成反馈**
   - 完成时：产物 icon 微放大 1.2× → 1.0（0.2s ease-out）+ 金色粒子 flash + 音效 `craft_complete.json`（`minecraft:entity.player.levelup`(pitch 1.5, volume 0.2)）
   - 产物自动进入 inventory（如满 → 提示"背包已满"红色 toast）
   - 批量制作：逐个完成，每个播一次小 flash（不是全做完才反馈）

4. **`CraftSessionC2s` / `CraftSessionS2c` 协议**
   - C2s：`{ recipe_id, quantity }` 请求开始制作
   - S2c：`{ progress_ticks, total_ticks, completed_count, error }` 进度同步
   - server 校验：材料够 / 空间够 / 不在战斗中

### 验收抓手
- 测试：`client::craft::tests::start_button_disabled_when_missing` / `server::craft::tests::session_progress_ticks` / `client::craft::tests::completion_flash_animation` / `server::craft::tests::batch_craft_sequential`
- 手动：选配方 → 材料够 → 点开始 → 进度条走 → 完成闪光+音效 → 产物进背包 → 批量 3 个 → 逐个 flash

---

## P3 — 配方解锁 + 搜索 ⬜

### 交付物

1. **配方解锁 UI**
   - 未解锁配方在列表中显示为 "???"（灰色 + 锁 icon）
   - hover 未解锁 → tooltip 显示解锁条件："需要引气境界" / "需要找到残卷碎片" / "需要向 NPC 学习"
   - 解锁瞬间：列表中该配方 flash 金色 + toast "新配方：铁甲"

2. **搜索栏**（顶部）
   - 输入文字 → 实时过滤配方列表（搜名称 + 材料名）
   - 中文模糊匹配（"铁" → 显示铁甲/铁剑/铁矿相关）
   - 清空按钮（×）

3. **收藏/置顶**
   - 右键配方 → "收藏" → 列表顶部置顶显示（★ 标记）
   - 常用配方快速访问

### 验收抓手
- 测试：`client::craft::tests::locked_recipe_shows_question_marks` / `client::craft::tests::search_filters_by_name` / `client::craft::tests::search_filters_by_material` / `client::craft::tests::favorite_pins_to_top`
- 手动：看到 "???" → hover 看解锁条件 → 搜索"甲" → 只显示盔甲类 → 右键收藏铁甲 → 置顶

---

## P4 — 饱和化测试 + tab 对齐 ⬜

### 交付物

1. **tab 对齐验证**
   - CraftScreen / InventoryScreen / ForgeScreen / AlchemyScreen 连续切换 → 高度严格一致 → 无 UI 跳动
   - 截图对比 4 个 screen 的 top/bottom 边缘 y 坐标

2. **全配方覆盖**
   - 所有已注册 craft 配方（盔甲 6 + 武器 N + 工具 N + 丹药 N）× 材料充足/不足 → UI 正确响应
   - 批量制作 10 个 → 每个逐次完成 + 正确扣材料

3. **边界 case**
   - 制作中关闭 screen → 取消制作 + 材料退回
   - 制作中被攻击 → 取消制作 + 材料退回 + toast "制作被打断"
   - inventory 满 → "背包已满" 提示 + 不开始制作
   - 同时打开多个制作（不允许）→ toast "正在制作中"

### 验收抓手
- tab 对齐截图 + 全配方 e2e + 边界 case 覆盖
- 低配 30fps 下 UI 响应 < 16ms

---

## Finish Evidence（待填）

- **落地清单**：`CraftScreen` / `CraftRecipeListWidget` / `CraftMaterialGrid` / `CraftOutputPreview` / `CraftActionBar` / 配方解锁 UI / 搜索栏 / 进度条 / 完成动画 / `CraftRecipeListS2c` + `CraftSessionC2s/S2c` 协议
- **关键 commit**：P0-P4 各自 hash
- **遗留 / 后续**：高级配方需 forge/alchemy 而非 craft → 各自 screen 已有 / craft 配方来源扩展（NPC 教导/残卷学习）→ 需 npc-engagement 联动
