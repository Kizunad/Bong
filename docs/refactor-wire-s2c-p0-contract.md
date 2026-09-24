# R6 P0 — S2C wire contract freeze

> 基线：2026-07-30，`plan-refactor-wire-s2c-v1` P0。本文只冻结后续 P1–P4 的接口与迁移账本；**不改变任何生产发送、过滤、序列化或 client dispatch 行为**。

## 1. 普查口径与可复现基线

### 1.1 client receiver 口径

权威口径是 `client/src/main/java/**/*.java` 中所有经 JDK AST + symbol attribution 解析的 `ClientPlayNetworking.registerGlobalReceiver(...)` 真实调用，不只看 `BongNetworkHandler.register()`。扫描基线共 **32 个 receiver**：

- `BongNetworkHandler.java`：31 个（含唯一目标轨 `bong:server_data`）。
- `IrisBootstrap.java`：1 个（`bong:shader_state`）。
- 扣除 `bong:server_data` 后，旁路基线是 **31 个**。

扫描同时冻结：每个 channel ID 唯一、调用解析到 Fabric API 的精确 owner、所有 receiver 从 `BongClient.onInitializeClient()` 的两个 production bootstrap（`BongNetworkHandler.register` / `IrisBootstrap.register`）可达。动态/不可解析 ID、receiver method reference、错误 owner 或 dead helper 均 fail closed。

所有旁路都必须出现在下表；新增 receiver 未登记、删除 receiver 未清账均由 `WireS2cContractPinTest` 阻断。

| channel | P3 决策 | 理由 / 前置 |
|---|---|---|
| `bong:npc_metadata` | 收编 | 普通 S2C 状态；补 envelope case 后一次性切换。 |
| `bong:npc_lod` | 收编 | 普通 S2C 状态；补 envelope case 后一次性切换。 |
| `bong:npc_bubble` | 收编 | 高频小包仍需走统一 session/router/scope，不因频率豁免。 |
| `bong:npc_mood` | 收编 | 普通 S2C 状态；补 envelope case 后一次性切换。 |
| `bong:tsy_boss_health` | 收编 | 维度局部 HUD；统一 scope 可阻断跨维串场。 |
| `bong:tsy_death_vfx` | 收编 | 维度局部瞬时事件；统一 scope 可阻断跨维串场。 |
| `bong:locust_swarm_warning` | 收编 | zone/dimension 事件，须统一作用域。 |
| `bong:vfx_event` | 收编 | proto 已有 `VFX_EVENT`；P1 首批迁移并加 Dimension/Zone 过滤。 |
| `bong:vfx/qi_attrition` | 收编 | 维度局部 VFX；迁入 envelope 后保留现有 payload 大小门。 |
| `bong:audio/play` | 收编 | proto 已有 `AUDIO_PLAY_EVENT`；P1 首批迁移并加作用域。 |
| `bong:audio/stop` | 收编 | proto 已有 `AUDIO_STOP_EVENT`；与 play 原子切换。 |
| `bong:tiandao_presence` | 收编 | 普通 S2C 状态；保留 client-thread dispatch。 |
| `bong:audio/ambient_zone` | 收编 | proto 已有 `AMBIENT_ZONE_EVENT`；Zone scope。 |
| `bong:zone_environment` | 收编 | proto 已有 `ZONE_ENVIRONMENT_STATE`；P1 首批，Dimension+Zone scope。 |
| `bong:mutation_visual` | 收编 | 玩家/维度可见状态；补 envelope case。 |
| `bong:crack_reading` | 收编 | player-local HUD，映射 Player scope。 |
| `bong:resonance_lock` | 收编 | 指定参与者 HUD/VFX，映射 Player scope（每人一发）。 |
| `bong:resonance_lock_end` | 收编 | 必须与 `resonance_lock` 同批切换，避免锁状态只进不出。 |
| `bong:void_erosion_visual` | 收编 | player-local/维度可见视觉状态；补 envelope case。 |
| `bong:spider_disguise_enter` | 收编 | 维度局部实体全量状态，必须 Dimension scope。 |
| `bong:spider_ambush_trigger` | 收编 | 维度局部实体事件，与 disguise 同批切换。 |
| `bong:rat_qi_tier` | 收编 | 维度局部实体全量状态，必须 Dimension scope。 |
| `bong:daozhan_disguise_enter` | 收编 | 维度局部实体全量状态，必须 Dimension scope。 |
| `bong:daozhan_reveal` | 收编 | 维度局部实体事件，与 disguise 同批切换。 |
| `bong:core_absorption_hallucination` | 收编（R4 交付 API 后） | sender 在 R4 独占 `client_request_handler.rs`；R6 不改该文件，只消费冻结的 emit API。 |
| `bong:elder_encounter` | 收编 | player-local HUD，映射 Player scope。 |
| `bong:era_ambiance` | 收编 | 维度/境界门控状态；须纳入 join replay。 |
| `bong:agent_ui_request` | 豁免 | 双向 UI 会话控制、裸 XML/JSON，生命周期与普通 server state 不同；保留专用 channel。 |
| `bong:agent_ui_close` | 豁免 | 必须与 request 成对保留；不能只迁一半。 |
| `bong:halfstep_rechallenge` | 收编 | 普通 player-local S2C 触发；补 envelope case，不因现状裸 JSON 永久豁免。 |
| `bong:shader_state` | 豁免 | dev/Iris capability channel，发送端是 `cmd/dev/shader_push.rs`；不属于 gameplay ServerData。 |

