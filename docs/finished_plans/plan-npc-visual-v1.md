# Plan: NPC Visual v1（NPC 视觉差异化）

> NPC 系统用 MineSkin API 随机拉皮肤，但**散修和凡人长一样、引气和化虚穿一样、魔修派和守正派颜色一样**。本 plan 给 NPC 按境界/派系/年龄/阶级做视觉分层，让玩家一眼分辨对方身份。

---

## 接入面 Checklist（防孤岛）

- **进料**：`npc::NpcArchetype` ✅ / `cultivation::Realm` ✅ / `npc::faction::FactionMembership` ✅ / `npc::lifecycle::NpcLifespan` ✅ / `skin::MineSkinClient` ✅ / `vfx::VfxRegistry` ✅
- **出料**：分层皮肤选择器 → `server/src/skin/npc_skin_selector.rs` / 境界 VFX → `VfxBootstrap` / 年龄渲染 → client renderer 增强
- **共享类型/event**：复用 `VfxEventRequest`，扩展 `NpcMetadataS2c`（plan-npc-engagement-v1 P0 新增）追加 `skin_tier` 字段
- **跨仓库契约**：server skin_selector 输出 skin UUID → client PlayerEntity skin 渲染
- **worldview 锚点**：§六 个性三层（外观反映修为）/ §五 流派（染色外观）/ §十一 "散修百态"

---

## 阶段总览

| 阶段 | 目标 | 状态 |
|------|------|------|
| P0 | 境界分层皮肤 + 派系颜色 | ✅ 2026-05-10 |
| P1 | 年龄外观 + 阶级标记 | ✅ 2026-05-10 |
| P2 | NPC 突破/死亡 VFX + 气质光环 | ✅ 2026-05-10 |

---

## P0 — 境界分层皮肤 + 派系颜色 ✅ 2026-05-10

### 交付物

1. **分层皮肤选择器**（`server/src/skin/npc_skin_selector.rs`）
   - 输入：`(archetype, realm, faction, age_ratio)` → 输出：`skin_pool_key`
   - 皮肤池分层：
     - 凡人(Commoner)：粗布麻衣系列（3-5 款）
     - 散修·低境界(Awaken-Induce)：灰袍/补丁袍系列（5-8 款）
     - 散修·中境界(Condense-Solidify)：素色长袍系列（5-8 款）
     - 散修·高境界(Spirit-Void)：华服/法袍系列（3-5 款）
   - MineSkin API 按 `skin_pool_key` 从预分类 skin collection 中选取

2. **派系颜色叠加**（`server/src/skin/faction_tint.rs`）
   - 不改皮肤本体，通过 equipment color 叠加派系标识：
     - 攻击派系：红色腰带（leather_chestplate dyed #CC2222）
     - 防御派系：蓝灰腰带（#4466AA）
     - 中立/散修：无腰带
   - 高阶(真传+)：钻石头盔作为冠饰（visible equipment slot）

3. **皮肤缓存预热**
   - 服务器启动时按 `(archetype, realm_tier, faction)` 组合预拉 skin pool（~60 款）
   - 缓存到 `server/cache/skins/` 避免运行时 MineSkin API 延迟
   - Fallback：缓存 miss 时用 realm_tier 对应的 hardcoded UUID

### 验收抓手

- 测试：`server::skin::tests::selector_returns_correct_pool` / `server::skin::tests::faction_tint_applies`
- 手动：spawn 5 个不同境界 NPC → 低境界灰袍 / 中境界素袍 / 高境界华服 → 攻击派系有红腰带

---

## P1 — 年龄外观 + 阶级标记 ✅ 2026-05-10

### 交付物

1. **年龄视觉**
   - `age_ratio < 0.3`（青年）：标准皮肤
   - `age_ratio 0.3-0.7`（壮年）：无变化
   - `age_ratio > 0.7`（老年）：头发白化效果（leather_helmet dyed #CCCCCC 模拟白发）+ 移动速度视觉减缓（client-side animation speed ×0.8）
   - `age_ratio > 0.9`（风烛）：弯腰姿态（client render pitch 微调 +5°）+ 步履蹒跚动画

2. **阶级标记**
   - 外门弟子：无特殊标记
   - 内门弟子：手持发光物品（灵石道具，client-side held item render）
   - 真传弟子：背后光环粒子（`BongSpriteParticle` `qi_aura` × 2，低频旋转）
   - 长老/掌门：全身淡金光环 + 境界压制粒子（走过的地面留 `BongGroundDecalParticle` 脚印 1s 消散）

3. **阶级粒子注册**
   - `VfxBootstrap` 新增 `npc_rank_aura_elder` / `npc_rank_aura_master` event ID
   - server：高阶 NPC hydrate 后每 100 tick emit rank aura VFX

### 验收抓手

- 测试：`server::skin::tests::age_ratio_selects_elder_variant` / `client::npc::tests::rank_aura_renders`
- 手动：spawn 年轻 vs 老年 NPC → 白发/弯腰区别 → spawn 长老 NPC → 金色光环 + 脚印

---

## P2 — NPC 突破/死亡 VFX + 气质光环 ✅ 2026-05-10

### 交付物

1. **NPC 突破 VFX**
   - NPC 成功突破时：复用 `BreakthroughPillarPlayer`（已存在）+ 全服 narration（已有 tiandao era 支持）
   - NPC 突破时 client 可见：光柱 + 境界名闪烁 toast + 名牌更新
   - server emit：`cultivation::breakthrough` NPC 路径追加 `VfxEventRequest::new("breakthrough_pillar", npc_pos)`

