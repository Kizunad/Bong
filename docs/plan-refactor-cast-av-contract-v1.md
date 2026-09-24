# plan-refactor-cast-av-contract-v1 — 施法同步/技能栏/AV 单一事实源契约（重构轨 R9）

> 所属总纲：`docs/plans-skeleton/plan-refactor-master-v1.md`（草案权威）。一句话：以 TypeBox-first 协议、唯一 reducer、真实 producer 授权表和原子 cutover DAG 收敛施法身份、终态与每招 AV 五件套。
>
> 阶段总览：P0 ✅ 2026-08-04；P1 ⬜；P2 ⬜；P3 ⬜；P4 ⬜。

## 现状证据（2026-08-03 P0 复核）

- `SkillRegistry` 当前只保存 `skill_id → SkillFn`，生产初始化共注册 **68** 个 resolver；`TECHNIQUE_IDS` / `TECHNIQUE_DEFINITIONS` 各 **49** 条。两集合交集为 **46**，registry-only **22**，definition-only **3**，说明 resolver、玩家入口、AV 元数据没有共同事实源（`server/src/cultivation/skill_registry.rs:71-122`；`server/src/cultivation/known_techniques.rs:67-166`）。
- server `Casting` 已保存 `source` 与 `skill_id`，但 `CastSyncV1` 只发 `phase/slot/duration_ms/started_at_ms/outcome`；client `CastSyncHandler.sourceFor()` 因此只能从当前快照猜来源并默认 `QUICK_SLOT`（`server/src/combat/components.rs:421-447`；`server/src/schema/combat_hud.rs:97-106`；`client/src/main/java/com/bong/client/network/CastSyncHandler.java:19-51,97-103`）。
- `CastPhaseV1` 已有 `Idle/Casting/Complete/Interrupt`，所以本轨不重复“新增 phase 字段”；真正缺的是稳定 cast 身份、权威来源/技能/目标与所有退出路径的一致终态。循环动画停止仍由 `cast_emit.rs` 的 skill-id 特判表分散维护，而非注册契约。
- AV 元数据已有 `DuguSkillVisual`、`TuikeSkillVisual`、`WoliuSkillVisual`、`YidaoSkillSpec` 等局部结构，字段与消费路径各异；Baomai/Tuike 仍可同时走 resolver 直发与事件 consumer，证明局部映射不能充当全局唯一真相源。
- #1287 的总纲 §1 基线门已由 `origin/main` commit `9931a3a1fdd5b4d6b38f4da2fce43f400e26bf0d`（PR #1287）满足；这只关闭该历史等待项。R9/R6 的跨轨 schema ownership、contract-first start 与 production activation 服从总纲 `docs/plans-skeleton/plan-refactor-master-v1.md` §3/§4.1 及其 adjudication commit `1d7a257ab7d1f72261aa290a8901df1be3e1dc43`（PR #1902）：Wave 0 可先落 R9 canonical cast content/reducer/tests 与 R6 generation contract，Wave 2 才放行对应 production bridge、transport 与 atomic activation。`dugu.penetrate` 当前也已改为 `visual_for(DuguSkillId::Penetrate)` 驱动 runtime animation/audio（`server/src/combat/dugu_v2/skills.rs:392-416`），旧错接结论已经关闭。

---

## 1. 术语、边界与不可变式

### 1.1 术语

| ID | 术语 | 唯一定义 |
|---|---|---|
| `I-01` | `CastSession` | 一个 server transport 连接内的施法命名空间，由 `(session_id, session_generation)` 唯一标识。 |
| `I-02` | `CastAttempt` | 一次进入 admission 的请求，包括 accepted 与所有 pre-cast reject；每次恰分配一个 `cast_instance_id`。 |
| `I-03` | `CastIdentity` | `(session_id, session_generation, cast_instance_id)`；accepted、reject、terminal、PLAY、STOP 全链路携带并按完整三元组匹配，禁止只用 session 或 instance 局部匹配。 |
| `I-04` | authoritative gameplay event | `CastSync(CASTING/COMPLETE/INTERRUPT)`；只有它能建立或终结 authoritative active cast。 |
| `I-05` | attempt feedback | `CastSync(IDLE + Reject* 或 MERIDIAN_GATED)`；只描述某次 attempt 被拒，不能冒充 active cast 终态。 |
| `I-06` | advisory AV event | `CastPlayAnim` / `CastStopAnim`；只操作同 identity 的 AV ownership，不决定 gameplay 生命周期。 |
| `I-07` | session gate | client 对 caster（player 或 NPC）保存的 `(caster,session_id,generation)`、floor、exhaustion 与 advertised active 边界。 |
| `I-08` | `AH` / `TH` | attempt high-water / authoritative terminal high-water；`TH` 只能由 COMPLETE/INTERRUPT 推进。 |
| `I-09` | terminal record | authoritative terminal identity 与 outcome 的有界成对记录；只由 COMPLETE/INTERRUPT 写入，单个 record 同时保存 replay 判定所需的 identity/outcome。 |
| `I-10` | AV owner / AV tombstone | 当前动画 token 的 `(identity, av_binding_key, animation_id)` 与 token，以及阻止迟到 PLAY 复活的独立有界标记；pre-admission STOP marker 属于 attempt 分区，只绑定尚未进入 CASTING 的 reserved identity，matching CASTING 消费后不得再影响该 identity 的 PLAY；`av_supersession_floor` 只记录当前 authoritative active identity 已被 STOP/terminal/supersession 拒绝的 identity，不得复用 `TH` 或 terminal record。 |
| `I-11` | player authorization | 玩家 `KnownTechniques` 中 learned 且 active；与某 ID 存在于 global registration 是两件事。 |
| `I-12` | pipeline boundary | real producer → transport → bridge → router → reducer → AV owner → shipped asset pack。fixture 直调任何中段都不等于生产可达。 |

### 1.2 不可变式