结论：**28 收编，3 豁免**。P3 不做双轨兼容；每个收编项在同一提交内完成 server sender → proto envelope → bridge/router → 删除专用 receiver 的一次性切换。

### 1.2 server emit 文件口径：API 形状与实际 wire 分轴

权威口径是 `server/src/**/*_emit.rs`（递归，不把普通 `bridge.rs`/`resourcepack.rs` 算作 emit 文件）。基线共 **68 个**。P0 pin 用 production-only lexical scanner 排除 import、注释、doc comment、字符串/字符字面量与 `#[cfg(test)]` 项，只识别真实 call shape，并冻结两条不能混为一谈的轴：

**API call shape：**

- 51 个 `helper_only`：仅调用 `send_server_data_payload`。
- 13 个 `direct_only`：仅调用 `Client::send_custom_payload`。
- 1 个 `both`：`cultivation_detail_emit.rs` 同时调用两种 API。
- 2 个 `no_client_send`：`meridian_severed_emit.rs`、`tuike_ash_emit.rs`。
- 1 个 `redis_only`：`identity/wanted_player_emit.rs`。

**实际 wire channel：**

- 53 个 `server_data_only`。
- 12 个 `dedicated_only`。
- 0 个 `channel_mixed`。
- 2 个 `domain_only`。
- 1 个 `redis_only`。

`cultivation_detail_emit.rs` 与 `qi_color_observed_emit.rs` 虽使用 direct API，实际 channel 仍是 `bong:server_data`，不能按 API 名称误判为专用旁路。P0 pin 同时冻结文件全集、API shape、wire class；scanner regression tests 覆盖 import、注释/doc comment、string/raw string、`#[cfg(test)]`、multiline call 与真实 call。

## 2. emit builder API 冻结（P1 实现目标）

重复模式由现有 emit 文件抽样归纳为：读取 ECS/event → 构造 `ServerDataV1` → `serialize_server_data_payload` → 选择 client → `send_server_data_payload` → 统一错误/trace。P1 的公共层固定在 `server/src/network/emit/`，最小公开契约如下（本节是设计签名，不是 P0 生产代码）：

```rust
pub enum EmitScope {
    Global,
    Dimension(DimensionKind),
    Zone {
        dimension: DimensionKind,
        zone: String,
    },
    Player(Entity),
}

pub struct ServerDataEmission {
    pub payload: ServerDataV1,
    pub scope: EmitScope,
    pub replay: ReplayPolicy,
}

pub enum ReplayPolicy {
    None,
    JoinSnapshot(JoinSnapshotKey),
}

pub fn emit_server_data(
    emission: &ServerDataEmission,
    recipients: &mut Query<(
        Entity,
        &mut Client,
        Option<&CurrentDimension>,
        Option<&Position>,
    )>,
    zones: Option<&ZoneRegistry>,
) -> EmitReport;
```

冻结约束：

1. builder 是唯一执行 `ServerDataV1` 序列化、类型标签、payload build error 日志和 `bong:server_data` send 的公共层；业务 emit 仍负责 DTO 构造与业务触发条件。
2. scope 是必填字段，不提供“默认 Broadcast”。调用方不知道 scope 时必须显式选择并在 review 中说明。
3. builder 对每个 emission **只序列化一次**，再复用不可变 bytes 发给匹配 recipients。
4. `EmitReport` 至少暴露 `matched`、`sent`、`serialization_failed`，仅供日志/测试；发送失败不得改 gameplay 状态。
5. `ReplayPolicy` 是登记元数据，不允许 builder 隐式查询所有业务 registry 拼快照；join replay 由 §5 的权威注册表调度业务 snapshot producer。
6. 非 `ServerDataV1` transport（Redis、C2S、资源包、dev shader）不接该 builder。

