# Bong · plan-movement-v1 · 修仙移动体系

修仙移动体系基础包：全局移速调整 + 冲刺 / 滑铲 / 二段跳 三个核心动作。当前玩家移动 = vanilla MC 默认速度（4.317 m/s 走、5.612 跑），末法残土应该整体更慢更沉重——修仙者在灵压不稳的世界里移动本身就是消耗。本 plan 先把基础移速降下来，再给出 3 个主动移动技能作为"修仙者比凡人强"的体现。**每个动作都有完整的 动画 + 粒子 + 音效 + HUD 反馈**，HUD 遵循自动隐藏策略（使用时 hover 显示，长时间不用自动消失）。

**世界观锚点**：`worldview.md §四` 零距离肉搏 → 冲刺缩距是战术核心 · `§三` 境界越高体能越强 → 移动技能按境界增强 · `§十三` 地形差异大 → 移动在不同 zone 有不同手感（死域减速/负灵域移速不稳）

**前置依赖**：
- `plan-cultivation-v1` ✅ → Realm / Cultivation / 体力（stamina）
- `plan-skill-v1` ✅ → SkillRegistry（移动技能注册为 utility skill）
- `plan-combat-no_ui` ✅ → CombatState（战斗中移动参数变化）
- `plan-vfx-v1` ✅ → 屏幕效果
- `plan-particle-system-v1` ✅ → 粒子基类
- `plan-audio-v1` ✅ → SoundRecipePlayer
- `plan-HUD-v1` ✅ → BongHudOrchestrator
- `plan-input-binding-v1` ✅ → keybind 注册

**反向被依赖**：
- `plan-combat-gamefeel-v1` → 闪避残影需要移动系统配合
- `plan-player-animation-implementation-v1` → 移动动画需要此系统的状态信号

---

## 接入面 Checklist

- **进料**：`cultivation::Cultivation { realm, stamina }` / `combat::CombatState` / `input_binding::KeybindRegistry` / `world::zone::ZoneEnvironment`（zone 移速修正）/ MC `PlayerEntity.getMovementSpeed()`
- **出料**：server `movement::MovementSystem`（全局移速调整 + 3 动作状态机）+ `MovementStateS2c` packet + client `MovementController`（动画/粒子/音效/HUD 触发）+ `MovementHudPlanner`（stamina 消耗指示 + 冷却弧线）
- **跨仓库契约**：server `movement::*` → `bong:movement_state` CustomPayload → client `MovementController`

---

## §0 设计轴心

- [x] **整体减速**：vanilla 默认 ×0.75（走 3.24 m/s / 跑 4.21 m/s）—— 末法残土比 vanilla 更沉重
- [x] **境界修正**：每升一境 +5% 移速（化虚 = ×0.75 × 1.30 = ×0.975 ≈ 接近 vanilla 速度）
- [x] **体力消耗**：冲刺/滑铲/二段跳均消耗 stamina，stamina 耗尽 → 动作不可用
- [x] **HUD 自动隐藏**：移动技能 HUD 仅在使用时短暂 hover 显示（冷却弧线 + stamina 消耗指示），3s 不使用自动 fade out
- [x] **zone 移速修正**：死域 ×0.8 / 负灵域 ×0.9 + 移速波动 / 残灰方块 ×0.7

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | 全局移速调整 server 实装 + 境界/zone 修正 + 基础 stamina 消耗框架 | ✅ |
| P1 | 冲刺（Sprint Dash）全流程：server 逻辑 + 动画 + 粒子 + 音效 + HUD | ✅ |
| P2 | 滑铲（Slide）全流程：server 逻辑 + 动画 + 粒子 + 音效 + HUD | ✅ |
| P3 | 二段跳（Double Jump）全流程：server 逻辑 + 动画 + 粒子 + 音效 + HUD | ✅ |
| P4 | HUD auto-hide 策略 + 全动作 × 全境界 × 全 zone 饱和化测试 | ✅ |

---

## P0 — 全局移速调整 ✅

### 交付物

