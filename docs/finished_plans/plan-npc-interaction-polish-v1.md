# Bong · plan-npc-interaction-polish-v1 · 完成

NPC 交互深度打磨——在 `plan-npc-engagement-v1` ✅ finished 基础上拓展。npc-engagement-v1 已覆盖 **P0 名牌+inspect / P1 交易UI+信誉定价 / P2 对话框架+NPC音效**，建立了 `NpcMetadataS2c`/`NpcTradeScreen`/`NpcDialogueScreen` 完整管道。本 plan **不重复**这些基础，而是在其之上做 4 件事：① 世界内对话气泡（代替 Screen 弹窗，更沉浸）② NPC 威胁评估条（境界限制可见性）③ NPC 情绪与记忆可视化 ④ 坍缩渊 NPC 特殊交互规则。

**世界观锚点**：`worldview.md §七` 散修评估威胁度（"你气息绵长 → 恭敬交易 / 真元见底 → 拔刀爆装备"）视觉化 · `§九` 面对面以物易物 → 交易 UI 保持"不安全"的紧张感 · `§十一` NPC 信誉度反应分级（高=主动给情报 / 极低=通缉）→ 信誉条可视化 · `§十六` 坍缩渊内道伥/执念行为（假示好/伏击）→ 需要用 NPC 行为暗示而不是 UI 文字

**library 锚点**：`peoples-0007 散修百态`（拾荒/游荡/占山/假死四路 NPC 行为模式）

**前置依赖**：
- `plan-npc-engagement-v1` ✅ finished → **硬依赖**（本 plan 全部 P 在其 P0-P2 完成后才有意义）
- `plan-npc-ai-v1` ✅ → NPC 状态机 + big-brain AI
- `plan-social-v1` ✅ → NPC 信誉度 / 交易
- `plan-identity-v1` ✅ → NPC 对身份的差异化反应
- `plan-HUD-v1` ✅ → Toast / Zone HUD 层（对话气泡放 Toast 层）
- `plan-audio-world-v1` 🆕 active → NPC 反应音效（惊慌/威胁/交易）
- `plan-npc-visual-v1` 🆕 active → NPC 视觉差异化（mood icon 基于视觉区分）

**反向被依赖**：
- `plan-narrative-political-v1` ✅ → NPC 传播的信息通过气泡 UI 显示

---

## 与 npc-engagement-v1 的边界

| 维度 | npc-engagement-v1 已做 | 本 plan 拓展 |
|------|----------------------|-------------|
| 对话 | `NpcDialogueScreen`（全屏弹窗 + 选项菜单） | `NpcDialogueBubbleRenderer`（世界内浮动气泡，NPC 主动喊话/短句用气泡，深度对话仍走 Screen） |
| 交易 | `NpcTradeScreen`（双栏 + 骨币 + 信誉定价） | 交易中实时威胁条（旁栏显示"此人气息在你之上/之下"）+ 游商傀儡特殊 UI（地图显示傀儡主人方向） |
| inspect | `NpcInspectScreen`（右键打开面板） | 不碰。inspect 留在 engagement-v1 |
| 情绪 | 无 | `NpcMoodIcon`（NPC 头顶微表情 icon：中立/警觉/敌对/恐惧）+ mood 切换时的微动画 |
| 威胁评估 | 名牌变红 + 拒绝交易（binary） | `ThreatAssessmentBar`（0-100 连续条，凝脉+ 可见，带翻脸碎裂动画） |
| NPC 记忆 | 无 | `NpcMemoryBubble`（NPC 记得上次交互结果 → 再次见面气泡提示） |
| 坍缩渊 | 无 | 道伥假示好 / 执念引诱 / 秘境 Boss 血条 |

---

## 接入面 Checklist

- **进料**：`NpcMetadataS2c`（engagement-v1 P0 出料）/ `NpcDialogueS2c`（engagement-v1 P2 出料）/ `npc::NpcState { archetype, mood, threat_assessment }` / `social::Renown` / `identity::IdentityProfile` / `npc::brain::NpcAction` / `cultivation::Realm`（境界决定 ThreatBar 可见性）
- **出料**：`NpcDialogueBubbleRenderer`（世界内气泡 + archetype 专属样式）+ `ThreatAssessmentBar`（NPC 对玩家的实时威胁评估 0-100 bar）+ `NpcMoodIcon`（NPC 头顶微表情/图标）+ `NpcMemoryBubble`（NPC 记忆回调气泡）+ `NpcReputationIndicator`（该 identity 对此 NPC 派系的信誉）+ TSY 交互特殊规则
- **跨仓库契约**：server 新增 `NpcMoodS2c { entity_id, mood, threat_level }` packet → client consumer；server 新增 `NpcMemoryS2c { entity_id, memory_text }` → client 气泡

