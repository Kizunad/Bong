# Bong · plan-era-state-v1 · 骨架

**天道时代状态机**——将演绎时代 Agent 发出的 `era_decree` 持久化为服务端 `WorldEraState` Resource，并将当前时代（`Calamity / Change / Deduction`）作为系数注入渡劫难度、异变兽刷出密度、派系 AI 忠诚偏置等下游系统，同时向客户端暴露高境界玩家可感知的时代环境层（天象/音效/HUD 微提示）。

## 目标

- 实装 worldview §八 天道三手段与时代博弈：agent `era_decree` 产生的全局指令转变为 server 端可查询的 `WorldEraState { era, onset_tick, intensity }`
- 工程目标：给渡劫系统 / 异变兽刷出系统 / NPC 派系 AI 提供统一时代系数读取接口（`world::era::current_modifiers()`），消除各系统各自拍板难度参数的散乱常数
- 客户端表现：通灵+ 玩家获得时代感知（天象微变化：灾劫时代雷云/变化时代灵气潮汐涌动/演绎时代叙事 overlay），低境玩家无任何 UI 提示
- 验收：agent 发出 era_decree → server WorldEraState 更新 → 渡劫阈值乘以时代系数 → 异变兽密度响应 → 高境界客户端收到天象包 e2e 闭环

**来源**：worldview §八「天道行为准则」+ §八「天道运维博弈」三手段（灵物密度/操作磨损/气运劫持），三手段分属不同时代强度区间；agent `agent/packages/tiandao/src/skills/era.md`（演绎时代 Agent，`era_decree` 风格全服广播；`global_effect` + `spirit_qi_delta` + `danger_level_delta` 已实装输出但 server 无持久 ERA 状态接收）

**前置条件**：
- `plan-agent-v2` ✅ — 演绎时代 Agent（era.md skill）已实装；agent 已能发 `era_decree` 且 Arbiter 已特殊处理该风格（不限频）
- `plan-tribulation-v1` ✅ — 渡劫系统（阈值/难度可注入系数）
- `plan-tiandao-hunt-v1` ⏳ active — 天道主动追杀修士（依赖时代系数：灾劫时代天道更敌视）
- `plan-qi-physics-v1` ✅ — 灵气浓度场（时代修正全局 spirit_qi delta 走 ledger 不可绕过）
- `plan-npc-ai-v1` ✅ — NPC big-brain AI（派系 NPC 忠诚偏置是 Scorer 输入）
- `plan-ipc-schema-v1` ✅ — IPC schema（EraDecree 已在 common.rs 定义）

**交叉引用**：`plan-tribulation-balance-v1` ⬜（骨架，时代系数是渡劫平衡校准的关键输入）· `plan-faction-wars-v1` ⬜（骨架，演绎时代推动派系冲突概率）· `plan-beast-horde-v1` ⬜（骨架，灾劫时代异变兽迁徙密度×1.5）· `plan-tiandao-hunt-v1` ⏳（灾劫时代 TiandaoAttentionScore 系数）

**worldview 锚点**：
- **§八:759 天道唯一目标**：延缓灵气消耗。时代本质是天道判断灵气消耗速率后调整干预烈度
- **§八:766 天道手段**：温和/中等/激烈/隐性/静观五档对应不同时代烈度——本 plan 将"中等"（异变兽刷新）/ "激烈"（天劫频率）作为灾劫时代，"温和"（灵气匀流）/ "隐性"（叙事引导）作为演绎时代，"变化"（生态替换）作为变化时代
- **§八:800 灵物密度阈值**：灾劫时代下阈值收紧（更容易触发道伥清理）
- **§七:742 稀有实体**：特定时代 spawn 特定稀有生物

