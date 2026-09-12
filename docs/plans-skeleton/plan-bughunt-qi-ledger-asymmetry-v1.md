# plan-bughunt-qi-ledger-asymmetry-v1：道伥凝结与死亡释放的 qi 账本不对称

> **骨架（草案）**。一句话主题：修复道伥 TiandaoCondense 凝结入口与死亡 release_external_qi_to_zone 归还入口之间的账本、external-owner 身份和 snapshot 投影不对称；本 bughunt 不把有条件的红测夸大为“物理侧必然吞真元”，也不在骨架阶段拍板 A/B/C/D 实施路线。

## 阶段总览

| 阶段 | 交付物 | 状态 | 验收日期 |
|---|---|---|---|
| P0 | P/L/T 守恒口径、owner 唯一性、单位/事务/失败边界和既有 plan 责任收口 | ⬜ | YYYY-MM-DD（待定） |
| P1 | 道伥凝结→spawn→死亡归还的 canonical transaction 与稳定 owner identity | ⬜ | YYYY-MM-DD（待定） |
| P2 | external-owner registry、summarize_world_qi 投影和持久化/重启生命周期 | ⬜ | YYYY-MM-DD（待定） |
| P3 | QiTransfer 影响面逐条归类、选定路径迁移和不双扣回归 | ⬜ | YYYY-MM-DD（待定） |
| P4 | 真实运行链路、snapshot/ledger 审计和守恒集成验收 | ⬜ | YYYY-MM-DD（待定） |

## 接入面与范围边界

- **进料**：server/src/fauna/daozhan.rs::daozhan_tiandao_condense_system 从高灵气 ZoneRegistry 读取 Zone.spirit_qi，计算 actual_cost，发出 SpawnDaoZhangFromCondenseRequest 和 QiTransferReason::TiandaoCondense；spawn consumer 将 condensed_qi 写入 DaoZhangBehaviorBlackboard.daozhan_qi。死亡/销毁入口读取同一 blackboard 余额，调用 release_external_qi_to_zone，再投影已提交的 transfers。
- **出料**：修复后的 canonical transaction 必须同时定义 zone、道伥 external owner、WorldQiAccount balance/audit、spawn request、死亡释放和 WorldQiSnapshot 的边界；失败时不可留下部分 zone debit、孤儿账户或重复 owner 余额。
- **共享类型 / event**：复用 ZoneRegistry、Zone.spirit_qi、DaoZhangBehaviorBlackboard、SpawnDaoZhangFromCondenseRequest、QiTransfer、QiTransferReason::{TiandaoCondense,ReleaseToZone}、WorldQiAccount、WorldQiBudget、summarize_world_qi、release_external_qi_to_zone、QI_ZONE_UNIT_CAPACITY。不得另造第二套 qi ledger 或把 QiTransfer event 当作自动 consumer。
- **跨仓库契约**：本 bughunt 的核心修复是 server 内部 owner/ledger contract，不新增 agent/client wire。已有 bong:world_state 若继续发布 qi snapshot，必须使用修复后的唯一 owner 投影；若后续需要新的 telemetry（例如 bong:qi/ledger），必须在 active 阶段明确 schema、发布者和消费者，不能只发无消费者的 event。
- **worldview 锚点**：docs/worldview.md §二 L30-L50 的正域/死域/负灵域与灵压语义；docs/worldview.md §十 L870-L880 的全服灵气零和与缓慢重分配。真元总量是质量流向，不是任意字段加减。
- **qi_physics 锚点**：底盘复用 qi_physics::ledger::{WorldQiAccount, QiTransfer, assert_conservation, summarize_world_qi}、qi_physics::release::qi_release_to_zone、qi_physics::constants::QI_ZONE_UNIT_CAPACITY。新增物理常数、单位换算或衰减公式必须先进入 qi_physics，本 plan 不自定义一份。

### 与既有 plan 的责任边界

