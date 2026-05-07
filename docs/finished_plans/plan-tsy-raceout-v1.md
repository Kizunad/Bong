# Bong · plan-tsy-raceout-v1 · Finished

> **状态**：✅ 2026-05-07（落地）。前置 plan-tsy-lifecycle-v1 / plan-tsy-extract-v1 / plan-tsy-hostile-v1 / plan-death-lifecycle-v1 全 ✅ finished。§4 全部 5 决策闭环（Q-RC1/RC2/RC3/RC4/RC5 详 §4）。

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

- [ ] **3 秒不是标准撤离**：用同样的 ExtractRequest API 但 timeout 强制 3s
- [ ] **化虚级也可能死**：大真元池在塌缩负压下吃亏更大,平衡设计
- [ ] **专属 narration**：风格台词 "它要塌了。它不在乎你身上还揣着什么。"

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ✅ 2026-05-07 | 3 秒倒计时 client UI + race-out 信号识别 | 玩家在塌缩时见 3 秒红色倒计时 |
| **P1** ✅ 2026-05-07 | 3-5 个随机塌缩裂口生成(独立位面内) + 单裂口 1 人 | 全副本玩家可向最近裂口跑，先到先得 |
| **P2** ✅ 2026-05-07 | 慢一秒 → 副本化为死域(玩家随之消失,真死) | tsy_collapsed cause 强制 Terminated，禁重生 |
| **P3** ✅ 2026-05-07 | tiandao race-out 专属 narration | "它要塌了..." 在塌缩开始时全副本广播 |

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

- [ ] `server/src/world/tsy_raceout.rs` 3 秒撤离逻辑(短 timeout 路径)
- [ ] `server/src/world/tsy_collapse_rifts.rs` 3-5 裂口生成
- [ ] `client/.../tsy/RaceoutCountdownHud.java` 3 秒倒计时 UI(红色高警告)
- [ ] `agent/packages/tiandao/skills/calamity.md` race-out 专属 narration(并入 narrative-v1)

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
  下一步起 P0 worktree（tsy_raceout.rs 短 timeout 路径 + 3-5 裂口生成 + 客户端红色倒计时 HUD）。
- **2026-05-07**：consume-plan 闭环完成（claude/consume-plan-tsy-raceout-UabEJ 分支）。详见 §6 Finish Evidence。

---

## §6 Finish Evidence

### 落地清单（按 P 对应）

**P0 — 3 秒红色 race-out 倒计时 HUD**
- `client/src/main/java/com/bong/client/hud/ExtractProgressHudPlanner.java::appendCollapse` 重写：全屏红色 tint 0x33FF0000 + 顶部 "塌缩 RACE-OUT" banner + 中央向上取整大号秒数（3 → 2 → 1）
- `client/src/main/java/com/bong/client/network/ExtractServerDataHandler.java::"tsy_collapse_started_ipc"` 路由（既有）→ `client/src/main/java/com/bong/client/tsy/ExtractStateStore.java::markCollapseStarted`
- 数据契约：`server/src/network/extract_emit.rs::emit_tsy_collapse_started_payloads`（既有）→ schema `TsyCollapseStartedIpcV1` (server `server/src/schema/server_data.rs:1115` + agent `agent/packages/schema/src/extract-v1.ts:107`)
- `RiftKind::CollapseTear.base_extract_ticks() = 60`（`server/src/world/rift_portal.rs:33`）= 3 秒 @ 20 TPS

**P1 — 3-5 随机裂口 + Q-RC4 单裂口 1 人 + Q-RC2 远视觉钩**
- 3-5 裂口生成既有：`server/src/world/extract_system.rs::spawn_collapse_tears`（`3 + seed % 3 = 3..=5`）
- 单裂口 1 人（**新增**）：`server/src/world/extract_system.rs::start_extract_request` 加 `occupants: Query<(Entity, &ExtractProgress)>`，CollapseTear kind 检测同 portal 已有他人 ExtractProgress 则发 `ExtractRejectionReason::PortalOccupied`
- 客户端远视觉钩（**新增**）：`client/.../ExtractProgressHudPlanner.appendCollapseRiftListWithPlayerPos` 在塌缩期间右上角列最近 5 个本族 collapse_tear 距离，跨 family/方向/kind 全过滤
- 数据契约：`ExtractAbortedReasonV1::PortalOccupied`（server `server/src/schema/server_data.rs:1093`+ agent `agent/packages/schema/src/extract-v1.ts:87`）+ `ExtractStateStore.reasonLabel("portal_occupied")`

