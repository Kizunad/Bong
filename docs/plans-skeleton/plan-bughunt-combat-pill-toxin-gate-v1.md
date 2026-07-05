# plan-bughunt-combat-pill-toxin-gate-v1（骨架）

> **骨架（草案）**。一句话主题：item runtime / consumable 路径确认 **1 个新真 bug**——`AlchemyTakePill -> handle_alchemy_take_pill` 的战斗丹真实消费链**完全绕过「同色丹毒超阈值禁服」门**，导致 combat pill 在当前丹毒已超 `TOXIN_THRESHOLD=1.0` 后仍可继续正常消耗、正常生效、正常叠丹毒。

> 立项动机：聚焦 consumable runtime。此问题不属于已排除的 quickslot breakthrough pill noop / npc trade bundle count bridge / inventory orphan pack / extra hand equip gate；它落在 **战斗丹真实运行时**，且直接违反 `plan-alchemy-v1` 已归档约束。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 🔴 战斗丹同色丹毒超阈值后仍可继续正常服用 | fix_pr | ⬜ |

## P0 — 🔴 战斗丹同色丹毒超阈值后仍可继续正常服用

- **major（fix_pr）**：`server/src/network/client_request_handler.rs:12191-12443` 的 `handle_alchemy_take_pill` 在真实生产路径里只做了 3 类前置门：
  - ① 物品存在校验；
  - ② shelflife `SpoilCheckOutcome::CriticalBlock` 门（`12232-12245`）；
  - ③ quick-slot-only effect 门（`12282-12302`）。
  - 随后直接 `consume_item_instance_once`（`12304`）扣物品，并在 `ItemEffect::CombatPill` 分支进入 `apply_combat_pill_runtime`（`12425-12443`）。
- `apply_combat_pill_runtime` 里，战斗丹只会把 `CombatPillSpec` 转成 `PillEffect` 后直接调用 `crate::alchemy::pill::consume_pill(...)`（`12626-12641`），**没有任何 `can_take_pill` 判定**；而且返回值被 `let _ =` 直接丢弃。
- `server/src/alchemy/pill.rs:73-77` 明确把 `TOXIN_THRESHOLD=1.0` 与 `can_take_pill` 定义为**同色丹毒聚合量小于阈值才可服用**；`docs/finished_plans/plan-alchemy-v1.md:159-168` 也把 `§2.2 重复服药约束` 写死为“同色丹毒未代谢到阈值 → 禁止再服（或强吃触发过量 debuff）”。
- 但生产代码全仓 grep `can_take_pill(` 只命中 `alchemy/pill.rs` 与测试代码；**零生产 caller**。这说明阈值规则停留在 library/test 层，没接到真实 consumable runtime。
- 复现路径（无需改代码）：
  1. 准备同色战斗丹，例如 `duan_xu_san`（`server/src/alchemy/pill.rs:283-291`，`toxin_amount=0.80`，`toxin_color=Turbid`）。
  2. 连续服用 2 颗后，同色丹毒已达 `1.60 > TOXIN_THRESHOLD(1.0)`；按 `can_take_pill` 语义，第 3 颗应被拒绝或走“强吃”专门分支。
  3. 实际第 3 颗仍会在 `12304` 被扣除，并在 `12626-12641` 正常继续注入丹毒；同时 `12650+` 的疗伤/断肢修复/体力/护甲 buff 逻辑照常生效。
  4. 更常见的平和丹也同病：例如 `huo_xue_dan` / `hui_li_dan` 单颗 `0.15`，第 8 颗前都不会被 runtime 拦。
- 影响面：
  - 10 个 `CombatPillSpec` 全量受影响（`server/src/alchemy/pill.rs:254-382`）。
  - `docs/finished_plans/plan-consumable-effects-v1.md:61` 已核定 `CombatPill` **只在** `AlchemyTakePill -> handle_alchemy_take_pill` 这条路径真实施效；因此这不是边角 dead code，而是全部战斗丹的主 runtime。

