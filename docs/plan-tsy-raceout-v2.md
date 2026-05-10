# Bong · plan-tsy-raceout-v2 · 骨架

race-out v1 polish 包：承接 `plan-tsy-raceout-v1` ✅ finished（PR #151，2026-05-08）落地了 Q-RC4 单 portal 单 player + race-out HUD 紧迫文案 + calamity narration prompt。**v2 收集 PR #149**（同期 cloud Claude session 重复消费 v1 的并行实施版本，已 closed）的**差异化深度**作为正式 polish：① 大屏 race-out banner（顶部 "塌缩 RACE-OUT" + 中央向上取整 3→2→1 大号秒数） ② Q-RC2 远视觉钩裂口列表（client HUD 右上角列最近 5 个本族裂口距离 + 跨 family/方向/kind 全过滤） ③ Schema 新增 `ExtractAbortedReasonV1::PortalOccupied` wire variant（additive 不破 client，区分"自己已撤" vs "被人占"两路 UX） ④ **P2 strict vs lenient 终结流水二次决策**——v1 选 lenient（走标准 `death_arbiter_tick` 含 `determine_revival_decision`），#149 选 strict（新增 `is_terminal_death_cause("tsy_collapsed")` 跳过 revival，`fortune_remaining > 0` 也救不了），plan §1 P2 写"无重生" vs §2 step 6/§4 Q-RC3 + worldview §十六.六"按 §十二 现有规则照走"两边都有支撑，**user 后续必须拍板** ⑤ P3 narration scope=zone（v1）vs broadcast（#149）二次决策。

**世界观锚点**：`worldview.md §十六.一 step 4 race-out`（line 1392，"还没撤出的所有修士面临 race-out"）· `§十六.六 坍缩渊塌缩`（line 1589-1592，秘境内死亡 100% 掉落 + 干尸化 / 按 §十二 现有规则照走 vs strict force-terminate）· `§十六:1537 塌缩裂口表`（race-out 期间随机开 3-5 / 撤离 3 秒 / 塌缩完成即封闭）· `§十六:1544 塌缩优先`（撤离时长强制缩短 3s）· `§十二 死亡终结流水`（fortune_remaining + 寿元 -5% + 化虚 -100 年）

**library 锚点**：`world-0003 诸渊录·卷一·枯木崖`（坍缩渊叙事范例）· 暂无专属图书馆条目（race-out v1 finish evidence 已点名 narrative-v1 后续 P 接入信号通道）

**前置依赖**：

- `plan-tsy-raceout-v1` ✅ finished（PR #151，2026-05-08；race-out HUD 文案 + Q-RC4 in-loop reservation + calamity narration prompt）
- `plan-tsy-extract-v1` ✅ finished（CollapseTear 60 ticks / spawn_collapse_tears / on_tsy_collapse_started 把 portal current_extract_ticks 压到 60 / on_tsy_collapse_completed 给残留玩家发 DeathEvent）
- `plan-tsy-lifecycle-v1` ✅ finished（COLLAPSE_DURATION_TICKS = 600 / TsyCollapseStarted+Completed events / tsy_collapse_completed_cleanup 50% 道伥喷出 + loot 蒸发）
- `plan-tsy-loot-v1` ✅ finished（apply_tsy_death_drop 把 drop 留 zone 不掉到主世界）
- `plan-tsy-hostile-v1` ✅ finished（CorpseEmbalmed → 道伥转化链路）
- `plan-death-lifecycle-v1` ✅ finished（death_arbiter_tick / apply_death_lifespan_penalty 化虚 cap 2000/20=100 / determine_revival_decision 含 fortune_remaining 救命）

**反向被依赖**：