1. **`MovementSystem`**（`server/src/movement/mod.rs`）
   - `BASE_MOVE_SPEED_MULTIPLIER: f32 = 0.75`（全局减速 25%）
   - `realm_speed_bonus(realm: Realm) -> f32`：每境 +0.05（醒灵 0.00 / 引气 0.05 / 凝脉 0.10 / 固元 0.15 / 通灵 0.20 / 化虚 0.25）
   - `zone_speed_modifier(zone: &ZoneEnvironment) -> f32`：正常 1.0 / 死域 0.8 / 负灵域 0.9 + random ±0.05 per tick / 残灰方块 0.7
   - 最终移速 = vanilla_speed × BASE × (1 + realm_bonus) × zone_modifier
   - 通过 MC `EntityAttributeModifier` 应用（`generic.movement_speed` attribute）

2. **Stamina 框架扩展**
   - `cultivation::Cultivation.stamina` 已有 → 本 plan 追加 `stamina_regen_rate` 按境界（引气 2/s / 凝脉 3/s / 固元 4/s / 通灵 5/s / 化虚 6/s）
   - stamina < 10 时移速额外 ×0.6（精疲力竭）
   - stamina = 0 时禁用所有移动技能

3. **`MovementStateS2c` packet**
   - `{ current_speed_multiplier: f32, stamina_cost_active: bool, movement_action: MovementAction }`
   - `MovementAction` enum：`None` / `Dashing` / `Sliding` / `DoubleJumping`
   - client 消费后更新 HUD + 触发对应视觉

4. **attribute modifier 同步**
   - 玩家上线 / realm 变化 / zone 切换时 → recalculate → apply modifier
   - 20 单测：各境界各 zone 移速计算正确

### 验收抓手
- 测试：`server::movement::tests::base_speed_reduction` / `server::movement::tests::realm_bonus_stacks` / `server::movement::tests::dead_zone_slows` / `server::movement::tests::exhausted_penalty`
- 手动：新角色进入 → 明显比 vanilla 慢 → 升境 → 微快 → 进死域 → 更慢

---

## P1 — 冲刺 ✅

### 交付物

1. **冲刺（Sprint Dash）server 逻辑**
   - keybind：双击 W（或 Shift+W）
   - 效果：瞬间向前方冲刺 4 block（0.2s 内完成）+ 冲刺过程无碰撞箱（穿过 entity，不穿方块）
   - stamina 消耗：15
   - 冷却：2s（凝脉+ 1.5s / 通灵+ 1s）
   - 战斗增强：冲刺接攻击 → 首击伤害 +20%（冲刺动量加成）

2. **动画**：`dash_forward.json`（body 前倾 25° + legs 大步 + arms 后摆，4 tick，FULL_BODY）

3. **粒子**：
   - 起始位置：脚底灰白气流爆发（`BongSpriteParticle` `cloud256_dust` × 6 向后喷出）
   - 移动轨迹：淡白色 afterimage 残影线（`BongLineParticle` 从起点→终点，lifetime 10 tick，alpha fade）
   - 到达位置：微小着陆尘（× 4 从脚底向四周扩散）

4. **音效**：`movement_dash.json`（`minecraft:entity.phantom.flap`(pitch 1.8, volume 0.3) + `minecraft:entity.player.attack.sweep`(pitch 1.5, volume 0.15) — 急风 + 破空）

5. **HUD**（`MovementHudPlanner.java`）：
   - 冲刺使用时：快捷栏上方短暂出现冷却弧线（2s 圆弧从满→空）+ stamina 条闪绿 flash
   - 3s 不使用 → 弧线 fade out
   - stamina 不足时按冲刺 → 弧线闪红 0.3s 提示

### 验收抓手
- 测试：`server::movement::tests::dash_distance_4_blocks` / `server::movement::tests::dash_stamina_cost` / `server::movement::tests::dash_cooldown_by_realm` / `server::movement::tests::dash_attack_bonus`

---

## P2 — 滑铲 ✅

### 交付物

