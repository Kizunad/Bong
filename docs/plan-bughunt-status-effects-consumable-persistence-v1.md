# plan-bughunt-status-effects-consumable-persistence-v1

> Skeleton Plan（BugHunt persistence 第 3 轮）。仅记录真实 bug 与修复计划，不做实际修复。

## Bug 摘要

消耗品产生的长期 `StatusEffects` 没有玩家持久化 slice。玩家通过 quick slot 食用灵果/陈酒，或通过服丹路径使用抗灵压丹后，服务端会扣掉物品并把 `CultivationAcceleration` / `AntiSpiritPressurePill` 写入权威 `StatusEffects` 组件；但断线、关服 flush、重登 load 都不保存或恢复这组状态，加入游戏时还会重新插入 `StatusEffects::default()`。

结果是玩家已消耗物品换来的 36000-48000 tick 长效 buff 在断线或重启后直接清空。

边界：本 plan 不主张持久化所有 `StatusEffects`。短时控制、格挡窗口、姿态、装备/灵宝被动、家具 aura 等 transient 状态应保持运行态；本 bug 聚焦可明确归因于消耗品、且设计时长跨越大量游戏 tick 的长期效果。

## 实际游玩体验影响

玩家吃下灵果、陈酒后本应获得 1.5-2 个游戏日的修炼加速；服下抗灵压丹后本应在较长窗口内抵御涡流反噬。现在只要中途断线重连或服务器重启，物品已经从背包扣除，服务端权威 buff 却消失。

玩家视角会表现为“丹药/灵食吃了但重登后药效没了”：修炼速度回到默认，抗灵压保护消失，危险区域或涡流玩法里可能因为重启丢保护而遭遇额外反噬。这不是单纯 HUD 不显示，而是服务端 `StatusEffects` 本身被默认化。

## 复现路径

1. 玩家背包中有 `food.spirit_fruit.ling_guo`、`food.spirit_wine.chen_jiu` 或 `anti_spirit_pressure_pill`。
2. 通过 quick slot 食用灵果/陈酒，或通过 `AlchemyTakePill` 服用抗灵压丹。
3. 服务端先扣掉物品，再发送 `ApplyStatusEffectIntent`，把长期效果写入玩家 `StatusEffects`。
4. 在剩余 tick 尚未到期时断线重连，或服主正常关服后重启。
5. 玩家重登后 join 流程插入新的 `StatusEffects::default()`，此前的 `CultivationAcceleration` / `AntiSpiritPressurePill` 不再存在。

## 根因证据

- `server/src/combat/mod.rs:93-121`：加入玩家时 `attach_combat_bundle_to_joined_clients` 无条件插入 `StatusEffects::default()`。
- `server/src/combat/components.rs:383-395`：`ActiveStatusEffect` / `StatusEffects` 是服务端权威组件，带 `remaining_ticks`，且已派生 `Serialize` / `Deserialize`。
- `server/src/player/state.rs:155-168`：`LoadedPlayerSlices` 只包含 state、position、dimension、inventory、lifespan、coffin、skill、known_techniques、ui_prefs，没有 status 字段。
- `server/src/player/state.rs:419-544`：`load_player_slices` 只恢复上述 slice，没有读取 `StatusEffects`。
- `server/src/player/mod.rs:320-443`：断线 cleanup 保存 cultivation、player slices、known techniques，没有查询或保存 `StatusEffects`。
- `server/src/player/mod.rs:470-593`：关服 flush 同样只保存 cultivation、player slices、known techniques，没有 status flush。
- `server/assets/items/food.toml:39-77`：灵果配置 `food_regen` 48000 tick，陈酒配置 `food_regen` 36000 tick。
- `server/src/network/cast_emit.rs:250-329`：quick slot cast 会在通过腐败前置检查后扣库存，并调用 item effect。
- `server/src/network/cast_emit.rs:679-775`：`FoodRegen` 会发送 `ApplyStatusEffectIntent`，写入 `CultivationAcceleration`。
- `server/assets/items/pills.toml:81-93`：`anti_spirit_pressure_pill` 配置 `anti_spirit_pressure` 36000 tick。
- `server/src/network/client_request_handler.rs:12790-12925`：服丹路径先校验 effect、定位实例并调用 `consume_item_instance_once` 扣物品。
- `server/src/network/client_request_handler.rs:12998-13013`：抗灵压丹发送 `AntiSpiritPressurePill` status intent。
- `server/src/combat/status.rs:17-45`：`ApplyStatusEffectIntent` 落入 `StatusEffects`。
- `server/src/combat/status.rs:124-144`：`StatusEffects` 按 tick 递减并清理过期条目，说明这是服务端权威运行态，不只是客户端展示。
- `server/src/combat/woliu.rs:825-826`：涡流逻辑直接读取 `AntiSpiritPressurePill` 判定抗反噬。
- `server/src/network/status_snapshot_emit.rs:95-126`：HUD 名称只是从权威状态派生；本 bug 不同于 #993 的客户端 HUD 跨 session 残留。

