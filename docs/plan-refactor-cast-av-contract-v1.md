# plan-refactor-cast-av-contract-v1 — 施法同步/技能栏/AV 单一事实源契约（重构轨 R9）

> 所属总纲：`plan-refactor-master-v1.md`。一句话：以 TypeBox-first 协议、唯一 reducer、真实 producer 授权表和原子 cutover DAG 收敛施法身份、终态与每招 AV 五件套。
>
> 阶段总览：P0 ✅ 2026-08-04；P1 ⬜；P2 ⬜；P3 ⬜；P4 ⬜。

## 现状证据（2026-08-03 P0 复核）

- `SkillRegistry` 当前只保存 `skill_id → SkillFn`，生产初始化共注册 **68** 个 resolver；`TECHNIQUE_IDS` / `TECHNIQUE_DEFINITIONS` 各 **49** 条。两集合交集为 **46**，registry-only **22**，definition-only **3**，说明 resolver、玩家入口、AV 元数据没有共同事实源（`server/src/cultivation/skill_registry.rs:71-122`；`server/src/cultivation/known_techniques.rs:67-166`）。
- server `Casting` 已保存 `source` 与 `skill_id`，但 `CastSyncV1` 只发 `phase/slot/duration_ms/started_at_ms/outcome`；client `CastSyncHandler.sourceFor()` 因此只能从当前快照猜来源并默认 `QUICK_SLOT`（`server/src/combat/components.rs:421-447`；`server/src/schema/combat_hud.rs:97-106`；`client/src/main/java/com/bong/client/network/CastSyncHandler.java:19-51,97-103`）。
- `CastPhaseV1` 已有 `Idle/Casting/Complete/Interrupt`，所以本轨不重复“新增 phase 字段”；真正缺的是稳定 cast 身份、权威来源/技能/目标与所有退出路径的一致终态。循环动画停止仍由 `cast_emit.rs` 的 skill-id 特判表分散维护，而非注册契约。
- AV 元数据已有 `DuguSkillVisual`、`TuikeSkillVisual`、`WoliuSkillVisual`、`YidaoSkillSpec` 等局部结构，字段与消费路径各异；Baomai/Tuike 仍可同时走 resolver 直发与事件 consumer，证明局部映射不能充当全局唯一真相源。
- #1287 的总纲 §1 基线门已由 `origin/main` commit `9931a3a1fdd5b4d6b38f4da2fce43f400e26bf0d`（PR #1287）满足；这只关闭该历史等待项，不覆盖总纲 §3 Wave 2 的 **R5 P1 + R6 P2 + R2 P1** release gate。R6 P3 是 §P0.3.2 中 P1-B production cutover 交付门，不是 R9-owned P1-A 的启动门。`dugu.penetrate` 当前也已改为 `visual_for(DuguSkillId::Penetrate)` 驱动 runtime animation/audio（`server/src/combat/dugu_v2/skills.rs:392-416`），旧错接结论已经关闭。

---

## 1. 术语、边界与不可变式

### 1.1 术语

| ID | 术语 | 唯一定义 |
|---|---|---|
| `I-01` | `CastSession` | 一个 server transport 连接内的施法命名空间，由 `(session_id, session_generation)` 唯一标识。 |
| `I-02` | `CastAttempt` | 一次进入 admission 的请求，包括 accepted 与所有 pre-cast reject；每次恰分配一个 `cast_instance_id`。 |
| `I-03` | `CastIdentity` | `(session_id, cast_instance_id)`；accepted、reject、terminal、PLAY、STOP 全链路不变。 |
| `I-04` | authoritative gameplay event | `CastSync(CASTING/COMPLETE/INTERRUPT)`；只有它能建立或终结 authoritative active cast。 |
| `I-05` | attempt feedback | `CastSync(IDLE + Reject*)`；只描述某次 attempt 被拒，不能冒充 active cast 终态。 |
| `I-06` | advisory AV event | `CastPlayAnim` / `CastStopAnim`；只操作同 identity 的 AV ownership，不决定 gameplay 生命周期。 |
| `I-07` | session gate | client 对 caster 保存的 generation、session、floor、exhaustion 与 advertised active 边界。 |
| `I-08` | `AH` / `TH` | attempt high-water / authoritative terminal high-water；`TH` 只能由 COMPLETE/INTERRUPT 推进。 |
| `I-09` | gameplay tombstone | authoritative terminal identity 的有界记录；只由 COMPLETE/INTERRUPT 写入。 |
| `I-10` | AV owner / AV tombstone | 当前动画 token 的 identity，以及阻止迟到 PLAY 复活的独立有界标记；不得复用 `TH` 或 gameplay tombstone。 |
| `I-11` | player authorization | 玩家 `KnownTechniques` 中 learned 且 active；与某 ID 存在于 global registration 是两件事。 |
| `I-12` | pipeline boundary | real producer → transport → bridge → router → reducer → AV owner → shipped asset pack。fixture 直调任何中段都不等于生产可达。 |

### 1.2 不可变式

- `INV-01`：**STOP 永远不拥有 terminal state。** STOP 不得推进 `TH`、不得写 gameplay tombstone、不得写/消费 `CastOutcomeV1`、不得清 authoritative active/reserved；它只可停止同 identity 的 AV owner并写 AV tombstone。
- `INV-02`：只有 `CastSync(COMPLETE|INTERRUPT)` 能终结 authoritative cast；STOP 先到也不能让后续 terminal 变成被吞掉的幂等空操作。
- `INV-03`：attempt feedback、authoritative state、AV ownership 三个分区独立；B reject 不清 active A。若产品语义要求取消 A，producer 必须先为 A 发 matching INTERRUPT（及 advisory STOP），再发 B reject。
- `INV-04`：所有 lifecycle、ordering、terminal、floor 与 exhaustion 场景必须逐步表示为 §3 的 `reduce(state, message)` trace；场景不得覆盖 reducer 行。
- `INV-05`：TypeBox source 拥有 shape 与 validation semantics；protobuf/Rust/Java、JSON Schema、dist、samples 是生成或受约束 mirror，不得再声明 TypeBox 为被动镜像。
- `INV-06`：本轨不引入 dual-form compatibility layer；旧、新 cast wire 不并行成为两个 canonical 入口。
- `INV-07`：本轨不改变扣费、释放或账本语义；P1/P2 只消费 R5 接口，任何 resolver 迁移不得顺手直写 qi。
- `INV-08`：每招独立可辨的 animation、VFX、SFX、HUD、icon 是同一 registration 交付物；纯 NPC binding 的豁免必须由类型表达。
- `INV-09`：FPV 手臂动画与 signature 音频资产不由 R9 接管；R9 只迁移共享 cast identity、store 和 juice token。
- `INV-10`：任何已合入中间态都不得丢 live cast traffic，也不得让同一 AV phase 双发；receiver removal 必须服从 §5 原子 cutover invariant。

