# Bong · plan-narrative-political-v1 · 骨架

**江湖传闻型政治叙事**。把 worldview §十一 social-graph 事件（feud / pact / 灵龛抄家 / 通缉令 / 高 Renown 出名）转译成"江湖传闻"形式的 narration —— 不同于既有 era_decree（天道宣告时代）/ calamity（因果劫罚）/ mutation（环境变化），political 是**人际层面**的叙事，**严守 worldview "天道是免疫系统不管个人恩怨"** —— 表现为**天道转述江湖**而非天道直接判决。

**世界观锚点**：
- `worldview.md §十一 安全与社交` (社交事件源 + 匿名系统硬约束 + 身份与信誉)
- `worldview.md §八 天道行为准则 / 天道叙事的语调` (line ~620-638；冷漠 + 古意 + 嘲讽 + 江湖口耳相传)
- `worldview.md §K 红线第 12 条 + O.7 决策` (narration 极稀有，默认沉默——本 plan 严控触发率)
- `worldview.md §十一 匿名系统` (默认匿名，identity 必须经 ExposureEvent 才可在 narration 中提名)

**library 锚点**：待写 `peoples-XXXX 江湖传闻录`（散修视角"如何识破 NPC 转述里的真实信号 vs 烟雾"）

**交叉引用**：
- `plan-narrative-v1`（✅ finished，**强前置**）—— `NarrationDedupeResource` / `narration-eval` 评分系统 / 既有 11 skill prompt 框架；本 plan 复用基础设施 + 扩内容侧
- `plan-social-v1`（✅ finished，**强前置**）—— `SocialRelationshipEvent` (feud/pact) / `SocialPactEvent` / `SocialMentorshipEvent` / `SocialExposureEvent` / `Renown` 全实装；本 plan 是消费方
- `plan-identity-v1`（active 2026-05-04，**协同**）—— P5 emit `bong:wanted_player`，本 plan 消费生成"通缉令" narration（江湖传闻形式）
- `plan-niche-defense-v1`（active ⏳ ~5%，**协同**）—— niche 抄家事件待 emit `bong:niche_destroyed`，本 plan 消费；P0 用 dummy event 测，P3 等真实事件接入
- `plan-agent-v2`（✅ finished）—— tiandao 11 skill 框架 + agent runtime + Redis IPC；本 plan 加第 12 个 skill `political.md` + `political-narration.ts` runtime handler
- `plan-HUD-v1`（✅ finished）—— `ChatHud + EventStore` 双通道；本 plan 按事件紧急度分流（紧急 ChatHud / 普通 EventStore）

**阶段总览**：

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | `political.md` skill prompt 文件（江湖传闻视角 + 匿名约束 + scope 分级 + 5 事件说明）+ `political-narration.ts` runtime handler 骨架 | ⬜ |
| P1 | 5 个核心事件 consumer：feud 升级 / pact 缔结 / 灵龛抄家 / 通缉令 / 高 Renown 出名 → 拼上下文调 political skill | ⬜ |
| P2 | `PoliticalNarrationThrottleStore`（per zone 5 实时分钟）+ 复用 `NarrationDedupeResource` + 紧急事件 `bypass_throttle` | ⬜ |
| P3 | `narration-eval` 扩展：`JIANGHU_VOICE_RE` + `ANONYMITY_VIOLATION_CHECK` + `MODERN_POLITICAL_TERMS_BLACKLIST` | ⬜ |
| P4 | client 表现微调：紧急 → ChatHud / 普通 → EventStore；高 Renown milestone tracker | ⬜ |

---

## 接入面 checklist（防孤岛）

