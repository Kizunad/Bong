# plan-bughunt-r7-findings-v1（已归档）

> 一句话主题：round7 十条 finding 已按 `origin/main @ c625d5a5` 拆散：五个 Insight 字段统一移交 modifier/effect successor，Botany drag 移交 focused owner，其余四项有 merged 修复。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| T0 | 十条 finding current-code + ancestor commit/PR 复核 | ✅ 2026-07-28 |
| T1 | Insight 与 Botany unique owner 登记 | ✅ 2026-07-28 |
| T2 | mapping table、Finish Evidence、归档 | ✅ 2026-07-28 |

## Finding Mapping

| Finding | 当前裁决 / current `file:line` | 分类 | Canonical owner / merged evidence | 文档动作 |
|---|---|---|---|---|
| #3 `qi_regen_mul` | `server/src/cultivation/insight_apply.rs:25,127` 仍只有定义/写入，无 regen production consumer | `independent-domain-fix` | `plan-bughunt-modifier-effect-consumer-completion-v1.md` P3 | 统一 successor |
| #4 `next_breakthrough_bonus` | `insight_apply.rs:27,159` 写入；breakthrough 主循环仍不读 | `independent-domain-fix` | 同上 P3 | 统一 successor |
| #5 `vortex_backfire_resist_mul` | `insight_apply.rs:36,178` 写入；woliu backfire 主循环不读 | `independent-domain-fix` | 同上 P3 | 统一 successor |
| #6 `vortex_delta_bonus_add` | `insight_apply.rs:38,184` 写入；vortex delta 仍取 realm 基值 | `independent-domain-fix` | 同上 P3 | 统一 successor |
| #7 `vortex_flow_speed_mul` | `insight_apply.rs:40,190` 写入；无 production flow-speed consumer | `independent-domain-fix` | 同上 P3 | 统一 successor |
| #9 AgentUiStore close 泄漏 | `client/.../AgentUiScreen.java:252,266` 两条关闭路径都调用 `AgentUiStore.clearIfActive` | `already-fixed/invalid`（already-fixed） | `f6d422250` / PR #709 | 仅归档 |
| #10 Botany LEFT RELEASE stale drag | `client/.../MixinMouse.java:101` screen-open 早退，`:116` 才调用 `BotanyDragState`，仍可漏收 release | `independent-domain-fix` | `docs/plans-skeleton/plan-bughunt-botany-drag-release-lifecycle-v1.md` | 新建唯一 focused owner |
| #2 duplicate skill registration | `server/src/cultivation/skill_registry.rs:85-92` 当前 duplicate registration assert fail-closed，`:200-214` 有专属 panic test，旧静默覆盖路径已删除 | `already-fixed/invalid`（already-fixed） | `fca6cdb30` / PR #711 | 仅归档 |
| #7chat PLAYER_CHAT 注释 | `agent/packages/schema/src/channels.ts:7-10` 当前注明 LRANGE/LTRIM batch drain | `already-fixed/invalid`（already-fixed） | `15a34ba4e` / PR #708 | 仅归档 |
| #8 Agent UI error union | `agent/packages/schema/src/payloads/agent-ui.ts:99-110` 已含 `invalid_command`、`xml_sanitize_failed` | `already-fixed/invalid`（already-fixed） | `ac998e6fa` / PR #707 | 仅归档 |

## Finish Evidence

- **落地清单**：五个 Insight finding 归统一 successor；Botany 新建 focused skeleton；四个已修 finding 结案；bundle 迁入本路径。
- **关键 commit / PR**：`f6d422250`/#709、`fca6cdb30`/#711、`15a34ba4e`/#708、`ac998e6fa`/#707 均为目标 HEAD 祖先且当前修复存在。
- **测试结果**：docs-only triage；最终以 docs static gates + exact-HEAD validator 验收。
- **跨仓库核验**：server Insight/registry、client Agent UI/Botany、agent TypeBox/channels 均逐条对拍。
- **遗留 / 后续**：Insight 五字段与 Botany 输入生命周期已各有唯一 owner；本 bundle 禁止再消费。
