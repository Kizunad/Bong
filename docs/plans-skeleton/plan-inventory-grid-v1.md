# Bong · plan-inventory-grid-v1 · 骨架

**背包格位放置与堆叠**。修复 `add_item_to_player_inventory` 硬编码 `row:0, col:0` 导致多物品冲突；实装贪心 free-slot 算法；实装 stackable 物品的合并逻辑与上限校验。

**世界观锚点**：无直接锚点（底层存储系统）。

**接入面**：
- **进料**：任何写入背包的路径（`harvest drop`、`pickup dropped loot`、`alchemy brew result`、`death drop recovery`）→ 都调 `add_item_to_player_inventory`
- **出料**：`InventoryEventV1::Added`（已有）→ 客户端 `InspectScreen` grid 渲染
- **共享类型**：`PlacedItem { row, col, item_instance_id }`（已有）· `ContainerV1.items: Vec<PlacedItem>`（已有）· `ItemTemplateV1.grid_w / grid_h`（已有）
- **worldview 锚点**：无

**交叉引用**：`plan-inventory-v1`（完成，基座；本 plan 修复其已知缺口）

---

## §0 设计轴心

- [ ] **贪心优先**：从左上角开始扫描，找到能放下 `grid_w × grid_h` 的第一个空矩形区域
- [ ] **堆叠合并**：可堆叠物品（`stack_limit > 1`）优先合并到同 template 的既有实例，溢出才开新格位
- [ ] **Fail-fast**：背包满时返回 `InventoryError::Full`，调用方负责处理（掉地 / 拒绝）
- [ ] **不改公共接口**——`add_item_to_player_inventory` 签名不变，只改内部实现

## §1 格位放置算法（P0）

**阶段状态**：⬜

**可核验交付物**：
- `server/src/inventory/placement.rs`（新文件）：
  ```rust
  pub fn find_free_slot(
      container: &Container,
      item_w: u8,
      item_h: u8,
  ) -> Option<(u8, u8)>
  ```
  - 逻辑：构建 `ROWS × COLS` 占用位图 → 逐行逐列扫描 → 检查 `item_w × item_h` 矩形是否全空 → 返回第一个合法 `(row, col)`
  - 容量来自 `Container.grid_rows × Container.grid_cols`（或从物品总数推断，按现有方案）
- `server/src/inventory/mod.rs::add_item_to_player_inventory`：
  - 替换 `push({ row:0, col:0 })` → 调 `find_free_slot` → `Ok((r,c))` 则放置，`None` → return `Err(InventoryError::Full)`
- 触发测试场景：采集多株药草同帧入包，验证每株 `(row, col)` 唯一
- 测试 `inventory::placement::*`：
  - `find_free_slot_empty_container`
  - `find_free_slot_partial_fill`
  - `find_free_slot_full_returns_none`
  - `multi_item_burst_no_collision`
  - `large_item_2x2_placement`
  - `boundary_exact_fit`（6 单测）

## §2 堆叠合并（P1）

**阶段状态**：⬜

**可核验交付物**：
- `ItemTemplateV1.stack_limit: u32`（schema 新增字段，默认 1 = 不可堆叠，`#[serde(default = "default_stack_limit")]`）
- `server/src/inventory/mod.rs::add_item_to_player_inventory`：
  - 先查找：同 `item_template_id` + `placed_item.stack_count < stack_limit` 的已有实例
  - 找到 → 合并：`existing.stack_count += incoming.stack_count`，若超上限则溢出剩余到新 slot
  - 找不到 → 走 `find_free_slot` 新建
- `stack_count` 上限校验：`stack_count > stack_limit` → clamp + log warning
- 灵石（SpiritStone）、草药类物品的 JSON template 更新 `stack_limit`（按现有 item registry）
- 测试 `inventory::stacking::*`：
  - `stackable_items_merge`
  - `stack_overflow_creates_new_slot`
  - `non_stackable_always_new_slot`
  - `stack_count_clamped_to_limit`（4 单测）

## §3 开放问题

- [ ] 容器尺寸（`grid_rows × grid_cols`）目前是否存在 `Container` struct 中？若没有需要新增字段还是从 `ContainerKind` 推断？
- [ ] 背包满时掉落在玩家脚下还是静默拒绝？（需对齐 plan-inventory-v1 "世界实体化掉落延后"决策——目前 MVP 建议静默拒绝 + 客户端提示"背包已满"）
- [ ] 装备槽（equipment slots）是否也需要 grid placement，还是固定槽位不适用本算法？
