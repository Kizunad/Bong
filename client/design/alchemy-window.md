# 炼丹窗口：需求、请求口与重做验收

**2026-09-28 交付：用户完成 Windows Native 联网验收，确认提交当前版本。** 以下交付清单描述最终实现；后续编号章节保留迭代历史，其中“等待模型合并”“尚未联网验收”和旧操作方案不代表最终状态。

## 当前交付与代码职责

| 用户要求 | 实现与入口 |
| --- | --- |
| 工位背景、独立部位悬浮、统一 G 交互 | `AlchemyWindows` 管理单炉生命周期；`AlchemyWindowContent`、`AlchemyFurnaceComponent` 负责部位交互及集中快捷键提示 |
| 丹方／炉记独立背景、纸页裁剪、小尺寸 | `AlchemyNotesContent`、`AlchemyPaperSurface` 与 `AlchemyWorkspaceLayout`；支持 100×100 面板与背景缩放 |
| 原作者模型、四脚朝外、开盖与沸腾／失败动画 | PR #2310 模型经 `export_alchemy_furnace.cjs` 导出；包含投料、微震、提前收取、残丹、炉渣、炸炉表现 |
| 直接拖料、中键量化分堆 | 投料取拖动数量与阶段剩余需求较小值；Inspect 数量选择后附着鼠标，普通容器通过带 `count` 的 move 请求原子分堆 |
| 火焰、材料染色烟气、注元与分类音效 | `AlchemyWorldEffects`、`AlchemyWorldRenderer` 和服务端 `world_effects.rs`；附近玩家不开 UI 也能看到动作并听到音效 |
| 权威炉次和真实付费注元 | `session.rs` 推进炉次及香料燃烧；`manual_qi.rs` 经账本事务付费，失败不扣费；关闭 UI 不结算 |
| 提前收取、成品呈现、错误可见 | 二次确认提前收取，生成炮制药渣并按炉温灼伤；结果开盖呈现并显示名称，领域错误显示在工位内 |
| 普通计时香、真实配方、OP 背包 | 场景提供灵息丸、灵草和普通香料；`inventory/operator.rs` 按权限授予 12×12 背包，撤权溢出与库存原子持久化 |
| 离炉关闭和 Windows 原生测试 | Inspect 每 tick 校验工位可达性；移除 HMCL 启动入口，服务端 ready 后由 Java 17 直接启动 Fabric |

请求沿用 `AlchemyIntent` → `ClientRequestProtocol` → `network/client_request/production.rs`，服务器校验距离、维度、炉主、当前施术者及库存。新增 `alchemy_place_incense {furnace_pos,item_instance_id}`；`inventory_move_intent` 的可选 `count` 表示分堆数量。`alchemy_session` 包含结构化阶段材料与可选 `incense`；`alchemy_world` 承载炉坐标、火候、材料、香状态及一次性动作，声音通过既有 audio/play 广播。Rust、TypeBox、protobuf 与 Fabric 同步维护。

运行 `bash scripts/alchemy-test-server.sh` 启动当前工作树的验收服，显式启用 TEST_ENV 与本地离线 OP。`/scene test_alchemy_furnace_1` 准备真实丹炉和材料，并把炉位所在区域灵气补到至少 0.8（门槛 0.3）；重复执行不降低更高灵气、不重复造炉。开发场景明确绕过自然灵气规则，生产起炉门槛保持有效。资源包需由本地 HTTP 服务提供，默认 `127.0.0.1:18765`。

`production_scene_alchemy_furnace` 已实际验证命令注册、开窗、起炉和收取；`production_alchemy_brew_pill` 覆盖完整配方及旁观者。客户端回归保护窗口生命周期、权威快照、分堆和结果协议；服务端回归保护库存原子性、香效时段、注元付费及场景区域选择。

**本次边界**：通用设施摆放／收纳／破坏抽象和混合兽等生物行为整理按用户要求后置。真实残卷学习入口、自动炼丹和炉实体持久化仍不在本次完成范围。炉记是连接内历史；协议尚无逐请求关联 ID。正常炼制沿用已有成色分桶，提前收取固定产生中间药渣。

## 历史迭代记录

2026-09-20，`feat/r7-alchemy-window`。本记录覆盖当前仓库的炼丹操作闭环，不代表整个炼丹领域已完成。功能参考 `ForgeWindows` 的工位生命周期、权威状态、拖放、请求等待和错误反馈。最新方向为独立工位背景、部位悬停交互与独立丹方窗口，详见 §7；§4 和 §6 保留上一轮被替代布局的问题与验证记录。

## 1. 功能需求与交付状态

| 环节 | 必需功能 | 当前结果 |
| --- | --- | --- |
| 放炉、进入 | 从真实炉物品放置；交互已有炉；识别炉主、阶数、完整度；校验可达性 | 复用既有入口；窗口重开等待炉信息和炉次快照 |
| 丹方册 | 独立阅读、翻页、研读残卷；展示材料、数量、阶段时机、炉阶要求、火候、真元与成丹指导 | 独立丹方窗口与背包双击/右键已接；正文由权威配方与物品表生成；真实 fragment 学习仍缺，详见 §8 |
| 起炉 | 无活动炉次且选定丹方才可操作；服务器确认后进入活动状态 | 炉位悬停按 I；发送后等待并锁定重复请求 |
| 投药 | 炉口拖放、选择阶段、调整总量、校验时机、读取实时库存；状态更新保留数量 | 减号/加号调整数量，方括号换阶段；支持同材料跨堆总量，过早、过晚、错过阶段本地拒绝；最终资格由服务器判定 |
| 香座 | 从背包拖入香料；不同香料影响火候容差、真元消耗/收益并消耗一个实例 | `incense_clear_mind`、`incense_warm_ash`、`incense_damp_wood` 已由服务端按模板计算；客户端等待库存快照确认，不本地扣除 |
| 火候、真元 | 查看当前值、目标、容差；升降火、注元；未起炉禁用 | 已接；调温每次 ±0.02，注元每次 1；服务端注元扣费缺口见 §5 |
| 炉次进展 | 阶段时机、完成/错过状态、温度、真元和进度；关闭后继续，再次交互恢复 | 服务端每 tick 推进 session，每秒向炉主周期推送权威快照 |
| 收取结果 | 主动取出结果；提前收取二次确认；显示成功/残丹/废丹/炸炉和库存结果 | 完成收取走整炉结算；提前收取保留当前进度，生成炮制药渣并按炉温施加小额灼伤；三秒内松开再按 R 确认 |
| 炉记 | 最近结果、成色、含毒量、服用真元收益、副作用、残方、炸炉伤害、经脉裂伤；炼丹技艺 | 已接；可选字段缺失时不伪造零值；沿用最多 20 条本次连接历史 |
| 成丹预测、丹毒 | 有权威数据时展示，无数据时明确缺省 | 去掉默认演示数据；窗口暂不展示缺乏生产供给的预测；丹毒无数据时显示缺省说明 |
| 服丹 | 从实际背包选择丹药使用，不能把炉记文本当成物品实例 | 复用背包 `apply_pill` 入口，不在炉内直接服丹 |
| 窗口 | 拖动、尺寸输入、最小化、恢复、关闭；宽窄自适应；悬停炉况、局部快捷键 | STATION 不固定为 HUD；最小 300×230；宽窄均保留整个工位，丹方/炉记另开窗口 |
| 反馈 | 请求等待、重复提交保护、拒绝原因、超时重试；不能以发包成功代替业务成功 | 已接本地反馈和 5 秒超时；服务器领域拒绝目前主要在聊天，尚无结构化 ack |
| 自动炼丹 | 只有实际启用自动方案、材料与真元供给均有权威契约时才提供入口 | 当前 `auto_profile` 干预为空操作，窗口不提供按钮 |