- docs/finished_plans/plan-qi-physics-v1.md 已提供 ledger、snapshot、释放算子和守恒断言底盘，但明确把既有 gameplay 接线留给 patch/后续 plan；它不是本具体缺陷的修复记录。
- docs/finished_plans/plan-qi-conservation-leaks-v1.md、plan-combat-qi-invest-conservation-v1.md 和历史 qi 清扫已处理其它释放/消耗路径；未覆盖道伥凝结 owner id 与 blackboard snapshot 投影这组问题。
- docs/finished_plans/plan-daozhan-v1.md 已落地道伥行为、TiandaoCondense reason 和死亡释放的玩法接线，但本报告证明其两端 owner identity/ledger 轨迹尚未对称。
- docs/plan-refactor-qi-ledger-v1.md 是更宽的 R5 架构重构轨，拥有全仓字段私有化和 fauna/npc 批次的总体边界。本 skeleton 只立道伥的证据、owner identity 和 transaction 决策入口；升 active 前须决定将 P1-P3 吸收到 R5 P3，还是保留为独立的具体修复 PR，不能重复实现 R5 的全仓私有化。
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
3. WorldQiAccount::transfer（server/src/qi_physics/ledger.rs:554-589）才是 balance-mutating ledger transaction。单纯 EventWriter<QiTransfer>::send 不改变 WorldQiAccount.balances，也不会由 Bevy 自动消费；事件只能在真实状态提交后作为轨迹投影，除非 P0 明确设计出完整、幂等、不会双扣的 consumer。
4. QiTransferReason::TiandaoCondense 是 BalanceMutating，不能把凝结事件事后降格成 AuditOnly。HalfStepBuff 只改 qi_max 容量并发审计事件，属于明确的 AuditOnly 设计，不得作为统一 consumer 的 balance transfer 输入。

## 已证实缺陷与物理侧限定

### 1. 凝结方向只改物理 zone，不提交 WorldQiAccount

- server/src/fauna/daozhan.rs:1064-1068 计算实际扣减量：

      let actual_cost = TIANDAO_CONDENSE_QI_COST
          .min(zone.spirit_qi - TIANDAO_CONDENSE_THRESHOLD);

- server/src/fauna/daozhan.rs:1070 直接写 zone.spirit_qi = (zone.spirit_qi - actual_cost).clamp(-1.0, 1.0)。
- server/src/fauna/daozhan.rs:1072-1079 构造 QiTransfer { from: zone, to: npc, amount: actual_cost, reason: TiandaoCondense } 后只调用 EventWriter::send；system 参数没有 WorldQiAccount，也没有 transfer_zone_qi_to_ledger、transfer_external_qi_to_ledger 或其它 ledger 提交。
- server/src/fauna/daozhan.rs:1085-1090 发出的 SpawnDaoZhangFromCondenseRequest.condensed_qi 是同一笔 actual_cost，不是独立固定的“初始量”。

### 2. 同一个 external owner 使用了两个账户 id

- 凝结 transfer 使用 daozhan:condense:<zone>:<tick> 这一按事件生成的账户 id（server/src/fauna/daozhan.rs:1072-1079）。
- 死亡释放使用 canonical_npc_id(entity)（server/src/fauna/daozhan.rs:1209-1221）读取/归还道伥实体的 blackboard 余额。
- 因而同一物理道伥从凝结到死亡没有一个贯穿生命周期的 durable owner identity；不能把凝结临时账户和死亡 canonical 账户当成同一 balance，也不能用两个账户的和掩盖重复/遗漏。

### 3. 通用 snapshot 没有投影道伥 blackboard owner

- server/src/qi_physics/ledger.rs:1013-1053 的 summarize_world_qi 统计 player、zone、inventory 和 ledger，但不查询 DaoZhangBehaviorBlackboard.daozhan_qi。
- WorldQiSnapshot 因此是通用 snapshot，不是当前道伥完整 external-owner projection；即使 spawn consumer 被挂进测试，仍需先决定 registry/投影契约，否则会漏计或双计。
- 本轮测试债现场的临时投影位于保留的旧分支 commit 99ce719a8，不是本 skeleton PR 的文件；该现场不能替代生产 snapshot contract。