**qi_physics 锚点**：
- 时代切换的灵气影响**不接受 agent 直接写全局 delta**（凭空增减全服灵气 = 守恒律红旗）。`era_decree` 的 `spirit_qi_delta` 只作 server 侧的**倾向/强度提示**，由 server 换算成守恒路径，从不直接落到 `WorldQiAccount`：
  - **负向（天道收紧）**：复用既有的天道每时代衰减机制 `qi_physics::tiandao::era_decay_step`（常数 `QI_TIANDAO_DECAY_PER_ERA_MIN/MAX` = 1-3%/时代上限），**不新增旁路、不新建衰减函数**。该机制**不是"系统外流/凭空蒸发"**——`WorldQiBudget::apply_era_decay` 把衰减量从 `current_total` 挪进**被追踪的沉降槽 `era_decay_accum`**，恒定锚 `initial_total` 不动，不变式 `current_total + era_decay_accum == initial_total` 始终成立（即衰减的真元仍被记账，只是不再可用）。`era_decree` 的负向 `spirit_qi_delta` 只调节 `era_decay_step` 的 `era_factor`（时代进度强度），不直接扣账
  - **正向（变化时代灵气潮汐）**：**不铸新真元**——二选一：(a) 仅改环境密度读数（`qi_physics::field` 风格，不动 ledger 余额）；(b) 在账户间走守恒 `qi_physics::ledger::QiTransfer { from, to, amount, reason: EraShift }`（如世界储池账户 → zone）。路线 (b) 需**新增 `QiTransferReason::EraShift` 变体**到 `qi_physics::ledger`（当前不存在；负向已有 `EraDecay`，正向账户间搬运语义不同，须独立变体）
  - 守恒口径锚定 `qi_physics::ledger::assert_conservation(before, after, era_decay)`——它已接受"observed 总量减少量 == 被追踪的 `era_decay` 沉降量"为合法（衰减进 `era_decay_accum`），但拒绝无去向的 drift。本 plan 任何路径都必须能通过此断言；audit-only 的 `EraShift` 若只留轨迹则 emit event、不调 `WorldQiAccount::transfer`（后者会变动 balance）
- 灾劫时代 danger_level_delta 不直接影响真元物理，只影响异变兽 aggression scorer weight

---

## 接入面 Checklist

- **进料**：agent `era_decree` 命令包（已通过 redis_bridge `bong:agent_cmd` 队列到达，server 已有 `NarrationStyle::EraDecree` 解析路径）；需从 `modify_zone` 命令的 `params.era_name / global_effect / spirit_qi_delta / danger_level_delta` 字段提取并写入 `WorldEraState`
- **出料**：`WorldEraState { era: EraType, onset_tick: u64, intensity: f32, qi_delta: f32, danger_delta: f32 }` Resource（Bevy）+ `era::current_modifiers() -> EraModifiers` 函数 + `EraChangedEvent` Bevy event；客户端 `EraAmbiance` CustomPayload（仅发给通灵+ 境界玩家）
- **共享类型**：复用 `NarrationStyle::EraDecree`（`server/src/schema/common.rs:57`）；新增 `EraType` enum（Calamity / Change / Deduction / Unknown）+ `EraModifiers { tribulation_threshold_mul, beast_density_mul, faction_loyalty_drift }`；IPC schema 新增 `EraState` 字段到 `world_state.era`
- **跨仓库契约**：server `world_state.era { era_type, intensity }` → agent 世界状态快照（agent 当前 world-model.ts 有 `era: "演绎时代 Agent"` 字段但仅作者标注，需扩为实际时代类型枚举）；client CustomPayload `bong:era_ambiance` 含 `sky_tint_hex / fog_density_delta / ambient_sound_id`，仅发通灵+ 境界
- **worldview 锚点**：§八 天道三手段 + §八 天道运维博弈
- **qi_physics 锚点**：`era_decree` 的 spirit_qi_delta 仅作倾向值；负向复用 `qi_physics::tiandao::era_decay_step`（衰减进追踪沉降槽 `era_decay_accum`，恒定锚 `initial_total = current_total + era_decay_accum` 不变，非凭空蒸发），正向走密度读数修正或账户间守恒 `QiTransfer{from,to,amount,reason:EraShift}`（新增变体）；**不接受直接写全局 delta**，全程须过 `assert_conservation`（守恒律红旗）

