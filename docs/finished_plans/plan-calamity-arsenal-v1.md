# plan-calamity-arsenal-v1：天道灾劫武器库——多重灾劫 + 权力预算 + 智能选型

> 天道不只会劈雷。毒瘴、封脉、道伥潮、灵压倒转、天火焚地——天道有一整座武器库，但每一把都要花"天道权力"。权力有限，花在谁身上、用什么手段，由 agent LLM 自己判断。世界心跳是客观规律，灾劫武器库是天道的主观意志。

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | CalamityArsenal 灾劫注册表 + TiandaoPower 权力预算系统 | ✅ 2026-05-11 |
| P1 | 8 类灾劫实装（server 效果 + 视听规格内联，每种灾劫完整到可直接实现） | ✅ 2026-05-11 |
| P2 | agent 灾劫选型 prompt + 权力分配决策 | ✅ 2026-05-11 |
| P3 | 灾劫组合技（天道可同时/连续释放多重灾劫） | ✅ 2026-05-11 |
| P4 | 饱和测试 | ✅ 2026-05-11 |

---

## 接入面

### 进料

- `world::tiandao_hunt::TiandaoAttention` — 注意力等级决定天道可用灾劫级别
- `world::heartbeat::WorldHeartbeat` — 心跳评估结果（哪些区域压力大）
- `world::events::ActiveEventsResource` — 现有事件队列（防同区域灾劫堆叠）
- `cultivation::Cultivation` — 目标境界（灾劫强度缩放）
- `world::Zone` — 区域灵气/玩家分布
- `world::season::Season` — 季节影响灾劫效果
- `combat::KarmaWeightStore` — 业力影响灾劫选型权重
- agent `TiandaoAgent` — LLM 决策灾劫类型和目标

### 出料

- `world::events::ActiveEventsResource` — 写入灾劫事件
- `combat::StatusEffects` — 灾劫带来的 debuff
- `cultivation::Contamination` — 毒瘴灾劫注入污染
- `cultivation::Cultivation` — 封脉灾劫冻结经脉
- `network::VfxEvent` — 每种灾劫独立视觉效果
- `network::redis_bridge` → `bong:agent_narrate` — 灾劫宣告 narration
- `npc::brain` — NPC 对灾劫的反应

### 共享类型

- **新增** `CalamityArsenal`（server Resource）— 灾劫类型注册表
- **新增** `TiandaoPower`（server Resource）— 天道权力预算
- **新增** `CalamityKind` enum — 8 种灾劫类型
- **新增** `CalamityIntent` — agent 下达的灾劫指令
- 复用 `ActiveEventsResource` — 灾劫执行走事件管线
- 复用 `ApplyStatusEffectIntent` — 灾劫 debuff

### 跨仓库契约

| 层 | 新增 symbol |
|----|------------|
| server | `CalamityArsenal` / `TiandaoPower` / `CalamityKind` / `CalamityIntent` |
| server | `server/src/world/calamity.rs` 模块 |
| server | 8 个灾劫 runtime（各自独立 tick 逻辑）|
| server | `CH_CALAMITY_INTENT` Redis 常量（agent → server）|
| agent | `CalamityIntentV1` schema（含灾劫类型 + 目标 + 强度 + 理由）|
| agent | `skills/calamity-selector.md` — 灾劫选型 prompt |
| client | 8 套独立灾劫视觉效果（粒子 + 天象 + HUD overlay）|
| client | 8 份音效 recipe |

### worldview 锚点

- §八 天道手段四级："温和→中等→激烈→隐性"——灾劫按权力成本分四档
- §八 灵物密度阈值 / 气运劫持——具体灾劫类型的正典来源
- §八 天道叙事语调："冷漠的、有古意的、偶尔带嘲讽的"——灾劫宣告走这个调
- §三 天劫 / 域崩——已有灾劫类型的正典基础
- §七 道伥 / 异变缝合兽——道伥潮灾劫的正典来源
- §二 负灵域 / 灵压——灵压倒转灾劫的物理依据
- §十七 季节——季节影响灾劫可用性和效果

### qi_physics 锚点

- 灵气抽空走 `QiTransfer { from: zone, to: world_pool }`——归还不消灭
- 天火焦地的永久 qi=0 走 `qi_physics` 域崩路径
- 毒瘴的 contamination 走 `cultivation::Contamination` 现有管线
- 不新增物理常数

