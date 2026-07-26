# BugHunt: 天道 Arbiter 中文玩家名脱敏在紧邻中文正文时静默失效，泄露玩家身份

## Bug 摘要

**严重度：high（skeptic 由 medium 调整为 high）**

`agent/packages/tiandao/src/arbiter.ts` 里的 `redactChinesePlayerNameToken`（L631-656）只有在 `hasNameBoundary`（L658-660）判定"匹配到的玩家名前后都不是 word-like 字符"时才会把该名字替换成"某修士"；否则原样保留匹配到的名字，脱敏静默跳过。`isWordLikeCharacter`（L662-664）用的正则 `/[\p{L}\p{N}_:-]/u` 里 `\p{L}` 这个 Unicode 字母大类**包含汉字**（Unicode 类目 Lo）。而中文书写没有词间空格，天道 narration 里玩家名紧挨着叙事正文（"张三向着血谷疾驰而去"）是默认写法而非例外——这正是最常见的写法，导致该 name-redaction 分支在绝大多数真实 narration 里都不生效，玩家真名原样广播出去。

这直接违反 `docs/worldview.md §十一 匿名系统 L930-943`（"修士之间默认不显示名字"）与该模块自身在 `agent/packages/tiandao/src/context.ts:187` 写明的约束（"匿名：正文不要主动写玩家名，除非渡虚劫点名或死亡遗念"）——脱敏这道安全网本身在最常见输入下形同虚设。

## 实际游玩体验影响

天道三 Agent（灾劫/变化/演绎）每 tick 都可能产出提到玩家名的中文 narration，且这类文本天然紧贴叙事正文（无空格分词）。只要玩家名后面（或前面）紧跟一个普通汉字，`redactPlayerNames`（arbiter.ts:239-254）在 `applyNarrationScopeRules`（arbiter.ts:187）里对**所有 scope（包括 `broadcast`，即全服可见）**统一调用的脱敏就会跳过该次出现，玩家真实姓名原样出现在全服聊天栏。

这直接击穿 worldview §十一 描述的"末法残土 PVP 信息差极重"（worldview.md:510）设计支柱——匿名本是让"藏后招"成为可能的前提；一旦天道 narration 频繁把具名信息泄露出去，PVP 信息不对称机制形同虚设，且玩家无法感知/无法防御（这是 server↔agent 单向广播，客户端没有二次过滤手段）。玩家会在毫无预警的情况下发现自己在坍缩渊/野外的具体位置、行为被全服看到具名描述，即使自己从未主动聊天、交易或暴露身份。

## 证据定位

- `agent/packages/tiandao/src/arbiter.ts:187`：`applyNarrationScopeRules` 对每条 narration（含 `scope: "broadcast"`）无条件调用 `this.redactPlayerNames(narration)`，脱敏发生在 broadcast/zone/player 分支判断之前。
- `agent/packages/tiandao/src/arbiter.ts:239-254`（`redactPlayerNames`）：对 `this.state.players` 逐个调用 `redactIdentifierToken(text, player.uuid)`（无条件全词替换）与 `redactChinesePlayerNameToken(text, player.name)`（有边界判定，见下）。
- `agent/packages/tiandao/src/arbiter.ts:622-629`（`redactIdentifierToken`）：对比组——uuid 走 `replaceAllLiteral`，无边界判定，全部命中都替换。
- `agent/packages/tiandao/src/arbiter.ts:631-656`（`redactChinesePlayerNameToken`）：核心缺陷函数。`trimmed.length < 2` 直接放行不脱敏（L633-635）；否则逐次 `indexOf` 找到匹配区间 `[index, end)`，只有 `hasNameBoundary(text, index, end)` 为真才替换成"某修士"（L647-651），否则 `output += text.slice(cursor, end)` 原样保留匹配到的名字明文。
- `agent/packages/tiandao/src/arbiter.ts:658-660`（`hasNameBoundary`）：`!isWordLikeCharacter(text[start - 1]) && !isWordLikeCharacter(text[end])`——要求名字前后字符都不是 word-like 才算"边界"，否则不脱敏。
- `agent/packages/tiandao/src/arbiter.ts:662-664`（`isWordLikeCharacter`）：正则 `/[\p{L}\p{N}_:-]/u`；`\p{L}` 覆盖 Unicode 类目 Lo（含全部常用汉字），因此紧邻的普通汉字会被判定为"word-like"，从而使 `hasNameBoundary` 返回 false，脱敏被跳过。
- 复现（本轮复核直接跑通该函数逻辑，非 JSON 转述）：
  - `redactChinesePlayerNameToken("张三向着血谷疾驰而去", "张三")` → 返回原文不变，"张三"明文泄露。
  - `redactChinesePlayerNameToken("此地有张三坐镇", "张三")` → 返回原文不变，"张三"明文泄露（前后均为汉字）。
  - `redactChinesePlayerNameToken("张三 在谷口", "张三")` → 返回 `"某修士 在谷口"`，只有名字后面紧跟空格这种非典型中文写法才会命中脱敏。
