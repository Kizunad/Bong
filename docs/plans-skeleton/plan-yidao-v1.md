# Bong · plan-yidao-v1 · 骨架

医道功法 —— **跟 7 战斗流派平行的支援流派**。平和色真元温和无攻击性，专注治疗、经脉接续与续命；医者既可是玩家角色，也可以是 NPC（通过 plan-npc-ai-v1 big-brain 扩展）。核心招式 6 个：**接经术**（恢复 SEVERED 经脉）/ **排异加速**（中和异种真元污染，效率 ×3 vs zhenmai ②）/ **自疗**（真元稳定 + HP 加速回复）/ **疗他人**（对他人外治）/ **续命术**（临终延缓，配合续命丹）/ **急救**（止血·封创）。是所有角色受重伤后最重要的求助路径，与 plan-meridian-severed-v1 形成强耦合（接经术实现接口）。

**世界观锚点**：
- `worldview.md §六:617 医道 / 平和色` —— 真元温和无攻击性 / 针灸通经络效率+ / 疗他人时排异成本- / 真元几乎无杀伤性（正典）
- `§四:280-307 经脉损伤 4 档` —— 接经术恢复目标为 SEVERED（0.0 流量）
- `§四:347 排异反应` —— 疗他人排异成本- 的物理依据（医道真元温和，触发异体排斥系数低）
- `§十一:947-970 NPC 信誉度系统` —— 医者 NPC 长期医患关系 + 价格/信誉结构
- `§十二:1043-1048 续命路径` —— 续命丹/续命术存在但无免费午餐，以业力/境界/qi_max 为代价
- `§五:537 流派由组合涌现` —— 医道支援流派无境界门槛只有能力天花板（境界决定上限）
- `§五:506 末法残土后招原则` —— 熟练度决定响应速率（cooldown / cast 窗口），符合 v2 通用机制

**library 锚点**：`cultivation-0006 经脉浅述`（接经术背景知识）· 待补 `peoples-medicine-0001 医者百态`（本 plan P1 配合补写 library 条目，记录末法残土下"医者之殇"——医道真元无攻击性，战时只能被动等待）

**前置依赖**：

- `plan-meridian-severed-v1` ⬜ → **接经术实现接口**（`acupoint_repair` 接口 + MeridianSeveredPermanent 数据层）；本 plan P1 依赖此接口已定稳
- `plan-alchemy-v1` ✅ → 续命丹（`RenewingPill`），续命术 cast 时消耗/配合此道具
- `plan-social-v1` ✅ → NPC 信誉度框架（医者 NPC 报价、拒绝、信誉成长）
- `plan-multi-style-v1` ✅ → 平和色 PracticeLog hook（医道修习记录 → 平和色养成）
- `plan-npc-ai-v1` ✅ → big-brain Scorer/Action 框架（医者 NPC 行为决策底盘）
- `plan-skill-v1` ✅ → `SkillSet` + `SkillXpGain` event（医道熟练度升级）
- `plan-qi-physics-v1` ✅ → `qi_physics::ledger::QiTransfer`（接经术大量真元流动守恒）

**反向被依赖**：

- `plan-meridian-severed-v1` ⬜ → 接经术 NPC 交互 UI（client part in plan-meridian-severed-v1 P2，本 plan 提供实现）
- 所有 v2 流派 plan → SEVERED 后求医主路径，player 会向医者 NPC 求接经
- `plan-narrative-political-v1` ✅ active → 化虚级 SEVERED 后求医江湖传闻
- `plan-multi-life-v1` ⏳ → 续命术在多周目边界行为（临终延缓不阻止周目切换）

---

## 接入面 Checklist

