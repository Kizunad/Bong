# plan-scroll-reading-v1 — 可阅读卷轴：经脉入门残卷 + 通用读卷 UI + 阅读动作

> **一句话主题**：给新手进服发一卷《经脉浅述·残卷》（正文摘自 library 正典），落地一条**可复用**的"可阅读物品"链路——server `readable_scroll_spec` + S2C `scroll_open` payload + client 卷轴阅读屏（owo，滚动长文）+ PlayerAnimator 双手持卷阅读循环姿态 + 手持白嫖 paper 模型——后续任何书信/残卷/图谱直接挂同一链路。

**状态**：active（§8 已收口，见 §8.1；2026-07-03）。验收日期：全 P ✅ 后填。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | server 底盘——物品模板 / C2S 读取 / S2C scroll_open 契约 / 首入发放 | ✅ 2026-07-03 |
| P1 | client 阅读屏——bridge 接缝 + Store/Opener/Screen（可复用壳） | ✅ 2026-07-03 |
| P2 | 视听——阅读循环动画 / 手持模型 / 展开微光 / SFX / 图标 | ✅ 2026-07-03 |

---

## 背景与调研结论（2026-07-02）

新手引导现状：出生引导棺（`plan-spawn-tutorial-v1`，沉默引导原则）+ 招式残卷（`plan-onboarding-loop-v1`）。**痛点**：现有"读卷"全是消耗式学招（`technique_scroll.rs:54 read_combat_technique_scroll`——读=消耗+学 technique，不弹任何界面），仓库**没有"翻开阅读"的物品先例**，新玩家无处了解经脉体系。

关键接入面（Explore 实证，均为活代码）：

- **server 打开屏先例**：`DeathScreen` / `LootContainerOpen` payload（`agent_bridge.rs:144,205`）→ client 范式 A「payload → Store → tick-poll opener → setScreen」（`CombatScreenOpener.tick():16`、各 `*ScreenBootstrap`）。raw-XML `ui_open` 桥有 512B 上限 + 标签白名单（`UiOpenHandler.java:31`），**不适合长正文，不走它**。
- **桥接手工两处**：新增 payload variant 必须改 `ProtoServerDataBridge` 的 `CASE_TO_TYPE`（`:131`）+ `extractInner` switch（`:337-470`），漏改则 `default: return null` 静默丢包；其余字段转换 JsonFormat 自动（纯字符串正文无 enum 前缀坑）。
- **first-join 只发一次**：`TutorialState` component 已持久化（`spawn_tutorial.rs:71-81` `TutorialHook` enum，`persistence/mod.rs:5166`），加 variant 即可，无需新建机制。
- **动画链路零 client 改动**：gen 脚本 → `assets/bong/player_animation/*.json` 自动加载，server emit `play_anim`（`vfx_animation_trigger.rs`）→ `VfxEventRouter.route()`（`VfxEventRouter.java:64-106`）播放。
- **正文现成**：`docs/library/cultivation/经脉浅述.json`（cultivation-0004，章节：总纲/十二正经/八奇经/任督之要/著者末语），与 worldview §三 经脉正典同源。

## 接入面（docs/CLAUDE.md §二 checklist）

- **进料**：`ItemRegistry`（`inventory/mod.rs:1332 load_item_registry`，`server/assets/items/onboarding_scrolls.toml` 现有 scroll 族）；`TutorialState`/`TutorialHook`（`spawn_tutorial.rs`）；`docs/library/cultivation/经脉浅述.json` 正文；`ClientRequestV1` C2S 通道（`client_request.rs`）。
- **出料**：S2C `ServerDataPayloadV1::ScrollOpen`（新）→ client 阅读屏；`bong:vfx_event` `play_anim` + `spawn_particle`；**复用出口**：后续书信/图谱/丹方预览挂同一 `readable_scroll_spec` + 同一屏。
- **共享类型 / event**：`ServerDataPayloadV1` 加 variant（不复用 `LootContainerOpenV1`——语义是容器非文本，且其 `deny_unknown_fields`）；`TutorialHook` 加 variant；复用 `VfxEventRequest` / `BongAnimationRegistry` / `HeldItemStackResolver`。**不新造** first-join / 动画 / VFX 任何机制。
- **跨仓库契约**（并行实施前锁死，字段/tag 定稿见 §9）：proto `envelope.proto` C2S 新 oneof `scroll_read_request`（**tag 100**）、S2C 新 oneof `scroll_open`（**tag 138**，原 137 被 #825 抢占，见 §9）；TS schema `client-request.ts` / `server-data.ts` + `samples/*.json` 双端对拍；client 桥两处 + `ScrollOpenHandler` 注册进 `ServerDataRouter`。
- **worldview 锚点**：§三 修炼体系（12 正经 + 8 奇经正典，`worldview.md:59-119`）；末法残卷命名惯例（《×××·残页/残卷》，onboarding_scrolls 先例）；`plan-spawn-tutorial-v1` O.13 沉默引导——**发放静默入包、阅读由玩家自发**，不弹强制引导。
- **qi_physics 锚点**：**无灵气流动**——纯阅读，不扣不还不衰减，不触 ledger。显式声明以过红旗清单。

