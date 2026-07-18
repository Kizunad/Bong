# plan-bughunt-niche-guardian-proto-kind-v1

> **Finished（2026-07-18）**。本文件从 `c301899575c0f918748556a57e6daf4166a942d7^:docs/plans-skeleton/plan-bughunt-niche-guardian-proto-kind.md` 恢复；原 PR #1186 合并时直接删除 skeleton，未完成正规 promotion / 归档。本次经正规升格、第一性原理复核、边界返工与对抗验证后归档，不追认当年已正确 promotion。

## 阶段总览

| 阶段 | 主题 | 状态 | 验收日期 |
|------|------|------|----------|
| P0 | 修复灵龛守护 `guardian_kind` proto enum 归一并锁定真实 proto 链路 | ✅ 2026-07-18 | 2026-07-18 |

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

## P0 - 修复灵龛守护 guardian_kind proto enum 归一 — ✅ 2026-07-18

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
- **未知枚举风险**：legacy `GuardianKindV1` 不包含 `unspecified`；本次明确移除 proto3 默认 sentinel，并让未知 numeric enum 由 legacy string gate 安全 no-op，避免伪造玩家可见守护类型。
- **修复边界风险**：不要顺手解决 #945 的 store 跨 session 清理，也不要把 #1010 的 season_state 修复混进同一 PR；本 skeleton 只处理 niche guardian proto enum 前缀。
- **覆盖风险**：只测 `PUPPET` 会漏掉 `ZHENFA_TRAP` / `BONDED_DAOXIANG` 的 snake_case 转换，因此至少要覆盖一个带下划线的 guardian kind。

## Finish Evidence

### 落地清单

- **P0 / 生产桥接**：`client/src/main/java/com/bong/client/network/ProtoServerDataBridge.java`
  - `NICHE_GUARDIAN_FATIGUE` / `NICHE_GUARDIAN_BROKEN` 均走 `bridgeStripEnumsOmittingUnspecified`。
  - 合法 `GUARDIAN_KIND_*` 继续复用 `stripEnumPrefix` 产出 legacy snake_case。
  - `removeUnspecifiedEnum` 仅在这两个 guardian payload 上移除 proto3 默认 sentinel；missing 与显式 `GUARDIAN_KIND_UNSPECIFIED` 在 wire 上等价，均由既有 required-field gate 安全 no-op。
  - 未知 enum number 保持 JSON number，由 `SocialServerDataHandler.readString` 拒绝，不污染 store / HUD / event。
- **P0 / 饱和回归**：`client/src/test/java/com/bong/client/network/ProtoServerDataBridgeTest.java`
  - 合法三值：`PUPPET`、`ZHENFA_TRAP`、`BONDED_DAOXIANG`。
  - 完整状态转换：真实 proto bytes → `ProtoServerDataBridge.bridge` → 同一 `ServerDataRouter`、同一 `zhenfa_trap` key、无 reset 的 fatigue → broken 连续迁移 → `NicheGuardianStore` 最终破损态。
  - 玩家可观察结果：`NicheGuardianPanel.buildLines()` 的最终唯一 guardian 行与单条告警，以及 `UnifiedEventStore` 的顺序、channel / priority / sourceTag / text。
  - 防重复发布：fatigue / broken 两条事件分别断言 `foldCount() == 1` 且 `displayText() == text()`，避免重复事件被 folding 后仍以两条表象假绿。
  - 错误与边界：missing guardian kind、显式 `GUARDIAN_KIND_UNSPECIFIED`、未知 enum number，均锁定 no-op 且 store / alert / event / HUD 无污染。

### 关键 commit

- `913759222abf577d16d17e9983d711204de46e0d` — 2026-07-13：历史 PR #1186 首次补上两个 guardian payload 的 enum 前缀归一。
- `c301899575c0f918748556a57e6daf4166a942d7` — 2026-07-13：PR #1186 合入主线；原 skeleton 同批被直接删除，未正规 promotion / 归档。
- `a8f161f5eaad74d498930c043aff84a7ab5e1d1d` — 2026-07-18：从历史恢复 skeleton 并正规升格为 active plan。
- `31583b3ab43e1ebe16879dbfbbfc3b8e95f83941` — 2026-07-18：补齐 broken / missing / UNSPECIFIED 的 proto→bridge→router 边界测试；随后由对抗 validator 发现默认字段语义仍会污染 `unspecified`。
- `656cd4cf869163de59836bf54bdb8d89abc1269e` — 2026-07-18：修正默认枚举边界，并补齐合法、缺失、UNSPECIFIED、未知数值的 store / HUD / 统一事件断言。
- `ba9676849df9a59e914ba0374923678ee3e07fee` — 2026-07-18：补齐枚举边界证据并把 active plan 正规归档到 `docs/finished_plans/`。
- `c5aa2228a3f3e778f00b16162cec565598708bb3` — 2026-07-18：新增同一 guardian、同一 router、无 reset 的 fatigue → broken 连续 proto 路由与最终 HUD / 事件顺序回归。
- `f1121724992a020db4472f75c39eea4e88ddc2f8` — 2026-07-18：用 `foldCount` / `displayText` 锁定两种事件各发布一次，关闭 UnifiedEventStream folding 假绿。
- `970d136a1bbc014161eb07975b4cf6ae50a35e1b` — 2026-07-18：合并最新 `origin/main` 后复验；合并仅带入无关 skeleton / reminder 文档，client 生产与测试 blob 保持不变。
- `4dceecb8c71f8d3f800dc968f07b16f99c9bd540` — 2026-07-18：原地校正唯一 Finish Evidence；相较 `970d136a` 仅修改本归档文档，`ProtoServerDataBridge.java` 与 `ProtoServerDataBridgeTest.java` 的 blob 保持不变。