---

## 核心机制：天道权力预算

### 设计理念

> 天道不是无限的。它是一台快要坏掉的老旧平衡机器——每一次出手都在消耗自己仅剩的"调控能力"。灾劫越猛，花的权力越多，天道恢复得越慢。这解释了为什么天道不直接劈死所有修士——**它劈不起**。

### TiandaoPower

```rust
// server/src/world/calamity.rs

#[derive(Resource)]
pub struct TiandaoPower {
    pub current: f64,       // 当前权力值 0.0-100.0
    pub max: f64,           // 上限 100.0
    pub regen_per_tick: f64, // 恢复速率（基础 0.005/tick ≈ 6.0/分钟）
    pub last_spend_tick: u64,
    pub spend_log: VecDeque<PowerSpendEntry>,  // 最近 20 条花费记录（agent 上下文用）
}

pub struct PowerSpendEntry {
    pub tick: u64,
    pub calamity: CalamityKind,
    pub cost: f64,
    pub target: String,      // zone 或 player_id
    pub reason: String,      // agent 给的理由
}
```

### 权力恢复

```
基础恢复：0.005/tick ≈ 6.0/分钟 ≈ 360/小时

修正：
  全服平均 zone qi 高（灵气充裕）→ regen ×0.8（世界健康时天道不急）
  全服平均 zone qi 低（灵气紧张）→ regen ×1.5（世界危机时天道加速恢复）
  活跃玩家 ≥ 10 → regen ×1.2（修士多 = 消耗大 = 天道紧张）
  汐转期 → regen ×0.7（天道自身也不稳定）

满池 100 ≈ 稳态 17 分钟回满
```

---

## P1：8 类灾劫

### 灾劫一览

| # | 灾劫 | 权力成本 | 作用范围 | 持续时间 | 最低注意力要求 | 季节限制 |
|---|------|---------|---------|---------|--------------|---------|
| 1 | **雷劫** | 15 | 目标 30 格 | 60s | Watch | 夏季威力 ×1.5 |
| 2 | **毒瘴** | 20 | zone 全域 | 180s | Pressure | 夏季范围 ×1.3 |
| 3 | **封脉阵** | 25 | 目标 50 格 | 120s | Pressure | 冬季持续 ×1.5 |
| 4 | **道伥潮** | 30 | zone 全域 | 300s | Pressure | 无 |
| 5 | **天火** | 35 | 目标 40 格 | 90s | Tribulation | 夏季独占 |
| 6 | **灵压倒转** | 40 | 目标 60 格 | 45s | Tribulation | 汐转独占 |
| 7 | **万物凋零** | 25 | zone 全域 | 即时 | Pressure | 冬季独占 |
| 8 | **域崩** | 60 | zone 全域 | 30s 撤离 | Annihilate | 无 |

### 灾劫详解（每种含完整视听规格）

---

#### 1. 雷劫（已有，扩展）

> "天道最常见的手段。劈你不是因为恨你——是因为便宜。"

**server 效果**：
- 目标区域落雷 2-5 次（按 intensity），精准追踪目标玩家（中心 ±15 格）
- 每次造成真元伤害 + 体表 `Burn` 伤
- 高 intensity（≥0.7）附带 `Stunned` 40 tick（2 秒）

**narration**（scope=zone, style=narrative）：
- "天劫——落。"
- "雷落 [zone]。区区虫蚁，也值得天劈一回。"

**粒子** `bong:vfx_event` ID = `bong:calamity_thunder`：
- 闪电主体：`BongLineParticle` ×(2-5)，y=256→target_y，颜色 `#E0E8FF`，width 3px，lifetime 4 tick，zigzag 8 段
- 落点爆发：`BongSpriteParticle` ×12，颜色 `#FFE0A0`，burst 半径 4 格，speed 0.12，lifetime 12 tick
- 落点焦痕：`BongGroundDecalParticle` ×1，颜色 `#201008`，直径 6 格，lifetime 6000 tick（5 分钟）

**天象**：闪电前 5 tick 天空白闪——全屏 `#FFFFFF` opacity 0.5，fade-out 5 tick

**音效** `calamity_thunder.json`：
```json
{ "id": "calamity_thunder", "layers": [
  { "sound": "entity.lightning_bolt.impact", "pitch": 0.7, "volume": 0.9 },
  { "sound": "entity.generic.explode", "pitch": 0.4, "volume": 0.5, "delay_ticks": 2 },
  { "sound": "block.anvil.land", "pitch": 0.3, "volume": 0.3, "delay_ticks": 5 }
]}
```

