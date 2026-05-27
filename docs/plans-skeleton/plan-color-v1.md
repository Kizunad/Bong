# Bong · plan-color-v1 · 骨架

真元色养成闭环——把 PracticeLog 积累转化为实际战斗/修炼加速规则，实装杂色惩罚与混元色特效，让染色成为可感知的长期身体改造。

**世界观锚点**：`worldview.md` §六.1（流派←→染色：「流派反过来塑造染色」）· §六.2（染色规则：主色~10h / 杂色≥3项 / 混元色均匀<25%）· §五 染色谱表（ColorKind 与流派对应）

**依赖 plan（前置必须完成）**：
- `plan-style-vector-integration-v1` ✅ — PracticeLog.add() 接入 6 流派 P0
- `plan-cultivation-canonical-align-v1` ✅ — Realm 正典名 + XP 曲线基线

---

## 接入面 Checklist

- **进料**：`cultivation::color::PracticeLog` ✅ / `cultivation::components::QiColor` ✅ / `combat::resolve` + 各流派 combat module（anqi_v2 / woliu_v2 / dugu_v2 / zhenmai_v2 / tuike / zhenfa）
- **出料**：combat multipliers 注入各流派结算；`bong:agent_narrate` narration 触发色调里程碑；`QiColorStateV1` schema（已有）client HUD 展示
- **共享类型**：复用 `QiColor` / `PracticeLog` / `ColorKind`，新增 `fn color_style_bonus(qi_color, active_color) -> f32`（不新增 Component/Event，只加纯函数）
- **跨仓库契约**：server 纯函数无需 IPC；client 展示走现有 `QiColorStateV1` proto；agent 可通过 `world_state` 已有 qi_color_state 字段感知色调
- **worldview 锚点**：§六.1 「匹配的特性让流派事半功倍」 / §六.2 「杂色 = 所有专精效果失效」「混元色 = 博而不精的玩法」
- **qi_physics 锚点**：本 plan 不修改真元流动；色彩加速是效率系数，不改守恒律

---

## 阶段总览

| 阶段 | 内容 | 状态 | 验收 |
|------|------|------|------|
| **P0** | 色-派对应加速规则（`color_style_bonus`）+ 接入 cultivation session 与 combat XP 累积 | ⬜ | 单测：main 匹配 → 0.9x；secondary → 0.95x；chaotic → 1.1x；hunyuan → 1.0x |
| **P1** | 杂色惩罚实装：`is_chaotic=true` 时关闭各流派专项效果 | ⬜ | 单测：杂色玩家 combat bonus 归零；各流派 module guard 测试 |
| **P2** | 混元色特效：`is_hunyuan=true` 时解锁全 10 色顿悟选项 + 通用 -5% 成本 | ⬜ | 单测：混元色 insight 池覆盖所有 ColorKind |
| **P3** | 色调里程碑 narration（首次主色涌现 / 色调转换 / 杂色堕落 / 混元觉醒） | ⬜ | 手动测试：4 个里程碑各触发一条 narration，scope=player |
| **P4** | 神视观察机制：高境施展神识可远程感知他人真元色调 | ⬜ | 单测：固元+ 可感知范围内目标的 QiColor；范围外返回 None |
| **P5** | 饱和测试（色演化稳定性 + 死亡清零 + PracticeLog 跨 session 衰减） | ⬜ | 20 单测全绿 |

---

## §1 P0：色-派加速规则（核心交付）

### 纯函数设计

```rust
// server/src/cultivation/color_bonus.rs（新文件）
pub fn color_style_bonus(qi_color: &QiColor, active_color: ColorKind) -> f32 {
    if qi_color.is_chaotic { return 1.1; }    // 杂色惩罚：效率 -10%
    if qi_color.is_hunyuan  { return 1.0; }   // 混元：不加速也不减速
    if qi_color.main == active_color { return 0.9; }                         // 主色匹配：事半功倍 -10% 成本
    if qi_color.secondary == Some(active_color) { return 0.95; }             // 次色匹配：-5%
    1.0                                                                        // 不匹配：基线
}
```

注：multiplier 作用于 `PracticeLog.add()` 权重（匹配色练习积累更快）和 cultivation session 真元回复速率（同色打坐效率更高），**不进战斗伤害公式**（worldview §六.2「染色不参与战斗公式」）。

