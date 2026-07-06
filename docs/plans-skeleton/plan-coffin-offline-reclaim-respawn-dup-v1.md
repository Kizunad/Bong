# plan-coffin-offline-reclaim-respawn-dup-v1（骨架）

> **骨架（草案）**。一句话主题：`coffin/player` 主链把“离线卧棺”同时当成**要持久化保留**和**要从运行态占用表里释放**两件互相冲突的事来做。结果是：玩家 A 断连时 `save_player_slices_with_coffin(..., coffin.map(|c| c.grade))` 仍把 `in_coffin=true` 写进 SQLite，但随后 `registry.clear_player(entity)` 又把该棺的 `occupied_by/player_in_coffin` 清空；其他玩家可趁 A 离线回收/破坏这口棺并拿走材料，而 A 重连时 `attach_player_state_to_joined_clients` 会基于 `persisted.in_coffin` 调 `registry.reclaim_occupied(...)`，在 registry 里**凭空补回棺记录**，并由 `rebuild_missing_coffin_markers` 自动重生 marker。影响是：**玩家能稳定遇到“离线卧棺被偷拆/被抢睡，重连后又冒出幽灵棺甚至材料 dup”的明确体验级 bug**。

> 立项动机：这不是冷门测试态，而是 `server/src/player/mod.rs` 的断连收尾、`server/src/coffin/mod.rs` 的回收/破坏、以及 join-time 恢复三条正式主路径首尾相接形成的状态机分叉。问题位于玩家高频可达的棺材玩法主链，且会直接影响占用、回收、重连与世界可见物。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 离线卧棺占用丢失，导致抢棺 / 拆棺 / 重连重生幽灵棺 | fix_pr | ⬜ |

## P0 — 离线卧棺占用丢失，导致抢棺 / 拆棺 / 重连重生幽灵棺

- **现象**：断连路径先把卧棺态带着存盘，再把运行态占用清掉。`server/src/player/mod.rs:411-420` 断连时调用 `save_player_slices_with_coffin(..., coffin.map(|c| c.grade))`，会把当前 `CoffinComponent` 转成 `player_lifespan.in_coffin=true`；但紧接着同文件 `:450-452` 又无条件 `registry.clear_player(entity)`。`server/src/coffin/mod.rs:223-231` 的 `clear_player` 会把 `player_in_coffin` 删除，并把该棺 `occupied_by` 置空。
- **可达链路**：A 在线卧棺后断连，棺在世界里还在，但 registry 已视为无人占用。此时 B 走正常交互即可继续主链：`handle_coffin_enter_requests` 只检查 `coffin.occupied_by.is_some()`（`server/src/coffin/mod.rs:589-612`）；`handle_coffin_breaks` / `handle_coffin_menu_reclaim` 也只在 `coffin.occupied_by` 非空时才弹出占用者并清库（`server/src/coffin/mod.rs:759-770`, `:892-903`）。所以 A 离线后，B 可以正常进同一口棺，或者直接回收/破坏拿走材料。
- **为什么这是 bug，不是设计**：join-time 恢复明确把“离线仍在棺内”当成真状态继续执行。`server/src/player/mod.rs:258-272` 若读到 `persisted.in_coffin`，会设隐身、按存档位置回推 `coffin_lower`，并调用 `registry.reclaim_occupied(coffin_lower, entity, 0, grade)`。而 `server/src/coffin/mod.rs:206-220` 的 `reclaim_occupied` 在棺已不存在时会直接 `unwrap_or(CoffinEntity { ... marker_entity: None })` 新建一条棺记录；棺存在但已被 B 占用时，又会把 `previous_player` 从 `player_in_coffin` 里剔掉，直接把 `occupied_by` 改回 A，却**不会同步移除 B 身上的 `CoffinComponent` / 隐身 / pinned 位置**。这说明系统并非“设计上允许离线自动出棺”，而是把同一事实同时走了“保留卧棺”和“释放占用”两套互斥语义。
- **对实际游玩体验的影响**：这会制造至少三类明确体感问题。1) A 只是正常下线挂棺，B 却能趁离线把棺回收或破坏拿走材料，形成“离线被偷拆”。2) 若 B 只是先睡进去，A 重连会把占用权硬抢回来，B 仍可能暂时保持隐身+卧棺组件，形成“一口棺双人/抢棺”。3) 若 B 已经把棺拆掉或回收完，A 重连又会凭 `persisted.in_coffin` 在 registry 里补回棺，并由 `rebuild_missing_coffin_markers`（`server/src/coffin/mod.rs:969-1000`）把 marker 重生出来，形成“材料已拿走但棺又长回来”的 dup/幽灵棺。
- **建议修复范围 / 模块**：优先收口 `server/src/player/mod.rs` 与 `server/src/coffin/mod.rs`。修复必须先做语义裁决并一次性闭环：要么“离线卧棺 = 继续占用该棺，不允许别人进入/回收/破坏”；要么“离线断连 = 视作离棺”，那就必须在断连时同步把 SQLite `in_coffin` 清零并把玩家移到出棺位，不能允许 join-time 再 `reclaim_occupied`。无论选哪条，`reclaim_occupied` 都不该在缺棺时默默补建实体语义，也不该无副作用夺走在线占用者的映射。
- **验收抓手**：至少补 4 组 pin。1) 玩家 A 卧棺断连后，B 不得能在不触发明确设计许可的前提下进同一口棺。2) A 卧棺断连后，B 回收/破坏该棺时，A 的持久化与世界态必须保持一致，不能重连复活幽灵棺。3) 若系统选择“离线继续占用”，则重连前后 registry / SQLite / marker / `CoffinComponent` 必须同源一致。4) 若系统选择“离线自动离棺”，则断连后 `persisted.in_coffin` 必须被清零，重连不得触发 `registry.reclaim_occupied`。

## 反方裁决摘要

1. Round 1 怀疑点是“也许离线清占用是刻意设计，表示断连就自动出棺”。复核后被 `server/src/player/mod.rs:258-272` 否掉：join-time 明确仍按 `persisted.in_coffin` 复原隐身、`CoffinComponent` 与 registry 占用；这和“自动出棺”语义直接冲突。
2. Round 2 怀疑点是“即便占用分叉，可能只是 UI/占位小错，不一定伤玩家”。复核 `server/src/coffin/mod.rs:206-220`, `:759-770`, `:892-903`, `:969-1000` 后否掉：缺棺时会补建 registry，recovery 会重生 marker，回收/破坏又会给当前操作者返材料，所以这是可落到**材料收益 + 世界可见物重生 + 双人抢棺**的实际玩法 bug，不是纯显示层错位。
3. 人工复核继续确认：仓库内没有任何“disconnect while in coffin”专门清理 `persisted.in_coffin` 的路径，也没有在 `reclaim_occupied` 里校验世界方块/物理棺仍存在。因此该候选在两轮证伪后继续存活，且置信度高。

## 开放问题

1. 设计应裁决“离线卧棺是否继续占用实体棺位”。这会决定 fix 是保占用，还是断连时彻底离棺并清持久化。
2. 若保留 `reclaim_occupied` 作为重连恢复工具，是否需要附加“棺实体仍存在 / 未被回收 / 未被他人占用”的校验与失败降级路径，避免它继续承担“补建棺”的越权语义。

## 审计来源

bughunt 线程 R，限定 `coffin/death_lifecycle/player` 主路径的 report-only 审计。结论来自主代理对 `disconnect -> reclaim/break -> reconnect recovery` 三段正式代码链的交叉复核；当前建议先以 skeleton-only PR 固化玩家影响、可达链路与修复边界，再由后续 fix PR 处理状态机收口。