## 3. scope 语义冻结

| scope | 精确定义 | 缺元数据策略 |
|---|---|---|
| `Global` | 发给所有当前 PLAY clients，允许跨维。只用于确实全服可见的叙事/系统状态。 | client 只要存在即可。 |
| `Dimension(d)` | 仅 `CurrentDimension == d` 的 clients；空间距离不能替代维度相等。 | client 缺 `CurrentDimension` 时 fail closed，不发送并计入诊断。 |
| `Zone { dimension, zone }` | 先要求同维度，再以权威 `ZoneRegistry` + client `Position` 求 canonical zone ID，结果等于登记的 `String` zone ID 才发送。zone ID 不能跨维单独匹配。 | 缺 dimension/position/registry 或无法解析 zone 时 fail closed。 |
| `Player(entity)` | 只发给目标 client entity；玩家换维不改变“发给该玩家”的语义。 | 目标不存在/不是 Client 时零发送；不得回退 broadcast。 |

额外约束：radius/view-distance 是 scope 命中后的**附加过滤器**，不能替代 `Dimension`；因此 VFX/audio 的 P1 顺序必须是“同维 → 半径/视距”。

## 4. proto enum normalization 冻结

proto3 enum → legacy handler 字符串转换的目标态集中在 `ProtoServerDataBridge`；P0 同时登记当前 production receive path 唯一的 handler-side 例外，不能把 P2 目标态冒充 P0 现状。冻结四种模式：

1. `snake_lower`：剥前缀后 `SCREAMING_SNAKE_CASE → snake_case`。
2. `capitalized`：剥前缀后单词首字母大写（`SHARP → Sharp`）；仅 Realm/Color 的既有 legacy consumer。
3. `pascal_case`：剥前缀后多段 PascalCase（`ZHENFA_WARD_ALERT → ZhenfaWardAlert`）。
4. `snake_lower_omit_unspecified`：与 1 相同，但 proto 默认 `*_UNSPECIFIED` 删除字段，让 legacy required-field gate fail closed。

P0 bridge-local 盘点为 **43 个唯一前缀 / 57 处 lexical literal 引用**；语义 pin 另以 prefix→field→mode 多重集冻结，因 helper 复用和 craft `qi_color_min[0]` 手写分支，语义操作共 **58 处**。`InventoryEventHandler` 仍在嵌套 `inventory_event.from|to.equip` 处理 `EQUIP_SLOT_` 与必填 `EQUIP_STATE_`（2 个前缀 / 2 处 production lexical 引用 / 2 处语义操作），所以完整 production receive path 是 **45 个唯一前缀 / 59 处 lexical literal 引用**，语义 pin 共 **60 处**。`WireS2cContractPinTest` 同时锁定 lexical multiset、bridge-local semantic multiset 与 full-path 文件归属；P2 才把该 handler 逻辑迁回 bridge。

## 5. join / reconnect replay 权威清单

权威清单由 `server/tests/wire_s2c_contract_pin.rs::REPLAY_PINS` 维护。每项冻结 source function、trigger/cache marker、production registration function 以及 `app.add_systems(...)` 内的 exact callee path；测试基于剥除 comments/strings/`#[cfg(test)]` 的 token 结构，而非 raw substring occurrence。P0 基线共 55 个分类项（同一 producer 可同时承担 cache-first 与 periodic convergence）：

| 分类 | 数量 | producer / 语义 |
|---|---:|---|
| `ProtocolHandshake` | 1 | `send_welcome_payload_on_join`：`Added<Client>` welcome。 |
| `StrictJoin` | 10 | dropped loot、remains、rift portal、TSY container、spider/Daozhan/rat 全量、era ambiance、coffin、`AwaitingRevival` death-screen reconnect。 |
| `JoinDerived` | 21 | player state、inventory、skill、techniques、realm vision、anonymity、zone dirty→broadcast、identity、quickslot、skillbar、unlocks、combat HUD、wounds、derived attrs、status、weapon、treasure、spirit treasure、false-skin stack、material discovery。 |
| `ActiveReplay` | 2 | tribulation state / broadcast 以 `known_clients: HashSet<Entity>` 的 identity set difference 向新 entity 重放 active state。 |
| `DefectiveReplay` | 1 | ascension quota 只比较全局 `last_client_count`：同数量 disconnect/reconnect 替换可能漏发 newcomer，数量变化又会重发旧 client。P0 只登记缺陷，不改生产行为。 |
| `CacheMissImmediate` | 10 | craft recipe、body plan、race gate、morph、zone info、tutorial coffin、skill config、ambient audio、healer AI、carrier。 |
| `CacheMissAtCadence` | 2 | NPC mood、TSY boss health；首次 cache miss 要等各自 cadence。 |
| `PeriodicConvergence` | 8 | cultivation detail、morph、NPC LOD、spider/Daozhan/rat periodic sync、craft session dirty/convergence、carrier periodic refresh；**不是 strict first-packet guarantee**。 |

