# plan-bughunt-gathering-tool-durability-session-bind-v1

> 骨架：来源 #1825。采集完成按 session 起始工具结算，途中换工具可免耐久消耗。

## §0 摘要

`server/src/gathering/tools.rs:282` 附近的完成逻辑保存 session 起始工具/耐久，但没有把结算绑定到完成时仍在手中的 instance。玩家可在采集结束前换下工具，原工具不扣耐久。

## §1 游玩影响

稀有采集工具可以被反复使用而不损耗，工具耐久经济和风险失去意义。

## §2 复现路径

1. 用工具 A 开始 gathering session。
2. 中途换成工具 B 或空手，等待完成。
3. 观察完成逻辑仍按起始快照结算，A/B 当前 instance 的耐久不符合实际持有关系。

## §3 今天 `origin/main` 根因证据

- `gathering/tools.rs:282` 使用 session 起始工具信息作为完成扣耐久依据。
- 完成时没有核对当前 slot 的 `instance_id`、template 与 durability。
- 既有 `plan-gathering-tool-bind-v1` 只覆盖草镰/herb bundle，不覆盖该 session 起始/完成绑定。

## §4 非重复比对

现有 `plan-gathering-tool-bind-v1` 的范围是工具类别绑定；本骨架补的是同一 instance 在 session 生命周期内的结算身份，不重复其草镰规则。

## §5 立项检查记录

本记录已按 `docs/worldview.md`、`docs/finished_plans/`、`docs/plan-*.md`、`docs/plans-skeleton/` 与 `docs/plans-skeleton/reminder.md` 逐项检索。

- **worldview**：查“采集、工具、耐久、风险”；`worldview.md §七/§八` 要求资源工具消耗可追溯。
- **finished_plans**：查 `GatheringSession`、`instance_id`、durability；未见完成时 identity recheck。
- **active plan**：查 gathering/tool bind；已有 plan 只覆盖草镰和 herb bundle，未覆盖本根因。
- **skeleton**：查 `gathering durability`、`session bind`；无同根因骨架。
- **reminder.md**：查 `gathering`、`tool durability`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：session 起始工具、完成时 inventory/equipment、`instance_id`、采集结果。
- **Outputs**：一次耐久扣减/失败结果、`GatheringOutcome`、inventory snapshot。
- **共享类型或事件**：复用 `GatheringSession`、`ItemInstance`、tool registry、inventory snapshot；不另造工具身份。
- **server 符号**：`gathering::tools`、`HarvestSessionStore`、`ItemRegistry`、inventory mutation funnel。
- **agent**：无变更；采集工具状态不进 agent schema。
- **client**：无变更；客户端继续渲染 server inventory snapshot。
- **worldview 锚点**：`docs/worldview.md §七` 采集与 §八`工艺资源消耗`。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 用 instance identity + 完成时装备状态定义耐久扣除，换工具时明确拒绝/结算 |
| P1 | ⬜ | 正常完成、换手、断线、取消、重复完成回归测试 |

## 来源 issue

- #1825 `[flash-review][major] 采集完成按会话起始工具结算耐久`