## 这个 bug 对实际游玩体验的影响

- 玩家可以在同色丹毒早已爆表后继续无门槛连嗑战斗丹，绕过“同色丹毒代谢窗口”的节奏约束。
- 结果是活血/续骨/断续/铁壁/金钟/疾风/回力等战斗丹都能被连续硬刷，伤口恢复、断肢修复、短时坦度和体力续航被异常放大。
- 体感上会变成“文档和数值都说有禁服/强吃代价，实际 runtime 却是想吃几颗吃几颗”，直接削弱服丹成本、污染管理和炼丹供给平衡。

## 根因链路

1. 设计层定义了 `can_take_pill` 和 `TOXIN_THRESHOLD`，并在 `plan-alchemy-v1 §2.2` 约束“同色丹毒超阈值禁服/强吃分流”。
2. 生产消费入口 `handle_alchemy_take_pill` 只接了 freshness/quick-slot 门，没有接丹毒阈值门。
3. 战斗丹主施效路径 `apply_combat_pill_runtime` 直接调用 `consume_pill`，而 `consume_pill` 自身只处理 shelflife `CriticalBlock`，不处理同色丹毒超阈值。
4. 因此当前 runtime 把“禁服或强吃”两种语义都坍缩成了“无条件正常消费”。

## 修复建议

- 在 `handle_alchemy_take_pill` 的 `ItemEffect::CombatPill` 路径里，**先**读取 `Contamination` 并执行 `can_take_pill(contam, spec.toxin_color)`；失败时在 `consume_item_instance_once` 之前拒绝，并给客户端明确 reject reason / HUD 提示。
- 若设计要保留“强吃”玩法，不要沿用当前无条件正常消费；应新增显式 `force_consume` 二次确认协议，再把该标志传到真正处理 overdose/额外代价的 runtime。
- 补一条 production-level 集成测试：同色战斗丹服到 `>= TOXIN_THRESHOLD` 后，再次 `AlchemyTakePill` 不应扣库存、不应继续施效。

## §8 反方裁决（当前会话无 subagent，退化为本地双轮自裁决）

### 第一轮反方：也许 `can_take_pill` 只服务修炼丹，不服务战斗丹

- **反方论点**：战斗丹已有独立 `CombatPillSpec` / `apply_combat_pill_runtime`，丹毒阈值可能只给 `consume_cultivation_pill` 用。
- **驳回理由**：`plan-alchemy-v1 §2.2` 写的是泛化 `can_take(pill)`，不是“修炼丹专属”；`alchemy/pill.rs:80-108` 的 `consume_pill` 注释也是通用“服药流程”，并明确 caller 需处理消费门。更关键的是，生产路径里战斗丹同样通过 `consume_pill` 注入 `ContamSource`，说明它们就在同一丹毒规则域内，不存在另一套已接好的 runtime 门。

### 第二轮反方：也许设计本来允许超阈值继续吃，只是以后再补惩罚

- **反方论点**：`plan-alchemy-v1.md:167-173` 写了“禁止再服（或强吃触发过量 debuff）”，也许当前版本选择了“默认强吃”。
- **驳回理由**：真正的“强吃”至少需要显式分支、额外代价或二次确认；但现状既没有 `force_consume=true` 的请求来源，也没有 overdose 分支，只是静默按普通服用处理。也就是说 runtime 既没实现“禁止再服”，也没实现“强吃有代价”，而是把两者都漏掉了，所以仍是 bug，不是 feature flag。

## 审计来源

bughunt 2026-07-05（item runtime / consumable 聚焦，report-only）。证据链来自 `server/src/network/client_request_handler.rs`、`server/src/alchemy/pill.rs` 与已归档 `plan-alchemy-v1` / `plan-consumable-effects-v1` 的交叉核对。结论：**REAL，major，fix_pr。**