- **进料**：
  - `plan-meridian-severed-v1::acupoint_repair(entity, meridian_id, caster_realm) -> RepairResult` → 接经术实现侧
  - `combat::contamination::ContamLevel` → 排异加速消费的污染值
  - `alchemy::RenewingPill` item + `shelflife::is_fresh()` → 续命术 cast 时校验
  - `social::npc::ReputationStore` → 医者 NPC 价格/信誉结构
  - `cultivation::Cultivation { qi_current, hp, lifespan_remaining }` → 治疗读写目标
  - `qi_physics::ledger::QiTransfer` → 接经术真元流转守恒
  - `skill::SkillXpGain` → 医道熟练度增长 event

- **出料**：
  - `yidao::AcupointRepairCast { caster, target, meridian_id }` event（接经仪式开始）
  - `yidao::HealEvent { caster, target, hp_restore, qi_stabilize }` event
  - `yidao::ContamPurgeEvent { caster, target, contam_reduced }` event
  - `yidao::EmergencyHemostasisEvent { target }` event（急救止血）
  - `yidao::LifeExtendEvent { target, seconds_extended }` event（续命术）
  - 平和色 PracticeLog hook → plan-multi-style-v1（每次成功治疗他人 +healing_points）
  - `SkillXpGain { skill: SkillId::YIDAO, amount }` event

- **共享类型**：
  - `MeridianId` enum（plan-meridian-severed-v1 定义，本 plan import）
  - `RepairResult { success, new_state: MeridianState }` → 接经术结果
  - `YidaoSkillId` enum（接经术 / 排异加速 / 自疗 / 疗他人 / 续命术 / 急救）
  - `ContainerKind::LivingMeridian`（qi_physics 底盘，接经术真元注入走此类型）

- **跨仓库契约**：
  - server: `server/src/yidao/` 主模块（6 招式 + 医者 NPC AI 扩展）
  - agent: `tiandao` 订阅 `bong:yidao/heal_event` + `bong:yidao/repair_event` → narration（求医叙事 / 治疗江湖传闻）
  - client: 接经仪式施法动画 + 被治疗者受益反馈 + 医者 NPC dialog UI（报价/成功/失败）

- **worldview 锚点**：见头部

- **qi_physics 锚点**：
  - 接经术：`qi_physics::ledger::QiTransfer { from: caster, to: target_meridian, amount }` — 大量真元注入目标经脉
  - 平和色排异成本-：`qi_physics::collision::排斥系数 × YIDAO_排斥_MULTIPLIER`（worldview §六:617，疗他人时排异成本降低，由 qi_physics P3 或本 plan P0 扩 CollisionParam 接口）
  - 续命术无真元"凭空生"：临终延缓是借走玩家 qi_max 换 lifespan（QiTransfer from player.qi_max_pool to lifespan_ledger）

---

## §0 设计轴心

- [ ] **支援流派，无境界门槛只有上限**（worldview §五:537）：医道不锁境界，任何人都可以修习，但境界决定接经术成功率上限、排异加速倍率上限。初级医者只能接手三阴/三阳，化虚医者才能稳稳接任督。技能 Lv（熟练度）决定 cooldown 和 cast 速度，符合 v2 通用机制

- [ ] **医道真元 ≈ 无攻击性**（worldview §六:617）：
  - 医道招式不能被当作攻击使用（接经术/排异加速/续命术对敌人无效，cast 判断 target 是否友好）
  - 接经术注入真元到对方经脉，若 target 主动排斥（战斗中），失败率大幅上升
  - 平和色真元在 PVP 环境里几乎无伤害——是 worldview 最纯粹的"非战斗流派"物理化身

- [ ] **接经术是高代价社交服务，不是廉价修复**（plan-meridian-severed-v1 §5）：
  - 大量真元消耗（施术者 qi_current 必须 > 治疗所需 QiTransfer）
  - 失败概率存在；失败后该经脉升级损伤（"死脉"，无法再尝试接经）
  - 需要患者配合（target 不能主动抵抗，否则失败率暴增）
  - 医者信誉度影响玩家是否愿意找他看诊（plan-social-v1）

