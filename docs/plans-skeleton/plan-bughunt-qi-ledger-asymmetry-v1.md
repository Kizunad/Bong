# plan-bughunt-qi-ledger-asymmetry-v1：道伥凝结与死亡释放的 qi 账本不对称

> **骨架（草案）**。一句话主题：修复道伥 TiandaoCondense 凝结入口与死亡 release_external_qi_to_zone 归还入口之间的真元单位、账本、external-owner 身份和 snapshot 投影缺陷；置顶裁决是当前分率/raw 错配会按 `actual_cost × (QI_ZONE_UNIT_CAPACITY - 1)` 真正蒸发真元，属于最高优先级守恒红旗/P0；producer 责任与事务边界已在下方 Pre-P0 Decisions 收口，生产实现仍须由 active 阶段完成。

## 阶段总览

| 阶段 | 交付物 | 状态 | 验收日期 |
|---|---|---|---|
| P0 | P/L/T 守恒口径、owner 唯一性、单位/事务/失败边界和既有 plan 责任收口 | ⬜ | YYYY-MM-DD（待定） |
| P1 | 道伥凝结→spawn→死亡归还的 canonical transaction、稳定 owner identity 与常量引用测试迁移 | ⬜ | YYYY-MM-DD（待定） |
| P2 | external-owner registry、summarize_world_qi 投影和持久化/重启生命周期 | ⬜ | YYYY-MM-DD（待定） |
| P3 | QiTransfer 影响面逐条归类、选定路径迁移和不双扣回归 | ⬜ | YYYY-MM-DD（待定） |
| P4 | 真实运行链路、snapshot/ledger 审计和守恒集成验收 | ⬜ | YYYY-MM-DD（待定） |

## 最高严重度红旗：分率/raw 单位错配导致真元蒸发（P0，阻塞合并）

本轮复核收口了此前的错误裁决：`Zone.spirit_qi` 是 `-1..1` 的 signed 归一化分率，绝对 zone 真元必须是 `zone.spirit_qi × QI_ZONE_UNIT_CAPACITY`；`Cultivation.qi_current`、`DaoZhangBehaviorBlackboard.daozhan_qi`、`QiTransfer.amount` 和 `release_external_qi_to_zone` 的 `requested` 则是 raw 绝对真元。凝结入口只对高于 `TIANDAO_CONDENSE_THRESHOLD` 的正灵气区计算正的 `actual_cost`，该值仍是分率。

当前实现先按分率扣掉 `actual_cost × QI_ZONE_UNIT_CAPACITY` 的 zone 绝对真元，却把未换算的 `actual_cost` 写入 `SpawnDaoZhangFromCondenseRequest.condensed_qi`，再写入 raw blackboard。若完整 spawn→死亡路径按现有接口把这笔 raw 余额归还，净效果仍是：

    zone 扣减 = actual_cost × QI_ZONE_UNIT_CAPACITY
    道伥获得/归还 = actual_cost
    净蒸发 = actual_cost × (QI_ZONE_UNIT_CAPACITY - 1)

例如 `actual_cost = 0.05`、`QI_ZONE_UNIT_CAPACITY = 50` 时，每次凝结→死亡净蒸发 `2.45` raw；这是物理量损失，不是单纯的 `WorldQiAccount` 审计不对称。该缺陷触犯仓库最高优先级真元守恒红旗，修复前阻塞合并；本 skeleton 只固定证据与实施门，不修改生产代码。

## 接入面与范围边界

- **进料**：server/src/fauna/daozhan.rs::daozhan_tiandao_condense_system 从高灵气 ZoneRegistry 读取 Zone.spirit_qi，计算 actual_cost，发出 SpawnDaoZhangFromCondenseRequest 和 QiTransferReason::TiandaoCondense；spawn consumer 将 condensed_qi 写入 DaoZhangBehaviorBlackboard.daozhan_qi。死亡/销毁入口读取同一 blackboard 余额，调用 release_external_qi_to_zone，再投影已提交的 transfers。
- **出料**：修复后的 canonical transaction 必须同时定义 zone、道伥 external owner、WorldQiAccount balance/audit、spawn request、死亡释放和 WorldQiSnapshot 的边界；失败时不可留下部分 zone debit、孤儿账户或重复 owner 余额。
- **共享类型 / event**：复用 ZoneRegistry、Zone.spirit_qi、DaoZhangBehaviorBlackboard、SpawnDaoZhangFromCondenseRequest、QiTransfer、QiTransferReason::{TiandaoCondense,ReleaseToZone}、WorldQiAccount、WorldQiBudget、summarize_world_qi、release_external_qi_to_zone、QI_ZONE_UNIT_CAPACITY。不得另造第二套 qi ledger 或把 QiTransfer event 当作自动 consumer。
- **跨仓库契约（按阶段）**：**server（P0-P4）**命中 `qi_physics::ledger::{WorldQiAccount, QiTransfer, WorldQiSnapshot, QiPhysicsIpcSnapshot, summarize_world_qi, assert_conservation}`、`fauna::daozhan::{daozhan_tiandao_condense_system, daozhan_death_qi_release_system, DaoZhangBehaviorBlackboard}`、`cultivation::components::qi_flow::{release_external_qi_to_zone, transfer_external_qi_to_ledger}`、`network::publish_qi_ledger_to_redis` 和 `schema::channels::QI_LEDGER_REDIS_KEY`。当前 `summarize_world_qi` 的结果在 `server/src/network/mod.rs:1425-1464` 只进入 `bong:qi/ledger` telemetry；`server/src/network/mod.rs:1297-1377` 的 `publish_world_state_to_redis` 不消费它，因此 **agent：N/A，client：N/A**——agent/packages/schema、agent runtime 和 client 没有订阅/解析 `bong:qi/ledger` 的现有 symbol，本 plan 当前也不改 `bong:world_state` 字段集或新增 wire。若 P2 决定把 external owner 投影进 `bong:world_state`，或改变 `QiPhysicsIpcSnapshot`/`bong:qi/ledger` 使 agent/client 需要消费，则该阶段不得继续写 N/A，必须同时列出新增 schema、发布者、agent/client consumer 和契约测试；当前范围不作该扩展。
- **worldview 锚点**：docs/worldview.md §二 L30-L50 的正域/死域/负灵域与灵压语义；docs/worldview.md §十 L870-L880 的全服灵气零和与缓慢重分配。真元总量是质量流向，不是任意字段加减。
- **qi_physics 锚点**：底盘复用 qi_physics::ledger::{WorldQiAccount, QiTransfer, assert_conservation, summarize_world_qi}、qi_physics::release::qi_release_to_zone、qi_physics::constants::QI_ZONE_UNIT_CAPACITY。新增物理常数、单位换算或衰减公式必须先进入 qi_physics，本 plan 不自定义一份。

### 与既有 plan 的责任边界

