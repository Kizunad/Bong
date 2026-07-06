# Bong · plan-placed-container-session-lifecycle-gap-v1

> **骨架（bughunt，report-only）**。一句话：`trade_crate` / `herb_crate_placed` / `dead_drop_box` 这类 placed world container 复用了 `ExternalContainer` session，但**只在显式关闭或容器被打碎时**释放 `opened_by`；超距、跨维、断线都没有生命周期清理，而 `ExternalContainerMove` 又只校验 session owner、不重验距离/维度，导致玩家可在离开原地甚至换区后继续远程搬运，同时把容器长期占锁。

## 结论

- **类型**：world runtime / container / interaction state machine
- **严重度**：major
- **是否与已排除题重复**：**否**
- **去重说明**：已排除题是“TSY 容器断线幽灵占锁”，对应 `LootContainer.searched_by` / `SearchProgress` 那套 TSY 搜刮状态机；本题是 **placed external container**（`ExternalContainer.opened_by`）的生命周期缺口，且不止断线，还包含**超距/跨维后仍可远程搬运**，运行态链路与影响面不同。

## 复现路径

1. 玩家 A 在 Overworld 放置 `trade_crate` / `herb_crate_placed` / `dead_drop_box` 任一世界容器。
2. 站在 4 格内打开容器。`handle_container_open` 会通过距离/维度校验后把 `ext.opened_by = Some(player)`（`server/src/world/container_open.rs:85-118`）。
3. 不点关闭，直接：
   - 走到 6 格外甚至更远；或
   - 传送到另一个 zone / 维度；或
   - 断线。
4. 观察：
   - 同一玩家仍可继续发 `ExternalContainerMove`，把物品从该容器拖到自己背包，或反向塞回容器；
   - 其他玩家再次尝试打开时会被 `opened_by` 挡住，收到“有人正在翻找”；
   - 若 owner 已断线，则这个占锁会停留在旧实体 id 上，其他人无法正常接管，直到容器被打碎或 owner 那边有机会走显式 close。

## 根因链路

1. `ExternalContainer` 框架自己的文件头把生命周期写成“interact → 创建 session → 玩家搜刮（双向拖拽）→ 超时/关闭/走远 → 销毁”（`server/src/inventory/external_container.rs:1-4`）。
2. 但 placed container 注册只做了 `ExternalContainerRegistry` 初始化和 `handle_container_block_break`；**没有任何 lifecycle tick** 接上超距/断线清理（`server/src/world/container_block.rs:111-117`）。
3. placed container 创建时 `timeout_wall_secs` 被写死成 `0`（`server/src/world/container_block.rs:155-170`），本身也没有后续系统去消费这个字段。
4. 唯一真正实现 lifecycle 的是 `supply_coffin::external_container_lifecycle_tick`，而它在入口处显式 `if !is_supply_coffin_lifecycle_managed(ext) { continue; }`，只处理 `ExternalContainerKind::SupplyCoffin`（`server/src/supply_coffin/lifecycle.rs:43-58`）；后面的 timeout / distance / disconnect 清理都不会覆盖 `StorageCrate` / `DeadDrop`（`:61-118`）。
5. open 侧只在**打开瞬间**校验玩家与容器同维且在 4.5 格内，然后把 `opened_by` 设为当前玩家（`server/src/world/container_open.rs:85-118`）。
6. move 侧 `handle_external_container_move` 取 session 后只校验 `ext.opened_by == Some(player_entity)`；**完全不重验**玩家当前位置、当前维度、容器位置、owner 是否仍在交互半径内（`server/src/network/client_request_handler.rs:13178-13545`）。
7. close 侧 `handle_external_container_close` 只响应玩家主动发 `ExternalContainerClose`，才会把 `ext.opened_by = None`（`server/src/network/client_request_handler.rs:13568-13599`）。因此状态机缺了“超距退出”“跨维退出”“断线退出”三条对称边。

## 这个 bug 对实际游玩体验的影响

