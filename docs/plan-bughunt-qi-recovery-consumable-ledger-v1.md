# plan-bughunt-qi-recovery-consumable-ledger-v1

## 0. BugHunt 结论

玩家自用 `ItemEffect::QiRecovery` 消耗品（回元丹、回元芷煎汤等）在 `AlchemyTakePill` / QuickSlot 施效时直接调用 `recover_current_qi` 增加 `Cultivation.qi_current`，没有从区域灵气、物品封存真元、容器账户或 `WorldQiAccount` 转移真元。

这违反 `docs/CLAUDE.md §四 L59` 与 `AGENTS.md §9` 的守恒红线：`cultivation.qi_current += X` 必须有对应来源，所有真元/灵气流动必须走 `qi_physics::ledger::QiTransfer` 或等价的账本落点。

## 1. 去重记录

- 已避开 #1050 craft qi_cost 固定落 `zone:spawn`。
- 已避开 #1056 NPC 日程 / 休息 / QiSpring / Far LOD 凭空恢复真元。
- 已避开 #1076 灵田 `plot_qi` 未进 `WorldQiAccount`。
- 已避开 #1082 灵蝗潮推进扣 zone qi 未入账。
- 已避开 #1089 垂死大能 rift drain 未落账。
- 已避开 #1096 气针过期抹掉负灵域缺口。
- 已避开 #1102 暴龙王 BossDrain zone shadow。
- #1072 覆盖 bot 三产三用 / 丹药链路，但只验证“吃丹后真元上升、物品扣除、clamp”，没有守恒账断言。
- `docs/finished_plans/plan-dandao-runtime-wiring-v1.md:43` 明确把 `consume_pill` 服丹恢复 qi 未走 ledger 标为“后续 alchemy 守恒专项 plan”债务，不是已修项。

## 2. 证据

- `server/src/network/client_request_handler.rs:12919`：`handle_alchemy_take_pill` 先 `consume_item_instance_once` 扣掉玩家背包中的丹药实例。
- `server/src/network/client_request_handler.rs:12965`：`ItemEffect::QiRecovery` 分支 clone 当前 `Cultivation`，调用 `recover_current_qi`，再 `commands.entity(entity).insert(cultivation)`；该路径没有扣 zone qi，也没有写 `WorldQiAccount`。
- `server/src/network/cast_emit.rs:524`：QuickSlot / cast 路径的 `ItemEffect::QiRecovery` 同样直接 `recover_current_qi(cultivation, *amount)`；该施效函数没有 `ZoneRegistry` / `WorldQiAccount` 入参。
- `server/src/cultivation/components.rs:401`：`recover_current_qi` 只做 `qi_current + amount` 后 clamp 到有效上限。
- `server/src/alchemy/pill.rs:180`：旧 `consume_pill` 的 `qi_gain` 也是直接增加 `cultivation.qi_current`。
- `server/assets/items/pills.toml:45`：`huiyuan_pill` 的 `spirit_quality_initial = 1.0`。
- `server/assets/items/pills.toml:47`：同一枚回元丹的 `qi_recovery magnitude = 60.0`。
- `server/assets/items/workbench_materials.toml:486`：`huiyuan_decoction` 的 `qi_recovery magnitude = 40.0`。
- `server/src/qi_physics/ledger.rs:701`：通用物品真元快照按 `spirit_quality * stack_count` 统计，没有“1.0 灵质等价 60 当前真元”的全局换算。

## 3. 守恒账推导

以一枚 `huiyuan_pill` 为例：

- 当前实现：背包物品被消耗，物品侧最多减少 `spirit_quality = 1.0`；玩家 `qi_current` 最多增加 `60.0`。
- 账面净变化：`player_qi +60.0`，`item_qi -1.0`，未见 `zone_qi -59.0`、`container_qi -59.0`、`alchemy_conversion_sink/source` 或 `WorldQiAccount` 迁移。
- 结果：一次服丹约净铸造 `59.0` 真元。

`huiyuan_decoction` 同理：若物品灵质按 `0.5` 计，恢复 `40.0` 当前真元，则一次净铸造约 `39.5`。

## 4. 实际游玩体验影响

玩家在战斗、逃亡、渡劫前后可以用回元丹 / 回元芷煎汤直接补回大量当前真元，但服务器不会从周围区域灵气或丹药封存真元中扣除对应数量。批量炼丹、刷丹或交易囤丹后，玩家可以持续把消耗品转成额外真元，绕开“末法世界修炼消耗就是别人少掉”的零和经济。

从玩家体感上，这会让回元丹从“昂贵补给”变成真元铸币机：短期表现是续航过强，长期表现是区域灵气压力、负灵域风险和丹药供给成本都被低估。

## 5. 修复要求

- [ ] 统一梳理所有玩家自用 `QiRecovery` 入口：`AlchemyTakePill`、QuickSlot / `cast_emit`、旧 `consume_pill` 辅助，以及测试中直接调用的 runtime helper。
- [ ] 明确定义丹药恢复真元的守恒来源：优先从被消耗物品的封存 qi / `freshness.current_qi` / `spirit_quality` 对应账户转入玩家；若设计需要“药性转化倍率”，必须在 `qi_physics` 中新增显式转化账户或沉降/释放规则，不允许裸加 `qi_current`。
- [ ] 施效时写 `WorldQiAccount` 或等价账本：`from=item/container/alchemy_source` → `to=player:<id>`，reason 独立于垂死大能 `TradeDan`，避免混淆任务给丹路径。
- [ ] QuickSlot 路径补齐 `ZoneRegistry` / `WorldQiAccount` 或集中调用同一个 `apply_qi_recovery_conserved` helper，避免两个入口再次漂移。
- [ ] 若恢复量超过丹药可供给真元，必须有可解释策略：按可供给量 clamp、消耗额外 zone qi、或把差额记入明确的炼丹转化来源；不能静默增发。
- [ ] 补饱和测试：回元丹、回元芷煎汤、满真元 clamp、低物品 qi、缺 `WorldQiAccount`、QuickSlot 与 `AlchemyTakePill` 两条入口、`WorldQiSnapshot` 总量不增。
- [ ] 更新 bot e2e 丹药场景：黑盒验证吃丹后 `qi_current` 上升仍保留，但同时要求服务端可观察日志 / dev 账本快照显示守恒闭合。

## 6. 验收

- `server/`：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`。
- 根目录：`BONG_SKIP_SKIN_PREFETCH=1 bash scripts/smoke-test-e2e.sh`。
- 丹药 bot 场景覆盖：`bash scripts/bot-e2e.sh` 或等价 CI bot stage。

## 7. 对抗结论

- 第一轮 A：确认玩家自用 `QiRecovery` 服丹直接恢复当前真元，守恒账约 `+59` / 枚回元丹。
- 第一轮 B：提出 shelflife 衰变账外流失候选；该主题与旧 `qi_physics` / attrition 债务更接近，重复风险更高，本 plan 不采用。
- 第二轮 A：反驳审查确认 `handle_apply_pill` / QuickSlot 无隐藏 zone、item 或 `WorldQiAccount` 对冲；#1072 只锁行为，不锁守恒。
- 第二轮 B：确认不是 #969-#1102 明显重复；历史 plan 明确承认这是未修 alchemy 守恒债务。

置信度：高。
