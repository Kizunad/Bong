# plan-refactor-c2s-gate-v1 — C2S 请求统一门禁中间件 + client_request_handler 巨石拆分（重构轨 R4）

> 所属总纲：`docs/plans-skeleton/plan-refactor-master-v1.md`（草案权威）。一句话：给所有 `ClientRequestV1` 变体建立穷尽式声明门禁（距离 / 维度 / 所有权 / 状态前置），同时把 `client_request_handler.rs` 拆成按域注册的 handler 模块，使“只信裸坐标即可跨维远程操作”这一类缺陷不能再静默进入生产。

## 阶段总览

| 阶段 | 主题 | 状态 |
|---|---|---|
| P0 | 设计收口 + 104 变体门禁矩阵 + 吸收清单验真 | ✅ 2026-08-03 |
| P1 | 全量门禁声明 + 中间件原子上线 + 已知漏洞簇首批接入 | ⬜ |
| P2 | 巨石拆分批次 A（combat / production / world / social / npc） | ⬜ |
| P3 | 删除重复门禁 + adapter 收敛 | ⬜ |
| P4 | bot 验收 + 被吸收 plan 结案 | ⬜ |

## 现状证据（P0 验证基线：2026-08-03，Rust enum/matrix inventory 基于 `663fc4391ca24d8c4586a9625723ee280d329fff`）

- 当前权威枚举是 `server/src/schema/client_request.rs:35-728` 的 `ClientRequestV1`，共有 **104** 个变体；原 skeleton 的“113 个”是 2026-07-27 侦察快照，不能继续作为实现计数。P1 的全量穷尽门以 Rust 枚举实际变体集为准，新增变体未声明即编译失败。
- `ClientRequestV1` 的 enum-level serde wire contract 固定为 `#[serde(deny_unknown_fields, tag = "type", rename_all = "snake_case")]`：三个选项各恰好一次，任何 `content`、`untagged`、enum-level `rename` 或其他未验证选项都必须由 checker fail-closed；field-level serde 属性不改变该 enum-level contract。
- `server/src/network/client_request_handler.rs:522-2960` 的 `handle_client_request_payloads` 仍以单个大 `match` 解码、验版本并派发全部请求；空间、所有权和状态校验分散在下游 helper/system，派发前没有统一、可审计的 mutation barrier。
- 已有门禁证明领域半径不能拍成一个全局数值：`craft/workbench.rs:59` 为 3 格，`mineral/probe.rs:13` 为 6 格，`supply_coffin/authority.rs:12-13` 为 4.5/6.5 格，`coffin/mod.rs:101,1288-1307` 为 6 格且主世界限定，`client_request_handler.rs:468-469,14632-14748` 的气色检视/NPC 为 6 格。- 维度感知 zone API 已存在：`world/zone.rs:303-345` 的 `find_zone(dim, pos)` / `find_zone_mut_by_pos(dim, pos)`；缺陷来自调用方硬编码或根本不携带 `CurrentDimension`，不是再造第二套 zone registry。
- #1287 已于 2026-07-27 merge（merge commit `9931a3a1fdd5b4d6b38f4da2fce43f400e26bf0d`）。R6 `plan-refactor-wire-s2c-v1` 仍是 skeleton，P1 尚未落地；依据总纲 §3/§4.1，R4 的 contract-first 声明、adapter 与 pin 可继续推进，但各 production activation 仍处于 Wave 2 阻塞态，须等待对应 R5/R6/R2 owner responsibility 按各自 plan 就绪并服从 atomicity invariants。本 PR 只完成 P0 docs closeout。

## 接入面

- **进料**：`bong:client_request` 单通道；`ClientRequestV1`；请求者 `Position` / `CurrentDimension` / `Lifecycle` / inventory；R1 提供的 session authority；R10 提供的 inventory transaction。
- **出料**：通过门禁的请求进入各域 handler；拒绝产生内部 `GateDenied { request_kind, reason }`，P1 先通过既有 `server_data::EventAlert` 向请求者发送不泄漏目标信息的 toast。
- **共享类型**：`server/src/network/gate/` 的 `GateSpec`、`GateTarget`、`DistanceRule`、`DimensionRule`、`OwnershipRule`、`StateGateId`、`GateDenialReason`、`GateContext`；zone 解析只复用 `ZoneRegistry::find_zone(dim, pos)`。
- **跨仓库契约**：P0 不改 wire。P1 复用现有 `event_alert`，不新增字段；若要可靠携带 `request_kind/reason_code/request_id`，由 R6 按其契约流程新增 `request_rejected`，同步 proto / Rust / TypeBox / samples / client handler，R4 不越权修改 `*_emit.rs` 或 `proto_convert.rs`。
- **worldview 锚点**：本轨不新增玩法规则；空间隔离沿用 `worldview.md §十一 L928`、`§十六.五 L1569` 的坍缩渊无安全锚点约束，以及各已落地玩法自己的近身交互语义。
- **qi_physics 锚点**：门禁必须发生在任何 inventory 或 qi mutation 前；R4 不引入真元公式或常数，不改变既有 ledger 路径。

## P0 决议：门禁模型冻结

### 1. 穷尽式声明，不用默认放行

`ClientRequestV1` 提供穷尽 `gate_spec()` match。每个变体必须返回 `RequestGate::Spec(GateSpec)` 或 `RequestGate::NoGate(NoGateReason)`；禁止 `_ => NoGate`、默认 spec、按名称字符串猜测或运行时反射。新增枚举变体时 Rust match 立即编译失败，迫使作者显式选择门禁。

目标结构冻结为：

```rust
pub enum RequestGate {
    Spec(GateSpec),
    NoGate(NoGateReason),
}

pub struct GateSpec {
    pub target: GateTarget,
    pub distance: DistanceRule,
    pub dimension: DimensionRule,
    pub ownership: OwnershipRule,
    pub state: &'static [StateGateId],
}
```

`GateTarget` 只描述权威解析方式（request block pos、protocol entity id、UUID、inventory instance、session id、player id、zone id），不缓存客户端声称的目标实体。解析失败统一拒绝；缺 `Position` / `CurrentDimension` / owner/session authority 时 fail-closed，不隐式当作 Overworld 或“无 owner”。

**落点**：未来 `server/src/network/gate/{mod,spec,target,reason}.rs`；`server/src/schema/client_request.rs:35-728`；plan P1/P3。

### 2. 距离 profile 同时冻结 metric 与半径

玩家 C2S reach 不强行统一几何形状。`DistanceRule::Profile` 必须引用同时携带 `metric` 与 `max_blocks` 的命名 profile；当前 metric 至少包含 `Euclidean3dSquared` 与 `Chebyshev3d`。欧氏 profile 比较 `distance_squared <= max_blocks²`，Chebyshev profile 比较 `max(|dx|, |dy|, |dz|) <= max_blocks`，两者边界恰好等于半径时都放行且不在热路径开方。方块邻接/结构拓扑继续使用领域 helper，不冒充玩家 reach。

冻结 profile：

| profile | metric | 半径 | 依据 / 使用面 |
|---|---|---:|---|
| `Workbench` | `Chebyshev3d` | 3.0 | 复用既有 `is_within_workbench_range` 接受区域；`[3,3,3]` 必须继续放行 |
| `DroppedLoot` | `Euclidean3dSquared` | 2.5 | 既有 server 权威拾取范围 |
| `SupplyCoffinOpen` | `Euclidean3dSquared` | 4.5 | 既有 authority |
| `ExternalSession` | `Euclidean3dSquared` | 6.5 | 既有打开后 session 容忍范围 |
| `NearbyInteract` | `Euclidean3dSquared` | 6.0 | NPC、气色、棺、炼丹、炼器、灵田、布阵、方块放置、给丹、夺舍 |
| `None` | — | — | 无世界目标；仍可有 ownership/state gate |

P1 不借重构改玩法接受区域；若已有领域 metric 或常数不同，先登记为命名 profile 并复用现有 predicate/值，后续平衡 PR 才能调整。测试既 pin 数值边界，也 pin metric 特有边界，防止“半径没变但可交互区域收缩”。

**落点**：`craft/workbench.rs:59`、`mineral/probe.rs:13`、`supply_coffin/authority.rs:12-13`、`coffin/mod.rs:101`、`client_request_handler.rs:468-469`；plan P1/P3。

### 3. 门禁顺序是 mutation barrier

固定求值顺序：