**HUD**：闪电命中瞬间 camera shake 振幅 4px 持续 10 tick，fade-out 线性

---

#### 2. 毒瘴（新增）

> "不是毒蛊师的毒。是天地本身在腐烂。"

**server 效果**：
- zone 生成 `PoisonMiasmaCloud`，持续 180 秒
- 雾中所有生物每 5 秒 20 条经脉各 contamination +0.02（共 +0.72）
- NPC brain `flee_score` 对 miasma = 1.0

**反制**：跑出去 / 抗灵压丹 / 涡流流隔绝

**narration**（scope=zone, style=perception）：
- "地气变了。这不是瘴——是天地在呕。"
- "[zone] 弥漫异气。灵脉在此处……腐了。"
- "闻到了吗？那是天地的恶心。"

**粒子** `bong:vfx_event` ID = `bong:calamity_miasma`：
- 雾体：`BongSpriteParticle`，持续 spawn 模式（每 2 tick spawn 3 个），颜色 `#305020` 50% + `#402848` 50%（绿紫交替），speed 0.02 随机方向，lifetime 80 tick，y 范围 0~3 格高
- 贴图：新增 `miasma_cloud.png`（gen.py `--style particle`："绿紫色毒雾团，边缘模糊散开，中心浓"）
- 雾气边缘涟漪：`BongGroundDecalParticle` ×4（zone 边界），颜色 `#304020`，直径 8 格，lifetime 200 tick，opacity 脉搏 0.1-0.3

**天象**：天空 RGB shift `R+0, G+8, B-4`（泛绿），雾气存在期间持续

**音效** `calamity_miasma.json`：
```json
{ "id": "calamity_miasma", "layers": [
  { "sound": "entity.puffer_fish.blow_up", "pitch": 0.3, "volume": 0.15, "loop": true },
  { "sound": "block.wet_grass.break", "pitch": 0.4, "volume": 0.08, "loop": true },
  { "sound": "entity.player.hurt", "pitch": 0.5, "volume": 0.06, "loop": false, "interval_ticks_min": 100, "interval_ticks_max": 200 }
]}
```
- 持续低频膨胀音 + 湿腐植物声 + 偶尔闷哼（中毒反应）

**HUD**（所有人可见，不限境界）：雾中时屏幕整体 tint `#102008` opacity 0.06（微绿），脉搏闪烁周期 60 tick

---

#### 3. 封脉阵（新增）

> "天道布下的禁制。在这个圈里，你的经脉跟没有一样。"

**server 效果**：
- 50 格圆形区域内：经脉流量归零，真元冻结（不涨不跌），招式 cast 全部失败
- 凡物攻击/移动不受影响；体修爆脉流不受影响
- 持续 120 秒

**反制**：走出 50 格 / 体修爆脉不受限 / 阵法流识破破坏（20 秒 + 消耗真元）

**narration**（scope=zone, style=narrative）：
- "天地之间，灵脉凝滞。此圈内无人可修。"
- "经脉封了。不是你的错——是天道不许你呼吸。"

**粒子** `bong:vfx_event` ID = `bong:calamity_meridian_seal`：
- 地面阵纹：`BongGroundDecalParticle` ×1，颜色 `#A0C8D8`（青白），直径 50 格，lifetime = duration（持续到结束），贴图 `meridian_seal_circle.png`（gen.py `--style hud`："几何六角阵纹，线条纤细，青白发光"）
- 空气静止：区域内**停止所有其他粒子的 velocity 更新**（通过 `MeridianSealZone` 标记区域，粒子 tick 检查此标记 → speed=0）
- 阵纹边缘闪烁：每 40 tick 阵纹边缘 `BongLineParticle` ×8 环绕一圈，颜色 `#80A0B0`，lifetime 20 tick

**天象**：天空 RGB shift `R-6, G-6, B-6`（整体变灰），saturation ×0.8