`completed` 仅表示阶段投过药，不能等同该阶段所有药材齐全。投料始终先起炉后投入；数量须等于丹方要求的一味总量。当前客户端 intent 校验阶段索引 0–3，服务端字段是 `u8`，四阶段限制不是 wire 自身上限。

## 2. 请求口

所有下列 JSON 请求带 `v: 1`，由 `ClientRequestProtocol` / `ClientRequestSender` 走现有 `bong:client_request`。`furnace_pos` 是 `[x,y,z]`。服务端统一入口为 `network/client_request/production.rs`，具体 handler 主要在 `network/client_request_handler.rs`。

| 功能 | type 与参数 | 返回及接入情况 |
| --- | --- | --- |
| 放置炉 | `alchemy_furnace_place {x,y,z,item_instance_id}` | 炉物品放置入口；服务器验证物品并建炉 |
| 打开、重新同步 | `alchemy_open_furnace {furnace_pos}` | `alchemy_furnace`、`alchemy_session`、`alchemy_recipe_book` |
| 翻页 | `alchemy_turn_page {delta}` | `alchemy_recipe_book`；服务器循环翻页；客户端不本地改页 |
| 旧丹方学习 | `alchemy_learn_recipe {recipe_id}` | `alchemy_recipe_book`；现客户端识别 `recipe_scroll_` 前缀；服务器不消费残卷，不能等同碎片研读 |
| 实例残卷学习 | `alchemy_learn_recipe_fragment {item_instance_id}` | Rust / protobuf 已有，Fabric sender 和 TypeBox 未接；事件学习并消耗实例，handler 未直接推送丹方和库存；当前真实入门残卷不能走旧前缀入口 |
| 起炉 | `alchemy_ignite {furnace_pos,recipe_id}` | `alchemy_furnace`、`alchemy_session`；起炉 handler 本身不推丹方书 |
| 投料 | `alchemy_feed_slot {furnace_pos,slot_idx,material,count}` | `alchemy_session`、库存；`slot_idx` 是阶段；按材料类型聚合扣除，未携带 instance_id |
| 投香 | `alchemy_place_incense {furnace_pos,item_instance_id}` | 服务端校验香料模板、炉主、炉次和香座状态，原子消耗一个实例后推送 session 与库存 |
| 降火、升火 | `alchemy_intervention {furnace_pos,intervention:{kind:"adjust_temp",temp}}` | `alchemy_session`；温度范围 0–1 |
| 注元 | `alchemy_intervention {furnace_pos,intervention:{kind:"inject_qi",qi}}` | `alchemy_session`；窗口每次请求 1 |
| 自动方案 | `alchemy_intervention {furnace_pos,intervention:{kind:"auto_profile",profile_id}}` | schema 保留；当前 session 不执行方案切换，窗口不发送 |
| 收丹、提前收取 | `alchemy_take_back {furnace_pos,slot_idx:0}` | 终态 session、空炉、结果和库存；炉次未到时不再快进，生成 `alchemy_residue_processing_dregs` 并记录灼伤 |
| 旧服丹 | `alchemy_take_pill {pill_item_id}` | 按模板选择丹药的既有入口 |
| 背包服丹 | `apply_pill {instance_id,target:{kind:"self"}}` | 背包现用实例入口；服务端内部转入同一服丹处理 |
| 拖动、缩放、最小化、关闭 | 无领域请求 | 关闭不收丹，也不取消炉次 |

请求编码：`client/src/main/java/com/bong/client/network/ClientRequestProtocol.java`。
实例残卷定义：`server/src/schema/client_request.rs::AlchemyLearnRecipeFragment`、`proto/bong/envelope.proto`；生产残卷例：`server/assets/items/onboarding_scrolls.toml::fragment_alchemy_hui_yuan_pill`。

## 3. 返回数据

| 返回 | 必需数据 | 当前约束 |
| --- | --- | --- |
| `alchemy_furnace` | 坐标、阶数、完整度、炉主、是否有炉次 | 窗口核对目标坐标；客户端还核验 5 格距离和方块 |
| `alchemy_recipe_book` | 当前页、已学丹方 id、名称、正文、作者 | 生产 emitter 从配方与物品表输出名称、材料和炼制要求；无来源的作者与时代留空 |
| `alchemy_session` | active、recipe_id、elapsed/target、火候目标/容差、真元目标、stage 时机/summary/completed/missed、近期干预、香料投影 | 香料以 `§7[香]kind|remaining_ticks|duration_ticks|temp_band_scale|qi_cost_scale` 追加在近期干预中；没有炉坐标/session_id，没有每味已投数量 |
| `alchemy_outcome_resolved` | 分桶、丹药、quality、toxin_amount/color、qi_gain、副作用、残方、damage、meridian_crack | 本次补齐 handler→history→炉记的可选数值；历史为连接内缓存 |
| `alchemy_outcome_forecast` | 各档概率、预估说明 | 查到的供给来自显式 join mock；不显示演示概率 |
| `alchemy_contamination` | 各类丹毒当前/上限/预警、代谢说明 | 同样缺少正常生产推送；现 Store 仅投影醇、烈两类 |
| inventory / skill | 真实材料、产物、真元、炼丹技艺等级和熟练度 | 不以本地副本冒充扣料和学习成功 |
| 聊天拒绝 | `[炼丹] ...` | 没有 request_id 与结构化 ack；有些旧路径只写服务端日志 |

