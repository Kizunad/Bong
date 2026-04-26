# Bong · plan-death-lifecycle-v1

**死亡 / 重生 / 终结专项**。combat §8 只讲 DeathScreen；worldview §十二 定义的「3 次运数 + 概率衰减」机制、亡者博物馆、遗念尚未落地。

**世界观锚点**：`worldview.md §十二`。**有寿元系统但宽裕**（凡人 80+，境界递增，一般玩家不会老死，用于给"长期苟着不突破"施加软上限）、**没有飞升**（化虚已是天花板，飞升属上古遗产）、**没有转世记忆继承**（终结后开新角色与旧角色无机制关联）。本 plan 不得引入"兵解 / 尸解成仙 / 转世保留资质"。

**交叉引用**：`plan-combat-v1.md §8` · `plan-cultivation-v1.md` · `plan-tribulation-v1.md` · `worldview.md §十二`。

---

## §0 设计轴心

- [x] 死亡是信息也是计数（不是简单 respawn）✅ Lifecycle.death_count + NearDeath 状态机
- [ ] 3 次运数 + 概率衰减劫数（worldview line 646-676）— 运数（fortune_remaining=3）已实装；概率衰减 Roll 未做
- [ ] **寿元宽裕**——正常玩家不会老死，但拒绝突破/长期挂机的"万年王八"必然受限
- [ ] 终结后角色入「亡者博物馆」永久公开 — 终结快照已落 `persist_termination_transition`，library-web 页未做
- [ ] 每次死必发一条「遗念」（Death Insight，由 agent 生成）

## §1 死因与去向

| 死因 | 去向 |
|---|---|
| 战斗阵亡 | 走运数/劫数判定 |
| 渡虚劫失败 | 退回通灵初期（不算死） |
| 走火入魔 / 经脉过载 | 走运数/劫数判定 |
| 域崩未撤离 | 走运数/劫数判定 |
| 死域/负灵域内死亡 | **跳过运数直接进劫数期** |
| **寿元耗尽（老死）** | **跳过运数/劫数，直接终结**（详见 §4b） |

## §2 运数期（前 3 死，满足条件即 100% 重生）

✅ 当前实装：`Lifecycle.fortune_remaining = 3` 默认；NearDeath 期满未稳定 → 消耗 1 运数 → revive，归零 → terminate。条件判定尚未介入。

条件（任一）：
- [ ] 死前 24h 内未死过
- [ ] 死亡地点不在死域/负灵域
- [ ] 业力 < 阈值
- [ ] 拥有灵龛归属

不满足 → 直接进概率期。

## §3 劫数期（第 4 死起，Roll 概率）

```
P(重生) = max(5%, 80% - 15% × (n - 3))
4死 65% / 5死 50% / 6死 35% / 7死 20% / 8+死 5% 保底
```

- [ ] 死亡瞬间 UI 显示当次概率
- [ ] **劝退 prompt**：第 4 死起，Roll 前显示"确认接受重生 / 接受终结"；主动选终结走"善终"路径（生平卷标记 `tag=自主归隐`，不掉物品）
- [ ] Roll 失败 → 角色真正终结

## §4a 寿元系统（宽裕上限，用于反挂机/反苟）

**设计意图**：寿元不是常态约束——它只卡两类人：(a) 开服数年不突破的极端苟者、(b) 被续命丹吊着却拒绝前进的 NPC/玩家。正常推进节奏的玩家在化虚前用不完。

**寿元上限（按当前境界）**：

| 境界 | 寿元上限（年） | 说明 |
|------|--------------|------|
| 无境界 / 凡人 | 80 | 基线，与现实接近 |
| 醒灵 | 120 | 初感灵气延寿 |
| 引气 | 200 | 经脉初通，代谢变慢 |
| 凝脉 | 350 | 体内循环成型 |
| 固元 | 600 | 真元核养肉身 |
| 通灵 | 1000 | 与天地共鸣，半步超脱衰老 |
| 化虚 | 2000 | 接近天花板，天道仍会催命 |

