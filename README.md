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

_自动生成于 2026-07-27 · 共 170 份 plan_

```text
总进度  ███████████████████████████░░░  88.7%
```

**分布**：`merged` 139 · `wip` 16 · `design` 8 · `skeleton` 3 · `done` 4

### 坍缩渊（TSY）

_搜打撤独立位面玩法（10 子 plan，已全归档） · 1 份 · 组均 100%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `merged` | **垂死大能 TSY zone 未并入运行态** <br/><sub>`finished_plans/plan-dying-elder-tsy-zones-unloaded-v1.md`</sub> | `████████████` 100% | #898 #1140 | 2026-07-26 |

### 战斗 / HUD / 视觉

_战斗 ECS、流派、HUD、粒子、动画、Iris · 20 份 · 组均 84%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `merged` | **HUD 设计专项闭环** <br/><sub>`finished_plans/plan-HUD-v1.md`</sub> | `████████████` 100% | #98 | 2026-06-08 |
| `merged` | **器修·暗器流** <br/><sub>`finished_plans/plan-anqi-v1.md`</sub> | `████████████` 100% | #121 | 2026-06-08 |
| `merged` | **突破 cinematic 三栈闭环** <br/><sub>`finished_plans/plan-breakthrough-cinematic-v1.md`</sub> | `████████████` 100% | #420 | 2026-06-08 |
| `merged` | **HUD 感知增强** <br/><sub>`finished_plans/plan-hud-immersion-v2.md`</sub> | `████████████` 100% | #203 | 2026-06-08 |
| `merged` | **G 键统一环境交互路由** <br/><sub>`finished_plans/plan-input-binding-v1.md`</sub> | `████████████` 100% | #101 | 2026-06-08 |
| `merged` | **施放经脉门校验** <br/><sub>`finished_plans/plan-skill-cast-meridian-gate-v1.md`</sub> | `████████████` 100% | #609 #610 | 2026-06-29 |
| `merged` | **状态效果 HUD 图标补全** <br/><sub>`finished_plans/plan-status-effect-icon-v1.md`</sub> | `████████████` 100% | #443 | 2026-06-08 |
| `merged` | **流派碰撞平衡** <br/><sub>`finished_plans/plan-style-balance-v1.md`</sub> | `████████████` 100% | #204 | 2026-06-08 |
| `merged` | **黑无室剑道 v2** <br/><sub>`finished_plans/plan-sword-path-v2.md`</sub> | `████████████` 100% | #429 | 2026-06-07 |
| `merged` | **黑无室剑道 v3** <br/><sub>`finished_plans/plan-sword-path-v3.md`</sub> | `████████████` 100% | #441 | 2026-06-08 |
| `merged` | **替尸蜕壳伪皮闭环** <br/><sub>`finished_plans/plan-tuike-v1.md`</sub> | `████████████` 100% | #124 | 2026-06-08 |
| `merged` | **绝灵涡流 v1** <br/><sub>`finished_plans/plan-woliu-v1.md`</sub> | `████████████` 100% | #113 #244 | 2026-06-08 |
| `merged` | **地师·阵法流 v1：诡雷与警戒场** <br/><sub>`finished_plans/plan-zhenfa-v1.md`</sub> | `████████████` 100% | #110 | 2026-06-08 |
| `merged` | **截脉震爆流 P1/P2** <br/><sub>`finished_plans/plan-zhenmai-v1.md`</sub> | `████████████` 100% | #122 | 2026-06-08 |
| `merged` | **毒蛊流：凝针与经脉永久损伤** <br/><sub>`finished_plans/plan-dugu-v1.md`</sub> | `███████████░`  95% | #126 | 2026-06-08 |
| `merged` | **真元色向量链路接入** <br/><sub>`finished_plans/plan-style-vector-integration-v1.md`</sub> | `███████████░`  95% | #123 #425 | 2026-06-08 |
| `wip` | **FPV 手臂动画+施法 juice+签名音效** <br/><sub>`plan-fpv-cast-av-v1.md`</sub> | `███████░░░░░`  55% | #1248 #1257 #1258 #1262 | 2026-07-25 |
| `wip` | **区域浓雾：浓雾档+动态雾堤+天道下雾** <br/><sub>`plan-dense-fog-v1.md`</sub> | `██░░░░░░░░░░`  18% | #1156 #1158 | 2026-07-11 |
| `wip` | **Iris 光影集成——修仙状态驱动 shader** <br/><sub>`plan-iris-integration-v1.md`</sub> | `██░░░░░░░░░░`  15% | #254 | 2026-05-17 |
| `design` | **全力一击蓄力 HUD 断线跨会话残留** <br/><sub>`plan-bughunt-full-power-charging-session-bleed-v1.md`</sub> | `░░░░░░░░░░░░`   0% | #1094 | 2026-07-09 |