2. **NPC 死亡 VFX**
   - 复用 `DeathSoulDissipatePlayer`（已存在）
   - 追加：尸体位置残留淡灰烟雾 3s（`BongSpriteParticle` cloud256_dust × 4，缓慢上升）
   - 高境界 NPC 死亡：额外真元爆散光环（`BongLineParticle` 辐射线 × 8，颜色按 QiColor）

3. **气质光环**（高境界 NPC 常驻）
   - 固元+：脚下微弱灵气涟漪（`BongGroundDecalParticle` `lingqi_ripple` 贴图，每 60 tick 一次脉动）
   - 通灵+：身周空气扭曲（client-side shader distortion，依赖 plan-iris-integration-v1；无 Iris 时 fallback 为粒子光环）
   - 化虚：走过的路径 3s 内灵气浓度可视化波动

### 验收抓手

- 测试：`server::cultivation::tests::npc_breakthrough_emits_vfx` / `client::npc::tests::death_smoke_renders`
- 手动：观察 NPC 突破 → 光柱 + toast → 观察高境界 NPC → 脚下涟漪 → 杀死 NPC → 灰烟 + 真元爆散

---

## 前置依赖

| 依赖 plan | 状态 | 用到什么 |
|-----------|------|---------|
| plan-npc-ai-v1 | ✅ finished | archetype / 寿命 / Realm 共享 |
| plan-npc-skin-v1 | ✅ finished | MineSkin 集成 / skin cache |
| plan-npc-virtualize-v1 | ✅ finished | hydrate 后 VFX emit |
| plan-cultivation-v1 | ✅ finished | Realm enum / breakthrough flow |
| plan-vfx-v1 | ✅ finished | VfxRegistry / BreakthroughPillarPlayer / DeathSoulDissipatePlayer |
| plan-particle-system-v1 | ✅ finished | 全部粒子几何类 |
| plan-iris-integration-v1 | ⏳ active | 空气扭曲 shader（P2 fallback 不依赖） |

**P0/P1 无阻塞。P2 空气扭曲 shader 依赖 iris-integration（有 fallback）。**

## Finish Evidence

### 落地清单

- P0 境界分层皮肤 + 派系颜色：`server/src/skin/npc_skin_selector.rs` 新增 `NpcSkinTier` / `NpcAgeBand` / `NpcVisualProfile` / `NpcSkinPoolKey`；`server/src/skin/pool.rs` 改为按 `NpcSkinPoolKey` 分桶预热与抽取；`server/src/skin/faction_tint.rs` 通过 `Equipment` 叠加攻击/防御派系颜色。
- P1 年龄外观 + 阶级标记：`server/src/skin/faction_tint.rs` 为老年 NPC 叠加白发头部标记，为弟子/领袖叠加手持物、冠饰和 rank aura；`server/src/npc/spawn.rs` 在散修、凡人、弟子 spawn 时挂载 `NpcVisualProfile` 与视觉装备。
- P2 NPC 突破/死亡 VFX + 气质光环：`server/src/combat/lifecycle.rs` 在 NPC 死亡路径发 `bong:npc_death_smoke`，高境界 NPC 额外发 `bong:npc_death_qi_burst`；`server/src/skin/faction_tint.rs` 定期发 `bong:npc_rank_aura_elder` / `bong:npc_rank_aura_master` / `bong:npc_qi_aura_ripple`；client 新增 `NpcDeathSmokePlayer`、`NpcDeathQiBurstPlayer`、`NpcRankAuraPlayer`、`NpcQiAuraRipplePlayer` 并在 `VfxBootstrap` 注册。

### 关键 commit

- `e471a09f7`（2026-05-10）`plan-npc-visual-v1：NPC 视觉分层与光环 VFX`
- `7a7ba594f`（2026-05-10）`plan-npc-visual-v1：补齐高境界死亡真元爆散`

### 测试结果

- `cd server && cargo fmt --check && CARGO_BUILD_JOBS=1 CARGO_PROFILE_TEST_DEBUG=0 cargo clippy --all-targets -- -D warnings && CARGO_BUILD_JOBS=1 CARGO_PROFILE_TEST_DEBUG=0 cargo test` → 3642 passed；`CARGO_PROFILE_TEST_DEBUG=0` 仅用于降低本机并发链接内存占用。
- `cd server && CARGO_BUILD_JOBS=1 CARGO_PROFILE_TEST_DEBUG=0 cargo test skin:: -- --nocapture` → 12 passed。
- `export JAVA_HOME=$HOME/.sdkman/candidates/java/17.0.18-amzn; export PATH=$JAVA_HOME/bin:$PATH; cd client && ./gradlew test build` → BUILD SUCCESSFUL；test XML 统计 1012 tests / 0 failures / 0 errors / 0 skipped。

### 跨仓库核验

- server：`select_npc_visual_profile`、`NpcVisualProfile`、`visual_equipment`、`npc_death_smoke_request`、`npc_death_qi_burst_request`、`bong:npc_rank_aura_master`、`bong:npc_qi_aura_ripple`。
- client：`NpcDeathSmokePlayer.EVENT_ID`、`NpcDeathQiBurstPlayer.EVENT_ID`、`NpcRankAuraPlayer.ELDER`、`NpcRankAuraPlayer.MASTER`、`NpcQiAuraRipplePlayer.EVENT_ID`。
- 共享 event：复用 `VfxEventRequest` / `VfxEventPayloadV1::SpawnParticle`，新增 event id 通过 `VfxRegistryTest` 锁定 bootstrap 注册。

### 遗留 / 后续

- `plan-iris-integration-v1` 仍是 active；本 plan 对通灵+空气扭曲按原计划走 `npc_qi_aura_ripple` 粒子 fallback，不新增 shader 依赖。
