# Bong · plan-yidao-v2 · 骨架

医道功法 **亚流派扩展包**——在 plan-yidao-v1（5 招完整医道 + 支援流派身份）基础上，选择一条亚流派方向深化。三个候选方向（P0 决策门选一）：

- **毒手医**（Toxic Medic）：借鉴毒蛊脏真元语言，用微量毒素治疗顽症 / 异蛊感染。区别于纯毒蛊师——毒手医"以毒攻毒"，目的是修复而非破坏。须与 plan-dugu-v2 接口对齐（脏真元 ρ=0.05 / 永久阈值 / 暴露概率）
- **兽医**（Beast Medic）：妖兽 / 灵兽的医道治疗专精。接驳 plan-fauna-v1 ✅ 妖兽材料 + plan-npc-ai-v1 ✅ 妖兽 NPC 行为。化虚兽医可与顶阶妖兽结契，获得不可被其他流派替代的派系信任
- **道伥医**（Daochang Medic）：研究道伥（worldview §七:727 被天劫劈死高手遗骸）的诊断 / 改造 / 控制路径。**注意**：道伥医是"读懂 / 操控遗骸战斗本能"，不是"治好道伥"——worldview §七 道伥没有意识可治。方向是信息战：读取道伥残存战斗记忆以学习失传招式片段（走 plan-tsy-loot-v1 残卷捡取的叙事替代路径）

P0 决策门选定方向后，本 plan 仅实装**一条亚流派**；其他两条留候补 vN+1。

**交叉引用**：`plan-yidao-v1.md` ✅（5 招医道底盘 + 平和色 + 医患信誉度）· `plan-dugu-v2.md` 🆕（毒手医方向依赖脏真元接口）· `plan-fauna-v1.md` ✅（兽医方向妖兽类型）· `plan-npc-ai-v1.md` ✅（妖兽 AI / 道伥 AI 行为节点）· `plan-tsy-loot-v1.md` ✅（道伥医方向残卷学习接口）· `plan-meridian-severed-v1.md` 🆕（接经术目标，v2 可能扩展至妖兽经脉）

**worldview 锚点**：

- **§六:617 医道平和色**：亚流派扩展不改变平和色根基——毒手医 / 兽医 / 道伥医都是平和色医道的专精分化，不引入新色谱
- **§七:727 道伥**：道伥无意识 / 无法治疗，道伥医是"解读遗骸战斗本能"的特殊信息获取路径，不是"感化道伥"
- **§五:537 流派由组合涌现**：亚流派是医道 + 毒蛊 / 医道 + 妖兽驯化 / 医道 + 遗迹探索的交叉组合，自然涌现而非人工设计
- **§十一 灵龛守护**：兽医与顶阶妖兽结契走灵龛见证仪式（plan-social-v1 复用），是普通玩家无法替代的长期关系

**qi_physics 锚点**：

- 毒手医：脏真元治疗走 `qi_physics::contamination::targeted_purge` 🆕（plan-qi-physics-patch-v1 P3 毒蛊算子扩展，需确认是否存在或需派生）
- 兽医：妖兽 qi_max / 经脉结构差异走妖兽专属容器类型（`ContainerType::Beast`，需 qi_physics::excretion 扩展 or 直接走通用路径）
- 道伥医：道伥遗骸无活跃灵气，不走 qi_physics 守恒路径；走信息提取（残卷类），qi_physics 不涉及

**前置依赖**：

- `plan-yidao-v1` ✅ → 5 招医道底盘 + 平和色 + KarmaCounter + HealerProfile + 医患信誉度系统
- `plan-meridian-severed-v1` 🆕 → 接经术目标（兽医方向需扩展至妖兽经脉系统）
- `plan-qi-physics-patch-v1` P3 ✅（ρ 矩阵 + 毒蛊算子，毒手医方向需要）
- `plan-dugu-v2` 🆕（毒手医方向：脏真元阈值 / 暴露概率接口对齐）
- `plan-fauna-v1` ✅（兽医方向：妖兽类型 / NpcArchetype::Beast 结构）
- `plan-tsy-loot-v1` ✅（道伥医方向：残卷学习接口，道伥死亡 → 残卷掉落链路）
- `plan-npc-ai-v1` ✅（妖兽 AI 节点 / 道伥 AI 节点复用）

**反向被依赖**：

- `plan-narrative-political-v1` ✅ → 亚流派医道的特殊江湖角色叙事（毒手医被误解为毒蛊师 / 道伥医被视为异端）
- `plan-style-balance-v1` 🆕 → 亚流派 ρ 矩阵调整（毒手医 ρ 是否继承 dugu ρ=0.05？）
- `plan-multi-life-v1` ⏳ → 亚流派平和色 / 业力跨周目继承

---

## 接入面 Checklist