---

## P0 server 底盘 ✅ 2026-07-03

- `ItemTemplate` 加 `readable_scroll_spec: Option<ReadableScrollSpec>`（`inventory/mod.rs:140` 现有 `*_scroll_spec` 族旁）：`{ title: String, body_pages: Vec<String>, anim_id: Option<String> }`。**读不消耗**（区别于 technique_scroll 消耗式）。
- 新物品 `scroll_meridian_primer`《经脉浅述·残卷》进 `onboarding_scrolls.toml`（`category="scroll"`, grid 1×2, max_stack=1），正文 **3 页（每页 ≤150 字）**，见 §8.1 #3 代拟正文（consume 已按 §8.1 收口值，草案 3~5页/≤500字 作废）。
- C2S `ClientRequestV1::ScrollReadRequest { v, instance_id }`：handler 查实例模板有 `readable_scroll_spec` → emit S2C `ScrollOpen { scroll_id, title, body_pages }` + `play_anim`；无 spec / 非本人物品 → 静默拒绝 + warn。
- S2C `ServerDataPayloadV1::ScrollOpen`（proto/serde/TS/samples 四件套，落点清单照 `reference_server_data_payload_field` 模式扩到新 variant）。
- **首入发放（原子性硬要求）**：join 时判定 → 发放 → 打 hook 必须在**同一 system 同一 tick**内完成，且 inventory 与 `TutorialState` 落进同一次持久化 flush。幂等用**双重判定**兜崩溃窗口：`TutorialState.has(TutorialHook::MeridianPrimerGranted)` **或** 背包已存在 `scroll_meridian_primer` 模板实例，任一命中即跳过补发——覆盖"物品已落 hook 未落"与"hook 已落物品未落"两个中断顺序（后者不该出现：实现顺序固定为先加物品后打 hook）。存量老玩家也补发；发放静默，无 narration，对齐沉默引导。发放方式最终拍板见 §8 #1。
- **测试**：spec 解析正反 sample；ScrollReadRequest happy/无 spec/伪 instance_id/重复请求；发放幂等（重连不重发、老玩家补发一次、**模拟"物品已发 hook 未持久化"的崩溃窗口重登不重发**）；payload roundtrip 对拍。

## P1 client 阅读屏（可复用壳）✅ 2026-07-03

范式 A 全套（仿 DeathScreen 链）：

- `ProtoServerDataBridge` 两处：`CASE_TO_TYPE` 加 `SCROLL_OPEN → "scroll_open"`、`extractInner` 加 case。
- 新 `ScrollOpenHandler`（进 `ServerDataRouter`）→ 新 `ScrollReadStore` → 新 `ScrollReadScreenBootstrap`（tick poll → `setScreen`）。
- `ScrollReadScreen extends BaseOwoScreen<FlowLayout>`：竖排卷轴质感面板，正文 `Components.label(...).maxWidth(px)` 自动换行（中文换行活先例 `InspectScreen.java:772`）+ `Containers.verticalScroll(Sizing.fill(100), **Sizing.fixed(viewportH)**, content)`；多页时翻页按钮。**两个已知坑必须遵守**：viewport 高度必须 `Sizing.fixed`（fill 顶飞坑）；翻页刷新用 diff 原地更新不 `clearChildren`（滚动回弹坑，照抄 `CraftRecipeListWidget.java:109-121` 模式）。ESC/关闭按钮退出，`shouldPause()=false`。
- **可复用性验收**：屏的输入只依赖 `ScrollOpen` payload 字段（title/pages），不 hardcode 经脉内容——第二卷任意 readable scroll 零 client 改动可读。
- **测试**：Handler 缺字段丢弃分支；Store 生命周期（open→close→再 open）；分页边界（1 页 / 最大页 / 空页拒绝）。

