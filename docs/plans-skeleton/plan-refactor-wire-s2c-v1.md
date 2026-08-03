# plan-refactor-wire-s2c-v1 — S2C Wire 层统一：emit builder + client 双轨归一 + 作用域广播（重构轨 R6）

> 所属总纲：`plan-refactor-master-v1.md`。一句话：server 侧 ~100 个 `*_emit.rs` 收敛到共享 emit builder（带维度/zone 作用域），client 侧把 28 个散装 CustomPayload 旁路归入 `ServerDataRouter` 单轨、枚举前缀剥离收敛到桥接层一处——契约漂移与跨维串场两簇从结构上封死。

## 现状证据（2026-07-27 侦察）

- server：144 个 `ServerDataType` 变体 × ~100 个独立 emit 文件，各自"读 ECS→建 DTO→序列化→EventWriter"无共享 builder；`schema/proto_gen.rs` 12423 行（生成）+ `proto_convert.rs` 8419 行（手写）并存，漏改一处即 drift。
- client：主干 `ServerDataRouter`（~90 handler 手写注册，`ServerDataRouter.java:111-289`）之外，`BongNetworkHandler.register()` 还有 ~28 个独立 channel 各写各的 receiver+parse（npc metadata/bubble、vfx_event、audio play/stop、agent_ui、era_ambiance 等）。
- `ProtoServerDataBridge.java`（1547 行）大 switch 手转 JSON，proto3 枚举前缀在 cast_sync/movement_state/container_state 等处各剥一遍（`:846-999`）——#1294 的 forge-session-enum-unstripped 就是又漏了一处的实证。
- 广播作用域缺失：vfx/audio 跨维 bleed、跨位面 env 不重发（q-world-season-dimension-env-resync）。

## 接入面

- **进料**：各域游戏事件（emit 调用点）、`world` 维度/zone 信息（作用域过滤）。
- **出料**：`bong:server_data` 单通道（目标态：28 旁路全部收编或显式豁免登记）；join/重连首包快照集契约（R2 清干净后靠它灌满）。
- **共享类型**：新 server `network/emit/` builder（`scope: Global | Dimension | Zone | Player`）；client 桥接层唯一的枚举前缀剥离函数。
- **跨仓库契约**：proto 形状原则上不动（收编旁路时如需并入 envelope 属破坏性变更，走 buf breaking + samples 同步）；TypeBox canonical content 是 repo-wide schema source of truth，R6 负责据此运行 generation pipeline 并同步 protobuf/generated Rust、Rust conversion、Java bridge、router plumbing 与 dist/JSON Schema/samples，agent 侧只消费生成结果，不反向定义 schema，也不重构 agent 逻辑。每个 generated/constrained artifact 必须携可核验的 TypeBox source hash 或 version reference，CI 对当前 source 与 pin 的不一致 fail-closed。**不做双轨兼容层**——旁路收编是一次性切换。R1 craft 例外要求显式冻结并一次性切换 `CraftOpen` / `CraftPause` / `CraftResume` C2S intent：三者均必须携带 `session_key: string` 与 `generation: uint64`，且两字段均 required；`CraftOpen` 使用 server hydration 后的当前 identity/version，`CraftPause` 与 `CraftResume` 必须回显该次 hydration 的 identity/version，禁止省略、默认值或仅以 client Entity 代替。并用 `CraftSessionStateV2` 一次性替换既有 S2C `CraftSessionStateV1`：V2 除现有进度字段外，固定携带 `session_key`、单调 `generation`、`phase: Idle | Running | Paused | Suspended | AwaitingDelivery | DeliveryPending | Terminal`；`Idle` 明确无 session identity，其余 phase 必须携带 identity/version。现有 `CraftCancel` 保留为唯一主动取消 intent，关屏不得复用它。R6 同时拥有 C2S 的 `agent/packages/schema/src/client-request.ts` TypeBox source、生成 JSON Schema 与提交的 `@bong/schema` dist、protobuf/Rust mirror/converter、samples、client encode/send APIs；并拥有 S2C `CraftSessionStateV2` 在 `agent/packages/schema/src/craft.ts`→generated JSON Schema/dist→`proto/bong/envelope.proto`→Rust `server/src/schema/craft.rs`/`proto_convert.rs`→`network/craft_emit.rs` producer→`ProtoServerDataBridge`→`CraftSessionStateHandler`/`CraftStore` consumer 的一次性全链替换，同一提交删除 V1 schema/proto/bridge 分支。R7 只消费这些冻结接口；R6 不复制或提前实现 R7 的 production Resume producer。