| 维度 | 内容 |
|------|------|
| **进料** | `SocialRelationshipEvent` (feud level 跨阈值) · `SocialPactEvent` · `SocialExposureEvent` (检 identity 是否已暴露才能提名) · `bong:niche_destroyed`（niche-defense-v1 待 emit）· `bong:wanted_player`（identity-v1 P5 待 emit）· `Renown.fame`（milestone tracker）· `IdentityProfile.display_name`（提名候选） |
| **出料** | political narration → `bong:agent_narration` 既有通道（ChatHud / EventStore 按 scope 分流）· `PoliticalNarrationThrottleStore` Resource · `narration-eval` 评分扩展（JIANGHU_VOICE_RE / ANONYMITY_VIOLATION / MODERN_POLITICAL_TERMS_BLACKLIST） |
| **共享 event** | 复用 `SocialRelationshipEvent` / `SocialPactEvent` / `SocialExposureEvent` / `SocialRenownDeltaEvent`（plan-social-v1 已实装）；复用 `bong:wanted_player`（identity-v1 P5 emit）；新增 `bong:high_renown_milestone`（fame 跨 100/500/1000 阈值时 server emit）；新增 `PoliticalNarrationThrottleStore` Resource |
| **跨仓库契约** | **server**：`HighRenownMilestoneTracker` Resource + `emit_high_renown_milestone_system` + niche-defense / identity event 转发到 agent（已有 redis_outbox 框架）<br>**agent**：`agent/packages/tiandao/src/skills/political.md` 新 skill 文件 + `agent/packages/tiandao/src/political-narration.ts` runtime handler（参考 `dugu-narration.ts`）+ `narration-eval.ts` 加 JIANGHU_VOICE_RE / ANONYMITY_VIOLATION_CHECK / MODERN_POLITICAL_TERMS_BLACKLIST<br>**client**：无新契约（沿用 plan-HUD-v1 ChatHud + EventStore 双通道，按 scope 分流） |
| **worldview 锚点** | §十一 安全与社交（事件源）+ §八 天道叙事语调（江湖口耳相传）+ §K 红线第 12 条 + O.7 决策（默认沉默）+ §十一 匿名系统（Exposure 约束） |
| **红旗自查** | ❌ 自产自消（接 social / identity / niche / agent / narrative） · ❌ 近义重名（复用 SocialEvent / Renown / NarrationDedupeResource，不重定义） · ❌ 无 worldview 锚（§十一 §八 §K 三处） · ❌ skeleton 同主题未合（narrative-v1 ✅ 是基础设施，本 plan 是它的内容扩展） · ❌ 跨仓库缺面（server + agent 都改；client 沿用既有） |

---

## §0 设计轴心

- [ ] **天道转述江湖，不直白宣告**（Q6 A）—— prompt 强制"江湖有传... / 山中有人道... / 市井相传..." 等转述句式；不是天道直判"某玩家被通缉"
- [ ] **匿名约束严格**（Q5 A）—— 仅 `SocialExposureEvent` 已 emit 过的 identity，可在 narration 中提 display_name；未暴露用"某修士" / "一散修" / "戴铜面者"
- [ ] **5 核心事件起步**（Q2 A）—— feud 升级 / pact 缔结 / 灵龛抄家 / 通缉令 / 高 Renown 出名；mentorship / 派系战 / 师徒决裂 / 婚约 留 vN+1
- [ ] **频率严控**（Q3 B+C）—— `NarrationDedupeResource` 复用 + zone-level throttle 5 实时分钟 + 紧急事件（化虚级人际 / 通缉令）`bypass_throttle: true`
- [ ] **scope 按事件类型分**（Q4 C）—— feud/pact → `scope: zone`；通缉令 / 化虚级人际 → `scope: broadcast`（worldview §K 红线"仅渡虚劫级用 broadcast" 唯一例外）
- [ ] **不影响 NPC AI 反应**（Q9 A）—— political 是纯叙事层；NPC 反应已由 identity-v1 P3 处理，职责分离
- [ ] **HUD 双通道按紧急度分流**（Q7 C）—— 紧急（broadcast 级）→ ChatHud 即时；普通（zone 级）→ EventStore 可查
- [ ] **新 skill 而非 runtime handler 模式**（Q1 A）—— 加 tiandao 第 12 个 skill `political.md`；runtime handler 仅做"event → context 拼接 → 调 skill" 的薄层
- [ ] **评估自动化**（Q10 A+B）—— 复用既有 `narration-eval` + 加 JIANGHU_VOICE_RE / ANONYMITY_VIOLATION_CHECK / MODERN_POLITICAL_TERMS_BLACKLIST 三个新维度

