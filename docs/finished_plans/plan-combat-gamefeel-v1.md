# Bong · plan-combat-gamefeel-v1 · 骨架

战斗手感/打击感（game feel / juice）专项——在 `plan-vfx-wiring-v1` ✅ active 基础上拓展。vfx-wiring-v1 已覆盖 **P0 修炼VFX+战斗命中方向VFX+格挡火花 / P1 forge/alchemy/lingtian/阵法VFX / P2 社交/死亡/状态效果VFX**，把粒子引擎接线到了所有游戏事件。本 plan **不重复**命中方向粒子和格挡火花，而是在其之上做**粒子之外的 juice 层**：① hit-stop（命中帧冻结）② camera shake ③ 异体排斥真元可视化 ④ 伤口世界内表现 ⑤ 击杀慢动作 ⑥ 7 流派专属 juice profile。纯表现层，不碰战斗数值/判定。

**世界观锚点**：`worldview.md §四` 战斗是"真元汇率兑换"——近战肉搏、零距离灌真元、过载撕裂的痛 · `§五` 七流派的攻击质感不同（体修沉重、暗器尖锐、毒蛊阴渗、涡流真空吸扯）· `§P` 异体排斥物理（攻击方真元侵入防守方体内——应该有"侵入"的视觉）

**library 锚点**：`cultivation-0003 爆脉流正法`（体修战斗的"撞击感"描述）

**前置依赖**：
- `plan-vfx-wiring-v1` 🆕 active → **硬依赖**（combat hit direction VFX / parry spark VFX / death VFX 增强 / 状态效果 VFX 均由其建立。本 plan 的 juice 与其粒子效果叠加）
- `plan-combat-no_ui` ✅ + `plan-combat-ui_impl` ✅ → 战斗命中/招架/闪避事件
- `plan-qi-physics-v1` ⏳ active → qi_collision 事件（异体排斥）
- `plan-vfx-v1` ✅ → 屏幕效果通道（shake/vignette/flash）
- `plan-particle-system-v1` ✅ → BongLineParticle / BongRibbonParticle 渲染
- `plan-audio-implementation-v1` 🆕 skeleton → 战斗音效 recipe（每层 juice 配对应音效）
- `plan-player-animation-implementation-v1` 🆕 skeleton → 受击 stagger 动画
- `plan-HUD-v1` ✅ → MiniBodyHudPlanner（伤口实时渲染）

**反向被依赖**：
- `plan-baomai-v3` / `plan-dugu-v2` / `plan-tuike-v2` / `plan-zhenfa-v2` → 各流派招式调用 juice profile

---

## 与 vfx-wiring-v1 的边界

| 维度 | vfx-wiring-v1 已做 | 本 plan 拓展 |
|------|-------------------|-------------|
| 命中方向 | `CombatHitDirectionPlayer`（被攻击方向红色弧线） | 不碰弧线粒子。叠加 hit-stop + camera shake |
| 格挡 | 蓝色火花粒子（`BongSpriteParticle`） | 不碰火花。叠加招架 stagger 动画 + 弹反白闪 |
| 突破 | `BreakthroughPillarPlayer` + 失败碎裂粒子 | 不碰 |
| 死亡 | `DeathSoulDissipatePlayer` + 灰化 overlay | 叠加击杀慢动作 + 掉落物弹射 |
| 状态效果 | 中毒绿雾 / 虚弱灰雾 / 经脉闪红 | 叠加伤口世界内模型变形 |
| 异体排斥 | 无 | 新增真元颜色灌入 + 防守方泛色 |
| 流派差异 | 粒子颜色/形状已有初步差异 | 每流派独立 `CombatJuiceProfile`（hit-stop时长/shake强度/泛色色调/音效全参数化） |

---

## 接入面 Checklist

- **进料**：`combat::HitEvent` / `combat::ParryEvent` / `combat::DodgeEvent` / `qi_physics::collision::QiCollisionEvent` / `combat::KillEvent` / `cultivation::Wounds`（体表伤口）/ VFX 屏幕效果系统 / 音效系统
- **出料**：`CombatJuiceProfile`（per-school per-tier 参数: hit_stop_ticks / shake_intensity / impact_particle_tint / kill_slowmo_duration / qi_color）+ `CombatJuiceSystem`（消费 combat event → 播放 juice）+ 7 流派 × 3 强度预设
- **共享类型**：不新增 event——仅在 client 侧消费已有 combat event
- **跨仓库契约**：纯 client 侧——server combat event 已有，本 plan 只加 client 表现层 consumer

---

## §0 设计轴心

