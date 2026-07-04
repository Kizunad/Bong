# plan-ambient-ratbite-ledger-leak-v1 — ambient Rat 鼠咬真元漏账修复

> **一句话主题**：`plan-ambient-threat-v1` 接入的 ambient Rat 通过 `RatBite` 真实扣掉玩家 `Cultivation.qi_current`，但被偷真元只 emit `QiTransfer(RatBiteDrain)` 事件、从无 system 消费落账，鼠死亡/超距回收又只把 `drained_qi` 的 1% 写回 `zone.spirit_qi`——被偷真元 99% 长期蒸发，`summarize_world_qi` 守恒审计持续失真。本 plan 把 `RatBiteDrain` 从"半接线的 audit event"改成"真实 `WorldQiAccount` 双腿记账"：咬击瞬间落入 `npc:rat:<id>` ledger 账户（不再蒸发），鼠死亡/超距回收时把该账户 100%（不再是 1%）转入 `zone:<name>` ledger 账户**并同步写回 `zone.spirit_qi` 字段**（field-authority 三段式，见 §8.1 #3——仓库正典是"字段权威、ledger 账户为镜像"，只改账户不写回字段会被生产常驻的 `zone_qi_inflow_tick` 覆盖式重同步二次抹掉）。

**状态**：active（§8 已收口，见 §8.1）。升 active 日期：2026-07-04。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 鼠咬真元漏账修复——咬击落账 + 死亡/超距回收全额转账 + 写回 `zone.spirit_qi` 字段（§8.1#1/#2/#3） | ⬜ |

---

## 背景诊断（2026-07-04，代码实证，origin/main HEAD `30666096`）

- **咬击扣真元但不落账**：`server/src/combat/rat_bite.rs:45-98` `apply_rat_bite_qi_drain` 直接扣 `cultivation.qi_current`（:70-72），把等量值累加进 ECS 组件 `RatBlackboard.drained_qi`（:75），随后只 `qi_transfers.send(QiTransfer::new(..., RatBiteDrain))`（:78-81）——`qi_physics::mod.rs:147` 只 `add_event::<QiTransfer>()` 注册事件类型，全仓 `grep -rn "EventReader<QiTransfer>" server/src` **零命中**，没有任何通用消费器把该事件写回 `WorldQiAccount`。
- **`RatBlackboard.drained_qi` 不在四桶统计内**：`qi_physics/ledger.rs:641-676` `summarize_world_qi` 的口径固定四桶——`player_qi`（Cultivation 组件）、`zone_qi`（`ZoneRegistry.spirit_qi` 字段）、`container_qi`（背包物品）、`ledger_qi`（`WorldQiAccount::total()`）。`RatBlackboard.drained_qi` 是普通 ECS 字段，不落在任何一桶——真元离开 `player_qi` 桶后即从审计口径消失。
- **死亡/超距回收只归还 1%**：`fauna/rat_phase.rs:28` `RAT_DRAINED_QI_DEATH_RETURN_RATIO = 0.01`；`fauna/rat_phase.rs:384-389` `return_rat_drained_qi_to_zone` 只把 `drained_qi × 0.01 / QI_ZONE_UNIT_CAPACITY` 写回 `zone.spirit_qi` 字段，其余 99% 随 `insert(Despawned)` 软删除或 `DeathEvent` 结算彻底消失。两处调用点：`fauna/rat_phase.rs:350-376` `release_drained_qi_on_death_system`（死亡）与 `npc/spawn/ambient_scheduler.rs:660-684`（超距回收，§ambient-threat-v1 Verify blocker② 已让回收前先走归还路径，但归还路径本身仍只还 1%）。
- **可达链路（新手区高频命中）**：`server/src/npc/mod.rs` 注册 `spawn::ambient_scheduler::register(app)`；`ambient_scheduler.rs` 把 danger 1~2 的 ambient threat pool 固定为 `ThreatSpecies::Rat`；`server/zones.json` 的 `spawn` 区 `danger_level: 1`；`brain_rat.rs` 的 `HarassPlayerQuery` 锁定 `With<ClientMarker>` 在线玩家近身 emit `RatBiteEvent`；`combat/mod.rs:281,343` 正式挂 `apply_rat_bite_qi_drain` 进 combat 主链。新手区/spawn 区正常走图就会被鼠患反复偷真元。

