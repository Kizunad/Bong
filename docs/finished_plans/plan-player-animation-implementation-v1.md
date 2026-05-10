# Bong · plan-player-animation-implementation-v1 · 完成

玩家动画系统端到端实现——在 `plan-vfx-wiring-v1`（VFX 事件接线）+ `plan-npc-visual-v1`（NPC 视觉差异化）+ `plan-entity-model-v1`（实体模型补全）✅ active 基础上拓展。上述 active plan 各自建立了事件→视觉的管道，但**全部走粒子/模型路线——没有骨骼动画**。NPC 攻击是 vanilla zombie 挥手、玩家打坐是站着不动、战斗是挥空气。本 plan 承接已完成的 `plan-player-animation-v1` ✅ finished（KosmX PlayerAnimator 库调研、可控骨骼/easing/双路径生产设计），把设计落地为 gradle 集成 + 首批 25+ 动画 + server→client 触发协议。**零美术工具依赖**：纯 Java 代码 + LLM 生成 PlayerAnimator JSON。

**世界观锚点**：`worldview.md §四` 战斗是近战肉搏（零距离贴脸施法）→ 需要挥拳/掌击/抱摔动画 · `§三` 打坐/突破姿态 → 静坐/经脉发光 · `§五` 七流派各自身法姿态（爆脉沉腰 / 暗器微抬腕 / 涡流展掌）· `§七` NPC 道伥模仿玩家日常行为（砍树/挖矿/蹲伏假示好）

**library 锚点**：`cultivation-0002 烬灰子内观笔记 §音论`（身体动 → 灵气振荡 → 视觉可见）

**前置依赖**：
- `plan-player-animation-v1` ✅ → 技术基础/可控骨骼/easing/双路径全部锚定
- `plan-vfx-wiring-v1` 🆕 active → VFX 事件通道（动画与 VFX 在同一事件上共同触发，叠加不冲突）
- `plan-npc-visual-v1` 🆕 active → NPC 外观差异化（NPC 播动画需先有视觉区分）
- `plan-entity-model-v1` 🆕 active → 实体 BlockBench 模型（部分动画需配合自定义模型）
- `plan-combat-no_ui` ✅ + `plan-combat-ui_impl` ✅ → 攻击/受击/弹反/闪避事件垫
- `plan-cultivation-v1` ✅ → 打坐/突破事件垫
- `plan-HUD-v1` ✅ → HUD 层不挡动画视野

**反向被依赖**：
- `plan-baomai-v3` 🆕 active → 崩拳 5 招逐帧动画
- `plan-dugu-v2` 🆕 active → 蚀针弹指/扬毒粉/自蕴吞吐动画
- `plan-zhenfa-v2` 🆕 active → 布阵/激活/收阵手势
- `plan-tuike-v2` 🆕 active → 蜕壳剥落动作
- `plan-combat-gamefeel-v1` 🆕 skeleton → 受击 stagger / 闪避残影 / 弹反振臂
- `plan-breakthrough-cinematic-v1` 🆕 skeleton → 突破姿态动画
- `plan-death-rebirth-cinematic-v1` 🆕 skeleton → 死亡倒地动画
- `plan-npc-interaction-polish-v1` 🆕 skeleton → NPC 表情动画

---

## 与各 active plan 的协同

| 维度 | active plan 已做 | 本 plan 补充 |
|------|-----------------|-------------|
| 战斗视觉 | vfx-wiring-v1：命中方向粒子 + 格挡火花 | 叠加骨骼动画：挥拳/剑斩/受击后仰/格挡振臂/闪避侧翻 |
| NPC 外观 | npc-visual-v1：皮肤/装备/颜色差异 | 叠加行为动画：NPC 巡逻走/砍树/蹲伏/攻击/逃跑 |
| 实体模型 | entity-model-v1：BlockBench 模型 + BlockEntity 渲染 | 叠加实体交互动画：丹炉搅拌/锻造台锤击/灵田翻土 |
| 修炼 VFX | vfx-wiring-v1 P0：吸灵粒子 + 经脉光路 | 叠加姿态：打坐盘腿 + 突破仰天 + 经脉打通微颤 |