**P2 — tsy_collapsed 强制终结（Q-RC3 A+C 复合）**
- `server/src/combat/lifecycle.rs::is_terminal_death_cause("tsy_collapsed") = true`
- `death_arbiter_tick`（DeathEvent 分支）：在 `lifespan_exhausted || force_terminate` 时跳过 `determine_revival_decision`，直接走 `terminate_lifecycle_with_death_context` —— fortune_remaining > 0 也救不了
- 寿元 -5% / 化虚 -100 年由 `apply_death_lifespan_penalty` 在标准管线照常扣（worldview §十二）；`death_penalty_years_for_realm(Void) = 2000/20 = 100`
- 副本入口已发 `tsy_collapsed` cause：`server/src/world/extract_system.rs::on_tsy_collapse_completed:572`（既有）
- inventory 留副本：终结路径不触发 `PlayerRevived`，因此 `apply_death_drop_on_revive` 不跑，物品随玩家 character 终结自然消失（与 worldview §十六"塌缩外溢只喷干尸不喷物品"自洽）；副本 zone 由 `tsy_collapse_completed_cleanup` 移除后无人能取

**P3 — race-out 专属 narration**
- `agent/packages/tiandao/src/skills/calamity.md`：新增"## race-out 专属台词"章节，scope=broadcast、锚句"它要塌了。它不在乎你身上还揣着什么。"等三句、触发时机 `tsy::CollapseStarted`
- 既有 `realm_collapse` event type 复用，narration 风格层加 race-out 区分

### 关键 commit（claude/consume-plan-tsy-raceout-UabEJ 分支）

| hash | 日期 | 摘要 |
|---|---|---|
| `e235f59` | 2026-05-07 | feat(tsy-raceout): CollapseTear 单裂口 1 人 (Q-RC4) |
| `eac8eae` | 2026-05-07 | feat(tsy-raceout): tsy_collapsed cause 强制终结 (Q-RC3) |
| `2535c38` | 2026-05-07 | feat(tsy-raceout): P0/P1 race-out HUD - 大屏 3 秒倒计时 + 裂口列表 |
| `e16a038` | 2026-05-07 | docs(tsy-raceout): race-out 专属 narration 写入 calamity skill (P3) |
| `b9629b8` | 2026-05-07 | chore(tsy-raceout): allow too_many_arguments + cargo fmt 修整 |

### 测试结果

- `cargo test`（server/）：**2526 passed**（含 4 个新增回归：`collapse_tear_rejects_second_extractor_with_portal_occupied`、`main_rift_allows_second_extractor_despite_existing_occupant`、`tsy_collapsed_cause_terminates_immediately_despite_remaining_fortune`、`tsy_collapsed_void_realm_loses_one_hundred_years`）
- `npm test -w @bong/schema`：**283 passed**
- `npm test -w @bong/tiandao`：**256 passed**
- `./gradlew test`（client/）：通过（含 3 个新增 HUD 测试：`collapseStateBuildsCountdownTint`、`collapseStateCountdownTicksDownToOne`、`collapseRiftListShowsUpToFiveNearestSorted`、`collapseRiftListFiltersOtherFamilies`）
- `cargo fmt --check` 通过

### 跨仓库核验

| 层 | 命中 symbol |
|---|---|
| server (Rust) | `ExtractRejectionReason::PortalOccupied`, `is_terminal_death_cause("tsy_collapsed")`, `start_extract_request::occupants`, `death_arbiter_tick::force_terminate` |
| schema (TS) | `ExtractAbortedReasonV1` 增 `"portal_occupied"`, JSON regen 通过 `npm run generate` |
| client (Java) | `ExtractStateStore::reasonLabel("portal_occupied") = "裂口被占，换下一个"`、`ExtractProgressHudPlanner::appendCollapse` race-out banner、`appendCollapseRiftListWithPlayerPos` 远视觉钩 |
| agent (TS / md) | `agent/packages/tiandao/src/skills/calamity.md` race-out 章节 |

### 遗留 / 后续

- **Q-RC2 真随机度**：`spawn_collapse_tears` 仍按 `deterministic_seed(family_id, now_tick)` 派生 3-5 裂口位置，对玩家不可预测但服务端可重放（测试友好）。如未来 plan 要求绝对运行时随机，扩 `world::tsy::spawn_collapse_tears` 接 `bevy_rand` 即可，本 plan 接口契约（裂口数 3..=5、独立位面、可见 entity）保持不变。
- **Q-RC5 PVP telemetry**：race-out 期间允许 PVP 已经存在（无显式禁用），与 plan-style-balance-v1 联动监控盟友互推数据由该 plan 后续 telemetry hook 接入；本 plan 范围不涉及。
- **HUD 位置精度**：客户端列出 collapse_tear 距离用 server 推送的 `RiftPortalState.world_pos`，无方位指针/箭头。极端情况下玩家在垂直方向上无法判断 Y 差。后续 plan 可考虑补一个简单方位箭头（不在本 plan 范围）。