## 阶段

- ⬜ P0 设计收口 + 吸收清单验真：28 旁路逐个普查（收编 vs 豁免理由）；100 emit 文件的重复模式取样归纳 builder API；枚举前缀剥离点全量清点；冻结 scope 语义与 join 首包快照集清单；正式登记 `rotate-footprint-sync`、`bot-inventory-pack-feedback` 的 inventory wire/feedback 工作，并冻结 `dropped_loot_sync` 分片 envelope（`snapshot_revision/page_index/page_count`、每页上限常量 `DROPPED_LOOT_SYNC_PAGE_SIZE = 256`）。
- ⬜ P1 schema generation chain contract + emit builder/scope：以 `agent/packages/schema/src/` 的分域 TypeBox declarations 与 `schema-registry.ts` 为 canonical source layout，交付 source→committed JSON Schema/dist→protobuf/generated Rust→Rust conversion→Java bridge/`ServerDataRouter` registration 的 generation manifest、单入口 tooling 与 artifact/source-pin inventory；CI 运行 freshness、missing/unexpected artifact、source hash/version pin 与 deterministic regeneration checks，任一 drift fail-closed。上述 generation artifacts 在本阶段必须 declared/unwired/test-only，不接 production traffic。同步上线与 schema chain 无关的共享 emit builder，并先让 vfx/audio/env 三类挂 scope（跨维 bleed 立灭）；跨位面切换时 env/season 全量重发。dropped-loot 只提交分页 envelope、producer/consumer API stub 和空/单页/恰好 256/257/末页缺失/混 revision 的正反 pin，不启用发送。
- ⬜ P2 client bridge/consumer contract：枚举前缀剥离收敛到单点（含 forge-session 修复）；`ServerDataRouter` 分域注册接口整备；以 declared/unwired/test-only 形式交付 dropped-loot revision page assembler/store reducer，pin 同 revision 全部分片收齐后原子替换，以及缺页/混 revision 保留旧视图并请求/等待重发；旧 receiver 与旧 producer 原样保留。
- ⬜ P3 schema mirrors + atomic production activation：运行 P1 generation chain 产出/刷新 protobuf/generated Rust、Rust conversion、Java bridge、dist/JSON Schema/samples，并接入 `ServerDataRouter`；28 channel 按 merge unit 逐批收编入 server_data envelope或登记豁免（资源包/握手类可豁免）。**dropped-loot 是一个不可拆 merge unit**：同一 merge unit 内同时启用共享 builder 的 paginated producer、generated mirror/conversion/router transport、P2 完整 revision page assembler 的 atomic store replace，并删除对应旧 producer/receiver；在总纲 §3 Wave 表放行前保持 P1/P2 artifacts unwired 且旧路径原样可用，禁止 P1 单独启用 producer、长期 dual emit 或 child-only 前置。
- ⬜ P4 契约 pin 全量化：双向 sample 对拍测试补齐（116 C2S + 144 S2C 每变体至少一条正反 sample，schema 改动连 sample 一起改）；emit 迁移到 builder 的长尾批次；完成 inventory receipt contract 子批次：`InventoryEventV1::Moved`（或等价 accepted receipt）必须携带 request identity、结果 revision、权威 item view，覆盖 schema/sample/convert/emit API、Fabric `InventoryEventHandler` 与 Python decoder，供 R4 handler 消费 R10 typed outcome。P1/P2 contract-first artifacts 的 pin tests 不得延期至本阶段。
=======
- **跨仓库契约**：proto 形状原则上不动（收编旁路时如需并入 envelope 属破坏性变更，走 buf breaking + samples 同步；agent 侧 TS 只做被动 regenerate，不重构 agent 逻辑）。**不做双轨兼容层**——旁路收编是一次性切换。R1 craft 例外要求显式冻结并一次性切换 `CraftOpen` / `CraftPause` / `CraftResume` C2S intent：三者均必须携带 `session_key: string` 与 `generation: uint64`，且两字段均 required；`CraftOpen` 使用 server hydration 后的当前 identity/version，`CraftPause` 与 `CraftResume` 必须回显该次 hydration 的 identity/version，禁止省略、默认值或仅以 client Entity 代替。并用 `CraftSessionStateV2` 一次性替换既有 S2C `CraftSessionStateV1`：V2 除现有进度字段外，固定携带 `session_key`、单调 `generation`、`phase: Idle | Running | Paused | Suspended | AwaitingDelivery | DeliveryPending | Terminal`；`Idle` 明确无 session identity，其余 phase 必须携带 identity/version。现有 `CraftCancel` 保留为唯一主动取消 intent，关屏不得复用它。R6 同时拥有 C2S 的 `agent/packages/schema/src/client-request.ts` TypeBox source、生成 JSON Schema 与提交的 `@bong/schema` dist、protobuf/Rust mirror/converter、samples、client encode/send APIs；并拥有 S2C `CraftSessionStateV2` 在 `agent/packages/schema/src/craft.ts`→generated JSON Schema/dist→`proto/bong/envelope.proto`→Rust `server/src/schema/craft.rs`/`proto_convert.rs`→`network/craft_emit.rs` producer→`ProtoServerDataBridge`→`CraftSessionStateHandler`/`CraftStore` consumer 的一次性全链替换，同一提交删除 V1 schema/proto/bridge 分支。R7 只消费这些冻结接口；R6 不复制或提前实现 R7 的 production Resume producer。

