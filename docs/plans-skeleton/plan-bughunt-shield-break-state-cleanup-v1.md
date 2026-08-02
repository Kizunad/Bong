# plan-bughunt-shield-break-state-cleanup-v1（骨架）

> **骨架（草案）**。一句话主题：盾牌耐久归零或举盾所依赖的副手实例消失时，在 server resolver 内同步终止 `ShieldBlock` / `ShieldBlocking` / `ShieldDrainOverride` / `StaminaState::ShieldBlocking` 的同一份举盾生命周期，阻止空副手回退成木盾继续减伤、持续扣体力并错误触发 `ParryRecovery`。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 破盾同步终止状态集合、时序与状态矩阵合同 | ⬜ |
| P1 | resolver 内同步终止实现与同一 EventReader batch 回归 | ⬜ |
| P2 | 既有 `ShieldBroken` S2C / client 反馈链不回归 | ⬜ |
| P3 | server 全门禁与真实 shield-break bot e2e | ⬜ |

## 接入面

- **进料**：`server/src/combat/resolve.rs::resolve_attack_intents` 消费 `AttackIntent`，读取 `StatusEffectKind::ShieldBlocking`、副手 `PlayerInventory`、`ItemInstance.durability` 与 `ShieldBlock` 生命周期；耐久归零后现有路径调用 `consume_item_instance_once` 并写出 `ShieldBroken`。
- **出料**：破盾或副手盾实例已不存在后，后续同一 resolver batch 的攻击不再产生 `DefenseKind::ShieldBlock`、不再命中 `shield_block_profile("wooden_shield", 0.0)` fallback；既有 `ShieldBroken` 仍只在真实耐久归零且该实例已销毁时发出一次。
- **共享类型 / event**：复用 `ShieldBlock`、`ShieldDrainOverride`、`StatusEffects` / `StatusEffectKind::ShieldBlocking`、`Stamina` / `StaminaState::ShieldBlocking`、`LowerShieldIntent`、`ShieldBroken`、`InventoryDurabilityChangedEvent` 与既有 stop-animation emitter。不得新增第二个破盾 event、平行 ECS 状态、`DefenseKind`、receipt 或 cleanup sweep。
- **跨仓库契约**：本 plan 的 authority 修复限 server。既有链 `ShieldBroken` → `network::weapon_equipped_emit::emit_shield_broken_payloads` → `bong:server_data` → client `ProtoServerDataBridge` / `ServerDataRouter` → `ShieldBrokenHandler` / `EquippedShieldStore.clearIfInstance` 保持形状与一次性语义；不改 proto、TypeBox、agent、client wire 字段或音画资产。client 清本地 store 不能替代 server ECS 清理。
- **worldview 锚点**：`docs/worldview.md §五 L432-L446` 的防御边界：木/骨盾是既有凡人物理防御；盾实体已碎时防御必须立即终止，不能借空槽伪造持续防御或演变为真元护盾。
- **qi_physics 锚点**：不新增或改变真元、灵气、污染账本、`QiTransfer`、物理常数或 `qi_physics` 调用。此 finding 只修举盾 ECS / stamina 生命周期；既有盾格挡对伤害与污染的计算范围不扩张。

## Canonical Finding Mapping（本 plan 的全部 delivery scope）

| Canonical finding | 本 plan 覆盖 | 明确不覆盖 |
|---|---|---|
| `docs/finished_plans/plan-bughunt-r10-findings-v1.md` r10 #5 / Finding Mapping `#5 shield-break state leak` | 破盾、结算前副手实例缺失时的 server 权威终止；同 batch fallback / stamina / `ParryRecovery` 回归；既有 `ShieldBroken` feedback 保真；shield bot e2e | `plan-defense-hardening-v1` 的全局减伤 cap 或失败反馈；`plan-bughunt-shield-feedback-network-thread-ui-v1` 的网络线程/UI；普通 `shield_block` combat-event 飘字分类；新盾种、熟练度、耐久公式、C2S 形状、proto/schema、client 视觉资产、通用 ECS 生命周期框架 |

计数固定为 **1 条 canonical finding**。主动放盾、耐力耗尽、死亡、断线是同一举盾生命周期的既有相邻终态，只作为回归边界，不增加 implementation owner。