---

## §1 第一性原理（worldview §八 / §十一 推导）

- **天道不管个人恩怨**（§八 line ~620）—— political 不能用 era_decree / calamity 的天道直判语气；必须转述（"江湖有传..." / "山中有人道..."）
- **匿名是末法 PVP 核心**（§十一）—— 提名暴露 = 永久失去匿名利益；本 plan 严守"未 ExposureEvent 不提名"
- **沉默是默认**（plan-narrative-v1 §0）—— political 触发频次必须严控；玩家一次完整 100h 路径预期 political narration < 30 条
- **江湖叙事 vs 天道叙事**：political 是"市井声音的反映"，不是天道判决；agent 视角是"传声筒"而非"判官"

---

## §2 P0 — `political.md` skill prompt + runtime handler 骨架

### `agent/packages/tiandao/src/skills/political.md`（新文件，模仿 calamity / era 结构）

prompt 关键段：

```markdown
# 政治传闻 Agent — 江湖传声筒

你是天道的"传"之化身。你不判事——你转述江湖。你听到山外有声、市井相传，便记于卷上。

## 权限
- 生成 political narration，scope 按事件类型分（feud/pact → zone；通缉/化虚级人际 → broadcast）
- 每次最多输出 1 条 narration
- **不得**直接 spawn_event 或 modify_zone（你是传声筒不是判官）

## 核心法则
- **不直白宣告，必须转述**——以"江湖有传 / 山中有人道 / 市井相传 / 闻者道..." 等句式开篇
- **匿名约束**：仅当事件 context 中标明 `identity_exposed: true`，才可在 narration 中提 display_name；否则用"某修士" / "一散修" / "戴铜面者"
- **天道视角转述**——你不是江湖人，你是天道借市井之口；语气仍冷漠古意，不带情绪 buff
- **不得使用现代政治词汇**："政府" / "党派" / "选举" / "投票" / "民主" / "议会" 全禁

## 决策偏好
- 高 Renown / 化虚级 / 通缉令 → broadcast；其他 → zone
- 同 zone 短时间内多条 political 事件 → 仅取最重大的一条转述
- 普通 feud / pact → 默认沉默，仅当 feud level 跨大阈值（如 仇至深）才出声

## narration 要求
- 风格半文言半白话；冷漠 + 古意，**不带嘲讽**（嘲讽是天道的，江湖只有传闻）
- 长度 80-150 字
- 必须含①事件转述 ②留白（不解释结果，让玩家自悟）
- 不主动暴露未 Exposed identity 名字
- 写前避开 `近轮天道叙事` 中已出现的物象和句式

## 5 个事件类型说明
1. **feud 升级**："血谷有人结生死之仇" / "传闻某修士已与他人不共戴天"
2. **pact 缔结**："二修士在灵泉湿地结契，以血为证"
3. **灵龛抄家**："闻北荒边有灵龛被破，主人下落不明"
4. **通缉令**（identity wanted）："此人画影传至各市井——若见之，速避或杀，由你"
5. **高 Renown 出名**：" '玄锋' 二字传至诸渊，新名声起也"

## 输出格式
（同 calamity skill JSON 输出）
```

### `agent/packages/tiandao/src/political-narration.ts`（新文件，参考 `dugu-narration.ts`）