- docs/finished_plans/plan-qi-physics-v1.md 已提供 ledger、snapshot、释放算子和守恒断言底盘，但明确把既有 gameplay 接线留给 patch/后续 plan；它不是本具体缺陷的修复记录。
- docs/finished_plans/plan-qi-conservation-leaks-v1.md、plan-combat-qi-invest-conservation-v1.md 和历史 qi 清扫已处理其它释放/消耗路径；未覆盖道伥凝结 owner id 与 blackboard snapshot 投影这组问题。
- docs/finished_plans/plan-daozhan-v1.md 已落地道伥行为、TiandaoCondense reason 和死亡释放的玩法接线，但本报告证明其两端 owner identity/ledger 轨迹尚未对称。
- docs/plan-refactor-qi-ledger-v1.md 是更宽的 R5 架构重构轨，拥有全仓字段私有化和 fauna/npc 批次的总体边界。本 skeleton 只立道伥的证据、owner identity 和 transaction 决策入口；Pre-P0 Decisions 已明确本计划负责 DaoZhang P1/P2 与 collapse P3，R5 负责更宽的字段私有化和其它 producer 批次，双方不得重复实现同一 owner API 或全仓私有化。
- **逐项正典预检：`docs/worldview.md`**：已复核 §二 L30-L50 的正域/死域/负灵域及灵压语义、§十 L870-L880 的全服灵气零和与缓慢重分配；这些是本 bug 的守恒依据，不把本 skeleton 当作 worldview 修改入口，worldview 不在本 PR 改动。
- **逐项待办预检：`docs/plans-skeleton/reminder.md`**：已核对 qi 相关登记；`practice_session_tick` 已归 `plan-dazuo-v1` P2，§808 inventory 转移税明确待另立且须走 qi_physics，均不拥有道伥凝结/死亡 owner 单位修复；本 skeleton 不吸收、不改写这两条 reminder。
- 已检查 docs/finished_plans/、docs/plan-*.md、docs/plans-skeleton/、历史 qi.*守恒/conservation 提交、同名远端 claim 和开放 PR；没有同 basename 或同一组道伥凝结/死亡 owner 缺陷的既有 plan，因此本 skeleton 不与已有具体 plan 重复立项。

## 守恒判据：P / L / T

统一使用绝对真元单位定义观测量：

    P = Σ player.qi_current
      + Σ (zone.spirit_qi × QI_ZONE_UNIT_CAPACITY)
      + Σ container/item qi
      + Σ 未存入 WorldQiAccount 的 external owner 余额

    L = WorldQiAccount 中稳定账户的余额总和
    T = P + L

在没有时代衰减的普通事务中，必须满足 T_after == T_before；有时代衰减时，必须满足 T_after == T_before - era_decay，并以 qi_physics::ledger::assert_conservation 的定义为准。

以下规则不可混淆：

1. zone.spirit_qi 是归一化浓度，不是绝对 qi。summarize_world_qi 必须用 QI_ZONE_UNIT_CAPACITY 转为绝对真元；不能把 qi_max、归一化 zone 字段或测试 fixture 字面量直接当成全服总量。
2. 一个 external owner 只能有一个物理权威表示。余额若由 ECS/blackboard 持有，就应通过 registry 计入 external-owner 投影且不能长期镜像到 ledger；余额若由 WorldQiAccount 稳定账户持有，就只计入 L，不能再把同一数额计入 ECS。WorldQiAccount::total() 只统计 balances，transfers 只是审计向量。
3. WorldQiAccount::transfer（server/src/qi_physics/ledger.rs:554-589）才是 balance-mutating ledger transaction。单纯 EventWriter<QiTransfer>::send 不改变 WorldQiAccount.balances，也不会由 Bevy 自动消费；本计划 Pre-P0 Decisions 明确拒绝全局 consumer，事件只能在真实状态提交后作为轨迹投影，各 producer 必须先完成自己的 typed transaction。
4. QiTransferReason::TiandaoCondense 是 BalanceMutating，不能把凝结事件事后降格成 AuditOnly。HalfStepBuff 只改 qi_max 容量并发审计事件，属于明确的 AuditOnly 设计，不得作为统一 consumer 的 balance transfer 输入。

## 已证实缺陷与物理侧限定

### 1. 凝结方向同时存在 ledger 缺口和分率/raw 单位错配

- server/src/fauna/daozhan.rs:1064-1068 计算实际扣减量；`actual_cost` 是与 `zone.spirit_qi` 同标尺的归一化分率：

      let actual_cost = TIANDAO_CONDENSE_QI_COST
          .min(zone.spirit_qi - TIANDAO_CONDENSE_THRESHOLD);

- server/src/fauna/daozhan.rs:1070 直接按分率写 `zone.spirit_qi = (zone.spirit_qi - actual_cost).clamp(-1.0, 1.0)`；对应绝对扣减应是 `actual_cost * QI_ZONE_UNIT_CAPACITY`。
- server/src/fauna/daozhan.rs:1072-1079 构造 `QiTransfer { from: zone, to: npc, amount: actual_cost, reason: TiandaoCondense }` 后只调用 `EventWriter::send`；`QiTransfer.amount` 的 raw 合同也被分率污染，system 参数没有 `WorldQiAccount`，没有 `transfer_zone_qi_to_ledger`、`transfer_external_qi_to_ledger` 或其它 ledger 提交。
- server/src/fauna/daozhan.rs:1085-1090 发出的 `SpawnDaoZhangFromCondenseRequest.condensed_qi` 是同一笔未换算的 `actual_cost`；server/src/fauna/daozhan.rs:1153 再把它写入 raw `DaoZhangBehaviorBlackboard.daozhan_qi`。这里缺失唯一的 `actual_cost * QI_ZONE_UNIT_CAPACITY` 换算入口，不能继续称作物理闭环。

### 2. 同一个 external owner 使用了两个账户 id

- 凝结 transfer 使用 daozhan:condense:<zone>:<tick> 这一按事件生成的账户 id（server/src/fauna/daozhan.rs:1072-1079）。
- 死亡释放使用 canonical_npc_id(entity)（server/src/fauna/daozhan.rs:1209-1221）读取/归还道伥实体的 blackboard 余额。
- 因而同一物理道伥从凝结到死亡没有一个贯穿生命周期的 durable owner identity；不能把凝结临时账户和死亡 canonical 账户当成同一 balance，也不能用两个账户的和掩盖重复/遗漏。

### 3. 通用 snapshot 没有投影道伥 blackboard owner

- server/src/qi_physics/ledger.rs:1013-1053 的 summarize_world_qi 统计 player、zone、inventory 和 ledger，但不查询 DaoZhangBehaviorBlackboard.daozhan_qi。
- WorldQiSnapshot 因此是通用 snapshot，不是当前道伥完整 external-owner projection；即使 spawn consumer 被挂进测试，仍需先决定 registry/投影契约，否则会漏计或双计。
- 本轮测试债现场的临时投影位于保留的旧分支 commit 99ce719a8，不是本 skeleton PR 的文件；该现场不能替代生产 snapshot contract。