- `agent/packages/tiandao/src/context.ts:187`：`"匿名：正文不要主动写玩家名，除非渡虚劫点名或死亡遗念。"`——确认这是该模块自陈的既有安全约束，脱敏 pass 正是为兜底这条约束而存在。
- `docs/worldview.md §十一 匿名系统 L930-943`："修士之间默认不显示名字"、"暴露名字的方式"仅限主动交流/交易/被天道点名/死亡——不包括"天道 narration 随手带出"。`worldview.md:510` 明确匿名是 PVP 信息差设计的物理依据。
- 既有测试覆盖缺口（掩盖本 bug 的测试）：
  - `agent/packages/tiandao/tests/arbiter.test.ts:513-539`：唯一覆盖 `redactChinesePlayerNameToken` 的用例，名字是 ASCII `"TestPlayer"` 且紧跟一个字面空格（`"TestPlayer 在谷口招来兽鸣..."`），空格触发了（碰巧成立的）边界判定，测试通过掩盖了"名字紧贴汉字"这一真实高频场景完全未被覆盖的事实。
  - `agent/packages/tiandao/tests/arbiter.test.ts:616-646`：覆盖的是 `redactIdentifierToken`（uuid `"offline:a"`，标点边界）以及 `redactChinesePlayerNameToken` 的 `trimmed.length < 2` 早退分支（`name: "a"`），同样没有触发"合法长度中文名紧贴中文正文"路径。

## 触发路径

1. 天道任一 Agent（灾劫/变化/演绎）产出一条提到某玩家中文名的 narration，写法为默认中文行文习惯——名字前后紧跟汉字、无空格（例如"张三向着血谷疾驰而去"）。
2. Arbiter 合并阶段 `applyNarrationScopeRules`（arbiter.ts:187）无条件调用 `redactPlayerNames`（arbiter.ts:239-254），对该 narration 走脱敏。
3. `redactChinesePlayerNameToken` 匹配到玩家名区间后调用 `hasNameBoundary`；由于前后字符是普通汉字，`isWordLikeCharacter` 判定它们为 word-like，`hasNameBoundary` 返回 false。
4. 脱敏分支被跳过，原始明文名字被保留在输出文本里（`output += text.slice(cursor, end)`）。
5. 该 narration 若 `scope: "broadcast"` 且通过 `isBroadcastAllowed`（era_decree / death_insight / du-xu 信号任一命中），未经收窄直接全服广播；若被收窄为 `zone`，也仍然带着玩家明文名广播给同 zone 所有玩家。
6. 玩家在未主动聊天/交易/渡劫/死亡的情况下，姓名及其在 narration 中描述的行为/位置被暴露给其他在场或全服玩家。

## 反方审查记录

- 第一轮质疑：
  - 质疑"这是不是只在人为构造的边界字符串下才触发，真实 LLM 输出会不会天然避开"——被驳回：中文书写惯例本身就没有词间空格，天道 narration 的 style 全部是中文叙事文本，LLM 不太可能可靠地在每次提及玩家名前后都插入标点/空格；skeptic 直接跑函数复现，`"张三向着血谷疾驰而去"` 与 `"此地有张三坐镇"` 均确认原样泄露。
  - 质疑"是不是有其他二次过滤兜底"——检查 `applyNarrationScopeRules`（L187）后续分支（broadcast/zone/player 判定），确认脱敏是发生在这些分支**之前**的唯一一道处理，之后没有第二道过滤。
  - 质疑"是不是与已知 plan `docs/plan-bughunt-world-social-anonymity-live-sync-v1.md` 重复"——核对后确认那份 plan 处理的是 `server/src/social/mod.rs` 侧 `SocialAnonymity` payload 在暴露事件后向客户端的实时刷新，与本 bug（天道 narration 文本本身的脱敏正则）是完全不同模块、不同失效模式，不构成重复。
  - 初裁：倾向通过，但严重度先给 medium（当时判断"只是文本脱敏正则的 bug"）。
