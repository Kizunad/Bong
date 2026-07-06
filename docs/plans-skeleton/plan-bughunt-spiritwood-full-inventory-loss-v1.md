# plan-bughunt-spiritwood-full-inventory-loss-v1

> **Skeleton Plan（BugHunt report-only）**。一句话主题：灵木采伐完成时先把原木标记已采并从世界移除，再尝试把 `ling_mu_gun` 放入玩家库存；背包满时 grant 失败只写 warn，玩家仍收到完成文案，稀缺灵木产物被吞。

## 1. 实际游玩体验影响

玩家正常砍一棵 SpiritWood 巨树，240 tick 进度结束时如果随身包和身袋没有可放 `1x2` 物品的位置，就会看到“采得灵木原木 ×N”的完成反馈，但背包里没有 `ling_mu_gun`，地上也没有掉落。与此同时，原木方块已经变成 AIR，`SpiritWoodHarvestedLogs` 也把该位置记成已采；玩家无法重新砍同一根木头，相当于把一份稀缺灵木资源永久吞掉。

这不是边角 UI 误报。`ling_mu_gun` 是 `rare` 灵木原木，尺寸 `1x2`，是灵木资源链入口（`server/assets/items/spiritwood/ling_mu_gun.toml:1-10`）。同类“满包完成但产物消失”已经在 botany 收获链被修成“入包或掉地”的原子语义（`docs/finished_plans/plan-botany-harvest-full-inventory-loss-v1.md:1-5`），灵木路径仍漏接。

## 2. 复现路径

1. 准备一名生存玩家，持有满足灵木采伐要求的斧头。
2. 把所有可携带容器塞满，确保没有 `1x2` 空位；即使已有 `ling_mu_gun`，只要 freshness 不同也不能合并。
3. 对 SpiritWood log 发起采伐并等待 240 tick 完成。
4. 观察结果：server 发送完成反馈，世界原木消失并被记录为已采，但玩家库存没有新增 `ling_mu_gun`，地面也没有 overflow 掉落。

## 3. 根因证据

- `WoodSession` 起采时记录 `dimension/log_pos`，完成时用这些字段处理世界状态（`server/src/spiritwood/session.rs:11-19`）。
- `complete_spiritwood_sessions` 在 grant 之前先执行不可逆副作用：`store.remove`、`harvested_logs.mark_harvested(session.dimension, session.log_pos, now_tick)`、`layer.set_block(..., AIR)`（`server/src/spiritwood/mod.rs:256-265`）。
- 之后才调用 `grant_ling_mu_gun_to_inventory`；失败分支只 `tracing::warn!`，没有回滚、掉地或保留 session（`server/src/spiritwood/mod.rs:285-298`）。
- 即使 grant 失败，后续仍会发送 `GatheringCompleteEvent` 和 `LumberTerminalEvent completed=true`，终态文案仍是 `采得灵木原木 ×{drop_count}`（`server/src/spiritwood/mod.rs:300-320`）。
- `grant_ling_mu_gun_to_inventory` 直调 `add_customized_item_to_player_inventory`，把 Err 原样上抛（`server/src/spiritwood/mod.rs:527-551`）。
- `add_customized_item_to_player_inventory` 只是 `add_item_to_player_inventory_inner` 的薄封装（`server/src/inventory/mod.rs:1699-1718`）；无可用格时 inner 明确返回 `Err("inventory full: ...")`（`server/src/inventory/mod.rs:1817-1899`）。
- 仓库已有通用满包兜底 helper `add_item_to_player_inventory_or_ground`，只在满包时写入 `DroppedLootRegistry`（`server/src/inventory/mod.rs:1720-1789`），botany 已按“grant 成功后再做不可逆副作用”的顺序使用它（`server/src/botany/harvest.rs:208-232`）。
- 堆叠不是可靠兜底：堆叠身份要求 freshness 完全相等（`server/src/inventory/mod.rs:2120-2139`），而灵木 grant 会按完成 tick 写入新的 `Freshness`（`server/src/spiritwood/mod.rs:538-550`）。

## 4. 去重说明

- 不重复 #1019 “灵木采伐关服前未强制落盘”：该题关注 flush 窗口内重启复刷，本题关注在线完成时满包吞 `ling_mu_gun`。
- 不重复 botany / craft / alchemy 的满包题：它们分别覆盖草药收获、craft 退款、炼丹取丹；本题代码路径是 `server/src/spiritwood/mod.rs` 的灵木采伐完成链。
- 本轮放弃“灵木跨维 session 不取消”作为主题：正式 TSY entry/exit 多数会改变 Position，可能被移动打断间接取消，置信度不如满包吞产物。

## 5. 修复计划骨架

- [ ] P0：把灵木完成链改成原子语义。产物成功 `Granted` 或 `DroppedToGround` 之前，不得执行 `harvested_logs.mark_harvested`、`layer.set_block(AIR)`、`store.remove` 和 completed=true 终态。
- [ ] P1：复用 `add_item_to_player_inventory_or_ground` 或等价策略。满包时生成 `DroppedLootEntry`，位置使用 `block_origin(session.log_pos)` 或采伐点，维度使用 `session.dimension`，并保留 `ling_mu_gun_v1` freshness 定制。
- [ ] P2：反馈诚实化。入包成功才说“采得灵木原木”，满包落地时明确提示“背包已满，灵木原木已落地”；结构性错误保持失败/中断语义，不伪装完成。
- [ ] P3：如果修复触及通用采集反馈，保持 botany/mineral 现有语义不回退；本题不改 qi、worldgen、persistence 或 client UI。

## 6. 验证计划

- [ ] 单测：满包且没有 `1x2` 空位时，灵木产物进入 `DroppedLootRegistry`，不是消失。
- [ ] 单测：已有 `ling_mu_gun` 但 freshness 不同且无空位时，不能误合并，仍走落地。
- [ ] 单测：只有在 grant 或落地成功后，才标记 harvested 并把 log 置 AIR。
- [ ] 单测：无 `DroppedLootRegistry` 或 unknown template 等结构性错误时，不标记已采、不置 AIR、不发送 completed=true。
- [ ] bot e2e：用黑盒 dev 命令填满背包后完成灵木采伐，断言能观察到地面掉落或明确反馈，后续可拾取。

## 7. 对抗结论

Round 1 反方质疑并核查了四类推翻点：`add_customized_item_to_player_inventory` 是否会自动落地、`ling_mu_gun` 是否总能堆叠、灵木是否有独立容器兜底、是否已有 spiritwood 满包测试或重复 PR。结论均不成立：满包会返回 `inventory full`，灵木 freshness 会阻止常规合并，现有测试只覆盖成功发放，不覆盖满包。

Round 2 主代理修正边界：放弃跨维 session 题，只保留“灵木满包吞产物”。反方最终裁决：**PASS**。该 bug 普通满包即可触发，当前路径确实先消耗世界资源再尝试发放物品，失败只 warn 且仍报完成；它不重复灵木落盘复刷，也不重复 botany/craft/alchemy 的满包题。
