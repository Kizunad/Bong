# Bong · plan-niche-defense-v1

**灵龛抄家防守体系**。基础灵龛（plan-social-v1 已落地）只提供"NPC 不主动攻击 + 方块不破"的被动隐蔽——本 plan 把灵龛升级为**多层守家体系**：守家傀儡 + 阵法陷阱 + 守宅道伥三档独立运作，覆盖在线 / 离线全场景。设计核心不是"打死入侵者"，而是**让抄家慢、亏、留痕**——抄家者必留"龛侵"真元色 + 物品上的异体真元残留，可被 perception 神识追凶；事件写入双方一生记录死仇，亡者博物馆永久公开。仇家不限玩家——**散修 NPC、食腐者、可能被收买的道伥**都能成为入侵者（worldview §十一 散修翻脸 + §九 食腐者背书）。

**世界观锚点**：
- `worldview.md §五.3 地师/阵法流`（"唯一能在灵龛周围建立有效防御体系的流派"——本 plan 主战场，缜密色玩家专属加成）
- `worldview.md §六.2 真元染色`（**扩"龛侵色"作为异种染色变体**——行为而非修习产生，inspect 可见，可洗；正典化进 worldview 可作为 P5 后续）
- `worldview.md §七 道伥`（守宅道伥本职——道伥"模仿玩家日常行为"特性直接复用为"模拟龛主在场"。绑定 / 反噬机制需扩 worldview，列为 P2 前置任务）
- `worldview.md §九 经济与交易`（**游商傀儡作为守家傀儡的对偶**——"抢劫触发主人收坐标"机制直接复用）
- `worldview.md §九 异体排斥 / 真元污染`（抄家者拿走的物品携带龛主真元残留，使用前不洗会触发反噬）
- `worldview.md §九 封灵骨币半衰期`（抄走的骨币照常贬值——抄家短期收益有限，逼迫快速变现）
- `worldview.md §十一 安全与社交`（灵龛基础规则 + 危机分层"散修 NPC 翻脸掠夺"——抄家 NPC 来源）
- `worldview.md §十二 一生记录`（抄家事件写"物质足迹 + 社交印记/死仇"，不可篡改，亡者博物馆永久公开）

**library 锚点**：
- `docs/library/ecology/ecology-0002 末法药材十七种.json`（夜枯藤——地师诡雷绝佳载体 / 阵石原料）
- 待写 `peoples-XXXX 龛主手记`（守家哲学 + 抄家追凶记，anchor §十一 安全 / §十二 死仇）
- 待写 `peoples-XXXX 食腐者口述`（仇家 NPC 视角，anchor §九 食腐者经济生态位）

**交叉引用**：
- `plan-social-v1.md`（前置；基础 SpiritNiche 已埋 `defense_mode: Option<DefenseModeId>` hook + ExposureLog 框架）
- `plan-zhenfa-v1.md`（深度协作；阵法陷阱是守家三档之一，本 plan 与 zhenfa 共享缚论底层）
- `plan-perception-v1.md`（追凶神识；龛侵色 + 物品残留可被神识精确感知）
- `plan-fauna-v1.md`（前置依赖；守家傀儡材料 = 异变兽骨；守宅道伥来源 = 收编低阶道伥）
- `plan-combat-no_ui.md`（异体排斥触发 AttackIntent / StatusEffect 管线）
- `plan-particle-system-v1.md`（守家载体 VFX：傀儡警戒 / 陷阱触发 / 道伥模仿 / 染色 inspect）
- `plan-cultivation-v1.md`（真元染色 RealmTaintedKind enum 扩展）
- `plan-death-lifecycle-v1.md`（一生记录"物质足迹 / 社交印记/死仇"写入入口）

**阶段总览**：
- P0 ⬜ 基础数据 + 守家傀儡（最简档）
- P1 ⬜ 阵法陷阱（zhenfa 接入）+ "龛侵色"染色入身机制
- P2 ⬜ 守宅道伥（含 worldview §七 道伥扩展前置任务）
- P3 ⬜ 异体真元残留（抄家物品反噬）+ 一生记录联动
- P4 ⬜ perception 神识追凶 + 离线场景广播 + 仇家 NPC 抄家行为接入
- P5 ⬜ worldview §六.2 染色谱补"异种染色"小节（正典化龛侵色）

