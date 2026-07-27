# plan-bughunt-npc-deceased-archive-db-open-rollback-v1（骨架）

> 一句话主题：补齐 NPC deceased archive 的 zstd bundle + SQLite 双写失败补偿，确保 DB 打开/建事务失败不会留下只有文件、没有索引行的半提交 archive。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 冻结 bundle/DB 双写事务语义与故障矩阵 | ⬜ |
| P1 | `persist_npc_deceased_archive` 全失败分支补偿 + 饱和测试 | ⬜ |
| P2 | server gate + restart/orphan scanner 回归 | ⬜ |

## 接入面

- **进料**：`server/src/persistence/mod.rs::persist_npc_deceased_archive`、`write_zstd_bundle`、`open_persistence_connection`、SQLite `npc_deceased_archive` 索引。
- **出料**：成功时 bundle 与 DB 行同时可见；失败时恢复旧 bundle（或删除本轮新 bundle），不得产生 orphan。
- **共享类型 / event**：复用 `NpcDeceasedArchiveRecord`、`rollback_file` 与现有 persistence error 类型；不另造第二套 archive store。
- **跨仓库契约**：纯 server persistence，无 wire 改动。
- **worldview 锚点**：NPC 死亡事实与离屏世界连续性；不新增玩法或数值。
- **qi_physics 锚点**：不改变 archive 携带的 qi 快照；失败补偿不得丢弃或重复释放 qi。

## 当前证据（origin/main @ c625d5a5）

`server/src/persistence/mod.rs:4413-4449` 先读取旧 archive、写新 zstd bundle，随后 `:4429` 的 `open_persistence_connection(settings)?` 与 transaction-open 均位于现有 `persisted` 补偿闭包之外；两者任一早退都会绕过 `:4447` 的 `rollback_file`，只有事务闭包内部失败才回滚文件。

## 验收

1. 把 DB-open、transaction-open、SQL execute、commit、bundle-write 五个故障点逐一注入；除 bundle-write 前失败外，所有半提交都恢复 exact previous bytes，原文件不存在时删除新文件。
2. success 覆盖旧文件替换与首次写入；重复保存同 archive 幂等，不删除已提交 DB 行。
3. restart/orphan scanner 在上述失败矩阵后不得发现本轮新 orphan；已有历史 orphan 的告警语义不变。
4. 运行 `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`。

## 边界

- **pre-R3 P1 窄例外**：本 plan 是 `persist_npc_deceased_archive` file+DB 补偿原子性及同地失败矩阵的唯一 owner，而非第二条 persistence track；仅在 R3 P1 尚未迁移/改写该 symbol 时实施。R3 P0 只冻结接口；若 R3 P1 先触及该 symbol，则本 plan 停止独立实施、由 R3 接管同一验收矩阵，禁止双 PR。
- 不扩展为通用分布式事务框架，不改 archive schema，不处理 SIGKILL/断电后的 fsync 保证。