### 4. 生产凝结路径不引用 TIANDAO_CONDENSE_INITIAL_QI；测试仍有 8 处引用

- `server/src/fauna/daozhan.rs:1002` 定义 `TIANDAO_CONDENSE_INITIAL_QI`；全仓实际有 9 处文本引用：该定义 1 处，加上 `server/src/fauna/daozhan_tests.rs:1709`、`:1745`、`:1750`、`:1751`、`:1758`、`:1760`、`:1761`、`:1762` 共 8 处测试引用。
- 生产凝结路径 `server/src/fauna/daozhan.rs:1064-1068` 不引用这个常量，而是用 `actual_cost = TIANDAO_CONDENSE_QI_COST.min(zone.spirit_qi - TIANDAO_CONDENSE_THRESHOLD)`；因此 `actual_cost` 在 zone 灵气接近阈值时可以小于 `TIANDAO_CONDENSE_QI_COST`。
- `server/src/fauna/daozhan_tests.rs:1741-1753` 只是手工构造 `SpawnDaoZhangFromCondenseRequest` 并在 `:1745` 写入、`:1750-1751` 断言 `condensed_qi == TIANDAO_CONDENSE_INITIAL_QI`；它没有创建 `ZoneRegistry`，也没有调用凝结 system，因此不覆盖接近阈值的 actual_cost boundary。当前 skeleton HEAD 中，`:1609-1628` 的 `spirit_qi_before = 0.90` 仅是算术测试，`:1631-1641` 才是 `threshold + 0.001` 的独立边界算术测试；二者都没有把实际 `actual_cost` 接入 request 字段。高灵气 fixture 与真实 system 调用只存在于保留的测试债现场 commit `99ce719a8cf2b3bce5601f5bfb38020497bcb3d0`（该现场的 `daozhan_tests.rs:1695-1709` 构造 App、`:1758` 使用 `0.90`、`:1763-1805` 调用 system），不能把它们归因给当前 skeleton HEAD。因而现有字段 pin 锁住的是 fixture 常量而不是生产契约，并掩盖了常量与实际语义的脱节。
- 因此删除或重命名该常量前，必须先迁移上述 8 处测试引用：把固定初始量断言改成实际 `actual_cost`/单位契约与低余量 boundary 的测试，不能按“死常量”直接删除，否则构建会失败。

### 5. 数值传递沿用同一 actual_cost，但单位不闭合，物理侧确实蒸发真元

此前“物理侧可能闭环”的判断只追到了同一个数值 `actual_cost` 被继续传递，没有追它在不同字段中的单位。当前实际链是：

    zone.spirit_qi (fraction) -= actual_cost
      → zone 绝对扣减 actual_cost × QI_ZONE_UNIT_CAPACITY
      → SpawnDaoZhangFromCondenseRequest.condensed_qi (当前误为 actual_cost raw)
      → DaoZhangBehaviorBlackboard.daozhan_qi (raw)
      → release_external_qi_to_zone(..., actual_cost, ...) (raw)
      → zone / overflow

因此在正灵气、spawn 成功、死亡释放全额可接受的普通路径，zone 少了 `actual_cost × QI_ZONE_UNIT_CAPACITY`，道伥只持有并归还 `actual_cost` raw，确定净蒸发 `actual_cost × (QI_ZONE_UNIT_CAPACITY - 1)`。`actual_cost` 数值本身没有被重复计算或凭空改变，但这不能弥补 fraction/raw 标尺错位；ledger/audit 方向不对称、owner identity 不一致、通用 snapshot 漏投影仍是并存缺陷。

本结论不替 active 阶段跳过真实 ECS 验证：spawn consumer 是否成功接管余额、zone 接近上限时的 accepted/overflow 分叉、缺 zone、重复 request、重启恢复和完整 snapshot owner projection 都必须用真实链路单独验收。这些条件决定余额落点和投影是否完整，不会把已证实的单位损失改写成“测试 artifact”。

### 6. 坍缩渊再分配存在独立的同类单位错误（P3 单独收口）

- server/src/world/events.rs:2004 的 `stored_qi = source.spirit_qi.max(0.0)` 仍是 zone fraction；`collapse_redistribute_qi` 返回的 `amount` 也沿 zone fraction 语义用于 :2049-2052 的邻接 zone 写回。
- 但无邻接 zone 的 :2026-2030，以及邻接 zone 满载的 :2058-2062，直接把 `stored_qi` / `overflow` fraction 作为 `QiTransfer::new(..., amount, ...)` 的 amount。该字段及 overflow stable account 需要 raw，缺失 `× QI_ZONE_UNIT_CAPACITY` 换算，传输低估因子为 `QI_ZONE_UNIT_CAPACITY`；漏记量是 `fraction × (QI_ZONE_UNIT_CAPACITY - 1)`。
- 当前 `QI_ZONE_UNIT_CAPACITY = 50` 时，若 collapse fraction 为 `0.05`，事件/overflow 只带 `0.05` raw，实际应带 `2.50` raw，单笔漏记 `2.45` raw。它不是道伥凝结缺陷的附带一句，也不能被“event-only / 后续核验”标签掩盖；P0 必须单独决定 owner/事务归属，P3 作为独立迁移或明确 follow-up 收口。

## 红测证据（保留原始条件，不为变绿而放宽）

以下红测来自保留的测试债分支 `fix/daozhan-self-referential-conservation-tests`、commit `99ce719a8cf2b3bce5601f5bfb38020497bcb3d0`，不是当前 skeleton HEAD；该现场的测试确实真实调用了 daozhan_tiandao_condense_system。过滤命令：

    scripts/build-token.sh cargo test fauna::daozhan::tests::tiandao_condense_conservation_zone_decreases_by_cost --lib

原始关键输出：

    running 1 test
    FAILED
    expected: 45.0
    actual: 42.5
    drift: 2.5
    tolerance: 1e-6

完整 snapshot 证据为：

    before=WorldQiSnapshot {
        player_qi: -0.0,
        zone_qi: 45.0,
        container_qi: -0.0,
        ledger_qi: 0.0,
        era_decay_accum: 0.0,
        budget_initial_total: 100.0,
        budget_current_total: 100.0,
    }
    after=WorldQiSnapshot {
        player_qi: -0.0,
        zone_qi: 42.5,
        container_qi: -0.0,
        ledger_qi: 0.0,
        era_decay_accum: 0.0,
        budget_initial_total: 100.0,
        budget_current_total: 100.0,
    }
    error=ConservationDrift {
        expected: 45.0,
        actual: 42.5,
        tolerance: 1e-6,
    }

