# plan-trap-runtime-v1 — 陷阱 runtime：困兽夹/绊线/诱饵桩

> 一句话：放置类僵尸物品「陷阱」消杀——`beast_trap`（困兽圈）、`trip_wire`（预警绊线）、新增 `bait_stake`（诱饵桩）实装真实触发功能。三者均为**凡物机械陷阱**，无真元参与，不碰 ledger。
>
> 来源：放置类 17 调查 workflow（opus 抽查 7/7 属实）；用户拍板：陷阱需具备对应功能。承接 finished `plan-zhenfa-content-v1`（凡阵触发底盘）+ `plan-npc-ai-v1`（NpcBlackboard 仇恨）。

**依赖**：[[plan-zhenfa-content-v2]] P0——**排期依赖**（zhenfa-content-v2 先扩 `ZhenfaKind` 枚举建立先例 + ID 裁决），非基础设施依赖：`ZhenfaPlace` 协议本就存在（`client_request.rs:257`），不必等其全部阶段落地。**排期硬约束见 §8.1 #5**：本 plan PR-1 不与 zhenfa-content-v2 PR 并行改 `ZhenfaKind` 枚举，必须串行（避免同一枚举 merge 冲突）。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 陷阱放置 + 触发底盘（扩 ZhenfaKind 三变体 + 目标过滤修正） | ⬜ |
| P1 | beast_trap 困兽（新 `Immobilized` 状态）+ trip_wire 报警 | ⬜ |
| P2 | bait_stake 诱饵（新 `DecoyTarget` component + NPC 仇恨改造） | ⬜ |

---

## 接入面（防孤岛 checklist）

- **进料**：
  - 触发底盘：`server/src/zhenfa/mod.rs:1531` `tick_zhenfa_registry`（距离扫描，WarningTrap/BlastTrap/SlowTrap 已实装）/ `:1478` `type ZhenfaDamageTarget`（**红旗 #13 坐实**：无 `With/Without` 过滤会误伤友方）/ `:1599` `ZhenfaKind::WarningTrap` 触发分支（`vertical_column_contains` + 触发检测节流 `WARNING_TRIGGER_THROTTLE_TICKS`=5s（`trap_content.rs:18`，命中点 :1614），trip_wire 直接复用；**注意**：报警出料节流是另一常数 `WARD_ALERT_THROTTLE_TICKS`=60s（`mod.rs:52`，命中点 :1688），二者不同语义不同数值，见报警出料项）
  - trap_content 规格：`server/src/zhenfa/trap_content.rs:24` `enum OrdinaryTrapKind`（`from_zhenfa_kind:31` / `detection_radius:48` / `vertical_height:55` / `survival_ticks:148` / `half_life_ticks:140`，新陷阱按此模式加规格函数）/ `:190` `vertical_column_contains` / `:18` `WARNING_TRIGGER_THROTTLE_TICKS = 5*20`
  - C2S 放置：`server/src/schema/client_request.rs:257` `ZhenfaPlace`/`ZhenfaTrigger`/`ZhenfaDisarm`（陷阱统一走 `ZhenfaPlace` + `ZhenfaKind` 扩枚举，**不开第二套放置路径**）
  - 报警出料：`server/src/zhenfa/mod.rs:1762` warning_alerts 处理（`pending_narrations.push_player` + `ZhenfaSensePulse{ kind: SenseKindV1::ZhenfaWardAlert }` + `emit_zhenfa_vfx(ZHENFA_WARD)`，整套 trip_wire 复用 99%）；此路径报警节流走 `WARD_ALERT_THROTTLE_TICKS`=60s（`mod.rs:52`，命中点 :1688）——**trip_wire 复用 warning_alerts 即继承此 60s 节流，不是 5s 的触发检测节流**（与同族 [[plan-zhenfa-content-v2]] line 10 契约一致：两 plan 共享 `WARD_ALERT_THROTTLE_TICKS`，命名必须一致）
  - NPC 仇恨：`server/src/npc/brain/mod.rs:289` `update_npc_blackboard`（**真实控制流**：`DuelTarget` continue 早退:305 → nearest player 扫描循环:318（先内联跑完暂存）→ retaliation continue 早退:343 → 末尾回填 nearest:358；**签名无 DecoyTarget query，无非玩家 decoy 入口**，P2 需新增 decoy_query 参数，见 §8.1 #2）/ `server/src/npc/spawn/common.rs:166` `struct DuelTarget(Entity)`（DecoyTarget 实现先例）/ `:170` `struct NpcBlackboard`
  - 状态效果：`server/src/combat/events.rs:81` `enum StatusEffectKind`（现无定身类，最近的 Slowed/Stunned/Disoriented 均无"不可移动"语义）/ `:159` `struct ApplyStatusEffectIntent`；注入先例 `server/src/npc/npc_skill.rs:219` `ApplyStatusEffectIntent` 发送路径
  - 物品资产：`server/assets/items/workbench_materials.toml:557` `trip_wire`（category=misc，spirit_quality=0.0）/ `:568` `beast_trap`（category=misc，spirit_quality=0.0）；`bait_stake` P2 新建
  - 配方：`server/src/craft/workbench_recipes.rs` `// #78 预警绊线`(CraftCategory::Misc) / `// #82 困兽圈`(CraftCategory::Misc)；P0 改 category=ZhenfaTrap（见 §8.1 #6）；`bait_stake` 配方 P2 新建
- **出料**：
  - beast_trap：proximity 命中野兽 → `ApplyStatusEffectIntent{ kind: Immobilized, target: beast_entity }` + 小伤害（`Wounds`），触发后转"已咬合"态可回收重置
  - trip_wire：命中任意活动实体 → `pending_narrations.push_player`（owner 报警）+ `ZhenfaSensePulse(ZhenfaWardAlert)` → HUD 事件流；无伤害
  - bait_stake：范围内野兽/敌对 NPC 仇恨优先指向桩 → `NpcBlackboard.decoy_target: Option<(Entity, u64)>` 驱动 `update_npc_blackboard`；桩有耐久，被攻击 N 次碎裂
