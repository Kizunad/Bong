# BugHunt: 暗器共鸣封印按比例放大真元却只扣基础量，miss 释放真造真元

> 一句话主题：把暗器共鸣从“无来源增量”收口为不超过投入量的封印效率，并以充能、未封印回流、miss 释放的统一账本不变式锁死真元守恒。
>
> 阶段总览：P0 验真与方案收口 ✅ 2026-09-24 ｜ P1 守恒修复 ✅ 2026-09-24 ｜ P2 饱和测试与全栈门禁 ✅ 2026-09-24 ｜ P3 主线同步、终验与归档 ✅ 2026-09-24

## Preflight（2026-09-24）

- `docs/worldview.md`：已核对真元只能转移、全服总量守恒及暗器相关语义；本 plan / PR 未修改该文件。
- `docs/finished_plans/` 与 `docs/plan-*.md`：执行 `grep -RilE 'carrier|resonance|seal' docs/finished_plans docs/plan-*.md`。命中既有暗器/共鸣/qi ledger 基础文档；直接相邻的是 `docs/plan-bughunt-anqi-throw-imprint-drop-v1.md`（投掷失败提前删除 imprint 的状态一致性问题），不涉及封印效率放大或 miss 铸币。未发现与本 plan 同一根因的重复 plan。
- `docs/plans-skeleton/`：执行 `grep -RilE 'carrier|resonance|seal' docs/plans-skeleton`。相关命中包括 `plan-bughunt-anqi-carrier-charged-agent-narration.md`（agent 叙事订阅缺口）与 `plan-bughunt-qi-ledger-asymmetry-v1.md`（全局 ledger producer 审计，提及 carrier 为待治理项）；两者均不覆盖本 plan 的共鸣效率铸币漏洞，未发现重复 plan。
- `reminder.md`：仓库根目录与 `docs/reminder.md` 均不存在；仓内仅有 `docs/plans-skeleton/reminder.md`，已确认是 skeleton 提醒文件，与本 plan 无关。

## 接入面

- **进料**：`ChargeCarrierIntent` / `CarrierCharging`、`Cultivation.qi_current`、`PlayerInventory` 中的 `ArtifactColor` 与凹槽深度、`ZoneRegistry`。
- **出料**：`CarrierImprint.qi_amount` → `QiProjectile.qi_payload`；未封印部分和 miss residual 统一经 `qi_release_to_zone` 回到落点 zone。
- **共享类型 / event**：复用 `QiTransfer`、`QiAccountId`、`QiTransferReason`、`CarrierChargedEvent`、`ProjectileDespawnedEvent`，不新增平行账户或事件。
- **跨仓库契约**：纯 server 数值与账本修复；不改 C2S payload、schema、client HUD 或 agent 契约。
- **worldview 锚点**：`worldview.md` §二/§十真元只能转移、全服总量守恒；暗器离体真元仍受既有距离损耗和 miss 回流规则约束。
- **qi_physics 锚点**：释放复用 `qi_physics::release::qi_release_to_zone`，折算复用 `QI_ZONE_UNIT_CAPACITY`；封印效率属于装备共鸣系数，保留于 `forge::resonance`，不新增真元物理衰减常量。

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

## P0 验真与方案收口 ✅ 2026-09-24

### 2026-09-24 pre-P0 决议

1. **确认真 bug**：`finish_charge` 的 full-charge 路径实际扣除量与 `base_qi_amount` 均为 `qi_target`，而 `carrier_seal_efficiency_multiplier(1.0) == 1.2` 使 imprint 可达投入量的 120%；`OutOfRange`/`NaturalDecay` 的 residual 会进入 `qi_release_to_zone`，来源账户仅为审计标签，不校验余额。
2. **采用方案 A**：封印共鸣定义为“从 80% 折损线性提升到 100% 无损”，公式收口为 `0.8 + 0.2 * clamp(resonance, 0, 1)`。高共鸣正反馈仍由“少损失封印量”与独立的 `damage_resonance_multiplier`（0.7–1.3×）共同承担；不向 zone 借取放大差额，避免跨 zone 套利和新账户状态。
3. **归还真实未封印量**：成功转换载体时，以实际写入 imprint 的 `qi_amount` 作为 sealed 量；转换失败时 sealed 为 0。`release_unsealed_carrier_qi` 归还 `total_deducted - sealed_qi`，保证折损部分与失败部分均回到 zone。
4. **范围边界**：不改 `damage_resonance_multiplier`、投射物距离损耗、miss residual 比例、C2S/schema/client；不新增 qi_physics 常量。