---

## 2. Canonical TypeBox-first protocol spec（唯一 schema authority）

本节只定义“消息是什么”。阶段、文件 owner 与 merge 顺序只见 §5；reducer 行为只见 §3。

### 2.1 生成方向

`P-01` 是唯一 schema authority：

```text
TypeBox source (shape + validation + discriminants)
  ├─ JSON Schema / schema dist / positive+negative samples
  ├─ protobuf declarations with pinned field numbers (constrained mirror)
  │    └─ generated Rust + Java bindings
  ├─ Rust domain DTO + serde/conversion (constrained mirror)
  └─ Java bridge DTO/normalization (constrained mirror)
```

protobuf field number 与 oneof number 仍须显式稳定 pin，但不能改变 `P-02..P-11` 的字段、合法组合或 validation semantics。任一 mirror 无法表达 canonical spec 时先改本节并完成 breaking decision，禁止在 converter 中私设例外。

### 2.2 规范行

| ID | canonical contract |
|---|---|
| `P-01` | `agent/packages/schema/src/server-data.ts` 与 `vfx-event.ts` 中的 cast TypeBox declarations 是 source of truth；package dist、JSON Schema、proto/Rust/Java 与 samples 必须由同一版本对拍。 |
| `P-02` | `CanonicalUint64String` 是 `1..=u64::MAX` 的无符号十进制字符串；拒绝 JSON number、0、符号、空串、前导零、非数字和 overflow。Rust/protobuf 内部为 `u64/uint64`，TypeBox/JSON/Java bridge 保留 canonical string 或无损无符号解析。 |
| `P-03` | `CastIdentity { session_id: UUID, cast_instance_id: CanonicalUint64String }`；字段 required、拒绝 unknown field。 |
| `P-04` | `CastSessionBegin { target_player: UUID, target_entity_id: int32, session_id: UUID, session_generation: CanonicalUint64String, allocator_exhausted: boolean, active_cast_instance_id?: CanonicalUint64String, minimum_cast_instance_id?: CanonicalUint64String }`。 |
| `P-05` | BEGIN 合法形状恰为四种：open/no-active=`false,None,Some(next)`；open/active=`false,Some(a),Some(a)`；exhausted/no-active=`true,None,None`；exhausted/active=`true,Some(a),Some(a)`。active/floor 必须非零且相等。 |
| `P-06` | `CastSourceV1 = QUICK_SLOT | SKILL_BAR | DEDICATED`。QuickSlot item cast 的 `skill_id=null`；SkillBar/Dedicated 的 `skill_id` required non-empty。unknown source fail-closed，无 fallback。 |
| `P-07` | `CastTargetRef = entity { entity_uuid: UUID } | block { dimension_id: non-empty string, x:i32, y:i32, z:i32 }`；target 整体 optional；空 wrapper、双 arm、unknown arm、非法坐标类型拒绝。 |
| `P-08` | `CastSync { identity, source, skill_id, target?, phase, slot, duration_ms, started_at_ms, outcome }`。`phase = IDLE | CASTING | COMPLETE | INTERRUPT`；phase/outcome 合法组合由本行固定：IDLE 只配 Reject*，CASTING 只配 NONE，COMPLETE 只配 COMPLETED，INTERRUPT 只配 interrupt outcome。 |
| `P-09` | `CastOutcomeV1` 完整集合固定为 `NONE, COMPLETED, INTERRUPT_MOVEMENT, INTERRUPT_CONTAM, INTERRUPT_CONTROL, USER_CANCEL, DEATH, MERIDIAN_GATED, REJECT_QI_INSUFFICIENT, REJECT_ON_COOLDOWN, REJECT_INVALID_TARGET, REJECT_IN_RECOVERY, REJECT_REALM_TOO_LOW, REJECT_NO_WEAPON, REJECT_TECHNIQUE_INACTIVE, REJECT_RACE_MISMATCH, REJECT_SKILL_CONFIG_INVALID`。每个 variant 在 TypeBox/proto/Rust/Java 各恰有一个同义映射；unknown value fail-closed。 |
| `P-10` | `CastPlayAnim { identity, animation_id }` 与 `CastStopAnim { identity, animation_id }` 都 required 完整 identity；非 cast 通用动画不得伪造 identity 进入此通道。 |
| `P-11` | `ServerDataV1` 有独立 `cast_session_begin`、`cast_sync`、`vfx_event.cast_play_anim`、`vfx_event.cast_stop_anim` arms；四者统一走 `bong:server_data`。 |
| `P-12` | `target_entity_id` 是 recipient 当前 tracking epoch 的 Minecraft protocol entity ID，不是 ECS Entity bits；client 仅在它当前解析到 `target_player` 时安装 BEGIN。 |
| `P-13` | generation 进程级单调非零；同 session 重发复用。connection 内 identity 从 1 单调分配；发出 `u64::MAX` 后 exhausted，后续 admission 无 identity、gameplay、AV 或 reject 副作用。 |
| `P-14` | server 必须先向 recipient 发合法 BEGIN，才可发送同 session 的其余 cast payload。observer 重进 tracking 时 BEGIN 携权威 floor/active/exhaustion 快照。 |
| `P-15` | unknown arm、unknown field、malformed UUID、非法 canonical uint64、非法 phase/outcome/source/target 组合均 fail-closed，且不得 coerce。 |

### 2.3 必交 wire samples

每个 `P-02..P-11` 都有正、反 sample；`P-09` 的 17 个 outcome 逐 variant 正向 roundtrip，并逐 variant 以 unknown/错拼/错 phase 负向 pin。数值边界至少含 `1`、`2^53-1`、`2^53`、`u64::MAX` 和相邻大值不折叠。BEGIN 四形状各一份正 sample，所有非法组合各一份负 sample。执行 schema build 后再运行 package tests，禁止只改 source 不重建 dist。

---

## 3. ONE normative reducer（唯一状态机）

### 3.1 状态与返回值

唯一入口：

```text
reduce(state, message) -> { next_state, disposition, side_effects }
```

`state` 由四个互不复用的分区组成：