- `INV-01`：**STOP 永远不拥有 terminal state。** STOP 不得推进 `TH`、不得写 terminal record、不得写/消费 `CastOutcomeV1`、不得清 authoritative active/reserved；通常只可停止同 identity 的 AV owner 并写 AV tombstone；若 identity 是尚未进入 CASTING 的 pending reserved attempt，则只能写 attempt-scoped `pending_pre_admission_stop` marker，不能改 AV tombstone/floor。
- `INV-02`：只有 `CastSync(COMPLETE|INTERRUPT)` 能终结 authoritative cast；STOP 先到也不能让后续 terminal 变成被吞掉的幂等空操作。pre-admission STOP 只能标记同 identity 的 pending reserved attempt；matching CASTING 原子消费该 marker，后续合法 PLAY 不得被它抑制。
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
| `P-01` | R9 authors/reviews the cast domain meaning in the canonical TypeBox content (BEGIN/CastSync/identity/source/target/outcome/PLAY/STOP); the repo-wide TypeBox source remains the schema authority. R6 owns the generation pipeline and all generated/constrained mirrors, including package dist, JSON Schema, proto/Rust/Java artifacts and samples; R9 must not duplicate or re-own those artifacts. |
| `P-02` | `CanonicalUint64String` 是 `1..=u64::MAX` 的无符号十进制字符串；拒绝 JSON number、0、符号、空串、前导零、非数字和 overflow。Rust/protobuf 内部为 `u64/uint64`，TypeBox/JSON/Java bridge 保留 canonical string 或无损无符号解析。 |
| `P-03` | `CastIdentity { session_id: UUID, session_generation: CanonicalUint64String, cast_instance_id: CanonicalUint64String }`；三个字段 required、拒绝 unknown field；所有非-BEGIN payload 必须携带并按完整三元组匹配。 |
| `P-04` | `CastSessionBegin { caster: CastCasterRef, target_entity_id: int32, session_id: UUID, session_generation: CanonicalUint64String, allocator_exhausted: boolean, active_cast_instance_id?: CanonicalUint64String, minimum_cast_instance_id?: CanonicalUint64String }`；`CastCasterRef = player { player_uuid: UUID } | npc { npc_uuid: UUID }`，只允许一个 arm。`target_entity_id` 是该 caster 在 recipient 当前 tracking epoch 的 protocol entity ID；player 与 NPC 共用同一 BEGIN/session gate，禁止用 player-only sentinel 表示 NPC。 |
| `P-05` | BEGIN 合法形状恰为四种：open/no-active=`false,None,Some(next)`；open/active=`false,Some(a),Some(a)`；exhausted/no-active=`true,None,None`；exhausted/active=`true,Some(a),Some(a)`。active/floor 必须非零且相等。`exhausted/active` 为 advertised-active wire shape；按 `R-01` 初态唯一为 `XR`，matching `CASTING(a)` 后才为 `XA`；`XA` 仅表示 gameplay-active。 |
| `P-06` | `CastSourceV1 = QUICK_SLOT | SKILL_BAR | DEDICATED`。`CastSync` 另带 required `npc: boolean` source discriminator 与 required `av_binding_key`：accepted `CASTING/COMPLETE/INTERRUPT` 及其 AV payload 中，QUICK_SLOT/SKILL_BAR 必须 `npc=false`；player dedicated 为 `DEDICATED,npc=false`；NPC resolver 为 `DEDICATED,npc=true`，且只能配 `audience=Npc|Both`。QUICK_SLOT 的 accepted `CASTING/COMPLETE/INTERRUPT` payload 保持 `skill_id=null`，但必须携 canonical item-template/cast-variant `av_binding_key`；SKILL_BAR/player DEDICATED/NPC DEDICATED 的 accepted `CASTING/COMPLETE/INTERRUPT` 的 `skill_id` required non-empty，`av_binding_key` 必须等于其 registration 声明的 key。`IDLE` 的 `Reject*`/`MERIDIAN_GATED` 使用唯一的 fail-closed reject envelope：`source=DEDICATED,npc=false,skill_id=null,slot=non_slot{kind=DEDICATED},av_binding_key="cast.reject/<canonical-outcome>"`；`<canonical-outcome>` 只能是 P-09 所列 IDLE rejection outcome 的固定 lower-snake-case 名称，key 不是 gameplay registration、不会进入 AV consumer，且绝不回显缺失/非法的请求 binding。unknown source 或非法 source/npc/binding 组合 fail-closed，无 fallback。
| `P-07` | `CastTargetRef = entity { entity_uuid: UUID } | block { dimension_id: non-empty string, x:i32, y:i32, z:i32 }`；target 整体 optional；空 wrapper、双 arm、unknown arm、非法坐标类型拒绝。 |
| `P-08` | `CastSync { identity, source, npc, skill_id, av_binding_key, target?, phase, slot, duration_ms, started_at_ms, outcome }`；`identity/source/npc/av_binding_key/phase/slot/duration_ms/started_at_ms/outcome` required，`skill_id` 按 source/phase 规则必填或为 null。`slot` 是 source-discriminated required union：`quick_slot { index: 0..=8 }` 仅 QUICK_SLOT 合法；`non_slot { kind: SKILL_BAR | DEDICATED }` 仅对应 source 合法，NPC DEDICATED 使用 `non_slot{kind=DEDICATED}`，不得伪造 F1-F9 或发送 client 丢弃的 sentinel。`phase = IDLE | CASTING | COMPLETE | INTERRUPT`；`P-08` 的 accepted-phase 规则按 source 与 phase 联合判定：`QUICK_SLOT` 的 accepted `CASTING/COMPLETE/INTERRUPT` 保持 `skill_id=null`，其 `av_binding_key` 仍必须是 canonical item-template/cast-variant key；`SKILL_BAR`、player `DEDICATED` 与 NPC `DEDICATED` 的 accepted `CASTING/COMPLETE/INTERRUPT` 才必须使用 registration-owned non-empty `skill_id` 与 registration-owned `av_binding_key`。`IDLE+Reject*`/`IDLE+MERIDIAN_GATED` 唯一使用 protocol-owned reject envelope：`source=DEDICATED,npc=false,skill_id=null,slot=non_slot{kind=DEDICATED},av_binding_key="cast.reject/<canonical-outcome>"`，其中 `<canonical-outcome>` 只能是 P-09 所列 IDLE rejection outcome 的固定 lower-snake-case 名称，key 只能来自固定 outcome allowlist，不回显或猜测缺失/非法请求 binding，且该 envelope 不进入 AV consumer。phase/outcome 合法组合由本行固定：IDLE 只配 `Reject*` 或 `MERIDIAN_GATED`，CASTING 只配 NONE，COMPLETE 只配 COMPLETED，INTERRUPT 只配 interrupt outcome。 |
| `P-09` | `CastOutcomeV1` 完整集合固定为 `NONE, COMPLETED, INTERRUPT_MOVEMENT, INTERRUPT_CONTAM, INTERRUPT_CONTROL, USER_CANCEL, DEATH, MERIDIAN_GATED, REJECT_QI_INSUFFICIENT, REJECT_ON_COOLDOWN, REJECT_INVALID_TARGET, REJECT_IN_RECOVERY, REJECT_REALM_TOO_LOW, REJECT_NO_WEAPON, REJECT_TECHNIQUE_INACTIVE, REJECT_RACE_MISMATCH, REJECT_SKILL_CONFIG_INVALID`。`MERIDIAN_GATED` 只允许与 `IDLE` 配对，语义等同 pre-cast rejection 但保留既有 reason；每个 variant 在 TypeBox/proto/Rust/Java 各恰有一个同义映射；unknown value fail-closed。 |
| `P-10` | `CastPlayAnim { identity, av_binding_key, animation_id }` 与 `CastStopAnim { identity, av_binding_key, animation_id }` 都 required 完整 identity 与同一 canonical `av_binding_key`；该 key 必须等于触发它们的 accepted CastSync binding（registration-owned skill key，或 QUICK_SLOT 的 item-template/cast-variant key），且 PLAY/STOP 只允许 accepted cast，不适用于 reject envelope。非 cast 通用动画不得伪造 identity 或 binding key 进入此通道。 |
| `P-11` | `ServerDataV1` 有独立 `cast_session_begin`、`cast_sync`、`vfx_event.cast_play_anim`、`vfx_event.cast_stop_anim` arms；四者统一走 `bong:server_data`。 |
| `P-12` | `target_entity_id` 是 caster 在 recipient 当前 tracking epoch 的 Minecraft protocol entity ID，不是 ECS Entity bits；client 仅在该 ID 当前解析到 BEGIN 的 `caster`（player 或 NPC）时安装 session。connection/tracking entry 与 observer re-entry 都必须重发该 caster 的权威 BEGIN；其后 CastSync/PLAY/STOP 只接受同一 `(caster,session_id,session_generation)` gate。 |
| `P-13` | generation 进程级单调非零；同 session 重发复用。connection 内 identity 从 1 单调分配；任何被接受的 attempt（Reject、CASTING、COMPLETE 或 INTERRUPT）一旦消费 `u64::MAX`，必须在同一 transition 后将 session 标为 exhausted；后续 admission 无 identity、gameplay、AV 或 reject 副作用。 |
| `P-14` | server 必须在 caster connection/tracking epoch entry 向每个 recipient 发合法 BEGIN，才可发送该 caster session 的其余 cast payload；observer 重进 tracking 时重新发送同一 caster 的权威 floor/active/exhaustion 快照。player 与 NPC caster 均走此 BEGIN → CastSync → PLAY/STOP → reducer/AV consumer 链，fixture 注入 BEGIN 不能代替真实 entry producer。 |
| `P-15` | unknown arm、unknown field、malformed UUID、非法 canonical uint64、非法 phase/outcome/source/target 组合均 fail-closed，且不得 coerce。 |

### 2.3 必交 wire samples

