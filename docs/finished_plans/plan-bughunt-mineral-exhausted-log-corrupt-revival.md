# BugHunt: 矿脉耗尽日志半写后重启复活

> 主题：让矿脉耗尽日志在失败刷盘时保留上一份有效文件，避免重启后已耗尽矿脉复活。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 失败刷盘不应覆盖 final path 的 RED 契约测试 | ✅ 2026-09-24 |
| P1 | `ExhaustedMineralsLog::flush` 同目录临时文件 + rename 原子写 | ✅ 2026-09-24 |
| P2 | 成功 roundtrip、失败保留、重启 hydrate、dirty 重试与锚点跳过回归 | ✅ 2026-09-24 |
| P3 | server 完整门禁、主线同步、Finish Evidence 与归档 | ✅ 2026-09-24 |

## Bug 摘要

`ExhaustedMineralsLog::flush` 直接用 `fs::write(&self.file_path, json)` 覆盖 `data/minerals/exhausted.json`。`std::fs::write` 走 `File::create + write_all`，打开成功后会截断既有文件；如果进程在写入中途崩溃、被 kill，或底层 I/O 在 truncate 后失败，最终路径可能留下空文件或半写 JSON。

启动期 `hydrated_from_path` 对解析失败只 warn 并返回空 log。随后 `spawn_mineral_anchor_nodes` 用这份空 log 计算 exhausted skip 集合，已耗尽矿脉位置会被重新物化。

边界：这不是 #972 dormant Redis dirty ACK 问题，也不是 `docs/plans-skeleton/plan-bughunt-r10-findings-v1.md` 已记录的“耗尽 entry 还在 30 秒节流窗口内、尚未落盘就关服/崩溃”。本缺陷是“已经存在的有效 exhausted log 被下一次 flush 直接覆盖写坏”。

## 实际游玩体验影响

玩家已经挖穿的矿脉，本应由 `data/minerals/exhausted.json` 记录为耗尽并在重启后跳过。若一次刷盘半写导致 JSON 损坏，下一次开服会把整份耗尽记录当成空状态。

结果是旧矿脉在重启后复活，永久耗尽矿物或尚未到 respawn 时间的矿物都可能提前出现。玩家会看到矿洞资源回滚，矿物有限性和矿区经济被破坏。

## 证据定位

- `server/src/mineral/persistence.rs:154-173`：`flush()` 序列化后直接 `fs::write(&self.file_path, json)` 写最终路径，没有 tmp 文件、rename、备份或恢复旧文件。
- `server/src/mineral/persistence.rs:228-254`：`hydrated_from_path` 对 corrupt/parse failure 只 warn，然后保留空 `entries`。
- `server/src/mineral/persistence.rs:415-421`：现有测试明确把坏 JSON 锁定为“空 log”。
- `server/src/mineral/mod.rs:69-77`：启动注册时从默认 `data/minerals/exhausted.json` hydrate，并把结果插入 ECS resource。
- `server/src/mineral/mod.rs:87-90`：启动阶段随后运行 `spawn_mineral_anchor_nodes`。
- `server/src/mineral/anchors.rs:95-110`：锚点物化只靠 `exhausted.entries()` 生成 skip 集合；log 为空时已耗尽位置不会被跳过。
- `server/src/mineral/anchors.rs:123-130`：化石矿脉同样使用这份 exhausted skip 集合。
- `server/src/mineral/break_handler.rs:401-410`：矿块剩余单位归零后发送 `MineralExhaustedEvent` 并 despawn/index remove。
- `server/src/mineral/persistence.rs:189-217`：生产路径每 600 tick 自动 flush dirty exhausted log，因此不需要手工破坏文件，正常运行中的刷盘即可进入风险窗口。
- 对照 `server/src/craft/unlock.rs:175-203`、`server/src/spiritwood/persistence.rs:150-193`：同仓其它 JSON 持久化已采用 tmp 写入 + rename，注释明确是为了避免写入中断留下截断 JSON。
- 对照 `server/src/craft/unlock.rs:1131-1165`、`server/src/spiritwood/persistence.rs:486-511`：同仓已有“失败写不得触碰 final path”的原子写安全测试。

## 触发路径

