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

_自动生成于 2026-04-28 · 共 59 份 plan_

```
总进度  █████████████████████░░░░░░░░░  71.2%
```

**分布**：`merged` 10 · `wip` 8 · `design` 6 · `skeleton` 7 · `done` 28

### 战斗 / HUD / 视觉
_战斗 ECS、流派、HUD、粒子、动画、Iris · 9 份 · 组均 60%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `merged` | **武器 v1.1 补完：schema 单一来源与验收证据** <br/><sub>`plan-weapon-v1.1.md`</sub> | `███████████░`  95% | #69 | 2026-04-28 |
| `merged` | **战斗系统客户端 UI 全量实现 U1–U7** <br/><sub>`plan-combat-ui_impl.md`</sub> | `███████████░`  90% | #20 | 2026-04-25 |
| `merged` | **武器法宝：数据模型 + 装备槽 + 3D 渲染** <br/><sub>`plan-weapon-v1.md`</sub> | `███████████░`  90% | #41 | 2026-04-25 |
| `wip` | **HUD 双层快捷栏 + 三状态条 + 事件流** <br/><sub>`plan-HUD-v1.md`</sub> | `██████████░░`  80% | #43 | 2026-04-25 |
| `wip` | **玩家动画 PlayerAnimator + 20 个 JSON 资产** <br/><sub>`plan-player-animation-v1.md`</sub> | `████████░░░░`  70% | — | 2026-04-25 |
| `wip` | **粒子与世界内 VFX 三基类 + 触发协议** <br/><sub>`plan-particle-system-v1.md`</sub> | `███████░░░░░`  60% | — | 2026-04-28 |
| `wip` | **快捷栏双行重构：1-9 技能行与 F1-F9 物品行** <br/><sub>`plan-hotbar-modify-v1.md`</sub> | `█████░░░░░░░`  45% | #65 | 2026-04-27 |
| `design` | **体修爆脉流 P0 崩拳战斗功法** <br/><sub>`plan-baomai-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | 2026-04-27 |
| `design` | **Iris 光影集成：修仙状态驱动 shader** <br/><sub>`plan-iris-integration-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | 2026-04-25 |

### 修炼 / 经济
_六境修炼、天劫、炼丹/炼器、矿物、灵田、保质期 · 4 份 · 组均 73%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `merged` | **炼器全链路收口：schema sample + 装备写入 + 三块客户端 UI** <br/><sub>`plan-forge-leftovers-v1.md`</sub> | `███████████░`  90% | #66 | 2026-04-28 |
| `merged` | **矿物材料正典：MineralRegistry + worldgen + forge/alchemy 钩子** <br/><sub>`plan-mineral-v1.md`</sub> | `███████████░`  90% | #44 | 2026-04-27 |
| `merged` | **灵田种植全链路：开垦/种植/生长/收获/补灵/偷菜** <br/><sub>`plan-lingtian-v1.md`</sub> | `███████████░`  88% | #26 | 2026-04-25 |
| `wip` | **天劫专项：渡虚劫/域崩/定向天罚三类天道手段** <br/><sub>`plan-tribulation-v1.md`</sub> | `███░░░░░░░░░`  25% | — | 2026-04-25 |

### 玩法 / NPC / 世界
_背包、NPC AI、感知、社交、技艺、死亡周期 · 6 份 · 组均 42%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `merged` | **背包系统 — Server 权威 inventory + dropped loot** <br/><sub>`plan-inventory-v1.md`</sub> | `███████████░`  88% | #27 | 2026-04-25 |
| `merged` | **子技能系统 — herbalism/alchemy/forging 熟练度** <br/><sub>`plan-skill-v1.md`</sub> | `█████████░░░`  78% | #42 | 2026-04-25 |
| `wip` | **NPC 行为 / archetype / 派系 / 渡劫 / LOD** <br/><sub>`plan-npc-ai-v1.md`</sub> | `████████░░░░`  68% | #22 #45 | 2026-04-25 |
| `design` | **玩家匿名 / 关系 / 声名 / 灵龛社交系统** <br/><sub>`plan-social-v1.md`</sub> | `█░░░░░░░░░░░`   8% | — | 2026-04-25 |
| `design` | **NPC 假玩家实体 + MineSkin 自定义皮肤注入** <br/><sub>`plan-npc-skin-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | 2026-04-25 |
| `design` | **视觉距离 + 神识感知双系统** <br/><sub>`plan-perception-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | 2026-04-25 |

