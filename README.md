# Bong — 末法残土

> **AI-Native 修仙沙盒**：一个灵气接近枯竭的末世修仙世界，运行在 Minecraft 之上。
> 天道不是脚本——它是 LLM Agent，实时推演这个世界的灾劫、变化与命运。
>
> 天地不仁，灵气已薄如纸。你醒来时，身处一片灵脉将枯的荒土。
> 这里没有宗门收你，没有前辈指路，连一株像样的灵草都要和野兽抢。
> 你唯一的优势是——你会死很多次，而每一次死亡都不是白死的。

---

## 这是什么

Bong 是一个以「**末法时代**」为核心的 AI-Native 修仙沙盒：

- **修仙不是数值堆叠，而是经脉拓扑的物理进化** —— 六境界（醒灵 → 引气 → 凝脉 → 固元 → 通灵 → 化虚）对应体内经脉的逐条贯通，境界可以掉落，突破需要触发性事件，化虚是服务器内仅容一两人的天花板。
- **灵气是守恒的** —— 全服灵气总量恒定（`SPIRIT_QI_TOTAL`），你修炼消耗的灵气就是别人少掉的灵气。灵气按压强法则从高浓度流向低浓度，所有流动都走统一账本 `qi_physics::ledger`，没有凭空产生、也没有凭空消失。
- **天道是活的** —— 三路 LLM Agent 并发推演世界（灾劫 / 变化 / 演绎时代），经 Arbiter 仲裁后通过 Redis 向游戏世界下达指令：降天劫、设伪灵脉陷阱、发动兽潮、垂青顿悟、更迭时代。玩家在聊天栏里看到的天道叙事，就是这套推演的实时输出。
- **末法残土的生存逻辑** —— 匮乏、算计、信息差、搜打撤。苟得住才活得久，但苟太久会被天道盯上。

