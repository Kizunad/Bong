# plan-inventory-hint-panel-v1 — 库存操作拒绝原因警示面板

**主题**：背包/装备界面里很多操作被 server 静默拒绝（拖入弹回），玩家不知道**为什么**。把 server 侧现在只写进 WARN 日志、从不下发 client 的拒绝原因（境界不足 / worn cap 满 / 手持互斥 / 双手锁定 / 分类不符 等）结构化后下发，用**失败 toast + tab/槽 hover 预警**两条通道显式告诉玩家。源于真机：拖伪皮进胸槽弹回（`realm too low for SpiderSilk`）全是静默回弹，体验差。

> **本 plan 性质**：纯 client UX + 一条 server→client 拒绝原因下发链路。无 worldview 玩法锚点（UI 辅助），无 qi_physics、无守恒。文案里的境界名走正典映射。

## 阶段总览

| 相位 | 交付物 | 状态 |
|------|--------|------|
| **P0** | server 拒绝原因结构化 —— `InventoryMoveRejectReason` enum + `apply_inventory_move`/`validate_move_semantics` 收敛 + 境界门控并入 + 复用 `EventAlert` 下发 | ⬜ |
| **P1** | client 失败 toast —— `EventAlertHandler` → `BongToast.show` 飘红 + 文案表（境界名走 `RealmLabel`） | ⬜ |
| **P2** | tab/槽 hover 预警 —— 照抄 `ItemTooltipPanel`+`updateTooltipFromHover`，只覆盖静态规则 | ⬜ |
| **P3** | 视听规格收尾 —— toast 时长/颜色、hover 面板位置/配色/淡入淡出、不遮挡拖拽 ghost | ⬜ |

验收日期：全相位 ✅ 后填。

## 接入面（跨仓库契约，已坐实）

- **进料**：
  - server `server/src/inventory/mod.rs` 的拒绝路径 —— `validate_move_semantics`（`mod.rs:4664`，现返回 `Result<(), String>`）/ `apply_inventory_move`（`mod.rs:3158`，现返回 `Result<InventoryMoveOutcome, String>`）；伪皮胸槽境界门控是**另一条独立硬编码**路径（`network/client_request_handler.rs:9896-9925`，命中只 `tracing::warn!` + `resync_snapshot`，连 `Err` 都不走）。
  - client tab/槽 hover —— 逐帧轮询 `InspectScreen.updateTooltipFromHover`（`InspectScreen.java:3991-4021`），owo `mouseEnter().subscribe()` 也可用（`InspectScreen.java:1631-1633` 有拖拽悬停切 tab 先例）。
- **出料**：
  - 复用现成通用事件 payload `ServerDataPayloadV1::EventAlert`（`schema/server_data.rs:308-313` / wire `:1167-1174`），client 侧 `BongEventAlertOverlay.java` + `EventAlertHandler.java` 全链现成。P1 不新增 proto message。
  - client 浮层复用 `hud/BongToast.java`（`show(...)`，`:72-77`，`SYSTEM_WARNING` 红色 "天道警示：" 前缀 `:160-163`）+ `inventory/component/ItemTooltipPanel.java`（hover 面板模板）。
- **共享类型 / 复用**：拒绝原因先例——`ProbeDenialReason`（`shelflife/probe.rs:60-69`）/ `MineralProbeDenialReason`（`mineral/events.rs`）/ `CastRejectReason`（`cultivation/skill_registry.rs:20-34`），三条都走 "server 内部 typed enum + 格式化文案下发" 模式，`EventAlert` 已是其中两条的落地载体。新 `InventoryMoveRejectReason` 仿 `CastRejectReason` 结构，**不复用**这几个 enum（语境不同）但抄其模式。
- **跨仓库契约 symbol**：`InventoryMoveRejectReason`（server 内部）；`EventAlert` wire payload（已存在，P1 复用）；client `EventAlertHandler` / `BongToast` / `RealmLabel.displayName`。境界 tag 跨端：server `realm_to_string`（`schema/cultivation.rs:321-330`，输出 `"Condense"` 等英文 tag）↔ client `RealmLabel.displayName`（`util/RealmLabel.java:9-26`，英文 tag → 醒灵/引气/凝脉/固元/通灵/化虚）。
- **worldview 锚点**：无玩法正典依赖（UI 辅助）。按需显示、不常驻（[[feedback_hud_immersive_minimal]] / [[feedback_hud_conditional]]）；文案境界名用正典（[[feedback_worldview_canonical]]）。
- **qi_physics 锚点**：无（纯 UI + 一条拒绝原因下发，不碰真元/守恒）。
- **红旗自检**：不自产自消（接 server 拒绝路径 + client HUD）；不新增守恒常数；跨端只加 server enum + 复用既有 wire payload。

