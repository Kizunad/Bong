# Bong · plan-yidao-v1 · 骨架

医道功法 —— 跟 7 战斗流派平行的**支援流派**。实装接经术（SEVERED 经脉恢复）/ 排异加速 / 自疗 / 疗他人 / 续命术 / 急救 六大招式方向，配合**医者 NPC 长期医患关系系统**（plan-meridian-severed-v1 §5 主路径）。平和色染色加成：真元温和无攻击性，针灸通经络效率+，疗他人时排异成本-。**医道是修仙世界中唯一能主动修复 SEVERED 经脉的流派**，因此对所有六境界玩家都有需求，与 plan-social-v1 + plan-narrative-political-v1 深度耦合。

**世界观锚点**：`worldview.md §六:617「医道 / 平和色 / 针灸通经络效率+ / 疗他人时排异成本-」`· `§四:280-307 经脉损伤 4 档（INTACT/MICRO_TEAR/TORN/SEVERED）+ 流量公式`· `§四:286 SEVERED = 该经脉承载流派效果废`· `§六:622-631 染色规则（平和色形成 ~10h 修炼）`· `§十一:947-970 NPC 信誉度系统`（医者 NPC 长期医患关系物理基础）· `§十二:1043-1048 续命路径存在但有代价`（续命丹 / 夺舍 / 坍缩渊深潜，无免费午餐）

**library 锚点**：`cultivation-0006 经脉浅述`（医道进修前置读物）· 待补 `peoples-medicine-0001 医者百态`（本 plan 配合补 library 条目，描述不同境界医者的风格差异与历史故事）

**前置依赖**：

- `plan-meridian-severed-v1` ⬜ → `MeridianSeveredPermanent` / `MeridianSeveredEvent` / 接经术接口（`acupoint_repair`）— **本 plan 最核心前置**
- `plan-alchemy-v1` ✅ → 续命丹（配合续命术使用，医者 NPC 可推荐 / 开方）
- `plan-social-v1` ✅ → NPC 信誉度系统（医者 NPC 长期医患关系，信誉度影响接经成功率 / 收费）
- `plan-multi-style-v1` ✅ → 平和色 PracticeLog 注册（`StyleColor::Peaceful` 染色轨迹）
- `plan-skill-v1` ✅ → SkillRegistry / SkillSet / 熟练度 skill_lv 框架
- `plan-hotbar-modify-v1` ✅ → 招式 hotbar 渲染

**反向被依赖**：

- 所有玩家（任何角色都可能需要接经术 / 续命术，医道是通用服务方）
- `plan-meridian-severed-v1` → 接经术 `acupoint_repair` 接口的实装方
- `plan-narrative-political-v1` ✅ active → 医者声誉 / 求医江湖传闻 / 化虚断脉求医叙事
- `plan-multi-life-v1` ⏳ → 续命术与多周目寿元机制联调

---

## 接入面 Checklist

- **进料**：`cultivation::MeridianSystem`（SEVERED 状态读取）/ `MeridianSeveredPermanent`（接经目标）/ `social::NpcReputation`（医者信誉度）/ `alchemy::Pill { ContaminationNeutralizer, LifeExtension }`（续命丹 / 排异丹入库）/ `combat::BleedEvent`（急救触发源）/ `qi_physics::ledger::QiTransfer`（所有真元流动走守恒账本）
- **出料**：
  - `MeridianRepairAttemptEvent { healer, target, meridian_id, success }` → 由 plan-meridian-severed-v1 消费（接经成功写入 / 失败升级损伤）
  - `ContaminationNeutralizeEvent { amount, target }` → 中和 contam（排异加速招式出料）
  - `BleedStopEvent { target }` → 急救止血出料
  - `LifespanExtendEvent { amount, cost_qi_max }` → 续命术出料（走 plan-death-lifecycle-v1 寿元账本）
  - 平和色 `PracticeLog::record(StyleColor::Peaceful, duration)` → 染色轨迹
