# Bong · plan-bughunt-supply-coffin-cross-dimension-session-gate-v1

> **Finished BugFix Plan（2026-07-13 归档，2026-07-15 追加 PR review 返工复验）**。来源：`docs/plans-skeleton/plan-bughunt-supply-coffin-cross-dimension-session-gate-v1.md`。已证真实例：物资棺 `supply_coffin` 的专属 open / lifecycle / external move 链路曾只信任全局 `entity_id/session_id` 与裸 XYZ 距离，伪造或陈旧 C2S 因而可能跨维开棺、搬运或持续占锁。

## 阶段总览

| 阶段 | 状态 | 可核验交付物 |
|---|---|---|
| P0 第一性原理证真 / 证伪 | ✅ 2026-07-13 | 可达调用链、已有防护盘点、修复前契约复现或非 bug 证据 |
| P1 最小权限修复 | ✅ 2026-07-13 | open、move、lifecycle 的服务端同维 / 距离授权与会话清理 |
| P2 饱和回归 | ✅ 2026-07-13 | server targeted tests 覆盖成功、边界、拒绝与状态转换 |
| P3 闭环验收 | ✅ 2026-07-15 | 主 agent 对抗审查、三栈完整门禁、最新主线复验、Finish Evidence 与归档 |

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

- [x] ✅ 2026-07-13 — 从 `bong:client_request` 到 open / move consumer 逐层确认正常玩家、bot、陈旧 UI 请求是否可达，检查 entity 可见层、`CurrentDimension`、owner、距离、session source 与 cleanup 的所有现有防护。
- [x] ✅ 2026-07-13 — 先以提交 `12f2a660` 加入修复前契约测试；94 项定向测试中 11 项按预期失败，证实跨维同 XYZ open / move / lifecycle 会产生真实副作用。
- [x] ✅ 2026-07-13 — 授权只读取 server 的 active source、`CurrentDimension`、`Position` 与 session owner；客户端可见性和 `EntityLayerId` 不作为安全边界。

### P1 最小权限修复

- [x] ✅ 2026-07-13 — `ActiveSupplyCoffin` 明确记录 `DimensionKind::Overworld`，与 refresh / dev spawn 的实际 layer 对齐。
- [x] ✅ 2026-07-13 — open 在 roll loot、分配 session 与发送 payload 前统一校验 active source、有限坐标、维度存在 / 一致及 4.5 格边界。
- [x] ✅ 2026-07-13 — move 与 lifecycle 通过 session 映射回实体，再统一复核 owner、active source、维度与 6.5 格边界；普通 `StorageCrate` 契约保持不变。
- [x] ✅ 2026-07-13 — move 拒绝保持物品与 revision 不变并按权限 resync；lifecycle 失效会清当前映射、释放锁且不碎棺，冲突映射不会被误删。
- [x] ✅ 2026-07-13 — 复用既有 `Distance` close reason，未改变 C2S / S2C schema。

### P2 饱和回归

- [x] ✅ 2026-07-13 — 覆盖 open、move、lifecycle 的 happy path、维度 mismatch / 缺失、超距、owner mismatch、source 消失、重复 / 陈旧 session、映射冲突与幂等清理。
- [x] ✅ 2026-07-13 — 公共 authority helper 覆盖精确边界、边界外、缺 source 与非有限坐标；隔离测试证明非物资棺 external container 不受新规则影响。

### P3 闭环验收

- [x] ✅ 2026-07-15 — 用户明确要求本次不跑 subagent，因此未运行、也未伪造独立 validator；主 agent 对最终代码候选树 `origin/main...00339c3e` 完成逐入口对抗审查，并落实协议夹具、断言诊断、返回重开、地形扰动隔离、独立 forged-open 目标、真实 C2S move 续权与 server-authoritative barrier 等假绿排除项。
- [x] ✅ 2026-07-15 — `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` 在最终 server 业务树全绿；主线同步只改变 client/docs，合并前另以 JDK 17 完成 client 全门禁。
- [x] ✅ 2026-07-15 — 真实 C2S → server → S2C bot 场景通过：主世界第一具棺建立 session 后暂停该玩家的 lifecycle cleanup，跨到 TSY 同裸 XYZ 时第二具未占用棺 forged open 被拒，原有效 session 的真实网络 move 也在 mapping / owner / source 全部仍有效时被拒并回推权威状态；恢复 lifecycle 后才发生唯一 close，再验证 cleanup、返回重开与 forged open 无隐藏副作用。
- [x] ✅ 2026-07-15 — `/review` 返工中的 `/tpdim` 只 emit 正式 `DimensionTransferRequest`，并在 Respawn 后通过可逆坐标脉冲给出最终权威 PositionLook；`/supply_coffin barrier` 则提供 request/open/lifecycle 系统之后的确定性黑盒处理水位，两者都只服务 dev/E2E。
- [x] ✅ 2026-07-15 — 先后同步 `origin/main@32a34f8e`、`390f22e5`、`c231666d`、`4ad0c170` 与 `40df3dce`；最新 merge `00339c3e` 无冲突，且 server / bot 业务树未变化，主线带入的 client 树以 JDK 17 完整复验。
- [x] ✅ 2026-07-13 — 全阶段标记完成并填写唯一 `## Finish Evidence`；归档后由主 agent 复核最终干净 HEAD，独立 validator 例外继续如实披露。