- [ ] 突破 → 寿元上限提升至新境界值；**已消耗年岁不回春**（剩余寿元 = 新上限 - 已活年岁）
- [ ] 降境 → 上限下修，若已活年岁 ≥ 新上限，立即进入「风烛」状态（见下）
- [ ] 寿元按服务器真实时间推进；默认 **1 real hour = 1 in-game year**（与 51.5h 化虚基线匹配：醒灵→化虚约 51 年，远低于化虚寿元 2000）
- [ ] **离线寿元依旧消耗**，倍率 **×0.1**（离线 10 小时 ≈ 1 年），防"挂机退游续命"
- [ ] **死域 / 负灵域寿元加速流逝**：倍率 **×2.0**（天地反吸，符合"深潜换寿"世界观）——进入/离开域时切换 tick rate
- [ ] **每次死亡扣寿**：扣除量 = 当前境界寿元上限 × 5%（凡人 4 / 醒灵 6 / 引气 10 / 凝脉 17 / 固元 30 / 通灵 50 / 化虚 100）
  - [ ] 运数期 / 劫数期 roll 成功重生也扣
  - [ ] 扣寿后剩余 ≤ 0 → 本次死亡直接转为老死结算（跳过运数/劫数）
  - [ ] 渡虚劫失败退境不扣寿（因为那不算"死"）
- [ ] inspect 面板可见：`已活 X / 上限 Y`
- [ ] 剩余寿元 < 10% → 「风烛」buff：真元回复减半 + 遗念频率提升 + 每日强制一次老化 narration
- [ ] 寿元 = 0 → 老死（不可抢救，不走运数/劫数）

## §4b 老死的特殊处理

- [ ] 不触发运数/劫数 roll，直接终结（老死是自然归宿，不是意外）
- [ ] 不掉落物品——遗物按"寿终正寝"规则：身边 3x3 留一口"遗骸"容器，其他人可来取
- [ ] 生平卷标记为"善终"（与"横死"区分，亡者博物馆分类）
- [ ] 触发一段长遗念（agent 生成回顾性 narration）

## §4c 续命（本 plan 定义代价曲线与接口，具体路径分期实装）

**核心原则**：续命有、但必须明码标价；代价随续命量递增，阻止无限叠加。

### 代价曲线（通用模型）

```
cost(Δyears) = base(method) × Δyears × (1 + 累计续寿 / 单境界寿元上限)^k
```

- `base(method)`：不同续命法的基础代价单位（业力 / 真元上限 / 境界进度）
- `k ≈ 1.5`：累计续寿越多，下一年越贵（超线性）
- 续命**不能突破当前境界寿元上限**，只能把"已活"往回拨——即：填坑不是造楼

### 续命方法（分路）

| 方法 | 主代价 | 特性 | Phase |
|------|-------|------|-------|
| 续命丹 | 真元上限永久扣除 | 线性 + 可叠加；稀有材料限制产量 | P6 |
| 坍缩渊换寿 | 境界进度回退 + 高风险 | 深潜负灵域获取"寿核"；失败直接老死 | P6+ |
| **夺舍** | 业力暴增 + 生平卷标记 | 见 §4e | P7 |
| **悟道延寿**（顿悟分支） | 一次顿悟名额 + 悟境永久下调 | 见 §4f | P6 |

### 全局约束

- [ ] 单角色总续命年岁 ≤ 当前境界寿元上限 × 2（硬上限）
- [ ] 续命后 **风烛判定不刷新**——只要剩余 < 10% 就仍在风烛，防"续 1 年解风烛"套利
- [ ] 所有续命事件写入生平卷 `lifespan_events[]`，公开可查（见 §5 不屏蔽原则）

## §4d 重生惩罚（worldview line 685-691）

