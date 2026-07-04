# plan-botany-harvest-full-inventory-loss-v1（骨架）

> **骨架（草案）**。一句话主题：修复 botany 收获在背包已满时的静默吞产出，避免成熟药材在正常游玩链路里“收完就没了”。

> **玩家影响**：玩家在田边或野外把背包塞满后去收获，植物会被标记为已收获并从场景消失，但产物因为入包失败被静默丢弃；表面上系统显示“收获完成”，实际没有拿到任何草药 / 种子 / 变种掉落。

## 阶段总览

| 阶段 | 主题 | 状态 |
|---|---|---|
| P0 | botany 收获满包吞产出的根因锁定 | ⬜ |
| P1 | 收获完成语义收口：失败不吞产出 | ⬜ |
| P2 | 满包回归与玩家可感知反馈 | ⬜ |

## P0 — botany 收获满包吞产出的根因锁定

- `server/src/botany/harvest.rs:105-123`
  `complete_harvest_for_player()` 先 `remove_session()`，随后就把 `plant.harvested = true`；这意味着后续任一步失败都已经进入“植物已收走”状态。
- `server/src/botany/harvest.rs:175-194`
  产物通过 `add_item_to_player_inventory(...) ?` / `add_customized_item_to_player_inventory(...) ?` 入包，背包满时会直接向上传 `Err`，但这里没有兜底逻辑。
- `server/src/botany/harvest.rs:517-535`
  `tick_harvest_sessions()` 直接 `let _ = complete_harvest_for_player(...)`，把上面的错误整个吞掉，玩家不会收到失败反馈。
- `server/src/inventory/mod.rs:1641`
  背包确实存在 `Err("inventory full: ...")` 路径，说明 botany 收获在满包时是可触发的正常失败分支，不是“理论不可能”。
- `server/src/botany/lifecycle.rs:382-405`
  `harvested=true` 会进入生命周期清理分支，植物后续会被当作已收获回收，进一步坐实“物品没进包、植物却没了”的可见损失。

## P1 — 收获完成语义收口：失败不吞产出

- 先定义失败策略，再改实现：
  - 方案 A：入包失败时保留 session / 植物状态，允许玩家腾包后重试。
  - 方案 B：先原子化准备掉落，再按满包策略回退或落地到明确的替代承载。
- 与 `plan-botany-v1` 的“drop 走背包”目标对齐，不能把满包失败默认为“成功完成”。
- 如需玩家反馈，反馈必须区分“收获成功但背包满”与“真正收获失败”，避免静默吞产出。

## P2 — 满包回归与玩家可感知反馈

- 满包时收获 `ci_she_hao` / `ning_mai_cao` / `ling_mu_miao` 不再丢失产出，至少要满足“物品还在可恢复路径里”。
- 自动收获与手动收获都要覆盖，不能只修某一条 session 入口。
- 任何失败提示都应在玩家正常 UI 可见，不依赖 console / server log。
- 回归测试要覆盖：满包、非满包、堆叠上限、带变种/品质修饰的 harvest、以及失败后再次尝试。

## 反方裁决摘要

- 证伪 round 1：`plan-botany-v1` 只定义“drop 走背包”，没有授权“满包直接吞掉产出”；`plan-lingtian-v1` 反而明确写了满包 `warn`，说明本仓对同类问题通常不会默认静默吞。
- 证伪 round 2：`botany/harvest.rs` 没找到任何失败补偿、地面掉落或回填 session 的旁路，`tick_harvest_sessions()` 还把错误吞掉；因此这个候选 survive，且玩家正常游玩可达。