- **共享类型**：复用 `MeridianId`（plan-meridian-severed-v1）/ `SeveredSource`（plan-meridian-severed-v1）/ `NpcReputation`（plan-social-v1）/ `Pill`（plan-alchemy-v1）
- **跨仓库契约**：
  - server: `cultivation::yidao::*` 主实装 + `social::npc::healer_ai::*` 医者 NPC AI
  - agent: `tiandao::yidao_narration`（接经成功/失败叙事 + 医者声誉叙事 + 续命代价叙事）
  - client: 接经术求医 NPC dialog UI + 医者技能读条动画 + 平和色染色显示
- **worldview 锚点**：见头部
- **qi_physics 锚点**：接经术 / 疗他人消耗医者自身真元，走 `qi_physics::ledger::QiTransfer { from: healer, to: env, amount }`（守恒——疗愈消耗从医者流出）；排异加速走 `qi_physics::excretion`（加速 contam 中和速率，不违反守恒）；续命术走 `qi_physics::ledger::QiTransfer { from: patient.qi_max_pool, to: lifespan }`（qi_max 永久减换寿元，worldview §十二:1048 无免费午餐）

---

## §0 设计轴心

- [ ] **医道是支援流派，不是战斗流派**（worldview §六:617）：平和色真元"几乎无杀伤性"——医道修士在 PVP 场合不是主战手，而是**被招募 / 雇佣 / 结契的核心资源**。这让医道成为修仙世界稀缺的社交货币，而非又一个战力维度

- [ ] **接经术是社交服务，不是单人 PvE**（worldview §十一 NPC 信誉度 + plan-meridian-severed-v1 §5）：
  - 主路径：**医者 NPC 长期医患关系**（而非玩家直接 cast）
  - 流程：`寻医 → dialog 评估 → 报价（骨币 + 信誉度 + 跑腿任务）→ 接经仪式（医者 cast 接经术）→ roll 成功率`
  - 成功：MeridianSeveredPermanent 该经脉移除，医者信誉度 +5
  - 失败：经脉永久损伤升级（无法再尝试接经），医者信誉度 -3，玩家额外受伤
  - **医者境界决定能接的经脉和成功率**（见 §3 医者 NPC 分级）
  - **备选 PvE 路径**：上古接经术残卷（plan-tsy-loot-v1 ✅，一次性使用 / 自动成功 / 极稀有 jackpot）

- [ ] **续命术代价正典**（worldview §十二:1048）：续命必须以**业力 / 境界 / 真元上限**为代价，没有免费路径。本 plan 实装的续命术消耗 `qi_max` 换寿元（结合续命丹，plan-alchemy-v1 ✅），医者只能执行仪式，不能绕过代价

- [ ] **排异加速比 zhenmai ② 局部中和效率高 ×3**（worldview §六:617「疗他人时排异成本-」）：平和色的核心优势——医道真元温和，注入他人经脉时引起的排异反应远低于其他流派，使高效中和 contam 成为可能。这是医道唯一的"战斗辅助"价值

- [ ] **平和色染色养成**（worldview §六:622 ~10h 主色调）：修炼接经术 / 排异加速 / 自疗 / 续命术任一招式，累计 ~10h 后出现平和色主色调。跟其他流派的色调冲突（如毒蛊阴诡色 vs 平和色），会形成"杂色"失效（worldview §六:628-631）