```text
gate      = { session_id, generation, floor?, exhausted, reserved_active? }
attempt   = { AH, latest_disposition?, reject_feedback? }
gameplay  = { active_identity?, TH, terminal_tombstones[<=256], outcome_by_tombstone }
av         = { owner?: (identity, animation_id, token), av_tombstones[<=256] }
```

七态只是上述字段的派生视图，不是第二状态机：`U` 无 gate；`O` open/no active；`R` open/reserved；`A` open/active；`X` exhausted/no active；`XR` exhausted/reserved；`XA` exhausted/active。`disposition = ACC | ACC_IDEM | IGN | SUP`。`IGN` 必须让四分区 byte-for-byte 不变。

### 3.2 规范 transition rows

条件按表从上到下首个匹配；所有非 BEGIN message 必须先通过 `P-03/P-12/P-14/P-15`、session/generation/floor gate。reserved/active identity 即使低于后来 reject 推高的 `AH` 仍可完成自己的 authoritative lifecycle。

| ID | message / guard | transition 与唯一副作用 |
|---|---|---|
| `R-01` | 合法 BEGIN，首个或 generation 更高 | `ACC/SUP`；停止旧 AV token，原子替换四分区；按 `P-05` 安装 `O/R/X/XR`。有 advertised active `a` 时 `reserved=a, AH=a, TH=0`；无 active 时 `AH=TH=0`。 |
| `R-02` | 合法 BEGIN，generation/session 相等 | 仅 exhaustion 可单向 `false→true`、floor 可单调提高且不得排除 reserved/active；advertised active 不得增删/改写。合法为 `ACC_IDEM`，否则 `R-03`。 |
| `R-03` | 旧 generation、等 generation 不同 session、BEGIN 非法/回退 | `IGN`。 |
| `R-04` | 新 Reject attempt，`n>AH` 且未 terminal | `ACC`；`AH=n`、latest=`REJECTED`、替换 feedback。保留不相同的 reserved/active/AV owner；`n=max` 只令 gate exhausted，不终结其它 identity。 |
| `R-05` | 同 identity、同 outcome 的 Reject replay | `ACC_IDEM`；不重复 HUD。其它 `n<=AH` reject 为 `R-14`。 |
| `R-06` | CASTING 命中 reserved，或 `n>AH/TH` 的新 accepted attempt | `ACC/SUP`；清较旧 feedback，置 `AH=n/latest=CASTING`，建立 active；若 supersede 旧 active，仅停止旧 identity AV token并为旧 identity 写 AV tombstone。 |
| `R-07` | CASTING 命中同 active 且 latest=`CASTING` | `ACC_IDEM`；不得重复 gameplay/AV。rejected/terminal identity 的 CASTING 为 `R-14`。 |
| `R-08` | COMPLETE/INTERRUPT，identity 未 terminal、未以同 identity reject，且通过 gate | `ACC`；这是唯一 terminal row：`TH=max(TH,n)`、写 gameplay tombstone+outcome、latest 在 `n>=AH` 时置 `TERMINAL`、清 matching reserved/active、停 matching AV owner并写 AV tombstone。迟到旧 terminal 不得改写较新 attempt feedback/disposition。 |
| `R-09` | COMPLETE/INTERRUPT 命中 gameplay tombstone | `ACC_IDEM`；不重复 outcome/HUD/release/complete/interrupt/token。冲突 outcome fail-closed 为 `R-14`。 |
| `R-10` | PLAY 命中 active identity、无 gameplay/AV tombstone | 首次 `ACC` 并武装 owner/token；完全相同 replay 为 `ACC_IDEM`。PLAY 不建立 active，不推进 AH/TH。 |
| `R-11` | STOP 命中当前 AV owner identity | `ACC`；只停 token、清 owner、写 AV tombstone。**gameplay、attempt、TH、gameplay tombstone、outcome 全不变。** |
| `R-12` | STOP 通过 gate但非 owner，且 identity 尚无 AV tombstone | `ACC`；只写 AV tombstone，允许 terminal-before-PLAY/STOP-before-terminal 的迟到 PLAY no-op。不得分类 attempt，后续 `R-08` 仍须完整生效。 |
| `R-13` | STOP 命中 AV tombstone | `ACC_IDEM`；四分区除既有 AV tombstone外不变。 |
| `R-14` | unknown/malformed/session mismatch/below floor/forbidden combination/stale message | `IGN`。 |
| `R-15` | tracking unload / disconnect lifecycle input | caster eviction 原子清四分区；这是 lifecycle teardown，不是 cast terminal，不生成 outcome。 |
| `R-16` | 容量维护 | gameplay tombstone 与 AV tombstone 各自最多 256；驱逐不得降低 `TH/AH`，故被驱逐 identity 的迟到 PLAY/CASTING 仍由 high-water/gate 拒绝。 |

### 3.3 规范 traces（所有场景只能写成这些 row 的组合）

```text
T-01  U --BEGIN(open,floor=8)[R-01]--> O
      O --CASTING(8)[R-06]--> A --PLAY(8)[R-10]--> A(owner=8)
      A --COMPLETE(8)[R-08]--> O --STOP(8)[R-13]--> O

T-02  A(active=7) --REJECT(8)[R-04]--> A(active=7,feedback=8)
      --COMPLETE(7)[R-08]--> O(feedback=8)

T-03  A(active=8,owner=8) --STOP(8)[R-11]--> A(active=8,av_tombstone=8)
      --INTERRUPT(8)[R-08]--> O(outcome written)

T-04  A(active=8) --INTERRUPT(8)[R-08]--> O(tombstones=8)
      --PLAY(8)[R-14]--> O

T-05  O --STOP(8)[R-12]--> O(av_tombstone=8)
      --COMPLETE(8)[R-08]--> O(gameplay tombstone+outcome=8)

T-06  cast 7 terminal[R-08] --UNLOAD[R-15]--> U
      --BEGIN(S,g,floor=8)[R-01]--> O --PLAY(8)[R-14]--> O
      --CASTING(8)[R-06]--> A --PLAY(8)[R-10]--> A(owner=8)

T-07  A(active=A) --REJECT(max=B)[R-04]--> XA(active=A,feedback=B)
      --PLAY(A)[R-10]--> XA(owner=A) --COMPLETE(A)[R-08]--> X(feedback=B)

T-08  X --STOP(max)[R-12]--> X(av_tombstone=max)
      --INTERRUPT(max)[R-08]--> X(gameplay tombstone+outcome=max)
```