---

## 阶段总览

| 阶段 | 状态 | 主要交付物 | 验收标准 |
|------|------|-----------|---------|
| **P0** | ✅ 2026-06-08 | `WorldEraState` + `EraType` + `EraModifiers` + era_decree→WorldEraState 写入(经 arbiter 转发 era_name 真进料) | EraDecreeIntent 经 agent→redis→server 生产链 emit + EraChangedEvent |
| **P1** | ✅ 2026-06-08 | 下游注入:渡劫阈值×1.1(灾劫)/异变兽 spawn gate×density_mul/派系 loyalty drift | 三系统 ERA 系数 + 守恒(era 仅改 modifier 不动 qi 账户,assert_conservation 通过) |
| **P2** | ✅ 2026-06-08 | IPC schema `world_state.era` 字段三件套(TypeBox+serde+sample)+ agent world-model 同步 | 双端 serde/TypeBox 正反 sample 对拍 |
| **P3** | ✅ 2026-06-08 | Client `EraAmbiance` S2C(realm gate 三档:通灵+完整/固元10%fog/凝脉-无)+ ZoneAtmosphere 天象插值 | 通灵端天象/固元微 fog/凝脉无;游戏tick 推进 |

---

## P0 — 时代状态数据模型

- [ ] `server/src/world/era.rs`：`EraType { Calamity, Change, Deduction, Unknown }` + `WorldEraState { era, onset_tick, intensity, qi_delta, danger_delta }` Bevy Resource
- [ ] `EraModifiers { tribulation_threshold_mul: f32, beast_density_mul: f32, faction_loyalty_drift: f32 }` + `current_modifiers(era: &WorldEraState) -> EraModifiers` 函数（三个 EraType 各自一组常数，放 era_params.rs）
- [ ] `EraDecreeSystem`：监听 `bong:agent_cmd` 队列里 `style == "era_decree"` 的 NarrationCommand，解析 `params.era_name / spirit_qi_delta / danger_level_delta` → 写 `WorldEraState`；spirit_qi_delta **仅作倾向值**，server 据此走守恒路径（负向 = 调 `era_decay_step` 的 era_factor，衰减入 `era_decay_accum` / 正向 = 密度读数修正或账户间 `QiTransfer{from,to,amount,reason:EraShift}`），**不直接写全局 delta**
- [ ] `EraChangedEvent { old_era, new_era, onset_tick }` Bevy event，写入系统 emit
- [ ] ≥ 8 单测（era_decree 解析 / WorldEraState 更新 / EraModifiers 三型正确 / **负向 EraDecay 前后 `assert_conservation` 通过 + `initial_total == current_total + era_decay_accum` 不变量保持** / 正向 EraShift transfer 守恒 / 倾向值不直接落全局 delta）

---

## P1 — 下游系统接入时代系数

- [ ] `tribulation/threshold.rs`：`required_qi_pool` 公式乘 `modifiers.tribulation_threshold_mul`（灾劫时代 0.9 = 更容易被劫，演绎时代 1.1 = 暂缓；实际数值见 §8 开放问题）
- [ ] `fauna/spawn_weights.rs`：`horde_density` 读 `modifiers.beast_density_mul`（灾劫时代 1.5，演绎时代 0.8）
- [ ] `npc/faction_ai.rs`：`LoyaltyScorer` 加 `era_loyalty_drift` 偏置（变化时代 ±0.1 随机偏转，演绎时代最稳）
- [ ] `tiandao_hunt/attention_score.rs`：灾劫时代 TiandaoAttentionScore 全局乘 1.3
- [ ] ≥ 12 单测（每系统 × 三时代；灾劫时代触发更易渡劫 / 兽潮更密 / 追猎更敏感）

---

## P2 — IPC schema + agent world-model 更新

