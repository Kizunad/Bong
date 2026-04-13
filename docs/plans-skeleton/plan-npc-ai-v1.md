# Bong · plan-npc-ai-v1 · 模板

**NPC 行为/派系专项**。server 已有 big-brain 骨架（僵尸），本 plan 扩展：修士/散修/宗门弟子/妖兽的 Scorer-Action 设计、代际/寿元、社交 AI。

**交叉引用**：CLAUDE.md「NPC AI」· `plan-server.md` · `plan-agent-v2.md`。

---

## §0 设计轴心

- [ ] big-brain Utility AI · Scorer → Action
- [ ] Position ↔ Transform 同步桥已就绪
- [ ] NPC 长期状态由 agent 推演，短期行为由 server ECS
- [ ] 不同 NPC 类型分 archetype

## §1 NPC 分类

| Archetype | 行为特点 | 代表 |
|---|---|---|
| 散修 | 漂流 / 寻机缘 / 避世 | |
| 宗门弟子 | 任务 / 日常 / 护山 | |
| 妖兽 | 领地 / 捕食 / 护崽 | |
| 凡人 | 耕作 / 商贸 / 恐惧修士 | |
| 仙家遗种 | 守护遗迹 / 考验 | |

## §2 Scorer / Action 列表

- [ ] 基础 Scorer：Hunger / Threat / CultivationDrive / Curiosity / Loyalty
- [ ] 基础 Action：Wander / Cultivate / Fight / Flee / Trade / Socialize
- [ ] 按 archetype 组合

## §3 代际 / 寿元

- [ ] NPC 出生 / 成长 / 衰老 / 死亡循环
- [ ] 寿元与境界挂钩
- [ ] 子嗣生成机制
- [ ] 与 agent 时代推演同步

## §4 派系 / 门派结构

- [ ] 派系数据模型
- [ ] 师承关系
- [ ] 声望系统
- [ ] 派系间关系矩阵（友/敌/中立）

## §5 社交 AI

- [ ] NPC ↔ NPC 交互（对话/交易/切磋）
- [ ] NPC ↔ 玩家（任务/师承/敌对）
- [ ] agent 如何影响 NPC 关系

## §6 数据契约

- [ ] NpcState / FactionState / RelationshipStore
- [ ] Channel

## §7 实施节点

## §8 开放问题
