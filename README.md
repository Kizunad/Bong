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

_自动生成于 2026-05-04 · 共 103 份 plan_

```
总进度  ████████████████████████░░░░░░  79.6%
```

**分布**：`merged` 35 · `wip` 2 · `design` 11 · `skeleton` 9 · `done` 46

### 战斗 / HUD / 视觉
_战斗 ECS、流派、HUD、粒子、动画、Iris · 10 份 · 组均 80%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `merged` | **G 键统一环境交互路由** <br/><sub>`plan-input-binding-v1.md`</sub> | `████████████` 100% | #101 | 2026-04-29 |
| `merged` | **真元色向量链路（流派组合涌现）** <br/><sub>`plan-style-vector-integration-v1.md`</sub> | `████████████` 100% | #123 | 2026-05-04 |
| `merged` | **绝灵·涡流流** <br/><sub>`plan-woliu-v1.md`</sub> | `████████████` 100% | #113 | 2026-05-02 |
| `merged` | **截脉·震爆流** <br/><sub>`plan-zhenmai-v1.md`</sub> | `████████████` 100% | #122 | 2026-05-03 |
| `merged` | **HUD 设计专项** <br/><sub>`plan-HUD-v1.md`</sub> | `███████████░`  95% | #98 | 2026-04-29 |
| `merged` | **器修·暗器流** <br/><sub>`plan-anqi-v1.md`</sub> | `███████████░`  95% | #121 | 2026-05-04 |
| `merged` | **替尸·蜕壳流：伪灵皮 contam 过滤** <br/><sub>`plan-tuike-v1.md`</sub> | `███████████░`  95% | — | 2026-05-04 |
| `merged` | **地师阵法流** <br/><sub>`plan-zhenfa-v1.md`</sub> | `███████████░`  90% | #110 | 2026-05-01 |
| `wip` | **毒蛊流：凝针 + 灌毒蛊 + 经脉永久损伤** <br/><sub>`plan-dugu-v1.md`</sub> | `███░░░░░░░░░`  25% | — | 2026-05-03 |
| `design` | **Iris 光影集成** <br/><sub>`plan-iris-integration-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | 2026-04-26 |

### 修炼 / 经济
_六境修炼、天劫、炼丹/炼器、矿物、灵田、保质期 · 16 份 · 组均 74%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `merged` | **炼丹客户端闭环** <br/><sub>`plan-alchemy-client-v1.md`</sub> | `████████████` 100% | — | 2026-05-03 |
| `merged` | **修炼正典对齐：六境界与经脉门槛** <br/><sub>`plan-cultivation-canonical-align-v1.md`</sub> | `████████████` 100% | #99 | 2026-04-29 |
| `merged` | **妖兽骨系材料** <br/><sub>`plan-fauna-v1.md`</sub> | `████████████` 100% | #105 | 2026-05-03 |
| `merged` | **炼器剩余收口** <br/><sub>`plan-forge-leftovers-v1.md`</sub> | `████████████` 100% | #62 #66 #103 | 2026-05-03 |
| `merged` | **寿元系统：境界年限 + 死亡扣寿** <br/><sub>`plan-lifespan-v1.md`</sub> | `████████████` 100% | #117 | 2026-05-03 |
| `merged` | **矿物材料正典** <br/><sub>`plan-mineral-v1.md`</sub> | `████████████` 100% | #31 #44 #104 | 2026-05-03 |
| `merged` | **灵眼系统** <br/><sub>`plan-spirit-eye-v1.md`</sub> | `████████████` 100% | #111 | 2026-05-01 |
| `merged` | **灵木材料体系** <br/><sub>`plan-spiritwood-v1.md`</sub> | `████████████` 100% | #106 | 2026-05-03 |
| `merged` | **凡器工具体系** <br/><sub>`plan-tools-v1.md`</sub> | `████████████` 100% | #84 #86 | 2026-05-03 |
| `merged` | **灵田人工种植** <br/><sub>`plan-lingtian-v1.md`</sub> | `███████████░`  93% | #26 | 2026-05-01 |
| `merged` | **植物生态扩展** <br/><sub>`plan-botany-v2.md`</sub> | `███████████░`  90% | #83 #84 #86 #105 | 2026-05-03 |
| `merged` | **天劫系统** <br/><sub>`plan-tribulation-v1.md`</sub> | `███████████░`  90% | #96 | 2026-05-01 |
| `design` | **丹药副作用 / 残卷 / 品阶 / AutoProfile / 丹心识别** <br/><sub>`plan-alchemy-v2.md`</sub> | `█░░░░░░░░░░░`   8% | — | 2026-05-04 |
| `design` | **炼丹废料反哺灵田** <br/><sub>`plan-alchemy-recycle-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | 2026-04-27 |
| `design` | **作物二级加工（晾晒/碾粉/炮制/萃取）** <br/><sub>`plan-lingtian-process-v1.md`</sub> | `░░░░░░░░░░░░`   0% | — | 2026-04-29 |
| `design` | **灵田天气季节（夏冬汐转）** <br/><sub>`plan-lingtian-weather-v1.md`</sub> | `░░░░░░░░░░░░`   0% | — | 2026-04-29 |

