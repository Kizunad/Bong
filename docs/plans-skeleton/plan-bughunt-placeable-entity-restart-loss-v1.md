# Bong · plan-bughunt-placeable-entity-restart-loss-v1

> **骨架（BugHunt H9 persistence，report-only）**。一句话：`workbench_item` / `trade_crate` / `herb_crate_placed` / `dead_drop_box` 放置后只生成运行态纯 entity 与 ECS 组件，没有持久化记录或启动恢复；服务器进程重启后工作台、货箱、灵草箱、死信箱本体消失，容器内容也随内存丢失，但玩家背包里的原物品已经被扣掉。

## Bug 摘要

可放置世界物件当前共享 `PlaceableBlockKind` 纯 entity 底盘：

- 工作台：`PlaceableBlockKind::Workbench` → `handle_workbench_place` → marker entity + `WorkbenchBlock`。
- 世界容器：`StorageCrate` / `DeadDrop` → `handle_container_block_place` → marker entity + `ContainerBlock` + `ExternalContainer`。

这些状态都只存在于 Bevy ECS / resource 内存中。`persistence` 启动 hydrate 路径没有任何 placed workbench / placed container / external container 内容恢复；服务器重启后，新进程只会重新初始化空的 `ExternalContainerRegistry` / `FurnitureRegistry` 等运行态资源，不会重建玩家已经放下的纯 entity。

这不是“断线后锁不释放”类 session bug，而是**服务器进程生命周期**上的本体与内容丢失。

## 实际游玩体验影响

- 玩家做出并放下 `workbench_item` 后，服务器重启一次，工作台从世界里消失；靠近工作台才能开的制作配方重新变成不可用，玩家需要重新制作并放置工作台。
- 玩家把材料、战利品或交易物放进 `trade_crate` / `herb_crate_placed` / `dead_drop_box` 后，服务器重启会让容器 entity 和 `ext.container.items` 一起丢失；箱内物品没有掉落、没有返还、没有恢复入口。
- `dead_drop_box` 是异步投递/埋箱玩法，重启丢失会把“买卖双方不见面”的核心体验变成不可信的吞物风险。
- 背包消耗已经在放置请求成功时发生并发了 inventory snapshot；玩家视角是“我正常放下了物件，维护/崩服/重启后资产凭空没了”。

## 证据定位

1. 放置请求先扣玩家背包物品，再进入 placeable 派发：`server/src/world/block_place.rs:220-226` 调 `consume_item_instance_once`，`:244-255` 调 `place_placeable`，成功后 `:299-307` 只给玩家发 inventory snapshot。
2. `place_placeable` 只按 kind 生成运行态 entity：`server/src/world/block_place.rs:463-500` 分别调用 `handle_workbench_place` / `handle_container_block_place`，没有持久层写入。
3. 工作台是纯 ECS component：`server/src/craft/workbench.rs:34-43` 定义 `WorkbenchBlock`；`:89-112` 通过 `spawn_visual_marker` 后 `insert(WorkbenchBlock)`，没有 save/load 字段或启动恢复。
4. 工作台配方门禁依赖运行态查询：`server/src/network/craft_emit.rs:186-198` 只查附近 `With<WorkbenchBlock>`；重启后实体不存在则 `has_nearby_workbench=false`。
5. 世界容器本体与内容也在运行态：`server/src/world/container_block.rs:119-145` spawn marker 并插入 `ContainerBlock` / `CurrentDimension` / `ExternalContainer`；`:155-171` 构造的 `ExternalContainer.container.items` 初始为内存 `vec![]`。
6. `ExternalContainer` 与 session registry 都是 ECS/resource：`server/src/inventory/external_container.rs:36-58` 定义 component 与 `ExternalContainerRegistry { next_session_id, sessions }`，没有持久化字段。
7. 打开和移动都依赖运行态 registry/entity：`server/src/world/container_open.rs:51-84` 要求 `registry.sessions[session_id] == target`；`server/src/network/client_request_handler.rs:13788-13812` move 时先从 `ext_container_registry.sessions` 找 entity，再取 `ExternalContainer`。
8. 容器内容更新直接从 `ext.container.items` 组 payload：`server/src/network/client_request_handler.rs:14128-14145`，说明箱内物品权威状态就在该运行态 component 里。
9. 启动持久化 bootstrap 仅 hydrate void cooldown、伪灵脉、zone runtime/overlay/influence：`server/src/persistence/mod.rs:737-789`，没有 workbench/container_block/external_container 的 hydrate 分支。
10. 既有 finished plan 只完成放置/交互/破坏闭环，且定案为纯 entity：`docs/finished_plans/plan-workbench-place-runtime-v1.md:48-60`、`:181`；容器 plan 完成 entity 容器放置、打开、move/close、破坏掉落：`docs/finished_plans/plan-placeable-container-blocks-v1.md:59-82`、`:205-208`，未声明重启恢复。