### 4. TIANDAO_CONDENSE_INITIAL_QI 是死常量

- server/src/fauna/daozhan.rs:1002 定义 TIANDAO_CONDENSE_INITIAL_QI，全仓只有定义处一处引用；真实凝结使用的是 actual_cost。实施时应删除/重命名死常量或补充其明确语义，不能让它继续暗示凝结总是固定量。

### 5. 这不是“物理侧必然吞真元”的无条件结论

当前物理 owner 链是：

    zone.spirit_qi -= actual_cost
      → SpawnDaoZhangFromCondenseRequest.condensed_qi = actual_cost
      → DaoZhangBehaviorBlackboard.daozhan_qi = actual_cost
      → release_external_qi_to_zone(..., actual_cost, ...)
      → zone / overflow

actual_cost 被传入 blackboard，物理上可以形成 zone → 道伥 → zone 的闭环；本 bug 的确定结论是 ledger/audit 方向不对称、owner identity 不一致、通用 snapshot 漏投影。凝结红测还不证明下游 spawn 后完整物理链必然销毁真元。

## 红测证据（保留原始条件，不为变绿而放宽）

真实调用 daozhan_tiandao_condense_system 的测试过滤命令：

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

fixture 的 spirit_qi_before = 0.90，凝结 system 发出一个 request，condensed_qi = actual_cost = 0.05，zone 归一化字段减少 0.05。snapshot 按 QI_ZONE_UNIT_CAPACITY = 50 观察到 zone 绝对量减少 2.5，所以 before.total_observed - after.total_observed = 45.0 - 42.5 = 2.5，era_decay = 0。

必须同时保留以下限定条件：

- 测试 App 只挂了凝结 system，**没有挂 spawn consumer**，因此测试时没有对应道伥 external owner 余额，也没有 ledger balance 变化。
- 即便把 spawn consumer 挂上，当前 summarize_world_qi 仍不投影 DaoZhangBehaviorBlackboard.daozhan_qi；通用 snapshot 仍不能单独证明完整道伥 owner 链守恒。
- 这条红测证明当前真实凝结入口不能满足通用 snapshot 的守恒闭环，不证明下游完整物理链必然销毁真元。禁止通过放宽 tolerance、只比较 zone、退回局部算术或把测试 App 改成自指投影来“修绿”。
- 生产 daozhan.rs 没有 assert_conservation 调用；全仓没有生产 EventReader<QiTransfer> consumer。事件发送本身不会应用到 WorldQiAccount。
- WorldQiAccount::transfers（审计轨迹）与 ledger_qi（稳定余额）是不同观测：死亡 zone accepted 路径可增加 audit 而不增加 ledger_qi，overflow 路径才把稳定 overflow balance 纳入 ledger。验收必须分别断言事件轨迹和余额总量。

## 全仓 EventWriter<QiTransfer> 影响面

以下表格来自对 server/src 直接声明 EventWriter<...QiTransfer> 的全量扫描；qi_physics/ledger.rs 的命中只有文档/测试注释，不是生产 writer。分类含义：canonical = 调用带 WorldQiAccount 的 typed ledger helper/事务；event-only = 物理字段改变后只发事件或只用低层 qi_release_to_zone，没有在该路径提交 WorldQiAccount；mixed = 两种路径并存；AuditOnly = 只改变容量/状态而非真元 balance，不能误当 transfer。