- **进料**：
  - `HealerProfile`（v1 实装，v2 新增亚流派标记字段 `subspecialty: YidaoSubspecialty`）
  - `PracticeLog`（亚流派专属 action 加独立维度 `subspec_dim`）
  - `FactionStore`（兽医结契走派系关系，道伥医走 tsy 遗迹 faction）
- **出料**：
  - `YidaoSubspecialty` enum：`ToxicMedic / BeastMedic / DaochangMedic`（P0 选定后注册）
  - 亚流派专属 2-3 招（在 v1 5 招基础上叠加，不替换）
  - 亚流派专属信誉度通道（兽医：妖兽信任度 / 道伥医：遗迹探索者声誉）
- **共享类型**：
  - 复用 `KarmaCounter`（亚流派独特业力积累方式）
  - 复用 `MedicalContract`（兽医结契妖兽走同一 component 扩展 patient_type: NpcKind）
  - **禁止**为亚流派独立造全新 HealerProfile 副本
- **跨仓库契约**：
  - server: `combat::yidao::subspec::*` 子模块（在 v1 `combat::yidao::*` 下扩展）
  - agent: `tiandao::yidao_subspec_runtime`（亚流派行为 narration + 特殊信誉度叙事）
  - client: 亚流派专属动画 + 粒子 + 音效（每亚流派 2-3 组）

---

## 阶段概览

| 阶段 | 目标 | 状态 |
|------|------|------|
| P0 | 选定亚流派方向（三选一）+ 2-3 招规格设计 | ⬜ |
| P1 | 亚流派专属招式实装 + YidaoSubspecialty 注册 | ⬜ |
| P2 | 亚流派信誉度通道 + NPC 行为节点 + client 动画 | ⬜ |
| P3 | e2e 验收 + 饱和化测试 | ⬜ |

---

## §0 设计轴心

- [ ] **P0 三选一（核心决策）**：
  - **毒手医**：接合度最高（dugu-v2 / qi_physics patch P3 已有底盘），但争议点：平和色 + 脏真元是否本质矛盾？worldview §六 没有明确排除，但需仔细推导
  - **兽医**：叙事独特性最强（只有兽医能跟顶阶妖兽结契），实装依赖 fauna-v1 ✅ 已完成，侵入性最小
  - **道伥医**：信息战路径最独特（残卷替代路径），但"不治疗道伥"的定调需严格守住 worldview §七，避免偏离
  - **推荐顺序**：兽医 > 道伥医 > 毒手医（兽医侵入性最小 + 叙事独特；道伥医世界观正典强；毒手医接口最多但争议也最多）
- [ ] **亚流派不替换 v1 招式**：v2 新增 2-3 招是对 v1 5 招的专精扩展（组合技 / 增强条件），不能删除或改变 v1 招式行为
- [ ] **平和色维持**：任何亚流派新招式都不引入攻击性 cast（worldview §六:617 真元几乎无杀伤性），即使毒手医方向也不能变成"以毒攻人"
- [ ] **业力代价扩展**：亚流派特殊操作（控制道伥 / 治疗顶阶妖兽 / 以毒治蛊）是否需要新业力代价公式？还是复用 v1 KarmaCounter 已有档位？

---

## §1 开放问题（P0 决策门收口）

- [ ] **Q1** 方向选定：三选一，具体叙事意象和核心招式规格
- [ ] **Q2** 毒手医平和色边界：脏真元注入患者后的排斥率 ρ 是否用 dugu 的 ρ=0.05 还是医道的最低排斥率？（两者同为 0.05 但来源不同）
- [ ] **Q3** 兽医经脉拓扑：妖兽经脉系统是否与人类 MeridianSystem 兼容？需要 plan-meridian-severed-v1 扩展 ContainerType::Beast 还是另建？
- [ ] **Q4** 道伥医风险定调：读取道伥残存战斗记忆的失败后果是什么？是否有"被道伥战斗记忆反噬"的代价（worldview §七 原文无此描述，需自定是否合理）？
- [ ] **Q5** 跨周目继承：亚流派 subspecialty 标记是否跨周目继承（plan-multi-life-v1 接口）？

---

## §9 进度日志

- **2026-05-07** 骨架立项。源自 plan-yidao-v1 §0 Q7 + 反向被依赖节 `plan-yidao-v2 占位` + reminder.md 医道首立 2026-05-06 段记录。

---

## Finish Evidence（待填）

迁入 `finished_plans/` 前必须填：

- **落地清单**：`server/src/combat/yidao/subspec/` + `YidaoSubspecialty` enum + 2-3 招实装 + 亚流派信誉度
- **关键 commit**：各 phase hash + 日期 + 一句话
- **测试结果**：`cargo test combat::yidao::subspec` 数量 / 亚流派 e2e 验收
- **跨仓库核验**：server 亚流派招式 / agent narration / client 动画
- **遗留 / 后续**：未选的两条亚流派方向（毒手医 / 兽医 / 道伥医 三选一后剩余两条，留 vN+1）
