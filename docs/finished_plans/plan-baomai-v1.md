# Bong · plan-baomai-v1

**体修·爆脉流**（攻击）。零距离贴脸，主动撕裂经脉换取超额真元流量瞬时灌入敌经脉。"经脉裂了可以养，命丢了养不了。"

**世界观锚点**：`worldview.md §五.1 体修/爆脉流` · `worldview.md §四.过载撕裂` · `worldview.md §六.2 沉重色`

**library 锚点**：`cultivation-0001 爆脉流正法`（唯一详写功法蓝本，5 招齐全）· `cultivation-0002 烬灰子内观笔记`（缚论"心镜龟裂"提供物理框架）· `ecology-0002 末法药材十七种`（养脉资源链）

**交叉引用**：`plan-cultivation-v1` · `plan-combat-no_ui` · `plan-combat-ui_impl` · `plan-player-animation-v1` · `plan-skill-v1` · `plan-armor-v1`（逆脉护体走 armor 反向）· `plan-HUD-v1` · **`plan-hotbar-modify-v1`**（1–9 SkillBar 接口供应方，本 plan 注册 fn pointer 替换其 P0 mock）

**P0 范围（本期实装，引气期最低级示范）**：**仅崩拳一招**（`burst_meridian.beng_quan`）— 项目里第一个真正落地的战斗功法，验证 plan-hotbar-modify-v1 的 mock skill consumer → 真实 fn pointer 替换链路。其他 4 招（贴山靠 / 血崩步 / 逆脉护体 / 燃命）章节保留作流派全景蓝本，但全部下沉 P1+。

**接口契约**：本 plan 不再扩 `QuickSlotBindings`（字符串前缀 `sk:` 方案已废弃），路由由 plan-hotbar-modify-v1 P0 接管的 `handle_skill_bar_cast` + `skill_registry` fn pointer 表负责；本 plan 只新建 `cultivation/burst_meridian.rs` 注册 fn 并实现真实结算。

**测试方针**（CLAUDE.md 饱和化测试）：每招新增的 fn / Component / Event / 状态转换都饱和覆盖（happy + 边界 + 错误分支 + 状态转换），P0 崩拳作为示范——后续 4 招按相同模板写测试，避免一招一种风格。

---

## §0 设计轴心

- [ ] 爆脉 = **主动让心镜龟裂**换取一瞬高于境界的真元投影 — 全期适用
- [ ] 5 招对应 5 个"龟裂深度档"，代价从浅到镜身崩 — **P0 仅崩拳浅档**，其余档下沉 P1+
- [ ] 克绝灵涡流（不外放）；克所有"减损"防御（求损者无视减损） — P2（VortexField 跳过判定）
- [ ] 末法约束：境界跌落，逆转条件极苛刻（高境养伤材料著者三十七年凑不齐） — 燃命才触发，P2（复用 `qi_zero_decay`）

---

## §1 第一性原理（基于烬灰子四论推演）

- **缚论·镜身限**：每境界有"安全流量阈值" = 经脉流量上限。爆脉 = 强制突破镜身阈值，让镜身龟裂换瞬时高流量
- **影论·擦镜亮光**：每次爆脉是"擦镜擦到龟裂换亮光"——亮光 = 瞬时高于境界的真元威能。代价是镜面留下永久龟裂痕（爆脉者经脉旧伤会"结疤变厚"，挨打多了反不易彻底断——书里原话）
- **音论·零距离灌音**：贴脸出招 = 用己之本音直接撞击对方心镜投影，引起对方剧烈排异
- **噬论·必须零距**：离体真元三尺即散尽，所以爆脉流唯一有效距离 = 0

---

## §2 五招分级（来自 cultivation-0001）

| 招式 | 经脉位 | 镜身龟裂深度 | 真元成本 | 战后代价 | 游戏语义 |
|---|---|---|---|---|---|
| **崩拳** | 单臂经脉（手三阴/三阳）| 浅龟裂（≤30%）| 当前真元 30%-50% | 战后片刻自愈 | **近战 AttackIntent**，qi_invest 过载倍率 ×1.5 |
| **贴山靠** | 任督二脉（躯干）| 中龟裂（50-80%）| 真元 60-80% | 任督养 1-2 日；震退断对方术法 | **近战 AttackIntent**，打断对方施法（重置 DefenseWindow）；击退 3 格 |
| **血崩步** | 双腿足三阳 | 双腿中龟裂 | 真元 60-70% | 落地双腿 SEVERED 半日；冲数丈 | **位移技**：瞬间前冲 ~8 格，落地自身双腿 SEVERED |
| **逆脉护体** | 受击点附近经脉 | 浅龟裂（被动反应）| 真元 20-30% | 体表伤口 +1 档，但污染 = 0 | **防御技**：toggle 姿态，受击自动触发皮下真元对冲 |
| **燃命** | 全身正经 | **镜身崩裂** | 真元 100% | 跌境 + 全部正经 ≥ TORN | **自毁 AoE**：自身半径 3 格内所有实体承受全额真元伤害 |

> "崩拳是第一次爆脉就会了。贴山靠是挨打挨多了自然会。血崩步是被人追杀时逼出来的。"——书里五招皆从实战逼出，非书本传授

---

## §3 玩家调用流程（核心集成路径）

### 3.0 与 plan-hotbar-modify-v1 的契约

**职责切分**（避免与 hotbar plan 重复造路径）：

