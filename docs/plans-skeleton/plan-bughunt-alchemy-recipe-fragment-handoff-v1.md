# BugHunt: 丹方残卷学习链路断裂

状态：Skeleton Plan，仅记录 BugHunt 发现。本 PR 不修改实际代码、配置、依赖或资源。

## Bug 摘要

真实丹方残卷物品已经在 server 侧作为 `recipe_fragment` 模板存在，server 也有权威的 `AlchemyLearnRecipeFragment { item_instance_id }` intent；但当前 client 炼丹炉 UI 仍只识别旧 mock 前缀 `recipe_scroll_`，并发送无物品门禁的 `alchemy_learn_recipe` 直接学习请求。agent schema/generated 也没有 fragment 请求类型。

结果是：玩家拿到真实 `fragment_alchemy_hui_yuan_pill` 后，无法通过当前真实 UI 走到 server 权威的“按背包实例学习并消耗残卷”链路；同时旧 direct learn 路径仍可绕过物品实例校验。

## 实际游玩体验影响

首个回元丹方的发现路径会在真实玩家主线中断裂。玩家从教程/散修遗缴/掉落获得《回元丹方·残》后，把残卷拖到炼丹炉 UI 的残卷区域不会被当前 UI 识别为可学习丹方，因为物品 ID 是 `fragment_alchemy_hui_yuan_pill`，不是 `recipe_scroll_*`。

对玩家表现为：明明获得了丹方残卷，却无法自然解锁回元丹配方，进而无法进入“采材料 → 学丹方 → 开炉炼回元丹”的 onboarding 炼丹主路径。另一方面，旧 `alchemy_learn_recipe` direct request 没有绑定背包实例，破坏“知识必须来自实际物品”的门禁和消耗语义。

## 证据定位

1. client 炼丹炉 UI 仍绑定旧 mock 前缀：
   - `client/src/main/java/com/bong/client/alchemy/AlchemyScreen.java:39`
   - `RECIPE_SCROLL_PREFIX = "recipe_scroll_"`

2. client drop 行为只接受旧前缀，并本地先 learn，再发 direct learn：
   - `client/src/main/java/com/bong/client/alchemy/AlchemyScreen.java:693`
   - `attemptDrop(...)` 中只有 `dragged.itemId().startsWith(RECIPE_SCROLL_PREFIX)` 才进入学习分支。
   - `client/src/main/java/com/bong/client/alchemy/AlchemyScreen.java:699`
   - `RecipeScrollStore.learn(...)` 先在本地写入。
   - `client/src/main/java/com/bong/client/alchemy/AlchemyScreen.java:704`
   - 随后调用 `ClientRequestSender.sendAlchemyLearnRecipe(id)`。

3. client request sender/protocol 只有旧 direct learn：
   - `client/src/main/java/com/bong/client/network/ClientRequestSender.java:373`
   - `sendAlchemyLearnRecipe(String recipeId)`
   - `client/src/main/java/com/bong/client/network/ClientRequestProtocol.java:368`
   - `encodeAlchemyLearnRecipe(...)` 编码 `type = "alchemy_learn_recipe"`。

4. server 已有权威 fragment 请求和 handler：
   - `server/src/schema/client_request.rs:118`
   - `AlchemyLearnRecipeFragment { v, item_instance_id }`
   - `server/src/network/client_request_handler.rs:813`
   - handler 收到后发送 `LearnRecipeFragmentIntent { player, item_instance_id }`。

5. server fragment 学习依赖背包实例上的 alchemy 数据：
   - `server/src/alchemy/mod.rs:168`
   - `handle_recipe_fragment_learning(...)`
   - `server/src/alchemy/mod.rs:175`
   - 通过 `inventory_item_by_instance_borrow(...)` 查玩家背包实例。
   - `server/src/alchemy/mod.rs:177`
   - 只接受 `item.alchemy == Some(AlchemyItemData::RecipeFragment { ... })`。
   - `server/src/alchemy/mod.rs:212`
   - 学习成功后调用 `consume_item_instance_once(...)` 消耗该实例。