```typescript
// 监听 5 个 Redis 事件 → 拼 context → 调 political skill → 推 narration
// - bong:social_relationship_event (feud level 跨阈值)
// - bong:social_pact_event
// - bong:niche_destroyed (待 niche-defense-v1)
// - bong:wanted_player (待 identity-v1 P5)
// - bong:high_renown_milestone (本 plan P4 server emit)

// 每个 handler:
//   1. 读 event payload
//   2. 查 identity exposure 状态（决定能否提名）
//   3. 拼上下文 (context: zone, identities, exposure_state, severity)
//   4. 调 throttle check (zone-level 5 min + dedupe)
//   5. 紧急事件 bypass_throttle: true
//   6. 拼 prompt 调 political skill
//   7. 推 narration 到 bong:agent_narration
```

### 测试

- [ ] `political_skill_prompt_loads_correctly`（skill 文件可被 agent 加载）
- [ ] `political_runtime_handler_subscribes_to_5_redis_channels`
- [ ] `political_narration_uses_jianghu_voice_phrasing`（输出含"江湖" / "传" / "市井"等关键字）
- [ ] `political_narration_does_not_use_modern_terms`（输出不含黑名单词）

---

## §3 P1 — 5 个核心事件 consumer

### 触发条件 + context 拼接

| 事件 | 触发 | context 字段 | scope |
|---|---|---|---|
| **feud 升级** | `SocialRelationshipEvent` kind=Feud + level 跨阈值（如 0→1, 1→2 仇深, 2→3 死仇）| `{ initiator_identity, target_identity, feud_level, exposed: bool, zone }` | zone |
| **pact 缔结** | `SocialPactEvent` 新缔结 | `{ left_identity, right_identity, pact_kind, exposed: bool, zone }` | zone |
| **灵龛抄家** | `bong:niche_destroyed` event（待 niche-defense-v1 P3 emit）| `{ owner_identity, attacker_identity, exposed: bool, zone }` | zone |
| **通缉令** | `bong:wanted_player` event（identity-v1 P5 emit，仅 Wanted 档触发）| `{ wanted_identity, primary_tag, reputation_score, exposed: true（通缉默认全暴露）}` | broadcast |
| **高 Renown 出名** | `bong:high_renown_milestone`（server emit when fame 跨 100/500/1000 阈值）| `{ identity_display_name, fame, milestone, exposed: bool, zone }` | broadcast（仅 1000 档）/ zone（100/500 档） |

### `HighRenownMilestoneTracker` Resource（server-side）

```rust
// server/src/social/high_renown_tracker.rs（新文件）
pub struct HighRenownMilestoneTracker {
    pub already_emitted: HashMap<(Uuid, IdentityId, u32), ()>,  // key: (player_uuid, identity_id, milestone_value)
}

const MILESTONE_THRESHOLDS: &[i32] = &[100, 500, 1000];

pub fn emit_high_renown_milestone_system(
    mut tracker: ResMut<HighRenownMilestoneTracker>,
    players: Query<(&PlayerIdentities, &Username)>,
    mut redis_outbox: ResMut<RedisOutbox>,
) {
    for (identities, username) in players.iter() {
        let active = identities.active();
        let fame = active.renown.fame;
        for &threshold in MILESTONE_THRESHOLDS {
            if fame >= threshold {
                let key = (player_uuid, active.id, threshold as u32);
                if tracker.already_emitted.insert(key, ()).is_none() {
                    redis_outbox.push("bong:high_renown_milestone", payload);
                }
            }
        }
    }
}
```

### Identity 暴露状态查询

- [ ] 在 server 侧查 `SocialExposureEvent` 历史（plan-social-v1 已有 `ExposureLog` Component）
- [ ] context 拼接时填 `exposed: bool`，让 agent prompt 决定是否能提名

### 测试

