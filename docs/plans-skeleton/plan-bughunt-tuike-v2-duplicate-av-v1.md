# plan-bughunt-tuike-v2-duplicate-av-v1

> **Active skeleton**。一句话主题：`tuike.don` / `tuike.shed` / `tuike.transfer_taint` 主动施放成功时，`skills.rs` 内联 A/V 与生产 `emit_tuike_v2_visual_triggers` / `emit_tuike_v2_audio_triggers` 同时发射，导致蜕壳三招出现同一动画重播、同一音效双响；粒子在服务端合批后不一定双包，但会有双源导致的数量偏高风险。

> 立项动机：本轮 client-combat 搜查范围要求检查 combat/skills/cast animation/VFX/SFX/HUD/icon registry/packet bridge，并避开 #987 / #997 / #1002 / #1012。该问题落在蜕壳 v2 主动技能 A/V 接线，非 Dugu 既有缺口，不涉及实际代码修改。

## Bug 摘要

- **高置信 bug（report-only）**：蜕壳 v2 三招的主动 cast 路径存在双 A/V 所有者。
- `cast_don` 成功后先发送 `DonFalseSkinEvent`，随后同一函数内又内联发送 `SpawnParticle`、`don_skin_low_thud`、`bong:tuike_don_skin`。
- `cast_shed` 成功后经 `shed_outer_layer(..., active=true, ...)` 发送 `FalseSkinSheddedEvent`，随后同一主动 cast 函数又内联发送 `SpawnParticle`、`shed_skin_burst`、`bong:tuike_shed_burst`。
- `cast_transfer_taint` 成功后先发送 `ContamTransferredEvent`，随后同一函数内又内联发送 `SpawnParticle`、`contam_transfer_hum`、`bong:tuike_taint_transfer`。
- 生产网络层又注册了 `emit_tuike_v2_visual_triggers` 和 `emit_tuike_v2_audio_triggers`，它们会消费上述领域事件，再按 `event.visual.animation_id` / `particle_id` / `sound_recipe_id` 发同一套动画、粒子、音效。

## 对实际游玩体验的影响

- 玩家按下着壳 / 蜕一层 / 转移污染时，会听到同一技能音效短时间重复触发，尤其 `shed_skin_burst` 与 `contam_transfer_hum` 会显得像服务器抖动或重复施法。
- 同一 `PlayAnim` 会在客户端同层重放并重新淡入，不会叠成两层，但会重置动作起帧，体感上可能表现为蜕壳动作抽动、起手被重复拉回。
- 粒子不会稳定发两个独立包，因为 `SpawnParticle` 有同 tick 合批；但双源仍会把同一 origin/bin 的 count 累加，导致蜕壳尘爆或上古皮光效比设计更浓。
- 这会破坏招式 A/V 差异化的可信度：玩家看到/听到的不是“三招各自清晰反馈”，而是某些招式像被重复执行。

## 证据定位

- `server/src/combat/tuike_v2/skills.rs:99-122`：`cast_don` 先 emit `DonFalseSkinEvent`，再内联 `emit_vfx` / `emit_audio` / `emit_anim`。
- `server/src/combat/tuike_v2/skills.rs:156-174`：`cast_shed` 主动调用 `shed_outer_layer(..., active=true, ...)` 后，再内联发送同一招 A/V。
- `server/src/combat/tuike_v2/skills.rs:335-352`：`shed_outer_layer` 构造 `FalseSkinSheddedEvent` 并 `emit_if_present`。
- `server/src/combat/tuike_v2/skills.rs:256-291`：`cast_transfer_taint` 先 emit `ContamTransferredEvent`，再内联发送转移污染 A/V。
- `server/src/combat/tuike_v2/skills.rs:606-657`：内联 helper 分别向 `Events<VfxEventRequest>` 写 `SpawnParticle` / `PlayAnim`，并向 `Events<PlaySoundRecipeRequest>` 写音效。
- `server/src/combat/tuike_v2/events.rs:41-69`：`TuikeSkillVisual::for_skill` 钉住三招 `animation_id` / `particle_id` / `sound_recipe_id`，与内联路径同名。
- `server/src/network/vfx_animation_trigger.rs:549-595` 与 `:1419-1452`：生产 trigger 消费三类 Tuike 事件，再发 `PlayAnim` + `SpawnParticle`。
- `server/src/network/audio_trigger.rs:900-959`：生产 trigger 消费三类 Tuike 事件，再按 `event.visual.sound_recipe_id` 发音效。
- `server/src/network/mod.rs:762,798`：生产 app 注册 Tuike visual/audio trigger。
- `server/src/network/vfx_event_emit.rs:227-260`：粒子会按 `event_id + origin_bin` 合批，非粒子请求不合并。
- `server/src/audio/implementation.rs:14-36` 与 `server/src/network/audio_trigger.rs:1161-1177`：音频 dedup 只包在 trigger 的 `AudioEmitContext` 路径；`skills.rs` 内联 `emit_audio` 直接写 request，绕过该 dedup。
- `client/src/main/java/com/bong/client/animation/BongAnimationPlayer.java:148-158`：同 animId 重触发会替换现有层并重放，不会被静默丢弃。