6. 真实残卷模板不是旧前缀 ID：
   - `server/assets/items/onboarding_scrolls.toml:100`
   - `id = "fragment_alchemy_hui_yuan_pill"`
   - `server/assets/items/onboarding_scrolls.toml:103`
   - `category = "recipe_fragment"`
   - `server/assets/items/onboarding_scrolls.toml:110`
   - `[item.recipe_fragment] recipe_id = "hui_yuan_pill_v0"`

7. 普通模板实例化疑似不会把 recipe fragment spec 带到运行时实例：
   - `server/src/inventory/mod.rs:1891`
   - `runtime_instance_from_template(...)`
   - `server/src/inventory/mod.rs:1931`
   - 固定 `alchemy: None`。
   - `server/src/inventory/mod.rs:15517`
   - 现有测试只覆盖 `fragment_alchemy_hui_yuan_pill` 模板解析出 `recipe_fragment_spec`，未覆盖运行时实例携带 `AlchemyItemData::RecipeFragment`。

8. agent schema/generated 缺 fragment request：
   - `agent/packages/schema/src/client-request.ts:449`
   - 炼丹请求段有 open/feed/take/ignite/intervention/turn/learn 等类型。
   - `agent/packages/schema/src/client-request.ts:1156`
   - union 只包含 `AlchemyLearnRecipeRequestV1`，没有 `AlchemyLearnRecipeFragmentRequestV1`。
   - `agent/packages/schema/generated/`
   - 仅有 `client-request-alchemy-learn-recipe-v1.json`，无 fragment generated schema。

9. 已归档设计明确不应使用旧 direct learn 作为首方发现路径：
   - `docs/finished_plans/plan-onboarding-loop-v1.md:464`
   - P2.2 规定 `fragment_alchemy_hui_yuan_pill` use → `RecipeFragment` → `LearnRecipeFragmentIntent`。
   - `docs/finished_plans/plan-onboarding-loop-v1.md:608`
   - 明确“不用 `AlchemyLearnRecipe` client request”，因为它无 item-gating。

## 触发路径

1. 玩家通过 onboarding 或掉落获得 `fragment_alchemy_hui_yuan_pill`。
2. 玩家打开炼丹炉 UI，尝试把《回元丹方·残》拖入残卷/丹方学习区域。
3. `AlchemyScreen.attemptDrop(...)` 检查 `dragged.itemId().startsWith("recipe_scroll_")`。
4. 真实 item id 为 `fragment_alchemy_hui_yuan_pill`，检查失败，UI 不会发送 fragment 学习请求。
5. 即使玩家能通过旧 mock `recipe_scroll_*` 路径触发学习，client 发送的是 `alchemy_learn_recipe`，server 无法校验/消耗背包里的真实残卷实例。
6. server 侧权威 `AlchemyLearnRecipeFragment` intent 因 client/schema handoff 缺失无法从真实 UI 主路径触达。

## 反方审查记录

### 第一轮：反方质疑，结论 LIKELY

反方质疑：
- server 已经有 `AlchemyLearnRecipeFragment` handler，不能仅凭 client 缺 sender 就断言学习链完全不可用。
- 可能存在“物品 use”或其他非炼丹炉 UI 路径直接 emit `LearnRecipeFragmentIntent`，绕开 client request。
- 若测试或特殊掉落手工构造 `ItemInstance.alchemy = RecipeFragment`，server fragment learning 本身可能是可用的。

补证/让步：
- 本 bug 不主张 server fragment intent 完全不可用；边界限定为“真实普通物品模板/掉落/发物 + 当前 client UI/agent schema”的正式玩家路径断链。
- client 当前真实 UI 只走 `recipe_scroll_` + `alchemy_learn_recipe`，没有 `item_instance_id` 请求，无法触达 server fragment handler。
- `runtime_instance_from_template(...)` 固定 `alchemy: None`，而 server learning 只看实例上的 `AlchemyItemData::RecipeFragment`；普通模板解析测试不足以证明运行时实例可学。

### 第二轮：反方最终裁决，结论 CONFIRMED

