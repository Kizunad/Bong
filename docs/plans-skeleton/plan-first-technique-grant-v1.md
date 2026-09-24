# plan-first-technique-grant-v1 — 前 5 分钟第一个可施放招式（纯获取路径）

> **一句话主题**：新玩家起手**没有任何可施放招式**——8 种招式残卷全部压在深层 TSY 容器 loot 里，教程箱只给一颗开脉丹，"修仙感"的第一个主动动作被锁在搜刮进度之后。本 plan 在新手引导链早期（≤5 分钟）以**沉默引导合规**的方式投放 1 张已落地招式的残卷，零新招式、零新资产、配置级改动。
>
> 来源：2026-07-18 早期玩法诊断——「进游戏只想搜箱子」的三根因之一（前 30 分钟能按的按钮太少，爽点全 gated 在 loot 之后）。

**状态**：骨架（skeleton）。升 active 前按 docs/CLAUDE.md §五收口 §8。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 投放点接线（教程链 loot 表 + 可选散修掉落）+ 学习门槛核验 | ⬜ |
| P1 | 选中招式的 A/V 链回归 + bot e2e（搜→学→施放→专属 AV payload） | ⬜ |

## 现状证据（2026-07-18 Explore 实证）

- 起手 loadout（`server/assets/inventory/loadouts/default.toml`）：铁剑 + 丹粉草种骨币，**无招式残卷**；出生静默发放的 `scroll_meridian_primer` 是 `readable_scroll` 纯阅读 lore，不给招式（`world/spawn_tutorial.rs:358`）。
- 教程箱 `tutorial_kaimai_chest` loot pool 只有 `kaimai_dan`×1（`server/loot_pools.json:164`）。
- 新手 POI 的「三门基础卷」是 `scroll_alchemy_basics`/`scroll_forging_basics`/`scroll_botany_basics` 生产入门卷，非战斗招式（`server/src/inventory/poi_loot.rs:17-33`）。
- 招式残卷（sword.cleave/thrust/parry/infuse、movement.dash、burst_meridian.beng_quan、zhenmai.parry 等 8 种）分布在深层 TSY pool（`server/loot_pools.json`）；学习链 `TechniqueScrollUse` → `handle_learn_technique_scroll`（`network/client_request_handler.rs:3118`）+ `can_learn_technique` 门槛已完整。
- 结论：**获取路径是唯一缺口**，招式本体 + 学习链 + A/V + icon（[[plan-skill-av-relink-v1]] skill_scroll 单一真相源）全部现成。

## 接入面（docs/CLAUDE.md §二 checklist）

- **进料**：`loot_pools.json` 教程箱 pool；`spawn_tutorial.rs` 教学链（可选投放点：石棺开棺 / `tutorial_rogue_anchor` 散修掉落）；既有招式注册表（`SkillRegistry` + technique scroll spec）。
- **出料**：招式残卷 `ItemInstance` 进玩家背包 → 既有 `TechniqueScrollUse` 学习链 → SkillBar 槽位（既有）。
- **共享类型 / event**：**零新增**——复用既有 scroll template / loot pool entry / 学习事件。不新造"教学专属残卷"模板（近义重名红旗）；用生产环境同款残卷，掉率 100% 只出现在教程 pool。
- **跨仓库契约**：无 wire 变更；client 零改动（icon/HUD/cast 链全部既有）。
- **worldview 锚点**：§三 修炼主流程（招式是引气期玩法的入口）；journey K 红线 **O.13 不做新手 UI 教程**——投放必须走环境线索（箱中残卷 / 尸体掉落），严禁弹窗提示"你学会了 XX"以外的新增引导 UI；journey §L 钩子表 15:00-30:00 段的"主动感"补强。
- **qi_physics 锚点**：不涉及——施放消耗走招式既有 qi 路径，本 plan 不触碰数值。

## P0 — 投放点接线 + 学习门槛核验 ⬜

- **首选投放**：`tutorial_kaimai_chest` pool 追加 1 张低阶剑技残卷（倾向 `sword.cleave`——起手就有铁剑，学完即可用；最终选型见 §8 #1）。
- **可选第二投放**（§8 #2 拍板后）：`tutorial_rogue_anchor` 散修 NPC 击杀掉落 `movement.dash` 残卷——给"打"环节第一个奖励动机（该 NPC 本就设计为可杀，`plan-spawn-tutorial-v1` Q97 已决）。
- **学习门槛核验**（P0 硬前置）：实地核对 `can_learn_technique` 对醒灵境 + 零经脉玩家的判定——若所选招式有境界/经脉前置挡住醒灵期，则改选无门槛招式或把投放点后移到开脉丹使用后（仍在 30min 链内）。**不为教程改门槛数值**。
- 招式注册须已在 `SkillMeridianDependencies::declare` 登记（docs/CLAUDE.md §四红旗）——复用既有招式即自动满足，P0 只做核验断言。
- 测试：教程 pool pin（含新残卷 + 数量 1 + 概率 1.0）；`can_learn_technique` 醒灵境正反 case；loot 模板引用启动校验。

## P1 — A/V 链回归 + bot e2e ⬜

- 所选招式的**既有** A/V 链完好性回归：icon（skill_scroll 真相源 → `SkillIconRegistry`）、cast 动画、粒子、SFX、HUD 反馈各一条断言——本 plan 不新增任何资产，只锁"新手拿到的第一招视听完整"（招式 A/V 差异化红线的验收面）。
- bot 场景 `scenarios/tutorial_first_technique.py`：join → G 键搜教程箱 → 拾取残卷 → `TechniqueScrollUse` intent → technique 列表 payload 含该招式 → SkillBar cast intent → 招式专属通道 payload 到达。
- 与 [[plan-newbie-30min-hooks-audit-v1]] 的 30min 整链场景对接：本场景绿后，30min 矩阵"主动玩法密度"项自动受益（两 plan 独立可 merge，无硬依赖顺序）。

## §8 开放问题（升 active / P0 决策门前收口）

1. **首招选型**：`sword.cleave`（横斩，配起手铁剑，战斗爽点直接）vs `movement.dash`（位移，无武器依赖，但 V 键已有内建冲刺、体感重叠）。倾向 sword.cleave；需实地核对其 qi 消耗在醒灵期 qi_max 下可施放次数 ≥2。
2. **单投放 vs 双投放**：教程箱保底 1 张 vs 箱+散修掉落各 1 张。双投放给"打"动机但两招齐发有教学过载/通胀嫌疑；若双投放，散修掉落概率是否 <1.0（沉默引导下玩家可能根本不打 NPC）。
3. **重生角色是否重复领取**：教程箱 per-instance 共享（spawn-tutorial Q99 同 spawn 共享教学）——重生玩家再搜是否重复出卷；残卷可交易（worldview 红线：不做绑定），重复出卷 = 无限白嫖源，需核对教程箱 refresh 语义（倾向：一次性 pool，搜空不重刷）。
4. **老玩家回溯**：已过新手期的存量角色无补发（推荐——残卷本就可从 TSY 获取，不回溯）；确认无成就/钩子依赖"从教程箱获得"。
