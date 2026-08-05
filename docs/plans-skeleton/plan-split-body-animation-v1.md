# plan-split-body-animation-v1：上下半身分离动画体系

玩家动画拆成「下半身跟移动状态 / 上半身跟视角 + 播招式」两层独立驱动，双锏作为首个落地武器。

| 阶段 | 状态 | 内容 |
|------|------|------|
| P0 分层机制 + 首批资产 | ✅ 2026-08-06 | 库语义验真、四条步态、视角跟随层、双锏两招动画、client 接线 |
| P1 双锏招式 A/V 全套 | ⬜ | server SkillDef + 粒子/SFX/HUD/icon，按招式 A/V 红线交付 |
| P2 老动画分身化 | ⬜ | 现网 141 条全身动画迁到分身规范 + 机械约束 allowlist 清零 |
| P3 上半身触发接线 | ⬜ | 招式事件 → UPPER_BODY 通道，与视角跟随层的接管/让位 |
| P4 真机验收 | ⬜ | runClient 校准 bend 方向、FPV 可见性、分层混合观感 |

---

## P0 分层机制 + 首批资产 ✅ 2026-08-06

**库语义验真**（读 PlayerAnimator `1.0.2-rc1+1.20` 源码，非猜测）：

- `AnimationStack.get3DTransform` 链式透传：`value0 = layer.get3DTransform(..., value0)`，低 priority 先算、高 priority 后覆盖
- `KeyframeAnimationPlayer.Axis.getValueAtCurrentTick` 尾部 `return currentValue` ⇒ **透传粒度到 axis**，不写的轴原样放行下层
- `findBefore` 的 `pos == -1` 分支返回 `defaultValue` ⇒ **每个用到的 axis 必须在 tick 0 写帧**，否则首帧前把下层踩成 0
- 可 bend 部位 = `torso`/`body`/双臂/双腿；`head` 与 item 槽不可 bend；`torso` 实际 bend = `torso.bend + body.bend`（`AnimationApplier`）

**交付物**：

| 类别 | 落点 |
|------|------|
| 下半身步态 | `client/src/main/resources/assets/bong/player_animation/lower_{walk,jog,sprint,dash}.json`，生成器 `client/tools/gen_lower_body_gait.py`（`assert_lower_only` 挡住写 arm/torso/head；步态周期强制被 4 整除，否则整数 tick 会把四相位挤歪） |
| 上半身招式 | 同目录 `jian_{stance_high_low,draw_waist,waist_spin_cross,dual_smash,dual_sweep}.json`，生成器 `client/tools/gen_jian_dual_strikes.py`（`assert_upper_only`：不许写 leg，body 仅允许 y/z） |
| 档位判定 | `client/.../animation/GaitSelector.java`（纯函数）+ `LowerBodyGaitController.java`（tick 接线，档位变了才换动画） |
| 视角跟随 | `client/.../animation/UpperBodyViewPitchLayer.java`（procedural `IAnimation`，priority 700，写 `torso.bend`，常态 15°／持械 40° 分档） |
| 预览工具 | `scripts/models/render_player_pose.py`（真实 cuboid + bend 变形 + emotecraft JSON 逐帧 + `--with-jian` 挂武器）、`render_jian_in_hand.py`（关节层级摆位） |
| 武器模型 | `local_models/BambooJian.bbmodel` / `BambooJianSingle.bbmodel` / `JianPlayer.bbmodel` / `JianPlayerAnim.bbmodel`（含内嵌动画），生成器 `scripts/models/gen_bamboo_jian.py` / `gen_jian_player.py` / `gen_jian_player_anim.py` |
| 测试 | `GaitSelectorTest`（10）+ `UpperBodyViewPitchLayerTest`（11） |

**架势形制**（参考实拍定型）：一高一低分持——右臂高举、左臂低位内收，两把锏的尖端在
身前汇聚（间距 ~6px），眼睛→锏尖是一条约 25° 的下斜线。关键认识是**锏沿小臂走**，
所以"抬高手臂"不等于"锏朝上"，得靠肘深弯（`bend≈100`）+ bend 朝前（`axis=0`）把小臂
折回前下方。参数由数值搜索定出（约束：两尖间距最小 + 尖端在身前 + 低于眼），非手调。

**幅度纪律**：躯干拧转 ~35° 就够读出发力，再大从"拧腰"变成"扭麻花"。曾到 ±58° 被打回，
统一收敛为 `torso.yaw×0.58 / torso.pitch×0.72 / 大幅 arm.yaw×0.72`。

### 血泪坑：bbmodel 预览与 emotecraft 语义的四道符号差（2026-08-06）

预览连续几版"动作相对脸朝向是反的"，根因是四个独立问题叠加，**全在预览侧，emotecraft
数据始终正确**（用真机验收过的 `fist_punch_right` 验证：impact 帧手心 z=-9.00 朝 -Z 脸侧）。