- 玩家可以把货箱/死信箱当成**跨区远程仓库**：先在近处打开，随后跑远、换区甚至换维，仍能继续拖拽物品，直接绕过原本的接近交互约束。
- 其他玩家会看到容器被长期占用，尤其死信箱这类高对抗场景里，会出现“人已经不在场，箱子却一直显示有人在翻”的假占锁。
- 若 owner 在打开后断线，session 会挂在旧实体 id 上，形成需要人工破坏容器才能恢复的幽灵锁，破坏交易/搬运/埋箱体验。

## 修复建议

1. 给 placed `ExternalContainerKind::{StorageCrate, DeadDrop}` 接上与 supply coffin 对齐的 lifecycle system，至少覆盖：
   - owner 超距；
   - owner 维度变化；
   - owner 断线 / `RemovedComponents<Client>`；
   - 容器实体已不在 registry / position 缺失的防御性清理。
2. 在 `handle_external_container_move` 内补二次校验：session owner、容器位置、玩家位置、当前维度必须仍满足交互不变量；不满足时先 close/release，再拒绝 move。
3. 统一 close reason：对 placed container 也按现有协议发 `distance` / 必要时新增更明确的断线或 invalid-session 关闭语义；client 已能消费 `timeout` / `distance` / `player_closed` / `container_destroyed`（`client/src/main/java/com/bong/client/network/LootContainerHandler.java:77-95`）。
4. 明确 `timeout_wall_secs = 0` 的语义：若 placed container 确实无倒计时，也仍应有 distance/disconnect lifecycle；不要把“无超时”误实现成“无退出边”。

## 验收抓手

1. 打开 `trade_crate` 后走到 >6 格，再发 `ExternalContainerMove`，server 应拒绝并释放 `opened_by`；第二名玩家可立即重新打开。
2. 打开 `dead_drop_box` 后跨维传送，再发 move，应收到 close / reject，且原容器不再保留 owner 锁。
3. owner 打开 placed container 后断线，下一 tick 或 disconnect cleanup 后 `opened_by` 必须归零；其他玩家无需打碎容器即可接管。
4. pin 测试区分 supply coffin 与 placed container：
   - supply coffin 维持原 timeout/distance/despawn 语义；
   - placed container 新增 distance/disconnect/跨维 close，但**不会**误触发超时碎箱。
5. 回归：容器被打碎时仍按现有逻辑发送 `container_destroyed` 并掉落内容物，不因新 lifecycle 影响破坏路径。

## 反方裁决摘要（退化执行）

> 当前会话未提供可用 subagent / delegate 通道；按要求做**两轮主代理手工反方裁决**，并在 PR 如实记录退化处理。

### Round 1

- **反方论点**：这可能只是“玩家没关界面”的正常占用，不算 bug；UI 关掉时客户端会发 `ExternalContainerClose`。
- **驳回理由**：问题不在“ESC 不发 close”，而在**服务端状态机没有超距/跨维/断线退出边**。`ExternalContainer` 框架注释明确把“走远”写成生命周期一环（`external_container.rs:1-4`），client 也接受 `distance` close reason（`LootContainerHandler.java:92-95`），说明 close-on-distance 不是想象中的需求外行为。只靠显式 close，无法覆盖断线/传送/异常掉线。

### Round 2

- **反方论点**：也许 lifecycle 只为 supply coffin 设计；placed container 本来就允许长时打开，所以不该复用 distance/timeout 语义。
- **驳回理由**：即便不需要 timeout，**也不能允许远程搬运**。open 时已经把交互约束定义为“同维 + 4.5 格内”（`container_open.rs:85-118`），move 时却完全不重验（`client_request_handler.rs:13178-13545`），这不是“放宽设计”，而是**open-time invariant 在 session 进入后失效**。同时 supply coffin lifecycle 还专门用 `is_supply_coffin_lifecycle_managed` 把 placed container 排除在外（`supply_coffin/lifecycle.rs:55-58`），进一步证明这里是接线缺口而不是已实现的特化设计。

## PR 文案抓手

- 标题建议：`docs(skeleton): 记录 placed container session 生命周期缺口`
- 摘要一句话：placed world container 的 `ExternalContainer` session 缺少超距/跨维/断线退出边，导致远程搬运和长期占锁。