- [ ] `feud_escalation_event_triggers_political_narration`
- [ ] `pact_event_triggers_political_narration`
- [ ] `niche_destroyed_dummy_event_triggers_political_narration`（dummy 测试，等 niche-defense-v1）
- [ ] `wanted_player_event_triggers_political_narration_broadcast_scope`
- [ ] `high_renown_milestone_emits_only_once_per_threshold`
- [ ] `narration_context_includes_exposed_field_correctly`
- [ ] `unexposed_identity_narration_uses_anonymous_phrase`（"某修士"而非真名）
- [ ] `exposed_identity_narration_can_use_display_name`

---

## §4 P2 — 频率控制

### `PoliticalNarrationThrottleStore` Resource

```rust
// agent/packages/tiandao/src/political-narration.ts (Map state)
class PoliticalNarrationThrottleStore {
    last_narration_by_zone: Map<string, number>;  // zone -> last tick
    
    canEmit(zone: string, currentTick: number, bypass: boolean): boolean {
        if (bypass) return true;
        const last = this.last_narration_by_zone.get(zone) ?? 0;
        const elapsed_real_ms = (currentTick - last) * TICK_TO_MS;
        return elapsed_real_ms >= POLITICAL_THROTTLE_MS;  // 5 实时分钟
    }
    
    record(zone: string, currentTick: number): void {
        this.last_narration_by_zone.set(zone, currentTick);
    }
}
```

- [ ] `POLITICAL_THROTTLE_MS = 5 * 60 * 1000` (5 实时分钟)
- [ ] **bypass_throttle 适用**：
  - 通缉令（broadcast 级永远 bypass）
  - 化虚级人际（如 fame > 1000 milestone）
  - 灵龛抄家（玩家个人重大事件）
- [ ] **不 bypass**：
  - feud 升级（普通仇深）
  - pact 缔结
  - fame 100 / 500 milestone

### 复用 NarrationDedupeResource

- [ ] 进入 dedupe key：`scope|target|style|text` 复用既有 plan-narrative-v1 dedupe 逻辑
- [ ] political style 标注：所有 political narration `style: "political_jianghu"`

### 测试

- [ ] `throttle_blocks_second_feud_in_same_zone_within_5_min`
- [ ] `throttle_does_not_block_wanted_player_event`（bypass）
- [ ] `throttle_does_not_block_niche_destroyed_event`（bypass）
- [ ] `throttle_resets_after_5_min_passes`
- [ ] `narration_dedupe_with_same_text_within_window_blocks`（复用 narrative-v1 链路）
- [ ] `bypass_event_still_runs_through_dedupe_check`（bypass throttle 不 bypass dedupe）

---

## §5 P3 — `narration-eval` 评估扩展

### 新增评估维度（`agent/packages/tiandao/src/narration-eval.ts`）

```typescript
// 1. 江湖传闻味检查
const JIANGHU_VOICE_RE = /(江湖|传|道是|市井|山中|闻者|相传|有人道|外有声|画影|消息|传至|流传|耳闻)/;

// 2. 匿名约束违反检查
function checkAnonymityViolation(
    narration: string,
    exposedIdentities: Set<string>
): boolean {
    // 扫描 narration 中所有"display_name 模式" → 不在 exposedIdentities 集合则违反
    // 例：narration 含 "玄锋" 但 exposed 集合无 "玄锋" → 违反
    // 实施：从 context 拼接时给 evalAgent 传 `unexposed_names` 列表，检查是否被 narration 命中
}

// 3. 现代政治词黑名单
const MODERN_POLITICAL_TERMS_BLACKLIST = /(政府|党派|选举|投票|民主|议会|总统|主席|内阁|联邦|国家|政权)/;

// political style 评分函数
function scorePoliticalNarration(narration: string, context: PoliticalContext): NarrationScore {
    const baseScore = scoreNarration(narration);  // 既有评分（半文言/古意/长度等）
    
    let politicalScore = baseScore.score;
    if (!JIANGHU_VOICE_RE.test(narration)) politicalScore -= 30;  // 必须有江湖味
    if (MODERN_POLITICAL_TERMS_BLACKLIST.test(narration)) politicalScore -= 50;  // 现代政治词重罚
    if (checkAnonymityViolation(narration, context.exposedIdentities)) politicalScore -= 60;  // 匿名违反最重
    
    return { ...baseScore, score: politicalScore };
}
```