## 4. 上一轮布局及已修问题（视觉结构已被 §7 替代）

```text
┌ 炼丹窗口标题及窗口控制 ────────────────────────┐
│ 炉主 / 炉阶 / 完整度                          │
│ 当前炉况                                     │
├──────────────┬───────────────────────────────┤
│              │ 丹方       投料       炉记     │
│ 固定炉景     ├───────────────────────────────┤
│ 权威进度     │ 独立滚动的正文、明确拖放区     │
│              │ 阶段时机、配方或结果          │
├──────────────┴───────────────────────────────┤
│ 投料数量              降火            升火   │
│ 起炉                  注元            收丹   │
│ 等待 / 拒绝 / 提前收丹确认                    │
└──────────────────────────────────────────────┘
```

内容宽度不足 500 时收起装饰炉景，工作笺占满宽度；窗口内所有业务功能保留。操作区两排三列对齐。炉主长名称用 tooltip 保留全文，正文滚动不带走操作区。

| 已确认旧问题 | 修复 |
| --- | --- |
| 首次 mount 前炉景已经进入正文，又被宽窗加进另一个容器；两处共用组件，后 mount 覆盖坐标 | 炉景只属于 art-slot；窄窗从该位置收起，绝不放进正文 |
| 旧预览只查屏幕越界，未检测左右重叠 | 原生检查增加左右、上下不相交，操作列对齐、顶部文字、宽窄恢复及输入身份检查 |
| 原截图“丹炉尚未确认”未被旧预览发现 | 预览检查有效炉坐标与丹方；该状态根因尚未单独复现，不归因于布局 |
| 未起炉仍能调火；投料不检查时间窗口，服务器会记 missed | 操作按炉况启用；本地拒绝窗口外投药 |
| 学方、翻页在发包后乐观修改 Store | 仅由权威 recipe_book 更新 |
| 无 pending / 超时，重复点击消耗类操作；任意库存更新都可能被当作投料确认 | 等相关状态变化或超时；投料核对材料总量减少，骨币等无关更新不解锁 |
| 投料限制为单堆数量，与服务器聚合语义不符 | 检查身上同类总量，覆盖容器、hotbar 和装备；材料资格仍由服务端核验 |
| 收丹确认跨炉保留、反馈变化不刷新 | 身份和炉次变更撤销确认，pending/feedback 变化独立刷新 |
| 整片正文吃拖放，滚动条也会发投料 | 仅明确且当前可见的拖放区域接收 |
| 炉景覆盖父层 scissor | 使用 owo ScissorStack 嵌套裁剪 |
| 默认成丹概率和丹毒是演示数据 | 初始/缺省快照中性；无预测时 HUD 不输出零概率断言 |
| wire 有成色/毒量/伤害，history 丢弃 | 补齐可选字段，保留未知语义 |

实现保持窗口所有者、内容布局、绘制、网络映射各自职责；复用现有窗口管理和 Store，不引入第二套库存/配方缓存，不硬编码配方。

## 5. 首次审查发现的领域与协议问题（当前状态见 §8）

以下为本次源码审查确认的剩余缺口；客户端测试通过不能替代服务端链路验证。

1. **P0：注元守恒（已由 #2355 接入）。** 生产请求由 `settle_alchemy_inject_qi_requests`（`server/src/network/client_request_handler.rs:5530`）调用 `debit_player_qi_to_furnace`（`server/src/alchemy/qi.rs`），先以 `QiTransferReason::Crafting` 通过 `WorldQiAccount` 完成玩家到炉体的转移，成功后才更新 `Cultivation.qi_current`、`AlchemySession.qi_reserved` 与 `qi_injected`；退款走 `refund_furnace_qi_to_player`，无法回到在线玩家时走 `release_furnace_qi_to_overflow`。`server/src/alchemy/qi.rs` 的 `paid_injection_moves_qi_without_changing_world_total`、`rejected_injection_leaves_player_session_and_ledger_unchanged` 和 `inject_request_system_commits_player_and_furnace_together` 测试以 `SPIRIT_QI_TOTAL`、`summarize_world_qi`、`assert_conservation` 锁住成功、拒绝和同帧路径。客户端只提交原有 InjectQi 请求，不自建第二套扣款逻辑。
2. **P1：学习和开炉权限不完整。** 旧 `handle_alchemy_learn` 按 recipe_id 学习，没有残卷所有权/消费；`handle_alchemy_ignite` 校验配方存在、区域灵气和炉阶，却未检查 LearnedRecipes。普通客户端限制不能替代服务端授权。
3. **P1：真实残卷链路未闭环。** 入门残卷 `fragment_alchemy_hui_yuan_pill` 不匹配当前 `recipe_scroll_` 前缀。新 fragment 请求只存在 Rust/protobuf，缺 Fabric sender、物品语义投影与 TypeBox；事件处理未直接回传丹方和库存，消费失败时也未回滚已学习状态。需要单独完成跨栈契约。
4. **P1：投药无法判断同味是否已经足量。** session 只给 completed 布尔；服务端 feed_stage 对同味继续累加，客户端不能只凭该布尔禁止重复材料，否则会误伤同阶段其他药材。需要每阶段每味已投入量与服务器幂等/重复提交约束。
5. **P1：工位操作距离没有服务端保证。** `with_owned_furnace_mut_with_entity` 校验坐标存在及 owner，但不读取玩家位置；客户端 5 格限制不是服务端距离门禁。
6. **P1：生产丹方不可完整阅读。** `send_recipe_book_from_learned` 只有 id 和占位 body；预览中的完整丹方是夹具，不能代表联网结果。应从 RecipeRegistry 输出完整展示和结构化材料/火候条件。
7. **P2：跨炉晚到消息缺少身份。** session/outcome 无炉坐标和 session_id，本客户端只持有一个炉次投影；等待新对象不能等同可靠的响应关联。
8. **P2：预测/丹毒及反馈缺失。** 正常生产缺预估和丹毒推送，拒绝无结构化 ack；pending 只能按相关状态变化判断，不能提供请求级恰好一次保证。