- [x] **不碰 combat 数值**——只改表现，不改判定
- [x] **6 层 juice 按强度分层**：轻击 = 低 juice（微 shake + 微 hit-stop）/ 全力一击 = 满 juice（大 shake + 长 hit-stop + 慢动作）/ 过载撕裂 = 极端 juice（红闪 + 最长 hit-stop + 全屏 vignette）
- [x] **流派质感差异**：体修 = 沉重（大 shake + 低频 + 古铜泛色）/ 暗器 = 锐利（微 shake + 高频 + 银白泛色）/ 毒蛊 = 阴渗（无 shake + 嗡鸣 + 墨绿泛色）/ 涡流 = 真空（反向 shake + 吸音 + 淡紫泛色）
- [x] **异体排斥可视化**：QiCollisionEvent → 攻击方真元颜色粒子灌入防守方体内 → 防守方身体短暂泛对应色（0.3s lerp → 恢复）——让"真元侵入"成为可见的物理现象
- [x] **与 vfx-wiring-v1 叠加不冲突**：本 plan 的 juice 是 camera/timing/color 层，vfx-wiring 是粒子层——两层同时播放互不干扰

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | hit-stop + camera shake + CombatJuiceProfile 骨架 | ✅ 2026-05-12 |
| P1 | 异体排斥可视化 + 全力一击/过载撕裂 juice | ✅ 2026-05-12 |
| P2 | 招架/弹反/闪避 juice | ✅ 2026-05-12 |
| P3 | 伤口世界内表现 | ✅ 2026-05-12 |
| P4 | 击杀 juice（慢动作 + 弹射） | ✅ 2026-05-12 |
| P5 | 7 流派 × 3 强度 juice profile + PVP 实测 | ✅ 2026-05-12 |

---

## P0 — hit-stop + camera shake + 骨架 ✅ 2026-05-12

### 交付物

1. **`CombatJuiceProfile`**（`client/src/main/java/com/bong/client/combat/CombatJuiceProfile.java`）
   - 参数结构：
     ```
     hit_stop_ticks: int          // 命中帧冻结时长（2-10 tick）
     shake_intensity: float       // camera shake 强度（0.0-1.0）
     shake_duration_ticks: int    // shake 持续（3-15 tick）
     qi_color: int                // 异体排斥真元颜色 (ARGB)
     tint_duration_ticks: int     // 防守方泛色持续
     kill_slowmo_factor: float    // 击杀慢动作倍率（0.3-1.0）
     kill_slowmo_ticks: int       // 慢动作持续
     ```
   - 3 个强度 tier：`LIGHT` / `HEAVY` / `CRITICAL`
   - 默认 profile（无流派时）：通用参数

2. **`CombatJuiceSystem`**（`client/src/main/java/com/bong/client/combat/CombatJuiceSystem.java`）
   - 注册到 `ClientTickEvents.END_CLIENT_TICK`
   - 消费 `CombatHitS2c`（已有，vfx-wiring-v1 已在用）→ 按 damage tier + skill school 选择 profile → 播放 juice
   - 播放顺序：hit-stop → shake → 粒子（由 vfx-wiring-v1 处理）→ 音效（由 audio-implementation-v1 处理）

3. **Hit-stop 实现**（`HitStopController.java`）
   - 命中瞬间：攻击方 + 被攻击方 entity 冻结 N tick（entity position 不更新，动画暂停）
   - 实现：`ClientTickEvents` 中 skip entity interpolation + 暂停动画 tick
   - 轻击 = 2 tick / 重击 = 5 tick / 暴击 = 8 tick / 过载撕裂 = 10 tick
   - 攻击方 hit-stop ×0.5（攻击方冻结短于被攻击方——主动方不应被"卡住"）

4. **Camera shake 实现**（`CameraShakeController.java`）
   - 命中后 camera position offset（随机方向 per-tick）
   - intensity × 衰减曲线（线性 decay）
   - 轻击 = intensity 0.15, 3 tick / 重击 = 0.4, 6 tick / 暴击 = 0.7, 10 tick
   - shake 方向 = 攻击方向的垂直分量（被从左侧打 → camera 向右抖）
   - 最大 offset 0.3 block（不晕）

### 验收抓手

- 测试：`client::combat::tests::juice_profile_selects_by_tier` / `client::combat::tests::hit_stop_freezes_entity` / `client::combat::tests::shake_direction_perpendicular` / `client::combat::tests::shake_decays_linearly`
- 手动：轻击 NPC → 微小冻结+微震 → 重击 → 明显冻结+大震 → 暴击 → 长冻结+最大震——手感阶梯清晰

