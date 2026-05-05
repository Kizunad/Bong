# Skeleton 未决事项登记

本目录下 plan 在展开/落地过程中累积的**延后处理**事项。每条带 plan 锚点 + 上下文，后续回来解决时直接对号入座。

---

## plan-tribulation-v1

- [ ] **半步化虚 buff 强度**：当前 +10% 真元上限 / +200 年寿元是占位。Phase 1-3 已上线，可观察"卡在半步化虚"的玩家比例后调整；名额空出时可重渡的升级机制也待确认（已在 plan-tribulation-v1 §8 标注）

---

> **约定**：每解决一条就从这里删。新增延后事项请直接追加到对应 plan 段，保持扁平。
>
> **已转为独立骨架（2026-04-27）**：
> - `plan-alchemy-client-v1`（炼丹系统 Fabric 客户端接入）
> - `plan-niche-defense-v1`（灵龛主动防御）
> - `plan-fauna-v1`（妖兽骨系材料）
> - `plan-spiritwood-v1`（灵木材料体系）
> - `plan-spirit-eye-v1`（灵眼系统）
> - `plan-botany-agent-v1`（植物生态快照接入天道 agent）
>
> **已转为独立骨架（2026-05-01）**：
> - `plan-lifespan-v1`（寿元精细化 / 风烛 / 续命路径 / 老死分类）— 源自 plan-death-lifecycle-v1 §4a/§4c reminder
> - `plan-anticheat-v1`（AntiCheatCounter / CHANNEL_ANTICHEAT）— 源自 plan-combat-no_ui §1.5.6 reminder
> - `plan-alchemy-v2`（side_effect_pool 映射 / 丹方残卷 / 品阶铭文开光 / AutoProfile / 丹心识别）— 源自 plan-alchemy-v1 reminder
> - `plan-inventory-v2`（Tarkov grid placement / stacking 合并）— 源自 plan-inventory-v1 reminder
>
> **已转为独立骨架（2026-05-05）—— qi_physics 底盘**：
> - `plan-qi-physics-v1`（修仙物理底盘：守恒律 + 压强法则 + 唯一物理实现入口）— **关键路径**。源自 plan-economy-v1 §1.5 衰变曲线裁决无解，上钻发现 worldview §二「真元极易挥发」是 9+ plan 同源现象（骨币/食材/距离/异体排斥/吸力/节律/末法残土/灵田漏液/搜刮磨损），各 plan 拍数才是问题根源。本 plan 立公理 + 算子 + 全局账本 WorldQiAccount，P1 完成 = 底盘 API 冻结
> - `plan-qi-physics-patch-v1`（qi-physics 迁移收口）— 承接 qi-physics-v1 P1 后的迁移工作；P0 红线 3 PR（combat/decay 0.06 vs 正典 0.03 翻倍 / tsy_drain×dead_zone 协调 / WorldQiAccount 合账）；P1 shelflife / P2 战斗+守恒释放 / P3 新机制（坍缩渊 redistribute / 7 流派异体排斥 ρ / 时代衰减 / 阈值灾劫）
>
> **已转为独立骨架（2026-05-05）—— 流派功法**：
> - `plan-woliu-v2`（涡流功法五招完整包：持涡 / 瞬涡 / 涡口 / 涡引 / 涡心）— 承接 plan-woliu-v1 ✅ finished。引入**搅拌器物理**（99% 紊流甩出 + 1% 入池 × 经脉流量 cap）+ **紊流场**（动态漩涡非可吸收浓度，是涡流流派专属 EnvField 边界，其他玩家在场内不可修炼 / 战斗精度 ×0.5 / shelflife ×3）+ 反噬阶梯 4 级（微感 / MICRO_TEAR / TORN / SEVERED）+ **无境界 gate 只有威力门坎**（worldview §五:537 流派由组合涌现）+ 化虚双场模型（致命场 ≤10 格 + 影响场 zone 量级）+ 化虚被动场可关。前置依赖 plan-qi-physics-v1 P1 + plan-qi-physics-patch-v1 P0/P2 完成。反向被依赖：plan-style-balance-v1（W/β/K_drain 矩阵）/ plan-color-v1（缜密色加成 hook）/ plan-tribulation-v1（化虚绝壁劫触发链）/ plan-zhenfa-v1（紊流场 vs 阵法冲突仲裁，留 zhenfa vN+1）。化虚涡心叙事意象 = worldview §四:380「化虚老怪走过新人来不及看清袍角」物理依据（整个山谷瞬间进入紊流死区）。骨架 §5 七个开放问题待 P0 决策门收口（化虚被动场默认开关 / 紊流场对 caster 自身影响 / 紊流 vs 阵法仲裁 / 99/1 比例 telemetry / 干尸涡引 / 被动场 qi 消耗 / 防御招精度衰减）
> - `plan-dugu-v2`（毒蛊功法五招完整包：蚀针 / 自蕴 / 侵染 / 神识遮蔽 / 倒蚀）— 承接 plan-dugu-v1 ✅ finished（PR #126）。引入**脏真元 ρ=0.05**（worldview §五:425 寄生虫机制）+ **永久阈值分三档**（worldview §三:368 越级原则物理化身——醒灵/引气/凝脉仅 HP+qi / 固元 24h 短期可恢复 / 通灵+ 永久 qi_max 衰减）+ **自蕴**（自身经脉养成毒源，非养虫，worldview §六:621）+ **暴露概率系统**（每招 roll，化虚 0.2%，被识破 → DuguRevealedEvent → plan-identity-v1）+ **阴诡色形貌异化永久不可洗**（worldview §六:618-621 + §五:531 社会代价）+ **化虚倒蚀触发绝壁劫**（同涡心化虚一致格调）。**严守 worldview §五 + §六 正典，去除"蛊母/蛊虫/虫卵"等偏离虫子叙事**，"蛊"仅作汉字"诡毒"意。前置依赖：plan-qi-physics-v1 P1 + plan-qi-physics-patch-v1 P0/P3（7 流派 ρ 矩阵）+ plan-identity-v1（暴露 consumer）+ plan-botany-v2（自蕴毒草）+ **plan-craft-v1**（蚀针手搓 + 自蕴煎汤通过通用手搓 tab 注册配方）。反向被依赖：plan-style-balance-v1 / plan-color-v1（永久不可洗染色接口）/ plan-tribulation-v1（化虚倒蚀绝壁劫）/ plan-narrative-political-v1（暴露后江湖传闻）。骨架 §5 七个开放问题待 P0 决策门收口（化虚倒蚀绝壁劫去留 / 暴露概率公式三选 / 自蕴时间曲线 / 痕迹三种统一接口 / 自蕴毒草复用 vs 新建 / 阴诡色 plan-color-v1 接口 / TaintMark 跨周目持久化）