1. 服务器已有一份有效 `data/minerals/exhausted.json`，记录至少一个已耗尽矿脉。
2. 玩家继续挖穿另一处矿脉，`handle_block_break_for_mineral` 发送 `MineralExhaustedEvent`，`record_exhausted_minerals` 将新 entry 记入内存并标 dirty。
3. 600 tick 节流窗口到期，`ExhaustedMineralsLog::flush` 开始直接覆盖最终 `exhausted.json`。
4. 进程在 `File::create` 成功截断后、`write_all` 完整写完前崩溃/被 kill，或底层写入失败；最终文件留下空文件或半写 JSON。
5. 下次开服 `hydrated_from_path` 解析失败，warn 后启动空 log。
6. `spawn_mineral_anchor_nodes` 看到 empty exhausted set，重新物化原本已耗尽的锚点/化石矿脉。

## 反方审查记录

- 第一轮反方：候选成立。确认开放 PR 中 #971 是矿脉锚点坐标漂移，#876 是矿脉采集移动打断，#972 是 dormant Redis 写失败 dirty ACK，均不覆盖本问题；r10 skeleton 只覆盖“未及时 flush”，不覆盖“final-path overwrite 写坏已有日志”。
- 第一轮反方还确认：没有找到矿脉 exhausted log 的 tmp+rename、备份、fsync、启动阻断或从旧日志恢复的保护；启动 anchor/fossil 物化确实依赖该 log。
- 第二轮反方：通过。进一步确认 `std::fs::write` 不是“写失败保留旧文件”的接口，打开成功后会截断既有文件；同仓 craft/spiritwood 已把同类风险作为持久化契约测试，说明该风险达到 BugHunt 真实 bug 门槛。
- 第二轮边界建议：fix plan 只聚焦 `ExhaustedMineralsLog::flush` 的 final-path 直接覆盖，避免混入 r10 的 Last/AppExit 强刷缺口，也不触碰 #972 的 Redis ACK 语义。

## Skeleton Fix Plan

- 将 `ExhaustedMineralsLog::flush` 改为同目录临时文件写入：
  - 先 `fs::write(tmp_path, json)`。
  - 成功后 `fs::rename(tmp_path, self.file_path)`。
  - rename 成功后才 `dirty = false`、`flush_clock = 0`。
- 失败路径保持现有语义：
  - 返回 `Err(...)`。
  - 保持 `dirty = true`，允许后续 tick 重试。
  - 不触碰最终 `exhausted.json` 的旧内容。
- 保留 corrupt 文件启动不阻断策略，但将其视为外部/历史损坏 fallback；正常 flush 不应再制造 corrupt final file。
- 不在本计划中处理 r10 已记录的 AppExit/Last shutdown 强刷问题；若未来合并修复，可作为矿脉持久化的另一个独立补丁。

## 验收测试计划

- 在 `server/src/mineral/persistence.rs` 增加与 craft/spiritwood 同款原子写测试：
  - 先 flush 一份有效 exhausted log。
  - 读取并保存原始 final path 字节。
  - 在 `path.with_extension("tmp")` 位置创建目录，强制下一次 tmp 写失败。
  - 新增一条 entry 后调用 `flush()`。
  - 断言 `flush()` 返回 Err。
  - 断言 final path 字节与原始内容完全一致。
  - 断言 log 仍为 dirty。
- 增加 roundtrip 测试确认成功 flush 后 tmp path 不残留，`load_exhausted_log` 能读回所有 entries。
- 增加启动物化回归测试：
  - 一份有效 exhausted log 能继续让 `spawn_mineral_anchor_nodes` 跳过已耗尽位置。
  - 一次失败 flush 后重启 hydrate 仍能读旧 final file，不会把已耗尽矿脉复活。
- 按 server 栈运行：
  - `cd server && cargo fmt --check`
  - `cd server && cargo clippy --all-targets -- -D warnings`
  - `cd server && cargo test`

## 风险

- 使用固定 `.tmp` 路径时，若上次崩溃遗留 tmp 文件，下一次 flush 会覆盖 tmp 再 rename；这符合“final path 只有成功写完才替换”的目标。
- `fs::rename` 在同一目录内保持原子替换语义；tmp 必须与 final path 在同一目录，避免跨文件系统 rename。
- 修复不改变 corrupt final file 的启动 fallback；已有坏文件仍会 warn + 空 log。若需要从 `.bak` 或 tmp 恢复，应另立计划，避免扩大本修复范围。
- AppExit/Last 强刷仍是 r10 skeleton 的独立缺口；本计划不宣称解决“尚未落盘的新 entry”。

