# BugHunt: 延寿丹与断续散缩容真元上限后 clamp 差额真元蒸发

## Bug 摘要

**严重度：critical（延寿丹分支，skeptic 由 high 调整为 critical）+ medium（断续散分支，unchanged）**

两处同根因缺陷合并为一份 plan：玩法中「服丹缩容 `qi_max`」场景下，代码用 `cultivation.qi_current = cultivation.qi_current.min(cultivation.qi_max)` 把超出新上限的真元直接截断丢弃，从未把这笔差额写回 `zone.spirit_qi` 或 `qi_physics::ledger::QiTransfer`，是纯粹的原地覆盖式真元蒸发。

1. **延寿丹（`life_extension_pill`，critical）**：`server/src/cultivation/lifespan.rs::apply_extension_cost` 在 `cost.qi_cap_delta < 0.0` 时先收缩 `qi_max` 再 clamp `qi_current`，差额没有任何去向。该函数挂在正式 Update schedule 上（`server/src/cultivation/mod.rs:522`），触发源是真实可制作的传说品阶延寿丹，非 dev-only 命令。仓库自己的既有测试 `lifespan_extension_pill_cost_increases_with_accumulated_extension` 恰好把这个销毁行为断言成"预期结果"，从未检查真元去了哪。
2. **断续散（`duan_xu_san`，medium）**：`server/src/network/client_request_handler.rs::apply_combat_pill_runtime` 的 `CombatPillKind::DuanXuSan` 分支执行几乎相同的 `qi_max *= 0.97` 缩容 + clamp，同样无任何回灌。同一文件里的其它战斗丹分支（HuoXueDan/XuGuGao/SuoDiSan/HuGuSan）都只动 `wounds`/`stamina`，唯独 DuanXuSan 顺带动了真元账本却没配套的守恒记账；`git log -S CombatPillKind::DuanXuSan` 显示该行来自旧提交 `3242dc08b`（plan-alchemy-combat-v1 战场丹药十方 #229），早于 2026-06-23/24 真元守恒清扫（PR#676-683），从未被那轮清扫覆盖到。

两处都违反 `CLAUDE.md`「真元/灵气守恒律」明文列出的红旗：「容器/衰变把真元凭空消失不归还 zone」。项目里已有正确先例——`server/src/cultivation/race_change.rs:192-347` 在完全相同的「qi_max 缩水导致 qi_current 需要 clamp」场景下，是先算出被 clamp 的差额、再构造 `QiExcessReleasePlan`、通过 `QiTransfer::new(..., ReleaseToZone)` 写回 `zone.spirit_qi`（或无 zone 时路由到 overflow 账户）——lifespan.rs 和 client_request_handler.rs 的这两个分支是这条正确模式里的漏网之鱼。

## 实际游玩体验影响

- 玩家吃「延寿丹」延寿的同时会永久性缩小真元上限（这是设计内代价），但若吃丹时真元接近满值，超出新上限的那部分会被直接抹除且**不进入任何 zone**——相当于每次续命都在悄悄抽走全服灵气总量的一部分，长期多次续命的老玩家会让 `SPIRIT_QI_TOTAL` 持续净流失，且没有任何日志/审计能追溯这笔损耗去哪了。
- 玩家在战斗中断肢后服用「断续散」疗伤时，如果当时真元接近满值，同样会在接骨的同时无声蒸发一小笔真元；虽然单次量级较小（`qi_max` 只降 3%），但战斗丹是高频消耗品，累积效应仍会持续拉低服务器灵气总量。
- 两处都是"看不见的税"：玩家界面上看到的只是"真元上限变小了"，不会意识到真元被凭空销毁，而 zone 侧本该获得的真元回灌也从未发生，破坏了 worldview 里"灵气总量恒定、消耗必有去处"的经济闭环。

## 证据定位

