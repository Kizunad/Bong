# plan-bughunt-technique-proficiency-growth-v1（骨架）

> **一句话主题**：`TECHNIQUE_DEFINITIONS` 49 招里只有五家流派有 gameplay 熟练度增长路径，其余约 13 个流派的 proficiency **只有 dev 命令能改**，而 `TechniquesSnapshotV1` 照样向玩家展示 p 值与 proficiency_label——一根永不动的进度条；同时熟练度提升反馈 payload `TechniqueProficiencyUpdate` 双端管道齐备却**服务端从不发射**，连在涨的那五家玩家也毫无感知。本 plan 补齐「涨」与「看得见涨」两条腿。

> 来源：technique 流派系统专项审查（2026-07-26，条目 M5 / M4；M3 practice_session 已划界给 [[plan-dazuo-v1]]）。骨架（草案），只记录缺陷与修复骨架，不含实施。

## 阶段总览

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 十三家流派熟练度增长接线（M5） | ⬜ |
| P1 | 熟练度提升 S2C 反馈发射 + snapshot 节流（M4） | ⬜ |
| P2 | 防回归 wiring guard + 误导测试修正 | ⬜ |

## 接入面

- **进料**：各流派 cast/命中事件（resolver/事件桥既有）；`cultivation::technique_proficiency`（`proficiency_gain`:43、`apply_proficiency_gain`:61，公式已备）。五家活范本：`track_woliu_proficiency_from_casts`（technique_proficiency.rs:132，消费 `VortexCastEvent`）、`combat/sword_basics.rs`、`combat/shield_block.rs`、`combat/body_conditioning.rs`、`movement/dash_proficiency.rs`。
- **出料**：`KnownTechniques.proficiency` → `TechniquesSnapshotV1`（`server/src/network/techniques_snapshot_emit.rs:14`）；新增发射 `ServerDataPayloadV1::TechniqueProficiencyUpdate` → client `TechniqueProficiencyUpdateHandler`（已存在，等接）；练满 → `TechniqueMasteredEvent`（叙事桥归 [[plan-bughunt-technique-feedback-bridge-v1]]）。
- **共享类型 / event**：**零新类型**——`TechniqueProficiencyUpdateV1`（`server/src/schema/server_data.rs:724`）、proto 转换（`proto_convert.rs:1671`）全已备，只缺发射端。
- **跨仓库契约**：server emit → proto → client `TechniqueProficiencyUpdateHandler` toast；`TechniquesSnapshotV1` 面板同步。
- **worldview 锚点**：worldview.md L305（功法长期特化/真元染色叙事——熟练度是功法掌握度的机制面）。
- **qi_physics 锚点**：熟练度增长本身不动真元；各流派 cast 的 qi 流动沿用既有守恒路径，**零新增 qi 公式/常数**。

## P0 — 十三家流派熟练度增长接线（M5）⬜

**证据**：全仓 proficiency 生产写入点只有五家（见接入面）；以下流派经残卷学到手 0.0 后永远 0.0，只有 `/technique proficiency` dev 命令能改。触发场景：玩家苦练某流派几百次施放，功法面板 p 值纹丝不动。

逐流派清单（增长事件源落点）：

| 流派 | 招式数 | 代码落点 |
|------|--------|----------|
| burst_meridian | 4 | `server/src/cultivation/burst_meridian.rs` |
| baomai | 2 | `server/src/combat/baomai_v3/` |
| zhenmai | 5 | `server/src/combat/zhenmai_v2.rs` |
| dugu v1 | — | `server/src/cultivation/dugu.rs` |
| dugu v2 | 5 | `server/src/combat/dugu_v2/`（**依赖 [[plan-bughunt-dugu-v2-technique-definition-gap-v1]] 先补 official definition**） |
| tuike | 3 | `server/src/combat/tuike.rs` + `tuike_v2/` |
| anqi | 6 | `server/src/combat/anqi_v2.rs` 等 |
| sword_path | 5 | `server/src/sword_path/` |
| morph | — | 变化系 resolver |
| woliu.vortex v1 | — | `server/src/combat/woliu.rs`（施放不发 `VortexCastEvent`，现增长管道只覆盖 v2——统一进 `VortexCastEvent` 或单独接线） |