---

## 接入面 Checklist

- **进料**：KosmX PlayerAnimator gradle 依赖 / 可控骨骼列表（plan-player-animation-v1 已锚定：head/body/leftArm/rightArm/leftLeg/rightLeg + torso/leftForeArm/rightForeArm）/ `PlayerAnimationAccess` API / `AnimationStack.addAnimLayer` / server combat/cultivation/skill 事件
- **出料**：`BongAnimationRegistry`（client 侧动画注册表，anim_id → PlayerAnimator JSON/Java 动画）+ 复用 `bong:vfx_event` 的 `play_anim` / `stop_anim` 载荷 + `AnimationLayerManager`（多层叠加管理：上身/下身/全身/表情）+ 首批 25+ 动画 + `client/tools/gen_*.py` 动画生成器
- **跨仓库契约**：server `animation::AnimationTrigger { entity, anim_id, priority, loop }` component → emit `VfxEventPayloadV1::PlayAnim` / `StopAnim` → client `ClientAnimationBridge` → `AnimationLayerManager.play()`

---

## §0 设计轴心

- [x] **零 Blender/Blockbench**：全部 Java 代码 + JSON 生产线（`client/tools/gen_*.py` + `render_animation.py` headless 验证）
- [x] **多层叠加**：表情/轻量 idle(priority 100+) / 下半身行走(priority 500+) / 上半身战斗(priority 1000+) / 全身剧情(priority 3000+) 独立通道；生产路径由 `ClientAnimationBridge` 进入 `AnimationLayerManager`
- [x] **服务端权威触发**：server 决定何时播什么动画，client 纯表演
- [x] **animation = 表演**：不做判定、不参与网络同步（动画中 entity 照常移动/受击）
- [x] **PlayerAnimator 四大坑位**（已知，plan-player-animation-v1 调研记录）：循环单帧衰减到 defaultValue / MC 无 IK 导致 leg.pitch 断腿 / body 走 MatrixStack 非 updatePart / bend 需 bendy-lib 否则静默 no-op
- [x] **与 VFX 叠加协议**：同一事件（如 hit）同时触发 VFX（vfx-wiring-v1）+ 动画（本 plan），两套 consumer 独立消费同一 event，互不依赖

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | gradle 集成 + 复用 `bong:vfx_event` + AnimationLayerManager 生产接入 + 验证动画资产 | ✅ 2026-05-11 |
| P1 | 战斗/姿态资产 + combat/parry/hit/botany/lingtian 等已接线触发 | ✅ 2026-05-11；⏳ meditate/idle/dodge/loot/stealth/inventory 待后续 |
| P2 | NPC 行为动画 5 个 + 产出交互动画 4 个 | ✅ 资产就绪；⏳ NPC/forge/alchemy/inventory 触发待后续 |
| P3 | 流派专属 stance 7 个 + 伤口/负重 limping | ✅ 资产就绪；⏳ stance/wound 自动触发待后续 |
| P4 | 突破 4 境动画 + 死亡/重生动画 | ✅ 突破触发 + death/rebirth 资产；⏳ death/rebirth lifecycle 触发待后续 |
| P5 | 流派 vN+1 联调 + gen_*.py 生产线收口 + 饱和化测试 | ✅ 生成器/manifest/headless 核验；⏳ 截图矩阵/FPS 压测待后续 |

---

## P0 — gradle 集成 + 骨架 + 验证动画 ✅ 2026-05-11

### 交付物

1. **gradle 依赖集成**
   - `build.gradle` 追加 KosmX PlayerAnimator `dev.kosmx.player-anim:player-animation-lib-fabric:1.0.2-rc1+1.20`
   - `./gradlew test build` 编译通过
   - `fabric.mod.json` 追加 `player-animation-lib` 依赖声明