- [ ] `agent/packages/schema/` `WorldStateV1` 新增 `era: EraStateV1 { era_type: string, intensity: number, onset_tick: number }`；TypeBox 定义 + JSON sample
- [ ] `server/src/schema/world_state.rs` 同步 `EraStateV1` struct + serde 对齐
- [ ] `agent/packages/tiandao/src/world-model.ts`：`era` 字段从 hardcode 字符串改为从 `WorldStateV1.era.era_type` 读取；era.md skill 可在决策前 read 当前时代避免重复宣告同一时代
- [ ] ≥ 6 双端 schema 校验单测（TypeBox 正反 sample × 3 era 类型）

---

## P3 — 客户端时代天象表现

- [ ] `EraAmbianceS2c` CustomPayload：含 `era_type: string / sky_tint_hex: string / fog_density_delta: f32 / ambient_sound_id: string`；server 在 `EraChangedEvent` 时 broadcast 给通灵+ 境界（`Realm >= TongLing`）玩家
- [ ] 三时代参数：灾劫时代 `sky_tint: #4A1A1A, fog: +0.15, sound: ambient.weather.thunder`；变化时代 `sky_tint: #1A3A4A, fog: +0.05, sound: block.water.ambient`；演绎时代 `sky_tint: #2A2A3A, fog: -0.05, sound: entity.enderman.ambient pitch 0.5`
- [ ] client `EraAmbianceHandler`（`EraAmbianceS2c` 收包）：以 1200 tick 渐变更新 SkyRenderer 参数（不突变）；固元- 境界客户端不发包不渲染
- [ ] ≥ 8 单测（境界过滤 / 渐变时长 / 三时代颜色值正确）

---

## §8 开放问题（P0 决策门收口）

1. **时代强度 intensity 如何由 agent 传入**：era_decree 的 `params` 没有 intensity 字段，是从 `danger_level_delta` 推算还是新增字段？
2. **时代持续时间与过期**：agent 间隔 5+ 分钟才能再发 era_decree；若超过 20 分钟无新 decree，`WorldEraState` 自动衰减到 Unknown？还是上一个时代无限持续？
3. **灾劫时代渡劫阈值系数的具体值**：0.9 是否太激进（每次灾劫时代突破难度降 10%，反向激励玩家等灾劫时代突破）？考虑以 1.0 基线、灾劫加难度而非降难度（天道敌视高境）
4. **三时代互斥还是并存**：worldview 描述"三 Agent 并发推演"——灾劫/变化/演绎三 Agent 同时在跑，但服务端只能存一个 EraType？建议 `WorldEraState` 改为 3 个独立强度系数：`calamity_intensity / change_intensity / deduction_intensity`，各自从对应 agent 的 era_decree 读取
5. **客户端天象是否应对低境界玩家彻底隐藏**：worldview §五"流派识别是事件" + §六"普通玩家无神识"意味着低境界感知力弱；但完全没有任何天气暗示会让游戏感觉"断层"——建议凝脉- 无任何提示，固元有极微妙天色变化（10% fog delta），通灵+才有完整音效

---

## Finish Evidence

**验收日期**：2026-06-08 · 全 P0-P3 ✅ · 经 consume-plan 自动消费(viability gate 验证+§8收口 + 实施 + opus 对抗自检 2 轮修复)