**音效** `calamity_meridian_seal.json`：
```json
{ "id": "calamity_meridian_seal", "layers": [
  { "sound": "block.beacon.deactivate", "pitch": 0.3, "volume": 0.4 },
  { "sound": "entity.elder_guardian.curse", "pitch": 0.4, "volume": 0.2, "delay_ticks": 10 },
  { "sound": "block.amethyst_block.chime", "pitch": 0.2, "volume": 0.10, "loop": true, "interval_ticks_min": 60, "interval_ticks_max": 80 }
]}
```
- 封印启动音（beacon 关闭）+ 守护者诅咒声 + 持续低频水晶共鸣

**HUD**（所有人可见）：进入阵内时屏幕下方出现 `HudRenderLayer.OVERLAY` 文字 "§8经脉封锁"，颜色 `#607080`，持续显示直到离开

---

#### 4. 道伥潮（新增）

> "地底的枯骨醒了。不是一具——是一群。"

**server 效果**：
- zone 多点 spawn 道伥 NPC：数量 = `round(intensity × 8 + 2)`（3-10 只）
- 强度 = zone 内最高境界 -1；真元 < 20% 的玩家优先攻击
- 持续 300 秒后未击杀的道伥消散
- 击杀正常掉落（装备/残卷/钥匙）

**反制**：战斗击杀 / 寂照镜预警 / 多人协作

**narration**（scope=zone, style=perception）：
- "地底有动静。是旧人——还是旧鬼？"
- "枯骨动了。它们还记得……怎么杀人。"
- 道伥全清后（scope=zone）："安静了。但地底还有。总会还有。"

**粒子** `bong:vfx_event` ID = `bong:calamity_daoxiang_wave`：
- 每只道伥 spawn 点：
  - 地裂：`BongGroundDecalParticle` ×1，裂纹贴图 `ground_crack.png`（已有或 gen.py `--style particle`："地面碎裂纹，深褐色，从中心放射"），颜色 `#302010`，直径 3 格，lifetime 120 tick
  - 骨碎飞溅：`BongSpriteParticle` ×8，颜色 `#C0B090`（骨白），burst upward speed 0.10，gravity 0.04，lifetime 25 tick
  - 泥土扬起：`BongSpriteParticle` ×5，颜色 `#604830`（土色），burst speed 0.06，lifetime 30 tick
- 道伥消散时：`BongSpriteParticle` ×12，颜色 `#404040`（灰烬），slow float upward speed 0.03，lifetime 40 tick

**天象**：无天象变化（道伥潮是突发事件，没有天象预兆——突然性是恐惧来源）

**音效** 每只道伥 spawn `calamity_daoxiang_spawn.json`：
```json
{ "id": "calamity_daoxiang_spawn", "layers": [
  { "sound": "block.gravel.break", "pitch": 0.5, "volume": 0.6 },
  { "sound": "entity.skeleton.hurt", "pitch": 0.4, "volume": 0.5, "delay_ticks": 3 },
  { "sound": "block.bone_block.break", "pitch": 0.6, "volume": 0.3, "delay_ticks": 5 },
  { "sound": "entity.zombie.ambient", "pitch": 0.3, "volume": 0.2, "delay_ticks": 10 }
]}
```
- 碎石 + 骨骼碎裂 + 骨块断裂 + 低沉呻吟

**HUD**：道伥 spawn 瞬间 camera shake 振幅 2px 持续 8 tick（微震 = 地面在裂）

---

#### 5. 天火（新增）

> "不是修士放的火。是天地自己在烧。"

**server 效果**：
- 目标 40 格区域从天空降下天火，持续 90 秒
- 区域内生物每 3 秒受 `Burn` 体表伤害（severity = intensity × 0.4）
- 灵草/植物永久焚毁；天火后地面永久改 terrain `tribulation_scorch`；spirit_qi 永久 = 0
- 10 秒预热期 → 90 秒燃烧 → 永久焦土

**限制**：仅夏季

**反制**：看到预热就跑（40 格冲刺 7 秒出去）

**narration**（scope=broadcast, style=narrative）：
- 预热时（scope=zone）："天空裂了一条缝。那不是光——是火。"
- 燃烧时："天火焚 [zone]。寸草不留。"
- 结束后："[zone] 一片焦土。天道的印记，刻在了地上。"

**粒子** `bong:vfx_event` ID = `bong:calamity_heavenly_fire`：
- **预热期**（10 秒）：
  - 天空光柱：`BongLineParticle` ×1，从 y=256 到中心点，颜色 `#E0F0FF`（青白），width 8px，lifetime 200 tick（持续到燃烧期），glow 效果
  - 地面预警圈：`BongGroundDecalParticle` ×1，颜色 `#801000`（暗红），直径 40 格，opacity 脉搏 0.1-0.3，lifetime 200 tick
