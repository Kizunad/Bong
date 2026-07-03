# plan-combat-hit-location-v1 — 近战命中部位真实化：废除"恒瞄胸口中心"射线

> **Active**（升 active 2026-07-03，§8 全部开放问题已收口于 §8.1）。一句话：命中部位由攻击者真实瞄准 + 目标几何决定，四肢可中、部位倍率表和腿伤系统终于有戏份；玩家与 NPC 双向同修（这不是 NPC 专属 bug）。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 瞄准射线改造（攻方 Look / NPC 散布） | ⬜ |
| P1 | 部位分布校准 + 消费端补齐（臂伤后果） | ⬜ |
| P2 | 非-raycast 旁路清理（原"硬编 Chest"） | ⬜ |
| P3 | 部位差异视听反馈 | ⬜ |

## 接入面（docs/CLAUDE.md §二）

- **进料**：`combat::events::AttackIntent`（`events.rs:34-45`，现无任何瞄准字段）；玩家 `Look` component（server 端已有，**无需 wire 变更**）；NPC 攻击构造点 `npc/brain/actions_combat.rs:297-306`；玩家攻击构造点 `combat/player_attack.rs:96-105`
- **出料**：`raycast_humanoid`（`combat/raycast.rs:90-108`）产出的 `hit_probe.body_part` → `Wound.location`（`resolve.rs:954/1416/1430/1502`）→ 既有消费端：部位倍率表 `body_part_multipliers`（`resolve.rs:1757-1766`，含 Back 0.9×）、腿伤减速 `movement/leg_wound.rs:13-65`、HUD 人体剪影红点（现成按部位渲染）
- **共享类型**：`BodyPart` 枚举（`combat/components.rs:30-41`）不动；`classify_body_part`（`raycast.rs:45-88`）分类逻辑本身支持四肢，不动阈值只修射线
- **跨仓库契约**：无 wire 变更（伤口 payload 已带 location；client HUD 已消费）
- **worldview 锚点**：§四:215 战力分层"体表、经脉、真元——多血条模型"——体表伤按部位分布是该模型的应有之义；§四:334 拼刺刀近战
- **qi_physics 锚点**：不涉及真元流动，无新增常数
- **正面发现（Explore 核验，2026-07-03）——Look 管线已现成**：`resolve.rs:162` `PositionLookItem = (&Position, Option<&Look>)` + `resolve.rs:240` positions 查询覆盖全体实体（玩家 + NPC）。P0 读攻方 Look **无需新增 SystemParam**——只需在 `resolve.rs:413` 决议前 `positions.get(attacker).1` 取，或扩 `resolve_combat_actor`（`resolve.rs:1849`）的返回值带出。`raycast_humanoid` 全仓唯一生产调用点就是 `resolve.rs:413`，改造面极窄、无 fan-out
- **正面发现——wire + client 零改动**：`CombatBodyPartV1`（`schema/combat_event.rs:5-14`）已含全 8 部位枚举；client `MiniBodyHudPlanner.appendWoundDots`（`:152`/`:214` switch）已实现 `arm_l`/`arm_r`/`leg_l` 分部位红点渲染。四肢命中激活对 client 零改动

## 背景调研结论（2026-07-03，三 agent 并查）

- 命中部位**非随机 roll、非攻方选择**：`raycast_humanoid` 把射线写死瞄准目标 AABB **胸口中心**（X/Z 取包围盒中心、Y 取脚底 + `CHEST_AIM_HEIGHT=1.2`，`raycast.rs:28,90-108`）
- 后果双杀：横向偏移 `lateral` 恒 ~0 → 永达不到手臂阈值 0.18（臂不可达）；命中 y 恒落胸区间 0.55~0.88 → 腿（<0.35）不可达。`resolve.rs:8813-8814` 测试注释白纸黑字承认"无法可靠命中 ArmL/ArmR"
- 玩家与 NPC **完全对称**地坏：三处 `AttackIntent` 构造点字段集一致，都不带视线；集成测试断言双向命中恒 `Chest`（`resolve.rs:3673/3680/3885`）
- 部位倍率表（头 2.0×/臂 0.7×/腿 0.6×，`resolve.rs:1757`）与腿伤减速全接好，只是永远轮不到四肢

