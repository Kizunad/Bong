# Bong · plan-meridian-severed-v1 · 骨架

经脉永久 SEVERED 通用底盘 —— 把 zhenmai-v2 ⑤ 绝脉断链私有的 `MeridianSeveredVoluntary` component 提取为 **`MeridianSeveredPermanent` 通用受伤类型**，统一处理所有 SEVERED 来源（主动断脉 / 反噬累积 / 过载撕裂 / 战场重伤 / 渡劫失败 / 阴诡色形貌异化）+ 建立 **招式依赖经脉强约束**（所有 SkillRegistry 注册必须声明依赖经脉，cast 前统一检查）+ inspect 经脉图可视化 SEVERED 状态 + 接经术恢复路径（**医者 NPC 服务**，由 plan-yidao-v1 🆕 实装，**为后续医术功法铺垫**）。底盘 plan，所有流派 v2 plan 实装时必须守此约束，是**类似 docs/CLAUDE.md 防孤岛的底盘强约束规则**。

**世界观锚点**：`worldview.md §四:280-307 经脉损伤 4 档（INTACT/MICRO_TEAR/TORN/SEVERED）+ 流量公式`· `§四:286 SEVERED = 该经脉承载流派效果废 + "断了肺经的飞剑手就废了"正典`· `§四:354 过载撕裂物理`· `§六:600-602 已通经脉不可关闭但可受伤变短板`· `§六:617 医道流派 + 平和色`（接经术物理依据 + 后续 yidao 锚点）· `§十一:947-970 NPC 信誉度系统`（医者 NPC 长期医患关系）· `§十二:1043 续命路径存在但有代价`· `§十六.三 上古遗物脆化`（接经术备选 PvE 路径）

**library 锚点**：`cultivation-0006 经脉浅述`（经脉拓扑教材，玩家自学材料）· 待补 `peoples-medicine-0001 医者百态`（plan-yidao-v1 配合补 library 条目）

**前置依赖**：

- `plan-cultivation-canonical-align-v1` ✅ → 经脉拓扑 + Realm + 12 正经 / 8 奇经基础
- `plan-skill-v1` ✅ + `plan-hotbar-modify-v1` ✅ → SkillRegistry / SkillSet 框架
- `plan-multi-style-v1` ✅ → 招式注册扩展（每招声明依赖经脉）
- `plan-combat-no_ui` ✅ → 战场战伤经脉损伤 4 档已部分实装，需扩展永久 SEVERED 写入

**反向被依赖**（强约束 — 所有招式 plan 实装时必须遵守）：

- `plan-woliu-v2` ⬜ → 反噬阶梯 SEVERED 走 MeridianSeveredEvent
- `plan-dugu-v2` ⬜ → 阴诡色 90%+ 形貌异化触发自身经脉 SEVERED + 招式依赖经脉声明
- `plan-tuike-v2` ⬜ → 招式依赖经脉声明（手三阴全）
- `plan-zhenmai-v2` ⬜ → ⑤ 绝脉断链 emit MeridianSeveredEvent + private component 迁出为通用
- 未来 `plan-baomai-v3` / `plan-anqi-v2` / `plan-zhenfa-v2` ⬜
- `plan-yidao-v1` 🆕 → 接经术 / 排异加速 / 自疗 / 续命术等（**为后续医术功法铺垫**）
- `plan-tsy-loot-v1` ✅ → 备选：上古接经术残卷作为 PvE jackpot（worldview §十六.三）
- `plan-multi-life-v1` ⏳ → SEVERED 跨周目不继承（新角色经脉重置）
- `plan-narrative-political-v1` ✅ active → 高境玩家求医接经术的江湖传闻

---

## 现状对齐（2026-05-08 升 active 时核验）

> 草稿先于 burst_meridian 调研；实际代码部分先行实装，本节锁定差异以便 P0 决策门收口。

### 命名差异：MeridianId 实际命名 ≠ 草稿缩写

`server/src/cultivation/components.rs:49-72` 已实装 20 经脉 enum，**用全称命名**而非草稿的 `LU/HT/PC/...` 两字母缩写：

| 草稿缩写 | 实际命名 | 草稿缩写 | 实际命名 |
|---|---|---|---|
| LU | `Lung` | LR | `Liver` |
| LI | `LargeIntestine` | GB | `Gallbladder` |
| HT | `Heart` | REN | `Ren` |
| SI | `SmallIntestine` | DU | `Du` |
| PC | `Pericardium` | CHONG | `Chong` |
| TE | `TripleEnergizer` | DAI | `Dai` |
| SP | `Spleen` | YINWEI | `YinWei` |
| ST | `Stomach` | YANGWEI | `YangWei` |
| KI | `Kidney` | YINQIAO | `YinQiao` |
| BL | `Bladder` | YANGQIAO | `YangQiao` |

§2 / §3 / §4 涉及代码 symbol 的位置一律按**实名**写。`MeridianId::REGULAR`（12 正经）+ `MeridianId::EXTRAORDINARY`（8 奇经）常量已存，P1 直接复用。

### 已部分实装（先于本 plan 落地）