## 第一性验真（`origin/main @ 9e38cdd1654fd31f652a48ff3d98726fde68a822`，2026-08-02）

1. `server/src/combat/resolve.rs:1234-1264` 仅以 `ShieldBlocking` status 进入盾格分支；副手为空时无条件 fallback `("wooden_shield".to_string(), 0.0)`，而 `server/src/combat/shield_block.rs:101-116` 的未知/木盾 profile 仍提供正 `block_ratio`。
2. `server/src/combat/resolve.rs:1332-1358` 在 `next_ratio <= 0.0` 时更新耐久、调用 `consume_item_instance_once` 并 send `ShieldBroken`，未移除 `ShieldBlock`、`ShieldBlocking`、`ShieldDrainOverride` 或恢复 `StaminaState`。
3. `server/src/combat/shield_block.rs:357-390` 的 `lower_shield_handler` 已定义正确的手动放盾语义：去 status、将 `StaminaState::ShieldBlocking` 置回 `Idle`、移除 `ShieldBlock` / `ShieldDrainOverride`、发 stop animation；但 component 移除经 `Commands` 延迟提交，且该 handler 只能消费 `LowerShieldIntent`。
4. `server/src/combat/mod.rs:279-288,495-506` 把 `Intent → Physics → Resolve → Emit` 串行，`lower_shield_handler` 位于 `Intent`；`resolve_attack_intents` 位于 `Resolve`（`:325`）。因此 resolver 在本帧 send `LowerShieldIntent` 时，handler 已运行完，至少下一次 `Update` 才会消费，不能消除同一 `EventReader<AttackIntent>` batch 的第二次盾减伤。
5. `server/src/combat/lifecycle.rs:272-320` 对 `StaminaState::ShieldBlocking` 持续 drain，归零后转 `Exhausted`；`server/src/combat/shield_block.rs:455-495` 又在 exhausted 分支移除旧 state 并施加 `ParryRecovery`，所以破盾残留会造成额外的错误硬直。
6. `server/src/network/weapon_equipped_emit.rs:211-243` 的生产 `ShieldBroken` reader 只序列化并推送 payload；`server/src/network/mod.rs:1037,1169` 仅注册其 emitter/event。现有 S2C reader 不做 server ECS cleanup。client `ShieldBrokenHandler.java:81-115` 只清 `EquippedShieldStore` 并播放既有反馈，也不是 server authority。
7. `server/src/combat/resolve.rs:10388-10760` 已有真实 ItemRegistry / offhand / status fixture 与 `shield_broken_event_emitted_exactly_once_when_durability_reaches_zero`，但该测试只锁 event 和 inventory 移除，不断言举盾状态终止或同 batch 第二击。
8. 现有 bot 仅有通用近战路径 `scripts/bot/scenarios/combat_attack_hit.py:18-36`、`_combat_helpers.py:26-79` 与 `Bot.intent()`（`scripts/bot/bot.py:292-298`）；没有 shield 专用场景、raise/lower 串联或 `ShieldBroken` protobuf oneof field `133` 的专属 decode/assertion，故现有 bot 不能证明破盾后 server authority 已闭环。

## P0 — 破盾同步终止合同

