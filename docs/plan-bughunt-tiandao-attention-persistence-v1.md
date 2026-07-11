# plan-bughunt-tiandao-attention-persistence-v1

> BugHunt persistence r08。仅记录真实 bug 与修复计划，不做实际修复。

## 一句话 bug

`TiandaoAttention` 是玩家权威追猎状态，但没有进入玩家持久化 slice；玩家断线重连或服务器重启后会由 `attach_tiandao_attention` 重新插入默认值，清空天道注意力、响应阶段、响应冷却、峰值与叙事计数。

## 实际游玩体验影响

- 高境界玩家把天道注意力打到 Watch / Pressure / Tribulation / Annihilate 后，只要断线重登或等服务器重启，就会回到 `level=0,response=None`，等于用重登洗掉“被天道盯上”的长期压力。
- 已经触发的天道响应冷却会被清空：`last_response_tick` / `last_emitted_response` / `narration_count` 丢失后，重登后的音效、VFX、叙事请求和事件触发节奏不再承接旧状态。
- 领地影响力把 `TiandaoAttention.level` 作为“越被天道盯越显眼”的驻守加速输入；重登清零会让高风险霸主在区域争夺里短暂变成低显眼度目标。
- NPC 恐惧 scorer 读取玩家 `TiandaoAttention.response` 给 Watch / Pressure / Tribulation / Annihilate 加恐惧权重；重登后 NPC 对刚被天道追猎的玩家立刻按普通玩家评估。

## 证据

- `server/src/world/tiandao_hunt.rs:52-61`：`TiandaoAttention` 是 ECS `Component`，字段包含 `level`、`response`、`last_eval_tick`、`peak_level`、`last_response_tick`、`last_emitted_response`、`narration_count`。
- `server/src/world/tiandao_hunt.rs:64-75`：`Default` 把注意力、峰值、冷却和叙事计数全部归零，`response` 设为 `None`。
- `server/src/world/tiandao_hunt.rs:496-502`：`attach_tiandao_attention` 对 `With<Client> + With<Cultivation> + Without<TiandaoAttention>` 玩家直接插入默认 `TiandaoAttention`。
- `server/src/world/tiandao_hunt.rs:719-735`：每次评估会推进 `level`、`accumulation_rate`、`peak_level`、`response`、`last_eval_tick`，说明这些不是纯展示缓存。
- `server/src/world/tiandao_hunt.rs:948-1001`：响应链依赖 `last_response_tick`、`last_emitted_response`、`narration_count` 控制重复响应间隔，并在 Watch 时抽 zone 真元。
- `server/src/player/state.rs:155-168`：`LoadedPlayerSlices` 只有 state、position、dimension、inventory、lifespan、coffin、skill、known_techniques、ui_prefs，没有 `TiandaoAttention`。
- `server/src/player/state.rs:419-543`：`load_player_slices` 只加载 core、slow、inventory、lifespan、skill、known techniques、UI prefs。
- `server/src/player/mod.rs:460-593`：关服 flush 查询和保存 cultivation/player slices/known techniques，不查询也不保存 `TiandaoAttention`。
- `server/src/player/mod.rs:596-750`：周期 autosave 只覆盖 core、slow/UI、cultivation、lifespan 等 slice，没有天道注意力 slice。
- `server/src/world/territory.rs:195-197` 与 `:327`：区域影响力用 `TiandaoAttention.level` 影响驻守显眼度。
- `server/src/npc/brain/scorers_survival.rs:310-340`：NPC 恐惧 scorer 用 `TiandaoAttention.response` 加权玩家威胁。

## 复现思路

1. 让一名高境界玩家在高灵气 zone 修炼或战斗，等待 `TiandaoAttention.level` 进入 Watch / Pressure 以上，并观察 `bong:tiandao_presence` payload。
2. 断线重连，或触发服务器正常关服 flush 后重启。
3. 玩家重新登录后，`attach_tiandao_attention` 因实体缺少该组件插入默认值。
4. 下一次 `tiandao_hunt_tick` 下发的 presence payload 中 `level` 从 0 重新累计，`response` 回到 `none`；领地和 NPC scorer 也读取到清零后的组件。