## P0 瞄准射线改造 ⬜

- `raycast_humanoid` 改签名：接受攻方瞄准方向（玩家 = `Look` 转向向量，不叠加 jitter，Look 本身已含真实瞄准误差；NPC = 指向目标几何中心 + 确定性散布 jitter，种子 `hash(attacker_id) ^ combat_tick`），替换 `fallback_aim` 恒定中心点
- **jitter σ 起始值**（P0 交付物写死为常量，非最终校准值，决议见 §8.1 #1）：瞄准方向叠加二维高斯角度 jitter，`pitch_sigma_deg ≈ 9.0`、`yaw_sigma_deg ≈ 7.0`（偏置胸心基线；melee ~2m 几何推算：够头需抬 pitch ~11°、够臂需偏 yaw ~5°、够腿需压 pitch ~16°，此组 σ 定性给出"胸多、头腹臂腿有尾巴"的分布形状，逼近目标分布留 P1 直方图校准）；按武器 kind 缩放该 σ：`dagger ×0.85` / `sword ×0.9` / `fist ×1.0` / `spear·staff ×1.1`（reach 越长散布越松）。武器 reach 基准取自 `events.rs:26-32`（`FIST=2.0`/`DAGGER=1.2`/`SWORD=2.0`/`SPEAR=2.6`/`STAFF=2.4`；武器只分拳/近战刃/长杆三档，无独立"枪"类）；真投射暗器/弓走 `carrier.rs`（见 §P2）不经此射线，散布模型不适用
- 瞄准源读取**复用现成 Look 管线，无需新增 SystemParam**：`resolve.rs:162` `PositionLookItem = (&Position, Option<&Look>)` + `resolve.rs:240` positions 查询已覆盖全体实体；`resolve.rs:413` 决议处按 attacker 取 `positions.get(attacker).1`，或扩 `resolve_combat_actor`（`resolve.rs:1849`）返回值带出；`AttackIntent` 本体不动（瞄准在决议端读组件，避免 wire/事件形状变更）
- 测试抓手：`raycast.rs` 部位分布统计测试（固定 seed 批量攻击，Head/Chest/Abdomen/ArmL/ArmR/LegL/LegR 命中率各 >0）；俯视/仰视/侧向命中专项；`resolve.rs:3673/3680/3885` 系列"恒 Chest"断言改为分布断言

## P1 部位分布校准 + 消费端补齐 ⬜

- 散布参数校准到目标分布（P0 已写入 σ 起始值，决议见 §8.1 #1；本阶段用固定 seed 直方图实测校准，允许微调 σ 数值本身，以及视命中数据微调 `classify_body_part` 阈值 0.18/0.30，决议见 §8.1 #4）：正面平视基线约 胸 40-50% / 腹 15% / 头 8-12% / 臂 15-20% / 腿 15-20%
- **臂伤消费端**（调研缺口：腿有 `leg_wound.rs` 减速，臂伤现无 gameplay 后果）：ArmL/ArmR 按伤势分级叠加攻击惩罚 + debuff（分级表见 §8.1 #2，镜像 `wound_severity_to_grade` 模式，落点 `combat/` 新 `arm_wound.rs`，结构照抄 `movement/leg_wound.rs:13-65`）
- 测试抓手：分布 pin 测试（固定 seed 直方图区间断言）；臂伤分级 → 攻击惩罚映射表专属 case

## P2 非-raycast 旁路清理（原"硬编 Chest 六处"，Explore 核实收窄为 3 处真旁路）⬜