> **已转为独立骨架（2026-05-06）—— 通用手搓底盘**：
> - `plan-craft-v1`（通用手搓面 / inventory 内「手搓」标签）— 源自 plan-dugu-v2 起草中发现"蚀针 / 自蕴煎汤手搓 UI 缺失"，上钻发现是通用问题（蚀针 / 自蕴煎汤 / 伪皮 / 阵法预埋件 / 凡器都需要"轻度仪式化"合成）。**inventory 标签集成（无方块）**+ **单任务无并发** + **in-game 时间推进（在线累积下线暂停）**+ **配方解锁三渠道**（残卷 / 师承 / 顿悟，**无流派自动解锁** —— worldview §九 信息比装备值钱物理化身）+ **首版不实装磨损税和装备加速**（留 v2）。区别于 forge（4 步状态机）/ alchemy（火候）/ vanilla（3x3 摆放）。配方分类 6 类：AnqiCarrier / DuguPotion / TuikeSkin / ZhenfaTrap / Tool / Misc。前置依赖 plan-skill-v1 ✅ + plan-inventory-v2 ✅ + plan-input-binding-v1 ✅ + plan-HUD-v1 ✅。反向被依赖：plan-dugu-v2 / plan-tuike-v2 / plan-zhenfa-v2 / plan-anqi-v1 vN+1 / plan-tools-v1 各自注册自家配方。qi_cost 必须走 qi_physics::ledger::QiTransfer 守恒（不允许直接扣 cultivation.qi_current）。骨架 §5 六个开放问题待 P0 决策门收口（配方分类是否补 / 排序方式 / 取消返还比例 / 死亡时任务处理 / 跟 vanilla 工作台边界 / 软硬 gate）
> - `plan-tuike-v2`（替尸·蜕壳功法**三招完整包**：着壳 / 蜕一层 / 转移污染）— 承接 plan-tuike-v1 ✅ finished（PR #124）。引入**影论假影承伤**（worldview §P 定律 6 + cultivation-0002 §影论）+ **物资派纯粹定调**（worldview §五:471 钱包代价不绑身体，永不 SEVERED）+ **三档伪皮 + 化虚上古级**（轻 50 / 中 150 / 重 400 / 上古 1000+ 伤吸收）+ 多层叠穿（醒灵 1 → 化虚 3 层）+ **专克毒蛊 + 化虚 hard counter**（化虚替尸者上古伪皮可吸 dugu 永久 qi_max 衰减标记 → 蜕落带走，物资派最纯粹极限化身）+ 制壳走 plan-craft-v1 通用手搓（不在战斗 plan）。**不做化虚专属新招式**（区别于涡流化虚紊流死区 + 毒蛊化虚倒蚀）—— 化虚级仅钱包更深，无身体质变。**不做死蛹假死**（worldview §十二 死亡机制冲突 + 物资派应纯粹）。前置依赖 plan-qi-physics-v1 P1 + plan-qi-physics-patch-v1 P0/P3（β=1.2）+ plan-spiritwood-v1 + plan-fauna-v1 + plan-craft-v1（4 档伪皮配方）+ plan-multi-style-v1（凝实色 PracticeLog）。反向被依赖：plan-style-balance-v1（W=0.7 / β=1.2 矩阵）/ plan-tribulation-v1（化虚 hard counter PVP 渡劫干扰）/ plan-narrative-political-v1（化虚一战烧上古伪皮的江湖传闻）/ plan-tsy-loot-v1（上古级伪皮材料源）。当前游戏伤害基线已确认 ATTACK_QI_DAMAGE_FACTOR=1.0（worldview §五:336 对齐实装），伪皮档位锚定"挡一次该境界全力一击"。骨架 §5 五开放问题待 P0 决策门收口（化虚级吸永久标记比例 / 蜕落物腐烂时间 / 多层叠穿维持 qi cost 是否线性 / 上古级伪皮材料源 / 凝实色 hook 实装位置）
> - `plan-zhenmai-v2`（截脉·震爆功法**五招完整包**：极限弹反 / 局部中和 / 多点反震 / 护脉 / **绝脉断链 化虚专属**）— 承接 plan-zhenmai-v1 ✅ finished（PR #122；P0 极限弹反已实装）。引入**音论物理**（worldview §P 定律 5 + cultivation-0002 §音论：单点 C_contact 高反震集中 / 多点 C 低分布广）+ **血肉派定调**（worldview §五:432-436 用 HP 经脉换 qi 免伤）+ **化虚专属绝脉断链**（worldview §四:319 主动 SEVERED 一条经脉换 60s 选择性免疫，4 类攻击 真元/物理载体/脏真元/阵法 选 1）+ 反噬阶梯含 SEVERED + **瞬时痕迹**（5-10s 散尽，区别于涡流紊流 5min / 毒蛊残留 30min / 替尸蜕落物 30min 的长期场）+ **熟练度生长二维划分（v2 通用机制首发）**：境界决定威力上限（K_drain / 反震点数 / 硬化抗性 / 中和兑换率）/ 熟练度决定响应速率（冷却 30→5s / 弹反窗口 100→250ms 跟 skill_lv 线性递变）。**反制化虚毒蛊师的关键路径之二**（与替尸 ③ 上古伪皮 hard counter 并列）：化虚截脉 ⑤ 选"脏真元类"免疫 60s 烧身体 vs 化虚替尸烧物资。**专克器修 W=0.7 / 完全失效 vs 毒蛊 W=0.0**（worldview §P 矩阵）。前置依赖：plan-qi-physics-v1 P1 + plan-qi-physics-patch-v1 P0/P3（β=0.6 + W 矩阵 + SEVERED 写入 MeridianSystem）+ plan-cultivation-canonical-align-v1（经脉拓扑选择）+ plan-skill-v1（熟练度系统）。反向被依赖：plan-style-balance-v1 / plan-tribulation-v1（化虚断脉触发天道注视）/ plan-narrative-political-v1（化虚断脉求生江湖传闻）/ plan-multi-life-v1（SEVERED 跨周目不继承）。骨架 §5 七开放问题待 P0 决策门收口（化虚断脉触发绝壁劫去留 / 选定经脉限制 / 化虚弹反窗口 200/250ms / 多点反震与护脉并存 / 暴烈色 hook 实装位置 / 熟练度公式 vs plan-skill-v1 lv 映射 / 熟练度生长机制是否回填其他 v2 plan）

