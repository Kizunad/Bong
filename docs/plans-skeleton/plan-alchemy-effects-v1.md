# Bong · plan-alchemy-effects-v1 · 骨架

**炼丹副效系统**：将 plan-alchemy-v1 server 侧三类遗留项落地——① `side_effect_pool` tag 接入真实 StatusEffect；② 丹心识别（玩家逆向推断配方，worldview §九"情报换命"钩子）；③ AutoProfile 傀儡绑炉自动化炼丹。附带 v2+ 方向：品阶 / 铭文 / 开光。

**世界观锚点**：
- `worldview.md §九`（情报换命——末法下配方是极高价值的信息不对称来源）
- `worldview.md §六`（真元染色 × 炼丹质量互动；缜密色修士丹心识别精度更高）
- `worldview.md §七`（傀儡系统联动；炉傀儡自动重放操作曲线）
- `worldview.md §十一`（修士之间默认敌对——丹方是最不愿泄露的秘密之一）

**library 锚点**：`ecology-0002 末法药材十七种`（已落，丹药材料基础参照）· 待写 `alchemy-XXXX 丹心感应术残章`（丹心识别物理依据：丹药残留灵气波纹 → 逆向读取配方信息）

**交叉引用**：
- `plan-alchemy-v1`（前置，finished；`recipe.rs` / `outcome.rs` / `FlawedFallback` 基础）
- `plan-alchemy-client-v1`（skeleton；正式配方名称正典化 §7 P5）
- `plan-combat-no_ui`（finished；StatusEffect 框架参照；若已有统一枚举则直接接入）
- `plan-cultivation-v1`（finished；境界影响丹心识别精度；`Contamination` / `QiCap` 接口）
- `plan-puppet-v1`（待立项；AutoProfile P2 依赖傀儡系统 P0）

**阶段总览**：
- P0 ⬜ side_effect_pool tag → StatusEffect 接入（真实游戏效果上身）
- P1 ⬜ 丹心识别（逆向配方推断，境界精度曲线）
- P2 ⬜ AutoProfile 傀儡绑炉（依赖 plan-puppet-v1 P0）
- P3 ⬜（v2+）品阶 / 铭文 / 开光（可拆为独立 plan）

---

## §0 设计轴心

- [ ] **副作用真正上身**：当前 `side_effect_pool` tag 仅字符串，`outcome.rs` 仅用 `is_beneficial_side_effect` 做权重缩放（行 103–108），炼丹博弈毫无实质感——P0 目标是让每个 tag 对应真实 StatusEffect
- [ ] **丹心识别 = 情报换命**：识别配方是末法下最高价值的信息之一，境界越高精度越高，但永远有信息衰减——不能直接给完整配方
- [ ] **AutoProfile 傀儡化**：重放操作曲线品质略低于手动，降低"在线时间 = 回报"的不公平性，符合末法散修生存逻辑

---

## §1 side_effect_pool → StatusEffect（P0）

**现状**：
- `server/src/alchemy/recipe.rs` `SideEffect { tag: String, probability, weight }` 存在（行 104–119）
- `server/src/alchemy/outcome.rs` 中 `is_beneficial_side_effect` 仅用于权重缩放（行 103–108）
- 实际游戏效果：无

**实装**：

```rust
// server/src/alchemy/side_effect_map.rs（新文件）
pub fn tag_to_status_effect(tag: &str) -> Option<StatusEffectApplication> {
    match tag {
        "minor_qi_regen_boost"   => Some(StatusEffectApplication::Temporary {
            kind: StatusEffectKind::QiRegen { pct_bonus: 0.05 },
            duration_ticks: 200,
        }),
        "rare_insight_flash"     => Some(StatusEffectApplication::Temporary {
            kind: StatusEffectKind::InsightFlash,
            duration_ticks: 100,  // 短暂灵感爆发，加速下次突破进度
        }),
        "qi_cap_perm_minus_1"    => Some(StatusEffectApplication::Permanent {
            kind: StatusEffectKind::QiCapDelta { delta: -1 },
        }),
        "minor_contamination"    => Some(StatusEffectApplication::Permanent {
            kind: StatusEffectKind::ContaminationDelta { delta: 0.05 },
        }),
        _ => None,  // 未知 tag → warn log + skip，不崩溃
    }
}
```