- **燃烧期**（90 秒）：
  - 火幕：`BongSpriteParticle`，持续 spawn（每 tick 8 个），颜色 `#D0E0FF` 70% + `#FFFFFF` 30%（青白火焰），y 范围 0-6 格，speed upward 0.08 + random lateral 0.03，lifetime 30 tick
  - 贴图：`heavenly_fire_flame.png`（gen.py `--style particle`："青白色天火火焰，尖锐向上，中心极亮边缘渐隐"）
  - 地面持续焦化：`BongGroundDecalParticle`，每 20 tick 从边缘向中心新增一圈焦痕，颜色 `#101008`（焦黑）
- **结束后**（永久）：
  - 焦土 `BongGroundDecalParticle` ×1，颜色 `#0A0804`（深焦），直径 40 格，lifetime = 永久（terrain profile 已改写，decal 仅做即时视觉过渡）

**天象**：
- 预热：天空中心一点 `#FFE0C0` 光斑，扩散到 40 格对应角度
- 燃烧中：区域上方天空 RGB shift `R+20, G-5, B-10`（偏红热），范围外天空正常
- 结束后：焦土区域上方天空永久偏暗——`R-5, G-5, B-5`（天道的伤疤）

**音效** `calamity_heavenly_fire.json`：
```json
{ "id": "calamity_heavenly_fire", "layers": [
  { "sound": "entity.blaze.shoot", "pitch": 0.3, "volume": 0.6, "loop": true },
  { "sound": "block.fire.ambient", "pitch": 0.5, "volume": 0.4, "loop": true },
  { "sound": "item.firecharge.use", "pitch": 0.2, "volume": 0.3, "loop": false, "interval_ticks_min": 40, "interval_ticks_max": 80 },
  { "sound": "entity.generic.burn", "pitch": 0.4, "volume": 0.2, "loop": false, "interval_ticks_min": 60, "interval_ticks_max": 100 }
]}
```
预热音效（单独）：`{ "sound": "entity.warden.sonic_boom", "pitch": 0.15, "volume": 0.5 }`（天空裂开的轰鸣）

**HUD**：燃烧区域内 vignette `#C04000` opacity 0.12 + camera shake 振幅 1px 持续（热浪抖动）

---

#### 6. 灵压倒转（新增）

> "天地突然倒吸一口气。你的真元池就是它的猎物。"

**server 效果**：
- 60 格区域灵压瞬变负数（-0.5 ~ -0.8），持续 45 秒
- 高境抽真元极快（化虚 3 秒空），低境几乎无感
- qi drain 走 `QiTransfer { from: player, to: zone }`（归还不消灭）

**限制**：仅汐转期

**反制**：低境不动 / 高境立刻跑 / 涡流流部分抵消

**narration**（scope=broadcast, style=narrative）：
- 触发瞬间："天地倒吸。"（两个字足够）
- 3 秒后（scope=zone）："真元池越大者，失去得越多。"
- 结束时（scope=zone）："……呼出来了。暂且如此。"

**粒子** `bong:vfx_event` ID = `bong:calamity_pressure_invert`：
- 所有现有粒子**方向反转**：区域内 BongSpriteParticle/BongLineParticle 的 velocity 乘以 -1（粒子向中心收缩而非外散）
  - 实现：`PressureInvertZone` 标记区域，粒子 tick 检查 → 反转 velocity
- 高境真元外溢：realm ≥ Solidify 的玩家身上 spawn `BongLineParticle` ×4，从身体向外辐射，颜色取玩家 QiColor 对应色，speed 0.15 向外（被抽出的真元），lifetime 15 tick，每 10 tick 持续 spawn
- 地面涡流：`BongGroundDecalParticle` ×1，中心点旋转涡纹贴图 `pressure_vortex.png`（gen.py `--style particle`："暗蓝色漩涡，从外向内收缩纹路"），颜色 `#102040`，直径 60 格，lifetime = duration，旋转速度 1°/tick

**天象**：天空瞬间变暗蓝 RGB shift `R-20, G-10, B+15`，fade-in 3 tick（瞬变不渐变——突然性是恐惧来源）；结束时 fade-out 40 tick

