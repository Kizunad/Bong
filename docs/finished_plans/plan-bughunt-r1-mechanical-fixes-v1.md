# plan-bughunt-r1-mechanical-fixes-v1（已归档）

> **归档说明（2026-07-28）**：除本说明与文末 Round bundle triage 外，下列正文完整保留本 plan 在冻结基线 `origin/main @ c625d5a5` 上的原始阶段、决议、测试与审计记录；正文里的 “Active / 骨架 / ⬜ / 开放问题” 是历史状态。当前唯一实施归属以文末 `Finding Mapping` 为准，移交 successor 的条目仍未实施，不因本 bundle 归档而视为完成。


> **Active（已从 skeleton 升级，待逐项消费）**。一句话主题：代码库自检 bug-hunt round1 确认的一批**机械确定型小 bug**（内存泄漏 / 缺兜底 / 死码 / 并发事务 / 漏分支）逐个修复，每个独立小 PR。

> 立项动机：bug-hunt round1（report-only）裁决出若干 `fix_pr` 类——非设计抉择、修复方向清晰、范围受限，但仍各有小取舍（清理时机/并发隔离级），归集本 plan 逐个落地。

## 阶段总览（每条独立小 PR）

| # | bug | file:line | 修法 | 状态 |
|---|-----|-----------|------|------|
| P0 | ForgeSessions 永不移除已完成 session = 无界内存泄漏 | `server/src/forge/session.rs:179` `remove` 零生产调用；`mod.rs:630 finalize` 断指针/设 Done 但不从 HashMap 移除 | 延迟清理系统（移除 Done 超 N tick 的 session），**非 finalize 内即时 remove**（`client_request_handler.rs:3047` 仍读 Done session 拒后续操作，即时 remove 会竞态 regress） | ⬜ |
| P1 | release_npc_qi_to_zone 缺 else 兜底→真元静默销毁 | `server/src/npc/npc_skill.rs:54` `if let Some(zones)=get_resource_mut::<ZoneRegistry>()` 无 else | 补 else route 到 `QiAccountId::overflow`（镜像同函数 no-patrol/find_zone None 的 overflow 兜底，机械） | ⬜ |
| P2 | find_crater_center 每 chunk 全量扫剑海无缓存（~5.4M 冗余 terrain.sample） | `server/src/world/terrain/giant_sword.rs:967` 无条件先调全扫（934-957 step=64 扫 ~1600×1600），AABB 剔除在 975 之后 | OnceLock 缓存（输入皆世界常量，确定性安全；`mega_tree.rs:994 cached_skeleton` 是现成先例）或 AABB 剔除前移 | ⬜ |
| P3 | ServerDataRouter tsy_collapse_started_ipc 重复注册死码 | `client/.../network/ServerDataRouter.java:183` copy-paste 重复 | 删重复注册 | ⬜ |
| P4 | release_ascension_quota_slot DEFERRED 事务 read-modify-write，并发名额减少丢失 | `server/src/persistence/mod.rs:2746` | 提升事务隔离（IMMEDIATE/独占）或原子 UPDATE，防并发丢更新 | ⬜ |
| P5 | complete_tribulation_ascension（DuXu）DEFERRED 事务，并发渡劫名额增量丢失 | `server/src/persistence/mod.rs:2722` | transaction_with_behavior(IMMEDIATE)，同 try_/P4 | ✅ 2026-06-16 |
| P6 | persist_npc_deceased_archive write_zstd_bundle 成功后 open_connection 失败漏处理 | `server/src/persistence/mod.rs:3630` | 补错误分支（bundle 已写但 DB 记录失败的一致性处理） | ⬜ |

## 接入面 / 注意

- 每条独立小 PR（一 PR 一 fix），测试覆盖该 bug 的回归锁定（CLAUDE.md 饱和化）。
- P0/P4/P5 有小设计点（清理时机/并发隔离级），实施时定夺，但方向明确。
- qi_physics：P1 涉及真元守恒（overflow 兜底），按 ledger 规约。

## §N 开放问题

1. P0 清理时机阈值（Done 后多少 tick 移除）。
2. P4/P5 并发隔离用 SQLite IMMEDIATE 事务 vs 原子 UPDATE...WHERE，择优。

