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

- [ ] 在 `server/src/craft/unlock.rs` 增加 `flush_recipe_unlocks_on_shutdown` system：读取 `AppExit`，收到退出事件时调用 `RecipeUnlockState::flush()`。
- [ ] 在 `server/src/craft/mod.rs` 将该 system 注册到 `Last`，对齐 `player::flush_connected_players_on_shutdown` 和 zone runtime shutdown flush 的生命周期语义。
- [ ] 保持现有 600 tick 节流 flush 不变，避免运行时每次解锁同步写盘；关服 hook 只补末次 dirty state。
- [ ] flush 失败时只记录 warn，不清 dirty，不破坏旧文件；沿用现有 `.tmp` + `rename` 原子落盘语义。

## 验收测试计划

- [ ] `app_exit_flushes_dirty_recipe_unlocks_without_waiting_interval`：构造 temp path + `flush_interval_ticks = 600`，`unlock()` 后发送 `AppExit::Success` 并只跑一次 `Last`/`app.update()`，断言文件已存在且 hydrate 后包含新配方。
- [ ] `app_exit_flush_noops_when_recipe_unlock_state_clean`：clean state 收到 `AppExit` 后不创建文件、不 panic。
- [ ] `app_exit_flush_failure_keeps_dirty_and_preserves_existing_file`：用阻塞 `.tmp` 路径制造写失败，断言旧 `recipe_unlocks.json` 字节不变且 `is_dirty()` 仍为 true。
- [ ] 回归现有测试：`cd server && cargo test craft::unlock`。
- [ ] 若触及注册路径，补 `cd server && cargo test craft::` 或更窄的 shutdown system 单测，确认不跨栈。

## 风险

- 关服时同步写 JSON，理论上玩家配方解锁集合很大时会拉长退出时间；但只在 `AppExit` 末次执行，且比吞掉秘传解锁更可接受。
- 如果 flush 失败，正常退出仍可能留下 dirty 数据未落盘；实现不应在失败时破坏旧文件或假装成功。
- 材料发现可在部分情况下重触发，测试和文案应聚焦残卷 / 师承 / 顿悟等不可稳定重放的秘传渠道，避免夸大影响面。