- `server/src/cultivation/lifespan.rs:732-749`（`apply_extension_cost` 全函数；破坏性 clamp 主体在 738-741：`cultivation.qi_max = (cultivation.qi_max * factor).max(1.0); cultivation.qi_current = cultivation.qi_current.min(cultivation.qi_max);`）
- `server/src/cultivation/lifespan.rs:79-96`（`ExtensionContract` trait 默认 `cost()`/`qi_cap_cost_factor()`，产出 `qi_cap_delta < 0.0`）
- `server/src/cultivation/lifespan.rs:111-123`（`PillExtensionContract` 实现，`source()` = `"life_extension_pill"`，`qi_cap_cost_factor()` = `LIFESPAN_EXTENSION_PILL_QI_MAX_COST_PER_YEAR`，`lifespan.rs:40` 定义为 `0.01`）
- `server/src/cultivation/lifespan.rs:671-684`（`extension_contract_from_source`：除 `"enlightenment_extension"`/`"collapse_core"` 外，任何 source——包括真实延寿丹——都落到会产出 `qi_cap_delta<0` 的 `PillExtensionContract` 分支）
- `server/src/cultivation/lifespan.rs:525-634`（`process_lifespan_extension_intents` 系统；查询元组只有 `LifespanComponent`/`LifespanExtensionLedger`/`Cultivation`/`PlayerState`/`LifeRecord`/`Lifecycle`，缺 `Position`/`CurrentDimension`/`ZoneRegistry`/`Events<QiTransfer>`）
- `server/src/cultivation/lifespan.rs:1338-1385`（既有测试 `lifespan_extension_pill_cost_increases_with_accumulated_extension`：`qi_current: 100.0, qi_max: 100.0` 服丹后 `qi_max` 跌破 90，`assert_eq!(cultivation.qi_current, cultivation.qi_max)`——把销毁行为断言成预期结果，没有检查差额去向）
- `server/src/cultivation/mod.rs:522`（`process_lifespan_extension_intents.after(lifespan_aging_tick)` 挂在真实 Update schedule，非 dev-only）
- `server/src/network/client_request_handler.rs:17046-17058`（`ItemEffect::LifespanExtension` 分支：`take_pill` 发出 `LifespanExtensionIntent{source: source.clone()}`，`source` 取自道具配置）
- `server/assets/items/pills.toml:68-78`（`life_extension_pill` 真实生产道具：`effect = { kind = "lifespan_extension", magnitude = 10, target = "life_extension_pill" }`，legendary 品阶，非 dev 专属）
- `server/src/network/client_request_handler.rs:17337-17346`（`CombatPillKind::DuanXuSan` 分支：`next_cultivation.qi_max = (next_cultivation.qi_max * 0.97).max(0.0); next_cultivation.qi_current = next_cultivation.qi_current.min(next_cultivation.qi_max);`）
- `server/src/network/client_request_handler.rs:17382-17385`（`touched_cultivation` 为真时 `commands.entity(entity).insert(next_cultivation.clone())` 直接写回 ECS，无任何守恒记账）
- `server/src/network/client_request_handler.rs:17270-17402`（`apply_combat_pill_runtime` 全函数体内 `release_qi_amount_to_zone`/`QiTransfer`/`zone.spirit_qi` 零命中）
- `server/src/alchemy/pill.rs:229`、`285-286`（`CombatPillKind::DuanXuSan` 枚举 + `name: "断续散"` 定义）
- `server/assets/items/pills.toml:262-271`（`duan_xu_san` 真实生产道具：`effect = { kind = "combat_pill", magnitude = 0, target = "duan_xu_san" }`）
- `server/src/network/client_request_handler.rs:225-`（`CombatRequestParams` struct：有 `positions`/`unique_ids`/`buff_tx` 等字段，但缺 `Query<&CurrentDimension>`/`Option<ResMut<ZoneRegistry>>`/`Option<ResMut<Events<QiTransfer>>>`；文件顶部 178/191 行已 `use crate::world::dimension::{CurrentDimension, DimensionKind}` 与 `use crate::world::zone::{ZoneRegistry, ...}`，接线成本低）
- `server/src/cultivation/race_change.rs:192-228`（**正确先例**：`new_qi_max` 算出后先 `excess = (cultivation.qi_current - new_qi_max).max(0.0)`，再查 `Position`/`CurrentDimension`/`ZoneRegistry` 构造 `QiExcessReleasePlan`）
- `server/src/cultivation/race_change.rs:311-350`（`apply_qi_excess_release`：把 `transfer.accepted` 写回 `zone.spirit_qi` 并 `emit QiTransfer(..., ReleaseToZone)`，`transfer.overflow` 走无上限 overflow 账户；此函数是 `fn`（非 `pub fn`），仅限 race_change.rs 内部复用）
- `server/src/cultivation/death_hooks.rs:278-338`（**已存在、已导出的共用 helper** `pub fn release_qi_amount_to_zone(entity, amount, position, current_dimension, life_record, zones, qi_transfers, source) -> f64`：Position/ZoneRegistry/CurrentDimension 任一缺失都自动 fallback 到 overflow 账户，语义与 race_change.rs 的 fallback 完全一致，两处修复应共同调用这一个函数，不再各写一份）
- `server/src/cultivation/lifespan.rs:7`（`use super::death_hooks::{CultivationDeathCause, CultivationDeathTrigger};`——`death_hooks` 模块已被 lifespan.rs 部分 import，接入 `release_qi_amount_to_zone` 只需扩展 use 列表）

