# BugHunt: 全力一击蓄力被打断时 60% 真元退还未从灵域扣回，净铸真元

## Bug 摘要

**critical**（skeptic 维持原判 unchanged，未调整）。

`server/src/cultivation/full_power_strike.rs::charge_interrupt_system`（500-534 行）在蓄力被打断时，把 `charging.qi_committed * 0.6` 直接加回施法者 `cultivation.qi_current`（516-520 行），却完全没有从任何真元存量里扣回这笔钱。而与之对称的蓄力累积路径 `charge_tick_system`（294-333 行）每 tick 扣玩家真元时，**确实**通过 `charge_tick_release_qi_to_zone`（339-464 行）把等量真元真实写入了施法者所在 `zone.spirit_qi`（或溢出账户）——这一半是守恒正确的（由 PR #689 / commit `d64d582b5` 修复）。两条路径的不对称造成：只要蓄力过程中被命中打断一次，世界就净增 `0.6 × qi_committed` 的真元，凭空铸造，属于 `CLAUDE.md` 明文列出的 `cultivation.qi_current += X（无对应 zone 减）` 红旗模式。

## 实际游玩体验影响

`bao_mai.full_power_charge` / `full_power_release` 是通过标准 hotbar SkillBar 注册的玩家常规技能（`combat/baomai_v3/skills.rs` 静态依赖注册，非 dev 命令）。任何玩家都能反复执行「蓄力 → 让自己被轻微命中（哪怕是野怪一爪、PVP 对手一刀、甚至环境伤害）打断 → 真元多出 60%」的循环，且每次循环几乎零成本、零风险（只需承受一次小额伤害）。这是一个可重复、可无限刷的真元铸造漏洞，直接破坏"全服灵气总量恒定"这条最高优先级世界观正典（`worldview.md` 真元守恒设定），长期运行会让服务器全局真元总量单调上涨，稀释所有其他玩法（炼丹、渡劫、灵域浓度经济）赖以运作的稀缺性假设。

## 证据定位

- `server/src/cultivation/full_power_strike.rs:62-68` — `ChargingState` 组件定义：只有 `slot` / `started_at_tick` / `qi_committed` / `target_qi`，**没有**任何字段记录"这笔 `qi_committed` 实际转移去了哪个 zone / overflow 账户、转移了多少"。
- `server/src/cultivation/full_power_strike.rs:294-333` — `charge_tick_system`：每 tick 从 `cultivation.qi_current` 扣除 `to_consume`（318-320 行），随后**必须**调用 `charge_tick_release_qi_to_zone` 把等量真元写回 zone（324-331 行注释明写"守恒：每 tick 消耗的真元必须归还 zone（或 overflow），以防真元从全局 ledger 凭空消失"）。
- `server/src/cultivation/full_power_strike.rs:339-464` — `charge_tick_release_qi_to_zone`：真实调用 `qi_release_to_zone`（`qi_physics::release`）并把 `zone.spirit_qi` 按 `outcome.zone_after / QI_ZONE_UNIT_CAPACITY` 写回（369-370 行）；zone 满载时把溢出部分路由到具名 `QiAccountId::overflow(...)` 账户（372-402 行）。这一路径已经过 PR #689（commit `d64d582b5`，"fix(qi): full_power_strike 蓄力tick消耗真元归还zone（守恒泄漏）"）修复，是本仓库现存唯一"记得"真元去向的地方，但记忆只存在于函数调用栈里，**没有持久化回 `ChargingState`**。
- `server/src/cultivation/full_power_strike.rs:500-534` — `charge_interrupt_system`：函数签名（500-506 行）只有 `Res<CombatClock>`、`Commands`、`EventReader<CombatEvent>`、`Query<&ChargingState>`、`Query<&mut Cultivation>`、`EventWriter<ChargeInterruptedEvent>`——**没有** `ResMut<ZoneRegistry>`，也没有 `EventWriter<QiTransfer>`。核心铸造点在 516-520 行：
  ```rust
  let qi_refunded = charging.qi_committed * 0.6;
  let qi_lost = (charging.qi_committed - qi_refunded).max(0.0);
  if let Ok(mut cultivation) = cultivations.get_mut(event.target) {
      cultivation.qi_current =
          (cultivation.qi_current + qi_refunded).clamp(0.0, cultivation.qi_max);
  }
  ```
  没有任何一行从 zone 或 overflow 账户里扣掉这笔 `qi_refunded`。
