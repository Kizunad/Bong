# plan-woliu-path-v1 设计稿

## Context

涡流流 v1-v4 已全量落地（10 招、153 测试），但缺少**深度路径扩展**——没有丹道那种"累积效应 → 不可逆变化 → 极端化身"的第二维度。本 plan 填补这个空白：**虚蚀路径**，让长期涡流修习者的身体灵压渐移为负值，解锁 5 个虚蚀专属招式 + 忘音台地形 + 观主残魂 NPC + 反噬死亡螺旋经济。

核心身份：**以身为虚，空即是我**（对偶丹道「以身试药，药即是我」）。

---

## 一、世界观锚点

- `worldview.md §五:442-444` — 涡流核心：掌心负灵域、真空吸扯、时机错误反噬永久残疾
- `worldview.md §五:467` — 涡流 Primary Axis：真元流速 + 池效率（持久博弈型）
- `worldview.md §五:472` — "算计型"防御，非爆发
- `worldview.md §五:547-548` — 缜密色 + 任督二脉 + 识规则顿悟 = 涡流大师（Δ +0.2）
- `worldview.md §二:44-56` — 负灵域灵压差物理：高境修士在负压区反而更脆弱
- `worldview.md §十二:1010` — 续命成瘾者原型（涡流极端 = "吸灵成瘾者"）
- `worldview.md §四:260-290` — 经脉 20 条 × 4 档损伤 + 污染
- **新增 worldview §五.2 虚蚀（Void Erosion）** — 待写入

**library 锚点**：
- `docs/library/geography/geography-0004.json` 北荒坍缩渊记
- `cultivation-0004` 涡流散人手札
- 新增：`cultivation-XXXX 静虚观覆灭志`

---

## 二、接入面 Checklist

- **进料**：
  - `cultivation::Cultivation { qi_current, qi_max, realm }` — 境界判定、qi 消耗
  - `cultivation::contamination::ContamSource { meridian_id: Some(Lung/Heart) }` — 定向经脉污染
  - `cultivation::meridian::MeridianSystem` — 肺经/心经完整度检查
  - `cultivation::meridian::severed::SkillMeridianDependencies` — Lung+Heart 依赖
  - `cultivation::insight::InsightTrigger` — 虚蚀阶段顿悟选择
  - `combat::woliu_v2::*` — 10 个已有招式、VortexCastEvent、TurbulenceField、PassiveVortex
  - `qi_physics::collision::qi_negative_field_drain_ratio()` — 负压吸取公式
  - `qi_physics::constants::VORTEX_*` — 涡流常数
  - `world::zone::Zone.spirit_qi` — zone 灵压读数

- **出料**：
  - `combat::woliu_v2::erosion::VoidErosion` — 虚蚀组件（新增）
  - `combat::woliu_v2::erosion::VoidErosionStage` — 4 阶段 enum
  - 5 新招式注册 SkillRegistry（`woliu.ambient_vortex` / `woliu.void_vortex` / `woliu.swallowing_vortex` / `woliu.vortex_echo` / `woliu.void_core`）
  - `VoidErosionAdvanceEvent` → InsightTrigger
  - `schema::woliu_erosion::VoidErosionStateV1` / `VoidErosionEventV1` IPC payload
  - Client: 虚蚀视觉（半透明、回响粒子、声音扭曲 HUD）
  - Agent: 天道 narration 虚蚀事件

- **共享类型/event**：
  - **复用** `VortexCastEvent` / `TurbulenceField`（虚涡 = 负压版 TurbulenceField）
  - **复用** `ContamSource { meridian_id }` 定向污染
  - **复用** `QiTransfer` 守恒律
  - **复用** `PassiveVortex` 切换模式（常驻涡流复用此 pattern）
  - **复用** `InsightTrigger` 顿悟链路
  - **新增** `VoidErosion` component + `VoidErosionStage` enum + `VoidErosionAdvanceEvent`
  - **新增** `StatusEffectKind::VoidCoreActive` — 虚心 3s 无敌态

- **跨仓库契约**：
  - server: `server/src/combat/woliu_v2/erosion.rs` 新模块
  - agent: `bong:void_erosion_event` narration
  - client: 虚蚀视觉渲染 + HUD 面板 + 声音扭曲
  - schema: `VoidErosionStateV1` / `VoidErosionEventV1` IPC payload

- **worldview 锚点**：§五 涡流核心 / §二 负灵域物理 / §十二 成瘾者原型 / **新增 §五.2 虚蚀**

- **qi_physics 锚点**：
  - `qi_physics::collision::qi_negative_field_drain_ratio()` — 常驻涡流 zone 抽取
  - `qi_physics::ledger::QiTransfer` — 所有 qi 流动走守恒
  - **不新增物理常数** — 虚蚀 contamination rate 是 VoidErosion 内部状态

---

## 三、§0 阶段总览

| 阶段 | 内容 | 状态 |
|---|---|---|
| **P-1** | contamination 系统扩展——`meridian_id` 定向裂痕路由 | ⬜ |
| **P0** | 虚蚀底盘——`VoidErosion` + 累计值追踪 + 阶段推进 + 2 基础招式（常驻涡流 + 虚涡） | ⬜ |
| **P1** | 3 招式 + 死亡螺旋——吞涡 + 涡流回响 + 虚心 + 正灵域效率惩罚集成 | ⬜ |
| **P2** | 忘音台地形 + 观主残魂 NPC + 馆藏书 | ⬜ |
| **P3** | Client 视觉/音效——虚蚀半透明 + 回响粒子重播 + 声音扭曲 HUD | ⬜ |
| **P4** | 境界递进 + 平衡 + 天道互动 | ⬜ |

---

## 四、P-1：contamination 定向裂痕路由

### 现状

`contamination.rs:142` 在 qi 不足时对**首条已开经脉**添加裂痕，忽略 `ContamSource.meridian_id`：

```rust
if let Some(m) = meridians.iter_mut().find(|m| m.opened) { ... }
```

### 改动

当 `entry.meridian_id == Some(id)` 时，裂痕加到指定经脉；`None` 时保留原行为：