2. **`BongAnimationRegistry`**（`client/src/main/java/com/bong/client/animation/BongAnimationRegistry.java`）
   - `HashMap<String, KeyframeAnimation>` 注册表
   - `register(anim_id, path_or_builder)` / `play(entity, anim_id, priority, loop)` / `stop(entity, anim_id)`
   - 自动加载 `assets/bong/animations/*.json`（PlayerAnimator JSON 格式）
   - hot-reload 支持（开发期改 JSON 不重启）

3. **`AnimationLayerManager`**（`client/src/main/java/com/bong/client/animation/AnimationLayerManager.java`）
   - 4 层通道管理：
     - `EXPRESSION` (priority 100)：表情/头部微动
     - `LOWER_BODY` (priority 500)：行走/跑步/跳跃（仅影响 legs）
     - `UPPER_BODY` (priority 1000)：攻击/施法/采集（仅影响 head/body/arms）
     - `FULL_BODY` (priority 3000)：打坐/倒地/突破（覆盖全部骨骼）
   - 同层新动画自动替换旧动画（带 0.15s crossfade）
   - 高 priority 层覆盖低 priority 层的同名骨骼

4. **动画触发协议复用**（`server/src/network/animation_trigger.rs` + `client/src/main/java/com/bong/client/animation/ClientAnimationBridge.java`）
   - 未新增平行 `BongAnimationTriggerS2c` / `bong:animation_trigger` channel。
   - server 侧 `AnimationTrigger { target_entity, anim_id, action, priority, fade_* }` component 统一转成既有 `VfxEventPayloadV1::PlayAnim` / `StopAnim`。
   - client 侧 `ClientAnimationBridge` 按 priority range 映射 `AnimationLayerManager.Channel`，通过 `AnimationLayerManager.play()` / `stop()` 维持同语义层替换。

5. **3 个验证动画**
   - `sword_swing_right.json`：右臂横扫 90°（rightArm pitch 0→-90° + body yaw 0→20°，duration 8 tick，easing ease-out）
   - `meditate_sit.json`：全身打坐姿态（legs cross + body lower + arms rest on knees，loop，FULL_BODY priority）
   - `hurt_stagger.json`：受击后仰（body pitch -15° + head pitch -10° + 微后退，duration 6 tick，easing ease-in-out）

6. **WSLg 实跑验证**
   - `./gradlew runClient` → 进入世界 → 攻击触发 sword_swing → 打坐触发 meditate_sit → 被攻击触发 hurt_stagger
   - 确认多层叠加：上半身 swing + 下半身行走同时播放

### 验收抓手

- 测试：`client::animation::tests::registry_loads_json` / `client::animation::tests::layer_priority_override` / `server::animation::tests::trigger_component_emits_packet`
- 手动 WSLg：三个验证动画实际播放 + 多层叠加不冲突

---

## P1 — 战斗动画 + 姿态动画 ✅ 资产与已接线子集 / ⏳ 部分触发后续

### 交付物

1. **战斗动画 8 个**（`assets/bong/animations/combat/`）
   - `fist_punch_right.json`：右拳直击（rightArm pitch -70° + body lean forward 10°，5 tick，UPPER_BODY）
   - `fist_punch_left.json`：左拳钩拳（leftArm swing arc，5 tick）
   - `palm_strike.json`：双掌推出（both arms extend forward + body lean 15°，6 tick）
   - `sword_slash_down.json`：下劈（rightArm pitch 90°→-60°，8 tick）
   - `windup_charge.json`：蓄力预备（body crouch + rightArm draw back，loop 持续至 charge 完成，UPPER_BODY）
   - `release_burst.json`：蓄力释放（全身展开 + 微跳离地，4 tick，FULL_BODY）
   - `parry_block.json`：格挡（双臂交叉胸前，3 tick 闪入 + hold 10 tick + 3 tick 归位）
   - `dodge_roll.json`：闪避侧翻（body roll 360° + 位移 1 block 方向，10 tick，FULL_BODY）