`T-06` 明确禁止旧文档的 `O -> PLAY`；`T-03/T-05/T-08` 明确证明 STOP 不消费 terminal。任何新增 scenario 必须列初态、每条 message、命中的 `R-*` 和终态；若无法逐边命中，本 plan 有错，禁止用 prose 为场景开例外。

---

## 4. Real producer authorization 与 AV contract

### 4.1 全注册集合与玩家可达性普查

集合口径固定为生产 `init_registry()` 与 `TECHNIQUE_DEFINITIONS`，不是文档清单或测试 fixture：

| 技能族 | registry | definitions 命中 | 权威可达性结论 | 五件套现状/本轨动作 |
|---|---:|---:|---|---|
| carrier/anqi v2 | 6 | 6 | 玩家可达 | 已有分散 AV；P3 纳入统一 binding |
| burst_meridian | 4 | 4 | 玩家可达 | 已有分散 AV；P3 纳入统一 binding |
| zhenmai v2 | 5 | 5 | 玩家可达 | AV 存在；`sever_chain` HUD 语义仍错，P3 修 |
| woliu v1/v2/v3 | 11 | 11 | 玩家可达 | 已有分散 AV；P3 纳入统一 binding |
| woliu 虚蚀路径 | 5 | 0 | **玩家定义断链** | 五招 animation 资源也缺失；P3 同时补 definition 与五件套 |
| yidao | 5 | 0 | **玩家定义断链** | resolver 有两段动画及 VFX/audio spec；P3 补权威定义/HUD/icon 后统一注册 |
| dugu v2 | 5 | 0 | **玩家定义断链** | 局部五件套结构存在但正式技能栏/HUD/icon 断链；P3 修 |
| baomai v3 | 6 | 2 | 4 招玩家定义断链 | resolver/event 双源仍在；P2 去重，P3 补 4 条定义 |
| tuike v2 | 3 | 3 | 玩家可达 | `shed` 音频已单源；其余视觉及 `don/transfer_taint` 音频仍双路，P2 收口 |
| sword_basics | 4 | 4 | 玩家可达 | 已有分散 AV；P3 纳入统一 binding |
| cultivation::dugu | 2 | 2 | 玩家可达 | 已有分散 AV；P3 纳入统一 binding |
| dandao | 3 | 0 | **玩家定义断链** | 三招仅局部粒子素材，正式 animation/VFX/SFX/HUD/icon 未闭环；P3 修 |
| sword_path | 5 | 5 | 玩家可达 | 已有独立事件 AV；P3 纳入统一 binding |
| npc-named skills | 3 | 3 | **Player+NPC 双受众**：既在玩家默认 definitions 中，也由 NPC AI 注册调用 | P3 用 `audience=Both` 显式化；玩家侧仍须五件套，NPC caster 使用专属粒子/audio 且明确无玩家骨架动画 |
| morph | 1 | 1 | 玩家可达 | 已有 AV；P3 纳入统一 binding |
| **合计** | **68** | **46** | **22 条 registry-only** | 22 = woliu 虚蚀 5 + yidao 5 + dugu v2 5 + baomai 4 + dandao 3 |

另有 definition-only 三条 `movement.dash`、`shield_block`、`body.guangbo_ticao`，它们走专用 intent/system 而非 `SkillRegistry`。P1 不创建或审计任何 registration 形状；P3 在完整 definition/AV 可验后首次落地 `cast_mode=Dedicated`，并与全部 resolver 一起原子切换到统一 registration，最终启动审计断言每条 player definition 恰有一个 resolver 或 dedicated handler。

本矩阵中的“五件套已有”只表示当前代码能找到对应局部映射/资产，不代表已由机器证明唯一消费。P1 建立新 schema/wire 但不切换生产 registration；P2 清除双源并补齐所有退出终态，同时按本阶段测试矩阵锁住双发与终态；P3 补齐已知资产/定义缺口后，一次性把全部 68 resolver + 3 dedicated 原子迁入统一 registration 并删除旧 canonical 表。P3 的 registry 精确集合测试逐条验证完整 definition 以及 animation、VFX/audio/HUD 的 start/release/complete/interrupt phase binding、icon 均非空且真实存在，缺口数必须为零。

### 4.2 权威 producer / authorization rows

| ID | real entry | authorization gate | source / skill_id | required emissions | live evidence |
|---|---|---|---|---|---|
| `A-01` | `handle_use_quick_slot` / `UseQuickSlotRequestV1` | slot、inventory instance、item cast/target rule；不查 global skill membership 冒充 item ownership | `QUICK_SLOT / null` | accepted CASTING 或瞬发 COMPLETE；所有结束带同 identity terminal；binding 声明的 AV | request-handler e2e + bot official entry |
| `A-02` | `handle_skill_bar_bind` / `SkillBarBindRequestV1` | global registration 存在 **且** 玩家 `KnownTechniques` learned+active | 不发 cast；绑定项保存 canonical skill ID | unauthorized bind 拒绝且不污染 binding | empty/missing/inactive/active `KnownTechniques` live request |
| `A-03` | `handle_skill_bar_cast` / `SkillBarCastRequestV1` | 当前槽绑定、global registration、玩家 learned+active、race/meridian/config/target/cooldown/qi gates | `SKILL_BAR / required` | 每次进入 admission 先分 identity；accepted/terminal 或 `IDLE+Reject*`；AV 仅 accepted | bind→cast 正负 e2e，禁止 pure helper 替代 |
| `A-04` | registration 指定的 official dedicated handler | unique handler ownership；玩家入口仍查 learned+active 与适用 gates | `DEDICATED / required` | 与 `A-03` 同身份/终态契约；不得旁路 reducer | `movement.dash`、`shield_block`、`body.guangbo_ticao` 各 official handler e2e |
| `A-05` | NPC resolver | global registration、NPC AI 自有 gate；不得伪造玩家 KnownTechniques | domain-declared source；不得进入玩家 skillbar | authoritative lifecycle；只消费 Npc/Both visual | NPC resolver integration |
| `A-06` | pre-cast reject producer | 对 mandatory config 缺失/非法逐字段 fail-closed | 保持请求真实 source/skill_id | 只发 `IDLE+Reject*`；不得扣 qi/stamina、写 cooldown/inventory/target、发任何 AV/STOP | active 与 idle 两前态全矩阵 |

`A-02/A-03/A-04` 强制区分 registration membership 与 player authorization。active A 时 attempt B reject 只按 `R-04` 更新 B feedback，A 的 gameplay/AV/token 与后续 `R-08` 能力保持；任何“reject 顺手 reset cast”的实现不合格。