该旧现场 fixture 的 spirit_qi_before = 0.90，凝结 system 发出一个 request，condensed_qi = actual_cost = 0.05，zone 归一化字段减少 0.05。snapshot 按 QI_ZONE_UNIT_CAPACITY = 50 观察到 zone 绝对量减少 2.5，所以 before.total_observed - after.total_observed = 45.0 - 42.5 = 2.5，era_decay = 0；当前 skeleton HEAD 的同名测试仍只是算术测试，不能把这段红测描述成当前 HEAD 已执行的 ECS 调用。

这段 `drift: 2.5` 的单位成因必须单独写清：`actual_cost × (QI_ZONE_UNIT_CAPACITY - 1) + actual_cost = 0.05 × 49 + 0.05 = 2.5`。前一项是完整凝结→死亡链中确定的真实单位损失，后一项对应旧测试 App 未挂 spawn consumer、且 snapshot 未投影 blackboard owner 时未被观察到的 `actual_cost`；“没有 consumer”只能解释观测到的分解，不能解释掉前一项，也不能把 2.5 整体归为测试脚手架 artifact。挂上 consumer 后，道伥仍只会获得/归还 `0.05` raw，当前单位错配仍留下 `2.45` raw 蒸发。

必须同时保留以下限定条件：

- 测试 App 只挂了凝结 system，**没有挂 spawn consumer**，因此测试时没有对应道伥 external owner 余额，也没有 ledger balance 变化。
- 即便把 spawn consumer 挂上，当前 summarize_world_qi 仍不投影 DaoZhangBehaviorBlackboard.daozhan_qi；通用 snapshot 仍不能单独证明完整道伥 owner 链守恒。
- 这条红测结合单位证据，证明当前真实凝结入口会在完整正向链留下真元蒸发；但它不单独覆盖 spawn consumer、overflow、接近 zone 上限、重启和完整 owner projection 的每个分支。禁止通过放宽 tolerance、只比较 zone、退回局部算术或把测试 App 改成自指投影来“修绿”。
- 生产 daozhan.rs 没有 assert_conservation 调用；全仓没有生产 EventReader<QiTransfer> consumer。事件发送本身不会应用到 WorldQiAccount。
- WorldQiAccount::transfers（审计轨迹）与 ledger_qi（稳定余额）是不同观测：死亡 zone accepted 路径可增加 audit 而不增加 ledger_qi，overflow 路径才把稳定 overflow balance 纳入 ledger。验收必须分别断言事件轨迹和余额总量。

## 全仓 EventWriter<QiTransfer> 影响面

以下表格来自对 server/src 直接声明 EventWriter<...QiTransfer> 的全量扫描；qi_physics/ledger.rs 的命中只有文档/测试注释，不是生产 writer。分类含义：canonical = 调用带 WorldQiAccount 的 typed ledger helper/事务；event-only = 物理字段改变后只发事件或只用低层 qi_release_to_zone，没有在该路径提交 WorldQiAccount；mixed = 两种路径并存；AuditOnly = 只改变容量/状态而非真元 balance，不能误当 transfer。新增的“单位核验”是正交维度：`zone.spirit_qi` 只表示 fraction，raw 字段/`QiTransfer.amount` 必须在唯一边界乘 `QI_ZONE_UNIT_CAPACITY`；每一行都单独记录“已换算 / 不适用 / 发现 mismatch”。

