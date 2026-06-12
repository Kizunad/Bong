# plan-zhenfa-content-v2 — 凡阶阵法内容:聚灵阵/散灵珠/阵旗组网

> 一句话:放置类僵尸物品「阵法」消杀——`gather_array_base` 接通 `ZhenfaKind::Lingju` 空分支(聚灵真生效)、`qi_scatter_bead` 走 ledger 守恒散逸、`array_flag_basic` + `array_eye_basic` 实装**组网阵**(旗圈边界 + 眼激活,用户拍板候选 A,2026-06-10)。
>
> 来源:放置类 17 调查 workflow(opus 抽查 7/7 属实);承接 finished `plan-zhenfa-content-v1`(三凡阵 警示/爆炸/缓速 已实装,P0-P6 ✅ 2026-05-12)。

**依赖**:
- ~~plan-block-lifecycle-v1 P4 合入(放置管线)~~ —— **已解除**:该 plan 已全 PR merge 并归档(commit `398ef0182` "finish evidence 并归档",P0-P5 ✅)。放置管线 `ZhenfaPlace` C2S(`client_request.rs:257`)+ `handle_zhenfa_place_requests`(`zhenfa/mod.rs:1025`)全部就绪,本 plan 各阶段无 blocking 前置。
- qi_physics 扩展先行(P0,本 plan 内自带)。
- 同族排期:`plan-trap-runtime-v1` 声明排期依赖本 plan P0(借 ZhenfaKind 扩枚举先例 + ID 裁决)。两 plan 共享 `WARD_ALERT_THROTTLE_TICKS`(`zhenfa/mod.rs:52`)报警节流常数 + `bong$zhenfaKindForItem` client 映射点,命名必须一致。
- **proto 枚举编号撞车**(P3):本 plan P3 给 `ZhenfaKind` 加 `NetworkArray`,`plan-trap-runtime-v1.md:39/56` 也给同一枚举加 `BEAST_TRAP/TRIP_WIRE/DECOY_STAKE`——两 plan 都抢占 `proto ZHENFA_KIND_*=10` 起始编号(现到 `ZHENFA_KIND_ILLUSION=9`)。**约定**:先 merge 者取 `=10`,后者 rebase 时在枚举末尾顺延(沿用 `plan-trap-runtime-v1` §8.1 #5「保留现有编号不重排」)。两 plan 接入面对 `ZhenfaKind` 必须同时列 `proto/bong/envelope.proto:3455` + `proto_convert.rs:2956`(穷尽 match)两个落点,保持一致。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | qi_physics 扩展(`ContainerKind::EmbeddedTrap` + 散珠/组网常数)+ ID 统一裁决落地 | ✅ 2026-06-12 |
| P1 | `gather_array_base` 接通 `ZhenfaKind::Lingju`(聚灵真生效) | ✅ 2026-06-12 |
| P2 | `qi_scatter_bead` 守恒散逸(use handler + ledger transfer) | ✅ 2026-06-12 |
| P3 | 阵旗组网:旗圈边界 + 阵眼激活(`ZhenfaKind::NetworkArray`) | ✅ 2026-06-12 |

---

## 接入面(防孤岛 checklist)

### 进料(从哪取数据/物品/event)

- **zhenfa 系统**:
  - `zhenfa/mod.rs:309` `ZhenfaRegistry`(放置阵实例注册表)/ `:1213` anchor spawn(`commands.spawn((ZhenfaAnchor, ArrayImprint, Position))`,合规非 vanilla hack)
  - `zhenfa/mod.rs:1582` `tick_zhenfa_registry`(proximity 距离扫描,WarningTrap/BlastTrap/SlowTrap/Lingju 等已分发)
  - `zhenfa/mod.rs:1711` **`ZhenfaKind::Lingju => {}` 空分支**(调查坐实:`LingArrayDeployEvent` 仅 Redis 广播,server 侧聚灵从未生效)
  - `zhenfa/mod.rs:1025` `handle_zhenfa_place_requests`(放置统一入口,所有 ZhenfaKind 走此,**不另开放置路径**)
  - `zhenfa/mod.rs:990` `ZhenfaKind::Lingju => &[MeridianId::Ren, MeridianId::Du, MeridianId::Kidney]`(阵法↔经脉依赖映射,**zhenfa 用独立 map 非 `SkillMeridianDependencies`**,P3 新 `NetworkArray` 须在此补一条)
  - `zhenfa/mod.rs:49-52` `ZHENFA_FLAG_ITEM_ID="array_flag"` / `ZHENFA_PEARL_ITEM_ID="scattered_qi_pearl"` / `WARD_ALERT_THROTTLE_TICKS = 60*TICKS_PER_SECOND`
- **lingtian**:`environment.rs:38` `PlotEnvironment.zhenfa_jvling: bool`(**当前 `:53`/`:72` 硬编 false,从未被写 true**)+ `:109-117` `compute_plot_qi_cap(env)`(`if env.zhenfa_jvling { cap += 1.0 }` 消费逻辑已实装,只缺写入触发)
- **qi_physics**:
  - `release.rs:12-44` `qi_release_to_zone(amount, from: QiAccountId, zone, zone_current, zone_cap) -> Result<ZoneReleaseOutcome, QiPhysicsError>`(`ZoneReleaseOutcome.transfer: Option<QiTransfer>`,`release.rs:5-9`)
  - `excretion.rs:6-32` `qi_excretion(initial, ContainerKind, elapsed_secs, EnvField) -> f64`(已 clamp 到 `env.local_zone_qi` 下限,符合压强法则)
  - `env.rs:9-16` `ContainerKind`(**当前仅 6 变体,无 `EmbeddedTrap`,P0 新增**)+ `:20-29` `seal_multiplier()` + `:31-33` `allows_reverse_pressure()`
  - `ledger.rs:144` `QiTransferReason::ReleaseToZone`(已存在,语义吻合,**不新增变体**)
  - `constants.rs` 真元常数唯一来源(P0 新增散珠/组网常数到此)
- **craft 配方(待接通的僵尸物品产出)**:`workbench_recipes.rs:1095` `gather_array_base`(#81 聚灵阵基座,`CraftCategory::ZhenfaTrap`)/ `:1084` `qi_scatter_bead`(#80 散真元珠)/ `:1033` `array_flag_basic`(#76 阵旗·凡)/ `:1044` `array_eye_basic`(#77 阵眼·凡)——四者均有配方无系统接入(僵尸物品)
- **client**:`MixinClientPlayerInteractionManagerAlchemy.java:180-187` `bong$zhenfaKindForItem`(物品→ZhenfaKind 映射,当前仅 WARNING/BLAST/SLOW_TRAP)+ `ClientRequestProtocol.java:87-96` `ZhenfaKind` 枚举(已含 `LINGJU("lingju")`)

### 出料(产出去哪)

- 聚灵阵:plot `zhenfa_jvling=true` → `compute_plot_qi_cap` 上限 +1.0(P1)/ 组网阵 +0.5(P3,减半)
- 散灵珠:`qi_release_to_zone` 注入 zone 浓度 → emit `outcome.transfer`(`QiTransferReason::ReleaseToZone`)给 ledger 落账 + zone tag「散逸扰动」(留给追踪 plan 消费,§8 #2 收口)
- 组网阵:范围 tag(警戒 → owner HUD 事件流;小幅聚灵走 P1 同款 `zhenfa_jvling`)
- agent 广播:`network/zhenfa_v2_event_bridge.rs:11-48` `publish_zhenfa_v2_events`(已桥 `LingArrayDeployEvent` → Redis `bong:zhenfa_v2`);组网阵需扩 **TS 源头** `agent/packages/schema/src/zhenfa-v2.ts` `ZhenfaArrayKindV2` union(加 `network_array` literal)+ Rust `schema/zhenfa_v2.rs::ZhenfaArrayKindV2::NetworkArray` + proto `ZHENFA_KIND_NETWORK_ARRAY=10` + 加桥分支(详见 P3 跨仓库契约表)

### 共享类型 / event(复用优先)

- 全部复用 `ZhenfaKind`(扩 `NetworkArray` 变体)+ `ZhenfaPlace` C2S(`client_request.rs:257`),**不另开放置路径**
- ledger 复用 `QiTransferReason::ReleaseToZone`(语义吻合,**不新增近义变体**——防红旗 §四「近义重名」)
- 实体复用 `FORMATION_CORE_ENTITY_KIND = EntityKind::new(154)`(`world/entity_model.rs:47`)+ `BongVisualKind::FormationCore`(`:73`)——阵眼实体直接挂,无需新 EntityKind(§8 #4 已定案)
- 报警节流复用 `WARD_ALERT_THROTTLE_TICKS`(`zhenfa/mod.rs:52`),与 `plan-trap-runtime-v1` trip_wire 共用

### 跨仓库契约(三端 symbol,P3 必须同步扩)

| 端 | symbol | 位置 | 动作 |
|----|--------|------|------|
| server | `ZhenfaKind::NetworkArray` | `zhenfa/mod.rs:64-74` | P3 新增变体 + 各 match 臂补全(`value`/`add`/proximity tick/meridian map) |
| server | proto `enum ZhenfaKind` `ZHENFA_KIND_NETWORK_ARRAY=10` | `proto/bong/envelope.proto:3455`(现到 `ZHENFA_KIND_ILLUSION=9`) | P3 末尾追加,**保留现有编号不重排**。⚠️**编号撞车**:同族 `plan-trap-runtime-v1.md:39/56` 也抢占 `=10`(`ZHENFA_KIND_BEAST_TRAP`)——两 plan 必须协调,先 merge 者取 10,后者 rebase 顺延(见 `plan-trap-runtime-v1` §8.1 #5 编号顺延约定) |
| server | `zhenfa_kind_to_proto` arm | `proto_convert.rs:2956`(**穷尽 match,9 arm 全列无 wildcard**) | P3 加一 arm `ZhenfaKind::NetworkArray => bong::ZhenfaKind::NetworkArray as i32`,**漏补即 non-exhaustive match 编译失败** |
| agent(源头) | TS `ZhenfaArrayKindV2` union | `agent/packages/schema/src/zhenfa-v2.ts:4-11`(现 6 literal,无 network_array) | P3 加 `Type.Literal("network_array")` + 同步 `tests/zhenfa-v2.test.ts` 正反 case。**TS 是 source-of-truth**(CLAUDE.md:TS→JSON→Rust);agent runtime `zhenfa-v2-runtime.ts:131` 严格 `validateZhenfaV2EventV1Contract`,union 未扩则 `kind=network_array` 事件被校验拒绝 → narration 静默丢失 |
| server | `ZhenfaArrayKindV2::NetworkArray` | `schema/zhenfa_v2.rs:5-12` | P3 新增 + event_bridge 映射分支 |
| client | `ZhenfaKind.NETWORK_ARRAY("network_array")` | `ClientRequestProtocol.java:87-96` | P3 新增变体,serde 字符串与 server `#[serde(rename_all="snake_case")]` 对齐 |
| client | `bong$zhenfaKindForItem` 映射 | `MixinClientPlayerInteractionManagerAlchemy.java:180` | P1 加 `gather_array_base→LINGJU`;P3 加 `array_flag_basic`/`array_eye_basic→NETWORK_ARRAY` |
| schema 测试 | `network_array` 正反 case | `agent/packages/schema/tests/zhenfa-v2.test.ts`(内联对象,**无 sample 文件**) | P3 加 vitest case:接受 `kind="network_array"`、拒绝未知 kind。三端 NetworkArray 字符串全部对齐 `network_array`(proto `ZHENFA_KIND_NETWORK_ARRAY` / Rust serde snake_case / TS literal) |

> **四端字符串对齐**(blocker):NetworkArray 的 kind 字符串必须四处一致——proto `ZHENFA_KIND_NETWORK_ARRAY`(数值 10)、Rust serde `network_array`(`#[serde(rename_all="snake_case")]`)、TS `Type.Literal("network_array")`、client wireName `"network_array"`。漏任一端即跨仓库契约缺面(docs/CLAUDE.md §四),后果:server 桥广播 → agent 校验拒绝 → narration 静默丢失。
> **schema 双向**:agent TS union 为 source-of-truth,Rust `schema/zhenfa_v2.rs` 为下游导出对齐;两侧都要扩 `NetworkArray`/`network_array`。

### worldview 锚点

- **§五.3 地师/阵法流(环境改造者)**(`worldview.md:417`):核心「真元封入环境方块做诡雷」,劣势「无人上套时预埋真元几小时后随载体朽坏白白流失」——散灵珠预埋逸散直接对应
- **§五 主战斗变量表**(`worldview.md:465`):地师·阵法主轴 = **真元逆逸散效率**(封存后自然消散速率)——聚灵阵提升 cap(降低逸散损耗)是该流派核心收益
- **§二 灵压环境**(`worldview.md:30`):灵气有物理压强,聚灵 = 局部浓度操作(cap 扩张而非真元搬运)
- **§十二.1 灵物密度阈值**(`worldview.md:804`):「天道忌满。把鸡蛋全放在一个聚灵阵里,是嫌自己死得不够快」——聚灵阵增加天道 gaze weight(已在 `LingArrayDeployEvent.tiandao_gaze_weight` 体现,P1/P3 沿用)

### qi_physics 锚点(强约束)

- **聚灵 = 容量扩张非真元搬运**:P1/P3 **不调** `qi_release_to_zone`/`transfer`,只写 `zhenfa_jvling=true` + emit 审计 `LingArrayDeployEvent`(cap 扩张是局部压强操作,无跨账户真元搬运 → 不触发守恒律)
- **散灵珠 = 守恒搬运**:P2 `qi_release_to_zone(bead_qi, QiAccountId::container("qi_scatter:{owner}:{instance}"), zone, zone_current, zone_cap)` → **必须 emit `outcome.transfer`** 并由 system apply 到 `WorldQiAccount`(前车之鉴:`abstract_combat` emit-only 无 consumer = 吞真元红旗,阻塞 merge)
- **预埋未触发衰减**:走 `qi_excretion(bead_qi, ContainerKind::EmbeddedTrap, elapsed_secs, env)`,**禁止自写衰减常数**;`EmbeddedTrap` 变体 P0 新增到 `env.rs`(详见 §8.1 #5 决议)
- **新物理常数全归 `qi_physics/constants.rs`**:`QI_SCATTER_BEAD_CAPACITY`、`QI_NETWORK_ARRAY_LINGJU_CAP_BONUS`,本 plan 只声明参数语义不写公式

---

## P0 — qi_physics 扩展 + ID 统一裁决落地

**纯 server / schema 逻辑,无玩家可感知行为,免视听规格。**

### 交付物

1. **`ContainerKind::EmbeddedTrap` 新增**(`qi_physics/env.rs`):
   - 枚举加变体 `EmbeddedTrap`(第 7 个)
   - `seal_multiplier()` 补臂:返回 `0.45`(介于 `WieldedInWeapon` 0.35 与 `LooseInPill` 0.55 之间——预埋方块封存优于丹药、弱于持械,对应正典「几小时随载体朽坏」中速逸散,见 §8.1 #5)
   - `allows_reverse_pressure()` 补臂:`false`(预埋诡雷不从环境反吸真元)
   - 修正历史幽灵引用:`plan-zhenfa-content-v1.md:43` 及本骨架原文误将 `BondKind::EmbeddedTrap`(`combat/carrier.rs:59`,暗器载体结合枚举,与 qi_physics 无关)当作 `ContainerKind`——P0 落实真正的 `ContainerKind::EmbeddedTrap`

2. **常数新增**(`qi_physics/constants.rs`):
   - `pub const QI_SCATTER_BEAD_CAPACITY: f64 = 3.0;`(单颗散珠封存真元量,语义参 `QI_TSY_REFERENCE_POOL` 量级,见 §8.1 #3)
   - `pub const QI_NETWORK_ARRAY_LINGJU_CAP_BONUS: f32 = 0.5;`(组网阵聚灵 cap 加成,Lingju 满阵 +1.0 的一半,见 §8.1 #3)

3. **三组 ID 统一裁决**:
   - **`gather_array_base` ↔ `zhenfa_array_lingju`**(红旗 #10):`gather_array_base`(workbench 配方 `workbench_recipes.rs:1095` 产出)定位为 Lingju 凡阶唯一来源,统一走 `ZhenfaKind::Lingju`;旧 `craft/mod.rs:460` 的 `zhenfa_array_lingju`(非 workbench 旧配方,无其他引用)标僵尸物品,本 plan 范围外删除归 `plan-economy-zombie-cleanup` 或加注释「deprecated,见 gather_array_base」,**不在 P0 删**(避免跨 plan 越界),只在 plan 文档锚定二者关系
   - **`qi_scatter_bead` ↔ `scattered_qi_pearl`**(红旗 #11):语义切割——`qi_scatter_bead`(`workbench_recipes.rs:1084` 产出)= 主动投掷/埋设散逸道具(P2 接);`scattered_qi_pearl`(`zhenfa/mod.rs:50` `ZHENFA_PEARL_ITEM_ID`)= 破阵被动掉落物(已实装)。两 item_id 各加代码注释互指,文档双锚定
   - **`array_flag_basic`/`array_eye_basic` ↔ `array_flag`**(红旗 #12):组网阵走**新** `ZhenfaKind::NetworkArray`(P3);旧 `array_flag`(`ZHENFA_FLAG_ITEM_ID`,`mod.rs:2561/3104` 拆阵检测用)保持原义不动

### 测试声明(饱和化)

- `qi_physics::env` pin 测试:`EmbeddedTrap` 的 `seal_multiplier()==0.45`、`allows_reverse_pressure()==false`;**全 7 变体** `seal_multiplier` 各一条专属断言(锁枚举完整性,新增变体不破坏旧值)
- `qi_excretion(_, EmbeddedTrap, _, _)`:happy path(逸散随时间衰减)、边界(`elapsed_secs=0` 不变、`initial<=local_zone_qi` 不逸散)、`initial=NaN`/`elapsed=inf` 错误分支返回 clamp 值
- 常数存在性 + 量级断言:`QI_SCATTER_BEAD_CAPACITY > 0`、`QI_NETWORK_ARRAY_LINGJU_CAP_BONUS == 0.5`
- 三组 ID grep 唯一性(CI 可加 `grep -c` 守卫脚本或人工核验,文档列出预期 hit 点)

---

## P1 — gather_array_base 接通 ZhenfaKind::Lingju(聚灵真生效)

### 交付物(server)

- **client `bong$zhenfaKindForItem`**(`MixinClientPlayerInteractionManagerAlchemy.java:180`):加 `case "gather_array_base" -> ClientRequestProtocol.ZhenfaKind.LINGJU`(复用已存在的 `LINGJU("lingju")` 变体,无需新增枚举)
  - **ItemCategory 风险核实**:`gather_array_base` 走 `ZhenfaPlace` C2S(右键放置)而非 inventory 装备 MAIN_HAND,不触发 `inventory/mod.rs:3839` 的装备槽校验路径,无 block-lifecycle MAIN_HAND 拒绝问题(放置管线已独立处理)
- **Lingju tick 实装**(`zhenfa/mod.rs:1711` 空分支):
  - `ZhenfaKind::Lingju => { ... }` 改为:阵法覆盖半径内的所有 plot 标 `zhenfa_jvling = true`
  - 阵移除/破坏时(`ArrayDecayEvent`/拆阵路径)回写 `zhenfa_jvling = false`
  - **双阵叠加规则**:同一 plot 被多阵覆盖时取 **max**(布尔 OR,有任一阵即 true);cap 加成不相加(守恒视角,§8.1 #3)
  - **不调 ledger**(聚灵 = cap 扩张,非真元搬运,符合 qi_physics 锚点)
  - `LingArrayDeployEvent` 继续 emit(回归保留,`tiandao_gaze_weight` 字段沿用)
- **fn 抓手**:`fn apply_lingju_effect(instance, &mut plot_env_writer)` / `fn clear_lingju_effect(...)`(命名供下游 grep)

### 视听规格(P1 聚灵激活)

- **粒子**:激活/持续期青绿光柱
  - 基类 `BongLineParticle`,8 根 radial(以阵心为原点 45° 等分),朝上(velocity y=+0.04,xz=0)
  - lifetime 20 ticks;颜色 `#7FD8A8`(灵气汇聚青绿,同源 `LingtianActionVfxPlayer` fallback `#66FFCC`)
  - spawn 模式 continuous 低频(每 40 ticks 触发一轮,避免刷屏)
  - 复用现有贴图(`BongLineParticle` 默认),新 VfxPlayer 类 `LingjuActivatePlayer`,`bong:vfx_event` ID `bong:lingju_activate`(VfxRegistry.java + VfxBootstrap.java 新注册)
- **音效**:audio_recipe `bong:lingju_activate`
  - layer 1:`block.amethyst_block.chime`,pitch 0.8,volume 0.6,delay_ticks 0
  - layer 2:`block.amethyst_cluster.step`,pitch 1.2,volume 0.3,delay_ticks 4(灵气流动余韵)
- **narration**(放置成功时,scope=zone,style=perception):
  - 「此地灵气似有汇聚之势,呼吸间多了几分清润。」
  - 「脚下方块隐隐泛起微光——聚灵阵已成。」
  - (天道视角嘲讽,scope=zone,style=narrative)「又一个把家当往一处堆的。天道的眼睛,最爱这种亮堂的地方。」

### 测试声明(饱和化)

- happy path:阵心半径内 plot `compute_plot_qi_cap` 比 base 高 1.0(断言取 `QI_*` 常数引用比对,不写字面)
- 边界:半径边缘 plot off-by-one(恰好在半径内 vs 外 1 格)、半径外 plot cap 不变
- 状态转换:放置→true、`ArrayDecayEvent`→false、破阵→false;双阵覆盖同 plot 取 max(两阵 cap 不叠加成 +2.0)
- 错误分支:无效阵实例(已移除)tick 不 panic;`gather_array_base` 之外物品不触发 Lingju
- 回归:`LingArrayDeployEvent` 仍 emit 且 `tiandao_gaze_weight > 0`
- client:`bong$zhenfaKindForItem("gather_array_base")` 返回 `LINGJU`(mixin 单测或映射表 pin)

---

## P2 — qi_scatter_bead 守恒散逸(use handler + ledger transfer)

### 交付物(server)

- **use handler**(投掷/埋设 `qi_scatter_bead`):
  - 主动使用 → 立即破裂:`qi_release_to_zone(QI_SCATTER_BEAD_CAPACITY, QiAccountId::container("qi_scatter:{owner}:{instance}"), zone, zone_current, zone_cap)` → 取 `outcome.transfer` → **由 system apply 到 `WorldQiAccount`**(`ledger.transfer(QiTransferReason::ReleaseToZone)`),非 emit-only
  - 注入后给 zone 挂 tag「散逸扰动」(zone-level tag,留给追踪/嗅探 plan 读;§8.1 #2:当前无消费方,效果收敛为「zone 浓度短时升高 + tag」,**不造 emit-only 孤岛**——浓度升高本身经 ledger 落实,tag 是预留消费面)
  - 预埋(放置未触发):每 tick 走 `qi_excretion(remaining, ContainerKind::EmbeddedTrap, elapsed_secs, env)` 持续逸散,逸散量经 ledger transfer 守恒还 zone;`remaining` 归零(≤ `QI_EPSILON`)→ 自毁实体
- **fn 抓手**:`fn handle_scatter_bead_use(...)` / `fn tick_scatter_bead_excretion(...)`(供 grep)
- **守恒不变量**:任意时刻 `bead_remaining + Σ(已注入 zone) == QI_SCATTER_BEAD_CAPACITY`(ledger 账面闭合)

### 视听规格(P2 散珠破裂)

- **粒子**:破裂白雾喷散
  - 基类 `BongSpriteParticle`,burst 14 颗,radial(以破裂点为原点,水平面 ±15° 锥散,velocity 0.08-0.15 随机)
  - lifetime 16 ticks;颜色 `#E8F0EE`(灰白雾)
  - spawn 模式 burst(一次性);复用 `BongSpriteParticle` 默认贴图
  - VfxPlayer 类 `ScatterBurstPlayer`,`bong:vfx_event` ID `bong:scatter_burst`(新注册)
- **音效**:audio_recipe `bong:scatter_burst`
  - layer 1:`block.glass.break`,pitch 1.4,volume 0.7,delay_ticks 0
  - layer 2:`entity.breeze.idle_air`,pitch 1.0,volume 0.4,delay_ticks 3(真元逸散气流)
- **narration**(主动破裂时,scope=zone,style=perception):
  - 「珠子应声碎裂,一缕灰白真元散入空气,周遭气息变得浑浊难辨。」
  - 「这片地灵气被搅乱了——任何想循气追踪的,此刻都会迷失方向。」

### 测试声明(饱和化)

- **守恒断言**(核心):zone 真元增量 == 珠子减量 == `outcome.transfer.amount`(取 ledger 账面,不取字面);`bead_remaining + 已注入 == QI_SCATTER_BEAD_CAPACITY`
- clamp:zone 已达 cap 时注入被 `qi_release_to_zone` clamp,溢出量不凭空消失(返回实际转移量)
- 逸散曲线:预埋 N 秒后 `qi_excretion(EmbeddedTrap)` 单调递减、`elapsed=0` 不变、归零自毁
- 错误分支:重复使用同一已破裂珠 → 拒绝(instance 已消耗);非 owner 触发埋设珠的处理
- emit→apply 闭环:断言 `WorldQiAccount` 真实变更(防 emit-only 红旗),不只断言 event 发出
- zone tag「散逸扰动」挂载/到期清除

---

## P3 — 阵旗组网(旗圈边界 + 阵眼激活)

### 玩法

阵旗(`array_flag_basic`,凡,spirit_quality=0)放 3-4 角圈定凸多边形边界 → 阵眼(`array_eye_basic`,spirit_quality=0.5)放圈内激活 → 组网成阵。凡阶效果弱:警戒 + 小幅聚灵。

### 交付物(server)

- **`ZhenfaKind::NetworkArray` 新变体**(`zhenfa/mod.rs:64`):各 match 臂补全(`value`/`add`/proximity tick 分发/`:990` 经脉依赖 map 加一条,凡阶建议 `&[MeridianId::Ren, MeridianId::Du]`)。**同步 proto 侧**:`proto/bong/envelope.proto:3455` 加 `ZHENFA_KIND_NETWORK_ARRAY=10` + `proto_convert.rs:2956` `zhenfa_kind_to_proto`(穷尽 match,加 arm 否则编译失败)——与 `plan-trap-runtime-v1` 编号协调(详见跨仓库契约表)
- **`NetworkArrayRegistry`**(新 struct,`zhenfa/mod.rs` 或 `zhenfa/network_array.rs` 新模块):
  - 字段:`flags: Vec<(BlockPos, owner)>`(旗位置集合)、`eye: Option<Entity>`(阵眼,复用 `FORMATION_CORE_ENTITY_KIND`)、`bounds: ConvexHull`、`active: bool`
  - 组网判定 `fn try_form_network(eye_pos, &flags, max_area, eye_flag_max_dist) -> Option<NetworkArray>`:扫描半径内旗 ≥3 且围成凸多边形面积 ≤ 上限 → 成阵
  - 几何:2D 凸包 / 鞋带公式算面积(Valence/Bevy 无现成 spatial query,自实装简单 2D 凸多边形面积 + 点在多边形内判定)
- **凡阶效果**:
  - 警戒:圈内 NPC/玩家进入 → owner 收 HUD 事件流提示,复用 `WARD_ALERT_THROTTLE_TICKS` 节流(与 WarningTrap 同款)
  - 小幅聚灵:圈内 plot 加 `+0.5` cap。**注意**:`compute_plot_qi_cap`(`environment.rs:117`)当前对 bool `zhenfa_jvling` 硬编 `+1.0`,无法表达 +0.5。实施二选一(实施定案,优先 A):**A** 把 `PlotEnvironment.zhenfa_jvling: bool` 升为 `zhenfa_lingju_tier: enum { None, Network, Full }`,`compute_plot_qi_cap` 按 tier 加 `QI_NETWORK_ARRAY_LINGJU_CAP_BONUS`(0.5)/ Lingju `+1.0`,P1/P3 同步改写入;**B** 加并列 bool `zhenfa_network_jvling` + 独立 `+0.5` 分支。两者双阵叠加均取 max(同 plot 被 Full+Network 覆盖取 Full +1.0,不相加)。tier 改动需同步 P1 写入路径与 `environment.rs` 既有 5 处 `compute_plot_qi_cap` 单测。
- **拆阵**:任一旗/眼被破坏 → `active=false` 全阵失效 + owner 提示 + 圈内 plot 回 `zhenfa_jvling=false`
- **agent 广播 + 三端 schema 对齐**(详见 P3 跨仓库契约表):
  - TS 源头 `agent/packages/schema/src/zhenfa-v2.ts:4` `ZhenfaArrayKindV2` union 加 `Type.Literal("network_array")` + `tests/zhenfa-v2.test.ts` 正反 case
  - Rust `schema/zhenfa_v2.rs:5` `ZhenfaArrayKindV2::NetworkArray` 新增
  - proto `proto/bong/envelope.proto:3455` `ZHENFA_KIND_NETWORK_ARRAY=10`(与 `plan-trap-runtime-v1` 编号协调,先 merge 取 10)+ `proto_convert.rs:2956` `zhenfa_kind_to_proto` 加 arm(穷尽 match)
  - `network/zhenfa_v2_event_bridge.rs` 加 `NetworkArrayDeployEvent` → Redis `bong:zhenfa_v2` 桥分支
  - **校验链**:agent runtime `zhenfa-v2-runtime.ts:131` 严格 `validateZhenfaV2EventV1Contract`,TS union 未扩则 network_array 事件被拒、narration 静默丢失
- **fn 抓手**:`fn try_form_network` / `fn dissolve_network` / `fn network_warning_tick`(供 grep)

### 视听规格(P3 成阵/破阵)

- **粒子(成阵瞬间)**:旗间青色光弦连线
  - 基类 `BongRibbonParticle`,沿凸多边形每条边一条光弦(3 旗=3 弦,4 旗=4 弦)
  - lifetime 30 ticks;颜色 `#96D6EC`(冷青);spawn 模式 burst(成阵触发一次)
  - 各旗顶火星:`BongSpriteParticle` burst 3 颗/旗,velocity y=+0.1,lifetime 12t,颜色 `#96D6EC`
  - 阵眼激活复用 `FormationActivatePlayer`(`bong:formation_activate`,已实装),新弦连线用新 player `NetworkArrayFormPlayer`,`bong:vfx_event` ID `bong:network_array_form`(新注册)
- **音效**:
  - 成阵 audio_recipe `bong:network_array_form`:layer 1 `block.beacon.activate` pitch 1.3 volume 0.5 delay 0;layer 2 `block.amethyst_block.chime` pitch 1.0 volume 0.3 delay 6
  - 破阵 audio_recipe `bong:network_array_break`:layer 1 `block.beacon.deactivate` pitch 0.9 volume 0.5 delay 0;layer 2 `block.glass.break` pitch 0.7 volume 0.4 delay 2
- **HUD 事件流**(owner,S2C):
  - 成阵:事件流条目「阵成」,文本「组网阵已成,边界 N 旗已连通」,图标青色,持续 5s 后 fade
  - 警戒:「阵内有动静——{方位} 有 {目标} 闯入」,WARD_ALERT_THROTTLE 节流避免刷屏
  - 破阵:「阵破」,文本「{旗/眼} 被毁,组网阵溃散」,红色,持续 5s
- **narration**(scope=player,style=narrative):
  - 成阵「旗影相连,一道无形的网在脚下铺开。这点微薄的灵气汇聚,聊胜于无,却也够你睡个安稳觉了。」
  - 破阵「一面旗倒下,整张网随之松脱——经营许久的布置,毁于一处疏漏。」

### 资产 TODO(3 轮打磨 + PROMISE,见 §10.1)

- **阵旗 bbmodel**:`array_flag_basic` 需新 bbmodel(小件,挂幡/旗杆造型,fmt5.0)——`scripts/models/gen_array_flag.py` 生成 → 用户 Blockbench 手改 → `render_bbmodel.py` 核对。3 轮打磨。
- **阵眼**:复用 `FORMATION_CORE_ENTITY_KIND`(EntityKind 154)现有模型,**不新建**(§8.1 #4)
- **item 图标**:`array_flag_basic`/`array_eye_basic`/`gather_array_base`/`qi_scatter_bead` 配套 `/gen-image item` 生成(若现无图标),生成后程序化扫透明度

### 测试声明(饱和化)

- 组网成阵:3 旗 + 眼 → 成阵;**2 旗 → 不成阵**(下限 off-by-one)
- 面积:恰好 16×16 → 成;超 1 格 → 拒(`max_area` off-by-one 边界)
- 眼-旗距离:超 `eye_flag_max_dist` 的旗不计入
- 凸多边形:点在多边形内/外判定(眼必须在圈内)、凹多边形旗布局处理(取凸包或拒绝,实施定案)
- 状态转换:成阵→`active=true`、任一旗破→`active=false` 全阵失效、眼破→失效、圈内 plot `zhenfa_jvling` 随 active 同步
- 双阵不重叠:两组网阵覆盖同 plot,cap 取 max 不叠加(守恒)
- 警戒节流:`WARD_ALERT_THROTTLE_TICKS` 内重复闯入只报一次
- 跨仓库 schema:TS `agent/packages/schema/tests/zhenfa-v2.test.ts` 加 vitest 正反 case(接受 `kind="network_array"`、拒绝未知 kind);Rust `schema/zhenfa_v2.rs` `ZhenfaArrayKindV2::NetworkArray` serde 正反断言。**注意现状无 sample 文件**(`ls samples/ | grep zhenfa` 为空,校验走内联对象),不写指向 `samples/*zhenfa*` 的假抓手
- 跨仓库 proto:`zhenfa_kind_to_proto` 全变体覆盖断言(含 `NetworkArray`,穷尽 match 漏一个即编译失败);client `bong$zhenfaKindForItem("array_flag_basic")==NETWORK_ARRAY`

---

## §8 开放问题(原表保留作历史回溯,实施以 §8.1 决议为准)

1. 组网几何:旗数下限、最大围合面积、眼-旗最大距离
2. 散灵珠「干扰追踪」的消费方
3. 聚灵幅度常数 + 双阵 cap 叠加规则
4. FormationCore bbmodel 复用 vs 新模型
5. `ContainerKind::EmbeddedTrap` 幽灵引用(调研新增)

> 全部已在 §8.1 收口。原表保留以备追溯,**实施时以 §8.1 决议为准**。

## §8.1 决议(pre-P0 收口,2026-06-10)

### #1 组网几何

**决议**:
1. 旗数下限 **3**(三角形是凸多边形最小单元),上限 **4**(凡阶不做更复杂边界)
2. 最大围合面积 **16×16=256 格²**(与 plot/chunk 量级对齐);眼-旗最大距离 **12 格**(半径内扫描);几何用鞋带公式算面积 + 射线法判点在多边形内
3. 凹多边形:旗布局非凸时**取凸包**后判面积(简化,不拒绝),凸包内任意点皆视为圈内

**落点**:`zhenfa/mod.rs` 新 `network_array` 模块 `fn try_form_network`;plan §P3 已写入常数。

### #2 散灵珠「干扰追踪」消费方

**决议**:
1. 全仓**无** `tracking_system`/`sniff_system` 模块(grep 确认),消费方暂不存在
2. 效果收敛为「zone 浓度短时升高(经 ledger 守恒注入,本身已是真实状态变更)+ zone tag『散逸扰动』」——浓度升高非空操作,tag 是预留面,**非 emit-only 孤岛**
3. 追踪逻辑消费留给未来追踪/嗅探 plan,本 plan 不实装追踪侧

**落点**:`zhenfa/mod.rs` `fn handle_scatter_bead_use`;plan §P2 已写入。

### #3 聚灵幅度常数 + 双阵 cap 叠加

**决议**:
1. Lingju 满阵 +1.0 cap(`environment.rs:117` 既有逻辑,不改);组网阵凡阶 +0.5(`QI_NETWORK_ARRAY_LINGJU_CAP_BONUS`)
2. 双阵覆盖同 plot **取 max 不相加**(守恒视角:聚灵是局部压强操作,叠加会凭空造 cap)。`compute_plot_qi_cap` 当前 bool `zhenfa_jvling` 硬编 +1.0,P3 需把它升为 tier enum(`None/Network/Full`)或加并列 bool 才能表达 +0.5,按 tier 单次判定天然取 max(见 §P3 实施二选一,优先 tier 方案 A)
3. 拒绝「相加」路线理由:cap 相加 = 局部灵气上限无限堆叠,违背 worldview §二「灵压物理压强」+ §十二「天道忌满」

**落点**:`lingtian/environment.rs:109-117` `compute_plot_qi_cap`(P3 改 tier);`lingtian/environment.rs:38` `PlotEnvironment.zhenfa_jvling`(升 tier 或加并列字段);`qi_physics/constants.rs` 新增常数;plan §P0/§P1/§P3。

### #4 FormationCore bbmodel 复用

**决议**:
1. 阵眼实体**直接挂** `FORMATION_CORE_ENTITY_KIND`(`world/entity_model.rs:47`,EntityKind 154)+ `BongVisualKind::FormationCore`,不新建 EntityKind
2. 阵旗需**新 bbmodel**(小件挂幡造型),走 §10.1 资产 3 轮打磨

**落点**:`world/entity_model.rs:47/73`;`scripts/models/gen_array_flag.py`(新);plan §P3 资产 TODO。

### #5 ContainerKind::EmbeddedTrap 幽灵引用(调研新增,blocking)

**决议**:
1. 骨架原文 + `plan-zhenfa-content-v1.md:43` 均误将 `BondKind::EmbeddedTrap`(`combat/carrier.rs:59`,暗器载体结合枚举)当作 `ContainerKind`——`qi_physics/env.rs:9-16` 的 `ContainerKind` 仅 6 变体,**无 `EmbeddedTrap`**
2. **选项 A 采纳**:P0 在 `qi_physics/env.rs` 新增 `ContainerKind::EmbeddedTrap` 变体,`seal_multiplier()` 返回 **0.45**(介于 WieldedInWeapon 0.35 与 LooseInPill 0.55),`allows_reverse_pressure()` 返回 `false`
3. 拒绝选项 B(用 `AmbientField` seal=1.0,逸散过快,几分钟流失,不符「几小时」语感);拒绝选项 C(v1 用 `survival_ticks_with_environment` 硬编半衰,绕开 qi_physics,会撞 docs/CLAUDE.md §四「禁止 plan 自写衰减」红旗)

**落点**:`qi_physics/env.rs:9-16` 枚举 + `:20-29` `seal_multiplier` + `:31-33` `allows_reverse_pressure`;plan §P0 交付物 1。

---

## §10 实施工作流

本 plan scope = 4 PR(P0/P1/P2/P3),适用 docs/CLAUDE.md §六多 PR 序列化 + subagent 隔离规范。

### §10.1 资产类 3 轮打磨 + PROMISE

P3 阵旗 bbmodel(`scripts/models/gen_array_flag.py`)+ 三个 VFX player 视觉调参属视觉资产,**禁止一次 commit**:
- Round 1 first cut → commit `(round 1/3)`
- Round 2 自我 review(`render_bbmodel.py` 渲染截图 / 粒子 spawn 参数核对)→ 修 → commit `(round 2/3)`
- Round 3 终轮(与 §P3 视听 spec 一致性 + 视觉叙事)→ commit `(round 3/3)`,末尾写 `<PROMISE>` 担保块(拼写 PROMISE)
- 纯逻辑 PR(P0/P1/P2)不适用,常规 atomic commit + 测试全绿

### §10.2 PR 拆分点(依赖顺序,前一 merge 后开下一)

1. **PR-1(P0)基础设施**:`qi_physics` 扩 `ContainerKind::EmbeddedTrap` + 常数 + 三组 ID 裁决落地 + 注释锚定。独立成 PR,避免与玩法 review 混杂。
2. **PR-2(P1)聚灵阵**:`gather_array_base` → Lingju 接通 + tick 实装 + client 映射 + 视听。依赖 PR-1 常数。
3. **PR-3(P2)散灵珠**:use handler + ledger 守恒 + `qi_excretion(EmbeddedTrap)` 逸散 + 视听。依赖 PR-1 `EmbeddedTrap`。
4. **PR-4(P3)阵旗组网**:`NetworkArray` 三端契约 + `NetworkArrayRegistry` 几何 + 警戒/聚灵 + 资产 + 视听。依赖 PR-1(常数)+ PR-2(Lingju 聚灵机制复用)。

### §10.3 subagent 配置(context 隔离)

主线不亲跑实施,每 PR 起独立 subagent:
```
Agent(
  subagent_type: "claude",
  model: "opus",
  prompt: "...本 PR 范围 + 必读 §10.1 多轮(仅 PR-4 资产)+ §测试声明饱和化要求...\n\nultrathink"
)
```
主线只接 result(200-500 token),解析 PR url,负责 merge。

### §10.4 CodeRabbit 等待协议

- `gh pr checks <PR>`:`pass`→merge;`pending`→`ScheduleWakeup delaySeconds=1200`;`fail`→按 commands/consume-plan.md 严重性桶
- 禁 sleep loop,最多 3 回合(60 min)卡死交人工
- 修完 review 必须重等 CR re-review,不自判「应该过」
- 每 PR 各走完整等待,前一未收敛不开下一

### §10.5 单次 consume-plan 全自动到 merge

用户提交 `/consume-plan plan-zhenfa-content-v2` 后即可下班——醒来看 plan 是否已迁入 `docs/finished_plans/`。全程 worktree 隔离,P0→P1→P2→P3 序列化,各 PR 走 §10.4 等待协议,全 ✅ + Finish Evidence 填完后归档。

## Finish Evidence

### 落地清单

- **P0 qi_physics 扩展与 ID 裁决**:`server/src/qi_physics/env.rs` 新增 `ContainerKind::EmbeddedTrap`,`server/src/qi_physics/constants.rs` 新增 `QI_SCATTER_BEAD_CAPACITY`/`QI_NETWORK_ARRAY_LINGJU_CAP_BONUS`,`server/src/craft/mod.rs`、`server/src/craft/workbench_recipes.rs`、`server/src/zhenfa/mod.rs` 补齐 `gather_array_base`/`qi_scatter_bead`/`array_flag_basic` 语义锚定。
- **P1 聚灵阵接通**:`server/src/zhenfa/mod.rs` 落地 `apply_lingju_effect`/`clear_lingju_effect` 与 `Lingju` tick 分发,`server/src/lingtian/environment.rs` 接入 plot cap 计算,`client/src/main/java/com/bong/client/mixin/MixinClientPlayerInteractionManagerAlchemy.java` 映射 `gather_array_base -> LINGJU`,并补 `LingjuActivatePlayer`/`lingju_activate.json` 视听。
- **P2 散灵珠守恒散逸**:`server/src/zhenfa/mod.rs` 落地 `handle_scatter_bead_use`/`tick_scatter_bead_excretion`,通过 `qi_release_to_zone` + `WorldQiAccount::transfer` 闭合 ledger,并接 `ScatterBurstPlayer`、`scatter_burst.json`、C2S `QiScatterBeadUse` 协议与 resourcepack manifest。
- **P3 阵旗组网**:`server/src/zhenfa/network_array.rs` 落地 `try_form_network` 几何,`server/src/zhenfa/mod.rs` 落地 `ZhenfaKind::NetworkArray`、`try_form_network_array`、`network_warning_tick`、`dissolve_network`/破阵反馈与凡阶聚灵,`proto/bong/envelope.proto`、`server/src/schema/proto_convert.rs`、`agent/packages/schema/src/zhenfa-v2.ts`、`agent/packages/schema/src/client-request.ts`、`client/src/main/java/com/bong/client/network/ClientRequestProtocol.java` 完成三端契约,`NetworkArrayFormPlayer`、`network_array_form.json`、`network_array_break.json` 与 `local_models/ArrayFlagBasic.bbmodel` 完成视听/模型。

### 关键 commit / PR

- `3321c06f44f2e8941bf061f5132b7564dcaafa36` · 2026-06-11 · PR #513 `plan-zhenfa-content-v2 P0：qi_physics 扩展与 ID 裁决` · merged。
- `8a39837c59b1ce67f100d60c2b5d2d185f071672` · 2026-06-11 · PR #514 `plan-zhenfa-content-v2 P1：聚灵阵接通玩法与视听` · merged。
- `4a2c985df95090d7d80260947d91a2b8596afe4b` · 2026-06-11 · PR #515 `plan-zhenfa-content-v2 P2：散灵珠守恒散逸与视听` · merged。
- `2b6d93694153a691280c64d42d7507f1bf81519d` · 2026-06-12 · PR #517 `实现阵旗组网 P3` · merged；P3 资产按 `(round 1/3)`、`(round 2/3)`、`(round 3/3)` 三轮提交,终轮 commit 含 `<PROMISE>` 担保。

### 测试结果

- `cd client && JAVA_HOME="$HOME/.sdkman/candidates/java/17.0.18-amzn" ./gradlew --no-daemon test build --max-workers=2` · 通过。
- `cd agent && npm test --workspace @bong/schema` · 通过。
- `cd server && CARGO_BUILD_JOBS=2 cargo fmt --check` · 通过。
- `cd server && CARGO_BUILD_JOBS=2 cargo test network_array -- --test-threads=2` · 通过。
- `cd server && CARGO_BUILD_JOBS=2 cargo test -- --test-threads=2` · 8699 passed,0 failed,1 ignored。
- PR #513 CI:`e2e` SUCCESS；PR #514/#515/#517 CI:`Build resource pack` SUCCESS,`e2e` SUCCESS,`Publish release asset` SKIPPED。

### 跨仓库核验

- **server**:`ContainerKind::EmbeddedTrap`、`QI_SCATTER_BEAD_CAPACITY`、`QI_NETWORK_ARRAY_LINGJU_CAP_BONUS`、`ZhenfaKind::NetworkArray`、`try_form_network`、`handle_scatter_bead_use`、`tick_scatter_bead_excretion`、`network_warning_tick` 均可 grep 命中。
- **proto / Rust schema**:`ZHENFA_KIND_NETWORK_ARRAY = 10`、`zhenfa_kind_to_proto` 的 `ZhenfaKind::NetworkArray` arm、`ZhenfaArrayKindV2::NetworkArray` serde 正反测试均已落地。
- **agent schema**:`agent/packages/schema/src/zhenfa-v2.ts` 与 `agent/packages/schema/src/client-request.ts` 均包含 `Type.Literal("network_array")`,generated JSON 已同步。
- **client**:`ClientRequestProtocol.ZhenfaKind.NETWORK_ARRAY("network_array")`、`bong$zhenfaKindForItem` 的 `array_flag_basic`/`array_eye_basic` 映射、`NetworkArrayFormPlayer` 与 VFX registry 均已接通。

### Review / CI 结论

- PR #513/#514/#515/#517 均已 merge；`/review` 均已触发,其中 PR #517 最终评论为「所有 10 个 finder 均已确认完毕,无新增阻塞问题」。
- CodeRabbit 多次返回 `Review limit reached` / usage credits 不足,按 `AGENTS.md` review gate 视为计费/限流噪声,不是代码阻塞；已有 `/review` 与 CI 结果足以归档。

### 遗留 / 后续

- 散灵珠 `scatter_disturbance` tag 的追踪/嗅探消费方仍属未来追踪类 plan,本 plan 已确保 tag 不是 emit-only 孤岛:zone 浓度变化通过 ledger 真实落账。
- inventory rollback、`zone_name_for_block` 与 `zone_name_at_pos` 合并、冗余 `init_resource`/`add_event` 清理为后续技术债,均非本 plan 阻塞项。