### 修炼 / 经济

_六境修炼、天劫、炼丹/炼器、矿物、灵田、保质期 · 28 份 · 组均 92%_

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
| `merged` | **半步化虚重渡跨层集成** <br/><sub>`finished_plans/plan-halfstep-rechallenge-integration-v1.md`</sub> | `████████████` 100% | — | 2026-06 |
| `merged` | **寿元系统精细实装** <br/><sub>`finished_plans/plan-lifespan-v1.md`</sub> | `████████████` 100% | #117 | 2026-06-08 |
| `merged` | **灵田作物二级加工** <br/><sub>`finished_plans/plan-lingtian-process-v1.md`</sub> | `████████████` 100% | #134 | 2026-06-08 |
| `merged` | **矿物材料正典** <br/><sub>`finished_plans/plan-mineral-v1.md`</sub> | `████████████` 100% | #44 #104 | 2026-06-08 |
| `merged` | **负灵域逃遁战术** <br/><sub>`finished_plans/plan-neg-domain-escape-v1.md`</sub> | `████████████` 100% | — | 2026-06 |
| `merged` | **灵气物品搬运磨损** <br/><sub>`finished_plans/plan-qi-handling-attrition-v1.md`</sub> | `████████████` 100% | — | 2026-06 |
| `merged` | **qi 物理迁移收口** <br/><sub>`finished_plans/plan-qi-physics-patch-v1.md`</sub> | `████████████` 100% | #133 #142 #152 #156 #160 #162 #165 | 2026-06-08 |
| `merged` | **真元物理守恒底盘** <br/><sub>`finished_plans/plan-qi-physics-v1.md`</sub> | `████████████` 100% | #132 | 2026-06-08 |
| `merged` | **灵眼系统链路** <br/><sub>`finished_plans/plan-spirit-eye-v1.md`</sub> | `████████████` 100% | #111 | 2026-06-08 |
| `merged` | **灵木采伐与封灵匣** <br/><sub>`finished_plans/plan-spiritwood-v1.md`</sub> | `████████████` 100% | #106 | 2026-06-08 |
| `merged` | **凡器工具体系** <br/><sub>`finished_plans/plan-tools-v1.md`</sub> | `████████████` 100% | #84 #86 | 2026-06-08 |
| `merged` | **并发渡劫 broadcast 单槽覆盖误清空** <br/><sub>`finished_plans/plan-tribulation-concurrent-broadcast-clobber-v1.md`</sub> | `████████████` 100% | #968 | 2026-07-26 |
| `merged` | **渡虚劫 / 域崩 / 定向天罚** <br/><sub>`finished_plans/plan-tribulation-v1.md`</sub> | `████████████` 100% | #96 | 2026-06-08 |
| `merged` | **化虚专属 action** <br/><sub>`finished_plans/plan-void-actions-v1.md`</sub> | `████████████` 100% | #163 | 2026-06-08 |
| `merged` | **世界灵气预算化虚名额** <br/><sub>`finished_plans/plan-void-quota-v1.md`</sub> | `████████████` 100% | #159 | 2026-06-08 |
| `merged` | **灵田专项** <br/><sub>`finished_plans/plan-lingtian-v1.md`</sub> | `███████████░`  88% | #26 #127 #115 | 2026-06-08 |
| `merged` | **灵田季节天气消费层** <br/><sub>`finished_plans/plan-lingtian-weather-v1.md`</sub> | `██████████░░`  85% | #154 | 2026-06-08 |
| `wip` | **种族 BodyPlan 通用化+固元易形功法** <br/><sub>`plan-race-system-v1.md`</sub> | `███████░░░░░`  60% | #1160 #1180 #1184 #1198 #1201 #1202 #1203 #1204 #1206 #1250 | 2026-07-27 |
| `wip` | **渡虚劫系统性平衡矩阵校准** <br/><sub>`plan-tribulation-balance-v1.md`</sub> | `██████░░░░░░`  50% | #533 #560 | 2026-06-14 |
| `design` | **半步化虚 buff 运营数值校准** <br/><sub>`plan-halfstep-buff-calibration-v1.md`</sub> | `░░░░░░░░░░░░`   0% | — | 2026-06-08 |