---

## §0 设计轴心

- [x] **气泡 ≠ 对话系统**：气泡用于 NPC 主动喊话（路过时 "…"、受伤时 "你找死！"、交易成功后 "…还算公道"）——不替代 `NpcDialogueScreen` 的选项对话
- [x] **威胁可见但隐晦**：NPC 头顶不显示数字 HP bar，但可通过 ThreatAssessmentBar + MoodIcon + stance/距离推断态度
- [x] **道伥假示好**：坍缩渊内道伥可能模仿 NPC 示好（蹲伏 + 挥手气泡 "…"）——UI 不区分真假，玩家靠经验辨别
- [x] **境界门槛**：ThreatAssessmentBar 仅凝脉+ 可见（低境看不到威胁评估——worldview §三 感知力进阶）
- [x] **记忆真实感**：NPC 记忆不是 tooltip——是 NPC"主动提起"的气泡（"上次你给的灵草是假的"），有 AI 驱动的时机选择（不是一见面就说）

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | 世界内对话气泡系统 | ✅ 2026-05-11 |
| P1 | NPC 情绪图标 + 情绪转换动画 | ✅ 2026-05-11 |
| P2 | 威胁评估条 + 信誉指示器 | ✅ 2026-05-11 |
| P3 | NPC 记忆气泡 + 行为暗示 | ✅ 2026-05-11 |
| P4 | 坍缩渊 NPC 特殊交互 | ✅ 2026-05-11 |
| P5 | 多 NPC 交互压测 + 全 archetype 矩阵验证 | ✅ 2026-05-11 |

---

## P0 — 世界内对话气泡 ✅ 2026-05-11

### 交付物