```rust
let target = match entry.meridian_id {
    Some(id) => meridians.iter_mut().find(|m| m.id == id && m.opened),
    None => meridians.iter_mut().find(|m| m.opened),
};
if let Some(m) = target { ... }
```

### 影响范围

- 现有涡流 v2 已写 `meridian_id: Some(Lung)` → 行为从"首开经脉吃裂痕"变为"肺经吃裂痕"（更精确，符合设计意图）
- 丹道 ContamSource 全部 `meridian_id: None` → 行为不变
- 其他所有 ContamSource `None` → 行为不变

### 测试

- `meridian_id: None` 仍走首开经脉（回归安全网）
- `meridian_id: Some(Lung)` 裂痕精确打肺经
- `meridian_id: Some(Heart)` 裂痕精确打心经
- 目标经脉未开时 fallback 到首开经脉
- 现有 `combat::woliu_v2` 153 测试全绿

---

## 五、P0：虚蚀底盘 + 2 基础招式

### §5.1 VoidErosion 组件

```rust
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct VoidErosion {
    /// 历史累计虚蚀值（只增不减）
    pub cumulative_erosion: f64,
    /// 当前阶段
    pub stage: VoidErosionStage,
    /// 常驻涡流是否启用
    pub ambient_active: bool,
    /// 常驻涡流切换时刻
    pub ambient_toggled_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VoidErosionStage {
    None,          // 0: cumulative < 20.0
    LowPressure,   // 1: 20.0-80.0
    VoidShadow,    // 2: 80.0-200.0
    EchoBody,      // 3: 200.0-400.0
    VoidEroded,    // 4: 400.0+
}
```

**初始化时机**：首次使用涡流系招式（含基础 10 招）时 lazy insert，`cumulative_erosion = 0.0`。

### §5.2 虚蚀累积来源

| 来源 | 累积速率 | 说明 |
|------|---------|------|
| 常驻涡流开启 | +0.01/s | 慢性积累 |
| 基础涡流招式施放 | +0.2/次 | 每用一次基础招涨一点 |
| 虚涡 | +3.0/次 | 重磅单次 |
| 吞涡·释放 | +1.0/次 | 安全选择 |
| 吞涡·吞噬 | +蓄力池 × 0.15 | 贪心代价——吃越多涨越快 |
| 涡流回响触发 | +0.5/次 | 每次回响都是虚蚀弹药 |
| 虚心 | +5.0/s × 3s = +15.0 | 单次最高 |
| 在负灵域停留 | +0.005/s × |spirit_qi| | 负灵域呆越久，侵蚀越快 |

### §5.3 阶段推进

`void_erosion_check_system` 每 600 tick（30s）检测 `cumulative_erosion`：

```
if cumulative_erosion >= threshold[next_stage] && current_stage < next_stage:
    emit VoidErosionAdvanceEvent { entity, from, to }
```

**VoidErosionAdvanceEvent 处理链**：
1. 触发顿悟选择（`InsightTrigger::VoidErosionAdvance`）
2. 更新 `stage`
3. 写入 `LifeRecord`
4. 阶段 3+: emit `AgentBridge::VoidErosionEvent` → 天道 narration
5. 阶段 3+: zone 广播

### §5.4 招式一：常驻涡流（Ambient Vortex）

- **解锁**：虚蚀阶段 1+
- **机制**：手动切换（复用 `PassiveVortex` toggle pattern）。开启后：
  - 被动 qi 回复 +0.3/s（`QiTransfer { from: zone, to: player, amount: 0.3, reason: Channeling }`）
  - 3 格内敌人 qi 逸散 ×1.3（阶段 2+）
  - 3 格内投射物命中率 -15%（阶段 3+，空气扭曲）
  - 阶段 4：范围扩至 5 格
- **真元消耗**：无直接 qi 消耗（从 zone 吸取）
- **反噬**：
  - 肺经 `ContamSource { amount: 0.05/s, meridian_id: Some(Lung) }`
  - 心经 `ContamSource { amount: 0.05/s, meridian_id: Some(Heart) }`
  - 虚蚀值 +0.01/s
  - zone spirit_qi -0.01/s（`QiTransfer { from: zone, to: player }`）
  - **硬上限**：zone spirit_qi ≤ 0 时自动关闭（无灵可吸）
- **经脉依赖**：`declare("woliu_ambient_vortex", vec![MeridianId::Lung, MeridianId::Heart])`

**视听规格**：
- **粒子**：脚下 3 格半径淡紫色螺旋纹 `BongGroundDecalParticle` × 1 persistent + `BongSpriteParticle` × 6 continuous 从地面缓慢旋向玩家脚部，颜色 `#9B7DB8`（淡紫），lifetime 40 tick，spawn 1/tick，贴图 `bong:particle/vortex_ambient`，VfxPlayer `WoliuAmbientVfx`，event ID `bong:vfx_woliu_ambient`
- **音效**：`{"layers": [{"sound": "block.portal.ambient", "pitch": 2.5, "volume": 0.04, "delay_ticks": 0}]}` loop every 60 tick（极低频嗡鸣，几乎听不到）
- **HUD**：左下角状态区出现淡紫色「涡」小图标（仅开启时），opacity 0.6

### §5.5 招式二：虚涡（Void Vortex）

- **解锁**：虚蚀阶段 2+
- **机制**：指定 10 格内位置 → 创建 3 格直径临时负灵域球体，持续 5s
  - 球内所有 qi 增强型攻击无效化（飞剑→木棍，暗器→废物，阵法→熄灭）：通过 `TurbulenceField` 负压模式，`remaining_swirl_qi < 0` 表示负灵域
  - 球内敌人 qi 被抽 8/s（`QiTransfer { from: target, to: zone }`——**不给施法者**，qi 还给天地）
  - 球内施法者涡流系招式伤害 +40%
- **真元消耗**：qi 40
- **冷却**：15s
- **反噬**：
  - 虚蚀值 +3.0（单次重磅）
  - 持续 5s 内肺经 `ContamSource { amount: 2.0/s, meridian_id: Some(Lung) }` = 共 10.0
  - 释放瞬间 MeridianCrack 概率 = `5% + lung_contamination × 2%`（肺经越脏越危险）
