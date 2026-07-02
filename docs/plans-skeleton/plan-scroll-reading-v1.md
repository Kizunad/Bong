# plan-scroll-reading-v1 — 可阅读卷轴：经脉入门残卷 + 通用读卷 UI + 阅读动作

> **一句话主题**：给新手进服发一卷《经脉浅述·残卷》（正文摘自 library 正典），落地一条**可复用**的"可阅读物品"链路——server `readable_scroll_spec` + S2C `scroll_open` payload + client 卷轴阅读屏（owo，滚动长文）+ PlayerAnimator 双手持卷阅读循环姿态 + 手持白嫖 paper 模型——后续任何书信/残卷/图谱直接挂同一链路。

**状态**：骨架（skeleton）。升 active 前按 docs/CLAUDE.md §五 收口 §8。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | server 底盘——物品模板 / C2S 读取 / S2C scroll_open 契约 / 首入发放 | ⬜ |
| P1 | client 阅读屏——bridge 接缝 + Store/Opener/Screen（可复用壳） | ⬜ |
| P2 | 视听——阅读循环动画 / 手持模型 / 展开微光 / SFX / 图标 | ⬜ |

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
- **跨仓库契约**（并行实施前锁死，见 §9）：proto `envelope.proto` C2S 新 oneof `scroll_read_request`、S2C 新 oneof `scroll_open`（tag 取下一空闲号）；TS schema `client-request.ts` / `server-data.ts` + `samples/*.json` 双端对拍；client 桥两处 + `ScrollOpenHandler` 注册进 `ServerDataRouter`。
- **worldview 锚点**：§三 修炼体系（12 正经 + 8 奇经正典，`worldview.md:59-119`）；末法残卷命名惯例（《×××·残页/残卷》，onboarding_scrolls 先例）；`plan-spawn-tutorial-v1` O.13 沉默引导——**发放静默入包、阅读由玩家自发**，不弹强制引导。
- **qi_physics 锚点**：**无灵气流动**——纯阅读，不扣不还不衰减，不触 ledger。显式声明以过红旗清单。

---

## P0 server 底盘 ⬜

- `ItemTemplate` 加 `readable_scroll_spec: Option<ReadableScrollSpec>`（`inventory/mod.rs:140` 现有 `*_scroll_spec` 族旁）：`{ title: String, body_pages: Vec<String>, anim_id: Option<String> }`。**读不消耗**（区别于 technique_scroll 消耗式）。
- 新物品 `scroll_meridian_primer`《经脉浅述·残卷》进 `onboarding_scrolls.toml`（`category="scroll"`, grid 1×2, max_stack=1），正文从 `经脉浅述.json` 摘编 3~5 页（每页 ≤ 500 字，具体分页见 §8 #3）。
- C2S `ClientRequestV1::ScrollReadRequest { v, instance_id }`：handler 查实例模板有 `readable_scroll_spec` → emit S2C `ScrollOpen { scroll_id, title, body_pages }` + `play_anim`；无 spec / 非本人物品 → 静默拒绝 + warn。
- S2C `ServerDataPayloadV1::ScrollOpen`（proto/serde/TS/samples 四件套，落点清单照 `reference_server_data_payload_field` 模式扩到新 variant）。
- **首入发放**：join 时 `TutorialState.has(TutorialHook::MeridianPrimerGranted)` 判定，未发则 `add_item_to_player_inventory` + 打 hook（存量老玩家也补发；发放静默，无 narration，对齐沉默引导）。发放方式最终拍板见 §8 #1。
- **测试**：spec 解析正反 sample；ScrollReadRequest happy/无 spec/伪 instance_id/重复请求；发放幂等（重连不重发、老玩家补发一次）；payload roundtrip 对拍。

## P1 client 阅读屏（可复用壳）⬜

范式 A 全套（仿 DeathScreen 链）：

- `ProtoServerDataBridge` 两处：`CASE_TO_TYPE` 加 `SCROLL_OPEN → "scroll_open"`、`extractInner` 加 case。
- 新 `ScrollOpenHandler`（进 `ServerDataRouter`）→ 新 `ScrollReadStore` → 新 `ScrollReadScreenBootstrap`（tick poll → `setScreen`）。
- `ScrollReadScreen extends BaseOwoScreen<FlowLayout>`：竖排卷轴质感面板，正文 `Components.label(...).maxWidth(px)` 自动换行（中文换行活先例 `InspectScreen.java:772`）+ `Containers.verticalScroll(Sizing.fill(100), **Sizing.fixed(viewportH)**, content)`；多页时翻页按钮。**两个已知坑必须遵守**：viewport 高度必须 `Sizing.fixed`（fill 顶飞坑）；翻页刷新用 diff 原地更新不 `clearChildren`（滚动回弹坑，照抄 `CraftRecipeListWidget.java:109-121` 模式）。ESC/关闭按钮退出，`shouldPause()=false`。
- **可复用性验收**：屏的输入只依赖 `ScrollOpen` payload 字段（title/pages），不 hardcode 经脉内容——第二卷任意 readable scroll 零 client 改动可读。
- **测试**：Handler 缺字段丢弃分支；Store 生命周期（open→close→再 open）；分页边界（1 页 / 最大页 / 空页拒绝）。

## P2 视听 ⬜

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

## §9 契约锁定（并行实施硬约束）

server/client 并行开工前，proto 两个新 oneof（`scroll_read_request` / `scroll_open`）的字段名、类型、tag 号必须先在本 plan 定稿并出 samples；实施 agent 不得擅改契约、不得擅自 commit 跨端接缝（[[feedback_parallel_impl_locked_contract]]）。

## §10（升 active 时补）

scope 预估 3 PR：PR-1 = P0（含契约四件套）、PR-2 = P1、PR-3 = P2（视听资产，图标走 gen-image 批产 + sha1 同步）。按 docs/CLAUDE.md §六补完整工作流章节。