| 文件 | 证据与判定 |
|---|---|
| server/src/fauna/daozhan.rs | 凝结 :1064-1079 手写扣 zone + 裸发 TiandaoCondense；死亡 :1209-1221 走 release_external_qi_to_zone，zone accepted 写 audit、overflow 真实入稳定 ledger。**mixed，且凝结方向不对称。** |
| server/src/combat/carrier.rs | charge_carrier_tick :550-560 手写扣 cultivation.qi_current，emit_carrier_channeling_transfer :692-708 裸发事件；残余释放 :1468-1480 调低层 qi_release_to_zone 并发事件，release_account_to_zone 没有 WorldQiAccount 参数。**event-only / 后续治理。** |
| server/src/combat/lifecycle.rs | revive staging :1782-1789 调 staged_cultivation.release_to_zone，进入 release_external_qi_to_zone；提交后的 transfers 在 :1916-1918 发出。**canonical，事件是事务结果投影。** |
| server/src/combat/needle.rs | 发射 :136-169 手写扣 qi_current 并发 Channeling；过期释放 :343-355 调低层 qi_release_to_zone，overflow :382-395 仍裸发。**event-only，物理扣减/释放路径没有 WorldQiAccount 提交。** |
| server/src/combat/rat_bite.rs | :104-112 调 cultivation.transfer_to_external_actor，实现 server/src/cultivation/components/qi_flow.rs:441-486 在修改 source 前调用 transfer_external_qi_to_ledger（:478-484）；:125-128 发已提交结果并从 ledger 读取 rat 余额。**canonical real ledger。** |
| server/src/combat/woliu.rs | projectile transfer :969-996；maintenance :629-645 手写扣 player，再由 :1016-1032 使用低层 qi_release_to_zone 并发事件。**event-only / 低层 helper。** |
| server/src/combat/woliu_v2/tick.rs | :131-144 低层 qi_release_to_zone 后发事件；overflow :175-206 用 set_balance + push_transfer_audit 手工入账再发事件。**mixed：overflow 有真实余额写入但非统一 transfer 事务，zone accepted 依赖低层事件投影。** |
| server/src/combat/zhenmai_v2.rs | multipoint_duration_tick :998-1012、harden_duration_tick :1044-1058 手写扣 player，再经 drain_release_to_zone :1248-1264 调低层 qi_release_to_zone 发事件。**event-only / 低层 helper。** |
| server/src/cultivation/death_hooks.rs | revive/terminate :187-218、:292-311 调 release_cultivation_qi_to_zone；公共 helper :349-388 校验 identity 并调用 cultivation.release_to_zone。**canonical。** |
| server/src/cultivation/full_power_strike.rs | charge_tick_system :306-331 手写扣 player；释放 helper :350-401 调低层 qi_release_to_zone，:461-463 发事件。**event-only / 低层 helper。** |
| server/src/cultivation/tribulation.rs | HalfStepBuff :2200-2212 只改变 qi_max 容量并发 HalfStepBuff；server/src/qi_physics/ledger.rs:346-386 明确该 reason 为 AuditOnly，不是 balance transfer。同文件 :3047-3066 从 gain_from_zone 发事务结果，属 **canonical**。 |
| server/src/dandao/boss_spawn.rs | :395-405 使用 cultivation.release_to_zone(..., &mut qi_account, ...) 后发 outcome。**canonical。** |
| server/src/fauna/dying_elder.rs | 给丹 overflow :712-728、rift drain :1159-1178 用 transfer_external_qi_to_ledger；elder 接收/夺取 :745-759、:962-981 用 push_transfer_audit + event；死亡 zone/overflow :1287-1356 组合低层释放与 overflow ledger。**mixed，但有明确 ledger 提交，需逐 external owner 审计。** |
| server/src/fauna/hybrid_beast.rs | 融合 :441-474 手写组件兽→hybrid event 和 zone 回写；rage :776-798 手写 zone 减、cultivation.qi_current += gain 后裸发 event，无对应 WorldQiAccount 事务参数。**event-only / 高风险。** |
| server/src/lingtian/systems.rs | :1592-1619 手写 plot 清零、player credit、ZoneQiAccount 回写，:1652-1710 组装/发送事件；使用独立 ZoneQiAccount，不是 WorldQiAccount。**不属于 canonical WorldQiAccount 路径。** |
| server/src/network/command_executor.rs | pseudo-vein spawn :1021-1028 的 transfer 来自 inject_zone_for_pseudo_vein；真实注入在 server/src/world/pseudo_vein_runtime.rs:498-542 使用 transfer_ledger_qi_to_zone。**forwarder，本身不是裸物理扣减；canonical。** |
| server/src/world/events.rs | flush_collapse_qi_transfers :1817-1827 只把 ActiveEventsResource 队列 flush 到 Bevy event；collapse redistribution :1995-2070 直接改 zone、把 overflow 放进 Vec<QiTransfer>，没有向该函数传入 WorldQiAccount。**event-only queue / 后续核验。** |
| server/src/world/heartbeat.rs | dynamic pseudo-vein :1378-1392、:1412-1427 调带 ledger 的 settlement，再发送返回 transfer；spawn omen 也把带 ledger 的 helper 结果发出 :1507-1518。**canonical。** |
| server/src/world/pseudo_vein_runtime.rs | 注入 :498-542 使用 transfer_ledger_qi_to_zone；结算 :564-585、:592-620 使用 transfer_zone_qi_to_ledger。**canonical。** |
| server/src/world/tsy_lifecycle.rs | family teardown :661-685 对 Cultivation/道伥 staged owner 调 release_to_zone/release_external_qi_to_zone，:716-720 发送已提交 staged ledger transfers。**canonical staged transaction。** |
| server/src/zhenfa/mod.rs | scatter bead :2383-2439 先低层计算后实际调用 ledger.transfer，属显式 ledger transaction；sealed trap release :4388-4415 只低层改 zone、发 event，overflow :4443-4459 也只构造事件。**mixed，trap release event-only。** |