| 层 | 由谁实装 | 内容 |
|---|---|---|
| 客户端按键拦截（按 1–9） | hotbar plan P0 | `KeyBindingMixin` → 查 `SkillBarStore.slots[n]` → 发 `SkillBarCast { slot, target }` |
| 协议 schema | hotbar plan P0 | `SkillBarCastRequestV1` / `SkillBarBindRequestV1` / `SkillBarBindingV1` union（kind: "skill" / "item"） |
| Server 路由 | hotbar plan P0 | `handle_skill_bar_cast` 读 `SkillBarBindings.slots[slot]` → 查 `skill_registry` 找 fn pointer → 调 fn |
| **真实 skill consumer** | **本 plan P0** | `cultivation/burst_meridian.rs::resolve_beng_quan(...)` 注册到 `skill_registry` 替换 mock |
| 冷却写回 | hotbar plan + 本 plan 协作 | fn 返回 `CastResult::Started { cooldown_ticks }`，hotbar plan 的 `tick_casts_or_interrupt` 写 `SkillBarBindings.cooldown_until_tick[slot]`（P0 简化路径，§4.1 详） |
| 动画/粒子推送 | 本 plan P0 | fn 内部 `EventWriter<BurstMeridianEvent>` → `network::burst_event_emit` → client `BurstMeridianAnimationPlayer` |
| HUD 状态灯 | 本 plan P1 | hotbar plan 的 `SkillBarHudPlanner` 加 `MeridianStatusLight` 子组件（绿/黄/红/灰）|

**fn pointer 注册位置**：plan-hotbar-modify-v1 §7.2 规定的 `server/src/cultivation/skill_registry.rs`。本 plan P0 在 `burst_meridian.rs::register_skills(&mut SkillRegistry)` 中按表注册：

```rust
// server/src/cultivation/burst_meridian.rs
pub fn register_skills(registry: &mut SkillRegistry) {
    registry.register("burst_meridian.beng_quan", resolve_beng_quan);
    // P1：4 招齐全
    // registry.register("burst_meridian.tie_shan_kao", resolve_tie_shan_kao);
    // registry.register("burst_meridian.xue_beng_bu", resolve_xue_beng_bu);
    // registry.register("burst_meridian.ni_mai_hu_ti", resolve_ni_mai_hu_ti);
    // registry.register("burst_meridian.ran_ming", resolve_ran_ming);
}
```

### 3.1 fn 签名与 CastResult（按 hotbar plan 锁定形态）

**签名**（与 plan-hotbar-modify-v1 §8 P0 路由表一致，本 plan 不重新定义）：

```rust
// 由 plan-hotbar-modify-v1 P0 实装的 skill_registry 类型
pub type SkillFn = fn(&mut World, caster: Entity, slot: u8, target: Option<Entity>) -> CastResult;

#[derive(Debug, Clone)]
pub enum CastResult {
    /// 招式成功起手 — fn 内已写好 Casting Component / 经脉扣减 / EventWriter
    Started { cooldown_ticks: u64, anim_duration_ticks: u32 },
    /// 准入失败 — fn 内未做任何 mutation
    Rejected { reason: CastRejectReason },
    /// cast 中途被打断（受击 / 控制 / 移动 >0.3m），由 tick_casts_or_interrupt 写入；fn 不直接返回这个
    Interrupted,
}

#[derive(Debug, Clone)]
pub enum CastRejectReason {
    RealmTooLow,           // 境界不足
    MeridianSevered,       // 关键经脉断
    QiInsufficient,        // 真元不够
    OnCooldown,            // 冷却未结束（client 蒙灰兜底）
    InvalidTarget,         // 目标无效（崩拳需近身实体）
    InRecovery,            // BurstRecoveryState 虚脱期内（P1 才生效，P0 永远 false）
}
```

**fn 内部职责（崩拳示范）**：
1. 查 caster 的 Cultivation / MeridianSystem / Position；任一缺失 → `Rejected`
2. 验境界 ≥ 引气；右臂手三阳（LI/SI/TE）任一非 SEVERED；qi_current ≥ cost；target 在 FIST_REACH 内
3. 通过 → 扣 qi_current（cost = qi_current × 0.4，写回前 snapshot 用于 overload_ratio 计算）；右臂经脉 integrity ×= 0.7；插 `Casting` Component（hotbar plan 的 `tick_casts_or_interrupt` system 接管 cast bar）
4. 写 `AttackIntent { target, reach: FIST_REACH, qi_invest: cost × 1.5, wound_kind: Blunt, source: AttackSource::BurstMeridian }`
5. 推 `BurstMeridianEvent { skill: "beng_quan", caster, target, tick, overload_ratio: 1.5, integrity_snapshot }` → `network::burst_event_emit`
6. 返回 `CastResult::Started { cooldown_ticks: 60, anim_duration_ticks: 8 }`

**关键设计**：fn 内部要原子化（要么全做要么全不做），不能"扣了真元但没发 AttackIntent"——`Rejected` 必须严格在所有 mutation 之前判定。

### 3.2 P0 范围：仅崩拳的完整实装路径