- **经脉依赖**：`declare("woliu_void_vortex", vec![MeridianId::Lung, MeridianId::Heart])`

**视听规格**：
- **粒子（球体）**：`BongSpriteParticle` × 24 球面分布 continuous，颜色 `#2D1B4E`（深紫黑）→ 中心 `#000000`，lifetime 20 tick，向中心收缩 0.05/tick，贴图 `bong:particle/void_sphere`，VfxPlayer `WoliuVoidVortexVfx`，event ID `bong:vfx_woliu_void_vortex`
- **粒子（qi 抽取）**：`BongLineParticle` × 4/target 从目标→球心，颜色 `#9B7DB8`（目标 qi 可视化被吸走）
- **音效（开启）**：`{"layers": [{"sound": "entity.enderman.teleport", "pitch": 0.3, "volume": 0.6, "delay_ticks": 0}, {"sound": "block.portal.ambient", "pitch": 0.5, "volume": 0.4, "delay_ticks": 0}]}`
- **音效（持续）**：`{"layers": [{"sound": "entity.warden.sonic_boom", "pitch": 0.2, "volume": 0.15, "delay_ticks": 0}]}` loop every 20 tick（低频压迫感）
- **音效（球内效果——qi 增强攻击被无效化时）**：`{"layers": [{"sound": "block.glass.break", "pitch": 0.4, "volume": 0.3, "delay_ticks": 0}]}`（敌人飞剑碎裂声）
- **HUD**：虚涡位置用 `BongGroundDecalParticle` 圆形标示半径，颜色 `#2D1B4E` opacity 0.3

### §5.6 测试要求（P0）

- `VoidErosion` 初始化：首次涡流施法触发 / 重复不重复初始化
- 阶段推进：5 个阶段各自阈值精确断言（0/20/80/200/400）
- 常驻涡流：开启 → qi 回复 + zone drain + 双经脉 contamination 同步
- 常驻涡流硬上限：zone spirit_qi ≤ 0 → 自动关闭断言
- 虚涡：球内敌人 qi drain 走 QiTransfer（zone 为 to，非 caster）
- 虚涡：MeridianCrack 概率公式 = 5% + lung_contam × 2%（Monte Carlo 10000 次验证分布）
- 守恒律：所有 QiTransfer 双端对账

---

## 六、P1：3 招式 + 死亡螺旋

### §6.1 招式三：吞涡（Swallowing Vortex）

- **解锁**：虚蚀阶段 2+
- **机制**：以自身为中心 2s 内 6 格范围吸引一切 → 积累"蓄力池"（从被吸目标 qi 中抽取，走 `QiTransfer { from: target, to: caster_charge_pool }`，charge_pool 是临时中转不入 qi_current）
  - 2s 后二择：
    - **释放**：蓄力池 × 0.8 作为紊流爆发伤害，池归零。虚蚀 +1.0
    - **吞噬**：蓄力池 × 0.4 进入 qi_current。但——
      - 每 1 qi 入账 → 肺经 ContamSource +0.15 + 心经 +0.15
      - 虚蚀 + 蓄力池 × 0.15
      - 吞噬量 > 30 时：心经 MeridianCrack 概率 = `(吞噬量 - 30) × 3%`
- **真元消耗**：qi 30（启动吸引）
- **冷却**：20s
- **经脉依赖**：Lung + Heart

**视听规格**：
- **粒子（吸引阶段）**：`BongSpriteParticle` × 40 从 6 格边缘向中心螺旋收缩，颜色 `#7B5EA7`（中紫），lifetime 40 tick continuous，spawn 4/tick
- **粒子（释放）**：`BongSpriteParticle` × 32 球形 burst 向外，颜色 `#9B7DB8` → `#4A2D6E`（紫→暗紫），lifetime 15 tick
- **粒子（吞噬）**：`BongLineParticle` × 8 从蓄力池位置向施法者胸口汇聚，颜色 `#2D1B4E`（深紫黑），lifetime 10 tick
- **音效（吸引）**：`{"layers": [{"sound": "entity.enderman.teleport", "pitch": 0.4, "volume": 0.5, "delay_ticks": 0}]}`
- **音效（释放）**：`{"layers": [{"sound": "entity.generic.explode", "pitch": 1.5, "volume": 0.5, "delay_ticks": 0}]}`
- **音效（吞噬）**：`{"layers": [{"sound": "entity.generic.drink", "pitch": 0.5, "volume": 0.6, "delay_ticks": 0}, {"sound": "entity.elder_guardian.curse", "pitch": 0.3, "volume": 0.3, "delay_ticks": 4}]}`（吞下"脏 qi"的不祥感）
- **HUD**：蓄力池数值在屏幕中央显示 2s（"释放 / 吞噬"选择提示），颜色 `#9B7DB8`

### §6.2 招式四：涡流回响（Vortex Echo）

- **解锁**：虚蚀阶段 1+
- **机制**：被动。任何涡流系招式（含基础 10 招 + 虚蚀 5 招）释放后，空间留下"褶皱" → 延迟后自动重播弱化版（伤害/效果 40%）
  - 延迟：阶段 1-2 = 2.5s，阶段 3 = 1.5s，阶段 4 = 0.8s
  - 每次回响消耗虚蚀值 0.5
  - 阶段 3+：10% 回响失控（方向随机，可命中自己）
  - 阶段 4：25% 失控，可命中队友
  - 回响不触发新的回响（防无限递归）
- **实现**：订阅 `VortexCastEvent` → 写入 `ScheduledEcho { original_event, replay_at_tick, power_ratio: 0.4 }` → tick system 到时间后 emit 弱化版 VortexCastEvent（标记 `is_echo: true`）

