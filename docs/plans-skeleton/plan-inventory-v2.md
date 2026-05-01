# Bong · plan-inventory-v2 · 骨架

Inventory 两项核心修复：Tarkov 式格子空间分配（替换全部 `row:0,col:0` 硬编码）+ 同种物品堆叠合并（stack merge）。对应 `plan-inventory-v1`（active）已知缺口条目。

**世界观锚点**：无直接世界观对应——纯背包系统技术修复，不影响玩法语义。

**交叉引用**：
- `plan-inventory-v1`（active）— 已落 `ContainerState` / `PlacedItemState` / `add_item_to_player_inventory`；本 plan 修复该函数 + 补 ItemTemplate 的 max_stack_count 字段
- `plan-botany-v2`（active）/ `plan-mineral-v1`（active）— 采草药/矿物批量入包时会同时调用多次 `add_item_to_player_inventory`，是 row-col 冲撞的主要触发场景

---

## 接入面 Checklist

- **进料**：`add_item_to_player_inventory(inventory, registry, allocator, template_id, stack_count)` 调用链（`server/src/inventory/mod.rs:822`）
- **出料**：`PlacedItemState { row, col, instance }` 正确分配不重叠格子 + stack 合并后单实例 stack_count 增大
- **共享类型**：扩展 `ItemTemplate`（新增 `max_stack_count: u32` 字段）；`placed_item_footprints_overlap` 辅助函数已存在（`mod.rs:2942`）可复用
- **跨仓库契约**：
  - server：`server/src/inventory/mod.rs`（修改 `add_item_to_player_inventory` + 新增 `find_free_slot` / `find_mergeable_stack`）
  - client：`PlacedItemState` 数据格式不变，渲染层不需改动
  - agent/schema：无变更
- **worldview 锚点**：无

---

## §0 设计轴心

- [ ] **格子分配算法简单优先**——行优先扫描（top-left first），找到第一个能放下 grid_w×grid_h 的空位就用，不做最优化排列
- [ ] **堆叠合并优先于新格子**——先尝试合并到现有同 template_id 栈，合并满了再开新格子
- [ ] **满包时明确返回错误**——找不到空位时 `Err("inventory full")` 而非静默放在 (0,0)
- [ ] **ItemTemplate.max_stack_count 为新字段**——默认值 1（不可堆叠），TOML 配置可覆写；草药类默认 64

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | `find_free_slot(container, grid_w, grid_h) -> Option<(u8, u8)>` + `add_item_to_player_inventory` 接入 | 单元：5×7 格子各种物品填满后 None；多物品不重叠断言 |
| **P1** ⬜ | `ItemTemplate.max_stack_count` 字段 + TOML 解析 + `find_mergeable_stack` | 单元：同 template_id 合并 stack_count；不超 max_stack；多余开新格子 |
| **P2** ⬜ | 批量入包场景修复（`botany` 采草药 / `mineral` 采矿 / `harvest drop`） | 集成：同时 grant 5 株草药 → 5 个不重叠格子 or 合并到现有栈 |

---

## §2 核心算法

### §2.1 find_free_slot

```rust
/// 行优先扫描，返回能容纳 (grid_w × grid_h) 的左上角 (row, col)。
/// 复用已有 placed_item_footprints_overlap（mod.rs:2942）做碰撞检测。
pub fn find_free_slot(
    container: &ContainerState,
    grid_w: u8,
    grid_h: u8,
) -> Option<(u8, u8)> {
    for r in 0..container.rows.saturating_sub(grid_h - 1) {
        for c in 0..container.cols.saturating_sub(grid_w - 1) {
            let candidate = PlacedItemState {
                row: r, col: c,
                instance: /* dummy sentinel */ ...,
            };
            if !container.items.iter().any(|existing|
                placed_item_footprints_overlap(&candidate, existing)
            ) {
                return Some((r, c));
            }
        }
    }
    None
}
```

**测试矩阵**：
- 空容器 → (0, 0)
- 左上被占 → 跳到下一个空位
- 容器全满 → None
- 物品 grid_w=2, grid_h=2 的边界不超出容器
- 格子碎片化（中间有空洞）→ 正确找到空洞

### §2.2 find_mergeable_stack

```rust
/// 在 container 中找第一个 template_id 相同且 stack_count < max_stack 的实例。
pub fn find_mergeable_stack<'a>(
    container: &'a mut ContainerState,
    template_id: &str,
    max_stack: u32,
) -> Option<&'a mut PlacedItemState> {
    container.items.iter_mut().find(|p| {
        p.instance.template_id == template_id
            && p.instance.stack_count < max_stack
    })
}
```

**合并逻辑**（`add_item_to_player_inventory` 内）：
```
1. 取 template.max_stack_count
2. 若 max_stack_count > 1：
   a. find_mergeable_stack → 若找到：merge min(available_space, stack_count)
   b. 若 stack_count 还有剩余：继续到步骤 3
3. find_free_slot → 放新实例
4. 若找不到空位 → Err("inventory full: {template_id}")
```

---

## §3 ItemTemplate 扩展

```toml
# server/assets/items/herbs.toml 示例
[[item]]
id = "qingxin_grass"
# ...
max_stack_count = 64   # 新增字段，默认 1（不可堆叠）
```

```rust
// server/src/inventory/mod.rs — ItemTemplateToml
pub struct ItemTemplateToml {
    // 现有字段...
    #[serde(default = "default_max_stack")]
    pub max_stack_count: u32,  // 默认 1
}

fn default_max_stack() -> u32 { 1 }
```

**需要更新 max_stack_count 的物品类型**（初始批次）：
| 类别 | max_stack_count |
|------|----------------|
| 草药 (Herb) | 64 |
| 矿物/原石 (Mineral) | 32 |
| 骨币 (BoneCoin) | 无上限（u32::MAX） |
| 丹药 (Pill) | 16 |
| 武器/法器/装备 | 1（不可堆叠） |
| 其他 Misc | 16 |

---

## §4 开放问题

- [ ] 容器满时的 UX 反馈：当前采草药失败只返回 Err 字符串，client 未显示明确提示；P2 需协商 client IPC
- [ ] 格子分配效率：行优先 O(rows × cols × items) 对大背包可能较慢；当前默认 5×7 主包共 35 格，实际最多几十个物品，暂不优化
- [ ] 混合容器（前挂包/小口袋）：`add_item_to_player_inventory` 目前只放 `MAIN_PACK_CONTAINER_ID`；是否要在主包满时自动溢出到其他容器？（当前不做，留后续扩展）

---

## §5 进度日志

- 2026-05-01：从 plan-inventory-v1 reminder 整理立项。现有代码：`add_item_to_player_inventory` 一律 `row:0, col:0`（`server/src/inventory/mod.rs:869`）；`placed_item_footprints_overlap` 已存在（`mod.rs:2942`）；`ItemTemplate` 无 `max_stack_count` 字段；堆叠合并逻辑未实装。