共同风险不是“所有裸事件都必然吞真元”：某些 producer 已经正确改变 ECS/专用状态，事件本来只应做 audit；问题在于 reason disposition、WorldQiAccount 余额、external owner registry 和 snapshot 口径没有统一契约，导致 TiandaoCondense 这种 BalanceMutating reason 也落入 event-only 形态。

## 修复方向备选（本 skeleton 不替实施者拍板）

### 方案 A：专用 canonical zone → external-owner 事务

在 qi_physics 增加与 release_external_qi_to_zone 对称的 typed helper，或将“扣 zone、创建/登记道伥 owner、写转移审计、写 spawn request”包成可回滚事务。必须统一 zone.spirit_qi 的归一化/绝对单位，明确 actual_cost 是 zone fraction 还是 raw qi；所有可失败步骤成功前不提交 zone debit，spawn 失败要整笔回滚或把余额转入稳定 overflow；凝结和死亡使用同一 durable NPC owner identity；TiandaoCondense 的 ledger/audit 方向与死亡归还方向可追踪且成对。

优点：影响面集中、事务边界清晰。风险：必须解决“先生成 durable entity id 还是先扣 zone”的跨 system 生命周期，以及 summarize_world_qi 不收集 blackboard owner 的问题。

### 方案 B：增加统一 QiTransfer consumer

增加有明确 system ordering 的 consumer，读取 QiTransfer，把 BalanceMutating 事件应用到 WorldQiAccount，并建立可解析的 zone/ECS external owner registry。

优点：可集中处理漏记事件。风险很高：大量 producer 已先改 ECS/zone，直接消费会双扣/双记；HalfStepBuff 等 AuditOnly reason 必须排除；现有 event 没有 durable owner registry，还要解决幂等、失败回滚和 ordering。不能简单添加“遍历事件然后 ledger.transfer”来修本问题。

### 方案 C：统一 external-owner registry + snapshot 契约

引入 typed external owner 注册接口，定义每类 owner 的唯一存储位置、余额读写、生命周期、转移 reason 和 audit 方式；summarize_world_qi 从 registry 汇总未镜像 owner，并为 TiandaoCondense/ReleaseToZone 提供同一事务 API。

优点：从根上解决 blackboard 不在 snapshot 以及 ledger 重复计数歧义，可供 carrier、needle、hybrid beast 等路径迁移。风险：改动跨 qi_physics、fauna、combat、snapshot/IPC 和持久化，必须与 plan-refactor-qi-ledger-v1 分阶段协调，不能在本 plan 中顺手做全仓重构。

### 方案 D：deferred transaction