- 原骨架列 6 处硬编 Chest，混淆了测试夹具与生产旁路。Explore 核实（2026-07-03）后收窄：
  - **删除**（非生产，测试夹具本身以 Chest 为设计意图，不是待修 bug）：`sword_basics.rs:1474`（`#[test] hit_events_raise_matching_sword_proficiency`）、`lifecycle.rs:2678`（`#[test]` 死亡生命周期）、`lifecycle.rs:4876`（`#[test]` shield-block）。**"剑招按剑轨迹定部位"是伪需求**——剑招本身走 `raycast_humanoid` 决议出的部位，`sword_basics.rs` 生产代码里没有硬编 Chest。
  - **真生产硬编只有 3 处**，且都是**本就不经 `raycast_humanoid` 的独立结算旁路**（投射 / AoE / 反伤各自有独立几何逻辑，不是"漏改的 melee 分支"）：
    1. 剑招招架反伤：`resolve.rs:989`（`Wound.location` 硬编 Chest）
    2. 暗器投射：`carrier.rs:975`（`Wound`）+ `carrier.rs:1003`（`CombatEvent`）成对硬编
    3. 涡流 AoE：`woliu_v2/skills.rs:652`（`Wound`）+ `woliu_v2/skills.rs:671`（`CombatEvent`）成对硬编
- 逐条决断（旁路简化 vs 该改）：
  - 反伤 `resolve.rs:989`：反伤打持盾/持械臂在物理上更合理（挡招的是拿盾那只手）——倾向改，本阶段实测校准后收口，若保留须写理由注释
  - 暗器投射 `carrier.rs:975/1003`：投射命中点应按弹道终点几何算部位（非胸心）——倾向改
  - 涡流 AoE `woliu_v2/skills.rs:652/671`：AoE 命中判定是半径覆盖，"部位"概念本身弱化——倾向保留 Chest，但须写明"AoE 无方向性，Chest 作代表部位"的理由注释
- 每处一条 pin 测试（保留者测"恒 Chest + 注释解释原因"；改掉者测"新判定逻辑输出非 Chest 分布"）

## P3 部位差异视听反馈 ⬜

- HUD：人体剪影红点已按部位渲染（现成，零改动验证即可）
- 命中反馈差异：头部命中 `BongSpriteParticle` 暴击星形 burst ×6、lifetime 8t、白金色 `#FFE9A0`；四肢命中血色 `BongLineParticle` ×3 沿命中法线、lifetime 6t、`#8C1F1F`；音效 audio_recipe：头部 `entity.player.attack.crit`(pitch 1.15) 叠 `entity.arrow.hit_player`(delay 1t)，四肢 `entity.player.attack.weak`(pitch 0.9)
- 腿伤触发减速时目标脚下 `BongGroundDecalParticle` 血渍 decal（复用既有 decal 基类），lifetime 100t
- narration 示例（zone / perception）：「一剑削中持刀的右臂，兵刃当啷落地半寸又被攥紧」「膝弯中箭，那散修的步子瞬间烂了」

## §8 开放问题（升 active / P0 决策门前收口）

> 全部已在 §8.1 收口（#2 用户拍板 2026-07-03；#1/#3/#4/#5 Explore 核验 + 拍板 2026-07-03）。

1. **散布参数与目标分布数值**：jitter 半径/椭圆比、按武器 kind（拳/刀/枪 reach 不同散布不同？）——需实测校准
2. **臂伤 gameplay 后果形态**：攻击惩罚 vs 持械掉落 vs 蓄力时长惩罚；与既有 `MeridianSeveredPermanent`（断脉禁招）的边界
3. **NPC 战术性瞄准**：狼咬腿、鼠袭手等物种偏好是否此 plan 做（与 plan-mundane-fauna-v1 preys_on 联动）还是留给 fauna plan
4. **玩家垂直视角自然涌现**：蹲下打腿/瞄头爆头在真实 Look 射线下应自然可行——需实机验证阈值（`classify_body_part` 0.88/0.55/0.35/0.18）是否要随之微调
5. **Back 部位激活**：`classify_body_part` 永不产出 Back（`resolve.rs:8935-8936` 注释）——背刺方向判定是否顺手补（攻方位于目标背半球 → Back，0.9× 伤害但可叠偷袭系数）

