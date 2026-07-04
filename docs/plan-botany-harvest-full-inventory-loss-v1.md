# plan-botany-harvest-full-inventory-loss-v1

> 一句话主题：修复 botany 收获在背包已满时的静默吞产出——满包时产出改为原子 fallback 到 `DroppedLootRegistry`（地面可拾取），`plant.harvested` 的不可逆副作用延后到入包/掉地成功之后才发生，`tick_harvest_sessions` 不再 `let _` 吞错误，玩家在 event_stream 收到可见提示。

> **玩家影响（骨架保留）**：玩家在田边或野外把背包塞满后去收获，植物会被标记为已收获并从场景消失，但产物因为入包失败被静默丢弃；表面上系统显示"收获完成"，实际没有拿到任何草药 / 种子 / 变种掉落。**本 plan 修复后**：满包时产物改为掉落在采集点地面（走既有 `DroppedLootRegistry` → client 通用同步，玩家可正常拾取），并在事件流 HUD 收到"背包已满，已放置于地面"的可见提示；真正的结构性错误（kind/inventory 缺失）不再提前把植物标记为已收获，玩家可以重新靠近收获。

## 阶段总览

| 阶段 | 主题 | 状态 |
|---|---|---|
| P0 | 核心修复：满包不丢产出（原子 grant-or-ground + 重排验证顺序 + 停止吞错误） | ⬜ |
| P1 | 玩家可感知反馈：event_stream 满包提示 | ⬜ |
| P2 | 回归饱和测试：满包/非满包/结构性失败/自动+手动全覆盖 | ⬜ |

验收日期：全部 P ✅ 后填 `YYYY-MM-DD`（当前升 active：2026-07-04）。

## 接入面（docs/CLAUDE §二 六要素）

- **进料**：`botany::harvest::complete_harvest_for_player` 从 `BotanyKindRegistry` 取 `kind.item_id` / `growth_cost`，从 `ItemRegistry` 取物品模板，从 `HarvestSession`（含 `origin_position`）取采集上下文；新增进料 `Option<ResMut<inventory::DroppedLootRegistry>>`（满包 fallback 落点）。
- **出料**：正常路径产物仍进 `PlayerInventory`（`inventory::add_item_to_player_inventory` / `add_customized_item_to_player_inventory`）；新增满包出料路径 —— 产物进 `inventory::DroppedLootRegistry.entries`（复用既有 `network::dropped_loot_sync_emit::emit_changed_dropped_loot_syncs` 通用广播，**不需要新增 client payload**，client 已有通用拾取渲染）；玩家可感知反馈出料到 `schema::combat_hud::EventStreamPushV1`（复用 `network::event_stream_emit` 既有 event_stream 管线，channel 从 `Combat` 泛化为参数化，新增 `World` 用例）。
- **共享类型 / event**：复用 `inventory::DroppedLootRegistry` / `DroppedLootEntry`（不新建注册表，与 `fauna::drop::fauna_drop_system` 同款直插模式）；复用 `botany::components::HarvestTerminalEvent`（加一个 `overflow_to_ground: bool` 字段，不新建 event 类型）；复用 `schema::combat_hud::EventStreamPushV1` / `EventChannelV1::World`（不新建 schema）。
- **跨仓库契约**：**无新增 wire payload / IPC schema symbol**。`DroppedLootSync`（`schema::server_data::ServerDataPayloadV1::DroppedLootSync`）和 `EventStreamPush`（`ServerDataPayloadV1::EventStreamPush`）均为已跨端落地的既有 schema，client 已有通用消费方（拾取渲染 / 事件流 HUD 条目），本 plan 只是让 server 侧多产出符合既有 schema 的数据，client 无需改动。agent 不涉及（纯 server 内部 gameplay 修复）。
- **worldview 锚点**：延续 `plan-botany-v1` 已锚定的采集玩法（灵草采集 / `growth_cost` 灵气流转），不新增境界 / 经济 / 传承概念。本 plan 是既有玩法的正确性修复（"收获完成"不应该等于"产物凭空消失"），无需新增 worldview 章节。
- **qi_physics 锚点**：不涉及。`kind.growth_cost` / `restore_ratio` 是 `botany/registry.rs` 既有静态字段（如 `ci_she_hao: growth_cost=0.002, restore_ratio=0.8`），本 plan 不改动其数值、不改动 `botany/lifecycle.rs` 的 `restore_ops` 灵气归还计算，只重排 `plant.harvested` 置位的**时机**（同一 tick 内，仍在 `complete_harvest_for_player` 单次调用内完成，不引入跨 tick 的真元残留窗口）。无新增衰减 / 释放路径，不触碰 `qi_physics::ledger`。