1. **`NpcDialogueBubbleRenderer`**（`client/src/main/java/com/bong/client/npc/NpcDialogueBubbleRenderer.java`）
   - NPC 头顶 2.5 block 位置浮动气泡（类似 MC nametag 但短暂浮现）
   - 气泡最大宽度 120px，超长自动换行，最大 3 行
   - 显示时长 3-6s（按文字长度缩放，每字 0.15s，最少 3s 最多 6s）
   - 距离衰减：15 格内全 alpha，15-25 格线性 alpha 衰减，25+ 隐藏
   - archetype 专属背景色：散修=灰褐(#8B7355) / 守墓人=暗金(#8B6914) / 炼丹疯子=浊紫(#6B3FA0) / 游商傀儡=铜锈绿(#5F8A5F) / 凡人=土黄(#C4A35A) / 道伥=苍白(#D0D0D0, alpha 0.6 — 半透明，暗示"不太对")
   - 字体：MC 默认字体，颜色 #F0F0F0（不用纯白避免刺眼）

2. **气泡触发协议**（`server/src/network/npc_bubble.rs`）
   - 新增 `NpcBubbleS2c { entity_id, text, duration_ticks, bubble_type }` packet
   - `bubble_type` enum：`Greeting`（路过触发）/ `Reaction`（被攻击/交易后）/ `Warning`（即将翻脸）/ `Memory`（记忆回调，P3 使用）
   - server 侧 `npc_bubble_system`：NPC 状态变化时按 archetype 模板生成 text → emit packet

3. **气泡与 `NpcDialogueScreen` 的区分**
   - 气泡：NPC 自发、短句、不可交互、有 archetype 颜色
   - DialogueScreen：玩家右键触发、有选项菜单、全功能对话
   - 气泡出现时如果玩家正在 DialogueScreen 内 → 不显示气泡（避免冲突）

### 验收抓手

- 测试：`server::npc::tests::bubble_text_by_archetype` / `client::npc::tests::bubble_alpha_distance_decay` / `client::npc::tests::bubble_hidden_during_dialogue_screen`
- 手动：靠近散修 → 头顶灰褐气泡 "道友…" 3s 消失 → 靠近守墓人 → 暗金气泡 → 同时在 DialogueScreen 中不弹气泡

---

## P1 — NPC 情绪图标 + 转换动画 ✅ 2026-05-11

### 交付物

1. **`NpcMoodIcon`**（`client/src/main/java/com/bong/client/npc/NpcMoodIcon.java`）
   - NPC 名牌上方（名牌由 engagement-v1 P0 提供）0.5 block 位置
   - 4 档 mood → 对应微表情符号：
     - `NEUTRAL`（默认）：无 icon（不增加视觉噪声）
     - `ALERT`：黄色 `!`（12×12px 贴图，非文字）
     - `HOSTILE`：红色 `!!`（14×14px，微振动 shader）
     - `FEARFUL`：灰蓝 `?!`（12×12px，微抖动 0.5px 幅度）
   - icon 出现/消失使用 0.3s alpha fade（不瞬出瞬消）

2. **`NpcMoodS2c` 协议**（`server/src/network/npc_mood.rs`）
   - `NpcMoodS2c { entity_id, mood: NpcMood, threat_level: f32 }` packet
   - server 侧 `npc_mood_sync_system`：NPC `ThreatAssessment` component 变化时 → emit
   - 批量同步：每 20 tick 扫描 32 格内 NPC，mood 有变化才 emit（避免洪流）

3. **mood 转换微动画**
   - NEUTRAL → ALERT：icon 从 0 alpha → 1.0 + 微放大 1.2× → 1.0（0.3s ease-out）
   - ALERT → HOSTILE：黄色 → 红色 color lerp 0.2s + icon 从 `!` swap 到 `!!` + 名牌颜色同步变红
   - 任意 → FEARFUL：icon 出现 + NPC 世界内后退 1 步动画（由 server big-brain `FleeAction` 驱动，client 侧不伪造）
   - HOSTILE → NEUTRAL（脱战）：icon 红色 → 灰 → 消失 2s 慢 fade

### 验收抓手

- 测试：`server::npc::tests::mood_change_emits_packet` / `client::npc::tests::mood_icon_alpha_fade` / `client::npc::tests::mood_transition_color_lerp`
- 手动：站在散修旁 → 无 icon → 攻击其盟友 → 黄色 `!` 出现 → 继续靠近 → 红色 `!!` 振动 → 脱战 → 慢消失

---

## P2 — 威胁评估条 + 信誉指示器 ✅ 2026-05-11

### 交付物

1. **`ThreatAssessmentBar`**（`client/src/main/java/com/bong/client/npc/ThreatAssessmentBar.java`）
   - **仅凝脉+ 玩家可见**（醒灵/引气看不到——worldview §三 感知力阶梯）
   - 位置：锁定目标时屏幕 `TargetInfoHudPlanner`（engagement-v1 已建）条下方
   - 外观：窄条 80×6px，三色段：0-30 深绿（恭敬）/ 30-60 暗黄（中立）/ 60-100 暗红（准备翻脸）
   - 数值来自 `NpcMoodS2c.threat_level`（0.0-1.0 → 0-100 映射）
   - 条旁文字提示（非数字）：`"恭敬"` / `"警惕"` / `"杀意"` / `"已癫狂"`（>90）
   - **翻脸碎裂动画**：threat > 90 时 bar 碎裂（碎片粒子向四周飞散 0.5s）→ NPC 名牌变红（触发 engagement-v1 的名牌颜色机制）→ 进入战斗

2. **`NpcReputationIndicator`**（`client/src/main/java/com/bong/client/npc/NpcReputationIndicator.java`）
   - 在 `NpcInspectScreen`（engagement-v1 P0 已建）内追加信誉段
   - 小条 60×4px：高(>50)=绿 / 中(0-50)=灰 / 低(<0)=橙 / 极低(<-50)=红
   - 旁文字：`"信任"` / `"中立"` / `"提防"` / `"敌视"`
   - 信誉数据来自 `NpcMetadataS2c.reputation_to_player`（engagement-v1 P0 已有）

3. **固元+ 额外感知**
   - 固元境+ 玩家：ThreatAssessmentBar 追加"NPC 真元大致水位"（低/中/高三档文字，不给数字）
   - 通灵境+ 玩家：可看到 NPC 内心独白（`NpcMoodS2c` 追加 `inner_monologue: Option<String>`，server 按 NPC archetype 生成——"此人真元快空了，动手！" / "打不过，先跑"）

### 验收抓手

- 测试：`client::npc::tests::threat_bar_hidden_below_ningmai` / `client::npc::tests::threat_bar_color_segments` / `client::npc::tests::flip_shatter_animation` / `client::npc::tests::reputation_indicator_in_inspect`
- 手动：凝脉角色锁定散修 → 目标条下方出现绿色威胁条 → 攻击其同伴 → 条变黄变红 → 90 碎裂 → NPC 攻击 → 通灵角色看到 NPC 内心独白

---

## P3 — NPC 记忆气泡 + 行为暗示 ✅ 2026-05-11

### 交付物

1. **`NpcMemoryBubble`**（复用 P0 的 `NpcDialogueBubbleRenderer`，`bubble_type = Memory`）
   - NPC 记得与该玩家最近 3 次交互：交易结果 / 被攻击 / 被偷窃 / 被帮助
   - 再次见面时 50% 概率主动气泡（不是每次，避免话唠）
   - 记忆模板按 archetype：
     - 散修（被骗过）："…你。上次的骨币，成色不对。"
     - 散修（友好交易过）："道友，还有灵草出让吗？"
     - 凡人（被打过）："大仙饶命！小人再不敢了…"
     - 守墓人（被打扰过）："…又来？"
   - server 侧 `NpcMemoryComponent { interactions: Vec<NpcMemoryEntry> }`（最多 8 条，FIFO）
   - `NpcMemoryEntry { player_uuid, interaction_type, timestamp, outcome }`

2. **NPC 行为暗示粒子**
   - 散修"丢下低级资源疯狂逃窜"行为（已有 `FleeAndDropAction`）→ 掉落物品 3D tag（物品名浮动 2s）+ NPC 逃跑时脚底烟尘粒子（`BongSpriteParticle` `cloud256_dust` tint 灰 × 2 per tick）
   - NPC "考虑翻脸"时的预兆行为：微后退 + 手持武器切换（server 发 equipment change → client 看到 NPC 从空手变握刀）+ 气泡 "…"（沉默气泡 = 不祥之兆）
   - 枯骨休眠体被靠近时：0.5s 微颤动（entity position ±0.02 per tick × 10 tick）→ 复苏后爆发 `combat_aggro` 音效

3. **NPC 交互历史 HUD**（`NpcInteractionLogHudPlanner.java`）
   - 按 F7（可配置）打开简洁列表：最近 10 个交互过的 NPC（名字 + archetype + 最后交互类型 + 时间戳）
   - 纯查阅，不可操作
   - 数据来自 client 侧 `NpcInteractionLog`（本地缓存，不 persist 到 server）

### 验收抓手

- 测试：`server::npc::tests::memory_component_fifo_8` / `server::npc::tests::memory_bubble_probability` / `client::npc::tests::flee_dust_particles` / `client::npc::tests::interaction_log_max_10`
- 手动：与散修交易 → 离开 → 再次靠近 → 50% 概率气泡提起上次交易 → 攻击凡人 → 离开 → 再来 → "大仙饶命"

---

## P4 — 坍缩渊 NPC 特殊交互 ✅ 2026-05-11

### 交付物

1. **道伥假示好**
   - 道伥（`TsyHostileArchetype::DaoChang`）在远距离时模仿 NPC 行为：蹲伏 + 挥手气泡 "…"（使用 P0 气泡系统，苍白色半透明）
   - 玩家靠近 8 格内时：0.3s 延迟 → 瞬间切换 HOSTILE → mood icon 红色 `!!` 出现 → 攻击
   - **关键设计**：气泡系统/mood icon 对道伥不做特殊标记——玩家无法从 UI 分辨真假，只能靠经验（"苍白色气泡的都是道伥？但有的散修本来就脸色苍白…"）
   - 通灵境+ 玩家：ThreatAssessmentBar 会提前显示红色（道伥即使假示好，threat_level 仍 > 80），但低境看不到

2. **执念引诱**
   - 执念体（`TsyHostileArchetype::Obsession`）半智能行为：可用物品引诱/误导
   - 执念靠近低价值物品 → 气泡 "…这是…"（像是在犹豫）→ 拾取后继续巡逻
   - 执念靠近高价值物品 → 奔向物品 + 拾取 → 5s 后警戒解除（利用这个窗口偷跑）
   - 引诱 HUD 提示（通灵+）：crosshair 右下角小文字 "它对灵物有执念"（仅首次提示，后续不重复）

3. **秘境 Boss 血条**
   - 秘境守灵 Boss 战：屏幕顶部全宽大血条（`TsyBossHealthBar.java`）
   - 分段显示：每段 = 一个 phase（3-5 段）
   - 阶段切换时 bar 闪白 0.3s + 段间分隔线碎裂 + boss 气泡 "…第二次了…"
   - Boss 名称 + realm 显示在 bar 上方
   - 非 Boss 战斗时不显示（不和 `TargetInfoHudPlanner` 冲突）

4. **干尸化死亡**（坍缩渊内特殊死亡）
   - 坍缩渊内死亡 → 正常 `DeathSoulDissipatePlayer` + 追加干尸化 VFX：
     - 玩家模型颜色 lerp → 灰褐 1s
     - 模型缩小 0.9×（脱水干瘪感）
     - 灰尘粒子从体表飘出
   - 死亡遗念追加"秘境所得 100% 掉落"文字提示（worldview §十六）

### 验收抓手

- 测试：`server::tsy::tests::dao_chang_lure_flip_timing` / `client::npc::tests::boss_health_bar_phases` / `client::npc::tests::boss_bar_hidden_in_normal_combat`
- 手动：进入坍缩渊 → 远处 NPC 蹲着挥手 → 靠近 → 被偷袭 → 高境角色再试 → ThreatBar 提前预警 → Boss 战 → 大血条分段 → 死亡 → 干尸化

---

## P5 — 多 NPC 压测 + 全 archetype 矩阵 ✅ 2026-05-11

### 交付物

1. **多 NPC 交互压测**
   - 场景：10 NPC + 1 玩家，所有 NPC 同时发气泡/mood icon/威胁条
   - 断言：气泡不重叠（同一 NPC 同时只有 1 气泡）/ mood icon 不闪烁 / ThreatBar 不卡顿
   - 帧率：30fps 基线下 NPC 交互渲染开销 < 2ms

2. **9 archetype × 4 mood × 3 reputation 矩阵**
   - 每个组合：气泡文字正确 / mood icon 正确 / ThreatBar 阈值正确 / 翻脸动画触发正确
   - 特别关注：道伥的假示好→翻脸路径 / 守墓人的极高 threat 但不翻脸（守墓人 threshold 更高）

3. **坍缩渊交互 e2e**
   - 完整 TSY 跑一遍：入场 → 遇道伥 → 被假示好 → 识破/被偷袭 → 遇执念 → 引诱 → Boss 战 → 死亡干尸化 → 重生

### 验收抓手

- 压测工具：`scripts/npc_interaction_stress.sh`（spawn 10 NPC + 1 bot player）
- 矩阵覆盖：`server::npc::tests::archetype_mood_reputation_matrix`（9×4×3 = 108 组合）

---

## Finish Evidence

- **落地清单**
  - P0 世界内气泡：`server/src/network/npc_bubble.rs` 新增 `NpcBubbleS2c` / `NpcBubbleType` / `emit_npc_bubble_payloads` / `emit_npc_reaction_bubbles`；`client/src/main/java/com/bong/client/npc/NpcDialogueBubbleRenderer.java` + `NpcBubbleHandler.java` 接 `bong:npc_bubble`，支持距离 alpha、3 行换行、DialogueScreen 抑制和同 NPC 单气泡替换。
  - P1 情绪图标：`server/src/network/npc_mood.rs` 新增 `NpcMoodS2c` / `NpcMood` / 20 tick sync；`client/src/main/java/com/bong/client/npc/NpcMoodIcon.java` / `NpcMoodStore.java` / `NpcMoodHandler.java` 接 mood icon、fade/抖动命令和 threat 缓存。
  - P2 威胁与信誉：`ThreatAssessmentBar.java` 接 `TargetInfoHudPlanner`，凝脉+ 可见、固元+ 真元水位、通灵+ inner monologue；`NpcReputationIndicator.java` 接入 `NpcInspectScreen`。
  - P3 记忆和交互历史：`server/src/npc/interaction_memory.rs` 新增 `NpcMemoryComponent` / `NpcMemoryEntry` FIFO 8 和 50% 稳定概率；`client/src/main/java/com/bong/client/npc/NpcInteractionLogStore.java` / `NpcInteractionLogHudPlanner.java` / `NpcInteractionLogControls.java` 提供 F7 最近 10 NPC 交互列表。
  - P4 坍缩渊 polish：`server/src/npc/tsy_hostile.rs` 增加道伥假示好翻脸 delay、执念高价值引诱窗口 helper；`server/src/network/tsy_polish.rs` 发 `bong:tsy_boss_health` / `bong:tsy_death_vfx`；`client/src/main/java/com/bong/client/tsy/` 新增 Boss 血条、阶段闪白、干尸化死亡 VFX HUD 层。
  - P5 压测入口：`scripts/npc_interaction_stress.sh` 固定 JDK 17，串行运行 NPC bubble/mood/memory/TSY server contract 测试和 client NPC/TSY HUD 矩阵测试。

- **关键 commit**
  - `baf264ea0`（2026-05-11）`plan-npc-interaction-polish-v1: 接入 NPC 交互事件协议`
  - `ac9032675`（2026-05-11）`plan-npc-interaction-polish-v1: 客户端渲染 NPC 交互 HUD`
  - `74f32e6d5`（2026-05-11）`plan-npc-interaction-polish-v1: 补 NPC 交互压测脚本`
  - `56933ba83`（2026-05-11）`plan-npc-interaction-polish-v1: 修复 NPC 同步失败重试`
  - `88a8dd19a`（2026-05-11）`plan-npc-interaction-polish-v1: 收紧客户端状态清理`
  - `9f930393d`（2026-05-11）`plan-npc-interaction-polish-v1: 收敛 review 性能与威胁判定`

- **测试结果**
  - `cargo fmt --check` ✅
  - `CARGO_PROFILE_TEST_DEBUG=0 cargo test -j1 -- --test-threads=1` ✅ `4349 passed; 0 failed`
  - `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo clippy --all-targets -j1 -- -D warnings` ✅
  - `JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 PATH=/usr/lib/jvm/java-17-openjdk-amd64/bin:$PATH ./gradlew test build` ✅ `BUILD SUCCESSFUL`
  - `bash scripts/npc_interaction_stress.sh` ✅ server 定向 `17 passed` + client NPC/TSY HUD targeted Gradle `BUILD SUCCESSFUL`
  - `git diff --check` ✅
  - review follow-up: `cargo fmt --check` ✅，`cargo test network::npc_mood -j1 -- --test-threads=1` ✅ `4 passed`，`./gradlew test --tests "com.bong.client.npc.NpcMoodStoreTest"` ✅

- **跨仓库核验**
  - server：`NpcBubbleS2c` / `NpcMoodS2c` / `TsyBossHealthS2c` / `TsyDeathVfxS2c` payload JSON 均有 contract 测试；`network::mod` 已注册 bubble、mood、TSY polish systems/resources；NPC 成功交易路径已调用 `record_player_npc_interaction` 写入 memory。
  - review follow-up：`npc_mood` / `tsy_boss_health` 仅在 payload 成功发送后刷新 `last_sent`，序列化失败不会封口后续重试；`NpcMoodStore.upsert` 已按 `updatedAtMillis` 拒绝旧包覆盖，并补 `NpcMoodStoreTest`。
  - review follow-up 2：`NpcMoodStore.snapshot()` 改为无排序浅拷贝，渲染方自行决定是否排序；`threat_level_for` 里 Daoxiang 的 floor 统一在 match 内计算，避免重复覆盖。
  - client：`BongNetworkHandler` 注册 `bong:npc_bubble`、`bong:npc_mood`、`bong:tsy_boss_health`、`bong:tsy_death_vfx`；`BongHudOrchestrator` 统一渲染 bubble、mood、ThreatBar、交互日志、TSY Boss/死亡 VFX。
  - agent/worldgen：本 plan 不改 agent schema 或 worldgen 产物。

- **遗留 / 后续**
  - NPC voice 音效仍等待 `plan-audio-world-v1` 的 NPC 音效 recipe，再接不同 archetype 喉音 / 威胁声。
  - NPC 表情/骨骼动画仍等待 `plan-player-animation-implementation-v1`；本 plan 只落客户端 HUD/renderer 命令层，不改模型骨骼。
  - 云端未执行真实 `./gradlew runClient` 视觉验收，也未跑真实 MC bot session；`scripts/npc_interaction_stress.sh` 是确定性 contract/harness 压测入口，不伪装为 10 NPC + 1 bot 的 live 场景录像。
