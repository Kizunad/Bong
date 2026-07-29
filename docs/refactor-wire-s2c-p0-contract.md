# R6 P0 — S2C wire contract freeze

> 基线：2026-07-30，`plan-refactor-wire-s2c-v1` P0。本文只冻结后续 P1–P4 的接口与迁移账本；**不改变任何生产发送、过滤、序列化或 client dispatch 行为**。

## 1. 普查口径与可复现基线

### 1.1 client receiver 口径

权威口径是 `client/src/main/java/**/*.java` 中所有 `ClientPlayNetworking.registerGlobalReceiver(...)`，不只看 `BongNetworkHandler.register()`。扫描基线共 **32 个 receiver**：

- `BongNetworkHandler.java`：31 个（含唯一目标轨 `bong:server_data`）。
- `IrisBootstrap.java`：1 个（`bong:shader_state`）。
- 扣除 `bong:server_data` 后，旁路基线是 **31 个**。plan 侦察中的“28”是近似旧值，后续验收以测试锁定的 31 为准。

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

### 1.2 server emit 文件口径

权威口径是 `server/src/**/*_emit.rs`（递归，不把普通 `bridge.rs`/`resourcepack.rs` 算作 emit 文件）。基线共 **68 个**：

- 51 个仅走 `send_server_data_payload`。
- 13 个仅走专用 `send_custom_payload`。
- 1 个混合文件（`cultivation_detail_emit.rs`，其中 direct send 仍是 `bong:server_data`）。
- 2 个只发内部/Redis/VFX domain event（`meridian_severed_emit.rs`、`tuike_ash_emit.rs`）。
- 1 个纯 Redis sender（`identity/wanted_player_emit.rs`）。

P0 pin 测试冻结文件名全集与分类，避免把非 S2C 的 `*_emit.rs` 错迁，也避免新文件绕过 builder 迁移账本。

## 2. emit builder API 冻结（P1 实现目标）

重复模式由现有 emit 文件抽样归纳为：读取 ECS/event → 构造 `ServerDataV1` → `serialize_server_data_payload` → 选择 client → `send_server_data_payload` → 统一错误/trace。P1 的公共层固定在 `server/src/network/emit/`，最小公开契约如下（本节是设计签名，不是 P0 生产代码）：