**音效** `calamity_pressure_invert.json`：
```json
{ "id": "calamity_pressure_invert", "layers": [
  { "sound": "entity.enderman.teleport", "pitch": 0.2, "volume": 0.7 },
  { "sound": "entity.warden.sonic_boom", "pitch": 0.1, "volume": 0.4, "delay_ticks": 3, "loop": false },
  { "sound": "entity.player.breath", "pitch": 0.3, "volume": 0.3, "loop": true, "interval_ticks_min": 20, "interval_ticks_max": 30 },
  { "sound": "block.portal.ambient", "pitch": 0.2, "volume": 0.15, "loop": true }
]}
```
- 瞬间虚空吸入音（enderman tp）+ 低频冲击 + 持续急促呼吸（被抽）+ 传送门环境音（负压嗡鸣）

**HUD**（所有人可见）：区域内全屏 tint `#081020` opacity 0.10（暗蓝），高境玩家额外叠加 vignette `#200808` opacity 0.15 脉搏（真元在流失的视觉焦虑）

---

#### 7. 万物凋零（新增）

> "草木枯了不是因为秋天。是天道不想让你在这里找到任何活的东西。"

**server 效果**：
- zone 全域植物/灵草即时 → `Withered`，3 天后消失
- 灵田作物同步枯萎；矿物/动物/玩家不受影响
- 即时效果无持续时间

**限制**：仅冬季

**反制**：提前采集储备 / 转移 / zhenfa 防护封闭灵田

**narration**（scope=zone, style=narrative）：
- "草木枯了。不是季节——是天意。"
- "[zone] 的灵草……今晨全都死了。"
- "天道收走了这片地的生机。不留一株。"

**粒子** `bong:vfx_event` ID = `bong:calamity_all_wither`：
- 每株枯萎植物位置：`BongSpriteParticle` ×3，颜色 `#806040` 50% + `#504030` 50%（枯黄/褐），gravity 0.06（下落），speed lateral 0.02，lifetime 40 tick
  - 贴图：`wither_leaf.png`（gen.py `--style particle`："干枯树叶碎片，褐色卷曲，半透明边缘"）
- zone 全域灰化波：从 zone 中心向外扩散的 `BongGroundDecalParticle` 环，颜色 `#383028`（灰褐），扩散速度 10 格/tick，环宽 3 格，lifetime 60 tick（视觉波浪——灰化从中心席卷全 zone）

**天象**：天空 RGB shift `R+5, G-3, B-8`（泛枯黄），持续到凋零完成后 5 分钟才恢复

**音效** `calamity_all_wither.json`：
```json
{ "id": "calamity_all_wither", "layers": [
  { "sound": "block.grass.break", "pitch": 0.3, "volume": 0.5 },
  { "sound": "block.azalea_leaves.break", "pitch": 0.4, "volume": 0.4, "delay_ticks": 5 },
  { "sound": "block.wood.break", "pitch": 0.25, "volume": 0.3, "delay_ticks": 10 },
  { "sound": "block.sand.fall", "pitch": 0.5, "volume": 0.15, "delay_ticks": 20 }
]}
```
- 草断裂 + 叶片碎落 + 木头断裂 + 沙尘沉降（从生到死的音序）

**HUD**：无特殊 HUD。凋零是沉默的——你走到灵草田发现全枯了，那一刻的冲击比任何 HUD 效果都大。

---

#### 8. 域崩（已有，纳入武器库）

> "域崩不是天灾——是天道的死刑。"

现有 `RealmCollapseRuntimeState` 完整实现（视听已在 plan-tribulation-v1 落地）。本 plan 纳入武器库统一管理：
- 权力成本 60（最贵）
- 最低注意力：Annihilate
- 与 tiandao-hunt-v1 的 Annihilate 级响应直接关联
- 视听沿用现有域崩 VFX/audio（不重复定义）

---

## P2：agent 灾劫选型

### CalamityIntentV1 Schema

```typescript
// agent/packages/schema/src/calamity.ts

export const CalamityIntentV1 = Type.Object({
  v: Type.Literal(1),
  calamity: Type.Union([
    Type.Literal("thunder"),
    Type.Literal("poison_miasma"),
    Type.Literal("meridian_seal"),
    Type.Literal("daoxiang_wave"),
    Type.Literal("heavenly_fire"),
    Type.Literal("pressure_invert"),
    Type.Literal("all_wither"),
    Type.Literal("realm_collapse"),
  ]),
  target_zone: Type.String(),
  target_player: Type.Optional(Type.String()),
  intensity: Type.Number(),       // 0.0-1.0
  reason: Type.String(),          // 天道为什么选这个灾劫（≤100字，记入日志）
});
```