| 文件 | 证据与账本判定 | 单位核验（fraction → raw） |
|---|---|---|
| server/src/fauna/daozhan.rs | 凝结 :1064-1079 手写扣 zone + 裸发 TiandaoCondense；死亡 :1209-1221 走 release_external_qi_to_zone，zone accepted 写 audit、overflow 真实入稳定 ledger。**mixed，且凝结方向不对称。** | **发现 mismatch**：`actual_cost`/zone debit 是 fraction，QiTransfer、request、blackboard、release requested 是 raw；缺少 `actual_cost × QI_ZONE_UNIT_CAPACITY`，净损失为 `actual_cost × (CAP - 1)`。 |
| server/src/combat/carrier.rs | charge_carrier_tick :550-560 手写扣 cultivation.qi_current，emit_carrier_channeling_transfer :692-708 裸发事件；残余释放 :1468-1480 调低层 qi_release_to_zone 并发事件，release_account_to_zone 没有 WorldQiAccount 参数。**event-only / 后续治理。** | **已换算**：charge/residual amount 以 actor raw qi 进入 helper；zone↔raw 由 qi_release_to_zone 的 `QI_ZONE_UNIT_CAPACITY` 边界处理；账本缺口仍独立存在。 |
| server/src/combat/lifecycle.rs | revive staging :1782-1789 调 staged_cultivation.release_to_zone，进入 release_external_qi_to_zone；提交后的 transfers 在 :1916-1918 发出。**canonical，事件是事务结果投影。** | **已换算**：staged Cultivation 是 raw，release_external_qi_to_zone 将 zone fraction 转 raw 后再调用释放算子。 |
| server/src/combat/needle.rs | 发射 :136-169 手写扣 qi_current 并发 Channeling；过期释放 :343-355 调低层 qi_release_to_zone，overflow :382-395 仍裸发。**event-only，物理扣减/释放路径没有 WorldQiAccount 提交。** | **已换算**：发射/释放量来自 raw `qi_current`，低层 helper 以 `zone.spirit_qi × QI_ZONE_UNIT_CAPACITY` 计算 room；未发现 fraction 直接写 raw 的单位错配。 |
| server/src/combat/rat_bite.rs | :104-112 调 cultivation.transfer_to_external_actor，实现 server/src/cultivation/components/qi_flow.rs:441-486 在修改 source 前调用 transfer_external_qi_to_ledger（:478-484）；:125-128 发已提交结果并从 ledger 读取 rat 余额。**canonical real ledger。** | **不适用/已隔离**：该行是 actor raw→external actor raw 的 typed transfer，不直接读写 `zone.spirit_qi`；没有 fraction→raw 跨界。 |
| server/src/combat/woliu.rs | projectile transfer :969-996；maintenance :629-645 手写扣 player，再由 :1016-1032 使用低层 qi_release_to_zone 并发事件。**event-only / 低层 helper。** | **已换算**：player/释放 amount 是 raw，zone 计算交给低层 helper 的 capacity 换算；ledger ownership 仍待治理。 |
| server/src/combat/woliu_v2/tick.rs | :131-144 低层 qi_release_to_zone 后发事件；overflow :175-206 用 set_balance + push_transfer_audit 手工入账再发事件。**mixed：overflow 有真实余额写入但非统一 transfer 事务，zone accepted 依赖低层事件投影。** | **已换算**：释放输入和 overflow balance 是 raw，zone room 由 helper 按 capacity 换算；mixed 的事务一致性不等于单位核验通过。 |
| server/src/combat/zhenmai_v2.rs | multipoint_duration_tick :998-1012、harden_duration_tick :1044-1058 手写扣 player，再经 drain_release_to_zone :1248-1264 调低层 qi_release_to_zone 发事件。**event-only / 低层 helper。** | **已换算**：player drain 是 raw，`drain_release_to_zone`/qi_release_to_zone 负责 zone fraction↔raw；未发现本行另有直接 fraction→raw 写入。 |
| server/src/cultivation/death_hooks.rs | revive/terminate :187-218、:292-311 调 release_cultivation_qi_to_zone；公共 helper :349-388 校验 identity 并调用 cultivation.release_to_zone。**canonical。** | **已换算**：Cultivation release 是 raw，公共 release helper 在 zone 边界使用 `QI_ZONE_UNIT_CAPACITY`；source/zone/overflow 走同一事务。 |
| server/src/cultivation/full_power_strike.rs | charge_tick_system :306-331 手写扣 player；释放 helper :350-401 调低层 qi_release_to_zone，:461-463 发事件。**event-only / 低层 helper。** | **已换算**：:359-370 明确 `zone.spirit_qi × QI_ZONE_UNIT_CAPACITY` 进 helper、结果再除回 fraction；player amount 保持 raw。 |
| server/src/cultivation/tribulation.rs | HalfStepBuff :2200-2212 只改变 qi_max 容量并发 HalfStepBuff；server/src/qi_physics/ledger.rs:346-386 明确该 reason 为 AuditOnly，不是 balance transfer。同文件 :3047-3066 从 gain_from_zone 发事务结果，属 **canonical**。 | **不适用（AuditOnly 分支）/已换算（gain_from_zone 分支）**：容量审计不涉及 qi balance；实际吸收由 canonical helper 处理 fraction↔raw。 |
| server/src/dandao/boss_spawn.rs | :395-405 使用 cultivation.release_to_zone(..., &mut qi_account, ...) 后发 outcome。**canonical。** | **已换算**：release 输入是 raw，canonical release helper 按 capacity 计算 zone accepted/overflow。 |
| server/src/fauna/dying_elder.rs | 给丹 overflow :712-728、rift drain :1159-1178 用 transfer_external_qi_to_ledger；elder 接收/夺取 :745-759、:962-981 用 push_transfer_audit + event；死亡 zone/overflow :1287-1356 组合低层释放与 overflow ledger。**mixed，但有明确 ledger 提交，需逐 external owner 审计。** | **已换算**：external owner/overflow 余额按 raw，zone release 由 helper 做 capacity 换算；需继续核对每个 external owner 的唯一物理表示。 |
| server/src/fauna/hybrid_beast.rs | 融合 :441-474 手写组件兽→hybrid event 和 zone 回写；rage :776-798 手写 zone 减、cultivation.qi_current += gain 后裸发 event，无对应 WorldQiAccount 事务参数。**event-only / 高风险。** | **已换算**：`regen_from_zone` 返回 `gain=raw`、`drain=fraction`；rage 分别写 raw actor 和 fraction zone，fusion 释放明确除以 capacity；账本提交仍不完整。 |
| server/src/lingtian/systems.rs | :1592-1619 手写 plot 清零、player credit、ZoneQiAccount 回写，:1652-1710 组装/发送事件；使用独立 ZoneQiAccount，不是 WorldQiAccount。**不属于 canonical WorldQiAccount 路径。** | **不适用/隔离**：`plot_qi` 与 `ZoneQiAccount` 是独立灵田账本，不能直接套用 `Zone.spirit_qi` 的 raw/fraction 结论；其跨账本契约另行核验。 |
| server/src/network/command_executor.rs | pseudo-vein spawn :1021-1028 的 transfer 来自 inject_zone_for_pseudo_vein；真实注入在 server/src/world/pseudo_vein_runtime.rs:498-542 使用 transfer_ledger_qi_to_zone。**forwarder，本身不是裸物理扣减；canonical。** | **已换算**：forwarder 不改余额，真实 helper 负责 ledger raw→zone fraction 的 capacity 换算。 |
| server/src/world/events.rs | flush_collapse_qi_transfers :1817-1827 只把 ActiveEventsResource 队列 flush 到 Bevy event；collapse redistribution :1995-2070 直接改 zone、把 overflow 放进 Vec<QiTransfer>，没有向该函数传入 WorldQiAccount。**event-only queue / 独立高危缺陷。** | **发现 mismatch**：:2004/`:2044` 的 `stored_qi`/`amount` 是 fraction，:2026-2030、`:2058-2062` 却原样作为 raw `QiTransfer.amount`；CAP=50 时 `0.05` 只记 `0.05` raw，应记 `2.50`，漏记 `49×0.05=2.45` raw。 |
| server/src/world/heartbeat.rs | dynamic pseudo-vein :1378-1392、:1412-1427 调带 ledger 的 settlement，再发送返回 transfer；spawn omen 也把带 ledger 的 helper 结果发出 :1507-1518。**canonical。** | **已换算**：settlement helper 统一处理 zone fraction 与 ledger raw，heartbeat 只投影已提交结果。 |
| server/src/world/pseudo_vein_runtime.rs | 注入 :498-542 使用 transfer_ledger_qi_to_zone；结算 :564-585、:592-620 使用 transfer_zone_qi_to_ledger。**canonical。** | **已换算**：两个 typed helper 是明确的 ledger raw↔zone fraction 换算边界。 |
| server/src/world/tsy_lifecycle.rs | family teardown :661-685 对 Cultivation/道伥 staged owner 调 release_to_zone/release_external_qi_to_zone，:716-720 发送已提交 staged ledger transfers。**canonical staged transaction。** | **已换算**：staged owner amount 是 raw，release helper 负责 zone fraction 转 raw 及 accepted/overflow；本行不复用道伥凝结的错误 request。 |
| server/src/zhenfa/mod.rs | scatter bead :2383-2439 先低层计算后实际调用 ledger.transfer，属显式 ledger transaction；sealed trap release :4388-4415 只低层改 zone、发 event，overflow :4443-4459 也只构造事件。**mixed，trap release event-only。** | **已换算**：scatter/sealed release 在 :4387-4396 以 `zone.spirit_qi × QI_ZONE_UNIT_CAPACITY` 进 helper、结果除回 fraction；trap 的 ledger/event 缺口仍独立待治理。 |

单位列与 canonical/event-only 列不能互相替代：本轮确认的单位错配至少包括道伥凝结和 `world/events.rs` collapse 两处；其余 event-only/mixed 行即使单位边界已换算，也仍需按 owner、reason disposition 和真实 ledger 提交继续治理，不能因“已换算”就宣称守恒闭环。

## Pre-P0 Decisions（2026-09-13）

1. **collapse redistribution 单位缺陷归属**：经 `server/src/world/events.rs:2004-2070` 代码核查，zone fraction→raw overflow 转换、该分支的 typed transaction 接入和真实回归验收纳入本计划 **§P3，由本计划唯一负责**；`plan-refactor-qi-ledger-v1` 的 P3 不重复实现这一 collapse 分支。R5 仍负责其更宽的字段私有化和其它 producer 批次，接入时必须复用本计划收口的单位/owner contract。