## 接入面（docs/CLAUDE.md §二 checklist）

- **进料**：`combat::rat_bite::RatBiteEvent`（既有，`brain_rat.rs` emit）；`cultivation::components::Cultivation`（既有扣减路径，不改）；`npc::spawn_rat::RatBlackboard`（既有 `drained_qi` 字段，改为镜像 ledger 余额）；`world::zone::ZoneRegistry`（死亡/超距回收路径已有的 zone 名解析，不改）。
- **出料**：`qi_physics::ledger::WorldQiAccount` 新增/更新两个账户——咬击时 `QiAccountId::npc("rat:<entity_index>")` 余额增加（`ledger_qi` 桶）；死亡/回收时该账户 100% 转入 `QiAccountId::zone("<zone_name>")` 余额，**并按 §8.1 #3 field-authority 三段式把结果同步写回 `ZoneRegistry` 对应 `Zone.spirit_qi` 字段**（不是"仍留在 ledger_qi 桶不动字段"——真元从 `ledger_qi` 桶回落进 `zone_qi` 桶，与仓库既有 `cultivation/tick.rs` / `world::pseudo_vein_runtime` 范式一致，且避免被生产常驻 `world::heartbeat::zone_qi_inflow_tick` 的覆盖式 `set_balance` 二次抹掉）。`summarize_world_qi` 的 `ledger_qi` 桶在鼠咬瞬间等量吸收被偷真元、死亡/回收时该增量转移到 `zone_qi` 桶，链路头尾总量不变。`bong:qi/ledger` Redis telemetry（`ledger.rs:584-620`）自动多出 `account:npc:rat:<id>` 字段行，供外部 e2e 精确断言，无需改 schema。
- **共享类型 / event**：复用既有 `QiTransferReason::RatBiteDrain`（`ledger.rs:225-230`，doc-comment 早已声明 `to=npc:rat:<id>` 语义，本 plan 只是把代码补齐到文档声明的行为）、`QiAccountId::npc` / `QiAccountId::zone` 构造函数、`WorldQiAccount::push_transfer_audit` / `WorldQiAccount::transfer` / `WorldQiAccount::balance` / `WorldQiAccount::set_balance`（全部已存在，无需新增 ledger API）、`qi_physics::constants::QI_ZONE_UNIT_CAPACITY`（字段↔账户换算单位，`world::pseudo_vein_runtime` 同款用法）。不新增 `QiTransferReason` 变体，不新增账户 kind。
- **跨仓库契约**：纯 server-side 修复。无 agent / client symbol 变更；`bong:qi/ledger` 是 server 内部 Redis telemetry（非 TypeBox IPC schema），字段集变化不影响双端 schema 对齐。
- **worldview 锚点**：worldview §二"真元极易挥发但守恒律不可破——全服灵气总量恒定，修炼消耗 = 别人少掉"；本 plan 修复的正是这条正典在 ambient Rat 路径上的违反（99% 无去向，且 §8.1 #3 修正了"改账户不写回字段"这一潜在二次蒸发形态）。
- **qi_physics 锚点**（红旗强约束）：只调用 `qi_physics::ledger` 既有 API（`WorldQiAccount::{balance,set_balance,push_transfer_audit,transfer}`、`QiTransfer::new`、`QiAccountId::{npc,zone}`）与 `qi_physics::constants::QI_ZONE_UNIT_CAPACITY`，不新增衰减/吸取常数或公式，不需要扩 `qi_physics` 模块本身。「鼠 → zone」腿的 field-authority 三段式（同步→transfer→写回）照抄 `world::pseudo_vein_runtime::inject_zone_for_pseudo_vein` 既有范本，不是本 plan 自创的新公式。

## P0 — 鼠咬真元漏账修复（§8.1 决议为准）

### 交付物

