# plan-bughunt-tribulation-settle-reason-schema-v1

> 严重级别：Major。来源 #1382。绝壁劫结算实际发送的触发来源 reason 不在 agent 的 TypeBox 契约中，导致结算事件在叙事入口被拒收。

## §0 摘要

今天的 `origin/main` 中，server 的 `juebi_settlement_system` 在非 quota 路径把 `JueBiTriggerSource::wire_name()` 拼成 `jue_bi:<source>` 写入 `DuXuResultV1.reason`；`JueBiTriggerSource` 当前有七个 wire name。agent schema 的 `DuXuResultReasonV1` 却只允许 `void_quota_exceeded`。因此 server 已完成结算，Redis 也已发出事件，但 `tribulation-runtime.ts` 的 `validateTribulationEventV1Contract` 会拒绝例如 `jue_bi:dugu_reverse`，结算叙事被静默丢弃。

修复方向是把真实生产 wire 契约收口到一个可枚举的 reason 集合：保留 `void_quota_exceeded`，为当前七个 `jue_bi:<JueBiTriggerSource::wire_name()>` 逐项建 literal；未知值仍必须拒绝。不得把 reason 放宽成任意字符串，也不得让 server、TypeBox、Rust mirror 各自维护不一致的命名。

## §1 游玩影响

渡劫者的结算结果仍会在 server 落库，但 agent 收不到合法事件，玩家和旁观者看不到对应的绝壁劫结算叙事。不同触发来源会表现成“结算完成却没有天道反馈”，也让运维无法从 agent 的拒收统计区分正常未知输入与已知生产来源。

## §2 复现路径

1. 让一个绝壁劫以 `VoidActionExplodeZone`、`DuguReverse`、`BaomaiDisperse`、`WoliuVortexHeart`、`ZhenfaDeceptionExposed` 或 `KarmaThreshold` 之一启动并进入结算。
2. `server/src/cultivation/tribulation.rs` 的 `juebi_settlement_system` 生成 `reason = "jue_bi:<source>"`，随后 settlement bridge 发布带 `result.reason` 的 `TribulationEventV1`。
3. agent `tribulation-runtime.ts` 解析事件并调用 `validateTribulationEventV1Contract`；该 reason 不匹配当前 `DuXuResultReasonV1`，事件在 LLM/narration 前被拒绝。
4. quota-origin 的 `void_quota_exceeded` 是对照正例：它能通过同一 validator，说明问题是已知 source reason 未进入共享契约，而不是 Redis 发布本身不可达。

## §3 今天 `origin/main` 证据

- `server/src/cultivation/tribulation.rs:234-268` 的 `JueBiTriggerSource` 定义七个稳定 `wire_name()`，并在 `:2064-2083` 的 `juebi_settlement_system` 对非 quota 结算写入 `format!("jue_bi:{}", source.wire_name())`。
- `server/src/cultivation/tribulation.rs:3943-3967` 将结算事件交给 `TribulationEventV1::jue_bi`，`server/src/network/redis_bridge.rs:994-1001,1810-1828` 负责送出；不存在“仅测试构造、生产不可达”的断点。
- `agent/packages/schema/src/tribulation.ts:40-42` 的 `DuXuResultReasonV1` 只有 `void_quota_exceeded`；`server/src/schema/tribulation.rs:33-42` 的 mirror 目前使用未约束的 `Option<String>`，没有替 TypeBox 兜底。
- `agent/packages/tiandao/src/tribulation-runtime.ts:307-325` 在处理 payload 前调用 `validateTribulationEventV1Contract`，reason 不在 schema 时直接增加 `rejectedContract` 并返回。

## §4 非重复比对