- [ ] 在 `server/src/combat/shield_block.rs` 提取或扩展一个 focused shield terminal primitive（名称可等义，例如 `terminate_shield_blocking_state`），使手动 `LowerShieldIntent` 与破盾复用**同一份**状态终止定义：移除 `ShieldBlocking`、将 `StaminaState::ShieldBlocking` 恢复为 `Idle`（`Exhausted` 不得被降级）、移除 `ShieldBlock` 与 `ShieldDrainOverride`，并只为真实、此前在举盾的玩家发既有 stop animation。
- [ ] primitive 必须区分两层可观察时序：`StatusEffects` 与 `Stamina` 的逻辑门必须在 resolver 当前 intent 内完成可见 mutation；`ShieldBlock` / `ShieldDrainOverride` 如依赖 Bevy command buffer，必须在本次 `app.update()` 返回前已被移除。不得声称 deferred component removal 会在同一 system loop 内变成 query 可见，且不得让下一条 attack 依赖这种不成立的假设。
- [ ] 冻结唯一破盾调用边界：`resolve_attack_intents` 在确认本次命中使**当前副手实例**耐久归零、该实例已成功从 inventory 移除后，同步调用 P0 primitive，并只写出一条既有 `ShieldBroken`。不得在 resolver 中仅 send `LowerShieldIntent`，不得新增 `EventReader<ShieldBroken>` cleanup system，亦不得等待下一 tick / stamina exhaustion / disconnect / death cleanup。
- [ ] 冻结 stale-inventory 边界：若 resolver 开始处理时 `ShieldBlocking` 仍在、但副手已无对应可用盾实例，必须终止 stale 举盾 state，禁止 fallback 产生任何盾减伤；该路径不是耐久归零，**不得伪造 `ShieldBroken`**。耐久写入失败且对应盾仍在时保持既有物品与状态、不得发 false break；实现按可观察 postcondition 区分“仍有有效盾”与“实例已不存在”，不以吞错 / 无条件清理代替。
- [ ] 形成以下状态矩阵的 focused pin（名称可等义）：

| 起因 | inventory / durability 终态 | 举盾 state 终态 | `ShieldBroken` | `ParryRecovery` |
|---|---|---|---|---|
| 正面命中，`next_ratio > 0` | 原实例保留且耐久降低 | 保持举盾 | 0 | 0 |
| 正面命中，`next_ratio <= 0`，实例成功销毁 | offhand 无该实例 | 全部终止 | 恰 1 | 0 |
| 处理前副手实例已缺失 | offhand 不变（已空 / 不匹配） | stale state 全部终止 | 0 | 0 |
| 主动 `LowerShieldIntent` | 盾保留 | 全部终止 | 0 | 0 |
| `StaminaState::Exhausted` | 盾保留 | 既有强制终止 | 0 | 既有恰 1 |

- [ ] P0 contract 测试同时覆盖 exact break、未破盾、offhand missing、**副手仍占用但已替换为非盾 / 不匹配实例**、`StaminaState::{Idle, ShieldBlocking, Exhausted}`、重复 resolver intent 与 stop-animation 一次性语义；对两类 stale-inventory fixture 均断言外部可观察状态 / event / inventory，不锁 private helper 调用次数或具体重构形状。

## P1 — resolver 同步终止与同 batch 回归

- [ ] 在 `server/src/combat/resolve.rs` 的耐久归零分支接入 P0 terminal primitive。实现必须让当前 resolver 能取得并同步修改 target 的 `StatusEffects` / `Stamina`，同时遵守 Bevy borrow / system-param 上限；若需改造 query 或 existing bucket，只采用最小局部重组，不引入通用 transaction、第二个 combat loop 或平行 status store。
- [ ] 在读取盾格状态时，不能把首个 intent 取得的不可变 `ShieldBlocking` 快照缓存到整个 batch；第二条 intent 必须重新观察破盾后的权威 status。唯一可接受的安全门是第二条攻击在 resolver 内看见 `ShieldBlocking` 已不存在，从而不能走 `shield_block_profile("wooden_shield", 0.0)` fallback。
- [ ] 扩展 `server/src/combat/resolve.rs` 现有 `make_shield_durability_app` / `equip_shield_off_hand` / `insert_shield_blocking` fixture（或等价的真实 App fixture），新增同一 `app.update()` 的 two-hit regression：
  1. 在 update 前写入针对同一防御者的两条有效 `AttackIntent`，使第一条恰好耗尽最后耐久、第二条仍由同一 `EventReader` batch 消费；测试应使用不被 cooldown / qi gate 干扰的有效输入。
  2. 断言第一条只新增一条 `ShieldBroken`，并移除目标 offhand instance。
  3. 从两条对应 `CombatEvent` 断言第一条可为 `DefenseKind::ShieldBlock`，第二条**不是** `DefenseKind::ShieldBlock`，且第二条伤害 / 污染不获得木盾 fallback 削减。
  4. 本次 `app.update()` 完成后断言 `ShieldBlock`、`ShieldBlocking`、`ShieldDrainOverride` 均不存在，`StaminaState != ShieldBlocking`；后续 update / 再攻击仍无盾减伤、stamina 不再因盾 drain、不会由旧 state 进入 `Exhausted` 或 `ParryRecovery`。