- `plan-narrative-v1` ⏳（race-out 信号通道接入：calamity Agent 当前看不到 zone Collapsing 状态——v1 finish evidence 点名 narrative-v1 后续 P 新增 `ZoneStatusV1::RaceOut` variant 或独立 collapse-active block）
- `plan-style-balance-v1` ⏳（Q-RC5 race-out 期间 PVP telemetry 监控盟友互捅是否过度恶化合作环境）
- `plan-vfx-v2` 🆕 placeholder（client polish 粒子/音效/Q-RC2 远视觉闪光指示——race-out 启动瞬间裂口位置闪光，配合本 plan 的距离列表）

---

## 接入面 Checklist

- **进料**：
  - server: `world::tsy_lifecycle::{TsyCollapseStarted, COLLAPSE_DURATION_TICKS, TsyZoneStateRegistry, TsyLifecycle}` / `world::extract_system::{spawn_collapse_tears, on_tsy_collapse_completed, ExtractRejectionReason}` / `combat::events::DeathEvent` / `combat::lifecycle::{death_arbiter_tick, apply_death_lifespan_penalty, determine_revival_decision, terminate_lifecycle_with_death_context}` / `cultivation::lifespan::{LifespanCapTable, DeathRegistry}` / `world::tsy::{RiftKind, RiftPortal, TsyPresence}`
  - client: `network::ExtractServerDataHandler::"tsy_collapse_started_ipc"` / `tsy::ExtractStateStore::{markCollapseStarted, collapseRemainingTicks, collapseActive}` / `tsy::RiftPortalView`（已有 entity_id / kind / family_id / world_pos / current_extract_ticks）
  - agent: `tiandao/skills/calamity.md` race-out 章节（v1 已写）+ ContextAssembler block（narrative-v1 后续 P 提供）
- **出料**：
  - server → IPC: `ExtractAbortedReasonV1::PortalOccupied` wire variant（additive，不破 client）/ 可选新增 `RaceoutPlayerTerminated` event（strict 路径专用，区分普通 tsy_collapsed DeathEvent）
  - client UX: 大屏 banner + 中央 3 倒计时 + 右上角 5 裂口距离列表 + portal_occupied 提示文案"裂口被占，换下一个"
  - agent: race-out narration scope=zone vs broadcast 二次决策（plan §4 加 Q-RC6）
- **共享类型 / event**：复用 v1 实装的 `TsyCollapseStarted`, `RiftPortal`, `ExtractProgress`, `ExtractRejectionReason`（仅 PortalOccupied 在 wire 层增 variant）。**禁止**新建近义 event/component（race-out 是 v1 已实装机制的 polish，本 plan 不引入新 system 概念）。
- **跨仓库契约**：
  - server: `combat::lifecycle::is_terminal_death_cause`（仅 strict 路径）/ `world::extract_system::start_extract_request::occupants` 已实装（v1 c58c805a9）/ `network::extract_emit::reject_reason_wire(PortalOccupied) → portal_occupied`（schema variant 落地后才生效）
  - schema (TS): `ExtractAbortedReasonV1` Type.Union 增 `Type.Literal("portal_occupied")` + JSON regen + samples 双端对拍 / `agent/packages/schema/tests/extract-v1.test.ts` 加正反 case
  - client (Java): `ExtractStateStore.reasonLabel("portal_occupied") = "裂口被占，换下一个"` + `ExtractProgressHudPlanner` 大屏 banner 组件 + `appendCollapseRiftListWithPlayerPos`（参考 #149 PR body 列出的命名）
  - agent (md): `agent/packages/tiandao/src/skills/calamity.md` race-out 章节 scope 字段二次决策（v1 当前是 zone）
- **worldview 锚点**：见头部
- **qi_physics 锚点**：本 plan 不涉真元 / 灵气流动（race-out 玩家死亡走 `combat::lifecycle` + `cultivation::lifespan` 标准管线；化虚 -100 年扣减由 `cap/20` cap-table 已实装）。**不引入新衰减常数 / 公式**。

---

## §0 设计轴心

