# Bong · plan-tsy-raceout-v1 · Finished

> **状态**：✅ finished（2026-05-08 落地，前置 plan-tsy-lifecycle-v1 / plan-tsy-extract-v1 / plan-tsy-hostile-v1 / plan-death-lifecycle-v1 / plan-tsy-loot-v1 全 ✅。§4 全部 5 决策闭环（Q-RC1/RC2/RC3/RC4/RC5 详 §4）。详见末段 `## Finish Evidence`。

坍缩渊塌缩 race-out **3 秒**撤离机制。当副本最后一件上古遗物被取走 → 触发塌缩 → 全副本玩家收到 `tsy::CollapseStarted` ✅ → 负压翻倍 + 随机 3-5 个塌缩裂口 + **3 秒**撤离窗口(非标准 7-15s)。**化虚级也可能死在 race-out**。

**世界观锚点**：`worldview.md §十六.六 坍缩渊塌缩`(最后一件遗物被取 → 塌缩 → race-out)

**library 锚点**：`world-0003 诸渊录·卷一·枯木崖`(坍缩渊叙事范例)

**交叉引用**：`plan-tsy-lifecycle-v1` ✅(已支持 collapse 触发) · `plan-tsy-extract-v1` ✅(标准撤离仪式) · `plan-narrative-v1` ⏳(race-out 专属台词) · `plan-gameplay-journey-v1` §M.1

---

## 接入面 Checklist

- **进料**：`tsy::CollapseStarted` event ✅ 已 emit + 副本上古遗物状态
- **出料**：3 秒倒计时 client UI + race-out 专属 narration + 慢一秒变死域的实质后果(真死)
- **共享类型**：复用 `StartExtractRequest` ✅ 但要求 `timeout=3s`(标准是 7-15s)
- **跨仓库契约**：`tsy_collapse_started` + `extract-aborted/completed` schema 已 ✅
- **worldview 锚点**：§十六.六

---

## §0 设计轴心

- [x] **3 秒不是标准撤离**：用同样的 ExtractRequest API 但 timeout 强制 3s（`RiftKind::CollapseTear.base_extract_ticks=60` + `on_tsy_collapse_started` 把所有 portal `current_extract_ticks` 压到 60 — 前置 plan-tsy-extract-v1 落地）
- [x] **化虚级也可能死**：大真元池在塌缩负压下吃亏更大,平衡设计（`LifespanCapTable::death_penalty_years_for_cap = cap/20`，Realm::Void cap 2000 → 100 年扣减；`tsy_collapsed` cause 进 `death_arbiter_tick` 标准流水）
- [x] **专属 narration**：风格台词 "它要塌了。它不在乎你身上还揣着什么。"（`agent/packages/tiandao/src/skills/calamity.md` race-out 章节，4 条种子台词 + 强约束）

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ✅ 2026-05-08 | 3 秒倒计时 client UI + race-out 信号识别 | 玩家在塌缩时见 3 秒红色倒计时 |
| **P1** ✅ 2026-05-08 | 3-5 个随机塌缩裂口生成(独立位面内) + Q-RC4 单 portal 单 player | 全副本玩家可向最近裂口跑 |
| **P2** ✅ 2026-05-08 | 慢一秒 → 副本化为死域(玩家随之消失,真死) | extract-aborted 触发,走 terminate_character |
| **P3** ✅ 2026-05-08 | tiandao race-out 专属 narration | calamity.md 风格台词种子 + 输出契约就位 |

---

## §2 关键流程

```
1. 副本内最后一件上古遗物被取走 (loot table 监听)
2. tsy::CollapseStarted event ✅ broadcast
3. 全副本玩家 client 显示红色 3 秒倒计时 + race-out 专属 narration
4. server 生成 3-5 个随机塌缩裂口位置(避免聚集)
5. 玩家选择最近裂口冲过去 + 启动 race-out ExtractRequest(timeout=3s)
   - **Q-RC4 决策**：单裂口同时只许 1 人；挤的人撞墙必须换下一个裂口
   - **Q-RC5 决策**：race-out 期间允许 PVP（互相推下裂口 / 抢裂口）
6. 3 秒到 → 仍在副本内的玩家全部走标准死亡终结流水（**Q-RC3 决策 A+C 复合**）：
   - 进 `plan-death-lifecycle-v1::terminate_character`
   - 寿元 -5%（化虚额外 -100 年，worldview §十二）
   - 干尸转道伥（复用 plan-tsy-hostile-v1 ✅ CorpseEmbalmed 链路）
   - **inventory 全副留副本**——副本化死域后无人能取，自然消失（与 worldview §十六"塌缩只喷干尸不喷物品"自洽）
7. ~~化虚玩家额外惩罚~~ 已并入 Step 6 标准流水（按 worldview §十二 寿元等比扣）
```