保留 request 作为意图，但让 request 携带可恢复 reservation/transaction id；spawn 创建并登记 durable external owner 成功后，再由统一事务提交 zone debit 和 owner credit。spawn/ledger 失败时释放 reservation，不让 zone 先减少后靠裸事件补账。

优点：适配当前凝结 system 不直接持有 Commands 的架构。风险：需要处理重复 event、重启恢复、冷却 state 和 reservation 持久化，不能只把现有 QiTransfer 从一个 system 移到另一个 system。

## 开放问题（转 active / P0 决策门前必须收口）

1. **actual_cost 的单位是什么？** 它是归一化 zone.spirit_qi fraction，还是已经换算后的 raw qi？zone debit、condensed_qi、DaoZhangBehaviorBlackboard.daozhan_qi、WorldQiAccount 和 snapshot 各自的单位/换算点必须由 QI_ZONE_UNIT_CAPACITY 唯一锚定。
2. **durable entity id 与扣款顺序是什么？** 应先生成/保留 canonical NPC id 再扣 zone，还是先创建可恢复 reservation 再提交 debit？每个 spawn、duplicate event、system restart、spawn failure 分支的回滚/overflow 落点是什么？
3. **external owner registry 放在哪里、活多久？** 由 blackboard/ECS 作为唯一 owner，还是持久化稳定账户作为唯一 owner？despawn、死亡、跨 tick、离屏、重启 hydrate 和实体复活时如何注册、转移和注销？
4. **是否需要统一 consumer？** TiandaoCondense、现有 canonical producer、event-only producer、AuditOnly reason 的处理边界是什么？如何以 reason disposition、owner capability、transaction id 和 system ordering 防止已有 producer 双扣/双记？
5. **summarize_world_qi 的投影契约是什么？** external owner 是否全部纳入 WorldQiSnapshot，还是只允许持久化 ledger owner；registry 中的 owner 是否要进入 persistence/IPC，以及如何证明 snapshot 不漏计、不重复计入。
6. **与 plan-refactor-qi-ledger-v1 如何交接？** 本问题的道伥具体 owner/transaction 修复由 R5 P3 吸收，还是独立 PR 先落；两者不得同时改变 qi_current/zone.spirit_qi API 或各自引入 registry。

原始开放问题保留以便追溯；在所有问题由代码证据支持并写入 active plan 的 §N.1 决议前，禁止进入 P0 实施。每条决议必须落到真实 file:line 与 plan 章节双锚点。

## P0 — 守恒合同与责任收口

**可核验交付物：**

- 冻结 P/L/T、WorldQiBudget、QI_ZONE_UNIT_CAPACITY、signed zone、external owner 与 stable ledger account 的单位和唯一性合同；明确 WorldQiAccount::transfers 仅是审计，不是余额。
- 对 QiTransferReason::{TiandaoCondense,ReleaseToZone,HalfStepBuff} 写出 disposition 表：哪些是 BalanceMutating，哪些是 AuditOnly，每个 reason 的唯一 source/target owner capability、transaction id、失败语义和 audit 顺序。
- 以 server/src/fauna/daozhan.rs::daozhan_tiandao_condense_system、SpawnDaoZhangFromCondenseRequest、DaoZhangBehaviorBlackboard、release_external_qi_to_zone 和 summarize_world_qi 为接入清单；不能用“统一 consumer”一句话替代 owner/transaction 设计。
- 由代码核查补齐开放问题的 §N.1 决议，并明确本 plan 与 plan-refactor-qi-ledger-v1 P3 的单一责任主体。

**测试声明：** 在不改生产代码的骨架阶段不新增测试；active P0 必须增加 qi_physics/transaction fixture，覆盖单位换算、same-account、insufficient、destination overflow、owner identity、AuditOnly 排除、失败零写入和 assert_conservation 的 era_decay 分支。

## P1 — 道伥凝结/死亡 canonical transaction

**可核验交付物：**