- 炼丹结算（`outcome.rs` `build_result`）生成 `AlchemyOutcome` 后，调用 `apply_side_effects_to_player`：
  - 遍历 `outcome.side_effects`，每条调用 `tag_to_status_effect`，Some 则应用到玩家实体
  - Temporary：attach `StatusEffectTimer` component；Permanent：直接修改对应字段（`QiCap` / `Contamination`）
- `StatusEffectKind` 若尚无统一枚举，P0 先在 `server/src/cultivation/status_effect.rs` 定义 alchemy 所需最小子集（4 种），接口设计与 plan-combat 可合并
- `server/assets/alchemy/recipes/*.json` 中现有 `side_effect_pool` tag 必须全部在映射表中有对应（或明确标 `_unmapped` 等待后续扩展）

**可核验交付物**：
- `server/src/alchemy/side_effect_map.rs` + `tag_to_status_effect` 映射函数
- `server/src/cultivation/status_effect.rs` `StatusEffectKind`（最小子集）
- `alchemy::side_effect_apply_*` 单测（至少 8 条）：
  - `minor_qi_regen_boost` → `QiRegen` temporary effect 上身 + duration 正确
  - `qi_cap_perm_minus_1` → `QiCap` 永久 −1（不被 remove 系统清掉）
  - `rare_insight_flash` → `InsightFlash` temporary 上身
  - `minor_contamination` → `Contamination` delta 正确叠加
  - unknown tag → warn + skip，其他副作用不受影响
  - `is_beneficial_side_effect` 权重缩放仍正常（回归）
  - 炼丹结算 e2e：配方命中 → `SideEffect` 应用到玩家 StatusEffect 列表

---

## §2 丹心识别（P1）

**世界观依据**：worldview §九"情报换命"——末法下配方价值极高；丹心感应可读取丹药残留灵气波纹，但信息有噪点，境界越高越清晰。

**触发流程**：

```
玩家右键丹药 → 选"鉴定"（消耗真元 5，CD 60s）
→ 发 DanxinInspectRequest { instance_id } C2S
→ server 校验：玩家境界 ≥ 凝脉期（否则返回 "境界不足，无法感应残留波纹"）
→ server 生成 DanxinInspectResult { hints: Vec<MaterialHint> }
→ 结果写入 BiographyEntry::AlchemyInspected
→ 推回客户端展示（owo-lib 浮窗）
```

**精度曲线**（按境界）：

| 境界 | 线索数 | 质量 |
|---|---|---|
| 醒灵 / 引气 | 不可鉴定 | — |
| 凝脉 | 1 条 | 乱序（材料分类标签，如"草本·凉性"，不含量） |
| 固元 | 2 条 | 有序 + 量范围（"草本·凉性，约 1–3 份"）|
| 通灵 | 3 条 | 含 item 分类名（如"凝灵草系·凉性·微量"）|
| 化虚 | 4 条 | 精确 item_id + 量（近乎完整配方，仍有 ±1 份误差）|

- 完整配方只能通过多次实验积累（每次鉴定结果可存为"推断笔记"，玩家自行归纳）或购买正式丹方
- 每次鉴定结果写 `BiographyEntry::AlchemyInspected { dan_template, hint_count, tick }`（公开，天道不保密）

**可核验交付物**：
- `DanxinInspectRequest` / `DanxinInspectResult` schema（`agent/packages/schema/src/alchemy.ts` 扩展）
- `server/src/alchemy/danxin.rs` 识别逻辑 + 精度函数
- C2S handler + server 校验（境界 / 物品存在 / CD）
- 单测（至少 8 条）：
  - 凝脉→1条乱序 / 化虚→4条精确（精度曲线 fixture）
  - 境界不足（引气期）→ 拒绝 + 错误提示
  - 物品不存在于背包 → 拒绝
  - CD 未到期 → 拒绝
  - 鉴定成功 → 写 `BiographyEntry::AlchemyInspected`
  - 真元消耗正确（5 点）

---

## §3 AutoProfile 傀儡绑炉（P2）

**前置**：plan-puppet-v1（傀儡系统，待立项）P0 完成后才可接入。

**设计**：