- [ ] 掉落身上 50% 随机物品（任何人可拾取）
- [x] 境界降一阶 + 真元归零 ✅ `apply_revive_penalty`（cultivation/death_hooks.rs）
- [x] **真元污染清空** ✅ `contam.entries.clear()`；伤口列表清空待办（仅 health 恢复至 20%）
- [x] 3 分钟「虚弱」debuff ✅ `REVIVE_WEAKENED_TICKS = 180 × 20`（combat/components.rs）
- [ ] 重生位置：灵龛 > 出生点

## §4e 夺舍

- [ ] 夺舍对象：**仅限凡人 / 醒灵**（不允许夺舍已入道修士，避免 PK 武器化）
- [ ] 代价：业力 +100（重罪区）、真元上限永久 -20%、生平卷 `tag=夺舍者` 永久公开
- [ ] 增益：重置已活年岁到被夺舍者年龄；保留自己境界与经脉
- [ ] 被夺舍者生平卷结算为"横死"，标注夺舍者 ID；两卷交叉引用
- [ ] 交叉引用：karma 系统（`KARMA_REBIRTH_THRESHOLD` 常量）、plan-npc-ai-v1（NPC 被夺舍路径）

## §4f 悟道延寿（顿悟续命包装）

**设计约束**：不是"顿悟 → 免费延寿"，而是**把一次顿悟名额兑换为续命**，且永久降低悟境天花板。

- [ ] 触发条件：风烛状态下，主动选择"悟道延寿"顿悟选项
- [ ] 效果：剩余寿元回拨至上限 30%（脱离风烛）
- [ ] 代价：
  - 消耗一次顿悟名额（不可恢复）
  - 悟境天花板永久 -1（影响顿悟池深度）
  - 计入 §4c 累计续寿总量，不突破硬上限
- [ ] 一生仅限一次；已用过悟道延寿的角色再进风烛只能走丹药/夺舍/坍缩渊
- [ ] 交叉引用：plan-cultivation-v1（顿悟池深度定义）

## §5 终结后

- [ ] 生平卷快照入「亡者博物馆」（library-web 公开）
- [ ] 身上物品全部留世 + 道统遗物 narration 引导
- [ ] 玩家可创建新角色，**与前角色无机制关联**（知识只在玩家脑子里）

## §6 遗念（Death Insight）

- [ ] 每次死亡必发一条遗念（agent 生成，异步进消息队列，不阻塞重生）
- [ ] 境界越高，临死感知越敏锐（worldview line 757-761）
  - 醒灵/引气：短句残念（≤ 20 字）
  - 凝脉/固元：含死因/地点的回顾片段（1-2 句）
  - 通灵/化虚：长遗念，含天道视角评语
- [ ] 劫数期特殊遗念：显式告知当次概率 + Roll 前「终焉之言」
- [ ] 老死遗念：回顾生平（agent 读 LifeRecord 合成），标记为 `category=natural`
- [ ] 遗念消费端：归档到生平卷 + 可选广播给师承/同盟
- [ ] Agent prompt 模板与 tiandao 叙事系统对齐（参考 `agent/packages/tiandao`）

## §7 数据契约

**核心结构**
- [x] `LifeRecordStore`（生平卷）✅ `server/src/cultivation/life_record.rs` — BiographyEntry 含 Rebirth/NearDeath/Terminated
- [x] `DeathRegistry` ✅ 由 `Lifecycle` 组件承载：`death_count` / `last_death_tick` / `fortune_remaining`；`last_death_zone` 字段未加
- [ ] `RebirthChanceCalc`：纯函数，输入 DeathRegistry + 条件 → 概率 / 运数消耗
- [ ] `LifespanComponent`：`{ born_at_tick, years_lived, cap_by_realm, offline_pause_tick? }` — 玩家未实装；NPC 侧有 `NpcLifespan`（不通用）
- [ ] `LifespanCapTable`：境界 → 上限年岁常量表（§4a）
- [ ] `TickRateModifier`：`{ source: "offline"|"zone_death"|"zone_void", multiplier: f32 }` 栈式叠加，运行时计算有效 tick rate
- [ ] `LifespanEvent`：`{ char_id, at_tick, kind: "aging"|"death_penalty"|"extension", delta_years, source }` — 续命/扣寿/老化全部走同一事件流，公开可查