- 第二轮补证：
  - 补充确认失效面不限于中文名——ASCII 名字紧贴中文正文同样泄露（复现 `"Kiz向着血谷疾驰而去"`、`"玩家Kiz向着血谷疾驰而去"` 均原样保留 "Kiz"），说明这不是"中文名专属"的边角情况，而是"任何名字只要紧邻汉字就失效"的普遍模式。
  - 补充确认触发面覆盖 `scope: "broadcast"`——不是仅限私聊/zone 内小范围泄露，era_decree / death_insight / du-xu 信号命中的 broadcast narration 会不经收窄直接全服可见。
  - 补充确认现有两条测试（`arbiter.test.ts:513-539`、`616-646`）都恰好绕开了"名字紧贴汉字"路径，是真实的覆盖盲区而非"已知且接受的行为"。
  - 严重度上调理由：这不是罕见 corner case，而是"中文 narration 提及玩家名"的**默认/近乎全部**情况，静默击穿了 worldview §十一 明确文档化、且是 PVP 信息差设计支柱的匿名机制，属于核心设计被架空而非个别边角失误——上调为 high。
  - 终裁：通过。反方认为这是脱敏正则的判定逻辑错误（"word-like" 概念不该覆盖 CJK 文字用于边界判定），修复范围限定在 `isWordLikeCharacter` / `hasNameBoundary` 的判定策略，不应扩展为重写整个脱敏架构。

主循环复核：已亲读关键行确认。

## Skeleton Fix Plan

- [ ] 修正 `isWordLikeCharacter`（`agent/packages/tiandao/src/arbiter.ts:662-664`）的判定策略：当前 `\p{L}` 覆盖了汉字（Unicode 类目 Lo），但中文书写没有词间空格的书写惯例，"紧邻汉字"不该被当作"这是同一个不可拆分 token 的延续"。改为把 Han 文字从"word-like（会阻断脱敏边界）"判定里剔除，例如：
  ```ts
  function isWordLikeCharacter(char: string | undefined): boolean {
    if (char === undefined) return false;
    if (/\p{Script=Han}/u.test(char)) return false; // 汉字不提供词边界信息，紧邻汉字应视为可脱敏边界
    return /[\p{L}\p{N}_:-]/u.test(char);
  }
  ```
  效果：名字前/后紧邻普通汉字时，`hasNameBoundary` 不再因为"汉字是 word-like"而返回 false，脱敏分支正常触发；同时保留对 ASCII 字母/数字/下划线/冒号/连字符连续拼接的边界保护（防止 `TestPlayer` 被 `TestPlayerX` 之类更长 token 误伤截断）。
- [ ] 同步更新 `hasNameBoundary`（L658-660）上方注释，明确其语义变化为"前后不是同一（非 Han）书写系统下的延续字符"，避免后续维护者误以为它仍是纯粹的"是否为字母数字"判定。
- [ ] 复核 `redactIdentifierToken`（L622-629，用于 uuid，无边界判定、全词替换）与修复后的 `redactChinesePlayerNameToken`（有边界判定）两条路径的语义差异是否仍然合理：uuid 误撞概率低所以走无条件替换，玩家名需要防止长单词内部子串误伤所以走边界判定——若认可这个既有设计区分，在函数上方各补一行注释说明"为什么这两条路径处理方式不同"，不合并成一套逻辑。
- [ ] 评估 `trimmed.length < 2` 早退分支（L633-635）：当前任何 1 字符玩家名完全不脱敏。修复本 bug 时**不擅自放开**这条阈值（1 字符中文名在正文中撞车概率极高，属于独立的设计取舍），但在 plan 文档里记录这是已知的、范围外的行为，留给后续 plan 决策是否需要更保守的策略（如禁止注册 1 字符显示名）。
- [ ] 不改动 `redactPlayerNames`（L239-254）/`applyNarrationScopeRules`（L187）的调用时机与顺序——本 bug 的修复面严格限定在"边界判定用什么字符集"这一层，不重新设计脱敏调用链。

## 验收测试计划

全部新增/回归用例落在 `agent/packages/tiandao/tests/arbiter.test.ts`，通过 `cd agent/packages/tiandao && npm test` 跑（vitest）。

- **happy path（本 bug 的核心复现场景）**：
  - 名字紧贴在正文开头且直接后跟汉字，无任何分隔符：`narration.text = "张三向着血谷疾驰而去"`，玩家 `name: "张三"`。断言修复后输出为 `"某修士向着血谷疾驰而去"`（修复前会原样保留 "张三"，此断言在修复前必须失败，锁住回归）。
  - 名字前后均紧贴汉字（双侧无分隔符）：`"此地有张三坐镇"` → 断言 `"此地有某修士坐镇"`。