- [ ] **急救是任何境界医道修士都能做的基础技**：HP 出血止血，不需要修炼等级，只要是医道修士就有。这是医道的"入场票"，让 worldview §十一 NPC 在任何战斗场景里都有求医需求

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | 决策门：6 招式设计定稿 + 接经术与 plan-meridian-severed-v1 协议（接口签名）+ 医者 NPC 分级定稿（境界 / 成功率 / 能接经脉范围）+ 续命术代价公式 + 平和色 PracticeLog 注册 | 接口签名 + NPC 分级 + 代价公式落 §2-§3 |
| **P1** ⬜ | server 主模块：`cultivation::yidao::*` 6 招式实装 + 医者 NPC AI（dialog / 评估 / 接经仪式 / 报价 / roll 成功率）+ 平和色 PracticeLog 注册 + 排异加速 / 急救 / 续命术实装 + 医者 NPC 分级逻辑 + ≥45 单测 | `cargo test cultivation::yidao` 全过 / 6 招式覆盖 / 医者 NPC 对话流程测试 / 接经术成功+失败 roll |
| **P2** ⬜ | client UI：接经术求医 NPC dialog + 医者报价界面 + 接经仪式读条动画 + 平和色染色显示更新 + 急救读条 | WSLg 实跑 NPC dialog / 接经术读条 / 平和色染色显示 |
| **P3** ⬜ | agent narration：接经成功/失败叙事（古意风格）+ 医者声誉叙事 + 续命术代价叙事 + plan-narrative-political-v1 联调（化虚断脉求医江湖传闻）| narration-eval ✅ 接经成功/失败 + 续命代价 + 医者声誉场景全过古意检测 |

---

## §2 招式设计

### 六招式方向

| # | 招式 | 描述 | 前置境界 | 依赖经脉（参考）|
|---|---|---|---|---|
| ① | **接经术** | 尝试恢复目标一条 SEVERED 经脉。医者 cast，走 plan-meridian-severed-v1 `acupoint_repair` 接口。成功率 = f(医者境界, 经脉位置, SEVERED 时长) | 凝脉+ | 任督（REN/DU）+ LU |
| ② | **排异加速** | 加速目标 contam 中和，效率 = zhenmai ② 局部中和 ×3。消耗医者自身真元走 QiTransfer | 引气+ | HT + SP |
| ③ | **自疗** | 自身 HP + MICRO_TEAR/TORN 快速回复（不能恢复 SEVERED）。消耗自身真元 | 醒灵+ | LU + KI |
| ④ | **疗他人** | 对目标 HP + MICRO_TEAR/TORN 回复（比自疗效率高 ×1.5，因平和色排异成本-）。消耗医者自身真元 | 引气+ | LU + KI + HT |
| ⑤ | **续命术** | 结合续命丹，将 qi_max 永久减少换取寿元延长（worldview §十二:1048 无免费午餐）。消耗骨币 + 续命丹 | 固元+ | 任督全通 |
| ⑥ | **急救** | HP 出血止血，不需要修炼等级，任何医道修士皆可用。无真元消耗（基础技术处置）| 醒灵 | LU（基础）|

### 真元消耗走 qi_physics 守恒

所有消耗医者真元的招式（① ② ③ ④ ⑤）必须走：
```rust
qi_physics::ledger::QiTransfer {
    from: healer_entity,   // 医者实体
    to: env_or_target,     // 环境 zone 或目标
    amount,
}
```

不允许 `cultivation.qi_current -= X` 无对应转账。

---

## §3 医者 NPC 分级

worldview §十一 NPC 信誉度系统 + §六:617 医道流派：

| 医者境界 | 能接经脉范围 | 成功率基础 | 费用（骨币）| 信誉要求 |
|---|---|---|---|---|
| 引气-凝脉 | 手三阴/三阳（正经，非任督）| 40-60% | 5-20 | 无 |
| 固元-通灵 | 12 正经全部 | 65-80% | 30-100 | 中等（≥50）|
| 化虚 | 20 经全部（含任督 / 奇经）| 85-95% | 200-500 | 高（≥120）+ 任务 |

- **跑腿任务**（固元+ 医者要求）：玩家需先完成 1-3 个采药 / 寻药 / 护送任务，医者才肯约诊
- **失败代价**（worldview §十一 NPC 信誉）：接经失败时医者信誉度 -3，玩家额外受伤 + 经脉损伤升级（不可再接）
- **化虚医者稀有度**：每服务器最多同时存在 2 名化虚医者 NPC，可死亡 / 消失（plan-npc-ai-v1 ✅ NPC 生命周期机制）

---

## §4 平和色养成接口