**视听规格**：
- **粒子（残留褶皱）**：原招式结束后，施法位置残留 `BongSpriteParticle` × 4 淡紫色涡纹 persistent，颜色 `#9B7DB8` opacity 0.3，缓慢旋转，lifetime = 延迟时长
- **粒子（回响触发）**：褶皱"抖动"一下 → 释放原招式 40% 规模的粒子（复用原招式 VfxPlayer，传入 scale=0.4）
- **音效（回响）**：原招式音效 pitch × 0.7 + volume × 0.3（"回声"感——更低沉、更远）
- **HUD**：无额外 HUD（回响是自动触发，不需要玩家操作）

### §6.3 招式五：虚心（Void Core）

- **解锁**：虚蚀阶段 3+
- **机制**：
  - 触发后 3s 完全坍缩——新增 `StatusEffectKind::VoidCoreActive`（duration 60 tick）
    - 不可被选中、不可被命中、不可攻击
    - 10 格范围持续吸引（pull 2 block/s 朝施法者）
    - 3s 内施法者不可移动
  - 3s 结束回归——灵压均衡冲击波：
    - 10 格半径
    - 伤害 = 50 + 虚蚀阶段 × 25（阶段 3 = 125，阶段 4 = 150）
    - 冲击波内高 qi 者被抽（额外 drain qi 20）、低 qi 者被灌（+10 qi）——虚实不分的"均衡"
  - 施法者后退 3 block（反冲力）
- **真元消耗**：qi 60（纯消耗，不从吸取回收）
- **冷却**：60s
- **反噬**：
  - 3s 内虚蚀值 +5.0/s = 共 +15.0（单次最高虚蚀增量）
  - 回归瞬间：肺经 ContamSource + 心经 ContamSource 各 = 虚蚀阶段 × 5
  - MeridianCrack 概率：阶段 3 = 25%，阶段 4 = 50%
  - **阶段 4 额外**：5% 概率肺经 SEVERED（`SeveredSource::BackfireOverload`）——**涡流生涯终结**
- **经脉依赖**：Lung + Heart

**视听规格**：
- **粒子（坍缩阶段）**：环境粒子 + `BongSpriteParticle` × 60 从 10 格边缘向施法者急速收缩，颜色 `#2D1B4E` → `#000000`，lifetime 30 tick，spawn 10/tick burst → continuous 2/tick
- **粒子（回归冲击波）**：`BongSpriteParticle` × 48 球面 burst 向外急速扩散，颜色 `#9B7DB8` → `#FFFFFF`（紫→白闪），lifetime 10 tick + `BongLineParticle` × 12 径向线条
- **音效（坍缩）**：`{"layers": [{"sound": "entity.warden.sonic_boom", "pitch": 0.2, "volume": 0.8, "delay_ticks": 0}]}` + 环境音 ducking 至 0.1（3s 内周围几乎无声——你把声音也吸走了）
- **音效（回归）**：`{"layers": [{"sound": "entity.generic.explode", "pitch": 0.8, "volume": 1.0, "delay_ticks": 0}, {"sound": "entity.lightning_bolt.thunder", "pitch": 0.3, "volume": 0.6, "delay_ticks": 2}]}`（真空碎裂 + 灵压均衡的"雷鸣"）
- **HUD**：坍缩期间屏幕边缘急剧收紧 vignette `#2D1B4E` opacity 0.8 → 回归瞬间全屏白闪 0.15s + camera 向后 strong shake

### §6.4 死亡螺旋集成

在 `resolve_woliu_v2_skill` 中注入 `erosion_modifier()`：

```rust
fn erosion_modifier(erosion: &VoidErosion, zone_spirit_qi: f64) -> ErosionMod {
    let positive_penalty = match erosion.stage {
        None => 0.0,
        LowPressure => 0.05,
        VoidShadow => 0.12,
        EchoBody => 0.20,
        VoidEroded => 0.35,
    };
    // 在正灵域（spirit_qi > 0.2）降低涡流效率
    let efficiency = if zone_spirit_qi > 0.2 { 1.0 - positive_penalty } else { 1.0 };
    // 在负灵域（spirit_qi < -0.2）提升涡流效果
    let neg_bonus = if zone_spirit_qi < -0.2 {
        match erosion.stage {
            None => 0.0,
            LowPressure => 0.10,
            VoidShadow => 0.25,
            EchoBody => 0.35,
            VoidEroded => 0.50,
        }
    } else { 0.0 };
    // contamination 乘数
    let contam_mult = match erosion.stage {
        None | LowPressure => 1.0,
        VoidShadow => 1.3,
        EchoBody => 1.5,
        VoidEroded => 1.8,
    };
    ErosionMod { efficiency, neg_bonus, contam_mult }
}
```

**螺旋机制**：阶段越高 → contam_mult 越大 → 经脉 contamination 累积更快 → 排异需要更多 qi → qi 不足频率升高 → 被迫开常驻涡流吸 qi → 虚蚀值继续涨 → 阶段继续升 → ……

### §6.5 测试要求（P1）

- 吞涡：释放 vs 吞噬两个分支各自 happy path + 边界值
- 吞噬量 > 30 时 MeridianCrack 概率公式验证
- 涡流回响：基础 10 招各触发 1 次回响 + 延迟正确
- 回响不触发新回响（递归防护）
- 阶段 3 失控概率 10% / 阶段 4 失控 25%（Monte Carlo）
- 虚心：3s 无敌态 → 受攻击无效 + 不可施法
- 虚心回归伤害 = 50 + stage × 25
- 阶段 4 虚心 5% SEVERED（Monte Carlo）
- 死亡螺旋：正灵域效率惩罚 × 各阶段正确
- 守恒律：所有 QiTransfer 双端对账

---

## 七、P2：忘音台 + 观主残魂

### §7.1 叙事

末法前，**静虚观**是九大宗门中唯一专研涡流的道观。观主试图用涡流阵列"打开一扇门看灵气的另一面"——成功了。然后全观弟子听到了"虚层的声音"，疯了。观主没疯，但他"回不来"了——他的身体卡在虚实之间，成了半虚体。

千年后这片山谷：
- 说话有回声但回声晚 2-3 秒
- 脚步声来自错误方向
- 中央「观天台」废墟仍在运转——一个永不关闭的微型涡流阵列
- 散落的弟子法器（涡流专属装备残片）
- 观主残魂游荡在「观天台」内，只有虚蚀阶段 2+ 的修士才能看到他

