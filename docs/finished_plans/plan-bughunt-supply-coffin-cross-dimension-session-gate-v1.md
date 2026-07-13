# Bong · plan-bughunt-supply-coffin-cross-dimension-session-gate-v1

> **Active BugFix Plan（2026-07-13 升格）**。来源：`docs/plans-skeleton/plan-bughunt-supply-coffin-cross-dimension-session-gate-v1.md`。待证伪假设：物资棺 `supply_coffin` 的专属 open / lifecycle / external move 链路只信任全局 `entity_id/session_id` 与裸 XYZ 距离，伪造或陈旧 C2S 因而可能跨维开棺、搬运或持续占锁。

## 阶段总览

| 阶段 | 状态 | 可核验交付物 |
|---|---|---|
| P0 第一性原理证真 / 证伪 | ⬜ | 可达调用链、已有防护盘点、修复前契约复现或非 bug 证据 |
| P1 最小权限修复 | ⬜ | open、move、lifecycle 的服务端同维 / 距离授权与会话清理 |
| P2 饱和回归 | ⬜ | server targeted tests 覆盖成功、边界、拒绝与状态转换 |
| P3 闭环验收 | ⬜ | validator、server 完整门禁、最新主线复验、Finish Evidence 与归档 |

## 0. 来源、接入面与范围

- 已避开 #1048 灵木满包吞产物、#1055 ForgeStationPlace 坐标门禁，以及 #981 炼丹炉交互门禁、#1004 制作台跨维误拆、#1007 掉落物跨维拾取、#1014 玩家交易跨维换货、#1022 灵田 C2S 门禁等既有 BugHunt 主题。
- 不是 `docs/plans-skeleton/plan-bughunt-entity-interact-range-desync-v1.md` 的“同维 4.5~5.0 格假交互带”。本题是服务端权威维度授权缺口。
- 不是 `docs/plans-skeleton/plan-placed-container-session-lifecycle-gap-v1.md` 的货箱 / 死信箱 placed container 生命周期缺口。物资棺使用 `server/src/supply_coffin/{interact,lifecycle}.rs` 专属路径。
- **进料**：`SupplyCoffinOpenRequest`、`ExternalContainerMove`、玩家位置 / `CurrentDimension`、`SupplyCoffinRegistry.active` 与 external-container session。
- **出料**：授权成功时的 `LootContainerOpen` / 物品移动；授权失效时的拒绝反馈、容器 / 背包 resync、`LootContainerClose` 与锁释放。
- **共享类型**：优先复用 `CurrentDimension`、`DimensionKind`、`ExternalContainer` 与现有 close / resync 协议，不新造平行维度或容器协议。
- **跨仓库契约**：预期为纯 server 授权修复；不得改变既有 C2S / S2C schema，若调查发现协议必须变化则标记 BLOCKED，不擅自扩成跨栈功能。
- **worldview 锚点**：`worldview.md §十六 L1377` 明确坍缩渊为独立位面，裸 XYZ 不能替代维度身份；`worldview.md §十五 L1368` 要求开箱 / 资源动作成本真实成立。
- **qi_physics**：本题不移动真元 / 灵气，不新增衰减常数或 ledger 路径。
- **范围内**：物资棺 open、物资棺来源的 external move、该 session 的 lifecycle / cleanup，以及直接支撑这些行为的 server 测试。
- **范围外**：客户端准星行为、通用 placed container 重构、新协议 / 新 UI / A/V、其它容器类型的无关生命周期、worldview 与依赖版本修改。

## 1. 实际游玩体验影响

