# plan-hud-qi-radar-mainpath-regression-v1（骨架）

> **骨架（草案）**。一句话主题：`plan-hud-immersion-v2` 已交付并归档的 `QiDensityRadarHudPlanner` 在当前生产 HUD 主路径里被整段注释掉，导致**凝脉及以上玩家正常游玩时永远看不到灵压雷达**；负灵域反向紫标、TSY 假信号、周边修士气息白点这些已写好/已测好的感知反馈全部失效。影响是：玩家突破到凝脉后，本该解锁的“灵气方向感知”能力并未上线，探索高低灵气区、规避负灵域、判断附近修士气息都只能靠盲走。

> 立项动机：这不是“功能未做”，而是**已做、已测、已在 finished plan/进度表宣称完成，但后续提交把生产接线剪断**。当前 drift 位于 `player_state/hud/cultivation` 主路径，玩家正常推进境界就会踩到，且与已归档 plan 的验收承诺直接矛盾，适合先立 skeleton 固化证据、回归点与修复面，再单独出 fix PR。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 凝脉+ 灵压雷达主路径回归 | fix_pr | ⬜ |

## P0 — 凝脉+ 灵压雷达主路径回归

- **现象**：`client/src/main/java/com/bong/client/hud/QiDensityRadarHudPlanner.java` 完整存在，且按 `HudRealmGate.atLeastCondense(player.realm())` 对凝脉+ 开放；但 `client/src/main/java/com/bong/client/hud/BongHudOrchestrator.java:221-229` 把 `commands.addAll(QiDensityRadarHudPlanner.buildCommands(...))` 整段注释成 dead code，生产 HUD 命令流里根本不会出现 `HudRenderLayer.QI_RADAR`。
- **回归来源**：`git show 1305a3531` 可见该 planner 在 `feat(plan-hud-immersion-v2): 增加 HUD 沉浸感知组件` 时曾正式接入 `BongHudOrchestrator`；`git show 084b0eeec5` 可见后续 `修复涡流HUD、战斗死亡与玩家持久化 (#244)` 提交把这段调用直接注释掉，并同步新增 `productionHudDoesNotRenderQiRadarArrayDisk` 测试，把“生产环境不渲染雷达”固化成当前行为。
- **为什么这是 bug，不是“功能未完成”**：
  - `docs/finished_plans/plan-hud-immersion-v2.md` P0 明确写了“**仅凝脉+ 玩家可见**”“走向高灵气区域 → 对应方向标记变长变金”；
  - `docs/plans-progress.yaml` 仍把 `QiDensityRadarHudPlanner` 记为 merged/100%；
  - `HudLayoutPreset` / `HudImmersionMode` / `HudRenderLayer` 仍保留 `QI_RADAR` 路径，说明系统并未整体下线，只是主 orchestrator 断线。
- **对实际游玩体验的影响**：
  - 凝脉角色突破后，**不会获得灵压雷达**，与世界观“凝脉+ 可感知区域灵气精确值”及 finished plan 验收不符；
  - 负灵域的“标记反转内指 + 紫色”提示、TSY 的“假标记”扰动、附近修士气息白点，这些原本帮助玩家找高灵气方向/识别危险环境/判断附近活体气息的反馈，在生产 HUD 中全部消失；
  - 结果是玩家在高低灵气区、负灵域、TSY、修炼路线选择上只能靠记忆或盲走，突破后的感知升级体感缺失。
- **建议修复范围 / 模块**：
  - 首选最小修法：恢复 `BongHudOrchestrator` 对 `QiDensityRadarHudPlanner.buildCommands(...)` 的主路径调用；
  - 同步修正/删除当前把“生产环境不渲染雷达”当成正确行为的测试，改回锁“凝脉+ 主路径确实能产出 `QI_RADAR` layer”；
  - 若有确实要临时下线的 UX/性能理由，需要补明确 feature gate 或新 plan/文档说明，不能继续维持“finished plan 宣称已上线、生产代码实际剪断”的无声漂移。
- **验收抓手**：
  1. 凝脉玩家经 `BongHudOrchestrator.buildCommands(...)` 能实际得到 `HudRenderLayer.QI_RADAR` 命令；
  2. 引气玩家仍无雷达（下限门槛不回归）；
  3. 负灵域 / TSY / 周边修士气息三类特化标记通过主路径可见，而不是只停留在 planner 单测；
  4. `BongHudOrchestratorTest` 不再把“生产 HUD 永不渲染雷达”当作正确行为。

## 反方裁决摘要

1. **Round 1 质疑：这可能是故意下线，不算 bug。**
   复核后结论：仓库里没有任何 active plan / skeleton / finished note 宣布“移除灵压雷达”；相反，`plan-hud-immersion-v2` 和 `plans-progress.yaml` 都把它当作已完成交付物。当前只有一条“暂时停用阵盘 HUD”的行内注释，没有 feature gate、没有替代方案、没有文档回写，因此更像未收口回归，不像有设计决议的下线。
2. **Round 2 质疑：也许只是 demo planner，真实游玩并不依赖。**
   复核后结论：`QiDensityRadarHudPlanner` 直接消费 `PlayerStateViewModel`、`ZoneState`、`PerceptionEdgeState`、`HudEnvironmentVariant` 和 runtime yaw；finished plan 还把“凝脉+ 可见”“负灵域反向紫标”“TSY 假标记”写成 P0/P3 验收项。也就是说，这就是面向正式游玩的 client HUD 组件，不是演示代码。主路径注释掉后，玩家端对应感知能力确实整块消失。

## 开放问题

1. 临时注释的原始动机是什么？若是 UI 干扰或性能顾虑，是否应补正式 feature flag / density gate，而不是整段 dead code。
2. 目前 `BongHudOrchestratorTest` 把“无雷达”锁成正确行为；修复 PR 应直接改断言，还是新补一条“凝脉主路径有雷达、引气没有”的更强 pin。

## 审计来源

bughunt AE 定点轮（范围：`cultivation/social/player-hud-state` 主路径，优先 client HUD/state + `player_state` 接线）。候选经主代理人工读码、历史 commit 复盘、两轮默认怀疑裁决后保留。当前结论为 **report-only**：先提交 skeleton plan 固化主路径断点、回归提交、玩家影响与验收抓手，再由后续 fix PR 恢复接线。
