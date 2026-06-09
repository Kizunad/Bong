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

_自动生成于 2026-06-08 · 共 133 份 plan_

```
总进度  █████████████████████████░░░░░  82.8%
```

**分布**：`merged` 59 · `wip` 1 · `design` 23 · `skeleton` 1 · `done` 49

### 战斗 / HUD / 视觉
_战斗 ECS、流派、HUD、粒子、动画、Iris · 15 份 · 组均 87%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `merged` | **HUD 设计专项闭环** <br/><sub>`finished_plans/plan-HUD-v1.md`</sub> | `████████████` 100% | #98 | 2026-06-08 |
| `merged` | **器修·暗器流** <br/><sub>`finished_plans/plan-anqi-v1.md`</sub> | `████████████` 100% | #121 | 2026-06-08 |
| `merged` | **突破 cinematic 三栈闭环** <br/><sub>`finished_plans/plan-breakthrough-cinematic-v1.md`</sub> | `████████████` 100% | #420 | 2026-06-08 |
| `merged` | **HUD 感知增强** <br/><sub>`finished_plans/plan-hud-immersion-v2.md`</sub> | `████████████` 100% | #203 | 2026-06-08 |
| `merged` | **G 键统一环境交互路由** <br/><sub>`finished_plans/plan-input-binding-v1.md`</sub> | `████████████` 100% | #101 | 2026-06-08 |
| `merged` | **状态效果 HUD 图标补全** <br/><sub>`finished_plans/plan-status-effect-icon-v1.md`</sub> | `████████████` 100% | #443 | 2026-06-08 |
| `merged` | **流派碰撞平衡** <br/><sub>`finished_plans/plan-style-balance-v1.md`</sub> | `████████████` 100% | #204 | 2026-06-08 |
| `merged` | **替尸蜕壳伪皮闭环** <br/><sub>`finished_plans/plan-tuike-v1.md`</sub> | `████████████` 100% | #124 | 2026-06-08 |
| `merged` | **绝灵涡流 v1** <br/><sub>`finished_plans/plan-woliu-v1.md`</sub> | `████████████` 100% | #113 #244 | 2026-06-08 |
| `merged` | **地师·阵法流 v1：诡雷与警戒场** <br/><sub>`finished_plans/plan-zhenfa-v1.md`</sub> | `████████████` 100% | #110 | 2026-06-08 |
| `merged` | **截脉震爆流 P1/P2** <br/><sub>`finished_plans/plan-zhenmai-v1.md`</sub> | `████████████` 100% | #122 | 2026-06-08 |
| `merged` | **毒蛊流：凝针与经脉永久损伤** <br/><sub>`finished_plans/plan-dugu-v1.md`</sub> | `███████████░`  95% | #126 | 2026-06-08 |
| `merged` | **真元色向量链路接入** <br/><sub>`finished_plans/plan-style-vector-integration-v1.md`</sub> | `███████████░`  95% | #123 #425 | 2026-06-08 |
| `design` | **剑修 v3：黑武士与化虚天门剑** <br/><sub>`plan-sword-path-v3.md`</sub> | `█░░░░░░░░░░░`  10% | — | 2026-06-08 |
| `design` | **Iris 光影集成** <br/><sub>`plan-iris-integration-v1.md`</sub> | `█░░░░░░░░░░░`   5% | #254 | 2026-06-08 |

