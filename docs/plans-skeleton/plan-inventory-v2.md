# Bong · plan-inventory-v2 · 骨架

**背包系统 v2**：解决 v1 遗留的两个关键缺口——新增物品自动网格寻位（避免 row-col 冲突）和 stacking 合并（同类可堆叠物品自动叠加并校验上限）。同时收录 v1 明确延后的世界实体化掉落和持久化方向。

**世界观锚点**：`worldview.md §686`（死亡 50% 掉落规则）· `worldview.md §486`（灵气流失税，延后到 v3+）

**交叉引用**：
- `plan-inventory-v1`（前置，P1–P5 ✅ Finish Evidence 已落）
- `plan-botany-v1`（`award_item_to_inventory` 调用方）
- `plan-lingtian-v1`（灵田 harvest 入包调用 `add_item_to_player_inventory`）
- `plan-fauna-v1`（击杀掉落入包）
- `plan-alchemy-v1`（成丹入包）

**阶段总览**：
- P0 ⬜ grid 自动寻位 + stacking 合并
- P1 ⬜ 客户端 stackCount 角标兼容确认 + award_item 调用方更新
- P2 ⬜（延后）世界实体化掉落 / 持久化 / 灵气流失税

---

## §0 设计轴心

- [ ] `add_item_to_player_inventory` 当前直接 `push({ row:0, col:0 })`，多株同时入包时 row-col 冲撞，客户端渲染堆叠异常——**自动寻位**是 P0 核心
- [ ] **stacking**：stackable 物品入包时先与已有同 template_id 条目合并 stack_count，超 max_stack 才新建格子——**合并入包**是 P0 另一核心
- [ ] **不改协议**：v1 的 `InventorySnapshotV1` / `InventoryEventV1` 字段保持不动，仅修改服务端 placement 逻辑；客户端 `InventoryEvent::StackChanged` 变体 v1 已预留，直接复用

---

## §1 Grid 自动寻位（P0）

**现状**：`server/src/inventory/mod.rs` `add_item_to_player_inventory`（行 820–881）直接 push `PlacedItem { row: 0, col: 0, ... }`。

已有基础：`placed_item_footprints_overlap`（行 2768–2783）实装了 AABB 碰撞检测（`row`/`col` + `grid_w`/`grid_h` 矩形重叠判定），但 `add_item_to_player_inventory` 从未调用它。

**实装要求**：

```rust
// server/src/inventory/mod.rs
fn find_placement_in_container(
    container: &Container,
    w: u8,
    h: u8,
) -> Option<(u8, u8)> {
    for row in 0..(container.rows.saturating_sub(h - 1)) {
        for col in 0..(container.cols.saturating_sub(w - 1)) {
            let candidate = PlacedItem { row, col, instance: ItemInstance::placeholder() };
            if !container.items.iter().any(|p| placed_item_footprints_overlap(&candidate, w, h, p)) {
                return Some((row, col));
            }
        }
    }
    None
}
```

- `add_item_to_player_inventory` 改为调用 `find_placement_in_container`，找不到位置时返回 `Err(InventoryError::ContainerFull)`
- 受影响的调用方（均需处理 `ContainerFull` 分支，当前以 warn + 丢弃为最小兜底）：
  - `award_item_to_inventory`（lingtian 收获）
  - `apply_death_drop_to_inventory`（死亡掉落）
  - `apply_tsy_death_drop`（tsy 死亡分流）

**可核验交付物**：
- `server/src/inventory/mod.rs` 新函数 `find_placement_in_container`
- `inventory::grid_placement::*` 单测（至少 8 条）：
  - empty 容器放第一个 1×1 → `(0, 0)`
  - 连续放 5 个 1×1 到 3×2 容器，各自不 overlap
  - 2×1 物品放满一行后换下一行
  - 2×2 物品在 3×3 容器找正确 `(row, col)`
  - 容器已满（所有位置都 overlap）→ 返回 `None`
  - `ContainerFull` 时 `add_item_to_player_inventory` 返回 `Err`
  - 非方形 grid（5×7 主背包）寻位正确

---

## §2 Stacking 合并（P0）

**现状**：`add_item_to_player_inventory` 每次直接新建 `ItemInstance`，不查找已有同类合并；`ItemTemplate` 有 `stack_count` 字段但无 `max_stack` 上限。

**实装要求**：

```rust
// server/src/inventory/mod.rs – ItemTemplate 新增字段
pub struct ItemTemplate {
    // ... 已有字段 ...
    pub max_stack: u32,   // 默认 1 = non-stackable
}
```

