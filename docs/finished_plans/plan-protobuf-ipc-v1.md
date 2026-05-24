# plan-protobuf-ipc-v1：Server↔Client Protobuf IPC 统一 Schema

> **一句话**：用 `.proto` 文件作为 server↔client CustomPayload 的 **唯一 source of truth**，prost（Rust）+ protobuf-java（Java）双端 codegen，消灭 106 个 S2C + 88 个 C2S payload 的手工对齐。

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | Proto 基建：目录结构 + codegen pipeline（新建 build.rs）+ CI 卡口 | ⬜ |
| P1 | 先锋迁移：10 个高频 payload 直接替换 | ⬜ |
| P2 | 批量迁移：剩余 ~184 个 payload | ⬜ |
| P3 | 割接：移除 JSON 旧路径 + 清理 | ⬜ |
| P4 | 饱和测试 + 跨端对拍 | ⬜ |
| P5 | 归档 | ⬜ |

---

## 接入面

- **进料**：`server/src/schema/*.rs`（60+ 文件，106 个 S2C variant `ServerDataPayloadV1` + 88 个 C2S variant `ClientRequestV1`）、`client/src/main/java/com/bong/client/network/`（65 handler 文件，`ServerDataRouter` 154 条注册）
- **出料**：`proto/bong/**/*.proto` → Rust generated structs（`server/src/schema/gen/`）+ Java generated classes（`client/build/generated/source/proto/`）
- **共享类型**：复用现有 `ServerDataPayloadV1` enum variant 命名；proto message name = 现有 variant name 的 PascalCase
- **跨仓库契约**：server（prost codegen via 新建 `build.rs`）↔ client（protobuf-gradle-plugin，当前 fabric-loom 1.6-SNAPSHOT + Java 17）；agent 层不受影响（agent↔server 仍走 TypeBox/Redis JSON）
- **Fabric channel**：client 侧 13 个 `ClientPlayNetworking.registerGlobalReceiver` 注册，主 channel `bong:server_data` 承载全部 S2C payload，另有 12 个专用 channel（NPC×3、TSY×2、VFX×1、Audio×3、环境×1、丹道×1、蝗灾×1）
- **worldview 锚点**：无直接锚点（纯基建），但 proto enum 命名必须遵循 worldview——`Realm` 枚举值 = `Awaken/Induce/Condense/Solidify/Spirit/Void`（代码现状，定义在 `server/src/cultivation/components.rs`）
- **qi_physics 锚点**：无

---

## 现状与动机

### 当前架构

```
Server (Rust)                                     Client (Java)
┌─────────────────────────────┐                   ┌──────────────────────────────────┐
│ ServerDataPayloadV1 (106)   │   JSON bytes      │ ServerDataEnvelope (Gson)         │
│ ClientRequestV1 (88 C2S)   │ ──────────────→   │ ServerDataRouter (154 注册)        │
│ serde tag="type" + flatten  │  bong:server_data  │ 65 个 *Handler.java 手写 parse    │
│ + 12 专用 channel           │  + 12 专用 channel │ 13 个 registerGlobalReceiver       │
└─────────────────────────────┘                   └──────────────────────────────────┘
                         ↑ 无共享 schema，各自独立定义 ↑
```

### 痛点

1. **手工对齐 194 个 payload**（106 S2C + 88 C2S）：Rust serde struct（`#[serde(tag = "type", rename_all = "snake_case")]`）和 Java Gson handler 各自独立手写，field 名/类型/顺序全靠人肉保证
2. **AI agent 易写偏**：AI 生成代码时最容易出现的 bug 就是两端不一致——加了字段忘同步、enum variant 拼写不同、数组顺序假设不同
3. **无编译期保证**：当前有 47 组 test fixture（`client/src/test/resources/bong/payloads/`），但覆盖率远低于 194 个 payload 总量
4. **enum wire name 漂移**：`SkillIdV1::Herbalism` → `"herbalism"` 靠 `#[serde(rename_all)]`，Java 端靠 `fromWire("herbalism")` 手写映射。部分 variant 有显式 `#[serde(rename)]` 覆盖（如 `QuickSlotConfig` → `"quickslot_config"`）
5. **并行数组陷阱**：`CultivationDetail` 有 **7 组并行数组**（opened/flow_rate/flow_capacity/integrity/open_progress/cracks_count + target_meridian index），各 20 元素，顺序与 `MeridianId` 判别式绑定（Lung=0..YangWei=19）。双端各有 order 测试但仍是运行时保证

### 目标架构