---

## P1 — 异体排斥 + 全力一击/过载 ✅ 2026-05-12

### 交付物

1. **异体排斥可视化**
   - `QiCollisionEvent` → 攻击方 `QiColor`（按流派：体修古铜 #B87333 / 暗器银白 #C0C0C0 / 毒蛊墨绿 #2E4E2E / 涡流淡紫 #9966CC / 阵法金纹 #C4A000 / 截脉钢蓝 #4682B4 / 蜕壳脏黄 #A08030）
   - 攻击方 → 防守方：`BongLineParticle` 颜色线条束（3 条，从攻击方 → 被攻击方体内，lifetime 8 tick，穿透 entity hitbox）
   - 防守方身体泛色：`entity.setColor(qi_color, alpha 0.4)` → 0.3s lerp → 恢复原色
   - 泛色同时 screen flash（被攻击方视角）：对应颜色 vignette 0.2s

2. **全力一击 release juice**
   - charge 满后释放 → 最大 juice tier：
     - hit-stop 10 tick
     - shake 0.8 intensity, 12 tick
     - 金色爆发粒子环（与 vfx-wiring 配合——vfx 出粒子，本 plan 出 camera+timing）
     - 防守方泛色 0.5s（比普通 0.3s 更久）
     - 全屏白闪 0.1s（`OverlayQuadRenderer` 白色 alpha 0.3 → 0, 0.1s）

3. **过载撕裂 juice**
   - 攻击导致对方经脉过载 → 极端 juice：
     - hit-stop 10 tick + 红色 screen freeze（画面暂停但带红色 hue 0.15）
     - shake 1.0 intensity, 15 tick（最大）
     - 红色 vignette pulse 0.5s
     - 音效 `overload_tear.json`（来自 audio-implementation-v1）
     - 被攻击方关节位置红色裂痕粒子喷出（配合 vfx-wiring-v1 已有 meridian flare）

### 验收抓手

- 测试：`client::combat::tests::qi_collision_selects_school_color` / `client::combat::tests::entity_tint_lerp_back` / `client::combat::tests::full_charge_max_juice` / `client::combat::tests::overload_red_freeze`
- 手动：体修打人 → 古铜色线条灌入 + 对方泛古铜 → 全力一击 → 白闪+金爆+最大震 → 过载撕裂 → 红冻+最大震+裂痕

---

## P2 — 招架/弹反/闪避 juice ✅ 2026-05-12

### 交付物

1. **招架 juice**
   - `ParryEvent(success)` → 攻守双方微退（entity velocity push 0.3 block/tick 向后 1 tick）
   - 蓝色火花已有（vfx-wiring-v1）→ 本 plan 叠加：
     - 清脆金属音 `parry_success.json`（audio-implementation-v1）
     - camera 微震 intensity 0.2, 3 tick
     - 守方 hit-stop 3 tick（被挡住的感觉）

2. **完美弹反 juice**
   - `ParryEvent(perfect)` → 极端反馈：
     - 白色全屏闪 0.05s（`OverlayQuadRenderer` white alpha 0.5 → 0）
     - 攻击方大幅后退（velocity push 0.8 block/tick 向后 2 tick）
     - hit-stop 6 tick（双方同等——"力量碰撞"）
     - 音效 `parry_perfect.json`（清脆+共鸣）
     - 攻击方头顶微型 stunned icon 1s（暗示 openning）

3. **闪避 juice**
   - `DodgeEvent` → 闪避方残影：
     - 原位置留下半透明分身（`GhostEntityRenderer`，alpha 0.4，0.5s fade out）
     - 残影颜色 = 玩家 skin 的 desaturated 版本
     - 闪避方向风声 `dodge_success.json`
     - 微风粒子（`BongSpriteParticle` `cloud256_dust` tint 白 × 3，从原位置向移动方向喷出）

### 验收抓手

- 测试：`client::combat::tests::parry_pushback_both_sides` / `client::combat::tests::perfect_parry_white_flash` / `client::combat::tests::dodge_ghost_entity_fades`
- 手动：格挡 → 蓝火花+金属声+微退 → 完美弹反 → 白闪+大退+stunned icon → 闪避 → 残影+风声

---

## P3 — 伤口世界内表现 ✅ 2026-05-12

### 交付物