---

## P0 — 核心修复：满包不丢产出

### 交付物

1. **`server/src/inventory/mod.rs`**（紧邻 `add_customized_item_to_player_inventory`，约 L1515-1534 之后新增）：
   - 新增 `pub enum GrantOrGroundOutcome { Granted(InventoryGrantReceipt), DroppedToGround(DroppedLootEntry) }`
   - 新增 `pub fn add_item_to_player_inventory_or_ground(inventory: &mut PlayerInventory, registry: &ItemRegistry, allocator: &mut InventoryInstanceIdAllocator, dropped_loot: Option<&mut DroppedLootRegistry>, template_id: &str, stack_count: u32, current_tick: u64, ground_pos: [f64; 3], ground_dimension: DimensionKind, customize_instance: Option<&dyn Fn(&mut ItemInstance)>) -> Result<GrantOrGroundOutcome, String>`：
     - 先调用既有私有 `add_item_to_player_inventory_inner(...)`（L1560，签名不变，**不改动**其 20+ 现有调用方的 Err 契约）。
     - `Ok(receipt)` → `Ok(Granted(receipt))`。
     - `Err(err) if err.starts_with("inventory full:")`（既有错误前缀，见 L1641 / L1738）→ 用同一 `registry.get(template_id)` + 私有 `runtime_instance_from_template(template, instance_id, stack_count, current_tick)`（L1818，**同文件内可见，无需改可见性**）构造 `ItemInstance`，套用 `customize_instance`，`allocator.next_id()?` 分配 id，包成 `DroppedLootEntry { source_container_id: format!("overflow:{template_id}"), world_pos: ground_pos, dimension: ground_dimension, .. }` 插入 `dropped_loot.entries`（`dropped_loot` 为 `None` 时返回 `Err("inventory full and no DroppedLootRegistry available to fall back: {err}")`——保留可观测失败而非 panic，属于系统装配缺陷不是正常玩法路径）。
     - 其他 `Err`（`unknown item template id` / `stack_count 0` / `no containers`）原样透传——这些是结构性配置错误，不该被静默转成"地面掉落"掩盖。
   - **拒绝的替代方案**：收获前 pre-check 容器是否有空位（`find_free_slot` / `container_accepts_runtime_grant` 独立跑一遍）——判定逻辑已内嵌在 `add_item_to_player_inventory_inner` 内部，pre-check 需要复刻一份同款逻辑，双实现容易漂移；改为"原子尝试 → 失败 fallback"保证唯一实现口径（即骨架 §8 决议 #1，见 `## §8.1 决议`）。