```
玩家在「战斗·修炼」tab（hotbar plan P1.b-e 已实装）
  → 拖崩拳到 1-9 战斗 strip · 槽 1
  → client 发 SkillBarBind { slot: 0, binding: { kind: "skill", skill_id: "burst_meridian.beng_quan" } }
  → server 写 SkillBarBindings.slots[0] = SkillSlot::Skill { skill_id }（hotbar plan 路径）

战斗中按 1
  → client KeyBindingMixin 查 SkillBarStore.slots[0] = Skill → 发 SkillBarCast { slot: 0, target: crosshair_entity_id }
  → server handle_skill_bar_cast（hotbar plan）→ skill_registry.lookup("burst_meridian.beng_quan") = resolve_beng_quan
  → resolve_beng_quan(world, caster, 0, target) →
      验证 + 经脉扣减 + qi 消耗 + AttackIntent + BurstMeridianEvent → CastResult::Started
  → hotbar plan 的 emit_skillbar_config 推 SkillBarConfigV1（含新 cooldown_until_ms）→ client 槽位蒙灰
  → 8 tick 后 tick_casts_or_interrupt 标 Casting 完成
  → AttackIntent → resolve_attack_intents（现网，不动）→ 命中 → CombatEvent / 污染推送
  → BurstMeridianEvent → client BurstMeridianAnimationPlayer 播 beng_quan 动画 + 沉重色粒子
```

### 3.3 客户端动画 + 反馈

**新增 `BurstMeridianEvent` payload**（server → client 推送）：

```typescript
// agent/packages/schema/src/server-data.ts 新增
export const BurstMeridianEventV1 = Type.Object({
  v: Type.Literal(1),
  type: Type.Literal("burst_meridian_event"),
  skill: Type.String(),                // P0 仅 "beng_quan"
  caster: Type.String(),               // canonical_player_id
  target: Type.Optional(Type.String()),
  tick: Type.Integer(),
  overload_ratio: Type.Number(),       // 过载倍率（动画力度）
  integrity_snapshot: Type.Number(),   // 涉及经脉平均 integrity（受伤动画幅度）
}, { additionalProperties: false });
```

**客户端收到 → 三件事**：
1. 调 `BongAnimationPlayer.play(caster_entity, Identifier.of("bong", "beng_quan"), priority=1500, fadeTicks=2)`
2. 粒子：沉重色（古铜 #C58B3F）真元爆发粒子，半径 0.5，持续 8 tick；命中目标时再撒一波
3. 音效：经脉龟裂音 + 闷拳击中音

**动画注册（P0 仅崩拳一行，4 招留蓝图）**：

| 招式 | animation_id | duration | 关键帧要点 | priority | P0 |
|---|---|---|---|---|---|
| 崩拳 | `beng_quan` | 8 tick (0.4s) | rightArm: pitch -80→45, z 前推 0.3; torso: yaw 微转 15° | 1500 | **✓** |
| 贴山靠 | `tie_shan_kao` | 12 tick (0.6s) | torso: pitch 前倾 30°, z 前冲 0.5; bothArm: 收拢→顶出 | 1500 | P1 |
| 血崩步 | `xue_beng_bu` | 6 tick (0.3s) | bothLeg: pitch 瞬蹲→弹直; body: z 爆发前冲 1.5; 落地: bothLeg 瘫软 | 1500 | P1 |
| 逆脉护体 | `ni_mai_hu_ti` | 持续姿态 | bothArm: 交叉胸前; 真元流光循环; 受击时额外 4 tick 震荡 | 600 | P1 |
| 燃命 | `ran_ming` | 20 tick (1.0s) | 全身骨骼震颤抖动; torso: pitch 仰天; 所有 limb 炸开; 粒子爆发 | 3000 | P2 |

**P1+ 4 招结算逻辑（蓝图保留，本期不实装）** — 详见 §3.4 备查。

### 3.4 P1+ 4 招结算逻辑（备查蓝图，本期不动）

#### 贴山靠（TieShanKao） — P1
- 任督二脉 integrity ×= 0.5（扣 50%）；qi_invest = qi_current × 0.7
- AttackIntent reach=FIST_REACH；命中后重置目标 DefenseWindow（打断施法）+ 击退 3 格

#### 血崩步（XueBengBu） — P1
- 双腿足三阳（ST/BL/GB）integrity ×= 0.4；qi_invest = qi_current × 0.65
- 瞬移 attacker Position +8 格（碰撞检查）；落地双腿 SEVERED；写 BurstRecoveryState `window_ticks=72000`

#### 逆脉护体（NiMaiHuTi） — P1（toggle 模式，与单次出招不同，需扩 SkillFn 签名 / CastResult variant）
- 开启：qi_invest = qi_current × 0.25 + `CombatState.nimai_active = true`
- 受击时（在 `resolve_attack_intents` 内判定）：受击点经脉 integrity ×= 0.85 / 污染 = 0 / 体表 wound +1
- 关闭：再次按键 toggle，或 qi_current < 5 qi/s 维持阈值

#### 燃命（RanMing） — P2（依赖 qi_zero_decay 复用）
- 全身 12 正经 integrity → 0.1；qi_invest = qi_current（全量）
- AoE：半径 3 格 AttackIntent，damage = qi_invest × 2.0
- 自身：触发 `qi_zero_decay::RealmRegressed`（复用，不重写跌境逻辑）+ 全部正经 TORN → SEVERED + `BurstRecoveryState { window_ticks: 432000, realm_drop: true }`

---

## §4 经脉虚脱与恢复系统

> **P0 顶位说明**：崩拳的"片刻自愈"特性决定它**不需要 BurstRecoveryState 完整虚脱期**——P0 用 `SkillBarBindings.cooldown_until_tick[slot] = now + 60`（hotbar plan 已规定的字段）顶位即可，60 tick = 3 秒后即可再发。`BurstRecoveryState` Component / 自愈 tick / 药材加速恢复都下沉到 P1（贴山靠/血崩步/逆脉护体/燃命才需要长虚脱期）。本章节保留作 P1 蓝图。

### 4.1 BurstRecoveryState（新 Component） — P1