---

## P0 — server 拒绝原因结构化 ⬜

**目标**：把当前散落成裸 `String` + 只进日志的拒绝原因收敛成一个 typed enum，并下发 client。

**交付物 / 抓手**：
- 新增 `server/src/inventory/` 内 `enum InventoryMoveRejectReason`（仿 `CastRejectReason` @ `cultivation/skill_registry.rs:20-34`），变体覆盖实测拒绝分支（`mod.rs` 内）：
  - `WornStackNotTop`（`mod.rs:4693`）、`ForbiddenInHotbar{category}`（`mod.rs:4708-4746` 6 类）、`PackDetached`（`mod.rs:4777`）、`HeldWornMismatch`（`mod.rs:4815`）、`EquipCategoryMismatch`/`OffHandTypeMismatch`（`mod.rs:4842/4851`）、`HandOccupied`/`TwoHandedLocksOther`（`mod.rs:4866/4883/4903`）、`ArmorDurabilityZero`（`mod.rs:4923`）、`ArmorSlotMismatch`（`mod.rs:4938`）、`PackEquipSlotMismatch`（`mod.rs:4950`）、`WornCapFull{slot, cap}`（`mod.rs:4967`）、`TargetOutOfBounds`/`TargetOccupied`（`mod.rs:4535/4614/4623` 等）
  - `RealmTooLow{required_realm}` —— **把 `client_request_handler.rs:9896-9925` 那条独立硬编码境界门控并入**（目前最大技术债：它连 `Result` 都不走）。`required_realm` 存 `realm_to_string` 英文 tag（如 `"Condense"`）。
- `validate_move_semantics` / `apply_inventory_move` 的 `Result<_, String>` 收敛成 `Result<_, InventoryMoveRejectReason>`（`?` 传播链一并改）。保留一个 `reason.to_log_string()` 供既有 `tracing::warn!` 继续打日志（不丢日志能力）。
- 拒绝下发：在 `client_request_handler.rs` 的 `Err(reason)` 分支（`:10105-10119`）+ false_skin 境界门控分支（`:9914`）旁，除 `resync_snapshot` 外**多 emit 一条 `ServerDataPayloadV1::EventAlert`**（`event` 用一个 inventory-reject 语义的 `EventKind`；`message` 先给英文 reason tag 或占位，真正中文文案交 client 侧 `RealmLabel`/文案表；`zone: None`；`duration_ticks: Some(~60)`）。
- **测试声明**：`inventory::` 每个 `InventoryMoveRejectReason` 变体一条专属 case（构造触发条件 → 断言 `apply_inventory_move` 返回该变体）；下发链一条：拒绝 → 断言 emit 了带正确 `event`/`required_realm` 的 `EventAlert`（契约测：断 payload 结构，不绑内部调用次数）。境界门控并入后：伪皮胸槽拒绝 → 断言走 enum 而非旧 warn-only 路径。

---

## P1 — client 失败 toast ⬜

**目标**：拒绝发生时飘一条红色警示,把原因用正典语感文案告诉玩家。

**交付物 / 抓手**：
- client `EventAlertHandler.java` 消费 P0 下发的 inventory-reject `EventAlert` → 调 `BongToast.show(text, SYSTEM_WARNING 色, now, ~3000ms)`（`hud/BongToast.java:72-77,160-163`）。或直接走已有 `BongEventAlertOverlay` 右上 banner——二选一在 P3 视听定，P1 先接通 toast。
- 文案表覆盖全部 reject 变体：worn cap 满 / 手持互斥 / 双手锁定 / 分类不符 / 护甲耐久 0 / **境界不足**。
- **境界名走正典**：`RealmTooLow{required_realm}` 的文案用 `RealmLabel.displayName(required_realm)`（`util/RealmLabel.java:9-26`）现场把英文 tag 转 "凝脉" 等中文,**不在 server 端硬编中文**（避免 server/client 两处措辞漂移；`freshness_probe_emit.rs:101` 那种 server 硬编中文是历史写法，新代码反向交 client 统一映射）。
- **测试声明**：client 单测——每个 reason → 断言 toast 文案正确（尤其境界不足 → "……凝脉……" 而非英文 tag）；契约测断可观察文案,不绑渲染内部。

---

## P2 — tab/槽 hover 预警 ⬜

**目标**：hover inventory tab / 装备槽时主动显示该容器/槽约束,不必等失败才提示。