- `server/src/network/full_power_emit.rs:66-85` — `ChargeInterruptedEvent` 的**唯一**消费者 `emit_full_power_charging_clear_payloads`：只做 HUD 清理（`send_charging_clear`）和停止蓄力动画（`stop_windup_charge_anim`），确认没有任何下游系统会替 `charge_interrupt_system` 补上 zone 扣回。
- `server/src/cultivation/full_power_strike.rs:953-1002` — 现有测试 `charge_interrupted_by_damage_refunds_60_percent_qi`：只断言 `qi_current` 从 50 涨到 110、`event.qi_refunded == 60.0`、`event.qi_lost == 40.0`，**没有任何 `ZoneRegistry` / `zone.spirit_qi` 断言**，掩盖了这个缺口。
- `server/src/cultivation/full_power_strike.rs:1005-1044` — 第二条测试 `charge_interrupted_by_multiple_hits_refunds_once` 同样只测"多次命中只退款一次"，不涉及 zone。
- `server/src/cultivation/full_power_strike.rs:1257-1306` — 对照组 `charge_tick_credits_zone_qi_when_caster_in_zone`（PR #689 引入的 SG-002 守恒修复测试）证明蓄力累积侧**有**完整的 zone 断言写法，恰恰是打断侧缺失的同款测试没有被补上。
- `docs/finished_plans/plan-baomai-v2.md:256-266` — 设计文档原文伪代码：`qi_refunded = (qi_committed as f32 * 0.6) as u32 ... caster.qi_current += qi_refunded`——这是 qi_physics ledger 引入**之前**的原始设计，PR #689 只补了 tick-drain 一侧的守恒，从未回头补打断侧，历史遗留的不对称就此定型。

## 触发路径

1. 玩家在任意场景对 hotbar 上的 `bao_mai.full_power_charge` 发起蓄力。
2. `charge_tick_system` 每 tick 扣玩家 `qi_current`，并通过 `charge_tick_release_qi_to_zone` 把等量真元真实写入施法者所在 zone 的 `spirit_qi`（或溢出账户）——世界总真元此刻仍然守恒。
3. 蓄力过程中，`charge_interrupt_system` 监听的是**任意来源**的通用 `CombatEvent`（`combat/events.rs` 定义，任何攻击者/任何伤害来源都会触发，包括 PVP、野怪、环境伤害）。玩家只需承受一次轻微命中即可打断蓄力。
4. `charge_interrupt_system` 用 `charging.qi_committed * 0.6` 直接把真元加回玩家 `qi_current`，但第 2 步里已经转移进 zone/overflow 的那部分真元**没有被扣回**。
5. 结果：世界净增 `0.6 × qi_committed` 真元。玩家可无限重复"蓄力 → 被轻微命中打断"循环刷真元，无需 dev 命令、无需越权指令，完全在标准玩法路径内。

## 反方审查记录

- 第一轮质疑：
  - 逐行核对 `charge_tick_system` 是否真的把消耗量写回 zone——确认 PR #689（commit `d64d582b5`）已经把这一半修复到位，注释和实现均自洽，怀疑本发现是不是"旧账"已经被修过；核对 `charge_interrupt_system` 函数签名，确认它确实完全没有 `ZoneRegistry`/`QiTransfer` 相关参数，判定这半边守恒缺口依然存在。
  - 追查 `ChargeInterruptedEvent` 的下游消费者，怀疑会不会有另一个 system 替它补扣 zone——`network/full_power_emit.rs::emit_full_power_charging_clear_payloads` 是唯一消费者，只做 HUD/动画清理，排除"下游兜底"的可能。
  - 追查设计历史 `docs/finished_plans/plan-baomai-v2.md:256-266`，确认打断退还逻辑从一开始就是 `qi_physics` ledger 引入前的裸 `+=` 设计，PR #689 只重构了 tick 消耗侧，从未触碰打断侧，解释了不对称的成因。
