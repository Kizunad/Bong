# plan-bughunt-scatter-bead-burial-restart-loss-v1（骨架）

> BugHunt persistence r02。主题：散真元珠 `qi_scatter_bead` 的预埋态只存在内存 `ScatterBeadBurials`，正常关服/重启后丢失；玩家库存里的珠子已被消费并持久化，埋设诡雷却无法触发、无法继续逸散。

## 实际游玩体验影响

玩家把散真元珠埋在地上当追踪干扰/诡雷后，只要服务器正常重启，珠子已经从背包扣掉，但埋设记录消失。回服后玩家不能主动触发自己的埋设珠，非触发的自然逸散也停止；表现为“材料花了、陷阱没了、区域扰动没了”，且真元封存/释放生命周期不再符合几小时逸散的阵法语义。

## 复现路径

1. 给玩家 Alice 一颗 `qi_scatter_bead`，发送 `qi_scatter_bead_use` 并携带 `x/y/z` 坐标；`server/src/network/client_request_handler.rs:1602` 到 `:1629` 会把完整坐标转成 `ScatterBeadUseRequest { bury_pos: Some(...) }`。
2. `handle_scatter_bead_use` 在埋设分支前先 `consume_item_instance_once` 扣掉背包实例，随后只调用 `burials.insert(...)` 并创建 `qi_scatter_buried:{owner}:{bead_id}` ledger source；见 `server/src/zhenfa/mod.rs:2489` 到 `:2526`。
3. 玩家断线或服务器关服时，玩家 `PlayerInventory` 会通过 `save_player_slices_with_coffin(..., player_inventory, ...)` 落盘；断线路径见 `server/src/player/mod.rs:318` 到 `:421`，关服路径见 `server/src/player/mod.rs:461` 到 `:570`。
4. 重启后 `zhenfa::register` 直接 `insert_resource(ScatterBeadBurials::default())`，没有从 sqlite/Redis/player slice hydrate；见 `server/src/zhenfa/mod.rs:601` 到 `:631`。
5. owner trigger 只从 `scatter.burials.trigger_buried(...)` 取记录，自然逸散 tick 也只遍历 `burials.beads`；见 `server/src/zhenfa/mod.rs:2570` 到 `:2628`、`:2632` 到 `:2714`。重启后 resource 为空，触发与逸散都失效。

## 根因证据

- `ScatterBeadBurial` / `ScatterBeadBurials` 是 zhenfa 模块内纯 Bevy `Resource`，字段包含 `owner: Entity`、`owner_player_id`、坐标、`remaining_qi`、`last_tick`，没有序列化模型；见 `server/src/zhenfa/mod.rs:178` 到 `:233`。
- 注册时每次启动都重建空 `ScatterBeadBurials`；见 `server/src/zhenfa/mod.rs:601` 到 `:606`。
- 全仓搜索 `ScatterBeadBurials` / `qi_scatter_buried` 只命中 zhenfa handler、tick、测试和文档，没有 `persistence` / `hydrate` / `save` / `load` 接线。
- 原设计不是一次性临时特效：`docs/finished_plans/plan-zhenfa-content-v2.md:75` 把散灵珠锚到“真元封入环境方块做诡雷”“预埋真元几小时后随载体朽坏”；同文 `:163` 到 `:168` 要求预埋后每 tick `qi_excretion(..., ContainerKind::EmbeddedTrap, ...)` 持续逸散，并维护 `bead_remaining + 已注入 zone == QI_SCATTER_BEAD_CAPACITY`。
- 已有 r9 skeleton 的“散灵珠 ledger 僵尸账户”只覆盖耗尽/触发后不清理 ledger account；本 plan 覆盖的是重启后 burial 本体丢失。#1044“可放置实体重启丢失”覆盖 workbench/container/dead_drop 纯 entity，不包含 `qi_scatter_bead` 的 `ScatterBeadBurials`。

## 修复计划骨架

### P0：持久化模型与权威键

- 新增 `zhenfa_scatter_bead_burials` 持久化表或等价 persistence slice，字段至少包含 `bead_id`、`owner_player_id`、`pos_x/y/z`、`remaining_qi`、`last_tick`、`last_wall_secs`、`schema_version`、`updated_wall_secs`。
- 存储权威 owner 使用 `owner_player_id`，不要把 Bevy `Entity` 当跨重启/跨重连身份。运行态需要当前在线实体时，用 username/canonical player id 重新绑定。
- 决定 `next_id` 策略：持久化 `next_id` 或从表内最大 `bead_id` 恢复，避免重启后新埋设覆盖旧 id。

### P1：埋设写入与失败回滚

