# Bong · plan-settlement-hideout-v1（Milestone「据点与驿路」骨架）

> **Milestone 定位**：CLAUDE.md `## Current milestone` 目前只登记 **M1 — 天道闭环** ✅。本 plan 提案为 **M2 — 据点与驿路**：给末法残土补上「玩家在世界上有固定坐标的家」「物资跨区域流动」「从野外活着回来的手段」这三条缺失的骨干。
> 编号与 CLAUDE.md 的 milestone 段落更新需人工执行（agent 不擅改 CLAUDE.md，见 §8 开放问题 #0）。

一句话：**聚落是末法唯一能停战的地方，宿窟是玩家在聚落里租下的一格秩序，驮队是把物资交给别人保管的赌博，而回家的路只有两条——自己走，或者花钱雇人陪你走。**

### 用户裁定（本 plan 的最强约束，2026-09-02）

1. **宿窟 = 灵龛的替代，不是并存。** 野外从此**没有任何玩家安全点**。既有 `social::SpiritNiche` 全套迁移改造为宿窟，不保留双轨（§〇.2 / P1）。
2. **去掉"中途没有传送"这一说法**（§二 C1）。但**本 milestone 不实装任何传送**——见裁定 3。
3. **脱困只做两档**：档 0 两条腿、档 1 **信号弹呼叫附近镖客来接应**。**归窟符 / 传送阵一类道具本期不做**，推迟到未来版本（§8 #12）。

---

## 〇、为什么末法会有「城」（设计第一性，先过这关再谈机制）

### 〇.1 聚落存在的三条正典推论

正典 §一「末法」+ §十「灵气零和」+ §八「天道驱散强者」的直接推论是**修士天然互相猎杀**。那聚落凭什么存在？三条推论全部从既有正典导出，不新增世界规则：

1. **安全来自贫瘠**。聚落必须建在灵气近零（0.02–0.08）的死域边缘。没人能在那修炼 → 没人为它厮杀 → 它才活得下来。这与 §十一 L924「灵龛不提供灵气」是同一条物理律的宏观版本——**这条律现在由宿窟继承**。
2. **秩序来自贬值**。§九 L850 骨币半衰逼迫所有人疯狂流转 → 流转必须见面 → 见面必须有不被当场砍死的场所。聚落的「不杀令」不是道德，是**生意**：破坏者被全镇追杀，因为他坏了所有人换现的通道。
3. **部落是血缘版本**。部落营（异变者/残族/流民）靠族群而非契约维系，排外、收费更高、对外来修士随时可能翻脸——它提供的是**更便宜但更不稳定**的秩序。

### 〇.2 宿窟替代灵龛：换掉的是「隐蔽」，不是「家」

灵龛的核心交易是 **用隐蔽换安全**（§十一 L920「只有自己知道坐标」，L925「坐标被发现保护失效」）。宿窟把这笔交易整个换掉：**用登记换武力背书**。

| 维度 | 灵龛（旧，将废止） | 宿窟（新） |
|---|---|---|
| 位置 | 野外任意点 | **聚落内固定编号铺位** |
| 安全来源 | 隐蔽（暴露即废） | 镇卫 + 不杀令（公开但有武力背书） |
| 成本 | 一次性龛石 | **持续骨币租金**（对抗囤积，§九 L850） |
| 代价 | 无保障 | **被登记**：身份暴露、被人知道"你有资产" |
| 灵气 | 不提供 | 不提供（**同一条正典，原样继承**） |
| 储存 | 小 | 大、可升级 L1-L5、可挂工位 |
| 复活 | 死后复活于此 | 死后复活于此（**平移**） |
| 守护者 | `HouseGuardian` 家傀 | `HouseGuardian` 家傀（**平移**） |
| 入侵 | `niche_defense::resolve_intrusion` | 同一裁决路径（**平移**） |

