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
- **跨仓库契约**：proto 形状原则上不动；破坏性变化走 buf/samples gate，不做双轨兼容。craft domain shape 只消费 A-CS A-01..A-08 的冻结 SHA；inventory receipt TypeBox/sample 只消费 master M-16 的 Agent-owned 冻结 SHA，R6 不修改 `agent/`。R6 拥有 proto/Rust/client generation machinery、converter、encode/send API、bridge/router 与 emit API。contract-first artifacts 可先合入但不得宣称 live；真实 producer/consumer 切换只按 master §3/§4.1 与 PR 1902 atomic activation 执行。`workbench_key` 仅为当前进程 runtime locator，wire/store/checkpoint 不把它当 durable identity。dropped-loot projection/page 消费 M-13 与 R3 guarded hydration evidence、作为 R6 producer evidence 汇入 M-14；R6 不等待自己参与生产的 M-14，只有两侧证据闭合后才 activation。跨语言 identity/revision scalar 沿用 A-CS §2 的 `OpaqueId` 与 `U64DecimalString`：R6 wire pin、converter 与 Java bridge 必须保持 canonical ASCII/decimal-string 形状及 0/1/u64::MAX、缺失、前导零、空白、overflow 反例，不得改用有损 JavaScript integer 或 signed Java long。

## 阶段

- ⬜ P0 设计收口 + 吸收清单验真：28 旁路逐个普查（收编 vs 豁免理由）；100 emit 文件的重复模式取样归纳 builder API；枚举前缀剥离点全量清点；冻结 scope 语义与 join 首包快照集清单；登记 `rotate-footprint-sync`、`bot-inventory-pack-feedback`，并冻结 dropped-loot recipient protocol：`DroppedLootProjectionReset { projection_revision, projection_epoch, reset_token, projection_digest }`、分页 envelope `snapshot_revision/page_index/page_count/projection_epoch/reset_token/projection_digest`、`DROPPED_LOOT_SYNC_PAGE_SIZE = 256`、`MAX_DROPPED_LOOT_PROJECTION_REBUILDS_PER_TICK = 64`、`MAX_DROPPED_LOOT_SYNC_BYTES_PER_TICK = 4_194_304`。
- ⬜ P1 emit builder + scope、craft machinery contracts：只交付 `network/emit/` builder API、scope declarations、converter/roundtrip pins，以及 A-CS/M-16 冻结 artifacts 的 proto/Rust/client mirrors、encode/send API、bridge/router/store contract pins 和 `craft_emit` **API/stub**；不把 generic vfx/audio/env 调用点迁移或宣称 production live。vfx/audio/env 的 scope declarations、migration inventory 和 builder contract 可先以 contract-first 形式记录，任何真实 producer 切换必须等待 master §4.1 已存在的对应 M-row；master 未登记的 generic migration 不得在本阶段启用。P1 acceptance 逐条覆盖 Open/Start/Pause/Resume/Cancel/OpenRejected、两种 target、request/session identity、accepted hydration 的 `open_request_id`、`session_transition` rollover/restore discriminant、`phase_revision`、u64 decimal-string、Start quantity、WorkbenchOpen 全字段、StateV2 phase/generation 的 wire roundtrip 与 malformed reject；A-05 必测 entity 0/1/u64::MAX 与 x/y/z 各自 i32::MIN/-1/0/i32::MAX，A-06 必测五 phase × generation 0/1/u64::MAX 及 phase revision/rollover/restore 边界，不得以单个消息样本代替值矩阵。不要求尚未允许存在的 R1 state producer、R4 gate 或 R7 intent producer，不宣称 production reachable。dropped-loot projection/page contract 可在 M-13 与 R3 hydration evidence 后实施并贡献 M-14，production activation 只在 M-14 双侧闭合后启用。
- ⬜ P2 client 桥接层收敛：枚举前缀剥离收敛到单点（含 forge-session 修复）；`ServerDataRouter` 注册表整备（分域注册文件，不再单个 1547 行 switch 追加）。dropped-loot store 维护 `projection_epoch`/`reset_token`/`projection_digest` 绑定，以及 `revision_floor`、`highest_committed_revision` 与每 revision assembly：每次 context change 先生成新的 context binding 并发送 `DroppedLootProjectionReset { projection_revision, projection_epoch, reset_token, projection_digest }`；每个 page 必须回显完全相同的 binding，binding 不匹配即 fail closed、不得组装或提交。reset revision 必须 `> max(revision_floor, highest_committed_revision)`，收到后立即清 visible store、丢弃更低 assembly并把 floor 设为该 revision；同 revision pages 只有在 binding 匹配时才可组装。无 reset 的完整 snapshot 只有 revision `> highest_committed_revision` 且 `>= revision_floor` 才可 commit；commit 后更新两者并清除所有更低 assembly，迟到/重放的较低 revision 或旧 binding no-op。empty projection 必须发送 `page_count=1,page_index=0,entries=[]` 并把旧非空 store 清空；缺页/混 revision/混 binding 保留当前 fail-closed 视图。
- ⬜ P3 旁路归一批次：只完成 28 channel 的 production inventory、server_data registration declarations 与逐项豁免记录；craft machinery 只消费 M-02，craft 真实 producer/consumer 切换只由 M-10 放行，dropped-loot projection 由 M-14、pickup/receipt 由 M-15 各自放行。generic channel 的真实 receiver 删除或 producer 切换若无 master M-row，不在本轨宣称完成，继续作为 contract-first follow-up。
- ⬜ P4 契约 pin 全量化：从当时 authoritative registries 派生 C2S/S2C type sets，对每个实际变体至少一条正反 sample；禁止手写 116/144 充当 freshness 证据。emit 迁移长尾。dropped-loot 分页必须覆盖旧非空→0（empty page 清空）、1/255、256（恰一满页且无空尾页）、257（256+1 两页）、缺末页、混 revision、N+1 commit 后 N 迟到完成；后四者不得回滚 store。accepted move/rotate/pack receipt 固定含 request identity、result revision、instance identity、from、to、带 post-operation footprint/orientation 的 authoritative item view；rejected receipt 固定含 request correlation、reason、instance、from、to。两类 receipt 必须逐 Rust domain→proto→M-16 TypeBox/sample→converter/emit→Java router/store（及适用 bot decoder）对拍。
- ⬜ P5 bot 验收 + 吸收 plan 批量归档。

