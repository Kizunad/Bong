# plan-bughunt-world-poi-finite-input-v1（骨架）

> **来源 issue**：#1611。POI `trigger_radius` 的 `parse_f64` 接受 NaN/Inf，非有限值能绕过距离入口。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | POI 数值输入拒绝 NaN/Inf，并保持正半径边界 | ⬜ |

## §0 摘要

`server/src/world/tsy_poi_consumer.rs` 只调用 `.parse::<f64>().ok()`；Rust 可解析 `NaN`/`inf`，后续比较会失去有限半径的门禁，造成触发范围异常。

## §1 游玩影响

坏的世界配置或外部 POI 数据可能让裂缝/触发器在全局或不可预测范围生效，影响事件密度与服务器负载。没有跨端协议变化。

## §2 复现路径

1. 在 POI JSON/字段中填 `trigger_radius=NaN` 或 `inf`。
2. 经过 `parse_f64` 与 POI consumer，观察值被接受。
3. 触发距离检查，确认非有限半径不能作为合法范围被拒绝。

## §3 今天 `origin/main` 的证据

- `server/src/world/tsy_poi_consumer.rs:136-145,184-194,395-397` 的 `parse_f64` 只 `.parse::<f64>().ok()`，没有 `is_finite()` 检查。

## §4 非重复比对

已查 Tsy POI/worldgen finished plans、active plan 和 skeleton 的 `trigger_radius`、`parse_f64`、finite；没有已有骨架处理该输入门。#1611 单独成因明确。

## §5 立项检查记录

- `docs/worldview.md`：查末法地貌/POI 触发器及区域边界；不新增 POI。
- `docs/finished_plans/`：查 `plan-tsy-*` 的 POI schema/校验约定。
- active plan：查 `parse_f64`、`trigger_radius`、`TsyPoi`，未发现同一解析入口改动。
- skeleton：查 #1611、NaN、Inf、finite，无同主题骨架。
- `docs/plans-skeleton/reminder.md`：已检查，无 POI finite 输入条目；不改该文件。

## §6 接入面与跨仓契约

- **Inputs**：Tsy POI 文件中的字符串数值。
- **Outputs**：server 内部 `TsyPoi`/trigger radius；非法输入被拒绝并记录可诊断错误。
- **共享类型/事件**：复用 `parse_f64` 与 POI consumer 的错误路径，不新增宽松 parser。
- **三端契约符号**：server `tsy_poi_consumer::parse_f64`、POI trigger systems；agent **无变更**、client **无变更**（配置校验在 server 侧完成）。
- **worldview 锚点**：末法 POI/裂缝的区域触发边界；不涉及 qi_physics。

## P0 验收

- NaN、+Inf、-Inf 和负半径不进入运行时 POI；合法有限半径保持原语义。
- 拒绝原因可定位到字段，不把坏资产静默转换为全局触发。