## 修复计划骨架

- [ ] 增加玩家 status 持久化 slice，例如 versioned `PlayerPersistentStatusEffects`，字段至少包含 `kind`、`magnitude`、`remaining_ticks`、可选来源/分类和写入时间。
- [ ] 建立可持久化白名单或来源分类：先覆盖 `CultivationAcceleration`（灵食）、`AntiSpiritPressurePill`、消耗品产生的 `BreakthroughBoost` / 兽核突破加成；明确排除短时控制、格挡窗口、姿态、装备/灵宝被动和家具 aura。
- [ ] 断线 cleanup、周期 flush、关服 AppExit flush 都保存白名单状态；join/load 时在 combat bundle 初始化后合并已保存状态，而不是用默认空状态覆盖。
- [ ] 定义离线 `remaining_ticks` 策略。建议 P0 先冻结离线 tick，避免玩家因为服务器重启或掉线损失已付费/已消耗物品效果；若以后要墙钟衰减，需在 UI 和规则上明确告知。
- [ ] 保存前过滤 `remaining_ticks == 0` 的条目；load 时校验未知 kind、非法 magnitude、异常超长 tick，坏数据 warn 后丢弃单条，不阻塞玩家登录。
- [ ] 避免把 `StatusEffects` 整体 JSON 盲存，防止把 transient 战斗窗口、装备派生 buff、灵宝感知等运行态错误恢复到新 session。

## 验证计划

- [ ] server 单测：构造玩家 `StatusEffects` 含 `CultivationAcceleration(remaining_ticks=48000)`，触发断线持久化并 load，断言重登后仍有该状态和剩余 tick。
- [ ] server 单测：抗灵压丹 `AntiSpiritPressurePill(remaining_ticks=36000)` 经 shutdown flush 后重启 hydrate，断言 `woliu::check_backfire_resistance` 仍为 true。
- [ ] 负向测试：`Stunned`、`VortexCasting`、`SwordParrying`、`ShieldBlocking` 等 transient 状态不进入持久化 slice。
- [ ] 腐败/非法数据测试：未知 status kind、负数/NaN magnitude、0 tick、异常大 tick 不应 panic；合法条目继续恢复。
- [ ] 回归测试：状态自然到期后下一次 flush 会从持久化 slice 移除，避免过期 buff 重启复活。
- [ ] 跑 server 栈命令：`cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`。

## 对抗结论

- 第一轮反方质疑：不能简单声称“所有 `StatusEffects` 都应持久化”；候选中 `source_pill=Some` 和部分修炼丹药生产路径证据不足，应收窄到真实线上消耗品路径，并区分长期消耗品 buff 与 transient 状态。
- 主 agent 修正：移除 `source_pill` 和未接线修炼丹药作为主证据，改用灵果/陈酒 `FoodRegen` quick slot 路径与抗灵压丹 take_pill 路径；修复计划改为白名单/来源分类持久化。
- 第二轮反方最终裁决：`CONFIRMED`。真实线上 `FoodRegen` / `AntiSpiritPressurePill` 会消耗物品并写入服务端 `StatusEffects`，而玩家 load/flush/join 路径完全没有 status slice 且重登默认化，足以确认长期消耗品 buff 会跨断线/重启丢失。

## 风险

- 直接恢复全部状态会引入新 bug：短时控制或格挡窗口可能跨重登保留，装备派生 buff 可能重复叠加。
- 离线 tick 冻结与墙钟衰减会影响平衡；本 plan 建议先冻结以保护已消耗物品价值，但需要在实现时留下显式策略点。
- 若未来 `ApplyStatusEffectIntent` 增加来源字段，应与本 slice 对齐，避免同类消耗品状态又走出第二条生命周期。