1. **`server/src/combat/rat_bite.rs:45-98` `apply_rat_bite_qi_drain`**：新增 `mut ledger: Option<ResMut<WorldQiAccount>>` 参数（对齐 `npc/skull_fiend.rs:269-270` 的降级写法：无资源时 `tracing::debug!` 跳过落账，不 panic、不阻断既有 HP/事件流）。在现有 `drained > 0.0` 分支（:74-86）内，除保留 `qi_transfers.send(transfer.clone())`（审计事件广播不变）外，新增 helper `fn credit_rat_bite_drain(account: &mut WorldQiAccount, rat_account: &QiAccountId, amount: f64, transfer: QiTransfer)`（同文件新增函数，逻辑照抄 `npc/skull_fiend.rs:747-766` `credit_skull_fiend_drain`：`set_balance(rat_account, balance(rat_account) + amount)` 后 `push_transfer_audit(transfer)`）。
2. **`RatBlackboard.drained_qi` 镜像 ledger 余额**：同一分支内，`rat.drained_qi` 从"独立累加"改为 `ledger.as_deref().map(|a| a.balance(&rat_account)).unwrap_or(rat.drained_qi + drained)`（照抄 `fauna/mimic_spider.rs:270-271` `blackboard.drained_qi = ledger.balance(&spider_account)` 的镜像模式；`ledger` 缺席时保留原地累加 fallback，无 ledger 资源的 headless 测试行为不退化）。
3. **`server/src/fauna/rat_phase.rs:28` `RAT_DRAINED_QI_DEATH_RETURN_RATIO`**：常量与其唯一消费者 `rat_phase.rs:384-389` `return_rat_drained_qi_to_zone`（1% 直写 `zone.spirit_qi` 字段）整体删除，替换为 **§8.1 #3（blocker 修正）field-authority 三段式**：`pub fn transfer_rat_drained_qi_to_zone(ledger: &mut WorldQiAccount, zone: &mut Zone, rat_account: &QiAccountId) -> Result<f64, QiPhysicsError>`（`zone_account` 由 `zone.name` 内部派生，不再由调用方单独传入，防止字段/账户对错 zone）：
   - 读 `ledger.balance(rat_account)` 为 `amount`；`amount <= 0.0` 时 no-op 返回 `Ok(0.0)`。
   - 转账前：`ledger.set_balance(zone_account.clone(), zone.spirit_qi.max(0.0) * QI_ZONE_UNIT_CAPACITY)?`，把 zone 账户镜像同步到字段真实值（照抄 `world::pseudo_vein_runtime::inject_zone_for_pseudo_vein`：459-489，与 `world::heartbeat::zone_qi_inflow_tick`：2131-2138 同款范式），避免用陈旧镜像余额做后续 insufficient 检查。
   - 执行 `ledger.transfer(QiTransfer::new(rat_account.clone(), zone_account.clone(), amount, QiTransferReason::RatBiteDrain)?)?`（100% 落账；`RatBiteDrain` 不在 `ledger.rs:414-431` `transfer()` 的 audit-only 拒绝名单里，可合法走真实双账户转账，天然获得 insufficient 检查 + from/to 同步扣加 + 审计追加）。
   - 转账后：把结果写回字段——`zone.spirit_qi = (ledger.balance(&zone_account) / QI_ZONE_UNIT_CAPACITY).clamp(-1.0, 1.0)`（clamp 边界照抄旧 `return_rat_drained_qi_to_zone` 既有写法）。**这一步是 blocker 修正的核心**：缺了这行，下一次生产 `zone_qi_inflow_tick`（Update schedule 常驻系统）会用未变的 `zone.spirit_qi` 覆盖式 `set_balance` 把刚转入账户的余额清零，造成二次蒸发（详见 §8.1 #3）。
   - 返回 `Ok(amount)`。
