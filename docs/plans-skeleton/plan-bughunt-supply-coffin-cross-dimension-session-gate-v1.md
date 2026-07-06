# Bong · plan-bughunt-supply-coffin-cross-dimension-session-gate-v1

> **Skeleton Plan（BugHunt，第 3 轮 server-gameplay）**。一句话：物资棺 `supply_coffin` 的专属 open / lifecycle / external move 链路只信任全局 `entity_id/session_id` 与裸 XYZ 距离，没有服务端同维校验；伪造或陈旧 C2S 可在异维同坐标开主世界物资棺，已开会话跨维后也可继续搬运或占锁。

## 0. 去重与范围

- 已避开 #1048 灵木满包吞产物、#1055 ForgeStationPlace 坐标门禁，以及 #981 炼丹炉交互门禁、#1004 制作台跨维误拆、#1007 掉落物跨维拾取、#1014 玩家交易跨维换货、#1022 灵田 C2S 门禁等既有 BugHunt 主题。
- 不是 `docs/plans-skeleton/plan-bughunt-entity-interact-range-desync-v1.md` 的“同维 4.5~5.0 格假交互带”。本题是服务端权威维度授权缺口。
- 不是 `docs/plans-skeleton/plan-placed-container-session-lifecycle-gap-v1.md` 的货箱 / 死信箱 placed container 生命周期缺口。物资棺使用 `server/src/supply_coffin/{interact,lifecycle}.rs` 专属路径。
- 本 plan 不做任何实际修复，只记录 skeleton。

## 1. 实际游玩体验影响

- 玩家如果通过 bot、调试包、陈旧 UI 状态或坏客户端提交主世界活跃物资棺的 `entity_id`，即使当前人在 TSY / 其他位面，只要数值坐标靠近，server 仍可能开棺并生成 loot 会话。
- 一旦会话打开，`ExternalContainerMove` 后续只看 `session_id` 和 `opened_by`，不再二次确认距离 / 维度；异维玩家可以把主世界棺内物品搬进自己背包，破坏搜打撤资源风险。
- 正常开棺后若玩家跨维传送到相近数值坐标，物资棺 lifecycle 只按 XYZ 距离判断是否 close，不看维度；会话可能继续持锁、继续搬运，直到 timeout，其他玩家看到“有人正在翻找”。
- 普通未改客户端通常受 layer 可见性保护，不应宣称“准星稳定跨维命中棺”。但 AGENTS.md §15 要求 server 对 bot / 任意 C2S 输入最大化宽容且保持权威，不能把客户端可见性当安全边界。

## 2. 复现路径

### 路径 A：伪造或陈旧 `supply_coffin_open`

1. 主世界存在一具活跃物资棺，记录其 protocol `entity_id` 与 `SupplyCoffinRegistry.active` 中的裸坐标。
2. 玩家进入 TSY / 其他位面，并移动到相同或接近的数值坐标（4.5 格内）。
3. 通过 bot / 调试客户端向 `bong:client_request` 发送 `{"type":"supply_coffin_open","v":1,"entity_id":<主世界棺 entity_id>}`。
4. 当前 server 只用全局 `EntityManager::get_by_id` resolve 目标，再在 `handle_supply_coffin_interact` 中按 XYZ 距离放行；若 target 是 active 物资棺，会发送 `LootContainerOpen` 并创建 `ExternalContainer` 会话。

### 路径 B：已开会话后跨维

1. 玩家在主世界正常打开物资棺，获得 `session_id`，`ExternalContainer.opened_by = player`。
2. 玩家在会话未关闭前通过传送 / 断线恢复 / 调试路径进入 TSY 或其他位面，并落在与物资棺相近的数值坐标。
3. 继续发送 `external_container_move`，server 只校验 `session_id` 与 `opened_by == player`，未重验玩家与棺是否仍同维。
4. lifecycle tick 也只按 XYZ 距离做 close；跨维但同数值坐标不会触发 distance close。

## 3. 根因证据

1. `server/src/network/client_request_handler.rs:2267-2293`：`SupplyCoffinOpen` 分支只读取 `entity_id`，经全局 `EntityManager::get_by_id` 解析后发送 `SupplyCoffinOpenRequest { client, target }`；没有读取玩家 `CurrentDimension`，也没有目标 layer / dimension 过滤。
2. `server/src/supply_coffin/mod.rs:108-115`、`:135-180`：`ActiveSupplyCoffin` 和 `SupplyCoffinRegistry.active` 只记录 `grade / pos / spawned_at_wall_secs`，没有 `DimensionKind`。
3. `server/src/supply_coffin/refresh.rs:86-107`：物资棺 marker spawn 到 `layers.overworld`，但只把裸 `pos` 写入 registry；没有给后续业务状态留下可比较的 dimension 字段。
4. `server/src/supply_coffin/interact.rs:83-102`：open handler 只查 target 是否在 `registry.active`，再用 `active.pos.distance(player_pos.get())` 做 4.5 格距离门禁；无 `CurrentDimension` / `EntityLayerId` 查询。
5. `server/src/supply_coffin/interact.rs:165-183`、`:185-240`：通过距离门禁后立刻 roll loot、分配 `session_id`、创建 `ExternalContainer` 并下发 `LootContainerOpen`，副作用真实生效。
6. `server/src/network/client_request_handler.rs:13793-13812`：`handle_external_container_move` 只通过 `session_id` 找实体，并校验 `ext.opened_by == Some(player_entity)`；没有位置 / 维度二次授权。
7. `server/src/supply_coffin/lifecycle.rs:81-103`：物资棺 lifecycle 只检查 `opened_by` 玩家是否存在和 `coffin_pos.distance(player_pos.get())`，不比较维度。
8. 正例对照：`server/src/world/container_open.rs:55-88` 已查询玩家与容器的 `CurrentDimension` 并拒绝 mismatch，说明通用世界容器已经有正确模式，物资棺专属路径是漏接。