**接入面**（按 docs/CLAUDE.md "防孤岛" checklist）：
- **进料**：social `SpiritNiche` + `ExposureLog` + zhenfa 阵石（P1 起）+ fauna 异兽骨（P0 起，可占位）+ perception 神识感知（P4）+ cultivation `RealmTaintedKind` enum + lifecycle 一生记录 system
- **出料**：抄家事件 → 双方生平卷写"物质足迹/死仇" + 染色入身（cultivation） + 物品携带异体残留（inventory ItemInstance 扩字段）+ 主人离线推送（network） + perception 8h 染色源精确定位
- **共享类型**：复用 `SpiritNiche` / `ExposureLog` / `RealmTaintedKind`（扩 `NicheIntrusion` variant）/ `Lifeline`（扩 IntrusionRecord）/ `VfxEvent`；**新建** `HouseGuardian` component（统一 3 档守家载体抽象层）+ `GuardianKind` enum
- **跨仓库契约**：
  - server: `social::SpiritNiche.guardians: Vec<HouseGuardian>` / `social::HouseGuardian` / `social::GuardianKind` / `social::IntrusionRecord` / `cultivation::RealmTaintedKind::NicheIntrusion`
  - schema: `SpiritNicheActivateGuardianV1` / `NicheIntrusionEventV1` / `NicheGuardianFatigueV1` / `NicheGuardianBrokenV1`
  - client: `NicheGuardianStore` / `NicheIntrusionAlertHandler` / `NicheGuardianPanel`（inspect 灵龛 tab 扩展）
  - agent: `niche_intrusion` narration kind（NarrationKind 扩展）
  - Redis channel: `bong:social/niche_intrusion`（独立 channel，便于追凶 narration 订阅）

---

## §0 设计轴心

- [ ] **延缓而非杀伤**：守家的目标是让抄家**慢、亏、留痕**——伤害值是副产品；核心是消耗入侵者的真元 / 时间 / 工具
- [ ] **染色不可避**：抄家者必留"龛侵"真元色 + 取走物品上的异体残留——双重追凶痕迹，inspect + perception 都能查
- [ ] **多层独立**：守家傀儡（远程警戒）+ 阵法陷阱（中层延缓）+ 守宅道伥（贴身潜伏）三档可独立 / 可叠加，互不替代
- [ ] **离线核心化**：抄家本质是"龛主不在场"的洗劫——所有守家载体在龛主离线时自动跑，事件推送到下次上线
- [ ] **NPC 抄家**：仇家 NPC（散修翻脸 / 食腐者卖坐标 / 收买道伥）也能抄家，不仅 PvP（worldview §十一 + §九 背书）
- [ ] **抄家收益贬值**：抄走的骨币照常半衰期（§九）；丹药 / 法器带龛主真元残留，使用前必须洗（§九 异体排斥）
- [ ] **不做传送**（worldview §十三 禁传送）：所有反击 / 追踪都基于实时坐标 + 神识感知，不做全局追兵
- [ ] **灵龛内不修炼**（worldview §十一 基本规则）：守家载体不改变这条——灵龛仍是藏物 / 养伤 / 复活点，不是修炼点

---

## §1 第一性原理（烬灰子四论挂点）

- **缚论·载体封灵**：守家三档都是"真元封缚在物质载体上"的延伸——傀儡封在异变兽骨、阵法封在环境方块、道伥封在残骸。与 zhenfa 共享底层缚论
- **影论·龛主投影**：守家载体不是独立个体，是龛主真元的"次级投影"——龛主在场时投影最稳，离线时缓慢衰减；衰减速率比阵法慢（灵龛石锁住更多灵气）
- **音论·龛侵留痕**：抄家行为本身就是真元的剧烈交涉，必留"音"——这个"音"不是声光暴露（短期），而是**烙在抄家者真元上的染色印记**（中期）+ 写入双方一生记录的业力史（永久）
- **噬论·载体朽坏**：守家载体随末法噬散逐渐朽坏；抄家也加速朽坏（每次触发损耗）。傀儡阵法核心耗尽 / 陷阱朽坏 / 道伥本能消解都是末法对真元的噬散

---

## §2 守家三档载体

| 档位 | 载体 | 主功能 | 触发条件 | 工艺难度 |
|---|---|---|---|---|
| **🥇 守家傀儡** | 异变兽骨 + 阵法 | 远程警戒 + 主人坐标推送 | 入侵者攻击傀儡或破其阵 | 中（参考 §九 游商傀儡） |
| **🥈 阵法陷阱** | 真元封入环境方块 | 中层延缓 + 真元伤害 | 入侵者踩中 / 触发触发器 | 低-中（依阵石档位） |
| **🥉 守宅道伥** | 收编低阶道伥残骸 | 贴身潜伏 + 背刺绝杀 | 入侵者背对 / 真元 < 20% | 高（**P2 前置：扩 worldview §七**） |

### 2.1 守家傀儡（GuardianKind::Puppet）

