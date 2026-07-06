# BugHunt: 灵木采伐关服前未强制落盘

## Bug 摘要

`SpiritWoodHarvestedLogs` 已经用 `data/spiritwood/harvested.json` 记录已采伐灵木，防止程序化地形在重启后把同一根灵木 log 重新生成。但当前只在 `Update` 中按 600 tick 节流 flush，没有接 `AppExit` / `Last` 关服强制 flush。玩家采完灵木后若在 30 秒节流窗口内正常停服或重启，玩家背包里的 `ling_mu_gun` 可以先随 inventory flush 落盘，而 harvested log 仍停留在内存 dirty 状态，重启 hydrate 后该灵木位置缺记录，会再次变成可采。

## 对实际游玩体验的影响

玩家砍完一处稀缺灵木、拿到灵木原木后，如果服主很快重启服务器，重进同一地点可能再次看到并采集同一根灵木。结果是高价值载体材料被复刷，破坏灵木作为稀缺资源的节奏，也会让依赖灵木的炼器、阵法、棺材等生产链成本失真。

## 证据定位

- `server/src/spiritwood/mod.rs:54-78`：`spiritwood::register` hydrate `SpiritWoodHarvestedLogs`，但只注册 `Update` 的 `tick_spiritwood_harvested_flush`，没有 `Last` / `AppExit` 系统。
- `server/src/spiritwood/mod.rs:256-260`：采伐完成路径先 `store.remove`，随后只调用 `harvested_logs.mark_harvested(...)`。
- `server/src/spiritwood/persistence.rs:85-92`：默认 `flush_interval_ticks = 600`，约 30 秒。
- `server/src/spiritwood/persistence.rs:126-135`：`mark_harvested` 只插入 entry 并置 `dirty = true`，不写盘。
- `server/src/spiritwood/persistence.rs:150-190`：`flush()` 是已有强制刷盘 API，注释也标明可供关服 hook 使用，但当前未被关服路径调用。
- `server/src/spiritwood/persistence.rs:257-272`：现有 flush system 只有 `flush_clock >= flush_interval_ticks && dirty` 时才写盘。
- `server/src/player/mod.rs:116`、`server/src/player/mod.rs:461-490`：玩家状态已有 `Last` + `AppExit` shutdown flush，对比说明本仓库正常关服路径可以做末次落盘，但 spiritwood 没接。
- `server/src/player/mod.rs:754-767`：玩家 inventory changed 会立即 flush，形成“物品已持久化、灵木 harvested log 未持久化”的不一致窗口。
- `server/src/world/terrain/mod.rs:622-630`、`server/src/world/terrain/mod.rs:755-806`、`server/src/world/terrain/mod.rs:812-823`：chunk 生成时从程序化地形重建，只有 `erase_harvested_spiritwood_logs` 读取 harvested log 后才擦除已采灵木。
- `server/src/world/terrain/mega_tree.rs:203-238`：灵木 log 位置由 terrain 过程式判定；日志缺 entry 时重启后仍会命中。

## 触发路径

1. 玩家在程序化灵木巨树位置完成采伐。
2. `complete_spiritwood_sessions` 调 `mark_harvested`，内存记录置 dirty，并把当前 `ChunkLayer` 对应 block 设为 AIR。
3. 同一完成路径给玩家发放 `ling_mu_gun`，inventory changed flush 可以先把物品写入玩家存档。
4. 600 tick 节流 flush 到达前，服主正常停服或重启。
5. 重启后 `SpiritWoodHarvestedLogs::hydrated()` 从旧 `harvested.json` 恢复，缺少刚采伐的位置。
6. 玩家再次加载该 chunk，程序化地形重新生成灵木 log，`erase_harvested_spiritwood_logs` 没有对应位置可擦除，同一资源复活。

## 反方审查记录

- 第 1 轮反方：尝试寻找即时 flush、全局 flush、`AppExit` 后自动跑满 600 tick、或开放 PR 重复项；未找到。结论支持候选，指出玩家 inventory immediate flush 与 spiritwood dirty window 会制造持久化不一致。
- 第 2 轮反方：重点挑战“是否只是 kill -9”和“当前方块 AIR 是否会由世界存档保留”。审查确认 player 模块已有 `AppExit` + `Last` 正常关服 flush 测试范式，而 spiritwood 无订阅；地形生成依赖 harvested log 擦除程序化灵木，不存在可见的 vanilla block diff 持久化链路能替代该 log。结论继续支持。
- 会推翻该 bug 的条件：若服务器正常停服不发送 `AppExit` 或发送后不执行 `Last`；若 `ChunkLayer::set_block(AIR)` 另有未发现的可靠世界差异存档且优先覆盖 Bong 程序化地形；若采伐完成时改为即时 flush。当前仓库证据不满足这些条件。

## Skeleton Fix Plan

- [ ] 在 `spiritwood::persistence` 增加 `flush_spiritwood_harvested_on_shutdown` system：读取 `AppExit`，若收到退出事件则调用 `SpiritWoodHarvestedLogs::flush()`。
- [ ] 在 `spiritwood::register` 将该 system 注册到 `Last`，对齐 `player::flush_connected_players_on_shutdown` 的关服落盘模式。
- [ ] 保持现有 600 tick 节流 flush 不变，避免每次采伐同步写盘；关服 hook 只补末次 dirty state。
- [ ] flush 失败时只记录 warn，不清 dirty，不破坏旧文件；沿用现有 `.tmp` + `rename` 原子落盘。

## 验收测试计划

- [ ] `app_exit_flushes_dirty_spiritwood_logs_without_waiting_interval`：构造 temp path + `flush_interval_ticks = 600`，`mark_harvested` 后发送 `AppExit::Success` 并只跑一次 `app.update()`，断言文件已存在且 hydrate 后包含该位置。
- [ ] `app_exit_flush_noops_when_clean`：clean log 收到 `AppExit` 后不创建文件、不 panic。
- [ ] `app_exit_flush_failure_keeps_dirty_and_preserves_existing_file`：用阻塞 `.tmp` 路径制造写失败，断言旧 `harvested.json` 字节不变且 `is_dirty()` 仍为 true。
- [ ] 回归现有测试：`cd server && cargo test spiritwood::persistence`。
- [ ] 需要触及注册路径时补 `cd server && cargo test spiritwood::` 或更窄的 register/flush system 单测，确保不跨栈。

## 风险

- 关服时同步写 JSON，理论上 harvested log 很大时会拉长退出时间；但只在 `AppExit` 末次执行，且比复刷稀缺资源更可接受。
- 若 flush 失败只 warn，正常退出仍可能留下 dirty 数据未落盘；但不能在失败时破坏旧文件或假装成功。
- 需要避免把关服 hook 挂进 `Update` 后依赖 tick 顺序；应使用 `Last` 明确表达 shutdown flush 语义。
