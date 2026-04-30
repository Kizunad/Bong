# Bong · plan-inventory-v2 · 骨架

**背包精化**——修复 plan-inventory-v1 已知的两处实装缺口：`add_item_to_player_inventory` 当前硬编 `(row:0, col:0)` 不做 grid placement；同 template 物品入包不合并 stack。塔科夫式 grid 数据结构（`PlacedItemState` + `placed_item_footprints_overlap`）已就绪，本 plan 补齐空位查找算法 + 堆叠合并逻辑。

**世界观锚点**：plan-inventory-v1 已确立的塔科夫风格背包（行列网格 + 占地大小 + 堆叠数）；本 plan 仅补完算法，无新世界观引入。

**代码锚点**：
- `server/src/inventory/mod.rs:820 add_item_to_player_inventory`（**当前 bug**：`row:0, col:0` 硬编，多株同时入包冲撞）
- `server/src/inventory/mod.rs:867 main_pack.items.push(PlacedItemState { row: 0, col: 0, instance })`（核心缺陷点）
- `server/src/inventory/mod.rs:193 PlacedItemState`（数据结构已就绪）
- `server/src/inventory/mod.rs:2813 placed_item_footprints_overlap`（碰撞检测函数已存在，可复用）

**主要调用方**：`plan-lingtian-v1`（harvest drop）· `plan-mineral-v1`（采矿入包）· `plan-fauna-v1`（兽核拾取）· `plan-alchemy-v1`（成丹入包）

**交叉引用**：`plan-inventory-v1`（active，本 plan 修复其已知缺口）· `plan-tools-v1`（采药工具入包路径）

---

## §1 Grid Placement（塔科夫式空位查找）

`add_item_to_player_inventory` 当前 `row:0, col:0` 硬编 → 多株同时入包 row-col 冲撞，客户端渲染堆叠异常：

- [ ] `find_free_slot(container: &ContainerState, item_w: u8, item_h: u8) -> Option<(u8, u8)>`：
  - 扫描顺序：row-major（左→右，上→下）
  - 对每个候选 (r, c) 构造 `PlacedItemState` 占位 → 调 `placed_item_footprints_overlap` 检测与既有 items 冲撞
  - 越界（r + h > grid_h 或 c + w > grid_w）跳过
  - 找到第一个可放点即返回 `Some((r, c))`；扫完无解 → `None`
- [ ] `add_item_to_player_inventory` 改写：
  - 先查 `ItemTemplate.grid_w / grid_h`
  - 调 `find_free_slot`；找到 → 用返回的 (row, col) push
  - 找不到 → `Err(InventoryGrantError::Full)`（当前行为：无声塞 0,0）
- [ ] 新增 `InventoryGrantError` 枚举区分 Full / SchemaInvalid / Other
- [ ] **饱和单测**（CLAUDE.md Testing 原则）：
  - happy：1×1 空背包入包 → (0,0)；继续入包第二个 → (0,1)
  - 跨容器：1×1 物品在第 5 列后跳第 6 列开头
  - 大物品：2×3 物品占 6 格，剩余空位正确
  - 边界：最后一行最后一列正好放下；超 1 格放不下
  - 错误：1×1 装满 16 格后第 17 个入包 → `Err(Full)`
  - 错误：grid_w / grid_h = 0 的 template → `Err(SchemaInvalid)`
  - 已有 2×1 物品在 (0,0)，新 1×1 应放 (0,2)（不挤占已有 footprint）

---

## §2 堆叠合并（Stack Merging）

`add_item_to_player_inventory` 不与既有同 template 实例合并，也不校验 `stack_count` 上限：

- [ ] `ItemTemplate` 追加字段 `max_stack: u32`（默认 1 = 不可堆叠）；server schema + agent schema 双端同步
- [ ] 合并逻辑（`add_item_to_player_inventory` 入口）：
  ```text
  if template.max_stack > 1:
    for placed in container.items where placed.instance.template_id == template.id:
      if placed.instance.stack_count < template.max_stack:
        capacity = template.max_stack - placed.instance.stack_count
        merge = min(capacity, incoming_count)
        placed.instance.stack_count += merge
        incoming_count -= merge
        if incoming_count == 0: break
    if incoming_count > 0:
      // 剩余走 §1 grid placement 新建格
  ```
- [ ] `InventoryGrantReceipt` 追加 `merged_into: Option<InstanceId>` + `created_new: Option<InstanceId>`：调用方可知是合并到既有格还是新格
- [ ] `inventory.revision` 在合并 / 新格场景一致 bump
- [ ] **饱和单测**：
  - happy 合并：草药入包两次 max_stack=99 → 同格 stack_count=2
  - 满栈跨格：max_stack=10 已满 → 新增第 11 个 → 既有格 10 + 新格 1
  - 不可堆叠：max_stack=1（默认）→ 每次新格，永不合并
  - 多既有格：3 个未满栈 + 1 个空位，新入 N 个 → 优先填满最早的，剩余新格
  - 跨 template：同 template 才合并；纹路不同 instance（如 quality 字段不同）的合并策略待定（默认禁止合并以保留个体差异）

---

## §3 数据契约变更

- [ ] `ItemTemplate.max_stack: u32`：server `inventory/template.rs` + agent `schema/inventory.ts` 同步；fixture 全量补 `max_stack` 默认 1
- [ ] `InventoryGrantError` 枚举：`schema/inventory.rs` 暴露 + agent schema 反序列化
- [ ] `InventoryGrantReceipt` 扩展：`merged_into` / `created_new` 让调用方区分操作类型
- [ ] schema sample 更新：`agent/packages/schema/samples/` 加 max_stack 字段示例

---

## §4 实施节点

- [ ] **P0**：`find_free_slot` + `add_item_to_player_inventory` grid 接入 + Full 错误
- [ ] **P1**：`max_stack` 字段（schema 双端同步） + 堆叠合并逻辑 + receipt 扩展
- [ ] **P2**：饱和单测（grid + 合并各 5+ case 覆盖边界 / 错误 / 多既有格）

---

## §5 开放问题

- [ ] 背包满时的客户端 UI 提示——item 掉地（plan-tsy-loot 自动 spawn）还是操作被拒绝？需在 plan-inventory-v1 / plan-HUD-v1 协调
- [ ] 同 template 但 `quality` / `augments` 不同的 instance 是否允许合并？（默认禁止；需 v3 引入"近似合并"）
- [ ] 客户端是否需要新事件 `item_merged` / `item_created`？（或单纯靠 `InventoryStateV1` diff 推断）
- [ ] `max_stack` 默认值——草药 99 / 灵石 999 / 丹药 是否可堆叠？（配置化，非代码决策）

---

## §6 进度日志

- 2026-04-29：骨架立项——覆盖 plan-inventory-v1 已知两处实装缺口（add_item_to_player_inventory grid 硬编 + stacking 不合并）。`PlacedItemState` 数据结构 + `placed_item_footprints_overlap` 碰撞函数已就绪，本 plan 仅补算法层。