**落点**：`server/src/forge/resonance.rs:31-37`、`server/src/combat/carrier.rs:579-734`、本 plan P1/P2。

## P1 守恒修复 ✅ 2026-09-24

真元流动必须走 `qi_physics::ledger` 口径，禁止凭空增减；本 fix 已决议采用方案 A，方案 B 仅保留为被拒绝路线的历史记录：

**方案 A（已采纳）：放大语义改为「效率折损」（≤1.0）**

- [x] 把 `carrier_seal_efficiency_multiplier`（`server/src/forge/resonance.rs:35-37`）收口为 `0.8 + 0.2 * resonance.clamp(0.0, 1.0)`，上限为 1.0。
- [x] 新增边界断言，确认 `carrier_sealed_qi_amount(base, resonance) <= base`，并覆盖 `resonance` 越界钳制。
- [x] 更新 `carrier_charge_qi_uses_artifact_resonance_efficiency_without_minting_qi` 的 pin 口径；`damage_resonance_multiplier` 保持 0.7–1.3，不在本修复范围内。
- [x] `finish_charge` 按实际写入 imprint 的 `qi_amount` 计算 `total_deducted - sealed_qi`，将效率折损部分经 `release_unsealed_carrier_qi` 归还 zone。

**方案 B（已拒绝）：放大差额从 zone 现场扣取，不足则按实际扣到的量封印**

- [x] 方案 B 已拒绝：不从 zone 借取放大差额，也不保留大于投入量的封印语义。
- [x] 方案 B 的 zone 扣取、余额不足钳制与对应 pin 测试均不实施，避免引入跨 zone 套利状态。

**两案共同项**：

- [x] 完整充能、封印、投掷、`OutOfRange` miss 回流链路以 `summarize_world_qi` 前后快照和 `assert_conservation` 验证，预算锚点引用 `SPIRIT_QI_TOTAL`。
- [x] 修复未触及 `damage_resonance_multiplier`，也未新增 C2S、schema、client 或 agent 变更。
- [x] 未新增独立真元物理常数；封印效率仍是 `forge::resonance` 的装备共鸣系数。

## P2 饱和测试与 server 门禁 ✅ 2026-09-24

### 验收测试计划

- **server（`cargo test`）单测 — happy path**：✅
  - [x] resonance=0.0 时封印效率下限：`carrier_seal_efficiency_multiplier(0.0) == 0.8`。
  - [x] resonance=1.0 时封印效率上限：`carrier_seal_efficiency_multiplier(1.0) == 1.0`，取代旧的 `1.2`。
  - [x] resonance=0.5 中点值：`carrier_seal_efficiency_multiplier(0.5) == 0.9`。
- **边界**：✅
  - [x] `resonance=None` 保持 `carrier_sealed_qi_amount(base, None) == base`。
  - [x] `resonance=-0.1/1.5` 仍先钳制到 `[0.0, 1.0]`，并断言封印量不超过投入量。
  - [x] `base_qi_amount=0.0` 返回零，覆盖 `finish_charge` 的 EPSILON 早退语义。
- **错误分支 / 状态转换**：✅
  - [x] `full_charge_resonance_loss_returns_unsealed_qi_to_zone` 验证 `finish_charge` 以实际 `qi_amount` 计算未封印回流。
  - [x] `full_resonance_charge_and_out_of_range_miss_preserve_world_qi_budget` 完整走过充能、投掷、`OutOfRange` 与 miss 回流，使用 `summarize_world_qi` 前后快照和 `assert_conservation`；蒸发量由 `residual_qi` 计入预期损耗。
  - [x] `hit_target_despawn_does_not_release_to_zone` 保证命中分支不重复回流；`damage_resonance_multiplier` 的 0.7–1.3 pin 测试继续通过。
  - [x] 方案 B 明确拒绝，不新增 zone 借取或余额不足钳制测试。
