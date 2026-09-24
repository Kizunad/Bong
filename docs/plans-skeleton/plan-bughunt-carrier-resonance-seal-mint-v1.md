# BugHunt: 暗器共鸣封印按比例放大真元却只扣基础量，miss 释放真造真元

## Bug 摘要

**critical**（skeptic 由 high 调整为 critical）：暗器（anqi）充能封印在 `finish_charge` 里按法器共鸣 resonance 把封印真元量放大到最高 1.2×，但玩家账户只被扣了未放大的基础量；这段被凭空放大的差额没有任何账户承担来源，一旦投射物 miss（`OutOfRange`/`NaturalDecay`，正常投掷即可触发，不需要命中任何目标），该差额会被 `qi_release_to_zone` 真实写进 zone.spirit_qi——玩家只需反复对空/远处投掷即可无限刷真元，直接违反 `CLAUDE.md` 明文的全服真元守恒硬约束（`SPIRIT_QI_TOTAL` 恒定）。

## 实际游玩体验影响

任何炼出「法器颜色与自身真元色匹配 + 凹槽已深化」暗器（`BoneChip`/`YibianShougu`/`LingmuArrow`/`DyedBone`/`FenglingheBone`/`ShangguBone` 均适用）的玩家，只需蓄力封印后随手朝空地扔出（无需瞄准、无需命中），每次即可让所在 zone 的灵气浓度凭空上涨（resonance 越接近 1.0，放大比例越接近 20%）。这条路径完全走正常玩法界面（ChargeCarrierIntent/ThrowCarrierIntent 均有真实 C2S 入口与客户端 HUD），没有任何 `/give`/`/qi set` 等 dev 命令参与，任何知道诀窍的玩家都能把自己或宗门的灵脉浓度刷到远超设计上限，破坏全服真元总量恒定这一世界观最高优先级物理法则（worldview.md §二「真元极易挥发」的对偶不变式：流动只能转移，不能无中生有）。持续刷这条路径还会让本应稀缺的高浓度 zone（供修炼吸收速率用）失去经济意义。

## 证据定位

- `server/src/combat/carrier.rs:579-681`（`finish_charge`）：
  - L604-608：`total_deducted`（full_charge 分支 = `charging.qi_target`，即真正从玩家 `Cultivation.qi_current` 扣掉的量）。
  - L609-613：`base_qi_amount`（full_charge 分支同样 = `charging.qi_target`，与 `total_deducted` 恒等）。
  - L614-615：`let resonance = artifact_resonance_for_inventory(...); let qi_amount = carrier_sealed_qi_amount(base_qi_amount, resonance);`——放大后的 `qi_amount` 才是真正写入封印的值。
  - L637-660：`sealed_base_qi = base_qi_amount;`（**未放大**）随后 `CarrierImprint { qi_amount, qi_amount_initial: qi_amount, .. }`（**已放大**）被写入 `store.imprints_by_instance`。
  - L672-679：`release_unsealed_carrier_qi(..., (total_deducted - sealed_base_qi).max(0.0))`——full_charge 下 `total_deducted == base_qi_amount == sealed_base_qi`，这里恒为 `0.0`：放大出来的差额（最高 20%）没有被任何"未封印部分回收"逻辑收回。
