# Bong · plan-skull-fiend-v1 · 骨架

**骨煞（Skull Fiend）**——坍缩渊负压畸变体亚种。多具修士头骨被负灵真元熔融为一体，残存的战斗本能驱动其飞行冲撞猎物。行为逻辑"直来直往"：锁定 → 蓄力 → 高速直线冲撞 → 命中后短暂眩晕 → 复位重来。

**世界观锚点**：
- `worldview.md §七` 动态生物生态：生物是竞争者/寄生虫/天道清理程序
- `worldview.md §十六.五` 坍缩渊敌对 NPC 表——**负压畸变体**（长期吸收负压的异兽/尸骸被改造；低血量狂暴）
- `worldview.md §十六.六` 干尸 → 道伥机制——骨煞是"头骨级畸变"：躯体已散尽，仅剩头骨被负压焊接
- 负压畸变体特征：**耗真元光环**（进入范围内真元抽吸 +50%）——骨煞变体：冲撞命中时额外扣真元

**美术资源**：`local_models/iridescent skull 3d model.glb`——扭曲虹彩多头骷髅，需降面后接入 client 渲染

**前置依赖**：
- NPC 框架 ✅（big-brain Utility AI，`server/src/npc/brain.rs`）
- 阵营系统 ✅（`server/src/npc/faction.rs`，`is_hostile_pair`）
- combat 事件 ✅（`AttackIntent` / `HitEvent`）
- NPC spawn 框架 ✅（`server/src/npc/spawn.rs`）
- 坍缩渊生命周期（`plan-tsy-*`）→ spawn 触发

**反向被依赖**：
- `plan-tsy-hostile-v*` / `plan-tsy-pve-wave-v*` — 坍缩渊 PvE 浪潮敌人池
- `plan-coffin-v1` — 棺内 boss 或精英怪候选

---

## 接入面 Checklist

- **进料**：`Position`（玩家位置）/ `Cultivation`（目标真元量，用于命中扣真元）/ `FactionStore`（敌对判定）/ 坍缩渊 spawn 事件
- **出料**：`SkullFiendMarker` component / `SkullFiendState` enum / `ChargeImpactEvent`（冲撞命中事件，供 combat 结算+client 表现）/ `S2C bong:skull_fiend_state` payload
- **共享类型**：`ChargeImpactEvent { attacker, target, damage, qi_drain, velocity }` → combat 模块消费
- **跨仓库契约**：
  - server: `SkullFiendMarker` / `ChargeImpactEvent` / spawn 函数
  - client: `SkullFiendRenderer`（GLB 模型加载+旋转动画）/ `ChargeImpactParticle` / SFX
  - schema: `S2C skull_fiend_state { entity_id, state, position, target_position, velocity }`

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | Server 实体 + 飞行移动 + 冲撞 AI + 碰撞伤害 | ⬜ |
| P1 | Client 模型渲染 + 冲撞粒子 + 音效 | ⬜ |
| P2 | 坍缩渊 spawn 接入 + 难度分级 | ⬜ |
| P3 | 狂暴阶段 + 真元光环 + 变体 | ⬜ |

---

## P0 — Server 实体 + 飞行 AI + 碰撞伤害

### 实体定义

`server/src/npc/skull_fiend.rs`

```
SkullFiendMarker           — 标记 component
SkullFiendConfig           — 配置（速度/伤害/探测距离/蓄力时间/眩晕时间）
SkullFiendState            — enum { Idle, Patrol, Locking, Charging, Stunned }
ChargeImpactEvent          — 冲撞命中事件
```

**数值草案**（凡器级，后续按坍缩渊层深倍率）：

| 参数 | 值 | 说明 |
|------|-----|------|
| HP | 40 | 约两个 zombie |
| 物理伤害 | 8（冲撞）/ 2（漂浮接触） | 冲撞是主要输出 |
| 真元吸取 | 命中扣 5 真元 | 畸变体特征 |
| 探测距离 | 24 格 | 飞行视野开阔 |
| 锁定蓄力 | 1.5 秒 | 给玩家反应窗口 |
| 冲撞速度 | 16 格/秒 | 快于玩家跑速（~5.6 格/秒） |
| 冲撞最大距离 | 32 格 | 超过即放弃本次冲撞 |
| 命中后眩晕 | 2 秒 | 惩罚窗口，可被反击 |
| 飞行高度 | 地面 +3~5 格 | 近战需跳劈或用远程 |

### 状态机

```
Idle ──(探测到敌对目标)──→ Locking
  ↑                            │
  │                    (1.5s 蓄力完毕)
  │                            ↓
Stunned ←──(命中目标 or        Charging
  │         飞满 32 格)            │
  │                            (命中方块)
  (2s 眩晕结束)                    ↓
  └──────→ Idle            Stunned (撞墙也眩晕)
```

**"直来直往"核心约束**：
- Charging 阶段**不转向**——锁定方向后直线飞行，玩家侧移即可躲避
- 不会绕障碍物——撞到方块就进入 Stunned
- 不会假装/诱敌——看到就锁，锁完就冲，冲完就晕
- 多只骨煞不协调——各自独立锁定，可能互相撞

### big-brain AI 接法