- **共享类型 / event**（**复用优先**）：
  - `ZhenfaKind` 扩三变体 `BeastTrap`/`TripWire`/`DecoyStake`（`zhenfa/mod.rs:64`）——**不新建独立 enum**
  - `StatusEffectKind::Immobilized` 新变体（`combat/events.rs:81`）——现无定身语义，新增有据
  - `DecoyTarget(Entity)` 新 component（`npc/spawn/common.rs:166` `DuelTarget` 同款单字段包装）+ `NpcBlackboard.decoy_target` 字段
  - trip_wire 报警复用 warning_alerts 路径，继承其 `WARD_ALERT_THROTTLE_TICKS`=60s（`mod.rs:52`）报警节流——**不自写节流常数，与 [[plan-zhenfa-content-v2]] 命名一致**（触发检测层另有 `WARNING_TRIGGER_THROTTLE_TICKS`=5s，二者不可混用）
  - 复用 `ZhenfaSensePulse` + `SenseKindV1::ZhenfaWardAlert`（trip_wire 报警，不新建感知变体）
- **跨仓库契约**（缺一面即红旗）：
  - server `ZhenfaKind`（`zhenfa/mod.rs:64`）↔ proto `enum ZhenfaKind`（`proto/bong/envelope.proto:3455`，现到 `ZHENFA_KIND_ILLUSION=9`，新增 10/11）↔ `proto_convert.rs:2956` `zhenfa_kind_to_proto`（加三 arm）↔ client `ClientRequestProtocol.ZhenfaKind`（`ClientRequestProtocol.java:87`，加三常量+wireName）↔ client `bong$zhenfaKindForItem`（`MixinClientPlayerInteractionManagerAlchemy.java:180`，加三 case）
  - trip_wire 报警 → HUD 事件流 S2C（复用现有 `ZhenfaSensePulse` → 事件流 emit 链，非新通路）
  - schema sample 对拍：`agent/packages/schema/samples/*.json`（ZhenfaKind 三变体正反 sample）
- **worldview 锚点**：
  - **§五.3（worldview.md:417-422）** 地师/阵法流：「真元不附着在武器上，而是封入环境方块……在狭窄通道、灵草旁、必经之路埋设陷阱」——beast_trap/trip_wire 作凡物陷阱契合（虽不封真元，属同流派的"凡物机关"分支）
  - **§七（worldview.md:715-752）** 动态生物生态：噬元鼠群/拟态灰烬蛛等低阶妖兽=「野兽」目标，判定走 `FaunaTag.beast_kind`（`fauna/components.rs:365`），野兽集合见 §8.1 #4
  - **§八.3（worldview.md:812-816）** 欺天阵替身木桩：**canon `decoy_stake` 是"向天道广播假劫气权重"的欺天阵法器，非引 NPC 仇恨**——本 plan 不动 decoy_stake 语义，新建 `bait_stake` 做凡物引仇恨（见 §8.1 #1）
- **qi_physics 锚点**：beast_trap/trip_wire/bait_stake 均为**凡物机械陷阱，无真元参与，不碰 ledger，不写任何衰减/逸散常数**。陷阱"存活时长"复用 `trap_content::survival_ticks`（已有凡阵口径），**不引入新 `*_DECAY*` 常数**。`bait_stake` 经 §8.1 #3 决议为纯凡物（不带真元吸引），不接 `qi_release_to_zone`。

---

## P0 — 陷阱放置 + 触发底盘

**模块**：`server/src/zhenfa/mod.rs`、`server/src/zhenfa/trap_content.rs`、`server/src/schema/proto_convert.rs`、`proto/bong/envelope.proto`、client `ClientRequestProtocol.java` + `MixinClientPlayerInteractionManagerAlchemy.java`

**交付物**：
- `ZhenfaKind` 扩 `BeastTrap`/`TripWire`/`DecoyStake` 三变体（`zhenfa/mod.rs:64`）。注：`DecoyStake` 变体此阶段仅占位放置走通（语义=凡物诱饵桩，对应新物品 `bait_stake`，见 §8.1 #1），实际仇恨逻辑 P2 落地
- proto `enum ZhenfaKind` 加 `ZHENFA_KIND_BEAST_TRAP=10` / `ZHENFA_KIND_TRIP_WIRE=11` / `ZHENFA_KIND_DECOY_STAKE=12`（`envelope.proto:3465` 后追加，**保留现有编号不重排**）；`proto_convert.rs:2956` `zhenfa_kind_to_proto` 加三 arm
- client：`ClientRequestProtocol.ZhenfaKind` 加 `BEAST_TRAP("beast_trap")`/`TRIP_WIRE("trip_wire")`/`DECOY_STAKE("decoy_stake")`；`bong$zhenfaKindForItem` 加三 case（`beast_trap`→BEAST_TRAP / `trip_wire`→TRIP_WIRE / `bait_stake`→DECOY_STAKE）。**注意**：物品 id `bait_stake` 映射到 wireName `decoy_stake`——因 `decoy_stake` 物品已被 canon 欺天阵占用，凡物引仇恨用新物品 id `bait_stake`，但复用 `DecoyStake` ZhenfaKind 变体（见 §8.1 #1）
- `trap_content::OrdinaryTrapKind` 扩三变体（Beast/TripWire/Decoy）+ `from_zhenfa_kind`(`:31`)/`expected_item_id`(`:40`)/`detection_radius`(`:48`)/`vertical_height`(`:55`)/`survival_ticks`(`:148`) 各加 arm
- **触发目标过滤修正（红旗 #13）**：`ZhenfaDamageTarget` query（`zhenfa/mod.rs:1478`）补 owner 排除 + 目标类型判别。新增 `fn is_beast_target(tag: &FaunaTag) -> bool`（查 `FaunaTag.beast_kind`（`fauna/components.rs:365`），判定 `beast_kind ∈ §8.1 #4 野兽集合`）；触发扫描需新增 `Query<&FaunaTag>`（或在 `ZhenfaDamageTarget` query 上 `Option<&FaunaTag>`）才能取到该 component——**实施时必须声明这一 query 改动**；陷阱触发分支统一 `if target == instance.owner { continue; }`（owner 不触发自家陷阱）

