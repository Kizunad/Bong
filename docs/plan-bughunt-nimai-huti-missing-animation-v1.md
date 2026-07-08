# plan-bughunt-nimai-huti-missing-animation-v1

> Skeleton Plan。主题：`burst_meridian.ni_mai_hu_ti`（逆脉护体）已经是 runtime 可施放的 SkillBar 技能，但成功施放路径没有发任何玩家动画，只剩护体粒子 / SFX / 通用减伤状态，违反战斗招式“每招独立 animation + particle/VFX + SFX + HUD + icon”的 AV 红线。

## 一句话 bug

逆脉护体成功施放时 `BurstAv.anim_id = None`，`emit_burst_av` 因此不会发送 `VfxEventPayloadV1::PlayAnim`，玩家和旁观者看不到任何独立护体姿态或施法动作。

## 实际游玩体验影响

- 已学账号或开发 / 测试服 `dev-techniques` 玩家把 `burst_meridian.ni_mai_hu_ti` 绑定到 1-9 后，按键施放只看到护体粒子、听到嗡音、获得通用 `DamageReduction` 状态；角色模型本身不做动作。
- 远处旁观者无法从玩家姿态判断“对方正在开逆脉护体”，只能依赖较短暂且复用崩拳事件 ID 的粒子反馈，战斗读招弱。
- 防御技不应复用崩拳出拳动画，但当前实现从“不能复用攻击动画”滑成“没有动画”，不满足每招独立 animation 契约。

## 复现路径

1. 使用已有 `KnownTechniques` 账号，或在开发 / 测试服启用 `dev-techniques`，确保玩家已学 `burst_meridian.ni_mai_hu_ti`。
2. 在功法面板把“逆脉护体”绑定到 1-9 技能栏。
3. 让角色达到 `Solidify`，心包经满足要求，真元不少于 45。
4. 按绑定槽位施放。
5. 观察结果：服务端成功扣真元、撕心包经、施加 `DamageReduction`、发粒子和音效，但 VFX 事件集合中没有 `PlayAnim`，客户端没有玩家动画。

## 根因证据

- `server/src/cultivation/known_techniques.rs:290` 定义 `burst_meridian.ni_mai_hu_ti`，含 cast/cooldown/icon/category，属于可绑定技能定义。
- `server/src/cultivation/burst_meridian.rs:139` 注册 `NI_MAI_HU_TI_SKILL_ID -> resolve_ni_mai_hu_ti`；`server/src/cultivation/skill_registry.rs:247` 注释说明这批 burst_meridian 招式已从 skeleton 进入 resolver 注册 + declare 状态。
- `server/src/cultivation/burst_meridian.rs:630` 成功路径调用 `emit_burst_av`，但 `BurstAv { anim_id: None, ... }`，注释写明“仅护体粒子环 + 嗡音”。
- `server/src/cultivation/burst_meridian.rs:914` 的 `emit_burst_av` 只有 `Some(anim_id)` 才发送 `VfxEventPayloadV1::PlayAnim`。
- `client/src/main/java/com/bong/client/network/BurstMeridianHandler.java:7` 只解析并记录 `burst_meridian_event`，没有把 `ni_mai_hu_ti` 转成 stance/animation。
- `client/src/main/resources/assets/bong/player_animation/` 目前没有 `ni_mai_hu_ti` 专属动画资源。

## 修复计划骨架

- [ ] 为逆脉护体补专属玩家动画，例如 `bong:ni_mai_hu_ti`：双臂内收护住胸腹、躯干下沉、短促逆转真元姿态；防御技可短动画或短循环，但不得复用崩拳出拳。
- [ ] 在 `resolve_ni_mai_hu_ti` 的 `BurstAv` 中接入专属 `anim_id`，保证成功施放发 `PlayAnim`。
- [ ] 审计 `NI_MAI_HU_TI_PARTICLE_ID` 复用 `bong:burst_meridian_beng_quan` 是否仍可接受；若粒子也需要独立，追加专属 event id 与 client player 注册。
- [ ] 维持现有 `DamageReduction` 状态和 `zhenmai_shield_hum` 音效，不改变真元/经脉/守恒逻辑。

## 验证计划

- [ ] server 单测：`resolve_ni_mai_hu_ti` happy path 的 `VfxEventRequest` 必须同时包含 `PlayAnim{anim_id="bong:ni_mai_hu_ti"}` 和 `SpawnParticle`。
- [ ] client 资源测试：`bong:ni_mai_hu_ti` 动画 JSON 可被 `BongAnimationRegistry` 命中；若是循环动画，按 PlayerAnimator 约束补 endTick 同值 keyframe。
- [ ] 回归测试：低境界、真元不足、心包经断、冷却中拒绝路径不得发动画/粒子/SFX。
- [ ] 视觉验收：第一人称和第三人称都能看出护体姿态；旁观者能区分“逆脉护体”不是崩拳、血崩步或普通减伤状态。

## 对抗复核结论

对抗 subagent 两轮复核后结论：接受，但按 minor / plan_skeleton 处理。理由是 `anim_id: None` 到“无 PlayAnim”的链路硬，通用 HUD/粒子/SFX 不能替代每招独立 animation；降级原因是自然学习来源未证明，只能收窄为 dev-techniques / 已学账号可复现，且当前不是完全无反馈。