### 4.3 `SkillAvBinding` 冻结

P3 production cutover 的数据形状冻结为：

```rust
enum SkillCastMode { Resolver, Dedicated { handler: DedicatedHandlerId } }
struct SkillRegistration {
    resolver: Option<SkillFn>, audience: SkillAudience, cast_mode: SkillCastMode,
    definition: TechniqueDefinition, av: SkillVisualBinding,
}
enum SkillVisualBinding {
    Player(SkillAvBinding), Npc(NpcVisualBinding),
    Both { player: SkillAvBinding, npc: NpcVisualBinding },
}
struct SkillAvBinding {
    animation: SkillAnimationBinding, vfx: SkillVfxBinding,
    audio: SkillAvPhaseBinding, hud: SkillAvPhaseBinding, icon: SkillIconBinding,
}
struct SkillAvPhaseBinding {
    start: Option<&'static str>, release: Option<&'static str>,
    complete: Option<&'static str>, interrupt: Option<&'static str>,
}
struct SkillAnimationBinding { start: &'static str, release: Option<&'static str>, looping: bool }
enum SkillIconBinding {
    Asset(&'static str),
    ExplicitPlaceholder { asset: &'static str, blocker: &'static str },
}
```

`TechniqueDefinition` 的 required fields 固定为 `id/display_name/grade/description/required_realm/required_meridians/required_race/qi_cost/stamina_cost/cast_ticks/cooldown_ticks/range/category`。

`SkillRegistration.definition` 是所有 gameplay/技能栏元数据的唯一 owner；现有 `TechniqueDefinition.icon_texture` 删除，图标只来自 `av.icon`。`TECHNIQUE_DEFINITIONS`、`TECHNIQUE_IDS` 和 skillbar snapshot 均由 registration 投影生成，不再保留手写 canonical 数组。`cast_mode=Dedicated` 使用 `resolver=None`，但仍须携完整 definition、官方 handler 标识与玩家 AV binding。`SkillAvBinding.vfx/audio/hud` 的每个 phase 槽位明确声明该效果是否在 `start`、权威 `release`、`complete` 或 `interrupt` 发射；空槽表示该阶段不适用，消费者只读 binding，不得在 resolver、router 或 skill-id 特判表里补阶段语义。`release` 与 `complete` 仅 COMPLETE 可消费，`interrupt` 仅 INTERRUPT 可消费；任何 INTERRUPT 必须跳过 release/complete。

约束：

1. `SkillRegistry::register` 改收完整 `SkillRegistration`；同一 `skill_id` 或同一 cast 的多个 emit owner 均启动 fail-fast。不同玩家技能不得复用任一完整 AV 通道：任意两个不同的 Player/Both 技能，其 animation、VFX、SFX、HUD feedback、icon 五个绑定值必须逐字段全部不同；单个底层素材只有在不落入玩家技能绑定通道时才可复用（否则无法辨招）。resolver 只发送“技能已接受/命中/结算”领域事件，不得直发绑定中的 animation/VFX/audio，唯一 AV consumer 按 registration 发射；每个 `SkillAvPhaseBinding` 槽位只能由该唯一 consumer 按其声明 phase 发射。
2. `definition.id` 必须等于 registration key；resolver 模式要求 `resolver=Some`，Dedicated 模式要求 `resolver=None` 且恰有一个经启动审计确认的官方 handler。所有 definition 字段均来自 registration；旧 `TECHNIQUE_DEFINITIONS`/`TECHNIQUE_IDS` 只允许成为派生只读视图，不得再手写条目。
3. 玩家受众 (`audience=Player|Both`) 五字段全部必填并验证真实 client 资源/recipe/handler；纯 NPC 受众显式免除 HUD/icon，animation 不适用时也必须用明确 `NpcVisual` 类型，禁止空串冒充。
4. 占位只允许 `SkillIconBinding::ExplicitPlaceholder`，且必须携 `[BLOCKED: 需 /gen-image ...]` blocker、引用真实占位资产并出现在启动汇总；animation/VFX/audio/HUD 不允许 placeholder 或静默 fallback。P3 归零所有 placeholder 后才可完成。
5. 多阶段招式用 `start + optional release + looping`；施法专用 `CastPlayAnim` 与 `CastStopAnim` 都携完整 `CastIdentity`，STOP 只可停止客户端记录为同一 identity 的 `start` 循环层，release 只在权威完成时播。禁止另建 `looping_cast_anim_id(skill_id)` 特判表。
6. icon 单一真相源迁入 registration 后，由它派生 technique/skillbar/client icon snapshot；不得继续维护同一 skill 的第二份路径字面量。

| ID | registration invariant |
|---|---|
| `A-07` | `definition.id == key`；Resolver 有 resolver；Dedicated 无 resolver且恰有一个 official handler；definition/skillbar 只读投影。 |
| `A-08` | Player/Both 五件套真实存在且逐通道可辨；Npc 用显式类型表达 HUD/icon/player animation 不适用。 |
| `A-09` | phase slot 是唯一 emit 声明；COMPLETE 消费 release/complete，INTERRUPT 只消费 interrupt。 |
| `A-10` | 仅 icon 允许显式 blocker placeholder，P3 完成前归零；其它四件套不允许 placeholder。 |
| `A-11` | 每个 `(CastIdentity,av_kind,phase)` 恰有一个 registration consumer owner。 |
| `A-12` | looping owner identity-aware；STOP/terminal 只走 `R-08/R-11..R-13`，无 skill-id stop 特判表。 |
| `A-13` | VFX 表示必须携 capability tier；Iris 是 client 可选能力，server 不假设渲染能力。 |

#### Iris shader capability（可选能力，不是 server 前置）

`SkillVfxBinding { phases: SkillAvPhaseBinding, capability: VfxCapabilityTier }`；tier 为 `Vanilla` 或 `ShaderOptional { iris_effect_id, fallback: VanillaParticle(id) | NoOp }`。client 运行时通过 Fabric Loader 检测 Iris，存在才选 shader；缺失则 fallback/no-op。server 只 emit semantic effect ID/tier；Iris 缺失不影响 gameplay、CastSync、AV ownership 或其它通道。

---

## 5. End-to-end ownership 与原子 cutover DAG

### 5.1 Artifact ownership rows