- [ ] **医者 NPC 与玩家医者共存**：NPC 医者用 big-brain AI 决策（plan-npc-ai-v1 框架），提供稳定但价高的接经服务；玩家医者通过修习医道功法，可自主开诊。两者使用同一套技能接口

- [ ] **续命术无免费午餐**（worldview §十二:1048）：
  - 续命术消耗施术者大量 qi_current + 目标 qi_max 永久下降（每次续命 -5%）
  - 配合续命丹（plan-alchemy-v1）：有丹药时成功率 ×2，无丹药时失败率极高
  - 续命次数上限：同一目标同一周目 ≤ 3 次续命术，超过则天道强制终结

- [ ] **排异加速 vs zhenmai ② 局部中和**：
  - zhenmai ② 是战斗中的即时中和（代价是自身经脉反震负荷）
  - 医道排异加速是非战斗环境下的缓慢深度排毒（×3 效率 + 无反噬，但 cast time 长 + 施术者需持续输出平和色真元）

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | 决策门：6 招式接口 + 数据模型定稿 + 排异系数接口确认（qi_physics 扩 or 本 plan 自处理）+ 成功率公式 + 医者 NPC AI 框架设计（big-brain Scorer 分工）+ SkillId::YIDAO 注册方式 + 续命术与 lifespan 账本接口 | 接口设计落入 §2 / §3 / §8 决策门收口 |
| **P1** ⬜ | server `server/src/yidao/` 主模块：6 招式 cast handlers + 接经术接 plan-meridian-severed-v1 `acupoint_repair` 接口 + 排异加速接 contamination system + 续命术接 lifespan / alchemy + 医者 NPC AI Scorer/Action + 平和色 PracticeLog hook + SkillId::YIDAO + `peoples-medicine-0001` library 条目 + ≥40 单测 | `cargo test yidao::*` 全过 / 6 招式 happy path + 边界 + 失败分支均有覆盖 / 接经术接 plan-meridian-severed-v1 接口契约测试通过 |
| **P2** ⬜ | client：接经仪式施法动画（玩家锁定目标 + 进度条）+ 被治疗者受益反馈（HP 绿粒子 + 真元稳定提示）+ 医者 NPC dialog（报价 / 拒绝 / 成功 / 失败文本）+ 平和色 inspect UI 显示 + 接经失败「死脉」提示 | WSLg 实跑：玩家 → 医者 NPC → 接经仪式动画 → 结果反馈（成功 / 失败各一）|
| **P3** ⬜ | agent narration：`bong:yidao/repair_event` 触发接经叙事 + `bong:yidao/heal_event` 治疗叙事 + 接经失败「死脉」叙事 + 续命术叙事 + plan-narrative-political-v1 接：化虚级 SEVERED 求医江湖传闻 + 平和色养成里程碑 narration | narration-eval ✅ 6 类医道事件全过古意检测 |

---

## §2 招式设计

### 6 招式总览

| # | 招式名 | 类型 | 目标 | 境界上限 | 核心效果 |
|---|---|---|---|---|---|
| ① | 接经术 | 核心 | 他人（需配合）| 化虚（稳定接任督）| 尝试恢复 SEVERED 经脉，成功率 = f(境界, 熟练度, 经脉位置, 时长) |
| ② | 排异加速 | 辅助 | 他人 | 通灵（全部经脉型污染）| 中和 contamination，×3 效率 vs zhenmai ②，cast time 30s |
| ③ | 自疗 | 基础 | 自身 | 引气（早期可用）| qi 稳定化（抑制异体排斥 tick）+ HP 回速 ×1.5，30s 持续 |
| ④ | 疗他人 | 辅助 | 他人 | 凝脉 | HP 回速 ×2 + 排异成本- 30s buff（平和色接触效果）|
| ⑤ | 续命术 | 极限 | 他人（临终）| 固元（稳定成功）| 延缓死亡 60-180s（看境界），消耗施术者 qi_current 80% + 目标 qi_max -5% 永久 |
| ⑥ | 急救 | 基础 | 他人 | 醒灵（最基础）| 止血（取消出血 debuff）+ 封创（5s 无法受 CombatWound），消耗少 |