| 接入面声明 | 代码现状 | 本 plan 处置 |
|---|---|---|
| `MeridianSystem` component（20 经脉） | ✅ `cultivation::components::MeridianSystem`（regular[12] + extraordinary[8]） | P1 直接读，不重建 |
| `Meridian` struct（含 `integrity: f64 / cracks: Vec<MeridianCrack>`） | ✅ `cultivation::components::Meridian`（line 119-131） | **现状用 `cracks + integrity` 表达 4 档损伤的连续值**——草稿设计独立 `INTACT/MICRO_TEAR/TORN/SEVERED` enum 与现状不同，P0 决策门 #5（新增）：① 由 cracks 派生 SEVERED 状态（`integrity ≤ 0` ⇒ SEVERED） vs ② 独立 `MeridianSeveredPermanent` component 与 cracks 共存（草稿原方案，长期记忆持久化更清晰） |
| `CrackCause` enum | ✅ 现 4 类：`Overload / Attack / Backfire / ForgeFailure`（line 143-149） | 草稿 `SeveredSource` 设 7 类（VoluntarySever / BackfireOverload / OverloadTear / CombatWound / TribulationFail / DuguDistortion / Other）—— P1 扩 `CrackCause` 加 3 类（VoluntarySever / TribulationFail / DuguDistortion）；BackfireOverload ≈ Backfire / OverloadTear ≈ Overload / CombatWound ≈ Attack |
| `CastRejectReason::MeridianSevered` | ✅ `cultivation::skill_registry.rs:20` 已实装 | 招式 cast 检查接口已就位，本 plan §3 强约束直接挂 |
| `cultivation::burst_meridian` SEVERED 拒绝 | ✅ `burst_meridian.rs:102 / 106` 已用，测试 `beng_quan_rejects_all_right_arm_meridians_severed` 已存 | 体修招式已**事实上**遵守"经脉 SEVERED → cast 失败"约束，本 plan 是把这个**散落实装提取为通用 trait**（`Skill::dependencies()`） |
| `Skill::dependencies()` trait 接口 | ❌ 未实装（招式各自硬编 SEVERED 检查） | P0 决策门 #3（已存）：依赖经脉清单粒度——粗粒度（流派级）vs 细粒度（每招），决定 trait 形态 |
| `MeridianSeveredEvent { entity, meridian_id, source }` | ❌ 未实装 | P1 新建 |
| `qi_physics::field::sever_meridian` 算子 | ❌ 未实装 | P1 走 plan-qi-physics-patch-v1 P3 加新算子（草稿头部已点） |
| inspect 经脉图 SEVERED 染色 | 🟡 inspect 经脉图框架由 plan-cultivation-canonical-align-v1 ✅ 已搭，染色扩展由 P2 加 | client 仅扩颜色映射，无新 UI 框架 |
| hotbar 招式灰显 | 🟡 hotbar 框架由 plan-hotbar-modify-v1 ✅ 已搭 | client 仅扩 tooltip + 灰显逻辑 |

### P0 决策门补充（草稿 §8 之外）

- **#5 SEVERED 状态表达**：① cracks 派生（`Meridian.integrity ≤ ε` ⇒ SEVERED） vs ② 独立 `MeridianSeveredPermanent` component（与 cracks 共存，永久标记） vs ③ 在 `Meridian` 加 `severed: bool` 字段
  - 默认推 ②（草稿原方案）—— 永久 SEVERED 是"出 cracks 列表的最终状态"，独立 component 持久化语义更清晰，且能记录 `severed_at: HashMap<MeridianId, (tick, SeveredSource)>`（草稿 §2 已设计）
  - 反对意见：① 派生最少改动，但语义模糊（暂时性 cracks 与永久 SEVERED 都靠 integrity 数值区分）

---

## 接入面 Checklist

- **进料**：`cultivation::Cultivation { meridian_system }` / `cultivation::MeridianSystem`（12 正经 + 8 奇经状态）/ `SkillRegistry` / `SkillSet` / `combat::WoundEvent`（战场重伤来源）/ `qi_physics::field::sever_meridian`（patch P3 加新算子）
- **出料**：
  - `MeridianSeveredPermanent` component（永久标记 + 跨 server restart 持久化 + 不跨周目）
  - `MeridianSeveredEvent { entity, meridian_id, source: SeveredSource }` 通用 event（所有 SEVERED 来源统一 emit）
  - `Skill::dependencies(): Vec<MeridianId>` 接口扩展（每招声明依赖经脉）
  - cast 检查 system：`check_meridian_dependencies(skill, caster)` → `Result<(), CastRejectReason::MeridianSevered(meridian_id)>`
  - HUD 反馈：依赖经脉 SEVERED 时 hotbar 灰显 + tooltip 显「依赖 X 经脉，已断」
  - inspect 经脉图染色：SEVERED 显**黑色**（区别 INTACT 绿 / MICRO_TEAR 黄 / TORN 橙）
- **共享类型**：`SeveredSource` enum（VoluntarySever / BackfireOverload / OverloadTear / CombatWound / TribulationFail / DuguDistortion / Other）+ `MeridianId` enum（手三阴/三阳/足三阴/三阳/任督共 20 条）
- **跨仓库契约**：
  - server: `cultivation::meridian::severed::*` 主实装 / `combat::skill_check::dependencies`
  - agent: `tiandao::meridian_severed_runtime`（SEVERED 触发 narration + 接经术求医叙事 + 化虚断脉江湖传闻）
  - client: inspect 经脉图染色更新 + hotbar 灰显 + 接经术 NPC 交互 UI（part of plan-yidao-v1）
- **worldview 锚点**：见头部
- **qi_physics 锚点**：SEVERED 写入走 `qi_physics::field::sever_meridian`（patch P3 加）；worldview §二 守恒——SEVERED 不影响 zone qi 总量（仅是经脉传导通路断绝，不消耗灵气）

---

## §0 设计轴心