### 修炼 / 经济
_六境修炼、天劫、炼丹/炼器、矿物、灵田、保质期 · 26 份 · 组均 81%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `merged` | **炼丹客户端闭环** <br/><sub>`finished_plans/plan-alchemy-client-v1.md`</sub> | `████████████` 100% | #93 | 2026-06-08 |
| `merged` | **炼丹废料反哺灵田** <br/><sub>`finished_plans/plan-alchemy-recycle-v1.md`</sub> | `████████████` 100% | #139 | 2026-06-08 |
| `merged` | **炼丹 v2：副作用与丹心识别** <br/><sub>`finished_plans/plan-alchemy-v2.md`</sub> | `████████████` 100% | #125 | 2026-06-08 |
| `merged` | **植物生态扩展** <br/><sub>`finished_plans/plan-botany-v2.md`</sub> | `████████████` 100% | #83 #128 | 2026-06-08 |
| `merged` | **修炼正典境界与经脉门槛对齐** <br/><sub>`finished_plans/plan-cultivation-canonical-align-v1.md`</sub> | `████████████` 100% | #99 | 2026-06-08 |
| `merged` | **骨币价格指数与经济叙事** <br/><sub>`finished_plans/plan-economy-v1.md`</sub> | `████████████` 100% | #171 #105 #162 | 2026-06-08 |
| `merged` | **妖兽掉落与封灵骨币** <br/><sub>`finished_plans/plan-fauna-v1.md`</sub> | `████████████` 100% | #105 | 2026-06-08 |
| `merged` | **收口炼器桥接与客户端交互** <br/><sub>`finished_plans/plan-forge-leftovers-v1.md`</sub> | `████████████` 100% | #66 #103 | 2026-06-08 |
| `merged` | **寿元系统精细实装** <br/><sub>`finished_plans/plan-lifespan-v1.md`</sub> | `████████████` 100% | #117 | 2026-06-08 |
| `merged` | **灵田作物二级加工** <br/><sub>`finished_plans/plan-lingtian-process-v1.md`</sub> | `████████████` 100% | #134 | 2026-06-08 |
| `merged` | **矿物材料正典** <br/><sub>`finished_plans/plan-mineral-v1.md`</sub> | `████████████` 100% | #44 #104 | 2026-06-08 |
| `merged` | **qi 物理迁移收口** <br/><sub>`finished_plans/plan-qi-physics-patch-v1.md`</sub> | `████████████` 100% | #133 #142 #152 #156 #160 #162 #165 | 2026-06-08 |
| `merged` | **真元物理守恒底盘** <br/><sub>`finished_plans/plan-qi-physics-v1.md`</sub> | `████████████` 100% | #132 | 2026-06-08 |
| `merged` | **灵眼系统链路** <br/><sub>`finished_plans/plan-spirit-eye-v1.md`</sub> | `████████████` 100% | #111 | 2026-06-08 |
| `merged` | **灵木采伐与封灵匣** <br/><sub>`finished_plans/plan-spiritwood-v1.md`</sub> | `████████████` 100% | #106 | 2026-06-08 |
| `merged` | **凡器工具体系** <br/><sub>`finished_plans/plan-tools-v1.md`</sub> | `████████████` 100% | #84 #86 | 2026-06-08 |
| `merged` | **渡虚劫 / 域崩 / 定向天罚** <br/><sub>`finished_plans/plan-tribulation-v1.md`</sub> | `████████████` 100% | #96 | 2026-06-08 |
| `merged` | **化虚专属 action** <br/><sub>`finished_plans/plan-void-actions-v1.md`</sub> | `████████████` 100% | #163 | 2026-06-08 |
| `merged` | **世界灵气预算化虚名额** <br/><sub>`finished_plans/plan-void-quota-v1.md`</sub> | `████████████` 100% | #159 | 2026-06-08 |
| `merged` | **灵田专项** <br/><sub>`finished_plans/plan-lingtian-v1.md`</sub> | `███████████░`  88% | #26 #127 #115 | 2026-06-08 |
| `merged` | **灵田季节天气消费层** <br/><sub>`finished_plans/plan-lingtian-weather-v1.md`</sub> | `██████████░░`  85% | #154 | 2026-06-08 |
| `design` | **半步化虚重渡跨层集成** <br/><sub>`plan-halfstep-rechallenge-integration-v1.md`</sub> | `█░░░░░░░░░░░`  10% | — | 2026-06-08 |
| `design` | **负灵域逃遁战术** <br/><sub>`plan-neg-domain-escape-v1.md`</sub> | `█░░░░░░░░░░░`  10% | — | 2026-06-08 |
| `design` | **灵气物品搬运磨损** <br/><sub>`plan-qi-handling-attrition-v1.md`</sub> | `█░░░░░░░░░░░`  10% | — | 2026-06-08 |
| `design` | **半步化虚 buff 强度运营校准** <br/><sub>`plan-halfstep-buff-calibration-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | 2026-06-08 |
| `design` | **渡虚劫平衡矩阵** <br/><sub>`plan-tribulation-balance-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | 2026-06-08 |