## 6. 上一轮验证证据（不能作为 §7 的验收）

旧版失败复现：Windows 原生 `item-windows-check-20260920-150153` 在第 3 张宽窗检查失败，炉景和工作笺同为 x=373。独立冷启动 `item-windows-check-20260920-153015` 也失败，说明不是连续截图独有现象。

最终版本：Java 17 经 `scripts/build-token.sh` 执行 `gradle test build --offline --no-daemon --console=plain` 通过。577 个测试套件、5,031 条 JUnit、3 条 Fabric GameTest，0 失败、0 跳过；构建日志 `local_images/r7-alchemy/redesign-final-build.log`。`git diff --check` 通过。

Windows 原生五场景 `item-windows-check-20260920-155344`：`status=passed, completed=5`。包含最小窗口、投料滚动、宽窗丹方/投料/炉记；验证同窗宽窄恢复、数量输入保持、明确区域投药及 intent 参数、滚动条不投药、上下左右区域不相交、操作列对齐。单独宽窗冷启动 `item-windows-check-20260920-155426`：`status=passed, completed=1`。二者位于 `D:/Minecraft/.minecraft/Fabric_Bang_Test/bong-native/`，每次独立保存截图、日志和结果文件。

Windows mods 内 jar 与本地最终构建 SHA-256 相同：`217a4aecabef55dee49594b34d7870210aaf4541372fa6dbe31b833898f5c5dc`。原生启动日志仍有离线账号认证及其他资源加载告警，不能据此称客户端全局无告警。

本轮未启动真实服务端，也未进行联网炼丹流程验收；预览 recorder 接收 intent，不发送真实炼丹请求。静态截图和几何断言不能替代用户的外观验收。

## 7. 沉浸工位重做：最新要求与实现边界

用户明确选择独立工位背景，中央丹炉、左侧桌面；丹方通过背包双击或右键独立阅读。炉况悬停出现，操作主要通过窗口作用域的快捷键完成。用户要求可交互部位足够多。模型以 PR #2310 合入后的版本为准；合并前跳过模型，不使用当前旧炉替代。

- 背景通过已获授权的 Cliproxy / gpt-image-2 生成，参考 Forge 的 workbench.png。源图和提示词为 `local_images/r7-alchemy/alchemy-workbench-v2.png`、`alchemy-workbench-v2_prompt.md`，生产资源为 `assets/bong-client/textures/gui/alchemy/workbench-v2.png`。背景不烘焙丹炉、火焰、烟气和操作文字。
- `AlchemyFurnaceComponent.Part` 定义炉盖、炉口、炉身、火门、引元处、出料处、香座、桌上丹方、桌上炉记九处交互区。当前使用炉位与轻量标记，属于模型未接入前的临时呈现；后续必须依真实模型的部件位置校准，不能宣称已完成模型交互。
- 炉口接收背包材料；火门滚轮调火；引元处点击注元；出料处点击收取结果；两处桌面分别打开丹方和炉记。炉盖与炉身提供悬停察看。悬浮信息区拦截点击穿透，过长信息可滚动。
- 香座也是独立部位，悬停显示当前香种、余香、火候容差和真元消耗倍率；背包香料拖到香座后才发 `alchemy_place_incense`，服务端库存快照确认扣除。
- 拖料时浮窗收起并高亮炉口，避免窄窗遮住落点；未拖料时，浮窗同时阻止点击和材料落点穿透。投料数量显示在工位底部。拖料期间只保留数量和阶段快捷键，不触发起炉、注元或结算。
- 快捷键只在聚焦工位且悬停具体部位、未编辑文本时生效：J/K 降火/升火、F 注元、I 起炉、R 收取结果、H 查看投香提示、N 丹方，减号/加号调整数量，左右方括号切换阶段。R 二次确认会明确提示炉火未停可能烫伤并损失材料，三秒过期；拖动物品的 R 仍属背包旋转。
- 收取确认固定显示在工位底部，不随炉况长文滚出视野；失焦、最小化、离开炉位或开始拖料都会撤销确认。带修饰键的输入不作为单键炉位操作，保留 Shift 加号以调整数量。
- `AlchemyNotesContent` / `alchemy-notes.xml` 为独立窗口，支持离炉阅读、翻阅已学丹方、选择当前炉方、查看原有炉记与结果字段。浏览页码本地独立，选择炉方才发既有 TurnPage 请求。背包 `recipe_scroll_*` 与 `fragment_alchemy_*` 双击/右键可以阅读；真实 fragment 只显示已有物品正文，不推导 recipe_id，也不伪造尚未接通的学习请求。
- 药烟由相关材料总量减少触发，按图标有效像素的透明度加权平均色染色；注元特效由 qiInjected 增加触发。重复快照不重播，资源重载清色彩缓存。缺少 request_id 的限制仍成立，不能把库存差分称为严格请求确认。阶段变为 missed 不再误判投料成功。
- 火焰强度跟随权威火候，不推算权威进度。当前背景、火焰、药烟、真元表现均独立于将来的炉模型。

### 尚需服务端实现的新玩法

“收取结果”可以在炉子工作中执行。服务端现在区分完整收取和提前收取：未到目标时不补 tick，转移一个带 `PillResidue` 元数据的炮制药渣实例，并按当前火候给玩家记录胸口灼伤；正常到时仍使用既有 resolver。客户端二次确认继续明确提示余热风险。阶段级更细的中间产物配方、灼伤抗性和 QiTransfer 结算仍需单独设计。

### 本轮验证

Java 17 经 build-token 执行 `gradle test build --offline --no-daemon --console=plain` 通过：577 个 JUnit 套件、5,031 条测试，0 失败/错误/跳过；3 条 Fabric GameTest 全部通过。日志 `local_images/r7-alchemy/immersive-final-build.log`。`git diff --check` 通过。