- 玩家如果通过 bot、调试包、陈旧 UI 状态或坏客户端提交主世界活跃物资棺的 `entity_id`，即使当前人在 TSY / 其他位面，只要数值坐标靠近，server 仍可能开棺并生成 loot 会话。
- 一旦会话打开，`ExternalContainerMove` 后续只看 `session_id` 和 `opened_by`，不再二次确认距离 / 维度；异维玩家可以把主世界棺内物品搬进自己背包，破坏搜打撤资源风险。
- 正常开棺后若玩家跨维传送到相近数值坐标，物资棺 lifecycle 只按 XYZ 距离判断是否 close，不看维度；会话可能继续持锁、继续搬运，直到 timeout，其他玩家看到“有人正在翻找”。
- 普通未改客户端通常受 layer 可见性保护，不应宣称“准星稳定跨维命中棺”。但 AGENTS.md §15 要求 server 对 bot / 任意 C2S 输入最大化宽容且保持权威，不能把客户端可见性当安全边界。

## 2. 待证伪复现路径

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

## 3. Skeleton 根因候选（P0 必须逐项复核）

1. `server/src/network/client_request_handler.rs:2267-2293`：`SupplyCoffinOpen` 分支只读取 `entity_id`，经全局 `EntityManager::get_by_id` 解析后发送 `SupplyCoffinOpenRequest { client, target }`；没有读取玩家 `CurrentDimension`，也没有目标 layer / dimension 过滤。
2. `server/src/supply_coffin/mod.rs:108-115`、`:135-180`：`ActiveSupplyCoffin` 和 `SupplyCoffinRegistry.active` 只记录 `grade / pos / spawned_at_wall_secs`，没有 `DimensionKind`。
3. `server/src/supply_coffin/refresh.rs:86-107`：物资棺 marker spawn 到 `layers.overworld`，但只把裸 `pos` 写入 registry；没有给后续业务状态留下可比较的 dimension 字段。
4. `server/src/supply_coffin/interact.rs:83-102`：open handler 只查 target 是否在 `registry.active`，再用 `active.pos.distance(player_pos.get())` 做 4.5 格距离门禁；无 `CurrentDimension` / `EntityLayerId` 查询。
5. `server/src/supply_coffin/interact.rs:165-183`、`:185-240`：通过距离门禁后立刻 roll loot、分配 `session_id`、创建 `ExternalContainer` 并下发 `LootContainerOpen`，副作用真实生效。
6. `server/src/network/client_request_handler.rs:13793-13812`：`handle_external_container_move` 只通过 `session_id` 找实体，并校验 `ext.opened_by == Some(player_entity)`；没有位置 / 维度二次授权。
7. `server/src/supply_coffin/lifecycle.rs:81-103`：物资棺 lifecycle 只检查 `opened_by` 玩家是否存在和 `coffin_pos.distance(player_pos.get())`，不比较维度。
8. 正例对照：`server/src/world/container_open.rs:55-88` 已查询玩家与容器的 `CurrentDimension` 并拒绝 mismatch，说明通用世界容器已经有正确模式，物资棺专属路径是漏接。

## 4. 收口决议与实施阶段

### P0 第一性原理证真 / 证伪

- [ ] 从 `bong:client_request` 到 open / move consumer 逐层确认正常玩家、bot、陈旧 UI 请求是否可达，检查 entity 可见层、`CurrentDimension`、owner、距离、session source 与 cleanup 的所有现有防护。
- [ ] 真 bug 时先加入修复前可失败的最小契约测试，证明跨维同 XYZ 会造成可观察副作用；非 bug 时记录玩家路径、现有防护、`file:line` 与复现结果，不造空修复。
- [ ] 只接受 server 权威状态作为授权依据；客户端可见性和 `EntityLayerId` 不能单独充当 gameplay 维度身份。

### P1 最小权限修复

- [ ] 物资棺 active/runtime 状态必须暴露明确的 `DimensionKind`；当前生成路径若经验证始终位于主世界，则记录 `DimensionKind::Overworld`，仍不得由裸 XYZ 推断。
- [ ] open 授权同时要求目标仍 active、玩家维度存在且与棺一致、距离不超过既有阈值；维度缺失 / mismatch 均保守拒绝，且拒绝前不得 roll loot、分配 session 或发送 open payload。
- [ ] 物资棺 session 必须保存足够的 source 身份，使 move 与 lifecycle 能在不信任客户端的前提下重验 owner、source 仍 active、同维和距离；不得把规则无差别施加给未证明同契约的其它 external container。
- [ ] move 授权失败时物品保持不变，并沿现有协议 resync 外部容器与玩家背包；lifecycle 授权失效时关闭 session、释放 `opened_by` / registry 映射且不碎棺。
- [ ] 复用现有 close reason；只有现有协议无法准确表达且可在纯 server 内完成时才新增内部原因，不扩写 wire schema。

