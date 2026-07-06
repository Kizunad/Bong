# BugHunt: dormant NPC 负灵域死亡释放抹掉负缺口

## Bug 摘要

`server/src/npc/dormant/mod.rs::release_dormant_qi_to_zone` 在 dormant NPC 自然老死或离屏战死释放残余真元时，把 `zone.spirit_qi.max(0.0) * QI_ZONE_UNIT_CAPACITY` 作为 `qi_release_to_zone` 的物理基线。

这会把负灵域的负缺口直接抹成 0。例：`zone.spirit_qi=-0.5`、死亡 NPC 残余 `qi_current=8` 时，当前代码按 `zone_current=0` 计算，最终写回 `zone.spirit_qi=0.16`；符合既有物理语义的结果应是 `(-25 + 8) / 50 = -0.34`。这属于负灵域 phantom credit / 守恒口径错误，不是单纯显示偏差。

## 实际游玩体验影响

- 主世界已有负值 zone，例如 `server/zones.json` 的 `baolongwang_cavern_deep` 为 `-0.729232`、`wangyintai` 为 `-0.15544`。这些不是测试构造，其中 `baolongwang_cavern_deep` 是坍缩渊边缘采集/遭遇场景。
- 默认启动会 seed dormant 散修人口；玩家离开相关区域后，dormant 自然老死或离屏互殴死亡会走这条释放路径。
- 负灵域可能被少量 NPC 死亡真元突然抬成正灵气区，影响负灵压危险、生态刷新、风险判定、修炼/吸收门槛和玩家对区域危险的预期。
- 对玩家表现为：本应危险、贫瘠、负压的区域在玩家不在场时被离屏 NPC 死亡错误“净化”，区域状态与世界观和其他在线释放路径不一致。

## 证据定位

- `server/src/npc/dormant/mod.rs::release_dormant_qi_to_zone`：使用 `zone.spirit_qi.max(0.0) * QI_ZONE_UNIT_CAPACITY` 作为 release 基线，并在成功后 `zone.spirit_qi = outcome.zone_after / QI_ZONE_UNIT_CAPACITY`。
- `server/src/qi_physics/release.rs::qi_release_to_zone`：支持负 `zone_current`；测试 `release_accepts_negative_zone_qi` 已锁定负基线向 0 靠近的语义。
- `server/src/cultivation/death_hooks.rs::release_qi_amount_to_zone`：在线死亡释放使用裸 `zone.spirit_qi * QI_ZONE_UNIT_CAPACITY`。
- `server/src/npc/npc_skill.rs::release_npc_qi_to_zone` 与测试 `heal_negative_home_zone_no_phantom_credit`：明确写明负灵域不得 `.max(0.0)`，否则会“抹掉负缺口、凭空多 qi”。
- 同类约束还见 `server/src/combat/woliu.rs`、`server/src/combat/baomai_v3/tests.rs`、`server/src/social/mod.rs`、`server/src/combat/dugu_v2/tick.rs` 的负灵域回灌测试/注释。
- `server/src/npc/dormant/mod.rs::DormantRoguePopulationSeedConfig::default` 默认 seed `1000` 个 dormant rogue；`dormant_rogue_seed_snapshot` 写入 faction 与 emergent group，离屏战斗和自然老死均可触发释放。
- `server/src/npc/spawn/rogue.rs::classify_zones_by_qi` 只过滤不能 spawn Rogue 的 dead zone；负灵域不是 dead zone，仍可进入 background 候选。

## 触发路径

1. server 启动，`seed_initial_dormant_population_on_startup` 在空 `NpcDormantStore` 时按 zone 候选 seed dormant rogue。
2. 某个 dormant rogue 位于主世界负值 zone，例如 `baolongwang_cavern_deep` 或 `wangyintai`。
3. `dormant_global_tick_system` 推进时间后，快照自然老死；或同 zone 内敌对 dormant 被 `run_dormant_combat_phase` 判为败者。
4. 结算调用 `release_dormant_qi_to_zone`。
5. 当前代码把负 `zone.spirit_qi` 当作 0 传入 `qi_release_to_zone`，导致写回的 zone 值从负数跳到正数或过高值。

