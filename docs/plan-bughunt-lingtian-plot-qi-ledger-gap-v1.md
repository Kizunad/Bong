# plan-bughunt-lingtian-plot-qi-ledger-gap-v1

## §1 摘要

灵田 `ReplenishSource::Zone` 把区域灵气从 `ZoneQiAccount` 扣出后写进 `LingtianPlot.plot_qi`，但 `plot_qi` 没有对应 `WorldQiAccount` 余额，也不在 `summarize_world_qi` 的 `container_qi` 统计口径内。之后偷灵路径却从 `QiAccountId::container("lingtian_plot:x,y,z")` emit 转账事件，高压清场还会直接把 `plot_qi = 0.0`。结果是：补灵时灵气从全服守恒账面消失，偷灵/清场时又出现账面跳变或直接蒸发。

一句话 bug：灵田 plot_qi 是真实灵气池，但没有进入 qi ledger，zone 补灵后全服真元守恒账断裂。

## §2 证据

- `server/src/lingtian/systems.rs:1289-1351`：`ReplenishSource::Zone` 在 session 完成时从 `ZoneQiAccount` 扣 `amount`，再执行 `plot.plot_qi += added`；只有 `overflow` 回加 zone。
- `server/src/lingtian/qi_account.rs:62-70`：`ZoneQiAccount::sync_world_qi_account` 只把 zone 余额镜像到 `WorldQiAccount::zone(...)`，没有任何 `lingtian_plot:*` 账户。
- `server/src/qi_physics/ledger.rs:641-665`：`summarize_world_qi` 只统计 `ZoneRegistry`、`Cultivation`、`PlayerInventory` 和 `WorldQiAccount::total()`；`LingtianPlot.plot_qi` 不在统计口径内。
- `server/src/lingtian/systems.rs:1198-1230`：偷灵完成时从 `QiAccountId::container("lingtian_plot:x,y,z")` 向玩家/zone emit `QiTransfer`，说明实现语义已经把 plot 当作独立真元容器，但该容器没有真实余额。
- `server/src/lingtian/systems.rs:1660-1665`：压力 HIGH 时直接清零同 zone 的 `plot.plot_qi`，如果这些 qi 来自 zone，则这条路径会把已抽出的真实灵气无账销毁。

## §3 实际游玩体验影响

玩家用区域灵气给灵田补灵后，区域账面下降，但田块里的灵气没有被全服 ledger 追踪。短期表现是灵田看起来“吃掉”了环境灵气；长期表现是多块灵田反复补灵、偷灵、压力清场后，区域灵气生态和天道守恒看板会出现不可解释的亏空或跳变。玩家会看到灵田产出/偷灵正常发生，但世界环境没有按零和规则回暖或结算，破坏 “修炼消耗就是别人少掉” 的核心体验。

## §4 去重说明

- 不重复 #1050：本问题不是 craft `qi_cost` 固定落 `zone:spawn`。
- 不重复 #1056：本问题不是 NPC 日程/休息/QiSpring/Far LOD 凭空恢复真元。
- 不重复近期 overflow 主题（#989/#1020/#1043/#1046 等）：本问题不是技能/死亡 overflow 未落账，而是灵田 `plot_qi` 作为中间容器从未入账。
- 与 `docs/plan-bughunt-lingtian-default-zone-shadow-v1.md` 不同：该计划关注默认 zone/fallback 串账；本计划关注任意 zone 补灵后 `plot_qi` 脱离 `WorldQiAccount`。

## §5 建议修复方向

1. 给每块灵田 plot 建立稳定 `QiAccountId::container("lingtian_plot:x,y,z")` 余额，`ReplenishSource::Zone` 完成时用 `WorldQiAccount::transfer(zone -> plot, added)` 真实搬运。
2. `overflow` 继续回 zone，但应与 plot 入账同一事务内处理，避免字段账和 ledger 分离。
3. 偷灵路径不能只 emit `QiTransfer`；应从 plot container 账户真实转给玩家账户和 zone 账户。
4. 压力 HIGH 清空 `plot_qi` 时，必须把剩余 plot 余额按设计释放到 zone/overflow/沉降槽之一，禁止直接 `plot_qi = 0.0`。

## §6 验收抓手

- 单测：zone 初始 10.0，plot 初始 0，`ReplenishSource::Zone` 注入 0.5 后，`WorldQiAccount::zone(zone)` 减 0.5，`WorldQiAccount::container(lingtian_plot:...)` 增 0.5，账本总量不变。
- 单测：plot 接近 cap 时补灵，`added` 进 plot account，`overflow` 回 zone account，总量不变。
- 单测：偷灵完成时，plot account 减少量等于玩家 account 增加量 + zone account 增加量。
- 单测：压力 HIGH 清场前后，plot 中剩余 qi 不得从 `player_qi + ledger_qi` 口径消失。
- e2e/bot 场景：玩家在同一 zone 补灵、等待可偷灵/触发压力清场，观测服务端反馈与 world qi hash 不出现无来源跳变。

## §7 对抗结论

已完成两轮只读对抗 subagent。

- 第一轮 A 提出 `combat::resolve` 基础 `qi_invest` 扣费、灵田 `plot_qi` 未入账；第一轮 B 提出死亡 overflow emit-only、shelflife lazy decay。
- 第二轮 A 推荐灵田候选，理由是 `plot_qi` 未进入 `WorldQiAccount` 且 HIGH 清场直接清零，重复风险最低。
- 第二轮 B 推荐死亡 overflow，但承认灵田候选真实；死亡 overflow 与近期 overflow/死亡释放修复族重复风险更高。

最终选择灵田候选：证据链覆盖 zone 扣款、plot 入账缺失、偷灵虚拟账户、清场销毁四段，且不撞 #1050/#1056 与近期 server-qi overflow 主题。