1. ingress 按请求者执行每 tick token budget；预算耗尽即丢弃/合并，不 decode、不解析目标、不进入 handler；
2. wire decode + `v == SUPPORTED_VERSION`；
3. 解析请求者权威上下文；
4. 对携目标的请求先建立 requester visibility/capability（session participant、inventory owner、已同步可见实体或领域显式 public target）；未建立前不得向请求者区分目标是否存在；
5. 解析目标并在内部依次检查 dimension、distance；
6. ownership / participant；
7. state preconditions；
8. 才进入 handler / event dispatch / inventory transaction / qi ledger。

同一 ingress batch 内，state gate 必须先读本 batch 的 projected state、再回退到领域权威状态；请求通过后、dispatch 前立即推进 projection。Forge 迁移必须复用现有 `pending_forge_steps` 语义，保持同 update 的 `ForgeStepAdvance → ForgeTemperingHit` 放行、逆序拒绝，不得只读尚未消费 event 的 `ForgeSessions` 快照。

同一请求多项失败时内部只记录最前一项，保证诊断与测试确定；外部反馈按下一节的安全类别折叠。域 handler 可以保留更细业务校验，但不得在统一门禁前消费物品、扣 qi、创建 session 或写世界。P1 迁移期允许旧校验继续运行，旧校验与 spec 结果不一致时测试/诊断报错；不允许“一个版本期”在生产中双执行副作用。

**落点**：`client_request_handler.rs:546-690` 的 decode/version 后、`match request` 前；plan P1。

### 4. 拒绝原因、资源预算和 wire 边界

内部稳定 enum 冻结为：`UnsupportedVersion`、`MissingAuthorityContext`、`TargetNotFound`、`NotVisible`、`WrongDimension`、`OutOfReach`、`NotOwner`、`InvalidState`、`Busy`、`Expired`、`Conflict`、`RateLimited`。内部诊断只记录 request kind + reason，不记录聊天、目标身份或原 payload。

所有客户端控制的 protocol id / UUID / player id / world target，在 requester visibility/capability 建立前发生的 `TargetNotFound`、`NotVisible`、`WrongDimension`、`OutOfReach`、`NotOwner` 必须映射为同一个外部类别与固定文案（`TargetUnavailable` / “目标不可用”）；只有已获授权 session/inventory 内的状态错误才可外显 `InvalidState` / `Busy` / `Expired` 等类别。相同探测序列无论目标不存在、跨维、超距或属于他人，外部 packet 类型、字段、文案和发送时序均不可区分；细原因只留内部指标。

P1 在 `bong:client_request` 未可信边界落每客户端 token bucket：默认容量 **32**、每 tick 补充 **8**、每个 payload 在 decode 前消费 1 token；同一玩家同一 tick 最多执行 32 个请求，后续请求不做 gate/handler 工作。拒绝输出另设每客户端预算：每 **20 ticks** 最多单播 **1** 条 `EventAlert`，窗口内同类拒绝合并；日志/metric 每客户端、request kind、内部 reason 每 **100 ticks** 最多一条汇总（携 suppressed count），禁止逐 denial `warn!`。断线/角色切换清理 bucket 状态，budget store 有界于当前连接数。

P1 复用现有 `ServerDataPayloadV1::EventAlert { event: Generic, message: "目标不可用"/"当前状态不可用", zone: None, duration_ticks: Some(70) }` 给请求者单播；它没有 request/reason/request_id 字段，不能被宣称为结构化 ack。结构化 `request_rejected` 只有在 R6 冻结并落地契约后才启用；字段至少为 `request_kind`、安全折叠后的 `reason_code`、可选 `request_id`，且必须携双端正反 sample，绝不发送内部 target-resolution reason。R4 P1 不私改 R6 独占的 emit/proto 文件。

**落点**：`server/src/schema/server_data.rs` 的 `EventAlert`、`server/src/network/freshness_probe_emit.rs:67-93`、`client/src/main/java/com/bong/client/network/EventAlertHandler.java:33-67`；plan P1，R6 P1/P4。

### 5. 延寿棺 ownership 使用已认证持久化主体

`CoffinBreak` 与 `CoffinMenuReclaim` 冻结为 **owner-only**；本 plan 不采用公共破坏策略。生产当前使用 `ConnectionMode::Offline`，握手 `Username` 可由客户端冒充，并用于恢复 `Lifecycle.character_id`；因此该字段只能标识持久角色记录，不能作为认证证明。Offline transport 不提供认证或加密通道，本 plan 不得生成、发送或在 C2S payload 中接受可重放的 durable bearer credential。按总纲 §3 Wave 表与 §4.1 第 3、5 条裁决，当前 owner plans 只有以下可引用的通用接缝：R3 P1 Slice framework、P3 shutdown flush/tick rebase、P4 runtime persistence extension points；R6 P1 schema-generation chain、P2 client bridge/consumer contract、P3 atomic production activation。R3 尚未定义 coffin-specific authenticated owner persistence/hydration API，R6 也尚未定义 authenticated owner-proof wire/client contract；因此在对应 owner plan 补齐 phase/artifact/consumer 并同步总纲 Wave 表前，R4 只能声明 contract-first gate/test seams（declared、unwired、test-only），不得接 production。owner-plan amendment 落地且提供可认证、不可重放的 owner authority 后，才可原子完成 registry lifecycle 与 gate consumer：

- `CoffinEntity` 增加持久化的 owner authority record，不能使用 ECS `Entity`、`Username` 或仅由 offline username 恢复的 `Lifecycle.character_id` 作为授权依据；
- `CoffinRegistry::insert` 必须接收并存储 owner authority record，`handle_coffin_place_requests` 缺认证 owner authority 时在消耗物品/写方块前 fail-closed；
- `CoffinRegistry::reclaim_occupied` 与断线恢复必须携同一 authenticated owner authority，fallback 重建也不得生成无 owner 的棺；
- break/reclaim 以服务端认证的 owner authority 校验请求；owner 缺失、离线连接无法证明 owner、恢复记录不一致一律拒绝且零 mutation，由迁移工具处理，不把同名连接猜作 owner。
- **R3/R6 owner-plan amendment（tracked follow-up）**：现有 R3 P1 只冻结通用 Slice API，R3 P3/P4 只提供 flush、tick rebase 与运行态持久化的扩展点；现有 R6 P1/P2/P3 只提供 generation chain、client bridge 与 atomic activation machinery。后续 owner plan 必须先分别声明：R3 侧的 owner authority 持久化记录、migration/load guard、restart/reconnect hydration consumer；R6/对应 domain owner 侧的 authenticated owner-proof request/response、私密单播或受保护通道与双端 samples。最终文件路径、symbol、phase、consumer 由 owner plan amendment 冻结；在 amendment 前不得把 `server/src/persistence/coffin_owner.rs`、`coffin_owner_credentials`、`hydrate_coffin_owner` 或 coffin-specific R6 wire names 当作已存在的上游 artifact。未来 R3 slice 必须按 `(dimension, lower_x, lower_y, lower_z)` 定位并持久化 owner authority、coffin grade/lifecycle 与 schema version；migration 保留旧行，坏行或缺字段进入 load guard，不以 ownerless 或新 authority 覆盖。启动和玩家重连在 registry reclaim 前只有成功读取并重新认证同一 owner authority 才能恢复 registry；缺记录、migration/hydration 错误或 authority 不一致保持 unowned 并让 break/reclaim fail-closed。不得把 raw bearer secret 放入 offline transport、durable client store 或可重放的 C2S 请求。

P1 当前测试必须覆盖 contract-first seam 的 owner-only 语义、同 offline username 但无 authenticated owner proof 的拒绝、缺失或错误 owner proof 的 fail-closed、以及拒绝时方块/registry/inventory 不变；在 R3/R6 amendment 与对应 artifact 合入、并完成原子 production cutover 后，再追加 owner 放置后 break/reclaim、已重新认证的 owner 重连、ECS entity 改变不影响 owner、进程重启 hydration 同一 authority 再恢复 registry、缺 owner recovery fail-closed 等持久化验收。

**落点**：当前可引用的上游接缝是 R3 P1/P3/P4 的 Slice/flush/tick-rebase/runtime-persistence framework，以及 R6 P1/P2/P3 的 generation-chain/client-bridge/atomic-activation machinery；`server/src/coffin/mod.rs::CoffinEntity`、`CoffinRegistry::{insert,reclaim_occupied}`、`handle_coffin_place_requests`、矩阵 #27/#28 和 plan P1 属 R4 接入面。coffin-specific persistence/hydration/wire/client-store artifact 是 tracked follow-up，必须先由 owner plan amendment 定义后才能形成 production cutover 依赖。


### 6. handler 拆分不做动态注册

P2/P3 使用编译期分域函数与穷尽 match，不采用 `HashMap<String, dyn Handler>`、反射或字符串路由。目标模块：