### 落地清单
- **P0**：`server/src/world/era.rs`(WorldEraState Resource + EraType{Calamity/Change/Deduction/Unknown} + EraModifiers + era_decree_system + 20min expires + intensity 从 |spirit_qi_delta|/0.05 推算);`server/src/qi_physics/ledger.rs` 新增 `QiTransferReason::EraShift`(P0 设计选 modifier-only,EraShift 暂为 deferred 脚手架,见遗留)。
- **P0 进料(B1 核心)**:`agent/packages/tiandao/src/arbiter.ts` materializeEraCommand 转发全局 era 命令(target=全局 + era_name,bypassHardConstraints)到 redis → `server/src/network/command_executor.rs:950` execute_modify_zone 全局分支读 era_name → emit EraDecreeIntent → era_decree_system 更新 WorldEraState。**agent→redis→server 生产链闭环**(非测试绕过)。
- **P1 注入**:渡劫阈值×tribulation_threshold_mul(灾劫1.1)/异变兽 `era_beast_spawn_gate`(botany/hazard.rs spawn_attracted_mobs_from_harvest 逐只门控)/派系 loyalty drift。守恒:era 仅改 modifier 系数,**从不动 qi 账户**(apply_decree 注释+测试锁不积累 era_decay_accum),assert_conservation 通过。
- **P2 schema**:`world_state.era` 字段(TypeBox source→serde→sample 双端对拍)+ agent world-model 同步 currentEra(避免重复宣告)。
- **P3 client**:`bong:era_ambiance` S2C(realm gate 三档:通灵+ 完整 sky_tint/fog/ambient_sound / 固元 10% fog / 凝脉- 不发包)+ client EraAmbianceState→ZoneAtmospherePlanner.plan() 插值(死灵域/折叠优先)+ tick 挂 ClientTickEvents.END_CLIENT_TICK(游戏tick)。

### 关键 commit(branch auto/plan-era-state-v1)
- `0cdbcdc04` P0 WorldEraState/EraType/EraModifiers/EraChangedEvent + 23测
- `7922c183c` P1 下游注入 渡劫/兽密度/派系loyalty + 守恒律测试
- `a6d6a62a2` P2 IPC schema era 三件套 + agent world-model 双端
- (P3 client EraAmbiance commit)
- `a60ab5bc2` fix1: 入口 execute_modify_zone 全局分支 + 兽密度 gate 生产接入 + client 天象接入 + 假测试修正
- `3a163de14` fix2(B1 闭环): arbiter 转发 era_name 到 redis-bound 命令,闭合 agent→server 时代状态机生产链路 + arbiter.test pin 修正

### 测试结果
- server `cargo fmt --check` ✅ / `cargo clippy --all-targets -- -D warnings` ✅ / `cargo test`:**7982 passed / 0 failed**(含 era_decree 生产链集成测试 modify_zone_global_era_name_emits_era_decree_intent_via_production_path)
- agent `npm test`:**581/581**(含 arbiter.test 更新后 pin + 走真实 Arbiter.merge() 验 era_name 到达 merged.commands 的 B1 回归守卫)
- schema `npm test`:**554/554**
- client:era + atmosphere 测试绿;1 pre-existing 失败 BongEntityModelAssetTest(gitignored local_models,零触及实体模型)

### 跨仓库核验
- **agent** ✅:`arbiter.ts` materializeEraCommand(era_name 转发)+ world-model currentEra 同步
- **server** ✅:`WorldEraState`/`EraType`/`EraModifiers`/`EraDecreeIntent`/`era_decree_system`/`era_beast_spawn_gate`/`QiTransferReason::EraShift`/execute_modify_zone 全局分支
- **client** ✅:`EraAmbianceState`/`EraAmbianceHandler`/ZoneAtmospherePlanner 天象插值
- **契约** ✅:redis `modify_zone{target=全局,era_name}` + S2C `bong:era_ambiance` + `world_state.era` 三端对齐

### 遗留 / 后续
- **client 天象视觉待 WSLg 验收**:sky_tint/fog/ambient_sound 实际观感(逻辑/契约已测,主观视觉需人眼)。
- **EraShift 正向账户搬运 deferred**:P0 选 modifier-only 安全设计(era 切换不创生/不蒸发真元),EraShift ledger 变体保留为脚手架,变化时代正向真元搬运(路线b)待后续 plan 启用;**当前时代 spirit_qi 倾向不实际改动真元场**(守恒未违反,纯 modifier 生效)。
- **tiandao_attention_score_mul 注入 deferred**:接口字段就绪,待 plan-tiandao-hunt-v1 P0 接入(消费者未实装)。
- 2 minor:EraShift 死脚手架(建议删或 deferred 标记)/current_modifiers 方法形态 dead_code(自由函数已用)。