```rust
// server/src/cultivation/burst_meridian.rs
pub struct BurstRecoveryState {
    /// 恢复窗口剩余 tick。在此窗口内不可再次爆脉。
    pub window_ticks: u64,
    /// 双腿是否 severed（血崩步后）
    pub legs_severed: bool,
    /// 是否因燃命跌落境界
    pub realm_drop: bool,
    /// 受波及的经脉 ID 列表
    pub affected_meridians: Vec<MeridianId>,
    /// 初始时记录的 integrity 值（用于恢复进度计算）
    pub initial_integrity_map: HashMap<MeridianId, f64>,
}
```

### 4.2 自愈 tick（新 system）

每个 server tick：
```
for each player with BurstRecoveryState:
  window_ticks -= 1
  if environment_qi > 0.3:
    for each affected meridian:
      healing_rate = 0.00001 × environment_qi × (1.0 - integrity)
      （自愈速率极慢：环境 qi=0.6 处，integrity 从 0.5 → 1.0 约需 83000 tick ≈ 70 分钟现实时间）
  else:
    // 死域内不愈
    skip
  
  if window_ticks == 0:
    remove BurstRecoveryState（但 integrity 不自动恢复——那是 healing 的职责）
```

### 4.3 药材加速恢复

当玩家服用养脉药材（凝脉草/赤髓草/固元丹等，见 §7）时：
```
healing_rate ×= 10.0 （凝脉草）
healing_rate ×= 50.0 （固元丹）
```

---

## §5 数值幅度梯度（按境界）

```
醒灵：仅崩拳，5 真元小爆，燃命直接死（镜身太薄）
引气：崩拳 + 逆脉护体；任督未通故无贴山靠；血崩步极险
凝脉：5 招齐全；血崩步双腿养 1 周；燃命跌回引气
固元：5 招熟练；养伤减半（凝核 + 真元池深）；贴山靠可断对方凝脉级术法
通灵：奇经初启，燃命跌境概率降低（缚论"无形缚影"按更紧但镜身也更厚）
化虚：理论上爆脉巅峰但化虚已断绝
```

**超载分级**（来自 cultivation-0001 其三·决堤）：
- ≤ 30%：日常小爆，片刻可复
- 50%–100%：经脉必伤，**真元上限暗减 ~20%**，半日不能战
- > 100%：拼命档，养半月起，可能直接断脉
- 全部一次：跌境（书里著者从凝脉跌回引气）

**按境界的过载倍率上限**：

| 境界 | 安全过载 | 最大过载 | 镜身耐受 |
|---|---|---|---|
| 醒灵 | 1.1× | 1.3× | 极薄（燃命即死）|
| 引气 | 1.2× | 1.6× | 薄 |
| 凝脉 | 1.4× | 2.0× | 中 |
| 固元 | 1.6× | 2.5× | 厚（养伤减半）|
| 通灵 | 1.8× | 3.0× | 很厚（燃命跌境概率 ↓）|

---

## §6 材料 / 养脉资源链

| 龟裂层 | 物理对应 | 资源 | library 来源 |
|---|---|---|---|
| 浅龟裂（integrity > 0.7）| 日常小爆自愈 | 凝脉草 / 解蛊蕊 / 清浊草 | ecology-0002 |
| 中龟裂（0.4–0.7）| 需药辅助 | 凝脉散 / 赤髓草 / 养经苔 | ecology-0002（赤髓草需清浊草煎汁配服）|
| 深龟裂（0.1–0.4）| 心镜本体龟裂 | 固元丹 + **异变兽核** | peoples-0005 异变图谱 |
| 镜身崩（< 0.1）| 投影通道断 | 通灵丹 + 兽核 + **灵眼调息** | ecology-0002 + 地理志 |

**自愈条件**：仅在 `environment_qi > 0.3` 处发生；死域内伤不会自愈

---

## §7 反噬 / 失败代价

- [ ] 燃命触发跌境，逆转条件极苛刻（需通灵丹+兽核+灵眼调息，著者三十七年未凑齐）
- [ ] 血崩步落地 SEVERED 半日不能行
- [ ] 贴山靠失败（对方未在施法 / 距离 > reach）= 任督白伤（integrity 照样扣，但无打断效果）
- [ ] 逆脉护体时机错（异种真元已深入）= 经脉照样污染（逆脉只防"入侵瞬间"）
- [ ] **战后虚脱期**（书里关键约束）：爆脉后 `BurstRecoveryState.window_ticks` 激活，最易招横祸——HUD 显式提示"经脉虚脱中"

---

## §8 克制关系

- **克**：绝灵涡流流（爆脉不外放真元，涡流无可吸）；所有"减损"防御流派（爆脉本身求损，无视减损）
- **被克**：被暗器流拉远 50 格白挨；被毒蛊流拖时间（爆脉者养伤期是死期）
- **染色亲和**：沉重色（拳修体修原生匹配，加成幅度待定）
- **错配**：锋锐色（剑修）走爆脉流——肉身扛打先天弱

---

## §9 HUD 增量

在 `plan-HUD-v1` 和 `plan-combat-ui_impl` 基础上，复用现有快捷栏：