## 触发路径

**延寿丹分支**：
1. 玩家持有并服用真实生产道具「延寿丹」（`life_extension_pill`，legendary，非 dev 专属）。
2. `take_pill` 处理 `ItemEffect::LifespanExtension`（`client_request_handler.rs:17046-17058`），发出 `LifespanExtensionIntent{source: "life_extension_pill"}`。
3. `process_lifespan_extension_intents`（挂在真实 Update schedule，`mod.rs:522`）读取 intent，`extension_contract_from_source` 落到 `PillExtensionContract`（`lifespan.rs:671-684`），产出 `qi_cap_delta < 0.0`。
4. `apply_extension_cost`（`lifespan.rs:732-749`）先按比例收缩 `cultivation.qi_max`，再用 `.min()` 把 `qi_current` clamp 下来——若玩家服丹前真元接近满值，被截断的差额直接消失，不写回任何 zone/ledger。

**断续散分支**：
1. 玩家在战斗中身体部位被"断"（severed），随后服用真实生产道具「断续散」（`duan_xu_san`）。
2. C2S `AlchemyTakePill` → `handle_alchemy_take_pill` → `apply_combat_pill_runtime`（`client_request_handler.rs:17270-17402`）。
3. `CombatPillKind::DuanXuSan` 分支先接骨（`apply_severed_mend`），再执行 `qi_max *= 0.97` + clamp `qi_current`（17341-17343）。
4. `touched_cultivation` 为真时把 `next_cultivation` 整体写回 ECS（17383），clamp 掉的差额同样无声消失。

## 反方审查记录

**延寿丹分支**：
- 第一轮质疑：这是不是代码库里"qi_max 缩容 + clamp"的通用惯例、本就豁免守恒记账？核对同代码库里完全同构的场景——`race_change.rs` 处理相同的"qi_max 缩水后 clamp qi_current"，是显式算出差额并通过 `QiExcessReleasePlan` → `apply_qi_excess_release` 写回 `zone.spirit_qi`（或 overflow）；`sword_path/systems.rs::sword_shatter_system` 文档注释直接写明"守恒律：`stored_qi = backlash_qi_current + qi_released_to_zone`"；`combat/dugu_v2`、`combat/baomai_v3` 的等价场景也都走 zone release。lifespan.rs 是这条既有模式里唯一的漏网模块，不是设计豁免。
- 核对可达性：`process_lifespan_extension_intents` 挂在正式 Update schedule（`mod.rs:522`），触发源是真实可制作、legendary 品阶的延寿丹道具，非 dev-only 命令、非死代码。
- 核对是否已被记录：grep 全部 in-flight plan basename 与开放 PR，均无 lifespan/续命丹/life_extension_pill 相关命中；`docs/finished_plans/plan-lifespan-v1.md` 自报的 P3 只承诺"`qi_max` 永久扣除代价曲线单测"，未提及要把超额 `qi_current` 释放回 zone，确认这是从未被设计到、从未被前几轮真元守恒清扫（PR#676-683）覆盖的缺口。
- 让步：本轮未新增测试，为静态代码路径复现；已有测试 `lifespan_extension_pill_cost_increases_with_accumulated_extension` 恰好锁死了这个销毁行为的"预期结果"，需要在修复时同步改写断言。
- 终裁：严重度由 high 上调为 critical——虽然单次绝对量相对 `SPIRIT_QI_TOTAL` 不算巨大，但这条路径精确命中 `CLAUDE.md` 明文标注为"最高优先级硬约束、吞真元 = 阻塞合并"的红旗定义（"容器/衰变把真元凭空消失不归还 zone"），按项目自己的分级规则理应上调。

