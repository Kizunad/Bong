# plan-bughunt-forge-item-material-validation-v1

> 骨架：来源 #1433。图谱校验只接受 MineralRegistry，后续运行时却支持 item material。

## §0 摘要

`server/src/forge/blueprint.rs:210-241` 的 `validate_with` 用 `MineralRegistry::get_by_str` 校验 required 材料；允许 item material 的 helper 只在 `server/src/forge/mod.rs:469-487` 的后续路径执行。因此含骨甲/封灵匣等 item 材料的图谱在加载阶段被错误拒绝。

## §1 游玩影响

玩家获得正典材料物品却无法启动对应图谱，界面表现为材料缺失，材料来源和配方内容被锁死。

## §2 复现路径

1. 加载 required material 为 item template 而非矿物 ID 的 blueprint。
2. 运行 `validate_with`，观察 registry 查找失败。
3. 即使 `forge/mod.rs` 后续拥有 item material 解析，流程也在此前结束。

## §3 今天 `origin/main` 根因证据

- `blueprint.rs:210-241` required 校验只调用矿物 registry。
- `forge/mod.rs:469-487` 才调用 item material helper，时序晚于 blueprint validate。
- 两个入口对同一材料集合的合法性定义不一致。

## §4 非重复比对

`plan-forge-v1` 的图谱/工站流程和 `plan-bughunt-forge-optional-carrier-overclaim-v1` 的数量消费不负责材料类型校验。本骨架只统一 blueprint load 与 runtime 的合法材料解析。

## §5 立项检查记录

本记录已按 `docs/worldview.md`、`docs/finished_plans/`、`docs/plan-*.md`、`docs/plans-skeleton/` 与 `docs/plans-skeleton/reminder.md` 逐项检索。

- **worldview**：查“骨甲、封灵匣、锻造材料”；`worldview.md §八` 将物品材料视为可用资源。
- **finished_plans**：查 `Blueprint::validate_with`、`MineralRegistry`、item material helper；未见统一校验入口。
- **active plan**：查 forge blueprint validation；无同一类型门禁修复。
- **skeleton**：查 `item material`、`validate_with`、`MineralRegistry`；无重复骨架。
- **reminder.md**：查 `forge material validation`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：blueprint required/optional stacks、`MineralRegistry`、item registry/template IDs。
- **Outputs**：可加载的 blueprint、材料错误诊断、forge session 输入。
- **共享类型或事件**：复用 `MaterialStack`、`BlueprintValidationError`、`ItemRegistry`；不增加同义材料 ID。
- **server 符号**：`forge::blueprint::validate_with`、`forge::material` helper、`forge::mod::resolve_billet`。
- **agent**：无变更；图谱材料校验不在 agent schema。
- **client**：无变更；客户端只显示既有 blueprint 错误/快照。
- **worldview 锚点**：`docs/worldview.md §八` 材料与锻造。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 让 blueprint 与 runtime 复用同一材料解析器 |
| P1 | ⬜ | 矿物、item、未知材料及旧图谱兼容回归测试 |

## 来源 issue

- #1433 `[flash-review][major] validate_with 拒绝物品材料图谱`