2. **`server/src/botany/harvest.rs`**：
   - 头部 `use crate::inventory::{...}`（L11-15）新增 `add_item_to_player_inventory_or_ground, GrantOrGroundOutcome, DroppedLootRegistry, DroppedLootEntry`；新增 `use crate::world::dimension::DimensionKind;`（沿用 botany 模块既有 `DimensionKind::Overworld` 惯例，见 `hazard.rs:250` / `ecology.rs:135` 等 12+ 处）。
   - `complete_harvest_for_player`（L87-291）签名新增末尾参数 `dropped_loot: Option<&mut DroppedLootRegistry>`。
   - **重排验证顺序**（当前 L105-194，逻辑顺序改为）：
     1. `store.remove_session(player_id)?`（不变，L105-107）
     2. **先** `kind_registry.get(session.target_plant)?`（原 L127-129，挪到最前）
     3. **再** `inventory_query.get_mut(session.client_entity)?`（原 L131-138，紧随其后）
     4. **然后**才执行原 L109-125 的 plant 查找与不可逆副作用块（`target_pos` / `target_zone_name` / `variant` 读取、`static_points` 解绑、`plant.harvested = true`）——两个 `?` 校验失败时这段代码不会执行，`plant.harvested` 保持 `false`，植物可被重新收获。
     5. 其余逻辑（工具判定 L140-165、`harvest_spirit_quality` 计算 L167-174）不变，仍依赖第 4 步产出的 `variant`。
   - 原 L175-194 的 `add_customized_item_to_player_inventory(...)?` / `add_item_to_player_inventory(...)?` 二选一分支，替换为单次调用：
     ```rust
     let ground_pos = target_pos.unwrap_or(session.origin_position);
     let outcome = add_item_to_player_inventory_or_ground(
         &mut inventory, item_registry, allocator, dropped_loot,
         kind.item_id, 1, now_tick, ground_pos, DimensionKind::Overworld,
         has_instance_modifier.then_some(&|instance: &mut ItemInstance| {
             apply_harvest_modifiers_to_item(instance, variant, herbalism_quality_bonus)
         } as &dyn Fn(&mut ItemInstance)),
     )?;
     let overflow_to_ground = matches!(outcome, GrantOrGroundOutcome::DroppedToGround(_));
     ```
     （`?` 此时只会因为"无 DroppedLootRegistry 兜底"或结构性错误才失败——正常"满包"场景不再走 `Err`。）
   - `HarvestTerminalEvent { .. }` 构造（原 L274-289）：`detail` 分支——`overflow_to_ground` 为真时 `format!("采得 1 株 · 背包已满，已放置于地面 · 灵气流出 {:.3}", kind.growth_cost)`，否则维持原文案；新增字段 `overflow_to_ground` 透传。
   - `tick_harvest_sessions`（L489-535，产出物 dropped_loot 通过 `mut dropped_loot: Option<ResMut<DroppedLootRegistry>>` 系统参数注入，`.as_deref_mut()` 传给 `complete_harvest_for_player`）：原 L518 `let _ = complete_harvest_for_player(...)` 改为：
     ```rust
     if let Err(err) = complete_harvest_for_player(...) {
         tracing::warn!(
             "[bong][botany] harvest completion failed for `{player_id}`: {err} — \
              session cleared, plant left un-harvested for retry"
         );
     }
     ```
     （不再静默；会打日志。session 已在函数内部第一步被移除——这是"轻量回滚"：结构性失败时玩家只需要重新走近植物再次发起收获，不会因此丢失任何已产出的物品，因为这些错误发生在任何产出构造之前，见上面重排。）

3. **`server/src/botany/components.rs`**（`HarvestTerminalEvent` 定义，L251-266）：新增字段 `pub overflow_to_ground: bool`。

4. **6 处既有测试 fixture**（`server/src/botany/harvest.rs` 内 `mod tests`，`app.add_systems(Update, tick_harvest_sessions)` 调用点，约 L787/867/940/988/1049/1106 前后）：不强制要求全部 6 处都 `insert_resource(DroppedLootRegistry::default())`（`Option<ResMut<..>>` 系统参数在缺省资源时优雅退化——沿用 `fauna::drop::fauna_drop_system` 的既有 `Option<ResMut<DroppedLootRegistry>>` 惯例，见 `fauna/drop.rs:276`），已有 6 条测试全部走"非满包"路径，行为不变、无需改。

### 测试（新增，饱和覆盖）

- `server/src/inventory/mod.rs`：`add_item_to_player_inventory_or_ground` 单元测试——① 有空位 → `Granted`，revision 递增；② 满包 + 有 `DroppedLootRegistry` → `DroppedToGround`，`registry.entries` 新增 1 条，`world_pos == ground_pos`；③ 满包 + `dropped_loot=None` → `Err` 含 `"no DroppedLootRegistry"`；④ `unknown item template id` → 原样透传 `Err`，不落地面。
- `server/src/botany/harvest.rs` `mod tests`：
  - `harvest_completion_overflow_drops_to_ground_when_inventory_full`：1x1 背包塞满后收获 → `DroppedLootRegistry.entries.len() == 1`、`HarvestTerminalEvent.overflow_to_ground == true`、`detail` 含"背包已满"、`plant.harvested == true`（产物已保证落地，植物按既有语义正常判定已收获）。
  - `harvest_completion_non_full_inventory_grants_normally_no_overflow`：对照组，`overflow_to_ground == false`，`DroppedLootRegistry` 无新增条目（回归既有 6 条测试外的显式反例）。
  - `harvest_completion_missing_kind_registry_leaves_plant_unharvested`：`kind_registry.get` 失败路径 → `plant.harvested == false`（可重试），`tick_harvest_sessions` 中该分支走 `tracing::warn!` 而非 panic / 静默。
  - `harvest_completion_missing_player_inventory_leaves_plant_unharvested`：同上，覆盖 `inventory_query.get_mut` 失败分支。
  - `harvest_completion_variant_and_quality_modifiers_survive_overflow`：变种植物 + herbalism 品质加成的满包分支——`DroppedLootEntry.item.spirit_quality` / `display_name` 前缀与非满包路径的 `apply_harvest_modifiers_to_item` 结果一致（防止 fallback 路径漏掉 `customize_instance`）。
  - `harvest_completion_stack_boundary_max_stack_count_still_overflows_correctly`：堆叠已到 `max_stack_count` 上限（非空但也放不下新堆）触发满包分支，非"完全空背包"边界。

