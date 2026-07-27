# plan-refactor-server-session-v1 — Server 交互 Session 统一生命周期框架（重构轨 R1）

> 所属总纲：`plan-refactor-master-v1.md`。一句话：把 7 套复制粘贴的 server 端交互 session（炼丹/手搓/锻造/采集/灵田/矿脉/灵木）收敛到一个统一的 Session 生命周期框架——断线清理、跨维清理、重启恢复/显式易失声明、忙态互斥、满包产物交付，全部只写一份。

## 现状证据（2026-07-27 侦察）

- 7 个功能域各自手写 session store/状态机：`alchemy/session.rs`、`craft/session.rs`、`forge/session.rs:157-208`、`gathering/session.rs:57`、`lingtian/session.rs:79-339`（6 个独立 session struct）、`mineral/session.rs`、`spiritwood/session.rs`。全仓 `SessionManager` 零命中。
- 断线检测唯一手段 `RemovedComponents<Client>` 仅 9 个文件在用，**上述 7 个 session 模块全部不在其中**——玩家断线时进行中 session 不清理。
- `world/dimension_transfer.rs:34` 跨维只改 `EntityLayerId`/Position，不触碰任何 session。
- 唯一较完整的范本是 craft：`CraftSession` 是玩家实体 Component、`tick_craft_sessions` 带 `With<Client>` 过滤、有 `player_craft_sessions` 持久化表 + join 首包 idle 自愈（`network/craft_emit.rs:541,2759`）。其余 6 域既无断线清理也无持久化。

## 接入面

- **进料**：玩家实体（`With<Client>` / `RemovedComponents<Client>`）、`world/dimension_transfer`、R3 的持久化 slice 框架（flush/restore 钩子）、R4 的 gate 中间件（session 开启前置校验）。
- **出料**：各域 session 状态 → 既有 `*_emit.rs` S2C 事件（craft_session_state 等契约不变）；session 终止时的产物/材料返还 → `inventory`（满包走 R10 的统一交付垫层）。
- **共享类型**：新 `server/src/session/`（`trait InteractionSession` + `SessionRegistry`），craft 现有行为是语义基准，不另造平行概念。
- **跨仓库契约**：不改 wire 形状；client 端对应的 store 清理归 R2。
- **qi_physics 锚点**：session 中断/取消涉及已扣真元的返还必须走 `qi_physics::ledger`（对齐 R5，禁止各域自写返还）。

## 阶段

- ⬜ P0 设计收口 + 吸收清单验真：逐个复读吸收清单里的 skeleton，确认仍是真缺陷；冻结 `InteractionSession` trait（生命周期钩子：`on_disconnect` / `on_dimension_change` / `on_shutdown`(接 R3) / `on_reconnect` / busy 互斥语义 / 产物交付语义）；写 §N.1 决议。
- ⬜ P1 框架落地：`server/src/session/` + craft 迁移为第一个宿主（行为不变，bot 场景锁住）；断线/跨维/重启三类生命周期系统统一注册。
- ⬜ P2 迁移批次 A：alchemy、forge、lingtian（含 #1294 在飞 skeleton 对应的 forge/lingtian session 缺陷一并消灭）。
- ⬜ P3 迁移批次 B + 删旧：gathering、mineral、spiritwood、placed-container/tsy 容器占锁；删除各域私有生命周期代码，不留兼容层。
- ⬜ P4 bot 验收 + 归档：新增 bot 场景全绿；被吸收 plan 批量归档（docs-only PR，Finish Evidence 指向本轨 PR + bot 场景）。

## 吸收清单（促升时 P0 逐个验真，短名省略 plan-bughunt- 前缀与 -v1 后缀）

skeleton：craft-close-pause-loss、craft-session-reconnect-lock、placed-container-session-lifecycle-gap、tsy-container-disconnect-lock-leak、tsy-search-extract-concurrent-busy、world-transport-tsy-relog-presence、client-insight-offer-strand（server 侧会话悬挂部分；client 弹窗部分归 R7）、alchemy-furnace-persistence（session 持久化经 R3 钩子）、alchemy-takeback-full-inventory-loss（teardown 顺序；满包交付垫层归 R10）、forge-c2s-session-wiring、bot-handcraft-craft-outcome-timeout；在飞 #1294：forge-outcome-full-inventory-loss、lingtian-session-disconnect-server。

## 文件所有权与边界

- 独占：`server/src/session/`（新）、7 个域的 `session.rs`、`network/craft_emit.rs` 的 session tick 区。
- 不碰：`persistence/mod.rs`（R3 域，经它暴露的钩子接入）、`client_request_handler.rs`（R4 域）、client 一切（R2/R7 域）。
- 依赖：R3 P1（flush/restore 钩子）落地后本轨 P2 才开；P0/P1 可先行。

## bot 验收场景（加入 scripts/bot/scenarios/）

1. `session_disconnect_cleanup`：bot 起炉/开工作台后断线 → 重连 → 断言 session 已清理或正确恢复（按域语义），无幽灵占锁。
2. `session_dimension_transfer`：交互中跨维 → 断言 session 终止 + 材料按规则返还（守恒过 ledger）。
3. `session_restart_recovery`：交互中关服重启 → 断言持久化域恢复、易失域干净终止不丢材料。
4. `session_busy_mutex`：并发发起互斥交互（搜刮中撤离）→ 断言忙态拒绝。
5. `session_full_inventory_delivery`：满包完成 session → 断言产物不丢（联动 R10）。

## 开放问题（pre-P0 收口）

1. session 持久化的粒度：全部入 R3 slice，还是 craft 模式（表 + join 自愈）推广？
2. 中断返还的统一策略：材料全退 / 按进度折损？涉及 worldview 经济锚点，需人工拍板。