- [ ] **SEVERED 是通用受伤类型，不是流派专属**（worldview §四:280-307 4 档损伤已正典）：
  ```
  INTACT       (1.0 流量)  ← 默认状态
  MICRO_TEAR   (0.85)      ← 短期可恢复（5min 静坐）
  TORN         (0.5)       ← 中期可恢复（30min 凝脉散内服）
  SEVERED      (0.0)       ← **永久不可逆，需医者接经术**
  ```
  SEVERED 一旦发生，**该经脉承载的流派效果全废**（worldview §四:286）。多个流派的多招式可能同时受影响

- [ ] **永久 + 跨周目重置**（worldview §十二 多周目 + plan-multi-life-v1）：
  - 同角色永久不可逆（除非接经术 + 医者 + 高代价）
  - 新角色（多周目）经脉全 INTACT 重新开始（生平卷可读，但身体不继承）
  - SEVERED 跟生平卷一起入亡者博物馆 → 后人可读到「某某于某战中断手三阳膀胱经，从此失去爆脉之力」

- [ ] **招式依赖经脉强约束（CLAUDE.md 风格规则）**：本节是 §3 强约束源头，所有招式 plan 必守。任一依赖经脉 SEVERED → cast 失败 + HUD 灰显。**未来所有流派 plan 实装时必须在 SkillRegistry 注册时调 `.with_dependencies(meridian_ids)`**

- [ ] **接经术是社交服务，不是 PvE jackpot**（worldview §十一 NPC 信誉度 + §六:617 医道）：区别于 worldview §十六.三 上古遗物脆化路径（备选）。主路径是**医者 NPC 长期医患关系**：
  - 医者 NPC 境界决定接经成功率（醒灵医者风险高 / 化虚医者贵但稳）
  - 玩家付骨币 + 信誉度 + 跑腿任务 → 医者尝试接经
  - 失败 → 该经脉永久 SEVERED 升级（无法再尝试），或玩家额外受伤
  - **plan-yidao-v1 🆕 实装具体接经术招式 + 医者 NPC**

- [ ] **inspect UI 可视化**：玩家必须能在 inspect 经脉图（plan-cultivation-canonical-align-v1 ✅ 已实装）一眼看出哪条 SEVERED + 哪招式因此废。这是 worldview §四:286 「断了肺经的飞剑手就废了」的物理可见性化身

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | 决策门：MeridianSeveredPermanent 数据模型定稿 + Skill::dependencies 接口 + 7 流派依赖经脉清单（§3 强约束）+ SeveredSource 7 类来源枚举 + cast 检查接口 design + 接经术与 plan-yidao-v1 协议（接口签名）| 数据模型 + 接口 + 强约束清单落 plan §2-§3 |
| **P1** ⬜ | server `cultivation::meridian::severed::*` 主模块 + MeridianSeveredPermanent component + MeridianSeveredEvent + cast 检查 system + 7 类 SEVERED 来源接入（zhenmai-v2 ⑤ 迁入 + woliu/dugu/baomai 反噬接入 + combat 战伤接入 + tribulation 渡劫失败接入 + dugu 阴诡色形貌异化接入）+ 跨周目重置 + 跨 server restart 持久化 + ≥40 单测 | `cargo test cultivation::meridian::severed` 全过 / 7 类来源覆盖 / 跨 server restart + 跨周目持久化测试 |
| **P2** ⬜ | client inspect 经脉图染色（SEVERED 黑色）+ hotbar 招式灰显（依赖经脉 SEVERED 时）+ tooltip「依赖 X 经脉，已断」+ 接经术求医 NPC 交互 UI（part of plan-yidao-v1，本 plan 留接口） | WSLg 实跑 inspect 切到经脉图看 SEVERED 黑色 / hotbar 招式灰显验证 |
| **P3** ⬜ | agent narration（SEVERED 触发即时叙事 + 接经术求医叙事 + 化虚断脉江湖传闻）+ 跟 plan-yidao-v1 联调（医者 NPC 接经术成功 / 失败 narration）+ 跟 plan-narrative-political-v1 联调（化虚级 SEVERED 江湖传闻）| narration-eval ✅ 7 类 SEVERED 来源 + 接经术叙事 全过古意检测 |

---

## §2 数据模型

```rust
#[derive(Component)]
pub struct MeridianSeveredPermanent {
    pub severed_meridians: HashSet<MeridianId>,
    pub severed_at: HashMap<MeridianId, (u64 /* tick */, SeveredSource)>,
}

#[derive(Event)]
pub struct MeridianSeveredEvent {
    pub entity: Entity,
    pub meridian_id: MeridianId,
    pub source: SeveredSource,
}

pub enum SeveredSource {
    VoluntarySever,        // zhenmai ⑤ 绝脉断链
    BackfireOverload,      // woliu/dugu/zhenmai/baomai 反噬累积超阈值
    OverloadTear,          // worldview §四:354 过载撕裂强行调动
    CombatWound,           // 战场被打 SEVERED（worldview §四:283-307）
    TribulationFail,       // 渡劫失败爆脉降境（worldview §三:124）
    DuguDistortion,        // dugu 阴诡色 90%+ 形貌异化 → 自身经脉慢性侵蚀
    Other(String),         // 扩展性
}

// MeridianId 已由 server/src/cultivation/components.rs:49-72 实装；
// 本 plan 直接复用，不重新声明。20 变体（12 正经 + 8 奇经）：
pub enum MeridianId {
    // 12 正经
    Lung,            // 手太阴肺
    LargeIntestine,  // 手阳明大肠
    Stomach,         // 足阳明胃
    Spleen,          // 足太阴脾
    Heart,           // 手少阴心
    SmallIntestine,  // 手太阳小肠
    Bladder,         // 足太阳膀胱
    Kidney,          // 足少阴肾
    Pericardium,     // 手厥阴心包
    TripleEnergizer, // 手少阳三焦
    Gallbladder,     // 足少阳胆
    Liver,           // 足厥阴肝
    // 8 奇经
    Ren, Du, Chong, Dai,
    YinQiao, YangQiao, YinWei, YangWei,
}

// SkillRegistry 扩展接口
pub trait Skill {
    fn dependencies(&self) -> Vec<MeridianId>;
    // ...
}

// cast 前检查
pub fn check_meridian_dependencies(
    skill: &dyn Skill,
    caster: Entity,
    severed: &MeridianSeveredPermanent,
) -> Result<(), CastRejectReason> {
    for meridian in skill.dependencies() {
        if severed.severed_meridians.contains(&meridian) {
            return Err(CastRejectReason::MeridianSevered(meridian));
        }
    }
    Ok(())
}
```