worldview §九 游商傀儡的灵龛版："凡能造傀儡守商队，便能造傀儡守家"。

- **材料**：异变兽骨 ×3（fauna-v1 P0 输出）+ 阵石·中级 ×1（zhenfa P1 输出）+ 真元 30 + 制作时间 1h
- **行为**：在灵龛 5 格内游荡，模拟"龛主家臣"在场感
  - 入侵者进入 5 格 → 傀儡释放警戒符（声光低强度 + 玩家收到坐标推送）
  - 入侵者攻击傀儡 → 傀儡阵法启动陷阱链（触发傀儡身上的预埋阵石）
  - 傀儡被破坏 → 龛主**立刻**收到坐标 + 入侵者身份（即使离线，下次上线推送）
- **耐久**：5 次反击或 24h 自然朽坏；朽坏后掉落 50% 材料，可回炉重做

### 2.2 阵法陷阱（GuardianKind::ZhenfaTrap）

worldview §五.3 阵法流的灵龛版："唯一能在灵龛周围建立有效防御体系的流派"。

- **材料**：阵石（按档位） + 真元
  - 初级陷阱：阵石·初级 ×3 + 真元 10 → 1 次触发，雷击伤害低
  - 中级陷阱：阵石·中级 ×1 + 真元 20 → 3 次触发，真元放射 AOE
  - 高级陷阱：阵石·高级 ×1 + 真元 50 → 5 次触发，幻象迷惑（StatusEffect::Disoriented，需扩 combat enum）
- **行为**：玩家在灵龛入口 / 通道 / 储物附近预埋陷阱
  - 入侵者踩中 → 真元如地刺贯穿（worldview §五.3 原话）
  - 触发后陷阱次数 -1，归零后阵核耗尽 → 自然朽坏
  - 几小时不触发也会随载体朽坏（worldview §五.3 "随载体朽坏而散尽"）
- **缜密色加成**：阵法流玩家（缜密色染色）布置时所有陷阱伤害 / 触发次数 +50%

### 2.3 守宅道伥（GuardianKind::BondedDaoxiang）⚠️ P2 前置依赖：扩 worldview §七

worldview §七 道伥本职 + 收编机制（worldview 缺口）。

- **来源**：玩家在活坍缩渊击杀低阶道伥后**收编残骸**（需要新机制——worldview §七 没明说道伥可被绑定）
- **材料**：道伥残骸 ×1 + 龛主血印（消耗龛主真元 50 + 寿元 -1%）+ 阵石·高级 ×1
- **行为**：worldview §七 原话"模仿玩家日常行为"——道伥伪装龛主在灵龛附近活动（砍树 / 挖矿 / 假装示好）
  - 龛主在场：道伥模仿龛主当前动作（伪装 + 误导追踪）
  - 龛主离线：道伥继续在灵龛附近模拟日常，迷惑 NPC 巡逻 / 玩家侦察
  - 入侵者背对道伥 / 入侵者真元 < 20% → 道伥按 worldview §七 本能"上古术法绝杀"
- **风险**（worldview §七 "天道清理程序" 逻辑）：道伥不可控
  - 每次绝杀触发 5% 概率"反噬龛主"——道伥意识返祖，攻击龛主本人
  - 反噬后必须重新通过血印压制，或被天道直接清除
  - 长期豢养（>30 天）→ 反噬概率累积上升
- **worldview §七 扩展点**（P2 前置任务，需 PR 单独提）：
  - 加"道伥绑定"小节：收编流程 + 血印消耗 + 反噬概率
  - 关联 §十六（坍缩渊死者→道伥）：死在秘境的修士也可能被他人收编为守宅道伥（伦理问题留 narration）

---

## §3 抄家流程

```
[入侵识别]
  非授权 entity（非龛主 / 非授权角色）进入灵龛 5 格内
  → 判断 SpiritNiche.guardians 是否激活
  → 启动多层防御链

[多层延缓]
  第 1 层 守家傀儡（如有）：
    傀儡警戒（声光 + 坐标推送给龛主）
    入侵者攻击傀儡 → 启动陷阱链
    傀儡破坏 → 龛主立刻收到完整入侵报告
  第 2 层 阵法陷阱（如有）：
    入侵者踩中 → 真元伤害 + 触发次数 -1
    多个陷阱独立触发，互不影响
  第 3 层 守宅道伥（如有）：
    伪装龛主日常行为
    入侵者背对 / 真元 < 20% → 按 §七 本能绝杀
    每次绝杀 5% 反噬龛主

[抄家发生]
  若入侵者突破所有层 → 灵龛储物可拿
  抄家者拿走物品 → 物品携带龛主真元残留（ItemInstance.lingering_owner_qi 字段）
  抄家者真元 → 染上"龛侵色"（RealmTaintedKind::NicheIntrusion）

[追凶痕迹]
  ⚪ 染色入身：抄家者 inspect 可见龛侵色，洗清需 8h 中性环境静坐 + 解染丹药
  ⚪ 物品残留：取走的骨币 / 丹药 / 法器使用前需洗 1h（异体排斥触发反噬）
  ⚪ 一生记录：双方生平卷写入"物质足迹 / 社交印记/死仇"
  ⚪ perception 追凶：8h 内固元+ 玩家用神识可精确感知染色源（worldview §五 神识穿透）
  ⚪ 亡者博物馆：双方任一终结后，抄家事件作为公开档案（worldview §十二 不可篡改）
```