### §7.2 Zone 定义

```json
{
  "name": "wangyintai",
  "display_name": "忘音台",
  "aabb": {
    "min": [3200.0, 40.0, -2800.0],
    "max": [4200.0, 200.0, -1800.0]
  },
  "spirit_qi": -0.15,
  "danger_level": 3,
  "ambient_recipe_id": "ambient_wangyintai",
  "patrol_anchors": [[3700.0, 92.0, -2300.0], [3500.0, 88.0, -2500.0]],
  "blocked_tiles": []
}
```

- **spirit_qi -0.15**：负灵域边缘但不深——声音扭曲但不直接致命。涡流修士在此常驻涡流回 qi 更稳定（zone 不会被很快吸到 0）
- **danger_level 3**：中等风险，凝脉可探索、引气需谨慎

### §7.3 Terrain Profile

```json
"wangyintai": {
  "height": { "base": [68, 86], "peak": 98, "compound_flatten_radius": 48 },
  "boundary": { "mode": "soft", "width": 64 },
  "surface": ["smooth_basalt", "deepslate", "calcite", "gray_concrete"],
  "water": { "level": "none", "coverage": 0.0 },
  "passability": "medium",
  "structure_density": {
    "silent_rubble": 0.006,
    "cracked_disc_fragment": 0.003
  },
  "architectural_layout": "wangyintai_compound",
  "ambient_hint": {
    "echo_delay": "constant_2s",
    "sound_misdirection": "moderate",
    "void_hum": "continuous"
  }
}
```

### §7.4 Layout（deterministic）

```python
WANGYINTAI_LAYOUT = LayoutSpec(
    name="wangyintai_compound",
    poi_kind="guantiantai",
    radius=48,
    placements=(
        # 中央：观天台废墟（圆形 20×20 石台 + 中心涡流阵列）
        Placement(offset=(0, 0, 0), rotation=0, kind="nbt",
                  payload="guantiantai_ruins.nbt"),
        # 四方：静虚观四座侧殿残基（按正方位 32 格距离）
        *(Placement(offset=(int(32*cos(radians(a))), 0, int(32*sin(radians(a)))),
                    rotation=int(a)%360, kind="nbt",
                    payload="jingxuguan_side_hall.nbt")
          for a in (0, 90, 180, 270)),
        # 连廊：4 段从中央连向侧殿
        *(Placement(offset=(int(16*cos(radians(a))), 0, int(16*sin(radians(a)))),
                    rotation=int(a)%360, kind="nbt",
                    payload="corridor_fragment.nbt")
          for a in (0, 90, 180, 270)),
        # 散落法器：8 个固定点（叙事/loot）
        *(Placement(offset=(dx, 0, dz), rotation=0, kind="nbt",
                    payload="fallen_vortex_disc.nbt")
          for (dx, dz) in (
              (-8, 20), (8, 24), (-12, -16), (14, -20),
              (-20, 8), (22, -6), (-6, -28), (10, 30),
          )),
    ),
)
```

### §7.5 观主残魂（NPC）

- **不是 BOSS**——导师/任务 NPC
- **可见条件**：观察者 `VoidErosion.stage >= VoidShadow`（阶段 2+）
- **外观**：半透明人形，轮廓闪烁（每 40 tick 出现/消失交替——他卡在虚实之间）
- **说话特性**：他的声音比嘴型早 3 秒到达（narration 先于 NPC 动画播放——音频先到，视觉后到，因为他的因果关系是错位的）
- **功能**：
  - 教授虚蚀路径 5 招（`UnlockSource::Npc { npc_id: "guanzhu_remnant" }`）
  - 告诉玩家静虚观覆灭真相（剧情铺垫）
  - 出售 1 件「观主残碟」——涡流专属辅助装备（持有时回响延迟 -0.3s，但常驻虚蚀 +0.003/s）

**narration 模板**：
- 首次可见时（scope: player, style: perception）：`"你看见了一个……人？不，不完全是人。他的轮廓在空气中闪烁，像水面上的倒影——但没有水。"`
- 对话触发（scope: player, style: dialogue）：`"你也开始'听到'了？——三秒前我说的那句话。是的。你现在听到的是三秒后我要说的。或者说，三秒前我已经说过了。你分不清。我也分不清了。"`
- 关于静虚观（scope: player, style: narrative）：`"我打开了那扇门。看到了灵气的背面——空无一物。真的什么都没有。然后'什么都没有'看到了我们。它不是恶意的。它只是……注意到了。然后声音就不对了。"`

### §7.6 Zone 环境视听

**ambient_wangyintai（audio_recipe）**：
```json
{
  "layers": [
    { "sound": "block.portal.ambient", "pitch": 3.0, "volume": 0.03, "delay_ticks": 0 },
    { "sound": "entity.enderman.stare", "pitch": 0.3, "volume": 0.02, "delay_ticks": 600 },
    { "sound": "ambient.cave", "pitch": 0.8, "volume": 0.05, "delay_ticks": 1200 }
  ]
}
```

**ZoneAtmosphereProfile**：
- **粒子**：`BongSpriteParticle` type `void_shimmer`，密度 0.2/s，tint `#4A2D6E`（暗紫），drift 无方向（原地闪烁），lifetime 30 tick
- **雾**：fogStart 40，fogEnd 120，density 0.01，color `#1A1025`（极暗紫——几乎是黑色的雾）
- **天空色温**：RGB shift `(-10, -8, +5)` 偏暗冷紫
- **特殊音效机制**：zone 内所有玩家声音事件延迟 2s 重播一次（volume × 0.6, pitch × 0.7）——"回声"是 zone 特性，不是 bug
- **首次进入 narration**（scope: player, style: perception）：
  - `"你踩在碎石上的声音——晚了两秒才传到你耳朵里。"`
  - `"空气很薄。不是缺氧，是缺灵。你能感觉到真元在向某个方向流——但你看不到那个方向。"`

### §7.7 测试要求（P2）

