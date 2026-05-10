# Plan: NPC Engagement v1（NPC 交互层）

> NPC 后端成熟（9 archetype、19 scorer/action、5000 dormant 容量、qi_physics 守恒），但**玩家看到的是一堆沉默的 Villager 皮**——不能对话、不能交易、不能 inspect、分不清散修和凡人。本 plan 让 NPC 从"移动战利品容器"变成"有身份的修仙者"。

---

## 接入面 Checklist（防孤岛）

- **进料**：`npc::NpcArchetype` ✅ / `npc::faction::FactionMembership` ✅ / `npc::faction::Reputation` ✅ / `cultivation::Realm` ✅ / `npc::lifecycle::NpcLifespan` ✅ / `npc::scattered_cultivator` ✅（trade/robbery 逻辑存在）/ `social::Renown` ✅
- **出料**：client NPC nametag renderer → `client/src/main/java/com/bong/client/npc/` / inspect UI → `NpcInspectScreen.java` / trade UI → `NpcTradeScreen.java` / 对话框架 → `NpcDialogueScreen.java`
- **共享类型/event**：新增 `NpcMetadataS2c { entity_id, archetype, realm, faction_name, faction_rank, reputation_to_player }` packet（server → client 同步 NPC 元数据）/ 新增 `NpcTradeRequestC2s` / `NpcTradeResponseS2c`
- **跨仓库契约**：server 新增 `bong:npc_metadata` CustomPayload channel → client `NpcMetadataStore` 消费
- **worldview 锚点**：§十一 信息价值（"信息比装备值钱"）/ §六 个性三层 / §九 经济（骨币交易）

---

## 阶段总览

| 阶段 | 目标 | 状态 |
|------|------|------|
| P0 | NPC 名牌 + 基础 inspect | ⬜ |
| P1 | 商人交易 UI + 信誉度机械效果 | ⬜ |
| P2 | 基础对话框架 + NPC 音效 | ⬜ |

---

## P0 — NPC 名牌 + 基础 inspect ⬜

### 交付物

1. **NPC 元数据同步**（`server/src/network/npc_metadata.rs`）
   - 新增 `NpcMetadataS2c` packet：`{ entity_id, archetype, realm, faction_name, faction_rank, reputation_to_player, display_name }`
   - 玩家进入 64 格范围时 sync；NPC 状态变化时 resync
   - 注册 `bong:npc_metadata` CustomPayload channel

2. **NPC 名牌渲染**（`client/src/main/java/com/bong/client/npc/NpcNametagRenderer.java`）
   - 头顶浮动文字：`[散修·凝脉]` / `[魔修派·真传弟子]`（archetype + realm + faction）
   - 颜色编码：hostile=红 / neutral=灰 / friendly=绿（基于 reputation_to_player）
   - 境界差距提示：比玩家高 2+ 境界 → 名牌加 `⚠` 前缀（"危险"暗示）
   - 距离衰减：20 格内显示全名牌，20-40 格仅显示 archetype icon，40+ 隐藏

3. **NPC inspect UI**（`client/src/main/java/com/bong/client/npc/NpcInspectScreen.java`）
   - 右键 NPC 打开 inspect 面板（仅 peaceful 状态，战斗中不可用）
   - 显示：archetype / realm / faction / 寿元比例（不精确，"正值壮年"/"风烛残年"）/ 境界描述
   - 高境界玩家 inspect 低境界 NPC：额外显示真元池大致范围
   - 低境界玩家 inspect 高境界 NPC：显示"你看不清此人深浅"

4. **NpcMetadataStore**（`client/src/main/java/com/bong/client/npc/NpcMetadataStore.java`）
   - client 侧缓存 NPC 元数据，entity despawn 时清理

### 验收抓手

- 测试：`server::network::tests::npc_metadata_packet_serializes` / `client::npc::tests::nametag_color_by_reputation`
- 手动：靠近 NPC → 头顶出现 `[散修·引气]` 灰色名牌 → 右键 → 打开 inspect 面板

---

## P1 — 商人交易 + 信誉度效果 ⬜

### 交付物