---

## §3 招式依赖经脉强约束（CLAUDE.md 风格规则）

> **本节是 v2 流派 plan 必守的底盘约束**。任何 SkillRegistry 注册必须调 `.with_dependencies(meridian_ids)`。漏写 = 红旗，必查本 plan。

### 7 流派依赖经脉清单

基于 worldview §六:583-599 经脉路径与真元属性。每流派列**核心依赖经脉**（任一 SEVERED → 该流派招式效率受影响或废）：

> 经脉名按 `MeridianId` 实名（`Lung/LargeIntestine/Heart/...`）。

| 流派 | 核心依赖经脉 | SEVERED 单条后果（举例）|
|---|---|---|
| **体修·爆脉**（baomai）| 手三阳全（`LargeIntestine` / `SmallIntestine` / `TripleEnergizer`）+ 任督（`Ren` / `Du`）| 任督断 → 全力一击废 / 手三阳任一断 → 崩拳威力 ×0.5（已部分实装于 `burst_meridian.rs:102/106`，beng_quan 测试 `beng_quan_rejects_all_right_arm_meridians_severed` 已绿）|
| **器修·暗器**（anqi）| 手三阴全（`Lung` / `Heart` / `Pericardium`）| `Lung` 断 → 飞剑废 / `Heart`/`Pericardium` 断 → 暗器封灵效率 ×0.3 |
| **地师·阵法**（zhenfa）| 任督 + 足三阴肾经（`Kidney`）| 任督断 → 阵法预埋成功率 ×0.3 / `Kidney` 断 → 真元封入方块失败 |
| **毒蛊**（dugu）| 足三阴全（`Spleen` / `Kidney` / `Liver`）+ 手三阴 `Lung`（飞针）| 足三阴任一断 → 自蕴失败率激增 / `Lung` 断 → 蚀针射程崩 |
| **截脉·震爆**（zhenmai）| 手三阴 `Lung` + 手三阳 `LargeIntestine`（接触反震协调）| 任一断 → ① 弹反 K_drain ×0.5 |
| **替尸·蜕壳**（tuike）| 手三阴全（御物-伪皮）| 手三阴任一断 → 着壳维持 qi/s ×3 |
| **涡流·绝灵**（woliu）| 任督 + 手少阴心经（`Heart`，流速控制）| 任督断 → 持涡 Δ ×0.5 / `Heart` 断 → 涡口吸取率 ×0.3 |
| **医道**（yidao，支援）| `Lung` + `LargeIntestine` + `Heart` + `Du`（化虚级群体接经）| `Lung` 断 → 接经 cast 失败 / `Du` 断 → 群体接经废（化虚专属）|

### 注册时声明范例

```rust
// 例：plan-zhenmai-v2 内
registry.register(SkillBuilder::new("zhenmai.parry")
    .resolve_fn(cast_parry)
    .dependencies(vec![MeridianId::Lung, MeridianId::LargeIntestine])  // ★ 强约束
    .build());
```

### 强约束规则

1. **所有招式 plan**（v2 / v3 / 未来）注册必须声明 `.dependencies(...)`
2. **不声明 = 红旗**，docs/CLAUDE.md 应加一条红旗：「招式注册不声明依赖经脉 → 必查 plan-meridian-severed-v1」
3. **依赖经脉清单可在 plan P0 决策门时细化**（本表是粗粒度参考，具体依赖每招可不同）
4. **多个招式可共享同一经脉**（如 zhenmai ① ②③④ 都依赖 LU）—— LU 断 → 4 招同时废
5. **cast 检查由通用 system 处理**，各流派招式 fn 不需自己写检查代码

---

## §4 SEVERED 7 类来源详细

| # | 来源 | 触发条件 | 由哪个 plan 实装 |
|---|---|---|---|
| 1 | VoluntarySever | zhenmai ⑤ 绝脉断链主动 cast | plan-zhenmai-v2 |
| 2 | BackfireOverload | woliu/dugu/zhenmai/baomai 反噬累积超阈值 | 各流派 v2 plan |
| 3 | OverloadTear | worldview §四:354 强行调动超流量真元（爆脉一次性） | plan-baomai-v3 |
| 4 | CombatWound | 战场被打经脉损伤累积 INTACT → MICRO_TEAR → TORN → SEVERED | plan-combat-no_ui ✅（已部分实装 4 档损伤） |
| 5 | TribulationFail | 渡劫失败爆脉降境（worldview §三:124-131 + §十二:316） | plan-tribulation-v1 |
| 6 | DuguDistortion | dugu 阴诡色 90%+ 形貌异化 → 自身经脉慢性侵蚀（worldview §六:621） | plan-dugu-v2 |
| 7 | Other | 扩展（未来未预见） | — |