| 元素 | 数据来源 | 触发条件 |
|---|---|---|
| **F1–F5 技能绑定** | InspectScreen 拖拽「已学功法」→ QuickSlotBind | 玩家在 Inspect 中把 5 招拖到 F 槽 |
| **技能槽状态灯** | `MeridianSystem.integrity` + `BurstRecoveryState` | 常驻于快捷栏槽位右下角（绿/黄/红/灰）|
| **mirror_integrity 条** | server BurstMeridianEvent / data sync | 常驻于 inspect 经脉层，每条经脉旁 integrity 百分比 |
| **虚脱倒计时** | `BurstRecoveryState.window_ticks` | HUD 顶部状态效果条中显示"经脉虚脱 2:45:00" |
| **cast bar** | BurstMeridianEvent.anim_duration_ticks | 按 F-key 后槽位下方渲染 cast bar（复用现有 Casting 渲染）|

---

## §10 数据契约

> **职责切分**：本 plan **只负责真实 skill consumer + animation**；按键拦截 / SkillBar 协议 / handle_skill_bar_cast 路由 / SkillBarHudPlanner 槽位渲染 全部由 plan-hotbar-modify-v1 P0 实装。本节文件清单按"P0=只做崩拳一招"严格收紧。

### 10.1 Server 端新文件（P0 仅 1 个）

| 文件 | P0 内容 | P1+ 内容 |
|---|---|---|
| `server/src/cultivation/burst_meridian.rs` | `resolve_beng_quan` 实现 + `register_skills(&mut SkillRegistry)` 注册 fn pointer + `BurstMeridianEvent` Event | 4 招 fn（tie_shan_kao / xue_beng_bu / ni_mai_hu_ti / ran_ming）；`BurstRecoveryState` Component 与自愈 tick |
| `server/src/network/burst_event_emit.rs` | `emit_burst_meridian_payloads` system：监听 `EventReader<BurstMeridianEvent>` → 推 `BurstMeridianEventV1`（mirror 现网 vfx_event_emit 模式） | — |

### 10.2 Server 端修改文件（P0）

| 文件 | P0 改动 |
|---|---|
| `server/src/cultivation/skill_registry.rs`（hotbar plan P1.a 新建） | 在 `init_registry()` 末尾调 `burst_meridian::register_skills(&mut registry)`；hotbar plan 的 mock fn 替换为 resolve_beng_quan |
| `server/src/cultivation/mod.rs` | 注册 `burst_meridian` 模块 |
| `server/src/combat/events.rs` | `AttackSource` enum 新增 `BurstMeridian` 变体（让 resolve_attack_intents 区分爆脉与普通近战，P0 暂不分流，留 hook） |
| `server/src/schema/server_data.rs` | 新增 `BurstMeridianEventV1` Rust serde 对齐 |

**P1+ 才动的（不在本期范围）**：
- `server/src/combat/resolve.rs` — 逆脉护体在 `resolve_attack_intents` 目标受击时检查 `CombatState.nimai_active`（P1 接逆脉护体时再开）
- `server/src/cultivation/components.rs` — `CombatState.nimai_active: bool` 字段（同上）
- `server/src/cultivation/qi_zero_decay.rs` 复用接口 — 燃命 P2 接入时调 `RealmRegressed` Event

**显式不做（已被 hotbar plan 接管）**：
- ~~`handle_use_quick_slot` 扩展~~ → hotbar plan `handle_skill_bar_cast` 已做
- ~~`QuickSlotBindings` Skill variant~~ → hotbar plan `SkillBarBindings` + `SkillSlot::Skill` 已做
- ~~`QuickSlotBind.item_id` 字符串前缀 `sk:`~~ → hotbar plan `SkillBarBindingV1` union 替代

### 10.3 Schema 端（P0）

| 文件 | P0 改动 |
|---|---|
| `agent/packages/schema/src/server-data.ts` | 新增 `BurstMeridianEventV1` TypeBox + 加入 `ServerDataV1` union |
| `agent/packages/schema/samples/burst_meridian_event_v1.json` | sample 双端对拍 |

P1 才加：`BurstRecoveryStateV1` payload（虚脱期 HUD 倒计时数据源）

### 10.4 Client 端新文件（P0）

| 文件 | P0 内容 |
|---|---|
| `client/.../animation/BurstMeridianAnimations.java` | 仅崩拳一招 keyframe（rightArm pitch -80→45 / z+0.3 / torso yaw +15°，8 tick）；4 招 stub 留 P1+ |
| `client/.../animation/BurstMeridianAnimationPlayer.java` | 监听 `BurstMeridianEventV1` → `BongAnimationPlayer.play(...)` + 沉重色（0xC58B3F）粒子 + 经脉龟裂音效 |
| `client/.../network/BurstMeridianEventHandler.java` | payload 解析 → `BurstMeridianAnimationPlayer.handle(event)` |

P1 才加：
- `client/.../store/BurstRecoveryStore.java`（虚脱期 HUD）
- `client/.../hud/MirrorIntegrityHudPlanner.java`（mirror_integrity 百分比覆盖）
- 经脉状态灯子组件（嵌入 hotbar plan 的 `SkillBarHudPlanner` 槽位 — 不新建文件）

---

## §11 实施节点

> **前置依赖**：plan-hotbar-modify-v1 P0 必须先 merged（mock skill consumer 路径打通），本 plan P0 才能"用真实 fn pointer 替换 mock"。两 plan 接口已在 §3.0 / §3.1 锁死。

### P0 · 崩拳单招贯通（引气期最低级示范，目标：4-5 天）

**目标**：项目里第一个真正落地的战斗功法 — 玩家学崩拳 → 拖入 1 号槽 → 战斗中按 1 → 命中目标 → 播放动画 + 粒子 + 命中结算。所有路径走 hotbar plan 接管的 SkillBar 通道，不碰 QuickSlot。

