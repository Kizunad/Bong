# plan-bughunt-r8-modifier-orphan-audit-v1（已归档）

> 一句话主题：本 plan 的验证史与 P1 修复已完成；P2-P5 未实施工作已逐项迁入唯一 successor，原 active audit 不再承担实现队列。

## 阶段总览

| 阶段 | 内容 | 归档状态 |
|---|---|---|
| P0 | orphan/非 orphan 第一性验证矩阵 | ✅ 2026-07-07 |
| P1 | scar circuit reach/regen/purge consumer | ✅ 2026-07-08 |
| P2 | Iron cocoon wound/flow | ✅ 2026-07-28（移交 successor，未宣称实现） |
| P3 | InsightModifiers consumer | ✅ 2026-07-28（移交 successor，未宣称实现） |
| P4 | jump authority/wire/consumer | ✅ 2026-07-28（移交 successor，未宣称实现） |
| P5 | anti-orphan manifest/lint | ✅ 2026-07-28（移交 successor，未宣称实现） |

## Finding Mapping

| Finding / 阶段 | 当前裁决 / current `file:line` | 分类 | Canonical owner / merged evidence | 文档动作 |
|---|---|---|---|---|
| P0 验证矩阵 | `DerivedAttrs`/`InsightModifiers`/jump producer-consumer 已完成第一性分类；`healing_rate_multiplier`、`composure_recover_mul` 等误报已剔除 | `already-fixed/invalid`（audit completed） | 本归档保留验证史；实现 owner 见下 | 不再作为 active queue |
| P1 scar circuit 三字段 | `server/src/combat/player_attack.rs` 消费 reach；`cultivation/tick.rs:240-270` 消费 regen；`cultivation/contamination.rs` 消费 purge | `already-fixed/invalid`（already-fixed） | `3e6981513` / PR #1143 | 不重复实施 |
| P2 Iron cocoon | `server/src/combat/baomai_v4/iron_cocoon.rs:110-140` 写 wound/flow 字段，resolve/effective meridian rate 仍不读 | `independent-domain-fix` | `docs/plans-skeleton/plan-bughunt-modifier-effect-consumer-completion-v1.md` P2 | 完整迁移设计门 |
| P3 InsightModifiers | `server/src/cultivation/insight_apply.rs:24-254` 仍有多组写入字段无 gameplay consumer；`observe_chance_bonus` helper 无 production caller | `independent-domain-fix` | 同 successor P3 | 仅迁 live 字段，排除已生效项 |
| P4 jump | `server/src/combat/status.rs:174` 有字段 reset、body conditioning 有写入；server movement/`DerivedAttrsSyncV1`/client store 无 consumer | `independent-domain-fix` | 同 successor P4 | 迁移 server/client 权威设计门 |
| P5 manifest/lint | `server/src/cultivation/insight_apply.rs:24-254` 等 producer surface 当前无 checked-in consumer manifest 强制 producer→consumer | `independent-domain-fix` | 同 successor P5 | 迁移 anti-orphan gate |

## Finish Evidence

- **落地清单**：P0 结论与 P1 代码修复历史保留；P2-P5 全部迁到 `plan-bughunt-modifier-effect-consumer-completion-v1`，后者明确列 canonical 字段、排除项、设计门与跨栈验收；本文件迁入 finished。
- **关键 commit**：`3e6981513`（2026-07-09，PR #1143）接通 reach/regen/purge；已验证为 `origin/main @ c625d5a5` 祖先且当前 consumers 存在。
- **测试结果**：原 P1 Finish Evidence 记录 server tests/gate；本次只做 docs-only triage，以 docs static gate + exact-HEAD validator 验收，不复跑旧代码测试。
- **跨仓库核验**：P1 为 server-only；jump 的未实施 server/schema/client 链已明确迁 successor，不能以单端 schema 代替 runtime consumer。
- **遗留 / 后续**：唯一 successor 为 `plan-bughunt-modifier-effect-consumer-completion-v1` P2-P5；本 audit 禁止再消费。