Windows 原生 `item-windows-check-20260920-175959`：`status=passed, completed=6`。覆盖最小工位、最小悬停窗、宽窗环境、炉口拖料与确认后特效、R 长按不能结算/松开再按才结算、左桌打开独立丹方。缩窗再恢复不移除工位，离开炉位不消费快捷键，非炉口不接收材料。截图与 recorder 验证来自显式夹具，不是联网业务验收；外观仍待用户审阅。

Windows mods 与本地 jar 的 SHA-256 同为 `aec062e1643918752d7a6511dffbc2f8397069f034c9edee27676670c3fb62a2`。截至本轮结束，PR #2310 仍为 OPEN，模型、骨骼动画资源保持未修改。

### 服务端增量（2026-09-21）

- `AlchemySession` 已由服务端每 tick 推进；每 20 tick 向炉主推送一次 `alchemy_session` 权威快照，客户端不再靠本地计时推进炉次。
- `alchemy_take_back` 在目标时刻前不再快进：收取会生成带 `PillResidue::ProcessingDregs` 元数据的 `alchemy_residue_processing_dregs`，并按当前火候给玩家记录胸口灼伤；正常到时仍走原有 resolver。
- 服务端炼丹测试集合为 283 passed、1 ignored；提前收取药渣/灼伤回归、炉次 tick 推进回归、`cargo clippy --all-targets -- -D warnings` 和 `git diff --check` 均通过。

### 交互边界复查（2026-09-20）

修复浮窗覆盖炉口时拖料穿透、窄窗提前收取提示被长文挤出、失焦后确认残留，以及 Shift+F 误触注元。拖料期间浮窗让开并突出炉口，关闭其他窗口不再由预览夹具持续拦截炉位输入。修正预览夹具的焦点、鼠标捕获和窗口层级，使用同一窗口管理器命中结果分发输入。

最终 Java 17 `gradle test build --offline --no-daemon --console=plain` 通过（5,031 JUnit、3 GameTest，0 失败），日志 `local_images/r7-alchemy/interaction-boundaries-final-build.log`；`git diff --check` 通过。

Windows 原生 `item-windows-check-20260920-184157`：`status=passed, completed=7`。新增最小窗炉口被浮窗覆盖时不得投料、进入拖料模式后可以投料、拖料中 F 不注元、Shift+F 不注元、失焦撤销收取确认、丹方最小化/关闭/重开后重新点击炉位恢复输入。仍为显式 fixture，不替代联网验收。最终本地及 Windows jar SHA-256 同为 `f5666a44551b4ba81c3017a0b1f2cc7f97ab546edf8689e2798d821ef15060f0`。PR #2310 仍 OPEN。

## 8. 合并主线后的代码复查（2026-09-21）

本地 merge `f50d1d6d4` 已包含 main `9974ad9e4`，包括混合兽模型替换与 PR #2310 的作者资产。之前“新丹炉已接入”的判断不成立：PR #2310 只更新了 bbmodel，运行时仍是旧几何。本轮通过 `modelScript/exporters/export_alchemy_furnace.cjs` 复用既有 Blockbench codec，完整导出 165 个 cube、7 个分组及嵌入贴图，新增开合、沸腾和投料动画。作者 bbmodel 不改。

| 复查问题 | 修正与验证边界 |
| --- | --- |
| 香效在燃尽时全部消失，最后一刻点香追溯影响全炉 | 记录实际燃烧区间；温度按当时香效判断，真元需求与收益按实际燃烧比例加权 |
| 香状态编码在聊天样式的干预日志里 | Rust、protobuf、TypeBox 和 Fabric 改为可选结构化 `incense`；旧快照可缺省 |
| 库存减少就误判投香成功 | 必须同时收到对应香种的权威炉次状态；客户端不扣库存、不自行推进计时 |
| 注元只有累计数值，没有真实付费 | `manual_qi` 独立系统调用 `Cultivation.release_to_zone` 原子事务；缺账本、非法值、非本炉施术者均无副作用 |
| 公共炉能操作他人的炉次，客户端能隔空请求 | 复用 `DistanceRule::NEARBY_INTERACT`，验证主世界、距离、炉主与当前施术者 |
| 首料窗口为第 0 刻，玩家还没拖入就错过 | 首料齐备前停留在第 0 刻，快照明确显示“待投首料” |
| 炉次无限计时与记录温度；完成后 UI 无法收取 | 到时停止采样，保留结果；收取条件使用 `has_session`，只在权威空炉快照后确认完成 |
| 背包放不下或编号器缺失时丢弃结果 | 保留停止的炉次供重试；入袋成功后才结算提前收取的灼伤 |
| 冷炉也扣灼伤；致死灼伤不触发死亡流程 | 低温不伤害，伤害遵守 GameMode，跨过零血量时发送标准 `DeathEvent` |
| 冷炉因温差绝对值被判“过热炸炉” | 过热仅判断高于目标温度的偏差；低温仍可能炼制失败 |
| 背景等比裁剪，热区按窗口比例定位 | 背景、模型、粒子与热区共用完整画布的等比缩放坐标 |
| 丹方因进度更新不断重建，阅读位置被重置 | 仅展示内容变化时替换正文；请求反馈和按钮状态独立刷新 |
| 丹方正文只有占位文本 | 从权威配方与物品表生成名称、材料数量、阶段时间窗、炉阶、温度、时长与真元要求 |
| 占位炉圈、空 layout、投香全 session/库存克隆回滚 | 替换为真实模型；删除空调用；投香先完成校验，再原子消费与赋值 |

模型资源测试移除了旧骨名字符串白名单，保留“动画确实引用骨骼，且所有引用都存在于几何”的资源契约，避免正确更换骨架后被旧实现细节误报。

仍未包含在本轮修正中的旧系统问题：残卷学习所有权与消费事务、开炉 LearnedRecipes 授权、逐阶段逐材料投入数量快照，以及跨炉请求关联 ID/结构化拒绝回执。它们不能由界面测试证明已解决。旧自动炉储量入口也没有接入本次手动注元事务，当前窗口不调用该入口。

运行验收固定使用 Windows Java 17 + Fabric jar。`/scene` 保留 OP 权限；本地测试服将 `Admin` 与原生用户名 `炼丹验收` 加入显式 OP 名单。必须先确认服务端 ready，再启动 `bong-native/forge-interact-launch.ps1`，不经 HMCL。

### 本轮验证与本地验收环境

