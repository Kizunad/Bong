# plan-bughunt-botany-growth-cost-harvest-ledger-v1

## §0 摘要

野生灵材生长扣 `zone.spirit_qi`（`growth_cost`）与采集获得的 `item.spirit_quality` 完全脱钩——生长侧从 zone 真实扣款（该字段直接是 `qi_physics::ledger::summarize_world_qi` 里 `zone_qi` 分量的组成部分），采集侧物品品质完全来自静态 item template + 技能加成、与该植株实际消耗的 `growth_cost` 无任何数量级关联；自然凋亡也只归还 `growth_cost*restore_ratio`（典型 80%），玩家实际采集（核心常规玩法）时归还逻辑整段跳过，`growth_cost` 100% 从 zone 永久消失。findings 报的 severity 是 high，但 skeptic 对峙结论建议基于「吞真元 = 阻塞合并」硬约束上调至 critical——本 skeleton 沿用 finder 原始 high 标注，把 skeptic 的上调建议记录在 §8，留给实施阶段拍板。

本 plan 仅是 BugHunt Skeleton Plan，不包含实际修复。

## §1 实际游玩体验影响

- 玩家标准 R 键采集野生灵材（核心日常采集玩法，不涉及灵田 plot、不涉及任何 dev 指令）会让全服真元总量 `total_observed`（`qi_physics::ledger::summarize_world_qi`）出现与该植株 `growth_cost` 完全不成比例的净增，典型净增 40~500 倍于扣除量。
- 这违反 CLAUDE.md 明确的"真元/灵气守恒律"最高优先级硬约束：任何 `zone.spirit_qi -= Y`（无对应玩家增）都是红旗，而本 finding 恰恰是"扣款"和"新增"两侧完全独立、互不对齐的教科书案例。
- 由于触发路径是游戏最基础高频的采集循环（非边缘场景），且随游玩时长无上界持续累积（只要 zone.spirit_qi 能被其他系统回充就可无限重复"种-采"循环），长期运行会显著扭曲全服真元经济。

## §2 复现路径

1. 玩家在任意挂载了野生灵材（几乎所有已配置 zone 均有对应 `BotanyPlantKind`）的 zone 内正常游玩。
2. `run_botany_lifecycle_tick`（`server/src/botany/mod.rs:87-93`）是无条件注册进 `Update` schedule 的常驻系统，自动按 `growth_cost`（典型 0.002~0.01）消耗 `zone.spirit_qi` 生成植株：`zone.spirit_qi = (zone.spirit_qi - growth_cost).clamp(-1.0, 1.0)`（`server/src/botany/lifecycle.rs:200,528,581`）。
3. 玩家用标准 R 键采集交互（`harvest.rs` 常规路径）拾取成熟植株。
4. 现状预期：`apply_harvest_modifiers_to_item`（`server/src/botany/harvest.rs:396-402`）把新物品 `spirit_quality` 设为 `template.spirit_quality_initial + herbalism_quality_bonus + variant.quality_modifier()`（静态配置 0.4~1.0 区间），与该植株消耗的 `growth_cost`（0.002~0.01 量级）无任何联动。
5. 修复后预期：生长/采集/凋亡三个断点之间建立真实可核算的 qi 转移账户，采集获得的 `spirit_quality` 与该植株实际消耗/持有的 qi 量真实挂钩，凋亡/采集的 zone 扣款要么全额可追踪归还、要么显式路由到 tracked sink，不再存在"扣款"和"新增"两侧完全独立的数值。

## §3 根因证据

