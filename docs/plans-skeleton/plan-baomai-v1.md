# Bong · plan-baomai-v1

**体修·爆脉流**（攻击）。零距离贴脸，主动撕裂经脉换取超额真元流量瞬时灌入敌经脉。"经脉裂了可以养，命丢了养不了。"

**世界观锚点**：`worldview.md §五.1 体修/爆脉流` · `worldview.md §四.过载撕裂` · `worldview.md §六.2 沉重色`

**library 锚点**：`cultivation-0001 爆脉流正法`（唯一详写功法蓝本，5 招齐全）· `cultivation-0002 烬灰子内观笔记`（缚论"心镜龟裂"提供物理框架）· `ecology-0002 末法药材十七种`（养脉资源链）

**交叉引用**：`plan-cultivation-v1` · `plan-combat-no_ui` · `plan-combat-ui_impl` · `plan-player-animation-v1` · `plan-skill-v1` · `plan-armor-v1`（逆脉护体走 armor 反向）· `plan-HUD-v1` · `plan-hotbar-modify-v1`（1–9 技能栏）

---

## §0 设计轴心

- [ ] 爆脉 = **主动让心镜龟裂**换取一瞬高于境界的真元投影
- [ ] 5 招对应 5 个"龟裂深度档"，代价从浅到镜身崩
- [ ] 克绝灵涡流（不外放）；克所有"减损"防御（求损者无视减损）
- [ ] 末法约束：境界跌落，逆转条件极苛刻（高境养伤材料著者三十七年凑不齐）

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

### 3.1 复用现有快捷栏系统

**已有基础设施**（plan-HUD-v1 §2.2 + §4 + §11.3，已实装）：
- **上层 F1–F9**：主动技能快捷使用栏，玩家在 InspectScreen 拖拽配置
- `ClientRequestV1::UseQuickSlot { v: 1, slot: 0..8 }` 按 F-key 时发送
- `ClientRequestV1::QuickSlotBind { v: 1, slot: 0..8, item_id? }` 配置绑定
- Server 侧 `QuickSlotBindings` Component 存储每槽绑定 → `handle_use_quick_slot` 消费
- Cast time / cooldown 已有完整的 `Casting` Component + `tick_casts` system

**爆脉五式接入方式**：玩家在 InspectScreen「已学功法」tab 中把 5 招拖到 F1–F5 槽位，战斗中直接按 F-key 出招。

```
InspectScreen 配置：
  拖「崩拳」→ F1
  拖「贴山靠」→ F2
  拖「血崩步」→ F3
  拖「逆脉护体」→ F4
  拖「燃命」→ F5

战斗中：手指不离开 WASD，按 F1 = 崩拳、按 F2 = 贴山靠 ……
```

**优势 vs 轮盘**：不占用鼠标（瞄准不断）、练成肌肉记忆后盲按、可自定义排列。

### 3.2 QuickSlotBindings 扩展：技能绑定

**现状**：`QuickSlotBindings` 目前仅存储 `Option<u64>`（物品 instance_id），只支持物品类快速使用。

**扩展**：增加 skill binding 类型。每槽可以是「物品 instance_id」或「技能 skill_id」：

```rust
// server/src/player/state.rs QuickSlotBindings 扩展
pub struct QuickSlotBinding {
    pub kind: QuickSlotBindingKind,
}

pub enum QuickSlotBindingKind {
    Item { instance_id: u64 },
    Skill { skill_id: String },  // e.g. "burst_meridian.beng_quan"
}
```

```typescript
// agent/packages/schema/src/client-request.ts QuickSlotBind 扩展
// item_id 字段语义扩展：
//   "it:<instance_id>"  → 物品绑定（现有）
//   "sk:burst_meridian.beng_quan" → 技能绑定（新增）
```

### 3.3 客户端发送 → 服务端路由