**视听**（放置/拆除复用现有 ZhenfaPlace VFX，触发视听在 P1/P2）：放置成功复用 `gameplay_vfx::ZHENFA_TRAP`（`gameplay_vfx.rs:22`）灰蓝落地脉冲，无新增。

**测试（饱和化）**：
- 三变体 round-trip pin 测试：Rust `ZhenfaKind` → proto i32 → 还原（happy）；新三变体各一条专属 case；UNSPECIFIED=0 不被误映射（错误分支）
- 跨仓库 wireName 一致性：`zhenfa_kind_to_proto` 全变体覆盖断言（含三新变体，漏一个撞红）
- 放置/拆除：三变体 `ZhenfaPlace` 走通建 anchor / `ZhenfaDisarm` 移除（状态转换 place→placed→disarmed）
- owner 排除：owner 站自家陷阱不触发（错误分支）；非 owner 触发（happy）
- `is_beast_target`：野兽集合内 true（`FaunaTag{beast_kind: Spider}` 灰烬蛛）/ 集合外 false（无 FaunaTag 的玩家/Zombie/Skeleton，及不在野兽集合的 BeastKind 变体）/ 边界（§8.1 #4 野兽集合边缘成员）
- `OrdinaryTrapKind::from_zhenfa_kind` 三新变体返回 Some，非陷阱变体（Lingju 等）返回 None

## P1 — beast_trap 困兽 + trip_wire 报警

**模块**：`server/src/zhenfa/mod.rs`、`server/src/combat/events.rs`、`server/src/combat/status.rs`（新 `Immobilized` 变体 + `has_active_status` 复用，**注意**：`status.rs:251` 是 `BodyPartResist/Weaken` 部位倍率 fold，非 Immobilized 落点——见 §8.1 #7）、`server/src/npc/navigator.rs`（`navigator_tick_system` 加 Immobilized 守卫，真正的"困不住"消费方）、`server/assets/audio/recipes/`、client `ZhenfaActionVfxPlayer.java`

**交付物**：
- `StatusEffectKind::Immobilized` 新变体（`combat/events.rs:81`）：定身，`magnitude`=挣扎抗性占位，`duration_ticks` 后释放或被击杀提前解除。**生效路径见 §8.1 #7**：给 `navigator_tick_system`（`npc/navigator.rs:276`，NPC 唯一写 Position 的 system）query 补 `Option<&StatusEffects>`，在 `:320` `navigator_should_yield()` 早退判定旁加 `if has_active_status(status, Immobilized) { continue; }`（与 Override yield 同位早退，不写 Position）。**不靠 move_speed_multiplier 聚合**（navigator 不读该乘子，会成死状态），**不改 `movement/mod.rs:117` MovementState 状态机**
- beast_trap 触发分支（`zhenfa/mod.rs` ZhenfaKind 匹配处新增 `ZhenfaKind::BeastTrap =>`）：`vertical_column_contains` proximity 扫描 → 命中 `is_beast_target` 实体 → emit `ApplyStatusEffectIntent{ kind: Immobilized, magnitude: 1.0, duration_ticks: 8*20 }` + 小伤害（`Wounds` +2.0）→ 陷阱转"已咬合"态（registry 标记，**单次性**，不重复触发）；回收重置走 `ZhenfaDisarm` 重新拾取
- trip_wire 触发分支（`ZhenfaKind::TripWire =>`）：复用 P0 接入面 warning_alerts 路径——命中任意活动实体（非 owner）→ push warning_alert（报警节流走 `WARD_ALERT_THROTTLE_TICKS`=60s，`mod.rs:52`，与 warning_alerts 一致）→ owner narration + `ZhenfaSensePulse(ZhenfaWardAlert)`；**无伤害**；断线后失效（`survival_ticks` 到期或触发一次后耐久-1，归零自毁）

**视听**：
- **beast_trap 咬合**：
  - SFX `audio/recipes/beast_trap_snap.json`：主层 `block.iron_trapdoor.close` pitch=1.6 vol=0.9 delay=0t；次层 `entity.iron_golem.hurt` pitch=1.4 vol=0.3 delay=1t（金属夹腿质感）
  - 粒子：新 vfx_event `gameplay_vfx::BEAST_TRAP_SNAP = "bong:beast_trap_snap"`；client `ZhenfaActionVfxPlayer` 新增 `Kind.BITE_SNAP`——`BongSpriteParticle` burst 8 颗，spawn 模式 radial（陷阱中心 0.4 格半径外抛），lifetime 14t，颜色 `#9AA0AA`（铁屑灰），初速 0.18 向外+0.1 向上，贴图复用现有 `lingqiRippleSprites` 调灰；`VfxBootstrap` 注册一行
- **trip_wire 触发**：
  - SFX `audio/recipes/trip_wire_trigger.json`：主层 `block.tripwire.click_on` pitch=1.0 vol=0.7 delay=0t；次层 `entity.arrow.hit_player` pitch=1.2 vol=0.4 delay=2t（报警铃感）
  - 粒子：复用 P1 接入面 `emit_zhenfa_vfx(ZHENFA_WARD, ...)`（蓝色 `#66BBFF` 脉冲，与 WarningTrap 同视觉，强化"报警阵"统一语言）
- **HUD 事件流**（owner 视角，复用现有事件流通路）：
  - trip_wire：narration `push_player` scope=player style=Perception「绊线被人碰断，三格内有动静。」
  - beast_trap：narration `push_player` scope=player style=Perception「困兽圈夹住了什么，那东西正在挣扎。」