1. **Blockbench 有两套并存的旋转语义**：静态 `group.rotation` 字段是标准右手系，但
   **动画关键帧**走 Bedrock 约定——`BoneAnimator` 应用时 **X/Y 取反、Z 不取反**。
   拿静态语义烘动画 ⇒ 每个旋转都反向。position 通道同理（X 取反）。
2. **多轴写进同一 group** ⇒ 欧拉顺序有解释空间。拆成嵌套单轴（内 pitch → 中 yaw →
   外 roll，与 MC `rotationZYX` 的作用次序一致）后无从歧义。
3. **锏的挂载假设不一致**：bbmodel 里锏 group 带静态腕角，而渲染器让锏沿小臂延长线
   （架势参数按后者搜的）⇒ 锏凭空多转 105°。统一为"沿小臂"。
4. **锏 +Y 是柄尾→锏尖，手臂 cuboid 从 pivot 向 -Y 长** ⇒ 腕角归零时锏尖指向小臂反
   方向，需绕 X 翻 180° 才同向。

**教训**：自己的渲染器与自己的烘焙互相验证是伪验证（共用同一套假设，自洽但整体可能全
错）。定位手段是 Playwright 驱动 web.blockbench.net，读 **group 的 `mesh.matrixWorld`**
变换已知向量、比对脸朝向与肢体朝向是否同侧——注意 **cube mesh 的世界位置不可当判据**
（Blockbench 对 cube 另有内部变换，第一次就是被它带偏的）。

**预览合成**：`lower_*` 按契约不写手臂，Blockbench 里播它们时手臂停在零姿态、锏垂下去，
读作"握法变了"。烘焙时给纯下半身动画补一份恒定的架势上半身轨道——**只补 bbmodel 预览，
emotecraft 源文件不动**，契约不破。

**档位映射**（2026-08-06 决定：复用 vanilla + 速度倍率，不动 server/schema）：
`DASH`（`MovementAction.DASHING`）> `SPRINT`（`currentSpeedMultiplier > 1.35`）> `JOG`（`player.isSprinting()`）> `WALK`（有水平位移）> `NONE`（静止／离地）。

## P1 双锏招式 A/V 全套 ⬜

P0 只出了动画本体，按招式 A/V 红线这不算「实装」。需要：

- server：`SkillDef` 双锏两招（id、cast_ticks、resolver），`icon_id` 双端镜像
- 每招独立的 particle/VFX + SFX（走 `audio_recipes` 架构，server 权威 emit）
- HUD 反馈 + hotbar/SkillBar 槽位 PNG icon（`/gen-image item`，路径 `client/.../textures/skill/<style>/<skill_id>.png`）
- `cast_ticks ↔ 动画时长` 对拍（`AnimCastTicksAlignmentTest`）
- 验收含视觉/听觉差异化回归：玩家能从远处分辨「对面在用 smash 不是 sweep」

## P2 老动画分身化 ⬜

现网 141 条动画全是七部位全写的全身动画（`dash_forward`/`idle_breathe`/`stance_*` 实测确认），播放时会盖掉下半身步态。

- 新增机械约束测试：上半身通道动画不许写 `leftLeg/rightLeg`，下半身通道不许写 `arm/torso/head`
- 老资产先进 allowlist，逐批迁移；allowlist 清零 = P2 完成判据
- 迁移优先级：常驻类（`stance_*`/`idle_breathe`）> 战斗类 > 一次性演出

## P3 上半身触发接线 ⬜

- 招式事件 → `AnimationLayerManager.Channel.UPPER_BODY` 播放（现有 `ClientAnimationBridge` 已具备通道派发能力）
- 与 `UpperBodyViewPitchLayer`（priority 700）的接管关系：招式写 `torso` 即自然压过视角跟随；招式结束后 fade 回视角跟随的验收
- 「持械」判定收窄：当前是「主手非空」占位，待武器注册表统一出口后替换

## P1 附：拔锏的腰间挂载 ⬜

`jian_draw_waist` 起手是"手空垂"，游戏里那一刻锏应当还插在腰间、拔出瞬间才切到手上。
需要腰间挂载点（marker 或独立渲染层）+ 拔出时机的模型切换；预览里没有挂载点，锏只能
一直挂在手上，看着像"从腰边把手抽出来"。

## P4 真机验收 ⬜

- `./gradlew runClient` 校准 `UpperBodyViewPitchLayer.BEND_AXIS_RAD`（headless 渲染器是 bendy-lib 语义的近似，接缝处按直角断口画，方向符号需实测）
- FPV 下 `FirstPersonMode` 与本层的相互作用（本层默认 `NONE`，第一人称不生效——需确认是否符合预期）
- 四档步态 + 视角跟随 + 招式三层同时在场的混合观感
- 步态切档瞬间的 fade 衔接（当前用 `DEFAULT_FADE_IN/OUT_TICKS`）