- **边界（boundary）**：
  - 名字出现在字符串起始位置（`text[start-1]` 越界为 `undefined`）且后面紧跟汉字：`"张三向着血谷疾驰而去"`（同上，起始位置边界与紧邻汉字边界叠加）——已在 happy path 覆盖，此处显式断言不因起始越界而抛异常或误判。
  - 名字出现在字符串末尾（`text[end]` 越界为 `undefined`）且前面紧跟汉字：`"血谷疾驰而去者乃张三"` → 断言替换为 `"...乃某修士"`。
  - 混合书写体系：ASCII 名字单侧/双侧紧贴中文正文（沿用 skeptic 复现用例）：`"Kiz向着血谷疾驰而去"` 与 `"玩家Kiz向着血谷疾驰而去"` → 均断言 "Kiz" 被替换为 "某修士"，验证修复不局限于纯 CJK 名字。
  - 同一 narration 文本中同一名字出现多次，且各次的前后邻接字符不同（有的紧贴汉字、有的紧贴空格）：断言全部出现都被统一替换，不出现"部分脱敏部分泄露"的半吊子结果。
- **错误分支 / 既有行为不回归**：
  - ASCII 名字紧邻长单词的场景仍要保护，不能因为放宽了汉字边界而连带放宽了字母数字边界：构造 `"TestPlayerX 才是真正的对手"` 与玩家名 `"TestPlayer"`，断言**不**被误伤替换（`TestPlayerX` 不应被拆成 `某修士X`）——锁住"只放开 Han，不放开 ASCII 连续拼接"这条修复边界。
  - 既有测试 `arbiter.test.ts:513-539`（"TestPlayer 在谷口..." + 空格边界）必须继续通过，作为无回归护栏。
  - 既有测试 `arbiter.test.ts:616-646`（uuid `offline:a` 标点边界 + 单字符名 `"a"` 早退）必须继续通过。
  - 1 字符玩家名早退行为按 plan 决议保持现状：构造 1 字符名 narration，断言当前"不脱敏"行为被**显式**测试锁定为已知行为（而非被后续修复无意间改变却无测试发现）。
- **状态转换（scope 分支）**：
  - `scope: "broadcast"` 且命中 `isBroadcastAllowed`（如 `style: "era_decree"`）：narration 文本内嵌紧贴汉字的玩家名 → 端到端跑 `applyNarrationScopeRules`，断言最终返回的 broadcast narration 文本里**不包含**玩家真名子串。
  - `scope: "broadcast"` 但未命中 `isBroadcastAllowed`，被 `narrowBroadcastNarration` 收窄为 `scope: "zone"`：断言收窄后的文本同样已脱敏（脱敏在收窄判断之前发生，不应因为 scope 改变而回退）。
  - `scope: "zone"` / `scope: "player"` 直发路径：各构造一条紧贴汉字玩家名的 narration，断言脱敏对三种 scope 一致生效（对齐"脱敏在 scope 分支之前统一执行"的架构不变式）。

## 风险

- **潜在过度脱敏**：把 Han 从 `isWordLikeCharacter` 剔除后，如果某玩家显示名恰好是一段常见中文短语的前缀/子串（例如两字名恰好是热门叙事用词的前两个字），可能出现无关文字被误判为玩家名而被替换。这是"匿名优先于偶发误伤"的既有设计取向（worldview §十一 本就要求最大限度匿名），本次修复不额外引入新的子串匹配保护机制，若后续发现真实误伤案例应交给独立 plan 评估（例如引入更精细的分词或名字唯一性校验），不在本 bug 范围内展开。
- **`redactIdentifierToken`（uuid 路径）未同步修复**：该函数无边界判定、全词替换，理论上比玩家名路径更容易误伤（uuid 恰好是另一段文本子串的概率极低但非零）。本次修复范围严格限定在 `redactChinesePlayerNameToken`/`hasNameBoundary`/`isWordLikeCharacter` 三者，不顺手改 `redactIdentifierToken`，避免修复面扩散；该函数的设计取舍需要在 plan 文档里存档说明，供后续追溯。
- **修复面仅限 agent 侧**：本 bug 与 server/client 无关（纯 TypeScript 文本处理，运行于 tiandao Agent 合并阶段），修复不触达 `server/src/social/mod.rs` 的 `SocialAnonymity` 广播逻辑。若日后 server 侧对同一段 narration 文本做二次转发/持久化，需要单独确认没有绕开本次修复重新引入明文玩家名的分支。
- **1 字符玩家名早退分支维持现状**：修复不改变 `trimmed.length < 2` 的既有放行行为，1 字符名玩家的脱敏缺口仍然存在且已知，留待独立决策（如是否应禁止注册单字显示名）。
