# BugHunt: 配方解锁关服前未强制落盘

## Bug 摘要

`RecipeUnlockState` 已经用 `data/craft/recipe_unlocks.json` 持久化玩家通过残卷、师承、顿悟和材料发现获得的配方解锁进度，但当前只在 `Update` 中按 600 tick 节流 flush，没有接 `AppExit` / `Last` 关服强制 flush。玩家刚解锁秘传配方后，如果服务器在 30 秒节流窗口内正常关服或重启，内存里的 dirty 解锁不会写入 `recipe_unlocks.json`，下次启动 `RecipeUnlockState::hydrated()` 会从旧文件恢复，导致最近解锁的配方消失。

## 对实际游玩体验的影响

玩家刚消耗残卷、完成师承或触发顿悟解锁一个配方，客户端当场会看到新配方出现在制作列表里；如果服主随后重启服务器，玩家重连后该配方可能从制作列表消失。残卷或触发代价如果已经由库存/任务等其他系统落盘，玩家会表现为“道具/机会花掉了，但配方被重启吞了”。材料发现类解锁在玩家仍持有原料时可能登录后再次触发，但这不覆盖秘传渠道，也不消除关服窗口内的持久化不一致。

## 证据定位

- `server/src/craft/unlock.rs:89-96`：`RecipeUnlockState::default()` 设置 `flush_interval_ticks = 600`，约 30 秒。
- `server/src/craft/unlock.rs:143-149`：`unlock()` 新增配方时只插入 `by_player` 并置 `dirty = true`，不立即写盘。
- `server/src/craft/unlock.rs:175-206`：已有 `flush()` 强制刷盘 API，注释写明可供“测试 / 关服 hook”使用，但当前没有关服调用点。
- `server/src/craft/unlock.rs:263-279`：`tick_recipe_unlock_flush` 只有在 `flush_clock >= flush_interval_ticks && dirty` 时才写 `recipe_unlocks.json`。
- `server/src/craft/mod.rs:127-153`：`craft::register` 启动 hydrate `RecipeUnlockState` 后，只注册 `Update` 的 `tick_recipe_unlock_flush`，没有 `Last` / `AppExit` system。
- `server/src/network/craft_emit.rs:600-650`：残卷、师承、顿悟 intent 处理路径写 `RecipeUnlockState` 并广播 `RecipeUnlockedEvent`，但不刷盘。
- `server/src/network/craft_emit.rs:719-760`：材料发现路径同样写 `RecipeUnlockState` 并刷新配方列表，仍依赖后续节流 flush。
- `server/src/player/mod.rs:116`、`server/src/player/mod.rs:461-490`：玩家状态已有 `Last` + `AppExit` shutdown flush，说明本仓库正常关服路径可以做末次落盘，但 craft unlock 没接。
- `server/src/persistence/mod.rs:661-685`、`server/src/persistence/mod.rs:854-868`：persistence 模块只对 zone runtime 做关服强制 flush，不会统一覆盖 `RecipeUnlockState`。
- `docs/finished_plans/plan-module-wiring-gaps-v1.md:80-85`：历史修复只落地 hydrate/dirty/Update 节流 flush，并明确把 AppExit 关服刷盘 defer，未覆盖本窗口。

## 触发路径

1. 玩家通过残卷、师承或顿悟触发 `CraftUnlockIntent`。
2. `apply_unlock_intents` 调用 `unlock_via_*`，最终 `RecipeUnlockState::unlock()` 把配方写入内存并置 dirty。
3. `RecipeUnlockedEvent` 和配方列表刷新让玩家当场看到配方已解锁。
4. 600 tick 节流 flush 到达前，服主正常停服或重启。
5. 关服 `Last` 阶段没有 craft unlock flush，`recipe_unlocks.json` 仍是旧内容。
6. 重启后 `RecipeUnlockState::hydrated()` 从旧文件恢复，最近解锁的秘传配方丢失。

## 反方审查记录

- 第 1 轮反方：独立检查 craft unlock dirty/flush 链路，确认 `unlock()` 只标 dirty、默认 600 tick 节流、`craft::register` 无关服 flush；未发现 #969-#1072 覆盖同主题。结论支持候选，置信度高。
- 第 2 轮反方：专门寻找反证、全局 flush、即时 flush 和重复 plan。确认玩家 shutdown flush 与 zone runtime shutdown flush 都是各自专用系统，不覆盖 `RecipeUnlockState`；历史 `plan-module-wiring-gaps-v1` 只修“完全不持久化”，且把 AppExit flush 明确留作后续。结论：成立，不重复。
- 被放弃的备选候选：灵田运行态 session 断线后继续结算也有证据，但更接近 gameplay/session 语义且与既有 botany/lingtian 断线主题相邻；本 plan 选择更贴合 persistence 分区、重复风险更低的 recipe unlock 关服落盘缺口。

## Skeleton Fix Plan

- [x] 在 `server/src/craft/unlock.rs` 增加 `flush_recipe_unlocks_on_shutdown` system：读取 `AppExit`，收到退出事件时调用 `RecipeUnlockState::flush()`。
- [x] 在 `server/src/craft/mod.rs` 将该 system 注册到 `Last`，对齐 `player::flush_connected_players_on_shutdown` 和 zone runtime shutdown flush 的生命周期语义。
- [x] 保持现有 600 tick 节流 flush 不变，避免运行时每次解锁同步写盘；关服 hook 只补末次 dirty state。
- [x] flush 失败时只记录 warn，不清 dirty，不破坏旧文件；沿用现有 `.tmp` + `rename` 原子落盘语义。

## 验收测试计划