> **已转为独立骨架（2026-05-06）—— 经脉永久 SEVERED 通用底盘**：
> - `plan-meridian-severed-v1`（经脉永久 SEVERED 通用底盘 + 招式依赖经脉强约束）— 源自 plan-zhenmai-v2 ⑤ 绝脉断链私有 component `MeridianSeveredVoluntary` 的提取需求 + 用户拍"SEVERED 应是通用受伤类型"。底盘机制：MeridianSeveredPermanent component（永久 + 跨 server restart 持久化 + 跨周目重置）+ MeridianSeveredEvent 通用 event（7 类来源：VoluntarySever / BackfireOverload / OverloadTear / CombatWound / TribulationFail / DuguDistortion / Other）+ Skill::dependencies 接口（招式注册声明依赖经脉）+ cast 前统一检查（任一依赖经脉 SEVERED → Reject + HUD 灰显 + tooltip）+ inspect 经脉图 SEVERED 黑色可视化。**§3 招式依赖经脉强约束（CLAUDE.md 风格规则）**：所有 SkillRegistry 注册必须 `.with_dependencies(meridian_ids)`，漏写 = 红旗。已锁定 7 流派粗粒度依赖清单（体修手三阳+任督 / 暗器手三阴 / 阵法任督+KI / 毒蛊足三阴+LU / 截脉 LU+LI / 替尸手三阴 / 涡流任督+HT），细粒度由各 v2 plan P0 决定。接经术主路径 = 医者 NPC 服务（plan-yidao-v1 🆕 实装），备选 PvE = 上古接经术残卷（plan-tsy-loot-v1）。**反向被依赖**：所有 v2 流派 plan（含已立 woliu/dugu/tuike/zhenmai 私有 SEVERED component 应迁出为通用）+ plan-yidao-v1 🆕 + plan-multi-life-v1（跨周目重置）+ plan-narrative-political-v1（化虚断脉江湖传闻）。骨架 §8 四开放问题待 P0 决策门收口。

