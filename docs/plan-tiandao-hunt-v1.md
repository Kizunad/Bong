# plan-tiandao-hunt-v1：天道狩猎——高境日常就是被追杀

> 境界越高，天道越想杀你。不是一次性天劫——是持续、升级、逼你不断移动的**注意力追踪系统**。固元开始感受到压力，通灵每天在跑路，化虚的存在本身就是一场战争。"强大不是终点，是新的危险的开始。"

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | TiandaoAttention 注意力系统（per-player 累积/衰减/阈值） | ⬜ |
| P1 | 四级天道响应链（微调→施压→天劫→灭绝）自动触发 | ⬜ |
| P2 | 反制玩法（欺天阵/游牧打坐/负灵域躲避/低境伪装） | ⬜ |
| P3 | 天道叙事接入（agent 对高注意力玩家产出个性化 narration） | ⬜ |
| P4 | HUD 环境感知（天意压迫感——非数字，是氛围） | ⬜ |
| P5 | 饱和测试（境界缩放 + 注意力衰减 + 反制有效性 + 守恒） | ⬜ |

---

## 接入面

### 进料

- `cultivation::Cultivation` — realm / qi_current / qi_max（注意力权重主因）
- `world::Zone` — spirit_qi（玩家所在区域灵气浓度影响注意力累积速率）
- `world::heartbeat::WorldHeartbeat` — 事件节拍（天道响应走心跳管线投放）
- `world::events::ActiveEventsResource` — 写入定向事件
- `combat::KarmaWeightStore` — 业力权重（叠加注意力）
- `world::season::Season` — 汐转期注意力累积 ×1.5
- `tribulation` — 天劫系统（第三级响应接入）
- `worldgen::pseudo_vein` — 伪灵脉系统（第一级响应：降低玩家区域灵气）
- `npc::brain` — NPC 对高注意力玩家的行为变化

### 出料

- `world::events::ActiveEventsResource` — 定向事件（针对特定玩家的天灾）
- `network::redis_bridge` → `bong:agent_narrate` — 天道对高注意力玩家的个性化叙事
- `combat::StatusEffects` — "天道注视"状态效果
- `network` → client HUD — 压迫感环境渲染
- `npc::brain` — NPC 远离高注意力玩家（"连 NPC 都怕你了"）

### 共享类型 / event

- **新增** `TiandaoAttention`（server Component）— 每个玩家的天道注意力值
- **新增** `TiandaoResponseLevel` enum — None/Watch/Pressure/Tribulation/Annihilate
- **新增** `TiandaoHuntTick` system — 每 10 秒评估注意力 + 触发响应
- 复用 `ActiveEventsResource.enqueue_*` — 定向事件投放
- 复用 `KarmaWeightStore` — 注意力与业力叠加

### 跨仓库契约

| 层 | 新增 symbol |
|----|------------|
| server | `TiandaoAttention` Component / `TiandaoResponseLevel` enum |
| server | `server/src/world/tiandao_hunt.rs` 模块 |
| server | `tiandao_hunt_tick()` system（FixedUpdate） |
| server | `TiandaoAttentionSnapshot` S2C payload（同步到客户端做环境渲染）|
| client | `TiandaoPresenceHudPlanner` — 天意压迫感 HUD 效果 |
| agent | `skills/tiandao-hunt-narration.md` — 高注意力玩家专属叙事 prompt |
| agent | `TiandaoHuntNarrationRuntime` — 监听 `bong:tiandao_hunt_narration_request` |

### worldview 锚点

- §八 天道的唯一目标："让这个世界多活一天"——高境修士是最大的灵气消耗源
- §八 天道手段四级：温和→中等→激烈→隐性——直接映射为四级响应
- §八 灵物密度阈值：高灵气聚集触发天道注视——注意力系统的物理依据
- §八 气运劫持：劫气标记暗调负面概率——注意力系统的正典实现
- §十一 危机分层：弱者/中等/强者三档危机——注意力按境界缩放
- §十五 #1："强大不是终点，是新的危险的开始"——本 plan 的设计纲领
- §三 化虚描述："天道会主动针对化虚修士——因为它们消耗太多灵气"