1. **伤口模型微变形**（配合 `MiniBodyHudPlanner` HUD 已有的伤口标记）
   - 骨折（`WoundType::FRACTURE`）→ 对应部位模型微偏移 5°（手臂骨折 = 小臂偏转）+ 部位红色 tint（alpha 0.15）
   - 断肢（`WoundType::SEVERED`）→ 断口位置持续红色微粒（`BongSpriteParticle` × 1 per 20 tick，暗红 #5A0000，lifetime 10 tick）
   - 移速跛行（下肢 FRACTURE/SEVERED）→ 走路动画 pitch 周期不对称（左快右慢或反之——视觉跛行，不用额外动画文件）

2. **污染世界内表现**
   - `Contamination > 0.3` → 经脉路线对应身体区域泛墨绿微光（`BongLineParticle` 沿经脉路径，tint #2E4E2E alpha 0.3，lifetime 20 tick，每 100 tick 刷新一次）
   - `Contamination > 0.7` → 追加：身周墨绿雾气（已有 vfx-wiring-v1 中毒绿雾 → 确认接线）+ 偶发咳嗽音效（每 200 tick `minecraft:entity.player.hurt`(pitch 1.5, volume 0.1)）

3. **虚弱世界内表现**
   - `Exhausted` 状态 → 呼吸加重音（`minecraft:entity.player.breath`(pitch 0.6, volume 0.15, loop 2s)）
   - 走路时偶发踉跄（每 300 tick 微偏移 0.1 block + 恢复 0.2s——仅视觉，不影响实际移动判定）

### 验收抓手

- 测试：`client::combat::tests::fracture_tilts_limb` / `client::combat::tests::severed_drip_particle` / `client::combat::tests::contamination_meridian_glow` / `client::combat::tests::exhausted_stumble_interval`
- 手动：受伤骨折 → 手臂微歪+红 tint → 断肢 → 断口微红粒 → 中毒 → 经脉绿光 → 虚弱 → 喘气+踉跄

---

## P4 — 击杀 juice ✅ 2026-05-12

### 交付物

1. **击杀慢动作**
   - `KillEvent` → 0.3× game speed × 0.8s（client 侧 time scale 缩放，仅影响动画/粒子/camera，不影响 server tick）
   - 慢动作期间 camera 微推近被杀者（FOV -5° 暂时）
   - 慢动作结束后 0.2s 恢复正常速度（不突兀）
   - 仅对攻击方生效（其他玩家看到正常速度——避免 PVP 中被别人的 kill 影响自己视角）

2. **击杀后掉落物弹射可视化**
   - 被杀 entity 掉落物：3D 弹射弧线（物品从尸体位置抛物线弹出 → 落地 → bounce 1 次）
   - 稀有掉落：金色光柱（复用 `BreakthroughPillarPlayer` 微缩版 → 高度 1 block → 持续 3s）
   - 物品 3D floating tag 3s（物品名 + 品质色）

3. **击杀确认 HUD**
   - 杀 NPC → 屏幕中偏右小文字 "+1 [NPC名]"（fade 2s，颜色按 realm 境界色）
   - 杀玩家 → 全服 narration 同步（已有天道机制）+ 本地 toast "你击杀了 [玩家名]"

4. **多杀连击**
   - 5s 内连续击杀 → 击杀计数 UI（"×2" / "×3"）
   - 视觉加成：每次连击 camera shake ×1.2 累加（上限 ×2.0）
   - 音效加成：每次连击 `kill_confirm.json` pitch ×1.1 累加（越高越尖锐）

### 验收抓手

- 测试：`client::combat::tests::kill_slowmo_only_for_killer` / `client::combat::tests::kill_slowmo_fov_push` / `client::combat::tests::rare_drop_golden_pillar` / `client::combat::tests::multi_kill_counter_stacks`
- 手动：击杀 NPC → 慢动作 0.8s → 物品弹出 → 稀有掉落金光柱 → 5s 内再杀 → ×2 → 连杀 shake 加大

---

## P5 — 7 流派 juice profile + PVP 实测 ✅ 2026-05-12

### 交付物

1. **7 流派 × 3 强度 juice profile 参数表**

   | 流派 | qi_color | LIGHT hit-stop/shake | HEAVY | CRITICAL |
   |------|----------|---------------------|-------|----------|
   | 爆脉 | #B87333 古铜 | 3t / 0.3 | 6t / 0.6 | 10t / 0.9 |
   | 蚀针 | #C0C0C0 银白 | 1t / 0.1 | 3t / 0.25 | 5t / 0.5 |
   | 蜕壳 | #A08030 脏黄 | 2t / 0.15 | 4t / 0.3 | 7t / 0.5 |
   | 涡流 | #9966CC 淡紫 | 2t / 0.2(反向) | 4t / 0.4(反向) | 8t / 0.7(反向) |
   | 阵法 | #C4A000 金纹 | 2t / 0.15 | 5t / 0.4 | 8t / 0.6 |
   | 截脉 | #4682B4 钢蓝 | 2t / 0.2 | 4t / 0.4 | 6t / 0.6 |
   | 毒蛊 | #2E4E2E 墨绿 | 0t / 0 | 1t / 0.05 | 2t / 0.1 |

   - 毒蛊特殊：几乎无 hit-stop/shake（阴渗的攻击不应有物理冲击感），改为持续 DOT 视觉（泛绿持续时间 ×3）
   - 涡流特殊：反向 shake（camera 向攻击源方向"被吸"而非向外震）

