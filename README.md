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

_自动生成于 2026-04-28 · 共 57 份 plan_

```
总进度  ███████████████████░░░░░░░░░░░  64.7%
```

**分布**：`merged` 9 · `wip` 21 · `design` 8 · `skeleton` 7 · `done` 12

### 坍缩渊（TSY）
_搜打撤独立位面玩法（10 子 plan） · 10 份 · 组均 91%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `merged` | **TSY 位面基础设施** <br/><sub>`plan-tsy-dimension-v1.md`</sub> | `████████████`  97% | #47 | 2026-04-26 |
| `wip` | **TSY 容器搜刮系统（5 档 + 钥匙 + 真元加速）** <br/><sub>`plan-tsy-container-v1.md`</sub> | `███████████░`  95% | #55 | 2026-04-27 |
| `merged` | **TSY 撤离点（RiftPortal + 撤离倒计时 + race-out）** <br/><sub>`plan-tsy-extract-v1.md`</sub> | `███████████░`  93% | #59 | 2026-04-27 |
| `wip` | **TSY 物资 99/1 + 秘境分流死亡 + 干尸** <br/><sub>`plan-tsy-loot-v1.md`</sub> | `███████████░`  92% | #53 | 2026-04-27 |
| `merged` | **TSY Zone P0 收尾（集成测 + Server→Redis 桥）** <br/><sub>`plan-tsy-zone-followup-v1.md`</sub> | `███████████░`  92% | #50 | 2026-04-26 |
| `merged` | **TSY Zone P0 基础** <br/><sub>`plan-tsy-zone-v1.md`</sub> | `███████████░`  92% | #49 | 2026-04-26 |
| `wip` | **TSY 生命周期（状态机 + 塌缩 + 道伥）** <br/><sub>`plan-tsy-lifecycle-v1.md`</sub> | `███████████░`  90% | #54 | 2026-04-27 |
| `merged` | **TSY 地形/POI/NPC anchor 自动生成** <br/><sub>`plan-tsy-worldgen-v1.md`</sub> | `███████████░`  90% | #51 | 2026-04-27 |
| `wip` | **TSY 敌对 NPC 四档（道伥/执念/守灵/畸变体）** <br/><sub>`plan-tsy-hostile-v1.md`</sub> | `███████████░`  88% | — | 2026-04-27 |
| `wip` | **搜打撤坍缩渊 meta plan** <br/><sub>`plan-tsy-v1.md`</sub> | `██████████░░`  85% | — | 2026-04-26 |