### qi_physics 锚点

- 天道响应第一级"区域灵气微调"走 `qi_physics` 的 zone qi 再分配——不新增衰减公式
- 天道响应不凭空消灭灵气——所有 qi 操作走 `QiTransfer` 守恒

---

## 核心机制：天道注意力

### TiandaoAttention 组件

```rust
// server/src/world/tiandao_hunt.rs

#[derive(Component)]
pub struct TiandaoAttention {
    pub level: f64,              // 0.0-100.0 连续值
    pub response: TiandaoResponseLevel,  // 离散阶段
    pub last_eval_tick: u64,
    pub accumulation_rate: f64,  // 当前累积速率（每 tick）
    pub peak_level: f64,         // 历史最高值（影响衰减速度）
}

#[derive(Clone, Copy, PartialEq)]
pub enum TiandaoResponseLevel {
    None,          // 0-15：天道未注意到你
    Watch,         // 15-40：天道在观察（微调灵气）
    Pressure,      // 40-70：天道在施压（刷怪 + 降灵气 + narration 暗示）
    Tribulation,   // 70-90：天道降劫（定向天灾）
    Annihilate,    // 90+：天道要你死（连续天劫 + 域崩 + 全服广播）
}
```

### 注意力累积

```
每 10 秒评估一次：

基础累积 = realm_base_rate × zone_qi_factor × activity_factor × season_factor

realm_base_rate（境界基础速率——核心标尺）：
  醒灵：   0.0（天道不理你）
  引气：   0.0（天道不理你）
  凝脉：   0.01/评估（≈ 0.06/分钟，要 4 小时才到 Watch）
  固元：   0.05/评估（≈ 0.3/分钟，50 分钟到 Watch）
  通灵：   0.15/评估（≈ 0.9/分钟，17 分钟到 Watch）
  化虚：   0.40/评估（≈ 2.4/分钟，6 分钟到 Watch，29 分钟到 Tribulation）

zone_qi_factor（你在高灵气区 = 吃天道的肉 = 注意力涨更快）：
  zone.spirit_qi ≤ 0.1 → ×0.3（死域里天道懒得看你）
  zone.spirit_qi 0.1-0.3 → ×0.6
  zone.spirit_qi 0.3-0.6 → ×1.0
  zone.spirit_qi 0.6+     → ×1.8（好地方天道盯得紧）

activity_factor（你在干什么）：
  打坐修炼中    → ×1.5（吸灵气 = 最招天道恨的事）
  战斗中        → ×1.2（打斗溅射灵气）
  移动/探索     → ×0.8（在走就没在吸）
  站立不动      → ×1.0
  在灵龛内      → ×0.5（低调）

season_factor：
  夏/冬 → ×1.0
  汐转 → ×1.5（天道节律紊乱，更敏感）
```

### 注意力衰减

```
自然衰减：
  每 10 秒评估时：level -= decay_rate

decay_rate（天道"遗忘"你的速度）：
  当前 response = None       → 0.08/评估（≈ 29 分钟从 15 降到 0）
  当前 response = Watch      → 0.05/评估（天道还在看，忘得慢）
  当前 response = Pressure   → 0.03/评估（天道已经上手了，很难脱身）
  当前 response = Tribulation → 0.01/评估（天道铁了心，基本不降）
  当前 response = Annihilate  → 0.00（不降——直到你死或跑进负灵域）

加速衰减（反制手段，→ P2）：
  在死域（qi=0）停留     → decay ×3.0（天道在死域里看不清你）
  在负灵域（qi<0）停留   → decay ×5.0（负压遮蔽）+ 但你自己也在掉真元
  使用欺天阵             → 注意力转移到阵上，你本体 decay ×4.0
  降低境界（主动或被动） → 重算 realm_base_rate（降了就是降了）
```