- [ ] **race-out HUD 大屏化（v1 polish 缺失项）**：v1 实装的是底部小号 label `"race-out · 化死域 Xs"`，紧迫感不够。v2 升级：
  - **顶部 banner**："塌缩 RACE-OUT"（红色加粗，全屏宽度，1/8 屏高）
  - **中央倒计时**：大号秒数（向上取整 3 → 2 → 1，红色发光，1/4 屏高）
  - **右上角裂口列表**（Q-RC2 远视觉钩）：最近 5 个本族 CollapseTear 距离（按距离升序），每行 "🌀 距 NN 格" + portal kind 图标 + 占用状态（已占→灰色 + ✗ 标记）
  - 全屏红色 tint 保留（v1 已实装 `0x22FF0000`）
- [ ] **Q-RC4 IPC 精确化**：v1 复用 `AlreadyBusy` wire variant 把"被人占用"和"自己已撤"混在一起——客户端无法区分给玩家的提示。v2 新增 `ExtractAbortedReasonV1::PortalOccupied` literal（schema additive 不破现有 client），client 区分两路 UX 提示：
  - `already_busy` → "你已在撤离中"（玩家自己 ExtractProgress 已存在）
  - `portal_occupied` → "裂口被占，换下一个"（CollapseTear 被他人占用）
- [ ] **P2 终结流水 strict vs lenient 决策（user 拍板，记 Q-RC6）**：plan-v1 §1 P2 写"extract-aborted 触发,无重生"，§2 step 6 / §4 Q-RC3 写"走标准死亡终结流水（fortune_remaining 救命 + 寿元 -5% / 化虚 -100 年 + 干尸转道伥）"，与 worldview §十六.六"按 §十二 现有规则照走"对齐——**两边矛盾**。v1 实施按 §2/§4/worldview 走 lenient，#149 按 §1 P2 字面走 strict。v2 必须 user 拍板：
  - **方案 A（strict force-terminate）**：新增 `combat::lifecycle::is_terminal_death_cause("tsy_collapsed") -> bool`，`death_arbiter_tick` 在该 cause 下跳过 `determine_revival_decision`，直接 `terminate_lifecycle_with_death_context`，fortune_remaining > 0 也救不了。哲学：race-out 失败是物理回收（worldview §十六"垃圾压缩机"），不给"运数"打补丁。
  - **方案 B（lenient 标准流水）**：保留 v1 现状走 `death_arbiter_tick` 全流程，fortune_remaining 仍可救命（运数期保底 / 鞘 8 等条件满足时 RebirthStage::Fortune），干尸 + 寿元 -5% / 化虚 -100 年照走。哲学：race-out 失败仍是死亡，但"末土残忍"在物理层（被抽干 / 干尸化）已经体现，运数救命留给玩家积累有意义的余量。
  - **方案 C（混合）**：fortune_remaining 仍可救命，但仅限**未在化虚境**——化虚境玩家在 race-out 失败时强制 terminate（worldview §十六.六 "化虚级也可能死"严格化 + worldview §三:78 化虚天道针对）。其他境界走 lenient。
