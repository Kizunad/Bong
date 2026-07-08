# plan-tsy-container-disconnect-lock-leak-v1（骨架）

> **骨架（草案）**。一句话主题：TSY 容器搜刮把占用锁记在 `LootContainer.searched_by: Entity` 上，但断线清理只 `insert(Despawned)` 玩家实体、完全不释放该锁；结果是**一个玩家中途掉线就能把当前 uptime 内的容器锁成幽灵占用**。更糟的是，joined-client 的 `container_state` 会把这个 stale entity 投影成 `searched_by_player_id = null`，客户端把容器判成“可交互”，但 server 仍在 `start_search_container` 里静默拒绝 `OccupiedByOther`，形成**前端看着空闲、按 G 没反应**的假象。

> **退化说明（无 subagent）**：当前会话无可用 subagent / delegate 能力，本 skeleton 的“两轮反方裁决”由本地代码对读完成，并在文末逐条记录反方论点与驳回理由。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | TSY 容器 `searched_by` 断线后不释放，造成幽灵占锁 + 客户端假空闲 | fix_pr | ⬜ |

## P0 — TSY 容器 `searched_by` 断线后不释放

- **可达链路 / 复现路径**
  - 1. 进入任意已注册 TSY family，找一个未搜空的 `LootContainer`（干尸/骨架/储物袋/石匣/法阵核心都可）。
  - 2. 玩家 A 开始搜刮：`server/src/world/tsy_container_search.rs:350-418` 在通过距离/战斗/钥匙校验后，把 `container.searched_by = Some(req.player)`，并给玩家挂 `SearchProgress + IsSearching`。
  - 3. 在进度完成前直接断线/杀客户端进程。`server/src/player/mod.rs:318-456` 的 `despawn_disconnected_clients` 会持久化玩家、清 coffin 占用，然后只对该实体 `insert(Despawned)`；全函数**没有**碰 `SearchProgress` / `IsSearching` / `LootContainer.searched_by`。
  - 4. 之后其他玩家，或同一玩家重连后以新 entity 再次回到该容器旁，server 仍会在 `start_search_container` 的 `if let Some(other) = container.searched_by` 分支（`tsy_container_search.rs:350-358`）命中 `SearchRejectionReason::OccupiedByOther`，因为锁里存的是旧 entity bits，不会因账号相同自动认领。
  - 5. 反馈层又是断的：`server/src/network/tsy_container_search_emit.rs:54-78` 只把 `StartSearchResult::Started` 下发给客户端，`Rejected` 全部 `continue`，所以按 G 后**没有任何 rejection payload**，表现为“按了没反应”。
  - 6. 对新加入或重连的客户端，`container_state_payloads` 把 `container.searched_by` 通过当前在线玩家 map 投影成 `searched_by_player_id`（`tsy_container_search_emit.rs:163-179`）；stale entity 不在在线玩家表里时被序列化成 `None`。客户端 `client/src/main/java/com/bong/client/tsy/TsyContainerView.java:14-16` 因 `searchedByPlayerId == null` 判定容器 `interactable()`，于是 UI 看起来是空闲的，但 server 仍静默拒绝。

- **为什么这是 bug，不是设计**
  - `docs/finished_plans/plan-tsy-container-v1.md:765` 明确把“`searched_by` 在玩家 disconnect 时不释放”列为已知风险，并指定缓解就是“玩家退出 session 的 cleanup hook 里 remove `SearchProgress` + 清 `container.searched_by`”。现实现没有这条 cleanup hook，属于设计已写、实现漏接，不是有意保留的锁语义。
  - 搜刮锁 owner 使用的是运行时 `Entity`，不是 `canonical_player_id`；断线后同一账号重连也会生成新 entity，因此该锁天然需要 disconnect cleanup 才能成立。现在少了清理，锁就变成无主僵尸状态。

- **根因链路**
  - `LootContainer.searched_by` 的释放点只有三类：搜刮中断/取消（`tsy_container_search.rs:520-530, 645-658`）、正常完成（`:593-606`）。
  - 断线路径完全绕开这些释放点：`RemovedComponents<Client>` → `player::despawn_disconnected_clients` → persist + `insert(Despawned)`，无任何桥接到 TSY 容器搜索状态机。
  - `container_state` 的展示层又把“锁 owner 是否仍在线”错误地当成“容器是否被占用”的代理：`searched_by` 仍有值，但 `searched_by_player_id` 可能已经序列化成空；client 只看后者，server 只看前者，形成状态分叉。
  - `StartSearchResult::Rejected` 没有 S2C emit，进一步把 server-side 占锁错误降级成无反馈静默失败。