### 玩法 / NPC / 世界
_背包、NPC AI、感知、社交、技艺、死亡周期 · 11 份 · 组均 58%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `merged` | **背包格子分配与堆叠修复** <br/><sub>`plan-inventory-v2.md`</sub> | `████████████` 100% | #115 | 2026-05-02 |
| `merged` | **视觉与神识感知系统 v1.1** <br/><sub>`plan-perception-v1.1.md`</sub> | `████████████` 100% | #85 | 2026-05-01 |
| `merged` | **新手 POI 动态选址 + 事件叙事** <br/><sub>`plan-poi-novice-v1.md`</sub> | `████████████` 100% | #109 | 2026-05-01 |
| `merged` | **30min 出生沉默引导** <br/><sub>`plan-spawn-tutorial-v1.md`</sub> | `████████████` 100% | — | 2026-05-01 |
| `merged` | **背包与掉落闭环** <br/><sub>`plan-inventory-v1.md`</sub> | `███████████░`  95% | #12 #27 | 2026-05-01 |
| `merged` | **已亡七宗宗门志 lore 入库** <br/><sub>`plan-library-jiuzong-history-v1.md`</sub> | `███████████░`  95% | #114 | 2026-05-04 |
| `design` | **7 色均衡 PracticeLog 自然演化混元色** <br/><sub>`plan-multi-style-v1.md`</sub> | `██░░░░░░░░░░`  15% | — | 2026-05-04 |
| `design` | **坍缩渊塌缩 3 秒 race-out 撤离** <br/><sub>`plan-tsy-raceout-v1.md`</sub> | `█░░░░░░░░░░░`  10% | — | 2026-05-04 |
| `design` | **多周目角色终结与知识继承** <br/><sub>`plan-multi-life-v1.md`</sub> | `█░░░░░░░░░░░`   8% | — | 2026-05-04 |
| `design` | **散修自主开荒种田生态** <br/><sub>`plan-lingtian-npc-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | 2026-04-30 |
| `design` | **灵龛多层守家体系（傀儡/陷阱/道伥）** <br/><sub>`plan-niche-defense-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | 2026-04-30 |