- 埋设成功路径在扣物、插入 `ScatterBeadBurials`、初始化 ledger source、写持久化记录之间保持原子语义。
- 若 ledger/source 初始化或持久化失败，必须回滚内存 burial，并恢复/拒绝库存扣减，避免“珠子扣了但没有权威埋设记录”。
- 写入后标 dirty，后续 tick 更新 `remaining_qi` / `last_tick` 时同步 dirty。

### P2：启动 hydrate 与关服 flush

- persistence bootstrap 后 hydrate `ScatterBeadBurials`，恢复坐标、remaining、owner id、next id。
- 重建 `WorldQiAccount` 中的 buried source balance，使 `balance(source) == remaining_qi`；不要把 `WorldQiAccount` 全局不持久化本身单独扩成 bug。
- 关服 `Last` 阶段 flush 所有 dirty burial；正常 tick 可节流落盘，但不能只靠长期 autosave，避免关服前回滚。

### P3：重启间隔与守恒补算

- 明确停服期间是否按墙钟补算逸散。若补算，hydrate 时用 `last_wall_secs` 计算 elapsed，调用既有 `qi_excretion(EmbeddedTrap)`，把泄露量通过 `release_scatter_qi_to_zone` / ledger transfer 守恒进入 zone 或 overflow。
- 若产品决定停服期间不推进，则仍必须恢复 `remaining_qi` 与后续 tick 的连续性，不能重置为满值或直接删除。
- remaining 归零后同时删除持久化记录；不要重复 r9 的 ledger 僵尸账户修复，但要保证本表无已耗尽孤儿行。

### P4：触发与重连身份

- owner trigger 判定从 `owner_player_id` 出发，允许同一玩家断线重连后触发自己的埋设珠。
- 非 owner 触发仍拒绝，并保留埋设记录。
- 触发成功释放剩余真元后删除内存与持久化记录，发出既有 zone disturbance、VFX、narration。

## 验证计划

- server 单测：埋设一颗散真元珠，模拟正常关服 flush，创建新 App / 新 persistence bootstrap，断言 `ScatterBeadBurials` 恢复同一 bead、pos、owner_player_id、remaining。
- server 单测：重启后 owner 能触发恢复的 bead，非 owner 仍拒绝；触发后持久化记录删除。
- server 单测：埋设后推进 60 秒、落盘、重启，remaining 不回到 `QI_SCATTER_BEAD_CAPACITY`；若启用墙钟补算，则断言重启间隔产生的泄露量守恒进入 zone/overflow。
- server 单测：持久化写失败时不允许只扣库存不建 burial；ledger 初始化失败时也要恢复/拒绝库存扣减。
- 回归：现有 `buried_scatter_bead_excretes_conservatively_and_elapsed_zero_is_stable`、`buried_scatter_bead_owner_trigger_releases_remaining_qi`、`buried_scatter_bead_trigger_requires_owner` 继续通过。
- 命令：`cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`。

## 对抗复核结论

### 候选证据

主张：`qi_scatter_bead` 埋设路径先消耗玩家物品，再把长期运行态只放进 `ScatterBeadBurials` 内存 Resource；重启后 Resource 默认空，触发和自然逸散都失效。

### 反方质疑

- 可能与 #1044“可放置实体重启丢失”重复。
- 需要证明玩家库存消耗会正常落盘，否则“玩家物品没了”不一定成立。
- 需要证明预埋语义应跨服务器重启，而不是一次性运行态特效。
- 不应把 `WorldQiAccount` 不持久化单独当 bug，因为部分 persistence 记录明确不碰 ledger。

### 修正 / 反驳

- #1044 覆盖 `workbench_item`、`trade_crate`、`herb_crate_placed`、`dead_drop_box` 等 `PlaceableBlockKind` 纯 entity，不包含 `qi_scatter_bead` 或 `ScatterBeadBurials`。
- 玩家断线与关服路径都把 `Option<&PlayerInventory>` 传给 `save_player_slices_with_coffin`，已有断线/关服测试断言 inventory JSON 落盘。
- `plan-zhenfa-content-v2` 明确预埋散灵珠应几小时持续逸散，并维护 `bead_remaining + 已注入 zone` 守恒，不是随进程生命周期结束的临时状态。
- 本 plan 范围限定为 burial 本体持久化与恢复；ledger 只作为恢复时重建 source balance / 守恒补算的配套，不另立“WorldQiAccount 不持久化”主题。

### 反方最终裁决

候选足够高置信，且适合开单一 Skeleton Plan。它独立于 r9“散灵珠 ledger 僵尸账户”和 #1044“可放置实体重启丢失”；修复范围应聚焦 `ScatterBeadBurials` 本体持久化、启动 hydrate、dirty/shutdown flush、重启后继续散逸或补算、owner 重连触发。
