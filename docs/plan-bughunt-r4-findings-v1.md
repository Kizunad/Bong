# plan-bughunt-r4-findings-v1（active）

> **Active（已从 skeleton 升级，待逐项消费）**。一句话主题：代码库自检 bug-hunt **round4**（fresh origin/main worktree ROOT，换角度：time/cooldown 边界 · agent↔server 命令回路 · 货币守恒(骨币/灵石) · buff 生命周期 · zone 边界守恒）确认的 **8 个新真 bug**——含 **1 critical（qi_zero_decay 跨重启 u64 下溢 → 重连瞬时不可逆降境）**。已对 r1+r2+r3 去重（含已剔除项），全部 real-on-main。

> 立项动机：round4 用 fresh origin/main worktree 为 ROOT，5 全新角度 finder → 怀疑者对抗 → opus 逐条 Read/Grep 复核，8 候选 **全部 REAL**（本轮无误报，去重清单覆盖 r1+r2+r3 已确认 25 项 + 已剔除 4 项）。本轮主线：**alchemy 副作用/buff 整簇接线断裂**（QiCapPermMinus/QiRegenBoost emit 无 consumer + DuJieDan 减伤永久泄漏）与**交易/货币双目录不一致**。

## 阶段总览（按主题分组，逐项独立可修）

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 🔴 qi_zero_decay 跨重启 u64 下溢 → 瞬时降境 | fix_pr | ⬜ |
| P1 | 货币/交易守恒（双目录不一致 + 稀有度门缺失） | fix_pr + plan_skeleton | ⬜ |
| P2 | alchemy buff/status 接线断裂簇（泄漏 + 孤岛） | fix_pr + plan_skeleton | ⬜ |
| P3 | 跨端契约 + zone 边界守恒 | plan_skeleton | ⬜ |

## P0 — 🔴 qi_zero_decay 跨重启下溢（critical）

- **#1 critical（fix_pr）**：`server/src/cultivation/qi_zero_decay.rs:86` `now - cultivation.last_qi_zero_at.unwrap()` **普通减法无 saturating**。链：
  - `Cultivation.last_qi_zero_at: Option<u64>`（`components.rs:380`）被 serde 原样持久化，hydration 时 `cultivation = decoded`（`mod.rs:570`）无清理/钳制；
  - `CultivationClock` 仅在 `mod.rs:225` 以 `default()`（tick=0）插入，persistence 全无加载 → 重启后从 0 起 `wrapping_add(1)`（`tick.rs:113`）；
  - 场景：玩家在 qi≤1% 时离线（`last_qi_zero_at` 设为大 tick 如 50000），服务器重启后 clock≈0 重连 → `since = 小值 - 50000` 下溢，release 模式 wrap 成 ≈u64::MAX，`since < DECAY_TRIGGER_TICKS` 立即为 false → **第一帧即触发降境一阶 + LIFO 关经脉 + qi_max 重算**（debug 模式直接 panic）。
  - 唯一单测（192-232）只覆盖 tick 高于时间戳的正常场景，缺跨重启反向用例。修：`saturating_sub` 返回 0（无害不触发降境）或 hydration 时清掉超当前 clock 的旧值；补跨重启回归测试。**critical 因后果瞬时不可逆降境。**

## P1 — 货币/交易守恒