```
Schema：
  - [ ] BurstMeridianEventV1 TypeBox 定义 + 加入 ServerDataV1 union
  - [ ] Rust serde 对齐 + samples/burst_meridian_event_v1.json 双端对拍

Server：
  - [ ] cultivation/burst_meridian.rs 创建（仅 beng_quan）
  - [ ] resolve_beng_quan 完整实装：
        · 准入校验（境界 ≥ 引气 / 右臂手三阳非 SEVERED / qi_current ≥ cost / target 在 FIST_REACH）
        · 原子 mutation（snapshot qi → 扣减 → 经脉 integrity ×= 0.7 → 插 Casting）
        · 写 AttackIntent { reach: FIST_REACH, qi_invest: cost × 1.5, wound_kind: Blunt, source: BurstMeridian }
        · 推 BurstMeridianEvent { skill: "beng_quan", caster, target, tick, overload_ratio: 1.5, integrity_snapshot }
        · 返回 CastResult::Started { cooldown_ticks: 60, anim_duration_ticks: 8 }
  - [ ] register_skills(&mut SkillRegistry)：注册 "burst_meridian.beng_quan" → resolve_beng_quan
  - [ ] cultivation/skill_registry.rs::init_registry() 末尾调 burst_meridian::register_skills（替换 hotbar plan P0 mock）
  - [ ] cultivation/known_techniques.rs（hotbar plan P1.a）的 stub 4 个 id 中确保 "burst_meridian.beng_quan" 是其中之一
  - [ ] cultivation/skill_registry.rs 静态 metadata（hotbar plan P1.a）写崩拳条目：
        description / required_realm: "引气" / required_meridians: [LI, SI, TE 任一] / qi_cost: 0.4 (ratio) /
        cast_ticks: 8 / cooldown_ticks: 60 / range: FIST_REACH / grade: "黄阶"
  - [ ] AttackSource enum 加 BurstMeridian variant（resolve_attack_intents 暂不分流，留 hook）
  - [ ] network/burst_event_emit.rs：emit_burst_meridian_payloads system（mirror vfx_event_emit）
  - [ ] schema/server_data.rs：BurstMeridianEventV1 构造 + Rust serde

Client：
  - [ ] animation/BurstMeridianAnimations.java：beng_quan 动画 keyframe
        rightArm: pitch -80→45 / z+0.3 / 8 tick easing
        torso: yaw +15° / 微抖
  - [ ] animation/BurstMeridianAnimationPlayer.java：listener 入口
        BongAnimationPlayer.play(caster, "bong:beng_quan", priority=1500, fadeTicks=2)
        粒子：沉重色 0xC58B3F，半径 0.5，8 tick
        命中目标时撒第二波（跟 AttackIntent resolve 联动 — 直接读 BurstMeridianEvent.target）
  - [ ] network/BurstMeridianEventHandler.java：payload 解码 → 派发到 player

测试（CLAUDE.md 饱和化清单 — 见 §13）：
  - [ ] resolve_beng_quan 12 条饱和 case（见 §13.1）
  - [ ] schema 双端对拍：BurstMeridianEventV1 happy + 全部 invalid
  - [ ] 集成：client 按 1 → server resolve_beng_quan → client 收到 BurstMeridianEvent → 动画播放
  - [ ] 回归：替换 hotbar plan P0 mock 后，hotbar plan P0 全部测试仍 100% 绿
  - [ ] 控制台命令 !burst beng_quan <target>（dev only）作 e2e 烟雾测试入口
```

### P1 · 4 招 + 完整虚脱期（目标：2-2.5 周）

```
  - [ ] 贴山靠 resolve_tie_shan_kao（任督扣减 + 打断 DefenseWindow + 击退 3 格 + 碰撞）
  - [ ] 血崩步 resolve_xue_beng_bu（双腿足三阳扣减 + 8 格瞬移 + 落地 SEVERED）
  - [ ] 逆脉护体 toggle 模式：
        · CastResult 加 ToggledOn / ToggledOff 两个 variant（与 hotbar plan 协调扩 enum）
        · combat/resolve.rs::resolve_attack_intents 受击钩子：检查 CombatState.nimai_active
        · CombatState.nimai_active: bool 字段（cultivation/components.rs）
        · 维持阈值检测：qi_current < 5 qi/s 自动 ToggledOff
  - [ ] BurstRecoveryState Component + 自愈 tick system（含 environment_qi 检测）
  - [ ] 4 招对应动画 keyframes（tie_shan_kao / xue_beng_bu / ni_mai_hu_ti / ran_ming 占位）
  - [ ] hotbar plan SkillBarHudPlanner 集成 MeridianStatusLight 子组件（绿/黄/红/灰）
  - [ ] HUD 虚脱期倒计时 + mirror_integrity 条 + BurstRecoveryStateV1 payload
  - [ ] cultivation/known_techniques.rs 扩到含 4 招（默认全已学，实际由 plan-skill 残卷决定）
  - [ ] 饱和测试：4 招各自 12 条 case + toggle 状态机 pin 测试
```

### P2 · 燃命（复用 qi_zero_decay）+ 染色加成 + 平衡调参（目标：1.5 周）

