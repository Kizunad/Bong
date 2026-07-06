# plan-bughunt-baomai-v3-av-double-source-v1（skeleton）

> BugHunt D8 / client-combat：确认 **爆脉 v3 成功施法同时走 skill resolver 直发 A/V 与 network trigger 事件发 A/V**，同一次 cast 会产生两套动画/音频意图。此 plan 只记录 skeleton，不消费、不归档。

## Bug 摘要

爆脉 v3 的技能 resolver 在成功施法时先发送 `BaomaiSkillEvent`，随后又直接调用 `emit_audio` / `emit_anim`。网络层同时注册 `emit_baomai_v3_visual_triggers` 与 `emit_baomai_v3_audio_triggers` 消费同一个 `BaomaiSkillEvent`，因此同一次玩家技能栏施法会产生 direct + trigger 两条 A/V 发射路径。

最强可见案例是 `FullPowerCharge` / `FullPowerRelease`：

- charge direct：`charge_start` + `bong:windup_charge`
- charge trigger：`baomai_cast` + `bong:guard_raise`
- release direct：`charge_release` + `bong:release_burst`
- release trigger：`baomai_signature` + `bong:fist_punch_right`

这不是 #1018 的蜕壳主动施放视听双源；本问题发生在 `baomai_v3` 自己的 resolver 与 Baomai event trigger 上。

## 对实际游玩体验的影响

玩家从技能栏施放爆脉 v3 时，客户端会收到两套音频/动画请求。对 `FullPowerCharge` / `FullPowerRelease`，玩家可感知为起手姿态闪变、释放动作被拳击/防御通用动作抢占，以及同一招同时播放两种音色。对同 anim id 的技能，客户端不会叠无限层，但会重触发/淡入替换，连招时表现为轻微抖动或节奏不稳。

结果是玩家无法稳定从远处读出“对方正在蓄力/释放爆脉全力一击”，也会听到一次施法对应两套不一致音效，直接削弱招式 A/V 差异化。

## 证据定位

- `server/src/combat/baomai_v3/skills.rs:159-193`：`BengQuan` 成功后发送 `BaomaiSkillEvent`，随后 direct 发 `baomai_cast` 与 `bong:beng_quan`。
- `server/src/combat/baomai_v3/skills.rs:228-247`：`FullPowerCharge` 成功后发送 `BaomaiSkillEvent`，随后 direct 发 `charge_start` 与 `bong:windup_charge`。
- `server/src/combat/baomai_v3/skills.rs:295-313`：`FullPowerRelease` 成功后发送 `BaomaiSkillEvent`，随后 direct 发 `charge_release` 与 `bong:release_burst`。
- `server/src/combat/baomai_v3/skills.rs:1049-1063`：`emit_audio` 直接写 `Events<PlaySoundRecipeRequest>`，绕过 `AudioEmitWriter` 的统一去重上下文。
- `server/src/combat/baomai_v3/skills.rs:1066-1084`：`emit_anim` 直接写 `VfxEventRequest::PlayAnim`，priority 1200。
- `server/src/network/mod.rs:760`：network update 注册 `emit_baomai_v3_visual_triggers`。
- `server/src/network/mod.rs:797`：network update 注册 `emit_baomai_v3_audio_triggers`。
- `server/src/network/vfx_animation_trigger.rs:457-472`：Baomai visual trigger 读取 `BaomaiSkillEvent` 并发 `PlayAnim`。
- `server/src/network/vfx_animation_trigger.rs:538-546`：Baomai 动画映射中，`FullPowerCharge` 走 `ANIM_GUARD_RAISE`，`FullPowerRelease` 走 `ANIM_FIST_PUNCH_RIGHT`。
- `server/src/network/audio_trigger.rs:654-684`：Baomai audio trigger 读取同一事件并发音频配方，`FullPowerCharge` 走 `baomai_cast`，`FullPowerRelease` 走 `baomai_signature`。
- `server/src/network/vfx_event_emit.rs:225-227`：合批只合并 `SpawnParticle`，`PlayAnim` / `StopAnim` 不合并。
- `server/src/network/audio_event_emit.rs:89-134`：每个 `PlaySoundRecipeRequest` 独立编码并发送到客户端。
- `client/src/main/java/com/bong/client/animation/BongAnimationPlayer.java:148-156`：同 anim id 重触发会在现有层上 replace/fade，不新增层；因此同 id 情况是重启/淡入抖动，不是无限叠层。
- `client/src/main/java/com/bong/client/animation/AnimationLayerManager.java:142-150`：同 channel 不同 anim 会 stop 旧 anim 再 play 新 anim；因此 full_power 的不同 anim 更像动作抢占/闪变。

## 触发路径

