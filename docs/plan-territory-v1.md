# plan-territory-v1：领地博弈——灵脉零和 + 驻守/游牧抉择

> 好地方就那么几个，灵气用完就没了。你要霸占青云残峰还是游牧全图？霸占 = 稳定灵气 + 天道盯上 + 别人来抢；游牧 = 低调安全 + 灵气不够 + 没有根据地。没有正确答案——只有你愿意付多大代价。

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | ZoneInfluence 区域影响力系统（按累计停留/修炼/战斗自动计算） | ⬜ |
| P1 | 驻守收益与代价（灵气优先权 + 天道注意力加速 + NPC 态度） | ⬜ |
| P2 | 领地争夺机制（侵入/驱逐/灵脉占据的博弈规则） | ⬜ |
| P3 | 领地信息暴露（narration 广播 + NPC 传话 + 环境痕迹） | ⬜ |
| P4 | 饱和测试 | ⬜ |

---

## 接入面

### 进料

- `world::Zone` — spirit_qi / 区域面积 / 位置
- `cultivation::Cultivation` — 境界 / 真元（影响力权重因子）
- `world::tiandao_hunt::TiandaoAttention` — 天道注意力（驻守加速累积）
- `npc::brain` — NPC 对区域主人的态度
- `social::Renown` / `Identity` — 声望系统（影响 NPC 反应）
- `combat::Wounds` / `StatusEffects` — 战斗结算
- `persistence` — 影响力数据持久化（跨 session）

### 出料

- `world::Zone` — 写入 `dominant_player` / `influence_map`
- `npc::brain` — NPC 行为变化（尊敬/恐惧/投靠区域霸主）
- `network::redis_bridge` → `bong:agent_narrate` — 领地变动 narration
- `world::tiandao_hunt::TiandaoAttention` — 驻守行为加速注意力累积
- `network` → client HUD — 区域归属感环境效果

### 共享类型 / event

- **新增** `ZoneInfluence`（Zone 子 Component）— 按玩家记录的累计影响力
- **新增** `ZoneDominance` — 区域当前霸主信息
- **新增** `InfluenceChangedEvent` — 影响力变动事件（触发 NPC 反应 + narration）
- 复用 `TiandaoAttention` — 驻守修正因子
- 复用 `Renown` — 声望影响 NPC 态度

### 跨仓库契约

| 层 | 新增 symbol |
|----|------------|
| server | `ZoneInfluence` / `ZoneDominance` / `InfluenceChangedEvent` |
| server | `server/src/world/territory.rs` 模块 |
| server | `territory_tick()` system（每 60s 评估） |
| client | 区域进入时微弱环境提示（"此地有人经营"） |
| agent | narration 可感知领地变动（"青云残峰换主了"） |

### worldview 锚点

- §十 灵气是零和的："好地方是会被用完的，然后变成废地"——领地价值随灵气消耗自然衰减
- §十一 灵龛："灵龛不提供灵气——你不能在里面修炼，只能藏东西和养伤"——灵龛是家但不是灵气来源，必须外出占地
- §八 天道中等手段："对高消耗修士所在区域降低灵气"——驻守 = 高消耗 = 天道盯上
- §十五 #3："玩家之间是默认敌对的陌生人。但合作有时比对抗更理性"——领地可以独占也可以默契共享
- §七 NPC 散修行为："靠近时立刻评估你的威胁度"——领地主人影响 NPC 行为

### qi_physics 锚点

- 领地系统不修改灵气数值——灵气的消耗/再分配走现有 zone qi 系统
- 驻守者不"锁定"灵气——任何人都能吸该区域灵气，只是驻守者有社交优势

---

## 核心机制：无墙的领地

### 设计原则

> 末法残土没有"占旗"、"领地边界"、"圈地宣告"这种 MMORPG 概念。**领地 = 你在这里待得够久、打得够狠，NPC 和路人都认你是这块地的人。** 是**涌现的社会状态**，不是系统授权。

没有任何 UI 按钮让你"占领"一个区域。你做的事决定了你有没有影响力：

- 在这里打坐修炼 → 影响力 +
- 在这里战斗获胜 → 影响力 ++
- 在这里击杀其他玩家 → 影响力 +++
- 在这里采集资源 → 影响力 +
- **离开** → 影响力自然衰减

### ZoneInfluence