2. **事务架构与 consumer 边界**：选定“专用 canonical typed transaction + external-owner registry + deferred request reservation”的组合；request 只携带可恢复的 `QiTransactionId`/reservation，登记同一 durable owner 后才提交 zone debit、owner credit/debit 和 stable overflow。明确拒绝全局 `QiTransfer` consumer：现有 producer 已先改变 ECS/zone，事后遍历事件会双扣/双记；`QiTransfer` 只在真实提交后作为幂等审计投影。

3. **唯一 owner、transaction id 与执行顺序**：live `Cultivation`、signed `Zone`、live DaoZhang blackboard/registry entry 和 `WorldQiAccount` stable overflow 各自只有一个物理权威，不长期互相镜像；`QiTransactionId = producer + source_identity + target_identity + game_tick + operation_ordinal`，跨 retry/restart 保持不变。所有 producer 统一按“单位/identity/reason/容量 preflight → 建 reservation（无 balance 写入）→ 一次性提交 source 与 target/overflow → 提交成功后 emit/push audit → 以 transaction id 幂等完成 request”执行，任一步 preflight/commit 失败均零写入。

4. **producer 边界矩阵（P0 前冻结）**：下表把影响面中的每一行归入唯一物理 owner、reason disposition 和 transaction/order contract；它冻结的是责任与顺序，实际迁移仍按 P1/P2/P3 交付物执行。

| producer 类别（覆盖文件） | 唯一物理 owner / target | reason disposition、transaction id 与顺序 |
|---|---|---|
| 道伥凝结、spawn、死亡（`fauna/daozhan.rs`） | zone fraction 由 `Zone` 持有；live 道伥余额由 canonical NPC registry/blackboard 持有；stable overflow 才进 `WorldQiAccount` | `TiandaoCondense` 与 `ReleaseToZone` 均为 **BalanceMutating**；`condense:<zone>:<tick>:<canonical_npc_id>` 作为 reservation/transaction，不作为第二账户；统一 preflight→owner 登记→fraction→raw→提交→audit。 |
| actor↔external owner（`combat/rat_bite.rs`、`fauna/dying_elder.rs`，以及 `fauna/daozhan.rs` 伏击） | source 是唯一 `Cultivation`/external owner，target 是 registry 绑定的 canonical actor/NPC owner；不把同一余额镜像进 ledger | `RatBiteDrain`、`DaoZhangDrain`、`TradeDan`/`SoulSeize` 按各自 producer 作为 **BalanceMutating**；id 使用 source/target canonical identity + tick + ordinal；source debit 与 target credit 同一 typed transaction 后才审计。 |
| actor/external owner→zone（`combat/lifecycle.rs`、`cultivation/death_hooks.rs`、`dandao/boss_spawn.rs`、`world/tsy_lifecycle.rs`、`combat/carrier.rs`、`combat/needle.rs`、`combat/woliu.rs`、`combat/woliu_v2/tick.rs`、`combat/zhenmai_v2.rs`、`cultivation/full_power_strike.rs`、`zhenfa/mod.rs`） | source live/staged actor 或 external owner 是唯一余额；target 是解析出的 signed zone，不能接收的 raw 余额只进固定 overflow | `ReleaseToZone`/`Channeling` 等 producer reason 是 **BalanceMutating**；低层 helper/typed release 先完成 raw source→zone fraction/overflow，再发送事件，禁止另加 consumer；id 由 source+zone+tick+operation 绑定，canonical 与 event-only 分支分别迁移但共用该顺序。 |
| zone→actor / zone↔ledger（`fauna/hybrid_beast.rs`、`network/command_executor.rs`、`world/heartbeat.rs`、`world/pseudo_vein_runtime.rs`） | source zone 或 stable ledger 是唯一来源；target live actor、hybrid 或 stable account 各自只保留一份 raw/fraction 权威 | `CultivationRegen`、`FusionMerge`、`ZoneInflow` 等为 **BalanceMutating**；zone fraction↔raw 只在 typed helper 边界转换，source debit 与 target credit 完成后才投影事件，id 绑定双方 canonical identity。 |
| collapse redistribution（`world/events.rs:1995-2070`） | source `Zone.spirit_qi`/operator 使用 fraction；neighbor zone 接收 fraction，no-neighbor/满载余量的固定 overflow 接收 raw | `RiftCollapse` 为 **BalanceMutating**，由本计划 §P3 唯一负责；operator 先按 fraction 计算 accepted，再把 overflow `× QI_ZONE_UNIT_CAPACITY` 作为 raw 提交，id 绑定 source/target/tick/ordinal，不能由全局 consumer 补账。 |
| 容量审计（`cultivation/tribulation.rs` HalfStepBuff 分支） | 不产生 qi balance owner；`qi_max`/capacity metadata 是唯一状态 | `HalfStepBuff` 是 **AuditOnly**，transaction 只记录容量变化；不调用 `WorldQiAccount::transfer`，不改变 `T`，也不进入 BalanceMutating consumer。 |
| 独立灵田账本（`lingtian/systems.rs`） | `plot_qi`/`ZoneQiAccount` 是该路径的唯一专用 owner，不冒充 `WorldQiAccount` 或普通 `Zone.spirit_qi` | 由灵田自身 transaction/audit contract 管理；对本 plan 的 `WorldQiAccount` reason matrix 为 **N/A/隔离**，不得把它并入通用 consumer 或复用本 bug 的 zone raw/fraction 结论。 |

5. **snapshot 与跨 plan 责任**：DaoZhang registry entry 和固定 overflow 的投影规则由本 plan P2 收口；`summarize_world_qi` 不重复计入 live owner。`plan-refactor-qi-ledger-v1` P3 负责更宽的字段私有化及其 producer 批次，但不得重复实现本计划的 DaoZhang P1/P2 或 collapse P3；其余影响面迁移必须复用上表 contract 并在 R5 PR 标明 owner，不得另造 consumer/ledger。

以下开放问题保留供转 active 时追溯；本节已收口的 owner、reason、transaction/order 和 collapse 归属不得在 P0 实施阶段重新二选一。