每类来源 emit `MeridianSeveredEvent { entity, meridian_id, source }` → 由本 plan 统一处理 + 写入 MeridianSeveredPermanent + agent narration

---

## §5 接经术恢复路径（医者 NPC 服务）

worldview §六:617 医道流派 + 平和色 + §十一 NPC 信誉度系统 + §十二:1043 续命路径。

**主路径：医者 NPC 长期医患关系**（plan-yidao-v1 🆕 实装）：

```
玩家寻医 → 医者 NPC dialog → 评估玩家境界 / 经脉数 / SEVERED 经脉
        → 报价（骨币 + 信誉度 + 跑腿任务）
        → 接经仪式（医者 cast 接经术招式，本 plan 提供接口）
        → roll 成功率 = f(医者境界, 玩家气运, 经脉位置, 已 SEVERED 时长)
        → 成功：MeridianSeveredPermanent 该经脉移除（INTACT 恢复）+ 医者信誉度 +5
        → 失败：经脉永久 SEVERED 升级（更深损伤，无法再尝试）+ 玩家额外受伤
```

**医者 NPC 分级**（worldview §十一 NPC 反应分级）：
- 醒灵医者：风险高 / 价低 / 仅恢复手三阴/三阳
- 凝脉-固元医者：中等 / 主流交易点
- 通灵-化虚医者：稀有 / 高价 / 唯一可恢复任督的（但仍有失败概率）

**备选 PvE 路径**（worldview §十六.三 上古遗物脆化）：
- 上古接经术残卷（坍缩渊深层 jackpot，plan-tsy-loot-v1 ✅）
- 一次性使用即碎，但**不需医者 + 自动成功**
- 极稀有，化虚级以上玩家偶尔遇到

**plan-yidao-v1 🆕 实装清单**：
- 接经术招式（医者 cast）
- 排异加速招式（中和效率比 zhenmai ② 局部中和高 ×3）
- 自疗 / 疗他人招式
- 续命丹（跟 plan-alchemy-v1 ✅ 配合）
- 急救（HP 出血止血）
- 平和色染色加成（worldview §六:617）

---

## §6 客户端新建资产

| 类别 | ID | 优先级 | 备注 |
|---|---|---|---|
| UI | inspect 经脉图扩展 | P2 | SEVERED 经脉显黑色 + 悬停显「永久断绝，需医者接经术」 |
| UI | hotbar 招式灰显 | P2 | 依赖经脉 SEVERED 时该招式灰显 + tooltip 解释 |
| UI | 接经术求医 NPC dialog | P2 | part of plan-yidao-v1，本 plan 留接口 |
| 粒子 | `MERIDIAN_SEVER_FLASH` | P2 | 经脉断绝瞬间金光 + 经脉位置短暂血雾（5-10s 散尽）|
| 音效 | `meridian_sever_crack` | P3 | layers: `[{ sound: "block.bone_block.break", pitch: 0.7, volume: 0.7 }]`（清脆裂声）|
| HUD | `SeveredMeridianListHud` | P2 | 永久 SEVERED 经脉列表（玩家长期身体记录，警示性）|

**无独立动画** —— SEVERED 触发时由各 plan 自己负责（zhenmai ⑤ 主动断脉有专属动画 / 战伤 SEVERED 没有动画 / 反噬累积 SEVERED 有反噬动画）

---

## §7 测试矩阵（饱和化）

下限 **40 单测**：

| 模块 | 测试组 | 下限 |
|---|---|---|
| `MeridianSeveredPermanent` | 写入 / 重复 SEVERED 同一经脉 / 跨 server restart 持久化 / 跨周目重置（plan-multi-life-v1 联调）| 8 |
| `check_meridian_dependencies` | 依赖经脉 INTACT 通过 / SEVERED 拒绝 / 多依赖任一 SEVERED 拒绝 / 招式无依赖通过 | 8 |
| `MeridianSeveredEvent` | 7 类来源 emit + 写入 component + 通知 inspect UI | 10 |
| `severed_persistence` | 跨 server restart + 跨周目（多角色）+ agent narration 触发 | 6 |
| `acupoint_repair`（接经术接口）| 成功恢复 INTACT + 失败升级损伤 + plan-yidao-v1 联调（接口契约测试）| 8 |

**P1 验收**：`grep -rcE '#\[test\]' server/src/cultivation/meridian/severed/` ≥ 40。

---

## §8 开放问题 / 决策门

### #1 SEVERED 跨周目处理

- **A**：完全不继承（新角色经脉全 INTACT，符合 worldview §十二 多周目）
- **B**：写入生平卷供后人查阅，但新角色经脉全 INTACT（推荐）
- **C**：部分继承（如任督 SEVERED 跨周目继承，正经不继承）

**默认推 B** —— 既符合多周目重生设定，又保留"道统记忆"价值

### #2 接经术失败升级损伤的具体机制

- **A**：SEVERED 升级为「死脉」（无法再尝试接经，仅可走上古残卷）
- **B**：玩家额外失血 + 多 SEVERED 一条经脉（连带损伤）
- **C**：A + B 组合

**默认推 A** —— 简洁，且与 worldview §十二 死亡是学费一致