关键边界：

1. `reemit_death_screen_for_reconnected_awaiting_revival_clients` 正确重放 `AwaitingRevival`；`NearDeath` 没有独立 death-screen，重连时 `Wounds::default()` 可能使其静默回 `Alive`。后者是已知 active-state 缺陷，属于 lifecycle/wounds 所有权，不在 R6 P0 改行为。
2. `mark_zone_environment_dirty_for_new_clients` 只是 dirty marker，实际 dedicated `bong:zone_environment` sender 是 `zone_environment_broadcast_system`；两者都需 pin，不能把 marker 冒充 sender。
3. realm vision、identity/quickslot/skillbar/unlocks、HUD/wounds/derived/status、weapon/treasure/spirit-treasure/false-skin/material-discovery 均由 join-time component attach/change 派生，不是直接 `Added<Client>`。
4. body/race/morph/craft/zone/tutorial/skill-config/audio/healer/carrier 依赖 per-client marker/cache miss；NPC mood / TSY boss health 还受 cadence 限制。
5. `emit_cultivation_detail_payloads`、morph periodic 等只能提供 eventual convergence，不能在 P1 被升级成首包承诺。

### 5.1 明确 exclusions

- `emit_join_alchemy_snapshots`：只在 `alchemy_join_mocks_enabled` 显式 mock 环境下发送。
- `emit_join_forge_snapshots`：`join hydration placeholder`，真实 forge snapshot 仍由 UI-open/request path 触发。
- `resourcepack::prompt_resource_pack_on_join`：原版资源包 prompt，不属于 ServerData replay。
- `identity/wanted_player_emit.rs`：Redis-only；`meridian_severed_emit.rs` / `tuike_ash_emit.rs`：domain-only。
- `client_request_handler.rs` 下的 C2S/request-response paths 归 R4；R6 不编辑。
- command/chunk/skin/native Valence state 及三个 client channel exemptions（agent UI request/close、dev shader）不进入 ServerData replay。
- 普通 event/change arms（break/attack/cast/VFX/audio delta、world add/remove 等）不因命名含 `emit` 就被当成 join guarantee。

P1 `JoinSnapshotKey` 只覆盖真正的 strict/join-derived/cache-driven 项；迁移不得借机改变 trigger 时序，也不得把 periodic convergence 标成 first-packet guarantee。

## 6. 所有权与 P0 不变量

- R6 P0 的生产改动仅收紧既有 `InventoryEventHandler.parseLocation` 的 equip wire 边界：`state` 必填，手持槽只接受 `HELD`，穿戴槽只接受 `WORN`；其余 production `.rs` / `.java` 零修改。
- R2 独占 `clearClientStateOnDisconnect`、Store lifecycle/gate；R6 的 receiver 扫描只读源码，不编辑这些区段。
- R3 独占 `server/src/persistence/**` 与 autosave；join 清单只登记网络 producer，不改变 hydration。
- R4 独占 `client_request_handler.rs`；`core_absorption_hallucination` 的 sender 等 R4 交付 API 后迁移。

## 7. P0 本地验收边界

- client：Java 17 下执行 `./gradlew test build -x runGametest --no-daemon`，完整 gate 通过。
- server：`wire_s2c_contract_pin` standalone lexical tests 18/18 通过；最终仍须在共享 `/tmp/bong-cargo.lock` 下完成 Cargo target test 与 server 全门禁。
- 安全隔离：本地未运行 `scripts/test-tmux-shutdown-order.sh`、`scripts/test-server-lifecycle.sh`，也未运行会间接调用两者的 suite；该覆盖留给 GitHub e2e。
- P0 除 `InventoryEventHandler.parseLocation` 的 equip state 边界外不改 production `.rs` / `.java`；其余改动仅涉及 contract 文档与 source/contract pin tests。
