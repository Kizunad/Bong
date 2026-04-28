# Bong · plan-niche-defense-v1 · 骨架

**灵龛主动防御**。基础灵龛（plan-social §2）仅提供"NPC 不主动攻击 + 方块不破"的被动保护；本 plan 允许玩家消耗**阵石 / 禁制**将灵龛升级为主动防御模式——入侵者触发反击（雷击 / 真元放射 / 幻象陷阱），但防御模式本身会暴露灵龛位置（声光效果），使"隐蔽 vs 防御"成为真实取舍。

**世界观锚点**：`worldview.md §十一 安全与社交`（灵龛基础规则：5 格内 NPC 不攻击 / 方块不破 / 坐标泄露即失效）· `worldview.md §五.3 地师/阵法流`（阵法流"唯一能在灵龛周围建立有效防御体系的流派"）· `worldview.md §六.2 缜密色`（阵法流原生染色，防御阵亲和）· `worldview.md §十一 匿名系统`（声光暴露灵龛坐标 = 保护部分失效的代价）

**library 锚点**：`docs/library/ecology/ecology-0002 末法药材十七种.json`（夜枯藤——地师诡雷绝佳载体，阵石原料）· 待写 `peoples-XXXX 龛主手记`（分布式存储哲学 / 阵石使用经验谈，anchor worldview §十一）

**交叉引用**：
- `plan-social-v1`（前置；基础 SpiritNiche 结构 + `defense_mode: Option<DefenseModeId>` hook 已预留）
- `plan-zhenfa-v1`（阵法 block 叠加到灵龛；防御阵是阵法流核心应用场景）
- `plan-combat-no_ui`（反击触发 AttackIntent / StatusEffect 管线）
- `plan-perception-v1`（神识感知用于探测灵龛防御层，破阵需感知达阈值）
- `plan-alchemy-client-v1`（炸炉 Explode 管线可复用到反击反噬计算）

**阶段总览**：
- P0 ⬜ 数据结构 + 阵石 item（SpiritNiche defense_mode 激活流程）
- P1 ⬜ 防御层实装（5 格触发 + 雷击反击基础版）
- P2 ⬜ 多反击类型（真元放射 / 幻象陷阱）
- P3 ⬜ 声光暴露机制（坐标部分泄露 + 匿名系统联动）
- P4 ⬜ 与 plan-zhenfa 整合（防御阵 block 叠加到灵龛）

---

## §0 设计轴心

- [ ] **隐蔽 vs 防御是核心取舍**：防御模式必须有代价——声光效果让"这里有灵龛"变得可猜测，消解部分隐蔽性
- [ ] **消耗性**：防御阵石一次性，不能无限维持；鼓励玩家在高价值时段才开防御
- [ ] **阵法流专属加成**：地师/缜密色玩家布防效果更强（数值幅度待定），非阵法流也能用但效果打折
- [ ] **不做传送阵**：防御反击只有局部效果，不能全局追踪或传送攻击者
- [ ] **灵龛内不能修炼**：防御模式不改变"灵龛不提供灵气"的基本规则（worldview §十一）

---

## §1 第一性原理（烬灰子四论挂点）

- **缚论·阵石封灵**：阵石将防御指令封缚在灵龛方块范围内，属于阵法镜印的延伸（与 plan-zhenfa 共享底层逻辑）
- **影论·防御层投影**：防御层是龛主真元的"次级投影"——龛主不在场，阵石代劳执行；朽坏速度比阵法更慢（因为灵龛石锁住了更多灵气）
- **音论·声光暴露**：防御反击爆发时，真元剧烈震荡发出特征"音"——从远处听到那片区域有雷光/波动，就知道有人守着什么东西
- **噬论·阵石消耗**：末法环境噬散真元，防御镜印也不例外；阵石每次触发反击后耗散加速，终会归于虚无

---

## §2 阵石与材料

| 阵石等级 | 制作材料 | 触发次数 | 反击类型 |
|---|---|---|---|
| 阵石·初级 | 灵气方块 × 3 + 真元 10 | 3 次 | 雷击（低伤害）|
| 阵石·中级 | 夜枯藤 × 1 + 灵气方块 × 5 + 真元 20 | 5 次 | 雷击 or 真元放射 |
| 阵石·高级 | 夜枯藤 × 2 + 异变兽核 × 1 + 真元 30 | 8 次 | 全三类随机 |

阵石制作归 plan-zhenfa 材料体系（§4 材料链）；本 plan 仅定义 item 枚举和触发次数。

---

## §3 防御层行为