### 战斗 / HUD / 视觉
_战斗 ECS、流派、HUD、粒子、动画、Iris · 10 份 · 组均 48%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `merged` | **护甲减免系统：ArmorProfile + 耐久 + 体修 buff** <br/><sub>`plan-armor-v1.md`</sub> | `████████████` 100% | #46 #52 #56 | 2026-04-27 |
| `wip` | **HUD 设计：双层快捷栏 + 三状态条 + 事件流** <br/><sub>`plan-HUD-v1.md`</sub> | `██████████░░`  80% | #43 | 2026-04-24 |
| `wip` | **战斗系统客户端 UI（施法/死亡/防御/天劫）** <br/><sub>`plan-combat-ui_impl.md`</sub> | `█████████░░░`  75% | #52 #56 | 2026-04-27 |
| `wip` | **玩家动画：PlayerAnimator + JSON 资产 + VFX 协议** <br/><sub>`plan-player-animation-v1.md`</sub> | `████████░░░░`  70% | — | 2026-04-25 |
| `wip` | **粒子与世界内 VFX 系统：三基类 + 触发协议** <br/><sub>`plan-particle-system-v1.md`</sub> | `███████░░░░░`  55% | — | 2026-04-28 |
| `wip` | **战斗系统服务端 ECS + IPC schema（无 UI）** <br/><sub>`plan-combat-no_ui.md`</sub> | `█████░░░░░░░`  40% | #29 #30 | 2026-04-21 |
| `wip` | **武器法宝数据模型 + 装备槽 + 3D 渲染骨架** <br/><sub>`plan-weapon-v1.md`</sub> | `████░░░░░░░░`  35% | #41 | 2026-04-25 |
| `design` | **体修·爆脉流崩拳 P0 + 后续 4 招蓝图** <br/><sub>`plan-baomai-v1.md`</sub> | `█░░░░░░░░░░░`  10% | — | 2026-04-27 |
| `design` | **快捷栏双行重构：1-9 战斗技能行 + F1-F9 物品行** <br/><sub>`plan-hotbar-modify-v1.md`</sub> | `█░░░░░░░░░░░`  10% | — | 2026-04-27 |
| `design` | **Iris 光影集成：修仙状态驱动 shader** <br/><sub>`plan-iris-integration-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | 2026-04-25 |

### 修炼 / 经济
_六境修炼、天劫、炼丹/炼器、矿物、灵田、保质期 · 6 份 · 组均 71%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `merged` | **炼丹专项：配方/熔炉/火候三系统 + 服药丹毒** <br/><sub>`plan-alchemy-v1.md`</sub> | `███████████░`  93% | #21 #28 | 2026-04-27 |
| `wip` | **矿物材料专项：矿脉运行时 + forge/alchemy 钩子全链路** <br/><sub>`plan-mineral-v1.md`</sub> | `███████████░`  88% | #31 #44 | 2026-04-27 |
| `wip` | **灵田专项：开垦/种植/补灵/收获/密度阈值完整闭环** <br/><sub>`plan-lingtian-v1.md`</sub> | `██████████░░`  82% | #26 | 2026-04-26 |
| `wip` | **通用保质期系统：三路径衰减/腐败/陈化 + 消费侧接入** <br/><sub>`plan-shelflife-v1.md`</sub> | `██████████░░`  80% | #32 #33 #34 #35 #36 #37 #38 #39 #40 | 2026-04-27 |
| `merged` | **炼器（武器）专项：四步状态机 + IPC Schema + 客户端占位** <br/><sub>`plan-forge-v1.md`</sub> | `████████░░░░`  65% | #19 #61 | 2026-04-28 |
| `design` | **天劫专项：渡虚劫/域崩/定向天罚三类天道手段** <br/><sub>`plan-tribulation-v1.md`</sub> | `██░░░░░░░░░░`  18% | — | 2026-04-27 |

### 玩法 / NPC / 世界
_背包、NPC AI、感知、社交、技艺、死亡周期 · 7 份 · 组均 49%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `merged` | **死亡 / 运数 / 寿元 / 遗念 / 亡者博物馆** <br/><sub>`plan-death-lifecycle-v1.md`</sub> | `███████████░`  92% | — | 2026-04-27 |
| `wip` | **权威背包系统 + 掉落闭环 + 丹药应用** <br/><sub>`plan-inventory-v1.md`</sub> | `███████████░`  88% | #27 | 2026-04-26 |
| `wip` | **NPC 多 archetype / 寿元 / 派系 / LOD 行为** <br/><sub>`plan-npc-ai-v1.md`</sub> | `█████████░░░`  72% | #22 #45 | 2026-04-25 |
| `wip` | **子技能系统（采药/炼丹/锻造 XP + 升级）** <br/><sub>`plan-skill-v1.md`</sub> | `█████████░░░`  72% | #42 | 2026-04-24 |
| `design` | **匿名社会 / 关系图 / 灵龛 / 声名** <br/><sub>`plan-social-v1.md`</sub> | `█░░░░░░░░░░░`   8% | — | 2026-04-25 |
| `design` | **NPC 假玩家实体 + MineSkin 自定义皮肤** <br/><sub>`plan-npc-skin-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | 2026-04-25 |
| `design` | **肉眼视野 + 神识感知双系统** <br/><sub>`plan-perception-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | 2026-04-25 |

### 基础设施 / 工作流
_IPC schema、持久化、工作流、内容、音效 · 5 份 · 组均 57%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `wip` | **末法残土图书馆 18 册 Astro content collection 内容填充** <br/><sub>`plan-library-web-content-v1.md`</sub> | `██████████░░`  80% | #11 | 2026-04-27 |
| `wip` | **server/agent 双侧持久化：SQLite 主存储 + Redis 仅缓存** <br/><sub>`plan-persistence-v1.md`</sub> | `█████████░░░`  75% | #24 | 2026-04-26 |
| `wip` | **Redis channel + TypeBox schema 双端对齐规范** <br/><sub>`plan-ipc-schema-v1.md`</sub> | `████████░░░░`  70% | — | 2026-04-26 |
| `wip` | **opencode + oh-my-opencode 全自动 plan 消费流水线** <br/><sub>`plan-opencode-workflow-v1.md`</sub> | `███████░░░░░`  60% | #15 | 2026-04-26 |
| `design` | **音效/音乐专项：100% 复用 MC vanilla SoundEvent 零自制** <br/><sub>`plan-audio-v1.md`</sub> | `░░░░░░░░░░░░`   0% | — | 2026-04-26 |

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
_M0/M1 阶段产物 + 已 docs/finished_plans 的子 plan · 12 份 · 组均 100%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `done` | **MVP 0.1 — Server scaffold + NPC + Fabric Client** <br/><sub>`mvp01-plan.md`</sub> | `████████████` 100% | — | 2026-03-25 |
| `done` | **Agent 端到端集成与可观测** <br/><sub>`plan-agent-v2.md`</sub> | `████████████` 100% | — | 2026-04-13 |
| `done` | **天道 Agent 闭环（v1）** <br/><sub>`plan-agent.md`</sub> | `████████████` 100% | — | 2026-04-10 |
| `done` | **野生植物采集生态** <br/><sub>`plan-botany-v1.md`</sub> | `████████████` 100% | — | 2026-04-25 |
| `done` | **Client Mod 网络消息路由** <br/><sub>`plan-client.md`</sub> | `████████████` 100% | — | 2026-04-20 |
| `done` | **Cultivation 双头清理：删旧 MVP 占位** <br/><sub>`plan-cultivation-mvp-cleanup-v1.md`</sub> | `████████████` 100% | #48 | 2026-04-27 |
| `done` | **修炼系统：六境/经脉/真元/污染/突破/顿悟** <br/><sub>`plan-cultivation-v1.md`</sub> | `████████████` 100% | #21 #26 #28 #29 #48 | 2026-04-27 |
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
