# plan-bughunt-technique-proficiency-growth-v1（骨架）

> **一句话主题**：`TECHNIQUE_DEFINITIONS`（`server/src/cultivation/known_techniques.rs:158`，49 招）中有 gameplay 熟练度增长路径的只有 7 招（sword×4 / shield_block / movement.dash / body.guangbo_ticao）加 woliu 的 v2 子集，**其余 30+ 招**的 proficiency 只有 dev 命令能改，而 `TechniquesSnapshotV1` 照样向玩家展示 p 值与 proficiency_label——一根永不动的进度条；同时熟练度提升反馈 payload `TechniqueProficiencyUpdate` 双端管道齐备却**服务端从不发射**，连在涨的那几招玩家也毫无感知。本 plan 以**逐 technique_id** 口径补齐「涨」与「看得见涨」两条腿（原审查报告的"十三家流派"是按前缀粗计，本 plan 一律以下方逐 id 普查表为准）。

> 来源：technique 流派系统专项审查（2026-07-26，条目 M5 / M4；M3 practice_session 已划界给 [[plan-dazuo-v1]]）。骨架（草案），只记录缺陷与修复骨架，不含实施。

## 阶段总览

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 逐 technique_id 增长接线 + snapshot 节流（**原子交付，不得拆分合并**） | ⬜ |
| P1 | 熟练度提升 S2C 反馈发射 + 客户端面板一致性契约 | ⬜ |
| P2 | 防回归：逐 id wiring guard + 负向契约测试 + e2e | ⬜ |

> **阶段原子性硬约束**：P0 的增长接线与 snapshot 节流是同一不可拆分交付物——接线放大 `Changed<KnownTechniques>` 全量重发流量（49 条含 description 全文的条目逐击序列化），**禁止只合入增长写入而不带节流**；P0 验收含高频命中负载测试（连续 N 次命中不逐击发送全量 snapshot）。

## 权威范围普查（TECHNIQUE_DEFINITIONS 逐前缀，49 招 = 14 前缀）

数据源：`grep -oE 'id: "[a-z_0-9.]+"' server/src/cultivation/known_techniques.rs`（转 active 前须重跑一遍并展开为**逐 id** 附表，防 drift）：

| 前缀 | 招数 | 现有增长路径 | 状态 |
|------|------|--------------|------|
| woliu | 11 | `track_woliu_proficiency_from_casts`（technique_proficiency.rs:132，消费 `VortexCastEvent`）——**仅 v2 招式发该事件**；v1 vortex 施放不发（`combat/woliu.rs` 零 proficiency 引用） | 部分冻结 |
| anqi | 6 | 无 | 冻结 |
| zhenmai | 5 | 无 | 冻结 |
| sword_path | 5 | 无 | 冻结 |
| sword | 4 | `combat/sword_basics.rs`（命中 +0.008） | 活 |
| burst_meridian | 4 | 无 | 冻结 |
| tuike | 3 | 无 | 冻结 |
| npc | 3 | 无（NPC 专属，见开放问题 #2，倾向显式 allowlist 排除） | 排除候选 |
| dugu | 2 | 无（dugu v2 五招不在 DEFINITIONS 内——**依赖 [[plan-bughunt-dugu-v2-technique-definition-gap-v1]] 先补 official definition**，补入后自动进入本清单口径） | 冻结+依赖 |
| baomai | 2 | 无 | 冻结 |
| shield_block | 1 | `combat/shield_block.rs` | 活 |
| movement | 1 | `movement/dash_proficiency.rs` | 活 |
| morph | 1 | 无 | 冻结 |
| body | 1 | `combat/body_conditioning.rs`（guangbo_ticao） | 活 |

**验收粒度 = technique_id，不是流派**：P0 完成定义为——上表除显式 allowlist 排除项（npc.\*，及注明依赖阻塞的 dugu v2 条目）外，**每个 id** 都有可核验的生产增长路径；"同流派接通一招"不算数。共享同一 resolver/事件源的招式允许按组接线，但 guard 仍逐 id 断言（见 P2）。

## 接入面