```
  - [ ] resolve_ran_ming：
        · AoE: 半径 3 格 AttackIntent，damage = qi_invest × 2.0
        · 全身 12 正经 integrity → 0.1 → TORN → SEVERED
        · 触发 qi_zero_decay::RealmRegressed（**复用，不重写跌境逻辑**）
        · BurstRecoveryState { window_ticks: 432000, realm_drop: true }
  - [ ] 沉重色加成（worldview §六.2）：overload_ratio += color_weight × 0.1
  - [ ] 战后虚脱被偷袭：BurstRecoveryState 期内受击 → 额外 MeridianCrack 概率 ×2
  - [ ] 药材加速恢复（凝脉草 ×10 / 固元丹 ×50）— 接 BurstRecoveryState.healing_rate
  - [ ] 平衡参数表落表（按境界过载上限 / integrity 扣减比例 / healing_rate 基线）
  - [ ] 克绝灵涡流：resolve_attack_intents 内若 source=BurstMeridian 则跳过 VortexField 抽干判定
  - [ ] 饱和测试：燃命独立测 + qi_zero_decay 联动测试 + 染色加成 boundary
```

### P3 · 死亡对齐（等待 plan-death-lifecycle）

```
  - [ ] 燃命跌境是否走「半死」状态还是完整 reboot？— 与 plan-death-lifecycle 联调
  - [ ] 与亡者博物馆时间戳对齐
  - [ ] 与 agent-v2 长线叙事（「有人燃命跌境」事件）对齐
```

---

## §12 开放问题

**已关闭（被 plan-hotbar-modify-v1 解决，本期不再讨论）**：
- [x] ~~QuickSlotBindings 扩展方案（enum 还是字符串前缀）~~ → hotbar plan 选 union（`SkillBarBindingV1.kind = "skill" | "item"`），本 plan 不再扩 QuickSlot
- [x] ~~F1–F5 固定 vs 自由排列~~ → 改走 1-9 SkillBar，玩家自由拖拽（hotbar plan §4.4）
- [x] ~~BurstMeridianEvent 走哪个 channel~~ → server-driven payload，复用现网 `bong:combat/realtime` + `network/burst_event_emit.rs` 推送（mirror `vfx_event_emit`）

**P0 必须明确（影响 resolve_beng_quan 实装）**：
- [ ] **崩拳的"攻击臂"选择策略**：硬编码右手 = 手三阳（LI/SI/TE），还是读 `Cultivation.handedness` 配置？P0 推荐硬编码右手（worldview / library 都默认右手），P1 加 handedness 字段
- [ ] **崩拳准入境界**：plan §5 数值梯度表里"醒灵 = 仅崩拳"——P0 是否允许醒灵期？推荐 P0 锁引气期门槛（避开醒灵期"5 真元小爆即死"边界），等数值平衡 P2 阶段再开放醒灵
- [ ] **target 必填 vs 可空**：崩拳是近身锁定还是范围扫击？推荐 P0 必填 target（无 target → CastRejectReason::InvalidTarget），与 worldview "零距贴脸"语义一致

**P1 才需要回答（不阻塞 P0）**：
- [ ] 逆脉护体 toggle 模式 fn 签名扩展：CastResult 加 `ToggledOn / ToggledOff`，还是另引一个 `SkillToggleResult` enum？
- [ ] 5 招互斥规则：维持逆脉护体时能否出崩拳？能否在血崩步落地 SEVERED 期间触发崩拳？（需要状态机表）
- [ ] 血崩步位移的碰撞：穿过实体？穿墙限制？

**P2 才需要回答**：
- [ ] 燃命跌境与 `qi_zero_decay::RealmRegressed` 协同：燃命主动触发 = 一次性写 RealmRegressed Event 让 qi_zero_decay 系统消费，还是新增独立路径？
- [ ] 沉重色加成：分招式（崩拳 vs 贴山靠）独立系数，还是统一倍率？
- [ ] 染色亲和与综合实力评估的关系（影响匹配/敌方 AI 评估）

**P3 才需要回答**：
- [ ] 燃命跌境走「半死」状态还是完整重生（需 plan-death-lifecycle 同步）

---

## §13 测试策略（CLAUDE.md 饱和化测试）

> 本章节为 P0 实施时的测试清单。所有 case 都必须把"目标行为"锁死，回归立刻撞红。后续 P1/P2 4 招按本章节模板写测试，避免"一招一种风格"。

### 13.1 resolve_beng_quan 饱和清单（P0 必填）

**Happy path**：
- [ ] 引气期 + 右臂手三阳全健康 + qi_current=100 + target 在 1.5m → `CastResult::Started { cooldown_ticks: 60, anim_duration_ticks: 8 }`；qi_current 减少 40；右臂三经 integrity 各 ×0.7；插了 Casting Component；EventWriter 写了 1 条 AttackIntent + 1 条 BurstMeridianEvent

**准入失败（每条 Rejected variant 一条）**：
- [ ] 醒灵期 → `Rejected { RealmTooLow }` + 无任何 mutation
- [ ] 右臂手三阳全 SEVERED → `Rejected { MeridianSevered }` + 无 mutation
- [ ] 右臂三经只有 1 条非 SEVERED（如只剩 LI）→ `Started`（单条经脉走通即可，不要求三条全健康）
- [ ] qi_current=10（< cost 40）→ `Rejected { QiInsufficient }` + 无 mutation
- [ ] target = None → `Rejected { InvalidTarget }`
- [ ] target 距离 = FIST_REACH + 0.01（边界外）→ `Rejected { InvalidTarget }`
- [ ] target 距离 = FIST_REACH（恰好边界内）→ `Started`
- [ ] cooldown_until_tick > now（来自上次出招）→ hotbar plan 在 handle_skill_bar_cast 已挡，但 server 兜底也要 `Rejected { OnCooldown }`