```
                proto/bong/*.proto
                (唯一 source of truth)
                    │
          ┌─────────┴──────────┐
          ▼                    ▼
   prost-build              protobuf-gradle-plugin
   (build.rs)               (build.gradle)
          │                    │
          ▼                    ▼
   server/src/gen/          client/src/gen/java/
   Rust structs             Java classes
   (编译器保证字段对齐)      (编译器保证字段对齐)
```

---

## P0：Proto 基建

### P0.1 目录结构

```
proto/
├── bong/
│   ├── common.proto          # 共享基础类型（RealmStage, SkillId, ColorKind, Vec3 等）
│   ├── envelope.proto        # ServerData 信封（替代 ServerDataV1 JSON envelope）
│   ├── cultivation.proto     # 修炼相关 payload
│   ├── inventory.proto       # 背包/物品
│   ├── combat.proto          # 战斗 HUD / 伤口 / 击退
│   ├── skill.proto           # 技能经验 / 等级
│   ├── alchemy.proto         # 炼丹
│   ├── craft.proto           # 工坊合成
│   ├── social.proto          # 社交系统
│   ├── npc.proto             # NPC 元数据 / 气泡 / 情绪
│   ├── zone.proto            # 区域信息 / 环境
│   ├── vfx.proto             # VFX 事件
│   ├── audio.proto           # 音频事件
│   ├── client_request.proto  # C2S 请求（客户端→服务器）
│   └── ...                   # 按领域继续拆分
└── buf.yaml                  # buf lint + breaking change 检测配置
```

### P0.2 Rust 端 codegen（prost）

- `server/Cargo.toml` 加依赖：`prost = "0.13"`, `prost-types = "0.13"`；`[build-dependencies]` 加 `prost-build = "0.13"`
- **新建 `server/build.rs`**（当前不存在）：`prost_build::Config` 编译 `proto/bong/**/*.proto`
- 输出到 `server/src/schema/gen/` 并 `include!` 引入（在 `server/src/schema/mod.rs` 的 60+ 平级 `pub mod` 中加 `pub mod gen;`）
- 当前项目非 Cargo workspace（单 crate），build.rs 配置简单直接
- 现有手写 struct 暂保留，新增 `From<ProtoType> for LegacyType` 转换（逐步替换用）

### P0.3 Java 端 codegen（protobuf-gradle-plugin）

- `client/build.gradle` 加 `com.google.protobuf` 插件 + `protobuf-java` 依赖（当前无任何 protobuf 配置，fabric-loom 1.6-SNAPSHOT + Java 17）
- `protobuf { protoc { ... } generateProtoTasks { ... } }` 配置指向 `proto/`
- 输出到 `client/build/generated/source/proto/`（Gradle 标准路径）
- 需确认与 fabric-loom 插件的 source set 兼容性（`sourceSets.main.java.srcDirs` 加 generated path）
- 现有 Gson handler 暂保留

### P0.4 CI 卡口

- `buf lint` 检查 proto 文件规范（命名、字段编号、包结构）
- `buf breaking --against .git#branch=main` 检测 proto 破坏性变更（删字段、改编号、改类型）
- 加到 `e2e.yml` 或新建 `proto-check.yml`，PR 阶段拦截

### P0.5 交付物

- [ ] `proto/bong/common.proto` — `Realm`（Awaken/Induce/Condense/Solidify/Spirit/Void）, `SkillId`, `ColorKind`, `MeridianId`（20 条，Lung=0..YangWei=19）, `Vec3`, `ItemSlot` 等共享类型
- [ ] `proto/bong/envelope.proto` — `ServerDataEnvelope` message，含 `oneof payload { ... }` 替代现有 `#[serde(tag = "type", flatten)]` 106 variant enum
- [ ] `server/build.rs`（**新建**）prost codegen 跑通
- [ ] `client/build.gradle` protobuf plugin + fabric-loom 兼容跑通
- [ ] `buf.yaml` + CI workflow step
- [ ] 两端 `cargo build` / `./gradlew build` 绿

---

## P1：先锋迁移（10 个高频 payload）

### 选取标准

选频率最高、结构从简到复杂覆盖各种模式的 10 个 payload：