## P2 视听 ✅ 2026-07-03

- **阅读动画**（`client/tools/gen_read_scroll.py` → `read_scroll.json`，loop）：双手持卷展开——`rightArm/leftArm pitch≈-75°, bend≈90°, axis=180°`（前折约定 `anim_common.py:31`），`head.pitch +12°` 低头，`torso.pitch +10°` 前倾 + **鞠躬补偿**（`body.y -0.04` 下沉、双腿 `pitch -8° bend +12°` 微屈，照抄 `gen_bow_salute.py:52-58`）；呼吸感微摆 ±2°，周期 40 tick，easeInOutSine；**loop 首尾帧全轴对齐**（`_check_loop_closure` 断言，防淡回 T-pose）。`render_animation.py` 出预览网格核姿态后再进游戏。server 在 ScrollOpen 时 emit `play_anim bong:read_scroll`（loop、中低 priority、fadeIn 4 tick），client 关屏发 close 请求或 server 收 `ScrollReadClosed` 后 `stop_anim`（关停机制见 §8 #4）。
- **手持模型**：白嫖原版 `minecraft:paper`——新 `ScrollVanillaIconMap.createStackFor()` 仿 `HoeVanillaIconMap`，纳入 `HeldItemStackResolver.resolveMainHand()` fallback 链（FPV+TPV 双入口自动同步）。不做 OBJ。
- **展开微光 VFX**：新 `ScrollOpenGlowPlayer implements VfxPlayer`（`EVENT_ID=bong:scroll_open_glow`），注册进 `VfxBootstrap.registerDefaults()`（漏注册=静默孤岛，`VfxBootstrap.java:233`）。规格：`BongSpriteParticle` 复用既有微光 sprite（不新增贴图），burst 12 粒 + continuous 1 粒/2tick × 20tick，lifetime 16 tick，色 `#E8D9A0` 淡金，自玩家胸前 0.4m 半径球面向外漂 0.02 b/t。server 在 ScrollOpen 同帧 emit `spawn_particle`。
- **SFX**（audio_recipe JSON）：开卷 `item.book.page_turn` pitch 0.9 vol 0.8 delay 0；翻页同 sound pitch 1.1 vol 0.6（client 本地播）；合卷 `item.book.put` pitch 1.0 vol 0.7。
- **HUD**：无新增常驻元素（沉浸极简约束）。narration：无（沉默引导）。
- **图标**：`/gen-image item` 生成 `scroll_meridian_primer` PNG（批量扫透明度防假透明），**同步 `resourcepack.rs` + manifest sha1/size**（CI 红线）。
- **测试**：动画 JSON loop 闭合断言过 gen 自检；VfxRegistry lookup 命中；icon 进包后资源包 CI 绿。

---

## §8 开放问题（升 active / P0 决策门前收口）

1. **发放方式**：join 补发 + 新 `TutorialHook::MeridianPrimerGranted`（老玩家也拿到，倾向此）vs 只进 `default.toml` loadout（最简但老玩家拿不到）vs 挂出生棺奖励（`grant_coffin_reward_once` 改成发两样）？
2. **可交易/可丢弃**：残卷是否允许丢弃/交易/死亡掉落？丢了是否可凭 hook 状态找 NPC/棺补领？
3. **正文分页与内容源**：3~5 页摘编的具体文案（从 `经脉浅述.json` 哪些章节摘、每页字数）；`body_pages` 内容放 toml 内联 vs 引用 library json（倾向内联进 spec，server 不在运行时读 docs/）。
4. **动画关停机制**：client 关屏后 anim 停止走什么路径——新增 C2S `ScrollReadClosed`（干净、多一条契约）vs loop anim 挂固定时长自然衰出 vs client 侧本地 stop（`VfxEventAnimationBridge` 有无本地 stop 入口）？
5. **阅读姿态与移动**：读卷时是否锁移动（Screen 打开本身吃输入，但 TPV 动画播放中被打断的表现）；被攻击时强制关屏？
6. **复用清单预登记**：下一批挂 `readable_scroll_spec` 的候选（丹方预览？黑武士遗书？图书馆残页实体化？）——只登记不实施，防止本 plan scope 蔓延。

