# plan-bughunt-r1-mechanical-fixes-v1（active）

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