- **进料**：各流派 cast/命中事件（resolver/事件桥既有）；`cultivation::technique_proficiency`（`proficiency_gain`:43、`apply_proficiency_gain`:61，公式已备）；活路径范本见普查表。
- **出料**：`KnownTechniques.proficiency` → `TechniquesSnapshotV1`（`server/src/network/techniques_snapshot_emit.rs:14`，节流后语义见 P0）；**新增内部事件 `TechniqueProficiencyGained`**（唯一权威变化源，见 P1）→ `ServerDataPayloadV1::TechniqueProficiencyUpdate` → client `TechniqueProficiencyUpdateHandler`；练满 → `TechniqueMasteredEvent`（technique_proficiency.rs:171 既有路径；叙事桥归 [[plan-bughunt-technique-feedback-bridge-v1]]）。
- **共享类型 / event**：S2C 侧零新类型——`TechniqueProficiencyUpdateV1`（`server/src/schema/server_data.rs:724`）、proto 转换（`proto_convert.rs:1671`）全已备，只缺发射端；server 内部新增 `TechniqueProficiencyGained { player, technique_id, old, new }` 一个 event（阈值判断需要 old/new，`Changed<KnownTechniques>` 表达不了"哪招变了+旧值"）。
- **跨仓库契约**：server emit → proto → client `TechniqueProficiencyUpdateHandler`；**payload 携带权威新值并更新客户端面板状态模型**（不只 toast，见 P1 一致性契约）。
- **worldview 锚点**：worldview.md L305（功法长期特化/真元染色叙事——熟练度是功法掌握度的机制面）。
- **qi_physics 锚点**：熟练度增长本身不动真元；**增长事件必须消费 qi_physics ledger 成功提交之后的 cast-committed / hit-confirmed 事件**（不挂 attempt、不挂扣费前），沿用各流派既有守恒路径，零新增 qi 公式/常数。

## P0 — 逐 id 增长接线 + snapshot 节流（原子交付）⬜

**证据（M5）**：普查表中"冻结"各行——经残卷学到手 0.0 后永远 0.0，只有 `/technique proficiency` dev 命令能改。触发场景：玩家苦练某流派几百次施放，功法面板 p 值纹丝不动。

**交付物 A —— 增长接线**：
- 每个非排除 technique_id 接到其流派 **ledger 成功提交后**的权威事件（cast-committed / hit-confirmed，逐流派在转 active 时按开放问题 #4 定口径并写死落点）→ `apply_proficiency_gain`
- **统一提交点**：所有增长写入（含既有五家迁移）一律经 `apply_proficiency_gain` 返回 (old, new) → emit `TechniqueProficiencyGained`；禁止散落直写 `KnownTechniques.proficiency`
- woliu v1 vortex：统一进 `VortexCastEvent` 或单独接线（择一，落点写进 plan 更新）
- 幂等：同一 cast/hit 事件不得重复记账（事件消费语义/幂等键写死并测试）

**交付物 B —— snapshot 节流（与 A 同 PR，不得拆）**：
- `techniques_snapshot_emit.rs:14` 熟练度微变不再触发全量 snapshot（差分或脏位掩码，具体机制实施定）；**全量 snapshot 的确定性触发器保留并写明**：join/重连、功法增删、开面板请求（如无既有请求链则新增 C2S）——这是 P1 一致性契约的兜底面
- 负载测试（**P0 口径只断言 snapshot 抑制**）：连续 N 次命中不逐击触发全量 snapshot、全量序列化次数有明确上界；「只产生阈值 payload」的断言属 P1（阈值 emitter 在 P1 交付，P1 依赖 P0 的 `TechniqueProficiencyGained` 内部事件），P0 不引用 P1 才存在的输出

**验收**：逐 id「权威事件 → proficiency 上升恰好一次」集成测试（共享事件源的组可参数化）；上限 clamp；练满 `TechniqueMasteredEvent`；负载测试绿。

## P1 — S2C 反馈发射 + 面板一致性契约（M4）⬜

**证据**：全 server 无一处构造 `ServerDataPayloadV1::TechniqueProficiencyUpdate`（`server_data.rs:3932/4998`、`proto_convert.rs:7269` 均为 schema 内部转换/样例桩，`network/agent_bridge.rs:219` 只是类型名映射表）；client `TechniqueProficiencyUpdateHandler` + toast 已存在。触发场景：任何熟练度增长玩家都收不到「XX 熟练度 N%」提示。

**交付物**：
- 发射 system：消费 `TechniqueProficiencyGained`（P0 的唯一权威源，自带 old/new，阈值判断不需要前值缓存）→ 跨整数百分比阈值才发、**定向发给该玩家**；注册点写死（network emit 层既有 schedule，实施时落点写进 plan 更新）
- **客户端一致性契约（唯一权威同步协议）**：阈值 payload **携带权威新 proficiency 值**；client handler 除 toast 外**同步更新本地功法面板状态模型**——面板打开期间的增量以阈值 payload 为准（分辨率=整数百分比），全量以 P0 的确定性 snapshot 触发器（join/重连/功法增删/开面板）为准；阈值以下变化允许面板暂留旧值、开面板/重连必收敛，最大陈旧窗口 = 距上次阈值 <1 个百分点
- **增量合并规则（乱序收敛的成立前提）**：gameplay 熟练度单调不减（唯一合法降值路径 = dev 命令/重置，**不走增量、只走全量 snapshot 覆盖语义**）→ client 对阈值 payload 取 `max(local, incoming)`，旧包晚到天然不回退；全量 snapshot 为覆盖语义，可承载合法降值
- 测试：未跨阈值不发；恰好跨阈值发一次；一次增长跨多个阈值只发最新值；乱序/重复 payload 按 max 合并不回退（含「新值先到、旧值后到」显式用例）；面板常驻打开时连续增长的显示收敛；重连后 snapshot 收敛。dev 降值（reset）与 max 合并的时序竞态见开放问题 #5，收口前不写死该场景断言