- `server/src/combat/carrier.rs:729-734`（`carrier_sealed_qi_amount`）：`base_qi_amount * resonance.map(carrier_seal_efficiency_multiplier).unwrap_or(1.0)`。
- `server/src/forge/resonance.rs:35-37`（`carrier_seal_efficiency_multiplier`）：`(0.8 + 0.4 * resonance.clamp(0.0, 1.0)) as f32`——resonance ∈ [0,1] 时乘数区间 `[0.8, 1.2]`，resonance > 0.5 即净放大（> 1.0）。
- `server/src/combat/carrier.rs:2472-2475`（既有 pin 测试 `carrier_charge_qi_uses_artifact_resonance_efficiency`）：明确钉死 `carrier_sealed_qi_amount(50.0, Some(1.0)) == 60.0`，即 50 真元封出 60——这是被测试锁死的既有设计缺陷，不是笔误。
- `server/src/combat/carrier.rs:824-880`（`throw_carrier_intents`）L872：`qi_payload: imprint.qi_amount`——放大后的封印量原样塞进 `QiProjectile`，飞行阶段没有任何 clamp 回未放大值。
- `server/src/combat/carrier.rs:920-1265`（`projectile_tick_system` / `emit_projectile_despawn`）：production 路径实际只触发 `ProjectileDespawnReason::NaturalDecay`（L940-953，`qi_payload` 衰减到 ε 以下）与 `OutOfRange`（L957-973，飞行距离超过 `ANQI_PROJECTILE_MAX_DISTANCE`）与 `HitTarget`；`HitBlock` 仅出现在测试代码（L2675）里，当前生产逻辑里投射物不做地形碰撞判定——即玩家**只需朝任意方向投掷、不瞄准任何东西**，飞行超距离即自动触发 `OutOfRange` miss 释放，比"对墙投掷"更容易复现。
- `server/src/combat/carrier.rs:1255-1276`（`emit_projectile_despawn`）：非 `HitTarget` 分支调用 `residual_qi_after_miss(qi_at_despawn)` 算出 `residual_qi`（其中 `qi_at_despawn` 由含放大部分的 `imprint.qi_amount` 派生）。
- `server/src/combat/carrier.rs:1292-1313`（`projectile_miss_qi_release_system`）→ `release_residual_to_zone`（L1318-1340）→ `release_account_to_zone`（L1342-约1410）：把 `residual` 经 `qi_release_to_zone` 写进 `zone.spirit_qi`，`from` 账户是合成 id `"anqi_projectile_miss:entity:<bits>"`，只是审计标签，没有任何余额校验。
- `server/src/qi_physics/release.rs:12-46`（`qi_release_to_zone`）：只做 `finite_non_negative` 和 `zone_cap` 容量 clamp，**不核验 `from` 账户是否真的持有这笔真元**——它是账本记账层，不是余额检查层，放大出来的凭空真元一旦到这里就会被无条件接受进 zone。
- 反证据（确认非重复）：`docs/plan-bughunt-anqi-throw-imprint-drop-v1.md`（同文件唯一在库骨架）处理的是"投掷方向为零向量/耐力不足时 imprint 被提前删除导致状态丢失"的状态错乱问题，与本 finding 的"共鸣放大量无来源"是完全不同的机制层面，无重叠。

## 触发路径

1. 玩家正常炼器流程（forge 熔炼）产出一件颜色与自身真元色匹配、凹槽已深化到位的暗器（`BoneChip` 等任一 anqi 载体），`ArtifactColor`/凹槽深度经 `forge/inventory_bridge.rs` 与 `artifact_meridian_deepen_on_use` 正常写入。
2. 玩家发起 `ChargeCarrierIntent` 蓄力封印，`begin_charge_carrier` 扣掉 `qi_target * 0.5`（prepaid），`charge_carrier_tick` 满蓄力时再扣剩余 `qi_target * 0.5`——共计从 `Cultivation.qi_current` 扣掉 `qi_target`（= `total_deducted` = `base_qi_amount`）。
3. `finish_charge` 用 `resonance`（此时接近 1.0）把 `base_qi_amount` 放大到最高 1.2× 写入 `CarrierImprint.qi_amount`；"未封印剩余量回收"逻辑因 `sealed_base_qi` 用的是未放大值，实际回收量恒为 0——放大差额凭空产生，无任何账户被扣。
4. 玩家发起 `ThrowCarrierIntent` 把暗器投掷出去（`throw_carrier_intents`），`imprint.qi_amount`（含放大部分）整体塞进 `QiProjectile.qi_payload`。
5. 玩家无需瞄准任何目标：只要投掷方向朝空地，投射物飞行超过 `ANQI_PROJECTILE_MAX_DISTANCE` 即触发 `OutOfRange` despawn（或飞行途中衰减到 `NaturalDecay`）。
6. `projectile_miss_qi_release_system` 把 `residual_qi`（衍生自含放大部分的 `qi_payload`）经 `qi_release_to_zone` 无条件写入玩家当前所在 zone 的 `spirit_qi`。
7. 重复步骤 2-6：每轮净增真元 ≈ `qi_target * (放大倍率 - 1) * miss 后残留比例`，可无限刷。

## 反方审查记录