### 阈值与滞后

```
升级阈值（上行）：
  None → Watch：       level ≥ 15
  Watch → Pressure：   level ≥ 40
  Pressure → Tribulation：level ≥ 70
  Tribulation → Annihilate：level ≥ 90

降级阈值（下行，有滞后防反复跳动）：
  Annihilate → Tribulation：level < 80（滞后 10）
  Tribulation → Pressure：  level < 60（滞后 10）
  Pressure → Watch：        level < 30（滞后 10）
  Watch → None：            level < 10（滞后 5）
```

---

## P1：四级天道响应

### Level 0 — None（注意力 0-15）

天道没看见你。前三境修士长期停在这里。

### Level 1 — Watch（注意力 15-40）

> "天道还不动手，但它在看。"

**效果**（自动，无需 agent 决策）：
- 玩家所在区域 spirit_qi **缓慢下降**：每分钟 -0.01（不走 WorldHeartbeat 事件管线——直接修改 zone qi，走 QiTransfer 归还世界）
- 玩家附近 50 格内 NPC 散修开始**不安**：brain 评分中 `flee_score += 0.2`（NPC 感觉到天道在看这个方向）
- 天道 narration 概率触发（每 5 分钟 30% 概率）：zone 广播一句阴阳怪气的话
  - "此间灵脉又薄了几分。仍有人在此贪恋。"
  - "青云残峰上，不知何人吞吐不休。"

**玩家体感**：感觉灵气在变差，NPC 开始躲你。老玩家看到 narration 就知道——该挪窝了。

### Level 2 — Pressure（注意力 40-70）

> "天道开始出手了，但还留着余地。"

**效果**（自动 + agent 可叠加）：
- **zone qi 下降加速**：每分钟 -0.03
- **定向刷怪**：每 3 分钟在玩家 100 格内刷 1-2 只异变兽（走 NpcRegistry 预算）
  - 兽的强度 = 玩家境界 -1（固元碰凝脉级兽，通灵碰固元级兽——不会秒杀但很烦）
- **灵物密度检查**：玩家背包灵物权重 > 阈值 → 该区块所有灵物灵气 -10%（§八 灵物密度阈值）
- **narration 频率提升**：每 3 分钟 50% 概率，措辞更直接
  - "尔在此处吞噬几何？天地薄了，你可知？"
  - "此地异变兽闻到了高浓真元。它们来了。"
- **HUD**：屏幕边缘开始出现极淡的暗红 vignette（几乎看不出——但时间久了会注意到）

**玩家体感**：灵气快速下降、怪开始来找你、天道在点你名。必须决定：扛着继续修炼还是跑。

### Level 3 — Tribulation（注意力 70-90）

> "天道来真的了。"

**效果**：
- **定向天劫**：每 5 分钟一次雷劫（走现有 ThunderRuntimeState），强度 = 0.6 + (level-70)/40
  - 范围：玩家为中心 30 格
  - 附带 narration 全服广播："天劫降于 [区域名]。"——**所有人知道你在哪**
- **zone qi 骤降**：每分钟 -0.08（很快变死域）
- **NPC 全面逃离**：100 格内 NPC flee_score = 1.0（无条件跑）
- **灵物密度强制清零**：该区块灵物灵气全部归零（§八 正典）
- **定向刷道伥**：天劫落点有 20% 概率刷 1 只道伥（天劫劈死的前人遗骸被激活）
- **HUD**：暗红 vignette 加重 + 持续低频轰鸣音效 + 屏幕微颤

**玩家体感**：5 分钟一劫，灵气暴跌，全服知道你在哪——其他玩家可能来截杀。扛得住就继续（说不定突破了），扛不住就跑。

### Level 4 — Annihilate（注意力 90+）

> "天道要你从这个世界消失。"