---

## §3 数据契约

- [x] ~~`server/src/world/tsy_raceout.rs` 3 秒撤离逻辑(短 timeout 路径)~~ → 复用 `server/src/world/extract_system.rs::on_tsy_collapse_started`（前置 `plan-tsy-extract-v1` 已落地，本 plan 不另起独立 mod 避免重复造轮子）
- [x] ~~`server/src/world/tsy_collapse_rifts.rs` 3-5 裂口生成~~ → 复用 `server/src/world/extract_system.rs::spawn_collapse_tears`（前置 plan 已落地，`count = 3 + deterministic_seed % 3`）
- [x] ~~`client/.../tsy/RaceoutCountdownHud.java` 3 秒倒计时 UI(红色高警告)~~ → 复用 `client/src/main/java/com/bong/client/hud/ExtractProgressHudPlanner.appendCollapse`（前置 plan-tsy-extract-v1 已落地基础 HUD；本 plan 增加 race-out 紧迫文案 + Q-RC4 撞墙提示）
- [x] `agent/packages/tiandao/src/skills/calamity.md` race-out 专属 narration（章节 + 4 条种子台词 + 强约束已就位；触发信号通道由 narrative-v1 后续 P 接入）

---

## §4 开放问题

- [x] **Q-RC1 ✅**（user 2026-05-04 A）：**统一 3s**——物理约束不是技能。化虚也死，符合 worldview §十六.六"化虚级也可能死"。
- [x] **Q-RC2 ✅**（user 2026-05-04 C）：**真随机 + 玩家可见远处闪光**——服务端真随机生成 3-5 个塌缩裂口位置（地形约束内），但 client 给所有玩家可见的闪光指示（远视觉钩）；不保证近，玩家自己跑。天道"不在乎你"语义保留。
- [x] **Q-RC3 ✅**（user 2026-05-04 A+C 复合）：**走标准死亡终结流水**——慢一秒玩家:
  - 进 plan-death-lifecycle-v1::terminate_character 终结流水（寿元正常扣 -5%；化虚额外 -100 年按 worldview §十二）
  - 干尸转道伥（plan-tsy-hostile-v1 ✅ CorpseEmbalmed → 道伥转化链路，无新代码）
  - **inventory 全副留副本**——副本化死域后无人能取，自然消失（与 worldview §十六"塌缩外溢只喷出干尸不喷物品"自洽）
- [x] **Q-RC4 ✅**（user 2026-05-04 B）：**单裂口同时只许 1 人**——先到先得，挤的人撞墙（撞不进，必须找下一个裂口；增加 race-out 紧迫感）。3-5 裂口足够 ≤5 人副本同时撤；规模更大副本会有人被卡死，符合 worldview"塌缩残忍"。
- [x] **Q-RC5 ✅**（user 2026-05-04 A）：**允许 PVP**——race-out 期间互相推下裂口/抢裂口可正常 PK。"末土残忍"设计：盟友也可能在塌缩瞬间反目。需 telemetry 监控（plan-style-balance-v1 联动），评估是否过度恶化合作环境。

## §5 进度日志

- 2026-05-01：骨架创建。plan-gameplay-journey-v1 §M.1 派生。
- **2026-05-04**：skeleton → active 升级（user 拍板）。§4 全部 5 决策闭环：
  - 3s 统一（化虚也死，符合 worldview §十六.六）
  - 真随机裂口 + 远视觉闪光指示（不保证近）
  - 慢一秒 = 标准死亡终结流水（terminate_character + 寿元 -5%/-100 年 + 干尸转道伥 + inventory 留副本）
  - 单裂口 1 人（先到先得 / 撞墙换下一个）
  - race-out 期间允许 PVP