1. **滑铲（Slide）server 逻辑**
   - keybind：跑步中按 Ctrl（或自定义）
   - 效果：跑步状态下低身滑行 3 block（0.4s）+ 碰撞箱高度降至 1 block（可穿过 2 格高通道）+ 滑行中对接触的敌人造成 8 伤害 + 微击退
   - stamina 消耗：12
   - 冷却：3s
   - 结束后 0.3s 站起过渡（此间不可再次滑铲）

2. **动画**：`slide_low.json`（body pitch forward 70° + legs 前伸 + 一膝着地，8 tick，FULL_BODY → 结束后 6 tick 站起过渡）

3. **粒子**：
   - 滑行全程：脚底方块材质色尘土飞扬（`BongSpriteParticle` tint 按当前方块颜色 × 3 per 2 tick 向后喷出）
   - 石面滑行：追加火星粒子（`tribulation_spark` tint #FF8800 × 1 per 3 tick）
   - 滑行结束站起：微尘散开

4. **音效**：`movement_slide.json`（`minecraft:entity.player.swim`(pitch 0.5, volume 0.3) + `minecraft:block.gravel.step`(pitch 1.2, volume 0.2) — 摩擦地面声）

5. **HUD**：同 P1 冷却弧线策略 + 滑行期间屏幕 FOV 微增 +5°（速度感）

### 验收抓手
- 测试：`server::movement::tests::slide_hitbox_reduction` / `server::movement::tests::slide_contact_damage` / `server::movement::tests::slide_requires_running` / `server::movement::tests::slide_to_stand_transition`

---

## P3 — 二段跳 ✅

### 交付物

1. **二段跳（Double Jump）server 逻辑**
   - keybind：空中再按空格
   - 效果：空中获得第二次跳跃（高度 = 普通跳跃 × 0.8）+ 可改变方向（空中转向 ±45°）
   - stamina 消耗：20
   - 冷却：无（但每次起跳只能用一次，落地重置）
   - 境界增强：固元+ 二段跳高度 ×1.0（和普通跳一样高）/ 通灵+ 空中可用 2 次
   - 落地无摔落伤害减免（修仙者不怕摔，但 stamina 消耗就是代价）

2. **动画**：`double_jump.json`（body 微蜷 → 展开 + legs kick down，4 tick，FULL_BODY）

3. **粒子**：
   - 二段跳瞬间：脚底淡白气流环（`BongSpriteParticle` `qi_aura` tint #CCCCFF × 8 环形向下喷出 — "踩在灵气上"）
   - 上升过程：微弱白色尾迹（`BongLineParticle` 从脚底向下 2 block，lifetime 8 tick）

4. **音效**：`movement_double_jump.json`（`minecraft:entity.phantom.flap`(pitch 2.0, volume 0.25) + `minecraft:block.amethyst_block.chime`(pitch 2.5, volume 0.08) — 轻盈风声 + 微灵气振）

5. **HUD**：
   - 空中时快捷栏上方短暂出现"可用次数"indicator（1 个/2 个小圆点，用掉变灰）
   - 落地后 indicator 恢复 → 1s 后 fade out（auto-hide）
   - 通灵+ 有 2 次时显示 2 个圆点

### 验收抓手
- 测试：`server::movement::tests::double_jump_only_airborne` / `server::movement::tests::double_jump_resets_on_land` / `server::movement::tests::double_jump_direction_change` / `server::movement::tests::tongling_gets_2_charges`

---

## P4 — HUD auto-hide + 饱和化测试 ✅

### 交付物

1. **`MovementHudPlanner` auto-hide 策略**
   - 所有移动 HUD 元素（冷却弧线/stamina flash/二段跳圆点）：
     - 使用移动技能时 → 立即 alpha 1.0 显示
     - 最后一次使用后 3s → 开始 0.5s fade out → alpha 0
     - stamina < 30% 时 → stamina 指示常驻 alpha 0.4（低电量警告）
   - 实现：`MovementHudPlanner.lastUsedTick` tracking + `alpha = calculateAutoHide(currentTick - lastUsedTick)`