- **#3 major（fix_pr）**：NPC 交易**双目录**——展示侧 `assign_npc_trade_inventory`（`combat/.../trade.rs:593+`）从 `TRADE_CATALOGUE` 取 `price_bone_coins` 经 `npc_metadata.rs:308-313` 送 client；执行侧 `client_request_handler.rs:1280` 用**独立** `npc_trade_catalog_entry` 解析价格/模板。两者从不对账：① `lingcao` 展示 12（trade.rs:510）执行 10（handler:7674）且模板换 `spirit_grass`（显示 lingcao 实得 spirit_grass）；② `fragment_scroll` 展示 45 执行 40 换模板；③ `bone_meal`/`rough_bandage`/`qi_condensing_powder`/`meridian_salve`/`spirit_stone_shard`/`bone_reinforcing_pill`/`spirit_jade` 七项在 TRADE_CATALOGUE 但 `npc_trade_catalog_entry` 无臂 → `_ => None`（1280-1293）"没有这件货"，**HUD 可见永远买不到**。违所见即所得 + 货币守恒。修：统一单一目录。**局部明确。**
- **#4 minor（plan_skeleton）**：`TradeEligibility::RefuseRare`（`trade.rs:219` 文档"低信誉：拒绝稀有品"，`check_trade_eligibility` 226-237 把 `RepTier::Low` 映射为之）在 `client_request_handler.rs:1354-1359` 的臂**仅算 1.3x 加价**，随后直落 `add_item_to_player_inventory`（1394），**全程无 `rarity` 检查** → 稀有度门从未实装。当前无 rare 物经此路径（候选库只有低品物 + 受 #3 双目录限制）故无法演示越权 → minor，但潜伏：一旦往 `npc_trade_catalog_entry` 加稀有物即越权。修：定义稀有度门设计后实装。**需设计决策。**

## P2 — alchemy buff/status 接线断裂簇

- **#7 major（fix_pr）**：`DuJieDan` 渡劫减伤**永久泄漏**。`cultivation_pill_effects(DuJieDan)`（`alchemy/pill.rs:908-921`）返回 `BreakthroughBoost(u64::MAX)` + `DamageReduction(0.30, u64::MAX)`，注释（912）称"由 tribulation_system 清理"。consume 时两条都 `push_status_effect` 进 `StatusEffects.active`（961-983）。但 `breakthrough.rs:758-761` 不论成败只调 `clear_breakthrough_boost`，该函数（`status.rs:102-106`）retain **仅移除 BreakthroughBoost**，DamageReduction 原封不动；`tribulation.rs` 全无清 DamageReduction 代码；`status_effect_tick` 用 `saturating_sub`，u64::MAX 永不归零；`attribute_aggregate_tick` 的 DamageReduction filter（`status.rs:187-193`）**无 `remaining_ticks>0` 检查**（对比 DamageVulnerability:227 有）→ 用过 DuJieDan 的玩家**永久保留 30% 减伤**直至濒死全清或重置。修：tribulation 结算时清 DamageReduction + filter 补 remaining_ticks 检查。**机械数值泄漏，局部明确。**
- **#5 major（plan_skeleton）**：`QiCapPermMinus` 永久 debuff **emit 无 consumer**。全仓引用仅：`events.rs:111`(定义)、`side_effect_apply.rs:24/118`(标签映射无消费)、`status_snapshot_emit.rs:92/153/196`(HUD 显示"真元上限折损"+优先级5)；perm=true 时 duration 返回 u64::MAX 永久挂载。但 `combat/status.rs:147-252` `attribute_aggregate_tick` 处理 Slowed/DamageAmp/.../DamageVulnerability **独缺 QiCapPermMinus**；`cultivation/tick.rs` 只用独立的 `qi_max_frozen`（172），全仓无任何系统因此 buff 改 `cultivation.qi_max`。**玩家承受 HUD 告知的永久惩罚但数据零变化**。永久挂载意味即便接 consumer 也无自然修复路径，需一并设计。**孤岛，需设计。**
- **#6 minor（plan_skeleton）**：`QiRegenBoost` 状态效果**无 consumer**。引用：`events.rs:107`、`side_effect_apply.rs:22/106/110`(映射 magnitude 0.10/0.25)、`pill.rs:603`(JinZhongDan 负面 0.001)、`status_snapshot_emit.rs:90/135`(HUD"回气变化")。但回气主路径 `cultivation/tick.rs` `qi_regen_and_zone_drain_tick` 只读 `CultivationAcceleration`(184) 和 `QiRegenSlowed`(187)，**全仓无 regen multiplier 读 QiRegenBoost**；`pill.rs:1041-1043` 注释称"仍用于短时战斗回气"属误导。recipe fallback 的 minor_qi_regen_boost + JinZhongDan 负面均静默无效。**孤岛，需定义回气 buff 接入点。**

## P3 — 跨端契约 + zone 边界守恒

