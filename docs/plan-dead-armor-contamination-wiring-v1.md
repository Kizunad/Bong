# plan-dead-armor-contamination-wiring-v1（active）

> **Active（已从 skeleton 升级，待逐项消费）**。一句话主题：接通「死脉甲污染豁免」——`should_block_contamination` 从未接入 `resolve.rs` 污染写入点，死脉甲防御机制对生产玩法完全失效（plan-baomai-v4 自报已实装但代码缺失，⚠️ 文档↔代码背离）。

> 立项动机：bug-hunt round1 确认（major，孤岛+doc-vs-code）：`combat/baomai_v4/dead_armor.rs:307` `should_block_contamination` **零生产调用**（grep 仅定义 + p3 测试 4 处）；`voluntary_sever_apply_system` 正确写 `armor.immune_regions`（dead_armor.rs:290），但 `resolve.rs:1324` 污染写入点对其**零过滤**（grep resolve.rs 无 is_immune/DeadMeridianArmor）；唯一非测试消费者 `crack_reading.rs` 只读显示标志不拦截。`plan-baomai-v4.md:510` 明确要求 `dead_armor_contam_filter_system` 在 resolve.rs 污染写入前拦截 + 被拦 delta 走 `qi_release_to_zone`（守恒），且该 plan **已归 finished_plans**、P3 标 ✅、Finish Evidence 行 1216 自报「resolve.rs 污染拦截」已落地——属 ⚠️ 文档自报完成但代码缺失。

## 阶段总览

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | resolve.rs 污染写入前接 should_block_contamination 拦截 + 被拦 delta 守恒处置（drop_no_release，见 §N.1） | ✅ 2026-06-16 |
| P1 | plan-baomai-v4 Finish Evidence 修正（doc↔code 背离审计） | ⬜ |

## 接入面 checklist

- **进料**：`dead_armor.rs::should_block_contamination(armor, region)` + `armor.immune_regions`。
- **接入点**：`combat/resolve.rs:1324` 污染写入。
- **qi_physics 锚点**：被拦截 delta 直接丢弃（drop_no_release，见 §N.1 #1）——污染量为派生量非守恒转移，release_to_zone 会通胀；plan-baomai-v4:510 的 release_to_zone 要求已被 §N.1 #1 决议推翻。
- **跨 plan**：`plan-baomai-v4`（finished，P3）——本 plan 补其缺失实装；plan-baomai-v4 P3 状态需人工修正（不自动改其他 plan）。

## P0 — 污染拦截接线

- **目标**：`resolve.rs` 污染写入前查 `should_block_contamination`/`immune_regions`，免疫区污染被拦；拦下的 delta 走 `qi_release_to_zone` 守恒。
- **可核验**：免疫区不被污染（端到端 case：voluntary_sever→immune_region→污染源→不沾）；守恒断言。

## P1 — doc↔code 背离审计修正

- plan-baomai-v4 P3「resolve.rs 污染拦截」Finish Evidence 自报与代码缺失的背离，需人工核实/修正该 plan 状态（本 PR 只补实装 + 注明，不自动改 baomai-v4 文档）。

## §N 开放问题

1. should_block_contamination 的 region 粒度与 resolve.rs 污染写入的 region 是否对齐。
2. 是否还有其他 plan 自报完成但同样缺实装（doc↔code 审计可扩为独立 plan）。

## §N.1 决议（pre-P0 收口，2026-06-16）

### #1 被拦污染 delta 处置：drop_no_release（**非** release_to_zone）

**决议**：被拦截 contamination delta **直接丢弃**，不调用 `qi_release_to_zone`。理由：污染量是派生量 `emitted_contam_delta = damage × 0.25 × contam_mul × wound_profile.contam_mul`（resolve.rs:822-825），**非攻方真元账户的 1:1 转移**——攻方 qi_invest 已在 resolve.rs:452 扣除并消耗于 damage/throughput，污染 delta 从未作为余额记进任何账户。若 release_to_zone 会向 zone 注入从未被扣走的真元=**通胀**。与既有所有污染削减路径一致（armor/jiemai/sword parry/shield/tuike 全部静默丢弃，无一 release）。

**落点**：`server/src/combat/resolve.rs:1326-1337`（filter+drop）/ `baomai_v4/dead_armor.rs:304-322`（doc）。

**⚠️ 与 finished plan 背离**：`docs/finished_plans/plan-baomai-v4.md` §4.3:510「归还环境」前提错误。本 PR **不自动改 finished plan**（CLAUDE.md 约束）——**需人工**把此决议追加到 plan-baomai-v4 §8.1 级决议块（即本 plan P1 doc-audit）。

### #2 region 对齐（meridian → 可命中 body_part）

**决议**：`classify_body_part` 穷举只产 Head/Chest/ArmL/ArmR/Abdomen/LegL/LegR，**永不产 Back**。原 `Du → Back` 是死代码（断 Du 零保护）→ 改 **Du → Chest**（与 Ren 共享躯干防护）。**Head/Abdomen 无经脉映射 = 刻意永久弱点区**（显式 doc 声明，死脉甲不保护头/腹）。

**落点**：`baomai_v4/dead_armor.rs:191-227`。⚠️ plan-baomai-v4 §4.3:508 映射表 Du→Back 错，同需人工修订。

## 审计来源

bug-hunt round1 confirmed（孤岛+doc-vs-code，major）。**report-only**：跨 resolve.rs+dead_armor 接线 + 守恒 + 审计修正。