- 第二轮补证（查重 + 可达性）：
  - 全仓 grep `docs/plan*`、`docs/plans-skeleton/*`、`docs/finished_plans/*` 中 "full_power" / "charge_interrupt" / "qi_refunded" / "全力一击" 关键词：唯一命中的在跑 plan 是 `docs/plan-bughunt-full-power-charging-session-bleed-v1.md`——经打开确认其主题是"断线后客户端 `FullPowerStateStore` HUD 残留",完全是另一个失效模式（client-only，无 qi/zone 守恒内容），不与本发现重叠；已归档的 `baomai-v1..v4` 系列 plan 只记录了本 bug 的"原始设计"，不是已修复证据。
  - 核实可达性：`bao_mai.full_power_charge`/`full_power_release` 经 `combat/baomai_v3/skills.rs` 静态依赖注册为标准 SkillBar 技能，非 dev-only 命令；`charge_interrupt_system` 监听的 `CombatEvent` 完全通用（任意攻击者/任意伤害来源），玩家反复"蓄力→被轻微命中打断"即可无成本套利，不需要越权或特殊前置条件。
  - 终裁：**通过，critical 不变**。这是真实的、可重复触发的真元凭空铸造漏洞，且修复不应扩大范围（不改变"打断退还 60%"这条设计本身，只补齐它必须从真实存量扣回的守恒义务）。

主循环复核：已亲读关键行确认（`full_power_strike.rs:62-68` / `294-333` / `339-464` / `500-534` / `516-520` / `953-1002` / `1005-1044` / `1257-1306`，`network/full_power_emit.rs:66-85`，`docs/finished_plans/plan-baomai-v2.md:256-266`；git log 核实 commit `d64d582b5`（PR #689）与当前 HEAD `b398c4071` 均存在于历史中）。

## Skeleton Fix Plan

- [ ] 在 `ChargingState`（`full_power_strike.rs:62-68`）新增真元沉积台账字段，例如：
  ```rust
  pub qi_deposits: Vec<QiDepositRecord>,
  ```
  其中 `QiDepositRecord { pub account: QiAccountId, pub amount: f64 }`（新类型，`Debug + Clone + PartialEq`），累计记录每个 tick 真元实际转移到的账户（zone 或 overflow）及金额。允许同一次蓄力产生多条记录（玩家蓄力途中跨 zone 移动、zone 从未满变满等场景）。
- [ ] 改造 `charge_tick_release_qi_to_zone`（339-464 行）：在每个真正落地写入 `zone.spirit_qi` 或发出 overflow `QiTransfer` 的分支后，把对应的 `(QiAccountId, amount)` 追加进调用方传入的 `&mut Vec<QiDepositRecord>` 出参；`charge_tick_system`（294-333 行）从 `charging.qi_deposits` 借出这个 `Vec` 传入。
- [ ] 新增守恒对称函数（如 `withdraw_qi_from_deposits`，与 `charge_tick_release_qi_to_zone` 同文件相邻放置），输入目标退还量 `qi_refunded: f64` 与 `&[QiDepositRecord]`，**从台账尾部开始**（最近沉积优先，或按占比均摊，二选一但要写清楚并测试）依次扣回：
  - 目标是 zone 账户：`zones.find_zone_mut(zone_name)`；换算方式与写入方向相反：`zone.spirit_qi = ((zone.spirit_qi * QI_ZONE_UNIT_CAPACITY) - amount).max(<该 zone 台账记录的下限>) / QI_ZONE_UNIT_CAPACITY`。**禁止**扣穿到台账记录范围之外的真实存量——如果 zone 已被其他系统（天道每时代衰减、其他玩家的采集/开光等）动过导致可扣余量不足，按 `fix_sketch` 退化：能扣多少扣多少，差额记入 `qi_lost` 而非硬塞进 `qi_refunded`，绝不倒扣出负值制造新的凭空。
  - 目标是 overflow 账户：由于本模块的 overflow 记账目前只是审计事件（未见任何 system 消费 `EventReader<QiTransfer>` 把它落地成可查询余额），退还时只需发一条对称的 `QiTransfer(from=overflow:<key>, to=player:<entity_bits>, reason=ChargeInterruptRefund)` 供审计，与沉积路径的"只发事件"对称，不引入新的不对称语义。
  - 返回值必须是**实际可确认扣回的量**，不能大于请求的 `qi_refunded`。