> 全部已在 §8.1 收口。原表保留以备追溯，**实施时以 §8.1 决议为准**。

## §8.1 决议（pre-P0 收口，2026-07-03，Explore agent 实地核查代码产出）

### #1 发放方式

**决议**：
1. **join 补发 + 新 `TutorialHook::MeridianPrimerGranted`**，双重判定幂等（hook 已打 **或** 背包已有 `scroll_meridian_primer` 实例，任一命中跳过）；存量老玩家也补发；发放静默无 narration（沉默引导）。
2. **否决 `default.toml` loadout**——`attach_inventory_to_joined_clients`（`inventory/mod.rs:1100-1128`）只在 `Added<Client>,Without<PlayerInventory>` 跑，老玩家拿不到。
3. 用 **tick-poll 扫未打标记 client** 的新系统（仿 `send_tutorial_coffin_pos_on_join` @ `spawn_tutorial.rs:256-290`），**不用 `Added<Client>` 一次性判定**（会漏发，`spawn_tutorial.rs:153-158` 注释记录过教训）；复用 `grant_coffin_reward_once`（`:487-515`）hook 幂等模式（`trigger()` 返回 bool 天然幂等）。

**落点**：新 `TutorialHook::MeridianPrimerGranted` → `spawn_tutorial.rs:69-82`；新 `grant_meridian_primer_once` 类函数 → 紧邻 `:487-515`；系统注册 → `:196-216`（register）；plan §P0。

### #2 可交易/可丢弃

**决议**：
1. **可交易、可丢弃、死亡按标准掉落——不设特殊限制走通用规则**。全仓无 soulbound/no-drop flag（`ItemTemplate` @ `inventory/mod.rs:120-157` 无此字段），发明锁定机制 = 孤岛红旗。
2. **不做补领**——`grant_coffin_reward_once` 先例也无"丢了再补发"路径，hook 一旦 trigger 永久 true。未来若成真实痛点再单开通用"遗失补领"机制服务多 hook。

**落点**：无新增字段/函数；plan §8.1 明确"沿用现有死亡掉落 + 无补领"防实施 agent 发明机制。

### #3 正文分页与内容源

**决议**：
1. **3 页、每页 ≤150 字、内联 TOML**（`onboarding_scrolls.toml` `body_pages` 数组写死，不运行时读 `docs/library/`，与现有 `*ScrollSpec` 纯 TOML 惯例统一）。
2. 摘自 `docs/library/cultivation/经脉浅述.json` 总纲/十二正经/任督之要 3 节（八奇经/著者末语留白呼应"残卷"设定），术语对齐 `worldview.md:83-101`。
3. **代拟正文**（末法残卷语感，末页留"后缺"损毁标记 + 校补邀请做隐性引导；`body_pages` 数组按序）：
   - **第 1 页·总纲**：人身经脉二十条，正经十二，奇经八脉。真元循经而行——未通如涧中水，散而不汇；既通如江河，沛然归海。此卷不作图谱，只记通序、通难、通惑三事。执此残卷者，愿你我经脉终有一日相汇于江海。
   - **第 2 页·十二正经**：通经须循邻接之序：已通肺经，下一条必在大肠、心经之间择一，不可跳级。已通之经永不闭，唯可被打伤致用途折半——断经之剑手，古今皆废人。手三阴偏气利远，手三阳偏力利近，足三阴偏韧抗毒，足三阳偏速利闪。
   - **第 3 页·任督之要**：坊间常道任督既通境界自抬，此说过半虚妄。二脉虽居八奇经之首，独通不足以致通灵——须奇经通其四，方可冲通灵之门槛。任督真开通之效，唯真元调息速增约三成，非境界之抬。（后缺半页，字迹为水渍所毁）——原卷藏于青云残峰外门木箱，后人可往校补。

**落点**：`server/assets/items/onboarding_scrolls.toml`（追加 item block）；`parse_readable_scroll_spec` 仿 `parse_technique_scroll_spec` @ `inventory/mod.rs:2425-2450`；plan §P0。**正文供用户 PR 时过目**。

### #4 动画关停机制