- `network/client_request/{combat,production,world,social,npc,inventory,session}.rs`；
- 顶层只保留 channel 过滤、decode、version、gate、按 enum 分域派发；
- Bevy `SystemParam` 按域拆分，跨域接缝只消费被依赖轨冻结的 API。

拆分批次只移动行为；不得趁机修改 R1 session、R6 emit/proto、R10 inventory transaction 或业务数值。

**落点**：`client_request_handler.rs:522-2960`；总纲 §4；plan P2/P3。

## P0 104 变体门禁矩阵

记法：距离列为目标 + profile；维度列只描述请求者与空间目标的实际维度关系，`同目标` 表示请求者与目标权威维度相同，`主世界` 表示请求者必须是 Overworld，`—` 表示无空间维度门；session / request 的 authenticated authority 不填入维度列，统一见下方 **Authority contract field**；所有权/状态是 **P3 终态要求**。现状列的“域内”表示已有下游校验但尚未统一，“缺”表示本轮验真的真实缺口，“显式 no_gate”仍必须写理由。

**Authority contract field**：矩阵的所有权 / participant 细化为独立的权威关系；`authenticated player` 表示由当前连接的玩家实体解析请求者，`session owner` 表示由 R1 session adapter 按 authenticated player 绑定并校验 session/request ID，`participant snapshot` 表示由 invite/offer 的服务端参与者快照校验。该字段不描述空间维度：`AgentUiResponse` 必须使用 `authenticated player → AgentUiSessionStore → request_id` 关系校验，不能写成 request dimension 或 request target。

- `authenticated player → session owner`: `BotanyHarvestRequest`, `CancelExtractRequest`, `CancelSearch`, `ExternalContainerMove`, `ExternalContainerClose`, `CraftCancel`, `ScrollReadClosed`。
- `authenticated owner authority → durable coffin owner`: `CoffinBreak`, `CoffinMenuReclaim`。
- `participant snapshot`: `SparringInviteResponse`。
- `authenticated player → AgentUiSessionStore → request_id`: `AgentUiResponse`。

上述 authority contract 不替代空间门：仅无空间目标的变体维度单元格为 `—`；`ExternalContainerMove` 必须由 session 解析其 world target，并校验请求者与该实体 `CurrentDimension` 相同，`ExternalSession` 仍是距离 profile。所有权 / participant 列仍保留每行的领域对象语义，以上字段为其可核验的 authority source。