- [ ] **P3 narration scope 决策（user 拍板，记 Q-RC7）**：v1 calamity.md 当前定 `scope:"zone"`（race-out 是单副本事件，外人感知只到远处异象）；#149 走 `scope:"broadcast"`（强调坍缩渊塌缩在末法残土的稀缺性 / 钩玩家关注）。两边都有理由，user 拍板。
  - **方案 A (zone)**：保持 v1，仅副本内玩家收 narration。其他玩家完全不知道。优点：契合 worldview "天道不在乎"；缺点：少一个游戏内传闻发酵机会。
  - **方案 B (broadcast)**：所有在线玩家收 narration。优点：传闻发酵 / 让活坍缩渊塌缩成为社交事件 / NPC 对话可引；缺点：违反 worldview "玩家可感知边界"（远处玩家不该直接知道发生了什么）。
  - **方案 C (zone + 远处异象 broadcast)**：副本内玩家收完整 narration，其他在场 zone 玩家随机收 30% 概率"远处隐有崩塌之声 / 风色异常"广播（不点名是哪个副本）。最契合 worldview，但实施复杂度高。

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | 大屏 race-out banner + 中央倒计时（向上取整 3→2→1） | 玩家在 race-out 期间看到顶部 banner + 中央大号秒数 + 红色全屏 tint，紧迫感显著高于 v1 |
| **P1** ⬜ | Q-RC2 远视觉钩裂口列表（右上角 5 最近本族 CollapseTear 距离 + 占用 / kind 图标） | 玩家不需到处找裂口，HUD 直接列最近 5 个 + 距离 + 占用状态 |
| **P2** ⬜ | Schema 新增 `portal_occupied` wire variant + client 区分 UX 提示 | client 在 portal 被占用时显示 "裂口被占，换下一个"，与"你已在撤离中"区分 |
| **P3** ⬜ | Q-RC6 strict vs lenient 终结流水决策落地 | user 拍板（A/B/C 三选一），按选择写测试 + 实施 |
| **P4** ⬜ | Q-RC7 narration scope 决策落地（zone / broadcast / 混合） | user 拍板，calamity.md / context-assembler 调整 |
| **P5** ⬜ | Finish Evidence + 归档 | plan §1 P0-P4 全 ✅ + Finish Evidence 五栏齐全 |

---

## §2 关键流程（差异化 polish）

```text
v1 已实装基础 race-out（不动）：
  TsyCollapseStarted → on_tsy_collapse_started → spawn_collapse_tears 3-5 个 + 现有 portal current_extract_ticks 压到 60
  client appendCollapse 显示底部小号 "race-out · 化死域 Xs" 文案
  on_tsy_collapse_completed (30s 后) → DeathEvent { cause: "tsy_collapsed" } → death_arbiter_tick 标准流水

v2 polish 增量：
  P0 client UI 升级：
    appendCollapse → 拆成 appendCollapseBanner（顶部）+ appendCollapseCountdown（中央）+ appendCollapse（保留底部 tint）
    倒计时数字 = ceil(remainingTicks / 20)，3 → 2 → 1，每秒切换
  P1 client UI 增加 appendCollapseRiftListWithPlayerPos：
    从 ExtractStateStore.allPortals() 过滤 family_id 匹配 + kind == CollapseTear + direction == Exit
    sort by distance(player_pos, portal.world_pos) asc
    take 5 → render 右上角 List
  P2 schema:
    extract-v1.ts ExtractAbortedReasonV1 Type.Union 增 Type.Literal("portal_occupied")
    server_data.rs ExtractAbortedReasonV1 enum 增 PortalOccupied
    extract_emit.rs reject_reason_wire(PortalOccupied) → ExtractAbortedReasonV1::PortalOccupied
    samples regen + double-side test
    client ExtractStateStore.reasonLabel("portal_occupied") = "裂口被占，换下一个"
  P3 (条件 strict 方案 A):
    combat/lifecycle.rs 新增 fn is_terminal_death_cause(cause: &str) -> bool { cause == "tsy_collapsed" || ... }
    death_arbiter_tick 在 lifespan_exhausted || is_terminal_death_cause(event.cause) 时 revival_decision = None
  P3 (条件 lenient 方案 B): 保留 v1 现状（无新代码）
  P3 (条件混合 方案 C):
    is_terminal_death_cause 接受 &Cultivation 参数 → cause == "tsy_collapsed" && cultivation.realm == Realm::Void
  P4 calamity.md scope:
    A: 不动
    B: scope: "zone" → "broadcast"
    C: scope: "zone" + 新增 calamity 输出 secondary_narration { scope: "broadcast", probability: 0.3, text: "远处异象" }
```

---

## §3 数据契约