## 去重

- 不重复 #1052：该题是化虚动作冷却持久化绝对 tick，本题是天道追猎注意力组件。
- 不重复 #1058：该题是散真元珠埋设态重启丢失，本题不涉及 zhenfa buried bead。
- 不重复 #1064：该题是长期消耗品 `StatusEffects` 未持久化，本题是 `TiandaoAttention`。
- 不重复 #1078：该题是 RecipeUnlockState 关服未强制 flush，本题不是配方解锁。
- 不重复 #1084：该题是物资棺冷却重启回滚，本题不涉及 supply coffin。
- 不重复 #1068-#1072：这些 PR 覆盖 bot playtest 批修、worldgen 预算锚、濒死 UX 骨架、bot 产用 e2e 等，没有处理 `TiandaoAttention` 玩家持久化。
- 与已有 `docs/plan-bughunt-status-effects-consumable-persistence-v1.md` 相邻但不同：那个 plan 明确针对可消耗品写入的长期 `StatusEffects`；本 plan 针对天道追猎系统的权威注意力、响应冷却和下游 AI/领地输入。

## 修复 TODO

- [ ] 为 `TiandaoAttention` 增加玩家持久化 slice，保存 `level`、`response`、`last_eval_tick`、`accumulation_rate`、`peak_level`、`last_response_tick`、`last_emitted_response`、`narration_count`。
- [ ] 登录 hydrate 时优先插入已加载的 `TiandaoAttention`；只有没有记录的新玩家才走 `TiandaoAttention::default()`。
- [ ] 周期 autosave、断线 cleanup、关服 flush 都覆盖该 slice，避免只在正常登出时保存。
- [ ] load 时校验非法 response、NaN/Inf level、异常 tick；坏单条 warn 后降级到默认，不能阻塞玩家登录。
- [ ] 明确时钟口径：若 `last_eval_tick` / `last_response_tick` 基于运行 tick，重启后需要转换为可恢复的世界 tick 或相对冷却剩余量，避免重启后冷却被误判成极久以前或未来 tick。
- [ ] 加 server 单测：构造玩家 `TiandaoAttention { level: 72.0, response: Tribulation, last_response_tick: X, narration_count: N }`，保存后 load，断言字段恢复。
- [ ] 加关服 flush 回归：在线玩家处于 Pressure/Tribulation，触发 `AppExit` 后重启 hydrate，presence payload 继续显示旧 level/response。
- [ ] 加下游回归：恢复后的 `TiandaoAttention.level` 仍参与 territory 显眼度，恢复后的 `response` 仍参与 NPC fear scorer。

## 对抗结论

- 第一轮对抗提出另一个高置信候选：`WeaponForgeStation` 放置后未持久化，重启后炼器砧权威实体丢失。该候选成立但与 forge station / placeable / C2S 相邻主题更多，本轮不采用，避免 PR 主题贴近已有 forge 系列 plan。
- 第二轮对抗专审本候选后裁决 `CONFIRMED`：未发现保存、load、hydrate 或 flush `TiandaoAttention` 的路径；它也不应视作 transient，因为字段包含连续值、峰值、响应冷却和长期追猎语义，且已被 territory 与 NPC AI 读取。

## 风险

- 不能盲目保存 `TiandaoActivityRuntimeState` 的 `Entity` keyed 位置缓存；它是本次 session 的评估辅助，真正应持久化的是玩家身份维度的 `TiandaoAttention`。
- 如果未来设计认为“死亡/转世”应清空天道注意力，修复需把生命周期事件作为显式 reset 条件，而不是依赖断线或重启偶然清零。