## 审计来源

bug-hunt round1 confirmed（fix_pr 类，7 条）。可逐个直接修，但归集本 plan 追踪。

---

## 2026-07-28 Round bundle finding triage

本节是 master §6.16 / §7 一次性 docs-only 归档移交记录；上文未实施 finding 只有在下表登记唯一 owner 后才退出原聚合队列。

## Finding Mapping

| Finding | 当前裁决 / current `file:line` | 分类 | Canonical owner / merged evidence | 文档动作 |
|---|---|---|---|---|
| P0 Forge Done session 无界增长 | `server/src/forge/mod.rs:1179-1187` 已注册延迟清理，`server/src/forge/session.rs:109-153` 记录完成龄 | `already-fixed/invalid`（already-fixed） | commit `b118c467a` / PR #599 | 仅归档 |
| P1 NPC skill 缺 zone 时吞 qi | `server/src/npc/npc_skill.rs:114-149` 已由 `route_spent_qi_to_overflow` 写 overflow | `already-fixed/invalid`（already-fixed） | commit `269c89e6e` / PR #1043 | 仅归档 |
| P2 crater center 每 chunk 全扫 | `server/src/world/terrain/giant_sword.rs:948-969` 已缓存，`server/src/world/terrain/giant_sword.rs:1344-1448` 有一致性测试 | `already-fixed/invalid`（already-fixed） | commit `02b646056` / PR #595 | 仅归档 |
| P3 TSY collapse route 重复注册 | `client/src/main/java/com/bong/client/network/ServerDataRouter.java:193` 当前只注册一次 | `already-fixed/invalid`（already-fixed） | commit `4478c6ff5` / PR #576 | 仅归档 |
| P4 quota release 并发丢更新 | `server/src/persistence/mod.rs:3338-3358` 已使用 IMMEDIATE transaction，`server/src/persistence/mod.rs:10703-10782` 锁行为 | `already-fixed/invalid`（already-fixed） | commit `230b9b784` / PR #590 | 仅归档 |
| P5 ascension completion 并发丢更新 | `server/src/persistence/mod.rs:3158-3181` 已使用 IMMEDIATE transaction，`server/src/persistence/mod.rs:10573-10664` 锁行为 | `already-fixed/invalid`（already-fixed） | commit `1f5d30580` / PR #585 | 仅归档 |
| P6 NPC deceased archive DB-open rollback | `server/src/persistence/mod.rs:5931-5982`（`persist_npc_deceased_archive_with_hooks`）先写 bundle；`open_connection`、`connection.transaction()`、index upsert 与 hot-row 删除均位于 `persisted` 补偿闭包内，任一失败均进入 `rollback_file`；`npc_archive_transaction_begin_failure_restores_previous_bundle` 等回归测试锁定失败原子性 | `already-fixed/invalid`（already-fixed） | R3 `docs/plan-refactor-persistence-slices-v1.md`；commit `57d6801b03b09a0c10b79a0cc0cca22c252642b4` / 2026-08-05 | 已由 R3 交付并闭环，仅归档，不再作为 live 吸收项 |

## Finish Evidence

- **落地清单**：逐条完成 current-code/祖先链复核；P6 的 DB-open/transaction-open rollback 已由 R3 交付并闭环，当前仅作已完成归档记录；本 bundle 由 `docs/plan-bughunt-r1-mechanical-fixes-v1.md` 迁入本路径。
- **关键 commit / PR**：P0 `b118c467a`/#599；P1 `269c89e6e`/#1043；P2 `02b646056`/#595；P3 `4478c6ff5`/#576；P4 `230b9b784`/#590；P5 `1f5d30580`/#585。所有 commit 均验证为 `origin/main @ c625d5a5` 祖先且当前修复仍存在。
- **测试结果**：本次为 master §6.16 授权的 docs-only triage，不运行代码栈门禁；最终以 `git diff --check`、mapping/owner/path/标题 static gate 与 exact-HEAD read-only validator 为准。
- **跨仓库核验**：server 六条 + client TSY router 一条均已核对；无 schema/wire 修改。
- **遗留 / 后续**：P6 已由 R3 交付并闭环，不再遗留 live 项；本归档文件禁止再消费。