---

## P1 — 玩家可感知反馈：event_stream 满包提示

### 交付物

1. **`server/src/network/event_stream_emit.rs`**：
   - `push_to_client_priority`（现 L95-131）新增 `channel: EventChannelV1` 参数（原硬编码 `EventChannelV1::Combat`，L108，改为使用传入的 `channel`），可见性从私有改为 `pub(crate)`（供新系统跨函数复用）；`push_to_client`（L78-93）保持默认 `EventChannelV1::Combat` 行为，内部转发时显式传 `EventChannelV1::Combat`，**不改变现有战斗调用方行为**。
   - 新增 `pub fn emit_botany_harvest_overflow_to_event_stream(mut terminal: EventReader<HarvestTerminalEvent>, mut clients: Query<(&Username, &mut Client)>)`：遍历 `HarvestTerminalEvent`，仅当 `event.overflow_to_ground && event.completed && !event.interrupted` 时调用 `push_to_client_priority(&mut clients, event.client_entity, "botany-overflow", &event.detail, EventChannelV1::World, EventPriorityV1::P2Normal, now_ms)`——非满包的正常收获不推送（呼应 `feedback_hud_immersive_minimal` 记忆：事件流不为 happy path 刷屏）。
2. **`server/src/network/mod.rs`**：`event_stream_emit::emit_combat_events_to_event_stream` 所在的 20 元素 `add_systems` 元组（L895-932）已达 Bevy 0.14.2 `IntoSystemConfigs` tuple impl 上限（1..=20），**不得**向该元组追加第 21 个系统——仿照紧邻其后的 `// plan-shield-block-v1 P3：盾牌破损推送（独立 add_systems 避免 Bevy 20元素 tuple 上限）`（L934）先例，新增一段独立调用：`app.add_systems(Update, event_stream_emit::emit_botany_harvest_overflow_to_event_stream.after(crate::botany::harvest::tick_harvest_sessions));`，紧跟在该注释所在独立 `add_systems` 调用之后。

### 测试

- `event_stream_emit` 新增测试：① `overflow_to_ground=true` 的 `HarvestTerminalEvent` → client 收到 1 条 `EventStreamPushV1{ channel: World, priority: P2Normal, text 含 "背包已满" }`；② `overflow_to_ground=false` → 无推送（0 条）；③ `interrupted=true` 即使误设 `overflow_to_ground=true` 也不推送（防御性用例，锁住"打断"分支恒不触发地面提示的契约）。
- `push_to_client_priority` 泛化后的既有战斗调用方（`emit_combat_events_to_event_stream`）保持原有测试全绿（回归，不新增）。

---

## P2 — 回归饱和测试：满包 / 非满包 / 结构性失败 / 自动 + 手动全覆盖

### 交付物

- 补齐 P0/P1 未覆盖的组合矩阵（均落在 `server/src/botany/harvest.rs` `mod tests`）：
  - `BotanyHarvestMode::Auto` 与 `BotanyHarvestMode::Manual` 各自触发满包 fallback 一条（当前 P0 用例默认走 Manual，需要显式补 Auto 对照）。
  - 满包失败后**再次**收获同一植物（若 `plant.harvested` 已置 true，符合既有"已收获"生命周期语义，不可重复收获——锁定"满包但成功=植物已消耗"而非"结构性失败=植物待重试"两者不能混淆的边界）。
  - `DroppedLootRegistry` 满包掉落条目经 `inventory::dropped_loot_snapshot(&registry)` 可枚举（复用 `dropped_loot_sync_emit.rs` 已用的同一快照函数），锁定"地面产物真的能被既有同步管线看见"的契约，而不是只锁内部 `entries` HashMap。
  - `server/src/network/dropped_loot_sync_emit.rs` 不新增专属测试（既有 diff-based 广播已是通用实现，见模块头注释），P2 只需在上面这条快照可见性测试里断言，不重复造轮子。