| ID | artifact / live edge | single owner | phase | completion evidence |
|---|---|---|---|---|
| `C-01` | TypeBox cast source：BEGIN/CastSync/identity/source/target/outcome/PLAY/STOP | R9 | P1 contract | `P-01..P-15` source tests + samples |
| `C-02` | schema dist、JSON Schema、registry 与 generated samples | R9 | P1 contract | package build/test；committed artifact diff |
| `C-03` | protobuf declarations/field numbers/oneofs + generated Rust/Java | R6 P3 shared-wire owner | P1 contract handoff | buf breaking + generated pin |
| `C-04` | Rust domain DTO、session/attempt allocator、real producers | R9 | P1 contract | `A-01..A-06` producer tests |
| `C-05` | `ServerDataEnvelope` arms 与 `proto_convert.rs` | R6 P3 shared-wire owner | cutover unit | four-arm conversion roundtrip |
| `C-06` | `ProtoServerDataBridge` conversions/normalization | R6 P3 shared-wire owner | cutover unit | proto→Java boundary/unknown tests |
| `C-07` | `ServerDataRouter` keys：`cast_session_begin`、`cast_sync`、`vfx_event.cast_play_anim`、`vfx_event.cast_stop_anim` | R6 P3 shared router owner | cutover unit | four-key registration/dispatch/duplicate/unknown tests |
| `C-08` | concrete BEGIN + CastSync consumer → `CastSessionRegistry.reduce` | R9 | cutover unit | real router dispatch to `R-01..R-09/R-14` |
| `C-09` | concrete PLAY + STOP consumer → `AnimationLayerManager` / `CastFovController` | R9 | cutover unit | real router dispatch to `R-10..R-14` |
| `C-10` | `bong:vfx_event` cast producer/receiver removal；all cast emit 改 `bong:server_data` | R6 P3 channel owner | cutover unit | old receiver zero-hit + new live path |
| `C-11` | `CastSessionRegistry`、lifecycle adapter、FPV identity/token | R9（消费 R2 P1 lifecycle API） | P1 cutover | `R-15/R-16` churn + juice identity |
| `C-12` | Baomai/Tuike single owner、全退出 producer、`meditate_sit.json` | R9 | P2 | `A-09/A-11/A-12` pins |
| `C-13` | `SkillRegistration`、68 resolver + 3 dedicated、definitions/AV bindings/assets | R9 | P3 | `A-02..A-13` projection/real-entry tests |
| `C-14` | `client/resourcepack/manifest.json`、built zip SHA-1/size、`server/src/network/resourcepack.rs::DEFAULT_RESOURCE_PACK_MANIFEST` | R9 asset PR | P3 same delivery | build-resourcepack + committed SHA-1/size exact match |
| `C-15` | contract/live/bot/release evidence 与 absorbed-plan archive | R9 | P4 | §6 derived index 全绿 |

BEGIN 与 CastSync 的 production registration 不再遗漏：它们由 `C-07+C-08` 明确拥有；PLAY/STOP 由 `C-07+C-09` 拥有。fixture 直接调用 reducer只能证明 `C-08/C-09` 的局部语义，不能完成 live edge。

### 5.2 DAG 与阶段门

```text
[R5 P1] ─┐
[R6 P2] ─┼─> G-W2 -> C-01/C-02/C-04 + reducer contract
[R2 P1] ─┘                         |
                                     v
                              C-03 handoff frozen
                                     |
                                     v
CUTOVER-MERGE-UNIT = C-05+C-06+C-07+C-08+C-09+C-10+C-11
                                     |
                                     v
                         P2 C-12 -> P3 C-13+C-14 -> P4 C-15
```

| ID | merge invariant |
|---|---|
| `C-INV-01` | contract phase只增加 declarations、generated mirrors、reducer/producer tests；未激活的 arms 不得宣称 production reachable。 |
| `C-INV-02` | production cutover 默认是同一 merge unit：新 channel producer、四 key router、BEGIN/CastSync/PLAY/STOP concrete consumers、bridge/conversion、旧 cast receiver removal 同时生效。 |
| `C-INV-03` | 若跨 track 必须拆 PR，只允许先合入 inert declarations/conversions/registrations；旧 receiver 与旧 producer保持完整。最终 activation PR 必须同一 merge unit切 producer、启用四 consumers并删除旧 receiver。 |
| `C-INV-04` | **禁止 receiver-removed-before-consumer-installed window**；也禁止 producer 双投旧/新 channel造成 AV 双发。每个可合入中间态必须是“完整旧路径”或“完整新路径”，不能是半条路径。 |
| `C-INV-05` | “可开始”只由 G-W2 决定；“可宣称生产可达”必须 CUTOVER-MERGE-UNIT 和 live evidence 全绿。R6 P3 不是 R9 contract start gate，但属于 production reachability gate。 |
| `C-INV-06` | P3 类型、68+3 production registrations、lookup/projection、唯一 AV consumer、旧 canonical tables 删除在同一 merge unit；禁止 test-only 平行 registry。 |
| `C-INV-07` | 新增/修改任何 client asset 的 PR 同时拥有 `C-14`，不得把 manifest/SHA-1/size 留给后续补丁。 |

---

## 6. Derived acceptance index（只引用规范 row IDs）

本节不定义状态或业务规则。测试名称、参数和 oracle 必须从引用的 row IDs 生成；测试失败信息必须打印 trace 中每一步的 row ID。新增规范 row 时 static consistency test 要求本索引有覆盖；新增测试不得复制 transition prose。