每个 `P-02..P-11` 都有正、反 sample；`P-09` 的 17 个 outcome 逐 variant 正向 roundtrip，并逐 variant 以 unknown/错拼/错 phase 负向 pin。数值边界至少含 `1`、`2^53-1`、`2^53`、`u64::MAX` 和相邻大值不折叠。BEGIN 四形状各一份正 sample，所有非法组合各一份负 sample。`CastCasterRef` 的 player/NPC 两个 arm 各有正 sample，并分别覆盖错误 arm、空 wrapper、双 arm 与 unknown arm。除此之外，P1 schema pin 必须为每个 required field 建立独立 missing-field negative sample：只从一个 otherwise-valid payload 删除被测字段，其余字段和枚举组合保持合法，必须 fail-closed；不得用一个 generic malformed sample 代替字段矩阵。字段矩阵至少逐项覆盖 `CastIdentity.session_id`、`session_generation`、`cast_instance_id`；`CastSessionBegin.caster`、其 player `player_uuid`/NPC `npc_uuid`、`target_entity_id`、`session_id`、`session_generation`、`allocator_exhausted`，以及 active/floor 形状下的 `active_cast_instance_id`、`minimum_cast_instance_id`；`CastTargetRef` 的 entity `entity_uuid` 与 block `dimension_id`、`x`、`y`、`z`；`CastSync.identity`、`source`、`npc`、`av_binding_key`、`phase`、`slot`、`duration_ms`、`started_at_ms`、`outcome`；按 source 规则 required 的 `skill_id`；以及 `CastPlayAnim`/`CastStopAnim` 的 `identity`、`av_binding_key`、`animation_id`。每个 `P-11` arm 还要对其嵌套 payload 的 required fields 各有删除样本；`QUICK_SLOT` 的 `skill_id=null` 与 `quick_slot.index=0/8` 是独立合法 pin，不能被缺失字段样本混淆。该矩阵属于 `D-01` 的 P1 必交验收，不是可选 completeness suggestion。执行 schema build 后再运行 package tests，禁止只改 source 不重建 dist。

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
attempt   = { AH, latest_disposition?, reject_feedback?, rejected_attempts: ordered map<identity, reject_outcome>, pending_identity?: identity, pending_pre_admission_stop?: identity }
gameplay  = { active_identity?, TH, terminal_records[<=256] }
av         = { owner?: (identity, av_binding_key, animation_id, token), av_tombstones: ordered set<(identity, av_binding_key, animation_id)>[<=256], av_supersession_floor?: identity }
```

七态只是上述字段的派生视图，不是第二状态机：`U` 无 gate；`O` open/no active；`R` open/reserved；`A` open/active；`X` exhausted/no active；`XR` exhausted/reserved；`XA` exhausted/active。`XR`/`XA` 的判定只看最终 gate 与 gameplay：`exhausted=true` 且无 gameplay `active_identity` 时为 `X`/`XR`（分别无/有 `reserved_active`），有 gameplay `active_identity` 时唯一为 `XA`；BEGIN 的 advertised active 即使内部先安装 `reserved_active/pending_identity`，也只能派生为 `XR`，matching CASTING 后才派生为 `XA`。`terminal_records` 每项同时保存唯一 `identity` 与其 `outcome`；它是 gameplay terminal tombstone 与 outcome 的唯一有界历史，容量维护必须原子移除整项，`TH/AH` 不因驱逐降低。`rejected_attempts` 保存仍受 `AH` 保护的每个已拒绝 identity 与 outcome；在对应 generation/session 存活期间不得覆盖或驱逐，R-15 或 generation replacement 才清空它。按 identity 去重；R-04 更新 `reject_feedback` 只改变最新 HUD 投影，不删除旧 rejected membership。`pending_identity` 表示已由 BEGIN 保留但尚未收到 CASTING 的 identity；`pending_pre_admission_stop` 最多一个，且只能与相同 identity 的 `pending_identity`/`reserved_active` 共存。进入 `A/X/XA` 后不得保留该 marker；matching CASTING 必须消费它而不生成 AV tombstone 或 floor。`disposition = ACC | ACC_IDEM | IGN | SUP`。`IGN` 必须让四分区 byte-for-byte 不变。

### 3.2 规范 transition rows

条件按表从上到下首个匹配；`R-01..R-15` 是 message row，`R-16` 是每个会新增 terminal record/AV tombstone 的匹配 row 在同一 `reduce` transition 末尾必须执行的容量后置条件，不是独立输入或第二次 dispatch。所有非 BEGIN message 必须先通过 `P-03/P-12/P-14/P-15`、完整 `(session_id,generation,cast_instance_id)` identity、session/generation/floor gate。`av_supersession_floor` 是当前 active identity 的 AV rejection floor：它随该 active 的 terminal、控制 supersession、owner STOP、命中当前 active 的非 owner STOP，或 BEGIN generation replacement 原子写入/清除，不能被 tombstone 容量维护驱逐。reserved/active identity 即使低于后来 reject 推高的 `AH` 仍可完成自己的 authoritative lifecycle。

| ID | message / guard | transition 与唯一副作用 |
|---|---|---|
| `R-01` | 合法 BEGIN，首个或 generation 更高 | `ACC/SUP`；停止旧 AV token，原子替换四分区并清除旧 generation 的 `av_supersession_floor`；按 `P-05` 安装 `O/R/X/XR`：open/no-active→`O`，open/active→`R`，exhausted/no-active→`X`，exhausted/active→`XR`。advertised active `a` 的内部表示仍是 `reserved=a, pending_identity=a, AH=a, TH=0`；它只能派生为 `R` 或 `XR`，matching CASTING 后清 pending 并进入 gameplay active，才派生为 `A` 或 `XA`。无 active 时 `AH=TH=0`。 | |
| `R-02` | 合法 BEGIN，generation/session 相等 | 仅 exhaustion 可单向 `false→true`、floor 可单调提高且不得排除 reserved/active；advertised active 不得增删/改写。合法为 `ACC_IDEM`，否则 `R-03`。 |
| `R-03` | 旧 generation、等 generation 不同 session、BEGIN 非法/回退 | `IGN`。 |
| `R-04` | 新 Reject attempt，`n>AH` 且未 terminal | `ACC`；`AH=n`、latest=`REJECTED`、`reject_feedback` 替换为最新反馈，并将该 identity/outcome 插入 `rejected_attempts`（同 identity 只保留一次）。保留不相同的 reserved/active/AV owner；`n=max` 只令 gate exhausted，不终结其它 identity。 |
| `R-05` | 同 identity、同 outcome 的 Reject replay | `ACC_IDEM`；不重复 HUD。其它 `n<=AH` reject 为 `R-14`。若 identity 已在 `rejected_attempts` 但 outcome 冲突，直接 `R-14`。 |
| `R-06` | CASTING 命中 reserved，或 `n>AH/TH` 的新 accepted attempt | `ACC/SUP`；matching `pending_identity/reserved_active` 的 CASTING 先消费并清除同 identity 的 `pending_pre_admission_stop`，该 marker 不得转成 AV tombstone/floor，因而后续合法 PLAY 仍可走 `R-10`。若存在较旧 authoritative active，先在同一 reducer transition 中为旧 identity 产生权威 `INTERRUPT(INTERRUPT_CONTROL)`，推进 `TH`、写旧 identity terminal record、清旧 active/reserved，并发送 matching advisory STOP、写旧 identity 的 AV tombstone/floor；随后清较旧 feedback，清 `pending_identity`，置 `AH=n/latest=CASTING`，建立新 active 并清除仅属于旧 identity 的 floor。旧 identity 的 terminal side effects 与新 identity admission 必须原子完成，不能只写 AV tombstone。任何被本行接受的 `n=u64::MAX` 都在同一 transition 原子设置 `gate.exhausted=true`；新 gameplay active 仍在时派生为 `XA`，否则派生为 `X`，且后续 admission 不得再产生 identity、gameplay、AV 或 reject 副作用。 |
| `R-07` | CASTING 命中同 active 且 latest=`CASTING` | `ACC_IDEM`；不得重复 gameplay/AV。rejected/terminal identity 的 CASTING 为 `R-14`。 |
| `R-08` | COMPLETE/INTERRUPT；若 identity 已有 terminal record 则走 `R-09`；若 identity 是当前 active/reserved，则即使 `n<=TH` 也必须先完成该 active 的 authoritative terminal；若 identity 已由 `R-04` 记录为 rejected，则直接 `R-14`，不得进入 supersession；若 identity 不同于当前 active 且 `n>active_instance_id`，仅在未 rejected 时执行 supersession cascade；仅在上述两者都不成立且 identity 未在 terminal records 中但 `n<=TH`（说明该 terminal record 已被 `R-16` 驱逐）时，才 `ACC_IDEM`；否则 identity 未 terminal、未以同 identity reject 且通过 gate | 新 terminal 才 `ACC`。普通 terminal 写 identity+outcome 的 terminal record、`TH=max(TH,n)`，并同步 `AH=max(AH,n)`；若 `n` 不低于 terminal 前的 `AH`，latest 置 `TERMINAL`，从而 delayed reject 不得覆盖该 feedback；清 matching reserved/active/pending identity 与 matching pending marker；若它是当前 active/reserved，则停 matching AV owner并写该 identity 的 AV tombstone/floor。supersession cascade 必须在同一 reducer transition 中先为旧 active `a` 写权威 `INTERRUPT(INTERRUPT_CONTROL)`、推进 `TH` 与 `AH`、写 `a` 的 terminal record、清旧 active/reserved、停旧 AV owner并写其 AV tombstone/floor，再接受 `n` 的 terminal、写其 terminal record 并同步推进 `AH/TH`，最后清除仅属于 `a` 的 floor；两组 terminal side effects 原子完成，最终不得留下旧 active。任何被本行接受的 `n=u64::MAX` 都在同一 transition 原子设置 `gate.exhausted=true`；若 terminal 清掉最后的 gameplay active/reserved，派生为 `X`，若仍有 gameplay active 则派生为 `XA`，后续 admission 无 identity、gameplay、AV 或 reject 副作用。迟到旧 terminal、尤其是已被 `R-04` 拒绝的 attempt，不得改写较新 attempt feedback/disposition 或 supersede active；被驱逐 terminal record 的 replay 只能幂等 no-op，绝不能重新执行 outcome/HUD/release/complete/interrupt。 |
| `R-09` | COMPLETE/INTERRUPT 命中 terminal record | `ACC_IDEM`；同 identity、同 outcome 不重复 outcome/HUD/release/complete/interrupt/token；冲突 outcome fail-closed 为 `R-14`。被 `R-16` 驱逐的 record 不在本行命中，由 `R-08` 依据 `TH` 处理为无副作用幂等 replay。 |
| `R-10` | PLAY 命中 active identity 与相同 `av_binding_key`、无 terminal record、无 AV tombstone，且 identity 严格高于 `av_supersession_floor`（若存在） | 首次 `ACC` 并以 `(identity,av_binding_key,animation_id)` 武装 owner/token；完全相同 replay 为 `ACC_IDEM`。若已有 owner 且 identity/binding 相同但 `animation_id` 不同，或 identity/binding 已命中拒绝 floor/tombstone，走 `R-14` 且不得替换 owner。PLAY 不建立 active，不推进 AH/TH。binding key 不匹配、低于或等于 AV floor 的迟到 PLAY 为 `R-14`，即使对应 AV tombstone 已被驱逐也不得复活。 |
| `R-11` | STOP 命中当前 AV owner identity 与相同 `av_binding_key` | `ACC`；只停同一 `(identity,av_binding_key,animation_id)` token、清 owner、写 AV tombstone，并令 `av_supersession_floor=max(floor,identity)`；**gameplay、attempt、TH、terminal record、outcome 全不变。** STOP 命中 identity/binding 但 `animation_id` 与当前 owner 冲突时走 `R-14`，不得清 owner 或写 tombstone。 |
| `R-12` | STOP 通过 gate但非 owner，且 identity/`av_binding_key` 尚无 AV tombstone | matching `pending_identity == reserved_active == identity` 且尚未收到 CASTING 时，`ACC`；只写 attempt-scoped `pending_pre_admission_stop=identity`，重复 marker 为 `ACC_IDEM`，不写 AV tombstone/floor、不推进 `AH/TH`，也不清 authoritative active/reserved；后续 matching `R-06` CASTING 必须消费 marker，合法 PLAY 仍走 `R-10`。其它 identity 才按普通非 owner STOP `ACC`；只写完整 `(identity,av_binding_key,animation_id)` AV tombstone，并在同一 R-12 transition 的容量后置条件中执行 R-16 的有界 eviction；仅当 STOP identity 等于当前 authoritative active identity 时，才令 `av_supersession_floor=identity`，否则不改变当前 active 的 floor。这样无关 identity 的 bounded tombstone churn 不会阻止当前 active 的 PLAY；terminal-before-PLAY/STOP-before-terminal 的迟到 PLAY 仍由 matching tombstone 或 floor 拒绝。不得分类 attempt，后续 `R-08` 仍须完整生效。 |
| `R-13` | STOP 命中相同 `(identity,av_binding_key,animation_id)` 的 AV tombstone | `ACC_IDEM`；四分区除既有 AV tombstone外不变。 |
| `R-14` | unknown/malformed/session mismatch/below floor/forbidden combination/stale message | `IGN`。 |
| `R-15` | tracking unload / disconnect lifecycle input | caster eviction 原子清四分区；这是 lifecycle teardown，不是 cast terminal，不生成 outcome。 |
| `R-16` | 容量维护后置条件（不是独立 message，也不参与首个匹配 row） | 每个会新增 terminal record 或 AV tombstone 的匹配 row 在同一 `reduce` transition 末尾执行本后置条件：terminal records 与 AV tombstones 各自最多 256；驱逐 terminal record 必须把 identity 与 outcome 原子整项删除，不得降低 `TH/AH`，并且 `R-08` 以 `n<=TH` 将任何已驱逐 terminal replay 分类为 `ACC_IDEM` no-op；驱逐 AV tombstone 时只移除 tombstone entry，**不得移除或降低 `av_supersession_floor`**，`R-10` 仍因 floor gate 拒绝对应迟到 PLAY。容量后置条件不得写入或清除 `av_supersession_floor`；floor 的唯一写入/清除来源是 matching active 的 terminal、控制 supersession、owner STOP、命中当前 active 的非 owner STOP，或 BEGIN generation replacement 的原子 transition。`R-16` 不接受单独的 `EVICT` 输入，所有 trace 只能把它写成触发 row 的同一 transition 后置条件。 |

### 3.3 规范 traces（所有场景只能写成这些 row 的组合）

```text
T-01  U --BEGIN(open,floor=8)[R-01]--> O
      O --CASTING(8)[R-06]--> A --PLAY(8)[R-10]--> A(owner=8)
      A --COMPLETE(8)[R-08]--> O --STOP(8)[R-13]--> O

T-02  A(active=7) --REJECT(8)[R-04]--> A(active=7,feedback=8)
      --COMPLETE(7)[R-08]--> O(feedback=8)

T-03  A(active=8,owner=8) --STOP(8)[R-11]--> A(active=8,av_tombstone=8)
      --INTERRUPT(8)[R-08]--> O(terminal record written)

T-04  A(active=8) --INTERRUPT(8)[R-08]--> O(terminal_records=8)
      --PLAY(8)[R-14]--> O

T-05  O --STOP(8)[R-12]--> O(av_tombstone=8)
      --COMPLETE(8)[R-08]--> O(terminal_record={8:outcome}, AV tombstone=8)

