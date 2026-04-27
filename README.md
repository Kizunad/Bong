# Bong

AI-Native Xianxia (修仙) sandbox on Minecraft. Three-layer architecture:

- **server/** — Rust 无头 MC 服务器（Valence on Bevy 0.14 ECS，MC 1.20.1 协议 763）
- **client/** — Fabric 1.20.1 微端（Java 17，owo-lib UI）
- **agent/** — LLM "天道" agent 层（TypeScript，三 Agent 并发推演）
- **worldgen/** — Python 地形生成流水线
- **library-web/** — 末法残土图书馆前端（Astro）

详见 [`CLAUDE.md`](CLAUDE.md)。

<!-- BEGIN:PLANS_PROGRESS -->
## Plan 进度

_自动生成于 2026-04-27 · 共 56 份 plan_

```
总进度  ████████████████░░░░░░░░░░░░░░  54.9%
```

**分布**：`merged` 11 · `wip` 19 · `design` 8 · `skeleton` 9 · `done` 9

### 坍缩渊（TSY）
_搜打撤独立位面玩法（10 子 plan） · 10 份 · 组均 62%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `merged` | **TSY 位面基础设施** <br/><sub>`plan-tsy-dimension-v1.md`</sub> | `███████████░`  95% | #47 | 2026-04-26 |
| `merged` | **TSY Zone P0 收尾（集成测+Redis 桥）** <br/><sub>`plan-tsy-zone-followup-v1.md`</sub> | `███████████░`  95% | #49 #50 | 2026-04-26 |
| `merged` | **TSY Zone P0 基础** <br/><sub>`plan-tsy-zone-v1.md`</sub> | `███████████░`  95% | #47 #49 | 2026-04-26 |
| `merged` | **TSY 生命周期与道伥** <br/><sub>`plan-tsy-lifecycle-v1.md`</sub> | `███████████░`  90% | #54 | 2026-04-27 |
| `merged` | **TSY 物资与秘境死亡分流** <br/><sub>`plan-tsy-loot-v1.md`</sub> | `███████████░`  90% | #53 | 2026-04-27 |
| `merged` | **TSY Worldgen + POI consumer** <br/><sub>`plan-tsy-worldgen-v1.md`</sub> | `█████████░░░`  75% | #51 | 2026-04-26 |
| `wip` | **搜打撤坍缩渊 meta plan** <br/><sub>`plan-tsy-v1.md`</sub> | `████████░░░░`  70% | #47 #49 #50 #51 #53 #54 | 2026-04-27 |
| `design` | **TSY 容器与搜刮** <br/><sub>`plan-tsy-container-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | 2026-04-26 |
| `design` | **TSY 敌对 NPC（道伥/执念/守灵）** <br/><sub>`plan-tsy-hostile-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | 2026-04-27 |
| `design` | **TSY 撤离点与 race-out** <br/><sub>`plan-tsy-extract-v1.md`</sub> | `░░░░░░░░░░░░`   3% | — | 2026-04-26 |

### 战斗 / HUD / 视觉
_战斗 ECS、流派、HUD、粒子、动画、Iris · 8 份 · 组均 53%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `wip` | **战斗系统客户端 UI** <br/><sub>`plan-combat-ui_impl.md`</sub> | `██████████░░`  80% | #52 | 2026-04-25 |
| `wip` | **客户端 HUD 全套（双层快捷栏）** <br/><sub>`plan-HUD-v1.md`</sub> | `█████████░░░`  78% | #52 | 2026-04-25 |
| `wip` | **武器/法宝（ItemInstance→3D 模型）** <br/><sub>`plan-weapon-v1.md`</sub> | `████████░░░░`  65% | #41 | 2026-04-25 |
| `wip` | **PlayerAnimator 玩家动画系统** <br/><sub>`plan-player-animation-v1.md`</sub> | `███████░░░░░`  60% | — | 2026-04-25 |
| `wip` | **粒子/世界内 VFX 系统** <br/><sub>`plan-particle-system-v1.md`</sub> | `███████░░░░░`  55% | — | 2026-04-25 |
| `wip` | **护甲减免（WoundKind×BodyPart）** <br/><sub>`plan-armor-v1.md`</sub> | `█████░░░░░░░`  45% | #46 #52 | 2026-04-25 |
| `wip` | **战斗 ECS（伤害/流派/状态/死亡）** <br/><sub>`plan-combat-no_ui.md`</sub> | `█████░░░░░░░`  40% | — | 2026-04-25 |
| `design` | **Iris 光影集成（修仙状态驱动）** <br/><sub>`plan-iris-integration-v1.md`</sub> | `░░░░░░░░░░░░`   2% | — | 2026-04-25 |

### 修炼 / 经济
_六境修炼、天劫、炼丹/炼器、矿物、灵田、保质期 · 8 份 · 组均 66%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `merged` | **Cultivation 单一来源清理** <br/><sub>`plan-cultivation-mvp-cleanup-v1.md`</sub> | `████████████`  98% | #48 | 2026-04-26 |
| `merged` | **核心修炼系统（六境/经脉/真元/顿悟）** <br/><sub>`plan-cultivation-v1.md`</sub> | `███████████░`  95% | — | 2026-04-25 |
| `merged` | **灵田种植（开垦/种植/补灵/收获）** <br/><sub>`plan-lingtian-v1.md`</sub> | `███████████░`  88% | #26 | 2026-04-25 |
| `wip` | **矿物材料（18 矿 + NFT 流转）** <br/><sub>`plan-mineral-v1.md`</sub> | `█████████░░░`  72% | #31 #44 | 2026-04-24 |
| `wip` | **保质期/过期（Decay/Spoil/Age）** <br/><sub>`plan-shelflife-v1.md`</sub> | `████████░░░░`  65% | #32 #33 #34 #35 #36 #37 #38 #39 #40 | 2026-04-25 |
| `wip` | **炼丹系统（配方/熔炉/火候）** <br/><sub>`plan-alchemy-v1.md`</sub> | `███████░░░░░`  55% | #21 #28 | 2026-04-25 |
| `wip` | **炼器系统（武器四步锻造）** <br/><sub>`plan-forge-v1.md`</sub> | `█████░░░░░░░`  40% | #19 | 2026-04-25 |
| `wip` | **天劫系统（虚劫/域崩/天罚）** <br/><sub>`plan-tribulation-v1.md`</sub> | `██░░░░░░░░░░`  15% | — | 2026-04-25 |

### 玩法 / NPC / 世界
_背包、NPC AI、感知、社交、技艺、死亡周期 · 7 份 · 组均 36%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `merged` | **权威背包/掉落/丹药/重量** <br/><sub>`plan-inventory-v1.md`</sub> | `██████████░░`  85% | #27 | 2026-04-25 |
| `merged` | **子技能 XP/升级/残卷/境界 cap** <br/><sub>`plan-skill-v1.md`</sub> | `█████████░░░`  75% | #42 | 2026-04-25 |
| `wip` | **NPC AI（archetype/生命周期/派系）** <br/><sub>`plan-npc-ai-v1.md`</sub> | `████████░░░░`  65% | #45 | 2026-04-25 |
| `wip` | **玩家死亡/重生/寿元/终结** <br/><sub>`plan-death-lifecycle-v1.md`</sub> | `██░░░░░░░░░░`  20% | — | 2026-04-25 |
| `design` | **匿名社会/关系图/灵龛/声名** <br/><sub>`plan-social-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | 2026-04-25 |
| `design` | **肉眼视距 + 神识感知双系统** <br/><sub>`plan-perception-v1.md`</sub> | `░░░░░░░░░░░░`   3% | — | 2026-04-25 |
| `design` | **NPC 假玩家实体 + MineSkin** <br/><sub>`plan-npc-skin-v1.md`</sub> | `░░░░░░░░░░░░`   0% | — | 2026-04-25 |

### 基础设施 / 工作流
_IPC schema、持久化、工作流、内容、音效 · 5 份 · 组均 58%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `wip` | **SQLite WAL 全局持久化** <br/><sub>`plan-persistence-v1.md`</sub> | `██████████░░`  82% | #24 | 2026-04-25 |
| `wip` | **Redis channel + TypeBox schema 双端对齐** <br/><sub>`plan-ipc-schema-v1.md`</sub> | `█████████░░░`  75% | — | 2026-04-25 |
| `wip` | **末法残土图书馆内容（28 册）** <br/><sub>`plan-library-web-content-v1.md`</sub> | `████████░░░░`  70% | — | 2026-04-25 |
| `wip` | **opencode 全自动 plan 消费流水线** <br/><sub>`plan-opencode-workflow-v1.md`</sub> | `███████░░░░░`  60% | — | 2026-04-25 |
| `design` | **零自制资源 vanilla 音效层叠** <br/><sub>`plan-audio-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | 2026-04-25 |

### 骨架 plan
_战斗流派 + 快捷栏，等待开工 · 9 份 · 组均 6%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `skeleton` | **天道叙事内容侧** <br/><sub>`plan-narrative-v1.md`</sub> | `█░░░░░░░░░░░`  10% | — | — |
| `skeleton` | **快捷栏双行重构** <br/><sub>`plan-hotbar-modify-v1.md`</sub> | `█░░░░░░░░░░░`   8% | — | — |
| `skeleton` | **器修·暗器流** <br/><sub>`plan-anqi-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | — |
| `skeleton` | **体修·爆脉流** <br/><sub>`plan-baomai-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | — |
| `skeleton` | **毒蛊流** <br/><sub>`plan-dugu-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | — |
| `skeleton` | **替尸·蜕壳流** <br/><sub>`plan-tuike-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | — |
| `skeleton` | **绝灵·涡流流** <br/><sub>`plan-woliu-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | — |
| `skeleton` | **地师·阵法流** <br/><sub>`plan-zhenfa-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | — |
| `skeleton` | **截脉·震爆流** <br/><sub>`plan-zhenmai-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | — |

### 已完成归档
_M0/M1 阶段产物 · 9 份 · 组均 100%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `done` | **Agent 端到端集成与可观测** <br/><sub>`plan-agent-v2.md`</sub> | `████████████` 100% | — | 2026-04-13 |
| `done` | **天道 Agent 闭环（v1）** <br/><sub>`plan-agent.md`</sub> | `████████████` 100% | — | 2026-04-10 |
| `done` | **野生植物采集生态** <br/><sub>`plan-botany-v1.md`</sub> | `████████████` 100% | — | 2026-04-25 |
| `done` | **Client Mod 网络消息路由** <br/><sub>`plan-client.md`</sub> | `████████████` 100% | — | 2026-04-20 |
| `done` | **Server 基础设施闭环** <br/><sub>`plan-server.md`</sub> | `████████████` 100% | — | 2026-04-21 |
| `done` | **视觉特效基础栈** <br/><sub>`plan-vfx-v1.md`</sub> | `████████████` 100% | — | 2026-04-13 |
| `done` | **巨树生成方向** <br/><sub>`plan-worldgen-v3.1.md`</sub> | `████████████` 100% | — | 2026-04-13 |
| `done` | **Rust 运行时地形生成** <br/><sub>`plan-worldgen-v3.md`</sub> | `████████████` 100% | — | 2026-04-20 |
| `done` | **世界生成混合方案** <br/><sub>`plan-worldgen.md`</sub> | `████████████` 100% | — | 2026-03-30 |

### 图例

- `merged` — 代码已合并主线，plan 主体落地
- `wip` — 设计 active，部分代码已落地，仍在推进
- `design` — 设计 active，零或近零代码
- `skeleton` — 骨架 plan，等待开工
- `done` — 已归档（M0/M1 阶段产物）

_数据源：[`docs/plans-progress.yaml`](docs/plans-progress.yaml) · 渲染脚本：[`scripts/plans_progress.py`](scripts/plans_progress.py) · 经 GitHub Action 在 plan 改动时自动更新_
<!-- END:PLANS_PROGRESS -->