R10 dropped-loot 契约优先：registry mutation、join，以及 recipient 的 dimension/range observation bucket/owner/admin permission key 变化都递增该 recipient projection revision；context key 变化先生成新的 `projection_epoch`/`reset_token`/`projection_digest` binding，发 reset 使 client fail-closed 清旧视图，再以 dimension/range spatial index + dirty-recipient queue 重建；每个 page 必须回显同一 binding，不允许旧 context page 参与新 assembly，也不允许每 recipient 扫描全 4096 rows。每 tick 最多重建 64 个 projection、发送 4 MiB sync bytes，超额保留 dirty/最新 revision并合并中间版本；reset/revocation 优先于新增可见页。只有完全相同 visibility key + projection digest 可复用编码；rejected receipt 携带 reason/instance/from/to，并测两 recipient 正反可见性。

## 吸收清单（短名省略 plan-bughunt- 前缀与 -v1 后缀）

active：server-data-s2c-schema-union-drift（TS union 补齐走 regenerate）、spirit-treasure-chat-key-conflict 除外（归 R7）。
skeleton：vfx-audio-dimension-bleed、q-world-season-dimension-env-resync、forge-session-enum-unstripped（#1294 在飞）、client-request-schema-drift（C2S 契约 pin 部分）、cl-ningmai-meridian-target-drop（payload 字段丢失）、alchemy-recipe-fragment-handoff（id 前缀契约）、vfx-event-slash-contract（event_id 格式契约；agent 侧改动最小化）、npc-trade-bundle-count-bridge（展示/结算数量桥）、dropped-loot-g-pickup-range-desync（拾取范围下发对齐部分）、rotate-footprint-sync（R10 typed outcome → 权威 item view）、bot-inventory-pack-feedback（动作级回执）、skillbar-cast-source-drift 与 skillconfig-castsync 除外（归 R9）。
注：server↔agent 方向的桥（anticheat-tiandao-drop、niche-guardian-redis-dispatch、npc-combat-relic-schema-drift、pseudo-vein-agent-deadwire、war-participate-agent-command-drift、天道叙事簇 14 项）**不吸收**——agent 不在本次重构范围，独立保留（见总纲 §6 独立轨）。

