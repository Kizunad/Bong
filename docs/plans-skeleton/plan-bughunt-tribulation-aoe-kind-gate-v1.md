# plan-bughunt-tribulation-aoe-kind-gate-v1

> 来源 Issue：#1410。`tribulation_aoe_system` 把 JueBi 波当成 DuXu 波重复结算。

## §0 摘要

`tribulation_aoe_system` 只按 `TribulationPhase::Wave` 进入 `du_xu_wave_profile`，没有先检查 `TribulationState` 的劫种。JueBi 使用 wave 形态时，同一帧还会经过 `juebi_phase_effect_system`，造成双份伤害、抽灵和 qi_max 冻结，并泄漏 DuXu 专属链闪/吞魂规则。

## §1 实际游玩体验影响

- 绝壁劫参与者受伤、失灵和真元抽取约为设计值两倍。
- JueBi 期间可能触发 DuXu 的资源门和冻结机制，导致不可预期的境界/真元状态。
- 这是 server 结算选择错误，不是客户端表现延迟。

## §2 复现路径

1. 通过 `JueBiTriggeredEvent` 产生带 `TribulationPhase::Wave` 的 `TribulationState`。
2. 在 `phase_started_tick` 运行 `tribulation_aoe_system`，观察它无条件读取 `du_xu_wave_profile`。
3. 同一 tick 运行 JueBi 阶段效果系统，比较伤害、抽灵和冻结次数。
4. 使用 DuXu 与 JueBi 各一条对照路径，确认只有 DuXu 可以进入 DuXu AOE profile。

## §3 根因证据

- `server/src/cultivation/tribulation.rs:1366-1412` 的 `tribulation_aoe_system` 从 `state.phase` 直接取 wave，未读取或过滤 `state.kind`。
- `server/src/cultivation/tribulation.rs` 中的 `juebi_phase_effect_system` 另有 JueBi 伤害/灵压坍缩结算；两套系统在同一波次都可运行。
- `du_xu_wave_profile` 的返回值包含 DuXu 专属 `qi_drain`、`qi_max_freeze_ratio` 和 strike 配置，不能作为所有劫种的默认 profile。

## §4 非重复比对

- `plan-bughunt-juebi-aftershock-zone-ledger-v1.md` 处理 JueBi 对 zone 灵气的物理影响；本 issue 只处理 AOE 系统的劫种门禁。
- 现有 `TribulationOriginDimension`/维度过滤解决空间边界，不解决 kind 选择；不应把两个条件混成一个 plan。
- 未发现今天的 `origin/main` 已经为 `tribulation_aoe_system` 添加 JueBi 排除。

### 立项检查记录（2026-09-28）

- `docs/worldview.md`：检索“渡虚劫、绝壁、劫种、波次”，核对 §三 L127-L130 与 §十六 L1566-L1569。
- `docs/finished_plans/`：检索 `tribulation_aoe_system`、`du_xu_wave_profile`、`JueBi`；命中渡劫基础/生命周期文档，但没有该 kind gate。
- active plan：检索 `tribulation_aoe_system`、`TribulationKind`、`juebi_phase_effect_system`；`plan-container-filter-and-completion-v1.md` 只登记 writer 迁移，不覆盖重复结算门禁。
- `docs/plans-skeleton/`：检索 `tribulation_aoe_system`、`du_xu_wave_profile`、`JueBi`；已有 quota marker lifecycle skeleton 但未覆盖 AOE kind gate，未发现同主题 skeleton。
- `docs/plans-skeleton/reminder.md`：已查阅并检索 `tribulation_aoe_system`、`du_xu_wave_profile`、`JueBi`，未发现同主题延后事项。

## 接入面与跨仓契约

- **Inputs**：`TribulationState.kind/phase`、`HeartDemonResolution`、参与者 `Cultivation/Wounds`、`ZoneRegistry`、失败/死亡事件。
- **Outputs**：只有 `TribulationKind::DuXu` 产生 DuXu AOE；JueBi 继续由自身阶段系统结算，事件类型和 payload 不变。
- **共享类型 / 事件**：复用 `TribulationKind`、`TribulationPhase`、`TribulationFailed`、`DeathEvent`；不新增 agent/client schema。
- **server 契约符号**：`tribulation_aoe_system`、`du_xu_wave_profile`、`juebi_phase_effect_system`、`TribulationState::kind`。
- **agent**：无变更。劫种判定和伤害在 server，agent 只接收既有叙事/世界状态。
- **client**：无变更。现有劫状态与伤害事件格式不变，修复只减少错误重复结算。
- **worldview 锚点**：`docs/worldview.md §三 L127-L130`（渡虚劫）；`§十六 L1566-L1569`（不同空间环境的作用边界）。
- **qi_physics**：本 issue 不改变真元总量，只阻止错误 profile 被调用；既有 DuXu 抽灵仍须沿 `Cultivation::release_to_zone`/`QiTransferReason::ReleaseToZone` 和 `assert_conservation` 验证。

## §5 修复骨架

### P0 劫种门禁

- 在读取 wave/profile 前明确 `state.kind == TribulationKind::DuXu`；JueBi 直接跳过 DuXu AOE，不能用默认 wave 值代替。
- 复核系统调度顺序，保证 JueBi 阶段效果不会被同一系统再次触发。

### P1 回归

- DuXu 每种 wave 保留一个代表测试；JueBi 波确认无 DuXu profile、无重复抽灵/冻结/伤害。
- 维度过滤、失败事件和死亡事件的既有契约继续通过。
