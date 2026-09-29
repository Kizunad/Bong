# plan-bughunt-forge-item-material-validation-v1

> 骨架：来源 #1433。`Blueprint::validate_with` 没有复用同文件已有的 `validate_forge_or_item_material`，运行时校验因此只接受矿物。

## §0 摘要

`server/src/forge/blueprint.rs:210-241` 的 `Blueprint::validate_with` 直接调用 `MineralRegistry::get_by_str`，没有复用同文件 `:401-417` 的 `validate_forge_or_item_material`。后者已经统一识别合法矿物和 `is_allowed_item_material`；validate_with 绕开它后，含骨甲/封灵匣等 item material 的图谱被错误当成未知矿物。实施时应统一调用现有 helper，必要时只做错误类型映射，不得另写解析器。

## §1 游玩影响

玩家获得正典材料物品却无法启动对应图谱，界面表现为材料缺失，材料来源和配方内容被锁死。

## §2 复现路径

1. 准备 required material 为允许 item material 而非矿物 ID 的 blueprint。
2. 运行 `Blueprint::validate_with`，观察其直接 `get_by_str` 返回 `UnknownMaterial`。
3. 对照同文件 `validate_forge_or_item_material` 已能接受该 material；现有 helper 没有被 validate_with 调用。

## §3 今天 `origin/main` 根因证据

- `server/src/forge/blueprint.rs:210-241` 的 required 循环直接 `minerals.get_by_str`，这是绕过 helper 的根因。
- `server/src/forge/blueprint.rs:382-399` 的 load-time 校验已经调用 `validate_forge_or_item_material`；helper 实体在 `:401-417`，内部复用 `is_valid_mineral_id` 与 `is_allowed_item_material`。
- `server/src/forge/mod.rs:469-487` 的 runtime 消费也支持 item material；三处应共享同一 helper 语义，不应再新建一套字符串解析。

## §4 非重复比对

`plan-forge-v1` 的图谱/工站流程和 `plan-bughunt-forge-optional-carrier-overclaim-v1` 的数量消费不负责材料类型校验。本骨架只让 `Blueprint::validate_with` 复用既有 `validate_forge_or_item_material`（含必要的 `BlueprintLoadError`→`ForgeValidationError` 映射），不另立材料解析规则。

## §5 立项检查记录

本记录已按 `docs/worldview.md`、`docs/finished_plans/`、`docs/plan-*.md`、`docs/plans-skeleton/` 与 `docs/plans-skeleton/reminder.md` 逐项检索。

- **worldview**：查“骨甲、封灵匣、锻造材料”；`worldview.md §八` 将物品材料视为可用资源。
- **finished_plans**：查 `Blueprint::validate_with`、`MineralRegistry`、item material helper；未见统一校验入口。
- **active plan**：查 forge blueprint validation；无同一类型门禁修复。
- **skeleton**：查 `item material`、`validate_with`、`MineralRegistry`；无重复骨架。
- **reminder.md**：查 `forge material validation`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：blueprint required/optional stacks、`MineralRegistry`、item registry/template IDs；校验入口必须把 material 字符串交给 `validate_forge_or_item_material`。
- **Outputs**：可加载的 blueprint、材料错误诊断、forge session 输入。
- **共享类型或事件**：复用 `MaterialStack`、`BlueprintValidationError`、`ItemRegistry`、`validate_forge_or_item_material`、`is_allowed_item_material`；只允许做错误类型映射，不增加同义材料 ID或新解析 helper。
- **server 符号**：`forge::blueprint::{Blueprint::validate_with,validate_blueprint_minerals,validate_forge_or_item_material,is_allowed_item_material}`、`forge::mod::resolve_billet`。
- **agent**：无变更；图谱材料校验不在 agent schema。
- **client**：无变更；客户端只显示既有 blueprint 错误/快照。
- **worldview 锚点**：`docs/worldview.md §八` 材料与锻造。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 让 `Blueprint::validate_with` 统一调用现有 `validate_forge_or_item_material`，保留矿物 tier 检查并映射错误，不新写解析器 |
| P1 | ⬜ | 矿物、item、未知材料及旧图谱兼容回归测试 |

## 来源 issue

- #1433 `[flash-review][major] validate_with 拒绝物品材料图谱`