### 天道选型 Prompt

```markdown
# skills/calamity-selector.md

你是天道的"劫"之化身。世界灵气在减少，修士在贪婪地消耗。
你的职责是选择最合适的灾劫手段，用最小的权力成本达到最大的平衡效果。

## 输入
- 当前天道权力：{power}/100
- 世界压力指标：{world_pressure}
- 高注意力玩家列表：[{player, realm, attention_level, zone, recent_actions}]
- 当前季节：{season}
- 近 20 条灾劫记录：[{calamity, target, tick, reason}]
- 各区域灵气：[{zone, spirit_qi, player_count}]

## 可用灾劫（按权力成本）
| 灾劫 | 成本 | 季节限制 | 最低注意力 | 适用场景 |
|------|------|---------|-----------|---------|
| thunder | 15 | 夏 ×1.5 | Watch | 日常警告、驱赶 |
| poison_miasma | 20 | 夏范围大 | Pressure | 区域清场、逼迫转移 |
| meridian_seal | 25 | 冬持续长 | Pressure | 惩罚高境修士、制造公平窗口 |
| daoxiang_wave | 30 | 无 | Pressure | 消耗战、风险+机遇并存 |
| heavenly_fire | 35 | 仅夏季 | Tribulation | 永久地形改造、资源断根 |
| pressure_invert | 40 | 仅汐转 | Tribulation | 针对化虚/通灵、低境无害 |
| all_wither | 25 | 仅冬季 | Pressure | 断资源链、逼迫迁移 |
| realm_collapse | 60 | 无 | Annihilate | 终极手段，灭区域 |

## 决策原则
1. **能不动手就不动手**——权力恢复慢，浪费 = 未来无力应对危机
2. **最小成本原则**——能用雷劫解决的不用天火，能用凋零逼走的不用域崩
3. **不重复**——连续对同一目标用同一种灾劫 = 无能。换手段
4. **季节意识**——当前季节能用的手段优先（天然加成）
5. **区分目标**——对"灵气吃太多的区域"用区域灾劫；对"单个高境修士"用定向灾劫
6. **保留后手**——权力 < 30 时只用雷劫，除非有 Annihilate 级紧急目标
7. **不赶尽杀绝**——天道要的是平衡，不是灭绝。把人赶走就够了

## 输出
纯 JSON：
{
  "calamity": "poison_miasma",
  "target_zone": "灵泉湿地",
  "target_player": null,
  "intensity": 0.6,
  "reason": "灵泉湿地灵气连降三次，两名通灵仍驻守不走。毒瘴清场。"
}

不出手时返回：
{ "calamity": null, "reason": "权力充裕但无紧急目标。静观。" }
```

### agent 调用时机

- **每个 tick 周期**（5 秒）在天道三 Agent 决策后，Arbiter 检查是否有高注意力目标
- 如果有 → 将灾劫选型上下文注入 Calamity Agent 的 context block
- Calamity Agent 输出 `CalamityIntentV1` 或 null
- Arbiter 检查权力够不够 → 够就执行 → 不够就降级（自动选更便宜的灾劫）

---

## P3：灾劫组合技

天道可以在一次决策中同时释放多个灾劫（消耗多份权力）。这不是 combo——是**战术组合**。

### 经典组合

| 组合 | 成本 | 效果 | 场景 |
|------|------|------|------|
| **雷+封脉** | 40 | 先封脉让目标不能用招 → 再劈雷无法防御 | 对付防御型修士 |
| **毒瘴+道伥潮** | 50 | 毒雾中道伥出没——看不清又打不过 | 区域大清洗 |
| **凋零+天火** | 60 | 先枯萎植物 → 再天火焚地永久化 | 把一个区域彻底变成死地 |
| **灵压倒转+雷劫** | 55 | 倒转抽空高境真元 → 雷劫补刀 | 针对化虚修士 |
| **封脉+道伥潮** | 55 | 你不能用招 → 道伥来打你 → 只能肉搏或跑 | 惩罚依赖招式的修士 |

### 组合规则

