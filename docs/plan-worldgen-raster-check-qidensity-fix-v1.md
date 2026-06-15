# plan-worldgen-raster-check-qidensity-fix-v1（active）

> **Active（已从 skeleton 升级，待逐项消费）**。一句话主题：修复 `raster_check.py` 的 qi_density 同源派生硬断言与历史 profile 既有 qi 值不收敛，导致 8 个 worldgen pytest pre-existing 失败。

> 立项动机：worldgen-v4 P7 收官时发现 worldgen 全量 pytest **853 passed / 8 failed**，核实为 pre-existing（merge-base P6 commit 同样失败，worldgen-v4 改动未触碰）。根因：`worldgen/scripts/terrain_gen/harness/raster_check.py` 的 qi_density 同源派生硬断言（plan-worldgen-v4 P4 引入）对历史 profile 既有 qi 值不收敛 + 历史 profile 注册断言。

## 阶段总览

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 定位 8 个失败的 qi_density 断言不收敛根因 | ⬜ |
| P1 | 修复（放宽硬断言为语义 pin / 或修历史 profile qi 值收敛 / 或断言加容差） | ⬜ |

## 接入面 checklist

- **落点**：`worldgen/scripts/terrain_gen/harness/raster_check.py`（qi_density 后验断言）+ 失败的 profile：`test_tribulation_scorch` / `test_rift_mouth_barrens` / `test_jiu_zong_ruin` / `test_pseudo_vein_oasis` / `test_terrain_gen_zone_overlays`。
- **跨 plan**：plan-worldgen-v4（finished）P4 qi 配平引入的硬断言；本 plan 收口其遗留（Finish Evidence 已列 8 个 pytest 待修）。
- **qi_physics 锚点**：qi_density 派生口径（同源于 qi_field.py），断言应允许历史 profile 的合理偏差。

## P0 — 失败根因定位

- 跑 `cd worldgen && python3 -m pytest <失败 test> -v`，看 qi_density 断言期望 vs 实际；判断是断言过严（同源派生 epsilon 太死）还是历史 profile qi 值确需调整。

## P1 — 修复

- 按根因：① 断言放宽为语义 pin（方向/范围而非精确值，参 worldgen-v4 P4 wangyintai 那次 < f64::EPSILON → 语义 pin 的先例）；② 或修历史 profile 使 qi 收敛；③ 或断言加合理容差。目标 worldgen 全量 pytest 绿。

## §N 开放问题

1. 8 个失败是同一根因还是多个；qi_density 断言的"正确"口径（精确同源 vs 容差）。
2. 修断言 vs 修 profile 数据——哪个更对（正典 qi 分布优先）。

## 审计来源

worldgen-v4 P7 收官时核实的 pre-existing 失败（已写入 plan-worldgen-v4 Finish Evidence 遗留）+ bug-hunt round1 已知线索。**report-only**。