| # | `ClientRequestV1` | 距离 | 维度 | 所有权 / participant | 状态前置 | P0 现状结论 |
|---:|---|---|---|---|---|---|
| 1 | `SetMeridianTarget` | — | — | self | lifecycle 可修炼 | 域内；无空间目标 |
| 2 | `BreakthroughRequest` | — | — | self | 非忙态、境界/资源前置 | 域内；挂 R1 state gate |
| 3 | `StartDuXu` | — | — | self | 非忙态、可渡虚 | 域内；挂 R1 state gate |
| 4 | `VoidAction` | zone / — | 当前 zone | self | 化虚、action 可用、zone 合法 | zone 锁缺；R4 只收 server 判定 |
| 5 | `MovementAction` | — | — | self | 存活、移动态允许 | 域内；显式无 target reach |
| 6 | `AbortTribulation` | — | — | self | 永久拒绝 | 当前 handler 明确 ignore |
| 7 | `HeartDemonDecision` | — | — | challenge participant | 事件未过期、choice 合法 | 域内 |
| 8 | `ForgeRequest` | — | — | self | 经脉锻造前置 | 域内；非世界炼器站请求 |
| 9 | `InsightDecision` | — | — | invite owner | trigger 未过期 | 域内 |
| 10 | `BotanyHarvestRequest` | session / — | — | session owner | session active、mode 合法 | 已有 session owner；迁 R1 adapter |
| 11 | `AlchemyOpenFurnace` | block / `NearbyInteract` | 同目标/主世界 | furnace owner (ownerless public) | furnace exists | 距离/维度缺 |
| 12 | `AlchemyFeedSlot` | block / `NearbyInteract` | 同目标/主世界 | active operator | furnace/session active、slot/material 合法 | 距离/维度缺 |
| 13 | `AlchemyTakeBack` | block / `NearbyInteract` | 同目标/主世界 | active operator | furnace active、slot 可取 | 距离/维度缺 |
| 14 | `AlchemyIgnite` | block / `NearbyInteract` | 同目标/主世界 | active operator | recipe/material/qi 前置 | 距离/维度缺 |
| 15 | `AlchemyIntervention` | block / `NearbyInteract` | 同目标/主世界 | active operator | phase 允许 intervention | 距离/维度缺 |
| 16 | `AlchemyTurnPage` | — | — | self | recipe book 可用 | 显式 no spatial gate |
| 17 | `AlchemyLearnRecipe` | — | — | self | recipe/unlock 合法 | 域内 |
| 18 | `AlchemyLearnRecipeFragment` | inventory / — | — | item owner | fragment 合法且未消费 | 域内；R10 transaction |
| 19 | `AlchemyTakePill` | inventory / — | — | item owner/self | 存活、丹毒阈值、丹可服 | 丹毒前置缺；mutation 前 gate |
| 20 | `AlchemyFurnacePlace` | block / `NearbyInteract` | 主世界 | item owner | 可放置、位置空闲 | 距离/维度缺 |
| 21 | `CoffinOpen` | block / `NearbyInteract` | 主世界 | — | tutorial coffin 可用 | 域内需登记 |
| 22 | `CoffinPlace` | block / `NearbyInteract` | 主世界 | item owner | 可放置 | 距离 + 主世界门已域内 |
| 23 | `BlockPlace` | block / `NearbyInteract` | 同目标 | item owner | target 可替换、无碰撞 | reach 缺；维度 layer 已域内 |
| 24 | `BlockPickerGive` | — | — | self | dev/creative 权限、count 合法 | 显式 no spatial gate |
| 25 | `CoffinEnter` | block / `NearbyInteract` | 主世界 | — | coffin exists、可进入 | 距离/维度已域内 |
| 26 | `CoffinLeave` | — | — | occupant self | 当前卧棺 | 显式 no spatial gate |
| 27 | `CoffinBreak` | block / `NearbyInteract` | 主世界 | authenticated owner authority | coffin exists、无人受保护占用 | 距离/维度已有；offline transport 不携带可重放 bearer credential；coffin-specific authenticated owner persistence/hydration/wire 是 R3/R6（及 domain owner）owner-plan amendment tracked follow-up；R4 仅 contract-first stub（declared/unwired/test-only），不接 production |
| 28 | `CoffinMenuReclaim` | block / `NearbyInteract` | 主世界 | authenticated owner authority | coffin exists、可回收 | 距离/维度已有；offline transport 不携带可重放 bearer credential；coffin-specific authenticated owner persistence/hydration/wire 是 R3/R6（及 domain owner）owner-plan amendment tracked follow-up；R4 仅 contract-first stub（declared/unwired/test-only），不接 production |
| 29 | `SpiritNichePlace` | block / `NearbyInteract` | 主世界 | item owner/self | 唯一锚点、位置合法 | 主世界/先验顺序需统一 |
| 30 | `SpiritNicheRepair` | block / `NearbyInteract` | 主世界 | niche owner + item owner | niche damaged | 域内需登记 |
| 31 | `SpiritNicheGaze` | block / `NearbyInteract` | 主世界 | — | niche exists、凝视达标 | 域内需登记 |
| 32 | `SpiritNicheMarkCoordinate` | block / `NearbyInteract` | 主世界 | — | niche exists、mark 能力 | 域内需登记 |
| 33 | `SpiritNicheActivateGuardian` | block / `NearbyInteract` | 主世界 | niche owner | guardian/material 前置 | 域内需登记 |
| 34 | `SparringInviteResponse` | invite / — | — | invite target | invite active、未过期 | participant/state 域内 |
| 35 | `TradeOfferRequest` | player / `NearbyInteract` | 同目标 | initiator owns offered item | 双方存活、目标可交易 | 同维缺；NPC reputation 误门另结案 |
| 36 | `TradeOfferResponse` | offer / `NearbyInteract` | 同 participant | offer target + 双方 item owner | offer active、未过期、双方存活 | 同维缺；接受时需复验 |
| 37 | `NpcInspectRequest` | entity / `NearbyInteract` | 同目标 | — | NPC 可交互 | 已有同维/距离 helper |
| 38 | `NpcDialogueChoice` | entity / `NearbyInteract` | 同目标 | dialogue participant | option 当前有效 | 已有空间门；participant 需统一 |
| 39 | `NpcTradeRequest` | entity / `NearbyInteract` | 同目标 | offered item owner | NPC 可交易、报价/信誉有效 | server 门已有；UI drift 非 R4 独占 |
| 40 | `ZhenfaPlace` | block / `NearbyInteract` | 主世界/同目标 | carrier/item owner | 位置、材料、qi、经脉前置 | 空间门缺 |
| 41 | `ZhenfaTrigger` | instance / profile-preserved | 同目标 | array owner | instance active、可触发 | owner 域内；空间语义需登记 |
| 42 | `ZhenfaDisarm` | block / existing 4.5 | 同目标 | — | target exists、mode/能力合法 | 域内常量需迁 profile |
| 43 | `QiScatterBeadUse` | optional block / `NearbyInteract` | 当前 zone | item owner | item/zone/ledger 前置 | 坐标可选路径需统一 |
| 44 | `LearnSkillScroll` | inventory / — | — | item owner | skill/unlock/meridian 合法 | 域内；R10 transaction |
| 45 | `TechniqueScrollUse` | inventory / — | — | item owner | technique/unlock 合法 | 域内；R10 transaction |
| 46 | `InventoryMoveIntent` | inventory / — | — | instance owner + container session | source/destination/revision 合法 | 域内；R10/R1 authority |
| 47 | `EquipFalseSkin` | inventory / — | — | item owner | slot/race/form gate | 域内 |
| 48 | `ForgeFalseSkin` | inventory / — | — | material owner | recipe/qi/race gate | 域内；transaction 前 gate |
| 49 | `InventoryDiscardItem` | inventory / — | current dimension for spawned loot | item owner | source/revision 合法 | 域内；spawn 维度必须保留 |
| 50 | `TreasureActivate` | inventory / — | — | item owner | slot capacity/equip gate | 域内 |
| 51 | `DropWeaponIntent` | inventory / — | current dimension for spawned loot | item owner | source/revision 合法 | 域内 |
| 52 | `RepairWeaponIntent` | station / `NearbyInteract` | 同目标 | weapon owner + station authority | station/material/session 合法 | station/distance/dimension/material 缺 |
| 53 | `PickupDroppedItem` | entity / `DroppedLoot` | 同目标 | — | entry active、capacity | server 距离已有；维度缺 |
| 54 | `RemainsLoot` | UUID entity / `DroppedLoot` | 同目标 | loot authority | remains active、capacity | 保持 server 权威 2.5m pickup range；域内检查需迁 spec |
| 55 | `MineralProbe` | block / existing 6.0 | 同目标 | — | realm/tool/ore 合法 | 距离域内；维度需显式 |
| 56 | `FreshnessProbe` | inventory / — | — | item owner | realm/profile 合法 | 已有 owner/state；显式 no spatial gate |
| 57 | `ApplyPill` | inventory / — | — | item owner | target/self、丹毒/状态合法 | 域内需统一 mutation barrier |
| 58 | `SelfAntidote` | inventory / — | — | item owner/self | poisoned、qi/antidote 合法 | 域内 |
| 59 | `DuoSheRequest` | character/entity / `NearbyInteract` | 同目标 | caster self | target type/lifecycle/realm/line-of-sight policy | 距离/维度缺 |
| 60 | `QiColorInspect` | player/entity / existing 6.0 | 同目标 | — | realm/能力、目标可观察 | 已有 `resolve_qi_color_inspect_target` |
| 61 | `UseLifeCore` | inventory / — | — | item owner/self | lifecycle/realm 合法 | 域内 |
| 62 | `Jiemai` | — | — | self | incoming window/skill 状态 | 显式 no target reach |
| 63 | `ChargeCarrier` | inventory/equipped / — | — | carrier owner | qi/slot/状态合法 | 域内 |
| 64 | `ThrowCarrier` | equipped / — | — | carrier owner | charged、方向/功率合法 | 域内 |
| 65 | `AnqiContainerSwitch` | inventory/equipped / — | — | container owner | 暴露窗口/目标容器合法 | 域内 |
| 66 | `UseQuickSlot` | inventory/config / — | — | binding owner | cooldown/状态/item 合法 | 域内 |
| 67 | `QuickSlotBind` | inventory/config / — | — | self | slot/request_id/item 合法 | 域内 |
| 68 | `SkillBarCast` | optional target / skill profile | target 存在时同目标 | self | cooldown/qi/meridian/cast state | 由 R9 定 cast target；R4 承载通用 gate |
| 69 | `SkillBarBind` | config / — | — | self | slot/skill/item 已解锁 | 域内 |
| 70 | `SkillConfigIntent` | config / — | — | self | skill 已解锁、config schema 合法 | 域内 |
| 71 | `CombatReincarnate` | — | — | self | death screen/state transition 合法 | 显式 no spatial gate |
| 72 | `CombatTerminate` | — | — | self | death state 可终结 | 显式 no spatial gate |
| 73 | `CombatCreateNewCharacter` | — | — | self | terminated/new-character transition | 显式 no spatial gate |
| 74 | `StartExtractRequest` | portal entity / profile-preserved | 同目标 | — | portal active、玩家可撤离、非忙态 | 域内需登记 |
| 75 | `CancelExtractRequest` | session / — | — | session owner | extract active | R1 session gate |
| 76 | `StartSearch` | container entity / profile-preserved | 同目标 | loot authority | container searchable、非忙态 | 域内需登记 |
| 77 | `CancelSearch` | session / — | — | session owner | search active | R1 session gate |
| 78 | `SupplyCoffinOpen` | entity / `SupplyCoffinOpen` | 同目标 | — | coffin active/unopened | 已有 authority helper |
| 79 | `ContainerOpen` | entity / profile-preserved | 同目标 | access authority | container active | 域内需登记 |
| 80 | `WorkbenchOpen` | entity / `Workbench` | 同目标 | — | workbench active | 距离已有；同维缺 |
| 81 | `ExternalContainerMove` | session world target / `ExternalSession` | 同目标 | session owner | revision/source/destination 合法 | session authority 已有；通用容器缺 target dimension/reach 复验，迁 R1/R10 adapter |
| 82 | `ExternalContainerClose` | session / — | — | session owner | session active | 已有 owner；迁 R1 adapter |
| 83 | `LingtianStartTill` | block / `NearbyInteract` | 同目标 | hoe owner | terrain/mode/非忙态 | 距离/维度缺 |
| 84 | `LingtianStartRenew` | block / `NearbyInteract` | 同目标 | hoe owner | plot 可翻新 | 距离/维度缺 |
| 85 | `LingtianStartPlanting` | block / `NearbyInteract` | 同目标 | seed owner | plot/plant/非忙态 | 距离/维度缺 |
| 86 | `LingtianStartHarvest` | block / `NearbyInteract` | 同目标 | plot access | crop ripe/mode/非忙态 | 距离/维度缺 |
| 87 | `LingtianStartReplenish` | block / `NearbyInteract` | 同目标 | source owner | plot/source/ledger/非忙态 | 距离/维度缺 |
| 88 | `LingtianStartDrainQi` | block / `NearbyInteract` | 同目标 | plot access | drain 条件/非忙态 | 距离/维度缺 |
| 89 | `ForgeStartSession` | station / `NearbyInteract` | 同目标 | station/session owner + materials owner | blueprint/material/非忙态 | 距离/维度缺 |
| 90 | `ForgeTemperingHit` | session / `NearbyInteract` | station dimension | session owner | phase/timing 合法 | 距离/维度缺 |
| 91 | `ForgeInscriptionScroll` | session / `NearbyInteract` | station dimension | session + scroll owner | phase/scroll 合法 | 距离/维度缺 |
| 92 | `ForgeConsecrationInject` | session / `NearbyInteract` | station dimension | session owner | phase/qi/ledger 合法 | 距离/维度缺 |
| 93 | `ForgeStepAdvance` | session / `NearbyInteract` | station dimension | session owner | current step complete | 距离/维度缺 |
| 94 | `ForgeBlueprintTurnPage` | — | — | self | blueprint book 可用 | 显式 no spatial gate |
| 95 | `ForgeLearnBlueprint` | inventory / — | — | scroll/material owner | blueprint/unlock 合法 | 域内；R10 transaction |
| 96 | `ForgeStationPlace` | block / `NearbyInteract` | 主世界/同目标 | item owner | tier/位置可放置 | 距离/维度缺 |
| 97 | `CraftStart` | recipe station / recipe profile | station 存在时同目标 | material owner | unlock/material/qi/非忙态 | station 规则域内；接 R1/R10 |
| 98 | `CraftCancel` | session / — | — | session owner | craft active | R1 session gate |
| 99 | `GiveDanToElder` | entity / `NearbyInteract` | 同目标 | pill owner | DyingElder + Plea/Recovering、存活 | 目标/状态/距离/维度须在扣丹前 |
| 100 | `RaiseShield` | equipped / — | — | shield owner | 存活、off-hand shield、非冲突态 | 域内 |
| 101 | `LowerShield` | — | — | self | blocking active；幂等退出允许 | 域内 |
| 102 | `ScrollReadRequest` | inventory / — | — | item owner | readable spec、非冲突态 | owner/spec 域内 |
| 103 | `ScrollReadClosed` | session / — | — | reader self | read session active；幂等关闭允许 | P2 接 R1 session |
| 104 | `AgentUiResponse` | request / — | — | authenticated player / session request owner | request_id/action/button 未过期且获准 | 域内；AgentUiSessionStore 按 player entity 校验 request_id；显式 no spatial gate |