| # | Payload | 复杂度 | 选取理由 |
|---|---------|--------|----------|
| 1 | `welcome` | 简单 flat | 最简单的 payload，验证管道通畅 |
| 2 | `narration` | 嵌套数组 | 含 `Vec<NarrationEntry>` |
| 3 | `zone_info` | 中等 flat | 高频，每次进 zone 触发 |
| 4 | `player_state` | 中等，含 enum | `RealmStage` enum 序列化验证 |
| 5 | `skill_xp_gain` | tagged union | `XpGainSource` oneof 验证 |
| 6 | `inventory_snapshot` | 重型嵌套 | `Vec<ItemSlot>` 含可选嵌套 |
| 7 | `cultivation_detail` | 并行数组 | **最易出 bug 的 payload**，20 经脉并行数组 |
| 8 | `combat_hud_state` | 中等 | 战斗 HUD 高频推送 |
| 9 | `knockback_sync` | 含 optional | `Option<f32>` 序列化 |
| 10 | `client_request` | C2S 方向 | 验证反向通道 |

### P1.1 Proto 定义

为上述 10 个 payload 编写 `.proto` message 定义。关键设计决策：

- **经脉用 message 不用并行数组**：`CultivationDetail` 的 20 条经脉从 `repeated bool opened` + `repeated double flow_rate`（并行数组，顺序隐式绑定）改为 `repeated MeridianState meridians`（每条经脉一个 message，id 显式标注），根治并行数组错位问题
- **enum 命名遵循 worldview**：`RealmStage` 的值 = `REALM_STAGE_XINGLING` / `REALM_STAGE_YINQI` / ...（拼音 + 全大写，protobuf enum 惯例）
- **oneof 替代 tagged union**：`XpGainSource` 从 JSON `{ "type": "action", ... }` 改为 protobuf `oneof source { ActionSource action = 1; ScrollSource scroll = 2; ... }`
- **字段编号稳定**：一旦分配，永不复用。buf breaking 检测保证

### P1.2 直接替换（无双格式并行）

Server 和 client 同仓同步更新，不需要版本兼容，直接替换：

- **Server**：已迁移的 payload 直接改为 protobuf 序列化，走 `bong:server_data`（binary 替代 JSON）
- **Client**：对应 handler 直接改为 protobuf 反序列化，移除 Gson 解析
- 未迁移的 payload 暂保留 JSON 旧路径，两种格式在同一 channel 共存（信封头区分）

### P1.3 对拍测试

每个迁移的 payload 新增 roundtrip 测试：
- Server 端：构造 payload → protobuf serialize → deserialize → assert 全字段一致
- Client 端：加载 protobuf binary fixture → deserialize → assert 字段值符合预期
- 共享 fixture：`proto/testdata/` 放 binary protobuf fixture，双端各自加载验证

### P1.4 交付物

- [ ] 10 个 payload 的 `.proto` 定义
- [ ] Server 端已迁移 payload 直接走 protobuf 序列化
- [ ] Client 端已迁移 payload 直接走 protobuf 反序列化，移除对应 Gson handler
- [ ] 10 组 roundtrip 测试全绿
- [ ] `cultivation_detail` 经脉并行数组 → `repeated MeridianState` 验证通过

---

## P2：批量迁移

### P2.1 剩余 payload 分批

按领域分 5 批，剩余约 184 个 payload（194 总 - 10 先锋）：

| 批次 | 领域 | S2C | C2S | 估算合计 |
|------|------|-----|-----|----------|
| B1 | 炼丹 + 锻造 + 工坊 + 采集 + 灵田 | ~20 | ~15 | ~35 |
| B2 | 战斗 + 击退 + 伤口 + 毒 + 载体 + 暗器 | ~25 | ~20 | ~45 |
| B3 | 社交 + NPC + 身份 + 交易 + 领地 | ~15 | ~15 | ~30 |
| B4 | 天劫 + 死亡 + 复活 + 突破 + 境界视觉 | ~15 | ~10 | ~25 |
| B5 | VFX + 音频 + 区域环境 + 杂项 + 12 专用 channel | ~20 | ~30 | ~50 |

每批流程同 P1：写 proto → codegen → 直接替换 → roundtrip 测试 → 绿灯。

### P2.2 专用 channel 迁移（12 个）

除主 `bong:server_data` 外的 12 个专用 Fabric channel 也迁 protobuf：
- `bong:npc_metadata` / `bong:npc_bubble` / `bong:npc_mood` → `proto/bong/npc.proto`
- `bong:tsy_boss_health` / `bong:tsy_death_vfx` → `proto/bong/tsy.proto`
- `bong:vfx_event` → `proto/bong/vfx.proto`
- `bong:audio/play` / `bong:audio/stop` / `bong:audio/ambient_zone` → `proto/bong/audio.proto`
- `bong:zone_environment` → `proto/bong/zone.proto`
- `bong:mutation_visual` → `proto/bong/dandao.proto`
- `bong:locust_swarm_warning` → `proto/bong/event.proto`
- `bong:client_request`（C2S，88 variants）→ `proto/bong/client_request.proto`