- Rust：`cargo fmt --check`、`cargo clippy --all-targets --offline -- -D warnings` 通过；完整 `cargo test --offline` 共 12,590 通过、0 失败、6 个既有忽略项。日志 `/tmp/bong-r7-review-server-final.log`。
- Java 17：完整 `gradle test build --offline` 通过，5,043 JUnit + 3 GameTest；最终预览准备条件修正后 `gradle build --offline` 再次通过。日志 `/tmp/bong-r7-review-client-readiness.log`。
- Schema：generate、build、check 和 913 项测试通过，日志 `/tmp/bong-r7-schema-review.log`。
- Windows 原生预览：7/7 场景通过，检查最小窗口、拖料防穿透、各部位快捷键边界、提前收取确认、丹方关闭/重开及模型加载失败。输出位于 Windows 实例的 `bong-native/alchemy-review-20260921/screenshots/`。这是自动输入与几何验证，不代表已经完成美术验收。
- 真实联网炼丹：专用 `BongR7Review` 账号通过放炉、数量拒绝、投料、付费注元、真实计时完成、产物入袋和旁观者结果隔离，日志 `/tmp/bong-r7-live-review-clean.log`。
- 场景和香：OP 通过 `/scene` 生成并打开真实丹炉；非 OP 收到权限拒绝；投香恰好消耗一份库存，protobuf 下发对应香种与 1.25 倍火候容差；提前收取获得炮制药渣，日志 `/tmp/bong-r7-scene-review.log`。
- 新 jar 与 Windows 实例 SHA-256 相同：`1916aa77f77dcce0aac9673527512fc05877dbe275a7679724d8dce4814287d3`。jar 内丹炉（7 bones / 165 cubes）和混合兽（74 bones / 719 cubes）与源码逐字节一致。

旧 `server/data/bong.db` 已持久化出生区 `realm_collapse`，会持续把灵气归零，不能用来验收正常起炉。本轮保留原存档，改用 `/tmp/bong-r7-acceptance-20260921/data/` 独立测试存档，仍监听 25565；服务端日志 `/tmp/bong-r7-acceptance-server-final.log`。测试服关闭远程资源包自动下发，使用新 jar 内的资源，避免旧发布包覆盖模型。

另确认 Windows Java 17 把 UTF-8 参数文件里的中文账号误读为 GBK 乱码。实例原生启动脚本现在读取 UTF-8 源文件，并生成系统代码页编码的 `launch-native.args`，使实际账号与 OP 名单一致。此实例脚本在仓库外，仓内说明见 `scripts/windows-client.md`。

## 9. 正式入口崩溃与动画补齐（2026-09-21）

正式入口曾对已经是 `alchemy-body` 的模板根节点再次调用 `childById`，得到 null 并在装配时崩溃。现在直接使用 adapter 根组件。旧 `alchemy-window` 预览独立装配窗口，因此此前 7/7 不能证明正式入口可用；新增 `alchemy-runtime` 场景用 OP `/scene test_alchemy_furnace_1` 创建真实丹炉，经全局交互路由、`AlchemyScreenBootstrap`、`UiWindowRuntime` 打开窗口，并等待真实炉讯。它还检查右键不再开窗、过期准星不派发、开窗后不继续派发世界交互。

丹炉入口现在与 Forge 共用可重绑定的交互键，默认 **G**。移除了实体及方块右键开窗逻辑，保留持有丹炉物品时的放置操作。服务端 `/scene` 提示同步改为默认 G。

工位相机俯仰归零，使炉体保持竖直。动画期间固定工位取景，避免顶盖弹起导致整个模型缩小、交互区域漂移。作者模型骨架与炉脚不作倾斜修改。

动画包含待机、炼制轻振、起炉开盖、停炉合盖、投料、正常收取、瑕丹、废丹、提前收取和炸炉。炉身五层同步轻振，炉盖另有小幅跳动；废丹泄出暗烟，提前收取释放蒸汽，炸炉表现为炉身冲击、顶盖弹起回落、火星和短暂局部闪光。世界实体的炸炉骨骼动画复用服务端 `alchemy_explode` VFX，空炉 metadata 后到也不会用合盖动作中断炸炉。

工位结果表现由本次收取请求后的权威空炉、结束进度和同丹方结算事件共同确认；提前收取以返回的实际结束进度判定。重绘、重复结果、窗口重开和超时消息不会主动重播。协议仍没有炉次 ID，因此既有同配方跨请求关联限制仍保留，不能宣称已经解决。服务端目前在收取时结算结果，动画遵循这一时机。

`alchemy-effects-ui-preview.json` 覆盖平视、炼制与各结果，保存连续渲染帧；这些结果是显式夹具，证明客户端表现路径，不能替代服务端随机结果与美术验收。

最终验证：Java 17 完整 `gradle test build --offline` 通过，5,051 JUnit + 3 GameTest；日志 `/tmp/bong-r7-alchemy-effects-client-final.log`。Rust fmt、clippy、完整测试与 build 通过，12,590 passed、0 failed、6 ignored；日志 `/tmp/bong-r7-alchemy-entry-{clippy,tests,build}.log`。Windows 新 jar SHA-256 为 `6414e8c05644b83b439650fa545c1f4ed45a7c846a1724290656fced6d814afc`，与本地一致。

Windows 正式联网入口回归 `alchemy-runtime-ui-preview/ui-preview-result.txt` 为 1/1 passed；服务端返回默认 G 提示并同步炉、炉次与丹方。`alchemy-effects-ui-preview/ui-preview-result.txt` 为 7/7 passed，各动画保存 20 帧及 GIF；运行时未记录模型或动画加载失败。服务端使用原独立验收存档重启，日志 `/tmp/bong-r7-acceptance-entry-server.log`。

## 10. 十项炼丹修订（2026-09-21）

本轮优先完成炼丹本身，摆放、收纳、破坏和实体公共化后置。