P1 测试必须从 TypeBox `agent/packages/schema/src/client-request.ts::ClientRequestV1` IPC source of truth 导出/对拍，并同时校验 Rust serde mirror、Markdown matrix、生成的 `agent/packages/schema/generated/client-request-v1.json` 与未来 gate registry；不把当前变体数写成永恒常数。当前 TypeBox/generated mirror 只覆盖 Rust enum 的 **86/104** 个变体，以下 **18** 个 wire gap 由 R6 generation machinery 负责生成链与 transport 接缝，但每个 domain 的 TypeBox declaration content 仍由对应 domain owner 定义。已由 Java client 与 Rust dispatcher 实际连通、必须纳入 P1 production gate 的 gap 为 `give_dan_to_elder`、`lingtian_start_till`、`craft_start`、`workbench_open`、`external_container_move`；它们暂缺 TypeBox/generated mirror 只影响 schema 对拍，不得被误判为没有 production producer/consumer。其余尚无 production producer/consumer 的 gap，以及 `coffin_break`、`coffin_menu_reclaim` 的 authenticated owner persistence/hydration、owner-proof wire 与 reject contract，仍属于 tracked owner-plan amendment follow-up。按总纲 §3（Wave 表为 inter-track ordering/start/cutover 唯一 authority）与 §4.1 第 3、5 条，R4 不修改 R6 独占的 TypeBox/generated wire 文件；上游 artifact 尚未就绪时，R4 P1 只落对应 `GateSpec`/adapter contract-first stub（declared、unwired、test-only），不得接 production 或以临时 `NoGate` 宣称完成；但已连通的五个 gap 必须在本 P1 接入 production gate，不能以 TypeBox 缺口推迟门禁。真实上游 artifact 是这些变体的 schema/transport production cutover dependency，不是 R4 P1 start gate。每个 `NoGateReason` 仍须非空。P0 的仓内静态对拍由 CI 强制执行的 `agent/packages/schema` `npm run check` 与 `python3 scripts/check_c2s_gate_matrix.py` 提供：前者编译 TypeBox source 并拒绝 committed generated artifacts 过期，后者读取生成 JSON 的 `type` discriminants，比较 Rust `ClientRequestV1`、Markdown matrix 与 TypeBox/generated mirror 的集合；Rust-only 的上述 18 个已登记 gap 被显式允许，但已解决的 gap 若出现在 schema 会使 documented baseline freshness 检查失败，任何新增缺口、schema-only 变体、重复 discriminant 或生成 JSON 畸形均 fail closed。矩阵表内畸形/额外行以及 unit/tuple/struct 以外的未知顶层 enum 语法也必须 fail closed。

## 吸收清单验真（2026-08-03）

以下结论只决定 R4 implementation owner，不修改或归档其它 plan。文件状态按当前 `origin/main` 核对；无 `-v1` 或无 `plan-bughunt-` 前缀的真实路径按实际文件记录，不再假报 missing。

### A. R4 完整吸收（空间/ownership/state gate 是根修复）

| plan | 当前状态 | P0 验真结论 / R4 落点 |
|---|---|---|
| `duoshe-scope-gate` | active | REAL；`DuoSheRequest` 当前直接 emit，缺 target resolve 前的同维/6 格/目标状态门。矩阵 #59。 |
| `dying-elder-give-dan-server-gate` | active | REAL；目标类型/状态/同维/距离必须早于扣丹。矩阵 #99。 |
| `workbench-cross-dimension-open` | active | REAL；`WorkbenchOpen` 距离已有但目标实体/玩家维度未成为统一门。矩阵 #80，并复用同维 helper 给 station recipe。 |
| `zhenfa-place-scope-gate` | active | REAL；裸坐标向主世界写阵，空间门缺。矩阵 #40。 |
| `alchemy-furnace-scope-gate` | skeleton（真实文件无 `-v1`） | REAL；open/feed/take/ignite/intervention/place 六条统一收进 #11-15/#20。 |
| `block-place-reach-gate` | skeleton | REAL；通用 `BlockPlace` 下游校验未包含 player reach。矩阵 #23。 |
| `coffin-reclaim-owner-gate` | skeleton | REAL；现有 `CoffinEntity` 无 owner 数据；R3 P1/P3/P4 与 R6 P1/P2/P3 仅提供通用 Slice、flush/tick-rebase、generation、bridge 与 atomic-activation 接缝，coffin-specific authenticated owner persistence/hydration/owner-proof wire 尚未由 owner plan 定义，标为 tracked follow-up；R4 只落 contract-first stub（declared/unwired/test-only），owner-plan amendment 后再接入 authenticated owner authority 并原子迁移 `insert`/`reclaim_occupied`/重连恢复，对 break/reclaim 执行 owner-only；offline username 派生的 `Lifecycle.character_id` 不构成认证。矩阵 #27/#28。 |
| `combat-pill-toxin-gate` | skeleton | REAL；`AlchemyTakePill` 的毒性状态前置必须进入 mutation barrier。矩阵 #19。 |
| `forge-station-place-gate` | skeleton | REAL；放砧缺距离/维度，矩阵 #96。 |
| `lingtian-c2s-range-gate` | skeleton | REAL；六类 plot 请求缺空间门，矩阵 #83-#88。 |
| `dropped-loot-cross-dimension-pickup` | skeleton | REAL；server 权威拾取补 entry dimension 与 requester dimension 比对，矩阵 #53；S2C/client 显示字段归 R6。 |
| `player-trade-cross-dimension` | skeleton | REAL；`social/mod.rs:1020-1175` 发起/接受仅比较 `Position` 距离，两次都必须复验同维。矩阵 #35/#36。 |
| `tsy-spirit-niche-dimension-gate` | skeleton（真实文件无 `-v1`） | REAL；主世界限定与“先验后扣”归矩阵 #29。 |
| `forge-session-range-dimension-gate` | skeleton（旧描述称在飞 #1294，当前未闭环） | REAL；起炉至推进共五条 session 操作需绑定 station dimension/reach。矩阵 #89-#93。 |

### B. 已闭环，仅作为迁移回归样本

| plan | 当前状态 | P0 验真结论 / R4 落点 |
|---|---|---|
| `coffin-dimension-gate` | finished | 已由 `coffin_requires_overworld` + 距离测试闭环；P1/P3 迁入 spec 时保持 fail-closed 和边界行为，不重复修复或重复归档。 |

### C. 部分吸收：R4 只拥有 server gate/helper 切片

| plan | 当前状态 | R4 切片 | 非 R4 owner |
|---|---|---|---|
| `weapon-repair-station-bypass` | active（正文仍自称 Skeleton，文档状态漂移） | #52 的 weapon owner + station 同维/reach + state-before-mutation | station 材料经济/transaction 由领域实现与 R10；R4 不拍新成本 |
| `dropped-loot-g-pickup-range-desync` | skeleton | #53 保持 server 2.5 格 authority，并导出单一命名 profile | client G 候选与范围下发属于 R6/R7 接缝 |
| `workbench-cross-dimension-break` | skeleton | 提供统一 same-dimension/reach helper，WorkbenchOpen/recipe 采用 | `DiggingEvent` 不是 `bong:client_request`，break consumer 仍须在 workbench 域修复 |
| `zone-lookup-overworld-hardcode` | skeleton | 统一 `GateContext`/helper 强制调用 `ZoneRegistry::find_zone(actual_dim, pos)` | HUD/agent/TSY attrition 非 C2S 调用点由各自领域修，不能因 helper 存在宣称完成 |
| `world-social-cross-dimension-witness-leak` | skeleton（无 `plan-bughunt-` 前缀） | 共享 same-dimension predicate | witness 收集与 social zone 写入不是 C2S handler，仍由 social owner 修 |
| `voidaction-target-zone-lock` | skeleton（无 `plan-bughunt-` 前缀） | #4 server 校验 action 对当前 zone 的合法性，拒绝不启动 server cooldown | client `VoidActionStore` 默认 spawn 与本地伪冷却由 client/R7-R9 相关 owner 修 |
| `npc-trade-gate-desync` | skeleton（无 `plan-bughunt-` 前缀） | #39 抽出单一 server `can_trade` state gate | metadata/UI 展示同步属 R6/R7；未同步前不得归档 |