### 接经术成功率公式（草案）

```rust
fn acupoint_repair_success_rate(
    caster: &Cultivation,      // 医者境界
    target_meridian: MeridianId,
    severed_duration_ticks: u64,
    yidao_skill_lv: u8,
) -> f32 {
    let realm_factor = match caster.realm {
        Realm::WakingSpirit => 0.30,   // 醒灵医者高风险
        Realm::QiGathering  => 0.45,
        Realm::PulseForging => 0.60,
        Realm::CoreSetting  => 0.72,
        Realm::SpiritLink   => 0.85,
        Realm::VoidReach    => 0.94,
    };
    let meridian_difficulty = match target_meridian {
        MeridianId::REN | MeridianId::DU => 0.60,  // 任督最难
        _ if is_main_meridian(target_meridian) => 0.85,
        _ => 1.0,
    };
    let duration_penalty = 1.0 - (severed_duration_ticks as f32 / (7200.0 * 24.0)).min(0.40);
    let skill_bonus = (yidao_skill_lv as f32 / 100.0) * 0.10;  // 最多 +10%
    (realm_factor * meridian_difficulty * duration_penalty + skill_bonus).min(0.95)
}
```

### 熟练度生长二维划分（v2 通用机制，yidao 应用）

- **境界 = 威力上限**：可接经脉范围 / 排异倍率上限 / 续命术延续秒数 / 急救封创时长
- **熟练度（SkillId::YIDAO lv）= 响应速率**：cast time 缩短 / cooldown 降低
  ```
  接经术 cast_time(lv) = 60s + (10s - 60s) × clamp(lv/100, 0, 1)  // lv100 最快 10s
  排异加速 cooldown(lv) = 120s + (30s - 120s) × clamp(lv/100, 0, 1)
  ```
- 前置：plan-skill-v1 ✅（SkillId 扩 `YIDAO` 枚举）

---

## §3 医者 NPC AI（big-brain 框架扩展）

基于 plan-npc-ai-v1 ✅ 的 big-brain Scorer/Action 模式。

```
YidaoNpcBrain {
    Scorer: SeekPatientNeedingCare   // 扫描周围 SEVERED / 临终 / 出血 玩家
    Scorer: CheckReputationThreshold // 信誉不足 → 拒绝
    Scorer: CheckQiSufficiency       // 自身 qi_current < 接经消耗 → 跳过
    Action: OfferDiagnosisDialog     // 开价 dialog（骨币 + 信誉需求）
    Action: CastAcupointRepair       // 执行接经术（成功/失败 emit event）
    Action: CastEmergencyHemostasis  // 急救止血（优先处理）
    Action: CastContamPurge          // 排异（次要）
    Action: RefusePatient            // 拒绝（信誉/qi不足 / 信仰冲突）
}
```

**医者 NPC 分级**（worldview §十一 NPC 反应分级）：

| 境界 | 可接经脉 | 接经成功率基础 | 价格（骨币）|
|---|---|---|---|
| 醒灵医者 | 手三阴 / 手三阳 | 0.30-0.45 | 低（20-80 骨币）|
| 凝脉-固元医者 | + 足三阴 / 足三阳 | 0.60-0.72 | 中（200-800 骨币）|
| 通灵医者 | + 任督（高风险）| 0.85 | 高（2000+ 骨币）|
| 化虚医者 | 所有经脉 | 0.94 | 极高（5000+ 骨币 + 信誉 5+）|

---

## §4 平和色养成

医道功法修习 → 平和色养成，走 plan-multi-style-v1 ✅ PracticeLog hook：

```rust
// 每次成功治疗他人（接经术成功 / 排异加速完成 / 续命术成功）
emit PracticeLogEvent {
    style: StyleKind::Yidao,
    points: match skill {
        AcupointRepair => 50,   // 核心技能，权重高
        ContamPurge    => 20,
        HealOther      => 10,
        LifeExtend     => 40,
        _              => 5,
    }
};
```