**断续散分支**：
- 第一轮质疑：逐一核对代码库里所有可比的"qi_max 缩容 + clamp"调用点——`combat/dugu_v2/skills.rs`（`apply_qi_max_loss` + `qi_release_to_zone`）、`combat/baomai_v3/skills.rs`（loss_ratio 缩容 + 显式 zone release）、`sword_path/skill_register.rs`（天门 `qi_max * HEAVEN_GATE_QI_MAX_RETAIN` + 显式 `QiTransfer::ReleaseToZone`）、`sword_path/systems.rs::sword_shatter_system`——无一例外都把缩容差额释放回 zone/ledger。DuanXuSan 是这条一致模式里唯一的例外，确认是遗漏而非有意设计。
- 核对是否已有通用兜底：未发现任何 `Changed<Cultivation>` 之类的全局对账系统能自动纠正这类静默覆盖——只找到纯 HUD-emit 观察者，不做修正。
- 核对起源：`git log -S "CombatPillKind::DuanXuSan"` 定位到旧提交 `3242dc08b`（plan-alchemy-combat-v1 战场丹药十方 #229），早于 2026-06-23/24 那轮系统性真元守恒清扫（PR#676-683），从未被那轮清扫触碰。
- 核对是否与在跑 plan 重复：`docs/plans-skeleton/plan-bughunt-combat-pill-toxin-gate-v1.md` 覆盖同一文件/同一函数区域，但是完全不同的失败模式（缺失 `can_take_pill` 毒性阈值门禁，与真元守恒无关）；`docs/plan-bughunt-qi-recovery-consumable-ledger-v1.md`（active）覆盖 `ItemEffect::QiRecovery` 丹药无源头凭空铸造真元，方向相反（创造而非蒸发），且明确限定 `QiRecovery` 类道具，不涉及 `CombatPillKind::DuanXuSan`。两者均无重叠，本 finding 未被已知覆盖。
- 终裁：严重度维持 medium（unchanged）——触发条件有前提（仅当服丹时 `qi_current` 处于新 `qi_max` 之上才会销毁，不是每次断肢疗伤必然命中），与代码库里其它已修复的同类缺口量级相当，不上调。

主循环复核：已亲读关键行确认。

## Skeleton Fix Plan

统一原则：**两处共用同一个已存在的公开 helper `crate::cultivation::death_hooks::release_qi_amount_to_zone`**（`death_hooks.rs:278-338`，已内建 Position/ZoneRegistry/CurrentDimension 缺失时的 overflow 兜底），照抄 `race_change.rs:192-347` 的"先算差额、再释放"设计，**禁止各自另写一份找 zone + 写 `zone.spirit_qi` + emit `QiTransfer` 的逻辑**。