2. **姿态动画 5 个**（`assets/bong/animations/posture/`）
   - `meditate_sit.json`（P0 已有 → 精修：手指细节 + 呼吸微动 body scale sin wave ±0.5%）
   - `harvest_crouch.json`：采药蹲（body lower + rightArm reach down，hold，FULL_BODY）
   - `loot_bend.json`：搜刮弯腰（body pitch 45° + arms reach forward，hold）
   - `stealth_crouch.json`：潜行伏低（body lower 50% + legs wide + head slight up，loop）
   - `idle_breathe.json`：站立呼吸（body scale sin wave ±1%，arms slight sway，loop 40 tick cycle，EXPRESSION priority）

3. **server/client 侧触发接线现实**
   - ✅ `combat` attack / defense / hit recoil 已通过 `server/src/network/vfx_animation_trigger.rs` 映射到 `fist_punch_*` / `palm_strike` / `sword_slash_down` / `parry_block` / `hurt_stagger` 等动画。
   - ✅ `botany` harvest、`lingtian` till、breakthrough、tribulation、woliu 等已接到 `AnimationTrigger` → `VfxEventPayloadV1::PlayAnim`。
   - ⏳ `combat::dodge_system` → `dodge_roll`、`cultivation::meditate_system` → `meditate_sit`、client 静止 5s → `idle_breathe`、`loot_bend` / `stealth_crouch` / `inventory_reach` 仍待后续触发计划。

### 验收抓手

- 测试：`server::combat::tests::hit_triggers_attack_animation` / `client::animation::tests::idle_breathe_starts_after_5s` / `client::animation::tests::combat_upper_body_with_walking_lower`
- 手动 WSLg：攻击 → 挥拳 + 同时走路 → 蓄力 → 释放 → 被打 → 后仰 → 格挡 → 双臂交叉 → 闪避侧翻 → 打坐 → 盘腿 → 站着不动 5s → 呼吸

---

## P2 — NPC 行为动画 + 产出交互动画 ✅ 资产就绪 / ⏳ 部分触发后续

### 交付物

1. **NPC 行为动画 5 个**（`assets/bong/animations/npc/`）
   - `npc_patrol_walk.json`：NPC 巡逻走（arms slight swing，legs walk cycle，body slight lean forward，loop 20 tick cycle）
   - `npc_chop_tree.json`：砍树（rightArm swing down repetitive，body lean，loop，用于道伥假示好 — worldview §七）
   - `npc_mine.json`：挖矿（rightArm pickaxe swing cycle，body crouch slight）
   - `npc_crouch_wave.json`：蹲伏+挥手（body lower + rightArm wave side，用于道伥假示好/NPC 招呼）
   - `npc_flee_run.json`：逃跑奔跑（arms pump + legs fast cycle + body lean forward 20°，loop）
   - NPC 复用玩家骨骼（PlayerAnimator 对 PlayerEntity 和 NPC entity 同样适用——NPC 本就是 player model reskin）

2. **产出交互动画 4 个**（`assets/bong/animations/interaction/`）
   - `forge_hammer.json`：锻造锤击（rightArm raise + slam down cycle，body lean，8 tick per cycle）
   - `alchemy_stir.json`：炼丹搅拌（rightArm circular motion，body slight lean over，loop 16 tick cycle）
   - `lingtian_till.json`：翻土（rightArm hoe swing down + body crouch，6 tick）
   - `inventory_reach.json`：背包翻找（rightArm reach to hip + body slight turn，4 tick，配合 ui-transition-animation-v1 背包打开）

3. **NPC 动画触发**
   - ⏳ NPC big-brain action 切换 → `npc_patrol_walk` / `npc_chop_tree` / `npc_mine` / `npc_crouch_wave` / `npc_flee_run` 仍待后续接线。
   - ✅ 当前基础能力已支持带 `UniqueId` 的 skinned player shell 作为动画目标；不同 archetype 复用同一动画的视觉差异化留给 npc-visual / npc-interaction 后续联调。