4. **调用点改造**（均需额外传入调用点已持有的 `&mut Zone` 借用，不再单独构造 `zone_account`）：
   - `fauna/rat_phase.rs:350-376` `release_drained_qi_on_death_system` 新增 `mut ledger: Option<ResMut<WorldQiAccount>>` 参数，把原 `return_rat_drained_qi_to_zone(zone, rat.drained_qi)` 调用替换为 `transfer_rat_drained_qi_to_zone(&mut ledger, zone, &QiAccountId::npc(format!("rat:{}", death.target.index())))`（`ledger` 缺席时跳过，对齐既有降级写法；`zone` 沿用既有 `ZoneRegistry` 查表得到的可变借用）。
   - `npc/spawn/ambient_scheduler.rs:604-624` `ambient_scheduler_system` 签名新增 `mut qi_account: Option<ResMut<WorldQiAccount>>`；`:660-684` 超距回收循环里 `return_rat_drained_qi_to_zone(zone, rat.drained_qi)` 调用同样替换为 `transfer_rat_drained_qi_to_zone(&mut qi_account, zone, &QiAccountId::npc(format!("rat:{}", entity.index())))`（`entity`/`zone` 均已在循环作用域内可用）。
5. **MimicSpider 不在本 P0 scope**（§8.1#2 决议）：`fauna/mimic_spider.rs` 保持现状不改，另立 follow-up 候选（见 §8.1#2 落点）。

### 验收抓手（4 组 pin，覆盖骨架 §P0 原验收清单）

1. `server/src/combat/rat_bite.rs` 测试模块（现 `mod tests`，:190 起）新增 `rat_bite_conserves_total_qi_end_to_end`：起 `App` 插入 `WorldQiAccount::default()`，咬击前后分别求 `player_qi + ledger_qi`（测试 app 无 zone/container 可省略后两项但需断言其恒为 0），断言严格相等——failure message 写清"drained qi 应转入 npc:rat 账户而非消失，实际 before=X after=Y"。
2. `server/src/fauna/rat_phase.rs` 测试模块新增 `rat_death_transfers_full_drained_qi_to_zone_account`（替换现有 1% 归还的同名旧 pin）：鼠死亡前 `npc:rat:<id>` 账户设为 `drained_qi=100.0`，死亡后断言该账户余额 `== 0.0` 且 `zone:<name>` 账户余额 `+= 100.0`（100%，非 1%），**并额外断言 `zone.spirit_qi` 字段本身同步变化**（`+= 100.0 / QI_ZONE_UNIT_CAPACITY`，clamp 后）——只断言账户余额、不断言字段，会漏放过"只改账户不写回字段"的回归（§8.1 #3 blocker 修正点）。
3. `server/src/npc/spawn/ambient_scheduler.rs` 测试模块新增 `recycle_transfers_full_rat_drained_qi_before_despawn`（替换现有 `recycle_returns_rat_drained_qi_to_zone_instead_of_evaporating` 断言的 1% 公式）：超距回收前 `npc:rat:<id>` 设为 `drained_qi=100.0`，回收后断言 `zone` 账户 `+=100.0`、`npc:rat` 账户归零，**同样额外断言 `zone.spirit_qi` 字段同步变化**（同上理由）。
4. 集成 pin `rat_bite_and_death_cycle_preserves_world_qi_total`（新增 `server/src/qi_physics/ledger.rs` 测试模块或 `combat` 集成测试）：模拟"咬 3 次 qi_steal=2 + 鼠死亡"完整链路（复用 `apply_rat_bite_qi_drain` + `release_drained_qi_on_death_system` 两系统跑同一个 `App`），断言链路头尾 `summarize_world_qi(...)` 的 `player_qi + zone_qi + ledger_qi` 总和严格相等（float epsilon 容差），failure message 引用 worldview §二守恒律。**本 pin 必须把 `world::heartbeat::zone_qi_inflow_tick`（生产 `register()` 里注册进 `Update` 的常驻系统，heartbeat.rs:454-468）一并注册进测试 `App` 并在鼠死亡结算后额外推进至少一个 tick**（例：`app.add_systems(Update, (apply_rat_bite_qi_drain, release_drained_qi_on_death_system, zone_qi_inflow_tick).chain())`）——如果 `transfer_rat_drained_qi_to_zone` 只改了 `zone:<name>` 账户余额、没有同步写回 `zone.spirit_qi` 字段，`zone_qi_inflow_tick` 的覆盖式 `set_balance`（heartbeat.rs:2131-2138）会把刚转入的账户余额用未变的字段值重新覆写清零，本 pin 必须能撞红这一二次蒸发回归；此前版本的 pin 不含 `zone_qi_inflow_tick` 视角，看不到这层碰撞，是本次 promote 博弈 blocker 指出的核心风险点。