- 第一轮质疑（skeptic 初判）：
  - 通读 `finish_charge` 全链路，确认 `total_deducted`（真实扣款）与 `base_qi_amount`（放大前基数）在 full_charge 下恒等，而写入 imprint 的 `qi_amount` 是放大后的值——两者数值来源不同轨。
  - 核对既有 pin 测试 `carrier_charge_qi_uses_artifact_resonance_efficiency`（carrier.rs:2472-2475），确认 `50.0 → 60.0` 是被测试主动钉死的既有行为，排除"读错代码/正在修的临时态"的可能。
  - 核对"未封印剩余量回收"分支（L672-679）用的 `sealed_base_qi` 是否是放大后的值：确认不是，是 `base_qi_amount`——full_charge 下差额恒为 0，回收逻辑对放大部分完全失效。
  - 核对 `imprint.qi_amount` 后续流向：确认原样进入 `QiProjectile.qi_payload`（carrier.rs:872），未做二次 clamp。
  - 核对 miss 释放路径 `qi_release_to_zone`（qi_physics/release.rs:12-46）是否会校验来源账户余额：确认不会，纯粹是"金额→zone 容量 clamp→记账"，没有查 `from` 是否真持有这笔真元的机制——即真元一旦被写进 imprint，就已经实质"存在"，不再受任何来源审计。
- 第二轮补证（可达性 + 查重）：
  - 核对 resonance 从何而来是否需要 dev 命令：确认 `ArtifactColor`/凹槽深度均由正常炼器（`forge/inventory_bridge.rs`）与战斗中深化凹槽（`artifact_meridian_deepen_on_use`）产出，无需 `/give`/`/realm` 等 dev-only 命令，resonance > 0.5（净放大阈值）只需颜色匹配（1.0）叠加凹槽深化过半，属于正常高强度玩家可达状态。
  - 核对投射物是否必须命中才能触发释放：确认不需要——`OutOfRange`（飞行超距）和 `NaturalDecay`（payload 衰减到 ε 以下）都会走 miss 释放分支，`HitBlock` 反而只存在于测试代码里，生产路径没有地形碰撞判定，比"对墙投掷"更容易触发（随手扔向任意方向即可）。
  - 查重：`gh pr list`/`docs/plans-skeleton/` 全库唯一涉及同一文件/函数的骨架是 `plan-bughunt-anqi-throw-imprint-drop-v1.md`，处理"投掷方向为零/耐力不足时 imprint 提前删除的状态丢失"，与"共鸣放大量无来源"是完全不同的失效模式，无重叠、非重复 finding。
  - 让步：当前为源码路径静态复现 + 既有 pin 测试数值互证，未额外起服实测；但既有 `carrier_charge_qi_uses_artifact_resonance_efficiency` 测试本身已经是对"放大行为存在"的运行时可验证证据（`cargo test` 可直接跑出 60.0）。
  - 终裁：严重度由 skeptic 初判 high 上调为 **critical**——这是教科书级别的 CLAUDE.md 红旗（"zone.spirit_qi 被写入但无对应玩家减少"），且是无需任何权限/dev 命令、纯正常玩法可重复刷的真元复制漏洞，直接击穿项目最高优先级硬约束（守恒律），故定为 critical。

主循环复核：已亲读关键行确认（`carrier.rs:579-681`/`729-734`/`824-880`/`920-1276`/`1292-1410`、`forge/resonance.rs:35-37`、`qi_physics/release.rs:12-46`），行号与 JSON 引用一致，且额外确认生产环境 `ProjectileDespawnReason::HitBlock` 未被触发（仅测试用），实际可达路径比 skeptic 原文举例的"对墙投掷"更宽松（任意方向投掷即可 `OutOfRange`）。

## Skeleton Fix Plan

真元流动必须走 `qi_physics::ledger` 口径，禁止凭空增减；本 fix 二选一（**决议时二选其一，不并存**，避免叠加式过度设计）：

**方案 A（推荐）：放大语义改为「效率折损」（≤1.0）**