**验收**：每流派至少一条「施放/命中 → proficiency 上升」集成测试；上限 clamp；练满触发 `TechniqueMasteredEvent`（technique_proficiency.rs:171 既有路径）。

## P1 — S2C 反馈发射 + snapshot 节流（M4）⬜

**证据**：全 server 无一处构造 `ServerDataPayloadV1::TechniqueProficiencyUpdate`（`server_data.rs:3932/4998`、`proto_convert.rs:7269` 均为 schema 内部转换/样例桩，`network/agent_bridge.rs:219` 只是类型名映射表）；client `TechniqueProficiencyUpdateHandler` + 提升 toast 已存在。触发场景：任何熟练度增长玩家都收不到「XX 熟练度 N%」提示。

**交付物**：
- 发射 system：熟练度跨越整数百分比阈值才发（防高频战斗每击 +0.008 刷屏），测试锁 payload 结构 + 阈值行为 + 不跨阈值不发
- **snapshot 节流（P0 的直接后果，必须一起做）**：`techniques_snapshot_emit.rs:14` 走 `Changed<KnownTechniques>` 全量重发——现状下 sword 每次命中都全量序列化 49 条 description 下发；P0 让 13 家开始涨后该流量会同比例放大，须改为熟练度微变不触发全量 snapshot（差分或位掩码脏标记），面板一致性由阈值 payload + 定期/开面板时 snapshot 兜底

## P2 — 防回归 ⬜

- wiring guard 清单测试：`TECHNIQUE_DEFINITIONS` 中每个玩家可学流派（npc.* 除外，见开放问题 #2）至少存在一个生产增长调用方
- 修正误导测试（审查 m7）：`technique_proficiency.rs:470/495` `dash_applies_proficiency_scalars` / `beng_quan_applies_proficiency_scalars` 只断言纯函数值——dash 实际走 `dash_proficiency.rs` 自己的曲线、beng_quan 无任何缩放；改名或补真实接线断言
- e2e：bot 场景对至少一个新接线流派走「施放 N 次 → snapshot p 值上升 + 收到阈值 payload」

## 划界（不在本 plan）

- **打坐/练功会话增长**（审查 M3）：`practice_session` 模块接活 + `*current_qi -= cost` 守恒还账，归 [[plan-dazuo-v1]] P2（reminder.md 登记条目已由其认领）；本 plan 只管战斗/施放路径增长。
- **Redis/天道叙事桥**（learned/mastered narration、`TECHNIQUE_PROFICIENCY_UP` 通道）：归 [[plan-bughunt-technique-feedback-bridge-v1]]。

## 开放问题（P0 决策门前需收口）

- **#1 各流派增长速率数值**：范本 sword 每命中 +0.008；转 active 前按 docs/CLAUDE.md §五 用 Explore 收口逐流派数值，不拍脑袋。
- **#2 npc.* 前缀流派是否参与玩家熟练度体系**：疑似 NPC 专属，不该出现在玩家增长清单；顺带处理 snapshot 对未知 id 的静默丢弃（审查 m4，`techniques_snapshot_emit.rs:44-47` filter_map 丢弃 + DB 永久占位）。
- **#3 `known_woliu_proficiency` fallback 0.5 与习得初值 0.0 不一致**（审查 m2，`combat/woliu_v2/skills.rs:1618` `unwrap_or(0.5)`——无 entry 的施放者比刚学会的更强）：口径统一并入本 plan P0 还是单独小修。
- **#4 增长挂「施放」还是「命中」**：挂施放会开对空气白挥刷熟练度的口子；范本五家中 sword 挂命中、dash 挂动作完成，逐流派定口径。