> 全部已在 §8.1 收口，原表留追溯，实施以 §8.1 为准。

## §8.1 决议（pre-P0 收口，2026-07-03）

### #2 臂伤 gameplay 后果形态

**决议**：
1. 主惩罚 = **攻击惩罚**（用户拍板）；不做持械掉落作为常规分级效果（保留给断臂极端级）。
2. 按伤势分级叠加 debuff（镜像 `leg_wound.rs` 的 `wound_severity_to_grade` 五级模式，P1 实施时以此为基线数值，允许 ±20% 校准）：

| 分级 | 攻击伤害 | 追加 debuff |
|------|---------|-------------|
| Bruise 淤伤 | ×0.95 | — |
| Abrasion 擦伤 | ×0.90 | — |
| Laceration 裂伤 | ×0.80 | 攻击冷却 +10%；格挡/招架减伤效果 −20%（格挡走臂） |
| Fracture 骨折 | ×0.60 | 攻击冷却 +25%；格挡减伤 −40%；投射类（凝针/暗器）散布角 +50%；蓄力类（全力一击）蓄力时长 +30% |
| Severed 断臂 | ×0.40 | 该侧手持武器立即脱手落地（走既有 dropped_loot 链）；无法双手持械；施法 cast_ticks +25%（结印手受损） |

- 左右臂区分：主手臂（持械侧）吃攻击/蓄力/散布惩罚，副手臂吃格挡惩罚；双臂皆伤取各自维度最重值，不叠乘。
- 拒绝路线：不做"臂伤禁招"硬门（与 `MeridianSeveredPermanent` 断脉禁招边界划清——经脉断=招式不可用，肉伤=可用但打折）。

**落点**：`combat/` 新建 `arm_wound.rs`（结构照抄 `movement/leg_wound.rs:13-65`）；攻击惩罚挂 `combat/resolve.rs` 伤害结算入口；格挡惩罚挂 shield_block/parry 减伤计算；散布惩罚挂 `anqi_v2` 散布角与 `needle.rs` dir 抖动；plan §P1 交付物按本表展开。

### #1 散布参数与目标分布数值

**决议**：
1. 散布模型：攻方瞄准方向 = 基础瞄准（玩家 `Look` 转向向量；NPC 指向目标几何中心）叠加一个二维高斯角度 jitter（俯仰 pitch、偏航 yaw 独立分量）。NPC 恒定应用 jitter；玩家不叠加（`Look` 已含真实瞄准误差，不再人工加噪）。
2. 起始 σ 值（P0 交付物写死为常量，非最终值，P1 才用直方图校准）：`pitch_sigma_deg ≈ 9.0`、`yaw_sigma_deg ≈ 7.0`，正态偏置胸心基线——melee 距离 ~2m 几何推算：够头需抬 pitch ~11°、够臂需偏 yaw ~5°、够腿需压 pitch ~16°，此组 σ 定性给出"胸多、头腹臂腿有尾巴"的分布形状。按武器 kind 缩放该 σ：`dagger ×0.85` / `sword ×0.9` / `fist ×1.0` / `spear·staff ×1.1`（reach 越长散布越松，符合近身武器更精准的直觉）。武器 reach 基准取自 `events.rs:26-32`（`FIST=2.0`/`DAGGER=1.2`/`SWORD=2.0`/`SPEAR=2.6`/`STAFF=2.4`）；武器只分拳/近战刃/长杆三档，无独立"枪"类。
3. 边界：这组 σ 只是 P0 起始值，不是最终校准结果——目标分布（胸 40-50% / 腹 15% / 头 8-12% / 臂 15-20% / 腿 15-20%）在 P1 用固定 seed 批量直方图 pin 测试校准，P1 阶段调整 σ 数值本身不算范围蔓延（P0 已声明"起始值待校准"）。真投射武器（暗器/弓）不适用此散布模型——它们走 `carrier.rs`（见 §P2），不经 `raycast_humanoid`。