### D. 不吸收：不是通用 C2S authority 根因

| plan | 当前状态 | P0 验真结论 / 去向 |
|---|---|---|
| `disciple-trade-gate-drift` | skeleton | REAL，但根因是 NPC catalog/archetype 白名单与真实库存漂移，不是距离/维度/ownership。由 NPC trade 领域独立修；R4 只消费其最终 `can_trade` API。 |
| `player-trade-npc-gate` | skeleton | REAL，但 `social::dispatch_trade_offers` 错把 `npc_should_decline_trade` 用于玩家交易，是业务谓词误接。由 social 领域独立修；不伪装成 GateSpec state。 |

验真总账：原清单 24 项中，**14 完整吸收 + 1 已闭环回归样本 + 7 部分吸收 + 2 移出**。P4 只可归档被 R4 与联动 owner 真正完整修复且证据齐全的 plan；部分吸收项在另一切片未合入前不得提前归档。

## 阶段交付物

- ✅ 2026-08-03 **P0 设计收口 + 吸收清单验真**：冻结穷尽 `RequestGate`、metric-aware 命名距离 profile、visibility-first fail-closed 顺序、authenticated coffin owner authority、内部 denial reason/安全外部折叠、每客户端 ingress/feedback/log 预算，以及 104 变体 Rust enum ↔ Markdown matrix 的 P0 inventory；逐项登记现行变体，完成 24 项吸收清单验真。P0 只新增 docs、吸收清单证据和 enum/matrix 静态检查器，不改运行时 gate、wire handler、TypeBox source 或 generated JSON Schema。TypeBox/generated 当前 86/104 的镜像覆盖及 18 个 gap 归 R6；其中 `give_dan_to_elder`、`lingtian_start_till`、`craft_start`、`workbench_open`、`external_container_move` 已存在 client producer 与 server consumer，P1 不得因 schema gap 将其排除在 production gate 外；R4 对其余缺少上游 artifact 的消费点按 contract-first 交付，真实 wire 仅约束对应 production cutover。
  - **文件 / symbol 抓手**：`docs/plan-refactor-c2s-gate-v1.md`；`scripts/check_c2s_gate_matrix.py`；Rust mirror `server/src/schema/client_request.rs::ClientRequestV1`；Markdown 104-row matrix；未来 `network/gate::{GateSpec,RequestGate,GateDenialReason}`。TypeBox authority `agent/packages/schema/src/client-request.ts::ClientRequestV1` 与生成产物 `agent/packages/schema/generated/client-request-v1.json` 在 P0 仅作为 P1 gap inventory 的证据，不由本 PR 修改。
  - **测试声明**：独立轻量 CI workflow `.github/workflows/c2s-gate-matrix.yml` 强制安装 agent workspace 依赖并在 `agent/packages/schema` 运行 `npm run check`（TypeBox source 编译 + `generate:check` freshness），再运行 `python3 scripts/check_c2s_gate_matrix.py` 与 checker 单测；`push.paths` 与 `pull_request.paths` 必须同时覆盖 Rust `server/src/schema/client_request.rs`、权威 TypeBox producer inputs 的显式 glob allowlist `agent/packages/schema/src/**`、提交的 generated producer artifacts 的显式 glob allowlist `agent/packages/schema/generated/**`、schema package build/check inputs `agent/packages/schema/package.json` 与 `agent/packages/schema/tsconfig.json`、workspace dependency resolution `agent/package-lock.json`、本 plan、checker、测试和 workflow 本身，因此任一 imported schema module、TypeBox union、committed generated schema、check script、compiler configuration 或 workspace dependency resolution 的增删/重排/字段变化都会触发同一 C2S contract job；checker 解析 struct/unit/tuple Rust variants、拒绝未知顶层语法及畸形矩阵行，并读取 generated `type` discriminants，断言 Rust enum、Markdown matrix 与 TypeBox/generated mirror 的集合一致（除 plan 已登记的 18 个 Rust-only gap）、重复项和连续编号；documented gap 出现在 generated schema 时 baseline freshness 失败；`scripts/tests/check_c2s_gate_matrix_test.py` 覆盖 parser 边界、schema drift、documented gap baseline 以及 enum/matrix 漂移、重复和编号测试。
  - **依赖证据**：总纲 §3 Wave 表是唯一 ordering authority；R4 implementation 属 Wave 2，各 production activation 仅等待对应 R5/R6/R2 owner responsibility 按各自 plan 就绪并服从 §4.1 atomicity invariants；缺失 artifact 只落 contract-first stub，不得把 production dependency 反写成整轨 start gate。
- ⬜ **P1 全量门禁声明 + 中间件原子上线 + 已知漏洞簇首批接入**：按总纲 §3 Wave 2，在 R5、R6、R2 的对应 owner responsibility 按各自 plan 就绪并服从 §4.1 ownership/atomicity invariants 后启用相应 production 切片；contract-first 工作不等待 production activation 条件。在同一 PR 落 `server/src/network/gate/`、为届时全部 `ClientRequestV1` 变体穷尽声明 `Spec`/`NoGate(reason)`、补齐已具备 authority/state artifact 的 adapter，并把已获 Wave 放行的 production mutation barrier 接到 decode/version 后。已存在 client producer 与 Rust dispatcher consumer 的 `give_dan_to_elder`、`lingtian_start_till`、`craft_start`、`workbench_open`、`external_container_move` 五个 TypeBox gap 必须在本 P1 接入 production gate，不能因尚未进入 TypeBox/generated mirror 而留在门禁外；其余尚缺 upstream artifact 的 gap 与 coffin owner-proof contract 只落 declared/unwired/test-only contract-first stub，不接 production；coffin-specific persistence/hydration/owner-proof wire 需先由 R3/R6（及 domain owner）owner-plan amendment 定义 phase/artifact/consumer，再在对应 cutover merge unit 启用。A 组 14 项及 B 组迁移样本在该全量 fail-closed 基础上完成根修复；禁止用临时 `NoGate` 绕过矩阵中要求门禁的变体，禁止让 stub 进入 dispatcher。
  - **测试抓手**：enum↔matrix↔registry 三方穷尽；每个 denial reason 正反/缺组件/fail-closed；每个 enum 变体专属 pin；所有 `NoGateReason` 非空；欧氏边界与 Workbench `Chebyshev3d` 对角 `[3,3,3]` 放行、任一轴 `3+ε` 拒绝；coffin owner-proof contract-first owner-only/同名 offline 无 owner proof 拒绝/缺失或错误 owner proof fail-closed；R3/R6 owner-plan amendment 与对应 artifact 合入后再验已重新认证 owner 重连、进程重启 hydration、缺 owner；forge 同 update advance→hit 放行且 hit→advance 拒绝；拒绝前 inventory/qi/world/session 零 mutation；EventAlert 只单播请求者。
  - **资源与安全抓手**：单客户端 burst/flood 证明每 tick gate/handler 工作 ≤32、20 ticks 内 EventAlert ≤1、100 ticks 内同 key 日志 ≤1 且 suppressed count 汇总；断线清理 budget；对不存在/跨维/超距/他人目标的探测断言外部 packet、文案和时序不可区分。