### 玩法 / NPC / 世界

_背包、NPC AI、感知、社交、技艺、死亡周期 · 35 份 · 组均 82%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `merged` | **灰烬蛛伪装态名牌泄漏** <br/><sub>`finished_plans/plan-ash-spider-disguise-nametag-leak-v1.md`</sub> | `████████████` 100% | #912 | 2026-07-26 |
| `merged` | **方块生命周期（破坏获取→入背包→放置）** <br/><sub>`finished_plans/plan-block-lifecycle-v1.md`</sub> | `████████████` 100% | — | 2026-06 |
| `merged` | **botany 采集模式错接旧 gather 链路** <br/><sub>`finished_plans/plan-botany-harvest-mode-request-misroute-v1.md`</sub> | `████████████` 100% | #897 | 2026-07-26 |
| `merged` | **满包退款吞材料修复** <br/><sub>`finished_plans/plan-bughunt-craft-refund-full-inventory-loss-v1.md`</sub> | `████████████` 100% | #1039 #1142 #1232 | 2026-07-27 |
| `merged` | **手搓消耗品空壳消杀（11 个产出物补齐使用闭环）** <br/><sub>`finished_plans/plan-consumable-effects-v1.md`</sub> | `████████████` 100% | #483 | 2026-06-10 |
| `merged` | **垂死的大能遭遇** <br/><sub>`finished_plans/plan-dying-elder-v1.md`</sub> | `████████████` 100% | — | 2026-06 |
| `merged` | **具名散修势力扩展** <br/><sub>`finished_plans/plan-faction-expansion-v1.md`</sub> | `████████████` 100% | #504 #508 #568 | 2026-06-29 |
| `merged` | **玩家可参与派系战争** <br/><sub>`finished_plans/plan-faction-wars-v1.md`</sub> | `████████████` 100% | — | 2026-06-29 |
| `merged` | **G 键 TSY 搜刮候选截胡其他交互** <br/><sub>`finished_plans/plan-g-interact-search-nearest-hijack-v1.md`</sub> | `████████████` 100% | #895 | 2026-07-26 |
| `merged` | **Inventory v1** <br/><sub>`finished_plans/plan-inventory-v1.md`</sub> | `████████████` 100% | #27 | 2026-06-08 |
| `merged` | **背包 v2 堆叠与批量入包** <br/><sub>`finished_plans/plan-inventory-v2.md`</sub> | `████████████` 100% | #115 | 2026-06-08 |
| `merged` | **已亡七宗宗门志入库** <br/><sub>`finished_plans/plan-library-jiuzong-history-v1.md`</sub> | `████████████` 100% | #114 | 2026-06-08 |
| `merged` | **散修灵田与天道叙事** <br/><sub>`finished_plans/plan-lingtian-npc-v1.md`</sub> | `████████████` 100% | #137 | 2026-06-08 |
| `merged` | **多世人生与历代生平** <br/><sub>`finished_plans/plan-multi-life-v1.md`</sub> | `████████████` 100% | #148 | 2026-06-08 |
| `merged` | **混元多流派修炼路径** <br/><sub>`finished_plans/plan-multi-style-v1.md`</sub> | `████████████` 100% | #129 | 2026-06-08 |
| `merged` | **灵龛守家与龛侵追凶闭环** <br/><sub>`finished_plans/plan-niche-defense-v1.md`</sub> | `████████████` 100% | #130 | 2026-06-08 |
| `merged` | **NPC 交易 bundle 成交少发货** <br/><sub>`finished_plans/plan-npc-trade-bundle-count-loss-v1.md`</sub> | `████████████` 100% | #1164 | 2026-07-26 |
| `merged` | **NPC Drowsy 三态虚拟化** <br/><sub>`finished_plans/plan-npc-virtualize-v2.md`</sub> | `████████████` 100% | — | 2026-06-29 |
| `merged` | **Dormant 批量战斗推演** <br/><sub>`finished_plans/plan-npc-virtualize-v3.md`</sub> | `████████████` 100% | — | 2026-06-29 |
| `merged` | **视觉与神识感知系统** <br/><sub>`finished_plans/plan-perception-v1.1.md`</sub> | `████████████` 100% | #85 | 2026-06-08 |
| `merged` | **新手 POI 动态选址** <br/><sub>`finished_plans/plan-poi-novice-v1.md`</sub> | `████████████` 100% | #109 | 2026-06-08 |
| `merged` | **SocialRenown 名声未回写 PlayerIdentities** <br/><sub>`finished_plans/plan-social-renown-identity-bridge-v1.md`</sub> | `████████████` 100% | #893 | 2026-07-26 |
| `merged` | **出生沉默引导** <br/><sub>`finished_plans/plan-spawn-tutorial-v1.md`</sub> | `████████████` 100% | — | 2026-06-08 |
| `merged` | **无墙领地影响力博弈** <br/><sub>`finished_plans/plan-territory-v1.md`</sub> | `████████████` 100% | — | 2026-06 |
| `merged` | **坍缩渊撤离压迫感** <br/><sub>`finished_plans/plan-tsy-raceout-v1.md`</sub> | `████████████` 100% | #151 | 2026-06-08 |
| `merged` | **zhenfa 陷阱阵旗手槽装备门缺口** <br/><sub>`finished_plans/plan-zhenfa-trap-client-equip-gate-v1.md`</sub> | `████████████` 100% | #861 #962 | 2026-07-26 |
| `wip` | **搜打撤循环风险节拍与情感曲线** <br/><sub>`plan-sou-da-che-v1.md`</sub> | `██████████░░`  80% | #509 #536 #540 #555 #556 #563 | 2026-06-16 |
| `design` | **普通人→化虚 100h 主线总线** <br/><sub>`plan-gameplay-journey-v1.md`</sub> | `███████░░░░░`  55% | — | 2026-06-09 |
| `wip` | **兽潮大迁徙：Flow Field 批量野兽迁移** <br/><sub>`plan-beast-horde-v1.md`</sub> | `██████░░░░░░`  50% | #535 #542 | 2026-06-13 |
| `wip` | **死脉甲污染豁免接线 + 守恒修正** <br/><sub>`plan-dead-armor-contamination-wiring-v1.md`</sub> | `██████░░░░░░`  50% | #581 | 2026-06-16 |
| `wip` | **一生记录·遗念碑刻** <br/><sub>`plan-life-record-epitaph-v1.md`</sub> | `███░░░░░░░░░`  25% | #538 | 2026-06-13 |
| `wip` | **容器品类筛选 + 12 僵尸容器补全** <br/><sub>`plan-container-filter-and-completion-v1.md`</sub> | `██░░░░░░░░░░`  20% | #526 | 2026-06-13 |
| `design` | **给丹 C2S 缺距离/维度权威校验** <br/><sub>`plan-bughunt-dying-elder-give-dan-server-gate-v1.md`</sub> | `░░░░░░░░░░░░`   0% | #1114 | 2026-07-09 |
| `design` | **塔科夫式套包（物品内嵌子容器）** <br/><sub>`plan-nested-pack-base-v1.md`</sub> | `░░░░░░░░░░░░`   0% | — | 2026-06-10 |
| `design` | **派系战争结果接入信誉传播链** <br/><sub>`plan-social-v2.md`</sub> | `░░░░░░░░░░░░`   0% | — | 2026-06-08 |

