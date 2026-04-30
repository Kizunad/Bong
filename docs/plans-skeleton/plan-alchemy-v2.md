# Bong · plan-alchemy-v2 · 骨架

**炼丹进阶**——plan-alchemy-v1 §7 TODO 集中落地：side_effect_pool → StatusEffect 映射、丹方残卷损坏结构、品阶系统、铭文/开光、AutoProfile 自动化炼丹、丹心识别。plan-alchemy-v1 已归档，本 plan 消费其全部"v2+"标记项。

**世界观锚点**：`worldview.md §九`（情报换命——丹心识别钩子）· `ecology-0002`（末法药材十七种，配方材料来源）· `cultivation-0002 §三·论影`（烬灰子：丹药作用的物理基础）

**代码锚点**：
- `server/assets/alchemy/recipes/*.json`：三份测试占位（kai_mai / hui_yuan / du_ming），不进生产
- `server/src/alchemy/`：已有炼丹 resolver / furnace / heat_profile 等模块
- `side_effect_pool` 当前是纯字符串 tag，`StatusEffects` component 在 `server/src/combat/components.rs` 已存在但炼丹尚未对接

**交叉引用**：`plan-alchemy-v1`（已归档）· `plan-alchemy-client-v1`（骨架，客户端 UI）· `plan-shouyuan-v1`（续命丹配方正典化）· `plan-forge-v1`（品阶/铭文共用 Augment 结构？）

---

## §1 配方正典化（清理测试 JSON）

`server/assets/alchemy/recipes/` 三份示例为测试 fixture，须在 P1 前完成生产配方正典化：

- [ ] 命名规范：`<拼音缩写>_dan_<tier>.json`（如 `hui_qi_dan_1.json`）
- [ ] P0 基础配方集（5 种）：
  | 配方 | 效果 | 材料来源 |
  |---|---|---|
  | `hui_qi_dan`（回气丹）| 真元快速恢复 | ecology-0002 |
  | `lian_shang_wan`（敛伤丸）| 止血 + 愈合加速 | ecology-0002 |
  | `bi_du_wan`（辟毒丸）| 毒素抵抗 | ecology-0002 |
  | `qing_huo_san`（清火散）| 清除灼热 StatusEffect | ecology-0002 |
  | `xu_ming_dan`（续命丹）| 见 plan-shouyuan-v1 | ecology-0002 |
- [ ] 保留 kai_mai / hui_yuan / du_ming 作**回归测试 fixture**（不删，迁到 `server/assets/alchemy/fixtures/`）

---

## §2 StatusEffect 对接（side_effect_pool）

`side_effect_pool` tag 字符串目前未映射到实际 buff/debuff 系统：

**前提**：`StatusEffects` component + `StatusEffectKind` 枚举在 `server/src/combat/status.rs` 已存在（Bleeding/Stunned/Slowed/DamageAmp/QiCorrosion），炼丹系统需扩展：

- [ ] 在 `StatusEffectKind` 追加炼丹专属变体：
  - `MinorQiRegenBoost`：真元回复 +15%，持续 30 分钟
  - `RareInsightFlash`：下次打坐 breakthrough 概率 +10%，一次性
  - `QiCapPermMinus1`：真元上限永久 -1（永久 debuff，危险）
  - `NauseaMinor`：15 分钟内无法进食丹药（防叠加）
- [ ] `alchemy/resolver.rs` 的 side_effect 抽取逻辑：将 tag 字符串映射到 `StatusEffectKind` 并发 `ApplyStatusEffectIntent`
- [ ] 完整 tag→effect 映射表（`alchemy/side_effects.rs`）；未知 tag 记 warn 日志但不 panic
- [ ] 单测：每个 tag 各一条正反样本验证；未知 tag 降级不崩

---

## §3 丹方残卷损坏（DamagedRecipe）

plan-alchemy-v1 §1.4 提出"损坏残卷只能学到残缺版配方"，数据结构未定：

- [ ] `DamagedRecipe` 结构体（`server/src/alchemy/recipe.rs` 或新 `damaged.rs`）：
  ```rust
  pub struct DamagedRecipe {
      pub original_id: RecipeId,
      pub missing_slots: Vec<usize>,  // 缺失的 ingredient slot 索引
      pub degradation: f32,           // 0.0~1.0，损坏程度（影响残缺 fallback 概率）
  }
  ```
- [ ] Item template `damaged_recipe_scroll`：`ItemEffect::LearnDamagedRecipe(DamagedRecipe)`
- [ ] 学习逻辑：拖残卷到卷轴区 → 注入 `DamagedRecipe` 到 `LearnedRecipes`；残缺配方炼丹走 `flawed_path`，抽取 side_effect 概率提升（`degradation × 1.5`）
- [ ] UI 提示（损坏程度 bar）交 plan-alchemy-client-v1
- [ ] 单测：正常残卷学习 → `missing_slots` 正确存入；炼丹走 flawed 分支 → side_effect 抽取次数符合预期

---

## §4 品阶系统