---

## §4 抄家者代价（追凶机制详化）

### 4.1 龛侵色（真元染色变体）

复用 worldview §六.2 染色机制 + 扩 `RealmTaintedKind::NicheIntrusion` enum variant：

- **形成**：每次成功抄家（拿走 ≥1 件物品）→ 染色 +20%；累积 100% 触发"龛侵主色"
- **可视**：inspect UI 显示墨灰色斑驳真元（与正向修习染色明显区别）
- **可洗**：中性环境静坐 8h（worldview §六.2 染色洗清基线，比正向染色快是因为这是"行为染色"非"沉淀"）；服解染丹药加速 50%
- **副作用**：染色期间感知能力 -10%（业力反噬）；多次抄家叠加染色，洗清时间指数上升
- **正典化**：plan-niche-defense-v1 P5 + worldview §六.2 末尾加"异种染色（行为产生）"小节，列龛侵色为首例

### 4.2 物品异体真元残留（基于 worldview §九 异体排斥）

- **机制**：抄家拿走的物品上残留龛主真元（`ItemInstance.lingering_owner_qi: Option<{ owner: CharId, expire_at: Tick }>`）
- **持续**：8h 自然挥发；中性环境静坐 1h 可主动洗清
- **使用反噬**：未洗清的物品使用时触发 `异体排斥` —— 丹药 -50% 效果 + 服用者气血 -10；法器 -50% 攻击力 + 装备者真元每秒 -1
- **对偶 worldview §九 盲盒死信箱**：抄家版本保持物品形态（不化灰），但通过反噬增加抄家成本

### 4.3 一生记录联动（worldview §十二）

- **被抄者**生平卷写入：
  - 物质足迹："灵龛 X 于 YYYY 被入侵，损失 [items]"
  - 社交印记 / 死仇：抄家者 ID（若可识别——通过染色 / 道伥目击 / 傀儡报告）
- **抄家者**生平卷写入：
  - 业力史：抄家次数 + 累计染色峰值（不可篡改）
  - 社交印记 / 死仇：被抄者 ID + 时间 + 地点
- **亡者博物馆**：任一方终结后，抄家事件作为公开档案永久可查（worldview §十二）

### 4.4 perception 神识追凶（接 plan-perception-v1）

- 固元+ 玩家神识在龛侵色 8h 染色窗口内，可精确感知染色源 entity 位置
- 染色越深，感知距离越远（20% 染色 = 50 格；100% 主色 = 200 格）
- 染色洗清后追凶窗口关闭，但生平卷记录永久——后代 / 仇家可付费查询（§十二 玩家间口碑）

---

## §5 龛主权限与配置

- [ ] `SpiritNiche.guardians: Vec<HouseGuardian>` 数组（每档载体一条 instance）
- [ ] 龛主免疫所有守家载体（进入 5 格不触发任何反击 / 警戒）
- [ ] 多档守家载体可同时激活（傀儡 + 陷阱 + 道伥 三档全开）
- [ ] 同档可叠加多 instance（如多个陷阱埋不同位置）但有上限：
  - 傀儡：1 个（多了真元投射不稳）
  - 陷阱：5 个（按位置分布）
  - 道伥：1 个（反噬风险线性叠加）
- [ ] 龛主可手动关闭任一守家载体（材料不退，损耗归 0）
- [ ] 多人灵龛（组队场景）：v1 不支持，但 hook 预留 `authorized_chars: Vec<CharId>` 字段空间

---

## §6 平衡考量

### 6.1 抄家 vs 守家的博弈

- **守家完整开三档**：制作总成本 ~80 真元 + 6 异变兽骨 + 多档阵石——非低境界玩家承担得起
- **抄家者收益**：物品 + 坐标情报；但物品贬值（骨币半衰期 + 丹药残留反噬）+ 染色追凶 → 短期收益有限
- **结论**：守家是高境界玩家的"资产保险"；抄家是仇家清算或情报商一次性获利
- **低境界玩家**：靠隐蔽生存（worldview §十一 灵龛默认隐蔽规则），守家三档对低境界过于昂贵