**决议**：
1. **新增 C2S `ScrollReadClosed`**，镜像 `raise_shield`/`lower_shield`（`combat/shield_block.rs:208`/`:357`）：开屏 emit `PlayAnim{isLoop:true}`，关屏 client 发 C2S → server emit `StopAnim`。否决"client 本地 stop"（`VfxEventAnimationBridge.stopAnim` @ `:33` 仅 router 可调，无 screen 直调先例）+"自然衰出"（阅读时长因人而异不适配）。
2. **加死亡/断线兜底清理**（镜像 `cleanup_shield_on_death` @ `shield_block.rs:405`），防读卷中掉线动画残留。

**落点**：新 `emit_scroll_read_stop_for_entity` 仿 `vfx_animation_trigger.rs:1289-1298`；handler + cleanup 模板 `shield_block.rs:208-405`；C2S handler `client_request_handler.rs:1588-1609`；plan §P2。

### #5 阅读姿态与移动

**决议**：
1. **不新增锁移动**——`ScrollReadScreen extends BaseOwoScreen` 标准 MC Screen，`setScreen` 后 GUI 接管键鼠，WASD 天然不驱动移动（所有现有 owo screen 共享免费行为）。
2. **不做受击强制关屏**——全仓无"受伤自动 setScreen(null)"先例；死亡场景已被 `CombatScreenOpener.tick()`（`:16-28`）无条件覆盖兜底；新手区基本无战斗风险。

**落点**：无新增代码；plan §8.1 写明理由防实施 agent 误加。

### #6 复用清单预登记（只登记不实施）

丹方预览残卷（`recipe_fragment_spec` 族）/ 黑武士遗书（`npc/heiwushi.rs`）/ 图书馆残页实体化。另记：`plan-life-record-epitaph-v1` 的长文本 UI 需求与本 `ScrollReadScreen` 相似，其收口时可提醒复用本 plan 的 owo 长文组件（非本 plan scope）。

### §9 tag 冲突处置（新增，pre-P0 复核）

S2C `scroll_open` 原定 tag 137，2026-07-03 复核发现 **#825 `InventoryMoveRejected` 已占 137**（并行抢号）。按 §9 既定"顺延取号"预案改用 **tag 138**（Explore 扫全 skeleton+active 无第二方声明）。C2S `scroll_read_request` tag 100 仍空闲不变。§9 代码块已同步更新。**P0 落 proto 时按当时 origin/main 再做一次二次确认（§9 条款）。**

## §9 契约锁定（并行实施硬约束）

字段、类型、tag 在此定稿（2026-07-03 复核 `envelope.proto` 现状：C2S oneof 最大 tag=99（`treasure_activate=99`），仍取 **100**；S2C payload oneof 最大 tag=**137**——`InventoryMoveRejected` 已于 #825 占 137，本 plan 原定 137 冲突，按 §9 顺延预案取 **138**（已扫全 skeleton+active 无第二方声明 138））：

```proto
// C2S，oneof payload tag = 100
message ScrollReadRequest {
  uint32 v           = 1;   // 版本，恒 1
  uint64 instance_id = 2;   // 背包内物品实例 id（consume 落地为 u64，对齐全仓 20+ 处 instance_id 惯例，非草案 string；§9「回写本节」已执行）
}

// S2C，oneof payload tag = 138（原定 137 被 #825 InventoryMoveRejected 抢占，顺延，见上方说明）
message ScrollOpen {
  string scroll_id           = 1;   // 模板 id，如 scroll_meridian_primer
  string title               = 2;
  repeated string body_pages = 3;   // 每元素一页，≥1
}
```

纯 string/uint32 字段，无 enum——不触 proto3 枚举全名前缀 fixup 坑。P0 实施落 proto 时若 tag 100/137 已被并行 PR 占用，顺延取号并**回写本节**（tag 号以本节为唯一真相，两端不得各自取号）。实施 agent 不得擅改契约、不得擅自 commit 跨端接缝（[[feedback_parallel_impl_locked_contract]]）。samples（`scroll-read-request.sample.json` / `scroll-open.sample.json`）随 P0 四件套一起出。

## §10 实施工作流

scope ~3 PR，单 plan 内序列化（`docs/CLAUDE.md` §六）。