- [ ] 明确锁住 event 精度：同一已碎 instance 的后续 intent 不可再 emit `ShieldBroken`；外部测试直接注入重复 event 不属于本 finding 的全局去重协议。无需新增 dedupe resource、persistent receipt 或 network acknowledgement。
- [ ] 覆盖边界：正面 FOV 未破盾保持现有减伤和持续 drain；背面 / 未格挡攻击不制造 shield terminal；主动放盾与 stamina exhaustion 保留现有各自语义；结算前移走副手盾**或用非盾物品替换副手实例**时均只清 stale server state、无 `ShieldBroken`；后者必须断言攻击无 `DefenseKind::ShieldBlock`，伤害 / 污染不获得木盾 fallback 削减；durability update / inventory consume 失败不制造 false `ShieldBroken`、不吞 inventory、不得留下可触发 fallback 的 stale state。
- [ ] 可核验 symbol：`terminate_shield_blocking_state`（名称可等义）、`shield_break_clears_authoritative_blocking_state`、`shield_break_second_intent_in_same_resolver_batch_is_not_blocked`、`missing_offhand_shield_clears_stale_blocking_state`、`shield_break_does_not_apply_exhaustion_parry_recovery`、`shield_break_emits_once_per_destroyed_instance`（名称可等义）。

## P2 — 既有 `ShieldBroken` feedback 不回归

- [ ] 保持 `server/src/combat/weapon.rs::ShieldBroken { entity, instance_id, template_id }` 及现有 `server/src/network/weapon_equipped_emit.rs::emit_shield_broken_payloads` 的唯一 S2C 形状；P1 只改变其 server producer 之前的 authority cleanup，不改 `bong:server_data`、protobuf payload、TypeBox、schema sample、router key、音效、粒子、toast 或 HUD 结构。
- [ ] server pin 从真实破盾 resolver 路径断言 event 的 `entity`、`instance_id`、`template_id` 正确，恰好一条 event 仍被 `emit_shield_broken_payloads` 发送；不得因为同步 cleanup 而丢失既有 client clear / toast / flash / material audio-VFX。
- [ ] 保持 client bridge / router / handler 现有验收：`ProtoServerDataBridge` 的 `SHIELD_BROKEN` case、`ServerDataRouter` 的 `"shield_broken"` route、`ShieldBrokenHandler` 的 `EquippedShieldStore.clearIfInstance` 与 wood/bone feedback 都仍能接收原 payload。server state 测试不以 client local store 作为 authority 证据，client test 不以 payload receipt 掩盖 resolver state。
- [ ] 不重开 `plan-bughunt-shield-feedback-network-thread-ui-v1` 的 network-thread/UI 修复，不改 `combat_event` 的 `shield_block` 分类，也不引入第二套 ShieldBroken / WeaponBroken 复用路径。

## P3 — 饱和验收与 bot e2e

- [ ] 新增 focused shield-break bot scenario（文件名可等义，例如 `scripts/bot/scenarios/combat_shield_break_cleanup.py`），复用 `Bot.intent()`、`_combat_helpers.py`、`/give`、inventory snapshot / equip move 与 `/npc_scenario fight` 的既有真实链路，走真实 C2S `RaiseShield` / `LowerShield` 请求和真实 server combat resolver；不得用直接写 ECS resource、直接 send `ShieldBroken`、client store mock 或假设不存在的 durability setter 代替生产路径。
- [ ] bot scenario 至少覆盖：通过既有真实路径装备木盾 → raise → 用真实正面 NPC 攻击将现有耐久逐次消耗至 break → 解码 / 断言恰一次 `ShieldBroken` oneof field `133` payload（内部 `instance_id` / `template_id` 均匹配刚装备实例）→ 随后真实攻击不再得到盾格 combat feedback；不得假设 `/give` 能直接给低耐久实例，若单次 bot run 的可接受攻击预算不足以打破满耐久盾，则先补可由真实 gameplay 到达的低耐久铺垫，而非增加耐久度开发后门。并运行“raise 后主动 lower 不发 break”的对照。
- [ ] bot 必须验证 server 对坏 / 重复 C2S 宽容：无盾 raise 或破盾后重复 lower 不踢人、不 panic、不产生 false `ShieldBroken`；这只验证既有 C2S 边界，不扩大为 client anti-cheat 重构。
- [ ] focused server test 先跑实际新增符号对应的非零测试过滤器（禁止零测试假绿）；最终执行 `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`，以及受脚本改动影响的 bot scenario gate。headless server 启动时设 `BONG_SKIP_SKIN_PREFETCH=1`；本地不运行 `scripts/test-tmux-shutdown-order.sh` 或任何调用它的 suite，关停覆盖留给 GitHub e2e。