**测试（饱和化）**：
- `Immobilized` apply：移速归零（happy）；duration 到期自动解除（状态转换 active→expired）；被击杀提前解除（状态转换 active→removed）；magnitude/duration 边界（0t 立即失效）
- beast_trap：野兽命中定身（happy）；玩家不被 beast_trap 定（目标过滤，错误分支）；owner 自己不触发；咬合后不重复触发（同实体二次进入无效）；回收重置后可再触发（状态转换 armed→snapped→disarmed→armed）
- trip_wire：命中任意实体报警（happy，玩家+野兽各一）；报警节流（`WARD_ALERT_THROTTLE_TICKS`=60s 内连续触发只报一次 owner narration）；owner 不触发自家绊线；无伤害断言（`Wounds` 不变）
- 音效 recipe 结构 pin：两个 JSON 层数/sound id/pitch/vol/delay 对拍（schema 校验）

## P2 — bait_stake 诱饵（NPC 仇恨改造）

**模块**：`server/src/npc/spawn/common.rs`、`server/src/npc/brain/mod.rs`、`server/src/zhenfa/mod.rs`、`server/assets/items/`（新 `bait_stake`）、`server/src/craft/workbench_recipes.rs`（新配方）、client VFX

**交付物**：
- 新物品 `bait_stake`（诱饵桩，凡物）：`server/assets/items/workbench_materials.toml` 新条目（category=misc 物料，grid 1×2，base_weight 2.0，rarity common，**spirit_quality_initial=0.0** 纯凡物，description「稻草捆人形，挂破布残肉。野兽与散修远远见了便要扑上来撕咬。」）；新配方走下一空闲编号（当前 recipe 注释到 `// #103`，新增取 `// #104 诱饵桩`，**实施时 grep `// #` 末号 +1 取号避免撞车**，CraftCategory::ZhenfaTrap，见 §8.1 #6）
- 新 `DecoyTarget(Entity)` component（`npc/spawn/common.rs:166` `DuelTarget` 同款），挂在桩 entity 上标记其为可被仇恨的诱饵
- `NpcBlackboard` 加 `decoy_target: Option<(Entity, u64)>` 字段（entity + expire tick）+ Default
- `update_npc_blackboard`（`npc/brain/mod.rs:289`）**新增 query 参数** `decoy_query: Query<(Entity, &Position), With<DecoyTarget>>`（当前签名只有 `npc_query/player_query/all_positions/game_tick/perf_probe`，无任何 DecoyTarget 入口——必须补这一参数才能扫到桩）。decoy 判定按**真实控制流**插入（见 §8.1 #2）：`DuelTarget` continue 早退（:305）→ nearest_player 扫描循环（:318，实际先内联跑完）→ retaliation 块（:343，continue 早退）→ **decoy 扫描判定（retaliation 块之后、最终 nearest_player 回填之前）** → 末尾 nearest_player 回填（:358）。decoy 只引走"未进入战斗的巡逻仇恨"，**不能从真实威胁（DuelTarget/retaliation continue）回切**（§8.1 #2）。decoy 命中时写 `blackboard.nearest_player = Some(stake_entity)` + `player_distance` + `target_position = 桩坐标`
- bait_stake 桩 entity：放置时 `commands.spawn((ZhenfaAnchor, DecoyTarget, Position, BaitDurability(N)))`（非 vanilla hack）；被攻击 N 次（§8.1：N=4）后碎裂 → despawn + 引走的 NPC 仇恨回退（清 `decoy_target`，下个 tick 重新扫描走 nearest player）

**视听**：
- **桩碎裂**：
  - SFX `audio/recipes/bait_stake_break.json`：主层 `block.wood.break` pitch=0.7 vol=0.8 delay=0t；次层 `block.bamboo.break` pitch=0.9 vol=0.4 delay=1t（稻草碎裂感）
  - 粒子：新 vfx_event `gameplay_vfx::DECOY_BREAK = "bong:decoy_break"`；`ZhenfaActionVfxPlayer` 新增 `Kind.STRAW_SCATTER`——`BongSpriteParticle` burst 10 颗，radial，lifetime 18t，颜色 `#C8B878`（稻草黄），初速 0.22 向外+重力下落，贴图复用调色
- **NPC 转火桩**（桩顶持续标记）：
  - 粒子：新 vfx_event `gameplay_vfx::DECOY_TAUNT = "bong:decoy_taunt"`；`Kind.TAUNT_PULSE`——`BongGroundDecalParticle`/sprite continuous 低频（每 10t 1 颗），桩顶 1.2 格高，lifetime 12t，颜色 `#D86060`（挑衅红），spawn=point
- **HUD**：无 owner 报警（诱饵是被动引仇恨，不主动报警，区别于 trip_wire）。仅桩碎裂时若 owner 在视野范围内见碎裂粒子。

**测试（饱和化）**：
- NPC 仇恨转移：范围内巡逻 NPC 仇恨指向 decoy（happy）；范围外不受影响（边界）
- 优先级回切：被玩家攻击中的 NPC（retaliation_target 活跃）**不被** decoy 引走（§8.1 #2，错误分支）；DuelTarget NPC 不被 decoy 引走
- 桩碎仇恨回原：桩被攻击 4 次碎裂后，引走的 NPC `decoy_target` 清空，下 tick 回 nearest player（状态转换 lured→released）
- 多 NPC：多个 NPC 同时被同一桩引（happy）；最近桩优先（多桩时）
- 回归：`update_npc_blackboard` 改造不破已有 DuelTarget/retaliation/nearest-player 三链路（既有 NPC 行为 pin 测试全绿）
- `DecoyTarget` 桩 despawn 后 dangling entity 不 panic（`all_positions.get` 失败 → 清 decoy_target）

---

## §8 开放问题（P0 决策门前需收口）