1. **actual_cost 的单位与唯一换算点（本轮证据已收口，P0 仍须冻结 API）**：代码证据表明它是归一化 `zone.spirit_qi` fraction，不是 raw qi；active P0 必须把 `actual_cost × QI_ZONE_UNIT_CAPACITY` 固定为唯一 raw 换算，明确 zone debit、condensed_qi、DaoZhangBehaviorBlackboard.daozhan_qi、WorldQiAccount、QiTransfer.amount、release requested 和 snapshot 的标尺。
2. **durable entity id 与扣款顺序（已收口）**：先建立可恢复 reservation 与 canonical NPC owner id，再由 typed transaction 原子提交 debit/credit；duplicate、restart、spawn failure 通过同一 transaction id 幂等重放或释放 reservation，失败不改 zone/owner/ledger。
3. **external owner registry（已收口）**：live DaoZhang blackboard 余额由 canonical registry 绑定并作为唯一物理 owner；stable overflow 才进入持久化 `WorldQiAccount`，despawn/death 走 terminal release，重启先 hydrate registry 再恢复 request，不把 store.remove 当释放。
4. **统一 consumer（已收口）**：不新增全局 consumer；每个 producer 按上方矩阵使用 typed transaction，事件仅为提交后审计，AuditOnly reason 排除在余额事务外。
5. **summarize_world_qi 投影（已收口）**：P2 从 registry 投影未镜像的 live external owner，stable ledger 只计固定账户；`WorldQiAccount::total()`、`ledger_qi`、audit transfers、`WorldQiBudget.current_total` 继续分开观察，不重复计同一 owner。
6. **与 plan-refactor-qi-ledger-v1 交接（已收口）**：本计划唯一负责 DaoZhang 凝结/死亡 P1/P2 与 collapse P3；R5 P3 负责更宽字段私有化/其它 producer 批次并复用本矩阵，不同时改变同一 owner API 或新建 registry。
7. **collapse redistribution 的单位缺陷（已收口）**：`world/events.rs` 的 zone fraction→raw overflow 转换由本计划 §P3 唯一负责，R5 P3 不重复实现；不得与道伥 P1 的缺陷合并成一条泛化 event-only 待办。

原始开放问题保留以便追溯；以上 Pre-P0 Decisions 必须原样带入 active plan 的 §N.1，P0 只能执行这些已决策内容，且每条决议必须落到真实 file:line 与 plan 章节双锚点。

## P0 — 守恒合同与责任收口

**可核验交付物：**

- 冻结 P/L/T、WorldQiBudget、QI_ZONE_UNIT_CAPACITY、signed zone、external owner 与 stable ledger account 的单位和唯一性合同；明确 WorldQiAccount::transfers 仅是审计，不是余额。
- 按 `Pre-P0 Decisions（2026-09-13）` 冻结两个单位缺陷的执行边界：道伥 `actual_cost` 必须在唯一入口从 zone fraction 换成 raw；`world/events.rs:2004-2070` 的 collapse operator 可继续以 fraction 处理 zone，但所有 `QiTransfer.amount`/overflow balance 必须显式换成 raw；两者都要写出换算前后量级和失败零写入语义，不得再把 collapse 归属留作 P0 实施时的二选一。
- 对 QiTransferReason::{TiandaoCondense,ReleaseToZone,HalfStepBuff} 写出 disposition 表：哪些是 BalanceMutating，哪些是 AuditOnly，每个 reason 的唯一 source/target owner capability、transaction id、失败语义和 audit 顺序。
- 以 server/src/fauna/daozhan.rs::daozhan_tiandao_condense_system、SpawnDaoZhangFromCondenseRequest、DaoZhangBehaviorBlackboard、release_external_qi_to_zone 和 summarize_world_qi 为接入清单；不能用“统一 consumer”一句话替代 owner/transaction 设计。
- 建立 TIANDAO_CONDENSE_INITIAL_QI 的迁移清单：定义处 server/src/fauna/daozhan.rs:1002 加上 server/src/fauna/daozhan_tests.rs:1709,1745,1750,1751,1758,1760,1761,1762；删除/重命名的前置交付物必须是 8 处测试引用已迁移并由 actual_cost boundary 契约替代。
- 将 `Pre-P0 Decisions（2026-09-13）` 的 owner、reason、transaction/order 与跨 plan 归属原样带入 active plan 的 §N.1；不得重新引入全局 consumer 或把 collapse P3 交给 R5 重复实现。

**测试声明：** 在不改生产代码的骨架阶段不新增测试；active P0 必须增加 qi_physics/transaction fixture，覆盖单位换算、same-account、insufficient、destination overflow、owner identity、AuditOnly 排除、失败零写入和 assert_conservation 的 era_decay 分支。

## P1 — 道伥凝结/死亡 canonical transaction

**可核验交付物：**

- server/src/fauna/daozhan.rs 的凝结 system 不再以裸 zone.spirit_qi 写入充当事务；`actual_cost` 明确是 zone fraction，并在唯一换算入口产生 `condensed_raw_qi = actual_cost * QI_ZONE_UNIT_CAPACITY`。只有这个 raw 值可进入 `SpawnDaoZhangFromCondenseRequest.condensed_qi`、`DaoZhangBehaviorBlackboard.daozhan_qi`、`QiTransfer.amount`、external owner/WorldQiAccount credit 和死亡 `release_external_qi_to_zone` 的 `requested`；`zone.spirit_qi` 的扣减/回写仍只用 fraction。
- spawn consumer 对 DaoZhangBehaviorBlackboard.daozhan_qi 的写入必须与 canonical owner registry 一致；死亡 server/src/fauna/daozhan.rs:1192-1221 必须从同一 owner 读取，并沿 release_external_qi_to_zone 走 zone accepted/overflow 的真实 ledger 边界。
- 所有可失败步骤（阈值/冷却/数量、spawn、owner registration、zone debit、ledger/audit）定义 preflight、rollback、重复 request 和 restart recovery；不能留下 daozhan:condense:<zone>:<tick> 与 canonical_npc_id(entity) 两个长期账户。
- 在删除或重命名 TIANDAO_CONDENSE_INITIAL_QI 前，先迁移 server/src/fauna/daozhan_tests.rs:1709,1745,1750,1751,1758,1760,1761,1762 八处引用；spawn_request_event_fields_accessible 必须改测实际 actual_cost/单位语义和接近阈值的边界，不能继续用高灵气 fixture 锁定固定常量。
- P1 真实 ECS 验收使用 `zone.spirit_qi = TIANDAO_CONDENSE_THRESHOLD + 0.001` 的贴阈值 fixture，使 `actual_cost = min(TIANDAO_CONDENSE_QI_COST, 0.001)`；断言 zone absolute、owner raw 余额、死亡回灌 raw、accepted/overflow 与 `T` 全部按同一 `QI_ZONE_UNIT_CAPACITY` 口径闭合。

**测试声明：** fauna::daozhan 真实 App 路径必须覆盖阈值 < / == / >、actual_cost 低于请求量的 boundary、zero/negative/overflow zone、spawn failure/no consumer、duplicate transaction、同一 owner 跨凝结→spawn→死亡、zone accepted 与 stable overflow、审计顺序和 T 严格守恒。测试断言取 SPIRIT_QI_TOTAL/QI_ZONE_UNIT_CAPACITY/QI_EPSILON 等 canonical 引用，不写全服总量字面量。

## P2 — external-owner registry 与 snapshot/持久化

**可核验交付物：**

