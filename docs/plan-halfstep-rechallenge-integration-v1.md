# Bong · plan-halfstep-rechallenge-integration-v1 · 骨架

半步化虚**重渡机制跨层集成**——承接 `plan-halfstep-buff-v1` P3 遗留三项：① client HUD "灵机涌现" 倒计时显示；② agent 侧三条 narration 模板接入；③ dormant NPC 重渡触发 hydrate 钩子。server 端 `HalfStepRechallengeTriggerEvent` / `dispatch_rechallenge_on_quota_opened_system` 已实装（`server/src/cultivation/tribulation.rs`），本 plan 只做接收层集成。

**前置条件**（全部满足才启动）：
- `plan-halfstep-buff-v1` ✅ 已 merge（`HalfStepRechallengeTriggerEvent` 已 emit）
- `plan-npc-virtualize-v1` ✅ 已 merge（`dormant_hydrate_trigger_system` 已实装）
- Redis IPC 通道 `bong:player_event` / `bong:npc/hydrate_trigger` 已稳定

**交叉引用**：`plan-halfstep-buff-v1.md` ✅（P3 音画规格 + narration 模板 + HUD 参数已内联）· `plan-npc-virtualize-v1.md` ✅（dormant hydrate 路径）· `plan-audio-v1.md` ✅（audio_recipe bus）· `plan-ipc-schema-v1.md` ✅（Redis channel 约定）

**worldview 锚点**：
- **§三:78 化虚稀缺性**：名额空出是全服事件，narration broadcast 对应"天道有感"
- **§三:124 NPC 与玩家平等**：dormant NPC 与玩家走同一 FIFO 队列，hydrate 后按同等规则起劫
- **§十二:1043 生死循环**：重渡是寿元耗尽前唯一第二次机会，HUD 显示强调稀缺窗口

**qi_physics 锚点**：无新物理常数，复用既有 qi_physics 路径（重渡起劫走 `check_qi_threshold`）。

**前置依赖**：
- `plan-halfstep-buff-v1` ✅ — 所有 server 事件 / queue / const 已落
- `plan-npc-virtualize-v1` ✅ — dormant hydrate on event trigger 框架
- `plan-audio-v1` ✅ — audio_recipe JSON consumer

**反向被依赖**：
- `plan-halfstep-buff-calibration-v1`（skeleton）— 需要 P0 遥测数据，而遥测数据要等重渡机制全链路闭合后才有意义

---

## 接入面 Checklist

- **进料**：server emit `HalfStepRechallengeTriggerEvent { char_id, entity, rechallenge_window_until }` → Redis `bong:player_event` 通道（待确认 schema）；`AscensionQuotaOpened` event（`bong:tribulation/quota_opened`）已广播；`HalfStepRechallengeEntry.is_dormant` flag
- **出料**：client HUD layer `tribulation_status`（倒计时 + 淡入/淡出）；agent narration 三条模板（broadcast / player / zone scope）；dormant NPC 触发 `hydrate_trigger_system`
- **共享类型**：复用 `HalfStepRechallengeTriggerEvent` schema（server/agent/client 三端对齐）；复用 `bong:npc/hydrate_trigger` Redis event（npc-virtualize-v1 已定义）
- **跨仓库契约**：server → Redis `bong:player_event` → agent TS 消费 narration；server → client 新增 `HALFSTEP_RECHALLENGE_TRIGGER` CustomPayload type；server → Redis `bong:npc/hydrate_trigger` → server npc-virtualize 模块（同进程 event bus 或 Redis 回环，见 P2 决策门）
- **worldview 锚点**：§三:78 稀缺 + §三:124 平等 + §十二:1043 生死循环
- **qi_physics 锚点**：无新引入

---

## §0 设计轴心

- **server 侧零改动**：所有 server 事件已 emit，本 plan 只接收不生产
- **HUD 规格已锁定**：参照 `plan-halfstep-buff-v1` P3 §P3 音画规格，直接实施；坐标/颜色/时长无需重新设计
- **narration 三条模板已锁定**：参照 `plan-halfstep-buff-v1` P3 narration 模板，直接接入 agent TS schema
- **dormant NPC 钩子**：`HalfStepRechallengeEntry.is_dormant=true` 时 dispatch 系统已 emit 带标记的 trigger；npc-virtualize 模块需监听该 event 并走既有 `dormant_hydrate_trigger_system`

---

## 阶段总览

| 阶段 | 状态 | 主要交付物 | 验收标准 |
|------|------|-----------|---------|
| **P0** | ⬜ | Client HUD layer（tribulation_status.java）| 重渡触发后 HUD "灵机涌现" 显示 + 倒计时正确；淡入/淡出正常 |
| **P1** | ⬜ | Agent narration TS 接入（三条模板 + scope 路由）| 名额空出后 agent broadcast narration 出现；player/zone scope 定向送达 |
| **P2** | ⬜ | dormant NPC hydrate 钩子 + e2e | dormant HalfStep NPC 收到触发后强制 hydrate，后续可正常起劫 |

---

## P0 — Client HUD

规格直接取自 `plan-halfstep-buff-v1` P3 §P3 音画规格（Client 侧）：