### #3 招式依赖经脉清单粒度

§3 给的是粗粒度（流派级别）。是否需要每招细化？

- **A**：每招独立声明依赖（细粒度，工作量大）
- **B**：流派级别共享依赖（粗粒度，简单）
- **C**：混合（核心招细粒度，辅招流派共享）

**默认推 C** —— 平衡精度和工作量，留各 v2 plan P0 自行决定

### #4 是否在 docs/CLAUDE.md §四 红旗加一条「招式注册不声明依赖经脉」

- **A**：加（强约束化）
- **B**：仅在本 plan §3 内强约束（限定 plan 内）

**默认推 A** —— 跟 qi_physics 红旗一致格调，是底盘约束应升级到项目级

---

## §9 进度日志

- **2026-05-08** 升 active。实地核验 `server/src/cultivation/components.rs:49-72`（MeridianId enum） + `Meridian.cracks/integrity` + `CrackCause` 4 类 + `CastRejectReason::MeridianSevered` + `burst_meridian.rs:102/106` 拒绝路径已实装；草稿命名差异（`LU/HT/PC` → `Lung/Heart/Pericardium`）通过"现状对齐"段落锁定；P0 决策门 #5（SEVERED 状态表达：cracks 派生 vs 独立 component）补入 §8。
- **2026-05-06** 骨架立项。源自 plan-zhenmai-v2 ⑤ 绝脉断链私有 component `MeridianSeveredVoluntary` 的提取需求 + 用户拍"SEVERED 应是通用受伤类型，依赖经脉的招式都失效"。
  - 设计轴心：SEVERED 通用受伤类型（worldview §四:280-307 4 档损伤已正典）+ 永久 + 跨周目重置 + 招式依赖经脉强约束（CLAUDE.md 风格规则）+ 接经术医者 NPC 服务（不是 PvE jackpot 主路径）+ inspect UI 可视化
  - 7 流派依赖经脉清单锁定（粗粒度，§3 强约束）—— 各 v2 plan 实装时必守此规则
  - SEVERED 7 类来源枚举（VoluntarySever / BackfireOverload / OverloadTear / CombatWound / TribulationFail / DuguDistortion / Other）+ 各 plan 接入路径明确
  - 接经术主路径 = 医者 NPC（plan-yidao-v1 🆕 实装），备选 PvE 路径 = 上古残卷（plan-tsy-loot-v1）
  - **派生 plan-yidao-v1 🆕**（医道功法，跟 7 战斗流派平行的支援流派）— 接经术 / 排异 / 自疗 / 续命 / 急救 + 平和色染色加成 + 医者 NPC dialog
  - worldview 锚点对齐：§四:280-307 + §四:286 + §四:354 + §六:600-602 + §六:617（yidao 锚点）+ §十一:947-970 + §十二:1043 + §十六.三
  - 反向被依赖：所有 v2 流派 plan + plan-yidao-v1 🆕 + plan-tsy-loot-v1 + plan-multi-life-v1 + plan-narrative-political-v1
  - 待补：reminder.md 登记 plan-yidao-v1 占位 / docs/CLAUDE.md §四 红旗加一条 / 现有 zhenmai-v2 私有 component 迁出为通用（zhenmai-v2 P1 联调时处理）

---

## Finish Evidence（待填）

迁入 `finished_plans/` 前必须填：
- **落地清单**：`server/src/cultivation/meridian/severed/` 主模块 + `combat::skill_check::dependencies` + 7 类 SEVERED 来源接入 + inspect UI 染色 + hotbar 灰显
- **关键 commit**：P0/P1/P2/P3 各自 hash + 日期 + 一句话
- **测试结果**：`cargo test cultivation::meridian::severed` 数量 / 7 类来源接入测试 / 跨 server restart + 跨周目持久化 / WSLg 实测 inspect SEVERED 染色 + hotbar 灰显
- **跨仓库核验**：server 主模块 + 7 类来源接入 / agent narration 7 类 + 接经术求医 / client inspect 经脉图染色 + hotbar 灰显 + plan-yidao-v1 NPC dialog
- **遗留 / 后续**：plan-yidao-v1 完整实装（接经术招式 / 医者 NPC AI / 续命丹 alchemy 联调）/ 其他 v2 流派 plan 招式依赖经脉声明回填（zhenmai/woliu/dugu/tuike + 未来 anqi/zhenfa/baomai-v3）/ 上古接经术残卷 PvE jackpot（plan-tsy-loot-v1 vN+1）

---

## Finish Evidence

**消费时间**：2026-05-08 一次性消费完整 P0 → P3
**消费者**：`/consume-plan meridian-severed-v1`（Claude Code）
**用户拍板**：2026-05-08 全 scope 推 + 决策门 §8 默认（B/A/C/A + #5②）

### P0 决策门拍板（user 2026-05-08）

| # | 主题 | 选项 | 落地 |
|---|---|---|---|
| #1 | 跨周目 SEVERED 处理 | **B**：写入生平卷（plan-life-record 负责），新角色 SEVERED 重置 INTACT | `on_player_terminated` 移除 `MeridianSeveredPermanent` component |
| #2 | 接经术失败升级机制 | **A**：升级为「死脉」（dead_meridians 子集，无法再尝试） | `try_acupoint_repair` Failed 分支写 `dead_meridians` |
| #3 | 招式依赖经脉粒度 | **C**：混合（核心招细粒度 + 辅招流派共享） | 留各 v2 plan P0 自行决定，本 plan 提供通用 `check_meridian_dependencies` 工具 + `SkillMeridianDependencies` 注册表 |
| #4 | docs/CLAUDE.md §四 红旗加一条 | **A**：加 | ⚠️ docs/CLAUDE.md 严禁 `/consume-plan` 自动写入；**待用户手动加红旗**（建议条文："招式注册不声明依赖经脉 → 必查 plan-meridian-severed-v1"） |
| #5 | SEVERED 状态表达 | **②**：独立 `MeridianSeveredPermanent` component 与 `Meridian.cracks/integrity` 共存 | `MeridianSeveredPermanent` 作 ECS Component；`enforce_severed_state` 把 `Meridian.integrity` 钳到 0 + opened=false |