## 范围边界与相邻 owner

- `docs/plans-skeleton/plan-defense-hardening-v1.md` 的全局减伤 cap、拒绝反馈、通用 defense hardening 不在本 plan；r10 #5 的唯一修复点是破盾 / stale offhand 后的举盾 state leakage。
- `docs/finished_plans/plan-bughunt-shield-feedback-network-thread-ui-v1.md` 只修 client network thread / UI thread 落地；本 plan 不改 `BongNetworkHandler`、`ServerDataRouter`、`ShieldBrokenHandler`、audio / particle / HUD 线程模型。
- `docs/finished_plans/plan-shield-block-combat-event-feedback-v1.md` 已明确区分正常盾格 `combat_event.kind="shield_block"` 的 client 飘字分类；本 plan 不改该 event、`CombatEventHandler` 或 `DamageFloaterStore`。
- 不改 `ShieldSpec`、木/骨盾配方、耐久 cost 公式、FOV、熟练度、经脉、CombatJuice、stop-animation 资产或 `RaiseShield` / `LowerShield` wire contract；只复用这些既有面以闭合 state 生命周期。
- 不新增 qi / 灵气 / 污染守恒逻辑，不修改 agent Redis、schema/proto、持久化、断线 / 死亡 cleanup 的 owner，也不把此局部 finding 扩张为通用 combat state-machine 框架。

## §8.1 决议（骨架实施合同，`origin/main @ 9e38cdd1654fd31f652a48ff3d98726fde68a822`）

1. **resolver 内同步终止，拒绝 EventReader 中转**：`CombatSystemSet::Intent` 已先于 `Resolve` 执行，故 P1 不得通过 send `LowerShieldIntent` 等待下一帧；破盾 path 必须直接完成下一条 attack 所需的 status/stamina 可见 mutation。`LowerShieldIntent` 仍是手动输入入口，二者复用 terminal semantics 而非互相代替。
2. **复用既有 `ShieldBroken`，不加 cleanup reader**：`ShieldBroken` 的生产 reader 是 S2C emitter；另挂 server cleanup reader仍不能满足同 batch，且会把 authority 从 producer 拆成延迟的第二条链。P1 在 producer 分支收口，P2 只回归原 event payload。
3. **分离逻辑即时性与 Bevy command 时序**：同 batch 门禁只依赖同步移除 `ShieldBlocking` / 改写 `StaminaState`；component / override 的 deferred removal 必须在 update 返回前兑现，并以 update 后 ECS 断言锁住。不得为追求“同一 system 内 component 不存在”而引入 `World` 独占访问或新的 loop。
4. **副手已缺失不是破盾**：无实际 zero-durability 销毁不得发 `ShieldBroken`；但 stale `ShieldBlocking` 也绝不可继续给 fallback 减伤。该分支只终止 server state，不伪造客户端破盾视听。
5. **破盾不施加 exhausted 硬直**：`ParryRecovery` 是既有 stamina 耗尽终态的惩罚，不是盾实体碎裂的副作用。P1 必须保证破盾清理后 `force_lower_shield_on_stamina_exhausted` 没有旧 `ShieldBlock` / `ShieldBlocking` 可消费。

以上五项是本 skeleton 的确定实施合同；active promotion 时仅允许更新 current `file:line`，不得重新扩大 delivery scope、复活第二 event / 读者 cleanup 路径，或将 client feedback / defense-hardening 并入本 plan。