## Finish Evidence

### 落地清单

- P0：`server/src/mineral/persistence.rs::tests::flush_failure_preserves_existing_final_file_and_dirty_state` 先以 `.tmp` 目录阻塞写入，证明旧实现会错误成功并覆盖语义。
- P1：`server/src/mineral/persistence.rs::ExhaustedMineralsLog::flush` 改为同目录 `.tmp` 写入后 `fs::rename` 替换 final path，只有 rename 成功才清除 `dirty` 与 `flush_clock`。
- P2：同文件覆盖成功 roundtrip、临时文件清理、失败后 final 字节保持、重启 `hydrated_from_path` 保留旧 entry、清除阻塞后 dirty 重试；`server/src/mineral/anchors.rs::startup_spawns_index_entries_and_skips_exhausted_positions` 保持启动物化跳过契约。
- P3：在最新 `origin/main` 上完成 server fmt、clippy、全量测试门禁，并将 active plan 归档至 `docs/finished_plans/`。

### 接入面

- 进料：`server/src/mineral/break_handler.rs` 发出 `MineralExhaustedEvent`；`server/src/mineral/persistence.rs::record_exhausted_minerals` 读取事件并调用 `ExhaustedMineralsLog::record`。
- 出料：`ExhaustedMineralsLog::flush` 写入 `data/minerals/exhausted.json`；`load_exhausted_log` 与 `ExhaustedMineralsLog::hydrated_from_path` 读取持久化结果；`spawn_mineral_anchor_nodes` 消费 entries 生成启动跳过集合。
- 复用类型：`MineralExhaustedEvent`、`ExhaustedEntry`、`ExhaustedLogFile`、`ExhaustedMineralsLog`。
- server 契约：`ExhaustedMineralsLog::flush` 采用同目录临时文件加原子替换，失败时保留上一份 final 文件并保持 dirty。
- agent/client 无变更；`server/src/mineral` 未接入 `qi_physics`，本修复不涉及真元账本。

### 关键 commit

- `20e167158`（2026-09-24）：升格矿脉耗尽日志原子刷盘 BugFix 计划。
- `b8f38f4d7`（2026-09-24）：移植旧提交 `68966a891` 的 RED 失败刷盘契约测试。
- `feb2e1335`（2026-09-24）：移植旧提交 `a4377270a` 的同目录临时文件原子刷盘修复。
- `2eaa18fc5`（2026-09-24）：移植旧提交 `b4b74791a` 的 roundtrip、重启 hydrate 与重试回归。

### 测试结果

- 修复前 RED：`scripts/build-token.sh cargo test -p bong-server mineral::persistence::tests::flush_failure_preserves_existing_final_file_and_dirty_state -- --exact`，1 failed（符合预期的缺陷证据）。
- 修复后目标测试：同命令 1 passed。
- 矿脉持久化模块：`scripts/build-token.sh cargo test -p bong-server mineral::persistence::tests::`，18 passed、0 failed。
- server 完整门禁：`scripts/build-token.sh cargo fmt --check && scripts/build-token.sh cargo clippy --all-targets -- -D warnings && scripts/build-token.sh cargo test` 全部退出码 0；lib 10,372 passed/1 ignored，main 18 passed，doc-tests 3 passed/5 ignored，其余 integration suites 全部通过。

### 跨仓库核验

- server：`ExhaustedMineralsLog::flush`、`hydrated_from_path`、`anchors::spawn_mineral_anchor_nodes` 与耗尽日志 JSON 契约均已命中并通过门禁。
- agent：无 IPC、schema 或 Redis 契约变更。
- client：无 Fabric payload 或 UI 变更。

### 遗留 / 后续

- 历史上已经损坏的 `exhausted.json` 仍按既有策略 warn 后以空 log 启动；备份恢复或启动阻断需另立计划。
- AppExit/Last 关服强刷与尚未进入 flush 窗口的新 entry 仍属于 r10 独立范围，本 plan 不覆盖。
