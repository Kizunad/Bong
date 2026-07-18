# plan-bughunt-niche-guardian-proto-kind

> **Active（2026-07-18 历史归档修复升格）**。本文件从 `c301899575c0f918748556a57e6daf4166a942d7^:docs/plans-skeleton/plan-bughunt-niche-guardian-proto-kind.md` 恢复；原 PR #1186 合并时直接删除 skeleton，未完成正规 promotion / 归档。本次先恢复并升格，状态以当前主线第一性原理核验结果为准，不追认当年已正确 promotion。

## 阶段总览

| 阶段 | 主题 | 状态 | 验收日期 |
|------|------|------|----------|
| P0 | 修复灵龛守护 `guardian_kind` proto enum 归一并锁定真实 proto 链路 | ⏳ | 待核验 |

## Bug 摘要

- **核心 bug**：`ProtoServerDataBridge` 已把 `NICHE_GUARDIAN_FATIGUE` / `NICHE_GUARDIAN_BROKEN` 映射到 legacy type string，但没有像 `social_exposure.kind` 那样对顶层 enum 字段做 `bridgeStripEnums(..., "guardian_kind", "GUARDIAN_KIND_")`。
- **根因形态**：server 生产路径走 protobuf bytes；Java bridge 使用 protobuf canonical JSON 打印 enum value。`GuardianKind` 的 proto value 是 `GUARDIAN_KIND_*`，generic bridge 不做 enum 归一，handler/store/HUD 又直接消费字符串。
- **覆盖缺口**：现有 client social 测试只喂 legacy JSON `"guardian_kind":"puppet"`，没有覆盖真实 proto bytes -> bridge -> router -> store/HUD。
- **不重复范围**：本问题不是 #974 丹方残卷、#988 给丹入口、#994 C2S 共享 schema、#999 炼器 C2S、#1010 季节状态 enum，也不是 #945 灵龛守护 HUD 跨 session 串局；本题只聚焦灵龛守护 S2C proto enum 前缀泄漏。

## 对实际游玩体验的影响

玩家布置灵龛守护后，一旦入侵者触发傀儡 / 阵法陷阱 / 道香守护，服务端会正常发出守护损耗或破损事件。但客户端事件流会显示类似：

- `守家载体损耗：GUARDIAN_KIND_PUPPET 剩余 4 次`
- `守家载体破损：GUARDIAN_KIND_ZHENFA_TRAP`
- 常驻灵龛守护 HUD 行：`GUARDIAN_KIND_PUPPET x4`

这不是内部日志问题。玩家正是在被入侵、需要判断哪种守护载体触发或耗尽时看到裸 proto 常量，直接破坏灵龛防御反馈的可读性，也会让带下划线的 `ZHENFA_TRAP` / `BONDED_DAOXIANG` 看起来像调试信息泄漏。

## 证据定位