**续命相关**
- [ ] `ExtensionContract` trait：`{ fn cost(years: u32, accumulated: u32) -> ExtensionCost; fn apply(...) }`
- [ ] `ExtensionCost`：`{ karma?, qi_cap_delta?, realm_progress_delta?, enlightenment_slot? }`（四代价通道，按方法组合）
- [ ] `DuoSheEvent`：`{ host_id, target_id, at_tick, karma_delta, host_prev_age, target_age }`
- [ ] `EnlightenmentExtendEvent`：`{ char_id, at_tick, ceiling_before, ceiling_after }`

**Channel（Redis pub/sub）**
- [ ] `bong:death`（死亡触发）
- [ ] `bong:rebirth`（重生结算）
- [ ] `bong:death_insight`（遗念生成，agent 订阅）
- [ ] `bong:aging`（风烛进入、老死预告、tick rate 切换）
- [ ] `bong:lifespan_event`（续命/夺舍/悟道延寿公开流水）

**Schema 导出**：全部先在 `agent/packages/schema`（TypeBox）定义，JSON Schema 导出后 Rust 侧 `serde_derive`。

## §8 实施节点

**Phase 0 — 数据层**
- [ ] `LifespanComponent`（server/src/cultivation/lifespan.rs）
- [ ] `LifespanCapTable`（静态表，随 realm 提升）
- [x] `DeathRegistry`（按角色累计死亡次数、上次死亡时间戳）✅ `Lifecycle` 组件 + `BiographyEntry`；域标签字段待补
- [ ] `RebirthChanceCalc`（§3 公式 + §2 条件判定）
- [ ] Schema 导出到 `agent/packages/schema`（TypeBox → JSON Schema → Rust serde）

**Phase 1 — 运数期 + 重生惩罚** 🟡 部分实装（NearDeath/fortune/penalty/weakness 已有，条件判定与扣寿未做）
- [x] 接管 combat §8 DeathScreen 的 respawn 路径 ✅ `combat/lifecycle.rs::near_death_tick` 走 `PlayerRevived`
- [ ] 条件判定（24h / 域类型 / 业力阈值常量 / 灵龛归属）
- [x] 重生惩罚结算（降阶、真元归零、虚弱 buff）✅ `apply_revive_penalty` + `REVIVE_WEAKENED_TICKS`；掉落 50% / 重生点选择待办
- [ ] 扣寿 5% 规则（含 ≤ 0 转老死）

**Phase 2 — 劫数期 Roll + 劝退**
- [ ] 概率公式 + UI 显示（死亡瞬间"此次运数 X%"）
- [ ] **劝退 prompt**：Roll 前二选（重生 / 自主归隐）；自主归隐走善终
- [ ] Roll 失败 → 终结流水线入口

**Phase 3 — 寿元推进与风烛**
- [ ] 寿元 tick（cultivation system，1 real hour = 1 year 可 config）
- [ ] `TickRateModifier` 栈：离线 ×0.1、死域/负灵域 ×2.0
- [ ] 离线 tick 推进（登出时记 `offline_pause_tick`，登入结算）
- [ ] 风烛 buff（<10% 剩余）：真元回复减半、遗念频率提升、每日老化 narration
- [ ] 老死判定与结算（§4b）
- [ ] inspect 面板：已活 / 上限 / 单死扣寿预览 / 当前 tick rate

**Phase 4 — 遗念管道**
- [ ] Redis channel：`bong:death_insight`（agent 订阅生成）
- [ ] 按境界差异化 prompt 模板
- [ ] 老死长遗念（读 LifeRecord 快照）
- [ ] 入库到生平卷 `insights[]`

**Phase 5 — 终结 & 亡者博物馆**
- [ ] 生平卷快照（冻结 LifeRecord，写入 `final_snapshot`）
- [ ] library-web 亡者博物馆页面（静态生成 + 增量更新）
- [ ] "善终 / 横死 / 自主归隐 / 夺舍者" 分类筛选
- [ ] 道统遗物 narration 钩子（天道广播稀有功法残篇）
- [ ] **不提供**新人 buff / 功法线索继承（坚守"与前角色无机制关联"）

