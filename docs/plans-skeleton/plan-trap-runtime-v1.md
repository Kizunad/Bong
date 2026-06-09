# plan-trap-runtime-v1 — 陷阱 runtime:困兽夹/绊线/诱饵桩(骨架)

> 一句话:放置类僵尸物品「陷阱」消杀——beast_trap(困兽夹)、trip_wire(绊线报警)、decoy_stake(诱饵桩)实装真实触发功能。
>
> 来源:放置类 17 调查 workflow(opus 抽查 7/7 属实);用户拍板:陷阱需具备对应功能。

**依赖**:[[plan-zhenfa-content-v2]] P0——**排期依赖**(zhenfa-content-v2 先扩 ZhenfaKind 枚举建立先例+ID 裁决),非基础设施依赖:ZhenfaPlace 协议本就存在(client_request.rs:257),不必等其全部阶段落地。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 陷阱放置 + 触发底盘(复用 zhenfa proximity) | ⬜ |
| P1 | beast_trap 困兽(新 Immobilized 状态)+ trip_wire 报警 | ⬜ |
| P2 | decoy_stake 诱饵(新 DecoyTarget,NPC 仇恨改造) | ⬜ |

---

## 接入面(防孤岛 checklist)

- **进料**:
  - 触发先例:`zhenfa/mod.rs:1582` tick_zhenfa_registry 距离扫描(WarningTrap/BlastTrap/SlowTrap 已实装);anchor spawn `:1213`(`commands.spawn((ZhenfaAnchor, ArrayImprint, Position))`——非 vanilla hack,合规)
  - C2S:`client_request.rs:257` ZhenfaPlace/Trigger/Disarm 已有,陷阱统一走 ZhenfaPlace + ZhenfaKind 扩枚举(**不开第二套放置路径**)
  - NPC 仇恨:`npc/brain/mod.rs:319` update_npc_blackboard 只追 `ClientMarker`(玩家),`DuelTarget` override(:305)仅 1v1——诱饵需新接口
  - 状态:`combat/events.rs:81` StatusEffectKind(现无定身类,只有 Slowed/Stunned)+ `:159` ApplyStatusEffectIntent
  - 生物:`mob_spawn.rs:12` NaturalMobKind(**无 Rat**,鼠仅 tutorial 脚本 spawn)
- **出料**:beast_trap 捕获野兽(掉落/拘束)、trip_wire 给 owner 报警、decoy_stake 引走 NPC 仇恨
- **共享类型 / event**:`StatusEffectKind::Immobilized` 新变体;`DecoyTarget` 新 component;复用 WARD_ALERT_THROTTLE_TICKS 报警节流
- **跨仓库契约**:ZhenfaKind 新变体 ↔ client `bong$zhenfaKindForItem` 映射 ↔ schema sample;trip_wire 报警走 HUD 事件流 S2C
- **worldview 锚点**:§七 动态生物生态(猎兽采集);§五.3 地师/阵法流(环境陷阱);worldview.md:417 预埋衰减(若陷阱带真元组件)
- **qi_physics 锚点**:beast_trap/trip_wire/decoy_stake 均为凡物机械陷阱,**无真元参与,不碰 ledger**。若 decoy_stake 决议带"散逸真元吸引"机制(§8 #3),则走 [[plan-zhenfa-content-v2]] P2 同款 `qi_release_to_zone`,本 plan 不自写。

---

## P0 — 陷阱放置 + 触发底盘

- ZhenfaKind 扩 `BeastTrap`/`TripWire`/`DecoyStake` 三变体;client 物品映射;ZhenfaPlace 放置走通
- **触发目标过滤修正**(调查红旗 #13):`ZhenfaDamageTarget` query(zhenfa/mod.rs:1478)无 With/Without 过滤会误伤友方——陷阱触发加 owner 排除 + 目标类型过滤(beast_trap 只命中野兽,trip_wire 全命中但只报警)
- 测试:三变体放置/拆除 pin 测试;owner 不触发自家陷阱;过滤分支全覆盖

## P1 — beast_trap 困兽 + trip_wire 报警

- `StatusEffectKind::Immobilized` 新变体(定身,不可移动可挣扎,duration 后释放或被击杀)+ wire 映射
- beast_trap:proximity 命中野兽 → Immobilized + 小伤害;陷阱单次性(触发后变"已咬合"态,可回收重置)
- trip_wire:命中任意活动实体 → owner HUD 事件流报警(WARD_ALERT_THROTTLE_TICKS 节流),无伤害;断线后失效需重布
- 视听:夹子咬合 SFX `block.iron_trapdoor.close`(pitch 1.6)+ 铁屑迸溅(BongSpriteParticle burst 8 颗 #9AA0AA);绊线触发 SFX `block.tripwire.click_on` + `entity.arrow.hit_player`(delay 2t,报警铃感);HUD 事件流「陷阱有响动/绊线被触」
- 测试:野兽命中定身/玩家不被 beast_trap 定(目标过滤)/咬合后不重复触发/重置流程;trip_wire 节流(连续触发只报一次)

## P2 — decoy_stake 诱饵

- 新 `DecoyTarget` component:挂在桩 entity 上,update_npc_blackboard 扩展——范围内野兽/敌对 NPC 仇恨优先指向 decoy(优先级低于真实威胁,见 §8 #2)
- **与 ZhenfaKind::DeceiveHeaven(天道遮蔽)语义切割**(调查红旗):decoy=引仇恨凡物,DeceiveHeaven=对天道隐身,文档+注释双锚定
- 桩有耐久:被攻击 N 次后碎裂(掉落不返还)
- 视听:被攻击碎裂 SFX `block.wood.break`(pitch 0.7)+ 稻草飞散(burst 10 颗 #C8B878);NPC 转火时桩顶冒挑衅红点粒子(continuous 低频 #D86060)
- 测试:NPC 仇恨转移/真实威胁优先级回切/桩碎仇恨回原目标/多 NPC 同时引;blackboard 改造不破已有 NPC 行为(回归)

---

## §8 开放问题(P0 决策门前需收口)

1. **beast_trap 捕获产出**:定身击杀掉常规 loot,还是"活捉"出特殊产物(兽笼/活兽物品)?活捉需新物品体系,建议 v1 只定身
2. **decoy 仇恨优先级**:被玩家攻击中的 NPC 是否可被 decoy 引走(建议不可——decoy 只影响未进入战斗的巡逻仇恨)
3. **decoy 是否带真元吸引**:凡物桩 vs 注入少量真元增强吸引(后者走 qi_physics 守恒+逸散,成本高吸引强)——建议 v1 纯凡物
4. **野兽目标定义**:NaturalMobKind 哪些算"野兽"可被困(列清单);Rat 不在 NaturalMobKind,鼠群是否纳入 beast_trap 目标