2. **zone 移速修正视觉反馈**
   - 进入死域/负灵域 → 屏幕边缘微 vignette（暗示"这里不好走"）
   - 踩残灰方块 → 微沉 camera bob（暗示"粘脚"）
   - 负灵域移速波动 → camera 轻微摇晃（per-tick random offset ±0.01 block）

3. **饱和化测试矩阵**
   - 3 动作 × 6 境界 × 4 zone 类型 × 2 状态（正常/stamina 耗尽）= 144 组合
   - 每组合：动作执行正确 / 动画播放 / 粒子触发 / 音效触发 / HUD 正确显示+hide
   - 连续操作压测：冲刺→滑铲→二段跳 连招 → 无动画卡顿 / 无状态错乱

### 验收抓手
- 测试：`client::movement::tests::hud_auto_hide_after_3s` / `client::movement::tests::dead_zone_vignette` / `client::movement::tests::combo_dash_slide_jump_no_desync`
- 手动完整流程：走路感受减速 → 冲刺穿人 → 滑铲过低洞 → 二段跳上高台 → 进死域 → 更慢+粘脚感 → stamina 耗尽 → 动作禁用

---

## Finish Evidence

- **落地清单**：
  - P0：`server/src/movement/mod.rs`、`server/src/schema/movement.rs`、`server/src/main.rs`、`server/src/network.rs` 接入基础移速、realm/zone/stamina modifier、movement_state S2C 与 movement_action C2S。
  - P1-P3：server 状态机覆盖 dash/slide/double_jump；client `MovementKeyRouter` / `MovementKeybindings` / `MovementStateHandler` / `MovementVfxPlayer` / `MovementHudPlanner` 负责输入、协议消费、动画粒子音效与 HUD。
  - 资源：`client/src/main/resources/assets/bong/player_animation/{dash_forward,slide_low,double_jump}.json`、`client/src/main/resources/assets/bong/particles/cloud256_dust.json`、`server/assets/audio/recipes/movement_{dash,slide,double_jump}.json`。
  - 协议：`agent/packages/schema/src/movement.ts`、generated schema/sample、`client_request.movement_action` 与 `server_data.movement_state` registry 均已接入。
- **关键 commit**：
  - `119d3d975`（2026-05-11T04:31:13+12:00）`feat(movement): 接入服务端移动状态机`
  - `391ba4d42`（2026-05-11T04:31:13+12:00）`feat(schema): 补齐 movement 协议契约`
  - `1414b5d97`（2026-05-11T04:31:57+12:00）`feat(client): 接入 movement 动作反馈`
  - `f109491ee`（2026-05-11T04:39:18+12:00）`fix(client): 适配 movement HUD 沉浸布局`
  - `d1ba39531`（2026-05-11T11:56:51+12:00）`fix(movement): 收紧移动状态契约与同步边界`
- **测试结果**：
  - `server/ cargo fmt --check` ✅
  - `server/ cargo clippy --all-targets -- -D warnings` ✅
  - `server/ cargo test` ✅ 4035 passed
  - `agent/packages/schema npm test` ✅ 17 files / 359 tests
  - `agent npm run build` ✅
  - `client/ JAVA_HOME=$HOME/.sdkman/candidates/java/17.0.18-amzn PATH=$HOME/.sdkman/candidates/java/17.0.18-amzn/bin:$PATH ./gradlew test build` ✅ 1200 tests, 0 failures, 0 errors
  - `client/ ./gradlew test --tests com.bong.client.hud.HudLayoutPresetTest --tests com.bong.client.hud.MovementHudPlannerTest --tests com.bong.client.hud.HudImmersionModeTest` ✅
- **跨仓库核验**：server `movement_state`/`MovementActionIntent`/audio recipes，agent `movement.ts`/generated artifacts，client `MovementStateStore`/`MovementHudPlanner`/`MOVEMENT_HUD`/VFX registry 均有测试或 build 覆盖。
- **遗留 / 后续**：轻功、御风、墙跑、真实 runClient 手感调参仍留给 `plan-movement-v2`；本 plan 未在当前 headless 流水线里跑图形化 `runClient` 手动流程。