```rust
pub enum EmitScope {
    Global,
    Dimension(DimensionKind),
    Zone {
        dimension: DimensionKind,
        zone: ZoneId,
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
| `Zone { dimension, zone }` | 先要求同维度，再以权威 `ZoneRegistry` + client `Position` 求 zone，结果等于强类型 `ZoneId` 才发送。zone ID 不能跨维单独匹配。 | 缺 dimension/position/registry 或无法解析 zone 时 fail closed。 |
| `Player(entity)` | 只发给目标 client entity；玩家换维不改变“发给该玩家”的语义。 | 目标不存在/不是 Client 时零发送；不得回退 broadcast。 |

额外约束：radius/view-distance 是 scope 命中后的**附加过滤器**，不能替代 `Dimension`；因此 VFX/audio 的 P1 顺序必须是“同维 → 半径/视距”。

## 4. proto enum normalization 冻结

所有 proto3 enum → legacy handler 字符串转换只允许发生在 `ProtoServerDataBridge`。冻结四种模式：

1. `snake_lower`：剥前缀后 `SCREAMING_SNAKE_CASE → snake_case`。
2. `capitalized`：剥前缀后单词首字母大写（`SHARP → Sharp`）；仅 Realm/Color 的既有 legacy consumer。
3. `pascal_case`：剥前缀后多段 PascalCase（`ZHENFA_WARD_ALERT → ZhenfaWardAlert`）。
4. `snake_lower_omit_unspecified`：与 1 相同，但 proto 默认 `*_UNSPECIFIED` 删除字段，让 legacy required-field gate fail closed。

P0 盘点到 **43 个唯一前缀字面量 / 57 处字面量引用**。`WireS2cContractPinTest` 锁定唯一前缀全集以及四个 helper/策略入口；P2 必须把 payload-specific 调用声明收敛成 bridge 内的单一 registry，禁止 handler 二次剥前缀。

## 5. join / reconnect 首包快照权威清单

权威清单由 R6 的 `server/tests/wire_s2c_contract_pin.rs` 维护，登记 producer symbol 与触发模型。不得放进 R2 的 Store lifecycle 文件，也不得让 R3 persistence 自己声明网络 replay。当前冻结项：

| producer | trigger | 首包内容 |
|---|---|---|
| `send_welcome_payload_on_join` | `Added<Client>` | welcome/协议握手状态 |
| `emit_player_state_payloads` | attach 后 `Added<PlayerState/Cultivation/...>` | player_state + season projection |
| `emit_join_inventory_snapshots` | `Added<PlayerInventory>` | inventory_snapshot |
| `emit_join_skill_snapshots` | `Added<SkillSet>` | skill_snapshot |
| `emit_join_techniques_snapshot_payloads` | `Added<KnownTechniques>` | techniques_snapshot |
| `emit_recipe_list_on_join` | per-client sent cache | craft recipe list + idle/active craft session |
| `emit_cultivation_detail_payloads` | 周期 emitter（在线后首个周期） | cultivation_detail |
| `emit_body_plan_layout_payloads` | `LastSentBodyPlanLayout` 缺失 | body_plan_layout |
| `emit_race_gate_meta_payloads` | `LastSentRaceGateMeta` 缺失 | race_gate_meta |
| `emit_morph_state_payloads` | `LastSentMorphStateJoin` 缺失 | morph full snapshot |
| `emit_join_dropped_loot_syncs` | `Added<Client>` | 当前世界掉落物全量 |
| `emit_join_remains_syncs` | `Added<Client>` | 当前遗骸全量 |
| `emit_rift_portal_state_payloads_to_joined_clients` | `Added<Client>` | 当前裂隙门全量 |
| `emit_container_state_payloads_to_joined_clients` | `Added<Client>` | 当前 TSY 容器全量 |
| `emit_tribulation_state_payloads` | known-client diff | 所有 active tribulation state |
| `emit_tribulation_broadcast_payloads` | known-client diff | 所有 active tribulation broadcasts |
| `emit_ascension_quota_payloads` | client count change | 当前化虚名额状态 |
| `on_player_join_send_spider_disguise_list` | `Added<Client>` | 视距内拟态蛛全量 |
| `on_player_join_send_daozhan_disguise_list` | `Added<Client>` | 视距内道伥伪装全量 |
| `on_player_join_send_rat_qi_tiers` | `Added<Client>` | 视距内噬元鼠档位全量 |
| `era_ambiance_on_join_system` | `Added<Client>` + realm gate | 当前时代天象 |
| `mark_zone_environment_dirty_for_new_clients` | `Added<Client>` | 标脏后由 env broadcaster 重发 |
| `send_tutorial_coffin_pos_on_join` | 延迟就绪 marker | 出生引导棺坐标 |
| `emit_coffin_state_to_joined_clients` | `Added<Client>` | 玩家棺状态 |
| `emit_anonymity_payloads_for_joined_clients` | `Added<Anonymity>` | 社交匿名状态 |

明确非快照：

- `emit_join_alchemy_snapshots` 当前仅显式 mock env 启用时发送，不能宣称生产 join hydration。
- `emit_join_forge_snapshots` 当前是空 placeholder，真实 forge 快照只在打开界面时发送。
- `resourcepack::prompt_resource_pack_on_join` 是协议 prompt，不属于 ServerData join snapshot，保留专用流程。

P1 实现 `JoinSnapshotKey` 时必须覆盖上表 ServerData/待收编项；迁移不能借机改变 producer 的业务触发时序。

## 6. 所有权与 P0 不变量

- R6 P0 只改本文与 pin tests；生产 `.rs` / `.java` 零修改。
- R2 独占 `clearClientStateOnDisconnect`、Store lifecycle/gate；R6 的 receiver 扫描只读源码，不编辑这些区段。
- R3 独占 `server/src/persistence/**` 与 autosave；join 清单只登记网络 producer，不改变 hydration。
- R4 独占 `client_request_handler.rs`；`core_absorption_hallucination` 的 sender 等 R4 交付 API 后迁移。