T-06  cast 7 terminal[R-08] --UNLOAD[R-15]--> U
      --BEGIN(S,g,floor=8)[R-01]--> O --PLAY(8)[R-14]--> O
      --CASTING(8)[R-06]--> A --PLAY(8)[R-10]--> A(owner=8)

T-07  A(active=A) --REJECT(max=B)[R-04]--> XA(active=A,feedback=B)
      --PLAY(A)[R-10]--> XA(owner=A) --COMPLETE(A)[R-08]--> X(feedback=B)

T-08  X --STOP(max)[R-12]--> X(av_tombstone=max)
      --INTERRUPT(max)[R-14]--> X(av_tombstone=max)
      the exhaustion-causing reject already owns identity max; the late terminal is rejected and creates no terminal record.

T-09  A(active=7,owner=7,AH=7,TH=0) --CASTING(8)[R-06]-->
      A(active=8,AH=8,TH=7,terminal_record=7,av_tombstone=7)
      side_effects = INTERRUPT_CONTROL(7) + matching STOP(7); identity 7 is terminal before 8 becomes active

T-10  A(active=257,AH=257,TH=256,terminal_records full,oldest identity=1) --COMPLETE(257)[R-08]--> O(TH=257,terminal_records full,oldest identity=2)
      the R-08 transition adds terminal 257 and its mandatory R-16 postcondition evicts terminal record 1;
      --COMPLETE(1)[R-08]--> O(ACC_IDEM,no side effects)

T-11  BEGIN(S,generation=1,active=8)[R-01] --COMPLETE(S,generation=1,8)[R-08]--> O
      --BEGIN(S,generation=2,active=8)[R-01]--> R(generation=2,reserved=8)
      --COMPLETE(S,generation=1,8)[R-14]--> R(generation=2,reserved=8)
      --COMPLETE(S,generation=2,8)[R-08]--> O
      old-generation terminal cannot match the generation-2 reserved identity despite reusing session and instance IDs.

T-12  A(active=1,owner=1) --STOP(1)[R-11]--> A(active=1,av_tombstone=1,av_floor=1)
      --STOP(2..256)[R-12]--> A(active=1,av_tombstones=1..256,av_floor=1)
      --STOP(257)[R-12]--> A(active=1,av_tombstones=2..257,av_floor=1)
      the single R-12 transition appends tombstone 257 and, as its mandatory capacity postcondition, evicts oldest tombstone 1; R-16 is the capacity invariant, not a second matchable edge.
      --PLAY(1)[R-14]--> A(active=1,no owner)
      floor remains after eviction and protects delayed PLAY.

T-13  A(active=7,AH=7) --COMPLETE(8)[R-08]--> O(TH=8,AH=8,terminal_records={7:INTERRUPT_CONTROL,8:COMPLETED},no active)
      side_effects = INTERRUPT_CONTROL(7) + matching STOP(7) + COMPLETED(8);
      --REJECT(8)[R-14]--> O(feedback unchanged)
      the newer instant terminal atomically supersedes and closes older active 7; AH=8 makes the delayed reject stale instead of fresh feedback.

T-14  U --BEGIN(S,generation=1,active=9)[R-01]-->
      R(reserved=9,pending_identity=9)
      --STOP(9)[R-12]-->
      R(reserved=9,pending_identity=9,pending_pre_admission_stop=9,no av_tombstone/floor)
      --CASTING(9)[R-06]-->
      A(active=9,no pending_pre_admission_stop,no av_tombstone/floor)
      --PLAY(9)[R-10]--> A(active=9,owner=9)
      the pre-admission STOP is consumed by the pending attempt and can never suppress the later valid PLAY.

T-15  A(active=7,AH=7) --REJECT(8)[R-04]--> A(active=7,AH=8,feedback=8,rejected_attempts={8})
      --REJECT(9)[R-04]--> A(active=7,AH=9,feedback=9,rejected_attempts={8,9})
      --COMPLETE(8)[R-08/R-14]--> A(active=7,AH=9,feedback=9,rejected_attempts={8,9})
      delayed terminal for older rejected attempt 8 cannot supersede active 7 or emit terminal side effects; retaining `rejected_attempts={8,9}` distinguishes this state from a state that received only REJECT(9).
