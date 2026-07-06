# plan-bughunt-rotate-footprint-sync-v1（骨架）

> **骨架（草案）**。一句话主题：`inventory_move_intent.rotated=true` 已在 server 端把 2x1 / 1x2 footprint 写入真实库存，但普通成功路径只向客户端广播 `moved` 事件，事件不携带 item view，导致 Fabric 客户端仍保留旧宽高，出现 server/client 背包占格分叉。

> 立项动机：PR #957 合并前的 0 上下文 validator 抓到 S2C 同步缺口。该问题不影响 server 端库存测试通过，但会让玩家 UI 在旋转落位后显示旧 footprint；后续拖拽、碰撞预览、落位合法性都可能基于脏模型继续操作。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | rotated move 成功后客户端未收到新 footprint | fix_pr | ⬜ |

## P0 — rotated move 成功后客户端未收到新 footprint

- **#1 major（fix_pr）**：普通 `InventoryMoveOutcome::Moved` 分支没有把旋转后的 item view 同步给客户端。
  - `server/src/inventory/mod.rs` 的 `apply_inventory_move(... rotated=true ...)` 会在目标是网格容器时先互换 `grid_w/grid_h`，再把 item attach 到目标格；现有 server e2e 已断言 `rotated:true` 后 2x1 会落成 1x2。
  - `server/src/network/client_request_handler.rs` 的普通成功路径匹配 `InventoryMoveOutcome::Moved { revision }` 后只调用 `send_moved_event(entity, clients, instance_id, from, to, revision.0)`。
  - `server/src/schema/inventory.rs` 里 `InventoryEventV1::Moved` 只有 `revision / instance_id / from / to`，没有 `InventoryItemViewV1`，也没有 `grid_w/grid_h`、`rotated` 或替换 item 的信息。
  - `client/src/main/java/com/bong/client/network/InventoryEventHandler.java` 的 moved handler 只能从当前 snapshot 找旧 `InventoryItem` 并移动它；旧 item 的 `gridW/gridH` 会原样保留。
  - 结果：server 已经把物品从 2x1 改成 1x2，client UI 仍认为它是 2x1。下一次拖拽、hover 占格、冲突检测和 server 回推事件会以不同模型运行。

## 玩家可见影响

- 背包 UI 中按 R 旋转后，物品可能视觉上仍占旧形状，玩家无法信任服务端实际落位结果。
- 后续移动同一物品时，客户端会用旧 footprint 计算预览与目标格，容易出现“本地看能放、server 拒绝”或“本地看冲突、server 实际允许”的反直觉体验。
- 如果同容器内还有其他物品，旧 footprint 可能遮挡错误格子，让 UI 显示与服务端权威库存长期分叉，直到下一次完整 snapshot 才恢复。

## 建议修法

- 方案 A：扩展 moved 事件，让 rotated move 成功路径携带完整 `InventoryItemViewV1`，client 收到后替换本地 item 再移动。
- 方案 B：新增 `item_replaced` / `item_moved_with_view` 事件，普通未旋转 move 保持轻量事件，footprint/stack/durability 等结构变化走带 item view 的事件。
- 方案 C：在 rotated plain move 成功后主动补发该玩家 `inventory_snapshot`。实现最小，但比事件级同步更重；需确保不会和乐观 UI 回放产生闪烁。
- 无论采用哪种方案，swap 分支和非网格目标 `rotated=true` no-op 分支都要明确同步口径，避免只修普通 moved。

## 测试抓手

- server schema / protocol：新增样例或测试，钉住 rotated 成功路径会产生足够让客户端得知新 footprint 的 S2C 数据。
- client 单测：构造旧 snapshot 中 2x1 item，模拟 rotated move 成功事件后，本地 `InventoryModel` 必须变为目标格 1x2，而不是只移动旧 2x1。
- cross-layer e2e：复用 #957 的 `inventory_move_intent_with_rotated_true_swaps_dims_end_to_end`，增加客户端事件处理侧断言，覆盖普通 moved 成功路径。
- 负向测试：rotated 越界/碰撞被拒后，client 仍保持原 footprint；1x1、2x2、hotbar 目标 no-op 不应误触发替换。

## 审计来源

- 来源：PR #957 合并前的 0 上下文 validator（2026-07-06）。
- 结论：**real-on-main，player-facing，局部明确，可 fix_pr**。
- 本骨架只立项，不在该 PR 内修业务代码；后续 fix PR 需要按 inventory schema / server emit / client handler 三端一起收束。