> 全部已在 §8.1 收口。原表保留以备追溯，**实施时以 §8.1 决议为准**。

1. **beast_trap 捕获产出**：定身击杀掉常规 loot，还是"活捉"出特殊产物（兽笼/活兽物品）？
2. **decoy 仇恨优先级**：被玩家攻击中的 NPC 是否可被 decoy 引走？
3. **decoy 是否带真元吸引**：凡物桩 vs 注入少量真元增强吸引（后者走 qi_physics 守恒+逸散）？
4. **野兽目标定义**：NaturalMobKind 哪些算"野兽"可被困？Rat 不在 NaturalMobKind，鼠群是否纳入？
5. **decoy_stake 正典语义冲突 / zhenfa-content-v2 排期时序**（升 active 调研新增）
6. **beast_trap / trip_wire / bait_stake 的 CraftCategory 归属**（client 制作分页一致性）
7. **Immobilized 与 movement 系统交互**：apply 时覆写 velocity 还是改 MovementState 状态机？

---

## §8.1 决议（pre-P0 收口，2026-06-10）

### #1 decoy_stake 正典语义冲突 → 新建 bait_stake

**决议**：
1. **保留 `decoy_stake` 的 canon 语义**（欺天阵替身木桩，向天道广播假劫气）——worldview §八.3（worldview.md:812-816）明确写"替身木桩"=欺骗天道，不可改。
2. 凡物引 NPC 仇恨用**新物品 `bait_stake`（诱饵桩）**，spirit_quality=0.0 纯凡物，放置走 `ZhenfaKind::DecoyStake` 变体（变体名沿用 DecoyStake，但物品 id 是 `bait_stake`，wireName=`decoy_stake`）。
3. 拒绝改写 `decoy_stake` description——它虽当前仅有配方无功能（`workbench_recipes.rs:1066` 唯一引用，是另一个待消杀僵尸），但其语义归属欺天阵 plan，不在本 plan 范围。本 plan 在 `bait_stake` 注释 + plan 内双锚定二者区别。

**落点**：`server/assets/items/workbench_materials.toml`（新 `bait_stake`）/ `server/assets/items/materials.toml:221-229`（`decoy_stake` 不动）/ plan §接入面 worldview 锚点 + P2 交付物 / worldview.md:812-816（依据，不改）

### #2 decoy 仇恨优先级

**决议（修正：按真实控制流描述 + 声明新增 query）**：
1. **核验澄清（控制流不是干净线性链）**：`update_npc_blackboard`（`brain/mod.rs:289`）实际不是 `DuelTarget>retaliation>nearest` 的线性 override 链——而是：①`DuelTarget` 命中 `continue` 早退（:305）；②`nearest_player` 扫描循环在 retaliation **之前**就已内联跑完（:318，结果暂存 `nearest_player/nearest_distance/nearest_pos`）；③retaliation 块用 `continue` 早退（:343）；④末尾才把暂存的 nearest_player 回填进 blackboard（:358）。另外该 system **签名无任何 DecoyTarget query**，要扫"范围内最近桩"必须新增 `decoy_query: Query<(Entity,&Position),With<DecoyTarget>>` 参数（plan P2 已补声明）。
2. decoy **只影响未进入战斗的巡逻仇恨**——`DuelTarget`/`retaliation_target` 命中即 `continue` 早退，天然不会落到 decoy 判定，无需额外回切逻辑。decoy 扫描**插在 retaliation 块之后、末尾 nearest_player 回填之前**：若有范围内最近 DecoyTarget 桩，则覆写 `blackboard.nearest_player=桩entity` + `player_distance` + `target_position=桩坐标`，否则走原 nearest_player 回填。被玩家攻击中的 NPC（retaliation 活跃 → 已 continue）不被 decoy 引走。
3. 拒绝"decoy 可中断战斗"——否则诱饵桩变成无脑脱战神器，破坏散修利己 AI（worldview §七）。

**落点**：`server/src/npc/brain/mod.rs:289`（新增 `decoy_query` 参数）/ `:343` retaliation 块后 + `:358` nearest 回填前（decoy 插入点）/ `server/src/npc/spawn/common.rs:170`（NpcBlackboard 加字段）/ plan §P2

### #3 decoy 是否带真元吸引

**决议**：
1. **v1 纯凡物**，不带真元吸引。`bait_stake` spirit_quality=0.0，不接 `qi_release_to_zone`，不碰 ledger。
2. 引仇恨纯靠 `DecoyTarget` 几何范围（半径走 `OrdinaryTrapKind::Decoy.detection_radius()`，建议 8 格），不依赖灵气浓度。
3. 拒绝 v1 注入真元——避免与 §七 噬元鼠群"灵气波动吸引"机制耦合、避免新增 qi_physics 调用面。"真元增强吸引"留 v2 待 fauna/追踪系统就绪再评估。

**落点**：`server/src/zhenfa/trap_content.rs:48`（Decoy detection_radius）/ plan §qi_physics 锚点 + §P2 交付物

### #4 野兽目标定义

