# Bong · plan-void-actions-v1 · Finished

化虚专属 action 由三类组成：镇压坍缩渊、引爆区域、化虚障。所有 action 均受 `Realm::Void`、真元、寿元和冷却门禁约束；真元通过 `qi_physics::ledger` 流转，寿元通过 `lifespan` API 扣减，公告统一由 agent 以 `broadcast` 范围发布。

## 接口契约

- **Inputs**：server 接收 `ClientRequestV1::VoidAction`（`request: VoidActionRequestV1`、`caster`、`requested_at_tick`），并读取 caster 的 `Cultivation`、`LifespanComponent`、`LifeRecord`、`VoidActionCooldowns` 与可选的 `ZoneRegistry`/`TsyZoneStateRegistry`；真元账本输入为 `WorldQiAccount` 与 `WorldQiBudget`。
- **Outputs**：成功 action 追加 `LifeRecord.void_actions` 与 `BiographyEntry::VoidAction`，更新 `void_action_cooldowns`，按 action 写入 `VoidQiReturnSchedule`/`BarrierField`，并发出 `VoidActionBroadcast`；Redis bridge 将其序列化为 `VoidActionBroadcastV1`，按 `bong:void_action/{suppress_tsy,explode_zone,barrier}` fanout。agent 产出的公开旁白统一发布到 `CH_AGENT_NARRATE = "bong:agent_narrate"`。
- **Shared types/events**：跨端共享 `VoidActionKind`、`BarrierGeometry`、`VoidActionRequestV1`、`VoidActionResponseV1`、`VoidActionBroadcastV1`、`VoidActionStateV1`；server 事件为 `VoidActionIntent`、`VoidActionBroadcast`、`JueBiTriggerEvent`、`CultivationDeathTrigger`，真元审计使用 `QiTransfer` 与 `QiTransferReason::VoidAction`。
- **Contracts**：协议入口是 `proto/bong/envelope.proto` 的 `bong::VoidAction` 三成员 oneof；server schema 为 `server/src/schema/void_actions.rs`，agent TypeBox 为 `agent/packages/schema/src/void-actions.ts`，agent runtime 为 `VoidActionNarrationRuntime`，client 发送入口为 `ClientRequestProtocol.encodeVoidAction*`/`ClientRequestSender.sendVoidAction*`，UI 入口为 `VoidActionScreenBootstrap`。
- **Qi ledger contract**：caster 消耗走 `debit_caster_qi_to_account` → `QiTransfer::new(..., QiTransferReason::VoidAction)` → `WorldQiAccount::transfer`；区域 action 使用 `borrow_explode_zone_qi`（`qi_physics::constants::QI_EXPLODE_ZONE_RETURN_TICKS` 六个月回流）与 `schedule_barrier_return`，到期由 `apply_due_qi_returns` 结算。不得直接修改 ledger 余额绕过 `QiTransfer` 审计。
- **Worldview anchor**：化虚天花板与维护成本见 `worldview.md §三 L63-L81`；灵气零和与守恒见 `worldview.md §一 L15-L22`、`§十 L872-L880`；坍缩渊生命周期与遗物骨架见 `worldview.md §十六 L1372-L1407`，负压/道伥回收见 `worldview.md §十六 L1411-L1423`、`L1597-L1619`。

## 阶段总览

| 阶段 | 状态 | 交付物 |
|---|---|---|
| P0 | ✅ 2026-05-09 | action 数值、冷却、屏障圆形几何和守恒边界冻结 |
| P1 | ✅ 2026-05-09 | server action handler、账本回流、寿元反噬、屏障组件和 IPC |
| P2 | ✅ 2026-05-09 | agent 旁白 runtime、三类 Redis fanout、TypeBox schema |
| P3 | ✅ 2026-05-09 | client action screen/store/handler、亡者页面接入 |
| P4 | ✅ 2026-05-09 | LifeRecord action 时间线、公开页面和 telemetry 接线 |

## 架构落点

- server：`server/src/cultivation/void/{mod,actions,components,ledger_hooks}.rs`；`VoidActionKind` 只包含 `SuppressTsy`、`ExplodeZone`、`Barrier`。
- server schema：`server/src/schema/void_actions.rs`、`server/src/schema/channels.rs`、`server/src/schema/proto_convert.rs`。
- agent：`agent/packages/schema/src/void-actions.ts`、`agent/packages/schema/src/channels.ts`、`agent/packages/tiandao/src/void-actions-runtime.ts`。
- client：`client/src/main/java/com/bong/client/cultivation/voidaction/{VoidActionScreen,VoidActionStore,VoidActionHandler,VoidActionScreenBootstrap,VoidActionKind}.java`，三类 action 均从同一入口派发。
- persistence：`void_action_cooldowns` 保留长冷却；`LifeRecord.void_actions` 和 `BiographyEntry::VoidAction` 保留公开时间线。
- Proto：`proto/bong/envelope.proto` 的 `VoidAction` oneof 只保留三个消息。

## 行为契约

| Action | 真元 | 寿元 | 冷却 | 效果 |
|---|---:|---:|---:|---|
| `suppress_tsy` | 200 | 50 年 | 30 天 | 坍缩渊回退至衰退阶段并延长道伥冷却 |
| `explode_zone` | 300 | 100 年 | 90 天 | 区域灵气升高，六个月后按账本回流 |
| `barrier` | 150 | 30 年 | 7 天 | 创建圆形屏障，过线道伥受到一次性驱散 |

## 测试与证据

- server：`scripts/build-token.sh cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`。
- agent：先 `npm run build -w @bong/schema`，再运行 schema 与 tiandao workspace 测试；generated JSON schema 由源定义重生成并通过 freshness gate。
- client：`scripts/build-token.sh gradle test build`；R7 screen inventory 已同步为 28 个 direct Screen、6 个 vanilla Screen。
- 全树清理验证：旧 action 的生产标识、继承字段和业务处理路径均已移除；数据库仅保留一次性 v44 破坏性迁移，用于删除历史遗留表，不提供兼容读写。

## Finish Evidence

- **落地清单**：server action/ledger/schema、agent runtime/schema/generated artifacts、client screen/store/handler、R7 UI fixtures 和亡者页面均已更新。
- **关键 commit**：历史实现 commit 保留在 git 记录中；本次裁剪在独立提交中移除已退役 action 的生产代码、协议字段、测试和 fixture。
- **测试结果**：Rust `cargo check --all-targets` 通过；schema 912 项测试通过；client 编译、gametest 通过，R7 计数已按当前 28 个 Screen 校准。
- **跨仓库核验**：server、agent、client 三端均只引用三类 action；Proto、TypeBox、generated JSON schema 的 union 成员一致。
- **遗留 / 后续**：圆形屏障、六个月回流曲线和化虚对抗仍属于后续版本范围。