- **测试所在栈**：全部为 `server/` Rust 单测与集成测试；已通过 `scripts/build-token.sh cargo fmt --check`、`scripts/build-token.sh cargo clippy --all-targets -- -D warnings`、`scripts/build-token.sh cargo test`。本 fix 不涉及 client/agent/worldgen，无需跨栈门禁。

## P3 主线同步、终验与归档 ✅ 2026-09-24

- [x] `git fetch origin && git merge origin/main` 已执行，结果为 already up to date；主线未带入变更，无需重复门禁。
- [x] 全阶段更新为 `✅ 2026-09-24`，补齐 `## Finish Evidence`，并迁入 `docs/finished_plans/`。
- [x] 最终 HEAD 完成 read-only 检查，随后推送 claim 分支并创建 PR。

## Finish Evidence

### 落地清单

- `server/src/forge/resonance.rs`：封印效率从 `[0.8, 1.2]` 收口到 `[0.8, 1.0]`，保留伤害共鸣倍率。
- `server/src/combat/carrier.rs`：`finish_charge` 以实际写入 imprint 的 `sealed_qi` 计算未封印真元回流。
- `server/src/combat/carrier_tests.rs`：共鸣边界、效率折损回流，以及充能→投掷→`OutOfRange`→miss 的守恒契约测试；守恒快照使用 `summarize_world_qi`、`assert_conservation` 与 `SPIRIT_QI_TOTAL`。

### 关键 commit

- `d82911690`（2026-09-24）：将本 bughunt 骨架升格为 active plan。
- `75035f51b`（2026-09-24）：修复暗器共鸣封印链路并加入守恒回归测试，commit trailer 为 `Model: gpt-6-luna`。

### 测试结果

- `scripts/build-token.sh cargo fmt --check`：PASS。
- `scripts/build-token.sh cargo clippy --all-targets -- -D warnings`：PASS。
- `scripts/build-token.sh cargo test`：PASS；server lib 10,374 passed、0 failed、1 ignored，main 18 passed，doc-tests 3 passed、5 ignored，其他 integration suites 全部 PASS。
- `scripts/build-token.sh cargo test carrier -- --test-threads=1`：PASS，命中 60 项 server 单元测试。

### 跨仓库核验

- server 命中 `finish_charge`、`carrier_sealed_qi_amount`、`projectile_miss_qi_release_system`、`summarize_world_qi`、`assert_conservation`、`SPIRIT_QI_TOTAL`。
- 本修复不改 agent、client、schema 或跨仓库 payload 契约。

### 遗留 / 后续

- 本 plan 范围内无遗留；`damage_resonance_multiplier`、miss 蒸发规则和跨仓库契约保持原行为。

## 风险

- 若采用方案 A 收窄效率区间，需要重新评估暗器封印在战斗强度曲线里的定位（原本"共鸣拉满多封 20% 真元"是一个正向养成激励，改成"最多不折损"会削弱高共鸣值的收益预期）——建议 pre-P0 决议阶段同步检查 `damage_resonance_multiplier`（伤害倍率仍保留 0.7-1.3× 放大）是否已经足够承担"共鸣拉满的正反馈"，避免玩家体感觉得共鸣系统变弱。
- 若采用方案 B，需要确保"从 zone 扣真元来补足封印"不会引入新的可刷分支——例如玩家专挑高浓度 zone 蓄力封印、再跑去低浓度 zone 扔出去，利用两地浓度差牟利；扣取时机必须在封印当下（`finish_charge` 内，扣当前所在 zone），不能推迟到投掷/miss 阶段，否则又会重新出现"扣取账户与释放账户不一致"的第二层守恒漏洞。
- 两案都改动了既有 pin 测试的期望数值（`carrier_charge_qi_uses_artifact_resonance_efficiency` 的 `60.0` 断言），必须在同一 PR 内同步修改，不允许留旧断言与新实现不一致导致测试假绿或红。
- 本 fix 范围明确排除 `damage_resonance_multiplier`（伤害倍率）——如果 fix 实施时顺手把伤害倍率也钳到 `<=1.0`，属于范围蔓延，会改变战斗强度平衡，应在 review 阶段拦截。