- [ ] `lifespan.rs::apply_extension_cost`：在收缩 `qi_max` 前记录 `qi_current_before = cultivation.qi_current`，clamp 后计算 `overflow = (qi_current_before - cultivation.qi_current).max(0.0)`，返回给调用方（或直接在函数内完成释放，视资源可用性决定签名形态）。
- [ ] 扩展 `process_lifespan_extension_intents`（`lifespan.rs:525`）的系统签名，穿入 `Query<&Position>`、`Query<&CurrentDimension>`、`Option<ResMut<ZoneRegistry>>`、`Option<ResMut<Events<QiTransfer>>>`（`ZoneRegistry` 已在文件顶部 import，`death_hooks` 模块也已部分 import，扩展 use 列表即可）。
- [ ] `overflow > 0.0` 时调用 `death_hooks::release_qi_amount_to_zone(entity, overflow, position, current_dimension, life_record.as_deref(), zones.as_deref_mut(), qi_transfers.as_deref_mut(), "life_extension_pill_shrink")`，不再由 `.min()` 静默吞掉。
- [ ] 修正既有测试 `lifespan_extension_pill_cost_increases_with_accumulated_extension`（`lifespan.rs:1338-1385`）：不能只断言 `qi_current == qi_max`，必须补上"差额已经通过某条可观察路径释放"的断言（提供 `Position`+`ZoneRegistry` 时断言目标 `zone.spirit_qi` 增加且对应量的 `QiTransfer(ReleaseToZone)` 事件被 emit；若测试场景未 spawn 这些资源，断言 fallback 到 overflow 账户的等价审计事件被 emit，而不是让差额彻底消失于断言之外）。
- [ ] `client_request_handler.rs::CombatRequestParams`（或专为 `apply_combat_pill_runtime` 传参的更窄结构）补充 `Query<&CurrentDimension>`、`Option<ResMut<ZoneRegistry>>`、`Option<ResMut<Events<QiTransfer>>>`（文件顶部 178/191 行已 `use` 对应类型，接线成本低）。
- [ ] `apply_combat_pill_runtime` 的 `CombatPillKind::DuanXuSan` 分支（`17337-17346`）同样记录 `qi_current_before`，clamp 后算 `overflow`，调用同一个 `death_hooks::release_qi_amount_to_zone(entity, overflow, ..., "combat_pill_duan_xu_san_shrink")`。
- [ ] 两处修复均只做"释放差额"，**不修改 `qi_max` 收缩本身的数值/比例**——这是道具的既定代价曲线，本 plan 只补齐守恒记账，不改变现有难度平衡。
- [ ] 本 bug 与 C2S 门禁无关（不是漏检请求合法性问题），纯粹是服务端内部真元记账缺陷；`take_pill`/`AlchemyTakePill` 的合法性校验本就完全在 server 侧完成，本次修复不新增任何 client 侧改动，也不需要额外的 client UX 隐藏——server 对真元流动的计算权威性保持不变，只是补全其应有的一步。
- [ ] 评估是否需要给 `overflow` 事件补充专属 `narration`/日志，便于运营侧观测这条此前完全无声的泄漏路径是否已被彻底堵住（可选，不阻塞本 plan 收口）。

## 验收测试计划

**server/ cargo test（`server/src/cultivation/lifespan.rs` 单测）**：
- happy path：玩家 `qi_current < qi_max`（服丹前真元未满，如 60/100），服用延寿丹后 `qi_max` 按既有公式收缩，`qi_current` 保持原值不变（收缩后仍小于新 `qi_max`），断言**没有**产生任何 `QiTransfer(ReleaseToZone)` 事件（overflow 应为 0，不应该有多余释放）。
- 边界 1（原有测试的修正版）：`qi_current == qi_max == 100.0`（服丹前真元满值），服用后 `qi_max` 跌破 90，断言 `qi_current == qi_max`（既有断言保留）**且**新增断言：若测试场景带 `Position`+`ZoneRegistry`，则目标 zone 的 `spirit_qi` 增加了对应换算量、且恰好一条 `QiTransferReason::LifespanExtensionShrink`（或复用 `ReleaseToZone`）事件被 emit，金额等于 `qi_current_before - qi_current_after`。
- 边界 2：无 `Position`/`ZoneRegistry` 资源时（当前既有测试的 app 配置），断言 overflow 路径被触发（`release_qi_overflow` 等价审计事件），而不是静默吞掉——锁死"缺资源也不能吞真元"这条约束。
- 错误分支：`requested_years == 0` 或 `enlightenment_used` 已耗尽等既有拒绝分支不应触碰真元账本（保持现状不回归）。
- 状态转换：连续两次服用延寿丹（`accumulated_years` 累加后代价曲线更陡），断言第二次同样触发差额释放且金额与新的 `qi_cap_delta` 一致（用 `lifespan_extension_cost_pressure` 公式反推期望值）。
- 断言取值口径：金额比较统一用 `qi_physics::constants` 里的 `QI_EPSILON` 容差，不写字面魔数；守恒断言用 `qi_physics::ledger::assert_conservation` 或等价的"zone 增量 == 差额"手工验证。