### P2.3 交付物

- [ ] 全部 194 payload variants（106 S2C + 88 C2S）有对应 proto message
- [ ] `bong:server_data` 全量走 protobuf binary
- [ ] 12 个专用 Fabric channel 也走 protobuf
- [ ] roundtrip 测试全覆盖

---

## P3：割接与清理

### P3.1 移除 JSON 残留

- Server：移除 `ServerDataPayloadWireV1`、`#[serde(rename)]` 等 JSON 序列化辅助
- Client：移除 `ServerDataEnvelope` JSON 解析、`ServerDataRouter` 字符串路由、所有 Gson handler
- 确认 `bong:server_data` channel 全量走 protobuf binary

### P3.2 清理遗留

- 删除 `server/src/schema/server_data.rs` 中 106 个手写 S2C variant + `client_request.rs` 中 88 个 C2S variant（由 codegen 替代）
- 删除 `client/src/main/java/com/bong/client/network/` 下 65 个手写 handler 文件 + `ServerDataRouter` 154 条字符串路由（由 generated message class 替代）
- 47 组 test fixture（`client/src/test/resources/bong/payloads/`）转为 protobuf binary fixture
- 更新 CLAUDE.md 架构说明

### P3.3 交付物

- [ ] JSON 旧路径全部移除（`ServerDataPayloadWireV1` / `ServerDataEnvelope` / `ServerDataRouter` / 65 handler 文件）
- [ ] `cargo build` / `./gradlew build` / `npm test` 全绿
- [ ] 无 JSON 序列化残留（grep `serde_json` in schema/ = 0，仅 agent IPC 保留）

---

## P4：饱和测试

### P4.1 跨端 roundtrip 测试

每个 proto message 至少一组：Server 构造 → serialize → Client deserialize → assert 全字段一致。

### P4.2 schema evolution 测试

- 新增 optional 字段：老 client 能正常反序列化（忽略未知字段）
- 删除 deprecated 字段：新 client 对老 server 数据不 crash
- enum 新增值：`UNRECOGNIZED` 处理

### P4.3 性能基线

- 对比 JSON vs protobuf 序列化耗时（175 payload roundtrip benchmark）
- 对比 payload 大小（预期 protobuf binary 约为 JSON 的 30-50%）

### P4.4 buf breaking CI 验证

- 故意提交一个删除字段的 PR → CI 红 → 验证卡口有效

### P4.5 交付物

- [ ] 194 组 roundtrip 测试（106 S2C + 88 C2S）
- [ ] schema evolution 测试（新增/删除/enum 扩展）
- [ ] 性能 benchmark 结果
- [ ] buf breaking CI 验证

---

## P5：归档

- [ ] 填写 `## Finish Evidence`
- [ ] `git mv docs/plan-protobuf-ipc-v1.md docs/finished_plans/`

---

## §8 开放问题（P0 决策门前需收口）

全部已在 §8.1 收口。原表保留以备追溯，**实施时以 §8.1 决议为准**。

### #1 agent↔server 是否也迁 protobuf？

~~当前 agent↔server 走 TypeBox/Redis JSON，独立于 server↔client。是否统一？~~

### #2 protobuf 版本：proto3 vs proto2？

~~默认值语义、optional 支持、社区方向~~

### #3 buf vs protoc？

~~lint + breaking change detection vs 原生编译器~~

### #4 channel 命名

~~双格式并行期 channel 命名~~

### #5 CultivationDetail 经脉数据结构

~~proto 层改结构时是否同时重构 server 内部 ECS component~~

### #6 版本兼容策略

~~是否需要双格式并行期~~

---

## §8.1 决议（pre-P0 收口，2025-05-23）

### #1 agent↔server 不迁

**决议**：
1. agent↔server 保持 TypeBox/Redis JSON 不动，本 plan 只管 server↔client CustomPayload
2. 两条 IPC 通道场景完全不同——Redis pub/sub（异步、大 payload、低频）vs MC CustomPayload（同步、小 payload、高频），没必要强统一
3. TypeScript 端 protobuf-ts 生态远不如 TypeBox 轻便（TypeBox = 零 codegen、纯类型推导），换了反而退步