- 生产 server-data wire 已切到 protobuf：`server/src/network/agent_bridge.rs:41-75` 在 `cfg(not(test))` 下调用 `to_proto_bytes_checked(payload)`。
- 灵龛守护是生产系统，不是测试夹具：`server/src/social/niche_defense.rs:64-76` 注册 `NicheGuardianFatigue` / `NicheGuardianBroken` 事件；`server/src/social/niche_defense.rs:177-192` 消耗守护 charge 并记录 fatigue/break；`server/src/social/niche_defense.rs:285-299` 发出对应事件。
- server S2C 构造真实 payload：`server/src/social/mod.rs:705-728` 把事件转成 `NicheGuardianFatigueV1` / `NicheGuardianBrokenV1`；`server/src/social/mod.rs:738-755` 发送给 owner / intruder。
- proto enum value 带前缀：`proto/bong/envelope.proto:2025-2029` 定义 `GUARDIAN_KIND_PUPPET`、`GUARDIAN_KIND_ZHENFA_TRAP`、`GUARDIAN_KIND_BONDED_DAOXIANG`；`proto/bong/envelope.proto:2142-2152` 两个 S2C message 都把 `guardian_kind` 定义为 `GuardianKind`。
- legacy/schema 约定是 snake_case：`server/src/schema/social.rs:22-28` 的 `GuardianKindV1` 使用 `#[serde(rename_all = "snake_case")]`；schema samples 也写 `"guardian_kind": "puppet"`。
- bridge 漏修：`client/src/main/java/com/bong/client/network/ProtoServerDataBridge.java:113-115` 只注册 type；`client/src/main/java/com/bong/client/network/ProtoServerDataBridge.java:313-421` 的 enum fixup 列表没有 `NICHE_GUARDIAN_*`；随后 `client/src/main/java/com/bong/client/network/ProtoServerDataBridge.java:423-439` 走 generic path；`client/src/main/java/com/bong/client/network/ProtoServerDataBridge.java:1452-1458` 的 `printAndNormalize` 只做 numeric string 归一，不处理 enum。
- client 消费端原样展示：`client/src/main/java/com/bong/client/network/ServerDataRouter.java:221-223` 路由到 `SocialServerDataHandler`；`client/src/main/java/com/bong/client/network/SocialServerDataHandler.java:222-240` 读取 `guardian_kind` 后原样传给 `NicheIntrusionAlertHandler`；`client/src/main/java/com/bong/client/social/NicheGuardianStore.java:36-60` 只 trim，不归一；`client/src/main/java/com/bong/client/social/NicheIntrusionAlertHandler.java:55-74` 把 raw kind 写进玩家事件文本；`client/src/main/java/com/bong/client/social/NicheGuardianPanel.java:11-17` 把 raw key 写进 HUD 行。
- 测试缺口明确：`client/src/test/java/com/bong/client/network/SocialServerDataHandlerTest.java:141-154` 只覆盖 legacy JSON `"puppet"`；`client/src/test/java/com/bong/client/network/ProtoServerDataBridgeTest.java:2080-2092` 已证明同类 `EXPOSURE_KIND_DIVINE -> divine` 需要 bridge 专门剥前缀，但没有 niche guardian 对应用例。

## 触发路径

1. 玩家激活任一灵龛守护载体，例如 `puppet` 或 `zhenfa_trap`。
2. 入侵者触发灵龛防御，`handle_niche_intrusion_attempts` 消耗守护 charge，产生 `NicheGuardianFatigue`，charge 归零时再产生 `NicheGuardianBroken`。
3. server 生产环境把 `NicheGuardianFatigueV1` / `NicheGuardianBrokenV1` 编成 protobuf bytes，通过 `bong:server_data` 下发。
4. client `ProtoServerDataBridge` 将 payload 打印成 JSON，但 `guardian_kind` 保持 proto enum value，如 `GUARDIAN_KIND_PUPPET`。
5. `SocialServerDataHandler`、`NicheGuardianStore`、`NicheIntrusionAlertHandler`、`NicheGuardianPanel` 全链路不再归一化，最终玩家可见 HUD / 事件文本泄漏 proto 常量。

## 反方审查记录

### Round 1

- **反方质疑**：这是否只是测试或死代码？是否已有 handler/store normalization 抵消？是否和 #945 或 #1010 重复？影响是否只是内部日志？
- **审查结论**：通过。`niche_defense` 注册和 `social/mod.rs` S2C emit 都是生产路径；bridge 无 `GUARDIAN_KIND_` fixup；handler/store 只 trim；`NicheIntrusionAlertHandler` 和 `NicheGuardianPanel` 都把 raw string 写到玩家可见事件/HUD。#1010 是 `season_state.season`，#945 是跨 session 清理，均不同。

### Round 2