**平和色实际加成**（worldview §六:617，通过 plan-multi-style-v1 机制）：
- 针灸通经络效率+：接经术 `meridian_difficulty` 系数 ×1.1（平和色≥主色调时）
- 疗他人排异成本-：`qi_physics::collision` 接口中 `YIDAO_排斥_MULTIPLIER = 0.5`（平和色主色调时）

---

## §5 客户端新建资产

| 类别 | ID | 优先级 | 备注 |
|---|---|---|---|
| 动画 | 接经仪式锁定动画 | P2 | 玩家锁定目标 10-60s，双方静止；进度条打断即失败 |
| 粒子 | `YIDAO_HEAL_GLOW` | P2 | 治疗成功后柔和金绿粒子（平和色真元可视化）|
| 粒子 | `YIDAO_REPAIR_THREAD` | P2 | 接经术：金线从施术者手 → 目标经脉位置（细丝连接）|
| 粒子 | `YIDAO_REPAIR_FAIL` | P2 | 接经术失败：金线断裂 + 暗红闪光（"死脉"提示）|
| UI | 医者 NPC dialog | P2 | 报价 / 拒绝 / 成功 / 失败文本框（复用 social-v1 dialog 框架）|
| UI | 「死脉」HUD 提示 | P2 | 接经失败后永久标记该经脉「死脉 · 无法再次接续」|
| 音效 | `yidao_thread_pull` | P3 | 接经过程中持续低频鸣响（细线绷紧感）|
| 音效 | `yidao_repair_success` | P3 | 接经成功：清脆铜铃 ×2 |
| 音效 | `yidao_repair_fail` | P3 | 接经失败：沉闷裂声 + 玩家受伤音 |

---

## §6 测试矩阵（饱和化）

下限 **40 单测**：

| 模块 | 测试组 | 下限 |
|---|---|---|
| `acupoint_repair` | 成功恢复（各境界 × 各经脉位置）/ 失败升级损伤 / target 抵抗时失败率上升 / qi 不足拒绝 | 10 |
| `contam_purge` | ×3 效率 vs zhenmai 基线 / cast time 按熟练度曲线 / target contam = 0 时 noop | 6 |
| `heal_other` | HP 回速 buff 持续 30s / 排异成本- 接 qi_physics 返回 / 对敌目标无效 | 6 |
| `life_extend` | 延缓秒数 = f(境界) / qi_max 永久 -5% / 同周目 ≤3 次限制 / 续命丹存在时成功率 ×2 | 8 |
| `emergency_hemostasis` | 取消出血 / 封创 5s 无 CombatWound / 对友目标有效 / 对敌无效 | 5 |
| `yidao_npc_ai` | SeekPatientNeedingCare Scorer 触发 / 信誉不足拒绝 / qi 不足跳过 / 接经成功 narration emit | 5 |

**P1 验收**：`grep -rcE '#\[test\]' server/src/yidao/` ≥ 40。

---

## §7 开放问题 / 决策门

### #1 排异成本- 接入 qi_physics 方式

- **A**：扩 `qi_physics::collision::CollisionParam` 加 `healer_style_bonus: Option<f32>` 字段（推荐）
- **B**：本 plan 在 `contam_purge` / `heal_other` cast 时临时修改 `ContaminationTick` 的消耗系数
- **C**：不接 qi_physics，本 plan 直接写常数 0.5 修正

**默认推 A** —— 走 qi_physics 唯一实现入口，符合 CLAUDE.md §四 红旗约束；需要 plan-qi-physics-patch-v1 P3 阶段扩接口

### #2 医者 NPC 拒绝患者的条件粒度

- **A**：只看信誉度 threshold（简单）
- **B**：信誉度 + 玩家对 NPC 的流派冲突（毒蛊玩家找善良医者可能被拒）
- **C**：信誉 + 流派冲突 + 当前 NPC qi_current 是否充足