**效果**：
- **连续天劫**：每 2 分钟一次，强度 = 0.9+
- **域崩预警**：该区域进入域崩倒计时（30 秒撤离窗口——走现有 RealmCollapseRuntimeState）
- **全服广播 narration**：
  - "天道之怒降于 [区域]。[区域] 将崩。诸修速避。"
  - "[某人] 的气息已触怒天地。这不是试炼——这是驱逐。"
- **注意力不衰减**——直到玩家死亡或逃入负灵域
- **HUD**：全屏暗红闪烁 + 裂纹纹理 + 雷鸣不断

**玩家体感**：世界在崩塌，全服都知道因为你。**这是化虚修士的日常**——他们的存在消耗太多灵气，天道每天都想赶走他们。唯一的出路是跑进负灵域（但高境在负灵域掉真元更快），或者死一次降境降注意力。

**设计意图**：化虚不是"我终于到顶了可以享受"——化虚是"我站在天地之间最高处，而天道要把我推下去"。每一天都是生存战。

---

## P2：反制玩法

### 欺天阵（§八 正典 "伪造天命"）

- 用阵法材料（灵木 + 骨币 + 兽骨）在远离自身 500+ 格处搭建假人阵
- 假人阵广播高灵气权重 → 天道注意力被吸过去
- 自身 decay ×4.0（快速脱离高注意力）
- **代价**：
  - 材料消耗（高境修士依赖低境帮忙搜集阵法材料——§经济循环）
  - 假阵持续 30 分钟后自毁（材料不可回收）
  - 天道有 10% 概率识破（识破后注意力 +20 惩罚——§八"天道不傻"）

### 游牧打坐

- 不在一个区域停超过 15 分钟 → 注意力累积会被移动中的低 activity_factor 抵消
- 高境修士的日常：打坐 10 分钟（在高灵气区回真元）→ 跑路 5 分钟（注意力衰减）→ 换一个区域打坐
- **感知**：通灵修士有"感知天道注意力"能力（§三 正典）——HUD 上能看到压迫感程度

### 负灵域躲避

- 跑进负灵域 → 天道在负数区无法索敌（§二 正典）→ 注意力 decay ×5.0
- **代价**：高境在负灵域掉真元极快——化虚修士在 -0.5 负灵域撑不过 3 分钟
- **战术**：短时间冲入负灵域甩掉天道注视（10 秒就够 decay 一大截），然后冲回来
- 这与坍缩渊玩法对称：TSY 深层对低境安全但高境致命——负灵域反制同理

### 降境求生（极端手段）

- 主动消耗真元到 0 → 触发经脉萎缩 → 降一阶 → realm_base_rate 大幅下降
- 从通灵降到固元：accumulation 从 0.15 降到 0.05——注意力增长变为 1/3
- **正典依据**：§三 "境界可以掉落"——这不是 bug，是设计好的逃生口
- **代价**：降境重回来要几十小时修炼

---

## P3：天道叙事接入

### agent 专属 narration runtime

高注意力玩家触发专属叙事生成——不是通用天道 narration，是**针对你个人的冷漠审判**。

```typescript
// agent/packages/tiandao/src/tiandao-hunt-narration-runtime.ts

// 监听 bong:tiandao_hunt_narration_request
// 请求包含：
{
  character_id: string,
  realm: string,
  attention_level: number,
  response_level: "watch" | "pressure" | "tribulation" | "annihilate",
  zone: string,
  recent_actions: string[],  // 近期行为摘要
  narration_count: number,   // 本轮注意力内已说过几句
}

// prompt 要求：
// - 措辞随 response_level 升级：暗示 → 警告 → 宣判 → 驱逐令
// - 不重复——参考 narration_count 避免说过的话
// - 保持天道的冷漠语气（§八 正典语调）
// - 可以点名也可以不点名（随 attention 升高越来越直接）
```

### 叙事升级示例

