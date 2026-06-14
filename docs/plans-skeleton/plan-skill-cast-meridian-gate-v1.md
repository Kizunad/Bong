# plan-skill-cast-meridian-gate-v1（骨架）

> **骨架（草案）**。一句话主题：玩家 skill-bar 施放路径**统一消费 `SkillMeridianDependencies`** 校验依赖经脉——修复「断经脉仍能施放对应招式」的物理可见性破坏（worldview §四:286 红线）。

> 立项动机：bug-hunt round1 确认 `woliu.vortex` 漏校验 Lung 经脉（major）：`combat/known_techniques.rs:427` 声明 `required_meridians=[Lung]`，但仅在 `technique_scroll.rs:131-149` **学技能时**校验；`resolve_woliu_vortex_skill`/`resolve_vortex_toggle_in_world` 施放路径无任何 `check_meridian_*`；玩家 cast 路径 `client_request_handler.rs:7101-7104` 直调 skill_fn 从不查 `SkillMeridianDependencies`（该资源仅 NPC/sword_path/movement 消费，skill/network 零引用）。致命可达：`woliu.rs:771-772` pick_hand_meridian 恒返回 Lung，woliu 自身 EnvQiTooLow backfire `woliu.rs:306` sever_meridian(Lung) → 学会→低灵域施放反噬断肺经→**仍可继续施放本招**。这正是 docs/CLAUDE.md 红旗「断肺经飞剑手仍能 cast」。

## 阶段总览

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 玩家 skill-bar 施放路径接 SkillMeridianDependencies 校验（架构抉择：resolver 内置 check vs cast 入口统一拦截） | ⬜ |
| P1 | 全招式依赖经脉声明审计（哪些 register 漏 declare） | ⬜ |

## 接入面 checklist

- **进料**：`cultivation::meridian::severed::SkillMeridianDependencies::declare`（招式注册红线）+ `check_meridian_dependencies`（NPC/sword_path 已用）。
- **跨仓库契约**：server `client_request_handler` skill cast 路径 + `combat/*` resolver。
- **worldview 锚点**：§四:286 物理可见性（经脉断=对应招式不可施放）。
- **跨 plan**：`plan-meridian-severed-v1`（finished，§3 流派依赖经脉清单）；所有 v2 流派 plan。

## P0 — skill-bar 施放路径接经脉校验

- **目标**：玩家 cast（`client_request_handler.rs:7101` skill_fn 调用前）统一查 `SkillMeridianDependencies` + `check_meridian_dependencies`，依赖经脉 SEVERED 则拒绝施放（narration 提示）。**架构抉择**：① resolver 内各招式开头自查；② cast 入口（skill_fn dispatch 前）统一拦截（牵动 skill_fn 签名/SkillRegistry 架构，但根治）。
- **可核验**：断 Lung 后 woliu.vortex cast 被拒；测试覆盖 backfire 断经脉→再 cast 被拦。

## P1 — 全招式依赖经脉声明审计

- 审计所有 `SkillRegistry::register`/`register_skills`，对照 `SkillMeridianDependencies::declare`，列出漏声明的招式（woliu.vortex 已确认；其余待审）。补 declare。

## §N 开放问题

1. 架构：resolver 内置 check（分散但小改）vs cast 入口统一（根治但动 skill_fn）——前者快，后者彻底，需决议。
2. backfire 自断经脉后该招式是否应立即不可用（即时 vs 下次 cast 拦）。

## 审计来源

bug-hunt round1 confirmed（woliu.vortex 漏 declare，major）。**report-only**：架构抉择，不擅自大改。