- [ ] `ExtractAbortedReasonV1` schema 双端 + samples + regen（**P2 阶段**）
- [ ] `client/.../hud/ExtractProgressHudPlanner.appendCollapseBanner` + `appendCollapseCountdown` + `appendCollapseRiftListWithPlayerPos`（**P0 + P1 阶段**）
- [ ] `client/.../tsy/ExtractStateStore.reasonLabel` mapping（**P2 阶段**）
- [ ] `combat::lifecycle::is_terminal_death_cause`（**仅 P3 strict / 混合方案**）
- [ ] `agent/packages/tiandao/src/skills/calamity.md` race-out 章节 scope 字段（**P4 阶段**）

---

## §4 开放问题

- [ ] **Q-RC6 P2 终结流水 strict vs lenient 决策**（详 §0）：A force-terminate / B lenient 标准 / C 化虚混合 → **user 拍板**
- [ ] **Q-RC7 P3 narration scope 决策**（详 §0）：A zone / B broadcast / C 混合 → **user 拍板**
- [ ] **Q-RC8 大屏 banner 是否压主玩法 HUD**：banner 占顶部 1/8 屏 + 中央 1/4 屏倒计时数字，会不会遮挡主战斗 hotbar / 真元条 / 状态条？测试 race-out 期间玩家是否仍能看清剩余 HUD → P0 验收标准 + 必要时调整位置 / 透明度
- [ ] **Q-RC9 5 裂口距离列表是否暴露过多信息**：worldview "天道不在乎你身上揣着什么" + "玩家可感知边界" 暗示玩家不该一目了然知道所有裂口位置——但 race-out 紧迫场景下 UX 优先级压倒"硬核可感知"。是否在 banner 旁标注 "（race-out 期间临时启用）"？或者只列 3 个 / 仅列方向不列距离？→ P1 拍板
- [ ] **Q-RC10 portal_occupied wire variant migration**：现有 client 收到 `portal_occupied` 但代码不识别会怎样？默认 client `reasonLabel` fallback 走 `已撤离失败` 兜底文案——确认现有 v1 client 不会崩，仅是 UX 不区分（与 v1 复用 `already_busy` 路径一致）→ P2 验收 + e2e 测试

---

## §5 已知风险

- [ ] **大屏 banner 遮挡其他 polish HUD**：v2 同期可能有其他 plan 加 HUD（例如 `plan-armor-v2` / `plan-style-balance-v1` 加状态指示）。banner 优先级最高，其他 polish HUD 在 race-out 期间应自动隐藏或下移。需 plan-HUD-v2 或本 plan P0 提供 `HudVisibility::race_out_active` flag
- [ ] **strict force-terminate 与 plan-multi-life-v1 跨周目继承交互**：strict 路径下 race-out 失败 = 跨周目数 +1，运数清零。需 P3 与 multi-life 联动验证
- [ ] **5 裂口距离列表性能**：每帧从 ExtractStateStore.allPortals() 过滤 + sort，假设 ≤5 玩家副本规模 < 10 portal，无性能问题。但 plan-tsy-extract-v2 若调高 spawn 上限，需重新评估
- [ ] **calamity scope=broadcast 滥用风险**：如果方案 B 落地，每次 race-out 全服 narration 推送，agent 每个 tick 触发——可能淹没正常 narration 频道。需限频（同 family_id 60s 冷却）

---

## §6 进度日志

- 2026-05-08：骨架创建。承接 `plan-tsy-raceout-v1` ✅ finished（PR #151，2026-05-08）+ `plan-tsy-raceout-v1` 的并行实施版本 PR #149（同期 cloud Claude session 重复消费，已 closed）。v2 范围明确：① 大屏 banner + 中央倒计时（v1 polish 缺失） ② Q-RC2 远视觉钩裂口列表 ③ Q-RC4 IPC 精确化 portal_occupied wire variant ④ P2 strict vs lenient 终结流水二次决策（Q-RC6 user 拍板） ⑤ P3 narration scope 二次决策（Q-RC7 user 拍板）。