## 阶段

- ⬜ P0 设计收口 + 吸收清单验真：28 旁路逐个普查（收编 vs 豁免理由）；100 emit 文件的重复模式取样归纳 builder API；枚举前缀剥离点全量清点；冻结 scope 语义与 join 首包快照集清单。
- ⬜ P1 emit builder + scope 落地：builder 上线，vfx/audio/env 三类先挂 scope（跨维 bleed 立灭）；跨位面切换时 env/season 全量重发；同时交付 R1 craft 所需 C2S `CraftOpen`/`CraftPause`/`CraftResume` TypeBox source→generated JSON Schema/dist→proto/Rust mirror/converter→正反 samples，以及 client `ClientRequestProtocol` encode / `ClientRequestSender` send APIs 与 producer contract tests。三种 intent 均必须携带并 roundtrip required 的 `session_key` + `generation`，生产 decoder/handler 由 R4 消费；同阶段原子交付 S2C `CraftSessionStateV2` hydration 扩展的 TypeBox/schema/dist、proto/Rust mirror/converter、`craft_emit` phase/session_key/generation producer、bridge/router/handler/CraftStore consumer 与 samples。R6 contract pins 只证明 schema/bridge/store 的身份字段、phase 全量序列化及 stale/mismatched identity 拒绝；不得在 R6 复制或提前实现 R7 的 production Resume producer。R7 的匹配 `Paused`→恰发一次 `CraftResume` 证明移交 R7 P2，依赖顺序与总纲 `plan-refactor-master-v1.md §3 Wave 2` 一致。R6 必须证明 Idle、Running、Suspended、AwaitingDelivery、DeliveryPending、Terminal、missing/stale/mismatched session key 或 generation 均不发 Resume；断线重连、乱序旧 generation 与同 key 新 generation 不得回退 store。
- ⬜ P2 client 桥接层收敛：枚举前缀剥离收敛到单点（含 forge-session 修复）；`ServerDataRouter` 注册表整备（分域注册文件，不再单个 1547 行 switch 追加）。
- ⬜ P3 旁路归一批次：28 channel 逐批收编入 server_data envelope 或登记豁免（资源包/握手类可豁免）；删除散装 receiver。
- ⬜ P4 契约 pin 全量化：R6 P1 落地三种 craft intent 后，以 116 C2S + 144 S2C 每变体至少一条正反 sample 对拍（在此之前当前 C2S baseline 仍为 113；schema 改动连 sample 一起改）；emit 迁移到 builder 的长尾批次。
>>>>>>> 2314a3735 (补齐 craft 会话身份与 Resume 契约)
- ⬜ P5 bot 验收 + 吸收 plan 批量归档。