## 文件所有权与边界

- 独占：server `network/*_emit.rs` 公共模式与新 `network/emit/`、`schema/proto_convert.rs`；client `network/`（ProtoServerDataBridge、ServerDataRouter、BongNetworkHandler 的 channel 注册区段）。
- 不碰：`BongNetworkHandler.clearClientStateOnDisconnect` 区段（R2 域，同文件分区段，merge 前互相 fetch）；`client_request_handler.rs`（R4）；各 emit 的业务语义。
- 依赖与 production activation 只引用 master §3/§4.1 及 PR 1902。R6 P1 可在 A-CS A-01..A-08 冻结后交付 machinery contracts；R1 state producer、R4 runtime rejection、R7 intent producer 的 production evidence 分别留给其 owner phase，不构成 R6 P1 completion。dropped-loot projection/page 只消费 M-13 与 R3 hydration producer evidence并贡献 M-14，M-14 闭合才允许 production activation。

## bot 验收场景

1. `wire_scope_dimension`：在 builder/router contract harness 中对主世界与 TSY scope 做双 recipient pin，主世界 producer 的 vfx/audio scope 不得进入 TSY projection；generic producer migration 只有 master §4.1 新增对应 M-row 后才转 production e2e。
2. `wire_dimension_transfer_resync`：在 scope contract harness 中验证跨位面 env/season/zone 大气的 re-emission declaration 与 recipient filtering；generic production activation 仍须等待 master §4.1 对应 M-row。
3. `wire_contract_sweep`：对 registry-derived S2C type set 做 sample 对拍；craft 逐 A-01..A-08 对拍，A-05 执行 entity 与每个 coordinate boundary 矩阵，A-06 执行五 phase × generation boundary 矩阵。runtime stale/rejection 与 producer behavior 引用 owner trace，不在本场景伪造。
4. `wire_inventory_receipt_and_pages`：accepted 2×1→1×2 rotate receipt 的 request/revision/instance/from/to 与 authoritative footprint/orientation 全链一致，rejected correlation/reason 仍完整；recipient projection 执行旧非空→empty reset/page 清空、1/255/256/257、缺页/混 revision、N+1 后 N 迟到，断言 revision 单调；移动出范围/换维/撤销 admin 而 registry 不变也必须先生成新的 `projection_epoch + reset_token + projection_digest` binding，reset 与所有 replacement pages 必须逐字段匹配该 binding，旧 binding 或未匹配 page 一律 fail closed 且不得提交。另以 4,096 entries × 多 distinct visibility keys 压测，逐 tick rebuild/bytes 不超过 64/4 MiB且不做 recipient×全表扫描。
5. `wire_join_snapshot`：重连首包快照集完整；craft hydration 只在 master activation row 完成后接入，并引用 R1 S-10/S-16。

## 开放问题（pre-P0 收口）

1. 28 旁路的收编/豁免分界（低频大 payload 如资源包显然豁免；npc bubble 这类高频小包是否值得并入 envelope）。
2. join 首包快照集的权威清单放哪维护（emit builder 注册时声明 `replay_on_join` 标志？）。
3. production `CraftResume`、runtime-key rejection 与 real `craft_emit` activation 不在 R6 P1 验收；分别由 R7/R4/R1 owner phase 按 master atomic cutover row 提供。R6 只验 converter/API/stub contract。
4. `CraftSessionStateV2.generation` 的 `0`、`u64::MAX`、overflow refusal 属 R6 wire pin；producer/store CAS behavior 属其 owner activation trace，不混入上游 phase。
