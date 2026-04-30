# Bong · plan-narrative-v1 · 模板

**天道叙事内容侧**。Agent-v2 只讲骨架，本 plan 定义：何时 narrate、节奏、视角剪裁、重复抑制、风格/语气。

**世界观锚点**：`worldview.md §八 天道行为准则 / 天道叙事的语调`（line ~620-638）天道语调"**冷漠的、有古意的、偶尔带嘲讽的**"。明示反例："恭喜！你发现了一个灵眼！"、"注意！血谷灵气下降了！"、"小心，前方有危险的怪物！" 都是**坏**叙事。本 plan 风格指南以 worldview §八 line ~620-638 的好/坏对照为基准。

**交叉引用**：`plan-agent-v2.md` · `plan-HUD-v1.md §9` 双通道路由 · `worldview.md §七/§八/§十二`（含遗念）。

---

## §0 设计轴心

- [x] 克制：天道惜字如金，不是每个事件都要 narration ✅（三 skill prompts 均要求"宁可不降劫/沉默观察"，dedupe 兜底）
- [ ] 玩家视角：只叙述玩家能感知的（含神识感知范围）
- [x] 风格：冷漠 + 古意 + 嘲讽，**禁现代腔/游戏化提示腔** ✅（`narration-eval.ts` MODERN_SLANG_RE + STYLE_KEYWORDS + 长度 100-200，prompts 强制半文言半白话）
- [ ] 节奏：与玩法节奏合拍，不打断战斗

## §1 Narration 触发表

| 事件类型 | 触发频率 | 通道 | 字数 |
|---|---|---|---|
| 渡虚劫全服广播 | 极低 | ChatHud | 中 |
| 域崩 / 区域灵气剧变 | 低 | ChatHud | 中 |
| 玩家境界突破 | 中 | ChatHud | 短 |
| 环境氛围（区域进入）| 中 | ChatHud | 短 |
| 死亡遗念（Death Insight）| 每次死必发 | ChatHud | 中长 |
| 终焉之言（角色终结）| 极低 | ChatHud + 生平卷末页 | 中 |
| 普通战斗 tick | — | EventStore | — |

## §2 视角剪裁规则

- [ ] 玩家不在场的事件 → 走传闻/NPC 口述/远方异象
- [ ] 神识可感知范围内（按境界）→ 可直接 narrate
- [ ] 跨地域同步：仅"渡虚劫"级事件做全服广播（schema 有 `scope: broadcast|zone|player`，但"仅渡虚劫级"为人为约束、暂未代码强制）
- [ ] 匿名约束：不主动暴露玩家名字（除非已被天道点名/已死）

## §3 风格指南

- [ ] **好**："血谷灵脉又枯了三分。仍有蠢人在那里打坐。"
- [ ] **好**："此间有修士渡劫。天地为之色变。旁观者……自求多福。"
- [ ] **坏**："恭喜！你发现了一个灵眼！" / "注意！xx 下降了！" / "小心，前方有危险的怪物！"
- [ ] 词汇黑名单：恭喜 / 注意 / 警告 / 小心 / xp / 等级提升（当前 `MODERN_SLANG_RE` 黑名单含 ok/lol/bro/buff/nerf/gg/wtf/哈哈/666/牛/服了/离谱/刷怪/yyds/233，未覆盖恭喜/注意/警告/小心/xp/等级提升）
- [x] 句式偏好：短句 + 古词 + 留白 ✅（`scoreNarration` 长度 100-200 + `OMEN_RE`（预兆/暗示/伏笔/将/欲/渐...）+ STYLE_KEYWORDS 评分）

## §4 重复抑制

- [x] 相同事件再触发的冷却 ✅（server `NarrationDedupeResource` 按 scope|target|style|text 拼 key，`NARRATION_DEDUPE_WINDOW_SECS` 时间窗 + `NARRATION_DEDUPE_CAPACITY` 容量丢重；`process_agent_narrations_with_dedupe` 已接入 main loop）
- [ ] 模板轮换（同义古风变体）
- [ ] LLM 去重 prompt（参考最近 N 条避免雷同）

## §5 三 Agent 职责分配（narrate 维度）

- [x] 灾劫 Agent：天劫 / 域崩 / 终焉之言
- [x] 变化 Agent：区域灵气变化 / 异象 / 异变兽刷新
- [x] 演绎时代 Agent：长线叙事（时代背景、亡者博物馆引用）

## §6 实施节点

## §7 开放问题

---

## 进度日志

- 2026-04-25：核对实装 —— schema (`Narration`/`NarrationV1` 含 scope+style)、agent 三 skill prompts (calamity/mutation/era 半文言半白话+预兆要求)、`narration-eval.ts` 风格评分（长度 100-200 + omen + 现代腔黑名单 + 风格关键词）、server `NarrationDedupeResource`（按 scope|target|style|text dedupe）、client `NarrationState` 按 style 分流 ChatHud + Toast 均已上线；§0/§3/§4 部分项勾选；§2 视角剪裁、§4 模板轮换、§4 LLM 去重 prompt 仍为待办；黑名单需补"恭喜/注意/警告/小心/xp/等级提升"中文条目。
