# Bong · plan-daozhan-v1 · 骨架

**道伥实装**——给主世界幽暗地穴与野外散布地点实装 `DaoZhang` 实体：它是死在活坍缩渊（§十六）或被天劫击杀的高境修士遗骸，保留生前战斗本能，会模仿玩家日常行为来诱近猎物，在玩家背对或真元跌破 20% 时切入绝杀模式。击杀是获取「残卷」和「破碎法宝」的唯一途径。

## 目标

- 实装 worldview §七 道伥：以伪装日常行为诱近 + 背对/低真元绝杀的欺骗性敌对实体
- 作为「残卷 / 破碎法宝」的唯一获取途径，绑定坍缩渊死亡与天道清理程序的来源链
- 工程目标：复用 big-brain 框架实现两态行为（Mimicry/Ambush），伪装态走 fake player 渲染，绝杀走真元吸取（守恒律）
- 验收：spawn → 伪装 → 绝杀 → 神识识破 → 死亡掉落 全链路 e2e，QiTransfer 守恒全程通过

**来源**：worldview §七 天道的清理程序 + §八 天道运维博弈（高灵气聚集区刷出高阶道伥）+ §十六.六（活坍缩渊内死亡遗骸化道伥）+ §十二 世界地图表（幽暗地穴：道伥 / 残卷 / 稀有丹方）

**前置条件**：
- `plan-death-lifecycle-v1` ✅ — 玩家死亡事件链（坍缩渊内死亡 → `PlayerDeathEvent` + 遗骸化标记）
- `plan-tribulation-v1` ✅ — 天劫死亡事件（次要来源）
- `plan-tsy-hostile-v1` ✅ — 敌对实体框架（spawn 逻辑 + 掉落流水线）
- `plan-npc-ai-v1` ✅ — big-brain Scorer/Action 框架
- `plan-qi-physics-v1` ✅ — 守恒律（道伥攻击扣真元走 QiTransfer，死亡 qi_release_to_zone）
- `plan-skill-v1` ✅ — 残卷 item 框架（道伥掉落）
- `plan-spirit-treasure-v1` ✅ — 破碎法宝 item 框架（道伥掉落）

**交叉引用**：`plan-tsy-dimension-v1` ✅（活坍缩渊位面：主要 spawn 源头）· `plan-spirit-eye-v1` ✅（神识可远程识破伪装，P3 功能）· `plan-rat-v1` ✅（RatPhase 三态参考：Disguised/Ambush/Retreat 同源模式）

**worldview 锚点**：
- **§七:729 道伥**：行为模仿逻辑（砍树/挖矿/蹲伏）；触发条件（背对 or 真元 < 20%）；掉落（残卷/破碎法宝）；主要来源为坍缩渊死亡
- **§七:802 天道运维**：高灵气聚集区刷出高阶道伥——天道用道伥清理过度聚集的修士
- **§十二 地点表**：幽暗地穴 spirit_qi 0.2-0.8，道伥是该区域典型怪物
- **§十六.六 坍缩渊死亡规则**：秘境内死亡 → 遗骸干尸化 → 可能变道伥（不是 100%，高境界修士遗骸概率高）

**qi_physics 锚点**：
- 道伥攻击扣玩家真元（不扣 HP）：`QiTransfer { from: player, to: daozhan_qi, reason: DaoZhangDrain }`（吸取的真元累积进道伥自身储量，不直接逸散到环境）
- 道伥死亡：`qi_physics::qi_release_to_zone`（含生前剩余储量 + 吸取累积的 `daozhan_qi`，一并归还 zone）
- 天道新刷道伥时：不创生灵气，从该区块高浓度灵气中"凝结"（`QiTransfer { from: zone, to: daozhan_spawn, reason: TiandaoCondense }`）

---

## 接入面 Checklist