### 接入点

| 接入位置 | 改动 | 说明 |
|---|---|---|
| `cultivation::color::record_style_practice` | 乘 `color_style_bonus` | 战斗动作练习时按色匹配调整权重 |
| `cultivation::color::record_cultivation_session_practice` | 同上 | 打坐练习也按色调整 |
| `combat::anqi_v2.rs` line ≈463（已有 TODO 注释） | 读取 bonus 作为距离衰减修正 | 凝实色 + 暗器距离衰减 -5%（worldview §六.2 正典化） |

---

## §2 P1：杂色惩罚实装

当 `QiColor.is_chaotic = true`：
- 各流派 combat module 在应用专项 bonus 前 guard：`if qi_color.is_chaotic { return; }`
- `cultivation_session`：`record_cultivation_session_practice` 积累权重 ×1.1（效率降低）
- 范围：anqi_v2 / woliu_v2 / dugu_v2 / zhenmai_v2 / tuike / zhenfa / sword_path 各流派专项函数

**关键约束**：杂色不影响基础真元消耗，只让专项加成失效。worldview 定义为「所有专精效果失效，只剩基础真元属性」，不是扣分。

---

## §3 P2：混元色特效

当 `QiColor.is_hunyuan = true`：

1. `cultivation::color_affinity::select_aligned_tradeoffs` 中：当前已处理 `is_hunyuan` → `hunyuan_tradeoffs()`，**只需核验是否覆盖全 10 色 InsightChoice**（现有函数可能是占位）
2. 通用效率系数 0.95（-5% practice 成本）对所有色/流派生效，作为"博而不精"的平衡补偿
3. 混元色 **不可被 `permanent_lock_mask`** 锁定（不允许任何色永久污染，worldview §六.2 混元色为第十一种特殊形态）

---

## §4 P3：色调里程碑 narration

4 个关键节点各触发 1 条 narration，scope=player，style=perception：

| 触发条件 | 示例台词 | 触发位置 |
|---|---|---|
| 首次 `main != Mellow` 变化（第一个主色涌现） | "你的真元开始沉淀出一种倾向——尚不明朗，但已与从前不同。" | `qi_color_evolution_tick` → emit `QiColorShiftEvent` |
| `main` 发生变更（色调转换） | "旧日的沉淀在松动。你走向了另一条轨迹。" | 同上，secondary 被旧 main 替换时 |
| `is_chaotic` 变为 true（杂色堕落） | "你什么都练，什么都不精。真元在你体内像一锅乱炖。" | 同上 |
| `is_hunyuan` 变为 true（混元觉醒） | "五色均衡，无主无从。这不是退而求其次——这是另一种路。" | 同上，`is_hunyuan()` 首次满足 |

---

## §5 P4：神视观察机制（来自 worldview §六.2）

> "未来开放'神视观察'机制——高境修士或特殊功法可远距离感知他人真元色调，将染色变成 PVP 信息战的一部分。"

- `server/src/cultivation/perception.rs`（如已存在扩展，否则新建）：`fn remote_color_sense_range(realm: Realm) -> Option<u32>` — 固元=None；通灵=32 格；化虚=128 格
- server 每 `PlayerTick` 对范围内玩家发送 `QiColorObserved`（现有 proto）
- client：收到 `QiColorObserved` 后在 target 头顶渲染轻微色晕 aura（不标注具体颜色名称，只给色调提示）
- **不改 inspect 机制**（inspect 仍可任意距离，神视是被动感知补充）

---

## §6 开放问题（P0 决策门前收口）

1. **加速幅度**：0.9x / 0.95x 是否正确？与 worldview §六「事半功倍」和 §五 P.4 combat matrix 数值一致性需 Explore 核验
2. **杂色触发门槛**：当前 `color.rs` 阈值 `≥3项 > 15%`——是否应改为绝对权重而非相对比例？考虑低练习量的新玩家不应轻易进杂色
3. **混元色成本 0.95x**：是否会让混元成为"always better"选项破坏专精意义？需对比专精 0.9x vs 混元 0.95x（专精在本流派里还是更快）
4. **神视观察与 plan-spirit-eye-v1 的关系**：spirit-eye 是灵眼坐标系统，神视是真元色感知——两者是分开的感知频道，确认不重叠