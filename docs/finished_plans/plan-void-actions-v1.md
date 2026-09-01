# Bong · plan-void-actions-v1 · Finished

化虚专属 action 由三类组成：镇压坍缩渊、引爆区域、化虚障。所有 action 均受 `Realm::Void`、真元、寿元和冷却门禁约束；真元通过 `qi_physics::ledger` 流转，寿元通过 `lifespan` API 扣减，公告统一由 agent 以 `broadcast` 范围发布。

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
- 全树清理验证：旧 action 标识、继承字段和死信箱符号在源码、协议、测试与文档中均无残留。

## Finish Evidence

- **落地清单**：server action/ledger/schema、agent runtime/schema/generated artifacts、client screen/store/handler、R7 UI fixtures 和亡者页面均已更新。
- **关键 commit**：历史实现 commit 保留在 git 记录中；本次裁剪在独立提交中移除已退役 action 的生产代码、协议字段、测试和 fixture。
- **测试结果**：Rust `cargo check --all-targets` 通过；schema 912 项测试通过；client 编译、gametest 通过，R7 计数已按当前 28 个 Screen 校准。
- **跨仓库核验**：server、agent、client 三端均只引用三类 action；Proto、TypeBox、generated JSON schema 的 union 成员一致。
- **遗留 / 后续**：圆形屏障、六个月回流曲线和化虚对抗仍属于后续版本范围。