## 触发路径

1. 玩家获得 `workbench_item`、`trade_crate`、`herb_crate_placed` 或 `dead_drop_box`。
2. 通过正常放置链路在世界中放下该物件；server 扣掉对应 item instance 并发背包 snapshot。
3. 若是容器，打开后把任意物品拖入箱内，server 更新 `ExternalContainer.container.items`。
4. 服务器进程正常重启或崩溃后重启。
5. 观察：
   - 工作台 marker / `WorkbenchBlock` 不存在，附近工作台门禁失败。
   - 货箱/灵草箱/死信箱 marker / `ContainerBlock` / `ExternalContainer` / registry session 不存在。
   - 箱内物品不在玩家背包、不在地面掉落、不在任何恢复路径。

## 反方审查记录

### Round 1：是否只是 runtime-only 设计

- **反方论点**：纯 entity + bbmodel 可能本来就是 runtime-only；Valence chunk/block 层或启动扫描也许会恢复 marker；即使 marker 没恢复也可能只是视觉问题。
- **核查结果**：未找到“不承诺重启保存”的设计依据。finished plan 定案的是“纯 entity + bbmodel”，不是“临时 entity”。`persistence` bootstrap 没有 placed entity hydrate；放置路径只 spawn ECS component；工作台 gate 查 `WorkbenchBlock`，容器内容在 `ext.container.items`，所以不是纯视觉。
- **Round 1 verdict**：反方未能驳回候选。

### Round 2：是否与已有 plan / PR 重复

- **反方论点**：可能重复 `plan-placed-container-session-lifecycle-gap-v1.md` 或已完成的 workbench/container finished plan；也可能被 #972/#985/#991/#996/#1001/#1019/#1024/#1035 等 persistence PR 覆盖。
- **核查结果**：`plan-placed-container-session-lifecycle-gap-v1.md` 处理的是已打开 `ExternalContainer.opened_by` 的超距/跨维/断线不释放，导致远程搬运和占锁；本题处理服务器进程重启后 entity 与 container contents 不恢复。finished plan 是功能落地，不含重启恢复。开放 persistence PR 标题分别覆盖 dormant/mineral/surface stash/botany/alchemy furnace/spiritwood/spirit eye/zone influence，不覆盖 placeable entity 族。
- **Round 2 verdict**：不重复；工作台与三类容器共享 `PlaceableBlockKind` / 纯 entity 根因，适合作为一个 plan，内部按阶段拆分。

## Skeleton Fix Plan

### P0：定义 placed entity 持久化权威模型

- 新增 SQLite 权威表或等价存储，记录：
  - `placed_id`：稳定主键，不使用 Bevy `Entity`。
  - `kind`：`workbench` / `storage_crate` / `herb_storage_crate` / `dead_drop`。
  - `dimension`、`pos`、`placed_by_player_id`、`placed_at_tick`。
  - 容器专属 payload：`ContainerState` items、rows/cols、dead drop ward owner/active 状态。
- 明确崩溃一致性：放置扣物与持久化写入必须同一语义边界内成功；若持久化失败，不得留下“背包已扣但世界物件没权威记录”的状态。
- 不持久化 runtime `session_id` 和 Bevy `Entity`；启动恢复时重新分配 session，并重建 registry 映射。

