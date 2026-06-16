# plan-qi-conservation-leaks-v1（active）

> **Active（已从 skeleton 升级，待逐项消费）**。一句话主题：闭合代码库自检 bug-hunt 第 1 轮确认的一簇**真元守恒泄漏（吞真元红线）**——多处招式释放/法器开光/养护扣减真元却不归还 zone 或凭空注入，违反 worldview §二「全服灵气总量恒定」+ docs/CLAUDE.md 守恒律红旗。统一接 `qi_physics::ledger` / `qi_release_to_zone`。

> 立项动机：worldgen-v4 收官后代码库自检 bug-hunt round1（8 子系统 finder→对抗裁决）确认 6 条吞真元，含 **1 critical**（开光真元凭空注入=通胀）。`plan-qi-physics-patch-v1`（已 finished）P2-7 + 行 174 明确把「招式释放只扣不还」列为**遗留**——本 plan 承接闭合，并扩展到 forge 模块（开光/养护/进化整簇未接 qi_physics）。

## 阶段总览

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 投射物招式 miss/despawn 残余真元归还 zone（anqi / needle）→ 落点 zone，qi_release_to_zone | ✅ 2026-06-16 |
| P1 | forge 开光真元注入接守恒 + client-trust 校验（critical 通胀修复） | ✅ 2026-06-16 |
| P2 | forge 法器养护/品阶进化扣减接 ledger（BossDrain：扣 qi_current==进玩家 zone） | ✅ 2026-06-17 |
| P3 | tsy_drain ledger 双重计入修正（玩家余额虚化进 WorldQiAccount） | ⬜ |

## 接入面 checklist

- **qi_physics 锚点（核心）**：所有真元流动必走 `qi_physics::ledger::QiTransfer{from,to,amount}`；释放走 `qi_release_to_zone(amount, region, env)`，吸收走 `qi_excretion(...)`。**不新增物理常数**（plan-qi-physics-v1 唯一实现）。
- **进料/出料**：从 `cultivation::Cultivation.qi_current` 扣 → 必须有对应 zone 账户增（`ZoneRegistry` / `WorldQiAccount`）。
- **正典锚点**：worldview §二「真元极易挥发」+「SPIRIT_QI_TOTAL 恒定，修炼消耗=别人少掉」。
- **跨 plan**：承接 `plan-qi-physics-patch-v1`（finished）P2-7 遗留；参 `plan-qi-physics-v1`。

## P0 — 投射物招式残余真元归还

- **bug（吞真元 major×2）**：
  - `server/src/combat/anqi_v2.rs:473` cast 扣 `qi_current -= qi_cost` 无 QiTransfer；`carrier.rs:929` emit `ProjectileDespawnedEvent{residual_qi}` 唯一消费者 `anqi_event_bridge.rs:67-86` 只序列化 Redis，从不 `qi_release_to_zone` → miss 真元蒸发。
  - `server/src/combat/needle.rs:130` `despawn_expired_qi_needles` 仅 `despawn()`，无任何释放/事件（比 anqi 更隐身），`QI_NEEDLE_QI_COST` 已扣。
- **目标**：投射物 miss/过期 despawn 时把残余真元 `qi_release_to_zone` 还给落点 zone（命中则走伤害/吸收路径）。统一 projectile-despawn 守恒接线。
- **§N 待决**：residual 去向账户（落点 zone? 发射点 zone?）；命中已结算的不重复释放。

## P1 — forge 开光真元注入守恒 + client-trust（CRITICAL）

