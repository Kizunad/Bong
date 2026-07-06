# plan-dying-elder-tsy-zones-unloaded-v1

> **Active plan（由 skeleton promotion）**。一句话主题：`plan-dying-elder-v1` 的主 spawn 路径要求 `ZoneRegistry` 里存在 `is_tsy()` 且 `spirit_qi < -0.4` 的 TSY zone，但服务器启动只加载 `server/zones.json`，当前 TSY zone 全躺在从未并入运行态的 `server/zones.tsy.json`。结果是：**垂死大能在正常开服/正常游玩路径里永远刷不出来**，整条“给丹换传承再防翻脸”的 fauna 稀有遭遇退化成仅 dev `!tsy-spawn` 才能看见的死功能。

> 立项动机：这不是数值不平衡，也不是低概率“难复现”。它是结构性断链：候选 zone 集合在启动时就为空，后续再跑 30 个 in-game day 的 timer 也只会反复空转。该问题位于 finished fauna 主玩法 `plan-dying-elder-v1` 的入口，玩家影响明确，适合先立 skeleton-only PR 固化证据、影响面和修复抓手，再拆 fix PR。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 垂死大能 spawn gate 永久命中空集合 | fix_pr | ⬜ |

## P0 — 垂死大能 spawn gate 永久命中空集合

- **现象**：`server/src/fauna/dying_elder.rs:251-305` 的 `dying_elder_spawn_system` 每次触发只会在 `zones.zones.iter().find(|z| z.is_tsy() && z.spirit_qi < DYING_ELDER_SPIRIT_QI_THRESHOLD)` 中找候选。只要 `ZoneRegistry` 里没有 TSY zone，这个候选就永久是 `None`，系统只会更新 `last_spawn_attempt_tick` 后返回，永不 emit `DyingElderSpawnRequest`。
- **根因链**：`server/src/world/zone.rs:219-221` 把 `ZoneRegistry::load()` 硬编码到 `DEFAULT_ZONES_PATH = "zones.json"`；`server/src/world/zone.rs:725-738` 启动时只跑 `initialize_zone_registry -> ZoneRegistry::load()`；`server/src/world/mod.rs:122-170` 注册了 TSY portal / TSY lifecycle / TSY POI consumer，但没有任何一步把 `server/zones.tsy.json` 合并进 `ZoneRegistry`。也就是说，**world 层确实起了 TSY 维度壳，但 zone 层从未装入 TSY zone 数据**。
- **运行态证据**：
  - `server/zones.json:1-700+` 当前只有 overworld zone（如 `rift_mouth_*`、`spawn`、`blood_valley`、`north_wastes`），全文无任何 `name: "tsy_*"` 条目。
  - `server/zones.tsy.json:1-220+` 明确存在多组 TSY zone 模板，例如 `tsy_zongmen_01_shallow`（`spirit_qi = -0.4`）与 `tsy_daneng_01_shallow`（`spirit_qi = -0.45`）；后者正好满足垂死大能的 `< -0.4` gate，但这份文件只被 `world/tsy_dev_command.rs` 调试命令蓝图读取，不在正式启动路径里。
  - `server/src/world/mod.rs:227-239` 只创建空的 `TsyLayer`，日志注释也写着 “empty, awaits worldgen”；这进一步排除了“TSY zone 也许在别处自动注入”的侥幸。
- **为什么这是 bug，不是设计**：`docs/finished_plans/plan-dying-elder-v1.md` 已把该玩法标成 **P0-P3 全部 ✅**，目标是“负灵域濒死大能的完整 e2e 遭遇”；文档自己的 P0/P3 描述也都假设正常运行时会在 TSY/负灵域刷出该实体。当前实现却把遭遇前提偷偷降格成“先跑 dev `!tsy-spawn` 给 ZoneRegistry 手工塞 TSY subzone”，这与 finished fauna 主线的“正常玩家可遇到稀有事件”不一致。
- **对实际游玩体验的影响**：玩家在正式服正常探索时，无论经过多少 in-game day，都不会遇到垂死大能；因此拿不到这条遭遇提供的“高风险给丹博弈 / 拖延等其自毙舔包 / 相关 agent 叙事 / SpiritEye 观测”整包体验。体感上就是：**一个已经 finished 的稀有 fauna 遭遇在正常玩法里完全消失**，相关掉落、叙事和社交影响全部无从触发。
- **建议修复范围 / 模块**：优先收口 `server/src/world/zone.rs`、`server/src/world/mod.rs`、`server/src/fauna/dying_elder.rs`。核心决策只有一个：正式启动到底是 ① 合并加载 `zones.json + zones.tsy.json`，还是 ② 在 worldgen/TSY bootstrap 阶段把 `zones.tsy.json` 产物显式注册进 `ZoneRegistry`。无论选哪条，都必须让 `dying_elder_spawn_system` 在**非 dev、正常启动**下能看到真实 TSY zone 候选，而不是继续依赖 `!tsy-spawn`。
- **验收抓手**：至少补 4 组 pin。1) 默认启动的 `ZoneRegistry` 必须含至少 1 个 `is_tsy()` zone。2) `server/zones.tsy.json` 里满足阈值的 `tsy_daneng_01_shallow` 能进入 `dying_elder_spawn_system` 候选集。3) 跑到 `DYING_ELDER_SPAWN_INTERVAL_TICKS` 后，应真实 emit `DyingElderSpawnRequest`，不再只更新时间戳。4) 非 dev e2e 中，玩家进入 TSY/负灵域后最终可以遇到垂死大能 HUD/叙事链路。

## 反方裁决摘要

1. **Round 1 怀疑**：“TSY zone 也许在别的 startup/worldgen 路径里已经被注册，只是 `zones.json` 没体现。”
   **裁决**：否。`ZoneRegistry` 的正式启动入口只有 `ZoneRegistry::load() -> zones.json`，`world/mod.rs` 只起空 `TsyLayer`，没有任何合并 `zones.tsy.json` 或 runtime add TSY zone 的启动逻辑。
2. **Round 2 怀疑**：“这可能是有意只让 dev `!tsy-spawn` 触发，不能算玩家 bug。”
   **裁决**：否。`plan-dying-elder-v1` 已 finished，目标和 Finish Evidence 都按正式遭遇写法表述；把整个遭遇锁死在 dev command 前置，不符合 finished fauna 功能的玩家可达性，也不是文档声明的设计边界。

## 开放问题

1. TSY zone 的正式来源应是 `zones.tsy.json` 直接并表，还是 worldgen/manifest bootstrap 后统一 runtime register？修复 PR 需要一次选定，避免双源漂移。
2. 若正式启动补载 TSY zone，需要顺手回归 `tsy_drain`、`tsy_lifecycle`、`elder_encounter`、`ambient_tsy` 等现有 consumer，确认它们不会因“终于看见 TSY zone”而暴露新的 ordering/重复注册问题。

## 审计来源

bughunt 线程 AL，定向收窄 `server/src/fauna/` + `server/src/world/` 的 fauna 主路径接入。主代理先做代码级可达性复核，再做两轮默认怀疑式反证；结论为 **report-only**：先提交 skeleton-only PR 固化“正常运行时永不刷出”的死功能缺口，再由后续 fix PR 单独收口加载策略与回归面。