### 测试

- [ ] `political_narration_with_jianghu_voice_scores_high`
- [ ] `political_narration_without_jianghu_voice_loses_30_points`
- [ ] `political_narration_with_modern_term_loses_50_points`
- [ ] `political_narration_naming_unexposed_identity_loses_60_points`
- [ ] `political_narration_naming_exposed_identity_passes`

---

## §6 P4 — client 表现微调 + HighRenownMilestoneTracker

### HUD channel 分流（沿用 plan-HUD-v1 双通道）

- [ ] political narration 按 scope 分流：
  - `scope: broadcast` → ChatHud 即时显示（全服可见）
  - `scope: zone` → EventStore 仅记录，玩家通过 inspect / 历史查阅
- [ ] **不新增 HUD 元素**——复用既有 plan-HUD-v1 通道

### `HighRenownMilestoneTracker` server-side（P1 设计已落，本阶段仅做 schema + persistence）

- [ ] schema `HighRenownMilestoneEventV1`（TypeBox in `agent/packages/schema/src/social.ts` 既有文件加段）
- [ ] persistence：tracker.already_emitted 落 SQLite（避免 server 重启重发）

### 测试

- [ ] `urgent_political_narration_routes_to_chathud`
- [ ] `non_urgent_political_narration_routes_to_eventstore`
- [ ] `high_renown_milestone_does_not_re_emit_after_server_restart`（persistence）

---

## §7 数据契约（下游 grep 抓手）

| 契约 | 位置 |
|---|---|
| `political.md` skill prompt | `agent/packages/tiandao/src/skills/political.md`（新文件） |
| `political-narration.ts` runtime handler | `agent/packages/tiandao/src/political-narration.ts`（新文件） |
| `PoliticalNarrationThrottleStore` (TypeScript) | `agent/packages/tiandao/src/political-narration.ts` |
| `POLITICAL_THROTTLE_MS = 5 * 60 * 1000` | `agent/packages/tiandao/src/political-narration.ts` |
| `JIANGHU_VOICE_RE` | `agent/packages/tiandao/src/narration-eval.ts` |
| `MODERN_POLITICAL_TERMS_BLACKLIST` | `agent/packages/tiandao/src/narration-eval.ts` |
| `checkAnonymityViolation()` | `agent/packages/tiandao/src/narration-eval.ts` |
| `scorePoliticalNarration()` | `agent/packages/tiandao/src/narration-eval.ts` |
| `HighRenownMilestoneTracker` Resource | `server/src/social/high_renown_tracker.rs`（新文件） |
| `emit_high_renown_milestone_system` | `server/src/social/high_renown_tracker.rs` |
| `MILESTONE_THRESHOLDS = [100, 500, 1000]` | `server/src/social/high_renown_tracker.rs` |
| `bong:high_renown_milestone` Redis pub | `agent/packages/schema/src/social.ts`（新 schema） + `server/src/redis_outbox.rs` |
| `HighRenownMilestoneEventV1` TypeBox schema | `agent/packages/schema/src/social.ts` |
| 5 事件 consumer（feud / pact / niche / wanted / milestone）| `agent/packages/tiandao/src/political-narration.ts` |
| political style narration 标注 `"political_jianghu"` | 全 narration 输出统一 style |

---

## §8 决议（立项时已闭环 10 项）

