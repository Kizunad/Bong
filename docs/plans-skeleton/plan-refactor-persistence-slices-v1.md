# plan-refactor-persistence-slices-v1 — 玩家/世界状态持久化 Slice 框架 + persistence 巨石拆分（重构轨 R3）

> 所属总纲：`plan-refactor-master-v1.md`。一句话：拆掉 16324 行的 `persistence/mod.rs` 巨石，建统一的持久化 Slice 框架——载入失败守护（绝不空表覆盖）、关服强制 flush registry、相对 tick 基准、autosave 竞态互斥——把"重启丢档/断线丢档/载入清零"整簇（20+ 份 plan）从根上消灭。

## 现状证据（2026-07-27 侦察）

- `persistence/mod.rs` 单文件 16324 行、53 处 `CREATE TABLE`、v1-v39 手写迁移链；`register(app)` 只挂 5 个系统，玩家数据另走 `player/mod.rs` 的分片 autosave（4 个 `autosave_player_*`）。
- 覆盖参差：craft/mineral/spiritwood/zone/npc 有表；alchemy/forge/gathering/lingtian session、ActiveEventsResource、TiandaoAttention、状态 buff、化虚冷却等纯内存重启即丢。
- 已确认的同构缺陷族（在飞 PR 群）：#1288 KnownTechniques 载入失败空表覆盖丢档、#1289 Lifecycle 从未持久化（重连清空濒死后果）、#1282 Wounds 重连满血、#1290 呼吁推广载入守护到所有 slice——这是"每个 slice 手写一遍、各漏各的"的直接证据。
- 绝对 tick 持久化导致重启漂移：mineral-respawn-tick-restart-drift、voidaction-cooldown-runtime-tick-restart 同构。
- 关服 flush 缺口：recipe-unlock（#1261 在修）、spiritwood、zone-influence 同构。

## 接入面

- **进料**：SQLite（bong.db，沿用）、`shutdown.rs`（#1261 之后的关服链路）、`CultivationClock`（相对 tick 基准）。
- **出料**：统一 Slice API 供各域注册：`load(guarded) / autosave(cadence) / flush_on_shutdown / tick_rebase`；R1 的 session 持久化钩子、各域运行态表全部走它。`tick_rebase` 对 R1 suspension lease 保持相对剩余时长；R1 冻结默认 `SESSION_SUSPENSION_TTL_TICKS = 1_728_000` 与扫描 cadence `1_200` ticks，本轨不得把重启折算成无限续租。
- **共享类型**：新 `server/src/persistence/` 多文件模块（按域拆表定义 + 迁移链保持线性单入口）；`PlayerSliceRegistry`（对齐 #1290 skeleton 的方向，直接吸收它）。
- **跨仓库契约**：零 wire 改动。
- **qi_physics 锚点**：任何带 qi 的快照持久化/恢复不得造成账面变化；恢复失败的兜底路径必须走 `release_dormant_qi_to_zone` 而非丢弃（对齐守恒律红旗清单）。

## 阶段

- ⬜ P0 设计收口 + 吸收清单验真：53 张表普查归域；冻结 Slice trait（载入守护语义：读失败 = 保留旧行 + 告警 + 只读降级，绝不写回空态；flush registry；tick rebase 协议）；等 #1288/#1289/#1261/#1259 merge 后定基线。
- ⬜ P1 框架落地 + 巨石拆分：`persistence/` 按域拆文件（迁移链不变、行为不变）；Slice 框架上线，KnownTechniques/Lifecycle（在飞 PR 的成果）平移为首批宿主；注册 R1 所需的 `TsyPresence` auxiliary Slice，字段固定为 `family_id`、`entered_at_tick`、`return_to`、schema/version，并接入 guarded load、autosave、`flush_on_shutdown`、`tick_rebase`。
- ⬜ P2 载入守护推广：全部玩家 slice（SkillSet/Wounds/状态 buff/身份键……）收编，#1290 模式全量落地；身份主键统一（identity-persist-key-mismatch）。
- ⬜ P3 关服 flush + tick rebase 批次：shutdown flush registry 收编全部"节流落盘"域；绝对 tick 全部改相对基准；autosave/事件写入竞态互斥（coffin-autosave-inflight-race 模式）；`TsyPresence` 的关服 flush 与 presence/position/dimension 同事务顺序固定。
- ⬜ P4 遗漏运行态补持久化批次：ActiveEvents、TiandaoAttention、状态效果、化虚冷却、灵眼、地表遗缴、散灵珠、可放置实体、dormant 往返身份完整性（heiwushi）等——逐个按 Slice 框架补表。加入 `TsyPresence` guarded relog parity 契约测试和 TSY 维度重启 bot 场景，断言失败加载不 attach presence、成功恢复后才重新开放 TSY 请求。
- ⬜ P5 bot 验收 + 吸收 plan 批量归档。