> **派生新流派占位（2026-05-06）—— 医道功法**：
> - `plan-yidao-v1`（医道功法，**跟 7 战斗流派平行的支援流派**）— worldview §六:617 已锚定「医道 / 平和色 / 针灸通经络效率+ / 疗他人时排异成本-」。招式范围：**接经术**（恢复 SEVERED，跟 plan-meridian-severed-v1 联调）/ 排异加速（中和 contam 比 zhenmai ② 局部中和效率高 ×3）/ 自疗 / 疗他人 / 续命术（worldview §十二:1043，跟 plan-alchemy-v1 ✅ 续命丹配合）/ 急救（HP 出血止血）。前置依赖：plan-meridian-severed-v1 🆕（接经术目标）+ plan-alchemy-v1 ✅（续命丹）+ plan-social-v1 ✅（医者 NPC 信誉度，worldview §十一）+ plan-multi-style-v1 ✅（平和色 PracticeLog）。worldview 锚定平和色染色 + 医道流派定义已正典，但当前**支援流派**机制（治疗他人、长期医患关系、流派身份认知）需在本 plan 设计。反向被依赖：所有玩家（任何角色都可能寻医）。范围：医者 NPC 行为 AI / 接经仪式机制 / 5-7 招式 / 平和色养成。**与 7 战斗流派区别**：医道是支援流派，PVP 中可被招募 / 雇佣 / 结契，跟 plan-social-v1 + plan-narrative-political-v1 深度耦合。**为后续医术体系铺垫**：未来可能扩展 plan-yidao-v2（毒手医 / 兽医 / 道伥医 等亚流派）。