```
激活：
  玩家右键灵龛方块 + 持阵石 → 弹 "是否开启防御模式？" 确认 →
  SpiritNiche.defense_mode = Some(DefenseModeId { stone_tier, remaining_charges })
  声光效果：灵龛方块微弱蓝紫光晕（已激活的视觉信号）

触发条件：
  玩家（非龛主）or 敌对 NPC 进入灵龛方块 5 格内
  → 判断 defense_mode.remaining_charges > 0
  → 随机选反击类型（按阵石等级权重）
  → 执行反击
  → remaining_charges -= 1
  → 0 时 defense_mode = None + 灵龛光晕消失

反击类型：
  - 雷击（Lightning）：目标受 qi_damage，有短暂麻痹 StatusEffect
  - 真元放射（QiBlast）：以灵龛为圆心 5 格 AOE，推退 + qi_damage
  - 幻象陷阱（IllusionTrap）：目标视野短暂幻象扰动（客户端 VFX + StatusEffect::Disoriented）

声光暴露：
  每次反击触发 → 方块 5 格内发出强光晕 + 音效（"灵阵御敌"）
  → 100 格内玩家可见光效（类似天劫预兆，但局部）
  → 匿名系统不直接暴露灵龛主，但暴露大致坐标（探索可达）
  → 神识强（plan-perception 达阈值）可精确感知灵龛位置
```

---

## §4 龛主权限与配置

- [ ] `SpiritNiche { owner: CharId, pos: BlockPos, defense_mode: Option<DefenseModeConfig> }` 结构（在 plan-social-v1 预留 hook 上填充）
- [ ] 龛主免疫自己的防御层（进入 5 格 → 防御不触发）
- [ ] 可随时手动关闭防御模式（阵石不退，charges 剩余归 0）
- [ ] 多个阵石可叠加 charges（上限 = 阵石最高等级上限 × 2）

---

## §5 平衡考量

- **防御 vs 隐蔽的博弈**：
  - 无防御模式：灵龛坐标若未泄露，NPC 不主动攻击 + 方块不破 → 绝对安全
  - 有防御模式：每次反击 → 大致坐标暴露 → 高阶玩家可能追踪 → 灵龛部分失去匿名保护
  - 结论：防御模式对"不怕被发现"的玩家（高境界 / 已暴露玩家）更有价值；对靠隐蔽存活的低境界玩家反而是风险
- **与阵法流的错配**：非阵法流玩家用初级阵石，反击伤害仅 50%（数值待定），使阵法流专业价值凸显
- **夜枯藤经济**：中级阵石消耗夜枯藤（botany 稀有，需 10 层麻手套采）→ 与阵法流其他应用竞争资源

---

## §6 数据契约（下游 grep 抓手）

- [ ] `SpiritNiche::defense_mode: Option<DefenseModeConfig>` 填充 — `server/src/social/niche.rs`
- [ ] `DefenseModeConfig { stone_tier: u8, remaining_charges: u8 }` — `server/src/social/niche.rs`
- [ ] `DefenseReactionKind` enum (Lightning / QiBlast / IllusionTrap) — `server/src/social/niche_defense.rs`（新文件）
- [ ] `NicheDefenseTriggered` event — server ECS
- [ ] `AlchemyFurnacePlace` 类比：`SpiritNicheActivateDefense { v, niche_pos, stone_item_id }` payload — `server/src/schema/client_request.rs`
- [ ] VFX：客户端 `NicheDefenseReaction` 粒子（接 plan-particle-system）— `client/src/main/java/moe/bong/client/vfx/`
- [ ] 阵石 item toml — `server/assets/items/niche/zhen_shi_chu.toml / zhen_shi_zhong.toml / zhen_shi_gao.toml`

---

## §7 实施节点

- [ ] **P0**：`DefenseModeConfig` struct + `SpiritNiche.defense_mode` hook 填充 + 阵石 item toml × 3 + `SpiritNicheActivateDefense` payload handler + 单测（无阵石拒绝 / 阵石初级激活 / charges 计数）
- [ ] **P1**：5 格触发 system + `NicheDefenseTriggered` 事件 + Lightning 反击 AttackIntent + 声光 VFX emit + remaining_charges 递减 + 归 0 关闭
- [ ] **P2**：QiBlast（AOE） + IllusionTrap（StatusEffect::Disoriented）+ 阵石等级权重路由
- [ ] **P3**：100 格内声光广播（匿名坐标暴露）+ plan-perception 神识感知精确定位接口
- [ ] **P4**：plan-zhenfa 防御阵 block 叠加（防御阵 + 灵龛 → 联动加强；细节由 zhenfa plan 定）

---

## §8 开放问题

- [ ] 防御触发时是否对**施法者自己**的境界有要求（引气期玩家放阵石 → 反击效果极低是否合理）？
- [ ] 防御阵石能否被入侵者**手动摧毁**（破阵眼）？→ 与 plan-zhenfa 破阵流程对齐
- [ ] 灵龛坐标暴露的精度：按方块精确坐标暴露，还是"大致 chunk 坐标"（±16 格随机偏移）？
- [ ] NPC 对防御层的反应：高阶异兽是否会破解防御 or 绕路？
- [ ] 多人灵龛（组队）：暂不支持；但 hook 应预留 `authorized_chars: Vec<CharId>` 字段空间
- [ ] 阵石制作归 plan-zhenfa 还是 plan-niche-defense 自身维护材料 recipe？

---

## §9 进度日志

- **2026-04-27**：骨架立项。来源：`docs/plans-skeleton/reminder.md` plan-social-v1 § 灵龛防御模式节。plan-social-v1 `SpiritNiche` 已预留 `defense_mode: Option<DefenseModeId>` hook（2026-04-16 决策）；server/social 现有代码未实装任何防御逻辑。本 plan 为该 hook 的正式承接。