## 触发路径

1. 玩家在技能栏绑定并成功施放 `tuike.don`、`tuike.shed` 或 `tuike.transfer_taint`。
2. resolver 在 `server/src/combat/tuike_v2/skills.rs` 内写入领域事件。
3. resolver 同一成功路径继续内联发 `VfxEventRequest` / `PlaySoundRecipeRequest`。
4. 本 tick 稍后生产网络系统消费领域事件：
   - `emit_tuike_v2_visual_triggers` 再发同一动画和粒子。
   - `emit_tuike_v2_audio_triggers` 再发同一音效。
5. 客户端收到重复音效与重复动画播放请求；粒子路径可能合批为偏高 count。

## 反方审查记录

### Round 1

- **反方论点**：候选可能不成立，因为粒子有同 tick 合批，动画同 id 不会叠层，音频可能有 dedup。
- **裁决**：部分采纳但不推翻。粒子从“必然双包”降级为“合批后 count 偏高风险”；动画不会叠层但会重播/重置淡入；音频 dedup 只包 trigger 路径，内联音频直接排队，主问题仍成立。

### Round 2

- **反方论点**：内联 A/V 可能是本地即时反馈，trigger 可能是旁观者广播；主动 shed 与被动 shed 共用事件，删 trigger 可能误伤被动反馈。
- **裁决**：候选仍成立。内联音频使用 `AudioRecipient::Radius`，内联 VFX/anim 也走广播半径，不是 self-only；因此两条都是广播链。主动/被动 shed 共用 `FalseSkinSheddedEvent`，所以后续修复不能粗暴删除 trigger，必须保留领域事件与被动 shed 反馈，只收敛主动 cast 的双源 A/V。

## Skeleton Fix Plan

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 蜕壳主动施放 A/V 单一所有者 | fix_pr | ⬜ |

### P0 - 蜕壳主动施放 A/V 单一所有者

- **建议修复方向**：以 `emit_tuike_v2_visual_triggers` / `emit_tuike_v2_audio_triggers` 为 canonical A/V 所有者。
- 从 `cast_don`、`cast_shed`、`cast_transfer_taint` 主动成功路径移除内联 `emit_vfx` / `emit_audio` / `emit_anim`。
- 保留 `DonFalseSkinEvent`、`FalseSkinSheddedEvent`、`ContamTransferredEvent`；这些事件仍是 agent 叙事、被动/维护蜕落反馈、生产 A/V trigger 的 source of truth。
- 不改客户端动画 dedup、不改全局 `VfxEventRequest` 合批、不改全局 audio dedup；这是 Tuike 双源接线问题，不是底层通用系统问题。
- 对 `FalseSkinSheddedEvent` 特别注意：主动 `active=true` 与维护/被动 `active=false` 共用事件，修复时不能让非主动蜕落失去视觉/音频反馈。

## 验收测试计划

- server 单测：主动 `cast_don` 在生产 trigger 系统运行后，每次成功 cast 只产生一条 `PlayAnim(bong:tuike_don_skin)` 与一条 `don_skin_low_thud`。
- server 单测：主动 `cast_shed` 只产生一条 `PlayAnim(bong:tuike_shed_burst)` 与一条 `shed_skin_burst`，同时 `FalseSkinSheddedEvent.active == true` 仍保留。
- server 单测：主动 `cast_transfer_taint` 普通/上古分支只产生一条 `PlayAnim(bong:tuike_taint_transfer)` 与一条 `contam_transfer_hum`。
- server 单测：维护真元不足或被动蜕落产生的 `FalseSkinSheddedEvent(active=false)` 仍能通过 trigger 发出 shed A/V。
- 粒子回归：断言修复后不会出现 resolver 内联 + trigger 双源未合批请求；如走合批断言，关注 count 不因双源翻倍。
- client 回归：不需要改客户端，但可用现有 VFX/audio payload fixture 验证 Tuike 三招仍能播放对应动画、粒子、音效。

## 风险

- 粗暴删除 `emit_tuike_v2_visual_triggers` / `emit_tuike_v2_audio_triggers` 会误伤被动 shed 与维护 shed，也会绕开领域事件到 A/V 的统一接线。
- 只在客户端做 dedup 会掩盖 server 双源问题，并可能影响其他技能的合法重复播放。
- 只依赖粒子合批会遗漏音频双响与动画重播；本 bug 的验收重点应放在 `PlayAnim` 与 `PlaySoundRecipeRequest` 数量。
- 如果未来想保留“本地即时反馈 + 旁观者广播”模式，必须先把内联路径改成真正 self-only，并明确旁观者不再收到重复广播；当前代码不是这种设计。

## 审计来源

BugHunt D5（client-combat 第五轮，`bughunt/20260706-r05-client-combat`）。已先查开放 PR，未发现 Tuike A/V 重复相关候选；已避开 #987 / #997 / #1002 / #1012。按本轮协议，本次只新增 skeleton plan，不消费、不归档 plan，不修改实际代码/配置/依赖/资源。
