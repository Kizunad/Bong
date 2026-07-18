# plan-newbie-30min-hooks-audit-v1 — 新手 30 分钟钩子表整链实测 + 断点修复

> **一句话主题**：`plan-gameplay-journey-v1` §L 的「新手 30 分钟分钟级钩子表」（L743-767）设计完整、子系统零件几乎全部 merged，但**整条链从未被端到端实测过**——本 plan 立一条协议级 bot 实测线逐钩子核验 + 修复实测暴露的断点，把「进游戏没事干」的体感变成逐分钟可核验的 ✅/❌ 矩阵。
>
> 来源：2026-07-18 早期玩法诊断（三路 Explore 实证）——新玩家前 30 分钟真正可达的主动玩法只有砍教程鼠/挖方块/G 键搜教程箱，搜箱成了唯一有即时正反馈的行为；§L 表里多个钩子疑似未按点兑现，但没人验证过。

**状态**：骨架（skeleton）。升 active 前按 docs/CLAUDE.md §五收口 §8。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | bot 整链实测场景 `journey_30min_hooks.py` + 钩子核验矩阵产出 | ⬜ |
| P1 | 断点修复（按 P0 矩阵，机制侧小修；大缺口登记转交专项 plan） | ⬜ |
| P2 | 30min 场景收编 CI bot e2e stage + 真人 30min 实测记录 | ⬜ |

## 为什么不并入现有骨架 / active plan（docs/CLAUDE.md §四红旗自查）

- **不并入 active `plan-gameplay-journey-v1`**：它是 100h 总线 plan，§H 的 6 段 E2E 通关脚本是其最终交付物；本 plan 只吃前 30 分钟一段，作为其 P0 段验收的先遣侦察。**本 plan 不修改 journey plan 文件**（一 PR 一 plan）；核验矩阵结论写在本 plan，journey §L 的状态回标留给人工或 journey 自己的 PR。
- **不并入 `plan-bot-e2e-coverage-v1`**：那是"逐模块场景覆盖"基建 plan（P1 修炼/P2 战斗/P3 库存…按模块切）；本 plan 是**跨模块整链体验验收 + 修复**，交付物含机制修复不只场景。P0 产出的 bot 场景挂进 `scripts/bot/scenarios/` 场景族，与 bot-e2e-coverage 共享框架（`scripts/bot/bot.py`），互为犄角不重复。

## 接入面（docs/CLAUDE.md §二 checklist）

- **进料**：journey §L 钩子表（9 行时刻表 + 5 句风格基准台词）；`server/src/world/spawn_tutorial.rs`（`TutorialState` 状态机 + `TutorialHook` 链 + POI markers :430 + `dynamic_rat_swarm_spawner` :229）；`scripts/bot/` 协议级 bot 框架（`mc_protocol.py`/`bot.py`，P0 已 ✅）；`server/loot_pools.json` `tutorial_kaimai_chest`（:164）；spawn raster（0.5+ 灵气点核验读 `zone_qi`/qi_density raster）。
- **出料**：bot 场景 `scripts/bot/scenarios/journey_30min_hooks.py`（多 leg，逐钩子断言）；钩子核验矩阵（写回本 plan §9 报告节）；P1 修复 commit（落在 spawn_tutorial / narration / worldgen 注入等既有模块内，不新建模块）。
- **共享类型 / event**：零新增 component/event/schema——本 plan 只消费既有 payload（`server_data` 各通道）做断言；修复也应优先在既有 symbol 内完成。
- **跨仓库契约**：bot 侧解码 `bong:server_data` 既有 payload（narration / qi / meridian / breakthrough / realm_vision 族）；无 wire 变更。
- **worldview 锚点**：§八 天道行为准则（O.13 沉默引导——bot 断言的是**环境线索与事件存在**，不是 UI 提示；本 plan 严禁以"加提示"作为修复手段）；§十三 初醒原（0.3 基底 + 北侧 200-500 格 0.5+ 馈赠区）。
- **qi_physics 锚点**：不涉及新真元流动。若 P1 修复触及 qi 数值（如灵气点注入浓度），只改 zone/raster 配置数据，不新增物理常数。

## P0 — bot 整链实测场景 + 核验矩阵 ⬜

`journey_30min_hooks.py` 按 §L 时刻表逐钩子做**可自动断言**的核验（时序压缩执行，不真等 30 分钟；每 leg 显式打印 PASS/FAIL/SKIP+原因）：