### 4.1 开放问题与决议

**开放问题：无未决开放问题。** P0 提出的授权口径、会话续权、入口级验收和独立 validator 四项均已决议；后续 PR gate 只验证这些既定契约，不再扩写需求。

| 项目 | 结论 | 实施方案 | 边界条件 | 双锚点 |
|---|---|---|---|---|
| open 授权口径 | 裸 XYZ 与客户端可见层都不是授权；必须由 server 同时确认 active source、逻辑位面、有限坐标和距离 | `authorize_supply_coffin_open` 在 roll loot、分配 session、发送 payload 前统一 fail closed | 精确 4.5 格放行；缺 source / dimension、跨维、超距、NaN、±Infinity 拒绝；不改变 wire schema | `server/src/supply_coffin/authority.rs:23`、`server/src/supply_coffin/interact.rs:101` + plan §0、§P1 |
| 已开 session 续权 | `session_id + owner` 不是持续授权；move 与 lifecycle 每次都要回查 active source 和玩家权威状态 | move 在迁移物品前调用 session authority；lifecycle 失效以既有 `distance` reason close、回推 snapshot 并释放映射 / 锁 | 精确 6.5 格放行；普通 `StorageCrate` 不套用物资棺规则；拒绝不得改变物品或 revision | `server/src/network/client_request_handler.rs:16552`、`server/src/supply_coffin/lifecycle.rs:107` + plan §P1、§P2 |
| 入口级与黑盒验收 | helper 单测不足以证明 wiring；非有限坐标、有效 session move 和真实跨维状态转换都必须走入口 | Rust 覆盖真实 open payload、有效 session 的真实 C2S move 与 lifecycle；bot 通过 `/tpdim`、玩家级 lifecycle pause/resume 与处理 barrier 覆盖 open → 合法 move/update → 同 XYZ TSY Respawn → 第二具棺 forged open 拒绝 → 原有效 session move 拒绝/resync → 恢复后 close/snapshot → 旧 move cleanup 拒绝 → 返回重开 | 合法与陈旧 move 均动态选择不同权威空位；第二具棺排除 `opened_by` 假拒绝并在返回后首次合法 open 取得紧邻 session；XYZ 偏移仅 0.25 格，Respawn 后坐标脉冲最终恢复精确目标且 XYZ flags 为绝对值；pause 只延迟 lifecycle，不豁免 move/open authority；E2E 不能替代 server 完整门禁 | `server/src/schema/proto_convert.rs:1662`、`server/src/network/client_request_handler.rs:4585`、`server/src/supply_coffin/lifecycle.rs:478`、`server/src/cmd/dev/supply_coffin.rs:63`、`server/src/cmd/dev/tpdim.rs:62`、`scripts/bot/scenarios/inventory_supply_coffin_cross_dimension.py:15` + plan §5、§P3 |
| 独立 validator 例外 | 用户明确禁用 subagent，本轮 validator 未运行；不得声称 PASS | 由主 agent 完成逐入口 diff 审查、完整门禁、协议回归、干净服 E2E，并在 plan / PR 透明披露 | 例外只覆盖独立 validator，不豁免测试、CodeRabbit、`/review` 或 e2e；不自动 merge | `docs/finished_plans/plan-bughunt-supply-coffin-cross-dimension-session-gate-v1.md:85` + plan §P3、§4.1、§Finish Evidence |

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

黑盒 bot e2e 是本 plan 的必需门禁：必须用现有 harness 走真实 C2S → server → S2C 跨维链路；不能用 helper 单测或 smoke 代替该场景，也不能用该场景代替 server 完整门禁。

## Finish Evidence

### 落地清单