- [x] `app_exit_flushes_dirty_recipe_unlocks_without_waiting_interval`：构造 temp path + `flush_interval_ticks = 600`，`unlock()` 后发送 `AppExit::Success` 并只跑一次 `Last`/`app.update()`，断言文件已存在且 hydrate 后包含新配方。
- [x] `app_exit_flush_noops_when_recipe_unlock_state_clean`：clean state 收到 `AppExit` 后不创建文件、不 panic。
- [x] `app_exit_flush_failure_keeps_dirty_and_preserves_existing_file`：用阻塞 `.tmp` 路径制造写失败，断言旧 `recipe_unlocks.json` 字节不变且 `is_dirty()` 仍为 true。
- [x] 回归现有测试：`cd server && cargo test craft::unlock`。
- [x] 补全生产 `SIGINT` / `SIGTERM → AppExit → Last` 子进程回归与精确 PID lifecycle shell 回归，确认不跨栈。

## 风险

- 关服时同步写 JSON，理论上玩家配方解锁集合很大时会拉长退出时间；但只在 `AppExit` 末次执行，且比吞掉秘传解锁更可接受。
- 如果 flush 失败，正常退出仍可能留下 dirty 数据未落盘；实现不应在失败时破坏旧文件或假装成功。
- 材料发现可在部分情况下重触发，测试和文案应聚焦残卷 / 师承 / 顿悟等不可稳定重放的秘传渠道，避免夸大影响面。

## Finish Evidence

### 落地清单

- `server/src/craft/unlock.rs` / `server/src/craft/mod.rs`：dirty 配方解锁继续按 600 tick 运行期节流，并在 `Last` 观察 `AppExit` 时执行原子末次 flush；失败仅告警且保留 dirty 状态。
- `server/src/shutdown.rs`、`server/src/main.rs`：命名 listener thread 在 Unix 注册 `SIGINT`/`SIGTERM`，通过容量为 1 的 channel 在 `PreUpdate` 仅发送一次 `AppExit::Success`；生产 `build_server_app()` 安装 bridge 并断言 resource 已装配，非 Unix Ctrl-C 初次 poll 已完成时直接排队退出而不重复 await。
- `server/tests/shutdown_signal.rs`：真实子进程通过生产 `build_server_app()` 注册链路在 ready 后接收 `kill -INT` 与 `kill -TERM`，验证正常退出、600 tick 前未提前落盘、JSON hydrate、版本、解锁和无 `.tmp` 残留。
- `scripts/lib/bong-server-lifecycle.sh`、`scripts/{start,stop,dev-reload}.sh`：完整生命周期事务由同一 `flock` 串行；ownership record 以 PID、`/proc/<pid>/stat` starttime、canonical executable 和 `/proc/<pid>/exe` device/inode image identity 验证。标准停服按 TERM→有界等待→身份复核→KILL 处理，未验证记录或任意 tmux 窗口 pane 后代中的未记录 server 会 fail closed，绝不按 server 名称杀进程。
- `scripts/test-server-lifecycle.sh`、`scripts/test-dev-reload-disown.sh`、`scripts/smoke-test-e2e.sh`：锁住优雅 TERM、KILL 升级、malformed/foreign record fail-closed、跨锁串行、可执行文件置换、跨 tmux 窗口 pane 后代 server 检测、相对 `CARGO_TARGET_DIR`、无 name-based kill 与 detached launch record 契约。

### 关键 commit

- `79b2c87e8`（2026-07-24）`修复配方解锁关服窗口丢失`。
- `c4ca31f54`（2026-07-25）`修复服务端信号关服刷盘`。
- `402fb6651`（2026-07-25）`修复服务端停服进程归属`。
- 本次提交：补齐生产 builder signal probe、非 Unix Ctrl-C 初次 poll 状态机、可执行 image identity 与全事务 lifecycle lock；commit hash 以本节归档提交后记录为准。

### 测试结果

- `cd server && cargo test craft::unlock`：54 passed。
- `cd server && cargo test shutdown`：7 passed。
- `cd server && cargo test --test shutdown_signal`：2 passed（真实 SIGINT/SIGTERM，经生产 builder）。
- `cd server && cargo test --test full_app_startup`：1 passed。
- `cd server && cargo clippy --all-targets -- -D warnings`：通过。
- `cd server && cargo test`：11,890 个库测试、11 个 binary 测试、全部 integration tests 通过；5 个既有 doc-test ignored。
- `bash scripts/test-server-lifecycle.sh` 与 `bash scripts/test-dev-reload-disown.sh`：通过。
- `bash scripts/smoke-test-e2e.sh`：本地两次在 release 编译尚未完成时由执行环境向 `rustc` 发送 SIGTERM，harness 因此报 missing world bootstrap anchor，未到 server 启动或本 plan 的 shutdown 路径；交由推送后的 CI 隔离环境重新执行，不能记为本地通过。
- `cd server && cargo fmt --check`：仅报告未触及的 `server/src/network/vfx_animation_trigger.rs:3252` 基线格式差异；本 plan 改动的 Rust 文件已执行 `rustfmt`。

### 跨仓库核验

本修复为 server 与本地 lifecycle 脚本闭环；完整 `cargo test` 已在 Agent/Schema 构建产物准备后通过既有 cross-stack narration 测试，未修改 client 或 Agent 协议契约。

### 遗留 / 后续

- `stop.sh` 中 Tiandao 与 Redis 的既有 name-based/process ownership 语义不在本 plan 范围；本次只消除 server 的宽泛 kill。
- 本地 smoke e2e 的 release build 被执行环境 SIGTERM 两次，CI 必须在新 HEAD 上重新确认完整 e2e；未取得 CI 通过前不得把本 plan 视为最终门禁全绿。
- `cargo fmt --check` 仍会报告未触及的 `server/src/network/vfx_animation_trigger.rs` 基线格式差异；本计划未将其纳入改动。