### 验收标准

- `cargo test -p bong-server botany::harvest` 与 `cargo test -p bong-server inventory::` 全绿（若无独立 crate 名，退化为 `cd server && cargo test botany::harvest -- --nocapture` + `cargo test add_item_to_player_inventory_or_ground`）。
- `cargo clippy --all-targets -- -D warnings` 通过（新增 `#[allow(clippy::too_many_arguments)]` 视实际参数数量需要添加，比照 `complete_harvest_for_player` 现有 `#[allow(clippy::too_many_arguments)]` 惯例）。
- 手动回归（可选，非阻塞）：`/clearinv all` 后 `/give` 塞满背包 → 采集任意灵草 → 观察 chat/event_stream 收到满包提示 + 地面出现可拾取掉落。

---

## §8 开放问题（P0 决策门前需收口）

> 本节为骨架阶段遗留的开放问题，已在下方 `## §8.1 决议` 收口，原表保留以备追溯，**实施时以 §8.1 决议为准**。

1. 满包时收获产物的去向：掉落地面（保产出）vs 阻止/回滚收获（方案 A/B）？
2. `tick_harvest_sessions` 对 `complete_harvest_for_player` 返回的 `Err` 该如何处理——继续 `let _` 吞掉，还是需要失败回滚 / 地面兜底？以及 `plant.harvested` 提前置位与后续校验失败之间的时序矛盾如何解？
3. "失败提示必须玩家 UI 可见"要求下，用哪个既有 channel 承载反馈（chat 文本 vs 结构化 event_stream）？

## §8.1 决议（pre-P0 收口，2026-07-04）

### #1 满包时收获产物的去向

**决议**：
1. 选方案 A（掉落地面，保产出）。理由：`docs/CLAUDE.md` §Testing 与 `docs/CLAUDE.md`（根）都强调"收获完成"是玩家可观察的语义承诺，一旦植物已从场景消失（`plant.harvested=true` 触发 `botany/lifecycle.rs` 的 wither 回收分支），阻止/回滚收获意味着要在已经不可逆的场景状态变化之后再"撤销"——比直接把产物放到地面复杂且更容易出新 bug（比如要把 static_point 解绑、mob_attraction 事件都撤销）。方案 A 只需要在"产物往哪放"这一步二选一，不需要撤销已经发生的场景副作用。
2. 实施：新增 `inventory::add_item_to_player_inventory_or_ground`（原子尝试入包，失败走 `DroppedLootRegistry` fallback），复用 `fauna::drop::fauna_drop_system` 已验证过的"直接 `registry.entries.insert(...)`"模式（同一 `DroppedLootEntry` 结构，同一广播管线），不新建掉落系统。
3. 拒绝方案 B（阻止收获）：需要在"是否会满包"这一步做 pre-check，而 pre-check 的空位判定逻辑（`find_free_slot` / `container_accepts_runtime_grant`）已经内嵌在 `add_item_to_player_inventory_inner` 内部，重新实现一份等价判定必然与真正的插入逻辑产生"两处判定各自维护、迟早漂移"的技术债，且不符合 `docs/CLAUDE.md`"不写兼容层/要干净代码"的约束。

**落点**：`server/src/inventory/mod.rs:1515-1534`（紧邻新增 `add_item_to_player_inventory_or_ground` + `GrantOrGroundOutcome`）/ `server/src/fauna/drop.rs:319-332`（参照的直插模式）/ 本 plan `## P0` 交付物 1-2。

### #2 `tick_harvest_sessions` 吞错误 + `plant.harvested` 时序矛盾