### 测试结果

- **历史可执行证据（PR #1186）**：`cd client && ./gradlew test build` 由 PR body 记录为 `BUILD SUCCESSFUL`；GitHub checks `respond`、`e2e`、`CodeRabbit` 均为 `SUCCESS`。该证据只覆盖当时的三条 guardian 测试，不冒充本 PR 后续快照的执行结果。
- **本地执行历史**：JDK 17 下三轮受控 Gradle 尝试均在 test discovery 前被 sandbox 基础设施阻断：前两轮为 single-use daemon `java.net.SocketException: Operation not permitted`，最终已消除 daemon fork，但 Gradle 初始化仍报 `Could not determine a usable wildcard IP for this machine`。三轮实际执行测试数均为 0，结果不是 PASS，也不作为本 PR 的可执行验收依据。
- **静态/对抗门**：`git diff --check` 通过；fresh read-only validator 对 `f1121724992a020db4472f75c39eea4e88ddc2f8` 给出 `PASS`，核验连续迁移、唯一 key / HUD、事件顺序、防 folding 断言与 protobuf Java API。合并主线后，独立 `codex exec --ephemeral --sandbox read-only` validator（`gpt-5.6-sol`）又对 `970d136a1bbc014161eb07975b4cf6ae50a35e1b` 给出 `PASS`；两轮均只做静态对抗核验，不冒充可执行测试。
- **主线同步**：2026-07-18 执行 `git fetch origin` 紧邻合并 `origin/main`，生成 `970d136a1bbc014161eb07975b4cf6ae50a35e1b`；无冲突，仅带入 10 个无关 `docs/plans-skeleton/*` / `reminder.md` 变更，`ProtoServerDataBridge.java` 与 `ProtoServerDataBridgeTest.java` 的 blob 和合并前一致。
- **已完成的 PR 可执行 gate 快照**：[GitHub Actions run 29635120100](https://github.com/Kizunad/Bong/actions/runs/29635120100)（`E2E Redis Smoke`，job `88055990976`）精确绑定 `4dceecb8c71f8d3f800dc968f07b16f99c9bd540`，于 2026-07-18 `SUCCESS`：`Setup Java 17`、`Client stage (gradlew test)`、schema build/check/test/generate、agent check/test、server release build、`Server stage (cargo test)`、smoke/e2e 与 bot e2e 全部成功。
- **最终 PR HEAD 绑定规则**：静态归档不再把任何先前 SHA 称为“当前 / 最终 HEAD”，因为修正证据的 commit 本身会生成新 SHA。待合入 `headRefOid`、fresh validator SHA 与最终 client/e2e 成功 run 必须在 PR Body 和平台 Checks 中精确对拍；任一项不一致即不得 merge。该动态平台记录只补充最终绑定，不改写上述已完成历史快照。

### 跨仓库核验

- **server**：`server/src/schema/social.rs::GuardianKindV1` 仅含 `Puppet / ZhenfaTrap / BondedDaoxiang`；`server/src/schema/proto_convert.rs` 的 `ServerDataPayloadV1::NicheGuardianFatigue` / `NicheGuardianBroken` 经 `guardian_kind_to_proto` 生成真实 protobuf enum。
- **agent/schema**：`agent/packages/schema/src/social.ts::NicheGuardianFatigueV1` / `NicheGuardianBrokenV1` 的 `guardian_kind` 复用 `GuardianKindV1`，legacy 契约不包含 `unspecified`。
- **client**：`ProtoServerDataBridge.bridgeStripEnumsOmittingUnspecified` → `SocialServerDataHandler.handleNicheGuardianFatigue/handleNicheGuardianBroken` → `NicheGuardianStore` / `NicheGuardianPanel` / `UnifiedEventStore` 全链命中；合法值不再泄漏 `GUARDIAN_KIND_`，默认/未知值不产生玩家可见污染，同一 guardian 的 fatigue → broken 连续迁移及两种事件各发布一次均有契约测试锁定。

### 遗留 / 后续

- 本地 sandbox 仍无法提供 Gradle 所需的本机 socket / wildcard-IP 能力；这是明确保留的基础设施历史，三轮本地执行均为 0 tests、不是 PASS。已完成快照的可执行验收由上述 PR CI run 29635120100 提供，最终待合入 HEAD 则按 PR Body / Checks 的动态绑定规则验收。
- 本 plan 不改 Rust / TypeBox / protobuf 定义、不改 Redis channel，也不处理 #945 跨 session 清理或 #1010 season enum；这些边界仍按各自 plan 管理。