### P2 饱和回归

- [ ] 覆盖 open、move、lifecycle 三个入口的 happy path、维度 mismatch、维度缺失、超距、owner mismatch、source 消失、重复 / 陈旧 session 与清理幂等状态转换。
- [ ] 若抽取公共 helper，测试其正反边界并证明普通 world container / placed container 契约不变；否则保持修复局部，不做顺手重构。

### P3 闭环验收

- [ ] 当前干净 HEAD 经全新无上下文 validator PASS。
- [ ] `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` 全绿。
- [ ] fetch 最新 `origin/main`，按 merge-base 分类同步；任何 HEAD 变化重跑相应门禁和全新 validator。
- [ ] 全阶段标记 `✅ 2026-07-13`，填写唯一 `## Finish Evidence`，受控归档后对最终 HEAD 再获 validator PASS。

## 5. 可执行测试矩阵

| 入口 / 状态 | 输入 | 必须观察到 | 必须不发生 |
|---|---|---|---|
| open happy path | Overworld、同维、阈值内、active coffin | 创建唯一 session 并发送 `LootContainerOpen` | 误拒绝或重复 session |
| open dimension mismatch | 玩家 TSY、棺 Overworld、同 XYZ | 明确拒绝 | roll loot、分配 session、发送 open |
| open missing dimension | 玩家无 `CurrentDimension` | 保守拒绝 | panic 或隐式按 Overworld 放行 |
| open boundary | 阈值内 / 正好阈值 / 阈值外 | 契约边界固定 | off-by-one 放行 |
| move happy path | owner、同维、范围内、source active | 依既有规则移动并 resync | 数量丢失 / 重复 |
| move stale authority | 跨维、超距、非 owner、source 消失、陈旧 session | 物品不变并 resync | 从棺转入背包或 panic |
| lifecycle invalidation | owner 跨维 / 超距 / 消失 | close、释放锁、清 session | 碎棺、孤儿映射、继续占锁 |
| lifecycle idempotency | 已关闭 session 再 tick | no-op 且状态一致 | 二次副作用或 panic |
| regression | 普通物资棺开取与其它 external container | 原契约保持 | 将物资棺规则错误外溢 |

黑盒 bot e2e 仅在现有 harness 能无协议扩写地覆盖且本地门禁不足以锁定 C2S consumer 时执行；不能用 smoke 代替 server 完整门禁。

## 6. Skeleton 对抗复核背景（不替代 P0 证据）

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

### Skeleton 最终裁决

反方最终接受：即使普通客户端通常被 layer 可见性挡住，`supply_coffin` 的服务端权威链路仍只信任全局 `entity_id/session_id + XYZ 距离/owner`，缺少同维校验，导致伪造 / 陈旧请求可跨维同坐标开棺并在会话内继续搬运，属于独立 server gameplay 授权漏洞。实施者仍须在 P0 独立证真或推翻此结论。

## 7. 完成契约

- 真 bug 与非 bug 两条分支都必须有独立 commit、绑定干净 HEAD 的 validator、完整 server 门禁、最新主线同步复验和最终归档 validator。
- 任一测试失败不得称为 pre-existing；同一 TODO 连续三轮无法通过时按流程记录 `[BLOCKED: 原因 + 测试名 + 关键错误]`，继续可独立推进项，但存在 BLOCKED 时不得归档。
- 归档前只允许更新本 active plan；不修改 `docs/worldview.md`、`docs/library/` 或其它 plan。