- `server/src/botany/lifecycle.rs:200,528,581`：`spawn_v2_plants_for_zone`/`ZoneRefresh`/`StaticPoint` 三条生长路径每次生成一株植物都执行 `zone.spirit_qi = (zone.spirit_qi - growth_cost).clamp(-1.0, 1.0)`。
- `server/src/qi_physics/ledger.rs:707-767` `summarize_world_qi`：`zone_qi` 直接对 `ZoneRegistry.zones[].spirit_qi` 求和，无中间镜像层——生长侧扣款是全服真元总量 `total_observed` 的真实组成部分。
- `server/src/botany/lifecycle.rs:413-433,458-462`：`restore_ops` 只在 `!plant.harvested`（自然凋亡）时 push，且只归还 `growth_cost*restore_ratio`（典型 0.8，即自然凋亡也永久蒸发 20% 且不进 `era_decay_accum` 任何可追踪账户）；`harvested=true`（常规 R 键采集，最常见玩法）时该归还逻辑整段跳过，`growth_cost` 100% 从 zone 永久消失且永不归还。
- `server/src/botany/harvest.rs:244-250,285,396-402`：`harvest_spirit_quality`/`apply_harvest_modifiers_to_item` 里新物品 `spirit_quality` 完全来自静态 `template.spirit_quality_initial`（`server/assets/items/core.toml` 实测 0.4~1.0，例如 `gu_yuan_gen=1.0`、`hui_yuan_zhi=0.96`）+ `herbalism_quality_bonus`（玩家草药技能加成）+ `variant.quality_modifier()`，与该植株实际消耗的 `growth_cost` 在数量级上毫无关联。
- `server/src/qi_physics/ledger.rs` 的 `item_qi()`/`inventory_qi()`：`item.spirit_quality*stack_count` 被计入 `container_qi`，是 `summarize_world_qi` 总量的另一真实组成分量；`qi_physics/attrition.rs` 的设计注释与测试（"item.spirit_quality 减少量（绝对值）== zone 接收量"）进一步证实 `item.spirit_quality` 在本项目里被当作"真实可核算的 qi 量"而非纯 flavor 数值——反向坐实"harvest 凭空新增 item.spirit_quality 而无对应扣款"确实是守恒律违反。
- 现存对照测试 `harvested_plant_does_not_restore_spirit_qi`（`lifecycle.rs` 附近）明确写着设计意图"灵气随玩家离开 zone"，但该意图从未真正落地成"item 携带等量 qi 离开"——item 的 qi 值是独立静态配置，不是从 zone 转移过来的量。

## §4 非重复比对

- 已读 `docs/plan-bughunt-lingtian-plot-qi-ledger-gap-v1.md`（必读 active plan）：其证据链全部锚定 `lingtian/systems.rs` 的 `LingtianPlot.plot_qi`（灵田田块补灵/偷灵/压力清场），与本发现操作的对象是两个完全不同的字段/子系统——本发现是 `server/src/botany/lifecycle.rs` 直接操作 `world::zone::Zone.spirit_qi`（野生灵材自然生长，不经过灵田 plot、不经过 `lingtian::qi_account::ZoneQiAccount` facade），文件、函数、触发路径均不重合。
- 已读 `docs/finished_plans/plan-zone-qi-economy-v1.md`（commit `e211a81cb`/`9f9c279e5`）：Finish Evidence 明确列出落地范围仅覆盖 `meridian_open`/`breakthrough` 消耗回充待分配池、zone 均衡回流、NPC 让灵地板、灵潮/灵眼借还款，完全没有触及 botany `growth_cost`/`harvest_spirit_quality` 路径；提交 `9f9c279e5`"持久化伪灵脉待分配真元并隔离未记账生长"只处理 `is_ephemeral_pseudo_vein_zone` 这一支线（heartbeat 伪灵脉），普通 zone 的 `growth_cost` 扣款仍然裸露。
- Grep 全部 `docs/plans-skeleton` 与 `docs/*.md` 未命中 `growth_cost`/`restore_ratio`/`spawn_v2_plants_for_zone`/`harvest_spirit_quality` 等 symbol（仅命中 3 份 feature 型 plan：`plan-botany-v1`/`v2`、`plan-lingtian-v1`，均为原始设计文档非 bughunt 报告）。
- 已核对 `docs/finished_plans/plan-qi-physics-v1.md` 与 `plan-qi-physics-patch-v1.md` 的红线审计清单（P0-3/P0-6 等逐条核对），均未提及 botany/`growth_cost`/`harvest_spirit_quality`，证实这是 `qi_physics` 迁移审计从未覆盖到的盲区。

## §5 修复计划骨架

### P0 生长-采集-凋亡真实 qi 账户

- 生长扣款时（`lifecycle.rs:200/528/581`）把 `growth_cost` 通过 `qi_physics::ledger` 转入一个每株植物专属或按 zone 聚合的可追踪 pending 账户（如 `QiAccountId::container("botany_plant:<entity>")` 或复用 zone 侧待分配池），不要只做裸字段减法。
- 自然凋亡（wither，非 harvested）时把该账户全额（不是仅 `growth_cost*restore_ratio` 的近似值）按当前实际持有量转回 zone；`restore_ratio` 造成的差额如果是刻意设计的"自然耗散"，也要显式路由到 `era_decay_accum` 或专用 tracked sink，不能悄悄消失。
- 采集（`harvested=true`）时，`harvest.rs` 生成 item 的 `spirit_quality` 不应再独立于 `growth_cost` 现算——要么把植物账户里实际持有的 qi 值真实转入新建 `ItemInstance` 对应的 ledger 表示，要么至少把 `growth_cost` 那部分小额账户在 harvest 时结算掉，避免其永久挂账蒸发，同时不能让 item 的静态品质值被 `qi_physics` 计入 `container_qi` 却没有真实来源。