- `server/src/supply_coffin/authority.rs` 提供 open 4.5 格、session 6.5 格的统一服务端权威校验；缺 source、缺维度、跨维、超距与非有限坐标全部 fail closed。
- open、external move、lifecycle 三条运行时链均接入 active source / `CurrentDimension` / `Position` 校验；拒绝发生在物品或 loot 状态变更前。
- lifecycle 清理只删除仍指向当前棺实体的 session 映射，释放 `opened_by` 且不碎棺；reopen 可恢复缺失映射但拒绝覆盖冲突映射。
- 非 owner / 陈旧 session 只回推请求者背包，避免泄露外部容器内容；普通 `StorageCrate` 的既有 move 契约保持不变。
- `/tpdim <overworld|tsy>` 只生成正式 `DimensionTransferRequest`，由既有 transfer consumer 权威更新 layer、`CurrentDimension`、`Position` 与 Respawn；目标 X 偏移 0.25 格，Respawn 后再以 0.001 格可逆脉冲生成最终权威 PositionLook，最终坐标不漂移且仍处于旧 4.5 格门限内。
- `/supply_coffin lifecycle pause|resume` 只对执行者延迟 session cleanup，move/open authority 不读取该 marker；`/supply_coffin barrier` 在 request/open/lifecycle 系统之后确认黑盒处理水位，二者均为 dev/E2E 控制面。

### 关键提交

- `12f2a660`：加入修复前红测，94 项定向测试中 11 项目标契约失败。
- `b5ae89a7`：接入物资棺跨维 session 权威修复。
- `377ea937`：修复测试初始化的 clippy 门禁。
- `130d63d3`：合并 `origin/main@bf0b2738` 并复验。
- `d2fd0f35`：补齐真实 open payload 与 NaN / ±Infinity 的 open、move、lifecycle 入口回归。
- `26d2ba4b`：增加 bot payload 解码、跨维黑盒场景，并修正 `/tsy_spawn` 入场后自动回弹。
- `47b479de`：合并 `origin/main@f889073a` 并完成最终复验。
- `b9731b4f`：用真实 update / close 协议夹具、合法 move 与不同权威空位强化黑盒验收，并补齐 review 要求的断言诊断。
- `fc66dddd`：让合法 move 残留与背包落位失败同时输出期望和权威实际位置。
- `28a392a3`：补齐 TSY Exit 返回主世界后按原 Marker 重开同 session，并逐字段验证棺内实例未变的黑盒回归。
- `1a57d65a`：合并 `origin/main@32a34f8e` 并复验物资棺跨维会话门禁。
- `230bef0a`：合并 `origin/main@390f22e5` 并复验跨维返回重开链路。
- `51e97681`：在场景清包后通过 server-authoritative `/tpzone spawn` 固定 setup 区域，隔离邻列地形抬升与穿地恢复扰动，同时保留真实跨维 session 链路。
- `6b79d299`：合并 `origin/main@c231666d`，在最终主线 Rust 树上完成完整门禁与黑盒复验。
- `9b07a2a0`：移除 `545e83c6` 引入的无关伪灵脉 persistence 测试 diff；在该历史 checkpoint 恢复为与当时 `origin/main` 无差异，不把 production persistence 逻辑算作本 plan 交付。
- `0caafa20`：新增 `/tpdim` 权威跨维测试入口，只 emit 正式 `DimensionTransferRequest` 并保留旧 XYZ 授权邻域。
- `3165cf2b`：用独立第二具未占用物资棺验证跨维同 XYZ forged open，并以真实 C2S Rust 测试锁定有效 session 的 move 续权门禁。
- `fe20600a`：跨维目标 X 偏移 0.25 格，强制生成可观测 PositionLook，同时保持在旧 4.5 格门限内。
- `17b38d18`：PositionLook 只校验 XYZ 相对 flags 低三位，允许 Valence 合法使用相对 yaw / pitch `0x18`。
- `65050c8d`：合并 `origin/main@4ad0c170`，无冲突；在最终代码候选树上重跑完整 server 门禁与 bot E2E。
- `2441292f`：记录 `/review` 对有效 session 黑盒续权、确定性处理水位与跨维后权威坐标的复审要求。
- `0bb0384d`：增加玩家级 lifecycle pause/resume 与 server-authoritative barrier，并以单测锁定 marker 生命周期和 session 保留边界。
- `252e7777`：在正式跨维 consumer 之后增加可逆坐标确认脉冲，确保 bot 在 Respawn 后读取最终权威 XYZ。
- `cc583815`：把有效 session 的跨维真实网络 move 拒绝、forged open 无副作用和 resume 后唯一 close 接入单场景 bot E2E；protocol 增加 Respawn 维度字段 pin。
- `b5d7cea9`：把 `supply_coffin barrier` 与 `lifecycle pause|resume` 补入调试命令树 pin，防止 E2E 控制面注册漂移。
- `87a52f43`：仅稳定伪灵脉 Startup hydration 测试的跨秒断言；读取落盘 `snapshot_wall` 并按启动前后墙钟夹住合法年龄，production persistence 逻辑未改。
- `00339c3e`：合并 `origin/main@40df3dce` 的快捷键修复，无冲突；server / bot 树不变，主线 client 增量以 JDK 17 完整复验。

### 测试与审查