- ⏳ 2026-08-30 **P2 巨石拆分批次 A**：已完成 inventory 子批次：新增 `server/src/network/client_request/inventory.rs` 的 `InventoryRequest`、`try_into_inventory_request`、`dispatch_inventory_request`，穷举 `InventoryMoveIntent`、`InventoryDiscardItem`、`PickupDroppedItem`、`ContainerOpen`、`ExternalContainerMove`、`ExternalContainerClose`；顶层 route 位于 Forge typed route 后、session route 前，session 移除后三个容器变体，旧 inventory/session helper 继续承载校验、日志与 mutation。`server/tests/inventory_request_dispatch.rs` pin 六变体字段保真与非目标回退；资源缺失仍沿用 helper fail-closed/no-op 语义。其余域批次另行推进。
  - **测试抓手**：移动前后 payload→域 handler 行为对拍；不修改 R1/R6/R10 独占文件；受影响 server 完整 gate。
  - ✅ 2026-08-27 **R4 P2-A3b Forge typed handler 拆分**：新增 `server/src/network/client_request/forge.rs` 并在 `server/src/network/client_request/mod.rs` 注册；`ForgeRequest`、`try_into_forge_request`、`dispatch_forge_request` 编译期闭合接收并分发 `ForgeStationPlace`、`ForgeInscriptionScroll`、`ForgeTemperingHit`、`ForgeConsecrationInject`、`ForgeStepAdvance`、`ForgeLearnBlueprint`、`ForgeStartSession`、`ForgeBlueprintTurnPage` 八个变体；顶层 `server/src/network/client_request_handler.rs` 仅保留 typed route，原 helper/事件/校验/日志/反馈复用且 `pending_forge_steps` 仍限定于单次 batch invocation。外置 `server/tests/forge_request_dispatch.rs` 锁定八变体字段保真、非 Forge 原样返回及 `StepAdvance` 后三个依赖请求的 projected-state 行为；未修改 schema、wire、R1 session、R10 transaction 或 Forge 业务规则。关键 commit：`a0cb6819c86542c5cdd52c4e783953e9a3703de8`（2026-08-27）。验证：Rust 完整门禁退出码 0（主库 12,780 passed/2 ignored，main 18 passed，外置 Forge 4 passed，其他集成与 doc tests 无失败）；fresh read-only validator（`gpt-5.6-luna`）对同一 SHA PASS。P2 其余域尚未完成，阶段总状态保持 ⬜。
- ⬜ **P3 删除重复门禁 + adapter 收敛**：在 P1 已全量 fail-closed 的前提下，逐域删除重复距离常数和重复维度/owner 判定；领域特有状态通过 `StateGateId` adapter 保留单一事实源，不再承担任何枚举声明补洞。
  - ✅ 2026-08-30 **P3-A1 制作台 reach profile 单一事实源收敛**：`server/src/craft/workbench.rs::is_within_workbench_range` 改为 `DistanceRule::WORKBENCH` 薄 adapter，`WORKBENCH_INTERACT_RANGE` 仅作为 `WORKBENCH_MAX_BLOCKS` 兼容别名；`handle_workbench_interact` 与 `network/craft_emit.rs` 继续通过旧 helper 保持原有制作台存在性、生命周期与 recipe reach 语义。外置 `server/tests/workbench_reach_profile.rs` 对拍 origin、轴向/对角边界、边界外、负坐标与非有限坐标 fail-closed。本切片不改 gate 算法/数值、wire/schema、inventory transaction 或 craft 业务规则；P3 其余域仍未收敛，阶段状态保持未完成。
  - ✅ 2026-08-31 **P3-A2 矿脉探测 reach profile 单一事实源收敛**：`server/src/mineral/probe.rs::is_probe_target_in_range` 改为仅调用独立共享 `server/src/reach.rs::DistanceRule::NEARBY_INTERACT`（`Euclidean3dSquared`、6.0 格、inclusive），`server/src/network/gate/mod.rs` 仅重导出该策略并保留 gate context 适配，避免 mineral→network 分层依赖；方块中心换算与既有公开半径兼容别名保持不变。新增 `server/tests/mineral_probe_reach_profile.rs` 覆盖贴脸、方块中心、6.0 精确边界、超出一 ULP、负坐标及 NaN/正负无穷 fail-closed。验证：`flock /tmp/bong-cargo.lock -c 'cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test'` 通过（主库 12,734 passed/2 ignored，main 18，外置集成测试全部通过）；本切片不改业务/schema/wire/Redis/inventory/qi 或其它 gate。
  - ✅ 2026-09-01 **P3-A3 物资棺 reach profile 单一事实源收敛**：`server/src/supply_coffin/authority.rs::{authorize_supply_coffin_open,authorize_supply_coffin_session}` 改为分别调用 `crate::reach::DistanceRule::{SUPPLY_COFFIN_OPEN,EXTERNAL_SESSION}`，保留 `MissingSource → MissingPlayerDimension → DimensionMismatch → OutOfRange` 顺序及实际欧氏距离返回语义；旧 `SUPPLY_COFFIN_*_MAX_DISTANCE` 仅作为 `reach` 共享半径别名。新增 `server/tests/supply_coffin_authority_reach_profile.rs`，通过现有 open/lifecycle API 对拍 4.5/6.5 欧氏 inclusive 轴向及对角线边界、超出一 ULP、负坐标、NaN/正负无穷、缺 source/维度与同 XYZ 跨维拒绝，并验证旧调用链无额外副作用。本切片仅改门禁 adapter 与 reach contract 测试，不改 gameplay 数值、schema、wire、Redis、inventory、qi 或其它 gate。
  - ✅ 2026-09-01 **P3-A4 延寿棺 reach profile 单一事实源收敛**：`server/src/coffin/mod.rs::is_coffin_target_in_range` 作为放置/进棺/破坏/菜单回收四条链路的薄 adapter，统一调用 `crate::reach::DistanceRule::NEARBY_INTERACT`（`Euclidean3dSquared`、6.0 格、inclusive），移除 `COFFIN_INTERACT_MAX_DISTANCE_SQ` 重复平方距离常量；新增 `server/tests/coffin_reach_profile.rs` 对拍目标中心/贴脸/精确边界/超出一 ULP/欧氏对角线/负坐标/NaN 与正负无穷 fail-closed，既有 ECS 入口继续验证拒绝前无副作用。本切片不改 coffin gameplay、owner、维度门、wire/schema、inventory、qi 或其它 gate。验证：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test` 全部通过（lib 12693、main 18、外置集成与 doc tests 0 失败；目标 reach profile 4/4）。
  - ✅ 2026-09-01 **P3-A4 NPC interaction reach profile 单一事实源收敛**：`server/src/network/client_request/npc.rs::resolve_npc_engagement_target` 保留既有 entity-id resolution、同维检查与 `Terminated` fail-closed 顺序，改用 `crate::reach::DistanceRule::nearby_interact()` 的 `Euclidean3dSquared`、6.0 格 inclusive predicate，删除 `NPC_INTERACTION_MAX_DISTANCE` 与本地平方距离比较。新增 `server/tests/npc_interaction_reach_profile.rs` 的 5 条 contract test，覆盖 profile identity、精确边界、超出一 ULP、欧氏对角线、负坐标、请求者/目标 NaN 与正负无穷，以及 resolver source wiring；不改 gate primitive、schema/proto、Redis、inventory、qi 或其它 gate。验证：`flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo test --test npc_interaction_reach_profile'` 与 server 完整 fmt/clippy/test gate 均通过。
  - ✅ 2026-09-02 **P3-A5 QiColorInspect reach profile 单一事实源收敛**：`server/src/network/client_request_handler.rs::is_qi_color_inspect_position_in_scope` 统一调用 `crate::reach::DistanceRule::NEARBY_INTERACT`，移除该路径的 `QI_COLOR_INSPECT_MAX_DISTANCE` 重复常数；保留 entity 解析、self-target 拒绝、位置解析、同维检查及 fail-closed 顺序。新增 `server/tests/qi_color_inspect_reach_profile.rs`，覆盖共享 profile identity/source wiring、6.0 格精确 inclusive 边界、超出一 ULP、欧氏对角线边界、负坐标、NaN/正负无穷 fail-closed，以及拒绝前不发送 `QiColorInspectRequest`；inline handler contract test 另覆盖 self-target、跨维、malformed entity id 与无 ECS 副作用。关键 commit：`12193eb300495ff16fc7ed6b8569f66eecb15f04`（2026-09-02）。验证：目标 contract test 6 passed；server 完整 fmt/clippy/test gate 通过（lib 12,680 passed/2 ignored，main 18 passed，外置集成与 doc tests 无失败）。本切片不改 gameplay 数值、schema、wire、Redis、inventory、qi 或其它 C2S handler/gate；P3 阶段仍未完成。
  - ✅ 2026-09-04 **P3-A7 灵田交互 reach profile 单一事实源收敛**：新增 `server/src/reach.rs::DistanceRule::{lingtian_interact, LINGTIAN_INTERACT}` 与独立 `LINGTIAN_INTERACT_MAX_BLOCKS = 4.5`，`server/src/lingtian/range_gate.rs::is_lingtian_position_in_scope` 保留 Overworld 维度门和方块中心换算后改为共享 profile 薄 adapter，删除无外部引用的本地距离/容差常量；既有 `DENIAL_LOGS` test seam 保持不动。沿用既有 `server/tests/unit/lingtian/range_gate_test.rs` / `range_gate_unit` 扩充 15 条合同测试，覆盖 profile identity/source wiring、4.5 格轴向 inclusive 与超出一 ULP、欧氏对角线、方块中心、负坐标、非有限坐标、跨维拒绝及拒绝前无 ECS mutation。一次性临时 Rust property probe 在 Minecraft 水平坐标 ±30,000,000、Y [-64,320] 的确定性边界样本中枚举比较旧 `distance()` 与新平方 profile，共 2,147,474 个样本发现 1 个 ULP 级分歧；实际 `DistanceRule::LINGTIAN_INTERACT` witness 为旧表达式放行、新表达式拒绝，结论是统一平方 metric 带来的边界差异，不调整 4.5 数值。本切片不改灵田业务规则、schema、wire、Redis、inventory、qi 或其它 gate；P3 阶段仍未完成。
  - ✅ 2026-09-04 **P3-A6 give_dan reach profile 单一事实源收敛**：`server/src/network/client_request_handler.rs::is_give_dan_target_in_scope` 保留同维检查先于距离检查的拒绝顺序，改用 `crate::reach::DistanceRule::NEARBY_INTERACT`（`Euclidean3dSquared`、6.0 格、inclusive），删除本地 `GIVE_DAN_MAX_DISTANCE` 重复常量；既有给丹状态/目标解析与扣丹前拒绝语义不变。新增 `server/tests/give_dan_reach_profile.rs` 6 条 contract test，覆盖 profile identity/source wiring、精确边界与一 ULP 外拒绝、欧氏对角线、负坐标、NaN/正负无穷 fail-closed，以及拒绝先于 intent 发出。关键 commit：`77404ecb645e406d9864cacc04c03d7ee1daa823`（2026-09-04）。验证：目标 contract test 6 passed；fresh read-only validator 对该 SHA PASS；server 完整 fmt/clippy/test gate 通过（lib 12,635 passed/2 ignored，main 18 passed，所有外置集成与 doc tests 无失败）。本切片仅收敛 reach adapter 与 contract 测试，不改 gameplay 数值、schema、wire、Redis、inventory、qi 或其它 C2S handler/gate；P3 阶段仍未完成。
  - **测试抓手**：enum↔registry 穷尽编译门继续常绿；旧 helper 与 spec 接受区域迁移对拍；删除重复判断后每个变体专属 pin 仍覆盖合法与拒绝路径；新增变体自动纳入。