- server/src/fauna/daozhan.rs 的凝结 system 不再以裸 zone.spirit_qi 写入充当事务；actual_cost 从唯一换算入口产生，并与 SpawnDaoZhangFromCondenseRequest、external owner credit、TiandaoCondense audit 绑定同一 durable owner/transaction id。
- spawn consumer 对 DaoZhangBehaviorBlackboard.daozhan_qi 的写入必须与 canonical owner registry 一致；死亡 server/src/fauna/daozhan.rs:1192-1221 必须从同一 owner 读取，并沿 release_external_qi_to_zone 走 zone accepted/overflow 的真实 ledger 边界。
- 所有可失败步骤（阈值/冷却/数量、spawn、owner registration、zone debit、ledger/audit）定义 preflight、rollback、重复 request 和 restart recovery；不能留下 daozhan:condense:<zone>:<tick> 与 canonical_npc_id(entity) 两个长期账户。

**测试声明：** fauna::daozhan 真实 App 路径必须覆盖阈值 < / == / >、actual_cost 低于请求量的 boundary、zero/negative/overflow zone、spawn failure/no consumer、duplicate transaction、同一 owner 跨凝结→spawn→死亡、zone accepted 与 stable overflow、审计顺序和 T 严格守恒。测试断言取 SPIRIT_QI_TOTAL/QI_ZONE_UNIT_CAPACITY/QI_EPSILON 等 canonical 引用，不写全服总量字面量。

## P2 — external-owner registry 与 snapshot/持久化

**可核验交付物：**

- 在 server/src/qi_physics/ledger.rs 或经 P0 决议指定的模块定义 typed external owner registry；明确 DaoZhangBehaviorBlackboard.daozhan_qi 与 WorldQiAccount 哪一方是唯一物理 owner，另一路只能是只读 projection。
- 扩展 summarize_world_qi/WorldQiSnapshot 的 external-owner 投影和去重规则；WorldQiAccount::total()、ledger_qi、audit transfers、WorldQiBudget.current_total 四种观测不得混用。
- 若 owner 跨 tick、despawn、离屏或重启仍存在，接入 persistence 的 encode/decode/hydrate、stable id、缺行/非法值 fail-closed 和 unregister/terminal release；不得把 store.remove 当作释放真元。

**测试声明：** snapshot 需覆盖 blackboard-only、ledger-only、合法转换中间态、zone accepted、overflow、缺失/重复 registry、despawn/death/restart hydrate、same owner 不双计和 unrelated owner 隔离；每个状态转换至少有专属用例，并对 assert_conservation(before, after, era_decay) 做外部可观察断言。

## P3 — 影响面治理与不双扣迁移

**可核验交付物：**

- 对上表每个 EventWriter<QiTransfer> 文件完成逐条责任记录：canonical producer、event-only 真实物理 owner、mixed 分支、AuditOnly 分支和后续 owner/plan；不能把全表机械改成统一 consumer。
- 先以 P0 的 reason/owner contract 选择本 plan 实际拥有的迁移范围；carrier.rs、needle.rs、woliu.rs、hybrid_beast.rs、world/events.rs、zhenfa/mod.rs 等 event-only/mixed 路径若不在本 PR，必须留下明确的 follow-up basename 和不重复接管说明。
- 对所有纳入迁移的 producer 使用同一 typed transaction API；保持已有物理字段语义，不因补 ledger 而再扣一次 zone/player，也不把 HalfStepBuff 等 AuditOnly 误转为 balance mutation。

**测试声明：** 每种分类至少有一条真实 producer→owner→zone/ledger 的契约测试；覆盖 pre-existing canonical path 不双扣、event-only 路径的 owner 归属、AuditOnly 不改变 T、mixed overflow/zone accepted 分叉、invalid owner/duplicate event fail-closed。测试断言 payload、余额、zone、audit 和 snapshot，不断言内部调用次数。

## P4 — 运行链路与验收关闭

**可核验交付物：**

- 真实 Update/Bevy system 顺序覆盖凝结、spawn、道伥伏击累积、死亡释放、overflow 和 snapshot；不以只挂一个 system 的 test app 冒充完整链路。
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