### P1 守恒单测

- 生长→采集全链路 `total_observed` 前后账，用 `qi_physics::ledger::assert_conservation` 锁死。
- 生长→自然凋亡全链路 `total_observed` 前后账，同样用 `assert_conservation` 锁死。
- 两条路径都不能再用"zone 侧局部字段不变"这种弱断言掩盖真实的账目缺口。

## §6 验证计划

- `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
- 手工/bot 复现矩阵：满仓 zone 长时间自然生长-凋亡循环（观察 total_observed 净变化）、玩家高频采集循环（观察 total_observed 净增）。

## §7 接入面与守恒说明

- 进料：`ZoneRegistry`/`Zone.spirit_qi`、`BotanyPlantKind`（`growth_cost`/`restore_ratio`）、`ItemRegistry`（`spirit_quality_initial`）、`qi_physics::ledger`。
- 出料：新建/复用的植物专属或 zone 聚合 pending qi 账户、`ItemInstance.spirit_quality`、`era_decay_accum` 或专用 tracked sink（差额路由）。
- 共享类型：复用 `qi_physics::ledger::QiTransfer`/`QiAccountId`，不新建独立记账结构。
- qi_physics 锚点：`qi_physics::ledger::summarize_world_qi`（`zone_qi`/`container_qi`/`item_qi`/`inventory_qi`）、`qi_physics::ledger::assert_conservation`；本 finding 本身是"未接入 qi_physics ledger"的孤岛，修复方向是接入既有 ledger 而非新增衰变常数。

## §8 对抗复核结论

- 候选证据：三条生长路径（`ZoneRefresh`/`StaticPoint`/`spawn_v2_plants_for_zone`）确认都执行 `zone.spirit_qi -= growth_cost`；`summarize_world_qi` 的 `zone_qi` 是对 `ZoneRegistry.zones[].spirit_qi` 直接求和，无镜像层；`restore_ops` 只在 `!harvested` 时生效且只归还 `growth_cost*restore_ratio`（常见 0.8），`harvested` 分支（常规 R 键采集）完全跳过归还；`apply_harvest_modifiers_to_item` 证实新物品 `spirit_quality` 与该植株消耗的 `growth_cost` 无任何联动；`item.spirit_quality*stack_count` 被计入 `container_qi`，是 `total_observed` 的另一真实组成分量；`qi_physics/attrition.rs` 的设计注释与测试证实 `item.spirit_quality` 在本项目里被当作真实可核算的 qi 量。
- 反方质疑：是否与 `plan-bughunt-lingtian-plot-qi-ledger-gap-v1` 重复（都是"灵气记账缺口"主题）？是否被 `plan-zone-qi-economy-v1` 已修复？
- 修正/反驳：`plot-qi-ledger-gap` 操作对象是 `LingtianPlot.plot_qi`（灵田田块，独立子系统，独立 facade `ZoneQiAccount`），文件/字段/触发路径均不重合，非重复；`plan-zone-qi-economy-v1` Finish Evidence 范围仅 meridian/breakthrough/NPC 让灵/灵潮借还，未触及 botany `growth_cost`/harvest 路径，commit `9f9c279e5` 明确只处理 `is_ephemeral_pseudo_vein_zone` 分支，普通 zone 路径未被触及；git log 对 `lifecycle.rs`/`harvest.rs` 的全部提交历史里无任何专门修复此 qi 脱钩的提交；`plan-qi-physics-v1`/`plan-qi-physics-patch-v1` 红线审计清单从未覆盖 botany。
- 反方最终裁决：通过（`is_real: true`, `reachable: true`）。**严重性建议**：finder 原始标注 high，但 skeptic 对峙结论认为应上调至 critical——理由是 CLAUDE.md 明确"真元/灵气守恒律"是全仓最高优先级硬约束、"吞真元 = 阻塞合并"，此缺陷系统性覆盖所有野生灵材品种/所有挂载 botany 的 zone，触发路径是游戏最基础高频的采集循环（非边缘场景），净增量级达 40~500 倍于扣除量，且随游玩时长无上界持续累积，与本仓库同批既往定级为 critical 的"开光通胀"等守恒漏洞同型同源。本 skeleton 保留 finder 原始 high 标注供实施阶段结合上述理由复核定级。
