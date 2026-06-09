# plan-gathering-tool-bind-v1 — 草药捆保鲜挂载 + 草镰采集本职接通(骨架)

> 一句话:僵尸物品审计的两件"临门一脚"适配——herb_bundle(草药捆)挂上已存在的 shelflife profile,cao_lian(草镰)成为割手草本的 required_tool,补齐采集本职闭环。
>
> 来源:材料断链调查 workflow 2026-06-10(opus 抽查 5/5 证据属实);用户授权自治裁决:「该做适配的适配」。删除类 9 件见 [[plan-economy-zombie-cleanup-v1]] P3。

**依赖**:无。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | herb_bundle:去重配方 + shelflife 挂载 | ⬜ |
| P1 | cao_lian:required_tool 接通(割手草本) | ⬜ |

---

## 接入面(防孤岛 checklist)

- **进料**:
  - shelflife:`shelflife/registry.rs:208-216` `fresh_herb_v1` profile **已存在**,herb_bundle 只差模板字段挂载
  - 配方:`workbench_recipes.rs:386`(`workbench.process.herb_bundle`,time=10)与 `:1812`(同 id,time=200)双处定义——🚩调查红旗 R1。`CraftRegistry::register()` 对重复 id 返回 Err(不 panic 不覆盖,Pi review 2026-06-10 核实),即 :1812 注册静默失败、生效的是 :386;问题本质是**配方 time 与设计/测试预期不一致**,任何动 herb_bundle 前先删掉失效的一处并收口预期值
  - 工具:`tools/kinds.rs:7,19,44` `ToolKind::CaoLian` 已注册(战斗兜底 1.10x),forge/workbench 双路可造
  - required_tool 机制:`botany/registry.rs:238-241` `HarvestHazard::WoundOnBareHand { wound, required_tool }`(5 株已用:DunQiJia×2/GuaDao×2/BingJiaShouTao×1)+ `botany/harvest.rs:533-544` `required_tool_for()`(耐久消耗+受伤判定)
- **出料**:草药捆进保鲜循环(批量存放减损耗);草镰成为割手草本的安全采集工具(徒手采=Laceration 受伤,持镰=免伤+耐久消耗)
- **共享类型 / event**:全部复用 shelflife profile / HarvestHazard / ToolKind,零新枚举零新系统
- **跨仓库契约**:纯 server(TOML + registry 常量);client 无改动(受伤/耐久 HUD 已有通道)
- **worldview 锚点**:§十 资源与匮乏(草药保鲜与采集风险);shelflife 体系正典(plan-shelflife-v1,finished)
- **qi_physics 锚点**:无。保鲜衰减走 shelflife 既有 profile(其底层已对齐 qi_physics),本 plan 只挂载不调参。

---

## P0 — herb_bundle 去重 + shelflife 挂载

- **先去重**(红旗 R1):确认 `CraftRegistry::register()` 对重复 id 的行为,删 `workbench_recipes.rs:1812` 一侧(保留 :386 的 time=10 设计值,或按实际意图收口,见 §8 #1)
- `workbench_materials.toml:153-161` herb_bundle 加 shelflife 字段(挂 `fresh_herb_v1`,或派生 bundle 专属慢速 profile,见 §8 #2)
- 测试:重复 id 行为 pin 测试(去重后 registry 单一命中);herb_bundle 实例随时间衰减曲线;捆 vs 单株衰减对照;过期行为(腐坏产物)分支

## P1 — cao_lian required_tool 接通

- 给 1-2 株"丛生/锐叶割手"类草本(候选:具备 v2 spec 且 required_tool=None 的丛生草本;**不选 spirit_grass**——最基础灵草不应设工具门槛,registry.rs:668-678 其 v2=None 维持)加 `HarvestHazard::WoundOnBareHand { wound: Laceration, required_tool: Some(ToolKind::CaoLian) }`
- 兼容性(已核验):required_tool(ToolKind 系统,受伤/耐久)与 gather_time(GatheringToolSpec 系统,速度/品质)**互不影响**;其他植物 required_tool 仍 None,徒手流程不破
- **不做**速度/品质加成(GATHERING_TOOL_SPECS 加 Sickle 变体)——与 bao_chu 已覆盖 Herb 目标的定位冲突需重新平衡,留作后续(见 §8 #3)
- 视听:持镰收割 SFX `block.grass.break`(pitch 0.8,vol 0.9)+ 草屑横扫粒子(BongSpriteParticle burst 8 颗,沿挥镰弧线,lifetime 8t,#7FA86A);徒手采割手瞬间 SFX `entity.player.hurt`(vol 0.5)+ 细红痕粒子(burst 3 颗 #C04848)+ HUD 事件流「叶缘割手」
- 测试:持镰免伤+耐久递减 / 徒手 Laceration 命中 / 镰耐久归零后等同徒手 / 目标植物外徒手不受伤(回归);每株候选专属用例

---

## §8 开放问题(P0 决策门前需收口)

1. **herb_bundle 配方真实意图**:time=10(:386)还是 time=200(:1812)?去重保留哪侧(建议 10s——捆扎是轻加工)
2. **bundle 保鲜倍率**:直接挂 fresh_herb_v1 还是派生 `bundled_herb_v1`(衰减减半,体现"批量存放减损耗"的设计语义)——倾向后者;plan-shelflife-v1 已归档无主,新 profile 数值**本 plan 内自决**(registry.rs 加一条 profile 常量,沿用 fresh_herb_v1 结构减半)
3. **草镰速度/品质加成**:本 plan 不做;若将来做,需 GatheringToolKind 加 Sickle 变体并与 bao_chu 重新平衡(登记待办)
4. **目标植物名单**:候选丛生草本的最终 1-2 株(实施前 grep v2 spec 现状定名单)