### 6.2 阵法流的核心地位

worldview §五.3 明文："唯一能在灵龛周围建立有效防御体系的流派"——本 plan 通过缜密色加成确认这条：

- 缜密色玩家：所有陷阱效果 +50%（伤害 / 次数 / 触发判定）
- 非缜密色玩家：基础效果，可用但效率打折
- 这与 worldview §六.2 染色物理沉淀逻辑一致——阵法流的"真元有规律纹路"对应陷阱阵法亲和

### 6.3 NPC 抄家来源（§九 + §十一 背书）

- **散修 NPC 翻脸**：worldview §十一 明文"散修 NPC 的翻脸掠夺"——本 plan P4 接入 npc-ai-v1 的 Rogue archetype 翻脸条件（如灵气竞争激烈区域）
- **食腐者转嫁情报**：worldview §九 "把同一坐标卖给三波人"——食腐者 NPC 卖玩家灵龛坐标 → 触发 NPC 抄家事件
- **被收买的道伥**：极端场景（高阶仇家收编道伥反派）——worldview §七 道伥可被绑定（P2 扩展后）也意味着可被仇家收买，这是 niche-defense P4 的边界场景

### 6.4 反噬与朽坏作为天然限制

- 守家三档都有"自然朽坏"（24h - 几天）——逼迫玩家定期回家维护
- 道伥 5% 反噬概率 + 长期豢养反噬累积——高收益高风险
- 这些机制确保守家不是"开了就躺平"，仍要玩家投入（worldview §一 "末法残土"基调一致）

---

## §7 数据契约（下游 grep 抓手）

### server

- [ ] `social::SpiritNiche.guardians: Vec<HouseGuardian>` 字段扩展 — `server/src/social/components.rs`
- [ ] `social::HouseGuardian { kind: GuardianKind, charges_remaining: u8, decay_at: Tick, owner: CharId }` — `server/src/social/components.rs`
- [ ] `social::GuardianKind` enum (Puppet / ZhenfaTrap / BondedDaoxiang) — `server/src/social/components.rs`
- [ ] `social::IntrusionRecord { intruder: Entity, time: Tick, items_taken: Vec<u64> }` event — `server/src/social/niche_defense.rs`（新文件）
- [ ] `cultivation::RealmTaintedKind::NicheIntrusion` variant + `qi_taint_severity` 累积字段 — `server/src/cultivation/realm_taint.rs`
- [ ] `inventory::ItemInstance.lingering_owner_qi: Option<LingeringQi>` 字段 + 异体排斥 system — `server/src/inventory/components.rs` + `server/src/combat/异体_resistance.rs`
- [ ] `lifeline::write_intrusion_record` system 写双方生平卷 — `server/src/death_lifecycle/intrusion_log.rs`
- [ ] `npc::FoodScavengerSellsCoord` event 食腐者卖坐标触发 NPC 抄家（P4） — `server/src/npc/intrusion_npc.rs`

### schema（agent/packages/schema）

- [ ] `SpiritNicheActivateGuardianV1 { niche_pos, guardian_kind, materials: Vec<ItemId> }` — `agent/packages/schema/src/social.ts`
- [ ] `NicheIntrusionEventV1 { niche_pos, intruder_id, items_taken, taint_delta }` — 同上
- [ ] `NicheGuardianFatigueV1 { guardian_kind, charges_remaining }` — 同上
- [ ] `NicheGuardianBrokenV1 { guardian_kind, intruder_id }` — 同上
- [ ] `NarrationKind::NicheIntrusion` 扩展 — `agent/packages/schema/src/narration.ts`

### client

- [ ] `NicheGuardianStore` — `client/src/main/java/moe/bong/client/social/`
- [ ] `NicheIntrusionAlertHandler` — 同上（处理 `NicheGuardianBrokenV1` 推送）
- [ ] `NicheGuardianPanel` inspect 灵龛 tab 扩展 — 同上
- [ ] `NicheDefenseReactionVfxPlayer`（接 plan-particle-system-v1 注册） — `client/src/main/java/moe/bong/client/vfx/`
- [ ] `RealmTaintedHudPlanner` 扩龛侵色显示 — `client/src/main/java/moe/bong/client/hud/`

### Redis channel

- [ ] `bong:social/niche_intrusion` — server → agent / client，独立 channel 便于 narration 订阅

### asset