- 丹方、炉记各用独立纸质背景，移除标准深色外框。丹方保留正文翻阅；炉记按每炉结果写简短手记，保留成色、余毒与反噬提醒。
- 快捷键常驻工位底部：I 起炉、J/K 降升火、F 注元、R 收取结果、N 丹方、H 炉记。窗口聚焦即可操作；拖料和文字输入期间不触发。九个部位分别悬浮显示其状态。
- 正常同步、等待确认不显示。炉窗打开时 `[炼丹]` 消息转入窗口底部，关闭或最小化后保留聊天。服务端仍无请求 ID，不能宣称实现请求级恰好一次语义。
- `/scene test_alchemy_furnace_1` 选用真实灵息丸配方：灵草三份、一阶炉、三成火、真元五份、八十刻。场景准备灵草 24、草木香 4、干草 4、木板 2，仍要求 OP。
- 草木香 `incense_plain`：干草 2 + 木板 1，在制作台耗时十秒制得四支，不耗真元、不需解锁。每支消耗一个库存单位，计时六十秒，不改变火候容差、真元需求或成丹收益，初始灵气为零。
- 首料未齐与停炉待收期间香继续燃烧；收取结束当前炉次时也结束本炉香状态。香目前依附炉次，并非跨炉次独立摆件。
- 炉口拖入按 `min(所拖堆叠数量, 阶段尚缺数量)` 自动投料。新增 `AlchemyStageHint.ingredients`（protobuf tag 6），每味包含 material/required/inserted。Rust `StagedMaterials.by_stage` 记录分次投入，旧档按阶段分配累计材料，新炉次只有全料齐备才标阶段完成。
- 材料扣除与阶段投入量增加双重确认后，优先展示物品模型，缺模型则显示图标；物品飞向炉口后下沉，由炉身遮挡。连续炉底火焰、炉顶烟气随火候变化，投料产生材料颜色的药烟。
- 炉脚由生成器修正为四向外撇、脚掌水平共面，再经同一导出器更新运行时资源。只更新此资产的 golden 基线。

投料 wire 仍以模板聚合扣料，没有物品实例字段：数量遵守所拖堆叠，具体消费哪一堆由既有服务端库存路径决定。真实残卷学习事务、跨炉消息关联仍为既有后续问题。

验证结果待本轮门禁及原生回归完成后补齐。

## 11. 纸张窗口收尾（2026-09-22）

- `OwoXmlWindowContentAdapter` 使用根容器的 owo 裁剪，不再外套会被 `OwoUIAdapter.render()` 重置的原版 scissor。`ModelPreviewComponent` 改用同一 `ScissorStack`，使局部模型裁剪与父窗口取交集，并在退出时恢复父边界；裁剪跟随窗口矩阵缩放。
- 纸张顶部恢复拖动，右上角墨色合卷入口独立接收点击。重复启用纸张模式不重复拆除控件，标题更新不会访问已移除的标题栏。
- `AlchemyNotesContent` 按内边距、实际可见控件和行间距计算正文高度。炉记隐藏丹方导航，空反馈不占版面；切换阅读对象回到正文开头，正常快照刷新保留滚动内容。
- `UiAlchemyWindowPreviewScene` 修正两份灵草误标三份齐备的夹具。预览新增纸边拖动、真实点击合卷及重开、最小窗口长文滚动、正文内边距检查。`alchemy-window-ui-preview.json` 从 9 个场景扩为 11 个，增加最小丹方和最小炉记。

本轮验证：使用 Java 17 与本机缓存的命名空间依赖，独立编译炼丹客户端、炼丹网络处理、公共窗口适配器、模型预览及相关测试；JUnit 55 项通过，0 失败。编译与测试入口位于临时目录 `/tmp/bong-alchemy-check/`，未替换 Gradle 输出或 Windows jar。`git diff --check` 与预览 JSON 检查通过。

尚未完成的验收：本会话 Gradle 因无法取得可用的网络接口而不能启动，Windows 实例目录不在可写范围，因此未运行完整 `gradle test build`、GameTest 或这 11 个 Windows Native 预览。上述局部验证不证明 GPU 裁剪、纸张美术及动画已经通过原生验收。测试仍须按服务端 ready → Windows Native Java 17 + Fabric jar 顺序运行。

素材缺口：`incense_plain` 的独立背包图标仍未生成；Cliproxy 网络调用被当前沙箱拒绝，普通香的玩法与配方已有代码，但图标仍走缺省资源。设施收纳、破坏公共路径和生物行为整理继续后置。

## 12. 工房布局、独立纸页与本次验证边界（2026-09-22）

### 用户要求与实施边界

- 当前优先解决炼丹布局、纸页越界、工房风格悬浮提示和 OP 测试背包。设施摆放／收纳／破坏公共化及混合兽等生物行为整理继续后置。
- 丹方和炉记分别使用 `book`、`journal` 窗口 identity，独立关闭、拖动和聚焦；标题及工作区标签分别为“丹方”“炉记”。
- 部位信息使用旧纸小笺、墨色正文和朱色标题，靠近对应部位显示；快捷键仍集中在工位底部。
- 具体代码由主 agent 编写。仅截图识别和审美反馈允许外包，使用本地 Codex 配置的 `gpt-6-astra` / `xhigh`，不改用 Cliproxy。
- 原生验收保持服务端 ready → Windows Java 17 + Fabric jar 的顺序，不使用 HMCL 或 WSLg。静态检查、单测和夹具截图均不能替代正式入口与美术验收。

### 职责分类与窗口修复

`AlchemyWorkspaceLayout` 负责纸页与炉景摆位；`AlchemyFurnaceComponent` 负责模型、部位命中和悬浮；`AlchemyWorkbenchEffects` 负责火、烟、香与权威事件触发的动画；`AlchemyNotesContent` 负责阅读和选方；`AlchemyPaperSurface` 用九片绘制纸材，保持纸边比例。

此前纸张模式直接拆除了 `window-header`，使背景从整个窗口顶部开始绘制。最新 `OwoXmlWindowContentAdapter.paperFrame()` 保留墨色标题与“合卷”入口，内容区高度统一扣除标题栏；背景只绘制在内容区。纸页不再重复绘制正文区标题。窄屏初始阅读位置让出工位顶部，并缩小正文滚动区；允许用户后续主动拖动叠放。

预览检查同步核对纸张顶部与标题拖动区的分界、纸张底部与窗口底部对齐、正文内边距，以及最小尺寸的滚动、拖动、合卷和重开。检查坐标和绘制共用组件布局，不能用背景图片尺寸代替真实窗口边界。

### OP 背包接口