- `docs/finished_plans/plan-void-quota-v1.md` 只为 quota-origin 的 `void_quota_exceeded` 增加 reason，并记录了该单一 token 的 runtime 文案；它没有覆盖后来由 `JueBiTriggerSource` 产生的六个非 quota wire reason。
- `docs/finished_plans/plan-tribulation-v1.md` 与 `plan-tribulation-v2.md` 定义了绝壁劫与触发源，但没有把 settlement reason 的 agent TypeBox literal 与所有生产 source 对拍。本骨架只收口 server↔schema↔runtime 的 reason 契约，不重做渡劫状态机或叙事管道。
- `plan-agent-narration-pipeline-v1` 负责通用 publish/validate 边界；本 issue 的具体缺口发生在进入该管道前的 `TribulationEventV1` schema，不以修改通用管道代替 reason 对齐。

## §5 立项检查记录

本记录按任务卡检查了世界观、归档计划、active plan、骨架和 reminder 入口，并以今天的 `origin/main` 代码为准。

- **worldview**：检索“渡虚劫、化虚、绝壁劫、广播、截胡”；命中 `docs/worldview.md §三 L126-L131`（通灵→化虚、渡虚劫与全服广播）及 `§十一 L999-L1005`（结算广播后可被截胡）。
- **finished_plans**：检索 `DuXuResultReasonV1`、`void_quota_exceeded`、`JueBiTriggerSource`、`TribulationEventV1`；确认 quota 计划只登记单一 reason，未登记七个 source 的完整 wire 集合。
- **active plan**：检索 `tribulation reason`、`schema`、`settle`；未发现已锁定本 reason 缺口的 active plan。
- **skeleton**：检索 `jue_bi:`、`DuXuResultReasonV1`、`wire_name`、`tribulation settle reason`；没有同主题骨架，只有 quota marker 生命周期等相邻渡劫生命周期工作。
- **reminder.md**：检索 `tribulation`、`reason`、`schema`；仓内无 `docs/plans-skeleton/reminder.md`。

## §6 接入面与跨仓契约

- **Inputs**：server 产生的 `TribulationEventV1` 结算 payload；`JueBiTriggerSource::wire_name()`；现有 quota marker 的 `void_quota_exceeded`。
- **Outputs**：共享 schema 接受七个 `jue_bi:<source>` 与 `void_quota_exceeded`；已知 reason 可进入 `tribulation-runtime.ts`，未知 reason 仍 fail-closed 并计入 `rejectedContract`。
- **共享类型或事件**：`TribulationEventV1`、`DuXuResultV1`、`DuXuResultReasonV1`、`JueBiTriggerSource`、`TribulationEventV1::jue_bi`；reason literal 必须由 server wire-name 列表和 TypeBox/ Rust mirror 对拍生成，不能使用开放字符串。
- **server 符号**：`cultivation::tribulation::{JueBiTriggerSource,juebi_settlement_system,TribulationSettled}`、`schema::tribulation::TribulationEventV1`、`network::redis_bridge::RedisOutbound::TribulationEvent`。
- **agent 符号**：`schema::tribulation::{DuXuResultReasonV1,DuXuResultV1,TribulationEventV1}`、`tiandao::tribulation_runtime::{validateTribulationEventV1Contract,handlePayload}`；agent 需补 schema 与 runtime 的已知 reason 回归测试。
- **client**：无变更；客户端消费已有 tribulation payload/HUD，不参与 server→agent reason 校验，依据是本 issue 的拒收发生在 agent `handlePayload`，不在 client 解析链。
- **worldview 锚点**：`docs/worldview.md §三 L126-L131`、`§十一 L999-L1005`；本计划不新增境界、触发源或真元规则，只恢复既有绝壁劫结算反馈。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 对拍七个 `JueBiTriggerSource::wire_name()`，冻结 `void_quota_exceeded` 与 `jue_bi:<source>` 的共享 reason literal，并同步 TypeBox 与 Rust mirror |
| P1 | ⬜ | schema validator、tribulation runtime 和 server 序列化回归测试：七个已知 source 全部通过，未知 reason 拒绝，quota token 保持兼容 |

## 来源 issue

- #1382 `[flash-review][major] 绝壁劫 settle 的 reason 被 schema 拒绝致结算叙事丢失`