- [ ] 把 `carrier_seal_efficiency_multiplier`（`server/src/forge/resonance.rs:35-37`）的区间从 `[0.8, 1.2]` 改为 `[<下限>, 1.0]`（例如维持下限 0.8，上限钳到 1.0：`0.8 + 0.2 * resonance.clamp(0.0, 1.0)`，resonance 决定"损耗多少"而非"倍增多少"）——数值上限具体取值走 pre-P0 决议（Explore 一次代码现状+平衡性核查，不在本骨架里拍板）。
- [ ] 确认改动后 `carrier_sealed_qi_amount(base, resonance) <= base` 对任意 `resonance ∈ [0,1]` 恒成立（新增专属边界测试，见验收测试计划）。
- [ ] 同步修改既有 pin 测试 `carrier_charge_qi_uses_artifact_resonance_efficiency`（carrier.rs:2472-2475）的口径：`carrier_sealed_qi_amount(50.0, Some(1.0))` 期望值需要从 `60.0` 改为新上限对应值（若上限收窄到 1.0，则为 `50.0`）；同时改 `damage_resonance_multiplier` 相关命名/文档若有混淆（该函数是伤害倍率，允许 >1.0，不在本 fix 范围内，需在 PR 描述中明确区分两者不是同一语义，避免误改）。
- [ ] `release_unsealed_carrier_qi`（L672-679）的 `sealed_base_qi` 语义保持"未放大基数"不变——效率折损方案下 `qi_amount <= base_qi_amount`，未封印回收部分（`total_deducted - qi_amount` 或等价量）需要重新过一遍：改为用 `total_deducted - qi_amount`（放大后/折损后的真实封印量）而不是 `total_deducted - sealed_base_qi`，让"没被封进去的真元"（无论是因为进度不足还是效率折损）**全部**如实归还 zone，不留任何差额悬空。

**方案 B（备选）：放大差额从 zone 现场扣取，不足则按实际扣到的量封印**

- [ ] 在 `finish_charge` 计算出 `qi_amount > base_qi_amount` 时，对放大差额 `delta = qi_amount - base_qi_amount` 尝试从玩家当前所在 zone 走 `qi_physics::ledger::QiTransfer { from: QiAccountId::zone(zone_name), to: <封印对应账户或直接算入 imprint 来源标签>, amount: delta, reason: QiTransferReason::... }` 真实扣取（复用 `qi_physics` 既有 zone 扣减路径，不新造公式）。
- [ ] zone 当前浓度不足以支付 `delta` 时，`qi_amount` 钳到 `base_qi_amount + <zone 实际能扣出的量>`（即"按实际扣到的量封印"），不允许出现 `delta` 部分来源不明的中间态。
- [ ] `finish_charge` 需要拿到 zone 引用（现有函数签名已接收 `zones: Option<&mut ZoneRegistry>` 与 `position`，具备扩展条件）；沿用文件内既有 `release_account_to_zone`/`qi_release_to_zone` 一样的"查 zone → 转账 → 写回 `zone.spirit_qi`"模式，不得自造新的扣减公式。
- [ ] 同步修改既有 pin 测试口径：`carrier_sealed_qi_amount(50.0, Some(1.0)) == 60.0` 若保留纯函数（此方案下该函数本身语义不变，仍是"意图放大量"），但 `finish_charge` 层面新增测试断言"当 zone 灵气不足以支付放大差额时，实际封印量被钳到 zone 能出的量 + base"。

**两案共同项**：

- [ ] 无论选哪案，`finish_charge`/`release_unsealed_carrier_qi`/`projectile_miss_qi_release_system` 全链路修完后必须满足不变式：**任意一次完整"充能→封印→投掷→miss 释放"循环后，`(玩家 qi_current 减少量) + (zone.spirit_qi 净变化量) == 0`**（守恒），新增集成测试直接断言这条不变式（见验收测试计划）。
- [ ] 修复不得触及 `damage_resonance_multiplier`（`resonance.rs:31-33`，伤害倍率，允许 >1.0 是合理的战斗强度设计，不是真元数量问题）——PR 描述需明确写清"只动封印效率，不动伤害倍率"，避免审查者混淆。
- [ ] 不新增独立衰减/放大公式——效率折损区间收窄如落在 `qi_physics::constants` 覆盖范围内应复用；若纯粹是"combat 特有的封印效率系数"（非全局真元物理常数），可保留在 `forge/resonance.rs` 内，但需在 PR 描述中说明为何不下沉 `qi_physics`（本质是装备强化系数，不是灵气物理衰减公式，符合 CLAUDE.md「新模块出现类衰减率常数」红旗的排除条件——但仍建议开 fix 时显式过一遍 `qi_physics::constants` 确认没有可复用的现成系数）。

## 验收测试计划

- **server（`cargo test`）单测 — happy path**：
  - resonance=0.0 时封印效率下限（方案 A：`carrier_seal_efficiency_multiplier(0.0) == 0.8`，保持不变）。
  - resonance=1.0 时封印效率新上限（方案 A：`carrier_seal_efficiency_multiplier(1.0) == <新上限，如 1.0>`，替换旧断言 `1.2`）。
  - resonance=0.5 中点值按新公式重新计算并断言（不能沿用旧 `1.0` 断言，需按新公式推导）。