```
玩家手动完成炼丹（success 或 flawed_success）时，可选"录制本次操作"
→ server 存 AlchemyProfile {
    recipe_id: String,
    steps: Vec<AlchemyStep { op: StepOp, qi_amount: f32, timing_offset_ticks: i32 }>,
    recorded_at: Tick,
    recorded_by: CharId,
  }
→ 玩家绑定傀儡到炉（`BindFurnacePuppetRequest`）后，FurnacePuppetSystem 在玩家离线时重放
→ 品质 = 手动品质基线 × 0.90（−10%，模拟时序抖动）；失败率 = 原配方失败率 + 5%
→ 一个傀儡只绑一份 AlchemyProfile；更换 profile 需重新录制（旧 profile 覆盖）
```

**可核验交付物**：
- `AlchemyProfile` struct（`server/src/alchemy/profile.rs`，新文件）
- `FurnacePuppetSystem` stub（等 plan-puppet-v1 P0 接口确定后填充）
- profile 录制 → 存储 → 重放 round-trip 单测（品质 ×0.9 / 失败率 +5% / steps 序列化）

---

## §4 品阶 / 铭文 / 开光（P3+，v2+）

> plan-alchemy-v1 §7 TODO，全部 v2+，可按需拆为独立 plan：

- **品阶**：丹药品阶（凡 / 下品灵 / 上品灵 / 极品）影响效果幅度和市场价值；品阶由炼丹火候精准度 + 材料灵气品质共同决定
- **铭文**：高境界炼丹师在关键时机刻入铭文，提升效果幅度（×1.2 ~ ×2.0）；铭文次数随境界解锁
- **开光**：炉具用特殊材料开光后提升基础品质（依赖 plan-forge-v1 炉具品阶系统）

---

## §5 数据契约

| 契约 | 位置 |
|---|---|
| `tag_to_status_effect` 映射函数 | `server/src/alchemy/side_effect_map.rs`（新文件）|
| `StatusEffectKind`（alchemy 子集；最终与 plan-combat 合并）| `server/src/cultivation/status_effect.rs`（新或已有）|
| `DanxinInspectRequest` / `DanxinInspectResult` | `agent/packages/schema/src/alchemy.ts`（扩展）|
| `BiographyEntry::AlchemyInspected` | `server/src/cultivation/life_record.rs`（新变体）|
| `AlchemyProfile` struct | `server/src/alchemy/profile.rs`（新文件）|
| `FurnacePuppetSystem` stub | `server/src/alchemy/puppet.rs`（新文件，等 plan-puppet-v1）|

---

## §6 实施节点

- [ ] **P0**：`side_effect_map.rs` tag→StatusEffectKind 映射 + `status_effect.rs` 最小枚举定义 + 炼丹结算接入 + 单测（≥8）
- [ ] **P1**：`danxin.rs` 识别逻辑 + C2S schema + 境界精度曲线 + BiographyEntry 新变体 + 客户端浮窗（依赖 plan-alchemy-client-v1 UI 接口）+ 单测（≥8）
- [ ] **P2**：`AlchemyProfile` 录制 + `FurnacePuppetSystem` stub + round-trip 单测（需 plan-puppet-v1 P0）
- [ ] **P3**：品阶 / 铭文 / 开光（v2+，另立 plan）

---

## §7 开放问题

- [ ] `StatusEffectKind` 是否与 plan-combat 的 StatusEffect 系统合并为同一枚举？（强烈建议合并，避免两套系统并行）
- [ ] `qi_cap_perm_minus_1` 是永久 debuff 还是可被"洗丹毒"解除？（建议：仅被专用 `ContaminationCleanse` 效果解，不随时间自愈）
- [ ] 丹心识别的线索展示方式：模糊材料名 vs 材料分类 + 颜色提示 vs 纯文字描述？（建议：纯文字，贴合末法手记风格）
- [ ] 测试 JSON 配方（`kai_mai` / `hui_yuan` / `du_ming`）正典化已转入 `plan-alchemy-client-v1 §7 P5`，本 plan 不重复做
- [ ] `FlawedFallback`（丹方残卷损坏）已在 `recipe.rs` 实装，不在本 plan 范围

---

## §8 进度日志

- 2026-04-28：骨架立项。来源：`docs/plans-skeleton/reminder.md` plan-alchemy-v1 节（side_effect_pool + 丹心识别 + AutoProfile + 品阶，共 4 条）。代码核查：`FlawedFallback`（丹方残卷）已实装（`recipe.rs` 122–248）→ 从 reminder 删除；`side_effect_pool` 仅字符串（`outcome.rs` 103–108 未接真实效果）、丹心识别 / AutoProfile / 品阶均未实装。
