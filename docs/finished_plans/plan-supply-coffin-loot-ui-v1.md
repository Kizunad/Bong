# Bong · plan-supply-coffin-loot-ui-v1

**物资棺塔科夫式搜刮 UI——从"开即碎"到会话式交互搜刮**。改造 `plan-supply-coffin-v1` 的即时取物模型为双栏拖拽 UI：左侧玩家容器列表，右侧棺材 loot 格子，双向拖拽；三档棺材三种格子尺寸（Common 4×3 / Rare 5×4 / Precious 6×5）；打开后倒计时，超时碎裂剩余物品销毁。框架设计为通用 ExternalContainer，供未来箱子/商人复用。

| 阶段 | 状态 | 验收日期 |
|------|------|----------|
| P0 ExternalContainer 框架 + `/supply_coffin tp` | ✅ | 2026-05-27 |
| P1 网络协议层 | ✅ | 2026-05-28 |
| P2 Server interact 改造 + lifecycle + handler | ✅ | 2026-05-28 |
| P3 Client LootContainerScreen | ✅ | 2026-05-28 |
| P4 开棺专属音效 | ✅ | 2026-05-28 |

**前置依赖**：
- `plan-supply-coffin-v1` ✅ — 三档棺材基础模型、loot 表、刷新系统
- `plan-inventory-v1` ✅ — `ContainerState` / `PlayerInventory` / `BackpackGridPanel`

**世界观锚点**：同 `plan-supply-coffin-v1`（纯凡物容器，不涉及真元流动）

---

## P0: ExternalContainer 框架

- `ExternalContainer` component（session_id, container, opened_by, timeout_wall_secs, source_kind）
- `ExternalContainerRegistry` resource（session 分配 + 查找）
- `pack_loot_into_grid()` — row-major first-fit 格子填充
- `SupplyCoffinGrade::loot_grid()` / `loot_timeout_secs()` 常量方法
- `/supply_coffin tp` dev 命令

## P1: 网络协议层

- S2C: `LootContainerOpen` / `LootContainerUpdate` / `LootContainerClose`（`ServerDataPayloadV1` 新 variant）
- C2S: `ExternalContainerMove` / `ExternalContainerClose`（`ClientRequestV1` 新 variant）
- Proto binary: `ServerDataEnvelope` fields 119-121, `ClientRequestEnvelope` fields 85-86
- Client: `LootContainerHandler` / `ClientRequestProtocol.encodeExternalContainerMove/Close` / `ClientRequestSender`

## P2: Server interact 改造 + lifecycle

- `interact.rs` 重写：右键 → roll → pack → attach ExternalContainer → send LootContainerOpen
- `lifecycle.rs` 新系统：timeout → despawn+cooldown / distance >6 → close / disconnect → release lock
- `client_request_handler.rs`: `handle_external_container_move`（跨容器拖拽 + 完整回滚）/ `handle_external_container_close`
- 互斥锁：`opened_by` 防双人同开

## P3: Client LootContainerScreen

- `LootContainerScreen` (411 行) — BaseOwoScreen 分栏 UI
- `LootContainerStateStore` — volatile session + listener pattern
- `LootContainerScreenBootstrap` — 自动开关 screen
- 拖拽：共享 DragState，跨容器高亮 + drop + move request
- 倒计时：进度条 + 剩余秒数标签，≤10s 变红，0s 自动关闭

## P4: 开棺音效

- 3 个 `supply_coffin_open_{common,rare,precious}` audio recipe
- Common: chest_open + wood_hit
- Rare: + gravel_step
- Precious: + amethyst_block.chime

---

## Finish Evidence

**验收**：2026-05-28 全部 P0-P4 ✅，115 个本 plan 相关单测通过，
server 6668 tests 全绿，client 1726 tests 全绿，clippy `--all-targets -D warnings` 干净。

### 落地清单（每阶段 ↔ 真实文件）