**交付物 / 抓手**：
- 照抄 `ItemTooltipPanel`（`inventory/component/ItemTooltipPanel.java`，固定宽 196px、`computeRequiredHeight` 动态高）+ `updateTooltipFromHover`（`InspectScreen.java:3991-4021`）的轮询式 hover 检测框架，新增一块 "约束说明" 区：hover 装备槽 → 显示 cap（`worn_cap`）、可装类别、当前境界能否装。
- **边界（关键，防 hover 说谎）**：P2 hover **只覆盖静态规则**——`worn_cap` 是纯常量（`mod.rs:636-643`，Head/Feet=2、Chest/Legs=3），可 client 镜像一份；但 `worn_cap_bonus`（`mod.rs:659-661`，P5 占位，未来由境界/功法派生）一旦启用无法纯 client 推算 → 动态加成类拒绝**仍走 P1 toast**，hover 文案不承诺动态结果。plan 内注明此边界。
- 避 owo `Sizing.fill(100)` 顶飞（[[feedback_owo_fill_overflow]]）：面板走绝对定位 / `Positioning.relative`（`EquipmentPanel.java:16-36` 已因这条教训改绝对坐标表 `SLOT_LAYOUT`）。
- **测试声明**：client 单测——hover 各槽 → 断言约束文案正确;worn_cap 满的槽 hover → 显示 "已满" 且与 P1 toast 文案一致。

---

## P3 — 视听规格收尾 ⬜

**HUD / 屏幕效果规格**（内联，达可实现精度）：
- **失败 toast**：复用 `BongToast` `SYSTEM_WARNING`（红色 `0xFFCC3333` 底 + "天道警示：" 前缀，`BongToast.java:160-163`），`durationMillis=3000`，右上/屏中 fade-in 6 tick / fade-out 10 tick（沿用 BongToast 现有曲线）。同一 reason 500ms 内去重,防连续拒绝刷屏。
- **hover 约束面板**：`ItemTooltipPanel` 风格,宽 196px,背景 `0xE0141414`,1px 边框 `0xFF3A3A3A`;约束逐行——满足=绿 `0xFF66BB66`,不满足=红 `0xFFCC5555`;绝对定位贴 hover 目标右侧,超右边界翻转到左侧;**不遮挡拖拽 ghost**（拖拽进行中隐藏 hover 面板）。fade-in 4 tick。
- **narration**：无（纯 UI 提示,不走天道 narration）。

**测试声明**：视听为 client 渲染,以 P1/P2 的文案/结构契约测为准;P3 只调参不新增可断言逻辑。

---

## §8 开放问题（P0 决策门前需收口）

1. 警示走「失败后 toast」还是「hover 预警」还是两者都要？（P1 vs P2 取舍）
2. reject reason 是新 S2C payload 还是塞 inventory_snapshot？
3. 文案与 worldview 语感对齐（境界名用正典：醒灵/引气/…）。

> 全部已在 §8.1 收口。原表保留以备追溯，**实施时以 §8.1 决议为准**。

## §8.1 决议（pre-P0 收口，2026-07-02，靠 Explore agent 实地核查代码产出）

### #1 警示走 toast 还是 hover 还是两者都要？

**决议**：
1. **两者都要,分 P 且优先级不同**。P1 失败 toast 优先——拒绝已真实发生（真机症状 "静默回弹"）,直接复用 `BongToast.show(...)` 零新增渲染,成本最低收益最直接。
2. P2 tab/槽 hover 预警作增强,照抄 `ItemTooltipPanel` + `updateTooltipFromHover` 轮询框架。
3. **边界**：hover 只覆盖静态规则（`worn_cap` 常量）,动态加成类（`worn_cap_bonus`,未来境界/功法派生）仍走 P1 toast,hover 不说谎。

**落点**：`client/.../hud/BongToast.java:72-77`（toast）/ `client/.../inventory/component/ItemTooltipPanel.java` + `InspectScreen.java:3991-4021`（hover）/ `server/src/inventory/mod.rs:636-643,659-661`（cap 常量 vs 动态 bonus 边界）/ plan §P1 §P2。

### #2 reject reason 是新 S2C payload 还是塞 inventory_snapshot？

**决议**：
1. **不塞 `inventory_snapshot`**——`InventorySnapshotV1`（`schema/inventory.rs:268-287`）是幂等全量状态镜像,reason 是一次性瞬时事件,塞进去语义别扭 + 竞态歧义。
2. **复用已有 `EventAlert`**（`schema/server_data.rs:308-313`）,不另起炉灶。仓库已有三条 "拒绝原因 payload" 先例（`ProbeDenialReason`/`MineralProbeDenialReason`/`CastRejectReason`）全走此模式,`EventAlert` 已是其中两条的落地载体,client `BongEventAlertOverlay`/`EventAlertHandler` 现成。
3. **唯一新增**：server 内部 `InventoryMoveRejectReason` enum（仿 `CastRejectReason`）,把裸 `String` 拒绝路径 + `client_request_handler.rs:9896-9925` 独立硬编码境界门控收敛进来,Err 旁多发一条 `EventAlert`（P1 不新增 proto message）。若后续要更精细结构化字段（slot/cap 数值）再单开 `InventoryMoveRejected` payload,成本可控。