- **边界**：
  - `resonance` 传 `None`（无 `QiColor`/无匹配）：`carrier_sealed_qi_amount(base, None) == base`（不放大不折损，维持既有行为，回归测试）。
  - `resonance` clamp 边界：传入 `-0.1`/`1.5` 等越界值，断言仍 clamp 到 `[0.0, 1.0]` 后再套公式（复用 `resonance.rs` 现有 clamp 逻辑测试模式）。
  - `base_qi_amount == 0.0`（例如 `qi_target` 极小蓄力提前中断）：断言 `qi_amount == 0.0`，走 `qi_amount <= f32::EPSILON` 分支直接 `release_unsealed_carrier_qi` 全额归还，不产生负数或 NaN。
- **错误分支 / 状态转换**：
  - 方案 A：新增 `finish_charge` 集成测试——构造 `resonance = Some(1.0)`（旧逻辑下会放大到 1.2×），断言修复后 `store.imprints_by_instance[instance_id].qi_amount <= base_qi_amount` 且 `release_unsealed_carrier_qi` 收到的归还量等于 `total_deducted - qi_amount`（不再恒为 0）。
  - 方案 B：新增测试覆盖两种状态转换——① zone 灵气充足时放大差额被正确从 `zone.spirit_qi` 扣除且 `qi_amount` 达到理论放大值；② zone 灵气不足（构造一个低浓度 zone）时 `qi_amount` 被钳到 `base_qi_amount + zone 实际可出量`，断言 zone 扣到接近 0 而非负值，且没有部分未入账的真元残留在 imprint 里。
  - **守恒不变式集成测试（两案通用，必须新增）**：完整走一遍 `begin_charge_carrier → charge_carrier_tick(full_charge) → finish_charge → throw_carrier_intents → projectile_tick_system(触发 OutOfRange) → projectile_miss_qi_release_system`，在测试里手动推进到 `OutOfRange`（构造超过 `ANQI_PROJECTILE_MAX_DISTANCE` 的飞行距离或直接调用 `emit_projectile_despawn` 传入 `OutOfRange`），断言循环前后 `(cultivation.qi_current 减少总量) == (zone.spirit_qi 增加对应的绝对真元量，按 `QI_ZONE_UNIT_CAPACITY` 折算)`，即净变化为 0（允许因 `residual_qi_after_miss` 正常蒸发损耗，蒸发部分不计入 zone，需要断言蒸发量 + zone 归还量 + （若方案B）zone 扣取量在数值上自洽，不出现来源不明的正向净增）。
  - **回归**：resonance 处于会导致旧逻辑放大的区间（如 0.6-1.0）时，`HitTarget` 命中分支（不走 miss 释放）不应受本次修复影响——保留/新增一条 `HitTarget` 分支下伤害计算不变的回归测试，确认没有误伤 `damage_resonance_multiplier`。
- **测试所在栈**：全部为 `server/` Rust 单测 + 集成测试，跑 `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`（本 fix 不涉及 client/agent/worldgen，无需跨栈门禁）。

## 风险

- 若采用方案 A 收窄效率区间，需要重新评估暗器封印在战斗强度曲线里的定位（原本"共鸣拉满多封 20% 真元"是一个正向养成激励，改成"最多不折损"会削弱高共鸣值的收益预期）——建议 pre-P0 决议阶段同步检查 `damage_resonance_multiplier`（伤害倍率仍保留 0.7-1.3× 放大）是否已经足够承担"共鸣拉满的正反馈"，避免玩家体感觉得共鸣系统变弱。
- 若采用方案 B，需要确保"从 zone 扣真元来补足封印"不会引入新的可刷分支——例如玩家专挑高浓度 zone 蓄力封印、再跑去低浓度 zone 扔出去，利用两地浓度差牟利；扣取时机必须在封印当下（`finish_charge` 内，扣当前所在 zone），不能推迟到投掷/miss 阶段，否则又会重新出现"扣取账户与释放账户不一致"的第二层守恒漏洞。
- 两案都改动了既有 pin 测试的期望数值（`carrier_charge_qi_uses_artifact_resonance_efficiency` 的 `60.0` 断言），必须在同一 PR 内同步修改，不允许留旧断言与新实现不一致导致测试假绿或红。
- 本 fix 范围明确排除 `damage_resonance_multiplier`（伤害倍率）——如果 fix 实施时顺手把伤害倍率也钳到 `<=1.0`，属于范围蔓延，会改变战斗强度平衡，应在 review 阶段拦截。