### 基础设施 / 工作流

_IPC schema、持久化、工作流、内容、音效 · 24 份 · 组均 84%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `merged` | **UI-as-Data 动态交互面板** <br/><sub>`finished_plans/plan-agent-ui-data-v1.md`</sub> | `████████████` 100% | — | 2026-06 |
| `merged` | **反作弊计数与 Redis 上报** <br/><sub>`finished_plans/plan-anticheat-v1.md`</sub> | `████████████` 100% | #116 | 2026-06-08 |
| `merged` | **植物生态快照接入天道 agent** <br/><sub>`finished_plans/plan-botany-agent-v1.md`</sub> | `████████████` 100% | #136 | 2026-06-08 |
| `merged` | **自检 round3 findings** <br/><sub>`finished_plans/plan-bughunt-r3-findings-v1.md`</sub> | `████████████` 100% | #589 #612 #616 | 2026-06-29 |
| `merged` | **自检 round4 findings** <br/><sub>`finished_plans/plan-bughunt-r4-findings-v1.md`</sub> | `████████████` 100% | #588 #601 #607 | 2026-06-29 |
| `merged` | **自检 round5 findings** <br/><sub>`finished_plans/plan-bughunt-r5-findings-v1.md`</sub> | `████████████` 100% | #603 #594 #614 | 2026-06-29 |
| `merged` | **客户端接线缺口收口** <br/><sub>`finished_plans/plan-client-wiring-gaps-v1.md`</sub> | `████████████` 100% | #236 | 2026-06-08 |
| `merged` | **跨系统接入缺口补丁** <br/><sub>`finished_plans/plan-cross-system-patch-v1.md`</sub> | `████████████` 100% | #92 | 2026-06-08 |
| `merged` | **Tripo3D 模型资产批产** <br/><sub>`finished_plans/plan-model-asset-v1.md`</sub> | `████████████` 100% | — | 2026-06-09 |
| `merged` | **持久化硬化** <br/><sub>`finished_plans/plan-persistence-v1.md`</sub> | `████████████` 100% | #24 | 2026-06-08 |
| `merged` | **preview 强关屏幕与过渡层互踩卡屏** <br/><sub>`finished_plans/plan-preview-pause-menu-transition-stall-v1.md`</sub> | `████████████` 100% | #903 | 2026-07-26 |
| `merged` | **多资源包构建与推送** <br/><sub>`finished_plans/plan-resourcepack-v1.md`</sub> | `████████████` 100% | — | 2026-06 |
| `merged` | **季节跨相位客户端状态陈旧** <br/><sub>`finished_plans/plan-season-phase-stale-client-v1.md`</sub> | `████████████` 100% | #885 | 2026-07-26 |
| `merged` | **服务端 Brigadier 命令迁移** <br/><sub>`finished_plans/plan-server-cmd-system-v1.md`</sub> | `████████████` 100% | #72 #90 | 2026-06-08 |
| `merged` | **天道狩猎注意力系统** <br/><sub>`finished_plans/plan-tiandao-hunt-v1.md`</sub> | `████████████` 100% | — | 2026-06 |
| `merged` | **视频动捕到玩家动画工具链** <br/><sub>`finished_plans/plan-video2anim-v1.md`</sub> | `████████████` 100% | #240 | 2026-06 |
| `merged` | **天道叙事模板** <br/><sub>`finished_plans/plan-narrative-v1.md`</sub> | `███████████░`  90% | #89 | 2026-06-08 |
| `wip` | **r2 跨端/境界门控/守恒 bug 修复** <br/><sub>`plan-bughunt-r2-findings-v1.md`</sub> | `███████████░`  90% | #605 #611 #617 #593 #602 #604 #597 | 2026-06-18 |
| `wip` | **r1 机械型 bug 批量修复** <br/><sub>`plan-bughunt-r1-mechanical-fixes-v1.md`</sub> | `██████████░░`  85% | #599 #702 #595 #576 #590 #585 | 2026-06-25 |
| `wip` | **CI Redis 镜像拉取韧性** <br/><sub>`plan-ci-redis-pull-resilience-v1.md`</sub> | `████████░░░░`  65% | #575 | 2026-07-27 |
| `wip` | **r7 顿悟 modifier/UI 生命周期 bug** <br/><sub>`plan-bughunt-r7-findings-v1.md`</sub> | `█████░░░░░░░`  40% | #709 #708 #707 #711 | 2026-06-25 |
| `wip` | **r8 派生属性孤岛审计** <br/><sub>`plan-bughunt-r8-modifier-orphan-audit-v1.md`</sub> | `████░░░░░░░░`  33% | #1143 | 2026-07-09 |
| `wip` | **r6 炼丹/装备 registry/渡劫 bug** <br/><sub>`plan-bughunt-r6-findings-v1.md`</sub> | `███░░░░░░░░░`  25% | #1068 | 2026-07-07 |
| `design` | **登录/资源包下载体验主题化** <br/><sub>`plan-client-login-ux-v1.md`</sub> | `░░░░░░░░░░░░`   0% | — | 2026-06-08 |