**默认推 C** —— 全面，且 NPC qi 消耗真实化（医者也会累）

### #3 SkillId::YIDAO 的 XP 曲线

- **A**：复用 plan-skill-v1 的通用曲线（快速实装）
- **B**：单独设计更平缓的曲线（医道是服务型，lv100 不代表"打得过"）

**默认推 A** —— 先用通用曲线，数值不对 P1 后再微调

### #4 接经仪式被打断后的处理

- **A**：直接失败，不消耗 qi，可重试
- **B**：失败 + 消耗已注入真元（但不造成经脉损伤升级）
- **C**：失败 + 消耗真元 + 一定概率触发轻微经脉损伤（施术者）

**默认推 B** —— 有代价但不过分惩罚，鼓励稳定环境求医

### #5 续命术「同周目 ≤3 次」技术实现

- **A**：在 `Cultivation` component 加 `life_extend_count: u8`（简单）
- **B**：在 `LifeRecord` 记录续命事件，server 查历史计数（更符合历史书逻辑）

**默认推 A** —— 技术简单；续命记录可并入 LifeRecord 作为叙事用途但不作为计数依据

### #6 医道 vs 炼丹师 染色区别

worldview §六:617 有医道/平和色；§六:610 有炼丹师/温润色（"自疗速度+，可中和异种真元"）。两者有重叠：

- **区别**：温润色靠丹药 / 平和色靠针灸经络；炼丹师自疗++ / 医道疗他人++
- **接口隔离**：本 plan 的排异加速走医道路径（需要持续注入平和色真元 × cast time）；炼丹师的中和走 alchemy::purge_antidote（消耗道具）—— 互不侵占，P0 决策门确认

---

## §8 进度日志

- **2026-05-06** 骨架立项。源自 plan-meridian-severed-v1 §5 「接经术主路径 = 医者 NPC 服务（plan-yidao-v1 🆕 实装）」+ reminder.md 占位「派生新流派占位（2026-05-06）—— 医道功法」。
  - 设计轴心：支援流派无境界门槛 / 医道真元 ≈ 无攻击性 / 接经术高代价 / 医者 NPC 与玩家医者共存 / 续命无免费午餐
  - 6 招式定稿：接经术 / 排异加速 / 自疗 / 疗他人 / 续命术 / 急救
  - worldview 锚点对齐：§六:617（医道正典）+ §四:280-307（经脉 4 档）+ §四:347（排异）+ §十一（NPC）+ §十二:1043-1048（续命）
  - 前置依赖：plan-meridian-severed-v1 ⬜（强依赖）+ plan-alchemy-v1 ✅ + plan-social-v1 ✅ + plan-multi-style-v1 ✅ + plan-npc-ai-v1 ✅ + plan-skill-v1 ✅ + plan-qi-physics-v1 ✅
  - 6 个开放问题待 P0 决策门收口

---

## Finish Evidence（待填）

迁入 `finished_plans/` 前必须填：
- **落地清单**：`server/src/yidao/` 主模块（6 招式 + 医者 NPC AI）/ `SkillId::YIDAO` / 平和色 PracticeLog hook / `peoples-medicine-0001` library 条目 / client 接经动画 + dialog UI
- **关键 commit**：P0/P1/P2/P3 各自 hash + 日期 + 一句话
- **测试结果**：`cargo test yidao::*` 数量 / 6 招式覆盖 / 医者 NPC AI 测试 / WSLg 实测接经仪式动画
- **跨仓库核验**：server `yidao::*` / agent `bong:yidao/*` narration 6 类事件 / client 接经动画 + NPC dialog + 死脉 HUD
- **遗留 / 后续**：plan-yidao-v2（毒手医 / 兽医 / 道伥医 等亚流派）/ 排异成本- 完整接入 qi_physics（视 plan-qi-physics-patch-v1 P3 进度）/ 「神视观察」平和色远距感知（worldview §六:染色规则 未来开放）