```
玩家按 F1 → client 发 { type: "use_quick_slot", slot: 0 }
  ↓
server handle_use_quick_slot：
  查 QuickSlotBindings[0] → QuickSlotBindingKind::Skill { skill_id: "burst_meridian.beng_quan" }
  ↓
路由到 burst_meridian consumer（不复用物品 cast 逻辑）：
  1. 查 attacker entity
  2. 查 Cultivation：境界 ≥ 崩拳准入（醒灵可）
  3. 查 MeridianSystem：攻击臂经脉（手三阴/三阳）未 SEVERED
  4. 查 BurstRecoveryState：不在虚脱期
  5. 计算 qi_invest = qi_current × 0.4
  6. 检查 qi_current ≥ qi_invest
  7. 通过 → 写入 BurstMeridianIntent Event
```

**关键区别**：爆脉技不走物品 cast time（不需要读 template 的 cast_duration_ms）。每招有自己的"前摇时间"（动画 tick 决定的自然延迟），由 server 在 BurstMeridianEvent 中标注 anim_duration_ticks，客户端播放动画期间锁定输入。

### 3.4 客户端快捷栏渲染

**扩展 `QuickUseSlotRenderer`**（已存在，render F1–F9 槽内容）：
- 物品槽：渲染物品图标 + 右下角数量（现有逻辑）
- 技能槽（新增）：渲染技能图标 + 右下角经脉 integrity 状态指示灯
  - 绿灯（integrity > 0.8）：可用
  - 黄灯（0.4–0.8）：可用但风险
  - 红灯（< 0.4 或虚脱期）：不可用
  - 灰灯（境界不足 / 经脉 SEVERED）：锁定

**cast bar 复用**：按 F-key 后，槽位下方渲染 cast bar（现有逻辑复用），时长 = anim_duration_ticks × 50ms。

### 3.3 服务端结算（新 system）

**新增 `server/src/combat/burst_meridian.rs`**，注册为一个 ECS system：

```rust
pub fn resolve_burst_meridian_intents(
    clock: Res<CombatClock>,
    mut intents: EventReader<BurstMeridianIntent>,
    mut attack_intents: EventWriter<AttackIntent>,
    mut cultivations: Query<&mut Cultivation>,
    mut meridians: Query<&mut MeridianSystem>,
    mut wounds: Query<&mut Wounds>,
    mut recovery_states: Query<&mut BurstRecoveryState>,
    mut combat_events: EventWriter<CombatEvent>,
    mut burst_events: EventWriter<BurstMeridianEvent>, // 新的 server→client 通知
    positions: Query<&Position>,
    clients: Query<(Entity, &Username, &Position), With<Client>>,
) {
    for intent in intents.read() {
        match intent.skill {
            BurstMeridianSkill::BengQuan => resolve_beng_quan(...),
            BurstMeridianSkill::TieShanKao => resolve_tie_shan_kao(...),
            BurstMeridianSkill::XueBengBu => resolve_xue_beng_bu(...),
            BurstMeridianSkill::NiMaiHuTi => toggle_ni_mai_hu_ti(...),
            BurstMeridianSkill::RanMing => resolve_ran_ming(...),
        }
    }
}
```

**各招式结算逻辑**：

#### 崩拳（BengQuan）
```
1. 选攻击臂经脉：按常用手（右手 → 手三阴 LI/SI/TE），默认右手
2. 该臂经脉 integrity ×= 0.7（扣 30%），写 MeridianCrack
3. qi_invest = qi_current × 0.4（取当前真元 40%）
4. 过载倍率 = 1 + (1.0 - avg_integrity) × 1.0 （integrity 越低越猛）
5. 发 AttackIntent { target, reach: FIST_REACH, qi_invest: qi_invest × 过载倍率, wound_kind: Blunt }
6. 若目标身上有 DefenseWindow / VortexField → 检查「克绝灵涡流」特殊规则
```

#### 贴山靠（TieShanKao）
```
1. 任督二脉 integrity ×= 0.5（扣 50%），写 MeridianCrack
2. qi_invest = qi_current × 0.7
3. 过载倍率 = 同上
4. 发 AttackIntent { reach: FIST_REACH（零距），qi_invest × 过载倍率 }
5. 命中后：重置目标的 DefenseWindow（打断施法）+ 击退目标 3 格
   （击退 = 目标 Position 沿攻击方向偏移 3.0，检查碰撞）
```