### 地形 / 世界生成

_末法残土 terrain profile、worldgen 流水线、CI 视觉快照 · 8 份 · 组均 86%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `merged` | **余烬死域地形首版** <br/><sub>`finished_plans/plan-terrain-ash-deadzone-v1.md`</sub> | `████████████` 100% | #94 | 2026-06-08 |
| `merged` | **九宗故地 terrain profile** <br/><sub>`finished_plans/plan-terrain-jiuzong-ruin-v1.md`</sub> | `████████████` 100% | #118 #114 | 2026-06-08 |
| `merged` | **worldgen 多通道 layer 查询接口** <br/><sub>`finished_plans/plan-terrain-layer-query-v1.md`</sub> | `████████████` 100% | #167 | 2026-06-08 |
| `merged` | **烬焰焦土 profile 与渡劫遗痕** <br/><sub>`finished_plans/plan-terrain-tribulation-scorch-v1.md`</sub> | `████████████` 100% | #207 | 2026-06-08 |
| `merged` | **worldgen qi_density 断言修复** <br/><sub>`finished_plans/plan-worldgen-raster-check-qidensity-fix-v1.md`</sub> | `████████████` 100% | #1047 | 2026-07-26 |
| `merged` | **伪灵脉绿洲地形与生命周期** <br/><sub>`finished_plans/plan-terrain-pseudo-vein-v1.md`</sub> | `███████████░`  95% | #107 | 2026-06-08 |
| `merged` | **渊口荒丘地形** <br/><sub>`finished_plans/plan-terrain-rift-mouth-v1.md`</sub> | `███████████░`  95% | #119 | 2026-06-08 |
| `design` | **静态 raster 异常热点无 runtime consumer** <br/><sub>`plan-bughunt-anomaly-raster-runtime-consumer-v1.md`</sub> | `░░░░░░░░░░░░`   0% | #1126 | 2026-07-09 |