```rust
// Scorer: SkullFiendAggroScorer
//   检测 detection_range 内最近敌对实体 → 输出 0.0~1.0
//   无目标 → 0.0（fallback 到 Idle/Patrol）

// Action: SkullFiendChargeAction
//   Locking → 面向目标，悬停不动，播放蓄力信号
//   Charging → 沿锁定方向匀速直线移动，每 tick 检查碰撞
//   碰撞判定 → AABB hitbox (1.5×1.5×1.5) 与玩家 hitbox 重叠
//   命中 → emit ChargeImpactEvent + 进入 Stunned
//   撞方块 → 进入 Stunned（自伤 HP×10%）
//   超距 → 进入 Stunned

// Action: SkullFiendIdleAction
//   缓慢上下浮动（sin wave，振幅 0.5 格，周期 3 秒）
//   原地缓转（yaw += 15°/s）
```

### 飞行移动

- 不用 `navigator.rs` 寻路——骨煞无视地形，直线飞
- Position 直接按 velocity 更新（`pos += dir * speed * dt`）
- Y 轴保持在地面 +3~5 格（Idle 时 sin wave 浮动）
- Charging 时 Y 锁定为锁定瞬间的高度

### 碰撞伤害结算

`ChargeImpactEvent` → combat 模块统一结算：
- 物理伤害走 `AttackIntent`（复用现有 damage pipeline）
- 真元吸取走 `QiTransfer`（如 qi_physics 已实装）或直接扣 `Cultivation.qi_pool`
- 击退：命中方向 knockback 6 格

### 测试

- `skull_fiend::state_machine` — 状态转换全覆盖（5 态 × 所有 edge）
- `skull_fiend::charge_hit` — 冲撞命中判定（正面/侧面/擦边/超距）
- `skull_fiend::charge_wall` — 撞方块眩晕 + 自伤
- `skull_fiend::aggro_range` — 探测距离内/外锁定行为
- `skull_fiend::no_steering` — Charging 阶段不转向（记录锁定方向 vs 实际飞行方向）
- `skull_fiend::knockback` — 命中后击退距离
- `skull_fiend::qi_drain` — 真元吸取量
- `skull_fiend::multi_independent` — 多只骨煞各自独立锁定

---

## P1 — Client 模型渲染 + 粒子 + 音效

### 模型处理

- **降面**：原始 GLB 用 Tripo `highpoly_to_lowpoly` 或 Blender decimate 降到 ~5000 面
- **渲染**：自定义 `SkullFiendRenderer` 继承 entity renderer，加载 GLB
- **动画**（无骨骼，纯程序化）：
  - Idle：缓慢自转 + sin 浮动 + 轻微摇晃
  - Locking：停止自转，面向目标，模型开始高频抖动（蓄力视觉信号）
  - Charging：沿冲撞方向高速移动，模型倾斜 15°，拖尾粒子
  - Stunned：模型翻滚旋转 + 缓慢下坠，粒子爆散

### 粒子效果

| 状态 | 粒子 | 说明 |
|------|------|------|
| Idle | 暗紫色微光碎片缓慢环绕 | 负压真元残留 |
| Locking | 红色收束线条指向目标 | 锁定警告，给玩家反应 |
| Charging | 暗紫色高速拖尾 + 空气扭曲 | 冲撞速度感 |
| 命中瞬间 | 白色冲击波 + 骨碎片飞溅 | 撞击反馈 |
| Stunned | 星星/眩晕环 + 暗紫粒子散落 | 可被反击信号 |

### 音效

| 事件 | 音效 | 说明 |
|------|------|------|
| 接近玩家 | 低沉呜鸣 + 骨骼摩擦声 | 距离衰减，先闻其声 |
| Locking | 尖锐金属音 crescendo | 蓄力警告 |
| Charging | 风啸 + 骨骼碰撞连响 | 高速冲来的压迫感 |
| 命中玩家 | 沉闷骨撞 + 真元吸取音效 | 痛感反馈 |
| 撞方块 | 骨碎裂声 | 失误惩罚 |
| 死亡 | 多声道骨裂 + 灵气消散 | 击杀成就感 |

---

## P2 — 坍缩渊 Spawn 接入

- **spawn 规则**：中层+深层坍缩渊，按"上古战场沉淀"起源加权（§十六.五）
- **浪潮集成**：PvE 浪潮敌人池加入骨煞，中层起出现
- **密度**：每层 1-3 只（深层可达 5 只），不成群——单体威胁
- **spawn 位置**：优先选高空/开阔区域（洞穴顶部、大厅），利用飞行优势
- **despawn**：坍缩渊关闭时随之消灭

---

## P3 — 狂暴阶段 + 真元光环 + 变体

### 狂暴（负压畸变体特征）

- HP < 30% 时进入**狂暴**：
  - 蓄力时间 1.5s → 0.8s
  - 冲撞速度 16 → 22 格/秒
  - 眩晕时间 2s → 1s
  - 模型发红光 + 粒子变红
  - 但同时自伤加倍（撞墙 HP×20%）

### 真元耗散光环

- 6 格范围内玩家真元自然消耗 +50%（worldview 负压畸变体特征）
- 视觉：玩家靠近时屏幕边缘出现暗紫色 vignette

### 变体（后续扩展）

| 变体 | 区别 | 出没 |
|------|------|------|
| **骨煞·残** | 基础款，单冲撞 | 中层 |
| **骨煞·执** | 蓄力后可修正一次方向（45°以内） | 深层 |
| **骨煞·狱** | 两连撞（第一次命中后不眩晕，立即锁定第二次） | 深层 boss 级 |

---

## 掉落

- 变异核心（炼器/丹道原料）——概率 30%
- 异兽骨骼（可制封灵匣）——概率 15%
- 破碎法宝——概率 5%（深层加倍）
- 残卷——概率 2%
- 狂暴状态击杀：上述概率 ×1.5

---

## Finish Evidence

（迁入 finished_plans/ 前填写）