### 测试声明

- `combat::rat_bite::tests` +1（`rat_bite_conserves_total_qi_end_to_end`），既有 2 条不动（`rat_bite_drains_only_qi_no_hp_damage` / `rat_bite_records_qi_transfer_to_rat_account`，后者需同步断言新增的 ledger 落账分支）。
- `fauna::rat_phase::tests` 1 条替换（`rat_death_returns_one_percent_drained_qi_as_zone_units` → `rat_death_transfers_full_drained_qi_to_zone_account`）。
- `npc::spawn::ambient_scheduler::tests` 1 条替换（`recycle_returns_rat_drained_qi_to_zone_instead_of_evaporating` 断言公式改为 100%）。
- `qi_physics::ledger::tests`（或等价集成模块）+1（`rat_bite_and_death_cycle_preserves_world_qi_total`）。

---

## 反方裁决摘要（继承骨架，未在收口过程中被推翻）

1. Round 1（本机本地模型，默认怀疑）没有提出任何能落到代码点位的实质反证，只给出"也许 `QiTransfer` 事件别处被消费"的弱怀疑。
2. Round 2 在补入 `qi_physics::register` 仅注册 event、无全局消费器，以及 `ambient_threat_pool_fn -> spawn_rat_npc_at` 与 `HarassPlayerQuery(With<ClientMarker>)` 这两条后，仍未给出新的代码级反证；可达性怀疑被排除，只剩模型输出质量不足。
3. 人工复核进一步确认：仓库内 audit-only 真实范式都是调用点自己 `push_transfer_audit` / `set_balance` / `transfer`，`RatBite` 三者全缺；因此该候选在两轮对抗后继续存活。
4. **本次升 active 收口复核（2026-07-04）**：origin/main HEAD `30666096` 上重新 grep 确认现象仍在（`rat_bite.rs:78-81` 仍只 `qi_transfers.send`，`rat_phase.rs:28` `RAT_DRAINED_QI_DEATH_RETURN_RATIO` 仍是 `0.01`），未被 `plan-ambient-threat-v1` 或后续 PR 提前修复。
5. **promote 博弈守恒 blocker 二次收口（2026-07-04）**：反方指出 §8.1 #1 point 3 原定「鼠 → zone 腿只调 `transfer()` 不写回 `zone.spirit_qi` 字段」会被生产常驻 `world::heartbeat::zone_qi_inflow_tick` 的覆盖式 `set_balance`（`heartbeat.rs:2131-2138`）二次抹掉，"100% 转入 zone 守恒"实际不成立。人工复核 grep `world::heartbeat` / `world::pseudo_vein_runtime` / `cultivation::tick` 确认仓库正典是"字段权威、ledger 账户为镜像"（`pseudo_vein_runtime.rs:459-461` doc-comment 与 `inject_zone_for_pseudo_vein` 实现即是该范本），采纳方案 A：`transfer_rat_drained_qi_to_zone` 改为同步→transfer→写回字段的三段式，见 §8.1 #3。

## §8 开放问题（升 active / P0 决策门前收口）

1. `RatBiteDrain` 应该归类为 audit-only 留痕，还是应该给 rat 接真实 ledger 账户并在死亡/回收时完整清算？需要在修复 PR 中一次性定清语义，避免继续半接线。
2. `MimicSpider` 目前也走"`drained_qi` 持有 + 1% 死亡回灌"家族语义；修 rat 时是否顺手复核同类实现，防止只补一处、继续保留同型守恒缺口。

> 全部已在 §8.1 收口。原表保留以备追溯，**实施时以 §8.1 决议为准**。

## §8.1 决议（pre-P0 收口，2026-07-04，实地 grep `qi_physics::ledger` / `npc::skull_fiend` / `fauna::mimic_spider` / `fauna::rat_phase` / `npc::spawn::ambient_scheduler` / `world::heartbeat::zone_qi_inflow_tick` / `world::pseudo_vein_runtime` / `cultivation::tick` 全量核实；#3 为 promote 博弈 blocker 二次收口新增，扩大 grep 范围至后三者）

