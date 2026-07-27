# plan-bughunt-r6-findings-v1（已归档）

> 一句话主题：round6 五条 finding 已按 `origin/main @ c625d5a5` 复核：凡甲注册已修，alchemy 两项、Freeze 容器与 JueBi marker 分别移交唯一 owner。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| T0 | 五条 finding current-code/祖先修复复核 | ✅ 2026-07-28 |
| T1 | 三个 canonical owner 去重登记 | ✅ 2026-07-28 |
| T2 | mapping table、Finish Evidence、归档 | ✅ 2026-07-28 |

## Finding Mapping

| Finding | 当前裁决 / current `file:line` | 分类 | Canonical owner / merged evidence | 文档动作 |
|---|---|---|---|---|
| #8 mundane armor duplicate 早退 | `server/src/armor/mundane.rs:249-270` 当前 duplicate 走 skip-on-duplicate；`server/src/armor/mundane.rs:315-352,371-380` 覆盖非空 registry | `already-fixed/invalid`（already-fixed） | `e42092c11` / PR #1068 | 仅归档 |
| #0 `ContaminationBoost` 无 consumer | `server/src/alchemy/side_effect_apply.rs:25` 仍生产；`server/src/cultivation/contamination.rs:97-205` 的 tick query/公式不读 `StatusEffects` | `independent-domain-fix` | `docs/plans-skeleton/plan-bughunt-modifier-effect-consumer-completion-v1.md` P1 | 统一 alchemy effect owner |
| #1 JinZhongDan negative slot 极性 | `server/src/alchemy/pill.rs:632-642` negative duration 仍 push 正向 `QiRegenBoost`；当前 regen consumer 会把它变成增益 | `independent-domain-fix` | 同上 P1 | 与 #0 同一 effect/极性批次 |
| #4 Freeze 容器接线 | `server/src/shelflife/container.rs:82,94` helper 存在；`server/src/network/client_request_handler.rs:18079` 消费仍硬编码 `Normal` | `independent-domain-fix` | 现有 active `docs/plan-container-filter-and-completion-v1.md` P2 | 已在 P2 精确补入 enter/exit/真实 behavior 验收 |
| #6 `JueBiAfterDuXuQuota` 泄漏 | `server/src/cultivation/tribulation.rs:305,949,3177` marker 插入/读取存在，终止态尚无统一 cleanup invariant | `independent-domain-fix` | `docs/plans-skeleton/plan-bughunt-duxu-juebi-quota-marker-lifecycle-v1.md` | 新建唯一 focused owner |

## Finish Evidence

- **落地清单**：alchemy 两项归统一 modifier/effect successor；Freeze 复用现有 container P2 并补精确验收；JueBi marker 新建 focused owner；bundle 迁入本路径。
- **关键 commit / PR**：凡甲修复 `e42092c11` / PR #1068 已验证为目标 HEAD 祖先且当前行为保留。
- **测试结果**：docs-only triage；最终以 docs static gates + exact-HEAD validator 验收。
- **跨仓库核验**：纯 server finding；Freeze owner 的后续 P2 仍按 inventory/shelflife 契约执行，无本次 wire 修改。
- **遗留 / 后续**：四条 live finding 已各有唯一 owner；本 bundle 禁止再消费。