4. **产出动画触发**
   - ✅ `lingtian` / botany 侧已有动画触发映射。
   - ⏳ `forge::session_system` → `forge_hammer`、`alchemy::session_system` → `alchemy_stir`、`inventory_reach` 背包交互仍待后续接线。

### 验收抓手

- 测试：`server::npc::tests::patrol_action_triggers_walk_anim` / `server::forge::tests::hammer_step_triggers_anim` / `client::animation::tests::npc_uses_player_skeleton`
- 手动：遇 NPC → 巡逻走动 → 道伥蹲伏挥手 → 锻造台 → 挥锤+火星粒子 → 丹炉 → 搅拌+蒸汽

---

## P3 — 流派 stance + 伤口 limping ✅ 资产就绪 / ⏳ 自动触发后续

### 交付物

1. **7 流派战斗 stance**（`assets/bong/animations/stance/`）
   - `stance_baomai.json`：爆脉沉腰（body lower + legs wide + fists clenched at waist，loop idle，FULL_BODY）
   - `stance_dugu.json`：暗器微抬腕（body straight + rightArm wrist slight up + leftArm behind back，loop idle）
   - `stance_zhenfa.json`：阵法展掌布符（both arms extend palms forward at 45°，fingers spread，loop idle）
   - `stance_dugu_poison.json`：毒蛊藏指捻针（rightArm close to body + fingers pinch + body slight hunch，loop idle）
   - `stance_zhenmai.json`：截脉侧身蓄劲（body turn 45° + rightArm draw back + leftArm guard，loop idle）
   - `stance_woliu.json`：涡流双掌开合（both palms face each other at chest level + slow open/close cycle 40 tick，loop）
   - `stance_tuike.json`：蜕壳披壳（body slight hunch + arms wrap around torso + head slight tilt，loop idle）
   - ⏳ 触发：进入战斗 + 已装备流派 → 自动切 stance（FULL_BODY priority 覆盖 idle_breathe）仍待流派 vN+1 后续接线。

2. **伤口/负重 limping**（`assets/bong/animations/status/`）
   - `limp_left.json`：左腿伤 → 行走动画左步短/右步长（leftLeg swing amplitude ×0.6，rightLeg ×1.0，loop替换默认 walk）
   - `limp_right.json`：右腿伤 → 反向
   - `arm_injured_left.json`：左臂伤 → 左臂下垂（leftArm hang at -5° pitch，不参与 swing，覆盖 UPPER_BODY idle）
   - `arm_injured_right.json`：右臂伤 → 右臂下垂
   - `exhausted_walk.json`：虚弱走 → 全身动画 amplitude ×0.5 + body slight stagger per step
   - ⏳ 触发：`cultivation::Wounds` component 变化 → server emit 对应 AnimationTrigger 仍待 wound/gamefeel 后续接线。
   - 与 combat-gamefeel-v1 P3 配合：gamefeel 做视觉 tint + 粒子，本 plan 做骨骼姿态

3. **stance → 攻击动画衔接**
   - 从 stance idle 过渡到攻击动画：0.1s crossfade（不瞬切）
   - 攻击结束后 0.15s crossfade 回 stance idle
   - 不同流派的通用攻击（fist_punch 等）在 stance 基础上微调（爆脉 fist_punch = stance 沉腰 + 直拳；暗器 fist_punch = stance 微抬腕 + 弹指）

### 验收抓手

- 测试：`client::animation::tests::stance_activates_on_combat_with_school` / `client::animation::tests::limp_replaces_walk_on_leg_wound` / `client::animation::tests::stance_to_attack_crossfade`
- 手动：装备爆脉流 → 进入战斗 → 沉腰 stance → 出拳 → 回沉腰 → 腿受伤 → 开始跛行 → 虚弱 → 走路无力

---