- **#2 major（plan_skeleton）**：`spawn_npc` 的 `daoxiang`/`zhinian`/`fuya`/`skull_fiend` 四 TSY 原型**契约漂移**——`agent/packages/schema/src/agent-command.ts:6-17` `ALLOWED_NPC_ARCHETYPES` 含这 4 值，`validateAgentCommandV1Contract`(134) 放行；但 server `command_executor.rs:548-563` `execute_spawn_npc` 的 match 只有 zombie/commoner/rogue/beast/disciple/guardian_relic 六臂，4 个 TSY 原型落 `_ => rejected_unsupported_archetype`（server 测试 2024 把 daoxiang 当 bad_archetype 证实）。这 4 个本应只由 server 内部 `spawn_tsy_*_at`（tsy_hostile/hydrate/daozhan）生成，属 server→agent 遥测语义不应进 agent→server 白名单 → LLM 产出含这 4 值的 spawn_npc 过校验、过 Redis、到 server 静默丢弃无玩家可见效果。修：从白名单删 4 值 vs 给 executor 补臂（设计抉择）。**需人工定夺。**
- **#8 major（plan_skeleton）**：`realm_collapse`/`heavenly_fire` 坍塌**违 zone 灵气零和守恒**。`collapse_zone`（`world/events.rs:2731`）调 `redistribute_zone_qi_before_collapse` 但**返回值不接收**，紧接 2736 无条件 `active_zone.spirit_qi = 0.0`；`heavenly_fire`（1287-1294）同模式（redistributed 仅供日志不阻 1294 归零）。`redistribute_zone_qi_before_collapse`（1990-1993）对每邻居 `(before+amount).clamp(-1,1)`；`collapse_redistribute_qi`（`tiandao.rs:28-48`）给满容量 zone 权重 0.01 算正 amount，但邻居已在 1.0 时 clamp 使实增量=0 → `total_redistributed≈0`，源 zone 仍全归零 → **满容量相邻 zone 场景下坍塌 zone 的 spirit_qi 被静默销毁**（`summarize_world_qi` `ledger.rs:449-452` 同 tick 可测守恒破缺）。无 overflow/budget 兜底。现有测试（events_tests:3966）只覆盖邻居有余量。修：引入 overflow/budget 回流账户的守恒设计。**需守恒设计。**

## §N 开放问题

1. #1 修法：`saturating_sub`(快) vs hydration 钳制旧时间戳(根治)——是否同时持久化 CultivationClock tick（当前 default 0 是根因之一）。
2. #5/#6 alchemy buff 接入点：QiCapPermMinus 改 qi_max 走 `attribute_aggregate_tick` 还是 `cultivation/tick.rs`；QiRegenBoost 回气 multiplier 接 `qi_regen_and_zone_drain_tick`——是否一并把 alchemy 副作用整簇接线（含已确认的 r-side）。
3. #2 spawn_npc 收口方向：删白名单 4 值（纯遥测语义）vs executor 补臂（允许 agent spawn TSY 原型）。
4. #8 zone 守恒：overflow 账户回流 vs 坍塌灵气并入坍缩渊中转（worldview §十 坍缩渊吸入是中转站）——与 r1 `plan-qi-conservation-leaks-v1` 同守恒域，是否并入。
5. #3/#7/#1 三条 fix_pr 是否合一个机械 fix PR（与 r1/r3 的机械 fix 同性质）还是各自独立。

## 审计来源

bug-hunt round4（workflow，5 全新角度 finder + 怀疑者对抗 + opus 裁决，8 候选全 REAL）。**ROOT = fresh origin/main worktree**（方法论修正后第二轮）。已对 r1+r2+r3 去重（25 已确认 + 4 已剔除）。**report-only**：critical qi_zero_decay 优先；#1/#3/#7 局部明确可直接 fix_pr，#2/#4/#5/#6/#8 需契约/buff 接线/zone 守恒/稀有度门设计决议。**本轮主线发现**：alchemy 副作用/buff 整簇接线断裂（多个 status effect emit 无 consumer），与 r1 forge 整簇未接 qi_physics 同类系统性缺口。