- ⬜ **P4 bot 验收 + 吸收 plan 结案**：落 `gate_cross_dimension`、`gate_reach`、`gate_ownership`、`gate_state_precondition`、`gate_matrix_sweep`；按总纲 §7 只归档完全闭环项。
  - **测试抓手**：`scripts/bot/scenarios/gate_*.py`；server `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`；真实 `bong:client_request` 黑盒拒绝与合法放行。

## 文件所有权与依赖

- **R4 独占**：`server/src/network/client_request_handler.rs` 的拆解、新 `server/src/network/gate/`、迁移后各域重复 reach/dimension/ownership 判定行。
- **不碰**：R6 的 `network/*_emit.rs`、`schema/proto_convert.rs` 与 client channel/router；R1 session 内部；R10 inventory transaction；业务领域不属于 gate 的规则表。
- **依赖门**：本节同步已合入 `origin/main` 的 PR #1902 settled ruling：跨轨 start/order/cutover 以总纲 §3 Wave 表为唯一 authority；R4 production activation 属 Wave 2，R5、R6、R2 的所属责任按各自 plan 就绪后服从总纲 §4.1 ownership/atomicity invariants，不再声明 `#1287 + R6 P1` 整轨 start gate。upstream wire/transport artifact 尚未就绪时，消费点只允许 contract-first stub（declared、unwired、test-only），真实 artifact 是对应 production cutover dependency，不得反写成 start gate。R6 若尚未提供结构化 reject，已获 Wave 放行的 gate 可使用 EventAlert 临时反馈，但不得宣称结构化 ack 已完成。
- **跨轨接缝**：R1 暴露 session authority adapter；R3 当前只暴露通用 Slice framework（P1）、shutdown flush/tick-rebase（P3）与 runtime persistence extension points（P4）；coffin-specific authenticated owner authority slice、migration、hydration 及其最终路径是 R3 owner-plan amendment tracked follow-up。R10 暴露 inventory ownership/transaction preflight；R6 当前只拥有 schema generation chain（P1）、client bridge/consumer contract（P2）与 atomic production activation machinery（P3），coffin owner-proof wire/sample 与结构化 reject wire 仍需 R6/对应 domain owner amendment tracked follow-up。R9 拥有 skill cast target/cooldown contract。R4 只消费已冻结 API；在 owner-plan amendment 与 artifact ready 前，棺材接缝保持 declared/unwired/test-only，不接 production，并遵循总纲 §4.1 第 3、5 条 contract-first/production-cutover 裁决。已连通的五个 TypeBox gap 请求不适用该延期：其 client producer、server dispatcher 与 gate adapter 必须在 P1 同一 production barrier 内闭环。

## bot 验收场景

1. `gate_cross_dimension`：TSY bot 对主世界 workbench/zhenfa/coffin/trade/loot 请求，全部拒绝且无副作用；同维合法请求放行。
2. `gate_reach`：block/alchemy/forge/lingtian/elder/duoshe 在边界、边界外和贴脸三档；断言命名 profile 的 metric + 半径，并锁住 Workbench `[3,3,3]` Chebyshev 对角边界。
3. `gate_ownership`：他人 coffin/container/inventory/session/item 请求拒绝，请求者和 owner 状态均不变；同 offline username 的连接不得取得 coffin owner 权限，offline transport 不携带可重放 bearer secret，只有通过未来冻结的 authenticated owner authority 重新证明 owner 的连接可操作，缺 owner/authority 恢复记录 fail-closed。
4. `gate_state_precondition`：给丹、服丹、forge/craft/session 在 invalid state 下拒绝且物品/qi/session/world 零 mutation。
5. `gate_matrix_sweep`：从现行 enum/spec registry 枚举全部变体；每个变体至少命中合法路径或明确 no-gate pin，并对可构造空间目标跑同维合法/超距/跨维/缺 authority context。
6. `gate_denial_flood`：单 client 同 tick burst 与持续 flood；断言 gate/handler、alert、日志均受预算上限约束，合并计数正确，另一 client 不受影响。
7. `gate_target_oracle`：用不存在、跨维、超距和他人目标 ID 探测，断言请求者看到的 payload/文案/时序完全相同，内部指标仍保留真实 reason。

## §10 实施工作流

### §10.1 适用边界

本 plan 是纯 server/network 逻辑重构，不产出 NBT、worldgen layout、模型或贴图，不适用视觉资产三轮与 `<PROMISE>`。每个逻辑单元使用中文 atomic commit，并带真实 `Model:` trailer。

### §10.2 PR 顺序

1. **PR-1 / P0**：本次 docs + 静态 matrix checker 的设计收口与吸收验真。
2. **PR-2 / P1**：按总纲 §3 Wave 2，在 R5、R6、R2 的对应 owner responsibility 就绪并服从 §4.1 ownership/atomicity invariants 后启用相应 production gate；全量 spec/no_gate、已有 authority adapter 与资源预算可先按 contract-first 交付，缺 upstream artifact 的消费点只落 declared/unwired/test-only stub，待对应 cutover merge unit 启用。
3. **PR-3 / P2**：拆 handler 批次 A，行为不变。
4. **PR-4 / P3**：删重复门禁并收敛领域 adapter，不补枚举声明欠账。
5. **PR-5 / P4**：bot/e2e 与符合条件的吸收项结案。

前一阶段 merge 且新 `origin/main` 门禁全绿前，不实施下一阶段。

### §10.3 每个 implementation PR 的闭环门

1. `git fetch origin` 后紧邻合并最新主线；merge 带入变更后重跑完整验证。
2. server 命令必须通过全局 cargo 锁执行：`flock /tmp/bong-cargo.lock -c 'cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test'`。
3. 禁止运行 `scripts/test-tmux-shutdown-order.sh` 和 `scripts/test-server-lifecycle.sh`；本轨不需要它们。
4. fresh-context validator 必须对最终 HEAD SHA 出结论；HEAD 改变则旧证据失效。
5. push 后确认 PR head 等于已验证 SHA；review/e2e 返工产生新 HEAD 时重跑全部门禁。

### §10.4 单次 consume-plan 终态

后续消费按 PR-2 至 PR-5 串行闭环；终态要求全部阶段 `✅ YYYY-MM-DD`、104 基线已由当时实际 enum 数量替换/核验、完全吸收项具备实现与 bot 证据、`## Finish Evidence` 完整后才迁入 `docs/finished_plans/`。部分吸收或移出项不得因 R4 归档而虚报完成。