## 4. 修复计划骨架

- [ ] 给物资棺 active/runtime 状态补权威维度语义。优先在 `ActiveSupplyCoffin` 中记录 `DimensionKind::Overworld`，或给物资棺 marker 插入并读取 `CurrentDimension(DimensionKind::Overworld)`；不要只依赖 `EntityLayerId`。
- [ ] `handle_supply_coffin_interact` 查询玩家 `CurrentDimension` 并与物资棺维度比较；缺失维度按现有交互门禁策略保守处理，拒绝时给玩家明确反馈。
- [ ] `external_container_lifecycle_tick` 对物资棺 session 同步比较 `opened_by` 玩家当前维度；维度不同时发送 `LootContainerClose(reason=Distance 或新 reason)`，释放锁，且不碎棺。
- [ ] `handle_external_container_move` 在物资棺 / 外部容器 move 前二次授权：session owner、同维、仍在范围内同时满足才允许搬运；拒绝时 resync 外部容器与玩家背包。
- [ ] 复查 `SupplyCoffinRegistry.active`、`ExternalContainerRegistry.sessions` 的 session 清理顺序，避免跨维拒绝后留下孤儿 session 或占锁。

## 5. 验证计划

- [ ] server 单测：玩家 `CurrentDimension(Tsy)`、物资棺 active 维度为 Overworld、同 XYZ 4.5 格内发送 `SupplyCoffinOpenRequest`，断言拒绝、未分配 `ExternalContainer`、未 roll loot、未发送 `LootContainerOpen`。
- [ ] server 单测：Overworld 同维同距开棺仍成功，避免误伤正常物资棺玩法。
- [ ] server 单测：已打开物资棺后把玩家 `CurrentDimension` 改为 TSY 且保持同 XYZ，运行 lifecycle tick 应关闭 session / 释放 `opened_by`，而不是继续占锁。
- [ ] server 单测：已打开物资棺后跨维发送 `ExternalContainerMove`，断言物品不从棺转入玩家背包，并触发 resync。
- [ ] bot e2e：用黑盒 `bong:client_request` 发送跨维 `supply_coffin_open` 和 `external_container_move`，断言连接保持、server 不 panic、请求被拒绝且资源不变。
- [ ] 回归：普通 `container_open` / placed container 不被物资棺专属修复改坏；若抽公共 helper，覆盖货箱、死信箱、物资棺三类 source kind。

## 6. 对抗复核结论

### 候选证据

主候选认为：物资棺 open / lifecycle / move 链路没有服务端同维校验，跨维同坐标 C2S 可以绕过客户端 layer 可见性，产生真实开棺与搬运副作用。

### 反方质疑（Round 1）

- 普通客户端 `SupplyCoffinInteractIntentHandler` 从当前 `crosshairTarget` 取实体并复核 entity id，维度切换时 server 会改 `VisibleEntityLayers`；因此“正常准星跨维开棺”证据不足。
- `EntityLayerId` / Valence layer 隔离可能已经让跨维实体不可见。
- 需确认是否被既有 `entity-interact-range-desync`、`placed-container-session-lifecycle`、#1007、#1014 覆盖。

### 修正 / 反驳（Round 2 输入）

- 接受收窄：本 bug 不声称普通客户端稳定跨维准星命中，而是 server-authoritative C2S 授权缺口。
- `EntityManager::get_by_id` 是全局 protocol id 映射，`SupplyCoffinOpenRequest` 只有 `client/target`；server 不能把客户端可见性作为安全边界。
- 影响继续扩展到 move 与 lifecycle：`ExternalContainerMove` 无二次距离 / 维度授权；lifecycle 无维度 close。
- 相关既有 skeleton / PR 是邻近主题但运行态对象不同，未覆盖物资棺专属路径。

### 最终裁决

反方最终接受：即使普通客户端通常被 layer 可见性挡住，`supply_coffin` 的服务端权威链路仍只信任全局 `entity_id/session_id + XYZ 距离/owner`，缺少同维校验，导致伪造 / 陈旧请求可跨维同坐标开棺并在会话内继续搬运，属于独立 server gameplay 授权漏洞。