```rust
// server/src/world/territory.rs

#[derive(Component)]  // 附加到 Zone entity
pub struct ZoneInfluence {
    /// 每个玩家在此区域的影响力
    pub players: HashMap<Entity, PlayerInfluence>,
    /// 当前霸主（影响力最高且超过阈值的玩家）
    pub dominant: Option<ZoneDominance>,
}

pub struct PlayerInfluence {
    pub value: f64,            // 0.0-100.0
    pub last_activity_tick: u64,
    pub source_breakdown: InfluenceSources,
}

pub struct InfluenceSources {
    pub meditation_time: f64,   // 在此区域累计打坐时间
    pub combat_wins: u32,       // 在此区域战斗胜利次数
    pub player_kills: u32,      // 在此区域击杀玩家次数
    pub gather_count: u32,      // 在此区域采集次数
    pub continuous_days: u32,   // 连续在此区域活动的天数
}

pub struct ZoneDominance {
    pub player: Entity,
    pub influence: f64,
    pub established_tick: u64,  // 何时成为霸主
    pub public_known: bool,     // NPC 是否已知（传播需要时间）
}
```

### 影响力计算

```
每 60 秒评估一次：

累积：
  打坐中（在此 zone）      → +0.3/分钟
  战斗获胜（在此 zone）    → +2.0/次
  击杀玩家（在此 zone）    → +5.0/次（最强的宣示）
  采集资源                  → +0.1/次
  连续天数 bonus           → value × (1 + continuous_days × 0.05)

衰减：
  不在此 zone 时           → -0.5/分钟（离开就在失去）
  超过 24h 未回            → 衰减速率 ×3（-1.5/分钟，快速失去）

上限：100.0

霸主判定：
  influence ≥ 30 → 成为候选霸主
  候选中 influence 最高者 → 如果领先第二名 ≥ 10 → 确认为霸主
  霸主影响力 < 20 → 失去霸主地位（滞后防抖动）
```

---

## P1：驻守收益与代价

### 收益

**1. NPC 态度倾斜**

区域霸主享受该区域 NPC 的优待：

| 影响力 | NPC 态度 | 具体效果 |
|--------|---------|---------|
| < 30 | 无视 | 正常交互 |
| 30-50 | 认识你 | 交易不加价 + NPC 主动打招呼 |
| 50-70 | 敬畏 | 交易折扣 10% + NPC 提供区域情报（"今天有人从东边来过"） |
| 70+ | 臣服 | 交易折扣 20% + NPC **主动报告入侵者**（"北边来了个陌生人，气息在你之下"）|

**2. 灵气优先权（非机制性）**

霸主不"锁定"灵气——但 NPC 会**让路**。当霸主在此 zone 打坐时：
- NPC 散修 brain：`meditation_near_dominant_flee = 0.8`（NPC 自觉远离霸主打坐的区域，减少灵气竞争）
- 其他玩家想来打坐得自己面对霸主——纯 PVP 博弈，系统不干预

**3. 环境痕迹**

长期驻守的区域会有环境变化（纯视觉，client 渲染）：
- 灵龛 50 格内地面出现微弱灵纹（长期真元浸润的痕迹）
- 常走的路径地面略有不同纹理
- 此效果让**后来者知道"这里有人经营"**——信息暴露

### 代价

**1. 天道注意力加速**

驻守 = 固定在高灵气区 = 天道狩猎（plan-tiandao-hunt-v1）最容易触发的场景。

```
驻守中的 tiandao_hunt activity_factor 修正：
  连续在同一 zone 打坐 > 30 分钟 → activity_factor ×1.3
  连续在同一 zone > 2 小时       → activity_factor ×1.6
  连续在同一 zone > 6 小时       → activity_factor ×2.0
```

**高境驻守 = 加速被天道盯上**。通灵修士在一个地方坐 2 小时，天道注意力累积比游牧快 60%。

**2. 位置暴露**

- 霸主身份会被 NPC 传播（见 P3）——想低调不可能
- 环境痕迹暴露领地位置
- 战斗全服可见的天劫闪电暴露位置

**3. 灵气消耗**

你驻守的区域灵气会被你和你吸引来的 NPC 消耗殆尽。一个好地方被一个通灵修士霸占 2 天，灵气可能从 0.5 降到 0.2——你把自己的领地吃空了。

---

## P2：领地争夺

### 侵入与驱逐

没有"攻城"机制——你走进去就是侵入。博弈完全是人与人之间的：