### #1 audit-only 留痕 vs 真实 ledger 账户

**决议**：
1. **选真实 ledger 账户，不做 audit-only-only 留痕**。`rat_bite.rs::apply_rat_bite_qi_drain` 照 `npc/skull_fiend.rs:747-766` `credit_skull_fiend_drain` 的范式（该函数本身是 `SkullFiendDrain` 既有正确实现，非骨架臆测），在同一系统内直接 `WorldQiAccount::set_balance` + `push_transfer_audit`，把被偷 qi 记入 `npc:rat:<id>` 账户——落在 `summarize_world_qi` 的 `ledger_qi` 桶，从咬击瞬间起就不再蒸发。
2. **不用 `WorldQiAccount::transfer()` 处理"玩家 → 鼠"这一腿**：玩家真元活在 ECS `Cultivation.qi_current`，不镜像进 `WorldQiAccount` 余额（同 `SkullFiendDrain` / `BossDrain` 既有约定），若调 `transfer()` 会因 `from` 账户余额恒为 0 触发 `InsufficientQi` 拒绝。因此"玩家 → 鼠"腿必须走手动 `set_balance` + `push_transfer_audit`（非 `transfer()`）。
3. **"鼠 → zone"这一腿（死亡/超距回收时）改用真实 `WorldQiAccount::transfer()`**：此时双方都是真实 ledger 账户（`npc:rat:<id>` 与 `zone:<name>`），`RatBiteDrain` 不在 `ledger.rs:414-431` 的 audit-only 拒绝名单（`HalfStepBuff` / `DuguReturnToZone` / `DuguReverseVictimQi` / `NegPressureDrain` 四个才被拒），可以合法调用，天然获得 insufficient 检查与账户自动清零（`(available - amount).max(0.0)` 语义）。
4. `RatBlackboard.drained_qi` ECS 字段保留（供 AI/UI 等既有读者），但语义从"独立真元存量"改为"镜像 `npc:rat:<id>` 账户余额的只读投影"，防止两本账分叉。

**落点**：`server/src/combat/rat_bite.rs:45-98`（新增 `credit_rat_bite_drain` helper + `ledger` 参数）/ `server/src/fauna/rat_phase.rs:28,350-389`（删 1% 常量与直写字段函数，改 100% 转账）/ `server/src/npc/spawn/ambient_scheduler.rs:604-624,660-684`（新增 `qi_account` 参数 + 调用点替换）/ plan §P0 交付物 1-4。

### #2 `drained_qi` 归还比例（现 1%）是否调 + MimicSpider 同型复核

**决议**：
1. **归还比例从 1% 调整为 100%**——这不是"数值放宽"而是"决议 #1 落地后的必然结果"：一旦被偷真元在咬击瞬间就进了 `ledger_qi` 桶（真实账户余额），死亡/超距回收时只需把该账户余额**全额**转给 zone 账户即可完成守恒收尾；继续保留"只转 1%"会造成**新的记账错误**——账户里还剩 99% 却在软删除/死亡时无人认领，变成永久滞留在 `ledger_qi` 桶下的死账户余额（不是"蒸发"，但会让 `bong:qi/ledger` 的 `account:npc:rat:*` 字段永久堆积僵尸账户，污染 telemetry）。因此 `RAT_DRAINED_QI_DEATH_RETURN_RATIO` 常量与其消费函数整体删除，见决议 #1 落点。
2. **MimicSpider 复核结论：结构不同，不并入本 P0，但发现一个相关但独立的缺口**。`fauna/mimic_spider.rs:188-272` `spider_disguised_qi_absorb_system` 的吸收腿**已经**是真实 `WorldQiAccount::transfer(zone_account -> spider_account)`（:255-268），`blackboard.drained_qi` 已镜像 `ledger.balance(&spider_account)`（:271）——不是 rat 这种"从未碰 ledger"的缺陷形状，因此**不属于本 plan 要收口的同型 bug**，不并入 P0（避免把与骨架诊断不同形状的问题混进同一 PR，违反 docs/CLAUDE.md §6.3 单 plan 单主题）。
3. 但新发现：`fauna/mimic_spider.rs:277-307` `spider_release_qi_on_death_system` 死亡时仍调用 `return_spider_drained_qi_to_zone`（:309-318，`SPIDER_DRAINED_QI_DEATH_RETURN_RATIO` 1% 直写 `zone.spirit_qi` 字段），**从未 debit `spider_account` 的 ledger 余额**——蛛死后 `spider_account` 里 100% 余额永久滞留 `ledger_qi` 桶（僵尸账户，非蒸发但污染 telemetry + 未来若误读会重复计入），且直写 zone 字段的 1% 是在没有对应 debit 的情况下凭空新增 `zone_qi` 桶——这是与 rat 不同形状但同样违反守恒 telemetry 干净性的独立缺口。**列为本 plan 范围外的 follow-up 候选**，不在本 P0 修，建议后续单独起 bug-hunt 发现或 skeleton plan（例如 `plan-mimic-spider-ledger-orphan-v1`）处理。