### 落地清单

**P1 — server**：
- `server/src/cultivation/meridian/severed.rs` 主模块（约 480 行实装 + 460 行测试）
  - `MeridianSeveredPermanent` Component（severed_meridians + severed_at + dead_meridians）
  - `SeveredSource` enum 7 类来源
  - `MeridianSeveredEvent` Event
  - `SeveredRecord { at_tick, source }` 快照
  - `check_meridian_dependencies` / `check_meridian_runtime_integrity` 强约束检查
  - `try_acupoint_repair` 接经术接口（plan-yidao-v1 占位）
  - `AcupointRepairOutcome` enum（Restored / Failed / NotSevered / AlreadyDead）
  - `apply_severed_event_system` 事件 → component 写入
  - `meridian_severed_detection_tick` cracks → SEVERED 自动捕获
  - `severed_source_from_crack` CrackCause → SeveredSource 7 映射
  - `enforce_severed_state` 钳零 + opened=false
  - `SkillMeridianDependencies` Resource（招式依赖经脉表）
- `server/src/cultivation/components.rs` `CrackCause` 扩 3 类（VoluntarySever / TribulationFail / DuguDistortion）
- `server/src/cultivation/skill_registry.rs` `CastRejectReason::MeridianSevered` 改为 `MeridianSevered(Option<MeridianId>)` 携带阻断经脉 id
- `server/src/cultivation/burst_meridian.rs` 切换到 `check_meridian_runtime_integrity`，统一 SEVERED 检查路径
- `server/src/cultivation/mod.rs` 注册 events / systems / Resource / 玩家默认挂 component
- `server/src/cultivation/death_hooks.rs` `on_player_terminated` 移除 component（跨周目重置 #1=B）
- `server/src/cultivation/tribulation.rs` `apply_tribulation_failure_penalty` 返回 closed ids → `tribulation_failure_system` / `abort_du_xu_on_client_removed` / `tribulation_escape_boundary_system` 三处 emit `MeridianSeveredEvent { TribulationFail }`
- `server/src/persistence/mod.rs` `persist_player_cultivation_bundle` 签名加 `meridian_severed` + JSON bundle "meridian_severed" 字段（跨 server restart 持久化）
- `server/src/player/mod.rs` 3 处 cultivation_bundle Query 加 `Option<&MeridianSeveredPermanent>` + 持久化调用

**P2 — client**：
- `client/src/main/java/com/bong/client/inventory/model/MeridianBody.java` 加 `severedChannels()` 访问器（hotbar 灰显将来用）
- `client/src/test/java/com/bong/client/network/CultivationDetailHandlerTest.java` 加 2 pin tests（integrity=0 → SEVERED 黑色 0xFF666666 / 0.10 阈值边界）
- `client/src/test/java/com/bong/client/inventory/model/MeridianBodyTest.java` 新增（3 tests）
- inspect 染色 ✅ 复用 plan-cultivation-canonical-align-v1 已铺好的 `damageFromIntegrity(< 0.10) → SEVERED`，零代码改动

**P3 — agent**：
- `agent/packages/schema/src/meridian-severed.ts` `MeridianSeveredEventV1` + `SeveredSource` 双端契约
- `agent/packages/schema/src/channels.ts` 加 `MERIDIAN_SEVERED: "bong:meridian_severed"` + `REDIS_V1_CHANNELS`
- `agent/packages/schema/src/index.ts` 重新导出
- `agent/packages/tiandao/src/meridian-severed-narration.ts` `meridianName` (20 经脉中文名) + `renderSeveredText` (7 来源叙事风格) + `renderMeridianSeveredNarration` (event → Narration)

### 关键 commit

| commit | 日期 | 主题 |
|---|---|---|
| `178452c32` | 2026-05-08 | feat(cultivation): plan-meridian-severed-v1 主模块 cultivation::meridian::severed（44 单测） |
| `fa002ddb9` | 2026-05-08 | feat(cultivation): CastRejectReason::MeridianSevered 携带 MeridianId + CrackCause 扩 3 类 |
| `8ecb614ae` | 2026-05-08 | feat(cultivation): 注册 MeridianSeveredEvent + apply_severed_event_system + 玩家默认挂 component |
| `df3cb1cdd` | 2026-05-08 | feat(persistence): cultivation bundle 加 meridian_severed 字段实现跨 server restart |
| `f7bebbe20` | 2026-05-08 | feat(cultivation): SEVERED 7 类来源接入 + detection system 自动捕获 cracks→SEVERED |
| `ec5a0b1f5` | 2026-05-08 | feat(client): plan-meridian-severed-v1 P2 inspect 染色 + severedChannels 访问器 |
| `df92f0d39` | 2026-05-08 | feat(agent): plan-meridian-severed-v1 P3 narration schema + 7 类来源渲染器 |

### 测试结果