| §L 时刻 | 钩子 | bot 断言抓手 |
|---:|---|---|
| 0:00 | 石棺旁醒来 | join 后扫描出生点半径内 coffin POI marker payload / 方块存在 |
| 0:00 | 天道第一句 narration（新角色 vs 重生分支） | narration payload 到达 + 文案分支断言（§L 标注「需补分支」，预期 ❌ → P1） |
| 5:00 | 移动 200 格灵气条变色 | 移动后 qi 感知 payload 数值变化 |
| 10:00 | 打坐真元缓涨 | **现状为纯被动**（`cultivation/tick.rs:7` 自注 P1 简化）——本 leg 只弱断言被动涨 qi 存在，主动打坐归 [[plan-dazuo-v1]]，此处标 SKIP+依赖 |
| 15:00 | 第一条经脉 + 经脉图 | `SetMeridianTarget` intent → meridian 进度 payload → 打通事件 |
| 20:00 | 噬元鼠偷真元（不掉血掉真元） | 走近灵泉触发 `dynamic_rat_swarm_spawner` → qi_current 下降且 HP 不降 |
| 25:00 | 0.5+ 灵气小区域存在 | 读 spawn zone raster/POI 注入坐标，断言 ≥1 处 qi ≥ 0.5（`tutorial_lingquan` 是否达标） |
| 27:00 | 3 分钟突破窗口（脆弱期可打断） | `BreakthroughRequest` → 窗口事件 payload；打断分支单独 leg |
| 30:00 | 醒灵→引气世界变色 | 突破成功 + realm_vision 族 payload 到达 |

- 交付：场景文件 + 本 plan §9 核验矩阵（每钩子 ✅/❌/⚠️ + 证据 payload 摘录）。
- 测试：场景自身即测试；leg 级失败信息带修复线索（期望 X 因为 §L 第 N 行，实际 Z）。

## P1 — 断点修复 ⬜

按 P0 矩阵实修，**已知候选**（P0 前即有证据）：

- 首句 narration 缺「新角色 vs 重生角色」分支（journey §L:750 自标"需补"）——接 `spawn_tutorial` 触发点 + narration 模板双文案（§L:760-761 台词为准）。
- `TutorialHook::FirstSitMeditate` 误名钩子（`spawn_tutorial.rs:693` qi>0 即自动触发，非玩家动作）——本 plan 只修**触发时序错误**；钩子改挂真打坐动作依赖 [[plan-dazuo-v1]] P0，落地前保持现状并在矩阵标注。
- 0.5+ 灵气点若实测不达标 → 调 spawn 注入配置（worldgen POI 注入或 zone_qi 配置，数据侧改动）。
- **范围排除**（防 scope 蔓延，各归专项 plan）：主动打坐 → [[plan-dazuo-v1]]；第一个招式获取 → [[plan-first-technique-grant-v1]]；噬元鼠数值/生态 → fauna 族 plan。P1 只登记依赖不重复实现。

## P2 — 收口 ⬜

- 30min 场景进 CI bot e2e stage（`scripts/bot-e2e.sh` 注册；SKIP leg 允许存在但必须显式打印依赖 plan 名）。
- 真人 30 分钟实测一轮，记录逐钩子体感（结论供 journey §H「100h 实测」复用，避免重复劳动）。
- 全钩子 ✅/显式 SKIP 后写 Finish Evidence 归档。

## §8 开放问题（升 active / P0 决策门前收口）

1. **10:00 打坐钩子的断言策略**：[[plan-dazuo-v1]] 落地前，30min 场景对该钩子是 SKIP 标注（推荐——诚实反映缺口）还是按被动涨 qi 做弱断言（会掩盖"没有主动动作"这一核心体感缺口）。
2. **时序压缩比**：bot 不真等 30 分钟——`/time advance` 快进 vs 直接触发各钩子前置条件；快进会不会跳过 `dynamic_rat_swarm_spawner` 的距离触发逻辑，需实地核对。
3. **重生角色分支的测试路径**：bot 如何进入"重生"态（`/kill self` + `/revive self` dev 链 vs 真死亡重生链）——影响 0:00 分支断言的可信度。
4. **矩阵回流 journey plan 的机制**：本 plan 归档时 journey §L 各行状态由谁回标（人工 / journey 自己的下个 PR），须留交接记录避免两份文档漂移。