反方最终未找到反例。最终边界如下：
- 若某个测试/调试路径手工构造带 `AlchemyItemData::RecipeFragment` 的 `ItemInstance`，并手工发送 server 已支持的 `AlchemyLearnRecipeFragment`，server 可消费它。
- 已确认的问题是正式玩家链路断在 handoff：真实残卷模板不是 `recipe_scroll_*`，client 没有 fragment request sender/protocol，agent schema/generated 没有 fragment 请求，普通实例化还疑似丢失 template spec。
- PR 去重未发现同题开放 PR；相邻 PR #886 是 alchemy HUD 目标值，#943 是战斗丹丹毒门禁，均不是丹方残卷 C2S/schema handoff。

## Skeleton Fix Plan

- [ ] TODO(server): 让普通 `recipe_fragment` 模板实例化时携带 `AlchemyItemData::RecipeFragment`，或在 `handle_recipe_fragment_learning(...)` 中从 `ItemTemplate.recipe_fragment_spec` 做权威兜底；必须保持“实例属于玩家、物品是残卷、recipe 存在、成功后消耗一次”的校验。
- [ ] TODO(server): 增加 fragment learning 单测：真实 `fragment_alchemy_hui_yuan_pill` 实例进入玩家背包，发送 `AlchemyLearnRecipeFragment { item_instance_id }` 后写入 `LearnedRecipes` 并消耗残卷。
- [ ] TODO(server): 增加负例：无该实例、非残卷实例、未知 recipe、重复学习时不误消耗。
- [ ] TODO(client): 新增 `ClientRequestProtocol.encodeAlchemyLearnRecipeFragment(long itemInstanceId)` 和 `ClientRequestSender.sendAlchemyLearnRecipeFragment(...)`，payload type 使用 server/schema 约定的 fragment 请求名。
- [ ] TODO(client): `AlchemyScreen` 拖入真实 `recipe_fragment` 物品时按 `instanceId` 发 fragment request，不再先本地 `RecipeScrollStore.learn(...)` 注入结果。
- [ ] TODO(client): UI 识别真实残卷应基于 item category/metadata 或明确 template id，不再依赖 `recipe_scroll_` mock 前缀。
- [ ] TODO(schema): 在 `agent/packages/schema/src/client-request.ts` 新增 `AlchemyLearnRecipeFragmentRequestV1`，加入 `ClientRequestV1` union，并重建 generated schema/dist。
- [ ] TODO(e2e): 新增真实玩家链路：残卷进入背包 → client 操作发送 fragment request → server 学习并消耗残卷 → `alchemy_recipe_book` 与 inventory snapshot 刷新。
- [ ] TODO(regression): 确认旧 `alchemy_learn_recipe` 不再作为玩家 UI 的残卷学习路径；若保留 direct learn，只能用于明确的测试/GM/调试入口并受门禁限制。

## 验收测试计划

- server：在 `server/` 跑 `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`，重点覆盖 `handle_recipe_fragment_learning(...)`、`runtime_instance_from_template(...)` 或其兜底实现。
- client：在 `client/` 跑 `./gradlew test build`，新增 protocol JSON pin 测试，确认 fragment request 编码包含 `item_instance_id` 而非 `recipe_id`。
- schema/agent：在 `agent/` 跑 `npm run build`；若改 `agent/packages/schema/src/*.ts`，必须额外跑 `npm run build -w @bong/schema`，确认 generated/dist 与 src 一致。
- e2e：在仓库根设置 `export BONG_SKIP_SKIN_PREFETCH=1` 后跑 `bash scripts/smoke-test-e2e.sh`，覆盖真实残卷学习链。
- 回归观察：玩家拿到 `fragment_alchemy_hui_yuan_pill` 后，炼丹 UI 中学习成功、残卷减少、回元丹方出现；重复拖同一已消耗实例应失败且不产生本地假学习。

## 风险

- 需要同时改 server、client、agent schema/generated，属于跨端协议修复；任一端漏改都会造成 request 解析失败或 UI 假成功。
- 如果现有 `alchemy_learn_recipe` 被测试或调试工具依赖，直接删除会扩大影响；更稳妥是从玩家 UI 主路径移除 direct learn，并在需要时保留受控入口。
- 普通实例化兜底若放在 `handle_recipe_fragment_learning(...)`，需要避免信任 client 传来的 template id；应始终从 server 背包实例和 server item registry 派生。
- 修复后需防止重复学习误消耗残卷：`learned.learn_fragment(...)` 返回已知/无变化时不应消费。