## 反方审查记录

### 第一轮质疑

- 反方指出不能简单把 dormant 路径改成裸 `zone.spirit_qi * QI_ZONE_UNIT_CAPACITY` 后继续 `ledger.set_balance(zone, signed_negative)`，因为 `WorldQiAccount::set_balance` 经 `finite_non_negative` 拒绝负数；直接照搬会让 release 返回 `None`。
- 反方指出 `death_hooks` / `npc_skill` 多为事件或审计路径，不能完全等同于 dormant 的真实 `WorldQiAccount::transfer` 路径。
- 反方指出 TSY lifecycle 可能同 tick 重写 TSY zone，TSY hostile/sentinel 也未证明一定有 faction/effective group，不宜把 TSY 战死作为核心复现。
- 反方要求补主世界负 zone、seeded dormant rogue 和修复策略边界。

### 第二轮补证

- 主路径改为主世界负值 zone，不依赖 TSY lifecycle：`baolongwang_cavern_deep`、`wangyintai` 均在 `server/zones.json`。
- 确认默认 dormant rogue seed 数为 1000，且 seeded 快照写入 `faction` 和 `emergent_group`，可自然老死，也可离屏战斗死亡。
- 明确修复不能直接向 `WorldQiAccount` 写负余额；必须拆分 signed 物理基线与非负 ledger 镜像语义。
- 同类负灵域 `.max(0.0)` phantom credit 已在多个模块被测试和注释锁定为 bug。

### 最终裁决

反方裁决：通过，可以开 Skeleton Plan PR。剩余难点是修复设计复杂度，不影响立项。

## Skeleton Fix Plan

- [ ] 在 `release_dormant_qi_to_zone` 增加回归单测：`zone.spirit_qi=-0.5`、`snapshot.cultivation.qi_current=8.0`，调用后 `zone.spirit_qi` 应为 `-0.34`，不是 `0.16`。
- [ ] 设计 dormant 专用释放 helper 或重构 `release_dormant_qi_to_zone`：`qi_release_to_zone` 的输入使用 signed absolute 基线 `zone.spirit_qi * QI_ZONE_UNIT_CAPACITY`。
- [ ] 同时保持 `WorldQiAccount` 非负余额约束：不得调用 `set_balance(zone, signed_negative)`；明确负区间的 ledger 镜像/审计策略，避免从 phantom credit 退化为 release 失败。
- [ ] 覆盖跨 0 大额释放：负 zone 下大额 release 先填负缺口，超过 0 的部分进入 zone 正余额，超过 cap 的部分保留在 NPC/overflow，不蒸发。
- [ ] 覆盖自然老死集成路径：构造负值 background zone 中的 dormant rogue，`dormant_global_tick_system` 后死亡释放不把 zone 抬成错误正值。
- [ ] 覆盖离屏战斗路径：同 zone 两个敌对 dormant，败者释放在负 zone 中按 signed 基线计算；该测试不依赖 TSY faction。
- [ ] 审计现有 `.max(0.0) * QI_ZONE_UNIT_CAPACITY` release 调用点，区分“吸收/借款取可用正余额”与“释放回 zone 必须 signed 基线”的语义，不做无关重构。

## 验收测试计划

- `cd server && cargo test release_accepts_negative_zone_qi`
- `cd server && cargo test heal_negative_home_zone_no_phantom_credit`
- `cd server && cargo test dormant_negative_zone_release_no_phantom_credit`
- `cd server && cargo test dormant_global_tick_negative_zone_natural_death_release_uses_signed_baseline`
- `cd server && cargo test dormant_combat_negative_zone_release_uses_signed_baseline`
- `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`

## 风险

- `WorldQiAccount` 不允许负余额，修复必须避免把 signed zone deficit 直接写进 ledger balance。
- 如果只改 `zone.spirit_qi` 而不调整 ledger 审计，`bong:qi/ledger` telemetry 可能出现语义分叉。
- 若错误地把所有 `.max(0.0)` release 点机械改成 signed，会误伤吸收/可用余额类路径。
- TSY lifecycle 会重写 TSY 子 zone，TSY 相关影响面需要单独验证；本 bug 的主验收不依赖 TSY。