MVP 只有 `quality: f32`，亡者博物馆/聊天无法展示品阶：

- [ ] `PillTier` 枚举（`server/src/alchemy/resolver.rs` 或 `tier.rs`）：
  ```rust
  pub enum PillTier { Low, Mid, High, Supreme }
  ```
  quality 阈值：< 0.60 → Low；0.60–0.80 → Mid；0.80–0.95 → High；≥ 0.95 → Supreme
- [ ] 效果系数：Low 0.7× / Mid 1.0× / High 1.3× / Supreme 1.6×（应用于所有 buff 幅度）
- [ ] `AlchemyResultV1` schema 追加 `tier: PillTier` 字段（agent + server 双端）
- [ ] 客户端品阶颜色标注交 plan-alchemy-client-v1
- [ ] 单测：quality 边界值各品阶覆盖

---

## §5 铭文 / 开光（高阶后处理）

炼成后追加专项效果，通灵境界以上才能激活：

- [ ] `Augment` 结构体（可复用 plan-forge-v1 的 `ItemAugment`？先独立，后统一）：
  ```rust
  pub struct AlchemyAugment { pub kind: AugmentKind, pub potency: f32 }
  ```
- [ ] 铭文动作：丹药炼成后消耗额外灵材 → 追加 `augments: Vec<AlchemyAugment>` 到 `ItemInstance`
- [ ] 开光动作：通灵+ 施法 → 强化铭文 potency；失败概率 = f(境界，铭文阶数)；失败 → 铭文消失
- [ ] `ItemInstance.augments` schema 扩展（server + agent）
- [ ] 单测：开光成功强化 potency；失败铭文归零；境界不足拒绝

---

## §6 AutoProfile 自动化炼丹

plan-alchemy-v1 §1.3 预留口 `AutoProfile(ProfileId)`，尚无 JSON 曲线库：

- [ ] 曲线文件格式（`server/assets/alchemy/profiles/<id>.json`）：
  ```json
  { "id": "standard_qi_pill", "name": "标准回气丹曲线",
    "steps": [{"phase": "heat_up", "target_temp": 120, "duration_s": 30}, ...] }
  ```
- [ ] `furnace_session.rs`（或 `heat_profile.rs`）读取 Profile JSON，自动推进热度阶段
- [ ] 傀儡绑炉：NPC 使用 AutoProfile 批量产丹（需 `NpcCombatant` + 固元境以上）
- [ ] 玩家解锁 Profile：购买/炼丹经验解锁；存档进 `PlayerPreferences.unlocked_profiles`
- [ ] 单测：Profile 加载 + 阶段自动推进 + 阶段超时处理

---

## §7 丹心识别（玩家逆向配方）

worldview §九"情报换命"钩子：高境界玩家可逆向未知丹药配方：

- [ ] 技能触发：检视未知丹药 → `AlchemyInspectIntent { item_instance_id }` 事件
- [ ] 识别系统（`alchemy/reverse.rs`）：
  - 成功率 = `sigmoid(境界权重 × log(炼丹经验 + 1) - 丹药复杂度)`
  - 成功 → 解锁 `DamagedRecipe`（missing_slots 随机保留 1-2 个），推进玩家 `LearnedRecipes`
  - 完全逆向（success × 2-3 次）→ 解锁完整 recipe
  - 失败 → 无信息（防穷举）
- [ ] 记录写入 `LifeRecord.alchemy_attempts`（`plan-alchemy-v1 §4` 数据契约，already implemented）
- [ ] 单测：成功率公式边界（境界 0/最大）；失败不泄露信息；多次成功解锁完整配方

---

## §8 实施节点

- [ ] **P0**：配方正典化 + fixture 迁移
- [ ] **P1**：StatusEffect 对接（side_effect_pool tag → effect 映射）
- [ ] **P2**：DamagedRecipe 结构 + damaged_recipe_scroll item
- [ ] **P3**：品阶系统 + schema 扩展
- [ ] **P4**：AutoProfile 曲线库（JSON 格式 + 炉控）
- [ ] **P5**：铭文 / 开光
- [ ] **P6**：丹心识别

---

## §9 开放问题

- [ ] StatusEffect 系统扩展是否需要单独 plan？（当前 plan-alchemy-v2 是消费方，直接在 `combat/status.rs` 追加变体）
- [ ] `Augment` 结构体与 plan-forge-v1 的强化槽是否统一？（复用 vs 分叉，影响 schema 设计）
- [ ] AutoProfile 是否开放玩家分享/交易（影响经济系统设计）？
- [ ] 丹心识别"情报换命"——配方解锁后可否转售给其他玩家？

---

## §10 进度日志

- 2026-04-29：骨架立项——覆盖 plan-alchemy-v1 §7 全部 TODO（side_effect 映射 / 残卷损坏 / 品阶 / 铭文 / AutoProfile / 丹心识别）+ 配方正典化。StatusEffect component 已存在于 combat 模块，炼丹对接是本 plan P1 核心。