- [ ] 守家傀儡 toml — `server/assets/items/niche/house_puppet.toml`
- [ ] 阵法陷阱 toml × 3（初/中/高）— `server/assets/items/niche/zhenfa_trap_*.toml`
- [ ] 守宅道伥 toml（P2）— `server/assets/items/niche/bonded_daoxiang.toml`
- [ ] 解染丹药 toml — `server/assets/items/alchemy/jiet_aning_dan.toml`（接 alchemy 配方）

---

## §8 实施节点

- [ ] **P0**：基础数据 + 守家傀儡（最简档）
  - `DefenseModeId = String` → `Vec<HouseGuardian>` 类型替换
  - `SpiritNiche.guardians` 字段填充 + DB 序列化（social/mod.rs:2082-2120 + 2342）
  - `HouseGuardian` struct + `GuardianKind::Puppet` variant
  - 守家傀儡 toml + entity spawn / despawn system
  - `SpiritNicheActivateGuardianV1` ClientRequest payload
  - 单测：未带材料拒绝激活 / 激活成功 / 5 次反击后朽坏 / 24h 自然朽坏
  - **fauna-v1 P0 阻塞**：异变兽骨 item 用占位 ID（fauna-v1 落地后回填）

- [ ] **P1**：阵法陷阱（zhenfa 接入）+ 龛侵色染色入身
  - `GuardianKind::ZhenfaTrap` variant + 3 档陷阱 toml
  - 阵法触发 system（5 格踩中判定 + 真元伤害 emit）
  - `RealmTaintedKind::NicheIntrusion` enum 扩展（cultivation/realm_taint.rs）
  - 抄家成功 → 染色 +20% system
  - 染色洗清 system（8h 中性静坐 / 解染丹药）
  - inspect UI 龛侵色显示
  - 单测：陷阱触发 / 染色累积 / 染色洗清 / 缜密色加成
  - **zhenfa-v1 阻塞**：阵石 item 占位（zhenfa P1 落地后回填）

- [ ] **P2**：守宅道伥
  - **前置**：扩 worldview §七 道伥章节（绑定 / 反噬 / 收编流程）—— 单独 PR
  - `GuardianKind::BondedDaoxiang` variant
  - 道伥模仿龛主行为 system（参考 §七 本能逻辑）
  - 5% 反噬概率 + 长期累积上升 system
  - 龛主血印消耗 + 寿元扣除
  - 单测：模仿正常 / 反噬触发 / 长期反噬累积 / 血印洗
  - **fauna-v1 阻塞**：道伥残骸 drop（fauna-v1 P0 道伥死亡掉残骸）

- [ ] **P3**：异体真元残留 + 一生记录联动
  - `ItemInstance.lingering_owner_qi: Option<LingeringQi>` 字段
  - 取走物品 → 残留入身 system
  - 使用未洗清物品 → 异体排斥触发（丹药 -50% / 法器 -50% + 反噬）
  - 残留洗清 system（1h 静坐 / 自然挥发 8h）
  - `Lifeline::IntrusionRecord` 写双方生平卷（物质足迹 + 死仇）
  - 单测：物品携带 / 反噬触发 / 洗清完整 / 自然挥发 / 一生记录写入

- [ ] **P4**：perception 神识追凶 + 离线广播 + 仇家 NPC 抄家
  - perception 神识在龛侵色 8h 内精确感知染色源 entity（接 plan-perception-v1）
  - 离线时抄家事件队列 → 玩家上线立即推送（`NicheIntrusionPendingV1`）
  - 仇家 NPC 抄家行为接入 npc-ai-v1：
    - Rogue archetype 在灵气竞争激烈区域翻脸触发 IntrusionAttempt
    - 食腐者 NPC 卖坐标 → 触发 NPC 抄家 spawn
  - 单测：神识感知距离按染色深度 / 离线推送队列 / Rogue 翻脸抄家 / 食腐者卖坐标抄家

- [ ] **P5**：worldview §六.2 染色谱补"异种染色"小节
  - 在 worldview §六.2 末尾加"异种染色（行为产生）"段落，列龛侵色为首例
  - 关联 §九 异体排斥 + §十二 业力史 → 染色谱的延伸维度
  - 标注"异种染色"与"修习染色"差异：
    - 修习：累积 10h 出主色 / 中性静坐慢慢洗
    - 异种：单次行为触发 / 染色峰值后多次叠加 / 洗清更快但累积成本指数上升
  - **此为 worldview 修改 PR，单独提交，不能在本 plan 内自动改**

---

## §9 开放问题