`server/src/inventory/operator.rs` 复用 `body_pocket`，依据 `DevCommandPermissions::is_operator` 把 OP 容器校准为 12×12、名称“OP 背包”，普通账号为 2×3。新建、恢复库存及在线权限变更均走校准路径。撤权时先安置越界物品，无法装回的物品转持久化掉落；`save_player_inventory_with_drops` 在同一事务保存库存与掉落，不覆盖制作会话。客户端容器尺寸根据真实行列和视口确定，保留滚动。

### 预览修复与验证记录

- 原生预览曾完成 11 张夹具截图，随后在 `alchemy-production-open` 空引用失败。原因是截图状态机关闭前一场景后直接配置下一张视口，没有调用下一场景的 `clientReady()`。现在每张截图重新经过准备阶段；正式场景等待 `/scene` 后的新权威库存，再寻找真实丹炉。
- 仓库 `alchemy-window-ui-preview.json` 包含 13 张纸页、工位、投料、错误和部位悬浮截图；输出目录改为 `alchemy-paper-boundary-preview`，避免把旧截图误当作最新结果。`alchemy-runtime-ui-preview.json` 包含正式 G 入口及真实 OP 背包两张截图。
- 最新七个相关 Java 类使用 Java 17、已有开发 jar 和本地命名空间依赖进行独立增量编译，通过；`UiPreviewSessionTest` 与 `UiWindowManagerTest` 共 12 项通过，0 失败。临时编译未覆盖 Gradle 产物或 Windows jar。预览配置 JSON 和 `git diff --check` 通过。
- 完整 `gradle test build --offline --no-daemon` 被当前沙箱阻止：改用工作区缓存后，Gradle daemon 创建 socket 报 `java.net.SocketException: Operation not permitted`。调整 JVM 参数仍未解除环境限制，未修改工程依赖或全局配置。
- 最新标题栏修复没有完成 Windows Native 重拍，也没有完整 Gradle／GameTest 结果。此前版本的完整测试及原生截图只作为历史证据，不代表本次修复已通过 GPU 裁剪或美术验收。
- 先前“普通香图标缺失”的记录已过时：源码已有 `textures/gui/items/incense_plain.png`；最终实际显示仍随下一次原生验收检查。

后续恢复验收时，先构建新 jar，核对 Windows 实例与构建产物哈希，再确认服务端监听并启动原生端。须重新检查宽／窄窗口下的标题、纸边、正文滚动、合卷入口、火门／香座小笺，以及真实 OP 背包。原生通过前不宣布美术完成；未经用户要求不提交、推送或创建 PR。

权限更新后的重试：Java 17、工作区内 `GRADLE_USER_HOME=client/.gradle`，经构建令牌执行完整 `gradle test build --offline --no-daemon --console=plain`，仍在 daemon 创建本地 socket 时被拒绝（`java.net.SocketException: Operation not permitted`），尚未进入编译或测试任务。日志为 `local_images/r7-alchemy/paper-tab-permission-retry.log`。本次未替换 jar、未启动原生端，也没有新增截图；已有 12 项局部测试记录不能代替完整构建结果。

## 13. 原生复验与 100×100 面板适配（2026-09-22）

### 已完成的原生复验

解除沙箱后，Java 17 完整 `gradle test build --offline --no-daemon --console=plain` 通过：5,054 项 JUnit、3 项 GameTest。日志 `local_images/r7-alchemy/paper-tab-unrestricted-build.log`。服务端 `cargo build --offline` 通过，并使用 `local_images/r7-alchemy/native-acceptance.vhPvhn/server/` 独立存档启动；ready 标记及 25565 监听确认后再启动 Windows Java 17 + Fabric jar。

该版本 jar 与 Windows 实例 SHA-256 一致，为 `1748369dcdafba202749b6e93a084e73b4bf6971150c6c201ec5feae05e2eec9`。纸页、工位与部位悬浮预览 13/13 通过，正式 G 入口和权威 12×12 OP 背包预览 2/2 通过。结果、原始截图及客户端日志已保存在 `local_images/r7-alchemy/native-acceptance.vhPvhn/{paper,runtime}/` 及同级日志，拼图为 `paper-contact.jpg`。这些结果早于下面的小窗口修改，不能作为 100×100 的通过证据；审美仍由用户验收。

### 用户新增要求与布局职责

用户要求低分辨率可用，例如 100×100，允许背景图缩放。工位、丹方与炉记的最小面板尺寸改为 100×100：背景、炉体、特效及部位命中继续共用等比画布；小窗快捷键缩成两行，纸页边缘和留白随尺寸收缩，正文与部位小笺保持文字字号并滚动阅读。窗口标题、管理按钮与尺寸输入框分别压缩，标题拖动使用实际按钮边界，避免命中错位。

`AlchemyWorkspaceLayout` 管理可用面积与视口边距；`AlchemyFurnaceComponent` 管理炉景、快捷键与悬浮区域；`AlchemyNotesContent` 管理纸页正文及翻页导航。没有为缩放另建一份界面或复制请求链路。

### 小窗口验证状态

- 首轮缩放代码完整 Gradle 构建通过，日志 `local_images/r7-alchemy/compact-build.log`；同步到 Windows 的 jar 哈希为 `ecb0eb11b93a448efa8d630186deea529116a59678c6e8104b31043083eae40e`。
- 首轮原生缩放预览尚未进入 UI 检查：请求整个游戏 framebuffer 为 100×100 时，Windows 实际给出 120×100，状态机在 `WAIT_VIEWPORT` 诚实报错，`completed=0`。
- 因此将业务面板与原生窗框尺寸分开：`alchemy-compact-ui-preview.json` 现在用正常原生窗口展示并精确断言实际 100×100 面板，另测 120×100 原生视口、横向矮窗、纵向窄窗和正式入口的小面板鼠标捕获。共 12 个场景，仍需运行。
- 后续压缩纸页正文间距、调整预览后的 9 个相关 Java 类已在独立临时目录增量编译通过；现有窗口及预览生命周期测试 12/12 通过。日志 `local_images/r7-alchemy/compact-local-check.log`。JSON 解析与 `git diff --check` 通过。
- 续跑后环境恢复为受限沙箱，完整构建再次在 daemon socket 被拒绝，日志 `local_images/r7-alchemy/compact-resume-build.log`。最新源代码尚未重建或更新 Windows jar，没有小窗口原生通过截图。恢复权限后应先完整构建，再重新执行 compact、paper、runtime 三套预览。