#### 血崩步（XueBengBu）
```
1. 双腿足三阳 (ST/BL/GB) integrity ×= 0.4（扣 60%），写 MeridianCrack
2. qi_invest = qi_current × 0.65
3. 瞬间位移：attacker Position 沿面朝方向 +8.0 格（检查碰撞、不可穿墙）
4. 落地后：双腿 wounds 各加 SEVERED（severity = 1.0, wound_kind = Internal）
5. 设置 BurstRecoveryState { window_ticks: 72000（~1 游戏日 = 1 real hour）, legs_severed: true }
```

#### 逆脉护体（NiMaiHuTi）
```
1. 此为 toggle 姿态，非一次性出招
2. 开启：消耗 qi_invest（qi_current × 0.25），设置 CombatState.nimai_active = true
3. 受击时自动触发（在 resolve_attack_intents 中判定）：
   - 受击点附近经脉 integrity ×= 0.85（扣 15%）
   - 污染 = 0（异种真元就地中和）
   - 体表 wound +1 档（"以血保真元"）
4. 关闭：玩家再次按 toggle 或 qi_current < 维持阈值（5 qi/s）
```

#### 燃命（RanMing）
```
1. 全身 12 正经 integrity 全部 → 0.1（镜身崩），逐条写 MeridianCrack { severity: 0.9 }
2. qi_invest = qi_current（全量）
3. AoE AttackIntent：半径 3 格内所有实体
   - damage = qi_invest × 2.0（自毁式超额倍率）
   - contam_delta = qi_invest（全部灌入）
4. 自身：境界跌落一阶（Realm 降级，qi_max 重算）
5. 自身：全部正经 TORN → SEVERED
6. 设置 BurstRecoveryState { window_ticks: 432000（~6 游戏日）, realm_drop: true }
```

### 3.4 服务端 → 客户端：动画 + 反馈通知

**新增 `BurstMeridianEvent`**（server→client 推送）：

```typescript
// agent/packages/schema/src/server-data.ts 新增
export const BurstMeridianEventV1 = Type.Object({
  v: Type.Literal(1),
  type: Type.Literal("burst_meridian_event"),
  skill: Type.String(),           // "beng_quan" / "tie_shan_kao" / ...
  caster: Type.String(),          // canonical_player_id
  target: Type.Optional(Type.String()),
  tick: Type.Integer(),
  // 动画参数：客户端用这些微调动画幅度
  overload_ratio: Type.Number(),  // 过载倍率（影响动画力度）
  integrity_snapshot: Type.Number(), // 涉及经脉平均 integrity（影响受伤动画）
}, { additionalProperties: false });
```

**客户端收到 `BurstMeridianEventV1`**：
1. 播放对应的 PlayerAnimator 动画（见 §3.5）
2. 粒子效果：沉重色（古铜）真元爆发粒子 + 经脉龟裂音效
3. HUD 更新：mirror_integrity 条闪红一下

### 3.5 动画集成（与 plan-player-animation-v1 对齐）

**动画注册**（client/src/main/java/.../BurstMeridianAnimations.java）：

| 招式 | animation_id | duration | 关键帧要点 | priority |
|---|---|---|---|---|
| 崩拳 | `beng_quan` | 8 tick (0.4s) | rightArm: pitch -80→45, z 前推 0.3; torso: yaw 微转 15° | 1500 |
| 贴山靠 | `tie_shan_kao` | 12 tick (0.6s) | torso: pitch 前倾 30°, z 前冲 0.5; bothArm: 收拢→顶出 | 1500 |
| 血崩步 | `xue_beng_bu` | 6 tick (0.3s) | bothLeg: pitch 瞬蹲→弹直; body: z 爆发前冲 1.5; 落地: bothLeg 瘫软 | 1500 |
| 逆脉护体 | `ni_mai_hu_ti` | 持续姿态 | bothArm: 交叉胸前; 真元流光循环; 受击时额外 4 tick 震荡 | 600（持久姿态优先低）|
| 燃命 | `ran_ming` | 20 tick (1.0s) | 全身骨骼震颤抖动; torso: pitch 仰天; 所有 limb 炸开; 粒子爆发 | 3000（不可打断）|

**播放路径**：`BongAnimationPlayer.play(player, Identifier.of("bong", anim_id), priority, 2)`

---

## §4 经脉虚脱与恢复系统