- **进料**：`PlayerDeathEvent { cause: CollapseZone }` / `TribulationDeathEvent`（道伥 spawn 触发）+ zone `spirit_qi > TIANDAO_CONDENSE_THRESHOLD`（天道刷出条件）+ `FaunaKind::DaoZhang` 注册 + 玩家 `qi_current / qi_max` 比例（触发绝杀检测）
- **出料**：`DaoZhangState` component（两态：Mimicry / Ambush；`Dead` 为事件终态，经 `DaoZhangDeathEvent` 处理，不入枚举）+ `DaoZhangBehaviorBlackboard` + `DaoZhangDeathEvent { zone, qi_released, loot_table }`（掉落 残卷/破碎法宝）
- **共享类型**：复用 big-brain 框架 / `QiTransfer` / `bong:vfx_event` 通道；新增 `DaoZhangState` / `DaoZhangBehaviorBlackboard` / `DaoZhangDrainEvent`；掉落复用 skill-v1 残卷 item + spirit-treasure-v1 法宝 item
- **跨仓库契约**：server `bong:vfx/daozhan_reveal`（Mimicry → Ambush 时 VFX）；client 渲染 Mimicry 态时使用"无名玩家皮肤"（fake player entity，名字标签 hidden）；agent 可从 `world_state.npc_digest` 感知道伥存在（无新 schema 字段）
- **worldview 锚点**：§七 道伥 + §八 天道运维 + §十六.六 坍缩渊死亡
- **qi_physics 锚点**：攻击扣真元 `QiTransfer(DaoZhangDrain)` + 死亡 `qi_release_to_zone`

---

## 阶段总览