**落点**：`server/src/inventory/mod.rs:3158,4664`（收敛 Result）/ `network/client_request_handler.rs:9896-9925,10105-10119`（境界门控并入 + Err 旁 emit）/ `schema/server_data.rs:308-313`（EventAlert 复用）/ plan §P0。

### #3 文案与 worldview 语感对齐

**决议**：
1. **复用现成 `Realm→中文` 链路**,不新造字符串表。server `realm_to_string`（`schema/cultivation.rs:321-330`）输出英文 tag,client `RealmLabel.displayName`（`util/RealmLabel.java:9-26`）转醒灵/引气/凝脉/固元/通灵/化虚。这条链路 `inventory_snapshot.realm` 已在端到端用。
2. `RealmTooLow` 的 reason payload 只带 `required_realm: "Condense"` 英文 tag,client 现场转中文。**不在 server 端硬编中文文案再下发**（避免措辞漂移）。

**落点**：`schema/cultivation.rs:321-330`（realm_to_string）/ `client/.../util/RealmLabel.java:9-26`（displayName）/ plan §P1。

> **过时点勘误（收口时发现）**：原骨架头部例句 "挪非空背包弹回（`container not empty`）" **已过时**——该拒绝分支被 `plan-tarkov-backpack-v1` P0 删除（`mod.rs:4700-4706` 注释 "此处不再因 pack 容器非空返回 Err"）。本 plan 不含该 reason 变体。

---

## §10 实施工作流

scope ~3 PR，单 plan 内序列化（`docs/CLAUDE.md` §六）。

- **§10.1 推荐拆分点**（依赖顺序，前一个 merge 后开下一个）：
  1. **PR-1 P0**：server 拒绝原因结构化（`InventoryMoveRejectReason` + 收敛 Result + 境界门控并入 + `EventAlert` 下发）。纯 server + 复用既有 wire,独立可 merge。
  2. **PR-2 P1**：client 失败 toast + 文案表（含境界名 `RealmLabel` 转换）。依赖 PR-1 的下发链。
  3. **PR-3 P2+P3**：hover 预警 + 视听规格收尾。依赖 PR-2 的文案表。
- **§10.2 撞车防护**：`fix/inventory-tab-live-refresh`（过期重复分支，功能已由 PR #775 落地）、`inventory-v1`/`atlas/inventory-v1-server-data-closure`（相对 origin/main 零独有提交，死分支）均**不构成冲突**（收口调研已核实）。每 PR 开前仍 `git fetch origin && git log origin/main` 比对 `inventory/mod.rs` / `InspectScreen.java` 是否被动过。
- **§10.3 测试要求**：P0 每个 reject 变体专属 case + 下发契约测；P1/P2 client 文案契约测（断可观察文案,不绑渲染内部）。饱和覆盖：每个 enum 变体、境界不足特例（转中文）、worn_cap 满边界。
- **§10.4 CR 等待**：每 PR `ScheduleWakeup` 1200s × ≤3 回合等 CodeRabbit（[[feedback_wait_coderabbit_approve]]）,修完重等 re-review。
- **§10.5 subagent 实施**：每 PR 独立 `claude` subagent（opus + `ultrathink`）,主线只收 result + merge（`docs/CLAUDE.md` §6.4）。
- **§10.6 单次 consume 全自动到 merge**：收口已完成（本 §8.1）,`/consume-plan` 即可,醒来看是否入 `finished_plans/`。

## 落地证据链

- 收口调研（2026-07-02，Explore agent 实地核查）：server 拒绝路径 `inventory/mod.rs` + 境界门控 `client_request_handler.rs:9896-9925`;下发先例 `EventAlert`/`ProbeDenialReason`/`CastRejectReason`;client 浮层 `BongToast`/`ItemTooltipPanel`/`updateTooltipFromHover`;境界映射 `realm_to_string`↔`RealmLabel.displayName`。
- 相关先例 plan：`docs/finished_plans/plan-tarkov-backpack-v1.md`（删了 container-not-empty 拒绝）;`plan-shelflife-v1`/`plan-mineral-*`（probe denial reason 下发先例）。