```rust
// plan-multi-style-v1 ✅ 已有 PracticeLog，本 plan 注册平和色
PracticeLog::register_style(
    StyleColor::Peaceful,
    vec![
        SkillId::Yidao_Acupoint,    // ① 接经术
        SkillId::Yidao_Detox,       // ② 排异加速
        SkillId::Yidao_SelfHeal,    // ③ 自疗
        SkillId::Yidao_HealOther,   // ④ 疗他人
        SkillId::Yidao_Lifespan,    // ⑤ 续命术
        SkillId::Yidao_FirstAid,    // ⑥ 急救
    ],
);
```

平和色主色调形成：~10h 医道修炼累积（worldview §六:622）。

与其他色调冲突规则（worldview §六:628-631）：
- 平和色 + 阴诡色（毒蛊）→ 杂色，两种专精效果失效（社会 / 功法选择冲突，worldview §六:618 毒蛊阴诡色 / §六:617 医道平和色）
- 平和色 + 凝实色（器修）→ 主副双修，各 70%（可接受）
- 平和色 + 温润色（炼丹师）→ 同性接近，主副双修，自疗速度接近叠加

---

## §5 客户端新建资产

| 类别 | ID | 优先级 | 备注 |
|---|---|---|---|
| UI | 接经术求医 NPC dialog | P2 | 评估 → 报价 → 接经仪式流程，分页 |
| UI | 医者报价界面 | P2 | 骨币 + 信誉度 + 跑腿任务列表 |
| 动画 | 接经术读条（医者端）| P2 | 8s 读条，医者站在目标侧面，光点沿经脉移动 |
| 动画 | 急救止血读条 | P2 | 3s 读条，简洁 |
| 粒子 | `MERIDIAN_REPAIR_GLOW` | P2 | 接经成功时，目标经脉位置发柔和金光（vs SEVERED 时的金光裂，plan-meridian-severed-v1） |
| 音效 | `yidao_heal_pulse` | P3 | layers: `[{ sound: "block.amethyst_block.chime", pitch: 1.2, volume: 0.5 }]`（柔和共鸣）|
| HUD | 平和色染色进度条（排行色调 tab）| P2 | 归入 plan-multi-style-v1 ✅ inspect UI 染色 tab |

---

## §6 测试矩阵（饱和化）

下限 **45 单测**：

| 模块 | 测试组 | 下限 |
|---|---|---|
| `yidao::skill::acupoint` | 成功恢复 INTACT（roll 强制成功）/ 失败升级损伤（roll 强制失败）/ 境界不足拒绝 / plan-meridian-severed-v1 接口 `acupoint_repair` 契约对拍 | 10 |
| `yidao::skill::detox` | 排异加速 contam 中和速率 ×3 验证（vs zhenmai ② 基准）/ QiTransfer 守恒断言 / 目标 contam=0 无副作用 | 6 |
| `yidao::skill::heal` | 自疗 HP + TORN 恢复 / 疗他人 ×1.5 效率 / 平和色排异成本- 验证 / QiTransfer 守恒断言 | 8 |
| `yidao::skill::lifespan` | 续命术 qi_max 扣减 vs 寿元延长比例 / 无续命丹拒绝 / worldview §十二:1048 "无免费午餐"不可绕过断言 | 6 |
| `yidao::skill::firstaid` | 急救止血 / 不消耗真元断言 / 醒灵境界可用 | 4 |
| `healer_npc::dialog_flow` | 寻医 → 评估 → 报价 → 任务 → 仪式 → 成功/失败 完整流程 / 信誉度 ± / 化虚医者稀有度上限 2 | 8 |
| `yidao::practice_log` | 平和色注册 / 10h 累积主色调形成 / 与阴诡色冲突杂色 / 与温润色双修 70% | 3 |

**P1 验收**：`grep -rcE '#\[test\]' server/src/cultivation/yidao/` ≥ 45。

---

## §7 开放问题 / 决策门

### #1 医道玩家 vs 医者 NPC 的边界