- **反方质疑**：修复应落在 bridge 还是 handler/store？Redis/agent handoff 是否同样要纳入？protobuf JSON 是否可能自动输出 lower/snake 而不是 `GUARDIAN_KIND_*`？
- **审查结论**：通过。最小修复应在 `ProtoServerDataBridge`，因为问题源头是 proto -> legacy JSON 桥接；把 protobuf常量知识塞进 `SocialServerDataHandler` / `NicheGuardianStore` 会污染 legacy schema 消费端。Redis outbound 对该事件走 Rust serde JSON（`server/src/network/redis_bridge.rs:1221-1229`），不是 Java proto bridge，非本 bug 主线。`JsonFormat.printer()` 未启用 enum-as-int，字段名 lower/case 配置不改变 enum value 名；同类 bridge tests 已按前缀剥离建模。

## P0 - 修复灵龛守护 guardian_kind proto enum 归一

- 在 `client/src/main/java/com/bong/client/network/ProtoServerDataBridge.java` 的 enum fixup 区加入两个 simple-field special case：
  - `NICHE_GUARDIAN_FATIGUE`：`guardian_kind` 从 `GUARDIAN_KIND_PUPPET` 剥成 `puppet`，从 `GUARDIAN_KIND_ZHENFA_TRAP` 剥成 `zhenfa_trap`。
  - `NICHE_GUARDIAN_BROKEN`：同样剥 `guardian_kind`。
- 复用现有 `bridgeStripEnums` / `stripEnumPrefix`，不要在 `SocialServerDataHandler` 或 `NicheGuardianStore` 里新增 protobuf enum 知识。
- 保持 legacy JSON 输入兼容：已经是 `puppet` / `zhenfa_trap` 的 payload 不应被二次改写。
- 不改 Rust schema、TypeBox schema、proto 定义、Redis channel 或资源文件。

## 验收测试计划

- 新增 `ProtoServerDataBridgeTest`：
  - 构造 `Envelope.NicheGuardianFatigue`，`guardian_kind = GUARDIAN_KIND_PUPPET`，断言 bridge 后 JSON `type == "niche_guardian_fatigue"` 且 `guardian_kind == "puppet"`。
  - 构造 `Envelope.NicheGuardianBroken`，`guardian_kind = GUARDIAN_KIND_ZHENFA_TRAP`，断言 bridge 后 `guardian_kind == "zhenfa_trap"`，覆盖带下划线枚举。
  - 可选加 `GUARDIAN_KIND_BONDED_DAOXIANG -> bonded_daoxiang`，防止多段 snake_case 漏测。
- 新增或扩展 route-level 回归：
  - proto bytes -> `ProtoServerDataBridge.bridge` -> `ServerDataRouter.route` 后，`NicheGuardianStore.guardianStatuses()` 只出现 `puppet` / `zhenfa_trap` key，不出现 `GUARDIAN_KIND_*` key。
  - `NicheGuardianPanel.buildLines()` 与 `UnifiedEventStore` 文本不包含 `GUARDIAN_KIND_`。
- 保留 existing legacy JSON tests，确保 `"guardian_kind":"puppet"` 仍可直接通过 `SocialServerDataHandler`。
- client 栈验收命令按仓库矩阵跑 `cd client && ./gradlew test build`；如后续要做完整联调，再在仓库根按约定跑 `export BONG_SKIP_SKIN_PREFETCH=1 && bash scripts/smoke-test-e2e.sh`。

## 风险

- **大小写风险**：不能用 `stripEnumPrefixCapitalized`，否则会得到 `Puppet` / `Zhenfa_trap`，与 legacy schema 的 snake_case 不符；应使用现有小写剥离 helper。
- **未知枚举风险**：`GUARDIAN_KIND_UNSPECIFIED` 理论上会剥成 `unspecified`。如果生产不应出现 unspecified，测试可只 pin 合法三值；不要在本修复里扩大语义。
- **修复边界风险**：不要顺手解决 #945 的 store 跨 session 清理，也不要把 #1010 的 season_state 修复混进同一 PR；本 skeleton 只处理 niche guardian proto enum 前缀。
- **覆盖风险**：只测 `PUPPET` 会漏掉 `ZHENFA_TRAP` / `BONDED_DAOXIANG` 的 snake_case 转换，因此至少要覆盖一个带下划线的 guardian kind。