**落点**：`combat/raycast.rs:90-108`（`raycast_humanoid` 改造点，σ 常量新增于此或紧邻处）+ plan §P0（jitter σ 起始值）/ §P1（直方图校准）。

### #3 NPC 战术性瞄准

**决议**：
1. 划非目标——本 plan 不 own"物种攻击偏好"（狼咬腿、鼠袭手等战术瞄准）。
2. 本 plan P0/P1 只交付通用机制：NPC 攻击走"指向目标 + 确定性无偏 jitter"（见 #1），即所有 NPC 对所有部位的可及性均等，不做物种特化。per-attacker jitter 参数（σ、武器缩放系数）作为钩子留给下游消费——未来若要做"狼偏好咬腿"，可在同一 jitter 框架内传入非对称/偏置分布，但该扩展依赖 `MundaneFaunaKind` 枚举（尚未落地，属 plan-mundane-fauna-v1 范畴），本 plan 不实现。
3. 边界/拒绝理由：拒绝在本 plan 内实现物种偏好，因为需要先有 fauna 侧的物种分类枚举落地，否则会在本 plan 里硬编一份跟 fauna plan 重复/冲突的物种表——违反 `docs/CLAUDE.md` §四"近义重名"红旗。

**落点**：`combat/raycast.rs:90-108`（jitter 钩子留空扩展点，不实现物种表）+ plan §P0（范围边界注记）。

### #4 玩家垂直视角自然涌现 / classify_body_part 阈值

**决议**：
1. P0 不改阈值。`classify_body_part`（`raycast.rs:45-88`）现有 0.88/0.55/0.35/0.18 阈值是合理保守默认——真实 `Look` 射线接入后应能自然点亮全部位，无需先调阈值再验证。
2. 阈值验证挪到 P1：实机测试蹲下瞄腿 / 仰头瞄头在真实射线下的实际命中分布，若臂命中率显著低于目标（15-20%），优先调 `0.18`（臂横向阈值）或 `0.30`（腿富余时可略降腿阈值），而非改动 Y 轴基准点（`CHEST_AIM_HEIGHT` 相关几何）。
3. 边界：拒绝"P0 先调阈值再接线"的顺序——阈值校准依赖真实分布数据，没有真实 jitter 输出之前调阈值是拍脑袋，违背 `docs/CLAUDE.md` §五"决议数据必须靠 Explore agent 核查代码现状产出"的约束。

**落点**：`combat/raycast.rs:45-88`（`classify_body_part` 阈值常量）+ plan §P1（分布校准测试抓手内新增"阈值微调"子项）。

### #5 Back 部位激活

**决议**：
1. 不是顺手能补的功能，**不塞进 P0**。
2. Back 需要新契约，而非"顺手在 raycast 里加一个 case"：`classify_body_part` 新增 defender facing 输入（当前签名无此参数）+ 攻方相对目标后半球 dot 判定 + 胸高后向 remap 逻辑 + client Back 红点 UI（`MiniBodyHudPlanner` 现有 switch 分支未含 back case，需另行确认/新增）+ 偷袭系数（0.9× 伤害并可能叠乘偷袭加成，涉及新的伤害修正路径）。
3. 边界/拒绝理由：本次 promote 不新增阶段承载它——同一批改动里同时改"瞄准来源"（P0 raycast 签名）与"defender facing 契约"（Back 判定）会让测试面同时爆炸，无法定位回归来源。列为**本 plan 范围外的 stretch / 独立小增量**：待 P0 raycast 改造落地、P1 四肢分布数据出来后，若要做 Back 应作为独立后续增量（新 plan 或本 plan 追加阶段），带专属 pin 测试（后半球 dot 判定边界、胸高 remap、偷袭系数叠乘），不与本 plan 的 P0-P3 主线合并。

**落点**：`combat/raycast.rs:45-88`（`classify_body_part` 签名，Back 恒不产出的位置）+ `resolve.rs:8935-8936`（Back 永不触发的现状注释）+ plan §8（列为非目标，不计入 P0-P3 主线交付物）。