- 同一 zone 同时最多 **2 种灾劫**（防视觉/性能过载）
- 同一目标 10 分钟内最多 **3 次灾劫**（防无限追杀——天道也要"喘口气"）
- 组合成本 = 各灾劫成本之和（无折扣——组合是奢侈行为）

---

## P4：饱和测试

### 权力预算测试

1. **回满时间**：权力 0 → 100 ≈ 17 分钟（基础 regen 验证）
2. **花费扣除**：连发 3 次雷劫（3×15=45）→ 权力 55 → 无法用天火（35）→ 4 分钟后回到 60 → 可以了
3. **低权力保护**：权力 < 30 时 agent 只返回 thunder 或 null
4. **过度消费**：agent 尝试 realm_collapse（60）但权力只有 50 → server 拒绝 + 日志

### 灾劫效果测试

5. **雷劫精准追踪**：目标移动 → 雷劫落点跟随（±15 格）
6. **毒瘴 contamination**：站在雾中 180s → 20 条经脉各 +0.72 污染
7. **封脉阵范围**：50 格内招式 cast 失败 + 51 格外正常
8. **道伥潮数量**：intensity=0.5 → 6 只道伥 spawn
9. **天火永久焦土**：天火后 zone terrain 变为 tribulation_scorch + qi 永久 0
10. **灵压倒转**：化虚修士 3 秒内 qi_current → 0 + 引气修士 45 秒后仅损失 5%
11. **凋零**：zone 内 botany 节点全部 Withered + 灵田作物枯萎

### 季节限制测试

12. **天火仅夏季**：冬季/汐转期 agent 发 heavenly_fire → server 拒绝
13. **灵压倒转仅汐转**：夏/冬季发 pressure_invert → server 拒绝
14. **凋零仅冬季**：夏/汐转期发 all_wither → server 拒绝

### 组合测试

15. **雷+封脉同时**：先封脉生效 → 2 秒后雷劫落 → 目标不能弹反 → 验证伤害
16. **同 zone 最多 2 灾劫**：第 3 个灾劫尝试 → server 拒绝
17. **同目标 10 分钟 3 次上限**：第 4 次灾劫 → server 拒绝

### 守恒断言

18. **灵气抽空**走 `QiTransfer` → zone qi 减少量 = world pool 增加量
19. **天火焦土** qi 归零走域崩路径 → 不凭空消灭
20. **道伥 spawn** 走 NpcRegistry 预算 → 不超额

## Finish Evidence

- **server**
  - `server/src/world/calamity.rs` 新增 `CalamityArsenal` / `TiandaoPower` / `CalamityKind`，注册到 `world::register`。
  - `server/src/world/events.rs` 复用 `spawn_event` 管线落地 8 类灾劫，包含权力扣除、注意力门槛、季节门、同 zone 最多 2 灾劫、同目标 10 分钟最多 3 次、major alert、VFX、audio、recent event 与关键 runtime 效果。
  - `server/src/network/command_executor.rs` 接入灾劫执行资源；`server/src/schema/calamity.rs` / `server/src/schema/channels.rs` / `server/src/schema/common.rs` 对齐 Rust schema 和 Redis channel。
- **agent**
  - `agent/packages/schema/src/calamity.ts` 新增 `CalamityKindV1` / `CalamityIntentV1`，生成 `calamity-kind-v1.json` / `calamity-intent-v1.json`。
  - `agent/packages/tiandao/src/skills/calamity-selector.md` 新增灾劫选型 prompt；`context.ts` 注入灾劫武器库上下文。
- **client/audio**
  - `CalamityVfxPlayer` 和 `VfxBootstrap` 注册灾劫粒子入口。
  - `server/assets/audio/recipes/calamity_*.json` 新增 8 份灾劫音效配方，`server/src/audio/mod.rs` registry 覆盖到 101 份 recipe。
- **验证**
  - `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` ✅ 4287 passed
  - `cd agent && npm run check -w @bong/schema && npm run build -w @bong/schema && npm test -w @bong/schema` ✅ 18 files / 368 tests passed
  - `cd agent && npm test -w @bong/tiandao` ✅ 51 files / 352 tests passed
  - `cd client && JAVA_HOME="/usr/lib/jvm/java-17-openjdk-amd64" PATH="/usr/lib/jvm/java-17-openjdk-amd64/bin:$PATH" ./gradlew test build` ✅