调研锚点：worldview §十一 (commit fe00532c) + §八 天道叙事语调 + §K 红线第 12 条 + O.7 决策 + plan-narrative-v1 ✅ (NarrationDedupeResource + narration-eval) + plan-social-v1 ✅ (Renown / Relationship / Pact / Mentorship / Exposure 全实装) + plan-identity-v1 (active 2026-05-04，P5 wanted_player emit) + plan-niche-defense-v1 (active ⏳，niche_destroyed 待 emit) + tiandao 现有 11 skill (calamity / mutation / era / 流派 / dugu / 等) 视角分析。

| # | 问题 | 决议 | 落地点 |
|---|------|------|--------|
| **Q1** | 实施模式 | ✅ A：tiandao 新 skill `political.md`（一个 skill 文件）+ `political-narration.ts` runtime handler 薄层 | §2 全节 |
| **Q2** | 触发事件清单 | ✅ A 核心 5：feud 升级 / pact 缔结 / 灵龛抄家 / 通缉令 / 高 Renown 出名；mentorship / 派系战 / 师徒决裂 / 婚约留 vN+1 | §3 触发表 |
| **Q3** | 频率控制 | ✅ B+C：复用 NarrationDedupeResource + zone-level throttle 5 实时分钟 + 紧急事件 bypass_throttle | §4 全节 |
| **Q4** | scope 默认范围 | ✅ C：feud/pact → `scope: zone`；通缉令 / 化虚级人际 → `scope: broadcast`（worldview §K 红线"仅渡虚劫级用 broadcast" 唯一例外） | §3 触发表 + §6 HUD 分流 |
| **Q5** | 匿名约束严格度 | ✅ A：仅已 ExposureEvent 暴露的 identity 才在 narration 中提 display_name；未暴露用"某修士" / "一散修" / "戴铜面者" | §2 prompt + §5 ANONYMITY_VIOLATION_CHECK |
| **Q6** | agent prompt 视角 | ✅ A：tiandao 转述江湖（"江湖有传..." / "山中有人道..."），仍是天道视角但转述；不直白宣告 | §2 prompt 关键段 |
| **Q7** | HUD channel | ✅ C 按事件分流：紧急（broadcast 级）→ ChatHud；普通（zone 级）→ EventStore | §6 HUD 分流 |
| **Q8** | 与 identity-v1 wanted_player 关系 | ✅ A：本 plan 消费 `bong:wanted_player` → 生成"通缉令" narration（江湖传闻形式）；通缉令本质是江湖传闻，归 political | §3 触发表 |
| **Q9** | 是否影响 NPC AI 反应 | ✅ A：不影响——纯叙事层。NPC 反应已由 identity-v1 P3 处理，职责分离 | §0 设计轴心 |
| **Q10** | 评估维度 | ✅ A+B：复用既有 narration-eval + 加 JIANGHU_VOICE_RE / ANONYMITY_VIOLATION_CHECK / MODERN_POLITICAL_TERMS_BLACKLIST | §5 全节 |

> **本 plan 无未拍开放问题**——P0 可立刻起。P3 等 niche-defense-v1 emit `bong:niche_destroyed` + identity-v1 P5 emit `bong:wanted_player`，可用 dummy event 先测。

---

## §9 进度日志

- **2026-05-04 立项**：骨架立项。来源：journey-v1 §G "agent: era_decree 全服可见的'政治叙事'已支持；缺：基于 social graph 的政治 narration（派生 plan-narrative-political-v1）"。**关键发现**：tiandao 11 skill 没有 social-graph 政治 skill；narrative-v1 ✅ 已提供基础设施（NarrationDedupeResource / narration-eval）；social-v1 ✅ 提供完整事件源（feud / pact / mentorship / Exposure）；identity-v1（刚立 active）将 emit wanted_player；niche-defense-v1（active ⏳ ~5%）将 emit niche_destroyed。本 plan 是**江湖传闻型叙事**，与 era_decree（天道宣告时代）/ calamity（因果劫罚）/ mutation（环境变化）形成第 4 个叙事维度——**人际层面**。10 决议（Q1-Q10）一次性闭环。