- **server**：
  - `cargo test cultivation::meridian::severed` → **57 passed**（>plan §7 下限 40）
  - `cargo test cultivation::burst_meridian` → 9 passed（含新增 SEVERED component path 测试）
  - `cargo test cultivation::tribulation` → 53 passed（4 个 fled/failed 路径无回归）
  - `cargo test`（全量）→ **2742 passed / 0 failed**
  - `cargo clippy --all-targets -- -D warnings` 干净
  - `cargo fmt --check` 干净
- **client**：
  - `./gradlew test build` → BUILD SUCCESSFUL
  - 新增 2 pin tests + 3 MeridianBody tests
- **agent**：
  - `npm test -w @bong/schema` → **296 passed**
  - `npm test -w @bong/tiandao` → **270 passed**（含新增 14 个 narration tests）

`grep -rcE '#\[test\]' server/src/cultivation/meridian/severed.rs` → **57**

### 跨仓库核验

| 层 | symbol | 路径 |
|---|---|---|
| server | `MeridianSeveredPermanent` / `MeridianSeveredEvent` / `SeveredSource` / `check_meridian_dependencies` / `try_acupoint_repair` / `apply_severed_event_system` / `meridian_severed_detection_tick` / `enforce_severed_state` / `SkillMeridianDependencies` / `AcupointRepairOutcome` | `server/src/cultivation/meridian/severed.rs` |
| server | `CrackCause::VoluntarySever` / `TribulationFail` / `DuguDistortion` | `server/src/cultivation/components.rs` |
| server | `CastRejectReason::MeridianSevered(Option<MeridianId>)` | `server/src/cultivation/skill_registry.rs` |
| server | tribulation 7 类来源 #5 接入 | `server/src/cultivation/tribulation.rs` |
| server | bundle "meridian_severed" 字段 | `server/src/persistence/mod.rs` |
| agent | `MeridianSeveredEventV1` / `SeveredSource` schema | `agent/packages/schema/src/meridian-severed.ts` |
| agent | `CHANNELS.MERIDIAN_SEVERED` | `agent/packages/schema/src/channels.ts` |
| agent | `renderMeridianSeveredNarration` / `renderSeveredText` / `meridianName` | `agent/packages/tiandao/src/meridian-severed-narration.ts` |
| client | `ChannelState.DamageLevel.SEVERED = 0xFF666666` ✅ 复用 plan-cultivation-canonical-align-v1 已实装 | `client/src/main/java/com/bong/client/inventory/model/ChannelState.java` |
| client | `damageFromIntegrity(0.0) → SEVERED` ✅ 复用 | `client/src/main/java/com/bong/client/network/CultivationDetailHandler.java` |
| client | `MeridianBody.severedChannels()` 新增便捷访问器 | `client/src/main/java/com/bong/client/inventory/model/MeridianBody.java` |

### 遗留 / 后续

**已知 [need-WSLg-verify]（云端无法验收）**：
- P2 inspect 经脉图视觉染色（玩家 SEVERED 一条经脉后切到 inspect tab 看黑色显示）—— 单测已锁住 integrity=0 → SEVERED 颜色，但 UI 渲染需 WSLg 实跑
- P2 hotbar 灰显视觉验收 —— 待 v2 流派 plan 在 `SkillMeridianDependencies` 注册依赖经脉 + client schema 推 severed 列表后才有可灰显的具体招式

**已知 [need-LLM-eval]（云端无法验收）**：
- P3 narration 古意检测 —— 单测覆盖锚点关键词 + 现代俚语黑名单，最终古意分数需 `npm run start:mock` 配合 narration-eval 实跑

**待 user 手动落地**（`/consume-plan` docs/ 写权限受限）：
- **决策门 #4 = A**：在 `docs/CLAUDE.md §四 孤岛红旗`末尾加一条：「**招式注册不声明依赖经脉**：新增 SkillBuilder / `register_skills` 调用未在 `SkillMeridianDependencies` 注册依赖经脉的（任一） SEVERED → 招式未被强约束保护 → **必查 `plan-meridian-severed-v1`**」

**依赖未启动 plan**：
- `plan-yidao-v1` 🆕（医道流派）—— 接经术招式 + 医者 NPC AI + 医者 NPC dialog UI + 续命丹 alchemy 联调 + 平和色染色加成。本 plan 已留 `try_acupoint_repair` 接口 + `AcupointRepairOutcome` 4 分支供其调用。
- `plan-zhenmai-v2` ⬜：⑤ 绝脉断链主动 cast → emit `MeridianSeveredEvent { VoluntarySever }`
- `plan-woliu-v2` / `plan-dugu-v2` / `plan-baomai-v3` ⬜：反噬阶梯 / 阴诡色 90%+ 形貌异化 / OverloadTear → 各自 emit
- `plan-tsy-loot-v1` ✅：上古接经术残卷 PvE jackpot 路径，未在本 plan 范围（备选 PvE 路径）
- `plan-multi-life-v1` ⏳：跨周目重置已通过 `on_player_terminated` 接好，但生平卷写入 SEVERED 摘要由 plan-life-record 负责

**SEVERED → client 永久标记 schema 推送**（待 v2 plan 触发时再扩）：
- 当前 client 通过 `damageFromIntegrity(< 0.10) → SEVERED` 显黑，但**无法区分**「瞬时低 integrity」与「永久 SEVERED」。区分需要 server 推 `severed_meridians[]` 给 client（plan-yidao-v1 / 各 v2 流派 plan 实装时统一扩 `cultivation_detail` 或新建 `meridian_severed_state` payload）。