```
Watch:
  "灵泉湿地之灵脉，今又薄了几分。仍有贪者在此。"（不点名，暗示）

Pressure:
  "有人在青云残峰吞噬灵气如饮水。天地记下了。"（半点名）

Tribulation:
  "天劫降于血谷。[散修]，你吞吐几何？"（点名 + 天劫预告）

Annihilate:
  "[散修] 的气息已触怒天地。血谷将崩。诸修速避。"（全服广播 + 域崩预告）
  "天道不容。"（最终宣判——只有三个字）
```

---

## P4：HUD 环境感知

### 天意压迫感（不是数字，是氛围）

通灵修士能"感知天道注意力"（§三），但不是一个数字——是环境变化：

| ResponseLevel | 视觉 | 音效 | 体感 |
|------|------|------|------|
| None | 正常 | 正常 | — |
| Watch | 屏幕极淡暗红 vignette（opacity 0.03） | 偶尔远处低频嗡鸣 | "好像有什么在看我" |
| Pressure | vignette 加深（opacity 0.08）+ 天空色温偏冷 | 持续低频压迫音 + 偶尔雷声 | "天在变" |
| Tribulation | vignette 明显（opacity 0.15）+ 屏幕微颤 + 天空裂纹 | 雷鸣 + 风声加剧 | "来了" |
| Annihilate | 全屏暗红脉搏闪烁 + 持续裂纹 + 画面色彩饱和度降低 | 连续雷鸣 + 低频轰鸣 | "逃" |

**关键**：低境修士（凝脉以下）**看不到这些效果**——他们没有感知天道的能力。但他们能看到**天劫的闪电、听到雷声、看到 narration**——间接知道附近有人被天道盯上了。

### 实现

```java
// client: TiandaoPresenceHudPlanner.java
// 监听 TiandaoAttentionSnapshot payload
// 根据 response_level 输出 HudRenderCommand:
//   - HudRenderLayer.EFFECT: vignette overlay (暗红渐变)
//   - HudRenderLayer.EFFECT: screen shake (low amplitude)
//   - AudioRecipePlayer: ambient_tiandao_pressure (layered)
// 只对 realm ≥ Spirit 的玩家渲染
```

---

## P5：饱和测试

### 注意力曲线测试

1. **醒灵/引气**：24h 在高灵气区打坐 → 注意力 = 0（始终 None）
2. **凝脉**：4h 连续打坐 → 注意力刚过 15（Watch 边缘）→ 跑路 30 分钟 → 回 0
3. **固元**：50 分钟到 Watch → 继续 2h 到 Pressure → 移动 15 分钟降回 Watch
4. **通灵**：17 分钟到 Watch → 45 分钟到 Pressure → 1.5h 到 Tribulation
5. **化虚**：6 分钟到 Watch → 29 分钟到 Tribulation → 38 分钟到 Annihilate

### 响应链测试

6. **Watch 级**：zone qi 每分钟 -0.01 验证 + NPC flee_score 提升
7. **Pressure 级**：异变兽 3 分钟 spawn 验证 + 灵物清零触发
8. **Tribulation 级**：5 分钟雷劫验证 + 全服 narration + 道伥 spawn 概率
9. **Annihilate 级**：域崩触发验证 + 注意力不衰减 + 死亡后降回

### 反制测试

10. **欺天阵**：搭建后自身 decay ×4 验证 + 天道 10% 识破惩罚
11. **游牧打坐**：10 分钟打坐+5 分钟跑路循环 → 注意力稳定在 Watch 不升 Pressure
12. **负灵域躲避**：冲入 -0.3 负灵域 10 秒 → 注意力下降验证
13. **降境求生**：通灵主动降固元 → 累积速率从 0.15 降到 0.05

### 守恒断言

14. **Watch 级 zone qi 变化**走 `QiTransfer`——灵气归还世界不消失
15. **Tribulation 级刷怪**走 NpcRegistry 预算——不超额
16. **Annihilate 域崩**走现有 RealmCollapse qi 再分配——全服 QI 守恒