**落点**：`server/src/fauna/rat_phase.rs:28,384-389`（决议 1 的删除范围）/ `server/src/fauna/mimic_spider.rs:188-272`（决议 2 复核确认，不改）/ `server/src/fauna/mimic_spider.rs:277-318`（决议 3 新发现的独立缺口，留作 follow-up，不改）/ plan §P0（scope 边界声明）。

### #3 `zone:<name>` ledger 账户与 `zone.spirit_qi` 字段权威冲突（promote 博弈 blocker 修正，2026-07-04 二次决议）

**背景（blocker 复核实证，grep 全量核实）**：决议 #1 point 3 原定"鼠 → zone"腿单纯调 `WorldQiAccount::transfer()` 把 `npc:rat:<id>` 账户余额转入 `zone:<name>` 账户，未写回 `zone.spirit_qi` 字段。但 `world::heartbeat::zone_qi_inflow_tick`（`heartbeat.rs:2077` 定义、`heartbeat.rs:454-468` `register()` 里注册进生产常驻 `Update` schedule）每次 tick 都先执行 `ledger.set_balance(zone_account, zone.spirit_qi.max(0.0) * QI_ZONE_UNIT_CAPACITY)`（`heartbeat.rs:2131-2138`）——**用字段值覆盖账户余额**，这正是仓库既有的"以字段为权威、ledger 账户只是同步镜像"范式：
- `world::pseudo_vein_runtime.rs:459-461` `inject_zone_for_pseudo_vein` doc-comment 明写"记账范本照抄 `zone_qi_inflow_tick`：调用前先用 `set_balance` 把 zone ledger 镜像同步到 `zone.spirit_qi * QI_ZONE_UNIT_CAPACITY` 真实值，转账后再把结果写回 `zone.spirit_qi`"（该函数 :487-508 完整实现了"同步→transfer→写回"三段式，`round3` 定点数精度抹平）。
- `cultivation/tick.rs:263-273` 同样是"`push_transfer_audit` 留痕 + 直写 `zone.spirit_qi -= drain` 字段"，从不单独调 `transfer()` 改 zone 账户余额而不写回字段。

若鼠死亡/超距回收只调 `transfer()` 把 qi 转入 `zone:<name>` 账户却不写回字段，下一次 `zone_qi_inflow_tick`（生产环境每 tick 都跑）会用未变的 `zone.spirit_qi` 重新 `set_balance` 覆盖掉刚转入的余额——被偷真元在两次 inflow tick 之间**第二次蒸发**，"100% 转入 zone 守恒"不成立。