**落点**：本 plan 范围限定为 `server/src/schema/`（60+ .rs 文件）↔ `client/src/main/java/com/bong/client/network/`（65 handler 文件 + `ServerDataRouter` 154 注册）。`agent/packages/schema/`（TypeBox）不动。

### #2 使用 proto3

**决议**：
1. 使用 `syntax = "proto3";`
2. 需要显式 presence 的字段用 `optional` 关键字（proto3 3.15+ 支持），对应 Rust `Option<T>` / Java `hasXxx()`
3. 不用 proto2——`required` 已被社区弃用，且 prost 对 proto3 支持最成熟

**落点**：所有 `proto/bong/*.proto` 文件头 `syntax = "proto3";`。`prost-build` 默认 proto3，无需额外配置。server 端 Rust edition 2021（`server/Cargo.toml`），prost 0.13 兼容。

### #3 buf + protoc 组合使用

**决议**：
1. **codegen 用 protoc**（prost-build 底层调 protoc，protobuf-gradle-plugin 也调 protoc）——这是编译器，不可替代
2. **lint + breaking change 检测用 buf**——`buf lint` 强制命名规范、`buf breaking --against .git#branch=main` CI 阶段拦截破坏性变更
3. 不用 buf 做 codegen（buf generate）——prost-build 和 gradle plugin 各自有成熟集成，多一层反而增加维护成本

**落点**：`proto/buf.yaml` 配置 lint rules + breaking 策略。CI workflow 加 `buf lint` + `buf breaking` step，放在 codegen 之前。

### #4 channel 保持原名 `bong:server_data`

**决议**：不需要双格式并行，channel 保持 `bong:server_data` 原名，内容从 JSON 直接改为 protobuf binary。

**落点**：`server/src/schema/channels.rs` 中 channel 名不变。`bong:server_data` 在 `server/src/network/mod.rs` 的 `send_server_data_payload()` 中通过 `ident!("bong:server_data")` 发送，client 在 `BongNetworkHandler.java` 的 `registerServerDataChannel()` 注册——两端 channel 名保持不变，只改 payload 编码格式。

### #5 proto 层改结构，server 内部本 plan 不动

**决议**：
1. Proto 层 `CultivationDetail` 从并行数组改为 `repeated MeridianState meridians`（每条经脉一个 message，`MeridianId` 显式字段），根治错位问题
2. Server 内部 ECS component 暂保持并行数组——在发送 proto 时做 `Vec<bool>` → `Vec<MeridianState>` 转换
3. Server 内部重构为 `Vec<MeridianState>` 是独立技术债，不在本 plan 范围，可后续单独立 plan

**落点**：
- Proto 层：`proto/bong/cultivation.proto` 定义 `message MeridianState { MeridianId id = 1; bool opened = 2; double flow_rate = 3; double flow_capacity = 4; double integrity = 5; double open_progress = 6; uint32 cracks_count = 7; }`
- Server 转换层：在 `server/src/network/cultivation_detail_emit.rs`（现有 `emit_cultivation_detail_payloads` system，行 78-95 的 `for m in meridians.regular.iter().chain(meridians.extraordinary.iter())` 循环）中，从并行数组构建改为 `repeated MeridianState` 构建
- Client 解析层：`CultivationDetailHandler.java` 的 `CHANNEL_ORDER[20]` 静态映射 + index 解析循环（行 157-178）替换为 proto generated class 的 `getMeridiansList()` 遍历
- 现有双端 order 测试（Server: `cultivation_detail_roundtrip_and_size_budget`，Client: `channelOrderExactly20()` + `targetMeridianExtraordinary()`）替换为 proto message field 测试

### #6 不需要双格式并行

**决议**：Server 和 client 同仓同步更新，直接替换。省掉并行期工作量。

---

## §10 实施工作流

### §10.1 多 PR 序列化

1. **PR-1 P0 基建**：proto 目录 + codegen pipeline + CI → 独立 PR，不动业务代码
2. **PR-2 P1 先锋迁移**：10 个高频 payload proto + 直接替换 + roundtrip 测试
3. **PR-3~7 P2 批量迁移**：每批一个 PR（B1~B5）
4. **PR-8 P3 割接清理**：移除 JSON 旧路径
5. **PR-9 P4 饱和测试**：补齐 roundtrip + evolution + benchmark

### §10.2 subagent 配置

```
Agent(
  subagent_type: "claude",
  model: "opus",
  prompt: "...任务...\n\nultrathink"
)
```

### §10.3 CodeRabbit 等待协议

按 `docs/CLAUDE.md` §6.5 标准执行。