| ID | test / evidence | normative refs |
|---|---|---|
| `D-01` | `cast_protocol_shape_cross_stack` | `P-01..P-15` |
| `D-02` | `cast_outcome_all_variants` | `P-08,P-09,P-15,C-01..C-03` |
| `D-03` | `cast_reducer_all_rows_all_states` | `I-04..I-10,INV-01..INV-04,R-01..R-16` |
| `D-04` | `cast_trace_static_consistency` | `INV-04,R-01..R-16,T-01..T-08` |
| `D-05` | `cast_session_lifecycle_churn` | `P-04,P-05,P-12..P-14,R-01..R-03,R-15,R-16,C-11` |
| `D-06` | `cast_real_producer_mapping` | `P-06..P-10,A-01,A-03..A-06,C-04` |
| `D-07` | `cast_known_techniques_bind_cast` | `I-11,A-02..A-04,A-07,C-13` |
| `D-08` | `cast_active_preservation_on_reject` | `INV-03,R-04,R-05,R-08,A-06` |
| `D-09` | `cast_allocator_exhaustion` | `P-02,P-04,P-05,P-13,R-01,R-04,R-08,R-12,R-15` |
| `D-10` | `cast_live_four_arm_dispatch` | `P-11,P-14,C-05..C-11,C-INV-01..C-INV-05` |
| `D-11` | `cast_stop_never_terminal` | `INV-01,INV-02,R-08,R-09,R-11..R-13,T-03,T-05,T-08` |
| `D-12` | `p2_av_single_owner` | `A-09,A-11,A-12,C-12` |
| `D-13` | `p2_terminal_state_matrix` | `I-04,I-06,R-08,R-11..R-15,C-12` |
| `D-14` | `p2_stop_reordering` | `R-06,R-08,R-10..R-14,A-12,C-12` |
| `D-15` | `p2_meditate_animation_pin` | `C-12` |
| `D-16` | `p2_av_phase_binding` | `A-09,A-11,A-12,C-12` |
| `D-17` | `p3_registration_projection` | `A-02..A-13,C-13,C-INV-06` |
| `D-18` | `p3_resourcepack_release` | `C-14,C-INV-07` |
| `D-19` | `p3_iris_capability_fallback` | `A-13,C-13,C-14` |
| `D-20` | `cast_registry_reachability` | `A-01..A-08,C-13` |
| `D-21` | `cast_stop_semantics` | `R-08..R-15,A-12,C-08..C-12` |
| `D-22` | `cast_av_uniqueness` | `A-07..A-13,C-12,C-13` |
| `D-23` | `cast_wire_identity` | `D-01..D-11` |
| `D-24` | `cast_av_phase_regression` | `D-12..D-16` |
| `D-25` | `cast_registration_projection` | `D-07,D-17..D-20` |
| `D-26` | `cast_juice_identity_bridge` | `R-08..R-14,C-05..C-11` |
| `D-27` | runClient 远处读招/HUD/icon/循环停止 + Iris present/absent | `A-08..A-13,C-13,C-14` |

`D-15` 的资产 oracle 原样固定为：用 headless 渲染/姿态断言 `meditate_sit.json` 维持直立 torso（`torso.pitch=0`）、垂目 head pitch 约 `+0.2094395rad`（+12°）及双腿目标盘坐姿态：两腿 pitch 必须落在明确的修复区间 `[-0.698132, 0.0]rad`（[-40°, 0°]），双腿 yaw 保持相反符号且绝对值约 `0.436332rad`（25°），双腿 bend 约 `1.570796rad`（90°）承担折腿；不得再出现当前 `-1.3962634rad`（-80°）过旋。循环动画每个使用轴在 endTick 有同值关键帧，P4 复跑同一完整姿态 oracle。

bot 场景主题保留为：`cast_registry_reachability`、`cast_stop_semantics`、`cast_av_uniqueness`、`cast_wire_identity`、`cast_av_phase_regression`、`cast_registration_projection`、`cast_juice_identity_bridge`。场景文件只引用对应 `D-*`，不得另写 reducer 语义。

---

## 7. 实施 phases（只落地既有规范节点）

### P0 — protocol/reducer/producer/cutover 设计收口 ✅ 2026-08-04

- 已形成 §1 不变量、§2 TypeBox-first authority、§3 唯一 reducer、§4 producer 授权表、§5 cutover DAG、§6 derived index。
- P0 static review 只检查 ID 唯一、引用存在、每个 trace 逐边命中 reducer、STOP 不引用 gameplay terminal side effect。

### P1 — cast contract + 原子 production cutover ⬜

1. **P1 contract**：完成 `C-01..C-04`、`D-01..D-09`；只增加 inert declarations/mirrors/tests，不声称 live reachable。
2. **P1 cutover**：按 `C-INV-01..C-INV-05` 完成 `C-05..C-11`，四 arms 在同一安全 merge unit 可达；通过 `D-10,D-11,D-23,D-26`。
3. 删除 `CastSyncHandler.sourceFor()` 与旧 cast `bong:vfx_event` receiver；不得保留 dual-form compatibility。

### P2 — 双源清除 + 全退出终态 ⬜

- 完成 `C-12`：Baomai/Tuike 余下双源归一；移动、污染、控制、用户取消、Fled、死亡、断线、换维度 producer 对齐 `R-08/R-11..R-15`。
- 完成 `D-12..D-16,D-24`；`dugu.penetrate` 只保留现状防回归 pin。
- 视觉资产 `meditate_sit.json` 按三轮打磨，Round 3 commit 带 `<PROMISE>`。

### P3 — 定义/资产补齐 + registration 原子迁移 ⬜

- 补 22 条 registry-only definitions 与缺失五件套；完成 `C-13,C-14` 和 `C-INV-06,C-INV-07`。
- 原子迁入 68 resolver + 3 dedicated；删除手写 canonical `TECHNIQUE_DEFINITIONS/TECHNIQUE_IDS`，projection 保留只读派生 API。
- 完成 `D-07,D-17..D-20,D-22,D-25`；icon placeholder 归零，Iris present/absent 两路均验收。
- 所有新增 animation/VFX/icon 资产执行 3 轮打磨；icon 走 `/gen-image item`，不能运行时标 blocker 但不得把 P3 标完成。

### P4 — 派生验收、人工回归与归档 ⬜

- 原样复跑 `D-01..D-27`，不得抽样；`scripts/bot/scenarios/` 对应七场景走真实 transport 与 official entries。
- runClient 人工验收远处读招、两层 hotbar 归属、HUD hint/icon、循环停止与 Iris present/absent；不能执行时如实记 blocker，不以单测代替。
- 为本 plan 与 §8 被吸收 plan 填写 Finish Evidence；全部 phase ✅ 后归档。

---

## 8. Salvage、吸收与范围边界

### 8.1 吸收清单第一性原理裁决