**平移比例远高于删除**。既有实装里真正被废止的只有「坐标隐蔽/揭示」一条链路（`SpiritNicheReveal*`）——因为宿窟本来就是公开登记的。储存、复活、守护者、入侵结算、修复、渲染、庇护所家具配方线（`craft/recipes/workbench/shelter.toml`：门闩/灯笼/防潮地基/床铺/伪装网/**灵龛基座**）全部继承。

**放置校验是替代的技术支点**：`handle_spirit_niche_place_requests`（`server/src/social/mod.rs:1783`）今天接受任意坐标；替代后只接受「聚落内空闲 slot」。一处校验收紧 + 一轮彻底重命名，就是这次替代的骨架。

### 〇.3 替代的玩法后果与对策

野外不再有安全点，四条连锁反应：

1. **「撤」变成真的要跑回去**。搜打撤的撤离从"就近钻灵龛"变成"回聚落"——这是对 §十三 L1215-1221 尺度感正典的**兑现**。→ P6 给出两档退路（自己走 / 花钱雇人陪你走），**都得走**。
2. **深入野外的风险上限被拉高**。没有中途落脚点、也没有瞬间脱身手段，越走越远 = 越难回来。**这是本期的刻意取向**：远征是真赌博，不是有保险的观光。信号弹只降低"死在半路"的概率，不缩短路程。
3. **死亡惩罚变重**：复活点全在聚落，跑尸距离变长。P8 数值校准专盯这条曲线。
4. **新手引导要改**：`world/spawn_tutorial.rs` 现在教"找地方放灵龛"，改为"走到锈骨镇落籍"——第一个目标从"藏起来"变成"进城"，更好教。

---

## 一、接入面 Checklist（防孤岛，docs/CLAUDE.md §二强制）

### 进料

| 来源模块 | 取什么 |
|---|---|
| `social::{SpiritNiche, SpiritNicheRegistry, ExposureLog}` | **承接改造**为宿窟本体（P1），不是并存 |
| `social::niche_defense::resolve_intrusion` + `HouseGuardian` | 宿窟入侵/守护者原样平移 |
| `craft/recipes/workbench/shelter.toml` | 庇护所家具线（含 `niche_base`）→ 宿窟开户凭 + 工位材料 + 信号弹同族配方，解锁源全保留 |
| `npc::faction::{FactionId, FactionStore, Reputation, FactionReputationTier, MissionQueue}` | 聚落派系 / 信誉门禁 / 押镖与**接应委托**挂 `MissionQueue` |
| `economy::{BoneCoinSupply, EconomyPriceIndex, bone_coin_face_value, price_index_for_supply}` | 租金 / 运费 / **接应费与赊账**定价；骨币半衰（**不新写衰减常数**） |
| `inventory::{ContainerSpec, ContainerAcceptFilter, ItemCategory}` | 宿窟储存格 / 驮车货箱复用容器模型 |
| `shelflife::container_storage_multiplier` | 宿窟储存格保鲜；封灵台只**减缓**不停止 |
| `world::zone::ZoneRegistry` + `server/zones.json` | 聚落 zone（低灵气）注册 |
| `world::territory::{ZoneInfluenceMap, ZoneDominance, InfluenceSource}` | 聚落控制权、战争对驿路的影响 |
| `world::karma::KarmaWeightStore::{mark_player, weight_for_player}` | **劫掠 → 天道定向降灾**（现成机制） |
| `world::risk_heatmap` / `risk_signals` | 驿路风险系数、伏击点选址、**镖客接单拒单判定** |
| `world::container_block::{ContainerBlock, ContainerBlockKind}` | 宿窟内容器 / 到货箱 |
| `world::loot_pool` | 劫掠驮车的掉落池 |
| `zhenfa::{ZhenfaKind, ZhenfaCarrierKind}` | 藏匿阵 / 镇卫警戒阵，新增 kind 变体（**本期无传送阵**） |
| `npc::{navigator, movement, patrol, dormant, lod, interaction_memory}` | 驮队/镖客寻路、护卫巡逻、离屏推进、记仇 |
| `npc::war` + `npc::territory` | 聚落间敌对 → 驿路封锁 / 运费飙升 / 接应拒单 |
| `identity` + `social::components::ExposureLog` | 入镇登记、**信号弹暴露**、通缉态 |
| `qi_physics::ledger` | 藏匿阵维持消耗的守恒出口（§9） |
| `player::home_return` / `player::spawn_selector` | 「回家」叙事 / 复活点锚点从灵龛改宿窟 |
| `world::spawn_tutorial` | 新手引导目标改为"进城落籍" |
| `persistence` | 老存档灵龛 → 宿窟迁移（P1；与 `plan-refactor-persistence-slices-v1` 协调） |
| `supply_coffin::{authority, refresh, loot}` | 权属判定 + 定时刷新范式 |

### 出料

- 新 zone + POI 进 `worldgen` blueprint（deterministic layout，P0）
- `SettlementRegistry` / `HideoutRegistry`（由 `SpiritNicheRegistry` 改名）/ `CaravanRegistry` / `DistressBeaconRegistry` 进持久化
- emit `SettlementEnteredEvent` / `HideoutUpgradedEvent` / `CaravanDepartedEvent` / `CaravanRaidedEvent` / `DistressBeaconFiredEvent` / `EscortDispatchedEvent` → agent 天道叙事 + `world::events`
- 劫掠 → `KarmaWeightStore::mark_player` → 既有定向天劫链路
- 押镖 / 接应完成 → `npc::faction` 信誉 + `MissionQueue` 结算
- 运费/租金/接应费流水 → `economy` 遥测（骨币不是真元，不入 ledger，§9）

### 共享类型 / event（复用与改名，不另造）

- **改名平移**：`NicheIntrusionEvent`→`HideoutIntrusionEvent`，`SpiritNicheActivateGuardianV1`→`HideoutActivateGuardianV1`，`NicheGuardianFatigueV1/BrokenV1` 同理
- **废止**：`SpiritNicheRevealRequest` / `SpiritNicheCoordinateRevealRequest` / `SpiritNicheRevealSource` / client `SpiritNicheRevealBootstrap`——宿窟公开登记，reveal 语义消失。`ExposureLog` 保留并新增「入镇登记」「信号弹」「资产暴露」变体
- **复用** `LootContainerOpenV1` 开宿窟储存 / 到货箱（改它必须 `.proto` + TypeBox samples 同改，见 `reminder.md` 套包族）
- **复用** `MissionQueue` 承载押镖与接应委托，不另造委托系统
- 新增 event 一律前缀 `Settlement*` / `Hideout*` / `Caravan*` / `Escort*`，**禁止近义重名**
- **不写兼容层**：一次改名到位，不留 `SpiritNiche` 别名（`feedback_no_compat_clean_code`）

### 跨仓库契约

| 端 | 命中 symbol |
|---|---|
| server | `world::settlement::*` / `player::hideout::*` / `world::caravan::*` / `world::escort::*` / `zhenfa::ZhenfaKind::ConcealArray` |
| schema | `HideoutStateV1` / `HideoutIntrusionEventV1` / `SettlementInfoV1` / `CaravanManifestV1` / `CaravanRaidedV1` / `DistressBeaconV1` / `EscortStatusV1` |
| proto | `bong.proto` payload case 改名 + 新增（**proto3 铁律**：uint64→字符串、enum→全名、坐标拍平，见 `project_wire_format_bridge_audit`） |
| client | `HideoutRenderer`（承接 `SpiritNicheRenderer`）/ `HideoutGuardian{Store,Panel,HudPlanner}` / `HideoutScreen` / `SettlementBoardScreen` / `CaravanDispatchScreen` / `DistressBeaconVfxPlayer` / `EscortStatusHudLayer` + `BongEntityModelKind` raw_id **169+**（168 `DeadDropBox` 已占，见 `world/entity_model.rs:739`） |
| agent | Redis `bong:world_state` 增聚落/驿路/求救快照；`bong:agent_cmd` 增 `settlement_price_shift` / `caravan_ambush_hint` / `route_blockade` |

### worldview 锚点

- §九 L846-866 骨币半衰 / 三种交易方式 / NPC 经济生态位 → 聚落是这三种交易的**物理场所**；镖客是新的经济生态位
- §十 L872-880 灵气零和 → 聚落建在贫瘠区的第一性理由
- §十一 L918-928 灵龛 → **整节重写为宿窟**（C3，人工 canon PR）
- §十一 L930-945 匿名 / 身份信誉 → 入镇登记与信号弹都是可控暴露
- §十三 L1215-1221 尺度感 → **L1219「中途没有传送」删除**（C1，为将来留门；本期实装里没有任何传送）
- §十三 L1272-1279 荒野 → 驿路穿荒野，伏击点与信号弹响应区在这里
- §十六 L928 灵龛不能设在坍缩渊 → 替代后自然成立
- §十七 汐转 → 驿路季节性通断

### qi_physics 锚点

- 藏匿阵/警戒阵维持消耗：`ledger::QiTransfer { from: Player, to: Zone(聚落), reason: ReleaseToZone }`
- 宿窟内**禁止** qi regen（zone spirit_qi 0.02–0.08，且 hideout AABB 内 regen 乘数硬 0）
- 信号弹**不消耗也不产生真元**（纯火药/兽油物件）——刻意设计成低境界可用
- 骨币半衰**不是真元流动**——**不得**新写任何 `*_DECAY*` / `*_HALF_LIFE*` 常数（§9）
- **本期无传送**，因此本 plan 不引入任何"位移消耗真元"的新路径

---

## 二、正典缺口

| # | 正典条文 | 状态 | 处理 |
|---|---|---|---|
| **C1** | §十三 L1219「走一趟远路要 10-20 分钟真实时间，**中途没有传送**」 | ✅ **用户已裁定：删除该说法** | 注意：**本期实装里没有任何传送**（信号弹是 NPC 走过来护送，不是位移）。这条改动是**为将来留门**，不阻塞任何代码 PR。措辞见下 |
| C2 | §十三 区域表（L1261-1268）无任何聚落 | ⬜ 待人工 PR | 5 个聚落全落在既有区域**边缘低灵气带**，不新增区域、不改既有中心坐标与灵气 |
| C3 | §十一 L918-928 灵龛整节 | ⬜ **待人工 PR，唯一硬门** | 整节重写：隐蔽换安全 → 登记换武力背书；L924 不供灵气、L926 复活点两条原样保留 |
| C4 | §十 L895-909 搜打撤「撤 → 回到安全点修养」 | ⬜ 待人工 PR | 「安全点」重定义为聚落；补一句"雇人护送"退路 |

> **agent 不得回写 `docs/worldview.md`**（CLAUDE.md 硬约束）。四条一律走**独立人工 PR**。其中 **C3 必须先于 P1 代码 PR land**；C1/C2/C4 建议同批但不阻塞。

**C1 建议 canon 措辞**（替换 L1219；刻意**不承诺具体传送机制**，将来加道具/阵不必再改一次 canon）：

> - 走一趟远路要 **10-20 分钟真实时间**，而且没有便宜的捷径。
> - 唯一能买到的帮助是**人**：在驿口挂过号的镖客可以被一发信号弹叫来，陪你走完剩下的路——要钱、要等，而且那道火光谁都看得见。至于更省事的法子，传闻里有，代价没人付得起。

---

## 三、边界（不碰什么）

| 维度 | 已有系统 | 本 plan |
|---|---|---|
| 战斗 | `combat` 全流派 ✅ | 不碰数值；劫掠与接应途中战斗只是**触发场景** |
| 容器 | `inventory` / 套包族 ✅ | 复用 `ContainerSpec`，不新造容器模型 |
| 骨币 | `economy` ✅ | 复用定价与半衰，新增**租金 / 运费 / 接应费**三条流出口 |
| NPC AI | big-brain ✅ | 复用 Scorer→Action，新增车夫/护卫（镖客）/劫匪三种 brain |
| 天劫 | `karma` ✅ | 劫掠只往 `KarmaWeightStore` 打权重，不改天劫机制 |
| 庇护所家具 | `shelter.toml` ✅ | 配方原样保留，只改放置约束与命名 |
| **传送** | 无 | **本期不做**（用户裁定 3）。归窟符 / 驿路传送阵一律推迟，见 §8 #12 |
| 搜打撤 | `plan-sou-da-che-v1` ⏳ | 提供该 plan P3 缺的"家"与退路；撤离成本变化需回写其节奏曲线 |

---

## 四、阶段总览

| 阶段 | 内容 | 状态 |
|---|---|---|
| P0 | 聚落骨架：worldgen deterministic layout + zone 注册 + 不杀令 + 镇卫 | ⬜ |
| P1 | **灵龛 → 宿窟迁移**：放置校验收紧 + 全栈改名 + 存档迁移 + 波及 plan 协调 | ⬜ |
| P2 | 宿窟等级与租约：L1-L5 / 骨币租金 / 欠租封存 / 客户端 UI | ⬜ |
| P3 | 宿窟工位：丹炉 / 锻台 / 灵田 / 蒲团 / 封灵台 / 藏匿阵 / 防潮架 | ⬜ |
| P4 | 驮队与镖客：驿口 / 委托 / 在途实体 / 护卫队 NPC / 离屏推进 / 到货箱 | ⬜ |
| P5 | 劫掠与护镖：NPC 伏击 / 玩家劫掠 / 通缉 / 押镖委托 / 镇保赔付 | ⬜ |
| P6 | **信号弹接应**：求救焰 / 接单拒单判定 / 护送 / 赊账 / 玩家当镖客 | ⬜ |
| P7 | 聚落经济与派系：物价联动 / 信誉门禁 / 战争封路 / 天道干预 | ⬜ |
| P8 | 集成与平衡：e2e 全链路 + 100h 曲线嵌入 + 数值校准 | ⬜ |

> **顺序理由**：P6 的信号弹接应复用 P4 的镖客队（`escort_squad`）与驿路寻路，必须排在 P4/P5 之后。

---

## P0 — 聚落骨架 ⬜

### 交付物

1. **5 个聚落定点**（`worldgen/blueprint/settlement/*.json`，deterministic layout，遵 docs/CLAUDE.md §6.2 禁 noise density）

| 聚落 | zone id | 位置 | 类型 | 灵气 | 定位 |
|---|---|---|---|---|---|
| 锈骨镇 | `xiugu_town` | 初醒原东缘 (600, 400) | RuinTown | 0.06 | 新手第一个家，护税最低 |
| 枯盆营 | `kupen_camp` | 青云残峰山脚 (-2600, -1500) | TribeCamp | 0.04 | 采药/矿石集散，排外 |
| 杂渡坞 | `zadu_wharf` | 灵泉湿地南岸 (-2300, 3100) | RuinTown | 0.05 | 水陆驿口，物价最高 |
| 断蹄营 | `duanti_camp` | 血谷外沿 (2700, -1900) | TribeCamp | 0.03 | 劫掠者混居，不杀令最弱 |
| 灰井驿 | `huijing_post` | 北荒南缘 (400, -5200) | RuinTown | 0.02 | 死域驿站，最险最贵 |

> **覆盖率校验（本期尤其关键）**：野外无安全点、且**没有任何传送兜底**，任一常用活动区到最近聚落须 ≤ 正典单程上限（3400-6800 格）。P0 必须产出**覆盖图**证明无"孤儿区"，否则加点或挪点。该图同时是 P6 信号弹响应半径的标定依据。

2. **`server/src/world/settlement/`**
   - `SettlementSpec { id, kind, center, radius, controlling_faction, tax_rate, peace_enforcement, hideout_slots, escort_squad_count }`
   - `SettlementRegistry`（Resource，从 `server/assets/settlements.toml` 加载，遵 registry-datafication 惯例）
   - `settlement_at(pos)` / `is_within_peace_zone(pos)` / `nearest_settlement(pos)`（**P1 迁移与 P6 接应判定都依赖**）
3. **不杀令**：聚落半径内 PvP 不是禁止而是**代价**——全镇镇卫敌对 + 该聚落 `FactionReputation` 直落 `Wanted` + `KarmaWeightStore::mark_player_floor`。`TribeCamp` 的 `peace_enforcement` 更弱 → 断蹄营是半法外之地
4. **镇卫 NPC**（`npc::patrol` + `npc::equipment`）：固元境 2-4 名 / 残镇，通灵境 1 名镇主
5. **入镇登记**：首次进入 → `ExposureLog` 记一条可控暴露 → 换取交易 / 开窟 / 雇驮队 / **被镖客响应**的资格

### 验收抓手

- `server::world::settlement::tests::{settlement_lookup_by_position, nearest_settlement_for_migration, peace_zone_pvp_marks_wanted, tribe_camp_weaker_enforcement}`
- worldgen layout determinism：同 seed 两次坐标一致；layout 半径内 decoration density mask 为 0
- zone：5 条新 zone，`spirit_qi ≤ 0.08`，`raster_check` 通过
- 建筑资产 **3 轮打磨 + `<PROMISE>`**

---

## P1 — 灵龛 → 宿窟迁移 ⬜（本 milestone 风险最高的一阶段）

> 前置硬门：**C3 canon PR 必须先 land**。正典还写着"只有自己知道坐标"时不许改代码。

### 1. 放置校验收紧（替代的技术支点）

- `handle_spirit_niche_place_requests`（`social/mod.rs:1783`）新增前置：`settlement_at(pos)` 为 `Some` 且该 slot 空闲，否则拒绝
- `identity::precondition::NichePreconditionError` 新增 `NotInSettlement` / `SlotOccupied` / `NotRegistered`
- 一名角色**同时只能持有 1 个宿窟**（沿用灵龛单一约束）

### 2. 全栈改名（一次到位，不留兼容层）

| 层 | 改动 |
|---|---|
| server | `SpiritNiche`→`Hideout`；`SpiritNicheRegistry`→`HideoutRegistry`；`position_is_within_own_active_spirit_niche`→`position_is_within_own_hideout`；`social::niche_defense`→`player::hideout::defense`；`NicheIntrusionEvent`→`HideoutIntrusionEvent` |
| schema | `SpiritNicheActivateGuardianV1`/`NicheIntrusionEventV1`/`NicheGuardianFatigueV1`/`NicheGuardianBrokenV1`/`ClientRequestSpiritNicheRepairV1` → `Hideout*`；`generated/*.json` + `samples/*.json` 全量重生成；**改完必须 `cd agent && npm run build -w @bong/schema`** |
| proto | `bong.proto` 对应 payload case 改名——**wire 破坏**，client `ProtoServerDataBridge` / `ServerDataRouter` / `WireS2cContractPinTest` 同步 |
| client | `SpiritNicheRenderer`→`HideoutRenderer`；`NicheGuardian{Store,Panel,HudPlanner}`→`HideoutGuardian*`；`NicheIntrusionAlertHandler`、`NicheRepairParticlePlayer`、`NicheDefenseReactionVfxPlayer` 同改（VfxBootstrap 注册同步）；`HomeSequence` 锚点改宿窟；lang `zh_cn/en_us` 全量文案改写 |
| 资产 | `bong/geo/spirit_niche.geo.json` + `animations/spirit_niche.animation.json` → 宿窟门/铺位模型（**3 轮打磨 + `<PROMISE>`**）；`server/assets/items/niche/` → `hideout/`；音效 recipe `niche_establish/repair.json` → `hideout_*` |
| 配方 | `workbench.shelter.niche_base` → `hideout_base`，材料/解锁源（残卷 / 师承 `hermit_builder` / 顿悟 `breakthrough`）**原样保留** |

### 3. 废止的能力

- 坐标隐蔽/揭示整链：`SpiritNicheRevealRequest` / `SpiritNicheCoordinateRevealRequest` / `SpiritNicheRevealSource` / client `SpiritNicheRevealBootstrap` + 测试 → **删除**
- `ExposureLog` 保留；`ExposureKindV1` 新增「入镇登记」「信号弹」「资产暴露」，删除「灵龛坐标泄露」

### 4. 存档迁移（参 `project_player_inventory_persist_migration_gap`：不迁移 = 老玩家卡死）

- 每个既有 `SpiritNiche` → `nearest_settlement(niche.pos)` 的 **L1 宿窟**，附赠免租 30 in-game day
- 灵龛内物品全量转入宿窟储存；超 L1 容量的溢出进「到货箱」暂存 7 in-game day
- 原灵龛处方块清理；上线 narration（scope=player）：「你在荒野里挖的那个洞塌了。有人把你的东西装进麻袋，送到了锈骨镇——附了一张欠条。」
- **幂等 + 版本号**；失败 fail-closed（不静默丢物品）
- 与 `plan-refactor-persistence-slices-v1` 协调，勿两边各写一套 slice

### 5. 波及 plan 协调（33 份 active/skeleton 引用 niche）

改名前必须处理直接冲突的：`plan-bughunt-niche-guardian-redis-dispatch`（active）、`plan-stale-spirit-niche-lifecycle-v1`、`plan-bughunt-niche-guardian-cross-session-leak-v1`、`plan-bughunt-tsy-spirit-niche-dimension-gate`、`plan-refactor-c2s-gate-v1`、`plan-refactor-wire-s2c-v1`。**要么先 merge，要么显式挂起**（`feedback_consume_e2e_merge_artifact`）。其余泛引用在 P8 统一回写。

### 验收抓手

- `server::player::hideout::tests::{place_rejected_outside_settlement, place_rejected_on_occupied_slot, one_hideout_per_character, no_qi_regen_inside_hideout}`
- 迁移测试五情形：无灵龛 / 1 个 / 多个 / 物品溢出 / 最近聚落跨维度
- 全仓 `grep -ri "spirit_niche\|SpiritNiche"` **零命中**（除迁移代码）
- client `gradle test build` 绿；schema samples 双端对拍绿；e2e 绿

---

## P2 — 宿窟等级与租约 ⬜

1. **五级**（升级 = 骨币 + 材料 + **真实工时**）

| 级 | 名 | 储存 | 租金/日 | 升级门槛 |
|---|---|---|---|---|
| L1 | 粗窟 | 6×5 | 2 骨币 | 入镇登记 |
| L2 | 石窟 | 8×6 | 5 | 聚落信誉 Normal |
| L3 | 锁窟 | 10×7 | 12 | 信誉 Medium + 凝脉 |
| L4 | 阵窟 | 12×8 | 25 | 信誉 High + 固元 + 阵法基础 |
| L5 | 髓窟 | 14×9 | 50 | 信誉 High + 通灵 + 镇主认可 |

2. **租金与欠租**：按 in-game day 扣骨币，优先从宿窟储存扣；欠租 → **封存**（不没收）；逾期 30 in-game day → 镇方**拍卖**进集市，原主收 narration
3. **储存与保鲜**：接 `shelflife::container_storage_multiplier`；骨币在宿窟内**照常半衰**
4. **复活绑定**：宿窟即复活点（灵龛路径平移）
5. **客户端** `HideoutScreen`：储存网格 + 等级 + 租约倒计时 + 工位槽；列表 diff 原地更新（`feedback_owo_scroll_clear_rebuild_bounce`），避免 `Sizing.fill(100)` 顶飞兄弟节点（`feedback_owo_fill_overflow`）

### 验收抓手

- `..::tests::{rent_deducts_from_storage_first, arrears_seals_not_confiscates, auction_after_30_days, respawn_at_hideout}`
- schema：`HideoutStateV1` 正反 sample；5 个 `HideoutLevel` 变体各一条专属 case
- 重启后 `HideoutRegistry` 完整还原（含租约 tick、封存态）

---

## P3 — 宿窟工位 ⬜

工位是**已有产出系统的驻地入口**，材料线走既有 `shelter.toml`：

| 工位 | 接入 | 效果 | 解锁 |
|---|---|---|---|
| 丹炉位 | `alchemy` | 驻地炼丹，失败率 -5% | L2 |
| 锻台位 | `forge` | 驻地锻造 + 修理 | L2 |
| 灵田槽 | `lingtian` | 2 格盆栽（灵气 0 → 只能种耐贫种） | L3 |
| 蒲团位 | `plan-dazuo-v1` | **不加修炼速度**（无灵气），只加恢复与伤势愈合 | L3 |
| 封灵台 | `economy` + `zhenfa` | 骨币半衰**减缓**至 0.6x（**不停止**，正典红线） | L4 |
| 藏匿阵 | `zhenfa::ConcealArray` | 抵抗搜查/撬窟，维持消耗真元走 ledger | L4 |
| 到货箱 | P4 驮队 | 接收驮队货物 | L1 |
| 防潮架 | `shelflife` | 承接 `reminder.md` 的 `moisture_base` 遗留待办 | L2 |

### 验收抓手

- `..::modules::tests::{sealing_altar_slows_never_stops_decay, cushion_gives_no_cultivation_speed_without_qi, conceal_array_qi_cost_goes_to_ledger}`
- 每个工位与宿主系统的 e2e

---

## P4 — 驮队与镖客 ⬜

1. **驿口**（每聚落 1 个）+ **`server/src/world/caravan/`**：`CaravanContract { id, origin, destination, kind: Freight|Passenger, manifest, guard_tier: 0..=3, insured, depart_tick, eta_tick, route, state }` + `CaravanRegistry`
2. **两种委托**：**货运**（宿窟 A → 宿窟 B 到货箱，可下线）/ **载客**（随车，1.5x 疾跑但只走驿路、显眼、可跳车）
3. **定价**：`距离 × 货重 × 路线风险系数 × EconomyPriceIndex`；护卫档 0-3 ×1.0/1.6/2.4/4.0；镇保 +30%
4. **镖客队 `EscortSquad`**（**P6 信号弹接应的基础设施，本阶段必须交付**）
   - 每聚落 `escort_squad_count` 支（残镇 2-3，部落营 1），各有 `state: Idle | OnContract | Dispatched | Returning`、`strength`（境界与人数）、`max_range`、`risk_tolerance`、`fee_multiplier`
   - 复用 `npc::patrol` + `npc::navigator` + `npc::equipment`；空闲时在驿口待命
5. **在途是世界里的真实体**：`BongEntityModelKind::CaravanCart` **raw_id 169** / `CargoCrate` **170**（168 已占）；复用既有马 + 马具资产（PR #2036）；沿 RouteNode 行进；**Marker + 自定义渲染 + C2S 交互**（禁 vanilla entity hack）；层实体销毁必须 `insert(Despawned)`（`feedback_valence_despawn_layer_entity`）
6. **离屏推进**：玩家下线/区块未加载时走 `npc::dormant` 抽象推进，到点位才实体化；抽象段遭遇战按 `plan-offscreen-war-v1` 范式——**携真元的死亡必须 `release_dormant_qi_to_zone`**
7. **到站**：货入到货箱（7 in-game day 未取退回驿口并收滞留费）
8. **客户端**：`CaravanDispatchScreen`（目的地/货物/护卫档/保险 + 实时报价）+ `CaravanStatusHudLayer`（进度 + ETA + 遇袭红标）+ 地图驿路标记
9. **视听**：车轮 `block.wood.step` pitch 0.7 每 12 tick；马 `entity.horse.gallop` vol 0.5；扬尘 `BongSpriteParticle` 每 8 tick ×2 `#6B6255`；遇袭 HUD 红闪 + `entity.horse.death`

### 验收抓手

- `server::world::caravan::tests::{price_scales_with_risk_and_index, offline_progress_matches_online_ticks, cargo_lands_in_destination_hideout, unclaimed_returns_after_seven_days, dormant_death_releases_qi_to_zone, escort_squad_states_cycle}`
- e2e：A 镇发货 → 下线 → 上线后 B 镇到货箱有货，全程守恒断言通过

---

## P5 — 劫掠与护镖 ⬜（张力核心）

**没有劫掠，驿运就只是一个慢一点的传送——整个 milestone 的意义崩塌。**

1. **NPC 伏击**：`npc::brain_bandit`（big-brain），按路线风险 + `risk_heatmap` roll 伏击点，断蹄营周边最密
2. **玩家劫掠**：打护卫 → 车夫求饶/逃跑（`interaction_memory` 记仇）→ 撬货箱（`loot_pool`，非全额）
3. **后果三件套**（全复用现成机制）：`KarmaWeightStore::mark_player` → **天道定向降灾**；聚落 `FactionReputation` → `Wanted`（镇卫见即攻击、驿口拒单、**镖客拒接你的求救**）；被目击且未匿名 → 苦主收线索，可挂**悬赏**（骨币托管 + `MissionQueue`）
4. **押镖**：驿口委托随车护送，成功得骨币 + 信誉——**新手最早的稳定收入**，接 `plan-gameplay-journey-v1` P1-P2
5. **镇保赔付**：投保货被劫赔 60% 骨币价值，**不赔物品**
6. **反制空间**：高护卫档 / 绕远低风险路线 / 分批小额 / 自己押镖 / 藏匿阵防"有人知道你在运什么"

### 验收抓手

- `..::raid::tests::{raid_marks_karma_and_wanted, insurance_pays_coins_not_items, escort_mission_settles_reputation, witness_generates_bounty_lead, wanted_player_gets_no_rescue}`
- e2e：A 劫 B 的货 → B 上线收 narration + 线索 → A 被定向降灾 + 三镇通缉

---

## P6 — 信号弹接应 ⬜（本 milestone 的玩家体验支点）

替代灵龛后，"怎么活着回去"是玩家最尖锐的痛点。**本期只有两档退路，而且两档都得走完全程**：

| 档 | 手段 | 门槛 | 代价 | 送到哪 | 延迟 | 可靠性 |
|---|---|---|---|---|---|---|
| 0 | 两条腿 | 无 | 时间 | 任何地方 | 10-20 min | 看你自己 |
| 1 | **信号弹 + 镖客接应** | 无（付得起 / 信誉不烂） | 骨币（可赊，赊了有债） | 最近聚落 | **3-8 min 等待 + 护送路程** | **可能没人来** |

> **本期没有第三档**（用户裁定 3）。这意味着**深入野外永远是单程赌注**——信号弹只降低"死在半路"的概率，**不缩短路程、不跳过路程**。远征的风险上限由此定死，P8 校准要盯住这条曲线别把中后期玩家逼到不敢出门（§8 #15）。

**核心张力：你的救命稻草，同时是给方圆几百格所有人的邀请函。**

### 交付物

1. **物品 `distress_flare`「求救焰」**：craft 可做（`rat_tail_oil` ×1 + `spirit_charcoal` ×1 + `rough_cloth` ×1，`shelter.toml` 同族配方），廉价、无境界门槛、**不消耗真元**（刻意设计成低境界的活路）
2. **点燃**：升空一发，高空悬停 **60 s**；**半径 400 格内所有玩家与 NPC 可见**（不是小粒子，是天上一朵看得见的火）
3. **响应判定**（`world::escort::dispatch_decision`，纯函数，可测）——`nearest_settlement` 的空闲 `EscortSquad` 逐支判：
   - 距离 > 该队 `max_range` → 拒
   - `risk_heatmap` 路线风险 > 该队 `risk_tolerance` → 拒（"那片地方我们不去"）
   - 求救者 `FactionReputation` 为 `Wanted` → **拒**（通缉犯没人救）
   - 求救者欠账 > 阈值 → 拒
   - 无空闲队（都在跑镖）→ 拒
   - `TribeCamp` 的队：有 `betrayal_chance`（断蹄营最高）→ 接了单，来的可能是**劫匪**
   - 全拒 → narration「没人来。火光烧完了，风把灰吹散。」**信号弹白费**
4. **接应流程**：接单队从驿口出发 → `npc::navigator` 真实跑过来（3-8 min）→ 抵达后进入**护送态**：玩家跟随，镖客挡怪挡人 → 回到聚落结算
   - **玩家不能乱跑**：离信号点 > 64 格视为放弃，队伍返程并**照收出车费**
   - **镖客保的是"把你带回城"**，不是保你打赢——你自己去挑衅/贪心捡东西，他们不管，甚至丢下你
5. **费用**：`基础出车费 + 距离 × 风险系数 × EconomyPriceIndex`；付不起 → 记账（`EscortDebt`），欠债影响信誉、达阈值全聚落拒接、极端情况扣宿窟储存抵债
6. **玩家也能当镖客**（涌现玩法，复用 `MissionQueue`）：信号弹在**附近玩家**的 HUD 上也是一条可接的委托 → 玩家跑去救人拿钱 + 信誉；当然也有人是冲着杀你去的
7. **天道视角**：`DistressBeaconFiredEvent` 进 `bong:world_state` —— 天道能看见"哪里有人在求救"，据此调整灾劫与叙事

### 视听规格（docs/CLAUDE.md §四强制精度）

- **发射**：`BongLineParticle` ×12 垂直上升，速度 2.2 格/tick，lifetime 30 tick，`#D9603A`；音效 `entity.firework_rocket.launch` pitch 1.0 vol 1.0 delay 0
- **高空火球**：`BongSpriteParticle` ×1，悬停高度 +40 格，lifetime **1200 tick（60 s）**，缓降 0.02 格/tick，`#D9603A` → 末段 `#7A2E20`，贴图**新增** `bong:textures/particle/distress_flare.png`（`/gen-image particle`）；`DistressBeaconVfxPlayer`，事件 `bong:vfx_event/distress_flare`
- **炸开**：`entity.firework_rocket.large_blast` pitch 0.8 vol 1.0 delay 25
- **HUD（求救者）**：`EscortStatusHudLayer` 屏幕右侧竖排——`等待响应…` → `枯盆营 三人队 已出发 · 约 5 分钟` → `已抵达 · 护送中` → `已抵达枯盆营 · 应付 34 骨币`；拒接时整条变灰 `#5A5550` 并显示拒接理由
- **HUD（附近玩家）**：屏幕边缘方向指示箭头 + 距离，`#D9603A`，可点开为 `MissionQueue` 委托
- **图标**：求救焰走 `/gen-image item` → `client/.../textures/gui/items/distress_flare.png`
- **narration**（3 条示例）：
  - scope=zone / style=perception：「西边天上挂起一朵橙火。有人在喊救命——也可能是饵。」
  - scope=player / style=narrative：「枯盆营的三人队来了。领头的看了眼你的伤，什么也没说，把你夹在中间往回走。」
  - scope=player / style=narrative（拒接）：「火烧完了。没人来。这一带的镖队最近折了两支，你的名字又不值钱。」

### 验收抓手

- `server::world::escort::tests::{wanted_player_refused, high_risk_route_refused, no_idle_squad_refused, debt_blocks_service, tribe_camp_betrayal_rolls, abandon_beyond_64_blocks_still_charges, escort_survives_ambush_en_route}`
- `..::flare::tests::{flare_costs_no_qi, visible_radius_400, beacon_expires_after_60s}`
- 视听回归：远处玩家能从天上火球判断"有人求救"并从 HUD 箭头找到方向
- **梯度回归**：档 1 的总代价（骨币 + 等待）在同一场景下严格高于档 0（纯时间），且**两档都不缩短路程**

---

## P7 — 聚落经济与派系 ⬜

1. 集市物价接 `EconomyPriceIndex` + 本地供需（灰井驿丹药 2x，枯盆营灵草 -40%）→ **跨镇套利**成为驿运的经济动机
2. `ZoneInfluenceMap` 决定聚落控制权；玩家可通过任务/贡献推动更替
3. `npc::war` 敌对 → 驿路封锁 / 运费飙升 / 驿口停运 / **跨镇接应拒单**（接 `territory_rumor` 放风声）
4. **天道干预**（agent）：`settlement_price_shift` / `caravan_ambush_hint` / `route_blockade`——天道靠**掐经济**驱散聚集，而不是直接砍人
5. §十七 汐转：凝汐期部分驿路封冻，运费与出车费 ×1.8

### 验收抓手

- `..::economy::tests::{cross_town_arbitrage_gap_within_bounds, war_blocks_route, season_multiplier_applies}`
- agent：`bong:world_state` 含聚落/求救快照；mock 模式下天道能发 3 类干预且被 server 消费

---

## P8 — 集成与平衡 ⬜

- 全链路 e2e：入镇登记 → 开 L1 宿窟 → 攒骨币升 L3 → 发一趟货 → 被劫 → 悬赏 → 深入野外重伤 → 放信号弹 → 镖客接应回城
- 嵌入 `plan-gameplay-journey-v1`：P1 押镖收入与第一发信号弹 / P2 第一个宿窟 / P3 跨镇套利
- **回写 `plan-sou-da-che-v1`**：撤退窗口与 Run 间节奏按两档退路重算
- 数值校准：
  - 租金 vs 日均收入（目标 L2 ≈ 15%）
  - **出车费 vs 一次 run 的收益**（目标：喊一次镖客 ≈ 吃掉本趟 20-30% 利润，肉痛但不致命）
  - 镖客拒接率（目标：常规区域 < 20%，北荒 > 60%）
  - 运费 vs 自己跑腿的时间价值
  - 劫掠期望收益 vs 天劫惩罚（**有利可图但活不长**）
  - **跑尸距离**与**远征意愿**（无第三档退路后的关键风险，见 §8 #15）
- P1 遗留：33 份泛引用 niche 的 plan 统一回写措辞

---

## §8 开放问题（P0 决策门前需收口）

| # | 问题 | 建议 |
|---|---|---|
| 0 | milestone 编号 M2？CLAUDE.md `Current milestone` 段更新 | 人工执行 |
| 1 | 命名拍板（§二表，含 `hideout_base` / `hideout_deed_stone` / `distress_flare`「求救焰」） | 用户拍板 |
| 2 | ~~C1 传送裁定~~ | ✅ 已裁定（删除"没有传送"说法；本期不实装传送） |
| 3 | C1-C4 canon PR 谁写、何时 land | **只有 C3 是硬门**（阻塞 P1）；C1/C2/C4 不阻塞代码，可同批也可后补。C1 措辞已备好（§二） |
| 4 | 野外是否保留任何"临时下线点" | 建议**不加**——加回来就是两套 |
| 5 | 老存档迁移补偿力度（免租 30 天 / 溢出 7 天） | 先按此实施，P8 按实际存档量调 |
| 6 | 宿窟能否被其他玩家撬（PvP 入室） | 建议**能**，代价极高（全镇追杀 + karma），L4 藏匿阵可拒绝一次；否则储存无风险，违反 §九「拒绝囤积」 |
| 7 | 载客驮车 1.5x 是否过快 | 建议保留：只走固定驿路、显眼、可被伏击——用风险换速度 |
| 8 | 驮车 raw_id 169/170 撞号 | 升 active 前重查 `entity_model.rs` 契约测试并登记进 `reminder.md` |
| 9 | 聚落大量 NPC 驻留是否吸干 zone 灵气 | 建议聚落 zone 设 `npc_absorb_floor` 豁免；**数值 owner 是 `plan-zone-qi-economy-v1`** |
| 10 | 33 份波及 plan 的处理顺序 | P1 启动前逐份定性，尤其 4 份 niche 专题 plan |
| 11 | 聚落 5 点布局是否无孤儿区 | P0 覆盖图后复核；**本期无传送兜底，这条校验的权重比原计划更高**；同时标定 400 格响应半径够不够 |
| 12 | **归窟符 / 驿路传送阵何时做** | ✅ 用户裁定：**本期不做**，将来另立 v2。已讨论过的设计参数留档于此备取：一次性符（异兽骨×2 + 灵木×1 + 满灵骨币×8）/ **通灵境+** / 只回自己宿窟 / 45 s 吟唱受伤即断且符废 / 代价 `qi_max × 0.40` 经 ledger 留在起点 zone / 起点半径 96 格全域暴露 / 3 in-game day CD / 负重超 50% 部分掉在起点 + 余物 5% 降耐久 / 坍缩渊内禁用。视听走 `BongRibbonParticle` 螺旋 + `BongGroundDecalParticle` 阵纹（`#7A6A55`）+ `block.beacon.activate` 起手 + 45 s 环形进度 HUD |
| 13 | 信号弹能否在坍缩渊内用 | 建议**不能**（§十六 秘境无外援，与龛石同理）；坍缩渊撤离仍走 race-out |
| 14 | 镖客背叛率（TribeCamp）上限 | 建议断蹄营 ≤ 25%，其余部落营 ≤ 10%；太高会让玩家彻底不敢喊人，退路等于没有 |
| 15 | **无第三档退路后，中后期远征意愿是否被压垮** | P8 必测：通灵玩家去北荒一趟的期望损益。若玩家普遍不敢出门，优先考虑放宽镖客覆盖（加驿口 / 提高 `max_range`）而不是急着加传送道具 |

---

## §9 守恒律与红旗自检（合并前逐条核对）

- [ ] 藏匿阵/警戒阵维持消耗**只走** `ledger::QiTransfer { reason: ReleaseToZone }`，且有 system 真正 apply 到 `WorldQiAccount`（不是 emit-only）
- [ ] 信号弹**不产生也不消耗真元**（纯物件），不得借它凭空造灵气
- [ ] 宿窟内 qi regen 乘数硬 0，不存在"在家白嫖回灵"路径
- [ ] 封灵台**不新增**任何 `*_DECAY*` / `*_HALF_LIFE*` 常数，只对 `economy` 既有半衰施加乘数
- [ ] 驮队/镖客离屏遭遇战死亡 → `release_dormant_qi_to_zone`，禁止 `store.remove` 丢快照
- [ ] 运费/租金/出车费是**骨币流动**不是真元流动
- [ ] 迁移不丢物品：失败 fail-closed，幂等 + 版本号
- [ ] 无近义重名；宿窟入侵复用改名后的 `HideoutIntrusionEvent`；接应委托复用 `MissionQueue`
- [ ] schema 改动 → `.proto` + TypeBox samples 双端同改 + `npm run build -w @bong/schema`
- [ ] 无 vanilla entity hack；驮车走 Marker + C2S IntentHandler
- [ ] 命名无禁词（玄/陨/星/仙/太/古）
- [ ] 建筑/模型资产 3 轮打磨 + `<PROMISE>`
- [ ] 全仓 `spirit_niche` 零残留
- [ ] **本期不得偷偷引入任何位移/传送路径**（含"载客驮车瞬移到站"这类简化实现）

---

## §10 实施工作流

### 10.1 PR 拆分（单 plan 多 PR 序列化，前一个 merge 后开下一个）

| PR | 范围 | 依赖 |
|---|---|---|
| **PR-0** | **C3（+C1/C2/C4）worldview canon 改动——人工 PR** | — |
| PR-1 | P0 worldgen layout + zone + `settlement` 模块 + 镇卫 + 覆盖图 | — |
| PR-2 | P1 迁移 A：放置校验收紧 + server/schema/proto 改名 | PR-0(C3), PR-1 |
| PR-3 | P1 迁移 B：client 改名 + 资产改造 + 存档迁移 | PR-2 |
| PR-4 | P2 等级/租约 + `HideoutScreen` | PR-3 |
| PR-5 | P3 工位接线 | PR-4 |
| PR-6 | P4 驮队 server + 镖客队 + 离屏推进 | PR-1/4 |
| PR-7 | P4 驮车实体 + 客户端 UI/HUD | PR-6 |
| PR-8 | P5 劫掠 + 押镖 + 通缉 | PR-7 |
| PR-9 | **P6 信号弹接应**（server + 视听 + HUD + 玩家接单） | PR-8 |
| PR-10 | P7 经济/派系/天道干预（server + agent） | PR-9 |
| PR-11 | P8 e2e + 平衡 + 波及 plan 回写 + 归档 | 全部 |

### 10.2 约束

- **C3 是硬门**：§十一 灵龛整节重写必须独立人工 PR 且先 land（CLAUDE.md 硬约束）
- PR-2/PR-3 是**改名 + 迁移**，风险最高：单独跑全栈门禁 + e2e，不与任何并行 PR 共改 `social/` 与 schema
- 建筑/模型资产 PR（PR-1、PR-3、PR-7）走 3 轮打磨 + `<PROMISE>`
- 每 PR 独立 subagent 实施，主线只收 200-500 token 结论
- 门禁按栈跑：server `cargo fmt/clippy/test`；client `gradle test build`；agent/schema `npm test`；worldgen `bash scripts/dev-reload.sh`

---

## Finish Evidence（待填）