## P4 — 突破 + 死亡动画 ✅ 突破触发与资产 / ⏳ 死亡重生触发后续

### 交付物

1. **突破 4 境动画**（`assets/bong/animations/breakthrough/`）
   - `breakthrough_yinqi.json`：醒灵→引气（仰头 + 双臂微张 + 全身微颤 3s → 突然平静 + 双掌合十，15 tick 主阶段 + 20 tick aftermath）
   - `breakthrough_ningmai.json`：引气→凝脉（经脉循行周身颤——body 微震 sin wave + arms 沿经脉路径微动，20 tick）
   - `breakthrough_guyuan.json`：凝脉→固元（真元凝核抱丹——双手环抱腹部丹田位置 + body 缓慢收紧，25 tick）
   - `breakthrough_tongling.json`：固元→通灵/通灵→化虚（天地共鸣——双臂缓慢展开 180° + 头仰天 + body 微浮 0.3 block 上升，30 tick，FULL_BODY 最高 priority）
   - 与 breakthrough-cinematic-v1 配合：cinematic 管 VFX/screen effect/agent narration，本 plan 管骨骼姿态

2. **死亡/重生动画**（`assets/bong/animations/death/`）
   - `death_collapse.json`：死亡倒地（body pitch forward 90° + legs crumple + arms fall，15 tick，FULL_BODY，不 loop——倒地后保持）
   - `death_disintegrate.json`：魂散消逝（body 向上微漂 0.5 block + 肢体逐渐展开 → 最终 T-pose + scale 0.5，配合 `DeathSoulDissipatePlayer` 粒子）
   - `rebirth_wake.json`：灵龛重生苏醒（从 crouching 慢慢站起 + 头环顾四周 + 身体微晃——"我还活着？"，20 tick）
   - 与 death-rebirth-cinematic-v1 配合：cinematic 管 screen shatter/overlay/fade，本 plan 管骨骼演出

3. **触发接线**
   - `cultivation::breakthrough_system`：按 realm transition 选择对应 breakthrough 动画
   - ⏳ `death_lifecycle::death_system`：死亡 → death_collapse → 1s → death_disintegrate 待后续 cinematic/lifecycle 接线。
   - ⏳ `death_lifecycle::rebirth_system`：重生 → rebirth_wake 待后续 cinematic/lifecycle 接线。

### 验收抓手

- 测试：`server::cultivation::tests::breakthrough_realm_triggers_animation` / `server::death::tests::death_triggers_collapse_then_disintegrate` / `client::animation::tests::breakthrough_animation_full_body_priority`
- 手动：突破引气 → 仰头微颤 → 成功平静 → 突破凝脉 → 周身经脉颤 → 死亡 → 倒地 → 魂散消逝 → 重生 → 缓慢站起

---

## P5 — 流派联调 + 生产线 + 饱和化测试 ✅ 生成器/测试基线 / ⏳ 完整矩阵后续

### 交付物

1. **流派 vN+1 联调 demo**
   - 给 baomai-v3 / dugu-v2 / tuike-v2 / zhenfa-v2 各提供 1-2 招动画 JSON demo
   - 确认 `SkillCastEvent` → `AnimationTrigger` → `ClientAnimationBridge` → `AnimationLayerManager.play()` 全链路通
   - 文档：每个流派 plan 如何引用 `BongAnimationRegistry` + 如何用 `gen_*.py` 生成新动画

2. **`client/tools/gen_*.py` 生产线收口**
   - 复用已有 21 个 generator 模式（`gen_fist_punch_right.py` 等）
   - 新增 `gen_stance.py`（通用 stance 生成器：输入 body_pose + arm_pose → 输出 PlayerAnimator JSON）
   - 新增 `gen_breakthrough.py`（突破动画生成器：输入 phase_sequence → 输出 JSON）
   - `render_animation.py` headless 批量回归测试：每次改动后自动渲染全部 25+ 动画截图 → diff 比对