**server/ cargo test（`server/src/network/client_request_handler.rs` 单测，`apply_combat_pill_runtime` / DuanXuSan 分支）**：
- happy path：severed body part 存在，`qi_current` 服丹前低于收缩后的新 `qi_max`（如 qi_current=50, qi_max=60→58.2），断言 `qi_max` 按 0.97 收缩、`qi_current` 不变、无 `QiTransfer` 释放事件。
- 边界：`qi_current == qi_max`（真元满值时服断续散），断言收缩后差额通过 `release_qi_amount_to_zone` 释放到对应 zone（提供 `Position`+`CurrentDimension`+`ZoneRegistry` 场景）或 overflow（缺资源场景），两种场景各一条专属 case。
- 错误分支：`worst_severed_part(&wounds)` 返回 `None`（没有断肢）时，`apply_severed_mend` 与真元缩容逻辑现状如何处理需要先读代码确认（若现状是"无 severed part 也照样缩真元"则同样需要覆盖释放；若现状是提前 return 则该分支不触碰真元，测试锁定"不缩容也不释放"）。
- 状态转换：非 DuanXuSan 的其它 `CombatPillKind`（HuoXueDan/XuGuGao/SuoDiSan/HuGuSan）不应触碰 `qi_max`/`qi_current`，回归测试确认这几个分支在本次改动后依旧不产生任何 `QiTransfer` 事件（防止改动误伤其它丹药分支）。
- 契约断言：不测内部调用次数，只测外部可观察结果——`Cultivation` 组件最终值、`zone.spirit_qi` 最终值、`QiTransfer` 事件序列（from/to/amount/reason）。

**跨模块守恒回归（可选，建议）**：
- 若时间允许，补一条"服延寿丹 + 服断续散"组合场景的集成测试，验证连续两种丹药触发的两笔释放各自独立记账，互不覆盖、互不吞并（分别落在各自的 `QiTransferReason` 变体或 source 标签下，便于审计区分）。

## 风险

- `apply_extension_cost` 目前是纯函数（无 ECS 资源访问），扩展签名穿入 `Position`/`ZoneRegistry`/`Events<QiTransfer>` 会牵动其调用方 `process_lifespan_extension_intents` 的 Query 元组——需注意 `Position`/`CurrentDimension` 在部分测试场景（离线批处理角色）可能不存在，必须复用 `death_hooks::release_qi_amount_to_zone` 自带的 overflow fallback，不能假设资源必然齐备。
- `apply_combat_pill_runtime` 当前是纯参数传递的自由函数（非 Bevy system），扩展它需要的 `Query<&CurrentDimension>`/`ZoneRegistry`/`Events<QiTransfer>` 必须从更上层调用链（`handle_alchemy_take_pill` → 更上层的 system）逐层穿入或塞进 `CombatRequestParams`——穿参层数较深，修复时需确认不破坏该函数已有的 `#[allow(clippy::too_many_arguments)]` 参数表可维护性（可考虑打包成一个小的 `QiReleaseContext` struct 传入，而非逐个新增裸参数）。
- 两处修复都不应改变道具本身的代价曲线数值（`qi_max` 收缩比例/年费率），只补记账——若顺手"调平衡"会把一个纯 bug 修复变成数值改动，扩大 review 范围。
- 既有测试 `lifespan_extension_pill_cost_increases_with_accumulated_extension` 修改断言时要小心不要削弱其原有的"缩容曲线随累计年限变陡"这条核心锁定，只是新增守恒相关断言，不能替换掉原有内容。
- 若线上已有大量真实玩家因这两条路径长期蒸发真元，服务器灵气总量可能已经偏离 `SPIRIT_QI_TOTAL` 初值；本 plan 范围只堵住未来的泄漏，不包含历史存量的对账/回填，需要另行评估是否值得追溯修正（大概率不值得，纯记账缺口通常量级很小，可作为已知限制记录而非强制回填）。