- [ ] 改造 `charge_interrupt_system`（500-534 行）签名，追加 `mut zones: ResMut<ZoneRegistry>` 与 `mut qi_transfer_writer: EventWriter<QiTransfer>`（比照 `charge_tick_system` 的资源依赖）。用上面的对称函数把 516 行原本裸算的 `charging.qi_committed * 0.6` 换成"先算出理论退还额度，再从 `charging.qi_deposits` 里实际扣回，取两者较小值"；`cultivation.qi_current` 只加实际扣回成功的部分；`ChargeInterruptedEvent.qi_refunded` / `qi_lost` 字段值必须反映扣回后的真实数字（server 是唯一权威，client HUD 展示只能跟随，不得自行估算）。
- [ ] 在 `qi_physics::ledger::QiTransferReason`（紧邻现有 `TiandaoCondense` / `DuguReturnToZone` 等 zone→其他账户方向变体旁）新增 `ChargeInterruptRefund` 变体，文档注释按仓库惯例写清守恒约束：
  ```
  /// 全力一击蓄力被打断时，把已沉积进 zone/overflow 的真元按已记录台账扣回并归还玩家。
  ///
  /// 守恒约束：
  ///   - zone.spirit_qi -= 实际退还量（或对应 overflow 账户扣减）；
  ///   - player.qi_current += 同一实际退还量（不得大于台账可确认扣回的量）；
  ///   - QiTransfer(from=zone:<name>/overflow:<key>, to=player:<entity_bits>, reason=ChargeInterruptRefund)；
  ///   - 台账不足时差额计入 qi_lost，绝不多退、绝不凭空铸造。
  ChargeInterruptRefund,
  ```
- [ ] 更新已存在的两条手工构造 `ChargingState` 的测试（`953-1002` 的 `charge_interrupted_by_damage_refunds_60_percent_qi`、`1005-1044` 的 `charge_interrupted_by_multiple_hits_refunds_once`），让它们改为**先跑一遍 `charge_tick_system` 走真实沉积路径再打断**（而不是直接手塞 `qi_committed`），确保测试覆盖真实生产链路而非只测半截。
- [ ] 明确**不改动**成功释放路径（`release_full_power_with_exhaust`，215-291 行）——release 时 `qi_committed` 早已 100% 转化为攻击强度输入，这是"蓄力期间真元持续外泄进环境、`qi_committed` 只是意图强度计数"的既定设计（与 `zhenmai_v2`/`baomai_v3` 一致），不属于本 bug 范围，不应借机重新设计。

## 验收测试计划

全部落在 `server/` cargo test（`cd server && cargo test full_power_strike`）：