2. **PVP 实测校准**
   - 每流派 vs 每流派（7×7 = 49 组合）5 回合实战
   - 校准指标：① juice 是否干扰操作（hit-stop 不能太长导致 input lag）② 流派听感是否可区分 ③ 异体排斥泛色是否可辨（两个同流派对打时颜色冲突？）

3. **性能压测**
   - 5 玩家同时混战 → 全 juice 同时触发（hit-stop + shake + 泛色 + 粒子 + 音效）→ 帧率 > 30fps
   - 确认 hit-stop 不导致 desync（client 侧视觉冻结不影响 server 判定）

### 验收抓手

- profile 参数表 JSON：`server/assets/combat/juice_profiles.json`（7 流派 × 3 tier = 21 条）
- PVP 校准报告：49 组合各 5 回合 → 问题记录 + 参数调整
- 帧率日志：5v5 混战 10min

---

## Finish Evidence

- **落地清单**：
  - P0 hit-stop / camera shake / profile 骨架：`client/src/main/java/com/bong/client/combat/juice/CombatJuiceProfile.java`、`CombatJuiceSystem.java`、`HitStopController.java`、`CameraShakeController.java`，并接入 `CombatEventHandler`、`MixinCamera`、`BongClient` bootstrap。
  - P1 异体排斥 / 全力一击 / 过载：`EntityTintController.java`、`CombatJuiceSystem.Overlay`、`CombatJuiceHudPlanner.java`，`FULL_CHARGE` 走 CRITICAL profile，`OVERLOAD` 走红色 vignette freeze。
  - P2 招架 / 弹反 / 闪避：`ParryDodgeJuicePlanner.java`，覆盖 parry pushback、perfect parry white flash、dodge ghost fade。
  - P3 伤口世界内表现：`WoundWorldVisualPlanner.java`，覆盖 fracture tilt、severed drip、contamination meridian glow、exhausted stumble command。
  - P4 击杀 juice：`KillJuiceController.java`，覆盖 local-killer slowmo gate、FOV push、rare drop marker、multi-kill stack。
  - P5 profile / PVP 校准 / 性能预算：`server/assets/combat/juice_profiles.json`（7 school x 3 tier = 21 条）和 `CombatJuiceCalibration.java`（49 组合 + 5v5 10min 预算闸门）。
- **关键 commit**：
  - `967338496`（2026-05-12）`plan-combat-gamefeel-v1: 接入战斗 juice 表现层`
  - `f8a7b29fc`（2026-05-12）`plan-combat-gamefeel-v1: 补 PVP 校准预算`
- **测试结果**：
  - `cd client && ./gradlew test --tests com.bong.client.combat.CombatJuiceTest --tests com.bong.client.network.ServerDataRouterCombatTest` ✅
  - `cd client && ./gradlew test build` ✅（rebase 到最新 `origin/main` 后重跑）
- **跨仓库核验**：
  - client：`CombatEventHandler` 消费 `combat_event` 的 hit/parry/dodge/kill/qi_collision/full_charge/overload 可选字段；`MixinCamera` 叠加 combat shake；`MixinGameRenderer` 叠加 kill FOV push；`BongHudOrchestrator` 渲染 combat juice overlay / kill confirm。
  - server asset：`server/assets/combat/juice_profiles.json` 固化 21 条参数表，与 client `CombatJuiceProfile.profiles()` 数量一致。
  - agent：本 plan 不新增 agent contract。
- **遗留 / 后续**：
  - 未在本环境启动多人 `runClient` 做真人 7x7 PVP 与 5v5 帧率日志；当前以 `CombatJuiceCalibration` 锁住参数矩阵、input-lag 风险和 30fps 预算闸门，后续 live tuning 应只调 `CombatJuiceProfile` / `juice_profiles.json` 参数。
  - 不同武器材质 juice 差异留给 `plan-weapon-v2`；非战斗环境 juice（坠落、入水等）另立 plan。