| 阶段 | 模块 / 文件 | 关键 symbol |
|------|-------------|-------------|
| P0 框架 | `server/src/inventory/external_container.rs` | `ExternalContainer` / `ExternalContainerRegistry` / `ExternalContainerKind` / `pack_loot_into_grid` / `place_item_into_container` / `remove_item_from_container` |
| P0 grade 扩展 | `server/src/supply_coffin/mod.rs` | `SupplyCoffinGrade::loot_grid()` / `loot_timeout_secs()` / `as_str()` |
| P0 tp 命令 | `server/src/cmd/dev/supply_coffin.rs` | `SupplyCoffinCmd::Tp` |
| P1 S2C schema | `server/src/schema/server_data.rs` | `LootContainerOpenV1` / `LootContainerUpdateV1` / `LootContainerCloseV1` / `LootContainerCloseReasonV1` / `LootContainerSourceKindV1` |
| P1 C2S schema | `server/src/schema/client_request.rs` | `ClientRequestV1::ExternalContainerMove` / `ExternalContainerClose` |
| P1 proto | `proto/bong/envelope.proto` | `ServerDataEnvelope` fields 119-121 / `ClientRequestEnvelope` fields 85-86 |
| P1 proto convert | `server/src/schema/proto_convert.rs` | S2C/C2S encode/decode 双向转换 |
| P1 client handler | `client/.../network/LootContainerHandler.java` | `handleOpen` / `handleUpdate` / `handleClose` / `parsePlacedItems` |
| P1 client protocol | `client/.../network/ClientRequestProtocol.java` | `encodeExternalContainerMove` / `encodeExternalContainerClose` |
| P1 client sender | `client/.../network/ClientRequestSender.java` | `sendExternalContainerMove` / `sendExternalContainerClose` |
| P1 client router | `client/.../network/ServerDataRouter.java` | 3 new handler registrations |
| P2 interact | `server/src/supply_coffin/interact.rs` | 重写为 session-based flow（roll → pack → attach → send） |
| P2 lifecycle | `server/src/supply_coffin/lifecycle.rs` | `external_container_lifecycle_tick`（timeout/distance/disconnect） |
| P2 handler | `server/src/network/client_request_handler.rs` | `handle_external_container_move` / `handle_external_container_close` / `resync_ext_and_inventory` / `resync_inventory_only` |
| P2 agent bridge | `server/src/network/agent_bridge.rs` | `LootContainerOpen/Update/Close` payload_type_label |
| P3 screen | `client/.../inventory/LootContainerScreen.java` | 411 行分栏拖拽 UI |
| P3 state store | `client/.../hud/LootContainerStateStore.java` | `OpenSession` / `Closed` / listener pattern |
| P3 bootstrap | `client/.../inventory/LootContainerScreenBootstrap.java` | 自动 open/close screen |
| P3 BongClient | `client/.../BongClient.java` | `LootContainerScreenBootstrap.register()` |
| P4 audio | `server/assets/audio/recipes/supply_coffin_open_{common,rare,precious}.json` | 3 个开棺音效 recipe |

### 关键 commit

| hash | 日期 | 说明 |
|------|------|------|
| `1a2f93e1f` | 2026-05-27 | feat: Supply Coffin ExternalContainer framework + /supply_coffin tp (#337) |
| `cf0bac28b` | 2026-05-28 | feat: Supply Coffin Loot UI P1-P3 — protocol, interact, lifecycle, screen (#338) |
| `2fd874064` | 2026-05-28 | feat: Supply Coffin P4 — open sound recipes per grade (#339) |

### 测试结果

| 命令 | 结果 |
|------|------|
| `cargo test`（server 全量） | 6668 passed / 0 failed |
| `cargo test -- external_container loot_container supply_coffin` | 115 passed（含 P0 框架 23 + P1 schema 28 + P1 proto 5 + P2 lifecycle/handler + server_data 19 + 原 supply_coffin 40） |
| `cargo clippy --all-targets -- -D warnings` | 0 warnings |
| `cargo fmt --check` | 0 diffs |
| `./gradlew test build`（client） | 1726 passed / BUILD SUCCESSFUL |

### 跨仓库核验

- **server**: `inventory::external_container` 模块 / `supply_coffin::{interact,lifecycle,mod}` / `network::client_request_handler` (ExternalContainerMove/Close) / `schema::{server_data,client_request,proto_convert}` (LootContainer* + ExternalContainer*) / `network::agent_bridge` (3 payload_type_label) / 7 audio recipe JSON (3 open + 4 原有)
- **client**: `LootContainerHandler` / `LootContainerStateStore` / `LootContainerScreen` / `LootContainerScreenBootstrap` / `ClientRequestProtocol` (2 encode) / `ClientRequestSender` (2 send) / `ServerDataRouter` (3 handler 注册) / `BongClient` (bootstrap 注册)
- **agent**: 不参与（纯本地循环）
- **proto**: `envelope.proto` fields 119-121 (S2C) + 85-86 (C2S)

### 遗留 / 后续

| 项 | 说明 |
|----|------|
| handler 函数无直接 ECS 集成测试 | `handle_external_container_move` / `handle_external_container_close` 仅有 schema roundtrip 测试间接覆盖；需构建 test ECS world 做行为测试 |
| `LootContainerScreen` 无自动化 UI 测试 | 411 行拖拽逻辑依赖实机 `/supply_coffin spawn` 手动验证 |
| 棺内物品 tooltip / 详情弹窗 | 当前拖拽只显示格子图标，无 hover 详情 |
| 多人同棺抢夺 | 当前互斥锁仅允许单人开棺，未来可扩展竞争模式 |