- zone 写入后 `cargo test world::zone` 全绿
- terrain profile 解析不抛
- layout determinism：同 seed 两次跑 wangyintai 坐标完全一致
- 观主残魂可见性门控：阶段 0-1 不可见、阶段 2+ 可见
- 观主残魂 NPC 注册 + spawn 不 crash

---

## 八、P3-P4（概要）

### P3：Client 视觉/音效

- 虚蚀半透明：玩家模型 alpha = `1.0 - stage × 0.15`（阶段 4 = 0.4 半透明）
- 回响粒子重播系统：VfxEventRequest 延迟重发 + scale 参数
- 声音扭曲 HUD overlay：阶段 3+ 屏幕边缘轻微波纹效果
- `VoidErosionVisualSyncPayload`（CustomPayload `bong:void_erosion_visual`）同步

### P4：境界递进 + 平衡 + 天道

| 境界 | 解锁 |
|------|------|
| 醒灵 | 可开始积累虚蚀（自然极慢） |
| 引气 | 涡流回响 |
| 凝脉 | 常驻涡流 + 阶段 1 可达 |
| 固元 | 虚涡 + 吞涡 + 阶段 2 可达 |
| 通灵 | 虚心 + 阶段 3 可达 |
| 化虚 | 阶段 4 可达 + 所有招式全解锁 |

天道互动：
- 阶段 3+：天道感知概率 -40%（你存在于天道盲区）
- 阶段 4：天道**放弃追踪**（如暴龙王）

---

## 九、§8 开放问题（P0 决策门前需收口）

1. **常驻涡流切换 vs 常驻被动**：手动 toggle（推荐，复用 PassiveVortex pattern）vs 阶段 1+ 永久开启？
2. **回响失控目标范围**：阶段 3 失控只命中自己 vs 也命中队友？推荐：阶段 3 仅自己，阶段 4 扩展到队友
3. **虚心无敌态实现**：新增 `StatusEffectKind::VoidCoreActive` vs 独立 `VoidCoreActive` component 在 resolve 层检查？推荐：新增 StatusEffectKind 变体（更统一）
4. **忘音台 layout 依赖**：如果 dandao PR-1 的 LayoutSpec 系统已 land → 直接复用；否则 fallback 到 blueprint 固定坐标。需确认 dandao PR-1 状态
5. **虚蚀值跨死亡持久性**：虚蚀值在死亡/重生后保留（推荐，与丹道 cumulative_toxin 对齐）还是重置？
6. **丹道变异 + 涡流虚蚀双叠加**：两者 contamination baseline 惩罚是否叠加？推荐：叠加——选双极端路径的代价应当极端
7. **zone spirit_qi 被常驻涡流永久抽低的恢复机制**：是否引入 zone 自然回复？或者接受"涡流者驻留过的地方灵气变薄"作为世界观后果？
8. **观主残碟装备的具体属性与 forge/inventory 集成**

> 全部已在 §8.1 收口。原表保留以备追溯，**实施时以 §8.1 决议为准**。

---

## 九.1、§8.1 决议（pre-P0 收口，2026-05-19）

### #1 常驻涡流切换 vs 常驻被动

**决议**：
1. 手动 toggle，复用 `PassiveVortex { enabled, toggled_at_tick }` pattern
2. `PassiveVortex.enabled` 字段当前存在但从未被读取（`state.rs:90-94`），toggle 命令未实现。P0 实现时：新增 `ambient_vortex_tick()` system 读 `enabled` 驱动 qi 回复 / zone drain / contamination；toggle 走 skill intent 或 dev command
3. 拒绝"永久开启"：worldview §五:472 "算计型"防御要求主动决策权；永久开启剥夺玩家在正灵域关闭止损的能力

**落点**：`server/src/combat/woliu_v2/state.rs:90-94`（PassiveVortex 定义）/ `server/src/combat/woliu_v2/tick.rs:150-173`（lifecycle 清理）/ plan §5.4

### #2 回响失控目标范围

**决议**：
1. 阶段 3 失控仅命中施法者自身；阶段 4 扩展到半径内所有实体（含队友）
2. 当前 `collect_targets_in_radius()`（`skills.rs:515-546`）仅排除 caster，无 team/faction 系统。失控实现方式：阶段 3 = 将回响目标强制重定向为 caster entity；阶段 4 = 随机选 `collect_targets_in_radius()` 结果中任一实体（含 caster）
3. **不引入 team/faction 基础设施**——回响失控是特殊随机事件，不需要通用阵营系统。若未来需要友方判定，那是独立 plan 的事

**落点**：`server/src/combat/woliu_v2/skills.rs:515-546`（collect_targets_in_radius）/ plan §6.2

### #3 虚心无敌态实现

**决议**：
1. 新增 `StatusEffectKind::VoidCoreActive` 枚举变体
2. 当前 `StatusEffectKind` 有 33 个变体（`events.rs:81-144`），通用检查 `has_active_status()`（`status.rs:62-67`）。在 `resolve.rs:185-187`（攻击判定）和 `resolve.rs:257-259`（防御判定）注入 VoidCoreActive 检查：施法者有此状态 → 不可发起攻击；目标有此状态 → 攻击不命中
3. 拒绝独立 component：增加检查分散点、不走 tick 自动过期、HUD/agent 渲染需额外适配。StatusEffectKind 统一框架更干净

**落点**：`server/src/combat/events.rs:144`（新增变体）/ `server/src/combat/resolve.rs:185-187, 257-259`（注入检查）/ `server/src/combat/status.rs:62-67`（通用检查函数）/ plan §6.3

### #4 忘音台 layout 依赖

**决议**：
1. 直接复用 LayoutSpec 系统——dandao PR-1 layout 基础设施**已完整落地**
2. `LayoutSpec` / `Placement` / `LayoutResult` 定义在 `worldgen/scripts/terrain_gen/layouts/base.py:16-101`；`dan_zong_compound.py` 是 49 placement 的完整范例。忘音台新建 `worldgen/scripts/terrain_gen/layouts/wangyintai_compound.py`，照 plan §7.4 的 `WANGYINTAI_LAYOUT` 定义 Placement 列表
3. 无需 fallback 到 blueprint 固定坐标——基础设施已就绪