- [ ] 守家三档同时激活时，触发顺序（同 tick）：傀儡 → 陷阱 → 道伥？还是按入侵者动作？
- [ ] 守宅道伥反噬触发时，是攻击龛主一次后消失，还是持续敌对（变成"野道伥"）？影响 §十六 坍缩渊道伥来源逻辑
- [ ] 抄家被抄玩家死亡（如同时被入侵者杀），战绩流水"杀者 ID"算入侵者还是守家载体（傀儡/陷阱/道伥）？
- [ ] 守家载体（特别是道伥）攻击其他玩家是否暴露龛主 entity？影响 §十一 匿名系统
- [ ] 离线时连续多次抄家如何节流（防止刷染色 farm）？建议同一抄家者 24h 内染色获取递减（首次 100% / 第二次 30% / 第三次 0%）
- [ ] 阵石制作归 plan-zhenfa 还是本 plan 自维护？建议 zhenfa 维护材料链，本 plan 只定义"阵石用于守家"的接口
- [ ] 多人灵龛的守家三档如何分账（材料贡献 vs 触发权限）？v1 不做但需预留 hook
- [ ] 守宅道伥豢养是否有"业力" debuff（玩家收编道伥本身是争议行为，worldview §十二 业力史是否记录"豢养道伥"）？
- [ ] 染色追凶的精度：8h 内 perception 是看到 entity 本人，还是看到"染色源轨迹"（最近经过的 chunk）？

---

## §10 进度日志

- **2026-04-27**：骨架立项。来源 `docs/plans-skeleton/reminder.md` plan-social-v1 § 灵龛防御模式节。social-v1 已预留 `defense_mode: Option<DefenseModeId>` hook（2026-04-16 决策）。
- **2026-04-30**：从 skeleton 升 active（commit e120bf6c），`/plans-status` 调研评级 ✅ Ready。social-v1 已埋好 `SpiritNiche.defense_mode` 字段 / `SPIRIT_NICHE_RADIUS=5.0` / `niche_breach` 音效 ID；P0 起手即 `DefenseModeId(=String) → DefenseModeConfig struct` + 阵石 item toml + `SpiritNicheActivateDefense` ClientRequest。
- **2026-04-30**（重写）：核心隐喻调整——从"激光炮塔反击"转向"抄家防守体系"。重读 worldview 后发现当前设计偏离 §五.3 阵法流原型（陷阱延缓而非雷击斩杀），且 worldview §九 已有完美对偶（盲盒死信箱 / 游商傀儡 / 异体排斥）。重构后核心机制：守家傀儡 + 阵法陷阱 + 守宅道伥三档独立运作 + 龛侵色 + 物品残留 + 一生记录联动。仇家 NPC 来源纳入（§十一 散修翻脸 + §九 食腐者）。新增 P5 worldview 染色谱扩展任务。守宅道伥 P2 前置任务：扩 worldview §七 道伥绑定 / 反噬章节。

## Finish Evidence

### 落地清单

- **P0 基础数据 + 守家傀儡**：
  - `server/src/social/components.rs` 新增 `HouseGuardian`、`GuardianKind`、`IntrusionRecord`，`SpiritNiche.guardians: Vec<HouseGuardian>` 替代单一 `defense_mode` 并纳入 sqlite `guardians_json` 序列化。
  - `server/src/social/niche_defense.rs` 新增守家激活、材料校验、同档上限、反击次数、自然朽坏和入侵处理主流程。
  - `server/assets/items/niche/house_puppet.toml` 与 `server/assets/items/fauna.toml` 补守家傀儡材料抓手。
- **P1 阵法陷阱 + 龛侵色**：
  - `server/src/social/niche_defense.rs` 支持 `GuardianKind::ZhenfaTrap`、触发扣次数、缜密色加成和疲劳/破损事件。
  - `server/src/cultivation/realm_taint.rs` 新增 `RealmTaintedKind::NicheIntrusion`、`qi_taint_severity` 累积与 8h 洗清规则。
  - `server/assets/items/niche/zhenfa_trap_*.toml` 与 `server/assets/items/zhenfa.toml` 补 3 档阵法陷阱资源。
- **P2 守宅道伥**：
  - `GuardianKind::BondedDaoxiang`、`server/assets/items/niche/bonded_daoxiang.toml`、反噬判定和长期朽坏窗口已作为运行时契约落地。
  - `docs/worldview.md` 的道伥绑定 prose anchor 按仓库 AGENTS 限制未自动回写，保留给专门 canon PR。
- **P3 异体真元残留 + 一生记录**：
  - `server/src/inventory/mod.rs` 新增 `LingeringQi` / `ItemInstance.lingering_owner_qi`，物品视图同步残留归属与过期 tick。
  - `server/src/combat/foreign_qi_resistance.rs` 新增未洗清异体真元物品的丹药/法器反噬规则。
  - `server/src/death_lifecycle/intrusion_log.rs` 与 `server/src/cultivation/life_record.rs` 写入双方 `BiographyEntry::NicheIntrusion`。