世界观正典见 [`docs/worldview.md`](docs/worldview.md)（唯一权威，所有玩法/命名/经济的锚点）。
世界观的**独立维护库**已拆出至 [末法Cantu](https://github.com/Kizunad/MofaCantu)——正典按章拆分 + 三十八卷馆藏 + 按题材分十卷的写作大纲。

## 核心玩法

### 修炼：经脉与突破
静坐冲击 / 丹药辅助 / 强行爆冲三条修炼途径；突破需「经脉数达标 + 触发事件」双条件。真元长期不足会灵脉萎缩、境界跌落。天劫（渡虚劫 / 域崩 / 定向天罚）与化虚名额由天道按世界灵气预算动态调控。

### 战斗：多血条与流派
体表 16 部位 × 6 档伤口、经脉 20 条 × 4 档损伤、真元池三层级联的死亡判定；距离衰减让末法战斗变成"拼刺刀"；过载撕裂是赌命的爆发。

已实装流派：**体修·爆脉流**、**器修·暗器流**、**地师·阵法流**（诡雷 / 警戒场 / 陷阱阵旗）、**毒蛊流**（凝针与经脉永久损伤）、**黑无室剑道**、**截脉震爆流**、**绝灵涡流**、**替尸蜕壳**（伪装）、**医道**（接续经脉 / 续命）。每招都有独立的动画、粒子、音效、HUD 反馈与图标。

### 生产与经济
炼丹（火候三系统 + 丹毒节拍 + 副作用识别）、炼器（四步状态机）、锻造台配方、野生植物采集生态、灵田（季节 / 天气 / 二级加工）、矿物体系、灵木采伐、通用保质期（三路径衰减）。**骨币是唯一真货币**（异变兽骨 + 阵法锁真元），灵石只是劣质燃料，金银是废土。

### 坍缩渊（TSY）：搜打撤秘境
上古大能透支天地而陨落之处，灵压极低（-1.2）。类塔科夫的搜打撤循环：5 档容器搜刮、四档敌对 NPC（道伥 / 执念 / 守灵 / 畸变体）、撤离点倒计时、秘境塌缩。负灵域也是战术空间——通灵境修士可以躲入逃避天劫，低境界也能靠灵压差极限反杀高境界。

### 世界与 NPC
灵压三态环境（馈赠区 / 死域 / 负灵域）、伪灵脉陷阱、游离风暴、兽潮迁徙、垂死大能遭遇、派系战争。NPC 由 big-brain Utility AI 驱动：老化、渡劫、散修日常，离屏时进入 Dormant 虚拟化批量推演——但任何离屏战死，残余真元都会守恒地归还世界。

### 社会与死亡
匿名社会 + 声名（SocialRenown）、灵龛守家、切磋交易、多世人生：死亡不是终点，遗念、碑刻与亡者博物馆让每一世留下痕迹。

## 技术架构

五层架构，跨层契约以 TypeBox schema 为唯一 source of truth（TS → JSON Schema → serde），Redis 承载 server ↔ agent IPC：

```
┌─────────────┐    CustomPayload     ┌──────────────────────┐
│  client/    │ ◄──────────────────► │  server/             │
│  Fabric 微端 │    MC 1.20.1 协议 763 │  Rust 无头服务器      │
│  Java 17     │                      │  Valence + Bevy ECS  │
│  owo-lib UI  │                      └──────────┬───────────┘
└─────────────┘                                 │ Redis IPC
                                  ┌──────────────▼───────────┐
                                  │  agent/ 天道 Agent       │
                                  │  TypeScript 三 Agent 并发 │
                                  │  灾劫 / 变化 / 演绎 → 仲裁 │
                                  └──────────────────────────┘
```

| 目录 | 技术栈 | 职责 |
|------|--------|------|
| `server/` | Rust · Valence · Bevy 0.14 ECS | 无头 MC 服务器（协议 763）。修炼 / 战斗 / 生产 / 经济 / NPC / 灵气守恒账本 `qi_physics` |
| `client/` | Java 17 · Fabric 1.20.1 · owo-lib | 微端：HUD、技能条、动画（PlayerAnimator）、粒子 VFX、交易/炼丹/炼器等全部 UI |
| `agent/` | TypeScript · openai · ioredis | "天道" LLM Agent 层：三 Agent 并发推演 + Arbiter 仲裁 + WorldModel 持久化 |
| `schema/`（agent/packages） | TypeBox | IPC schema 唯一真源，双端生成 + sample 对拍 |
| [BongWorldGen](https://github.com/Kizunad/BongWorldGen) | Python · NumPy | 独立的 seed 可复现地形生成器；导出 mmap-friendly raster 供运行时按需生成 chunk |
| [末法Cantu](https://github.com/Kizunad/MofaCantu) | Markdown · JSON | 世界观设定库：正典十八节 + 馆藏三十八卷 + 十卷写作大纲 + 悬案与留白清单 |
| `scripts/` | bash / Python | dev harness：构建、e2e、bot 场景回归、视觉资产工具链 |

## 快速开始

```bash
# Server（offline mode，监听 :25565）
scripts/build-token.sh cargo run

# Client（Fabric 微端，经 WSLg 启动）
scripts/build-token.sh gradle runClient

# 天道 Agent（需真实 LLM key，或 mock 模式）
cd agent/packages/tiandao && npm start
cd agent/packages/tiandao && npm run start:mock     # 无 key 跑通全链路

# 地形生成（独立仓库）
cd ../BongWorldGen
.venv/bin/pip install -e '.[dev]'
.venv/bin/bong-worldgen --width 256 --height 256 --seed 812731 --output generated/demo.npz

# 一键开发重载（regen + validate + rebuild + restart）
bash scripts/dev-reload.sh

# 完整冒烟测试
bash scripts/smoke-test.sh
```

## 文档索引

| 文档 | 内容 |
|------|------|
| [`docs/worldview.md`](docs/worldview.md) | 世界观正典（唯一权威：境界 / 经济 / 命名 / 区域表） |
| [末法Cantu](https://github.com/Kizunad/MofaCantu) | 世界观设定库（独立仓库）：分章正典 · 馆藏 · 写作大纲 · 悬案清单 |
| [`docs/CLAUDE.md`](docs/CLAUDE.md) | 开发工作流：Plan 体系、接入面 checklist、孤岛红旗清单 |
| [`docs/roadmap.md`](docs/roadmap.md) | 里程碑路线图 |
| [`docs/finished_plans/`](docs/finished_plans/) | 已落地玩法 plan（100+ 份，含各系统接口面） |
| [`docs/server-architecture.md`](docs/server-architecture.md) | 服务端架构设计 |
| [`docs/player-animation-conventions.md`](docs/player-animation-conventions.md) | 动画约定（PlayerAnimator 四大坑） |

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