### 基础设施 / 工作流
_IPC schema、持久化、工作流、内容、音效 · 6 份 · 组均 86%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `merged` | **反作弊计数与 Redis 上报** <br/><sub>`plan-anticheat-v1.md`</sub> | `████████████` 100% | #116 | 2026-05-03 |
| `merged` | **Cross-system 接入面补丁** <br/><sub>`plan-cross-system-patch-v1.md`</sub> | `████████████` 100% | — | 2026-04-30 |
| `merged` | **天道叙事风格节奏** <br/><sub>`plan-narrative-v1.md`</sub> | `████████████` 100% | #89 | 2026-05-01 |
| `merged` | **SQLite/WAL 存档持久化** <br/><sub>`plan-persistence-v1.md`</sub> | `████████████` 100% | #24 | 2026-05-02 |
| `merged` | **服务端原生命令系统** <br/><sub>`plan-server-cmd-system-v1.md`</sub> | `████████████` 100% | #72 | 2026-05-01 |
| `wip` | **植物生态接入天道** <br/><sub>`plan-botany-agent-v1.md`</sub> | `██░░░░░░░░░░`  15% | — | 2026-04-27 |

### 地形 / 世界生成
_末法残土 terrain profile、worldgen 流水线、CI 视觉快照 · 6 份 · 组均 70%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `merged` | **余烬死域首版** <br/><sub>`plan-terrain-ash-deadzone-v1.md`</sub> | `████████████` 100% | #94 | 2026-05-01 |
| `merged` | **九宗故地废墟群** <br/><sub>`plan-terrain-jiuzong-ruin-v1.md`</sub> | `████████████` 100% | #118 | 2026-05-03 |
| `merged` | **伪灵脉绿洲** <br/><sub>`plan-terrain-pseudo-vein-v1.md`</sub> | `████████████` 100% | #107 | 2026-05-02 |
| `merged` | **渊口荒丘地表** <br/><sub>`plan-terrain-rift-mouth-v1.md`</sub> | `████████████` 100% | #119 | 2026-05-03 |
| `design` | **TerrainProvider 按层查询（typed API）** <br/><sub>`plan-terrain-layer-query-v1.md`</sub> | `█░░░░░░░░░░░`  10% | — | 2026-04-29 |
| `skeleton` | **烬焰焦土地形** <br/><sub>`plan-terrain-tribulation-scorch-v1.md`</sub> | `█░░░░░░░░░░░`  10% | — | 2026-04-29 |