```

`T-06` 明确禁止旧文档的 `O -> PLAY`；`T-03/T-05/T-08` 明确证明 STOP 不消费 terminal；`T-09` 固定 supersede 必须先终结旧 active；`T-10` 固定已驱逐 terminal replay 不重放副作用；`T-11` 固定 generation 必须进入非-BEGIN identity；`T-12` 固定 R-12 的单一 transition 在末尾执行 R-16 容量后置条件且不得移除 floor；`T-13` 固定 newer terminal 的 supersession cascade 必须清理旧 active；`T-14` 固定 pending pre-admission STOP 只影响 pending attempt，CASTING 消费后 PLAY 仍可达；`T-15` 固定被 `R-04` 拒绝的迟到 terminal 不得进入 supersession。任何新增 scenario 必须列初态、每条 message、命中的 `R-*` 和终态；若无法逐边命中，本 plan 有错，禁止用 prose 为场景开例外。

---

## 4. Real producer authorization 与 AV contract

### 4.1 全注册集合与玩家可达性普查

集合口径固定为生产 `init_registry()` 与 `TECHNIQUE_DEFINITIONS`，不是文档清单或测试 fixture：

| 技能族 | registry | definitions 命中 | 权威可达性结论 | 五件套现状/本轨动作 |
|---|---:|---:|---|---|
| carrier/anqi v2 | 1+5=6 | 6 | 玩家可达 | 已有分散 AV；P3 纳入统一 binding |
| burst_meridian | 4 | 4 | 玩家可达 | 已有分散 AV；P3 纳入统一 binding |
| zhenmai v2 | 5 | 5 | 玩家可达 | AV 存在；`sever_chain` HUD 语义仍错，P3 修 |
| woliu v1/v2/v3 | 1+5+5=11 | 11 | 玩家可达 | 已有分散 AV；P3 纳入统一 binding |
| woliu 虚蚀路径 | 5 | 0 | **玩家定义断链** | 五招 animation 资源也缺失；P3 同时补 definition 与五件套 |
| yidao | 5 | 0 | **玩家定义断链** | resolver 有两段动画及 VFX/audio spec；P3 补权威定义/HUD/icon 后统一注册 |
| dugu v2 | 5 | 0 | **玩家定义断链** | 局部五件套结构存在但正式技能栏/HUD/icon 断链；P3 修 |
| baomai v3 | 6 | 2 | 4 招玩家定义断链 | resolver/event 双源仍在；Wave 2 atomic activation 前去重，P3 补 4 条定义 |
| tuike v2 | 3 | 3 | 玩家可达 | `shed` 音频已单源；其余视觉及 `don/transfer_taint` 音频仍双路；Wave 2 atomic activation 前去重 |
| sword_basics | 4 | 4 | 玩家可达 | 已有分散 AV；P3 纳入统一 binding |
| cultivation::dugu | 2 | 2 | 玩家可达 | 已有分散 AV；P3 纳入统一 binding |
| dandao | 3 | 0 | **玩家定义断链** | 三招仅局部粒子素材，正式 animation/VFX/SFX/HUD/icon 未闭环；P3 修 |
| sword_path | 5 | 5 | 玩家可达 | 已有独立事件 AV；P3 纳入统一 binding |
| npc-named skills | 3 | 3 | **Player+NPC 双受众**：既在 `TECHNIQUE_DEFINITIONS` 玩家集合，也由 NPC AI 注册调用 | P3 用 `audience=Both` 显式化；玩家侧须五件套，NPC caster 使用专属粒子/audio，明确无玩家骨架动画 |
| morph | 1 | 1 | 玩家可达 | 已有 AV；P3 纳入统一 binding |
| **合计** | **68** | **46** | **22 条 registry-only** | exact `init_registry()` module ledger：`carrier=1 + anqi_v2=5 + burst_meridian=4 + zhenmai_v2=5 + woliu=1 + yidao=5 + woliu_v2=15 + dugu_v2=5 + baomai_v3=6 + tuike_v2=3 + sword_basics=4 + cultivation::dugu=2 + dandao=3 + sword_path=5 + npc=3 + morph=1 = 68`；definition 命中非零项：`carrier/anqi=6 + burst_meridian=4 + zhenmai_v2=5 + woliu v1/v2/v3=11 + baomai_v3=2 + tuike_v2=3 + sword_basics=4 + cultivation::dugu=2 + sword_path=5 + npc=3 + morph=1 = 46`，其余四行命中为零；registry-only：`woliu 虚蚀=5 + yidao=5 + dugu v2=5 + baomai_v3=4 + dandao=3 = 22`；definition-only `3`，故 `TECHNIQUE_DEFINITIONS/TECHNIQUE_IDS = 46+3=49`。唯一计数来源是 `server/src/cultivation/skill_registry.rs::init_registry()` 具体注册函数，禁止从视觉族名重复计数。 |

另有 definition-only 三条 `movement.dash`、`shield_block`、`body.guangbo_ticao`，它们走专用 intent/system 而非 `SkillRegistry`。`SkillRegistration` 的 canonical shape、唯一 lookup 与 consumer interface 在 P1 contract-first 冻结并建立为同一 authority 的 inert implementation/entry ledger；该 lookup 必须同时支持两类 entry：`SkillRegistration::Skill { skill_id, definition, resolver/dedicated handler, av }` 与 `SkillRegistration::ItemCast { item_template_id, cast_variant, av_binding_key, av }`。后者是 QUICK_SLOT 的唯一权威 AV entry，`skill_id` 保持 null，不通过隐式 item→skill 映射解析。P1 只建立并审计该统一 lookup 的 inert shape/ledger，不让它 production-reachable，也不创建 test-only 平行 registry。Wave 2 production activation 必须等待 P2 双源清除与 P3 registration/assets 就绪；P3 在同一 atomic activation merge unit 内完成全量 68 resolver + 3 dedicated 与 item-cast entries、definitions/AV bindings/assets、唯一 AV consumer 和 producer cutover。不得用“先启用已完成子集、再在 P3 补其余 registration”的分段 live path 绕过完整 registration authority。

本矩阵中的“五件套已有”只表示当前代码能找到对应局部映射/资产，不代表已由机器证明唯一消费。P1 冻结统一 lookup、accepted-key contract 与唯一 consumer interface，但不切换生产 registration；P2 清除 duplicate owners 并锁定终态契约；Wave 2 只在 P3 全量 registration/binding/assets、duplicate owner 清零且 unique consumer 已就绪后，将全部 real cast producers 原子切换。P3 在同一 unified lookup 上完成全量 68 resolver + 3 dedicated 与 item-cast entries，并删除旧 canonical 表。P3 的 registry 精确集合测试逐条验证完整 definition 以及 animation、VFX/audio/HUD 的 start/release/complete/interrupt phase binding、icon 均非空且真实存在，缺口数必须为零。

### 4.2 权威 producer / authorization rows

| ID | real entry | authorization gate | source / skill_id | required emissions | live evidence |
|---|---|---|---|---|---|
| `A-01` | `handle_use_quick_slot` / `UseQuickSlotRequestV1` | slot、inventory instance、item cast/target rule；不查 global skill membership 冒充 item ownership | `QUICK_SLOT / null / canonical av_binding_key(item_template_id, cast_variant)` | accepted CASTING 或瞬发 COMPLETE；所有结束带同 identity terminal；binding key 声明的 AV | request-handler e2e + bot official entry |
| `A-02` | `handle_skill_bar_bind` / `SkillBarBindRequestV1` | global registration 存在 **且** 玩家 `KnownTechniques` learned+active | 不发 cast；绑定项保存 canonical skill ID | unauthorized bind 拒绝且不污染 binding | empty/missing/inactive/active `KnownTechniques` live request |
| `A-03` | `handle_skill_bar_cast` / `SkillBarCastRequestV1` | 当前槽绑定、global registration、玩家 learned+active、race/meridian/config/target/cooldown/qi gates | `SKILL_BAR / required / registration av_binding_key` | 每次进入 admission 先分 identity；accepted/terminal 或 `IDLE+Reject*` / `IDLE+MERIDIAN_GATED`；AV 仅 accepted | bind→cast 正负 e2e，禁止 pure helper 替代 |
| `A-04` | registration 指定的 official dedicated handler | unique handler ownership；玩家入口仍查 learned+active 与适用 gates | `DEDICATED,npc=false / required / registration av_binding_key / non_slot{kind=DEDICATED}` | 与 `A-03` 同身份/终态契约；不得旁路 reducer | `movement.dash`、`shield_block`、`body.guangbo_ticao` 各 official handler e2e |
| `A-05` | NPC resolver；NPC spawn/tracking-epoch entry 与 observer re-entry producer 先为每个 recipient 发该 NPC 的 authoritative BEGIN，再发 lifecycle | global registration、NPC AI 自有 gate；不得伪造玩家 KnownTechniques；BEGIN/后续 payload 共用 caster=npc ref 与 session generation | `DEDICATED,npc=true / skill_id required / registration av_binding_key / non_slot{kind=DEDICATED}`；只能配 `audience=Npc|Both`，不得进入玩家 skillbar | authoritative CASTING/terminal lifecycle；只消费 Npc/Both visual；BEGIN、CastSync、PLAY/STOP 均走同一 reducer/AV chain | NPC resolver integration + real tracking entry/re-entry e2e |
| `A-06` | pre-cast reject producer | 对 mandatory config 缺失/非法逐字段 fail-closed；不得回显或猜测缺失/非法请求字段 | 对外唯一使用 protocol-owned reject envelope：`source=DEDICATED,npc=false,skill_id=null,slot=non_slot{kind=DEDICATED},av_binding_key="cast.reject/<canonical-outcome>"`；固定 allowlist key 不属于 gameplay registration | 只发 `IDLE+Reject*` 或 `IDLE+MERIDIAN_GATED`；不得扣 qi/stamina、写 cooldown/inventory/target、发任何 AV/STOP；该 envelope 不进入 AV consumer | active 与 idle 两前态全矩阵 |

`A-02/A-03/A-04` 强制区分 registration membership 与 player authorization。active A 时 attempt B reject 只按 `R-04` 更新 B feedback，A 的 gameplay/AV/token 与后续 `R-08` 能力保持；任何“reject 顺手 reset cast”的实现不合格。

### 4.3 `SkillAvBinding` 冻结

`SkillAvBinding` 由统一 registration lookup 索引；skill entry 以 `skill_id` 识别，item-cast entry 以 `(item_template_id, cast_variant)` 识别并拥有自己的 canonical `av_binding_key`。QUICK_SLOT 的 `skill_id` 保持 null，只能查 item-cast entry；SKILL_BAR/DEDICATED 使用 skill entry 的 `skill_id` 与同一 registration key，NPC DEDICATED 选择 `SkillVisualBinding::Npc`/`Both` 的 NPC arm。任何 producer 或 consumer 都不得从 `skill_id=null` 猜测 binding，也不得另建 lookup 或隐式 item→skill 表。

P3 production cutover 的数据形状冻结为：

```rust
enum SkillCastMode { Resolver, Dedicated { handler: DedicatedHandlerId } }
struct SkillRegistration {
    resolver: Option<SkillFn>, audience: SkillAudience, cast_mode: SkillCastMode,
    definition: TechniqueDefinition, av_binding_key: &'static str, av: SkillVisualBinding,
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

`SkillRegistration.definition` 是所有 gameplay/技能栏元数据的唯一 owner；现有 `TechniqueDefinition.icon_texture` 删除，图标只来自 `av.icon`。`TECHNIQUE_DEFINITIONS`、`TECHNIQUE_IDS` 和 skillbar snapshot 均由 registration 投影生成，不再保留手写 canonical 数组。`cast_mode=Dedicated` 使用 `resolver=None`，但仍须携完整 definition、官方 handler 标识、`av_binding_key` 与玩家 AV binding。`SkillAvBinding.vfx/audio/hud` 的每个 phase 槽位明确声明该效果是否在 `start`、权威 `release`、`complete` 或 `interrupt` 发射；空槽表示该阶段不适用，消费者只读 binding，不得在 resolver、router 或 skill-id 特判表里补阶段语义。`release` 与 `complete` 仅 COMPLETE 可消费，`interrupt` 仅 INTERRUPT 可消费；任何 INTERRUPT 必须跳过 release/complete。

约束：

1. `SkillRegistry::register` 改收完整 `SkillRegistration`；同一 `skill_id` 或同一 cast 的多个 emit owner 均启动 fail-fast。不同玩家技能不得复用任一完整 AV 通道：任意两个不同的 Player/Both 技能，其 animation、VFX、SFX、HUD feedback、icon 五个绑定值必须逐字段全部不同；单个底层素材只有在不落入玩家技能绑定通道时才可复用（否则无法辨招）。resolver 只发送“技能已接受/命中/结算”领域事件，不得直发绑定中的 animation/VFX/audio，唯一 AV consumer 按 registration 发射；每个 `SkillAvPhaseBinding` 槽位只能由该唯一 consumer 按其声明 phase 发射。
2. `definition.id == key` 且 `av_binding_key` 在 registration lookup 中唯一；Resolver 有 resolver；Dedicated 无 resolver且恰有一个 official handler；definition/skillbar 只读投影。
3. 玩家受众 (`audience=Player|Both`) 五字段全部必填并验证真实 client 资源/recipe/handler；纯 NPC 受众显式免除 HUD/icon，animation 不适用时也必须用明确 `NpcVisual` 类型，禁止空串冒充。
4. 占位只允许 `SkillIconBinding::ExplicitPlaceholder`，且必须携 `[BLOCKED: 需 /gen-image ...]` blocker、引用真实占位资产并出现在启动汇总；animation/VFX/audio/HUD 不允许 placeholder 或静默 fallback。P3 归零所有 placeholder 后才可完成。
5. 多阶段招式用 `start + optional release + looping`；施法专用 `CastPlayAnim` 与 `CastStopAnim` 都携完整 `CastIdentity`，STOP 只可停止客户端记录为同一 identity 的 `start` 循环层，release 只在权威完成时播。禁止另建 `looping_cast_anim_id(skill_id)` 特判表。
6. icon 与 quick-slot item binding 单一真相源迁入 registration lookup 后，由它派生 technique/skillbar/client icon snapshot；不得继续维护同一 skill/item cast 的第二份路径字面量。`av_binding_key` 必须随 CastSync 穿过 producer、bridge、router、reducer 与唯一 AV consumer，consumer 不得重新推导。

| ID | registration invariant |
|---|---|
| `A-07` | `definition.id == key`；`av_binding_key` 在 registration lookup 中唯一；Resolver 有 resolver；Dedicated 无 resolver且恰有一个 official handler；definition/skillbar 只读投影。 |
| `A-08` | Player/Both 五件套真实存在且逐通道可辨；Npc 用显式类型表达 HUD/icon/player animation 不适用。 |
| `A-09` | phase slot 是唯一 emit 声明；COMPLETE 消费 release/complete，INTERRUPT 只消费 interrupt。 |
| `A-10` | 仅 icon 允许显式 blocker placeholder，P3 完成前归零；其它四件套不允许 placeholder。 |
| `A-11` | 每个 `(CastIdentity,av_binding_key,av_kind,phase)` 恰有一个 registration consumer owner。 |
| `A-12` | looping owner identity-aware；STOP/terminal 只走 `R-08/R-11..R-13`，无 skill-id stop 特判表。 |
| `A-13` | VFX 表示必须携 capability tier；Iris 是 client 可选能力，server 不假设渲染能力。 |

#### Iris shader capability（可选能力，不是 server 前置）

`SkillVfxBinding { phases: SkillAvPhaseBinding, capability: VfxCapabilityTier }`；tier 为 `Vanilla` 或 `ShaderOptional { iris_effect_id, fallback: VanillaParticle(id) | NoOp }`。client 运行时通过 Fabric Loader 检测 Iris，存在才选 shader；缺失则 fallback/no-op。server 只 emit semantic effect ID/tier；Iris 缺失不影响 gameplay、CastSync、AV ownership 或其它通道。

---

## 5. End-to-end ownership 与原子 cutover DAG

### 5.1 Artifact ownership rows

| ID | artifact / live edge | single owner | phase | completion evidence |
|---|---|---|---|---|
| `C-01` | Canonical TypeBox cast domain content：BEGIN/CastSync/identity/source/target/outcome/PLAY/STOP semantics | R9 domain owner；schema authority and cross-track boundary按总纲 §4.1/PR #1902 adjudication | P1 contract-first | `P-01..P-15` domain rows + source-level semantic tests |
| `C-02` | R6 generation pipeline output for cast：schema dist、JSON Schema、registry/generated samples 与 constrained mirrors | R6 shared-wire owner | generation contract / cutover unit | package build/test；generated diff and sample pins |
| `C-03` | protobuf declarations/field numbers/oneofs + generated Rust/Java | R6 shared-wire owner | generation contract / cutover unit | buf breaking + generated pin |
| `C-04` | Rust domain DTO、session/attempt allocator 与 producer contract declarations/tests；其 **Wave 2 live slice** 才拥有 connection/tracking-epoch BEGIN producer 与 real producers | R9 | P1 contract-first（contract slice）+ Wave 2 activation（live slice） | P1 仅验 DTO/allocator/producer contract 与 inert tests；Wave 2 才验真实 connection entry、caster tracking entry、observer re-entry 发 player/NPC authoritative BEGIN，fixture 注入不能替代 live producer |
| `C-05` | `ServerDataEnvelope` arms 与 `proto_convert.rs` | R6 shared-wire owner | cutover unit | four-arm conversion roundtrip |
| `C-06` | `ProtoServerDataBridge` conversions/normalization | R6 shared-wire owner | cutover unit | proto→Java boundary/unknown tests |
| `C-07` | `ServerDataRouter` keys：`cast_session_begin`、`cast_sync`、`vfx_event.cast_play_anim`、`vfx_event.cast_stop_anim` | R6 shared router owner | cutover unit | four-key registration/dispatch/duplicate/unknown tests |
| `C-08` | concrete BEGIN + CastSync consumer → `CastSessionRegistry.reduce` | R9 | cutover unit | real router dispatch to `R-01..R-09/R-14`，含 player/NPC tracking-entry BEGIN snapshot 与后续 CastSync |
| `C-09` | concrete PLAY + STOP consumer → `AnimationLayerManager` / `CastFovController` | R9 | cutover unit | real router dispatch to `R-10..R-14` |
| `C-10` | `bong:vfx_event` cast producer/receiver removal；all cast emit 改 `bong:server_data` | R6 channel machinery + R9 consumer activation in the same unit | cutover unit | old receiver zero-hit + new live path |
| `C-11` | `CastSessionRegistry`、lifecycle adapter、FPV identity/token、movement/contamination/control/user-cancel/death/disconnect/dimension-change terminal producer adapters；Fled generic tribulation brace 不属于 cast producer，沿独立 non-cast animation lifecycle 处理 | R9（消费 R2 P1 lifecycle API） | cutover unit | `R-15/R-16` churn + juice identity + every cast terminal producer reaches authoritative `R-08/R-11..R-15` on the live path；Fled 由其 generic `StopAnim` contract 单独验收，不伪造 CastIdentity |
| `C-12` | Baomai/Tuike single owner 与 `meditate_sit.json` pose oracle：torso.pitch=0；head.pitch≈+0.2094395rad；both leg.pitch∈[-0.698132,0]rad；opposite leg yaw≈±0.436332rad；leg bend≈1.570796rad；each used loop axis repeats at endTick；P2 先清除 duplicate owners，Wave 2 activation 再切换 canonical consumer/producer | R9 | P2（single-owner cleanup + pose）+ Wave 2 activation（cutover） | P2 `A-09/A-11/A-12` pins prove no duplicate emit owner；Wave 2 `D-21` proves live stop semantics；不得在 duplicate owners 仍存在时启用新 consumer |
| `C-13` | P1 冻结并建立唯一 registration lookup（skill entries + item-cast entries）、accepted-key lookup 与 unique AV consumer interface 的 inert contract/entry ledger；P2 清除 Baomai/Tuike duplicate owners；P3 在同一 Wave 2 atomic activation merge unit 内完成全量 68 resolver + 3 dedicated、item-cast entries、definitions/AV bindings/assets、consumer enablement 与 producer cutover | R9 | P1 contract-first（shape/lookup/consumer contract）+ P2 single-owner cleanup + P3/Wave 2 atomic activation（full registration migration/cutover） | P1 `D-06` 只验 contract mapping 与 inert lookup shape；P2 `D-12` 必须证明 duplicate owner 已为零；P3/Wave 2 的 `D-06`/`D-10` 必须证明每个 accepted producer key（含 QUICK_SLOT item-cast key）可由同一 lookup 进入唯一 consumer，并与 transport/terminal producers 同一 merge unit 切换；`A-02..A-13` projection/real-entry tests 与 `D-25` 逐项对拍 |
| `C-14` | `client/resourcepack/manifest.json`、built zip SHA-1/size、`server/src/network/resourcepack.rs::DEFAULT_RESOURCE_PACK_MANIFEST` | R9 asset PR | P3 same delivery | build-resourcepack + committed SHA-1/size exact match |
| `C-15` | contract/live/bot/release evidence 与 absorbed-plan archive | R9 | P4 | §6 derived index 全绿 |

BEGIN 与 CastSync 的 production registration 不再遗漏：它们由 `C-07+C-08` 明确拥有；PLAY/STOP 由 `C-07+C-09` 拥有。fixture 直接调用 reducer只能证明 `C-08/C-09` 的局部语义，不能完成 live edge。

### 5.2 DAG 与阶段门

```text
WAVE-0 CONTRACT-FIRST:
  R9 C-01+C-04 + C-13 contract slice (canonical shape/lookup/consumer, inert entries)
       │
       └── R6 consumes the frozen domain content for C-02/C-03 generation contract

WAVE-2 PRODUCTION:
  R5 P1 + R6 generation/transport artifacts + R2 P1 seam + P2 C-12 single-owner cleanup
       │
       └── P3 FULL REGISTRATION + ATOMIC-ACTIVATION = complete all 68+3/item-cast registrations/bindings/assets
                                                  + enable C-13 canonical lookup + unique AV consumer
                                                  + C-05+C-06+C-07+C-08+C-09+C-10+C-11
                                                  + cut all real producers and required terminal producers
                               │
                               └── C-14 -> P4 C-15
```

`C-13` is intentionally split by delivery slice: P1 freezes and lays down one canonical lookup shape covering skill and item-cast entries plus its inert accepted-key lookup/consumer interface; P2 removes the known Baomai/Tuike duplicate owners; P3 then completes the full 68-resolver + 3-dedicated and item-cast registration/binding/asset migration and, in the same Wave 2 atomic activation merge unit, enables the unique AV consumer and cuts all real producers/required terminal producers over. No accepted live cast may be activated before its complete registration entry and consumer are present. P3 does not redefine the lookup or introduce a second registry.

| ID | merge invariant |
|---|---|
| `C-INV-01` | contract phase只增加 declarations、generated mirrors、reducer/producer tests，以及 P1 已冻结的 canonical `SkillRegistration` shape/lookup/consumer interface 与 inert entry ledger；未激活的 arms 不得宣称 production reachable。 |
| `C-INV-02` | production cutover 默认是同一 merge unit：新 channel producer、四 key router、BEGIN/CastSync/PLAY/STOP concrete consumers、bridge/conversion、P2 已清除 duplicate owners、P3 已完成全量 registration/binding/assets、唯一 registration lookup 与 AV consumer、旧 cast receiver removal，以及 movement/contamination/control/user-cancel/death/disconnect/dimension-change cast terminal producers 同时生效；generic Fled brace 不在 cast cutover。Wave 2 前任何未完成 registration/binding 或仍有 duplicate owner 的 producer 都必须保持完整旧路径，不得被新 consumer 部分接管。 |
| `C-INV-03` | 若跨 track 必须拆 PR，只允许先合入 inert declarations/conversions 与 P1 已冻结但 Wave 2 前不被 producer 使用的 unified lookup entries；旧 receiver 与旧 producer保持完整。最终 activation PR 必须同一 merge unit 完成 P2 duplicate-owner removal 后的全量 registration/binding/assets、切所有 real producer、启用四 consumers、启用该唯一 lookup/AV consumer并删除旧 receiver；不得以“先开 consumer、后补 registration/去重”的中间态宣称 production reachable。 |
| `C-INV-04` | **禁止 receiver-removed-before-consumer-installed window**；也禁止 producer 双投旧/新 channel造成 AV 双发。每个可合入中间态必须是“完整旧路径”或“完整新路径”，不能是半条路径。 |
| `C-INV-05` | “可开始”只由 WAVE-0 CONTRACT-FIRST 决定；“可宣称生产可达”必须 ATOMIC-ACTIVATION 和 live evidence 全绿。R6 generation/transport artifacts 与 R2 seam 属 production reachability gate，不是 R9 contract start gate。 |
| `C-INV-06` | P3 full registration migration includes init_registry ledger 所列 68 resolver + 3 dedicated production registrations、item-cast entries、definitions、projection 与旧 canonical tables 删除；这些条目必须沿用 P1 已冻结的 unified lookup，且只有在 P2 duplicate-owner removal 完成后，才能与 Wave 2 的同一 activation unit 一起启用该 lookup/unique AV consumer、全部 real producer 与 required terminal producers；禁止 subset live path、第二 registry 或 registration 未齐时的 consumer activation。 |
| `C-INV-07` | 新增/修改任何 client asset 的 PR 同时拥有 `C-14`，不得把 manifest/SHA-1/size 留给后续补丁。 |


---

## 6. Derived acceptance index（只引用规范 row IDs）

本节不定义状态或业务规则。测试名称、参数和 oracle 必须从引用的 row IDs 生成；测试失败信息必须打印 trace 中每一步的 row ID。新增规范 row 时 static consistency test 要求本索引有覆盖；新增测试不得复制 transition prose。

| ID | test / evidence | normative refs |
|---|---|---|
| `D-01` | `cast_protocol_shape_cross_stack`（含逐 required-field missing-field matrix） | `P-01..P-15` |
| `D-02` | `cast_outcome_all_variants`（含 `IDLE+MERIDIAN_GATED` 合法配对） | `P-08,P-09,P-15,C-01..C-03` |
| `D-03` | `cast_reducer_all_rows_all_states`（R-01..R-15 message rows 与 R-16 容量后置条件） | `I-04..I-10,INV-01..INV-04,R-01..R-16` |
| `D-04` | `cast_trace_static_consistency`（每条 message edge 命中一个 R-01..R-15；涉及新增 terminal/AV tombstone 的 edge 同时执行 R-16 后置条件） | `INV-04,R-01..R-16,T-01..T-15` |
| `D-05` | `cast_session_lifecycle_churn`（P1 仅验 inert contract；Wave 2 另验 connection/tracking entry 与 observer re-entry 的 player/NPC BEGIN replay） | `P-04,P-05,P-12..P-14,R-01..R-03,R-15,R-16,C-04,C-08,C-11` |
| `D-06` | `cast_real_producer_mapping`（P1 仅验 contract mapping；Wave 2 live slice 必须在任何 real producer activation 之前完成，随后才验真实 connection/tracking/official producer path，含 `IDLE+MERIDIAN_GATED` mapping） | `P-06..P-10,A-01,A-03..A-06,C-04` |
| `D-07` | `cast_known_techniques_bind_cast` | `I-11,A-02..A-04,A-07,C-13` |
| `D-08` | `cast_active_preservation_on_reject` | `INV-03,R-04,R-05,R-08,A-06,T-15` |
| `D-09` | `cast_allocator_exhaustion` | `P-02,P-04,P-05,P-13,R-01,R-04,R-08,R-12,R-15` |
| `D-10` | `cast_live_four_arm_dispatch`（只有在 Wave 2 atomic activation 同时完成 P2 duplicate-owner cleanup、C-13 全量 registration/binding/assets、unique AV consumer 安装与 `D-06` live mapping 后，才切 real producers） | `P-11,P-14,C-05..C-11,C-13,C-INV-01..C-INV-05` |
| `D-11` | `cast_stop_never_terminal` | `INV-01,INV-02,R-08,R-09,R-11..R-13,R-16,T-03,T-05,T-08,T-12,T-14` |
| `D-12` | `p2_av_single_owner` | `A-09,A-11,A-12,C-12` |
| `D-13` | `p2_terminal_state_matrix` | `I-04,I-06,R-08,R-11..R-16,T-10,T-13,T-14,C-12` |
| `D-14` | `p2_stop_reordering` | `R-06,R-08,R-10..R-14,R-16,A-12,C-12,T-12,T-13,T-14` |
| `D-15` | `p2_meditate_animation_pin` | `C-12` |
| `D-16` | `p2_av_phase_binding` | `A-09,A-11,A-12,C-12` |
| `D-17` | `p3_registration_projection` | `A-02..A-13,C-13,C-INV-06` |
| `D-18` | `p3_resourcepack_release` | `C-14,C-INV-07` |
| `D-19` | `p3_iris_capability_fallback` | `A-13,C-13,C-14` |
| `D-20` | `cast_registry_reachability` | `A-01..A-08,C-13` |
| `D-21` | `cast_stop_semantics` | `R-08..R-16,A-12,C-08..C-12,T-12,T-13,T-14` |
| `D-22` | `cast_av_uniqueness` | `A-07..A-13,C-12,C-13` |
| `D-23` | `cast_wire_identity` | `P-01..P-15,C-01..C-07,I-03,I-04,INV-05,T-11` |
| `D-24` | `cast_av_phase_regression` | `A-09,A-11,A-12,C-12,I-06,INV-01,INV-02,T-14` |
| `D-25` | `cast_registration_projection` | `A-07..A-13,C-13,C-INV-06` |
| `D-26` | `cast_juice_identity_bridge` | `R-08..R-14,C-05..C-11,I-03,I-10,T-11..T-14` |
| `D-27` | runClient 远处读招/HUD/icon/循环停止 + Iris present/absent | `A-08..A-13,C-13,C-14` |

`D-15` 的资产 oracle 原样固定为：用 headless 渲染/姿态断言 `meditate_sit.json` 维持直立 torso（`torso.pitch=0`）、垂目 head pitch 约 `+0.2094395rad`（+12°）及双腿目标盘坐姿态：两腿 pitch 必须落在明确的修复区间 `[-0.698132, 0.0]rad`（[-40°, 0°]），双腿 yaw 保持相反符号且绝对值约 `0.436332rad`（25°），双腿 bend 约 `1.570796rad`（90°）承担折腿；不得再出现当前 `-1.3962634rad`（-80°）过旋。循环动画每个使用轴在 endTick 有同值关键帧，P4 复跑同一完整姿态 oracle。

`bot 场景` 的七个主题为：`cast_registry_reachability`、`cast_stop_semantics`、`cast_av_uniqueness`、`cast_wire_identity`、`cast_av_phase_regression`、`cast_registration_projection`、`cast_juice_identity_bridge`。七个 `scripts/bot/scenarios/` 文件/anchor 必须在 P1 contract-first 与 reducer/producer work 同步首次提交；P1 只要求它们引用冻结的 `D-*` 并保持 inert，不得宣称 production reachable。P2 将 `cast_stop_semantics`、`cast_av_phase_regression` 与真实 single-owner/terminal producers 接线，P3 将 `cast_registry_reachability`、`cast_av_uniqueness`、`cast_registration_projection` 与真实 registration/assets 接线，`cast_wire_identity` 与 `cast_juice_identity_bridge` 随 P1 atomic activation 的 live transport/bridge 接线；P4 只完整复跑七场景、执行真实 transport 与 official entries 并归档 evidence，不得把 scenario 首次创建推迟到 P4。场景文件只引用对应 `D-*`，不得另写 reducer 语义。

---

## 7. 实施 phases（只落地既有规范节点）

### P0 — protocol/reducer/producer/cutover 设计收口 ✅ 2026-08-04

- 已形成 §1 不变量、§2 TypeBox-first authority、§3 唯一 reducer、§4 producer 授权表、§5 cutover DAG、§6 derived index。
- P0 static review 只检查 ID 唯一、引用存在、每个 trace 逐边命中 reducer、STOP 不引用 gameplay terminal side effect。

### P1 — cast contract ⬜

- 完成 `C-01` 与 `C-04` 的 **contract slice**、`D-01..D-06,D-08..D-09`；只增加 R9 domain declarations/reducer/producer tests 与未启用声明。cast_sync 的 source/target/phase、STOP/INTERRUPT 权威事件与 `SkillAvBinding` fail-fast artifacts 首次提交必须同步其完整 pin suite（正例覆盖每个 phase/discriminant，反例覆盖 required field 缺失及 invalid/unknown source/target/phase），不得延期到后续 P4/P5 或仅以 bot/e2e 补测。七个 `scripts/bot/scenarios/` 文件/anchor 与本阶段 contract/reducer work 同步首次提交，引用冻结的 `D-*` 但保持 inert，不宣称 production reachable。
- 冻结并建立唯一 unified registration lookup（skill entries + item-cast entries）、accepted-key lookup/consumer interface 与 inert entry ledger（`C-13` 的 contract slice），但不让该 lookup 或任何 producer production-reachable，也不创建 test-only 平行 registry。`C-04` 的 connection/tracking-epoch BEGIN producer 与 real-producer live slice 留到 Wave 2 atomic activation；R6 据冻结的 domain content 生成 `C-02/C-03` mirrors，本阶段不激活 production path，也不等待 R5/R6/R2 production artifacts。`D-07` 依赖 P3 的 `C-13` full registration migration，留在 P3 gate。
- PUC-01 只建立 Wave 2 activation 所需的完整 registration ledger：逐项列出全部 accepted producer keys、对应完整五件套、terminal producer 与唯一 consumer；该 ledger 不授权 subset live path。严禁在 duplicate owner 未清、完整 registration/assets 未齐或 unique consumer 未安装时启用任何 real producer。

#### P1/C-13 contract-first evidence（2026-09-02）

- `server/src/cast/mod.rs` 建立唯一 `SkillRegistrationLedger`：`SkillRegistration::Skill { skill_id, definition, resolver, cast_mode, audience, av }` 与 `SkillRegistration::ItemCast { item_template_id, cast_variant, av_binding_key, av }` 共用 `RegistrationKey` lookup；item-cast/QUICK_SLOT 的 `skill_id()` 固定返回 `None`，没有 item→skill 隐式映射。
- 同一文件冻结 `SkillAvBinding` 五件套（animation/VFX/audio(SFX)/HUD/icon）及 phase binding、NPC 显式免除类型、`SkillAvConsumer` 唯一 consumer seam；ledger 只保存 `Declared`/`Unwired`/`TestOnly` inert 状态，未安装 Bevy resource、未接真实 producer，不创建 test-only 平行 registry。
- `cast::tests` 的 10 条独立 contract pin 覆盖统一 lookup、key identity、skill/item-cast 区分、QUICK_SLOT null skill id、AV key/channel uniqueness、resolver/dedicated 约束、definition/phase 非法输入、audience/NPC arm 与 inert state/request；重复、空值和非法 registration 在 ledger 发生 mutation 前 fail-fast。

### P2 — 双源清除 + 全退出终态 ⬜

- 完成 `C-12`：Baomai/Tuike 余下双源归一；duplicate-owner removal 是 Wave 2 activation 的前置门，不能推迟到 consumer 已启用之后。对 Wave 2 将接入的移动、污染、控制、用户取消、死亡、断线、换维度 cast producer 做 `R-08/R-11..R-15` 强制回归，但不在 P2 首次接线。generic Fled brace 继续由独立 non-cast lifecycle 验收，不列入 `D-21` cast stop semantics。`cast_stop_semantics`、`cast_av_phase_regression` 两个 bot scenario 在本阶段锁定 single-owner/终态契约，Wave 2 再接入真实 live path。其余 producer 继续完整旧路径。
- 完成 `D-12..D-16,D-21,D-24`；`D-21` 是 P2 completion gate，必须覆盖 movement/contamination/control/user-cancel/death/disconnect/dimension-change 的真实 cast stop semantics；`dugu.penetrate` 只保留现状防回归 pin。
- 视觉资产 `meditate_sit.json` 按三轮打磨，Round 3 commit 带 `<PROMISE>`。

### P3 — 全量 registration/assets + Wave 2 原子切换 ⬜

- 补齐全量 registry-only definitions、item-cast entries 与缺失五件套；完成 `C-13,C-14` 和 `C-INV-06,C-INV-07`。
- 仅在总纲 §3 **Wave 2**、R5 P1、R6 generation/transport artifacts、R2 P1 seam、PR #1902 production conditions、P2 duplicate-owner cleanup 与 `D-06` live mapping 全部满足后，在同一 `ATOMIC-ACTIVATION` merge unit 上完成 unified lookup 的全量 init_registry ledger 所列 68 resolver + 3 dedicated 与 item-cast entries、definitions/AV bindings/assets、unique AV consumer、所有 real cast/terminal producers、transport/bridge/router、四 consumers 与旧 receiver removal。删除手写 canonical `TECHNIQUE_DEFINITIONS/TECHNIQUE_IDS`，projection 保留只读派生 API；不得先切 subset 或另建第二 owner/registry。
- 完成 `D-07,D-10,D-17..D-20,D-22,D-25,D-26`；`cast_registry_reachability`、`cast_av_uniqueness`、`cast_registration_projection` 三个 bot scenario 在同一 atomic activation 中随全量真实 registration/assets 接线并验收。`cast_registry_reachability` 必须从 official entry 走真实 server producer → transport → bridge/router → `CastSyncHandler`/唯一 AV consumer 全链路，逐招断言权威 CASTING 与 cast_sync 的 source/target/phase；`cast_stop_semantics` 的打断路径必须断言 INTERRUPT（而非 STOP）由真实 producer 穿越同一链路，逃跑/断线才断言 STOP，禁止用手工构造 payload 或仅单元注入 INTERRUPT 代替 production evidence。icon placeholder 归零，Iris present/absent 两路均验收。釆用同一 production merge unit 完成 producer、generated/protobuf mirror、Rust conversion、Java bridge/router、client consumer 与旧路径移除；未闭合前旧路径必须完整保留，不得长期双发。
- 所有新增 animation/VFX/icon 资产执行 3 轮打磨；icon 走 `/gen-image item`，不能运行时标 blocker 但不得把 P3 标完成。未满足 Iris runtime capability gate 的 shader/VFX 只交付 capability gate 与 vanilla/no-op fallback，并列入 skipped，不得宣称 shader effect 已完成。其余未完成项在 activation 前保持完整旧路径。

### P4 — 派生验收、人工回归与归档 ⬜

- 原样复跑 `D-01..D-27`，不得抽样；七个已在 P1 创建、并在 P1/P2/P3 分阶段接线的 `scripts/bot/scenarios/` 场景走真实 transport 与 official entries，P4 不得首次创建场景。
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
| `skillbar-cast-source-drift` | **仍成立**：wire 无 source，client `sourceFor()` 猜测 | P1 权威 source + identity + binding key |
| `skillconfig-castsync` | **仍成立**：配置 fail-close 分支无纠正回执 | P1 新 reject outcome |
| `zhenmai-sever-marker-hud` | **仍成立**：client 固定显示“断链增幅”，无 amplification 语义分支 | P3 修 payload/HUD |
| `baomai-v3-av-double-source` | **仍成立**：resolver 直发与 `BaomaiSkillEvent` consumers 并存 | P2 唯一 consumer |
| `dugu-penetrate-av-mismatch` | **已关闭，不再实施**：当前 runtime 已取 `visual_for(Penetrate)` 的针掷/针嘶映射 | P4 只归档并记录现状证据 |
| `meditate-sit-leg-pitch` | **仍成立**：`meditate_sit.json` 双腿 pitch 为 -1.396rad（约 -80°），超过约 40°红线 | P2 调低 pitch、以 bend 承担折腿并 headless 回归 |
| `tribulation-fled-brace-stop` | **不吸收为 R9 cast terminal**：其 `TribulationAnnounce`/`TribulationFled` generic brace 没有 `CastSession` 或 `CastIdentity`；继续由独立 non-cast `StopAnim` lifecycle 处理，不伪造 identity | R9 只记录边界，不在 P2/C-11/D-21 要求它进入 cast reducer |
| `tuike-v2-duplicate-av` | **部分关闭**：`shed` 主动签名音已改由事件单源；`don/transfer_taint` 音频及三招视觉仍同时有 resolver 直发和事件 consumer | P2 仅修剩余双源，不回退已完成项 |
| `combat-event-juice-runtime-bridge-gap` | **仅部分吸收**：吸收 cast identity/source/phase/target 与施法 juice 所需字段；命中侧 UUID/school/direction/kill 富化仍归原 plan | P1/P4 限定 cast 子域 |

`plan-fpv-cast-av-v1` **不吸收**：它已有实质进度并独立收尾；R9 P1 只在共享 `CastStateStore`/wire identity 处迁移对齐，不接管 FPV 手臂动画、signature 音频资产或其验收。

原吸收表全部已在本节收口；实施与归档以本裁决为准，不以旧 skeleton 行号或旧结论为准。

### 8.2 明确边界

- **worldview/AV 锚点**：每招独立可辨的 animation/VFX/SFX/HUD/icon 是根 `CLAUDE.md` 红线；audio 保持 Pattern A（使用施法时 `cast_center` 快照，不读取消费时实时 `Position`）。
- **qi_physics 锚点**：本轨不改变扣费、释放或账本语义；P1/P2 只消费 R5 接口，任何 resolver 迁移不得顺手直写 qi。
- **不碰**：FPV 手臂动画与 signature 音频资产；combat hit-event 富化；天道 agent runtime；worldview。
- TypeBox cast schema source 属 `C-01` 的有限跨目录交付，但不触碰 tiandao runtime、prompt、arbiter；这是 R9 domain content ownership 的跨目录例外，不改写 R6 generation/mirror ownership。
- definition-only 三个 dedicated 入口与 NPC 双受众只按 `A-04/A-05/A-07/A-08` 处理，不为迁移方便伪装成普通 resolver。

---

## 9. Operational workflow、校验与 rewrite gate

### 9.1 PR 顺序与文件纪律

1. **PR-1 contract-first**：R9 交付 `C-01` 与 `C-04` 的 contract slice + `D-01..D-06,D-08..D-09`，只落 canonical domain content、reducer/producer tests、统一 skill/item-cast registration lookup shape/consumer interface 与未启用声明；P1 可建立该 lookup 的 inert entry ledger，但不得让它 production-reachable，也不得另建 test-only registry。`C-04` 的 connection/tracking-epoch BEGIN producer 与 real-producer live slice 留到 Wave 2 atomic activation，R6 可据冻结 content 生成 `C-02,C-03` inert mirrors，但双方均不得宣称 production reachable。`D-07` 随 P3 `C-13` full registration migration 交付。
2. **PR-2 P2 single-owner/terminal/meditation**：完成 `C-12` + `D-12..D-16,D-21,D-24`；`D-21` 是 mandatory live-path preparation gate，Wave 2 前必须证明 duplicate-owner 已清零，但不在本 PR 删除旧 receiver 或切新 producer。
3. **PR-3 P3/Wave 2 atomic activation**：仅在总纲 §3 **Wave 2**、R5 P1、R6 generation/transport artifacts、R2 P1 seam、PR #1902 production conditions 与 P2 single-owner gate 满足后，由 R6/R9 在同一 `ATOMIC-ACTIVATION` merge unit 完成全量 `C-13,C-14` registration/binding/assets、启用唯一 AV consumer、交付 `C-02,C-03,C-05..C-11` 与 `D-06,D-07,D-10,D-11,D-17..D-20,D-22,D-23,D-25,D-26`，最后切换全部 real cast/terminal producers 并删除旧 receiver。generic Fled brace 不属于 cast producer，继续走独立 non-cast `StopAnim` lifecycle；若无法形成同一 merge unit，完整旧路径继续保留。
4. **PR-4 P4 full evidence/archive**：`C-15` + `D-01..D-27`。

本顺序显式执行草案总纲 `docs/plans-skeleton/plan-refactor-master-v1.md` §3/§4.1 与 adjudication commit `1d7a257ab7d1f72261aa290a8901df1be3e1dc43`（PR #1902）：contract-first 可在 Wave 0 合入，R9 production activation 必须在 **Wave 2** 按上述依赖原子切换。前一 PR merge 后才开始后一 PR；每次 fetch 最新 `origin/main` 后验证 DAG gate。跨 owner 文件只能按 §5 owner/handoff 修改，不得复制 converter、bridge、router 或 consumer 绕门。任何 visual asset PR 执行 3 轮打磨与 `<PROMISE>`；资源包同步是同 PR 交付物。

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
| one normative reducer | `INV-04`、`R-01..R-16`、`T-01..T-15` | `D-03,D-04`：所有 scenario 边必须命中一个 reducer row |
| one schema authority | `INV-05`、`P-01..P-15` | `D-01,D-02`：TypeBox-first 全 mirror/all-outcome 对拍 |
| one ownership/cutover DAG | `C-01..C-15`、`C-INV-01..C-INV-07` | `D-10,D-18`：四-arm live reachability + release hash，禁止丢包中间态 |
| one acceptance index | `D-01..D-27` | static lint：acceptance 只引用存在的 `P/R/A/C/I/INV/T` IDs，不复制 transition |

### 9.4 Falsification / rollback clause

本次 rewrite 的首轮正式 review 必须同时满足：

1. **major findings 少于 9 条**；
2. **rewrite delta 不得超过 immutable baseline，且最终文档小于 66KB**：baseline 固定为 rewrite commit `df11b2168b140bd22fc26ec354f2cccf038a2ff6` 相对 parent `aa2c8a48363cba8186cb7cb983aa7b1e13ab8081` 的单文件 `docs/plan-refactor-cast-av-contract-v1.md` patch；可复现 baseline 命令为 `git diff --no-ext-diff --binary df11b2168^ df11b2168 -- docs/plan-refactor-cast-av-contract-v1.md | wc -c`，固定结果 **92186 bytes**。验收时必须测量 **parent-to-submission rewrite delta**，命令为 `git diff --no-ext-diff --binary aa2c8a48363cba8186cb7cb983aa7b1e13ab8081 HEAD -- docs/plan-refactor-cast-av-contract-v1.md | wc -c <= 92186`；不得使用 `df11b2168 HEAD`，后者只测 post-baseline drift。最终文档仍须小于 66KB。

任一失败即视为新架构未证明收敛：回滚本 rewrite commit，恢复诊断前版本，不进入 finding-by-finding 增补。禁止通过降级 finding 严重性、删除规范 evidence 或把内容移到未受审文件规避本条。

### 9.5 Finish Evidence 模板

- 落地清单：按 `C-01..C-15` 列真实路径/symbol。
- 关键 commit：每个 phase 的 hash、日期与 atomic merge invariant。
- 测试结果：按 `D-01..D-27` 列命令、数量、失败修复。
- 跨仓库核验：TypeBox/proto/Rust/Java、real producer、四 router keys、resource-pack hash/size。
- 遗留/后续：只列不在 `P/R/A/C` rows 内的范围，不得把未完成 row 写成后续。
