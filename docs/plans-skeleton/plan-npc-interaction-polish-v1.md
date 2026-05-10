# Bong · plan-npc-interaction-polish-v1 · 骨架

NPC 交互打磨。当前 big-brain Utility AI NPC 行为丰富（拾荒散修 / 天道守墓人 / 炼丹疯子 / 游商傀儡等 8 种 archetype）但**交互 UI 为骨架状态**：对话是一行 text、交易是原始 item transfer、威胁评估无视觉反馈。本 plan 打磨 4 个交互触点：对话气泡 / 交易界面 / 威胁感知条 / NPC 情绪与记忆可视化。

**世界观锚点**：`worldview.md §七` 散修评估威胁度（"你气息绵长 → 恭敬交易 / 真元见底 → 拔刀爆装备"）视觉化 · `§九` 面对面以物易物 → 交易 UI 保持"不安全"的紧张感 · `§十一` NPC 信誉度反应分级（高=主动给情报 / 极低=通缉）→ 信誉条可视化 · `§十六` 坍缩渊内道伥/执念行为（假示好/伏击）→ 需要用 NPC 行为暗示而不是 UI 文字

**library 锚点**：`peoples-0007 散修百态`（拾荒/游荡/占山/假死四路 NPC 行为模式）

**前置依赖**：
- `plan-npc-ai-v1` ✅ → NPC 状态机 + big-brain AI
- `plan-social-v1` ✅ → NPC 信誉度 / 交易
- `plan-identity-v1` ⏳ active → NPC 对身份的差异化反应
- `plan-HUD-v1` ✅ → Toast / Zone HUD 层（对话气泡放 Toast 层）
- `plan-audio-implementation-v1` 🆕 skeleton → NPC 反应音效（惊慌/威胁/交易）

**反向被依赖**：
- `plan-narrative-political-v1` ✅ → NPC 传播的信息通过对话 UI 显示

---

## 接入面 Checklist

- **进料**：`npc::NpcState { archetype, mood, threat_assessment }` / `social::Renown` / `identity::IdentityProfile` / `inventory::TradeSession` / `npc::brain::NpcAction`
- **出料**：`NpcDialogueBubbleRenderer`（对话气泡 + archetype 专属样式）+ `NpcTradeScreen`（以物易物双栏 + 骨币显示 + "不安全"氛围）+ `ThreatAssessmentBar`（NPC 对玩家的实时威胁评估 0-100 bar）+ `NpcReputationIndicator`（该 identity 对此 NPC 派系的信誉）+ `NpcMoodIcon`（NPC 头顶微表情/图标）
- **跨仓库契约**：纯 client 侧——server NPC state 已有，client 侧新增渲染层

---

## §0 设计轴心

- [ ] **对话气泡代替聊天栏**：NPC 说的话不淹没在聊天栏——世界内气泡（类似 MC name tag 但短暂浮现 5s）
- [ ] **交易保持紧张感**：双方距离 10 格、物品丢在地上互换——UI 不提供"安全交易确认"按钮
- [ ] **威胁可见但隐晦**：NPC 头顶不显示数字 HP bar，但可通过 stance/距离/对话语调推断其态度
- [ ] **道伥假示好**：坍缩渊内道伥可能模仿 NPC 示好（蹲伏、挥手）——UI 不区分真假，玩家靠经验辨别

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | `NpcDialogueBubbleRenderer`（世界内气泡，archetype 专属背景色: 散修灰/守墓人暗金/炼丹疯子紫/道伥苍白）+ 气泡最大 5s + 距离衰减 alpha + `NpcMoodIcon`（NPC 头顶 4 档 mood: 中立/警觉/敌对/恐惧 → 对应 icon 或微表情符号） | ⬜ |
| P1 | `NpcTradeScreen`：以物易物双栏（左=我方 offer / 右=NPC offer）+ 骨币 counter + "不安全"视觉语言（UI 边框为粗糙麻布质感、无确认按钮——双方拖放完成即成交）+ 散修交易时威胁条旁显（"此人气息在你之上/之下" textual）+ 游商傀儡特殊 UI（交易中地图显示傀儡主人大致方向） | ⬜ |
| P2 | `ThreatAssessmentBar`：当玩家靠近 NPC 时显示 NPC 对玩家的实时评估 0-100（仅凝脉+ 可见）—— 0-30 绿色（恭敬） / 30-60 黄色（中立） / 60-100 红色（准备翻脸）+ 翻脸瞬间视觉（UI 碎裂动画 + NPC stance 切换）+ `NpcReputationIndicator`：该 identity 对当前 NPC 派系信誉（高=绿色 / 低=红色） | ⬜ |
| P3 | NPC 记忆与对话链：`NpcMemoryBubble`（NPC 记得上次与你交易的结果 → 再次见面气泡提示 "上次你给的骨币是假的"）+ 散修"丢下低级资源疯狂逃窜"行为 → 地上掉落物品 3D tag + 逃跑粒子（烟尘）+ 枯骨休眠 → 摸遗骸时的"骸骨复苏" cutscene（micro 1s 动画 + 惊吓音效） | ⬜ |
| P4 | 坍缩渊 NPC 交互特殊规则：道伥假示好（蹲伏 + 挥手气泡 "…" → 玩家靠近后瞬间攻击）+ 执念半智能行为（可用物品引诱/误导）+ 秘境守灵 Boss 血条 UI（大血条在屏幕顶部 + 分段显示 + 阶段转换提示） | ⬜ |
| P5 | 多 NPC 交互压测（10 NPC + 1 玩家同时对话气泡/威胁条/mood icon 不重叠、不卡顿）+ 各 archetype × 各 mood × 各 reputation 交互 e2e | ⬜ |

---

## Finish Evidence（待填）

- **落地清单**：`NpcDialogueBubbleRenderer` / `NpcTradeScreen` / `ThreatAssessmentBar` / `NpcReputationIndicator` / `NpcMoodIcon` / `NpcMemoryBubble` / 坍缩渊特殊规则
- **关键 commit**：P0-P5 各自 hash
- **遗留 / 后续**：NPC voice 音效（不同 archetype 有不同喉音——需等 `plan-audio-implementation-v1`）