### 基础设施 / 工作流
_IPC schema、持久化、工作流、内容、音效 · 5 份 · 组均 55%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `merged` | **Redis channel + TypeBox schema 双端对齐管理** <br/><sub>`plan-ipc-schema-v1.md`</sub> | `██████████░░`  80% | — | 2026-04-25 |
| `merged` | **Server/Agent 统一 SQLite 持久化存档规范** <br/><sub>`plan-persistence-v1.md`</sub> | `█████████░░░`  75% | #24 | 2026-04-25 |
| `wip` | **末法残土图书馆 18 册 Astro 内容填充** <br/><sub>`plan-library-web-content-v1.md`</sub> | `███████░░░░░`  60% | — | 2026-04-25 |
| `wip` | **opencode + oh-my-opencode 全自动 plan 消费流水线** <br/><sub>`plan-opencode-workflow-v1.md`</sub> | `███████░░░░░`  55% | #15 | 2026-04-24 |
| `design` | **修仙音效：100% 复用 vanilla SoundEvent 组合播放** <br/><sub>`plan-audio-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | 2026-04-25 |

### 骨架 plan
_战斗流派 + 快捷栏，等待开工 · 7 份 · 组均 6%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `skeleton` | **天道叙事内容侧** <br/><sub>`plan-narrative-v1.md`</sub> | `█░░░░░░░░░░░`  10% | — | — |
| `skeleton` | **器修·暗器流** <br/><sub>`plan-anqi-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | — |
| `skeleton` | **毒蛊流** <br/><sub>`plan-dugu-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | — |
| `skeleton` | **替尸·蜕壳流** <br/><sub>`plan-tuike-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | — |
| `skeleton` | **绝灵·涡流流** <br/><sub>`plan-woliu-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | — |
| `skeleton` | **地师·阵法流** <br/><sub>`plan-zhenfa-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | — |
| `skeleton` | **截脉·震爆流** <br/><sub>`plan-zhenmai-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | — |

### 已完成归档
_M0/M1 阶段产物 + 已 docs/finished_plans 的子 plan · 28 份 · 组均 100%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `done` | **MVP 0.1 — Server scaffold + NPC + Fabric Client** <br/><sub>`mvp01-plan.md`</sub> | `████████████` 100% | — | 2026-03-25 |
| `done` | **Agent 端到端集成与可观测** <br/><sub>`plan-agent-v2.md`</sub> | `████████████` 100% | — | 2026-04-13 |
| `done` | **天道 Agent 闭环（v1）** <br/><sub>`plan-agent.md`</sub> | `████████████` 100% | — | 2026-04-10 |
| `done` | **炼丹专项：配方/熔炉/火候三系统 + 服药丹毒** <br/><sub>`plan-alchemy-v1.md`</sub> | `████████████` 100% | #21 #28 | 2026-04-27 |
| `done` | **护甲减免系统：ArmorProfile + 耐久 + 体修 buff** <br/><sub>`plan-armor-v1.md`</sub> | `████████████` 100% | #46 #52 #56 | 2026-04-27 |
| `done` | **野生植物采集生态** <br/><sub>`plan-botany-v1.md`</sub> | `████████████` 100% | — | 2026-04-25 |
| `done` | **Client Mod 网络消息路由** <br/><sub>`plan-client.md`</sub> | `████████████` 100% | — | 2026-04-20 |
| `done` | **战斗系统服务端 ECS + IPC schema（无 UI）** <br/><sub>`plan-combat-no_ui.md`</sub> | `████████████` 100% | #29 #30 | 2026-04-21 |
| `done` | **Cultivation 双头清理：删旧 MVP 占位** <br/><sub>`plan-cultivation-mvp-cleanup-v1.md`</sub> | `████████████` 100% | #48 | 2026-04-27 |
| `done` | **修炼系统：六境/经脉/真元/污染/突破/顿悟** <br/><sub>`plan-cultivation-v1.md`</sub> | `████████████` 100% | #21 #26 #28 #29 #48 | 2026-04-27 |
| `done` | **死亡 / 运数 / 寿元 / 遗念 / 亡者博物馆** <br/><sub>`plan-death-lifecycle-v1.md`</sub> | `████████████` 100% | — | 2026-04-27 |
| `done` | **炼器（武器）专项：四步状态机 + IPC Schema + 客户端占位** <br/><sub>`plan-forge-v1.md`</sub> | `████████████` 100% | #19 #61 | 2026-04-28 |
| `done` | **Server 基础设施闭环** <br/><sub>`plan-server.md`</sub> | `████████████` 100% | — | 2026-04-21 |
| `done` | **通用保质期系统：三路径衰减/腐败/陈化 + 消费侧接入** <br/><sub>`plan-shelflife-v1.md`</sub> | `████████████` 100% | #32 #33 #34 #35 #36 #37 #38 #39 #40 #67 | 2026-04-27 |
| `done` | **TSY 容器搜刮系统（5 档 + 钥匙 + 真元加速）** <br/><sub>`plan-tsy-container-v1.md`</sub> | `████████████` 100% | #55 | 2026-04-27 |
| `done` | **TSY 位面基础设施** <br/><sub>`plan-tsy-dimension-v1.md`</sub> | `████████████` 100% | #47 | 2026-04-26 |
| `done` | **TSY 撤离点（RiftPortal + 撤离倒计时 + race-out）** <br/><sub>`plan-tsy-extract-v1.md`</sub> | `████████████` 100% | #59 | 2026-04-27 |
| `done` | **TSY 敌对 NPC 四档（道伥/执念/守灵/畸变体）** <br/><sub>`plan-tsy-hostile-v1.md`</sub> | `████████████` 100% | — | 2026-04-27 |
| `done` | **TSY 生命周期（状态机 + 塌缩 + 道伥）** <br/><sub>`plan-tsy-lifecycle-v1.md`</sub> | `████████████` 100% | #54 | 2026-04-27 |
| `done` | **TSY 物资 99/1 + 秘境分流死亡 + 干尸** <br/><sub>`plan-tsy-loot-v1.md`</sub> | `████████████` 100% | #53 | 2026-04-27 |
| `done` | **搜打撤坍缩渊 meta plan** <br/><sub>`plan-tsy-v1.md`</sub> | `████████████` 100% | #47 #49 #50 #51 #53 #54 #55 #59 | 2026-04-27 |
| `done` | **TSY 地形/POI/NPC anchor 自动生成** <br/><sub>`plan-tsy-worldgen-v1.md`</sub> | `████████████` 100% | #51 | 2026-04-27 |
| `done` | **TSY Zone P0 收尾（集成测 + Server→Redis 桥）** <br/><sub>`plan-tsy-zone-followup-v1.md`</sub> | `████████████` 100% | #50 | 2026-04-26 |
| `done` | **TSY Zone P0 基础** <br/><sub>`plan-tsy-zone-v1.md`</sub> | `████████████` 100% | #49 | 2026-04-26 |
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