| plan | 2026-08-03 裁决 | R9 落点 |
|---|---|---|
| `dugu-v2-technique-definition-gap` | **仍成立**：5 resolver 全部 registry-only | P3 补定义/HUD/icon/回归 |
| `woliu-voidpath-missing-animations` | **仍成立且范围扩大**：五招 animation 缺失，同时 registry-only | P3 补定义与五件套 |
| `dandao-basic-skillbar-bridge` | **仍成立**：三 resolver registry-only；只有局部粒子素材 | P3 完整接入 |
| `dugu-v2-hud-skill-hint` | **仍成立**：局部 `hud_hint` 未进入 skillbar/runtime HUD 契约 | P3 由 binding 下发/渲染 |
| `skillbar-cast-source-drift` | **仍成立**：wire 无 source，client `sourceFor()` 猜测 | P1 权威 source + identity |
| `skillconfig-castsync` | **仍成立**：配置 fail-close 分支无纠正回执 | P1 新 reject outcome |
| `zhenmai-sever-marker-hud` | **仍成立**：client 固定显示“断链增幅”，无 amplification 语义分支 | P3 修 payload/HUD |
| `baomai-v3-av-double-source` | **仍成立**：resolver 直发与 `BaomaiSkillEvent` consumers 并存 | P2 唯一 consumer |
| `dugu-penetrate-av-mismatch` | **已关闭，不再实施**：当前 runtime 已取 `visual_for(Penetrate)` 的针掷/针嘶映射 | P4 只归档并记录现状证据 |
| `meditate-sit-leg-pitch` | **仍成立**：`meditate_sit.json` 双腿 pitch 为 -1.396rad（约 -80°），超过约 40°红线 | P2 调低 pitch、以 bend 承担折腿并 headless 回归 |
| `tribulation-fled-brace-stop` | **仍成立**：Fled 生命周期未形成绑定式权威 STOP | P2 接统一终态 |
| `tuike-v2-duplicate-av` | **部分关闭**：`shed` 主动签名音已改由事件单源；`don/transfer_taint` 音频及三招视觉仍同时有 resolver 直发和事件 consumer | P2 仅修剩余双源，不回退已完成项 |
| `combat-event-juice-runtime-bridge-gap` | **仅部分吸收**：吸收 cast identity/source/phase/target 与施法 juice 所需字段；命中侧 UUID/school/direction/kill 富化仍归原 plan | P1/P4 限定 cast 子域 |

`plan-fpv-cast-av-v1` **不吸收**：它已有实质进度并独立收尾；R9 P1 只在共享 `CastStateStore`/wire identity 处迁移对齐，不接管 FPV 手臂动画、signature 音频资产或其验收。

原吸收表全部已在本节收口；实施与归档以本裁决为准，不以旧 skeleton 行号或旧结论为准。

### 8.2 明确边界

- **worldview/AV 锚点**：每招独立可辨的 animation/VFX/SFX/HUD/icon 是根 `CLAUDE.md` 红线；audio 保持 Pattern A（使用施法时 `cast_center` 快照，不读取消费时实时 `Position`）。
- **qi_physics 锚点**：本轨不改变扣费、释放或账本语义；P1/P2 只消费 R5 接口，任何 resolver 迁移不得顺手直写 qi。
- **不碰**：FPV 手臂动画与 signature 音频资产；combat hit-event 富化；天道 agent runtime；worldview。
- TypeBox cast schema source 属 `C-01` 的有限跨目录交付，但不触碰 tiandao runtime、prompt、arbiter；这不是“被动 mirror”例外。
- definition-only 三个 dedicated 入口与 NPC 双受众只按 `A-04/A-05/A-07/A-08` 处理，不为迁移方便伪装成普通 resolver。

---

## 9. Operational workflow、校验与 rewrite gate

### 9.1 PR 顺序与文件纪律

1. PR-1 contract：`C-01..C-04` + `D-01..D-09`，不激活 production path。
2. PR-2 atomic cutover：`C-05..C-11` + `D-10,D-11,D-23,D-26`，严格服从 `C-INV-01..C-INV-05`。
3. PR-3 P2 single-owner/terminal/meditation：`C-12` + `D-12..D-16,D-24`。
4. PR-4 P3 registration/assets/release：`C-13,C-14` + `D-07,D-17..D-20,D-22,D-25`。
5. PR-5 P4 full evidence/archive：`C-15` + `D-01..D-27`。

前一 PR merge 后才开始后一 PR；每次 fetch 最新 `origin/main` 后验证 DAG gate。跨 owner 文件只能按 §5 owner/handoff 修改，不得复制 converter、bridge、router 或 consumer 绕门。任何 visual asset PR 执行 3 轮打磨与 `<PROMISE>`；资源包同步是同 PR 交付物。

### 9.2 必跑 gate

- server：`cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
- schema source 改动：`cd agent && npm run build -w @bong/schema && npm test -w @bong/schema`
- client：`cd client && ./gradlew test build`
- bot/e2e：对应 `scripts/bot/scenarios/` + `bash scripts/smoke-test-e2e.sh`；headless server 设 `BONG_SKIP_SKIN_PREFETCH=1`。
- resource pack：`bash scripts/build-resourcepack.sh`、manifest test，并对拍 zip 实际 SHA-1/size 与 `C-14` 两处 committed 值。
- UI：runClient 检查 `D-27`；Iris 安装与未安装各一轮。

任何管道命令必须保留真实退出码，不得用 `| tail` 制造假绿。P4 Finish Evidence 逐项记录命令、测试数、commit、跨栈 symbols、人工 blocker。

### 9.3 Rewrite gate（四项必须同时成立）

| outcome | 本文落点 | 机器检查 |
|---|---|---|
| one normative reducer | `INV-04`、`R-01..R-16`、`T-01..T-08` | `D-03,D-04`：所有 scenario 边必须命中一个 reducer row |
| one schema authority | `INV-05`、`P-01..P-15` | `D-01,D-02`：TypeBox-first 全 mirror/all-outcome 对拍 |
| one ownership/cutover DAG | `C-01..C-15`、`C-INV-01..C-INV-07` | `D-10,D-18`：四-arm live reachability + release hash，禁止丢包中间态 |
| one acceptance index | `D-01..D-27` | static lint：acceptance 只引用存在的 `P/R/A/C/I/INV/T` IDs，不复制 transition |

### 9.4 Falsification / rollback clause

本次 rewrite 的首轮正式 review 必须同时满足：

1. **major findings 少于 9 条**；
2. **本次文档 diff byte count 不得大于提交时记录的 rewrite diff byte count，且最终文档小于 66KB**。

任一失败即视为新架构未证明收敛：回滚本 rewrite commit，恢复诊断前版本，不进入 finding-by-finding 增补。禁止通过降级 finding 严重性、删除规范 evidence 或把内容移到未受审文件规避本条。

### 9.5 Finish Evidence 模板

- 落地清单：按 `C-01..C-15` 列真实路径/symbol。
- 关键 commit：每个 phase 的 hash、日期与 atomic merge invariant。
- 测试结果：按 `D-01..D-27` 列命令、数量、失败修复。
- 跨仓库核验：TypeBox/proto/Rust/Java、real producer、四 router keys、resource-pack hash/size。
- 遗留/后续：只列不在 `P/R/A/C` rows 内的范围，不得把未完成 row 写成后续。