R10 dropped-loot 契约优先：编码前按 recipient dimension/range/owner 投影，仅同 visibility key 复用；rejected receipt 含 reason/instance/from/to，并测两 recipient 正反可见性。

## 吸收清单（短名省略 plan-bughunt- 前缀与 -v1 后缀）

active：server-data-s2c-schema-union-drift（TS union 补齐走 regenerate）、spirit-treasure-chat-key-conflict 除外（归 R7）。
skeleton：vfx-audio-dimension-bleed、q-world-season-dimension-env-resync、forge-session-enum-unstripped（#1294 在飞）、client-request-schema-drift（C2S 契约 pin 部分）、cl-ningmai-meridian-target-drop（payload 字段丢失）、alchemy-recipe-fragment-handoff（id 前缀契约）、vfx-event-slash-contract（event_id 格式契约；agent 侧改动最小化）、npc-trade-bundle-count-bridge（展示/结算数量桥）、dropped-loot-g-pickup-range-desync（拾取范围下发对齐部分）、rotate-footprint-sync（R10 typed outcome → moved/accepted 权威 item view）、bot-inventory-pack-feedback（成功/拒绝动作级机器回执）、skillbar-cast-source-drift 与 skillconfig-castsync 除外（归 R9）。
注：server↔agent 方向的桥（anticheat-tiandao-drop、niche-guardian-redis-dispatch、npc-combat-relic-schema-drift、pseudo-vein-agent-deadwire、war-participate-agent-command-drift、天道叙事簇 14 项）**不吸收**——agent 不在本次重构范围，独立保留（见总纲 §6 独立轨）。

## 文件所有权与边界

- 独占：`agent/packages/schema/src/{schema-registry,generated-artifacts,generate}.ts` generation machinery、`agent/packages/schema/generated/` 与 `proto/bong/` generated/constrained mirrors、`server/build.rs`/`server/src/schema/proto_gen.rs` generation 接缝、server `network/*_emit.rs` 公共模式与新 `network/emit/`、`schema/proto_convert.rs`；client `network/`（generated ProtoServerDataBridge、ServerDataRouter、BongNetworkHandler 的 channel 注册区段）。各 domain TypeBox declaration 内容仍归对应 domain owner，R6 只消费 canonical content。
- 不碰：`BongNetworkHandler.clearClientStateOnDisconnect` 区段（R2 域，同文件分区段，merge 前互相 fetch）；`client_request_handler.rs`（R4）；各 emit 的业务语义。
- 依赖：跨轨 start/order/cutover 以总纲 §3 Wave 表为唯一 authority；contract-first 工作可在 Wave 0 开始，涉及其他 track 所属 production 接缝时只引用总纲 Wave 放行，不在本 plan 重述或新增前置。dropped-loot 在放行前仅保留 P1/P2 contract/test-only artifacts 与完整旧路径；放行后由 P3 单一 merge unit 原子切换，不阻塞其他 scope 子项。

## bot 验收场景

1. `wire_scope_dimension`：主世界 bot + TSY bot 双开，主世界触发 vfx/audio→断言 TSY bot 收不到（P6 protobuf 深断言配合）。
2. `wire_dimension_transfer_resync`：bot 跨位面→断言 env/season/zone 大气全量重发。
3. `wire_contract_sweep`：对 144 S2C 变体的 sample 对拍在 CI 常绿（配 proto-breaking 深检，联动 V 轨）。
4. `wire_join_snapshot`：重连首包快照集完整（与 R2 的 `reconnect_state_freshness` 同场景）。

## 开放问题（pre-P0 收口）

1. 28 旁路的收编/豁免分界（低频大 payload 如资源包显然豁免；npc bubble 这类高频小包是否值得并入 envelope）。
2. join 首包快照集的权威清单放哪维护（emit builder 注册时声明 `replay_on_join` 标志？）。
3. R7 production `CraftResume` producer 的验收证明不在 R6 P1 提前实现；由 R7 P2 在 R6 wire/bridge、R4 handler/gate 与 R2 store 前置合入后完成，依据总纲 `plan-refactor-master-v1.md §3 Wave 2`。R6 P1 只关闭 identity/phase/schema/bridge/store 的负向契约，避免复制未来 producer。
