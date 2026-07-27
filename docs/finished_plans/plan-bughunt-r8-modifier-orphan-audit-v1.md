# plan-bughunt-r8-modifier-orphan-audit-v1（已归档）

> 一句话主题：本 plan 的验证史与 P1 修复已完成；P2-P5 未实施工作已逐项迁入唯一 successor，原 active audit 不再承担实现队列。

## 阶段总览

| 阶段 | 内容 | 归档状态 |
|---|---|---|
| P0 | orphan/非 orphan 第一性验证矩阵 | ✅ 2026-07-07 |
| P1 | scar circuit reach/regen/purge consumer | ✅ 2026-07-08 |
| P2 | Iron cocoon wound/flow | ↗ 已移交 successor（未实施；归档仅保留验证/迁移记录） |
| P3 | InsightModifiers consumer | ↗ 已移交 successor（未实施；归档仅保留验证/迁移记录） |
| P4 | jump authority/wire/consumer | ↗ 已移交 successor（未实施；归档仅保留验证/迁移记录） |
| P5 | anti-orphan manifest/lint | ↗ 已移交 successor（未实施；归档仅保留验证/迁移记录） |

## Finding Mapping

| Finding / 阶段 | 当前裁决 / current `file:line` | 分类 | Canonical owner / merged evidence | 文档动作 |
|---|---|---|---|---|
| P0 验证矩阵 | `server/src/combat/components.rs:311-392`、`server/src/cultivation/insight_apply.rs:24-98` 与 `server/src/combat/body_conditioning.rs:157-167` 的 producer/consumer 已完成第一性分类；误报项已剔除 | `already-fixed/invalid`（audit completed） | 本归档保留验证史；实现 owner 见下 | 不再作为 active queue |
| P1 scar circuit 三字段 | `server/src/combat/player_attack.rs:37-107` 消费 reach；`server/src/cultivation/tick.rs:240-269` 消费 regen；`server/src/cultivation/contamination.rs:97-211` 消费 purge | `already-fixed/invalid`（already-fixed） | `3e6981513` / PR #1143 | 不重复实施 |
| P2 Iron cocoon | `server/src/combat/baomai_v4/iron_cocoon.rs:110-140` 写 wound/flow 字段，resolve/effective meridian rate 仍不读 | `independent-domain-fix` | `docs/plans-skeleton/plan-bughunt-modifier-effect-consumer-completion-v1.md` P2 | 完整迁移设计门 |
| P3 InsightModifiers | `observe_chance_bonus` 当前仅有默认值（`server/src/cultivation/insight_apply.rs:34,81`；无 effect 写入）；`server/src/cultivation/technique_observe.rs:64-87` 的 `observe_learn_chance` 读取该字段，但 `server/src/cultivation/technique_observe.rs:90-134` 的 `evaluate_observe_attempt` 仅由 tests（`server/src/cultivation/technique_observe.rs:253,280,315`）调用、无 production caller；其余 live 字段见 `server/src/cultivation/insight_apply.rs:24-254` | `independent-domain-fix` | 同 successor P3 | 仅迁 live 字段，排除已生效项 |
| P4 jump | `server/src/combat/status.rs:174` 有字段 reset、`server/src/combat/body_conditioning.rs:157-167` 有写入；`server/src/schema/combat_hud.rs:209-220`、`server/src/network/derived_attrs_emit.rs:76-90` 与 `client/src/main/java/com/bong/client/combat/store/DerivedAttrsStore.java:13-28` 均无 jump 字段/consumer | `independent-domain-fix` | 同 successor P4 | 迁移 server/client 权威设计门 |
| P5 manifest/lint | `server/src/cultivation/insight_apply.rs:24-254` 等 producer surface 当前无 checked-in consumer manifest 强制 producer→consumer | `independent-domain-fix` | 同 successor P5 | 迁移 anti-orphan gate |

## Finish Evidence

- **落地清单**：P0 结论与 P1 代码修复历史保留；P2-P5 全部迁到 `plan-bughunt-modifier-effect-consumer-completion-v1`，后者明确列 canonical 字段、排除项、设计门与跨栈验收；本文件迁入 finished。
- **关键 commit**：`3e6981513`（2026-07-09，PR #1143）接通 reach/regen/purge；已验证为 `origin/main @ c625d5a5` 祖先且当前 consumers 存在。
- **测试结果**：原 P1 Finish Evidence 记录 server tests/gate；本次只做 docs-only triage，以 docs static gate + exact-HEAD validator 验收，不复跑旧代码测试。
- **跨仓库核验**：P1 为 server-only；jump 的未实施 server/schema/client 链已明确迁 successor，不能以单端 schema 代替 runtime consumer。
- **遗留 / 后续**：唯一 successor 为 `plan-bughunt-modifier-effect-consumer-completion-v1` P2-P5；本 audit 禁止再消费。