- 定向（最终 server 业务树 `87a52f43`；`00339c3e` 未改变 server）：`cargo test tpdim` 为 6/6，`cargo test supply_coffin` 为 110/110，`cargo test external_move_` 为 8/8，调试命令树 pin 1/1，非物资棺隔离用例 1/1。
- server 最终完整门禁：`cargo fmt --check` 与 `cargo clippy --all-targets -- -D warnings` 通过；最终默认并行 `cargo test` 主测试集 11,687 passed / 0 failed / 1 ignored，附加测试集 11/11、1/1、4/4 通过，doc tests 5 ignored，退出码 0。由于 `/tmp` 20GB tmpfs 已满，门禁显式使用根盘 `TMPDIR`；一次未改写临时目录的运行产生 462 个 I/O 失败，不作为代码结果。
- 伪灵脉墙钟测试在 `87a52f43` 前以默认并行全量连续两轮稳定得到 11,686 passed / 1 failed / 1 ignored，唯一失败均为同一秒边界跨越导致 `expected age 800 / actual 820`。修正后仍单独断言落盘年龄为 800、离线增量只能是 `TICKS_PER_SECOND` 整倍数，并按 bootstrap 前后真实墙钟计算合法区间；最终高负载全量通过，production hydration 逻辑未改。
- 早期跨栈同步复验：JDK 17.0.19 下 `./gradlew test build`，13 actionable tasks 全部执行，`BUILD SUCCESSFUL`；`npm run build` 通过，`npm test -w @bong/schema` 为 29 files / 872 tests 全绿。review 业务返工未修改 agent 或 schema。
- 最新主线 client 复验（未提交 merge tree → `00339c3e`）：JDK 17.0.19 下 `./gradlew test build --rerun-tasks` 为 4,077 tests / 0 failed，13 actionable tasks 全部执行，`BUILD SUCCESSFUL`。首次尝试因已满 `/tmp` 使 8 个临时文件测试失败；给 Gradle、编译器和测试 JVM 显式设置根盘 `java.io.tmpdir` 后完整重跑全绿。
- 真实 C2S Rust 回归：`supply_coffin_external_move_real_c2s_rejects_cross_dimension_same_xyz_while_session_is_valid_and_resyncs` 在 session mapping、`opened_by` 与 active source 均仍有效时，证明跨维同 XYZ move 被拒、物品 / revision 不变，并回推 container + inventory resync。
- bot protocol（最终 bot 树）：`python3 scripts/bot/test_protocol.py` 为 59/59，退出码 0；测试结束另报告未关闭 socket `ResourceWarning`，不影响结果。
- 干净服 bot E2E（最终 bot 树，run-tag `s20v3`）：`inventory_supply_coffin_cross_dimension` 为 1/1 PASS（9.0s）；真实走第一具棺 open / 合法 move → lifecycle pause → `/tpdim tsy` Respawn + 最终坐标确认 → 第二具棺 forged open 拒绝并越过 barrier → 原有效 session 跨维 move 拒绝/resync → lifecycle resume 后唯一 close/snapshot → 旧 move cleanup 拒绝 → `/tpdim overworld` → 第二具棺首次合法 open 取得紧邻 session → 第一具棺返回重开同 session。
- 最终 E2E 证据：`.sisyphus/evidence/bot-e2e-1199-review-s20v3/protocol.log`、`.sisyphus/evidence/bot-e2e-1199-review-s20v3/scenarios.log`、`.sisyphus/evidence/bot-e2e-1199-review-s20v3/server.log`。
- 主 agent 对 `origin/main...00339c3e` 完成第一性原理与对抗式 diff 审查，确认授权先于副作用、session 映射所有权正确、普通 external container 未被误伤，且 setup 地形、第二具 forged-open 目标、0.25 格权威坐标偏移、Respawn 后坐标确认脉冲、合法 / 陈旧 move 目标格、处理 barrier 与 close 水印均不会制造假绿。
- 独立 validator：未运行。用户明确要求“本次不跑 subagent，仅主agent实施”；本记录不声称 validator PASS，PR 继续透明披露该例外。

### 遗留与后续

- 无 `[BLOCKED: ...]` 项，无 wire schema、worldview、依赖版本或真元守恒改动。
- `/review`、CodeRabbit 与 GitHub e2e 属 PR gate，推送最终 HEAD 后继续等待并处理；不自动 merge。

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

- 真 bug 与非 bug 两条分支都必须有独立 commit、完整 server 门禁和最新主线同步复验。本次用户明确禁用 subagent，绑定 HEAD 的独立 validator 条款由用户指令覆盖；必须如实记录未运行，禁止伪造 PASS。
- 任一测试失败不得称为 pre-existing；同一 TODO 连续三轮无法通过时按流程记录 `[BLOCKED: 原因 + 测试名 + 关键错误]`，继续可独立推进项，但存在 BLOCKED 时不得归档。
- 归档前只允许更新本 active plan；不修改 `docs/worldview.md`、`docs/library/` 或其它 plan。