### P1：放置 / 破坏 / 内容移动写入权威存储

- 放置成功后立即 upsert placed record。
- 工作台破坏后删除 record，并维持现有返还物品逻辑。
- 容器破坏后删除 record，并保持现有内容掉落 / dead drop 化灰与 `ContainerDestroyed` close 语义。
- `ExternalContainerMove` 成功后同步持久化 container items；拖入、拖出、交换、失败回滚都要覆盖。

### P2：启动 hydrate 与 runtime 重建

- 在 world/layer 完成后启动 hydrate placed records：
  - 重建 Workbench marker + `WorkbenchBlock` + `CurrentDimension`。
  - 重建 Container marker + `ContainerBlock` + `ExternalContainer` + `ExternalContainerRegistry.sessions`。
  - 对 position collision / chunk unloaded / duplicate pos 做确定性处理并记录 warning，不 silent drop。
- 恢复时 `placed_by_player_id` 不能直接等同旧 Bevy entity；需要按用户名/UUID/持久身份解析，无法解析时保留 owner 字段但禁用依赖 runtime entity 的攻击归属，或在首次玩家上线后 rebind。

### P3：边界与迁移

- 旧库无 placed records 时不做补偿性猜测；只能保证修复后新放置物不再丢失。
- 对已有纯 entity 内存对象升级时可在启动后首次 flush 将其写入新表，避免热更新窗口。
- 与 `plan-placed-container-session-lifecycle-gap-v1` 的 session close/move 二次校验保持兼容：重启恢复的是本体和内容，不代表打开 session 可跨重启延续。

## 验收测试计划

1. **工作台重启恢复**：放置 `workbench_item` → 断言背包扣 1、DB 有 placed record → 构造新 App/重跑 hydrate → 查询到 `WorkbenchBlock`；靠近玩家的 `has_nearby_workbench` 为 true。
2. **工作台破坏清理**：恢复后的工作台被破坏 → 返还/掉落 `workbench_item`，DB record 删除；再次重启不出现幽灵工作台。
3. **货箱内容恢复**：放置 `trade_crate` → 拖入两个不同 item instance → 成功 move 后 DB payload 更新 → 重启 hydrate → 打开容器，`LootContainerOpenV1.placed_items` 与重启前一致。
4. **死信箱语义恢复**：`dead_drop_box` 恢复后仍带 `DeadDropWard` owner/active 状态；非 owner 破坏仍走化灰 + 毒气，owner 破坏仍走既有普通分支。
5. **session id 不持久化 pin**：重启前后 `ExternalContainer.session_id` 可变化，但 `ExternalContainerRegistry.sessions` 必须指向恢复后的新 entity，open/move/close 正常。
6. **崩溃一致性**：模拟 placed record 写入失败时，放置请求不得扣物；模拟 move 持久化失败时，容器内容和玩家背包不进入一边成功一边失败的分裂状态。
7. **去重回归**：`plan-placed-container-session-lifecycle-gap-v1` 的超距/跨维/断线 close 语义仍按其 plan 验收；本 plan 不引入远程搬运回归。

## 风险

- **持久身份风险**：当前 `placed_by` 是 Bevy `Entity`，不能跨进程保存；必须迁移到玩家持久身份，否则 dead drop owner、combat attribution、权限判断会在重启后漂移。
- **写入频率风险**：容器 move 每次都可能改变 `ContainerState`；需要选择同步写库、批量 dirty flush，或 WAL/事务策略，不能为了性能牺牲掉物安全。
- **崩溃窗口风险**：放置链路现在先扣物再 spawn；新增持久化时必须重新审计失败顺序，否则会把当前 bug 缩小成“DB 写失败吞物”。
- **重复位置风险**：旧运行态没有 stable placed id；hydrate 时遇到同维同坐标多 record 必须 deterministic 决策，不能生成重叠 marker 或重复容器 session。
- **跨 plan 风险**：本 plan 修复重启恢复，不替代 placed-container session lifecycle plan；两者都改 `ExternalContainer` 周边时要避免 close reason、registry、move 校验互相覆盖。