## 吸收清单（短名省略 plan-bughunt- 前缀与 -v1 后缀）

active：active-events-restart-loss、mineral-respawn-tick-restart-drift、realm-taint-restart-amnesia、recipe-unlock-shutdown-flush（若 #1261 已 merge 则只归档）、season-override-restart、spiritwood-shutdown-flush、status-effects-consumable-persistence、supply-coffin-cooldown-restart-rollback、tiandao-attention-persistence、zone-influence-shutdown-flush、dormant-redis-dirty-ack、heiwushi-dormant-identity-loss；round-bundle 精确吸收：r1-mechanical-fixes P6 NPC deceased archive DB-open rollback、r10-findings #1 `mineral::record_exhausted_minerals` shutdown flush。
skeleton：coffin-autosave-inflight-race、identity-persist-key-mismatch、mineral-exhausted-log-corrupt-revival、placeable-entity-restart-loss、scatter-bead-burial-restart-loss、spirit-eye-runtime-persistence、surface-stash-lifecycle-volatile、voidaction-cooldown-runtime-tick-restart、coffin-offline-reclaim-respawn-dup、stale-spirit-niche-lifecycle；在飞：wounds-relog-full-heal（#1282）、player-slice-load-failure-clears（#1290）、shelflife-clock-restart-freshness（#1294）。

## 文件所有权与边界

- 独占：`server/src/persistence/**`、`player/state.rs`+`player/mod.rs` 的 autosave/载入区段、各域的持久化接线点（新增表定义）。
- 不碰：session 业务逻辑（R1 经钩子接入）、qi 语义（R5）、`client_request_handler.rs`（R4）。
- 依赖：基线等 #1288/#1289/#1259/#1261 merge；R1 P2 依赖本轨 P1。**本轨是 Wave 0 的锚，最优先动工。**

## bot 验收场景

1. `restart_player_slices`：bot 建号→修炼/学功法/受伤→关服重启→重连→断言功法/伤势/濒死后果/buff 全部还原。
2. `restart_world_runtime`：触发矿脉枯竭/配方解锁/zone influence→SIGTERM 关服→重启→断言无回滚无复活。
3. `load_failure_guard`：注入一行损坏 slice 数据→启动→断言该玩家进入守护降级而非清零覆盖（配 dev 命令注入）。
4. `tick_rebase`：带冷却/再生倒计时重启→断言倒计时按真实流逝折算（对齐 #1289 的 deadline 折算先例）。
5. `tsy_presence_relog_parity`：进入 TSY→关服 flush→guarded load→只有 `family_id`、`entered_at_tick`、`return_to`、schema/version 校验通过才 attach `TsyPresence` 并开放 TSY 请求；损坏或缺失 Slice 保持未 attach 且拒绝请求。
6. `tsy_presence_shutdown_order`：断言 session registry 静止后才 flush presence slice，且 position/dimension 与 presence 同一恢复事务对拍。

## 开放问题（pre-P0 收口）

1. 载入守护的玩家体验：只读降级 vs 拒绝进服 vs 回滚到上一备份？需人工拍板。
2. 迁移链是否借机做一次 squash（v1-v39 合并基线）？风险与老存档兼容性需评估。