**决议（覆盖 #1 point 3，采纳方案 A 字段权威）**：
1. `transfer_rat_drained_qi_to_zone` 不再是"account-only"转账，改为**field-authority 三段式**（照抄 `pseudo_vein_runtime::inject_zone_for_pseudo_vein` 的同步→transfer→写回结构）：
   - **转账前**：`ledger.set_balance(zone_account.clone(), zone.spirit_qi.max(0.0) * QI_ZONE_UNIT_CAPACITY)`，把 zone 账户镜像同步到字段真实值（防止用陈旧镜像余额做 insufficient 检查，与 `zone_qi_inflow_tick`/`inject_zone_for_pseudo_vein` 同款开场动作）。
   - **执行转账**：`ledger.transfer(QiTransfer::new(rat_account, zone_account, amount, QiTransferReason::RatBiteDrain))`——`npc:rat` 账户全额清零，`zone` 账户增加 `amount`。
   - **转账后写回字段**：`zone.spirit_qi = (ledger.balance(&zone_account) / QI_ZONE_UNIT_CAPACITY).clamp(-1.0, 1.0)`（clamp 边界照抄旧 `return_rat_drained_qi_to_zone` 既有写法）——**这是 blocker 修正的核心一步**，缺了它就会被下一次 `zone_qi_inflow_tick` 覆盖清零。
   - 函数签名因此从「`(account: &mut WorldQiAccount, rat_account: &QiAccountId, zone_account: &QiAccountId)`」改为「`transfer_rat_drained_qi_to_zone(ledger: &mut WorldQiAccount, zone: &mut Zone, rat_account: &QiAccountId) -> Result<f64, QiPhysicsError>`」——`zone_account` 改由 `zone.name` 内部派生，不再由调用方单独传入一个 `QiAccountId::zone(...)`，防止 caller 传错 zone 导致"字段属于 zone A、账户却写进 zone B"的对不上问题。
2. **`npc:rat:<id>` 账户（决议 #1 point 1-2 的"玩家 → 鼠"腿）不受本次修正影响**——那一腿本来就不镜像回任何字段（玩家真元活在 ECS `Cultivation` 组件，鼠账户是纯 ledger 记账，决议 #1 point 2 已排除 `transfer()`），blocker 只影响"鼠 → zone"这一腿的收尾写法，不动决议 #1 point 1-2。
3. **`RatBlackboard.drained_qi` 镜像语义不变**（决议 #1 point 4）——它镜像的是 `npc:rat:<id>` 账户余额，不是 zone 账户，字段权威冲突与它无关。
4. **P0 验收抓手同步扩项**（详见 §P0 验收抓手 #2/#3/#4）：#2/#3 断言从"只断言账户余额"扩为"账户余额 + `zone.spirit_qi` 字段同步变化"双断言；#4 集成 pin 必须把 `zone_qi_inflow_tick` 一并注册进测试 `App` 并多推进至少一个 tick，让"账户改了、字段没写回→下一 tick 被覆盖清零"这条回归路径对测试可见，否则该 pin 无法暴露本条 blocker 指出的碰撞。

**落点**：`server/src/fauna/rat_phase.rs:28,350-389`（`transfer_rat_drained_qi_to_zone` 签名与实现改为 field-authority 三段式）/ `server/src/npc/spawn/ambient_scheduler.rs:604-624,660-684`（调用点同步传入 `&mut Zone`）/ `server/src/world/heartbeat.rs:454-468,2077-2145`（`zone_qi_inflow_tick` 覆盖行为只读引用确认，不改，P0 验收 pin #4 需注册该系统）/ `server/src/world/pseudo_vein_runtime.rs:459-510`（`inject_zone_for_pseudo_vein` field-authority 范本引用，不改）/ `server/src/cultivation/tick.rs:263-273`（字段权威范本引用，不改）/ plan §P0 交付物 3-4（已同步改写为本决议签名）/ §P0 验收抓手 #2-#4（已同步扩项）。

## 审计来源

bug-hunt 定点轮（仅收窄 `plan-ambient-threat-v1` / 当前 `HEAD` 附近 server-side gameplay 代码）。路线限定为环境威胁、spawn、AI、守恒、生命周期；候选经主代理人工复核 + 本机反方子代理两轮默认怀疑裁决后保留（PR #847 merged）。升 active 时经 §8.1 决议收口（实地 grep `qi_physics::ledger` / `npc::skull_fiend` / `fauna::mimic_spider` / `fauna::rat_phase` / `npc::spawn::ambient_scheduler` 全量核实决议数据，非拍脑袋）。promote 阶段博弈守恒 blocker 二次收口（§8.1 #3）追加 grep `world::heartbeat::zone_qi_inflow_tick` / `world::pseudo_vein_runtime` / `cultivation::tick`，实地核实"字段权威、ledger 账户为镜像"范式后定案方案 A。
