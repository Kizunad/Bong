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

_自动生成于 2026-04-29 · 共 72 份 plan_

```
总进度  ████████████████████░░░░░░░░░░  66.7%
```

**分布**：`merged` 12 · `wip` 7 · `design` 4 · `skeleton` 17 · `done` 32

### 战斗 / HUD / 视觉
_战斗 ECS、流派、HUD、粒子、动画、Iris · 8 份 · 组均 72%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `merged` | **武器 v1.1 补完：schema 对齐 / channel 语义 / Evidence 闭环** <br/><sub>`plan-weapon-v1.1.md`</sub> | `███████████░`  95% | #69 | 2026-04-27 |
| `merged` | **战斗系统客户端 UI 全套（screens/stores/HUD planners）** <br/><sub>`plan-combat-ui_impl.md`</sub> | `███████████░`  90% | #20 | 2026-04-25 |
| `merged` | **武器法宝：ItemInstance+Weapon Component+3D 渲染+装备槽** <br/><sub>`plan-weapon-v1.md`</sub> | `██████████░░`  85% | #41 | 2026-04-25 |
| `wip` | **客户端全局 HUD（双层快捷栏+三状态条+事件流）** <br/><sub>`plan-HUD-v1.md`</sub> | `██████████░░`  82% | #43 | 2026-04-25 |
| `merged` | **粒子与世界 VFX（Line/Ribbon/GroundDecal 三基类）** <br/><sub>`plan-particle-system-v1.md`</sub> | `██████████░░`  80% | #17 #64 | 2026-04-28 |
| `wip` | **玩家动画系统（PlayerAnimator + AI 生产线 + 首批 20 动画）** <br/><sub>`plan-player-animation-v1.md`</sub> | `█████████░░░`  75% | — | 2026-04-25 |
| `merged` | **双行快捷栏：1-9 技能行 + F1-F9 物品行分离** <br/><sub>`plan-hotbar-modify-v1.md`</sub> | `████████░░░░`  65% | #65 | 2026-04-28 |
| `design` | **Iris 光影集成：修仙状态驱动 shader uniform** <br/><sub>`plan-iris-integration-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | 2026-04-25 |

### 修炼 / 经济
_六境修炼、天劫、炼丹/炼器、矿物、灵田、保质期 · 5 份 · 组均 57%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `merged` | **灵田人工种植 — 开垦→种植→补灵→收获闭环** <br/><sub>`plan-lingtian-v1.md`</sub> | `███████████░`  95% | #26 | 2026-04-26 |
| `merged` | **矿物材料正典 — 18 矿物 + worldgen + forge/alchemy 钩子** <br/><sub>`plan-mineral-v1.md`</sub> | `███████████░`  90% | #31 #44 #57 | 2026-04-27 |
| `merged` | **炼器全链路收口 — schema / bridge / 装备写入 / 三块 UI** <br/><sub>`plan-forge-leftovers-v1.md`</sub> | `██████████░░`  85% | #62 #66 | 2026-04-28 |
| `wip` | **天劫 — 渡虚劫 / 域崩 / 定向天罚** <br/><sub>`plan-tribulation-v1.md`</sub> | `██░░░░░░░░░░`  15% | — | 2026-04-27 |
| `design` | **末法残土植物生态扩展 — 17 新物种 + 三层抽象** <br/><sub>`plan-botany-v2.md`</sub> | `░░░░░░░░░░░░`   0% | — | 2026-04-29 |

### 玩法 / NPC / 世界
_背包、NPC AI、感知、社交、技艺、死亡周期 · 6 份 · 组均 57%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `merged` | **背包 / 物品 / 死亡掉落 / G 键拾取全链路** <br/><sub>`plan-inventory-v1.md`</sub> | `███████████░`  92% | #27 #42 | 2026-04-28 |
| `merged` | **NPC 行为 / 老化 / 派系 / 渡劫多 archetype** <br/><sub>`plan-npc-ai-v1.md`</sub> | `██████████░░`  85% | #22 #45 #75 | 2026-04-29 |
| `merged` | **NPC 假玩家皮肤池 / MineSkin 协议** <br/><sub>`plan-npc-skin-v1.md`</sub> | `██████████░░`  80% | #73 | 2026-04-28 |
| `wip` | **子技能成长（采药/炼丹/锻造）XP 与残卷** <br/><sub>`plan-skill-v1.md`</sub> | `█████████░░░`  75% | #25 #42 #68 | 2026-04-26 |
| `design` | **匿名 / 关系图 / 声名 / 灵龛 / 玩家派系挂靠** <br/><sub>`plan-social-v1.md`</sub> | `█░░░░░░░░░░░`   8% | — | 2026-04-28 |
| `design` | **视野 / 神识双感知（雾化 + 边缘指示器）** <br/><sub>`plan-perception-v1.md`</sub> | `░░░░░░░░░░░░`   3% | — | 2026-04-26 |

### 基础设施 / 工作流
_IPC schema、持久化、工作流、内容、音效 · 4 份 · 组均 75%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `merged` | **server !xxx 命令迁移至 Valence 原生 brigadier /xxx** <br/><sub>`plan-server-cmd-system-v1.md`</sub> | `███████████░`  90% | #72 | 2026-04-29 |
| `wip` | **server/agent 双侧跨重启持久化（SQLite WAL）** <br/><sub>`plan-persistence-v1.md`</sub> | `██████████░░`  80% | #24 | 2026-04-26 |
| `wip` | **library-web Astro 静态站 18 册馆藏内容** <br/><sub>`plan-library-web-content-v1.md`</sub> | `█████████░░░`  75% | #11 | 2026-04-25 |
| `wip` | **opencode + oh-my-opencode 全自动 plan 消费流水线** <br/><sub>`plan-opencode-workflow-v1.md`</sub> | `███████░░░░░`  55% | — | 2026-04-25 |

### 地形 / 世界生成
_末法残土 terrain profile、worldgen 流水线、CI 视觉快照 · 5 份 · 组均 6%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `skeleton` | **渊口荒丘 — TSY 主世界入口锚点的地表表达** <br/><sub>`plan-terrain-rift-mouth-v1.md`</sub> | `█░░░░░░░░░░░`  10% | — | 2026-04-28 |
| `skeleton` | **余烬死域 — qi=0 真死地** <br/><sub>`plan-terrain-ash-deadzone-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | — |
| `skeleton` | **九宗故地 — 上古七崩宗门废墟群** <br/><sub>`plan-terrain-jiuzong-ruin-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | — |
| `skeleton` | **伪灵脉绿洲 — 天道刻意陷阱** <br/><sub>`plan-terrain-pseudo-vein-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | — |
| `skeleton` | **烬焰焦土 — 长期天劫累积带** <br/><sub>`plan-terrain-tribulation-scorch-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | — |

### 骨架 plan
_战斗流派 + 快捷栏，等待开工 · 12 份 · 组均 5%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `skeleton` | **天道叙事内容侧** <br/><sub>`plan-narrative-v1.md`</sub> | `█░░░░░░░░░░░`  10% | — | — |
| `skeleton` | **炼丹回收** <br/><sub>`plan-alchemy-recycle-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | — |
| `skeleton` | **器修·暗器流** <br/><sub>`plan-anqi-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | — |
| `skeleton` | **毒蛊流** <br/><sub>`plan-dugu-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | — |
| `skeleton` | **灵田 NPC 行为扩展** <br/><sub>`plan-lingtian-npc-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | — |
| `skeleton` | **灵田流程深化** <br/><sub>`plan-lingtian-process-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | — |
| `skeleton` | **灵田天气与季节** <br/><sub>`plan-lingtian-weather-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | — |
| `skeleton` | **矿物 v2 扩展** <br/><sub>`plan-mineral-v2.md`</sub> | `█░░░░░░░░░░░`   5% | — | — |
| `skeleton` | **替尸·蜕壳流** <br/><sub>`plan-tuike-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | — |
| `skeleton` | **绝灵·涡流流** <br/><sub>`plan-woliu-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | — |
| `skeleton` | **地师·阵法流** <br/><sub>`plan-zhenfa-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | — |
| `skeleton` | **截脉·震爆流** <br/><sub>`plan-zhenmai-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | — |

### 已完成归档
_M0/M1 阶段产物 + 已 docs/finished_plans 的子 plan · 32 份 · 组均 100%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `done` | **MVP 0.1 — Server scaffold + NPC + Fabric Client** <br/><sub>`mvp01-plan.md`</sub> | `████████████` 100% | — | 2026-03-25 |
| `done` | **Agent 端到端集成与可观测** <br/><sub>`plan-agent-v2.md`</sub> | `████████████` 100% | — | 2026-04-13 |
| `done` | **天道 Agent 闭环（v1）** <br/><sub>`plan-agent.md`</sub> | `████████████` 100% | — | 2026-04-10 |
| `done` | **炼丹专项：配方/熔炉/火候三系统 + 服药丹毒** <br/><sub>`plan-alchemy-v1.md`</sub> | `████████████` 100% | #21 #28 | 2026-04-27 |
| `done` | **护甲减免系统：ArmorProfile + 耐久 + 体修 buff** <br/><sub>`plan-armor-v1.md`</sub> | `████████████` 100% | #46 #52 #56 | 2026-04-27 |
| `done` | **MC vanilla 音效 SoundRecipe 组合管线** <br/><sub>`plan-audio-v1.md`</sub> | `████████████` 100% | #74 | 2026-04-28 |
| `done` | **体修·爆脉流崩拳 P0（首个真实战斗功法闭环）** <br/><sub>`plan-baomai-v1.md`</sub> | `████████████` 100% | #76 | 2026-04-28 |
| `done` | **野生植物采集生态** <br/><sub>`plan-botany-v1.md`</sub> | `████████████` 100% | — | 2026-04-25 |
| `done` | **Client Mod 网络消息路由** <br/><sub>`plan-client.md`</sub> | `████████████` 100% | — | 2026-04-20 |
| `done` | **战斗系统服务端 ECS + IPC schema（无 UI）** <br/><sub>`plan-combat-no_ui.md`</sub> | `████████████` 100% | #29 #30 | 2026-04-21 |
| `done` | **Cultivation 双头清理：删旧 MVP 占位** <br/><sub>`plan-cultivation-mvp-cleanup-v1.md`</sub> | `████████████` 100% | #48 | 2026-04-27 |
| `done` | **修炼系统：六境/经脉/真元/污染/突破/顿悟** <br/><sub>`plan-cultivation-v1.md`</sub> | `████████████` 100% | #21 #26 #28 #29 #48 | 2026-04-27 |
| `done` | **死亡 / 运数 / 寿元 / 遗念 / 亡者博物馆** <br/><sub>`plan-death-lifecycle-v1.md`</sub> | `████████████` 100% | — | 2026-04-27 |
| `done` | **炼器（武器）专项：四步状态机 + IPC Schema + 客户端占位** <br/><sub>`plan-forge-v1.md`</sub> | `████████████` 100% | #19 #61 | 2026-04-28 |
| `done` | **Redis channel + TypeBox schema 双端对齐管理** <br/><sub>`plan-ipc-schema-v1.md`</sub> | `████████████` 100% | — | 2026-04-28 |
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
| `done` | **Worldgen 视觉快照 CI（5 角度真画面 + raster 双轨）** <br/><sub>`plan-worldgen-snapshot-v1.md`</sub> | `████████████` 100% | #71 | 2026-04-28 |
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
