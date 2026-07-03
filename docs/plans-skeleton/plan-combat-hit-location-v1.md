# plan-combat-hit-location-v1 — 近战命中部位真实化：废除"恒瞄胸口中心"射线

> **骨架**（2026-07-03）。一句话：命中部位由攻击者真实瞄准 + 目标几何决定，四肢可中、部位倍率表和腿伤系统终于有戏份；玩家与 NPC 双向同修（这不是 NPC 专属 bug）。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 瞄准射线改造（攻方 Look / NPC 散布） | ⬜ |
| P1 | 部位分布校准 + 消费端补齐（臂伤后果） | ⬜ |
| P2 | 硬编 Chest 旁路清理 | ⬜ |
| P3 | 部位差异视听反馈 | ⬜ |

## 接入面（docs/CLAUDE.md §二）

- **进料**：`combat::events::AttackIntent`（`events.rs:35-44`，现无任何瞄准字段）；玩家 `Look` component（server 端已有，**无需 wire 变更**）；NPC 攻击构造点 `npc/brain/actions_combat.rs:297-306`；玩家攻击构造点 `combat/player_attack.rs:96-105`
- **出料**：`raycast_humanoid`（`combat/raycast.rs:90-108`）产出的 `hit_probe.body_part` → `Wound.location`（`resolve.rs:954/1416/1430/1502`）→ 既有消费端：部位倍率表 `body_part_multipliers`（`resolve.rs:1757-1764`）、腿伤减速 `movement/leg_wound.rs:13-65`、HUD 人体剪影红点（现成按部位渲染）
- **共享类型**：`BodyPart` 枚举（`combat/components.rs:30-41`）不动；`classify_body_part`（`raycast.rs:45-88`）分类逻辑本身支持四肢，不动阈值只修射线
- **跨仓库契约**：无 wire 变更（伤口 payload 已带 location；client HUD 已消费）
- **worldview 锚点**：§四:215 战力分层"体表、经脉、真元——多血条模型"——体表伤按部位分布是该模型的应有之义；§四:334 拼刺刀近战
- **qi_physics 锚点**：不涉及真元流动，无新增常数

## 背景调研结论（2026-07-03，三 agent 并查）

- 命中部位**非随机 roll、非攻方选择**：`raycast_humanoid` 把射线写死瞄准目标 AABB **胸口中心**（X/Z 取包围盒中心、Y 取脚底 + `CHEST_AIM_HEIGHT=1.2`，`raycast.rs:28,90-108`）
- 后果双杀：横向偏移 `lateral` 恒 ~0 → 永达不到手臂阈值 0.18（臂不可达）；命中 y 恒落胸区间 0.55~0.88 → 腿（<0.35）不可达。`resolve.rs:8808-8809` 测试注释白纸黑字承认"无法可靠命中 ArmL/ArmR"
- 玩家与 NPC **完全对称**地坏：三处 `AttackIntent` 构造点字段集一致，都不带视线；集成测试断言双向命中恒 `Chest`（`resolve.rs:3670/3677/3882`）
- 部位倍率表（头 2.0×/臂 0.7×/腿 0.6×，`resolve.rs:1757`）与腿伤减速全接好，只是永远轮不到四肢

## P0 瞄准射线改造 ⬜

- `raycast_humanoid` 改签名：接受攻方瞄准方向（玩家 = `Look` 转向向量；NPC = 指向目标 + 确定性散布 jitter，种子取 `(attacker_id, combat_tick)`），替换 `fallback_aim` 恒定中心点
- `resolve.rs:413-420` 决议处按 attacker 类型取瞄准源；`AttackIntent` 本体不动（瞄准在决议端读组件，避免 wire/事件形状变更）
- 测试抓手：`raycast.rs` 部位分布统计测试（固定 seed 批量攻击，Head/Chest/Abdomen/ArmL/ArmR/LegL/LegR 命中率各 >0）；俯视/仰视/侧向命中专项；`resolve.rs:3670` 系列"恒 Chest"断言改为分布断言

## P1 部位分布校准 + 消费端补齐 ⬜

- 散布参数校准到目标分布（数值 §8 #1 收口）：正面平视基线约 胸 40-50% / 腹 15% / 头 8-12% / 臂 15-20% / 腿 15-20%
- **臂伤消费端**（调研缺口：腿有 `leg_wound.rs` 减速，臂伤现无 gameplay 后果）：ArmL/ArmR 重伤 → 攻击力度/蓄力惩罚（复用 `wound_severity_to_grade` 分级模式，落点 `combat/` 新 `arm_wound.rs` 或并入 status）
- 测试抓手：分布 pin 测试（固定 seed 直方图区间断言）；臂伤分级 → 攻击惩罚映射表专属 case

## P2 硬编 Chest 旁路清理 ⬜

- 逐个决断 6 处硬编：格挡反伤 `resolve.rs:989`（反伤打持盾臂更合理？）、`carrier.rs:1003`、`sword_basics.rs:1474`（剑招按剑轨迹高度定部位）、`lifecycle.rs:2678/4876`、`woliu_v2/skills.rs:671`
- 保留合理者（写明理由注释），改掉懒惰者；每处一条 pin 测试

## P3 部位差异视听反馈 ⬜

- HUD：人体剪影红点已按部位渲染（现成，零改动验证即可）
- 命中反馈差异：头部命中 `BongSpriteParticle` 暴击星形 burst ×6、lifetime 8t、白金色 `#FFE9A0`；四肢命中血色 `BongLineParticle` ×3 沿命中法线、lifetime 6t、`#8C1F1F`；音效 audio_recipe：头部 `entity.player.attack.crit`(pitch 1.15) 叠 `entity.arrow.hit_player`(delay 1t)，四肢 `entity.player.attack.weak`(pitch 0.9)
- 腿伤触发减速时目标脚下 `BongGroundDecalParticle` 血渍 decal（复用既有 decal 基类），lifetime 100t
- narration 示例（zone / perception）：「一剑削中持刀的右臂，兵刃当啷落地半寸又被攥紧」「膝弯中箭，那散修的步子瞬间烂了」

## §8 开放问题（升 active / P0 决策门前收口）

1. **散布参数与目标分布数值**：jitter 半径/椭圆比、按武器 kind（拳/刀/枪 reach 不同散布不同？）——需实测校准
2. **臂伤 gameplay 后果形态**：攻击惩罚 vs 持械掉落 vs 蓄力时长惩罚；与既有 `MeridianSeveredPermanent`（断脉禁招）的边界
3. **NPC 战术性瞄准**：狼咬腿、鼠袭手等物种偏好是否此 plan 做（与 plan-mundane-fauna-v1 preys_on 联动）还是留给 fauna plan
4. **玩家垂直视角自然涌现**：蹲下打腿/瞄头爆头在真实 Look 射线下应自然可行——需实机验证阈值（`classify_body_part` 0.88/0.55/0.35/0.18）是否要随之微调
5. **Back 部位激活**：`classify_body_part` 永不产出 Back（`resolve.rs:8930` 注释）——背刺方向判定是否顺手补（攻方位于目标背半球 → Back，0.9× 伤害但可叠偷袭系数）