**决议（修正：野兽判定走 `FaunaTag`，不走 `NaturalMobKind`）**：
1. **核验澄清**：`NaturalMobKind`（`world/mob_spawn.rs:12`）**没有 derive Component**，只是 `spawn_mob_at` 的调度枚举，从不挂到实体上——按它查目标永远查不到。实际生成的灰烬蛛（`spawn_spider.rs:84`）携带的是 `FaunaTag::new(BeastKind::Spider)`（`fauna/components.rs:364` `FaunaTag` 有 `Component` derive），这才是挂在实体上、可被 ECS query 的野兽标签。**野兽判定必须查 `FaunaTag.beast_kind`，不查 `NaturalMobKind`**。
2. v1「野兽」集合 = `BeastKind::{Rat, Spider, GreenSpider, JungleScorpion, CockadeSnake}`（醒灵/引气级低阶野兽，`fauna/components.rs:9`）。引气级以上 BlueSpider/IceScorpion/MandrakeSnake 等及 Boss 级（LivingPillar/Heiwushi/PoisonDragon/BoneDragon/Whale）**不困**——困兽圈是凡物机关，困不住高境界妖兽。Zombie/Skeleton/Creeper（凡怪/亡灵，无 FaunaTag）、Rogue/Daoxiang（道伥=人形，无 FaunaTag）天然不命中（无 FaunaTag → `is_beast_target` 取不到 component → false）。
3. `is_beast_target(tag: &FaunaTag) -> bool` 判定 `tag.beast_kind ∈ 上述集合`。**玩家/凡怪/道伥无 `FaunaTag` 恒 false**（玩家不被困兽圈困）。触发扫描需 query `Option<&FaunaTag>`，无 FaunaTag 直接跳过。
4. 拒绝"全 BeastKind 都困"——会让困兽圈变成万能控场，破坏陷阱"针对低阶野兽"的流派定位。后续若要纳入更多 BeastKind 变体（如固元级），在集合 match 加一行即可（接口先于实现锁定）。

**落点**：`server/src/fauna/components.rs:9`（`enum BeastKind`，v1 野兽集合源）+ `:365`（`FaunaTag.beast_kind` 字段）/ `server/src/world/spawn_spider.rs:84`（灰烬蛛实际携带 FaunaTag 的依据）/ `server/src/zhenfa/mod.rs`（新 `fn is_beast_target(&FaunaTag)` + 触发扫描补 `Option<&FaunaTag>` query）/ worldview.md:715-752（依据）/ plan §P0 交付物 + 测试 + §接入面 worldview 锚点

### #5 zhenfa-content-v2 排期时序

**决议**：
1. 两个 plan 都扩 `ZhenfaKind` 枚举，**必须串行**：本 plan PR-1（扩 BeastTrap/TripWire/DecoyStake）与 zhenfa-content-v2 PR（扩 NetworkArray 等）不并行开。
2. 顺序不强制谁先——先 merge 的那个落地后，后者 rebase 时在枚举末尾追加自己的变体（proto 编号顺延）。冲突仅在枚举定义 + proto + proto_convert + client 四处，rebase 成本低。
3. consume-plan 实施前 grep `ZhenfaKind` 确认当前最大 proto 编号，新变体从 max+1 起编号（避免编号撞车）。

**落点**：`server/src/zhenfa/mod.rs:64` / `proto/bong/envelope.proto:3455` / plan 头部依赖说明 / [[plan-zhenfa-content-v2]] §8

### #6 CraftCategory 归属