1. 玩家按 1-9 技能栏键触发 `skill_bar_cast`。
2. 服务端 `handle_skill_bar_cast` 查技能绑定、KnownTechniques 与 `SkillRegistry`，成功后通过 `Commands.add` 执行对应 resolver。
3. `SkillRegistry::init_registry` 注册 `combat::baomai_v3::register_skills`，爆脉 v3 resolver 被生产路径调用。
4. resolver 成功分支发送 `BaomaiSkillEvent`，并在同一分支 direct 发音频/动画。
5. network update 的 Baomai visual/audio trigger 读取同一 `BaomaiSkillEvent`，再次发动画/音频。
6. 客户端收到两套请求；full_power 因 direct 与 trigger 的 anim/recipe 不同，表现最明显。

## 反方审查记录

### Round 1

反方尝试推翻点：

- `PlayAnim` 可能被客户端同 id replace，不会叠层。
- trigger 与 direct 的调度可能不是同 tick。
- direct 路径可能只是旧测试路径。

结论：未推翻。措辞需收窄：不要写“粒子重复”，因为 Baomai visual trigger 当前只发 `PlayAnim`；不要泛化写“两层动画叠加”，同 id 会 replace/fade。高风险事实仍成立：同一次成功 cast 会发 direct + trigger 两套 A/V 请求，full_power 的 direct/trigger 动画和音频配方不同。

### Round 2

反方继续检查调度、生产路径与音频去重：

- `commands.add` 可能让 resolver 延后，trigger 可能同 tick 或下一次 update 读到事件，但 Bevy 事件会跨 frame 保留，未见永久漏读路径。
- `skill_bar_cast` 是玩家生产路径，不是测试专用；`baomai_v3::register_skills` 已进 `SkillRegistry`。
- `AudioEmitWriter` 的 dedup 只覆盖 trigger 路径；`skills.rs::emit_audio` 直接写 `Events<PlaySoundRecipeRequest>`，绕过该上下文。
- 客户端音频播放未见按 recipe 兜底去重。

结论：第二轮仍不能推翻。高置信确认“同一次 Baomai v3 成功 cast 会产生 direct + trigger 双 A/V 请求”；中高置信认为玩家可感知为重复音效、动画重触发或 full_power 动画抢占。

## Skeleton Fix Plan

### P0 — 统一 Baomai v3 A/V 单一来源

- [ ] 选择 Baomai v3 的权威 A/V 发射源。建议让 network trigger 统一拥有动画与音频，移除/禁用 `skills.rs` 中的 direct `emit_audio` / `emit_anim`。
- [ ] 不要顺手删除 direct particle。当前 Baomai visual trigger 只补动画，没有替代 direct `SpawnParticle`；若要迁移粒子，必须先在 trigger 中补齐每招独立粒子映射。
- [ ] 对 `FullPowerCharge` / `FullPowerRelease` 统一最终动画 id 与音频 recipe，保证与设计中的蓄力/释放语义一致，不被通用 `guard_raise` / `fist_punch_right` 抢占。
- [ ] 检查 `BengQuan` / `MountainShake` / `BloodBurn` / `Disperse`，确保每招保留独立 animation、particle/VFX、SFX、HUD 反馈、hotbar icon 接线，不因去重修复破坏招式可辨识度。

### P1 — 回归防线

- [ ] 增加 server 侧成功 cast 回归：一次 `FullPowerCharge` 不会同时产生 `bong:windup_charge` 与 `bong:guard_raise`，音频不会同时产生 `charge_start` 与 `baomai_cast`。
- [ ] 增加 server 侧成功 cast 回归：一次 `FullPowerRelease` 不会同时产生 `bong:release_burst` 与 `bong:fist_punch_right`，音频不会同时产生 `charge_release` 与 `baomai_signature`。
- [ ] 覆盖同 id 技能：一次 `BengQuan` 不应发两条相同 `bong:beng_quan` animation 请求，避免 replace/fade 重启抖动。
- [ ] 覆盖客户端 A/V 差异化验收：技能栏施放爆脉 v3 各招时，远距离仍能区分招式，音频不出现双配方错配。

## 验收测试计划

- server：在 `server/` 跑 `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`。
- client：若改到客户端接线或资源注册，在 `client/` 且 JDK 17 环境下跑 `./gradlew test build`。
- 联调：跑 `bash scripts/smoke-test-e2e.sh`，并设置 `BONG_SKIP_SKIN_PREFETCH=1`。
- 手动/录屏验收：技能栏连续施放 `FullPowerCharge` → `FullPowerRelease`，确认不出现 guard raise / fist punch 抢占，不出现同一 cast 双音色。

## 风险

- 删除 direct A/V 时若误删 direct particle，会让 Baomai 现有粒子反馈消失；必须把粒子与动画/音频分开处理。
- trigger 与 resolver 的调度可能存在 tick 边界差异，测试断言应围绕“同一次成功 cast 的最终请求集合”，不要硬编码必须同 tick。
- full_power 当前两套映射暴露出语义不一致：修复时需要确认最终采用专属爆脉动画，避免退回通用防御/拳击动作。
- 本 plan 只新增文档 skeleton，未修改生产代码；实际修复需另开实现 PR。