- **A**：医道**玩家**直接对其他玩家 cast 疗愈招式（PvP 辅助角色）
- **B**：接经术仅医者 NPC 能做，玩家只能对自身使用疗愈招式
- **C**：玩家医道可对他人 cast 排异加速 / 急救 / 疗他人，但接经术仅 NPC（因为需要专业判断 + 失败代价）

**默认推 C** —— 让医道玩家有实际 PVP 支援价值（疗他人 + 排异加速），同时接经术保持 NPC 专属的稀缺感 + 叙事张力（失败代价由 NPC 承担 / 玩家选择信任谁）

### #2 续命术的 qi_max 消耗比例

- **A**：固定 qi_max 消耗 20% 换 50 年寿元（简单）
- **B**：线性：消耗 X qi_max（点数）换 `X × 5` 年寿元（可控制）
- **C**：tiered：醒灵-固元低效（10:30 year）/ 通灵-化虚高效（10:80 year，境界决定转化率）

**默认推 C** —— 跟境界体系对齐，高境界续命代价越小但入门越高，叙事合理

### #3 医者 NPC 失败后能否再次尝试

- **A**：失败一次即锁死该经脉（不可再接），唯一出路是上古残卷
- **B**：失败后 7 天冷却可再次尝试，但每次成功率 -5%（叠加衰减）
- **C**：不同医者境界可重试（引气医者失败后可去通灵医者重试，但成功率受损伤升级影响）

**默认推 A** —— 简洁，与 worldview §十二 "死亡是学费" 一致，保留上古残卷的稀缺价值

### #4 plan-yidao-v2 范围

医道后续扩展（本 plan 不实装，占位）：
- 毒手医（医道 + 毒蛊的双修路径，平和色 + 阴诡色冲突 → 杂色，但有特殊技能）
- 兽医（妖兽绑定疗愈，plan-fauna-v1 ✅ 联动）
- 道伥医（治亡灵，plan-multi-life-v1 ⏳ 联动）

### #5 医道招式依赖经脉声明（plan-meridian-severed-v1 §3 强约束）

P0 决策门时必须明确 6 招式各自的依赖经脉（§2 表格是粗粒度参考），在 SkillRegistry 注册时调 `.with_dependencies(meridian_ids)`。

---

## §8 进度日志

- **2026-05-06** 骨架立项。源自 plan-meridian-severed-v1 §5 接经术主路径 + §9 派生记录。worldview §六:617 医道平和色正典已锚定。核心设计：支援流派定调（非战斗主力）+ 接经术 NPC 社交服务 + 续命术代价正典 + 平和色养成。反向被依赖：所有玩家（通用服务需求）+ plan-meridian-severed-v1 接口消费方 + plan-narrative-political-v1 医者声誉叙事。
  - 6 招式方向锁定（接经术/排异加速/自疗/疗他人/续命术/急救）
  - 医者 NPC 三境界分级（引气凝脉/固元通灵/化虚）+ 失败代价机制
  - 开放问题 #1-#5 待 P0 决策门收口

---

## Finish Evidence（待填）

迁入 `finished_plans/` 前必须填：
- **落地清单**：`server/src/cultivation/yidao/` 主模块 + `social::npc::healer_ai::*` + 6 招式实装 + 平和色 PracticeLog 注册
- **关键 commit**：P0/P1/P2/P3 各自 hash + 日期 + 一句话
- **测试结果**：`cargo test cultivation::yidao` 数量 / 接经术成功+失败 roll / 医者 NPC 对话流程 / WSLg 实测 NPC dialog + 接经读条
- **跨仓库核验**：server 主模块 + 医者 NPC AI / agent narration 接经叙事 + 续命代价 / client NPC dialog + 平和色染色显示
- **遗留 / 后续**：plan-yidao-v2 占位（毒手医 / 兽医 / 道伥医）/ plan-social-v1 医患关系 reputation 联调 / plan-tsy-loot-v1 上古接经术残卷 jackpot 备选路径回填