**决议**：
1. 核心结论：`let _ = complete_harvest_for_player(...)` 必须改成 `if let Err(err) = ... { tracing::warn!(...) }`——错误不能无痕迹。但仅仅加日志不够，因为当前 `plant.harvested = true`（`server/src/botany/harvest.rs:123`，重排前行号）发生在 `kind_registry.get`（原 L127-129）和 `inventory_query.get_mut`（原 L131-138）这两个可能失败的校验**之前**，意味着这两个结构性错误分支会在"植物已经标记为收获"之后才 `Err`，届时植物已注定在下一次 `botany::lifecycle` tick 被当作已收获回收（`server/src/botany/lifecycle.rs:386,397-410`：`wither_due_harvest = plant.harvested` 为真直接进 `wither_targets`），而玩家什么都没拿到——这正是骨架原本诊断出的 bug 本体，不能只加日志掩盖。
2. 实施方案：把 `kind_registry.get(...)?` 和 `inventory_query.get_mut(...)?` 两个校验挪到 `plant.harvested = true` 那段副作用代码**之前**执行（"先验证、后动作"的标准原子性顺序）。这样：
   - 结构性错误（kind/inventory 缺失）→ 函数在设置 `plant.harvested` 之前就返回 `Err`，植物保持 `harvested=false`，玩家可以重新走近再次发起收获——这就是"失败回滚"，不需要额外撤销逻辑，因为根本没发生副作用。
   - "满包"不再是这个函数的 `Err` 来源（见决议 #1，被 `add_item_to_player_inventory_or_ground` 内部吸收），所以 `plant.harvested=true` 之后唯一还可能报错的路径消失了，`tick_harvest_sessions` 的 `Err` 分支从"常见的满包"退化为"罕见的系统装配缺陷"（比如两个系统间实体生命周期竞态），这种 `tracing::warn!` 恰如其分——高频路径不报错，低频路径可观测但不阻断游戏循环。
3. 边界条件：`store.remove_session(player_id)` 仍然在最开始就执行、不随结构性错误回滚——session 是"这一次点击"的瞬态状态，清掉只代表玩家需要重新触发一次收获动作（成本是重新走近植物 1-2 秒），不是数据丢失；真正会丢失的是"产物"和"植物"，这两者现在都被保护。

**落点**：`server/src/botany/harvest.rs:105-194`（`complete_harvest_for_player` 重排验证顺序，见 `## P0` 交付物 2）/ `server/src/botany/harvest.rs:517-535`（`tick_harvest_sessions` 的 `let _` 改 `if let Err`）/ `server/src/botany/lifecycle.rs:386,397-410`（`wither_due_harvest` 回收分支，本决议要保护的下游）。

### #3 玩家可感知反馈用哪个 channel

**决议**：
1. 核心结论：用结构化 `event_stream`（`schema::combat_hud::EventStreamPushV1`），不用裸聊天文本（`Client::send_chat_message`，`server/src/coffin/mod.rs:1045-1047` 的既有先例）。
2. 实施：`HarvestTerminalEvent` 新增 `overflow_to_ground: bool` 字段（见 `## P0` 交付物 3），新增 `network::event_stream_emit::emit_botany_harvest_overflow_to_event_stream` 系统只在 `overflow_to_ground=true` 时推送，`channel=EventChannelV1::World`、`priority=EventPriorityV1::P2Normal`（见 `## P1`）。
3. 拒绝裸聊天文本方案：`coffin/mod.rs:1012` 的注释已经明确写"真·地面掉落待 P4 或 item-entity 机制就位（`DroppedLootRegistry` 路径）后升级"——即 coffin 模块当年选聊天文本是因为 `DroppedLootRegistry` 尚未验证适合这个场景；现在 botany 这边恰恰是把"待升级"的目标（真正走 `DroppedLootRegistry`）落地的场景，理应同步把反馈也升级到结构化 `event_stream`（`docs/CLAUDE.md` 记忆 `feedback_hud_immersive_minimal`：事件流是通用非战斗专用的既有 HUD 承载面，不需要再造一个聊天刷屏）。`event_stream` payload 结构化（`channel`/`priority`/`text`），client 侧已有通用消费方，不需要新增 wire schema。

**落点**：`server/src/botany/components.rs:251-266`（`HarvestTerminalEvent` 加字段）/ `server/src/network/event_stream_emit.rs:95-131`（`push_to_client_priority` 泛化 channel 参数）/ `server/src/network/mod.rs:934` 附近（新系统注册——**不追加进 L895-932 已满 20 元素的 add_systems 元组**，仿 `plan-shield-block-v1` P3 先例另起独立 `app.add_systems`，见 `## P1` 交付物 2）/ 本 plan `## P1`。

---

## 反方裁决摘要（骨架阶段，保留）

- 证伪 round 1：`plan-botany-v1` 只定义"drop 走背包"，没有授权"满包直接吞掉产出"；`plan-lingtian-v1` 反而明确写了满包 `warn`，说明本仓对同类问题通常不会默认静默吞。
- 证伪 round 2：`botany/harvest.rs` 没找到任何失败补偿、地面掉落或回填 session 的旁路，`tick_harvest_sessions()` 还把错误吞掉；因此这个候选 survive，且玩家正常游玩可达。