- **happy path（单 zone 内完整闭环）**：构造一个干净的 `spirit_qi = 0.0` 的 zone，施法者站在 zone 内，先跑 N 个 tick `charge_tick_system` 让 `qi_committed` 累积到某个值（同时校验 `qi_deposits` 台账被填充、`zone.spirit_qi` 按 `charge_tick_credits_zone_qi_when_caster_in_zone`（1257-1306 行）同款断言方式上升），再触发一次 `CombatEvent` 跑 `charge_interrupt_system`，断言：`player.qi_current` 增加量 == `zone.spirit_qi` 减少量（换算回 raw 单位）== 理论 60% 退还额度；`player.qi_current + zone.spirit_qi*QI_ZONE_UNIT_CAPACITY` 打断前后总和不变（用 `qi_physics::ledger::assert_conservation` 或等价的手工 `WorldQiSnapshot` 前后对拍，不可写字面魔法数）。
- **边界：蓄力刚开始即被打断**（`qi_committed == 0.0`，尚未有任何 tick 执行）：断言 `qi_refunded == 0.0`、`qi_lost == 0.0`、`zone.spirit_qi` 完全不变（无沉积记录，退还函数应对空台账正确返回 0）。
- **边界：蓄力到 `target_qi` 上限（完全蓄满）后被打断**（未及时释放）：断言按满额 `qi_committed` 计算的 60% 能被完整从 zone 扣回。
- **错误分支：overflow 场景**（zone 提前灌满至 `spirit_qi = 1.0`，蓄力期间真元全部路由进 overflow 账户）：打断后断言 `player.qi_current` 仍然正确增加，且发出的 `QiTransfer` 是 `from=overflow:<key>` 而不是错误地试图从满载 zone 里再扣一次。
- **错误分支：台账不足（外部系统在蓄力期间清空了 zone）**——模拟蓄力期间另一个 system 把 `zone.spirit_qi` 手动清零（模拟天道衰减/其他玩家开光抽干），打断时断言：实际退还给玩家的量 `<=` zone 当前可扣余量、`qi_lost` 相应增大、**不产生负的 `zone.spirit_qi`**、不静默地"还是按 60% 全额退还"。
- **状态转换**：`ChargingState` 打断后仍被正确 `remove`（既有断言保留）；`charge_interrupted_by_multiple_hits_refunds_once`（1005-1044 行）场景改造后仍应只退还一次（`interrupted_this_tick` 去重逻辑不受影响）；补一条"两次连续蓄力-打断循环"测试，断言第二轮蓄力开始前 `zone.spirit_qi` 与第一轮打断后的值一致（证明没有跨蓄力周期的状态泄漏）。
- **回归**：`charge_tick_credits_zone_qi_when_caster_in_zone`（1257-1306）与 `charge_tick_with_full_zone_emits_overflow_transfer_not_credits_zone`（1309 起）两条既有 PR #689 守恒测试必须继续通过，证明本次修复不破坏已有的 tick-drain 守恒路径。
- **跨栈**：本 bug 纯 server 逻辑，客户端 `network/full_power_emit.rs` 只读 `ChargeInterruptedEvent` 的 `caster`/`at_tick` 字段做 HUD 清理，字段类型不变，无需改 client；若 `ChargeInterruptedEvent.qi_refunded` 数值变化影响任何 HUD 展示文案，需要在 `client/` 补一条 `./gradlew test` 层面的展示值断言（若现存 HUD 有展示该数字的话，先确认是否存在，没有则跳过并在 plan 里注明）。

## 风险

- 台账扣回逻辑必须严格保证"退还给玩家的量 == 从 zone/overflow 实际扣回的量"，如果实现时图省事直接扣回原始 `qi_committed * 0.6` 而不做"实际可扣回量"的 clamp，等于换了个位置的凭空铸造（把红旗从"直接加钱"挪到"假装扣了但其实没扣够"），必须靠上面"台账不足"那条测试卡死。
- `QiAccountId::overflow` 账户当前在本模块里只是审计事件、没有被任何 system 落地成可查询余额（全仓未见 `EventReader<QiTransfer>` 消费者）；本 fix 的 overflow 分支只能做到"发对称审计事件"而不能做"验证 overflow 账户真实还有这么多余额可扣"——这是仓库既有的更大缺口（overflow 记账本身不闭环），不在本 plan 范围内展开重构，只要求新代码不引入比现状更差的不对称即可。
- 不得借本次修复顺手改变"打断退还 60%"这个数值设计本身（那是 `plan-baomai-v2.md` 定的游戏性数值，不属于守恒 bug），也不得把成功释放路径（`release_full_power_with_exhaust`）一并"顺手"改造成从 zone 扣钱——release 侧的 `qi_committed` 早已在蓄力阶段真实转移完毕，是另一套设计，混在一起改会扩大 PR 范围、增加 review 负担。
- 修改 `charge_interrupt_system` 函数签名（新增 `ResMut<ZoneRegistry>`/`EventWriter<QiTransfer>`）需要确认 `register()`（124-150 行）里的 system 调度顺序（`CombatSystemSet::Resolve`，在 `resolve_attack_intents` 之后）不会因为新增的 `ResMut<ZoneRegistry>` 与 `charge_tick_system`（`CombatSystemSet::Intent`）产生 Bevy ECS 调度冲突/借用冲突——两者本就在不同 SystemSet 且顺序上 Intent 先于 Resolve，理论上安全，但需要跑 `cargo test` 全量确认没有新的 ambiguity 警告。