- `add_item_to_player_inventory` 若 `template.max_stack > 1`：
  1. 扫 container 内所有 `template_id` 相同且 `stack_count < max_stack` 的 `PlacedItem`，优先叠加
  2. 叠满后溢出部分另起新 `PlacedItem`（走 `find_placement_in_container`）
  3. 完全塞不下时返回 `Err(ContainerFull { remaining: u32 })`
- `server/assets/items/*.toml` 补 `max_stack` 字段：
  - 草药 / 种子（`ci_she_hao`、`ning_mai_cao_seed` 等）：`max_stack = 32`
  - 骨币（`bone_coin`）：`max_stack = 64`
  - 法器 / 武器 / 丹药（`guyuan_pill` 等）：`max_stack = 1`（默认，可不写）

**可核验交付物**：
- `server/assets/items/*.toml` stackable 草药 / 种子条目加 `max_stack`
- `inventory::stacking::*` 单测（至少 8 条）：
  - stackable herb ×1 入有同类格（余量 10）→ 叠加，不新建 PlacedItem
  - 叠到 `max_stack` 后溢出 → 新 PlacedItem
  - non-stackable（`max_stack = 1`）连续入包 3 次 → 各占一格
  - 混合 `template_id` 不合并
  - 量超过一格 max_stack 时溢出部分找新位置
  - `ContainerFull` 含 `remaining` 字段正确返回
  - 每次 stack 变化发 `InventoryEvent::StackChanged` delta

---

## §3 延后项（来自 plan-inventory-v1 Finish Evidence §3）

- **真正世界实体化掉落**：server Item entity + 物理位置 + 多人争抢 + 自动 proximity pickup（当前 client billboard 仅视觉 surrogate）
- **持久化**：inventory 状态存盘（DB / 文件），offline 重启不丢失
- **灵气流失税**（worldview §486）：物品灵气随时间衰减，逼迫流转
- **多人交易 / 箱子 / 骨币死信箱**（worldview §526/§528）

---

## §4 数据契约

| 契约 | 位置 |
|---|---|
| `find_placement_in_container(container, w, h) -> Option<(u8, u8)>` | `server/src/inventory/mod.rs`（新函数）|
| `ItemTemplate.max_stack: u32` | `server/src/inventory/mod.rs` struct |
| `max_stack` TOML 字段 | `server/assets/items/herbs.toml` / `seeds.toml` / `pills.toml` 等 |
| `InventoryError::ContainerFull { remaining: u32 }` | `server/src/inventory/mod.rs`（新变体或已有 error 扩展）|
| `InventoryEvent::StackChanged` | v1 已预留，直接复用 |

---

## §5 实施节点

- [ ] **P0**：`find_placement_in_container` + `max_stack` TOML 字段 + stacking 合并 + `ContainerFull` 错误处理 + 所有调用方兜底 + 单测（grid × 7 + stacking × 8）
- [ ] **P1**：确认客户端 `stackCount` 角标（v1 §7.3 已实装），核实 `InventoryEvent::StackChanged` 解析无误；`award_item_to_inventory` warn 日志更新为有意义提示
- [ ] **P2**（延后）：世界实体化掉落 / 持久化 / 灵气税

---

## §6 开放问题

- [ ] `award_item_to_inventory` 当 `ContainerFull` 时：warn + 丢弃 vs 自动转为地面 dropped loot？（丢弃更简单；地面实体依赖 §3 世界实体化掉落完成后才合理）
- [ ] 死亡 50% 掉落时若背包满（`apply_death_drop_to_inventory` ContainerFull）：整批当"遗物"留在地面，还是截断入包？
- [ ] `max_stack` 应放在 TOML 显式声明，还是用 `ItemCategory` 隐式决定（`Herb`/`Seed` 默认 32，`Weapon` 默认 1）？
- [ ] stacking 合并时是否需要对应 `InventoryEvent::StackChanged` delta 推送到 client？（v1 已有此变体，实现时直接发）

---

## §7 进度日志

- 2026-04-28：骨架立项。来源：`docs/plans-skeleton/reminder.md` plan-inventory-v1 节（grid placement + stacking 两条缺口）。代码核查确认：`placed_item_footprints_overlap` 已实装（mod.rs 2768–2783），但 `add_item_to_player_inventory`（820–881）未调用，仍直接 push `{row:0,col:0}`；stacking / `max_stack` 完全未实装。