- **2026-05-08**：`/consume-plan` 落地全部 P0/P1/P2/P3。骨架范围内绝大部分语义已由前置 `plan-tsy-extract-v1` / `plan-tsy-lifecycle-v1` / `plan-tsy-loot-v1` / `plan-death-lifecycle-v1` 实装；本 PR 增量是 race-out polish + Q-RC4 单 portal 单 player + race-out HUD 紧迫文案 + calamity prompt 语料。

## Finish Evidence

### 落地清单（按 P 阶段）

**P0 ✅ 2026-05-08 — 3 秒红色倒计时 client UI + race-out 信号识别**
- HUD：`client/src/main/java/com/bong/client/hud/ExtractProgressHudPlanner.java::appendCollapse` 红色屏幕 tint（`0x22FF0000`）+ DANGER 主标 `"race-out · 化死域 Xs"` + MUTED 副标 `"→ 冲入塌缩裂口（已占即换下一个）"` —— 紧迫文案对齐 worldview §十六.六 race-out 术语 + 提示 Q-RC4 撞墙规则。
- 信号识别：`client/src/main/java/com/bong/client/network/ExtractServerDataHandler.java` 已消费 `tsy_collapse_started_ipc` → `ExtractStateStore.markCollapseStarted` → `ExtractState.collapseRemainingTicks` → HUD（前置 `plan-tsy-extract-v1` 落地，本 plan 复用）。
- 3s 撤离时长：`server/src/world/rift_portal.rs::RiftKind::CollapseTear.base_extract_ticks() = 60`（前置 plan 已锁；plan §0 "ExtractRequest API timeout 强制 3s" 在 race-out 启动时由 `on_tsy_collapse_started` 把所有 portal `current_extract_ticks` 压到 60）。

**P1 ✅ 2026-05-08 — 3-5 个随机塌缩裂口 + Q-RC4 单 portal 单 player**
- 3-5 裂口生成：`server/src/world/extract_system.rs::spawn_collapse_tears`（前置 plan 已落地；本 PR 复用，无新代码），`count = 3 + deterministic_seed(...) % 3 ∈ [3, 5]`，跨 family 多 zone 随机分布。
- Q-RC4 单 portal 单 player（**本 PR 新增**）：
  - `ExtractRejectionReason::PortalOccupied` 新 variant
  - `start_extract_request` 加 `portal_occupants: Query<&ExtractProgress>` + `portal.kind == RiftKind::CollapseTear` 时的占用判定
  - `extract_emit.rs::reject_reason_wire` 复用 `ExtractAbortedReasonV1::AlreadyBusy`（不引入 schema breaking change）
- 测试：`world::extract_system::tests::collapse_tear_rejects_second_player_with_portal_occupied` / `main_rift_allows_concurrent_extracts_unlike_collapse_tear` / `collapse_tear_independent_portals_allow_parallel_extracts` / `collapse_tear_unlocks_after_first_player_completes` —— 4 个新测试 ✅，server 共 2526 测试全绿。

**P2 ✅ 2026-05-08 — 慢一秒走 terminate_character 流水**
- 死线触发：`server/src/world/extract_system.rs::on_tsy_collapse_completed` 给所有还有 `TsyPresence` 的玩家发 `DeathEvent { cause: "tsy_collapsed" }`（前置 `plan-tsy-extract-v1` 已落地）。
- 终结链路：`DeathEvent` → `combat::lifecycle::death_arbiter_tick` → `apply_death_lifespan_penalty(cap_by_realm)` → `LifespanCapTable::death_penalty_years_for_cap = cap / 20`（前置 `plan-death-lifecycle-v1` 已落地）。
- 化虚 -100 年：`Realm::Void` cap = 2000 → 100 年扣减；测试 `cultivation::lifespan::tests::death_penalty_uses_five_percent_floor` 已 pin（前置 plan 已锁）。
- inventory 留副本：`inventory::tsy_death_drop::apply_tsy_death_drop` 把 drop 留 zone（不掉到主世界），副本 cleanup 时 `tsy_collapse_completed_cleanup` 蒸发（前置 `plan-tsy-loot-v1` / `plan-tsy-lifecycle-v1` 已落地）。
- 干尸 → 道伥：`tsy_collapse_completed_cleanup` 50% Roll 道伥喷出主世界（前置 `plan-tsy-hostile-v1` + `plan-tsy-lifecycle-v1` 已落地）。
- 集成测试：`world::tsy_lifecycle_integration_test::tests::collapse_completed_kills_player_in_tsy` 已锁 cause="tsy_collapsed"（前置 plan 已锁）。