- 在 server/src/qi_physics/ledger.rs 或经 P0 决议指定的模块定义 typed external owner registry；明确 DaoZhangBehaviorBlackboard.daozhan_qi 与 WorldQiAccount 哪一方是唯一物理 owner，另一路只能是只读 projection。
- 扩展 summarize_world_qi/WorldQiSnapshot 的 external-owner 投影和去重规则；WorldQiAccount::total()、ledger_qi、audit transfers、WorldQiBudget.current_total 四种观测不得混用。
- 若 owner 跨 tick、despawn、离屏或重启仍存在，接入 persistence 的 encode/decode/hydrate、stable id、缺行/非法值 fail-closed 和 unregister/terminal release；不得把 store.remove 当作释放真元。
- 核对 P2 的发布边界：当前 summarize_world_qi 经 publish_qi_ledger_to_redis 发布到 server/src/schema/channels.rs:91-93 的 bong:qi/ledger，不进入 publish_world_state_to_redis；若 P2 不新增 agent/client wire，则以该事实完成 server telemetry 验收，agent/client 仍为 N/A。若改为 bong:world_state 或新增消费方，必须在本阶段改列真实跨仓 symbol 和测试。

**测试声明：** snapshot 需覆盖 blackboard-only、ledger-only、合法转换中间态、zone accepted、overflow、缺失/重复 registry、despawn/death/restart hydrate、same owner 不双计和 unrelated owner 隔离；每个状态转换至少有专属用例，并对 assert_conservation(before, after, era_decay) 做外部可观察断言。

## P3 — 影响面治理与不双扣迁移

**可核验交付物：**

- 对上表每个 EventWriter<QiTransfer> 文件完成逐条责任记录：canonical producer、event-only 真实物理 owner、mixed 分支、AuditOnly 分支和后续 owner/plan；不能把全表机械改成统一 consumer。
- 以 Pre-P0 producer 矩阵作为唯一责任基线：本计划 P3 实际迁移 `world/events.rs` collapse fraction/raw 分支；carrier.rs、needle.rs、woliu.rs、hybrid_beast.rs、zhenfa/mod.rs 等其它 event-only/mixed producer 的字段私有化与统一 API 迁移归 `plan-refactor-qi-ledger-v1` P3，必须在 R5 PR 复用本矩阵并禁止重复接管。
- `world/events.rs:2004-2070` 的 collapse fraction/raw mismatch 作为本计划唯一负责的独立 P3 条目处理：邻接 zone 继续接收 fraction，overflow/no-neighbor 的 `QiTransfer.amount` 必须按 `× QI_ZONE_UNIT_CAPACITY` 进入 raw 账户；不能因它同时是 event-only queue 就只写“后续核验”，也不能把它冒充道伥 P1 已修或交给 R5 P3 重复实现。
- 对所有纳入迁移的 producer 使用同一 typed transaction API；保持已有物理字段语义，不因补 ledger 而再扣一次 zone/player，也不把 HalfStepBuff 等 AuditOnly 误转为 balance mutation。

**测试声明：** 每种分类至少有一条真实 producer→owner→zone/ledger 的契约测试；覆盖 pre-existing canonical path 不双扣、event-only 路径的 owner 归属、AuditOnly 不改变 T、mixed overflow/zone accepted 分叉、invalid owner/duplicate event fail-closed。测试断言 payload、余额、zone、audit 和 snapshot，不断言内部调用次数。

## P4 — 运行链路与验收关闭

**可核验交付物：**

- 真实 Update/Bevy system 顺序覆盖凝结、spawn、道伥伏击累积、死亡释放、overflow 和 snapshot；不以只挂一个 system 的 test app 冒充完整链路。
- collapse redistribution 的无邻接/邻接满载分支必须另有真实回归，证明 fraction zone 写回与 raw overflow 记账分别守恒；该证据与道伥凝结→死亡证据分开列出。
- assert_conservation 进入相应 server integration/e2e 场景；如 bong:world_state 或既有 qi telemetry 发布 snapshot，需核对 owner projection、ledger balance、audit sequence 和 era_decay_accum 的含义一致。
- server 端完成 fmt --check、clippy --all-targets -- -D warnings、cargo test；若 P3 触及 agent/client schema 或 IPC，再分别补对应栈的契约/sample/e2e 测试。纯 server 修复不强行新增跨仓 wire。

**验收声明：** 红测不得通过放宽 tolerance 变绿；必须有“物理 owner 总量”“稳定 ledger 总量”“audit 轨迹”“snapshot 投影”四个层面的可解释证据，并能说明任何 event-only/AuditOnly 路径为何不属于本次吞真元结论。

## 证据与后续测试债边界

- 测试债现场已在旧分支 fix/daozhan-self-referential-conservation-tests 以 commit 99ce719a8 保留；该现场不进入本 skeleton PR。后续回到旧分支处理 tiandao_condense_conservation_zone_decreases_by_cost 时，按本 skeleton 的 snapshot/owner 决策改为诚实命名、缺口注释和指向本 plan，不能放宽断言。
- 本 skeleton 只记录事实、判据、影响面和决策门，不修改 server/src/**、agent、client、worldview 或 library；实现阶段仍须以最新 main 重新核对所有 file:line。

## §10 实施工作流（升 active 后适用）

本 plan 涉及 owner contract、道伥事务、snapshot/persistence 和影响面治理，若拆为多个 PR，仍由同一 plan 按依赖顺序串行消费：

1. PR-1 收口 P0 与 qi_physics/API contract；PR-2 落地 P1 道伥 canonical transaction；PR-3 落地 P2 registry/snapshot/persistence；PR-4 仅处理 P0 明确纳入的 P3 producer 迁移与 P4 integration。若 plan-refactor-qi-ledger-v1 吸收其中阶段，必须在 active 决议中删去重复 PR，不并行改同一 owner API。
2. 每个 PR 必须使用独立实施 subagent、真实目标 worktree/HEAD 和无上下文 read-only validator；validator 必须核对 git rev-parse HEAD，对每条 file:line 自己开文件验真，PASS/FAIL 绑定 SHA，出结论即关闭。
3. 任一 HEAD 变化（返工或 merge main）都重新 validator；按受影响栈跑完整门禁。PIPESTATUS 必须保留真实退出码，失败不能归咎为 pre-existing。
4. 每个 PR 等当前 HEAD 的自动 review/e2e；review 返工沿原分支重新进驻 slot，合入前不把“event 已发出”当作“ledger 已提交”。
5. 全部阶段完成后才填写 ## Finish Evidence 并由归档流程将 active plan 移入 docs/finished_plans/；骨架不直接升级 active，也不在本 PR 归档其它 plan。

## Finish Evidence

> 骨架阶段未填写。升 active、所有阶段完成并准备归档时，必须补齐：各阶段真实文件/函数落点、关键 commit（hash + 日期 + 一句话）、运行过的测试命令与数量、server/agent/client 命中的跨仓 symbol，以及未纳入本 plan 的遗留/后续事项。