## P2 — 防回归 ⬜

- **逐 id wiring guard**：遍历 `TECHNIQUE_DEFINITIONS` 每个 id，断言其存在生产增长路径映射（allowlist 显式排除 npc.\* + 理由；依赖阻塞项显式标注）；新增 definition 未登记增长路径时 guard 必须红
- **负向契约测试（增长不得发生）**：真元不足/学习门拒绝/施法取消不增长；命中制流派 miss 不增长；未拥有该功法不增长；同一事件重复消费不重复记账——每类至少一条，命中制/施放制各覆盖
- 修正误导测试（审查 m7）：`technique_proficiency.rs:470/495` `dash_applies_proficiency_scalars` / `beng_quan_applies_proficiency_scalars` 只断言纯函数值——dash 实际走 `dash_proficiency.rs` 自己的曲线、beng_quan 无任何缩放；改名或补真实接线断言
- e2e：bot 对至少一个新接线流派「施放 N 次 → 收到阈值 payload → 开面板 snapshot p 值收敛」（snapshot 触发器 = 开面板请求，确定性）

## 划界（不在本 plan）

- **打坐/练功会话增长**（审查 M3）：`practice_session` 模块接活 + `*current_qi -= cost` 守恒还账，归 [[plan-dazuo-v1]] P2（reminder.md 登记条目已由其认领）；本 plan 只管战斗/施放路径增长。
- **Redis/天道叙事桥**（learned/mastered narration、`TECHNIQUE_PROFICIENCY_UP` 通道）：归 [[plan-bughunt-technique-feedback-bridge-v1]]。
- **dugu v2 五招 official definition**：归 [[plan-bughunt-dugu-v2-technique-definition-gap-v1]]（active）；其落地后新增 id 自动进入本 plan 逐 id 口径与 guard。

## 开放问题（P0 决策门前需收口）

- **#1 各流派增长速率数值**：范本 sword 每命中 +0.008；转 active 前按 docs/CLAUDE.md §五 用 Explore 收口逐流派数值，不拍脑袋。
- **#2 npc.\* 前缀 3 招是否显式排除**：疑似 NPC 专属，不该出现在玩家增长清单与玩家 snapshot；顺带处理 snapshot 对未知 id 的静默丢弃（审查 m4，`techniques_snapshot_emit.rs:44-47` filter_map 丢弃 + DB 永久占位）。
- **#3 `known_woliu_proficiency` fallback 0.5 与习得初值 0.0 不一致**（审查 m2，`combat/woliu_v2/skills.rs:1618` `unwrap_or(0.5)`——无 entry 的施放者比刚学会的更强）：口径统一并入本 plan P0 还是单独小修。
- **#4 逐流派定「施放成功」还是「命中确认」口径**：挂施放会开对空气白挥刷熟练度的口子；范本中 sword 挂命中、dash 挂动作完成。口径决议须同时定**记账单位**（每 cast / 每命中目标 / 每段 / 每持续 tick）与幂等键（cast_id/hit_id），AOE 多目标、多段、持续伤害的期望增量随之写死。转 active 时逐流派收口（本条收口后 P0 交付物 A 的"权威事件"列才可实施）。
- **#5（review r3）max 合并与合法降值（dev reset）的时序竞态**：旧增量包晚于降值 snapshot 到达会按 max 回写——引入 generation/epoch 版本协议，还是接受 dev-only 边界不设防——转 active 前决议。
- **#6（review r3）P0→P1 间隔期常驻打开面板的陈旧窗口**：P0 抑制逐击重发但保留确定性触发器（join/重连/功法增删/开面板），过渡期常驻面板比现状陈旧——接受过渡窗口，还是 P0/P1 同 PR 合入——转 active 前决议。
- **#7（review r3）增长写入与 `TechniqueProficiencyGained` 发射的封装粒度**：P0 已定统一提交点（`apply_proficiency_gain` 唯一入口 + 禁止散落直写）；是否进一步私有化直写入口、clamp 内聚到提交 API——转 active 前决议。
- **#8（review r3）逐 id wiring guard 与生产注册级验证的强度取舍**：P2 已含注册表级必红 guard；是否升级为逐事件源家族从生产 app 驱动的运行时可达性测试——转 active 前决议。
