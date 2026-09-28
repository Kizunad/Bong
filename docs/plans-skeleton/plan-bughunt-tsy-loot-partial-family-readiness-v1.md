# plan-bughunt-tsy-loot-partial-family-readiness-v1

> 骨架：来源 #1832。中层成功即标记 family ready，深层尚未生成的遗物永久跳过。

## §0 摘要

`server/src/inventory/tsy_loot_spawn.rs:169-194` 分别尝试 mid/deep 层；`:196-206` 只在完全没有 relic 时不标 family。mid 有结果、deep 未 ready 时仍写 family 标记，后续 spawn 将认为全家已处理，丢失 deep 遗物与易形残卷。

## §1 游玩影响

TSY 家族 loot 只生成一部分却被永久视为完成，玩家无法通过重试获得深层奖励。

## §2 复现路径

1. 让 mid layer 有空间、deep layer 未准备好。
2. 执行 family spawn，mid 成功而 deep 返回 not-ready。
3. 观察 family 标记已写入，重试不再尝试 deep。

## §3 今天 `origin/main` 根因证据

- `tsy_loot_spawn.rs:169-194` 独立尝试 mid/deep。
- `:196-206` 的 readiness 判定只检查 relic 总数，而不是每层 outcome。
- family marker 与实际生成集合不一致，后续幂等 gate 锁死缺口。

## §4 非重复比对

`plan-tsy-loot-v1` 定义掉落分类，`plan-bughunt-tsy-death-drop-anchor-v1` 处理死亡坐标；本骨架只处理 family 分层生成的部分成功事务。

## §5 立项检查记录

本记录已按 `docs/worldview.md`、`docs/finished_plans/`、`docs/plan-*.md`、`docs/plans-skeleton/` 与 `docs/plans-skeleton/reminder.md` 逐项检索。

- **worldview**：查“TSY、家族遗物、深层、易形残卷”；`worldview.md §十二` 要求秘境奖励按层可追溯。
- **finished_plans**：查 `TsyLootFamily`、mid/deep spawn、readiness marker；无部分成功状态。
- **active plan**：查 tsy loot spawn；无同一 marker 修复。
- **skeleton**：查 `partial family`、`readiness`、`mid deep`；无重复骨架。
- **reminder.md**：查 `TSY`、`family`、`relic`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：family id、mid/deep layer readiness、loot registry、spawn attempts。
- **Outputs**：逐层 relic/scroll entries、per-layer completion marker、重试状态。
- **共享类型或事件**：复用 `TsyLootFamily`、`TsyLootSpawnOutcome`、`DroppedLootRegistry`；marker 必须表达 partial。
- **server 符号**：`inventory::tsy_loot_spawn`、`TsyPresence`/family lifecycle。
- **agent**：无变更；TSY loot 不在 agent schema。
- **client**：无变更；掉落 entries 继续用既有 payload。
- **worldview 锚点**：`docs/worldview.md §十二` TSY 分层奖励。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 以逐层 outcome 计算 family readiness，partial 结果可安全重试 |
| P1 | ⬜ | mid/deep 各种就绪组合、重复 spawn、塌缩清理回归测试 |

## 来源 issue

- #1832 `[flash-review][major] 部分层 spawn 成功即标记 family`