1. **NPC 交易 UI**（`client/src/main/java/com/bong/client/npc/NpcTradeScreen.java`）
   - 双栏布局：左=NPC 出售物品列表 / 右=玩家出价物品槽
   - 价格以骨币为基准（worldview §九），显示半衰期剩余
   - NPC 根据 `scattered_cultivator.rs` 已有 trade 逻辑提供物品（丹药/灵草/残卷）

2. **信誉度→定价**（`server/src/npc/scattered_cultivator.rs` 增强）
   - `Reputation > 50`：价格 ×0.8（友善折扣）
   - `Reputation < -30`：拒绝交易 + 名牌变红
   - `Reputation < -70`：主动攻击（已有 FearCultivatorScorer 框架，反向利用）

3. **交易协议**
   - `NpcTradeRequestC2s { npc_entity_id, offered_items, requested_item_id }`
   - `NpcTradeResponseS2c { success, reason, final_items }`
   - server 校验：物品存在 + 骨币足够 + reputation 允许 + NPC 仍存活

4. **NPC 拒绝交互反馈**
   - reputation 不足时右键 NPC：名牌闪红 + 音效 `npc_refuse.json`（`minecraft:entity.villager.no` pitch 0.8）
   - inspect 面板显示"此人对你充满敌意"

### 验收抓手

- 测试：`server::npc::tests::trade_reputation_pricing` / `server::npc::tests::trade_reject_low_reputation`
- 手动：找到散修 → 右键交易 → 用骨币买灵草 → 杀掉同派系 NPC → 再来交易被拒绝

---

## P2 — 基础对话 + NPC 音效 ⬜

### 交付物

1. **对话框架**（`client/src/main/java/com/bong/client/npc/NpcDialogueScreen.java`）
   - 右键 NPC 首先进入对话界面（不直接跳 inspect）
   - 3 个基础选项：「查看」(→ inspect) / 「交易」(→ trade，仅 Rogue/Commoner) / 「离开」
   - 对话文案由 server 下发（`NpcDialogueS2c { greeting_text, options[] }`），不硬编码
   - NPC greeting 按 archetype 模板：散修="道友，可有灵草出让？" / 凡人="大仙，小人不敢…"

2. **NPC 音效**
   - `npc_greeting_cultivator.json`：`minecraft:entity.villager.ambient`(pitch 0.9, volume 0.4)（散修招呼）
   - `npc_greeting_commoner.json`：`minecraft:entity.villager.ambient`(pitch 1.2, volume 0.3)（凡人怯声）
   - `npc_hurt.json`：`minecraft:entity.player.hurt`(pitch 0.7)（NPC 受击）
   - `npc_death.json`：`minecraft:entity.player.death`(pitch 0.6) + `minecraft:block.soul_sand.break`（NPC 死亡）
   - `npc_aggro.json`：`minecraft:entity.pillager.celebrate`(pitch 0.5, volume 0.6)（NPC 进入攻击）
   - server emit：NPC 状态切换时 emit 对应 audio

### 验收抓手

- 测试：`server::npc::tests::dialogue_greeting_by_archetype` / `server::npc::tests::npc_death_emits_audio`
- 手动：靠近散修 → 听到招呼声 → 右键 → 对话菜单 → 选交易 → 完成交易 → 攻击 NPC → 听到受击/死亡音效

---

## 前置依赖

| 依赖 plan | 状态 | 用到什么 |
|-----------|------|---------|
| plan-npc-ai-v1 | ✅ finished | 9 archetype / big-brain thinker / FearCultivatorScorer |
| plan-social-v1 | ✅ finished | Renown / Reputation component |
| plan-npc-virtualize-v1 | ✅ finished | dormant/hydrated 双态 / NpcMetadata 概念 |
| plan-npc-perf-v1 | ✅ finished | spatial index / LOD gate |
| plan-economy-v1 | ✅ finished | 骨币经济 / 半衰期 |
| plan-audio-v1 | ✅ finished | SoundRecipePlayer / AudioTriggerS2c |
| plan-HUD-v1 | ✅ finished | BongHudOrchestrator |
| plan-identity-v1 | ✅ finished | IdentityProfile / DuguRevealedEvent consumer |

**全部依赖已 finished，无阻塞。**