**Phase 6 — 续命 I：丹药 + 悟道延寿**
- [ ] `ExtensionContract` trait + `ExtensionCost` 四通道
- [ ] 续命丹线（真元上限永久扣除）
- [ ] 悟道延寿（§4f，依赖 plan-cultivation 顿悟池）
- [ ] `LifespanEvent` 公开流水 + 生平卷 `lifespan_events[]`
- [ ] 硬上限校验（累计续命 ≤ 2× 当前境界寿元）

**Phase 7 — 续命 II：夺舍 + 坍缩渊换寿**
- [ ] 夺舍流水线（§4e），限凡人/醒灵目标
- [ ] `DuoSheEvent` + 双卷交叉引用
- [ ] 坍缩渊"寿核"掉落（依赖 worldgen 负灵域 POI）
- [ ] 业力连锁（karma plan 成熟前用本地常量）

**Phase 8 — NPC 老化**
- [ ] NPC 共用 `LifespanComponent`（可开关，性能 vs 真实感）
- [ ] NPC 老死 → 道统遗物 + 散修刷新挂钩（协商 plan-npc-ai-v1）
- [ ] NPC 被夺舍路径

**Phase 9 — 打磨**
- [ ] 平衡回归：观察玩家寿元消耗曲线，调 tick 比例 / 续命系数 k
- [ ] 死域 tick rate 的 UI 警示（避免误踩）
- [ ] 夺舍 PVP 防滥用（冷却 / 目标警告）

## §9 已决定（原开放问题）

- ✅ **离线寿元**：继续消耗，倍率 ×0.1（见 §4a）
- ✅ **死域/负灵域加速**：×2.0（见 §4a）
- ✅ **NPC 寿元真实推进**：NPC 会老化；需配套"散修刷新/代际更替"机制（交由 plan-npc-ai-v1 承接）
- ✅ **续命代价曲线**：超线性模型 + 硬上限（见 §4c）
- ✅ **夺舍**：引入，限制对象为凡人/醒灵（见 §4e）
- ✅ **悟道延寿**：顿悟分支之一，一生一次（见 §4f）
- ✅ **业力阈值**：不强行对齐 karma plan；本 plan 内部用可配置常量 `KARMA_REBIRTH_THRESHOLD`，后续 karma plan 成熟后再收敛
- ✅ **师承继承 / 功法线索给新人**：**不做**——坚守"与前角色无机制关联"；新玩家从零开始，亡者博物馆仅供阅读不授 buff
- ✅ **生平卷屏蔽**：**不屏蔽**——"不可篡改"延伸到展示层，所有字段公开
- ✅ **劝退 prompt**：提供（见 §3）；自主终结走"善终"路径

## §10 剩余开放问题

- [ ] **NPC 代际更替节奏**：真实老化下散修死光谁来补？与 plan-npc-ai-v1 协商刷新率 / 新角色出生点
- [ ] **续命代价曲线系数 k**：k=1.5 是初值，需上线回归调整
- [ ] **夺舍的 PVP 防滥用**：限制在凡人/醒灵已降低风险，但是否要加冷却 / 夺舍者目标锁定警告？
- [ ] **悟道延寿与其他顿悟分支冲突**：顿悟池深度 -1 的具体度量（目前 plan-cultivation 未定义悟境池）
- [ ] **死域寿元加速的告知**：UI 是否实时显示当前 tick rate？（避免玩家误踩）

## §11 进度日志

- 2026-04-25：Phase 1 骨架已落 — `Lifecycle{death_count, fortune_remaining=3, weakened_until_tick}` + `apply_revive_penalty`（境界-1/qi 归零/contam 清空/LIFO 关脉）+ 3 分钟虚弱 + 终结快照持久化已实装；劫数 Roll、寿元、续命、遗念、亡者博物馆、域条件判定均未启动。