### 4.1 BurstRecoveryState（新 Component）

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

### 10.1 Server 端新文件

| 文件 | 内容 |
|---|---|
| `server/src/cultivation/burst_meridian.rs` | `BurstMeridianSkill` 枚举、`BurstMeridianIntent` Event、`BurstRecoveryState` Component、`resolve_burst_meridian_intents` system、自愈 tick system |
| `server/src/combat/burst_handler.rs` | 各招式结算函数（beng_quan / tie_shan_kao / xue_beng_bu / ni_mai_hu_ti / ran_ming）|

### 10.2 Server 端修改文件

| 文件 | 改动 |
|---|---|
| `server/src/combat/events.rs` | 新增 `BurstMeridianIntent` Event |
| `server/src/combat/mod.rs` | 注册 `BurstMeridianIntent` event + `resolve_burst_meridian_intents` system |
| `server/src/combat/resolve.rs` | 逆脉护体：在 `resolve_attack_intents` 目标受击时检查 `CombatState.nimai_active`，触发皮下对冲 |
| `server/src/network/client_request_handler.rs` | `handle_use_quick_slot`：当 binding 类型为 Skill 时，路由到 burst_meridian consumer（不走物品 cast 逻辑）|
| `server/src/player/state.rs` | `QuickSlotBindings` 扩展：binding 类型支持 Skill { skill_id }；新增 quick_slot 序列化/反序列化适配 |
| `server/src/schema/server_data.rs` | 新增 `BurstMeridianEventV1` 构造 + channel 推送 |
| `server/src/cultivation/components.rs` | `CombatState` 新增 `nimai_active: bool` 字段 |

### 10.3 Schema 端

| 文件 | 改动 |
|---|---|
| `agent/packages/schema/src/client-request.ts` | `QuickSlotBind.item_id` 字段语义扩展：支持 `"sk:burst_meridian.beng_quan"` 前缀 |
| `agent/packages/schema/src/server-data.ts` | 新增 `BurstMeridianEventV1` + `BurstRecoveryStateV1` |

### 10.4 Client 端新文件

| 文件 | 内容 |
|---|---|
| `client/.../animation/BurstMeridianAnimations.java` | 5 招动画 Java 定义（路径 A）|
| `client/.../animation/BurstMeridianAnimationPlayer.java` | 接收 `BurstMeridianEvent` → 播放对应动画 + 沉重色古铜粒子 |
| `client/.../store/BurstRecoveryStore.java` | 存储 `BurstRecoveryStateV1`，驱动 HUD 倒计时 |
| `client/.../hud/MirrorIntegrityHudPlanner.java` | inspect 经脉层 integrity 百分比覆盖 |

### 10.5 Client 端修改文件

| 文件 | 改动 |
|---|---|
| `client/.../hud/QuickUseSlotRenderer.java` | 技能槽渲染：技能图标 + 经脉状态指示灯（绿/黄/红/灰）|
| `client/.../screen/InspectScreen.java` | 「已学功法」tab 支持拖拽技能 → F1–F9 快捷栏（发 `QuickSlotBind { item_id: "sk:..." }`）|

---

## §11 实施节点（从客户端到服务端完整链路）

### P0 · 单招贯通（崩拳起手，目标：1 周）

```
Schema：
  - [ ] BurstMeridianEventV1 + BurstRecoveryStateV1 TypeBox 定义 + Rust serde 对齐
  - [ ] QuickSlotBind.item_id 语义扩展文档（sk: 前缀）
Server：
  - [ ] BurstMeridianSkill 枚举 + BurstMeridianIntent Event
  - [ ] BurstRecoveryState Component + 自愈 tick system（基础版）
  - [ ] resolve_beng_quan：选臂经脉 integrity 扣减 × 过载倍率 → AttackIntent
  - [ ] QuickSlotBindings 扩展：支持 Skill { skill_id } binding
  - [ ] handle_use_quick_slot 路由扩展：遇到 sk:burst_meridian.* → 走 burst handler
  - [ ] BurstMeridianEvent 推送（bong:combat/realtime channel 复用）
客户端：
  - [ ] 崩拳动画 1 帧（rightArm 前推 + torso 微转，8 tick）
  - [ ] BurstMeridianAnimationPlayer：接收 BurstMeridianEvent → 播放动画 + 沉重色粒子
  - [ ] InspectScreen「已学功法」tab：显示 5 招（崩拳可用，其余灰掉）
  - [ ] QuickUseSlotRenderer：技能槽渲染（崩拳图标 + integrity 状态灯）
测试：
  - [ ] !burst beng_quan <target> 调试命令直通 server
  - [ ] 单元：崩拳后臂经脉 integrity 确实 < 1.0
  - [ ] 集成：玩家 A 按 F1 → BurstMeridianEvent → 玩家 B 受到超额 qi_invest AttackIntent
```