### 玩法 / NPC / 世界
_背包、NPC AI、感知、社交、技艺、死亡周期 · 24 份 · 组均 53%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `merged` | **Inventory v1** <br/><sub>`finished_plans/plan-inventory-v1.md`</sub> | `████████████` 100% | #27 | 2026-06-08 |
| `merged` | **背包 v2 堆叠与批量入包** <br/><sub>`finished_plans/plan-inventory-v2.md`</sub> | `████████████` 100% | #115 | 2026-06-08 |
| `merged` | **已亡七宗宗门志入库** <br/><sub>`finished_plans/plan-library-jiuzong-history-v1.md`</sub> | `████████████` 100% | #114 | 2026-06-08 |
| `merged` | **散修灵田与天道叙事** <br/><sub>`finished_plans/plan-lingtian-npc-v1.md`</sub> | `████████████` 100% | #137 | 2026-06-08 |
| `merged` | **多世人生与历代生平** <br/><sub>`finished_plans/plan-multi-life-v1.md`</sub> | `████████████` 100% | #148 | 2026-06-08 |
| `merged` | **混元多流派修炼路径** <br/><sub>`finished_plans/plan-multi-style-v1.md`</sub> | `████████████` 100% | #129 | 2026-06-08 |
| `merged` | **灵龛守家与龛侵追凶闭环** <br/><sub>`finished_plans/plan-niche-defense-v1.md`</sub> | `████████████` 100% | #130 | 2026-06-08 |
| `merged` | **视觉与神识感知系统** <br/><sub>`finished_plans/plan-perception-v1.1.md`</sub> | `████████████` 100% | #85 | 2026-06-08 |
| `merged` | **新手 POI 动态选址** <br/><sub>`finished_plans/plan-poi-novice-v1.md`</sub> | `████████████` 100% | #109 | 2026-06-08 |
| `merged` | **出生沉默引导** <br/><sub>`finished_plans/plan-spawn-tutorial-v1.md`</sub> | `████████████` 100% | — | 2026-06-08 |
| `merged` | **坍缩渊撤离压迫感** <br/><sub>`finished_plans/plan-tsy-raceout-v1.md`</sub> | `████████████` 100% | #151 | 2026-06-08 |
| `wip` | **100h 玩家旅程总线** <br/><sub>`plan-gameplay-journey-v1.md`</sub> | `████████░░░░`  65% | #159 | 2026-06-08 |
| `design` | **搜打撤风险节拍** <br/><sub>`plan-sou-da-che-v1.md`</sub> | `██░░░░░░░░░░`  15% | — | 2026-06-08 |
| `design` | **无墙领地影响力博弈** <br/><sub>`plan-territory-v1.md`</sub> | `█░░░░░░░░░░░`  12% | — | 2026-06-08 |
| `design` | **兽潮迁徙与负压灭杀** <br/><sub>`plan-beast-horde-v1.md`</sub> | `█░░░░░░░░░░░`  10% | — | 2026-06-08 |
| `design` | **垂死的大能遭遇** <br/><sub>`plan-dying-elder-v1.md`</sub> | `█░░░░░░░░░░░`  10% | — | 2026-06-08 |
| `design` | **玩家可参与的派系战争** <br/><sub>`plan-faction-wars-v1.md`</sub> | `█░░░░░░░░░░░`  10% | — | 2026-06-08 |
| `design` | **生平碑刻与临终遗念** <br/><sub>`plan-life-record-epitaph-v1.md`</sub> | `█░░░░░░░░░░░`  10% | — | 2026-06-08 |
| `design` | **Dormant NPC 批量战斗推演** <br/><sub>`plan-npc-virtualize-v3.md`</sub> | `█░░░░░░░░░░░`  10% | — | 2026-06-08 |
| `design` | **方块生命周期（破坏获取→入背包→放置）** <br/><sub>`plan-block-lifecycle-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | 2026-06-09 |
| `design` | **手搓消耗品空壳消杀（11 个补使用闭环）** <br/><sub>`plan-consumable-effects-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | 2026-06-10 |
| `design` | **具名散修势力扩展** <br/><sub>`plan-faction-expansion-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | 2026-06-08 |
| `design` | **NPC Drowsy 三态虚拟化** <br/><sub>`plan-npc-virtualize-v2.md`</sub> | `█░░░░░░░░░░░`   5% | — | 2026-06-08 |
| `design` | **战争结果与信誉系统联动** <br/><sub>`plan-social-v2.md`</sub> | `█░░░░░░░░░░░`   5% | — | 2026-06-08 |

### 基础设施 / 工作流
_IPC schema、持久化、工作流、内容、音效 · 13 份 · 组均 64%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `merged` | **反作弊计数与 Redis 上报** <br/><sub>`finished_plans/plan-anticheat-v1.md`</sub> | `████████████` 100% | #116 | 2026-06-08 |
| `merged` | **植物生态快照接入天道 agent** <br/><sub>`finished_plans/plan-botany-agent-v1.md`</sub> | `████████████` 100% | #136 | 2026-06-08 |
| `merged` | **客户端接线缺口收口** <br/><sub>`finished_plans/plan-client-wiring-gaps-v1.md`</sub> | `████████████` 100% | #236 | 2026-06-08 |
| `merged` | **跨系统接入缺口补丁** <br/><sub>`finished_plans/plan-cross-system-patch-v1.md`</sub> | `████████████` 100% | #92 | 2026-06-08 |
| `merged` | **Tripo3D 模型资产批产** <br/><sub>`finished_plans/plan-model-asset-v1.md`</sub> | `████████████` 100% | — | 2026-06-09 |
| `merged` | **持久化硬化** <br/><sub>`finished_plans/plan-persistence-v1.md`</sub> | `████████████` 100% | #24 | 2026-06-08 |
| `merged` | **服务端 Brigadier 命令迁移** <br/><sub>`finished_plans/plan-server-cmd-system-v1.md`</sub> | `████████████` 100% | #72 #90 | 2026-06-08 |
| `merged` | **天道叙事模板** <br/><sub>`finished_plans/plan-narrative-v1.md`</sub> | `███████████░`  90% | #89 | 2026-06-08 |
| `design` | **天道狩猎注意力系统** <br/><sub>`plan-tiandao-hunt-v1.md`</sub> | `██░░░░░░░░░░`  15% | — | 2026-06-08 |
| `design` | **UI-as-Data 动态交互面板** <br/><sub>`plan-agent-ui-data-v1.md`</sub> | `█░░░░░░░░░░░`  10% | — | 2026-06-08 |
| `design` | **多资源包构建与推送** <br/><sub>`plan-resourcepack-v1.md`</sub> | `█░░░░░░░░░░░`  10% | — | 2026-06-08 |
| `design` | **客户端登录与资源包 UX** <br/><sub>`plan-client-login-ux-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | 2026-06-08 |
| `skeleton` | **视频动捕到玩家动画工具链** <br/><sub>`plan-video2anim-v1.md`</sub> | `█░░░░░░░░░░░`   5% | #240 | 2026-06-08 |