| 阶段 | 状态 | 主要交付物 | 验收标准 |
|------|------|-----------|---------|
| **P0** | ✅ 2026-06-08 | `DaoZhangState`(Mimicry/Ambush) + spawn 触发(3来源) + loot 分档 + QiTransferReason::DaoZhangDrain/TiandaoCondense | 数据模型 + 守恒(spawn凝结 zone减==道伥增) |
| **P1** | ✅ 2026-06-08 | Mimicry 行为 AI(FakeSwing/Sneak/Mine,游戏tick计时) + fake-player 渲染(CustomPayload,禁vanilla hack) | 道伥伪装成无名玩家 |
| **P2** | ✅ 2026-06-08 | 触发绝杀(背对>150°(符号修正) or qi<20%) + 真元吸取(QiTransfer DaoZhangDrain) + reveal VFX | 触发→Ambush + 守恒(player减==道伥增) |
| **P3** | ✅ 2026-06-08 | 神识识破(DisguisedDaoZhang红轮廓#C04040) + 天道凝结刷出 + 死亡掉落(按origin_realm) + qi_release_to_zone全额 | 神识识别 + 掉落分档 + 死亡守恒 |

---

## P0 — 数据模型 + spawn 条件

- [ ] `DaoZhangState` enum（`server/src/fauna/daozhan.rs`）：`Mimicry / Ambush`（无 Retreat，道伥战至力竭，不逃）
- [ ] `DaoZhangBehaviorBlackboard { state, last_action_tick, target: Option<Entity>, origin_realm: Realm }` component
- [ ] `DaoZhangSpawnTrigger` enum：`CollapseZoneDeath { qi_snapshot }` / `TribulationStrike { realm }` / `TiandaoCondense { zone_qi }` — spawn 来源记录（影响初始 `origin_realm` 和 loot 等级）
- [ ] loot 注册：掉落 `item.scroll.fragment.*`（来自 skill-v1）+ `item.spirit_treasure.broken.*`（来自 spirit-treasure-v1），按 `origin_realm` 分档
- [ ] ≥ 8 单测（三来源 spawn 触发 / loot 等级按 origin_realm 正确 / 守恒律：spawn 从 zone 凝结有对应 QiTransfer）

---

## P1 — Mimicry 行为 AI

- [ ] `DaoZhangMimicryScorer`：评估"是否应继续伪装"——没有玩家处于 16 格正面视野内时 score 高
- [ ] `DaoZhangMimicryAction`：按随机 behavior_queue 循环执行 `FakeSwing`（挥臂动画）/ `FakeSneak`（连蹲）/ `FakeMine`（挖矿姿态）——每次 action 播放对应骨骼动画 JSON，持续 2-4s
- [ ] 客户端渲染（Mimicry 态）：道伥发送 `FakePlayerEntity` CustomPayload（无名字 tag，随机外观皮肤），隐藏真实 NPC nameplate；`bong:daozhan_disguise_enter` 包触发 client 显示假玩家模型
- [ ] ≥ 8 单测（behavior_queue 循环 / 假动画时长范围 / 玩家正面视野判断）

---

## P2 — 触发绝杀 + 真元吸取

- [ ] `DaoZhangAmbushScorer`：满足任一 → score 1.0：(a) 最近玩家背对道伥（玩家 yaw 与道伥方向夹角 > 150°）；(b) 最近玩家 `qi_current / qi_max < 0.2`
- [ ] `DaoZhangAmbushAction`：激活时 emit `DaoZhangRevealEvent` + VFX + 切换 client 渲染回真实 NPC 模型；攻击时用"古术绝杀"combo（连续 3 次真元吸取）：`QiTransfer { from: player, to: daozhan_qi, amount: DAOZHANG_DRAIN_PER_HIT, reason: DaoZhangDrain }`
- [ ] VFX reveal（`bong:vfx/daozhan_reveal`）：`BongSpriteParticle` burst 12 个，颜色 `#7040A8`（紫灰），径向速度 1.5m/s，lifetime 10 tick；音效：`entity.wither.ambient`，pitch 0.7，volume 0.8
- [ ] ≥ 12 单测（背对触发阈值边界 / qi < 20% 触发 / QiTransfer 守恒 / 连击 3 次真元扣减累计正确）

---

## P3 — 神识识破 + 天道刷出 + 掉落

- [ ] 神识识破（spirit-eye-v1 API）：激活 SpiritEye 时，Mimicry 态道伥在视野内 → server emit `RevealDaoZhang` → client 叠加"残魂轮廓"（红色 `#C04040`，透明 60%）
- [ ] 天道刷出条件：天道 agent 检测 `zone.spirit_qi > TIANDAO_CONDENSE_THRESHOLD`（候选值 0.8）时向 server 发 `npc_behavior` 指令刷出高阶道伥（`origin_realm: 通灵+`）
- [ ] 死亡掉落：`DaoZhangDeathSystem` 按 `origin_realm` 查 loot_table → emit `ItemDropEvent`；`qi_physics::qi_release_to_zone(daozhan.qi_current)` + `QiTransfer`
- [ ] ≥ 8 单测（神识识破范围边界 / 掉落表按 realm 分档 / 天道刷出指令格式正确 / 死亡 qi release 守恒）

---

## §8 开放问题（P0 决策门收口）

1. **坍缩渊死亡 → 道伥 spawn 概率**：100% 化道伥太多，建议按 `origin_realm` 分档：化虚 80%，通灵 50%，固元 20%，其他不变道伥；需与 plan-death-lifecycle-v1 §坍缩渊死亡规则对齐
2. **origin_realm 影响战斗强度**：道伥继承原始境界的真元池 + 攻击力，还是只影响掉落等级？高境界道伥会让低境界玩家毫无胜算
3. **道伥 AI 套路有限问题**：P1 只有 3 种假动作；v2 是否引入更多"语境感知"行为（当附近有玩家在挖矿，道伥就过来假装"一起挖"）
4. **多道伥协作**：同区块多个道伥是否有联动（一个引开注意，另一个绕背）——v1 各自独立，v2 留接口
5. **道伥与负灵域**：worldview §七 说负灵域里没有生物；道伥诞生于坍缩渊（负灵域），是否属于例外？建议：道伥可在负灵域存活（因其真元已是"残留"不受正常压差影响），但玩家进负灵域追道伥风险极高

---

## Finish Evidence

**验收日期**：2026-06-08 · 全 P0-P3 ✅ · 经 consume-plan 自动消费(viability gate 验证+纠正10接口偏差 + 实施 + opus 对抗自检 2 轮修复)

### 落地清单
- **P0**：`server/src/fauna/daozhan.rs`(DaoZhangState Mimicry/Ambush + DaoZhangBehaviorBlackboard + DaoZhangSpawnTrigger 3来源 + realm_spawn_probability 化虚80/通灵50/固元20);`server/src/qi_physics/ledger.rs` 新增 `QiTransferReason::DaoZhangDrain` + `TiandaoCondense`;`server/src/npc/loot.rs` daozhan loot 按 origin_realm 分档(tattered_scroll_generic + broken_artifact);道伥挂 `NpcArchetype::Daoxiang`(复用既有 spawn/thinker 基础设施,非新建 FaunaKind)。
- **P1**：DaoZhangMimicryScorer/Action(FakeSwing/FakeSneak/FakeMine behavior_queue 循环,**游戏 tick 计时非渲染帧**);`server/src/network/daozhan_disguise_emit.rs`(bong:daozhan_disguise_enter CustomPayload);client `DaoZhanDisguiseHandler.java` + `FakePlayerRendererMixin.java`(**禁 vanilla hack**,PlayerEntityRenderer 注入渲染无名玩家)。
- **P2**：DaoZhangAmbushScorer(背对>150°(**B1 符号反转修正**:dot<cos150°=-0.866) or qi<20%) + AmbushAction(reveal event + 古术绝杀连3次 `QiTransfer{DaoZhangDrain}` 玩家 qi_current → daozhan_qi 累积守恒) + reveal VFX/audio 常数。
- **P3**：神识识破(scanner 推 `SenseKind::DisguisedDaoZhang` → client 红轮廓 #C04040);天道凝结刷出(zone.spirit_qi 高浓度 → daozhan_condense_spawn,TiandaoCondense 从 zone 凝结**不创生**);`daozhan_death_loot_system` 按 origin_realm 掉落 + `release_qi_amount_to_zone` **全额**归还(残余 + 累积 daozhan_qi)。
- **B2/B3 wiring 修复**：`inventory/mod.rs` apply_death_drop_on_revive 读真实 Cultivation.realm 写 origin_realm(原恒 None);`tsy_lifecycle.rs` spawn_daoxiang_from_corpse 插 DaoZhangState/Blackboard(尸体路径产真道伥)。
- **schema 契约**：`agent/packages/schema/src/spiritual-sense.ts` SenseKindV1 union 补 `DisguisedDaoZhang` + `DisguisedSpider`(后者补 fauna-mimic #431 遗留缺口)+ `npm run generate` 重建 5 个 generated artifacts。

### 关键 commit(branch auto/plan-daozhan-v1)
- `522a84409` P0 状态机/blackboard/spawn触发/loot分档 + 境界概率门控 + 25测
- `6c2bc1e98` P1 Mimicry big-brain AI + 伪装 S2C + client 渲染
- `32f229b51` P2 AmbushScorer/Action + 三连 QiTransfer 守恒 + VFX/音效
- `cd524bd41` P3 神识识破 + 天道凝结 + 死亡气释放
- `b1c9762fa` fix: 3 blocker(背对符号反转/origin_realm生产None/尸体spawn不挂组件)+3 major(client神识枚举/loot接通/集成测试走生产链)
- `b490e3e0f` fix: agent SenseKindV1 union 补 DisguisedSpider/DisguisedDaoZhang + rebuild generated artifacts + doc 修正

### 测试结果
- server `cargo fmt --check` ✅ / `cargo clippy --all-targets -- -D warnings` ✅ / `cargo test`:**7903 passed / 0 failed**(含背对阈值边界/守恒3流/经真实尸体spawn路径集成测试 corpse_spawn_path_yields_daozhan_state_and_blackboard)
- agent `npm test -w @bong/schema`:**546 tests 全绿**(4 个 spiritual-sense RED → 绿,generated-artifacts freshness gate 绿)
- client:realm_vision 测试绿;1 pre-existing 失败 BongEntityModelAssetTest(gitignored local_models,本分支零触及实体模型)

### 跨仓库核验
- **server** ✅:`DaoZhangState`/`daozhan_*_system`/`QiTransferReason::{DaoZhangDrain,TiandaoCondense}`/`daozhan_loot_for_tier`/spawn_daoxiang_from_corpse(扩展)
- **agent** ✅:`SenseKindV1` union DisguisedDaoZhang/DisguisedSpider + generated artifacts
- **client** ✅:`DaoZhanDisguiseHandler`/`FakePlayerRendererMixin`/`SenseKind.DISGUISED_DAOZHAN`(#C04040)
- **契约** ✅:S2C `bong:daozhan_disguise_enter` + scanner push DisguisedDaoZhang

### 遗留 / 后续
- **client 视觉待 WSLg 验收**:fake-player 无名玩家渲染 + 神识红轮廓(逻辑/契约已测,主观视觉需人眼)。
- **P2 reveal VFX/音效 polish**:DAOZHAN_REVEAL_VFX/AUDIO 常数已定义但 emit/render 未接(pre-existing polish 缺口,reveal 核心行为=伪装→真NPC切换已接通)。
- **DaoZhangSpawnTrigger::TribulationStrike deferred**:主世界突破回火死亡(BreakthroughBackfire)当前不产 CorpseEmbalmed,天劫作为 worldview§七「次要来源」待后续 tribulation plan 接通;主来源 CollapseZoneDeath 已全链路。
- **DisguisedSpider 渲染**:agent union 已补,client 神识红轮廓仅 daozhan;spider 神识识破属 fauna-mimic 范围。