3. **饱和化测试矩阵**
   - 25+ 动画 × 3 境界 × 2 性别皮肤 × 5 叠加场景（行走/跳跃/游泳/被打/蓄力中被打断）= 750+ 组合
   - 自动化：`scripts/animation_matrix_test.sh`（遍历关键组合 + 截图）
   - 多玩家压测：5 玩家同时播 10 个不同动画 → 帧率 > 30fps
   - 被打断恢复：每个动画在任意帧被打断 → 0.15s crossfade 回 idle → 无 T-pose / 无骨骼卡死

### 验收抓手

- 流派联调：baomai 崩拳动画 → server emit → client play → VFX 同步粒子
- gen_*.py 回归：`python client/tools/render_animation.py --batch` 通过
- 压测：5 player × 10 anim 帧率日志

---

## Finish Evidence

- **落地清单**：
  - client 既有 PlayerAnimator 基线继续复用：`client/build.gradle` / `client/src/main/resources/fabric.mod.json` 已接 `player-animation-lib`，`BongAnimationRegistry`、`BongAnimationPlayer`、`ClientAnimationBridge`、`VfxEventRouter` 已承接 `play_anim` / `stop_anim`。
  - 本 plan 新增并接入 `client/src/main/java/com/bong/client/animation/AnimationLayerManager.java`，把 `EXPRESSION` / `LOWER_BODY` / `UPPER_BODY` / `FULL_BODY` 映射到稳定 priority range；`ClientAnimationBridge`、`BongAnimCommand`、`BongPunchCombo` 均已走该入口，保证同语义层替换、跨语义层共存。
  - 本 plan 扩展 `BongAnimations.IMPLEMENTATION_V1_ANIMATIONS`，新增 39 个 PlayerAnimator JSON；`assets/bong/player_animation/` 当前共 67 个动画资源，覆盖战斗、姿态、NPC、产出交互、七流派 stance、伤口/虚弱步态、突破、死亡/重生。
  - 新增 `client/tools/gen_player_animation_implementation_v1.py`、`client/tools/gen_stance.py`、`client/tools/gen_breakthrough.py`，继续使用 `render_animation.py` 做 headless 资源核验。
  - Review 收尾：`AnimationLayerManager` 仅在 stop 成功后更新通道追踪，并在生成器基线补齐 head/torso `roll=0` 边界复位，防止 `dodge_roll` / limping 类动画残留侧倾；生产播放路径已接入 LayerManager。
  - server 新增 `server/src/network/animation_trigger.rs`：`AnimationTrigger` component 适配到既有 `VfxEventPayloadV1::PlayAnim` / `StopAnim`，并在同 tick 清除 component。
  - `server/src/network/vfx_animation_trigger.rs` 已把 combat attack / defense / hit recoil / breakthrough / tribulation / woliu / botany harvest / lingtian till 事件映射到骨骼动画；目标查询放宽到 `Position + UniqueId`，支持带 skinned player shell 的 NPC。
  - 协议决议：未新增平行 `bong:animation_trigger` CustomPayload，统一复用既有 `bong:vfx_event` 动画 payload，避免两套 server→client 表演协议并存。
- **关键 commit**：
  - `0f2290d35` · 2026-05-11 · `实现 player-animation 动画资源与分层管理`
  - `cbc563b47` · 2026-05-11 · `接入 player-animation 服务端触发适配`
  - `067e60794` · 2026-05-11 · `补强 player-animation 事件触发覆盖`
  - `e7a8e85a1` · 2026-05-11 · `修复 player-animation review 反馈`
  - `af567f34a` · 2026-05-11 · `接入 player-animation 分层播放入口`
  - `5f3c7298a` · 2026-05-11 · `补强 player-animation 分层状态同步`
