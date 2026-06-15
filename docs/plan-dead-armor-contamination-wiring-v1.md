# plan-dead-armor-contamination-wiring-v1（active）

> **Active（已从 skeleton 升级，待逐项消费）**。一句话主题：接通「死脉甲污染豁免」——`should_block_contamination` 从未接入 `resolve.rs` 污染写入点，死脉甲防御机制对生产玩法完全失效（plan-baomai-v4 自报已实装但代码缺失，⚠️ 文档↔代码背离）。

> 立项动机：bug-hunt round1 确认（major，孤岛+doc-vs-code）：`combat/baomai_v4/dead_armor.rs:307` `should_block_contamination` **零生产调用**（grep 仅定义 + p3 测试 4 处）；`voluntary_sever_apply_system` 正确写 `armor.immune_regions`（dead_armor.rs:290），但 `resolve.rs:1324` 污染写入点对其**零过滤**（grep resolve.rs 无 is_immune/DeadMeridianArmor）；唯一非测试消费者 `crack_reading.rs` 只读显示标志不拦截。`plan-baomai-v4.md:510` 明确要求 `dead_armor_contam_filter_system` 在 resolve.rs 污染写入前拦截 + 被拦 delta 走 `qi_release_to_zone`（守恒），且该 plan **已归 finished_plans**、P3 标 ✅、Finish Evidence 行 1216 自报「resolve.rs 污染拦截」已落地——属 ⚠️ 文档自报完成但代码缺失。

## 阶段总览

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | resolve.rs 污染写入前接 should_block_contamination 拦截 + 被拦 delta 守恒释放 | ⬜ |
| P1 | plan-baomai-v4 Finish Evidence 修正（doc↔code 背离审计） | ⬜ |

## 接入面 checklist

- **进料**：`dead_armor.rs::should_block_contamination(armor, region)` + `armor.immune_regions`。
- **接入点**：`combat/resolve.rs:1324` 污染写入。
- **qi_physics 锚点**：被拦截 delta 走 `qi_release_to_zone`（守恒，plan-baomai-v4:510 要求）。
- **跨 plan**：`plan-baomai-v4`（finished，P3）——本 plan 补其缺失实装；plan-baomai-v4 P3 状态需人工修正（不自动改其他 plan）。

## P0 — 污染拦截接线

- **目标**：`resolve.rs` 污染写入前查 `should_block_contamination`/`immune_regions`，免疫区污染被拦；拦下的 delta 走 `qi_release_to_zone` 守恒。
- **可核验**：免疫区不被污染（端到端 case：voluntary_sever→immune_region→污染源→不沾）；守恒断言。

## P1 — doc↔code 背离审计修正

- plan-baomai-v4 P3「resolve.rs 污染拦截」Finish Evidence 自报与代码缺失的背离，需人工核实/修正该 plan 状态（本 PR 只补实装 + 注明，不自动改 baomai-v4 文档）。

## §N 开放问题

1. should_block_contamination 的 region 粒度与 resolve.rs 污染写入的 region 是否对齐。
2. 是否还有其他 plan 自报完成但同样缺实装（doc↔code 审计可扩为独立 plan）。

## 审计来源

bug-hunt round1 confirmed（孤岛+doc-vs-code，major）。**report-only**：跨 resolve.rs+dead_armor 接线 + 守恒 + 审计修正。