**场景 1：新人进入老大哥的地盘**
```
新人走进青云残峰 → 霸主的 NPC 眼线通报 "北边来了人"
→ 霸主选择：无视 / 去看看 / 直接驱逐
→ 如果战斗发生在此 zone → 胜者 influence +5.0
→ 败者可能死亡 + influence 归零
```

**场景 2：两个同级修士争夺灵泉湿地**
```
双方都在此 zone 活动，influence 接近
→ NPC 两边都不站——"观望"
→ 某天一方在此 zone 击败另一方 → influence 拉开 +5.0
→ NPC 转向胜者
→ 败者可以留下（接受从属地位）或走人
```

**场景 3：低境联合驱逐高境**
```
3 个引气联合骚扰 1 个固元霸主
→ 固元打不过三个人一起上（或打得过但真元耗尽）
→ 固元被迫转移
→ 3 个引气分享 zone，各自 influence 较低但总和控制
```

### 影响力竞争规则

- 同一 zone 多人都有 influence → 谁高谁是霸主
- 击杀对方：自己 +5.0，对方**归零**（被杀 = 彻底失去此地话语权）
- 击败不杀（对方逃跑）：自己 +2.0，对方 -5.0
- **长期无冲突共存**：两人各 influence 40-50，差距 < 10 → 无霸主（共治/冷战状态）

---

## P3：领地信息暴露

### NPC 传话机制

当某 zone 有霸主（`public_known = true`）后，NPC 散修会在**跨 zone 移动时传播信息**：

```
NPC 从青云残峰移动到灵泉湿地 → 
  灵泉湿地的 NPC 聊天中出现：
    "听说青云残峰那边有个固元境的在。气息很稳，怕是驻下了。"

NPC 传话延迟：1-3 小时（game time）
NPC 传话范围：相邻 zone（不会跨越整个地图传）
NPC 传话精度：只传境界段 + 大致时间，不传名字（匿名系统）
```

### 天道 narration

天道会在以下时刻评论领地变动：
- 新霸主确立："青云残峰上，不知何时多了一位常客。灵草因此疏了。"
- 霸主被驱逐："青云残峰换了主人。旧主落荒东逃——未知生死。"
- 区域灵气耗尽："青云残峰之灵气……已不足以养活一株灵草了。占此地者，可还住得下去？"

### 环境痕迹（client 视觉）

| 影响力 | 痕迹 | 实现 |
|--------|------|------|
| 30+ | 灵龛周边地面微弱灵纹 | GroundDecal 粒子，淡色 |
| 50+ | 常走路径地面有脚印痕迹 | GroundDecal 粒子，更明显 |
| 70+ | 区域入口方向有微弱真元残留气味（该修士的染色色调） | Sprite 粒子，色调对应 QiColor |

---

## P4：饱和测试

### 影响力曲线

1. **累积速率**：固元修士连续打坐 60 分钟 → influence ≈ 18（+0.3/min）
2. **战斗加成**：在同 zone 赢 3 场 → influence +6（跳到 24）
3. **击杀加成**：击杀 1 名玩家 → influence +5（到 29，接近霸主）
4. **衰减速率**：离开 1 小时 → influence -30（快速失去）
5. **霸主确立**：influence ≥ 30 + 领先第二名 ≥ 10
6. **霸主失去**：influence < 20

### NPC 行为

7. **NPC 让路**：霸主打坐时 NPC 100 格内 flee → 减少灵气竞争
8. **NPC 报信**：influence 70+ → NPC 报告入侵者（延迟 ≤ 30s）
9. **NPC 传话**：跨 zone NPC 移动后目标 zone NPC 聊天中出现领地信息

### 与天道狩猎联动

10. **驻守加速**：同 zone 连续 2h → activity_factor ×1.6 验证
11. **领地+天道双压**：驻守灵泉湿地的通灵修士 → 1h 内同时到 Pressure（天道）+ influence 50（领地）
12. **灵气耗尽**：霸主驻守 + 天道 Watch 双重 qi drain → zone qi 加速归零

### 守恒断言

13. **influence 不影响灵气数值**——纯社交层面，灵气走现有 zone qi 系统
14. **NPC 行为变化不创造/消灭灵气**
15. **击杀 influence 零和**：A 杀 B → A +5, B 归 0（总量不守恒——这是故意的，击杀是毁灭性的）