- **§10.1 拆分点**（依赖顺序，前一个 merge 后开下一个）：
  1. **PR-1 P0**：server 底盘——`readable_scroll_spec` + C2S `ScrollReadRequest`(tag 100) + S2C `ScrollOpen`(tag **138**) 四件套双端（proto+serde+TS schema+samples，走 `reference_server_data_payload_field` 六点）+ `scroll_meridian_primer` TOML（3 页正文见 §8.1 #3）+ join tick-poll 补发系统 + `TutorialHook::MeridianPrimerGranted`。契约锁死（§9），独立可 merge。
  2. **PR-2 P1**：client 阅读屏——`ProtoServerDataBridge` 两处（CASE_TO_TYPE + extractInner）+ `ScrollOpenHandler` + `ScrollReadStore` + `ScrollReadScreenBootstrap` + `ScrollReadScreen` 可复用壳（viewport `Sizing.fixed` 防顶飞、翻页 diff 原地更新防回弹）+ C2S `ScrollReadClosed` 关屏。依赖 PR-1 payload。
  3. **PR-3 P2**：视听——`gen_read_scroll.py` 动画(loop 闭合断言) + `ScrollVanillaIconMap` 手持 paper + `ScrollOpenGlowPlayer` VFX(注册进 `VfxBootstrap`) + SFX audio_recipe + gen-image 图标 + `resourcepack.rs`/manifest sha1 同步。依赖 PR-2。
- **§10.2 撞车防护**：每 PR 开前 `git fetch origin && git log origin/main`，比对 `ProtoServerDataBridge.java` / `envelope.proto` tag（**防 138 再被抢**——落 proto 时二次确认空闲号，§9 条款）/ `spawn_tutorial.rs`。tag 以 §9 为唯一真相，两端不各自取号。
- **§10.3 测试要求**：P0 spec 解析正反 sample + ScrollReadRequest happy/无spec/伪id/重复 + 发放幂等（重连不重发/老玩家补发一次/崩溃窗口重登不重发）+ payload roundtrip；P1 Handler 缺字段丢弃/Store 生命周期(open→close→再open)/分页边界(1页/最大/空页拒绝)；P2 动画 loop 闭合断言/VfxRegistry lookup 命中/图标资源包 CI 绿。饱和覆盖。
- **§10.4 CR 等待**：每 PR `ScheduleWakeup` 1200s × ≤3 回合等 CodeRabbit（[[feedback_wait_coderabbit_approve]]）；CR 限流时博弈过 + e2e 绿即 merge（CR 非 required）。
- **§10.5 subagent 实施**：每 PR 独立 `claude` subagent（opus + `ultrathink`），主线只收 result + merge；**每 PR push 前跑对抗博弈自检**（sonnet 控方/辩方/端到端 → opus 裁决，[[feedback_consume_presubmit_debate]]），辩方胜出才 merge。P2 视听资产走 gen 脚本 + `render_animation.py` 预览核姿态（非 NBT 建筑，不强制 3 轮 PROMISE，但动画 loop 闭合断言必过）；**残卷正文已 §8.1 #3 代拟，PR 时供用户过目**。
- **§10.6 单次 consume 全自动到 merge**：收口已完成（本 §8.1），`/consume-plan` 即可，醒来看是否入 `finished_plans/`。

## 落地证据链

- 收口调研（2026-07-03，Explore agent 实地核查）：§9 tag 137→138 冲突定位（#825 InventoryMoveRejected 抢占）；六条 §8 决议 file:line（发放仿 `grant_coffin_reward_once`/关停仿 `shield_block`/正文内联 TOML）；残卷正文代拟（摘 `经脉浅述.json`）；接入面 file:line 清单（server 物品模板/C2S/S2C/proto/client 桥接/UI/动画）。
- 相关先例：`plan-spawn-tutorial-v1`（沉默引导 + TutorialHook）/ `plan-onboarding-loop-v1`（招式残卷）/ `plan-shield-block-v1`（loop 动画启停 + 死亡兜底）。

---

## Finish Evidence

**验收日期**：2026-07-03（`/consume-plan` 全自动：Design→Implement P0-P2→博弈 Verify，round-1 needs_fix(发起端孤岛)→补发起端→round-2 ready，15 commit + 全绿）