**P3 ✅ 2026-05-08 — tiandao race-out 专属 narration**
- `agent/packages/tiandao/src/skills/calamity.md` 新增 `## 坍缩渊塌缩 race-out（worldview §十六.六 · plan-tsy-raceout-v1）` 章节：
  - 触发条件 + 强约束（直白紧迫不留余地、禁运动口号、不点名玩家、贯穿"它不在乎"母题）
  - 风格台词种子 4 条（含 plan §0 钦定的 "它要塌了。它不在乎你身上还揣着什么。"）
  - 输出契约：`scope:"zone"` / `style:"system_warning"`（沿用既有 NarrationStyle，不引入 schema breaking change） / 长度 60-120 字
  - 配套 commands 通常为空（race-out 由 lifecycle 推进，不与天道 `realm_collapse` 事件共用通道）
- 测试：`agent/packages/tiandao npm test` 256 / `agent/packages/schema npm test` 283 全绿。
- **遗留**：race-out 信号通道（如 `ZoneStatusV1::RaceOut` variant 或 ContextAssembler block）由 narrative-v1 后续 P 接入；本 plan 范围只锁 calamity Agent 端 prompt 语料，确保接通时台词风格已就位。

### 关键 commit
- `230d6314f` 2026-05-08 — feat(extract): CollapseTear 单 portal 单 player（plan §4 Q-RC4）+ 4 测试
- `ac6ab1f54` 2026-05-08 — feat(hud): race-out HUD 紧迫文案 + Q-RC4 撞墙提示（plan §1 P0）
- `52254b85b` 2026-05-08 — feat(tiandao): calamity.md race-out 专属 narration 章节（plan §1 P3）

### 测试结果
- `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` → 2526 passed
- `cd client && ./gradlew test build` → BUILD SUCCESSFUL
- `cd agent/packages/schema && npm test` → 283 passed
- `cd agent/packages/tiandao && npm test` → 256 passed

### 跨仓库核验
- **server**：`ExtractRejectionReason::PortalOccupied` / `start_extract_request` 加 `portal_occupants` Query / `reject_reason_wire` 映射 / 4 个新测试覆盖单 portal 单 player
- **client**：`ExtractProgressHudPlanner.appendCollapse` race-out 紧迫文案 / `ExtractProgressHudPlannerTest.collapseStateBuildsCountdownTint` 断言 race-out + 化死域 + 已占即换三处
- **agent**：`agent/packages/tiandao/src/skills/calamity.md` race-out 章节 + 风格台词种子 4 条
- **schema**：未改动（Q-RC4 / race-out HUD / narration 全部复用既有 IPC 字段）

### 遗留 / 后续
- **race-out 信号通道**：calamity Agent 当前看不到 zone Collapsing 状态（`ZoneStatusV1` 只有 `normal` / `collapsed`）。narrative-v1 / context-assembler 后续 P 需新增 RaceOut variant 或独立 collapse-active block，本 plan 不涉。
- **client polish**：粒子 / 音效 / Q-RC2 远视觉闪光指示（玩家可见远处闪光）—— 由 plan-vfx-v2 或 plan-tsy-extract-v2 接手（前置 `plan-tsy-extract-v1` finish evidence §7 已点名）。
- **AFK / disconnect**：race-out 期间 disconnect 处理由 `plan-tsy-extract-v1` finish evidence §7 推迟到 v2，本 plan 不涉。
- **CollapseTear spawn raycast**：避免 portal 落进墙里，前置 plan §7 推迟到 v2，本 plan 不涉。
- **Q-RC5 PVP telemetry**：race-out 期间 PVP 数据由 `plan-style-balance-v1` 后续接入，监控盟友互捅是否过度恶化合作环境。