**落点**：`worldgen/scripts/terrain_gen/layouts/base.py:16-101`（LayoutSpec 定义）/ `worldgen/scripts/terrain_gen/layouts/dan_zong_compound.py`（范例）/ plan §7.4

### #5 虚蚀值跨死亡持久性

**决议**：
1. `VoidErosion` component 跨死亡保留（`cumulative_erosion` + `stage` 不重置），与 dandao `cumulative_toxin` 对齐
2. dandao `DandaoStyle`（`dandao/components.rs:12-24`）是 `#[derive(Component, Serialize, Deserialize)]`，death cleanup 不移除此类 component。`VoidErosion` 采用相同 derive pattern，持久化层自动处理序列化
3. 死亡时仅重置临时状态：`ambient_active = false`（常驻涡流关闭）、清除 `StatusEffectKind::VoidCoreActive`。`cumulative_erosion` 和 `stage` 永久保留——虚蚀是不可逆的

**落点**：`server/src/dandao/components.rs:12-24`（DandaoStyle 持久化范例）/ `server/src/npc/lifecycle.rs:942-1089`（death handling）/ plan §5.1

### #6 丹道变异 + 涡流虚蚀双叠加

**决议**：
1. 两者 contamination 惩罚叠加（乘法组合）。选双极端路径的代价应当极端
2. dandao `MERIDIAN_PENALTY_BY_STAGE`（`mutation.rs:15`）= `[0.0, 0.03, 0.08, 0.15, 0.20]`，当前**未接入 combat resolve**（`resolve.rs:762-765` 无 MutationState 查询）。本 plan P1 `erosion_modifier().contam_mult` 实现时：查询 `Option<&MutationState>` → 公式 `(1.0 + meridian_penalty) × contam_mult`。例：dandao 阶段 3 + 虚蚀阶段 3 = `(1.0 + 0.15) × 1.5 = 1.725×` 基线 contamination
3. dandao meridian_penalty 接入 combat resolve 属于丹道 plan 遗留缺口，本 plan 顺带补上（在同一个 contamination emission 点注入两个查询，不扩大改动面）

**落点**：`server/src/dandao/mutation.rs:15`（MERIDIAN_PENALTY_BY_STAGE）/ `server/src/combat/resolve.rs:762-765`（contamination emission）/ plan §6.4

### #7 zone spirit_qi 恢复机制

**决议**：
1. 不引入 zone 自然回复——接受"涡流者驻留过的地方灵气变薄"作为世界观后果
2. 当前 zone spirit_qi 恢复途径仅有：botany 枯萎归还（`botany/lifecycle.rs:438`）和鼠类生态泄回（`fauna/rat_phase.rs:371`，仅 1% 回收率）。worldview §二 负灵域设计为天地的倒吸，无自然恢复描述。`qi_physics/constants.rs` 无 `ZONE_RECOVERY_*` 常数
3. 常驻涡流已有硬上限（zone spirit_qi ≤ 0 → 自动关闭，plan §5.4），防止无限负值。涡流者要么周期性迁移、要么忍受枯竭——这本身就是"吸灵成瘾者"（worldview §十二:1010）的经济代价

**落点**：`server/src/world/zone.rs:31`（Zone.spirit_qi 定义）/ `server/src/botany/lifecycle.rs:438`（唯一自然恢复）/ plan §5.4（硬上限）

### #8 观主残碟装备属性

**决议**：
1. 新增 `ItemEffect` 变体 `VortexEchoAccelerator { delay_reduction_secs: f64, erosion_cost_per_sec: f64 }`，注册为 JSON item template
2. `ItemEffect` enum（`inventory/mod.rs:234-243`）已有 passive-while-held pattern（`BreakthroughBonus`、`QiRecovery` 等）。观主残碟注册为 `category: Treasure`，`rarity: Legendary`，效果：持有时 `ScheduledEcho.replay_at_tick` 减 0.3s（6 tick），但常驻虚蚀 +0.003/s
3. P2 实现时：在 `woliu_v2` tick 系统中查询 `PlayerInventory.equipment` → 读 `ItemEffect` → 修改 echo 延迟 + 注入额外虚蚀。不需要新的 forge 配方——观主残魂 NPC 直接出售（`UnlockSource::Npc`）

**落点**：`server/src/inventory/mod.rs:234-243`（ItemEffect enum）/ `server/src/inventory/mod.rs:246-250`（ItemRegistry）/ plan §7.5

---

## 十、§10 实施工作流

### §10.1 建筑类：3 轮 + PROMISE

忘音台 NBT（观天台废墟 + 4 侧殿 + 连廊 + 散落法器碟）走 3 轮打磨 + `<PROMISE>` 担保。

### §10.2 多 PR 拆分

| PR | 内容 | 依赖 |
|----|------|------|
| PR-1 | P-1：contamination meridian_id 定向裂痕 | 无 |
| PR-2 | P0+P1：VoidErosion 组件 + 5 招式 + 死亡螺旋 + schema | PR-1 merged |
| PR-3 | P2：忘音台 zone + terrain + layout + NPC + 馆藏 | PR-1 merged + dandao PR-1 layout 系统 |
| PR-4 | P3+P4：client 视觉/音效 + 境界递进 + 平衡 | PR-2 merged |

### §10.3 CodeRabbit 等待协议

ScheduleWakeup 1200s × 最多 3 回合，修完必须重等 APPROVED。

### §10.4 单次 consume-plan 全自动到 merge

### §10.5 Subagent 配置

```
Agent(
  subagent_type: "claude",
  model: "opus",
  prompt: "...PR-N 任务...\n\nultrathink"
)
```

---

## 关键文件清单