- [ ] 新增 `client/src/hud/tribulation_status.java`（`HudRenderLayer: ABOVE_HOTBAR`，anchor top-right，right=24px / top=64px）
- [ ] 监听 `HALFSTEP_RECHALLENGE_TRIGGER` CustomPayload：解析 `rechallenge_window_until`，计算剩余 tick → 展示 `Xd Yh`（刷新率 20Hz，精度分钟）
- [ ] 文字 `"灵机涌现：可重渡虚劫"` hex `#E8DFCF`；倒计时强调色 `#FF9F5E`（剩余 < 24h 时切换）
- [ ] 淡入 400ms ease-out-cubic（收到 payload 后）；淡出 800ms ease-in-cubic（`/tribulation_rechallenge` 成功 / 窗口过期 / 玩家化虚 → server 发 HIDE payload）
- [ ] opacity 0.85 常驻；禁用 vignette/tint（不污染战斗视野）
- [ ] 音效绑定：收到 trigger payload 时播放 `halfstep_rechallenge_trigger_player` audio recipe（两层，见 plan-halfstep-buff-v1 P3 JSON）
- [ ] ≥ 6 单测 / 渲染测试（HUD 显示时机 / 倒计时计算正确 / 颜色切换边界 / 淡入触发 / 淡出触发（三种终止条件）/ 非 HalfStep 玩家不显示）

**P0 验收**：启动 `./gradlew runClient` 手测：化虚修士被击杀 → 名额空出 server emit → client HUD 弹出"灵机涌现"+ 倒计时；7d 窗口过期后自动消失；非半步化虚玩家无 HUD

---

## P1 — Agent Narration TS 接入

三条模板直接取自 `plan-halfstep-buff-v1` P3 narration 段：

- [ ] TypeBox schema 扩展：`HalfStepRechallengeTriggerPayload { char_id: string, rechallenge_window_until: number }` + `QuotaReleasedPayload { quota_current: number, quota_max: number }`（`agent/packages/schema/src/events.ts`）
- [ ] agent `packages/tiandao` 订阅 `bong:tribulation/quota_opened` Redis channel
- [ ] 收到 `AscensionQuotaOpened` → emit narration：
  - `"灵脉间隐约传来一股真元波动，似有化虚修士陨落，名额空出一席。"` — scope: `broadcast`，style: `perception`，priority: `high`
- [ ] 收到 `bong:player_event` 中 `HalfStepRechallengeTriggerEvent` → emit：
  - `"你感到曾遭封压的经脉微微松动，或许时机已到。"` — scope: `player`（target entity），style: `perception`
  - `"虚空中某处的修士收到了相同的消息。"` — scope: `zone`（entity 所在 zone，触发条件：同 zone ≥ 2 个 HalfStep 修士），style: `perception`
- [ ] 音效：broadcast → `halfstep_quota_release_broadcast` recipe；player → `halfstep_rechallenge_trigger_player` recipe；zone echo → `halfstep_rechallenge_trigger_zone_echo` recipe（全部取自 plan-halfstep-buff-v1 P3 JSON）
- [ ] ≥ 8 单测（三条 narration scope 路由 / broadcast 触发条件 / zone echo 触发条件 ≥2 / 无 HalfStep 时不 emit player narration / schema 正反例对拍）

**P1 验收**：`cd agent && npm test` 全绿；mock 场景注入 `AscensionQuotaOpened` event → 验证 broadcast narration 输出；注入 `HalfStepRechallengeTriggerEvent` → 验证 player + zone narration

---

## P2 — Dormant NPC Hydrate 钩子

- [ ] `server/src/npc/virtualize/dormant.rs`：监听 `HalfStepRechallengeTriggerEvent`（`is_dormant: true`），在 `dormant_hydrate_trigger_system` 中处理：
  - 从 `NpcDormantStore` 取 `char_id` snapshot → 走既有 `hydrate_npc` 路径（复用 npc-virtualize-v1 hydrate fn，不另写）
  - hydrate 完成后该 NPC 进入 Hydrated 状态，由既有大脑系统驱动 → 可正常起劫
- [ ] **决策门**（P0 前收口）：`HalfStepRechallengeTriggerEvent` 当前是 Bevy ECS event，同进程直接 listen 不需 Redis 回环；确认 npc-virtualize 模块和 tribulation 模块在同一 Bevy App 实例
- [ ] ≥ 5 单测（dormant NPC 收到 trigger 后从 NpcDormantStore 移除 + ECS entity 创建 / 非 dormant 修士 is_dormant=false 走玩家 HUD 路径不 hydrate / hydrate 后 NPC 进入正常 tick / 队列 FIFO 顺序不因 dormant hydrate 乱序）
- [ ] e2e 手测：dormant HalfStep NPC 距玩家 > 256 格 → 名额空出 → dispatch 触发 hydrate → NPC 出现在玩家附近 → 正常起劫

**P2 验收**：`cd server && cargo test npc::virtualize::rechallenge` 全绿 + e2e 手测通过

---

## §8 开放问题（P0 决策门收口）

1. **CustomPayload type ID**：`HALFSTEP_RECHALLENGE_TRIGGER` 确认 ID 值（需与 client PacketType 枚举对齐，检查 `client/src/network/PacketType.java` 当前最大值）
2. **HalfStepRechallengeTriggerEvent 经 Redis 还是直接 ECS event 触达 npc-virtualize**：确认 npc-virtualize 与 tribulation 是否同一 Bevy App（同进程则直接 event；异进程需 Redis 回环）
3. **HUD 隐藏 payload**：server 需在以下场景发 HIDE payload：① `/tribulation_rechallenge` 成功起劫 ② `window_until` 过期（server-side check 每 tick vs 定时推送）③ 玩家化虚结算
4. **zone echo 触发条件 ≥2 的统计时机**：收到每次 trigger event 时查当前 zone 内 HalfStep entity 数 vs 维护一个 zone-level HalfStep 计数器（前者实时准确，后者性能更好）