**原子性边界**：
- [ ] 准入校验后到 mutation 之间任意 panic → 全部 mutation 回滚（用 World scoped commit / panic catch 测试，确保 qi 不会"扣了但没出招"）

**数值精度**：
- [ ] qi_current=99.9（浮点边界）→ cost=39.96，qi 扣减后 = 59.94（不要求精确舍入到整数，但必须 deterministic）
- [ ] integrity=0.1（极低）→ ×0.7 后 = 0.07（不能 < 0 或 > 1）
- [ ] integrity=1.0 → ×0.7 后 = 0.7（不能因浮点误差 = 0.6999...9 触发后续判定 bug）

**事件副作用**：
- [ ] BurstMeridianEvent.overload_ratio = 1.5（P0 固定值）
- [ ] BurstMeridianEvent.integrity_snapshot = 攻击前三经平均 integrity（不是攻击后）
- [ ] AttackIntent.qi_invest = cost × overload_ratio = 40 × 1.5 = 60（不是 cost × 1.0）

### 13.2 schema 双端对拍

- [ ] BurstMeridianEventV1 happy sample：TS 编码 → Rust 解码 → 字段全等
- [ ] BurstMeridianEventV1 invalid samples：缺字段 / 多字段 / 类型错 / overload_ratio 负数 / tick 负数 → 双端都 reject
- [ ] sample 文件 `samples/burst_meridian_event_v1.json` 跟 schema 改动一起更新

### 13.3 集成测试（端到端链路）

- [ ] 玩家 A 按 1 → server resolve_beng_quan → client 收到 BurstMeridianEvent → 动画 player 播 beng_quan
- [ ] 命中后 client 收 CombatEvent（现网路径不动）
- [ ] cooldown 60 tick 内再按 1：client 蒙灰挡住 + server 兜底 reject
- [ ] **回归**：替换 hotbar plan P0 mock fn pointer 为 resolve_beng_quan 后，hotbar plan P0 全部 12 条饱和测试仍 100% 绿（这条最关键 — 证明接口契约没破）

### 13.4 P1+ 模板（参考用，本期不实装）

每招都按 §13.1 同款 12+ 条饱和清单写：happy / 每条 Rejected variant / 边界数值 / 原子性 / 事件副作用 / 状态机转换。逆脉护体 toggle 模式额外加状态机 pin（开 → 受击 → 关 / 开 → 维持阈值不足自动关 / 重复开 → no-op）。

---

## §14 进度日志

- 2026-04-26 初：骨架创建。书源 cultivation-0001，世界观 §五.1 + §四 + §六.2 已锁定。
- 2026-04-26：大幅展开——§3 改为复用现有快捷栏 F1–F9 系统（去掉轮盘方案），§3.2 设计 QuickSlotBindings 技能绑定扩展，§9 HUD 改为快捷栏技能槽渲染，§10 数据契约重写（不再新增 ClientRequestV1 变体），§11 P0 改为 QuickSlot 路由扩展路径。
- 2026-04-27：**对齐 plan-hotbar-modify-v1 + P0 收紧到仅崩拳**（项目里第一个真正落地的战斗功法示范）。
  - 头部：标 P0 范围 = 仅崩拳（引气期最低级），加测试方针引 CLAUDE.md 饱和化测试，删 F1-F5 + QuickSlotBind 路径声明。
  - §3 整段重写：3.0 与 hotbar plan 的契约（注册 fn pointer 替换 mock）/ 3.1 fn 签名 + CastResult enum（按 hotbar plan 锁定形态）/ 3.2 P0 崩拳完整路径 / 3.3 动画反馈（P0 仅崩拳一行）/ 3.4 P1+ 4 招蓝图保留备查。
  - §4 BurstRecoveryState：标 P0 用 `SkillBarBindings.cooldown_until_tick` 顶位，完整虚脱期下沉 P1。
  - §10 数据契约：删 handle_use_quick_slot 扩展 / QuickSlotBind 字符串前缀（已被 hotbar plan 接管）；server 新文件收紧到 `cultivation/burst_meridian.rs` + `network/burst_event_emit.rs` 两个；明示 `qi_zero_decay` 复用接口（燃命 P2 用）。
  - §11 实施节点：重排为 P0 崩拳单招 / P1 4 招 + 完整虚脱期 / P2 燃命（复用 qi_zero_decay）+ 染色 / P3 死亡对齐。每段标 fn 签名 / 文件清单 / 测试。
  - §12 开放问题：关闭 3 项（QuickSlot 扩展方案 / F-key 映射 / event channel）；新增 3 项 P0 必决（攻击臂选择 / 准入境界 / target 必填）；P1/P2/P3 各分组。
  - 新增 §13 测试策略：饱和化清单（resolve_beng_quan 12+ 条 case / schema 双端对拍 / 集成 e2e / 替换 mock 回归 = P0 必填模板）；P1+ 4 招按同款模板。

## Finish Evidence

- 2026-04-29：P0 `burst_meridian.beng_quan` 已落地为真实 SkillBar fn pointer 链路。
- 提交：`9d6100bf feat(schema): 增加爆脉崩拳事件契约`
- 提交：`9a3b4472 feat(server): 接通爆脉崩拳真实结算`
- 提交：`2cc83134 feat(client): 支持崩拳目标与表现`
- Server gate：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`，1690 passed。
- Agent schema gate：`npm test && npm run generate:check && npm run build`，194 passed，161 generated schemas fresh。
- Client gate：`JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 PATH="/usr/lib/jvm/java-17-openjdk-amd64/bin:$PATH" ./gradlew test build`，BUILD SUCCESSFUL。