- **bug（吞真元 critical）**：`server/src/forge/steps.rs:226` + `client_request_handler.rs:2991-3021` `handle_forge_consecration_inject` 只校验 qi_amount 有限非负，emit `ConsecrationInject{qi_amount}`（**client 上报值**）；消费者 `forge/mod.rs:395-441` 只 `inject_qi(state,amount)` 累加，**全程不扣 Cultivation.qi_current、无 QiTransfer** → 真元凭空注入法器=**通胀**（比单向扣减更严重），且 client 可送任意值。对比正确模式 `craft/session.rs:374-389`（QiTransfer(Crafting)+ledger.transfer 后才扣 cultivation）。
- **目标**：开光注入走 qi_physics（玩家真元→法器，account 守恒）；**client-trust 校验** qi_amount ≤ 玩家余额（不信 client 值）。
- **正典**：`plan-qixiu-depth-v1`（finished）:49「不自定物理公式」+:129 开光须灌注玩家真元——自报完成却未接 qi_physics。
- **§N.1 决议（2026-06-16）**：开光真元 = player `Cultivation.qi_current` → station 所属 zone `WorldQiAccount`（BossDrain 记账 + `push_transfer_audit`；zone 不可解析进 `overflow`，真元绝不消失）；**非** player→artifact（法器不持 qi，开光真元在仪式中逸散入环境 zone，worldview §二「真元极易挥发」）。client-trust：注入量钳制 ≤ `qi_current`，显式拒非有限值，server 唯一权威。ledger 写成功才扣玩家真元（原子，写失败不毁真元）。落点 `server/src/forge/mod.rs::handle_consecration_injects`。

## P2 — forge 养护/进化扣减接 ledger

- **bug（吞真元 major×2）**：
  - `server/src/forge/artifact_meridian.rs:750` `artifact_meridian_maintenance_tick` 读→扣→写 `cultivation.qi_current` 无 QiTransfer，每 TICKS_PER_DAY 每玩家跑。
  - `artifact_meridian.rs:549` `apply_evolution_qi_cost` 品阶进化扣 `qi_current*0.3` 直扣无 ledger（`artifact_meridian_deepen_on_use:556`→630 调用）。
- **目标**：养护/进化扣减走 `QiTransfer`/`qi_release_to_zone`（消耗去向 zone 或合理账户）。测试补守恒断言（现有 `maintenance_drains_qi` 只验扣除掩盖泄漏）。

## P3 — tsy_drain ledger 双重计入修正

- **bug（吞真元 major）**：`server/src/world/tsy_drain.rs:78/93-100` `record_tsy_drain_transfer` 用 `set_balance(player, before_player_qi)` + `account.transfer()` 把玩家余额**虚化进 WorldQiAccount ledger**，而 `ledger.rs:336-342` 契约规定玩家真元存 ECS Cultivation 不进 ledger balances（BossDrain `boss_spawn.rs:406-409` 正确只 set zone + push_transfer_audit）。`summarize_world_qi`（ledger.rs:454-467）同时累加 player_qi(ECS) + ledger_qi → `total_observed` 虚增；经 `network/mod.rs:1155` 写 `bong:qi/ledger` Redis 暴露给外部守恒监控。
- **目标**：tsy_drain 改用 `push_transfer_audit` 模式（不改 from balance），修正 summarize 双重计入；同步改掉自证错误不变量的测试 `transfer_records_tsy_drain_without_losing_qi`。
- **§N 待决**：外部 telemetry 契约（bong:qi/ledger 消费方）是否依赖现有（错误）值。

## §N 开放问题

1. 各 residual/消耗真元的去向账户统一规约（落点 zone / 发射 zone / overflow）。**qc-P0 决议**：投射物 miss/expire 残余归**落点 zone**（despawn 消亡点，针无 Position 故按 velocity 外推；无 zone 进 overflow），走 qi_release_to_zone。
2. forge 整簇（开光/养护/进化）是否一并接一个 forge-qi-ledger 子系统，还是逐点。
3. client-trust 校验在 handler 层统一（≤玩家余额）。
4. **qc-P0 调研遗留**：anqi 注入型技能（SingleSnipe/MultiShot/SoulInject/ArmorPierce/EchoFractal）cast 扣 qi 但瞬时命中、无飞行实体 → miss 无归还路径、残真元蒸发（不在 P0 投射物 despawn 范围，待单独跟进）。

## 审计来源

代码库自检 bug-hunt round1（workflow，8 子系统 finder + 对抗裁决），confirmed 6 条吞真元。承接 `plan-qi-physics-patch-v1` P2-7 遗留。**report-only**：守恒律红线 + 跨模块 + 账户去向设计抉择，不擅自大改，待人工/consume 决议。