### 骨架 plan
_玩家旅程 / 经济 / 化虚 / 流派平衡等待开工骨架 · 8 份 · 组均 5%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `skeleton` | **骨币半衰期 + 末法节律 + 100h 经济曲线** <br/><sub>`plan-economy-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | — |
| `skeleton` | **终极验收：6 段 E2E + 100h 实测** <br/><sub>`plan-gameplay-acceptance-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | — |
| `skeleton` | **玩家旅程总线（普通人 → 化虚 100h）** <br/><sub>`plan-gameplay-journey-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | 2026-05-04 |
| `skeleton` | **玩家全程旅途（deepseek 版）** <br/><sub>`plan-player-journey-deepseek.md`</sub> | `█░░░░░░░░░░░`   5% | — | — |
| `skeleton` | **100h 游玩路程（gpt 版）** <br/><sub>`plan-playthrough-100h-gpt-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | — |
| `skeleton` | **流派克制系数 config + telemetry 回填** <br/><sub>`plan-style-balance-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | — |
| `skeleton` | **化虚专属 action（镇压/引爆/阻挡/传承）** <br/><sub>`plan-void-actions-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | — |
| `skeleton` | **化虚名额按世界灵气总量动态调控** <br/><sub>`plan-void-quota-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | — |

### 已完成归档
_M0/M1 阶段产物 + 已 docs/finished_plans 的子 plan · 46 份 · 组均 100%_

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
| `done` | **战斗系统客户端 UI 全部组件实现（U1-U7 + 并行）** <br/><sub>`plan-combat-ui_impl.md`</sub> | `████████████` 100% | #20 | 2026-04-30 |
| `done` | **Cultivation 双头清理：删旧 MVP 占位** <br/><sub>`plan-cultivation-mvp-cleanup-v1.md`</sub> | `████████████` 100% | #48 | 2026-04-27 |
| `done` | **修炼系统：六境/经脉/真元/污染/突破/顿悟** <br/><sub>`plan-cultivation-v1.md`</sub> | `████████████` 100% | #21 #26 #28 #29 #48 | 2026-04-27 |
| `done` | **死亡 / 运数 / 寿元 / 遗念 / 亡者博物馆** <br/><sub>`plan-death-lifecycle-v1.md`</sub> | `████████████` 100% | — | 2026-04-27 |
| `done` | **炼器（武器）专项：四步状态机 + IPC Schema + 客户端占位** <br/><sub>`plan-forge-v1.md`</sub> | `████████████` 100% | #19 #61 | 2026-04-28 |
| `done` | **双行快捷栏：1-9 技能行 + F1-F9 物品行** <br/><sub>`plan-hotbar-modify-v1.md`</sub> | `████████████` 100% | #65 | 2026-04-29 |
| `done` | **Redis channel + TypeBox schema 双端对齐管理** <br/><sub>`plan-ipc-schema-v1.md`</sub> | `████████████` 100% | — | 2026-04-28 |
| `done` | **library-web 内容（已弃置）** <br/><sub>`plan-library-web-content-v1.md`</sub> | `████████████` 100% | — | 2026-05-03 |
| `done` | **矿物体系打磨 — UX/采矿/炉阶/配方/shelflife/resourcepack/化石** <br/><sub>`plan-mineral-v2.md`</sub> | `████████████` 100% | — | 2026-04-30 |
| `done` | **NPC 行为 / 老化 / 派系 / 渡劫多 archetype** <br/><sub>`plan-npc-ai-v1.md`</sub> | `████████████` 100% | #22 #45 #75 | 2026-04-29 |
| `done` | **NPC 假玩家皮肤池 / MineSkin 协议** <br/><sub>`plan-npc-skin-v1.md`</sub> | `████████████` 100% | #73 | 2026-04-28 |
| `done` | **opencode 命令工作流（已弃置）** <br/><sub>`plan-opencode-workflow-v1.md`</sub> | `████████████` 100% | — | 2026-05-03 |
| `done` | **粒子与世界 VFX 系统（三基类 + S2C 协议 + 首批资产）** <br/><sub>`plan-particle-system-v1.md`</sub> | `████████████` 100% | #17 | 2026-04-30 |
| `done` | **玩家骨骼动画系统（PlayerAnimator + AI-Native）** <br/><sub>`plan-player-animation-v1.md`</sub> | `████████████` 100% | #82 | 2026-04-29 |
| `done` | **Server 基础设施闭环** <br/><sub>`plan-server.md`</sub> | `████████████` 100% | — | 2026-04-21 |
| `done` | **通用保质期系统：三路径衰减/腐败/陈化 + 消费侧接入** <br/><sub>`plan-shelflife-v1.md`</sub> | `████████████` 100% | #32 #33 #34 #35 #36 #37 #38 #39 #40 #67 | 2026-04-27 |
| `done` | **子技能成长（采药/炼丹/锻造）XP 与残卷** <br/><sub>`plan-skill-v1.md`</sub> | `████████████` 100% | #25 #42 #68 | 2026-04-29 |
| `done` | **匿名社会 / 声名 / 灵龛 / 切磋 / 交易** <br/><sub>`plan-social-v1.md`</sub> | `████████████` 100% | #77 | 2026-04-29 |
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
| `done` | **武器 v1.1 补完：schema/channel/伤害/持久化/资源** <br/><sub>`plan-weapon-v1.1.md`</sub> | `████████████` 100% | #69 #80 | 2026-04-28 |
| `done` | **武器法宝完整链路（ItemInstance → Weapon Component → 3D 渲染）** <br/><sub>`plan-weapon-v1.md`</sub> | `████████████` 100% | #41 | 2026-04-30 |
| `done` | **Worldgen raster → Anvil region exporter** <br/><sub>`plan-worldgen-anvil-export-v1.md`</sub> | `████████████` 100% | #79 | 2026-04-30 |
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