### 骨架 plan

_玩家旅程 / 经济 / 化虚等待开工骨架 · 4 份 · 组均 29%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `merged` | **库存提示面板骨架** <br/><sub>`finished_plans/plan-inventory-hint-panel-v1.md`</sub> | `████████████` 100% | — | 2026-06 |
| `skeleton` | **自检 round10 骨架** <br/><sub>`plans-skeleton/plan-bughunt-r10-findings-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | 2026-06 |
| `skeleton` | **自检 round8 骨架** <br/><sub>`plans-skeleton/plan-bughunt-r8-findings-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | 2026-06 |
| `skeleton` | **自检 round9 骨架** <br/><sub>`plans-skeleton/plan-bughunt-r9-findings-v1.md`</sub> | `█░░░░░░░░░░░`   5% | — | 2026-06 |

### 已完成归档

_M0/M1 阶段产物 + 已 docs/finished_plans 的子 plan · 50 份 · 组均 100%_

| 状态 | Plan | 进度 | PR | 最近更新 |
|---|---|---|---|---|
| `merged` | **MVP 0.1 — Server scaffold + NPC + Fabric Client** <br/><sub>`finished_plans/mvp01-plan.md`</sub> | `████████████` 100% | — | 2026-03-25 |
| `merged` | **Agent 端到端集成与可观测** <br/><sub>`finished_plans/plan-agent-v2.md`</sub> | `████████████` 100% | — | 2026-04-13 |
| `merged` | **天道 Agent 闭环（v1）** <br/><sub>`finished_plans/plan-agent.md`</sub> | `████████████` 100% | — | 2026-04-10 |
| `merged` | **炼丹专项：配方/熔炉/火候三系统 + 服药丹毒** <br/><sub>`finished_plans/plan-alchemy-v1.md`</sub> | `████████████` 100% | #21 #28 | 2026-04-27 |
| `merged` | **护甲减免系统:ArmorProfile + 耐久 + 体修 buff** <br/><sub>`finished_plans/plan-armor-v1.md`</sub> | `████████████` 100% | #46 #52 #56 | 2026-04-27 |
| `merged` | **MC vanilla 音效 SoundRecipe 组合管线** <br/><sub>`finished_plans/plan-audio-v1.md`</sub> | `████████████` 100% | #74 | 2026-04-28 |
| `merged` | **体修·爆脉流崩拳 P0（首个真实战斗功法闭环）** <br/><sub>`finished_plans/plan-baomai-v1.md`</sub> | `████████████` 100% | #76 | 2026-04-28 |
| `merged` | **野生植物采集生态** <br/><sub>`finished_plans/plan-botany-v1.md`</sub> | `████████████` 100% | — | 2026-04-25 |
| `merged` | **Client Mod 网络消息路由** <br/><sub>`finished_plans/plan-client.md`</sub> | `████████████` 100% | — | 2026-04-20 |
| `merged` | **战斗系统服务端 ECS + IPC schema（无 UI）** <br/><sub>`finished_plans/plan-combat-no_ui.md`</sub> | `████████████` 100% | #29 #30 | 2026-04-21 |
| `merged` | **战斗系统客户端 UI 全部组件实现（U1-U7 + 并行）** <br/><sub>`finished_plans/plan-combat-ui_impl.md`</sub> | `████████████` 100% | #20 | 2026-04-30 |
| `merged` | **Cultivation 双头清理：删旧 MVP 占位** <br/><sub>`finished_plans/plan-cultivation-mvp-cleanup-v1.md`</sub> | `████████████` 100% | #48 | 2026-04-27 |
| `merged` | **修炼系统：六境/经脉/真元/污染/突破/顿悟** <br/><sub>`finished_plans/plan-cultivation-v1.md`</sub> | `████████████` 100% | #21 #26 #28 #29 #48 | 2026-04-27 |
| `merged` | **死亡 / 运数 / 寿元 / 遗念 / 亡者博物馆** <br/><sub>`finished_plans/plan-death-lifecycle-v1.md`</sub> | `████████████` 100% | — | 2026-04-27 |
| `done` | **时代状态机** <br/><sub>`finished_plans/plan-era-state-v1.md`</sub> | `████████████` 100% | — | 2026-06-08 |
| `merged` | **炼器（武器）专项：四步状态机 + IPC Schema + 客户端占位** <br/><sub>`finished_plans/plan-forge-v1.md`</sub> | `████████████` 100% | #19 #61 | 2026-04-28 |
| `done` | **草药捆保鲜挂载 + 草镰采集接通** <br/><sub>`finished_plans/plan-gathering-tool-bind-v1.md`</sub> | `████████████` 100% | #1293 | 2026-07-27 |
| `merged` | **双行快捷栏：1-9 技能行 + F1-F9 物品行** <br/><sub>`finished_plans/plan-hotbar-modify-v1.md`</sub> | `████████████` 100% | #65 | 2026-04-29 |
| `merged` | **Redis channel + TypeBox schema 双端对齐管理** <br/><sub>`finished_plans/plan-ipc-schema-v1.md`</sub> | `████████████` 100% | — | 2026-04-28 |
| `merged` | **library-web 内容（已弃置）** <br/><sub>`finished_plans/plan-library-web-content-v1.md`</sub> | `████████████` 100% | — | 2026-05-03 |
| `merged` | **矿物体系打磨 — UX/采矿/炉阶/配方/shelflife/resourcepack/化石** <br/><sub>`finished_plans/plan-mineral-v2.md`</sub> | `████████████` 100% | — | 2026-04-30 |
| `merged` | **NPC 行为 / 老化 / 派系 / 渡劫多 archetype** <br/><sub>`finished_plans/plan-npc-ai-v1.md`</sub> | `████████████` 100% | #22 #45 #75 | 2026-04-29 |
| `merged` | **NPC 假玩家皮肤池 / MineSkin 协议** <br/><sub>`finished_plans/plan-npc-skin-v1.md`</sub> | `████████████` 100% | #73 | 2026-04-28 |
| `merged` | **opencode 命令工作流（已弃置）** <br/><sub>`finished_plans/plan-opencode-workflow-v1.md`</sub> | `████████████` 100% | — | 2026-05-03 |
| `merged` | **粒子与世界 VFX 系统（三基类 + S2C 协议 + 首批资产）** <br/><sub>`finished_plans/plan-particle-system-v1.md`</sub> | `████████████` 100% | #17 | 2026-04-30 |
| `merged` | **玩家骨骼动画系统（PlayerAnimator + AI-Native）** <br/><sub>`finished_plans/plan-player-animation-v1.md`</sub> | `████████████` 100% | #82 | 2026-04-29 |
| `done` | **玩家全程旅途 deepseek 稿** <br/><sub>`finished_plans/plan-player-journey-deepseek.md`</sub> | `████████████` 100% | — | 2026-05-16 |
| `done` | **100h 游玩路程 gpt 稿** <br/><sub>`finished_plans/plan-playthrough-100h-gpt-v1.md`</sub> | `████████████` 100% | — | 2026-05-16 |
| `merged` | **Server 基础设施闭环** <br/><sub>`finished_plans/plan-server.md`</sub> | `████████████` 100% | — | 2026-04-21 |
| `merged` | **通用保质期系统:三路径衰减/腐败/陈化 + 消费侧接入** <br/><sub>`finished_plans/plan-shelflife-v1.md`</sub> | `████████████` 100% | #32 #33 #34 #35 #36 #37 #38 #39 #40 #67 | 2026-04-27 |
| `merged` | **子技能成长（采药/炼丹/锻造）XP 与残卷** <br/><sub>`finished_plans/plan-skill-v1.md`</sub> | `████████████` 100% | #25 #42 #68 | 2026-04-29 |
| `merged` | **匿名社会 / 声名 / 灵龛 / 切磋 / 交易** <br/><sub>`finished_plans/plan-social-v1.md`</sub> | `████████████` 100% | #77 | 2026-04-29 |
| `merged` | **TSY 容器搜刮系统（5 档 + 钥匙 + 真元加速）** <br/><sub>`finished_plans/plan-tsy-container-v1.md`</sub> | `████████████` 100% | #55 | 2026-04-27 |
| `merged` | **TSY 位面基础设施** <br/><sub>`finished_plans/plan-tsy-dimension-v1.md`</sub> | `████████████` 100% | #47 | 2026-04-26 |
| `merged` | **TSY 撤离点（RiftPortal + 撤离倒计时 + race-out）** <br/><sub>`finished_plans/plan-tsy-extract-v1.md`</sub> | `████████████` 100% | #59 | 2026-04-27 |
| `merged` | **TSY 敌对 NPC 四档（道伥/执念/守灵/畸变体）** <br/><sub>`finished_plans/plan-tsy-hostile-v1.md`</sub> | `████████████` 100% | — | 2026-04-27 |
| `merged` | **TSY 生命周期（状态机 + 塌缩 + 道伥）** <br/><sub>`finished_plans/plan-tsy-lifecycle-v1.md`</sub> | `████████████` 100% | #54 | 2026-04-27 |
| `merged` | **TSY 物资 99/1 + 秘境分流死亡 + 干尸** <br/><sub>`finished_plans/plan-tsy-loot-v1.md`</sub> | `████████████` 100% | #53 | 2026-04-27 |
| `merged` | **搜打撤坍缩渊 meta plan** <br/><sub>`finished_plans/plan-tsy-v1.md`</sub> | `████████████` 100% | #47 #49 #50 #51 #53 #54 #55 #59 | 2026-04-27 |
| `merged` | **TSY 地形/POI/NPC anchor 自动生成** <br/><sub>`finished_plans/plan-tsy-worldgen-v1.md`</sub> | `████████████` 100% | #51 | 2026-04-27 |
| `merged` | **TSY Zone P0 收尾（集成测 + Server→Redis 桥）** <br/><sub>`finished_plans/plan-tsy-zone-followup-v1.md`</sub> | `████████████` 100% | #50 | 2026-04-26 |
| `merged` | **TSY Zone P0 基础** <br/><sub>`finished_plans/plan-tsy-zone-v1.md`</sub> | `████████████` 100% | #49 | 2026-04-26 |
| `merged` | **视觉特效基础栈** <br/><sub>`finished_plans/plan-vfx-v1.md`</sub> | `████████████` 100% | — | 2026-04-13 |
| `merged` | **武器 v1.1 补完：schema/channel/伤害/持久化/资源** <br/><sub>`finished_plans/plan-weapon-v1.1.md`</sub> | `████████████` 100% | #69 #80 | 2026-04-28 |
| `merged` | **武器法宝完整链路（ItemInstance → Weapon Component → 3D 渲染）** <br/><sub>`finished_plans/plan-weapon-v1.md`</sub> | `████████████` 100% | #41 | 2026-04-30 |
| `merged` | **Worldgen raster → Anvil region exporter** <br/><sub>`finished_plans/plan-worldgen-anvil-export-v1.md`</sub> | `████████████` 100% | #79 | 2026-04-30 |
| `merged` | **Worldgen 视觉快照 CI（5 角度真画面 + raster 双轨）** <br/><sub>`finished_plans/plan-worldgen-snapshot-v1.md`</sub> | `████████████` 100% | #71 | 2026-04-28 |
| `merged` | **巨树生成方向** <br/><sub>`finished_plans/plan-worldgen-v3.1.md`</sub> | `████████████` 100% | — | 2026-04-13 |
| `merged` | **Rust 运行时地形生成** <br/><sub>`finished_plans/plan-worldgen-v3.md`</sub> | `████████████` 100% | — | 2026-04-20 |
| `merged` | **世界生成混合方案** <br/><sub>`finished_plans/plan-worldgen.md`</sub> | `████████████` 100% | — | 2026-03-30 |

### 图例

- `merged` — 代码已合并主线，plan 主体落地
- `wip` — 设计 active，部分代码已落地，仍在推进
- `design` — 设计 active，零或近零代码
- `skeleton` — 骨架 plan，等待开工
- `done` — 已归档（M0/M1 阶段产物）

_数据源：[`docs/plans-progress.yaml`](docs/plans-progress.yaml) · 渲染脚本：[`scripts/plans_progress.py`](scripts/plans_progress.py) · 经 GitHub Action 在 plan 改动时自动更新_
<!-- END:PLANS_PROGRESS -->
