# Bong · plan-alchemy-v1 · 模板

**炼丹 / 炼器专项**。HUD §10 快捷使用栏全是丹药，inventory 里 Consumable/Weapon 来源未定；本 plan 定义产出流程。

**交叉引用**：`plan-inventory-v1.md` · `plan-cultivation-v1.md` · `plan-HUD-v1.md §10`。

---

## §0 设计轴心

- [ ] 炼丹 / 炼器是玩家产出 Consumable/Weapon 的主要途径
- [ ] 非纯数值合成：有失败 / 品阶 / 丹毒
- [ ] 与修炼境界挂钩（炉火 = 境界消耗）
- [ ] 玩家可自行调整火候（玩法深度）

## §1 炼丹流程

```
选方 → 备料 → 起炉 → 火候调节（交互） → 成丹 / 炸炉
```

- [ ] 交互 UI（DynamicXmlScreen 或 BaseOwoScreen）
- [ ] 火候参数（温度 / 时长 / 真元投入）
- [ ] 成功率公式

## §2 丹药品阶

| 品阶 | 效果倍率 | 丹毒 |
|---|---|---|
| 下品 | 0.6 | 高 |
| 中品 | 1.0 | 中 |
| 上品 | 1.5 | 低 |
| 极品 | 2.0 | 无 |

## §3 炼器流程

- [ ] 材料 → 坯 → 淬炼 → 铭文 → 开光
- [ ] 品阶：凡 / 法 / 灵 / 仙
- [ ] ForgeWeapon 技能（combat §5 钩子）

## §4 丹方 / 图谱获取

- [ ] 初始解锁
- [ ] NPC 购买 / 任务 / 遗迹

## §5 数据契约

- [ ] RecipeStore / AlchemySessionStore
- [ ] Channel

## §6 实施节点

## §7 开放问题
