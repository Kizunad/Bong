# plan-bughunt-r1-mechanical-fixes-v1（已归档）

> 一句话主题：round1 七条机械 finding 已按 `origin/main @ c625d5a5` 逐项复核；六条有 merged 修复，一条转交 focused successor。本 bundle 不再是实施队列。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| T0 | 七条 finding current-code + ancestor commit/PR 复核 | ✅ 2026-07-28 |
| T1 | still-live finding 登记唯一 successor owner | ✅ 2026-07-28 |
| T2 | mapping table、Finish Evidence、归档 | ✅ 2026-07-28 |

## Finding Mapping

| Finding | 当前裁决 / current `file:line` | 分类 | Canonical owner / merged evidence | 文档动作 |
|---|---|---|---|---|
| P0 Forge Done session 无界增长 | `server/src/forge/mod.rs:1179-1187` 已注册延迟清理，`server/src/forge/session.rs:109-153` 记录完成龄 | `already-fixed/invalid`（already-fixed） | commit `b118c467a` / PR #599 | 仅归档 |
| P1 NPC skill 缺 zone 时吞 qi | `server/src/npc/npc_skill.rs:114-149` 已由 `route_spent_qi_to_overflow` 写 overflow | `already-fixed/invalid`（already-fixed） | commit `269c89e6e` / PR #1043 | 仅归档 |
| P2 crater center 每 chunk 全扫 | `server/src/world/terrain/giant_sword.rs:948-969` 已缓存，`server/src/world/terrain/giant_sword.rs:1344-1448` 有一致性测试 | `already-fixed/invalid`（already-fixed） | commit `02b646056` / PR #595 | 仅归档 |
| P3 TSY collapse route 重复注册 | `client/src/main/java/com/bong/client/network/ServerDataRouter.java:193` 当前只注册一次 | `already-fixed/invalid`（already-fixed） | commit `4478c6ff5` / PR #576 | 仅归档 |
| P4 quota release 并发丢更新 | `server/src/persistence/mod.rs:3338-3358` 已使用 IMMEDIATE transaction，`server/src/persistence/mod.rs:10703-10782` 锁行为 | `already-fixed/invalid`（already-fixed） | commit `230b9b784` / PR #590 | 仅归档 |
| P5 ascension completion 并发丢更新 | `server/src/persistence/mod.rs:3158-3181` 已使用 IMMEDIATE transaction，`server/src/persistence/mod.rs:10573-10664` 锁行为 | `already-fixed/invalid`（already-fixed） | commit `1f5d30580` / PR #585 | 仅归档 |
| P6 NPC deceased archive DB-open rollback | `server/src/persistence/mod.rs:4413-4449` 先写 bundle；`server/src/persistence/mod.rs:4429-4447` 的 DB-open 与 transaction-open 在补偿闭包外，早退仍绕过 `rollback_file` | `independent-domain-fix` | `docs/plans-skeleton/plan-bughunt-npc-deceased-archive-db-open-rollback-v1.md` | 新建 focused skeleton，唯一 owner |

## Finish Evidence

- **落地清单**：逐条完成 current-code/祖先链复核；P6 已迁唯一 successor；本 bundle 由 `docs/plan-bughunt-r1-mechanical-fixes-v1.md` 迁入本路径。
- **关键 commit / PR**：P0 `b118c467a`/#599；P1 `269c89e6e`/#1043；P2 `02b646056`/#595；P3 `4478c6ff5`/#576；P4 `230b9b784`/#590；P5 `1f5d30580`/#585。所有 commit 均验证为 `origin/main @ c625d5a5` 祖先且当前修复仍存在。
- **测试结果**：本次为 master §6.16 授权的 docs-only triage，不运行代码栈门禁；最终以 `git diff --check`、mapping/owner/path/标题 static gate 与 exact-HEAD read-only validator 为准。
- **跨仓库核验**：server 六条 + client TSY router 一条均已核对；无 schema/wire 修改。
- **遗留 / 后续**：仅 P6 仍 live，由 `plan-bughunt-npc-deceased-archive-db-open-rollback-v1` 实施；本归档文件禁止再消费。