| 文件 | 改动类型 |
|------|---------|
| `server/src/cultivation/contamination.rs:142` | P-1 定向裂痕路由 |
| `server/src/combat/woliu_v2/erosion.rs` | 新增：VoidErosion 模块 |
| `server/src/combat/woliu_v2/skills.rs` | 扩展：5 新招式注册 |
| `server/src/combat/woliu_v2/events.rs` | 扩展：WoliuSkillId 新变体 + VoidErosionAdvanceEvent |
| `server/src/combat/woliu_v2/state.rs` | 扩展：ScheduledEcho / VoidCoreState |
| `server/src/combat/events.rs` | 扩展：StatusEffectKind::VoidCoreActive |
| `server/src/schema/woliu_erosion.rs` | 新增：IPC payload |
| `server/src/combat/resolve.rs` | 扩展：VoidCoreActive 检查 |
| `server/zones.worldview.example.json` | 新增：wangyintai zone |
| `client/src/.../woliu/` | 新增：虚蚀视觉渲染 |

## 验证方式

```bash
cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
cd server && cargo test combat::woliu_v2  # 涡流全量
cd server && cargo test cultivation::contamination  # P-1 回归
cd agent && npm test -w @bong/schema
cd client && ./gradlew test build
```

## Finish Evidence

### 落地清单

| 阶段 | 内容 | 关键文件 |
|------|------|---------|
| **P-1** | contamination `meridian_id` 定向裂痕路由 | `server/src/cultivation/contamination.rs` |
| **P0** | `VoidErosion` 组件 + 阶段推进 + 累计追踪 | `server/src/combat/woliu_v2/erosion.rs` |
| **P0** | 5 虚蚀招式注册（AmbientVortex / VoidVortex / SwallowingVortex / VortexEcho / VoidCore） | `server/src/combat/woliu_v2/events.rs`, `skills.rs` |
| **P1** | 死亡螺旋 `erosion_modifier()` + 正/负灵域效率修正 | `server/src/combat/woliu_v2/erosion.rs` |
| **P1** | `StatusEffectKind::VoidCoreActive` 无敌态 + resolve 检查 | `server/src/combat/events.rs`, `resolve.rs` |
| **P1** | `ScheduledEcho` + 回响递归防护 | `server/src/combat/woliu_v2/state.rs` |
| **P2** | 忘音台 zone（spirit_qi -0.15, danger_level 3） | `server/zones.worldview.example.json` |
| **P2** | 忘音台 terrain profile + `wangyintai_compound` layout | `worldgen/scripts/terrain_gen/profiles/wangyintai.py`, `layouts/wangyintai_compound.py` |
| **P2** | 观主残魂 NPC（虚蚀阶段 2+ 可见门控） | `server/src/npc/guanzhu_remnant.rs` |
| **P2** | 馆藏书 `cultivation-0005.json` 静虚观覆灭志 | `docs/library/cultivation/cultivation-0005.json` |
| **P3** | `VoidErosionVisualSyncPayloadV1` CustomPayload 发射 | `server/src/network/void_erosion_visual_emit.rs` |
| **P3** | schema `woliu_erosion.ts` + `woliu_erosion.rs` 双端对齐 | `agent/packages/schema/src/woliu_erosion.ts`, `server/src/schema/woliu_erosion.rs` |
| **P4** | `realm_unlocks_skill()` + `realm_erosion_cap()` 境界递进 | `server/src/combat/woliu_v2/erosion.rs` |
| **P4** | `tiandao_detection_modifier()` 天道感知衰减 | `server/src/combat/woliu_v2/erosion.rs` |

### 关键 commit

| hash | 日期 | 说明 |
|------|------|------|
| `3687f91ab` | 2026-05-19 | PR-1: contamination meridian_id 定向裂痕路由 (#273) |
| `4d095de9d` | 2026-05-19 | PR-2: VoidErosion 虚蚀底盘 + 5 招式 + 死亡螺旋 (#274) |
| `9a64ed1b9` | 2026-05-19 | PR-3 (P2): 忘音台 zone + 观主残魂 NPC + 馆藏书 (#275) |
| `ca14c242f` | 2026-05-19 | PR-4 (P3+P4): visual sync + realm gating + tiandao modifier (#276) |

### 测试结果

```
cd server && cargo test                           → 5548 passed, 0 failed
cd server && cargo test combat::woliu_v2          → 239 passed (含虚蚀全量)
cd server && cargo test cultivation::contamination → 13 passed (P-1 回归)
cd server && cargo test network::void_erosion     → 3 passed (P3 视觉发射)
cd agent && npm test -w @bong/schema              → 405 passed (含 woliu_erosion 双端)
```

### 跨仓库核验

| 仓库 | 命中 symbol |
|------|------------|
| **server** | `VoidErosion`, `VoidErosionStage`, `VoidErosionAdvanceEvent`, `erosion_modifier()`, `realm_unlocks_skill()`, `realm_erosion_cap()`, `tiandao_detection_modifier()`, `StatusEffectKind::VoidCoreActive`, `ScheduledEcho`, `VoidErosionVisualSyncPayloadV1`, `emit_void_erosion_visual_sync()`, `guanzhu_remnant` NPC |
| **agent/schema** | `VoidErosionStateV1`, `VoidErosionEventV1`, `VoidErosionVisualSyncPayloadV1`, `VoidErosionTiandaoModifierV1`, `validateVoidErosionStateV1Contract()`, `validateVoidErosionEventV1Contract()`, `validateVoidErosionVisualSyncV1Contract()` |
| **worldgen** | `WangyintaiGenerator`, `wangyintai_compound` layout, terrain profile registered in `__init__.py` |

### 遗留 / 后续

- **worldview.md §五.2 虚蚀**：plan 头部标"待写入"——属 worldview 修改，需人工 PR（consume-plan 禁止自动改 worldview）
- **client 渲染实装**：P3 schema + server 发射已落地，client Fabric 侧渲染（半透明 / 回响粒子 / 声音扭曲 HUD）需后续 client plan
- **CodeRabbit 低优先级建议（PR #276 round 2）**：HashSet dedup、magic number constant、EPSILON tolerance、补充边界测试——均为 polish，不阻塞功能
- **观主残碟装备**：plan §7.5 提及的涡流专属辅助装备，需 forge/inventory 集成后续 plan
- **zone spirit_qi 自然回复机制**：plan §9 #7 开放问题——当前接受"涡流者驻留过的地方灵气变薄"作为世界观后果