> **v2 流派 plan 通用机制：熟练度生长二维划分**（2026-05-06 zhenmai-v2 首发）
> - **境界 = 威力上限**（K_drain / 反震点数 / 硬化抗性 / 中和兑换率 / 紊流半径 / 自蕴乘数等"大小"维度）
> - **熟练度 = 响应速率**（冷却 cooldown / 弹反窗口 window / cast time / cast 充能时间）
> - 公式：`cooldown(lv) = base + (min - base) × clamp(lv/100, 0, 1)`，线性递减
> - 哲学：worldview §五:537 流派由组合涌现 + §五:506 末土后招原则物理化身——醒灵苦修 lv 100 弹反窗口 250ms / 化虚老怪 lv 0 仅 100ms（练得多胜过境界高）
> - **应回填的 v2 plan**（各自 P0 决策门统一处理）：plan-woliu-v2（瞬涡 5s / 涡口 8s / 涡引 30s 改为按熟练度）/ plan-dugu-v2（蚀针 3s / 侵染 8s 改）/ plan-tuike-v2（蜕一层 8s / 转移污染 30s 改）+ 未来 plan-anqi-v2 / plan-zhenfa-v2 / plan-baomai-v3
> - 前置依赖：plan-skill-v1 ✅（SkillSet.skill_lv + SkillXpGain event 已实装）
> - 各 plan P0 需决定：(a) 公式 vs plan-skill-v1 lv 映射区间；(b) 各自招式 base/min 值；(c) 是否需要派生 plan-skill-proficiency-v1 通用 plan 提取 cooldown/window curve helper
>
> **依赖链关键路径（plan-economy / plan-style-balance / 等等都在等）**：
> ```
> plan-qi-physics-v1 P0 红线决议 → P1 算子 ship
>   → plan-qi-physics-patch-v1 P0/P1/P2/P3 逐 PR 迁移
>     → plan-economy-v1 / plan-style-balance-v1 / 其他 ~9 个 plan 解阻
> ```
>
> **同步动作（2026-05-05）**：
> - `docs/CLAUDE.md §二 接入面 / §四 红旗` 各加 qi_physics 锚点条目，约束新 plan 不再自己拍真元常数
> - `plan-economy-v1` §1.5 三选一裁决整体废弃；§0 持有=贬值补地点制约推导；§4 收口 2 条原悬而未决
> - `plan-style-balance-v1` 现状对齐：7 流派 plan 全 finished（`docs/finished_plans/`）；P1 telemetry 已在 PR #129 顺手实装混元色维度，但 spec 5 维未对齐
>
> **已核实可删除（2026-05-01）**：
> - plan-tribulation-v1：预兆窗口 60s ✅（已在 plan §2.1 定义）；域崩阈值 spirit_qi<0.1 持续 1h ✅（已在 plan §4.1 定义）；欺天阵接口 → 已归 plan-zhenfa-v1 tracking
> - plan-zhenfa-v1 两条开放问题 → 已在 active plan §10 tracking
> - plan-lingtian-v1 两条 → 已在 active plan tracking
> - plan-combat-no_ui 遗念 deathInsight → 已实装（`server/src/schema/death_insight.rs` / `combat/lifecycle.rs`）
> - plan-alchemy-v1 测试 JSON 占位 → 仅提示注释，不需要 plan tracking
> - plan-alchemy-v1 SVG 草图尺寸差异 → 仅草图，不影响实装
> - 通用 "开放问题节 review pass" → meta-task，太宽泛；直接推进各 plan
> - plan-tools-v1 采药工具系统 → ✅ 已完成（2026-04-29 立项，已有骨架）