- **影响面**
  - 任意 TSY 容器都可被一次断线锁死到本次 server uptime 结束，尤其是高价值的 `StoneCasket` / `RelicCore`。
  - 对已经在场的其他玩家：容器可能永远保持“有人在搜”的旧状态，因为断线不会触发 `Changed<LootContainer>` 去广播解锁。
  - 对重连后/后加入的玩家：容器可能显示为空闲、可交互，但按 G 没反应，因为 client 认为可搜、server 实际回 `OccupiedByOther` 且无 rejection payload。
  - 对搜打撤体验：单次网络抖动/客户端崩溃即可把秘境关键容器变成“既不能搜、又不给解释”的坏交互，破坏 TSY 高压搜刮的核心节奏。

- **建议修复范围 / 模块**
  - `server/src/player/mod.rs`
    - 在 `despawn_disconnected_clients` 里显式桥接 TSY 搜刮 cleanup：移除 `SearchProgress` / `IsSearching`，并按 `progress.container` 清 `LootContainer.searched_by`。
  - `server/src/world/tsy_container_search.rs`
    - 补一个可复用的 `release_search_lock(player_entity)` / `abort_search_for_player(player_entity, reason)` helper，避免取消/中断/断线三处复制逻辑继续漂移。
  - `server/src/network/tsy_container_search_emit.rs`
    - 最低限度应把 `Rejected` 也转成 S2C（哪怕只是 reason 字段），否则后续再出现占锁/缺钥匙/超距等 reject 仍是“按了没反应”。
  - `client/src/main/java/com/bong/client/tsy/`
    - 若短期不补 rejection payload，至少不能把 `searchedByPlayerId == null` 等同于“未占用”；否则断线 owner 的 stale lock 仍会在 UI 侧投成假空闲。

- **验收抓手**
  - 单测 1：玩家 A 开搜后触发 `RemovedComponents<Client>`，断言 A 的 `SearchProgress` / `IsSearching` 被清，容器 `searched_by == None`。
  - 单测 2：A 断线后，玩家 B 或 A 的新 entity 重新发 `StartSearchRequest`，应能成功收到 `StartSearchResult::Started`，不再命中 `OccupiedByOther`。
  - 单测 3：`container_state_payloads` 对 stale `searched_by` 不应再出现“server 仍锁着、payload 却发 null”分叉；要么锁已被清，要么 payload/协议显式表达“幽灵占用/断线占用”并同步给 client。
  - 若补 S2C reject：补 `SearchRejected` / 等价 payload 的 schema sample、server emit、client handler 与 HUD/toast 测试，覆盖 `OccupiedByOther` / `MissingKey` / `OutOfRange` / `InCombat` / `DailyLimitExceeded`。

## 反方裁决（退化：本地两轮）

### Round 1

- **反方论点**：断线后玩家实体被 `Despawned`，也许后续 Bevy/Valence 清理会顺手让 `LootContainer.searched_by` 失效，所以这只是短暂悬空引用，不会真阻塞交互。
- **驳回理由**：
  - `searched_by` 是 `Option<Entity>` 裸值，不是 `EntityRef` / `Query` 联动句柄；player entity 被标 `Despawned` 不会自动回写容器组件。
  - `start_search_container` 只做 `if let Some(other) = container.searched_by { if other != req.player { reject } }`，完全不检查 `other` 是否仍带 `Client` / 是否已 `Despawned` / 是否还能从 query 取回。
  - 全仓对 `SearchProgress` / `searched_by` 的释放只找到取消、中断、完成三条路径，未见任何 `RemovedComponents<Client>` 或 `Despawned` cleanup。

### Round 2

- **反方论点**：就算锁残留，客户端至少会继续显示“有人在搜”，玩家知道是占用态；这更像 UX 小缺口，不是 runtime 真 bug。
- **驳回理由**：
  - 现网存在两种坏表现，而且都不是“正常占用”：
    - 已在场玩家：看见旧的 occupied 状态永久不变，因为断线不触发 `Changed<LootContainer>` 去广播解锁/重置。
    - 新加入/重连玩家：`searched_by` 仍在，但 `searched_by_player_id` 通过在线玩家映射后会变 `null`，client `interactable()` 误判为空闲。
  - `StartSearchResult::Rejected` 不下发，意味着后一种玩家按 G 后几乎只会得到静默失败。这个结果已经从“UI 小瑕疵”升级为 server/client 状态机分叉。

## 去重结论

- 已避开用户列出的既有题：灵田多区结算串回默认区、空 `ZoneGraph`、surface stash 标签缺口、orphan `pack_<id>`、伪灵脉 runtime zone 重启丢失。
- 与近期 skeleton / finished plan 未重复：全仓仅在 `plan-tsy-container-v1` 风险表里写过“disconnect 时应释放 `searched_by`”，但未见独立 bug skeleton / fix 题落地；当前缺陷仍在生产代码中真实存在。