### 地形 / 世界生成
_末法残土 terrain profile、worldgen 流水线、CI 视觉快照 · 6 份 · 组均 98%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `merged` | **余烬死域地形首版** <br/><sub>`finished_plans/plan-terrain-ash-deadzone-v1.md`</sub> | `████████████` 100% | #94 | 2026-06-08 |
| `merged` | **九宗故地 terrain profile** <br/><sub>`finished_plans/plan-terrain-jiuzong-ruin-v1.md`</sub> | `████████████` 100% | #118 #114 | 2026-06-08 |
| `merged` | **worldgen 多通道 layer 查询接口** <br/><sub>`finished_plans/plan-terrain-layer-query-v1.md`</sub> | `████████████` 100% | #167 | 2026-06-08 |
| `merged` | **烬焰焦土 profile 与渡劫遗痕** <br/><sub>`finished_plans/plan-terrain-tribulation-scorch-v1.md`</sub> | `████████████` 100% | #207 | 2026-06-08 |
| `merged` | **伪灵脉绿洲地形与生命周期** <br/><sub>`finished_plans/plan-terrain-pseudo-vein-v1.md`</sub> | `███████████░`  95% | #107 | 2026-06-08 |
| `merged` | **渊口荒丘地形** <br/><sub>`finished_plans/plan-terrain-rift-mouth-v1.md`</sub> | `███████████░`  95% | #119 | 2026-06-08 |

### 已完成归档
_M0/M1 阶段产物 + 已 docs/finished_plans 的子 plan · 49 份 · 组均 100%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `done` | **时代状态机** <br/><sub>`finished_plans/plan-era-state-v1.md`</sub> | `████████████` 100% | — | 2026-06-08 |
| `done` | **玩家全程旅途 deepseek 稿** <br/><sub>`finished_plans/plan-player-journey-deepseek.md`</sub> | `████████████` 100% | — | 2026-05-16 |
| `done` | **100h 游玩路程 gpt 稿** <br/><sub>`finished_plans/plan-playthrough-100h-gpt-v1.md`</sub> | `████████████` 100% | — | 2026-05-16 |
| `done` | **MVP 0.1 — Server scaffold + NPC + Fabric Client** <br/><sub>`mvp01-plan.md`</sub> | `████████████` 100% | — | 2026-03-25 |
| `done` | **Agent 端到端集成与可观测** <br/><sub>`plan-agent-v2.md`</sub> | `████████████` 100% | — | 2026-04-13 |
| `done` | **天道 Agent 闭环（v1）** <br/><sub>`plan-agent.md`</sub> | `████████████` 100% | — | 2026-04-10 |
| `done` | **炼丹专项：配方/熔炉/火候三系统 + 服药丹毒** <br/><sub>`plan-alchemy-v1.md`</sub> | `████████████` 100% | #21 #28 | 2026-04-27 |
| `done` | **护甲减免系统:ArmorProfile + 耐久 + 体修 buff** <br/><sub>`plan-armor-v1.md`</sub> | `████████████` 100% | #46 #52 #56 | 2026-04-27 |
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
| `done` | **通用保质期系统:三路径衰减/腐败/陈化 + 消费侧接入** <br/><sub>`plan-shelflife-v1.md`</sub> | `████████████` 100% | #32 #33 #34 #35 #36 #37 #38 #39 #40 #67 | 2026-04-27 |
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