### 落地清单
- **P0 server 底盘**：`ItemTemplate.readable_scroll_spec`（`inventory/mod.rs`，读不消耗，区别 technique_scroll）+ `parse_readable_scroll_spec` + `scroll_meridian_primer`《经脉浅述·残卷》3 页正文入 `onboarding_scrolls.toml`；C2S `ScrollReadRequest`(tag 100) handler(`client_request_handler.rs` 查 spec → emit ScrollOpen + play_anim)；S2C `ScrollOpen`(tag 138) 四件套双端(`envelope.proto`+`server_data.rs`+`proto_convert.rs`+`agent_bridge.rs` label+TS `server-data.ts`+samples)；首入 `TutorialHook::MeridianPrimerGranted` + `grant_meridian_primer_once` + tick-poll 补发系统（仿 `send_tutorial_coffin_pos_on_join`，双重判定幂等，老玩家也补）。
- **P1 client 阅读屏**：`ProtoServerDataBridge` 两处(CASE_TO_TYPE + extractInner) + `ScrollOpenHandler`→`ScrollReadStore`→`ScrollReadScreenBootstrap`→`ScrollReadScreen`(viewport `Sizing.fixed` 防顶飞、翻页 diff 原地更新防回弹、可复用只依赖 payload 不 hardcode 经脉)；C2S `ScrollReadClosed`(**tag 101**，§9 顺延) 关屏。
- **P2 视听**：`gen_read_scroll.py` 阅读循环动画(40 tick loop 闭合断言)；`ScrollVanillaIconMap` 白嫖 paper 接 `HeldItemStackResolver` 第五级(惰性 Supplier 避 headless panic)；`ScrollOpenGlowPlayer` VFX(注册进 `VfxBootstrap` 防孤岛)；SFX audio_recipe；gen-image 图标 + `resourcepack.rs` sha1 同步。
- **发起端(round-2 补,73c8d65b6)**：`InspectScreen` 右键残卷 → `[阅读]` context menu action(`ActionKind.READ_SCROLL`，与 `[研读功法]` 同范式、与 TECHNIQUE_SCROLL_USE 互斥) → `dispatchScrollReadRequest` → `ClientRequestSender.sendScrollReadRequest` → `ClientRequestProtocol.encodeScrollReadRequest`(instance_id u64)。三重守卫(null/instanceId==0/未注册 template 拒绝)。**打通读卷全链路(此前收端全套是死代码)。**

### 关键 commit（分支 `auto/plan-scroll-reading-v1`，15 个）
P0 `e7dfce563`/`94a1e19a1`/`0a6a33bcb`/`3f495af9a` · P1 `21a38e3c4`(ScrollReadClosed tag101)/`64d30a4df`/`9eddc9765` · P2 `edea43356`(三路径止动画)/`846f61261`/`e0feb7876`/`401e3df2c` 等 · fix `73c8d65b6`(发起端)

### 测试结果
- **server**：`cargo fmt+clippy -D warnings+test` = **10967 passed**
- **client**：`./gradlew test build` = **3517 passed / 0 failed**（含 `InspectScreenScrollReadTest` 8 例 + 发起端 pin）
- **agent/schema**：`npm test` drift gate 绿

### 跨仓库核验
- **server**：`readable_scroll_spec`/`ScrollReadRequest`/`ScrollOpen`/`ScrollReadClosed`/`TutorialHook::MeridianPrimerGranted`/`emit_scroll_open`/`grant_meridian_primer_once`
- **agent**：`server-data.ts` ScrollOpen TypeBox + samples
- **client**：`ProtoServerDataBridge`(scroll_open)/`ScrollOpenHandler`/`ScrollReadStore`/`ScrollReadScreen`/`ScrollVanillaIconMap`/`ScrollOpenGlowPlayer`/`InspectScreen` READ_SCROLL/`ClientRequestSender.sendScrollReadRequest`

### 博弈自检
- Verify round-1：opus needs_fix，唯一 blocker = **读卷链路对玩家不可达**（收端全套建了、发起端缺失=死代码孤岛，`feedback_spawn_chain_wiring` 经典）→ 补 `InspectScreen [阅读]` 触发端(`73c8d65b6`)。
- round-2：控方 charges=[]、端到端 consumable=true，opus 复核发起端真接线端到端通(右键→C2S→handler→ScrollOpen→Screen 无断点) → **verdict=ready，defenseWins=true**。

### 遗留 / 后续
- §9 契约草案 `instance_id: string` → 实际落地 `u64`（对齐全仓惯例，已回写 §9）。
- 残卷正文（§8.1 #3 代拟）供用户过目；worldview↔`topology.rs` 经脉邻接有 pre-existing 漂移（肺经代码邻接大肠+肝、worldview 文本大肠+心，非本 plan 引入）。
- §6 复用清单登记的候选（丹方预览/黑武士遗书/图书馆残页）未实施，未来挂同一 `readable_scroll_spec` 链路。