- **P4 perception / 离线广播 / NPC 抄家接入**：
  - `server/src/cultivation/spiritual_sense/push.rs` 与 `scanner.rs` 新增 `NicheIntrusionTrace` sense marker，固元+ 玩家可在 8h 龛侵色窗口内按 50-200 格强度范围追踪染色源。
  - `server/src/npc/intrusion_npc.rs` 接入仇家 NPC 入侵事件抓手。
  - `server/src/social/mod.rs`、`server/src/network/redis_bridge.rs`、`server/src/schema/server_data.rs` 输出 `niche_intrusion`、`niche_guardian_fatigue`、`niche_guardian_broken` 到在线 client 与 `bong:social/niche_intrusion`。
  - `agent/packages/tiandao/src/redis-ipc.ts` 订阅 `SOCIAL_NICHE_INTRUSION`，保证天道 narration 侧能消费抄家事件。
- **跨端契约 / client 可观测面**：
  - `agent/packages/schema/src/social.ts`、`client-request.ts`、`server-data.ts`、`channels.ts` 新增 `GuardianKindV1`、`SpiritNicheActivateGuardianV1`、`NicheIntrusionEventV1`、`NicheGuardianFatigueV1`、`NicheGuardianBrokenV1` 与生成 JSON/schema samples。
  - `client/src/main/java/com/bong/client/network/ClientRequestProtocol.java` 与 `ClientRequestSender.java` 支持激活守家载体请求。
  - `client/src/main/java/com/bong/client/network/SocialServerDataHandler.java`、`client/src/main/java/com/bong/client/social/NicheGuardianStore.java`、`NicheIntrusionAlertHandler.java`、`NicheGuardianPanel.java`、`NicheDefenseReactionVfxPlayer.java` 和 `RealmTaintedHudPlanner.java` 接入告警、疲劳、破损、龛侵色 HUD 与 VFX token。

### 关键 commit

- `111ebf76`（2026-05-04）`feat(server): 落地灵龛守家与龛侵痕迹`
- `9ebd19e2`（2026-05-04）`feat(schema): 扩展灵龛守家契约`
- `020c0e5f`（2026-05-04）`feat(client): 接入灵龛守家告警`
- `55f51f4d`（2026-05-04）`fix(niche-defense): 补齐抄家事件推送链路`
- `eaa91956`（2026-05-04）`feat(niche-defense): 接入龛侵色神识追凶`

### 测试结果

- `server/`: `cargo fmt --check`
- `server/`: `cargo clippy --all-targets -- -D warnings`
- `server/`: `cargo test` — 2315 passed
- `agent/`: `npm run build`
- `agent/packages/schema/`: `npm test` — 9 files / 271 tests passed
- `agent/packages/tiandao/`: `npm test` — 35 files / 236 tests passed
- `client/`: `JAVA_HOME="/usr/lib/jvm/java-17-openjdk-amd64" PATH="/usr/lib/jvm/java-17-openjdk-amd64/bin:$PATH" ./gradlew test build` — BUILD SUCCESSFUL

### 跨仓库核验

- server: `SpiritNiche.guardians`、`HouseGuardian`、`GuardianKind::{Puppet,ZhenfaTrap,BondedDaoxiang}`、`NicheIntrusionEvent`、`RealmTaintedKind::NicheIntrusion`、`LingeringQi`、`RedisOutbound::NicheIntrusion`、`SenseKindV1::NicheIntrusionTrace`。
- agent/schema: `SpiritNicheActivateGuardianV1`、`NicheIntrusionEventV1`、`NicheGuardianFatigueV1`、`NicheGuardianBrokenV1`、`CHANNELS.SOCIAL_NICHE_INTRUSION`、`SenseKindV1.NicheIntrusionTrace`。
- agent/tiandao: `CROSS_SYSTEM_EVENT_CHANNELS` 订阅 `SOCIAL_NICHE_INTRUSION`，测试覆盖缓存与回调。
- client: `encodeSpiritNicheActivateGuardian`、`SocialServerDataHandler` 的三类 niche server-data 分发、`NicheGuardianStore`、`NicheGuardianPanel`、`RealmTaintedHudPlanner` 龛侵色标签。

### 遗留 / 后续

- `docs/worldview.md` 的 P2 道伥绑定/反噬与 P5 异种染色 prose anchor 需要后续专门 canon PR；本 plan 消费流程遵守 AGENTS 禁令未自动回写 worldview。
- 守宅道伥伦理业力、多人灵龛授权分账、染色追凶精度等 §9 开放问题仍保留为后续设计项。