### P1 · 五招齐全 + 虚脱系统（目标：2 周）

```
  - [ ] 贴山靠（打断 DefenseWindow + 击退 3 格）
  - [ ] 血崩步（位移 + 双腿 SEVERED）
  - [ ] 逆脉护体（toggle 姿态 + resolve 侧皮下对冲）
  - [ ] 燃命（AoE + 跌境 + 全身 TORN/SEVERED）
  - [ ] BurstRecoveryState 完整自愈 tick（含 environment_qi 检测）
  - [ ] 4 招对应动画（贴山靠/血崩步/逆脉护体/燃命）
  - [ ] QuickUseSlotRenderer 技能槽状态灯（绿/黄/红/灰）+ cast bar 复用
  - [ ] InspectScreen 拖拽技能 → F 槽绑定（发 QuickSlotBind { item_id: "sk:..." }）
  - [ ] HUD：虚脱倒计时、mirror_integrity 条
```

### P2 · 染色加成 + 平衡调参（目标：1 周）

```
  - [ ] 沉重色加成：过载倍率 + (color_weight × 0.1)
  - [ ] 战后虚脱被偷袭检测（虚脱期内受击 → 额外 MeridianCrack 概率）
  - [ ] 药材加速恢复（凝脉草/固元丹 接入 BurstRecovery healing_rate）
  - [ ] 平衡参数表（境界过载上限、integrity 扣减比例、healing_rate 基线）
```

### P3 · 燃命跌境完整路径 + 死亡对齐（等待 plan-death-lifecycle）

```
  - [ ] 燃命跌境是否走「半死」状态还是完整 reboot？
  - [ ] 与亡者博物馆时间戳对齐
  - [ ] 与 agent-v2 长线叙事（「有人燃命跌境」事件）对齐
```

---

## §12 开放问题

- [ ] 燃命跌境是否走完整重生流程？还是单独"半死"状态？
- [ ] 沉重色加成幅度是否要分招式（崩拳 vs 贴山靠）？
- [ ] 逆脉护体作为被动反应——是否需要玩家主动 toggle？还是自动检测受击？
- [ ] 5 招的接触判定：崩拳/贴山靠用现有 FIST_REACH；血崩步 0 格；燃命 AoE 半径 3
- [ ] 克绝灵流：在 AttackIntent resolve 中，若 intent 来自 BurstMeridian → 跳过 VortexField qi 抽干判定
- [ ] QuickSlotBindings 扩展方案：是用 `QuickSlotBindingKind` enum（侵入 QuickSlotBindings 结构），还是用 `item_id` 字符串前缀 `sk:` 约定（最小侵入）？
- [ ] 血崩步位移的碰撞检测——是否能穿过实体？穿墙限制？
- [ ] BurstMeridianEvent 走哪个 channel？`bong:combat/realtime` 还是新建 `bong:combat/burst`？
- [ ] F1–F5 固定映射 vs 玩家自由排列 —— 建议自由排列（同现有物品快捷栏逻辑），每槽可绑任意招

---

## §13 进度日志

- 2026-04-26 初：骨架创建。书源 cultivation-0001，世界观 §五.1 + §四 + §六.2 已锁定。
- 2026-04-26：大幅展开——§3 改为复用现有快捷栏 F1–F9 系统（去掉轮盘方案），§3.2 设计 QuickSlotBindings 技能绑定扩展，§9 HUD 改为快捷栏技能槽渲染，§10 数据契约重写（不再新增 ClientRequestV1 变体），§11 P0 改为 QuickSlot 路由扩展路径。