- **测试结果**：
  - `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` → 3650 passed。
  - `cd server && cargo test vfx_animation_trigger && cargo test animation_trigger` → 8 passed + 10 passed，覆盖 `PlayAnim` / `StopAnim` component 适配、combat/breakthrough/botany/lingtian 映射、skinned NPC `UniqueId` 目标。
  - Java 17 (`$HOME/.sdkman/candidates/java/17.0.18-amzn`) 下 `cd client && ./gradlew test --tests "com.bong.client.animation.*"` → BUILD SUCCESSFUL。
  - Java 17 下 `cd client && ./gradlew test build` → BUILD SUCCESSFUL。
  - Review 收尾后 Java 17 下 `cd client && JAVA_HOME="$HOME/.sdkman/candidates/java/17.0.18-amzn" PATH="$JAVA_HOME/bin:$PATH" ./gradlew test build` → BUILD SUCCESSFUL。
  - LayerManager 生产接入后 Java 17 下 `cd client && ./gradlew test --tests "com.bong.client.animation.AnimationLayerManagerTest"` → BUILD SUCCESSFUL。
  - LayerManager 生产接入后 Java 17 下 `cd client && ./gradlew test build` → BUILD SUCCESSFUL。
  - LayerManager 状态同步修复后 Java 17 下 `cd client && ./gradlew test --tests "com.bong.client.animation.AnimationLayerManagerTest"` → BUILD SUCCESSFUL。
  - LayerManager 状态同步修复后 Java 17 下 `cd client && ./gradlew test build` → BUILD SUCCESSFUL。
  - `python3 -m py_compile client/tools/gen_player_animation_implementation_v1.py client/tools/gen_stance.py client/tools/gen_breakthrough.py` → pass。
  - `python3 client/tools/render_animation.py client/src/main/resources/assets/bong/player_animation/stance_baomai.json --ticks "0,10,20" -o /tmp/bong-player-animation-implementation-v1-render` → wrote `stance_baomai_grid.png`。
- **跨仓库核验**：
  - server：`AnimationTrigger`、`AnimationTriggerAction`、`emit_animation_trigger_components`、`emit_attack_animation_triggers`、`emit_breakthrough_animation_triggers`、`emit_lingtian_visual_triggers`。
  - schema：继续使用 `VfxEventPayloadV1::PlayAnim` / `StopAnim` 以及 agent schema 的 `VfxEventPlayAnimV1` / `VfxEventStopAnimV1`。
  - client：`BongAnimationRegistry`、`BongAnimationPlayer`、`ClientAnimationBridge`、`AnimationLayerManager`、`BongAnimations.IMPLEMENTATION_V1_ANIMATIONS`。
- **遗留 / 后续**：
  - 当前环境未执行 WSLg `./gradlew runClient` 手测；以 headless renderer、Java 17 build/test 和 server 单元/全量测试替代。
  - 已接线触发范围明确为：combat attack / defense / hit recoil、breakthrough、tribulation、woliu、botany harvest、lingtian till；其余资产先作为可播放资源与后续 plan 接入面。
  - 待后续触发接线：`meditate_sit`、`idle_breathe`、`dodge_roll`、`loot_bend`、`stealth_crouch`、`inventory_reach`、`npc_patrol_walk` / `npc_chop_tree` / `npc_mine` / `npc_crouch_wave` / `npc_flee_run`、`forge_hammer`、`alchemy_stir`、七流派 `stance_*` 自动切换、`limp_*` / `arm_injured_*` / `exhausted_walk` wound 步态、`death_collapse` / `death_disintegrate` / `rebirth_wake` lifecycle 触发。
  - 原文 P5 的 750+ 截图矩阵与 5-player FPS 压测未落成自动脚本，后续可独立做 `animation_matrix_test.sh` / screenshot diff plan。
  - 非 player-shell 的 GeckoLib/fauna entity 仍走独立 renderer；本 plan 的 NPC 动画覆盖带 `UniqueId` 且客户端可按 player model resolve 的 NPC。
  - 第一人称手臂、bendy-lib 肢体弯曲、死亡倒地分阶段 stop/hold 细化仍适合拆后续 polish plan。