**决议**：
1. `beast_trap`(#82，`workbench.array.beast_trap`，已核 :1106) / `trip_wire`(#78，`workbench.array.trip_wire`，已核 :1048) 配方 category 改为 `CraftCategory::ZhenfaTrap`，`bait_stake`(**#104 新**，与 P2 交付物一致——#83 已被 `骨币胚` 占用 :1139，当前最大编号 #103 :1474) 同走 ZhenfaTrap。统一进 client 制作界面"阵"标签页，与 decoy_stake(#79,已 ZhenfaTrap) 一致。
2. 理由：三者均为"放置类机关陷阱"，玩家心智模型属"阵法/陷阱"，分散在 Misc 杂类会找不到。
3. 物品本身的 `category`（TOML 里 misc）不动——那是物品分类（material），CraftCategory 是配方分页，二者正交。

**落点**：`server/src/craft/workbench_recipes.rs`（`// #78` :1048 / `// #82` :1106 改 category，`// #104` 新增——实施时 grep `// #` 末号 +1 复核）/ plan §P0 + §P2 交付物

### #7 Immobilized 与 movement 系统交互

**决议（修正：消费方定位到 `navigator_tick_system`，删除 status.rs:251 错锚）**：
1. **核验澄清 A（消费方在哪）**：NPC 移动**唯一**写 `Position` 的 system 是 `navigator_tick_system`（`npc/navigator.rs:276`），不是 `brain/mod.rs`（brain/mod.rs 只做 blackboard/scorer/action，无任何位移 system，grep `velocity|MovementState|pathfind` 零命中）。该 system 已有 `MovementController::navigator_should_yield()`（`navigator.rs:320`，Dash/Leap 等 Override 期间让出 Position 写权）的"跳过移动"先例。**Immobilized 守卫挂在这里**：给 `navigator_tick_system` query 补 `Option<&StatusEffects>`，在 yield 判定旁加 `if has_active_status(status, Immobilized) { continue; }`（与 Override yield 同位早退，不写 Position）。
2. **核验澄清 B（Immobilized 怎么生效）**：`combat/status.rs:251` 是 `BodyPartResist/BodyPartWeaken` 的部位倍率 fold，**不是** Slowed/Stunned 的 tick arm——不存在 plan 原先设想的"Slowed/Stunned 共用单一 tick match arm"。实际上 Slowed 在 `attribute_aggregate_tick`（`status.rs:135`，filter 在 `:152`）聚合成 `move_speed_multiplier`；Stunned 是各 callsite 用 `has_active_status` 门控。**v1 Immobilized 走 (b) 显式守卫路线**（§本条 #1）：因 `navigator_tick_system` 当前**不读** `DerivedAttrs.move_speed_multiplier`（它直接按 nav goal speed 步进），靠移速归零（status.rs:135 聚合路线）对 NPC 无效，故必须显式守卫。
3. **不改 `movement/mod.rs:117` MovementState 状态机**（那是玩家移动状态机）。玩家被困（§8.1 #4 玩家不被 beast_trap 困，v1 不触发）：v1 不实装玩家 velocity 覆写（客户端预测复杂，留 v2）。beast_trap 目标仅 NPC，故 v1 只需 `navigator_tick_system` 守卫。
4. 拒绝改 MovementState、拒绝走 move_speed_multiplier 聚合（NPC navigator 不读该乘子，会成死状态）——控制本 plan scope。

**落点**：`server/src/npc/navigator.rs:276`（`navigator_tick_system`，query 补 `Option<&StatusEffects>` + Immobilized 守卫，与 `:320` `navigator_should_yield` 同位早退）/ `server/src/combat/status.rs:135`（`attribute_aggregate_tick`，仅作 Slowed 写法参照，**不是** Immobilized 落点）/ `server/src/movement/mod.rs:117`（MovementState 不动，仅声明边界）/ plan §P1 交付物

---

## §10 实施工作流

本 plan scope = 3 PR（P0/P1/P2 各一），< 4 PR 门槛但跨 server+client+proto+资产三层、含新枚举跨仓库契约 + 新状态 + NPC AI 改造，按 docs/CLAUDE.md §六 全套执行。

### §10.1 视觉资产多轮打磨

本 plan **无 NBT 建筑 / worldgen layout**，仅粒子 VFX + 音效 recipe + 物品图标。粒子贴图复用现有 `lingqiRippleSprites` 调色，无新建贴图（不触发 3 轮建筑打磨）。**唯一资产 TODO**：`bait_stake` 物品图标走 `/gen-image item`（见 memory `feedback_item_icon_gen`），P2 PR 阶段批量产出，生成后程序化扫透明度（memory `feedback_gen_image_transparency_failures`）。音效 recipe 是 JSON 配置非视觉资产，按常规 atomic commit。

### §10.2 PR 拆分点（依赖顺序，前一 merge 后开下一）

1. **PR-1（P0）基础设施 + 跨仓库契约**：`ZhenfaKind` 三变体 + proto + proto_convert + client 枚举/映射 + trap_content 规格扩展 + 目标过滤修正。**独立成 PR**——纯枚举/契约扩展，与 zhenfa-content-v2 串行（§8.1 #5）。
2. **PR-2（P1）beast_trap + trip_wire + Immobilized**：依赖 PR-1 的 ZhenfaKind 变体。含新 StatusEffectKind 变体 + 触发逻辑 + 视听（VFX event + 音效 recipe）。
3. **PR-3（P2）bait_stake + DecoyTarget + NPC 仇恨**：依赖 PR-1。含新物品 + 配方 + NpcBlackboard 改造 + 物品图标。NPC AI 改造回归测试是重点。

### §10.3 subagent 配置（context 隔离）

每个 PR 起独立 subagent，主线只接收 result：

```
Agent(
  subagent_type: "claude",
  model: "opus",
  prompt: "...本 PR 范围 + 必读 §8.1 决议 + 跨仓库契约四处同步要求（Rust enum→proto→proto_convert→client）+ 饱和化测试要求...\n\nultrathink"
)
```

主线负责 merge 命令（简单不耗 context）+ PR 间 CR 等待编排。

### §10.4 CodeRabbit ScheduleWakeup 等待协议

每 PR 走完整协议：`gh pr checks` pending → `ScheduleWakeup delaySeconds=1200`（最多 3 回合 60min）；fail 按严重性桶处理；修完意见**必须重等 CR re-review**，不自判通过（memory `feedback_wait_coderabbit_approve`）。前一 PR APPROVED + merge 后才开下一 PR。**额外 gate**：跨仓库契约 PR-1 必须确认 client `./gradlew build` + server `cargo test` 双绿（proto 改动易漏一端）。

### §10.5 单次 consume-plan 全自动到 merge

用户提交 `/consume-plan plan-trap-runtime-v1` 后即可下班，醒来看本 plan 是否已在 `docs/finished_plans/`。三 PR 串行自动跑完，仅严重设计问题（如 §8.1 决议在实施中被证伪）才停交人工。

---

## 红旗自查（docs/CLAUDE.md §四）

- ✅ **不自造 qi 物理常数**：三陷阱纯凡物，spirit_quality=0.0，不写 `*_DECAY*`，存活时长复用 `trap_content::survival_ticks`（已有凡阵口径）。
- ✅ **真元流动走 ledger**：本 plan 无真元流动（§8.1 #3 决议纯凡物），不涉及 QiTransfer。
- ✅ **不造近义重名 event**：复用 `ZhenfaSensePulse`/`SenseKindV1::ZhenfaWardAlert`（trip_wire 报警）、复用 `ApplyStatusEffectIntent`、复用 `WARD_ALERT_THROTTLE_TICKS`（报警节流，与 [[plan-zhenfa-content-v2]] 命名一致）；新增 `Immobilized`/`DecoyTarget` 均确认现无等价物（`StatusEffectKind` 30+ 变体无定身、NpcBlackboard 无 decoy 入口）。
- ✅ **不漏招式经脉依赖**：本 plan 不注册任何 `SkillRegistry` 招式（陷阱是物品放置触发，非主动招式），`SkillMeridianDependencies::declare` 不适用。
- ✅ **不写 emit-only 孤岛**：beast_trap 的 `ApplyStatusEffectIntent` 由现有 status_effect system 写进 `StatusEffects`（`npc_skill.rs:219` 同路），**真正"困住"的 consumer 是 `navigator_tick_system`（`npc/navigator.rs:276`）补 Immobilized 守卫**（§8.1 #7：navigator 是 NPC 唯一写 Position 的 system，brain/mod.rs 无位移逻辑，原 plan 锚点错位已修）；trip_wire 报警由现有 warning_alerts → 事件流消费；bait_stake 的 `decoy_target` 由 `update_npc_blackboard` 消费（需新增 DecoyTarget query，见 §8.1 #2 + P2）——三条出料均有真 consumer。
- ✅ **跨仓库契约不缺面**：ZhenfaKind 四处同步（Rust enum / proto / proto_convert / client 枚举+映射）写入 P0 交付物 + §10.4 双绿 gate。
- ✅ **不开第二套放置路径**：统一走 `ZhenfaPlace` + `ZhenfaKind`（client_request.rs:257 既有）。
- ✅ **decoy 语义切割**：`bait_stake`(凡物引仇恨) vs `decoy_stake`(欺天阵) vs `ZhenfaKind::DeceiveHeaven`(对天道隐身) 三者 §8.1 #1 文档双锚定，避免混淆。

## Finish Evidence

### 落地清单

- P0 陷阱放置 + 触发底盘：PR #519 / commit `6a269af76` 接通 `ZhenfaKind::{BeastTrap,TripWire,DecoyStake}`、proto/client 映射、`OrdinaryTrapKind` 规格、owner 排除与野兽目标过滤。
- P1 beast_trap + trip_wire：PR #521 / merge commit `2c844adb97bfacb9210f0d48522e42cc600e6259` 接通 `StatusEffectKind::Immobilized`、`navigator_tick_system` 定身消费、困兽夹咬合、绊线报警、P1 VFX/SFX。
- P2 bait_stake + DecoyTarget：`server/src/npc/spawn/common.rs` 新增 `DecoyTarget` 和 `NpcBlackboard.decoy_target`；`server/src/npc/brain/mod.rs` 在 retaliation 后、nearest-player 回填前扫描最近诱饵桩；`server/src/zhenfa/mod.rs` 放置 `DecoyStake` 时挂 `DecoyTarget` + `BaitDurability(4)`，4 次 `AttackIntent` 后碎裂、移除 registry/custom block/anchor 并发 `DECOY_BREAK`。
- P2 资源与配方：`server/assets/items/workbench_materials.toml` 新增 `bait_stake` 纯凡物条目；`server/src/craft/workbench_recipes.rs` 新增 `workbench.array.bait_stake` #104，并将 `trip_wire`/`beast_trap` 归入 `CraftCategory::ZhenfaTrap`；`server/assets/audio/recipes/bait_stake_break.json` 新增碎裂音效。
- P2 客户端视听：`server/src/network/gameplay_vfx.rs` 新增 `DECOY_BREAK`/`DECOY_TAUNT`；`client/src/main/java/com/bong/client/visual/particle/ZhenfaActionVfxPlayer.java` 新增 `STRAW_SCATTER`/`TAUNT_PULSE`；`client/src/main/java/com/bong/client/visual/particle/VfxBootstrap.java` 注册两个事件。

### 关键 commit

- `6a269af76`（2026-06-10）`plan-trap-runtime-v1 P0: 接通陷阱放置契约 (#519)`。
- `2c844adb97bfacb9210f0d48522e42cc600e6259`（2026-06-11）`plan-trap-runtime-v1 P1：实装困兽夹与绊线触发`。
- `7637dd0b9`（2026-06-12）`plan-trap-runtime-v1 P2：接入诱饵桩仇恨黑板`。
- `56b0a469b`（2026-06-12）`plan-trap-runtime-v1 P2：实装诱饵桩运行时与配方`。
- `6481a120b`（2026-06-12）`plan-trap-runtime-v1 P2：接通诱饵桩客户端视听`。
- `71cdd412c`（2026-06-12）`fix(plan-trap-runtime-v1): 恢复 zhenfa 独立注册 AttackIntent`。

### 测试结果

- `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test -- --test-threads=2`：8724 passed / 0 failed / 1 ignored（P2 小收口前全量验证）。
- `cd server && cargo fmt && CARGO_BUILD_JOBS=2 cargo fmt --check && CARGO_BUILD_JOBS=2 cargo clippy --all-targets -- -D warnings`：通过（P2 小收口后）。
- `cd server && CARGO_BUILD_JOBS=2 cargo test blackboard -- --test-threads=2`：19 passed / 0 failed。
- `cd server && CARGO_BUILD_JOBS=2 cargo test bait_stake -- --test-threads=2`：2 passed / 0 failed。
- `cd server && CARGO_BUILD_JOBS=2 cargo test world::tiandao_hunt::tests::production_zhenfa -- --test-threads=2`：2 passed / 0 failed。
- `cd server && CARGO_BUILD_JOBS=2 cargo test trap_runtime_audio_recipes_are_pinned -- --test-threads=2`：1 passed / 0 failed。
- `cd server && CARGO_BUILD_JOBS=2 cargo test trap_runtime_recipes_are_grouped_under_zhenfa_trap -- --test-threads=2`：1 passed / 0 failed。
- `cd client && JAVA_HOME="$HOME/.sdkman/candidates/java/17.0.18-amzn" ./gradlew --max-workers=2 test build`：BUILD SUCCESSFUL。

### 跨仓库核验

- server：`ZhenfaKind::{BeastTrap,TripWire,DecoyStake}`、`OrdinaryTrapKind::{Beast,TripWire,Decoy}`、`StatusEffectKind::Immobilized`、`DecoyTarget`、`NpcBlackboard.decoy_target`、`BaitDurability`、`gameplay_vfx::{BEAST_TRAP_SNAP,TRIP_WIRE_TRIGGER,DECOY_BREAK,DECOY_TAUNT}`。
- proto/schema：`proto/bong/envelope.proto` / `server/src/schema/proto_convert.rs` / `server/src/schema/proto_gen.rs` 覆盖陷阱三变体编号与转换。
- client：`ClientRequestProtocol.ZhenfaKind::{BEAST_TRAP,TRIP_WIRE,DECOY_STAKE}`、`MixinClientPlayerInteractionManagerAlchemy` 的 `bait_stake -> DECOY_STAKE` 映射、`ZhenfaActionVfxPlayer` 的四类陷阱视听事件。
- agent：本 plan 无 agent runtime/schema 消费面；未改 `agent/`。

### 遗留 / 后续

- `[BLOCKED: 需 /gen-image 生成 bait_stake 图标]`：Codex 不生成图标；server/client 已完成物品、配方、放置、runtime、VFX/SFX 接线，图标资产留给 `/gen-image item`。
